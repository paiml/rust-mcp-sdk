//! SEP-2640 `skills/list` ROUTING and the guarantees the route must keep true
//! (Phase 125, plan 01).
//!
//! # Two halves, and they are not the same kind of test
//!
//! **Half one — the LIVE WIRE.** `skills/list` is routed through the
//! crate-private `InternalClientRequest` classifier that Phase 112 built for
//! `server/discover`, so nothing about the answer can be trusted from source
//! alone. These tests drive REAL bytes over a loopback socket through the real
//! `StreamableHttpServer` and assert the shape of the JSON that comes back —
//! including the digest, which is verified ON THE WIRE and not on an in-process
//! struct.
//!
//! **Half two — the LOST GUARDS.** Because the design adds NOTHING to the public
//! `ClientRequest` enum, none of that enum's compile-time protections apply here.
//! The source scan, the runtime wire proof, the era-gate control, the
//! name-bearing assertion and the stdio-reach probe replace them, and each one
//! catches a DIFFERENT mistake.
//!
//! # The properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `skills_list_returns_a_conforming_entry_on_v2` | a live v2 POST answers with a conforming entry: verbatim frontmatter, `sha256:`+64-hex digest, byte-accurate size, `resultType: "complete"`, no `nextCursor` |
//! | 2 | `skills_list_answers_on_v1_without_the_caching_attributes` | the D-07 era split, in BOTH directions: v2 carries `ttlMs`+`cacheScope`, v1 carries NEITHER |
//! | 3 | `empty_registry_answers_skills_list_with_an_empty_array` | a declared-but-empty catalog ANSWERS the method rather than denying it |
//! | 4 | `client_request_has_no_skills_variants` | the SEMVER mistake, as a source scan — there is no `cargo semver-checks` in `Makefile` or `.github/workflows/` (grep: zero hits), so this test IS the enforcement |
//! | 5 | `skills_methods_do_not_parse_as_public_client_requests` | the same property at RUNTIME, with a `resources/list` control that MUST parse |
//! | 6 | `skills_list_has_no_era_gate_and_server_discover_still_does` | the no-era-gate property, with the `server/discover` `-32601` negative control that stops it passing vacuously |
//! | 7 | `neither_skills_method_is_routing_name_bearing` | the omission from the `Mcp-Name` table is a DECISION, resolved through the production seam rather than a restated method list |
//! | 8 | `stdio_ingress_rejects_a_skills_list_frame` | the D-01 reach: over stdio the frame fails at `parse_message`, with a `resources/list` control that MUST parse |
//!
//! Plan 125-02 adds the `skills/get` half and the `ServerCore` boundary:
//!
//! | # | test | property |
//! |---|------|----------|
//! | 9 | `skills_get_returns_the_single_conforming_entry_on_v2` | a live `skills/get` on a registered SKILL.md URI answers with ONE entry identical in shape to a `skills/list` entry, `resultType: "complete"`, no cursor |
//! | 10 | `skills_get_unknown_uri_returns_invalid_params` | D-06: an unserved URI is `-32602`, asserted as the exact numeric code |
//! | 11 | `skills_get_reference_uri_returns_invalid_params` | the draft says `uri` MUST name a SKILL.md; a registered skill's REFERENCE file is `-32602` even though `resources/read` serves it |
//! | 12 | `skills_get_malformed_params_return_invalid_params` | absent params, non-object params and a non-string `uri` all reach `-32602` from the SERVED branch, and none panics |
//! | 13 | `skills_get_traversal_shaped_uri_returns_invalid_params` | T-125-06: `..` segments and appended path suffixes are `-32602` because the lookup is an exact map hit, never a path join |
//! | 14 | `resources_read_unknown_uri_still_returns_method_not_found` | the DIVERGENCE control, MEASURED: `resources/read` keeps its pre-existing behaviour — handler-level `-32601`, re-wrapped to `-32603` on the wire — and in particular is not `-32602`, so the `skills/get` code reads as a decision rather than an inconsistency |
//! | 15 | `skills_get_result_carries_no_cache_attributes_on_either_era` | R-17: `ttlMs` and `cacheScope` are ABSENT from a `skills/get` result on v1 AND on v2 — asserted on the wire, not inferred from the source not naming `Cacheable::Yes` |
//! | 16 | `skills_get_auth_refusal_precedes_the_params_error` | R-16: against an auth-carrying server an UNCREDENTIALED malformed-params `skills/get` gets the authentication refusal; the SAME request WITH credentials gets `-32602`. Both halves are required |
//! | 17 | `server_core_builder_still_serves_skills_as_resources` | the reachable half of the retired parity claim: a `ServerCoreBuilder`-built core answers typed `resources/list` / `resources/read` for its skills |
//! | 18 | `server_core_declares_no_skills_field_or_skills_method` | the dead-code guard: `ServerCore` must gain neither a `skill_entries` field nor a skills method, because neither could ever be reached |
//!
//! Tests 4 and 5 are deliberately NOT redundant, and the difference is
//! measurable: typing `SkillsList` into the `pub enum ClientRequest` block fails
//! 4 and not 5 until something also makes serde accept it, while a
//! `#[serde(rename = "skills/list")]` smuggled onto an EXISTING variant fails 5
//! and not 4 (the block gains no new variant name). Tests 3 and 6 are likewise
//! distinct: 3 is about an empty CATALOG, 6 about an absent ERA GATE.
//!
//! # Why the method literals are restated here
//!
//! A Rust integration test is its own crate, so it cannot reach the crate's
//! single-sourced `pub(crate) SKILLS_LIST_METHOD` / `SKILLS_GET_METHOD`. The
//! restatement below carries the same justification
//! `tests/v2_tasks_update_routing.rs` uses for `tasks/update`.

