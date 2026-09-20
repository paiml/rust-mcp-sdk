---
phase: 73
slug: typed-client-helpers-list-all-pagination-parity-client-01
status: task-ids-finalized
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-21
---

# Phase 73 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest + cargo-fuzz |
| **Config file** | root `Cargo.toml`, `fuzz/Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp --lib client:: --features full` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~45 seconds (quick) / ~6 minutes (full) |

---

## Sampling Rate

- **After every task commit:** `cargo test -p pmcp --lib client:: --features full`
- **After every plan wave:** `cargo test -p pmcp --features full && cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green
- **Max feedback latency:** 60 seconds for quick, 360 seconds for full

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 73-01-01 | 01 | 1 | PARITY-CLIENT-01 (ClientOptions scaffold) | — | `ClientOptions::default().max_iterations == 100`; `#[non_exhaustive]` enforces field-update idiom | unit | `cargo test -p pmcp --lib client::options::tests --features full` | ❌ W0 | ⬜ pending |
| 73-01-02 | 01 | 1 | PARITY-CLIENT-01 (ClientOptions wired through constructors) | — | Every `Client<T>` constructor initialises `options`; new `with_client_options` does not collide with existing `with_options` | unit | `cargo test -p pmcp --lib client::tests::test_client_new_uses_default_options client::tests::test_client_with_client_options_threads_value client::tests::test_client_with_options_preserves_default_client_options --features full` | ❌ W0 | ⬜ pending |
| 73-01-03 | 01 | 1 | PARITY-CLIENT-01 (typed helpers) | — | Serialize errors → `Error::Validation`, never panic; `get_prompt_typed` rejects non-object; string values not quoted | unit + property + doctest | `cargo test -p pmcp --lib client::tests::test_call_tool_typed_serialize_error_maps_to_validation client::tests::test_get_prompt_typed_non_object_rejected client::tests::test_get_prompt_typed_string_values_not_quoted --features full && cargo test -p pmcp --test property_tests prop_call_tool_typed_sends_expected_value --features full && cargo test --doc -p pmcp --features full client::` | ❌ W0 | ⬜ pending |
| 73-02-01 | 02 | 2 | PARITY-CLIENT-01 (list_all helpers + in-module tests) | T-73-01 | `max_iterations` cap enforced; `Some("")` cursor continues, only `None` terminates; multi-page aggregation in order | unit + integration + doctest | `cargo test -p pmcp --lib client::tests::test_list_all --features full && cargo test --doc -p pmcp --features full client::` | ❌ W0 | ⬜ pending |
| 73-02-02 | 02 | 2 | PARITY-CLIENT-01 (integration + properties) | T-73-01 | Cap enforcement survives randomised `cap in 1..20`; flat-concatenation holds for any N-page sequence | integration + property | `cargo test -p pmcp --test list_all_pagination --features full && cargo test -p pmcp --test property_tests prop_list_all_tools_flat_concatenation prop_list_all_tools_cap_enforced --features full` | ❌ W0 | ⬜ pending |
| 73-02-03 | 02 | 2 | PARITY-CLIENT-01 (cursor fuzz) | T-73-01 | Adversarial cursor sequences (empty, long, repeated, cyclic) never panic; only `Error::Validation` or transport variant returned | fuzz | `cd fuzz && cargo check --bin list_all_cursor_loop && cargo +nightly fuzz run list_all_cursor_loop -- -runs=1000` | ❌ W0 | ⬜ pending |
| 73-03-01 | 03 | 3 | PARITY-CLIENT-01 (example) | — | `c09_client_list_all` and updated `c02_client_tools` compile; both exercise typed helpers | example-check | `cargo check --example c09_client_list_all --features full && cargo check --example c02_client_tools --features full` | ❌ W0 | ⬜ pending |
| 73-03-02 | 03 | 3 | PARITY-CLIENT-01 (version bump coherence) | T-73-P3-01 | All 8 pmcp pins in 7 Cargo.toml files bumped atomically to 2.6.0; workspace builds | integration | `cargo build --workspace --all-features && ! grep -rE 'pmcp = \{ version = "2\.5\.0"' --include=Cargo.toml .` | ❌ W0 | ⬜ pending |
| 73-03-03 | 03 | 3 | PARITY-CLIENT-01 (docs + CI gate) | T-73-P3-02, T-73-P3-03 | CHANGELOG + REQUIREMENTS + README reflect shipped surface; full CI-matching gate green | integration | `make quality-gate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/client/options.rs` — new module with `ClientOptions` struct (created 73-01-01, needed by all waves)
- [ ] `src/client/mod.rs::tests` — extended with typed-helper + list_all in-module tests (73-01-02, 73-01-03, 73-02-01)
- [ ] `tests/list_all_pagination.rs` — new integration test wiring a paginated MockTransport (73-02-02)
- [ ] `tests/property_tests.rs` — extended with `prop_call_tool_typed_sends_expected_value`, `prop_list_all_tools_flat_concatenation`, `prop_list_all_tools_cap_enforced` (73-01-03, 73-02-02)
- [ ] `fuzz/fuzz_targets/list_all_cursor_loop.rs` — new fuzz target for `max_iterations` + cursor safety (73-02-03)
- [ ] `fuzz/Cargo.toml` — `[[bin]]` stanza for `list_all_cursor_loop` (73-02-03)
- [ ] `examples/c09_client_list_all.rs` — new example (filename avoids c08 collision with Phase 74's `c08_oauth_dcr.rs`) (73-03-01)

No framework install needed — `cargo test`, `proptest`, and `cargo-fuzz` are already wired in the workspace.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Rustdoc rendering on docs.rs | PARITY-CLIENT-01 | docs.rs build only triggers after release publish | After `pmcp 2.6.0` publishes, verify https://docs.rs/pmcp/latest/pmcp/client/struct.Client.html shows `call_tool_typed`, `list_all_tools`, `with_client_options`, and `pmcp::ClientOptions` without rendering warnings |
| CHANGELOG accuracy | PARITY-CLIENT-01 | Human judgement on wording/placement | Reviewer reads CHANGELOG v2.6.0 entry for completeness and semver-appropriateness |
| `cargo run --example c09_client_list_all` smoke | PARITY-CLIENT-01 | Requires an MCP server on stdio to complete the round-trip | Operator runs `cargo run --example c09_client_list_all --features full` against a known-good stdio MCP server; expects the "discovered N tools/prompts/resources" prints |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 60s (quick) / 360s (full)
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Task IDs finalised (`73-01-01` through `73-03-03`)

**Approval:** ready-for-execution
