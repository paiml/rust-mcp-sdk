//! Server-side identity-provider discovery: the SEP-2351 ordered probe, the
//! RFC 8414 §3.3 issuer anchor, and bounded whole-body reads.
//!
//! `src/client/auth.rs` was brought onto all three in 116-06. The two
//! server-side providers built the same single, wrong URL by naive
//! concatenation, so fixing one of three would have left a multi-tenant `IdP`
//! broken in the other two and left two more unvalidated anchors in the tree.
//! This file is the wire-level fence for the provider half.
//!
//! # Why `GenericOidcProvider` carries the wire-level rows and `CognitoProvider`
//! does not
//!
//! `GenericOidcConfig::new` takes the issuer as a free-form string, so a
//! provider can be pointed at a `mockito` server from outside the crate.
//! `CognitoProvider::new(region, user_pool_id, client_id)` DERIVES its issuer as
//! `https://cognito-idp.{region}.amazonaws.com/{user_pool_id}`, so no public
//! constructor can be aimed at a local mock — and this plan must not add one,
//! because it creates no new public surface. Cognito's wire-level rows
//! therefore live in that module's own `#[cfg(test)]` block, where the struct
//! can be built directly; the rows reachable from outside the crate (its real
//! issuer flowing into the shared derivation, and the trailing-slash defect)
//! are here.
//!
//! Gated on `http-client`, NOT on `oauth`: `src/server/auth/mod.rs:56` gates
//! `pub mod providers` on `http-client`, and nothing here constructs an
//! `OAuthHelper`. The narrower gate means `make lint` — which runs
//! `--features "full" --lib --tests`, and `full` does NOT contain `oauth`
//! (D-116-LINT-OAUTH) — also compiles this file.

#![cfg(feature = "http-client")]

use mockito::{Mock, Server, ServerGuard};
use pmcp::server::auth::provider::IdentityProvider;
use pmcp::server::auth::providers::{GenericOidcConfig, GenericOidcProvider};
use pmcp::shared::oauth_validation::discovery_url_candidates;
use serde_json::json;

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
/// `OpenID` Connect Discovery §4.1 append — candidate 3, and this provider's
/// ONLY form before this plan. RESEARCH Pitfall 2 measured it as the only form
/// Microsoft Entra ID answers with 200, which is why it survives as the last
/// candidate rather than being replaced.
const CANDIDATE_3: &str = "/tenant1/.well-known/openid-configuration";

/// A discovery document whose `"issuer"` is a free variable.
///
/// `document_issuer` is what the document CLAIMS; the caller decides whether it
/// matches the issuer the URL was built from. Every other member is well formed,
/// so a suite that could not tell "validated" from "not validated" would accept
/// the lying fixture — which is precisely RESEARCH Pitfall 1's warning sign.
fn discovery_body(document_issuer: &str, base: &str) -> String {
    json!({
        "issuer": document_issuer,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "jwks_uri": format!("{base}/jwks"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    })
    .to_string()
}

/// The path-bearing issuer under test.
fn issuer_of(server: &ServerGuard) -> String {
    format!("{}/{TENANT}", server.url())
}

/// A mock serving 404 — the ordinary "this authorization server does not serve
/// this well-known form" answer the ordered probe exists to handle.
///
/// Every candidate this file expects to fall through is mocked EXPLICITLY:
/// `mockito` answers an unmatched request with 501, which the shared matrix
/// classifies as `Retry`, so an unmocked path would silently spend the retry
/// budget instead of falling through.
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
        .with_body(discovery_body(&issuer, &base))
        .expect(expected_hits)
        .create_async()
        .await
}

/// A mock serving a document that LIES about its issuer — the specification's
/// own worked attack, well formed in every other respect.
async fn mock_lying(server: &mut ServerGuard, path: &str, expected_hits: usize) -> Mock {
    let base = server.url();
    server
        .mock("GET", path)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body("https://honest.example", &base))
        .expect(expected_hits)
        .create_async()
        .await
}

/// A `GenericOidcProvider` config aimed at `issuer`.
fn config_for(issuer: &str) -> GenericOidcConfig {
    GenericOidcConfig::new("under-test", "Provider Under Test", issuer, "client-id")
}

