---
phase: 121-local-round-trip-e2e
reviewed: 2026-08-24T01:26:42Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - Makefile
  - crates/pmcp-openapi-server/Cargo.toml
  - crates/pmcp-openapi-server/tests/common/mod.rs
  - crates/pmcp-openapi-server/tests/parity_replay.rs
  - crates/pmcp-openapi-server/tests/pmcp_package_pin.rs
  - crates/pmcp-openapi-server/tests/roundtrip_e2e.rs
  - crates/pmcp-package/src/slot/deviation.rs
findings:
  critical: 2
  warning: 10
  info: 4
  total: 16
status: issues_found
---

# Phase 121: Code Review Report

**Reviewed:** 2026-08-24T01:26:42Z
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Seven files reviewed: one Makefile target (`test-openapi-server`, chained into `test-all` →
`quality-gate`), one manifest, four integration-test files, and one doc-only change to
`crates/pmcp-package/src/slot/deviation.rs`.

`deviation.rs` is a pure documentation edit — no behavioural change, no finding.

The test code is unusually careful and most of its self-guards hold up under scrutiny. The
`capture_tool_surface` three-guard shape is well-founded: I confirmed
`ServerTester::list_tools` really does swallow the listing error
(`crates/mcp-tester/src/tester.rs:2901-2909`: `let _ = self.test_tools_list().await;` then
`self.tools.clone().unwrap_or_default()`), so without the explicit `test_tools_list()` status
assertion the parity comparison would be `[] == []`. I also confirmed the per-step gate in
`roundtrip_scenarios_replay_green_in_env_b` is sound — `ScenarioExecutor::execute` pushes a
failed `StepResult` *before* it breaks (`scenario_executor.rs:86-92`), so a failing step is
always visible in `step_results`. `expected_required_slots()` is a correct hand-transcription
of `tests/fixtures/london-tube.toml:55-73`, including the `backend-auth-mode` name/key trap.

Two blockers were found anyway, and they are both in the load-bearing infrastructure rather
than in the round-trip logic:

1. The new versioned `[dev-dependencies].pmcp-package = "0.2"` makes `cargo publish -p
   pmcp-openapi-server` **fail outright** — `pmcp-package` 0.2.x does not exist on crates.io
   (max published version is 0.1.1), and `pmcp-openapi-server` publishes at
   `release.yml:339`, a hundred steps *before* `pmcp-package` at `release.yml:440`. I
   reproduced the exact failure mode in an isolated two-crate probe.
2. The `REQUIRED_TEST_BINARIES` named-suite guard does not prove what its own comment says it
   proves. It matches cargo's `Running tests/<name>.rs` line, which cargo prints even when
   that binary reports `running 0 tests` — the precise false-green the same Makefile
   documents 200 lines further down for `run-era-matrix.sh`.

Ten warnings follow, concentrated in the manifest-shape structural guard (which has three
independent evasion routes and one self-corruption route), in two vacuous-filter shapes in
`parity_replay.rs`, and in env-var isolation that depends entirely on a `--test-threads=1`
flag nothing but the Makefile supplies.

## Critical Issues

### CR-01: Versioned dev-dep on an unpublished `pmcp-package 0.2` breaks the release

**File:** `crates/pmcp-openapi-server/Cargo.toml:85`
(reinforced by `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs:43,79-88`)

**Issue:**

```toml
pmcp-package = { version = "0.2", path = "../pmcp-package" }
```

`pmcp-package`'s highest published version on crates.io is **0.1.1** (verified against
`https://crates.io/api/v1/crates/pmcp-package`: `"max_version":"0.1.1"`, 2 versions). The
local crate is 0.2.0 and unpublished.

Cargo strips a `[dev-dependencies]` entry from the published manifest **only when it carries
no `version` key**. This one carries `version = "0.2"`, so it is retained and must resolve
against crates.io. I proved the failure with an isolated two-crate probe (a `[dev-dependencies]`
path dep with `version = "0.2"` on a name that does not exist on crates.io):

```
error: failed to prepare local package for uploading
Caused by:
  no matching package named `zz-nonexistent-helper-xyz` found
  location searched: crates.io index
```

