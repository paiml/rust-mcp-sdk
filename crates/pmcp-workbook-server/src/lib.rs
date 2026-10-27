//! Shape A pure-config workbook MCP server (`pmcp-workbook-server`).
//!
//! This crate is the **Shape A** delivery of the v2.3 governed-workbook
//! milestone: a standalone binary that an operator points at a compiled
//! `bundle@version` directory and runs as a live MCP server serving the five
//! workbook tools — **without writing any Rust**. It assembles a
//! [`pmcp::Server`] entirely from a `pmcp-server-toolkit` workbook bundle (the
//! fail-closed boot integrity gate runs before any tool is registered),
//! mirroring the field-for-field structure of `pmcp-sql-server`.
//!
//! # Crate layout (lib + bin split)
//!
//! The testable assembly entry point lives here in the library ([`run`]); the
//! `pmcp-workbook-server` binary (`src/main.rs`) is a thin `#[tokio::main]` shim
//! that parses CLI arguments and delegates to [`run`]. This split keeps the
//! server-construction logic unit-testable without spawning a process.
//!
//! # Pipeline ([`run`])
//!
//! [`run`] is the full Shape A pipeline: select a `BundleSource` from
//! `--bundle-dir` (+ the optional `--bundle-id` assertion) → [`build_server`]
//! the [`pmcp::Server`] via the toolkit's fail-closed
//! `try_with_workbook_bundle` boot gate → serve it over streamable HTTP via the
//! Phase 56 Tower/axum adapter ([`serve`]).
//!
//! # Seams
//!
//! - [`cli`]: the clap [`Args`] surface (`--bundle-dir` / `--bundle-id` /
//!   `--http`).
//! - [`assemble`]: the NOVEL `--bundle-dir` (+ `--bundle-id`) →
//!   `try_with_workbook_bundle` → built [`pmcp::Server`] seam.

pub mod assemble;
pub mod cli;

pub use assemble::build_server;
pub use cli::Args;

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use tokio::task::JoinHandle;

