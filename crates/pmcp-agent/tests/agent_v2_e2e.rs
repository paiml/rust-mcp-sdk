#![cfg(all(feature = "url-connector", not(target_arch = "wasm32")))]
//! CLNT-03: `pmcp-agent` end-to-end against a LIVE MCP server — both eras.
//!
//! # This file is the contract, and it is written RED
//!
//! These four cases were written BEFORE plan 117-07 changes
//! `UrlConnectorClientFactory::client_for`, so the implementation is driven by a
//! contract that already exists rather than the contract being back-filled to
//! whatever the implementation happened to do. Three of them fail today, each
//! for a reason recorded in `117-04-SUMMARY.md`, and every failure message names
//! plan 117-07 as the implementer.
//!
//! # The two negative cases carry EQUAL weight to the happy path
//!
//! D-07 is explicit that fallback paths are where dual-version bugs hide, so
//! `agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2` and
//! `an_unreachable_host_propagates_and_is_not_reported_as_era_v1` are
//! first-class tests, not footnotes on the happy path.
//!
//! # The fallback rule under test classifies by REACHABILITY
//!
//! ```text
//! The server ANSWERED (any HTTP response, any JSON-RPC error) => era rejection => FALL BACK to v1.
//! The server did NOT answer (DNS / TCP / TLS / timeout)       => infrastructure => PROPAGATE.
//! ```
//!
//! It is NEVER decided by string-matching an error message. § Q4.3 measured four
//! distinct rejection signatures across implementations, and the pmcp phrasing
//! `"Unsupported protocol version"` is neither stable nor the only one. No test
//! here inspects the TEXT of an error to classify it; the happy/fallback split is
//! observed on the SERVER side, from the requests the server actually received.
//!
//! # Every era claim is made from OBSERVED WIRE BEHAVIOUR
//!
//! A client-side log line cannot prove that no `initialize` was sent. The pinned
//! harness therefore records every request the server received, with the
//! protocol version that request declared, and the assertions read THAT.
//!
//! # Two constraints this file deliberately writes down
//!
//! **D-09 — the tasks coupling is bounded to one seam.** `pmcp-agent`'s entire
//! `tasks/*` surface is `ConnectorClient::wait_for_related_task` plus its one
//! caller and one impl. Nothing here adds a `ConnectorClient` method naming a
//! `tasks/*` wire method; the task interaction below goes through
//! `wait_for_related_task` and `CallToolResult::related_task()`. Do not widen it.
//!
//! **A-D08 — no era probe inside `pmcp::Client`.** `src/client/mod.rs:871-878`
//! carries the literal instruction `do not "restore" the latter`, and
//! `server_discover()` opens with `require_v2(..)` (`:892`), which fails LOCALLY
//! on a v1 client. The probe belongs in `pmcp-agent`'s connector factory, which
//! plan 117-07 owns — not in the SDK client.

#[path = "common/v2_server.rs"]
mod v2_server;

use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::time::timeout;

use pmcp::server::task_store::TaskStore;
use pmcp::types::protocol::{protocol_era, Era};
use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, Content, ToolInfo};
use pmcp::WaitForTaskOptions;

use pmcp_agent::invoker::{
    ClientToolInvoker, ConnectorClient, ConnectorClientFactory, InvokerError,
    UrlConnectorClientFactory,
};
use pmcp_agent::seams::{ToolCall, ToolInvoker};

use v2_server::{
    closed_loopback_endpoint, spawn_v1_only, spawn_v2, teardown, LiveServer, RequestLog,
    BOUNDED_WAIT, NON_TERMINAL_POLLS_BEFORE_TERMINAL, PINNED_OWNER, PINNED_TOOL_COUNT,
    PLAIN_RESULT_MARKER, PLAIN_TOOL, TASK_TOOL, TERMINAL_RESULT_MARKER, TERMINAL_TASK_STATUS,
};

/// The plan that owns the era negotiation these tests describe.
///
/// Named in every failure message so a RED run says WHO must act, not just WHAT
/// broke.
const IMPLEMENTER: &str = "plan 117-07 (UrlConnectorClientFactory::client_for)";