// ---------------------------------------------------------------------------
// Group A — probe ORDER (SEP-2351)
//
// A fall-through and a correct probe produce the same `Ok`, so ordering is
// proven by mock hit counts and `expect(0)` guards, never by the result.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_generic_provider_probes_the_spec_candidates_before_the_appended_form() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    GenericOidcProvider::new(config_for(&issuer)).await.unwrap();

    // Hit counts prove candidates 1 and 2 were attempted FIRST. Before this
    // plan both were zero: the provider built candidate 3 and nothing else.
    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn a_candidate_one_success_stops_the_generic_probe_and_candidate_three_is_never_requested() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = mock_valid(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 0).await;
    let c3 = mock_404(&mut server, CANDIDATE_3, 0).await;

    GenericOidcProvider::new(config_for(&issuer)).await.unwrap();

    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

#[tokio::test]
async fn when_every_candidate_fails_the_generic_error_names_every_candidate_tried() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = mock_404(&mut server, CANDIDATE_3, 1).await;

    let message = GenericOidcProvider::new(config_for(&issuer))
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
// Group B — RFC 8414 §3.3 anchor validation (RESEARCH Pitfall 1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_document_whose_issuer_matches_the_url_it_came_from_is_accepted() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let provider = GenericOidcProvider::new(config_for(&issuer)).await.unwrap();
    assert_eq!(provider.discovery().await.unwrap().issuer, issuer);
}

#[tokio::test]
async fn a_generic_document_that_lies_about_its_issuer_is_rejected_naming_both_values() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let _c3 = mock_lying(&mut server, CANDIDATE_3, 1).await;

    let message = GenericOidcProvider::new(config_for(&issuer))
        .await
        .unwrap_err()
        .to_string();
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
async fn only_the_issuer_field_decides_between_the_accepted_and_rejected_generic_documents() {
    // The two fixtures differ in exactly one field. Without the anchor check the
    // two constructions are indistinguishable, which is the whole reason this
    // pair exists rather than a single positive test.
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let base = server.url();
    assert_ne!(
        discovery_body(&issuer, &base),
        discovery_body("https://attacker.example", &base)
    );

    let _c1 = mock_404(&mut server, CANDIDATE_1, 2).await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 2).await;

    let _honest = mock_valid(&mut server, CANDIDATE_3, 1).await;
    assert!(GenericOidcProvider::new(config_for(&issuer)).await.is_ok());

    let _lying = mock_lying(&mut server, CANDIDATE_3, 1).await;
    assert!(
        GenericOidcProvider::new(config_for(&issuer)).await.is_err(),
        "the only difference between the two documents is the issuer"
    );
}

// ---------------------------------------------------------------------------
// Group C — TERMINAL, not fallback
//
// Each row puts a PERFECTLY VALID document behind candidate 3 with `expect(0)`.
// A fall-through would satisfy the caller and fail the test — which is the
// point: an attacker who can make candidate 1 fail in a security-relevant way
// must not be able to steer the provider onto a candidate they serve.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_lying_document_aborts_the_generic_probe_instead_of_downgrading_to_a_later_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let _c1 = mock_lying(&mut server, CANDIDATE_1, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 0).await;

    assert!(GenericOidcProvider::new(config_for(&issuer)).await.is_err());
    c3.assert_async().await;
}

