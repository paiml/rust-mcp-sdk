---
phase: 50-improve-binary-release
plan: 02
subsystem: infra
tags: [installer, shell-script, powershell, cargo-binstall, binary-distribution, sha256]

# Dependency graph
requires:
  - phase: 50-improve-binary-release (plan 01)
    provides: Reusable release workflows with 5-target matrix and SHA256 checksums using Rust target triple naming
provides:
  - POSIX install.sh for Linux/macOS binary installation with OS/arch auto-detection
  - PowerShell install.ps1 for Windows binary installation
  - cargo-binstall metadata for mcp-tester and mcp-preview
affects: [binary-distribution, release-pipeline, developer-onboarding]

# Tech tracking
tech-stack:
  added: []
  patterns: [POSIX shell installer with checksum verification, PowerShell installer with Get-FileHash, cargo-binstall pkg-url templates]

key-files:
  created:
    - install/install.sh
    - install/install.ps1
  modified:
    - crates/mcp-tester/Cargo.toml
    - crates/mcp-preview/Cargo.toml

key-decisions:
  - "POSIX /bin/sh (not bash) for maximum compatibility across Linux and macOS"
  - "TMPDIR variable renamed to TMPDIR_INSTALL to avoid shadowing the system TMPDIR env var"
  - "Explicit repo URL in binstall pkg-url (not { repo } template) to avoid redirect issues"
  - "pkg-fmt = bin for bare binaries matching the release workflow output (not archived)"
  - "Binstall uses v{ version } prefix in tag URL matching release tag convention"

patterns-established:
  - "Installer naming: install/install.sh and install/install.ps1 at repo root"
  - "One-liner install via curl pipe: curl -fsSL .../install.sh | sh"

requirements-completed: [INSTALL-SH, INSTALL-PS1, BINSTALL]

# Metrics
duration: 2min
completed: 2026-03-13
---

# Phase 50 Plan 02: Installer Scripts and cargo-binstall Metadata Summary

**POSIX install.sh and PowerShell install.ps1 for zero-friction binary installation with SHA256 checksum verification, plus cargo-binstall metadata for mcp-tester and mcp-preview**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-13T17:11:01Z
- **Completed:** 2026-03-13T17:13:16Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- Created POSIX-compatible install.sh that auto-detects Linux/macOS + x86_64/aarch64, downloads correct binary using Rust target triple naming, and verifies SHA256 checksum
- Created PowerShell install.ps1 for Windows x86_64 with architecture detection, binary download, SHA256 verification, and PATH guidance
- Added cargo-binstall metadata to both mcp-tester and mcp-preview Cargo.toml files for `cargo binstall` support

## Task Commits

Each task was committed atomically:

1. **Task 1: Create install.sh for Linux and macOS** - `f69b4ce` (feat)
2. **Task 2: Create install.ps1 for Windows** - `75fa848` (feat)
3. **Task 3: Add cargo-binstall metadata to mcp-tester and mcp-preview Cargo.toml** - `20aaa00` (feat)

## Files Created/Modified
- `install/install.sh` - POSIX shell installer with OS/arch detection, download, SHA256 checksum verification, and --tool/--version/--dir flags
- `install/install.ps1` - PowerShell installer with architecture detection, download, SHA256 verification, and -Tool/-Version/-InstallDir parameters
- `crates/mcp-tester/Cargo.toml` - Added [package.metadata.binstall] with pkg-url and pkg-fmt = "bin"
- `crates/mcp-preview/Cargo.toml` - Added [package.metadata.binstall] with pkg-url and pkg-fmt = "bin"

## Decisions Made
- Used /bin/sh (POSIX) instead of bash for install.sh to maximize compatibility across Linux distributions and macOS
- Named temp directory variable TMPDIR_INSTALL instead of TMPDIR to avoid shadowing the standard TMPDIR environment variable (shellcheck SC2030-safe)
- Used explicit GitHub repo URL in binstall pkg-url template rather than { repo } to avoid potential redirect from the crates.io repository field
- Set pkg-fmt = "bin" since the release workflow uploads bare binaries (not tarballs)
- Binstall pkg-url includes hardcoded `v` prefix before `{ version }` to match GitHub release tag convention (tags are v1.18.0, crate versions are 1.18.0)
- Note: cargo-binstall version resolution assumes crate version matches release tag; since the workspace uses a single SDK-version tag but crates have independent versions, binstall may require `--version` matching the release tag until tool-specific tags are added

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Both installer scripts ready to use once the next GitHub release is published with binary assets
- Users can install via `curl -fsSL .../install.sh | sh` or `irm .../install.ps1 | iex`
- cargo-binstall metadata active; `cargo binstall mcp-tester` will work when version/tag alignment is resolved
- Phase 50 (improve-binary-release) is now complete

## Self-Check: PASSED

All 4 files verified present. All 3 task commits verified in git log.

---
*Phase: 50-improve-binary-release*
*Completed: 2026-03-13*
