---
phase: 77
plan: 04
subsystem: cargo-pmcp/cli
tags: [configure, add, use, credential-validator, workspace-marker, regex, atomic-write]
dependency_graph:
  requires:
    - "77-03 (TargetConfigV1 schema, write_atomic helper, find_workspace_root, GlobalFlags::default, serial_test dev-dep)"
  provides:
    - "configure::add::execute — full add subcommand with name validation, prompt loop, 6-pattern raw-credential rejection"
    - "configure::use_cmd::execute — full use subcommand writing <name>\\n marker file"
    - "configure::use_cmd::read_active_marker(workspace_root) -> Option<String> — BOM/whitespace-tolerant marker reader, consumed by Plan 77-05/77-06 resolver"
  affects:
    - cargo-pmcp/src/commands/configure/add.rs (stub → 384 lines: AddArgs + execute + validators + 8 tests)
    - cargo-pmcp/src/commands/configure/use_cmd.rs (stub → 335 lines: UseArgs + execute + read_active_marker + 8 tests)
tech_stack:
  added: []
  patterns:
    - "Hand-rolled prompt loop (eprint! + io::stderr().flush() + io::stdin().read_line()) — no dialoguer dep per RESEARCH Pitfall §10"
    - "Anchored regex validators (^...$) with explicit credential KIND in error message and never the matched value (T-77-01-A)"
    - "Read-modify-write at the file level: TargetConfigV1::read → mutate → write_atomic — last-writer-wins on concurrent invocation"
    - "Per-test HOME+CWD isolation via #[serial_test::serial] + saved-cwd fallback to env::temp_dir() when prior test corrupts CWD"
key_files:
  created: []
  modified:
    - cargo-pmcp/src/commands/configure/add.rs
    - cargo-pmcp/src/commands/configure/use_cmd.rs
key_decisions:
  - "Single commit per task (test+impl co-located) — matches Plan 03 precedent. Tests live in `#[cfg(test)] mod tests` inside the same file as the implementation; co-locating means a single `feat(...)` commit per task is the minimum useful diff."
  - "saved_cwd fallback to env::temp_dir() — current_dir() can fail when a prior #[serial] test set CWD into a tempdir that was subsequently dropped. tempdir() always exists, so it's a safe default; tests don't depend on the *exact* restore path."
  - "write_config_with_target test helper in use_cmd reads existing config first — not write-from-empty — so the 2-target overwrite tests (use_overwrites_existing_marker, gem2_use_overwrite_emits_switching_note) work without the second call erasing the first target."
  - "validate_target_name duplicated across add.rs and use_cmd.rs (NOT extracted) — Plan 77-09 will consolidate during quality-gate cleanup. Per Plan 77-04 task body 'duplicate the function and rely on Plan 09 to consolidate'."
  - "Compile regex inside the validator loop (not lazy_static / OnceLock) — at most 6×N calls per add, where N=scalar field count; never hot. Avoids new dependency surface; readable inline."
  - "GEM-1 (escape hatch in error): bail!() string explicitly mentions `--allow-credential-pattern` so users can recover from a false positive without grep'ing source."
  - "GEM-2 (overwrite stderr note): when use marker is overwritten with a DIFFERENT target, emit `note: switching active target from '<prev>' to '<new>' in <path>` to stderr (suppressible via --quiet). Plan 77-08 integration tests will assert the literal note string via subprocess + grep stderr."
metrics:
  duration: ~30m
  completed: 2026-04-26
  tasks_completed: 2
  files_modified: 2
  files_created: 0
  tests_added: 16
---

# Phase 77 Plan 04: configure add + configure use Summary

**`configure add` (interactive + flag-driven, 6-pattern raw-credential rejection, name regex `[A-Za-z0-9_-]+`) and `configure use` (workspace marker writer, BOM-tolerant `read_active_marker` helper, GEM-2 switching note on overwrite) — both write-side subcommands of the configure group landed with 16 unit tests passing.**

## Performance

- **Duration:** ~30m
- **Started:** 2026-04-26 (sequential executor on main worktree)
- **Completed:** 2026-04-26
- **Tasks:** 2
- **Files modified:** 2
- **Files created:** 0
- **Tests added:** 16 (8 add + 8 use_cmd)

## Accomplishments

