//! Single-crate `workbook-server` template emitter (Shape B, WBCL-05).
//!
//! Mirrors [`crate::templates::sql_server`]: one `generate` orchestrator that
//! calls one private `generate_<file>` fn per output file, each a single raw
//! `fs::write(dir.join("X"), <literal>).context(...)`. There is NO template
//! engine — text emission is raw string literals (`format!` escapes literal
//! braces as `{{`/`}}`).
//!
//! Unlike `sql_server` (which emits ONLY text files) this scaffold also carries
//! BINARY assets — the source `.xlsx` and the pre-compiled `tax-calc@1.1.0`
//! bundle (D-07). Those bytes are EMBEDDED into the published `cargo-pmcp`
//! binary via [`include_dir!`]/[`include_bytes!`] over an in-package asset dir
//! (`src/templates/workbook_bundle/`). They are NEVER read from `crates/*` at
//! generate-time: copying from a workspace path works in the monorepo but BREAKS
//! once `cargo-pmcp` is published standalone (the workspace siblings are not in
//! the published crate). Embedding makes them travel inside the published crate.
//!
//! It emits the Shape B payload (D-06) into a SINGLE crate directory:
//! - `Cargo.toml` — pins `pmcp-server-toolkit` with `default-features = false,
//!   features = ["workbook-embedded", "http"]`. The `default-features = false`
//!   is MANDATORY (T-95-06 purity gate — the toolkit's DEFAULT `code-mode`
//!   feature pulls `pmcp-code-mode`/SWC into the served tree and trips
//!   `make purity-check`). `workbook-embedded` (NOT bare `workbook`) supplies
//!   [`EmbeddedSource`].
//! - `src/main.rs` — the `EmbeddedSource` wiring from the canonical
//!   `workbook_server_http.rs` example, drift-locked to it by a golden test.
//! - `pmcp.toml` — a sample project config mapping the tax-calc workbook to its
//!   bundle id (mirrors the Phase 94 `[[workbook]]` shape).
//! - `workbook/tax-calc.xlsx` — the source workbook (D-07), written from the
//!   embedded bytes so the dev can edit it → `cargo pmcp workbook compile` →
//!   rerun (the full authoring loop).
//! - `bundle/tax-calc@1.1.0/*` — the pre-compiled embedded bundle (D-07),
//!   written from the embedded `include_dir!` assets; `cargo run` works
//!   immediately against it.

use anyhow::{Context, Result};
use colored::Colorize;
use include_dir::{include_dir, Dir};
use std::fs;
use std::path::Path;

/// The pre-compiled `tax-calc@1.1.0` golden bundle, embedded from the in-package
/// asset dir so it ships inside the published `cargo-pmcp` crate (T-96-06b).
static EMBEDDED_BUNDLE: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/templates/workbook_bundle/tax-calc@1.1.0");

/// The source `tax-calc.xlsx`, embedded from the in-package asset dir (T-96-06b).
static EMBEDDED_XLSX: &[u8] = include_bytes!("workbook_bundle/tax-calc.xlsx");

/// The pinned `pmcp` version the emitted `Cargo.toml` declares. A test asserts
/// this equals the workspace-root `pmcp` version so the hardcoded pin cannot
/// silently drift from the released crate (Codex MEDIUM).
const PMCP_VERSION: &str = "2.19.2";

/// The pinned `pmcp-server-toolkit` version the emitted `Cargo.toml` declares. A
/// test asserts this equals the workspace `pmcp-server-toolkit` package version so
/// the hardcoded pin cannot silently drift from the released crate (ME-01) —
/// mirroring the `PMCP_VERSION` drift guard.
const TOOLKIT_VERSION: &str = "0.1.2";

/// Emit the files of a single runnable `workbook-server` crate into `dir`.
pub fn generate(dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(dir, name)?;
    generate_main_rs(dir)?;
    generate_pmcp_toml(dir, name)?;
    generate_workbook_xlsx(dir)?;
    generate_bundle(dir)?;

    if std::env::var("PMCP_QUIET").is_err() {
        println!("  {} Generated workbook server crate files", "✓".green());
    }
    Ok(())
}

