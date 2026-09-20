---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
type: phase-aggregate
status: COMPLETE
waves: 5
total_commits: 16
total_tests_added: 121
total_duration_min: ~317
plans_executed: ["79-05 (Wave 0)", "79-01 (Wave 1)", "79-02 (Wave 2)", "79-03 (Wave 3)", "79-04 (Wave 4)"]
completed: 2026-05-03
crates_released:
  - "cargo-pmcp 0.11.0 → 0.12.0 (minor — additive)"
  - "mcp-tester 0.5.3 → 0.6.0 (minor — new public PostDeployReport module)"
crates_dep_pin_bumped:
  - "cargo-pmcp/Cargo.toml mcp-tester pin 0.5.3 → 0.6.0"
  - "crates/pmcp-server/Cargo.toml mcp-tester pin 0.5.0 → 0.6.0 (Rule 3 auto-fix in Wave 4)"
crates_unchanged:
  - "pmcp 2.6.0 (no core changes)"
  - "mcp-preview 0.3.0"
  - "pmcp-widget-utils"
  - "pmcp-server 0.2.2 (mcp-tester pin bumped but no public API change)"

requirements_completed_total: 18  # REQ-79-01..18 across all 5 plans
high_supersessions_applied: 6     # HIGH-1, HIGH-2, HIGH-C1, HIGH-C2, HIGH-G1, HIGH-G2
medium_supersessions_applied: 3   # scaffold opt-in, argv-vec, Yarn PnP detection
failure_modes_closed: ["A (build pre-Rust)", "B (Cargo cache, for include_str! crates)", "C (verify post-deploy)"]
---

# Phase 79: cargo pmcp deploy widget pre-build + post-deploy verification — Phase Aggregate Summary

**Phase 79 ships `cargo pmcp deploy` end-to-end widget pre-build orchestration + post-deploy live-endpoint verification, closing all three Failure Modes (A: stale widget bundle, B: Cargo incremental-cache miss for include_str! crates, C: broken-but-live deploy reported "successful"). 5 waves landed across 16 commits with 121 new tests; cargo-pmcp 0.12.0 + mcp-tester 0.6.0 release-ready; all 18 REQ-79-* requirements satisfied; all 6 REVISION 3 cross-AI HIGH supersessions applied; all 3 MEDIUM polish items folded in.**

## Wave-by-Wave Aggregate

| Wave | Plan  | Commits | Tests | Duration | Subsystem                                          | Outcome                                                                                          |
| ---- | ----- | ------- | ----- | -------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 0    | 79-05 | 3       | 18    | ~75 min  | mcp-tester `PostDeployReport` JSON contract        | New public module + `--format=<pretty|json>` flag on test commands; HIGH-1 supersession source   |
| 1    | 79-01 | 4       | 28    | ~60 min  | Schema types (`WidgetsConfig` + `PostDeployTests`) | Wave-1 schema + custom OnFailure Deserialize + ROLLBACK_REJECT_MESSAGE constant (HIGH-G2)        |
| 2    | 79-02 | 3       | 24    | ~75 min  | Widget pre-build orchestrator                      | `run_widget_build` + `PMCP_WIDGET_DIRS` colon-list (HIGH-C1) + `--no-widget-build`/`--widgets-only` |
| 3    | 79-03 | 3       | 39    | ~32 min  | Post-deploy verifier orchestrator                  | warmup→check→conformance→apps lifecycle + 4 CLI flags + clap rollback hard-reject (HIGH-G2)      |
| 4    | 79-04 | 3       | 12    | ~24 min  | Doctor + scaffold + example + fuzz + version bump  | check_widget_rerun_if_changed + --embed-widgets + build.rs template + 0.12.0 release             |
| **Total** | **5**   | **16**  | **121**   | **~266 min** | **Phase 79 build-half + verify-half end-to-end**       | **All 3 Failure Modes closed; all 6 HIGH + 3 MEDIUM cross-AI review items applied**                  |

(Total minutes is sum of wave durations; some waves had recovery overhead — see Deviations below for the two `git stash` incidents that did not corrupt anything but added time.)

