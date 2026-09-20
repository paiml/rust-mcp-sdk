# Phase 95: Shape A Binary `pmcp-workbook-server` - Pattern Map

**Mapped:** 2026-06-14
**Files analyzed:** 8 new files (1 crate)
**Analogs found:** 8 / 8 (every new file has an exact in-repo analog under `crates/pmcp-sql-server/`)

> This phase is a **field-for-field re-skin of `crates/pmcp-sql-server/`**. Every
> new file copies its analog almost verbatim; the ONE seam that genuinely changes
> is the connector-dispatch (`dispatch.rs` → `Arc<dyn SqlConnector>`) being
> replaced by a `LocalDirSource` + `--bundle-id` assertion seam. The executor's
> job is mechanical mirroring — read each analog, copy structure, swap the seam.

---

## File Classification

| New File (`crates/pmcp-workbook-server/`) | Role | Data Flow | Closest Analog | Match Quality |
|-------------------------------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | n/a | `crates/pmcp-sql-server/Cargo.toml` | exact (metadata/exclude/docs.rs) — feature set + deps DIFFER |
| `src/lib.rs` | library (entry) | request-response | `crates/pmcp-sql-server/src/lib.rs` | exact (`run`/`serve`/`run_serving` + `RunError`) |
| `src/cli.rs` | config (clap `Args`) | request-response | `crates/pmcp-sql-server/src/cli.rs` | exact — flag SET differs (`--bundle-dir`/`--bundle-id` vs `--config`/`--schema`) |
| `src/main.rs` | binary shim | request-response | `crates/pmcp-sql-server/src/main.rs` | exact (verbatim, rename only) |
| `src/assemble.rs` | service (builder seam) | transform | `crates/pmcp-sql-server/src/assemble.rs` | role-match — body is the NOVEL seam; only the `RunError`/error-wrap shape is copied |
| `src/source.rs` (the NEW seam, replaces `dispatch.rs`) | service (source selection) | transform | `crates/pmcp-sql-server/src/dispatch.rs` | role-match — `dispatch.rs` is the structural template for the error enum + `Args` → typed-source switch |
| `tests/assemble.rs` + `tests/http_smoke.rs` | test | request-response | `crates/pmcp-sql-server/tests/assemble.rs`, `tests/http_lazy_startup.rs` | exact |
| `tests/parity_workbook.rs` | test | request-response | `crates/pmcp-sql-server/tests/parity_chinook.rs` | exact (mcp-tester ephemeral-port replay) |

**Note on the `source`/`assemble` split:** `pmcp-sql-server` splits the novel seam
into `dispatch.rs` (select the connector) + `assemble.rs` (config+connector+schema →
`pmcp::Server`). For the workbook binary the two collapse heavily: the toolkit's
`WorkbookBuilderExt::try_with_workbook_bundle(&dyn BundleSource)` does BOTH the
fail-closed load AND tool registration in one call. CONTEXT.md's "Claude's
Discretion" lists an `assemble`-equivalent seam; the planner may keep `source.rs`
(build `LocalDirSource` + assert `--bundle-id`) and `assemble.rs` (call
`try_with_workbook_bundle`) separate, OR fold both into `assemble.rs`. Either way
the **error-enum + crash-surfacing template** below is what gets mirrored.

---

## Pattern Assignments

### `src/lib.rs` (library entry, request-response)

**Analog:** `crates/pmcp-sql-server/src/lib.rs` — copy its STRUCTURE verbatim.

The whole `run` / `serve` / `run_serving` / `load_*` split + `RunError` enum +
the `handle.await.map_err(RunError::Serving)` crash-surfacing pattern (threat
T-85-10-02) transfers 1:1. Only the load + assemble seam swaps.

**Imports block** (`lib.rs:42-49`) — copy verbatim, drop `Arc`/`ServerConfig` if unused:
```rust
use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
```
(Workbook binary adds: `use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};` — these are re-exported through the TOOLKIT per D-11; NEVER `use pmcp_workbook_runtime::...`.)

