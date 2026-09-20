//! The FIRST harness in `crates/pmcp-agent/tests/` that drives a REAL
//! [`pmcp::server::streamable_http_server::StreamableHttpServer`] over a
//! loopback socket.
//!
//! # Why this file exists at all
//!
//! The SDK already owns a live-HTTP harness at the repository root
//! (`tests/common/v2.rs`). It cannot be reused here: it belongs to a
//! **different crate** (`pmcp`'s own integration-test tree), and Rust has no way
//! to import one crate's `tests/` module into another crate's `tests/` module.
//! The root file is therefore a DESIGN REFERENCE — the ephemeral-port,
//! readiness-from-`start()`, and drop→abort→await teardown doctrine below is
//! carried from it deliberately — and this file is an independent
//! implementation, not a re-export.
//!
//! # How to consume it
//!
//! Files under `tests/common/` are NOT compiled as their own test binaries, so
//! this is the correct home for shared test machinery. Include it per test
//! binary with the in-crate convention that `tests/common/duplex.rs` already
//! establishes:
//!
//! ```ignore
//! #[path = "common/v2_server.rs"]
//! mod v2_server;
//! ```
//!
//! # Gating lives on the CONSUMER, not here
//!
//! This file carries no `#![cfg]` of its own. Every consuming test binary must
//! open with
//! `#![cfg(all(feature = "url-connector", not(target_arch = "wasm32")))]`,
//! because the era-probe path needs `url-connector` and
//! `StreamableHttpTransport` is `#[cfg(not(target_arch = "wasm32"))]`.
//!
//! # What it provides
//!
//! - [`spawn_v2`] — a live server whose accept-list carries BOTH `2025-11-25`
//!   and `2026-07-28`, i.e. the "one binary serves both eras" deployment. This
//!   is the DISCRIMINATING shape: a connector that silently keeps speaking v1
//!   still gets a working connection here, so only a wire-level assertion
//!   catches it.
//! - [`spawn_v1_only`] — a live server with the DEFAULT (v1-only) accept-list,
//!   so a v2 probe is rejected with the measured signature (the v2 gate returns
//!   `Passthrough`, then `validate_protocol_version_supported` answers
//!   HTTP 400 / JSON-RPC `-32600`).
//! - [`closed_loopback_endpoint`] — an address that is guaranteed unreachable
//!   without depending on the network.
//! - A SERVER-SIDE [`RequestLog`], because absence of a request (no
//!   `initialize` on the wire) cannot be proven from the client.
//! - A [`ScriptedTaskStore`] that GUARANTEES a task-associated tool result which
//!   needs at least one NON-terminal poll before it settles.

// Each consuming test binary uses a different subset of this harness; the
// unused remainder is another file's entry point, not dead code.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpRequest,
};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::task_store::{InMemoryTaskStore, StoreConfig, TaskStore, TaskStoreError};
use pmcp::server::{ToolHandler, ToolOutput};
use pmcp::shared::http_constants::MCP_PROTOCOL_VERSION;
use pmcp::testing::META_PROTOCOL_VERSION;
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::tasks::{Task, TaskMetadata, TaskStatus};
use pmcp::types::{CallToolResult, Content, ToolInfo};
use pmcp::{RequestHandlerExtra, Server};

// ===========================================================================
// The harness contract, as constants. Tests assert against THESE, never
// against a magic value re-spelled at the assertion site.
// ===========================================================================

/// Upper bound on any single await a test performs against this harness.
///
/// A hung server must FAIL the test, not hang it: every helper that could block
/// indefinitely is wrapped in `tokio::time::timeout(BOUNDED_WAIT, ..)`, and a
/// consuming test is expected to do the same for its own awaits.
pub const BOUNDED_WAIT: Duration = Duration::from_secs(20);

/// The status the harness's task tool GUARANTEES its task reaches.
///
/// Exposed so a test asserts against the harness's contract rather than
/// re-spelling `TaskStatus::Completed` at the assertion site.
pub const TERMINAL_TASK_STATUS: TaskStatus = TaskStatus::Completed;

/// The text content carried by the terminal `CallToolResult` the harness's
/// task settles on. A poller that reached terminal sees exactly this string.
pub const TERMINAL_RESULT_MARKER: &str = "pinned-task-reached-terminal";

