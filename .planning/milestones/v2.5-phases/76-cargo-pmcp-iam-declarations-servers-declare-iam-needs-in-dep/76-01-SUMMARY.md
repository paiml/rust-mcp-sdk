---
phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
plan: 01
subsystem: cargo-pmcp
tags: [cargo-pmcp, iam, cdk, cfn-output, stack-ts, backward-compat, golden-file, toyota-way]

# Dependency graph
requires:
  - phase: pre-76
    provides: existing `InitCommand::create_stack_ts` two-branch template generator (pmcp-run + aws-lambda)
provides:
  - Stable `McpRoleArn` CfnOutput in both stack.ts template branches (PART-1 of the CR)
  - `pub(doc(hidden)) InitCommand::render_stack_ts(server_name, &IamConfig) -> String` seam for Waves 3/5 to inject rendered iam blocks
  - Zero-sized `IamConfig` stub in `deployment/config.rs` wired into `DeployConfig` via `#[serde(default, skip_serializing_if = "IamConfig::is_empty")]`
  - Byte-identical golden-file guard (`tests/golden/{pmcp-run,aws-lambda}-empty.ts`) enforcing D-05 backward-compat invariant
  - `aws-cdk-lib/aws-iam` import in the aws-lambda branch (D-03 fix)
affects: [76-02-iam-schema, 76-03-render-iam-block, 76-04-validator, 76-05-fuzz-example]

# Tech tracking
tech-stack:
  added: [none — additive changes to existing serde + format! codebase]
  patterns:
    - "Pure-string render_stack_ts extraction: filesystem writer delegates to a pure renderer so tests compare strings directly."
    - "Golden-file with UPDATE_GOLDEN=1 escape hatch: new convention in cargo-pmcp for byte-identical regression tests (first instance in the crate)."
    - "Forward-compatible serde stub: empty struct + #[serde(skip_serializing_if)] lets Wave 2 add fields without breaking D-05 or touching init.rs signatures."

key-files:
  created:
    - cargo-pmcp/tests/backward_compat_stack_ts.rs
    - cargo-pmcp/tests/golden/pmcp-run-empty.ts
    - cargo-pmcp/tests/golden/aws-lambda-empty.ts
    - .planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/deferred-items.md
  modified:
    - cargo-pmcp/src/deployment/config.rs  # IamConfig stub + DeployConfig wiring + iam_wave1_tests
    - cargo-pmcp/src/commands/deploy/init.rs  # render_stack_ts extraction + 2× McpRoleArn + aws-iam import + wave1_stack_ts_tests

key-decisions:
  - "Task 3 pivot (Rule 3 — blocking issue): `InitCommand` and `IamConfig` are not re-exported from `cargo-pmcp/src/lib.rs` (only `loadtest`/`pentest`/`test_support_cache` are). Byte-identical comparison therefore lives in-crate as `wave1_stack_ts_tests::golden_*`; the integration-test file in `tests/` does grep-level invariant checks against the committed goldens. Same D-05 guarantee, no library-surface expansion."
  - "Golden files live at `cargo-pmcp/tests/golden/*.ts`, generated/regenerated via `UPDATE_GOLDEN=1 cargo test -p cargo-pmcp -- golden`. First golden-file pattern in the crate — establishes convention for Waves 2-5."
  - "`IamConfig` is a zero-sized struct in Wave 1 (real fields land in Wave 2). The `is_empty` method is a Wave-1 stub returning `true` unconditionally, wired into `#[serde(skip_serializing_if = \"IamConfig::is_empty\")]` so Wave 2's real check drops in without re-plumbing DeployConfig."
  - "`render_stack_ts` is `pub #[doc(hidden)]` rather than `pub(crate)` so future-wave integration tests could reach it if lib.rs is ever expanded; the current lib surface is unchanged — no new public API. The leading-underscore `_iam: &IamConfig` parameter silences `clippy::unused_variables` in Wave 1; Wave 3 drops the underscore and threads `render_iam_block(iam)` in."

