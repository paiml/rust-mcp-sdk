//! Integration tests for `OAuthHelper`'s SEP-2352 credential-store wiring.
//!
//! The subject is the WIRING, not the store: `116-05` already proved that a
//! `(issuer, account, server)` key keeps both collision classes apart, on the
//! live path, the migration path and the trait level. What is proved here is
//! that `OAuthHelper` actually CONSTRUCTS that key — with all three components,
//! the third derived through `normalize_server_key` — and persists through
//! `save_with_issuer` rather than through the flat, issuer-less
//! `~/.pmcp/oauth-tokens.json` it used to write.
//!
//! # Why several rows SEED the store rather than driving a second flow
//!
//! Absent an explicitly-configured issuer, `pmcp` derives the authorization
//! server from the MCP base URL directly (`get_metadata_with_extras` ->
//! `discover_metadata_with_extras`), and RFC 8414 section 3.3 anchoring (116-07)
//! then requires the fetched document to declare exactly that issuer. So two
//! MCP servers at two different origins resolve two different issuers on the
//! DERIVED path.
//!
//! That is no longer the only path. Issue #368 made an explicitly-configured
//! issuer outrank a derived one, so two MCP server URLs sharing one
//! `MCP_OAUTH_ISSUER` now DO resolve the same issuer, which makes the D-116-R1
//! collision reachable through the live flow — contrary to what this header
//! claimed while nothing ran it. Seeding is retained because it remains the
//! sharper assertion (see below), not because driving it is impossible; a live
//! two-URL/one-issuer row would be a legitimate addition. RFC 9728 Protected
//! Resource Metadata is still unimplemented and DEFERRED by owner decision
//! (2026-08-02, DEF-116-01).
//!
//! Seeding the second server's entry directly is therefore not a shortcut, it is
//! the only way to build the collision at all — and it makes the assertion
//! SHARPER, because the two entries then differ in NOTHING except the server
//! component. Under the two-part key this phase replaced, they would be one
//! entry.
//!
//! # Feature gate
//!
//! `OAuthHelper` lives behind `oauth`, which `full` does NOT contain, so
//! `make quality-gate` runs **none** of this file. See `D-116-LINT-OAUTH`.

#![cfg(feature = "oauth")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mockito::{Matcher, Mock, Server, ServerGuard};
use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
use pmcp::shared::credential_store::normalize_server_key;
use pmcp::{
    CredentialKey, CredentialStore, CredentialStoreAdmin, InMemoryCredentialStore,
    StoredCredentials,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// An ephemeral loopback port, so concurrently-running rows cannot collide on
/// the callback listener.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Absolute Unix seconds, for expiry fixtures.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// A discovery document that declares `base` as its own issuer, which RFC 8414
/// section 3.3 anchoring requires.
fn discovery_body(base: &str, with_registration: bool) -> String {
    let mut doc = json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": ["openid", "mcp:read"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_registration {
        doc["registration_endpoint"] = json!(format!("{base}/register"));
    }
    doc.to_string()
}

/// A mock authorization server that answers discovery, the token endpoint and
/// (optionally) dynamic client registration.
///
/// The guard AND the mocks are RETURNED, never dropped inline: dropping a
/// `ServerGuard` stops the server and dropping a `Mock` removes it, so a row
/// that let either go would be asserting against a dead endpoint.
async fn authorization_server(with_registration: bool) -> (ServerGuard, Vec<Mock>, String) {
    let mut server = Server::new_async().await;
    let base = server.url();
    let mut mocks = Vec::new();

    mocks.push(
        server
            .mock("GET", "/.well-known/openid-configuration")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_body(&base, with_registration))
            .expect_at_least(0)
            .create_async()
            .await,
    );

    mocks.push(
        server
            .mock("POST", "/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "access_token": "fresh-access-token",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "refresh_token": "fresh-refresh-token",
                    "scope": "openid mcp:read",
                })
                .to_string(),
            )
            .expect_at_least(0)
            .create_async()
            .await,
    );

    if with_registration {
        mocks.push(
            server
                .mock("POST", "/register")
                .match_body(Matcher::PartialJsonString(
                    json!({ "application_type": "native" }).to_string(),
                ))
                .with_status(201)
                .with_header("content-type", "application/json")
                .with_body(json!({ "client_id": "dcr-issued-id" }).to_string())
                .expect_at_least(0)
                .create_async()
                .await,
        );
    }

    (server, mocks, base)
}

