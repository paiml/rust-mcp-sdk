---
phase: 117-agents-tester-v1-severability
plan: 11
subsystem: mcp-tester
tags: [dual-run, era-detection, conformance, clnt-04, additivity]
requires:
  - "117-08 (era-deltas.yaml baseline + era_diff parser)"
  - "117-07 (the classification contract, cited not re-derived)"
  - "117-03 (report_compat.rs additivity goldens)"
provides:
  - "ServerTester::with_protocol_version — era pinning without touching new()'s arity"
  - "detect_eras -> EraSupport (Dual/V1Only/V2Only/Unreachable/NoEraSpoken)"
  - "ServerTester::raw_jsonrpc_probe{,_with_session} / raw_verb_probe — the raw wire seam"
  - "era_observations: 14 probes emitting stable ObservationIds"
  - "era_diff::{DualRunReport, compare_eras, build_dual_run_report}"
  - "ConformanceRunner::run_dual"
  - "mcp-tester conformance --dual-run"
affects:
  - "crates/mcp-tester (library API — purely additive)"
  - "cargo-pmcp (verified UNAFFECTED: build + check --tests both green)"
tech-stack:
  added: []
  patterns:
    - "New top-level struct instead of extending TestReport (post_deploy_report precedent, A-D11)"
    - "Stable observation IDs as the join key, never display names"
    - "Contract CITATION across crates that cannot share code"
key-files:
  created:
    - crates/mcp-tester/src/era_observations.rs
    - crates/mcp-tester/tests/dual_run.rs
  modified:
    - crates/mcp-tester/src/tester.rs
    - crates/mcp-tester/src/era_diff.rs
    - crates/mcp-tester/src/conformance/mod.rs
    - crates/mcp-tester/src/conformance/core_domain.rs
    - crates/mcp-tester/src/main.rs
    - crates/mcp-tester/src/lib.rs
decisions:
  - "Session-leak mitigation: DELETE (Transport::close on a retained transport handle), not client reuse"
  - "117-07's classification contract is shared by CITATION — pmcp-agent is not and must not become an mcp-tester dependency"
  - "Reserved _meta keys are spelled as literals in tester.rs, guarded by a non-circular SDK drift tripwire, because pmcp::testing is feature-gated and Cargo.toml must stay byte-identical"
  - "The era-deltas.yaml baseline was NOT edited to make the comparison quiet"
metrics:
  duration_min: 48
  completed: 2026-08-08
  tasks: 3
  commits: 3
  files_changed: 8
  lines: "+3855 / -34"
---

# Phase 117 Plan 11: mcp-tester dual-run era comparison Summary

`mcp-tester` now reaches v2 without `initialize`, auto-detects a dual-era server, runs the suite
twice, and diffs the two runs against the expected-difference baseline by stable observation ID —
all strictly additively, and it immediately found a real server-side severance gap.

## Commits

| Task | Commit | What |
|------|--------|------|
| 1 | `1bd44f21` | Era-pinned `ServerTester`, raw wire-probe seam, non-leaking `detect_eras`, era-aware `core_domain.rs` |
| 2 | `5d80fa07` | `era_observations.rs` (14 probes), `DualRunReport` + `compare_eras`, `run_dual` |
| 3 | `2451527a` | `--dual-run` flag, `tests/dual_run.rs` (12 live-socket tests), two Rule-1 probe fixes |

## The headline finding

**The pmcp SERVER still answers a well-formed `initialize` on the `2026-07-28` wire.**

Baseline ERA-01 records `initialize` as `served` on v1 and **`absent`** on v2. Measured against a
real opted-in in-process pmcp server:

```
initialize (params: protocolVersion only)                  -> HTTP 404, JSON-RPC -32601
initialize (params: protocolVersion + clientInfo + caps)   -> HTTP 200, RESULT
    result = { "protocolVersion": "2025-11-25",            <- v1 field
                "resultType": "complete",                  <- v2 field
                "_meta": { "io.modelcontextprotocol/serverInfo": {...} } }
```

