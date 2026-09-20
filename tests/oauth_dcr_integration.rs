//! Integration tests for Dynamic Client Registration (RFC 7591) in `OAuthHelper`.
//!
//! Uses mockito to simulate a real OAuth discovery server + DCR endpoint
//! without needing network access. Covers:
//! - RFC 7591 §3.1 `response_types: ["code"]` must appear in the wire body
//! - Scheme guard: `http://`-non-localhost `registration_endpoint` rejected
//! - DCR response body capped at 1 MiB
//! - **SEP-837** (116-10): a derived `application_type` on the wire
//! - **SEP-2207** (116-10): `refresh_token` in `grant_types`, and
//!   `offline_access` declared as client metadata AND requested at the
//!   authorization request — both only when the server advertises it
//!
//! # Why these assertions are made on the WIRE
//!
//! `application_type` is carried in `DcrRequest`'s `#[serde(flatten)] extra`
//! map (116-03), whose entire purpose is to produce bytes byte-identical to a
//! declared serde field. Only a wire assertion proves that it did. The
//! `Matcher::PartialJsonString` idiom is what makes them detectors rather than
//! decoration: the mock matches ONLY when the body contains the asserted
//! fields, so a regression that drops one yields a mockito 501 and a red test
//! rather than a quietly weaker request.
//!
//! Two of the rows below are ABSENCE assertions, which partial JSON cannot
//! express directly. They are written as exact-value matches on `scope` — a
//! single space-joined string, so `"openid"` matches only when `offline_access`
//! is NOT there.

#![cfg(feature = "oauth")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mockito::{Matcher, Server};
use pmcp::client::oauth::{BrowserLauncher, DcrRequest, OAuthConfig, OAuthHelper};
use pmcp::shared::oauth_validation::{derive_application_type, ApplicationType};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

fn discovery_body(base: &str, with_reg: bool) -> String {
    let mut v = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_reg {
        v["registration_endpoint"] = json!(format!("{}/register", base));
    }
    v.to_string()
}

#[tokio::test]
async fn dcr_fires_when_eligible() {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _m_disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, /*with_reg*/ true))
        .create_async()
        .await;

    // Mock only matches when RFC 7591 §3.1 `response_types` is in the body;
    // a regression that drops the field will produce a 501 and fail the test.
    let _m_dcr = server
        .mock("POST", "/register")
        .match_header("content-type", Matcher::Regex("application/json.*".into()))
        .match_body(Matcher::PartialJsonString(
            json!({ "response_types": ["code"] }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "dcr-issued-id"}).to_string())
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("integration-test".into()),
        ..OAuthConfig::default()
    };
    let helper = OAuthHelper::new(cfg).unwrap();

    let resolved = helper
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
    assert_eq!(resolved, "dcr-issued-id");
}

#[tokio::test]
async fn dcr_body_matches_rfc7591() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({
                "grant_types": ["authorization_code"],
                "token_endpoint_auth_method": "none",
                "response_types": ["code"],
            })
            .to_string(),
        ))
        .with_status(200)
        .with_body(json!({"client_id": "x"}).to_string())
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("assert-body".into()),
        ..OAuthConfig::default()
    };
    OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
}

#[tokio::test]
async fn dcr_rejects_http_non_localhost_registration_endpoint_against_live_mock() {
    // Mock server's discovery advertises a non-localhost http registration
    // endpoint and expects ZERO calls to /register — confirms the SDK rejects
    // the URL before issuing any HTTP request.
    let mut server = Server::new_async().await;
    let base = server.url();

    // Discovery advertises a hostile non-localhost http registration endpoint.
    let discovery = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        "registration_endpoint": "http://evil.invalid/register",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery.to_string())
        .create_async()
        .await;

    // Guard: expect ZERO calls to any /register path on our mock server
    // (the SDK must not even attempt the POST).
    let reg_guard = server
        .mock("POST", "/register")
        .expect(0)
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("regression-t74a".into()),
        ..OAuthConfig::default()
    };
    let err = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("must be https"),
        "expected scheme-guard error, got: {msg}"
    );
    reg_guard.assert_async().await;
}

#[tokio::test]
async fn dcr_not_fired_when_client_id_present() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, true))
        .create_async()
        .await;
    let reg_mock = server
        .mock("POST", "/register")
        .with_body(json!({"client_id": "SHOULD-NOT-BE-USED"}).to_string())
        .expect(0) // asserts zero calls
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: Some("preset".into()),
        ..OAuthConfig::default()
    };
    let resolved = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
    assert_eq!(resolved, "preset");
    reg_mock.assert_async().await;
}