**`RunError` enum** (`lib.rs:51-97`) — mirror the variant SHAPE; swap the middle variants per CONTEXT "Claude's Discretion":
```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    #[error("failed to read {what} file: {source}")]
    Io { what: &'static str, source: std::io::Error },

    // SQL had: Config / Dispatch / Assemble.
    // Workbook keeps an Assemble-equivalent wrapping the toolkit error from
    // try_with_workbook_bundle (which wraps BundleLoadError fail-closed), PLUS a
    // NEW id-mismatch variant for the --bundle-id assertion (D-01):
    #[error("workbook bundle load/integrity failed: {0}")]
    Bundle(#[from] pmcp_server_toolkit::ToolkitError),   // wraps BundleLoadError

    #[error("bundle id mismatch: --bundle-id '{expected}' but loaded bundle is '{actual}'")]
    BundleIdMismatch { expected: String, actual: String },  // D-01 fail-closed guard

    #[error("invalid --http bind address '{addr}': {source}")]
    Addr { addr: String, source: std::net::AddrParseError },

    #[error("streamable-HTTP server failed to start: {0}")]
    Serve(#[source] pmcp::Error),

    // COPY VERBATIM — the crash-surfacing variant (T-85-10-02):
    #[error("streamable-HTTP serving task failed: {0}")]
    Serving(#[source] tokio::task::JoinError),
}
```

**`serve()` — COPY VERBATIM** (`lib.rs:156-164`). This is transport-only and
identical for both binaries (same Tower/axum `StreamableHttpServer` adapter, D-04):
```rust
pub async fn serve(server: Server, addr: SocketAddr) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let shared = Arc::new(Mutex::new(server));
    let http = StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    http.start().await.map_err(RunError::Serve)
}
```

**`run_serving()`** (`lib.rs:200-211`) — mirror; swap the SQL load+dispatch+build
lines for the workbook load+assert+assemble seam:
```rust
pub async fn run_serving(args: &Args) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    // SQL was: let (cfg, schema_ddl) = load_config_and_schema(args)?;
    //          let connector = dispatch(&cfg).await?;
    //          let server = build_server(&cfg, connector, schema_ddl)?;
    // Workbook: build LocalDirSource from --bundle-dir, assert --bundle-id, assemble:
    let server = build_server(args)?;   // assemble.rs seam (see below)

    let addr: SocketAddr = args.http.parse().map_err(|source| RunError::Addr {
        addr: args.http.clone(),
        source,
    })?;
    serve(server, addr).await
}
```

**`run()` — COPY VERBATIM** (`lib.rs:245-265`), retarget the tracing string. The
critical line to preserve EXACTLY (crash-surfacing, T-85-10-02):
```rust
let (bound, handle) = run_serving(&args).await?;
tracing::info!(target: "pmcp_workbook_server", %bound, "streamable-HTTP server listening");
handle.await.map_err(RunError::Serving)?;  // a crashed listener exits non-zero
Ok(())
```

**Inline unit tests** (`lib.rs:267-328`) — copy the two crash-surfacing tests
verbatim (`serving_task_panic_maps_to_run_error_serving`,
`run_error_serving_display_is_descriptive`); they assert `RunError::Serving`
behaviour and are binary-agnostic.

---

### `src/cli.rs` (clap `Args`, request-response)

**Analog:** `crates/pmcp-sql-server/src/cli.rs` — copy structure; swap the flag set.

SQL has two required path args (`--config`, `--schema`) + `--http` loopback
default. Workbook has ONE required path (`--bundle-dir`) + an OPTIONAL
`--bundle-id` assertion + the SAME `--http` default (D-01/D-03/D-04).

**`Args` struct** (`cli.rs:32-52`) — mirror, swap fields:
```rust
#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "pmcp-workbook-server",
    version,
    about = "Shape A pure-config workbook MCP server — point it at a compiled bundle dir and serve five workbook tools with no Rust required"
)]
pub struct Args {
    /// Path to the EXACT compiled `bundle@version` directory (D-01: the version
    /// is implicit in the path; handed straight to LocalDirSource::new). Required.
    #[arg(long)]
    pub bundle_dir: PathBuf,

    /// Optional assertion: the loaded BUNDLE.lock bundle_id MUST equal this, else
    /// the binary fails closed (D-01). A guard, not a resolution input.
    #[arg(long)]
    pub bundle_id: Option<String>,

    /// Bind address for the streamable-HTTP transport (host:port). COPY VERBATIM.
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
}
```

**Inline tests** (`cli.rs:54-98`) — mirror the four: required-path-parses-with-
default-http, http-override, missing-required-is-usage-error. Add one for
`--bundle-id` being optional (absent → `None`).

