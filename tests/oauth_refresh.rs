//! Integration tests for `OAuthHelper`'s REFRESH path — D-14's three defects,
//! the scope RFC 6749 §6 permits it to send, and D-08's headless mode.
//!
//! # What is under test, and why it is the refresh path specifically
//!
//! All three D-14 defects block UNATTENDED operation, which is the only mode in
//! which a refresh matters at all. A human re-logs in; an agent cannot. So each
//! row here is written against the wire body or the stored record rather than
//! against a return value: "the refresh succeeded" is satisfied by a great many
//! implementations that would strand an agent an hour later.
//!
//! # Why the stored record is SEEDED in most rows
//!
//! A refresh is only reachable from `get_access_token` when the store already
//! holds an EXPIRED credential carrying a refresh token. Driving a full
//! interactive flow and then waiting for it to expire is not a test, it is an
//! hour. Seeding the record is therefore the only way to reach the code path,
//! and it makes the assertions sharper: the seeded value is the exact thing the
//! refresh must read (the DCR-issued `client_id`, the GRANTED scopes) or must
//! not destroy (the refresh token).
//!
//! # Feature gate
//!
//! `OAuthHelper` lives behind `oauth`, which `full` does NOT contain, so
//! `make quality-gate` runs **none** of this file. See `D-116-LINT-OAUTH`.

#![cfg(feature = "oauth")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mockito::{Mock, Server, ServerGuard};
use pmcp::client::oauth::{BrowserLauncher, Interactivity, OAuthConfig, OAuthHelper};
use pmcp::shared::credential_store::normalize_server_key;
use pmcp::{CredentialKey, CredentialStore, InMemoryCredentialStore, StoredCredentials};
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

/// Let a spawned callback-driving task finish reading before a mock server is
/// dropped, so nextest does not report the reader as leaky.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

/// A discovery document that declares `base` as its own issuer, which RFC 8414
/// section 3.3 anchoring requires, advertising `scopes_supported` verbatim.
fn discovery_body(base: &str, scopes_supported: &[&str]) -> String {
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": scopes_supported,
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    })
    .to_string()
}

/// Every body POSTed to the mock token endpoint, in order.
///
/// The refresh assertions are made against THIS rather than against the
/// helper's return value: a refresh that returns a token while sending the
/// wrong `client_id` or a scope the authorization server never granted is
/// exactly the failure D-14 describes, and it is invisible from the outside.
#[derive(Clone, Debug, Default)]
struct WireBodies(Arc<Mutex<Vec<String>>>);

impl WireBodies {
    fn record(&self, body: &str) {
        self.0.lock().expect("wire bodies").push(body.to_string());
    }

    /// Only the bodies that carry `grant_type=refresh_token`, so a code
    /// exchange on the same endpoint cannot be mistaken for a refresh.
    fn refreshes(&self) -> Vec<Vec<(String, String)>> {
        self.0
            .lock()
            .expect("wire bodies")
            .iter()
            .map(|body| form_pairs(body))
            .filter(|pairs| value(pairs, "grant_type") == Some("refresh_token"))
            .collect()
    }

    /// The single refresh this row expected, failing loudly on zero or many.
    fn only_refresh(&self) -> Vec<(String, String)> {
        let mut all = self.refreshes();
        assert_eq!(
            all.len(),
            1,
            "expected exactly one refresh request, saw {}",
            all.len()
        );
        all.remove(0)
    }
}

/// Decode an `application/x-www-form-urlencoded` body into ordered pairs.
///
/// Ordered, and not a map: "exactly the stored granted scopes, in the stored
/// order" is an assertion about a sequence, and a map would silently make the
/// order untestable. Duplicate keys survive for the same reason.
fn form_pairs(body: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body.as_bytes())
        .map(|(key, val)| (key.into_owned(), val.into_owned()))
        .collect()
}

/// The first value for `key`, or `None` when the key is ABSENT.
///
/// Absent and present-but-empty are deliberately distinguishable: `scope=` and
/// no `scope` at all are different requests, and only the second is correct
/// when nothing was granted.
fn value<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// A token-endpoint answer.
struct TokenReply {
    status: usize,
    body: String,
}

impl TokenReply {
    /// A successful refresh response built from the fields an authorization
    /// server MAY omit, so each row states exactly what it is exercising.
    fn ok(access_token: &str, refresh_token: Option<&str>, expires_in: Option<u64>) -> Self {
        let mut body = json!({ "access_token": access_token, "token_type": "Bearer" });
        if let Some(refresh) = refresh_token {
            body["refresh_token"] = json!(refresh);
        }
        if let Some(ttl) = expires_in {
            body["expires_in"] = json!(ttl);
        }
        Self {
            status: 200,
            body: body.to_string(),
        }
    }

    fn failure(status: usize, body: String) -> Self {
        Self { status, body }
    }
}

