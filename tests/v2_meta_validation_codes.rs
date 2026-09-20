//! Phase 118.1 plan 06 (CONF-06, gaps G-6 and G-8): the SIX-ROW `_meta`
//! validation contract the MCP `2026-07-28` transport requires, as literal wire
//! assertions over a real HTTP socket.
//!
//! # Provenance — this file transcribes the referee, not pmcp
//!
//! Every row below was read out of the PINNED official suite,
//! `conformance/node_modules/@modelcontextprotocol/conformance/dist/index.js`
//! (`0.2.0-alpha.11`), around byte offsets 366019 / 370104 / 371612. The suite
//! probes `server/discover` and requires:
//!
//! | Condition | Code | HTTP | Suite check (rpcId) |
//! |---|---|---|---|
//! | `params._meta` absent entirely | `-32602` | 400 | `RequestMetaInvalid` missing-meta (101) |
//! | `_meta` without `io.modelcontextprotocol/protocolVersion` | `-32602` | 400 | missing-protocol-version (102) |
//! | `_meta` without `io.modelcontextprotocol/clientCapabilities` | `-32602` | 400 | missing-client-capabilities (104) |
//! | `_meta` without `io.modelcontextprotocol/clientInfo` | **success** | **200** | `RequestMetaClientInfoOptional` (105) |
//! | version present, EQUAL to the header, unsupported | `-32022` | 400 | `ServerUnsupportedVersionError` (301) |
//! | version present, DIFFERENT from the header | `-32020` | 400 | `HttpServerHeaderMismatch400` (302) |
//!
//! `HttpServerErrorJsonrpcId` additionally requires every one of those error
//! responses to ECHO the request's JSON-RPC id, so each rejection row asserts it.
//!
//! The JSON-RPC ids used here ARE the suite's own rpcIds (101, 102, 104, 105,
//! 301, 302), so a red run maps one-to-one onto the referee's scenario list.
//!
//! # `clientInfo` is a SHOULD — row 4 is a MUST-SERVE row
//!
//! Row 4 is the trap in this contract. It is tempting to require all three
//! reserved keys symmetrically; doing so turns a check that passes TODAY red.
//! `clientInfo` is deliberately optional and its absence must still be served.
//!
//! # G-8 is a pure ORDERING defect
//!
//! Rows 5 and 6 send the SAME body (`_meta.protocolVersion = "v999.0.0"`) and
//! differ only in the `MCP-Protocol-Version` header. Row 5's header AGREES with
//! `_meta`, so the accept-list check must fire and answer `-32022`. Row 6's
//! header DISAGREES, so the header/body cross-check must fire FIRST and answer
//! `-32020`. Both codes already existed before this plan — the defect was that
//! the accept list ran first and swallowed row 6.
//!
//! # Why `spawn_default_config` (the STATEFUL config)
//!
//! `StreamableHttpServerConfig::stateless()` is a BUILD-TIME config: it clears
//! the session generator once, at construction. A test spawned that way never
//! exercises the PER-REQUEST era gate this file is about, so every assertion
//! below would be vacuous (RESEARCH Pitfall 1). The stateful default is also the
//! realistic dual-version production shape.
//!
//! Test reliability doctrine (carried from `tests/v2_retired_methods.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN via [`common::v2::teardown`].
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, default_client_capabilities, header, jsonrpc_envelope, post,
    spawn_default_config, teardown, v2_headers_claiming, Resp, META_CLIENT_CAPABILITIES,
    META_CLIENT_INFO, META_PROTOCOL_VERSION, REQUEST_META_KEY, V1, V2,
};
use pmcp::shared::http_constants::{MCP_METHOD, MCP_PROTOCOL_VERSION};
use pmcp::types::protocol::error_codes::{
    HEADER_MISMATCH, INVALID_PARAMS, UNSUPPORTED_PROTOCOL_VERSION,
};
// The whole property-test block below is a v1-CONTRAST assertion (its matrix
// contains v1 cells), so it and its imports are gated together.
#[cfg(feature = "v1-compat")]
use proptest::test_runner::{Config as ProptestConfig, TestRunner};
use serde_json::{json, Value};
use std::net::SocketAddr;