/// A [`BrowserLauncher`] that COMPLETES the flow — it lifts `state` out of the
/// authorization URL and delivers a legitimate callback to the loopback
/// listener the flow already bound — and COUNTS how many times it was asked to.
///
/// The count is the observable behind "no browser flow was started": a cache hit
/// and a refused authorization-server substitution must both leave it at zero.
#[derive(Debug)]
struct CountingCallbackLauncher {
    port: u16,
    opened: Arc<AtomicUsize>,
}

impl CountingCallbackLauncher {
    fn new(port: u16) -> (Arc<Self>, Arc<AtomicUsize>) {
        let opened = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Self {
                port,
                opened: opened.clone(),
            }),
            opened,
        )
    }
}

impl BrowserLauncher for CountingCallbackLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        self.opened.fetch_add(1, Ordering::SeqCst);

        let state = Url::parse(url)
            .ok()
            .and_then(|parsed| {
                parsed
                    .query_pairs()
                    .find(|(key, _)| key == "state")
                    .map(|(_, value)| value.into_owned())
            })
            .unwrap_or_default();

        let port = self.port;
        tokio::spawn(async move {
            let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await else {
                return;
            };
            let request = format!(
                "GET /callback?code=granted-code&state={state} HTTP/1.1\r\n\
                 Host: 127.0.0.1\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(request.as_bytes()).await.is_err() {
                return;
            }
            let _ = stream.flush().await;
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response).await;
        });

        Ok(())
    }
}

/// Build a helper wired to `store`, driving its callback through a counting
/// launcher.
fn helper_for(
    base: &str,
    store: &Arc<dyn CredentialStore>,
    client_id: Option<&str>,
) -> (OAuthHelper, Arc<AtomicUsize>) {
    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.to_string()),
        client_id: client_id.map(str::to_string),
        dcr_enabled: client_id.is_none(),
        scopes: vec!["openid".to_string()],
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    (helper, opened)
}

/// Let a spawned callback-driving task finish reading before a mock server is
/// dropped, so nextest does not report the reader as leaky.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

/// Captures WARN-level event messages, so "a warning fired, and it named X" is
/// ASSERTED rather than assumed.
///
/// Without this, the D-18 warn-and-proceed path would be tested only by its side
/// effect (the flow proceeded), which a version that never warned at all would
/// also satisfy — the same shape as 116-10's "a presence assertion is not a
/// detector" finding.
#[derive(Debug, Default)]
struct WarnCapture {
    messages: Arc<Mutex<Vec<String>>>,
}

/// Pulls the formatted `message` field out of a `tracing` event.
struct MessageVisitor<'a>(&'a mut Vec<String>);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push(format!("{value:?}"));
        }
    }
}

impl tracing::Subscriber for WarnCapture {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        *metadata.level() <= tracing::Level::WARN
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut held = self.messages.lock().expect("captured warnings");
        event.record(&mut MessageVisitor(&mut held));
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Warnings that mention an authorization-server substitution, filtered out of
/// everything else this module warns about (an unregistered redirect URI, an
/// expired token, a discarded legacy cache).
fn substitution_warnings(captured: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    captured
        .lock()
        .expect("captured warnings")
        .iter()
        .filter(|message| message.contains("authorization server"))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Group A — the three-part key
// ---------------------------------------------------------------------------

/// A completed flow lands under `(discovered issuer, account scope, normalized
/// MCP server)`, and the record carries everything a later refresh needs.
#[tokio::test]
async fn a_completed_flow_stores_credentials_under_the_three_part_key() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));

    let result = helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    assert_eq!(opened.load(Ordering::SeqCst), 1, "one browser open");
    assert_eq!(result.access_token, "fresh-access-token");

    let expected = CredentialKey::new(
        &base,
        "",
        normalize_server_key(&base).expect("a normalizable server URL"),
    );
    let stored = store
        .load(&expected)
        .await
        .expect("a readable store")
        .expect("credentials under the three-part key");

    assert_eq!(stored.access_token(), "fresh-access-token");
    assert_eq!(stored.refresh_token(), Some("fresh-refresh-token"));
    assert_eq!(stored.client_id(), "preset-client");
    assert_eq!(stored.granted_scopes(), ["openid", "mcp:read"]);
    assert!(
        stored.expires_at().is_some_and(|at| at > unix_now()),
        "expires_in must become an absolute expiry in the future"
    );
}

