---
phase: 50
slug: improve-binary-release
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 50 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual validation (CI/CD workflow changes + installer scripts) |
| **Config file** | `.github/workflows/release.yml`, `release-tester.yml`, `release-preview.yml` |
| **Quick run command** | `yamllint .github/workflows/release*.yml && shellcheck install/install.sh` |
| **Full suite command** | Push pre-release tag to trigger full pipeline |
| **Estimated runtime** | ~15 minutes (cross-platform CI builds) |

---

## Sampling Rate

- **After every task commit:** Run `yamllint .github/workflows/release*.yml` + `shellcheck install/install.sh`
- **After every plan wave:** Manual `workflow_dispatch` of binary workflows
- **Before `/gsd:verify-work`:** Full pre-release tag push test
- **Max feedback latency:** 15 minutes (CI build time)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 50-01-01 | 01 | 1 | TRIGGER | smoke | Push tag, verify assets on release | N/A | ⬜ pending |
| 50-01-02 | 01 | 1 | ARM-MAC | smoke | Check aarch64-apple-darwin asset | N/A | ⬜ pending |
| 50-01-03 | 01 | 1 | ARM-LIN | smoke | Check aarch64-unknown-linux-gnu asset | N/A | ⬜ pending |
| 50-02-01 | 02 | 2 | INSTALL-SH | manual | `curl -fsSL .../install.sh \| sh` | ❌ W0 | ⬜ pending |
| 50-02-02 | 02 | 2 | INSTALL-PS1 | manual | Run on Windows | ❌ W0 | ⬜ pending |
| 50-02-03 | 02 | 2 | BINSTALL | manual | `cargo binstall mcp-tester --dry-run` | N/A | ⬜ pending |
| 50-02-04 | 02 | 2 | CHECKSUMS | smoke | `gh release view --json assets` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `install/install.sh` — new file, OS/arch detection + download + checksum verify
- [ ] `install/install.ps1` — new file, Windows PowerShell installer
- [ ] Workflow modifications are in-place updates, no new test infrastructure needed

*Existing infrastructure: yamllint, shellcheck for linting*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Binary workflows auto-trigger on tag push | TRIGGER | Requires actual GitHub release event | Push a v0.0.0-test tag, verify binary workflows start |
| install.sh works on macOS ARM64 | INSTALL-SH | Requires macOS hardware | Run on Apple Silicon Mac |
| install.ps1 works on Windows | INSTALL-PS1 | Requires Windows | Run in PowerShell |
| cargo binstall resolves metadata | BINSTALL | Requires published crate | `cargo binstall mcp-tester --dry-run` after publish |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 900s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
