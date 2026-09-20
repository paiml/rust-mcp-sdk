---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 07
subsystem: api
tags: [mcp-2026-07-28, mrtr, client, elicitation, sampling, roots, semver, security]

# Dependency graph
requires:
  - phase: 113-02
    provides: "src/types/mrtr.rs — InputRequest/InputResponse::decode_for, InputRequiredResult, MrtrOutcome, MrtrRequestParams, splice_mrtr_params (stale-clearing), the ONE MRTR_METHODS table"
  - phase: 113-05
    provides: "the client v2 transport seam — send_raw, splice_v2_meta, send_untyped_request, the is_v2() era latch, and tests/v2_client.rs"
  - phase: 113-04
    provides: "the CODE-driven v2 HTTP status mapper that puts JSON-RPC error codes on 4xx responses (the reason D-113-E exists)"
  - phase: 113-06
    provides: "the server half of the loop — MRTR ingress, D-15 verdict routing, and the -32602-at-400 rejection this plan's client must be able to read"
provides:
  - "Error::mrtr_round_limit_exceeded / input_required_unfulfilled + their predicates and accessors — typed CLIENT-LOCAL errors on the existing Protocol variant"
  - "client::host::classify_input_request + ClientHostRegistry::preflight_input_requests — the inputRequests classifier and the before-any-handler fulfillability gate"
  - "Client::fold_input_requests — the all-or-nothing inputRequests -> inputResponses fold, routed through the FULL host pipeline"
  - "Client::send_with_mrtr — the bounded gather->resend loop with a fresh id and stale-key-free params per round"
  - "Client::call_tool_mrtr / get_prompt_mrtr / read_resource_mrtr returning MrtrOutcome<T>"
  - "ClientBuilder::mrtr_round_limit (default 8)"
  - "StreamableHttpTransport reads a JSON-RPC error envelope off a v2 non-2xx (D-113-E)"
affects: [113-09, 113-11, 113-12, 113-13, 114, 117]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Typed errors as named constructors + marker-discriminated predicates over an EXISTING enum variant, because the enum is not #[non_exhaustive]"
    - "One pipeline, two renderers: the shared host helper returns a TYPED outcome and each entry point decides how to serialize it"
    - "Preflight-before-invoke for any all-or-nothing fan-out that can prompt a human or spend an agent's tokens"
    - "A mock transport that replays a canned script and records the frames actually sent, so protocol assertions are against observed bytes"

key-files:
  created: []
  modified:
    - src/shared/streamable_http.rs
    - src/error/mod.rs
    - src/client/host/mod.rs
    - src/client/mod.rs
    - src/types/mrtr.rs
    - tests/v2_client.rs

key-decisions:
  - "Both MRTR errors ride the EXISTING Error::Protocol variant discriminated by a stable data.pmcpError marker string, because pmcp::Error is not #[non_exhaustive] and a new variant is a MAJOR semver break; the rationale is rustdoc'd at the constructors so a future contributor does not 'fix' them into variants"
  - "input_required_result() returns an OWNED InputRequiredResult, not a reference, because the payload is stored serialized inside the error's data — which is exactly what keeps the change additive"
  - "The error carries result.raw (the VERBATIM server result object) rather than the modeled projection, so unmodelled keys survive the round trip"
  - "The shared sampling pipeline returns a TYPED HostSamplingCompletion, not a serialized Value: the v1 host response must keep carrying the full CreateMessageResultWithTools while an MRTR inputResponses value is spec-typed as a CreateMessageResult"
  - "A WithTools-only client answers an MRTR sampling entry via the SAME projection the result-review gate uses (project_with_tools_to_legacy) — otherwise it would advertise the sampling capability and then under-supply forever"
  - "classify_input_request delegates to the STRING classifier so there is exactly ONE mapping and the production path is the one the property test drives"
  - "A missing resultType is TERMINAL, not an error: only an explicit input_required continues the loop, so any later result type (Phase 114's task) composes without touching the loop"
  - "D-113-E is gated on the transport's private v2_mode latch and is deliberately strict (jsonrpc == 2.0 AND an error member), so a proxy's JSON error document is never laundered into a server-authored protocol error"

