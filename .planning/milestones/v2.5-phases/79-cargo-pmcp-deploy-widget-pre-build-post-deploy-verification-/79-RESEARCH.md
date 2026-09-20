# Phase 79: cargo pmcp deploy — widget pre-build + post-deploy verification — Research

**Researched:** 2026-05-03
**Domain:** Rust CLI tooling (cargo-pmcp) — deploy orchestration, JS toolchain shelling, Cargo build-script wiring, post-deploy MCP verification.
**Confidence:** HIGH for the codebase findings (direct file reads); MEDIUM for the Lambda warmup recommendation (AWS docs cite provisioned-concurrency, not the without-PC default); HIGH on the dependency state (Phase 78 verified shipped).

## Summary

Phase 79 wires two default-on lifecycle hooks into `cargo pmcp deploy`: a widget JS pre-build (failure mode A/B) and a post-deploy verification orchestrator (failure mode C). All four critical surfaces — `commands/test/{check,conformance,apps}.rs::execute()`, `commands/deploy/mod.rs::execute_async()`, `deployment/config.rs::DeployConfig`, `commands/deploy/init.rs::render_stack_ts` — are direct, well-paved extension points. The Phase 76 IAM precedent is a one-to-one schema-extension template (`#[serde(default, skip_serializing_if = "X::is_empty")]` with byte-identity round-trip).

The single biggest reuse-vs-fork decision (test commands as library functions) tilts firmly to **shell-out, not in-process call** — the existing `pub async fn execute(...)` signatures take CLI strings + `AuthFlags` + `GlobalFlags` and write to stdout via `colored::Colorize` + `println!`, with `anyhow::bail!` on failure. They are not designed as library entry points. A small refactor to expose `mcp_tester` library functions (which already exist as `pub use ConformanceRunner`, `pub use AppValidator`) is feasible and preferable to subprocess spawning, but doing it in this phase doubles surface area. **Recommendation: subprocess-spawn the existing `cargo pmcp test` subcommands in v3 with structured exit-code interpretation; file a follow-on phase to expose mcp-tester as a first-class library API.**

The biggest planning risks (in order): (1) Phase 78 dependency status is **stale in CONTEXT.md** — Phase 78 actually shipped (STATE.md line 5 `status: Phase 78 complete`, `AppValidationMode::ClaudeDesktop` is a real strict mode at `crates/mcp-tester/src/app_validator.rs:401-421` with per-signal failure-row emission at line 794 and 4 unit tests at lines 1476/1570/1601/1634). v3 is **NOT blocked**. (2) The CONTEXT.md `build.rs` template is missing one line (`cargo:rerun-if-env-changed=PMCP_WIDGET_DIR`) — without it, Cargo can fail to re-run the build script when the env var value flips between two deploys. (3) "Reserved-but-rejected" `on_failure="rollback"` semantics need careful schema design so that a future Phase wiring the actual rollback flow does not break the v3 schema.

**Primary recommendation:** Wave 1 = config schema + widget pre-build orchestrator + cargo-cache invalidation via generated `build.rs`; Wave 2 = post-deploy verifier (subprocess-spawn the existing `cargo pmcp test` commands); Wave 3 = doctor check + `cargo pmcp app new` template injection + tests + example. Single phase, three waves, ship-points are wave gates.

---

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Convention search (build half):**
- Auto-detect widget directories ONLY at `widget/` and `widgets/` (workspace root). DROP `ui/` and `app/`.
- Detect package manager from lockfile, in order: `bun.lockb` → `bun run build`, `pnpm-lock.yaml` → `pnpm run build`, `yarn.lock` → `yarn build`, `package-lock.json` → `npm run build`. No lockfile → fall back to `npm run build`; if `npm` not on `PATH`, fail loud.
- If `package.json` has no `build` script, error with: `"package.json at <path> has no 'build' script — add one or configure widgets in .pmcp/deploy.toml"`. Do NOT silently skip.
- If `node_modules/` is missing, run the auto-detected install command before `build`.
- Widget build failure stops the whole deploy. Surface stdout/stderr from the build. Do NOT proceed to `cargo build`.
- Widget build succeeds but emits zero output files → WARN, do not fail.

**Cargo cache invalidation:**
- Generated `build.rs` template (Option 1) is the primary mechanism.
- `build.rs` template MUST resolve the widget directory via env var (`PMCP_WIDGET_DIR`), NOT via `..` traversal. Fallback: no-op `cargo:rerun-if-changed=Cargo.toml` if unset.
- `cargo pmcp doctor` gains a check: `include_str!` against widget path AND no `build.rs` with `cargo:rerun-if-changed` for that path → WARN.
- Option 2 (`cargo clean -p`) and Option 3 (mtime-touch) are NOT implemented.

**`[[widgets]]` config:**
- New optional section in `.pmcp/deploy.toml`. Schema:
  ```toml
  [[widgets]]
  path = "widget"
  build = "npm run build"        # optional, auto-detected
  install = "npm install"        # optional, auto-detected
  output_dir = "dist"            # optional, default "dist"
  embedded_in_crates = ["cost-coach-lambda"]  # REQUIRED when present
  ```
- `embedded_in_crates` is the EXPLICIT source of truth. Auto-detection via `grep include_str!` is BRITTLE — demote to a `cargo pmcp doctor` HINT only.
- When NO `[[widgets]]` AND auto-detection finds `widget/` or `widgets/`, deploy synthesizes an in-memory `[[widgets]]` from convention; `embedded_in_crates` defaults to ALL workspace bin crates (safe over-invalidation; doctor hints user).
- Multiple `[[widgets]]` blocks supported. Stop on first failure, do not deploy.

**Escape hatches (build half):**
- `cargo pmcp deploy --no-widget-build`
- `cargo pmcp deploy --widgets-only`
- DEFERRED to Claude's discretion: same hook on `cargo pmcp build` (planner: check whether it exists).

**Post-deploy verification:**
1. Warmup grace (`warmup_grace_ms`, default 2000ms).
2. `cargo pmcp test check` — connectivity probe with retry up to `timeout_seconds`.
3. `cargo pmcp test conformance` — protocol conformance.
4. `cargo pmcp test apps --mode claude-desktop` — only if widgets present.
- `[post_deploy_tests]` config block; default `enabled = true`, `on_failure = "fail"`.
- `on_failure = "rollback"` is parsed-but-rejected at runtime with WARN: `"on_failure='rollback' is reserved for a future phase — see Phase 79 deferred items. Treating as 'fail' for this deploy."`
- `on_failure = "fail"` semantics: CLI exits non-zero; **the new (broken) Lambda revision STAYS LIVE.** Screaming-loud docs in BOTH rustdoc AND `--help`. Reproduce the example failure-output box from the proposal verbatim.
- Distinguish "test FAILED" from "test command itself errored" (network/auth/timeout — distinct exit code).
- Endpoint + auth pass-through from deploy context to test commands. Reuse the same OAuth token deploy already has.

**Escape hatches (verify half):**
- `cargo pmcp deploy --no-post-deploy-test`
- `cargo pmcp deploy --post-deploy-tests=conformance,apps`
- `cargo pmcp deploy --on-test-failure=warn|fail`
- `cargo pmcp deploy --apps-mode=standard|chatgpt|claude-desktop`

**Help text:** `cargo pmcp deploy --help` MUST mention widgets per CONTEXT.md exact wording.

### Claude's Discretion
- Internal module layout (e.g., `cargo-pmcp/src/deployment/widgets/` vs. inline in `commands/deploy/mod.rs`).
- Internal module layout for post-deploy verifier (e.g., `cargo-pmcp/src/deployment/post_deploy_tests/`).
- Wave structure (4-wave typical for this codebase).
- Reuse-vs-shell-out decision for `cargo pmcp test {check,conformance,apps}`. **(Recommendation: shell-out — see "Reuse-vs-fork decision" below.)**
- Default value tuning for `warmup_grace_ms` and `timeout_seconds`.
- Test-side coverage strategy.
- Whether to land v1+v2 as one phase and v3 as a follow-on, OR ship all three. **CONTEXT.md says "default to all-three-in-this-phase unless Phase 78 isn't executable."** Phase 78 IS shipped, so ship all three.

### Deferred Ideas (OUT OF SCOPE)
- `on_failure="rollback"` auto-rollback execution.
- Multi-target deploys (`--target prod,staging`).
- `cargo pmcp app build --shared-sdk`.
- `widget/dist/` gitignore scaffolding fix.
- Widget hot-reload during `cargo pmcp dev`.
- Engines-version mismatch detection.
- `PreviewMode::ClaudeDesktop` host emulator in `mcp-preview`.

---

## Project Constraints (from CLAUDE.md)

These directives are equal-priority to CONTEXT.md decisions. Plan must comply.

