---
phase: 110
slug: cargo-pmcp-agent-team-verbs
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 110 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + assert_cmd for CLI integration |
| **Config file** | cargo-pmcp/Cargo.toml (dev-dependencies already include assert_cmd) |
| **Quick run command** | `cargo test -p cargo-pmcp` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~120 seconds (quick), ~600 seconds (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | | | CLI-01..CLI-04 | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Tripwire test scaffolding for `pmcp-agent` / `pmcp-package` version pins (const + include_str! + assert_eq! pattern from `templates/workbook_server.rs`)
- [ ] assert_cmd integration test stubs for `agent new`, `agent dev`, `team dev`, `package capture|show`

*Existing infrastructure (cargo test + assert_cmd) covers the framework layer; Wave 0 adds test stubs only.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `agent dev` against a live OpenAI-compat endpoint | CLI-02 | Requires external LLM endpoint credentials | Run `cargo pmcp agent dev` with a configured endpoint; observe a completed loop turn |
| `package capture` against the live platform API | CLI-04 | Platform capture API is out-of-repo (Open Question Q2) | Run `cargo pmcp package capture` against staging once endpoint contract is confirmed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
