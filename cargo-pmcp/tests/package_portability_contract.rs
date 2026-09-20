//! Offline blocking contract test for the pmcp.run package-portability
//! contract (Phase 123 plan 02, PKGX-02).
//!
//! Validates that the CLI's `getPackageArtifact` operation
//! (`GET_PACKAGE_ARTIFACT_QUERY` in
//! `src/deployment/targets/pmcp_run/graphql_contract.rs`) has not drifted from
//! the vendored SDL at `contracts/pmcp-run/portability-v1.graphql`.
//!
//! This is a pure offline/static check: no network access, no pmcp.run
//! credentials. (The fifth test in this file CAN reach the network — it is
//! `#[ignore]`d behind a double env gate; see its own docs.)
//!
//! It is executed by the `test-cargo-pmcp-integration` Makefile target, which
//! is chained into `test-all` and therefore into `make quality-gate`. That
//! target does not merely run this file — it asserts a NONZERO passed count for
//! this binary BY NAME, via `scripts/named-test-binary-count.awk`. That
//! per-binary assertion is what makes the word "blocking" a measured property
//! rather than a claim: an `#[ignore]` sweep or a `#[cfg]` gate turning false
//! here reports `0 passed` and fails the build, even though the summed total
//! across the target's other binaries would stay comfortably nonzero. The
//! append registering this binary in BOTH of that target's lists was made in
//! the same commit that created this file — see the Makefile's own comment
//! block for why a name that lands BEFORE its binary is the hazard, and a name
//! that lands WITH it is the discipline.
//!
//! # What this file CANNOT prove — read this before trusting a green run
//!
//! **Both halves of the comparison are SDK-written.** The schema
//! (`contracts/pmcp-run/portability-v1.graphql`) was authored here, not exported
//! from a live API, and the query it is checked against was authored here too.
//! The sibling `package_capture_contract.rs` validates SDK queries against a
//! PLATFORM-EXPORTED schema; this file does not, and neither does the other
//! sibling, `package_attestation_contract.rs`.
//!
//! So this test pins **SDK-INTERNAL agreement** today — that the operation
//! string and the proposed schema cannot drift apart, and that a change to one
//! forces a change to the other in the same PR. **It cannot detect drift from a
//! platform that has not spoken.** A green run here is not platform agreement,
//! and nobody should read it as one.
//!
//! **And that window is LONGER than `capture-v1` would suggest.** Per the
//! pmcp.run team's 2026-08-26 correction, a real cross-boundary drift net
//! arrives with IMPLEMENTATION, not with ratification: a ratified-but-
//! unimplemented operation still has no live schema to export, so there is
//! still nothing on the other side of the boundary to be pinned against. Until
//! `getPackageArtifact` is implemented and its SDL exported, every green run
//! here means only that the SDK agrees with itself.
//!
//! The ask lives in `docs/design/package-portability-pmcp-run-handoff.md` §5.1.

use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

use cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_ARTIFACT_QUERY;

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/portability-v1.graphql"
);

/// The contract leaf carrying the SDK's half of this contract. Scanned by
/// `sdl_is_honest_about_its_provenance_and_parking` for parking discipline.
const CONTRACT_LEAF_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/deployment/targets/pmcp_run/graphql_contract.rs"
);

/// The fields the SDK reads back out of `GetPackageArtifactReturnType`. Kept in
/// one place so the validation test, the drift check and the live leg cannot
/// disagree about them.
const EXPECTED_RESPONSE_FIELDS: [&str; 3] = ["payloadDigest", "downloadUrl", "expiresAt"];

// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read portability-v1.graphql");
    Schema::parse_and_validate(sdl, "portability-v1.graphql").expect("vendored SDL is itself valid")
}

