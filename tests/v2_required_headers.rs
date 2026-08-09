//! Phase 112-06 (VERS-05 / D-05 / D-06 / D-11): live-HTTP acceptance gate for the
//! required v2 headers on the streamable-HTTP path.
//!
//! These tests drive a REAL `StreamableHttpServer` over a loopback TCP socket
//! with a raw `reqwest` client (NOT the in-memory transport — RESEARCH Pitfall
//! 11) so every header/`_meta` combination crosses the actual axum HTTP boundary.
//! They prove the full classification matrix, the strict all-three-headers
//! reject (D-05), the `Mcp-Method`/`Mcp-Name` body cross-check (D-06), outbound
//! header emission on success AND error, and that v1 / non-opted-in servers get
//! ZERO enforcement (D-04 / D-11).
//!
//! Test reliability (carried from the Phase 102/104 HTTP harness): EPHEMERAL
//! PORT (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()`
//! binds before returning), SHUTDOWN (`JoinHandle::abort()` after each round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use pmcp::types::protocol::error_codes::{HEADER_MISMATCH, UNSUPPORTED_PROTOCOL_VERSION};

use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpResponse,
};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::testing::META_SERVER_INFO;
use pmcp::types::prompts::GetPromptRequest;
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28 as V2};
use pmcp::types::resources::ReadResourceRequest;
use pmcp::types::{
    CallToolRequest, Content, GetPromptResult, ListResourcesResult, ReadResourceResult, RequestMeta,
};
use pmcp::ServerCapabilities;
use pmcp::{RequestHandlerExtra, ToolHandler};
use tokio::sync::Mutex;

/// A trivial tool so `tools/call` has a real dispatch target.
struct SearchTool;

#[async_trait]
impl ToolHandler for SearchTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        // Plain payload — must NOT structurally resemble a built CallToolResult
        // (a `content` array) or the double-wrap tripwire (TOUT-02) fires.
        Ok(serde_json::json!({ "answer": "ok" }))
    }
}

/// A trivial prompt so `prompts/get` has a real dispatch target.
struct GreetingPrompt;

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
struct GreetingResource;

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

/// The reverse-DNS extension id the v2-opted-in server advertises in its
/// `capabilities.extensions` map, so the `server/discover` projection has a
/// non-empty `extensions` map to assert over the real HTTP wire (VERS-04).
const DISCOVER_EXTENSION_KEY: &str = "io.example/experimental";

/// A `ServerCapabilities` carrying ONLY the extensions map. Registering handlers
/// after `.capabilities(..)` layers the tool/prompt/resource sub-capabilities on
/// top (each set only when absent), so the extensions survive.
fn extensions_capabilities() -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let mut ext = HashMap::new();
    ext.insert(
        DISCOVER_EXTENSION_KEY.to_string(),
        serde_json::json!({ "enabled": true }),
    );
    caps.extensions = Some(ext);
    caps
}

/// Build a `Server` exposing the `search` tool, a `greeting` prompt, and a
/// `mem://greeting` resource, optionally v2-opted-in.
///
/// The v2-opted-in server pre-seeds a `capabilities.extensions` entry (BEFORE the
/// handlers, which layer their own sub-capabilities on top) so the
/// `server/discover` projection has a non-empty `extensions` map (Plan 112-10).
fn build_server(opt_in_v2: bool) -> Server {
    let mut builder = Server::builder()
        .name("v2-required-headers")
        .version("1.0.0");
    if opt_in_v2 {
        builder = builder
            .capabilities(extensions_capabilities())
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(V2.to_string()),
            ]);
    }
    builder
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}

/// A `server/discover` request body. `meta_version` (when `Some`) carries the
/// reserved protocol-version key in `params._meta` so the raw-_meta gate resolves
/// the era from the authoritative `_meta` signal.
fn discover_body(meta_version: Option<&str>) -> String {
    let mut params = serde_json::Map::new();
    if let Some(v) = meta_version {
        params.insert(
            "_meta".to_string(),
            serde_json::json!({ "io.modelcontextprotocol/protocolVersion": v }),
        );
    }
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": params,
    })
    .to_string()
}

/// An auth provider that REJECTS any request without a valid bearer token, used
/// to prove `server/discover` is subject to auth (no bypass — finding #3/#6).
struct RejectingAuth;

