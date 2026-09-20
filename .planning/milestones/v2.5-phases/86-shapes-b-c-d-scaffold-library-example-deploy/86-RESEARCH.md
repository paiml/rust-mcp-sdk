# Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy - Research

**Researched:** 2026-05-26
**Domain:** Rust CLI scaffolding (`cargo pmcp new`), library-use ergonomics (`pmcp-server-toolkit` + SQLite), config-driven Lambda deploy (`cargo pmcp deploy` + `pmcp::assets`)
**Confidence:** HIGH (all findings verified against the live codebase; no external/network claims required)

## Summary

Phase 86 wraps three developer-ergonomics "shapes" around the Phase 85 `pmcp-sql-server` pure-config core. Every primitive these three shapes need **already exists and is verified working** in the tree — `ServerBuilderExt`, `SqliteConnector`, `ServerConfig` (strict-parse), the `pmcp-sql-server::{run, run_serving, serve, build_server, dispatch}` pipeline, the `cargo pmcp new`/`deploy` commands, the `[assets]` bundling machinery, and the Phase 79 `run_post_deploy_tests` orchestrator. Phase 86 is overwhelmingly **wiring + emission + test work**, not new subsystem design.

The single biggest planning risk is the **≤15-line budget for Shape C** (D-07, SHAP-C-01). The Phase 85 `build_server` in `assemble.rs` is ~22 lines of body because it merges resources, synthesizes code-mode resources, and registers prompts. A naive Shape C `main.rs` that replicates `build_server` will blow the budget. The achievable ≤15-line shape uses the toolkit builder chain directly (`Server::builder().name().version().try_tools_from_config_with_connector().try_code_mode_from_config_with_connector().build()`) + `serve()`, *not* a re-implementation of `build_server`. See Code Examples §1 for a verified 14-line body. The existing `examples/sqlite_minimal.rs` and `examples/e01_toolkit_minimal.rs` are NOT runnable HTTP servers — they only synthesize tools and print — so Phase 86 must add a NEW serving example.

The second risk is the **inline DEV token_secret** (D-06): the toolkit rejects inline `token_secret` literals by default (`ConfigValidationError::InlineSecretRejected`). The generated `config.toml` must set `allow_inline_token_secret_for_dev = true` AND supply a ≥16-byte literal, or `try_code_mode_from_config_with_connector` errors at build time. This is the precise mechanism D-06 depends on and it is non-obvious.

Shape D needs **zero new deploy subsystem code**: `[assets] include = ["config.toml", "schema.sql"]` in `deploy.toml` already drives `bundle_assets_if_configured`, the `PmcpRun` target is selected by detection in `get_target_id`, the Lambda runtime resolves bundled assets via `pmcp::assets` (which maps to `/var/task` on Lambda), and `run_post_deploy_tests` already runs the `connectivity`/`conformance`/`apps` lifecycle. The "config-driven project detection" (D-09) is a *new build path that compiles the project's own crate* — the current `BinaryBuilder` runs `cargo lambda build` against the lambda package dir, so the scaffolded crate must be lambda-buildable.

