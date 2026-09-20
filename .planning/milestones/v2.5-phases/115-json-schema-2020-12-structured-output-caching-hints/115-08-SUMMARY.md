---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 08
subsystem: testing
tags: [tripwire, cargo-metadata, jsonschema, sep-2106, ssrf, caching-hints, wasm, source-scanner]

# Dependency graph
requires:
  - phase: 115-03
    provides: the Draft 2020-12 pin, the era-keyed compile cache, and the workspace-wide `jsonschema` 0.49 bump this file asserts against
  - phase: 115-06
    provides: the projector wiring at the native chokepoint and the wasm dispatcher's strip call, plus the measurement that removing the latter leaves `make wasm-build` green
provides:
  - "tests/v2_schema_tripwires.rs — 13 tests fencing SEP-2106, D-12, the wasm strip call and the projection/middleware ordering"
  - "A dependency-graph tripwire idiom: cargo metadata parsed as JSON over the DECLARED and RESOLVED graphs, never Cargo.toml as text"
  - "Measured correction: the production `inject_v2_result_envelope` population is SIX call sites, not four"
affects: [115-09, 115-10, 115-11, future phases touching output validation, caching hints or the wasm dispatcher]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "cargo-metadata tripwire: assert on cargo's own resolved dependency graph rather than on grepped manifest text"
    - "Two-kind justified allowlist with MEASURED hit counts (114-16 / 113-21 instrument shape), restated not shared"

key-files:
  created:
    - tests/v2_schema_tripwires.rs
  modified: []

key-decisions:
  - "Parse `cargo metadata` JSON instead of scanning Cargo.toml text — a renamed `package = \"jsonschema\"` alias is caught by the `name` field and reported with `rename: Some(\"js\")`, which a text scan of the manifest key misses entirely"
  - "The ~400-line scanner duplication is DECLINED as a trim: a Rust integration test is its own crate, so the primitives are restated on purpose to keep one source-scanning shape in the repository"
  - "D-10's structural half (caching and tasks modules never import each other) is added HERE at the tripwire layer after 115-05 declined it at the types layer in favour of reciprocal rustdoc"
  - "The ordering test pins a KNOWN LIMITATION by measurement, not a desirable property; its failure is the intended signal that the deferred item was addressed"

patterns-established:
  - "Dependency-graph tripwires: `cargo metadata --no-deps` for DECLARED deps, `cargo metadata --features <f>` for the RESOLVED node's features — the second is the only layer that sees graph-wide feature unification from a dev-dependency or example"
  - "Write-position classification: a wire-key literal counts as a projection only when preceded by a mutator call (`insert`/`entry`/`remove`/`or_insert`) or in an index-assign position, so the thousands of read-side assertions in the tree do not false-positive"
  - "Source tripwires as the ONLY gate for cfg-excluded code: `wasm_server.rs` is compiled by exactly one command and executed by none, so a source assertion is load-bearing rather than belt-and-braces"

requirements-completed: [SCHM-01, SCHM-03]

# Metrics
duration: 60min
completed: 2026-08-01
---

# Phase 115 Plan 08: Schema and Projection Tripwires Summary

**`tests/v2_schema_tripwires.rs` fences SEP-2106 against cargo's own declared AND resolved dependency graphs — catching renamed, table-style and unification-induced resolver enablement a text scan cannot — plus D-12's single projection point, the wasm dispatcher's strip call that no native gate compiles, and the projection/middleware ordering, all proven non-vacuous by eight negative controls.**

## Performance

- **Duration:** ~60 min
- **Started:** 2026-08-01T11:50Z
- **Completed:** 2026-08-01T12:50Z
- **Tasks:** 2/2
- **Files modified:** 1 created, 0 production files changed

## Accomplishments

