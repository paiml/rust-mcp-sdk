#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(feature = "v1-compat"),
    not(target_arch = "wasm32")
))]
//! On a build with NO v1, an `initialize` POST's `MCP-Protocol-Version` header
//! still agrees with its own body — proven by RUNNING on that build.
//!
//! # The defect this file closes
//!
//! Plan 117-12 put `is_initialize_request` and `extract_negotiated_version` into
//! the `v1` paired module and gave them `false` / `None` twins. Both functions
//! hold **zero v1 state** — one is a `matches!` over a `TransportMessage`, the
//! other a `serde_json::from_value` over a response payload — so the twins were
//! not severance, they were a semantic change, and it reached the wire:
//!
//! ```text
//! HttpIngress::is_initialize()  -> v1::is_initialize_request(msg) -> false   (the twin)
//! negotiated_version            -> None                                      (branch not taken)
//! compute_outbound_protocol_version(.., is_init_request = false, None)
//!                               -> crate::DEFAULT_PROTOCOL_VERSION == "2025-03-26"
//! MCP-Protocol-Version: 2025-03-26      <- while the body says "2025-11-25"
//! ```
//!
//! A `full-v2` server still SERVES `initialize`: `v2_verb_rejection` is wired
//! only to GET and DELETE, so the POST reaches `Server` core and is dispatched
//! normally. The severed build therefore answered with a header that disagreed
//! with its own `InitializeResult` body, and the value it sent was LOWER than the
//! one it negotiated. `StreamableHttpTransport` stores that header
//! (`src/shared/streamable_http.rs`) and replays it on every subsequent request,
//! so the twin caused a silent protocol DOWNGRADE decided by nothing but the
//! feature set the server was compiled with.
//!
//! # Why the existing severed-build suite could not catch it
//!
//! `tests/v2_verbs_405_on_severed_build.rs` and
//! `tests/v2_client_carries_no_session_on_severed_build.rs` between them send
//! GET, DELETE and a `tools/list` POST. Neither ever sends `initialize`, which is
//! the ONE request the two twins are reachable from. The severance proof was
//! strong about what does not EXIST and silent about the one behaviour the cut
//! actually changed. This file is that missing execution.
//!
//! # What this file pins
//!
//! The header and the body must agree. That formulation is deliberate and is
//! stronger than pinning a literal version string: it stays true when the
//! negotiation table changes, and it is EXACTLY the invariant the twins broke.
//! Re-twinning either classifier makes
//! [`the_initialize_header_agrees_with_the_initialize_body`] fail, because the
//! header collapses to `DEFAULT_PROTOCOL_VERSION` while the body does not.
//!
//! # The `not(feature = "v1-compat")` predicate, and its failure mode
//!
//! Same reasoning as its sibling files (see
//! `tests/v2_verbs_405_on_severed_build.rs` for the full argument): this is a
//! `cfg` predicate inside pmcp's own test compilation, not a subtractive cargo
//! feature. The corollary is the same too — on a `v1-compat` build this file
//! compiles to ZERO tests and `cargo test` still exits 0, so a run reporting
//! `0 tests` is a FAILURE of its purpose. The count is enforced in CI by
//! `scripts/run-severance-proofs.sh`, not by an in-file assertion: a test inside a
//! conditionally-compiled file can never police whether that file was compiled.
//!
//! # Run it with
//!
//! ```text
//! cargo test --test v2_initialize_negotiated_version_header \
//!   --no-default-features --features full-v2
//! ```

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::v2::{build_v2_server, header, post, spawn_with, teardown, Resp, V1};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::shared::http_constants::MCP_PROTOCOL_VERSION;
use serde_json::json;
use tokio::task::JoinHandle;

/// Upper bound on any single request in this file — a hung server must FAIL the
/// test rather than hang the suite.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Spawn the shared v2 fixture naming only fields present on BOTH feature sets.
///
/// `enable_json_response` keeps the reply a bare JSON body rather than an SSE
/// frame, so a header/body disagreement is read off one response with no framing
/// in the way.
async fn spawn() -> (SocketAddr, JoinHandle<()>) {
    let config = StreamableHttpServerConfig {
        enable_json_response: true,
        ..Default::default()
    };
    spawn_with(build_v2_server(), config).await
}

