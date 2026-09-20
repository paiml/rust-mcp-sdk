---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
verified: 2026-05-03T22:30:00Z
status: passed
score: 25/25 must-haves verified
dims_passed: 25
dims_total: 25
overrides_applied: 0
re_verification:
  previous_status: passed
  previous_score: 24/24
  prior_uat_gap: "UAT Test 3 (severity: major) — raw HTML / CDN-import widget pre-build crashed with raw os-error-2 and triggered npm install parent-walk audit"
  gaps_closed:
    - "Widget pre-build supports the documented zero-config raw HTML / CDN-import widget use case (single-file widgets without package.json)"
  gaps_remaining: []
  regressions: []
plan_executed: 79-06 (gap-closure)
---

# Phase 79: cargo pmcp deploy widget pre-build + post-deploy verification — Verification Report (RE-VERIFICATION)

**Phase Goal:** Close two silent-failure gaps in `cargo pmcp deploy` proven by Cost Coach: (A) deploy ships stale `widget/dist/*.html` because nobody ran `npm run build`; (B) Cargo's incremental cache holds a stale `include_str!`-built binary; (C) widget JS SDK is misconfigured but deploy reports success because nothing probes the live endpoint.

**Verified:** 2026-05-03 (re-verification after Plan 79-06 gap-closure)
**Status:** PASSED — all 25 verification dimensions match the codebase (24 from initial verification + 1 new dimension for the raw-HTML widget archetype guard added by Plan 79-06).
**Re-verification:** Yes — initial 79-VERIFICATION.md scored 24/24 then UAT discovered Test 3 regression (severity: major); Plan 79-06 closed it; this re-verification confirms the close.

## Re-verification Summary

| Aspect | Initial Verification | Plan 79-06 Gap Closure | This Re-verification |
|--------|---------------------|------------------------|----------------------|
| Score | 24/24 | n/a | **25/25** (+1 new dimension) |
| Status | passed | gap-closed | **passed** |
| UAT Test 3 | n/a | issue → pass | **pass** (verified) |
| cargo-pmcp tests | 1064 | 1083 | **1083** (verified) |
| cargo-pmcp version | 0.12.0 | 0.12.1 | **0.12.1** (verified) |
| New code added | n/a | is_node_project + 2 guard sites + 9 tests | **all present** |
| HIGH-G2/HIGH-C1/HIGH-G1 invariants | preserved | preserved | **preserved** (grep evidence) |

## Goal Achievement — Verification Dimension Matrix

### Build half (Failure Modes A + B) — 8 dimensions, all VERIFIED

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 1  | Widget convention `widget/`+`widgets/` ONLY  | VERIFIED   | `cargo-pmcp/src/deployment/widgets.rs` literal `["widget", "widgets"]`     |
| 2  | Lockfile-driven PM picker (bun>pnpm>yarn>npm) | VERIFIED  | `cargo-pmcp/src/deployment/widgets.rs` PackageManager::detect_from_dir     |
| 3  | `PMCP_WIDGET_DIRS` env-var (HIGH-C1)         | VERIFIED   | `cargo-pmcp/src/commands/deploy/mod.rs:545` `set_var` ONCE                  |
| 4  | build.rs template splits on `:` + local fallback (HIGH-G1+C1) | VERIFIED | `cargo-pmcp/src/templates/mcp_app.rs:109,130` `discover_local_widget_dirs` |
| 5  | `--no-widget-build`, `--widgets-only` flags  | VERIFIED   | `cargo-pmcp/src/commands/deploy/mod.rs` flags                               |
| 6  | `embedded_in_crates` explicit, not auto      | VERIFIED   | `cargo-pmcp/src/deployment/widgets.rs` `Vec<String>` schema field          |
| 7  | Yarn PnP `is_yarn_pnp` early-return (Codex MED) | VERIFIED | `cargo-pmcp/src/deployment/widgets.rs:317`                                  |
| 8  | `build`/`install` argv-vec schema (Codex MED) | VERIFIED  | `cargo-pmcp/src/deployment/widgets.rs` `Option<Vec<String>>`                |

