//! Example: MCP `2026-07-28` CACHING HINTS and NON-OBJECT structured output.
//!
//! Run this server with:
//! ```bash
//! cargo run --example s52_v2_caching_hints --features full
//! ```
//!
//! Self-contained and non-interactive: it starts a v2-opted-in server on an
//! ephemeral loopback port inside this same process, issues every request
//! itself, prints the raw JSON-RPC responses, ASSERTS each behaviour it claims,
//! and exits 0 (non-zero if any claim fails). Nothing leaves the machine and
//! nothing waits for input.
//!
//! # What this demonstrates
//!
//! - **A server-set cache posture.** The `ResourceHandler` below returns its
//!   `ListResourcesResult` with `.with_ttl_ms(300_000).with_cache_scope(
//!   CacheScope::Public)` — a five-minute freshness hint on a CATALOGUE that
//!   contains no user-specific data. Its `ReadResourceResult` for a per-user
//!   document is left at the SDK default, deliberately.
//! - **The SDK default, and why it is inert.** `tools/list` carries
//!   `ttlMs: 0` / `cacheScope: "private"` on v2 without the author doing
//!   anything. `0` means "SHOULD be considered immediately stale", so the
//!   default asserts NOTHING about cacheability: a conformant peer receiving it
//!   behaves exactly as it would have without the field, while the v2 wire's
//!   requirement that both keys be present is still satisfied (D-08).
//! - **Era gating.** The SAME server answering a `2025-11-25` request emits
//!   NEITHER key. The hints live on the v2 projection only, and the projection
//!   actively STRIPS them rather than merely not adding them — so a handler that
//!   sets a hint and then serves a legacy client still emits a byte-identical
//!   legacy response (D-11).
//! - **Non-object structured output.** The `answer` tool declares
//!   `{"type": "integer"}` as its `outputSchema` and its result carries
//!   `"structuredContent": 42` — a bare scalar, which `2026-07-28` permits and
//!   which `CallToolResult::structured_value` is the constructor for (SCHM-02).
//!
//! # SECURITY: what `cacheScope: "public"` actually authorizes
//!
//! Read this before copying the `with_cache_scope(CacheScope::Public)` line.
//!
//! `"public"` tells every client and intermediary — a shared gateway, a caching
//! proxy — that it MAY cache the response and serve it **across authorization
//! contexts**. A different caller, holding a different access token, can be
//! handed the bytes your server produced for someone else, and your server is
//! what said that was allowed. Marking a per-user response `Public` is therefore
//! a cross-authorization-context data leak, not a performance tuning mistake.
//!
//! Use `Public` only when the body is IDENTICAL for every caller regardless of
//! identity, token or tenant — a static catalogue, a version manifest, a public
//! schema. Everything else is `Private`, which is also the SDK default, so the
//! value that cannot leak is the one you get for free. When in doubt, do
//! nothing.

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::server::{ResourceHandler, Server};
use pmcp::shared::http_constants::{MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION, MCP_SESSION_ID};
use pmcp::testing::{META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION};
use pmcp::types::protocol::{
    ProtocolVersion, RequestMeta, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::{
    CacheScope, CallToolResult, Content, ListResourcesResult, ReadResourceResult, ResourceInfo,
    DEFAULT_TTL_MS,
};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// The catalogue's freshness hint: five minutes, in milliseconds.
///
/// `ttlMs` is a CACHE-FRESHNESS hint — how long a client may reuse this
/// response body. It is NOT a task lifetime; copying a long task TTL in here
/// would make stale data look fresh.
const CATALOGUE_TTL_MS: u64 = 300_000;

// The SDK-supplied defaults are IMPORTED, not restated: `DEFAULT_TTL_MS`
// (immediately stale, therefore inert) comes from `pmcp::types`, and the
// default `cacheScope` is `CacheScope::default()` — the value that cannot leak.
// Typing `0` and `"private"` here would let this example keep asserting the old
// defaults if the SDK ever changed them, which is the opposite of what a
// reference example is for.

/// The catalogue resource — no user-specific data, so it may be shared.
const CATALOGUE_URI: &str = "docs://catalogue";

/// A per-user document. Its body differs per caller, so its posture stays at
/// the default.
const PROFILE_URI: &str = "docs://me/profile";

// ===========================================================================
// The server.
// ===========================================================================

/// One handler serving both a shareable catalogue and a per-user document, so
/// the contrast between the two postures is visible in a single file.
struct CatalogueAndProfile;

#[async_trait]
impl ResourceHandler for CatalogueAndProfile {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        // The catalogue is byte-identical for every caller — no names, no
        // tenant ids, nothing derived from the access token — so it is safe to
        // let a shared gateway serve it ACROSS AUTHORIZATION CONTEXTS. That is
        // exactly what `Public` authorizes, and it is the whole reason this
        // line needs a moment's thought rather than a copy-paste: were this
        // list filtered per caller, `Public` would hand one caller's view to
        // another.
        Ok(ListResourcesResult::new(vec![
            ResourceInfo::new(CATALOGUE_URI, "catalogue")
                .with_description("A public document catalogue — identical for every caller"),
            ResourceInfo::new(PROFILE_URI, "profile")
                .with_description("The calling user's own profile — never shareable"),
        ])
        .with_ttl_ms(CATALOGUE_TTL_MS)
        .with_cache_scope(CacheScope::Public))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        // DELIBERATELY left at the SDK default. This body is per-user, so the
        // safe answer is `ttlMs: 0` / `cacheScope: "private"` — which the SDK
        // supplies without the handler saying anything. Expressing no
        // preference is the correct thing to do here, not an oversight.
        Ok(ReadResourceResult::new(vec![Content::text(format!(
            "contents of {uri}"
        ))]))
    }
}