/// A well-formed MCP 2025-11-25 `initialize` request body.
fn initialize_body() -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": V1,
            "capabilities": {},
            "clientInfo": { "name": "severance-probe", "version": "0.0.0" }
        }
    })
    .to_string()
}

/// Await `future` under [`REQUEST_TIMEOUT`], failing the test if it does not settle.
async fn within<F: std::future::Future<Output = Resp>>(what: &str, future: F) -> Resp {
    tokio::time::timeout(REQUEST_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| {
            panic!("FAILURE MODE: {what} did not answer within {REQUEST_TIMEOUT:?}.")
        })
}

/// POST `initialize` to the severed build and return the response.
async fn post_initialize(addr: SocketAddr) -> Resp {
    within(
        "an initialize POST",
        post(
            addr,
            &[header(MCP_PROTOCOL_VERSION, V1)],
            &initialize_body(),
        ),
    )
    .await
}

/// The control: a `full-v2` build ANSWERS `initialize`, it does not refuse it.
///
/// Stated as its own test because the whole defect rests on this fact, and
/// because it is the one a reader is most likely to doubt. If a later change
/// makes the severed build refuse `initialize` outright, this test fails loudly
/// and points at the doc that must be corrected — rather than silently making
/// [`the_initialize_header_agrees_with_the_initialize_body`] vacuous by never
/// reaching a `200`.
#[tokio::test]
async fn a_severed_build_still_answers_initialize() {
    let (addr, handle) = spawn().await;

    let response = post_initialize(addr).await;

    assert_eq!(
        response.status, 200,
        "FAILURE MODE: `initialize` on a `full-v2` build answered {}, not 200.\n\
         CONSEQUENCE: either the severed build now REFUSES `initialize` — a wire change this \
         phase never claimed, and one `docs/v1-sunset-policy.md` documents the opposite of — or \
         the fixture is broken, in which case every other assertion here is vacuous.\n\
         BODY: {}\n\
         WHAT TO DO: if refusing `initialize` is now deliberate, say so in \
         `docs/v1-sunset-policy.md` and in `v1_session_off.rs`, and rewrite this file. Do not \
         delete it.",
        response.status, response.raw
    );

    let negotiated = response
        .body
        .get("result")
        .and_then(|r| r.get("protocolVersion"))
        .and_then(serde_json::Value::as_str);
    assert!(
        negotiated.is_some(),
        "FAILURE MODE: the `initialize` reply carried no `result.protocolVersion`.\n\
         BODY: {}\n\
         WHAT TO DO: the control must be a REAL handshake round trip, not merely a 200.",
        response.raw
    );

    teardown(handle, ()).await;
}