/// The server component goes through `normalize_server_key`, so a path and a
/// trailing slash on the configured MCP server URL do not create a second
/// login.
#[tokio::test]
async fn the_server_component_is_normalized_so_a_path_does_not_fork_the_login() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let port = free_port();
    let (launcher, _opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        // A path AND a trailing slash — both dropped by `normalize_server_key`.
        mcp_server_url: Some(format!("{base}/mcp/")),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    let key = CredentialKey::new(&base, "", normalize_server_key(&base).expect("normalized"));
    assert!(
        store.load(&key).await.expect("readable").is_some(),
        "the path-and-slash form must key to the same normalized server"
    );
}

/// A second call with the same issuer, account and server reads the cache and
/// opens no browser.
#[tokio::test]
async fn a_second_call_with_the_same_key_hits_the_cache_and_opens_no_browser() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (first, first_opened) = helper_for(&base, &store, Some("preset-client"));
    let token = first.get_access_token().await.expect("a full flow");
    settle().await;
    assert_eq!(token, "fresh-access-token");
    assert_eq!(first_opened.load(Ordering::SeqCst), 1);

    let (second, second_opened) = helper_for(&base, &store, Some("preset-client"));
    let cached = second.get_access_token().await.expect("a cache hit");
    assert_eq!(cached, "fresh-access-token");
    assert_eq!(
        second_opened.load(Ordering::SeqCst),
        0,
        "a cache hit must not start an interactive flow"
    );
}

// ---------------------------------------------------------------------------
// Group B — the two collision classes
// ---------------------------------------------------------------------------

/// SEP-2352: credentials issued by one authorization server are not reachable
/// from another. The specification's MUST is satisfied by the KEY SHAPE — there
/// is no enforcement branch anywhere for this test to exercise.
///
/// The seeded entry deliberately carries NO issuer RECORD (`save`, not
/// `save_with_issuer`), so this row exercises the cache miss alone and does not
/// also trip D-18's substitution detection, which has its own rows.
#[tokio::test]
async fn credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");

    let foreign = CredentialKey::new("https://other-as.example", "", &server_key);
    store
        .save(
            &foreign,
            &StoredCredentials::new("CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER", "foreign-client")
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding the foreign entry");

    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));
    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(
        token, "CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER",
        "a token issued by a different authorization server must never be returned"
    );
    assert_eq!(token, "fresh-access-token");
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "the miss must produce a real flow, not a silent reuse"
    );

    // The foreign entry is untouched — a miss is not a delete.
    let survivor = store
        .load(&foreign)
        .await
        .expect("readable")
        .expect("the foreign entry survives");
    assert_eq!(
        survivor.access_token(),
        "CANARY-FROM-ANOTHER-AUTHORIZATION-SERVER"
    );
}

/// **D-116-R1 — this is the load-bearing row of this plan.**
///
/// Two MCP servers behind ONE authorization server, under ONE account, hold
/// different dynamic registrations, different client IDs and different granted
/// scopes. The two-part `(issuer, account)` key this phase replaced collapses
/// them into a single entry, so whichever authenticated last silently
/// overwrites the other and a logout on one deletes the other's credentials.
///
/// The assertion is on the credential CONTENTS, not merely on the key count: a
/// two-part key produces "1 != 2" without showing WHAT was lost, and what was
/// lost is the whole point.
///
/// The second server's entry is seeded rather than driven — see this file's
/// module documentation for why that is the only way to build the collision
/// while RFC 9728 is deferred.
#[tokio::test]
async fn two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let server_a = normalize_server_key(&base).expect("normalized");
    let server_b = normalize_server_key("https://other-mcp.example").expect("normalized");
    assert_ne!(server_a, server_b);

    // Server B: same issuer, same (empty) account, different server.
    let key_b = CredentialKey::new(&base, "", &server_b);
    store
        .save(
            &key_b,
            &StoredCredentials::new("CANARY-BELONGING-TO-SERVER-B", "server-b-client")
                .with_granted_scopes(["mcp:write"])
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding server B");

    let (helper, opened) = helper_for(&base, &store, Some("server-a-client"));
    let token = helper.get_access_token().await.expect("a full flow for A");
    settle().await;

    // (a) A's load did NOT see B's credentials.
    assert_ne!(token, "CANARY-BELONGING-TO-SERVER-B");
    assert_eq!(token, "fresh-access-token");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // (b) Two distinct keys now exist, differing ONLY in the server component.
    let key_a = CredentialKey::new(&base, "", &server_a);
    let a = store
        .load(&key_a)
        .await
        .expect("readable")
        .expect("server A's credentials");
    let b = store
        .load(&key_b)
        .await
        .expect("readable")
        .expect("server B's credentials must survive A's write");
    assert_eq!(a.access_token(), "fresh-access-token");
    assert_eq!(a.client_id(), "server-a-client");
    assert_eq!(b.access_token(), "CANARY-BELONGING-TO-SERVER-B");
    assert_eq!(b.client_id(), "server-b-client");
    assert_eq!(b.granted_scopes(), ["mcp:write"]);
    assert_eq!(key_a.issuer(), key_b.issuer());
    assert_eq!(key_a.account(), key_b.account());
    assert_ne!(key_a.server(), key_b.server());

    // (c) Deleting one leaves the other intact.
    store.delete(&key_a).await.expect("deleting A");
    assert!(store.load(&key_a).await.expect("readable").is_none());
    let survivor = store
        .load(&key_b)
        .await
        .expect("readable")
        .expect("B survives A's logout");
    assert_eq!(survivor.access_token(), "CANARY-BELONGING-TO-SERVER-B");
}