The served result is a **mixed envelope**: a v1 `protocolVersion` alongside v2
`resultType`/`_meta.serverInfo`. ERA-01's own `source` column cites only CLIENT-side artifacts
(`REQUIREMENTS.md:911 (CLNT-01)`, `src/client/mod.rs:726-741 v2_synthetic_initialize_result`). The
client's `initialize` is indeed local and synthetic; the **server** side was never severed.

**The baseline was deliberately NOT edited.** It is the phase's spec artifact, and rewriting it so
the comparison goes quiet is the re-recorded-golden anti-pattern `report_compat.rs` warns about. The
tester reporting ERA-01 as MISSING is the tool working. Pinned by
`the_server_still_answers_initialize_on_the_v2_wire`, whose failure message tells a future reader
exactly what to update when 117-12/117-13 sever the server side.

This is a hand-off item for **117-12 / 117-13 / Phase 118**, not something this plan may fix — the
plan's scope guard is explicit that a domain turning out to be initialize-dependent "is a FINDING to
report with the evidence, not a licence to widen the edit."

## Live dual-run result (real pmcp server, `--domain core`)

`9 EXPECTED / 0 UNEXPECTED / 5 MISSING`.

| Class | Observation IDs |
|-------|-----------------|
| EXPECTED (9) | `header.last_event_id`, `header.mcp_method_and_name`, `header.mcp_session_id`, `http.verb.get_delete`, `method.resources_subscribe`, `method.server_discover`, `result.cache_scope`, `result.result_type`, `result.server_info` |
| UNEXPECTED (0) | — |
| MISSING (5) | `method.initialize` (the finding above), `http.status.error_code_mapping`, `capability.tasks_location` [prov], `method.tasks_list` [prov], `method.subscriptions_listen` |

Of the five MISSING rows, **one is the real server finding** (`method.initialize`), **three are
fixture limitations** — the test server has no task store and advertises no subscribable
capabilities, so `capability.tasks_location`, `method.tasks_list` and `method.subscriptions_listen`
cannot reproduce against it (ERA-13's own note calls a `-32601` from a capability-less server
SKIPPED-conformant) — and **one needs adjudication**: `http.status.error_code_mapping`'s probe rule
assumes the v1 "legacy table" returns HTTP 200 for a JSON-RPC error, but the measured v1 path
returns a mapped non-200 status. That is either a baseline wording issue or a probe-rule refinement
for Phase 118; it is recorded rather than papered over.

## Task 1 — era-pinned tester and non-leaking detection

### `ServerTester::new` — before and after (IDENTICAL)

```rust
// BEFORE (0.7.0) and AFTER (this plan) — byte-identical
pub fn new(
    url: &str,
    timeout: Duration,
    insecure: bool,
    api_key: Option<&str>,
    force_transport: Option<&str>,
    http_middleware_chain: Option<
        std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>,
    >,
) -> Result<Self> {
```

Era pinning arrives as a `#[must_use]` consuming builder instead:
`with_protocol_version(self, ProtocolVersion) -> Self`. All five `cargo-pmcp` call sites compile
unchanged.

### THE ONE era branch

`test_initialize` has seven call sites inside `tester.rs` plus the Core domain. Rather than branch at
each, the branch lives once at the top of `test_initialize`, which delegates to
`establish_v2_connection` when pinned to v2. That function sends **zero** handshake bytes —
`ClientBuilder::build` marks a v2 client already-initialized, so `server/discover` is the first and
only request.

### Session-leak mitigation: **DELETE** (not reuse)

Chosen because it is directly observable. `probe_v1` retains a `StreamableHttpTransport` handle (a
clone shares the `Arc<RwLock<config>>`, so it sees the session id the response installed) and calls
`Transport::close`, which issues the spec `DELETE` and clears the id. Reuse would have left N
sessions alive after N detect+run cycles and been much harder to assert.

Asserted by `era_detection_does_not_leak_a_session_per_invocation`: 3 detections against a
session-minting stub produce **3 mints and 3 DELETEs**. (The stub exists for this one case only —
pmcp exposes no session-count accessor, so counting what the server was asked to do is the only way
to observe the mitigation.)

### Contract sharing