### Verify half (Failure Mode C) — 10 dimensions, all VERIFIED

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 9  | Lifecycle warmup→check→conformance→apps      | VERIFIED   | `post_deploy_tests.rs` build_run_plan ordering, gated on `widgets_present` for apps |
| 10 | Subprocess approach via Tokio Command        | VERIFIED   | `post_deploy_tests.rs` Tokio Command spawn                                   |
| 11 | JSON consumer via `serde_json::from_str`     | VERIFIED   | `post_deploy_tests.rs` `serde_json::from_str::<PostDeployReport>` — NO regex |
| 11b | `parse_conformance_summary` / `parse_apps_summary` removed | VERIFIED | grep returns 0 matches across `cargo-pmcp/src/` and `crates/mcp-tester/src/` |
| 12 | Auth via env inheritance (HIGH-C2)           | VERIFIED   | `post_deploy_tests.rs` no `env_clear`, no `MCP_API_KEY` injection. `resolve_auth_token` does not exist (grep 0 matches) |
| 13 | Rollback hard-reject `OnFailure {Fail,Warn}` (HIGH-G2) | VERIFIED | `post_deploy_tests.rs:162` `ROLLBACK_REJECT_MESSAGE`, `:177` Deserialize, `:191` FromStr, `commands/deploy/mod.rs:221` clap value_parser |
| 14 | Exit codes BrokenButLive=3 / InfraError=2 (HIGH-2) | VERIFIED | `post_deploy_tests.rs` enum + exit_code() impl |
| 15 | CI annotation `::error::` when `CI=true`     | VERIFIED   | `post_deploy_tests.rs` `emit_ci_annotation` + `write_ci_annotation` |
| 16 | Failure banner "(N/M tests/widgets passed/failed)" + IS LIVE + rollback cmd | VERIFIED | `post_deploy_tests.rs` banner + noun dispatch (Apps→widgets, else→tests) |
| 17 | All 4 verify-half flags exist                | VERIFIED   | `commands/deploy/mod.rs` (`--no-post-deploy-test`, `--post-deploy-tests=`, `--on-test-failure=`, `--apps-mode=`) |
| 18 | InfraErrorKind = `{Subprocess,Timeout,AuthOrNetwork}` only | VERIFIED | `post_deploy_tests.rs` (no AuthMissing variant) |

### Cross-cutting — 6 dimensions, all VERIFIED

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 19 | Doctor `check_widget_rerun_if_changed` distinguishes WidgetDir vs include_str! | VERIFIED | `commands/doctor.rs` regex `r#"include_str!\s*\(\s*"[^"]*widgets?/[^"]*"\s*\)"#` |
| 20 | Scaffold opt-in `--embed-widgets` (Codex MED) | VERIFIED | `templates/mcp_app.rs` `embed_widgets: bool` param branches main.rs + writes build.rs only when true |
| 21 | Runnable example exists                      | VERIFIED   | `cargo-pmcp/examples/widget_prebuild_demo.rs` exists |
| 22 | Fuzz target exists                           | VERIFIED   | `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` exists; tests both DeployConfig parse + OnFailure rollback rejection |
| 23 | Version bumps applied                        | VERIFIED   | `cargo-pmcp/Cargo.toml:3 version = "0.12.1"` (was 0.12.0); `crates/mcp-tester/Cargo.toml:version = "0.6.0"`; both CHANGELOGs dated `## [0.12.1] - 2026-05-03` (cargo-pmcp) and `## [0.6.0] - 2026-05-03` (mcp-tester) |
| 24 | Quality gate green / build clean             | VERIFIED   | Orchestrator-confirmed: `make quality-gate` exit 0 in executor worktree (fmt + lint + build + test + audit + unused-deps + check-todos + check-unwraps + validate-always); cargo nextest workspace-wide 1409 tests pass |