/// The SDL with every `#` comment removed, so shape assertions read the SCHEMA
/// and not the prose about it.
///
/// This is load-bearing, not tidiness. `portability-v1.graphql`'s header
/// deliberately DISCUSSES the words `enum`, `push` and `import` in order to
/// record why they are absent, and its field comments quote the very question
/// text the schema must not assert as fact. A naive `sdl.contains("enum")`
/// would therefore be satisfied by the very comment explaining the ban — a
/// self-invalidating check. Copied in substance from
/// `package_attestation_contract.rs`, which makes the same argument.
fn sdl_body() -> String {
    std::fs::read_to_string(SDL_PATH)
        .expect("read portability-v1.graphql")
        .lines()
        .map(|line| match line.find('#') {
            Some(idx) => line[..idx].to_string(),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The field names the operation actually selects out of `getPackageArtifact`,
/// read from the PARSED operation rather than by string matching, so an added
/// field cannot hide behind formatting.
fn selected_response_fields() -> Vec<String> {
    let schema = schema();
    let doc = ExecutableDocument::parse_and_validate(
        &schema,
        GET_PACKAGE_ARTIFACT_QUERY,
        "get_package_artifact.graphql",
    )
    .expect("the operation parses and validates (see the dedicated test for the message)");

    let operation = doc
        .operations
        .get(None)
        .expect("the document declares exactly one operation");

    let root_field = operation
        .selection_set
        .selections
        .iter()
        .find_map(|selection| match selection {
            apollo_compiler::executable::Selection::Field(field)
                if field.name.as_str() == "getPackageArtifact" =>
            {
                Some(field)
            },
            _ => None,
        })
        .expect("the operation selects the `getPackageArtifact` root field");

    root_field
        .selection_set
        .selections
        .iter()
        .filter_map(|selection| match selection {
            apollo_compiler::executable::Selection::Field(field) => {
                Some(field.name.as_str().to_string())
            },
            _ => None,
        })
        .collect()
}

/// SC4's core assertion: the runtime operation must validate against the
/// vendored contract.
///
/// It validates the CLIENT'S constant — imported from
/// `cargo_pmcp::pmcp_run_graphql` — and not a copy of the query text, so the
/// test cannot drift from the client it is supposed to pin.
#[test]
fn get_package_artifact_op_validates_against_contract() {
    let schema = schema();
    ExecutableDocument::parse_and_validate(
        &schema,
        GET_PACKAGE_ARTIFACT_QUERY,
        "get_package_artifact.graphql",
    )
    .unwrap_or_else(|e| {
        panic!("`getPackageArtifact` op does not match portability-v1.graphql: {e}")
    });
}

/// Self-admitted-technical-debt markers, split across a `concat!` boundary so
/// that THIS FILE does not itself become a hit for the repo-wide SATD scanners
/// that read it. A check that creates the thing it bans is the same class of
/// self-invalidating mistake `sdl_body()` exists to avoid.
const SATD_MARKERS: [&str; 4] = [
    concat!("TO", "DO"),
    concat!("FIX", "ME"),
    concat!("HA", "CK"),
    concat!("XX", "X"),
];

/// The honesty pin, in two halves — one for each side of the parked contract.
///
/// **The SDL half.** It must say `SDK-PROPOSED` and must carry NEITHER a
/// source-naming nor an export-date provenance line. Its sibling
/// `capture-v1.graphql` carries both because it genuinely was exported from the
/// live pmcp.run AppSync API; this file was not, and imitating that header
/// would be a lie about who owns it. Asserted here rather than left to the eye.
/// These assertions read the RAW file deliberately — the provenance header IS
/// comments, so asserting its absence is precisely a statement about comments.
///
/// **The Rust half.** The parked state in `graphql_contract.rs` is expressed
/// with `#[allow(dead_code)]` plus rustdoc, never with a self-admitted-
/// technical-debt marker (C-3). A parked contract announced by a debt marker
/// reads as rot to be cleaned up rather than as a contract awaiting a
/// counterparty, and the repo has zero tolerance for SATD besides.
#[test]
fn sdl_is_honest_about_its_provenance_and_parking() {
    let raw = std::fs::read_to_string(SDL_PATH).expect("read portability-v1.graphql");

    assert!(
        raw.contains("SDK-PROPOSED"),
        "portability-v1.graphql must mark itself SDK-PROPOSED — it was not exported \
         from any live API"
    );

    for line in raw.lines() {
        let trimmed = line.trim_start();
        for forged in ["# Source:", "# Exported:"] {
            assert!(
                !trimmed.starts_with(forged),
                "portability-v1.graphql has no provenance and must not imply one; \
                 found a `{forged}` header line: {line}"
            );
        }
    }

    let leaf = std::fs::read_to_string(CONTRACT_LEAF_PATH).expect("read graphql_contract.rs");
    for marker in SATD_MARKERS {
        assert!(
            !leaf.contains(marker),
            "graphql_contract.rs must express parking with #[allow(dead_code)] plus \
             rustdoc, never with a `{marker}` debt marker (C-3)"
        );
    }
}

/// The shape pin: the three response fields are declared, `reference` is a
/// non-null `String` argument, exactly one operation field is proposed, and the
/// SDL declares no enum.
///
/// An enum would make any later schema-versus-schema diff (this proposal vs.
/// the platform's eventual export) show permanent drift — the same discipline
/// `capture-v1.graphql` records for its `status` fields and
/// `attestation-v1.graphql` for its `verdict`.
///
/// Everything here reads `sdl_body()`, the comment-stripped text. The header
/// discusses `enum`, `push` and `import` precisely in order to record why they
/// are absent, so reading the raw file would let the ban's own explanation
/// satisfy the ban.
#[test]
fn sdl_shape_is_pinned_to_d02_scope() {
    let body = sdl_body();

    for field in EXPECTED_RESPONSE_FIELDS {
        assert!(
            body.contains(&format!("{field}: String!")),
            "`{field}` must be declared `String!` in portability-v1.graphql:\n{body}"
        );
    }

    assert!(
        body.contains("getPackageArtifact(reference: String!)"),
        "the operation takes a single non-null String reference:\n{body}"
    );

    for line in body.lines() {
        assert!(
            !line.trim_start().starts_with("enum "),
            "portability-v1.graphql must declare no enum, found: {line}"
        );
        assert!(
            !line.contains(" enum "),
            "portability-v1.graphql must declare no enum, found: {line}"
        );
    }

    // Exactly one operation field is proposed — the whole point of D-02. The
    // retired `push` (D-01) and the platform-owned `import` (D-03) may appear
    // only in a `#` comment explaining their absence, which `sdl_body()` has
    // already removed.
    assert_eq!(
        body.matches("getPackageArtifact(").count(),
        1,
        "the proposal declares exactly one operation field:\n{body}"
    );
    for omitted in ["submitImport", "getImportStatus", "pushPackageArtifact"] {
        assert!(
            !body.contains(omitted),
            "`{omitted}` is outside D-02's scope and must not be declared \
             (it may appear only in a `#` comment explaining the omission)"
        );
    }
}

/// The selection-set drift check: the operation selects EXACTLY the fields the
/// SDK reads back — no more, no fewer.
///
/// "No fewer" catches a decoder that would read a field the query stopped
/// asking for. "No more" catches the opposite drift, a field added to the query
/// without being added to the SDL, which is why this and
/// `get_package_artifact_op_validates_against_contract` go red together on that
/// change (recorded as a negative control in this plan's SUMMARY) — one because
/// the schema does not declare it, one because the SDK does not read it.
///
/// It reads the PARSED selection set, not the query text, so whitespace or a
/// comment inside the operation cannot hide an added field.
#[test]
fn selection_set_matches_expected_response_fields_exactly() {
    let mut selected = selected_response_fields();
    selected.sort_unstable();

    let mut expected: Vec<String> = EXPECTED_RESPONSE_FIELDS
        .iter()
        .map(|f| (*f).to_string())
        .collect();
    expected.sort_unstable();

    assert_eq!(
        selected, expected,
        "the getPackageArtifact selection set drifted from the fields the SDK reads back"
    );
}

// ===========================================================================
// The PARKED live leg (PKGX-F1)
// ===========================================================================

/// PKGX-02's live egress leg — PARKED on a pmcp.run backend that does not
/// implement `getPackageArtifact` yet.
///
/// # This is an executable request path, not a description of one
///
/// Below the two gates, this test builds its body with the PRODUCTION builder
/// (`get_package_artifact_request_body`), POSTs it with the PRODUCTION header
/// constant (`GRAPHQL_AUTH_HEADER`), and decodes the answer with the PRODUCTION
/// decoder (`decode_get_package_artifact_response`). Nothing here is a stub:
/// the only piece that lives in the test rather than in `graphql_contract.rs`
/// is the `reqwest` transport itself, which is deliberate — that leaf must not
/// gain a `reqwest` dependency. Plan 05 wired the same transport into
/// `graphql.rs` for the shipped `pull` verb; this leg is its analogue at the
/// explicit-endpoint layer.
///
/// **And then it runs the WHOLE shipped verb** (plan 05): after the decode
/// assertions it invokes `cargo pmcp package pull` against the same reference
/// and asserts a working layout landed. The explicit-endpoint half above
/// answers "does the platform's ANSWER match the contract?"; the verb half
/// answers "do the platform's BYTES survive the gates?". Both are questions
/// only a live run can settle.
///
/// # Unparking, in one sentence
///
/// Delete the `#[ignore]` attribute and the two early-return gate blocks. The
/// request path below them already runs; there is nothing else to write.
///
/// # The two gates
///
/// - `PMCP_PORTABILITY_LIVE_TEST` — must be exactly `"1"`. Test-only; no
///   production path reads it.
/// - `PMCP_API_URL` — the endpoint, and `PMCP_ACCESS_TOKEN` — the credential.
///   Both REUSED, never invented: `PMCP_API_URL` is already the
///   highest-precedence endpoint source for the shipped client, and
///   `PMCP_ACCESS_TOKEN` is one of the three sources `get_credentials()` itself
///   reads (the documented CI/CD branch). SC3 forbids a second pmcp.run API
///   path, and introducing a second base-URL variable here would pre-break it.
///
/// Each gate PRINTS why it skipped. A silent skip is indistinguishable from a
/// pass, and the print is the part people drop.
///
/// # Why this reads the variables instead of calling `get_credentials()`
///
/// MEASURED and unchanged since Phase 122: `auth.rs` references
/// `crate::commands::configure::*`, so the whole module lives in the BIN target
/// only and an integration test in `tests/` cannot reach it. Mounting the
/// oauth2/browser-flow tree into the lib target to make one ignored test tidier
/// is the wrong trade. The cost: a live run cannot use the interactive
/// `~/.pmcp/credentials.toml` file or the client-credentials flow, and
/// `PMCP_API_URL` must point at the GRAPHQL endpoint rather than the API base,
/// because endpoint discovery lives in that same bin-only tree.
///
/// # What the backend must ship, and what the first live run answers
///
/// The assertions below already NAME the contract, so answering these means
/// TIGHTENING them, not writing new ones:
///
/// 1. Whether `payloadDigest` is the OCI manifest digest or a digest over the
///    tar bytes (the SDK assumes the former — SDL comment, research A4).
/// 2. Whether `downloadUrl` is fetched with a plain unauthenticated GET (A3).
///    The verb half below now DOES fetch it, and doing so is safe by
///    construction rather than by restraint: the shipped downloader takes no
///    credential parameter, so it cannot attach the pmcp.run header to another
///    origin. A live run therefore answers A3 directly — if the presigned URL
///    requires authentication, the pull fails at the download stage and the
///    platform's answer is "no".
/// 3. Whether the tar behind it is uncompressed and tolerates an `oci-layout`
///    marker entry (A2, A6).
///
/// Run with:
/// ```sh
/// PMCP_PORTABILITY_LIVE_TEST=1 \
///   PMCP_API_URL=<real pmcp.run GraphQL endpoint> \
///   PMCP_ACCESS_TOKEN=<real token> \
///   cargo test -p cargo-pmcp --test package_portability_contract \
///     get_package_artifact_live -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "live network — requires PMCP_PORTABILITY_LIVE_TEST=1 + PMCP_API_URL + PMCP_ACCESS_TOKEN, and a pmcp.run backend that implements getPackageArtifact (PARKED)"]
async fn get_package_artifact_live() {
    // Gate 1 — explicit opt-in. Never reach the network by accident.
    if std::env::var("PMCP_PORTABILITY_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_PORTABILITY_LIVE_TEST=1 to enable the \
             live leg"
        );
        return;
    }

    // Gate 2 — the endpoint AND the credential. Without the credential the test
    // would POST unauthenticated and report a transport-level failure as if it
    // were a contract finding.
    let Some(api_url) = std::env::var("PMCP_API_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_API_URL to the pmcp.run GraphQL endpoint"
        );
        return;
    };
    let Some(access_token) = std::env::var("PMCP_ACCESS_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_ACCESS_TOKEN to a real pmcp.run access \
             token (one of get_credentials()'s own three sources)"
        );
        return;
    };

    let reference = std::env::var("PMCP_PORTABILITY_LIVE_REFERENCE")
        .unwrap_or_else(|_| "london-tube@1.0.0".to_string());

    let body = cargo_pmcp::pmcp_run_graphql::get_package_artifact_request_body(&reference)
        .expect("production builder accepts a non-empty reference");

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

    // Decoded through the PRODUCTION decoder. Assertions read typed fields and
    // never echo `download_url` — it is a bearer credential (§5.1), so a
    // failure here must not print it into CI logs.
    let outcome = cargo_pmcp::pmcp_run_graphql::decode_get_package_artifact_response(&parsed)
        .unwrap_or_else(|e| panic!("getPackageArtifact (HTTP {status}) did not decode: {e}"));

    assert!(
        outcome.payload_digest.starts_with("sha256:"),
        "the platform must answer with a `sha256:`-prefixed digest the SDK can \
         compare its locally re-derived value against"
    );
    assert!(
        !outcome.download_url.trim().is_empty(),
        "the platform must answer with a download location — that is the entire \
         reason this call exists (value deliberately not printed: bearer credential)"
    );
    assert!(
        !outcome.expires_at.trim().is_empty(),
        "a presigned URL with no stated expiry cannot be reasoned about; §5.1 \
         proposes ~5 minutes"
    );

    // ---------------------------------------------------------------
    // The FULL pipeline against the real backend (Phase 123 plan 05)
    // ---------------------------------------------------------------
    //
    // Everything above stops at the decode boundary. Plan 02 deliberately did
    // NOT fetch `downloadUrl` there, because doing so before the platform
    // confirmed the auth shape risked sending a pmcp.run token to another
    // origin. That risk is now retired STRUCTURALLY rather than by restraint:
    // the shipped downloader (`download_artifact_bytes` in
    // `deployment/targets/pmcp_run/graphql.rs`) takes no credential parameter
    // at all, so it cannot attach the header even if asked to.
    //
    // So the live leg now drives the WHOLE verb — request, download, verify,
    // install, report — through the shipped binary, which resolves its endpoint
    // and credential from the same two variables gating this test.
    // `cargo_pmcp::package_pull_pipeline` itself is exercised offline by the
    // section below this one; what only a live run can answer is whether the
    // PLATFORM's bytes satisfy the gates, and that is what this does.
    //
    // This is deliberately an extension of the existing live test rather than a
    // second one: one gate to delete at unparking, not two.
    let scratch = tempfile::tempdir().expect("create a temp dir");
    let dest = scratch.path().join("pulled-layout");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .env("PMCP_API_URL", &api_url)
        .env("PMCP_ACCESS_TOKEN", &access_token)
        .args([
            "package",
            "pull",
            &reference,
            "--output",
            dest.to_str().expect("utf-8 path"),
        ])
        .assert()
        .success();

    assert!(
        dest.join("index.json").exists(),
        "a live pull must materialize a working OCI layout at the destination"
    );
    assert!(
        dest.join("oci-layout").exists(),
        "the layout marker is regenerated on write"
    );
}