- `cargo pmcp configure add <name>` is now a real subcommand: validates the name against `[A-Za-z0-9_-]+`, builds a `TargetEntry` from flags or interactive prompts, rejects 6 well-known credential patterns at insertion time, and atomically inserts into `~/.pmcp/config.toml`.
- `cargo pmcp configure use <name>` writes a single-line `<name>\n` marker to `<workspace_root>/.pmcp/active-target`. Idempotent on identical names; emits stderr "switching" note on overwrite-with-different-name.
- `read_active_marker(workspace_root) -> Result<Option<String>>` is `pub` and BOM/whitespace-tolerant — exact contract Plan 77-05/77-06's resolver expects.
- 16 unit tests pass with `--test-threads=1`. No clippy errors in the new code (all "never used" warnings are pre-existing because `ConfigureCommand` isn't yet wired to the top-level Cli — Plan 77-07's job).

## Task Commits

Each task was committed atomically (test + implementation co-located in single file, single commit per task — matches Plan 03 precedent):

1. **Task 1: configure add — interactive + flag-driven target creation with credential validator** — `03908c2f` (feat)
2. **Task 2: configure use — write workspace marker file with name validation** — `0d8bb8ee` (feat)

## Files Modified

### Modified (2)

- `cargo-pmcp/src/commands/configure/add.rs` — stub (20 lines) → full impl (384 lines: `AddArgs`, `execute`, `validate_target_name`, `build_entry_from_args_or_prompts`, `optional_field`, `required_field`, `prompt`, `validate_no_raw_credentials`, `collect_scalar_field_values`, 8 tests)
- `cargo-pmcp/src/commands/configure/use_cmd.rs` — stub (20 lines) → full impl (335 lines: `UseArgs`, `execute`, `validate_target_name` (mirrored), `read_active_marker` (pub), 8 tests)

## Decisions Made

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Single commit per task (test + impl) | Matches Plan 03 precedent; tests are in the same file as implementation, so splitting into RED/GREEN commits would be artificial. | 2 commits total instead of 4. |
| `saved_cwd` falls back to `env::temp_dir()` when `current_dir()` fails | Prior `#[serial]` tests can set CWD into a tempdir that's later dropped; subsequent tests then crash on `current_dir().unwrap()`. `temp_dir()` always exists, and tests never depend on the exact restore path. | Eliminated 6/8 use_cmd test failures on first run. |
| `write_config_with_target` reads existing then writes (not write-from-empty) | The 2-target overwrite tests (`use_overwrites_existing_marker`, `gem2_use_overwrite_emits_switching_note`) call this helper twice with different names; without read-modify-write the second call erased the first target, causing "target 'dev' not found" failures. | Both overwrite tests now pass. |
| `validate_target_name` duplicated across add.rs + use_cmd.rs (not extracted) | Per plan body: "duplicate the function and rely on Plan 09 to consolidate during quality-gate cleanup". Avoids speculative module structure for a 14-line helper. | Plan 77-09 will DRY this up. |
| Inline `Regex::new` in validator loop (not OnceLock / lazy_static) | At most 6 patterns × N scalar fields per `add` call (N≤3); never hot. Avoids new dep surface and keeps the validator readable in one place. | No measurable cost; one extra dep avoided. |
| GEM-1: error message mentions `--allow-credential-pattern` literally | 77-REVIEWS.md feedback — escape hatch must be discoverable from the error itself, not from source/docs. | Test `reject_aws_access_key_pattern` asserts the literal string is present. |
| GEM-2: overwrite emits stderr "switching" note (suppressible via --quiet) | 77-REVIEWS.md feedback — silently overwriting an active target is surprising. One-line note keeps it visible without an interactive prompt. | Test `gem2_use_overwrite_emits_switching_note` exercises the code path; Plan 77-08 will subprocess-assert the literal stderr line. |

## Deviations from Plan

None — plan executed as written. Two minor implementation refinements that the plan body anticipated but didn't spell out exactly:

1. **Plan body's helper showed `let saved_cwd = std::env::current_dir().unwrap();`** — fixing a real test failure (cross-test CWD pollution from prior `#[serial]` tests dropping their tempdirs while still being CWD) required a fallback to `env::temp_dir()`. This is a Rule 1 (auto-fix bug in test infra) — the plan's helper as written caused 6 tests to fail; the fallback is the smallest fix that makes the suite green. Documented in Decisions table.
2. **Plan body's `write_config_with_target` test helper called `TargetConfigV1::empty()` unconditionally** — this erased the first target when the helper was called twice in a single test (overwrite test scenarios). Switched to read-modify-write. Same Rule 1 classification.

Both fixes are isolated to test code — no behavior change in production add/use code paths.

## Issues Encountered

- **Cross-test CWD corruption** (described above) — fixed by fallback to `env::temp_dir()` and ignoring `set_current_dir` errors during cleanup. Production code is unaffected; only the test helper.
- **Pre-existing "never used" clippy warnings** in the configure module (carried over from Plan 03) — `ConfigureCommand`, `add::execute`, `use_cmd::execute`, `read_active_marker`, etc. are reported as never used because the configure subcommand is not yet wired into the top-level Cli. This is by design — Plan 77-07 will register the Configure variant and dispatch arm. Verified pre-existing via `git stash` baseline.

