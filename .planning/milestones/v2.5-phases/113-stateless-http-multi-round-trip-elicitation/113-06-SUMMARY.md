---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 06
subsystem: api
tags: [mcp-2026-07-28, mrtr, streamable-http, request-state, aead, security, semver]

# Dependency graph
requires:
  - phase: 113-02
    provides: "src/types/mrtr.rs — extract_mrtr_params (fail-loud Result), splice_mrtr_params, salient_param_digest, the ONE MRTR_METHODS table, MrtrSignal/InputRequests, and every MAX_* ingress bound"
  - phase: 113-03
    provides: "the SERVER-OWNED Arc<RequestStateCodec> on Server + ServerCore, RequestBinding::from_request, and the Verdict enum whose Expired arm carries the decrypted Continuation"
  - phase: 113-04
    provides: "the raw-body v2 header gate (run_v2_header_gate), the widened V2GateOutcome::Reject { code, message, data }, and the CODE-driven v2 HTTP status mapper"
  - phase: 112
    provides: "ProtocolContext + its with_* builder extension point, handle_request_with_context (the twin-site dispatch seam), inject_v2_result_envelope, and ResponseDisposition::InputRequired"
provides:
  - "ProtocolContext MRTR carrier — crate-private `mrtr` (raw ingress params) + `mrtr_verified` (decrypted continuation), with `with_mrtr_params` / `with_verified_continuation` / `without_mrtr`"
  - "RequestHandlerExtra::{input_responses, mrtr_continuation, mrtr_round} — the handler-facing MRTR surface"
  - "core::mrtr_ingest + MrtrIngest::apply — the ONE shared D-15 verdict router, called from BOTH native dispatch sites"
  - "core::mrtr_egress — the minimal input_required emission plan 09 hardens, with an unconditional internal-signal strip on every path"
  - "core::ANONYMOUS_PRINCIPAL + the fail-closed principal rule"
  - "pmcp::testing::{mint_request_state, open_request_state} — the production-codec seam integration tests mint and open tokens through"
  - "tests/v2_mrtr_ingress.rs — live-HTTP acceptance for all four verdicts"
affects: [113-07, 113-09, 113-11, 113-12, 114]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "One raw-body parse per HTTP ingress, shared by the era read, the header cross-check and the MRTR params read — the three readers cannot disagree"
    - "Present-but-unusable is REJECTED, never treated as absent: a Result-returning extractor whose Err short-circuits into the transport's rejection path"
    - "AAD bindings derived from the TYPED request dispatch will execute, never from an attacker-echoed params copy"
    - "Strip-and-re-run as the degraded-continuation mechanic: clear every signal on the threaded context, invoke the ORIGINAL handler, carry only the round forward"
    - "Feature-scoped `#[cfg_attr(not(feature = ...), allow(dead_code))]` instead of a blanket allow, so the gated build still errors on real dead code"

key-files:
  created:
    - tests/v2_mrtr_ingress.rs
  modified:
    - src/types/protocol/context.rs
    - src/server/streamable_http_server.rs
    - src/server/cancellation.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/testing/mod.rs

key-decisions:
  - "The ProtocolContext MRTR field and its write-side builders are pub(crate), NOT pub: MrtrRequestParams is pub(crate) by D-10, so a pub field would expose a private type from a public interface (the `private_interfaces` lint, denied under -D warnings). Handlers reach the values through the three pub RequestHandlerExtra accessors instead, which is a smaller public-API delta than the plan's literal `pub fn with_mrtr_params`."
  - "mrtr_ingest is called ONCE per dispatch site — before handle_request_internal in core.rs and before the request match in mod.rs — rather than separately inside the CallTool/GetPrompt/ReadResource arms. The helper already dispatches on the request variant via mrtr_binding_parts, so three call sites per file would have been three copies of the same decision."
  - "The resolved verdict rides on the ProtocolContext (which is already threaded end-to-end into RequestHandlerExtra) rather than on new parameters through handle_client_request -> process_client_request -> handle_call_tool. That is the plan's 'do not add a second plumbing channel' rule taken literally."
  - "Plan 06 ships a MINIMAL mrtr_egress. Without it, two of the plan's own must-haves ('producing a fresh input_required with real inputRequests') and four of Task 3's ten acceptance tests are unreachable, because the handler's signal has nothing to convert it. Plan 09 keeps ownership of the hardening: the declared-capability precheck before minting, the exhaustive eligible-method tripwire, full reserved-field ownership, ReadResourceResult._meta, and the serverInfo relocation."
  - "Expiry is exercised with a zero TTL (exp == now), not a FixedClock or a sleep — RequestStateClock is pub(crate) and unreachable from an integration test, and a 1-second server TTL would risk the server's own reply token expiring before the test could open it."
  - "The client-facing MRTR rejection is ONE generic string for tamper, wrong principal and cross-request replay alike; the discriminated reason is tracing::warn!-logged server-side only (T-113-10)."

