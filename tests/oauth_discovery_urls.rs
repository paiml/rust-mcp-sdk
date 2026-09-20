//! ALWAYS coverage for SEP-2351's ordered discovery-URL probe, the RFC 8414
//! §3.3 issuer-anchor comparison, the hardened issuer parse and the discovery
//! outcome matrix (`pmcp::shared::oauth_validation`).
//!
//! Rows covered, in the order they appear below:
//!   1. The specification's two worked candidate lists, asserted as full
//!      ORDERED vectors (3 entries for a path-bearing issuer, 2 for a
//!      path-less one), plus the trailing-slash case and the Microsoft
//!      Entra ID regression fence for RESEARCH Pitfall 2.
//!   2. The hardened `validate_issuer_url` parse: scheme, userinfo, fragment,
//!      query, host — one test per rejection row, each asserting the refusal
//!      names the rule it enforced, plus the three loopback acceptances.
//!   3. `discovery_url_candidates` delegating to that parse, so a hostile
//!      issuer cannot reach candidate construction.
//!   4. `issuer_matches_metadata`: the specification's own worked attack, and
//!      the four RFC 3986 normalizations that MUST NOT make two issuers equal.
//!   5. `same_origin`: the six rows a discovery redirect is judged against.
//!   6. `classify_discovery_failure`: one test per matrix row.
//!   7. Properties, derived from the specification text rather than from the
//!      implementation's branch conditions.
//!
//! The candidate assertions are deliberately written as ORDERED LISTS. RESEARCH
//! Pitfall 2 records that a suite asserting a single expected URL per issuer is
//! exactly what let the "replace append with insert" reading look correct — it
//! cannot see an ordering defect and it cannot see a dropped candidate.
//!
//! **This file is deliberately NOT `#![cfg(feature = "oauth")]`.** The tier
//! under test is ungated, which is the entire point: it must also run under a
//! plain `--features full` build, the feature set `make quality-gate` uses.

use pmcp::shared::oauth_validation::{
    classify_discovery_failure, discovery_url_candidates, issuer_matches_metadata, same_origin,
    validate_issuer_url, DiscoveryFailure, DiscoveryOutcome,
};
use proptest::prelude::*;
use url::Url;

/// The specification's worked path-bearing example.
const ISSUER_WITH_PATH: &str = "https://auth.example.com/tenant1";
/// The specification's worked path-less example.
const ISSUER_WITHOUT_PATH: &str = "https://auth.example.com";

/// The candidate list as plain strings, so a failure prints the whole ordered
/// vector rather than a `Url` debug rendering.
fn candidates(issuer: &str) -> Vec<String> {
    discovery_url_candidates(issuer)
        .unwrap_or_else(|e| panic!("`{issuer}` must produce candidates: {e}"))
        .iter()
        .map(|url| url.as_str().to_string())
        .collect()
}

/// The rendered refusal for an issuer the hardened parse must reject.
fn refusal(issuer: &str) -> String {
    validate_issuer_url(issuer)
        .expect_err("this issuer must be rejected")
        .to_string()
}

fn url(value: &str) -> Url {
    Url::parse(value).unwrap_or_else(|e| panic!("`{value}` must parse: {e}"))
}

// ===========================================================================
// 1. The specification's two worked candidate lists.
// ===========================================================================

/// SEP-2351, verbatim: "For issuer URLs WITH path components (e.g.
/// `https://auth.example.com/tenant1`)" the client MUST attempt these three
/// endpoints, in this order. The third is the ONLY form pmcp builds today.
#[test]
fn a_path_bearing_issuer_yields_the_three_spec_candidates_in_order() {
    assert_eq!(
        candidates(ISSUER_WITH_PATH),
        vec![
            "https://auth.example.com/.well-known/oauth-authorization-server/tenant1",
            "https://auth.example.com/.well-known/openid-configuration/tenant1",
            "https://auth.example.com/tenant1/.well-known/openid-configuration",
        ]
    );
}

/// SEP-2351, verbatim: "For issuer URLs WITHOUT path components (e.g.
/// `https://auth.example.com`)" there are exactly two candidates. The first is
/// RFC 8414 §3.1's default suffix, which pmcp never tries today.
#[test]
fn a_path_less_issuer_yields_the_two_spec_candidates_in_order() {
    assert_eq!(
        candidates(ISSUER_WITHOUT_PATH),
        vec![
            "https://auth.example.com/.well-known/oauth-authorization-server",
            "https://auth.example.com/.well-known/openid-configuration",
        ]
    );
}