## Verification Results

| Check | Result |
|---|---|
| `cargo build -p cargo-pmcp` | exit 0 (only pre-existing pentest dead-code warnings) |
| `cargo test -p cargo-pmcp --bins commands::configure::add::tests` | 8/8 passed |
| `cargo test -p cargo-pmcp --bins commands::configure::use_cmd::tests -- --test-threads=1` | 8/8 passed |
| `cargo test -p cargo-pmcp --bins commands::configure -- --test-threads=1` (full configure suite) | 28/28 passed (8 add + 8 use_cmd + 10 config + 2 workspace) |
| `cargo clippy -p cargo-pmcp` | exit 0; 0 errors; only pre-existing "never used" warnings (carried from Plan 03) |
| `grep -c "pub fn execute" configure/add.rs` | 1 ✓ |
| `grep -c "validate_no_raw_credentials" configure/add.rs` | 6 ✓ (≥2 required) |
| `grep -c 'AKIA\[0-9A-Z\]{16}' configure/add.rs` | 1 ✓ |
| `grep -c "ghp_" configure/add.rs` | 2 ✓ (regex + test fixture) |
| `grep -c "\-\-allow-credential-pattern" configure/add.rs` | 4 ✓ (≥1 required for GEM-1) |
| `grep -c "validate_target_name" configure/add.rs` | 9 ✓ (≥2 required) |
| `grep -c "pub fn execute" configure/use_cmd.rs` | 1 ✓ |
| `grep -c "pub fn read_active_marker" configure/use_cmd.rs` | 1 ✓ |
| `grep -c "find_workspace_root" configure/use_cmd.rs` | 3 ✓ (≥1 required) |
| `grep -c "validate_target_name" configure/use_cmd.rs` | 2 ✓ (≥2 required) |
| 6 raw-credential patterns exercised | AWS access key (AKIA), AWS temp session (ASIA — covered by regex), GitHub PAT (ghp_), GitHub fine-grained PAT (github_pat_), Stripe live (sk_live_), Google API key (AIza) — all 6 in regex table; 3 explicitly tested (AKIA, ghp_, sk_live_) |

## Threat Surface Compliance

The plan's `<threat_model>` flagged 4 threats. All plan-specified mitigations landed:

| Threat | Mitigation Result |
|---|---|
| T-77-01 (raw credentials in config.toml) | `validate_no_raw_credentials` rejects 6 anchored regex patterns; `--allow-credential-pattern` provides explicit override; rejection error names credential KIND + lists 3 reference alternatives (AWS profile, env-var name, Secrets Manager ARN) |
| T-77-03 (path-traversal in target name) | `validate_target_name` enforces `[A-Za-z0-9_-]+`; rejects empty / leading-dash / non-ASCII-alphanumeric-dash-underscore characters; `name_validation_rejects_path_traversal` test covers `../foo`, `foo/bar`, `-foo`, empty |
| T-77-06 (concurrent add corruption) | Inherited from Plan 03 — `tempfile::NamedTempFile::persist` does atomic rename in `write_atomic`; concurrent writers are last-writer-wins, never partial |
| T-77-01-A (stderr leak of credential value) | Error message includes credential KIND (e.g. "AWS access key") but NEVER the matched value itself; `bail!` format string only interpolates `kind`, never `value` |

No new threat surface introduced beyond what the plan anticipated.

## Next Phase Readiness

Plans 77-05 (`configure list` + `configure show`) and 77-06 (resolver + banner) are unblocked:

- `configure add` and `configure use` work end-to-end against `~/.pmcp/config.toml` and `<workspace>/.pmcp/active-target`.
- `read_active_marker(&Path) -> Result<Option<String>>` is `pub` and ready for Plan 77-05's `show` subcommand and Plan 77-06's resolver.
- `validate_target_name` duplication is documented — Plan 77-09 will consolidate.

## Self-Check: PASSED

**Files verified:**

- `[ -f cargo-pmcp/src/commands/configure/add.rs ]` → FOUND (384 lines)
- `[ -f cargo-pmcp/src/commands/configure/use_cmd.rs ]` → FOUND (335 lines)

**Commits verified in `git log --oneline`:**

- `03908c2f` (Task 1) → FOUND
- `0d8bb8ee` (Task 2) → FOUND

---
*Phase: 77-cargo-pmcp-configure-commands*
*Plan: 04*
*Completed: 2026-04-26*