patterns-established:
  - "MRTR params travel on the ProtocolContext, never as typed request-struct fields — the D-113-D route, re-affirmed"
  - "A verdict enum whose `apply` folds into the threaded context and returns Err for the reject arm, so the handler physically cannot run on a rejection"
  - "Integration tests configure the requestState key through the BUILDER and mint through a documented pmcp::testing wrapper over the production codec — never through process-global env mutation"

requirements-completed: []

# Metrics
duration: 95min
completed: 2026-07-25
---

# Phase 113 Plan 06: Server-Side MRTR Ingress Summary

**A v2 retry carrying `inputResponses` + `requestState` now reaches the handler with the continuation decrypted and the round intact; a tampered, cross-principal or cross-request token is a JSON-RPC error the handler never sees; a malformed field is rejected at HTTP ingress instead of silently looking absent; and an unknown-key or expired token strips the MRTR fields and RE-RUNS the original handler, producing a real `input_required` with answerable `inputRequests` and a preserved round.**

## Performance

- **Duration:** ~95 min
- **Tasks:** 3
- **Files modified:** 6 (+1 created)

## Accomplishments

- **Closed the "malformed silently became `None`" hole (T-113-44).** `extract_mrtr_params` returns a `Result`, and the transport now maps every one of its eight error shapes to `-32602` at HTTP 400 **before dispatch**. Previously a wrong-shaped `requestState` would have looked absent and bypassed the entire verdict table. Eight malformed shapes are unit-tested at the gate and two more live over HTTP.
- **Implemented the D-15 table exactly, in ONE place.** `core::mrtr_ingest` is the single verdict router; `server/mod.rs` calls it and defines nothing (`grep -c 'fn mrtr_ingest' src/server/mod.rs` is `0`). `Ok` resumes, `AuthFailed` rejects, `UnknownKey` re-elicits from round 0, `Expired(c)` re-elicits **at `c.round`** so a hostile server cannot reset a client's D-09 bound by letting tokens expire (T-113-49).
- **Made the re-elicitation actually answerable — the consensus fix.** `Reelicit` does not synthesize a state-only result. It clears every MRTR signal from the threaded context and lets the ORIGINAL handler run as a pristine first call, so the response carries the real `inputRequests` the handler generates. `unknown_key_reelicits_with_input_requests` asserts the map is non-empty over live HTTP; the recording handler asserts all three accessors were `None` inside the re-run.
- **Fail-closed identity (T-113-22 / T-113-06).** The principal expression has exactly one source (`AuthContext::subject`), one named `ANONYMOUS_PRINCIPAL` constant, and no session-id branch. A server WITH an auth provider refuses MRTR outright to an unauthenticated caller — verification is not even attempted, at ingress or at egress.
- **Held the milestone additive.** No public request struct gained a field; the only new public surface is three `RequestHandlerExtra` accessors and two `pmcp::testing` wrappers. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` → **`223 checks: 223 pass, 30 skip / Summary no semver update required`**.

## Task Commits

| Task | Name | Commit | Key files |
| ---- | ---- | ------ | --------- |
| 1 | MRTR params on `ProtocolContext` + reject malformed fields at v2 ingress | `fcd047bb` | `src/types/protocol/context.rs`, `src/server/streamable_http_server.rs` |
| 2a | `RequestHandlerExtra` MRTR accessors | `a3d5fdd2` | `src/server/cancellation.rs` |
| 2b | Verify at dispatch, route the D-15 verdicts, minimal egress | `de06a1d1` | `src/server/core.rs`, `src/server/mod.rs`, `src/types/protocol/context.rs`, `src/server/streamable_http_server.rs` |
| 3 | Live-HTTP acceptance for the verdict table | `52d8b8dc` | `tests/v2_mrtr_ingress.rs`, `src/testing/mod.rs` |

Task 2 landed as two commits: the accessors were complete and independently
compile-clean, so they were committed for durability before the larger dispatch
unit (an earlier run of this plan was interrupted mid-task by a transport error).

## Files Created/Modified

- **`src/types/protocol/context.rs`** — `ProtocolContext` gains crate-private `mrtr` (the raw ingress params) and `mrtr_verified` (a `VerifiedContinuation { state, round }`), three read accessors, and a separate write-side `impl` block carrying `with_mrtr_params` / `with_verified_continuation` / `without_mrtr` / `request_state_token`. 4 new unit tests.
- **`src/server/streamable_http_server.rs`** — the v2 gate now parses the raw body **exactly once** (`raw_body_json`) and feeds `params_meta_of`, `method_and_name_of` and the new `params_of` from that single value; `attach_v2_mrtr_params` extracts on the accepted-v2 path only and converts any `MrtrParseError` into an `INVALID_PARAMS` rejection. 5 new unit tests (one covering all eight malformed shapes).
- **`src/server/cancellation.rs`** — `input_responses()` / `mrtr_continuation()` / `mrtr_round()`, each rustdoc'd with the trust contrast (client-supplied and untrusted vs. server-minted and verified).
- **`src/server/core.rs`** — the shared MRTR unit: `ANONYMOUS_PRINCIPAL`, `MRTR_REJECT_MESSAGE`, `MrtrPrincipal`, `resolve_mrtr_principal`, `mrtr_binding_parts`, `MrtrIngest` + `MrtrIngestInputs` + `mrtr_ingest` + `route_mrtr_verdict` + `MrtrIngest::apply`, and `MrtrEgressInputs` + `mrtr_egress` + `take_mrtr_signal` + `seal_input_required`. Wired into `ProtocolHandler::handle_request`. 19 new unit tests.
- **`src/server/mod.rs`** — `Server::handle_request_with_context` calls the SAME two helpers; the inner client-request future is `Box::pin`ned because the new locals pushed it past clippy's `large_futures` threshold.
- **`src/testing/mod.rs`** — `mint_request_state` / `open_request_state` (production-codec wrappers) plus `ANONYMOUS_PRINCIPAL` and `MRTR_SIGNAL_META_KEY` re-exports.
- **`tests/v2_mrtr_ingress.rs`** — 675 lines, 10 live-HTTP tests.

## Verification

| Check | Result |
| ----- | ------ |
| `cargo test --lib --features full -- protocol::context` | 19 passed |
| `cargo test --lib --features full -- mrtr_ingest` | 19 passed |
| `cargo test --test v2_mrtr_ingress --features full` | **10 passed** |
| `cargo test --test v2_stateless_http --features full` | 15 passed |
| `cargo test --test v2_required_headers --features full` | 25 passed |
| `cargo test --test v2_client --features full` | 10 passed |
| `cargo test --lib --features full` | 1411 passed |
| `cargo build --lib --target wasm32-unknown-unknown` | OK |
| `cargo build --lib --no-default-features` | OK (3 warnings, all pre-existing) |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 pass, 30 skip / no semver update required` |
| `/usr/bin/make quality-gate` (UNPROXIED) | **ALL TOYOTA WAY QUALITY CHECKS PASSED** |