This fails at `cargo package`/`cargo publish` time — *before* the upload — so the
`exclude = [..., "tests/"]` in this same manifest does **not** save it: excluding the test
targets removes the *consumers*, not the manifest entry.

The release ordering makes it unrecoverable:

- `.github/workflows/release.yml:339` — `Publish pmcp-openapi-server`
- `.github/workflows/release.yml:440` — `Publish pmcp-package`

`pmcp-openapi-server` publishes ~100 lines of workflow *before* the dependency it now needs.
The step's fallback only tolerates output containing `"already exists"`; `"no matching package
named"` does not match, so the step calls `::error::` and the whole release job dies — before
the `pmcp-package` step that would have published 0.2.0 ever runs.

Note this is **new scope**, not pre-existing: the other in-repo `pmcp-package = "0.2"` pins
(`crates/pmcp-agent`, `crates/pmcp-team-servers`, `crates/pmcp-cfn-renderer`, `cargo-pmcp`)
all sit *after* `release.yml:440`. This phase is the first to put the requirement in front of
it. `scripts/check-release-coverage.sh` is blind to workspace-excluded crates (CLAUDE.md
records this), so no existing gate catches it. The Cargo.toml comment defers "publish order,
the release ledger" to Phase 124 — but the breakage is live *now*, on the next tag push.

**Fix:** two changes, both required.

(a) Drop the `version` key so the dev-dep is stripped at publish time (this is the standard
form for a test-only path dep into an unpublished sibling):

```toml
# [dev-dependencies]
pmcp-package = { path = "../pmcp-package" }
```

and update the tripwire to assert *that* shape instead — the tripwire currently mandates the
publish-breaking form:

```rust
// tests/pmcp_package_pin.rs
// The dev-dep is intentionally PATH-ONLY: a `version` key would be retained in the
// published manifest and require pmcp-package on crates.io before this crate publishes.
#[test]
fn pmcp_package_dev_dep_is_path_only() {
    let manifest: toml::Value = toml::from_str(OPENAPI_SERVER_CARGO_TOML).expect("parse");
    let dep = manifest["dev-dependencies"]["pmcp-package"].as_table()
        .expect("[dev-dependencies].pmcp-package is the table form");
    assert!(dep.get("path").is_some(), "must be a path dep");
    assert!(
        dep.get("version").is_none(),
        "a `version` key here is retained in the published manifest and makes \
         `cargo publish -p pmcp-openapi-server` fail until pmcp-package 0.2 is on \
         crates.io — and pmcp-openapi-server publishes BEFORE it (release.yml:339 vs :440)"
    );
}
```

(b) If the versioned pin must be kept for pmcp.run's out-of-repo consumers, then
`release.yml`'s `Publish pmcp-package` step (and its indexing wait) must move to *before*
line 339, and `pmcp-package` 0.2.0 must be published first. Option (a) is strictly safer and
keeps the experimental 0.x crate from gating the core SDK release, which is the stated
policy in CLAUDE.md.

---

### CR-02: The `REQUIRED_TEST_BINARIES` guard cannot detect a named suite that runs zero tests

**File:** `Makefile:342-348`

**Issue:**

```make
	REQUIRED_TEST_BINARIES="parity_replay pmcp_package_pin roundtrip_e2e"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		if ! echo "$$out" | grep -q "tests/$$b\.rs"; then \
			echo "... required test binary '$$b' did not run — a nonzero total ($$ran) does not prove a NAMED suite ran"; \
```

The comment at `Makefile:315-323` states this "closes the count guard's OWN blind spot" and
that the check proves a *named suite ran*. It does not:

1. **Zero-test binaries still print the matched line.** Cargo prints
   `Running tests/roundtrip_e2e.rs (target/debug/deps/roundtrip_e2e-<hash>)` for every test
   target it executes, *including* one that compiles to nothing and reports
   `running 0 tests` / `test result: ok. 0 passed`. So a `#![cfg(feature = "…")]` added at the
   top of `roundtrip_e2e.rs`, a `#[cfg]`-gated module, or every test becoming `#[ignore]`
   leaves the grep satisfied while the suite executes nothing — and the summed `ran` stays
   comfortably nonzero from `parity_replay` + the lib tests, so the count guard does not fire
   either. This is the *exact* failure mode the same Makefile documents at lines 545-552 for
   `run-era-matrix.sh` ("omitting the flag compiles it to nothing and prints `running 0 tests`
   while exiting 0"). The new target reproduces the hole it claims to close.
2. **The pattern is not anchored to the `Running` line.** `tests/roundtrip_e2e.rs` also appears
   in every rustc diagnostic emitted for that file (`--> tests/roundtrip_e2e.rs:123:5`). This
   target sets no `-D warnings`, so a warning alone can satisfy the grep.

Given that PKG-04's deliverable *is* the regression net and this target is its only executor
inside `make quality-gate`, a guard that green-lights an unexecuted suite is a defect in the
phase's headline claim.

**Fix:** assert a **per-binary nonzero test count**, anchored to the `Running` line:

```make
	REQUIRED_TEST_BINARIES="parity_replay pmcp_package_pin roundtrip_e2e"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v b="tests/$$b.rs" '\
			$$1 == "Running" && $$2 == b { seen = 1; next } \
			seen && /^test result:/ { print $$4 + 0; exit } \
			END { if (!seen) print "-1" }'); \
		if [ "$$n" = "-1" ]; then \
			echo "$(RED)✗ required test binary '$$b' was never RUN (no 'Running tests/$$b.rs' line)$(NC)"; \
			exit 1; \
		fi; \
		if [ "$$n" -eq 0 ]; then \
			echo "$(RED)✗ required test binary '$$b' ran but executed 0 tests — a cfg gate or an #[ignore] sweep silently switched the suite off while the total ($$ran) stayed nonzero$(NC)"; \
			exit 1; \
		fi; \
	done; \
```

## Warnings

### WR-01: The "STRONG form" identity-bearing assertions pass for three unrelated reasons

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:966-982`

**Issue:** The comment claims:

> The STRONG form of the contrast: two DIFFERENT credentials still yield `None`. Pairing a
> slot against a clone of ITSELF would prove nothing […] that test would pass even if the
> identity-bearing short-circuit did not exist

The chosen alternative has the same weakness it diagnoses. `rotated` is
`SlotType::Secret { name: "TFL_APP_KEY_ROTATED" }` and `packed_secret.slot` is
`SlotType::Secret { name: "TFL_APP_KEY" }`. Tracing `detect_deviation`
(`crates/pmcp-package/src/slot/deviation.rs:46-64`), the call returns `None` via **three
independent paths**, only one of which is the property under test:

1. `classify(..) != BehaviorRelevant` — the short-circuit being tested (`:47-51`);
2. `tested.key() != proposed.key()` — the *names differ*, so this fires too (`:52-54`);
3. `tested.tested_value()?` — `SlotType::Secret` carries no `tested_value`, so this fires
   as well (`:55`).

Delete the short-circuit at `:47-51` entirely and both assertions still pass. The assertion
therefore cannot detect the regression its comment names.

This is unfixable with `Secret` inputs — no `Secret` pair can ever produce `Some`, which is
precisely the "structurally incapable" claim the assertion *message* makes and which is
genuinely proven. The defect is the doc comment overselling it.

**Fix:** either correct the comment to say what is actually proven, or isolate the
short-circuit with a same-name pair plus a *behaviour-relevant control* that shows the same
call shape does fire:

```rust
// Control: the SAME comparison shape on a behavior-relevant slot DOES fire, so the
// `None` above is attributable to the identity-bearing family and not to the call shape.
let a = SlotType::Endpoint { name: "N".into(), tested_value: "x".into() };
let b = SlotType::Endpoint { name: "N".into(), tested_value: "y".into() };
assert!(detect_deviation(&a, &b).is_some(), "the control must fire");
// Same NAME, identity-bearing family: only the short-circuit / absent tested_value can
// explain the None.
let s1 = SlotType::Secret { name: "TFL_APP_KEY".into() };
let s2 = SlotType::Secret { name: "TFL_APP_KEY".into() };
assert!(detect_deviation(&s1, &s2).is_none());
```

---

### WR-02: `parity_live_tfl`'s discovery gate is a filter that already matches a dead prefix

**File:** `crates/pmcp-openapi-server/tests/parity_replay.rs:397-407`

**Issue:**

```rust
let discovery_failed: Vec<_> = result.step_results.iter()
    .filter(|s| s.step_name.starts_with("List ") || s.step_name.starts_with("Tools include"))
    .filter(|s| !s.success)
    ...
assert!(discovery_failed.is_empty(), ...);
```

There is no floor on how many steps the *first* filter selected. If it selects zero, the
assertion is vacuously true and the entire live parity gate is silently off — the exact
"filter whose scan can match nothing" shape this phase's own header forbids.

This is not hypothetical: `"Tools include"` **already matches nothing**. The current step
names in `tests/fixtures/london-tube-scenarios.yaml` are `List available tools`,
`List available resources`, `List available prompts`, `Code Mode tool validate_code is
present…`, `get-tube-status returns…`, `disrupted-lines-with-detail surfaces…`. Half the
predicate is dead today; renaming the three `List available …` steps kills the other half and
turns the assertion into a no-op with no signal.

**Fix:** assert the selection floor before the success gate.

```rust
let discovery: Vec<_> = result.step_results.iter()
    .filter(|s| s.step_name.starts_with("List "))
    .collect();
assert!(
    discovery.len() >= 3,
    "the live discovery gate selected only {} steps — a renamed scenario step switches \
     this assertion off entirely. step_names={:?}",
    discovery.len(),
    result.step_results.iter().map(|s| &s.step_name).collect::<Vec<_>>()
);
let discovery_failed: Vec<_> = discovery.iter().filter(|s| !s.success).collect();
assert!(discovery_failed.is_empty(), "...");
```

(and drop or repair the dead `"Tools include"` prefix).

---

### WR-03: `london_tube_parity_through_real_binary_path` has no step-count floor, unlike its new sibling

**File:** `crates/pmcp-openapi-server/tests/parity_replay.rs:256-268`

**Issue:** `roundtrip_scenarios_replay_green_in_env_b` (`roundtrip_e2e.rs:1049-1053`) added the
`result.steps_total > 0` floor with a comment explaining that an empty step list makes the
`failed.is_empty()` gate vacuously true. The identical gate in `parity_replay.rs` — the file
this phase *modified* — never got it. The two tests share `tests/common/mod.rs` and the same
scenario fixture; the asymmetry means an emptied or unparsed `london-tube-scenarios.yaml`
degrades one test loudly and the other silently. (The `!recorded.is_empty()` check at
`:277-281` is a partial backstop, but it is a *different* claim and would not survive a future
edit that reorders it.)

**Fix:** add the same floor immediately after the `execute()` call:

```rust
assert!(
    result.steps_total > 0,
    "the scenario contract must contain at least one step — an empty step list makes \
     the per-step gate below vacuously true"
);
```

---

### WR-04: The manifest-shape sanitizer does not handle `/* */` block comments — and a `"` inside one blanks the rest of the file

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1571-1616` (`sanitize_line` /
`sanitize_source`)

**Issue:** `sanitize_line` handles `//` line comments (`starts_line_comment`), string
literals, raw strings and char literals — but **not** `/* … */`. Two consequences:

1. **False positive.** A deny-listed token inside a block comment within an assertion span
   fires the guard, since block-comment text is emitted as code. The file already contains a
   block comment (`ScenarioExecutor::new(&mut tester, true /* detailed */)` at `:1037`), so
   the style is in use here.
2. **Silent self-blinding (the worse one).** A single unpaired `"` inside a block comment —
   e.g. `/* the "digest" field */` or an apostrophe-free `/* don"t */` — is read as the start
   of a string literal. `close_string` fails to find a terminator, `sanitize_line` returns
   `(out, true)`, and `sanitize_source` then carries `in_string = true` across **every
   subsequent line** until another `"` appears. Those lines are blanked, so any deny-listed
   token in them is invisible to the check and any `assert!` in them is not even recognised
   as a span start. The span-floor self-check would only fire if enough spans vanished to drop
   below 32 of ~42.

Unbalanced delimiters in block comments corrupt `delimiter_delta` the same way.

**Fix:** carry block-comment state alongside string state.

```rust
fn sanitize_source(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut in_string, mut in_block) = (false, false);
    for line in source.lines() {
        let (sanitized, s, b) = sanitize_line(line, in_string, in_block);
        out.push(sanitized);
        in_string = s;
        in_block = b;
    }
    out
}
```

with `sanitize_line` consuming `/* … */` (including nested `/*`, which Rust permits) before
the string branch and returning the updated block flag. Add a unit test that a `"` inside a
block comment does not blank the following lines.

---

### WR-05: The structural guard scans only assertion macro spans — `panic!` / `expect` sites are invisible

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1657-1671, 1713-1757`

**Issue:** `scan_assertion_spans` starts a span only on one of `ASSERTION_MACROS`
(`assert!`, `assert_eq!`, `assert_ne!`, `debug_assert*`). The test's title claims *"this file
asserts NOTHING about the package's on-disk representation"*, but this file already routes two
of its comparisons through non-macro forms that the scanner never reads:

- `:742-747` — `if let Err(mismatch) = compare_tool_surfaces(..) { panic!(...) }`
- `:1295-1309` — `match &err { … other => panic!(...) }`
- `.expect(...)` / `.expect_err(...)` throughout (e.g. `:341`, `:1285-1289`)

A future edit adding `let idx = b_layout.read_index().expect("…");` followed by a `panic!` on
its contents introduces exactly the manifest coupling D-09 forbids, with `read_index` — a
deny-listed token — sitting in plain sight, and the guard stays green. The header's
"REGRESSION LINT, not semantic proof" caveat covers *aliased* access, not this: these are the
listed tokens in an unscanned syntactic position.

**Fix:** extend `ASSERTION_MACROS` to the panic/expect family, which are assertions in this
file's idiom:

```rust
const ASSERTION_MACROS: [&str; 9] = [
    "assert!", "assert_eq!", "assert_ne!",
    "debug_assert!", "debug_assert_eq!", "debug_assert_ne!",
    "panic!", ".expect(", ".expect_err(",
];
```

and re-measure `MANIFEST_SHAPE_GUARD_SPAN_FLOOR` with the same margin rule the constant's doc
prescribes.

---

### WR-06: Deny-list tokens `annotations` and `media_type` collide with *served* MCP fields

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1423-1443`

**Issue:** `annotations` is both an OCI manifest field **and** an MCP `ToolInfo`/`ToolMetadata`
field — this very file constructs `ToolMetadata { …, annotations: Some(json!(…)) }` at `:211-215`,
and `parity_replay.rs:112-120` asserts on a served tool's `annotations.cost_hint`. Likewise
`media_type` is a plausible served/content field name. The guard's stated contract is "every
assertion here must be on SERVED BEHAVIOUR", yet asserting on a served tool's `annotations`
would turn it red.

A guard that fires on the behaviour it exists to permit is a guard that gets deleted the first
time it is inconvenient — the exact outcome the header at `:1705-1712` warns about.

**Fix:** scope the tokens to the accessor path rather than the bare field name, mirroring the
existing `layers()` / `read_index` entries:

```rust
    // the annotations field ON THE MANIFEST/INDEX, not on a served ToolInfo
    ".annotations", // as reached from a manifest/index/descriptor binding
```

or, more robustly, list the manifest-side *type/accessor* names (`OciLayout`, `ImageIndex`,
`Descriptor`, `read_index`, `read_blob`, `manifests()`, `layers()`) and drop the ambiguous
bare field names entirely.

---

### WR-07: `handle_a.abort()` does not guarantee environment A is gone before B's variables are written

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:722-731`

**Issue:** The comment asserts a guarantee the code does not provide:

```rust
    // A is fully gone before B's variables are written (D-10) — the two cannot
    // be alive simultaneously under env-var slot resolution.
    handle_a.abort();
```

`JoinHandle::abort()` only *requests* cancellation; the task is cancelled at its next await
point and the handle is never awaited. Nothing here observes A's termination, so A's server
task can still be alive (and its listener still bound) when `serve_environment` writes B's
`TFL_APP_KEY`/`TFL_BASE_URL` two lines later.

The consequence is benign *today* — A resolved its slots at assembly time and nothing queries
A afterwards — but the comment is what a future maintainer will trust when adding a check that
does depend on the sequencing.

**Fix:** make the claim true, or narrow it.

```rust
handle_a.abort();
// abort() only REQUESTS cancellation; await the handle so A is provably gone
// (JoinError::is_cancelled is the expected outcome) before B's variables are written.
let _ = handle_a.await;
drop(tester_a);
drop(backend_a);
```

---

### WR-08: `--test-threads=1` is enforced only by the Makefile; a bare `cargo test -p` races on `set_var`

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:406-407, 1346-1398`;
`crates/pmcp-openapi-server/tests/common/mod.rs:34-64`

**Issue:** `serve_environment` calls `std::env::set_var` and `EnvVarGuard` calls
`set_var`/`remove_var`. `tfl_env_lock` serializes the tests that *take it*, but
`env_var_guard_restores_prior_state_including_on_panic` deliberately takes no lock
(`:1322-1328`: "uniqueness […] is what stands in for a lock"). Uniqueness of the *variable
name* prevents value corruption; it does **not** prevent the process-global `setenv`/`getenv`
data race, which is unsound in a multi-threaded process regardless of which key is written.

The only thing forcing single-threaded execution is `-- --test-threads=1` in
`Makefile:333`. `org-gate-checks.yml`'s `workspace-test`, any `cargo test -p
pmcp-openapi-server` a developer types, and `cargo nextest` (process-per-test, but with
`ServerTester`/tokio threads reading env inside each) all bypass it. The test binary itself
carries no enforcement.

**Fix:** enforce it in-binary rather than only in the build system.

```rust
// tests/common/mod.rs
/// Fails loudly when this binary is not running single-threaded — the process-global
/// env mutations below are unsound otherwise, and the Makefile flag is the ONLY thing
/// supplying it today.
pub fn assert_single_threaded() {
    if let Ok(n) = std::env::var("RUST_TEST_THREADS") {
        assert_eq!(n, "1", "this binary must run with --test-threads=1");
    }
}
```

or, better, route *every* env mutation in the binary (including the guard-probe test) through
`tfl_env_lock` so a single mutex covers all writers regardless of thread count.

---

### WR-09: `serve_environment` writes `TFL_*` with bare `set_var` and never restores, while `EnvVarGuard` exists in the same module

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:406-407`

**Issue:**

```rust
    std::env::set_var("TFL_APP_KEY", app_key);
    std::env::set_var("TFL_BASE_URL", base_url);
```

Every test that serves an environment leaves a **dead wiremock URI** (its `MockServer` is
dropped at test end) in `TFL_BASE_URL` for the remainder of the binary. The knock-on effect is
concrete: `degraded_env_b_unfilled_slot_is_reported` does
`EnvVarGuard::unset("TFL_BASE_URL")` at `:1278`, which captures that dead URI as `previous`
and faithfully restores it on drop — so the guard's stated purpose ("the variable then leaks
into every later test in this binary — a green run against the wrong endpoint",
`common/mod.rs:216-222`) is defeated by the very leak the guard was written to prevent, just
one level up.