- **SEP-2106 is fenced against cargo, not against text.** Two layers: `cargo metadata --format-version 1 --no-deps` walks `.packages[].dependencies[]` for every entry whose `name == "jsonschema"` and asserts `uses_default_features == false` with no resolver feature; `cargo metadata --format-version 1 --features validation` joins `.resolve.nodes[].id` against `.packages[].id` and asserts the node's `features` array is EMPTY and that exactly ONE node exists. The second layer is the only one that can see a dev-dependency, an example or a sibling crate turning `resolve-http` on through graph-wide feature unification.
- **The renamed-declaration case is proven caught.** Negative control C declared `js = { package = "jsonschema", … }`; the test failed and its message printed `rename: Some("js")` while still naming the dependency `jsonschema`. A text scan of the manifest KEY would have found nothing.
- **The wasm strip call now has an automated gate.** Negative control F removed `project_caching_hints` from `src/server/wasm_server.rs`; `make wasm-build` exited **0** (green) while test 9 FAILED. That pair is the entire justification for the test existing, and it was observed together in one run.
- **The `inject_v2_result_envelope` population was re-measured.** The plan predicted four production call sites; the measured population is **six**. The two the original map missed (`streamable_http_server.rs`, `testing/mod.rs`) were already recorded by 115-06 and are now allowlisted with written justifications.
- **Zero production bytes changed.** `git diff --stat -- src/ Cargo.toml crates/` is empty and `git status --short` on those paths is empty after all eight controls were reverted.

## Task Commits

1. **Task 1: The SEP-2106 fence, built on cargo's own dependency graph** — `aa3c562c` (test)
2. **Task 2: The D-12 single-projection fence, the wasm call-site fence, and the ordering fence** — `3c46215f` (test)

## Files Created/Modified

- `tests/v2_schema_tripwires.rs` (created, 2075 lines) — 13 tests, all prefixed `v2_schema_tripwires_` so both `binary(v2_schema_tripwires)` and `test(/v2_schema_tripwires/)` select the suite.

### The 13 tests

| # | Test | What it fences |
|---|------|----------------|
| 1 | `no_manifest_declares_jsonschema_with_default_features` | DECLARED graph: `uses_default_features == false`, no resolver feature |
| 2 | `the_resolved_graph_enables_no_jsonschema_resolver_feature` | RESOLVED graph: empty feature array, exactly one node, version `0.49.x` |
| 3 | `the_manifest_scan_is_not_vacuous` | ≥3 declared deps incl. `pmcp`/`pmcp-agent`/`pmcp-server-toolkit`; ≥1 resolved node |
| 4 | `no_source_installs_a_ref_retriever` | `with_retriever` / `with_http_options` / `Retrieve` / `AsyncRetrieve` / `Retriever` across all workspace `src/` trees |
| 5 | `validator_construction_sites_are_accounted_for` | Justified allowlist over `validator_for(` / `draft202012::` + dialect-policy check |
| 6 | `the_source_scan_is_not_vacuous` | `src/` >50 files, workspace >300 files, both needles present in `output_validation.rs` |
| 7 | `caching_hints_are_written_in_exactly_one_place` | D-12: every `"ttlMs"` / `"cacheScope"` WRITE is `project_caching_hints` in `src/types/caching.rs` |
| 8 | `no_result_type_projects_independently` | Per-field serde attributes on all 12 hint fields; no hand-written `Serialize`/`Deserialize` |
| 9 | `every_cacheable_serialization_site_routes_through_the_projector` | The wasm strip call — the ONLY gate that catches its removal |
| 10 | `every_envelope_call_site_names_its_cacheability` | Six production `inject_v2_result_envelope` sites, each justified |
| 11 | `the_projection_precedes_response_middleware_by_measurement` | Byte-offset ordering inside `ServerCore::handle_request` |
| 12 | `the_projection_scan_is_not_vacuous` | ≥2 hint writes, ≥4 wasm `to_value` sites, all 6 envelope sites located, exactly 12 hint fields |
| 13 | `ttl_ms_definitions_stay_in_separate_modules` | D-10: `types::caching` and `types::tasks` never import each other |

## The eight negative controls, with their observed failure messages

