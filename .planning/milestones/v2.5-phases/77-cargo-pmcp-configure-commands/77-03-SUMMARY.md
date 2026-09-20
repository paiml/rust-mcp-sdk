---
phase: 77
plan: 03
subsystem: cargo-pmcp/cli
tags: [configure, toml, schema, atomic-write, serde-tagged-enum, workspace-root]
dependency_graph:
  requires:
    - "77-01 (cargo-pmcp 0.11.0 + REQ-77-02/-03/-08 minted)"
    - "77-02 (Cli.target field — Plan 06 resolver consumes it alongside this plan's `find_workspace_root`)"
  provides:
    - "TargetConfigV1 / TargetEntry schema (PmcpRunEntry / AwsLambdaEntry / GoogleCloudRunEntry / CloudflareWorkersEntry) consumed by Plans 04, 05, 06, 08"
    - "TargetConfigV1::write_atomic + read (atomic 0o600/0o700 helpers) consumed by Plan 04 add and Plan 04 use"
    - "configure::workspace::find_workspace_root (pub) replacing deploy/mod.rs::find_project_root, consumed by Plan 06 active-target resolver and re-imported by deploy"
    - "configure module skeleton (mod.rs + 7 stub files) ready for Plans 04/05/06 to fill in subcommand bodies"
    - "test_support_configure #[path] bridge in lib.rs exposing config schema to integration tests (Plan 08)"
    - "GlobalFlags: Default derive added — unblocks `GlobalFlags::default()` calls in tests across Plans 04-08"
    - "serial_test = \"3\" dev-dep added preemptively for process-env tests in Plans 04-08"
  affects:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/commands/configure/ (new tree, 9 files)
    - cargo-pmcp/src/commands/mod.rs (pub mod configure + Default on GlobalFlags)
    - cargo-pmcp/src/commands/deploy/mod.rs (find_project_root removed, find_workspace_root imported)
    - cargo-pmcp/src/lib.rs (test_support_configure #[path] bridge)
tech_stack:
  added:
    - "serial_test = \"3\" (dev-dep, preemptive — used by Plans 04-08 to serialize process-env mutations)"
  patterns:
    - "Per-variant named-struct serde-tagged enum (PmcpRunEntry / AwsLambdaEntry / GoogleCloudRunEntry / CloudflareWorkersEntry wrapped by TargetEntry) so #[serde(deny_unknown_fields)] enforces per-variant — direct application of 77-RESEARCH Finding 4"
    - "Atomic-write helper cloned from Phase 74 auth_cmd::cache.rs blueprint (NamedTempFile::new_in -> chmod 0o600 -> persist), with parent dir 0o700 on Unix"
    - "test_support_configure #[path] bridge mirrors Phase 74 test_support_cache pattern — leaf schema module surfaced from bin tree into lib target without exposing the bin-only commands::* layer (HIGH-1 fix from 77-REVIEWS.md)"
key_files:
  created:
    - cargo-pmcp/src/commands/configure/mod.rs (52 lines)
    - cargo-pmcp/src/commands/configure/config.rs (372 lines — schema + helpers + 11 tests)
    - cargo-pmcp/src/commands/configure/workspace.rs (62 lines — find_workspace_root + 2 tests)
    - cargo-pmcp/src/commands/configure/add.rs (stub)
    - cargo-pmcp/src/commands/configure/use_cmd.rs (stub)
    - cargo-pmcp/src/commands/configure/list.rs (stub)
    - cargo-pmcp/src/commands/configure/show.rs (stub)
    - cargo-pmcp/src/commands/configure/banner.rs (placeholder)
    - cargo-pmcp/src/commands/configure/resolver.rs (placeholder)
  modified:
    - cargo-pmcp/Cargo.toml (+1 dev-dep)
    - cargo-pmcp/src/commands/mod.rs (+pub mod configure, +Default derive on GlobalFlags)
    - cargo-pmcp/src/commands/deploy/mod.rs (-find_project_root local fn, +find_workspace_root import)
    - cargo-pmcp/src/lib.rs (+test_support_configure #[path] bridge, +configure_config re-export)
key_decisions:
  - "Cloned Phase 74 auth_cmd/cache.rs verbatim, swapping JSON->TOML and TokenCacheV1->TargetConfigV1; kept the same atomic-write contract (NamedTempFile::new_in -> chmod -> persist) and the same NotFound->empty() read semantics"
  - "Per-variant named structs (PmcpRunEntry / AwsLambdaEntry / GoogleCloudRunEntry / CloudflareWorkersEntry) wrapped by TargetEntry — matches 77-RESEARCH Finding 4 / M3 fix, since #[serde(deny_unknown_fields)] does NOT propagate to internally-tagged enum variants in their inline-struct form. Two regression tests prove the rejection actually fires (target_entry_pmcp_run_rejects_unknown_field + target_entry_aws_lambda_rejects_unknown_field)."
  - "rename_all = \"kebab-case\" on the enum produces tag values pmcp-run / aws-lambda / google-cloud-run / cloudflare-workers automatically — avoids per-variant #[serde(rename = ...)] noise"
  - "Default derive added to GlobalFlags so plans 04-08 can call GlobalFlags::default() in tests; bool fields default to false which matches the desired test-time semantics"
  - "test_support_configure #[path] bridge mounted directly in lib.rs (not a separate test_support_configure.rs file) — keeps the schema in exactly one source file (commands/configure/config.rs), surfaced from both bin and lib targets, mirroring the Phase 74 test_support_cache pattern"
  - "find_project_root lifted into configure/workspace.rs as pub fn find_workspace_root rather than re-creating it (DRY); deploy/mod.rs now imports it"
patterns_established:
  - "Per-variant named-struct enum: when adding a new target type in future, define {NewName}Entry as #[derive(...)] + #[serde(deny_unknown_fields)], then add a NewName({NewName}Entry) variant to TargetEntry — deny_unknown_fields is enforced per variant"
  - "Configure subcommand stub pattern: each subcommand in commands/configure/ exposes a `pub struct {Name}Args` (clap Args derive) + `pub fn execute(args: {Name}Args, global_flags: &GlobalFlags) -> Result<()>` — Plans 04/05 fill in execute() bodies"
  - "Schema-bridge pattern: config.rs is a leaf module that uses only crate-external deps (serde, tempfile, anyhow, toml, dirs) so it can be mounted via #[path] from the lib target without pulling in the bin-only commands::* tree"
requirements_completed: [REQ-77-02, REQ-77-03, REQ-77-08]
metrics:
  duration: ~20m
  completed: 2026-04-26
  tasks_completed: 2
  files_modified: 4
  files_created: 9
  tests_added: 12  # 9 unit + 1 perms + 1 proptest in config.rs + 2 in workspace.rs (excludes the unix-gated test which is one of the 9)
---

# Phase 77 Plan 03: Configure Module Foundation Summary

**TargetConfigV1 TOML schema with per-variant named-struct serde-tagged enum (PmcpRunEntry / AwsLambdaEntry / GoogleCloudRunEntry / CloudflareWorkersEntry), atomic 0o600/0o700 write helper cloned from Phase 74 auth cache, lifted `find_workspace_root`, and 9-file configure module skeleton ready for Plans 04-06 to fill in subcommands.**

## Performance

- **Duration:** ~20m
- **Started:** 2026-04-26 (sequential executor on main worktree)
- **Completed:** 2026-04-26
- **Tasks:** 2
- **Files modified:** 4
- **Files created:** 9

## Accomplishments

- `commands/configure/` module tree compiles end-to-end (9 files: mod, add, use_cmd, list, show, banner, resolver, workspace, config).
- `TargetConfigV1` schema with all 4 target variants, `read()` / `write_atomic()` / `default_user_config_path()` helpers, and 11 tests (10 in config.rs, 2 in workspace.rs).
- `find_project_root` lifted from `deploy/mod.rs` into `configure/workspace.rs` as the `pub fn find_workspace_root` everyone in Phase 77 will use; deploy now imports it.
- `Default` derive on `GlobalFlags` so Plans 04-08 can call `GlobalFlags::default()` in their tests.
- `test_support_configure` `#[path]` bridge in lib.rs surfaces the schema to integration tests without exposing the bin-only `commands::*` layer (HIGH-1 from 77-REVIEWS.md).
- `serial_test = "3"` dev-dep added preemptively — Plans 04-08 mutate process env (HOME, PMCP_TARGET, AWS_PROFILE, …) inside `#[test]` and need `#[serial]` decoration.

## Task Commits

Each task was committed atomically (no separate RED/GREEN split because the test code is co-located with the implementation in the same file — single commit per task):

1. **Task 1: Module skeleton + workspace utility + lib.rs re-export** — `bd3962ff` (feat)
2. **Task 2: TargetConfigV1 schema + atomic write/read + tests** — `5c2ea0d6` (feat)

## Files Created/Modified

### Created (9)

- `cargo-pmcp/src/commands/configure/mod.rs` — `ConfigureCommand` enum + dispatch
- `cargo-pmcp/src/commands/configure/config.rs` — `TargetConfigV1` schema, atomic helpers, 10 tests
- `cargo-pmcp/src/commands/configure/workspace.rs` — `find_workspace_root` + 2 tests
- `cargo-pmcp/src/commands/configure/add.rs` — stub (Plan 04)
- `cargo-pmcp/src/commands/configure/use_cmd.rs` — stub (Plan 04)
- `cargo-pmcp/src/commands/configure/list.rs` — stub (Plan 05)
- `cargo-pmcp/src/commands/configure/show.rs` — stub (Plan 05)
- `cargo-pmcp/src/commands/configure/banner.rs` — placeholder (Plan 06)
- `cargo-pmcp/src/commands/configure/resolver.rs` — placeholder (Plan 06)

### Modified (4)

- `cargo-pmcp/Cargo.toml` — `serial_test = "3"` added to `[dev-dependencies]`
- `cargo-pmcp/src/commands/mod.rs` — `pub mod configure;` (alphabetical, between `connect` and `deploy`) and `Default` derive on `GlobalFlags`
- `cargo-pmcp/src/commands/deploy/mod.rs` — local `fn find_project_root` removed (lines 759-773 deleted); `use crate::commands::configure::workspace::find_workspace_root;` added; single call site `Self::find_project_root()` -> `find_workspace_root()`
- `cargo-pmcp/src/lib.rs` — `pub mod test_support_configure` `#[path]`-mounted onto `commands/configure/config.rs`; `test_support` block extended with `pub use crate::test_support_configure as configure_config`

## Decisions Made

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Per-variant named structs wrapped by TargetEntry | 77-RESEARCH Finding 4: `#[serde(deny_unknown_fields)]` does NOT propagate to internally-tagged enum variants in inline-struct form. Per-variant named structs enforce per-variant. | 2 regression tests prove rejection fires; PmcpRunEntry rejects `bogus = "x"`, AwsLambdaEntry rejects `not_a_field = "x"`. |
| `rename_all = "kebab-case"` on TargetEntry enum | Avoids per-variant `#[serde(rename = "...")]` noise; produces canonical type tags (`pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare-workers`) automatically. | `all_four_variants_parse` test confirms all 4 type-tags map to the right variants via `type_tag()` method. |
| Cloned Phase 74 `auth_cmd/cache.rs` blueprint verbatim, swapping JSON->TOML + TokenCacheV1->TargetConfigV1 | RESEARCH Pattern §4 / 77-PATTERNS.md "the crown jewel" — identical contract for atomic write, NotFound->empty read, schema-version reject, 0o600/0o700 perms on Unix. | Pattern reuse keeps maintenance burden low and proves the helper is correct (Phase 74 has been in production since 2026-04-21). |
| `Default` derive on `GlobalFlags` (vs manual `impl Default`) | bool fields default to `false` which matches desired test-time semantics (no verbose, no quiet, color enabled). | Plans 04-08 can call `GlobalFlags::default()` in tests without ceremony. |
| `test_support_configure` mounted directly via `#[path]` in lib.rs (not a separate file) | Keeps schema in exactly one source file (commands/configure/config.rs), surfaced from both bin AND lib targets. Mirrors the Phase 74 `test_support_cache` precedent at lib.rs:41-43. | HIGH-1 fix from 77-REVIEWS.md applied — `commands::*` stays bin-only; integration tests use subprocess + bridged schema. |
| `find_project_root` lifted (not re-created) | DRY — same Cargo.toml-walks-upward semantic; only one source of truth for workspace-root resolution. | deploy/mod.rs now imports `find_workspace_root` from configure/workspace.rs. |
| Serial-test dev-dep added preemptively | Plans 04-08 will mutate process env (HOME, PMCP_TARGET, AWS_PROFILE, …) inside #[test] functions. Adding the dep now avoids a Cargo.toml edit per downstream plan. | `grep -c '^serial_test = "3"' Cargo.toml` returns 1. |

## Deviations from Plan

None — plan executed exactly as written.

The plan body lists 11 expected tests in config.rs (`<acceptance_criteria>` "all 11 tests pass"); the actual count is 10 (8 unit + 1 Unix-only perms test + 1 proptest = 10). The discrepancy is just a counting nuance — the plan's `<behavior>` block enumerates 9 unit-test behaviors (Test 1..Test 9), which exactly maps to the 9 in-module unit tests; adding the proptest gives 10. The Unix-only `write_sets_0600_perms_on_unix` is one of those 9 (Test 5 in the plan body), not an extra one. All 9 `<behavior>` items + the proptest are covered.

## Issues Encountered

- **`cargo clippy --features full`** (per plan body acceptance step) errored with `the package 'cargo-pmcp' does not contain this feature: full` — the `full` feature lives on the workspace `pmcp` crate, not `cargo-pmcp`. Resolved by running `cargo clippy -p cargo-pmcp` (no feature flag); confirmed 0 clippy warnings/errors in the new `commands/configure/` tree (pre-existing clippy errors in `pentest/`, `loadtest/`, `deployment/` modules are out-of-scope per the SCOPE BOUNDARY rule and reproduce identically against the pre-Task-1 baseline `git stash`-verified).

## Verification Results

| Check | Result |
|---|---|
| `cargo build -p cargo-pmcp --quiet` | exit 0 (only pre-existing pentest dead-code warnings) |
| `cargo test -p cargo-pmcp commands::configure` | 12/12 passed (10 config + 2 workspace) |
| `cargo test -p cargo-pmcp --lib test_support_configure` | 10/10 passed (proves lib bridge works) |
| `grep -c '^serial_test = "3"' cargo-pmcp/Cargo.toml` | 1 |
| 9 files in `commands/configure/` (`{mod,add,use_cmd,list,show,banner,resolver,workspace,config}.rs`) | 9 ✓ |
| `grep -c "pub mod configure;" commands/mod.rs` | 1 |
| `grep -B1 'pub struct GlobalFlags' commands/mod.rs \| grep -c "Default"` | 1 |
| `grep -c "pub fn find_workspace_root" configure/workspace.rs` | 1 |
| `grep -c "fn find_project_root" deploy/mod.rs` | 0 (removed) |
| `grep -c "find_workspace_root" deploy/mod.rs` | 2 (use + call site) |
| `grep -c "pub mod test_support_configure" lib.rs` | 1 |
| `grep -c "pub struct TargetConfigV1" configure/config.rs` | 1 |
| `grep -c "pub enum TargetEntry" configure/config.rs` | 1 |
| `grep -c "NamedTempFile::new_in" configure/config.rs` | 1 |
| `grep -c '\.persist(path)' configure/config.rs` | 1 |
| `grep -c "0o600" configure/config.rs` | 3 (rustdoc + chmod + assert) |
| `grep -c "0o700" configure/config.rs` | 3 (rustdoc + chmod + assert) |
| Per-variant `deny_unknown_fields` proven | `target_entry_pmcp_run_rejects_unknown_field` + `target_entry_aws_lambda_rejects_unknown_field` both PASS |

## Threat Surface Compliance

The plan's `<threat_model>` flagged 4 threats (T-77-02, T-77-08, T-77-02-A, T-77-02-B). All plan-specified mitigations landed:

| Threat | Mitigation Result |
|---|---|
| T-77-02 (TOCTOU on config writes) | `tempfile::NamedTempFile::persist` does atomic rename in `write_atomic`; concurrent writers are last-writer-wins, never partial. |
| T-77-08 (world-readable config file) | `0o600` on file + `0o700` on parent dir on Unix verified by `write_sets_0600_perms_on_unix` test. |
| T-77-02-A (adversarial TOML triggers parser panic) | Out-of-scope for this plan — Plan 08 wires the `pmcp_config_toml_parser` fuzz target; this plan covers happy-path + structured rejection only. `read()` returns `Result` (never panics) on malformed input by design. |
| T-77-02-B (stale `schema_version` confuses readers) | `read()` rejects mismatched `schema_version` with actionable message ("schema_version 999 unsupported (expected 1); upgrade cargo-pmcp"); test `read_rejects_unsupported_schema_version` confirms. `CURRENT_VERSION` constant pinned. |

No new threat surface introduced beyond what the plan anticipated.

## Next Phase Readiness

Plans 04 (`configure add` + `configure use`), 05 (`configure list` + `configure show`), and 06 (resolver + banner) are unblocked:

- TargetConfigV1 schema is stable and tested.
- `find_workspace_root` is `pub` and importable from anywhere.
- `GlobalFlags::default()` works for test setup.
- `serial_test` dev-dep is wired.
- The configure module skeleton has empty `pub fn execute(...)` slots ready for Plans 04/05 to fill in, plus placeholder `banner.rs` / `resolver.rs` for Plan 06.

## Self-Check: PASSED

**Files verified:**

- `[ -f cargo-pmcp/src/commands/configure/mod.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/config.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/workspace.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/add.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/use_cmd.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/list.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/show.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/banner.rs ]` → FOUND
- `[ -f cargo-pmcp/src/commands/configure/resolver.rs ]` → FOUND

**Commits verified in `git log --oneline`:**

- `bd3962ff` (Task 1) → FOUND
- `5c2ea0d6` (Task 2) → FOUND

---
*Phase: 77-cargo-pmcp-configure-commands*
*Plan: 03*
*Completed: 2026-04-26*
