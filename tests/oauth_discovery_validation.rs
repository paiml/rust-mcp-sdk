//! RFC 8414 §3.3 anchor validation and the SEP-2351 ordered discovery probe.
//!
//! Uses `mockito` to drive a real authorization-server discovery endpoint with
//! no network access. The suite exists to distinguish "validated" from "not
//! validated", so the central fixture parameterises BOTH the document's
//! `"issuer"` and the JSON TYPE of its RFC 9207 flag: a suite where every
//! fixture sets `issuer == base` cannot tell the two apart, which is precisely
//! the warning sign RESEARCH Pitfall 1 records.
//!
//! Gated on `http-client`, NOT on `oauth`: `OidcDiscoveryClient` lives behind
//! `http-client`, and nothing here constructs an `OAuthHelper`. The narrower
//! gate means `make lint` (which runs `--features "full" --lib --tests`) also
//! compiles this file.

#![cfg(feature = "http-client")]

use mockito::{Mock, Server, ServerGuard};
use pmcp::client::auth::OidcDiscoveryClient;
use serde_json::{json, Value};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The tenant path every path-bearing issuer in this file uses, so the three
/// SEP-2351 candidate paths below are derivable by eye.
const TENANT: &str = "tenant1";

/// RFC 8414 §3.1 insertion, `oauth-authorization-server` suffix — candidate 1.
const CANDIDATE_1: &str = "/.well-known/oauth-authorization-server/tenant1";
/// RFC 8414 §3.1 insertion, `openid-configuration` suffix — candidate 2.
const CANDIDATE_2: &str = "/.well-known/openid-configuration/tenant1";
/// OIDC Discovery §4.1 append — candidate 3, and pmcp's only form before this
/// plan. RESEARCH Pitfall 2 measured this as the ONLY form Microsoft Entra ID
/// answers with 200.
const CANDIDATE_3: &str = "/tenant1/.well-known/openid-configuration";

/// A discovery document whose `"issuer"` and RFC 9207 flag are both free
/// variables.
///
/// `document_issuer` is what the document CLAIMS; the caller decides whether it
/// matches the issuer the URL was built from. `iss_flag` is inserted verbatim,
/// so a caller can hand it a non-boolean and drive the malformed-metadata row.
fn discovery_body(document_issuer: &str, base: &str, iss_flag: Option<Value>) -> String {
    let mut document = json!({
        "issuer": document_issuer,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if let Some(flag) = iss_flag {
        document["authorization_response_iss_parameter_supported"] = flag;
    }
    document.to_string()
}

/// The path-bearing issuer under test, and the base its endpoints are built on.
fn issuer_of(server: &ServerGuard) -> String {
    format!("{}/{TENANT}", server.url())
}

/// A mock serving 404 — the ordinary "this authorization server does not serve
/// this well-known form" answer the ordered probe exists to handle.
async fn mock_404(server: &mut ServerGuard, path: &str, expected_hits: usize) -> Mock {
    server
        .mock("GET", path)
        .with_status(404)
        .expect(expected_hits)
        .create_async()
        .await
}

/// A mock serving an HONEST, well-formed discovery document.
async fn mock_valid(server: &mut ServerGuard, path: &str, expected_hits: usize) -> Mock {
    let issuer = issuer_of(server);
    let base = server.url();
    server
        .mock("GET", path)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&issuer, &base, None))
        .expect(expected_hits)
        .create_async()
        .await
}

/// A client whose retries cost milliseconds rather than seconds.
fn fast_client() -> OidcDiscoveryClient {
    OidcDiscoveryClient::with_settings(2, Duration::from_millis(1))
}

// ---------------------------------------------------------------------------
// Group A — RFC 8414 §3.3 anchor validation (RESEARCH Pitfall 1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_document_whose_issuer_matches_the_url_it_came_from_is_accepted() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let metadata = fast_client().discover(&issuer).await.unwrap();
    assert_eq!(metadata.issuer, issuer);
}

#[tokio::test]
async fn a_document_that_lies_about_its_issuer_is_rejected_naming_both_values() {
    // The specification's own worked attack, driven end to end: everything about
    // this document is well formed EXCEPT the one field AUTH-01 later anchors
    // its RFC 9207 `iss` comparison on.
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = server
        .mock("GET", CANDIDATE_3)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body("https://honest.example", &base, None))
        .expect(1)
        .create_async()
        .await;

    let error = fast_client().discover(&issuer).await.unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(&issuer),
        "the refusal must name the EXPECTED issuer: {message}"
    );
    assert!(
        message.contains("https://honest.example"),
        "the refusal must name the DOCUMENT's issuer: {message}"
    );
}

