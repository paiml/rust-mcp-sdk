//! Phase 113-13 (HTTP-04 / CLNT-01): a REAL pmcp v2 `Client` receives change
//! notifications from a REAL pmcp v2 server.
//!
//! Plan 10 proved the WIRE with a raw HTTP/1.1 client. That does not demonstrate
//! what HTTP-04's requirement text actually asks for — "**v2 clients get change
//! notifications** via a `subscriptions/listen` long-lived stream" — whose
//! grammatical subject is the CLIENT. Every test in this file therefore drives
//! `pmcp::Client::subscriptions_listen` against a `StreamableHttpServer` over a
//! loopback TCP socket, and reads the notifications back as typed
//! [`ServerNotification`]s.
//!
//! # Reliability doctrine
//!
//! Carried from `tests/v2_required_headers.rs` and `tests/v2_subscriptions.rs`:
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round trip). EVERY stream poll goes through [`next_frame`] or
//! [`expect_no_frame`], both of which are `tokio::time::timeout`-bounded, so a
//! hung or never-acknowledged stream FAILS the test instead of wedging CI
//! (T-113-36).
//!
//! # Why some servers here carry an auth provider
//!
//! The per-principal stream cap keys off the authenticated subject. Plan 10's
//! `anonymous_principal` is a PER-STREAM counter, so for an unauthenticated
//! caller the per-principal cap never binds and only the (much larger) global
//! cap does. `client_stream_drop_releases_server_slot` therefore authenticates,
//! which makes the cap reachable in a handful of streams.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, build_v2_server_with, extensions_capabilities, spawn_default_config,
    spawn_shared, BearerSubjects, SearchTool, FRAME_TIMEOUT, V1, V2,
};

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use pmcp::client::subscriptions::SubscriptionStream;
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpRequest,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::server::Server;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::types::protocol::error_codes::METHOD_NOT_FOUND;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::subscriptions::SubscriptionFilter;
use pmcp::types::{
    ClientCapabilities, PromptCapabilities, ResourceCapabilities, ServerCapabilities,
    ServerNotification, ToolCapabilities,
};
use pmcp::{Client, ClientBuilder};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use url::Url;

/// The `mem://` resource `GreetingResource` serves.
const RESOURCE_URI: &str = "mem://greeting";

// ===========================================================================
// Servers.
// ===========================================================================

/// Capabilities advertising `tools.listChanged` (and optionally
/// `prompts.listChanged` / `resources.subscribe`), on top of the shared
/// harness's extensions map so `server/discover` still works.
fn advertising(prompts: bool, resource_subscribe: bool) -> ServerCapabilities {
    let mut caps = extensions_capabilities();
    caps.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    if prompts {
        caps.prompts = Some(PromptCapabilities {
            list_changed: Some(true),
        });
    }
    if resource_subscribe {
        caps.resources = Some(ResourceCapabilities {
            subscribe: Some(true),
            list_changed: Some(true),
        });
    }
    caps
}

/// A v2-opted-in server with `caps` and one real handler per method.
fn server_with(caps: ServerCapabilities) -> Server {
    build_v2_server_with("v2-subscriptions-client", caps)
}

/// An authenticated server advertising `tools.listChanged`.
fn authenticated_server() -> Server {
    Server::builder()
        .name("v2-subscriptions-client-auth")
        .version("1.0.0")
        .capabilities(advertising(false, false))
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .auth_provider(BearerSubjects)
        .tool("search", SearchTool)
        .build()
        .expect("server builds")
}

/// Spawn a server this test does not need a handle to.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(server).await
}

/// Counts the JSON-RPC methods that actually arrived at the HTTP boundary.
///
/// Preferred over log parsing: the fact under observation — "did a
/// `resources/subscribe` request reach the server at all?" — lives at the
/// transport layer, and a count of `0` is the only way to prove a LOCAL
/// fail-fast rather than a server-side rejection.
#[derive(Debug, Default)]
struct MethodCounts {
    subscribe: AtomicUsize,
    unsubscribe: AtomicUsize,
    total: AtomicUsize,
}

struct CountingMiddleware {
    counts: Arc<MethodCounts>,
}

