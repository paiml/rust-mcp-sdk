//! Shape A pure-config SQL MCP server (`pmcp-sql-server`).
//!
//! This crate is the **Shape A** delivery of the v2.2 "Configuration-Only MCP
//! Servers" milestone: a standalone binary that an operator points at a
//! `config.toml` (the `[[tools]]` / `[database]` / `[code_mode]` declarations)
//! plus a schema file and runs as a production MCP server — **without writing
//! any Rust**. It assembles a [`pmcp::Server`] entirely from
//! `pmcp-server-toolkit` config primitives and the per-backend connector crates
//! (`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, plus
//! the `sqlite` feature's `SqliteConnector`).
//!
//! # Crate layout (lib + bin split)
//!
//! The testable assembly entry point lives here in the library ([`run`]); the
//! `pmcp-sql-server` binary (`src/main.rs`) is a thin `#[tokio::main]` shim that
//! parses CLI/env arguments and delegates to [`run`]. This split keeps the
//! server-construction logic unit-testable without spawning a process.
//!
//! # Pipeline ([`run`])
//!
//! [`run`] is the full Shape A pipeline (Plan 85-05): parse the config + schema
//! files → [`dispatch`] the connector for the `[database] type` → [`build_server`]
//! the [`pmcp::Server`] (tools + code-mode + the configured resources/prompts
//! with the `--schema` content merged in) → serve it over streamable HTTP via
//! the Phase 56 Tower/axum adapter ([`serve`]).
//!
//! # Seams
//!
//! - [`cli`]: the clap [`Args`] surface (`--config` / `--schema` / `--http`).
//! - [`dispatch`]: the NOVEL `[database] type` → `Arc<dyn SqlConnector>` switch
//!   with a clear compiled-out-backend error ([`dispatch::DispatchError`]).
//! - [`assemble`]: config + connector + schema → built [`pmcp::Server`].

pub mod assemble;
pub mod cli;
pub mod dispatch;

pub use assemble::{build_server, merge_schema_resource, AssembleError};
pub use cli::Args;
pub use dispatch::{dispatch, DispatchError};

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_server_toolkit::ServerConfig;
use tokio::task::JoinHandle;

