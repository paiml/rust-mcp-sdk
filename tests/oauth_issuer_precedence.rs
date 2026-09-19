//! Discovery-issuer PRECEDENCE: an explicitly-configured issuer outranks one
//! derived from the MCP server URL (issue #368).
//!
//! # The defect these tests fence
//!
//! `OAuthHelper::get_metadata_with_extras` used to test `mcp_server_url` FIRST,
//! so `OAuthConfig::issuer` — the value `cargo pmcp auth login --oauth-issuer`
//! and `MCP_OAUTH_ISSUER` set — was UNREACHABLE for discovery whenever a server
//! URL was present. On `auth login` that is always: the URL is a required
//! positional. The flag was therefore inert on the one verb whose help text
//! advertised it, while the failure message told the operator to "provide
//! --oauth-issuer explicitly".
//!
//! The operational consequence: for any deployment whose authorization server is
//! a third party (Cognito, Auth0, Okta, Entra), the SDK could only ever anchor
//! the RFC 8414 §3.3 comparison on the MCP server's own origin — the WRONG
//! issuer — and refused an honest document with no way for the operator to
//! correct it.
//!
//! # Oracle
//!
//! `specified`, not implicit. The expected outcomes come from RFC 8414 §3.3
//! ("identical to the authorization server's issuer identifier value into which
//! the well-known URI string was inserted") plus the documented contract of the
//! `--oauth-issuer` flag. No assertion here is a mere crash/no-crash check.
//!
//! # Why the matrix is exhaustive rather than property-based
//!
//! The precedence logic is a two-`Option` decision with four inhabitants, and
//! all four are covered below (issuer+url, url only, issuer only, neither). A
//! generator could add no case the matrix lacks. The byte-exact comparison the
//! precedence FEEDS is separately property-tested in `tests/oauth_discovery_urls.rs`.
//!
//! # The load-bearing test
//!
//! `an_explicit_issuer_does_not_exempt_its_own_document_from_the_anchor_check`
//! is the one that matters most: it proves the reordering did NOT weaken the
//! OAuth mix-up defence. Reordering WHICH issuer is the anchor must not make the
//! anchor optional. If that test ever goes green-by-omission, the fix has
//! regressed into a security hole.

#![cfg(feature = "oauth")]

use mockito::{Server, ServerGuard};
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
use serde_json::json;

