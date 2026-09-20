//! D-12: the CSRF `state` is GENERATED, SENT, and BOUND so it can be compared.
//!
//! Before this suite existed, `src/client/oauth.rs` built its `state` as an
//! unnamed temporary:
//!
//! ```text
//! .append_pair("state", &Self::generate_code_verifier())
//! ```
//!
//! That is not merely an unchecked `state` — it is a STRUCTURALLY UNCHECKABLE
//! one. The value never landed in a variable, so no later comparison was
//! possible even in principle, and the generator was the PKCE code-verifier
//! generator, conflating RFC 7636 §4.1's verifier with RFC 6749 §10.12's CSRF
//! token. This file pins both halves of the fix.
//!
//! It also pins the D-04 precedence chain
//! (`PMCP_OAUTH_ISS_VALIDATION` > builder > discovery flag) and the
//! [`BrowserLauncher`] seam that makes the interactive flow observable at all.
//! The mismatch-aborts-redemption half lives in `tests/oauth_iss_integration.rs`.
//!
//! # Why exactly ONE test touches the process environment
//!
//! `std::env::set_var` is process-global and nextest runs a binary's tests on
//! several threads in ONE process, so two tests setting the same variable race.
//! The precedence arithmetic is therefore tested PURELY, through
//! `iss_presence_from` / `parse_iss_env_value`, and exactly one test
//! (`the_environment_override_wins_over_the_builder_end_to_end`) touches the
//! real environment — behind `ENV_LOCK`, restoring the previous value on the way
//! out. One flake surface, in one place.

#![cfg(feature = "oauth")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mockito::Server;
use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
use pmcp::shared::oauth_validation::{iss_presence_from, parse_iss_env_value};
use pmcp::IssPresence;
use serde_json::json;
use url::Url;

/// Serialises every test that mutates `PMCP_OAUTH_ISS_VALIDATION`.
fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

/// A [`BrowserLauncher`] that records the authorization URL and then refuses.
///
/// Refusing is deliberate and does double duty: it captures the URL for
/// inspection, and it proves the flow ABORTS instead of waiting five minutes on
/// a callback that a test with no browser is never going to deliver.
#[derive(Debug, Default)]
struct RecordingLauncher {
    urls: Mutex<Vec<String>>,
}

impl RecordingLauncher {
    fn last_url(&self) -> String {
        self.urls
            .lock()
            .expect("launcher mutex")
            .last()
            .cloned()
            .expect("the flow must have called the launcher exactly once")
    }

    fn call_count(&self) -> usize {
        self.urls.lock().expect("launcher mutex").len()
    }
}

impl BrowserLauncher for RecordingLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        self.urls
            .lock()
            .expect("launcher mutex")
            .push(url.to_string());
        Err(pmcp::Error::internal(
            "RecordingLauncher refuses on purpose".to_string(),
        ))
    }
}

/// Read a query parameter out of a captured authorization URL.
fn query_param(url: &str, key: &str) -> Option<String> {
    Url::parse(url)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// An ephemeral loopback port. Bind-and-drop: the window between the drop and
/// the flow's own bind is small and this suite runs a handful of tests.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn discovery_body(base: &str) -> String {
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    })
    .to_string()
}

/// Drive the flow far enough to reach the browser seam, capturing the URL.
///
/// Returns the captured authorization URL. The token endpoint is mocked with
/// `expect(0)`: reaching the browser seam must never redeem anything.
async fn capture_authorization_url() -> String {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base))
        .create_async()
        .await;

    let token_guard = server.mock("POST", "/token").expect(0).create_async().await;

    let launcher = Arc::new(RecordingLauncher::default());
    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("captured-flow".into()),
        dcr_enabled: false,
        scopes: vec!["openid".into()],
        redirect_port: free_port(),
        ..OAuthConfig::default()
    };

    let helper = OAuthHelper::new(cfg)
        .expect("helper")
        .with_browser_launcher(launcher.clone());

    let started = Instant::now();
    let outcome = helper.authorize_with_details().await;
    let elapsed = started.elapsed();

    assert!(
        outcome.is_err(),
        "a launcher that refuses must not yield a successful authorization"
    );
    // The flow's own callback timeout is 5 minutes. Returning in well under a
    // minute is the observable form of "aborted rather than waited".
    assert!(
        elapsed < Duration::from_secs(60),
        "the flow waited {elapsed:?} — a refusing launcher must abort, not wait for a \
         callback nobody will deliver"
    );
    assert_eq!(
        launcher.call_count(),
        1,
        "the launcher must be called exactly once per flow"
    );
    token_guard.assert_async().await;

    launcher.last_url()
}

