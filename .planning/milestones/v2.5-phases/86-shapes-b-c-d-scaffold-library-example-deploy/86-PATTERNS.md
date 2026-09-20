# Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy - Pattern Map

**Mapped:** 2026-05-26
**Files analyzed:** 11 (4 new, 7 modified)
**Analogs found:** 11 / 11

## File Classification

| New/Modified File | New/Mod | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|---------|------|-----------|----------------|---------------|
| `cargo-pmcp/src/templates/sql_server.rs` | NEW | template / file-emitter | file-I/O (`fs::write`) | `cargo-pmcp/src/templates/workspace.rs` | exact (role + flow) |
| `crates/pmcp-server-toolkit/examples/sql_server_http.rs` | NEW | example / server bin | request-response (HTTP serve) | `crates/pmcp-sql-server/src/{lib.rs::serve, assemble.rs::build_server}` + `examples/sqlite_minimal.rs` (shape) | role-match (serve lifted from sql-server lib) |
| `cargo-pmcp/tests/scaffold_sql_server.rs` | NEW | integration test | event-driven (spawn → poll → call) | `crates/pmcp-sql-server/tests/parity_chinook.rs` | exact (spawn-poll-ServerTester) |
| `cargo-pmcp/tests/deploy_config_only.rs` | NEW | integration test (gated) | request-response (deploy → verify) | `cargo-pmcp/tests/widgets_orchestrator.rs::npm_skip_gate` + `post_deploy_tests::run_post_deploy_tests` | role-match (gate idiom + verifier call) |
| `cargo-pmcp/src/commands/new.rs` | MOD | command dispatch | branch/transform | its own `execute` (workspace dispatch) | exact (extend in place) |
| `cargo-pmcp/src/main.rs` | MOD | CLI variant | arg parse | `New {}` variant + `Dev {}`'s `#[arg(long)]` fields | exact |
| `cargo-pmcp/src/templates/mod.rs` | MOD | module index | — | existing `pub mod` lines | trivial |
| `cargo-pmcp/src/commands/deploy/mod.rs` (+`deploy.rs`/`init.rs`) | MOD | command / build seam | request-response + file-I/O (bundle) | `execute_async` None-arm + `bundle_assets_if_configured` | exact (extend existing seam) |
| `crates/pmcp-server-toolkit/src/sql/sqlite.rs` | MOD | connector helper | batch file-I/O (DDL exec) | its own `SqliteConnector::open` + `execute` | role-match (new `execute_batch`/`bootstrap_from_sql`) |
| `crates/pmcp-server-toolkit/Cargo.toml` | MOD | manifest / example reg | config | existing `[[example]]` blocks (lines 70-76) | exact |
| (emitted) scaffold `config.toml` / `schema.sql` / `Cargo.toml` / `main.rs` | NEW (string literals) | template payload | — | `reference-config.toml` + RESEARCH §1 main.rs | exact |

---

## Pattern Assignments

### `cargo-pmcp/src/templates/sql_server.rs` (NEW — template / file-emitter, file-I/O)

**Analog:** `cargo-pmcp/src/templates/workspace.rs` (verified read end-to-end).

**The canonical emitter shape** — one `generate()` orchestrator that calls one private `generate_<file>` fn per output file, each a single `fs::write(dir.join("X"), <literal>).context(...)`. Mirror this exactly; do NOT introduce a template engine (RESEARCH "Alternatives Considered": all existing templates are raw `fs::write` of `r#"..."#`/`format!`).

```rust
// Source: cargo-pmcp/src/templates/workspace.rs:1-17
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

pub fn generate(workspace_dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(workspace_dir, name)?;
    generate_makefile(workspace_dir, name)?;
    generate_readme(workspace_dir, name)?;
    generate_gitignore(workspace_dir)?;
    println!("  {} Generated workspace files", "✓".green());
    Ok(())
}
```

**Per-file emitter pattern — raw literal (no interpolation needed):**
```rust
// Source: cargo-pmcp/src/templates/workspace.rs:19-69
fn generate_cargo_toml(workspace_dir: &Path, _name: &str) -> Result<()> {
    let content = r#"[workspace]
resolver = "2"
...
"#;
    fs::write(workspace_dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;
    Ok(())
}
```

