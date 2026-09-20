---
phase: 29
slug: auth-flag-propagation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 29 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml workspace test config |
| **Quick run command** | `cargo test -p cargo-pmcp --lib -- auth` |
| **Full suite command** | `cargo test -p cargo-pmcp` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp --lib -- auth`
- **After every plan wave:** Run `cargo test -p cargo-pmcp && make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 29-01-01 | 01 | 1 | AUTH-01 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |
| 29-01-02 | 01 | 1 | AUTH-02 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |
| 29-01-03 | 01 | 1 | AUTH-03 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |
| 29-01-04 | 01 | 1 | AUTH-04 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |
| 29-01-05 | 01 | 1 | AUTH-05 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |
| 29-01-06 | 01 | 1 | AUTH-06 | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cargo-pmcp/src/commands/auth.rs` — shared `resolve_auth_middleware` function + tests
- [ ] Unit tests for `AuthFlags::resolve()` covering None, ApiKey, OAuth variants
- [ ] Unit tests for clap parse-level mutual exclusion (`--api-key` conflicts with `--oauth-client-id`)
- [ ] Integration pattern: cannot test real OAuth flow in unit tests; verify struct wiring only

*Existing infrastructure covers test framework — only test files need creation.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OAuth browser flow works | AUTH-01..06 | Requires real OAuth provider and browser | Run `cargo pmcp test check <oauth-url> --oauth-client-id <id>` and verify browser opens, token cached |
| API key auth works end-to-end | AUTH-01..06 | Requires running auth-protected MCP server | Run `cargo pmcp test check <url> --api-key <key>` and verify successful connection |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
