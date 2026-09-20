---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
plan: 04
subsystem: deployment
tags: [cargo-pmcp, deploy, widgets, doctor, scaffold, build-rs, fuzz, example, version-bump, changelog]

requires:
  - phase: 79
    plan: 01
    provides: "WidgetsConfig + WidgetConfig + PackageManager + PostDeployTestsConfig + OnFailure (custom Deserialize hard-rejects 'rollback') + ROLLBACK_REJECT_MESSAGE constant"
  - phase: 79
    plan: 02
    provides: "run_widget_build async orchestrator + ResolvedPaths public struct + cargo_pmcp::deployment::widgets:: lib-visible surface"
  - phase: 79
    plan: 03
    provides: "post_deploy_orchestrator hook + parse_on_test_failure_flag clap value parser referencing OnFailure::FromStr"
  - phase: 79
    plan: 05
    provides: "mcp-tester PostDeployReport public module + --format=json flag (justifies mcp-tester 0.5.3 → 0.6.0 minor bump)"
provides:
  - "`cargo pmcp doctor` `check_widget_rerun_if_changed` extension that distinguishes WidgetDir crates (silent) from include_str! crates (warns when build.rs missing)"
  - "`cargo pmcp app new --embed-widgets` flag that switches scaffold from default WidgetDir to include_str! + writes a build.rs"
  - "`generate_build_rs()` template (REVISION 3 HIGH-C1 + HIGH-G1) — PMCP_WIDGET_DIRS colon-list consumer + discover_local_widget_dirs() local-discovery fallback"
  - "Verbatim REQ-79-18 widgets+verification mention on `cargo pmcp deploy --help`"
  - "`cargo run -p cargo-pmcp --example widget_prebuild_demo` runnable schema-direct demo against tempdir+fake-package.json"
  - "`fuzz_widgets_config` adversarial TOML fuzz target including OnFailure rollback-rejection path (REVISION 3 HIGH-G2)"
  - "cli_acceptance integration tests: deploy_help_mentions_widgets_verbatim + deploy_on_test_failure_rollback_hard_rejected + app_new_embed_widgets_flag_parses"
  - "cargo-pmcp 0.12.0 release-ready (CHANGELOG dated, all version bumps applied)"
  - "mcp-tester 0.6.0 release-ready (NEW CHANGELOG.md created — first one for this crate)"
affects: [phase 79 close]

tech-stack:
  added: []
  patterns:
    - "Phase-79 RESEARCH.md Pattern 2 (per-stage pipeline decomposition) extended into doctor — `check_widget_rerun_if_changed` (cog 5) + `scan_workspace_for_pattern` (cog 8) + `print_widget_rerun_warning` (cog 4) + `find_enclosing_crate_dir` (cog 5) + `crate_has_rerun_if_changed` (cog 3) + `is_ignored_dir` (cog 2)"
    - "Module-level `&'static str BUILD_RS_TEMPLATE` so `generate_build_rs()` is cog 1 (single return) and the template body is unit-testable directly via `body.contains(...)`"
    - "Embed_widgets opt-in pattern: scaffold variant chosen at parse time, not detected — explicit user intent required to switch from default WidgetDir to include_str!"
    - "Doctor scan filter-entry skip-list (`target/`, `node_modules/`, `.git/`) — performance-bounded walk via WalkDir's filter_entry"
    - "rustc-direct compile test for build.rs template (avoids needing a full Cargo dependency graph for the standalone-compiles test)"
    - "Fuzz target dual-path: parse-then-validate (T-79-01) + synthesize-OnFailure-TOML (T-79-05/HIGH-G2) in a single fuzz_target! body"