// ===========================================================================
// The OFFLINE pull pipeline (Phase 123 plan 05, PKGX-02)
// ===========================================================================
//
// Everything below drives the REAL `package pull` pipeline —
// `cargo_pmcp::package_pull_pipeline` — with a fake transport, so the whole
// verb runs with no backend in existence. Only the ONE impure step is
// substituted; request building, verification, the transactional install and
// the report are the shipped code paths.
//
// # Why the pipeline had to move into the lib for any of this to be possible
//
// `cargo-pmcp/src/lib.rs` declares no `mod commands`, so a pipeline declared
// only under `commands/package/` compiles into the BIN target and nowhere else.
// This file is an external crate linking the LIB: it can neither see a
// bin-private module nor implement a trait declared in one. The `#[path]`-mount
// of `pull_pipeline.rs` as `cargo_pmcp::package_pull_pipeline` is what makes
// `ArtifactTransport` implementable here at all.
//
// A contributor who wants to "simplify" by moving the pipeline back under
// `commands/` should know the cost first: this entire section stops COMPILING,
// while `cargo build -p cargo-pmcp` stays green. That contrast is recorded as a
// negative control in the plan's SUMMARY.
//
// # The refusal inputs are bytes NEITHER SIDE GENERATED
//
// They come from `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/` —
// checked-in bytes authored from the framing specification by a one-off script
// that touched no pmcp code, and never regenerated from the writer under test.
// That makes the `pull` path provable against bytes neither the SDK's writer nor
// this test produced, which is as close to a platform-produced artifact as this
// repo can get and strictly stronger than a self-generated tar.
//
// # The ACCEPT path cannot use that corpus, and the reason is MEASURED
//
// `conformant.tar` clears every framing, integrity and descriptor-graph gate but
// then fails `unpack_server` with "manifest is missing the 'bootstrap or
// binary-ref' layer" (measured 2026-08-26 by running the shipped `package load`
// against it). It is therefore used here as the SEMANTIC hostile case — the
// class that reaches a write in an install-then-validate design — and the accept
// path is driven by a real package produced by `cargo pmcp package save`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use assert_cmd::Command;
use cargo_pmcp::package_artifact::ArtifactLimits;
use cargo_pmcp::package_pull_pipeline::{
    pull_package, pull_package_with_limits, ArtifactTransport, FetchedArtifact,
    PARKED_CAPABILITY_CONTEXT, STAGE_DOWNLOAD, STAGE_INSTALL, STAGE_VERIFY,
};

