//! Shared live-HTTP harness for every Phase-113 `tests/v2_*.rs` file.
//!
//! Lifted from the Phase-112 `tests/v2_required_headers.rs` harness and extended
//! for MRTR. These helpers drive a REAL `StreamableHttpServer` over a loopback TCP
//! socket with a raw `reqwest` client (NOT the in-memory transport — RESEARCH
//! Pitfall 11) so every header / `_meta` combination crosses the actual axum HTTP
//! boundary.
//!
//! Test reliability doctrine (carried verbatim from Phase 112): EPHEMERAL PORT
//! (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()` binds
//! before returning), SHUTDOWN (`JoinHandle::abort()` after each round-trip).
//!
//! # What changed versus the Phase-112 helper (and why)
//!
//! - **Request id is a parameter.** The Phase-112 helper hard-coded `1`. MRTR
//!   retries MUST use a DIFFERENT JSON-RPC id than the initial request, and plan 08
//!   needs string ids, so [`v2_body`] takes the id.
//! - **`clientCapabilities` is always present.** Every shared request declares all
//!   three MRTR-fulfillable capabilities. A harness that omitted them would make
//!   every MRTR test accidentally exercise the undeclared-capability
//!   (`-32021`) path instead of the happy path. Use [`v2_body_with_caps`] to
//!   deliberately under-declare.
//! - **`Mcp-Name` is ALWAYS emitted**, empty for a name-less method. Since Phase
//!   118 D-13 the SERVER requires it only on name-bearing methods and discards it
//!   elsewhere, so this is a valid superset rather than an obligation; use
//!   [`v2_headers_for`] when the value must agree with the body (D-18 made that
//!   agreement enforced for `tasks/*` too).
//! - **[`Resp`] captures `mcp_session_id` and `content_type`**, which HTTP-01
//!   (assert the session header is ABSENT on v2) and HTTP-04 (assert
//!   `text/event-stream`) both need.
//! - **Two spawn helpers.** [`spawn_default_config`] uses
//!   `StreamableHttpServerConfig::default()` — a STATEFUL config with a live
//!   `session_id_generator`. Per RESEARCH Pitfall 1, `::stateless()` is a
//!   BUILD-TIME config, so a test that uses it never exercises the per-request era
//!   gate at all. [`spawn_stateless_config`] is kept for the tests that genuinely
//!   want the build-time stateless branch.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]
// Each consumer test binary uses a different subset of this harness; the unused
// remainder is not dead code, it is another file's entry point.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::shared::http_constants::{
    ACCEPT_STREAMABLE, MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION, MCP_SESSION_ID,
};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::{Content, GetPromptResult, ListResourcesResult, ReadResourceResult, RequestMeta};
use pmcp::ServerCapabilities;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// The 2026-07-28 protocol version string, sourced from pmcp's own constant so the
/// harness cannot drift from the crate.
pub const V2: &str = PROTOCOL_VERSION_2026_07_28;

/// The v1 protocol version an opted-in server keeps accepting alongside [`V2`].
///
/// Sourced from pmcp's own constant, like [`V2`] — this was the one version string
/// in the harness still spelled as a literal.
pub const V1: &str = LATEST_PROTOCOL_VERSION;

// ===========================================================================
// Reserved `_meta` keys — re-exported from the crate, not re-spelled.
// ===========================================================================

pub use pmcp::testing::{META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION};

// ===========================================================================
// Handlers — one real dispatch target per MRTR-eligible method.
// ===========================================================================

/// A trivial tool so `tools/call` has a real dispatch target.
pub struct SearchTool;

#[async_trait]
impl ToolHandler for SearchTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Plain payload — must NOT structurally resemble a built CallToolResult
        // (a `content` array) or the double-wrap tripwire (TOUT-02) fires.
        Ok(json!({ "answer": "ok" }))
    }
}

/// A trivial prompt so `prompts/get` has a real dispatch target.
pub struct GreetingPrompt;

#[async_trait]
impl PromptHandler for GreetingPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], Some("greeting".to_string())))
    }
}

/// A trivial resource handler so `resources/read` has a real dispatch target.
pub struct GreetingResource;

#[async_trait]
impl ResourceHandler for GreetingResource {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            uri.to_string(),
            "hello".to_string(),
            "text/plain".to_string(),
        )]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![]))
    }
}

/// The name of the task-capable tool [`spawn_tasks_server`] registers.
pub const TASKS_TOOL_NAME: &str = "long_task";