Every control was applied, observed, and reverted. Controls A–D ran against the Task 1 file; E–H against the complete file. All were run by invoking the ALREADY-BUILT test binary directly (`./target/debug/deps/v2_schema_tripwires-c80419810f125fd9`), because every scan reads manifests and sources at RUNTIME — this avoids a full rebuild per control and, for control A/B, avoids compiling `reqwest` + `rustls` into the tree.

### Control A — `features = ["resolve-http"]` on the root `jsonschema` declaration

Tests 1 AND 2 both failed, as required.

```
  RESOLVER FEATURE ON: package `pmcp` (rename: None, kind: None, req: ^0.49, optional: true) enables `jsonschema/resolve-http`.
    Remove it from that declaration's `features` list.
```
```
  RESOLVED node `registry+https://github.com/rust-lang/crates.io-index#jsonschema@0.49.2` compiles with features ["reqwest", "resolve-http"], not [].
  This is the ONLY check that sees the effect of a DEV-dependency, an example, or a sibling workspace crate turning a feature on: unification is graph-wide, so the declared dependency check above would still pass while the retriever is compiled in.
```

### Control B — `default-features = false` removed from `crates/pmcp-agent/Cargo.toml`

Test 1 failed naming `pmcp-agent`; test 2 failed showing a non-empty resolved feature set. Note that the SIBLING crate's declaration polluted the shared node — that is unification in action.

```
  DEFAULT FEATURES ON: package `pmcp-agent` (rename: None, kind: None, req: ^0.49, optional: false) declares `jsonschema` WITHOUT `default-features = false`.
```
```
  RESOLVED node `registry+https://github.com/rust-lang/crates.io-index#jsonschema@0.49.2` compiles with features ["default", "reqwest", "resolve-file", "resolve-http", "tls-aws-lc-rs"], not [].
```

### Control C — the root declaration renamed to `js = { package = "jsonschema", … }`

**The case a text scan misses.** Test 1 failed and its message showed `rename: Some("js")`:

```
  DEFAULT FEATURES ON: package `pmcp` (rename: Some("js"), kind: None, req: ^0.49, optional: true) declares `jsonschema` WITHOUT `default-features = false`.
```
```
  RESOLVED node `registry+https://github.com/rust-lang/crates.io-index#jsonschema@0.49.2` compiles with features ["default", "reqwest", "resolve-file", "resolve-http", "tls-aws-lc-rs"], not [].