/// A mock authorization server whose token endpoint RECORDS every refresh body
/// it is sent and answers refreshes with `reply`.
///
/// # Why the token endpoint carries TWO mocks
///
/// One endpoint serves two grants. A single mock answering both would make
/// every "the refresh was REFUSED" row also refuse the authorization-code
/// exchange that the fall-through then performs, so the row would fail on
/// `No supported OAuth flow available` and never reach the assertion it was
/// written for. Measured: that is exactly how the first RED run of the two
/// error-path rows failed. Routing by `grant_type` keeps the two independent,
/// so a refusal is a refusal of the REFRESH and nothing else.
///
/// The guard AND the mocks are returned, never dropped inline: dropping a
/// `ServerGuard` stops the server and dropping a `Mock` removes it, so a row
/// that let either go would be asserting against a dead endpoint.
async fn refresh_server(
    scopes_supported: &[&str],
    reply: TokenReply,
) -> (ServerGuard, Vec<Mock>, String, WireBodies) {
    let mut server = Server::new_async().await;
    let base = server.url();
    let bodies = WireBodies::default();
    let mut mocks = Vec::new();

    mocks.push(
        server
            .mock("GET", "/.well-known/openid-configuration")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(discovery_body(&base, scopes_supported))
            .expect_at_least(0)
            .create_async()
            .await,
    );

    // Mock 1 — the CATCH-ALL, i.e. the authorization-code exchange the
    // interactive fall-through performs. Always succeeds, so a row that asserts
    // "the flow fell through" is asserting about the fall-through and not about
    // a second, unrelated failure.
    //
    // It deliberately issues NO `refresh_token`: RFC 6749 §6's "MUST NOT assume
    // refresh tokens will be issued" is a real authorization-server behaviour,
    // and making it the fixture's default means every fall-through row also
    // exercises it rather than only the one row that names it.
    //
    // It is created FIRST because of a measured mockito rule: when several
    // mocks match one request and none of them has an unmet `expect(n)`,
    // `mockito` serves the LAST matching mock (`server.rs`, `matching_mocks.
    // last_mut()`), not the first. Creating the catch-all first is therefore
    // what makes the more specific refresh mock below win. Getting this
    // backwards is silent: every refresh row simply receives the code-exchange
    // answer and fails on a token value, which is how the first GREEN run of
    // this suite failed.
    mocks.push(
        server
            .mock("POST", "/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "access_token": "interactive-access-token",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "scope": "openid",
                })
                .to_string(),
            )
            .expect_at_least(0)
            .create_async()
            .await,
    );

    // Mock 2 — the REFRESH grant only, created LAST so it wins. The body is
    // recorded inside the matcher and only on a match, so a code exchange
    // cannot be recorded as a refresh.
    let recorder = bodies.clone();
    mocks.push(
        server
            .mock("POST", "/token")
            .match_request(move |request| {
                let Ok(raw) = request.body() else {
                    return false;
                };
                let body = String::from_utf8_lossy(raw);
                if !body.contains("grant_type=refresh_token") {
                    return false;
                }
                recorder.record(&body);
                true
            })
            .with_status(reply.status)
            .with_header("content-type", "application/json")
            .with_body(reply.body)
            .expect_at_least(0)
            .create_async()
            .await,
    );

    mocks.push(
        server
            .mock("POST", "/register")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(json!({ "client_id": "freshly-registered-id" }).to_string())
            .expect_at_least(0)
            .create_async()
            .await,
    );

    (server, mocks, base, bodies)
}

/// A [`BrowserLauncher`] that COMPLETES the flow — it lifts `state` out of the
/// authorization URL and delivers a legitimate callback to the loopback
/// listener the flow already bound — and COUNTS how many times it was asked to.
///
/// The count is the observable behind "no browser flow was started". A timing
/// assertion is not: it passes on a fast machine for the wrong reason.
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
                    .map(|(_, val)| val.into_owned())
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

/// How a row wants its helper configured, so the shared builder does not grow a
/// six-argument signature nobody can read at a call site.
struct HelperSpec<'a> {
    base: &'a str,
    client_id: Option<&'a str>,
    config_scopes: &'a [&'a str],
}

impl<'a> HelperSpec<'a> {
    fn new(base: &'a str) -> Self {
        Self {
            base,
            client_id: Some("preset-client"),
            config_scopes: &["openid"],
        }
    }

    /// A DCR client: no configured `client_id`, so the only id available to a
    /// refresh is the one the store holds. This is D-14 defect 2's shape.
    fn dcr(mut self) -> Self {
        self.client_id = None;
        self
    }

    fn config_scopes(mut self, scopes: &'a [&'a str]) -> Self {
        self.config_scopes = scopes;
        self
    }
}

/// Build a helper wired to `store`, driving its callback through a counting
/// launcher, and report the launcher count plus the redirect port.
fn helper_for(
    spec: &HelperSpec<'_>,
    store: &Arc<dyn CredentialStore>,
) -> (OAuthHelper, Arc<AtomicUsize>, u16) {
    let port = free_port();
    let (launcher, opened) = CountingCallbackLauncher::new(port);
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(spec.base.to_string()),
        client_id: spec.client_id.map(str::to_string),
        dcr_enabled: spec.client_id.is_none(),
        scopes: spec
            .config_scopes
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher)
    .with_credential_store(store.clone());

    (helper, opened, port)
}

