---
phase: 34
slug: fix-mcp-apps-chatgpt-compatibility
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 34 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p pmcp --lib types::mcp_apps --lib types::ui -- --test-threads=1` |
| **Full suite command** | `cargo test --workspace -- --test-threads=1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp --lib -- --test-threads=1`
- **After every plan wave:** Run `cargo test --workspace -- --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 34-01-01 | 01 | 1 | N/A-1 | unit | `cargo test -p pmcp --lib types::mcp_apps::tests -- --test-threads=1` | Existing tests need update | ⬜ pending |
| 34-01-02 | 01 | 1 | N/A-2 | unit | `cargo test -p pmcp --lib types::protocol -- --test-threads=1` | Need new test | ⬜ pending |
| 34-01-03 | 01 | 1 | N/A-3 | unit | `cargo test -p pmcp --lib server::typed_tool -- --test-threads=1` | Need new test | ⬜ pending |
| 34-01-04 | 01 | 1 | N/A-4 | unit | `cargo test -p pmcp --lib types::mcp_apps::tests -- --test-threads=1` | Existing test needs update | ⬜ pending |
| 34-02-01 | 02 | 1 | N/A-5 | build | `cargo check -p mcp-preview` | Build verification | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test for `ToolInfo::with_ui()` output format — no existing test covers _meta structure
- [ ] Test for `TypedTool::metadata()` with `openai/outputTemplate` key
- [ ] Test for new MIME type variant `text/html;profile=mcp-app`

*Existing infrastructure covers mcp-preview build verification.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Widget renders in ChatGPT | N/A | Requires ChatGPT UI | Test with MCP Inspector or ChatGPT developer mode |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