```

Test 3 (anti-vacuity) correctly still PASSED under this control, because the rename does not change the dependency's `name` or its owning package.

### Control D — a new `src/control_d_probe.rs` calling `jsonschema::validator_for(`

```
  UNKNOWN validator construction site: `control_d_probe` in src/control_d_probe.rs at line(s) [2].
```

The file was never declared as a module and never compiled — the scan is a runtime `read_dir` walk, which is exactly the property that makes a new file unable to escape it.

### Control E — `obj.insert("ttlMs".to_string(), json!(1));` appended to `src/server/mod.rs`

```
  OUT-OF-PLACE hint write: `"ttlMs"` written by `control_e_probe` at src/server/mod.rs:6901
```

### Control F — the `project_caching_hints` call removed from `src/server/wasm_server.rs`

**The pair, observed together — this is the whole justification for test 9's existence:**

```
MAKE_WASM_BUILD_EXIT=0
```
```
test v2_schema_tripwires_every_cacheable_serialization_site_routes_through_the_projector ... FAILED
  src/server/wasm_server.rs no longer references `project_caching_hints` at all.
```

`make wasm-build` is the ONLY command in the repository that compiles this file, and it stayed green with the strip deleted. Nothing else compiles it (`src/server/mod.rs` gates it on `target_arch = "wasm32"`) and nothing at all executes it (its own `cfg(all(test, target_arch = "wasm32"))` module does not compile). Test 9 is therefore the only automated gate that can catch the removal.

A second, unplanned observation from this control: after the call was deleted, `grep -c project_caching_hints src/server/wasm_server.rs` still returned **1** — the surviving hit is the rustdoc reference on line 35. The test still failed correctly because it scans COMMENT-STRIPPED text. A naive `grep`-based version of this tripwire would have passed.

### Control G — a seventh production `inject_v2_result_envelope(` call site

```
  UNLISTED envelope call site: `control_g_probe` in src/server/mod.rs at line(s) [6901].
```

### Control H — the injection moved after `process_response_with_context` in `ServerCore::handle_request`

```
the caching projection at src/server/core.rs:3416 now runs AFTER `process_response_with_context` at src/server/core.rs:3409.
  This test pins a KNOWN LIMITATION by measurement: middleware takes `&mut JSONRPCResponse` and can therefore forge or strip `ttlMs` / `cacheScope` / `resultType` / `serverInfo` after the projection. The reorder was CONSIDERED and declined because it changes what middleware observes about Phase 114's `resultType` / `serverInfo`, and it is booked as a DEFERRED ITEM by 115-10.
```

The message names the deferred item, as the acceptance criterion required.

## Measured facts recorded by this plan

### `cargo metadata` — the DECLARED graph (2026-08-01, post-115-03)

Exactly three packages declare `jsonschema`, all clean:

| Package | rename | optional | uses_default_features | features | req |
|---------|--------|----------|----------------------|----------|-----|
| `pmcp` | `None` | true | **false** | `[]` | `^0.49` |
| `pmcp-agent` | `None` | false | **false** | `[]` | `^0.49` |
| `pmcp-server-toolkit` | `None` | true | **false** | `[]` | `^0.49` |

### `cargo metadata --features validation` — the RESOLVED graph

- **Nodes matching `jsonschema`: 1** (two would mean two copies compiled in)
- **Version: `0.49.2`**
- **Features: `[]`** — empty, i.e. no resolver and no TLS backend is compiled in
- Node id: `registry+https://github.com/rust-lang/crates.io-index#jsonschema@0.49.2`

### Final allowlists, with measured hit counts

**`VALIDATOR_SITES`** (3 entries):

| File | Function | Hits | Disposition |
|------|----------|------|-------------|
| `src/server/output_validation.rs` | `compile_2020_12` | 1 | `PinnedByPolicy` |
| `src/server/output_validation.rs` | `compile_for_era` | 1 | `EraFrozenV1` |
| `crates/pmcp-agent/src/iteration/decide.rs` | `evaluate_submit_result` | 1 | `OutOfScopeAllowlisted` |

The `pmcp-agent` justification states that it validates agent submit-results rather than the MCP `outputSchema` seam, that SCHM-01 scopes to the server output-validation path, and that pinning the draft there would be a behaviour change to a different surface — booked as a deferred item rather than changed inside a schema-pinning phase.

A second assertion beyond the counts: the `PinnedByPolicy` function must still contain `draft202012` and the `EraFrozenV1` function must still contain `validator_for`. A straight swap of the two constructors leaves the count identical and would otherwise pass.

**`WASM_SERIALIZATION_SITES`** (4 entries, 5 measured `serde_json::to_value(` hits):

| Function | Hits | Disposition |
|----------|------|-------------|
| `cacheable_result_to_value` | 1 | `RoutesThroughProjector` |
| `handle_initialize` | 1 | `NotCacheable` (`InitializeResult`) |
| `handle_call_tool` | 2 | `NotCacheable` (`CallToolResult`, success + rejection arms) |
| `handle_get_prompt` | 1 | `NotCacheable` (`GetPromptResult`) |

Plus a named list of the four handlers that MUST route through the helper (`handle_list_tools`, `handle_list_resources`, `handle_read_resource`, `handle_list_prompts`), each asserted to call `cacheable_result_to_value` and to contain no direct `serde_json::to_value(`; plus a catch-all over every function that constructs one of the six cacheable types.

**`ENVELOPE_SITES`** (6 entries — see deviation 1):

| File | Function | Hits |
|------|----------|------|
| `src/server/core.rs` | `build_discover_response` | 1 |
| `src/server/core.rs` | `handle_request` | 1 |
| `src/server/mod.rs` | `handle_tasks_update` | 1 |
| `src/server/mod.rs` | `handle_request_with_context` | 1 |
| `src/server/streamable_http_server.rs` | `listen_terminal_result_frame` | 1 |
| `src/testing/mod.rs` | `run_envelope` | 1 |

### Caching-hint write sites (test 7)

Exactly **4** write positions in the whole `src/` tree, all in `project_caching_hints` in `src/types/caching.rs`: `entry("ttlMs")`, `entry("cacheScope")`, `remove("ttlMs")`, `remove("cacheScope")`.

### Caching-hint field declarations (test 8/12)

Exactly **12**: six `CacheableResult` extenders × two fields, all carrying `#[serde(skip_serializing_if = "Option::is_none")]` and nothing else. All 74 `skip_serializing_if` values in the four result modules are `"Option::is_none"`. The one `serialize_with` in `src/types/resources.rs` is on `ReadResourceResult::contents`, not on a hint field — which is exactly why the check is scoped per-field rather than per-module.

## Decisions Made

1. **The ~400-line scanner duplication is DECLINED as a trim.** The cross-AI review flagged it as surface cost. The repository's stated doctrine is that a Rust integration test is its own crate — `tests/v2_schema_tripwires.rs` cannot import `tests/v2_tasks_tripwires.rs`'s scanner and vice versa — so the primitives are duplicated ON PURPOSE and the idiom is kept identical so the repository has ONE source-scanning shape rather than three divergent ones. The declination is recorded in the module doc.
2. **D-10's structural half was added here after being declined at the types layer.** 115-CONTEXT.md leaves the tripwire to the planner's discretion under D-10; 115-05 exercised that discretion by declining it at the types layer in favour of reciprocal rustdoc. This plan exercised it the other way at the tripwire layer, asserting only the cheap structural property (neither module imports the other) rather than anything about the values.
3. **The ordering test asserts a KNOWN LIMITATION, not a desirable property.** Its rustdoc names all three artifacts that already state the limitation (the rustdoc prohibition on `inject_v2_result_envelope`, 115-06's behavioural limitation test, and 115-10's deferred item) and instructs a future reorderer to update all three together.
4. **The write-position classifier excludes reads.** `get(` and `contains_key(` are deliberately absent from `WRITE_CALLS`: a test asserting a key's ABSENCE is not a projection, and folding reads in would fire on the read-side assertions in `src/server/core.rs`, `src/server/task_dispatch.rs` and `src/types/caching.rs`'s own test module.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Stale measurement] The production `inject_v2_result_envelope` population is SIX, not four**

- **Found during:** Task 2 (test 10)
- **Issue:** The plan's action text said *"assert the count of `inject_v2_result_envelope(` call sites in `src/` PRODUCTION code … is exactly 4, and that each is in one of the four known functions."* The measured population is six. The two extra sites are `src/server/streamable_http_server.rs:3106` (`listen_terminal_result_frame`) and `src/testing/mod.rs:505` (`run_envelope`) — both already recorded by 115-06 as call sites the original map missed. `src/testing/mod.rs` is gated `#[cfg(any(test, feature = "testing"))]`, which is production (feature-gated) code, not `cfg(test)`, so it is correctly in scope.
- **Fix:** Encoded the MEASURED population as a six-entry justified allowlist keyed by (file, function) with per-entry hit counts, rather than a bare `assert_eq!(count, 4)`. This is strictly stronger than the plan's shape — it fails on an unlisted site, a changed count inside a listed function, and a stale entry — and it satisfies the plan's stated intent ("each is in one of the known functions"). Negative control G confirms it fires on a new site.
- **Files modified:** `tests/v2_schema_tripwires.rs` only
- **Verification:** Test 10 green at 6 sites; control G observed failing
- **Committed in:** `3c46215f`

**2. [Rule 3 - Blocking] Two primitives were introduced in Task 2 rather than Task 1**

- **Found during:** Task 1
- **Issue:** The plan asked for the `<interfaces>` primitives to be restated verbatim in Task 1. `make lint` runs `RUSTFLAGS="-D warnings" cargo clippy --features full --lib --tests`, so `dead_code` is a hard ERROR — restating `strip_keeping_literals` and `fn_body` in Task 1, where nothing yet used them, would have left Task 1's commit failing the gate.
- **Fix:** Restated each primitive in the task that first uses it: Task 1 carries the identifier-scan primitives, Task 2 adds `strip_keeping_literals` (needed for the wire-string and serde-attribute scans), `fn_body`, `matching_open_bracket`, `attrs_before` and `attr_value`. `needle_present` and `block_after` from the reference file were omitted entirely because nothing here uses them. The shape and doc idiom are unchanged.
- **Files modified:** `tests/v2_schema_tripwires.rs` only
- **Verification:** `make lint` exit 0 at both commits
- **Committed in:** `aa3c562c`, `3c46215f`

**3. [Rule 3 - Blocking] Three clippy fixes to satisfy the gate**

- **Found during:** Tasks 1 and 2
- **Issue:** `RUSTFLAGS="-D warnings"` promotes pedantic/nursery warnings to errors. Three fired: `clippy::map_unwrap_or` (`.map(…).unwrap_or_else(panic!)` in the dialect-policy check), `clippy::needless_collect` (collecting projector hits only to test emptiness), and `clippy::never_loop` ×2 (the D-10 `for … { panic! }` shape).
- **Fix:** Replaced with a `let … else { panic! }`, an `.any(…)`, and `if let Some(at) = … .first().copied()` respectively. No assertion semantics changed.
- **Files modified:** `tests/v2_schema_tripwires.rs` only
- **Verification:** `make lint` exit 0; all 13 tests still green
- **Committed in:** `aa3c562c`, `3c46215f`

---

**Total deviations:** 3 auto-fixed (1× Rule 1, 2× Rule 3)
**Impact on plan:** No scope creep. Deviation 1 makes the fence STRONGER than planned and corrects a stale count the plan inherited; deviations 2 and 3 are gate-compliance mechanics with no effect on what is asserted.

## Issues Encountered

1. **Negative controls A–C would have forced a `reqwest` + `rustls` compile.** Enabling `resolve-http` and then running `cargo nextest` would rebuild the whole tree with a TLS stack. Resolved by running the ALREADY-BUILT test binary directly — every scan in this file reads manifests and sources at RUNTIME, so no rebuild is needed for a control to be observed. `Cargo.lock` (gitignored here) was backed up and restored alongside each manifest.
2. **`make lint` gives no usable transcript.** Consistent with the trap recorded by prior plans: a genuinely failing `make lint` printed nothing actionable. Resolved by trusting the exit code and re-running the same clippy invocation directly against `--test v2_schema_tripwires` with `$HOME/.cargo/bin/cargo` to get the real diagnostics.
3. **A `grep`-based version of test 9 would have passed control F.** After the strip call was deleted, `grep -c project_caching_hints src/server/wasm_server.rs` still returned 1 (the surviving rustdoc reference on line 35). The test failed correctly only because it scans comment-stripped text. Recorded because it is a concrete demonstration that the comment-awareness of this scanner idiom is load-bearing, not decoration.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Ready for 115-09 and 115-10.** The tripwire suite is green, zero production bytes changed, and the scoped gate (`cargo fmt --all -- --check`, `make lint`, `make check-todos`) is clean. `make quality-gate` runs once for the phase in 115-10, per the phase commit policy.
- **Two items for 115-10's deferred ledger** (both stated inside the test file's own failure messages, so they cannot be lost):
  1. `crates/pmcp-agent/src/iteration/decide.rs`'s `validator_for` site is allowlisted, NOT fixed — the agent submit-result validation path is still dialect-auto-detecting.
  2. The projection/middleware ordering limitation: a registered response middleware can still forge or strip `ttlMs` / `cacheScope` / `resultType` / `serverInfo` after the projection.
- **No blockers.**

---
*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Completed: 2026-08-01*

## Self-Check: PASSED

- `tests/v2_schema_tripwires.rs` — FOUND
- `.planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-08-SUMMARY.md` — FOUND
- Commit `aa3c562c` — FOUND
- Commit `3c46215f` — FOUND
- Commit `dfbc9b04` — FOUND
