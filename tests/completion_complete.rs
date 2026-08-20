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

use std::collections::HashMap;
#[cfg(feature = "v1-compat")]
use std::net::SocketAddr;
#[cfg(feature = "v1-compat")]
use std::time::Duration;

#[cfg(feature = "v1-compat")]
use common::example_process::{
    spawn_example, target_dir, wait_until_listening, wait_until_released,
};
#[cfg(feature = "v1-compat")]
use common::v2::{header, post, spawn_default_config, teardown, v1_body, v2_body, v2_headers_for};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
#[cfg(feature = "v1-compat")]
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::completable::{
    CompletionItem, CompletionProviderTrait, CompletionRequest, CompletionResponse,
    StaticCompletionProvider,
};
use pmcp::types::jsonrpc::ResponsePayload;
#[cfg(feature = "v1-compat")]
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use pmcp::types::protocol::{
    CompleteRequest, CompletionArgument, CompletionReference, InitializeRequest,
};
use pmcp::types::{ClientCapabilities, ClientRequest, JSONRPCResponse, Request, RequestId};
use pmcp::Implementation;
#[cfg(feature = "v1-compat")]
use pmcp::Server;
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
#[cfg(feature = "v1-compat")]
use serde_json::json;
use serde_json::Value;

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
#[cfg(feature = "v1-compat")]
fn suite_params() -> Value {
    params_with_partial(SUITE_ARG_VALUE)
}

/// The suite's params with a caller-chosen partial value.
#[cfg(feature = "v1-compat")]
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
#[cfg(feature = "v1-compat")]
fn server_without_completions() -> Server {
    Server::builder()
        .name("completion-complete-http")
        .version("1.0.0")
        .build()
        .expect("server builds")
}

/// The same server WITH a provider registered through `ServerBuilder`
/// (`src/server/mod.rs`), the high-level builder family.
#[cfg(feature = "v1-compat")]
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
// The v1 leg needs a MINTED SESSION, and a `--no-default-features
// --features full-v2` build mints none: v1 is severed, so `initialize`
// answers without an `Mcp-Session-Id` and this helper panics. CI's v1
// Severance Gate runs the AGGREGATE `cargo test -p pmcp
// --no-default-features --features full-v2`, which compiles and RUNS every
// test target in this file, so the v1-only items must be gated out of that
// build rather than left to fail in it.
#[cfg(feature = "v1-compat")]
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
#[cfg(feature = "v1-compat")]
async fn http_complete(addr: SocketAddr) -> common::v2::Resp {
    http_complete_with(addr, suite_params()).await
}

/// [`http_complete`] with caller-chosen params.
#[cfg(feature = "v1-compat")]
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

#[cfg(feature = "v1-compat")]
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

#[cfg(feature = "v1-compat")]
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

// ===========================================================================
// PROPERTY: the `@maxItems 100` bound holds over provider outputs of any size.
//
// Driven through the REAL `ServerCore` dispatcher rather than the internal
// helper, because the bound is only load-bearing if the dispatcher is the thing
// that applies it. A property over the helper alone would still pass if a
// future dispatch arm bypassed it.
// ===========================================================================

/// A provider returning exactly `count` synthetic values, unconditionally.
///
/// Deliberately NOT [`StaticCompletionProvider`]: that one prefix-filters, so a
/// property over "provider output length" would really be a property over the
/// filter. This one ignores `partial` entirely, which makes `count` the single
/// independent variable.
struct CountingProvider {
    count: usize,
}

#[async_trait::async_trait]
impl CompletionProviderTrait for CountingProvider {
    async fn complete(&self, _request: CompletionRequest) -> pmcp::Result<CompletionResponse> {
        Ok(CompletionResponse {
            completions: (0..self.count)
                .map(|index| CompletionItem {
                    value: format!("candidate-{index}"),
                    label: None,
                    description: None,
                    icon: None,
                    metadata: HashMap::new(),
                })
                .collect(),
            has_more: false,
            continuation_token: None,
        })
    }
}