#[tokio::test]
async fn only_the_issuer_field_decides_between_the_accepted_and_rejected_documents() {
    // The two fixtures differ in exactly one field. Without the anchor check the
    // two calls are indistinguishable, which is the whole reason this pair
    // exists rather than a single positive test.
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    let _c1 = mock_404(&mut server, CANDIDATE_1, 2).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 2).await;

    let honest = discovery_body(&issuer, &base, None);
    let lying = discovery_body("https://attacker.example", &base, None);
    assert_ne!(honest, lying);

    let _honest_mock = server
        .mock("GET", CANDIDATE_3)
        .with_status(200)
        .with_body(honest)
        .expect(1)
        .create_async()
        .await;
    let client = fast_client();
    assert!(client.discover(&issuer).await.is_ok());

    let _lying_mock = server
        .mock("GET", CANDIDATE_3)
        .with_status(200)
        .with_body(lying)
        .expect(1)
        .create_async()
        .await;
    assert!(
        client.discover(&issuer).await.is_err(),
        "the only difference between the two documents is the issuer"
    );
}

// ---------------------------------------------------------------------------
// Group B — probe order (SEP-2351)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_probe_tries_the_spec_candidates_in_order_before_the_appended_form() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    fast_client().discover(&issuer).await.unwrap();

    // Hit counts prove candidates 1 and 2 were attempted FIRST — the appended
    // form is the LAST candidate, not the only one.
    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn a_candidate_one_success_stops_the_probe_and_candidate_three_is_never_requested() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = mock_valid(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 0).await;
    let c3 = mock_404(&mut server, CANDIDATE_3, 0).await;

    fast_client().discover(&issuer).await.unwrap();

    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn when_every_candidate_fails_the_error_names_every_candidate_tried() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = mock_404(&mut server, CANDIDATE_3, 1).await;

    let message = fast_client()
        .discover(&issuer)
        .await
        .unwrap_err()
        .to_string();
    for path in [CANDIDATE_1, CANDIDATE_2, CANDIDATE_3] {
        assert!(
            message.contains(path),
            "the failure must enumerate every candidate tried, missing {path}: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Group C — TERMINAL, not fallback
//
// Each row puts a PERFECTLY VALID document behind candidate 3 with `expect(0)`.
// A fall-through would satisfy the caller and fail the test — which is the
// point: an attacker who can make candidate 1 fail in a security-relevant way
// must not be able to steer the client onto a candidate they serve.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_lying_document_aborts_the_probe_instead_of_downgrading_to_a_later_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    let _c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(200)
        .with_body(discovery_body("https://attacker.example", &base, None))
        .expect(1)
        .create_async()
        .await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 0).await;

    assert!(fast_client().discover(&issuer).await.is_err());
    c3.assert_async().await;
}

#[tokio::test]
async fn an_oversized_body_aborts_the_probe_instead_of_downgrading_to_a_later_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    // Comfortably past the 1 MiB DEFAULT_AUTH_RESPONSE_BYTES cap.
    let padding = "x".repeat(1_200_000);
    let oversized = format!(r#"{{"issuer":"{issuer}","pad":"{padding}"}}"#);
    let _c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(200)
        .with_body(oversized)
        .expect(1)
        .create_async()
        .await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 0).await;

    let message = fast_client()
        .discover(&issuer)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("1048576"),
        "the refusal must name the cap: {message}"
    );
    assert!(
        !message.contains("xxxxxxxxxx"),
        "the refusal must echo no byte of the refused body: {message}"
    );
    c3.assert_async().await;
}

#[tokio::test]
async fn a_non_boolean_iss_flag_aborts_the_probe_instead_of_downgrading_to_a_later_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    let _c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(200)
        // A STRING, not a boolean. `as_bool()` on it yields None, which reads as
        // "not advertised" and RELAXES strictness — a fail-open the abort exists
        // to prevent.
        .with_body(discovery_body(&issuer, &base, Some(json!("true"))))
        .expect(1)
        .create_async()
        .await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 0).await;

    assert!(fast_client().discover(&issuer).await.is_err());
    c3.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group D — retry, then fallback
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_five_xx_is_retried_to_the_budget_then_falls_back_to_the_next_candidate() {
    const MAX_RETRIES: usize = 3;
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(503)
        .expect(MAX_RETRIES)
        .create_async()
        .await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let client = OidcDiscoveryClient::with_settings(MAX_RETRIES, Duration::from_millis(1));
    client.discover(&issuer).await.unwrap();

    c1.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn a_two_hundred_carrying_a_non_json_body_falls_back_to_the_next_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(200)
        .with_body("<html>not a discovery document</html>")
        .expect(1)
        .create_async()
        .await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    fast_client().discover(&issuer).await.unwrap();
    c3.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group E — the per-issuer candidate cache
//
// A path-bearing issuer such as `https://login.microsoftonline.com/common/v2.0`
// otherwise pays two 404s on EVERY probe. The cache holds an INDEX, never a
// document, so all three rows below are about what the cache may NOT weaken.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_second_probe_for_the_same_issuer_requests_the_remembered_candidate_first() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    // ONE hit each across BOTH probes: the second probe must not re-pay them.
    let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 2).await;

    let client = fast_client();
    client.discover(&issuer).await.unwrap();
    client.discover(&issuer).await.unwrap();

    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn a_failing_cached_candidate_restarts_the_full_ordered_sequence_from_candidate_one() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1_first = mock_404(&mut server, CANDIDATE_1, 1).await;
    let c2_first = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3_first = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let client = fast_client();
    client.discover(&issuer).await.unwrap();
    c1_first.assert_async().await;
    c2_first.assert_async().await;
    c3_first.assert_async().await;

    // The remembered candidate has started failing, and candidate 1 now serves.
    // Falling forward to "the next index" would find nothing after index 2 and
    // fail the whole probe; restarting from candidate 1 resolves it.
    let c3_second = mock_404(&mut server, CANDIDATE_3, 1).await;
    let c1_second = mock_valid(&mut server, CANDIDATE_1, 1).await;

    client.discover(&issuer).await.unwrap();
    c3_second.assert_async().await;
    c1_second.assert_async().await;
    // Candidate 2 was never revisited: candidate 1 answered first.
    c2_first.assert_async().await;
}