**Per-file emitter pattern — interpolated (when the crate `name` is injected):**
```rust
// Source: cargo-pmcp/src/templates/workspace.rs:111-114, 262-266
fn generate_readme(workspace_dir: &Path, name: &str) -> Result<()> {
    let content = format!(r#"# {}
...
"#, name, name);  // NOTE: literal `{{ }}` braces escape JSON/curly content
    fs::write(workspace_dir.join("README.md"), content).context("Failed to create README.md")?;
    Ok(())
}
```

> For Phase 86 the new module emits FOUR files into a SINGLE crate dir (not a workspace): `Cargo.toml`, `src/main.rs`, `config.toml`, `schema.sql`. Each gets its own `generate_<file>` fn. The `main.rs` body is the verified ≤15-line shape (see Shared Pattern: ≤15-line wiring). `config.toml` is the literal in Shared Pattern: generated config. Note `format!` requires `{{`/`}}` to escape literal braces (workspace.rs:243 escapes JSON-RPC examples this way) — relevant if the emitted `main.rs`/`config.toml` contains `{}`.

---

### `crates/pmcp-server-toolkit/examples/sql_server_http.rs` (NEW — example / serving HTTP server, request-response)

**Analog (shape):** `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` (file layout, attrs, feature-gating).
**Analog (wiring + serve):** `crates/pmcp-sql-server/src/assemble.rs::build_server` (lines 401-423) + `crates/pmcp-sql-server/src/lib.rs::serve` (lines 156-164).

**CRITICAL — do NOT re-implement `build_server` (anti-pattern, blows the ≤15-line budget).** `build_server` is ~22 body lines (merges 3 resources, synthesizes code-mode instructions/policies resources, registers prompts). The example uses the toolkit builder chain DIRECTLY + a 2-line `StreamableHttpServer` start. See Shared Pattern: ≤15-line wiring for the verified 14-line body.

