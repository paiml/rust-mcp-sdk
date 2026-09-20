---
phase: 45
slug: extend-mcp-apps-support-to-claude-desktop
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-09
---

# Phase 45 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + inline tests |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test --lib -p pmcp -- --test-threads=1` |
| **Full suite command** | `cargo test --lib --tests -p pmcp -- --test-threads=1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -p pmcp -- --test-threads=1`
- **After every plan wave:** Run `cargo test --lib --tests -- --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| P45-01 | 01 | 1 | Standard-only build_meta_map | unit | `cargo test --lib -p pmcp types::ui::tests::test_build_meta_map -- --test-threads=1` | Exists (needs update) | ⬜ pending |
| P45-02 | 01 | 1 | with_host_layer adds openai/* keys | unit | `cargo test --lib -p pmcp -- host_layer --test-threads=1` | ❌ W0 | ⬜ pending |
| P45-03 | 01 | 1 | uri_to_tool_meta indexes standard keys | unit | `cargo test --lib -p pmcp -- uri_to_tool_meta --test-threads=1` | Exists (needs update) | ⬜ pending |
| P45-04 | 02 | 2 | Standard mode preview serves standard bridge | integration | `cargo test --lib -p mcp-preview -- --test-threads=1` | ❌ W0 | ⬜ pending |
| P45-05 | 01 | 1 | ChatGPT adapter still injects window.openai | unit | `cargo test --lib -p pmcp -- test_chatgpt_adapter --test-threads=1` | ✅ | ⬜ pending |
| P45-06 | 02 | 2 | Examples render in standard mode | smoke | Manual: `cargo pmcp preview` per example | Manual | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Tests for `with_host_layer()` on ServerCoreBuilder
- [ ] Tests for standard-only `build_meta_map()` output (update existing)
- [ ] Tests for host-layer enrichment pipeline
- [ ] Tests for `build_uri_to_tool_meta()` with/without host layers

*Existing test infrastructure covers framework and config. Wave 0 adds phase-specific test stubs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Examples render correctly in Claude Desktop | Standard mode compatibility | Requires Claude Desktop app | Connect example server to Claude Desktop, verify widget renders |
| Examples render correctly in ChatGPT | ChatGPT layer backward compat | Requires ChatGPT account | Connect example server with ChatGPT layer to ChatGPT, verify widget renders |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