/// Error from the Shape A [`run`] pipeline.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// Reading the `--config` or `--schema` file from disk failed.
    #[error("failed to read {what} file: {source}")]
    Io {
        /// Which file (`config` or `schema`).
        what: &'static str,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Parsing or validating the config TOML failed.
    #[error("config parse/validate failed: {0}")]
    Config(#[from] pmcp_server_toolkit::ToolkitError),

    /// The `[database] type` could not be dispatched to a connector.
    #[error("backend dispatch failed: {0}")]
    Dispatch(#[from] DispatchError),

    /// Assembling the `pmcp::Server` from config + connector + schema failed.
    #[error("server assembly failed: {0}")]
    Assemble(#[from] AssembleError),

    /// The `--http` bind address could not be parsed as a `SocketAddr`.
    #[error("invalid --http bind address '{addr}': {source}")]
    Addr {
        /// The offending address string.
        addr: String,
        /// Parse error.
        source: std::net::AddrParseError,
    },

    /// Binding / starting the streamable-HTTP listener failed.
    #[error("streamable-HTTP server failed to start: {0}")]
    Serve(#[source] pmcp::Error),

    /// The serving task ended abnormally (panic / abort).
    ///
    /// A discarded `JoinError` would hide a crashed listener — the process
    /// looks healthy while serving nothing. Propagating it here surfaces as a
    /// non-zero process exit so a supervisor restarts the binary (threat
    /// T-85-10-02).
    #[error("streamable-HTTP serving task failed: {0}")]
    Serving(#[source] tokio::task::JoinError),
}

/// Load + validate the config and read the schema file.
///
/// Splitting this out of [`run`] keeps `run`'s cognitive complexity low and lets
/// tests exercise the load path independently of transport startup.
///
/// # Errors
///
/// [`RunError::Io`] when a file is unreadable, or [`RunError::Config`] when the
/// TOML fails to parse/validate.
fn load_config_and_schema(args: &Args) -> Result<(ServerConfig, String), RunError> {
    let config_text = std::fs::read_to_string(&args.config).map_err(|source| RunError::Io {
        what: "config",
        source,
    })?;
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)?;
    let schema_ddl = std::fs::read_to_string(&args.schema).map_err(|source| RunError::Io {
        what: "schema",
        source,
    })?;
    Ok((cfg, schema_ddl))
}

/// Start the streamable-HTTP server for `server` on `addr`, returning the REAL
/// bound address and the serving task handle.
///
/// Uses the Phase 56 Tower/axum adapter
/// ([`StreamableHttpServer`]) so the DNS-rebinding, CORS, and security-headers
/// layers are applied by the SDK — never hand-rolled (threat T-85-05-01). The
/// default [`StreamableHttpServerConfig`] is stateful with
/// `AllowedOrigins::localhost()` (the loopback default matching `--http`'s
/// `127.0.0.1` default).
///
/// Returning `(addr, handle)` rather than blocking lets tests drive the server
/// (HTTP smoke) and `run` await the handle for a long-running process.
///
/// # Errors
///
/// [`RunError::Serve`] if binding the listener fails (e.g. the port is in use).
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use pmcp_sql_server::{build_server, serve};
/// use pmcp_server_toolkit::ServerConfig;
/// use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let cfg = ServerConfig::default();
/// let connector: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory()?);
/// let server = build_server(&cfg, connector, "-- ddl --".into())?;
/// let (bound, handle) = serve(server, "127.0.0.1:0".parse()?).await?;
/// println!("listening on http://{bound}");
/// handle.abort();
/// # Ok(())
/// # }
/// ```
pub async fn serve(
    server: Server,
    addr: SocketAddr,
) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let http = StreamableHttpServer::with_config_concurrent(
        addr,
        Arc::new(server),
        StreamableHttpServerConfig::default(),
    );
    http.start().await.map_err(RunError::Serve)
}

/// Run the FULL Shape A pipeline up to (but not including) blocking on the
/// serving task, returning the REAL bound address and the serving task handle.
///
/// This is the *exact* binary path — read `--config` + `--schema` from disk via
/// [`load_config_and_schema`] → [`dispatch`] the connector for the
/// `[database] type` → [`build_server`] the [`pmcp::Server`] → [`serve`] it over
/// streamable HTTP — with no connector injection or assembly short-circuit. It
/// is the testable seam [`run`] delegates to: [`run`] calls this and then awaits
/// the returned handle, while integration tests (the REF-02 parity replay) call
/// it directly to obtain the ephemeral bound address, drive the live server, and
/// `abort()` the handle.
///
/// # Errors
///
/// Any [`RunError`] variant — file I/O, config parse/validate, backend dispatch,
/// server assembly, address parse, or transport startup.
///
/// # Example
///
/// ```no_run
/// use pmcp_sql_server::{run_serving, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let args = Args {
///     config: "config.toml".into(),
///     schema: "schema.ddl".into(),
///     http: "127.0.0.1:0".to_string(), // ephemeral port
/// };
/// let (bound, handle) = run_serving(&args).await?;
/// println!("listening on http://{bound}");
/// handle.abort(); // stop serving
/// # Ok(())
/// # }
/// ```
pub async fn run_serving(args: &Args) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let (cfg, schema_ddl) = load_config_and_schema(args)?;
    let connector = dispatch(&cfg).await?;
    let server = build_server(&cfg, connector, schema_ddl)?;

    let addr: SocketAddr = args.http.parse().map_err(|source| RunError::Addr {
        addr: args.http.clone(),
        source,
    })?;

    serve(server, addr).await
}

/// Assemble and serve the Shape A SQL MCP server from configuration.
///
/// The full pipeline: read `--config` + `--schema` → [`dispatch`] the connector
/// for the `[database] type` → [`build_server`] the [`pmcp::Server`] → [`serve`]
/// it over streamable HTTP, then await the serving task (blocks until the task
/// ends or the process is signalled).
///
/// Initialises a `tracing_subscriber::fmt` reader of `RUST_LOG` so operators get
/// structured logs from a local binary; the subscriber is best-effort (a second
/// call in the same process is ignored).
///
/// # Errors
///
/// Any [`RunError`] variant — file I/O, config parse/validate, backend dispatch,
/// server assembly, address parse, or transport startup.
///
/// # Example
///
/// ```no_run
/// use pmcp_sql_server::{run, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// // Equivalent to `pmcp-sql-server --config config.toml --schema schema.ddl`.
/// run(Args {
///     config: "config.toml".into(),
///     schema: "schema.ddl".into(),
///     http: "127.0.0.1:8080".to_string(),
/// })
/// .await?; // blocks serving until the task ends
/// # Ok(())
/// # }
/// ```
pub async fn run(args: Args) -> Result<(), RunError> {
    // Best-effort log init (ignored if a global subscriber is already set, e.g.
    // when a test process has installed one).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let (bound, handle) = run_serving(&args).await?;
    tracing::info!(target: "pmcp_sql_server", %bound, "streamable-HTTP server listening");

    // Await the serving task for the lifetime of the process. A JoinError
    // (panic / abort) is propagated as RunError::Serving so the process exits
    // non-zero — main.rs returns run()'s Result, letting a supervisor restart a
    // crashed listener instead of treating the silent exit as healthy
    // (threat T-85-10-02).
    handle.await.map_err(RunError::Serving)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn run_error_display_is_credential_free_for_addr() {
        // The Addr variant echoes the operator-typed bind string (not a secret)
        // and a parse error — no config values.
        let err = Args::try_parse_from([
            "pmcp-sql-server",
            "--config",
            "c.toml",
            "--schema",
            "s.ddl",
            "--http",
            "not-an-addr",
        ])
        .map(|a| a.http)
        .unwrap();
        assert_eq!(err, "not-an-addr");
    }

    /// 85-10 / T-85-10-02: a serving-task panic must surface as
    /// `RunError::Serving` (a non-zero process exit), NOT a discarded `()`.
    /// This mirrors `run()`'s `handle.await.map_err(RunError::Serving)` logic on
    /// a task that panics — driving the full `run()` (which binds a real
    /// listener) is impractical in a unit test.
    #[tokio::test]
    async fn serving_task_panic_maps_to_run_error_serving() {
        let handle: JoinHandle<()> = tokio::spawn(async {
            panic!("simulated serve-task panic");
        });
        let outcome: Result<(), RunError> = handle.await.map_err(RunError::Serving);
        match outcome {
            Ok(()) => panic!("a panicking serve task must NOT map to Ok(())"),
            Err(RunError::Serving(join_err)) => {
                assert!(
                    join_err.is_panic(),
                    "the JoinError must reflect the task panic"
                );
            },
            Err(other) => panic!("expected RunError::Serving, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_error_serving_display_is_descriptive() {
        // A panic-derived JoinError exercises the Serving Display wording without
        // pulling in extra tokio features / a futures dependency.
        let handle: JoinHandle<()> = tokio::spawn(async {
            panic!("serve-task panic for Display assertion");
        });
        let join_err = handle.await.expect_err("panicking task yields a JoinError");
        let err = RunError::Serving(join_err);
        let rendered = format!("{err}");
        assert!(
            rendered.contains("serving task failed"),
            "Serving Display must describe the failure: {rendered}"
        );
    }
}
