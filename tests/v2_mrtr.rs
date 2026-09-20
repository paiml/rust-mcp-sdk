//! Phase 113-11 (HTTP-02 / HTTP-03 / CLNT-02): the Rust mirror of every `sep-2322`
//! conformance scenario, plus the real-client interoperability proof.
//!
//! # Why this file exists
//!
//! Phase 118 owns the official Node `@modelcontextprotocol/conformance` suite in CI.
//! Phase 113 has to be self-verifying WITHOUT a Node toolchain, so every `sep-2322`
//! check id emitted by the PINNED suite commit is mirrored here as a native Rust
//! integration test. Phase 118 then inherits a codebase that already passes rather
//! than discovering the gaps late.
//!
//! The inventory is
//! `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-CONFORMANCE-MANIFEST.md`,
//! generated from `113-SPEC-RECHECK.md` § B (23 check ids across 14 scenario classes
//! at conformance pin `a865118206d4d8cc8dbc5f5201607839281d0c3b`) and NOT from the
//! `113-RESEARCH.md` table, which omits four ids and mis-keys a scenario CLASS name
//! as a check id. [`manifest_maps_every_pinned_scenario`] is the enforcement: an
//! upstream scenario with no local mirror is a test FAILURE, not a silent omission.
//!
//! # Two halves, deliberately different
//!
//! * **Scenario mirrors** drive the wire with raw `post`, so every assertion is on
//!   actual bytes and concrete JSON paths — a `pmcp::Client` in the middle would
//!   make the test pass whenever client and server share a bug.
//! * **Interoperability** (`client_server_mrtr_*`) drives the SAME fixture server
//!   with a real `pmcp::Client`. Until now the server half (plans 06/09) and the
//!   client half (plans 05/07) had each only been tested against a hand-built
//!   counterpart.
//!
//! Plan 06's `requestState` verdict table lives in `tests/v2_mrtr_ingress.rs` and is
//! NOT duplicated here; this file covers the round-trip scenarios.
//!
//! # Keys come from the BUILDER, never from the environment
//!
//! Every server here is built with
//! [`ServerBuilder::with_request_state_key`](pmcp::ServerBuilder::with_request_state_key),
//! so no test mutates the `requestState` key environment variable — a process-global
//! whose value is order-dependent under `cargo test`'s in-process parallel threads.
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`): EPHEMERAL
//! PORT (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()` binds
//! before returning), SHUTDOWN (`JoinHandle::abort()` after each round trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use common::v2::{
    post, spawn_default_config, spawn_stateless_config, spawn_with, v1_body, v2_body,
    v2_body_with_caps, v2_headers, Resp, V1, V2,
};
use pmcp::client::host::{HostElicitationHandler, HostSamplingHandler};
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpRequest,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::testing::{open_request_state, ANONYMOUS_PRINCIPAL};
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::types::mrtr::{
    InputRequest, InputRequests, InputResponse, MrtrSignal, MRTR_SIGNAL_META_KEY,
};
use pmcp::types::protocol::error_codes::{
    INTERNAL_ERROR, INVALID_PARAMS, MISSING_REQUIRED_CLIENT_CAPABILITY,
};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::roots::ListRootsResult;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use pmcp::types::{Content, GetPromptResult, ListResourcesResult, MrtrOutcome, ReadResourceResult};
use pmcp::{ClientBuilder, RequestHandlerExtra, ServerCapabilities, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use url::Url;

// ===========================================================================
// Fixture vocabulary.
// ===========================================================================

/// The `requestState` key every fixture server in this file is BUILT with.
const KEY: [u8; 32] = [0x2b; 32];

/// Marker the scripted handlers put in an INCOMPLETE result's payload.
const NEED_INPUT: &str = "need-input";

/// Marker the scripted handlers put in a RESUMED result's payload.
const RESUMED: &str = "resumed";

/// Tool that asks for one `elicitation/create` input, then completes.
const TOOL_ELICIT: &str = "elicit_once";

/// Tool that asks for one `sampling/createMessage` input, then completes.
const TOOL_SAMPLE: &str = "sample_once";

/// Tool that asks for one `roots/list` input, then completes.
const TOOL_ROOTS: &str = "roots_once";

/// Tool that asks for all three kinds in ONE result, then completes.
const TOOL_MIXED: &str = "mixed_kinds";

/// Tool that needs THREE rounds, evolving its continuation each time.
const TOOL_THREE_ROUNDS: &str = "three_rounds";

/// Tool that asks for TWO entries and re-requests whichever is still missing.
const TOOL_TWO_ENTRIES: &str = "two_entries";

/// Tool that NEVER completes — it re-elicits forever (round-limit fixture).
const TOOL_FOREVER: &str = "never_completes";

/// Tool that asks for an `elicitation/create` under [`KIND_STRICT_KEY`] and
/// completes only once the answer arrives TYPED as an elicitation (D-113-O).
const TOOL_KIND_STRICT: &str = "kind_strict";

/// The single `inputRequests` key [`TOOL_KIND_STRICT`] asks under — the literal
/// `"k"` of D-113-O's failure narrative.
const KIND_STRICT_KEY: &str = "k";

/// Tool that needs as many rounds as the SHIPPED client default allows, then
/// completes — the legitimate-deep-flow fixture for the server round ceiling.
const TOOL_CLIENT_DEFAULT_DEPTH: &str = "client_default_depth";

/// Tool that always fails, so a genuine protocol error can be observed.
const TOOL_BOOM: &str = "always_fails";

/// The prompt that participates in MRTR.
const PROMPT_NAME: &str = "greeting";

/// The resource that participates in MRTR.
const RESOURCE_URI: &str = "mem://mrtr";

/// The two keys [`Script::TwoEntries`] insists on before it completes.
const TWO_ENTRY_KEYS: [&str; 2] = ["first", "second"];

/// How many elicitation rounds [`Script::ThreeRounds`] performs.
const TOTAL_ROUNDS: u64 = 3;

/// `DEFAULT_MRTR_ROUND_LIMIT` from `src/client/mod.rs` — the shipped client-side
/// D-09 bound, which is private, so it is mirrored here.
///
/// Not a comment-pinned duplicate:
/// [`a_flow_within_the_client_default_limit_is_unaffected`] MEASURES the MRTR
/// round it reaches by opening each minted token with the server's own key, so
/// this value is checked against live behaviour rather than asserted about.
const CLIENT_DEFAULT_ROUND_LIMIT: u64 = 8;

/// `MAX_MRTR_ROUNDS` from `src/server/core.rs` — the SERVER-side ceiling closed
/// by D-113-L. `pub(crate)`, so an integration test cannot name it.
///
/// Also not a comment-pinned duplicate:
/// [`a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server`]
/// measures the ceiling by resending until the server refuses and asserts the
/// measurement equals this value. Lowering `MAX_MRTR_ROUNDS` without updating
/// this constant is a test FAILURE, which is the tripwire — and the boundary is
/// pinned by name in `core.rs`'s own unit tests, so the two halves cannot both
/// drift silently.
const SERVER_MAX_MRTR_ROUNDS: u64 = 16;

/// The server ceiling is deliberately TWICE the shipped client default, so a
/// default-configured pmcp client can never trip it (D-113-L). Asserting the
/// relationship here means a future edit to either constant has to confront it.
const _: () = assert!(SERVER_MAX_MRTR_ROUNDS == CLIENT_DEFAULT_ROUND_LIMIT * 2);

// ===========================================================================
// Scripted handlers.
// ===========================================================================

/// What a scripted handler does when it is invoked.
#[derive(Clone, Copy, Debug)]
enum Script {
    /// Ask for one `elicitation/create` input, then complete.
    Elicit,
    /// Ask for one `sampling/createMessage` input, then complete.
    Sample,
    /// Ask for one `roots/list` input, then complete.
    Roots,
    /// Ask for all three kinds at once, then complete.
    Mixed,
    /// Ask three times, evolving the continuation each round.
    ThreeRounds,
    /// Ask [`CLIENT_DEFAULT_ROUND_LIMIT`] times, evolving the continuation each
    /// round — a legitimate DEEP flow, at exactly the depth the shipped client
    /// default reaches.
    ClientDefaultDepth,
    /// Ask for two named entries, re-requesting whichever is still missing.
    TwoEntries,
    /// Never complete.
    Forever,
    /// Always fail with a protocol error.
    Boom,
    /// Ask for ONE `elicitation/create` under [`KIND_STRICT_KEY`] and complete
    /// only when the answer arrives typed as [`InputResponse::Elicitation`]
    /// (D-113-O).
    KindStrict,
}

/// One `inputRequests` map holding a single entry.
fn one(key: &str, request: InputRequest) -> InputRequests {
    let mut requests = InputRequests::new();
    requests.insert(key.to_string(), request);
    requests
}

/// An `elicitation/create` entry, built through the shipped authoring type.
fn elicit_entry(message: &str) -> InputRequest {
    InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
        message: message.to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": { "value": { "type": "string" } },
        }),
    }))
}

/// A `sampling/createMessage` entry, decoded from the wire shape so this file
/// cannot drift from `CreateMessageParams`' field set.
fn sampling_entry() -> InputRequest {
    decode_entry(json!({
        "method": "sampling/createMessage",
        "params": { "messages": [], "maxTokens": 16 },
    }))
}

/// A `roots/list` entry.
fn roots_entry() -> InputRequest {
    decode_entry(json!({ "method": "roots/list" }))
}

/// Decode an `inputRequests` VALUE into the typed entry.
fn decode_entry(value: Value) -> InputRequest {
    serde_json::from_value(value).expect("the input-request entry decodes")
}

/// The keys the client answered on this invocation.
fn answered_keys(extra: &RequestHandlerExtra) -> Vec<String> {
    extra
        .input_responses()
        .map(|responses| responses.keys().cloned().collect())
        .unwrap_or_default()
}

