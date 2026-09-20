---
phase: 60
slug: clean-up-mcp-preview-side-tabs
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 60 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual browser verification (vanilla HTML/CSS/JS SPA) |
| **Config file** | none — no test framework for frontend assets |
| **Quick run command** | `cargo build -p mcp-preview && cargo run -p mcp-preview` |
| **Full suite command** | `cargo build -p mcp-preview && cargo clippy -p mcp-preview -- -D warnings` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build -p mcp-preview`
- **After every plan wave:** Run `cargo clippy -p mcp-preview -- -D warnings`
- **Before `/gsd:verify-work`:** Full build + manual browser check
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 60-01-01 | 01 | 1 | D-10..D-13 | grep + build | `grep -c "console" crates/mcp-preview/assets/index.html` | ✅ | ⬜ pending |
| 60-01-02 | 01 | 1 | D-01..D-06 | build + manual | `cargo build -p mcp-preview` | ✅ | ⬜ pending |
| 60-01-03 | 01 | 1 | D-07..D-09 | grep + build | `grep "Clear All" crates/mcp-preview/assets/index.html` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No new test framework needed — changes are to a single HTML asset file validated by build + grep + manual browser inspection.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Drag-to-resize DevTools panel | D-02, D-03 | Mouse interaction in browser | Open preview, drag left boundary of DevTools panel. Verify resizes smoothly. Drag to zero width — panel should collapse. |
| Dev Tools toggle button | D-01 | Click interaction | Click "Dev Tools" button in header. Panel should close. Click again — should reopen at 350px. |
| Global Clear All | D-07 | Click interaction | Open preview, trigger some tool calls to populate tabs. Click "Clear All". Verify all tabs (Network, Events, Protocol, Bridge) are cleared. |
| Console tab removed | D-10, D-11 | Visual inspection | Open preview. Verify only 4 tabs visible: Network, Events, Protocol, Bridge. No "Console" tab. |
| Default tab is Network | D-11 | Visual inspection | Open preview fresh. Network tab should be active by default. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