fn generate_cargo_toml(dir: &Path, name: &str) -> Result<()> {
    // Purity posture (T-95-06, Pitfall 4): `default-features = false` is
    // MANDATORY — the toolkit's DEFAULT feature set includes `code-mode` which
    // pulls `pmcp-code-mode` (SWC/JS) into the served tree and trips
    // `make purity-check`. The workbook server serves PRE-COMPILED bundles, not
    // SQL/JS code-mode, so it needs `workbook-embedded` (EmbeddedSource) + `http`
    // ONLY. NEVER copy sql_server's `features = ["code-mode", ...]`.
    let content = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "{PMCP_VERSION}", features = ["streamable-http"] }}
# default-features = false is MANDATORY (T-95-06 purity gate): the toolkit
# DEFAULT pulls `code-mode` → pmcp-code-mode (SWC/JS) into the served tree and
# trips `make purity-check`. `workbook-embedded` (NOT bare workbook) supplies
# EmbeddedSource; `http` forwards the streamable-HTTP server.
pmcp-server-toolkit = {{ version = "{TOOLKIT_VERSION}", default-features = false, features = ["workbook-embedded", "http"] }}
include_dir = "0.7.4"
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
"#,
    );

    fs::write(dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;
    Ok(())
}

/// The emitted `src/main.rs`. This is the canonical `EmbeddedSource` wiring from
/// `crates/pmcp-server-toolkit/examples/workbook_server_http.rs`, with the ONLY
/// deviations being (a) the example's `//!` header doc, (b) the harness-only
/// `--bundle-dir`/`LocalDirSource` branch (dropped — the scaffold always serves
/// its embedded bundle), and (c) the `include_dir!` path rewritten from the
/// example's `tests/fixtures/tax-calc@1.1.0` to the scaffold-local
/// `bundle/tax-calc@1.1.0`. The golden test
/// `emitted_main_matches_example_modulo_setup` enforces this cannot drift.
fn emitted_main_rs() -> &'static str {
    r#"//! Config-driven governed-Excel workbook MCP server (streamable HTTP).
//!
//! Generated by `cargo pmcp new --kind workbook-server`. Bakes the pre-compiled
//! `tax-calc@1.1.0` bundle into the binary via [`include_dir!`] +
//! [`EmbeddedSource`] and serves one named compute tool per output Table
//! (`calculate_tax`, `estimate_refund`) plus the four meta tools (`explain`,
//! `get_manifest`, `diff_version`, `render_workbook`). Edit
//! `workbook/tax-calc.xlsx` → `cargo pmcp workbook compile` → rerun for the full
//! authoring loop.

use std::net::SocketAddr;
use std::sync::Arc;

use include_dir::{include_dir, Dir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_server_toolkit::workbook::{EmbeddedSource, WorkbookBuilderExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// The pre-compiled bundle, baked into the binary at compile time.
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundle/tax-calc@1.1.0");

/// Inline streamable-HTTP serve helper: collapses `with_config` + `start` and
/// binds an ephemeral port (`127.0.0.1:0`) so the bound address is reported back.
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
    let builder = Server::builder().name("workbook-tax-calc").version("1.1.0");
    let builder = builder.try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))?;
    let server = builder.build()?;
    let (addr, handle) = serve(server).await?;
    println!("PMCP_WORKBOOK_SERVER_ADDR=http://{addr}"); // machine-readable bound addr
    handle.await?;
    Ok(())
}
"#
}

fn generate_main_rs(dir: &Path) -> Result<()> {
    fs::write(dir.join("src").join("main.rs"), emitted_main_rs())
        .context("Failed to create src/main.rs")?;
    Ok(())
}

fn generate_pmcp_toml(dir: &Path, name: &str) -> Result<()> {
    // Sample project config mapping the tax-calc workbook → its bundle id
    // (mirrors the Phase 94 `[[workbook]]` shape: path → bundle_id → out_dir).
    // Editing `workbook/tax-calc.xlsx` then `cargo pmcp workbook compile` writes
    // the refreshed bundle to `out_dir`, which `src/main.rs` embeds.
    let content = format!(
        r#"# Project config for the governed-Excel workbook MCP server `{name}`
# (mirrors the Phase 94 `[[workbook]]` shape). `cargo pmcp workbook compile`
# reads this to recompile the source workbook into `out_dir`.

[[workbook]]
# The source workbook this crate scaffolds with (D-07 — the proven tax-calc
# golden). Edit it, then `cargo pmcp workbook compile` to refresh the bundle.
path = "workbook/tax-calc.xlsx"
bundle_id = "tax-calc"
out_dir = "bundle"
"#,
    );

    fs::write(dir.join("pmcp.toml"), content).context("Failed to create pmcp.toml")?;
    Ok(())
}

