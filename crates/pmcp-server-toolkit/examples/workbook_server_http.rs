//! Shape A/B: a streamable-HTTP MCP server that serves a governed-Excel workbook
//! over ONE named compute tool per output Table (WBV2-04 — `calculate_tax` and
//! `estimate_refund` for the `tax-calc` golden) plus the four workbook-wide meta
//! tools (`explain`, `get_manifest`, `diff_version`, `render_workbook`) and the
//! `workbook://` render resource.
//!
//! This is THE canonical wiring of [`WorkbookBuilderExt::try_with_workbook_bundle`]
//! (D-09/D-12): a single chained call loads + integrity-verifies the bundle at
//! boot (fail-closed — a tampered bundle aborts the boot, WBSV-08) and registers
//! the served surface. The consumer imports SOLELY from `pmcp_server_toolkit`,
//! never naming `pmcp-workbook-runtime` (D-11).
//!
//! # Bundle source: embedded by default, `--bundle-dir` for live updates
//!
//! By default the example bakes the committed synthetic `tax-calc@1.1.0` golden
//! directly into the binary via [`include_dir!`] + [`EmbeddedSource`] — a
//! self-contained deploy artifact (WBSV-09): the spreadsheet logic ships INSIDE
//! the binary, so a remote deploy (Lambda / container) carries no out-of-band
//! bundle directory. That is the production default for an immutable workbook.
//!
//! Passing `--bundle-dir <path>` switches to a [`LocalDirSource`] over that
//! directory instead. This is how a production operator points the SAME binary
//! at a workbook updated OUT-OF-BAND (a newly promoted bundle dropped onto a
//! mounted volume) WITHOUT rebuilding — the embedded→local transition is the
//! seam between "ship the logic in the binary" and "update the logic at runtime".
//! Either way the bundle is loaded fail-closed: a tampered directory aborts boot.
//!
//! Run with:
//! ```sh
//! cargo run --example workbook_server_http \
//!   --features workbook-embedded,http -p pmcp-server-toolkit
//! # or point at an out-of-band bundle directory:
//! cargo run --example workbook_server_http \
//!   --features workbook-embedded,http -p pmcp-server-toolkit -- \
//!   --bundle-dir bundles/tax-calc@1.1.0
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use include_dir::{include_dir, Dir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_server_toolkit::workbook::{EmbeddedSource, LocalDirSource, WorkbookBuilderExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// The committed synthetic golden bundle, baked into the binary at compile time.
/// Points STRAIGHT at the 92-02 committed `@1.1.0` golden (no examples/fixtures
/// duplication) so the embedded bytes are byte-identical to the on-disk golden
/// the integration tests load via `LocalDirSource`.
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0");

/// Inline streamable-HTTP serve helper: collapses `with_config` + `start` and
/// binds an ephemeral port (`127.0.0.1:0`) so the bound address is reported back
/// for the smoke test / operator to read.
async fn serve(server: Server) -> Result<(SocketAddr, JoinHandle<()>), Box<dyn std::error::Error>> {
    let shared = Arc::new(Mutex::new(server));
    let cfg = StreamableHttpServerConfig::default();
    Ok(
        StreamableHttpServer::with_config("127.0.0.1:0".parse()?, shared, cfg)
            .start()
            .await?,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle_dir = std::env::args().skip_while(|a| a != "--bundle-dir").nth(1);
    let builder = Server::builder().name("workbook-tax-calc").version("1.1.0");
    let builder = match bundle_dir {
        Some(dir) => builder.try_with_workbook_bundle(&LocalDirSource::new(dir))?,
        None => builder.try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))?,
    };
    let server = builder.build()?;
    let (addr, handle) = serve(server).await?;
    println!("PMCP_WORKBOOK_SERVER_ADDR=http://{addr}"); // machine-readable bound addr
    handle.await?;
    Ok(())
}