/// The `(issuer, account, server)` address a helper built by [`helper_for`]
/// uses, so a row seeds exactly where the helper will look.
fn key_for(base: &str) -> CredentialKey {
    CredentialKey::new(base, "", normalize_server_key(base).expect("normalized"))
}

/// How a row wants its seeded, EXPIRED credential record shaped.
struct SeedSpec<'a> {
    client_id: &'a str,
    refresh_token: Option<&'a str>,
    granted_scopes: &'a [&'a str],
}

impl<'a> SeedSpec<'a> {
    fn new() -> Self {
        Self {
            client_id: "preset-client",
            refresh_token: Some("stored-refresh-token"),
            granted_scopes: &["openid"],
        }
    }

    fn client_id(mut self, client_id: &'a str) -> Self {
        self.client_id = client_id;
        self
    }

    fn refresh_token(mut self, refresh_token: Option<&'a str>) -> Self {
        self.refresh_token = refresh_token;
        self
    }

    fn granted_scopes(mut self, scopes: &'a [&'a str]) -> Self {
        self.granted_scopes = scopes;
        self
    }
}

/// Seed an EXPIRED credential record — the only state from which a refresh is
/// reachable — and return the key it was written under.
///
/// `save`, not `save_with_issuer`: recording an issuer here would also arm
/// 116-11's substitution detection, and these rows are about the refresh, not
/// about D-18. The rows that DO want the detection record the issuer
/// explicitly.
async fn seed_expired(
    store: &Arc<dyn CredentialStore>,
    base: &str,
    spec: &SeedSpec<'_>,
) -> CredentialKey {
    let key = key_for(base);
    let mut credentials = StoredCredentials::new("STALE-ACCESS-TOKEN", spec.client_id)
        .with_granted_scopes(spec.granted_scopes.iter().map(|s| (*s).to_string()))
        // Already expired: `expires_at` in the past is what makes the refresh
        // path reachable at all.
        .with_expires_at(unix_now().saturating_sub(60));
    if let Some(refresh) = spec.refresh_token {
        credentials = credentials.with_refresh_token(refresh);
    }
    store
        .save(&key, &credentials)
        .await
        .expect("seeding the expired record");
    key
}

/// Read back the record a row seeded, after the helper has run.
async fn stored(store: &Arc<dyn CredentialStore>, key: &CredentialKey) -> StoredCredentials {
    store
        .load(key)
        .await
        .expect("a readable store")
        .expect("a record under the seeded key")
}

// ---------------------------------------------------------------------------
// Warning capture — the refresh failure path has no other observable
// ---------------------------------------------------------------------------

/// Captures WARN-level event messages, so "the refresh failed, and the SDK said
/// why" is ASSERTED rather than assumed.
#[derive(Debug, Default)]
struct WarnCapture {
    messages: Arc<Mutex<Vec<String>>>,
}

/// Pulls the formatted `message` field out of a `tracing` event.
struct MessageVisitor<'a>(&'a mut Vec<String>);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, val: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push(format!("{val:?}"));
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

/// Warnings that name a refresh failure, filtered out of everything else this
/// module warns about (an unregistered redirect URI, an expiring token, a
/// discarded legacy cache).
fn refresh_failure_warnings(captured: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    captured
        .lock()
        .expect("captured warnings")
        .iter()
        .filter(|message| message.contains("refresh") && message.contains("failed"))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Group A — D-14 defect 1: the stored refresh token survives
// ---------------------------------------------------------------------------

/// **The load-bearing row of this plan.**
///
/// RFC 6749 §6 lets an authorization server answer a refresh with NO
/// `refresh_token` field, meaning "keep the one you have"; many OIDC servers
/// do. `TokenResponse::refresh_token` is `#[serde(default)]`, so an omitted
/// field deserializes to `None` — and `None` looks exactly like data at the
/// credential write.
///
/// Writing that `None` over a perfectly good token limits an UNATTENDED agent
/// to exactly one refresh cycle before it demands a human. That is the
/// difference between an agent that keeps working overnight and one that stops
/// an hour in, which is why this row asserts the stored value AND drives a
/// SECOND refresh with it rather than merely reading the record back.
#[tokio::test]
async fn an_omitted_refresh_token_in_the_response_preserves_the_stored_one() {
    // `expires_in: 0` so the refreshed record is immediately stale again and a
    // second `get_access_token` must refresh once more — with the token the
    // first refresh was supposed to preserve.
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", None, Some(0)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let key = seed_expired(&store, &base, &SeedSpec::new()).await;

    let spec = HelperSpec::new(&base);
    let (first, first_opened, _) = helper_for(&spec, &store);
    assert_eq!(
        first.get_access_token().await.expect("a refresh"),
        "refreshed-access-token"
    );
    assert_eq!(
        first_opened.load(Ordering::SeqCst),
        0,
        "a successful refresh must not open a browser"
    );

    assert_eq!(
        stored(&store, &key).await.refresh_token(),
        Some("stored-refresh-token"),
        "an omitted refresh_token means KEEP the stored one, not discard it"
    );

    // The second cycle is the part that matters: the preserved token must still
    // be usable, not merely still present.
    let (second, second_opened, _) = helper_for(&spec, &store);
    assert_eq!(
        second.get_access_token().await.expect("a second refresh"),
        "refreshed-access-token"
    );
    assert_eq!(
        second_opened.load(Ordering::SeqCst),
        0,
        "the second cycle must also be unattended"
    );

    let refreshes = wire.refreshes();
    assert_eq!(refreshes.len(), 2, "two refresh cycles, got {refreshes:?}");
    for (cycle, pairs) in refreshes.iter().enumerate() {
        assert_eq!(
            value(pairs, "refresh_token"),
            Some("stored-refresh-token"),
            "cycle {cycle} must present the surviving refresh token"
        );
    }
}

/// The other half of the rule: a response that DOES supply a new refresh token
/// replaces the stored one, so rotation is honoured.
#[tokio::test]
async fn a_refresh_response_that_supplies_a_new_refresh_token_replaces_the_stored_one() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok(
            "refreshed-access-token",
            Some("rotated-refresh-token"),
            Some(3600),
        ),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let key = seed_expired(&store, &base, &SeedSpec::new()).await;

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper.get_access_token().await.expect("a refresh");

    assert_eq!(opened.load(Ordering::SeqCst), 0);
    assert_eq!(
        stored(&store, &key).await.refresh_token(),
        Some("rotated-refresh-token"),
        "a supplied refresh_token must REPLACE the stored one"
    );
    assert_eq!(
        value(&wire.only_refresh(), "refresh_token"),
        Some("stored-refresh-token"),
        "the request itself still presents the OLD token"
    );
}

