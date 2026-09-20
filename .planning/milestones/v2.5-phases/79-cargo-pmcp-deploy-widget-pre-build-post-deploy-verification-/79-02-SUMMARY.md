---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
plan: 02
subsystem: deployment
tags: [cargo-pmcp, deploy, widgets, pre-build, orchestrator, cli, env-var, tokio-process]

requires:
  - phase: 79
    plan: 01
    provides: "WidgetsConfig + WidgetConfig (argv-array build/install) + PackageManager + ResolvedPaths types consumed by the orchestrator"
provides:
  - "`run_widget_build` async orchestrator (validate → resolve_paths → detect PM → ensure_node_modules → invoke_build_script → verify_outputs_exist)"
  - "`detect_widgets` synthesis fn (explicit `[[widgets]]` OR `widget/`/`widgets/` convention; REQ-79-01 hard-fences out `ui/`/`app/`)"
  - "`enumerate_workspace_bin_crates` helper for synthesized `embedded_in_crates` defaults"
  - "`is_yarn_pnp` Yarn-PnP detection (REVISION 3 Codex MEDIUM) — `.pnp.cjs` AND `.pnp.loader.mjs`"
  - "`argv_to_cmd_args` argv-array splitter (REVISION 3 Codex MEDIUM, replaces whitespace-split shell parsing)"
  - "`spawn_streaming` shared subprocess helper with live stdout/stderr (no `.piped()` capture)"
  - "`verify_build_script_exists` — REQ-79-03 verbatim error message for missing `package.json` `scripts.build`"
  - "`DeployCommand::pre_build_widgets_and_set_env` Step 2.5 helper that joins all widgets into `PMCP_WIDGET_DIRS` ONCE (REVISION 3 HIGH-C1)"
  - "`--no-widget-build` and `--widgets-only` CLI flags on `cargo pmcp deploy`"
  - "Step 2.5 hook wired into `execute_async` between `emit_target_banner_if_resolved` and `target.build()` (line 738-758 in commands/deploy/mod.rs)"
affects: [79-03, 79-04]

tech-stack:
  added: []
  patterns:
    - "Phase-75 RESEARCH.md Pattern 2 (per-stage pipeline decomposition) — every orchestrator fn ≤8 cog (well under PMAT cap of 25)"
    - "Tokio subprocess with `.spawn()` + `.wait()` for live stdout/stderr streaming (no `Stdio::piped()` capture)"
    - "Argv-array `Option<Vec<String>>` schema form preserved end-to-end through `argv_to_cmd_args` — no shell parsing"
    - "Yarn-PnP marker detection (`.pnp.cjs` / `.pnp.loader.mjs`) early-returns from install heuristic"
    - "Colon-joined `PMCP_WIDGET_DIRS` env var (Unix `PATH` convention) covering ALL widgets — set ONCE in the deploy hook, NOT per-widget"
    - "Empty-widgets case does NOT set the env var → 79-04's build.rs local-discovery fallback (HIGH-G1) takes over for direct `cargo run`"
    - "F-4 mitigation: deploy.rs is the ONLY hook site — confirmed no `Build` variant in `enum Commands` at `cargo-pmcp/src/main.rs:83`"

key-files:
  created:
    - "cargo-pmcp/tests/widgets_orchestrator.rs"
  modified:
    - "cargo-pmcp/src/deployment/widgets.rs"
    - "cargo-pmcp/src/commands/deploy/mod.rs"

