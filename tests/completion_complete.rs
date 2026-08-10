//! Phase 118.1-04 (CONF-05, G-4): `completion/complete` must answer the spec
//! `CompleteResult` shape on **both** native dispatchers.
//!
//! # What G-4 actually was
//!
//! Before plan 118.1-04, `completion/complete` had no handler seam at all. It
//! fell into a catch-all arm on each native dispatcher, and the two arms
//! **disagreed**:
//!
//! - `src/server/mod.rs` — the high-level `Server` lumped
//!   `Subscribe | Unsubscribe | Complete | SetLoggingLevel | Ping` together and
//!   answered `Ok(json!({}))`. A client therefore got HTTP 200 with a result
//!   object that had no `completion` key. Verbatim red:
//!   `{"jsonrpc":"2.0","id":1,"result":{}}`.
//! - `src/server/core.rs` — `ServerCore`'s `_ =>` arm answered
//!   `{"code":-32601,"message":"Method not supported"}`.
//!
//! That divergence is a pre-existing defect the gaps document does not record,
//! and it is why this file has two legs rather than one. Only the high-level
//! `Server` is on the HTTP path, so the official suite could only ever have
//! measured half of it: the suite's `completion-complete` scenario failed with
//! `Missing completion field` because `result.completion.values` was not an
//! array. `ServerCore`'s half was invisible to the measurement.
//!
//! The spec shape is `CompleteResult extends Result { completion: { values:
//! string[] /* @maxItems 100 */; total?: number; hasMore?: boolean } }`
//! (`schema/vendored/core-2026-07-28/schema.ts:2644-2663`). The suite's own
//! comment sanctions a minimal implementation — "completion support can be
//! minimal or return empty arrays" — so a registered-handler-free server
//! answering `values: []` is a PASS, and an error or a bare `{}` is not.
//!
//! # Which spawn helper each leg uses, and why
//!
//! - **HTTP leg** (`Server::process_client_request`): `common::v2`'s
//!   [`spawn_default_config`], i.e. `StreamableHttpServerConfig::default()`.
//!   Per the spawn-choice doctrine at `tests/common/v2.rs:9-23`, `::stateless()`
//!   is a BUILD-TIME config that removes the session machinery before a request
//!   is ever seen; the default config keeps a live `session_id_generator`, which
//!   is the configuration a real v1 client meets and the one the dual-conformance
//!   example runs. The cost is that a bare v1 POST answers `HTTP 400 "Session ID
//!   required"`, so this file carries a `v1_open_session` handshake helper
//!   (copied from `tests/embedded_resource_example_run.rs`, which paid for that
//!   lesson in plan 03).
//! - **`ServerCore` leg**: driven DIRECTLY through the `ProtocolHandler` trait,
//!   the way `src/server/core_tests.rs` does, because **no live HTTP path
//!   reaches `ServerCore`**. There is no spawn helper to choose; a leg that
//!   "covered" this dispatcher over HTTP would in fact be re-measuring the other
//!   one.
//!
//! # Every failure message interpolates the raw response
//!
//! A red run of a wire-shape fence is only useful if it says what the wire
//! actually carried. Each assertion below prints the raw body (HTTP leg) or the
//! serialized payload (`ServerCore` leg), so the two distinct RED reasons are
//! readable from one run without a re-run under a debugger.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::net::SocketAddr;

use common::v2::{header, post, spawn_default_config, teardown, v1_body};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::completable::StaticCompletionProvider;
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::protocol::{
    CompleteRequest, CompletionArgument, CompletionReference, InitializeRequest,
    LATEST_PROTOCOL_VERSION,
};
use pmcp::types::{ClientCapabilities, ClientRequest, JSONRPCResponse, Request, RequestId};
use pmcp::{Implementation, Server};
use serde_json::{json, Value};

// ===========================================================================
// The suite's exact request.
// ===========================================================================

/// The prompt the pinned suite completes against.
///
/// Read out of `conformance/node_modules/@modelcontextprotocol/conformance/dist/index.js`
/// at offset 386462. Restating it here rather than deriving it from the example
/// is deliberate: this fence must fail if the SDK stops answering the shape, not
/// if a fixture is renamed.
const SUITE_PROMPT: &str = "test_prompt_with_arguments";

/// The argument name the suite completes.
const SUITE_ARG_NAME: &str = "arg1";

/// The partial value the suite sends.
const SUITE_ARG_VALUE: &str = "test";

/// The wire method under test.
const METHOD: &str = "completion/complete";

/// The suite's `completion/complete` params, as JSON.
fn suite_params() -> Value {
    params_with_partial(SUITE_ARG_VALUE)
}

/// The suite's params with a caller-chosen partial value.
fn params_with_partial(partial: &str) -> Value {
    json!({
        "ref": { "type": "ref/prompt", "name": SUITE_PROMPT },
        "argument": { "name": SUITE_ARG_NAME, "value": partial },
    })
}