#[tokio::test]
async fn dcr_rejects_response_larger_than_1mib() {
    // Defense-in-depth: the SDK caps DCR response bodies at 1 MiB to mitigate
    // DoS from a hostile registration_endpoint.
    let mut server = Server::new_async().await;
    let base = server.url();

    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, /*with_reg*/ true))
        .create_async()
        .await;

    // Build a valid-JSON but >1 MiB response body.
    let mut huge = String::with_capacity(1_200_000);
    huge.push_str(r#"{"client_id":"x","extra_padding":""#);
    huge.push_str(&"A".repeat(1_200_000));
    huge.push_str(r#""}"#);

    let _r = server
        .mock("POST", "/register")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(huge)
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("oversize-body-test".into()),
        ..OAuthConfig::default()
    };
    let err = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("exceeds") && msg.contains("byte cap"),
        "expected 1 MiB cap rejection, got: {msg}"
    );
}

// ===========================================================================
// SEP-837 / SEP-2207 harness (116-10)
// ===========================================================================

/// A discovery document parameterised on what the authorization server
/// ADVERTISES in `scopes_supported` — which is the condition SEP-2207 places on
/// requesting `offline_access` at all.
fn discovery_advertising(base: &str, scopes_supported: &[&str], with_reg: bool) -> String {
    let mut doc = json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": scopes_supported,
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_reg {
        doc["registration_endpoint"] = json!(format!("{base}/register"));
    }
    doc.to_string()
}

/// An ephemeral loopback port, so concurrently-running rows cannot collide on
/// the callback listener.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// A [`BrowserLauncher`] that CAPTURES the authorization URL and then declines
/// to deliver it, so the flow aborts immediately instead of waiting five
/// minutes for a callback nobody will send.
///
/// The trait's contract reserves `Err` for "the URL could not be delivered to a
/// human at all", which is exactly true of this launcher.
#[derive(Debug)]
struct CaptureOnlyLauncher {
    captured: Arc<Mutex<Option<String>>>,
}

impl BrowserLauncher for CaptureOnlyLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        *self.captured.lock().expect("captured") = Some(url.to_string());
        Err(pmcp::Error::internal(
            "captured for assertion; not delivering".to_string(),
        ))
    }
}

/// Run a flow far enough to build the authorization URL, and return that URL.
///
/// The `client_id` is preset and DCR disabled, so this exercises the
/// AUTHORIZATION-REQUEST stage in isolation from the registration stage.
async fn captured_authorization_url(scopes_supported: &[&str], configured: &[&str]) -> String {
    let mut server = Server::new_async().await;
    let base = server.url();
    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, scopes_supported, false))
        .create_async()
        .await;

    let captured = Arc::new(Mutex::new(None));
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("preset-for-auth-url".into()),
        dcr_enabled: false,
        scopes: configured.iter().map(|s| (*s).to_string()).collect(),
        redirect_port: free_port(),
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(Arc::new(CaptureOnlyLauncher {
        captured: captured.clone(),
    }));

    // The launcher refuses, so this resolves promptly and returns `Err`. The URL
    // was already built and captured by then, which is the whole point.
    let _ = helper.authorize_with_details().await;

    let url = captured
        .lock()
        .expect("captured")
        .clone()
        .expect("the flow must have built and offered an authorization URL");
    url
}