/// A task-capable tool whose task stays `working`.
///
/// It declares [`TaskSupport::Required`](pmcp::types::TaskSupport::Required) and
/// returns a task-shaped value with NO nested terminal `result`, so the shared
/// create gate mints a store task that is genuinely pending. That is the useful
/// default for a tasks suite: a pending task can be polled, updated and
/// cancelled, whereas a synchronously-completed one has already left the states
/// most of those tests are about.
pub fn long_task_tool() -> impl ToolHandler {
    pmcp::server::typed_tool::TypedTool::new_with_schema(
        TASKS_TOOL_NAME,
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "createdAt": "2026-07-28T00:00:00Z",
                    "lastUpdatedAt": "2026-07-28T00:00:00Z"
                }))
            })
        },
    )
    .with_description("a task-capable tool whose task stays pending")
    .with_execution(
        pmcp::types::ToolExecution::new().with_task_support(pmcp::types::TaskSupport::Required),
    )
}

/// The name of the SYNCHRONOUSLY-COMPLETING task-capable tool
/// [`spawn_tasks_server`] registers alongside [`long_task_tool`].
pub const COMPLETING_TOOL_NAME: &str = "reporting_task";

/// A task-capable tool that completes SYNCHRONOUSLY with `isError: true`.
///
/// The tool RAN. It produced an outcome. That outcome happens to report a
/// failure to the caller — which under the extension's terminal-status rule is
/// still `completed`, with the error detail inside `result`, NOT `failed`
/// (`failed` is reserved for a JSON-RPC protocol error during execution). The
/// two look identical from a "the tool failed" mindset and are opposite on the
/// wire, so the discipline needs a fixture that exercises the real create path
/// (`extract_terminal_result` -> `set_result` -> `update_status(Completed)`)
/// rather than a store poke that presupposes the answer.
pub fn completing_error_task_tool() -> impl ToolHandler {
    pmcp::server::typed_tool::TypedTool::new_with_schema(
        COMPLETING_TOOL_NAME,
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "createdAt": "2026-07-28T00:00:00Z",
                    "lastUpdatedAt": "2026-07-28T00:00:00Z",
                    "result": {
                        "content": [{ "type": "text", "text": "the upstream returned 404" }],
                        "isError": true
                    }
                }))
            })
        },
    )
    .with_description("a task-capable tool that completes with an isError result")
    .with_execution(
        pmcp::types::ToolExecution::new().with_task_support(pmcp::types::TaskSupport::Required),
    )
}

/// The name of the PAUSING task-capable tool [`spawn_tasks_server`] registers
/// alongside [`long_task_tool`] and [`completing_error_task_tool`].
pub const PAUSING_TOOL_NAME: &str = "elicit_task";

/// The `inputRequests` key the [`pausing_task_tool`] declares.
pub const PAUSING_TOOL_REQUEST_KEY: &str = "roots";

/// A task-capable tool that declares it needs INPUT before it can continue.
///
/// Its task-shaped value carries `status: "input_required"` and a server-authored
/// `inputRequests` map. That is the handler-side half of the create -> pause ->
/// resume loop (plan 114-12): the store mints the canonical id AFTER this handler
/// has returned, so the handler cannot record the requests itself;
/// `build_task_created_response` re-extracts them and records them against the
/// STORE-minted id, and the handle the caller receives is therefore ALREADY
/// paused and pollable.
///
/// It lives in the SHARED harness rather than in one suite because it is the only
/// CLIENT-REACHABLE way to produce an `input_required` task: before this tool
/// existed, `tests/v2_tasks_shapes.rs` had to poke the store directly
/// (`record_input_requests`) to construct that status at all. `tasks/update`
/// (114-13 / 114-14) needs a paused task the same way.
///
/// The map is spelled as the wire JSON `InputRequest` serializes to
/// (`{"method": "roots/list"}` for the unit `ListRoots` variant), which is what a
/// real handler emitting a `serde_json::Value` writes.
pub fn pausing_task_tool() -> impl ToolHandler {
    pmcp::server::typed_tool::TypedTool::new_with_schema(
        PAUSING_TOOL_NAME,
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "input_required",
                    "createdAt": "2026-07-28T00:00:00Z",
                    "lastUpdatedAt": "2026-07-28T00:00:00Z",
                    "inputRequests": {
                        PAUSING_TOOL_REQUEST_KEY: { "method": "roots/list" }
                    }
                }))
            })
        },
    )
    .with_description("a task-capable tool that pauses for a roots/list answer")
    .with_execution(
        pmcp::types::ToolExecution::new().with_task_support(pmcp::types::TaskSupport::Required),
    )
}