/// A conformant authorization-server document: `issuer` equals the base it is
/// served from, so the RFC 8414 §3.3 anchor check passes.
///
/// Field set mirrors the known-good fixture in `tests/oauth_dcr_integration.rs`;
/// a deserialization gap in the fixture would otherwise masquerade as a
/// discovery failure and make these tests lie.
fn conformant(base: &str) -> String {
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
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

/// The shape reported in issue #368: served from the MCP host, but `issuer`
/// names a third-party IdP and the endpoints are the MCP host's own proxy.
///
/// This is what makes the derived-issuer path refuse: the document is fetched
/// from `mcp_base` but declares `idp_base`.
fn third_party_issuer_shape(mcp_base: &str, idp_base: &str) -> String {
    json!({
        "issuer": idp_base,
        "authorization_endpoint": format!("{mcp_base}/oauth2/authorize"),
        "token_endpoint": format!("{mcp_base}/oauth2/token"),
        "registration_endpoint": format!("{mcp_base}/oauth2/register"),
        "jwks_uri": format!("{idp_base}/.well-known/jwks.json"),
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

/// An authorization server that serves a conformant document on both well-known
/// forms and accepts DCR, returning `client_id`.
async fn honest_authorization_server(client_id: &str) -> (ServerGuard, String) {
    let mut server = Server::new_async().await;
    let base = server.url();
    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/openid-configuration",
    ] {
        server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(conformant(&base))
            .expect_at_least(0)
            .create_async()
            .await;
    }
    server
        .mock("POST", "/register")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(json!({ "client_id": client_id }).to_string())
        .expect_at_least(0)
        .create_async()
        .await;
    (server, base)
}

fn helper(issuer: Option<String>, mcp_server_url: Option<String>) -> OAuthHelper {
    OAuthHelper::new(OAuthConfig {
        issuer,
        mcp_server_url,
        dcr_enabled: true,
        client_id: None,
        client_name: Some("precedence-test".into()),
        ..OAuthConfig::default()
    })
    .expect("OAuthConfig is valid")
}

// ---------------------------------------------------------------------------
// The precedence matrix — all four inhabitants of (issuer, mcp_server_url)
// ---------------------------------------------------------------------------

/// Both set: the EXPLICIT issuer wins. This is the issue-#368 fix.
///
/// The MCP host serves the third-party shape that the derived path refuses; the
/// IdP serves a conformant one. Resolving the IdP's `client_id` proves discovery
/// went to the IdP and its document was accepted.
#[tokio::test]
async fn an_explicit_issuer_outranks_one_derived_from_the_mcp_server_url() {
    let (_idp, idp_base) = honest_authorization_server("registered-at-the-real-idp").await;

    let mut mcp = Server::new_async().await;
    let mcp_base = mcp.url();
    let _mcp_doc = mcp
        .mock("GET", "/.well-known/oauth-authorization-server")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(third_party_issuer_shape(&mcp_base, &idp_base))
        .expect_at_least(0)
        .create_async()
        .await;

    let resolved = helper(Some(idp_base.clone()), Some(mcp_base.clone()))
        .test_resolve_client_id_from_discovery()
        .await
        .expect("an explicitly-configured issuer must be used for discovery");

    assert_eq!(
        resolved, "registered-at-the-real-idp",
        "discovery must have run against the configured issuer {idp_base}, not the derived \
         issuer {mcp_base}"
    );
}

/// Only `mcp_server_url` set: the derived path is UNCHANGED.
///
/// Regression fence for the reordering's blind spot — the overwhelmingly common
/// same-origin case must behave exactly as before.
#[tokio::test]
async fn with_no_explicit_issuer_the_derived_path_is_unchanged() {
    let (_server, base) = honest_authorization_server("derived-path-client").await;

    let resolved = helper(None, Some(base.clone()))
        .test_resolve_client_id_from_discovery()
        .await
        .expect("the derived-issuer path must still work when no issuer is configured");

    assert_eq!(resolved, "derived-path-client");
}

/// Only `issuer` set: unchanged, and still the issuer that is used.
#[tokio::test]
async fn an_issuer_alone_is_used_for_discovery() {
    let (_server, base) = honest_authorization_server("issuer-only-client").await;

    let resolved = helper(Some(base.clone()), None)
        .test_resolve_client_id_from_discovery()
        .await
        .expect("an issuer alone must drive discovery");

    assert_eq!(resolved, "issuer-only-client");
}

/// Neither set: a configuration error naming both fields, not a panic.
#[tokio::test]
async fn neither_issuer_nor_server_url_is_a_configuration_error() {
    let error = helper(None, None)
        .test_resolve_client_id_from_discovery()
        .await
        .expect_err("a helper with no discovery seed cannot discover");

    let text = error.to_string();
    assert!(
        text.contains("oauth_issuer") && text.contains("mcp_server_url"),
        "the refusal must name both missing fields; got: {text}"
    );
}

// ---------------------------------------------------------------------------
// The security fence: precedence changed, the anchor check did NOT
// ---------------------------------------------------------------------------

/// **The load-bearing test.** An explicitly-configured issuer is still subject
/// to the byte-exact RFC 8414 §3.3 anchor check.
///
/// Configuring an issuer chooses WHICH value the anchor is, and must never make
/// the anchor OPTIONAL. Here the configured issuer serves the specification's
/// own worked attack — a document naming a different issuer than the URL it came
/// from — and it must be refused. If this test passes only because nothing was
/// checked, the mix-up defence has been silently removed.
#[tokio::test]
async fn an_explicit_issuer_does_not_exempt_its_own_document_from_the_anchor_check() {
    let mut liar = Server::new_async().await;
    let liar_base = liar.url();
    // Served from `liar_base`, but declares someone else as the issuer.
    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/openid-configuration",
    ] {
        liar.mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(conformant("https://honest.example"))
            .expect_at_least(0)
            .create_async()
            .await;
    }

    let error = helper(Some(liar_base.clone()), None)
        .test_resolve_client_id_from_discovery()
        .await
        .expect_err(
            "a document that lies about its issuer must be refused even when the issuer was \
             configured explicitly — this is the OAuth mix-up defence",
        );

    let text = error.to_string();
    assert!(
        text.contains("honest.example") && text.contains(&liar_base),
        "the refusal must name BOTH issuers so an operator can see the conflict; got: {text}"
    );
    assert!(
        text.contains("8414"),
        "the refusal must cite the rule it enforced; got: {text}"
    );
}

/// The same fence for the both-set precedence path: winning precedence does not
/// win an exemption.
#[tokio::test]
async fn an_explicit_issuer_that_lies_is_refused_even_when_a_server_url_is_also_set() {
    let mut liar = Server::new_async().await;
    let liar_base = liar.url();
    liar.mock("GET", "/.well-known/oauth-authorization-server")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(conformant("https://attacker.example"))
        .expect_at_least(0)
        .create_async()
        .await;
    liar.mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(conformant("https://attacker.example"))
        .expect_at_least(0)
        .create_async()
        .await;

    // A perfectly healthy MCP-derived authorization server is also available.
    // It must NOT be used as a silent fallback for the refused explicit issuer:
    // falling back would convert a security refusal into a downgrade.
    let (_fallback, fallback_base) = honest_authorization_server("must-not-be-used").await;

    let error = helper(Some(liar_base.clone()), Some(fallback_base))
        .test_resolve_client_id_from_discovery()
        .await
        .expect_err("a lying explicit issuer must be refused, not silently fallen back from");

    let text = error.to_string();
    assert!(
        !text.contains("must-not-be-used"),
        "the refused explicit issuer must NOT fall back to the derived one; got: {text}"
    );
    assert!(
        text.contains("attacker.example"),
        "the refusal must name the document's declared issuer; got: {text}"
    );
}

// ---------------------------------------------------------------------------
// The reporter's exact scenario, without the flag
// ---------------------------------------------------------------------------

/// Issue #368 as reported: no explicit issuer, and the MCP host serves a
/// document declaring a third-party issuer. Still refused — correctly.
///
/// This documents that the fix did not make the reported document acceptable.
/// It remains nonconformant per RFC 8414 §3.3; what changed is that the operator
/// now has a supported way to name the real issuer instead.
#[tokio::test]
async fn the_reported_third_party_shape_is_still_refused_without_an_explicit_issuer() {
    let mut mcp = Server::new_async().await;
    let mcp_base = mcp.url();
    let idp = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_example";
    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/openid-configuration",
    ] {
        mcp.mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(third_party_issuer_shape(&mcp_base, idp))
            .expect_at_least(0)
            .create_async()
            .await;
    }

    let error = helper(None, Some(mcp_base.clone()))
        .test_resolve_client_id_from_discovery()
        .await
        .expect_err("the reported document shape must still be refused");

    let text = error.to_string();
    assert!(
        text.contains(idp) && text.contains(&mcp_base),
        "the refusal must name both issuers; got: {text}"
    );
    // The remediation must name the two real fixes, since the advice this
    // replaced pointed at a candidate the probe never requests after a
    // terminal issuer mismatch.
    assert!(
        text.contains("--oauth-issuer"),
        "the refusal must point at the explicit-issuer remedy; got: {text}"
    );
    assert!(
        text.contains("--api-key") || text.contains("MCP_API_KEY"),
        "the refusal must point at the unattended/CI remedy, which bypasses discovery \
         entirely; got: {text}"
    );
}