**Existing example file conventions to mirror** (imports, attrs, run-command doc, feature-gate):
```rust
// Source: crates/pmcp-server-toolkit/examples/sqlite_minimal.rs:1-17, 29-36
//! Run with:
//! ```sh
//! cargo run --example sql_server_http --features sqlite,code-mode -p pmcp-server-toolkit
//! ```
use std::sync::Arc;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
// ... (sqlite_minimal uses #[tokio::main(flavor = "current_thread")] — but a
//      serving example needs the multi-thread runtime; use plain #[tokio::main])
```

> Two deviations from `sqlite_minimal.rs` the planner must apply: (1) `sqlite_minimal` only synthesizes + prints (NO serve) — the new example MUST actually serve HTTP (D-05); (2) `sqlite_minimal` uses `database = ":memory:"` — the new one is file-backed + bootstraps from `schema.sql` (D-04). `serve()` lives in `pmcp-sql-server`, NOT the toolkit (Pitfall §2) — the example inlines `StreamableHttpServer::with_config(...).start()` (3-4 lines, what `serve()` itself does) rather than importing `pmcp_sql_server::serve`.

**The `serve()` body to inline (verified):**
```rust
// Source: crates/pmcp-sql-server/src/lib.rs:156-164
pub async fn serve(server: Server, addr: SocketAddr) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let shared = Arc::new(Mutex::new(server));
    let http = StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    http.start().await.map_err(RunError::Serve)  // returns (REAL bound addr, handle)
}
```

---

### `cargo-pmcp/tests/scaffold_sql_server.rs` (NEW — integration test, event-driven spawn→poll→call)

**Analog:** `crates/pmcp-sql-server/tests/parity_chinook.rs` (verified read, lines 113-211) — the canonical spawn → readiness-poll → `ServerTester` drive pattern.

**Readiness-poll loop (copy verbatim; 20 attempts, linear backoff):**
```rust
// Source: crates/pmcp-sql-server/tests/parity_chinook.rs:171-194
let url = format!("http://{bound}");
let mut tester = ServerTester::new(
    &url, Duration::from_secs(30),
    false,        // insecure
    None,         // api_key
    Some("http"), // force_transport
    None,         // http_middleware_chain
)?;
let mut initialized = false;
for attempt in 0..20u32 {
    let result = tester.test_initialize().await;
    if matches!(result.status, mcp_tester::report::TestStatus::Passed) {
        initialized = true; break;
    }
    tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
}
assert!(initialized, "server must become ready");
```

**Tempdir + write-fixtures + ephemeral-port idiom (from same file, lines 113-163):**
```rust
// Source: parity_chinook.rs:113-150
fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
// ... tempfile::tempdir() → write config.toml + schema → bind "127.0.0.1:0"
```

> KEY DIFFERENCE from the analog (Pitfall §1): `parity_chinook.rs` calls `run_serving(&args)` **in-process** (no cargo build). TEST-05 must instead (a) `new --kind sql-server` into a tempdir, then (b) shell out to a real `cargo run` subprocess in that tempdir (SC-1's "verified end-to-end" promise), parsing the bound addr from stdout OR using a fixed port + the same poll loop. The unpublished `pmcp-server-toolkit 0.1.0` means the tempdir `Cargo.toml` needs a `[patch.crates-io]` / path override (Pitfall §1, Assumption A1). Use `tester.test_tools_list()` (tester.rs:1343) + `tester.test_tool(name, args)` (tester.rs:1501) for the `tools/list` + `tools/call` assertions. `--test-threads=1` mandatory (Pitfall §5).

---

### `cargo-pmcp/tests/deploy_config_only.rs` (NEW — integration test, gated request-response)

**Analog (gate idiom):** `cargo-pmcp/tests/widgets_orchestrator.rs::npm_skip_gate` (lines 36-45).
**Analog (what to assert ran):** `cargo-pmcp/src/deployment/post_deploy_tests.rs::run_post_deploy_tests` (line 791).

**Env-gate early-return (copy the shape, swap env var to `PMCP_RUN_DEPLOY_TEST`):**
```rust
// Source: cargo-pmcp/tests/widgets_orchestrator.rs:36-45 (npm_skip_gate)
fn deploy_gate() -> Option<&'static str> {
    if std::env::var("PMCP_RUN_DEPLOY_TEST").is_ok() { None }
    else { Some("PMCP_RUN_DEPLOY_TEST not set — skipping real pmcp.run deploy integration test") }
}

#[tokio::test]
async fn config_only_deploy_runs_phase79_lifecycle() {
    if let Some(reason) = deploy_gate() { eprintln!("{reason}"); return; } // SKIP — do NOT fail
    // ... cargo pmcp deploy against gated pmcp.run, then assert run_post_deploy_tests Ok.
}
```

> The gate idiom prints the reason + `return`s (never fails the suite when creds absent) — matches `npm_skip_gate`'s "DO NOT fail the suite" contract (widgets_orchestrator.rs:37-38). D-11 / SC-4: this gated test IS the deliverable; there is no always-on mock. `run_post_deploy_tests` returns `Result<(), OrchestrationFailure>` — assert `Ok` (or that the deploy.rs flow exited 0). The deploy flow itself reuses the existing `execute_async` path (no new deploy code).

---

### `cargo-pmcp/src/commands/new.rs` (MOD — command dispatch, branch)

**Analog:** its own `execute(name, path, tier, global_flags)` (lines 29-92).

**Extend the existing `execute` with a `--kind` branch.** Today `execute` always runs the workspace path (`create_workspace_structure` → `templates::workspace::generate` → `templates::server_common::generate`). Add a `kind: Option<String>` param; when `kind == Some("sql-server")`, branch to a NEW single-crate emitter (`fs::create_dir_all(<name>/src)` then `templates::sql_server::generate(...)`), bypassing the workspace structure entirely. The `None` path stays unchanged (D-01 / Pattern 2).

```rust
// Source: cargo-pmcp/src/commands/new.rs:29-70 (the seam to branch)
pub fn execute(
    name: String,
    path: Option<String>,
    tier: Option<String>,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    // ... existing: determine workspace_dir, bail if exists ...
    // [NEW] if kind == Some("sql-server") { return single_crate_emit(...); }
    create_workspace_structure(&workspace_dir, &name, tier)?;   // existing None path
    templates::workspace::generate(&workspace_dir, &name)?;
    // ...
}
```

> Existing directory guard to reuse verbatim (new.rs:62-64): `if workspace_dir.exists() { anyhow::bail!("Directory '{}' already exists", ...) }`. Next-steps printing pattern (new.rs:160-191 `print_default_next_steps`) — add a `print_sql_server_next_steps` that prints `cd <name> && cargo run` (CONTEXT discretion).

---

### `cargo-pmcp/src/main.rs` (MOD — CLI variant + dispatch)

**Analog:** the `New {}` variant (lines 92-99) for the arg block; the `Dev {}` variant (lines 161-173) for the `#[arg(long)]` optional-field style.