`pmcp-agent` is not, and must not become, an `mcp-tester` dependency (experimental 0.x vs published
0.7.0). So the two crates share the **written contract by citation**: `endpoint_is_reachable`'s doc
in `tester.rs` names `THE CLASSIFICATION CONTRACT` block on `endpoint_is_reachable` in
`crates/pmcp-agent/src/invoker/factory.rs` as the single authored copy and restates only its rule.
Classification uses a typed reachability `bool` established *before* any error exists —
`grep -n 'to_string().contains\|\.contains("' crates/mcp-tester/src/tester.rs` shows no match inside
any era-classification function (the hits are pre-existing tool heuristics plus one content-type
check in `extract_jsonrpc_envelope`).

### `core_domain.rs` v2 branches (verbatim — no synthesised `InitializeResult`)

C-01's v2 path asserts the connection holds **no** `InitializeResult` and then probes the wire:

```rust
    if tester.server_info().is_some() {
        return TestResult::failed(
            name,
            TestCategory::Core,
            start.elapsed(),
            "a v2 connection must carry NO InitializeResult; one is present, which \
             means an initialize handshake was performed or synthesised",
        );
    }
```

C-04's v2 path reads the projection at its **v2 location** and flags the v1 spelling:

```rust
    let extensions: Vec<String> = caps
        .extensions
        .as_ref()
        .map(|ext| { let mut keys: Vec<String> = ext.keys().cloned().collect(); keys.sort(); keys })
        .unwrap_or_default();
    ...
    if caps.tasks.is_some() {
        return TestResult::warning(
            name, TestCategory::Core, start.elapsed(),
            format!(
                "the v2 projection still advertises `capabilities.tasks`; on the \
                 2026-07-28 wire the tasks surface belongs at \
                 extensions[{TASKS_EXTENSION_KEY}] and both v1 spellings are \
                 suppressed (ERA-10). Advertised: {details}"
            ),
        );
    }
```