/// How many NON-terminal `tasks/get` reads the harness's task serves before it
/// settles.
///
/// One, not zero: an immediately-terminal task would let a poller
/// short-circuit, and the "including task polling" clause would go unproven.
pub const NON_TERMINAL_POLLS_BEFORE_TERMINAL: usize = 1;

/// The plain (non-task) tool the pinned server registers.
pub const PLAIN_TOOL: &str = "pinned_plain";

/// The tool whose result is GUARANTEED to carry a related-task envelope.
pub const TASK_TOOL: &str = "pinned_task";

/// The text the plain tool returns.
pub const PLAIN_RESULT_MARKER: &str = "pinned-plain-ok";

/// The poll interval (milliseconds) the task tool advertises in its
/// related-task envelope, so the polling loop runs fast and deterministically
/// instead of at the 1000 ms protocol default.
pub const TASK_POLL_INTERVAL_MS: u64 = 50;

/// The number of tools [`pinned_server`] registers. Pinned so `tools/list` is
/// byte-stable across runs.
pub const PINNED_TOOL_COUNT: usize = 2;

/// The single owner bucket [`ScriptedTaskStore`] normalizes every task onto.
///
/// The SDK binds an unauthenticated task owner differently per era (v1 uses a
/// shared `"local"` bucket, v2 uses the anonymous principal), and both
/// constants are crate-private. Normalizing here makes the fixture era-neutral
/// so one task is reachable from whichever era the connector negotiated —
/// which is the whole point of a dual-era harness.
pub const PINNED_OWNER: &str = "pinned-harness-owner";

/// Which protocol versions a spawned server accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accept {
    /// `2025-11-25` AND `2026-07-28` — the dual-era deployment.
    V1AndV2,
    /// The builder default: `2025-11-25` only. A v2 probe is REJECTED.
    V1Only,
}

// ===========================================================================
// Server-side request log. Absence cannot be proven from the client.
// ===========================================================================

/// One observed HTTP request, reduced to the two facts the tests assert on.
#[derive(Debug, Clone)]
pub struct Observed {
    /// The JSON-RPC `method` read out of the request body.
    pub method: String,
    /// The protocol version the request DECLARED, read from `params._meta`
    /// (the v2 per-request era signal) and falling back to the
    /// `MCP-Protocol-Version` header. `None` when the request declared neither.
    pub protocol_version: Option<String>,
}

/// A thread-safe record of every JSON-RPC request the live server received.
#[derive(Debug, Default)]
pub struct RequestLog {
    entries: StdMutex<Vec<Observed>>,
}