/// The checked-in framing corpus, resolved relative to `cargo-pmcp/`.
const ARTIFACT_CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/"
);

/// The checked-in london-tube config-server fixture the accept path packs.
const LONDON_TUBE_CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/"
);

/// Read one framing fixture's bytes.
fn artifact_fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(ARTIFACT_CORPUS).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

// ---------------------------------------------------------------------
// The seam double
// ---------------------------------------------------------------------

/// A transport double that records how many times it was invoked.
///
/// The counter is not decoration. An assertion that `pull` "returned an error"
/// cannot distinguish a refusal that happened BEFORE the network from one that
/// happened after a wasted round trip — and "refuses before any network call" is
/// precisely what two of the behaviours below claim.
struct SeamDouble {
    calls: AtomicUsize,
    answer: Box<dyn Fn() -> anyhow::Result<FetchedArtifact> + Send + Sync>,
}

impl SeamDouble {
    fn new(answer: impl Fn() -> anyhow::Result<FetchedArtifact> + Send + Sync + 'static) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            answer: Box::new(answer),
        }
    }

    /// A double that hands back `tar_bytes` under `declared_payload_digest`.
    fn serving(tar_bytes: Vec<u8>, declared_payload_digest: String) -> Self {
        Self::new(move || {
            Ok(FetchedArtifact {
                tar_bytes: tar_bytes.clone(),
                declared_payload_digest: declared_payload_digest.clone(),
            })
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl ArtifactTransport for SeamDouble {
    async fn fetch_artifact(
        &self,
        _request_body: &serde_json::Value,
    ) -> anyhow::Result<FetchedArtifact> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        (self.answer)()
    }
}

/// A double whose answer is always a transport-level failure.
fn failing_double() -> SeamDouble {
    SeamDouble::new(|| {
        Err(anyhow::anyhow!(
            "simulated transport failure: connection refused (os error 61)"
        ))
    })
}

/// The digest a conformant answer would declare for `tar_bytes`, derived the
/// same way the pipeline derives it — locally, over the manifest blob.
///
/// Used ONLY to build a WELL-FORMED answer for cases whose subject is something
/// other than the digest check. The mismatch case deliberately does not use it.
fn declared_digest_for(tar_bytes: &[u8]) -> String {
    cargo_pmcp::package_artifact::read_verified(tar_bytes)
        .expect("this helper is only called with bytes that verify")
        .manifest_digest
        .as_str()
        .to_string()
}

// ---------------------------------------------------------------------
// Refusal helper — every refusal must leave the destination untouched
// ---------------------------------------------------------------------

/// Drive `pull` with a double serving `fixture`, assert it refuses naming
/// `needle`, and assert the destination does not exist afterwards.
///
/// The destination check is the half that matters. A test asserting only the
/// error would pass against an implementation that wrote a partial layout and
/// then failed — which is exactly the ordering `install_layout` exists to
/// prevent, and exactly what an install-then-validate design would do.
async fn pull_refusal(fixture: &str, needle: &str) {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");
    assert!(!dest.exists(), "the destination must start out absent");

    // These fixtures are refused before the digest comparison is reached, so
    // the declared value is deliberately irrelevant.
    let double = SeamDouble::serving(artifact_fixture(fixture), "sha256:unused".to_string());

    let error = pull_package(&double, "framing-example@1.0.0", &dest, false)
        .await
        .err()
        .unwrap_or_else(|| panic!("{fixture} must be refused by pull"));

    assert_eq!(
        double.calls(),
        1,
        "the transport must have been invoked once"
    );

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(needle),
        "the refusal for {fixture} must name its own cause.\n  wanted: {needle}\n  got: {rendered}"
    );
    assert!(
        !dest.exists(),
        "{fixture} was refused but {} exists — a refused pull must write nothing",
        dest.display()
    );
}

// ---------------------------------------------------------------------
// One refusal test per hostile golden fixture
// ---------------------------------------------------------------------
//
// Each needle is DISTINCT. Asserting only `is_err()` would let a single early
// gate swallow every case while eleven tests stayed green — the regression this
// corpus exists to catch, now checked through the whole `pull` pipeline rather
// than only at the reader.

#[tokio::test]
async fn pull_refuses_a_parent_directory_component_writing_nothing() {
    pull_refusal(
        "hostile_parent_directory_component.tar",
        "parent-directory ('..') component",
    )
    .await;
}

#[tokio::test]
async fn pull_refuses_an_absolute_path_writing_nothing() {
    pull_refusal("hostile_absolute_path.tar", "the path is absolute").await;
}

#[tokio::test]
async fn pull_refuses_a_symlink_entry_writing_nothing() {
    pull_refusal(
        "hostile_symlink_entry.tar",
        "only regular files are admitted",
    )
    .await;
}

#[tokio::test]
async fn pull_refuses_a_wrapper_directory_writing_nothing() {
    pull_refusal(
        "hostile_wrapper_directory.tar",
        "framing-example/oci-layout",
    )
    .await;
}

#[tokio::test]
async fn pull_refuses_a_duplicate_path_writing_nothing() {
    pull_refusal(
        "hostile_duplicate_path.tar",
        "refusing duplicate archive entry",
    )
    .await;
}

#[tokio::test]
async fn pull_refuses_a_blob_digest_mismatch_writing_nothing() {
    pull_refusal(
        "hostile_blob_digest_mismatch.tar",
        "blob content does not match its own name",
    )
    .await;
}

#[tokio::test]
async fn pull_refuses_an_artifact_with_no_index_writing_nothing() {
    pull_refusal("hostile_no_index.tar", "carries no index.json").await;
}

#[tokio::test]
async fn pull_refuses_an_empty_archive_writing_nothing() {
    pull_refusal("hostile_empty_archive.tar", "contains no entries at all").await;
}

#[tokio::test]
async fn pull_refuses_a_dangling_descriptor_writing_nothing() {
    pull_refusal("hostile_dangling_descriptor.tar", "dangling descriptor").await;
}

#[tokio::test]
async fn pull_refuses_an_orphan_blob_writing_nothing() {
    pull_refusal("hostile_orphan_blob.tar", "orphan blob").await;
}

#[tokio::test]
async fn pull_refuses_two_manifests_writing_nothing() {
    pull_refusal(
        "hostile_two_manifests.tar",
        "expected exactly one manifest in index.json, found 2",
    )
    .await;
}

// ---------------------------------------------------------------------
// The SEMANTIC hostile case (review finding H4)
// ---------------------------------------------------------------------

/// An artifact that passes framing, integrity AND descriptor-graph closure, and
/// fails only at `unpack_*`.
///
/// This is the class an install-then-validate design would WRITE before
/// failing, so it is the case that actually distinguishes `install_layout`'s
/// stage-validate-rename from a plain write. The framing fixtures above never
/// reach a write in the first place, so on their own they would leave the
/// "destination unchanged on failure" claim tested on the easy half only.
///
/// `conformant.tar` is used because it is INDEPENDENTLY AUTHORED and happens to
/// occupy exactly this class: measured, it is accepted all the way through
/// `read_verified` and refused by `unpack_server` for a missing bootstrap layer.
#[tokio::test]
async fn pull_refuses_a_semantically_malformed_package_writing_nothing() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");

    let bytes = artifact_fixture("conformant.tar");
    let declared = declared_digest_for(&bytes);
    let double = SeamDouble::serving(bytes, declared);

    let error = pull_package(&double, "framing-example@1.0.0", &dest, false)
        .await
        .err()
        .expect("a semantically malformed package must be refused");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("bootstrap or binary-ref"),
        "the refusal must come from the SEMANTIC gate, not an earlier one: {rendered}"
    );
    assert!(
        error.chain().any(|c| c.to_string().contains(STAGE_INSTALL)),
        "the chain must identify the install stage: {rendered}"
    );
    assert!(
        !dest.exists(),
        "a semantic failure must leave {} absent — that is what staging buys",
        dest.display()
    );
}