/// The version string the referee probes an unsupported version with (rpcIds
/// 301 and 302), spelled exactly as the suite spells it.
const UNSUPPORTED_VERSION: &str = "v999.0.0";

/// The method every row targets — the same one the referee probes.
const DISCOVER: &str = "server/discover";

// ===========================================================================
// Body construction: the canonical `_meta`, MINUS one key per row.
// ===========================================================================

/// The referee's default `_meta` object (`l` in the suite source): all three
/// reserved keys, with a supported `protocolVersion`.
///
/// Key spellings come from `pmcp::testing::META_*` (re-exported through the
/// shared harness), never re-typed here, so this file cannot drift from the
/// constants the server resolver reads.
fn canonical_meta() -> Value {
    json!({
        META_PROTOCOL_VERSION: V2,
        META_CLIENT_INFO: { "name": "pmcp-conformance-probe", "version": "0.0.0" },
        META_CLIENT_CAPABILITIES: default_client_capabilities(),
    })
}

/// [`canonical_meta`] with exactly one reserved key REMOVED.
///
/// Subtraction rather than construction on purpose: each row is then provably
/// "the well-formed body minus one key", so a row cannot accidentally differ in
/// a second respect and pass or fail for the wrong reason.
fn canonical_meta_without(key: &str) -> Value {
    let mut meta = canonical_meta();
    meta.as_object_mut()
        .expect("canonical _meta is an object")
        .remove(key)
        .unwrap_or_else(|| panic!("{key} must be present in the canonical _meta to be removed"));
    meta
}

/// [`canonical_meta`] with a DIFFERENT `protocolVersion` value.
fn canonical_meta_claiming(version: &str) -> Value {
    let mut meta = canonical_meta();
    meta.as_object_mut()
        .expect("canonical _meta is an object")
        .insert(META_PROTOCOL_VERSION.to_string(), json!(version));
    meta
}

/// A JSON-RPC request body for `method`. `meta` of `None` omits the
/// `params._meta` key ENTIRELY — the row-1 shape, which is `params: {}`.
///
/// This cannot route through [`common::v2::v2_body_claiming_version`]: every row
/// here is the canonical `_meta` MINUS or MUTATED in exactly one respect, which is
/// a shape the well-formed builders cannot express by construction. Only the
/// envelope is shared, and the reserved key is spelled from the harness'
/// [`REQUEST_META_KEY`] rather than re-typed.
fn matrix_body_with_id(method: &str, id: Value, meta: Option<Value>) -> String {
    let mut params = serde_json::Map::new();
    if let Some(meta) = meta {
        params.insert(REQUEST_META_KEY.to_string(), meta);
    }
    jsonrpc_envelope(method, id, Value::Object(params))
}

/// [`matrix_body_with_id`] pinned to the referee's own probe target.
fn discover_body(id: Value, meta: Option<Value>) -> String {
    matrix_body_with_id(DISCOVER, id, meta)
}

/// The v2 routing headers for `server/discover` with an explicit
/// `MCP-Protocol-Version` value.
///
/// The referee omits `Mcp-Name` here and this sends it empty; the two are
/// indistinguishable at the gate, because `server/discover` carries no routing
/// name and Phase 118 D-13 has the server DISCARD an empty value on exactly such
/// a method. [`v2_headers_claiming`] carries the full argument. These rows assert
/// `_meta` validation CODES, so the header's presence is not what they measure.
fn discover_headers(version: &str) -> Vec<(String, String)> {
    v2_headers_claiming(DISCOVER, "", version)
}

// ===========================================================================
// Shared assertions.
// ===========================================================================