| # | Directive | How it constrains Phase 79 |
|---|-----------|---------------------------|
| C1 | Cognitive complexity ≤25 per function (PMAT gated in CI; pre-merge blocker) | Every new function in widget orchestrator + post-deploy verifier MUST land ≤25. Phase 75 history shows P1 (per-stage pipeline) and P4 (per-variant dispatch) are the standard refactor patterns; Phase 76 used same. Hard cap is cog 50 with `// Why:`-annotated `#[allow]` per Phase 75 D-03. |
| C2 | Zero clippy warnings (`make quality-gate` enforced) | `cargo clippy --features full -- -D warnings` (pedantic + nursery via Makefile) is the local gate. CI uses same. |
| C3 | Zero SATD (`TODO`, `FIXME`, `XXX` in committed code) | Use `// Why:` annotations for accepted complexity, never `// TODO:`. |
| C4 | All commits via `make quality-gate` (fmt, clippy, build, test, audit) | Plan tasks must invoke `make quality-gate` as the verification command, not bare `cargo` calls. |
| C5 | ALWAYS-required testing for new features: FUZZ + PROPERTY + UNIT + cargo run --example | Phase 79 must add: 1 fuzz target (TOML parsing of new sections), property tests (lockfile→PM detection determinism), unit tests (≥80% line coverage on new modules), 1 runnable example demonstrating the deploy flow. |
| C6 | Toyota Way: "ZERO tolerance for defects" — pre-commit hook blocks on quality failure | No --no-verify commits. Pre-commit hook output is the source of truth. |
| C7 | Contract-first development for new features (provable-contracts YAML) | Optional but recommended for new public APIs. Per Phase 76/77, this has been treated as soft-required; planner may treat as discretion. |
| C8 | PMAT runs in CI only (per Phase 75 D-07, to keep dev loop fast) | Plan tasks should NOT include `pmat` in pre-commit; only `make quality-gate`. PMAT failures surface at PR time. |

---

## Phase Requirements

This phase has no numbered REQ-IDs in REQUIREMENTS.md. Per CONTEXT.md, the scope is captured fully in the Implementation Decisions block. The planner should construct local IDs (REQ-79-01 through REQ-79-NN) at plan time so individual tasks have stable references; suggested mapping:

| Suggested ID | Source | Behavior |
|--------------|--------|----------|
| REQ-79-01 | CONTEXT.md "Convention search" | Auto-detect `widget/` or `widgets/` only; reject `ui/`/`app/`. |
| REQ-79-02 | CONTEXT.md "Convention search" | Lockfile→PM detection in priority order; npm fallback. |
| REQ-79-03 | CONTEXT.md "Convention search" | Missing `build` script → error with actionable message. |
| REQ-79-04 | CONTEXT.md "Convention search" | Missing `node_modules/` → run install first. |
| REQ-79-05 | CONTEXT.md "Convention search" | Build failure stops deploy; surface stdout/stderr. |
| REQ-79-06 | CONTEXT.md "Cargo cache invalidation" | Generated `build.rs` resolves via `PMCP_WIDGET_DIR` env var. |
| REQ-79-07 | CONTEXT.md "Cargo cache invalidation" | Doctor warns when crate uses `include_str!` against widget path without matching `build.rs`. |
| REQ-79-08 | CONTEXT.md "[[widgets]] config" | `[[widgets]]` schema parses + round-trips byte-identically when absent. |
| REQ-79-09 | CONTEXT.md "[[widgets]] config" | `embedded_in_crates` is the source of truth; convention-synthesized form invalidates ALL bin crates. |
| REQ-79-10 | CONTEXT.md "Escape hatches (build)" | `--no-widget-build`, `--widgets-only` flags. |
| REQ-79-11 | CONTEXT.md "Post-deploy verification" | Warmup → check → conformance → apps lifecycle. |
| REQ-79-12 | CONTEXT.md "Post-deploy verification" | `[post_deploy_tests]` schema parses + round-trips when absent. |
| REQ-79-13 | CONTEXT.md "Post-deploy verification" | `on_failure="fail"` exits nonzero with screaming-loud rustdoc + help text. |
| REQ-79-14 | CONTEXT.md "Post-deploy verification" | `on_failure="rollback"` parsed-but-rejected with WARN. |
| REQ-79-15 | CONTEXT.md "Post-deploy verification" | Distinguish test-failed from infra-error (distinct exit code/message). |
| REQ-79-16 | CONTEXT.md "Post-deploy verification" | Endpoint + auth flow from deploy context to test commands without re-prompting. |
| REQ-79-17 | CONTEXT.md "Escape hatches (verify)" | `--no-post-deploy-test`, `--post-deploy-tests=...`, `--on-test-failure=`, `--apps-mode=` flags. |
| REQ-79-18 | CONTEXT.md "Help text" | `cargo pmcp deploy --help` mentions widgets verbatim. |

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Widget directory detection (`widget/`/`widgets/`) | Deploy orchestrator (`commands/deploy/mod.rs`) | — | Target-agnostic; runs once per deploy regardless of which target ships the binary. |
| Package-manager selection from lockfile | Widget pre-build module (Claude's discretion: `deployment/widgets/`) | — | Pure file-detection logic; isolated for unit/property testing. |
| Spawning `npm/pnpm/yarn/bun run build` | Widget pre-build module | — | Process spawning + stdout streaming. Use `tokio::process::Command` per existing pattern in `commands/test/mod.rs::execute` (already uses tokio runtime). |
| Setting `PMCP_WIDGET_DIR` for `cargo build` | Deploy orchestrator | Per-target build trampoline | Set once in env before invoking `target.build(&config)`. All four targets call `cargo build` indirectly via the CDK / wrangler pipelines; env var inherits naturally. |
| Generated `build.rs` template | `commands/deploy/init.rs` (per Phase 76 precedent) | `templates/mcp_app.rs` (extends current scaffold) | Phase 76 IAM template lives in `init.rs`; the same file is the per-target template branch site. **Open question:** is `cargo pmcp app new` (`commands/app.rs::create_app` line 87, calling `templates::mcp_app::generate`) a separate scaffold from `commands/deploy/init.rs`? **Yes** — they are different scaffolds (`new.rs` for workspace bootstrap, `app.rs` for MCP App project, `init.rs` for the deploy CDK project). The `build.rs` injection target is `templates::mcp_app::generate` (the MCP App scaffold), NOT `init.rs` (which generates the CDK stack, not the user's Rust crate). |
| `[[widgets]]` config parsing | `deployment/config.rs::DeployConfig` (extend) | New `deployment/widgets.rs` (sibling of `iam.rs`) | Mirror Phase 76's `iam: IamConfig` field with `#[serde(default, skip_serializing_if = "WidgetsConfig::is_empty")]`. Sub-types live in `deployment/widgets.rs`. |
| Post-deploy test orchestrator | New `deployment/post_deploy_tests.rs` (sibling of `iam.rs`) | Deploy orchestrator hook site | Pure Rust orchestration over subprocess spawns; isolated for unit testing. |
| Subprocess spawn of `cargo pmcp test {check,conformance,apps}` | Post-deploy test orchestrator | — | Use the same binary (`std::env::current_exe()` or `cargo run --bin cargo-pmcp`) to avoid PATH lookup + ensure version match. |
| Auth + endpoint pass-through | Deploy orchestrator → orchestrator-built CLI args | — | Already has URL via `outputs.url`; auth flows through `--auth-token` flag (no env-var leakage). |
| Doctor check (`include_str!` ↔ `build.rs` correlation) | `commands/doctor.rs` (extend) | — | Direct extension of existing `check_*` pattern at lines 52, 80, 102, 122, 143. |

---

## Standard Stack

### Core (already in cargo-pmcp dependencies — verify versions)

| Library | Version (locked in Cargo.toml) | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` + `serde_derive` | `1` (workspace) | Config schema serde | `[VERIFIED: cargo-pmcp/Cargo.toml line 25]` Already used by `IamConfig` and all of `DeployConfig`. |
| `toml` | (transitive via existing crates) | TOML parse/emit | `[VERIFIED: cargo-pmcp/src/deployment/config.rs:461 toml::from_str]` Used everywhere for `.pmcp/deploy.toml`. |
| `anyhow` | (workspace) | Error propagation | `[VERIFIED: ubiquitous in cargo-pmcp/]` |
| `tokio` | `1` with `["full", "rt-multi-thread"]` | Async runtime | `[VERIFIED: cargo-pmcp/Cargo.toml line 36]` Existing test commands wrap `tokio::runtime::Runtime::new()?.block_on(...)`. |
| `tokio::process::Command` | (part of tokio "process" feature) | Async subprocess spawn for npm/pnpm/yarn/bun + cargo pmcp test re-spawn | `[CITED: tokio docs]` Standard pattern for capturing stdout/stderr while letting the parent stream output. |
| `colored` | (workspace) | Output formatting (`✓`, `✗`, color codes) | `[VERIFIED: cargo-pmcp/src/commands/test/check.rs:11 use colored::Colorize]` Existing pattern. |
| `clap` | `4` with `["derive", "env"]` | CLI flag parsing | `[VERIFIED: cargo-pmcp/Cargo.toml line 22]` `#[arg(long)]` already used everywhere. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `which` (NEW dep candidate) | latest 6.x | `command -v` check for npm/pnpm/yarn/bun + actionable "not on PATH" error | When npm fallback path triggers AND `npm` binary missing. **Alternative:** use `tokio::process::Command::new("npm").arg("--version").spawn()` and treat ENOENT as "not on PATH" — no new dep. **Recommendation: latter** (zero-dep cost, error message can still be actionable). |
| `regex` | (already a transitive dep via `cargo-pmcp/src/deployment/iam.rs:40`) | `include_str!` grep in doctor check | `[VERIFIED: cargo-pmcp/src/deployment/iam.rs:40 use regex::Regex]` Doctor check needs `include_str!\("[^"]*widget(s)?/[^"]*"\)` matching. |
| `walkdir` (NEW dep candidate) | latest | Recursive workspace scan for `include_str!` references in doctor check | Single-use; alternative is `std::fs::read_dir` recursion (handful of lines). **Recommendation: stdlib** unless workspace already has `walkdir`. |

**Verification of `walkdir` already present:**

```bash
grep -rn "walkdir" /Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/Cargo.toml /Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml
```
Result: not present in either. Use stdlib.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Subprocess-spawn `cargo pmcp test ...` | Direct in-process call to `mcp_tester::ConformanceRunner` + `mcp_tester::AppValidator` | In-process is faster (~50ms saved per test) and avoids exit-code interpretation, but **requires refactoring the existing `commands/test/{check,conformance,apps}.rs::execute()` functions to be reusable as library callables** — they currently take `(url: String, ..., AuthFlags, GlobalFlags)` and write directly to stdout. The lib-callable refactor is a separate phase's worth of work. **Recommendation: subprocess for v3; file follow-on phase to expose mcp-tester library API.** Direct invocation can use `mcp_tester::ConformanceRunner::new(strict, parsed_domains).run(&mut tester).await` (already a public API per `crates/mcp-tester/src/lib.rs:61 pub use ConformanceRunner`) — but this drops the auto-formatted output. Hybrid: in-process for connectivity (cheapest), subprocess for conformance + apps (gets the rich CLI output for free). Decision: planner may chose hybrid; pure-subprocess is simpler. |
| Generated `build.rs` per CONTEXT.md template | Mtime-touch sentinel `.rs` (Option 3 from proposal) | Rejected by CONTEXT.md — locked decision is `build.rs`. |
| `tokio::process::Command` for npm | `std::process::Command` (sync) | Sync would block the async deploy flow for long widget builds; tokio's variant streams stdout while the parent prints via `colored` exactly like `commands/test/check.rs::execute`. |
| Spawn the `cargo-pmcp` binary by name | Use `std::env::current_exe()` | `current_exe()` guarantees the SAME binary version runs the post-deploy tests as ran the deploy itself — eliminates a class of "I deployed with cargo-pmcp 0.12 but my PATH had 0.10's test command" subtle bugs. **Recommendation: use `current_exe()`.** |

**Installation:** No new external crate dependencies required. (See `which` discussion above — stdlib suffices.)

**Version verification:**
- `cargo-pmcp 0.11.0` (root, this phase bumps to `0.12.0` minor — additive)
- `mcp-tester 0.5.3` (no public-API change required; subprocess decision avoids it)
- `mcp-preview 0.3.0` (unchanged)
- `pmcp 2.6.0` (unchanged — this phase is cargo-pmcp-only)

`[VERIFIED: cargo-pmcp/Cargo.toml line 3 + crates/mcp-tester/Cargo.toml line 3]`

---

## Architecture Patterns

### System Architecture Diagram

```
USER: cargo pmcp deploy [--no-widget-build] [--no-post-deploy-test] [--on-test-failure=...]
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  commands/deploy/mod.rs::execute_async()                                    │
│   Step 1: project_root + DeployConfig::load() + secret resolution           │
│   Step 2: emit_target_banner_if_resolved() (Phase 77)                       │
│                                                                             │
│   *** NEW Step 2.5: WIDGET PRE-BUILD HOOK ***                               │
│   if !no_widget_build {                                                     │
│     widgets = detect_widgets(&config, &project_root)?;  // synth or explicit│
│     for widget in widgets {                                                 │
│       run_widget_build(widget)?;  // bun/pnpm/yarn/npm; install first if NM │
│     }                                                                       │
│     std::env::set_var("PMCP_WIDGET_DIR", widget.absolute_output_dir());     │
│     // Optionally: cargo clean -p crate for crates in embedded_in_crates    │
│     // (NOT done — build.rs is the mechanism, per CONTEXT.md D-12)          │
│   }                                                                         │
│   if widgets_only { return Ok(()); }   // --widgets-only escape             │
│                                                                             │
│   Step 3 (existing): target.build(&config) — invokes `cargo build`          │
│                                                                             │
│   Step 4 (existing): target.deploy(&config, artifact) — Lambda hot-swap     │
│                                                                             │
│   *** NEW Step 4.5: POST-DEPLOY VERIFY HOOK ***                             │
│   if !no_post_deploy_test && config.post_deploy_tests.enabled {             │
│     sleep(warmup_grace_ms);                                                 │
│     // Use std::env::current_exe() to ensure version match                  │
│     run_check(outputs.url, auth_token, retries)?;     // until live         │
│     run_conformance(outputs.url, auth_token)?;                              │
│     if widgets.is_some() {                                                  │
│       run_apps(outputs.url, auth_token, apps_mode)?;                        │
│     }                                                                       │
│     // on_failure="fail" → bail with rollback-command-pre-printed banner    │
│     // on_failure="warn" → continue, print warning                          │
│     // on_failure="rollback" → WARN-and-treat-as-fail (per CONTEXT.md)      │
│   }                                                                         │
│                                                                             │
│   Step 5 (existing): outputs.display() + save_deployment_info()             │
└─────────────────────────────────────────────────────────────────────────────┘
                            │
                            ▼
                    USER: deploy success/fail with verified-or-flagged endpoint

PARALLEL TRACK — `cargo pmcp app new <name>`:
  commands/app.rs::create_app()
    → templates::mcp_app::generate(project_dir, name)
    → ALSO writes build.rs    ← *** NEW SCAFFOLD ARTIFACT ***
       (template per CONTEXT.md "Specific Ideas")

PARALLEL TRACK — `cargo pmcp doctor`:
  commands/doctor.rs::execute()
    → existing 5 check_* fns (lines 52, 80, 102, 122, 143)
    → check_widget_rerun_if_changed()  ← *** NEW CHECK FUNCTION ***
       walks workspace, greps for include_str!\("[^"]*widget(s)?/[^"]*"\)
       confirms each matched crate has build.rs with cargo:rerun-if-changed
```

### Recommended Project Structure (additions)

```
cargo-pmcp/
├── src/
│   ├── deployment/
│   │   ├── widgets.rs              # NEW — WidgetsConfig, WidgetConfig, detect_widgets, build_widgets
│   │   ├── post_deploy_tests.rs    # NEW — PostDeployTestsConfig, run_post_deploy_tests, OnFailure enum
│   │   ├── config.rs               # EXTEND — add `widgets: WidgetsConfig`, `post_deploy_tests: Option<PostDeployTestsConfig>`
│   │   ├── iam.rs                  # REFERENCE — Phase 76 pattern to mirror
│   │   └── ...
│   ├── commands/
│   │   ├── deploy/
│   │   │   ├── mod.rs              # EXTEND — wire widget pre-build + post-deploy verify hooks; add 5 new flags
│   │   │   └── init.rs             # NO CHANGE for build.rs (it scaffolds the CDK project, not the user crate)
│   │   ├── app.rs                  # EXTEND — make create_app() also write build.rs
│   │   └── doctor.rs               # EXTEND — add check_widget_rerun_if_changed
│   └── templates/
│       └── mcp_app.rs              # EXTEND — emit build.rs as part of the scaffold
├── tests/
│   ├── widgets_config.rs           # NEW — schema parsing/round-trip + lockfile→PM property tests
│   ├── post_deploy_tests_config.rs # NEW — schema parsing/round-trip + on_failure semantics
│   ├── widgets_orchestrator.rs     # NEW — integration: synthesize widgets + run build + verify exit codes
│   └── post_deploy_orchestrator.rs # NEW — integration: mock cargo-pmcp test subprocess + verify orchestration
├── examples/
│   └── deploy_with_widgets.rs      # NEW — end-to-end demo: scaffold → widget build → deploy (against mock target)
└── fuzz/
    └── fuzz_targets/
        └── fuzz_widgets_config.rs  # NEW — toml::from_str panic-free for adversarial [[widgets]] / [post_deploy_tests]
```

### Pattern 1: Schema extension via `#[serde(default, skip_serializing_if = "X::is_empty")]`

Phase 76 IAM precedent — copy this verbatim shape.

```rust
// Source: cargo-pmcp/src/deployment/config.rs:30-31, 736-763
//   /// IAM declarations for the Lambda execution role (tables, buckets, raw statements).
//   ///
//   /// The `skip_serializing_if` guard preserves byte-identity on the no-`[iam]`
//   /// path so pre-existing `.pmcp/deploy.toml` files round-trip unchanged.
//   #[serde(default, skip_serializing_if = "IamConfig::is_empty")]
//   pub iam: IamConfig,

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WidgetsConfig {
    #[serde(default, rename = "widgets")]
    pub widgets: Vec<WidgetConfig>,
}

impl WidgetsConfig {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}

// In DeployConfig:
//   #[serde(default, skip_serializing_if = "WidgetsConfig::is_empty")]
//   pub widgets: WidgetsConfig,
```

**When to use:** ALL new `.pmcp/deploy.toml` schema additions. Preserves byte-identity round-trip for projects without the new section.

### Pattern 2: Per-stage pipeline decomposition (cog ≤25)

Phase 75 P1 pattern — used heavily across cargo-pmcp.

```rust
// Source: cargo-pmcp/src/commands/test/check.rs::execute (lines 20-60) — already follows this shape.
// Top-level orchestrator stays under cog 25 by delegating to small helpers.

pub async fn run_widget_build(widget: &WidgetConfig, root: &Path, quiet: bool) -> Result<()> {
    let resolved = resolve_widget_paths(widget, root)?;          // helper
    let pm = detect_package_manager(&resolved.path)?;             // helper
    ensure_node_modules(&pm, &resolved, quiet).await?;           // helper
    invoke_build_script(&pm, &resolved, quiet).await?;            // helper
    verify_outputs_exist(&resolved, quiet)?;                      // helper
    Ok(())
}
```

**When to use:** Every new orchestrator function in widget pre-build and post-deploy verify modules.

### Pattern 3: Subprocess spawn with stdout streaming

```rust
// Source pattern: synthesized from `cargo-pmcp/src/commands/test/check.rs` async pattern + tokio docs.
// Key: stream stdout/stderr in real-time so user sees `npm install` progress, don't buffer-then-dump.

async fn invoke_build_script(pm: &PackageManager, paths: &ResolvedPaths, quiet: bool) -> Result<()> {
    use tokio::process::Command;
    let (cmd, args) = pm.build_command();  // ("npm", &["run", "build"]) etc.
    let mut child = Command::new(cmd)
        .args(args)
        .current_dir(&paths.path)
        .env("PMCP_WIDGET_DIR", &paths.absolute_output_dir)  // forwarded for downstream cargo
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to spawn `{cmd}`. Is it on PATH?"))?;
    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("Widget build (`{cmd}`) failed — see output above");
    }
    Ok(())
}
```

**When to use:** All process spawns (npm/pnpm/yarn/bun, cargo-pmcp test re-spawn). NOTE: when re-spawning `cargo-pmcp` itself for post-deploy tests, use `std::env::current_exe()?` for the command path.

### Pattern 4: Generated `build.rs` template (CONTEXT.md template + missing line)

```rust
// Auto-generated by `cargo pmcp app new`.
// Forces cargo to recompile when widget bundles change.
//
// PMCP_WIDGET_DIR is set by `cargo pmcp deploy` before invoking `cargo build`.
// If you run `cargo build` directly (bypassing `cargo pmcp deploy`), this falls
// back to a no-op — you are responsible for running `cargo clean -p <crate>` or
// the JS build manually. See `cargo pmcp doctor` for a check.
fn main() {
    // CRITICAL: this line ensures Cargo re-runs build.rs whenever PMCP_WIDGET_DIR
    // *changes value* between cargo invocations. Without it, switching widget
    // dirs (e.g. multi-widget projects with per-widget output dirs) would not
    // trigger re-evaluation. [CITED: doc.rust-lang.org/cargo/reference/build-scripts.html
    // — "rerun-if-env-changed"]
    println!("cargo:rerun-if-env-changed=PMCP_WIDGET_DIR");

    if let Ok(dir) = std::env::var("PMCP_WIDGET_DIR") {
        // Walk one level (widget bundles are flat under output_dir per cost-coach reference).
        // Recursion is intentionally NOT done — Vite/Rollup output is flat HTML files.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            }
        }
        // Also re-run if the directory itself appears/disappears.
        println!("cargo:rerun-if-changed={}", dir);
    } else {
        // Fallback: no PMCP_WIDGET_DIR — direct `cargo build` invocation.
        // Tie rerun to Cargo.toml so at least dep changes trigger rebuild.
        println!("cargo:rerun-if-changed=Cargo.toml");
    }
}
```

**Difference from CONTEXT.md template:** added `cargo:rerun-if-env-changed=PMCP_WIDGET_DIR` line (per [Cargo Book — Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-env-changed): "tells Cargo to re-run the build script if the value of an environment variable of the given name has changed"). Without this, the multi-widget case where two consecutive deploys swap PMCP_WIDGET_DIR can hit a stale build script result.

**`[VERIFIED: github.com/rust-lang/cargo PR#14756 (2026-01-15)]` `[CITED: doc.rust-lang.org/cargo/reference/build-scripts.html]`**

### Anti-Patterns to Avoid

- **Auto-detecting `embedded_in_crates` via `grep include_str!` at deploy time:** brittle (`concat!`, macros, computed paths defeat it). CONTEXT.md locked this as doctor-hint-only. Hard rule: deploy reads only the explicit field; convention-synthesized form invalidates ALL workspace bin crates (safe over-invalidation).
- **Calling `cargo clean -p <crate>` as a fallback when `build.rs` is missing:** also explicitly rejected by CONTEXT.md. Doctor surfaces the gap; deploy does not silently nuke caches.
- **In-process `pub async fn execute(...)` calls into `commands/test/{check,conformance,apps}.rs`:** these write directly to stdout via `colored::Colorize` and `bail!` on failure. Treating them as library entry points means inheriting their CLI-output semantics. Subprocess-spawn instead.
- **Setting `PMCP_WIDGET_DIR` per-target in each of the 4 `targets/*/deploy.rs`:** the env var must be set ONCE in the orchestrator, before any `target.build()` call. Per-target setting fragments the contract and risks one target forgetting it.
- **Synchronously blocking on `npm install` with stdout buffered:** for cost-coach's 8 widgets × ~5MB-each install footprint, install can be 30-60s. Stream stdout/stderr live so the user sees progress.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML parsing for `[[widgets]]` and `[post_deploy_tests]` | Custom parser | `serde` + `toml` crate already in workspace | `[VERIFIED: cargo-pmcp/src/deployment/config.rs:461]` Existing pattern. Schema additions are pure derive. |
| Lockfile→package-manager dispatch table | A registry/trait abstraction | A simple enum + match | 4 lockfiles, no extension expected. Over-abstracting violates KISS. |
| `cargo:rerun-if-changed` directive emission | Run-time `build.rs` generation via templating engine | Static template string with one substitution point (`PMCP_WIDGET_DIR`) | Zero runtime decisions; compile-time constant. Same as Phase 76's `render_iam_block` for static parts. |
| Subprocess spawning + stream capture | `std::process` with manual thread-per-pipe | `tokio::process::Command` | tokio's variant integrates with the existing async runtime; one less std-thread per spawn. |
| Test orchestration retry logic for "Lambda not yet live" | Custom backoff implementation | `tokio::time::sleep` + bounded loop | `cargo pmcp test check` already handles connection-refused; the orchestrator loops `check` itself with `tokio::time::sleep(timeout / max_attempts)` between attempts. Simple. |
| Auth token pass-through | Re-running OAuth PKCE flow | Pass via `--auth-token` flag (already supported by `AuthFlags`) | `[VERIFIED: cargo-pmcp/src/commands/flags.rs AuthFlags]` Phase 74 wired this. |

**Key insight:** Phase 79 is glue, not new mechanisms. Every primitive it needs (TOML schema, subprocess spawn, retry, auth token, output formatting) already exists in cargo-pmcp.

---

## Runtime State Inventory

This is **NOT** a rename/refactor/migration phase — it adds new behavior. **Skipped per agent spec:** "Include this section for rename/refactor/migration phases only."

For completeness, no runtime state needs migration:
- New TOML sections are additive; absent sections round-trip byte-identically (Phase 76 pattern).
- No DB/datastore state.
- No OS-registered tasks.
- No env vars renamed (`PMCP_WIDGET_DIR` is new).
- One new build artifact (generated `build.rs`); existing scaffolded projects are unchanged unless user opts in via doctor warning + manual fix.

---

## Common Pitfalls

### Pitfall 1: Cargo cache hold-on bypassing the build.rs (Failure Mode B re-edition)
**What goes wrong:** A user pulls in the new template via `cargo pmcp app new` but their existing crate has `target/` from before the build.rs was added. First deploy after the upgrade serves the stale cached binary.
**Why it happens:** `cargo:rerun-if-changed` only triggers a re-run when build.rs actually exists and is in the dep graph. Adding build.rs to a project with stale cache means the FIRST build still trusts the cache.
**How to avoid:** When `cargo pmcp doctor` detects the missing-build.rs case AND warns the user, the user is instructed to run `cargo clean -p <crate>` once, ALSO. Document this in the doctor warning text.
**Warning signs:** Deploy reports success with mtime unchanged on `target/release/bootstrap` despite `widget/dist/*.html` mtime newer.

### Pitfall 2: `npm install` runs even when developer pre-installed (CI redundancy)
**What goes wrong:** CI pipeline already runs `npm install` in a separate step; cargo pmcp deploy re-runs it; double install time.
**Why it happens:** "If `node_modules/` is missing, run install" — but in CI, node_modules might be cached from a previous step that succeeded.
**How to avoid:** Only check existence of `node_modules/` directory. If it exists, skip install (correct — CONTEXT.md decision). The `--no-widget-build` flag is the explicit escape for full CI control.
**Warning signs:** Doubled npm install runs in CI logs.

### Pitfall 3: Lambda alias-swap returns "deployed" but old version still serves connection-pooled requests
**What goes wrong:** `target.deploy()` returns success the moment Lambda's UpdateAlias call completes, but ALB / API Gateway pre-existing TCP/HTTP2 connections may continue routing to the old version for a brief window.
**Why it happens:** Lambda alias resolution happens at request-time, but connection pooling at the ALB/APIGW layer is independent. A pooled connection that established before the swap can serve a few stragglers.
**How to avoid:** TWO mitigations together:
  1. `warmup_grace_ms = 2000` floor (CONTEXT.md default).
  2. `cargo pmcp test check` retries on connection-refused AND on a "version-mismatch" signal — since Lambda alias swaps are atomic at the alias level, the actual misroute is rare in modern API Gateway (HTTP/2 multiplexing limits keep-alive impact). 2000ms is empirically sane for cost-coach class deployments.
  3. NEW: re-issue the failing test once after a 1s pause before declaring failure — distinguishes "stale pooled connection served us" from "new version is broken." The `check` command's existing connection-refused retry doesn't catch this; orchestrator-level retry-on-failure-once is needed.
**Warning signs:** First test invocation fails, retry passes — log this as a INFO (not an error) so operators learn the warmup floor is too low for their workload.

`[ASSUMED: 2000ms is enough for cost-coach-class deployments]` — operator should validate against real cost-coach cold-start logs. AWS provisioned-concurrency docs ([Provisioned Concurrency, AWS](https://docs.aws.amazon.com/lambda/latest/dg/provisioned-concurrency.html)) describe "double-digit ms" with PC, but without PC cold starts can be 200ms-2s for ARM64 Rust. `[CITED: docs.aws.amazon.com/lambda/latest/dg/provisioned-concurrency.html]`

### Pitfall 4: Distinguishing test-failed from infra-error leaks into exit codes
**What goes wrong:** A network timeout mid-conformance-test produces the same nonzero exit code as a real test failure; CI pipeline's "auto-rollback" interpretation can't tell them apart.
**Why it happens:** Default subprocess exit codes are 0/1; `cargo pmcp test conformance` itself has the same semantics.
**How to avoid:** Wrap the subprocess result in a structured `enum` BEFORE deciding the exit code:
```rust
enum TestOutcome {
    Passed,                              // exit 0
    TestFailed(String),                  // exit 1 — verdict on the new code; on_failure governs
    InfraError(InfraErrorKind, String),  // exit 2 — DON'T treat as a verdict; print "infra error" banner
}
```
The orchestrator surfaces InfraError with a distinct exit code (e.g., 2) AND a distinct banner. CI scripts can grep for the banner text (or use exit code) to decide whether to rollback.
**Warning signs:** "Auto-rollback" triggered by transient network blips during the post-deploy window.

### Pitfall 5: `on_failure="rollback"` schema field acceptance creates user expectation of the actual behavior
**What goes wrong:** User reads docs, sets `on_failure="rollback"`, expects auto-rollback, gets a WARN-and-fail. Deploys broken code thinking it'll auto-revert.
**Why it happens:** The schema field is parsed correctly; only the runtime rejects it.
**How to avoid:** TWO mitigations:
  1. The WARN message (per CONTEXT.md exact wording) is printed BEFORE any deploy work begins (not after the test failure).
  2. Rustdoc on the `OnFailure::Rollback` variant carries the same warning, so `cargo doc` is honest.
  3. Plan should add a unit test that, given `on_failure="rollback"` in TOML, asserts BOTH (a) parse succeeds, (b) when consumed by the orchestrator, the WARN is logged before deploy starts.
**Warning signs:** Operators surprised in production.

### Pitfall 6: PMCP cog ≤25 hard cap requires aggressive decomposition
**What goes wrong:** Phase 75 history shows that a "simple" widget orchestrator can balloon to cog 30+ once all the lockfile + missing-script + missing-modules + zero-output paths are wired. PMAT gates in CI; PR blocked.
**Why it happens:** Match arms + nested if-let chains compound cog cost.
**How to avoid:** Plan tasks at function granularity, with target cog ≤20 per function (5-point safety margin). Use Phase 75 P1 (per-stage pipeline) and P4 (per-variant dispatch) refactor patterns from `.planning/phases/75-fix-pmat-issues/75-RESEARCH.md`. If a function legitimately can't be decomposed (e.g., a single switch over 6 lockfile types), use `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]` per Phase 75 D-03 hard cap of cog 50.
**Warning signs:** PR's `quality-gate` job fails on `pmat analyze complexity --max-cognitive 25`.

---

## Code Examples

### Loading `[[widgets]]` from `.pmcp/deploy.toml` (mirrors Phase 76 IAM)

```rust
// Source pattern: cargo-pmcp/src/deployment/config.rs:7-36 (DeployConfig struct).
// New addition follows lines 28-31's IamConfig pattern.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    // ... existing fields ...

    /// Widget pre-build declarations (Phase 79).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the no-`[[widgets]]`
    /// path so pre-existing `.pmcp/deploy.toml` files round-trip unchanged.
    #[serde(default, skip_serializing_if = "WidgetsConfig::is_empty")]
    pub widgets: WidgetsConfig,

    /// Post-deploy verification config (Phase 79).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_deploy_tests: Option<PostDeployTestsConfig>,

    // ... existing fields ...
}
```

### Lockfile-driven package-manager detection

```rust
// cargo-pmcp/src/deployment/widgets.rs (new module).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager { Bun, Pnpm, Yarn, Npm }

impl PackageManager {
    /// Detect from lockfile presence in `dir`. Priority order is locked by CONTEXT.md.
    pub fn detect_from_dir(dir: &Path) -> Self {
        if dir.join("bun.lockb").exists() { return Self::Bun; }
        if dir.join("pnpm-lock.yaml").exists() { return Self::Pnpm; }
        if dir.join("yarn.lock").exists() { return Self::Yarn; }
        if dir.join("package-lock.json").exists() { return Self::Npm; }
        // Fallback: npm (CONTEXT.md decision)
        Self::Npm
    }

    pub fn install_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Bun => ("bun", &["install"]),
            Self::Pnpm => ("pnpm", &["install"]),
            Self::Yarn => ("yarn", &["install"]),
            Self::Npm => ("npm", &["install"]),
        }
    }

    pub fn build_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Bun => ("bun", &["run", "build"]),
            Self::Pnpm => ("pnpm", &["run", "build"]),
            Self::Yarn => ("yarn", &["build"]),
            Self::Npm => ("npm", &["run", "build"]),
        }
    }
}
```

### Subprocess spawn for `cargo pmcp test conformance`

```rust
// cargo-pmcp/src/deployment/post_deploy_tests.rs (new module).

use tokio::process::Command;

async fn run_conformance_subprocess(
    url: &str,
    auth_token: Option<&str>,
    timeout_secs: u64,
) -> Result<TestOutcome> {
    let exe = std::env::current_exe()
        .context("Failed to resolve current executable for subprocess spawn")?;

    let mut cmd = Command::new(&exe);
    cmd.arg("test").arg("conformance").arg(url)
       .arg("--timeout").arg(timeout_secs.to_string());
    if let Some(token) = auth_token {
        cmd.arg("--auth-token").arg(token);  // AuthFlags supports this
    }

    let status = cmd.status().await
        .context("Failed to spawn `cargo pmcp test conformance` subprocess")?;

    Ok(match status.code() {
        Some(0) => TestOutcome::Passed,
        Some(1) => TestOutcome::TestFailed("conformance".into()),
        Some(_) | None => TestOutcome::InfraError(
            InfraErrorKind::Subprocess,
            format!("conformance subprocess exited unexpectedly: {status:?}")
        ),
    })
}
```

**Note on argv**: cargo-pmcp's CLI takes `cargo-pmcp test conformance <URL>` when invoked as a subprocess directly (not via `cargo pmcp test conformance`). When the binary is invoked as `cargo-pmcp` (its actual binary name), the first `test` is a top-level subcommand. **Verification needed at plan-time**: confirm argv shape via `cargo-pmcp --help`. The recommended approach is `Command::new(current_exe()).arg("test").arg("conformance")` — no `cargo` prefix.

### Doctor check for missing `cargo:rerun-if-changed`

```rust
// cargo-pmcp/src/commands/doctor.rs (extension).

/// Walk workspace src/, grep for include_str! against widget paths,
/// confirm matched crates have build.rs with cargo:rerun-if-changed.
/// Returns issue count.
fn check_widget_rerun_if_changed(quiet: bool) -> u32 {
    use regex::Regex;

    let pattern = Regex::new(r#"include_str!\(\s*"[^"]*widgets?/[^"]*"\s*\)"#)
        .expect("regex compiles");

    let mut issues = 0u32;
    let workspace = std::path::Path::new(".");

    let crates_with_widget_include = scan_workspace_for_pattern(workspace, &pattern);
    for krate in crates_with_widget_include {
        let build_rs = krate.join("build.rs");
        let has_rerun = std::fs::read_to_string(&build_rs)
            .map(|s| s.contains("cargo:rerun-if-changed") || s.contains("cargo::rerun-if-changed"))
            .unwrap_or(false);
        if !has_rerun {
            if !quiet {
                println!(
                    "  {} crate `{}` includes widget files but has no build.rs cargo:rerun-if-changed; widget changes may not trigger recompilation.",
                    "!".yellow(),
                    krate.display(),
                );
                println!(
                    "    Fix: add the build.rs template documented at https://github.com/paiml/rust-mcp-sdk/blob/main/cargo-pmcp/docs/build-rs-template.md"
                );
            }
            issues += 1;
        }
    }
    issues
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual `npm run build` before `cargo pmcp deploy` | Auto-detected widget pre-build | This phase | Closes failure mode A. |
| `cargo clean` workarounds for `include_str!` cache hold-on | Generated `build.rs` with `cargo:rerun-if-changed` | This phase | Closes failure mode B. |
| Trust deploy success reporter | Post-deploy live verification | This phase | Closes failure mode C. |
| `AppValidationMode::ClaudeDesktop` placeholder ("same as Standard") | Real strict mode with per-handler ERROR rows | Phase 78 (shipped 2026-05-02) | Phase 79 v3 builds on this. |
| Phase 76: Add new `[iam]` section to `.pmcp/deploy.toml` via `#[serde(default, skip_serializing_if)]` | Pattern is now the standard for additive schema | Phase 76 (shipped 2026-04-22) | Phase 79 mirrors verbatim. |

**Deprecated/outdated:**
- The `cargo pmcp deploy --help` text that doesn't mention widgets (CONTEXT.md mandates fix).
- `commands/test/{check,conformance,apps}.rs::execute()` as black-box CLI handlers — they should eventually be reusable as library entry points via `mcp_tester::*` re-exports. Phase 79 doesn't fix this; flagged as deferred.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `warmup_grace_ms = 2000` is sufficient for cost-coach-class ARM64 Rust Lambdas without provisioned concurrency | Pitfall 3, post-deploy verifier defaults | LOW — operator can tune via config; default is conservative. AWS docs cite double-digit ms WITH PC; without PC, can be 200ms-2s for cold start. 2000ms covers cold start + warmup. |
| A2 | Subprocess-spawn (vs. in-process call) saves ~50ms per test | Reuse-vs-fork decision | LOW — 50ms is a rough order-of-magnitude estimate. Real overhead is process fork + TLS reinit; cargo-pmcp is a thin binary. Could be 100-200ms. Not load-bearing for v3 (vs. 5-10s test runtime). |
| A3 | `cargo:rerun-if-env-changed=PMCP_WIDGET_DIR` triggers re-run when env value changes | Pattern 4 (build.rs template) | LOW — verified against [Cargo Book](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-env-changed) explicit text. |
| A4 | All 4 deployment targets pick up `PMCP_WIDGET_DIR` via env-var inheritance to the spawned `cargo build` | Architectural Responsibility Map | MEDIUM — pmcp_run + aws_lambda use CDK which spawns `cargo build` via Node.js child_process; env var inheritance is the default but should be plan-time confirmed. cloudflare and google_cloud_run pipelines need verification. **Plan task: verify env-var inheritance in each target's `build()` call site.** |
| A5 | Cost-coach uses npm (`package-lock.json` present) | Convention search | VERIFIED — `[VERIFIED: ls /Users/guy/projects/mcp/cost-coach/widget/]` shows `package-lock.json`. |
| A6 | All 4 targets currently parse the same `.pmcp/deploy.toml` via `DeployConfig::load(&project_root)` and would automatically pick up new fields | Schema additions | VERIFIED — `[VERIFIED: cargo-pmcp/src/deployment/config.rs:450-462 + grep "DeployConfig::load" across targets]` All call `DeployConfig::load`. |
| A7 | Phase 78 `AppValidationMode::ClaudeDesktop` is real and ready for use by v3 | v3 unblocked | VERIFIED — `[VERIFIED: STATE.md line 5 "status: Phase 78 complete" + crates/mcp-tester/src/app_validator.rs:401-421 + lines 794+ per-signal Failed-row emission + 4 unit tests at lines 1476/1570/1601/1634]` |
| A8 | Argv shape for re-spawned cargo-pmcp test is `cargo-pmcp test conformance <URL>` not `cargo pmcp test conformance <URL>` | Subprocess spawn pattern | LOW — verifiable with `cargo-pmcp --help` at plan time. Both shapes work depending on entry point; current_exe() invokes the bare binary. |
| A9 | `cargo pmcp build` does NOT exist as a separate command (CONTEXT.md asks planner to check) | Escape hatches | VERIFIED — `[VERIFIED: ls /Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/]` no `build.rs` command. CONTEXT.md "deferred to discretion" item resolves to: NO command to wire. |
| A10 | All 4 targets' `rollback()` impls currently print "coming soon" — including pmcp_run | Reserved-but-rejected design | VERIFIED — `[VERIFIED: grep "fn rollback" all 4 target mod.rs]` aws_lambda:214, cloudflare:390, google_cloud_run:341, pmcp_run:397 — ALL 4 are placeholders. CONTEXT.md "operator confirmed UNTESTED" is correct; pmcp_run line 398 says "coming in Phase 2!" — never landed. |

---

## Open Questions (RESOLVED)

All five open questions resolved at planning time and locked in `79-00-PLAN.md` `<locked_planner_decisions>`. Inline cross-references below.

1. **Subprocess re-spawn argv shape (A8): does `cargo pmcp test ...` work as direct invocation?**
   - What we know: `current_exe()` returns the cargo-pmcp binary path. Invoking it as `<path> test conformance <URL>` should work.
   - What's unclear: cargo-pmcp's main.rs may dispatch differently when invoked as `cargo-pmcp` (with hyphen) vs. via `cargo pmcp ...` (cargo subcommand).
   - Recommendation: plan task verifies via `<binary> --help` at the start of post-deploy verifier work; adjust `Command::new(current_exe()).arg(...)` accordingly.
   - **RESOLVED:** master plan locked decision **#1** — verified 2026-05-03 against `cargo-pmcp test {check, conformance, apps} --help`; argv shape locked; uniform subprocess via `current_exe()`.

2. **Per-target env-var inheritance (A4): does `PMCP_WIDGET_DIR` reach `cargo build` invoked by Node.js CDK in pmcp-run?**
   - What we know: CDK uses `child_process.spawn("cargo", ...)` internally; node child processes inherit parent env by default.
   - What's unclear: if the CDK process is itself spawned with a sanitized env, the var won't propagate.
   - Recommendation: plan adds a smoke test for cost-coach. Empirically the existing IAM Phase 76 path-passes env vars through (CDK reads `PMCP_ORGANIZATION_ID`/`PMCP_SERVER_ID` per init.rs:592); the same mechanism applies to `PMCP_WIDGET_DIR`.
   - **RESOLVED:** master plan locked decision **#2** — set `PMCP_WIDGET_DIR` ONCE in orchestrator before `target.build`; 79-02 ships an integration test that asserts the env var reaches a child `cargo build` invocation.

3. **Hybrid in-process/subprocess for `check`?**
   - The connectivity probe is the cheapest step (~200ms for a real Lambda HTTP roundtrip). In-process via `mcp_tester::ServerTester::run_quick_test()` saves ~100ms vs. subprocess.
   - Conformance and apps are more expensive (5-10s) — subprocess overhead is rounding error.
   - Recommendation: plan-time decision. Pure subprocess is simpler; hybrid saves 100ms but doubles the abstraction.
   - **RESOLVED:** master plan locked decision **#3** — uniform subprocess for all three (`check`, `conformance`, `apps`); ~100ms saved by in-process check does not justify maintaining two abstractions.

4. **Should `--no-post-deploy-test` skip warmup grace as well?**
   - The 2000ms warmup is part of the verify lifecycle; if the user opts out of post-deploy tests, the warmup serves no purpose.
   - Recommendation: skip warmup when `--no-post-deploy-test` is set. (Side benefit: removes 2s from the no-test-deploy path.)
   - **RESOLVED:** master plan locked decision **#4** — yes; `enabled = false` short-circuits both warmup AND test invocations; 79-03 Task 2 implementation matches.

5. **Where should the runnable example demonstrate?**
   - cost-coach is a real fixture but is in a different repo (`/Users/guy/projects/mcp/cost-coach`).
   - A self-contained example needs a mock target (no real Lambda).
   - Recommendation: example shows `[[widgets]]` config wiring + the widget pre-build orchestrator in isolation against a tempdir + a fake `package.json` with a no-op build script. Don't try to demo full deploy. (This matches Phase 77's `multi_target_monorepo.rs` example which uses schema-direct setup.)
   - **RESOLVED:** master plan locked decision **#5** — `cargo-pmcp/examples/widget_prebuild_demo.rs` (NOT `deploy_with_widgets.rs`); schema-direct demo against tempdir.

---

## Environment Availability

External-tool audit per the agent spec.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `node` | Widget build (any PM) | ✓ | v20.8.1 | — |
| `npm` | Default PM fallback + widget build | ✓ | 10.1.0 | Fail-loud per CONTEXT.md |
| `bun` | Lockfile-detected PM | ✓ | 1.2.6 | npm fallback |
| `pnpm` | Lockfile-detected PM | ✓ | 10.15.0 | npm fallback |
| `yarn` | Lockfile-detected PM | ✓ | 1.22.22 | npm fallback |
| `cargo` | Workspace + scaffold | ✓ | 1.95.0 (2026-03-21) | — |
| `pmat` | CI quality gate (NOT pre-commit per Phase 75 D-07) | (CI) | 3.15.0 (pinned) | — |
| `make` | `make quality-gate` | ✓ | (system) | — |
| AWS Lambda runtime | Verify half target | (cloud) | — | Plan tests use mock; integration tests need real account or skip-gate |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** none missing on the dev host.

**Note:** for CI integration tests of the post-deploy verifier, a mock subprocess + a localhost loopback MCP server is the recommended fixture (avoids cloud account dependency + AWS billing).

---

## Validation Architecture

Per `nyquist_validation` enabled (config absent → treat as enabled per agent spec).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust stdlib + tokio runtime via `#[tokio::test]`) |
| Config file | none (cargo discovers via `tests/` and `#[cfg(test)]`) |
| Quick run command | `cargo test --package cargo-pmcp --features full -- --test-threads=1` |
| Full suite command | `make quality-gate` (fmt + clippy + build + tests + audit) |
| Integration test pattern | `cargo-pmcp/tests/<feature>_integration.rs` (e.g., `configure_integration.rs`, `iam_config.rs`) |
| Fuzz pattern | `cargo-pmcp/fuzz/fuzz_targets/fuzz_<area>.rs`; nightly-only execution; stable compile-check |
| Property pattern | `cargo-pmcp/tests/property_tests.rs` (existing) — proptest-style |
| Examples pattern | `cargo-pmcp/examples/<name>.rs`; runnable via `cargo run --example <name>` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-79-01 | Auto-detect `widget/` or `widgets/` only | unit | `cargo test --package cargo-pmcp --test widgets_config -- detection` | ❌ Wave 0 |
| REQ-79-02 | Lockfile→PM detection priority order | property | `cargo test --package cargo-pmcp --test widgets_config -- pm_detection_property` | ❌ Wave 0 |
| REQ-79-03 | Missing `build` script → error | unit | `cargo test --package cargo-pmcp --test widgets_orchestrator -- missing_build_script` | ❌ Wave 0 |
| REQ-79-04 | Missing `node_modules/` → install first | integration | `cargo test --package cargo-pmcp --test widgets_orchestrator -- runs_install_when_node_modules_missing` | ❌ Wave 0 |
| REQ-79-05 | Build failure stops deploy | integration | `cargo test --package cargo-pmcp --test widgets_orchestrator -- build_failure_aborts` | ❌ Wave 0 |
| REQ-79-06 | Generated build.rs uses PMCP_WIDGET_DIR | unit + manual | `cargo test --package cargo-pmcp --test widgets_config -- build_rs_template_shape` + manual: clone-template-build-twice-with-changed-PMCP_WIDGET_DIR | ❌ Wave 0 |
| REQ-79-07 | Doctor warns on missing rerun-if-changed | integration | `cargo test --package cargo-pmcp --test cli_acceptance -- doctor_widget_check` | ❌ Wave 0 (existing file exists; new test added) |
| REQ-79-08 | `[[widgets]]` schema parses + round-trips | unit + fuzz | `cargo test --package cargo-pmcp --test widgets_config -- round_trip` + nightly: `cargo +nightly fuzz run fuzz_widgets_config -- -max_total_time=60` | ❌ Wave 0 |
| REQ-79-09 | `embedded_in_crates` is source of truth | unit | `cargo test --package cargo-pmcp --test widgets_config -- embedded_in_crates_explicit` | ❌ Wave 0 |
| REQ-79-10 | `--no-widget-build`, `--widgets-only` flags | unit (clap parsing) + integration | `cargo test --package cargo-pmcp --test cli_acceptance -- deploy_no_widget_build` | ❌ Wave 0 (existing file exists; new test added) |
| REQ-79-11 | Warmup → check → conformance → apps lifecycle | integration | `cargo test --package cargo-pmcp --test post_deploy_orchestrator -- full_lifecycle` | ❌ Wave 0 |
| REQ-79-12 | `[post_deploy_tests]` schema parses + round-trips | unit + fuzz | `cargo test --package cargo-pmcp --test post_deploy_tests_config -- round_trip` + nightly fuzz | ❌ Wave 0 |
| REQ-79-13 | `on_failure="fail"` exits nonzero with loud docs | unit (rustdoc snapshot) + integration | `cargo test --package cargo-pmcp --test post_deploy_orchestrator -- on_failure_fail_exits_nonzero` | ❌ Wave 0 |
| REQ-79-14 | `on_failure="rollback"` parsed-but-rejected with WARN | unit | `cargo test --package cargo-pmcp --test post_deploy_tests_config -- rollback_reserved` | ❌ Wave 0 |
| REQ-79-15 | Distinguish test-failed from infra-error | integration (mock subprocess) | `cargo test --package cargo-pmcp --test post_deploy_orchestrator -- infra_error_distinct_exit_code` | ❌ Wave 0 |
| REQ-79-16 | Endpoint + auth pass-through | integration | `cargo test --package cargo-pmcp --test post_deploy_orchestrator -- auth_token_propagated` | ❌ Wave 0 |
| REQ-79-17 | Verify-side flags accepted | unit (clap) | `cargo test --package cargo-pmcp --test cli_acceptance -- deploy_verify_flags` | ❌ Wave 0 (extension) |
| REQ-79-18 | `--help` text mentions widgets | snapshot | `cargo test --package cargo-pmcp --test cli_acceptance -- deploy_help_mentions_widgets` | ❌ Wave 0 (extension) |

### Sampling Rate
- **Per task commit:** `cargo test --package cargo-pmcp --features full -- --test-threads=1` (~30s on a quiet machine)
- **Per wave merge:** `make quality-gate` (~3-5min — fmt + clippy + full test + audit)
- **Phase gate:** Full suite green + manual verification of cost-coach end-to-end deploy + `pmat quality-gate --fail-on-violation --checks complexity` exits 0 (matches CI per Phase 75 D-07).

### Wave 0 Gaps
- [ ] `cargo-pmcp/tests/widgets_config.rs` — covers REQ-79-01, REQ-79-02, REQ-79-06, REQ-79-08, REQ-79-09
- [ ] `cargo-pmcp/tests/widgets_orchestrator.rs` — covers REQ-79-03, REQ-79-04, REQ-79-05
- [ ] `cargo-pmcp/tests/post_deploy_tests_config.rs` — covers REQ-79-12, REQ-79-14
- [ ] `cargo-pmcp/tests/post_deploy_orchestrator.rs` — covers REQ-79-11, REQ-79-13, REQ-79-15, REQ-79-16
- [ ] `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` — fuzz for REQ-79-08, REQ-79-12 combined (TOML adversarial input)
- [ ] `cargo-pmcp/examples/deploy_with_widgets.rs` — demonstrates the widget pre-build orchestrator end-to-end
- [ ] Extensions to `cargo-pmcp/tests/cli_acceptance.rs` — covers REQ-79-07, REQ-79-10, REQ-79-17, REQ-79-18
- [ ] Framework install: none (cargo test is built in)

### PMAT cog-budget per new function (must stay ≤25)

| Function | Estimated cog | Pattern |
|----------|---------------|---------|
| `WidgetsConfig::is_empty` | 1 | trivial |
| `WidgetConfig::resolve_paths` | ≤10 | path joining |
| `PackageManager::detect_from_dir` | ≤8 | 4-way `if` chain |
| `run_widget_build` (orchestrator) | ≤15 | per-stage pipeline (Pattern 2 — P1) |
| `ensure_node_modules` | ≤12 | exists-check + spawn |
| `invoke_build_script` | ≤10 | spawn + status check |
| `verify_outputs_exist` | ≤8 | dir walk |
| `detect_widgets` (synthesize from convention) | ≤18 | 2-way fallback |
| `PostDeployTestsConfig::is_default` | ≤5 | comparison |
| `OnFailure::FromStr` | ≤8 | match w/ rollback handling |
| `run_post_deploy_tests` (orchestrator) | ≤20 | per-stage pipeline |
| `run_check_subprocess` | ≤12 | spawn + retry loop |
| `run_conformance_subprocess` | ≤10 | spawn + status interpretation |
| `run_apps_subprocess` | ≤12 | spawn + apps_mode flag handling |
| `format_failure_banner` | ≤8 | string formatting |
| `check_widget_rerun_if_changed` (doctor) | ≤18 | regex scan + per-crate check |
| `scan_workspace_for_pattern` (helper) | ≤15 | recursive file walk |

**Total estimated complexity budget:** all 17 new functions ≤20 cog avg = 0 PMAT escapees. If any function exceeds 25 during implementation, apply P1 (per-stage pipeline) or P4 (per-variant dispatch) per Phase 75 standard refactor toolkit. Hard cap: cog 50 with `// Why:`-annotated `#[allow]`.

---

## Security Domain

`security_enforcement` absent from config — treat as enabled per agent spec.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | OAuth tokens flow through `AuthFlags` (Phase 74); no new auth paths |
| V3 Session Management | no | No session state introduced |
| V4 Access Control | yes | Doctor check reads workspace files (read-only); subprocess inherits parent's auth context — no privilege escalation |
| V5 Input Validation | YES | TOML parse of new sections is the primary input vector. Fuzz target for `[[widgets]]` + `[post_deploy_tests]` mandatory. Schema-level validation for `on_failure` enum values, `apps_mode` enum values, `path` traversal (must reject `../` paths in `widgets[].path`) |
| V6 Cryptography | no | No new crypto primitives; auth tokens passed via stdlib subprocess env (or `--auth-token` flag) |
| V7 Error Handling and Logging | yes | Failure-mode banner contains live URL + suggestion of rollback command; ensure no auth tokens are echoed |
| V12 File and Resources | YES | Widget build SPAWNS arbitrary npm/yarn/etc commands. Threat: malicious `package.json` with `"build": "rm -rf /"`. Standard control: this is INHERENT to JS toolchain trust — same threat model as `npm install`. Document in security section as accepted risk; users running `cargo pmcp deploy` already trust their own `package.json`. |
| V14 Configuration | yes | New config fields default-on but skip-able; insecure-defaults check: `on_failure="fail"` default is safer than `"warn"` (which would silently ship broken code) |

### Known Threat Patterns for Rust CLI + JS-toolchain wrapper

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| TOML parser DoS via deeply nested or pathological input | Denial of Service | Fuzz target `fuzz_widgets_config` runs `toml::from_str::<DeployConfig>` on adversarial input; mirror Phase 76 `fuzz_iam_config` |
| Path traversal via `widgets[].path = "../../etc"` | Tampering, Information Disclosure | Reject paths containing `..` in `WidgetConfig::resolve_paths`; only accept relative paths under workspace root |
| Auth token leakage via `--auth-token` arg in process listing (`ps`) | Information Disclosure | Prefer env var passthrough (`PMCP_AUTH_TOKEN`) over flag for subprocess spawn; documented in `commands/auth/` already (Phase 74) — verify subprocess uses env-var path, not flag |
| Untrusted lockfile triggering wrong PM | Tampering | Lockfile presence is treated as authoritative; no parse-and-trust of lockfile contents — only the FILENAME is read. Safe. |
| Subprocess hangs forever (no timeout on widget build) | Denial of Service | `tokio::process::Command` + wrap with `tokio::time::timeout` at the widget-build orchestrator level; default timeout configurable, generous (10min default — JS builds can be slow) |
| `package.json` arbitrary script execution | Elevation of Privilege (in attacker's mental model) | Inherent to JS ecosystem. Document in --help: "cargo pmcp deploy will execute the build script defined in your package.json. Do not run against untrusted projects." |
| Subprocess inherits parent's full env (potential AWS creds leak to npm scripts) | Information Disclosure | The npm/pnpm/yarn build process inherits AWS_* env vars. This is normal Lambda-deploy posture and matches existing Phase 76 IAM template wiring. No new exposure introduced by this phase. |
| `current_exe()` resolves to a tampered binary | Spoofing | Binary integrity is OS-level; `current_exe()` is the safest available reference. `cargo install` + crates.io signing covers the upstream supply chain. |

---

## Sources

### Primary (HIGH confidence)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-/79-CONTEXT.md` — primary source of truth, all locked decisions.
- `/Users/guy/projects/mcp/cost-coach/drafts/proposal-pmcp-deploy-widget-build.md` — origin proposal.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/STATE.md` — Phase 78 status (line 5: complete), Phase 79 entry (line 107).
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/deploy/mod.rs` (lines 402, 407, 685-792) — orchestrator hook sites.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/deploy/init.rs` (lines 485-747) — Phase 76 IAM template-injection precedent.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/config.rs` (lines 7-36, 450-462, 736-763) — DeployConfig struct + IamConfig pattern.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/iam.rs` (lines 162, 341) — Phase 76 validation + render pattern.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/trait.rs` (line 230) — `DeploymentTarget::rollback`.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/test/mod.rs` (lines 11-396) — test command dispatcher + execute() signatures.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/test/check.rs` (lines 20-60) — check execute pattern.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/test/conformance.rs` (lines 16-90) — conformance execute pattern.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/test/apps.rs` (lines 32-199) — apps execute pattern.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/doctor.rs` (lines 15-200) — doctor extension precedent.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/app.rs` (lines 87-133) — `cargo pmcp app new` scaffold (target for build.rs injection).
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/app_validator.rs` (lines 401-446, 794+) — `AppValidationMode::ClaudeDesktop` real strict mode (Phase 78).
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/src/lib.rs` (lines 60-70) — public exports (`AppValidator`, `ConformanceRunner`, `ServerTester`) usable for direct in-process call if planner chooses hybrid.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` — Phase 76 fuzz pattern to mirror.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/Cargo.toml` (lines 3, 22, 25, 36-39) — version + dep audit.
- All 4 target rollback impl files: aws_lambda/mod.rs:214, cloudflare/mod.rs:390, google_cloud_run/mod.rs:341, pmcp_run/mod.rs:397 — confirmed all are placeholder "coming soon" stubs.
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-RESEARCH.md` — Phase 76 research as the schema-extension precedent.

### Secondary (MEDIUM confidence)
- [Cargo Book — Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) — `rerun-if-changed` and `rerun-if-env-changed` semantics.
- [AWS Lambda Provisioned Concurrency](https://docs.aws.amazon.com/lambda/latest/dg/provisioned-concurrency.html) — cold-start guidance informing warmup_grace_ms default.
- [rust-lang/cargo PR #14756](https://github.com/rust-lang/cargo/pull/14756) — `rerun-if-env-changed` config-table interaction (informational; doesn't affect this phase since we use shell env, not config).

### Tertiary (LOW confidence)
- A1 — `warmup_grace_ms = 2000` empirical sufficiency for cost-coach: not verified against actual cold-start logs. Operator should confirm.
- A2 — subprocess overhead estimate: not measured.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in workspace; verified via `Cargo.toml`.
- Architecture: HIGH — all hook sites read directly; Phase 76 precedent is structurally identical.
- Pitfalls: HIGH for #1, #2, #4, #5, #6 (verified); MEDIUM for #3 (warmup grace assumption).
- Validation Architecture: HIGH — pattern matches Phase 76/77 verbatim.
- Security Domain: HIGH — threat model is straightforward; no novel crypto/auth surface.
- Phase 78 dependency state: HIGH — STATE.md line 5 + AppValidationMode source code both confirm.

**Research date:** 2026-05-03
**Valid until:** 2026-06-03 (30 days for stable; bump to 7 days if Phase 78 follow-on or `cargo pmcp test` refactor lands first).

## RESEARCH COMPLETE

**Biggest planning risks (in order):**

1. **CONTEXT.md says Phase 78 is "NOT yet planned/executed" — this is stale. Phase 78 actually shipped (STATE.md line 5: `status: Phase 78 complete`; `AppValidationMode::ClaudeDesktop` is a real strict mode at `crates/mcp-tester/src/app_validator.rs:401-421` with per-signal Failed-row emission and 4 unit tests).** v3 is NOT blocked. The planner should ship all three (v1+v2+v3) in this phase per CONTEXT.md "default to all-three-in-this-phase" guidance, not split v3 as a follow-on.

2. **CONTEXT.md `build.rs` template is missing `cargo:rerun-if-env-changed=PMCP_WIDGET_DIR`.** Without it, two consecutive deploys with different widget output directories can hit a stale build-script result. Plan must include the corrected template (see Pattern 4 above) — this is a one-line fix that prevents a re-incidence of Failure Mode B.

3. **Reuse-vs-fork for `cargo pmcp test {check,conformance,apps}`: subprocess-spawn is the right call.** The existing `pub async fn execute(...)` signatures are CLI handlers with `colored::Colorize` stdout writes and `bail!` flow control — refactoring to library-callable shape is a separate phase's worth of work and would expand Phase 79's surface area significantly. Subprocess (via `std::env::current_exe()` for binary-version coupling) is simpler, matches the existing test-command UX (operators see the same banners they see in CI), and avoids a public-API churn on `mcp-tester`. File a follow-on phase to expose `mcp_tester` as a first-class library API for embedders.

4. **`on_failure="rollback"` reserved-but-rejected schema design needs both rustdoc + runtime WARN.** Operator UX risk: if the WARN fires only after the test failure, the operator may already have walked away from the terminal. Plan should fire the WARN at deploy-START when `on_failure="rollback"` is detected, not at test-failure-time. This way the operator sees "your config is reserved but won't auto-rollback" before the deploy commits.

5. **PMAT cog ≤25 hard cap is gated in CI; Phase 75 history shows orchestrator code routinely lands at cog 30-40 without aggressive decomposition.** Plan must specify per-function cog budgets (estimated table provided in Validation Architecture) and reserve P1/P4 refactor patterns from Phase 75 toolkit. Hard cap is cog 50 with `// Why:`-annotated `#[allow]` per Phase 75 D-03.

Sources:
- [Cargo Book — Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [AWS Lambda Provisioned Concurrency](https://docs.aws.amazon.com/lambda/latest/dg/provisioned-concurrency.html)
- [rust-lang/cargo PR #14756](https://github.com/rust-lang/cargo/pull/14756)