patterns-established:
  - "MRTR fields are written ONLY through splice_mrtr_params, which removes both keys before inserting — a resend structurally cannot carry an earlier round's data"
  - "requestState is moved, never read: no code path deserializes or inspects it, with the spec prohibition cited at the assignment site"
  - "Rounds are counted per LOGICAL round, so a multi-key inputRequests map costs one"

requirements-completed: []

# Metrics
duration: 41min
completed: 2026-07-25
---

# Phase 113 Plan 07: Client MRTR Loop Summary

**A pmcp v2 `Client` now answers a server's `inputRequests` from its already-registered Phase-106 handlers — through the FULL approval/result-review pipeline, only once every requested kind is known fulfillable — and resends the original request with a fresh JSON-RPC id, a stale-key-free params object and a verbatim-echoed `requestState`, up to a configurable bound; and an `input_required` result the client cannot answer now reaches the caller as `MrtrOutcome::InputRequired` or a typed error carrying the verbatim result, instead of a silently EMPTY `CallToolResult`.**

## Performance

- **Duration:** ~41 min
- **Tasks:** 3 (+ the owned D-113-E fix)
- **Files modified:** 6

## Accomplishments

- **Closed D-113-E, which this plan owned.** `post_body` turned any non-2xx into an opaque `TransportError::Request`, so a v2 client could not read `error.code` at all — even though plan 04 maps the v2 codes onto 4xx and plan 06 answers a tampered/expired `requestState` with `-32602` at HTTP 400. A v2-gated envelope reader now surfaces those structurally. v1 is byte-identical.
- **Fixed the return-type defect the cross-AI review found (Codex Plan-07 HIGH #1).** `CallToolResult.content` carries `#[serde(default)]`, so an `input_required` result deserialized into a silently EMPTY success; `ReadResourceResult.contents` has no default, so the same result failed to deserialize at all. Neither is "returns the result to the caller" (D-06). The existing methods now return `Err(Error::input_required_unfulfilled(result))` and the additive `*_mrtr` siblings return it as a value. `no_handler_existing_method_returns_typed_error` asserts explicitly that an `Ok(CallToolResult)` is a FAILURE.
- **Held the milestone additive.** Two typed errors, zero new `Error` variants. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` → **`223 checks: 223 pass, 30 skip / no semver update required`**.
- **Preflight before any handler runs (Codex Plan-07 MEDIUM).** `preflight_input_requests` proves EVERY entry's kind is fulfillable before anything is invoked. `preflight_failure_invokes_zero_handlers` uses an invocation counter over a map whose first entry is fulfillable and whose second is not.
- **The fold reuses the FULL host pipeline (Codex Plan-07 MEDIUM / T-113-57).** `on_sampling_approval` and `on_sampling_result_review` apply on the v2 path exactly as on v1, proven by `rejecting_approval_yields_cannot_fulfil` (zero handler calls) and `result_review_runs_on_a_sampling_result`.
- **Every unfulfillable path is LOGGED, not swallowed (Codex Plan-07 MEDIUM).** A missing handler, a policy denial, a handler error, a kind-mismatched response and a declined elicitation each emit a `tracing::warn!` naming the entry key and the reason.
- **The elicitation handler signature is untouched.** `grep -c 'fn handle_elicitation' src/client/host/elicitation.rs` is `1`, unchanged. One callback serves v1 interactive elicitation and v2 MRTR (D-06/D-10).

## Task Commits

| Task | Name | Commit | Key files |
| ---- | ---- | ------ | --------- |
| 0 | D-113-E — preserve the JSON-RPC error envelope on a v2 non-2xx | `cec054d4` | `src/shared/streamable_http.rs` |
| 1 | Two typed client-local errors without a new `Error` variant | `7e045166` | `src/error/mod.rs` |
| 2 | Classify + preflight `inputRequests`, fold through the full host pipeline | `964b1c16` | `src/client/host/mod.rs`, `src/client/mod.rs` |
| 3 | The bounded gather→resend loop and the `MrtrOutcome` public API | `774bdcef` | `src/client/mod.rs`, `src/types/mrtr.rs`, `tests/v2_client.rs`, `src/client/host/mod.rs` |

## Files Created/Modified

- **`src/shared/streamable_http.rs`** — `jsonrpc_error_envelope` (a strict, v2-only reader) plus the branch in `post_body` that feeds a 4xx-carried JSON-RPC error through the normal response channel. 3 new `mockito`-driven unit tests.
- **`src/error/mod.rs`** — `MRTR_ROUND_LIMIT_MARKER` / `MRTR_INPUT_REQUIRED_MARKER`, `mrtr_round_limit_exceeded` / `is_` / `mrtr_round_limit()`, `input_required_unfulfilled` / `is_` / `input_required_result()`, and the two private readers `protocol_data` / `pmcp_error_marker`. 8 new unit tests + 2 doctests.
- **`src/client/host/mod.rs`** — `classify_input_request`, `classify_input_method`, the private `host_kind_of` (the ONE mapping), and `ClientHostRegistry::{can_fulfil, preflight_input_requests}`. 6 new unit tests + 1 proptest.
- **`src/client/mod.rs`** — `HostRefusal`, `HostSamplingCompletion`, `FoldOutcome`, `RoundOutcome`, `MrtrLoopOutcome`, `DEFAULT_MRTR_ROUND_LIMIT`; the extracted `run_host_sampling` / `answer_host_sampling` / `answer_mrtr_sampling` / `answer_host_elicitation` / `answer_host_roots` / `host_value` / `host_response`; `fold_input_requests`; `send_with_mrtr` + `mrtr_round_step`; the three `*_mrtr` public methods; the v2 branch inside `call_tool` / `get_prompt` / `read_resource`; and `ClientBuilder::mrtr_round_limit`. 9 new unit tests.
- **`src/types/mrtr.rs`** — `CALL_TOOL_METHOD` / `GET_PROMPT_METHOD` / `READ_RESOURCE_METHOD` derived from `MRTR_METHODS` + a row-order drift guard.
- **`tests/v2_client.rs`** — a `MockV2Transport` that replays a canned script and records the frames the client actually sent, driving 11 MRTR tests. The file now holds 21 tests (plan 05's 10 live + these 11).

## Verification

| Check | Result |
| ----- | ------ |
| `cargo test --lib --features full -- error::` | 25 passed |
| `cargo test --lib --features full -- host` | 33 passed |
| `cargo test --lib --features full -- client` | 147 passed |
| `cargo test --test v2_client --features full` | **21 passed** (plan requires ≥19) |
| `cargo test --lib --features full` | 1439 passed |
| `cargo test --test v2_mrtr_ingress --features full` | 10 passed |
| `cargo test --test v2_stateless_http --features full` | 15 passed |
| `cargo test --test v2_required_headers --features full` | 25 passed |
| `cargo test --test common_harness_smoke --features full` | 7 passed |
| `cargo build --lib --target wasm32-unknown-unknown` | OK |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 pass, 30 skip / no semver update required` |
| `/usr/bin/make quality-gate` (UNPROXIED) | **ALL TOYOTA WAY QUALITY CHECKS PASSED** |

The gate was run unproxied via `/usr/bin/make` with cargo at `/Users/guy/.cargo/bin/cargo` (plan 03 proved the `rtk` shell proxy truncates the clippy stage and can report exit 0 for a run that failed), and `git status --porcelain -- src/ tests/` was empty afterwards — so the green gate is of the committed tree.

Note that plan 05's ten LIVE tests are the strongest regression evidence here: `emits_required_headers`, `mcp_name_from_uri_for_resources_read` and `mcp_name_from_name_for_prompts_get` now execute through `send_with_mrtr` against a real server, so the rerouting of `call_tool` / `get_prompt` / `read_resource` onto the loop is proven end-to-end, not just against the mock.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] A `WithTools`-only sampling handler cannot answer an MRTR sampling entry**

- **Found during:** Task 2 (`folds_a_with_tools_only_sampling_handler` failed on the first run)
- **Issue:** The plan says a sampling entry "produces an `InputResponse` from the handler's `CreateMessageResult`" for **either** registered shape. But `HostSamplingHandlerWithTools` produces a `CreateMessageResultWithTools`, whose `content` is an ARRAY, and `InputResponse::decode_for(Sampling, ..)` deserializes a `CreateMessageResult`, whose `content` is a single value. The decode failed, so a `WithTools`-only client would have advertised `sampling` (plan 05 deviation #6 made it do so deliberately) and then under-supplied every round until the bound tripped.
- **Fix:** The shared pipeline `run_host_sampling` returns a TYPED `HostSamplingCompletion` instead of a serialized value, and the two entry points render it differently: the v1 host response serializes the tool-aware result in FULL (unchanged wire behavior), while `answer_mrtr_sampling` projects it down through the SAME `project_with_tools_to_legacy` the result-review gate already used. `project_with_tools_for_review` was renamed to that (two consumers now).
- **Files modified:** `src/client/mod.rs`
- **Verification:** `folds_a_with_tools_only_sampling_handler`, plus `test_sampling_result_review_deny_after_handler` and the other pre-existing host tests still green.
- **Committed in:** `964b1c16`

**2. [Rule 3 - Blocking] `classify_input_request` cannot return `Unhandled`, but the plan requires that arm and a proptest over arbitrary method strings**

- **Found during:** Task 2
- **Issue:** The plan's signature is `classify_input_request(req: &InputRequest) -> HostRequestKind` with "anything else → `Unhandled`" plus "a proptest asserting it never panics over arbitrary method strings". `InputRequest` is a closed 3-variant adjacently-tagged enum, so an unknown method cannot be represented at all and the `Unhandled` arm would be unreachable.
- **Fix:** Added `classify_input_method(method: &str)` — the string half, total over arbitrary input with the `Unhandled` fallback — and made `classify_input_request` DELEGATE to it. There is exactly one mapping (`host_kind_of`), the production path is the one the proptest drives, and the plan's named function keeps its signature.
- **Files modified:** `src/client/host/mod.rs`
- **Verification:** `classify_method_is_unhandled_for_anything_else`, `classify_input_method_never_panics` (proptest)
- **Committed in:** `964b1c16`, delegation tightened in `774bdcef`

**3. [Rule 1 - Bug] `Error::input_required_unfulfilled` would have lost the verbatim result**

- **Found during:** Task 1
- **Issue:** The plan says the error's `data` carries "the serialized `InputRequiredResult` (**including its verbatim `raw`**)". `InputRequiredResult.raw` is `#[serde(skip_serializing)]`, so `to_value(&result)` drops it and every unmodelled key the server sent with it.
- **Fix:** The error carries `result.raw` directly whenever it is an object (it is a superset of the modeled fields and is what `InputRequiredResult`'s own `Deserialize` reconstructs from), falling back to the modeled projection for a hand-built value.
- **Files modified:** `src/error/mod.rs`
- **Verification:** `input_required_error_round_trips_the_result` asserts `recovered.raw == raw` including a `somethingUnmodelled` key.
- **Committed in:** `7e045166`

### Intentional simplifications (recorded, not auto-fixes)

- **`input_required_result()` returns an OWNED value, not `Option<&InputRequiredResult>`.** The plan's `<behavior>` bullet says `Some(&InputRequiredResult)`; its `<action>` section says `Option<InputRequiredResult>`. A reference is impossible — the payload lives serialized inside `Error::Protocol.data`, which is precisely what makes the error additive. The `<acceptance_criteria>` line ("round-trips the `request_state`") is satisfied either way.
- **`get_prompt_mrtr` takes `HashMap<String, String>`, not the plan's `Option<..>`.** It mirrors the existing `get_prompt` signature exactly, which is the point of an additive sibling.
- **`send_with_mrtr` clamps the limit to `max(1)`.** A `mrtr_round_limit(0)` would otherwise send nothing at all and report a round-limit breach for a request that never left — a confusing lie.
- **The `*_mrtr` methods delegate on v1** rather than erroring, so application code that migrates to them keeps working on a non-opted-in connection and always sees `MrtrOutcome::Complete`.
- **A missing `resultType` is treated as TERMINAL**, alongside any non-`input_required` value. Only an explicit `input_required` continues the loop, which is both the conservative reading and what makes Phase 114's `"task"` compose for free.

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug) + 5 recorded simplifications
**Impact on plan:** No stated behavior was dropped; every `<behavior>` bullet and every `<acceptance_criteria>` line is satisfied. Deviations 1 and 2 were required to make plan-stated behaviors reachable at all.

## The `send_notification` gap — assessed, NOT blocking (recorded as D-113-F)

The executor prompt flagged that `Client::send_notification` never received plan 05's v2 branch, so on a v2 client `cancel_request` / `send_progress` / `notify_roots_list_changed` go out with the v2 headers but **no `_meta` era key**, which pmcp's own gate rejects as `-32020 HEADER_MISMATCH` at HTTP 400. Client→server RESPONSES to server-initiated sampling/elicitation have the same shape.

**It does not block this plan, and nothing was refactored for it.** The MRTR loop sends nothing that is not a request: `inputRequests` are answered **locally** from the host registry, and the answers travel back as `params.inputResponses` on the next `tools/call` / `prompts/get` / `resources/read` **request**, which goes through `dispatch_request`'s v2 raw-frame path and is fully conformant. The host-reply `send` inside `dispatch_request` is likewise unreachable on a conformant v2 connection, because the spec forbids a v2 server sending independent requests — MRTR replaces that direction entirely.

Recorded in `deferred-items.md` as **D-113-F**, owner **plan 13** (client `subscriptions/listen`), with the affected call table and the fix shape (mirror `dispatch_request`'s `is_v2()` branch through `create_notification` + `splice_v2_meta` + `send_raw`, and thread `send_raw`'s hard-coded `is_notification: false` through).

## Threat Flags

None. Every file touched is inside the plan's declared threat surface. The one net-new inbound behavior — the transport accepting a JSON-RPC error body off a non-2xx (D-113-E) — is a NARROWING of trust, not a widening: it is v2-gated, requires `jsonrpc == "2.0"` **and** an `error` member, and its failure mode is the pre-existing status-only error. Every threat the plan assigned a `mitigate` disposition is covered by a named test:

| Threat | Mitigation evidence |
| ------ | ------------------- |
| T-113-11 (server loops the client) | `round_limit` (exact send count + typed error), `default_round_limit_is_eight` |
| T-113-07 (id replay across rounds) | `retry_uses_new_id` (pairwise-distinct ids the mock OBSERVED) |
| T-113-25 (client parsing `requestState`) | no code path deserializes it; the spec prohibition is cited at the assignment site |
| T-113-26 (fabricating a response) | `FoldOutcome::CannotFulfil` is all-or-nothing; `no_handler_returns_outcome` asserts no resend |
| T-113-27 (coaxing a human through repeated elicitation) | `preflight_failure_invokes_zero_handlers` + the bound applies identically to both client shapes |
| T-113-28 (stale MRTR fields on a resend) | `retry_carries_no_stale_mrtr_fields` (round 3 params equal exactly round 2's values) |
| T-113-56 (unfulfilled result silently emptied) | `no_handler_existing_method_returns_typed_error` fails the test if `Ok(CallToolResult)` is returned |
| T-113-57 (MRTR bypassing approval/result-review) | `rejecting_approval_yields_cannot_fulfil`, `result_review_runs_on_a_sampling_result` |

## Known Stubs

None.

## Issues Encountered

- **The Task-2 commit is momentarily dead-code-warning-bearing.** `fold_input_requests` and its helpers have no non-test caller until Task 3 wires `send_with_mrtr`, so `make lint`'s `--lib` pass (which compiles without `cfg(test)`) would have reported `dead_code` at `964b1c16` in isolation. The alternative — a temporary `#[allow(dead_code)]` removed one commit later — is worse churn for the same information. The gate was run and is GREEN on the final tree, with `git status --porcelain -- src/ tests/` empty.
- **`.pmat/*` and `pmcp-course/*` show as modified** in the working tree. They pre-date this plan and were deliberately NOT staged, per the executor scope boundary. `.planning/config.json` and `.planning/tmp/` were likewise left alone.

## TDD Gate Compliance

Tasks 1–3 are `tdd="true"`. Implementation and tests were committed together per task, so there is no separate `test(...)` commit preceding each `feat(...)` commit — the same pattern plans 05 and 06 recorded. RED was verified by construction and, in two cases, observed directly:

- **Observed RED:** `folds_a_with_tools_only_sampling_handler` FAILED on its first run (the `CreateMessageResultWithTools` array-vs-single `content` mismatch) and went GREEN only after the projection landed — deviation 1 above.
- **Verified by construction:** every other test asserts a behavior that did not exist before its commit — there was no `mrtr_round_limit_exceeded`, no `classify_input_request`, no `fold_input_requests` and no `send_with_mrtr`. `v2_surfaces_a_jsonrpc_error_carried_on_a_400` is RED by inspection against the pre-`cec054d4` code, which returned `Err` for every non-2xx before reading a byte of the body.

## Next Phase Readiness

- **Plan 09 (egress hardening)** now has a client that can READ its `-32021 MissingRequiredClientCapability` — D-113-E was the blocker. `error.data.requiredCapabilities` arrives intact on the `Error::Protocol` `data` member.
- **Plan 11 (conformance / live MRTR)** has both halves: this plan's client loop and plan 06's ingress. It owns the live client↔server MRTR tests this plan deliberately did not add (Codex Plan-07 HIGH #2), and can point `sep-2322-multi-round-r1/r2/r3` at `mrtr_multi_round`'s shape.
- **Plan 12 (public-API + semver audit)** should note the new public surface: `Error::{mrtr_round_limit_exceeded, is_mrtr_round_limit_exceeded, mrtr_round_limit, input_required_unfulfilled, is_input_required_unfulfilled, input_required_result}`, `pmcp::error::{MRTR_ROUND_LIMIT_MARKER, MRTR_INPUT_REQUIRED_MARKER}`, `ClientBuilder::mrtr_round_limit`, `Client::{call_tool_mrtr, get_prompt_mrtr, read_resource_mrtr}`, and the `#[doc(hidden)]` `client::host::classify_input_request`. Current measurement is unchanged at `223 checks: 223 pass, 30 skip`.
- **Plan 13 (client subscriptions/listen)** inherits **D-113-F** — give `send_notification` the v2 branch.
- **Phase 114 (Tasks)** composes without touching this loop: `non_input_required_result_type_is_terminal` pins that a `resultType` of `"task"` returns immediately and invokes no handler.
- **CLNT-02 is NOT marked complete** — per the 113-01 recorded exception, plan 12 owns the binding re-verification of the whole phase.

## Self-Check: PASSED

- All claimed files exist on disk: `src/shared/streamable_http.rs`, `src/error/mod.rs`, `src/client/host/mod.rs`, `src/client/mod.rs`, `src/types/mrtr.rs`, `tests/v2_client.rs`, `deferred-items.md`, and this SUMMARY.
- All four claimed commits (`cec054d4`, `7e045166`, `964b1c16`, `774bdcef`) resolve in `git log`.
- Acceptance-criteria greps: `src/error/mod.rs` carries all eight required symbols; `src/client/host/mod.rs` carries `pub fn classify_input_request` (1) and `fn preflight_input_requests` (1); `src/client/mod.rs` carries `fold_input_requests`, `Fulfilled`, `CannotFulfil`, `decode_for`, `pub fn mrtr_round_limit`, `fn send_with_mrtr`, the three `*_mrtr` methods, `splice_mrtr_params` and `input_required_unfulfilled`; `grep -c 'fn handle_elicitation' src/client/host/elicitation.rs` is `1`; all ten required test fn names are present in `tests/v2_client.rs`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
