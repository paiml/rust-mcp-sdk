//! Offline blocking contract test for the pmcp.run attestation-verification
//! contract (Phase 122 plan 04, PKGX-01).
//!
//! Validates that the CLI's `verifyAttestation` operation
//! (`VERIFY_ATTESTATION_QUERY` in
//! `src/deployment/targets/pmcp_run/graphql_contract.rs`) has not drifted from
//! the vendored SDL at `contracts/pmcp-run/attestation-v1.graphql`.
//!
//! This is a pure offline/static check: no network access, no pmcp.run
//! credentials. (The fourth test in this file CAN reach the network — it is
//! `#[ignore]`d behind a triple env gate; see its own docs.)
//!
//! It is executed by the `test-cargo-pmcp-integration` Makefile target, which
//! is chained into `test-all` and therefore into `make quality-gate`. That
//! target does not merely run this file — it asserts a NONZERO passed count for
//! this binary BY NAME, via `scripts/named-test-binary-count.awk`. That
//! per-binary assertion is what makes the word "blocking" a measured property
//! rather than a claim: an `#[ignore]` sweep or a `#[cfg]` gate turning false
//! here reports `0 passed` and fails the build, even though the summed total
//! across the target's other binaries would stay comfortably nonzero.
//!
//! # What this file CANNOT prove — read this before trusting a green run
//!
//! The sibling `package_capture_contract.rs` validates SDK queries against a
//! PLATFORM-EXPORTED schema. This file does not. Its schema
//! (`contracts/pmcp-run/attestation-v1.graphql`) is SDK-PROPOSED and
//! unratified: the pmcp.run platform team has not responded to it, and no
//! introspection export produced it.
//!
//! So this test pins **SDK-INTERNAL agreement** today — that the operation
//! string and the proposed schema cannot drift apart, and that a change to one
//! forces a change to the other in the same PR. It becomes a real
//! cross-boundary drift net the moment the platform exports its own SDL to
//! replace the proposal. **It cannot detect drift from a platform that has not
//! spoken.** A green run here is not platform agreement, and nobody should
//! read it as one.
//!
//! The ratification ask lives in
//! `docs/platform-requests/package-portability-alignment.md`.

use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

use cargo_pmcp::pmcp_run_graphql::VERIFY_ATTESTATION_QUERY;

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/attestation-v1.graphql"
);

/// The fields the SDK reads back out of `VerifyAttestationReturnType`. Kept in
/// one place so tests 1 and 3 and the live leg cannot disagree about them.
const EXPECTED_RESPONSE_FIELDS: [&str; 3] = ["verdict", "verifiedIdentity", "verifiedAt"];

// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read attestation-v1.graphql");
    Schema::parse_and_validate(sdl, "attestation-v1.graphql").expect("vendored SDL is itself valid")
}