// ---------------------------------------------------------------------
// The declared-digest cross-check
// ---------------------------------------------------------------------

/// The platform's declared `payloadDigest` is never taken as authority over the
/// bytes: a mismatch refuses with nothing written.
///
/// The bytes here are perfectly good — it is the CLAIM about them that is
/// wrong, which is the case a reader that trusted the transport would miss
/// entirely.
///
/// It is driven with a package that WOULD install cleanly, and that choice is
/// load-bearing rather than incidental. Fed a package that fails some later
/// gate anyway, this test's "destination does not exist" assertion would stay
/// green no matter where the digest comparison sat in the pipeline — it would
/// be measuring the other gate. With a package that installs, the assertion
/// genuinely pins the ORDERING, which is what the recorded negative control in
/// this plan's SUMMARY demonstrates.
#[tokio::test]
async fn pull_refuses_a_declared_digest_that_does_not_match_the_bytes() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");

    let (_project, bytes) = saved_london_tube_tar();
    let honest = declared_digest_for(&bytes);
    let lie = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    assert_ne!(honest, lie, "the fixture must not collide with the lie");

    let double = SeamDouble::serving(bytes, lie.to_string());

    let error = pull_package(&double, "london-tube@1.0.0", &dest, false)
        .await
        .err()
        .expect("a declared-digest mismatch must be refused");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("payloadDigest mismatch"),
        "the refusal must name the digest cross-check: {rendered}"
    );
    assert!(
        rendered.contains(&honest),
        "the refusal must show the locally re-derived digest: {rendered}"
    );
    assert!(
        !dest.exists(),
        "a digest mismatch must leave {} absent",
        dest.display()
    );
}