/// Assert a rejection row: HTTP 400, the required JSON-RPC code, and the echoed
/// request id (`HttpServerErrorJsonrpcId`).
fn assert_rejected(response: &Resp, expected_code: i32, id: i64, what: &str) {
    assert_eq!(
        response.status, 400,
        "{what}: the referee requires HTTP 400; raw: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["code"], expected_code,
        "{what}: wrong JSON-RPC error code; raw: {}",
        response.raw
    );
    assert_eq!(
        response.body["id"], id,
        "{what}: HttpServerErrorJsonrpcId requires the request id to be echoed; raw: {}",
        response.raw
    );
}

/// Assert a served row: HTTP 200, a `result`, and NO `error`.
fn assert_served(response: &Resp, what: &str) {
    assert_eq!(
        response.status, 200,
        "{what}: must be SERVED; raw: {}",
        response.raw
    );
    assert!(
        response.body.get("error").is_none(),
        "{what}: must carry no JSON-RPC error; raw: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "{what}: must carry a result; raw: {}",
        response.raw
    );
}

/// Spawn the shared dual-version fixture.
async fn spawn() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    spawn_default_config(build_v2_server()).await
}

// ===========================================================================
// Row 1 — rpcId 101, `RequestMetaInvalid` missing-meta.
// ===========================================================================

/// `params._meta` absent ENTIRELY on a v2 request → `-32602` + HTTP 400.
///
/// Today this answers `-32020`: the shared resolver sees no per-request version,
/// falls back to v1, and the 2x2 era classifier then reads (header v2, `_meta`
/// not v2) as a header/body disagreement. An ABSENT required key is not a
/// disagreement — that collapse is exactly gap G-6.
#[tokio::test]
async fn request_meta_invalid_missing_meta_is_32602() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(V2),
        &discover_body(json!(101), None),
    )
    .await;
    teardown(handle, ()).await;

    assert_rejected(&response, INVALID_PARAMS, 101, "missing-meta (rpcId 101)");
}

// ===========================================================================
// Row 2 — rpcId 102, `RequestMetaInvalid` missing-protocol-version.
// ===========================================================================

/// `_meta` present but WITHOUT `io.modelcontextprotocol/protocolVersion` →
/// `-32602` + HTTP 400.
#[tokio::test]
async fn request_meta_invalid_missing_protocol_version_is_32602() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(V2),
        &discover_body(
            json!(102),
            Some(canonical_meta_without(META_PROTOCOL_VERSION)),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_rejected(
        &response,
        INVALID_PARAMS,
        102,
        "missing-protocol-version (rpcId 102)",
    );
}

// ===========================================================================
// Row 3 — rpcId 104, `RequestMetaInvalid` missing-client-capabilities.
// ===========================================================================

/// `_meta` present but WITHOUT `io.modelcontextprotocol/clientCapabilities` →
/// `-32602` + HTTP 400.
///
/// Today this is SERVED at 200: `parse_reserved_object` returns `Ok(None)` for
/// an absent key, so the resolver cannot tell "absent" from "not required". That
/// is the other half of G-6.
#[tokio::test]
async fn request_meta_invalid_missing_client_capabilities_is_32602() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(V2),
        &discover_body(
            json!(104),
            Some(canonical_meta_without(META_CLIENT_CAPABILITIES)),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_rejected(
        &response,
        INVALID_PARAMS,
        104,
        "missing-client-capabilities (rpcId 104)",
    );
}

// ===========================================================================
// Row 4 — rpcId 105, `RequestMetaClientInfoOptional`. THE MUST-SERVE ROW.
// ===========================================================================

/// `_meta` WITHOUT `io.modelcontextprotocol/clientInfo` → **200 with a result**.
///
/// `clientInfo` is a SHOULD. This row passes today and must still pass after the
/// fix; making the required-key rule symmetric over all three reserved keys is
/// the single most likely way to turn a green suite check red.
#[tokio::test]
async fn request_meta_client_info_optional_is_served() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(V2),
        &discover_body(json!(105), Some(canonical_meta_without(META_CLIENT_INFO))),
    )
    .await;
    teardown(handle, ()).await;

    assert_served(&response, "clientInfo-optional (rpcId 105)");
}