key-files:
  created:
    - "cargo-pmcp/examples/widget_prebuild_demo.rs (~110 lines, runnable schema-direct demo)"
    - "cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs (~50 lines, dual-path adversarial fuzz)"
    - "crates/mcp-tester/CHANGELOG.md (NEW — first CHANGELOG for this crate, ~25 lines)"
    - ".planning/phases/79-.../79-04-SUMMARY.md (this file)"
    - ".planning/phases/79-.../79-SUMMARY.md (phase-level aggregate, also created by this wave)"
  modified:
    - "cargo-pmcp/src/main.rs (+5 lines: REQ-79-18 verbatim doc on Commands::Deploy)"
    - "cargo-pmcp/src/commands/doctor.rs (+267 lines: check_widget_rerun_if_changed + 5 helpers + 5 unit tests)"
    - "cargo-pmcp/src/commands/app.rs (+18 lines: --embed-widgets flag + wiring)"
    - "cargo-pmcp/src/templates/mcp_app.rs (+387 lines: generate_main_rs_include_str + generate_build_rs + BUILD_RS_TEMPLATE + 4 new tests + 2 existing tests updated for new arity)"
    - "cargo-pmcp/tests/cli_acceptance.rs (+82 lines: 3 new tests — REQ-79-18 verbatim, HIGH-G2 rollback rejection, embed_widgets flag parse)"
    - "cargo-pmcp/Cargo.toml (version 0.11.0 → 0.12.0; mcp-tester pin 0.5.3 → 0.6.0)"
    - "crates/mcp-tester/Cargo.toml (version 0.5.3 → 0.6.0)"
    - "crates/pmcp-server/Cargo.toml (mcp-tester pin 0.5.0 → 0.6.0 — Rule 3 deviation, see below)"
    - "cargo-pmcp/CHANGELOG.md (+58 lines: full [0.12.0] entry documenting all REVISION 3 supersessions)"
    - "cargo-pmcp/fuzz/Cargo.toml (+8 lines: new [[bin]] entry for fuzz_widgets_config)"

key-decisions:
  - "Doctor extension uses regex `include_str!\\s*\\(\\s*\"[^\"]*widgets?/[^\"]*\"\\s*\\)` (anchored to widgets-OR-widget path segments). REVISION 3 Codex MEDIUM: this regex matches ONLY include_str! against widget paths — WidgetDir crates that grep-match `WidgetDir::new(\"widget/dist\")` are SILENTLY SKIPPED (no warn fires). Mixed crates (BOTH WidgetDir AND include_str!) WARN because the include_str! branch drives the cache problem."
  - "BUILD_RS_TEMPLATE is a module-level `const &str` (verbatim Rust source) so `generate_build_rs()` becomes cog 1 (single return). Trade-off: the template body is not parameterised on user input — but since the contract is fixed (PMCP_WIDGET_DIRS env var name + standard candidate dirs), there's nothing to parameterise. Tests inspect the template via `body.contains(...)` directly."
  - "Scaffold's include_str!-variant `main.rs` dispatches widgets via a `match name { \"hello\" => HELLO_WIDGET_HTML, _ => err }` block. Each new widget needs an explicit branch + `const` — explicit-by-design tradeoff vs. WidgetDir's directory-scan auto-discovery. Operators who want auto-discovery should NOT use --embed-widgets."
  - "scaffold_build_rs_compiles_standalone test drives `rustc` directly (not `cargo build`) since the build.rs body has zero external deps — no Cargo dependency graph required, ~100ms test runtime."
  - "REVISION 3 Codex MEDIUM (--embed-widgets opt-in) realised as branching at the templates::mcp_app::generate level — the same Cargo.toml is generated either way; only main.rs and the (presence of) build.rs differ. Keeps the existing scaffold contract intact for default users."
  - "cli_acceptance tests for REQ-79-18 + HIGH-G2 + Codex MEDIUM (Test 1.6, 1.7, 2.6) ship in Task 1's commit (`bfe5665c`) instead of Task 2 because they sit naturally with the help-text + flag-parsing changes. Task 2's commit (`88d4d84e`) ships only the example + fuzz files. Net test count (12) matches the plan."
  - "Plan 79-04 originally specified `cargo test --features full` in <verify>, but cargo-pmcp has no `full` feature — used the default-feature build (which still exercises every code path; `aws-secrets` is the only optional cargo-pmcp feature and isn't touched by this wave)."
  - "pmcp-server crate's mcp-tester dep pin (`0.5.0`) was a blocking issue (Rule 3) once mcp-tester bumped to 0.6.0 — semver `^0.5.0` excludes 0.6.0. Updated the pin to 0.6.0; pmcp-server itself unchanged at 0.2.2 because mcp-tester is consumed internally only (no public re-exports). See deviations section below."

requirements-completed: [REQ-79-06, REQ-79-07, REQ-79-08, REQ-79-12, REQ-79-18]

duration: ~24min
completed: 2026-05-03
---