/// The account scope is the third axis. A helper scoped to one account must not
/// reach another's credentials, and the default scope is the empty string.
#[tokio::test]
async fn a_different_account_scope_is_a_cache_miss_and_stores_its_own_entry() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");

    let other_account = CredentialKey::new(&base, "cognito-sub-123", &server_key);
    store
        .save(
            &other_account,
            &StoredCredentials::new("CANARY-FOR-ANOTHER-ACCOUNT", "other-account-client")
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding the other account");

    // Default scope — the single-user CLI case.
    let (helper, _opened) = helper_for(&base, &store, Some("preset-client"));
    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;
    assert_ne!(token, "CANARY-FOR-ANOTHER-ACCOUNT");
    assert!(store
        .load(&CredentialKey::new(&base, "", &server_key))
        .await
        .expect("readable")
        .is_some());

    // A helper scoped to that account stores under it.
    let port = free_port();
    let (launcher, _) = CountingCallbackLauncher::new(port);
    let scoped = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("scoped-client".to_string()),
        dcr_enabled: false,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone())
    .with_account_scope("cognito-sub-456");

    scoped.authorize_with_details().await.expect("a full flow");
    settle().await;

    let scoped_key = CredentialKey::new(&base, "cognito-sub-456", &server_key);
    assert_eq!(
        store
            .load(&scoped_key)
            .await
            .expect("readable")
            .expect("the scoped entry")
            .client_id(),
        "scoped-client"
    );
    // And the seeded account is still exactly as it was.
    assert_eq!(
        store
            .load(&other_account)
            .await
            .expect("readable")
            .expect("untouched")
            .access_token(),
        "CANARY-FOR-ANOTHER-ACCOUNT"
    );
}

// ---------------------------------------------------------------------------
// Group C — D-17, the legacy issuer-less cache
// ---------------------------------------------------------------------------

/// The flat `oauth-tokens.json` this module used to write records NO issuer, so
/// it cannot be re-keyed without GUESSING which authorization server issued it —
/// which is exactly what SEP-2352 forbids. It is never opened for reading, and
/// it is left on disk for the user to delete.
///
/// The planted token is distinctive so that "never read" is asserted by its
/// ABSENCE from every result, not by the presence of something else.
#[tokio::test]
async fn the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    let planted = json!({
        "access_token": "CANARY-FROM-THE-LEGACY-FLAT-CACHE",
        "refresh_token": "CANARY-LEGACY-REFRESH",
        "expires_at": unix_now() + 9_000,
        "scopes": ["openid"],
    })
    .to_string();
    std::fs::write(&legacy, &planted).expect("planting the legacy cache");

    let (_server, _mocks, base) = authorization_server(false).await;
    let concrete = Arc::new(InMemoryCredentialStore::new());
    let store: Arc<dyn CredentialStore> = concrete.clone();

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(
        token, "CANARY-FROM-THE-LEGACY-FLAT-CACHE",
        "the issuer-less cache must never be read as credentials"
    );
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "discarding the legacy cache means exactly one forced re-login"
    );

    // The canary reached NOTHING in the new store.
    let listed = concrete.list_keys().await.expect("listable");
    assert!(!listed.is_empty(), "the flow must have stored something");
    for key in listed {
        let record = store
            .load(&key)
            .await
            .expect("readable")
            .expect("a listed key resolves");
        assert_ne!(record.access_token(), "CANARY-FROM-THE-LEGACY-FLAT-CACHE");
        assert_ne!(record.refresh_token(), Some("CANARY-LEGACY-REFRESH"));
    }

    // Left in place, byte for byte — not deleted, not renamed, not rewritten.
    let after = std::fs::read_to_string(&legacy).expect("the legacy file still exists");
    assert_eq!(after, planted, "the legacy file must be left for the user");
}

