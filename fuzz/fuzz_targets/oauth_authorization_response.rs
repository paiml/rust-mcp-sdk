//! Fuzz target for `pmcp::shared::oauth_validation` — the pure authorization
//! response validator (RFC 9207 `iss` + CSRF `state`) and the SEP-2351 ordered
//! discovery-candidate derivation.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run oauth_authorization_response`
//! (plain form, no `+nightly` — matches the repo Makefile `test-fuzz` target).
//!
//! Phase 116, AUTH-01 / AUTH-03. Registration only: this target adds NO
//! dependency. Both entry points are UNGATED pure functions, so it reaches them
//! through the `pmcp` dependency already declared in `fuzz/Cargo.toml` without
//! any feature at all.
//!
//! # Why these two entry points
//!
//! A hostile authorization server controls **every byte** of the callback query
//! string — parameter names, ordering, duplicates and encoding — and a hostile
//! `mcp_server_url` controls every byte of the issuer that reaches the discovery
//! URL arithmetic. Both are attacker-supplied input to a pure function, which is
//! the ideal fuzz surface.
//!
//! # Invariants
//!
//! **1. `validate_authorization_response` never panics** (threat `T-116-27`).
//! Error paths are acceptable; panics are not.
//!
//! **1a. Whenever it returns `Ok`, the response really did match** (threat
//! `T-116-29`). The `state` decoded from the query equals the record's `state`,
//! and either no `iss` was present or the decoded `iss` equals the recorded
//! issuer. Additionally: no security parameter was repeated, no `error` was
//! present, the returned code is the query's own `code`, and the query was
//! within [`MAX_CALLBACK_QUERY_BYTES`].
//!
//! These `Ok`-side assertions are derived from RFC 9207 and RFC 6749 §10.12,
//! and the query is decoded by the **hand-rolled** `form_decode` below rather
//! than by calling back into the crate. Phase 115 measured that a fence which
//! restates the code's own rule cannot see a rule defect the code and the fence
//! share; a decoder written here shares neither the crate's rule nor the crate's
//! decoder.
//!
//! **2. `discovery_url_candidates` never panics** (threat `T-116-28`), and
//! whenever it returns `Ok` the list is non-empty, every element is an absolute
//! http(s) URL with a host and with neither query nor fragment, the RFC 8414
//! inserted form is FIRST, and the `OpenID` Connect appended form — the only
//! form RESEARCH measured as HTTP 200 against Microsoft Entra ID — is LAST.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::shared::oauth_validation::{
    discovery_url_candidates, validate_authorization_response, AuthorizationRequestRecord,
    IssPresence, MAX_CALLBACK_QUERY_BYTES,
};

/// The issuer the fixed record expects. A hostile query has to reproduce this
/// byte for byte to be accepted, because RFC 9207's comparison performs no
/// normalization of any kind.
const EXPECTED_ISSUER: &str = "https://as.example";

/// The CSRF `state` the fixed record expects.
const EXPECTED_STATE: &str = "st4te-9f2c-4b71";

/// The parameters whose repetition RFC 6749 §10.12 hardening refuses. Restated
/// here from the specification, deliberately not imported from the crate.
const SECURITY_PARAMETERS: [&str; 5] = ["state", "iss", "code", "error", "error_description"];

fuzz_target!(|data: &[u8]| {
    // Lossy UTF-8 keeps the corpus dense: every byte string yields a query.
    let query = String::from_utf8_lossy(data);

    // Both `iss` policy rows, because row 2 (advertised-and-absent is fatal)
    // is unreachable from an `Optional` record.
    for presence in [IssPresence::Optional, IssPresence::Required] {
        let record = AuthorizationRequestRecord::new(
            EXPECTED_ISSUER,
            "pkce-code-verifier-for-the-fuzz-record",
            EXPECTED_STATE,
            presence,
        );
        check_authorization_response(&query, &record, presence);
    }

    // The same bytes double as a hostile issuer identifier.
    check_discovery_candidates(&query);
});