---

### `src/main.rs` (binary shim) — COPY VERBATIM (rename only)

**Analog:** `crates/pmcp-sql-server/src/main.rs` (whole file, 14 lines):
```rust
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = pmcp_workbook_server::Args::parse();
    pmcp_workbook_server::run(args).await?;
    Ok(())
}
```
Only the crate name (`pmcp_sql_server` → `pmcp_workbook_server`) changes. No
business logic, no `process::exit` — the non-zero exit comes from returning
`run()`'s `RunError`.

---

### `src/source.rs` + `src/assemble.rs` (the NOVEL seam — replaces `dispatch.rs`)

**Analog (template, NOT content):** `crates/pmcp-sql-server/src/dispatch.rs` for
the error-enum + module-doc shape; `crates/pmcp-sql-server/src/assemble.rs:401-423`
(`build_server`) for the "config → built `pmcp::Server`" function shape and the
`AssembleError` (thiserror, `#[non_exhaustive]`, `#[from]`) pattern.

**The workbook seam is FAR SIMPLER than SQL's** — the toolkit's
`WorkbookBuilderExt::try_with_workbook_bundle` does the fail-closed load + all
five tool registrations + the `workbook://` resource in ONE call. No connector
dispatch, no resource/prompt merge. The whole `build_server` body is essentially:

```rust
use pmcp::Server;
// D-11: import the bundle surface ONLY from the toolkit, NEVER pmcp_workbook_runtime.
use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};

pub fn build_server(args: &Args) -> Result<Server, RunError> {
    // 1. One source instance = one bundle@version; version implicit in the path (D-01).
    let source = LocalDirSource::new(&args.bundle_dir);

    // 2. Fail-closed load + register all 5 tools + workbook:// resource (one call).
    //    try_with_workbook_bundle wraps BundleLoadError into ToolkitError → RunError::Bundle.
    let builder = Server::builder()
        .name("pmcp-workbook-server")
        .version(env!("CARGO_PKG_VERSION"))
        .try_with_workbook_bundle(&source)?;   // #[from] ToolkitError → RunError::Bundle

    let server = builder.build().map_err(RunError::Serve /* or an Assemble variant */)?;
    Ok(server)
}
```

**The `--bundle-id` assertion (D-01, the genuinely-new logic).** The toolkit's
one-call `try_with_workbook_bundle` consumes the bundle internally, so the
assertion needs the loaded `bundle_id`. Two viable shapes for the planner:

- **(a) Pre-load + assert, then assemble** — call the re-exported
  `pmcp_server_toolkit::workbook::load_bundle(&source)?` FIRST (the same
  fail-closed gate, `bundle_loader.rs:268` `pub fn load(source: &dyn BundleSource)`),
  read `bundle.stamp.bundle_id` (`WorkbookBundle.stamp: BundleLock`,
  `bundle_loader.rs:84,98`; the field is `lock.bundle_id`), compare to
  `args.bundle_id`, return `RunError::BundleIdMismatch` on mismatch, THEN call
  `try_with_workbook_bundle`. (Note: double-loads the bundle — acceptable; or)
- **(b)** assert against the served `ProvStamp.bundle_id` after assembly.

