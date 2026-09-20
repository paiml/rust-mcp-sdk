---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 05
subsystem: infra
tags: [cargo-pmcp, deploy, lambda, pmcp-run, assets, secrets, config-driven, sql-server, scaffold]

requires:
  - phase: 86-01
    provides: "deploy single-crate-resolution spike (find_lambda_package_dir bails on single-crate; seam at builder.rs:132; deploy.toml must set target_type=pmcp-run; bundler config.toml@root + assets/<rel>)"
  - phase: 86-03
    provides: "cargo pmcp new --kind sql-server scaffold emitter (templates/sql_server.rs) — the file extended here with generate_deploy_toml"
provides:
  - "find_lambda_package_dir single-crate config-driven fallback (project root IS the Lambda package) — H3"
  - "is_config_driven_project detection seam in deploy/mod.rs (config.toml + schema.sql + pmcp-server-toolkit dep) — M3"
  - "scaffold-emitted deploy.toml (root + .pmcp/) with target_type=pmcp-run + [assets] include=[config.toml,schema.sql] — M3/H1"
  - "H4 bundled-secret substitution: bundle_assets_if_configured rewrites token_secret to ${CODE_MODE_SECRET} for config-driven projects (on-disk config untouched)"
  - "deploy_config_driven.rs integration test (D-10 compile-time enum guard + deploy.toml parse) + in-module packaging/secret-posture test"
affects: [86-06, deploy, pmcp-run]

tech-stack:
  added: []
  patterns:
    - "Additive single-crate Lambda-package fallback: existing <server>-lambda + *-lambda paths tried FIRST; project-root-as-package only when no wrapper found (no regression)"
    - "Detection-based deploy routing with ZERO TargetEntry enum change (D-10) — pmcp.run selected purely by deploy.toml target_type (M3)"
    - "Bundle-time secret rewrite (bytes-in/bytes-out) so the deployed artifact never ships the inline DEV secret while the local config keeps it for cargo run (D-06/H4)"
    - "Compile-time enum regression guard: exhaustive match with no wildcard arm trips on variant add/rename (D-10)"
    - "M3 in-module testability: bin-only pub(crate)/private helpers unit-tested in-module, not from integration tests"

key-files:
  created:
    - "cargo-pmcp/tests/deploy_config_driven.rs (D-10 enum guard + deploy.toml parse test)"
  modified:
    - "cargo-pmcp/src/deployment/builder.rs (single-crate fallback + helpers + H4 secret rewrite + in-module H3/H4 tests)"
    - "cargo-pmcp/src/commands/deploy/mod.rs (is_config_driven_project seam + None-arm note + in-module detection tests)"
    - "cargo-pmcp/src/templates/sql_server.rs (generate_deploy_toml)"

key-decisions:
  - "[Plan 86-05] H4 mechanism = (b) in-bundle rewrite, NOT a separate config.deploy.toml: bundle_assets_if_configured calls sanitize_config_bytes_for_deploy (bytes-in/out) on BOTH the zip-root config.toml and the runtime-read assets/config.toml when is_config_driven_project(root) is true. The on-disk local config.toml is never modified (D-06)."
  - "[Plan 86-05] deploy.toml is emitted to BOTH the project root (deploy.toml, human-visible + the Task-2 verify grep target) AND .pmcp/deploy.toml (the path DeployConfig::load reads). Identical content; the .pmcp copy is what cargo pmcp deploy + the parse test load."
  - "[Plan 86-05] find_lambda_package_dir's workspace-member search is now non-fatal: a cargo-metadata error (e.g. a single-crate project that does not resolve as a workspace) falls through to the single-crate fallback instead of bailing early. Extracted find_workspace_lambda_package_dir (Option-returning) to keep cog ≤25."
  - "[Plan 86-05] is_config_driven_project + find_lambda_package_dir single-crate resolution are unit-tested IN-MODULE (M3); the integration test only carries the compile-time D-10 enum guard + the DeployConfig::load parse assertion. The H4 packaging/zip-inspection test is in-module in builder.rs because bundle_assets_if_configured is private (the plan's documented fallback)."
  - "[Plan 86-05] ZERO TargetEntry enum change (D-10) — git diff of configure/config.rs is empty; pmcp.run is selected only by the scaffold's target_type=pmcp-run (get_target_id has no shape inference, per the 86-01 spike)."

patterns-established:
  - "Single-crate config-driven project = its own Lambda package (project root); detected via [package] manifest + config.toml + schema.sql markers"
  - "Deploy-time secret posture: inline DEV literal locally, ${CODE_MODE_SECRET} env ref in the bundled artifact"

requirements-completed: [SHAP-D-01]

duration: 18min
completed: 2026-05-27
---

# Phase 86 Plan 05: Shape D — Config-Driven Deploy (single-crate Lambda + pmcp-run + secret posture) Summary