The gate was run unproxied via `/usr/bin/make` with cargo at
`/Users/guy/.cargo/bin/cargo` (plan 03 proved the `rtk` shell proxy truncates the
clippy stage and reports exit 0 for a run that actually failed), and
`git status --porcelain -- src/ tests/` was empty afterwards, so the green gate is
of the committed tree.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The plan's `pub` MRTR field/builder cannot compile**

- **Found during:** Task 1
- **Issue:** The plan specifies `pub mrtr: Option<crate::types::mrtr::MrtrRequestParams>` and `#[must_use] pub fn with_mrtr_params(...)`. `MrtrRequestParams` is `pub(crate)` (D-10 keeps the MRTR plumbing off the public API), so a `pub` field/parameter of that type trips the `private_interfaces` lint, which is denied under the gate's `-D warnings`. Making `MrtrRequestParams` public instead would add a constructible `pub` struct to the API — exactly the shape D-113-D just measured as a future major-bump hazard.
- **Fix:** The field and the three write-side builders are `pub(crate)`. Handlers reach the values through the three genuinely `pub` `RequestHandlerExtra` accessors the plan also specifies, which is where the plan wanted the handler-facing surface anyway. `grep 'fn with_mrtr_params'` still matches; only the visibility keyword differs.
- **Files modified:** `src/types/protocol/context.rs`
- **Verification:** `protocol_context_with_mrtr_params_round_trips`, `cargo semver-checks` 223/223
- **Committed in:** `fcd047bb`

