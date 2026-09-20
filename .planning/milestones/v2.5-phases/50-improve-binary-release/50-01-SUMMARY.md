---
phase: 50-improve-binary-release
plan: 01
subsystem: infra
tags: [github-actions, ci-cd, binary-release, workflow_call, sha256, arm64]

# Dependency graph
requires: []
provides:
  - Reusable release-tester.yml workflow (workflow_call + workflow_dispatch)
  - Reusable release-preview.yml workflow (workflow_call + workflow_dispatch)
  - 5-target binary matrix for both tools (x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows)
  - Per-binary SHA256 checksum files uploaded to release
  - release.yml orchestrator calling both binary workflows after create-release
affects: [release-pipeline, binary-distribution]

# Tech tracking
tech-stack:
  added: []
  patterns: [workflow_call reusable workflows, Rust target triple asset naming, per-binary sha256 checksums]

key-files:
  modified:
    - .github/workflows/release-tester.yml
    - .github/workflows/release-preview.yml
    - .github/workflows/release.yml

key-decisions:
  - "Use Rust target triples for asset naming (mcp-tester-{target}) instead of OS-arch shorthand"
  - "Per-binary .sha256 files rather than aggregated SHA256SUMS file for simplicity"
  - "macos-15-intel for x86_64-apple-darwin, macos-14 for aarch64-apple-darwin (macos-13 deprecated Dec 2025)"
  - "ubuntu-24.04-arm for native aarch64-linux builds (free for public repos)"
  - "fail-fast: false so one platform failure does not cancel others"

patterns-established:
  - "Reusable workflow pattern: workflow_call with tag_name input + workflow_dispatch fallback"
  - "Binary naming: {tool}-{rust-target-triple}{ext} for unambiguous architecture identification"

requirements-completed: [TRIGGER, ARM-MAC, ARM-LIN, CHECKSUMS]

# Metrics
duration: 2min
completed: 2026-03-13
---

# Phase 50 Plan 01: Fix Binary Release Pipeline Summary

**Reusable workflow_call pattern for mcp-tester and mcp-preview with 5-target matrix (x86_64/aarch64 Linux+macOS, x86_64 Windows) and per-binary SHA256 checksums**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-13T17:06:38Z
- **Completed:** 2026-03-13T17:08:39Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Fixed broken binary release trigger by converting from `on: release: types: [published]` to `workflow_call` reusable workflows called from release.yml
- Added ARM64 targets for both macOS (aarch64-apple-darwin via macos-14) and Linux (aarch64-unknown-linux-gnu via ubuntu-24.04-arm)
- Fixed macOS runner mislabeling: x86_64 builds now use macos-15-intel instead of macos-latest (which is now ARM64)
- Added per-binary SHA256 checksum generation and upload for all 10 binaries
- Standardized asset naming to Rust target triples for unambiguous architecture identification

## Task Commits

Each task was committed atomically:

1. **Task 1: Convert release-tester.yml to reusable workflow with 5-target matrix** - `4bbbffa` (feat)
2. **Task 2: Convert release-preview.yml to reusable workflow with 5-target matrix** - `9343127` (feat)
3. **Task 3: Wire binary workflows into release.yml as downstream jobs** - `669d8b5` (feat)

## Files Created/Modified
- `.github/workflows/release-tester.yml` - Reusable workflow building mcp-tester for 5 targets with SHA256 checksums
- `.github/workflows/release-preview.yml` - Reusable workflow building mcp-preview for 5 targets with SHA256 checksums
- `.github/workflows/release.yml` - Added outputs.version to create-release, added build-tester and build-preview downstream jobs

## Decisions Made
- Used Rust target triples for asset naming (e.g., `mcp-tester-x86_64-apple-darwin`) -- unambiguous, cargo-binstall compatible, standard in Rust ecosystem
- Per-binary `.sha256` files rather than aggregated SHA256SUMS -- simpler, no artifact passing between jobs needed
- `macos-15-intel` for x86_64-apple-darwin since `macos-13` was deprecated Dec 2025
- `macos-14` for aarch64-apple-darwin (native Apple Silicon M1 runner)
- `ubuntu-24.04-arm` for aarch64-linux (native ARM64, free for public repos since Aug 2025)
- `fail-fast: false` to prevent single-platform failure from canceling other builds
- Added cargo cache to binary workflows for faster builds

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Binary release pipeline is functional; next tag push will produce 10 binaries + 10 checksums
- Plan 02 (installer scripts, cargo-binstall metadata) can proceed independently
- The renamed asset naming convention (target triples) means old installer/download scripts expecting the old names will need updating

## Self-Check: PASSED

All 3 files verified present. All 3 task commits verified in git log.

---
*Phase: 50-improve-binary-release*
*Completed: 2026-03-13*
