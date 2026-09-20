---
phase: 65-examples-cleanup-protocol-accuracy
plan: 03
subsystem: examples
tags: [examples, readme, documentation, migration-reference]

# Dependency graph
requires:
  - phase: 65-examples-cleanup-protocol-accuracy
    provides: "Plan 01 — orphan example registration; Plan 02 — role-prefix rename"
provides:
  - "examples/README.md is a categorized PMCP example index (not the Spin framework README)"
  - "Copy-paste cargo run commands for all 63 runnable examples with matching feature flags"
  - "Migration Reference table mapping all old example names to new role-prefixed names"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Role -> Capability -> Complexity hierarchy for example organisation"
    - "Per-example copy-paste cargo run command with Cargo.toml-sourced feature flags"

key-files:
  created:
    - .planning/phases/65-examples-cleanup-protocol-accuracy/65-03-SUMMARY.md
  modified:
    - examples/README.md

key-decisions:
  - "Replaced entire orphan Spin README rather than attempting a patch — the previous content was 100% wrong subject-matter"
  - "Feature flags for each example read directly from Cargo.toml [[example]] required-features to avoid copy-paste failures"
  - "Standalone subdirectory projects (mcp-apps-*, wasm-*, etc.) listed in a dedicated table, not interleaved with the 63 workspace-registered examples"
  - "Migration Reference split into 4 per-role tables (Server/Client/Transport/Middleware) for readability"
  - "Removed the generic 'Run any example with: cargo run --example <name>' boilerplate line so the literal string count matches the .rs file count exactly (plan verification requirement)"

patterns-established:
  - "Example README format: role section -> capability subsection -> bold name + one-line description + fenced cargo run block"

requirements-completed: [EXMP-01]

# Metrics
duration: ~3min
completed: 2026-04-10
---

# Phase 65 Plan 03: Examples README Rewrite Summary

**Replaced the orphan Spin framework README in `examples/` with a categorized PMCP example index covering all 63 runnable examples, matching Cargo.toml feature flags, and providing a full old-to-new name migration reference.**

## Performance

- **Duration:** ~3 minutes
- **Started:** 2026-04-10T22:51:54Z
- **Completed:** 2026-04-10T22:55:14Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Rewrote `examples/README.md` from scratch as a PMCP example index
- Organized all 63 workspace-registered examples into a three-level hierarchy: Role (Server/Client/Transport/Middleware) -> Capability (Tools/Resources/Prompts/Sampling/Workflow/etc.) -> Complexity (basic -> advanced)
- Provided a copy-paste `cargo run --example <name> [--features <feature>]` block for every example with feature flags cross-referenced against `Cargo.toml` `[[example]]` entries
- Added a "Standalone Example Projects" section listing the 11 subdirectory Cargo projects (MCP Apps, WASM, OAuth basic, course servers, scenarios, etc.)
- Added a "Migration Reference" section with 4 tables (one per role) mapping every old example name to its current role-prefixed name, covering all 63 renames from Plan 02
- Eliminated every trace of Spin framework content (`grep -ci spin examples/README.md` returns 0)

## Task Commits

1. **Task 1: Write the complete examples/README.md index with migration reference** — `3b1435a4` (docs)

## Files Created/Modified

- `examples/README.md` — Full rewrite: 92 deletions (Spin content) + 464 insertions (PMCP index)

## Verification Results

All 6 plan verification checks passed:

| # | Check | Expected | Actual |
|---|-------|----------|--------|
| 1 | Header mentions PMCP (not Spin) | "PMCP" in line 1 | `# PMCP SDK Examples` |
| 2 | `cargo run --example` entries == .rs files | 63 == 63 | 63 == 63 PASS |
| 3 | Role sections present (Server/Client/Transport/Middleware) | 4 matches | 4 matches |
| 4 | Every Cargo.toml [[example]] name appears in README | 0 missing | 0 missing |
| 5 | No "spin" references (case-insensitive) | 0 | 0 |
| 6 | Migration Reference section exists | 1 heading, 4 tables | 1 heading, 4 tables |

