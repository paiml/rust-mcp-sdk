---
phase: 54
slug: protocol-version-2025-11-25-type-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 54 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo check -p pmcp --features full` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p pmcp --features full`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 54-01-01 | 01 | 1 | TYPE-CLEANUP | build | `cargo check -p pmcp --features full` | ✅ | ⬜ pending |
| 54-01-02 | 01 | 1 | VERSION-NEGOTIATION | build+test | `cargo test -p pmcp --features full --lib -- version` | ✅ | ⬜ pending |
| 54-02-01 | 02 | 2 | PROTO-2025-11-25 | build | `cargo check -p pmcp --features full` | ✅ | ⬜ pending |
| 54-02-02 | 02 | 2 | PROTO-2025-11-25 | build+test | `cargo test -p pmcp --features full --lib` | ✅ | ⬜ pending |
| 54-03-01 | 03 | 3 | TYPE-CLEANUP | build | `cargo check --workspace` | ✅ | ⬜ pending |
| 54-03-02 | 03 | 3 | TYPE-CLEANUP | build+test | `make quality-gate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| MIGRATION.md accuracy | TYPE-CLEANUP | Document review | Read MIGRATION.md, verify all renames and path changes match actual code |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