// ---------------------------------------------------------------------
// The byte cap
// ---------------------------------------------------------------------

/// An artifact over the in-memory budget is refused NAMING the cap.
///
/// Driven through the injectable limits rather than the production default for
/// the reason `ArtifactLimits` records: proving a gibibyte cap with real bytes
/// is not a test anyone will run, so the cap is made falsifiable by feeding one
/// input under a tiny budget and asserting the specific refusal.
///
/// This is the IN-MEMORY cap. Its sibling — the streaming download cap enforced
/// with a running total over `Response::chunk()` — lives behind the transport
/// seam by construction and so is not reachable from a test that substitutes
/// that seam; it is pinned by the two constants and by reading.
#[tokio::test]
async fn pull_refuses_an_artifact_over_the_byte_cap_naming_it() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");

    let bytes = artifact_fixture("conformant.tar");
    let declared = declared_digest_for(&bytes);
    let double = SeamDouble::serving(bytes, declared);

    let tiny = ArtifactLimits {
        per_entry: 16,
        total: 32,
    };

    let error = pull_package_with_limits(&double, "framing-example@1.0.0", &dest, false, tiny)
        .await
        .err()
        .expect("an over-cap artifact must be refused");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("16") || rendered.contains("32"),
        "the refusal must name the cap that caused it: {rendered}"
    );
    assert!(
        error.chain().any(|c| c.to_string().contains(STAGE_VERIFY)),
        "the chain must identify the verification stage: {rendered}"
    );
    assert!(!dest.exists(), "an over-cap artifact must write nothing");
}