/// The hard task-poll cap the invoker under test is constructed with.
///
/// `ClientToolInvoker` promises this reaches `wait_for_related_task` as
/// `WaitForTaskOptions::max_poll_duration_secs`, which is what bounds polling.
const POLL_CAP_SECS: u64 = 15;

/// The JSON-RPC method a v2 era probe sends.
const V2_PROBE_METHOD: &str = "server/discover";

/// The JSON-RPC method that exists ONLY on v1.
const V1_HANDSHAKE_METHOD: &str = "initialize";

/// The task-poll method, counted on the SERVER side.
const TASK_POLL_METHOD: &str = "tasks/get";

// ===========================================================================
// Helpers. None of them inspects the TEXT of an error.
// ===========================================================================

/// The methods of every request whose DECLARED era was not v2.
///
/// A request that declared no version at all classifies as v1 — the same
/// conservative default the server applies — so an `initialize` (which carries
/// no `_meta` era signal) shows up here, which is exactly the point.
fn non_v2_request_methods(log: &RequestLog) -> Vec<String> {
    log.entries()
        .into_iter()
        .filter(|entry| {
            entry
                .protocol_version
                .as_deref()
                .map_or(Era::V1, protocol_era)
                != Era::V2
        })
        .map(|entry| entry.method)
        .collect()
}

/// The methods of every request whose DECLARED era was v2.
fn v2_request_methods(log: &RequestLog) -> Vec<String> {
    log.entries()
        .into_iter()
        .filter(|entry| {
            entry
                .protocol_version
                .as_deref()
                .map_or(Era::V1, protocol_era)
                == Era::V2
        })
        .map(|entry| entry.method)
        .collect()
}

/// The exact `ToolCallResult::content` an agent sees once the harness's task has
/// settled, built through the SAME serializer the invoker uses so the two cannot
/// drift.
fn terminal_payload() -> Value {
    serde_json::to_value(vec![Content::text(TERMINAL_RESULT_MARKER)]).unwrap_or(Value::Null)
}

/// The exact `ToolCallResult::content` the plain tool produces.
fn plain_payload() -> Value {
    serde_json::to_value(vec![Content::text(PLAIN_RESULT_MARKER)]).unwrap_or(Value::Null)
}

/// The tool names a connector advertised, sorted so the assertion does not
/// depend on the server's registry iteration order.
fn sorted_names(tools: &[ToolInfo]) -> Vec<String> {
    let mut names: Vec<String> = tools.iter().map(|tool| tool.name.clone()).collect();
    names.sort();
    names
}

/// The pinned tool names, sorted — the expectation `sorted_names` is compared to.
fn expected_tool_names() -> Vec<String> {
    let mut names = vec![PLAIN_TOOL.to_string(), TASK_TOOL.to_string()];
    names.sort();
    names
}

/// Establish a connector for `live`, bounded by [`BOUNDED_WAIT`].
///
/// A hung factory FAILS the test rather than hanging it.
async fn connect(live: &LiveServer) -> Result<Arc<dyn ConnectorClient>, InvokerError> {
    let factory = UrlConnectorClientFactory::new();
    timeout(BOUNDED_WAIT, factory.client_for(&live.endpoint()))
        .await
        .expect("client_for completed within BOUNDED_WAIT")
}

// ===========================================================================
// A `ConnectorClient` decorator that records what the invoker asked for.
//
// D-09: this adds NO new trait method and names NO `tasks/*` wire method. It
// implements the EXISTING seam and forwards every call, capturing only the
// arguments the invoker chose.
// ===========================================================================

/// Wraps a connector and records every `wait_for_related_task` invocation.
struct WaitRecordingConnector {
    inner: Arc<dyn ConnectorClient>,
    waits: StdMutex<Vec<(TaskMetadata, WaitForTaskOptions)>>,
}