`grep -n 'InitializeResult' crates/mcp-tester/src/conformance/core_domain.rs` returns **2** hits,
both prose (the module doc's prohibition and the failure message above). No construction anywhere.

C-02 and C-03 stayed single-bodied by reading the connection through new era-agnostic accessors
(`negotiated_protocol_version`, `negotiated_server_info`), so the v1 path is unchanged. The plan
predicted C-03 would otherwise fail spuriously on v2; it would have.

## Task 2 — observations and the comparison

### The full `ObservationId` set (14), two-direction coverage

```
capability.tasks_location   header.last_event_id           header.mcp_method_and_name
header.mcp_session_id       http.status.error_code_mapping http.verb.get_delete
method.initialize           method.resources_subscribe     method.server_discover
method.subscriptions_listen method.tasks_list              result.cache_scope
result.result_type          result.server_info
```

Coverage is asserted **both ways** by `every_baseline_entry_has_a_probe` and
`every_probe_has_a_baseline_entry`, each naming the unmatched IDs on failure. A baseline entry with
no probe could only ever report MISSING forever; a probe with no entry would report every run as
UNEXPECTED.

### The JOIN rule

For each id in `v1 ids ∪ v2 ids ∪ baseline ids`:

| differed? | delta? | values agree? | class |
|-----------|--------|---------------|-------|
| yes | yes | yes | **EXPECTED** |
| yes | yes | no  | **UNEXPECTED** (differs, but not in the documented way) |
| yes | no  | —   | **UNEXPECTED** (no baseline entry) |
| no  | yes | —   | **MISSING** (recorded delta no longer reproduces, or never observed) |
| no  | no  | —   | not reported |

"Differed" requires **both** sides established; an `Unavailable` observation is "we could not tell",
never a difference, so a flaky probe cannot manufacture findings. Values are compared as canonical
tokens drawn from the baseline's own `v1:`/`v2:` vocabulary.

Tested directly by `join_rule_matches_a_recorded_delta_and_rejects_a_mismatched_one` with one
matching and one non-matching case, so a silently-never-matching rule fails a test. Plus:
`an_unrecorded_difference_is_unexpected`, `a_delta_that_no_longer_reproduces_is_missing`,
`a_delta_never_observed_is_missing_not_silently_dropped`,
`an_unavailable_observation_is_not_a_difference`,
`provisional_missing_is_reported_distinctly_from_non_provisional`,
`render_reports_missing_and_unexpected_as_distinct_categories`,
`an_empty_difference_list_is_surfaced_as_suspicious`, `classification_order_is_deterministic`.

### No display-name matching

`grep -n 'TestResult\|\.name' crates/mcp-tester/src/era_diff.rs` returns **7** hits, at lines 27, 28,
35, 44, 80, 335, 343 — **all seven are doc comments** explaining the A-D11 rule. Zero `.name` reads
and zero `TestResult` uses in the classification path.

### Anti-vacuity guard

An empty difference list, with two non-empty suite reports, sets `DualRunReport::suspicion` rather
than rendering "all clear" — because the baseline lists 14 by-design differences, so two era runs
against a dual-era server MUST differ. Suppressed when either suite ran zero tests (a comparison over
two empty reports is merely empty, not suspicious). Covered live, too:
`dual_run_against_a_dual_era_server_classifies_against_the_baseline` asserts a non-empty list, `>= 6`
EXPECTED rows, and **zero** UNEXPECTED rows against a stock pmcp server.

## Task 3 — the flag

```
      --dual-run
          Detect whether the server serves BOTH MCP eras and, if so, run the suite twice and print a v1-vs-v2 comparison.

          OFF by default. With the flag absent, nothing about the output changes — single-run output stays byte-identical to 0.7.0, which is the additivity contract `tests/report_compat.rs` pins.

          Differences are classified against the checked-in expected-difference baseline (`baselines/era-deltas.yaml`): a listed delta is correct by design, an unlisted one is a finding, and a listed one that no longer reproduces is also a finding. Against a server that serves only one era this degrades to a single run and says so.
```

### Both live-binary invocations (real dual-era server, `--domain core`)

**`conformance <url> --domain core` → exit `0`** (single-run path, unchanged):

```
Core:
  ✓ Core: initialize handshake                  115ms Server: era-fixture v0.0.0, Protocol: 2025-11-25
  ✓ Core: protocol version                            Protocol version: 2025-11-25
  ✓ Core: server info                                 era-fixture v0.0.0
  ✓ Core: capabilities structure                      tools
  ✓ Core: unknown method returns -32601               Server rejected unknown method (error code not available through transport)
  ✓ Core: malformed request handling                  Server returned error for malformed request
Overall Status: PASSED
```

No `ERA COMPARISON` string appears anywhere in that output.

**`conformance <url> --domain core --dual-run` → exit `0`**:

```
DUAL-RUN ERA COMPARISON
============================================================
Era support : dual
v1 suite    : 6 tests, 0 failed
v2 suite    : 1 tests, 1 failed
Differences : 9 expected, 0 unexpected, 5 missing

MISSING (5)
  capability.tasks_location (ERA-10) [PROVISIONAL]
  http.status.error_code_mapping (ERA-14)
  method.initialize (ERA-01)
  method.subscriptions_listen (ERA-13)
  method.tasks_list (ERA-09) [PROVISIONAL]

EXPECTED (9)
  header.last_event_id (ERA-05) ... method.server_discover (ERA-02) ... result.server_info (ERA-08) [provisional]
```

`v2 suite: 1 tests, 1 failed` is the finding surfacing: C-01 fails and the Core domain stops, exactly
as `run_core_conformance` has always done on a failed C-01. The **exit code stays 0** because
`--dual-run` returns the v1 report, so the process-exit contract does not change meaning when the
flag is passed.

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 1 - Bug] C-01's v2 probe sent a MALFORMED `initialize`, so it passed for the wrong reason**

- **Found during:** Task 3, when the live tests disagreed with each other
- **Issue:** The probe sent `params: { protocolVersion }` only. MEASURED: that is refused `-32601`
  by the **typed parse, before dispatch**. Refusing a malformed request is not evidence the METHOD
  is gone — so C-01 would have certified a server that serves `initialize` perfectly well, which is
  the exact regression it exists to catch.
