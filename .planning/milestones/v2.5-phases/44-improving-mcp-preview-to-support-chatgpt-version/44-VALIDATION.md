---
phase: 44
slug: improving-mcp-preview-to-support-chatgpt-version
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-08
---

# Phase 44 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p mcp-preview` |
| **Full suite command** | `make tests` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mcp-preview`
- **After every plan wave:** Run `make tests`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 44-01-01 | 01 | 1 | PreviewMode enum | unit | `cargo test -p mcp-preview` | ❌ W0 | ⬜ pending |
| 44-01-02 | 01 | 1 | ResourceInfo _meta | unit | `cargo test -p mcp-preview` | ❌ W0 | ⬜ pending |
| 44-01-03 | 01 | 1 | ConfigResponse mode+keys | unit | `cargo test -p mcp-preview` | ❌ W0 | ⬜ pending |
| 44-01-04 | 01 | 1 | CLI --mode flag | manual | `cargo pmcp preview --mode chatgpt --help` | N/A | ⬜ pending |
| 44-02-01 | 02 | 2 | Protocol tab renders | manual | Browser visual inspection | N/A | ⬜ pending |
| 44-02-02 | 02 | 2 | ChatGPT postMessage | manual | Browser + widget test | N/A | ⬜ pending |
| 44-02-03 | 02 | 2 | window.openai stub | manual | Browser console check | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for PreviewMode enum serialization/deserialization
- [ ] Unit tests for ResourceInfo _meta field handling
- [ ] Unit tests for ConfigResponse with mode and descriptor_keys fields

*Wave 0 stubs needed since mcp-preview has minimal existing test coverage for these new structs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Protocol tab renders checks | Protocol diagnostics | Browser-only UI | Load preview, open Protocol tab, verify pass/fail rows |
| ChatGPT postMessage delivery | ChatGPT emulation | Requires running server + browser | Run `cargo pmcp preview --mode chatgpt`, trigger tool call, check widget receives data |
| window.openai stub present | ChatGPT emulation | Browser console check | Run in chatgpt mode, open iframe console, verify `window.openai` object exists |
| Mode badge displayed | UI indicator | Visual check | Verify header shows "Standard" or "ChatGPT Strict" badge |
| Terminal banner shows mode | CLI feedback | Visual check | Start preview in each mode, verify banner text |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
