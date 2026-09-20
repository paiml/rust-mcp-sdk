# Phase 79: cargo pmcp deploy — widget pre-build + post-deploy verification — Context

**Gathered:** 2026-05-03
**Status:** Replanning (revision 3, review-driven)
**Source:** PRD-style synthesis from cost-coach proposal at `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-deploy-widget-build.md` + scope decisions locked during the `/gsd-phase --insert` conversation on 2026-05-03 + cross-AI review supersessions from `79-REVIEWS.md` (commit `714b5b2d`) on 2026-05-03.

## Review-Driven Supersessions (2026-05-03)

Cross-AI peer review by Codex (gpt-5) and Gemini (gemini-3-pro-preview) returned 2 HIGH-CONSENSUS + 3 reviewer-unique HIGH findings. Operator decisions on the two scope/policy questions:

1. **HIGH-1 (stdout-parsing brittleness) → SCOPE EXPANSION:** Add a new Plan **79-05** that lands `--format=json` on `cargo pmcp test {check, conformance, apps}` as a Wave-0 prerequisite. Wave 3 verifier consumes structured `TestReport` from the JSON output instead of regex-parsing pretty terminal text (which Codex confirmed doesn't even produce the strings the planner expected — pretty output is in `crates/mcp-tester/src/report.rs`, not the test commands themselves; `--quiet` suppresses output further). Wave structure becomes 5 waves (was 4): **Wave 0 (NEW): test commands `--format=json` flag** → Wave 1 schema → Wave 2 build orchestrator → Wave 3 verify orchestrator (now consumes JSON, not regex) → Wave 4 doctor + scaffold + example + version bump. ~+3 days estimate. Removes the entire `parse_conformance_summary` / `parse_apps_summary` / regex-fallback path; removes the F-2 verbatim banner brittleness; removes Codex's "verify-half parser argv mismatch" finding.

2. **HIGH-G2 (rollback UX trap) → POLICY CHANGE:** `on_failure="rollback"` is **hard-rejected at config validation**, not parsed-but-warned. See updated lock at "Verify half" section below. Loses forward-compat reservation; future phase that ships rollback support will add the variant + migration note.

3. **HIGH-C1 (multi-widget cache invalidation broken) → REPLAN:** Single `PMCP_WIDGET_DIR` env var doesn't survive multiple `[[widgets]]` entries. Replace with `PMCP_WIDGET_DIRS` (colon-separated list, Unix `PATH` convention) OR per-crate watcher generation from `embedded_in_crates`. Planner picks; Codex prefers the env list for simplicity.

4. **HIGH-C2 (resolve_auth_token fights existing auth) → REPLAN:** Strip the `resolve_auth_token` helper. Let child subprocesses inherit the deploy's env and resolve auth via the existing `AuthMethod::None` → Phase 74 cache → auto-refresh path in `cargo-pmcp/src/commands/auth.rs`. Only inject `MCP_API_KEY` when the user explicitly provides `--api-key` (currently a `pmcp test` flag the user can already set). Removes the static-bearer-token path that loses refresh behavior in CI/under-different-user scenarios. Removes the `InfraErrorKind::AuthMissing` variant added in revision 1 (no longer needed since subprocesses self-resolve).

5. **HIGH-G1 (build.rs breaks local cargo run) → REPLAN:** Generated `build.rs` template extends fallback path: when `PMCP_WIDGET_DIR(S)` unset, attempt local discovery via `CARGO_MANIFEST_DIR + ../widget|widgets` lookups (workspace-root-relative, walking up from the manifest dir). Restores local dev loop for `cargo run` / `cargo build` directly. Doctor check still warns when neither env var nor local-discovery hit succeeds.

6. **MEDIUM consensus polish items** (also for the planner to fold in):
   - **Scaffold target alignment** (Codex MEDIUM): current `cargo pmcp app new` template at `cargo-pmcp/src/templates/mcp_app.rs` uses `WidgetDir` file-serving (run-time read), NOT `include_str!`. Adding `build.rs` to the scaffold is mostly harmless but doesn't address Failure Mode B for new apps using the default scaffold. Doctor check should detect WidgetDir usage explicitly; scaffold task should clarify that `build.rs` is only for projects that opt into `include_str!` embedding.
   - **`build`/`install` argv schema** (Codex MEDIUM): change schema field type from `Option<String>` (whitespace-split breaks quoting) to `Option<Vec<String>>` (argv array). Or accept both and document the difference.
   - **`node_modules` heuristic** (Codex MEDIUM): Yarn PnP omits `node_modules` legitimately. Detect Yarn PnP via `.pnp.cjs` / `.pnp.loader.mjs` presence and skip the install heuristic when found.

**Decisions NOT changed by review:** the convention narrowing (`widget/` and `widgets/` only — both reviewers LOW-flagged the `ui/` exclusion but accepted it as defensible default with config escape), `embedded_in_crates` as explicit source of truth, all build-half escape hatches, the `on_failure="fail"` semantics (loud-doc strategy retained — both reviewers want machine signal augmentation, not removal). Per HIGH-2 consensus, ADD a unique exit code 3 = "deploy succeeded but new revision broken and live" (distinct from 1 = test-failed verdict and 2 = infra-error) plus GitHub Actions `::error::` annotation when running in CI (auto-detected via `CI=true` env var). This is an addition, not a replacement of the loud-banner UX.

---


<domain>
## Phase Boundary

`cargo pmcp deploy` orchestrates: `cargo build --release --arm64` → package → upload → Lambda hot-swap → "deployed successfully". For MCP App servers (Rust crates that `include_str!` widget HTML built by an external JS toolchain) this orchestration has two silent-failure gaps proven in production by Cost Coach:

- **Failure Mode A (build half, pre-Rust):** developer edited `widget/cost-over-time.html`, ran `cargo pmcp deploy`, and shipped the OLD widget because nobody ran `npm run build` — `widget/dist/*.html` was stale. `include_str!` happily inlined the stale file. Deploy reported success.
- **Failure Mode B (build half, Cargo cache):** developer ran `npm run build`, verified mtime on `widget/dist/cost-over-time.html` was newer than `target/release/bootstrap`, then ran `cargo pmcp deploy`. Cargo decided the binary was up-to-date because no `.rs` changed — `include_str!` is opaque to Cargo's dep tracker without `cargo:rerun-if-changed`. OLD binary re-shipped. Unblocked with `cargo clean`. Almost shipped before catching it.
- **Failure Mode C (verify half, post-deploy):** widget bundle was correct and Lambda was live, but the JS SDK was misconfigured (missing `onteardown` etc.) — runtime broken, deploy reported "successful" anyway because deploy never probes the live endpoint. Caught by eyeballing screenshots, not by tooling.

This phase closes A + B (extend deploy to build widgets and force cache invalidation) and C (extend deploy to verify the live endpoint via existing `cargo pmcp test {check,conformance,apps}` commands). Default-on, opt-out via flags/config.

**In scope (3 ship-points):**
- v1 — convention-based widget build, package-manager runner from lockfile, escape hatches.
- v2 — generated `build.rs` template + `cargo pmcp doctor` check + workspace `include_str!` invalidation. Solves Failure Mode B.
- v3 — post-deploy verification orchestrator (warmup → check → conformance → apps), `[post_deploy_tests]` config, `on_failure="fail"` default. Closes Failure Mode C.

**Out of scope (deferred / separate work):**
- `on_failure="rollback"` auto-rollback flow. The `DeployTarget::rollback` trait method exists at `cargo-pmcp/src/deployment/trait.rs:230` with stub-or-untested implementations across all four targets. Operator wants the existing rollback functionality verified (and likely promoted from "coming soon!" stubs in `cloudflare/mod.rs:391`, `aws_lambda/mod.rs:214`) before this phase wires `on_failure="rollback"`. Schema field MAY be reserved (parsed-but-rejected) so v3 doesn't lock out a future addition; do not implement the behavior.
- Multi-target deploys (`--target prod,staging` in one invocation). Single-target only.
- `--shared-sdk` widget bundling (cost-coach observation: 8 widgets each inline ~50KB-gzip of duplicated SDK). Out of scope.
- `widget/dist/` gitignore scaffolding fix in `cargo pmcp app new`. Real but separate cleanup.
- Widget hot-reload during `cargo pmcp dev`. JS toolchain's job.

</domain>

<decisions>
## Implementation Decisions

### Convention search (build half)
- **LOCKED:** Auto-detect widget directories ONLY at `widget/` and `widgets/` (workspace root). DROP `ui/` and `app/` from the proposal's search list. **Why:** `app/` collides hard with the standard Rust convention for binary-crate directories — would false-positive on any workspace member named `app/`. `ui/` is borderline-collision (some projects use it for non-widget UI code). Anything else needs explicit `[[widgets]]` config.
- **LOCKED:** Detect package manager from lockfile, in this order: `bun.lockb` → `bun run build`, `pnpm-lock.yaml` → `pnpm run build`, `yarn.lock` → `yarn build`, `package-lock.json` → `npm run build`. No lockfile → fall back to `npm run build`; if `npm` is not on `PATH`, fail loud with actionable error.
- **LOCKED:** If `package.json` has no `build` script, error with: `"package.json at <path> has no 'build' script — add one or configure widgets in .pmcp/deploy.toml"`. Do NOT silently skip.
- **LOCKED:** If `node_modules/` is missing, run the auto-detected install command (`bun install` / `pnpm install` / `yarn install` / `npm install`) before `build`.
- **LOCKED:** Widget build failure stops the whole deploy. Surface stdout/stderr from the build. Do NOT proceed to `cargo build`.
- **LOCKED:** Widget build succeeds but emits zero output files → WARN, do not fail (likely misconfigured but may be intentional during scaffolding).

### Cargo cache invalidation (build half — second-order bug)
- **LOCKED:** Generated `build.rs` template (Option 1 from proposal) is the primary mechanism. `cargo pmcp app new` writes a `build.rs` that emits `cargo:rerun-if-changed` against the widget output dir.
- **LOCKED:** `build.rs` template MUST resolve the widget directory via env var (`PMCP_WIDGET_DIR`, set by `cargo pmcp deploy` before invoking `cargo build`), NOT via fragile `../../widget/dist` relative-path traversal that silently breaks if the crate is moved within the workspace. Fallback: if `PMCP_WIDGET_DIR` is unset (e.g., user runs `cargo build` directly), emit a no-op `cargo:rerun-if-changed` against `Cargo.toml` and rely on user's discipline. Document this gotcha in the generated `build.rs` comment header.
- **LOCKED:** `cargo pmcp doctor` gains a check: when any workspace crate calls `include_str!` against a widget path AND has no `build.rs` with `cargo:rerun-if-changed` for that path, emit a WARN with the fix command. Pattern follows existing checks at `cargo-pmcp/src/commands/doctor.rs:52-143`.
- **LOCKED:** Option 2 (`cargo clean -p <crate>`) and Option 3 (mtime-touch sentinel) are NOT implemented. The operator explicitly recommended Option 1 as primary. If a project lacks the build.rs scaffold, doctor surfaces it; deploy does NOT silently `cargo clean` as a fallback.

### `[[widgets]]` config (`.pmcp/deploy.toml`)
- **LOCKED:** New optional section in `.pmcp/deploy.toml` (existing file used by all 4 deployment targets — see `cargo-pmcp/src/deployment/targets/{aws_lambda,cloudflare,google_cloud_run,pmcp_run}/mod.rs`). Schema:
  ```toml
  [[widgets]]
  path = "widget"                              # workspace-root-relative
  build = "npm run build"                       # optional, auto-detected from lockfile
  install = "npm install"                       # optional, auto-detected from lockfile
  output_dir = "dist"                            # optional, default "dist"
  embedded_in_crates = ["cost-coach-lambda"]    # REQUIRED when present, see below
  ```
- **LOCKED:** `embedded_in_crates` is the **explicit source of truth** for which crates need cache invalidation when this widget rebuilds. Auto-detection via `grep include_str!` is BRITTLE — `concat!`, macros, computed paths, and relative-path templating defeat it — so it is NOT used as a deploy-time silent assumption. Demote auto-detection to a `cargo pmcp doctor` HINT only ("crate X appears to include widget Y but is not listed in `embedded_in_crates` for that widget"). Deploy reads only the explicit field.
- **LOCKED:** When NO `[[widgets]]` block exists AND auto-detection finds `widget/` or `widgets/`, deploy synthesizes an in-memory `[[widgets]]` from the convention. `embedded_in_crates` defaults to ALL workspace bin crates (safe over-invalidation; doctor hints user to write the explicit config).
- **LOCKED:** Multiple `[[widgets]]` blocks supported. Stop on first failure, do not run subsequent widgets, do not deploy.

### Escape hatches (build half)
- **LOCKED:** `cargo pmcp deploy --no-widget-build` — skip widget build entirely (CI pipeline pre-built widgets in a separate step).
- **LOCKED:** `cargo pmcp deploy --widgets-only` — build widgets, skip `cargo build`/upload/hot-swap (dev loop, fast iteration).
- **DEFERRED to discretion:** Whether `cargo pmcp build` (does it exist?) gets the same widget-build hook. Planner: check `cargo-pmcp/src/commands/` for an existing `build.rs` command and either wire it or note absence in the plan.

### Post-deploy verification (verify half)
- **LOCKED:** After Lambda hot-swap completes, before reporting deploy success:
  1. **Warmup grace.** Wait `warmup_grace_ms` (default 2000ms; configurable). Why: Lambda alias swaps are atomic but ALB/API Gateway connection pooling can serve old version on in-flight pooled connections briefly.
  2. **`cargo pmcp test check`.** Quick connectivity probe (~1s, retries on connection refused up to `timeout_seconds`). Reuses existing logic at `cargo-pmcp/src/commands/test/check.rs`.
  3. **`cargo pmcp test conformance`.** MCP protocol conformance (~5–10s). Reuses `cargo-pmcp/src/commands/test/conformance.rs`.
  4. **`cargo pmcp test apps --mode claude-desktop`.** Run if `[[widgets]]` is configured OR widgets were detected via convention. Reuses `cargo-pmcp/src/commands/test/apps.rs` + `crates/mcp-tester/src/app_validator.rs:401` (`AppValidationMode::ClaudeDesktop`). **Depends on Phase 78** to promote that variant from placeholder to a real strict mode.
- **LOCKED:** `[post_deploy_tests]` config block in `.pmcp/deploy.toml`:
  ```toml
  [post_deploy_tests]
  enabled = true                                  # default
  checks = ["connectivity", "conformance", "apps"]  # default
  apps_mode = "claude-desktop"                     # default; "chatgpt" or "standard" also valid
  on_failure = "fail"                              # "fail" | "warn" — "rollback" hard-rejected at config validation
  timeout_seconds = 60
  warmup_grace_ms = 2000
  ```
- **LOCKED (SUPERSEDED 2026-05-03 by 79-REVIEWS.md HIGH-G2):** `on_failure = "rollback"` is **hard-rejected at config-validation time** with an actionable error: `"on_failure='rollback' is not yet implemented in this version of cargo-pmcp. Change to 'fail' (default) or 'warn'. Auto-rollback support will land in a future phase that verifies the existing DeployTarget::rollback() trait implementations."` Deploy refuses to start until the user changes the value. **Rationale (Gemini review):** the previously-planned reserve-but-warn-then-fallback-to-fail behavior was a UX trap — operators who explicitly configure "rollback" assume rollback happened and ignore the broken-but-live state. Hard-reject removes the trap. **Cost:** loses forward-compat reservation — when rollback support ships, that future phase must (a) add the variant to the schema enum and (b) ship a migration note for users. Operator accepted this trade in the 2026-05-03 review-decision call. *Earlier "LOCKED: parse-but-warn-and-fallback" decision is replaced by this entry.*
- **LOCKED:** `on_failure = "fail"` semantics: CLI exits non-zero; **the new (broken) Lambda revision STAYS LIVE.** This MUST be documented in screaming-loud language in BOTH the rustdoc on the config field AND the help text for `--on-test-failure`. CI/CD pipelines that interpret non-zero exit as "auto-rollback me" will misread this and serve traffic from a known-broken revision. The example failure-output box from the proposal MUST be reproduced verbatim in `--help` and rustdoc — it pre-prints the rollback command for the operator.
- **LOCKED:** Distinguish "test FAILED" (verdict on the new code, exit non-zero) from "test command itself errored" (network/auth/timeout — infrastructure flake). The latter is a deploy error with a distinct exit code or message; do not let infra flakiness verdict the new code.
- **LOCKED:** Endpoint + auth pass-through: deploy already knows the public URL and OAuth token (Phase 74 wiring). Pseudocode:
  ```rust
  let endpoint = deploy_target.public_url();
  let auth = deploy_target.auth_token();
  mcp_tester::run_conformance(endpoint, auth, ...).await?;
  mcp_tester::run_apps(endpoint, auth, AppValidationMode::ClaudeDesktop, ...).await?;
  ```
  No new auth UX. Reuse the same token the deploy already had to authenticate with.

### Escape hatches (verify half)
- **LOCKED:** `cargo pmcp deploy --no-post-deploy-test` — skip all post-deploy tests.
- **LOCKED:** `cargo pmcp deploy --post-deploy-tests=conformance,apps` — explicit subset.
- **LOCKED (SUPERSEDED 2026-05-03):** `cargo pmcp deploy --on-test-failure=warn|fail` — override config. `--on-test-failure=rollback` is **hard-rejected at clap parse time** with the same error message as the config-validation reject (no silent fallback). Mirrors the config-validation supersession above (HIGH-G2).
- **LOCKED:** `cargo pmcp deploy --apps-mode=standard|chatgpt|claude-desktop` — override default strict mode.

### Help text + docs
- **LOCKED:** `cargo pmcp deploy --help` MUST mention widgets: `"Builds widgets (auto-detected from widget/ or widgets/) before compiling and deploying the Rust binary. Verifies the deployed endpoint via cargo pmcp test {check,conformance,apps} before reporting success."`
- **LOCKED:** Generated `build.rs` template ships with a comment header explaining the `PMCP_WIDGET_DIR` env-var contract and the `cargo build` direct-invocation gotcha.

### Claude's Discretion
- Internal module layout for the widget-build orchestrator (e.g., new module `cargo-pmcp/src/deployment/widgets/` vs. inline in `commands/deploy/mod.rs`). Planner picks based on existing patterns.
- Internal module layout for the post-deploy verifier (e.g., `cargo-pmcp/src/deployment/post_deploy_tests/`).
- Wave structure (4-wave is typical for this codebase; planner decides based on dependency graph).
- Whether to reuse `cargo-pmcp/src/commands/test/{check,conformance,apps}.rs` `execute()` functions directly via Rust call OR shell out to the CLI subcommand. Direct Rust call preferred (no fork+auth roundtrip), but if the public API isn't reusable as a library, document the gap and either refactor or shell out.
- Default value tuning for `warmup_grace_ms` and per-test `timeout_seconds` (proposal suggests 2000 / 60 — planner can adjust based on observed Lambda cold-start latencies in cost-coach).
- Test-side coverage strategy: how many unit tests for path resolution + lockfile detection + config parsing, integration tests for the orchestration end-to-end, and what to mock vs. exercise via a real cargo-pmcp test fixture.
- Whether to land v1+v2 as one phase (build half) and v3 as a separate later phase, OR ship all three in this phase. **LOCKED to all-three-in-this-phase.** Phase 78 (`AppValidationMode::ClaudeDesktop` real strict mode) shipped 2026-05-02 (commit `ba694e43`, verified at `crates/mcp-tester/src/app_validator.rs:401-421` with per-signal Failed-row emission and 4 unit tests). v3 is not blocked. (Earlier draft of this doc said Phase 78 was unplanned — corrected per 79-RESEARCH.md finding #1.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Source of truth for the proposal
- `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-deploy-widget-build.md` — the originating PRD. ALL behavior in this phase derives from it as modified by the locked decisions above.

### cargo-pmcp deploy entry points
- `cargo-pmcp/src/commands/deploy/mod.rs` (line 402 `pub fn execute`, line 407 `async fn execute_async`) — the orchestrator that this phase extends. Widget pre-build hook goes BEFORE the existing `cargo build --release --arm64` call. Post-deploy verifier goes AFTER the existing Lambda hot-swap reports completion, BEFORE returning success.
- `cargo-pmcp/src/commands/deploy/init.rs` (76KB — template generation per Phase 76) — the file `cargo pmcp app new` uses to scaffold new servers. The generated `build.rs` template lives here.
- `cargo-pmcp/src/commands/deploy/deploy.rs` (5.4KB) — secondary deploy plumbing.

### Deployment target trait + implementations
- `cargo-pmcp/src/deployment/trait.rs:230` — `async fn rollback(&self, config: &DeployConfig, version: Option<&str>) -> Result<()>` — exists across all targets but operator confirmed UNTESTED. Phase 79 does NOT call this; reserves `on_failure="rollback"` config field for a future phase that verifies + wires rollback.
- `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` — primary cost-coach target. `[iam]` config support already wired here per Phase 76; `[[widgets]]` support follows the same pattern.
- `cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs` + `mod.rs` — second-priority target.
- `cargo-pmcp/src/deployment/targets/{cloudflare,google_cloud_run}/mod.rs` — also receive widget pre-build hook (no extra runtime cost when no widget dir present); rollback stubs at `cloudflare/mod.rs:390` ("coming soon!") and `aws_lambda/mod.rs:214` ("This will rollback to version: ...") confirm the existing-but-unverified state.
- `cargo-pmcp/src/deployment/builder.rs:595` — already documents `include_str!` resolution order for Rust servers; widget pre-build inherits from this convention.

### Test commands to reuse for verification
- `cargo-pmcp/src/commands/test/mod.rs` — top-level test command dispatcher.
- `cargo-pmcp/src/commands/test/check.rs` — connectivity probe.
- `cargo-pmcp/src/commands/test/conformance.rs` — protocol conformance.
- `cargo-pmcp/src/commands/test/apps.rs` (24KB) — apps validation; this is the entry point that delegates to `mcp-tester`'s `AppValidator`.
- `crates/mcp-tester/src/app_validator.rs:401` — `AppValidationMode` enum with `ClaudeDesktop` variant. **Phase 78 promotes this from placeholder to real strict mode** — Phase 79 v3 depends on Phase 78 landing.

### Doctor command
- `cargo-pmcp/src/commands/doctor.rs:15` — `pub fn execute(url: Option<&str>, global_flags: &GlobalFlags)`. Existing checks at lines 52 (`check_cargo_toml`), 80 (`check_rust_toolchain`), 102 (`check_rustfmt`), 122 (`check_clippy`), 143 (`check_server_connectivity`). Phase 79 adds `check_widget_rerun_if_changed` following the same shape.

### Config schema files (`.pmcp/deploy.toml` consumers)
- `cargo-pmcp/src/deployment/config.rs` — `DeployConfig` struct. Add `widgets: Vec<WidgetConfig>` and `post_deploy_tests: Option<PostDeployTestsConfig>` fields here.
- `cargo-pmcp/src/deployment/targets/pmcp_run/mod.rs:397`, `cargo-pmcp/src/deployment/targets/aws_lambda/mod.rs:164`, `cargo-pmcp/src/deployment/targets/cloudflare/mod.rs:229` — existing `.pmcp/deploy.toml` read sites; ensure new sections deserialize cleanly across all four targets.
- `cargo-pmcp/src/landing/config.rs:322-323` — reads `.pmcp/deployment.toml` (note: `deployment.toml`, not `deploy.toml` — a separate file). Out of scope for this phase but planner should verify no schema collision.

### Project conventions
- `CLAUDE.md` (project root) — Toyota Way quality gates, `make quality-gate`, PMAT cog ≤25, ALWAYS-required testing (fuzz, property, unit, example), feature-development kata.
- `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-BRIEF.md` — Phase 76 reference for how a new `.pmcp/deploy.toml` section was added. Mirror this structure for `[[widgets]]` and `[post_deploy_tests]`.

### Roadmap context
- `.planning/ROADMAP.md` Phase 79 entry — full scope statement. Phase 78 (the dependency) is at the immediately preceding entry.
- `.planning/STATE.md` Roadmap Evolution entry for Phase 79 (2026-05-03) — captures the locked decisions in narrative form.

</canonical_refs>

<specifics>
## Specific Ideas

### Reference implementation for widget structure
- Cost Coach widget directory at `pmcp-run/built-in/sql-api/servers/open-images` (referenced from MEMORY.md "Active Work" — open-images is the reference impl for MCP Apps wiring, including widget bundle layout).
- Cost Coach actual widget dir: `/Users/guy/projects/mcp/cost-coach/widget/` (read-only reference; do not modify from this phase's work).

### Concrete failure-output spec from the proposal (lines 184-198)
The post-deploy failure output shape is locked:
```
✓ Deployed cost-coach to pmcp-run (us-west-2)
  Lambda revision: abc123 (live)
  Widget bundles: 8 embedded

Running post-deploy verification:
  ✓ Connectivity         (200ms)
  ✓ Conformance          (8/8 tests passed)
  ✗ Apps validation      (1/8 widgets failed)
     tool 'get_spend_summary': widget cost-summary.html missing onteardown handler
     reproduce: cargo pmcp test apps --url ... --mode claude-desktop --tool get_spend_summary

⚠ The deployed version IS LIVE and contains issues.
  To roll back: cargo pmcp deploy rollback --target prod
```
Note the explicit `IS LIVE` warning and the pre-printed reproduction command — both are required UX, not optional polish.

### Generated build.rs template shape (locked direction)
```rust
// Auto-generated by cargo pmcp app new.
// Forces cargo to recompile when widget bundles change.
// PMCP_WIDGET_DIR is set by `cargo pmcp deploy` before invoking `cargo build`.
// If you run `cargo build` directly (bypassing `cargo pmcp deploy`), this falls
// back to a no-op — you are responsible for running `cargo clean -p <crate>` or
// the JS build manually. See `cargo pmcp doctor` for a check.
fn main() {
    if let Ok(dir) = std::env::var("PMCP_WIDGET_DIR") {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
        println!("cargo:rerun-if-changed={}", dir);
    } else {
        println!("cargo:rerun-if-changed=Cargo.toml");
    }
}
```
Planner refines exact path-walking semantics (recursive vs. one-level, handle missing dir, handle non-UTF-8 paths).

### CHANGELOG entry expectation
- `cargo-pmcp` minor bump (additive: new behavior + new flags + new config sections, all opt-out-with-default-on for CONVENTION-detected widgets, opt-in for explicit `[[widgets]]`). Coordinate with `pmcp` workspace per CLAUDE.md release rules.

</specifics>

<deferred>
## Deferred Ideas

- **`on_failure="rollback"` auto-rollback execution.** Schema field reserved but rejected at runtime in v3. Operator-deferred until existing `DeployTarget::rollback()` implementations across all 4 targets are verified end-to-end (currently stubs / "coming soon!" markers in cloudflare + aws_lambda).
- **Multi-target deploys (`--target prod,staging`).** Build widgets once, verify both? Out of scope. Single-target only.
- **`cargo pmcp app build --shared-sdk`.** Pre-bundle the JS SDK once, embed as separate `include_str!`, have widgets reference an injected `<script>` tag. Cost-coach observation: 8 widgets × ~50KB-gzip duplicated SDK = real bundle bloat. Worth filing as a follow-on phase.
- **`widget/dist/` gitignore scaffolding fix.** `cargo pmcp app new` generated `.gitignore` should exclude `widget/dist/` (or `dist/` under each `[[widgets]].path`). Cost-coach is currently dirtied by a single tracked artifact in the dist dir. Real but separate cleanup.
- **Widget hot-reload during `cargo pmcp dev`.** JS toolchain (Vite etc.) handles this; not deploy's concern.
- **Engines-version mismatch detection.** `package.json` `engines.node` vs. installed Node version — cheap doctor check, but out of this phase's scope.
- **`PreviewMode::ClaudeDesktop` host emulator** in `mcp-preview`. Per Phase 78 deferred items, operator wants to think about preview UX unification before adding a third mode.

</deferred>

---

*Phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification*
*Context gathered: 2026-05-03 via PRD Express Path (cost-coach proposal + locked scope decisions)*