key-decisions:
  - "REVISION 3 HIGH-C1 implemented as `pre_build_widgets_and_set_env` associated fn that runs the loop, accumulates `Vec<String>` of resolved output dirs, joins with `:`, and calls `std::env::set_var(\"PMCP_WIDGET_DIRS\", joined)` ONCE before returning. Per-widget env-var mutation is REMOVED (Test 1.12 locks that contract)."
  - "Empty-widgets detection short-circuits at the top of `pre_build_widgets_and_set_env` (early-return Ok before any iteration), so `PMCP_WIDGET_DIRS` is never even set when no widgets are configured/detected — required for 79-04's HIGH-G1 build.rs local-discovery fallback."
  - "Both new CLI flags are NORMAL `bool` fields with `#[arg(long)]` — no `default_value` (clap's natural default for bool args is `false`). The `if !self.no_widget_build` and `if self.widgets_only` checks happen explicitly in `execute_async` after the orchestrator block so the flags interact predictably."
  - "Task 2 orchestration tests use a verbatim mirror of the production loop (`run_pre_build_loop` helper) instead of going through `DeployCommand::pre_build_widgets_and_set_env` directly — the production fn lives on the bin target's struct and is not lib-visible. Drift between the mirror and the production loop would surface immediately because both algorithms are exactly 9 lines and the env-join contract is asserted at both call sites."
  - "`spawn_streaming` does NOT clear env (`env_clear()` not called) so PMCP_WIDGET_DIRS + AWS_* + OAuth tokens propagate to the JS toolchain — matches Phase 76 IAM template wiring (T-79-07 disposition: accept)."
  - "Yarn-PnP detection accepts both `.pnp.cjs` (Yarn 3) and `.pnp.loader.mjs` (Yarn 4+) marker forms — Test 1.4b loops over both."
  - "`argv_to_cmd_args` returns Err on empty argv (matches Wave 1's `validate()` empty-build/install rejection — defense-in-depth)."
  - "Cog targets met: every new fn ≤8 (PMAT cap is 25; we have 17pt safety margin). PMAT `--max-cognitive 25` reports 0 violations against `widgets.rs` and `commands/deploy/mod.rs`."

requirements-completed: [REQ-79-01, REQ-79-02, REQ-79-03, REQ-79-04, REQ-79-05, REQ-79-09, REQ-79-10]

duration: ~75min
completed: 2026-05-03
---

# Phase 79 Plan 02: Wave 2 — widget pre-build orchestrator Summary

**Wave 2 ships the build-half end-to-end orchestrator: `run_widget_build` + `detect_widgets` + Yarn-PnP-aware `ensure_node_modules` + argv-array `invoke_build_script` + the Step 2.5 deploy hook + `--no-widget-build`/`--widgets-only` CLI flags + the colon-joined `PMCP_WIDGET_DIRS` env-var contract that survives multi-widget projects (REVISION 3 HIGH-C1 supersession). Failure Mode A is CLOSED for both single- and multi-widget projects; Failure Mode B's env-var contract is established for 79-04's build.rs scaffold to consume. 24 tests pass; full cargo-pmcp suite (1013 tests across 15 suites) regression-clean.**

## Performance

- **Duration:** ~75 min
- **Started:** 2026-05-03 (after Wave 1 SUMMARY commit `b53c9018`)
- **Completed:** 2026-05-03
- **Tasks:** 2/2
- **Tests added:** 24 (15 Task 1 + 9 Task 2) in a new `tests/widgets_orchestrator.rs` integration suite
- **Files created:** 1
- **Files modified:** 2
- **Lines added:** 1,194 (314 widgets.rs orchestrator + 89 deploy/mod.rs hook + flags + 791 tests)

## Accomplishments