Recommend **(a)** — it fails closed BEFORE registering any tool, matching the
`dispatch.rs` "never serve the wrong backend" discipline. The bundle-id is the
operator-typed identifier (like SQL's `[database] type` string), NOT a secret, so
echoing both `expected`/`actual` in the error is safe (mirror `DispatchError`'s
`Display`-is-credential-free posture, `dispatch.rs:42-49`).

**`AssembleError` / error-wrap shape to mirror** (`assemble.rs:71-83`):
```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssembleError {
    #[error("toolkit assembly step failed: {0}")]
    Toolkit(#[from] pmcp_server_toolkit::ToolkitError),
    #[error("pmcp server build failed: {0}")]
    Build(#[from] pmcp::Error),
}
```
The planner may keep `AssembleError` separate (mirroring SQL) or fold straight
into `RunError::Bundle` + `RunError::Serve` — discretion per CONTEXT.

**Key toolkit-surface facts (from `crates/pmcp-server-toolkit/src/workbook/mod.rs`):**
- `try_with_workbook_bundle(self, source: &dyn BundleSource) -> Result<Self>` is on
  trait `WorkbookBuilderExt for ServerBuilder` (`workbook/mod.rs:217-274`); it
  loads via `load_bundle(source)?` then `tool_arc(...)` ×5 + `resources_arc(...)`.
- It is re-exported at the toolkit CRATE ROOT too (`lib.rs:157-160`), so
  `use pmcp_server_toolkit::WorkbookBuilderExt;` also works — but the
  `workbook::` module path is the documented one.
- `LocalDirSource::new(path: impl Into<PathBuf>)` lives in
  `crates/pmcp-workbook-runtime/src/bundle_source.rs` (~line 117) and is
  re-exported through `pmcp_server_toolkit::workbook` (`workbook/mod.rs:71-73`).

---

### `tests/assemble.rs` + `tests/http_smoke.rs` (integration tests)

**Analogs:** `crates/pmcp-sql-server/tests/assemble.rs` (server-surface assertions)
and `crates/pmcp-sql-server/tests/http_lazy_startup.rs` (ephemeral-port HTTP smoke).

**Server-surface test pattern** (`tests/assemble.rs:59-75`) — mirror, assert the
FIVE workbook tools instead of `search_tracks`/`validate_code`:
```rust
let server = build_reference_server().await; // builds from the golden bundle dir
assert!(server.get_tool("calculate").is_some());
assert!(server.get_tool("explain").is_some());
assert!(server.get_tool("get_manifest").is_some());
assert!(server.get_tool("diff_version").is_some());
assert!(server.get_tool("render_workbook").is_some());
// and the workbook:// resource is registered
```

**HTTP-smoke pattern — COPY the shape** (`tests/http_lazy_startup.rs:86-142`):
bind `127.0.0.1:0`, capture the REAL bound addr via `serve`, drive an MCP
`initialize` over the SDK `StreamableHttpTransport`, assert the response id
echoes, `handle.abort()`. The transport-config struct
(`StreamableHttpTransportConfig { url, extra_headers, auth_provider, session_id,
enable_json_response, on_resumption_token, http_middleware_chain }`,
`http_lazy_startup.rs:105-113`) transfers verbatim.

**Fixture path pattern** — the golden bundle dir is read by absolute path from
`CARGO_MANIFEST_DIR` (mirror `chinook_db_path()`, `tests/assemble.rs:35-37`):
```rust
fn golden_bundle_dir() -> std::path::PathBuf {
    // reuse the EXISTING committed golden (CONTEXT "Claude's Discretion"):
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}
```
> The committed synthetic golden lives at
> `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` (members: `BUNDLE.lock`,
> `cell_map.json`, `executable.ir.json`, `layout.json`, `manifest.json`,
> `evidence/`). `bundle_id = "tax-calc"`, `version = "1.1.0"`. This is the ONLY
> committed workbook bundle in the repo — reuse it, do NOT regenerate (D-05;
> zero-customer-material hard constraint). Cross-crate `include_str!`/path
> reference is already precedented (`http_lazy_startup.rs:35-37` reads
> `../../pmcp-server-toolkit/tests/fixtures/...`).

---

### `tests/parity_workbook.rs` (mcp-tester ephemeral-port replay)

**Analog:** `crates/pmcp-sql-server/tests/parity_chinook.rs` — the binding
end-to-end parity test through the REAL binary path.

Mirror the FIVE-step shape (`parity_chinook.rs:133-267`):
1. point at the committed golden bundle dir (no temp DB copy needed — the bundle
   is read-only and already committed),
2. construct a programmatic `Args { bundle_dir, bundle_id: None, http: "127.0.0.1:0".into() }`,
3. invoke the REAL `pmcp_workbook_server::run_serving(&args).await` (the exact
   `cargo run -- --bundle-dir X` path),
4. poll `ServerTester::test_initialize()` with backoff for readiness
   (`parity_chinook.rs:174-194`), construct `ServerTester::new(&url, 30s, false,
   None, Some("http"), None)`,
5. drive `calculate`/`explain`/`get_manifest` via mcp-tester (either a committed
   `.yaml` scenario á la `generated.yaml`, or direct tool calls) and assert
   success; `handle.abort()` at the end.

The mcp-tester imports transfer verbatim:
`use mcp_tester::{ScenarioExecutor, ServerTester, TestScenario};`
(`parity_chinook.rs:61`). `dev-dependencies` `mcp-tester` is the parity harness
ONLY (not a published dep) — mirror `Cargo.toml:57`.

---

### `Cargo.toml` (crate metadata + deps + features)

**Analog:** `crates/pmcp-sql-server/Cargo.toml` — copy the metadata/`exclude`/
`docs.rs`/`[lib]`+`[[bin]]` blocks; the `[dependencies]` and `[features]` DIFFER.

**COPY VERBATIM (metadata shape):**
```toml
[package]
name = "pmcp-workbook-server"
version = "0.1.0"            # NEW crate — start at 0.1.0 (CONTEXT)
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Shape A pure-config workbook MCP server binary — point it at a compiled bundle dir and serve five workbook tools with no Rust required"
keywords = ["mcp", "workbook", "config-driven", "toolkit", "excel"]
categories = ["development-tools", "command-line-utilities"]

# Mirror sql-server's exclude (CONTEXT explicitly names this):
exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
name = "pmcp_workbook_server"
path = "src/lib.rs"

[[bin]]
name = "pmcp-workbook-server"
path = "src/main.rs"
```

**DEPENDENCIES — the key delta (D-02 / D-11).** Link ONLY `pmcp` +
`pmcp-server-toolkit[workbook + http]`; do NOT name `pmcp-workbook-runtime`
(its `LocalDirSource`/`load_bundle`/`BundleSource` are re-exported through the
toolkit — success-criterion-3 purity):
```toml
[dependencies]
pmcp = { version = "2.9.0", path = "../..", features = ["streamable-http"] }
pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit", features = ["workbook", "http"] }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }
thiserror = "2"
```
> `workbook` = `["dep:pmcp-workbook-runtime", "dep:base64"]` (LocalDir-only, no
> `include_dir` — D-02; NOT `workbook-embedded`).
> `http` forwards `pmcp/streamable-http` (toolkit `Cargo.toml:117-127`). Both come
> from the toolkit, so the binary's `[dependencies]` names only the toolkit.

**FEATURES — much simpler than SQL** (no per-backend matrix; D-02 is LocalDir-only).
The SQL `[features]` `default = ["sqlite","postgres","mysql","athena"]` block is
DROPPED entirely. The workbook binary has no optional-backend features (the single
`workbook` posture is fixed). Omit `[features]` or keep an empty default.

**DEV-DEPENDENCIES — mirror the parity-harness shape** (`Cargo.toml:56-73`),
trimmed to what the workbook tests use:
```toml
[dev-dependencies]
mcp-tester = { version = "0.7.0", path = "../mcp-tester" }   # parity replay harness
tempfile = "3"
serde_json = "1"
url = "2.5"   # StreamableHttpTransportConfig.url is a url::Url (http smoke test)
# (drop rusqlite/serde_yaml/proptest unless a workbook test needs them)
```

---

## Shared Patterns

### Crash-surfacing (threat T-85-10-02) — applies to `lib.rs`
**Source:** `crates/pmcp-sql-server/src/lib.rs:89-96` (`Serving` variant) + `:263`
(`handle.await.map_err(RunError::Serving)?`). A discarded `JoinError` would let a
crashed listener look healthy. COPY this exactly — it is binary-agnostic and is
the whole reason for the `run`/`run_serving` split.

### Fail-closed boot integrity — applies to `assemble.rs`/`source.rs`
**Source:** `crates/pmcp-server-toolkit/src/workbook/mod.rs:226-230` →
`pmcp_workbook_runtime::load_bundle` (`bundle_loader.rs:268`, `pub fn load`). The
toolkit's `try_with_workbook_bundle` loads + integrity-verifies (BUNDLE.lock hash
recomputation) BEFORE registering any tool; a tampered/incomplete bundle returns
`Err` (→ `RunError::Bundle`), never a partial server. The binary just maps the
error — it adds NO served logic (CONTEXT code_context).

