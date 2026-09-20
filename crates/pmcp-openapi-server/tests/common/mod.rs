//! Helpers shared by this crate's integration-test binaries (Phase 121 D-02).
//!
//! Files under `tests/common/` are NOT compiled as their own test binaries —
//! only files directly under `tests/` are. Each of those is its own crate, so a
//! helper used by more than one of them has to live in a subdirectory module
//! like this one rather than being copy-pasted. Consume it with a bare
//! `mod common;` from a test binary.
//!
//! # Who shares these, and why sharing matters
//!
//! - `tests/parity_replay.rs` — the OAPI-08 london-tube reference-parity
//!   assertion, replayed offline through `wiremock`.
//! - `tests/roundtrip_e2e.rs` (Phase 121) — the PKG-04 round-trip: pack in a
//!   simulated environment A, unpack in a distinct environment B, prove B serves
//!   the same tools once its slots are filled.
//!
//! Both drive the SAME london-tube fixture against the SAME mocked backend
//! shape. If each binary carried its own copy of `mount_london_tube` or of the
//! slot assertions, the two could drift and the round-trip test would be
//! comparing against a backend the parity test no longer describes — the
//! failure the single definition exists to make impossible.
//!
//! The dead-code allow below is deliberate: each test binary uses a DIFFERENT
//! subset of this module, so items exposed here are legitimately unused in one
//! of them, and without the allow the lift would produce dead-code warnings in
//! whichever binary uses the smaller subset.
#![allow(dead_code)]

use pmcp_server_toolkit::ServerConfig;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serializes the tests that point the fixture somewhere through the
/// process-GLOBAL `TFL_BASE_URL` / `TFL_APP_KEY` environment variables.
///
/// Without it, `london_tube_parity_through_real_binary_path` (wiremock URI) and
/// the `#[ignore]`d `parity_live_tfl` (the real TfL URL) race under
/// `--include-ignored` on the default multi-threaded runner: whichever
/// `set_var` lands last wins for BOTH servers' assembly-time resolution, so the
/// "offline" replay can silently target live TfL, or the live test a dead
/// wiremock port. The toolkit's `tests/support::env_lock` exists for exactly
/// this discipline; this is that crate-local twin. The guard is held for the
/// whole test body because the variables must stay stable until `run_serving`
/// has assembled (they are read once, at dispatch time). The variables are
/// deliberately NOT restored afterwards by `parity_replay.rs` — no other test
/// in that binary reads them, and a restore racing a still-running server would
/// be a worse trade. Binaries that DO need restoration use [`EnvVarGuard`].
///
/// Living in `tests/common/` gives each consuming test binary its OWN copy of
/// this static, and that is CORRECT rather than a bug to fix: cargo runs each
/// integration-test binary as its own process, and the process environment is
/// per-process, so two binaries cannot interfere with each other's `TFL_*`
/// variables in the first place. The mutex only ever needs to serialize tests
/// WITHIN one binary. Do not "fix" this into a cross-binary global — no such
/// thing exists at this layer, and reaching for one (a lock file, a shared
/// port) would add failure modes to guard against a hazard that cannot occur.
static TFL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn tfl_env_lock() -> std::sync::MutexGuard<'static, ()> {
    TFL_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Absolute path to the vendored fixtures directory.