patterns-established:
  - "Golden-file + UPDATE_GOLDEN pattern (new to cargo-pmcp): test file reads committed baseline; env-var escape hatch regenerates on intentional changes."
  - "In-crate test module co-location for private-field types: `wave1_stack_ts_tests` inside `init.rs` drives `InitCommand` without exposing private fields."
  - "Integration-shim pattern for bin-heavy crates: when the lib surface is minimal, integration tests operate on committed artifacts (goldens) rather than calling private APIs."

requirements-completed: [PART-1]

# Metrics
duration: ~35min
completed: 2026-04-23
---

# Phase 76 Plan 01: Wave 1 — `McpRoleArn` CfnOutput + `render_stack_ts` seam + `IamConfig` stub Summary

**Stable `McpRoleArn` role-ARN export lands in both `stack.ts` template branches with backward-compat golden guard, unblocking Waves 2-5 and the operator's cost-coach bolt-on stack.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-23T03:51:00Z (approx)
- **Completed:** 2026-04-23T04:26:13Z
- **Tasks:** 3/3 + 1 fmt fix-up
- **Files modified:** 2 source, 3 test/golden, 1 deferred-items log

## Accomplishments

- **PART-1 of the CR shipped:** `new cdk.CfnOutput(this, 'McpRoleArn', { value: mcpFunction.role!.roleArn, exportName: ... })` now lives in both template branches. Export names: `pmcp-${serverId}-McpRoleArn` (pmcp-run) and `pmcp-${serverName}-McpRoleArn` (aws-lambda). D-01 (use auto-created Lambda role) and D-03 (add missing aws-iam import) both honoured.
- **Single seam for Waves 3/5:** `render_stack_ts` is now a pure-string renderer that future waves inject a rendered iam block into. No more filesystem-dependent tests for CDK template assertions.
- **D-05 backward-compat guard in place:** two committed golden files pin the byte-identical output for the no-`[iam]` case. A single misplaced whitespace in Waves 2-5 now fails CI loud with a clear UPDATE_GOLDEN hint.
- **Forward-compatible serde wiring:** `IamConfig` landed as a zero-sized struct with `is_empty` always-true. Wave 2 replaces the struct body without touching `DeployConfig`'s field declaration, `default_for_server`, or init.rs's render signature.
- **14 new tests green:** 3 `iam_wave1_tests` (serde invariants) + 4 `wave1_stack_ts_tests` (emission) + 2 `golden_*` bootstrap/guard + 5 `backward_compat_stack_ts` integration invariants. Full `cargo test -p cargo-pmcp -- --test-threads=1` reports 649 passed, 7 suites.

## Task Commits

Each task committed atomically:

1. **Task 1: IamConfig stub + render_stack_ts extraction + McpRoleArn emission** — `0d586740` (feat)
2. **Task 2: In-crate wave1_stack_ts_tests for McpRoleArn emission** — `dd3d502d` (test)
3. **Task 3: Golden-file backward-compat guard (in-crate byte-compare + integration grep invariants + committed goldens)** — `da1a1ef5` (test)
4. **Fmt fix-up: cargo fmt whitespace rewrap inside `check_or_update_golden`** — `9a5e6a46` (style)

_Note: TDD was applied task-internally (write test, make it pass, verify); task commits bundle the RED+GREEN together rather than emitting separate `test(...)` and `feat(...)` commits per step because each task's RED would have been trivially compilable-but-failing under the existing code path._

## Files Created/Modified

### Created
- `cargo-pmcp/tests/backward_compat_stack_ts.rs` — integration test file; reads committed goldens, asserts phase-76 invariants (McpRoleArn present, D-01 role value, D-03 aws-iam import, ordering), 5 tests.
- `cargo-pmcp/tests/golden/pmcp-run-empty.ts` — 153-line baseline of the pmcp-run template with IamConfig::default().
- `cargo-pmcp/tests/golden/aws-lambda-empty.ts` — 102-line baseline of the aws-lambda template with IamConfig::default().
- `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/deferred-items.md` — out-of-scope discoveries log (20 pre-existing clippy errors in pentest/, secrets/, deployment/metadata.rs).