// ===========================================================================
// Row 5 — rpcId 301, `ServerUnsupportedVersionError`.
// ===========================================================================

/// Header and `_meta` AGREE on an unsupported version → `-32022` + HTTP 400,
/// with `data.supported` a non-empty array and `data.requested` echoing it.
///
/// This row passes today and pins the half of the ordering fix that must NOT
/// move: when the two sides agree there is no disagreement to report, so the
/// accept-list check is the correct answer.
#[tokio::test]
async fn server_unsupported_version_error_is_32022_with_data() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(UNSUPPORTED_VERSION),
        &discover_body(
            json!(301),
            Some(canonical_meta_claiming(UNSUPPORTED_VERSION)),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_rejected(
        &response,
        UNSUPPORTED_PROTOCOL_VERSION,
        301,
        "unsupported-version (rpcId 301)",
    );
    let supported = response.body["error"]["data"]["supported"].as_array();
    assert!(
        supported.is_some_and(|versions| !versions.is_empty()),
        "-32022 MUST carry a NON-EMPTY error.data.supported array; raw: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["data"]["requested"], UNSUPPORTED_VERSION,
        "error.data.requested must echo the requested version; raw: {}",
        response.raw
    );
}

// ===========================================================================
// Row 6 — rpcId 302, `HttpServerHeaderMismatch400`.
// ===========================================================================

/// Header and `_meta` DISAGREE about the protocol version → `-32020` + HTTP 400.
///
/// Today this answers `-32022`: the accept-list check runs first, fails on
/// `v999.0.0`, and the disagreement never gets a chance to classify. That
/// ordering IS gap G-8 — the check exists, it just runs too late.
#[tokio::test]
async fn http_server_header_mismatch_400_is_32020() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &discover_headers(V2),
        &discover_body(
            json!(302),
            Some(canonical_meta_claiming(UNSUPPORTED_VERSION)),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_rejected(
        &response,
        HEADER_MISMATCH,
        302,
        "header-mismatch (rpcId 302)",
    );
}

// ===========================================================================
// Row 7 — the v1-UNAFFECTED guard. Not a referee row; the scope fence.
// ===========================================================================

/// Mint a v1 session and return the headers a v1 request must carry.
///
/// A `StreamableHttpServerConfig::default()` server is STATEFUL, so every v1
/// non-`initialize` request needs an `Mcp-Session-Id`; without one the answer is
/// `-32600 "Session ID required for non-initialization requests"` — a v1 session
/// guard that has nothing to do with `_meta` validation and would make a v1
/// control assert the wrong thing. v2 has no sessions and ignores the header.
#[cfg(feature = "v1-compat")]
async fn v1_session_headers(addr: SocketAddr) -> Vec<(String, String)> {
    use common::v2::v1_body;

    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(700),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    let session = init
        .mcp_session_id
        .clone()
        .expect("a v1 initialize on a stateful server MUST mint a session id");
    vec![
        header("mcp-session-id", &session),
        header(MCP_PROTOCOL_VERSION, V1),
    ]
}