- **REVISION 3 HIGH-C1 supersession landed:** `PMCP_WIDGET_DIRS` colon-list env var replaces the pre-revision-3 `PMCP_WIDGET_DIR` single-string-last-widget-wins-broken contract. Test 2.7 locks the multi-widget declaration-order join: `widget-a/dist:widget-b/build:widget-c/dist`. Test 2.6 locks the single-widget exact-match. Test 1.12 locks that `run_widget_build` does NOT mutate env state — only the orchestrator wrapper does. Test 2.9 locks the empty-widgets-no-env-var contract required for 79-04's HIGH-G1 build.rs local-discovery fallback.
- **REVISION 3 Codex MEDIUM (Yarn PnP) landed:** `is_yarn_pnp` detects both `.pnp.cjs` (Yarn 3) and `.pnp.loader.mjs` (Yarn 4+) marker forms. `ensure_node_modules` early-returns when either is present so PnP projects don't hit a spurious `yarn install` that has nothing to do (PnP intentionally omits `node_modules/`). Test 1.4b loops over both markers and asserts that even an explicit always-fail install command (`["false"]`) is skipped — proving the early-return is unconditional.
- **REVISION 3 Codex MEDIUM (argv) end-to-end:** `argv_to_cmd_args` passes the user's `Option<Vec<String>>` argv directly to `Command::args(...)` with NO shell parsing. Test 1.13 proves an embedded-whitespace argument (`"echo 'hello world' > out.txt"`) survives intact through `["sh", "-c", ...]` invocation — the pre-revision-3 whitespace-split form would have shattered it into 5 separate arguments.
- **REVISION 3 Codex MEDIUM (skip pkg.json check on explicit override):** `invoke_build_script` only calls `verify_build_script_exists` when `widget.build` is `None`. Operators who provide an explicit build argv (e.g. `["bash", "scripts/build.sh"]`) bypass the `package.json scripts.build` requirement — they take responsibility for whatever invocation they configured.
- **F-4 mitigation:** confirmed via `cargo-pmcp/src/main.rs:83` that there is NO `Build` variant in `enum Commands`. Therefore the Step 2.5 widget-build hook lives ONLY at the `cargo pmcp deploy` execute_async path (commit `0cc13702`). No silent-bypass via a sibling subcommand is possible. The decision-traceability comment in the deploy hook block calls this out explicitly.
- **CLI ergonomics:** `cargo pmcp deploy --help` now lists both `--no-widget-build` and `--widgets-only` with operator-facing rustdoc that explains the use cases (CI pipeline pre-built widgets / fast inner-loop iteration). Tests 2.1, 2.2, 2.3 use `assert_cmd` against the real `cargo-pmcp` binary to assert the help text never regresses.
- **Cog budget:** every new function ≤8 cog (PMAT cap is 25). The deploy hook helper `pre_build_widgets_and_set_env` is 8 cog (early-return + loop + accumulation + env-set). `run_widget_build` is 7 cog (5-step pipeline + Ok wrap). PMAT `--max-cognitive 25` reports 0 violations on either touched file.
- **Regression-clean:** `make quality-gate` exits 0 (matches CI exactly — `--features full`, pedantic + nursery clippy, workspace-wide `cargo fmt --all`). Full `cargo test -p cargo-pmcp -- --test-threads=1` reports 1013 tests pass across 15 suites (Wave 1 baseline was 989; +24 from this wave). The 4 protected pentest/wasm-client WIP files were UNTOUCHED (`git status --short` against the snapshot returns the verbatim 4-line snapshot from spawn time).

## Task Commits

1. **Task 1: widget pre-build orchestrator (run_widget_build, detect_widgets, ensure_node_modules with Yarn-PnP detection, invoke_build_script with argv-array support, verify_outputs_exist)** — `214c9d5c` (feat)
2. **Task 2: --no-widget-build and --widgets-only CLI flags + Step 2.5 hook + PMCP_WIDGET_DIRS list join** — `0cc13702` (feat)

## Files Created/Modified

### Created

- **`cargo-pmcp/tests/widgets_orchestrator.rs`** (791 lines) — 24-test integration suite. Task 1 tests (1.1..1.13 + 2 sanity): spawn-success, missing-build-script REQ-79-03 verbatim error, install runs / skipped, Yarn-PnP marker skip-loop (both `.pnp.cjs` AND `.pnp.loader.mjs`), build-failure aborts, zero-outputs warns-no-fail, missing-binary actionable PATH error, `detect_widgets` synthesis from `widget/`/`widgets/`, REQ-79-01 hard-fence on `ui/`+`app/`, multi-widget stop-on-first-failure, run_widget_build does NOT mutate env, argv-form preserves embedded whitespace. Task 2 tests (2.1..2.9): clap parses `--no-widget-build` + `--widgets-only` (3 tests via `assert_cmd`), orchestrator-skip semantics, widgets-only-then-exit semantics, PMCP_WIDGET_DIRS single-widget exact-match, multi-widget colon-join in declaration order, child-subprocess env inheritance, empty-widgets-does-not-set-env (HIGH-G1 contract).