**Primary recommendation:** Build Shape C first (the ≤15-line serving `main.rs` as a runnable toolkit example), prove the line budget and the inline-secret config shape, then make Shape B's `--kind sql-server` emit that exact `main.rs` + a `config.toml` + `schema.sql` + a single-crate `Cargo.toml`. Build Shape D as `[assets]`-config + detection wiring reusing the existing PmcpRun + Phase 79 lifecycle. Use the `parity_chinook.rs` spawn-poll-`ServerTester` pattern verbatim for TEST-05; use the `npm_skip_gate` early-return idiom for TEST-06's env gate.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** `cargo pmcp new --kind sql-server` is the command, taken literally (both SC-1 and TEST-05 name it). Teach `new` a `--kind` path producing a **single runnable crate** (`Cargo.toml` + `main.rs` + `config.toml` + `schema.sql`), distinct from `new`'s current multi-crate workspace scaffolding.
- **D-02:** SQLite-only backend for `--kind sql-server` this phase. `--backend postgres|mysql|athena` is a future additive sub-flag, NOT Phase 86.
- **D-03:** Keep the existing 526-line Rust `sqlite-explorer` template (`add --template sqlite-explorer`) UNTOUCHED as the Rust-driven escape-hatch. The new config-driven `--kind sql-server` is the v2.2 default. Do NOT remove or deprecate the Rust template this phase.
- **D-04:** Bundled `schema.sql` (DDL + a few INSERTs) bootstraps the demo SQLite DB on first run. Git-friendly, deterministic; the same DDL doubles as the `--schema`/code-mode schema resource. Rejected: a binary `.db` blob; an env-only `${SQLITE_DB_PATH}` path.
- **D-05:** HTTP transport everywhere — scaffold `main.rs` serves streamable HTTP (matching the Phase 85 binary + the Shape D Lambda target). One transport for scaffold + example + deploy. TEST-05 spawns the server on a local address and polls for readiness, then exercises `tools/list` + one `tools/call`. NO stdio support added.
- **D-06:** `[code_mode] enabled = true` by default in the generated config, with an **inline DEV-ONLY `token_secret`** + a loud "DEV ONLY — replace via a secrets ref for production" comment. Showcases v2.2's NL→SQL surface on first `cargo run` without an env var. The Shape D deploy path sources `token_secret` from a secrets ref, not the inline dev default. Rejected: `enabled = false` opt-in; `${CODE_MODE_SECRET}` env-only.
- **D-07:** Explicit `pmcp-server-toolkit` + connector wiring in ≤15 lines (load config+schema → build SQLite connector → `ServerBuilderExt`/`code_mode_from_config` → serve HTTP). Genuine "library use of the toolkit" per SHAP-C-01, and the **same shape the Shape B scaffold `main.rs` emits**. Relies on Phase 85's builder ergonomics. Rejected: a `pmcp_sql_server::run(config)` one-liner.
- **D-08:** SQLite-only runnable example using the toolkit's built-in SQLite connector, runs fully in CI (zero creds). **Deviation note for verifier:** SHAP-C-01's "+ a chosen `pmcp-toolkit-<backend>` crate" clause is satisfied *in intent* by the SQLite feature (SQLite's connector lives inside `pmcp-server-toolkit`, not a separate crate). Record as an approved scope reading, not a gap. (Mirrors Phase 85 REF-02 open-images deviation precedent.)
- **D-09:** Per-project build + asset bundle. `cargo pmcp deploy` detects a config-driven project (config.toml + schema present) and builds *its own* ≤15-line crate into a pure-Rust Lambda binary, bundling `config.toml` + `schema.sql` as deploy assets resolved via `pmcp::assets`. Reuses the existing deploy pipeline. Rejected: shipping the prebuilt universal `pmcp-sql-server` binary + uploading only config/schema.
- **D-10:** Existing `PmcpRun` target used unchanged — ZERO enum changes. The Phase 77 `TargetEntry` tagged enum (`PmcpRun`/`AwsLambda`/`GoogleCloudRun`/`CloudflareWorkers`) is NOT modified. Config-only-server support is detection-based in the deploy command. No new variant, no new field.
- **D-11:** TEST-06 runs against a REAL pmcp.run target behind a creds/env gate — authentic deploy + Phase 79 post-deploy lifecycle (`check` + `conformance` + `apps` verifier), skipped in normal CI when the gate is absent. SC-4 explicitly permits "a mock or real pmcp.run target". **Tension to record (not a gap):** no deploy assertion runs on *every* PR; CI stays green without creds. The verifier should treat the env-gated test as the SC-4 deliverable, not flag the absence of an always-on mock.

### Claude's Discretion

- The scaffold's exact `config.toml` comment text and the demo table schema/seed rows in `schema.sql` (e.g., a small books/movies/notes table).
- The `--kind` plumbing in `new.rs` (new code path vs branch in `execute`), template module layout under `cargo-pmcp/src/templates/`, and how the single-crate output differs structurally from the workspace path.
- Exact default HTTP bind address/port for the scaffolded server and how TEST-05 detects readiness (prefer a poll, per Phase 85 D-15 precedent).
- The inline DEV-ONLY `token_secret` literal value and the precise wording of the "replace for production" comment.
- How `cargo pmcp deploy` detects "config-driven project" (heuristic: presence of `config.toml` + `schema.sql` + the `pmcp-server-toolkit` dep).
- The env-var/feature name that gates TEST-06 (e.g., `PMCP_RUN_DEPLOY_TEST`).
- The exact ≤15-line example file name/location (must dep on toolkit + sqlite; likely `crates/pmcp-server-toolkit/examples/` — root `pmcp` `examples/` may lack the toolkit/sqlite deps; planner confirms).

### Deferred Ideas (OUT OF SCOPE)

- Non-SQLite scaffold backends (`--backend postgres|mysql|athena`) — additive sub-flag, deferred past Phase 86 (D-02).
- `pmcp_sql_server::run(config, schema)` library convenience — rejected for the SHAP-C-01 example (D-07); could land additively later.
- Always-on mock pmcp.run deploy test — Phase 86 chose the real-gated path (D-11).
- stdio transport for local scaffold/example — Phase 86 commits to HTTP everywhere (D-05).
- Deprecating/removing the Rust `sqlite-explorer` template — kept as escape-hatch (D-03); revisit Phase 89.
- Broad cargo-pmcp CLI README rewrite — only the Phase 86 command-surface additions are documented here; full README is Phase 89.
- SQLite `:table` identifier-substitution in `SqlConnector` — a connector-trait concern, NOT Phase 86; confirm Phase 84 already covers the curated `[[tools]]` the scaffold ships (it does — see Open Questions §1).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SHAP-B-01 | `cargo pmcp new --kind sql-server` scaffolds a single runnable crate (`Cargo.toml` + `main.rs` + `config.toml` + `schema.sql`); `cargo run` against embedded SQLite serves `tools/list` + ≥1 `tools/call` | `new.rs::execute` is the dispatch seam (currently NO `--kind` arg; the CLI `New {}` variant at `main.rs:92` has only `name`+`path`). Template machinery is raw `fs::write` of string literals (`templates/workspace.rs`). Single-crate emission mirrors the workspace generators. ≤15-line `main.rs` proven in Code Examples §1. |
| SHAP-C-01 | A runnable `examples/` server in ≤15 lines proving Shape C library use (toolkit + a backend connector) | `ServerBuilderExt` (`builder_ext.rs`) + `SqliteConnector` (`sql/sqlite.rs`) + `serve()` (`pmcp-sql-server/lib.rs:156`) compose to a 14-line body (Code Examples §1). D-08 deviation: SQLite connector is in-toolkit, not a separate crate. |
| SHAP-D-01 | `cargo pmcp deploy` packages a config-only server as a pure-Rust Lambda binary + deploys to pmcp.run; Phase 77 target system handles it with NO breaking changes | `bundle_assets_if_configured` (`builder.rs:78`) already zips bootstrap + `[assets] include` globs + special-cases `config.toml`. `PmcpRun` target selected by `get_target_id` detection (`deploy/mod.rs`). `TargetEntry` enum (`configure/config.rs:173`) stays untouched. `pmcp::assets` resolves to `/var/task` on Lambda (`src/assets/loader.rs:95-99`). |
| TEST-05 | Tempdir integration test: scaffold → spawn HTTP server → poll readiness → `tools/list` + one `tools/call` | Verbatim analog: `parity_chinook.rs` spawns via `run_serving`, polls with `ServerTester::test_initialize` backoff loop (20 attempts), then drives scenarios. mcp-tester API: `test_tools_list` (`tester.rs:1343`), `test_tool(name, args)` (`tester.rs:1501`), `call_tool_raw` (`tester.rs:1444`). `--test-threads=1` required (ephemeral port + per-process env). |
| TEST-06 | Env-gated deploy integration test against real pmcp.run target running Phase 79 post-deploy lifecycle | `run_post_deploy_tests(url, target_id, widgets_present, &PostDeployTestsConfig, quiet)` (`post_deploy_tests.rs:791`). Env-gate idiom: `npm_skip_gate()`-style early-return (`widgets_orchestrator.rs:39`) — print reason + `return` when the gate env var is absent. D-11 names `PMCP_RUN_DEPLOY_TEST` as the likely gate var. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Scaffold file emission (`new --kind sql-server`) | CLI / build tooling (`cargo-pmcp`) | — | `cargo pmcp` is the dev-tooling tier; emission is pure filesystem `fs::write`, no runtime. |
| ≤15-line wiring (`main.rs`) | API / server runtime (`pmcp-server-toolkit` + `pmcp`) | — | The wiring builds a `pmcp::Server` and serves HTTP — runtime tier. The toolkit owns config→handler synthesis; pmcp owns the HTTP transport + security layers. |
| SQLite connector + DB bootstrap | Database / storage (`SqliteConnector`, `rusqlite bundled`) | API runtime (executes DDL on first run) | Persistence + query execution is the storage tier; bootstrap is a runtime concern owned by the scaffolded `main.rs`. |
| Config parse + validation | API runtime (`ServerConfig`) | — | `deny_unknown_fields` strict parsing is a runtime-startup concern. |
| Code-mode policy + HMAC secret | API runtime (`code_mode`, `SqlCodeExecutor`) | Secrets (`SecretsProvider` / env) | Authorization + token signing run in the server; the secret reference resolves from env/secrets-manager (secrets tier). |
| Lambda build + asset bundle | CLI / build tooling (`BinaryBuilder`, `bundle_assets_if_configured`) | — | Cross-compilation + zip packaging is build-tooling; `cargo lambda build --arm64`. |
| Asset path resolution at runtime | Database/storage + API runtime (`pmcp::assets`) | — | `pmcp::assets` resolves `/var/task/assets/*` on Lambda, workspace-relative locally. |
| Deploy target selection + post-deploy verify | CLI / build tooling (`TargetRegistry`, `run_post_deploy_tests`) | — | Target dispatch + the `check`/`conformance`/`apps` lifecycle run in the deploy command. |

## Standard Stack

> Phase 86 introduces NO new runtime libraries. Every dependency below is already in the workspace and version-pinned. The scaffold's emitted `Cargo.toml` pins the same versions. Versions verified from the live `Cargo.toml` files (not crates.io — these are path crates in this workspace at the time of research).

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp-server-toolkit` | `0.1.0` (path) | Config-driven server primitives: `ServerConfig`, `ServerBuilderExt`, `SqliteConnector`, code-mode wiring | The anchor crate of v2.2; the whole "no Rust required" surface. `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml]` |
| `pmcp` | `2.8.1` (path), features `streamable-http` | Core SDK: `Server`, `ServerBuilder`, `StreamableHttpServer`, `pmcp::assets` | The transport + server builder + the asset loader Shape D relies on. `[VERIFIED: crates/pmcp-sql-server/Cargo.toml]` |
| `rusqlite` | `0.39`, feature `bundled` | Pure-Rust SQLite driver behind the toolkit `sqlite` feature | `bundled` = no system SQLite, no Docker (project no-Docker rule). `[VERIFIED: feedback_avoid_docker_pure_rust_lambda.md + Cargo.toml]` |
| `tokio` | `1`, features `macros`, `rt-multi-thread` | Async runtime for `#[tokio::main]` + `spawn_blocking` SQLite calls | SQLite connector runs sync `rusqlite` inside `spawn_blocking`. `[VERIFIED: sql/sqlite.rs:210]` |
| `clap` | `4`, features `derive`, `env` | CLI arg parsing for the scaffolded binary's `--config`/`--schema`/`--http` (and the `cargo pmcp new --kind` flag) | The Phase 85 binary uses `clap::Parser` for `Args`. `[VERIFIED: pmcp-sql-server/Cargo.toml]` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing` + `tracing-subscriber` | `0.1` / `0.3.20` (env-filter) | Structured logs from the scaffolded binary | The Phase 85 `run()` inits a best-effort `fmt` subscriber reading `RUST_LOG`. Scaffold `main.rs` should mirror. `[VERIFIED: pmcp-sql-server/lib.rs:248]` |
| `thiserror` | `2` | Error enums for the connector dispatch + run pipeline | Matches the connector crates' error style. `[VERIFIED: pmcp-sql-server/Cargo.toml]` |
| `mcp-tester` | `0.7.0` (path) — DEV-DEP ONLY | TEST-05 server-spawn + `tools/list`/`tools/call` harness; TEST-06 conformance | `ServerTester` + `ScenarioExecutor`. Dev-dependency only — NEVER a published dependency (CLAUDE.md publish order note). `[VERIFIED: pmcp-sql-server/Cargo.toml dev-deps]` |
| `tempfile` | `3` — DEV-DEP | TEST-05 tempdir for the scaffolded project | Used by `parity_chinook.rs`. `[VERIFIED]` |
| `proptest` | `1` — DEV-DEP | Property tests (CLAUDE.md ALWAYS) | Already in the dev-deps. `[VERIFIED]` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw `fs::write` string-literal templates | A template engine (`handlebars`, `tera`) | Existing `cargo-pmcp` templates (`workspace.rs`, `server_common.rs`, `sqlite_explorer.rs`) are ALL raw `fs::write` of `r#"..."#` literals + `format!`. Introducing an engine breaks the established pattern + adds a dep. STICK WITH raw `fs::write`. `[VERIFIED: templates/workspace.rs]` |
| `pmcp-sql-server::run()` library entry for Shape C | The toolkit `ServerBuilderExt` chain directly | D-07 LOCKED: the toolkit chain demonstrates *library use of the toolkit*, not *using the pmcp-sql-server binary*. The one-liner is REJECTED. |
| Shipping the prebuilt universal binary for deploy | Per-project `cargo lambda build` of the scaffold crate | D-09 LOCKED: per-project build (needs no platform-side hosting change). |