**Add `kind` to the `New {}` variant:**
```rust
// Source: cargo-pmcp/src/main.rs:92-99 (current New variant — add the kind arg)
New {
    name: String,
    #[arg(long)]
    path: Option<String>,
    #[arg(long)]                 // [NEW] — mirror the path arg style
    kind: Option<String>,        // "sql-server"
},
```

**Thread it through dispatch (the single call site):**
```rust
// Source: cargo-pmcp/src/main.rs:519 (dispatch_trait_based — currently passes None for tier)
Commands::New { name, path } => commands::new::execute(name, path, None, global_flags),
// becomes:
Commands::New { name, path, kind } => commands::new::execute(name, path, kind, /* or new param */, None, global_flags),
```

> The current `tier` arg is ALWAYS passed `None` at the call site (main.rs:519) and is unused on the CLI surface — the planner may repurpose the `tier` slot or add `kind` as a distinct param. Keep `#[arg(long)]` (matches `path`/`Dev`'s `server`/`connect`).

---

### `cargo-pmcp/src/templates/mod.rs` (MOD — module index, trivial)

**Analog:** existing `pub mod` lines (1-8). Add `pub mod sql_server;` alphabetically (after `server_common`, before `sqlite_explorer` — or wherever sorted).

```rust
// Source: cargo-pmcp/src/templates/mod.rs:1-8
pub mod calculator;
pub mod complete_calculator;
pub mod mcp_app;
pub mod oauth;
pub mod server;
pub mod server_common;
pub mod sql_server;        // [NEW]
pub mod sqlite_explorer;
pub mod workspace;
```

---

### `cargo-pmcp/src/commands/deploy/mod.rs` (+ `deploy.rs`/`init.rs`) (MOD — build seam, request-response + file-I/O)

**Analog:** the `execute_async` None-arm (lines 880-947) + `bundle_assets_if_configured` (builder.rs:462-561) + `get_target_id` (lines 1017-1030).

**Detection-based, ZERO enum changes (D-09/D-10).** The bundling machinery ALREADY exists — `bundle_assets_if_configured` reads `[assets] include` + special-cases `config.toml`; `get_target_id` selects `pmcp-run` by detection (flag > deploy.toml `target_type` > default). No new `TargetEntry` variant, no new field (Anti-Pattern). The new seam is: detect a config-driven project (config.toml + schema.sql + `pmcp-server-toolkit` dep) and ensure the generated `deploy.toml` carries `[assets] include = ["config.toml", "schema.sql"]` so the existing bundler picks them up.

**The existing build → deploy → verify flow to plug into (UNCHANGED — just confirm single-crate `find_lambda_package_dir` resolves):**
```rust
// Source: cargo-pmcp/src/commands/deploy/mod.rs:901-930
let artifact = target.build(&config).await?;            // BinaryBuilder → cargo lambda build + bundle_assets
let outputs = target.deploy(&config, artifact).await?;
// ... post-deploy verify (Phase 79):
crate::deployment::post_deploy_tests::run_post_deploy_tests(
    url, target_id_str, widgets_present, &pdt_config, quiet,
).await
```

**`get_target_id` detection (D-10 — reuse, do NOT modify the enum):**
```rust
// Source: cargo-pmcp/src/commands/deploy/mod.rs:1017-1030
fn get_target_id(&self, project_root: &PathBuf) -> Result<String> {
    if let Some(target) = &self.target_type { return Ok(target.clone()); }   // --target-type flag
    if let Ok(config) = crate::deployment::DeployConfig::load(project_root) {
        return Ok(config.target.target_type.clone());                        // deploy.toml
    }
    Ok("aws-lambda".to_string())                                             // default
}
```

> `AssetsConfig` (deployment/config.rs:374-427) exposes `include: Vec<String>`, `base_dir: Option<String>`, `has_assets()`, `resolve_files()`. Lambda extracts `assets/<rel>` to `$LAMBDA_TASK_ROOT/assets/` (builder.rs:521); `config.toml` is special-cased separately (builder.rs:516-518). The deployed `config.toml` `file_path` must be `/var/task/assets/<db>` (Pitfall §6 — reference-config.toml:37 already uses this). Open Question §2: verify `find_lambda_package_dir` (builder.rs:312) resolves the single-crate layout — Wave-0 spike.

---

### `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (MOD — new connector helper, batch DDL)

**Analog:** its own `SqliteConnector::open` (lines 72-77) + `execute` (lines 202-228) — same `Arc<Mutex<Connection>>` + `spawn_blocking` shape.

**CONFIRMED ABSENT:** `grep execute_batch|bootstrap` in sqlite.rs returns ZERO matches — the helper is genuinely new (Pitfall §3, Assumption A3). `SqlConnector::execute` is single-statement (prepares ONE statement, lines 220-222), so a multi-statement `schema.sql` cannot run through it. Add a `bootstrap_from_sql` / `execute_batch` so the scaffold `main.rs` bootstrap stays one line (keeps ≤15-line budget).

**Mirror the existing `execute` blocking-closure shape, but call `rusqlite`'s `execute_batch`:**
```rust
// Pattern to mirror — Source: crates/pmcp-server-toolkit/src/sql/sqlite.rs:202-228
async fn execute(&self, sql: &str, params: &[(String, Value)]) -> Result<Vec<Value>, ConnectorError> {
    let conn = Arc::clone(&self.conn);
    let sql = sql.to_string();
    tokio::task::spawn_blocking(move || -> Result<_, ConnectorError> {
        let guard = conn.lock().map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
        // [NEW helper instead] guard.execute_batch(&sql).map_err(|e| ConnectorError::Query(e.to_string()))?;
        // ... existing single-statement prepare/bind/collect ...
    }).await.map_err(|e| ConnectorError::Driver(format!("join error: {e}")))?
}
```

> ALWAYS coverage (CLAUDE.md): the new helper needs a doctest + unit test. Doctest convention is at the top of `sqlite.rs:30-41` (`open_in_memory()` → `execute` → assert). `rusqlite::Connection::execute_batch` runs multiple `;`-separated statements in one call — the clean fix vs the ≤15-line scaffold looping `;`-split (which is fragile against `;` inside seed string literals — keep seed data semicolon-free).

---

### `crates/pmcp-server-toolkit/Cargo.toml` (MOD — `[[example]]` registration)

**Analog:** the two existing `[[example]]` blocks (lines 70-76).

```toml
# Source: crates/pmcp-server-toolkit/Cargo.toml:70-76
[[example]]
name = "e01_toolkit_minimal"
required-features = ["code-mode"]

[[example]]
name = "sqlite_minimal"
required-features = ["sqlite", "code-mode"]

# [NEW] — register the serving Shape C example:
[[example]]
name = "sql_server_http"
required-features = ["sqlite", "code-mode"]
```

> The toolkit already has `sqlite = ["dep:rusqlite", "dep:tokio"]` (line 68) and `code-mode` in `default` (line 58). The new example needs `tokio` `macros` + `rt-multi-thread` for `#[tokio::main]` serving — confirm the `sqlite` feature pulls a sufficient `tokio` feature set (line 68 enables `dep:tokio`; check its feature list covers the multi-thread runtime, or the example's `required-features` / a dev-dep supplies it).

---

## Shared Patterns

### Shared Pattern: the ≤15-line wiring (D-07) — used by BOTH the Shape C example AND the scaffold `main.rs`

**Source:** composed + verified in RESEARCH §1 from `builder_ext.rs::try_*_with_connector` + `sql/sqlite.rs` + `lib.rs::serve`. This is the ONE shape, two emitters (the example file and the emitted `src/main.rs` are byte-identical modulo paths).

```rust
// VERIFIED 14-line body (RESEARCH Code Examples §1). Apply to: sql_server_http.rs + scaffold main.rs.
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
    conn.execute_batch(&std::fs::read_to_string("schema.sql")?).await?;   // NEW helper — 1 line (Pitfall §3)
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

The builder chain is the SAME one `build_server` uses (verified — `assemble.rs:411-415`); the example simplifies the resource surface to `StaticResourceHandler::from(&cfg)` rather than the merged/synthesized handler (acceptable per RESEARCH Pattern 1). Entry points used: `ServerConfig::from_toml_strict_validated` (config.rs:197), `try_tools_from_config_with_connector` (builder_ext.rs:299), `try_code_mode_from_config_with_connector` (builder_ext.rs:344).

### Shared Pattern: generated `config.toml` (D-06) — must parse through `deny_unknown_fields`

**Source:** RESEARCH Code Examples §7, distilled from `reference-config.toml` + `config.rs` field set.
**Apply to:** the scaffold `config.toml` emitter.

```toml
[server]
name = "demo-sql-server"
version = "0.1.0"

[database]
type = "sqlite"
file_path = "demo.db"          # local; deploy overrides to /var/task/assets/demo.db (Pitfall §6)

[code_mode]
enabled = true                 # D-06: headline NL→SQL visible on first run
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

**MANDATORY mechanism (Pitfall §4 / Anti-Pattern):** inline `token_secret` is REJECTED by default (`ConfigValidationError::InlineSecretRejected`). The generated config MUST set `allow_inline_token_secret_for_dev = true` AND a ≥16-byte literal, or `try_code_mode_from_config_with_connector` errors at build. Emit ONLY known fields (config.rs:39-63 enumerates them) — `deny_unknown_fields` turns any typo into a hard parse error.

### Shared Pattern: SQL parameter binding — value `:param` only (never identifiers)

**Source:** `sql/sqlite.rs:143-168` (`bind_params` via `raw_bind_parameter`).
**Apply to:** the scaffold's curated `[[tools]]` SQL.

Curated `[[tools]]` use named `:value` params bound via `raw_bind_parameter` (never string-concatenated — T-84-04-01). Phase 84 confirmed this works for the curated tools (RESEARCH Open Question §1). Do NOT use `:table` identifier substitution (deferred, out of scope).

### Shared Pattern: quiet-output guard (cargo-pmcp CLI convention)

**Source:** `new.rs:117` / `workspace.rs:15`.
**Apply to:** any new emitter / command output.

```rust
if std::env::var("PMCP_QUIET").is_err() { println!("  {} ...", "✓".green()); }
// or via global_flags.should_output() (new.rs:35)
```

---

## No Analog Found

None. Every Phase 86 file has a strong in-repo analog (RESEARCH confidence HIGH; all primitives verified working).

| File | Note |
|------|------|
| — | All 11 files map to verified analogs. The only genuinely-new *logic* is the `execute_batch`/`bootstrap_from_sql` helper (no existing batch executor — confirmed by grep) and the deploy detection seam (an additive branch in an existing function, not a new subsystem). |

---

## Metadata

**Analog search scope:**
- `cargo-pmcp/src/templates/` (workspace.rs, mod.rs, sqlite_explorer ref), `cargo-pmcp/src/commands/{new.rs, deploy/}`, `cargo-pmcp/src/deployment/builder.rs`, `cargo-pmcp/src/main.rs`, `cargo-pmcp/tests/widgets_orchestrator.rs`
- `crates/pmcp-sql-server/src/{lib.rs, main.rs, assemble.rs}`, `crates/pmcp-sql-server/tests/parity_chinook.rs`
- `crates/pmcp-server-toolkit/src/{builder_ext.rs, sql/sqlite.rs, config.rs}`, `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs`, `crates/pmcp-server-toolkit/Cargo.toml`

**Files scanned:** ~14 (all read directly or grep-verified this session).
**Pattern extraction date:** 2026-05-26