**2. [Rule 2 - Missing Critical] `Reelicit` is unobservable without an egress half**

- **Found during:** Task 2
- **Issue:** Two of the plan's own `must_haves.truths` and four of Task 3's acceptance criteria require the re-elicitation to produce "a fresh `input_required` with real `inputRequests`" and a token whose round survived. Plan 09 owns the egress, so as written nothing in plan 06 could convert a handler's signal — `unknown_key_reelicits_with_input_requests` and `expired_state_reelicits_preserving_round` were unreachable.
- **Fix:** Added a minimal `core::mrtr_egress` (+ `take_mrtr_signal`, `seal_input_required`): it strips the pmcp-internal `dev.pmcp/mrtr` key **unconditionally on every path including v1** (leaking the plaintext continuation would defeat the AEAD), and on an eligible v2 method mints a fresh `requestState` at `round + 1` and writes the three SERVER-OWNED fields (`resultType`, `inputRequests`, `requestState`) by overwrite, never `entry().or_insert`. It fails closed with `INTERNAL_ERROR` rather than emitting a bogus complete result. **Plan 09 still owns the hardening** — declared-capability precheck before minting, the exhaustive eligible-method tripwire, `ReadResourceResult._meta`, and the `serverInfo` relocation.
- **Files modified:** `src/server/core.rs`, `src/server/mod.rs`
- **Verification:** `egress_emits_input_required_with_a_round_plus_one_token`, `egress_strips_the_internal_signal_on_every_path`, `egress_fails_closed_when_it_cannot_mint`, `egress_is_a_noop_without_a_signal`, plus the two live re-elicit tests
- **Committed in:** `de06a1d1`

**3. [Rule 3 - Blocking] `clippy::large_futures` on the dispatch site**

- **Found during:** Task 2 (`make lint`, not plain `cargo clippy`)
- **Issue:** The MRTR locals pushed `Server::handle_request_with_context`'s future from just under to just over clippy's 16 KB `large_futures` threshold, which cascaded into 7 errors at its callers (`batch.rs`, both POST entrypoints, `mod.rs`).
- **Fix:** `Box::pin` the inner `handle_client_request` future — the same treatment the two POST entrypoints and the discover assembly already carry, with the rationale in a comment. All 7 errors cleared at once.
- **Files modified:** `src/server/mod.rs`
- **Verification:** `make lint` clean
- **Committed in:** `de06a1d1`

**4. [Rule 3 - Blocking] `RequestStateClock` / `FixedClock` are unreachable from an integration test**

- **Found during:** Task 3
- **Issue:** The plan's `expired_state_reelicits_preserving_round` says to use `with_request_state_ttl(Duration::from_secs(1))` and "the deterministic codec with a `FixedClock` set in the past". `RequestStateClock` is `pub(crate)`, so an integration test cannot build one; and a 1-second server TTL would risk the server's OWN freshly minted reply token expiring before the test could open it.
- **Fix:** `pmcp::testing::mint_request_state` takes the TTL, and `Duration::ZERO` seals `exp == now` — already expired on the next `verify`. Deterministic, instant, no sleeping, and the server keeps its default 300-second TTL so the reply token is comfortably live.
- **Files modified:** `src/testing/mod.rs`, `tests/v2_mrtr_ingress.rs`
- **Verification:** `expired_state_reelicits_preserving_round` asserts `round == 4` after minting at round 3
- **Committed in:** `52d8b8dc`

### Intentional simplifications (recorded, not auto-fixes)

- **`mrtr_ingest` is called once per dispatch site, not once per eligible arm.** The plan says "call `mrtr_ingest` from the `CallTool`, `GetPrompt` and `ReadResource` arms". The helper already dispatches on the request variant through `mrtr_binding_parts`, so three call sites per file would have been three copies of one decision. One call before dispatch is equivalent, and it is also the only place the resolved verdict can be folded into the context *before* it is threaded downward.
- **`principal_mismatch_errors` uses one server, not two.** The plan suggests "two servers with different fixed-subject auth providers". Since minting is done in-test with the shared key, the principal is already a free parameter: the test mints for `alice` and posts to a server that authenticates every credentialed caller as `bob`. Same proof, less machinery.
- **`splice_mrtr_params` lives in `mrtr_binding_parts`, not literally inside the `Reelicit` arm.** The typed dispatch request has no MRTR fields to strip (D-113-D deliberately kept them off the structs), so the meaningful strip is the context-level `without_mrtr` the `Reelicit` arm calls. `splice_mrtr_params` with the default is applied to the reconstructed live params instead, as belt-and-braces so a future widening of the salient whitelist can never admit a client-echoed MRTR field into the AAD.

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 missing-critical) + 3 recorded simplifications
**Impact on plan:** No stated behavior was dropped. Every `<behavior>` bullet and every `<acceptance_criteria>` line is satisfied; the deviations were required to make them reachable under the repo's own lint gate and visibility rules.

