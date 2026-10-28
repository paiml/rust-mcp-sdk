//! Shape C: a runnable streamable-HTTP MCP server in ≤15 lines of `main` body.
//!
//! This is THE canonical library-use wiring of `pmcp-server-toolkit` + the
//! in-toolkit `SqliteConnector` (D-07, D-08, SHAP-C-01). Plan 03's `cargo pmcp
//! new --kind sql-server` scaffold emits a byte-identical `main.rs` (minus the
//! `PMCP_ASSETS_DIR` harness line — the scaffold's assets are cwd-local).
//!
//! Asset/DB resolution is the shared Plan 01 H1 resolver so the SAME wiring
//! works locally AND on Lambda: `config.toml` + `schema.sql` are read via
//! [`pmcp::assets::load_string`] (resolves cwd/`PMCP_ASSETS_DIR` locally and
//! `/var/task/assets/` on Lambda) and SQLite opens at
//! [`pmcp_server_toolkit::demo_db_path`] (`/tmp/demo.db` on Lambda where
//! `/var/task` is read-only; local `demo.db` otherwise).
//!
//! `execute_batch` is called on the CONCRETE `SqliteConnector` BEFORE the
//! `Arc<dyn SqlConnector>` wrap (H2) — the inherent helper is not on the trait.
//!
//! Run with:
//! ```sh
//! cargo run --example sql_server_http --features sqlite,code-mode,http -p pmcp-server-toolkit
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig, StaticResourceHandler};
use tokio::task::JoinHandle;

/// Harness-only fixtures dir (compile-time constant). The Plan 03 scaffold drops
/// this seam because its assets are cwd-local — not counted toward the M4 body
/// budget since it lives outside `main`.
const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/fixtures");

/// Inline streamable-HTTP serve helper (CONVERGENCE NOTE): collapses
/// `with_config_concurrent` + `start` so `main` stays under the ≤15-line budget while still
/// inlining the `StreamableHttpServer` body (NOT importing `pmcp_sql_server::serve`,
/// Pitfall §2). Plan 03's scaffold emits a call to the same shape.
async fn serve(server: Server) -> Result<(SocketAddr, JoinHandle<()>), Box<dyn std::error::Error>> {
    let shared = Arc::new(server);
    let cfg = StreamableHttpServerConfig::default();
    Ok(
        StreamableHttpServer::with_config_concurrent("127.0.0.1:0".parse()?, shared, cfg)
            .start()
            .await?,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("PMCP_ASSETS_DIR", FIXTURES_DIR); // harness only — scaffold drops this
    let cfg = ServerConfig::from_toml_strict_validated(&pmcp::assets::load_string("config.toml")?)?;
    let conn = SqliteConnector::open(pmcp_server_toolkit::demo_db_path().as_ref())?; // CONCRETE (H2)
    conn.execute_batch(&pmcp::assets::load_string("schema.sql")?)
        .await?; // bootstrap on concrete
    let conn: Arc<dyn SqlConnector> = Arc::new(conn); // NOW wrap for the builder
    let builder = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version);
    let builder = builder.try_tools_from_config_with_connector(&cfg, conn.clone())?;
    let builder = builder.try_code_mode_from_config_with_connector(&cfg, conn)?;
    let server = builder
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .build()?;
    let (addr, handle) = serve(server).await?;
    println!("PMCP_SQL_SERVER_ADDR=http://{addr}"); // machine-readable bound addr (M1)
    handle.await?;
    Ok(())
}