impl RequestLog {
    /// Create an empty shared log.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Record one observation.
    pub fn record(&self, observed: Observed) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(observed);
        }
    }

    /// Every observation, in arrival order.
    #[must_use]
    pub fn entries(&self) -> Vec<Observed> {
        self.entries
            .lock()
            .map(|entries| entries.clone())
            .unwrap_or_default()
    }

    /// How many requests carried `method`.
    #[must_use]
    pub fn count(&self, method: &str) -> usize {
        self.entries()
            .iter()
            .filter(|entry| entry.method == method)
            .count()
    }

    /// A single-line rendering of the whole log, for assertion messages.
    #[must_use]
    pub fn render(&self) -> String {
        self.entries()
            .into_iter()
            .map(|entry| {
                format!(
                    "{}@{}",
                    entry.method,
                    entry
                        .protocol_version
                        .unwrap_or_else(|| "<none>".to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// HTTP middleware that records every request into a [`RequestLog`].
///
/// This runs at the HTTP layer, BEFORE JSON-RPC dispatch, so it observes
/// requests the protocol layer would later reject — which is exactly what the
/// v1-fallback case needs.
struct RecordingMiddleware {
    log: Arc<RequestLog>,
}

#[async_trait]
impl ServerHttpMiddleware for RecordingMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let method = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("<unparsed>")
            .to_string();
        let from_meta = body
            .get("params")
            .and_then(|params| params.get("_meta"))
            .and_then(|meta| meta.get(META_PROTOCOL_VERSION))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let from_header = request
            .headers
            .get(MCP_PROTOCOL_VERSION)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        self.log.record(Observed {
            method,
            protocol_version: from_meta.or(from_header),
        });
        Ok(())
    }

    fn priority(&self) -> i32 {
        // Lowest number runs first: observe the request before anything else
        // can transform it.
        1
    }
}

// ===========================================================================
// The scripted task store: a GUARANTEED non-terminal poll, then terminal.
// ===========================================================================

/// An [`InMemoryTaskStore`] wrapper that makes the harness's task deterministic.
///
/// Two behaviours are added, and nothing else:
///
/// 1. **Owner normalization.** Every call is re-scoped onto [`PINNED_OWNER`], so
///    the same task is reachable from a v1 connection and a v2 connection even
///    though the SDK binds unauthenticated owners differently per era.
/// 2. **A scripted settle.** The first [`NON_TERMINAL_POLLS_BEFORE_TERMINAL`]
///    reads OF A GIVEN TASK answer `working`; the next read settles that task on
///    [`TERMINAL_TASK_STATUS`] with a persisted terminal result carrying
///    [`TERMINAL_RESULT_MARKER`]. The trigger is a COUNTER, not a clock, so the
///    fixture has no timing dependency.
///
/// The read counter is PER TASK, not global. A test that calls the task tool
/// more than once (e.g. once directly to inspect the envelope, once through the
/// invoker) mints more than one task, and a global counter would let the second
/// task settle on its FIRST read — silently removing the non-terminal poll the
/// whole fixture exists to guarantee.
#[derive(Debug)]
pub struct ScriptedTaskStore {
    inner: InMemoryTaskStore,
    reads_by_task: StdMutex<HashMap<String, usize>>,
}

impl ScriptedTaskStore {
    /// Create an empty scripted store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: InMemoryTaskStore::new(),
            reads_by_task: StdMutex::new(HashMap::new()),
        }
    }

    /// Record one read of `task_id` and return that task's new read count.
    ///
    /// A poisoned lock is recovered rather than panicked on: a fixture must
    /// fail the assertion under test, never abort the test process.
    fn record_read(&self, task_id: &str) -> usize {
        let mut map = self
            .reads_by_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = map.entry(task_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// The terminal result the scripted settle persists.
    #[must_use]
    pub fn terminal_result() -> CallToolResult {
        CallToolResult::new(vec![Content::text(TERMINAL_RESULT_MARKER)])
    }

    /// Settle `task_id` onto [`TERMINAL_TASK_STATUS`], persisting the terminal
    /// result FIRST so a terminal read never observes a completed task with no
    /// result. Idempotent: an already-terminal task is left alone.
    async fn settle(&self, task_id: &str) -> Result<(), TaskStoreError> {
        let task = self.inner.get(task_id, PINNED_OWNER).await?;
        if task.status.is_terminal() {
            return Ok(());
        }
        self.inner
            .set_result(task_id, PINNED_OWNER, Self::terminal_result())
            .await?;
        self.inner
            .update_status(
                task_id,
                PINNED_OWNER,
                TERMINAL_TASK_STATUS,
                Some(TERMINAL_RESULT_MARKER.to_string()),
            )
            .await?;
        Ok(())
    }
}

impl Default for ScriptedTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskStore for ScriptedTaskStore {
    async fn create(&self, _owner_id: &str, ttl: Option<u64>) -> Result<Task, TaskStoreError> {
        self.inner.create(PINNED_OWNER, ttl).await
    }

    async fn get(&self, task_id: &str, _owner_id: &str) -> Result<Task, TaskStoreError> {
        if self.record_read(task_id) > NON_TERMINAL_POLLS_BEFORE_TERMINAL {
            self.settle(task_id).await?;
        }
        self.inner.get(task_id, PINNED_OWNER).await
    }

    async fn update_status(
        &self,
        task_id: &str,
        _owner_id: &str,
        status: TaskStatus,
        message: Option<String>,
    ) -> Result<Task, TaskStoreError> {
        self.inner
            .update_status(task_id, PINNED_OWNER, status, message)
            .await
    }

    async fn list(
        &self,
        _owner_id: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<Task>, Option<String>), TaskStoreError> {
        self.inner.list(PINNED_OWNER, cursor).await
    }

    async fn cancel(&self, task_id: &str, _owner_id: &str) -> Result<Task, TaskStoreError> {
        self.inner.cancel(task_id, PINNED_OWNER).await
    }

    async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> {
        self.inner.cleanup_expired().await
    }

    fn config(&self) -> &StoreConfig {
        self.inner.config()
    }

    async fn set_result(
        &self,
        task_id: &str,
        _owner_id: &str,
        result: CallToolResult,
    ) -> Result<(), TaskStoreError> {
        self.inner.set_result(task_id, PINNED_OWNER, result).await
    }

    async fn get_result(
        &self,
        task_id: &str,
        _owner_id: &str,
    ) -> Result<CallToolResult, TaskStoreError> {
        self.inner.get_result(task_id, PINNED_OWNER).await
    }

    fn supports_results(&self) -> bool {
        true
    }
}

// ===========================================================================
// The two pinned tools. Both handlers are TOTAL — no panic path, no unwrap.
// ===========================================================================

/// A tool returning an immediate, non-task [`CallToolResult`].
struct PlainTool;

#[async_trait]
impl ToolHandler for PlainTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Never reached: `handle_output` below owns the envelope. Kept total so
        // a future dispatcher change degrades to a correct answer, not a panic.
        Ok(json!({ "marker": PLAIN_RESULT_MARKER }))
    }

    async fn handle_output(
        &self,
        _args: Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        Ok(ToolOutput::Result(CallToolResult::new(vec![
            Content::text(PLAIN_RESULT_MARKER),
        ])))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            PLAIN_TOOL,
            Some("Immediate, non-task result.".to_string()),
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ))
    }
}