- **Fix:** The probe now sends a well-formed `initialize` (`protocolVersion` + `clientInfo` +
  `capabilities`), with the reasoning in a comment at the call site. Both shapes are asserted in
  `the_server_still_answers_initialize_on_the_v2_wire` so the distinction cannot be lost again.
- **Files:** `crates/mcp-tester/src/conformance/core_domain.rs`
- **Commit:** `2451527a`

**2. [Rule 1 - Bug] The raw v1 probes carried no `Mcp-Session-Id`, contaminating four observations**

- **Found during:** Task 3, from the live classification dump
- **Issue:** A stateful v1 server refuses every non-initialization request that arrives without a
  session (`400`, "Session ID required for non-initialization requests",
  `streamable_http_server.rs:1666-1673`). Four probes were observing that refusal and reporting it
  as the fact they were sent to establish.
- **Fix:** Added `raw_jsonrpc_probe_with_session`; `observe()` now sends the initialize probe first,
  captures its session header, and threads it through every subsequent v1 probe. The initialize
  response is sent once and classified twice (ERA-01 and ERA-03).
- **Measured effect:** the live comparison went from **6 EXPECTED / 3 UNEXPECTED / 5 MISSING** to
  **9 EXPECTED / 0 UNEXPECTED / 5 MISSING**. In particular `method.server_discover` moved from a
  bogus `error:-32600` to the baseline-matching `error:-32601`. **An earlier draft of this summary
  would have reported that `-32600` as a server defect; it was my probe's fault.**
- **Files:** `crates/mcp-tester/src/tester.rs`, `crates/mcp-tester/src/era_observations.rs`
- **Commit:** `2451527a`

**3. [Rule 1 - Bug] `probe_subscriptions_listen` read any non-`-32601` refusal as "served"**

- **Issue:** Same class of mis-inference — a `-32600` parse failure would have been recorded as a
  served, capability-gated stream.
- **Fix:** Only a JSON-RPC `result` counts as served; every refusal is recorded as its own
  `error:{code}` token.
- **Files:** `crates/mcp-tester/src/era_observations.rs`
- **Commit:** `2451527a`

**4. [Rule 3 - Blocking] Disk exhaustion faked a `cargo-pmcp` test-compile failure**

- **Found during:** Task 2 verification. `cargo check -p cargo-pmcp --tests` reported
  `could not compile ... due to 1 previous error` with an EMPTY error body.
- **Diagnosis:** `df -h /` showed **217Mi free, 100% capacity** — the recorded
  "disk exhaustion fakes code regressions" failure mode.
- **Fix:** Removed `target/debug/incremental` (48G of a 80G `target/`), a pure regenerable build
  artifact and gitignored. 89Gi free afterwards; the same command then passed cleanly. No source or
  tracked file was touched. **This was an environment fault, not a code defect** — worth stating
  because the raw output looked exactly like a real `TestResult`-literal break.

### Deliberate design choices worth flagging

**Reserved `_meta` keys are spelled as literals, guarded by a non-circular tripwire.** A raw v2
request cannot be built without them (the server's era gate requires the header and the `_meta` value
to AGREE and rejects a header-only claim `-32020`), but pmcp's constants are `pub(crate)` and their
only public re-export `pmcp::testing::*` sits behind the `testing` feature — which
`crates/mcp-tester/Cargo.toml` does not enable and MUST NOT start enabling (T-117-SC pins that
manifest). So `tester.rs` spells them, and
`the_sdk_emits_the_reserved_meta_key_this_crate_spells` captures the bytes a **real SDK-built v2
client** puts on a socket and asserts the literal appears in them. That compares the constant against
the SDK's behaviour, not against itself.

**Two `#[allow(dead_code)]`s on `main.rs`'s `mod era_diff` / `mod era_observations`.** A Rust binary
and its sibling library are separate crates, so these modules are compiled twice with independent
dead-code analysis; the binary reaches only the `--dual-run` path through them. Scoped to those two
declarations with the reasoning inline, never crate-wide. All other new API is reached by the binary,
so `cargo build -p mcp-tester` emits **zero** `mcp-tester` warnings.