#[async_trait]
impl AuthProvider for RejectingAuth {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> pmcp::Result<Option<AuthContext>> {
        match authorization_header {
            Some("Bearer good-token") => Ok(None),
            _ => Err(pmcp::Error::authentication("missing or invalid token")),
        }
    }
}

/// A response-middleware that records whether it observed a response, used to
/// prove `server/discover` flows through the response-middleware path.
struct RecordingMiddleware {
    saw_response: Arc<AtomicBool>,
}

#[async_trait]
impl ServerHttpMiddleware for RecordingMiddleware {
    async fn on_response(
        &self,
        _response: &mut ServerHttpResponse,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        self.saw_response.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Spawn a v2-opted-in server with an auth provider installed (fast path).
async fn spawn_with_auth() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let server = Arc::new(Mutex::new(
        Server::builder()
            .name("v2-required-headers-auth")
            .version("1.0.0")
            .capabilities(extensions_capabilities())
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(V2.to_string()),
            ])
            .tool("search", SearchTool)
            .auth_provider(RejectingAuth)
            .build()
            .expect("server builds"),
    ));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http =
        StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::stateless());
    http.start().await.expect("server starts")
}

/// Spawn a v2-opted-in server with an HTTP middleware chain (middleware path).
async fn spawn_with_middleware(
    saw_response: Arc<AtomicBool>,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let server = Arc::new(Mutex::new(build_server(true)));
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(RecordingMiddleware { saw_response }));
    let mut config = StreamableHttpServerConfig::stateless();
    config.http_middleware = Some(Arc::new(chain));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(addr, server, config);
    http.start().await.expect("server starts")
}

/// Stand the server up over REAL HTTP (stateless JSON mode); return the bound
/// address + the server task handle.
async fn spawn(opt_in_v2: bool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let server = Arc::new(Mutex::new(build_server(opt_in_v2)));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http =
        StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::stateless());
    http.start().await.expect("server starts")
}

/// Raw response view: HTTP status + the three v2 headers + the JSON body + the
/// RAW response text (kept for byte-identity assertions — the parsed `body`
/// alone cannot prove the v1 wire is byte-for-byte unchanged).
struct Resp {
    status: u16,
    mcp_method: Option<String>,
    mcp_name: Option<String>,
    mcp_version: Option<String>,
    body: serde_json::Value,
    raw: String,
}

/// POST a raw body with the given extra headers and return a [`Resp`].
///
/// Always sends the transport-required `content-type`/`accept`; `extra` carries
/// the v2 headers under test (or omits them).
async fn post(addr: SocketAddr, extra: &[(&str, &str)], body: &str) -> Resp {
    let client = reqwest::Client::new();
    let mut req = client
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body.to_string());
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let resp = req.send().await.expect("request sent");
    let status = resp.status().as_u16();
    let hget = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let mcp_method = hget("mcp-method");
    let mcp_name = hget("mcp-name");
    let mcp_version = hget("mcp-protocol-version");
    let text = resp.text().await.unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    Resp {
        status,
        mcp_method,
        mcp_name,
        mcp_version,
        body,
        raw: text,
    }
}

/// A raw `tools/call` body. `meta_version` (when `Some`) is carried in
/// `params._meta` under the reserved protocol-version key so the SHARED Plan-04
/// resolver classifies the era from `_meta` (the authoritative signal).
///
/// Built via pmcp's OWN serialization so the wire `_meta` field name round-trips
/// exactly what the server deserializes (the request `_meta` field is renamed by
/// serde's camelCase rule — building through the typed struct avoids depending on
/// that spelling).
fn call_body(tool: &str, meta_version: Option<&str>) -> String {
    let mut req = CallToolRequest::new(tool, serde_json::json!({}));
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": params,
    })
    .to_string()
}

/// A raw `prompts/get` body built through the TYPED `GetPromptRequest` so the
/// wire `_meta` camelCase spelling round-trips exactly what the server
/// deserializes. `meta_version` carries the reserved protocol-version key.
fn prompt_body(name: &str, meta_version: Option<&str>) -> String {
    let mut req = GetPromptRequest {
        name: name.to_string(),
        arguments: HashMap::new(),
        _meta: None,
    };
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "prompts/get",
        "params": params,
    })
    .to_string()
}

