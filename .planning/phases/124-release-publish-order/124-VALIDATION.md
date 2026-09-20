---
phase: 124
slug: release-publish-order
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-26
---

# Phase 124 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + bash scripts (`scripts/check-release-coverage.sh`) |
| **Config file** | Root `Cargo.toml` workspace + `Makefile` (`make quality-gate` matches CI) |
| **Quick run command** | `./scripts/check-release-coverage.sh` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~600 seconds (quality-gate); ~5 seconds (coverage script) |

---

## Sampling Rate

- **After every task commit:** Run `./scripts/check-release-coverage.sh`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | — | — | PKGR-01 | — | Gate fails when a publishable crate's publish step is deleted | integration | `./scripts/check-release-coverage.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements — `scripts/check-release-coverage.sh` exists and is wired into CI (`.github/workflows/ci.yml:233`); the coverage-gate extension is a modification, not new infrastructure.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| crates.io publish + tag push | PKGR-01 | Publishing is a one-way external action triggered by pushing a `v*` tag to upstream; cannot be dry-run against the live registry in CI | After merge + green CI: tag per CLAUDE.md Release Steps; watch `.github/workflows/release.yml` run; verify with `curl -s https://crates.io/api/v1/crates/<name>/versions` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 600s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