**`cargo pmcp deploy` now packages a config-driven single-crate SQL server to pmcp.run via the existing Lambda build + asset bundle — detection-based, ZERO `TargetEntry` enum changes, with the project root resolved as its own Lambda package (H3), `[assets]` bundling config.toml + schema.sql to `/var/task/assets/` (H1), and the bundled `token_secret` rewritten to `${CODE_MODE_SECRET}` so the deployed artifact never ships the inline DEV secret (H4).**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-05-27T08:07:04-07:00 (first task commit)
- **Completed:** 2026-05-27T08:24:57-07:00 (last task commit)
- **Tasks:** 3
- **Files modified:** 4 (3 modified, 1 created)

## Accomplishments

- **H3 — single-crate Lambda build:** `find_lambda_package_dir` (builder.rs) gains an additive single-crate-root fallback (the project root IS the Lambda package) after the existing `<server>-lambda` + `*-lambda` paths, so a `cargo pmcp new --kind sql-server` project deploys without a wrapper crate. Extracted `find_workspace_lambda_package_dir` (Option-returning, metadata errors non-fatal) + `is_single_crate_config_root` to keep cognitive complexity low.
- **M3 — detection seam:** `is_config_driven_project` (deploy/mod.rs, `pub(crate)`) detects the D-09 heuristic (config.toml + schema.sql + a `pmcp-server-toolkit` dep) and emits an informational note in the deploy `None` arm. No new code path, no enum variant — the existing `[assets]`-bundling + single-crate fallback handle the layout; pmcp.run is selected purely by `target_type=pmcp-run`.
- **M3/H1 — scaffold deploy.toml:** `generate_deploy_toml` (sql_server.rs) emits a complete, parseable `DeployConfig` to both `deploy.toml` (root) and `.pmcp/deploy.toml` (the `DeployConfig::load` path) with `target_type="pmcp-run"`, `[assets] include = ["config.toml","schema.sql"]`, and comments documenting the `/var/task/assets/` + `/tmp/demo.db` + `${CODE_MODE_SECRET}` Lambda posture.
- **H4 — secret posture:** `bundle_assets_if_configured` rewrites the BUNDLED config's `token_secret` to `${CODE_MODE_SECRET}` (new `sanitize_config_bytes_for_deploy` helper) for both the zip-root `config.toml` and the runtime-read `assets/config.toml`, gated on `is_config_driven_project`. The on-disk local config keeps the inline DEV secret for out-of-box `cargo run` (D-06).
- **D-10 — regression guard + tests:** a compile-time exhaustive-match guard over the four `TargetEntry` variants, a deploy.toml parse test (target_type + assets.include), and an in-module non-cloud packaging test proving the zip carries `assets/{config.toml,schema.sql}` and that NEITHER bundled config copy contains the dev-secret literal while BOTH carry `${CODE_MODE_SECRET}`.

## Task Commits

1. **Task 1: single-crate Lambda resolution (H3) + detection seam (M3)** — `c7def757` (feat)
2. **Task 2: emit pmcp-run deploy.toml + H4 bundled-secret substitution** — `0f94ffc9` (feat)
3. **Task 3: D-10 enum guard + deploy.toml parse + H4 packaging/secret-posture tests** — `7b4d93a7` (test)

## Files Created/Modified

- `cargo-pmcp/src/deployment/builder.rs` — single-crate fallback in `find_lambda_package_dir`; `find_workspace_lambda_package_dir` + `is_single_crate_config_root` helpers; `add_config_toml_to_zip` gains a `config_driven` flag; new `sanitize_config_bytes_for_deploy`; asset-loop `config.toml` rewrite; 3 in-module H3 tests + 1 in-module H4 packaging test.
- `cargo-pmcp/src/commands/deploy/mod.rs` — `pub(crate) is_config_driven_project`; informational note in the `None` deploy arm; 3 in-module detection unit tests (M3).
- `cargo-pmcp/src/templates/sql_server.rs` — `generate_deploy_toml` wired into `generate`; emits `deploy.toml` + `.pmcp/deploy.toml`.
- `cargo-pmcp/tests/deploy_config_driven.rs` (new) — `target_entry_enum_unchanged` (compile-time D-10 guard) + `emitted_deploy_toml_parses_and_selects_pmcp_run`.

## Verification