/// The ENTIRE server-side MRTR authoring surface: build the requests you need
/// answered, attach the continuation that lets you resume, and put the returned
/// pair on the result's `_meta`.
fn signal_meta(
    input_requests: InputRequests,
    continuation: Value,
) -> pmcp::Result<serde_json::Map<String, Value>> {
    let (key, value) = MrtrSignal {
        input_requests,
        continuation,
    }
    .into_meta_entry()
    .map_err(|error| pmcp::Error::internal(error.to_string()))?;
    let mut meta = serde_json::Map::new();
    meta.insert(key, value);
    Ok(meta)
}

/// Signal `input_required` from a TOOL handler (the payload dispatch path).
fn ask(
    extra: &RequestHandlerExtra,
    input_requests: InputRequests,
    continuation: Value,
) -> pmcp::Result<Value> {
    extra.set_result_meta(signal_meta(input_requests, continuation)?);
    Ok(json!({ "answer": NEED_INPUT }))
}

/// A tool whose behavior is fixed by its [`Script`].
#[derive(Clone)]
struct ScriptedTool {
    script: Script,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ToolHandler for ScriptedTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.script {
            Script::Boom => Err(pmcp::Error::validation("this tool always fails")),
            Script::ThreeRounds => counting_rounds(&extra, TOTAL_ROUNDS),
            Script::ClientDefaultDepth => counting_rounds(&extra, CLIENT_DEFAULT_ROUND_LIMIT),
            Script::TwoEntries => two_entries(&extra),
            Script::Forever => ask(
                &extra,
                one("again", elicit_entry("and again")),
                json!({ "forever": true }),
            ),
            Script::KindStrict => kind_strict(&extra),
            other => single_kind(&extra, other),
        }
    }
}

/// One round of elicit-then-complete, for whichever single kind the script names.
fn single_kind(extra: &RequestHandlerExtra, script: Script) -> pmcp::Result<Value> {
    if extra.mrtr_continuation().is_some() {
        return Ok(json!({ "answer": RESUMED, "answered": answered_keys(extra) }));
    }
    let requests = match script {
        Script::Sample => one("model_says", sampling_entry()),
        Script::Roots => one("workspace", roots_entry()),
        Script::Mixed => {
            let mut requests = one("who", elicit_entry("Who is asking?"));
            requests.insert("model_says".to_string(), sampling_entry());
            requests.insert("workspace".to_string(), roots_entry());
            requests
        },
        _ => one("user_name", elicit_entry("What is your name?")),
    };
    ask(extra, requests, json!({ "step": 1 }))
}

/// `total` rounds with an EVOLVING continuation: `round` counts up inside the
/// sealed `requestState`, so each round's token is minted from different state.
///
/// Parameterised over `total` so the three-round scenario mirror and the
/// deep-flow fixture that guards the server round ceiling share ONE shape — two
/// hand-written variants would be free to drift into proving different things.
fn counting_rounds(extra: &RequestHandlerExtra, total: u64) -> pmcp::Result<Value> {
    let round = extra
        .mrtr_continuation()
        .and_then(|state| state.get("round"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if round >= total {
        return Ok(json!({ "answer": RESUMED, "rounds": round }));
    }
    let next = round + 1;
    ask(
        extra,
        one(&format!("q{next}"), elicit_entry("one more, please")),
        json!({ "round": next }),
    )
}

/// Ask for [`TWO_ENTRY_KEYS`], accumulating what has been answered in the sealed
/// continuation and RE-REQUESTING whatever is still missing (server obligation 9).
fn two_entries(extra: &RequestHandlerExtra) -> pmcp::Result<Value> {
    let mut seen: Vec<String> = extra
        .mrtr_continuation()
        .and_then(|state| state.get("answered"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    for key in answered_keys(extra) {
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    let missing: Vec<String> = TWO_ENTRY_KEYS
        .iter()
        .filter(|key| !seen.iter().any(|answered| answered == *key))
        .map(|key| (*key).to_string())
        .collect();
    if missing.is_empty() {
        return Ok(json!({ "answer": RESUMED, "answered": seen }));
    }
    let mut requests = InputRequests::new();
    for key in &missing {
        requests.insert(key.clone(), elicit_entry("still needed"));
    }
    ask(extra, requests, json!({ "answered": seen }))
}

/// D-113-O's handler, written exactly as the deferred item narrates it: ask for
/// an ELICITATION under `"k"`, then MATCH ON THE VARIANT the SDK typed the answer
/// as. The `Elicitation` arm completes; anything else falls through and
/// re-elicits.
///
/// This is what a handler that trusts `InputResponse`'s typing looks like — and
/// trusting it is the documented contract, since `InputRequest::kind()` and
/// `InputResponse::decode_for` exist precisely so a handler need not re-inspect
/// raw JSON. A server that types the answer by SHAPE rather than by the kind it
/// requested hands this handler the wrong variant, the match falls through, and
/// the operation loops with no error raised anywhere.
fn kind_strict(extra: &RequestHandlerExtra) -> pmcp::Result<Value> {
    let answered_as_elicitation = extra
        .input_responses()
        .and_then(|responses| responses.get(KIND_STRICT_KEY))
        .is_some_and(|response| matches!(response, InputResponse::Elicitation(_)));
    if answered_as_elicitation {
        return Ok(json!({ "answer": RESUMED }));
    }
    ask(
        extra,
        one(KIND_STRICT_KEY, elicit_entry("What is your name?")),
        json!({ "step": 1 }),
    )
}

/// A prompt that asks once and then resumes — the `prompts/get` leg of
/// `sep-2322-non-tool-*`.
struct MrtrPrompt;

#[async_trait]
impl PromptHandler for MrtrPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        if extra.mrtr_continuation().is_some() {
            return Ok(GetPromptResult::new(vec![], Some(RESUMED.to_string())));
        }
        let meta = signal_meta(
            one("user_name", elicit_entry("Who should I greet?")),
            json!({ "step": 1 }),
        )?;
        Ok(GetPromptResult::new(vec![], Some(NEED_INPUT.to_string())).with_meta(meta))
    }
}

/// A resource that asks once and then resumes — the `resources/read` leg of
/// `sep-2322-non-tool-*`. `ReadResourceResult._meta` is the third leg of the
/// authoring surface, added by plan 113-09.
struct MrtrResource;

#[async_trait]
impl ResourceHandler for MrtrResource {
    async fn read(
        &self,
        uri: &str,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        let resuming = extra.mrtr_continuation().is_some();
        let text = if resuming { RESUMED } else { NEED_INPUT };
        let mut result = ReadResourceResult::new(vec![Content::resource_with_text(
            uri.to_string(),
            text.to_string(),
            "text/plain".to_string(),
        )]);
        if !resuming {
            let meta = signal_meta(
                one("user_name", elicit_entry("Who is reading?")),
                json!({ "step": 1 }),
            )?;
            result._meta = Some(Value::Object(meta));
        }
        Ok(result)
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![]))
    }
}

// ===========================================================================
// Spawning.
// ===========================================================================

/// A v2-opted-in server exposing every scripted tool plus the MRTR prompt and
/// resource, holding [`KEY`] and no auth provider (so the AAD principal is the
/// anonymous one).
fn build_fixture_server(calls: &Arc<AtomicUsize>) -> Server {
    let tool = |script: Script| ScriptedTool {
        script,
        calls: Arc::clone(calls),
    };
    Server::builder()
        .name("v2-mrtr-conformance")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .with_request_state_key(KEY)
        .tool(TOOL_ELICIT, tool(Script::Elicit))
        .tool(TOOL_SAMPLE, tool(Script::Sample))
        .tool(TOOL_ROOTS, tool(Script::Roots))
        .tool(TOOL_MIXED, tool(Script::Mixed))
        .tool(TOOL_THREE_ROUNDS, tool(Script::ThreeRounds))
        .tool(TOOL_TWO_ENTRIES, tool(Script::TwoEntries))
        .tool(TOOL_FOREVER, tool(Script::Forever))
        .tool(TOOL_CLIENT_DEFAULT_DEPTH, tool(Script::ClientDefaultDepth))
        .tool(TOOL_BOOM, tool(Script::Boom))
        .tool(TOOL_KIND_STRICT, tool(Script::KindStrict))
        .prompt(PROMPT_NAME, MrtrPrompt)
        .resources(MrtrResource)
        .build()
        .expect("the fixture server builds")
}

/// Spawn the fixture on the STATEFUL default config, so what makes these round
/// trips session-free is the PER-REQUEST era gate rather than a build-time
/// stateless branch (RESEARCH Pitfall 1).
async fn spawn_fixture() -> (SocketAddr, JoinHandle<()>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let (addr, handle) = spawn_default_config(build_fixture_server(&calls)).await;
    (addr, handle, calls)
}

// ===========================================================================
// Request construction.
// ===========================================================================

/// `tools/call` params, identical between the first call and every retry —
/// `name` and `arguments` are the digest-salient pair the `requestState` AAD
/// binds to, so they must not move.
fn tool_params(name: &str) -> Value {
    json!({ "name": name, "arguments": {} })
}

/// A first-round `tools/call`.
async fn call_tool(addr: SocketAddr, id: i64, name: &str) -> Resp {
    post(
        addr,
        &v2_headers("tools/call", name),
        &v2_body("tools/call", json!(id), tool_params(name)),
    )
    .await
}

/// A retry of ANY MRTR-eligible method, carrying `requestState` and
/// `inputResponses` as TOP-LEVEL `params` siblings — never inside `_meta`.
///
/// Method-agnostic because MRTR is: `prompts/get` and `resources/read` splice the
/// two fields at exactly the same place `tools/call` does, and a helper that
/// baked in `tools/call` is what made the non-tool test inline this block twice.
async fn retry(
    addr: SocketAddr,
    method: &str,
    logical_name: &str,
    id: Value,
    mut params: Value,
    state: &str,
    answers: Value,
) -> Resp {
    let object = params.as_object_mut().expect("params is an object");
    object.insert("requestState".to_string(), json!(state));
    object.insert("inputResponses".to_string(), answers);
    post(
        addr,
        &v2_headers(method, logical_name),
        &v2_body(method, id, params),
    )
    .await
}

/// [`retry`] for `tools/call`, whose params are derived from the tool name.
async fn retry_tool(addr: SocketAddr, id: Value, name: &str, state: &str, answers: Value) -> Resp {
    retry(
        addr,
        "tools/call",
        name,
        id,
        tool_params(name),
        state,
        answers,
    )
    .await
}

/// A well-formed `ElicitResult`.
fn elicit_answer() -> Value {
    json!({ "action": "accept", "content": { "value": "ada" } })
}

/// A well-formed `CreateMessageResult`.
fn sampling_answer() -> Value {
    json!({ "content": { "type": "text", "text": "hello" }, "model": "fixture-model" })
}

/// A well-formed `ListRootsResult`.
fn roots_answer() -> Value {
    json!({ "roots": [] })
}

// ===========================================================================
// Response assertions.
// ===========================================================================

/// The `result` object, or a panic naming the raw body.
fn result_of(response: &Resp) -> &Value {
    response
        .body
        .get("result")
        .unwrap_or_else(|| panic!("expected a result, got: {}", response.raw))
}

/// Assert an `input_required` result and return its `(requestState, keys)`.
fn expect_input_required(response: &Resp) -> (String, Vec<String>) {
    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = result_of(response);
    assert_eq!(
        result.get("resultType").and_then(Value::as_str),
        Some("input_required"),
        "expected an input_required result, got: {result}"
    );
    let state = result["requestState"]
        .as_str()
        .unwrap_or_else(|| panic!("an input_required result must carry a requestState: {result}"))
        .to_string();
    let keys = result["inputRequests"]
        .as_object()
        .unwrap_or_else(|| panic!("an input_required result must carry inputRequests: {result}"))
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        !keys.is_empty(),
        "inputRequests must not be empty: {result}"
    );
    (state, keys)
}

