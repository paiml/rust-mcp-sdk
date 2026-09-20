---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 06
subsystem: testing
tags: [cargo-pmcp, deploy, pmcp-run, integration-test, env-gated, lambda, post-deploy-lifecycle, sql-server, scaffold]

requires:
  - phase: 86-04
    provides: "shared tests/support/scaffold_patch.rs — append_crates_io_patch ([patch.crates-io] writer for the unpublished toolkit) + ChildGuard (Drop-kill subprocess guard) + repo_root (M1 reuse)"
  - phase: 86-05
    provides: "config-driven deploy path: single-crate Lambda fallback (H3), is_config_driven_project detection (M3), scaffold-emitted .pmcp/deploy.toml target_type=pmcp-run + [assets], H4 ${CODE_MODE_SECRET} bundled-secret substitution"
provides:
  - "TEST-06: env-gated (PMCP_RUN_DEPLOY_TEST) + #[ignore] integration test that scaffolds a config-only server and deploys it to a REAL pmcp.run target via the real cargo-pmcp binary subprocess, asserting the Phase 79 post-deploy lifecycle (connectivity + conformance + apps) ran clean"
  - "Double-gate skip pattern for cloud-deploy tests: env-var early-return (eprintln + return, never fails) AND #[ignore], so normal CI stays green without creds (D-11 / SC-4 deliverable)"
affects: [deploy, pmcp-run, ci]

tech-stack:
  added: []
  patterns:
    - "Double-gated authentic cloud test: PMCP_RUN_DEPLOY_TEST env early-return (npm_skip_gate idiom) + #[ignore] (defense in depth) — the deploy is only constructed under `--ignored` WITH the env var set"
    - "Drive a process-exiting command via a REAL binary SUBPROCESS and assert exit code (M1) instead of an in-process call that could std::process::exit and abort the test harness"
    - "Reuse the shared scaffold_patch support module (append_crates_io_patch + ChildGuard) across both scaffold-to-serve (TEST-05) and scaffold-to-deploy (TEST-06) — written once"
    - "Disposable per-run server name (nanosecond suffix) + best-effort `deploy destroy --yes` teardown tolerating logged cleanup failure (cost/DoS mitigation)"

key-files:
  created:
    - "cargo-pmcp/tests/deploy_config_only.rs (env-gated + #[ignore] real pmcp.run config-only deploy integration test)"
  modified: []

key-decisions:
  - "[Plan 86-06] Capture the deployed URL from <crate>/.pmcp/deployment.toml's `[deployment].endpoint` (save_deployment_info, deploy/mod.rs:1072) via a small toml::Value parse, then ALSO call run_post_deploy_tests(endpoint, \"pmcp-run\", false, &Default, false) for an explicit second Phase-79 confirmation against the live endpoint (the deploy subprocess already ran the lifecycle; exit-0 is the primary assertion)."
  - "[Plan 86-06] Drive deploy with a bare `cargo pmcp deploy` (no subcommand) — the deploy action is Option<DeployAction> and the None arm (deploy/mod.rs:852) runs the deploy + the Phase 79 post-deploy lifecycle. The scaffold's .pmcp/deploy.toml already selects target_type=pmcp-run (Plan 05), so no extra flags are needed."
  - "[Plan 86-06] Wait on the deploy child explicitly for its exit code, but wrap it in ChildGuard FIRST so a panic between spawn and wait cannot leak the process; ChildGuard's Drop kill+wait both tolerate an already-reaped child (M1)."
  - "[Plan 86-06] widgets_present=false in the explicit run_post_deploy_tests call — a config-only SQL server ships no widgets, so the apps/widget step has nothing to pre-build."
  - "[Plan 86-06] NO always-on mock added (D-11) — the gated test is the SC-4 deliverable; no new deploy subsystem code (reuses the Plan 05 path entirely)."

patterns-established:
  - "Cloud-deploy integration tests are double-gated (env var early-return + #[ignore]) so they are authentic when run by an operator with creds but invisible/green in normal CI"
  - "Assert a process-exiting command's success via subprocess exit code rather than an in-process call"

requirements-completed: [TEST-06]

duration: 9min
completed: 2026-05-27
---

# Phase 86 Plan 06: TEST-06 — Env-Gated Real pmcp.run Config-Only Deploy Summary