### Modified
- `cargo-pmcp/src/deployment/config.rs` — added `IamConfig` struct (30 lines), wired into `DeployConfig` field (4 lines) + `default_for_server` (1 line), appended `iam_wave1_tests` module (42 lines).
- `cargo-pmcp/src/commands/deploy/init.rs` — split `create_stack_ts` into filesystem wrapper + `pub #[doc(hidden)] render_stack_ts` renderer (17 lines), added 2× McpRoleArn CfnOutput emissions (10 lines total) + 1× aws-iam import (1 line), appended `wave1_stack_ts_tests` module (170 lines including 4 emission tests + golden bootstrap/guard infra + 2 golden tests).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Integration test cannot access `InitCommand` / `IamConfig` / `render_stack_ts`**
- **Found during:** Task 3 planning, before writing `tests/backward_compat_stack_ts.rs`.
- **Issue:** The plan instructed the integration test to import `cargo_pmcp::commands::deploy::InitCommand` and `cargo_pmcp::deployment::config::IamConfig`. Neither path is re-exported from `cargo-pmcp/src/lib.rs` (lib exposes only `loadtest`, `pentest`, `test_support_cache` / `test_support` to keep the bin-only OAuth + AWS Secrets Manager + template trees out of the library surface).
- **Fix:** Kept the byte-identical comparison in-crate as `wave1_stack_ts_tests::golden_pmcp_run_stack_ts_empty_iam` and `golden_aws_lambda_stack_ts_empty_iam` (gated on `UPDATE_GOLDEN=1` for regeneration). `tests/backward_compat_stack_ts.rs` now reads the committed golden files from disk and asserts phase-76 grep invariants (McpRoleArn present, D-01 role value, D-03 import, DashboardUrl ordering, files exist). Both the in-crate byte-compare AND the integration-level invariants run on every `cargo test`; D-05 is doubly guarded.
- **Alternative considered:** Expose a minimal `#[doc(hidden)] pub mod test_support_init` in `lib.rs` that re-exports `InitCommand` + `IamConfig`. Rejected because it would transitively require re-exporting `templates::oauth::{authorizer, proxy}` and the rest of the CognitoConfig-heavy `deployment::config` tree — massive surface expansion for one test file, and contrary to the existing lib-surface minimalism.
- **Files modified:** `cargo-pmcp/src/commands/deploy/init.rs` (added in-crate golden tests), `cargo-pmcp/tests/backward_compat_stack_ts.rs` (created — 5 grep-level integration assertions).
- **Commit:** `da1a1ef5`

**2. [Rule 1 — Formatting] cargo fmt whitespace in `check_or_update_golden` panic message**
- **Found during:** Post-Task-3 `cargo fmt --all -- --check`.
- **Issue:** The `unwrap_or_else` closure body for the `create_dir_all` fallback had a line-length rewrap that fmt preferred inline.
- **Fix:** Ran `cargo fmt --all`; whitespace-only delta.
- **Files modified:** `cargo-pmcp/src/commands/deploy/init.rs`.
- **Commit:** `9a5e6a46`

### Extra Tests Added Beyond Plan

- The plan targeted 9 total tests (3 iam_wave1 + 4 wave1_stack_ts + 2 backward_compat). Delivered **14** — extra coverage in the integration file (5 instead of 2: added explicit `goldens_exist_for_both_targets` smoke test, moved the byte-compare assertions into the in-crate module, and expanded the integration file to 4 grep invariants + 1 existence check).

## Verification Results

