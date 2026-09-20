---
status: gap-closed
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
source: [79-SUMMARY.md, 79-01-SUMMARY.md, 79-02-SUMMARY.md, 79-03-SUMMARY.md, 79-04-SUMMARY.md, 79-05-SUMMARY.md, 79-06-SUMMARY.md]
started: 2026-05-03T00:00:00Z
updated: 2026-05-03T00:00:00Z
---

## Current Test

[gap closed — Test 3 fixed by Plan 79-06; 11 passed, 0 issues, 3 skipped without reason]

## Tests

### 1. Help text mentions widget pre-build (REQ-79-18)
expected: |
  Run `cargo pmcp deploy --help`. Output mentions widget pre-build
  behavior verbatim per CONTEXT.md (auto-detects widget/ and widgets/).
result: pass

### 2. All 6 new deploy flags appear in --help
expected: |
  Run `cargo pmcp deploy --help`. The following flags are listed:
  `--no-widget-build`, `--widgets-only`, `--no-post-deploy-test`,
  `--post-deploy-tests=<list>`, `--on-test-failure=<fail|warn>`,
  `--apps-mode=<mode>`.
result: pass

### 3. Widget pre-build runs before cargo build
expected: |
  In a project with `widget/package.json` and a `build` script, run
  `cargo pmcp deploy --widgets-only`. The detected package manager
  (bun > pnpm > yarn > npm based on lockfile) runs `<pm> run build`
  BEFORE any `cargo build`. Console output shows the build script's
  output, then exits.

  GAP-CLOSURE EXTENSION (Plan 79-06): for a `widgets/` directory
  containing only raw `*.html` files (no `package.json`), the
  orchestrator MUST treat it as a raw-HTML / CDN-import widget set,
  print `treating <path> as raw HTML / CDN bundle, skipping build`,
  spawn ZERO subprocesses, and still return Ok so the caller appends
  the directory to `PMCP_WIDGET_DIRS` for the build.rs cache
  invalidation chain.
result: pass
evidence: |
  Plan 79-06 closed the gap. Verified by integration test
  cargo-pmcp/tests/widgets_raw_html.rs::raw_html_widget_archetype_does_not_spawn_npm
  which mirrors the Scientific-Calculator-MCP-App reproduction:
  builds a tempdir-rooted workspace with `widgets/keypad.html`
  containing `import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps"`,
  no `package.json`, no lockfile. Calls `run_widget_build` directly
  (schema-direct). Asserts:
    - returns Ok(resolved)
    - resolved.absolute_output_dir = workspace_root/widgets/dist
    - widgets/node_modules does NOT exist (no npm install spawned)
    - widgets/package-lock.json does NOT exist
    - workspace_root/package-lock.json does NOT exist (no parent-walk)
  Test passes on a runner with NO npm/pnpm/yarn/bun on PATH — proof
  that no subprocess is attempted.

  Code changes (commit 7b3fe93a):
  - New `is_node_project(widget_dir)` helper using Path::is_file().
  - Early-return guard at the top of `run_widget_build` when
    `!is_node_project`.
  - Defense-in-depth Path::is_file() guard at the head of
    `verify_build_script_exists` replacing raw os-error-2 with a
    friendly diagnostic naming the directory and three remediation
    paths.

  cargo-pmcp version bumped 0.12.0 → 0.12.1 (commit 28381f34).
  CHANGELOG.md `## [0.12.1] - 2026-05-03` entry under `### Fixed`.

  Originally reported severity: major.

### 4. Missing node_modules triggers auto-install
expected: |
  In a `widget/` with `package.json` but no `node_modules/`, run
  `cargo pmcp deploy --widgets-only`. The detected PM runs `install`
  automatically, then runs `build`. (Yarn PnP projects with `.pnp.cjs`
  or `.pnp.loader.mjs` skip the install step.)
result: pass

### 5. Widget build failure aborts entire deploy
expected: |
  Force a failing widget build (e.g. broken `widget/build.js`).
  Run `cargo pmcp deploy`. The deploy aborts BEFORE `cargo build --release`
  runs. Error message is verbatim and actionable (mentions the failed
  widget script + exit status).