/// RFC 6749 §5.1 makes `expires_in` optional. An omitted one must not leave a
/// FRESH access token wearing the stale record's already-past expiry, which
/// would make every later call believe a brand-new token was born expired.
#[tokio::test]
async fn a_refresh_response_that_omits_expires_in_does_not_corrupt_the_stored_expiry() {
    let (_server, _mocks, base, _wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), None),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let key = seed_expired(&store, &base, &SeedSpec::new()).await;
    let seeded_expiry = stored(&store, &key).await.expires_at();

    let (helper, _opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper.get_access_token().await.expect("a refresh");

    let after = stored(&store, &key).await;
    assert_eq!(
        after.access_token(),
        "refreshed-access-token",
        "the new token is what is stored"
    );
    assert_ne!(
        after.expires_at(),
        seeded_expiry,
        "the stale expiry must not be carried over onto a brand-new token"
    );
    assert!(
        !matches!(after.expires_at(), Some(at) if at <= unix_now()),
        "an unknown expiry is recorded as unknown, never as a moment in the past: {:?}",
        after.expires_at()
    );
}

/// RFC 6749 §6's "MUST NOT assume refresh tokens will be issued": an
/// authorization that produced none stores `None`, and the later expiry is a
/// clean fall-through rather than an unwrap on a missing token.
#[tokio::test]
async fn an_authorization_that_issued_no_refresh_token_stores_none_and_falls_through() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("interactive-access-token", None, Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    // Seeded EXPIRED and with no refresh token at all — the state an
    // authorization server that never issues one leaves behind.
    let key = seed_expired(&store, &base, &SeedSpec::new().refresh_token(None)).await;

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base), &store);
    let token = helper
        .get_access_token()
        .await
        .expect("no refresh token is a fall-through, never a panic");
    settle().await;

    assert_eq!(token, "interactive-access-token");
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "with nothing to refresh, the interactive flow is the correct answer"
    );
    assert!(
        wire.refreshes().is_empty(),
        "no refresh may be attempted when no refresh token was ever issued"
    );
    assert_eq!(stored(&store, &key).await.refresh_token(), None);
}

// ---------------------------------------------------------------------------
// Group B — D-14 defect 2: a DCR-registered client can refresh
// ---------------------------------------------------------------------------

/// Under DCR the `client_id` is ISSUED, so it lives in the store and never in
/// `OAuthConfig`. Reading it from config makes a DCR client unable to refresh
/// even once — it errors with "cannot refresh token without a cached
/// client_id" and falls all the way back to a fresh browser login.
#[tokio::test]
async fn a_dcr_registered_client_refreshes_with_the_stored_issued_client_id() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(
        &store,
        &base,
        &SeedSpec::new().client_id("dcr-issued-client-id"),
    )
    .await;

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base).dcr(), &store);
    assert_eq!(
        helper
            .get_access_token()
            .await
            .expect("a DCR client must be able to refresh"),
        "refreshed-access-token"
    );

    assert_eq!(
        opened.load(Ordering::SeqCst),
        0,
        "a working refresh must not re-open a browser"
    );
    assert_eq!(
        value(&wire.only_refresh(), "client_id"),
        Some("dcr-issued-client-id"),
        "the refresh must carry the client_id the authorization server ISSUED"
    );
}

