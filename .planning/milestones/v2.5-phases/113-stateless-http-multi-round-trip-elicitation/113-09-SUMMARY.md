---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 09
subsystem: api
tags: [mcp-2026-07-28, mrtr, streamable-http, elicitation, capabilities, security, semver]

# Dependency graph
requires:
  - phase: 113-06
    provides: "the minimal `core::mrtr_egress` (unconditional signal strip + round+1 mint), `MrtrEgressInputs`, `mrtr_binding_parts` derived from serde + MRTR_METHODS, and the two twin-site dispatch calls"
  - phase: 113-03
    provides: "the server-owned Arc<RequestStateCodec> + RequestBinding::from_request the egress mints through"
  - phase: 113-02
    provides: "MrtrSignal / InputRequests / InputRequest, the ONE MRTR_METHODS table, and the INPUT_REQUESTS_KEY / REQUEST_STATE_KEY / MRTR_SIGNAL_META_KEY wire constants"
  - phase: 113-01
    provides: "MISSING_REQUIRED_CLIENT_CAPABILITY (-32021) and INTERNAL_ERROR in the centralized error_codes table"
  - phase: 112
    provides: "inject_v2_result_envelope, ResponseDisposition, and the twin-site dispatch seam"
provides:
  - "core::strip_mrtr_signal + StrippedSignal — the unconditional three-state strip that runs before any era or eligibility branch"
  - "core::client_request_mrtr_eligible — the EXHAUSTIVE no-wildcard ClientRequest tripwire, gating mrtr_binding_parts on the production path"
  - "core::own_reserved_result_fields + core::result_meta_object_mut — the authoritative reserved-field registry and the one _meta accessor for all three result shapes"
  - "core::RESERVED_SERVER_INFO_KEY + pmcp::testing::META_SERVER_INFO — serverInfo at result._meta[\"io.modelcontextprotocol/serverInfo\"]"
  - "the submode-aware declared-client-capability precheck (-32021 with an object-shaped requiredCapabilities), evaluated BEFORE any minting"
  - "ReadResourceResult._meta — the third leg of the MRTR authoring surface"
  - "MrtrSignal::into_meta_entry — the documented, fallible handler authoring entry point"
affects: [113-10, 113-11, 113-12, 114, 118]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A three-state strip outcome (Absent / Present / Malformed) instead of an Option, so 'present but wrong' cannot degrade into 'absent' on a security-relevant path"
    - "Reserved protocol fields are SERVER-OWNED: overwrite-or-remove with a tracing::warn! naming the field, never collision-safe entry().or_insert"
    - "The disposition IS the ownership signal — `InputRequired` is constructed by exactly one function, so `disposition == InputRequired` needs no second `mrtr_owned` parameter"
    - "Ordering proven structurally rather than with a counter: run the precheck with NO codec, so a mint attempt would fail differently — getting -32021 is only possible if the check ran first"
    - "A set of enum members instead of five bools (clippy struct_excessive_bools caps a struct at three, and the set shape says the thing directly)"
    - "Nest a test suite in a module named after the production symbol so the plan's own `cargo test -- <symbol>` filter selects it instead of matching nothing"

key-files:
  created: []
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/simple_resources.rs
    - src/testing/mod.rs
    - src/types/mrtr.rs
    - src/types/resources.rs
    - tests/v2_required_headers.rs