/// The reverse-DNS extension id the v2-opted-in server advertises in its
/// `capabilities.extensions` map, so a `server/discover` projection has a
/// non-empty `extensions` map to assert over (VERS-04).
pub const DISCOVER_EXTENSION_KEY: &str = "io.example/experimental";

/// A `ServerCapabilities` carrying ONLY the extensions map. Registering handlers
/// after `.capabilities(..)` layers the tool/prompt/resource sub-capabilities on
/// top (each set only when absent), so the extensions survive.
pub fn extensions_capabilities() -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let mut ext = HashMap::new();
    ext.insert(
        DISCOVER_EXTENSION_KEY.to_string(),
        json!({ "enabled": true }),
    );
    caps.extensions = Some(ext);
    caps
}

/// Build a v2-OPTED-IN `Server` exposing the `search` tool, the `greeting` prompt
/// and the `mem://greeting` resource, so all three MRTR-eligible methods have a
/// real handler.
///
/// The accept-list carries BOTH [`V1`] and [`V2`]; the extensions map is pre-seeded
/// BEFORE the handlers (which layer their own sub-capabilities on top).
pub fn build_v2_server() -> Server {
    build_v2_server_with("v2-harness", extensions_capabilities())
}

/// [`build_v2_server`] with a caller-chosen name and capability set.
///
/// The subscription suites need servers that advertise a specific combination of
/// `listChanged` / `subscribe`, but must otherwise be the SAME fixture — same
/// protocol versions, same three handlers. Keeping one builder means a handler
/// added here reaches every v2 test file instead of only the ones that had not
/// forked their own copy.
pub fn build_v2_server_with(name: &str, capabilities: ServerCapabilities) -> Server {
    Server::builder()
        .name(name)
        .version("1.0.0")
        .capabilities(capabilities)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}

/// An auth provider mapping `Bearer <name>` onto the subject `<name>`.
///
/// Lets a test choose its principals: two requests with different bearers arrive
/// as two DIFFERENT principals (which the `ListenKey` collision tests need),
/// while several streams under one bearer share ONE principal (which is what
/// makes the per-principal stream cap reachable — `anonymous_principal` is a
/// per-stream counter, so an unauthenticated caller never binds it).
pub struct BearerSubjects;

#[async_trait]
impl pmcp::server::auth::AuthProvider for BearerSubjects {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> pmcp::Result<Option<pmcp::server::auth::AuthContext>> {
        match authorization_header.and_then(|h| h.strip_prefix("Bearer ")) {
            Some(subject) if !subject.is_empty() => {
                Ok(Some(pmcp::server::auth::AuthContext::new(subject)))
            },
            _ => Err(pmcp::Error::authentication("missing or invalid token")),
        }
    }
}

/// An auth provider that ADMITS unauthenticated requests: `Ok(None)`, not `Err`.
///
/// Lifted verbatim (rustdoc included) from `tests/v2_subscriptions.rs`, where it
/// was introduced for D-113-N and where a second copy no longer exists — the
/// same precondition is now needed by the Phase-114 tasks suites, and two
/// divergent definitions of "the server admits anonymous callers" is exactly the
/// kind of fixture drift that makes a security test pass for the wrong reason.
///
/// [`BearerSubjects`] CANNOT serve this role: it returns `Err` for a missing
/// token, so the transport answers `401` long before dispatch and the
/// auth-refusal branch inside the route is never reached. So the precondition is
/// constructed EXPLICITLY here rather than hoping the shared bearer fixture
/// happens to have that shape.
///
/// `Ok(None)` for an absent token is a real and common configuration — optional
/// auth, an anonymous read tier, a gateway that forwards claims only when it has
/// them. It is also the ONLY way a test can reach the
/// `(None, has_auth_provider = true)` row of the fail-closed identity table,
/// which is the whole of TASK-05's refusal branch.
pub struct OptionalBearer;

#[async_trait]
impl pmcp::server::auth::AuthProvider for OptionalBearer {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> pmcp::Result<Option<pmcp::server::auth::AuthContext>> {
        Ok(authorization_header
            .and_then(|header| header.strip_prefix("Bearer "))
            .filter(|subject| !subject.is_empty())
            .map(pmcp::server::auth::AuthContext::new))
    }
}

