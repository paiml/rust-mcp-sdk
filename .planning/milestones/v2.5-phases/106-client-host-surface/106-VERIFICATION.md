---
phase: 106-client-host-surface
verified: 2026-07-18T00:01:51Z
status: passed
score: 16/16 must-haves verified
overrides_applied: 0
---

# Phase 106: Client Host Surface Verification Report

**Phase Goal:** A pmcp `Client` can answer server→client requests (spec-direction `sampling/createMessage` incl. tools/tool_choice, `elicitation/create`, `roots/list`) through a client-side handler registry with a human-in-the-loop approval hook, and the legacy inverted sampling path is documented as the distinct "LLM-server pattern" — all additive (pmcp minor bump). Small, independently shippable, and unblocks Phase 108's `SamplingSource`.
**Verified:** 2026-07-18T00:01:51Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria + plan-level detail, merged)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A registered `HostSamplingHandler` answers a server's inbound `sampling/createMessage` (incl. tools/tool_choice) instead of erroring "Unexpected message type" (HOST-01) | VERIFIED | `src/client/mod.rs` `dispatch_host_sampling`/`dispatch_host_request` replaces the old error arm (grep confirms `Unexpected message type` string count = 0 in dispatch path); `tests/client_host_roundtrip.rs::sampling_answered_by_host_handler` passes (`cargo test --test client_host_roundtrip` → 5/5 incl. `prop_sampling_passthrough` for tools/tool_choice) |
| 2 | A registered `HostElicitationHandler` answers inbound `elicitation/create`; a registered roots provider answers `roots/list` (HOST-02, HOST-03) | VERIFIED | `src/client/host/elicitation.rs`, `src/client/host/roots.rs`; `tests/client_host_roundtrip.rs::elicitation_answered_via_raw_pump` and `::roots_answered_by_host_provider` pass |
| 3 | The sampling path invokes an async human-in-the-loop approval callback (default allow) before returning a completion (HOST-04) | VERIFIED | `dispatch_host_sampling` (`src/client/mod.rs:2416-2454`) runs `host_registry.approval` BEFORE `handler.handle_create_message`; unit tests `test_sampling_no_preflight_runs_handler`, `test_sampling_preflight_allow_runs_handler`, `test_sampling_preflight_deny_skips_handler` (handler proven NOT invoked on Deny), `test_sampling_result_review_deny_after_handler`, `test_sampling_result_review_absent_is_passthrough` all pass (`cargo test --lib client` → 99/99); duplex proof `tests/client_host_approval.rs::sampling_preflight_deny_survives_connection` passes |
| 4 | `ClientCapabilities` advertised on initialize reflect which host handlers are registered — sampling/elicitation/roots (HOST-05) | VERIFIED | `derive_host_capabilities` (`src/client/mod.rs:385-420`) called from `initialize`; unit tests `test_capability_sampling_registered_is_present`, `test_capability_sampling_unregistered_default_is_absent`, `test_capability_sampling_anti_lie_discards_caller_value`, `test_capability_sampling_preserves_caller_detail`, `test_capability_elicitation_and_roots_parallel`, `test_capability_derivation_leaves_tasks_and_experimental_untouched` all pass |
| 5 | The legacy `Client::create_message` → server `SamplingHandler` path is documented as the "LLM-server pattern", disambiguated from spec sampling in rustdoc and book, with zero breaking changes (HOST-06) | VERIFIED | `src/client/mod.rs:1864-1878` rustdoc names "LLM-server pattern", links `pmcp::SamplingHandler` + `client::host::HostSamplingHandler`; `pmcp-book/src/ch17-04-sampling-hosting.md` contrast table + direction diagram; grep confirms zero occurrences of `server::traits` in either file; `create_message` signature/behavior unchanged (CR-01 fixed a pre-existing dead-branch bug, not a new breaking change — see Anti-Patterns note below) |
| 6 | Inbound `sampling/createMessage` parsed as `Request::Client(CreateMessage)` (parse ambiguity) still reaches the host handler | VERIFIED | `classify_host_request` maps both `Request::Client(ClientRequest::CreateMessage)` and `Request::Server(ServerRequest::CreateMessage)` to `HostRequestKind::Sampling`; unit tests `classify_sampling_client_alias_variant`, `classify_sampling_server_variant`, `test_dispatch_sampling_alias_reaches_handler` pass |
| 7 | A KNOWN host request type with no registered handler returns `-32601`, the receive loop continues, and a subsequent normal client call still completes | VERIFIED | `host_method_not_found` helper; `tests/client_host_approval.rs::unhandled_known_method_returns_method_not_found_and_survives` passes (asserts a second `tools/list` succeeds after the `-32601`) |
| 8 | The `client::host` module compiles cfg-agnostically on `wasm32-unknown-unknown` (roots types relocated target-agnostically) | VERIFIED (static inspection) | `src/types/roots.rs` created, `pub mod roots;` added ungated in `src/types/mod.rs`; every `use` in `src/client/host/{mod,sampling,elicitation,roots}.rs` resolves to target-agnostic items (`std::sync::Arc`, `futures::future::BoxFuture`, `async_trait`, `serde`, `crate::types::{roots,sampling,elicitation}`, `crate::error`) — none wasm-gated by grep. A live `cargo check --target wasm32-unknown-unknown` was intentionally skipped per verification-run instructions to avoid a heavy concurrent build while `make quality-gate` was running in another process; this is a live wasm target (`rustup target list --installed` shows it installed) and the plan's own acceptance criteria permit "record why it could not [run] and that the module is cfg-agnostic by inspection" as a fallback. |
| 9 | Back-compat: `pmcp::server::roots::{Root, ListRootsResult}` still resolves | VERIFIED | `src/server/roots.rs` contains `pub use crate::types::roots::{ListRootsResult, Root};`; existing `RootsManager` tests in that file are unaffected and still present |
| 10 | A denial (preflight or result-review) returns a JSON-RPC error with a generic message; the raw deny reason is logged locally only, never forwarded; the connection is not torn down | VERIFIED | `host_policy_denied` returns the fixed string `"request denied by host policy"`; `tracing::warn!(%reason, ...)` logs locally; `tests/client_host_approval.rs::sampling_preflight_deny_survives_connection` asserts the raw reason string is absent from the wire response and a subsequent request completes |
| 11 | Handler/provider errors are sanitized to `-32603`, full error logged locally only | VERIFIED | `host_handler_error`/`host_internal_error` helpers; `tracing::error!` call; unit test `test_dispatch_handler_error_is_sanitized_32603` passes |
| 12 | The routing fuzz target drives the real `parse_request → classify_host_request` path (not standalone serde) | VERIFIED | `fuzz/fuzz_targets/client_host_routing.rs` calls `pmcp::shared::parse_request` then `classify_host_request`; `cd fuzz && cargo build --bin client_host_routing` succeeds (confirmed live in this verification run) |
| 13 | pmcp version bumped 2.15.0 → 2.16.0; cargo-pmcp scaffold `PMCP_VERSION` pin bumped in lockstep; drift-guard test logic is satisfied | VERIFIED | `Cargo.toml` `version = "2.16.0"`; `cargo-pmcp/src/templates/workbook_server.rs` `PMCP_VERSION: &str = "2.16.0"`; `cargo-pmcp/Cargo.toml` bumped to `0.17.4`; the drift-guard test (`emitted_pmcp_version_matches_workspace_pin`) reads the root `Cargo.toml` version dynamically and asserts equality with `PMCP_VERSION` — both are `2.16.0`, so it is satisfied by construction |
| 14 | A runnable, registered example demonstrates the nested sampling-host flow | VERIFIED | `examples/s49_sampling_host.rs` exists; `Cargo.toml` `[[example]] name = "s49_sampling_host"`; `examples/README.md` documents it; `cargo run --example s49_sampling_host` exits 0 and prints a round-trip completion (confirmed live in this verification run) |
| 15 | The pmcp-book has a Sampling & Hosting page reachable from the TOC that unambiguously contrasts the two directions using real public trait paths | VERIFIED | `pmcp-book/src/ch17-04-sampling-hosting.md` exists with intro, "Spec host sampling" section, "LLM-server pattern" section, 4-column contrast table, `s49_sampling_host` pointer; `pmcp-book/src/SUMMARY.md:57` links it under Chapter 17, immediately after ch17-03, no reordering |
| 16 | Post-plan code-review fixes (CR-01, WR-01, WR-02, WR-03, WR-04) are actually applied in the tree, not just claimed in 106-REVIEW.md | VERIFIED | All 5 fix commits present in `git log` (`96987bbf`, `ee3415d5`, `606bbcf3`, `9cea2f7a`, `5c95fecb`) plus a follow-up `345f4402`; each fix's code/tests independently re-verified in this pass: CR-01 (`assert_capability` sampling arm + `tests/client_create_message_llm_server.rs` passing live), WR-01 (`HostRequestKind::Ping` + `inbound_ping_answered_with_empty_result` passing live), WR-02 (sampling.rs docs now say "optional"/"invoked ... as of this phase", grep confirms no remaining "Mandatory" claim outside a benign ordering-only usage), WR-03 (rustdoc + CHANGELOG.md 2.16.0 entry + `send_roots_list_changed` doctest rewritten and passing live), WR-04 (`active_requests` cleanup on host-response send failure + `test_host_response_send_failure_cleans_active_requests` passing live) |

