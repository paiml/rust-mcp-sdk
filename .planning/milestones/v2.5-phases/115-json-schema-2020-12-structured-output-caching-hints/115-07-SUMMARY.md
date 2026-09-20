---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 07
subsystem: testing
tags: [mcp-2026-07-28, caching-hints, cacheable-result, streamable-http, servercore, era-gating, golden-fixtures]

# Dependency graph
requires:
  - phase: 115-06
    provides: "project_caching_hints wired at the native chokepoint on both eras plus the shared request_is_cacheable classifier"
  - phase: 115-05
    provides: "src/types/caching.rs (CacheScope, DEFAULT_TTL_MS, the total projector) and the with_ttl_ms / with_cache_scope builders on the six CacheableResult extenders"
  - phase: 115-04
    provides: "the era-aware in-process seam in tests/common/duplex.rs (call_tool_request, raw_via_core, assert_v2_witness)"
  - phase: 115-02
    provides: "tests/v1_lists_golden.rs — the pre-change v1 byte-identity capture and its (then vacuous) v1_leak_guard"
provides:
  - "tests/v2_caching_hints.rs — 19 tests proving SCHM-03 on the wire across six methods, two eras and both native dispatchers"
  - "read_resource_request(uri, era) in tests/common/duplex.rs — the only cacheable method reachable at Era::V2 through the typed in-process route"
  - "a sixth v1_lists_golden fixture that makes its ttlMs/cacheScope leak guard load-bearing against a handler that genuinely opted in"
  - "a named, asserted record of the structural bound on in-process v2 for the four list methods"
  - "MEASURED: over HTTP a non-opted-in server refuses a v2 request with 400/-32600 rather than serving it silently as v1"
affects: [115-08, 115-09, 115-10, 116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "in-band era witness before every v2 payload assertion (resultType presence/absence)"
    - "leak guards factored as Option/Result-returning predicates so their own anti-vacuity test can drive them directly"
    - "greppable negative invariants: the forbidden identifier is deliberately never spelled in the file that forbids it"

key-files:
  created:
    - tests/v2_caching_hints.rs
  modified:
    - tests/common/duplex.rs
    - tests/v1_lists_golden.rs

key-decisions:
  - "D-115-07-A: the era-witness anti-vacuity contrast over HTTP is request-shaped (v2 body vs v1 body, one opted-in server), not server-shaped, because a non-opted-in server refuses a v2 request at the transport with 400/-32600 instead of serving it as v1 — MEASURED, and asserted in its own test so a future transport change that started serving such a request cannot silently weaken every HTTP era test in the phase"
  - "D-115-07-B: the resultType wire spelling stays in ONE constant and every era-sensitive test calls a shared witness helper, rather than inlining the literal eight times; the plan's `grep -c resultType >= 8` criterion is replaced by the measured, stronger `grep -c 'assert_v2_era_witness|assert_no_v2_era_witness' == 10`"
  - "D-115-07-C: the wire integers 300000 / 60000 are pinned as STRING literals asserting on the raw response text, because Rust source cannot write the un-separated form under clippy::unreadable_literal — which also makes them a stronger check (the value is proven to reach the wire as a JSON integer, not 3e5 or a string)"
  - "D-115-07-D: the sixth v1_lists_golden fixture pins NO new golden literal; it drives a different (hint-setting) server, so its bytes were never captured pre-change, and what it asserts is the leak guard, which is the property D-11 actually needs"

patterns-established:
  - "Twin-dispatcher coverage is scoped to what is structurally reachable, and the unreachable half is asserted and explained at a named test rather than silently omitted"
  - "Every absence assertion in a suite gets one anti-vacuity test that drives the SAME predicate over synthetic positives and a synthetic clean case"

requirements-completed: [SCHM-03]

# Metrics
duration: 96min
completed: 2026-08-01
---

# Phase 115 Plan 07: On-the-wire SCHM-03 proof Summary

**19 live-HTTP + in-process tests proving all six `2026-07-28` `CacheableResult` methods emit `ttlMs`/`cacheScope` with the safe defaults `0`/`"private"`, that handler-set values reach the v2 wire verbatim, and that both keys are actively stripped on v1 — each v2 claim gated behind an in-band `resultType` era witness, across both native dispatchers, with zero production bytes changed.**

## Performance

- **Duration:** ~96 min
- **Started:** 2026-08-01T11:20Z
- **Completed:** 2026-08-01T12:56Z
- **Tasks:** 2
- **Files modified:** 3 (1 created, 2 modified) — all under `tests/`

## Accomplishments

- **All six cacheable methods proven on the wire.** `tools/list`, `prompts/list`, `resources/list`, `resources/templates/list`, `resources/read` and `server/discover` each carry `ttlMs: 0` and `cacheScope: "private"` on a real loopback HTTP round trip, asserted on the parsed result AND on the raw camelCase key spellings.
- **The sixth extender is named, not folded in.** `server/discover` gets `v2_caching_hints_discover_is_the_sixth_cacheable_result`, whose rustdoc states why the count disagrees with the requirement text's "five" — and it reaches the projection by a different route (`build_discover_response`'s own `Cacheable::Yes` at `src/server/core.rs:1935`) from the other five (`request_is_cacheable`), so the test also proves the two routes agree.
- **Handler-set values proven to survive on v2 and to be stripped on v1**, over HTTP and in-process, with two deliberately different pairs (300000/`public` on `resources/list`, 60000/`private` on `resources/read`) so a cross-contamination bug cannot pass by coincidence.
- **Both native dispatchers covered.** The HTTP half exercises the high-level `Server` (injection at `src/server/mod.rs:1723`); a `server_core` module exercises `ServerCore` (injection at `src/server/core.rs:3404`) for everything structurally reachable.
- **The structural bound is measured, asserted and explained at the code** rather than only in a plan, with an anti-vacuity twin proving the dropped signal was a real one.
- **`v1_lists_golden`'s caching-hint leak guard is no longer vacuous.** A sixth fixture drives a handler that genuinely called `with_ttl_ms` / `with_cache_scope` over v1; the only thing between those values and the wire is the era-gated projection.
- **Zero production bytes changed.** `git diff --stat -- src/ Cargo.toml` is empty at HEAD.