/// Assert a COMPLETE result whose payload carries the resumed marker.
fn expect_resumed(response: &Resp) {
    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = result_of(response);
    assert_eq!(
        result.get("resultType").and_then(Value::as_str),
        Some("complete"),
        "expected a complete result, got: {result}"
    );
    assert!(
        result.get("inputRequests").is_none() && result.get("requestState").is_none(),
        "a complete result carries no MRTR fields: {result}"
    );
    assert!(
        response.raw.contains(RESUMED),
        "the handler must have taken its RESUME branch: {}",
        response.raw
    );
}

// ===========================================================================
// sep-2322 basic kinds: incomplete then complete.
// ===========================================================================

/// `sep-2322-elicitation-incomplete` + `sep-2322-elicitation-complete`.
#[tokio::test]
async fn sep_2322_elicitation_incomplete_then_complete() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let (state, keys) = expect_input_required(&first);
    assert_eq!(keys, vec!["user_name".to_string()]);
    let entry = &result_of(&first)["inputRequests"]["user_name"];
    assert_eq!(
        entry["method"], "elicitation/create",
        "the entry is a full request object: {entry}"
    );

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
    assert!(
        second.raw.contains("user_name"),
        "the handler must have observed the answered key: {}",
        second.raw
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2, "one call per round");
}

/// `sep-2322-sampling-incomplete` + `sep-2322-sampling-complete`.
#[tokio::test]
async fn sep_2322_sampling_incomplete_then_complete() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_SAMPLE).await;
    let (state, keys) = expect_input_required(&first);
    assert_eq!(keys, vec!["model_says".to_string()]);
    assert_eq!(
        result_of(&first)["inputRequests"]["model_says"]["method"],
        "sampling/createMessage"
    );

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_SAMPLE,
        &state,
        json!({ "model_says": sampling_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
}

/// `sep-2322-list-roots-incomplete` + `sep-2322-list-roots-complete`.
#[tokio::test]
async fn sep_2322_list_roots_incomplete_then_complete() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ROOTS).await;
    let (state, keys) = expect_input_required(&first);
    assert_eq!(keys, vec!["workspace".to_string()]);
    assert_eq!(
        result_of(&first)["inputRequests"]["workspace"]["method"],
        "roots/list"
    );

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_ROOTS,
        &state,
        json!({ "workspace": roots_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
}

/// `sep-2322-request-state-incomplete` + `sep-2322-request-state-complete`: the
/// round trip carries BOTH `inputRequests` and `requestState`, and the retry
/// echoing both resumes the handler's SEALED continuation.
#[tokio::test]
async fn sep_2322_request_state_incomplete_then_complete() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let result = result_of(&first);
    assert!(
        result.get("inputRequests").is_some() && result.get("requestState").is_some(),
        "the spec requires at least one; pmcp always emits BOTH: {result}"
    );
    let state = result["requestState"]
        .as_str()
        .expect("requestState is a string")
        .to_string();
    assert!(
        !state.is_empty(),
        "the continuation token must be a non-empty opaque blob"
    );

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
}

// ===========================================================================
// sep-2322 multi-entry and multi-round.
// ===========================================================================

/// `sep-2322-multiple-inputs-incomplete` + `-complete`: several `inputRequests`
/// of DIFFERENT kinds in one result, all answered in ONE retry.
#[tokio::test]
async fn sep_2322_multiple_inputs() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_MIXED).await;
    let (state, mut keys) = expect_input_required(&first);
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "model_says".to_string(),
            "who".to_string(),
            "workspace".to_string()
        ],
        "all three kinds ride in ONE result"
    );
    let requests = &result_of(&first)["inputRequests"];
    assert_eq!(requests["who"]["method"], "elicitation/create");
    assert_eq!(requests["model_says"]["method"], "sampling/createMessage");
    assert_eq!(requests["workspace"]["method"], "roots/list");

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_MIXED,
        &state,
        json!({
            "who": elicit_answer(),
            "model_says": sampling_answer(),
            "workspace": roots_answer(),
        }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "one retry, not three");
}