**Score:** 16/16 truths verified (0 overrides)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/types/roots.rs` | target-agnostic `Root` + `ListRootsResult` | VERIFIED | Present, no wasm gating, re-exported from `server::roots` |
| `src/client/host/mod.rs` | `ClientHostRegistry` + `classify_host_request`/`HostRequestKind` | VERIFIED | Present, matches spec, unit tests pass |
| `src/client/host/sampling.rs` | `HostSamplingHandler`, `ApprovalDecision`, `PreflightApproval`, `SamplingResultReview` | VERIFIED | Present, owned-params signatures, docs corrected post-review |
| `src/client/host/elicitation.rs` | `HostElicitationHandler` | VERIFIED | Present |
| `src/client/host/roots.rs` | `RootsProvider` (Result-returning) | VERIFIED | Present |
| `src/client/mod.rs` | `host_registry` on every ctor + Clone, builder methods, `dispatch_host_request`/`dispatch_host_sampling`, `create_message` rustdoc, `assert_capability` sampling arm, ping handling, `derive_host_capabilities` | VERIFIED | All present and exercised by 99 passing lib unit tests |
| `tests/client_host_roundtrip.rs` | sampling/roots/elicitation/ping round-trips + passthrough proptest | VERIFIED | 5/5 tests pass live |
| `tests/client_host_approval.rs` | duplex denial + unhandled-method survival | VERIFIED | 2/2 tests pass live |
| `tests/client_create_message_llm_server.rs` | CR-01 end-to-end regression | VERIFIED | 1/1 test passes live |
| `examples/s49_sampling_host.rs` | runnable sampling-host example | VERIFIED | Registered in Cargo.toml + README; runs and exits 0 live |
| `fuzz/fuzz_targets/client_host_routing.rs` | routing fuzz over real dispatch path | VERIFIED | Registered in `fuzz/Cargo.toml`; builds live |
| `Cargo.toml` | pmcp `2.16.0` + s49 example registration | VERIFIED | Both present |
| `cargo-pmcp/src/templates/workbook_server.rs` | `PMCP_VERSION = "2.16.0"` | VERIFIED | Present |
| `pmcp-book/src/ch17-04-sampling-hosting.md` | HOST-06 disambiguation page | VERIFIED | Present, content verified |
| `pmcp-book/src/SUMMARY.md` | TOC entry | VERIFIED | Present |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `send_request` receive loop | `dispatch_host_request` | replaced `TransportMessage::Request` error arm | WIRED | `src/client/mod.rs:2339-2360`; "Unexpected message type" string no longer present in this arm |
| `dispatch_host_request` | `classify_host_request` + per-kind helpers | pure classify then route | WIRED | `src/client/mod.rs:2373-2391` |
| `dispatch_host_sampling` | `host_registry.approval` (preflight, before handler) | deny prevents LLM call | WIRED | `src/client/mod.rs:2431-2436` runs before line 2439 handler call; unit test proves handler not invoked on Deny |
| `initialize` | `derive_host_capabilities` | called before store+send | WIRED | `src/client/mod.rs:334` |
| `fuzz/fuzz_targets/client_host_routing.rs` | `classify_host_request` | `parse_request` then `classify` | WIRED | confirmed by source read + live build |
| `src/server/roots.rs` | `src/types/roots.rs` | back-compat re-export | WIRED | `pub use crate::types::roots::{ListRootsResult, Root};` |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Host round-trips (sampling/roots/elicitation/ping) + passthrough proptest | `cargo test --test client_host_roundtrip` | 5 passed | PASS |
| Preflight-deny + unhandled-method duplex survival | `cargo test --test client_host_approval` | 2 passed | PASS |
| CR-01 LLM-server-pattern regression | `cargo test --test client_create_message_llm_server` | 1 passed | PASS |
| Client-side unit test suite (host dispatch, capabilities, approval) | `cargo test --lib client` | 99 passed | PASS |
| `send_roots_list_changed` doctest (WR-03 rewrite) | `cargo test --doc send_roots_list_changed` | 1 passed | PASS |
| Routing fuzz target builds against real dispatch path | `cd fuzz && cargo build --bin client_host_routing` | build succeeded | PASS |
| s49 example runs end-to-end | `cargo run --example s49_sampling_host` | exit 0, printed completion | PASS |
| wasm32 lib check | `cargo check --lib --target wasm32-unknown-unknown` | SKIPPED (instructed to avoid heavy concurrent build; verified by static inspection instead) | SKIP |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| HOST-01 | 106-01 | Client answers `sampling/createMessage` incl. tools/tool_choice | SATISFIED | Truth #1, #6 |
| HOST-02 | 106-01 | Client answers `elicitation/create` | SATISFIED | Truth #2 |
| HOST-03 | 106-01 | Client answers `roots/list` | SATISFIED | Truth #2 |
| HOST-04 | 106-02 | Human-in-the-loop approval hook, default allow | SATISFIED | Truth #3, #10 |
| HOST-05 | 106-02 | Capabilities reflect registered handlers | SATISFIED | Truth #4 |
| HOST-06 | 106-01, 106-03 | LLM-server pattern documented, disambiguated, no breaking changes | SATISFIED | Truth #5 |

All 6 requirement IDs declared across the three plans (`106-01-PLAN.md`: HOST-01/02/03/06; `106-02-PLAN.md`: HOST-04/05; `106-03-PLAN.md`: HOST-06) are accounted for and match the `REQUIREMENTS.md` HOST section exactly (`HOST-01` through `HOST-06`, lines 11-16). No orphaned requirements — `REQUIREMENTS.md`'s traceability table (lines 82-87) maps all six to Phase 106 and none are missing from the plans.

**Note:** `REQUIREMENTS.md` checkboxes (lines 11-16) and the traceability table's "Status" column (lines 82-87) still read unchecked / "Pending". This is an administrative bookkeeping gap, not a code gap — this is the first phase of a freshly-defined milestone (v2.4, defined 2026-07-17) and the requirements file has not yet been updated to reflect Phase 106's completion. Recommend flipping these to checked/"Satisfied" as part of phase close-out, but it does not affect the codebase-level goal achievement this report verifies.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `pmcp-book/src/ch17-04-sampling-hosting.md` | 76-78 | Stale claim: "Nothing about this pattern's behavior changed in Phase 106" | Info | Written by plan 106-03 before the CR-01 review fix (`96987bbf`) landed. CR-01 fixed a pre-existing bug where `create_message` *always* failed (missing `"sampling"` match arm) — so the LLM-server pattern's behavior did change within this phase's commit range, from "always errors" to "works when the server advertises sampling". This is a bug fix, not a breaking change (previous behavior was unconditionally broken and unusable — no test or example exercised it), so it does not violate the HOST-06 "zero breaking changes" requirement. Recommend a one-line book update in the Phase 111 docs pass noting the CR-01 fix, but this does not block phase 106 goal achievement. |
| — | — | No TBD/FIXME/XXX/HACK/PLACEHOLDER/TODO markers found in any of the 13 files touched by this phase | — | Clean |

No blocker-tier anti-patterns found.

### Human Verification Required

None. All must-haves are verifiable by static inspection and automated test/build execution; no visual, real-time, or external-service-dependent behavior is introduced by this phase.

### Gaps Summary

No gaps. All 16 derived observable truths (5 ROADMAP Success Criteria plus plan-level granular truths) are VERIFIED against the actual codebase, not just SUMMARY.md claims:

- Live test runs confirm 99/99 lib unit tests, 5/5 `client_host_roundtrip` tests, 2/2 `client_host_approval` tests, and 1/1 `client_create_message_llm_server` test all pass on the current HEAD (commit `12f682ac`, which includes an additional in-flight clippy-nursery style fix beyond the 5 fix commits recorded in `106-REVIEW.md`'s Fix Outcomes table — reviewed and confirmed to be a mechanical, behavior-preserving change to a test mock).
- All 5 code-review fix commits (CR-01, WR-01, WR-02, WR-03, WR-04) are present in git history and independently re-verified in the source, not just trusted from the review report.
- The fuzz target and the s49 example were both built/run live in this verification pass, not merely trusted from SUMMARY claims.
- The one item skipped live (wasm32 `cargo check`) was skipped per explicit verification-run instructions to avoid a heavy concurrent build; static inspection of every import in the affected files shows no wasm-incompatible dependencies, consistent with the plan's own documented fallback.
- D-106-A (server-loop sampling deadlock, deferred to Phase 108) and D-106-B (unrelated `quick-xml` purity-check ambiguity, since verified passing) are correctly out of scope for this phase's must-haves — both were explicitly deferred with tracked owners in `deferred-items.md`, not silently dropped.
- The only note surfaced is an Info-tier documentation staleness (book claims "nothing changed" when a bug fix did change behavior from broken to working) — does not block phase completion.

---

*Verified: 2026-07-18T00:01:51Z*
*Verifier: Claude (gsd-verifier)*