**`cargo-pmcp/tests/deploy_config_only.rs` is an env-gated (`PMCP_RUN_DEPLOY_TEST`) + `#[ignore]` integration test that scaffolds a config-only SQL server, deploys it to a REAL pmcp.run target by spawning the actual `cargo-pmcp` binary as a subprocess (reusing the Plan 04 `[patch.crates-io]` writer + `ChildGuard`), asserts exit-0 (⇒ the Phase 79 post-deploy lifecycle of connectivity + conformance + apps ran clean), captures the deployed URL from `.pmcp/deployment.toml`, re-confirms the lifecycle explicitly, and best-effort tears down a disposable server — skipping cleanly (never failing) in normal CI without creds (SC-4 / D-11).**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-05-27T16:02:30Z
- **Completed:** 2026-05-27T16:12:14Z
- **Tasks:** 1
- **Files modified:** 1 (1 created)

## Accomplishments

- **Double-gate (T-86-06-01):** the deploy activates ONLY when `PMCP_RUN_DEPLOY_TEST` is set (the `widgets_orchestrator.rs:36-45` `npm_skip_gate` idiom — `deploy_gate()` returns `Some(reason)` → `eprintln! + return`, never `panic!`/`assert!`) AND the test carries `#[ignore]`. A bare `cargo test` neither constructs the deploy path nor fails; even under `--ignored` the env-gate still short-circuits without creds.
- **M1 — real-binary subprocess:** the gated-on path drives deploy by spawning `env!("CARGO_BIN_EXE_cargo-pmcp")` `deploy` in the scaffolded crate dir and asserts exit code 0. It does NOT call the deploy fn in-process (which may `std::process::exit` on a post-deploy failure and abort the test process). A clean exit means the command's own `run_post_deploy_tests` (connectivity + conformance + apps) ran clean.
- **M1 reuse:** pulls in `#[path = "support/scaffold_patch.rs"] mod scaffold_patch;` and reuses `append_crates_io_patch` (so `cargo lambda build` inside the deploy resolves the unpublished `pmcp-server-toolkit 0.1.0` + transitive workspace crates against in-repo paths) and `ChildGuard` (so a panic cannot leak the deploy subprocess).
- **URL capture + explicit lifecycle re-confirm:** reads `<crate>/.pmcp/deployment.toml`'s `[deployment].endpoint` (written by `save_deployment_info`, deploy/mod.rs:1072) and additionally calls `run_post_deploy_tests(endpoint, "pmcp-run", false, &PostDeployTestsConfig::default(), false)` asserting `Ok(())` against the live endpoint.
- **Teardown (T-86-06-03):** a disposable, per-run server name (`pmcp_deploy_test_<nanos>`) + a best-effort `cargo pmcp deploy destroy --yes` that tolerates and clearly LOGS a cleanup failure (does not fail the test — D-11 records that a deliberately-left server is acceptable).

## Task Commits

1. **Task 1: env-gated + #[ignore] real pmcp.run config-only deploy via real binary subprocess + Phase 79 lifecycle assertion** — `ebf3f570` (test)

## Files Created/Modified

- `cargo-pmcp/tests/deploy_config_only.rs` (new) — `deploy_gate()` env-gate; `disposable_server_name()`; `read_deployed_endpoint()` (toml parse of `.pmcp/deployment.toml`); `#[tokio::test] #[ignore] config_only_deploy_runs_phase79_lifecycle` driving scaffold → patch → deploy-subprocess (exit-0 assert) → URL capture → explicit `run_post_deploy_tests` Ok → best-effort destroy teardown. Module-level docs document the BOTH-required `PMCP_RUN_DEPLOY_TEST=1` + creds + `cargo lambda` (A5) and the exact operator run command.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cargo-pmcp --test deploy_config_only -- --test-threads=1` (CI default, gate absent) | `0 passed, 1 ignored` — SKIPPED via `#[ignore]`, no deploy attempted, clean exit |
| `cargo test -p cargo-pmcp --test deploy_config_only -- --ignored --test-threads=1 --nocapture` (gate absent) | `1 passed` — env-gate early-return prints `PMCP_RUN_DEPLOY_TEST not set — skipping…` and returns, never deploys (defense in depth) |
| `cargo fmt -p cargo-pmcp -- --check cargo-pmcp/tests/deploy_config_only.rs` | clean (after auto-format of match-arm trailing commas) |
| `cargo clippy -p cargo-pmcp --test deploy_config_only` | 0 errors (only pre-existing bin-target pentest dead-code warnings) |
| Source: gate keys on `PMCP_RUN_DEPLOY_TEST`, skip uses `eprintln! + return` (not `panic!`), test carries `#[ignore]` | confirmed |
| Source: deploy driven via `CARGO_BIN_EXE_cargo-pmcp` subprocess + exit-code assert; reuses `append_crates_io_patch` + `ChildGuard` | confirmed |
| Source: best-effort `deploy destroy --yes` teardown, disposable name, tolerates+logs cleanup failure | confirmed |
| No new deploy subsystem code; no always-on mock (D-11) | confirmed (test-only addition) |