`common/mod.rs:44-48` justifies non-restoration for `parity_replay.rs` on the grounds that "no
other test in that binary reads them". That is **not** true of `roundtrip_e2e.rs`: four tests
read them.

**Fix:** have `serve_environment` return the guards along with the bound address so the caller
owns the restoration lifetime:

```rust
async fn serve_environment(
    config_path: &Path, base_url: &str, app_key: &str,
) -> (SocketAddr, JoinHandle<()>, EnvVarGuard, EnvVarGuard) {
    let key_guard = EnvVarGuard::set("TFL_APP_KEY", app_key);
    let url_guard = EnvVarGuard::set("TFL_BASE_URL", base_url);
    // ... existing body ...
    (bound, handle, key_guard, url_guard)
}
```

Callers bind them as `let (bound, handle, _k, _u) = ...`, which restores on both the normal
and the panicking path.

---

### WR-10: `test-openapi-server` buffers all output — no streaming, and a hang produces zero diagnostics

**File:** `Makefile:333-336`

**Issue:** `@out=$$(… 2>&1); status=$$?; echo "$$out";` captures the entire run and prints it
only after completion. For this crate that is a multi-minute build plus a serialized
integration run that stands up HTTP servers. If a test hangs — a realistic outcome here, since
`serve_environment`'s readiness loop and `ScenarioExecutor` both do network I/O — CI shows
nothing at all until the job-level timeout kills it, with no partial output to diagnose from.