/// With a `client_id` in config AND one in the store, the stored one wins: it
/// is the id that was actually paired with this refresh token.
#[tokio::test]
async fn the_stored_client_id_is_preferred_over_the_configured_one() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new().client_id("stored-id")).await;

    let (helper, _opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper.get_access_token().await.expect("a refresh");

    assert_eq!(
        value(&wire.only_refresh(), "client_id"),
        Some("stored-id"),
        "the record's own client_id is the one paired with its refresh token"
    );
}

/// With neither a configured nor a stored `client_id` there is nothing to send.
/// The refusal must NAME BOTH PLACES it looked, because "no client_id" without
/// that is unactionable — a caller cannot tell whether to configure one or to
/// re-run registration.
#[tokio::test]
async fn a_refresh_with_no_client_id_anywhere_names_both_places_it_looked() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("interactive-access-token", Some("r"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    // An empty stored client_id and no configured one.
    seed_expired(&store, &base, &SeedSpec::new().client_id("")).await;

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base).dcr(), &store);
    helper
        .get_access_token()
        .await
        .expect("a missing client_id falls through, it does not panic");
    settle().await;

    assert!(
        wire.refreshes().is_empty(),
        "the refusal must happen BEFORE any request reaches the token endpoint"
    );
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "the interactive flow is the fall-through"
    );

    let warnings = refresh_failure_warnings(&messages);
    assert_eq!(
        warnings.len(),
        1,
        "exactly one refresh-failure warning, got {warnings:?}"
    );
    let warning = &warnings[0];
    assert!(
        warning.contains("client_id"),
        "the refusal must name what is missing: {warning}"
    );
    assert!(
        warning.contains("OAuthConfig::client_id"),
        "the refusal must name the CONFIG place it looked: {warning}"
    );
    assert!(
        warning.contains("stored credential"),
        "the refusal must name the STORE place it looked: {warning}"
    );
}

// ---------------------------------------------------------------------------
// Group C — D-14 defect 3: the refresh carries the GRANTED scope, or none
// ---------------------------------------------------------------------------

/// Some OIDC authorization servers require `scope` on a refresh, and others
/// narrow the grant when it is absent. It was never sent at all.
///
/// The value is the GRANTED scope — what the authorization server actually
/// issued — in the order it was recorded.
#[tokio::test]
async fn the_refresh_body_carries_exactly_the_stored_granted_scopes_in_order() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid", "mcp:read", "offline_access"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(
        &store,
        &base,
        &SeedSpec::new().granted_scopes(&["mcp:read", "openid", "offline_access"]),
    )
    .await;

    let (helper, _opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper.get_access_token().await.expect("a refresh");

    assert_eq!(
        value(&wire.only_refresh(), "scope"),
        Some("mcp:read openid offline_access"),
        "exactly the stored granted scopes, in the stored order"
    );
}

/// When nothing is recorded as granted there is nothing to ask for, and RFC
/// 6749 §6 makes `scope` OPTIONAL — so the key is omitted entirely.
///
/// The two wrong answers this row rules out are `scope=` (an empty value, which
/// a conforming authorization server may read as "no scopes") and a fall-back
/// to `config.scopes`, which is what was ASKED for and not what was granted.
/// `config.scopes` is deliberately non-empty and deliberately disjoint from
/// anything else in the fixture, so a fall-back could not hide.
#[tokio::test]
async fn empty_stored_granted_scopes_omit_the_scope_key_entirely() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid", "profile", "email"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new().granted_scopes(&[])).await;

    let spec = HelperSpec::new(&base).config_scopes(&["profile", "email"]);
    let (helper, _opened, _) = helper_for(&spec, &store);
    helper.get_access_token().await.expect("a refresh");

    let pairs = wire.only_refresh();
    assert_eq!(
        value(&pairs, "scope"),
        None,
        "no granted scope means NO scope key — not an empty one: {pairs:?}"
    );
    assert!(
        !pairs.iter().any(|(key, _)| key == "scope"),
        "the key itself must be absent from the wire body: {pairs:?}"
    );
}

