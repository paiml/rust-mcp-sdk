//! Shape A pure-config OpenAPI MCP server (`pmcp-openapi-server`).
//!
//! This crate is the OpenAPI sibling of `pmcp-sql-server` in the v2.2
//! "Configuration-Only MCP Servers" milestone: a standalone binary that an
//! operator points at a `config.toml` (the `[[tools]]` / `[backend]` /
//! `[code_mode]` declarations) plus an OPTIONAL OpenAPI spec and runs as a
//! production MCP server — **without writing any Rust**. It assembles a
//! [`pmcp::Server`] entirely from `pmcp-server-toolkit` config primitives + an
//! HTTP/OpenAPI connector (`HttpConnector` / `HttpCodeExecutor`).
//!
//! # Crate layout (lib + bin split)
//!
//! The testable assembly entry point lives here in the library; the
//! `pmcp-openapi-server` binary (`src/main.rs`) is a thin `#[tokio::main]` shim
//! that parses CLI/env arguments and delegates to [`run`].
//!
//! # Pipeline ([`run`])
//!
//! [`run`] is the full Shape A pipeline: [`load_config_and_spec`] (parse the
//! config; parse the spec ONLY when `--spec` is supplied — D-03) → [`dispatch`]
//! the `(HttpConnector, HttpCodeExecutor)` pair lazily (CF-2) → [`build_server`]
//! the [`pmcp::Server`] (single-call + script tools + Code Mode + the configured
//! resources/prompts, with the spec merged as the `api_schema` resource when
//! present) → serve it over streamable HTTP (CF-1) via [`serve`].
//!
//! # Seams
//!
//! - [`cli`]: the clap [`Args`] surface (`--config` / `--spec` / `--http`).
//! - [`dispatch`]: the `[backend]` → `(HttpConnector, HttpCodeExecutor)` pair
//!   ([`dispatch::DispatchError`]).
//! - [`assemble`]: config + pair + optional spec → built [`pmcp::Server`], plus
//!   the inbound token capture for `oauth_passthrough` (H1).

pub mod assemble;
pub mod cli;
pub mod dispatch;

