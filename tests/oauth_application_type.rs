//! ALWAYS coverage for D-10's `application_type` derivation from
//! `redirect_uris` (`pmcp::shared::oauth_validation::derive_application_type`),
//! the value SEP-837 makes MCP clients MUST send at Dynamic Client
//! Registration.
//!
//! Rows covered, in the order they appear below:
//!   1. The four `native` classifications: the three loopback spellings and a
//!      custom (private-use) scheme.
//!   2. The two `web` classifications: a single https redirect and a uniform
//!      multi-entry vector.
//!   3. The three hard errors: a MIXED vector, an empty vector and an
//!      unparseable URI. D-10 is explicit that a mixed vector is an ERROR and
//!      never a silent pick, so each refusal is asserted to name what a
//!      developer needs in order to act on it.
//!   4. The exact wire literals `"native"` and `"web"`.
//!   5. A property over single-element inputs asserting determinism and the
//!      absence of panics.
//!
//! **The wire literals are asserted explicitly and by hand.** A silent change
//! from `"native"` to `"Native"` would be accepted by every authorization
//! server as "unknown value, ignore", would fail no round trip, and would
//! silently reinstate OIDC's `web` default for a CLI whose redirect URI is
//! `http://127.0.0.1:{port}/callback` — which is the exact conflict SEP-837
//! exists to prevent.
//!
//! **This file is deliberately NOT `#![cfg(feature = "oauth")]`.** The tier
//! under test is ungated, so it must also run under a plain `--features full`
//! build, the feature set `make quality-gate` uses.

use pmcp::shared::oauth_validation::{derive_application_type, ApplicationType};
use proptest::prelude::*;

fn derive(uris: &[&str]) -> pmcp::Result<ApplicationType> {
    let owned: Vec<String> = uris.iter().map(|uri| (*uri).to_string()).collect();
    derive_application_type(&owned)
}

fn refusal(uris: &[&str]) -> String {
    derive(uris)
        .expect_err("this redirect_uris vector must be refused")
        .to_string()
}

// ===========================================================================
// 1. `native` — RFC 8252's loopback redirects and private-use schemes.
// ===========================================================================

/// pmcp's own DCR call hardcodes `http://127.0.0.1:{port}/callback`
/// (`src/client/oauth.rs:239`), with the literal IPv4 chosen per RFC 8252
/// §7.3. That is the single most important row in this file: it is the value
/// 116-10 will actually send.
#[test]
fn the_ipv4_loopback_pmcp_itself_registers_is_native() {
    assert_eq!(
        derive(&["http://127.0.0.1:8080/callback"]).expect("a loopback redirect is native"),
        ApplicationType::Native
    );
}

#[test]
fn the_localhost_name_is_native() {
    assert_eq!(
        derive(&["http://localhost:8080/callback"]).expect("a loopback redirect is native"),
        ApplicationType::Native
    );
}

/// The bracketed IPv6 authority form, which a browser produces when it
/// resolves `localhost` to `::1`.
#[test]
fn the_ipv6_loopback_is_native() {
    assert_eq!(
        derive(&["http://[::1]:8080/callback"]).expect("a loopback redirect is native"),
        ApplicationType::Native
    );
}

/// A private-use URI scheme is the other RFC 8252 native redirect shape.
#[test]
fn a_custom_scheme_is_native() {
    assert_eq!(
        derive(&["myapp://oauth/callback"]).expect("a private-use scheme is native"),
        ApplicationType::Native
    );
}

// ===========================================================================
// 2. `web` — a remote, browser-based application.
// ===========================================================================

#[test]
fn a_remote_https_redirect_is_web() {
    assert_eq!(
        derive(&["https://app.example.com/callback"]).expect("a remote https redirect is web"),
        ApplicationType::Web
    );
}

/// Unanimity does not mean "exactly one URI": a vector that classifies the
/// same way throughout is fine however long it is.
#[test]
fn a_uniform_multi_entry_vector_is_web() {
    assert_eq!(
        derive(&["https://app.example.com/cb", "https://app.example.com/cb2",])
            .expect("a uniformly remote vector is web"),
        ApplicationType::Web
    );
}