/// SEP-2207's `offline_access` lifecycle has four stages, and a refresh is the
/// last one. An authorization server that ADVERTISES `offline_access` in
/// `scopes_supported` but never GRANTED it must not see it on a refresh: RFC
/// 6749 §6 forbids widening, and a conforming server answers `invalid_scope`,
/// which would break refresh entirely — the opposite of what D-14 is for.
#[tokio::test]
async fn an_advertised_but_never_granted_offline_access_is_absent_from_the_refresh() {
    let (_server, _mocks, base, wire) = refresh_server(
        // Advertised…
        &["openid", "offline_access"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    // …but never granted.
    seed_expired(&store, &base, &SeedSpec::new().granted_scopes(&["openid"])).await;

    let spec = HelperSpec::new(&base).config_scopes(&["openid", "offline_access"]);
    let (helper, _opened, _) = helper_for(&spec, &store);
    helper.get_access_token().await.expect("a refresh");

    let pairs = wire.only_refresh();
    let scope = value(&pairs, "scope").expect("a granted scope is sent");
    assert_eq!(scope, "openid");
    assert!(
        !scope.contains("offline_access"),
        "advertised is not granted: {scope}"
    );
}

/// **RFC 6749 §6: "the scope of the access request MUST NOT include any scope
/// not originally granted by the resource owner".**
///
/// The property is set containment over the WIRE BODY, quantified over
/// generated grant sets — it is derived from the specification's sentence, not
/// restated from the implementation, so an implementation that computed the
/// refresh scope some other way would still have to satisfy it.
///
/// The grants are generated with `proptest`'s strategies and then driven
/// through a real refresh, rather than run inside `proptest!`: each case needs
/// an async round trip against a mock authorization server, and `proptest!`
/// bodies are synchronous.
#[tokio::test]
async fn a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6() {
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::TestRunner;

    let scope_atom = proptest::sample::select(vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
        "offline_access".to_string(),
        "mcp:read".to_string(),
        "mcp:write".to_string(),
    ]);
    let grants = proptest::collection::vec(scope_atom, 0..6);

    let mut runner = TestRunner::deterministic();
    let cases: Vec<Vec<String>> = (0..24)
        .map(|_| {
            grants
                .new_tree(&mut runner)
                .expect("a generated grant set")
                .current()
        })
        .collect();

    let (_server, _mocks, base, wire) = refresh_server(
        // Everything is ADVERTISED, so only the GRANT can bound the request.
        &[
            "openid",
            "profile",
            "email",
            "offline_access",
            "mcp:read",
            "mcp:write",
        ],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(0)),
    )
    .await;

    for granted in &cases {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let owned: Vec<&str> = granted.iter().map(String::as_str).collect();
        seed_expired(&store, &base, &SeedSpec::new().granted_scopes(&owned)).await;

        // Everything is configured, so a fall-back to `config.scopes` would be
        // a widening this property catches.
        let spec = HelperSpec::new(&base).config_scopes(&[
            "openid",
            "profile",
            "email",
            "offline_access",
            "mcp:read",
            "mcp:write",
        ]);
        let (helper, opened, _) = helper_for(&spec, &store);
        helper
            .get_access_token()
            .await
            .expect("a refresh for every generated grant");
        assert_eq!(opened.load(Ordering::SeqCst), 0);

        let pairs = wire
            .refreshes()
            .pop()
            .expect("the refresh this case just drove");
        let sent: Vec<&str> = value(&pairs, "scope")
            .map(|scope| scope.split_whitespace().collect())
            .unwrap_or_default();

        for scope in &sent {
            assert!(
                granted.iter().any(|g| g == scope),
                "RFC 6749 §6: sent {sent:?} includes {scope:?}, which was never granted \
                 ({granted:?})"
            );
        }
        if granted.is_empty() {
            assert!(
                sent.is_empty(),
                "an empty grant must send no scope at all, sent {sent:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Group D — the refresh error path is bounded
// ---------------------------------------------------------------------------

/// A hostile authorization server controls its ERROR bodies too, and the error
/// path is where it has the most freedom. An oversized refusal body must be
/// refused by the bounded reader, and the refusal must name the CAP and
/// reproduce no byte of what it dropped.
#[tokio::test]
async fn an_oversized_refresh_error_body_is_refused_naming_the_cap_and_no_content() {
    const CANARY: &str = "CANARY-FROM-A-HOSTILE-REFRESH-ERROR-BODY";

    let mut huge = String::with_capacity(1_300_000);
    huge.push_str(CANARY);
    huge.push_str(&"A".repeat(1_300_000));

    let (_server, _mocks, base, _wire) =
        refresh_server(&["openid"], TokenReply::failure(400, huge)).await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new()).await;

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper
        .get_access_token()
        .await
        .expect("an oversized refusal falls through, it does not abort the caller");
    settle().await;
    assert_eq!(opened.load(Ordering::SeqCst), 1);

    let warnings = refresh_failure_warnings(&messages);
    assert_eq!(
        warnings.len(),
        1,
        "exactly one refresh-failure warning, got {warnings:?}"
    );
    let warning = &warnings[0];
    assert!(
        warning.contains("1048576"),
        "the refusal must name the cap: {warning}"
    );
    assert!(
        !warning.contains(CANARY),
        "the refusal must not become a channel for the bytes it refused"
    );
    assert!(
        !warning.contains("AAAAAAAAAA"),
        "no padding from the refused body may appear either"
    );
}

/// A within-cap refusal is still a refusal: the flow falls through rather than
/// aborting, and the SDK says why.
#[tokio::test]
async fn a_rejected_refresh_falls_through_and_says_why() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::failure(400, json!({ "error": "invalid_grant" }).to_string()),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new()).await;

    let capture = WarnCapture::default();
    let messages = capture.messages.clone();
    let _guard = tracing::subscriber::set_default(capture);

    let (helper, opened, _) = helper_for(&HelperSpec::new(&base), &store);
    helper
        .get_access_token()
        .await
        .expect("a rejected refresh falls through to an interactive login");
    settle().await;

    assert_eq!(wire.refreshes().len(), 1, "the refresh was attempted");
    assert_eq!(opened.load(Ordering::SeqCst), 1);
    let warnings = refresh_failure_warnings(&messages);
    assert_eq!(warnings.len(), 1, "got {warnings:?}");
    assert!(
        warnings[0].contains("invalid_grant"),
        "the authorization server's own reason must survive: {}",
        warnings[0]
    );
}

// ---------------------------------------------------------------------------
// Group E — D-08: `Interactivity::RefreshOnly`
// ---------------------------------------------------------------------------
//
// What today's silent fall-through costs a headless caller, measured from the
// source rather than guessed: `get_access_token` swallows a refresh failure,
// calls `authorization_code_flow_inner`, which binds a loopback listener
// nothing can reach, opens a browser nobody can see, and then waits on
// `tokio::time::timeout(Duration::from_mins(5), ..)`. That is five minutes of
// wall clock per attempt in a Lambda, ending in a timeout error that does not
// say "a human is required".
//
// The rows below assert the mode's guarantee with TWO observables and never
// with timing alone: a wall-clock assertion passes on a fast machine for the
// wrong reason, and would keep passing if the listener were bound and closed.
// The launcher count is the direct assertion; the port-bindability check is
// the one that also catches a listener bound BEFORE the browser call.

/// Build a `RefreshOnly` helper, returning the launcher count and the redirect
/// port so a row can prove nothing was bound to it.
fn refresh_only_helper_for(
    spec: &HelperSpec<'_>,
    store: &Arc<dyn CredentialStore>,
) -> (OAuthHelper, Arc<AtomicUsize>, u16) {
    let (helper, opened, port) = helper_for(spec, store);
    (
        helper.with_interactivity(Interactivity::RefreshOnly),
        opened,
        port,
    )
}

/// Assert the full "nothing interactive happened" property: the browser
/// launcher was never asked, and the redirect port is still free — which it
/// would not be if a listener were still bound, and which together with the
/// launcher count covers a listener bound and then dropped.
fn assert_nothing_interactive_happened(opened: &Arc<AtomicUsize>, port: u16) {
    assert_eq!(
        opened.load(Ordering::SeqCst),
        0,
        "RefreshOnly must never invoke the browser launcher"
    );
    assert!(
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "the redirect port {port} must still be bindable: RefreshOnly must bind no loopback \
         listener"
    );
}

/// The default is `Interactive`, and it is EXACTLY today's behaviour: a refresh
/// failure still falls through to the browser flow. No existing caller changes.
///
/// This row is the reason the mode is opt-in rather than a behaviour change.
#[tokio::test]
async fn the_default_mode_still_falls_through_to_the_browser_flow() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::failure(400, json!({ "error": "invalid_grant" }).to_string()),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new()).await;

    // No `with_interactivity` call at all.
    let (helper, opened, _) = helper_for(&HelperSpec::new(&base), &store);
    let token = helper
        .get_access_token()
        .await
        .expect("the default must still fall through");
    settle().await;

    assert_eq!(token, "interactive-access-token");
    assert_eq!(wire.refreshes().len(), 1, "the refresh was still attempted");
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "the default mode still opens a browser on a failed refresh"
    );
}

