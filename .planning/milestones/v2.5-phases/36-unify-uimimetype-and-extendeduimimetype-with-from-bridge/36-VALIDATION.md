---
phase: 36
slug: unify-uimimetype-and-extendeduimimetype-with-from-bridge
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 36 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test --lib -p pmcp --features mcp-apps types::mcp_apps::tests -- mime` |
| **Full suite command** | `cargo test --features mcp-apps -p pmcp` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -p pmcp --features mcp-apps types::mcp_apps::tests -- mime`
- **After every plan wave:** Run `cargo test --features mcp-apps -p pmcp`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 36-01-01 | 01 | 1 | N/A | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_from_ui_mime_type` | ❌ W0 | ⬜ pending |
| 36-01-02 | 01 | 1 | N/A | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_try_from_extended` | ❌ W0 | ⬜ pending |
| 36-01-03 | 01 | 1 | N/A | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_try_from_extended_fails` | ❌ W0 | ⬜ pending |
| 36-01-04 | 01 | 1 | N/A | unit | `cargo test --features mcp-apps -p pmcp types::mcp_apps::tests::test_mime_type_round_trip` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Tests for `From<UIMimeType> for ExtendedUIMimeType` — new tests in `src/types/mcp_apps.rs`
- [ ] Tests for `TryFrom<ExtendedUIMimeType> for UIMimeType` — new tests in `src/types/mcp_apps.rs`
- [ ] Round-trip property test — new test in `src/types/mcp_apps.rs`

*Existing infrastructure covers framework and config — only new test functions needed.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