// ---------------------------------------------------------------------
// D-05 — every failure names the capability, cause chain intact
// ---------------------------------------------------------------------

/// A transport failure surfaces under ONE line naming the capability the
/// platform has not shipped, with the real cause still one `-v` away.
///
/// This test drives the pipeline DIRECTLY and never touches clap, which is why
/// the context frame is applied at the pipeline's entry point rather than in
/// `pull.rs`: a frame applied only in the bin would be a frame no test ever
/// exercises.
#[tokio::test]
async fn a_transport_failure_names_the_parked_capability_and_keeps_its_cause() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");
    let double = failing_double();

    let error = pull_package(&double, "london-tube@1.0.0", &dest, false)
        .await
        .err()
        .expect("a transport failure must surface");

    assert_eq!(double.calls(), 1, "the transport must have been invoked");
    assert_eq!(
        error.to_string(),
        PARKED_CAPABILITY_CONTEXT,
        "the OUTERMOST frame must name getPackageArtifact and say it is not yet available"
    );
    assert!(
        error.to_string().contains("getPackageArtifact"),
        "the top-level message must name the missing platform capability"
    );
    assert!(
        error.to_string().contains("not yet available"),
        "the top-level message must say the capability is not yet available"
    );

    // Walked, not string-matched over the whole formatted output: the claim is
    // that the ORIGINAL error is REACHABLE, not that its text appears somewhere.
    assert!(
        error
            .chain()
            .skip(1)
            .any(|c| c.to_string().contains("connection refused")),
        "the original cause must remain reachable BELOW the frame: {error:#}"
    );
    assert!(
        error
            .chain()
            .any(|c| c.to_string().contains(STAGE_DOWNLOAD)),
        "the chain must identify the download stage: {error:#}"
    );
    assert!(!dest.exists(), "a failed pull must write nothing");
}

// ---------------------------------------------------------------------
// The PRE-NETWORK refusals, proven to be pre-network
// ---------------------------------------------------------------------

/// A pre-existing destination is refused WITHOUT a round trip.
#[tokio::test]
async fn a_pre_existing_destination_is_refused_before_the_double_is_invoked() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");
    std::fs::create_dir_all(&dest).expect("create the destination");
    let double = failing_double();

    let error = pull_package(&double, "london-tube@1.0.0", &dest, false)
        .await
        .err()
        .expect("a pre-existing destination must be refused without --force");

    assert_eq!(
        double.calls(),
        0,
        "the destination check must run BEFORE any network call — an error alone \
         cannot distinguish a refusal from a failed round trip"
    );
    assert!(
        format!("{error:#}").contains("already exists"),
        "the refusal must name its own cause: {error:#}"
    );
}

/// An empty reference is refused WITHOUT a round trip.
#[tokio::test]
async fn an_empty_reference_is_refused_before_the_double_is_invoked() {
    let tmp = tempfile::tempdir().expect("create a temp dir");
    let dest = tmp.path().join("destination-layout");
    let double = failing_double();

    let error = pull_package(&double, "   ", &dest, false)
        .await
        .err()
        .expect("an empty reference must be refused");

    assert_eq!(
        double.calls(),
        0,
        "an empty reference must be refused BEFORE any network call"
    );
    assert!(
        format!("{error:#}").contains("EMPTY reference"),
        "the refusal must name its own cause: {error:#}"
    );
    assert!(!dest.exists(), "a refused pull must write nothing");
}

// ---------------------------------------------------------------------
// The ACCEPT path, and D-16: `pull` and `load` agree
// ---------------------------------------------------------------------

/// A `.pmcp/deploy.toml` that parses BOTH as cargo-pmcp's `DeployConfig` and as
/// `pmcp-package`'s narrower closed-set `DeployDescriptor`. Kept byte-identical
/// in intent to `package_save_load.rs`'s copy: this file drives the same `save`
/// path to obtain a real package.
const LONDON_TUBE_DEPLOY_TOML: &str = r#"[target]
type = "pmcp-run"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "london-tube"
memory_mb = 1024
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[secrets]

[auth]
enabled = false
provider = "none"
callback_urls = []

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[assets]
include = []
exclude = ["**/*.tmp"]
"#;

