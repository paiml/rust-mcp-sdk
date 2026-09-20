---
phase: 70-add-extensions-typemap-and-peer-back-channel-to-requesthandl
verified: 2026-04-16T00:00:00Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: n/a
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra — Verification Report

**Phase Goal:** Extend `RequestHandlerExtra` with two drop-in additive capabilities — a typed-key `Extensions` map (HANDLER-02) and an optional `PeerHandle` back-channel (HANDLER-05) exposing `sample` / `list_roots` / `progress_notify` from inside tool/prompt/resource handlers — without breaking any existing `::new(...)` or `::with_session(...)` call site.

**Verified:** 2026-04-16
**Status:** VERIFICATION PASSED
**Re-verification:** No — initial verification
**Requirements:** PARITY-HANDLER-01

## Goal Achievement

### Observable Truths / Must-Haves Matrix

| # | Must-Have | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Extensions typemap exists and works (accessors on BOTH structs) | VERIFIED | `src/server/cancellation.rs:219` (`pub extensions: http::Extensions`), `:303`/`:315` (accessors); `src/shared/cancellation.rs:76` (field), `:127`/`:132` (accessors) + `:142` inline parity test |
| 2 | PeerHandle back-channel + DispatchPeerHandle + peer() accessor | VERIFIED | `src/shared/peer.rs:52` (`pub trait PeerHandle`), `src/server/peer_impl.rs:37` (`pub struct DispatchPeerHandle`), `:55`/`:68`/`:78` (sample/list_roots/progress_notify impls); `src/server/cancellation.rs:341` (`with_peer`), `:351` (`peer()`); `src/lib.rs:96` re-export |
| 3 | ServerRequestDispatcher foundation (dispatch + handle_response + drain + Server::run routing) | VERIFIED | `src/server/server_request_dispatcher.rs:69` (struct), `:110` (dispatch), `:166` (handle_response), `:207` (spawn_server_request_drain); `src/server/mod.rs:733`-`:743` (wired into Server::run), `:819`-`:831` (TransportMessage::Response routed through dispatcher) |
| 4 | Backwards-compat + `#[non_exhaustive]` marker present + documented | VERIFIED | `cargo check --features "full"` passes in 4.25s; `src/server/cancellation.rs:178` (`#[non_exhaustive]`); `:17` `# Semver posture` section documents breaking-change posture; `::new()` + `::with_session()` compile unchanged |
| 5 | 12-site struct-literal refactor complete | VERIFIED | `grep -c "RequestHandlerExtra {" src/server/workflow/prompt_handler.rs` returns 0; no struct-literal call sites remain under `src/` or `tests/` (only type definitions at `src/server/cancellation.rs:179` and `src/shared/cancellation.rs:53`) |
| 6 | ALWAYS coverage: property + unit + integration + fuzz + examples | VERIFIED | Property: `tests/handler_extensions_properties.rs` has 5 `#[test]` proptests at 100 cases each (lines 14, 25, 34, 44, 54); Integration: `tests/server_request_dispatcher_integration.rs` + `tests/handler_peer_integration.rs` present; Fuzz: `fuzz/fuzz_targets/fuzz_peer_handle.rs` exists + compiles cleanly; Examples: `examples/s42_handler_extensions.rs` + `examples/s43_handler_peer_sample.rs` — both implement `ToolHandler`; 9 integration+property tests PASS |
| 7 | Working example demonstrates peer.sample() round-trip from INSIDE ToolHandler (Codex Finding 5) | VERIFIED | `examples/s43_handler_peer_sample.rs:70` (`impl ToolHandler for PeerSamplingTool`), `:80` (`extra.peer()`), `:84` (`peer.sample(params).await?`) — inside `handle()` body, not main(); `cargo run --example s43_handler_peer_sample` exits 0, output: `"user_id=42, peer sampled via model: mock-model"` |
| 8 | WASM build still works | VERIFIED | `cargo check --target wasm32-unknown-unknown --features schema-generation` succeeds; 32 pre-existing missing_docs warnings in runtime.rs/adapters are NOT Phase-70-introduced |
| 9 | Quality gate green (`make quality-gate` exit 0, zero clippy, zero fmt, zero rustdoc warnings) | VERIFIED | `make quality-gate` exit code: **0**; `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features full` succeeds; clippy output: "No lint issues" |
| 10 | Migration prose explicit — `# Semver posture` + `# Known limitation: session-id plumbing` | VERIFIED | `src/server/cancellation.rs:17` (`//! # Semver posture`), `:29` (`//! # Known limitation: session-id plumbing`) |