/// A trailing slash is a formatting difference, not a path component. It must
/// produce the same two candidates and no doubled slash — the defect the
/// current `trim_end_matches('/')` call site exists to avoid.
#[test]
fn a_trailing_slash_does_not_invent_a_path_component() {
    assert_eq!(
        candidates("https://auth.example.com/"),
        candidates(ISSUER_WITHOUT_PATH)
    );
    for candidate in candidates("https://auth.example.com/") {
        assert!(!candidate.contains("//.well-known"), "{candidate}");
    }
}

/// **RESEARCH Pitfall 2 regression fence (measured 2026-08-02).** Microsoft
/// Entra ID serves ONLY the OIDC appended form:
///
/// | URL form | Measured |
/// |---|---|
/// | `.../common/v2.0/.well-known/openid-configuration` (append) | **200** |
/// | `/.well-known/openid-configuration/common/v2.0` (OIDC insert) | 404 |
/// | `/.well-known/oauth-authorization-server/common/v2.0` (RFC 8414 insert) | 404 |
///
/// Its URL is in this SDK's own doctest (`src/client/auth.rs:127`). If this
/// assertion ever fails, the fix has become the "replace append with insert"
/// regression the ordered probe exists to prevent.
#[test]
fn the_microsoft_entra_id_form_survives_as_the_last_candidate() {
    let list = candidates("https://login.microsoftonline.com/common/v2.0");
    assert_eq!(
        list.last().map(String::as_str),
        Some("https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration"),
        "the only form measured as HTTP 200 must remain in the list: {list:?}"
    );
    assert_eq!(list.len(), 3, "{list:?}");
}

/// A nested tenant path is carried through both insertion forms intact.
#[test]
fn a_multi_segment_path_is_carried_through_every_candidate() {
    assert_eq!(
        candidates("https://auth.example.com/a/b"),
        vec![
            "https://auth.example.com/.well-known/oauth-authorization-server/a/b",
            "https://auth.example.com/.well-known/openid-configuration/a/b",
            "https://auth.example.com/a/b/.well-known/openid-configuration",
        ]
    );
}

/// A non-default port is part of the authority and must survive untouched.
#[test]
fn an_explicit_port_survives_into_every_candidate() {
    for candidate in candidates("https://auth.example.com:8443/tenant1") {
        assert!(
            candidate.starts_with("https://auth.example.com:8443/"),
            "{candidate}"
        );
    }
}

// ===========================================================================
// 2. The hardened parse. An absolute `url::Url` parse is NOT sufficient:
//    RFC 8414 §2 constrains the issuer far more tightly, and userinfo is a
//    classic authority-confusion trick.
// ===========================================================================

#[test]
fn a_plain_https_issuer_is_accepted() {
    let parsed = validate_issuer_url(ISSUER_WITH_PATH).expect("a plain https issuer is valid");
    assert_eq!(parsed.as_str(), ISSUER_WITH_PATH);
}

/// RFC 8252 §7.3's loopback development exception — the ONLY permitted `http`.
/// All three spellings must be accepted, because a listener that binds IPv4
/// and a browser that resolves `localhost` to `::1` produce different ones.
#[test]
fn the_three_loopback_spellings_are_the_only_permitted_http() {
    for issuer in [
        "http://127.0.0.1:8080",
        "http://localhost:9000",
        "http://[::1]:9000",
    ] {
        assert!(
            validate_issuer_url(issuer).is_ok(),
            "the loopback exception must accept {issuer}"
        );
    }
}

#[test]
fn http_on_a_non_loopback_host_is_rejected_naming_the_scheme_rule() {
    let message = refusal("http://auth.example.com");
    assert!(message.contains("scheme"), "{message}");
    assert!(message.contains("https"), "{message}");
}

#[test]
fn every_non_http_scheme_is_rejected_by_name() {
    for (issuer, scheme) in [
        ("ftp://auth.example.com", "ftp"),
        ("file:///etc/passwd", "file"),
        ("data:text/plain,x", "data"),
        ("javascript:alert(1)", "javascript"),
    ] {
        let message = refusal(issuer);
        assert!(message.contains("scheme"), "{issuer}: {message}");
        assert!(message.contains(scheme), "{issuer}: {message}");
    }
}