fn generate_workbook_xlsx(dir: &Path) -> Result<()> {
    // Source workbook (D-07), written from the EMBEDDED bytes — never copied
    // from `crates/*` at generate-time (publish-safety, T-96-06b).
    let workbook_dir = dir.join("workbook");
    fs::create_dir_all(&workbook_dir).context("Failed to create workbook directory")?;
    fs::write(workbook_dir.join("tax-calc.xlsx"), EMBEDDED_XLSX)
        .context("Failed to create workbook/tax-calc.xlsx")?;
    Ok(())
}

fn generate_bundle(dir: &Path) -> Result<()> {
    // Pre-compiled bundle (D-07), written from the EMBEDDED `include_dir!`
    // assets — never copied from `crates/*` at generate-time (publish-safety,
    // T-96-06b). `extract` recreates the full nested tree (incl. `evidence/`).
    let bundle_dir = dir.join("bundle").join("tax-calc@1.1.0");
    fs::create_dir_all(&bundle_dir).context("Failed to create bundle directory")?;
    EMBEDDED_BUNDLE
        .extract(&bundle_dir)
        .context("Failed to extract embedded tax-calc@1.1.0 bundle")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical Shape A/B `EmbeddedSource` example. The emitted
    /// `src/main.rs` must be the same wiring modulo (a) the example's `//!`
    /// header doc, (b) the harness-only `--bundle-dir`/`LocalDirSource` branch,
    /// and (c) the `include_dir!` path (the example points at
    /// `tests/fixtures/tax-calc@1.1.0`, the scaffold at `bundle/tax-calc@1.1.0`).
    /// This proves the scaffold cannot drift from the canonical wiring.
    const EXAMPLE_SRC: &str =
        include_str!("../../../crates/pmcp-server-toolkit/examples/workbook_server_http.rs");

    /// The workspace-root `Cargo.toml` (for the version-drift guard).
    const ROOT_CARGO_TOML: &str = include_str!("../../../Cargo.toml");

    /// The committed golden bundle dir (source of truth for the bundle-bytes
    /// drift guard). The in-package embedded copy must match it byte-for-byte.
    const GOLDEN_BUNDLE_REL: &str = "crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0";

    /// Collapse a Rust source to its load-bearing wiring, LF/CRLF-insensitive
    /// (Gemini LOW — a Windows-authored example must not trip equality): drop
    /// blank lines, `//!` header docs, `///`/`//` comments, the example's
    /// `LocalDirSource` import + its `--bundle-dir` harness branch, and normalize
    /// the `include_dir!` bundle path so the example's `tests/fixtures/...` and
    /// the scaffold's `bundle/...` compare equal.
    fn wiring_lines(src: &str) -> Vec<String> {
        src.lines()
            .map(|l| {
                // Strip trailing line comments so e.g. "...; // machine-readable"
                // compares equal across files.
                let l = match l.find("//") {
                    Some(idx) => &l[..idx],
                    None => l,
                };
                // Normalize the include_dir! bundle path: the example points at
                // the committed fixture, the scaffold at its local copy.
                let l = l
                    .replace("tests/fixtures/tax-calc@1.1.0", "tax-calc@1.1.0")
                    .replace("bundle/tax-calc@1.1.0", "tax-calc@1.1.0");
                // Normalize the toolkit import: the example pulls in LocalDirSource
                // (for its --bundle-dir branch); the scaffold does not. Drop the
                // extra import token so the two import lines compare equal.
                l.replace(
                    "{EmbeddedSource, LocalDirSource, WorkbookBuilderExt}",
                    "{EmbeddedSource, WorkbookBuilderExt}",
                )
            })
            .filter(|l| {
                let t = l.trim();
                if t.is_empty() {
                    return false;
                }
                if t.starts_with("//") {
                    return false; // doc / line comments (incl. //! header, /// docs)
                }
                // The example imports LocalDirSource (scaffold does not — it has
                // no --bundle-dir branch); normalize the import line.
                if t.contains("LocalDirSource") {
                    return false;
                }
                // The example's harness-only --bundle-dir branch (the scaffold
                // always serves its embedded bundle, so it has no match arm).
                if t.contains("--bundle-dir") {
                    return false;
                }
                // The example's `match bundle_dir { ... }` selection collapses to
                // the scaffold's direct `try_with_workbook_bundle` call. Filter
                // the match scaffolding lines so the EmbeddedSource arm aligns.
                if t.starts_with("let bundle_dir =") {
                    return false;
                }
                if t.starts_with("let builder = match bundle_dir") {
                    return false;
                }
                if t == "};" {
                    return false; // closing brace of the match expression
                }
                true
            })
            .map(|l| {
                // The example's EmbeddedSource arm is indented inside a match arm
                // ("        None => builder.try_..."); the scaffold's is a plain
                // statement ("    let builder = builder.try_..."). Normalize both
                // to their load-bearing call so the wiring compares equal.
                let t = l.trim();
                if let Some(rest) = t.strip_prefix("None => ") {
                    // example match arm: "None => builder.try_...(...)?,"
                    format!("try:{}", rest.trim_end_matches(','))
                } else if let Some(rest) = t.strip_prefix("let builder = builder.") {
                    // scaffold statement: "let builder = builder.try_...(...)?;"
                    format!("try:builder.{}", rest.trim_end_matches(';'))
                } else {
                    t.to_string()
                }
            })
            .collect()
    }

    #[test]
    fn emitted_main_matches_example_modulo_setup() {
        let emitted = wiring_lines(emitted_main_rs());
        let example = wiring_lines(EXAMPLE_SRC);
        assert_eq!(
            emitted, example,
            "emitted src/main.rs wiring drifted from the canonical \
             workbook_server_http.rs example (modulo //! header, the \
             --bundle-dir/LocalDirSource harness branch, and the include_dir! path)"
        );
    }

    #[test]
    fn emitted_cargo_toml_is_purity_safe() {
        // Render into a scratch dir and read back the emitted Cargo.toml so the
        // assertion exercises the REAL emitter, not a copy of the literal.
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src dir");
        generate_cargo_toml(tmp.path(), "wb_purity_demo").expect("emit Cargo.toml");
        let cargo =
            std::fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read Cargo.toml");

        // Strip `#` comment lines so the prose explaining the purity posture
        // (which legitimately MENTIONS code-mode) does not trip the negative
        // assertion below — only the actual dependency/feature lines matter.
        let effective: String = cargo
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            effective.contains("default-features = false"),
            "emitted Cargo.toml MUST disable toolkit default features (T-95-06): {cargo}"
        );
        assert!(
            effective.contains(r#"features = ["workbook-embedded", "http"]"#),
            "emitted Cargo.toml MUST use the workbook-embedded + http feature set: {cargo}"
        );
        assert!(
            !effective.contains("code-mode"),
            "emitted Cargo.toml MUST NOT enable code-mode (purity gate): {cargo}"
        );
    }

    #[test]
    fn emitted_main_has_embedded_source_wiring() {
        let m = emitted_main_rs();
        for tok in [
            "EmbeddedSource",
            "try_with_workbook_bundle",
            "pmcp_server_toolkit::workbook",
            "include_dir!(\"$CARGO_MANIFEST_DIR/bundle/tax-calc@1.1.0\")",
            "PMCP_WORKBOOK_SERVER_ADDR",
        ] {
            assert!(
                m.contains(tok),
                "emitted main.rs missing wiring token: {tok}"
            );
        }
        // It must NOT name pmcp-workbook-runtime (D-11) nor the LocalDirSource
        // harness branch (the scaffold always serves its embedded bundle).
        assert!(
            !m.contains("pmcp_workbook_runtime") && !m.contains("pmcp-workbook-runtime"),
            "emitted main.rs must import SOLELY from pmcp_server_toolkit::workbook (D-11)"
        );
        assert!(
            !m.contains("LocalDirSource") && !m.contains("--bundle-dir"),
            "the scaffold has no --bundle-dir/LocalDirSource branch (it serves its embedded bundle)"
        );
    }

    #[test]
    fn embedded_bundle_matches_committed_golden() {
        // The in-package embedded bundle (src/templates/workbook_bundle/) must be
        // byte-identical to the committed golden so the scaffold serves the same
        // boot-integrity-verified bundle (Pitfall 5 / T-96-06).
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let golden_root = manifest_dir.join("..").join(GOLDEN_BUNDLE_REL);
        assert!(
            golden_root.is_dir(),
            "committed golden bundle missing at {}",
            golden_root.display()
        );

        // Every committed golden file must exist in the embedded copy with
        // identical bytes.
        for entry in walkdir::WalkDir::new(&golden_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(&golden_root)
                .expect("under golden root");
            let golden_bytes = std::fs::read(entry.path()).expect("read golden file");
            let embedded = EMBEDDED_BUNDLE
                .get_file(rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| panic!("embedded bundle missing {}", rel.display()));
            assert_eq!(
                embedded.contents(),
                golden_bytes.as_slice(),
                "embedded bundle file {} drifted from the committed golden",
                rel.display()
            );
        }

        // The five contract members must all be present in the embedded copy.
        for member in [
            "BUNDLE.lock",
            "cell_map.json",
            "executable.ir.json",
            "layout.json",
            "manifest.json",
        ] {
            assert!(
                EMBEDDED_BUNDLE.get_file(member).is_some(),
                "embedded bundle missing contract member: {member}"
            );
        }
    }

    #[test]
    fn embedded_xlsx_matches_committed_source() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let golden_xlsx = manifest_dir
            .join("..")
            .join("crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx");
        let golden_bytes = std::fs::read(&golden_xlsx).expect("read committed source xlsx");
        assert_eq!(
            EMBEDDED_XLSX, golden_bytes,
            "embedded tax-calc.xlsx drifted from the committed source"
        );
    }

    #[test]
    fn generate_emits_full_file_tree() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src dir");
        generate(tmp.path(), "wb_tree_demo").expect("generate scaffold");

        for rel in [
            "Cargo.toml",
            "src/main.rs",
            "pmcp.toml",
            "workbook/tax-calc.xlsx",
            "bundle/tax-calc@1.1.0/BUNDLE.lock",
            "bundle/tax-calc@1.1.0/cell_map.json",
            "bundle/tax-calc@1.1.0/executable.ir.json",
            "bundle/tax-calc@1.1.0/layout.json",
            "bundle/tax-calc@1.1.0/manifest.json",
        ] {
            assert!(
                tmp.path().join(rel).is_file(),
                "scaffold did not emit expected file: {rel}"
            );
        }
    }

    #[test]
    fn emitted_pmcp_version_matches_workspace_pin() {
        // Codex MEDIUM: the hardcoded PMCP_VERSION must not drift from the
        // workspace-root `pmcp` package version. Parse the root Cargo.toml
        // `[package] version` and compare.
        let parsed: toml::Value =
            toml::from_str(ROOT_CARGO_TOML).expect("parse workspace root Cargo.toml");
        let root_version = parsed
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .expect("root Cargo.toml has [package] version");
        assert_eq!(
            PMCP_VERSION, root_version,
            "the scaffold's hardcoded pmcp version `{PMCP_VERSION}` drifted from the \
             workspace-root pin `{root_version}` — bump PMCP_VERSION in workbook_server.rs"
        );
    }

    #[test]
    fn emitted_toolkit_version_matches_workspace_pin() {
        // ME-01: the hardcoded TOOLKIT_VERSION must not drift from the workspace
        // `pmcp-server-toolkit` package version (publish-item 5, a strong candidate
        // to bump). Parse its Cargo.toml `[package] version` and compare — mirroring
        // the PMCP_VERSION drift guard so a stale toolkit pin fails a test rather
        // than shipping silently in `cargo pmcp new --kind workbook-server` output.
        const TOOLKIT_CARGO_TOML: &str =
            include_str!("../../../crates/pmcp-server-toolkit/Cargo.toml");
        let parsed: toml::Value =
            toml::from_str(TOOLKIT_CARGO_TOML).expect("parse pmcp-server-toolkit Cargo.toml");
        let toolkit_version = parsed
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .expect("pmcp-server-toolkit Cargo.toml has [package] version");
        assert_eq!(
            TOOLKIT_VERSION, toolkit_version,
            "the scaffold's hardcoded pmcp-server-toolkit version `{TOOLKIT_VERSION}` drifted \
             from the workspace pin `{toolkit_version}` — bump TOOLKIT_VERSION in workbook_server.rs"
        );
    }
}