### NEW: Plan 79-06 Gap Closure — 1 dimension, VERIFIED

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 25 | Raw-HTML / CDN-import widget archetype guard (closes UAT Test 3) | VERIFIED | `cargo-pmcp/src/deployment/widgets.rs:338` `fn is_node_project` (cog ≤2); `:403` early-return guard in `run_widget_build` (`if !is_node_project`); `:406` verbatim `"  treating {} as raw HTML / CDN bundle, skipping build"`; `:547` defense-in-depth `Path::is_file()` guard at head of `verify_build_script_exists`; `:549` friendly bail message `"widget dir {} has no package.json — add one with a 'build' script, configure widgets in .pmcp/deploy.toml, or remove the build = ... override"`; 6 unit tests U1..U6 at `:879..1112`; 3 integration tests I1..I3 at `cargo-pmcp/tests/widgets_raw_html.rs:61..161`; commits `7b3fe93a` + `28381f34`; cargo-pmcp 0.12.0→0.12.1 |

### Test totals (re-verified post-79-06)

| Crate / Suite     | Initial Claim | After 79-06 Claim | Re-verified Observed | Status   |
|-------------------|---------------|-------------------|----------------------|----------|
| cargo-pmcp suite  | 1064          | 1083              | **1083** (18 suites, 25.16s) | VERIFIED |
| cargo-pmcp deployment::widgets unit | 9 | 15 | **15** (1 suite, 379 filtered out) | VERIFIED |
| cargo-pmcp tests/widgets_raw_html | 0 | 3 | **3** (1 suite, 0.01s) | VERIFIED |
| Workspace nextest (orchestrator gate) | n/a | 1409 | **1409 pass, 0 failed, 2 skipped** | VERIFIED |
| mcp-tester suite | 205 | 205 (unchanged) | not re-run (no changes since initial verification) | INHERITED-PASS |

## Required Artifacts (Plan 79-06 deliverables)

| Artifact | Expected (per 79-06 must_haves) | Status | Details |
|----------|---------------------------------|--------|---------|
| `cargo-pmcp/src/deployment/widgets.rs` | `fn is_node_project` + raw-HTML early-return in `run_widget_build` + defense-in-depth file-existence guard at head of `verify_build_script_exists` + 6 new unit tests U1..U6 | VERIFIED | All present at lines 338, 403–411, 547–554, 879–1112 |
| `cargo-pmcp/Cargo.toml` | version bump 0.12.0 → 0.12.1 | VERIFIED | line 3: `version = "0.12.1"` |
| `cargo-pmcp/CHANGELOG.md` | `## [0.12.1] - 2026-05-03` entry under `### Fixed` describing raw-HTML regression closed + npm parent-walk eliminated + defense-in-depth diagnostic | VERIFIED | line 8 header, lines 11–28 Fixed entries, lines 30–38 Notes preserving REVISION 3 invariants |
| `cargo-pmcp/tests/widgets_raw_html.rs` | 3 integration tests (I1: archetype no-spawn, I2: explicit npm argv early-return, I3: Node-project baseline) | VERIFIED | 3 `#[tokio::test]` markers at :61, :107, :135 |
| `.planning/phases/79-.../79-UAT.md` | Test 3 flipped issue → pass with evidence reference | VERIFIED | Test 3 result: pass; evidence cites integration test I1; summary updated passed: 11 / issues: 0 |
| Initial-verification artifacts (14) | All preserved unchanged | VERIFIED | All file paths from initial verification's "Required Artifacts" table still present and substantive (verified via grep on REVISION 3 invariant strings + version metadata) |

## Key Link Verification (Goal-Backward Wiring) — Plan 79-06 additions