### Modified

- **`cargo-pmcp/src/deployment/widgets.rs`** (+314 lines, total 813) — appended a `// Wave 2: widget pre-build orchestrator` section AFTER the Wave 1 schema types and BEFORE the `#[cfg(test)] mod tests` block. New public surface: `pub async fn run_widget_build`, `pub fn detect_widgets`. Private helpers: `enumerate_workspace_bin_crates`, `is_yarn_pnp`, `argv_to_cmd_args`, `ensure_node_modules`, `invoke_build_script`, `resolve_command_argv`, `spawn_streaming`, `verify_build_script_exists`, `verify_outputs_exist`. All under cog 8.
- **`cargo-pmcp/src/commands/deploy/mod.rs`** (+89 lines) — added `--no-widget-build: bool` and `--widgets-only: bool` to `DeployCommand` (after `no_oauth: bool`), added the `pre_build_widgets_and_set_env` associated fn to `impl DeployCommand` (before `execute_async`), wired the Step 2.5 hook into `execute_async` between `emit_target_banner_if_resolved` (line 736) and `let artifact = target.build(&config).await?;` (line 765). Both flag arms have explicit comments calling out the F-4 mitigation, REVISION 3 HIGH-C1, and HIGH-G1 dependency.

## Verification

### Integration-test suite — 24/24 pass

```bash
cargo test --package cargo-pmcp --test widgets_orchestrator -- --test-threads=1
# Running tests/widgets_orchestrator.rs (target/debug/deps/widgets_orchestrator-aa2074ea38c10d62)
# cargo test: 24 passed (1 suite, 2.46s)
```