# Phase 79 Plan 79-04: Wave 4 — doctor + scaffold + example + fuzz + version bump Summary

**Wave 4 closes Phase 79: ships the `cargo pmcp doctor` widget-rerun extension that distinguishes WidgetDir from include_str! crates, the `cargo pmcp app new --embed-widgets` opt-in scaffold + REVISION 3 HIGH-C1+HIGH-G1 build.rs template (PMCP_WIDGET_DIRS colon-list + local-discovery fallback), the verbatim REQ-79-18 widgets+verification help text on `cargo pmcp deploy --help`, the runnable `widget_prebuild_demo` example, the `fuzz_widgets_config` adversarial TOML fuzz target (now also exercising the OnFailure rollback-rejection path per HIGH-G2), and the cargo-pmcp 0.12.0 + mcp-tester 0.6.0 minor bumps with full CHANGELOG entries documenting all 6 REVISION 3 HIGH supersessions + 3 MEDIUM polish items. 12 new tests (5 doctor + 4 scaffold + 3 cli_acceptance) all green; full cargo-pmcp suite 1064/1064; mcp-tester 205/205; pmcp-server 9/9; `make quality-gate` exits 0; protected pentest/wasm-client WIP files snapshot unchanged across the entire wave.**

## Performance

- **Duration:** ~24 min (start 22:11Z, end 22:36Z UTC 2026-05-03)
- **Tasks:** 3/3
- **Tests added:** 12 (5 doctor unit + 4 scaffold unit + 3 cli_acceptance integration)
- **Files created:** 3 (example + fuzz target + new mcp-tester CHANGELOG.md)
- **Files modified:** 9 (main.rs + doctor.rs + app.rs + mcp_app.rs + cli_acceptance.rs + 2 Cargo.toml + cargo-pmcp/CHANGELOG.md + fuzz/Cargo.toml + pmcp-server/Cargo.toml)
- **Commits:** 3 (Task 1 `bfe5665c`, Task 2 `88d4d84e`, Task 3 `d7ede7a3`)

## Accomplishments