/// A tool whose result is GUARANTEED to carry a related-task envelope.
///
/// It mints a REAL task in the [`ScriptedTaskStore`] and returns a
/// [`ToolOutput::Result`], which reaches the wire VERBATIM — so the `_meta`
/// related-task envelope survives instead of being stringified by the payload
/// path's text-wrap tail.
struct TaskTool {
    store: Arc<ScriptedTaskStore>,
}

impl TaskTool {
    /// Mint a task and build the envelope that references it.
    async fn start(&self) -> pmcp::Result<CallToolResult> {
        let task =
            self.store.create(PINNED_OWNER, None).await.map_err(|e| {
                pmcp::Error::internal(format!("pinned task store create failed: {e}"))
            })?;
        Ok(
            CallToolResult::new(vec![Content::text("pinned task started")]).with_related_task(
                TaskMetadata::new(task.task_id).with_poll_interval(TASK_POLL_INTERVAL_MS),
            ),
        )
    }
}

#[async_trait]
impl ToolHandler for TaskTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Never reached (see `PlainTool::handle`). Deliberately NOT a
        // CallToolResult-shaped value: the payload path would stringify it.
        Ok(json!({ "marker": "pinned task started" }))
    }

    async fn handle_output(
        &self,
        _args: Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        self.start().await.map(ToolOutput::Result)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            TASK_TOOL,
            Some("Result carries a related-task envelope.".to_string()),
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ))
    }
}

// ===========================================================================
// Building and spawning.
// ===========================================================================

/// A built-but-not-yet-spawned pinned server, plus the handles a test needs to
/// observe it from the SERVER side.
pub struct PinnedServer {
    /// The server itself, ready to hand to [`spawn`].
    pub server: Server,
    /// Every request the server will observe.
    pub requests: Arc<RequestLog>,
    /// The scripted task backend.
    pub tasks: Arc<ScriptedTaskStore>,
}

/// Build the deterministic pinned server.
///
/// It registers exactly [`PINNED_TOOL_COUNT`] tools, both with TOTAL handlers,
/// so `tools/list` is byte-stable and every `tools/call` terminates:
/// [`PLAIN_TOOL`] returns an immediate result, and [`TASK_TOOL`] returns a
/// result that GUARANTEES a related-task envelope backed by a real store-minted
/// task.
///
/// # Panics
///
/// Panics if the server fails to build, which in this fixed configuration
/// indicates the harness itself is broken rather than a test failure.
#[must_use]
pub fn pinned_server(accept: Accept) -> PinnedServer {
    let requests = RequestLog::new();
    let tasks = Arc::new(ScriptedTaskStore::new());

    let mut builder = Server::builder()
        .name("pmcp-agent-pinned-harness")
        .version("1.0.0")
        .tool(PLAIN_TOOL, PlainTool)
        .tool(
            TASK_TOOL,
            TaskTool {
                store: Arc::clone(&tasks),
            },
        )
        .task_store(Arc::clone(&tasks) as Arc<dyn TaskStore>);

    // The accept-list is what opts a server into 2026-07-28. Leaving it unset
    // is the v1-only default, which is exactly what `spawn_v1_only` wants.
    builder = match accept {
        Accept::V1AndV2 => builder.with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ]),
        Accept::V1Only => builder,
    };

    let server = builder.build().expect("pinned harness server builds");
    PinnedServer {
        server,
        requests,
        tasks,
    }
}