## End-to-End Phase Outcome

### Failure Mode A: Build half, pre-Rust (CLOSED — Wave 2)

**Before Phase 79:** developer edited `widget/cost-over-time.html`, ran `cargo pmcp deploy`, and shipped the OLD widget because nobody ran `npm run build` — `widget/dist/*.html` was stale, `include_str!` happily inlined it, deploy reported success.

**After Phase 79:** Wave 2's `pre_build_widgets_and_set_env` runs `npm run build` (or PM equivalent — auto-detected from lockfile: `bun > pnpm > yarn > npm`) BEFORE `cargo build --release`. Build failure aborts the whole deploy. Empty-output WARN-but-don't-fail. Yarn PnP detection (REVISION 3 Codex MEDIUM) skips the spurious install heuristic.

**Closed in:** Plan 79-02 (commit `0cc13702`).

### Failure Mode B: Build half, Cargo cache (CLOSED for include_str! crates — Wave 4)

**Before Phase 79:** developer ran `npm run build`, verified mtime on the widget output was newer than `target/release/bootstrap`, ran `cargo pmcp deploy`. Cargo decided the binary was up-to-date because no `.rs` changed — `include_str!` is opaque to Cargo's dep tracker without `cargo:rerun-if-changed`. OLD binary re-shipped.

**After Phase 79:** Wave 2 sets `PMCP_WIDGET_DIRS` colon-separated list (REVISION 3 HIGH-C1; survives multi-widget projects, fixes the pre-revision last-widget-wins bug) before invoking `cargo build`. Wave 4's generated `build.rs` template (only emitted by `cargo pmcp app new --embed-widgets` — Codex MEDIUM scaffold opt-in) consumes the env var via `.split(':')` and emits per-dir + per-file `cargo:rerun-if-changed`. Direct `cargo run` / `cargo build` (without the deploy wrapper) still works via `discover_local_widget_dirs()` (REVISION 3 HIGH-G1) walking `CARGO_MANIFEST_DIR + ../widget|widgets/dist` up to 3 parents.

**`cargo pmcp doctor` `check_widget_rerun_if_changed`** warns when an `include_str!` crate lacks the matching `build.rs` (REVISION 3 Codex MEDIUM: WidgetDir crates do NOT trigger the warning because run-time file-serving has no Cargo cache problem). Pitfall 1 mitigated: warning text recommends `cargo clean -p <crate>` ONCE after adding the build.rs.

**Closed in:** Plan 79-04 (commit `bfe5665c`). Note: Failure Mode B does NOT apply to default WidgetDir scaffolds (run-time file serving) — only to `--embed-widgets` opt-in projects.

### Failure Mode C: Verify half, post-deploy (CLOSED — Wave 3)

**Before Phase 79:** widget bundle was correct and Lambda was live, but the JS SDK was misconfigured (missing `onteardown` etc.) — runtime broken, deploy reported "successful" anyway because deploy never probed the live endpoint. Caught by eyeballing screenshots, not by tooling.

**After Phase 79:** Wave 3's `run_post_deploy_tests` orchestrator runs warmup-grace (default 2000ms) → `cargo pmcp test check` → `cargo pmcp test conformance` → `cargo pmcp test apps --mode claude-desktop` (last only when widgets are present) BEFORE reporting deploy success. Subprocess argv passes `--format=json` (REVISION 3 HIGH-1, sourced from Wave 0) so the verifier consumes `mcp_tester::PostDeployReport` typed JSON — NO regex parsing of pretty terminal output.

**Exit codes (REVISION 3 HIGH-2):** 0 success, 2 infra-error (network/spawn/timeout), **3 broken-but-live** (post-deploy test verdict against the live revision). Distinct from infrastructure failures so CI/CD pipelines can decide rollback policy correctly. GitHub Actions / GitLab `::error::` annotation auto-emitted to stderr when `CI=true` env detected — augments the loud banner, doesn't replace it.