/// The `scope` query parameter of an authorization URL, as a token list.
fn scope_tokens(authorization_url: &str) -> Vec<String> {
    Url::parse(authorization_url)
        .expect("a parseable authorization URL")
        .query_pairs()
        .find(|(k, _)| k == "scope")
        .map(|(_, v)| {
            v.split_whitespace()
                .map(std::string::ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A [`BrowserLauncher`] that completes the flow: it lifts the `state` out of
/// the captured URL and delivers a legitimate callback to the loopback listener
/// the flow has already bound. Ported from `tests/oauth_iss_integration.rs`.
#[derive(Debug)]
struct CallbackDrivingLauncher {
    port: u16,
}

impl BrowserLauncher for CallbackDrivingLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        let state = Url::parse(url)
            .ok()
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "state")
                    .map(|(_, v)| v.into_owned())
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

/// Drive one COMPLETE authorization-code flow and return its result.
///
/// `token_scope` is the `scope` the token endpoint echoes; `None` OMITS the
/// parameter, which is the RFC 6749 §5.1 row.
async fn completed_flow(
    scopes_supported: &[&str],
    configured: &[&str],
    token_scope: Option<&str>,
) -> pmcp::Result<pmcp::client::oauth::AuthorizationResult> {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, scopes_supported, false))
        .create_async()
        .await;

    let mut token_body = json!({
        "access_token": "granted-access-token",
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": "granted-refresh-token",
    });
    if let Some(scope) = token_scope {
        token_body["scope"] = json!(scope);
    }

    let _token = server
        .mock("POST", "/token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(token_body.to_string())
        .create_async()
        .await;

    let port = free_port();
    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("granted-scope-row".into()),
        dcr_enabled: false,
        scopes: configured.iter().map(|s| (*s).to_string()).collect(),
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(Arc::new(CallbackDrivingLauncher { port }));

    let outcome = helper.authorize_with_details().await;
    // Let the driving task finish reading before the mock server is dropped.
    tokio::time::sleep(Duration::from_millis(20)).await;
    outcome
}

// ---------------------------------------------------------------------------
// Group A — the DCR wire body (SEP-837 application_type, SEP-2207 grants)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_dcr_wire_body_carries_the_derived_native_application_type() {
    // pmcp registers `http://127.0.0.1:{port}/callback` (RFC 8252 §7.3), so the
    // derivation is `native`. Omitting the parameter would default to `web`
    // under OIDC — the exact contradiction SEP-837 exists to stop, and the one
    // that gets a loopback registration rejected.
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, &["openid"], true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({ "application_type": "native" }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "native-issued"}).to_string())
        .create_async()
        .await;

    let resolved = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("application-type-wire".into()),
        ..OAuthConfig::default()
    })
    .expect("helper")
    .test_resolve_client_id_from_discovery()
    .await
    .expect("the mock matches only when application_type=\"native\" is on the wire");
    assert_eq!(resolved, "native-issued");
}

#[tokio::test]
async fn the_dcr_wire_body_declares_the_refresh_token_grant() {
    // SEP-2207: an authorization server that was never told the client wants a
    // refresh grant has every reason not to issue a refresh token. The array is
    // asserted in full, in order, so dropping either entry fails the match.
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, &["openid"], true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({ "grant_types": ["authorization_code", "refresh_token"] }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "refresh-capable"}).to_string())
        .create_async()
        .await;

    let resolved = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("grant-types-wire".into()),
        ..OAuthConfig::default()
    })
    .expect("helper")
    .test_resolve_client_id_from_discovery()
    .await
    .expect("the mock matches only when refresh_token is declared");
    assert_eq!(resolved, "refresh-capable");
}

#[tokio::test]
async fn the_dcr_wire_body_registers_offline_access_when_the_server_advertises_it() {
    // This is CLIENT METADATA: it declares what the client may ask for. The
    // asking happens at the authorization request, which the Group B rows cover.
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(
            &base,
            &["openid", "offline_access"],
            true,
        ))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({ "scope": "openid offline_access" }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "offline-capable"}).to_string())
        .create_async()
        .await;

    let resolved = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("offline-metadata".into()),
        scopes: vec!["openid".into()],
        ..OAuthConfig::default()
    })
    .expect("helper")
    .test_resolve_client_id_from_discovery()
    .await
    .expect("the mock matches only when offline_access is registered");
    assert_eq!(resolved, "offline-capable");
}

#[tokio::test]
async fn the_dcr_wire_body_omits_offline_access_when_the_server_does_not_advertise_it() {
    // An ABSENCE assertion. `scope` is one space-joined string, so an exact
    // match on `"openid"` succeeds ONLY when `offline_access` was not appended:
    // an unconditional implementation sends `"openid offline_access"`, the mock
    // stops matching, mockito answers 501 and this row goes red.
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, &["openid", "profile"], true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({ "scope": "openid" }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "no-offline"}).to_string())
        .create_async()
        .await;

    let resolved = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("no-offline-metadata".into()),
        scopes: vec!["openid".into()],
        ..OAuthConfig::default()
    })
    .expect("helper")
    .test_resolve_client_id_from_discovery()
    .await
    .expect("the mock matches only when scope is exactly \"openid\"");
    assert_eq!(resolved, "no-offline");
}

// ---------------------------------------------------------------------------
// Group B — the AUTHORIZATION REQUEST, the stage at which asking means anything
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_authorization_url_requests_offline_access_when_the_server_advertises_it() {
    let url = captured_authorization_url(&["openid", "offline_access"], &["openid"]).await;
    assert_eq!(
        scope_tokens(&url),
        vec!["openid".to_string(), "offline_access".to_string()],
        "the authorization request is the only stage at which requesting \
         offline_access does anything; got {url}"
    );
}