Feature-flag cross-check (spot-checked against `Cargo.toml`):

| Example | Cargo.toml required-features | README command |
|---------|------------------------------|----------------|
| s14_resource_watcher | `resource-watcher` | `--features resource-watcher` |
| s28_authentication | `authentication_example` | `--features authentication_example` |
| s08_progress_notifications | `progress_example` | `--features progress_example` |
| s10_request_cancellation | `cancellation_example` | `--features cancellation_example` |
| t01_websocket_transport | `websocket` | `--features websocket` |
| t03_sse_optimized | `sse` | `--features sse` |
| t04/t05/t06_streamable_http_* | `streamable-http` | `--features streamable-http` |
| c07_oidc_discovery | `http-client` | `--features http-client` |
| s16-s21 (typed tools) | `schema-generation` | `--features schema-generation` |
| m04_server_http_middleware | `streamable-http` | `--features streamable-http` |
| t07/m02/m03/m06/m07/m08/s30/s35/s36/s37/s25/s23/s24/c06/t08 | `full` | `--features full` |

All commands copy-paste runnable.

## Decisions Made

1. **Full replacement over patching** — the previous README was 100% wrong subject matter (Spin framework). Patching would waste effort and leave inconsistencies.
2. **Feature flags from source-of-truth** — every `--features` flag in the README was read directly from the Cargo.toml `[[example]]` `required-features` line, so no copy-paste failures are possible for existing users.
3. **Literal string count match** — the plan's acceptance criterion demanded that `grep -c "cargo run --example"` equal the number of `.rs` files. The initial draft had 65 matches (63 per-example + 2 in prose templates). Reworded the prose sections so the only occurrences are per-example commands, achieving exact 63 == 63 match.
4. **Migration tables by role** — rather than one flat 63-row table, split into Server (40 rows), Client (7), Transport (8), Middleware (8) for readability.
5. **Standalone subdirectory projects listed separately** — the 11 subdirectories (mcp-apps-*, wasm-*, scenarios, test-basic, 25-oauth-basic, 26-server-tester, 27-course-server-minimal) have their own Cargo.toml and are excluded from the root workspace, so they live in a dedicated table at the bottom with brief descriptions.

## Deviations from Plan

### Prose-level deduplication (Rule 1 — Bug)

**Found during:** Task 1 acceptance verification
**Issue:** Initial README draft had 65 `cargo run --example` occurrences (63 per-example commands + 1 generic template in the Prerequisites section + 1 narrative reference in the Migration Reference paragraph). The plan's verification requires exact equality with the 63 .rs files.
**Fix:** Rewrote both prose sentences to avoid the literal substring without losing user guidance. The Prerequisites section now says "Run any example using the copy-paste command block shown directly under its entry." The Migration Reference paragraph now says "Any old `cargo run` invocations should be updated to the new role-prefixed names."
**Files modified:** examples/README.md
**Commit:** 3b1435a4 (included in the same task commit)

No other deviations.

## Issues Encountered

None.

## User Setup Required

None.

## Known Stubs

None. The README is complete documentation with no placeholder content.

## Next Phase Readiness

- `examples/README.md` now serves as the definitive example reference for the PMCP SDK
- EXMP-01 resolved; the examples-cleanup-protocol-accuracy phase's three EXMP requirements (EXMP-01, EXMP-02, EXMP-03) and PROT-01 are all now complete
- Phase 65 is ready for verification (post-phase `make quality-gate` + verifier agent)

## Self-Check: PASSED

- FOUND: examples/README.md (modified)
- FOUND: commit 3b1435a4 `docs(65-03): replace Spin README with PMCP example index`
- All 6 plan verification checks passed
- Feature flags cross-verified against Cargo.toml

---
*Phase: 65-examples-cleanup-protocol-accuracy*
*Completed: 2026-04-10*