/// The DEFAULT (uninjected) store lives BESIDE the legacy file, never on top of
/// it.
///
/// This row exists because the row above cannot see the difference: it injects a
/// store, so it never exercises default resolution at all. Pointing the
/// issuer-keyed store at `cache_file` itself would both read the legacy document
/// — which D-17 forbids — and overwrite it, when the whole instruction is to
/// leave it for the user. Measured: with the store pointed at the legacy file,
/// the row above still PASSES and this one fails.
#[tokio::test]
async fn the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    let planted = json!({
        "access_token": "CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE",
        "refresh_token": "CANARY-DEFAULT-PATH-REFRESH",
        "expires_at": unix_now() + 9_000,
        "scopes": ["openid"],
    })
    .to_string();
    std::fs::write(&legacy, &planted).expect("planting the legacy cache");

    let (_server, _mocks, base) = authorization_server(false).await;

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    // NO injected store — this is the default-resolution path.
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let token = helper.get_access_token().await.expect("a full flow");
    settle().await;

    assert_ne!(token, "CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // The legacy file is byte-identical: not read, not rewritten, not removed.
    assert_eq!(
        std::fs::read_to_string(&legacy).expect("still there"),
        planted,
        "the default store must not write over the legacy flat cache"
    );

    // And the issuer-keyed document appeared BESIDE it, carrying the new token.
    let store_path = dir.path().join("oauth-cache.json");
    let document = std::fs::read_to_string(&store_path)
        .expect("the issuer-keyed store must live beside the legacy file");
    assert!(document.contains("fresh-access-token"));
    assert!(!document.contains("CANARY-FROM-THE-DEFAULT-PATH-LEGACY-CACHE"));

    // A second helper over the same directory reads that document back.
    let port2 = free_port();
    let (launcher2, opened2) = CountingCallbackLauncher::new(port2);
    let second = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port2,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher2);
    assert_eq!(
        second.get_access_token().await.expect("a cache hit"),
        "fresh-access-token"
    );
    assert_eq!(opened2.load(Ordering::SeqCst), 0);
}

/// An injected store REPLACES the file store entirely: the whole flow touches no
/// file at all, which is what makes the helper usable in an environment with no
/// home directory.
#[tokio::test]
async fn an_injected_store_makes_the_flow_touch_no_file_at_all() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let legacy = dir.path().join("oauth-tokens.json");
    std::fs::write(&legacy, "{}").expect("planting");

    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let port = free_port();
    let (launcher, _opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(legacy.clone()),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readable dir")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "an injected store must create nothing on disk; found {entries:?}"
    );
    assert_eq!(entries[0], "oauth-tokens.json");
}

/// Construction performs no filesystem access — the store, and therefore any
/// home-directory resolution, happens on FIRST USE.
#[tokio::test]
async fn constructing_a_helper_touches_no_filesystem() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let missing = dir.path().join("a").join("b").join("oauth-tokens.json");

    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some("https://mcp.example".to_string()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: Some(missing.clone()),
        ..OAuthConfig::default()
    })
    .expect("helper");

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readable dir")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert!(
        entries.is_empty(),
        "OAuthHelper::new must not create anything; found {entries:?}"
    );
    assert!(!missing.exists());
    drop(helper);
}

/// With no `cache_file` and no injected store the helper persists NOTHING, which
/// is what `cargo pmcp auth login --no-cache` has always meant
/// (`cargo-pmcp/src/commands/auth.rs`: `no_cache` sets `cache_file` to `None`).
///
/// The observable is that a completed flow succeeds and creates no file in the
/// only directory it could plausibly reach.
#[tokio::test]
async fn no_cache_file_and_no_injected_store_persists_nothing() {
    let (_server, _mocks, base) = authorization_server(false).await;

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: None,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let result = helper.authorize_with_details().await.expect("a full flow");
    settle().await;
    assert_eq!(result.access_token, "fresh-access-token");
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    // A second helper over the same configuration has nothing to read, so it
    // runs the interactive flow again rather than reusing anything.
    let port2 = free_port();
    let (launcher2, opened2) = CountingCallbackLauncher::new(port2);
    let second = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-client".to_string()),
        dcr_enabled: false,
        cache_file: None,
        redirect_port: port2,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher2);
    second.get_access_token().await.expect("a second full flow");
    settle().await;
    assert_eq!(
        opened2.load(Ordering::SeqCst),
        1,
        "nothing was persisted, so nothing can be reused"
    );
}