## Threat Flags

None. Every file touched is inside the plan's declared threat surface, and no new
network endpoint, auth path, file access pattern or schema change at a trust
boundary was introduced. The `pmcp::testing` mint/open wrappers are behind the
`testing` feature (in `full`, not in `default`) and expose no key material.

## Known Stubs

None. `mrtr_egress` is deliberately MINIMAL rather than stubbed — it is fully
functional for the paths plan 06 must prove, and plan 09 extends it. It is
recorded above as a scope hand-off, not as unwired placeholder code.

## Issues Encountered

- **`.pmat/*` and `pmcp-course/*` show as modified** in the working tree. They pre-date this plan and were deliberately NOT staged, per the executor scope boundary. `.planning/config.json` and `.planning/tmp/` were likewise left alone.
- **An earlier run of this plan was interrupted by a transport-level API error** mid-Task-2. Nothing was lost: Task 1 was already committed, and the in-flight `cancellation.rs` accessors were re-verified and committed as `a3d5fdd2` before work resumed.

## TDD Gate Compliance

Tasks 1 and 2 are `tdd="true"`. Implementation and tests were committed together
per task, so there is no separate `test(...)` commit preceding each `feat(...)`
commit. RED was verified by construction rather than by commit ordering: each
verdict test asserts a behavior that did not exist before its commit (there was
no `mrtr` field, no `mrtr_ingest`, and no MRTR extraction at the gate), and the
malformed-input tests were written against the eight `MrtrParseError` variants
plan 02 had already shipped but nothing consumed. Task 3 carries the `test(...)`
commit.

## Next Phase Readiness

- **Plan 09 (egress hardening)** inherits a working `mrtr_egress` and should extend rather than replace it. Still outstanding and explicitly ITS scope: the declared-client-capability precheck **before** minting (`-32021` with an object-shaped `requiredCapabilities`), the exhaustive compile-time eligible-method tripwire, `ReadResourceResult._meta` so resource handlers can signal, and moving `serverInfo` into `result._meta["io.modelcontextprotocol/serverInfo"]`. `ResponseDisposition::InputRequired` is now constructed, so its `#[allow(dead_code)]` is gone (only `Task` retains one, for Phase 114).
- **Plan 07 (client retry loop)** has both halves of the wire contract proven server-side. It still owns D-113-E (the v2 client cannot read a structured JSON-RPC error off a 4xx response) — and this plan makes that more pressing, since a tampered/expired-token rejection now arrives as `-32602` at HTTP 400.
- **Plan 11 (conformance)** can point `sep-2322-reject-tampered-state` at `tampered_state_errors`, which already performs the exact `-TAMPERED` mutation and asserts the absence of `result`.
- **Plan 12 (public-API + semver audit)** should note the new public surface: `RequestHandlerExtra::{input_responses, mrtr_continuation, mrtr_round}` and `pmcp::testing::{mint_request_state, open_request_state, ANONYMOUS_PRINCIPAL, MRTR_SIGNAL_META_KEY}`. Current measurement is unchanged at `223 checks: 223 pass, 30 skip`.
- **HTTP-02 and HTTP-03 are NOT marked complete** — per the 113-01 recorded exception, plan 12 owns the binding re-verification of the whole phase.

## Self-Check: PASSED

- `src/types/protocol/context.rs`, `src/server/streamable_http_server.rs`, `src/server/cancellation.rs`, `src/server/core.rs`, `src/server/mod.rs`, `src/testing/mod.rs`, `tests/v2_mrtr_ingress.rs` and this SUMMARY all exist on disk.
- All four claimed commits (`fcd047bb`, `a3d5fdd2`, `de06a1d1`, `52d8b8dc`) resolve in `git log`.
- Contract greps: `fn mrtr_ingest` in `core.rs` = 1, in `mod.rs` = 0; `request_state::codec()` in `core.rs` = 0; `PMCP_REQUEST_STATE_KEY` in `tests/v2_mrtr_ingress.rs` = 0; `MrtrParseError` in `streamable_http_server.rs` ≥ 1; `tests/v2_mrtr_ingress.rs` = 675 lines (min 200).

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