| From | To  | Via | Status |
|------|-----|-----|--------|
| `widgets.rs::run_widget_build` | `widgets.rs::is_node_project` | early-return guard before `PackageManager::detect_from_dir` → `ensure_node_modules` → `invoke_build_script` | WIRED (line 403: `if !is_node_project(&resolved.path)`) |
| `widgets.rs::verify_build_script_exists` | `Path::is_file()` defense-in-depth guard | `if !pkg_json_path.is_file() { bail!(...) }` BEFORE `std::fs::read_to_string` | WIRED (line 547) |
| `commands/deploy/mod.rs::pre_build_widgets_and_set_env` (UNCHANGED) | `PMCP_WIDGET_DIRS` env var (HIGH-C1 invariant) | `all_output_dirs.push(resolved.absolute_output_dir...)` collected for ALL widgets including raw-HTML ones (since early-return returns `Ok(resolved)`), then colon-joined and `set_var` ONCE | WIRED (lines 532–545) |

## Data-Flow Trace (Level 4) — Plan 79-06 critical paths

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|---------------------|--------|
| `is_node_project` | bool result | `widget_dir.join("package.json").is_file()` — direct filesystem syscall | YES (real fs.stat) | FLOWING |
| early-return branch | `Ok(resolved)` (ResolvedPaths) | `widget.resolve_paths(workspace_root)` — same path used by Node pipeline | YES (real PathBuf with absolute_output_dir) | FLOWING |
| `pre_build_widgets_and_set_env` | `all_output_dirs: Vec<String>` | `resolved.absolute_output_dir.to_string_lossy()` for ALL widgets including raw-HTML | YES (raw-HTML widgets ARE included in PMCP_WIDGET_DIRS) | FLOWING |
| `verify_build_script_exists` friendly bail | error string | `bail!(...)` with `widget_dir.display()` interpolated | YES (real path in error message) | FLOWING |

**Critical HIGH-C1 invariant proof:** the early-return path returns `Ok(resolved)` (NOT `Ok(())` or skip), so the caller's loop at `commands/deploy/mod.rs:531-536` still appends the raw-HTML widget's `absolute_output_dir` to `all_output_dirs`, which is then colon-joined and set as `PMCP_WIDGET_DIRS`. Verified by reading the call site code post-79-06: the loop is unchanged; only the callee `run_widget_build` short-circuits.

## Behavioral Spot-Checks (re-run for 79-06)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Plan 79-06 integration tests pass | `cargo test -p cargo-pmcp --test widgets_raw_html -- --test-threads=1` | `cargo test: 3 passed (1 suite, 0.01s)` | PASS |
| Widget unit tests pass (15: 9 pre + 6 new) | `cargo test -p cargo-pmcp --lib deployment::widgets:: -- --test-threads=1` | `cargo test: 15 passed, 379 filtered out (1 suite, 0.03s)` | PASS |
| Full cargo-pmcp suite (1083) | `cargo test -p cargo-pmcp -- --test-threads=1` | `cargo test: 1083 passed (18 suites, 25.16s)` | PASS |
| Workspace nextest (orchestrator gate) | `cargo nextest run --features full` | 1409 pass, 0 failed, 2 skipped | PASS |
| `make quality-gate` (orchestrator gate) | (in executor worktree) | exit 0 | PASS |
| `is_node_project` helper exists | `grep -n 'fn is_node_project' cargo-pmcp/src/deployment/widgets.rs` | line 338 | PASS |
| Verbatim raw-HTML skip line | `grep -n 'treating .* as raw HTML / CDN bundle' cargo-pmcp/src/deployment/widgets.rs` | line 406 | PASS |
| Friendly bail message | `grep -n 'has no package.json' cargo-pmcp/src/deployment/widgets.rs` | line 549 | PASS |
| CHANGELOG 0.12.1 entry | `grep -n '## \[0.12.1\] - 2026-05-03' cargo-pmcp/CHANGELOG.md` | line 8 | PASS |
| Cargo.toml version bump | `grep -n '^version = ' cargo-pmcp/Cargo.toml \| head -1` | `3:version = "0.12.1"` | PASS |
| Both gap-closure commits exist | `git log --format="%H %s" 7b3fe93a 28381f34` | `7b3fe93a fix(79-06)...` + `28381f34 chore(79-06)...` | PASS |
| HIGH-G2 invariant preserved | `grep -rn 'ROLLBACK_REJECT_MESSAGE' cargo-pmcp/src/` | 7 references unchanged in `post_deploy_tests.rs` and `commands/deploy/mod.rs` | PASS |
| HIGH-C1 invariant preserved | `grep -n 'PMCP_WIDGET_DIRS' cargo-pmcp/src/commands/deploy/mod.rs` | `set_var("PMCP_WIDGET_DIRS", &joined)` at line 545 unchanged | PASS |
| HIGH-G1 invariant preserved | `grep -rn 'discover_local_widget_dirs' cargo-pmcp/src/` | `templates/mcp_app.rs:109,130,1009,1044` unchanged | PASS |

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | No new SATD/TODO/FIXME/PLACEHOLDER markers in 79-06's modified files. The 4 grep matches for `placeholder` are test-fixture filenames (`node_modules/.placeholder`) used to short-circuit `ensure_node_modules` in unit/integration baseline tests — these are real fixtures, not stubs. |