This is inherited from `test-tester`/`test-cargo-pmcp`, but those run far faster and do no
network I/O.

**Fix:** tee instead of capture, so output streams and is still greppable:

```make
	@log=$$(mktemp); \
	set -o pipefail; \
	RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p pmcp-openapi-server \
	  -- --test-threads=1 2>&1 | tee "$$log"; \
	status=$$?; \
	if [ $$status -ne 0 ]; then rm -f "$$log"; exit $$status; fi; \
	out=$$(cat "$$log"); rm -f "$$log"; \
	... existing guards over "$$out" ...
```

(`set -o pipefail` is required for the `tee` pipeline to preserve cargo's status; `SHELL` is
already pinned to bash at `Makefile:8`.)

## Info

### IN-01: Two tautological assertions

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:707-710, 770`

`assert_ne!(DUMMY_APP_KEY, ENV_B_APP_KEY, …)` compares two `const &str` literals — it can only
fail if someone edits the constants, never as a result of behaviour. `assert_ne!(round_trip.a_layout_root,
round_trip.b_layout_root)` at `:770` restates an assertion already made inside
`pack_a_and_move_to_b` (`:323-327`) and its own comment admits it exists "so the struct's two
`PathBuf` fields are read rather than merely stored" — i.e. to silence a dead-field warning.
Both are harmless as drift guards but they inflate the span count that
`MANIFEST_SHAPE_GUARD_SPAN_FLOOR` is calibrated against. Prefer a `const _: () = assert!(…)`
for the former and `#[allow(dead_code)]` on the struct field for the latter.