**`on_failure="rollback"` HARD-REJECTED at config-validation AND clap parse time** (REVISION 3 HIGH-G2). Verbatim error: `on_failure='rollback' is not yet implemented in this version of cargo-pmcp. Change to 'fail' (default) or 'warn'. Auto-rollback support will land in a future phase that verifies the existing DeployTarget::rollback() trait implementations.` Removes the UX trap where operators who explicitly configured "rollback" assumed rollback happened and ignored the broken-but-live state.

**Auth pass-through (REVISION 3 HIGH-C2):** subprocesses inherit parent env (Tokio Command default — no `env_clear()`) and resolve via the existing `AuthMethod::None` path which already supports Phase 74 OAuth cache + automatic refresh. NO parent-side `MCP_API_KEY` injection. NO `--api-key` argv. The pre-revision-3 `resolve_auth_token` helper was REMOVED entirely.

**Closed in:** Plan 79-03 (commit `0d6de1fa`).

## Cross-AI Review Supersessions Applied (REVISION 3)

All 6 HIGH findings + 3 MEDIUM polish items from the 2026-05-03 Codex (gpt-5) + Gemini (gemini-3-pro-preview) cross-AI review are landed:

| Finding   | Severity | Wave        | Mitigation                                                                                                  |
| --------- | -------- | ----------- | ----------------------------------------------------------------------------------------------------------- |
| HIGH-1    | HIGH     | 79-05 (W0) + 79-03 (W3) | NEW `mcp_tester::PostDeployReport` typed JSON contract; verifier consumes structured report, NO regex parsing |
| HIGH-2    | HIGH     | 79-03 (W3)  | Exit code 3 (broken-but-live) + GitHub Actions/GitLab `::error::` annotation when CI=true                   |
| HIGH-C1   | HIGH     | 79-02 (W2) + 79-04 (W4) | `PMCP_WIDGET_DIRS` colon-separated list (Unix `PATH` convention); build.rs splits on `:` and iterates       |
| HIGH-C2   | HIGH     | 79-03 (W3)  | Removed `resolve_auth_token` helper; subprocesses inherit parent env via Tokio default; existing `AuthMethod::None` Phase 74 cache + refresh path |
| HIGH-G1   | HIGH     | 79-04 (W4)  | `discover_local_widget_dirs()` fallback walking `CARGO_MANIFEST_DIR + ../widget|widgets/dist` up 3 parents  |
| HIGH-G2   | HIGH     | 79-01 (W1) + 79-03 (W3) + 79-04 (W4) | HARD-REJECT `on_failure="rollback"` at custom Deserialize AND at clap parse; verbatim ROLLBACK_REJECT_MESSAGE; fuzz target exercises rejection path |
| Codex MED — scaffold opt-in   | MEDIUM | 79-04 (W4) | `cargo pmcp app new --embed-widgets` opts into include_str! + writes build.rs; default stays WidgetDir; doctor distinguishes the two |
| Codex MED — argv arrays       | MEDIUM | 79-01 (W1) + 79-02 (W2) | `[[widgets]].build` / `.install` are `Option<Vec<String>>` argv arrays; `argv_to_cmd_args` does NO whitespace splitting |
| Codex MED — Yarn PnP          | MEDIUM | 79-02 (W2) | `is_yarn_pnp` detects `.pnp.cjs` / `.pnp.loader.mjs`; `ensure_node_modules` early-returns when either present |

## Requirements Completed (18/18)

All REQ-79-{01..18} satisfied across the 5 waves:

| REQ      | Description                                                                | Closed by      |
| -------- | -------------------------------------------------------------------------- | -------------- |
| REQ-79-01 | Auto-detect ONLY `widget/` and `widgets/` (NOT `ui/` or `app/`)            | 79-02 (W2)     |
| REQ-79-02 | Lockfile-driven PM detection (bun > pnpm > yarn > npm)                     | 79-01 + 79-02  |
| REQ-79-03 | `package.json` missing `build` script → verbatim actionable error          | 79-02 (W2)     |
| REQ-79-04 | Missing `node_modules/` runs auto-detected install                         | 79-02 (W2)     |
| REQ-79-05 | Widget build failure aborts entire deploy                                  | 79-02 (W2)     |
| REQ-79-06 | Generated `build.rs` consumes `PMCP_WIDGET_DIRS` (REVISION 3 list form)    | 79-04 (W4)     |
| REQ-79-07 | `cargo pmcp doctor` checks for missing `build.rs` on include_str! crates   | 79-04 (W4)     |
| REQ-79-08 | `cargo pmcp app new --embed-widgets` opt-in for include_str! scaffold      | 79-04 (W4)     |
| REQ-79-09 | `--no-widget-build` and `--widgets-only` CLI flags                         | 79-02 (W2)     |
| REQ-79-10 | `[[widgets]]` config block in `.pmcp/deploy.toml` with `embedded_in_crates` | 79-01 (W1)     |
| REQ-79-11 | `[post_deploy_tests]` config block + lifecycle (warmup → check → conformance → apps) | 79-01 + 79-03  |
| REQ-79-12 | OnFailure `{fail, warn}` with rollback HARD-REJECTED at parse + load       | 79-01 + 79-03 + 79-04 |
| REQ-79-13 | Subprocess auth pass-through via env inheritance (REVISION 3 HIGH-C2)      | 79-03 (W3)     |
| REQ-79-14 | Exit code 3 broken-but-live (REVISION 3 HIGH-2 — distinct from infra exit 2) | 79-03 (W3)     |
| REQ-79-15 | GitHub Actions `::error::` annotation when CI=true                         | 79-03 (W3)     |
| REQ-79-16 | `--no-post-deploy-test` / `--post-deploy-tests=` / `--on-test-failure=` / `--apps-mode=` flags | 79-03 (W3) |
| REQ-79-17 | `mcp_tester::PostDeployReport` JSON contract (REVISION 3 HIGH-1)           | 79-05 (W0)     |
| REQ-79-18 | `cargo pmcp deploy --help` mentions widgets verbatim per CONTEXT.md        | 79-04 (W4)     |

## CLAUDE.md ALWAYS-Required Testing Dimensions — ALL Covered

| Dimension | Implementation                                                                                                |
| --------- | ------------------------------------------------------------------------------------------------------------- |
| FUZZ      | `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` — adversarial TOML fuzz; ALSO exercises OnFailure rollback-rejection path (REVISION 3 HIGH-G2). Plan 79-04. |
| PROPERTY  | `cargo-pmcp/tests/widgets_config.rs::pm_detection_priority_order` — proptest invariant: lockfile presence drives PM choice in fixed bun > pnpm > yarn > npm order. Plan 79-01. |
| UNIT      | 60+ unit tests across all 5 plans. Highest concentration in Plan 79-03 (39 tests) covering the verify orchestrator. Plans 79-01/02/04 add 28+24+12 = 64 more. |
| EXAMPLE   | `cargo-pmcp/examples/widget_prebuild_demo.rs` — schema-direct demo. `cargo run -p cargo-pmcp --example widget_prebuild_demo` exits 0. Plan 79-04. |

## Crate Versions At Phase Close

| Crate              | Before    | After     | Bump kind | Rationale                                                                              |
| ------------------ | --------- | --------- | --------- | -------------------------------------------------------------------------------------- |
| cargo-pmcp         | 0.11.0    | **0.12.0** | minor     | Additive: new behavior, new flags, new config sections, new doctor check (Phase 79)    |
| mcp-tester         | 0.5.3     | **0.6.0**  | minor     | NEW public `post_deploy_report` module + `--format=json` flag (REVISION 3 HIGH-1)      |
| pmcp               | 2.6.0     | 2.6.0      | unchanged | No core changes                                                                         |
| mcp-preview        | 0.3.0     | 0.3.0      | unchanged | Untouched                                                                               |
| pmcp-widget-utils  | unchanged | unchanged  | unchanged | Untouched                                                                               |
| pmcp-server        | 0.2.2     | 0.2.2      | unchanged | mcp-tester dep pin bumped 0.5.0 → 0.6.0, but mcp-tester used internally only; pmcp-server's own public API unchanged |