/// A v1 request whose `_meta` omits `clientCapabilities` is STILL SERVED.
///
/// The required-key rule is V2-ONLY. `resolve_protocol_context` serves BOTH
/// eras and its absent-`protocolVersion` arm IS the v1 fallback, so a rule
/// planted there would reject every v1 request on the server. This test is the
/// executable proof that the rule stayed in the transport's v2 gate.
///
/// Two shapes, because they would fail differently: a v1 request with NO `_meta`
/// at all (the overwhelming majority of real v1 traffic) and a v1 request that
/// DOES carry a `_meta` with a v1 `protocolVersion` and no `clientCapabilities`
/// (the shape that trips a rule keyed on "`_meta` is present").
///
/// v1 CONTROL — gated behind `v1-compat` (Phase 117): on a
/// `--no-default-features --features full-v2` build there is no v1 half to
/// contrast against, and the severance itself is the proof. Gated per-TEST so
/// the six v2 rows above keep RUNNING on the severed build.
#[cfg(feature = "v1-compat")]
#[tokio::test]
async fn v1_requests_omitting_client_capabilities_are_still_served() {
    let (addr, handle) = spawn().await;
    let session = v1_session_headers(addr).await;

    let bare = post(
        addr,
        &session,
        &matrix_body_with_id("tools/list", json!(701), None),
    )
    .await;
    let with_v1_meta = post(
        addr,
        &session,
        &matrix_body_with_id(
            "tools/list",
            json!(702),
            Some(json!({
                META_PROTOCOL_VERSION: V1,
                META_CLIENT_INFO: { "name": "v1-client", "version": "1.0.0" },
            })),
        ),
    )
    .await;
    teardown(handle, ()).await;

    for (label, response) in [
        ("no _meta at all", &bare),
        ("v1 _meta with no clientCapabilities", &with_v1_meta),
    ] {
        assert_served(response, &format!("v1 tools/list ({label})"));
        assert!(
            response.body["result"]["tools"].is_array(),
            "v1 ({label}) must reach dispatch, not the v2 gate; raw: {}",
            response.raw
        );
    }
}

// ===========================================================================
// The property test.
//
// # Its oracle is INDEPENDENT of the code under test
//
// `referee_verdict` below is a transcription of the suite's own rule, and the
// six literal tests above are its calibration: each of them is one point on this
// surface, asserted separately with the referee's own rpcId. A property whose
// oracle is the predicate under test proves nothing — that is the exact shape
// that reopened Phase 115 four times — so nothing in `referee_verdict` reads a
// pmcp function.
//
// The whole module is gated behind `v1-compat` because the matrix contains v1
// cells; on a `--no-default-features --features full-v2` build there is no v1
// half to assert and the severance itself is the proof. The six v2 rows above
// stay ungated and keep RUNNING on the severed build.
// ===========================================================================
#[cfg(feature = "v1-compat")]
mod referee_matrix {
    use super::{
        default_client_capabilities, header, matrix_body_with_id, post, spawn, teardown,
        v1_session_headers, ProptestConfig, Resp, TestRunner, HEADER_MISMATCH, INVALID_PARAMS,
        MCP_METHOD, MCP_PROTOCOL_VERSION, META_CLIENT_CAPABILITIES, META_CLIENT_INFO,
        META_PROTOCOL_VERSION, UNSUPPORTED_PROTOCOL_VERSION, UNSUPPORTED_VERSION, V1, V2,
    };
    use proptest::prelude::{prop_oneof, Just, Strategy};
    use serde_json::{json, Value};