/// A live cached token is served under `RefreshOnly` exactly as it is under
/// `Interactive`: the mode narrows the FALL-BACK, not the cache.
#[tokio::test]
async fn refresh_only_with_a_live_cached_token_returns_it() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let key = key_for(&base);
    store
        .save(
            &key,
            &StoredCredentials::new("LIVE-ACCESS-TOKEN", "preset-client")
                .with_refresh_token("stored-refresh-token")
                .with_granted_scopes(vec!["openid".to_string()])
                .with_expires_at(unix_now() + 9_000),
        )
        .await
        .expect("seeding a live record");

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    assert_eq!(
        helper
            .get_access_token()
            .await
            .expect("a live cached token"),
        "LIVE-ACCESS-TOKEN"
    );
    assert!(
        wire.refreshes().is_empty(),
        "a live token needs no network at all"
    );
    assert_nothing_interactive_happened(&opened, port);
}

/// An expired token with a WORKING refresh is the mode's happy path: it
/// refreshes and returns the new token, entirely unattended.
#[tokio::test]
async fn refresh_only_with_an_expired_token_and_a_working_refresh_returns_the_new_token() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let key = seed_expired(&store, &base, &SeedSpec::new()).await;

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    assert_eq!(
        helper
            .get_access_token()
            .await
            .expect("an unattended refresh"),
        "refreshed-access-token"
    );

    assert_eq!(wire.refreshes().len(), 1);
    assert_nothing_interactive_happened(&opened, port);
    assert_eq!(
        stored(&store, &key).await.access_token(),
        "refreshed-access-token",
        "the refreshed credential is persisted, so the next call is a cache hit"
    );
}

