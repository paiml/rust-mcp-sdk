//! ALWAYS coverage for the wasm-safe OAuth authorization-response validator
//! (`pmcp::shared::oauth_validation`).
//!
//! Rows covered, in the order they appear below:
//!   1. The four normative `iss` table rows, one test each, plus the
//!      Optional-present-and-DIFFERENT case — the D-01 floor's teeth.
//!   2. Four no-normalization properties, one per RFC 3986 §6.2.2-§6.2.3
//!      normalization, each asserting MISMATCH.
//!   3. The CSRF `state` comparison (D-12), including its non-disclosure.
//!   4. Error-response non-disclosure: an `iss` mismatch must suppress
//!      `error_description`, and a valid `iss` must surface it.
//!   5. Duplicate security parameters (fail-closed) plus the unknown-parameter
//!      control.
//!   6. The `MAX_CALLBACK_QUERY_BYTES` size bound.
//!   7. `iss_presence_from` D-04 precedence and `parse_iss_env_value`.
//!   8. The crate-root re-export resolves to the same items as the module path.
//!
//! The property invariants are derived from the specification's
//! no-normalization SENTENCE — "clients MUST NOT apply scheme or host case
//! folding, default-port elision, trailing-slash, or percent-encoding
//! normalization (RFC 3986 Sections 6.2.2-6.2.3) before comparison" — rather
//! than restated from the implementation's comparison operator. A test that
//! says "the code uses `==`, so `==` is what it does" pins nothing.
//!
//! **This file is deliberately NOT `#![cfg(feature = "oauth")]`.** The tier
//! under test is ungated, which is the entire point: it must also run under a
//! plain `--features full` build, the feature set `make quality-gate` uses.

use pmcp::shared::oauth_validation::{
    iss_presence_from, parse_iss_env_value, validate_authorization_response,
    AuthorizationRequestRecord, IssPresence, MAX_CALLBACK_QUERY_BYTES,
};
use proptest::prelude::*;

const ISSUER: &str = "https://as.example";
const STATE: &str = "opaque-csrf-state";

fn record(presence: IssPresence) -> AuthorizationRequestRecord {
    AuthorizationRequestRecord::new(ISSUER, "code-verifier", STATE, presence)
}

