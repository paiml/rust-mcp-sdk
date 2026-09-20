---
phase: 48-mcp-apps-documentation-and-education-refresh
plan: 01
subsystem: docs
tags: [mcp-apps, documentation, readme, mdbook, github-actions, mcp-tester, mcp-preview]

# Dependency graph
requires:
  - phase: 47-add-mcp-app-support-to-mcp-tester
    provides: mcp-tester apps subcommand and cargo pmcp test apps wrapper
  - phase: 44-improving-mcp-preview-to-support-chatgpt-version
    provides: mcp-preview --mode chatgpt and Protocol/Bridge tabs
  - phase: 45-extend-mcp-apps-support-to-claude-desktop
    provides: with_host_layer, ToolInfo::with_ui, standard-first SDK APIs
provides:
  - Updated mcp-tester README with Installation section and apps subcommand documentation
  - Updated mcp-preview README expanded from 40 to 171 lines with full feature docs
  - Rewritten ch12-5-mcp-apps.md using GUIDE.md as authoritative source
  - App Metadata Testing subsection in ch15-testing.md
  - Cross-platform mcp-preview binary release workflow
affects: [48-02, course-content, release-workflow]

# Tech tracking
tech-stack:
  added: []
  patterns: [standard-first-documentation, guide-as-source-of-truth]

key-files:
  created:
    - .github/workflows/release-preview.yml
  modified:
    - crates/mcp-tester/README.md
    - crates/mcp-preview/README.md
    - pmcp-book/src/ch12-5-mcp-apps.md
    - pmcp-book/src/ch15-testing.md

key-decisions:
  - "Used GUIDE.md as authoritative source for ch12-5 rewrite rather than patching the existing chapter"
  - "Eliminated ChatGptAdapter from ch12-5 entirely -- standard SDK APIs (with_host_layer, ToolInfo::with_ui) are the primary pattern"
  - "Kept ch15 App Metadata Testing subsection brief (3-5 paragraphs) with cross-reference to ch12-5 for full docs"
  - "Modeled release-preview.yml on release-tester.yml for consistency"

patterns-established:
  - "Standard-first documentation: with_host_layer and standard SDK APIs appear before any ChatGPT-specific content"
  - "Installation sections use three options: pre-built binaries, cargo install, cargo-pmcp wrapper"

requirements-completed: [DOCS-01, DOCS-02, DOCS-03]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 48 Plan 01: Documentation and Education Refresh Summary

**Updated tool READMEs with Installation sections and apps validation docs, rewrote book ch12-5 with standard-first GUIDE.md content (ext-apps App class, with_host_layer, ToolInfo::with_ui, structuredContent, Vite bundling), and added mcp-preview binary release workflow**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T19:46:54Z
- **Completed:** 2026-03-12T19:52:35Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- mcp-tester README now documents the apps subcommand with all flags, 8 validation checks, sample output, and Installation section
- mcp-preview README expanded from 40 lines to 171 lines covering Installation, DevTools Tabs, Multi-Host Preview, and Architecture
- ch12-5 rewritten from ChatGptAdapter-first paradigm to standard-first using GUIDE.md as source of truth
- ch15 has new App Metadata Testing subsection with mcp-tester apps and cargo pmcp test apps examples
- release-preview.yml workflow builds cross-platform mcp-preview binaries on GitHub Release events

## Task Commits

Each task was committed atomically:

1. **Task 1: Update mcp-tester and mcp-preview READMEs** - `ab11c93` (docs)
2. **Task 2: Rewrite book ch12-5 MCP Apps chapter and add ch15 testing subsection** - `b7bce92` (docs)
3. **Task 3: Create mcp-preview binary release workflow** - `06f47b7` (chore)

## Files Created/Modified
- `crates/mcp-tester/README.md` - Added Installation section, apps subcommand docs, App Metadata Tests category
- `crates/mcp-preview/README.md` - Full rewrite with Installation, Usage, DevTools Tabs, Bridge Modes, Multi-Host Preview, Architecture
- `pmcp-book/src/ch12-5-mcp-apps.md` - Major rewrite using GUIDE.md as source of truth with standard-first ordering
- `pmcp-book/src/ch15-testing.md` - Added App Metadata Testing subsection before Summary
- `.github/workflows/release-preview.yml` - Cross-platform binary release workflow for mcp-preview

## Decisions Made
- Used GUIDE.md as authoritative source for ch12-5 rewrite -- existing chapter was too outdated to patch incrementally
- Eliminated ChatGptAdapter entirely from ch12-5 -- standard SDK APIs (with_host_layer, ToolInfo::with_ui, UIResource::html_mcp_app) are now the primary documented pattern
- Kept ch15 App Metadata Testing subsection brief with cross-reference to ch12-5 for comprehensive coverage
- Modeled release-preview.yml exactly on release-tester.yml (same triggers, matrix, upload pattern) for workflow consistency

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tool READMEs and book chapters are current with Phases 34-47 changes
- Course content (48-02) can now align with the updated book chapter
- mcp-preview binaries will be built automatically on next release

## Self-Check: PASSED

All files exist, all commits verified, all verification criteria met.

---
*Phase: 48-mcp-apps-documentation-and-education-refresh*
*Completed: 2026-03-12*