/// Invariants 1 and 1a.
fn check_authorization_response(
    query: &str,
    record: &AuthorizationRequestRecord,
    presence: IssPresence,
) {
    // Invariant 1 is enforced by libFuzzer itself: a panic here aborts.
    let Ok(code) = validate_authorization_response(query, record) else {
        // A refusal is a correct outcome for almost every input. Nothing to
        // assert: this target claims no-panic, not any particular refusal.
        return;
    };

    // Everything below runs ONLY on the accept path, and is derived from the
    // specification rather than from the implementation.
    let pairs = form_decode(query);

    assert!(
        query.len() <= MAX_CALLBACK_QUERY_BYTES,
        "accepted a callback query of {} bytes, over the {MAX_CALLBACK_QUERY_BYTES}-byte cap",
        query.len()
    );

    for name in SECURITY_PARAMETERS {
        let seen = pairs.iter().filter(|(key, _)| key == name).count();
        assert!(
            seen <= 1,
            "accepted a callback query carrying `{name}` {seen} times; a repeated security \
             parameter is a smuggling primitive and must be refused, not resolved first-wins"
        );
    }

    let state = first_value(&pairs, "state");
    assert_eq!(
        state.as_deref(),
        Some(EXPECTED_STATE),
        "accepted a callback whose independently decoded `state` is {state:?}, not the recorded \
         value — the CSRF check did not hold"
    );

    match first_value(&pairs, "iss") {
        Some(iss) => assert_eq!(
            iss, EXPECTED_ISSUER,
            "accepted a callback whose independently decoded `iss` is not the recorded issuer; \
             RFC 9207 §2.4 permits no normalization, so this is an issuer-spoofing acceptance"
        ),
        None => assert_eq!(
            presence,
            IssPresence::Optional,
            "accepted a callback with NO `iss` while the authorization server had advertised \
             `authorization_response_iss_parameter_supported: true` (table row 2 is fatal)"
        ),
    }

    assert!(
        first_value(&pairs, "error").is_none(),
        "accepted an authorization-server ERROR response as a success"
    );

    assert_eq!(
        first_value(&pairs, "code").as_deref(),
        Some(code.as_str()),
        "the returned authorization code is not the `code` the query carried"
    );
}

/// Invariant 2.
fn check_discovery_candidates(issuer: &str) {
    let Ok(candidates) = discovery_url_candidates(issuer) else {
        return;
    };

    assert!(
        !candidates.is_empty(),
        "accepted issuer yielded an EMPTY candidate list; a caller would probe nothing and \
         report 'discovery failed' for a reachable authorization server"
    );
    assert!(
        candidates.len() == 2 || candidates.len() == 3,
        "expected the specification's two- or three-candidate list, got {}",
        candidates.len()
    );

    for candidate in &candidates {
        let rendered = candidate.as_str();
        assert!(
            matches!(candidate.scheme(), "http" | "https"),
            "candidate `{rendered}` is not an http(s) URL"
        );
        assert!(
            candidate.host_str().is_some_and(|host| !host.is_empty()),
            "candidate `{rendered}` has no host — there is nothing to probe"
        );
        assert!(
            !candidate.cannot_be_a_base(),
            "candidate `{rendered}` is not an absolute, base-capable URL"
        );
        assert!(
            candidate.query().is_none(),
            "candidate `{rendered}` carries a query; RFC 8414 §2 forbids one on the issuer and \
             it must not survive into the probe URL"
        );
        assert!(
            candidate.fragment().is_none(),
            "candidate `{rendered}` carries a fragment; RFC 8414 §2 forbids one"
        );
    }

    // RFC 8414 §3.1's inserted form is the specification's FIRST probe.
    let first = candidates
        .first()
        .expect("non-emptiness asserted immediately above");
    assert!(
        first.path().starts_with("/.well-known/oauth-authorization-server"),
        "the first candidate is `{}`, not RFC 8414 §3.1's inserted form",
        first.as_str()
    );

    // The appended form must survive as the LAST candidate: it is the only form
    // measured as HTTP 200 against Microsoft Entra ID, so an implementation that
    // replaced append with insert would break every server of that shape.
    let last = candidates
        .last()
        .expect("non-emptiness asserted immediately above");
    assert!(
        last.path().ends_with("/.well-known/openid-configuration"),
        "the last candidate is `{}`, so the OpenID Connect appended form was dropped",
        last.as_str()
    );
}

/// The FIRST value recorded for `name`, or `None`.
fn first_value(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

/// A hand-rolled `application/x-www-form-urlencoded` decoder.
///
/// Deliberately NOT `url::form_urlencoded::parse`: the crate under test decodes
/// with that function, so reusing it would make this fence share the crate's
/// decoder as well as its rule. Writing the decode here is what lets the target
/// disagree with the implementation at all.
///
/// The algorithm is the WHATWG one the specification and the `url` crate both
/// implement: split on `&`, skip empty sequences, split each sequence on the
/// FIRST `=` (a sequence with no `=` has an empty value), replace `+` with a
/// space, percent-decode `%HH` for ASCII-hex `HH` only (leaving any other `%`
/// literal), then lossily decode as UTF-8.
fn form_decode(query: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for sequence in query.as_bytes().split(|&byte| byte == b'&') {
        if sequence.is_empty() {
            continue;
        }
        let (name, value) = match sequence.iter().position(|&byte| byte == b'=') {
            Some(split) => (&sequence[..split], &sequence[split + 1..]),
            None => (sequence, &[][..]),
        };
        pairs.push((percent_plus_decode(name), percent_plus_decode(value)));
    }
    pairs
}

/// `+` → space, `%HH` → byte, everything else verbatim; then lossy UTF-8.
fn percent_plus_decode(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte == b'+' {
            out.push(b' ');
            index += 1;
            continue;
        }
        if byte == b'%' && index + 2 < raw.len() {
            if let (Some(high), Some(low)) = (hex_digit(raw[index + 1]), hex_digit(raw[index + 2]))
            {
                out.push(high * 0x10 + low);
                index += 3;
                continue;
            }
        }
        out.push(byte);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// ASCII hex digit → value. Anything else is not a percent escape.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