#[async_trait]
impl ServerHttpMiddleware for CountingMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        self.counts.total.fetch_add(1, Ordering::SeqCst);
        let body = String::from_utf8_lossy(&request.body);
        if body.contains("\"resources/subscribe\"") {
            self.counts.subscribe.fetch_add(1, Ordering::SeqCst);
        }
        if body.contains("\"resources/unsubscribe\"") {
            self.counts.unsubscribe.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Spawn `server` behind a [`CountingMiddleware`].
async fn spawn_counting(server: Server) -> (SocketAddr, JoinHandle<()>, Arc<MethodCounts>) {
    let counts = Arc::new(MethodCounts::default());
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(CountingMiddleware {
        counts: Arc::clone(&counts),
    }));
    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };
    let (addr, handle) = common::v2::spawn_with(server, config).await;
    (addr, handle, counts)
}

// ===========================================================================
// Clients.
// ===========================================================================

fn transport_for(addr: SocketAddr, bearer: Option<&str>) -> StreamableHttpTransport {
    let url = Url::parse(&format!("http://{addr}/")).expect("loopback URL parses");
    let mut builder = StreamableHttpTransportConfigBuilder::new(url);
    if let Some(bearer) = bearer {
        builder = builder.with_header("authorization", format!("Bearer {bearer}"));
    }
    StreamableHttpTransport::new(builder.build())
}

/// A pmcp client that OPTED INTO `2026-07-28`.
fn v2_client(addr: SocketAddr, bearer: Option<&str>) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr, bearer))
        .with_protocol_version(ProtocolVersion(V2.to_string()))
        .expect("2026-07-28 is selectable")
        .build()
}

/// A pmcp client that made NO era selection — today's handshake behavior.
fn v1_client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr, None)).build()
}

/// Request only `toolsListChanged`.
fn tools_only() -> SubscriptionFilter {
    SubscriptionFilter {
        tools_list_changed: Some(true),
        ..SubscriptionFilter::default()
    }
}

// ===========================================================================
// Timeout-bounded polling.
// ===========================================================================

/// Poll the stream for one item, failing the test rather than hanging.
async fn next_frame(stream: &mut SubscriptionStream) -> Option<pmcp::Result<ServerNotification>> {
    tokio::time::timeout(FRAME_TIMEOUT, stream.next())
        .await
        .expect("a subscriptions/listen frame must arrive within the timeout")
}

/// Assert that NOTHING arrives on the stream within `window`.
async fn expect_no_frame(stream: &mut SubscriptionStream, window: Duration) {
    if let Ok(Some(item)) = tokio::time::timeout(window, stream.next()).await {
        panic!("an unrequested notification reached the client: {item:?}");
    }
}

// ===========================================================================
// Tests.
// ===========================================================================

/// The acknowledgement is consumed by `subscriptions_listen` itself, so its
/// AGREED filter and the subscription id are readable BEFORE the first poll.
#[tokio::test]
async fn client_receives_acknowledgement_first() {
    let (addr, handle) = spawn(server_with(advertising(true, true))).await;
    let client = v2_client(addr, None);

    let stream = client
        .subscriptions_listen(tools_only())
        .await
        .expect("an advertising server serves the stream");

    assert_eq!(
        stream.acknowledged().notifications,
        SubscriptionFilter {
            tools_list_changed: Some(true),
            ..SubscriptionFilter::default()
        },
        "the ack reports the AGREED filter, never a superset of the request"
    );
    // The client minted the id, so this is also the proof that the tag the
    // server echoed is the one this very request carried.
    let id = stream.subscription_id().to_string();
    assert!(!id.is_empty(), "the stream knows its own subscription id");

    drop(stream);
    handle.abort();
}

