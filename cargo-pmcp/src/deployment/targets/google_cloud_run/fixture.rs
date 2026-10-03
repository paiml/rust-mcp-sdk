//! Test fixtures for the Cloud Run target (debug session
//! `cargo-pmcp-deploy-targets`, PR-C).
//!
//! Real crates on disk, so `cargo metadata --no-deps` (no network) sees the
//! same targets it would in an operator's project.

use std::path::Path;

/// The crate shape of the field report's acceptance test (forecast-coach):
/// one package named `*-lambda`, a `lambda_http` dependency, a `[workspace]`
/// table, and an absolute `[patch]` path that is commented out.
pub const X_LAMBDA_EXTRA: &str = r#"[dependencies]
lambda_http = "0.13"

[workspace]

[patch.crates-io]
# aprender-forecast = { path = "/Users/guy/src/aprender/forecast" }
"#;

/// A `Cargo.toml` for package `package` with one `[[bin]]` per name in
/// `bins` (at `src/bin/<name>.rs`), followed by `extra`.
#[must_use]
pub fn cargo_toml(package: &str, bins: &[&str], extra: &str) -> String {
    let mut text =
        format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n");
    for bin in bins {
        text.push_str(&format!(
            "[[bin]]\nname = \"{bin}\"\npath = \"src/bin/{bin}.rs\"\n\n"
        ));
    }
    text.push_str(extra);
    text
}

/// Write a crate at `root`: `Cargo.toml` from [`cargo_toml`] plus a trivial
/// `src/bin/<name>.rs` per binary.
///
/// # Panics
///
/// Panics when a file cannot be written.
pub fn write_crate(root: &Path, package: &str, bins: &[&str], extra: &str) {
    std::fs::create_dir_all(root.join("src/bin")).expect("mkdir src/bin");
    std::fs::write(root.join("Cargo.toml"), cargo_toml(package, bins, extra)).expect("Cargo.toml");
    for bin in bins {
        std::fs::write(root.join(format!("src/bin/{bin}.rs")), "fn main() {}\n").expect("bin");
    }
}

/// The acceptance fixture: package `x-lambda`, bins `bootstrap` + `serve`.
///
/// # Panics
///
/// Panics when a file cannot be written.
pub fn write_x_lambda(root: &Path) {
    write_crate(root, "x-lambda", &["bootstrap", "serve"], X_LAMBDA_EXTRA);
}

/// forecast-coach's real shape: `bootstrap`, `local` and `serve` (two
/// candidates for the server binary).
///
/// # Panics
///
/// Panics when a file cannot be written.
pub fn write_x_lambda_with_local(root: &Path) {
    write_crate(
        root,
        "x-lambda",
        &["bootstrap", "local", "serve"],
        X_LAMBDA_EXTRA,
    );
}
