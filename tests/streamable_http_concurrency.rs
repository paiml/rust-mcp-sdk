//! The streamable-HTTP transport serves requests concurrently (pmcp 2.22.7
//! field report F-4; the mechanism is recorded in `server_handle.rs`).
//!
//! No timing assumptions: the rendezvous tool can only return once N calls are
//! inside it at the same moment, which a serializing transport makes impossible.
//! Every scenario also runs against the legacy `Arc<Mutex<Server>>` constructor,
//! which still serializes by contract; that run is the positive control proving
//! the scenario can tell the two apart.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::v2::{post, result_of, spawn_concurrent_with, spawn_with, v1_body, Resp};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use proptest::prelude::*;
use serde_json::json;
use tokio::sync::{Barrier, Notify};
use tokio::task::JoinHandle;

/// Generous: only ever reached when the transport is serializing.
const CONCURRENT_DEADLINE: Duration = Duration::from_secs(10);
/// How long the positive controls wait before declaring "blocked".
const SERIAL_PROBE: Duration = Duration::from_millis(500);

/// Returns only once `barrier`'s full party count is inside `handle` at once.
struct RendezvousTool {
    barrier: Arc<Barrier>,
}

#[async_trait]
impl ToolHandler for RendezvousTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        self.barrier.wait().await;
        Ok(json!({ "met": true }))
    }
}

/// Stays in flight until `release` is notified, standing in for a long call.
struct ParkedTool {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl ToolHandler for ParkedTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(json!({ "released": true }))
    }
}

/// Which public constructor a scenario drives, and so what it must observe.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Concurrent,
    Legacy,
}

impl Shape {
    /// The concurrent path must finish well inside the deadline; the legacy
    /// path is only probed long enough to show it is blocked.
    const fn wait(self) -> Duration {
        match self {
            Self::Concurrent => CONCURRENT_DEADLINE,
            Self::Legacy => SERIAL_PROBE,
        }
    }
}

struct Fixture {
    barrier: Arc<Barrier>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl Fixture {
    fn new(parties: usize) -> Self {
        Self {
            barrier: Arc::new(Barrier::new(parties)),
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }

    async fn start(&self, shape: Shape) -> (SocketAddr, JoinHandle<()>) {
        let server = Server::builder()
            .name("concurrency-test")
            .version("1.0.0")
            .tool(
                "rendezvous",
                RendezvousTool {
                    barrier: Arc::clone(&self.barrier),
                },
            )
            .tool(
                "parked",
                ParkedTool {
                    entered: Arc::clone(&self.entered),
                    release: Arc::clone(&self.release),
                },
            )
            .build()
            .expect("server builds");
        let config = StreamableHttpServerConfig::stateless();
        match shape {
            Shape::Concurrent => spawn_concurrent_with(server, config).await,
            Shape::Legacy => spawn_with(server, config).await,
        }
    }
}

async fn call_tool(addr: SocketAddr, id: u64, tool: &str) -> Resp {
    let params = json!({ "name": tool, "arguments": {} });
    post(addr, &[], &v1_body("tools/call", json!(id), params)).await
}

async fn initialize(addr: SocketAddr) -> Resp {
    let params = json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "concurrency-test", "version": "1.0.0" },
    });
    post(addr, &[], &v1_body("initialize", json!(0), params)).await
}

/// Fire `parties` rendezvous calls at once. `Some` iff they all met in time.
async fn rendezvous(shape: Shape, parties: usize) -> Option<Vec<Resp>> {
    let fixture = Fixture::new(parties);
    let (addr, task) = fixture.start(shape).await;
    let calls = (1..=parties as u64).map(|id| call_tool(addr, id, "rendezvous"));
    let met = tokio::time::timeout(shape.wait(), futures::future::join_all(calls))
        .await
        .ok();
    task.abort();
    if let Some(responses) = &met {
        for resp in responses {
            let text = result_of(resp)["content"][0]["text"].as_str();
            assert!(text.is_some_and(|t| t.contains("\"met\"")), "{}", resp.raw);
        }
    }
    met
}

/// Park one long call, then try `initialize`. `true` iff it was answered in time.
async fn initialize_during_long_call(shape: Shape) -> bool {
    let fixture = Fixture::new(1);
    let (addr, task) = fixture.start(shape).await;

    let entered = fixture.entered.notified();
    let long_call = tokio::spawn(call_tool(addr, 1, "parked"));
    entered.await;

    let answered = tokio::time::timeout(shape.wait(), initialize(addr))
        .await
        .ok();
    if let Some(init) = &answered {
        assert!(
            result_of(init)["serverInfo"]["name"].is_string(),
            "{}",
            init.raw
        );
    }

    fixture.release.notify_one();
    let parked = long_call.await.expect("long call joins");
    result_of(&parked);
    task.abort();
    answered.is_some()
}

#[tokio::test]
async fn concurrent_server_answers_initialize_during_a_long_call() {
    assert!(initialize_during_long_call(Shape::Concurrent).await);
}

#[tokio::test]
async fn legacy_server_blocks_initialize_during_a_long_call() {
    assert!(
        !initialize_during_long_call(Shape::Legacy).await,
        "the legacy path is expected to hold initialize behind the parked call"
    );
}

#[tokio::test]
async fn legacy_server_still_serializes_tool_calls() {
    assert!(
        rendezvous(Shape::Legacy, 2).await.is_none(),
        "the legacy Arc<Mutex<Server>> path is expected to serialize; if this \
         rendezvous completed, the concurrent property below proves nothing"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// For any party count, every call reaches the handler simultaneously, so
    /// all N rendezvous calls complete. A transport that admitted fewer than N
    /// handlers at once would deadlock the barrier and hit the deadline.
    #[test]
    fn concurrent_server_admits_n_simultaneous_calls(parties in 2usize..=8) {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let met = rt.block_on(rendezvous(Shape::Concurrent, parties));
        prop_assert_eq!(met.map(|r| r.len()), Some(parties));
    }
}