/// **The row D-08 exists for.**
///
/// A failing refresh under `RefreshOnly` returns a TYPED refusal immediately.
/// Three independent observables, because any one of them alone can pass for
/// the wrong reason:
///
/// - `is_reauth_required()` / `reauth_issuer()` — the programmatic identity, not
///   a substring match, so a caller can branch on it.
/// - a browser-launcher count of zero — the direct assertion.
/// - the redirect port still bindable — catches a listener bound before the
///   browser call, which the launcher count alone would miss.
///
/// The wall clock is asserted too, but LAST and loosely: it is corroboration
/// that the five-minute callback timeout was not entered, not the proof.
#[tokio::test]
async fn refresh_only_with_a_failing_refresh_is_reauth_required_and_starts_nothing() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::failure(400, json!({ "error": "invalid_grant" }).to_string()),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new()).await;

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    let started = std::time::Instant::now();
    let err = helper
        .get_access_token()
        .await
        .expect_err("a failing refresh under RefreshOnly is an error, not a browser");
    let elapsed = started.elapsed();

    assert!(
        err.is_reauth_required(),
        "the refusal must carry the programmatic reauth-required identity: {err}"
    );
    assert_eq!(
        err.reauth_issuer(),
        Some(base.as_str()),
        "and must name the issuer the caller has to re-authorize against"
    );
    assert_eq!(wire.refreshes().len(), 1, "the refresh WAS attempted first");
    assert_nothing_interactive_happened(&opened, port);
    assert!(
        elapsed < Duration::from_secs(5),
        "corroboration only: the five-minute callback wait was plainly not entered ({elapsed:?})"
    );
}

/// No stored credentials at all is the same typed refusal, immediately. This is
/// the cold-start case for a headless runtime whose credential store has not
/// been seeded yet — and it must be actionable, not a five-minute silence.
#[tokio::test]
async fn refresh_only_with_no_cached_credentials_is_the_same_typed_refusal() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    let err = helper
        .get_access_token()
        .await
        .expect_err("no credentials under RefreshOnly is an error");

    assert!(err.is_reauth_required(), "{err}");
    assert_eq!(err.reauth_issuer(), Some(base.as_str()));
    assert!(
        wire.refreshes().is_empty(),
        "with nothing stored there is nothing to refresh"
    );
    assert_nothing_interactive_happened(&opened, port);
}

/// An expired record carrying NO refresh token is the third way to reach the
/// refusal, and it must be reported as its own condition rather than folded
/// into "the refresh failed" — the fix is different (re-authorize with
/// `offline_access`, not retry).
#[tokio::test]
async fn refresh_only_with_an_expired_token_and_no_refresh_token_refuses_distinctly() {
    let (_server, _mocks, base, wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    seed_expired(&store, &base, &SeedSpec::new().refresh_token(None)).await;

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    let err = helper
        .get_access_token()
        .await
        .expect_err("no refresh token under RefreshOnly is an error");

    assert!(err.is_reauth_required(), "{err}");
    let message = err.to_string();
    assert!(
        message.contains("refresh token"),
        "the refusal must say WHICH condition it is: {message}"
    );
    assert!(
        wire.refreshes().is_empty(),
        "there was no refresh token to present"
    );
    assert_nothing_interactive_happened(&opened, port);
}

/// `authorize_with_details` is the OTHER public entry point, and it is the one
/// that means "log me in". Under `RefreshOnly` it must refuse rather than open a
/// browser, or the mode's guarantee has a hole a caller can fall through by
/// picking the other method.
#[tokio::test]
async fn refresh_only_refuses_the_explicit_login_entry_point_too() {
    let (_server, _mocks, base, _wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());

    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    let err = helper
        .authorize_with_details()
        .await
        .expect_err("an explicit login under RefreshOnly is a contradiction");

    assert!(err.is_reauth_required(), "{err}");
    assert_nothing_interactive_happened(&opened, port);
}

/// 116-11's authorization-server substitution refusal is also a
/// reauth-required, and it must keep NAMING the change: a headless caller that
/// receives it needs to know its identity provider moved, not merely that it
/// needs to log in again.
///
/// This row is under `RefreshOnly` deliberately — the two refusals must be
/// distinguishable by MESSAGE even though they share a programmatic identity.
#[tokio::test]
async fn a_reauth_required_from_an_issuer_change_still_names_the_change() {
    let (_server, _mocks, base, _wire) = refresh_server(
        &["openid"],
        TokenReply::ok("refreshed-access-token", Some("rotated"), Some(3600)),
    )
    .await;
    let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
    let server_key = normalize_server_key(&base).expect("normalized");
    store
        .record_issuer(&server_key, "https://previous-as.example")
        .await
        .expect("seeding the previous issuer");

    // A PRE-REGISTERED client_id, which is the provenance D-18 makes fatal.
    let (helper, opened, port) = refresh_only_helper_for(&HelperSpec::new(&base), &store);
    let err = helper
        .get_access_token()
        .await
        .expect_err("a substitution with a pre-registered client_id is fatal");

    assert!(err.is_reauth_required(), "{err}");
    let message = err.to_string();
    assert!(
        message.contains("https://previous-as.example"),
        "the refusal must name the OLD authorization server: {message}"
    );
    assert!(
        message.contains(&base),
        "the refusal must name the NEW authorization server: {message}"
    );
    assert!(
        message.contains(&server_key),
        "the refusal must name the MCP server the change is about: {message}"
    );
    assert_nothing_interactive_happened(&opened, port);
}