result: pass

### 6. on_failure="rollback" HARD-REJECTED at CLI flag (HIGH-G2)
expected: |
  Run `cargo pmcp deploy --on-test-failure=rollback`. clap rejects
  the value with the verbatim ROLLBACK_REJECT_MESSAGE: "on_failure='rollback'
  is not yet implemented in this version of cargo-pmcp. Change to 'fail'
  (default) or 'warn'. Auto-rollback support will land in a future phase
  that verifies the existing DeployTarget::rollback() trait implementations."
  Exit code is non-zero.
result: pass

### 7. on_failure="rollback" HARD-REJECTED at TOML parse (HIGH-G2)
expected: |
  Add `[post_deploy_tests]\non_failure = "rollback"` to
  `.pmcp/deploy.toml`. Run any `cargo pmcp deploy` invocation that
  loads that config. Config load fails with the same verbatim
  ROLLBACK_REJECT_MESSAGE before any deploy work begins.
result: pass

### 8. cargo pmcp doctor warns on missing build.rs for include_str! crates
expected: |
  In a crate that uses `include_str!("widgets/foo.html")` but has no
  `build.rs`, run `cargo pmcp doctor`. Output includes a WARNING that
  recommends `cargo clean -p <crate>` ONCE after adding the build.rs.
  In a default WidgetDir scaffold (no `include_str!`), the same check
  emits NO warning.
result: pass

### 9. cargo pmcp app new --embed-widgets writes build.rs scaffold
expected: |
  Run `cargo pmcp app new my_app --embed-widgets`. Generated project
  contains a `build.rs` file with `cargo:rerun-if-changed` directives
  that consume `PMCP_WIDGET_DIRS` (split on `:`) AND a local
  `discover_local_widget_dirs()` fallback walking `CARGO_MANIFEST_DIR`
  parents up to 3 levels.
result: skipped

### 10. cargo pmcp app new (default) does NOT include build.rs
expected: |
  Run `cargo pmcp app new my_app` (no `--embed-widgets`). Generated
  project uses runtime WidgetDir (file serving) and contains NO
  `build.rs` and NO `include_str!`. This is the default scaffold per
  Codex MED scaffold-opt-in.
result: skipped

### 11. Post-deploy lifecycle runs warmup → check → conformance → apps
expected: |
  Run `cargo pmcp deploy` against a deployable project with widgets.
  After Lambda hot-swap, console shows in order:
  warmup grace (default 2000ms) → `cargo pmcp test check` →
  `cargo pmcp test conformance` → `cargo pmcp test apps --mode claude-desktop`
  (apps step only when widgets present). All 4 steps consume
  `--format=json` (no regex parsing of pretty output).
result: pass

### 12. Failure banner format on broken-but-live (exit 3)
expected: |
  When post-deploy tests fail, the banner explicitly says the
  Lambda revision IS LIVE, includes the metric in
  `(passed/total noun passed)` or `(failed/total noun failed)`
  format (noun = "tests" or "widgets" depending on which step failed),
  and includes a rollback command. Process exits with code 3 (NOT 2,
  which is reserved for infra errors).
result: pass

### 13. CI=true emits ::error:: annotation on broken-but-live
expected: |
  Run a deploy that triggers a broken-but-live state with `CI=true`
  in env. stderr contains the GitHub Actions / GitLab annotation
  `::error::Deployment succeeded but post-deploy tests failed (exit code 3).
  Lambda revision is LIVE.` Banner is still printed (annotation
  augments, not replaces).
result: pass
evidence: |
  ::error::Deployment succeeded but post-deploy tests failed (exit code 3). Lambda revision is LIVE. To roll back: cargo pmcp deploy rollback --target pmcp-run

### 14. widget_prebuild_demo example runs to completion
expected: |
  Run `cargo run -p cargo-pmcp --example widget_prebuild_demo`.
  Process exits 0 and prints
  `=== Example complete — Phase 79 build half verified end-to-end ===`
  (or equivalent). Demo exercises the schema-direct path.