| Check | Result |
|-------|--------|
| `cargo build -p cargo-pmcp` | success |
| `cargo test -p cargo-pmcp --test deploy_config_driven -- --test-threads=1` | 2 passed (D-10 guard + deploy.toml parse) |
| `cargo test -p cargo-pmcp --bins find_lambda_package_dir -- --test-threads=1` | 3 passed (single-crate resolves, *-lambda unchanged, bare root bails) |
| `cargo test -p cargo-pmcp --bins config_driven_detection -- --test-threads=1` | 3 passed (M3 detection) |
| `cargo test -p cargo-pmcp --bins bundled_artifact -- --test-threads=1` | 1 passed (H4 packaging/secret posture) |
| `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` | 1 passed (no regression — extra emitted file unbroken) |
| Task-2 verify: scaffold + grep pmcp-run/assets/config.toml/schema.sql/CODE_MODE_SECRET | `DEPLOY_TOML_OK`; local config.toml keeps the dev secret |
| `cargo fmt -p cargo-pmcp -- --check` | clean (touched files) |
| `cargo clippy -p cargo-pmcp --all-targets` | 0 errors (pre-existing pentest/banner dead-code warnings only) |
| `git diff cargo-pmcp/src/commands/configure/config.rs` (D-10) | empty (TargetEntry enum unchanged) |

## Decisions Made

See frontmatter `key-decisions`. Summary: H4 implemented as in-bundle rewrite (mechanism b); deploy.toml emitted to both root and `.pmcp/`; workspace-member search made non-fatal so the single-crate fallback is reachable; detection + single-crate resolution unit-tested in-module (M3), packaging/secret-posture test in-module (private `bundle_assets_if_configured`); zero `TargetEntry` change (D-10).

## Deviations from Plan

None — plan executed as written. All four review HIGHs/MEDIUM (H1/H3/H4/M3) were resolved by the planned mechanisms. Two within-plan implementation choices the plan explicitly left open were made and recorded:

- **H4 mechanism:** the plan offered (a) a separate `config.deploy.toml` or (b) an in-bundle rewrite. Chose **(b)** — a single rewrite point in `bundle_assets_if_configured` keeps the scaffold's file set minimal and is exactly what the Task-3 packaging test drives.
- **deploy.toml location:** the plan's Task-2 verify greps a root `deploy.toml` while Task-3 uses `DeployConfig::load` (which reads `.pmcp/deploy.toml`). Emitted to **both** with identical content to satisfy both surfaces.

## Issues Encountered

- **`detect_workspace_binaries` (cargo metadata) would `bail!` before the single-crate fallback.** A single-crate config-driven tempdir does not resolve as a workspace, so the original `?` propagation blocked H3. Restructured the workspace search into `find_workspace_lambda_package_dir` (Option-returning, `.ok()?` on metadata) so a metadata error falls through to the single-crate fallback rather than erroring. Resolved within Task 1.
- **cargo-pmcp tests live in the `[[bin]]` target, not `--lib`.** The in-module unit tests are run via `--bins` (the integration test via `--test deploy_config_driven`). Noted for the verifier.

## Pre-existing Issues (out of scope — NOT fixed)

Per the orchestrator's build-efficiency note and consistent with the 86-01 deferred-items log:

- `make quality-gate` has PRE-EXISTING unrelated failures: Phase 84 connector-crate rustfmt drift (`pmcp-toolkit-{athena,mysql,postgres}`), a `code_mode.rs:520` clippy lint from 85-10, and dead-code warnings in `cargo-pmcp/src/pentest/` + `banner.rs`. None are touched by this plan; all four files THIS plan modified are fmt-clean and clippy-clean (0 errors). The two benign pre-existing working-tree edits (test-file rustfmt reflows + a `config.json` flag edit) were left untouched and NOT staged.

## Known Stubs

None — the deploy path is fully wired: detection drives bundling, the scaffold emits a real parseable deploy.toml, and the secret rewrite produces real bundled bytes. The real cloud deploy round-trip (TEST-06) is intentionally deferred to the env-gated Plan 06 (not a stub — a gated end-to-end test).

## Next Phase Readiness

- Shape D is complete for the non-cloud surface: SHAP-D-01 closed. `cargo pmcp deploy` packages a config-only single-crate server with correct asset/DB/secret posture and no breaking changes.
- Plan 06 (TEST-06) can now run the env-gated real deploy round-trip against pmcp.run using the emitted deploy.toml + the bundled artifact this plan produces.

## Self-Check: PASSED

- `cargo-pmcp/tests/deploy_config_driven.rs` — FOUND
- `cargo-pmcp/src/deployment/builder.rs` — FOUND
- `cargo-pmcp/src/commands/deploy/mod.rs` — FOUND
- `cargo-pmcp/src/templates/sql_server.rs` — FOUND
- `.planning/phases/86-shapes-b-c-d-scaffold-library-example-deploy/86-05-SUMMARY.md` — FOUND
- commit `c7def757` — FOUND
- commit `0f94ffc9` — FOUND
- commit `7b4d93a7` — FOUND

---
*Phase: 86-shapes-b-c-d-scaffold-library-example-deploy*
*Completed: 2026-05-27*