**Score:** 10/10 must-haves verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/cancellation.rs` | Extensions + peer field + `#[non_exhaustive]` + migration prose | VERIFIED | 24.7K, all four markers present |
| `src/shared/cancellation.rs` | Extensions field + accessors (WASM-safe) + parity test | VERIFIED | 145 lines incl. inline `#[cfg(test)] mod tests` |
| `src/shared/peer.rs` | `pub trait PeerHandle` with sample/list_roots/progress_notify | VERIFIED | Line 52 |
| `src/server/peer_impl.rs` | `DispatchPeerHandle` wrapper delegating to dispatcher | VERIFIED | 5.8K, impls at lines 55/68/78 |
| `src/server/server_request_dispatcher.rs` | Dispatcher struct + dispatch + handle_response + drain | VERIFIED | All 4 surfaces present |
| `tests/handler_extensions_properties.rs` | ≥5 proptests @ ≥100 cases | VERIFIED | 5 tests, `cases: 100` |
| `tests/server_request_dispatcher_integration.rs` | Integration test for dispatcher | VERIFIED | 4.6K |
| `tests/handler_peer_integration.rs` | Integration test for peer round-trip | VERIFIED | 3.9K |
| `fuzz/fuzz_targets/fuzz_peer_handle.rs` | Fuzz target compiles | VERIFIED | Compiles standalone |
| `examples/s42_handler_extensions.rs` | Real ToolHandler demonstrating extensions | VERIFIED | Line 26 has `impl ToolHandler` |
| `examples/s43_handler_peer_sample.rs` | Real ToolHandler demonstrating peer.sample() | VERIFIED | Line 70 has `impl ToolHandler`, line 84 calls `peer.sample()` inside `handle()` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `Server::run` | `ServerRequestDispatcher` | `handle_response` on `TransportMessage::Response` | WIRED | `src/server/mod.rs:819-831` — response payload routed through `dispatcher.handle_response(&correlation_id, payload)` |
| `ServerCore` | `DispatchPeerHandle` | `with_peer` on extra | WIRED | `src/server/core.rs:457-461, 629-633, 658-662, 710-714` (4 sites) |
| Server.rs dispatch paths | `DispatchPeerHandle` | `with_peer` on extra | WIRED | `src/server/mod.rs:1114-1118, 1265, 1308, 1391, 1455` (5 sites) — total 9 dispatch sites match plan |
| `ServerCore::with_server_request_dispatcher` | `ServerRequestDispatcher` | Arc hand-off | WIRED | `src/server/core.rs:366-370` |
| `examples/s43` handler body | `PeerHandle::sample` | `extra.peer().sample(params).await` | WIRED + RUNS | Runtime output confirms: `"peer sampled via model: mock-model"` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `s43_handler_peer_sample` | `sample_result` in handle() | `peer.sample(CreateMessageParams::new(...))` → `MockPeer::sample` → `CreateMessageResult::new(Content::text, "mock-model")` | Yes — real constructor output flows through handler return | FLOWING |
| `s42_handler_extensions` | `retrieved` in handle() | `extra.extensions().get::<RequestContext>()` after middleware `insert` | Yes — runtime output shows `"user_id=42, source=example"` | FLOWING |
| `ServerRequestDispatcher` correlation table | pending map entries | `dispatch()` inserts → `handle_response()` resolves oneshot | Yes — integration test `server_request_dispatcher_integration` exercises concurrent multiplex round-trip | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| s42 example runs | `cargo run --example s42_handler_extensions` | Exit 0; stdout: `cross-middleware value retrieved: user_id=42, source=example` | PASS |
| s43 example runs + demonstrates peer.sample round-trip | `cargo run --example s43_handler_peer_sample` | Exit 0; stdout: `user_id=42, peer sampled via model: mock-model` | PASS |
| Workspace compiles with `full` features | `cargo check --features full` | Finished in 4.25s, 0 errors | PASS |
| WASM target compiles | `cargo check --target wasm32-unknown-unknown --features schema-generation` | Succeeds (pre-existing runtime.rs doc warnings only) | PASS |
| Lib tests pass | `cargo test --lib --features full -- --test-threads=1` | 1022 passed (matches Plan 03 floor exactly) | PASS |
| Phase 70 integration + property tests pass | `cargo test --test handler_extensions_properties --test server_request_dispatcher_integration --test handler_peer_integration --features full -- --test-threads=1` | 9 passed (3 suites) | PASS |
| Fuzz target builds | `cd fuzz && cargo check --bin fuzz_peer_handle` | Finished, 0 errors | PASS |
| Rustdoc zero warnings | `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features full` | Succeeds | PASS |
| Quality gate | `make quality-gate` | Exit code: **0** — "No lint issues" | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PARITY-HANDLER-01 | 70-01…70-04 | Enrich `RequestHandlerExtra` with typed-key extensions map and optional peer back-channel | SATISFIED | All 5 Proposal-1 Success Criteria met (see must-haves 1, 2, 3, 4, 5); proven by runtime-executing s43 peer.sample() round-trip |

