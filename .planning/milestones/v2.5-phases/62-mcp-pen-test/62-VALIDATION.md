---
phase: 62
slug: mcp-pen-test
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-28
---

# Phase 62 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[cfg(test)]` + `#[tokio::test]` + `proptest` |
| **Config file** | None needed -- uses cargo test |
| **Quick run command** | `cargo test -p cargo-pmcp --lib pentest -- --test-threads=1` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~30 seconds (quick), ~120 seconds (full quality gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp --lib pentest -- --test-threads=1`
- **After every plan wave:** Run `cargo test -p cargo-pmcp`
- **Before `/gsd:verify-work`:** `make quality-gate` must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 62-01-01 | 01 | 1 | D-08 (Severity) | unit | `cargo test -p cargo-pmcp --lib pentest::types -- --test-threads=1` | Wave 0 | pending |
| 62-01-01 | 01 | 1 | D-10 (fail-on) | unit | `cargo test -p cargo-pmcp --lib pentest::config -- --test-threads=1` | Wave 0 | pending |
| 62-01-01 | 01 | 1 | D-11 (rate-limit) | unit | `cargo test -p cargo-pmcp --lib pentest::rate_limiter -- --test-threads=1` | Wave 0 | pending |
| 62-01-01 | 01 | 1 | D-09 (JSON output) | unit | `cargo test -p cargo-pmcp --lib pentest::report -- --test-threads=1` | Wave 0 | pending |
| 62-01-01 | 01 | 1 | D-09 (SARIF output) | unit | `cargo test -p cargo-pmcp --lib pentest::sarif -- --test-threads=1` | Wave 0 | pending |
| 62-01-02 | 01 | 1 | D-02 (payloads) | unit | `cargo test -p cargo-pmcp --lib pentest::payloads -- --test-threads=1` | Wave 0 | pending |
| 62-01-02 | 01 | 1 | D-05 (CLI) | build | `cargo run -p cargo-pmcp -- pentest --help` | Wave 0 | pending |
| 62-02-01 | 02 | 2 | D-02 (PI attacks) | unit | `cargo test -p cargo-pmcp --lib pentest::attacks::prompt_injection -- --test-threads=1` | Wave 0 | pending |
| 62-02-02 | 02 | 2 | D-03 (TP attacks) | unit | `cargo test -p cargo-pmcp --lib pentest::attacks::tool_poisoning -- --test-threads=1` | Wave 0 | pending |
| 62-03-01 | 03 | 2 | D-04 (SS attacks) | unit+prop | `cargo test -p cargo-pmcp --lib pentest::attacks::session_security -- --test-threads=1` | Wave 0 | pending |
| 62-03-02 | 03 | 2 | D-12 (destructive) | integration | `make quality-gate` | Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `cargo-pmcp/src/pentest/` directory -- entire module is new (created in Plan 01 Task 1)
- [ ] `serde-sarif` and `governor` dependencies added to `cargo-pmcp/Cargo.toml` (Plan 01 Task 1)
- [ ] Test infrastructure for pentest report serialization (Plan 01 Task 1 unit tests)
- [ ] `proptest` already in cargo-pmcp dev-dependencies -- no additional install needed

---

## Property Test Coverage

| Module | Property | Test Command |
|--------|----------|--------------|
| session_security | Shannon entropy is non-negative for any input | `cargo test -p cargo-pmcp --lib pentest::attacks::session_security::proptest -- --test-threads=1` |
| session_security | Shannon entropy bounded by log2(charset_size) | same as above |
| session_security | Distinct random strings produce high entropy | same as above |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| D-13 discovery against live server | D-13 | Requires running MCP server | Start example server, run `cargo pmcp pentest http://localhost:8080` |
| Session tests against live server | D-04 | Requires HTTP-based MCP server with session support | Start streamable-http example, run `cargo pmcp pentest http://localhost:8080 --category session-security` |
| SARIF upload to GitHub Security tab | D-09 | Requires GitHub Actions pipeline | Run pentest with `--format sarif -o results.sarif`, upload via `gh` CLI |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