/// The spec's bound, restated from the schema rather than imported from the
/// crate.
///
/// `MAX_COMPLETION_VALUES` is `pub(crate)`, so this file could not import it
/// even if that were desirable — and it is not: a fence that reads the same
/// constant the implementation reads cannot catch a wrong constant. Source:
/// `schema/vendored/core-2026-07-28/schema.ts:2649`, `@maxItems 100`.
const SPEC_MAX_VALUES: usize = 100;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// For ANY provider output length, the emitted answer is a well-formed
    /// `CompleteResult` that never exceeds the spec bound and whose `hasMore`
    /// agrees with whether anything was dropped.
    #[test]
    fn emitted_values_never_exceed_the_spec_bound(count in 0usize..250) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime builds");

        runtime.block_on(async move {
            let core = ServerCoreBuilder::new()
                .name("completion-complete-property")
                .version("1.0.0")
                .completions(CountingProvider { count })
                .build()
                .expect("core builds");

            let response = core_complete_with(&core, EMPTY_PARTIAL).await;
            let result = result_of(&response, "property: any provider output length");
            let values = values_of(&result, "property: any provider output length");

            prop_assert!(
                values.len() <= SPEC_MAX_VALUES,
                "a provider returning {count} values must be truncated to at most \
                 {SPEC_MAX_VALUES} (schema.ts:2649 @maxItems 100). Got {}: {result}",
                values.len()
            );
            prop_assert_eq!(
                values.len(),
                count.min(SPEC_MAX_VALUES),
                "truncation must keep as many values as the bound allows, not fewer: {}",
                result
            );

            let truncated = count > SPEC_MAX_VALUES;
            prop_assert_eq!(
                result["completion"]["hasMore"].as_bool(),
                Some(truncated),
                "hasMore must be true EXACTLY when elements were dropped, so a truncated \
                 list is never readable as exhaustive: {}",
                result
            );
            prop_assert_eq!(
                result["completion"]["total"].as_u64(),
                Some(count as u64),
                "this provider reports has_more = false, so it returned everything it had \
                 and `total` is the TRUE total including the dropped elements: {}",
                result
            );
            Ok(())
        })?;
    }
}

// ===========================================================================
// EXAMPLE leg — the dual-conformance example is RUN, not merely built.
//
// CLAUDE.md's ALWAYS requirements demand a working example per feature, and
// `make test-examples` only BUILDS examples (`Makefile:254-258` says so in its
// own banner). So "the example demonstrates the closed gap" was, until this
// leg, an unenforced SUMMARY claim. A compiled example that never answers a
// request cannot distinguish a closed gap from a fix written into a handler
// that is never reached.
//
// This leg spawns the ALREADY-BUILT binary, drives a real `completion/complete`
// against it on BOTH eras through the `common::v2` handshake helpers, and
// records both answers as an artifact. It FAILS rather than skipping when the
// binary is absent: a skip would restore exactly the unenforced criterion it
// exists to close.
//
// The request is framed through `tests/common/v2.rs`, NOT a hand-rolled curl.
// The v2 era signal is three headers (`Mcp-Method`, `Mcp-Name`,
// `MCP-Protocol-Version`) plus a reserved `params._meta`, and a shell probe
// gets that contract wrong in ways that produce a red saying nothing about the
// gap under test.
// ===========================================================================

/// The example's compiled path, relative to the target directory.
#[cfg(feature = "v1-compat")]
const EXAMPLE_REL_PATH: &str = "debug/examples/s54_v2_dual_conformance";

/// Port 8153, deliberately.
///
/// 8147 is `s47_v2_stateless_mrtr`, 8149 is this example's own default, 8150 is
/// `s50`/`s51`, 8151 belongs to `scripts/run-conformance-suite.sh`, 8155 is the
/// fallback hint in the example's own bind-failure message, and 8157 is held by
/// `tests/embedded_resource_example_run.rs` (plan 03), which runs CONCURRENTLY
/// with this file under nextest. 8080/8081 belong to
/// `scripts/test_examples_with_tester.sh`.
#[cfg(feature = "v1-compat")]
const BIND_ADDR: &str = "127.0.0.1:8153";