**Manual-only (operator, requires creds — 86-VALIDATION.md row):** with `PMCP_RUN_DEPLOY_TEST=1` + live pmcp.run/AWS creds + `cargo lambda`, run:
```sh
PMCP_RUN_DEPLOY_TEST=1 cargo test -p cargo-pmcp --test deploy_config_only -- --ignored --test-threads=1 --nocapture
```
This deploys the disposable server, asserts exit-0 (Phase 79 lifecycle clean) + the explicit `run_post_deploy_tests` Ok against the captured endpoint, then tears it down. NOT run in this session (no creds available — D-11 / A5).

## Decisions Made

See frontmatter `key-decisions`. Summary: URL captured from `.pmcp/deployment.toml` `[deployment].endpoint` + explicit second `run_post_deploy_tests` confirmation; deploy driven by bare `cargo pmcp deploy` (None action arm runs the lifecycle); child wrapped in `ChildGuard` before the explicit `wait()`; `widgets_present=false` (config-only server); no mock, no new deploy code (D-11).

## Deviations from Plan

None - plan executed exactly as written. One within-plan choice the plan left optional was taken: the explicit `run_post_deploy_tests` second confirmation (step (d) "optionally") was INCLUDED for a stronger live-endpoint assertion on top of the primary exit-0 assertion.

## Issues Encountered

- **rustfmt match-arm trailing commas:** `cargo fmt --check` flagged missing trailing commas on the teardown `match` arms. Auto-formatted the file (`cargo fmt -p cargo-pmcp -- cargo-pmcp/tests/deploy_config_only.rs`); re-ran the test (still `0 passed, 1 ignored`). Cosmetic, no logic change.

## Pre-existing Issues (out of scope — NOT fixed)

Per the orchestrator's build-efficiency note and consistent with the 86-01/86-05 deferred-items logs:

- `make quality-gate` has PRE-EXISTING unrelated failures handled at phase-end: Phase 84 connector-crate rustfmt drift (`pmcp-toolkit-{athena,mysql,postgres}`), a `code_mode.rs:520` clippy lint from 85-10, and dead-code warnings in `cargo-pmcp/src/pentest/` + `banner.rs`. None are touched by this plan; the one file THIS plan created is fmt-clean and clippy-clean (0 errors). The two benign pre-existing working-tree edits (test-file rustfmt reflows + a `config.json` flag edit) were left untouched and NOT staged.

## Known Stubs

None — the test is a real (env-gated) end-to-end deploy round-trip, not a stub. The mock-free authentic path IS the SC-4 deliverable (D-11): the gate keeps it out of credential-less CI while preserving an executable, authentic proof for an operator with creds.

## Next Phase Readiness

- TEST-06 closes the SC-4 / D-11 deliverable: an authentic, double-gated config-only pmcp.run deploy test that is green-by-skip in CI and deployable-for-real with creds.
- Phase 86 Shapes B/C/D + scaffold + library example + deploy + the gated deploy test are now all in place; the phase verifier can run the full suite (the gated test skips cleanly).

## Self-Check: PASSED

- `cargo-pmcp/tests/deploy_config_only.rs` — FOUND
- `.planning/phases/86-shapes-b-c-d-scaffold-library-example-deploy/86-06-SUMMARY.md` — FOUND
- commit `ebf3f570` — FOUND

---
*Phase: 86-shapes-b-c-d-scaffold-library-example-deploy*
*Completed: 2026-05-27*