#[tokio::test]
async fn an_oversized_body_aborts_the_generic_probe_instead_of_downgrading_to_a_later_candidate() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    // Comfortably past the 1 MiB DEFAULT_AUTH_RESPONSE_BYTES cap, and otherwise
    // a perfectly honest document.
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

    let message = GenericOidcProvider::new(config_for(&issuer))
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
async fn a_cross_origin_discovery_redirect_is_not_followed_by_the_generic_provider() {
    let mut origin = Server::new_async().await;
    let mut elsewhere = Server::new_async().await;

    // A SECOND mockito server is a genuinely different origin (its port
    // differs), so `expect(0)` proves the redirect was not followed rather than
    // merely that the call failed.
    let never = elsewhere
        .mock("GET", CANDIDATE_1)
        .with_status(200)
        .with_body("SHOULD NOT BE FETCHED")
        .expect(0)
        .create_async()
        .await;
    let issuer = issuer_of(&origin);
    let _c1 = origin
        .mock("GET", CANDIDATE_1)
        .with_status(302)
        .with_header("location", &format!("{}{CANDIDATE_1}", elsewhere.url()))
        .expect(1)
        .create_async()
        .await;
    // A refused redirect is a statement about who would have AUTHORED the
    // document, so it is TERMINAL: candidates 2 and 3 are never reached either.
    let c3 = mock_valid(&mut origin, CANDIDATE_3, 0).await;

    assert!(GenericOidcProvider::new(config_for(&issuer)).await.is_err());
    never.assert_async().await;
    c3.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group D — retry, then fallback
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_five_xx_is_retried_to_the_budget_then_falls_back_to_the_next_candidate() {
    // The provider's retry budget is 3 attempts against ONE candidate. The
    // `expect(3)` is what distinguishes "retried then fell back" from "fell
    // back immediately" — both of which end in the same successful `Ok`.
    const ATTEMPTS: usize = 3;
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = server
        .mock("GET", CANDIDATE_1)
        .with_status(503)
        .expect(ATTEMPTS)
        .create_async()
        .await;
    let _c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    GenericOidcProvider::new(config_for(&issuer)).await.unwrap();

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

    GenericOidcProvider::new(config_for(&issuer)).await.unwrap();
    c3.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group E — the provider's own TTL cache still short-circuits
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_generic_ttl_cache_still_short_circuits_the_ordered_probe() {
    let mut server = Server::new_async().await;
    let issuer = issuer_of(&server);
    let c1 = mock_404(&mut server, CANDIDATE_1, 1).await;
    let c2 = mock_404(&mut server, CANDIDATE_2, 1).await;
    // ONE hit across the construction AND two further `discovery()` calls: the
    // TTL cache sits directly above the fetch and must keep short-circuiting.
    let c3 = mock_valid(&mut server, CANDIDATE_3, 1).await;

    let provider = GenericOidcProvider::new(config_for(&issuer)).await.unwrap();
    provider.discovery().await.unwrap();
    provider.discovery().await.unwrap();

    c1.assert_async().await;
    c2.assert_async().await;
    c3.assert_async().await;
}

// ---------------------------------------------------------------------------
// Group F — the shared derivation, reached through the REAL providers
//
// These rows need no network. They exist because the defect this plan closes is
// URL ARITHMETIC, and the arithmetic is now owned by one shared function that
// both providers must be shown to feed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_cognito_issuer_flows_into_the_shared_ordered_derivation() {
    use pmcp::server::auth::providers::CognitoProvider;

    let provider = CognitoProvider::new("us-east-1", "us-east-1_ABC123", "client-id")
        .await
        .unwrap();
    let issuer = provider.issuer().to_string();
    assert_eq!(
        issuer,
        "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123"
    );

    let rendered: Vec<String> = discovery_url_candidates(&issuer)
        .unwrap()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    assert_eq!(
        rendered,
        vec![
            "https://cognito-idp.us-east-1.amazonaws.com/.well-known/oauth-authorization-server/us-east-1_ABC123",
            "https://cognito-idp.us-east-1.amazonaws.com/.well-known/openid-configuration/us-east-1_ABC123",
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123/.well-known/openid-configuration",
        ],
        "a Cognito user pool is a PATH-bearing issuer, so it needs all three candidates"
    );
}

#[test]
fn a_trailing_slash_issuer_produces_no_doubled_slash_in_any_candidate() {
    // `cognito.rs:270` built `format!("{}/.well-known/openid-configuration",
    // self.issuer)` with NO `trim_end_matches('/')`, so a trailing-slash issuer
    // produced `.../..//.well-known/openid-configuration`. Routing through the
    // shared derivation fixes it; this row is what keeps it fixed.
    for issuer in [
        "https://as.example/",
        "https://as.example/tenant1/",
        "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123/",
    ] {
        for candidate in discovery_url_candidates(issuer).unwrap() {
            let rendered = candidate.to_string();
            let after_scheme = rendered.trim_start_matches("https://");
            assert!(
                !after_scheme.contains("//"),
                "issuer {issuer} produced a doubled slash: {rendered}"
            );
        }
    }
}

#[test]
fn a_path_bearing_issuer_derives_three_candidates_not_one_appended_url() {
    let rendered: Vec<String> = discovery_url_candidates("https://idp.example/tenant1")
        .unwrap()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    assert_eq!(
        rendered,
        vec![
            "https://idp.example/.well-known/oauth-authorization-server/tenant1",
            "https://idp.example/.well-known/openid-configuration/tenant1",
            "https://idp.example/tenant1/.well-known/openid-configuration",
        ]
    );
}
