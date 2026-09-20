//! Smoke example — build a `pmcp-workbook-server` from the committed synthetic
//! golden bundle in library form (no CLI, no transport), in a small `main` body.
//!
//! This is the ALWAYS-matrix **runnable example** for the binary crate: it
//! demonstrates that the same `build_server` the binary uses can be driven
//! directly from a few lines of Rust against the committed synthetic golden
//! bundle `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0` — zero
//! customer / TowelRads material (hard constraint, D-05). The fail-closed boot
//! integrity gate runs inside `build_server`, so a successful build proves the
//! bundle's `BUNDLE.lock` hashes verified before any tool was registered.
//!
//! Run with:
//! ```sh
//! cargo run -p pmcp-workbook-server --example workbook_server_min
//! ```

use std::path::PathBuf;

use pmcp_workbook_server::{build_server, Args};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The committed synthetic golden bundle (read-only; reuse, do NOT regenerate
    // — D-05). Resolved from CARGO_MANIFEST_DIR so the example is cwd-invariant.
    let bundle_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0");

    let args = Args {
        bundle_dir,
        bundle_id: None,
        http: "127.0.0.1:0".to_string(),
    };

    // build_server runs the fail-closed boot integrity gate; a successful return
    // means the synthetic golden bundle verified and its tools registered — one
    // named compute tool per output Table (WBV2-04) plus the four meta tools.
    let server = build_server(&args)?;
    println!(
        "pmcp-workbook-server example: built a server from the synthetic golden \
         'tax-calc@1.1.0' bundle (calculate_tax present: {})",
        server.get_tool("calculate_tax").is_some()
    );
    let _ = server; // a real binary would `run_serving(&args).await?` then serve.
    Ok(())
}