/// `sep-2322-multi-round-r1` + `-r2` + `-r3`: THREE rounds with an EVOLVING
/// `requestState`, every token distinct from the previous one.
#[tokio::test]
async fn sep_2322_multi_round() {
    let (addr, handle, calls) = spawn_fixture().await;

    let r1 = call_tool(addr, 1, TOOL_THREE_ROUNDS).await;
    let (state1, keys1) = expect_input_required(&r1);
    assert_eq!(keys1, vec!["q1".to_string()]);

    let r2 = retry_tool(
        addr,
        json!(2),
        TOOL_THREE_ROUNDS,
        &state1,
        json!({ "q1": elicit_answer() }),
    )
    .await;
    let (state2, keys2) = expect_input_required(&r2);
    assert_eq!(keys2, vec!["q2".to_string()], "the ask EVOLVED");

    let r3 = retry_tool(
        addr,
        json!(3),
        TOOL_THREE_ROUNDS,
        &state2,
        json!({ "q2": elicit_answer() }),
    )
    .await;
    let (state3, keys3) = expect_input_required(&r3);
    assert_eq!(keys3, vec!["q3".to_string()]);

    let r4 = retry_tool(
        addr,
        json!(4),
        TOOL_THREE_ROUNDS,
        &state3,
        json!({ "q3": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&r4);
    assert_ne!(state1, state2, "round 2's token must differ from round 1's");
    assert_ne!(state2, state3, "round 3's token must differ from round 2's");
    assert_ne!(state1, state3, "no token may repeat across the exchange");
    assert_eq!(calls.load(Ordering::SeqCst), 4, "three asks, then a resume");
}

/// `sep-2322-missing-response-rerequests`: under-supplied `inputResponses`
/// produce a NEW `input_required` re-requesting ONLY the missing entry — never
/// an error.
#[tokio::test]
async fn sep_2322_missing_response_rerequests() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_TWO_ENTRIES).await;
    let (state1, mut keys1) = expect_input_required(&first);
    keys1.sort();
    assert_eq!(keys1, vec!["first".to_string(), "second".to_string()]);

    // Answer only ONE of the two.
    let second = retry_tool(
        addr,
        json!(2),
        TOOL_TWO_ENTRIES,
        &state1,
        json!({ "first": elicit_answer() }),
    )
    .await;
    let (state2, keys2) = expect_input_required(&second);
    assert!(
        second.body.get("error").is_none(),
        "an under-supplied retry must RE-REQUEST, not error: {}",
        second.raw
    );
    assert_eq!(
        keys2,
        vec!["second".to_string()],
        "only the MISSING entry is re-requested"
    );
    assert_ne!(state1, state2, "the re-request carries a fresh token");

    let third = retry_tool(
        addr,
        json!(3),
        TOOL_TWO_ENTRIES,
        &state2,
        json!({ "second": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&third);
}

// ===========================================================================
// sep-2322 non-tool methods.
// ===========================================================================

/// `sep-2322-non-tool-incomplete` + `-complete`, on `prompts/get` AND on
/// `resources/read`.
#[tokio::test]
async fn sep_2322_non_tool_incomplete_then_complete() {
    let (addr, handle, _calls) = spawn_fixture().await;

    // --- prompts/get ---
    let prompt_params = json!({ "name": PROMPT_NAME, "arguments": {} });
    let prompt_first = post(
        addr,
        &v2_headers("prompts/get", PROMPT_NAME),
        &v2_body("prompts/get", json!(1), prompt_params.clone()),
    )
    .await;
    let (prompt_state, prompt_keys) = expect_input_required(&prompt_first);
    assert_eq!(prompt_keys, vec!["user_name".to_string()]);

    let prompt_second = retry(
        addr,
        "prompts/get",
        PROMPT_NAME,
        json!(2),
        prompt_params,
        &prompt_state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    expect_resumed(&prompt_second);

    // --- resources/read ---
    let resource_params = json!({ "uri": RESOURCE_URI });
    let resource_first = post(
        addr,
        &v2_headers("resources/read", RESOURCE_URI),
        &v2_body("resources/read", json!(3), resource_params.clone()),
    )
    .await;
    let (resource_state, resource_keys) = expect_input_required(&resource_first);
    assert_eq!(resource_keys, vec!["user_name".to_string()]);

    let resource_second = retry(
        addr,
        "resources/read",
        RESOURCE_URI,
        json!(4),
        resource_params,
        &resource_state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&resource_second);
}

// ===========================================================================
// sep-2322 envelope and confinement.
// ===========================================================================

/// `sep-2322-result-type-included`: `resultType` is EXPLICITLY present with the
/// right value on BOTH legs — never inferred from the presence of
/// `inputRequests`.
#[tokio::test]
async fn sep_2322_result_type_included() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let incomplete = result_of(&first);
    assert!(
        incomplete.get("resultType").is_some(),
        "the KEY itself must be present: {incomplete}"
    );
    assert_eq!(incomplete["resultType"], "input_required");
    let state = incomplete["requestState"]
        .as_str()
        .expect("requestState")
        .to_string();

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    let complete = result_of(&second);
    assert_eq!(complete["resultType"], "complete");

    // ...and a method that never participates in MRTR still states it.
    let listed = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(3), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(result_of(&listed)["resultType"], "complete");
}

/// `sep-2322-not-on-unsupported-requests`: `input_required` NEVER appears on a
/// method outside `tools/call` / `prompts/get` / `resources/read`.
///
/// Two halves, both asserted at the wire level:
/// 1. MRTR fields presented on `tools/list` are INERT — a normal complete result;
/// 2. the same signalling tool invoked where MRTR is impossible (here: on v1)
///    fails LOUDLY with `-32603`, which is the behavior plan 113-09 shipped —
///    never a mangled "complete" and never a leaked internal signal.
#[tokio::test]
async fn sep_2322_not_on_unsupported_requests() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let listed = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body(
            "tools/list",
            json!(1),
            json!({
                "requestState": "not-even-a-real-token",
                "inputResponses": { "user_name": elicit_answer() },
            }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(listed.status, 200, "body was {}", listed.raw);
    let result = result_of(&listed);
    assert_eq!(result["resultType"], "complete");
    assert!(result["tools"].is_array(), "the listing is the real one");
    assert!(
        result.get("inputRequests").is_none() && result.get("requestState").is_none(),
        "no MRTR field may appear on a non-eligible method's result: {result}"
    );

    // A signal where MRTR is IMPOSSIBLE is a server bug, and plan 09 made it
    // loud. The stateless config is used only so the absence of a v1 session is
    // not the variable under test.
    let calls = Arc::new(AtomicUsize::new(0));
    let (v1_addr, v1_handle) = spawn_stateless_config(build_fixture_server(&calls)).await;
    let v1 = post(
        v1_addr,
        &[],
        &v1_body("tools/call", json!(1), tool_params(TOOL_ELICIT)),
    )
    .await;
    v1_handle.abort();

    assert_eq!(
        v1.body["error"]["code"], INTERNAL_ERROR,
        "a signal on v1 must fail loudly, got: {}",
        v1.raw
    );
    assert!(
        v1.body.get("result").is_none(),
        "no result may accompany the failure: {}",
        v1.raw
    );
    assert!(
        !v1.raw.contains(MRTR_SIGNAL_META_KEY),
        "the internal signal key must never reach the wire: {}",
        v1.raw
    );
}

/// `sep-2322-reject-tampered-state`: a tampered `requestState` is a JSON-RPC
/// error — never a complete result and never a re-prompt.
///
/// Distinct from `tests/v2_mrtr_ingress.rs::tampered_state_errors`, which mints
/// its token through the testing seam: this one tampers with a token the SERVER
/// actually minted during a live round trip, which is the conformance shape.
#[tokio::test]
async fn sep_2322_reject_tampered_state() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let (state, _keys) = expect_input_required(&first);

    let tampered = retry_tool(
        addr,
        json!(2),
        TOOL_ELICIT,
        &format!("{state}-TAMPERED"),
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    assert_eq!(
        tampered.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        tampered.raw
    );
    assert!(
        tampered.body.get("result").is_none(),
        "neither a complete result nor a re-prompt: {}",
        tampered.raw
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only the first round reached the handler"
    );
}

/// `sep-2322-respect-client-capabilities` (scenario class
/// `input-required-result-capability-check`): a server that needs a capability
/// the client did not declare answers `-32021` at HTTP 400 with an
/// OBJECT-shaped `data.requiredCapabilities`.
///
/// The under-declaration is DELIBERATE — built with `v2_body_with_caps` rather
/// than by omitting the harness default — so this cannot pass by accident.
#[tokio::test]
async fn input_required_result_capability_check() {
    let (addr, handle, calls) = spawn_fixture().await;

    let response = post(
        addr,
        &v2_headers("tools/call", TOOL_SAMPLE),
        &v2_body_with_caps(
            "tools/call",
            json!(1),
            tool_params(TOOL_SAMPLE),
            json!({ "elicitation": {}, "roots": {} }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 400, "body was {}", response.raw);
    assert_eq!(
        response.body["error"]["code"], MISSING_REQUIRED_CLIENT_CAPABILITY,
        "body was {}",
        response.raw
    );
    let required = &response.body["error"]["data"]["requiredCapabilities"];
    assert!(
        required.is_object(),
        "requiredCapabilities is a ClientCapabilities OBJECT, never an array or a \
         list of strings: {required}"
    );
    assert!(
        required.get("sampling").is_some(),
        "the undeclared capability must be named: {required}"
    );
    assert!(
        response.body.get("result").is_none(),
        "the whole result is refused, all-or-nothing: {}",
        response.raw
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the handler ran; the REFUSAL happened at egress, before any minting"
    );
}

/// `sep-2322-ignore-unexpected-params`: unexpected/extra `params` are TOLERATED
/// rather than erroring — on the first call AND on the retry.
#[tokio::test]
async fn sep_2322_ignore_unexpected_params() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let mut params = tool_params(TOOL_ELICIT);
    let object = params.as_object_mut().expect("params is an object");
    object.insert(
        "unexpectedField".to_string(),
        json!({ "anything": [1, 2, 3] }),
    );
    object.insert("anotherOne".to_string(), json!("surplus"));

    let first = post(
        addr,
        &v2_headers("tools/call", TOOL_ELICIT),
        &v2_body("tools/call", json!(1), params.clone()),
    )
    .await;
    let (state, _keys) = expect_input_required(&first);

    let object = params.as_object_mut().expect("params is an object");
    object.insert("requestState".to_string(), json!(state));
    object.insert(
        "inputResponses".to_string(),
        json!({ "user_name": elicit_answer() }),
    );
    let second = post(
        addr,
        &v2_headers("tools/call", TOOL_ELICIT),
        &v2_body("tools/call", json!(2), params),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
}

/// `sep-2322-validate-input-responses`: a structurally invalid `inputResponses`
/// map is REJECTED before the handler runs — absent is never conflated with
/// invalid.
#[tokio::test]
async fn sep_2322_validate_input_responses() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let (state, _keys) = expect_input_required(&first);

    let invalid = retry_tool(
        addr,
        json!(2),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": "not a result object at all" }),
    )
    .await;
    handle.abort();

    assert_eq!(invalid.status, 400, "body was {}", invalid.raw);
    assert_eq!(invalid.body["error"]["code"], INVALID_PARAMS);
    assert!(
        invalid.body.get("result").is_none(),
        "an invalid map must not produce a result: {}",
        invalid.raw
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "validation happens BEFORE the handler is invoked"
    );
}

/// `sep-2322-error-on-protocol-error`: a genuine protocol error surfaces as a
/// JSON-RPC error, not as a re-prompt.
#[tokio::test]
async fn sep_2322_error_on_protocol_error() {
    let (addr, handle, _calls) = spawn_fixture().await;

    // (a) a handler that fails.
    let boom = call_tool(addr, 1, TOOL_BOOM).await;
    assert!(
        boom.body.get("error").is_some() || result_of(&boom).get("isError").is_some(),
        "a failing handler must surface as an error, got: {}",
        boom.raw
    );
    assert!(
        !boom.raw.contains("input_required"),
        "an error must never be dressed up as a re-prompt: {}",
        boom.raw
    );

    // (b) a request naming a tool that does not exist.
    let unknown = call_tool(addr, 2, "no-such-tool").await;
    handle.abort();

    assert!(
        unknown.body.get("error").is_some() || result_of(&unknown).get("isError").is_some(),
        "an unknown tool is a protocol error, got: {}",
        unknown.raw
    );
    assert!(
        !unknown.raw.contains("input_required"),
        "...and never a re-prompt: {}",
        unknown.raw
    );
}

// ===========================================================================
// The SERVER-side round ceiling (D-113-L, HTTP-02 / HTTP-03).
//
// Both tests here drive the wire with RAW frames on purpose. `pmcp::Client`
// honours its own `mrtr_round_limit`, so a Client on both ends would prove only
// that two peers sharing an assumption agree with each other — which is exactly
// the state D-113-L found the SDK in: the D-09 counter was enforced solely by
// the party it exists to constrain. The loop below is the party that does not
// cooperate.
// ===========================================================================

/// Open a minted `requestState` with the fixture server's own key and report the
/// round sealed inside it.
///
/// The round is INSIDE the AEAD, so this is the only way a test can observe what
/// the server actually minted rather than what it hoped was minted.
fn sealed_round(tool: &str, state: &str) -> u8 {
    let (_, round) = open_request_state(
        &KEY,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &tool_params(tool),
        state,
    )
    .expect("a token the fixture server minted verifies against its own key");
    round
}

/// A client that ignores its own round limit and resends forever is stopped by
/// the SERVER (D-113-L, T-113-110/111/114).
///
/// Four things are asserted, and only together do they mean anything:
///
/// 1. the loop TERMINATES — before D-113-L was closed it did not;
/// 2. the terminating frame is a typed `-32602` at HTTP 400 whose message names
///    the round limit, not the generic `invalid requestState` used for
///    authentication failures;
/// 3. the handler ran exactly `SERVER_MAX_MRTR_ROUNDS` times — the refusing
///    round did NOT reach it, which is the property the INGRESS enforcement
///    point owns and the mint-site backstop cannot provide;
/// 4. the deepest round the server ever sealed is the ceiling, so the counter
///    never approaches the 255 saturation that made
///    `RequestHandlerExtra::mrtr_round` useless to a self-limiting handler.
#[tokio::test]
async fn a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server() {
    let (addr, handle, calls) = spawn_fixture().await;

    // `TOOL_FOREVER` never completes, so nothing but the server's ceiling can
    // end this exchange.
    let first = call_tool(addr, 1, TOOL_FOREVER).await;
    let (mut state, _) = expect_input_required(&first);
    let mut deepest = sealed_round(TOOL_FOREVER, &state);

    // Three times the ceiling: far enough past it that a regression producing a
    // saturating counter would be visible, and bounded so a regression fails
    // FAST instead of hanging the suite.
    let attempts = SERVER_MAX_MRTR_ROUNDS * 3;
    let mut resends = 0_u64;
    let mut refusal = None;
    for id in 2..=(attempts + 1) {
        resends += 1;
        let response = retry_tool(
            addr,
            json!(id),
            TOOL_FOREVER,
            &state,
            json!({ "again": elicit_answer() }),
        )
        .await;
        if response.body.get("error").is_some() {
            refusal = Some(response);
            break;
        }
        let (next, _) = expect_input_required(&response);
        deepest = deepest.max(sealed_round(TOOL_FOREVER, &next));
        state = next;
    }
    handle.abort();

    let refusal =
        refusal.expect("the server must terminate an unbounded resend loop, not follow it");
    assert_eq!(refusal.status, 400, "body was {}", refusal.raw);
    assert_eq!(
        refusal.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        refusal.raw
    );
    let message = refusal.body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("a JSON-RPC error carries a message: {}", refusal.raw));
    assert!(
        message.contains("round limit"),
        "the refusal must NAME the ceiling — it happens after the token verified, so it \
         is not an authentication oracle and has nothing to hide: {message}"
    );
    assert_ne!(
        message, "invalid requestState",
        "and it must not hide behind the generic authentication message"
    );
    assert!(
        refusal.body.get("result").is_none(),
        "neither a complete result nor a re-prompt: {}",
        refusal.raw
    );

    assert_eq!(
        resends, SERVER_MAX_MRTR_ROUNDS,
        "the ceiling is measured, not assumed: the server admitted exactly this many \
         resends before refusing"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst) as u64,
        SERVER_MAX_MRTR_ROUNDS,
        "the refused round must NOT have reached the handler — the first call plus \
         `ceiling - 1` admitted resends is exactly `ceiling` invocations"
    );
    assert_eq!(
        u64::from(deepest),
        SERVER_MAX_MRTR_ROUNDS,
        "the deepest round the server ever sealed is the ceiling itself — nowhere near \
         the 255 saturation D-113-L described"
    );
}

// ===========================================================================
// Kind-directed typing at ingress (D-113-O, HTTP-03).
//
// Raw frames again, and for a sharper reason than the round-ceiling tests
// above: `pmcp::Client` builds its `inputResponses` from the very
// `inputRequests` the server sent, through `InputResponse::decode_for`. A
// Client on both ends therefore CANNOT produce a mismatched answer — it is
// structurally incapable of the thing under test. That is exactly the shape
// D-113-O found the SDK in: the kind-directed guarantee held on the client, and
// only on the client, so no test involving one could ever have caught it.
// ===========================================================================

/// The overlapping answer of D-113-O: `action` (making it an `ElicitResult`)
/// AND `content` + `model` (making it a `CreateMessageResult`).
///
/// `try_from_value_untagged` tries Sampling before Elicitation, so this is the
/// value the server used to reclassify.
fn overlapping_answer() -> Value {
    json!({
        "action": "accept",
        "content": { "type": "text", "text": "hello" },
        "model": "attacker-chosen-model",
    })
}

/// An answer that can ONLY be a `CreateMessageResult`: dropping `action` makes
/// it undecodable as an `ElicitResult`, whose `action` has no default.
fn sampling_only_answer() -> Value {
    json!({
        "content": { "type": "text", "text": "hello" },
        "model": "attacker-chosen-model",
    })
}

/// D-113-O's loop TERMINATES: the overlapping answer is typed as the elicitation
/// it actually is, the handler's `Elicitation` arm matches, and the operation
/// completes on the FIRST resend.
///
/// # What this looked like before the fix
///
/// Recorded here because the test's value is the contrast. The server typed the
/// answer by shape, `Sampling` matched first, `kind_strict`'s `Elicitation` arm
/// fell through, and it re-elicited. The client answered identically, forever.
/// Measured against the tree at `9a7024cd`, the exchange ran **16 resends and 16
/// handler invocations** before dying on a MISLEADING
/// `-32602 "this request exceeded the server's multi-round-trip round limit"` —
/// an error blaming the client's round count for a mistyped answer on round one.
///
/// That 16 is plan 113-24's `MAX_MRTR_ROUNDS`, and the composition is worth
/// stating: the round ceiling turned an infinite loop into a bounded but still
/// wrong one, and this plan turns it into a correct outcome. Neither fix
/// subsumes the other — 113-24 bounds a loop it cannot diagnose, and this one
/// removes the cause.
#[tokio::test]
async fn the_literal_d113o_answer_completes_instead_of_looping() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_KIND_STRICT).await;
    let (state, keys) = expect_input_required(&first);
    assert_eq!(keys, vec![KIND_STRICT_KEY.to_string()]);

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_KIND_STRICT,
        &state,
        json!({ KIND_STRICT_KEY: overlapping_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the handler ran exactly twice — the call that asked, and the resend that \
         answered. Any more means it re-elicited, which is the defect."
    );
}

/// An answer that CANNOT be the requested kind is refused with a typed
/// `INVALID_PARAMS` naming the key, and the handler is never invoked on the
/// refused round.
///
/// The refusal is the half of D-113-O that could not be expressed at all before
/// the kinds travelled: ingress had nothing to compare the answer against, so
/// there was no shape it could call wrong.
#[tokio::test]
async fn an_answer_that_cannot_be_the_requested_kind_is_refused() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_KIND_STRICT).await;
    let (state, _keys) = expect_input_required(&first);

    let refusal = retry_tool(
        addr,
        json!(2),
        TOOL_KIND_STRICT,
        &state,
        json!({ KIND_STRICT_KEY: sampling_only_answer() }),
    )
    .await;
    handle.abort();

    assert_eq!(refusal.status, 400, "body was {}", refusal.raw);
    assert_eq!(
        refusal.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        refusal.raw
    );
    assert!(
        refusal.body.get("result").is_none(),
        "neither a complete result nor a re-prompt: {}",
        refusal.raw
    );
    let message = refusal.body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("a JSON-RPC error carries a message: {}", refusal.raw));
    assert!(
        message.contains(&format!("{KIND_STRICT_KEY:?}")),
        "the refusal must NAME the key, which is server-assigned and read back out of \
         the sealed continuation: {message}"
    );
    assert!(
        message.contains("elicitation/create"),
        "...and the kind the server actually requested under it: {message}"
    );
    assert!(
        !refusal.raw.contains("attacker-chosen-model"),
        "...and must never echo the attacker-controlled VALUE: {}",
        refusal.raw
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the refusal fires at ingress, so the refused round never reaches the handler"
    );
}