/// A raw `resources/read` body built through the TYPED `ReadResourceRequest`
/// carrying ONLY `uri` (no synthetic `params.name`) — this is the standards
/// shape that exercises the finding #2 path (logical name from `params.uri`).
fn resource_body(uri: &str, meta_version: Option<&str>) -> String {
    let mut req = ReadResourceRequest {
        uri: uri.to_string(),
        _meta: None,
    };
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/read",
        "params": params,
    })
    .to_string()
}

/// Abort the server task and swallow the cancellation.
async fn shutdown(handle: tokio::task::JoinHandle<()>) {
    handle.abort();
    let _ = handle.await;
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 header + v2 _meta + all headers + matching body → ACCEPT.
// Also proves the SUCCESS response carries all three outbound headers.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_accepts_well_formed_v2_and_echoes_headers() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "search"),
        ],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "well-formed v2 request should be accepted");
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
    // Outbound headers on SUCCESS.
    assert_eq!(r.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(r.mcp_name.as_deref(), Some("search"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 header but non-v2 _meta (absent) → REJECT (fail closed).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_v2_header_with_non_v2_meta() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "search"),
        ],
        &call_body("search", None), // no v2 _meta → era resolves v1
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "header/_meta disagreement must fail closed");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 _meta but absent MCP-Protocol-Version header → REJECT.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_v2_meta_without_version_header() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "_meta v2 with no version header must reject");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// ---------------------------------------------------------------------------
// D-05: a v2 request missing `Mcp-Method` or `MCP-Protocol-Version`, or missing
// `Mcp-Name` on a NAME-BEARING method → 4xx + JSON-RPC error.
//
// `tools/call` is name-bearing, so the `Mcp-Name` row below survives Phase 118
// D-13 unchanged — the relaxation reaches only methods with no routing name.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_missing_mcp_name() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-protocol-version", V2), ("mcp-method", "tools/call")],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "missing Mcp-Name must reject (D-05)");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
    assert_eq!(r.body["jsonrpc"], "2.0");
}

