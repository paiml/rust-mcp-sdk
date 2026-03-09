---
phase: 39
slug: add-deep-merge-for-ui-meta-key-to-prevent-collision
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 39 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test --lib -p pmcp` |
| **Full suite command** | `make tests` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -p pmcp`
- **After every plan wave:** Run `make tests`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 39-01-01 | 01 | 1 | N/A | unit | `cargo test deep_merge_disjoint -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-02 | 01 | 1 | N/A | unit | `cargo test deep_merge_nested -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-03 | 01 | 1 | N/A | unit | `cargo test deep_merge_leaf_collision -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-04 | 01 | 1 | N/A | unit | `cargo test deep_merge_array -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-05 | 01 | 1 | N/A | unit | `cargo test with_meta_entry -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-06 | 01 | 1 | N/A | unit | `cargo test typed_tool_with_output.*ui -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-07 | 01 | 1 | N/A | unit | `cargo test typed_tool_with_output.*coexist -p pmcp` | ❌ W0 | ⬜ pending |
| 39-01-08 | 01 | 1 | N/A | unit | `cargo test tool_info_with_ui -p pmcp` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `deep_merge` unit tests in `src/types/ui.rs`
- [ ] `with_meta_entry` tests in `src/types/protocol.rs`
- [ ] `TypedToolWithOutput` UI tests in `src/server/typed_tool.rs`

*Existing test infrastructure covers framework setup.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