#[tokio::test]
async fn the_authorization_url_omits_offline_access_when_the_server_does_not_advertise_it() {
    let url = captured_authorization_url(&["openid", "profile"], &["openid"]).await;
    let tokens = scope_tokens(&url);
    assert_eq!(tokens, vec!["openid".to_string()]);
    assert!(
        !tokens.iter().any(|t| t == "offline_access"),
        "SEP-2207 conditions the request on the server advertising support: {url}"
    );
}

#[tokio::test]
async fn two_consecutive_flows_do_not_accumulate_scopes_on_the_shared_config() {
    // `OAuthConfig::scopes` is a PUBLIC field on a public struct. A caller who
    // reuses one config across two flows against an advertising server must not
    // watch it grow an `offline_access` per flow — which is what an in-place
    // `push` would produce, and which the single-flow rows above cannot see.
    let mut server = Server::new_async().await;
    let base = server.url();
    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(
            &base,
            &["openid", "offline_access"],
            false,
        ))
        .expect_at_least(1)
        .create_async()
        .await;

    let config = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("accumulation-check".into()),
        dcr_enabled: false,
        scopes: vec!["openid".into()],
        redirect_port: free_port(),
        ..OAuthConfig::default()
    };

    let mut urls = Vec::new();
    for _ in 0..2 {
        let captured = Arc::new(Mutex::new(None));
        let helper = OAuthHelper::new(config.clone())
            .expect("helper")
            .with_browser_launcher(Arc::new(CaptureOnlyLauncher {
                captured: captured.clone(),
            }));
        let _ = helper.authorize_with_details().await;
        urls.push(captured.lock().expect("captured").clone().expect("a URL"));
    }

    assert_eq!(
        scope_tokens(&urls[0]),
        scope_tokens(&urls[1]),
        "two flows over one config must request the SAME scope, not a longer one"
    );
    assert_eq!(
        scope_tokens(&urls[1]),
        vec!["openid".to_string(), "offline_access".to_string()]
    );
    assert_eq!(
        config.scopes,
        vec!["openid".to_string()],
        "the caller's config must be untouched after both flows"
    );
}

// ---------------------------------------------------------------------------
// Group C — the GRANTED scope (RFC 6749 §5.1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_token_response_with_a_scope_records_exactly_what_was_granted() {
    // The request asked for three scopes and the server granted two. Recording
    // the REQUEST here would tell every later refresh it holds a scope it does
    // not (T-116-38b).
    let result = completed_flow(
        &["openid", "profile", "email", "offline_access"],
        &["openid", "profile", "email"],
        Some("openid profile"),
    )
    .await
    .expect("the happy path completes");

    assert_eq!(
        result.scopes,
        vec!["openid".to_string(), "profile".to_string()],
        "the token response's `scope` IS the granted scope, even when narrower \
         than the request"
    );
    assert!(
        !result.scopes.iter().any(|s| s == "offline_access"),
        "offline_access was REQUESTED but not granted, so it must not be recorded"
    );
}

#[tokio::test]
async fn a_token_response_without_a_scope_records_the_requested_scope_rfc6749_5_1() {
    // RFC 6749 §5.1: the `scope` parameter is OPTIONAL "if identical to the
    // scope requested by the client". An omission therefore means the request
    // was granted IN FULL — including the `offline_access` this flow added
    // because the server advertised it.
    let result = completed_flow(&["openid", "offline_access"], &["openid"], None)
        .await
        .expect("the happy path completes");

    assert_eq!(
        result.scopes,
        vec!["openid".to_string(), "offline_access".to_string()],
        "an omitted `scope` means the REQUESTED scope was granted — and the \
         request included offline_access, so recording only `config.scopes` \
         would silently narrow every subsequent refresh"
    );
}

// ---------------------------------------------------------------------------
// Group D — the override, and the `web` derivation pmcp's own flow cannot reach
// ---------------------------------------------------------------------------

