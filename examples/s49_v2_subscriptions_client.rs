//! Example: a 2026-07-28 CLIENT receiving change notifications over
//! `subscriptions/listen`.
//!
//! Run it with:
//! ```bash
//! cargo run --example s49_v2_subscriptions_client --features full
//! ```
//!
//! Self-contained: it starts a v2 server on an ephemeral loopback port inside
//! this same process, drives a real `pmcp::Client` against it, and exits 0 when
//! every demonstration behaved as documented (non-zero otherwise). Nothing
//! leaves the machine.
//!
//! # What this demonstrates
//!
//! 1. **The stream replaces the retired RPCs.** `resources/subscribe` and
//!    `resources/unsubscribe` were REMOVED from the 2026-07-28 schema. On a v2
//!    client they now fail fast LOCALLY with an actionable error naming
//!    `subscriptions/listen` — no pointless round trip to a `404`.
//! 2. **The acknowledgement comes first, and it is the AGREED filter.** The
//!    server answers with the intersection of what you asked for and what it
//!    supports — never a superset. `subscriptions_listen` consumes that frame
//!    for you, so `acknowledged()` is populated before you poll.
//! 3. **Only what you asked for arrives.** This example advertises BOTH
//!    `tools.listChanged` and `prompts.listChanged` but subscribes to tools
//!    only, then fires both. Only the tools notification is delivered.
//! 4. **Teardown is ownership, not a method call.** Dropping the
//!    `SubscriptionStream` closes the HTTP response, which is what releases the
//!    server's registry entry and its concurrency permit. There is no `close()`
//!    to forget on an error path.
//!
//! # Which mechanism should you actually use?
//!
//! Polling over the Tasks mechanism remains pmcp's RECOMMENDED mechanism for
//! enterprise remote deployments. A held-open stream pins ONE server instance
//! for its whole lifetime and the server's subscription registry is
//! instance-local, so behind a non-sticky load balancer a subscriber silently
//! under-receives. This API exists because the spec defines it — reach for it
//! when you control the routing.

use async_trait::async_trait;
use futures::StreamExt;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::Server;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::subscriptions::SubscriptionFilter;
use pmcp::types::{PromptCapabilities, ServerCapabilities, ServerNotification, ToolCapabilities};
use pmcp::{ClientBuilder, RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use url::Url;

/// How long to wait for a notification before declaring the demo failed.
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// A trivial tool, so the server has something to change the list OF.
struct SearchTool;

#[async_trait]
impl ToolHandler for SearchTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "answer": "ok" }))
    }
}

/// A v2-opted-in server advertising BOTH list-changed capabilities.
fn build_server() -> Server {
    let mut capabilities = ServerCapabilities::default();
    capabilities.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    capabilities.prompts = Some(PromptCapabilities {
        list_changed: Some(true),
    });

    Server::builder()
        .name("s49-subscriptions")
        .version("1.0.0")
        .capabilities(capabilities)
        .with_supported_protocol_versions([
            ProtocolVersion(pmcp::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .tool("search", SearchTool)
        .build()
        .expect("server builds")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- the server ------------------------------------------------------
    let server = Arc::new(Mutex::new(build_server()));
    let bind = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let (addr, http) = StreamableHttpServer::with_config(
        bind,
        Arc::clone(&server),
        StreamableHttpServerConfig::default(),
    )
    .start()
    .await?;
    println!("server listening on http://{addr}/");

    // --- the client ------------------------------------------------------
    let url = Url::parse(&format!("http://{addr}/"))?;
    let transport =
        StreamableHttpTransport::new(StreamableHttpTransportConfigBuilder::new(url).build());
    let client = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
        .build();
    println!("client opted into {PROTOCOL_VERSION_2026_07_28} — no handshake, no session\n");

    // --- 1. the retired RPCs fail fast, locally --------------------------
    println!("1. the retired RPCs");
    let retired = client
        .subscribe_resource("mem://greeting".to_string())
        .await
        .expect_err("resources/subscribe is gone from the 2026-07-28 schema");
    assert!(retired.is_retired_on_v2(), "expected the typed error");
    println!("   resources/subscribe -> {retired}");
    println!("   (nothing was sent: the client refused before touching the wire)\n");

    // --- 2. open the stream; the ack is already consumed -----------------
    println!("2. subscriptions/listen");
    let requested = SubscriptionFilter {
        tools_list_changed: Some(true),
        ..SubscriptionFilter::default()
    };
    let mut stream = client.subscriptions_listen(requested).await?;
    println!("   subscriptionId : {}", stream.subscription_id());
    println!(
        "   agreed filter  : {}",
        serde_json::to_string(&stream.acknowledged().notifications)?
    );
    assert_eq!(
        stream.acknowledged().notifications.prompts_list_changed,
        None,
        "the server advertises promptsListChanged but we did not ask for it"
    );
    println!("   (promptsListChanged is advertised but NOT agreed — we never asked)\n");

    // --- 3. fire both; receive only the subscribed one -------------------
    println!("3. change notifications");
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::PromptsChanged)
            .await;
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }
    println!("   fired: prompts/list_changed, then tools/list_changed");

    let delivered = tokio::time::timeout(FRAME_TIMEOUT, stream.next())
        .await
        .map_err(|_| "no notification arrived within the timeout")?
        .ok_or("the stream ended before delivering anything")??;
    println!("   received: {delivered:?}");
    assert!(
        matches!(delivered, ServerNotification::ToolsChanged),
        "only the subscribed type may be delivered"
    );

    // Nothing else is coming: the prompts notification was filtered SERVER-side.
    let extra = tokio::time::timeout(Duration::from_millis(300), stream.next()).await;
    assert!(
        extra.is_err(),
        "an unrequested notification reached the client: {extra:?}"
    );
    println!("   nothing else arrived — the unrequested type was filtered\n");

    // --- 4. teardown is ownership ----------------------------------------
    println!("4. teardown");
    drop(stream);
    println!("   dropped the stream: the HTTP response closed, so the server's");
    println!("   registry entry and concurrency permit were reclaimed by RAII\n");

    http.abort();
    println!("all demonstrations behaved as documented");
    Ok(())
}