/// The control: an UNAMBIGUOUS `ElicitResult` still completes the round.
///
/// Without this, a fix that refused every answer would pass both tests above —
/// the first by never getting past the elicitation, the second trivially.
#[tokio::test]
async fn a_correctly_shaped_answer_still_completes_the_round() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_KIND_STRICT).await;
    let (state, _keys) = expect_input_required(&first);

    let second = retry_tool(
        addr,
        json!(2),
        TOOL_KIND_STRICT,
        &state,
        json!({ KIND_STRICT_KEY: elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// An answer under a key the server never asked about is refused — and the
/// refusal does NOT echo that key, because it is client-chosen.
#[tokio::test]
async fn an_unsolicited_key_is_refused_without_being_echoed() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_KIND_STRICT).await;
    let (state, _keys) = expect_input_required(&first);

    let unsolicited = "zzz_client_chosen_key_zzz";
    let refusal = retry_tool(
        addr,
        json!(2),
        TOOL_KIND_STRICT,
        &state,
        json!({ unsolicited: elicit_answer() }),
    )
    .await;
    handle.abort();

    assert_eq!(refusal.status, 400, "body was {}", refusal.raw);
    assert_eq!(refusal.body["error"]["code"], INVALID_PARAMS);
    assert!(
        !refusal.raw.contains(unsolicited),
        "an unsolicited key is CLIENT-chosen and bounded only by the 256 KiB \
         inputResponses total, so echoing it would both amplify and poison logs: {}",
        refusal.raw
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "refused at ingress, so the handler never ran on that round"
    );
}

/// BLAST RADIUS: every OTHER MRTR flow in this file is untouched.
///
/// The rejection is wire-visible, so the boundary has to be stated as something
/// checkable rather than asserted. It is: an answer is refused only when it is
/// answered under a key the continuation did not request, or cannot decode as
/// the kind requested there. A client that answers the `inputRequests` it was
/// actually sent — which is all `pmcp::Client` can do — sees no change.
///
/// The three single-kind fixtures exercise all three kinds through the full
/// mint/verify/re-decode path, so a re-decode that got any one of them backwards
/// fails here rather than in a deployment.
#[tokio::test]
async fn well_behaved_answers_of_every_kind_are_unaffected() {
    let (addr, handle, _calls) = spawn_fixture().await;

    for (id, tool, key, answer) in [
        (1_i64, TOOL_ELICIT, "user_name", elicit_answer()),
        (3, TOOL_SAMPLE, "model_says", sampling_answer()),
        (5, TOOL_ROOTS, "workspace", roots_answer()),
    ] {
        let first = call_tool(addr, id, tool).await;
        let (state, keys) = expect_input_required(&first);
        assert_eq!(keys, vec![key.to_string()]);
        let second = retry_tool(addr, json!(id + 1), tool, &state, json!({ key: answer })).await;
        expect_resumed(&second);
    }
    handle.abort();
}

/// A legitimate deep flow — at exactly the depth the shipped client default
/// reaches — is UNAFFECTED by the ceiling.
///
/// This is what makes the 2x headroom a checked property rather than a comment
/// in `core.rs`: a future "tighten the ceiling" change that broke real
/// multi-round flows would fail here rather than in a user's deployment.
///
/// The arithmetic, stated so it cannot be misread: `TOOL_CLIENT_DEFAULT_DEPTH`
/// completes once its sealed counter reaches [`CLIENT_DEFAULT_ROUND_LIMIT`], so
/// the exchange is one initial call plus eight resends, the deepest MRTR round
/// sealed is 8, and the handler runs nine times. That is one request MORE than a
/// default-limited `pmcp::Client` would ever send, which makes the guard
/// strictly stronger than the client-side bound it protects.
#[tokio::test]
async fn a_flow_within_the_client_default_limit_is_unaffected() {
    let (addr, handle, calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_CLIENT_DEFAULT_DEPTH).await;
    let (mut state, mut keys) = expect_input_required(&first);
    let mut deepest = sealed_round(TOOL_CLIENT_DEFAULT_DEPTH, &state);
    let mut resends = 0_u64;

    let complete = loop {
        resends += 1;
        assert!(
            resends <= CLIENT_DEFAULT_ROUND_LIMIT,
            "the fixture must complete within the client default; it did not by resend \
             {resends}"
        );
        let answers = keys
            .iter()
            .map(|key| (key.clone(), elicit_answer()))
            .collect::<serde_json::Map<String, Value>>();
        let response = retry_tool(
            addr,
            json!(resends + 1),
            TOOL_CLIENT_DEFAULT_DEPTH,
            &state,
            Value::Object(answers),
        )
        .await;
        assert!(
            response.body.get("error").is_none(),
            "a flow within the shipped client default must never be refused, and this \
             one was at resend {resends}: {}",
            response.raw
        );
        if result_of(&response)
            .get("resultType")
            .and_then(Value::as_str)
            == Some("complete")
        {
            break response;
        }
        let (next, next_keys) = expect_input_required(&response);
        deepest = deepest.max(sealed_round(TOOL_CLIENT_DEFAULT_DEPTH, &next));
        state = next;
        keys = next_keys;
    };
    handle.abort();

    expect_resumed(&complete);
    assert_eq!(
        resends, CLIENT_DEFAULT_ROUND_LIMIT,
        "the flow ran to the full depth of the shipped client default"
    );
    assert_eq!(
        u64::from(deepest),
        CLIENT_DEFAULT_ROUND_LIMIT,
        "and the deepest round the server sealed is that same depth — comfortably under \
         the ceiling, which is deliberately 2x it"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst) as u64,
        CLIENT_DEFAULT_ROUND_LIMIT + 1,
        "one handler invocation per request, refusals included — there were none"
    );
}