#[test]
fn an_explicit_application_type_reaches_the_wire_instead_of_the_derivation() {
    // D-09's documented override path, asserted where it is observable: the
    // serialized bytes. The redirect URI here DERIVES `native`, so a wire body
    // carrying `"web"` can only be the override surviving.
    let mut request = DcrRequest {
        redirect_uris: vec!["http://127.0.0.1:8080/callback".into()],
        client_name: Some("override".into()),
        client_uri: None,
        logo_uri: None,
        contacts: vec![],
        token_endpoint_auth_method: Some("none".into()),
        grant_types: vec!["authorization_code".into(), "refresh_token".into()],
        response_types: vec!["code".into()],
        scope: None,
        software_id: None,
        software_version: None,
        extra: Default::default(),
    };
    assert_eq!(
        derive_application_type(&request.redirect_uris).expect("derives"),
        ApplicationType::Native,
        "the derivation for this URI is `native`, which is what makes the \
         override observable"
    );

    request.set_application_type("web");

    let wire = serde_json::to_value(&request).expect("serializes");
    assert_eq!(
        wire["application_type"], "web",
        "the override, not the derivation"
    );
    let text = serde_json::to_string(&request).expect("serializes");
    assert_eq!(
        text.matches("application_type").count(),
        1,
        "exactly one application_type key may reach the wire: {text}"
    );
}

#[test]
fn an_https_non_loopback_registration_derives_web() {
    // pmcp's own flow hardcodes a loopback redirect, so this row is unreachable
    // end to end. A platform `oauth-proxy` registering an https redirect is the
    // real caller, and it must register as `web`.
    let redirect_uris = vec!["https://proxy.example.com/oauth/callback".to_string()];
    let derived = derive_application_type(&redirect_uris).expect("https non-loopback derives");
    assert_eq!(derived, ApplicationType::Web);
    assert_eq!(derived.as_str(), "web");

    let mut request = DcrRequest {
        redirect_uris,
        client_name: Some("oauth-proxy".into()),
        client_uri: None,
        logo_uri: None,
        contacts: vec![],
        token_endpoint_auth_method: Some("none".into()),
        grant_types: vec!["authorization_code".into(), "refresh_token".into()],
        response_types: vec!["code".into()],
        scope: None,
        software_id: None,
        software_version: None,
        extra: Default::default(),
    };
    request.set_application_type(derived.as_str());
    let wire = serde_json::to_value(&request).expect("serializes");
    assert_eq!(wire["application_type"], "web");
}

// ---------------------------------------------------------------------------
// Group E — the RESPONSE half: echo divergence (never fatal) and an actionable
// rejection
// ---------------------------------------------------------------------------

/// Drive DCR against a registration endpoint that answers with `status` and
/// `body`, and return whatever the resolver produced.
///
/// The discovery document advertises a `registration_endpoint`, `client_id` is
/// absent and `dcr_enabled` is on, so the resolver always reaches `/register`.
async fn dcr_against(status: usize, body: String) -> pmcp::Result<String> {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_advertising(&base, &["openid"], true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("dcr-response-half".into()),
        ..OAuthConfig::default()
    })
    .expect("helper")
    .test_resolve_client_id_from_discovery()
    .await
}

#[tokio::test]
async fn an_echoed_application_type_that_diverges_still_registers_the_client() {
    // The load-bearing behaviour is "never fatal". RFC 7591 § 3.2.1 explicitly
    // permits the authorization server to modify requested client metadata, so
    // failing here would turn a legal server behaviour into an outage
    // (T-116-36, disposition `accept`). `native` was sent; `web` comes back;
    // registration must still SUCCEED.
    //
    // The warning itself is pinned by the inline unit tests of
    // `application_type_divergence`, not by capturing a log line — see that
    // module's doc for why.
    let resolved = dcr_against(
        201,
        json!({ "client_id": "registered-as-web", "application_type": "web" }).to_string(),
    )
    .await
    .expect("a divergent echo is a WARNING, never a registration failure");
    assert_eq!(resolved, "registered-as-web");
}

#[tokio::test]
async fn an_omitted_application_type_echo_is_not_divergence_and_registration_succeeds() {
    // RFC 7591 does not require the server to echo accepted metadata, so an
    // omission is "no answer" and must not be reported as a disagreement.
    let resolved = dcr_against(201, json!({ "client_id": "terse-server" }).to_string())
        .await
        .expect("an absent echo is not divergence");
    assert_eq!(resolved, "terse-server");
}

#[tokio::test]
async fn an_identical_application_type_echo_registers_without_incident() {
    let resolved = dcr_against(
        201,
        json!({ "client_id": "agreed", "application_type": "native" }).to_string(),
    )
    .await
    .expect("an agreeing server is the ordinary case");
    assert_eq!(resolved, "agreed");
}