/// A v2-opted-in server with one resource handler and one scalar-returning tool.
///
/// `.with_supported_protocol_versions([..])` is the OPT-IN: without it the
/// server does not accept `2026-07-28` at all, and every response below would
/// be a v1 response.
fn build_server() -> Server {
    // The handler returns the bare scalar; the dispatcher bridges it into
    // `structuredContent` because the tool DECLARES an `outputSchema`. A
    // handler that builds its own `CallToolResult` uses
    // `CallToolResult::structured_value` for the same effect — demonstrated
    // directly in `demonstrate_structured_value_constructor` below.
    let answer = TypedToolWithOutput::new_with_schemas(
        "answer".to_string(),
        json!({ "type": "object" }),
        Some(json!({ "type": "integer" })),
        |_args: Value, _extra| Box::pin(async move { Ok(json!(42)) }),
    );

    Server::builder()
        .name("s52-caching-hints")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .tool("answer", answer)
        .resources(CatalogueAndProfile)
        .build()
        .expect("server builds")
}

// ===========================================================================
// A minimal JSON-RPC-over-HTTP client, so the RAW wire is visible.
// ===========================================================================

/// POST a `2026-07-28` request: the era is declared in `params._meta`, which is
/// why no `initialize` handshake appears anywhere in this file.
async fn post_v2(
    client: &reqwest::Client,
    addr: SocketAddr,
    method: &str,
    name: &str,
    id: u64,
    params: Value,
) -> Value {
    let meta = RequestMeta::new()
        .with_meta(META_PROTOCOL_VERSION, json!(PROTOCOL_VERSION_2026_07_28))
        .with_meta(
            META_CLIENT_INFO,
            json!({ "name": "s52-example", "version": "1.0.0" }),
        )
        .with_meta(META_CLIENT_CAPABILITIES, json!({}));
    let mut params = params;
    if let Some(object) = params.as_object_mut() {
        object.insert(
            "_meta".to_string(),
            serde_json::to_value(&meta).expect("request meta serializes"),
        );
    }

    let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let response = client
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header(MCP_METHOD, method)
        .header(MCP_NAME, name)
        .header(MCP_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28)
        .body(body.to_string())
        .send()
        .await
        .expect("the loopback server answers");
    parse(response, method).await
}

