---
phase: 105
slug: task-poll-decision-classifier-and-durable-consumer-docs
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-05
---

# Phase 105 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `#[tokio::test]`; `proptest` 1.7 for property tests |
| **Config file** | none (cargo-native); `Cargo.toml` dev-deps |
| **Quick run command** | `cargo test --lib types::tasks` + `cargo test --test task_augmented_result` |
| **Full suite command** | `make quality-gate` then `make doc-check` |
| **Estimated runtime** | ~60 seconds (quick); several minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib types::tasks` (fast, pure classifier/resolver tests)
- **After every plan wave:** Run `cargo test --test task_augmented_result` (11-test regression net) + `cargo run --example s48_durable_poll_decision`
- **Before `/gsd:verify-work`:** `make quality-gate` AND `make doc-check` both green
- **Max feedback latency:** ~60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | D-03 (exhaustive map: every `TaskStatus` → expected `TaskPollDecision`) | — | N/A | unit/property | `cargo test --lib types::tasks::poll_decision` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D-08/D-12 (resolver precedence: caller override > hint > 1000 default; 50 ms floor) | — | N/A | unit | `cargo test --lib types::tasks::resolve_poll_interval` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D-02/D-13 (drift pin: `wait_for_task` matches classifier; `input_required` error byte-identical) | — | N/A | integration | `cargo test --test task_augmented_result wait_for_task_surfaces_input_required` | ✅ | ⬜ pending |
| TBD | TBD | TBD | D-09 (budget clamp prevents oversleep, stays outside resolver) | — | N/A | integration | `cargo test --test task_augmented_result wait_for_task_timeout_is_not_overshot` | ✅ | ⬜ pending |
| TBD | TBD | TBD | D-13 (terminal → returns `tasks/result` unchanged) | — | N/A | integration | `cargo test --test task_augmented_result wait_for_task_returns_terminal_result` | ✅ | ⬜ pending |
| TBD | TBD | TBD | D-10 (runnable plain-loop durable-consumer example) | — | N/A | example | `cargo run --example s48_durable_poll_decision` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*(Task IDs filled in by the planner — requirement set for this phase is CONTEXT.md decisions D-01..D-16; no separate REQ IDs are mapped.)*

---

## Wave 0 Requirements

- [ ] `src/types/tasks.rs` — `#[cfg(test)]` unit + proptest for `poll_decision()` exhaustive map (D-03)
- [ ] `src/types/tasks.rs` — unit tests for `resolve_poll_interval` precedence + floor (D-08/D-12)
- [ ] `examples/s48_durable_poll_decision.rs` — new runnable example (D-10)
- [ ] (optional) strengthen `tests/task_augmented_result.rs:417` to assert the `input_required` message substring (D-13 pin)
- [ ] Framework install: none — proptest already a dev-dep

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| pmcp-book durable-consumer page reads correctly and renders | D-11 | Prose quality not automatable; `make doc-check` checks only rustdoc (not mdbook anchors), `make book` only compiles the book — neither validates the intra-page anchor | `make book` (compiles book) + grep the `#durable-and-replay-consumers` slug matches the heading; then open the rendered `pmcp-book/src/ch12-7-tasks.md` and review the new durable-consumer section |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (6/6 tasks carry automated verify — plan-checker Dimension 8a/8b PASS)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (100% coverage — 8c PASS)
- [x] Wave 0 covers all MISSING references (no `<automated>MISSING</automated>` markers — 8d PASS)
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-05 (gsd-plan-checker Dimension 8 PASS)