/// Percent-encode a value for placement in a query, so every test exercises the
/// RFC 9207 §2.4 form-urlencoded decode the validator is required to perform
/// rather than sidestepping it with a pre-decoded literal.
fn encoded(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn query_with_iss(iss: &str) -> String {
    format!("code=auth-code&state={STATE}&iss={}", encoded(iss))
}

// ===========================================================================
// 1. The four normative table rows, plus the floor's teeth.
// ===========================================================================

/// Row 1 — the authorization server advertised the parameter and sent a
/// matching `iss`. This is the only fully-happy path.
#[test]
fn row1_required_and_present_and_equal_is_accepted() {
    let code =
        validate_authorization_response(&query_with_iss(ISSUER), &record(IssPresence::Required))
            .expect("a matching iss under Required must be accepted");
    assert_eq!(code, "auth-code");
}

/// Row 2 — the authorization server advertised
/// `authorization_response_iss_parameter_supported` and sent no `iss`. Reject,
/// and report the absence as `iss_actual() == None` rather than as a second
/// marker.
#[test]
fn row2_required_and_absent_is_rejected_with_no_actual_issuer() {
    let err = validate_authorization_response(
        &format!("code=auth-code&state={STATE}"),
        &record(IssPresence::Required),
    )
    .expect_err("an advertised-but-absent iss must be rejected");
    assert!(err.is_iss_mismatch(), "{err}");
    assert_eq!(err.iss_expected(), Some(ISSUER));
    assert_eq!(err.iss_actual(), None);
}

/// Row 3 — nothing was advertised, but an `iss` arrived anyway. It is STILL
/// compared: the floor is unconditional.
#[test]
fn row3_optional_and_present_and_equal_is_accepted() {
    let code =
        validate_authorization_response(&query_with_iss(ISSUER), &record(IssPresence::Optional))
            .expect("a matching iss under Optional must be accepted");
    assert_eq!(code, "auth-code");
}

/// Row 4 — nothing advertised and nothing sent. Proceed. This is the D-01
/// lenient floor, and it is lenient ONLY here.
#[test]
fn row4_optional_and_absent_proceeds() {
    let code = validate_authorization_response(
        &format!("code=auth-code&state={STATE}"),
        &record(IssPresence::Optional),
    )
    .expect("row 4 proceeds");
    assert_eq!(code, "auth-code");
}

/// The floor's TEETH, and the reason v1 gets strictly safer rather than merely
/// unchanged: under `Optional`, an `iss` that is present and DIFFERENT is still
/// a rejection. A client that only checked when the flag was set would accept
/// a mix-up attack from any authorization server that omits the metadata.
#[test]
fn optional_with_a_present_but_different_iss_is_still_rejected() {
    let err = validate_authorization_response(
        &query_with_iss("https://evil.example"),
        &record(IssPresence::Optional),
    )
    .expect_err("a present-but-wrong iss is rejected whatever the flag said");
    assert!(err.is_iss_mismatch(), "{err}");
    assert_eq!(err.iss_expected(), Some(ISSUER));
    assert_eq!(err.iss_actual(), Some("https://evil.example"));
}

// ===========================================================================
// 2. No-normalization properties (RFC 3986 §6.2.2-§6.2.3).
//
// The specification names four normalizations a client MUST NOT apply before
// comparison. Each generator below produces an `iss` that a NORMALIZING
// comparison would accept, and asserts it is a MISMATCH. If any of these ever
// passes, the implementation has started normalizing.
// ===========================================================================

proptest! {
    /// RFC 3986 §6.2.2.1 (case normalization): scheme and host are
    /// case-insensitive as a matter of URI equivalence, and a client MUST NOT
    /// case-fold them before this comparison.
    #[test]
    fn no_scheme_or_host_case_folding(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        path in "(|/[a-z]{1,10})",
    ) {
        let recorded = format!("https://{host}{path}");
        let arriving = format!("HTTPS://{}{path}", host.to_uppercase());
        prop_assume!(recorded != arriving);

        let rec = AuthorizationRequestRecord::new(
            recorded.clone(), "code-verifier", STATE, IssPresence::Optional,
        );
        let err = validate_authorization_response(
            &format!("code=auth-code&state={STATE}&iss={}", encoded(&arriving)),
            &rec,
        )
        .expect_err("case folding must NOT make these equal");
        prop_assert!(
            err.is_iss_mismatch(),
            "case-folded issuer {arriving} was accepted against {recorded}"
        );
    }

    /// RFC 3986 §6.2.3 (scheme-based normalization): eliding the default port
    /// makes `https://h:443` equivalent to `https://h` for general URI
    /// comparison, and a client MUST NOT do it here.
    #[test]
    fn no_default_port_elision(host in "[a-z]{3,12}\\.[a-z]{2,6}") {
        let recorded = format!("https://{host}");
        let arriving = format!("https://{host}:443");

        let rec = AuthorizationRequestRecord::new(
            recorded.clone(), "code-verifier", STATE, IssPresence::Optional,
        );
        let err = validate_authorization_response(
            &format!("code=auth-code&state={STATE}&iss={}", encoded(&arriving)),
            &rec,
        )
        .expect_err("default-port elision must NOT make these equal");
        prop_assert!(
            err.is_iss_mismatch(),
            "port-elided issuer {arriving} was accepted against {recorded}"
        );
    }

    /// RFC 3986 §6.2.3 (scheme-based normalization): an empty path is
    /// equivalent to `/` for general URI comparison, and a client MUST NOT
    /// apply that equivalence here.
    #[test]
    fn no_trailing_slash_normalization(host in "[a-z]{3,12}\\.[a-z]{2,6}") {
        let recorded = format!("https://{host}");
        let arriving = format!("https://{host}/");

        let rec = AuthorizationRequestRecord::new(
            recorded.clone(), "code-verifier", STATE, IssPresence::Optional,
        );
        let err = validate_authorization_response(
            &format!("code=auth-code&state={STATE}&iss={}", encoded(&arriving)),
            &rec,
        )
        .expect_err("trailing-slash normalization must NOT make these equal");
        prop_assert!(
            err.is_iss_mismatch(),
            "trailing-slash issuer {arriving} was accepted against {recorded}"
        );
    }

    /// RFC 3986 §6.2.2.2 (percent-encoding normalization): decoding an
    /// unreserved character's percent-triplet is a normalization a client MUST
    /// NOT apply. `%74` is `t`, so `/%74enant` would normalize to `/tenant`.
    ///
    /// Note this is a property of the ISSUER comparison, not of the transport
    /// decode: the form-urlencoded decode RFC 9207 §2.4 requires has already
    /// happened by the time the two strings meet, and it is the SECOND,
    /// URI-level decode that is forbidden.
    #[test]
    fn no_percent_encoding_normalization(
        host in "[a-z]{3,12}\\.[a-z]{2,6}",
        rest in "[a-z]{1,8}",
    ) {
        let recorded = format!("https://{host}/t{rest}");
        let arriving = format!("https://{host}/%74{rest}");

        let rec = AuthorizationRequestRecord::new(
            recorded.clone(), "code-verifier", STATE, IssPresence::Optional,
        );
        // `%2574` is the form-urlencoded encoding of the literal text `%74`,
        // so what reaches the comparison is `.../%74…` and not `.../t…`.
        let err = validate_authorization_response(
            &format!("code=auth-code&state={STATE}&iss={}", encoded(&arriving)),
            &rec,
        )
        .expect_err("percent-encoding normalization must NOT make these equal");
        prop_assert!(
            err.is_iss_mismatch(),
            "percent-encoded issuer {arriving} was accepted against {recorded}"
        );
    }
}

// ===========================================================================
// 3. The CSRF `state` comparison (D-12).
// ===========================================================================

/// A matching `state` passes on to `iss` validation rather than short-circuiting
/// the whole response — proven by the fact that a WRONG `iss` behind a RIGHT
/// `state` still fails, and fails as an `iss` mismatch.
#[test]
fn a_matching_state_passes_on_to_iss_validation() {
    let err = validate_authorization_response(
        &query_with_iss("https://evil.example"),
        &record(IssPresence::Required),
    )
    .expect_err("the iss check runs after state");
    assert!(err.is_iss_mismatch(), "{err}");
    assert!(!err.is_state_mismatch(), "{err}");
}

/// A forged `state` is refused, and the refusal reproduces NEITHER value: the
/// expected one is a CSRF secret and the received one is attacker-controlled
/// (T-116-03).
#[test]
fn a_different_state_is_refused_without_disclosing_either_value() {
    let forged = "attacker-chosen-state";
    let err = validate_authorization_response(
        &format!("code=auth-code&state={forged}"),
        &record(IssPresence::Optional),
    )
    .expect_err("a forged state is refused");
    assert!(err.is_state_mismatch(), "{err}");

    let rendered = err.to_string();
    assert!(
        !rendered.contains(STATE),
        "expected state leaked: {rendered}"
    );
    assert!(
        !rendered.contains(forged),
        "received state echoed: {rendered}"
    );
}

/// Absence of `state` is a MISMATCH, not a skip. A response that simply omits
/// the parameter must not be treated as one that matched.
#[test]
fn an_absent_state_is_a_mismatch_not_a_skip() {
    let err = validate_authorization_response("code=auth-code", &record(IssPresence::Optional))
        .expect_err("an absent state is refused");
    assert!(err.is_state_mismatch(), "{err}");
}

/// `state` is compared BEFORE `iss`: a response that fails both reports the
/// state failure, because the client has not yet established that the response
/// belongs to this request at all.
#[test]
fn state_is_evaluated_before_iss() {
    let err = validate_authorization_response(
        &format!(
            "code=auth-code&state=wrong&iss={}",
            encoded("https://evil.example")
        ),
        &record(IssPresence::Required),
    )
    .expect_err("both checks fail");
    assert!(err.is_state_mismatch(), "{err}");
    assert!(!err.is_iss_mismatch(), "{err}");
}

// ===========================================================================
// 4. Error-response non-disclosure.
//
// "This validation applies equally to error responses — on mismatch the client
// MUST NOT act on or display `error`, `error_description`, or `error_uri`."
// ===========================================================================

/// The MUST NOT: an authorization-server `error_description` behind a WRONG
/// `iss` never reaches the caller, because an unauthenticated party would
/// otherwise get to choose text the client displays (T-116-04).
#[test]
fn an_error_description_behind_a_wrong_iss_is_not_disclosed() {
    let err = validate_authorization_response(
        &format!(
            "error=access_denied&error_description=SECRET&state={STATE}&iss={}",
            encoded("https://evil.example")
        ),
        &record(IssPresence::Required),
    )
    .expect_err("a wrong iss is rejected even on an error response");
    assert!(err.is_iss_mismatch(), "{err}");
    assert!(
        !err.to_string().contains("SECRET"),
        "attacker-chosen text was displayed: {err}"
    );
}

/// The complement: once `state` and `iss` both check out, the authorization
/// server's error IS actionable and must be surfaced — suppressing it here
/// would leave the caller with no idea why the flow failed.
#[test]
fn an_error_description_behind_a_valid_iss_is_surfaced() {
    let err = validate_authorization_response(
        &format!(
            "error=access_denied&error_description=visible&state={STATE}&iss={}",
            encoded(ISSUER)
        ),
        &record(IssPresence::Required),
    )
    .expect_err("the authorization server refused");
    assert!(!err.is_iss_mismatch(), "{err}");
    assert!(!err.is_state_mismatch(), "{err}");
    let rendered = err.to_string();
    assert!(rendered.contains("access_denied"), "{rendered}");
    assert!(rendered.contains("visible"), "{rendered}");
}

// ===========================================================================
// 5. Duplicate security parameters (fail-closed), plus the control.
// ===========================================================================

/// Each of the five security parameters is refused on repetition, whether or
/// not the two occurrences agree. A first-wins rule is a smuggling primitive
/// (T-116-05a).
#[test]
fn a_duplicated_state_is_refused() {
    assert_duplicate_refused("state");
}

#[test]
fn a_duplicated_iss_is_refused() {
    assert_duplicate_refused("iss");
}

#[test]
fn a_duplicated_code_is_refused() {
    assert_duplicate_refused("code");
}

#[test]
fn a_duplicated_error_is_refused() {
    assert_duplicate_refused("error");
}

#[test]
fn a_duplicated_error_description_is_refused() {
    assert_duplicate_refused("error_description");
}

/// Appends `key` twice to an otherwise entirely valid response and asserts the
/// whole response is refused — once with the two occurrences DISAGREEING (the
/// smuggling case) and once with them AGREEING (so the rule is "a security
/// parameter may not repeat", not "the values must match").
fn assert_duplicate_refused(key: &str) {
    let base = format!("code=auth-code&state={STATE}&iss={}", encoded(ISSUER));
    for (label, second) in [
        ("differing", "smuggled-value"),
        ("agreeing", "repeated-value"),
    ] {
        let query = format!("{base}&{key}=repeated-value&{key}={second}");
        let err = match validate_authorization_response(&query, &record(IssPresence::Required)) {
            Ok(code) => panic!(
                "a {label} duplicate `{key}` was ACCEPTED (returned {code:?}); a first-wins rule \
                 on a security parameter is a smuggling primitive: {query}"
            ),
            Err(err) => err,
        };
        let expected = format!("`{key}` more than once");
        assert!(
            err.to_string().contains(&expected),
            "the refusal must name the repeated parameter ({expected}): {err}"
        );
    }
}

/// The control that keeps the rule from being "reject anything repeated":
/// unknown and vendor parameters may repeat freely, because ignoring a value
/// twice is the same as ignoring it once.
#[test]
fn a_duplicated_unknown_parameter_is_not_an_error() {
    let code = validate_authorization_response(
        &format!(
            "vendor_trace=a&code=auth-code&vendor_trace=b&state={STATE}&iss={}",
            encoded(ISSUER)
        ),
        &record(IssPresence::Required),
    )
    .expect("a repeated unknown parameter is ignored, not refused");
    assert_eq!(code, "auth-code");
}

// ===========================================================================
// 6. The size bound (T-116-05b).
// ===========================================================================

/// A query over `MAX_CALLBACK_QUERY_BYTES` is refused before parsing. The
/// refusal names the limit and the observed length, and reproduces no byte of
/// the query — a marker planted inside it must be absent from the message.
#[test]
fn an_oversize_query_is_refused_and_never_echoed() {
    let marker = "PLANTED-CANARY-9c1f";
    let query = format!(
        "code={marker}{}&state={STATE}",
        "p".repeat(MAX_CALLBACK_QUERY_BYTES)
    );
    assert!(query.len() > MAX_CALLBACK_QUERY_BYTES);

    let err = validate_authorization_response(&query, &record(IssPresence::Optional))
        .expect_err("an oversize query is refused");
    let rendered = err.to_string();

    assert!(
        rendered.contains(&MAX_CALLBACK_QUERY_BYTES.to_string()),
        "the refusal must name the limit: {rendered}"
    );
    assert!(
        rendered.contains(&query.len().to_string()),
        "the refusal must name the observed length: {rendered}"
    );
    assert!(
        !rendered.contains(marker),
        "the refusal echoed the query: {rendered}"
    );
}

// ===========================================================================
// 7. D-04 precedence and environment-value parsing.
// ===========================================================================

/// Precedence is environment > builder > discovery flag, in that order, and
/// each tier is proven to beat the one below it rather than merely to be
/// consulted.
#[test]
fn iss_presence_precedence_is_env_then_builder_then_discovery() {
    // env beats both, in both directions, so neither result is an accident of
    // which value happens to be the default.
    assert_eq!(
        iss_presence_from(
            Some(IssPresence::Required),
            Some(IssPresence::Optional),
            Some(false)
        ),
        IssPresence::Required,
    );
    assert_eq!(
        iss_presence_from(
            Some(IssPresence::Optional),
            Some(IssPresence::Required),
            Some(true)
        ),
        IssPresence::Optional,
    );
    // builder beats the discovery flag.
    assert_eq!(
        iss_presence_from(None, Some(IssPresence::Required), Some(false)),
        IssPresence::Required,
    );
    assert_eq!(
        iss_presence_from(None, Some(IssPresence::Optional), Some(true)),
        IssPresence::Optional,
    );
    // the discovery flag decides when nothing above it spoke.
    assert_eq!(
        iss_presence_from(None, None, Some(true)),
        IssPresence::Required
    );
    assert_eq!(
        iss_presence_from(None, None, Some(false)),
        IssPresence::Optional
    );
    // "false" and "absent" are the same thing per the specification.
    assert_eq!(iss_presence_from(None, None, None), IssPresence::Optional);
}

/// The three accepted forms of each value: exact, trimmed, and mixed case.
#[test]
fn parse_iss_env_value_accepts_strict_and_lenient_in_three_forms() {
    for value in ["strict", " STRICT ", "Strict"] {
        assert_eq!(
            parse_iss_env_value(value),
            Some(IssPresence::Required),
            "{value:?} must parse as strict"
        );
    }
    for value in ["lenient", " LENIENT ", "Lenient"] {
        assert_eq!(
            parse_iss_env_value(value),
            Some(IssPresence::Optional),
            "{value:?} must parse as lenient"
        );
    }
}

/// The plausible-but-wrong values an operator might actually type return
/// `None`, so the call site can warn by name instead of failing open
/// (T-116-05c). This is why the parse is a separate function.
#[test]
fn parse_iss_env_value_rejects_plausible_but_wrong_values() {
    for value in ["true", "1", "yes", "", "strictly", "on", "enabled", "false"] {
        assert_eq!(
            parse_iss_env_value(value),
            None,
            "{value:?} must NOT silently resolve to a presence setting"
        );
    }
}

/// The split is load-bearing: an unrecognized value must be distinguishable
/// from an unset variable, which is exactly what a caller composing the two
/// functions can see.
#[test]
fn an_unrecognized_env_value_is_distinguishable_from_an_unset_one() {
    // Unset: no override, so the discovery flag decides.
    assert_eq!(
        iss_presence_from(None, None, Some(true)),
        IssPresence::Required
    );
    // Set-but-unparseable: the parse yields None, which the caller can SEE and
    // warn about before falling through to the same result.
    let raw = Some("true");
    let parsed = raw.and_then(parse_iss_env_value);
    assert!(
        raw.is_some() && parsed.is_none(),
        "the caller can tell the two apart"
    );
    assert_eq!(
        iss_presence_from(parsed, None, Some(true)),
        IssPresence::Required
    );
}

// ===========================================================================
// 8. Public surface.
// ===========================================================================

/// The crate-root re-export resolves to the SAME items as the module path — a
/// platform handler that reaches for `pmcp::validate_authorization_response`
/// gets the one implementation, not a second copy.
#[test]
fn the_crate_root_reexport_resolves_to_the_same_items() {
    let via_root = pmcp::AuthorizationRequestRecord::new(
        ISSUER,
        "code-verifier",
        STATE,
        pmcp::IssPresence::Optional,
    );
    let via_module = record(IssPresence::Optional);

    let query = query_with_iss(ISSUER);
    assert_eq!(
        pmcp::validate_authorization_response(&query, &via_root).expect("root path"),
        validate_authorization_response(&query, &via_module).expect("module path"),
    );

    // The types are the same type, not merely same-shaped: the module-path
    // record is accepted by the crate-root function and vice versa.
    assert!(pmcp::validate_authorization_response(&query, &via_module).is_ok());
    assert!(validate_authorization_response(&query, &via_root).is_ok());
}