### Anti-Patterns Found

None. Specifically verified:
- Zero `CreateMessageParams::default()` / `CreateMessageResult::default()` anti-patterns in `src/`, `tests/`, `examples/` (addresses Codex Finding 4 — real constructors only)
- Zero TODO/FIXME introduced in Phase 70 modules
- Zero placeholder return statements in new code
- s43 example does NOT call `peer.sample()` from `main()` — confirmed called from inside `ToolHandler::handle()` (Codex Finding 5)

### Regression Check (Prior Phases)

| Check | Baseline | Current | Verdict |
|-------|----------|---------|---------|
| Lib test count | 1022 passed (Phase 69 Plan 03 floor) | 1022 passed | NO REGRESSION — matches floor exactly |
| `src/server/elicitation.rs` last modified | `ef3e2dcc` (v2.0.0, well before Phase 70) | Still `ef3e2dcc` | BYTE-FOR-BYTE UNCHANGED as required by Plan 02 scope boundary |
| `make quality-gate` | Green on Phase 69 close | Green (exit 0) | NO REGRESSION |

### Known Deferred Items (Out of Scope — NOT failures)

From `deferred-items.md`:

| Item | Reason | Addressed In |
|------|--------|--------------|
| `fuzz/fuzz_targets/fuzz_token_code_mode.rs` pre-existing E0599 compile errors on `pmcp-code-mode` API | Unrelated to Phase 70 extensions/peer surface; surfaced incidentally during Plan 04 Task 2 verification; Phase 70's own `fuzz_peer_handle` target compiles cleanly in isolation via `cargo check --bin fuzz_peer_handle` | Future cleanup phase |

This is an incidental unrelated defect discovered (but not introduced) during Phase 70 execution. It is logged for cleanup and does not count as a Phase 70 gap.

### Human Verification Required

None. All must-haves verifiable programmatically via grep/compile/run/test.

## Gaps Summary

No gaps. All 10 must-haves VERIFIED, all 5 Success Criteria from Proposal 1 (69-PROPOSALS.md) met:

1. ✅ Typed-key extensions map — `http::Extensions` field + accessors on both cancellation structs
2. ✅ Optional peer back-channel — `PeerHandle` trait + `DispatchPeerHandle` + `peer()` accessor
3. ✅ sample / list_roots / progress_notify reachable from inside ToolHandler — proven by s43 runtime output
4. ✅ `::new(...)` / `::with_session(...)` call sites unchanged — full-features build + tests pass
5. ✅ 12-site struct-literal refactor (`#[non_exhaustive]` + semver docs) — 0 struct-literal call sites in `src/server/workflow/prompt_handler.rs`

## Recommendation

**Proceed to next phase.** Phase 70 is complete, quality-gated, and satisfies PARITY-HANDLER-01 end-to-end:

- Roadmap Plan 01–04 checkboxes can be ticked
- PARITY-HANDLER-01 in REQUIREMENTS.md (line 54) can flip from pending → complete; status table at line 143 likewise
- v2.2 changelog entry should call out the `#[non_exhaustive]` semver posture (explicit breaking change for downstream struct-literal users)

---

_Verified: 2026-04-16_
_Verifier: Claude (gsd-verifier)_
_Branch: feat/sql-code-mode; HEAD: 2d900011 (docs(70-04): complete polish + fuzz + migration prose plan — Phase 70 done)_