// ===========================================================================
// pmcp-added wire-shape assertions.
// ===========================================================================

/// The MRTR fields are TOP-LEVEL `params` siblings, NOT `_meta` members.
///
/// This is the single most likely silent interop failure in the phase: an
/// in-house-only round-trip test that put them in `_meta` at both ends would
/// pass while every conformance check failed (T-113-28).
#[tokio::test]
async fn mrtr_fields_are_params_siblings() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let (state, _keys) = expect_input_required(&first);

    // (a) the WRONG placement: inside `params._meta`.
    let mut frame: Value =
        serde_json::from_str(&v2_body("tools/call", json!(2), tool_params(TOOL_ELICIT)))
            .expect("the harness emits valid JSON");
    let meta = frame["params"]["_meta"]
        .as_object_mut()
        .expect("the harness always writes params._meta");
    meta.insert("requestState".to_string(), json!(state.clone()));
    meta.insert(
        "inputResponses".to_string(),
        json!({ "user_name": elicit_answer() }),
    );
    let misplaced = post(
        addr,
        &v2_headers("tools/call", TOOL_ELICIT),
        &frame.to_string(),
    )
    .await;
    let (_state, keys) = expect_input_required(&misplaced);
    assert_eq!(
        keys,
        vec!["user_name".to_string()],
        "an `_meta`-placed retry must NOT resume — it re-elicits from scratch"
    );

    // (b) the RIGHT placement: top-level `params` siblings.
    let correct = retry_tool(
        addr,
        json!(3),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&correct);
}

/// Spec MUST: the JSON-RPC id differs between the initial request and the retry.
/// The server accepts the different id and echoes the RETRY's id back.
#[tokio::test]
async fn mrtr_retry_uses_different_id() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let first = call_tool(addr, 101, TOOL_ELICIT).await;
    assert_eq!(first.body["id"], json!(101), "round 1 echoes its own id");
    let (state, _keys) = expect_input_required(&first);

    // A STRING id, which is also a different JSON type from round 1's.
    let second = retry_tool(
        addr,
        json!("retry-202"),
        TOOL_ELICIT,
        &state,
        json!({ "user_name": elicit_answer() }),
    )
    .await;
    handle.abort();

    expect_resumed(&second);
    assert_eq!(
        second.body["id"],
        json!("retry-202"),
        "the retry's OWN id is echoed, never round 1's: {}",
        second.raw
    );
}

/// The pmcp-internal MRTR signal key never reaches the wire, on ANY path.
///
/// `dev.pmcp/mrtr` carries the handler's PLAINTEXT continuation — the very state
/// the AEAD `requestState` token exists to seal (T-113-31).
#[tokio::test]
async fn mrtr_signal_key_never_on_wire() {
    let (addr, handle, _calls) = spawn_fixture().await;

    let mut bodies: Vec<String> = Vec::new();

    let first = call_tool(addr, 1, TOOL_ELICIT).await;
    let (state, _keys) = expect_input_required(&first);
    bodies.push(first.raw.clone());

    bodies.push(
        retry_tool(
            addr,
            json!(2),
            TOOL_ELICIT,
            &state,
            json!({ "user_name": elicit_answer() }),
        )
        .await
        .raw,
    );
    bodies.push(call_tool(addr, 3, TOOL_MIXED).await.raw);
    bodies.push(call_tool(addr, 4, TOOL_BOOM).await.raw);
    bodies.push(
        post(
            addr,
            &v2_headers("prompts/get", PROMPT_NAME),
            &v2_body(
                "prompts/get",
                json!(5),
                json!({ "name": PROMPT_NAME, "arguments": {} }),
            ),
        )
        .await
        .raw,
    );
    bodies.push(
        post(
            addr,
            &v2_headers("resources/read", RESOURCE_URI),
            &v2_body("resources/read", json!(6), json!({ "uri": RESOURCE_URI })),
        )
        .await
        .raw,
    );
    bodies.push(
        post(
            addr,
            &v2_headers("tools/list", ""),
            &v2_body("tools/list", json!(7), json!({})),
        )
        .await
        .raw,
    );
    handle.abort();

    assert_eq!(bodies.len(), 7, "every exchange was captured");
    for body in &bodies {
        assert!(!body.is_empty(), "a captured body must not be empty");
        assert!(
            !body.contains(MRTR_SIGNAL_META_KEY),
            "the internal signal key leaked onto the wire: {body}"
        );
    }
}

// ===========================================================================
// Manifest enforcement.
// ===========================================================================

/// The phase directory holding the manifest and the spec re-check record.
fn phase_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".planning")
        .join("phases")
        .join("113-stateless-http-multi-round-trip-elicitation")
}

/// The lines of `text` under the `## `-level `heading`, up to the next `## `.
///
/// `###` sub-headings are kept, since they belong to the section.
fn section(text: &str, heading: &str) -> String {
    let mut collected = String::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            if inside {
                break;
            }
            inside = line.trim() == heading;
            continue;
        }
        if inside {
            collected.push_str(line);
            collected.push('\n');
        }
    }
    collected
}

/// The cells of every markdown table row in `text`, minus separator rows.
fn table_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches('`').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|cells| {
            !cells
                .iter()
                .all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':'))
        })
        .collect()
}