#![cfg(all(
    feature = "skills",
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    header, post, post_raw, spawn_default_config, teardown, v1_body, v2_body, v2_headers_for,
    BearerSubjects, Resp, V1, V2,
};
use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::Server;
use serde_json::{json, Value};
use std::net::SocketAddr;

/// The wire method under test. Spelled once here; the crate's own single-sourced
/// constant is `pub(crate)` and therefore unreachable from an integration crate.
const SKILLS_LIST: &str = "skills/list";

/// The sibling SEP-2640 method. It has no route until plan 125-02, but two of the
/// properties below are about the method STRING and hold before any route exists:
/// its absence from `ClientRequest` (tests 4 and 5) and its absence from the
/// routing-name table (test 7).
const SKILLS_GET: &str = "skills/get";

/// The `Mcp-Session-Id` request header, for the v1 legs: a v1 caller against a
/// default-config harness must complete a real handshake or it gets `-32600`,
/// which looks like a skills bug and is not one.
const SESSION_HEADER: &str = "Mcp-Session-Id";

/// The canonical fixture skill's SKILL.md body.
///
/// Frontmatter discipline (phase-wide rule): `name` equals the FINAL `/` segment
/// of the resolved URI. Plan 125-03 makes a mismatch a build-time rejection, so a
/// fixture that violated it here would surface in a later wave as a confusing
/// failure inside a plan that did not author it.
///
/// `license` is the non-required third field: it is what proves the frontmatter
/// is emitted VERBATIM rather than curated down to the two fields SEP-2640
/// requires.
const REFUNDS_BODY: &str = "---\nname: refunds\ndescription: Process customer refund requests\nlicense: Apache-2.0\n---\n# Refunds\n\nFollow company policy.\n";

/// Build the fixture server for this suite.
///
/// # The explicit accept list is LOAD-BEARING — do not "simplify" it away
///
/// `common::v2::spawn_default_config` takes an already-built `Server` and only
/// selects the HTTP config; it does not touch the server's accepted protocol
/// versions. A plain `Server::builder()` accepts the DEFAULT version set, so a
/// 2026-07-28-framed POST would be refused by the version gate BEFORE skills
/// dispatch is ever reached — and the test would fail on a version error while
/// appearing to indict the routing work. The chain below is the same explicit
/// `[V1, V2]` accept list `common::v2::build_v2_server_with` uses, with both
/// values read from the harness rather than retyped.
fn skills_fixture_server(registry: Skills) -> Server {
    Server::builder()
        .name("skills-routing-fixture")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .skills(registry)
        .build()
        .expect("fixture server builds")
}

/// A registry holding the one canonical fixture skill.
fn one_skill_registry() -> Skills {
    Skills::new().add(Skill::new("refunds", REFUNDS_BODY))
}

/// The canonical fixture skill's SKILL.md URI — the ONE `skills/get` key that
/// resolves.
const REFUNDS_SKILL_MD: &str = "skill://refunds/SKILL.md";

/// A reference file registered on the fixture skill.
///
/// SEP-2640 §9: a reference is READABLE via `resources/read` but is never a
/// `skills/get` key — the draft says the `uri` MUST name a skill's SKILL.md. That
/// asymmetry is what test 11 pins.
const REFUNDS_REFERENCE_PATH: &str = "examples/email.md";
const REFUNDS_REFERENCE_URI: &str = "skill://refunds/examples/email.md";
const REFUNDS_REFERENCE_BODY: &str = "Dear customer,\n";

/// The one-skill registry plus a reference file, for the tests that need a URI
/// which the RESOURCE surface serves and the SKILLS surface must not.
fn skill_with_reference_registry() -> Skills {
    Skills::new().add(
        Skill::new("refunds", REFUNDS_BODY).with_reference(SkillReference::new(
            REFUNDS_REFERENCE_PATH,
            "text/markdown",
            REFUNDS_REFERENCE_BODY,
        )),
    )
}

/// POST a v2-framed `skills/get` for `uri` and return the raw response view.
async fn post_v2_skills_get(addr: SocketAddr, id: i64, uri: &str) -> Resp {
    let params = json!({ "uri": uri });
    post(
        addr,
        &v2_headers_for(SKILLS_GET, &params),
        &v2_body(SKILLS_GET, json!(id), params),
    )
    .await
}

/// The numeric JSON-RPC error code carried by a response, or a panic naming the
/// body.
///
/// Every `-32602` assertion below goes through this rather than merely checking
/// that `error` is present: "an error was returned" would also hold for the
/// `-32601` this plan must NOT emit, which is the specific confusion D-06 exists
/// to prevent.
fn error_code_of(response: &Resp) -> i64 {
    response.body["error"]["code"].as_i64().unwrap_or_else(|| {
        panic!(
            "expected a JSON-RPC error with a numeric code, got: {}",
            response.raw
        )
    })
}

/// `-32602`, read from the crate's own constant rather than retyped.
fn invalid_params() -> i64 {
    i64::from(pmcp::types::protocol::error_codes::INVALID_PARAMS)
}