/// `https://honest.example@evil.example` reads as the honest host to a human
/// and resolves to the attacker's. There is no legitimate issuer with
/// userinfo, so both spellings are refused.
#[test]
fn userinfo_is_rejected_with_and_without_a_password() {
    for issuer in [
        "https://user:pw@auth.example.com",
        "https://user@auth.example.com",
    ] {
        let message = refusal(issuer);
        assert!(message.contains("userinfo"), "{issuer}: {message}");
    }
}

/// The refusal must not reproduce the offending value: a userinfo component
/// can carry a password, and an error string ends up in logs.
#[test]
fn the_userinfo_refusal_does_not_reproduce_the_credential() {
    let message = refusal("https://user:hunter2@auth.example.com");
    assert!(!message.contains("hunter2"), "{message}");
}

#[test]
fn a_fragment_is_rejected_per_rfc_8414_section_2() {
    let message = refusal("https://auth.example.com/tenant#frag");
    assert!(message.contains("fragment"), "{message}");
}

#[test]
fn a_query_is_rejected_per_rfc_8414_section_2() {
    let message = refusal("https://auth.example.com/tenant?x=1");
    assert!(message.contains("query"), "{message}");
}

#[test]
fn an_issuer_with_no_host_is_rejected_naming_the_host_rule() {
    for issuer in ["https://", "https://:8443", "https://?x=1"] {
        let message = refusal(issuer);
        assert!(message.contains("host"), "{issuer}: {message}");
    }
}

/// **Measured, and contrary to the intuition the plan was written with.**
/// `https:///path` is NOT a host-less URL: WHATWG's "special authority ignore
/// slashes" state consumes the extra slash, so the authority becomes `path` and
/// the path becomes `/`. Every browser and every conforming parser agrees, so
/// this is accepted as the perfectly ordinary issuer `https://path/` rather
/// than rejected — and the empty-host rule is exercised by `https://` above,
/// which is the input that genuinely has no authority.
///
/// Pinned as a test because the alternative is a later reader "fixing" the
/// empty-host branch against an input that never reaches it.
#[test]
fn a_third_slash_is_authority_syntax_not_a_missing_host() {
    let parsed = validate_issuer_url("https:///path").expect("this is `https://path/`");
    assert_eq!(parsed.host_str(), Some("path"));
    assert_eq!(parsed.path(), "/");
}

#[test]
fn a_relative_or_unparseable_issuer_is_rejected() {
    for issuer in ["auth.example.com/tenant1", "", "not a url", "/tenant1"] {
        let message = refusal(issuer);
        assert!(message.contains("absolute URL"), "{issuer}: {message}");
    }
}

// ===========================================================================
// 3. Candidate construction goes THROUGH the hardened parse.
// ===========================================================================

/// A hostile issuer must never reach candidate construction — otherwise the
/// hardening is decorative for the one call path that matters.
#[test]
fn candidate_construction_rejects_everything_the_parse_rejects() {
    for issuer in [
        "https://user:pw@auth.example.com",
        "https://auth.example.com/tenant?x=1",
        "https://auth.example.com/tenant#frag",
        "http://auth.example.com",
        "javascript:alert(1)",
        "not a url",
    ] {
        assert!(
            discovery_url_candidates(issuer).is_err(),
            "`{issuer}` must not produce candidates"
        );
    }
}

// ===========================================================================
// 4. RFC 8414 §3.3 / OIDC Discovery §4.3 — the anchor comparison.
// ===========================================================================

/// The specification's own worked attack, verbatim: "a document fetched from
/// `https://attacker.example/.well-known/oauth-authorization-server` that
/// contains `"issuer": "https://honest.example"` MUST be rejected."
#[test]
fn the_spec_worked_attack_is_rejected() {
    assert!(!issuer_matches_metadata(
        "https://attacker.example",
        "https://honest.example"
    ));
}

#[test]
fn an_identical_issuer_matches() {
    assert!(issuer_matches_metadata(
        "https://as.example/tenant1",
        "https://as.example/tenant1"
    ));
}

/// The same no-normalization rule as the RFC 9207 `iss` comparison, from the
/// same family of specifications. Each variant below is one a NORMALIZING
/// comparison would accept.
#[test]
fn no_normalization_of_any_kind_is_applied() {
    for (used, document) in [
        // RFC 3986 §6.2.3 — trailing slash.
        ("https://as.example", "https://as.example/"),
        // RFC 3986 §6.2.2.1 — scheme and host case folding.
        ("https://as.example", "HTTPS://AS.EXAMPLE"),
        // RFC 3986 §6.2.3 — default-port elision.
        ("https://as.example", "https://as.example:443"),
        // RFC 3986 §6.2.2.2 — percent-encoding normalization (`%74` is `t`).
        ("https://as.example/tenant", "https://as.example/%74enant"),
    ] {
        assert!(
            !issuer_matches_metadata(used, document),
            "`{used}` must NOT match `{document}`"
        );
    }
}

