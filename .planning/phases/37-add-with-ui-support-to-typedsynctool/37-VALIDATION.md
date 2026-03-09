---
phase: 37
slug: add-with-ui-support-to-typedsynctool
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 37 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test framework) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test --lib -- typed_tool wasm_typed_tool` |
| **Full suite command** | `cargo test --test-threads=1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -- typed_tool wasm_typed_tool`
- **After every plan wave:** Run `cargo test --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 37-01-01 | 01 | 1 | P37-01 | unit | `cargo test --lib -- typed_tool::tests::test_typed_sync_tool_metadata_with_ui` | ❌ W0 | ⬜ pending |
| 37-01-02 | 01 | 1 | P37-02 | unit | `cargo test --lib -- typed_tool::tests::test_typed_sync_tool_metadata_without_ui` | ❌ W0 | ⬜ pending |
| 37-01-03 | 01 | 1 | P37-03 | unit | `cargo test --lib -- wasm_typed_tool::tests::test_wasm_typed_tool_info_with_ui` | ❌ W0 | ⬜ pending |
| 37-01-04 | 01 | 1 | P37-04 | unit | `cargo test --lib -- wasm_typed_tool::tests::test_wasm_typed_tool_info_without_ui` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements — tests will be created as part of implementation in existing `mod tests` blocks.*

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
