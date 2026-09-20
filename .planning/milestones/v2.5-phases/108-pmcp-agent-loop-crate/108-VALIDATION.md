---
phase: 108
slug: pmcp-agent-loop-crate
status: ready
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
updated: 2026-07-17
---

# Phase 108 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Refreshed after the `--reviews` replan (transport-actor pump, WithTools end-to-end, `RunOutcome` retry classes, `wait_for_related_task`, real task lifecycle, `url-connector` feature gate, stdio cancel-safety).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `proptest` 1.7 + doctests |
| **Config file** | none (cargo default); workspace `Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-agent -- --test-threads=1` |
| **Full suite command** | `make quality-gate` (fmt --all, clippy pedantic+nursery -D warnings, build, workspace test, audit) |
| **Estimated runtime** | ~60s quick / ~10min full gate |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-agent -- --test-threads=1` (plus `cargo test -p pmcp --lib -- --test-threads=1` and `cargo test -p pmcp --test in_tool_peer_roundtrip` for the 108-01 core-transport tasks)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green (incl. PMAT cog ≤25 in CI)
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 108-01-01 | 01 | 1 | AGNT-04 | T-108-01/02/03/03b | Unbounded worker queue never blocks receive/drain; stdio cancel-safe (no byte loss) | integration (D-03 real loop) | `cargo test -p pmcp --test in_tool_peer_roundtrip -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 108-01-02 | 01 | 1 | AGNT-04 | T-108-04/05 | WithTools legacy decode fallback; preflight approval preserved | integration | `cargo test -p pmcp --test in_tool_peer_roundtrip -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 108-01-03 | 01 | 1 | AGNT-04 | — | Scaffold-pin drift guard tracks 2.17.0 | integration + drift guard | `cargo test -p cargo-pmcp emitted_pmcp_version_matches_workspace_pin` | ❌ W0 | ⬜ pending |
| 108-02-01 | 02 | 2 | AGNT-01 | T-108-SC | No new packages; default build pulls no reqwest | unit (compile) | `cargo build -p pmcp-agent && cargo tree -p pmcp-agent \| grep -c reqwest` | ❌ W0 | ⬜ pending |
| 108-02-02 | 02 | 2 | AGNT-01 | — | Object-safe seams; ToolCall id correlation | unit | `cargo test -p pmcp-agent seams::` | ❌ W0 | ⬜ pending |
| 108-02-03 | 02 | 2 | AGNT-01 | — | Resumable store; RunPhase checkpoint | unit | `cargo test -p pmcp-agent store::` | ❌ W0 | ⬜ pending |
| 108-03-01 | 03 | 3 | AGNT-02 | — | Pure decisions, no wall-clock/RNG; RunOutcome retry class as data | unit | `cargo test -p pmcp-agent iteration::decide` | ❌ W0 | ⬜ pending |
| 108-03-02 | 03 | 3 | AGNT-02 | — | load+checkpoint ordering (save pending before dispatch) | unit | `cargo test -p pmcp-agent iteration::engine` | ❌ W0 | ⬜ pending |
| 108-03-03 | 03 | 3 | AGNT-03 | — | Identical effects ⇒ identical decision sequences | property + fuzz | `cargo test -p pmcp-agent replay_safety` | ❌ W0 | ⬜ pending |
| 108-04-01 | 04 | 3 | AGNT-04 | T-108-04a | SamplingSource over server-side peer; SecretString redaction | integration (D-03) | `cargo test -p pmcp-agent sources::sampling` | ❌ W0 | ⬜ pending |
| 108-04-02 | 04 | 3 | AGNT-05 | T-108-04b | Loopback HTTP allowed, HTTPS-else; request timeout + bounded body; no key logging | unit (mock HTTP) | `cargo test -p pmcp-agent --features openai-compat sources::openai` | ❌ W0 | ⬜ pending |
| 108-04-03 | 04 | 3 | AGNT-06 | — | Anthropic history normalization (role alternation, system hoist) | unit (mock HTTP) | `cargo test -p pmcp-agent --features anthropic sources::anthropic` | ❌ W0 | ⬜ pending |
| 108-05-01 | 05 | 3 | AGNT-08 | — | wait_for_related_task hard max (no infinite poll); bounded batch; url-connector gated | integration | `cargo test -p pmcp-agent --test invoker_task_augmented -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 108-05-02 | 05 | 3 | AGNT-09 | T-108-05a | Env resolver reads secrets to redacted form, never logs; scoped/serialized env tests | unit | `cargo test -p pmcp-agent --test config_resolver -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 108-06-01 | 06 | 4 | AGNT-07 | T-108-06a | Real store-backed task lifecycle; request-scoped source factory | integration | `cargo test -p pmcp-agent --test adapter_agent_as_server -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 108-06-02 | 06 | 4 | AGNT-04/05/06/09 | — | wasm32 default-feature build clean (no url-connector/reqwest/StreamableHttpTransport) | example + integration + wasm compile gate | `cargo run -p pmcp-agent --example s50_standalone_vs_sampled && cargo build -p pmcp-agent --target wasm32-unknown-unknown` | ❌ W0 | ⬜ pending |
| 108-06-03 | 06 | 4 | AGNT-09 | — | D-09 mapping copies no private code | doc artifact | manual review of D-09 mapping vs iteration.rs | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-agent/Cargo.toml` — new workspace member (mirror `crates/pmcp-tasks`); add to root `[workspace].members`; features `openai-compat`, `anthropic`, `url-connector` all non-default (wasm-clean per D-13)
- [ ] `tests/in_tool_peer_roundtrip.rs` (pmcp core) — D-106-A proof: in-tool `sample` + `list_roots` + `sample_with_tools` round-trips on stock `Server::run`; saturation (queued second request) + shutdown cases
- [ ] stdio cancel-safety test (pmcp core) — drop a `receive()` mid-read, feed rest, assert the next `receive()` returns the whole line (BLOCKER-1)
- [ ] `crates/pmcp-agent/tests/replay_safety.rs` + `tests/fixtures/*.json` golden traces — AGNT-03 property over `EffectTrace`/`DecisionTrace`
- [ ] Fuzz target `fuzz/fuzz_targets/agent_digest.rs` (or proptest equivalent) — CLAUDE.md ALWAYS requirement
- [ ] `crates/pmcp-agent/tests/invoker_task_augmented.rs`, `tests/config_resolver.rs`, `tests/real_loop_sampling.rs`, `tests/e2e_package_to_adapter.rs`, `tests/adapter_agent_as_server.rs`
- [ ] `examples/s50_standalone_vs_sampled.rs` — ALWAYS requirement (default-feature runnable, network-free)
- [ ] CI wasm32 default-feature compile gate + native `--features openai-compat,anthropic,url-connector` build line (D-13)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Standalone example against a live local model (Ollama) | AGNT-05 | Requires a running LLM endpoint; CI uses mock HTTP | `ollama serve` then `cargo run -p pmcp-agent --features openai-compat --example s50_standalone_vs_sampled -- --standalone` |
| D-09 shape-compatibility mapping vs durable-agent-lambda | AGNT-02/03/09 design validation | Private-repo reference; human review that no private code was copied | Review the D-09 mapping doc (108-06 Task 3) against `pmcp-run/.../iteration.rs` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 600s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
