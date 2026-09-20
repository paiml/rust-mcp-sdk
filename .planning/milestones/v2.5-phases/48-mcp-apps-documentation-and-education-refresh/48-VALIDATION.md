---
phase: 48
slug: mcp-apps-documentation-and-education-refresh
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-12
---

# Phase 48 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | mdBook build verification |
| **Config file** | pmcp-book/book.toml, pmcp-course/book.toml |
| **Quick run command** | `mdbook build pmcp-book 2>&1 | tail -5` |
| **Full suite command** | `mdbook build pmcp-book && mdbook build pmcp-course` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `mdbook build pmcp-book 2>&1 | tail -5`
- **After every plan wave:** Run `mdbook build pmcp-book && mdbook build pmcp-course`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 48-01-01 | 01 | 1 | DOCS-01, DOCS-02 | content | `grep -c "apps" crates/mcp-tester/README.md && wc -l crates/mcp-preview/README.md` | N/A | ⬜ pending |
| 48-01-02 | 01 | 1 | DOCS-03 | content + build | `grep -c "with_host_layer" pmcp-book/src/ch12-5-mcp-apps.md && mdbook build pmcp-book 2>&1 | tail -5` | N/A | ⬜ pending |
| 48-02-01 | 02 | 2 | DOCS-04 | content + build | `grep -c "with_host_layer" pmcp-course/src/part8-advanced/ch20-mcp-apps.md && grep -c "onteardown" pmcp-course/src/part8-advanced/ch20-03-postmessage.md && mdbook build pmcp-course 2>&1 | tail -5` | N/A | ⬜ pending |
| 48-02-02 | 02 | 2 | DOCS-04 | content | `grep -c "mcp-tester apps" pmcp-course/src/part4-testing/ch11-02-mcp-tester.md` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Content accuracy vs GUIDE.md | DOCS-03 | Semantic comparison | Review ch12-5 section structure against GUIDE.md sections |
| Course-book alignment | DOCS-04 | Semantic comparison | Spot-check 3+ API names in course match book |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-12