---

### IN-02: `copy_dir_recursive` mishandles symlinks

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:231-245`

`entry.file_type()` reports `is_dir() == false` for a symlink pointing at a directory, so the
`else` branch calls `std::fs::copy`, which fails with "Is a directory". OCI layouts contain no
symlinks today so this is latent, but the function's doc claims it is deliberately
name-agnostic and copies "whatever entries it finds" — which is not true of symlinks. Consider
either following symlinks explicitly (`std::fs::metadata` instead of `entry.file_type()`) or
asserting their absence.

---

### IN-03: The endpoint re-baking check is whitespace-sensitive

**File:** `crates/pmcp-openapi-server/tests/common/mod.rs:142-145`

```rust
!config_text.contains(r#"base_url = "https://api.tfl.gov.uk""#)
```

matches only that exact spacing. `base_url="https://api.tfl.gov.uk"` or
`base_url  = "https://api.tfl.gov.uk"` both slip through, so the "must not re-bake the
endpoint" guard is defeated by a formatting choice. Prefer asserting over the *parsed*
`cfg.backend.base_url` (already done on the line above) and dropping the text check, or
normalising whitespace before the `contains`.

---

### IN-04: `SPAN_LINE_CAP` silently truncates the deny-list scan of long spans

**File:** `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:1464-1469, 1635-1654`

A span exceeding 40 lines is ended at the cap and counted, so any deny-listed token past line
40 of that span is never examined, and `idx += span.line_count` resumes mid-construct
(potentially skipping a subsequent assertion). The doc explains the trade-off but the guard
does not report when the cap is actually hit. Emitting a warning — or asserting that no span
reaches the cap — would make the truncation visible rather than silent.

---

_Reviewed: 2026-08-24T01:26:42Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