/// The suite's `completion/complete` params with a caller-chosen partial value,
/// as the typed request the `ServerCore` leg dispatches.
fn request_with_partial(partial: &str) -> Request {
    Request::Client(Box::new(ClientRequest::Complete(CompleteRequest {
        r#ref: CompletionReference::Prompt {
            name: SUITE_PROMPT.to_string(),
        },
        argument: CompletionArgument {
            name: SUITE_ARG_NAME.to_string(),
            value: partial.to_string(),
        },
    })))
}

// ===========================================================================
// The registered provider.
// ===========================================================================

/// What [`registered_provider`] answers with.
///
/// These strings appear NOWHERE in the SDK, so a test seeing them back has
/// proved the registered provider was actually reached. An empty-array
/// assertion cannot make that distinction: a builder slot wired to nothing
/// passes every no-handler case while silently ignoring every provider.
const PROVIDER_VALUES: [&str; 3] = ["alpha-118-1-04", "beta-118-1-04", "gamma-118-1-04"];

/// A provider returning [`PROVIDER_VALUES`], recording nothing.
fn registered_provider() -> StaticCompletionProvider {
    StaticCompletionProvider::from_strings(
        PROVIDER_VALUES.iter().map(|v| (*v).to_string()).collect(),
    )
}

/// The values [`registered_provider`] returns for the suite's partial value.
///
/// [`StaticCompletionProvider`] filters by prefix against `partial`, and the
/// suite sends `"test"`, which matches none of [`PROVIDER_VALUES`]. So the
/// provider-reached assertions below drive an EMPTY partial: the point of those
/// two tests is that the seam is wired, not that the filter works.
const EMPTY_PARTIAL: &str = "";

// ===========================================================================
// HTTP leg — the high-level `Server` dispatcher.
// ===========================================================================

/// A minimal v1 server with NO completion handler registered.
///
/// The no-handler case is the one the suite actually meets on a typical server,
/// and it is the case that must answer a SUCCESS with an empty array rather than
/// an error or a bare `{}`.
fn server_without_completions() -> Server {
    Server::builder()
        .name("completion-complete-http")
        .version("1.0.0")
        .build()
        .expect("server builds")
}

/// The same server WITH a provider registered through `ServerBuilder`
/// (`src/server/mod.rs`), the high-level builder family.
fn server_with_completions() -> Server {
    Server::builder()
        .name("completion-complete-http-provider")
        .version("1.0.0")
        .completions(registered_provider())
        .build()
        .expect("server builds")
}

/// Complete the v1 session handshake and return the minted session id.
///
/// Copied from `tests/embedded_resource_example_run.rs` rather than re-derived:
/// [`spawn_default_config`] installs a LIVE `session_id_generator`, so a bare v1
/// POST answers `HTTP 400 "Session ID required"` and the resulting red says
/// nothing whatsoever about `completion/complete`. v2 has no session at all
/// (Phase 117 severed it), which is why only the v1 framing needs this.
async fn v1_open_session(addr: SocketAddr) -> String {
    let params = json!({
        "protocolVersion": LATEST_PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "completion-complete", "version": "0.0.0" },
    });
    let response = post(addr, &[], &v1_body("initialize", json!(0), params)).await;
    assert_eq!(
        response.status, 200,
        "the v1 handshake must succeed before {METHOD}: HTTP {} {}",
        response.status, response.raw
    );
    response.mcp_session_id.unwrap_or_else(|| {
        panic!(
            "the server minted no Mcp-Session-Id on initialize, so the v1 leg cannot \
             proceed. Response was: {}",
            response.raw
        )
    })
}

/// POST the suite's `completion/complete` over a live v1 session.
async fn http_complete(addr: SocketAddr) -> common::v2::Resp {
    http_complete_with(addr, suite_params()).await
}

/// [`http_complete`] with caller-chosen params.
async fn http_complete_with(addr: SocketAddr, params: Value) -> common::v2::Resp {
    let session = v1_open_session(addr).await;
    post(
        addr,
        &[header(MCP_SESSION_ID, &session)],
        &v1_body(METHOD, json!(1), params),
    )
    .await
}

/// Read `result.completion.values` as a `Vec<String>`, or fail loudly.
fn values_of(result: &Value, context: &str) -> Vec<String> {
    let values = result["completion"]["values"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("{context}: result.completion.values must be an array. Was: {result}")
        });
    values
        .iter()
        .map(|value| {
            value.as_str().map_or_else(
                || panic!("{context}: values must be string[] (schema.ts:2649). Was: {result}"),
                str::to_string,
            )
        })
        .collect()
}