/// POST a `2025-11-25` request: no reserved `_meta`, no v2 headers. This is a
/// legacy client, byte for byte.
///
/// Returns the response's `Mcp-Session-Id` alongside the result, because a v1
/// client on the STATEFUL default config must complete an `initialize`
/// handshake and echo the minted session on every later request. The contrast
/// with [`post_v2`] — which sends neither — is the per-request era gate doing
/// its job on one running server.
async fn post_v1(
    client: &reqwest::Client,
    addr: SocketAddr,
    method: &str,
    id: u64,
    params: Value,
    session: Option<&str>,
) -> (Option<String>, Value) {
    let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let mut request = client
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(session_id) = session {
        request = request.header(MCP_SESSION_ID, session_id);
    }
    let response = request
        .body(body.to_string())
        .send()
        .await
        .expect("the loopback server answers");
    let minted = response
        .headers()
        .get(MCP_SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    (minted, parse(response, method).await)
}

/// Read a response body as JSON, unwrapping an SSE frame if the transport chose
/// one, and return its `result` object.
async fn parse(response: reqwest::Response, method: &str) -> Value {
    let status = response.status();
    let raw = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "{method} returned HTTP {status}; body was {raw}"
    );

    // The streamable-http transport may answer with a single SSE frame.
    let payload = raw
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .unwrap_or(&raw);
    let body: Value =
        serde_json::from_str(payload).unwrap_or_else(|e| panic!("{method}: {e}; body was {raw}"));
    assert!(
        body.get("error").is_none(),
        "{method} returned a JSON-RPC error: {body}"
    );
    body.get("result")
        .cloned()
        .unwrap_or_else(|| panic!("{method} carries no result: {body}"))
}

/// Print a result with the two caching keys called out.
fn show(label: &str, result: &Value) {
    println!("  {label}");
    println!(
        "    ttlMs       = {}",
        result
            .get("ttlMs")
            .map_or_else(|| "<absent>".to_string(), ToString::to_string)
    );
    println!(
        "    cacheScope  = {}",
        result
            .get("cacheScope")
            .map_or_else(|| "<absent>".to_string(), ToString::to_string)
    );
    println!("    raw         = {result}");
}

