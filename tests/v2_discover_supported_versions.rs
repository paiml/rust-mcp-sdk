//! Phase 118.1-07 (G-7 / CONF-06): `server/discover` MUST advertise
//! `supportedVersions`, and that list MUST be the SAME accept list an
//! unsupported-version rejection reports in `error.data.supported`.
//!
//! # What the spec and the suite require
//!
//! `schema/vendored/core-2026-07-28/schema.ts:678-696` declares
//! `DiscoverResult extends CacheableResult` with a REQUIRED
//! `supportedVersions: string[]` — "MCP Protocol Versions this server supports.
//! The client should choose a version from this list for use in subsequent
//! requests." Before this file existed, `grep -rn supportedVersions src/`
//! returned ZERO hits.
//!
//! The official conformance suite (`@modelcontextprotocol/conformance`
//! 0.2.0-alpha.11, `dist/index.js` offset 370104) goes further than the schema:
//! its `ServerUnsupportedVersionError` check asserts that
//! `error.data.supported` is a NON-EMPTY array **every element of which appears
//! in the discover result's `supportedVersions`**, and that
//! `error.data.requested` echoes the version that was asked for. Two lists that
//! merely happen to agree today would satisfy the schema and still fail the
//! suite the moment one of them drifted.
//!
//! # Why the correlation test spawns exactly ONE server
//!
//! [`v2_discover_supported_versions_contains_every_rejection_supported_entry`]
//! issues BOTH probes — the discover request and the unsupported-version
//! rejection — against a single [`spawn_default_config`] call. That is the whole
//! point of the test. Two spawned servers would let two INDEPENDENT accept lists
//! both pass: each response would be internally consistent with its own source
//! and the drift this file exists to catch would be invisible. One server, two
//! probes, one accept list.
//!
//! # Why the unsupported probe sends `v999.0.0` in BOTH the header and `_meta`
//!
//! The v2 gate classifies a header/`_meta` protocol-version DISAGREEMENT as
//! `-32020` (`HEADER_MISMATCH`) and only an AGREED-but-unsupported version as
//! `-32022` (`UNSUPPORTED_PROTOCOL_VERSION`). `error.data.supported` rides the
//! second branch only, so the probe must make the two agree or it lands on the
//! mismatch branch and never produces the list under test.
//!
//! # Why `spawn_default_config`
//!
//! `StreamableHttpServerConfig::default()` keeps a LIVE `session_id_generator`,
//! so the PER-REQUEST era gate is genuinely exercised. `::stateless()` is a
//! BUILD-TIME config that clears the session machinery once at construction, so
//! a test spawned that way never reaches the per-request gate at all (RESEARCH
//! Pitfall 1). A dual-version production server is built with
//! `Default::default()`, so the stateful config is also the realistic case.
//!
//! Test reliability doctrine (carried from `tests/v2_stateless_http.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after the
//! round-trips).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, post, spawn_default_config, v2_body_claiming_version, v2_discover_body,
    v2_headers, v2_headers_claiming, V1, V2,
};
use pmcp::types::protocol::error_codes::UNSUPPORTED_PROTOCOL_VERSION;
use serde_json::{json, Value};

/// The version the rejection probe asks for. Deliberately unparseable as a date
/// so it can never collide with a real accept-list entry.
const UNSUPPORTED: &str = "v999.0.0";

/// The method every probe in this file targets.
const DISCOVER: &str = "server/discover";

/// A `server/discover` body whose reserved `_meta` claims `version` rather than
/// [`V2`], for the unsupported-version rejection probe.
///
/// [`v2_body`] hard-codes [`V2`] as the `_meta` protocol version, and this probe's
/// entire purpose is to claim a DIFFERENT one while keeping the other two reserved
/// keys well-formed, so the rejection is attributable to the version alone —
/// which is exactly what [`v2_body_claiming_version`] builds, through the shared
/// `RequestMeta` seam.
///
/// [`v2_body`]: common::v2::v2_body
fn discover_body_claiming_version(id: Value, version: &str) -> String {
    v2_body_claiming_version(DISCOVER, id, json!({}), version)
}