// ===========================================================================
// 5. `same_origin` — what a discovery redirect is judged against.
// ===========================================================================

#[test]
fn same_origin_ignores_the_path() {
    assert!(same_origin(
        &url("https://a.example/x"),
        &url("https://a.example/y")
    ));
}

#[test]
fn same_origin_uses_the_effective_port() {
    assert!(same_origin(
        &url("https://a.example"),
        &url("https://a.example:443")
    ));
}

#[test]
fn same_origin_distinguishes_scheme_host_and_explicit_port() {
    assert!(!same_origin(
        &url("https://a.example"),
        &url("http://a.example")
    ));
    assert!(!same_origin(
        &url("https://a.example"),
        &url("https://b.example")
    ));
    assert!(!same_origin(
        &url("https://a.example"),
        &url("https://a.example:8443")
    ));
}

// ===========================================================================
// 6. The discovery outcome matrix, one test per row.
// ===========================================================================

#[test]
fn row_not_found_falls_through_to_the_next_candidate() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::NotFound),
        DiscoveryOutcome::Fallback
    );
}

#[test]
fn row_other_4xx_falls_through_to_the_next_candidate() {
    for status in [400, 401, 403, 405, 410, 429] {
        assert_eq!(
            classify_discovery_failure(DiscoveryFailure::HttpStatus(status)),
            DiscoveryOutcome::Fallback,
            "status {status}"
        );
    }
}

#[test]
fn row_5xx_is_retried() {
    for status in [500, 502, 503, 504] {
        assert_eq!(
            classify_discovery_failure(DiscoveryFailure::HttpStatus(status)),
            DiscoveryOutcome::Retry,
            "status {status}"
        );
    }
}

#[test]
fn row_transport_is_retried() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::Transport),
        DiscoveryOutcome::Retry
    );
}

#[test]
fn row_invalid_json_falls_through_to_the_next_candidate() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::InvalidJson),
        DiscoveryOutcome::Fallback
    );
}

#[test]
fn row_issuer_mismatch_is_terminal() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::IssuerMismatch),
        DiscoveryOutcome::Terminal
    );
}

#[test]
fn row_body_over_cap_is_terminal() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::BodyOverCap),
        DiscoveryOutcome::Terminal
    );
}

#[test]
fn row_malformed_security_metadata_is_terminal() {
    assert_eq!(
        classify_discovery_failure(DiscoveryFailure::MalformedSecurityMetadata),
        DiscoveryOutcome::Terminal
    );
}

// ===========================================================================
// 7. Properties.
//
// Each invariant below is derived from specification TEXT — the ordered probe
// sequence, the measured Entra ID result, the authority the candidate must
// keep — rather than from the implementation's branch condition. A property
// that restates `if path.is_empty()` cannot see a defect in that condition.
// ===========================================================================