## Task Commits

1. **Task 1: Prove all six v2 methods carry the hints on the wire, with the safe defaults** — `8aced935` (test)
2. **Task 2: Prove handler-set values survive on v2 and are stripped on v1, across both native dispatchers** — `078a3661` (test)

## Files Created/Modified

- `tests/v2_caching_hints.rs` (created, 1183 lines) — 19 tests: six per-method v2 default tests, the non-cacheable `tools/call` control, a six-method v1 contrast loop, the handler-set preservation and v1-strip tests, two anti-vacuity tests, and a five-test `server_core` module.
- `tests/common/duplex.rs` (+60 lines) — `pub fn read_resource_request(uri: &str, era: Era) -> Request`, the `resources/read` sibling of 115-04's `call_tool_request`, with a rustdoc recording the measured three-variant bound and an explicit "do not add a list-method sibling" warning that deliberately never spells the forbidden identifiers.
- `tests/v1_lists_golden.rs` (+95 lines) — `HintedResources`, `hinted_v1_server()` and `v1_lists_golden_handler_set_hints_never_reach_the_v1_wire`. **No pre-existing golden literal was removed or edited** (verified: `git diff tests/v1_lists_golden.rs | grep '^-' | grep -c 'r#"'` → `0`).

## Verification

| Command | Result |
|---|---|
| `cargo nextest run --features full -E 'binary(v2_caching_hints)'` | **19 tests run: 19 passed** (plan required ≥15) |
| `cargo nextest run --features full -E 'binary(v1_lists_golden)'` | **7 tests run: 7 passed** (plan required ≥7) |
| `cargo nextest run --features full -E 'binary(structured_tool_output)'` | still green (46 total across the three suites) |
| duplex-consumer regression (6 other `#[path]` includers + `common_harness_smoke`) | 41 tests run: 41 passed |
| `make lint` | exit 0 |
| `make fmt-check` | exit 0 |
| `make check-todos` | exit 0 |
| `git diff --stat -- src/ Cargo.toml` | EMPTY |
| `grep -c 'server_core_' tests/v2_caching_hints.rs` | 7 (required ≥4) |
| `grep -c 'list_resources_request\|list_tools_request\|list_prompts_request' tests/common/duplex.rs` | 0 (required 0) |

## Negative Controls (observed failures, transcribed)

### Task 1 — Control A: `request_is_cacheable` returns `Cacheable::No` for `ListTools`