/// Every backticked `sep-2322-*` check id in `113-SPEC-RECHECK.md` § B.2.
fn pinned_check_ids(recheck: &str) -> Vec<String> {
    let start = recheck.find("### B.2").expect("§ B.2 exists");
    let rest = &recheck[start..];
    let end = rest.find("### B.3").expect("§ B.3 follows § B.2");
    let mut ids: Vec<String> = table_rows(&rest[..end])
        .into_iter()
        .filter_map(|cells| cells.get(1).cloned())
        .filter(|cell| cell.starts_with("sep-2322-"))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The 40-character pinned conformance sha recorded in `113-SPEC-RECHECK.md`.
fn pinned_sha(recheck: &str) -> String {
    table_rows(recheck)
        .into_iter()
        .find(|cells| cells.first().is_some_and(|c| c.contains("Pinned sha")))
        .and_then(|cells| cells.get(1).cloned())
        .expect("§ B.1 records a pinned sha")
}

/// The auxiliary manifest tables whose FIRST column names a local test.
const AUXILIARY_SECTIONS: [&str; 3] = [
    "## pmcp-Added Wire-Shape Rows",
    "## Enforcement",
    "## Real-Client Interoperability (Plan 113-11 Task 2)",
];

/// Every `sep-2322` scenario at the PINNED commit has a named test in this file.
///
/// This is what makes an unmapped upstream scenario a BUILD-VISIBLE FAILURE
/// rather than a silent omission (T-113-65). It re-reads
/// `113-SPEC-RECHECK.md` § B (the authority) and `113-CONFORMANCE-MANIFEST.md`
/// (the derived inventory) and cross-checks both against this file's source.
#[test]
fn manifest_maps_every_pinned_scenario() {
    let dir = phase_dir();
    let Ok(manifest) = fs::read_to_string(dir.join("113-CONFORMANCE-MANIFEST.md")) else {
        // `.planning/` is excluded from the published crate (Cargo.toml
        // `exclude`), so a downstream `cargo test` has no manifest to check.
        // In THIS repo the directory exists, so the check below always runs —
        // deleting the manifest while keeping the phase directory FAILS here.
        assert!(
            !dir.exists(),
            "the phase directory exists but its conformance manifest is missing — an \
             upstream scenario would go unmeasured"
        );
        return;
    };
    let recheck = fs::read_to_string(dir.join("113-SPEC-RECHECK.md"))
        .expect("the spec re-check record is the manifest's authority");
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("v2_mrtr.rs"),
    )
    .expect("this test file is readable");

    // 1. The manifest is pinned to the SAME conformance commit as the record.
    let sha = pinned_sha(&recheck);
    assert_eq!(sha.len(), 40, "a conformance pin is a full sha: {sha}");
    assert!(
        manifest.contains(&sha),
        "the manifest must carry the pinned sha {sha} copied from 113-SPEC-RECHECK.md"
    );

    // 2. The mapping covers the pinned check ids EXACTLY, in both directions.
    let pinned = pinned_check_ids(&recheck);
    assert!(!pinned.is_empty(), "§ B.2 enumerates the check ids");
    let mapping = table_rows(&section(&manifest, "## Scenario → Test Mapping"));
    let rows: Vec<(String, String)> = mapping
        .into_iter()
        .filter(|cells| cells.len() == 5 && cells[0].parse::<usize>().is_ok())
        .map(|cells| (cells[1].clone(), cells[4].clone()))
        .collect();
    assert_eq!(
        rows.len(),
        pinned.len(),
        "one manifest row per pinned check id"
    );
    let mut mapped: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
    mapped.sort();
    assert_eq!(
        mapped, pinned,
        "the manifest's check-id set must equal 113-SPEC-RECHECK.md § B.2's — a \
         difference means either an UNMAPPED upstream scenario or a stale row"
    );

    // 3. Every mapped test exists in this file.
    for (check_id, test_name) in &rows {
        assert!(
            source.contains(&format!("fn {test_name}(")),
            "{check_id} maps to `{test_name}`, which does not exist in tests/v2_mrtr.rs"
        );
    }

    // 4. The auxiliary tables name real tests too (strict when present).
    for heading in AUXILIARY_SECTIONS {
        for cells in table_rows(&section(&manifest, heading)) {
            let Some(name) = cells.first() else { continue };
            if name.is_empty() || name.contains(' ') {
                continue;
            }
            assert!(
                source.contains(&format!("fn {name}(")),
                "{heading} names `{name}`, which does not exist in tests/v2_mrtr.rs"
            );
        }
    }

    // 5. `## Unmapped` is EMPTY.
    let unmapped = section(&manifest, "## Unmapped");
    assert!(
        !unmapped.contains("sep-2322-"),
        "the Unmapped section lists an unmeasured scenario:\n{unmapped}"
    );
}

// ===========================================================================
// Real pmcp Client <-> real pmcp server (Plan 113-11 Task 2, CLNT-02).
//
// Everything above drives the wire by hand. Everything below drives the SAME
// fixture server with a REAL `pmcp::Client`, which is the first proof that the
// server half (plans 06/09) and the client half (plans 05/07) agree — each had
// only ever been tested against a hand-built counterpart. These tests live here
// rather than in plan 07 because a SCRIPTED real server cannot emit
// `input_required` until plan 09 exists.
// ===========================================================================

/// What the recording middleware observed about the requests that arrived.
#[derive(Debug, Default)]
struct Observed {
    /// How many HTTP requests reached the server.
    requests: AtomicUsize,
    /// Set when ANY body carried `initialize` or `notifications/initialized`.
    handshake: AtomicBool,
    /// Set when ANY request carried an inbound `Mcp-Session-Id` header.
    inbound_session_id: AtomicBool,
}

/// A thin recording wrapper at the HTTP boundary.
///
/// The three facts under observation — how many requests left the client,
/// whether a handshake was attempted, and whether a session id travelled — all
/// live at the transport layer, not in a handler, and counting them here is
/// what makes "exactly N requests" an observation rather than an inference.
struct RecordingMiddleware {
    observed: Arc<Observed>,
}

#[async_trait]
impl ServerHttpMiddleware for RecordingMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        self.observed.requests.fetch_add(1, Ordering::SeqCst);
        if request.get_header(MCP_SESSION_ID).is_some() {
            self.observed
                .inbound_session_id
                .store(true, Ordering::SeqCst);
        }
        let method = serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|body| {
                body.get("method")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        if matches!(
            method.as_deref(),
            Some("initialize" | "notifications/initialized")
        ) {
            self.observed.handshake.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Spawn the fixture behind a [`RecordingMiddleware`], on the STATEFUL default
/// config so session-freedom is proven by the per-request era gate.
async fn spawn_recorded_fixture() -> (SocketAddr, JoinHandle<()>, Arc<AtomicUsize>, Arc<Observed>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Observed::default());
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(RecordingMiddleware {
        observed: Arc::clone(&observed),
    }));
    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };
    let (addr, handle) = spawn_with(build_fixture_server(&calls), config).await;
    (addr, handle, calls, observed)
}

/// A `StreamableHttpTransport` pointed at `addr`.
fn transport_for(addr: SocketAddr) -> StreamableHttpTransport {
    let url = Url::parse(&format!("http://{addr}/")).expect("the loopback URL parses");
    StreamableHttpTransport::new(StreamableHttpTransportConfigBuilder::new(url).build())
}

/// A client builder already opted into `2026-07-28`.
fn v2_builder(addr: SocketAddr) -> ClientBuilder<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr))
        .with_protocol_version(ProtocolVersion(V2.to_string()))
        .expect("2026-07-28 is selectable")
}

/// A host elicitation handler that counts its invocations and answers with a
/// fixed [`ElicitAction`].
struct CountingElicitation {
    calls: Arc<AtomicUsize>,
    action: ElicitAction,
}

impl CountingElicitation {
    /// The accepting handler most tests use.
    fn accepting(calls: &Arc<AtomicUsize>) -> Self {
        Self {
            calls: Arc::clone(calls),
            action: ElicitAction::Accept,
        }
    }

    /// A handler that DECLINES — the reachable "the client cannot fulfil" path
    /// (see `client_server_mrtr_undeclared_capability_is_refused` for why an
    /// EMPTY registry cannot reach it).
    fn declining(calls: &Arc<AtomicUsize>) -> Self {
        Self {
            calls: Arc::clone(calls),
            action: ElicitAction::Decline,
        }
    }
}

#[async_trait]
impl HostElicitationHandler for CountingElicitation {
    async fn handle_elicitation(&self, _params: ElicitRequestParams) -> pmcp::Result<ElicitResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut content = HashMap::new();
        content.insert("value".to_string(), json!("ada"));
        Ok(ElicitResult {
            action: self.action,
            content: matches!(self.action, ElicitAction::Accept).then_some(content),
        })
    }
}

/// A host sampling handler that counts its invocations.
struct CountingSampling {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl HostSamplingHandler for CountingSampling {
    async fn handle_create_message(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CreateMessageResult::new(
            Content::text("sampled"),
            "fixture-model",
        ))
    }
}

/// A real `Client` completes a one-round MRTR exchange over HTTP, and the host
/// elicitation handler ran EXACTLY once.
#[tokio::test]
async fn client_server_mrtr_elicitation_roundtrip() {
    let (addr, handle, server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::accepting(&elicit_calls))
        .build();

    let result = client.call_tool(TOOL_ELICIT.to_string(), json!({})).await;

    handle.abort();
    let completed = result.expect("the gather->resend loop completes for the caller");
    assert!(
        !completed.content.is_empty(),
        "the caller receives the COMPLETE result, not an empty success"
    );
    assert_eq!(
        elicit_calls.load(Ordering::SeqCst),
        1,
        "exactly one elicitation was answered"
    );
    assert_eq!(
        server_calls.load(Ordering::SeqCst),
        2,
        "one ask, one resume"
    );
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        2,
        "one initial request and one retry"
    );
}