key-decisions:
  - "strip_mrtr_signal returns a THREE-state `StrippedSignal`, not the plan's `Option<MrtrSignal>`. An Option collapses 'the reserved key was present but is not a well-formed MrtrSignal' into 'no signal', which would ship a silently EMPTY success for an operation the handler never completed — a regression against plan 06, where seal_input_required's parse failure produced INTERNAL_ERROR."
  - "The capability precheck landed WHOLE in Task 1 (kinds AND submodes) rather than kind-level in Task 1 + submodes in Task 2. Task 1's mrtr_egress calls it, so it must exist for Task 1 to compile; splitting one cohesive function across two commits would have been churn. Task 2 carries the submode TESTS and the -32021 payload-shape tests."
  - "The eligibility tripwire gates `mrtr_binding_parts`, not just the disposition selection. Putting it on the production path (rather than assertion-only) means an unclassified future ClientRequest variant is inert at ingress AND at egress, and the compile error lands before any behavior can depend on the silence."
  - "A TOP-LEVEL `serverInfo` is deliberately NOT in the reserved registry. It is a legitimate schema field of ServerDiscoverResult and InitializeResult; removing or overwriting it would corrupt those results. Only `_meta[\"io.modelcontextprotocol/serverInfo\"]` is server-owned, so `server/discover` is the one shape that carries both."
  - "RESERVED_SERVER_INFO_KEY lives in src/server/core.rs, not beside the request-side RESERVED_*_KEY constants in types/protocol/context.rs. Those three are all `params._meta` (ingress) keys read by the era resolver; this is a `result._meta` (egress) key written by exactly one function, which sits here. pmcp::testing::META_SERVER_INFO re-exports it so tests never re-spell the string."
  - "`own_reserved_result_fields` also removes `dev.pmcp/mrtr` — defense in depth, because `mrtr_egress` (the unconditional stripper) is `streamable-http`-only by D-14, so on a build without that feature NOTHING else would remove the key from a v2 result."
  - "The mint-counter the plan asks for is replaced by a stronger structural proof: run the precheck with `codec: None`. A mint attempt on that path fails with INTERNAL_ERROR ('no requestState codec configured'), so observing -32021 instead is only possible if the capability check short-circuited first."

patterns-established:
  - "Reserved-field registry as one table, documented in three places that must agree: the function that enforces it, the MRTR_SIGNAL_META_KEY rustdoc handler authors read, and the tests"
  - "Compile-time tripwires DERIVE their positive answers from the single source of truth (mrtr_eligible(CALL_TOOL_METHOD), not a bare `true`), so enum and table cannot drift in the permissive direction either"
  - "Additive `_meta` on a `#[non_exhaustive]` result struct is semver-clean; the same edit on a constructible struct is a MAJOR bump (D-113-D)"

requirements-completed: []

# Metrics
duration: 118min
completed: 2026-07-25
---

# Phase 113 Plan 09: Server-Side MRTR Egress Hardening Summary

**All three MRTR-eligible handler kinds can now signal "I need more input" through one documented `_meta` key; the pmcp-internal signal is stripped on EVERY path and a signal where MRTR is impossible fails LOUDLY instead of shipping a mangled "complete"; the declared-client-capability check is submode-aware, all-or-nothing and runs BEFORE any cryptographic work; `resultType`, `serverInfo`, `requestState` and `inputRequests` are SERVER-OWNED with a logged overwrite while non-reserved handler `_meta` survives untouched; and `serverInfo` now sits at the schema-correct `result._meta["io.modelcontextprotocol/serverInfo"]`.**

## Performance

- **Duration:** ~118 min
- **Tasks:** 3 (+1 verification-integrity commit)
- **Files modified:** 7

## Accomplishments