// ---------------------------------------------------------------------------
// Group D — what the record carries for 116-12
// ---------------------------------------------------------------------------

/// A DCR flow stores the DCR-ISSUED `client_id` — not the `None` that lives in
/// `config.client_id` — together with the `application_type` the authorization
/// server registered. Both are what makes SEP-2352's "MUST re-register with the
/// new authorization server" automatic: a client id issued by AS-A is stored
/// under AS-A's key and is unreachable from AS-B.
#[tokio::test]
async fn a_dcr_flow_stores_the_issued_client_id_and_the_registered_application_type() {
    let (_server, _mocks, base) = authorization_server(true).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (helper, _opened) = helper_for(&base, &store, None);
    let result = helper.authorize_with_details().await.expect("a DCR flow");
    settle().await;
    assert_eq!(result.client_id, "dcr-issued-id");

    let key = CredentialKey::new(&base, "", normalize_server_key(&base).expect("normalized"));
    let stored = store
        .load(&key)
        .await
        .expect("readable")
        .expect("the DCR record");
    assert_eq!(stored.client_id(), "dcr-issued-id");
    assert_eq!(
        stored.registered_application_type(),
        Some("native"),
        "the registered application_type must reach the store (116-10 -> 116-11)"
    );
}

// ---------------------------------------------------------------------------
// Group E — D-18, the authorization-server substitution
// ---------------------------------------------------------------------------

/// The first connection to a given MCP server URL records the issuer discovery
/// resolved for it, against `normalize_server_key(server_url)`.
///
/// That record is the comparison anchor every later connection uses, so it has
/// to exist before there is anything to compare.
#[tokio::test]
async fn a_first_connection_records_the_discovered_issuer_against_the_normalized_server_key() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");

    assert_eq!(
        store.last_issuer(&server_key).await.expect("readable"),
        None,
        "nothing is recorded before the first connection"
    );

    let (helper, _opened) = helper_for(&base, &store, Some("preset-client"));
    helper.authorize_with_details().await.expect("a full flow");
    settle().await;

    assert_eq!(
        store.last_issuer(&server_key).await.expect("readable"),
        Some(base.clone())
    );
}

/// A second connection that discovers the SAME issuer produces no warning and
/// no error — the positive control for both change rows below.
#[tokio::test]
async fn an_unchanged_issuer_on_a_second_connection_neither_warns_nor_errors() {
    let (_server, _mocks, base) = authorization_server(false).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");
    store
        .record_issuer(&server_key, &base)
        .await
        .expect("seeding the same issuer");

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));
    let result = helper.authorize_with_details().await;
    settle().await;

    assert!(result.is_ok(), "an unchanged issuer must not be fatal");
    assert_eq!(opened.load(Ordering::SeqCst), 1);
    assert!(
        substitution_warnings(&messages).is_empty(),
        "an unchanged issuer must not warn: {:?}",
        substitution_warnings(&messages)
    );
}