/// The regression: the outbound header equals the negotiated version in the body.
///
/// This is the assertion that FAILS if `is_initialize_request` or
/// `extract_negotiated_version` is pushed back into the `v1` pair. With either
/// twin in place the header collapses to `DEFAULT_PROTOCOL_VERSION`
/// (`2025-03-26`) while the body keeps reporting what the server actually
/// negotiated, so the two stop agreeing.
#[tokio::test]
async fn the_initialize_header_agrees_with_the_initialize_body() {
    let (addr, handle) = spawn().await;

    let response = post_initialize(addr).await;

    let body_version = response
        .body
        .get("result")
        .and_then(|r| r.get("protocolVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: no `result.protocolVersion` to compare the header against.\n\
                 BODY: {}",
                response.raw
            )
        })
        .to_string();

    let header_version = response.mcp_version.clone().unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: the `initialize` reply carried no `{MCP_PROTOCOL_VERSION}` header at \
             all.\n\
             CONSEQUENCE: a client that reads the header to learn the negotiated version has \
             nothing to read, on the build this phase created.\n\
             BODY: {}",
            response.raw
        )
    });

    assert_eq!(
        header_version, body_version,
        "FAILURE MODE: the `initialize` reply's `{MCP_PROTOCOL_VERSION}: {header_version}` header \
         disagrees with its own body's `protocolVersion: {body_version}`.\n\
         CONSEQUENCE: `StreamableHttpTransport` stores the HEADER and replays it on every \
         subsequent request, so the client is silently downgraded to a version the server never \
         negotiated — and it happens only on `full-v2`, i.e. it is decided by the feature set the \
         server was compiled with rather than by anything on the wire.\n\
         WHAT TO DO: `is_initialize_request` and `extract_negotiated_version` are PURE message \
         classifiers holding no v1 state. They belong ungated in \
         `src/server/streamable_http_server.rs`, NOT in the `v1` pair. If you moved either into \
         `v1_session.rs`/`v1_session_off.rs`, move it back; the `false`/`None` twins are what \
         produce exactly this disagreement.\n\
         BODY: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

/// A NON-init POST echoes the version the CLIENT asserted.
///
/// # History — KEEP. This test asserted the OPPOSITE, deliberately
///
/// Until `tests/stateless_negotiated_version_header.rs` landed, this test was
/// named `a_non_init_post_still_echoes_the_default` and pinned
/// `DEFAULT_PROTOCOL_VERSION` here as CORRECT — "pre-existing behaviour", with
/// the explicit purpose of making a future blanket-raise of the header "face a
/// test rather than look like an improvement". That guard did its job: this is
/// the test being faced, so the reasoning is recorded rather than deleted.
///
/// What changed is evidence, not taste. The behaviour was measured on a live
/// deployment: a STATELESS server has no session to recover the negotiated
/// version from, so it answered `2025-03-26` to every request after the
/// handshake, and `StreamableHttpTransport` latched that header and replayed it
/// — the identical downgrade this file's own sibling test calls a FAILURE MODE
/// forty lines above, arriving by a different route. Two things settle it:
///
/// 1. The consequence is the one this file already names. A header that
///    disagrees with the negotiated version silently downgrades the client. The
///    init branch was fixed for exactly that reason; the mechanism does not
///    become benign because a different request triggered it. On a stateless
///    server it is strictly worse — every request, not one.
/// 2. It was never "no information available". The client asserted
///    `MCP-Protocol-Version: 2025-11-25` on THIS request. Answering `2025-03-26`
///    contradicts information the server was handed, and the asserted header
///    exists precisely so a session-less server can read it.
///
/// The old rationale's load-bearing claim was that the value predates the phase.
/// That is true and is a fact about history, not about correctness.
///
/// The echo remains bounded: `validate_protocol_version_supported` answers `400`
/// to anything outside `SUPPORTED_PROTOCOL_VERSIONS`, and
/// `known_protocol_version` re-maps the survivor onto the SDK's own `&'static
/// str`, so no client-chosen bytes can reach a response header.
#[tokio::test]
async fn a_non_init_post_echoes_the_version_the_client_asserted() {
    let (addr, handle) = spawn().await;

    let body = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    })
    .to_string();
    let response = within(
        "a v1-era tools/list POST",
        post(addr, &[header(MCP_PROTOCOL_VERSION, V1)], &body),
    )
    .await;

    assert_eq!(
        response.status, 200,
        "FAILURE MODE: the non-init control answered {}, so this test proves nothing.\n\
         BODY: {}",
        response.status, response.raw
    );
    assert_eq!(
        response.mcp_version.as_deref(),
        Some(V1),
        "FAILURE MODE: a session-less non-init POST echoed {:?} rather than the \
         `{V1}` the client asserted on the request.\n\
         CONSEQUENCE: a `{}` here is `compute_outbound_protocol_version` \
         reaching its DEFAULT fallback again. The client latches this header and \
         replays it, so the server has silently downgraded the connection to a \
         version it never negotiated — on every request after the handshake, for \
         every stateless deployment.\n\
         WHAT TO DO: restore the `asserted_version` arm of \
         `compute_outbound_protocol_version`, not this assertion. \
         `tests/stateless_negotiated_version_header.rs` is the live-HTTP fence \
         for the same rule on a v1-compat build.\n\
         BODY: {}",
        response.mcp_version,
        pmcp::DEFAULT_PROTOCOL_VERSION,
        response.raw
    );

    teardown(handle, ()).await;
}
