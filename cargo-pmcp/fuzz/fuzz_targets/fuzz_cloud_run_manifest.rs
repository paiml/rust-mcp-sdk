//! Fuzz target for the Cloud Run absolute-path guard's manifest scan (debug
//! session `cargo-pmcp-deploy-targets`, finding #9).
//!
//! `cargo pmcp deploy --target google-cloud-run` parses every `Cargo.toml`
//! the Docker build loads and refuses absolute `path` dependencies. The
//! manifests are operator input, so the scan must be total: arbitrary bytes
//! either fail to parse or yield a finding list, never a panic. On a
//! successful parse it also checks the scan's own invariants: every reported
//! dependency carries the `path` it was found with, and `is_host_absolute`
//! is total over those paths.
//!
//! Run with: `cargo +nightly fuzz run fuzz_cloud_run_manifest`
//! Quick smoke: `cargo +nightly fuzz run fuzz_cloud_run_manifest -- -max_total_time=30`

#![no_main]

use cargo_pmcp::deployment::google_cloud_run::manifest::{is_host_absolute, manifest_paths};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(paths) = manifest_paths(text) {
        for dependency in &paths.path_dependencies {
            assert!(!dependency.table.is_empty());
            let _ = is_host_absolute(&dependency.path);
            let _ = dependency.to_string();
        }
        let _ = (paths.members.len(), paths.exclude.len());
    }
});
