//! Fuzz target for the AUTH-02 and AUTH-03 pure surfaces: the SEP-2352
//! credential document parser and its schema 1 → 2 migration, the SEP-837
//! `application_type` derivation, and the DCR response's `application_type`
//! accessor.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run oauth_credential_and_dcr`
//! (plain form, no `+nightly` — matches the repo Makefile `test-fuzz` target).
//!
//! Phase 116, AUTH-02 / AUTH-03. Registration only: this target adds NO
//! dependency. Every entry point below is a pure, ungated function reached
//! through the `pmcp` dependency already declared in `fuzz/Cargo.toml`.
//!
//! # Why these surfaces
//!
//! Cross-AI review found the phase's fuzz plan covered AUTH-01's callback
//! surfaces only, leaving AUTH-02's registration metadata and AUTH-03's
//! credential file outside the house ALWAYS policy. `~/.pmcp/oauth-cache.json`
//! is read from disk **before any authentication has happened**, so a corrupt or
//! attacker-planted file reaches `parse_credential_snapshot` with nothing in
//! front of it; and a DCR response is bytes from a registration endpoint that
//! the client has, by construction, not yet authenticated.
//!
//! # Invariants
//!
//! **1. `parse_credential_snapshot` never panics** over arbitrary bytes (threat
//! `T-116-27a`). On `Ok`, and INDEPENDENTLY of the crate's own logic: for a
//! schema-1 document every surviving key carries a non-empty issuer — SEP-2352's
//! rule, and the migration is the only thing enforcing it (threat `T-116-27b`) —
//! and `migrated + dropped` equals the number of top-level entries the input's
//! own JSON describes, counted here with `serde_json::Value` rather than asked
//! of the report. A schema-2 document migrates nothing.
//!
//! **2. Round-trip stability.** Whenever a document parses, re-parsing its own
//! serialization must succeed and yield the SAME keys. A parser and serializer
//! that disagree are how a "successful" save silently drops a login.
//!
//! **3. `derive_application_type` never panics** over any vector of arbitrary
//! strings (threat `T-116-27c`). On `Ok`, the wire value is exactly one of the
//! two literals `OpenID` Connect Dynamic Client Registration §2 defines; the
//! result does not depend on the ORDER of the vector (a first-wins
//! implementation answers differently when reversed); every element on its own
//! derives the SAME type, which is what unanimity means; and an empty vector is
//! ALWAYS an error, never a silent default.
//!
//! **4. `DcrResponse` deserialization plus `application_type()` never panics**
//! (threat `T-116-27c`). Whenever the accessor returns `Some`, the value came
//! VERBATIM from a JSON string in the input — asserted by finding it in the
//! input bytes — which catches a stringification bug that a `None`-only
//! assertion would miss.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::auth::provider::DcrResponse;
use pmcp::shared::credential_store::parse_credential_snapshot;
use pmcp::shared::oauth_validation::derive_application_type;

/// The largest number of redirect URIs derived from one input. Bounded so the
/// target stays fast; the derivation's behaviour does not depend on length
/// beyond "empty / one / many".
const MAX_REDIRECT_URIS: usize = 8;

fuzz_target!(|data: &[u8]| {
    check_credential_snapshot(data);
    check_application_type(data);
    check_dcr_response(data);
});