- **REVISION 3 Codex MEDIUM (scaffold target alignment) landed end-to-end:** `cargo pmcp app new <name>` defaults to the existing `WidgetDir` scaffold (run-time file serving, no Cargo cache invalidation problem). `cargo pmcp app new <name> --embed-widgets` opts into `include_str!` widget embedding AND writes a `build.rs`. Test `scaffold_default_does_not_write_build_rs` locks the default behavior; `scaffold_with_embed_widgets_writes_build_rs_and_main_uses_include_str` locks the opt-in behavior.
- **REVISION 3 Codex MEDIUM (doctor WidgetDir-aware) landed:** `check_widget_rerun_if_changed` matches ONLY `include_str!\s*\(\s*"[^"]*widgets?/[^"]*"\s*\)` — WidgetDir-only crates are silently skipped. Tests `doctor_widget_check_silent_for_widget_dir_crates` (1.3b) and `doctor_widget_check_warns_for_mixed_crate_without_build_rs` (1.3c) lock the discriminator.
- **REVISION 3 HIGH-C1 (PMCP_WIDGET_DIRS list) realised in build.rs template:** `BUILD_RS_TEMPLATE` consumes the colon-separated list via `.split(':')`. Test `build_rs_template_contains_required_directives` asserts `"v.split(':')"` and `"cargo:rerun-if-env-changed=PMCP_WIDGET_DIRS"` are both present in the emitted source.
- **REVISION 3 HIGH-G1 (local-discovery fallback) realised:** `discover_local_widget_dirs` walks `CARGO_MANIFEST_DIR + ../widget|widgets/dist` up to 3 parents. Restored the direct `cargo run` / `cargo build` dev loop without requiring `cargo pmcp deploy` wrapping. Same template-content test asserts the fallback fn is present + the `["widget", "widgets"]` candidate list is intact.
- **REVISION 3 HIGH-G2 (rollback hard-reject) integration-tested:** `deploy_on_test_failure_rollback_hard_rejected` drives `cargo pmcp deploy --on-test-failure=rollback` against the real binary and asserts the clap-parse layer rejects with stderr containing `"not yet implemented"` (the verbatim `ROLLBACK_REJECT_MESSAGE` substring set in 79-01). The fuzz target `fuzz_widgets_config` ALSO exercises the OnFailure custom Deserialize on adversarial inputs.
- **REQ-79-18 verbatim help text shipped:** `cargo pmcp deploy --help` now contains both verbatim phrases from CONTEXT.md REQ-79-18: `"Builds widgets (auto-detected from widget/ or widgets/) before compiling and deploying the Rust binary."` AND `"Verifies the deployed endpoint via cargo pmcp test {check,conformance,apps} before reporting success."` Test `deploy_help_mentions_widgets_verbatim` locks against accidental rewording.
- **CLAUDE.md ALWAYS-required testing dimensions ALL covered for Phase 79:** FUZZ (`fuzz_widgets_config.rs` — Plan 79-04), PROPERTY (`tests/widgets_config.rs::pm_detection_priority_order` — Plan 79-01), UNIT (60+ unit tests across all 5 plans), EXAMPLE (`widget_prebuild_demo.rs` — Plan 79-04). Final ALWAYS-coverage closeout achieved.
- **Version bumps clean:** cargo-pmcp 0.11.0 → 0.12.0; mcp-tester 0.5.3 → 0.6.0; pmcp 2.6.0 unchanged; mcp-preview 0.3.0 unchanged. Two downstream pin updates: `cargo-pmcp/Cargo.toml` mcp-tester pin `0.5.3 → 0.6.0` (planned) AND `crates/pmcp-server/Cargo.toml` mcp-tester pin `0.5.0 → 0.6.0` (Rule 3 auto-fix — see Deviations).
- **Two new CHANGELOG entries dated 2026-05-03:**
  - `cargo-pmcp/CHANGELOG.md ## [0.12.0]` documents the full Phase 79 surface area (build half + verify half + Security T-79-* mitigations + Notes section calling out the doctor-driven migration path for pre-0.12.0 projects).
  - `crates/mcp-tester/CHANGELOG.md` is a NEW FILE (this crate didn't have one before) — its `## [0.6.0]` entry documents the new public `post_deploy_report` module + `--format=<pretty|json>` flag (defaults to `pretty` so older cargo-pmcp continues to work).
- **Cog budget held:** every new function ≤ cog 8 (PMAT cap is 25, 17pt safety margin):
  - `check_widget_rerun_if_changed` cog 5
  - `scan_workspace_for_pattern` cog 8 (filter-entry + extension check + file-read + regex-match + crate-dir lookup + insert)
  - `print_widget_rerun_warning` cog 4
  - `find_enclosing_crate_dir` cog 5
  - `crate_has_rerun_if_changed` cog 3
  - `is_ignored_dir` cog 2
  - `generate_build_rs` cog 1 (single return)
  - `generate_main_rs_include_str` cog 2 (replace + format!)
- **Regression-clean:** full test suites run after each task. Final state: cargo-pmcp 1064 passed (17 suites), mcp-tester 205 passed (9 suites), pmcp-server 9 passed (3 suites). `make quality-gate` exits 0. Protected pentest/wasm-client WIP-file snapshot (4 files) UNCHANGED across the entire wave (`git status --short` against the snapshot returns the verbatim 4-line state from spawn time at every task boundary).

## Task Commits

1. **Task 1: doctor widget-rerun check + --embed-widgets scaffold + deploy --help text** — `bfe5665c` (feat)
2. **Task 2: widget_prebuild_demo example + fuzz_widgets_config target** — `88d4d84e` (feat)
3. **Task 3: bump cargo-pmcp 0.12.0 + mcp-tester 0.6.0 + CHANGELOGs** — `d7ede7a3` (chore)

## Files Created/Modified

### Created

- **`cargo-pmcp/examples/widget_prebuild_demo.rs`** (~110 lines) — runnable schema-direct demo. Exercises: tempdir setup with fake `package.json`, lockfile-driven `PackageManager::Npm` detection, node_modules install-skip heuristic, `run_widget_build` orchestrator end-to-end (with explicit `["sh", "-c", ...]` build argv so the demo runs without Node), `PMCP_WIDGET_DIRS` env-var contract for the single-widget list-of-one case. Assertions throughout; exits 0 on success.
- **`cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs`** (~50 lines) — dual-path adversarial fuzz target. Path 1: `toml::from_str::<DeployConfig>` on raw bytes + `WidgetConfig::validate()` on each parsed widget (T-79-01 + T-79-02). Path 2: synthesises a TOML doc with `on_failure = "<fuzzed-string>"` so the OnFailure custom Deserialize hard-reject path is exercised on adversarial inputs (REVISION 3 HIGH-G2 / T-79-05).
- **`crates/mcp-tester/CHANGELOG.md`** (NEW FILE — first CHANGELOG for this crate, ~25 lines) — `## [0.6.0] - 2026-05-03` entry documents the new public `post_deploy_report` module + `--format=<pretty|json>` flag.

### Modified

- **`cargo-pmcp/src/main.rs`** (+5 lines) — verbatim REQ-79-18 doc-comment on the `Commands::Deploy(...)` enum variant. Clap exposes the doc as the `--help` summary text.
- **`cargo-pmcp/src/commands/doctor.rs`** (+267 lines) — added `check_widget_rerun_if_changed` (wired into `execute()` after `check_clippy(quiet)`) + 5 private helpers (`scan_workspace_for_pattern`, `print_widget_rerun_warning`, `find_enclosing_crate_dir`, `crate_has_rerun_if_changed`, `is_ignored_dir`) + 5 unit tests (Tests 1.1, 1.2, 1.3, 1.3b, 1.3c). New `use regex::Regex; use walkdir::WalkDir;` imports — both already in cargo-pmcp's deps so no new dependency added.
- **`cargo-pmcp/src/commands/app.rs`** (+18 lines) — added `embed_widgets: bool` field to `AppCommand::New { ... }` clap variant + wired through to `create_app(name, path, embed_widgets)` + updated rustdoc on `create_app` to call out the WidgetDir-vs-include_str! branch.
- **`cargo-pmcp/src/templates/mcp_app.rs`** (+387 lines) — `generate(project_dir, name, embed_widgets)` arity bumped to 3; added `generate_main_rs_include_str` (the include_str! variant, ~150 lines), `generate_build_rs` (cog-1 wrapper) + `BUILD_RS_TEMPLATE` const (the verbatim build.rs source, ~60 lines), 4 new tests (Tests 1.4, 1.4b, 1.5b/c unit form, 1.5 standalone-compiles), and updated the 2 existing `test_generate_creates_*` tests to pass the new `embed_widgets=false` argument.
- **`cargo-pmcp/tests/cli_acceptance.rs`** (+82 lines) — 3 new integration tests against the real `cargo-pmcp` binary: `deploy_help_mentions_widgets_verbatim` (REQ-79-18 verbatim assertion on both phrases), `deploy_on_test_failure_rollback_hard_rejected` (REVISION 3 HIGH-G2 — clap parse rejects with stderr containing `"not yet implemented"`), `app_new_embed_widgets_flag_parses` (Codex MEDIUM — `--embed-widgets` listed in `app new --help` output).
- **`cargo-pmcp/Cargo.toml`** — `version = "0.11.0"` → `version = "0.12.0"`; `mcp-tester = { version = "0.5.3", path = "../crates/mcp-tester" }` → `version = "0.6.0"`.
- **`crates/mcp-tester/Cargo.toml`** — `version = "0.5.3"` → `version = "0.6.0"`.
- **`crates/pmcp-server/Cargo.toml`** — `mcp-tester = { version = "0.5.0", ... }` → `version = "0.6.0"` (Rule 3 auto-fix; see Deviations).
- **`cargo-pmcp/CHANGELOG.md`** (+58 lines) — full `## [0.12.0] - 2026-05-03` entry: Build half (closes A+B), Verify half (closes C), Changed (mcp-tester bump), Security (T-79-01/02/04/05/17), Notes (doctor-driven migration path + multi-widget + direct cargo-run notes).
- **`cargo-pmcp/fuzz/Cargo.toml`** (+8 lines) — new `[[bin]] name = "fuzz_widgets_config"` entry mirrors the `fuzz_iam_config` shape (no test/no doc/no bench; same path/extension convention).

## Verification

### Test suites — all green

```bash
# New tests added in this wave (12 total)
cargo test -p cargo-pmcp --bin cargo-pmcp -- --test-threads=1 doctor::tests::
# 5 passed: doctor_widget_check_warns_when_include_str_lacks_build_rs (1.1),
#            doctor_widget_check_silent_when_build_rs_present (1.2),
#            doctor_widget_check_silent_when_no_widget_includes (1.3),
#            doctor_widget_check_silent_for_widget_dir_crates (1.3b),
#            doctor_widget_check_warns_for_mixed_crate_without_build_rs (1.3c)

cargo test -p cargo-pmcp --bin cargo-pmcp -- --test-threads=1 templates::mcp_app
# 11 passed (7 pre-existing + 4 new): scaffold_default_does_not_write_build_rs (1.4),
#            scaffold_with_embed_widgets_writes_build_rs_and_main_uses_include_str (1.4b),
#            build_rs_template_contains_required_directives (1.5b/c unit form),
#            scaffold_build_rs_compiles_standalone (1.5)

cargo test -p cargo-pmcp --test cli_acceptance -- --test-threads=1
# 7 passed (4 pre-existing + 3 new): deploy_help_mentions_widgets_verbatim (1.6),
#            deploy_on_test_failure_rollback_hard_rejected (2.6),
#            app_new_embed_widgets_flag_parses (1.7)

# Full cargo-pmcp regression suite
cargo test -p cargo-pmcp -- --test-threads=1
# 1064 passed (17 suites, 27.12s)

# mcp-tester regression after version bump
cargo test -p mcp-tester -- --test-threads=1
# 205 passed (9 suites, 6.03s)

# pmcp-server regression after mcp-tester pin bump
cargo test -p pmcp-server -- --test-threads=1
# 9 passed (3 suites)
```

### Runnable example + fuzz target

```bash
cargo run -p cargo-pmcp --example widget_prebuild_demo
# === Phase 79 — Widget pre-build orchestrator (isolation demo, REVISION 3) ===
# ... 5 sections, all assertions pass, exit 0

cargo build -p cargo-pmcp --examples
# Finished `dev` profile, widget_prebuild_demo binary present in target/debug/examples/

cd cargo-pmcp/fuzz && cargo build --bin fuzz_widgets_config
# Compiles cleanly on stable (cargo-fuzz / nightly not required for compile-check)
```

### Quality gate

```bash
make quality-gate    # → exit 0
# (Includes: fmt-check, lint with --features full + pedantic+nursery clippy,
# build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always)
```

### Version bump audit

```bash
grep -c '^version = "0.12.0"' cargo-pmcp/Cargo.toml          # → 1
grep -c '^version = "0.6.0"'  crates/mcp-tester/Cargo.toml   # → 1
grep -c '^## \[0.12.0\]'      cargo-pmcp/CHANGELOG.md         # → 1
grep -c '^## \[0.6.0\]'       crates/mcp-tester/CHANGELOG.md  # → 1
grep -E '^version' Cargo.toml | head -1                       # version = "2.6.0"  (UNCHANGED)
grep -E '^version' crates/mcp-preview/Cargo.toml              # version = "0.3.0"  (UNCHANGED)
```

### Protected files snapshot — UNCHANGED across all 3 commits

```bash
git status --short cargo-pmcp/src/pentest/attacks/tool_poisoning.rs \
                   examples/wasm-client/Cargo.toml \
                   examples/wasm-client/src/lib.rs \
                   examples/wasm-client/src/pentest.rs
#  M cargo-pmcp/src/pentest/attacks/tool_poisoning.rs
#  M examples/wasm-client/Cargo.toml
#  M examples/wasm-client/src/lib.rs
#  M examples/wasm-client/src/pentest.rs
```

Verbatim 4-line snapshot from spawn time. Audit confirmed at: pre-Task-1 commit, post-Task-1 commit, pre-Task-2 commit, post-Task-2 commit, pre-Task-3 commit, post-Task-3 commit. Zero `git stash` / `git clean` / `git reset --hard` invocations.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `pmcp-server` crate's mcp-tester dep pin needed bumping from `0.5.0` → `0.6.0`**

- **Found during:** Task 3, first `cargo build -p mcp-tester` after the version bump.
- **Issue:** `error: failed to select a version for the requirement 'mcp-tester = "^0.5.0"' ... required by package 'pmcp-server v0.2.2'`. Semver `^0.5.0` excludes `0.6.0` (minor bumps are NOT compatible under caret-requirements with versions <1.0).
- **Fix:** Updated `crates/pmcp-server/Cargo.toml` `mcp-tester = { version = "0.5.0", ... }` → `version = "0.6.0"`.
- **pmcp-server itself NOT bumped:** Verified pmcp-server uses mcp-tester only INTERNALLY (`mcp_tester::ScenarioGenerator`, `AppValidator`, `ServerTester`, `TestStatus` consumed in `tools/test_*.rs` + `tools/schema_export.rs` + `tools/mod.rs`) with NO `pub use mcp_tester::*` re-exports. mcp-tester's 0.6.0 surface is fully additive (NEW `post_deploy_report` module + NEW `--format=json` flag) — pmcp-server's existing usage continues to work. Therefore pmcp-server's public API is unchanged → no version bump required for pmcp-server itself per CLAUDE.md release rules. Stays at 0.2.2.
- **CLAUDE.md justification:** "Downstream crates that pin a bumped dependency must also be bumped" applies to PINS (the dep version line) — it does NOT mandate bumping the downstream crate's own version when the dep change is internal-only. Documented this nuance inline in Task 3's commit message (`d7ede7a3`).
- **Files modified:** `crates/pmcp-server/Cargo.toml` (1-line dep pin update).
- **Commit:** `d7ede7a3` (Task 3, atomic — caught + fixed before commit).

### Plan-Spec Notes (no behavior deviation)

**2. [Plan-spec note] cli_acceptance tests for REQ-79-18 + HIGH-G2 + Codex MEDIUM shipped in Task 1's commit, not Task 2's**

The plan's Task 2 `<files>` clause listed `cargo-pmcp/tests/cli_acceptance.rs` for the REQ-79-18 and HIGH-G2 tests. I shipped those 3 tests in Task 1's commit (`bfe5665c`) instead because they sit naturally next to the help-text + flag-parsing changes (`Commands::Deploy` doc + `parse_on_test_failure_flag` clap value parser + `app new --embed-widgets` flag). Task 2's commit (`88d4d84e`) ships only the example + fuzz target. Net test count is 12 (matches plan's 9 Task-1 + 3 Task-2 cli_acceptance), just rearranged across commits for cohesion.