/// Save the london-tube fixture into a fresh project and return
/// `(temp dir, tar bytes)`.
///
/// The accept path needs a SEMANTICALLY VALID package, which the framing corpus
/// deliberately does not contain (see this section's header). Producing one with
/// the shipped `save` verb is the honest way to get one: the bytes are then a
/// real artifact rather than a hand-assembled approximation of one.
///
/// The `TempDir` is returned so the caller keeps it alive — dropping it deletes
/// everything.
fn saved_london_tube_tar() -> (tempfile::TempDir, Vec<u8>) {
    let dir = tempfile::tempdir().expect("create a temp project");
    let root = dir.path();

    let corpus = PathBuf::from(LONDON_TUBE_CORPUS);
    let config = root.join("london-tube.toml");
    std::fs::copy(corpus.join("london-tube.toml"), &config).expect("copy the fixture config");
    std::fs::copy(
        corpus.join("london-tube-api.yaml"),
        root.join("london-tube-api.yaml"),
    )
    .expect("copy the fixture spec");

    let pmcp_dir = root.join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir).expect("create .pmcp/");
    std::fs::write(pmcp_dir.join("deploy.toml"), LONDON_TUBE_DEPLOY_TOML)
        .expect("write .pmcp/deploy.toml");

    // A configuration server NAMES its runtime binary rather than carrying one,
    // so any well-formed digest exercises the same path the real one would.
    let binary_digest = pmcp_package::ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.1.0")
        .as_str()
        .to_string();

    let output = root.join("london-tube.tar");
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "package",
            "save",
            "--config",
            config.to_str().expect("utf-8 path"),
            "--spec",
            root.join("london-tube-api.yaml")
                .to_str()
                .expect("utf-8 path"),
            "--project-root",
            root.to_str().expect("utf-8 path"),
            "--output",
            output.to_str().expect("utf-8 path"),
            "--binary-digest",
            &binary_digest,
        ])
        .assert()
        .success();

    let bytes = std::fs::read(&output).expect("read the saved artifact");
    (dir, bytes)
}

/// Every entry under `root`, as `(relative path, bytes)`, sorted — so two
/// independently produced layouts can be compared structurally.
fn layout_entries(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {dir:?}: {e}"))
            .map(|e| e.expect("read a dir entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("entry is under the layout root")
                    .to_string_lossy()
                    .into_owned();
                out.push((rel, std::fs::read(&path).expect("read a layout file")));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// The rendered report begins at the first `Package` section — everything
/// before it is the verb's own one-line banner (`Loaded …` / `Pulled …`), which
/// legitimately differs between the two verbs.
fn report_body(stdout: &str) -> &str {
    let marker = "\nPackage\n";
    let start = stdout
        .find(marker)
        .unwrap_or_else(|| panic!("no rendered report found in:\n{stdout}"));
    &stdout[start..]
}

/// D-16: `pull` and `load` produce the same LAYOUT and the same REPORT for the
/// same bytes.
///
/// The report half is the one that matters. `render.rs` is compiled TWICE — into
/// the bin, which is how `load` reaches it, and into the lib as
/// `package_render`, which is how the pipeline reaches it — so "one renderer" is
/// a claim about one SOURCE, and only a byte-comparison of the two outputs turns
/// it into a measured fact. A layout-only comparison would leave the two
/// compilations free to drift into two different reports.
///
/// Only the destination path is normalized, because the two verbs necessarily
/// install to different directories.
#[tokio::test]
async fn pull_and_load_agree_on_both_the_layout_and_the_report() {
    let (_project, tar_bytes) = saved_london_tube_tar();

    let scratch = tempfile::tempdir().expect("create a temp dir");
    let tar_path = scratch.path().join("london-tube.tar");
    std::fs::write(&tar_path, &tar_bytes).expect("write the tar for `load`");

    // --- the `load` side: the real binary, the BIN copy of the renderer ---
    let load_dest = scratch.path().join("loaded-layout");
    let load_out = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "package",
            "load",
            tar_path.to_str().expect("utf-8 path"),
            "--output",
            load_dest.to_str().expect("utf-8 path"),
        ])
        .assert()
        .success();
    let load_stdout = String::from_utf8(load_out.get_output().stdout.clone())
        .expect("`load` output must be utf-8");

    // --- the `pull` side: the real pipeline, the LIB copy of the renderer ---
    let pull_dest = scratch.path().join("pulled-layout");
    let declared = declared_digest_for(&tar_bytes);
    let double = SeamDouble::serving(tar_bytes.clone(), declared);
    let outcome = pull_package(&double, "london-tube@1.0.0", &pull_dest, false)
        .await
        .expect("a valid package must pull cleanly");

    assert_eq!(double.calls(), 1, "exactly one round trip");
    assert!(
        pull_dest.exists(),
        "a successful pull must install a layout"
    );
    assert!(
        outcome.subject_mismatch.is_none(),
        "no attestation was packed"
    );

    // Layouts: identical entry sets AND identical bytes.
    assert_eq!(
        layout_entries(&load_dest),
        layout_entries(&pull_dest),
        "`pull` and `load` must materialize byte-identical layouts"
    );

    // Reports: byte-identical after normalizing ONLY the destination path.
    let load_report =
        report_body(&load_stdout).replace(load_dest.to_str().expect("utf-8 path"), "<DEST>");
    let pull_report =
        report_body(&outcome.report).replace(pull_dest.to_str().expect("utf-8 path"), "<DEST>");
    assert_eq!(
        load_report, pull_report,
        "`pull` and `load` must render byte-identical reports — one renderer, two \
         compilations, and this comparison is what makes that a measured fact"
    );
    assert!(
        load_report.contains("london-tube"),
        "the comparison must be over a NON-EMPTY report: {load_report}"
    );
}