- `cargo test -p cargo-pmcp -- --test-threads=1` → **649 passed, 7 suites, 9.11s**.
- `cargo test -p cargo-pmcp iam_wave1_tests` → 3 passed.
- `cargo test -p cargo-pmcp wave1_stack_ts_tests` → 6 passed (4 emission + 2 golden).
- `cargo test -p cargo-pmcp --test backward_compat_stack_ts` → 5 passed.
- `cargo build -p cargo-pmcp` → clean; 12 pre-existing warnings in pentest/secrets (unchanged — see deferred-items.md).
- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` → **20 pre-existing errors in pentest/, secrets/, deployment/metadata.rs, deployment/config.rs:494** (all dated 2025-12-15 or earlier, 0 errors in lines touched by this plan). Logged to `deferred-items.md` per scope-boundary rule.

## Golden-file Content Checks (grep)

`cargo-pmcp/tests/golden/pmcp-run-empty.ts`:
- Line 4: `import * as iam from 'aws-cdk-lib/aws-iam';` (pre-existing, preserved)
- Line 147: `new cdk.CfnOutput(this, 'McpRoleArn', {`
- Line 150: `exportName: \`pmcp-\${serverId}-McpRoleArn\`,`
- McpRoleArn positioned AFTER DashboardUrl (role_idx > dashboard_idx).

`cargo-pmcp/tests/golden/aws-lambda-empty.ts`:
- Line 5: `import * as iam from 'aws-cdk-lib/aws-iam';` (NEW — D-03 fix)
- Line 96: `new cdk.CfnOutput(this, 'McpRoleArn', {`
- Line 99: `exportName: \`pmcp-\${serverName}-McpRoleArn\`,`

## Deferred Issues

See `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/deferred-items.md` — 20 pre-existing clippy errors in unrelated files. These will break `make quality-gate` at the phase boundary; flag for Wave 5 or a pre-phase cleanup task. Not fixed in Wave 1 per scope-boundary rule (files not touched by this plan).

## CLAUDE.md / Toyota Way Compliance

- ✅ Unit tests (3 + 4 + 2 = 9 in-crate, plus 5 integration) — exceeds plan target.
- ✅ Zero SATD in new code.
- ✅ Comprehensive rustdoc on `IamConfig`, `IamConfig::is_empty`, `render_stack_ts`, and every new test module.
- ❌ Fuzz testing — deferred to Wave 5 per phase design (IamConfig has no parsing surface in Wave 1; fuzz target lands once the struct has real fields).
- ❌ Property testing — deferred to Wave 5 (no translation rules to property-check in Wave 1).
- ❌ Example demonstrating usage — deferred to Wave 5 per phase design (cost-coach-shaped `.pmcp/deploy.toml` example demonstrates full IamConfig, not the empty stub).
- ❌ `make quality-gate` green — blocked by 20 pre-existing clippy errors in unrelated files (see deferred-items.md). This plan's own files pass `cargo clippy -- -D warnings`.

## Threat Flags

None. Phase 76 Plan 01's `<threat_model>` catalogued T-76-01, T-76-02, T-76-03; this plan only mitigates T-76-01 (role ARN output shape is pinned by tests; no new privilege granted — role was already auto-created by CDK; the CfnOutput exposes a non-secret ARN equivalent to the `ServiceRole`-suffixed outputs CDK synthesises implicitly). T-76-02 and T-76-03 stay `accept`-for-Wave-1 per plan.

## Self-Check: PASSED

- Created files exist:
  - `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/backward_compat_stack_ts.rs` — FOUND
  - `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/golden/pmcp-run-empty.ts` — FOUND
  - `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/golden/aws-lambda-empty.ts` — FOUND
  - `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/deferred-items.md` — FOUND
- Modified files carry Wave-1 markers:
  - `cargo-pmcp/src/deployment/config.rs` contains `pub struct IamConfig` at line 746 — FOUND
  - `cargo-pmcp/src/commands/deploy/init.rs` contains `pub fn render_stack_ts` at line 511 — FOUND
  - `cargo-pmcp/src/commands/deploy/init.rs` contains `McpRoleArn` (4 matches) — FOUND
- Commits in git log:
  - `0d586740` (Task 1) — FOUND
  - `dd3d502d` (Task 2) — FOUND
  - `da1a1ef5` (Task 3) — FOUND
  - `9a5e6a46` (style fix-up) — FOUND