result: skipped

## Summary

total: 14
passed: 11
issues: 0
pending: 0
skipped: 3
blocked: 0

## Gaps

- truth: "Widget pre-build supports the documented zero-config raw HTML / CDN-import widget use case (single-file widgets without package.json)"
  status: closed
  reason: "User reported: widgets/ without package.json hard-crashes with raw OS error 2 (No such file or directory). CLI runs `npm install` unconditionally — npm walks up to a parent workspace, audits ~1839 packages, and may modify node_modules / package-lock.json outside the project. Reproduction: ~/projects/mcp/Scientific-Calculator-MCP-App"
  closed_by: "Plan 79-06 (commits 7b3fe93a + 28381f34) — `is_node_project` early-return guard + defense-in-depth + cargo-pmcp 0.12.1. Verified by integration test `cargo-pmcp/tests/widgets_raw_html.rs::raw_html_widget_archetype_does_not_spawn_npm`."
  severity: major
  test: 3
  root_cause: |
    cargo-pmcp/src/deployment/widgets.rs detect_widgets() (lines 269-289) synthesises a WidgetConfig for any
    `widget/` or `widgets/` directory that exists, with ZERO check for package.json. The synthesised config
    flows into run_widget_build() (line 362) → ensure_node_modules() (line 380) which only short-circuits on
    node_modules/ present OR Yarn-PnP markers (NOT on missing package.json) — so it spawns `npm install` in a
    dir with no manifest. Then verify_build_script_exists() (line 495) calls `fs::read_to_string(pkg_json_path)`
    which produces the raw "os error 2" the user saw. The friendly bail at line 502 is unreachable.

    Bug surface: Path A (deploy pre-build) AFFECTED. Path B (`cargo pmcp test apps`) NOT affected — it only
    walks *.html via scan_widgets_dir and never reads package.json or runs npm. The user observed the crash
    in the deploy pipeline before the test-apps subprocess was even spawned.

    npm parent-walk (1839 packages audited) is npm's own behavior when it can't find a local package.json,
    NOT a CWD bug in our code (spawn_streaming correctly sets current_dir to widgets/). Fix is to NOT spawn
    npm when local package.json is missing, not to change cwd.
  artifacts:
    - path: "cargo-pmcp/src/deployment/widgets.rs:269-289"
      issue: "detect_widgets synthesises Node-shaped config for any widget/widgets dir without checking package.json"
    - path: "cargo-pmcp/src/deployment/widgets.rs:362-374"
      issue: "run_widget_build enters Node pipeline unconditionally — no raw-HTML branch"
    - path: "cargo-pmcp/src/deployment/widgets.rs:380-404"
      issue: "ensure_node_modules spawns npm install without checking package.json (only checks node_modules/ + Yarn-PnP)"
    - path: "cargo-pmcp/src/deployment/widgets.rs:493-507"
      issue: "verify_build_script_exists raw read_to_string produces os error 2 instead of friendly diagnostic; bail at :502 is unreachable"
    - path: "cargo-pmcp/src/commands/deploy/mod.rs:522-547,880-890"
      issue: "call site that chains into the broken pipeline"
  missing:
    - "is_node_project(widget_dir) helper checking widget_dir.join('package.json').is_file()"
    - "Early-return in run_widget_build when !is_node_project — print 'treating as raw HTML / CDN bundle, skipping build' and still populate PMCP_WIDGET_DIRS"
    - "Defense-in-depth Path::is_file() guard inside verify_build_script_exists with friendly bail (covers explicit-argv path)"
    - "Optional: bail when explicit widget.build/widget.install argv starts with npm|pnpm|yarn|bun AND no package.json (warn user-take-the-wheel)"
    - "Unit test: run_widget_build against widget dir with only *.html, no package.json → Ok + skip-build console message"
    - "Integration test: documented Raw HTML / CDN widget archetype (keypad.html style)"
  debug_session: ""