/// POST a v2-framed `skills/list` and return the raw response view.
async fn post_v2_skills_list(addr: SocketAddr, id: i64) -> Resp {
    let params = json!({});
    post(
        addr,
        &v2_headers_for(SKILLS_LIST, &params),
        &v2_body(SKILLS_LIST, json!(id), params),
    )
    .await
}

/// Complete a v1 handshake and return the minted session id.
async fn v1_session(addr: SocketAddr) -> String {
    // A `--no-default-features --features full-v2` build mints nothing and
    // validates nothing, so a placeholder keeps every v1 leg RUNNING there.
    if !cfg!(feature = "v1-compat") {
        return "no-session-on-a-severed-build".to_string();
    }
    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(1),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "skills-routing-v1-client", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    init.mcp_session_id
        .unwrap_or_else(|| panic!("a v1 initialize mints a session; body was {}", init.raw))
}

fn result_of(response: &Resp) -> &Value {
    let result = &response.body["result"];
    assert!(
        !result.is_null(),
        "expected a JSON-RPC result, got: {}",
        response.raw
    );
    result
}

// ===========================================================================
// 1 — the live wire.
// ===========================================================================

/// A live `StreamableHttpServer` carrying ONE frontmatter-bearing skill answers a
/// v2 `skills/list` POST with a single conforming entry.
///
/// Every assertion below reads the WIRE BODY, not an in-process struct. That is
/// the point of the tracer: the entry could be perfect in memory and still never
/// reach a caller if any of the five transport sites were missed.
#[tokio::test]
async fn skills_list_returns_a_conforming_entry_on_v2() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;

    let response = post_v2_skills_list(addr, 4).await;
    assert_eq!(
        response.status, 200,
        "a served method answers at HTTP 200: {}",
        response.raw
    );
    assert_eq!(
        response.body["id"],
        json!(4),
        "the ORIGINAL id is preserved"
    );

    let result = result_of(&response);
    assert_eq!(
        result["resultType"], "complete",
        "SEP-2640's own skills/list example carries resultType complete: {}",
        response.raw
    );

    let skills = result["skills"]
        .as_array()
        .unwrap_or_else(|| panic!("result.skills is an array; got {}", response.raw));
    assert_eq!(skills.len(), 1, "one registered skill, one entry");

    let entry = &skills[0];
    assert_eq!(entry["uri"], "skill://refunds/SKILL.md");

    // VERBATIM: all three authored fields survive, including the non-required
    // one. A curated `{name, description}` subset is a guaranteed host-side
    // rejection, because the host re-reads the SKILL.md and compares field by
    // field.
    assert_eq!(entry["frontmatter"]["name"], "refunds");
    assert_eq!(
        entry["frontmatter"]["description"],
        "Process customer refund requests"
    );
    assert_eq!(entry["frontmatter"]["license"], "Apache-2.0");

    let manifest = entry["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("entry.resources is an array; got {}", response.raw));
    assert_eq!(
        manifest[0]["uri"], "skill://refunds/SKILL.md",
        "SKILL.md is the first manifest row"
    );

    // The digest, verified on the wire: `sha256:` + 64 LOWERCASE hex characters.
    let digest = manifest[0]["digest"].as_str().unwrap_or_else(|| {
        panic!(
            "a manifest row carries a string digest; got {}",
            response.raw
        )
    });
    let hex = digest
        .strip_prefix("sha256:")
        .unwrap_or_else(|| panic!("digest must carry the `sha256:` prefix, got {digest}"));
    assert_eq!(hex.len(), 64, "64 hex characters, got {digest}");
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "digest hex must be LOWERCASE, got {digest}"
    );

    assert_eq!(
        manifest[0]["size"],
        json!(REFUNDS_BODY.len()),
        "size is the byte length of the SERVED SKILL.md"
    );

    // D-11: a single complete page. An absent cursor means the listing is
    // complete, so the key must not appear at all.
    assert!(
        result.get("nextCursor").is_none(),
        "skills/list returns a single page and emits NO nextCursor: {}",
        response.raw
    );

    // D-07 / R-03: the caching attributes. These two keys have exactly ONE writer
    // in the tree — the shared v2 projection, which ensures both under
    // `Cacheable::Yes` with `Some(Era::V2)`. Their presence here is therefore the
    // only OBSERVABLE proof that the `Cacheable::Yes` named at the
    // `build_skills_list_response` call site actually reached the envelope;
    // without this assertion the claim would be about source text, not behaviour.
    assert!(
        result.get("ttlMs").is_some(),
        "a v2 skills/list result carries ttlMs: {}",
        response.raw
    );
    assert!(
        result.get("cacheScope").is_some(),
        "a v2 skills/list result carries cacheScope: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 2 — the v1 twin: answered, but WITHOUT the caching attributes.
// ===========================================================================

/// `skills/list` answers on a v1 connection too — it carries no era gate — and
/// its v1 result carries NEITHER caching attribute.
///
/// The negative half is what makes the v2 assertion meaningful: without it, a
/// server that emitted `ttlMs`/`cacheScope` unconditionally would pass test 1 and
/// nothing would notice.
#[tokio::test]
async fn skills_list_answers_on_v1_without_the_caching_attributes() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;
    let session = v1_session(addr).await;

    let response = post(
        addr,
        &[header(SESSION_HEADER, &session)],
        &v1_body(SKILLS_LIST, json!(11), json!({})),
    )
    .await;

    assert_eq!(
        response.status, 200,
        "answered, not refused: {}",
        response.raw
    );
    let result = result_of(&response);
    assert_eq!(
        result["skills"]
            .as_array()
            .unwrap_or_else(|| panic!("result.skills is an array; got {}", response.raw))
            .len(),
        1,
        "the same catalog is served on 2025-11-25"
    );

    assert!(
        result.get("ttlMs").is_none(),
        "the v1 result carries NO ttlMs — the attributes are 2026-07-28+: {}",
        response.raw
    );
    assert!(
        result.get("cacheScope").is_none(),
        "the v1 result carries NO cacheScope: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 3 — an empty catalog ANSWERS the method.
// ===========================================================================

/// A server built with an EMPTY skills registry answers `skills/list` with an
/// empty array, not `-32601`.
///
/// The capability is declared at build time regardless of catalog size, so an
/// empty catalog that denied the method would make that declaration a lie.
#[tokio::test]
async fn empty_registry_answers_skills_list_with_an_empty_array() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(Skills::new())).await;

    let response = post_v2_skills_list(addr, 5).await;
    assert_eq!(
        response.status, 200,
        "answered, not refused: {}",
        response.raw
    );

    let result = result_of(&response);
    assert_eq!(
        result["skills"],
        json!([]),
        "an empty catalog is an empty array: {}",
        response.raw
    );
    assert_eq!(result["resultType"], "complete");

    teardown(handle, ()).await;
}