#[tokio::test]
async fn a_cache_hit_still_runs_the_anchor_check() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    let _c1_first = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2_first = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3_first = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let client = fast_client();
    client.discover(&issuer).await.unwrap();

    // The remembered candidate now lies. The cache short-circuits URL CHOICE,
    // never trust — and a terminal failure must not fall through to candidate 1.
    let _c3_second = server
        .mock("GET", CANDIDATE_3)
        .with_status(200)
        .with_body(discovery_body("https://attacker.example", &base, None))
        .expect(1)
        .create_async()
        .await;
    let c1_second = mock_valid(&mut server, CANDIDATE_1, 0).await;

    assert!(client.discover(&issuer).await.is_err());
    c1_second.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group F — the RFC 9207 flag, via the new sibling type
//
// The flag reaches callers WITHOUT a field being added to
// `OidcDiscoveryMetadata`, which is all-public-field and not `#[non_exhaustive]`
// and would therefore take a MAJOR semver break to extend.
// ---------------------------------------------------------------------------

/// A mock serving an honest document carrying `flag` verbatim as its RFC 9207
/// member, or omitting the member entirely when `flag` is `None`.
async fn mock_with_iss_flag(server: &mut ServerGuard, path: &str, flag: Option<Value>) -> Mock {
    let issuer = issuer_of(server);
    let base = server.url();
    server
        .mock("GET", path)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&issuer, &base, flag))
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn discover_with_extras_reports_an_advertised_true_flag() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_with_iss_flag(&mut server, CANDIDATE_1, Some(json!(true))).await;

    let (metadata, extras) = fast_client().discover_with_extras(&issuer).await.unwrap();
    assert_eq!(metadata.issuer, issuer);
    assert_eq!(extras.iss_parameter_supported(), Some(true));
}

#[tokio::test]
async fn discover_with_extras_reports_an_advertised_false_flag() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_with_iss_flag(&mut server, CANDIDATE_1, Some(json!(false))).await;

    let (_, extras) = fast_client().discover_with_extras(&issuer).await.unwrap();
    assert_eq!(
        extras.iss_parameter_supported(),
        Some(false),
        "an explicit false is DIFFERENT from an absent key and must not collapse into None"
    );
}

#[tokio::test]
async fn discover_with_extras_reports_none_when_the_key_is_absent() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_with_iss_flag(&mut server, CANDIDATE_1, None).await;

    let (_, extras) = fast_client().discover_with_extras(&issuer).await.unwrap();
    assert_eq!(
        extras.iss_parameter_supported(),
        None,
        "absence is legal and means `not advertised`"
    );
}

#[tokio::test]
async fn every_non_boolean_iss_flag_shape_is_rejected_rather_than_read_as_none() {
    for hostile in [json!("true"), json!(1), json!(null), json!({})] {
        let mut server = Server::new_async().await;
        let issuer = issuer_of(&server);
        let _c1 = mock_with_iss_flag(&mut server, CANDIDATE_1, Some(hostile.clone())).await;

        let result = fast_client().discover_with_extras(&issuer).await;
        assert!(
            result.is_err(),
            "flag value {hostile} must abort discovery, not be read as Ok(None)"
        );
    }
}

#[tokio::test]
async fn discover_returns_exactly_what_the_extras_call_returns_minus_the_extras() {
    // `discover` keeps its ORIGINAL signature and delegates, so no existing
    // caller changes.
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_with_iss_flag(&mut server, CANDIDATE_1, Some(json!(true))).await;
    let _c1_again = mock_with_iss_flag(&mut server, CANDIDATE_1, Some(json!(true))).await;

    let client = fast_client();
    let via_extras = client.discover_with_extras(&issuer).await.unwrap().0;
    let via_discover = client.discover(&issuer).await.unwrap();

    assert_eq!(via_discover.issuer, via_extras.issuer);
    assert_eq!(
        via_discover.authorization_endpoint,
        via_extras.authorization_endpoint
    );
    assert_eq!(via_discover.token_endpoint, via_extras.token_endpoint);
}