///
/// `CARGO_MANIFEST_DIR` expands at compile time to the CRATE root, not to a
/// path relative to this file, so it resolves identically from `tests/common/`
/// as it did from `tests/parity_replay.rs`.
pub fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Absolute path to the published `examples/` directory (ships with the crate;
/// `tests/` is excluded from the tarball but `examples/` is NOT — Cargo.toml:14).
pub fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// PKG-03: BOTH london-tube copies declare exactly the same three config slots
/// — the endpoint, the api-key credential and the auth mode — and hold the
/// `${TFL_BASE_URL}` endpoint placeholder rather than a baked literal.
///
/// Asserting it in one helper is what keeps the fixture and the pointable
/// example from drifting apart: a future edit that silently drops the block from
/// either copy, or re-bakes the endpoint, fails here rather than at pack time.
pub fn assert_london_tube_config_slots(cfg: &ServerConfig, config_text: &str, label: &str) {
    use pmcp_server_toolkit::config::ConfigSlotKind;

    assert_eq!(
        cfg.config_slots.len(),
        3,
        "{label} declares exactly three PKG-03 config slots: {:?}",
        cfg.config_slots
    );

    let endpoint = cfg
        .config_slots
        .iter()
        .find(|s| s.key == "backend.base_url")
        .unwrap_or_else(|| panic!("{label} declares the backend.base_url endpoint slot"));
    assert_eq!(endpoint.kind, ConfigSlotKind::Endpoint);
    assert_eq!(endpoint.name, "TFL_BASE_URL");
    assert_eq!(
        endpoint.tested_value.as_deref(),
        Some("https://api.tfl.gov.uk"),
        "{label} records the tested endpoint as DATA, not as a baked base_url"
    );

    let secret = cfg
        .config_slots
        .iter()
        .find(|s| s.key == "backend.auth.query_params.app_key")
        .unwrap_or_else(|| panic!("{label} declares the app_key credential slot"));
    assert_eq!(secret.kind, ConfigSlotKind::Secret);
    assert_eq!(secret.name, "TFL_APP_KEY");
    assert!(
        secret.tested_value.is_none(),
        "{label}'s identity-bearing slot structurally carries no tested_value"
    );

    let auth_mode = cfg
        .config_slots
        .iter()
        .find(|s| s.key == "backend.auth.type")
        .unwrap_or_else(|| panic!("{label} declares the auth-mode slot"));
    assert_eq!(auth_mode.kind, ConfigSlotKind::AuthMode);
    assert_eq!(auth_mode.tested_value.as_deref(), Some("api_key"));

    // D-04: the endpoint travels as a placeholder, so the package digest is
    // environment-independent. The resolved literal must NOT sit on a
    // `base_url` assignment (it survives only as the slot's tested_value).
    let backend = cfg
        .backend
        .as_ref()
        .unwrap_or_else(|| panic!("{label} has a [backend] section"));
    assert_eq!(
        backend.base_url, "${TFL_BASE_URL}",
        "{label} holds the endpoint PLACEHOLDER, not a baked literal"
    );
    assert!(
        !config_text.contains(r#"base_url = "https://api.tfl.gov.uk""#),
        "{label} must not re-bake the endpoint onto a base_url assignment"
    );
}

/// The Code Mode showcase surface that BOTH the enriched fixture and the
/// pointable example must ship (P901-FIXTURE / P901-EXAMPLE): the three context
/// resource URIs + the `start_code_mode` prompt. Asserting it in one place keeps
/// the two configs from drifting apart on the showcase surface. `label` names the
/// config under test so a failure points at the right file.
pub fn assert_london_tube_code_mode_surface(cfg: &ServerConfig, label: &str) {
    let resource_uris: Vec<&str> = cfg.resources.iter().map(|r| r.uri.as_str()).collect();
    for uri in [
        "docs://london-tube/schema",
        "docs://london-tube/examples",
        "code-mode://learnings",
    ] {
        assert!(
            resource_uris.contains(&uri),
            "{label} ships the {uri} resource: {resource_uris:?}"
        );
    }
    assert!(
        cfg.prompts.iter().any(|p| p.name == "start_code_mode"),
        "{label} ships the start_code_mode prompt: {:?}",
        cfg.prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

/// The dummy api key the parity test resolves `${TFL_APP_KEY}` to. The wiremock
/// backend REQUIRES `app_key=dummy` on every request — proving BOTH the api_key
/// query-param outgoing-auth path (D-04) AND that `${TFL_APP_KEY}` was RESOLVED
/// (not sent as the literal `${...}`, T-90-07-04).
pub const DUMMY_APP_KEY: &str = "dummy";

/// Mount the london-tube backend responses on the wiremock server, REQUIRING
/// the `app_key=<app_key>` query param on every matcher (the secret-expansion +
/// api_key query-param proof). Victoria is disrupted (statusSeverity 6 < 10),
/// Central is healthy (10); the per-line `/Line/victoria/Disruption` returns
/// "Severe delays".
///
/// # Why `app_key` is a parameter (D-12 / RESEARCH CF-6)
///
/// The round-trip E2E gives environments A and B their OWN backend on their OWN
/// port with a DIFFERENT credential, so that SC1's "different endpoint and
/// credential values" is real rather than cosmetic. A matcher hardcoded to
/// [`DUMMY_APP_KEY`] would 404 every one of B's backend calls, turning SC3's
/// scenario replay red for a reason that has nothing to do with parity. Callers
/// that want the historical behaviour pass [`DUMMY_APP_KEY`] explicitly.
pub async fn mount_london_tube(server: &MockServer, app_key: &str) {
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .and(query_param("app_key", app_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "victoria", "name": "Victoria", "lineStatuses": [ { "statusSeverity": 6, "statusSeverityDescription": "Severe Delays" } ] },
            { "id": "central", "name": "Central", "lineStatuses": [ { "statusSeverity": 10, "statusSeverityDescription": "Good Service" } ] }
        ])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Line/victoria/Disruption"))
        .and(query_param("app_key", app_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "category": "RealTime",
            "description": "Severe delays due to an earlier signal failure."
        })))
        .mount(server)
        .await;
}

/// RAII guard: set (or remove) an env var, restore the prior value on drop —
/// including when an assertion panics mid-test.
///
/// A trailing `remove_var`/`set_var` cleanup line at the end of a test body is
/// SKIPPED when an earlier assertion panics, and the variable then leaks into
/// every later test in the SAME binary — so a later test silently inherits a
/// value it never asked for and may pass for the wrong reason (a green run
/// against the wrong endpoint). Restoration in `Drop` is what makes the cleanup
/// survive a panicking body, which is the case a manual trailing line always
/// misses. Modelled on the same-crate analog at `src/dispatch.rs:163-196` and
/// the toolkit's `tests/support::EnvVarGuard`.
///
/// This guard and [`tfl_env_lock`] solve two DIFFERENT problems and are held
/// together, not one instead of the other: the lock prevents CONCURRENT tests
/// from interleaving their writes, while this guard prevents SEQUENTIAL
/// leakage.
///
/// `parity_replay.rs` deliberately does NOT use this — its no-restore behaviour
/// is documented on [`tfl_env_lock`] and is out of scope for the D-02 lift.
///
/// # Example
///
/// ```ignore
/// let _lock = common::tfl_env_lock();
/// let _guard = common::EnvVarGuard::unset("TFL_BASE_URL");
/// // ... assertions that may panic ...
/// // TFL_BASE_URL is restored to its prior value (or left unset) here.
/// ```
pub struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    /// Capture `key`'s current value, then set it to `value`. The captured
    /// value is restored on drop.
    pub fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }

    /// Capture `key`'s current value, then REMOVE it. The captured value is
    /// restored on drop — the "prove the unset path" companion to [`Self::set`].
    pub fn unset(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // BOTH branches matter. A `Drop` that only removes would turn a
        // previously-SET variable into an unset one, which is a different leak
        // rather than a fix.
        match self.previous.take() {
            Some(old) => std::env::set_var(self.key, old),
            None => std::env::remove_var(self.key),
        }
    }
}