Initial verification's INFO-level note about `cargo-pmcp/src/deployment/mod.rs:21-23` `unused imports` clippy warnings on the cargo-pmcp `--all-targets` lint scope continues to apply — pre-existing, not introduced by 79-06, and outside the root-crate `make lint` scope (so `make quality-gate` still exits 0).

No SATD comments, no TODO/FIXME placeholders, no `return null`/empty stubs, no hardcoded empty data flowing to user output. The new `is_node_project` helper is a pure boolean predicate; the early-return branch returns the SAME `ResolvedPaths` value that the Node pipeline would compute, so no data is dropped on the raw-HTML path.

## Requirements Coverage

All 18 REQ-79-* requirements (REQ-79-01 through REQ-79-18) declared in `79-00-PLAN.md` are accounted for:

- **Initial verification (Plans 79-01..79-05):** all 18 REQ-IDs satisfied via the dimension matrix #1–#24.
- **Plan 79-06 (gap closure):** declares `requirements: [REQ-79-01, REQ-79-02, REQ-79-03]` (the convention-detection + PM-picker + missing-build-script error-message requirements that the raw-HTML guard touches). All three remain satisfied — the new guard is purely additive (an early-return branch BEFORE these mechanisms fire), so the existing satisfaction evidence carries forward.

No orphaned requirements: every REQ-79-* in `79-00-PLAN.md` line-by-line is mapped to a wave in the requirement-to-plan mapping table at `79-00-PLAN.md:90-109`.

Phase has no entries in the global `REQUIREMENTS.md` numbered tracker (per `CONTEXT.md` "Implementation Decisions" — local-derivation noted in `79-00-PLAN.md:88` "Phase has no numbered REQ-IDs in REQUIREMENTS.md; suggested IDs are constructed locally for traceability per RESEARCH.md note"). All requirements verified against the local mapping.

## UAT Status

`79-UAT.md` final state (post-79-06):
- Total: 14
- Passed: **11** (was 10 pre-79-06)
- Issues: **0** (was 1 pre-79-06 — Test 3 closed)
- Skipped: 3 (Tests 9, 10, 14 — not blocking; require explicit operator UAT-on-real-project or `app new` scaffold integration check that wasn't covered in any wave)
- Blocked: 0
- Gap entry: status **closed**, closed_by `Plan 79-06 (commits 7b3fe93a + 28381f34)`, severity major

The 3 skipped UAT tests are non-blocking and are documented as such in the SUMMARY's "Next Phase Readiness" section. They do not affect phase goal achievement (the goal is closing Failure Modes A, B, C — all three are now closed; UAT Test 3 was the proof-of-fix for the regression in Failure Mode A's raw-HTML sub-case).