    /// The `MCP-Protocol-Version` header axis.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HeaderCase {
        /// `2026-07-28`.
        V2,
        /// `2025-11-25` — supported, but not v2.
        Legacy,
        /// `v999.0.0` — decodable and supported by nobody.
        Unsupported,
        /// No `MCP-Protocol-Version` header at all.
        Absent,
    }

    impl HeaderCase {
        fn value(self) -> Option<&'static str> {
            match self {
                Self::V2 => Some(V2),
                Self::Legacy => Some(V1),
                Self::Unsupported => Some(UNSUPPORTED_VERSION),
                Self::Absent => None,
            }
        }
    }

    /// The `params._meta` axis.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MetaCase {
        /// No `params._meta` key at all.
        Absent,
        /// `_meta` present, `protocolVersion` key absent.
        NoVersion,
        /// `_meta.protocolVersion` = `2026-07-28`.
        V2Version,
        /// `_meta.protocolVersion` = `2025-11-25`.
        LegacyVersion,
        /// `_meta.protocolVersion` = `v999.0.0`.
        UnsupportedVersion,
    }

    impl MetaCase {
        /// The `protocolVersion` this case carries, if any.
        fn version(self) -> Option<&'static str> {
            match self {
                Self::Absent | Self::NoVersion => None,
                Self::V2Version => Some(V2),
                Self::LegacyVersion => Some(V1),
                Self::UnsupportedVersion => Some(UNSUPPORTED_VERSION),
            }
        }

        /// The `_meta` value for this case, given whether `clientCapabilities` is
        /// declared.
        fn build(self, caps: bool) -> Option<Value> {
            if self == Self::Absent {
                return None;
            }
            let mut meta = serde_json::Map::new();
            if let Some(version) = self.version() {
                meta.insert(META_PROTOCOL_VERSION.to_string(), json!(version));
            }
            meta.insert(
                META_CLIENT_INFO.to_string(),
                json!({ "name": "pmcp-conformance-probe", "version": "0.0.0" }),
            );
            if caps {
                meta.insert(
                    META_CLIENT_CAPABILITIES.to_string(),
                    default_client_capabilities(),
                );
            }
            Some(Value::Object(meta))
        }
    }

    /// What the referee requires for one cell of the matrix.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Verdict {
        /// HTTP 200 with a `result`.
        Served,
        /// HTTP 400 with this JSON-RPC error code.
        Rejected(i32),
    }

    /// THE ORACLE — the referee's rule, written out longhand.
    ///
    /// Reads only the two axis enums; calls nothing from `pmcp`.
    ///
    /// The last arm is the one clause here that is NOT one of the referee's six
    /// rows: pmcp's v1 path independently rejects an `MCP-Protocol-Version` header
    /// outside `SUPPORTED_PROTOCOL_VERSIONS` with `INVALID_REQUEST`. It is included
    /// so the matrix can be a full product rather than a hand-pruned subset, and it
    /// is flagged as pmcp-specific rather than referee-derived.
    fn referee_verdict(header: HeaderCase, meta: MetaCase, caps: bool) -> Verdict {
        let meta_version = meta.version();
        let signals_v2 = header == HeaderCase::V2 || meta_version == Some(V2);

        if signals_v2 {
            // Row 1: `_meta` absent entirely.
            if meta == MetaCase::Absent {
                return Verdict::Rejected(INVALID_PARAMS);
            }
            // Row 2: `protocolVersion` key absent.
            let Some(version) = meta_version else {
                return Verdict::Rejected(INVALID_PARAMS);
            };
            // Row 6: the two sides disagree. Checked BEFORE the accept list.
            if !(header == HeaderCase::V2 && version == V2) {
                return Verdict::Rejected(HEADER_MISMATCH);
            }
            // Row 3 vs row 4: capabilities are required, clientInfo is not.
            if !caps {
                return Verdict::Rejected(INVALID_PARAMS);
            }
            return Verdict::Served;
        }

        // No v2 signal on either side — the v1 / negotiation path.
        // Row 5: an unsupported version in `_meta` is the accept-list rejection.
        if matches!(meta_version, Some(v) if v != V1) {
            return Verdict::Rejected(UNSUPPORTED_PROTOCOL_VERSION);
        }
        // pmcp's own v1 header guard (NOT a referee row): an unknown header version.
        if header == HeaderCase::Unsupported {
            return Verdict::Rejected(pmcp::types::protocol::error_codes::INVALID_REQUEST);
        }
        Verdict::Served
    }

    fn header_strategy() -> impl Strategy<Value = HeaderCase> {
        prop_oneof![
            Just(HeaderCase::V2),
            Just(HeaderCase::Legacy),
            Just(HeaderCase::Unsupported),
            Just(HeaderCase::Absent),
        ]
    }

    fn meta_strategy() -> impl Strategy<Value = MetaCase> {
        prop_oneof![
            Just(MetaCase::Absent),
            Just(MetaCase::NoVersion),
            Just(MetaCase::V2Version),
            Just(MetaCase::LegacyVersion),
            Just(MetaCase::UnsupportedVersion),
        ]
    }

    /// The HTTP status a [`Verdict`] requires. Every code in this matrix — the three
    /// v2 transport codes plus pmcp's v1 header guard — is a 400.
    fn expected_status(verdict: Verdict) -> u16 {
        match verdict {
            Verdict::Served => 200,
            Verdict::Rejected(_) => 400,
        }
    }

    /// Classify a live response into the same [`Verdict`] vocabulary the oracle uses.
    fn observed_verdict(response: &Resp) -> Verdict {
        response.body["error"]["code"].as_i64().map_or(
            Verdict::Served,
            #[allow(clippy::cast_possible_truncation)]
            |code| Verdict::Rejected(code as i32),
        )
    }

    /// Every cell of `(header) x (_meta) x (capabilities declared?)` answers what
    /// the referee's rule says it must.
    ///
    /// Driven over REAL HTTP against ONE long-lived server: the whole subject is a
    /// wire contract, and an in-process classifier call would not prove the HTTP
    /// status.
    ///
    /// # Why `tools/list` here and `server/discover` in the six literal rows
    ///
    /// The referee probes `server/discover`, and the six rows above follow it
    /// exactly. But `server/discover` is a v2-ONLY method: on v1 it correctly
    /// answers `-32601`, which would make every v1 cell of this matrix a rejection
    /// for a reason that has nothing to do with `_meta` validation. `tools/list`
    /// exists on BOTH eras, so the v1 half of the matrix is a genuine SERVED
    /// control. The rule under test is method-independent — it is a transport gate —
    /// so exercising it on a second method strengthens rather than weakens the claim.
    ///
    /// `proptest` is driven through [`TestRunner`] rather than the `proptest!` macro
    /// because the macro expands to a bare `#[test] fn` that cannot see the spawned
    /// server; this shape lets one runtime and one socket serve every case.
    ///
    /// Gated behind `v1-compat` for the same reason as the v1 control above: the
    /// matrix contains v1 cells, which need a v1 session to be judged on `_meta`
    /// rather than on the stateful session guard.
    #[cfg(feature = "v1-compat")]
    #[test]
    fn the_six_row_rule_holds_over_the_whole_header_meta_matrix() {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime builds");
        let (addr, handle) = runtime.block_on(spawn());
        // Carried on EVERY request, v2 included: v2 has no sessions and ignores an
        // inbound `Mcp-Session-Id`, while every v1 cell needs one to get past the
        // stateful session guard and be judged on its `_meta`.
        let session = runtime.block_on(v1_session_headers(addr));

        let config = ProptestConfig {
            // 40 distinct cells; 120 cases samples each one several times over
            // while keeping the whole test to a few seconds of loopback traffic.
            cases: 120,
            ..ProptestConfig::default()
        };
        let outcome = TestRunner::new(config).run(
            &(header_strategy(), meta_strategy(), proptest::bool::ANY),
            |(header_case, meta, caps)| {
                let expected = referee_verdict(header_case, meta, caps);
                let mut headers = session.clone();
                headers.push(header(MCP_METHOD, "tools/list"));
                if let Some(version) = header_case.value() {
                    // Replace the v1 session's own version header.
                    headers.retain(|(name, _)| !name.eq_ignore_ascii_case(MCP_PROTOCOL_VERSION));
                    headers.push(header(MCP_PROTOCOL_VERSION, version));
                } else {
                    headers.retain(|(name, _)| !name.eq_ignore_ascii_case(MCP_PROTOCOL_VERSION));
                }
                let body = matrix_body_with_id("tools/list", json!(1), meta.build(caps));
                let response = runtime.block_on(post(addr, &headers, &body));
                let label =
                    format!("cell (header={header_case:?}, meta={meta:?}, caps={caps}) disagreed");
                proptest::prop_assert_eq!(
                    observed_verdict(&response),
                    expected,
                    "{} with the referee's rule; raw: {}",
                    label,
                    response.raw
                );
                proptest::prop_assert_eq!(
                    response.status,
                    expected_status(expected),
                    "{} with the referee's HTTP status; raw: {}",
                    label,
                    response.raw
                );
                Ok(())
            },
        );

        runtime.block_on(teardown(handle, ()));
        outcome.expect("every matrix cell must match the referee's rule");
    }
} // mod referee_matrix