/// Three rounds: the caller still sees ONE completed result, and the handler ran
/// EXACTLY three times.
#[tokio::test]
async fn client_server_mrtr_three_rounds() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::accepting(&elicit_calls))
        .build();

    let result = client
        .call_tool(TOOL_THREE_ROUNDS.to_string(), json!({}))
        .await;

    handle.abort();
    assert!(result.is_ok(), "three rounds complete: {result:?}");
    assert_eq!(
        elicit_calls.load(Ordering::SeqCst),
        3,
        "one handler invocation per LOGICAL round"
    );
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        4,
        "three asks plus the resuming retry"
    );
}

/// Elicitation + sampling + roots arrive in ONE result and are all fulfilled in
/// ONE retry, with exactly one invocation of each handler.
#[tokio::test]
async fn client_server_mrtr_mixed_kinds() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let sample_calls = Arc::new(AtomicUsize::new(0));
    let roots_calls = Arc::new(AtomicUsize::new(0));
    let roots_counter = Arc::clone(&roots_calls);
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::accepting(&elicit_calls))
        .on_sampling(CountingSampling {
            calls: Arc::clone(&sample_calls),
        })
        .on_roots(move || {
            let counter = Arc::clone(&roots_counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(ListRootsResult { roots: vec![] })
            }
        })
        .build();

    let result = client.call_tool(TOOL_MIXED.to_string(), json!({})).await;

    handle.abort();
    assert!(result.is_ok(), "a mixed-kind round completes: {result:?}");
    assert_eq!(elicit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(sample_calls.load(Ordering::SeqCst), 1);
    assert_eq!(roots_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        2,
        "all three kinds are answered in ONE retry"
    );
}

/// The whole exchange happened with NO `initialize` and NO `Mcp-Session-Id` —
/// the stateless promise (HTTP-01) holds across a MULTI-request MRTR loop, not
/// just for a single round trip.
#[tokio::test]
async fn client_server_mrtr_no_session_no_handshake() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::accepting(&elicit_calls))
        .build();

    let result = client
        .call_tool(TOOL_THREE_ROUNDS.to_string(), json!({}))
        .await;

    handle.abort();
    assert!(result.is_ok(), "the exchange completes: {result:?}");
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        4,
        "guard against a vacuous pass — traffic must have arrived"
    );
    assert!(
        !observed.handshake.load(Ordering::SeqCst),
        "v2 has no handshake: neither initialize nor notifications/initialized may be sent"
    );
    assert!(
        !observed.inbound_session_id.load(Ordering::SeqCst),
        "no request in the MRTR loop may carry Mcp-Session-Id"
    );
}

/// A server that never completes trips the client's bound: EXACTLY `limit`
/// requests leave, and the caller gets the programmatically distinguishable
/// error rather than an infinite loop (T-113-11).
#[tokio::test]
async fn client_server_mrtr_round_limit_typed_error() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .mrtr_round_limit(2)
        .on_elicitation(CountingElicitation::accepting(&elicit_calls))
        .build();

    let error = client
        .call_tool(TOOL_FOREVER.to_string(), json!({}))
        .await
        .expect_err("a looping server must not loop the client forever");

    handle.abort();
    assert!(
        error.is_mrtr_round_limit_exceeded(),
        "the bound must be distinguishable: {error}"
    );
    assert_eq!(error.mrtr_round_limit(), Some(2));
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        2,
        "exactly `limit` requests may reach the server"
    );
    assert_eq!(
        elicit_calls.load(Ordering::SeqCst),
        2,
        "one invocation per round, and none after the bound trips"
    );
}

/// An UNFULFILLABLE `input_required` reaches the caller as a VALUE on the
/// additive `*_mrtr` path, and the client does NOT resend.
///
/// # Why the handler is registered but DECLINING
///
/// The plan asked for "no handlers at all". Against a REAL pmcp server that
/// scenario is unreachable, and correctly so: the client's v2
/// `clientCapabilities` are REGISTRY-AUTHORITATIVE (capability honesty,
/// HOST-05), so an empty registry declares no `elicitation`, and the server's
/// declared-capability precheck refuses the whole result with `-32021` BEFORE
/// minting anything (T-113-32). Two correct rules compose into "the server
/// never asks a question this client could not answer" — which is exactly what
/// `client_server_mrtr_undeclared_capability_is_refused` locks.
///
/// A DECLINING handler is the reachable D-06 path with the identical shape: the
/// capability IS declared, the server DOES mint, and the client still cannot
/// fulfil — because the user said no.
#[tokio::test]
async fn client_server_mrtr_outcome_input_required() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::declining(&elicit_calls))
        .build();

    let outcome = client
        .call_tool_mrtr(TOOL_ELICIT.to_string(), json!({}))
        .await
        .expect("an unfulfillable result is not an error on the *_mrtr path");

    handle.abort();
    assert_eq!(
        elicit_calls.load(Ordering::SeqCst),
        1,
        "the handler DID run — and declined"
    );
    let MrtrOutcome::InputRequired(result) = outcome else {
        panic!("expected MrtrOutcome::InputRequired");
    };
    assert!(
        result
            .input_requests
            .as_ref()
            .is_some_and(|requests| !requests.is_empty()),
        "the inputRequests the client could not answer must survive"
    );
    let state = result
        .request_state
        .as_deref()
        .expect("the continuation token reaches the caller");
    assert!(!state.is_empty());
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        1,
        "the client must NOT resend what it cannot answer"
    );
}

/// The SAME scenario through the EXISTING `call_tool` is the typed error
/// carrying the FULL result — explicitly NOT an empty `CallToolResult`.
///
/// This is the live proof of the review's most consequential finding:
/// `CallToolResult::content` is `#[serde(default)]`, so an `input_required`
/// result deserializes into it SUCCESSFULLY and yields a silently empty
/// success. The token recovered from the error is opened with the server's own
/// key, proving it is the server's real minted continuation and not a shell.
///
/// The handler is registered-but-declining for the reason documented on
/// [`client_server_mrtr_outcome_input_required`].
#[tokio::test]
async fn client_server_mrtr_existing_method_typed_error() {
    let (addr, handle, _server_calls, observed) = spawn_recorded_fixture().await;
    let elicit_calls = Arc::new(AtomicUsize::new(0));
    let client = v2_builder(addr)
        .on_elicitation(CountingElicitation::declining(&elicit_calls))
        .build();

    let outcome = client.call_tool(TOOL_ELICIT.to_string(), json!({})).await;

    handle.abort();
    let error = match outcome {
        Ok(result) => panic!(
            "an input_required result must NOT deserialize into a CallToolResult — \
             content is #[serde(default)], so this is a silently EMPTY success: {result:?}"
        ),
        Err(error) => error,
    };
    assert!(
        error.is_input_required_unfulfilled(),
        "the error must be programmatically distinguishable: {error}"
    );
    let recovered = error
        .input_required_result()
        .expect("the full result must be recoverable from the error");
    assert!(
        recovered
            .input_requests
            .as_ref()
            .is_some_and(|requests| !requests.is_empty()),
        "the inputRequests survive the error path"
    );
    let state = recovered
        .request_state
        .as_deref()
        .expect("the continuation token survives the error path");

    // The SAME `requestState` the server minted: opening it with the server's
    // own key recovers the continuation the scripted handler sealed.
    let (continuation, round) = open_request_state(
        &KEY,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &tool_params(TOOL_ELICIT),
        state,
    )
    .expect("the token verifies against the fixture server's key");
    assert_eq!(continuation, json!({ "step": 1 }));
    assert_eq!(round, 1, "minted at round + 1");
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        1,
        "no resend on the typed-error path either"
    );
}

/// A client with an EMPTY host registry is REFUSED with `-32021` rather than
/// handed an `input_required` it could never answer.
///
/// Discovered by this plan while wiring the real client to the real server, and
/// locked here because it is the composition of two independently-correct rules
/// that no single-sided test could observe:
///
/// * the CLIENT's v2 `clientCapabilities` are registry-authoritative — it cannot
///   advertise `elicitation` without an elicitation handler (capability honesty,
///   HOST-05); and
/// * the SERVER refuses, all-or-nothing, to emit `inputRequests` for a
///   capability the client did not declare, BEFORE minting any continuation
///   (T-113-32, `sep-2322-respect-client-capabilities`).
///
/// So the "handler-less client receives an `input_required`" scenario is
/// UNREACHABLE between two conformant pmcp peers: the server answers `-32021`
/// first. That is the better outcome — it costs no cryptographic work and tells
/// the client exactly what to declare — and it is why the two D-06 tests above
/// use a DECLINING handler instead of an empty registry.
#[tokio::test]
async fn client_server_mrtr_undeclared_capability_is_refused() {
    let (addr, handle, server_calls, observed) = spawn_recorded_fixture().await;
    let client = v2_builder(addr).build();

    let error = client
        .call_tool_mrtr(TOOL_ELICIT.to_string(), json!({}))
        .await
        .expect_err("a server may not ask for an undeclared capability");

    handle.abort();
    let pmcp::Error::Protocol { code, data, .. } = &error else {
        panic!("expected a JSON-RPC protocol error, got: {error:?}");
    };
    assert_eq!(
        code.as_i32(),
        MISSING_REQUIRED_CLIENT_CAPABILITY,
        "got: {error:?}"
    );
    let required = data
        .as_ref()
        .and_then(|data| data.get("requiredCapabilities"))
        .expect("the refusal names what to declare");
    assert!(
        required.is_object() && required.get("elicitation").is_some(),
        "requiredCapabilities is a ClientCapabilities OBJECT naming the gap: {required}"
    );
    assert_eq!(
        observed.requests.load(Ordering::SeqCst),
        1,
        "one request, refused — no retry loop"
    );
    assert_eq!(
        server_calls.load(Ordering::SeqCst),
        1,
        "the handler ran; the refusal happened at egress"
    );
}
