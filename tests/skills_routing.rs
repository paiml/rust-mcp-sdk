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
//! catches a DIFFERENT mistake. They are added by this plan's second task.
//!
//! # The properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `skills_list_returns_a_conforming_entry_on_v2` | a live v2 POST answers with a conforming entry: verbatim frontmatter, `sha256:`+64-hex digest, byte-accurate size, `resultType: "complete"`, no `nextCursor` |
//! | 2 | `skills_list_answers_on_v1_without_the_caching_attributes` | the D-07 era split, in BOTH directions: v2 carries `ttlMs`+`cacheScope`, v1 carries NEITHER |
//! | 3 | `empty_registry_answers_skills_list_with_an_empty_array` | a declared-but-empty catalog ANSWERS the method rather than denying it |
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
    header, post, spawn_default_config, teardown, v1_body, v2_body, v2_headers_for, Resp, V1, V2,
};
use pmcp::server::skills::{Skill, Skills};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::Server;
use serde_json::{json, Value};
use std::net::SocketAddr;

/// The wire method under test. Spelled once here; the crate's own single-sourced
/// constant is `pub(crate)` and therefore unreachable from an integration crate.
const SKILLS_LIST: &str = "skills/list";

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