// ---------------------------------------------------------------------------
// D-06: Mcp-Method header disagreeing with the body method → REJECT.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_method_body_mismatch() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "resources/read"), // header lies about the method
            ("mcp-name", "search"),
        ],
        &call_body("search", Some(V2)), // body method is tools/call
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "Mcp-Method vs body-method mismatch must reject"
    );
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// ---------------------------------------------------------------------------
// D-06: Mcp-Name header disagreeing with params.name on a name-bearing method.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_name_body_mismatch() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "not-search"), // disagrees with params.name
        ],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "Mcp-Name vs params.name mismatch must reject"
    );
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// ---------------------------------------------------------------------------
// A v2 request that PASSES the gate but hits an unknown tool → the handler's
// structured JSON-RPC error still carries all three outbound headers (VERS-05).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_error_response_still_echoes_headers() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "ghost"),
        ],
        &call_body("ghost", Some(V2)), // valid v2 shape, unknown tool
    )
    .await;
    shutdown(handle).await;

    // HTTP 200 with a JSON-RPC error payload (unknown tool), headers present.
    assert_eq!(r.status, 200);
    assert!(
        r.body.get("error").is_some(),
        "expected JSON-RPC error: {}",
        r.body
    );
    assert_eq!(r.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(r.mcp_name.as_deref(), Some("ghost"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix cell: unsupported per-request version in _meta → explicit reject.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_unsupported_version() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", Some("1999-01-01")), // not in the accept-list
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "unsupported version must reject");
    // Phase 113 plan 04: the spec-allocated UNSUPPORTED_PROTOCOL_VERSION replaces
    // the generic INVALID_PARAMS, and the payload MUST list what the server accepts
    // so the client can retry with a mutually supported version instead of probing.
    assert_eq!(r.body["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION);
    assert!(
        r.body["error"]["data"]["supported"].is_array(),
        "-32022 MUST carry an error.data.supported ARRAY: {}",
        r.raw
    );
}

// ---------------------------------------------------------------------------
// Matrix cell: v1 request on an OPTED-IN server (no v2 signals) → v1 behavior.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_v1_request_on_opted_in_server_untouched() {
    let (addr, handle) = spawn(true).await;
    let r = post(addr, &[], &call_body("search", None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 tools/call must still work");
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
    // No v2 enforcement, no v2 outbound headers forced.
    assert_eq!(r.mcp_method, None);
    assert_eq!(r.mcp_name, None);
}

// ---------------------------------------------------------------------------
// D-04: a NON-opted-in server runs ZERO enforcement — a request carrying stray
// Mcp-Method/Mcp-Name headers is NOT subject to the v2 gate (legacy behavior).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_non_opted_in_server_ignores_v2_headers() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", None),
    )
    .await;
    shutdown(handle).await;

    // The stray headers are ignored; the request flows the normal v1 path.
    assert_eq!(
        r.status, 200,
        "non-opted-in server must not enforce v2 headers"
    );
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (Gap C): a well-formed v2 prompts/get is ACCEPTED (200) and its
// inner result carries the resultType/serverInfo envelope (VERS-05 + VERS-07).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_prompts_get_accepts_and_envelopes() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "prompts/get"),
            ("mcp-name", "greeting"),
        ],
        &prompt_body("greeting", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "well-formed v2 prompts/get must be accepted");
    let result = r.body.get("result").expect("expected a result");
    assert_eq!(
        result.get("resultType").and_then(|v| v.as_str()),
        Some("complete"),
        "v2 prompts/get result must carry resultType:complete: {}",
        r.body
    );
    // Plan 113-09 Task 3: server identity lives INSIDE `result._meta` at the
    // schema key, never as a top-level result key.
    assert!(
        result["_meta"][META_SERVER_INFO].is_object(),
        "v2 prompts/get result must carry _meta[{META_SERVER_INFO}]: {}",
        r.body
    );
    assert!(
        result.get("serverInfo").is_none(),
        "v2 prompts/get must not carry a top-level serverInfo: {}",
        r.body
    );
    // Outbound headers echoed on success.
    assert_eq!(r.mcp_method.as_deref(), Some("prompts/get"));
    assert_eq!(r.mcp_name.as_deref(), Some("greeting"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Phase 112-09 (Gap C / finding #2): a standards-shaped v2 resources/read
// (Mcp-Name = the URI, body built from a real ReadResourceRequest with ONLY
// `uri` — NO synthetic params.name) is ACCEPTED (200) with the envelope. This
// FAILS if Task 2's params.uri method-aware fix is missing (would reject 400).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_resources_read_accepts_and_envelopes() {
    let (addr, handle) = spawn(true).await;
    let uri = "mem://greeting";
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "resources/read"),
            ("mcp-name", uri), // Mcp-Name carries the resource URI
        ],
        &resource_body(uri, Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "standards-shaped v2 resources/read (uri only) must be accepted: {}",
        r.body
    );
    let result = r.body.get("result").expect("expected a result");
    assert_eq!(
        result.get("resultType").and_then(|v| v.as_str()),
        Some("complete"),
        "v2 resources/read result must carry resultType:complete: {}",
        r.body
    );
    assert!(
        result["_meta"][META_SERVER_INFO].is_object(),
        "v2 resources/read result must carry _meta[{META_SERVER_INFO}]: {}",
        r.body
    );
    assert!(
        result.get("serverInfo").is_none(),
        "v2 resources/read must not carry a top-level serverInfo: {}",
        r.body
    );
    assert_eq!(r.mcp_method.as_deref(), Some("resources/read"));
    assert_eq!(r.mcp_name.as_deref(), Some(uri));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix consistency: a v2-header prompts/get with NON-v2 _meta is still
// REJECTED (the fail-closed cell) — the fix did not loosen the gate.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_prompts_get_rejects_v2_header_with_non_v2_meta() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "prompts/get"),
            ("mcp-name", "greeting"),
        ],
        &prompt_body("greeting", None), // no v2 _meta → era resolves v1
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "v2-header prompts/get with non-v2 _meta must fail closed"
    );
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

/// Parse the raw v1 response text and assert full structural equality against a
/// pinned golden JSON-RPC shape, plus assert the raw string carries no v2 keys.
fn assert_v1_byte_identical(raw: &str, expected_result: &serde_json::Value) {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).expect("v1 response must be valid JSON");
    let expected = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": expected_result,
    });
    assert_eq!(
        parsed, expected,
        "v1 wire must be structurally identical to the golden fixture; got raw: {raw}"
    );
    // Byte-level guard: none of the v2-only keys leak onto the v1 wire.
    assert!(
        !raw.contains("resultType"),
        "v1 raw must not contain resultType: {raw}"
    );
    assert!(
        !raw.contains("serverInfo"),
        "v1 raw must not contain serverInfo: {raw}"
    );
    assert!(
        !raw.contains("_meta"),
        "v1 raw must not contain _meta: {raw}"
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (v1 byte-identity, finding #5a): a v1 prompts/get on a
// NON-opted-in server produces a response whose RAW bytes equal a pinned golden
// fixture — full structural equality, not merely two-key absence.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v1_prompts_get_byte_identical() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let r = post(addr, &[], &prompt_body("greeting", None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 prompts/get must still work");
    // GreetingPrompt returns description "greeting" + empty messages; on v1 the
    // wire omits None/empty _meta and carries NO envelope.
    assert_v1_byte_identical(
        &r.raw,
        &serde_json::json!({
            "description": "greeting",
            "messages": [],
        }),
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (v1 byte-identity): a v1 resources/read on a NON-opted-in server
// is byte-for-byte the pinned golden fixture (no envelope, no _meta leak).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v1_resources_read_byte_identical() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let uri = "mem://greeting";
    let r = post(addr, &[], &resource_body(uri, None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 resources/read must still work");
    // GreetingResource returns a single text resource content at the URI.
    assert_v1_byte_identical(
        &r.raw,
        &serde_json::json!({
            "contents": [{
                "uri": uri,
                "text": "hello",
                "mimeType": "text/plain",
            }],
        }),
    );
}

// ===========================================================================
// Phase 112-10 (VERS-04): LIVE server/discover over the real HTTP transport.
// ===========================================================================

/// Standard v2 headers for a `server/discover` request. `server/discover` is NOT
/// a name-bearing method, so `Mcp-Name` is presence-only (any value); the
/// `Mcp-Method`/body cross-check pins it to `server/discover`.
fn discover_v2_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("mcp-protocol-version", V2),
        ("mcp-method", "server/discover"),
        ("mcp-name", "server/discover"),
    ]
}

// A v2 server/discover returns the capability projection INCLUDING the
// `.skills`-populated extensions map, plus serverInfo + resultType:complete, and
// preserves the request id.
#[tokio::test]
async fn server_discover_v2_returns_capability_projection_with_extensions() {
    let (addr, handle) = spawn(true).await;
    let r = post(addr, &discover_v2_headers(), &discover_body(Some(V2))).await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "v2 server/discover must be accepted: {}",
        r.body
    );
    let result = r.body.get("result").expect("expected a result");
    // The extensions map is projected (finding: reachable in production).
    assert_eq!(
        result["capabilities"]["extensions"][DISCOVER_EXTENSION_KEY]["enabled"],
        serde_json::json!(true),
        "discover projection must carry the registered extension id: {}",
        r.body
    );
    // serverInfo + resultType envelope present. `server/discover` is the one
    // shape carrying BOTH: its own `ServerDiscoverResult.serverInfo` schema
    // field (which the reserved-field registry deliberately does not own) and
    // the envelope's `_meta` key that every v2 result gets.
    assert!(
        result.get("serverInfo").is_some_and(|v| v.is_object()),
        "discover result must keep its OWN serverInfo schema field: {}",
        r.body
    );
    assert!(
        result["_meta"][META_SERVER_INFO].is_object(),
        "discover result must also carry _meta[{META_SERVER_INFO}]: {}",
        r.body
    );
    assert_eq!(
        result.get("resultType").and_then(|v| v.as_str()),
        Some("complete"),
        "discover result must carry resultType:complete: {}",
        r.body
    );
    // Negotiated version + preserved request id.
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some(V2)
    );
    assert_eq!(r.body["id"], 1, "the original request id must be preserved");
    // Outbound v2 headers echoed on the accepted discover.
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
    assert_eq!(r.mcp_method.as_deref(), Some("server/discover"));
}