#[tokio::test]
async fn http_server_answers_the_spec_completion_shape_with_no_handler_registered() {
    let (addr, handle) = spawn_default_config(server_without_completions()).await;
    let response = http_complete(addr).await;

    assert_eq!(
        response.status, 200,
        "{METHOD} must be a SUCCESS even with no completion handler registered — the \
         suite's own comment sanctions an empty array. Got HTTP {}: {}",
        response.status, response.raw
    );
    assert!(
        response.body["error"].is_null(),
        "{METHOD} must not answer a JSON-RPC error with no handler registered: {}",
        response.raw
    );

    let values = response.body["result"]["completion"]["values"].clone();
    assert!(
        values.is_array(),
        "G-4: result.completion.values MUST be an array \
         (CompleteResult, schema.ts:2644-2663). The suite fails this exact assertion \
         with `Missing completion field`. Raw response was: {}",
        response.raw
    );
    assert_eq!(
        values.as_array().map(Vec::len),
        Some(0),
        "with no handler registered the array must be EMPTY, not populated: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// `ServerCore` leg — driven directly, because no HTTP path reaches it.
// ===========================================================================

/// The `initialize` request every `ServerCore` leg sends first.
///
/// `ServerCore` refuses non-`initialize` requests with `-32002` until the
/// handshake completes, so a leg that skipped this would measure the
/// initialization gate instead of `completion/complete`.
fn core_init_request() -> Request {
    // `InitializeRequest` is `#[non_exhaustive]`, so it is built through its
    // constructor rather than a struct literal.
    Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
        Implementation::new("completion-complete", "0.0.0"),
        ClientCapabilities::default(),
    ))))
}

/// Render a `ServerCore` response as JSON so a failure message can quote it.
fn payload_of(response: &JSONRPCResponse) -> String {
    serde_json::to_string(&response.payload)
        .unwrap_or_else(|error| format!("<payload did not serialize: {error}>"))
}

/// Drive `initialize` then `completion/complete` against a built `ServerCore`.
async fn core_complete(core: &pmcp::server::core::ServerCore) -> JSONRPCResponse {
    core_complete_with(core, SUITE_ARG_VALUE).await
}

/// [`core_complete`] with a caller-chosen partial value.
async fn core_complete_with(
    core: &pmcp::server::core::ServerCore,
    partial: &str,
) -> JSONRPCResponse {
    let init = core
        .handle_request(RequestId::from(1i64), core_init_request(), None)
        .await;
    assert!(
        matches!(init.payload, ResponsePayload::Result(_)),
        "the ServerCore handshake must succeed before {METHOD}: {}",
        payload_of(&init)
    );
    core.handle_request(RequestId::from(2i64), request_with_partial(partial), None)
        .await
}

/// Unwrap a `ServerCore` response's result, failing with the payload on error.
fn result_of(response: &JSONRPCResponse, context: &str) -> Value {
    match &response.payload {
        ResponsePayload::Result(value) => value.clone(),
        ResponsePayload::Error(_) => panic!(
            "{context}: {METHOD} answered a JSON-RPC error rather than the spec shape. \
             Payload was: {}",
            payload_of(response)
        ),
    }
}

#[tokio::test]
async fn server_core_answers_the_spec_completion_shape_with_no_handler_registered() {
    let core = ServerCoreBuilder::new()
        .name("completion-complete-core")
        .version("1.0.0")
        .build()
        .expect("core builds");

    let response = core_complete(&core).await;

    let result = result_of(
        &response,
        "G-4 (the ServerCore half): `ServerCore`'s `_ =>` catch-all used to answer -32601 \
         here, which is a DIFFERENT defect from the high-level `Server`'s empty result object",
    );

    assert_eq!(
        values_of(&result, "ServerCore, no handler"),
        Vec::<String>::new(),
        "with no handler registered the array must be EMPTY, not populated: {result}"
    );
}

// ===========================================================================
// The registered provider actually reaches its own dispatcher.
//
// These are the two cases a slot wired to nothing would fail: a `completions`
// field on ONE builder family with the dispatch arm reading the OTHER server's
// (permanently `None`) field still answers the spec shape with an empty array,
// so every case above would pass while the provider was silently ignored.
// ===========================================================================

#[tokio::test]
async fn http_server_returns_the_values_of_a_provider_registered_through_server_builder() {
    let (addr, handle) = spawn_default_config(server_with_completions()).await;
    let response = http_complete_with(addr, params_with_partial(EMPTY_PARTIAL)).await;

    assert_eq!(
        response.status, 200,
        "{METHOD} with a registered provider must succeed: HTTP {} {}",
        response.status, response.raw
    );
    assert_eq!(
        values_of(
            &response.body["result"],
            "ServerBuilder-registered provider"
        ),
        PROVIDER_VALUES.to_vec(),
        "a provider registered through `ServerBuilder::completions` must reach the \
         high-level `Server` dispatcher. An empty array here means the slot exists but \
         is wired to nothing. Raw response was: {}",
        response.raw
    );

    teardown(handle, ()).await;
}

#[tokio::test]
async fn server_core_returns_the_values_of_a_provider_registered_through_core_builder() {
    let core = ServerCoreBuilder::new()
        .name("completion-complete-core-provider")
        .version("1.0.0")
        .completions(registered_provider())
        .build()
        .expect("core builds");

    let response = core_complete_with(&core, EMPTY_PARTIAL).await;
    let result = result_of(&response, "ServerCoreBuilder-registered provider");

    assert_eq!(
        values_of(&result, "ServerCoreBuilder-registered provider"),
        PROVIDER_VALUES.to_vec(),
        "a provider registered through `ServerCoreBuilder::completions` must reach the \
         `ServerCore` dispatcher. An empty array here means the slot exists but is wired \
         to nothing. Result was: {result}"
    );
}