// ===========================================================================
// Spawning.
// ===========================================================================

/// Spawn `server` over REAL HTTP with `StreamableHttpServerConfig::default()`.
///
/// **This is the DEFAULT choice for Phase-113 tests, and it is a deliberate
/// deviation from the Phase-112 helper** (which used `::stateless()`). RESEARCH
/// Pitfall 1: `stateless()` is a BUILD-TIME config that removes the session
/// machinery before a request is ever seen, so a test that uses it can never prove
/// the PER-REQUEST era gate suppresses sessions on v2. The default config keeps a
/// live `session_id_generator` (and `enable_json_response: false`, hence SSE-framed
/// responses — [`Resp`] unwraps those transparently).
///
/// Async because `StreamableHttpServer::start` binds the socket before returning,
/// which is what gives the caller its readiness guarantee.
pub async fn spawn_default_config(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_with(server, StreamableHttpServerConfig::default()).await
}

/// Spawn `server` with the BUILD-TIME `::stateless()` config, for the tests that
/// genuinely want that branch (and for Phase-112 parity).
pub async fn spawn_stateless_config(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_with(server, StreamableHttpServerConfig::stateless()).await
}

/// Spawn `server` with an arbitrary config on an ephemeral loopback port.
pub async fn spawn_with(
    server: Server,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    spawn_shared_with(Arc::new(Mutex::new(server)), config).await
}

/// [`spawn_with`] for a server the test still holds a handle to.
///
/// The subscription suites need the `Arc<Mutex<Server>>` back so they can drive
/// the REAL notification path (`Server::send_notification`) rather than injecting
/// a frame into the registry — which is the whole point of those tests. Taking
/// the server by value, as [`spawn_with`] does, makes that impossible, so this is
/// the primitive and `spawn_with` is the wrapper.
pub async fn spawn_shared_with(
    server: Arc<Mutex<Server>>,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(addr, server, config);
    http.start().await.expect("server starts")
}

/// [`spawn_shared_with`] with `StreamableHttpServerConfig::default()`.
pub async fn spawn_shared(server: Arc<Mutex<Server>>) -> (SocketAddr, JoinHandle<()>) {
    spawn_shared_with(server, StreamableHttpServerConfig::default()).await
}

/// The auth posture [`spawn_tasks_server`] installs.
///
/// There is deliberately NO `Default` and no defaulted overload: every caller
/// must state its posture, so a security test cannot pass because it silently
/// got a server with no auth provider (T-114-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPosture {
    /// No auth provider at all. An unauthenticated caller is the ONLY caller,
    /// and `has_auth_provider` is `false`.
    None,
    /// [`OptionalBearer`]: a provider that returns `Ok(None)` for a missing
    /// token, so `has_auth_provider` is `true` AND an unauthenticated request
    /// still reaches dispatch. This is the only posture that can produce the
    /// `(None, true)` row of the fail-closed identity table.
    Optional,
    /// [`BearerSubjects`]: a provider that returns `Err` for a missing token, so
    /// the transport answers `401` before dispatch. Use it for two-principal
    /// isolation tests, NOT for auth-refusal-branch tests.
    Required,
}

/// Spawn a v2-opted-in, tasks-backed server over real loopback HTTP.
///
/// The server carries:
///
/// * both [`V1`] and [`V2`] in its accept-list, so a per-request era gate is
///   actually exercised (the same reason [`spawn_default_config`] is used here
///   rather than the build-time `stateless()` config),
/// * an in-crate [`InMemoryTaskStore`](pmcp::server::task_store::InMemoryTaskStore),
///   whose presence auto-advertises the `tasks` capability,
/// * the task-capable [`long_task_tool`], and
/// * the caller's chosen [`AuthPosture`].
///
/// The in-crate store is deliberate. `pmcp-tasks`' `GenericTaskStore` refuses
/// the anonymous owner unless `allow_anonymous` is set (114-RESEARCH Pitfall 3),
/// and a SHARED harness must not bake that configuration decision into every
/// test that merely wants a task backend.
///
/// Returns the same `(addr, handle)` shape as [`spawn_default_config`], so
/// [`post`], [`post_raw`], [`Resp`] and [`teardown`] all work unchanged.
pub async fn spawn_tasks_server(posture: AuthPosture) -> (SocketAddr, JoinHandle<()>) {
    let (addr, handle, _store) = spawn_tasks_server_with_store(posture).await;
    (addr, handle)
}