// The SAME rejection matrix as tools/call: v2 _meta but NO version header.
#[tokio::test]
async fn server_discover_rejects_v2_meta_without_header() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-method", "server/discover"),
            ("mcp-name", "server/discover"),
        ],
        &discover_body(Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "v2 _meta with no version header must reject");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// v2 version header but NO v2 _meta → REJECT (fail closed).
#[tokio::test]
async fn server_discover_rejects_header_without_v2_meta() {
    let (addr, handle) = spawn(true).await;
    let r = post(addr, &discover_v2_headers(), &discover_body(None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "v2 header with no v2 _meta must reject");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// Mcp-Method header disagreeing with the fixed discover body method → REJECT.
#[tokio::test]
async fn server_discover_rejects_mismatched_mcp_method() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"), // lies about the method
            ("mcp-name", "server/discover"),
        ],
        &discover_body(Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "mismatched Mcp-Method must reject");
    assert_eq!(
        r.body["error"]["code"], HEADER_MISMATCH,
        "a missing required header or a header/body disagreement is HEADER_MISMATCH"
    );
}

// Missing Mcp-Name on the v2 discover → ACCEPT (Phase 118 D-13).
//
// `server/discover` carries no routing name, so `Mcp-Name` is optional on it.
// This test asserted a 400 until Phase 118: the Phase-113 DRIFT-1 adjudication
// required the header on EVERY v2 request. D-13 reverses that to match the
// transport spec and the official conformance suite, which never sends an
// `Mcp-Name` for a name-less method. The remedy for a failure here is to fix the
// gate, not to relax the assertion back to the old rule.
#[tokio::test]
async fn server_discover_accepts_missing_mcp_name() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "server/discover"),
        ],
        &discover_body(Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "server/discover carries no routing name, so a missing Mcp-Name must be \
         ACCEPTED (Phase 118 D-13)"
    );
    assert!(
        r.body["error"].is_null(),
        "a name-less v2 method with no Mcp-Name must reach dispatch, not the gate; got {}",
        r.body
    );
}