/// Error from the Shape A [`run`] pipeline.
///
/// No `Io` variant: the binary reads no config/schema files (D-03 pure CLI
/// args), so an `Io` variant would have no producer (Codex MEDIUM #5).
/// `pmcp-sql-server`'s `Io` (config/schema file reads) is deliberately dropped.
/// The bundle integrity-load I/O is already wrapped fail-closed by the toolkit's
/// [`pmcp_server_toolkit::ToolkitError`] → [`RunError::Bundle`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// Loading / integrity-verifying the bundle through the toolkit failed
    /// (fail-closed boot gate). Carries the toolkit's own error for diagnostics.
    #[error("workbook bundle load/verify failed: {0}")]
    Bundle(#[from] pmcp_server_toolkit::ToolkitError),

    /// The `--bundle-id` assertion did not match the loaded bundle's identity
    /// (D-01 fail-closed). Echoes ONLY the operator-typed `bundle_id` strings
    /// (credential-free, mirrors `pmcp-sql-server`'s `DispatchError`) — never the
    /// raw `--bundle-dir` filesystem path.
    #[error("--bundle-id '{expected}' does not match the loaded bundle id '{actual}'")]
    BundleIdMismatch {
        /// The operator-supplied expected bundle id.
        expected: String,
        /// The actual bundle id read from the loaded `BUNDLE.lock`.
        actual: String,
    },

    /// The `--http` bind address could not be parsed as a `SocketAddr`.
    #[error("invalid --http bind address '{addr}': {source}")]
    Addr {
        /// The offending address string.
        addr: String,
        /// Parse error.
        source: std::net::AddrParseError,
    },

    /// The final `pmcp::Server::builder().build()` failed (in-process assembly,
    /// before any listener is bound). Distinct from [`RunError::Serve`]
    /// (transport startup): a build failure never touches a socket, so reporting
    /// it as a transport-start error would misdirect an operator toward
    /// port/bind faults for what is actually a server-construction error.
    #[error("workbook server build failed: {0}")]
    Build(#[source] pmcp::Error),

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

/// Start the streamable-HTTP server for `server` on `addr`, returning the REAL
/// bound address and the serving task handle.
///
/// Uses the Phase 56 Tower/axum adapter
/// ([`StreamableHttpServer`]) so the DNS-rebinding, CORS, and security-headers
/// layers are applied by the SDK — never hand-rolled. The default
/// [`StreamableHttpServerConfig`] is stateful with `AllowedOrigins::localhost()`
/// (the loopback default matching `--http`'s `127.0.0.1` default — D-04).
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
/// use pmcp_workbook_server::{build_server, serve, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let args = Args {
///     bundle_dir: "bundles/tax-calc@1.1.0".into(),
///     bundle_id: None,
///     http: "127.0.0.1:0".to_string(),
/// };
/// let server = build_server(&args)?;
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
/// This is the *exact* binary path — select the `BundleSource` from
/// `--bundle-dir` (+ the optional `--bundle-id` assertion) → [`build_server`]
/// the [`pmcp::Server`] via the fail-closed boot gate → [`serve`] it over
/// streamable HTTP. It is the testable seam [`run`] delegates to: [`run`] calls
/// this and then awaits the returned handle, while integration tests call it
/// directly to obtain the ephemeral bound address, drive the live server, and
/// `abort()` the handle.
///
/// `build_server` runs BEFORE the `--http` parse, so a bundle-load failure is
/// surfaced before the address is even inspected.
///
/// # Errors
///
/// Any [`RunError`] variant — bundle load/integrity, `--bundle-id` mismatch,
/// address parse ([`RunError::Addr`] when `--http` is not a `SocketAddr`), or
/// transport startup.
///
/// # Example
///
/// ```no_run
/// use pmcp_workbook_server::{run_serving, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let args = Args {
///     bundle_dir: "bundles/tax-calc@1.1.0".into(),
///     bundle_id: None,
///     http: "127.0.0.1:0".to_string(), // ephemeral port
/// };
/// let (bound, handle) = run_serving(&args).await?;
/// println!("listening on http://{bound}");
/// handle.abort(); // stop serving
/// # Ok(())
/// # }
/// ```
pub async fn run_serving(args: &Args) -> Result<(SocketAddr, JoinHandle<()>), RunError> {
    let server = build_server(args)?;

    let addr: SocketAddr = args.http.parse().map_err(|source| RunError::Addr {
        addr: args.http.clone(),
        source,
    })?;

    serve(server, addr).await
}

/// Assemble and serve the Shape A workbook MCP server from a compiled bundle.
///
/// The full pipeline: select the `BundleSource` from `--bundle-dir` (+ the
/// optional `--bundle-id` assertion) → [`build_server`] the [`pmcp::Server`] via
/// the fail-closed boot gate → [`serve`] it over streamable HTTP, then await the
/// serving task (blocks until the task ends or the process is signalled).
///
/// Initialises a `tracing_subscriber::fmt` reader of `RUST_LOG` so operators get
/// structured logs from a local binary; the subscriber is best-effort (a second
/// call in the same process is ignored).
///
/// # Errors
///
/// Any [`RunError`] variant — bundle load/integrity, `--bundle-id` mismatch,
/// address parse, or transport startup.
///
/// # Example
///
/// ```no_run
/// use pmcp_workbook_server::{run, Args};
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// // Equivalent to `pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0`.
/// run(Args {
///     bundle_dir: "bundles/tax-calc@1.1.0".into(),
///     bundle_id: None,
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
    tracing::info!(target: "pmcp_workbook_server", %bound, "streamable-HTTP server listening");

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
    use std::path::PathBuf;

    /// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
    /// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR` so the test is
    /// invariant to the cwd.
    fn golden_bundle_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
    }

    /// Codex MEDIUM #3: an invalid `--http` value must fail CLOSED at the
    /// address-parse step of `run_serving` and surface as [`RunError::Addr`],
    /// never reaching transport startup (no listener is ever bound).
    ///
    /// `run_serving` calls `build_server` BEFORE parsing `--http`, so a VALID
    /// golden `--bundle-dir` is used here to isolate the address-parse failure
    /// (a bad bundle dir would instead trip [`RunError::Bundle`] first).
    #[tokio::test]
    async fn invalid_http_maps_to_run_error_addr() {
        let args = Args {
            bundle_dir: golden_bundle_dir(),
            bundle_id: None,
            http: "not-an-addr".to_string(),
        };
        let err = run_serving(&args)
            .await
            .expect_err("an invalid --http must fail closed before serving");
        match err {
            RunError::Addr { addr, .. } => {
                assert_eq!(
                    addr, "not-an-addr",
                    "the Addr variant echoes the operator-typed bind string"
                );
            },
            other => panic!("expected RunError::Addr, got {other:?}"),
        }
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

    /// WR-01: a `Server::builder().build()` failure must surface as the distinct
    /// [`RunError::Build`] variant whose Display names the build phase — NOT the
    /// transport-start [`RunError::Serve`] wording. This guards against the
    /// re-skin regression that reported an in-process assembly fault as a
    /// "streamable-HTTP server failed to start" error.
    #[test]
    fn run_error_build_display_names_the_build_phase() {
        let err = RunError::Build(pmcp::Error::internal("synthetic build failure"));
        let rendered = format!("{err}");
        assert!(
            rendered.contains("workbook server build failed"),
            "Build Display must name the build phase, not transport start: {rendered}"
        );
        assert!(
            !rendered.contains("failed to start"),
            "Build Display must NOT borrow the transport-start wording: {rendered}"
        );
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