/// The requirement's own subject: a real change notification, triggered through
/// the server's REAL notification path, reaches a real pmcp client.
#[tokio::test]
async fn client_receives_tools_list_changed() {
    let server = Arc::new(Mutex::new(server_with(advertising(false, false))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;
    let client = v2_client(addr, None);

    let mut stream = client
        .subscriptions_listen(tools_only())
        .await
        .expect("the stream is served");

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let notification = next_frame(&mut stream)
        .await
        .expect("a notification arrives")
        .expect("and it decodes");
    assert!(
        matches!(notification, ServerNotification::ToolsChanged),
        "the client receives the tools/list_changed it subscribed to: {notification:?}"
    );

    drop(stream);
    handle.abort();
}

/// TRIPWIRE for the fresh-id reconnect contract (113-18, T-113-86).
///
/// The server refuses a second LIVE registration under a
/// `(principal, subscriptionId)` pair it already holds, and it cannot tell an
/// ungracefully disconnected peer from a live one — the receiver and the
/// registry guard live in ONE `stream::unfold` state tuple, so the entry
/// survives until Hyper drops the response body, at which point RAII has already
/// reclaimed it. The property that makes a pmcp client immune to that is
/// therefore NOT a server-side liveness probe (which is not implementable at
/// that layer) but the client's own guarantee that
/// `Client::subscriptions_listen` mints a FRESH `Uuid::new_v4()` id per call.
///
/// This test makes that guarantee a CHECKED property rather than a claim. Both
/// streams present the SAME bearer, so they resolve to ONE server-side principal
/// — the exact configuration in which a sticky or counter-derived id WOULD
/// collide. Make the id constant and this test fails twice over: the ids compare
/// equal, and the second `subscriptions_listen` is refused outright.
///
/// The per-stream tagging is proven by construction rather than by reading
/// `_meta` here: `SubscriptionStream` yields a frame as `Ok` only when the
/// frame's `subscriptionId` matches ITS OWN id, and surfaces a mismatch as an
/// `Err` item (T-113-66). Two `Ok(ToolsChanged)` from one `fan_out` is therefore
/// exactly "each stream received a frame tagged with its own id".
#[tokio::test]
async fn successive_listen_calls_mint_distinct_subscription_ids() {
    let server = Arc::new(Mutex::new(authenticated_server()));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;
    // ONE bearer for BOTH streams: `BearerSubjects` maps it to one subject, so
    // both registrations land on the same principal.
    let client = v2_client(addr, Some("alice"));

    let mut first = client
        .subscriptions_listen(tools_only())
        .await
        .expect("the first stream is served");
    let mut second = client.subscriptions_listen(tools_only()).await.expect(
        "the second stream is served too: its id is FRESH, so it cannot collide \
         with the first — which the server still holds LIVE under the same principal",
    );

    assert_ne!(
        first.subscription_id(),
        second.subscription_id(),
        "every subscriptions_listen call MUST mint a fresh subscription id; a \
         sticky or counter-derived id would collide with a live incumbent on \
         reconnect and be refused for the rest of the keep-alive window"
    );

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    for (label, stream) in [("first", &mut first), ("second", &mut second)] {
        let notification = next_frame(stream)
            .await
            .unwrap_or_else(|| panic!("{label}: a notification must arrive"))
            .unwrap_or_else(|e| {
                panic!("{label}: the frame must be tagged with THIS stream's id: {e}")
            });
        assert!(
            matches!(notification, ServerNotification::ToolsChanged),
            "{label}: both streams coexist and both receive the fan-out: {notification:?}"
        );
    }

    drop(first);
    drop(second);
    handle.abort();
}

/// A notification type the client did not request never reaches it — the
/// client-side half of T-113-34.
#[tokio::test]
async fn client_does_not_receive_unrequested_types() {
    // TWO capabilities advertised, only ONE requested.
    let server = Arc::new(Mutex::new(server_with(advertising(true, false))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;
    let client = v2_client(addr, None);

    let mut stream = client
        .subscriptions_listen(tools_only())
        .await
        .expect("the stream is served");
    assert_eq!(
        stream.acknowledged().notifications.prompts_list_changed,
        None,
        "an unrequested type is OMITTED from the agreed filter"
    );

    // Trigger BOTH, prompts FIRST.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::PromptsChanged)
            .await;
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    let delivered = next_frame(&mut stream)
        .await
        .expect("a notification arrives")
        .expect("and it decodes");
    assert!(
        matches!(delivered, ServerNotification::ToolsChanged),
        "only the REQUESTED type appears, and it appears FIRST despite prompts \
         being triggered first: {delivered:?}"
    );
    expect_no_frame(&mut stream, Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
}

/// Dropping the client handle closes the HTTP response, which fires the
/// server's RAII `ListenGuard` and reclaims its slot — the client-side half of
/// plan 10's `disconnect_releases_registry_slot` (T-113-63).
#[tokio::test]
async fn client_stream_drop_releases_server_slot() {
    let (addr, handle) = spawn(authenticated_server()).await;
    let client = v2_client(addr, Some("capped"));

    // Walk up to the per-principal cap (a private constant) until refused.
    let mut held: Vec<SubscriptionStream> = Vec::new();
    let mut refusal = None;
    for _ in 0..16 {
        match client.subscriptions_listen(tools_only()).await {
            Ok(stream) => held.push(stream),
            Err(e) => {
                refusal = Some(e);
                break;
            },
        }
    }
    let refusal = refusal.expect("the per-principal cap must refuse an N+1th stream");
    assert!(
        refusal.to_string().contains("too many concurrent"),
        "the refusal names the concurrency bound: {refusal}"
    );
    assert!(!held.is_empty(), "some streams were accepted first");

    // Disconnect ONE client and let the server observe the closed socket.
    drop(held.pop().expect("at least one open stream"));

    // The reclaim is asynchronous (the server has to notice the dropped
    // connection), so poll for it rather than sleeping blindly.
    let mut accepted = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(stream) = client.subscriptions_listen(tools_only()).await {
            held.push(stream);
            accepted = true;
            break;
        }
    }
    assert!(
        accepted,
        "dropping a SubscriptionStream must release the server's registry entry AND its permit"
    );

    drop(held);
    handle.abort();
}

/// Against a server that advertises NO subscription-delivered capability, the
/// server's own `-32601` reaches the caller — not a hang, not a panic, and not
/// an opaque transport error.
#[tokio::test]
async fn client_listen_against_non_advertising_server_errors() {
    // `build_v2_server` advertises the extensions map only; handler registration
    // fills the sub-capabilities with `Some(false)`.
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr, None);

    let error = client
        .subscriptions_listen(tools_only())
        .await
        .expect_err("a non-advertising server does not serve the stream");

    match error {
        pmcp::Error::Protocol { code, .. } => assert_eq!(
            code.as_i32(),
            METHOD_NOT_FOUND,
            "the server's structured -32601 reaches the caller unchanged"
        ),
        other => panic!("expected a structured protocol error, got {other:?}"),
    }

    handle.abort();
}

/// The retired RPC fails LOCALLY: the server records ZERO
/// `resources/subscribe` requests.
#[tokio::test]
async fn client_subscribe_resource_retired_on_v2() {
    let (addr, handle, counts) = spawn_counting(server_with(advertising(false, true))).await;
    let client = v2_client(addr, None);

    for (method, result) in [
        (
            "resources/subscribe",
            client.subscribe_resource(RESOURCE_URI.to_string()).await,
        ),
        (
            "resources/unsubscribe",
            client.unsubscribe_resource(RESOURCE_URI.to_string()).await,
        ),
    ] {
        let error = result.expect_err("the RPC is gone from the 2026-07-28 schema");
        assert!(error.is_retired_on_v2(), "{method}: {error}");
        assert_eq!(error.retired_method(), Some(method));
        assert!(
            error.to_string().contains("subscriptions/listen"),
            "{method}: the error names the replacement: {error}"
        );
    }

    assert_eq!(
        counts.subscribe.load(Ordering::SeqCst),
        0,
        "a v2 resources/subscribe must never reach the server"
    );
    assert_eq!(
        counts.unsubscribe.load(Ordering::SeqCst),
        0,
        "a v2 resources/unsubscribe must never reach the server"
    );

    // Guard against a vacuous assertion: the SAME client on the SAME server does
    // reach it for a method that still exists.
    client
        .list_tools(None)
        .await
        .expect("a live v2 request still works");
    assert!(
        counts.total.load(Ordering::SeqCst) > 0,
        "traffic must have reached the server, or the counts above prove nothing"
    );

    handle.abort();
}

/// The v1 path is untouched: `subscribe_resource` still works end to end.
#[tokio::test]
async fn v1_client_subscribe_unchanged() {
    let (addr, handle, counts) = spawn_counting(server_with(advertising(false, true))).await;
    let mut client = v1_client(addr);

    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("the v1 handshake still works");

    client
        .subscribe_resource(RESOURCE_URI.to_string())
        .await
        .expect("v1 resources/subscribe is unchanged");

    assert_eq!(
        counts.subscribe.load(Ordering::SeqCst),
        1,
        "a v1 subscribe really does travel to the server"
    );

    handle.abort();
}