## Override Suggestions

None needed. All dimensions PASSED.

## Phase Self-Check

- [x] Initial 24/24 dimensions still VERIFIED (re-checked via grep on invariant strings + version metadata)
- [x] New 25th dimension (raw-HTML widget archetype guard) VERIFIED via direct file inspection + 9 new tests passing
- [x] All 14 required artifacts from initial verification still exist with substantive content
- [x] All Plan 79-06 deliverables (5 file changes + 9 new tests + 2 atomic commits) match the SUMMARY
- [x] All 7+ test files exist; new `widgets_raw_html.rs` integration test file added
- [x] All key links wired (3 new wirings from Plan 79-06: is_node_project early-return, defense-in-depth guard, unchanged PMCP_WIDGET_DIRS append)
- [x] Test totals match SUMMARY claims (1083 cargo-pmcp + workspace 1409 + 3 integration + 15 unit including new 6)
- [x] Release build clean / `make quality-gate` exit 0 (orchestrator-confirmed)
- [x] HIGH-G2 / HIGH-C1 / HIGH-G1 invariants preserved (grep-verified after 79-06)
- [x] Cargo-pmcp version bumped 0.12.0 → 0.12.1
- [x] CHANGELOG 0.12.1 entry under `### Fixed` with Notes preserving REVISION 3 invariants
- [x] UAT Test 3 flipped from issue → pass with evidence reference to integration test I1
- [x] Both gap-closure commits exist in git log (7b3fe93a fix + 28381f34 chore)
- [x] No new SATD / TODO / FIXME / placeholder markers introduced
- [x] No regressions in pre-existing 24 dimensions

## VERIFICATION PASSED

No HIGH gaps remain. All 25 verification dimensions match the contract:
- 24 pre-existing dimensions from the initial verification (all preserved through Plan 79-06).
- 1 new dimension (Plan 79-06 raw-HTML / CDN-import widget archetype guard) verified via direct code inspection, 9 new tests passing, two atomic commits, version bump, CHANGELOG entry, and UAT Test 3 status flip.

Phase 79 is goal-complete: Failure Modes A, B, and C are closed by code that exists, is substantive, is wired, and produces real data flow end-to-end. The previously-discovered UAT Test 3 regression (severity: major — raw HTML / CDN-import widget pre-build crashed with raw os-error-2 and triggered an `npm install` parent-walk audit of 1839 packages from a parent workspace) is now closed at the source (`is_node_project` early-return guard) with defense-in-depth (`Path::is_file()` guard at the head of `verify_build_script_exists`).

The Scientific-Calculator-MCP-App reproduction now exits cleanly: zero subprocesses spawned, no `node_modules/` or `package-lock.json` created in the project or any parent, the orchestrator's `PMCP_WIDGET_DIRS` env var still includes the raw-HTML widget's `dist/` path (HIGH-C1 invariant preserved), and the build.rs `cargo:rerun-if-changed` chain still rebuilds the binary on `*.html` edits.

**Release-ready:** cargo-pmcp 0.12.1 can be tagged + published via the existing `v*` tag → `release.yml` workflow per CLAUDE.md "Release Steps". The Release workflow's per-crate skip logic ensures only cargo-pmcp 0.12.1 publishes; pmcp 2.6.0, mcp-tester 0.6.0, and pmcp-widget-utils remain unchanged.

Phase 79 is ready for closeout.

---

_Re-verified: 2026-05-03T22:30:00Z_
_Verifier: Claude (gsd-verifier)_
_Plan executed: 79-06 (gap-closure for UAT Test 3, severity: major)_
_Initial verification: 79-VERIFICATION.md (2026-05-03, 24/24 dims passed)_
