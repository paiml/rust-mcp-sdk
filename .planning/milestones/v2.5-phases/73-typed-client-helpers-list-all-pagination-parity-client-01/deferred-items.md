# Phase 73 Deferred Items

Out-of-scope issues discovered during execution but NOT caused by phase 73 changes.

## Pre-existing clippy warnings (not caused by Phase 73)

Discovered during 73-01 Task 3 clippy run. All in files untouched by Phase 73:

- `src/error/recovery.rs:229` — `clippy::arithmetic_side_effects` (manual_checked_ops)
- `src/shared/middleware.rs:138` — same lint
- `src/shared/middleware.rs:1258` — same lint
- `src/server/workflow/*` — additional manual_checked_ops hits
- `src/shared/sse_parser.rs:228` — `clippy::collapsible_match`

These are triggered by newer rustc/clippy (1.95.0) lint tightening. The base
worktree branch (`worktree-agent-a0e69ceb`) was cut from an older commit
(`edc16b17`) that pre-dated some pre-commit hook fixes on main. Pre-existing
clippy warnings exist at baseline before Phase 73 changes. Phase 73 new code
(`src/client/options.rs`, typed helpers in `src/client/mod.rs`) contributes
ZERO new warnings.

Remediation plan: these belong to a separate housekeeping phase; the Phase
73 verifier / orchestrator should fold them into a clippy-cleanup follow-up
after wave merge, not block Phase 73.