// ===========================================================================
// 3. The three hard errors. A guess here is an open-redirect primitive.
// ===========================================================================

/// D-10's central rule: a MIXED vector is an ERROR, never a silent pick. The
/// message must name BOTH classifications and BOTH offending URIs, because the
/// operator's next action is to decide which of the two the client actually is.
#[test]
fn a_mixed_vector_is_an_error_naming_both_uris_and_both_classifications() {
    let message = refusal(&["http://127.0.0.1:8080/cb", "https://app.example.com/cb"]);
    assert!(message.contains("http://127.0.0.1:8080/cb"), "{message}");
    assert!(message.contains("https://app.example.com/cb"), "{message}");
    assert!(message.contains("native"), "{message}");
    assert!(message.contains("web"), "{message}");
}

/// Order must not change the outcome: the same vector reversed is the same
/// error. A first-wins implementation would return `Web` for one order and
/// `Native` for the other.
#[test]
fn a_mixed_vector_is_refused_in_either_order() {
    for pair in [
        ["http://127.0.0.1:8080/cb", "https://app.example.com/cb"],
        ["https://app.example.com/cb", "http://127.0.0.1:8080/cb"],
    ] {
        assert!(derive(&pair).is_err(), "{pair:?}");
    }
}

#[test]
fn an_empty_vector_is_an_error_naming_the_empty_input() {
    let message = refusal(&[]);
    assert!(message.contains("empty"), "{message}");
    assert!(message.contains("redirect_uris"), "{message}");
}

#[test]
fn an_unparseable_redirect_uri_is_an_error_naming_the_uri() {
    let message = refusal(&["not a uri"]);
    assert!(message.contains("not a uri"), "{message}");
}

/// `http` on a non-loopback host is neither a permitted native loopback nor a
/// valid web redirect — a cleartext redirect to a remote host leaks the
/// authorization code to anyone on the path.
#[test]
fn cleartext_http_to_a_remote_host_is_an_error() {
    let message = refusal(&["http://app.example.com/cb"]);
    assert!(message.contains("http://app.example.com/cb"), "{message}");
}

// ===========================================================================
// 4. The wire literals.
// ===========================================================================

/// The exact values `OpenID` Connect Dynamic Client Registration §2 defines.
/// They are compatibility surface, not an implementation detail.
#[test]
fn the_wire_literals_are_exactly_native_and_web() {
    assert_eq!(ApplicationType::Native.as_str(), "native");
    assert_eq!(ApplicationType::Web.as_str(), "web");
}

// ===========================================================================
// 5. Properties.
// ===========================================================================

proptest! {
    /// A single redirect URI — whatever it is — produces the same answer every
    /// time and never panics. `derive_application_type` runs on
    /// operator-supplied and, for a platform proxy, on peer-influenced
    /// configuration, so a panic here is a denial of service in a registration
    /// path.
    #[test]
    fn a_single_uri_is_deterministic_and_never_panics(uri in ".{0,60}") {
        let first = derive(&[uri.as_str()]);
        let second = derive(&[uri.as_str()]);
        prop_assert_eq!(first.is_ok(), second.is_ok());
        if let (Ok(first), Ok(second)) = (first, second) {
            prop_assert_eq!(first, second);
        }
    }

    /// Duplicating a URI cannot change its classification: unanimity over one
    /// value is the same as unanimity over the same value repeated. This is
    /// the invariant a "first wins" or "last wins" shortcut would satisfy too,
    /// so it is paired with the mixed-vector tests above rather than relied on
    /// alone.
    #[test]
    fn repetition_does_not_change_the_classification(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        path in "/[a-z]{1,10}",
    ) {
        let uri = format!("https://{host}{path}");
        let once = derive(&[uri.as_str()]).expect("a remote https redirect is web");
        let thrice = derive(&[uri.as_str(), uri.as_str(), uri.as_str()])
            .expect("the same redirect repeated is still unanimous");
        prop_assert_eq!(once, thrice);
        prop_assert_eq!(thrice, ApplicationType::Web);
    }
}