/// Where the recorded answers land, for the SUMMARY to quote verbatim.
#[cfg(feature = "v1-compat")]
const ARTIFACT_REL_PATH: &str = "118.1-04-example-response.json";

/// How long the child gets to bind its socket before the leg gives up.
#[cfg(feature = "v1-compat")]
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the port gets to become free again after the child is killed.
#[cfg(feature = "v1-compat")]
const RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// Assert one era's answer carries the spec `CompleteResult` shape with at least
/// one candidate — the example registers a real provider, so an empty array here
/// would mean the seam was never reached.
#[cfg(feature = "v1-compat")]
fn assert_example_answer(era: &str, response: &common::v2::Resp) {
    assert_eq!(
        response.status, 200,
        "{era}: the example must serve {METHOD}, got HTTP {}: {}",
        response.status, response.raw
    );
    let values = values_of(&response.body["result"], era);
    assert!(
        !values.is_empty(),
        "{era}: the example registers a completion provider whose values are prefixed \
         `test`, and the suite's partial is `test`, so at least one candidate must come \
         back. An empty array means the registered provider was not reached. Raw \
         response was: {}",
        response.raw
    );
    for value in &values {
        assert!(
            value.starts_with(SUITE_ARG_VALUE),
            "{era}: `{value}` does not start with the requested partial `{SUITE_ARG_VALUE}`, \
             so the provider was not consulted with the request's argument value: {}",
            response.raw
        );
    }
}

#[cfg(feature = "v1-compat")]
#[tokio::test]
async fn the_dual_conformance_example_serves_completion_complete_on_both_eras() {
    let (addr, mut guard) = spawn_example(EXAMPLE_REL_PATH, BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // BOTH legs run before EITHER is asserted: an assertion on the v1 leg would
    // panic before the v2 leg executed, leaving the v2 half of the claim with
    // zero executed evidence. The artifact is written first for the same reason.
    let params = suite_params();
    let session = v1_open_session(addr).await;
    let v1 = post(
        addr,
        &[header(MCP_SESSION_ID, &session)],
        &v1_body(METHOD, json!(1), params.clone()),
    )
    .await;
    let v2 = post(
        addr,
        &v2_headers_for(METHOD, &params),
        &v2_body(METHOD, json!(2), params.clone()),
    )
    .await;

    // The artifact carries the PARSED body as well as the raw text: the raw
    // text alone is JSON-escaped inside this file, so a `"completion"` key would
    // not appear as `"completion"` to a reader (or a grep) of the artifact.
    let artifact = json!({
        "note": format!(
            "Live `{METHOD}` on {SUITE_PROMPT} (argument {SUITE_ARG_NAME} = \
             \"{SUITE_ARG_VALUE}\"), served by target/{EXAMPLE_REL_PATH} bound to \
             {BIND_ADDR}. Phase 118.1-04, G-4 / CONF-05."
        ),
        "request": params,
        "v1": { "raw": v1.raw, "body": v1.body },
        "v2": { "raw": v2.raw, "body": v2.body },
    });
    let artifact_path = target_dir().join(ARTIFACT_REL_PATH);
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&artifact).expect("the artifact always serializes"),
    )
    .unwrap_or_else(|error| panic!("could not write {}: {error}", artifact_path.display()));

    assert_example_answer("v1 (2025-11-25)", &v1);
    assert_example_answer("v2 (2026-07-28)", &v2);

    // The v2 leg must genuinely have been served as v2, or "both eras" is a
    // claim about a request that was silently downgraded. `resultType` is
    // Phase 112's v2-only result-envelope key.
    assert!(
        v2.raw.contains("resultType"),
        "the v2 leg carries no v2 result-envelope key, so it was not served as v2: {}",
        v2.raw
    );
    assert!(
        !v1.raw.contains("resultType"),
        "the v1 leg carries a v2 result-envelope key, so the per-request era gate was \
         bypassed: {}",
        v1.raw
    );

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