/// The constructor half of SCHM-02, shown in process rather than over the wire.
///
/// A tool whose handler returns a plain value lets the dispatcher build the
/// `CallToolResult` (that is what the `answer` tool does, and section 4 below
/// shows its wire). A handler that builds its OWN result reaches for
/// `CallToolResult::structured_value`, which names the non-object case at the
/// call site instead of leaving a reader to wonder whether a scalar was
/// intended.
fn demonstrate_structured_value_constructor() {
    let result = CallToolResult::structured_value(json!(42));
    assert_eq!(
        result.structured_content,
        Some(json!(42)),
        "structured_value must carry a bare scalar verbatim"
    );

    let wire = serde_json::to_string(&result).expect("a result serializes");
    assert!(
        wire.contains(r#""structuredContent":42"#),
        "the scalar must reach the wire unwrapped, got {wire}"
    );
    println!("  CallToolResult::structured_value(json!(42)) => {wire}");

    // `null` is a value, not an absence: the key is EMITTED explicitly.
    let null_result = CallToolResult::structured_value(Value::Null);
    let null_wire = serde_json::to_string(&null_result).expect("a result serializes");
    assert!(
        null_wire.contains(r#""structuredContent":null"#),
        "Some(Value::Null) must emit an explicit null rather than be omitted, got {null_wire}"
    );
    println!("  CallToolResult::structured_value(json!(null)) => {null_wire}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Arc::new(Mutex::new(build_server()));
    let bind = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let (addr, http) = StreamableHttpServer::with_config(
        bind,
        Arc::clone(&server),
        StreamableHttpServerConfig::default(),
    )
    .start()
    .await?;
    println!("server listening on http://{addr}/\n");

    let client = reqwest::Client::new();

    // --- 1. a handler-set posture ----------------------------------------
    println!("1. resources/list — the HANDLER set the posture (v2)");
    let list = post_v2(&client, addr, "resources/list", "", 1, json!({})).await;
    show("resources/list", &list);
    assert_eq!(
        list.get("ttlMs"),
        Some(&json!(CATALOGUE_TTL_MS)),
        "the handler's five-minute freshness hint must survive the projection verbatim"
    );
    assert_eq!(
        list.get("cacheScope"),
        Some(&json!("public")),
        "the handler's scope must survive the projection verbatim"
    );
    println!("    -> `public` authorizes a shared gateway to serve this body");
    println!("       across authorization contexts. Correct here (the catalogue is identical");
    println!("       for every caller); a data leak on a per-user body.\n");

    // --- 2. the SDK default, on a body the handler said nothing about ----
    println!("2. resources/read and tools/list — the SDK DEFAULT (v2)");
    let read = post_v2(
        &client,
        addr,
        "resources/read",
        PROFILE_URI,
        2,
        json!({ "uri": PROFILE_URI }),
    )
    .await;
    show("resources/read", &read);
    let tools = post_v2(&client, addr, "tools/list", "", 3, json!({})).await;
    show("tools/list", &tools);
    for (label, result) in [("resources/read", &read), ("tools/list", &tools)] {
        assert_eq!(
            result.get("ttlMs"),
            Some(&json!(DEFAULT_TTL_MS)),
            "{label} must carry the SDK default ttlMs on v2"
        );
        assert_eq!(
            result.get("cacheScope"),
            Some(&json!(CacheScope::default())),
            "{label} must carry the SDK default cacheScope on v2"
        );
    }
    println!("    -> `ttlMs: 0` means \"immediately stale\", so the default asserts NOTHING");
    println!("       about cacheability — inert, yet the v2 wire's required keys are present.\n");

    // --- 3. the era gate --------------------------------------------------
    println!("3. the SAME server answering a 2025-11-25 client — NEITHER key");
    // A legacy client DOES handshake, and DOES carry a session — the two things
    // the v2 requests above did without. Same process, same server, same port.
    let (session, _init) = post_v1(
        &client,
        addr,
        "initialize",
        4,
        json!({
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "s52-example", "version": "1.0.0" }
        }),
        None,
    )
    .await;
    let session = session.expect("a 2025-11-25 initialize mints an Mcp-Session-Id");
    println!("  (v1 handshake completed; session {session} — v2 needed neither)");

    let (_, v1_list) = post_v1(
        &client,
        addr,
        "resources/list",
        5,
        json!({}),
        Some(&session),
    )
    .await;
    show("resources/list (v1)", &v1_list);
    let (_, v1_tools) = post_v1(&client, addr, "tools/list", 6, json!({}), Some(&session)).await;
    show("tools/list (v1)", &v1_tools);
    for (label, result) in [("resources/list", &v1_list), ("tools/list", &v1_tools)] {
        assert!(
            result.get("ttlMs").is_none(),
            "{label}: a v1 response must never carry ttlMs, got {result}"
        );
        assert!(
            result.get("cacheScope").is_none(),
            "{label}: a v1 response must never carry cacheScope, got {result}"
        );
    }
    println!("    -> the handler SET a hint on resources/list, and the v1 projection STRIPPED it.");
    println!("       Not \"did not add\" — actively removed, so the legacy wire is unchanged.\n");

    // --- 4. non-object structured output ---------------------------------
    println!("4. tools/call — NON-OBJECT structuredContent (v2)");
    let call = post_v2(
        &client,
        addr,
        "tools/call",
        "answer",
        7,
        json!({ "name": "answer", "arguments": {} }),
    )
    .await;
    println!("    raw         = {call}");
    assert_eq!(
        call.get("structuredContent"),
        Some(&json!(42)),
        "a tool declaring {{\"type\": \"integer\"}} must emit a BARE scalar, not an object \
         wrapper, got {call}"
    );
    assert!(
        call.get("ttlMs").is_none() && call.get("cacheScope").is_none(),
        "tools/call is not a CacheableResult and must carry neither hint, got {call}"
    );
    println!("    -> `structuredContent: 42`. A scalar, not `{{\"value\": 42}}`.");
    demonstrate_structured_value_constructor();

    // --- teardown ---------------------------------------------------------
    http.abort();
    // Give the accept loop a moment to unwind before the process exits.
    tokio::time::sleep(Duration::from_millis(50)).await;
    println!("\nall four demonstrations asserted — exiting 0");
    Ok(())
}