impl WaitRecordingConnector {
    fn new(inner: Arc<dyn ConnectorClient>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            waits: StdMutex::new(Vec::new()),
        })
    }

    /// The FIRST recorded `wait_for_related_task` call, or a deliberately empty
    /// stand-in.
    ///
    /// The stand-in (empty task id, unbounded options) is what makes the
    /// assertions below TOTAL: if the invoker never drove the seam at all, the
    /// task-id and bounded-options assertions fail with their own messages
    /// instead of the test panicking on an `unwrap` and saying nothing useful.
    fn first_wait(&self) -> (TaskMetadata, WaitForTaskOptions) {
        self.waits
            .lock()
            .ok()
            .and_then(|waits| waits.first().cloned())
            .unwrap_or_else(|| (TaskMetadata::new(""), WaitForTaskOptions::default()))
    }
}

#[async_trait]
impl ConnectorClient for WaitRecordingConnector {
    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult, InvokerError> {
        self.inner.call_tool(name, arguments).await
    }

    async fn wait_for_related_task(
        &self,
        meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult, InvokerError> {
        if let Ok(mut waits) = self.waits.lock() {
            waits.push((meta.clone(), opts.clone()));
        }
        self.inner.wait_for_related_task(meta, opts).await
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>, InvokerError> {
        self.inner.list_tools().await
    }
}

// ===========================================================================
// Case 1 — the v2 happy path.
// ===========================================================================

/// Against a server that accepts BOTH eras, the agent must negotiate v2 and send
/// NO `initialize`.
///
/// The server accepts v1 too, which is what makes this discriminating: a
/// connector that silently keeps speaking v1 still gets a working connection, so
/// only the SERVER-side assertions tell the two apart.
#[tokio::test]
async fn agent_reaches_a_v2_server_end_to_end() {
    let live = spawn_v2().await;

    let connector = connect(&live).await.unwrap_or_else(|err| {
        panic!("{IMPLEMENTER} must establish a connector against a v2-accepting server; got {err}")
    });

    let tools = timeout(BOUNDED_WAIT, connector.list_tools())
        .await
        .expect("list_tools completed within BOUNDED_WAIT")
        .expect("list_tools succeeds against the pinned server");
    assert_eq!(
        tools.len(),
        PINNED_TOOL_COUNT,
        "the pinned server advertises exactly {PINNED_TOOL_COUNT} tools; saw {:?}",
        sorted_names(&tools)
    );
    assert_eq!(
        sorted_names(&tools),
        expected_tool_names(),
        "tools/list must return BOTH pinned tools"
    );

    let result = timeout(BOUNDED_WAIT, connector.call_tool(PLAIN_TOOL, json!({})))
        .await
        .expect("call_tool completed within BOUNDED_WAIT")
        .expect("call_tool on the plain tool succeeds");
    assert_eq!(
        serde_json::to_value(&result.content).unwrap_or(Value::Null),
        plain_payload(),
        "the plain tool's result must reach the agent verbatim"
    );

    // ---- The era claim, made from the SERVER side. ----
    //
    // A client-side assertion cannot prove ABSENCE. These two read the requests
    // the server actually received.
    assert_eq!(
        live.requests.count(V1_HANDSHAKE_METHOD),
        0,
        "{IMPLEMENTER} must not send `{V1_HANDSHAKE_METHOD}` to a server that accepts 2026-07-28 \
         — v2 has no handshake. Server observed: [{}]",
        live.requests.render()
    );
    assert_eq!(
        non_v2_request_methods(&live.requests),
        Vec::<String>::new(),
        "{IMPLEMENTER} must declare the 2026-07-28 era on EVERY request to a v2-accepting server. \
         Server observed: [{}]",
        live.requests.render()
    );

    teardown(live.handle, connector).await;
}

// ===========================================================================
// Case 2 — the CLNT-03 "including task polling" clause, UNCONDITIONALLY.
// ===========================================================================

/// The agent's `ToolInvoker` must drive a task-associated tool result to terminal
/// over v2.
///
/// All four facts are asserted separately and with NO conditional guarding any of
/// them: a "if a task came back" guard would let this phase ship with CLNT-03's
/// task-polling clause unproven. The harness GUARANTEES the task and guarantees
/// that it needs at least one NON-terminal poll, so there is nothing to guard.
#[tokio::test]
async fn agent_drives_task_polling_to_terminal_on_v2() {
    let live = spawn_v2().await;

    let connector = connect(&live).await.unwrap_or_else(|err| {
        panic!("{IMPLEMENTER} must establish a connector against a v2-accepting server; got {err}")
    });
    let recorder = WaitRecordingConnector::new(Arc::clone(&connector));
    let invoker = ClientToolInvoker::new(
        Arc::clone(&recorder) as Arc<dyn ConnectorClient>,
        POLL_CAP_SECS,
    );

    let outcome = timeout(
        BOUNDED_WAIT,
        invoker.invoke(ToolCall {
            id: "call-1".to_string(),
            name: TASK_TOOL.to_string(),
            arguments: json!({}),
            connector: None,
        }),
    )
    .await
    .expect("the invoker completed within BOUNDED_WAIT");

    let (meta, opts) = recorder.first_wait();
    let task_polls = live.requests.count(TASK_POLL_METHOD);
    let min_polls = NON_TERMINAL_POLLS_BEFORE_TERMINAL + 1;
    let settled = live
        .tasks
        .get(&meta.task_id, PINNED_OWNER)
        .await
        .unwrap_or_else(|err| panic!("the harness task {:?} must exist: {err}", meta.task_id));

    // FACT 1 — a task id was DISCOVERED from the tool result.
    assert!(
        !meta.task_id.is_empty(),
        "the `{TASK_TOOL}` result must carry a related-task envelope the invoker discovers via \
         CallToolResult::related_task(); the invoker never reached wait_for_related_task. \
         Server observed: [{}]",
        live.requests.render()
    );
    // FACT 2 — the seam was driven with BOUNDED options.
    assert_eq!(
        opts.max_poll_duration_secs,
        Some(POLL_CAP_SECS),
        "ClientToolInvoker must hand wait_for_related_task a hard max_poll_duration_secs cap"
    );
    // FACT 3 — the SERVER observed the polling. A client-side "I called it" proves
    // the call, not the poll.
    assert!(
        task_polls >= min_polls,
        "the server must observe at least {min_polls} `{TASK_POLL_METHOD}` requests (the harness \
         serves {NON_TERMINAL_POLLS_BEFORE_TERMINAL} non-terminal read before it settles, so a \
         short-circuit on an immediately-terminal task is impossible); saw {task_polls}. \
         Server observed: [{}]",
        live.requests.render()
    );
    // FACT 4 — the task reached the harness's terminal contract, and that terminal
    // result is what the agent received.
    assert_eq!(
        (settled.status, outcome.content.clone()),
        (TERMINAL_TASK_STATUS, terminal_payload()),
        "the polled task must reach the harness's pub terminal-state constant and the agent must \
         receive its terminal result within the {POLL_CAP_SECS}s cap; invoker error = {:?}",
        outcome.error
    );

    // ---- And all of that must have happened on v2. ----
    assert_eq!(
        live.requests.count(V1_HANDSHAKE_METHOD),
        0,
        "{IMPLEMENTER} must poll tasks over 2026-07-28, with no `{V1_HANDSHAKE_METHOD}`. \
         Server observed: [{}]",
        live.requests.render()
    );

    teardown(live.handle, (connector, recorder)).await;
}

// ===========================================================================
// Case 3 — the server ANSWERED and rejected v2, so fall back.
// ===========================================================================

/// Against a v1-only server, the agent must ATTEMPT v2, accept the rejection, and
/// fall back to a working v1 connection.
///
/// The v2 attempt is asserted on the wire: a connector that never tried v2 would
/// otherwise pass this test by accident, since a v1-only server answers a plain
/// v1 handshake perfectly well.
#[tokio::test]
async fn agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2() {
    let live = spawn_v1_only().await;

    let connector = connect(&live).await.unwrap_or_else(|err| {
        panic!(
            "{IMPLEMENTER} must fall back to v1 when the server ANSWERS and rejects the v2 era; \
             got {err}. Server observed: [{}]",
            live.requests.render()
        )
    });

    let tools = timeout(BOUNDED_WAIT, connector.list_tools())
        .await
        .expect("list_tools completed within BOUNDED_WAIT")
        .expect("list_tools succeeds over the v1 fallback");
    assert_eq!(
        sorted_names(&tools),
        expected_tool_names(),
        "the v1 fallback connector must still advertise both pinned tools"
    );

    let result = timeout(BOUNDED_WAIT, connector.call_tool(PLAIN_TOOL, json!({})))
        .await
        .expect("call_tool completed within BOUNDED_WAIT")
        .expect("call_tool succeeds over the v1 fallback");
    assert_eq!(
        serde_json::to_value(&result.content).unwrap_or(Value::Null),
        plain_payload(),
        "the v1 fallback must return the plain tool's result verbatim"
    );

    // ---- The negotiated era is recorded as v1, on the wire. ----
    assert!(
        live.requests.count(V1_HANDSHAKE_METHOD) >= 1,
        "the v1 fallback must complete the `{V1_HANDSHAKE_METHOD}` handshake. \
         Server observed: [{}]",
        live.requests.render()
    );
    let v2_after_the_probe: Vec<String> = v2_request_methods(&live.requests)
        .into_iter()
        .filter(|method| method != V2_PROBE_METHOD)
        .collect();
    assert_eq!(
        v2_after_the_probe,
        Vec::<String>::new(),
        "after the era rejection every subsequent request must declare v1; only the probe itself \
         may declare 2026-07-28. Server observed: [{}]",
        live.requests.render()
    );

    // ---- And the fallback must have been REACHED by attempting v2 first. ----
    assert!(
        live.requests.count(V2_PROBE_METHOD) >= 1,
        "{IMPLEMENTER} must ATTEMPT the v2 era before falling back — the server must observe a \
         `{V2_PROBE_METHOD}` request that it then rejects. Without it the connector never tried \
         v2 and the fallback is untested. Server observed: [{}]",
        live.requests.render()
    );

    teardown(live.handle, connector).await;
}

// ===========================================================================
// Case 4 — an unreachable host is infrastructure, not a protocol signal.
// ===========================================================================

/// Pitfall 7 / T-117-10: an agent that silently downgrades on a network failure
/// reports "connected via v1" against a server that is provably v2 — or against
/// no server at all.
///
/// The endpoint here is an address nothing is listening on, so the server never
/// ANSWERS. That must propagate as an error, and must NOT be laundered into an
/// era decision. This case is GREEN before plan 117-07 and must STAY green: it
/// pins the behaviour that 117-07's new fallback branch is most likely to break.
#[tokio::test]
async fn an_unreachable_host_propagates_and_is_not_reported_as_era_v1() {
    let endpoint = closed_loopback_endpoint();
    let factory = UrlConnectorClientFactory::new();

    let outcome = timeout(BOUNDED_WAIT, factory.client_for(&endpoint))
        .await
        .expect("client_for against an unreachable host must FAIL fast, not hang");

    assert!(
        outcome.is_err(),
        "{IMPLEMENTER} must PROPAGATE an unreachable endpoint ({endpoint}); a fallback branch \
         that treats 'the connection failed' as 'the server rejected v2' would silently report \
         era v1 for a host that never answered"
    );
    assert!(
        outcome.as_ref().err().is_some() && outcome.as_ref().ok().is_none(),
        "no connector may be produced for an unreachable endpoint ({endpoint})"
    );

    let err = outcome.err().unwrap_or(InvokerError::Config(
        "no error was produced at all".to_string(),
    ));
    assert!(
        matches!(err, InvokerError::Transport(_)),
        "an unreachable endpoint is a TRANSPORT failure, classified by REACHABILITY and never by \
         the text of the message; got {err:?}"
    );
}