// ===========================================================================
// 4 — the SEMVER mistake, as a source scan.
// ===========================================================================

/// `pub enum ClientRequest` must never gain a `SkillsList` / `SkillsGet` variant.
///
/// `ClientRequest` carries `#[serde(tag = "method", content = "params")]` with
/// **no `#[non_exhaustive]`**, so `enum_variant_added` is a semver-MAJOR break and
/// every downstream exhaustive `match` stops compiling; adding
/// `#[non_exhaustive]` instead is itself a source break. Both SEP-2640 methods
/// are carried by the crate-private `InternalClientRequest` and classified by
/// `classify_internal_method`.
///
/// **This test IS the enforcement.** There is no `cargo semver-checks` in
/// `Makefile` or `.github/workflows/` — grep returns zero hits — so nothing else
/// in the repository catches the regression. It reads the SOURCE from disk rather
/// than a compiled constant, exactly like
/// `client_request_has_no_tasks_update_variant`, and is scoped to the enum BLOCK
/// rather than the file: `src/types/protocol/mod.rs` legitimately names
/// `SkillsList` on the crate-private enum that carries the method instead.
#[test]
fn client_request_has_no_skills_variants() {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/types/protocol/mod.rs");
    let source = std::fs::read_to_string(&path).expect("protocol/mod.rs is readable");

    let start = source
        .find("\npub enum ClientRequest {")
        .expect("the `pub enum ClientRequest` declaration still exists");
    let rest = &source[start + 1..];
    let end = rest
        .find("\n}\n")
        .expect("the ClientRequest block is brace-terminated at column 0");
    let block = &rest[..end];

    // Anti-vacuity: an empty or truncated block would pass both assertions below
    // while measuring nothing.
    assert!(
        block.contains("ListResources"),
        "the extracted block is not the real ClientRequest enum; scan is vacuous.\n\
         Block was:\n{block}"
    );

    for variant in ["SkillsList", "SkillsGet"] {
        assert!(
            !block.contains(variant),
            "`pub enum ClientRequest` gained a {variant} variant. That is \
             enum_variant_added on a PUBLIC EXHAUSTIVE enum — a semver-MAJOR break, and every \
             downstream exhaustive match stops compiling. Adding #[non_exhaustive] instead is \
             ALSO a source break. The SEP-2640 methods are carried by the crate-private \
             InternalClientRequest and classified by classify_internal_method; route them \
             there.\n\nBlock was:\n{block}"
        );
    }
}

// ===========================================================================
// 5 — the same property at RUNTIME, with a control that MUST parse.
// ===========================================================================

/// Neither SEP-2640 method deserializes into the public `ClientRequest` enum,
/// and `resources/list` still does.
///
/// Spike 008's technique. The passing CONTROL is load-bearing: without it the
/// assertion would also hold against a malformed fixture, a broken `from_value`
/// call, or a `ClientRequest` that stopped parsing anything at all — none of
/// which says a word about routing.
///
/// Not redundant with the source scan above. That one fails the moment the
/// variant NAME is typed into the enum block, before anything routes it; this one
/// fails only once serde would actually accept the method — so a
/// `#[serde(rename = "skills/list")]` smuggled onto an existing variant would
/// pass the scan and fail here.
#[test]
fn skills_methods_do_not_parse_as_public_client_requests() {
    for method in [SKILLS_LIST, SKILLS_GET] {
        let frame = json!({ "method": method, "params": {} });
        assert!(
            serde_json::from_value::<pmcp::types::ClientRequest>(frame).is_err(),
            "{method} must NOT deserialize into the public ClientRequest enum — \
             the 2.x exhaustive-enum promise depends on it having no variant"
        );
    }

    // The control. If this ever fails, the two assertions above prove nothing.
    let control = json!({ "method": "resources/list", "params": {} });
    assert!(
        serde_json::from_value::<pmcp::types::ClientRequest>(control).is_ok(),
        "resources/list MUST parse — without a passing control the Err assertions \
         above are unfalsifiable"
    );
}