CHANGELOG entries dated `2026-05-03`:
- `cargo-pmcp/CHANGELOG.md` `## [0.12.0]` — full Phase 79 surface area, all REVISION 3 supersessions documented + Security T-79-* mitigations + migration Notes for pre-0.12.0 projects.
- `crates/mcp-tester/CHANGELOG.md` `## [0.6.0]` — NEW FILE (this crate had no CHANGELOG before); documents new public module + flag.

## Deviations Across the Phase

### Wave-Level Deviations

- **Wave 0 (79-05)** — `git stash` incident (recovery successful, but 1 file lost mid-wave; rebuilt deterministically). Documented in 79-05-SUMMARY.
- **Wave 3 (79-03)** — second `git stash` incident; recovery was lossless. Documented in 79-03-SUMMARY.
- **Wave 4 (79-04)** — Rule 3 auto-fix: `crates/pmcp-server/Cargo.toml` mcp-tester dep pin needed bumping `0.5.0 → 0.6.0` after the mcp-tester release bump (semver `^0.5.0` excludes `0.6.0`). pmcp-server itself NOT bumped (mcp-tester used internally only — no public re-exports — so pmcp-server's public API is unchanged at 0.2.2 per CLAUDE.md release rules).

The two `git stash` incidents (Waves 0 + 3) were the catalyst for the hardened git-safety protocol enforced in Wave 4's spawn (zero `git stash` invocations in Wave 4; protected-files snapshot audit passed at every commit boundary).

### Plan-Spec Notes (no behavior deviation)

- **Wave 4 cli_acceptance test placement:** the 3 cli_acceptance tests planned for Task 2 (REQ-79-18 verbatim, HIGH-G2 rollback rejection, embed_widgets parse) shipped in Task 1's commit (`bfe5665c`) for cohesion with the help-text + flag-parsing changes. Net test count unchanged.
- **Wave 4 test count is 12, not 9:** added defense-in-depth tests (1.3b WidgetDir-silent, 1.3c mixed-warns, 1.5b/c unit-form template-content) that the plan called for in prose but didn't lift into separate test fns.

## Quality Gate State At Phase Close

```bash
make quality-gate           # exit 0 (run after Wave 4 Task 3 commit)
cargo test -p cargo-pmcp -- --test-threads=1   # 1064 passed (17 suites)
cargo test -p mcp-tester  -- --test-threads=1   # 205 passed (9 suites)
cargo test -p pmcp-server -- --test-threads=1   # 9 passed (3 suites)
cargo run -p cargo-pmcp --example widget_prebuild_demo   # exit 0
cd cargo-pmcp/fuzz && cargo build --bin fuzz_widgets_config   # exit 0
```

## Phase Self-Check: PASSED

- [x] All 5 waves complete with SUMMARY.md present at `.planning/phases/79-.../79-{00,01,02,03,04,05}-SUMMARY.md` (note: 79-00-PLAN.md is the master plan; only Waves 1..5 have execution summaries — Wave 0 is 79-05).
- [x] All 18 REQ-79-* requirements satisfied (verification table above).
- [x] All 6 REVISION 3 HIGH supersessions applied (verification table above).
- [x] All 3 REVISION 3 MEDIUM polish items folded in (verification table above).
- [x] All 3 Failure Modes A + B + C closed (Failure Mode B closed for include_str! crates only; default WidgetDir scaffolds don't have Failure Mode B).
- [x] cargo-pmcp 0.12.0 + mcp-tester 0.6.0 release-ready with dated CHANGELOG entries.
- [x] CLAUDE.md ALWAYS-required testing dimensions (FUZZ + PROPERTY + UNIT + EXAMPLE) ALL covered.
- [x] `make quality-gate` exits 0 at phase close.
- [x] Protected pentest/wasm-client WIP files snapshot UNCHANGED across all 5 waves' commits.

## Recommended Next Step

`/gsd-verify-work 79` to validate the phase end-to-end against the must_haves from all 5 plans.

Once verified, the release can proceed: tag `v0.12.0` for cargo-pmcp + `v0.6.0` for mcp-tester per CLAUDE.md release workflow (`git tag -a vX.Y.Z` + push to upstream → automated `release.yml` publishes to crates.io in dependency order).