## Verification

Run **directly**, because `make quality-gate` provably runs **zero** `mcp-tester` tests
(`grep -c 'dual_run' quality-gate.log` = **0**, `grep -c 'era_baseline'` = **0**). The recorded
LIM-116-10 gate-scope hole holds for this whole crate: a green gate is not evidence any test here
ran.

| Gate | Result |
|------|--------|
| `cargo test -p mcp-tester --test dual_run` | **12 passed, 0 failed**, 1.41s (plan asked ≥6 in <120s) |
| `cargo test -p mcp-tester --test report_compat` | **7 passed** — single-run output still byte-identical to 0.7.0 |
| `cargo test -p mcp-tester --test era_baseline` | **6 passed** |
| `cargo test -p mcp-tester` (whole crate) | **328 passed, 12 suites**, 4.15s |
| `cargo build -p mcp-tester` | exit 0, **zero** mcp-tester warnings |
| `cargo build -p cargo-pmcp` | exit 0 |
| `cargo check -p cargo-pmcp --tests` | exit 0 — the second `TestResult` literal in `check.rs:522` that `cargo build` structurally cannot reach |
| `make lint` | exit 0 |
| `make quality-gate` | exit 0, full success banner (9268-line log) |
| `git diff crates/mcp-tester/src/report.rs` | **empty** |
| `git diff --stat src/` | **empty** (includes `src/client/mod.rs` — no SDK probe added) |
| `git diff crates/mcp-tester/Cargo.toml` | **empty** — zero packages added |
| `git diff --stat crates/mcp-tester/src/conformance/` | only `mod.rs` + `core_domain.rs`; the other five domain files byte-identical |
| `grep -c 'TODO\|FIXME\|XXX' tests/dual_run.rs` | **0** |
| nextest `-E 'test(` selector | not used anywhere; plain `cargo test --test dual_run` throughout |

`run_dual`'s body calls `self.run(` exactly **2** times and `era_observations::observe(` exactly
**2** times.

### Carry-forward hazards that bit again

- **`make` through the rtk hook produced a corrupted, untrustworthy log.** The first
  `make quality-gate` run wrote a 774-line file ending in a literal `... (7421 lines truncated)`
  marker with **no success banner**, while still reporting exit 0. Re-run as
  `/usr/bin/make -C <repo> quality-gate` it produced 9268 lines, exit 0, and the real
  `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` banner. Always use the absolute binary path for the gate.
- **macOS `sed` BRE has no `\s`.** A `sed -i '' '/^\s*content_type: .../d'` silently deleted nothing
  and reported success. It went unnoticed because `cargo build --lib` does not compile
  `#[cfg(test)]` blocks — only `cargo test` caught it. Deletions were redone in Python and verified
  by count (`removed 4`).
- **`cargo fmt` reflowed function signatures** so two `python` replacements over pre-`fmt` text
  didn't match; the compiler caught them. Re-derived from the post-`fmt` file.

## Known Stubs

None. Every probe reaches a real socket; no placeholder or hardcoded observation exists.

## Threat Flags

None. No new network endpoint, auth path, file access or schema at a trust boundary was introduced —
all new traffic is outbound probes from a testing tool to a URL the operator already supplied.

## Hand-off

| Item | Owner |
|------|-------|
| Server still serves `initialize` on the v2 wire (mixed envelope) | 117-12 / 117-13 / Phase 118 |
| `http.status.error_code_mapping` — baseline wording vs probe rule needs adjudication | Phase 118 |
| Three MISSING rows are fixture limitations (no task store, no subscribable caps) — a richer fixture would raise EXPECTED from 9 | Phase 118 |
| Pre-existing `pmcp` unused-import warnings in `src/server/auth/jwt{,_validator}.rs` | logged to `deferred-items.md`, out of scope |

## Self-Check: PASSED

All 8 claimed files exist on disk; all 4 claimed commits resolve in `git log`; all 6 key symbols
(`with_protocol_version`, `detect_eras`, `run_dual`, `DualRunReport`, `compare_eras`, the
`dual_run: bool` CLI field) are present in the tree.