/// [`spawn_tasks_server`] that ALSO hands the caller the store it installed.
///
/// The SAME fixture, one primitive lower — `spawn_tasks_server` is now a
/// wrapper, so a tool or capability added here reaches every existing tasks
/// suite unchanged.
///
/// The store handle exists because several task STATUSES are not reachable from
/// outside the process at all. A `tools/call` can produce a `working` task and a
/// `tasks/cancel` can produce a `cancelled` one, and since plan 114-12
/// [`pausing_task_tool`] makes `input_required` reachable from a real
/// `tools/call` too — but `failed` still needs a server-side `set_error`, which
/// has no client-facing trigger in this phase. A shape suite that could not
/// construct that status could not assert the shape the extension defines for
/// it.
pub async fn spawn_tasks_server_with_store(
    posture: AuthPosture,
) -> (
    SocketAddr,
    JoinHandle<()>,
    Arc<pmcp::server::task_store::InMemoryTaskStore>,
) {
    let store = Arc::new(pmcp::server::task_store::InMemoryTaskStore::new());
    let mut builder = Server::builder()
        .name("v2-tasks-harness")
        .version("1.0.0")
        .capabilities(extensions_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool(TASKS_TOOL_NAME, long_task_tool())
        .tool(COMPLETING_TOOL_NAME, completing_error_task_tool())
        .tool(PAUSING_TOOL_NAME, pausing_task_tool())
        .task_store(store.clone() as Arc<dyn pmcp::server::task_store::TaskStore>);
    builder = match posture {
        AuthPosture::None => builder,
        AuthPosture::Optional => builder.auth_provider(OptionalBearer),
        AuthPosture::Required => builder.auth_provider(BearerSubjects),
    };
    let server = builder.build().expect("tasks server builds");
    let (addr, handle) = spawn_default_config(server).await;
    (addr, handle, store)
}

/// The `Allow` value RFC 9110 §15.5.6 requires on every `405` this suite provokes.
///
/// Single-sourced here because the production side is likewise single-sourced:
/// `method_not_allowed_for_verb` is THE only 405 constructor, so the expectation
/// should have exactly one spelling too. `GET` and `DELETE` are deliberately
/// absent — both stay ROUTED on every feature set (an unrouted verb answers
/// `404`, a different claim), but `Allow` enumerates SUPPORT, and neither is
/// supported on `2026-07-28`.
pub const ALLOW: &str = "POST, OPTIONS";

/// Upper bound on any single stream read or poll in the subscription suites.
///
/// A hung stream must FAIL the test, not hang it.
pub const FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Shut a spawned server down in the order: drop sockets → `abort()` → `await`.
///
/// The order is the point. D-113-T recorded an intermittent nextest `LEAK` on
/// four `tests/v2_subscriptions.rs` tests caused by a bare `handle.abort()` with
/// no await: the aborted task has not necessarily finished when the test
/// function returns, and nextest's 100 ms leak timeout then fires as noise. A
/// still-open client socket keeps the server's connection task alive across the
/// abort, so the sockets go first.
///
/// `sockets` is anything the test owns that must die before the server: one
/// stream, a `Vec` of them, or `()` when the test only used the pooled
/// `reqwest` client and owns no socket of its own.
pub async fn teardown<S: Send>(handle: JoinHandle<()>, sockets: S) {
    drop(sockets);
    handle.abort();
    let _ = handle.await;
}

// ===========================================================================
// Request construction.
// ===========================================================================

/// The client capabilities every shared request declares: all three
/// MRTR-fulfillable kinds.
///
/// A server MUST NOT send an `inputRequests` entry for an undeclared capability
/// (it must answer `-32021 MissingRequiredClientCapability` instead), so a harness
/// that omitted these would silently route every MRTR test down the error path.
pub fn default_client_capabilities() -> Value {
    json!({ "elicitation": {}, "sampling": {}, "roots": {} })
}

/// A JSON-RPC v2 request body whose `params._meta` carries all three reserved keys,
/// including [`default_client_capabilities`].
///
/// `id` is a PARAMETER because an MRTR retry MUST use a different JSON-RPC id than
/// the initial request, and some plans need string ids.
pub fn v2_body(method: &str, id: Value, params: Value) -> String {
    v2_body_with_caps(method, id, params, default_client_capabilities())
}

/// The spec spelling of the per-request reserved-metadata object.
///
/// Phase-113 plan 04 (finding D-113-A) fixed the typed request structs, which
/// previously carried a struct-level `#[serde(rename_all = "camelCase")]` that
/// renamed the `_meta` FIELD to `meta` and so silently dropped a conformant
/// client's era signal. Both ingress paths — the raw `server/discover` read and
/// every typed request — now agree on `_meta`, so this harness emits ONE
/// spelling. `tests/common_harness_smoke.rs` carries the regression guard.
pub const REQUEST_META_KEY: &str = "_meta";

/// [`v2_body`] with an explicit `clientCapabilities` value, for tests that
/// deliberately under-declare.
pub fn v2_body_with_caps(method: &str, id: Value, params: Value, caps: Value) -> String {
    let mut params = match params {
        Value::Object(map) => Value::Object(map),
        _ => json!({}),
    };
    // Built through pmcp's OWN `RequestMeta` serialization so the reserved-key
    // spelling round-trips exactly what the server deserializes.
    let meta = RequestMeta::new()
        .with_meta(META_PROTOCOL_VERSION, json!(V2))
        .with_meta(
            META_CLIENT_INFO,
            json!({ "name": "pmcp-test-client", "version": "0.0.0" }),
        )
        .with_meta(META_CLIENT_CAPABILITIES, caps);
    let meta = serde_json::to_value(&meta).expect("request meta serializes");
    if let Some(object) = params.as_object_mut() {
        object.insert(REQUEST_META_KEY.to_string(), meta);
    }
    jsonrpc_envelope(method, id, params)
}

/// Assemble a JSON-RPC request envelope, CONSUMING `id` and `params`.
///
/// Built through a `serde_json::Map` rather than the `json!` macro because the
/// macro borrows its interpolated values, which would leave `id`/`params` as
/// pass-by-value-but-not-consumed parameters.
fn jsonrpc_envelope(method: &str, id: Value, params: Value) -> String {
    let mut body = serde_json::Map::new();
    body.insert("jsonrpc".to_string(), json!("2.0"));
    body.insert("id".to_string(), id);
    body.insert("method".to_string(), json!(method));
    body.insert("params".to_string(), params);
    Value::Object(body).to_string()
}

/// [`v2_body`] whose `clientCapabilities` DECLARES a set of protocol extensions.
///
/// The reserved `_meta` key written is
/// `io.modelcontextprotocol/clientCapabilities` (sourced from the crate's own
/// [`META_CLIENT_CAPABILITIES`], never re-spelled here), and its value is
/// `{"extensions": {<key>: {}}, …}` — the wire-correct home for Extensions-Track
/// declarations, alongside the three MRTR-fulfillable capabilities every shared
/// request already declares.
///
/// This is an ADDITIVE sibling of [`v2_body_with_caps`], which is left
/// untouched: several Phase-113 suites depend on its exact behaviour.
///
/// `extension_keys` is taken as `&str`s rather than read from a production
/// constant so this helper has NO dependency on plan 114-03, which introduces
/// the `io.modelcontextprotocol/tasks` key. Once that constant exists, callers
/// should pass it instead of a literal.
pub fn v2_body_with_client_extensions(
    method: &str,
    id: Value,
    params: Value,
    extension_keys: &[&str],
) -> String {
    let mut extensions = serde_json::Map::new();
    for key in extension_keys {
        extensions.insert((*key).to_string(), json!({}));
    }
    let mut caps = match default_client_capabilities() {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    caps.insert("extensions".to_string(), Value::Object(extensions));
    v2_body_with_caps(method, id, params, Value::Object(caps))
}

/// A v2 `tasks/*` request body carrying a `taskId`.
///
/// Covers `tasks/get`, `tasks/update`, `tasks/cancel` and `tasks/result` — every
/// `tasks/*` method whose params are "a task id" — so a per-method matrix in a
/// later plan is one loop rather than six near-identical `json!` literals. Pass
/// `method = "tasks/list"` only if the suite genuinely wants a `taskId` on a list
/// request (a malformed-params case); the well-formed list body is
/// `v2_body("tasks/list", id, json!({}))`.
pub fn tasks_request_body(method: &str, id: Value, task_id: &str) -> String {
    v2_body(method, id, json!({ "taskId": task_id }))
}

/// A `server/discover` request body — a v2-capable method that carries no logical
/// name, so it exercises the empty-`Mcp-Name` header rule end to end.
///
/// Since plan 04 closed D-113-B, `tools/list` (and every other list-shaped method)
/// can also carry the v2 `_meta` signal and is equally usable for that rule.
pub fn v2_discover_body(id: Value) -> String {
    v2_body("server/discover", id, json!({}))
}

/// A JSON-RPC v1 request body — no reserved `_meta` keys at all.
pub fn v1_body(method: &str, id: Value, params: Value) -> String {
    jsonrpc_envelope(method, id, params)
}

// ===========================================================================
// `Mcp-Name` value encoding.
// ===========================================================================

// The sentinel markers are NOT re-exported here. They were only ever used by the
// hand-copied encoder this module used to carry; a test that needs them should read
// `pmcp::testing::HEADER_SENTINEL_PREFIX` / `_SUFFIX` directly, which is one hop
// from the production constants rather than two.

/// pmcp's `Mcp-Name` value encoder — the PRODUCTION one.
///
/// This used to be a hand-copied mirror, and the mirror had already drifted: it
/// omitted the `MAX_HEADER_VALUE_LEN` clause from its passthrough predicate, so the
/// harness would emit a raw >8 KiB header where the real encoder sentinel-encodes.
/// Six Phase-113 plans build every request through this file, so the tests were
/// validating the harness against itself. Now it calls the shipped codec via the
/// `pmcp::testing` seam and cannot drift at all.
pub use pmcp::testing::encode_mcp_name as encode_header_value;

/// The v2 routing headers, with `name` sentinel-encoded as needed.
///
/// `Mcp-Name` is ALWAYS emitted, including the empty string for a name-less
/// method such as `tools/list` — that is what plan 05's client does, and the
/// server still accepts it after Phase 118 D-13 (an empty value on a method with
/// no routing name is discarded). The header being OPTIONAL there since D-13 does
/// not make emitting it wrong.
///
/// For a NAME-BEARING method the value must agree with the body, so prefer
/// [`v2_headers_for`], which derives it from the params through the production
/// table rather than trusting the caller to restate it.
pub fn v2_headers(method: &str, name: &str) -> Vec<(String, String)> {
    v2_headers_raw(method, &encode_header_value(name))
}

/// [`v2_headers`] with `Mcp-Name` DERIVED FROM THE BODY, exactly as a conformant
/// client derives it (Phase 118 D-18).
///
/// The routing key is resolved through the PRODUCTION combined table, via the
/// `pmcp::testing::routing_name_key` seam — so a test cannot restate "where the
/// name lives" and drift from the server's own predicate. A method with no
/// routing name, or params carrying no string at its key, yields the empty
/// string, which the gate discards.
///
/// Use this instead of hand-passing `""` for a `tasks/*` request: since D-18 the
/// server cross-checks `Mcp-Name` against `params.taskId`, so an empty value on a
/// request that HAS a `taskId` is a genuine `-32020` header/body disagreement.
pub fn v2_headers_for(method: &str, params: &Value) -> Vec<(String, String)> {
    let name = pmcp::testing::routing_name_key(method)
        .and_then(|key| params.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default();
    v2_headers(method, name)
}

/// [`v2_headers`] without the value encoder, for tests that deliberately send a
/// malformed sentinel or a raw non-ASCII value.
pub fn v2_headers_raw(method: &str, raw_name: &str) -> Vec<(String, String)> {
    vec![
        (MCP_METHOD.to_string(), method.to_string()),
        (MCP_NAME.to_string(), raw_name.to_string()),
        (MCP_PROTOCOL_VERSION.to_string(), V2.to_string()),
    ]
}

/// Convenience constructor for one extra header.
pub fn header(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

// ===========================================================================
// Response capture.
// ===========================================================================

/// Raw response view: HTTP status + the v2 headers + the session header + the
/// content type + the JSON body + the RAW response text.
///
/// `raw` is kept for byte-identity assertions — the parsed `body` alone cannot
/// prove a v1 wire is byte-for-byte unchanged.
#[derive(Debug, Clone)]
pub struct Resp {
    /// HTTP status code.
    pub status: u16,
    /// The echoed `Mcp-Method` header, if any.
    pub mcp_method: Option<String>,
    /// The echoed `Mcp-Name` header, if any.
    pub mcp_name: Option<String>,
    /// The echoed `MCP-Protocol-Version` header, if any.
    pub mcp_version: Option<String>,
    /// The `Mcp-Session-Id` header — MUST be absent on a v2 response (HTTP-01).
    pub mcp_session_id: Option<String>,
    /// The response `Content-Type` — `text/event-stream` for an SSE reply.
    pub content_type: Option<String>,
    /// The `Allow` header. RFC 9110 §15.5.6 makes it a MUST on every `405`, so a
    /// `None` here on a refused verb is a spec violation, not a detail.
    pub allow: Option<String>,
    /// The parsed JSON body. An SSE reply is unwrapped from its first `data:`
    /// frame, so callers assert the same way in both framings.
    pub body: Value,
    /// The verbatim response text, SSE framing included.
    pub raw: String,
}

/// Parse a response body that may be either bare JSON or a single SSE frame.
///
/// `StreamableHttpServerConfig::default()` has `enable_json_response: false`, so a
/// POST reply arrives as `event: message\ndata: {…}`. Unwrapping it here means every
/// consumer asserts on `body` identically regardless of which spawn helper it used.
fn parse_body(text: &str, content_type: Option<&str>) -> Value {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return value;
    }
    let looks_like_sse = content_type.is_some_and(|ct| ct.starts_with("text/event-stream"));
    if looks_like_sse || text.contains("data:") {
        for line in text.lines() {
            if let Some(payload) = line.strip_prefix("data:") {
                if let Ok(value) = serde_json::from_str::<Value>(payload.trim()) {
                    return value;
                }
            }
        }
    }
    Value::Null
}

/// Drive a prepared request to completion and capture a [`Resp`].
///
/// NOTE: this reads the body to EOF. A long-lived `subscriptions/listen` stream
/// (HTTP-04) needs a streaming reader instead — that is plan 13's surface, not this
/// request/response helper.
async fn send(request: reqwest::RequestBuilder, extra: &[(String, String)]) -> Resp {
    let mut request = request;
    for (name, value) in extra {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.expect("request sent");
    let status = response.status().as_u16();
    let hget = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let mcp_method = hget(MCP_METHOD);
    let mcp_name = hget(MCP_NAME);
    let mcp_version = hget(MCP_PROTOCOL_VERSION);
    let mcp_session_id = hget(MCP_SESSION_ID);
    let content_type = hget("content-type");
    let allow = hget("allow");
    let raw = response.text().await.unwrap_or_default();
    let body = parse_body(&raw, content_type.as_deref());
    Resp {
        status,
        mcp_method,
        mcp_name,
        mcp_version,
        mcp_session_id,
        content_type,
        allow,
        body,
        raw,
    }
}

/// The `Accept` value a v2 client sends: both content types, per the transport spec.
pub const ACCEPT_BOTH: &str = ACCEPT_STREAMABLE;

/// POST a body with the given extra headers.
pub async fn post(addr: SocketAddr, extra: &[(String, String)], body: &str) -> Resp {
    post_with_accept(addr, ACCEPT_BOTH, extra, body).await
}

/// One shared `reqwest::Client` for the whole harness.
///
/// Each `Client::new()` builds a fresh rustls `ClientConfig` and root-certificate
/// store and gets its own connection pool, so a per-call client meant no connection
/// was ever reused across the hundreds of requests the Phase-113 test files make.
static CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

/// [`post`] with an explicit `Accept` value, for the content-negotiation tests.
pub async fn post_with_accept(
    addr: SocketAddr,
    accept: &str,
    extra: &[(String, String)],
    body: &str,
) -> Resp {
    let request = CLIENT
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", accept)
        .body(body.to_string());
    send(request, extra).await
}

/// POST RAW bytes with NO JSON validation, so a test can send malformed JSON, an
/// unknown method or a string id at the wire level.
pub async fn post_raw(addr: SocketAddr, extra: &[(String, String)], raw_body: &str) -> Resp {
    post_with_accept(addr, ACCEPT_BOTH, extra, raw_body).await
}

/// GET the MCP endpoint — v2 must answer 405 (HTTP-01).
pub async fn get(addr: SocketAddr, extra: &[(String, String)]) -> Resp {
    let request = CLIENT
        .get(format!("http://{addr}"))
        .header("accept", "text/event-stream");
    send(request, extra).await
}

/// DELETE the MCP endpoint — v2 must answer 405 (HTTP-01).
pub async fn delete(addr: SocketAddr, extra: &[(String, String)]) -> Resp {
    let request = CLIENT
        .delete(format!("http://{addr}"))
        .header("accept", "application/json");
    send(request, extra).await
}