/// Invariants 1 and 2.
fn check_credential_snapshot(data: &[u8]) {
    // Invariant 1's no-panic half is enforced by libFuzzer: a panic aborts.
    let Ok((snapshot, report)) = parse_credential_snapshot(data) else {
        return;
    };

    // Independent view of the very same bytes. If this does not parse as JSON,
    // the crate could not have accepted it either, so there is nothing to check.
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let version = document.get("schema_version").and_then(serde_json::Value::as_u64);

    match version {
        // Schema 1 — the migration path.
        Some(1) => {
            for key in snapshot.keys() {
                assert!(
                    !key.issuer().is_empty(),
                    "a schema-1 entry survived migration under an EMPTY issuer. SEP-2352 requires \
                     credentials to be addressed by their issuer, and an entry that recorded none \
                     cannot be re-keyed without guessing which authorization server issued it — \
                     it must be DROPPED and reported, not attributed"
                );
            }

            let entries = document
                .get("entries")
                .and_then(serde_json::Value::as_object)
                .map_or(0, serde_json::Map::len);
            let accounted = report.migrated() + report.dropped().len();
            assert_eq!(
                accounted, entries,
                "the migration accounted for {accounted} of the {entries} entries the document \
                 describes; an entry that is neither migrated nor reported as dropped is a login \
                 that vanished silently"
            );
            assert_eq!(
                report.migrated(),
                snapshot.keys().len(),
                "the report claims {} migrated entries but the snapshot holds {} keys",
                report.migrated(),
                snapshot.keys().len()
            );
        },
        // Schema 2 — already current, so nothing may be migrated or dropped.
        Some(2) => {
            assert!(
                report.is_noop(),
                "a document already at the current schema version reported a migration \
                 ({} migrated, {} dropped)",
                report.migrated(),
                report.dropped().len()
            );
        },
        // Any other version is refused by the parser, so `Ok` is unreachable
        // here; and a non-integer version means the crate refused it too.
        _ => {},
    }

    // Invariant 2 — round-trip stability.
    let bytes = snapshot
        .to_bytes()
        .expect("a snapshot that parsed must be serializable; otherwise a save would drop it");
    let (reparsed, reparsed_report) = parse_credential_snapshot(&bytes)
        .expect("re-parsing our own serialization must succeed, or a save silently loses logins");
    assert_eq!(
        reparsed.keys(),
        snapshot.keys(),
        "a save/load round trip changed the set of credential keys"
    );
    assert!(
        reparsed_report.is_noop(),
        "re-parsing freshly written bytes reported a migration; the writer is not emitting the \
         current schema version"
    );
}

/// Invariant 3.
fn check_application_type(data: &[u8]) {
    // An empty vector is ALWAYS an error: never a silent default, because
    // guessing here decides where an authorization code is delivered.
    assert!(
        derive_application_type(&[]).is_err(),
        "an empty `redirect_uris` vector was given a default `application_type` instead of being \
         refused"
    );

    let uris: Vec<String> = data
        .split(|&byte| byte == b'\n')
        .take(MAX_REDIRECT_URIS)
        .map(|line| String::from_utf8_lossy(line).into_owned())
        .collect();

    let Ok(derived) = derive_application_type(&uris) else {
        // A refusal must be order-independent too: a first-wins implementation
        // would accept one ordering and refuse the other.
        let mut reversed = uris;
        reversed.reverse();
        assert!(
            derive_application_type(&reversed).is_err(),
            "the same redirect URIs were REFUSED in one order and ACCEPTED in the other"
        );
        return;
    };

    // The two wire literals from OpenID Connect Dynamic Client Registration §2.
    // An authorization server that receives anything else treats the parameter
    // as absent and silently falls back to the `web` default.
    assert!(
        matches!(derived.as_str(), "native" | "web"),
        "derived application_type rendered as `{}`, which is neither wire literal",
        derived.as_str()
    );

    let mut reversed = uris.clone();
    reversed.reverse();
    assert_eq!(
        derive_application_type(&reversed).ok(),
        Some(derived),
        "the derived application_type depends on the ORDER of `redirect_uris`, so the \
         registration would declare a different client type for the same client"
    );

    // Unanimity means exactly this: every element, on its own, agrees.
    for uri in &uris {
        let single = std::slice::from_ref(uri);
        assert_eq!(
            derive_application_type(single).ok(),
            Some(derived),
            "the vector derived one type unanimously, yet one of its own members derives \
             something else"
        );
    }
}

/// Invariant 4.
fn check_dcr_response(data: &[u8]) {
    let Ok(response) = serde_json::from_slice::<DcrResponse>(data) else {
        return;
    };
    let Some(application_type) = response.application_type() else {
        // `None` is the correct answer for an omitted parameter AND for a
        // non-string JSON value: the accessor projects a string or nothing.
        return;
    };

    // The value must have come verbatim out of a JSON string in the input.
    //
    // Guarded on two conditions, both of which make the check UNSOUND rather
    // than merely weaker if ignored: the bytes must be valid UTF-8 (otherwise
    // there is no text to search), and they must contain no backslash. A JSON
    // string escape — "\u006eative", or "nativ\u0065" — decodes to `native`
    // without the six letters ever appearing consecutively in the input, so
    // asserting the substring unconditionally would fire on a CORRECT accessor.
    // This is a correction to the plan's literally-stated invariant.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if text.contains('\\') {
        return;
    }
    assert!(
        text.contains(application_type),
        "DcrResponse::application_type() returned `{application_type}`, which does not appear \
         anywhere in the input bytes — the accessor stringified something instead of projecting \
         the JSON string the authorization server actually sent"
    );
}