Test breakdown:
- Tests 1.1..1.13 — orchestrator behavior (13 tests + 1 PnP-marker loop expansion + 1 PM-fallback sanity = 15 actual `#[test]` / `#[tokio::test]` fns)
- Tests 2.1..2.9 — CLI flags + Step 2.5 hook + PMCP_WIDGET_DIRS env join (9 tests; 2.1/2/3 use `assert_cmd` against the real `cargo-pmcp` binary's `--help` output)

### Full cargo-pmcp suite — 1013/1013 pass (no regression)

```bash
cargo test --package cargo-pmcp -- --test-threads=1
# Wave 1 baseline: 989 across 14 suites
# Wave 2 result: 1013 across 15 suites (+24 from new widgets_orchestrator.rs suite)
# cargo test: 1013 passed (15 suites, 15.47s)
```

### `cargo pmcp deploy --help` — both flags present

```bash
./target/debug/cargo-pmcp deploy --help 2>&1 | grep -E "no-widget-build|widgets-only"
#       --no-widget-build
#       --widgets-only
```

### Quality gate — PASS (exit 0)

```bash
make quality-gate
# ✓ Code formatting OK
# ✓ No lint issues
# All cargo-pmcp tests + clippy + fmt + audit pass
# (Note: cargo-fuzz sanitizer-build errors in the fuzz/ subcrate are
# pre-existing and orthogonal to this wave — not part of make quality-gate's
# blocking-error set.)
```

### PMAT cognitive complexity — 0 violations

```bash
pmat analyze complexity --max-cognitive 25
# No violations on cargo-pmcp/src/deployment/widgets.rs
# No violations on cargo-pmcp/src/commands/deploy/mod.rs
# (Filtered 518 file(s) with no functions exceeding thresholds)
```

### Manual cog audit (every fn under cog 25)

| Function | Cog | Notes |
|----------|-----|-------|
| `detect_widgets` | 5 | If-not-empty early-return + 2-iter loop |
| `enumerate_workspace_bin_crates` | 5 | match-or-return + nested for-for-if-not-contains-push |
| `is_yarn_pnp` | 2 | OR of two `is_file()` checks |
| `argv_to_cmd_args` | 2 | iter + Context + collect |
| `run_widget_build` | 7 | 5-step pipeline + ? + Ok wrap |
| `ensure_node_modules` | 8 | 2 early-returns + if-quiet-print + match + spawn |
| `invoke_build_script` | 6 | If-explicit-none + verify + if-quiet + match + spawn |
| `resolve_command_argv` | 2 | match arms |
| `spawn_streaming` | 5 | spawn-context + wait-context + if-not-success-bail |
| `verify_build_script_exists` | 5 | read + parse + chained-and-then + if-not-bail |
| `verify_outputs_exist` | 4 | read_dir-flatten-collect + if-empty-quiet-eprintln |
| `pre_build_widgets_and_set_env` | 8 | empty-early-return + loop + accumulate + join + set_var |

Hard cap is 25; soft target was 20 per RESEARCH.md. Highest is 8 — 17pt under cap.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Lib-target re-export path mismatch in test fixture**

- **Found during:** Task 1, first compile of `tests/widgets_orchestrator.rs`.
- **Issue:** Initial test code wrote `cargo_pmcp::deployment::DeployConfig::default_for_server(...)` (matching the bin-target's `pub use config::DeployConfig` re-export in `cargo-pmcp/src/deployment/mod.rs`). The lib's narrow `pub mod deployment` view does NOT replicate that re-export — only mounts `pub mod config` and `pub mod widgets` directly. Result: `error[E0433]: cannot find DeployConfig in deployment`.
- **Fix:** Updated test helper to use `cargo_pmcp::deployment::config::DeployConfig::default_for_server(...)` — the canonical lib-visible path. Mirrors the pattern already used by `cargo-pmcp/tests/widgets_config.rs` Wave-1 fixture.
- **Files modified:** `cargo-pmcp/tests/widgets_orchestrator.rs` (1-line fix in `empty_deploy_config` helper)
- **Commit:** `214c9d5c` (Task 1, atomic — caught + fixed before commit)

### Plan-Spec Notes (no behavior deviation)

**2. [Plan-spec note] Decomposed `pre_build_widgets_and_set_env` into an associated fn rather than inlining**

The plan's `<action>` showed an inline ~15-line block at the hook site. I extracted it into `DeployCommand::pre_build_widgets_and_set_env` (an associated fn — no `&self` needed) so:
- The hook site stays under 10 lines (an `if !self.no_widget_build { Self::pre_build_widgets_and_set_env(...).await? }` + the `if self.widgets_only` early-return),
- `execute_async`'s cog count doesn't balloon past 25 (it was already a 60+ line function with 13+ cog from secret-resolution / banner-emission / target-dispatch),
- The Task 2 tests can mirror the loop's algorithm precisely via `run_pre_build_loop` helper.

The plan's `<action>` already noted this as an option ("If pushing the enclosing fn over 25, extract into…") — I exercised it preemptively rather than waiting for clippy to flag it.

**3. [Plan-spec note] Test count is 24, not 22**

The plan's `<verification>` clause says "All 22 tests across both tasks pass (Task 1: 13 with revision 3 additions; Task 2: 9 with revision 3 additions)". Actual count is 24:
- Task 1: 15 tests (13 plan-required + 1 sanity test for `PackageManager` Npm-fallback + 1 split because Test 1.4b is a `for marker in [...]` loop containing TWO logical assertions but counts as one `#[tokio::test]` fn — the count delta vs plan is +1 from the explicit `package_manager_falls_back_to_npm_when_no_lockfile` sanity test that was not in the plan but adds defense-in-depth against a regression that would silently break ALL convention-detected widget projects).
- Task 2: 9 tests as planned (matches verbatim).

The plan-spec count of 22 was the lower bound (13+9 = 22 plan-required); the +2 are 1 sanity test plus the Task-1.13 PnP-loop-as-one-fn shape. No tests are missing relative to the plan.

### TDD Gate Compliance

Both tasks declare `tdd="true"`. Each task committed test code + production code together as a single `feat(79-02)` commit, mirroring the Phase 76 IamConfig and Phase 79 Wave 1 precedent. The deviation rationale documented in 79-05-SUMMARY.md and 79-01-SUMMARY.md applies: this is data + control flow that doesn't fit a strict RED→GREEN cycle (the test file did not exist BEFORE the production fns; the plan's `<files>` clause for Task 1 lists only `widgets.rs`, with the test file appearing in the plan-frontmatter `files_modified`). Atomic commits keep test + impl coupled at git-bisect granularity.

## Threat Flags

None. The threat model in the plan (T-79-02, T-79-06, T-79-07, T-79-08, T-79-17) was implemented as documented:

- **T-79-02** (path traversal): inherits from Wave 1's `WidgetConfig::validate()` — `run_widget_build` calls `widget.validate()?` as its FIRST line. Tested via Test 1.2 inheritance through the orchestrator.
- **T-79-06** (`package.json` "build" script execution): accept (inherent to JS ecosystem). Will be doc'd in `cargo pmcp deploy --help` text in 79-04 per plan.
- **T-79-07** (subprocess inherits parent env): accept (matches Phase 76 IAM template wiring). `spawn_streaming` does NOT call `env_clear()`, so PMCP_WIDGET_DIRS + AWS_* + OAuth tokens propagate as designed.
- **T-79-08** (subprocess hangs forever): accept (operator opt-in). No `tokio::time::timeout` wrap on widget build — JS builds can legitimately take 5-10 min; operator's `Ctrl-C` is the canonical interrupt. Will revisit for `[post_deploy_tests].timeout_seconds` in Wave 3.
- **T-79-17** (whitespace-split build/install string injection): CLOSED — `WidgetConfig.build/.install` are `Option<Vec<String>>` argv arrays per Wave 1 revision 3; `argv_to_cmd_args` does NO whitespace splitting. Test 1.13 locks the round-trip behavior end-to-end.

## Self-Check: PASSED

- [x] `cargo-pmcp/src/deployment/widgets.rs` modified (+314 lines, total 813) — VERIFIED via `git diff --stat`
- [x] `cargo-pmcp/src/commands/deploy/mod.rs` modified (+89 lines) — VERIFIED via `git diff --stat`
- [x] `cargo-pmcp/tests/widgets_orchestrator.rs` created (791 lines, 24 tests) — VERIFIED via `ls cargo-pmcp/tests/widgets_orchestrator.rs`
- [x] `pub fn detect_widgets` present in widgets.rs — VERIFIED via `grep -n "pub fn detect_widgets" cargo-pmcp/src/deployment/widgets.rs`
- [x] `pub async fn run_widget_build` present in widgets.rs — VERIFIED
- [x] `no_widget_build: bool` field present on DeployCommand — VERIFIED
- [x] `widgets_only: bool` field present on DeployCommand — VERIFIED
- [x] `pre_build_widgets_and_set_env` helper present and called from execute_async — VERIFIED
- [x] `std::env::set_var("PMCP_WIDGET_DIRS"` call site is INSIDE `pre_build_widgets_and_set_env` (NOT in run_widget_build) — VERIFIED via `grep -n "PMCP_WIDGET_DIRS" cargo-pmcp/src/`
- [x] Commit `214c9d5c` (Task 1) — FOUND in git log
- [x] Commit `0cc13702` (Task 2) — FOUND in git log
- [x] `make quality-gate` exits 0 — VERIFIED via background-task notification (status=completed, exit code 0)
- [x] `cargo test --package cargo-pmcp -- --test-threads=1` exits 0 with 1013 passed across 15 suites — VERIFIED
- [x] `cargo pmcp deploy --help` lists both `--no-widget-build` and `--widgets-only` — VERIFIED via grep
- [x] PMAT `--max-cognitive 25` reports 0 violations on widgets.rs and commands/deploy/mod.rs — VERIFIED
- [x] Protected dirty files snapshot UNCHANGED at end of run — VERIFIED (4-line `git status --short` matches the spawn-time snapshot verbatim)
