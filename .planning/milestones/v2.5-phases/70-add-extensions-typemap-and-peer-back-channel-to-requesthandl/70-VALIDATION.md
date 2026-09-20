---
phase: 70
slug: add-extensions-typemap-and-peer-back-channel-to-requesthandl
status: regenerated-for-4-plan-structure
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-16
regenerated: 2026-04-17
---

# Phase 70 — Validation Strategy (4-plan structure)

> Per-phase validation contract for feedback sampling during execution.
> Regenerated 2026-04-17 after reviews-driven replan (3 plans → 4 plans; Plan 02 now dispatcher foundation, Plan 03 peer wiring, Plan 04 examples/fuzz/docs).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `proptest 1.7` + `cargo-fuzz` (dev-deps at `Cargo.toml:131, 140`) |
| **Config file** | Default proptest config; `.cargo/config.toml` controls test threads |
| **Quick run command** | `cargo test --lib server:: -- --test-threads=1` |
| **Full suite command** | `cargo test --features "full" -- --test-threads=1` |
| **Estimated runtime** | Quick: ~5s · Full: ~2–3 min · Quality gate: ~5 min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib server:: -- --test-threads=1`
- **After every plan wave:** Run `cargo test --features "full" -- --test-threads=1`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green (matches CI exactly)
- **Max feedback latency:** 5s (per-task) / 180s (per-wave)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 70-01-01 | 01 | 1 | PARITY-HANDLER-01 | — | Extensions typemap field compiles into both `RequestHandlerExtra` structs with `#[non_exhaustive]` marker; `Default`/`Debug`/`Clone` preserved | compilation | `cargo check --features "full"` | ✅ existing | ⬜ pending |
| 70-01-02 | 01 | 1 | PARITY-HANDLER-01 | — | 12 positional struct-literal sites in `src/server/workflow/prompt_handler.rs` refactored to `::new()` builder form; `grep -c 'RequestHandlerExtra {' src/server/workflow/prompt_handler.rs` returns 0 | unit | `cargo test --lib server::workflow -- --test-threads=1` | ✅ existing | ⬜ pending |
| 70-01-03 | 01 | 1 | PARITY-HANDLER-01 | T-70-01 (Info Disclosure) | 5 proptests: insert/get roundtrip, key collision returns old, clone preserves, remove returns value, two types coexist | property | `cargo test --test handler_extensions_properties -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-01 | 02 | 2 | PARITY-HANDLER-01 | T-70-02 (Tampering — channel routing) | `ServerRequestDispatcher` created at `src/server/server_request_dispatcher.rs` with `outbound_tx` + `pending` HashMap + `dispatch()` + `handle_response()` + timeout | unit | `cargo test --lib server::server_request_dispatcher::tests -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-02 | 02 | 2 | PARITY-HANDLER-01 | T-70-02 | `spawn_server_request_drain` task wraps outbound items as `TransportMessage::Request(Request::Server(...))`; `Server::run` routes `TransportMessage::Response` through `dispatcher.handle_response` instead of dropping | integration | `cargo test --test server_request_dispatcher_integration test_single_request_response_roundtrip -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-03 | 02 | 2 | PARITY-HANDLER-01 | T-70-05 (DoS — timeout) | Concurrent multiplex: 3 requests in-flight, responses arrive out-of-order, all oneshots fulfilled correctly | integration | `cargo test --test server_request_dispatcher_integration test_concurrent_multiplex_out_of_order -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-03-01 | 03 | 3 | PARITY-HANDLER-01 | — | `PeerHandle` trait defined in `src/shared/peer.rs` (cross-transport); public re-export at `src/lib.rs` | unit | `cargo test --lib shared::peer::tests::test_peer_handle_trait_shape -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-03-02 | 03 | 3 | PARITY-HANDLER-01 | T-70-02 | `DispatchPeerHandle::new(Arc<ServerRequestDispatcher>)` delegates via `self.dispatcher.dispatch(ServerRequest::CreateMessage(...))`; `peer.progress_notify` always returns Ok (best-effort semantics) | unit | `cargo test --lib server::peer_impl::tests -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-03-03 | 03 | 3 | PARITY-HANDLER-01 | — | Conditional `.with_peer(...)` wiring at 9 ServerCore dispatch sites; WASM build compiles without peer field (cfg-gate honored); existing tests constructing ServerCore without dispatcher still pass (graceful fallback) | compilation+integration | `cargo check --features "full" && cargo check --target wasm32-unknown-unknown --features schema-generation && cargo test --test handler_peer_integration -- --test-threads=1` | ❌ W0 (integration test) / ✅ existing (compile) | ⬜ pending |
| 70-04-01 | 04 | 4 | PARITY-HANDLER-01 | — | `examples/s42_handler_extensions.rs` implements real `ToolHandler` invoking `extra.extensions()` from inside `handle(args, extra)`; compiles and runs in <5s | example | `cargo run --example s42_handler_extensions` | ❌ W0 | ⬜ pending |
| 70-04-02 | 04 | 4 | PARITY-HANDLER-01 | — | `examples/s43_handler_peer_sample.rs` implements real `ToolHandler` invoking `extra.peer().sample(CreateMessageParams::new(Vec::new()))` from inside `handle(args, extra)` with in-example MockPeer; uses REAL constructors (no `::default()` on sampling types); compiles and runs in <5s | example | `cargo run --example s43_handler_peer_sample` | ❌ W0 | ⬜ pending |
| 70-04-03 | 04 | 4 | PARITY-HANDLER-01 | T-70-04 (DoS — malformed input) | Fuzz target `fuzz_peer_handle.rs` exercises serialization/deserialization of `ServerRequest::CreateMessage` and correlation-id parsing; survives ≥100 iterations without panic | fuzz | `cargo +nightly fuzz run fuzz_peer_handle -- -max_total_time=30` | ❌ W0 | ⬜ pending |
| 70-04-04 | 04 | 4 | PARITY-HANDLER-01 | — | rustdoc passes with `-D warnings`; migration prose includes explicit `# Semver posture` + `# Known limitation: session-id plumbing` sections | doc | `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features full` | ✅ existing | ⬜ pending |
| 70-04-05 | 04 | 4 | PARITY-HANDLER-01 | — | Full CI gate green (fmt + clippy pedantic+nursery + build + test + audit) | gate | `make quality-gate` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/handler_extensions_properties.rs` — 5 proptests for Extensions semantics
- [ ] `src/server/server_request_dispatcher.rs` — new dispatcher module with inline `#[cfg(test)] mod tests`
- [ ] `tests/server_request_dispatcher_integration.rs` — round-trip + concurrent multiplex tests
- [ ] `src/shared/peer.rs` — `PeerHandle` trait definition
- [ ] `src/server/peer_impl.rs` — `DispatchPeerHandle` impl with inline unit tests
- [ ] `tests/handler_peer_integration.rs` — end-to-end peer tests
- [ ] `fuzz/fuzz_targets/fuzz_peer_handle.rs` — fuzz target for `ServerRequest::CreateMessage` serde
- [ ] `examples/s42_handler_extensions.rs` — ToolHandler demonstrating extensions
- [ ] `examples/s43_handler_peer_sample.rs` — ToolHandler with MockPeer
- [ ] No framework install needed — proptest + quickcheck + mockito + insta in dev-deps; `cargo-fuzz` installed via `cargo install cargo-fuzz`

---

## Manual-Only Verifications

*None — all phase behaviors have automated verification across 13 task-level gates.*

---

## Validation Sign-Off

- [ ] All 13 tasks have `<automated>` verify commands or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (longest gap: 1)
- [ ] Wave 0 covers all ❌ MISSING file references
- [ ] No watch-mode flags (`--watch`, `--loop`)
- [ ] Feedback latency <5s (per-task quick) / <180s (per-wave full)
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 files land

**Approval:** pending