/// A running pinned server on a loopback socket.
pub struct LiveServer {
    /// The OS-assigned loopback address the server is already accepting on.
    pub addr: SocketAddr,
    /// The server task. Pass it to [`teardown`].
    pub handle: JoinHandle<()>,
    /// Every request the server has observed.
    pub requests: Arc<RequestLog>,
    /// The scripted task backend.
    pub tasks: Arc<ScriptedTaskStore>,
}

impl LiveServer {
    /// The `http://{addr}/` string the connector factory takes.
    #[must_use]
    pub fn endpoint(&self) -> String {
        endpoint(self.addr)
    }
}

/// Spawn `pinned` on an ephemeral loopback port with the STATEFUL default HTTP
/// config plus the recording middleware.
///
/// The default config is deliberate: it keeps a live `session_id_generator`, so
/// a v2 request being session-free is the PER-REQUEST era gate doing its job
/// rather than a build-time stateless switch that removed sessions before any
/// request was seen.
///
/// Async because `StreamableHttpServer::start` binds the socket BEFORE
/// returning — that is the caller's readiness guarantee, so no sleep is needed.
///
/// # Panics
///
/// Panics if the socket cannot be bound within [`BOUNDED_WAIT`].
pub async fn spawn(pinned: PinnedServer) -> LiveServer {
    let PinnedServer {
        server,
        requests,
        tasks,
    } = pinned;

    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(RecordingMiddleware {
        log: Arc::clone(&requests),
    }));

    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };

    let bind = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(bind, Arc::new(Mutex::new(server)), config);
    let (addr, handle) = tokio::time::timeout(BOUNDED_WAIT, http.start())
        .await
        .expect("pinned harness server binds within BOUNDED_WAIT")
        .expect("pinned harness server starts");

    LiveServer {
        addr,
        handle,
        requests,
        tasks,
    }
}

/// Spawn a live server opted into the 2026-07-28 accept-list ALONGSIDE
/// 2025-11-25, so a v2 `server/discover` succeeds and a v1 `initialize` would
/// ALSO succeed.
///
/// Keeping v1 acceptable is what makes this the discriminating fixture: a
/// connector that never attempts v2 still gets a working connection, so only a
/// SERVER-side wire assertion distinguishes "spoke v2" from "silently kept
/// speaking v1".
pub async fn spawn_v2() -> LiveServer {
    spawn(pinned_server(Accept::V1AndV2)).await
}

/// Spawn a live server with the DEFAULT (v1-only) accept-list.
///
/// A v2 `server/discover` against this server is rejected with the measured
/// signature: the v2 gate returns `Passthrough`, then
/// `validate_protocol_version_supported` answers HTTP 400 with JSON-RPC
/// `-32600`. The server ANSWERED, which is what makes this an era rejection
/// rather than an infrastructure failure.
pub async fn spawn_v1_only() -> LiveServer {
    spawn(pinned_server(Accept::V1Only)).await
}

/// Shut a spawned server down in the order: drop sockets → `abort()` → `await`.
///
/// The ORDER is the point. A bare `abort()` with no await produces intermittent
/// nextest `LEAK` noise: the aborted task has not necessarily finished when the
/// test function returns, and nextest's leak timeout then fires as a false
/// failure. A still-open client socket also keeps the server's connection task
/// alive across the abort, so the sockets go FIRST.
///
/// `sockets` is anything the test owns that must die before the server — a
/// connector, a `Vec` of them, or `()` when the test owns no socket of its own.
pub async fn teardown<S: Send>(handle: JoinHandle<()>, sockets: S) {
    drop(sockets);
    handle.abort();
    let _ = handle.await;
}

/// The `http://{addr}/` endpoint string the connector factory takes.
#[must_use]
pub fn endpoint(addr: SocketAddr) -> String {
    format!("http://{addr}/")
}

/// An endpoint that is GUARANTEED unreachable, without touching the network.
///
/// Binds an ephemeral loopback port, captures the OS-assigned address, then
/// DROPS the listener. Nothing is listening on that address afterwards, so a
/// connect attempt is refused immediately and deterministically — which is what
/// the unreachable-host case needs.
///
/// # Panics
///
/// Panics if an ephemeral loopback port cannot be bound.
#[must_use]
pub fn closed_loopback_endpoint() -> String {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("bind an ephemeral loopback port");
    let addr = listener.local_addr().expect("read back the bound address");
    drop(listener);
    endpoint(addr)
}