/// The headers for the rejection probe: the SAME unsupported version the body's
/// `_meta` claims, so the gate sees AGREEMENT and reaches the accept-list check.
fn unsupported_headers(version: &str) -> Vec<(String, String)> {
    v2_headers_claiming(DISCOVER, "", version)
}

/// Read `value` as a NON-EMPTY array of strings, or fail with the raw body.
///
/// Every failure message interpolates the verbatim response text: a parsed
/// `Value` alone cannot show whether the key was absent, null, or an array of
/// the wrong element type.
fn non_empty_string_array(value: &Value, what: &str, raw: &str) -> Vec<String> {
    let items = value
        .as_array()
        .unwrap_or_else(|| panic!("{what} must be a JSON ARRAY; body: {raw}"));
    assert!(!items.is_empty(), "{what} must be NON-EMPTY; body: {raw}");
    items
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| {
                    panic!("{what} must contain only STRINGS, found `{item}`; body: {raw}")
                })
                .to_string()
        })
        .collect()
}

// ===========================================================================
// 1. The field exists, is non-empty, and pmcp's extras survive beside it.
// ===========================================================================

/// `server/discover` emits `supportedVersions` as a non-empty array of strings
/// carrying the server's ACTUAL accept list.
///
/// The harness server is built with
/// `.with_supported_protocol_versions([V1, V2])`, so both versions must appear:
/// a test that only asserted "non-empty" would pass on a hard-coded singleton.
///
/// This test additionally PINS pmcp's two extras — `protocolVersion` and
/// `serverInfo` — which the spec `DiscoverResult` does not declare but also does
/// not forbid (the schema sets no `additionalProperties: false`). Removing them
/// to "match the spec type" would be `struct_field_missing`, a MAJOR semver
/// break; pinning them here makes that accidental removal loud.
#[tokio::test]
async fn v2_discover_emits_a_non_empty_supported_versions_array() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("server/discover", ""),
        &v2_discover_body(json!(1)),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "a well-formed v2 server/discover must be served; body: {}",
        response.raw
    );

    let advertised = non_empty_string_array(
        &response.body["result"]["supportedVersions"],
        "the discover result's `supportedVersions`",
        &response.raw,
    );
    assert!(
        advertised.iter().any(|v| v == V2),
        "the accept list the harness server was BUILT with includes {V2}, so discover must \
         advertise it; got {advertised:?}; body: {}",
        response.raw
    );
    assert!(
        advertised.iter().any(|v| v == V1),
        "the accept list the harness server was BUILT with includes {V1}, so discover must \
         advertise it — a hard-coded singleton would pass a non-empty check but fail here; \
         got {advertised:?}; body: {}",
        response.raw
    );

    // The pmcp extras the spec tolerates. Their removal is a MAJOR break.
    assert!(
        response.body["result"]["protocolVersion"].is_string(),
        "pmcp's `protocolVersion` extra must SURVIVE the field addition — removing it would be \
         `struct_field_missing`, a major semver break; body: {}",
        response.raw
    );
    assert!(
        response.body["result"]["serverInfo"].is_object(),
        "pmcp's top-level `serverInfo` extra must SURVIVE the field addition — removing it \
         would be `struct_field_missing`, a major semver break; body: {}",
        response.raw
    );

    // MEASUREMENT (RESEARCH assumption A7, which was unverified): the shared v2
    // envelope writes the server identity into `result._meta` under the reserved
    // key, which is where the suite's WARNING-severity
    // `ServerIdentifiesInResultMeta` check looks. Pinned here so a regression
    // that moved it back to the "pre-#3002 result body only" position is caught.
    assert!(
        response.body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"].is_object(),
        "the v2 envelope must identify the server INSIDE `result._meta`, not only in the \
         pre-#3002 result body; body: {}",
        response.raw
    );
}

