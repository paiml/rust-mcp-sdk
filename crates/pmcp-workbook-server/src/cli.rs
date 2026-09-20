//! Command-line surface for the `pmcp-workbook-server` binary.
//!
//! Honours the D-03 pure-CLI-args model: `--bundle-dir` points at a compiled
//! `bundle@version` directory (the version is implicit in the path — D-01),
//! `--bundle-id` is an optional fail-closed assertion that the loaded bundle's
//! identity matches what the operator expects, and `--http` selects the
//! streamable-HTTP bind address (loopback default — D-04).
//!
//! The struct is parsed by [`Args::parse`] in `main.rs` and is re-exported from
//! the crate root so both the binary and the test suite reach it via a single
//! path (`pmcp_workbook_server::Args`).
//!
//! # Example
//!
//! ```
//! use pmcp_workbook_server::Args;
//! use clap::Parser;
//!
//! let args = Args::try_parse_from(["pmcp-workbook-server", "--bundle-dir", "bundles/tax-calc@1.1.0"])
//!     .expect("required --bundle-dir parses");
//! assert_eq!(args.bundle_dir.to_str(), Some("bundles/tax-calc@1.1.0"));
//! assert_eq!(args.bundle_id, None);
//! assert_eq!(args.http, "127.0.0.1:8080");
//! ```

use std::path::PathBuf;

/// Parsed CLI arguments for the Shape A pure-config workbook MCP server.
///
/// `--bundle-dir` is required (clap emits a usage error when it is missing);
/// `--bundle-id` is an optional fail-closed identity assertion (D-01); `--http`
/// defaults to `127.0.0.1:8080` (D-04 loopback, so the out-of-the-box binary
/// does not expose a public listener).
///
/// `#[command(version)]` makes the binary report its OWN crate version (0.1.0)
/// for audit logs (Gemini #7). This is the binary's version and is unrelated to
/// the bundle's `--bundle-version` (which is excluded — D-01: the version is
/// implicit in the `--bundle-dir` path), so it does not conflict with D-01.
///
/// D-03: pure CLI args, no `env(...)` overrides on any flag. Env-var parity with
/// `pmcp-sql-server` for containerized deploys was DECLINED to honour the locked
/// "pure CLI args" decision (Gemini #9 acknowledged, not silently dropped).
#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "pmcp-workbook-server",
    version,
    about = "Shape A pure-config workbook MCP server — point it at a compiled bundle dir and serve its governed-Excel workbook tools with no Rust required"
)]
pub struct Args {
    /// Path to the compiled `bundle@version` directory (e.g.
    /// `bundles/tax-calc@1.1.0`). One directory = one bundle@version; the
    /// version is implicit in the path (D-01). Required.
    #[arg(long)]
    pub bundle_dir: PathBuf,

    /// Optional fail-closed assertion that the loaded bundle's `bundle_id`
    /// matches this value (D-01). On mismatch the binary exits non-zero BEFORE
    /// registering any tool.
    #[arg(long)]
    pub bundle_id: Option<String>,

    /// Bind address for the streamable-HTTP transport (`host:port`).
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn parses_required_bundle_dir_with_defaults() {
        let args = Args::try_parse_from([
            "pmcp-workbook-server",
            "--bundle-dir",
            "bundles/tax-calc@1.1.0",
        ])
        .expect("required --bundle-dir must parse");
        assert_eq!(args.bundle_dir.to_str(), Some("bundles/tax-calc@1.1.0"));
        assert_eq!(args.bundle_id, None, "--bundle-id absent parses to None");
        assert_eq!(
            args.http, "127.0.0.1:8080",
            "http defaults to loopback:8080"
        );
    }

    #[test]
    fn bundle_id_flag_parses_to_some() {
        let args = Args::try_parse_from([
            "pmcp-workbook-server",
            "--bundle-dir",
            "bundles/tax-calc@1.1.0",
            "--bundle-id",
            "tax-calc",
        ])
        .expect("explicit --bundle-id must parse");
        assert_eq!(args.bundle_id.as_deref(), Some("tax-calc"));
    }

    #[test]
    fn http_flag_overrides_default() {
        let args = Args::try_parse_from([
            "pmcp-workbook-server",
            "--bundle-dir",
            "bundles/tax-calc@1.1.0",
            "--http",
            "0.0.0.0:9000",
        ])
        .expect("explicit --http must parse");
        assert_eq!(args.http, "0.0.0.0:9000");
    }

    #[test]
    fn missing_bundle_dir_is_a_usage_error() {
        let err = Args::try_parse_from(["pmcp-workbook-server", "--bundle-id", "tax-calc"]);
        assert!(err.is_err(), "missing --bundle-dir must fail clap parsing");
    }
}