**Installation (scaffold-emitted `Cargo.toml` for the single crate):**
```toml
[package]
name = "<scaffolded-name>"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = { version = "2.8.1", features = ["streamable-http"] }
pmcp-server-toolkit = { version = "0.1.0", features = ["code-mode", "sqlite"] }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```
> ⚠️ The scaffold `Cargo.toml` must pin `pmcp-server-toolkit` with BOTH `code-mode` AND `sqlite` features (D-06 needs code-mode; D-02 needs sqlite). `[VERIFIED: pmcp-sql-server/Cargo.toml:33]`

**Version verification note:** these are path-crates in this workspace, not crates.io publishes yet (per STATE.md, `pmcp-server-toolkit 0.1.0` cannot ship until `pmcp 2.9.x` publishes Phase 82's `tool_arc` — Plan 83-09 publish-gate). The scaffold's emitted `Cargo.toml` should therefore use the **same version strings the workspace pins** so that once published, `cargo run` resolves from crates.io. Planner must decide whether the scaffold's deps point at crates.io versions or path deps for in-repo CI testing (TEST-05 likely needs path/patch deps in the tempdir — see Pitfall §1).

## Architecture Patterns

### System Architecture Diagram

```
SHAPE B (cargo pmcp new --kind sql-server)
  CLI args (name, --kind sql-server)
        │
        ▼
  new::execute  ──dispatch on --kind──▶  [NEW] single-crate emitter
        │ (existing: None → workspace path)        │
        │                                          ▼  fs::write × 4
        │                          ┌────────────────────────────────┐
        │                          │ <name>/Cargo.toml  (pinned deps)│
        │                          │ <name>/src/main.rs (Shape C)    │
        │                          │ <name>/config.toml (commented)  │
        │                          │ <name>/schema.sql  (DDL+INSERTs)│
        │                          └────────────────────────────────┘
        ▼
  prints next-steps (cd <name> && cargo run)

SHAPE C (the emitted main.rs == the runnable example) — RUNTIME
  read config.toml + schema.sql (fs)
        │
        ▼
  ServerConfig::from_toml_strict_validated ──▶ (deny_unknown_fields, validate)
        │
        ▼
  SqliteConnector::open(<db path>)  ──first run──▶ execute schema.sql DDL+INSERTs
        │                                          (bootstrap demo DB, D-04)
        ▼
  Server::builder()
      .name().version()
      .try_tools_from_config_with_connector(&cfg, connector)   ──▶ curated [[tools]]
      .try_code_mode_from_config_with_connector(&cfg, connector)──▶ validate_code/execute_code
      .resources_arc(...).build()
        │
        ▼
  serve(server, "127.0.0.1:PORT")  ──▶ StreamableHttpServer (CORS/DNS-rebind/security headers)
        │
        ▼
  MCP clients: initialize → tools/list → tools/call

SHAPE D (cargo pmcp deploy on a config-driven project) — BUILD + DEPLOY
  deploy::execute_async
        │
        ▼
  get_target_id (flag > deploy.toml target_type > default)  ──▶ "pmcp-run" (detection, D-10)
        │
        ▼
  BinaryBuilder.build()
      ├─ cargo lambda build --release --arm64   ──▶ bootstrap (pure-Rust, no Docker)
      └─ bundle_assets_if_configured
             reads [assets] include = ["config.toml","schema.sql"]
             zips bootstrap + assets ──▶ deployment.zip
        │
        ▼
  PmcpRunTarget.deploy(config, artifact)  ──▶ DeploymentOutputs { url }
        │
        ▼  (at Lambda runtime)  pmcp::assets resolves /var/task/assets/{config.toml,schema.sql}
        │
        ▼
  run_post_deploy_tests(url, "pmcp-run", widgets_present, &cfg, quiet)
        └─ subprocess: cargo pmcp test {check, conformance, apps} --format=json  (Phase 79)
```
> File-to-implementation mapping is in Component Responsibilities below — the diagram shows data flow only.

### Component Responsibilities

| Component | File | Role in Phase 86 |
|-----------|------|------------------|
| `new::execute` | `cargo-pmcp/src/commands/new.rs:29` | Dispatch seam. Currently takes `(name, path, tier, flags)` but `tier` is passed `None` from `main.rs:519`. Add a `--kind` arg + branch to a new single-crate emitter. |
| CLI `New {}` variant | `cargo-pmcp/src/main.rs:92` | Add `#[arg(long)] kind: Option<String>`; thread to `new::execute`. |
| `templates::workspace::generate` | `cargo-pmcp/src/templates/workspace.rs:9` | Pattern to mirror: each file = a `fs::write(dir.join("X"), r#"..."#)` fn. New module `templates::sql_server` (or similar) mirrors this. |
| `templates::sqlite_explorer` | `cargo-pmcp/src/templates/sqlite_explorer.rs` | UNTOUCHED (D-03). Reference only. |
| `ServerConfig` | `pmcp-server-toolkit/src/config.rs:102` | The strict-parse target for the emitted `config.toml`. `from_toml_strict_validated` is the production entry. |
| `ServerBuilderExt` | `pmcp-server-toolkit/src/builder_ext.rs:34` | The ≤15-line wiring trait. Key methods: `try_tools_from_config_with_connector`, `try_code_mode_from_config_with_connector`. |
| `SqliteConnector` | `pmcp-server-toolkit/src/sql/sqlite.rs:59` | `open(path)` / `open_in_memory()`; `execute(sql, params)`; `schema_text()`. Bootstrap DB by calling `execute` per DDL statement (see Pitfall §3). |
| `serve` | `pmcp-sql-server/src/lib.rs:156` | `serve(server, addr) -> (SocketAddr, JoinHandle)`. Non-blocking; the scaffold `main.rs` awaits the handle. NOTE: this is in `pmcp-sql-server`, not the toolkit — the scaffold should use `StreamableHttpServer` directly OR depend on `pmcp-sql-server`. See Pitfall §2. |
| `bundle_assets_if_configured` | `cargo-pmcp/src/deployment/builder.rs:466` | Reads `[assets] include` globs + special-cases `config.toml`; zips bootstrap + assets into `deploy/.build/deployment.zip`. |
| `pmcp::assets` loader | `src/assets/loader.rs:93` | `assets_base_path()` → `/var/task` on Lambda (`LAMBDA_TASK_ROOT`), workspace-relative locally. `load_string("config.toml")` etc. |
| `run_post_deploy_tests` | `cargo-pmcp/src/deployment/post_deploy_tests.rs:791` | The Phase 79 lifecycle: warmup → `connectivity`/`conformance`/`apps` subprocesses → `OnFailure` interpretation. |
| `ServerTester` / `ScenarioExecutor` | `crates/mcp-tester/src/tester.rs` | TEST-05 client: `test_initialize` (poll), `test_tools_list`, `test_tool(name, args)`. |

### Pattern 1: The ≤15-line Shape C wiring (D-07)
**What:** A complete HTTP MCP server from config + schema + SQLite connector via the toolkit builder chain — NOT a re-implementation of `pmcp-sql-server::build_server`.
**When to use:** Both the runnable `examples/` Shape C file AND the Shape B scaffold's emitted `main.rs`.
**Critical insight:** `pmcp-sql-server::build_server` (assemble.rs) is ~22 body lines because it merges resources, synthesizes code-mode resources, and registers prompts. The ≤15-line shape uses the toolkit builder chain directly and accepts the simpler resource surface (the `[[resources]]` from config via `StaticResourceHandler::from(&cfg)`). See Code Examples §1.

### Pattern 2: Single-crate emission vs workspace emission
**What:** `new --kind sql-server` produces ONE crate (`Cargo.toml` + `src/main.rs` + `config.toml` + `schema.sql`), NOT the `crates/`/`scenarios/`/`lambda/` workspace `new` currently emits.
**When to use:** Only the `--kind sql-server` branch. The `None`-kind path stays the existing workspace flow.
**Example:** `fs::create_dir_all(<name>/src)` then four `fs::write` calls. Mirror `templates/workspace.rs`'s structure (one `generate_X` fn per file).

### Pattern 3: Detection-based deploy (D-09 / D-10)
**What:** No new `TargetEntry` variant. The deploy command detects a config-driven project (heuristic: `config.toml` + `schema.sql` + `pmcp-server-toolkit` dep present) and routes to the per-project Lambda build, then reuses the `PmcpRun` target selected by `get_target_id`.
**When to use:** Shape D deploy.
**Example:** The `[assets] include = ["config.toml", "schema.sql"]` block in the scaffold's `deploy.toml` is what `bundle_assets_if_configured` already consumes — no code change needed for bundling. The detection seam is in `deploy::execute_async`'s `None` arm (before `target.build()`).

### Anti-Patterns to Avoid
- **Re-implementing `build_server` in the ≤15-line shape:** blows the line budget. Use the toolkit builder chain directly.
- **Inline `token_secret` without the dev flag:** `try_code_mode_from_config_with_connector` returns `ConfigValidationError::InlineSecretRejected`. MUST set `allow_inline_token_secret_for_dev = true` (D-06).
- **Loosening `deny_unknown_fields` to make the generated config parse:** the generated config must be a strict subset of `ServerConfig`'s known fields (config.rs:39-63 enumerates them). Add no unknown keys.
- **Adding a `TargetEntry` variant or `PmcpRunEntry` field:** violates D-10. Detection only.
- **Using a binary `.db` blob in the template:** violates D-04. Use `schema.sql` DDL+INSERTs.
- **Depending on `mcp-tester` as a runtime dep:** it is a dev-dependency only (CLAUDE.md publish-order note).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP transport + CORS + DNS-rebind protection | A hand-rolled axum server | `StreamableHttpServer::with_config` (via `serve()` or directly) | The SDK applies DNS-rebinding, CORS, and security-header layers (threat T-85-05-01). `[VERIFIED: pmcp-sql-server/lib.rs:124-163]` |
| SQLite driver | A C-SQLite FFI binding or query string-concat | `SqliteConnector` (`rusqlite bundled`) | Pure-Rust, parameterized binds via `raw_bind_parameter` (never concat — T-84-04-01). `[VERIFIED: sql/sqlite.rs]` |
| Config parsing + validation | Manual `toml` deserialization | `ServerConfig::from_toml_strict_validated` | `deny_unknown_fields` catches typos as parse errors; `validate()` catches empty required fields. `[VERIFIED: config.rs:197]` |
| Code-mode HMAC token signing | Hand-rolled HMAC | `try_code_mode_from_config_with_connector` → `SqlCodeExecutor` | 16-byte-min secret enforcement, policy gating (allow_writes/deletes/ddl), token TTL — all built. `[VERIFIED: builder_ext.rs:344]` |
| Lambda cross-compilation | A Docker build | `cargo lambda build --release --arm64` (`BinaryBuilder`) | Pure-Rust ARM64 via Zig; no Docker (project rule). `[VERIFIED: builder.rs:134]` |
| Asset bundling (zip + path resolution) | A manual zip + hardcoded `/var/task` path | `bundle_assets_if_configured` + `pmcp::assets` | Already zips bootstrap + globbed assets; `pmcp::assets` auto-detects Lambda vs local. `[VERIFIED: builder.rs:466, loader.rs:93]` |
| Post-deploy verification | A custom probe script | `run_post_deploy_tests` (Phase 79) | Runs `check`/`conformance`/`apps` as subprocesses with structured `OnFailure` exit-code semantics. `[VERIFIED: post_deploy_tests.rs:791]` |
| Server-spawn test harness | A raw HTTP client + JSON-RPC hand-coding | `ServerTester` + `ScenarioExecutor` (mcp-tester) | `test_initialize` (readiness poll), `test_tools_list`, `test_tool`. `[VERIFIED: tester.rs]` |
| Env-gated test skip | A custom CI-detection env check | The `npm_skip_gate()` early-return idiom | Print reason + `return` when gate absent; do NOT fail the suite. `[VERIFIED: widgets_orchestrator.rs:39]` |

**Key insight:** Phase 86 is almost entirely composition of existing, tested primitives. The only genuinely-new code is (1) the `--kind sql-server` template emitter, (2) the bootstrap-DB-from-schema.sql logic in the emitted `main.rs`, (3) the config-driven-project detection seam in deploy, and (4) the two integration tests. Everything else is wiring.

## Runtime State Inventory

> Phase 86 is greenfield emission + new tests + deploy wiring. It does NOT rename or migrate existing runtime state. This section is included because Shape D touches a *live* deploy target, which has cloud-side state.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None for scaffold/example (ephemeral SQLite, recreated from `schema.sql` each run). Shape D deploy creates a Lambda function + bundles `chinook`-style demo DB *as an asset* — but the demo SQLite is read-mostly and rebuildable. | None — verified ephemeral; DB is rebuildable from `schema.sql`. |
| Live service config | Shape D TEST-06 deploys to a **real pmcp.run target**. pmcp.run holds the deployed server's config/registration (URL, secrets, OAuth pool) in its managed platform, NOT in git. The deploy uses the existing `PmcpRun` target + `deploy.toml`. | TEST-06 must clean up after itself OR target a disposable test server name; planner should specify teardown (or accept that the env-gated test leaves a deployed server — D-11 records this as acceptable when run intentionally). |
| OS-registered state | None — no Task Scheduler/launchd/systemd/pm2 involvement. The scaffolded server is a plain `cargo run`. | None — verified. |
| Secrets/env vars | Code-mode `token_secret`: inline DEV literal locally (D-06), env/secrets-ref for deploy. Deploy reads secrets via `crate::secrets::resolve_secrets` from `.env` / Secrets Manager (`deploy/mod.rs:832`). TEST-06 gate env var (e.g. `PMCP_RUN_DEPLOY_TEST`) + pmcp.run creds. | Document the inline-dev-secret requires `allow_inline_token_secret_for_dev = true`; document the deploy secrets-ref path; name the TEST-06 gate var. |
| Build artifacts | Shape D produces `deploy/.build/{bootstrap, deployment.zip}` + `target/lambda/<name>/bootstrap`. TEST-05's tempdir build produces a `target/` inside the tempdir (or a shared target). | TEST-05 tempdir is auto-cleaned by `tempfile`. Shape D `deploy/.build` is a normal gitignored artifact. |

## Common Pitfalls

### Pitfall 1: TEST-05 dependency resolution in a tempdir
**What goes wrong:** The scaffolded `Cargo.toml` pins `pmcp-server-toolkit = "0.1.0"` etc. In a tempdir, `cargo run` resolves from crates.io — but `pmcp-server-toolkit 0.1.0` is NOT yet published (STATE.md: blocked on `pmcp 2.9.x` publish). The tempdir build will fail to resolve.
**Why it happens:** The scaffold emits crates.io-style version deps for end users, but CI has no published crate.
**How to avoid:** TEST-05 must inject `[patch.crates-io]` or path overrides pointing the tempdir's `Cargo.toml` at the in-repo crates, OR set a `CARGO_NET_OFFLINE`/workspace-member arrangement. Phase 85's `parity_chinook.rs` sidesteps this by calling `run_serving` *in-process* (no separate cargo build). The planner must decide: does TEST-05 actually shell out to `cargo run` in the tempdir (authentic but needs dep resolution), or does it compile the emitted `main.rs` in-process? D-01/SC-1 says "running `cargo run` ... verified end-to-end by an integration test in a tempdir" — favor an actual subprocess `cargo run` with a patch override.
**Warning signs:** `error: failed to select a version for pmcp-server-toolkit` in the test.

### Pitfall 2: `serve()` lives in `pmcp-sql-server`, not the toolkit
**What goes wrong:** The natural ≤15-line shape wants a one-call `serve(server, addr)`. That helper is defined in `pmcp-sql-server/src/lib.rs:156`, NOT in `pmcp-server-toolkit`. If the Shape C example is in `crates/pmcp-server-toolkit/examples/`, it can't call `pmcp_sql_server::serve` without a circular/extra dep.
**Why it happens:** Phase 85 put `serve` in the binary crate's lib, not the toolkit.
**How to avoid:** TWO options for the planner: (a) the scaffold/example call `StreamableHttpServer::with_config(addr, Arc::new(Mutex::new(server)), Default::default()).start().await` directly (3-4 lines — see Code Examples §1, this is what `serve()` itself does); or (b) lift a `serve`-equivalent helper into the toolkit (additive). Option (a) keeps the budget tight and adds no toolkit surface. The ≤15-line count in Code Examples §1 uses option (a).
**Warning signs:** `unresolved import pmcp_sql_server::serve` in a toolkit example.

### Pitfall 3: `SqliteConnector::execute` is single-statement; `schema.sql` is multi-statement
**What goes wrong:** Bootstrapping the DB by calling `connector.execute(whole_schema_sql, &[])` fails — `rusqlite`'s `prepare` handles ONE statement; the toolkit `SqlConnector::execute` prepares a single statement (`sql/sqlite.rs:221`). A multi-statement `schema.sql` (multiple `CREATE TABLE` + `INSERT`) won't run in one call.
**Why it happens:** `SqlConnector::execute` is query-shaped (prepare → bind → collect rows), not a batch executor. Phase 85's `schema_fixture.rs` test used `rusqlite`'s `execute_batch` (a dev-dep) precisely because the toolkit connector can't run multi-statement DDL (STATE.md Plan 85-03 note).
**How to avoid:** The scaffolded `main.rs` must either (a) split `schema.sql` on `;` and loop `connector.execute(stmt, &[])` per statement, or (b) open the `rusqlite::Connection` directly and call `execute_batch` before wrapping it in `SqliteConnector` — but `SqliteConnector` owns its `Connection` privately (`conn: Arc<Mutex<Connection>>`), so (b) needs a new constructor. Simplest within the budget: loop `execute` per `;`-split statement on first run (when the DB file is new/empty). The planner may want a small `bootstrap_schema(&connector, schema_sql)` helper (additive to the toolkit or local to the scaffold) — but that adds lines to the ≤15 budget UNLESS it's a toolkit helper. **Recommend: add a `SqliteConnector::execute_batch` or a toolkit `bootstrap_from_sql` helper** so the scaffold `main.rs` stays ≤15 lines. This is a likely Wave-0 task.
**Warning signs:** `Query("near \"...\": syntax error")` or only the first table created.

### Pitfall 4: Inline `token_secret` rejected by default (D-06)
**What goes wrong:** The generated `config.toml` ships `[code_mode] enabled = true` with an inline literal `token_secret = "dev-only-..."`. Without `allow_inline_token_secret_for_dev = true`, `try_code_mode_from_config_with_connector` returns `ToolkitError::Validation(ConfigValidationError::InlineSecretRejected)` and the server won't build.
**Why it happens:** Review R9 default-deny on inline secrets (config.rs:423, code_mode.rs:653).
**How to avoid:** Generated config MUST include `allow_inline_token_secret_for_dev = true` AND a literal ≥16 bytes (`HmacTokenGenerator` 16-byte minimum, code_mode.rs:626). Add the loud "DEV ONLY" comment per D-06.
**Warning signs:** `InlineSecretRejected` at first `cargo run`.

### Pitfall 5: `--test-threads=1` for server-spawn tests
**What goes wrong:** TEST-05 (and Phase 85's HTTP tests) bind ephemeral ports and set process-wide env (`CODE_MODE_SECRET`). Parallel test threads race on env + ports.
**Why it happens:** Project CI convention (`--test-threads=1`) + per-process env in code-mode secret resolution.
**How to avoid:** Run TEST-05/TEST-06 with `-- --test-threads=1`; bind `127.0.0.1:0` (ephemeral) and read the real bound addr back (as `serve()` returns it). Mirror `parity_chinook.rs` and `http_lazy_startup.rs`.
**Warning signs:** Flaky `Address already in use` or wrong-secret failures under parallel runs.

### Pitfall 6: `pmcp::assets` resolves `/var/task/assets/` — config must match
**What goes wrong:** The deployed Lambda's `config.toml` references `file_path = "/var/task/assets/<db>"` (Lambda asset path), but the bundled asset lands at a different path, so the SQLite open fails at runtime.
**Why it happens:** `pmcp::assets` Lambda base is `/var/task` (or `LAMBDA_TASK_ROOT`), and `bundle_assets_if_configured` places assets accordingly. The reference Chinook config already uses `file_path = "/var/task/assets/chinook.db"` (reference-config.toml:37).
**How to avoid:** The Shape D deploy config's `file_path` (and any asset reads) must use the `/var/task/assets/...` Lambda path; locally the scaffold uses a relative/in-memory path. The generated `config.toml` may need a deploy-vs-local distinction (env-expanded path, or two config profiles). Planner: confirm whether one config serves both or deploy uses an overridden `file_path` (Phase 85's parity test overrides the `file_path` line — same technique).
**Warning signs:** post-deploy `check` fails with a DB-open error.

## Code Examples

### 1. The verified ≤15-line Shape C `main.rs` (D-07, SHAP-C-01)
```rust
// Source: composed from pmcp-server-toolkit/src/builder_ext.rs + sql/sqlite.rs +
//         pmcp-sql-server/src/lib.rs:156 (serve) verified in this session.
// BODY line count = 14 (imports + attrs excluded, matching the "≤15-line main.rs" reading).
use std::sync::Arc;
use pmcp::Server;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig, StaticResourceHandler};
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(&std::fs::read_to_string("config.toml")?)?;
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open("demo.db".as_ref())?);
    let schema = std::fs::read_to_string("schema.sql")?;
    for stmt in schema.split(';').filter(|s| !s.trim().is_empty()) {
        conn.execute(stmt, &[]).await?;            // bootstrap DB (Pitfall §3) — see note
    }
    let server = Server::builder()
        .name(&cfg.server.name).version(&cfg.server.version)
        .try_tools_from_config_with_connector(&cfg, conn.clone())?
        .try_code_mode_from_config_with_connector(&cfg, conn)?
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .build()?;
    let http = StreamableHttpServer::with_config(
        "127.0.0.1:8080".parse()?, Arc::new(Mutex::new(server)), StreamableHttpServerConfig::default());
    let (_addr, handle) = http.start().await?;
    handle.await?;
    Ok(())
}
```
> **Line-budget note:** the body (between the `{` of `main` and the final `Ok(())`) is ~14 statements. The bootstrap loop (Pitfall §3) costs 3 lines; if a toolkit `SqliteConnector::execute_batch` / `bootstrap_from_sql` helper is added (Wave 0), the loop collapses to 1 line, leaving comfortable headroom. The `serve()` helper from `pmcp-sql-server` would save 2 lines but adds a dep (Pitfall §2) — the planner picks. **≤15 is achievable; recommend adding the batch helper to be safe.**

### 2. The toolkit builder chain (verified existing API)
```rust
// Source: crates/pmcp-server-toolkit/src/builder_ext.rs:344 (try_code_mode_from_config_with_connector)
//         + assemble.rs:411 (build_server uses exactly this chain).
let builder = Server::builder()
    .name(&cfg.server.name)
    .version(&cfg.server.version)
    .try_tools_from_config_with_connector(cfg, connector.clone())?  // curated [[tools]] → tool_arc
    .try_code_mode_from_config_with_connector(cfg, connector)?;     // validate_code + execute_code
```

### 3. `serve()` — the non-blocking HTTP start (verified)
```rust
// Source: crates/pmcp-sql-server/src/lib.rs:156
pub async fn serve(server: Server, addr: SocketAddr) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let shared = Arc::new(Mutex::new(server));
    let http = StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    http.start().await.map_err(RunError::Serve)  // returns (REAL bound addr, join handle)
}
```

### 4. TEST-05 spawn + readiness poll + tools/list + tools/call (pattern from parity_chinook.rs)
```rust
// Source: crates/pmcp-sql-server/tests/parity_chinook.rs:164-203 (spawn + poll)
//         + crates/mcp-tester/src/tester.rs:1343 (test_tools_list), :1501 (test_tool)
let url = format!("http://{bound}");
let mut tester = ServerTester::new(&url, Duration::from_secs(30), false, None, Some("http"), None)?;
let mut ready = false;
for attempt in 0..20u32 {
    if matches!(tester.test_initialize().await.status, mcp_tester::report::TestStatus::Passed) {
        ready = true; break;
    }
    tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;  // backoff
}
assert!(ready, "server must become ready");
let _list = tester.test_tools_list().await;                              // tools/list
let _call = tester.test_tool("search_tracks", json!({"query": "Rock"})).await?;  // tools/call
```
> ⚠️ For TEST-05 the server is the *scaffolded* crate, so the test must spawn `cargo run` in the tempdir (subprocess) rather than call `run_serving` in-process. Poll the subprocess's HTTP port the same way. Bind to a fixed port via the scaffold's default OR have the scaffold print its bound addr to stdout for the test to parse (D-15 readiness-poll precedent).

### 5. TEST-06 env-gate idiom (pattern from widgets_orchestrator.rs)
```rust
// Source: cargo-pmcp/tests/widgets_orchestrator.rs:39 (npm_skip_gate)
fn deploy_gate() -> Option<&'static str> {
    if std::env::var("PMCP_RUN_DEPLOY_TEST").is_ok() { None }
    else { Some("PMCP_RUN_DEPLOY_TEST not set — skipping real pmcp.run deploy integration test") }
}

#[tokio::test]
async fn config_only_deploy_runs_phase79_lifecycle() {
    if let Some(reason) = deploy_gate() { eprintln!("{reason}"); return; }  // skip, do NOT fail
    // ... cargo pmcp deploy against the gated pmcp.run target, then assert
    //     run_post_deploy_tests succeeded (check + conformance + apps).
}
```

### 6. `run_post_deploy_tests` signature (verified — what TEST-06 asserts ran cleanly)
```rust
// Source: cargo-pmcp/src/deployment/post_deploy_tests.rs:791
pub async fn run_post_deploy_tests(
    url: &str,
    target_id: &str,           // "pmcp-run"
    widgets_present: bool,
    config: &PostDeployTestsConfig,   // .checks default = ["connectivity","conformance","apps"]
    quiet: bool,
) -> std::result::Result<(), OrchestrationFailure>;
```

### 7. Generated `config.toml` MINIMAL valid shape (D-06, verified against config.rs + code_mode.rs)
```toml
# Source: distilled from reference-config.toml + config.rs (deny_unknown_fields field set)
[server]
name = "demo-sql-server"
version = "0.1.0"

[database]
type = "sqlite"
file_path = "demo.db"          # local; deploy overrides to /var/task/assets/demo.db (Pitfall §6)

[[database.tables]]
name = "books"
description = "Demo books table"

[code_mode]
enabled = true                 # D-06: headline NL→SQL visible on first run
allow_writes = false
require_limit = true
max_limit = 1000
# DEV ONLY — replace with a secrets ref (token_secret = "env:CODE_MODE_SECRET") for production.
token_secret = "dev-only-insecure-secret-min-16-bytes"
allow_inline_token_secret_for_dev = true   # REQUIRED for the inline literal (Pitfall §4)

[[tools]]
name = "list_books"
description = "List all books"
sql = "SELECT id, title FROM books ORDER BY title LIMIT :limit"

[[tools.parameters]]
name = "limit"
type = "integer"
required = false
default = 20
```
> Mandatory: `[server].name` + `[server].version` (validate() rejects empty). Optional everything else. `[code_mode]` is optional but D-06 wants it on. `[[tools]]` needs a non-empty `name`. The full known-field set is enumerated in `config.rs:39-63` — emit ONLY those keys (deny_unknown_fields).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 526-line hand-coded Rust `sqlite-explorer` template | Config-driven `--kind sql-server` (≤15-line `main.rs` + `config.toml`) | Phase 86 (this) | Two templates coexist (D-03): config-driven default + Rust escape-hatch. |
| `synthesize_from_config` (no connector) | `synthesize_from_config_with_connector` + `try_*_with_connector` | Phase 84 (CONN-01) | `tools/call` now executes SQL + emits `structuredContent`; the connector-less path only yields schemas. |
| Inline `token_secret` silently accepted | `InlineSecretRejected` unless `allow_inline_token_secret_for_dev` | Phase 83 (review R9) | D-06's inline dev secret MUST set the flag. |
| `SqlConnector` 2-method (Phase 83 minimized) | 3-method: `dialect` + `execute` + `schema_text` | Phase 84 | `SqliteConnector` is fully usable for `tools/call` now. |

**Deprecated/outdated:**
- The existing `examples/sqlite_minimal.rs` + `examples/e01_toolkit_minimal.rs` only build/synthesize + print — they do NOT serve HTTP. Phase 86's Shape C example must actually serve (D-05). The planner should decide whether to upgrade `sqlite_minimal.rs` in place or add a new `sql_server_http` example. Note `sqlite_minimal.rs` uses `database = ":memory:"` (no file/bootstrap) — the new example needs file-backed + schema.sql bootstrap.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | TEST-05 will shell out to a real `cargo run` in the tempdir (vs in-process compile), requiring a `[patch.crates-io]`/path override for the unpublished `pmcp-server-toolkit 0.1.0` | Pitfall §1, Code Examples §4 | If the planner chooses in-process compilation instead, the patch-override work is unneeded; if subprocess, the override is mandatory or the test can't resolve deps. |
| A2 | TEST-06 gate env var is `PMCP_RUN_DEPLOY_TEST` | Code Examples §5, TEST-06 | Cosmetic — D-11 leaves the exact name to discretion; any documented gate var works. |
| A3 | A `SqliteConnector::execute_batch` / toolkit `bootstrap_from_sql` helper will be added (Wave 0) so the ≤15-line budget holds with the multi-statement schema bootstrap | Pitfall §3, Code Examples §1 | If NOT added, the per-`;`-split loop costs ~3 lines; the budget still holds (14 lines) but is tighter and less clean. Verify the `;`-split is robust against `;` inside string literals in seed INSERTs (an edge case — keep seed data simple, no embedded semicolons). |
| A4 | The Shape D deploy config uses `/var/task/assets/<db>` for `file_path` on Lambda (matching reference-config.toml) and a local relative path locally | Pitfall §6 | If pmcp.run resolves assets at a different base than `/var/task`, the deployed DB open fails; the reference config + `loader.rs:95-99` strongly support `/var/task`. |
| A5 | `cargo lambda` + AWS/pmcp.run creds are available in the TEST-06 gated environment only (NOT normal CI) | Environment Availability | If a contributor sets the gate var without creds, the test will fail at deploy — document the dual requirement (gate var AND creds). |
| A6 | The scaffold's emitted `Cargo.toml` should pin published crates.io versions for end users (not workspace path deps) | Standard Stack | If `pmcp-server-toolkit` is still unpublished at Phase 86 ship, end-user `cargo run` of the scaffold can't resolve deps until the publish lands; this couples Phase 86's user-facing promise to the publish-gate (STATE.md Plan 83-09). Planner should note this dependency. |

## Open Questions

1. **Does Phase 84's curated `[[tools]]` synthesis already handle the SQLite `:param` binding the scaffold ships?**
   - What we know: `SqliteConnector::execute` binds `:name` named params via `raw_bind_parameter` (sql/sqlite.rs:143-167), and `synthesize_from_config_with_connector` filters to declared `[[tools.parameters]]` (STATE.md Plan 84-03). The reference `search_tracks`/`list_artists` tools use `:query`/`:limit`/`:offset` and pass the 29-scenario parity replay (Plan 85-06). So curated `:param` tools work.
   - What's unclear: The folded-todo's `:table` *identifier*-substitution (table name as a param) is a different concern and is OUT OF SCOPE (deferred). The scaffold's curated `[[tools]]` use value params only, which work.
   - Recommendation: Keep the scaffold's `[[tools]]` to value-`:param` queries (like the reference). Do NOT use table-name substitution. Confirmed safe.

2. **Where does the config-driven-project detection seam live in `deploy::execute_async`, and what triggers the per-project build vs the existing widget/workspace build?**
   - What we know: `execute_async`'s `None` arm calls `target.build(&config)` then `target.deploy()`. `BinaryBuilder.build()` runs `cargo lambda build` against the lambda package dir. `bundle_assets_if_configured` already bundles `config.toml` + `[assets]` globs.
   - What's unclear: For a single-crate config-driven project (NOT the multi-crate workspace `new` emits), does `find_lambda_package_dir` resolve correctly? The scaffolded single crate IS the lambda package, so it should — but this needs verification during planning (read `find_lambda_package_dir` in `builder.rs`).
   - Recommendation: Plan a Wave-0 spike task: scaffold the crate, run `cargo pmcp deploy --target-type pmcp-run` against it locally (dry/mock), and confirm `find_lambda_package_dir` + `bundle_assets_if_configured` pick up the single-crate layout + the `config.toml`/`schema.sql` assets. The detection heuristic (config.toml + schema.sql + toolkit dep) gates whether to add the `[assets]` block to the generated `deploy.toml`.

3. **Should the Shape C example be a NEW file or an upgrade of `sqlite_minimal.rs`?**
   - What we know: `sqlite_minimal.rs` is registered as a toolkit example with `required-features = ["sqlite", "code-mode"]` and only synthesizes+prints (no serve).
   - What's unclear: D-07 wants the example to be the SAME shape the scaffold emits (serving HTTP). Upgrading `sqlite_minimal.rs` to serve changes its current "build-only" purpose; adding a new example avoids churn but duplicates.
   - Recommendation: Add a NEW example (e.g. `sql_server_http.rs`) that serves; leave `sqlite_minimal.rs` as the build-only demo OR retire it if redundant. Planner's discretion (CONTEXT.md lists example location as discretion).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` + Rust toolchain | All shapes | ✓ | workspace MSRV 1.91 | — |
| `rusqlite bundled` (no system SQLite) | Shape B/C SQLite | ✓ (compiled in) | 0.39 | — (bundled is the point — no system dep) |
| `cargo lambda` | Shape D Lambda build | ✗ (not verified installed; `BinaryBuilder.ensure_cargo_lambda` checks at runtime) | — | Shape D deploy fails fast with an install hint if absent; TEST-06 is env-gated so normal CI skips it. `[VERIFIED: builder.rs:89]` |
| AWS / pmcp.run credentials | Shape D TEST-06 deploy | ✗ (gated) | — | TEST-06 skips when `PMCP_RUN_DEPLOY_TEST` (or chosen gate) absent (D-11). |
| `mcp-tester` (dev-dep) | TEST-05 client, TEST-06 conformance | ✓ (path dev-dep) | 0.7.0 | — |
| Docker | NONE | n/a | — | Project rule: NO Docker (pure-Rust Lambda). `cargo lambda` uses Zig, not Docker. |
| Network (crates.io) | TEST-05 scaffold dep resolution (if subprocess `cargo run`) | conditionally | — | Use `[patch.crates-io]`/path override in the tempdir (Pitfall §1) to avoid needing the unpublished crate from crates.io. |

**Missing dependencies with no fallback:**
- None that block the core phase. `cargo lambda` + cloud creds gate ONLY Shape D's real deploy (TEST-06), which is intentionally opt-in (D-11).

**Missing dependencies with fallback:**
- `cargo lambda` / cloud creds → TEST-06 env-gate skip (normal CI green).
- crates.io published `pmcp-server-toolkit` → path/patch override for TEST-05.

## Validation Architecture

> `workflow.nyquist_validation` is ABSENT from `.planning/config.json` → treated as ENABLED. Section included.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` (cargo test) + `proptest 1` + `mcp-tester 0.7.0` harness |
| Config file | none (cargo convention); per-crate `[dev-dependencies]` + `[[example]]` in `Cargo.toml` |
| Quick run command | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode -- --test-threads=1` |
| Full suite command | `cargo test -p pmcp-server-toolkit -p pmcp-sql-server -p cargo-pmcp --features sqlite,code-mode -- --test-threads=1` then `make quality-gate` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SHAP-B-01 | `new --kind sql-server` emits 4 files; emitted project `cargo run` serves `tools/list` + 1 `tools/call` | integration (tempdir) | `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` | ❌ Wave 0 |
| SHAP-C-01 | ≤15-line example serves HTTP; `tools/list` + `tools/call` succeed | integration + runnable example | `cargo run -p pmcp-server-toolkit --example sql_server_http --features sqlite,code-mode` + a spawn-poll test | ❌ Wave 0 |
| SHAP-C-01 | example body is ≤15 lines | unit/lint (line-count assertion or doc) | grep/line-count check in the example test | ❌ Wave 0 |
| SHAP-D-01 | config-driven project detection + `[assets]` bundling + PmcpRun reuse with NO enum change | unit (detection) + integration (bundle) | `cargo test -p cargo-pmcp deploy_config_driven -- --test-threads=1`; plus a `TargetEntry`-unchanged regression assert | ❌ Wave 0 |
| TEST-05 | tempdir scaffold → spawn → poll → `tools/list` + 1 `tools/call` | integration | `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` | ❌ Wave 0 |
| TEST-06 | env-gated real pmcp.run deploy → Phase 79 `check`+`conformance`+`apps` clean | integration (gated) | `PMCP_RUN_DEPLOY_TEST=1 cargo test -p cargo-pmcp --test deploy_config_only -- --test-threads=1` | ❌ Wave 0 |
| ALWAYS | property test on config emission / parse round-trip | property | `cargo test -p ... proptest -- --test-threads=1` | ❌ Wave 0 |
| ALWAYS | fuzz reuse (config parser already fuzzed Phase 84) | fuzz | `cargo fuzz run pmcp_server_toolkit_config_parser` (reuse existing corpus) | ✓ corpus exists |
| ALWAYS | doctests on any new public toolkit helper (e.g. `bootstrap_from_sql`) | doctest | `cargo test --doc -p pmcp-server-toolkit --features sqlite,code-mode` | ❌ Wave 0 (if helper added) |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched-crate> --features sqlite,code-mode -- --test-threads=1`
- **Per wave merge:** full suite across `pmcp-server-toolkit`, `pmcp-sql-server`, `cargo-pmcp` + `cargo run --example` for the Shape C example.
- **Phase gate:** `make quality-gate` green (fmt + clippy pedantic/nursery + build + test + audit) before `/gsd-verify-work`; PMAT cog ≤25 in CI.

### Wave 0 Gaps
- [ ] `cargo-pmcp/tests/scaffold_sql_server.rs` — TEST-05 tempdir scaffold→spawn→poll→tools/list+call (with `[patch.crates-io]` override — Pitfall §1)
- [ ] `cargo-pmcp/tests/deploy_config_only.rs` — TEST-06 env-gated real-pmcp.run deploy + Phase 79 lifecycle assertion
- [ ] `crates/pmcp-server-toolkit/examples/sql_server_http.rs` (or upgraded `sqlite_minimal.rs`) — the runnable ≤15-line serving example (SHAP-C-01)
- [ ] `[[example]]` registration in `crates/pmcp-server-toolkit/Cargo.toml` with `required-features = ["sqlite", "code-mode"]`
- [ ] `cargo-pmcp/src/templates/sql_server.rs` (new template module) + `pub mod sql_server;` in `templates/mod.rs`
- [ ] `--kind` arg on the CLI `New {}` variant (`main.rs:92`) + threading through `new::execute` (`new.rs:29`, currently passes `None` for tier at `main.rs:519`)
- [ ] (Recommended) `SqliteConnector::execute_batch` or toolkit `bootstrap_from_sql` helper (Pitfall §3) + its doctest
- [ ] Property test for config-emission/parse round-trip; reuse the existing config-parser fuzz corpus
- [ ] Detection-seam unit test asserting `TargetEntry` enum is unchanged (D-10 regression guard)

## Security Domain

> `security_enforcement` is ABSENT from config → treated as ENABLED. Section included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (Shape D) | `StaticAuthProvider`/`BearerAuthProvider` from toolkit; pmcp.run OAuth pool for deploy. Scaffold local default may be unauthenticated dev. |
| V3 Session Management | partial | `StreamableHttpServer` default is stateful w/ `AllowedOrigins::localhost()` (lib.rs:127). |
| V4 Access Control | yes | Code-mode policy gating (`allow_writes`/`allow_deletes`/`allow_ddl`/`require_limit`) — static `[code_mode]` flags ARE the authorization (Plan 85-02 NoopPolicyEvaluator). |
| V5 Input Validation | yes | `ServerConfig` `deny_unknown_fields` (typos = parse errors); `[[tools.parameters]]` typed validation; SQL params bound via `raw_bind_parameter` (never concatenated). |
| V6 Cryptography | yes | Code-mode HMAC token signing via `HmacTokenGenerator` (16-byte-min secret). NEVER hand-roll. Inline secret rejected by default (R9). |
| V7 Error Handling / Logging | yes | `DispatchError`/`RunError` Display are credential-free (never echo file paths/URLs/secrets — T-85-04-01); `SqliteOpen` is path-free. |

### Known Threat Patterns for {Rust config-driven SQL MCP server + Lambda}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via tool params | Tampering | Parameterized `:name` binds via `raw_bind_parameter` (sql/sqlite.rs:160); `translate_placeholders`. NEVER string-concat. |
| Committed secret in scaffold `config.toml` | Information Disclosure | Inline `token_secret` rejected unless `allow_inline_token_secret_for_dev=true`; loud DEV-ONLY comment (D-06); deploy uses env/secrets-ref. |
| Config typo silently weakening policy | Tampering | `deny_unknown_fields` makes typos hard parse errors (config.rs:101); `validate()` catches empty required fields. |
| Credential leak in deploy logs/errors | Information Disclosure | Connector errors redacted at source (`sanitize_url`/`strip_aws_credentials`); deploy secrets never written to `deploy.toml` (transient env only, deploy/mod.rs:846). |
| Forged code-mode approval token | Spoofing | HMAC-signed approval tokens with TTL; `execute_code` invalid-token rejection (verified in 29-scenario parity). |
| Write/DDL on a read-only demo | Elevation of Privilege | `allow_writes=false`/`allow_ddl=false` reject INSERT/DELETE/DROP at validation; scaffold ships read-only-leaning defaults. |
| DNS rebinding against the local server | Spoofing | `StreamableHttpServer` `AllowedOrigins::localhost()` default; never hand-roll the HTTP layer. |

## Sources

### Primary (HIGH confidence) — verified by direct file read this session
- `crates/pmcp-sql-server/src/{lib.rs,main.rs,assemble.rs,dispatch.rs}` — Phase 85 pipeline, `serve()`, `build_server`, dispatch.
- `crates/pmcp-server-toolkit/src/{builder_ext.rs,config.rs,sql/sqlite.rs,code_mode.rs,lib.rs}` — `ServerBuilderExt`, `ServerConfig`, `SqliteConnector`, inline-secret rejection, crate-root re-exports.
- `crates/pmcp-sql-server/tests/{parity_chinook.rs,http_lazy_startup.rs}` + `tests/fixtures/{reference-config.toml,chinook.ddl}` — TEST-05 spawn/poll pattern, config shape, DDL shape.
- `crates/pmcp-server-toolkit/examples/{sqlite_minimal.rs,e01_toolkit_minimal.rs}` + toolkit `Cargo.toml` — existing (non-serving) examples + `[[example]]` registration.
- `cargo-pmcp/src/commands/new.rs` + `cargo-pmcp/src/main.rs:83-219,519` — `new::execute`, CLI `New {}`/`Deploy` variants.
- `cargo-pmcp/src/templates/{mod.rs,workspace.rs}` — raw `fs::write` template pattern.
- `cargo-pmcp/src/commands/deploy/{deploy.rs,mod.rs}` — deploy flow, `get_target_id`, `execute_async` None arm, post-deploy call.
- `cargo-pmcp/src/commands/configure/config.rs` — `TargetEntry` tagged enum (D-10 unchanged target).
- `cargo-pmcp/src/deployment/{trait.rs,registry.rs,builder.rs,config.rs,post_deploy_tests.rs}` — DeploymentTarget trait, registry, `BinaryBuilder`+`bundle_assets_if_configured`, `AssetsConfig`, `run_post_deploy_tests`.
- `src/assets/{mod.rs,loader.rs}` — `pmcp::assets` Lambda `/var/task` path resolution.
- `crates/mcp-tester/src/tester.rs` — `ServerTester` API (`test_initialize`, `test_tools_list`, `test_tool`, `call_tool_raw`).
- `.planning/phases/86-.../86-CONTEXT.md`, `.planning/STATE.md`, `.planning/config.json`, `CLAUDE.md`, `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md`.

### Secondary (MEDIUM confidence)
- `widgets_orchestrator.rs:39` (npm_skip_gate) — env-gate skip idiom applied to TEST-06 (analogous, not identical use-case).

### Tertiary (LOW confidence)
- None — no external/network sources were needed; all claims are codebase-verified.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dep verified in live `Cargo.toml` files; no new libraries.
- Architecture: HIGH — all wiring primitives read end-to-end; the ≤15-line shape composed and line-counted against verified APIs.
- Pitfalls: HIGH — each pitfall traced to a specific file/line (inline-secret rejection, single-statement execute, serve() location, dep-resolution, asset path).
- Deploy (Shape D): MEDIUM-HIGH — bundling/target/lifecycle machinery verified; the single-crate `find_lambda_package_dir` resolution + detection seam placement need a Wave-0 confirmation spike (Open Question §2).

**Research date:** 2026-05-26
**Valid until:** 2026-06-25 (30 days — stable in-repo APIs; the one moving part is the crates.io publish status of `pmcp-server-toolkit`, which affects scaffold dep pinning, Assumption A6).
