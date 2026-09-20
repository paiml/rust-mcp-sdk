//! RFC 8414 §2 OPTIONAL metadata fields must not be required to deserialize.
//!
//! # The defect this fences
//!
//! `OidcDiscoveryMetadata` (`src/server/auth/oauth2.rs`) declared five `Vec`
//! fields with no `#[serde(default)]` and no `Option`, so serde treated all of
//! them as required. Four of the five are OPTIONAL or RECOMMENDED under
//! RFC 8414 §2, and a conformant authorization server may omit them.
//!
//! Amazon Cognito omits exactly two — `grant_types_supported` and
//! `code_challenge_methods_supported` — so discovery against a Cognito pool
//! failed to parse a perfectly valid document. The serde error points at the
//! closing brace (a 976-byte document reported `line 1, column 976`), which is
//! the standard missing-field signature and reads confusingly like truncation.
//!
//! This surfaced immediately downstream of the #368 `--oauth-issuer` fix: with
//! the flag finally reaching discovery, the client got past the RFC 8414 §3.3
//! anchor check and straight into this. The two defects are independent, and
//! fixing the first is what made the second reachable.
//!
//! # Why the fields are NOT all defaulted to empty
//!
//! RFC 8414 §2 specifies a default for two of them, so an empty vector would be
//! a different claim than the one the spec makes:
//!
//! - `grant_types_supported` — OPTIONAL, "If omitted, the default value is
//!   `["authorization_code", "implicit"]`". `GrantType` models no `Implicit`
//!   variant (the SDK does not implement the implicit flow, deprecated by
//!   OAuth 2.1), so the default is `[AuthorizationCode]` — the representable
//!   part of the spec's default, never an empty list that would read as "this
//!   server supports no grants at all".
//! - `token_endpoint_auth_methods_supported` — OPTIONAL, "If omitted, the
//!   default is `client_secret_basic`".
//! - `scopes_supported` — RECOMMENDED, no specified default; empty is correct.
//! - `code_challenge_methods_supported` — OPTIONAL, no specified default; empty
//!   is correct, and absence means "not advertised", not "not supported".
//!   Cognito supports S256 while omitting the advertisement. That is safe here
//!   because the client never consults the field — PKCE is unconditional
//!   (`src/client/oauth.rs` always appends `code_challenge_method=S256`).
//!
//! `response_types_supported` stays REQUIRED: RFC 8414 §2 marks it REQUIRED, and
//! failing loudly on a document that omits it is correct behaviour.

#![cfg(feature = "http-client")]

use mockito::ServerGuard;
use pmcp::client::auth::OidcDiscoveryClient;

/// Serve `body` at BOTH well-known paths and return the guard with its base.
///
/// The guard is returned, never dropped inline: dropping a `ServerGuard` stops
/// the server, so a test that let it go would be probing a dead endpoint and
/// would fail for a reason unrelated to parsing.
async fn serving(build: impl FnOnce(&str) -> String) -> (ServerGuard, String) {
    let mut server = mockito::Server::new_async().await;
    let base = server.url();
    let body = build(&base);

    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/openid-configuration",
    ] {
        server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&body)
            .expect_at_least(0)
            .create_async()
            .await;
    }

    (server, base)
}

/// A Cognito-shaped `openid-configuration`: twelve fields, omitting
/// `grant_types_supported` and `code_challenge_methods_supported`.
///
/// `issuer` is the mock's own base so the RFC 8414 §3.3 anchor check passes and
/// this test can only fail on the parse.
fn cognito_shaped(base: &str) -> String {
    serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth2/authorize"),
        "token_endpoint": format!("{base}/oauth2/token"),
        "jwks_uri": format!("{base}/.well-known/jwks.json"),
        "userinfo_endpoint": format!("{base}/oauth2/userInfo"),
        "revocation_endpoint": format!("{base}/oauth2/revoke"),
        "end_session_endpoint": format!("{base}/logout"),
        "response_types_supported": ["code", "token"],
        "scopes_supported": ["openid", "email", "phone", "profile"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
    })
    .to_string()
}

/// A document missing a genuinely REQUIRED field (`issuer`), to prove the
/// relaxation did not turn the parser into one that accepts anything.
fn missing_required_issuer(base: &str) -> String {
    serde_json::json!({
        "authorization_endpoint": format!("{base}/oauth2/authorize"),
        "token_endpoint": format!("{base}/oauth2/token"),
        "response_types_supported": ["code"],
    })
    .to_string()
}

#[tokio::test]
async fn a_document_omitting_rfc_8414_optional_fields_still_parses() {
    let (_guard, base) = serving(cognito_shaped).await;

    let (metadata, _extras) = OidcDiscoveryClient::new()
        .discover_with_extras(&base)
        .await
        .expect(
            "a Cognito-shaped document omitting only RFC 8414 OPTIONAL fields must parse; a \
             serde error at the closing brace here is the missing-required-field signature",
        );

    assert_eq!(metadata.issuer, base, "the anchor field survived the parse");

    // The RFC-specified default, not an empty list: omitting the field does not
    // mean the server supports no grant types.
    assert!(
        !metadata.grant_types_supported.is_empty(),
        "grant_types_supported defaulted to an empty list; RFC 8414 §2 specifies \
         [authorization_code, implicit] when the field is absent"
    );

    // No specified default, so empty is the correct reading of "not advertised".
    assert!(
        metadata.code_challenge_methods_supported.is_empty(),
        "code_challenge_methods_supported should default to empty — the field was absent, and \
         inventing S256 would advertise a capability the server did not claim"
    );

    assert!(
        !metadata.token_endpoint_auth_methods_supported.is_empty(),
        "token_endpoint_auth_methods_supported was served and must be preserved"
    );
}

#[tokio::test]
async fn a_document_missing_a_required_field_is_still_refused() {
    let (_guard, base) = serving(missing_required_issuer).await;

    let result = OidcDiscoveryClient::new().discover_with_extras(&base).await;

    assert!(
        result.is_err(),
        "a document with no `issuer` must be refused — relaxing the OPTIONAL fields must not \
         relax the REQUIRED ones, or this suite passes on documents that cannot be used"
    );
}