**3. [Plan-spec note] `cargo test --features full` substituted with default-feature `cargo test`**

The plan's Task 1 `<verify>` clause specified `cargo test --package cargo-pmcp --features full`. cargo-pmcp has NO `full` feature (only `default = []` and the optional `aws-secrets`). The plan's wording was a copy-paste from the root pmcp crate's `--features full` convention. Used the default-feature build instead — exercises every code path I touched (the new doctor + scaffold + cli_acceptance code is feature-gate-free).

**4. [Plan-spec note] Test count is 12, not 9 (plan said 22 across both tasks)**

The plan's `<verification>` clause says "All 22 tests across all three tasks pass". Actual count is:
- Task 1: 12 tests (5 doctor + 4 scaffold + 3 cli_acceptance — exceeds plan's 9 because I split Test 1.5b/c into a unit-form `build_rs_template_contains_required_directives` test (no rustc/cargo dependency) AND retained the Test 1.5 `scaffold_build_rs_compiles_standalone` rustc-direct compile check for the round-trip guarantee).
- Task 2: 0 new tests (the cli_acceptance tests planned-for-Task-2 shipped in Task 1's commit per note 2 above).
- Task 3: 4 verification grep commands (NOT `#[test]` items — these are CHANGELOG/Cargo.toml grep checks, see plan's Task 3 `<verify>` clause).

Total `#[test]` / `#[tokio::test]` items added by this wave: 12. Plan-required minimum was 9 (Task 1's count). The +3 are: `doctor_widget_check_silent_for_widget_dir_crates` (1.3b), `doctor_widget_check_warns_for_mixed_crate_without_build_rs` (1.3c), `build_rs_template_contains_required_directives` (1.5b/c unit form). All three add defense-in-depth that the plan called for in prose (REVISION 3 Codex MEDIUM + HIGH-C1 + HIGH-G1) but didn't lift into separate test fns.

### TDD Gate Compliance

All three tasks declare `tdd="true"`. Task 1 + Task 2 each committed test code + production code together as a single `feat(79-04)` commit. Task 3 is a `chore(79-04)` (version bump + CHANGELOG only — no test code, no production code, no TDD cycle applies). The atomic-commit rationale documented in 79-01-SUMMARY, 79-02-SUMMARY, 79-03-SUMMARY, 79-05-SUMMARY applies: Phase 79 ships test+impl coupled at git-bisect granularity per the established Phase 76 IamConfig + Phase 79 Wave-{0,1,2,3} precedent.

## Threat Flags

None. The threat model in the plan (T-79-01, T-79-12, T-79-13) was implemented as documented:

- **T-79-01** (TOML parser DoS): mitigated via `fuzz_widgets_config.rs` (Task 2). Compiles on stable; `cargo +nightly fuzz run fuzz_widgets_config -- -max_total_time=60` is the smoke-test contract per the plan.
- **T-79-12** (doctor walks ALL `src/*.rs`): accepted per plan. Doctor only READS files; never executes; user already trusts their own checkout. The `is_ignored_dir` helper additionally skips `target/`, `node_modules/`, `.git/` for performance.
- **T-79-13** (doctor warning prints crate paths to stderr): accepted per plan. Crate paths are workspace-relative and already user-known; no secrets disclosed.

Additionally inherited from prior waves:
- **T-79-05** (OnFailure rollback user-expectation gap): closed via HARD-REJECT at parse time (REVISION 3 HIGH-G2). Test `deploy_on_test_failure_rollback_hard_rejected` in this wave's cli_acceptance suite locks the verbatim error.

## Self-Check: PASSED

- [x] `cargo-pmcp/src/main.rs` modified — VERIFIED via `git diff --name-only bfe5665c~1 bfe5665c`
- [x] `cargo-pmcp/src/commands/doctor.rs` modified (+267 lines) — VERIFIED via `git diff --stat`
- [x] `cargo-pmcp/src/commands/app.rs` modified (+18 lines) — VERIFIED
- [x] `cargo-pmcp/src/templates/mcp_app.rs` modified (+387 lines) — VERIFIED
- [x] `cargo-pmcp/tests/cli_acceptance.rs` modified (+82 lines) — VERIFIED
- [x] `cargo-pmcp/examples/widget_prebuild_demo.rs` created — VERIFIED via `ls`
- [x] `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` created — VERIFIED
- [x] `cargo-pmcp/fuzz/Cargo.toml` modified (new [[bin]]) — VERIFIED
- [x] `cargo-pmcp/Cargo.toml` version "0.12.0" — VERIFIED via `grep -c`
- [x] `crates/mcp-tester/Cargo.toml` version "0.6.0" — VERIFIED
- [x] `crates/mcp-tester/CHANGELOG.md` created with `## [0.6.0]` entry — VERIFIED
- [x] `cargo-pmcp/CHANGELOG.md` `## [0.12.0]` entry present — VERIFIED
- [x] `crates/pmcp-server/Cargo.toml` mcp-tester pin bumped to "0.6.0" — VERIFIED (Rule 3 auto-fix)
- [x] Commit `bfe5665c` (Task 1) — FOUND in git log
- [x] Commit `88d4d84e` (Task 2) — FOUND in git log
- [x] Commit `d7ede7a3` (Task 3) — FOUND in git log
- [x] `make quality-gate` exit 0 — VERIFIED
- [x] `cargo test -p cargo-pmcp -- --test-threads=1` exit 0 with 1064 passed across 17 suites — VERIFIED
- [x] `cargo test -p mcp-tester -- --test-threads=1` exit 0 with 205 passed across 9 suites — VERIFIED
- [x] `cargo test -p pmcp-server -- --test-threads=1` exit 0 with 9 passed across 3 suites — VERIFIED
- [x] `cargo run -p cargo-pmcp --example widget_prebuild_demo` exit 0 — VERIFIED
- [x] `cargo build -p cargo-pmcp --examples` succeeds — VERIFIED
- [x] `cd cargo-pmcp/fuzz && cargo build --bin fuzz_widgets_config` exit 0 — VERIFIED
- [x] Protected dirty files snapshot UNCHANGED at end of run — VERIFIED (4-line `git status --short` matches the spawn-time snapshot verbatim)
- [x] Zero `git stash` invocations across the wave — VERIFIED via self-audit of all Bash tool calls
- [x] All 5 REQ-79-{06, 07, 08, 12, 18} requirements satisfied — VERIFIED via plan frontmatter + acceptance tests above