// ===========================================================================
// 2. The rejection carries the same list under a different name.
// ===========================================================================

/// An AGREED-but-unsupported version answers `400` +
/// `UNSUPPORTED_PROTOCOL_VERSION` with `data.supported` non-empty and
/// `data.requested` echoing what was asked for.
///
/// Expected to pass on the unfixed tree — it pins the half of the correlation
/// that already worked, so a later regression on THIS side is attributable.
#[tokio::test]
async fn v2_unsupported_version_rejection_lists_supported_and_requested() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &unsupported_headers(UNSUPPORTED),
        &discover_body_claiming_version(json!(2), UNSUPPORTED),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "an unsupported protocol version must be HTTP 400; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION,
        "the header and `_meta` AGREE on {UNSUPPORTED}, so this is an accept-list rejection \
         (-32022), not a header/body mismatch (-32020); body: {}",
        response.raw
    );
    let supported = non_empty_string_array(
        &response.body["error"]["data"]["supported"],
        "the rejection's `error.data.supported`",
        &response.raw,
    );
    assert!(
        supported.iter().any(|v| v == V2),
        "the rejection must report the server's REAL accept list; got {supported:?}; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["data"]["requested"], UNSUPPORTED,
        "`error.data.requested` must echo the version the client asked for; body: {}",
        response.raw
    );
}

// ===========================================================================
// 3. THE CORRELATION TEST — one server, two probes, one accept list.
// ===========================================================================

/// Every element of `error.data.supported` appears in the discover result's
/// `supportedVersions` — the exact predicate the suite's
/// `ServerUnsupportedVersionError` check applies.
///
/// **Both probes hit ONE server.** The single `spawn_default_config` call below
/// is the load-bearing detail: a version of this test that spawned a server per
/// probe could not distinguish "one accept list read twice" from "two
/// independent lists that happen to agree", and the second is exactly the defect
/// this plan exists to prevent.
#[tokio::test]
async fn v2_discover_supported_versions_contains_every_rejection_supported_entry() {
    // ONE server. Both probes below share it.
    let (addr, handle) = spawn_default_config(build_v2_server()).await;

    let discover = post(
        addr,
        &v2_headers("server/discover", ""),
        &v2_discover_body(json!(3)),
    )
    .await;
    let rejected = post(
        addr,
        &unsupported_headers(UNSUPPORTED),
        &discover_body_claiming_version(json!(4), UNSUPPORTED),
    )
    .await;
    handle.abort();

    assert_eq!(
        discover.status, 200,
        "the discover probe must be served; body: {}",
        discover.raw
    );
    assert_eq!(
        rejected.status, 400,
        "the rejection probe must be refused; body: {}",
        rejected.raw
    );

    let advertised = non_empty_string_array(
        &discover.body["result"]["supportedVersions"],
        "the discover result's `supportedVersions`",
        &discover.raw,
    );
    let supported = non_empty_string_array(
        &rejected.body["error"]["data"]["supported"],
        "the rejection's `error.data.supported`",
        &rejected.raw,
    );

    for version in &supported {
        assert!(
            advertised.contains(version),
            "SOURCE DRIFT: the SAME server reported `{version}` in \
             `error.data.supported` but not in `server/discover`'s `supportedVersions`. \
             The two must read ONE accept list.\n  supportedVersions: {advertised:?}\n  \
             data.supported:    {supported:?}\n  discover body: {}\n  rejection body: {}",
            discover.raw,
            rejected.raw
        );
    }

    // Belt and braces: on this server the two are the SAME list, not merely a
    // subset relation that an empty-ish `supported` could satisfy vacuously.
    // `non_empty_string_array` already rejects the vacuous case; this pins the
    // stronger property the single-source implementation actually provides.
    assert_eq!(
        advertised, supported,
        "one accept list, read twice, must produce the SAME sequence; \
         discover body: {}\n  rejection body: {}",
        discover.raw, rejected.raw
    );
}