pub use assemble::{build_server, AssembleError};
pub use cli::Args;
pub use dispatch::{dispatch, DispatchError};

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_server_toolkit::http::OpenApiSchema;
use pmcp_server_toolkit::ServerConfig;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Error from the Shape A [`run`] pipeline.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// Reading the `--config` or `--spec` file from disk failed.
    #[error("failed to read {what} file: {source}")]
    Io {
        /// Which file (`config` or `spec`).
        what: &'static str,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Parsing or validating the config TOML failed.
    #[error("config parse/validate failed: {0}")]
    Config(#[from] pmcp_server_toolkit::ToolkitError),

    /// Parsing the `--spec` OpenAPI document failed.
    #[error("OpenAPI spec parse failed: {0}")]
    Spec(#[source] pmcp_server_toolkit::http::HttpConnectorError),

    /// The `[backend]` could not be dispatched to a connector/executor pair.
    #[error("backend dispatch failed: {0}")]
    Dispatch(#[from] DispatchError),

    /// Assembling the `pmcp::Server` from config + pair + spec failed.
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
    /// A discarded `JoinError` would hide a crashed listener — the process looks
    /// healthy while serving nothing. Propagating it surfaces as a non-zero
    /// process exit so a supervisor restarts the binary (threat T-90-06-03 /
    /// T-85-10-02).
    #[error("streamable-HTTP serving task failed: {0}")]
    Serving(#[source] tokio::task::JoinError),
}

/// Load + validate the config and (ONLY when `--spec` is supplied) parse the
/// OpenAPI spec (D-03).
///
/// The spec is parsed lazily — a curated-only server (`--spec` absent) returns
/// `(cfg, None)` and never touches the parser.
///
/// # Errors
///
/// [`RunError::Io`] when a file is unreadable, [`RunError::Config`] when the TOML
/// fails to parse/validate, or [`RunError::Spec`] when the OpenAPI document fails
/// to parse.
pub fn load_config_and_spec(
    args: &Args,
) -> Result<(ServerConfig, Option<OpenApiSchema>), RunError> {
    let config_text = std::fs::read_to_string(&args.config).map_err(|source| RunError::Io {
        what: "config",
        source,
    })?;
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)?;

    // D-03: parse the spec ONLY when --spec is supplied.
    let spec = match args.spec.as_ref() {
        Some(path) => {
            let spec_text = std::fs::read_to_string(path).map_err(|source| RunError::Io {
                what: "spec",
                source,
            })?;
            Some(OpenApiSchema::parse(&spec_text).map_err(RunError::Spec)?)
        },
        None => None,
    };
    Ok((cfg, spec))
}

/// Start the streamable-HTTP server for `server` on `addr`, returning the REAL
/// bound address and the serving task handle (CF-1).
///
/// Uses the Phase 56 Tower/axum adapter ([`StreamableHttpServer`]) so the
/// DNS-rebinding, CORS, and security-headers layers are applied by the SDK —
/// never hand-rolled (threat T-90-06-02). Returning `(addr, handle)` rather than
/// blocking lets tests drive the server and `run` await the handle.
///
/// # Errors
///
/// [`RunError::Serve`] if binding the listener fails (e.g. the port is in use).
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use pmcp_openapi_server::{build_server, serve};
/// use pmcp_server_toolkit::ServerConfig;
/// use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
/// use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
/// use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let cfg = ServerConfig::default();
/// let auth = create_auth_provider(&AuthConfig::None)?;
/// let client = reqwest::Client::new();
/// let connector: Arc<dyn HttpConnector> =
///     Arc::new(HttpClient::new(client.clone(), "https://api.example.com".into(), auth.clone())?);
/// let http_exec = HttpCodeExecutor::new(client, "https://api.example.com".into(), auth);
/// let server = build_server(&cfg, connector, http_exec, None, &Default::default())?;
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
    let shared = Arc::new(Mutex::new(server));
    let http =
        StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    http.start().await.map_err(RunError::Serve)
}

/// Run the FULL Shape A pipeline up to (but not including) blocking on the
/// serving task, returning the REAL bound address and the serving task handle.
///
/// This is the *exact* binary path — [`load_config_and_spec`] → [`dispatch`] →
/// [`build_server`] → [`serve`] — with no injection. It is the testable seam
/// [`run`] delegates to: [`run`] calls this and awaits the handle, while
/// integration tests call it directly to obtain the ephemeral bound address,
/// drive the live server, and `abort()` the handle (bounded shutdown).
///
/// # Errors
///
/// Any [`RunError`] variant — file I/O, config parse/validate, spec parse,
/// backend dispatch, server assembly, address parse, or transport startup.
///
/// # Example
///
/// ```no_run
/// use pmcp_openapi_server::{run_serving, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let args = Args {
///     config: "config.toml".into(),
///     spec: None, // D-03: curated-only boot
///     http: "127.0.0.1:0".to_string(), // ephemeral port
/// };
/// let (bound, handle) = run_serving(&args).await?;
/// println!("listening on http://{bound}");
/// handle.abort(); // stop serving
/// # Ok(())
/// # }
/// ```
pub async fn run_serving(args: &Args) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let (cfg, spec) = load_config_and_spec(args)?;
    let (connector, http_exec) = dispatch(&cfg).await?;
    // Phase 128: the pure-config binary path registers no Rust-side hook — there is
    // no Rust in a Shape A deployment to register one — so it passes the empty
    // default. A Rust embedder calls `build_server` directly with its own
    // `ToolkitHooks`, which is the wiring E1/E2 are reachable through.
    let server = build_server(
        &cfg,
        connector,
        http_exec,
        spec,
        &pmcp_server_toolkit::ToolkitHooks::default(),
    )?;

    let addr: SocketAddr = args.http.parse().map_err(|source| RunError::Addr {
        addr: args.http.clone(),
        source,
    })?;

    serve(server, addr).await
}

/// Assemble and serve the Shape A OpenAPI MCP server from configuration, then
/// await the serving task (blocks until the task ends or the process is
/// signalled).
///
/// Initialises a `tracing_subscriber::fmt` reader of `RUST_LOG` (best-effort).
///
/// # Errors
///
/// Any [`RunError`] variant.
///
/// # Example
///
/// ```no_run
/// use pmcp_openapi_server::{run, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// // Equivalent to `pmcp-openapi-server --config config.toml`.
/// run(Args {
///     config: "config.toml".into(),
///     spec: None,
///     http: "127.0.0.1:8080".to_string(),
/// })
/// .await?; // blocks serving until the task ends
/// # Ok(())
/// # }
/// ```
pub async fn run(args: Args) -> Result<(), RunError> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let (bound, handle) = run_serving(&args).await?;
    tracing::info!(target: "pmcp_openapi_server", %bound, "streamable-HTTP server listening");

    // Await the serving task for the process lifetime. A JoinError (panic /
    // abort) is propagated as RunError::Serving so the process exits non-zero
    // (threat T-90-06-03 / T-85-10-02).
    handle.await.map_err(RunError::Serving)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// T-90-06-03: a serving-task panic must surface as `RunError::Serving` (a
    /// non-zero process exit), NOT a discarded `()`.
    #[tokio::test]
    async fn serving_task_panic_maps_to_run_error_serving() {
        let handle: JoinHandle<()> = tokio::spawn(async {
            panic!("simulated serve-task panic");
        });
        let outcome: Result<(), RunError> = handle.await.map_err(RunError::Serving);
        match outcome {
            Ok(()) => panic!("a panicking serve task must NOT map to Ok(())"),
            Err(RunError::Serving(join_err)) => {
                assert!(join_err.is_panic(), "the JoinError must reflect the panic");
            },
            Err(other) => panic!("expected RunError::Serving, got {other:?}"),
        }
    }

    #[test]
    fn run_error_addr_display_is_credential_free() {
        let http = Args::try_parse_from([
            "pmcp-openapi-server",
            "--config",
            "c.toml",
            "--http",
            "not-an-addr",
        ])
        .map(|a| a.http)
        .unwrap();
        assert_eq!(http, "not-an-addr");
    }
}