/// **DCR-issued credentials: warn and PROCEED.**
///
/// SEP-2352 requires that a client MUST NOT reuse credentials from a different
/// authorization server and MUST re-register with the new one. Issuer-keyed
/// storage already accomplishes both by MISSING the cache, so nothing here has
/// to enforce anything — the warning is what makes the automatic
/// re-registration visible. Hard-failing would turn a legitimate operational
/// event (a tenant move, a provider migration) into an outage, which D-18
/// explicitly rejects.
#[tokio::test]
async fn an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds() {
    let (_server, _mocks, base) = authorization_server(true).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");
    store
        .record_issuer(&server_key, "https://previous-as.example")
        .await
        .expect("seeding the previous issuer");

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    // client_id is None + dcr_enabled, i.e. DCR-issued provenance.
    let (helper, opened) = helper_for(&base, &store, None);
    let result = helper
        .authorize_with_details()
        .await
        .expect("a DCR change must PROCEED, not fail");
    settle().await;

    // The flow ran, and re-registration happened against the NEW server.
    assert_eq!(opened.load(Ordering::SeqCst), 1);
    assert_eq!(result.client_id, "dcr-issued-id");

    // The warning names both issuers and the server.
    //
    // The failure message below carries the whole decision context on purpose.
    // This assertion has failed in CI while passing everywhere locally, and its
    // old message — `exactly one substitution warning, got []` — could not
    // distinguish the two candidate causes: the warn never FIRED (some early
    // return inside `announce_authorization_server_change`), or it fired and the
    // CAPTURE missed it. Those need opposite fixes. Reading `last_issuer` here is
    // a side-effect-free read, and the unfiltered message list separates "no
    // warnings at all" from "warnings, but none matched the filter".
    let warnings = substitution_warnings(&messages);
    let all_messages = messages.lock().expect("captured warnings").clone();
    let recorded_issuer = store.last_issuer(&server_key).await;
    assert_eq!(
        warnings.len(),
        1,
        "exactly one substitution warning, got {warnings:?}\n\
         \x20 all captured WARN messages : {all_messages:?}\n\
         \x20 server_key (test-computed) : {server_key:?}\n\
         \x20 base (mock AS issuer)      : {base:?}\n\
         \x20 seeded previous issuer     : \"https://previous-as.example\"\n\
         \x20 last_issuer AFTER the flow : {recorded_issuer:?}\n\
         Reading this: an EMPTY capture with last_issuer == Some(base) means the \
         warn never fired and the store was re-recorded, i.e. an early return in \
         announce_authorization_server_change (most likely last_issuer having \
         returned None when it was consulted). A NON-empty capture means the warn \
         fired and only the `authorization server` filter missed it."
    );
    let warning = &warnings[0];
    assert!(
        warning.contains("https://previous-as.example"),
        "the warning must name the OLD issuer: {warning}"
    );
    assert!(
        warning.contains(&base),
        "the warning must name the NEW issuer: {warning}"
    );
    assert!(
        warning.contains(&server_key),
        "the warning must name the MCP server: {warning}"
    );

    // The newly recorded last-seen issuer is the new one.
    assert_eq!(
        store.last_issuer(&server_key).await.expect("readable"),
        Some(base.clone())
    );
}

/// **Pre-registered credentials: REFUSE.**
///
/// A pre-registered `client_id` is provisioned for one authorization server and
/// is meaningless at another, so silently walking a user through a browser login
/// at an unexpected IdP with that id is the exact case the specification warns
/// about. The refusal carries the stable `reauth_required` identity, so a caller
/// branches on `is_reauth_required()` rather than on message text.
#[tokio::test]
async fn an_issuer_change_with_a_pre_registered_client_id_is_reauth_required_and_starts_no_flow() {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, false))
        .create_async()
        .await;
    // The refusal must happen BEFORE any authorization request, so the token
    // endpoint must never be reached.
    let token = server
        .mock("POST", "/token")
        .with_status(200)
        .with_body("{}")
        .expect(0)
        .create_async()
        .await;

    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");
    store
        .record_issuer(&server_key, "https://previous-as.example")
        .await
        .expect("seeding the previous issuer");

    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("provisioned-for-the-previous-as".to_string()),
        dcr_enabled: false,
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    let err = helper
        .authorize_with_details()
        .await
        .expect_err("a pre-registered client_id must refuse an AS substitution");

    // The programmatic identity, not a substring.
    assert!(err.is_reauth_required(), "expected reauth_required: {err}");
    assert_eq!(
        err.reauth_issuer(),
        Some(base.as_str()),
        "reauth_issuer must name the NEW issuer"
    );

    // The message still names all three values, for a human.
    let rendered = format!("{err}");
    assert!(
        rendered.contains("https://previous-as.example"),
        "{rendered}"
    );
    assert!(rendered.contains(&base), "{rendered}");
    assert!(rendered.contains(&server_key), "{rendered}");

    // No browser, no loopback listener, no token exchange.
    assert_eq!(
        opened.load(Ordering::SeqCst),
        0,
        "the browser flow must NOT be started"
    );
    assert!(
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "the loopback callback port must never have been bound"
    );
    token.assert_async().await;

    // `get_access_token` refuses identically — both entry points, one rule.
    let port2 = free_port();
    let (launcher2, opened2) = CountingCallbackLauncher::new(port2);
    let second = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("provisioned-for-the-previous-as".to_string()),
        dcr_enabled: false,
        redirect_port: port2,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher2)
    .with_credential_store(store.clone());
    let err2 = second
        .get_access_token()
        .await
        .expect_err("get_access_token must refuse too");
    assert!(err2.is_reauth_required(), "{err2}");
    assert_eq!(opened2.load(Ordering::SeqCst), 0);
}