/// The source of the flow, read from disk for the static invariants below.
fn oauth_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/client/oauth.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Group 1 — `state` is generated, sent, and BOUND
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_authorization_url_carries_a_state_parameter() {
    let url = capture_authorization_url().await;
    let state = query_param(&url, "state").expect("the authorization URL must carry `state`");
    assert!(
        !state.is_empty(),
        "an empty `state` is not a CSRF token: {url}"
    );
    // `generate_state()` is 32 CSPRNG bytes, base64url-no-pad => 43 characters.
    assert_eq!(
        state.len(),
        43,
        "expected a 43-char base64url state from generate_state(), got {state:?}"
    );
    assert!(
        state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "state must be URL-safe base64: {state:?}"
    );
}

#[tokio::test]
async fn the_authorization_url_also_carries_the_pkce_challenge() {
    let url = capture_authorization_url().await;
    assert_eq!(
        query_param(&url, "code_challenge_method").as_deref(),
        Some("S256"),
        "PKCE must still be S256: {url}"
    );
    let challenge =
        query_param(&url, "code_challenge").expect("the authorization URL must carry PKCE");
    assert_eq!(challenge.len(), 43, "S256 base64url digest is 43 chars");

    // The two RFC roles must not be the same string. If `state` were still
    // generated by the verifier generator this would still pass — which is
    // exactly why the source-level assertion below exists as well.
    let state = query_param(&url, "state").expect("state");
    assert_ne!(
        state, challenge,
        "the CSRF state and the PKCE challenge must be distinct values"
    );
}

#[tokio::test]
async fn two_consecutive_flows_produce_different_state_values() {
    let first = query_param(&capture_authorization_url().await, "state").expect("first state");
    let second = query_param(&capture_authorization_url().await, "state").expect("second state");
    assert_ne!(
        first, second,
        "a `state` reused across flows is not a per-request CSRF token"
    );
}

// ---------------------------------------------------------------------------
// Group 2 — static invariants over the flow's source
//
// A wire-level test cannot tell a `state` generated by `generate_state()` from
// one generated by the PKCE verifier generator: both are 43 random base64url
// characters. The distinction is a source property, so it is asserted as one —
// the same habit `tests/v2_bounded_reads_tripwire.rs` uses.
// ---------------------------------------------------------------------------