- **Closed the "signal where MRTR is impossible" hole loudly (Codex Plan-09 HIGH #1/#2).** Plan 06 already stripped the key unconditionally, so nothing leaked; but a stripped signal on v1 or on a non-eligible v2 method then produced a `complete` result for an operation the handler had not completed. It is now `INTERNAL_ERROR` with a `tracing::error!` naming the method. `egress_strips_the_internal_signal_on_every_path` greps the WHOLE serialized frame (not just the result object, which no longer exists on those paths) for both the reserved key and the plaintext continuation.
- **Made a malformed signal fail rather than vanish.** `strip_mrtr_signal` distinguishes `Absent` from `Malformed`. Under the plan's literal `Option<MrtrSignal>` return, a reserved key carrying a non-`MrtrSignal` payload would have become "no signal" — a silent empty success, and a regression against plan 06.
- **Reserved fields are SERVER-OWNED (Codex Plan-09 HIGH #3/#4 — the review's top consensus item).** `entry("resultType"` and `entry("serverInfo"` are both `0` (each was `1` pre-plan). A handler writing `resultType: "input_required"` on `tools/list` now gets `"complete"`, and its forged `requestState`/`inputRequests` are removed. The ownership scope is exactly the enumerated set: `non_reserved_handler_meta_survives` pins that `vendor/key` and `io.example/trace` come through untouched.
- **Two independent eligibility mechanisms that cannot drift.** `client_request_mrtr_eligible` is an exhaustive 18-arm match with zero `_ =>` arms, and its three eligible arms return `mrtr_eligible(CALL_TOOL_METHOD)` etc. rather than `true` — so the enum cannot become MORE permissive than `MRTR_METHODS` either. It gates `mrtr_binding_parts` on the production path. `binding_parts_cover_exactly_the_method_table` now drives from `MRTR_METHODS` instead of plan 06's hand-written list, so a new table row widens the test automatically.
- **Capability rejection precedes minting, provably.** `capability_precheck_precedes_minting` passes `codec: None`. A mint attempt on that path fails with `INTERNAL_ERROR`; observing `-32021` is only reachable if the check short-circuited first. A second assertion greps the rendered response for `requestState` and finds none.
- **Submode-aware `-32021` with an object payload.** Form vs URL elicitation, tool-augmented vs plain sampling, and roots each map to a distinct missing-capability projection: `{"elicitation":{}}`, `{"elicitation":{"url":{}}}`, `{"sampling":{}}`, `{"sampling":{"tools":{}}}`, `{"roots":{…}}`. `a_mixed_map_is_rejected_wholesale_never_partially_emitted` proves a DECLARED capability never appears in the missing set and that no partial `inputRequests` is emitted.
- **`serverInfo` relocated to the schema nesting.** `result._meta["io.modelcontextprotocol/serverInfo"]`, created when absent and merged into when the handler set other keys, overwritten when the handler set the reserved one. Seven stale placement assertions across three files were updated — exactly the build signal the plan wanted.
- **Held the milestone additive.** `ReadResourceResult` is `#[non_exhaustive]`, so adding `_meta` does not trip `constructible_struct_adds_field` (contrast D-113-D, where five constructible list-request structs forced a 3.0). `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` → **`223 checks: 223 pass, 30 skip / Summary no semver update required`**.

## Task Commits

| Task | Name | Commit | Key files |
| ---- | ---- | ------ | --------- |
| 1 | Handler signal → capability precheck → mint → `input_required`, stripped on EVERY path | `58416336` | `src/types/resources.rs`, `src/types/mrtr.rs`, `src/server/core.rs`, `src/server/simple_resources.rs` |
| 2 | Confine `input_required` to three methods, own the reserved fields, check SUBMODES | `89b31f75` | `src/server/core.rs` |
| 3 | Correct the `serverInfo` placement to `result._meta` | `ffa8866f` | `src/server/core.rs`, `src/server/mod.rs`, `src/testing/mod.rs`, `tests/v2_required_headers.rs` |
| — | Scope the envelope suite so its own test filter selects it | `ca065010` | `src/server/core.rs` |

The fourth commit is a verification-integrity fix, not new behavior — see "Deviations".

## Files Created/Modified

- **`src/types/resources.rs`** — `ReadResourceResult._meta: Option<Value>` with `rename = "_meta"` (defeating the struct-level `rename_all = "camelCase"` that caused D-113-A), `skip_serializing_if` and `default`; `new()` defaults it to `None`. The doc records why this is additive here and was not for the D-113-D structs.
- **`src/types/mrtr.rs`** — `MrtrSignal::into_meta_entry` (fallible, rustdoc'd with a runnable tool-handler example and the handler-idempotence requirement the D-15 re-run path implies), and the reserved-field registry table copied onto `MRTR_SIGNAL_META_KEY`'s doc alongside the recorded rationale for why the signal rides in `_meta` rather than a typed `HandlerOutcome` (a trait return-type change is a MAJOR break; right shape for a future 3.0).
- **`src/server/core.rs`** — `StrippedSignal` + `strip_mrtr_signal`, `eligible_mrtr_target`, `fail_mrtr_egress`, the restructured `mrtr_egress` (strip → fail-loud → precheck → mint), `reject_undeclared_capabilities`, `MissingCapability` + `MissingCapabilities` + `missing_client_capabilities`, `client_request_mrtr_eligible`, `RESERVED_SERVER_INFO_KEY`, `own_reserved_result_fields`, `result_meta_object_mut`, and `inject_v2_result_envelope` reduced to era/payload/object gates plus one delegation. 27 new unit tests across two nested suites.
- **`src/server/mod.rs`** — twin-site envelope tests updated to the new nesting; `test_server_dispatch_v1_no_envelope` gained an assertion that v1 gains no `_meta` at all.
- **`src/testing/mod.rs`** — `META_SERVER_INFO`, the response-side sibling of `META_PROTOCOL_VERSION` / `META_CLIENT_INFO` / `META_CLIENT_CAPABILITIES`.
- **`src/server/simple_resources.rs`** — two in-crate `ReadResourceResult` struct literals defaulted.
- **`tests/v2_required_headers.rs`** — three placement assertions moved to `_meta`, with `server/discover` asserting BOTH its own schema field and the envelope key.

## Verification

| Check | Result |
| ----- | ------ |
| `cargo test --lib --features full -- mrtr` | **115 passed** |
| `cargo test --lib --features full -- mrtr_egress` | **21 passed** |
| `cargo test --lib --features full -- inject_v2_result_envelope` | **16 passed** |
| `cargo test --lib --features full` | **1478 passed** |
| `cargo test --test v2_required_headers --features full` | 25 passed |
| `cargo test --test v2_mrtr_ingress --features full` | 10 passed |
| `cargo test --test v2_stateless_http --features full` | 23 passed |
| `cargo test --test v2_client --features full` | 21 passed |
| `cargo test --test common_harness_smoke --features full` | 7 passed |
| `cargo test --features full` (entire surface) | no failures |
| `cargo test --doc --features full -- mrtr` | 3 passed (incl. the new `into_meta_entry` example) |
| `cargo build --lib --target wasm32-unknown-unknown` | OK |
| `cargo build --lib --no-default-features` | OK (3 warnings, all pre-existing) |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 pass, 30 skip / no semver update required` |
| `/usr/bin/make quality-gate` (UNPROXIED) | **ALL TOYOTA WAY QUALITY CHECKS PASSED** |

The gate was run unproxied via `/usr/bin/make` with cargo at `/Users/guy/.cargo/bin/cargo`
(plan 03 proved the `rtk` shell proxy truncates the clippy stage and reports exit 0 for a run
that actually failed), and `git status --porcelain -- src/ tests/` was empty afterwards, so the
green gate is of the committed tree.

### Acceptance-criteria greps (baselines recorded as the plan requires)

| Grep | Pre-plan | Post-plan |
| ---- | -------- | --------- |
| `grep -c '#\[allow(dead_code)\]' src/server/core.rs` | 3 | **2** |
| `grep -c 'entry("resultType"' src/server/core.rs` | **1** | **0** |
| `grep -c 'entry("serverInfo"' src/server/core.rs` | **1** | **0** |
| `grep -c 'fn strip_mrtr_signal' src/server/core.rs` | 0 | 1 |
| `grep -c 'fn own_reserved_result_fields' src/server/core.rs` | 0 | 1 |
| `grep -c 'fn result_meta_object_mut' src/server/core.rs` | 0 | 2 (decl + call) |
| `_ =>` wildcard arms in `client_request_mrtr_eligible` | — | **0** |
| `RESERVED_SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo"` | 0 | 1 |
| `pub fn into_meta_entry` in `src/types/mrtr.rs` | 0 | 1 |
| `pub _meta` in `src/types/resources.rs` | 0 | 2 (field + doc-adjacent) |

Both `entry(...)` baselines were NON-zero before the refactor and are zero after, so the
assertions are live rather than vacuously true — the plan explicitly called this out because a
pattern ending in `")` would have matched nothing in either state.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `Option<MrtrSignal>` would have made a malformed signal vanish**

- **Found during:** Task 1
- **Issue:** The plan specifies `strip_mrtr_signal(result) -> Option<MrtrSignal>`. `None` then means both "no reserved key" and "the reserved key was present but is not a well-formed `MrtrSignal`". `mrtr_egress` returns `Complete` for the first, so the second would have shipped a silently EMPTY success for an operation the handler never completed — a REGRESSION against plan 06, where `seal_input_required`'s parse failure produced `INTERNAL_ERROR`.
- **Fix:** A three-state `StrippedSignal { Absent, Present(Box<MrtrSignal>), Malformed }`. `Malformed` fails loudly with `INTERNAL_ERROR` and a `tracing::error!`.
- **Files modified:** `src/server/core.rs`
- **Verification:** `egress_fails_loudly_on_a_malformed_signal`
- **Committed in:** `58416336`

**2. [Rule 3 - Blocking] The capability precheck cannot be split across Tasks 1 and 2**

- **Found during:** Task 1
- **Issue:** Task 1's `mrtr_egress` is specified to call "the Task-2 capability precheck", and Task 1's own acceptance criteria include a test that a failed precheck mints zero tokens. The function must therefore exist for Task 1's commit to compile.
- **Fix:** The full submode-aware check (kinds AND submodes) landed in Task 1's commit; Task 2 carries the submode TESTS, the object-shaped-payload test and the all-or-nothing test. No behavior was dropped or reordered — only the commit boundary moved.
- **Files modified:** `src/server/core.rs`
- **Verification:** Task 1's `capability_precheck_precedes_minting`; Task 2's six submode/payload tests
- **Committed in:** `58416336` (implementation), `89b31f75` (tests)

**3. [Rule 3 - Blocking] `clippy::struct_excessive_bools` on the missing-capability accumulator**

- **Found during:** Task 1 (`make lint`, not plain `cargo clippy`)
- **Issue:** The natural shape — five `bool` fields for elicitation / elicitation-url / sampling / sampling-tools / roots — exceeds clippy's three-bool cap, which is `-D warnings` in the gate.
- **Fix:** A `BTreeSet<MissingCapability>` over a five-variant enum. Also the better shape: these are members of a domain, not independent switches.
- **Files modified:** `src/server/core.rs`
- **Verification:** `make lint` clean
- **Committed in:** `58416336`

**4. [Rule 1 - Bug] Two plan verification commands matched ZERO tests**

- **Found during:** Task 3 close-out
- **Issue:** `cargo test --lib --features full -- mrtr_egress` and `-- inject_v2_result_envelope` both exited 0 while selecting nothing: the egress tests lived in `mod mrtr_ingest_tests` and the envelope tests were named `result_type_envelope_*` in the flat `tests` module. Both acceptance criteria and both `<verify><automated>` blocks would have passed vacuously — including after a regression.
- **Fix:** Nested each suite in a module named after the production symbol (`mod mrtr_egress`, `mod inject_v2_result_envelope`). The filters now select 21 and 16 tests respectively.
- **Files modified:** `src/server/core.rs`
- **Verification:** both filters reported above
- **Committed in:** `58416336` (egress), `ca065010` (envelope)

### Recorded deviations (not auto-fixes)

- **The `#[allow(dead_code)]` count criterion was written against a pre-plan-06 baseline.** The plan asks for the count to drop AND for `ResponseDisposition::InputRequired` to no longer carry the attribute. The second half was ALREADY true: plan 06 wired the variant and replaced its blanket allow with a load-bearing `#[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]` (MRTR is `streamable-http`-only by D-14, so without the feature the variant genuinely has no constructor). To satisfy the count half honestly, `ResponseDisposition::Task`'s BLANKET allow was scoped to `not(test)` — the same tightening plan 06 applied, and a real improvement: the test build (which is what `make quality-gate` runs) now lints it, so if Phase 114 never wires it and the exercising test is dropped, the gate says so. Count 3 → 2. **Measured and rejected:** removing the third occurrence, `#[allow(dead_code)]` on `ServerCore` itself, was tried and reverted — it suppresses three genuinely-unread fields (`sampling`, `roots_manager`, `subscription_manager`) that are pre-existing and out of this plan's scope.
- **`RESERVED_SERVER_INFO_KEY` lives in `src/server/core.rs`, not in `types/protocol/context.rs`.** The plan says "beside the other reserved `io.modelcontextprotocol/*` constants", while its own acceptance criterion requires the literal to be a named constant IN `core.rs`. Those two are only reconcilable one way, and the code supports the choice: the three constants in `context.rs` are all `params._meta` (ingress) keys read by the era resolver, while this is a `result._meta` (egress) key written by exactly one function. Both docs cross-reference each other, and `pmcp::testing::META_SERVER_INFO` gives tests a single reader.
- **The plan's "asserted with a mint-counter" became a structural ordering proof.** `RequestStateCodec` exposes no counter and adding one would be test-only production surface. Passing `codec: None` is strictly stronger: a mint attempt on that path fails with a DIFFERENT error (`INTERNAL_ERROR`, "no requestState codec configured"), so observing `-32021` can only mean the check ran first.
- **A TOP-LEVEL `serverInfo` is not removed.** Task 3's behavior list says "`result.serverInfo` (top level) is ABSENT from every v2 response", but `ServerDiscoverResult` and `InitializeResult` carry `serverInfo` as their OWN schema field. The plan's action text is precise where the behavior summary is not — it says to "replace the top-level `serverInfo` INSERTION", which is what was done. `server/discover` therefore carries both, and `every_v2_result_shape_carries_server_info_identically` plus the live discover test assert exactly that.

---

**Total deviations:** 4 auto-fixed (2 blocking, 1 missing-critical, 1 bug) + 4 recorded decisions
**Impact on plan:** No stated behavior was dropped. Every `<behavior>` bullet and every `<acceptance_criteria>` line is satisfied or explicitly reconciled above with measurement.

## Threat Flags

None. Every file touched is inside the plan's declared threat surface. No new network
endpoint, auth path, file access pattern, or schema change at a trust boundary was introduced.
`ReadResourceResult._meta` widens a RESULT type (server → client), not an ingress type, and the
one new public constant (`pmcp::testing::META_SERVER_INFO`) is behind the `testing` feature and
exposes no key material.

Two threat-register entries were STRENGTHENED beyond what the plan required:

- **T-113-60** — `own_reserved_result_fields` removes `dev.pmcp/mrtr` as well, so the internal
  signal cannot reach the wire even on a build WITHOUT `streamable-http`, where `mrtr_egress`
  (the unconditional stripper) is not compiled at all.
- **T-113-31** — the leak tests now grep the ENTIRE serialized JSON-RPC frame rather than the
  result object, and additionally assert the absence of the plaintext continuation payload
  itself, not just the key name.

## Known Stubs

None.

## Issues Encountered

- **`.pmat/*` and `pmcp-course/*` show as modified** in the working tree. They pre-date this
  plan and were deliberately NOT staged, per the executor scope boundary. `.planning/config.json`
  and `.planning/tmp/` were likewise left alone.
- **`cargo fuzz` targets do not build in this environment** (nightly `-Zsanitizer` unavailable).
  Pre-existing and non-fatal in the Makefile; not caused by and not in scope for this plan.

## TDD Gate Compliance

All three tasks are `tdd="true"`. Implementation and tests were committed together per task, so
there is no separate `test(...)` commit preceding each `feat(...)`. RED was verified by
construction and, for Task 3, by OBSERVATION: after the `serverInfo` relocation landed,
`cargo test --lib` reported exactly the two predicted failures
(`egress_emits_input_required_with_a_round_plus_one_token`,
`test_server_dispatch_injects_v2_result_envelope_parity`) and
`cargo test --test v2_required_headers` reported the two live-HTTP ones — the "a stale assertion
elsewhere would fail the build and is exactly the signal wanted" outcome the plan predicted,
observed before the assertions were updated. The plan's own Task-2 behavior
("a handler that writes `resultType: "input_required"` still produces `"complete"`") likewise
started RED against the Phase-112 `result_type_envelope_preserves_handler_disposition` test,
which asserted the opposite and had to be inverted. The fourth commit carries the `test(...)`
type.

## Next Phase Readiness

- **Plan 10 (`subscriptions/listen`)** — if it makes that method v2-capable it must classify the
  `Subscribe`/`Unsubscribe` variants in `client_request_mrtr_eligible` too; they are currently in
  the NOT-eligible arm. Adding a fourth `MRTR_METHODS` row is now sufficient to make a method
  bind end-to-end: `mrtr_binding_parts`, `mrtr_eligible` and the enum tripwire all derive from
  the table, and `enum_eligibility_agrees_with_the_method_table` will fail loudly if only one of
  the two mechanisms is updated.
- **Plan 11 (conformance)** — `sep-2322-not-on-unsupported-requests` can point at
  `exactly_three_client_request_variants_are_mrtr_eligible` and
  `handler_forged_input_required_is_overwritten_to_complete`. The `-32021` scenarios can point at
  the six submode tests, whose payloads are already the object shape the suite grades.
- **Plan 12 (public-API + semver audit)** — the only new PUBLIC surface is
  `ReadResourceResult._meta`, `MrtrSignal::into_meta_entry` and
  `pmcp::testing::META_SERVER_INFO`. Measurement unchanged at `223 checks: 223 pass, 30 skip`.
  Plan 12 must also re-verify TWO spec-pending items against the published `schema/2026-07-28`:
  the `-32021` value itself (already tracked in `113-SPEC-RECHECK.md`) and whether URL-mode
  elicitation support is still expressed as `ElicitationCapabilities.url` — the submode check
  reads that sub-field, and the rationale is recorded in `note_elicitation`'s rustdoc.
- **Phase 114 (Tasks)** — `ResponseDisposition::Task` is already emitted by the one shared
  envelope writer; wiring it is a disposition SELECTION at dispatch, with no envelope edit. Note
  that its dead-code allow is now `not(test)`-scoped, so the gate will report it the moment the
  exercising unit test is removed without the variant being wired.
- **HTTP-02 and HTTP-03 are NOT marked complete** — per the 113-01 recorded exception, plan 12
  owns the binding re-verification of the whole phase.

## Self-Check: PASSED

- All seven modified files exist on disk: `src/server/core.rs`, `src/server/mod.rs`,
  `src/server/simple_resources.rs`, `src/testing/mod.rs`, `src/types/mrtr.rs`,
  `src/types/resources.rs`, `tests/v2_required_headers.rs`.
- All four claimed commits (`58416336`, `89b31f75`, `ffa8866f`, `ca065010`) resolve in `git log`.
- Contract greps re-run and recorded in the table above; both `entry(...)` baselines were
  non-zero pre-plan and are zero post-plan, and the eligibility function has zero wildcard arms.
- Every `<acceptance_criteria>` line across the three tasks was executed as a command or a test
  and passes, except the `#[allow(dead_code)]` count, which passes (3 → 2) via the reconciliation
  recorded under "Recorded deviations".

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
