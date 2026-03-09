---
phase: 41
slug: chatgpt-mcp-apps-upgraded-version
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-07
---

# Phase 41 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (nextest) |
| **Config file** | Cargo.toml, .config/nextest.toml |
| **Quick run command** | `cargo test --features full --lib` |
| **Full suite command** | `make test` |
| **Estimated runtime** | ~35 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full --lib`
- **After every plan wave:** Run `make test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 35 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 41-01-01 | 01 | 1 | MIME type fix | unit | `cargo test --features full types::mcp_apps` | ✅ | ⬜ pending |
| 41-01-02 | 01 | 1 | Resource content _meta | unit | `cargo test --features full types::protocol` | ✅ | ⬜ pending |
| 41-02-01 | 02 | 1 | Bridge protocol names | unit | `cargo test --features full` | ❌ W0 | ⬜ pending |
| 41-02-02 | 02 | 1 | Preview strict mode | integration | `cargo test --features full` | ❌ W0 | ⬜ pending |
| 41-03-01 | 03 | 2 | Scaffold template fix | unit | `cargo test -p cargo-pmcp` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Tests for new `_meta` field on `Content::Resource` resource contents
- [ ] Tests for corrected MIME type `text/html;profile=mcp-app`
- [ ] Tests for bridge protocol message name mapping
- [ ] Tests for preview strict widget resolution

*Existing test infrastructure covers framework needs — no new deps required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Widget renders in ChatGPT | End-to-end | Requires ChatGPT host | Deploy example server, open in ChatGPT, invoke tool, verify widget renders |
| Preview matches ChatGPT behavior | Strict mode | Requires browser | Run `cargo pmcp preview`, verify widget only loads when tool links to it |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 35s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