`v2_caching_hints_tools_list_carries_the_defaults` FAILED as intended (and so did the witness test, which also drives `tools/list`):

```
thread 'v2_caching_hints_tools_list_carries_the_defaults' panicked at tests/v2_caching_hints.rs:170:5:
assertion `left == right` failed: v2 tools/list: D-07 makes `ttlMs` REQUIRED on every v2
`CacheableResult`, and D-08 fixes the SDK default at 0 (immediately stale, which asserts
nothing about cacheability). Expected 0. Raw response was:
{"jsonrpc":"2.0","id":1,"result":{"tools":[...],"resultType":"complete","_meta":{...}}}
  left: None
 right: Some(Number(0))
```

Note the raw body: `resultType` IS present, so the request genuinely resolved v2 and only the classifier changed. Reverted; `10 tests run: 10 passed`.

### Task 1 — Control B: remove `.with_supported_protocol_versions(...)` from the fixture

All six method tests failed, plus the non-cacheable control and the witness test — **8 of 10 failed, every one of them at `tests/v2_caching_hints.rs:106`**, i.e. inside `assert_v2_era_witness` (via `result_of`'s status check), never at a `ttlMs` assertion. The era check fires strictly before the payload check, as required:

```
thread 'v2_caching_hints_tools_list_carries_the_defaults' panicked at tests/v2_caching_hints.rs:106:5:
assertion `left == right` failed: v2 tools/list: expected HTTP 200, raw response was:
{"jsonrpc":"2.0","error":{"code":-32600,"message":"Unsupported protocol version: 2026-07-28"},"id":null}
  left: 400
 right: 200
```

**Measured refinement:** the plan expected these to fail on the `resultType` assertion. Over HTTP they fail one line earlier still — the transport's version gate answers 400 before dispatch. See Deviation 1.

### Task 2 — Control A: disable the strip arm of `project_caching_hints`

All three intended tests failed, and only those three (`26 tests run: 23 passed, 3 failed`):

- `v2_caching_hints_v1_strips_handler_set_values`
- `server_core::v2_caching_hints_server_core_resources_read_v1_strips_handler_set_values`
- `v1_lists_golden_handler_set_hints_never_reach_the_v1_wire`

```
thread 'v2_caching_hints_v1_strips_handler_set_values' panicked at tests/v2_caching_hints.rs:254:5:
v1 resources/list against a handler that SET both hints: the response carries the SCHM-03
caching hint `ttlMs` where it must carry neither. D-11 era-gates the hints OFF on v1, and a
v1 response carrying a v2 field breaks this milestone's severability story: Phases 116-119
all rest on v1 responses staying byte-identical. Fix the projection — never relax this
assertion. Wire was: {"jsonrpc":"2.0","id":33,"result":{"resources":[...],"ttlMs":300000,"cacheScope":"public"}}
```

```
thread 'v1_lists_golden_handler_set_hints_never_reach_the_v1_wire' panicked at tests/v1_lists_golden.rs:753:13:
the handler SET both hints and this is a v1 wire, so the era-gated projection must have
stripped them: v1 raw carries the SCHM-03 caching hint `ttlMs`. D-11 era-gates the caching
hints OFF on v1 ... Raw response was:
{"jsonrpc":"2.0","id":6,"result":{"resources":[...],"ttlMs":300000,"cacheScope":"public"}}
```

```
thread 'server_core::v2_caching_hints_server_core_resources_read_v1_strips_handler_set_values'
panicked at tests/v2_caching_hints.rs:254:5:
ServerCore / v1 resources/read, handler-set: the response carries the SCHM-03 caching hint
`ttlMs` where it must carry neither. ... Wire was:
{"jsonrpc":"2.0","id":1,"result":{"contents":[...],"ttlMs":60000,"cacheScope":"private"}}
```

Reverted; `src/` diff empty.

### Task 2 — Control B: remove the `_meta` block from `read_resource_request`'s `Era::V2` arm

The two `server_core` tests that use that builder for a v2 request both failed:

```
thread 'server_core::v2_caching_hints_server_core_resources_read_v2_carries_the_defaults'
panicked at tests/common/duplex.rs:405:13:
expected a Result payload, got error: JSONRPCError { code: -32002,
message: "Server not initialized. Call initialize first.", data: None }
```

(identically for `..._v2_preserves_handler_set_values`.)

The failure lands inside `assert_v2_witness` → `result_object`, one level earlier than the plan anticipated: a request that resolves non-v2 is caught by the v1 initialize gate (`v1_initialize_gate_applies`, `src/server/core.rs:4089`) and answered `-32002` before a result exists to inspect. The other two v2 `server_core` tests correctly stayed GREEN — they use `call_tool_request` and the inline `signalling_v2` builder respectively, so the control's blast radius is exactly the builder under test. See Deviation 2.

## Decisions Made

- **D-115-07-A — the HTTP anti-vacuity contrast is request-shaped, not server-shaped.** MEASURED: pointing a v2-signalling request at a non-opted-in server over HTTP yields `400` / `-32600 "Unsupported protocol version"`, not a silently-v1 200. `tests/structured_tool_output.rs`'s in-process twin gets the opposite (a silent v1 200), which is what makes the era witness load-bearing on that route. So the witness test here holds the SERVER constant and varies the REQUEST, and the refusal itself is asserted in `v2_caching_hints_a_non_opted_in_server_refuses_a_v2_request_over_http` so the stronger transport-level guarantee is recorded rather than assumed.
- **D-115-07-B — one wire-spelling constant, shared witness helpers.** See Deviation 3.
- **D-115-07-C — the wire integers are pinned as raw-text string literals.** `assert!(list.raw.contains(r#""ttlMs":300000"#))`. Rust source cannot write `300000` (clippy `unreadable_literal`), and the string form additionally proves the value serializes as a JSON integer rather than `3e5`, `300000.0` or `"300000"`.
- **D-115-07-D — the new `v1_lists_golden` fixture pins no golden literal.** It uses a different (hint-setting) server, so there are no pre-change bytes to pin; what it asserts is `v1_leak_guard`, which is the property D-11 needs.
- **Leak guards are predicates, not inline asserts.** `leaked_hint_key(wire) -> Option<&'static str>` is shared by the HTTP and `ServerCore` halves, so the two dispatchers are held to one definition of "carries no hint", and `v2_caching_hints_the_no_hints_guard_is_load_bearing` drives it over synthetic positives and a clean case — the idiom `tests/v1_lists_golden.rs:309` established.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's era-witness anti-vacuity test could not run as specified over HTTP**

- **Found during:** Task 1
- **Issue:** The plan's `<action>` implied the in-process pattern — send the same v2 request to a non-opted-in server and observe a v1-served response. Over HTTP the transport refuses with `400` / `-32600` before dispatch, so `assert_no_v2_era_witness` panicked on a missing `result`.
- **Fix:** Split into two tests. `v2_caching_hints_the_v2_era_witness_is_load_bearing` now varies the request era against one opted-in server, and `v2_caching_hints_a_non_opted_in_server_refuses_a_v2_request_over_http` asserts the measured refusal (status, `-32600`, the "Unsupported protocol version" message, absence of a result and of hints) so the stronger guarantee is fenced.
- **Files modified:** `tests/v2_caching_hints.rs`
- **Verification:** both tests green; the refusal message transcribed above
- **Committed in:** `8aced935`

**2. [Rule 1 - Doc/citation drift] The plan's line citations for `extract_request_meta_value` were stale**

- **Found during:** Task 2
- **Issue:** The plan cites `src/server/core.rs:3796-3831` / `:3796-3862` for `extract_request_meta_value` and its go-forward rustdoc. That region is now the `ClientRequest` dispatch `match`; the function is at `:3997` with its rustdoc at `:3960-3996` and `resolve_ingress_protocol_context` at `:4038`.
- **Fix:** every citation written into `duplex.rs` and `v2_caching_hints.rs` uses the verified ranges (`3960-4026`, `3971-3991`, `3997-4026`, `4038`, `4089`).
- **Files modified:** `tests/common/duplex.rs`, `tests/v2_caching_hints.rs`
- **Verification:** each range re-read before it was written
- **Committed in:** `078a3661`

**3. [Rule 2 - Missing critical / criterion substitution] `grep -c 'resultType' >= 8` replaced by a stronger, measured check**

- **Found during:** Task 1
- **Issue:** The criterion's stated purpose is "every v2 test proves its era in-band". Satisfying it literally requires inlining the `resultType` wire spelling at eight call sites — the exact copy-drift this repo has already been bitten by (`tests/common/v2.rs:673-681`, a hand-copied header encoder that silently diverged from the shipped one), and contrary to `tests/common/duplex.rs`, which the plan itself points at as the model and which keeps the spelling in one `RESULT_TYPE_KEY` constant.
- **Fix:** the spelling stays in one constant; every era-sensitive test calls `assert_v2_era_witness` / `assert_no_v2_era_witness`. Measured substitute: `grep -c 'assert_v2_era_witness(&\|assert_no_v2_era_witness(&'` → **10**, one per era-sensitive test, which is the property the criterion proxies for. A module-doc paragraph tells a future reviewer to grep the helpers rather than the literal. Literal `resultType` count is 5.
- **Files modified:** `tests/v2_caching_hints.rs`
- **Verification:** Task 1 Control B — all six method tests plus two more failed on the witness path before any payload assertion
- **Committed in:** `8aced935`

**4. [Rule 2 - Missing critical] The plan's `300000` / `60000` literal criterion is unsatisfiable in Rust source; pinned on the wire instead**

- **Found during:** Task 2
- **Issue:** `make lint` runs pedantic clippy with `RUSTFLAGS=-D warnings` and does not allow-list `unreadable_literal`, so `300000` in source is a hard error. (115-06 hit the same wall.)
- **Fix:** the constants are `300_000` / `60_000`; the un-separated wire forms are pinned as raw-text assertions on the response — `r#""ttlMs":300000"#` and `r#""ttlMs":60000"#` — which additionally proves each reaches the wire as a JSON integer.
- **Files modified:** `tests/v2_caching_hints.rs`
- **Verification:** `grep -c '300000'` → 4, `grep -c '60000'` → 3; `make lint` exit 0
- **Committed in:** `078a3661`

**5. [Rule 3 - Blocking] Two greppable-invariant detectors were broken by the prose that explains them**

- **Found during:** Task 2
- **Issue:** The rustdoc warning "do not add a `list_resources_request` sibling" itself made `grep -c 'list_resources_request\|list_tools_request\|list_prompts_request' tests/common/duplex.rs` return 1, failing its own acceptance criterion (same for one comment in `tests/v2_caching_hints.rs`).
- **Fix:** both passages reworded to forbid the builders without ever spelling their names, citing `tests/v1_lists_golden.rs:432-439`, which uses exactly this device to keep its not-opted-in invariant greppable.
- **Files modified:** `tests/common/duplex.rs`, `tests/v2_caching_hints.rs`
- **Verification:** both greps → 0
- **Committed in:** `078a3661`

**6. [Rule 1 - Bug] The plan's fixture spec for `resources/templates/list` was not implementable**

- **Found during:** Task 1
- **Issue:** The plan's `<action>` asks the fixture `ResourceHandler` to return "a fixed one-entry `ListResourceTemplatesResult`", and its `<method_matrix>` marks `resources/templates/list` handler-settable ("YES via ResourceHandler"). Neither is true: `ResourceHandler` declares only `read` and `list` (`src/server/mod.rs:368-382`), and both dispatchers return `resource_templates: vec![]` unconditionally (`src/server/mod.rs:2498`, `src/server/core.rs:1015`) — the same fact 115-02 already measured.
- **Fix:** the method still gets its own default-hints test (the dispatcher-built result is exactly what the projection must reach); the handler-set half of Task 2 uses `resources/list` and `resources/read`, which the plan already specified. The unreachability is documented on `HintFreeResources`.
- **Files modified:** `tests/v2_caching_hints.rs`
- **Verification:** `v2_caching_hints_resources_templates_list_carries_the_defaults` green
- **Committed in:** `8aced935`

**7. [Rule 2 - Missing critical] Added an anti-vacuity twin for the structural-bound test**

- **Found during:** Task 2
- **Issue:** As specified, `v2_caching_hints_list_methods_cannot_reach_v2_through_the_typed_dispatch_route` would pass identically if the `_meta` literal it sends were mis-spelled or mis-nested — proving a typo rather than the extractor's behaviour.
- **Fix:** `signalling_v2(method, params)` builds the literal once; `v2_caching_hints_server_core_the_dropped_signal_is_a_real_one` sends the IDENTICAL literal on `resources/read` (a `_meta`-bearing variant) to the SAME opted-in core and asserts it DOES resolve v2 with the default hints. Signal, server and route held constant; only the variant differs. The bound test also now asserts the pre-handshake `-32002` refusal, which a v2-resolved request would not receive.
- **Files modified:** `tests/v2_caching_hints.rs`
- **Verification:** both tests green; Control B leaves them green, confirming builder isolation
- **Committed in:** `078a3661`

---

**Total deviations:** 7 auto-fixed (3 bugs in plan specification, 3 missing-critical additions, 1 blocking). No production code changed by any of them.
**Impact on plan:** every deviation strengthened or corrected a check. Two acceptance criteria (`grep -c resultType >= 8`, the bare `300000` literal) were replaced by measured, stronger substitutes because satisfying them literally would have degraded the code or failed `make lint`; both substitutions are recorded above with their measurements. No scope creep.

## Issues Encountered

**1. A reverted `sed -i.bak` backup silently defeats cargo's rebuild detection.** Reverting Task 1's negative control with `mv src/server/core.rs.negctlA src/server/core.rs` restored the file with its ORIGINAL (older) mtime, so cargo saw nothing newer than the last build and reused the control-A library — making the reverted tree still fail `tools/list` while `git diff -- src/` was clean and `grep` showed the correct source. Resolved with `touch src/server/core.rs`. **This is a trap for any negative control in this repo:** after reverting, `touch` the file, or verify the next run actually recompiles. (`make lint` exited 0 throughout, because clippy re-read the reverted source — the staleness was confined to the test binary.)

**2. `git checkout -- <file>` on an UNCOMMITTED file discards the work, not just the control.** Reverting Task 2's Control B with `git checkout -- tests/common/duplex.rs` wiped `read_resource_request`, because Task 2's duplex change was not yet committed. Detected immediately by the acceptance greps and re-added. **Rule for future controls:** revert an edit-in-place control with the inverse edit, and reserve `git checkout --` for files whose surrounding work is already committed.

**3. Timing.** No flakiness observed; every suite ran in isolation, sequentially, per the environment note. Total wall time across all runs is dominated by two full `pmcp` recompiles (~2m30s each).

## Known Stubs

None. Every test in this plan drives a real dispatcher over a real transport or a real in-process seam; there are no placeholder assertions, no `todo!()`, and no fixture returning fabricated data in place of a dispatcher result.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file access and no schema change — it changes three files under `tests/` and zero production bytes. `T-115-03`, `T-115-04`, `T-115-19`, `T-115-20` and `T-115-39` from the plan's register are all mitigated by named tests listed above; `T-115-SC` holds trivially (no package installed, no manifest touched).

## Next Phase Readiness

- **115-08** (source tripwires) can rely on: `project_caching_hints` having exactly one writer per era on both native dispatchers, proven at the wire; and on `tests/v2_caching_hints.rs` failing loudly if a second writer appears.
- **115-09 / 115-10** should note the two acceptance-criterion substitutions (Deviations 3 and 4) so verification does not re-run the literal greps and report a false miss. The measured substitutes are `grep -c 'assert_v2_era_witness(&\|assert_no_v2_era_witness(&' tests/v2_caching_hints.rs` → 10 and the raw-text `"ttlMs":300000` / `"ttlMs":60000` assertions.
- **Deferred to 115-10, unchanged from 115-06:** response middleware still runs AFTER the projection and can rewrite `ttlMs` / `cacheScope`; this plan does not test that ordering (it is fenced by `response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation` in `src/server/core.rs`).
- **New for 115-10's consideration:** `resources/templates/list` has no handler hook at all, so a server author cannot set a caching hint on it. That is a pre-existing SDK gap (not introduced here) and is now documented at `HintFreeResources` in `tests/v2_caching_hints.rs`. It may be worth booking as a deferred item.
- **Ledger note (unchanged):** `.planning/REQUIREMENTS.md` books SCHM-01/02/03 Complete on contract-only evidence and `contracts/binding.yaml` still reads `status: planned` — both already logged for 115-10 (D-115-11-G, D-115-03-A, D-115-04-B, D-115-05-E). This plan does not touch either ledger.

---
*Phase: 115-json-schema-2020-12-structured-output-caching-hints*
*Completed: 2026-08-01*

## Self-Check: PASSED

All four claimed files exist on disk; all three claimed commits (`8aced935`, `078a3661`, `841c57ad`) exist in the repository.