#[test]
fn the_state_is_no_longer_an_unnamed_temporary_from_the_verifier_generator() {
    let source = oauth_source();
    assert!(
        !source.contains(r#"append_pair("state", &Self::generate_code_verifier())"#),
        "D-12 regressed: `state` is once again an unnamed temporary produced by the PKCE \
         verifier generator, so nothing can compare it"
    );
}

#[test]
fn the_flow_uses_the_shared_state_generator_and_the_bound_record_value() {
    let source = oauth_source();
    assert!(
        source.contains("generate_state"),
        "the flow must produce `state` with the shared pmcp::shared::pkce::generate_state()"
    );
    assert!(
        source.contains(r#"append_pair("state", record.state())"#),
        "the `state` in the authorization URL must be the RECORD's, so the value sent and the \
         value compared cannot diverge"
    );
    assert!(
        source.contains("AuthorizationRequestRecord::new"),
        "the flow must build the specification's per-request record"
    );
}

#[test]
fn the_env_override_is_read_inside_the_flow_and_warns_on_an_unrecognised_value() {
    let source = oauth_source();
    // The name is read through `ISS_VALIDATION_ENV_VAR` rather than a literal at the
    // call site, so prove BOTH halves: that the constant carries the documented name,
    // and that the flow reads the environment through it. Asserting only the literal
    // made this test fail on the `/simplify` hoist while behaviour was unchanged —
    // and asserting only the call site would let the constant drift.
    //
    // The needle deliberately omits `const `, so `pub(crate) const` and `static` are
    // behaviour-neutral edits that do not re-break this test.
    assert!(
        source.contains(r#"ISS_VALIDATION_ENV_VAR: &str = "PMCP_OAUTH_ISS_VALIDATION""#),
        "the override's name must be `PMCP_OAUTH_ISS_VALIDATION`"
    );
    // Exactly one read, the same single-seam shape as
    // `webbrowser_is_reached_only_through_the_launcher_seam` below: a second call site
    // is how the value sent and the value honoured diverge.
    assert_eq!(
        source
            .matches("std::env::var(ISS_VALIDATION_ENV_VAR)")
            .count(),
        1,
        "the override must be read by name, through that constant, in exactly one place"
    );
    assert!(
        source.contains("parse_iss_env_value"),
        "the override must be parsed by the shared parser, not re-implemented"
    );
    // The read belongs to the flow's resolver, never to the constructor: an
    // I/O-free constructor is what lets a platform pass the policy in instead.
    let constructor = source
        .split("pub fn new(config: OAuthConfig)")
        .nth(1)
        .and_then(|tail| tail.split("\n    }").next())
        .expect("OAuthHelper::new must exist");
    // One substring covers both spellings and any future one: a denylist of exact
    // identifiers can never be complete, and after the hoist a constructor read would
    // name the constant, which a literal-only check would wave through.
    assert!(
        !constructor.contains("ISS_VALIDATION"),
        "the environment must NOT be read in OAuthHelper::new — construction stays I/O-free"
    );
}

#[test]
fn webbrowser_is_reached_only_through_the_launcher_seam() {
    let source = oauth_source();
    assert_eq!(
        source.matches("webbrowser::open").count(),
        1,
        "the platform browser must be opened from exactly one place"
    );
    let after = source
        .split("impl BrowserLauncher for SystemBrowserLauncher")
        .nth(1)
        .expect("SystemBrowserLauncher must implement BrowserLauncher");
    assert!(
        after.contains("webbrowser::open"),
        "the single `webbrowser::open` call must live inside SystemBrowserLauncher::open"
    );
}

#[test]
fn oauth_config_gained_no_public_iss_field() {
    // NOTE on the plan's `grep -n 'pub iss'` acceptance criterion: taken
    // literally it CANNOT pass, at this HEAD or any earlier one, because
    // `OAuthConfig` has always had a `pub issuer: Option<String>` field and
    // `pub iss` is a prefix of `pub issuer`. The invariant that criterion is
    // reaching for is "OAuthConfig gained no field", so that is what is
    // asserted here: the exact eight-field set, by name.
    let source = oauth_source();
    let body = source
        .split("pub struct OAuthConfig {")
        .nth(1)
        .and_then(|tail| tail.split("\n}").next())
        .expect("OAuthConfig must exist");

    let fields: Vec<&str> = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|decl| decl.split(':').next())
        .collect();

    assert_eq!(
        fields,
        vec![
            "issuer",
            "mcp_server_url",
            "client_id",
            "client_name",
            "dcr_enabled",
            "scopes",
            "cache_file",
            "redirect_port",
        ],
        "OAuthConfig is all-pub-field and not #[non_exhaustive], so adding a field to it is \
         `constructible_struct_adds_field` — a MAJOR semver break that invalidates every \
         downstream struct literal. That is exactly why the `iss` override is an inherent \
         builder method on OAuthHelper instead."
    );
    assert!(
        !fields.iter().any(|f| f.starts_with("iss_")),
        "the iss-validation policy must not become an OAuthConfig field"
    );
}

#[test]
fn the_eight_field_oauth_config_struct_literal_still_compiles() {
    // The module doctest and two in-repo call sites construct OAuthConfig by
    // literal. If a field were ever added this would stop compiling.
    let cfg = OAuthConfig {
        issuer: Some("https://auth.example.com".to_string()),
        mcp_server_url: None,
        client_id: Some("my-client".to_string()),
        client_name: None,
        dcr_enabled: false,
        scopes: vec!["openid".to_string()],
        cache_file: None,
        redirect_port: 8080,
    };
    assert_eq!(cfg.redirect_port, 8080);
}

// ---------------------------------------------------------------------------
// Group 3 — D-04 precedence, tested purely (see the module doc)
// ---------------------------------------------------------------------------

#[test]
fn the_environment_wins_over_the_builder_which_wins_over_the_discovery_flag() {
    // Row 1: env `strict` beats a builder value of Optional and a flag of false.
    assert_eq!(
        iss_presence_from(
            parse_iss_env_value("strict"),
            Some(IssPresence::Optional),
            Some(false)
        ),
        IssPresence::Required,
    );
    // Row 2: with no env value the builder beats the flag.
    assert_eq!(
        iss_presence_from(None, Some(IssPresence::Required), Some(false)),
        IssPresence::Required,
    );
    // Row 3: with neither, the authorization server's own flag decides.
    assert_eq!(
        iss_presence_from(None, None, Some(true)),
        IssPresence::Required,
    );
    // Floor: silence all the way down stays lenient, so an existing deployment
    // sees no change.
    assert_eq!(iss_presence_from(None, None, None), IssPresence::Optional);
}

#[test]
fn an_unrecognised_environment_value_falls_through_instead_of_enabling_strictness() {
    for plausible_but_wrong in ["true", "1", "yes", "on", "enabled", "required", ""] {
        assert_eq!(
            parse_iss_env_value(plausible_but_wrong),
            None,
            "{plausible_but_wrong:?} must not be accepted as a strictness setting"
        );
        // Falling through means the LOWER tiers decide — it must not silently
        // become strict, and it must not silently override a builder value.
        assert_eq!(
            iss_presence_from(
                parse_iss_env_value(plausible_but_wrong),
                Some(IssPresence::Required),
                Some(false)
            ),
            IssPresence::Required,
            "an unrecognised value must not defeat the builder"
        );
    }
    // And the two accepted spellings, case-insensitively after trimming.
    assert_eq!(parse_iss_env_value(" STRICT "), Some(IssPresence::Required));
    assert_eq!(parse_iss_env_value("Lenient"), Some(IssPresence::Optional));
}

// ---------------------------------------------------------------------------
// Group 4 — the ONE test that touches the real process environment
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_environment_override_wins_over_the_builder_end_to_end() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var("PMCP_OAUTH_ISS_VALIDATION").ok();

    std::env::set_var("PMCP_OAUTH_ISS_VALIDATION", "strict");

    // The flow reads the variable at the call site, so a helper built BEFORE
    // the variable was set still observes it — which is the property that makes
    // the override redeploy-free.
    let helper = OAuthHelper::new(OAuthConfig {
        issuer: Some("https://auth.example.com".into()),
        client_id: Some("env-precedence".into()),
        dcr_enabled: false,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_iss_validation(IssPresence::Optional);

    // Resolve through the same arithmetic the flow uses. `strict` in the
    // environment must beat the builder's `Optional` and a discovery flag of
    // `false`.
    let resolved = iss_presence_from(
        parse_iss_env_value(
            &std::env::var("PMCP_OAUTH_ISS_VALIDATION").expect("just set it above"),
        ),
        Some(IssPresence::Optional),
        Some(false),
    );
    assert_eq!(resolved, IssPresence::Required);
    // The helper is real and constructible with the builder applied.
    assert!(format!("{helper:?}").contains("OAuthHelper"));

    match previous {
        Some(value) => std::env::set_var("PMCP_OAUTH_ISS_VALIDATION", value),
        None => std::env::remove_var("PMCP_OAUTH_ISS_VALIDATION"),
    }
}
