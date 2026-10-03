//! Fuzz target for the deploy endpoint settings (debug session
//! `cargo-pmcp-deploy-targets`, findings #7 and #8).
//!
//! `[server] mcp_path` and `[gcp] repository` are operator input from
//! `.pmcp/deploy.toml`. The checks must be total (never panic), and what they
//! accept must stay safe where it is used: an accepted `mcp_path` joined onto
//! a service URL yields an endpoint under that URL with no query or fragment,
//! and joining twice changes nothing; an accepted repository id can never be
//! read as a `gcloud` flag nor split the image path.
//!
//! Input: the bytes up to the first NUL are the path / id, the rest the
//! service URL.
//!
//! Run with: `cargo +nightly fuzz run fuzz_mcp_path`
//! Quick smoke: `cargo +nightly fuzz run fuzz_mcp_path -- -max_total_time=30`

#![no_main]

use cargo_pmcp::deployment::google_cloud_run::image::{validate_repository, ImageRegistry};
use cargo_pmcp::deployment::mcp_endpoint::{endpoint_url, join_endpoint, validate_mcp_path};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let (value, base) = text
        .split_once('\0')
        .unwrap_or((text, "https://svc.a.run.app"));

    if validate_mcp_path(value).is_ok() {
        let joined = join_endpoint(base, value);
        assert!(joined.starts_with(base.trim_end_matches('/')));
        assert_eq!(join_endpoint(&joined, value), joined);
        if !base.contains(['?', '#']) {
            assert!(!joined.contains(['?', '#']));
        }
        for target in ["google-cloud-run", "pmcp-run", "aws-lambda"] {
            let _ = endpoint_url(target, Some(value), base);
        }
    }

    if validate_repository(value).is_ok() {
        assert!(!value.starts_with('-'));
        let image = ImageRegistry::for_config(Some(value), "us-central1").image("p", "s");
        assert_eq!(image.matches('/').count(), 3);
    }
});