### Credential-/secret-free error `Display` — applies to `RunError`/`source.rs`
**Source:** `crates/pmcp-sql-server/src/dispatch.rs:42-49` (`DispatchError` names
backend/feature only, never paths/credentials). For the workbook binary: the
`BundleIdMismatch` variant echoes only operator-typed `bundle_id` strings (not
secrets); the `Bundle` variant forwards the toolkit/loader error, which is already
path-careful. Do NOT echo `--bundle-dir` raw filesystem paths into MCP-client-
visible errors (log via tracing at the binary's discretion).

### Tower/axum streamable-HTTP serve — applies to `serve()`
**Source:** `crates/pmcp-sql-server/src/lib.rs:156-164`. The SDK's Phase 56
`StreamableHttpServer::with_config(addr, Arc<Mutex<Server>>, StreamableHttpServerConfig::default())`
applies DNS-rebinding/CORS/security-header layers — NEVER hand-rolled (D-04 /
threat T-85-05-01). Default config is stateful with `AllowedOrigins::localhost()`
matching the `127.0.0.1` loopback default. Identical for both binaries.

### Ephemeral-port + `abort()` integration-test discipline — applies to all tests
**Source:** `crates/pmcp-sql-server/tests/http_lazy_startup.rs:99-142` and
`tests/parity_chinook.rs:159-266`. Bind `127.0.0.1:0`, capture the REAL bound addr
from `serve`/`run_serving`, drive the live server, `handle.abort()`. Run
`--test-threads=1` (ephemeral port + per-process env). Same shape for workbook.

---

## No Analog Found

None. Every new file maps to an exact in-repo analog. The single piece of
genuinely-NEW logic — the `--bundle-id` assertion (D-01) — is small (read
`bundle.stamp.bundle_id` from the re-exported `load_bundle`, compare, return
`RunError::BundleIdMismatch`) and uses the existing `dispatch.rs` error-enum
template + the existing toolkit `load_bundle` re-export. No RESEARCH.md fallback
needed.

---

## Workbook-Specific Deltas (vs the SQL analog) — quick reference for the planner

| Concern | SQL (`pmcp-sql-server`) | Workbook (`pmcp-workbook-server`) |
|---------|--------------------------|-----------------------------------|
| CLI inputs | `--config` + `--schema` (both required paths) | `--bundle-dir` (required) + `--bundle-id` (optional assertion, D-01) + `--http` |
| The novel seam | `dispatch.rs`: `[database] type` → `Arc<dyn SqlConnector>` | `source.rs`/`assemble.rs`: `--bundle-dir` → `LocalDirSource` + `--bundle-id` assert |
| Assembly call | `build_server(cfg, connector, schema)` (merge resources/prompts/code-mode) | `Server::builder().try_with_workbook_bundle(&source)` (one call, 5 tools + resource) |
| Tools served | curated `[[tools]]` + `validate_code`/`execute_code` | fixed 5: `calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook` |
| Boot integrity | config parse/validate | fail-closed `load_bundle` (BUNDLE.lock hash recompute) — inside the toolkit call |
| Deps | toolkit `[code-mode, sqlite]` + 3 connector crates | toolkit `[workbook, http]` ONLY; NEVER `pmcp-workbook-runtime` directly (D-11) |
| Features | 4-backend matrix (`sqlite`/`postgres`/`mysql`/`athena`) | none (single fixed `workbook` posture, D-02) |
| Test fixture | committed `chinook.db` (data-bearing) | committed `tax-calc@1.1.0` golden bundle dir (reuse, do NOT regenerate; D-05) |
| `RunError` variants | Io/Config/Dispatch/Assemble/Addr/Serve/Serving | Io/Bundle/BundleIdMismatch/Addr/Serve/Serving |

**Publish-slot wiring (CONTEXT discretion):** add `pmcp-workbook-server` to
`CLAUDE.md` ## Release & Publish Workflow as "slot 9a" — AFTER
`pmcp-server-toolkit` (item 5) and `pmcp-workbook-runtime` (its transitive dep).
Served cone for the purity gate: binary → toolkit[workbook] → pmcp-workbook-runtime
→ pmcp (must stay reader-free: no `umya`/`quick-xml`/`zip`).

---

## Metadata

**Analog search scope:** `crates/pmcp-sql-server/{src,tests}/`,
`crates/pmcp-server-toolkit/src/{lib.rs,workbook/mod.rs}`,
`crates/pmcp-server-toolkit/Cargo.toml`,
`crates/pmcp-workbook-runtime/src/{bundle_source.rs,bundle_loader.rs}`,
`crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/`.
**Files scanned:** 11 (8 analogs + 3 toolkit/runtime surface files).
**Pattern extraction date:** 2026-06-14.