// ===========================================================================
// 6 — the no-era-gate property, with the server/discover negative control.
// ===========================================================================

/// `skills/list` answers on a v1 connection; `server/discover` still does not.
///
/// `server/discover` is a 2026-07-28-only method and answers `-32601` on v1.
/// `skills/list` has no such gate in SEP-2640 — it rides the base Resources
/// primitive. The `server/discover` leg is the negative control: it is what stops
/// this test from passing vacuously against a server that answers EVERYTHING, and
/// both legs run against the SAME server over the SAME session so the difference
/// is attributable to the method alone.
#[tokio::test]
async fn skills_list_has_no_era_gate_and_server_discover_still_does() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;
    let session = v1_session(addr).await;

    let served = post(
        addr,
        &[header(SESSION_HEADER, &session)],
        &v1_body(SKILLS_LIST, json!(21), json!({})),
    )
    .await;
    assert!(
        served.body.get("error").is_none(),
        "skills/list carries NO era gate and must be served on v1: {}",
        served.raw
    );
    assert!(
        !result_of(&served)["skills"].is_null(),
        "the v1 answer is a real result: {}",
        served.raw
    );

    let refused = post(
        addr,
        &[header(SESSION_HEADER, &session)],
        &v1_body("server/discover", json!(22), json!({})),
    )
    .await;
    assert_eq!(
        refused.body["error"]["code"],
        json!(pmcp::types::protocol::error_codes::METHOD_NOT_FOUND),
        "the control: server/discover IS era-gated and answers -32601 on v1: {}",
        refused.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 7 — the not-name-bearing property, as a tested NON-change.
// ===========================================================================

/// Neither SEP-2640 method carries a routing name, so the v2 `Mcp-Name` header is
/// discarded for both — and a v2 `skills/list` POST built with an empty `Mcp-Name`
/// is ACCEPTED rather than rejected with `-32020`.
///
/// This is pinned as a DECISION, not left as an absence. `skills/get` carries a
/// `uri` param, structurally identical to `resources/read`, which IS in the
/// name-bearing table — so the omission reads like an oversight. Adding either
/// method to that table is a deliberate deferral: it would require editing the
/// method table in `contracts/mcp-protocol-sdk-v1.yaml` and the literal-contract
/// test in `src/server/streamable_http_server.rs` alongside `src/types/mrtr.rs`.
///
/// The lookup resolves through `pmcp::testing::routing_name_key`, the PRODUCTION
/// combined table both the `Mcp-Name` emitter and the server's cross-check read.
/// A test that restated a method list here would be validating itself.
#[tokio::test]
async fn neither_skills_method_is_routing_name_bearing() {
    for method in [SKILLS_LIST, SKILLS_GET] {
        assert!(
            pmcp::testing::routing_name_key(method).is_none(),
            "{method} must carry no routing name — see this test's rustdoc for the \
             deferral this pins"
        );
    }

    // And the wire consequence: `v2_headers_for` derives `Mcp-Name` through that
    // same production table, so it emits the EMPTY string here. An accepted
    // request is the proof the server discards it rather than cross-checking it.
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;
    let response = post_v2_skills_list(addr, 31).await;
    assert_eq!(
        response.status, 200,
        "an empty Mcp-Name on a name-less method is accepted, not -32020: {}",
        response.raw
    );
    assert!(
        response.body.get("error").is_none(),
        "no header-mismatch refusal: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 8 — the D-01 stdio reach, measured rather than asserted in prose.
// ===========================================================================

/// Over stdio a `skills/list` frame fails at `parse_message`; `resources/list`
/// parses.
///
/// # The full measured chain, of which this asserts the FIRST link
///
/// `classify_http_ingress` is the ONLY production consumer of
/// `IngressRequest::Internal` (that sentence is verbatim in the seam's own
/// rustdoc at `src/shared/protocol_helpers.rs`), so every other transport reaches
/// requests through the PUBLIC `parse_request`, which maps `Internal` to
/// method-not-found. On stdio that `Err` becomes a
/// `TransportError::InvalidMessage`, and `run_transport_actor`'s receive arm
/// (`src/server/mod.rs`) BREAKS the loop on any receive error — so over stdio a
/// `skills/list` does not merely answer `-32601`, it tears the connection down.
///
/// The teardown half is NOT asserted here: reproducing it means driving a real
/// stdio actor with a live process on both ends. What is asserted is the link the
/// teardown follows from, and the deferral of stdio reach itself is a recorded
/// phase deferral with an owner — never a code TODO.
///
/// The `resources/list` control is load-bearing: without it this assertion would
/// pass equally against a parser that rejected EVERY frame, which would say
/// nothing about skills.
#[test]
fn stdio_ingress_rejects_a_skills_list_frame() {
    let skills_frame =
        format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{SKILLS_LIST}","params":{{}}}}"#);
    let parsed = pmcp::shared::transport::parse_message(skills_frame.as_bytes());
    assert!(
        parsed.is_err(),
        "a skills/list frame must FAIL the stdio ingress parse today (D-01): \
         the InternalClientRequest route reaches streamable HTTP only"
    );

    // The control. Without it, a parser that rejected everything would pass.
    let control = br#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;
    assert!(
        pmcp::shared::transport::parse_message(control).is_ok(),
        "resources/list MUST parse — without a passing control the assertion above \
         is unfalsifiable"
    );
}

// ===========================================================================
// 9 — `skills/get`: the hit.
// ===========================================================================

/// A live `skills/get` on a registered SKILL.md URI answers with ONE entry whose
/// shape is IDENTICAL to a `skills/list` entry.
///
/// The shape identity is the property, not an incidental convenience: SEP-2640
/// defines `skills/get` as returning the same entry object a listing carries, and
/// a host that read one and then the other would reject a skill on any
/// discrepancy. The assertions below therefore restate the `skills/list` entry
/// assertions verbatim rather than checking a looser "some object came back".
#[tokio::test]
async fn skills_get_returns_the_single_conforming_entry_on_v2() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;

    let response = post_v2_skills_get(addr, 41, REFUNDS_SKILL_MD).await;
    assert_eq!(
        response.status, 200,
        "a served method answers at HTTP 200: {}",
        response.raw
    );
    assert_eq!(response.body["id"], json!(41), "the ORIGINAL id is preserved");

    let result = result_of(&response);
    assert_eq!(
        result["resultType"], "complete",
        "D-07: a skills/get result is complete: {}",
        response.raw
    );

    let entry = &result["skill"];
    assert!(
        !entry.is_null(),
        "the result carries a single `skill` object, not an array: {}",
        response.raw
    );
    assert_eq!(entry["uri"], REFUNDS_SKILL_MD);
    assert_eq!(entry["frontmatter"]["name"], "refunds");
    assert_eq!(
        entry["frontmatter"]["description"],
        "Process customer refund requests"
    );
    assert_eq!(
        entry["frontmatter"]["license"], "Apache-2.0",
        "VERBATIM frontmatter — the non-required field survives here too: {}",
        response.raw
    );

    let manifest = entry["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("entry.resources is an array; got {}", response.raw));
    assert_eq!(manifest[0]["uri"], REFUNDS_SKILL_MD);
    assert!(
        manifest[0]["digest"]
            .as_str()
            .is_some_and(|d| d.starts_with("sha256:") && d.len() == "sha256:".len() + 64),
        "the manifest digest survives the get projection unchanged: {}",
        response.raw
    );
    assert_eq!(manifest[0]["size"], json!(REFUNDS_BODY.len()));

    assert!(
        result.get("nextCursor").is_none(),
        "a single-entry get carries no pagination cursor: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 10 — `skills/get`: the unknown URI is -32602 (D-06).
// ===========================================================================

/// `skills/get` on a URI the server does not serve answers `-32602`, the code the
/// CURRENT SEP-2640 draft specifies.
///
/// The numeric code is asserted, not merely the presence of an error: the shipped
/// `SkillsHandler::read` answers `-32601` for the analogous `resources/read` miss,
/// so "an error came back" would pass against exactly the code this test exists to
/// rule out. Test 14 pins the other side of that divergence.
#[tokio::test]
async fn skills_get_unknown_uri_returns_invalid_params() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;

    let response = post_v2_skills_get(addr, 42, "skill://nope/SKILL.md").await;
    assert_eq!(
        error_code_of(&response),
        invalid_params(),
        "D-06: an unresolvable skills/get uri is -32602: {}",
        response.raw
    );
    assert_ne!(
        error_code_of(&response),
        i64::from(pmcp::types::protocol::error_codes::METHOD_NOT_FOUND),
        "stated separately because -32601 here is the SPECIFIC regression this test \
         exists for — it is what resources/read returns and what must NOT be copied: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 11 — a REFERENCE file is readable but is not a skills/get key.
// ===========================================================================

/// `skills/get` on a registered skill's REFERENCE file answers `-32602`, while the
/// same URI is READABLE through `resources/read`.
///
/// The `resources/read` half is the control, and it is what makes the assertion
/// mean something: without it the `-32602` would also hold on a server that had
/// simply failed to register the reference at all, which says nothing about the
/// draft's "the `uri` MUST be a skill's SKILL.md" rule.
#[tokio::test]
async fn skills_get_reference_uri_returns_invalid_params() {
    let (addr, handle) =
        spawn_default_config(skills_fixture_server(skill_with_reference_registry())).await;

    let refused = post_v2_skills_get(addr, 43, REFUNDS_REFERENCE_URI).await;
    assert_eq!(
        error_code_of(&refused),
        invalid_params(),
        "a reference file is not a skills/get key: {}",
        refused.raw
    );

    // The control: the very same URI IS served by the resource surface.
    let params = json!({ "uri": REFUNDS_REFERENCE_URI });
    let read = post(
        addr,
        &v2_headers_for("resources/read", &params),
        &v2_body("resources/read", json!(44), params),
    )
    .await;
    assert!(
        read.body.get("error").is_none(),
        "the control: resources/read SERVES the reference URI, so the -32602 above is \
         about the skills surface and not about registration: {}",
        read.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 12 — malformed params reach -32602 from the SERVED branch, never a panic.
// ===========================================================================

/// Absent params, non-object params and a non-string `uri` all answer `-32602`.
///
/// Each leg reaches the served branch: the classifier is body-blind by design, so
/// a malformed body cannot become a parse error ahead of the pipeline. A panic in
/// any leg would surface as a dropped connection rather than a JSON-RPC error, so
/// "the server answered at all" is itself part of what is asserted.
#[tokio::test]
async fn skills_get_malformed_params_return_invalid_params() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;
    let session = v1_session(addr).await;

    // (a) NO `params` key at all — the `Value::Null` path.
    let absent = post_raw(
        addr,
        &[header(SESSION_HEADER, &session)],
        &format!(r#"{{"jsonrpc":"2.0","id":45,"method":"{SKILLS_GET}"}}"#),
    )
    .await;
    assert_eq!(
        error_code_of(&absent),
        invalid_params(),
        "absent params answer -32602 rather than panicking: {}",
        absent.raw
    );

    // (b) `params` present but NOT an object.
    let non_object = post_raw(
        addr,
        &[header(SESSION_HEADER, &session)],
        &format!(r#"{{"jsonrpc":"2.0","id":46,"method":"{SKILLS_GET}","params":"nonsense"}}"#),
    )
    .await;
    assert_eq!(
        error_code_of(&non_object),
        invalid_params(),
        "a non-object params body answers -32602: {}",
        non_object.raw
    );

    // (c) `params.uri` present but NOT a string.
    let wrong_type = {
        let params = json!({ "uri": 17 });
        post(
            addr,
            &v2_headers_for(SKILLS_GET, &params),
            &v2_body(SKILLS_GET, json!(47), params),
        )
        .await
    };
    assert_eq!(
        error_code_of(&wrong_type),
        invalid_params(),
        "a non-string uri answers -32602: {}",
        wrong_type.raw
    );

    // (d) `params` an object with NO `uri` key.
    let missing_key = {
        let params = json!({ "cursor": "x" });
        post(
            addr,
            &v2_headers_for(SKILLS_GET, &params),
            &v2_body(SKILLS_GET, json!(48), params),
        )
        .await
    };
    assert_eq!(
        error_code_of(&missing_key),
        invalid_params(),
        "a missing uri key answers -32602: {}",
        missing_key.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 13 — traversal-shaped URIs are -32602 because the lookup is an exact map hit.
// ===========================================================================

/// A `skills/get` `uri` carrying `..` segments, or a suffix appended to a
/// registered URI, answers `-32602` (T-125-06).
///
/// The mitigation is structural rather than a filter: the lookup is an exact-match
/// hit in the entry map keyed by SKILL.md URI, so no caller string is ever joined,
/// normalized or decoded into a path. This test is what proves the structure holds
/// on the wire — a future implementation that "helpfully" normalized the key would
/// pass every other test in this file and fail here.
///
/// The positive control at the end is load-bearing: it proves the server does
/// resolve the CANONICAL form, so the four refusals above are about the mangled
/// shapes rather than about a lookup that resolves nothing at all.
#[tokio::test]
async fn skills_get_traversal_shaped_uri_returns_invalid_params() {
    let (addr, handle) =
        spawn_default_config(skills_fixture_server(skill_with_reference_registry())).await;

    for (id, uri) in [
        (51_i64, "skill://refunds/../refunds/SKILL.md"),
        (52, "skill://refunds/SKILL.md/../../SKILL.md"),
        (53, "skill://refunds/SKILL.md.bak"),
        (54, "skill://refunds/SKILL.md/"),
    ] {
        let response = post_v2_skills_get(addr, id, uri).await;
        assert_eq!(
            error_code_of(&response),
            invalid_params(),
            "`{uri}` must not resolve — the lookup is an exact map hit, never a path \
             join (T-125-06): {}",
            response.raw
        );
    }

    // The positive control: the CANONICAL spelling of the same skill resolves.
    let ok = post_v2_skills_get(addr, 59, REFUNDS_SKILL_MD).await;
    assert!(
        ok.body.get("error").is_none(),
        "the control: the canonical URI resolves, so the refusals above are about the \
         mangled shapes and not about a lookup that finds nothing: {}",
        ok.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 14 — the DIVERGENCE control: resources/read keeps its -32601.
// ===========================================================================

/// `resources/read` on an unknown URI answers exactly what it answered before this
/// plan — and that is MEASURED here rather than assumed.
///
/// # The measurement, which corrects D-06's phrasing
///
/// D-06 records the divergence as "`SkillsHandler::read` returns `-32601`". That is
/// true at the HANDLER level and is pinned by
/// `resources_read_unknown_uri_method_not_found` (`tests/skills_integration.rs`),
/// which calls `ResourceHandler::read` directly and matches on
/// `Error::Protocol { code }`.
///
/// ON THE WIRE it is not what a caller sees. The handler's `Error::Protocol` is
/// re-wrapped by the dispatch tail, so a `resources/read` POST for an unserved URI
/// comes back as `-32603` (internal error) whose MESSAGE carries the original
/// `-32601`. Both facts are asserted below, because a reader who trusted the
/// summary phrasing alone would write a wrong test.
///
/// This plan changes neither. It does not copy the code into `skills/get` and it
/// does not fix the wrapping: changing a shipped error code is observable
/// behaviour with its own test, and D-06 scopes the whole divergence out. The
/// assertion lives HERE, beside the `skills/get` `-32602` tests, so a reader sees
/// the codes side by side and reads the difference as a decision rather than an
/// inconsistency to "clean up".
#[tokio::test]
async fn resources_read_unknown_uri_still_returns_method_not_found() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;

    let params = json!({ "uri": "skill://nope/SKILL.md" });
    let response = post(
        addr,
        &v2_headers_for("resources/read", &params),
        &v2_body("resources/read", json!(60), params),
    )
    .await;

    assert_eq!(
        error_code_of(&response),
        i64::from(pmcp::types::protocol::error_codes::INTERNAL_ERROR),
        "MEASURED: the handler's -32601 reaches the wire re-wrapped as -32603. This \
         plan changes neither the handler code nor the wrapping: {}",
        response.raw
    );
    assert!(
        response.body["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("-32601")),
        "the handler-level -32601 survives inside the message, which is the half \
         `resources_read_unknown_uri_method_not_found` pins directly: {}",
        response.raw
    );
    assert_ne!(
        error_code_of(&response),
        invalid_params(),
        "the contrast that matters: `resources/read` does NOT answer -32602, so the \
         `skills/get` -32602 above is a deliberate divergence and not a shared \
         convention: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 15 — the get result carries NEITHER caching key, on BOTH eras (R-17).
// ===========================================================================

/// A `skills/get` result carries neither `ttlMs` nor `cacheScope`, on a v2 framing
/// and on a v1 framing.
///
/// The draft leaves the `skills/get` caching question open (unlike `skills/list`,
/// which it explicitly gives the base list-caching attributes), so pmcp claims
/// nothing. Absence is asserted ON THE WIRE rather than inferred from the source
/// not naming `Cacheable::Yes`, and the v2 leg is the one that matters:
/// `project_caching_hints` is the single writer of both keys and is TOTAL — every
/// input either ensures both or removes both — so an absent pair under a v2 era is
/// a real measurement of the disposition passed at the projection call site.
///
/// The v2 leg pairs with `skills_list_returns_a_conforming_entry_on_v2`, which
/// asserts both keys PRESENT on the same server under the same framing. Together
/// they show the two methods are treated differently on purpose.
#[tokio::test]
async fn skills_get_result_carries_no_cache_attributes_on_either_era() {
    let (addr, handle) = spawn_default_config(skills_fixture_server(one_skill_registry())).await;

    let v2 = post_v2_skills_get(addr, 61, REFUNDS_SKILL_MD).await;
    let v2_result = result_of(&v2);
    assert!(
        v2_result.get("ttlMs").is_none(),
        "a v2 skills/get result carries NO ttlMs — the draft leaves get-caching open: {}",
        v2.raw
    );
    assert!(
        v2_result.get("cacheScope").is_none(),
        "a v2 skills/get result carries NO cacheScope: {}",
        v2.raw
    );

    let session = v1_session(addr).await;
    let v1 = post(
        addr,
        &[header(SESSION_HEADER, &session)],
        &v1_body(SKILLS_GET, json!(62), json!({ "uri": REFUNDS_SKILL_MD })),
    )
    .await;
    let v1_result = result_of(&v1);
    assert!(
        v1_result.get("ttlMs").is_none() && v1_result.get("cacheScope").is_none(),
        "a v1 skills/get result carries neither key: {}",
        v1.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 16 — the auth gate runs BEFORE the params error (R-16).
// ===========================================================================

/// Against a server carrying an auth provider, an UNCREDENTIALED `skills/get` with
/// deliberately malformed params receives the authentication refusal; the SAME
/// request WITH credentials receives `-32602`.
///
/// **Both halves are required and neither is decorative.** The first alone would
/// also pass against a server that refused everything, including well-formed
/// requests. The second alone would say nothing about ordering. Run together on
/// ONE server with ONE body, the only difference is the bearer, so the change in
/// answer is attributable to authentication alone.
///
/// This is the test that would catch a future refactor which started deserializing
/// `params` inside `classify_internal_method` or `classify_http_ingress`. The
/// classifier's raw-params discipline is the MECHANISM, but the mechanism is not
/// the proof — such a refactor would leak a params error to an unauthenticated
/// caller while every other test in this file still passed.
#[tokio::test]
async fn skills_get_auth_refusal_precedes_the_params_error() {
    let server = Server::builder()
        .name("skills-routing-auth-fixture")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .auth_provider(BearerSubjects)
        .skills(one_skill_registry())
        .build()
        .expect("auth fixture server builds");
    let (addr, handle) = spawn_default_config(server).await;

    // The ONE malformed body used by both legs: `uri` is a number.
    let params = json!({ "uri": 17 });
    let headers = v2_headers_for(SKILLS_GET, &params);
    let body = v2_body(SKILLS_GET, json!(70), params);

    let anonymous = post(addr, &headers, &body).await;
    assert_eq!(
        error_code_of(&anonymous),
        i64::from(pmcp::types::protocol::error_codes::AUTHENTICATION_REQUIRED),
        "the auth refusal precedes the params parse even though the classifier saw the \
         body: {}",
        anonymous.raw
    );
    assert_ne!(
        error_code_of(&anonymous),
        invalid_params(),
        "stated separately because -32602 here is the SPECIFIC regression this test \
         exists for: an unauthenticated caller must not get a free parse of its own \
         chosen body: {}",
        anonymous.raw
    );

    let mut credentialed = headers.clone();
    credentialed.push(header("Authorization", "Bearer alice"));
    let authenticated = post(addr, &credentialed, &body).await;
    assert_eq!(
        error_code_of(&authenticated),
        invalid_params(),
        "the SAME malformed body, once authenticated, reaches the served branch and \
         answers -32602 — without this half the assertion above would also hold on a \
         server that refused everything: {}",
        authenticated.raw
    );

    teardown(handle, ()).await;
}