// A v1 discover (opted-in server, NO v2 signal) returns JSON-RPC -32601 at HTTP
// 200 with the original id. This -32601@200 is the DELIBERATE, documented change
// from the pre-112 incidental PARSE_ERROR 400 (id:null) — server/discover is a
// v2-only method no conforming v1 client sends (finding #4 / D-10).
#[tokio::test]
async fn server_discover_v1_returns_method_not_found() {
    let (addr, handle) = spawn(true).await;
    let r = post(addr, &[], &discover_body(None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "v1 discover is -32601 AT HTTP 200 (D-10)");
    assert_eq!(r.body["error"]["code"], -32601);
    assert_eq!(r.body["id"], 1, "the original request id must be preserved");
}

// A NON-opted-in server also returns -32601 for server/discover.
#[tokio::test]
async fn server_discover_non_opted_in_returns_method_not_found() {
    let (addr, handle) = spawn(false).await;
    let r = post(addr, &[], &discover_body(None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200);
    assert_eq!(r.body["error"]["code"], -32601);
    assert_eq!(r.body["id"], 1);
}

// With an auth provider installed, an UNAUTHENTICATED v2 discover is rejected 401
// — proving discover is NOT bypassing auth (classify-then-continue, finding #3).
#[tokio::test]
async fn server_discover_requires_auth_when_provider_installed() {
    let (addr, handle) = spawn_with_auth().await;
    let r = post(addr, &discover_v2_headers(), &discover_body(Some(V2))).await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 401,
        "unauthenticated server/discover must be rejected 401 (no auth bypass)"
    );
}

// A valid token lets the SAME discover through — the 401 above is auth, not a
// discover-path failure.
#[tokio::test]
async fn server_discover_with_valid_token_is_served() {
    let (addr, handle) = spawn_with_auth().await;
    let mut headers = discover_v2_headers();
    headers.push(("authorization", "Bearer good-token"));
    let r = post(addr, &headers, &discover_body(Some(V2))).await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "authenticated v2 discover must be served: {}",
        r.body
    );
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
}

// With HTTP middleware installed, the discover response passes through response
// middleware — proving discover is NOT bypassing the middleware path (finding #3/#6).
#[tokio::test]
async fn server_discover_runs_response_middleware() {
    let saw = Arc::new(AtomicBool::new(false));
    let (addr, handle) = spawn_with_middleware(Arc::clone(&saw)).await;
    let r = post(addr, &discover_v2_headers(), &discover_body(Some(V2))).await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "v2 discover on the middleware path must be served: {}",
        r.body
    );
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
    assert!(
        saw.load(Ordering::SeqCst),
        "discover response must pass through response middleware (no bypass)"
    );
}