#[tokio::test]
async fn a_rejected_registration_names_the_status_the_sent_type_and_the_sent_redirect_uri() {
    // SEP-837 ¶3: "When a registration request is rejected, clients SHOULD
    // surface a meaningful error to the user or developer." A bare status does
    // not tell a developer which knob to turn; the `application_type` and the
    // `redirect_uris` are exactly the pair the server enforced its constraints
    // over, so both are named.
    let err = dcr_against(
        400,
        json!({
            "error": "invalid_redirect_uri",
            "error_description": "loopback redirect URIs are not permitted for this client",
        })
        .to_string(),
    )
    .await
    .expect_err("a 400 is a rejection");
    let msg = err.to_string();

    for expected in [
        "400",
        "invalid_redirect_uri",
        "loopback redirect URIs are not permitted",
        "application_type=\"native\"",
        "127.0.0.1",
        "pre-registered client_id",
    ] {
        assert!(
            msg.contains(expected),
            "the rejection must name {expected:?}; got: {msg}"
        );
    }
}

#[tokio::test]
async fn a_rejected_registration_does_not_echo_unparsed_fields_of_the_body() {
    // T-116-37: the rejection path must not become the echo channel the bounded
    // read exists to close. Only `error` and `error_description` are reproduced;
    // everything else in an entirely server-controlled body is dropped.
    const CANARY: &str = "CANARY-9c2e41-DO-NOT-DISCLOSE";
    let err = dcr_against(
        422,
        json!({
            "error": "invalid_client_metadata",
            "internal_trace": CANARY,
            "stack": [CANARY, CANARY],
        })
        .to_string(),
    )
    .await
    .expect_err("a 422 is a rejection");
    let msg = err.to_string();

    assert!(msg.contains("422"), "names the status: {msg}");
    assert!(
        msg.contains("invalid_client_metadata"),
        "names the server's own reason: {msg}"
    );
    assert!(
        !msg.contains(CANARY),
        "no body content beyond the two parsed error fields may reach the message: {msg}"
    );
}

#[tokio::test]
async fn a_rejected_registration_with_a_non_json_body_reproduces_none_of_it() {
    const CANARY: &str = "CANARY-4b77f0-DO-NOT-DISCLOSE";
    let err = dcr_against(
        503,
        format!("<html><body><h1>503</h1><p>{CANARY}</p></body></html>"),
    )
    .await
    .expect_err("a 503 is a rejection");
    let msg = err.to_string();

    assert!(msg.contains("503"), "names the status: {msg}");
    assert!(
        !msg.contains(CANARY),
        "an unparseable body is dropped entirely, not echoed: {msg}"
    );
    assert!(
        msg.contains("application_type=\"native\"") && msg.contains("127.0.0.1"),
        "and the message is still actionable without it: {msg}"
    );
}

#[tokio::test]
async fn a_rejected_registration_truncates_an_oversized_error_description() {
    // The body is capped at 1 MiB, but a 1 MiB `error_description` is still an
    // echo channel wearing a specification-approved hat.
    let long = "Q".repeat(5_000);
    let err = dcr_against(
        400,
        json!({ "error": "invalid_redirect_uri", "error_description": long }).to_string(),
    )
    .await
    .expect_err("a 400 is a rejection");
    let msg = err.to_string();

    assert!(
        msg.contains("characters withheld"),
        "the truncation must be announced: {msg}"
    );
    assert!(
        msg.len() < 2_000,
        "a 5000-character description must not reach the message in full ({} bytes)",
        msg.len()
    );
}

#[tokio::test]
async fn a_rejection_body_over_the_cap_is_refused_without_reproducing_it() {
    // The rejection body is read through the SAME bounded reader as the success
    // body. Before 116-10 this path called `response.text()` with no cap at all
    // and then interpolated the whole thing into the error.
    const CANARY: &str = "CANARY-e01d55-DO-NOT-DISCLOSE";
    let mut huge = String::with_capacity(1_300_000);
    huge.push_str(r#"{"error":"invalid_client_metadata","error_description":""#);
    huge.push_str(CANARY);
    huge.push_str(&"B".repeat(1_300_000));
    huge.push_str(r#""}"#);

    let err = dcr_against(400, huge)
        .await
        .expect_err("an over-cap rejection body is refused");
    let msg = err.to_string();

    assert!(
        msg.contains("1048576") && msg.contains("cap"),
        "the refusal names the cap it enforced: {msg}"
    );
    assert!(
        !msg.contains(CANARY),
        "and reproduces no byte of the refused body: {msg}"
    );
}