/// An issuer change observed for MCP server A neither warns, errors nor rewrites
/// anything for MCP server B — the `issuers` map is keyed by server, exactly as
/// D-116-R1's credential key is.
///
/// Without this, a multi-server agent would produce spurious `reauth_required`
/// refusals the moment any one of its servers moved.
#[tokio::test]
async fn an_issuer_change_for_one_server_leaves_another_servers_issuer_record_untouched() {
    let (_server, _mocks, base) = authorization_server(true).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let server_a = normalize_server_key(&base).expect("normalized");
    let server_b = normalize_server_key("https://other-mcp.example").expect("normalized");

    // BOTH servers last saw the same old authorization server.
    store
        .record_issuer(&server_a, "https://previous-as.example")
        .await
        .expect("seeding A");
    store
        .record_issuer(&server_b, "https://previous-as.example")
        .await
        .expect("seeding B");

    let (helper, _opened) = helper_for(&base, &store, None);
    helper
        .authorize_with_details()
        .await
        .expect("A's change warns and proceeds");
    settle().await;

    assert_eq!(
        store.last_issuer(&server_a).await.expect("readable"),
        Some(base.clone()),
        "A's record advanced to the new issuer"
    );
    assert_eq!(
        store.last_issuer(&server_b).await.expect("readable"),
        Some("https://previous-as.example".to_string()),
        "B's record must be untouched by A's substitution"
    );
}

/// A store that declines D-18's issuer tracking — implementing only
/// `load`/`save`/`delete` and inheriting the `last_issuer` / `record_issuer`
/// defaults — still works: no warning, no error, no panic. It simply never
/// detects a substitution, which is today's behaviour and is what the trait's
/// defaults promise.
#[tokio::test]
async fn a_store_that_does_not_track_issuers_still_works() {
    use std::collections::BTreeMap;

    #[derive(Debug, Default)]
    struct MinimalStore {
        held: Mutex<BTreeMap<String, StoredCredentials>>,
    }

    impl MinimalStore {
        fn address(key: &CredentialKey) -> String {
            format!("{}|{}|{}", key.issuer(), key.account(), key.server())
        }
    }

    #[async_trait::async_trait]
    impl CredentialStore for MinimalStore {
        async fn load(&self, key: &CredentialKey) -> pmcp::Result<Option<StoredCredentials>> {
            Ok(self
                .held
                .lock()
                .expect("held")
                .get(&Self::address(key))
                .cloned())
        }

        async fn save(
            &self,
            key: &CredentialKey,
            credentials: &StoredCredentials,
        ) -> pmcp::Result<()> {
            self.held
                .lock()
                .expect("held")
                .insert(Self::address(key), credentials.clone());
            Ok(())
        }

        async fn delete(&self, key: &CredentialKey) -> pmcp::Result<()> {
            self.held.lock().expect("held").remove(&Self::address(key));
            Ok(())
        }
    }

    let (_server, _mocks, base) = authorization_server(false).await;
    let minimal = Arc::new(MinimalStore::default());
    let store: Arc<dyn CredentialStore> = minimal.clone();

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    let (helper, opened) = helper_for(&base, &store, Some("preset-client"));
    let result = helper
        .authorize_with_details()
        .await
        .expect("the defaults must not break a flow");
    settle().await;

    assert_eq!(result.access_token, "fresh-access-token");
    assert_eq!(opened.load(Ordering::SeqCst), 1);
    assert!(
        substitution_warnings(&messages).is_empty(),
        "a store that reports no last issuer must never warn about a change"
    );

    // `save_with_issuer`'s default really did store the credentials.
    let key = CredentialKey::new(&base, "", normalize_server_key(&base).expect("normalized"));
    assert_eq!(
        store
            .load(&key)
            .await
            .expect("readable")
            .expect("the record")
            .access_token(),
        "fresh-access-token"
    );
    // And the default `last_issuer` still reports nothing, as specified.
    assert_eq!(
        store
            .last_issuer(&normalize_server_key(&base).expect("normalized"))
            .await
            .expect("readable"),
        None
    );
}