proptest! {
    /// The probe sequence is a list, not a choice: it is always non-empty, and
    /// no candidate repeats (a repeat would spend a network round trip proving
    /// the same thing twice, and would make "fall through to the next
    /// candidate" a no-op).
    #[test]
    fn every_candidate_list_is_non_empty_and_free_of_duplicates(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        path in "(|/[a-z]{1,10}|/[a-z]{1,8}/[a-z]{1,8})",
    ) {
        let issuer = format!("https://{host}{path}");
        let list = discovery_url_candidates(&issuer).expect("a plain https issuer is valid");
        prop_assert!(!list.is_empty());
        let mut seen: Vec<&str> = list.iter().map(Url::as_str).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        prop_assert_eq!(seen.len(), before, "duplicate candidate for {}", issuer);
    }

    /// SEP-2351 changes only the PATH of the well-known URL. An implementation
    /// that reached a different host or downgraded the scheme would be building
    /// a request the caller never asked for.
    #[test]
    fn every_candidate_keeps_the_issuer_scheme_and_host(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        path in "(|/[a-z]{1,10}|/[a-z]{1,8}/[a-z]{1,8})",
    ) {
        let issuer = format!("https://{host}{path}");
        let list = discovery_url_candidates(&issuer).expect("a plain https issuer is valid");
        for candidate in &list {
            prop_assert_eq!(candidate.scheme(), "https");
            prop_assert_eq!(candidate.host_str(), Some(host.as_str()));
            prop_assert!(!candidate.cannot_be_a_base());
            prop_assert!(candidate.query().is_none());
            prop_assert!(candidate.fragment().is_none());
        }
    }

    /// **The Pitfall 2 fence, as a property.** The OIDC appended form — today's
    /// behaviour, and the only form measured as HTTP 200 against Microsoft
    /// Entra ID — is present in EVERY candidate list, for every issuer shape.
    /// If this ever fails, the ordered probe has silently become the
    /// "replace append with insert" regression.
    #[test]
    fn the_oidc_appended_form_is_present_for_every_issuer(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        path in "(|/[a-z]{1,10}|/[a-z]{1,8}/[a-z]{1,8})",
    ) {
        let issuer = format!("https://{host}{path}");
        let appended = format!("https://{host}{path}/.well-known/openid-configuration");
        let list = discovery_url_candidates(&issuer).expect("a plain https issuer is valid");
        prop_assert!(
            list.iter().any(|candidate| candidate.as_str() == appended),
            "the appended form {} is missing from {:?}",
            appended,
            list.iter().map(Url::as_str).collect::<Vec<_>>()
        );
    }

    /// Totality: every constructible failure — including an arbitrary status
    /// code, which is peer-controlled — maps to exactly one outcome and never
    /// panics.
    #[test]
    fn classification_is_total_over_arbitrary_status_codes(status in any::<u16>()) {
        let all = [
            DiscoveryFailure::NotFound,
            DiscoveryFailure::HttpStatus(status),
            DiscoveryFailure::Transport,
            DiscoveryFailure::InvalidJson,
            DiscoveryFailure::IssuerMismatch,
            DiscoveryFailure::BodyOverCap,
            DiscoveryFailure::MalformedSecurityMetadata,
        ];
        for failure in all {
            let outcome = classify_discovery_failure(failure);
            prop_assert!(
                matches!(
                    outcome,
                    DiscoveryOutcome::Fallback
                        | DiscoveryOutcome::Retry
                        | DiscoveryOutcome::Terminal
                ),
                "{:?} produced an outcome outside the documented three: {:?}",
                failure,
                outcome
            );
        }
    }

    /// **The security invariant, stated as a security property rather than as
    /// a restatement of the match arms.**
    ///
    /// A failure class that means "bytes ARRIVED and cannot be trusted" must
    /// never cause the client to try a different endpoint. Falling through on
    /// one of these would be a silent DOWNGRADE: an attacker who can make
    /// candidate 1 fail in a security-relevant way would get the client to
    /// accept candidate 3 instead.
    ///
    /// Conversely, a failure class that means only "this endpoint was not
    /// available" must never abort discovery outright — no peer-chosen status
    /// code may turn an availability failure into a terminal one.
    #[test]
    fn an_untrusted_document_never_falls_through_and_availability_is_never_terminal(
        status in any::<u16>(),
    ) {
        for untrusted in [
            DiscoveryFailure::IssuerMismatch,
            DiscoveryFailure::BodyOverCap,
            DiscoveryFailure::MalformedSecurityMetadata,
        ] {
            prop_assert_ne!(
                classify_discovery_failure(untrusted),
                DiscoveryOutcome::Fallback,
                "{:?} must not trigger fallback",
                untrusted
            );
        }
        for availability in [
            DiscoveryFailure::NotFound,
            DiscoveryFailure::Transport,
            DiscoveryFailure::InvalidJson,
            DiscoveryFailure::HttpStatus(status),
        ] {
            prop_assert_ne!(
                classify_discovery_failure(availability),
                DiscoveryOutcome::Terminal,
                "{:?} must not abort discovery",
                availability
            );
        }
    }

    /// The anchor comparison is a plain string comparison, so any two DISTINCT
    /// strings are a mismatch and any string matches itself. Written over
    /// generated issuers so a future "helpful" normalization anywhere in the
    /// function is caught rather than argued about.
    #[test]
    fn the_anchor_comparison_matches_only_identical_strings(
        left in "[!-~]{1,40}",
        right in "[!-~]{1,40}",
    ) {
        prop_assert!(issuer_matches_metadata(&left, &left));
        prop_assert_eq!(issuer_matches_metadata(&left, &right), left == right);
    }
}