/// The SDL with every `#` comment removed, so shape assertions read the SCHEMA
/// and not the prose about it.
///
/// This matters more here than in the capture sibling: this file's header
/// deliberately DISCUSSES the words `enum`, `getAttestation` and
/// `issueAttestation` in order to record why they are absent. A naive
/// `sdl.contains("enum")` would therefore be satisfied by the very comment
/// explaining the ban — a self-invalidating check.
fn sdl_body() -> String {
    std::fs::read_to_string(SDL_PATH)
        .expect("read attestation-v1.graphql")
        .lines()
        .map(|line| match line.find('#') {
            Some(idx) => line[..idx].to_string(),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The runtime operation must validate against the vendored contract.
#[test]
fn verify_attestation_op_validates_against_contract() {
    let schema = schema();
    ExecutableDocument::parse_and_validate(
        &schema,
        VERIFY_ATTESTATION_QUERY,
        "verify_attestation.graphql",
    )
    .unwrap_or_else(|e| {
        panic!("`verifyAttestation` op does not match attestation-v1.graphql: {e}")
    });
}

/// The shape pin: `verdict` is a plain String, the SDL declares no enum, and
/// the two D-11 deferrals appear nowhere as declared fields.
///
/// An enum would make any later schema-versus-schema diff (proposal vs. the
/// platform's eventual export) show permanent drift — the same discipline
/// `capture-v1.graphql` records for its `status` fields. `getAttestation` and
/// `issueAttestation` are deferred by D-11: the attestation arrives inside the
/// package so the CLI never fetches one, and issuance is entirely the
/// platform's to design.
#[test]
fn sdl_shape_is_pinned_to_d11_scope() {
    let body = sdl_body();

    assert!(
        body.contains("verdict: String"),
        "verdict must be typed String in attestation-v1.graphql:\n{body}"
    );
    for line in body.lines() {
        assert!(
            !line.trim_start().starts_with("enum "),
            "attestation-v1.graphql must declare no enum, found: {line}"
        );
    }
    for deferred in ["getAttestation", "issueAttestation"] {
        assert!(
            !body.contains(deferred),
            "`{deferred}` is deferred by D-11 and must not be declared \
             (it may appear only in a `#` comment explaining the deferral)"
        );
    }

    // Exactly one operation field is proposed — the whole point of D-11.
    assert_eq!(
        body.matches("verifyAttestation(").count(),
        1,
        "the proposal declares exactly one operation field:\n{body}"
    );
}

/// The selection-set drift check: every field the SDK expects to read back must
/// appear in the operation string.
///
/// NOTE: this greps the hardcoded field-name list above against the query
/// STRING; it does NOT parse the `VerifyAttestationOutcome` Rust struct. The
/// query itself is separately validated against the vendored SDL by
/// `verify_attestation_op_validates_against_contract` (apollo-compiler,
/// field-existence + type checking). So this is a drift sanity check on the
/// selection set, not a full struct-versus-schema proof: a decoder that stopped
/// reading one of these fields, with no accompanying query change, would go
/// uncaught here.
#[test]
fn response_fields_match_selection_set() {
    for field in EXPECTED_RESPONSE_FIELDS {
        assert!(
            VERIFY_ATTESTATION_QUERY.contains(field),
            "`{field}` missing from the verifyAttestation selection set"
        );
    }
}

// ===========================================================================
// The PARKED live leg (SC5)
// ===========================================================================

/// A fixture attestation payload. Opaque bytes as far as the SDK is concerned —
/// it never deserializes an attestation, which is the whole carriage boundary.
fn fixture_attestation_payload() -> Vec<u8> {
    br#"{"schemaVersion":1,"verdict":"pass-with-warnings"}"#.to_vec()
}

/// Derive a `sha256:...` subject digest LOCALLY, so the live request carries a
/// real re-derived subject rather than a placeholder string.
fn subject_digest_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in &digest {
        let _ = write!(hex, "{b:02x}");
    }
    format!("sha256:{hex}")
}

/// PKGX-01's live verification leg — PARKED on a pmcp.run backend that does not
/// exist yet.
///
/// # This is an executable request path, not a description of one
///
/// Below the three gates, this test builds its body with the PRODUCTION builder
/// (`verify_attestation_request_body`), POSTs it with the PRODUCTION header
/// constant (`GRAPHQL_AUTH_HEADER`), and decodes the answer with the PRODUCTION
/// decoder (`decode_verify_attestation_response`). Nothing here is a stub: the
/// only piece that lives in the test rather than in `graphql_contract.rs` is the
/// `reqwest` transport itself, which is deliberate — that leaf must not gain a
/// `reqwest` dependency.
///
/// # Unparking, in one sentence
///
/// Delete the `#[ignore]` attribute and the three early-return gate blocks. The
/// request path below them already runs; there is nothing else to write.
///
/// # The three gates
///
/// - `PMCP_ATTESTATION_LIVE_TEST` — must be exactly `"1"`. Test-only; no
///   production path reads it.
/// - `PMCP_API_URL` — the endpoint. REUSED, never invented: it is already the
///   highest-precedence endpoint source for the shipped client
///   (`auth.rs:113-114`, ahead of the legacy alias, the configured target, and
///   the default). Phase 123's SC3 forbids a second pmcp.run API path, and
///   introducing a second base-URL variable here would pre-break it.
/// - `PMCP_ACCESS_TOKEN` — the credential. Also REUSED: it is one of the three
///   sources `get_credentials()` itself reads (`auth.rs:527-538`, the
///   documented CI/CD branch), so a live run authenticates through a source the
///   shipped client already honours.
///
/// # Why this reads the variable instead of calling `get_credentials()`
///
/// MEASURED: `auth.rs` references `crate::commands::configure::*` at
/// `auth.rs:150-153`, so the whole module lives in the BIN target only. It is
/// not mounted into the lib (`cargo-pmcp/src/lib.rs`'s `pub mod deployment`
/// exposes only `config`, `iam`, `widgets`, `post_deploy_tests` and
/// `google_cloud_run`), and an integration test in `tests/` therefore cannot
/// reach it. Mounting the oauth2/browser-flow tree into the lib target to make
/// one ignored test tidier is the wrong trade; the honest move is to read the
/// one variable and say so here.
///
/// WHAT THAT COSTS: a live run cannot use the interactive
/// `~/.pmcp/credentials.toml` file or the client-credentials
/// (`PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET`) flow. The operator exports
/// `PMCP_ACCESS_TOKEN` first.
///
/// # A second measured limitation the operator must know
///
/// The shipped client derives the GraphQL endpoint from the API base by running
/// discovery (`resolve_graphql_url` in `graphql.rs`), then calls
/// `execute_graphql_at` with the result. This test is the analogue of
/// `execute_graphql_at` — the layer that takes an explicit endpoint — because
/// discovery lives in the same bin-only tree as `get_credentials()`. So for a
/// live run the operator must export `PMCP_API_URL` pointing at the GRAPHQL
/// endpoint, not merely at the API base. Threading discovery back in belongs
/// with Phase 123's single-API-path work; inventing a second variable for it
/// here would be exactly the thing SC3 forbids.
///
/// # Open questions the first live run answers
///
/// Answering these means TIGHTENING the assertions below, not writing new ones:
///
/// 1. The verdict VOCABULARY — what strings the platform actually returns.
/// 2. The verified-identity SPELLING — key id, issuer URI, or org identifier.
/// 3. Whether a MISMATCHED subject digest produces a `verdict` value or a
///    GraphQL error. The decoder handles both today; only the platform can say
///    which one is the contract.
///
/// Run with:
/// ```sh
/// PMCP_ATTESTATION_LIVE_TEST=1 \
///   PMCP_API_URL=<real pmcp.run GraphQL endpoint> \
///   PMCP_ACCESS_TOKEN=<real token> \
///   cargo test -p cargo-pmcp --test package_attestation_contract \
///     verify_attestation_live -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "live network — requires PMCP_ATTESTATION_LIVE_TEST=1 + PMCP_API_URL + PMCP_ACCESS_TOKEN, and a pmcp.run backend that implements verifyAttestation (PARKED)"]
async fn verify_attestation_live() {
    // Gate 1 — explicit opt-in. Never reach the network by accident.
    if std::env::var("PMCP_ATTESTATION_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!(
            "verify_attestation_live skipped: set PMCP_ATTESTATION_LIVE_TEST=1 to enable the \
             live leg"
        );
        return;
    }

    // Gate 2 — the endpoint. Reused from the existing pmcp.run seam; see the
    // rustdoc above for why no second base-URL variable is introduced.
    let Some(api_url) = std::env::var("PMCP_API_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "verify_attestation_live skipped: set PMCP_API_URL to the pmcp.run GraphQL endpoint"
        );
        return;
    };

    // Gate 3 — the CREDENTIAL. PMCP_API_URL supplies an endpoint and nothing
    // else; without this gate the test would POST unauthenticated and report a
    // transport-level failure as if it were a contract finding.
    let Some(access_token) = std::env::var("PMCP_ACCESS_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "verify_attestation_live skipped: set PMCP_ACCESS_TOKEN to a real pmcp.run access \
             token (one of get_credentials()'s own three sources)"
        );
        return;
    };

    let payload = fixture_attestation_payload();
    let subject_digest = subject_digest_of(&payload);

    let body =
        cargo_pmcp::pmcp_run_graphql::verify_attestation_request_body(&payload, &subject_digest)
            .expect("production builder accepts a non-empty payload and digest");

    let response = reqwest::Client::new()
        .post(&api_url)
        .header(
            cargo_pmcp::pmcp_run_graphql::GRAPHQL_AUTH_HEADER,
            &access_token,
        )
        .json(&body)
        .send()
        .await
        .expect("POST to the pmcp.run GraphQL endpoint");

    let status = response.status();
    let parsed: serde_json::Value = response
        .json()
        .await
        .unwrap_or_else(|e| panic!("response (HTTP {status}) was not JSON: {e}"));

    // Decoded through the PRODUCTION decoder — assertions read typed fields,
    // never the request body or the header value, so a failure cannot echo the
    // access token into CI logs.
    let outcome = cargo_pmcp::pmcp_run_graphql::decode_verify_attestation_response(&parsed)
        .unwrap_or_else(|e| panic!("verifyAttestation (HTTP {status}) did not decode: {e}"));

    assert!(
        !outcome.verdict.trim().is_empty(),
        "the platform must answer with a non-empty verdict"
    );
    assert!(
        !outcome.verified_identity.trim().is_empty(),
        "the platform must name the identity it verified against — that is the \
         entire reason this call is remote"
    );
}
