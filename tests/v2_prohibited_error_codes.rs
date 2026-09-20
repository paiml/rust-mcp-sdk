//! Finding 11 — the spec-prohibited error codes, traced by EXECUTION.
//!
//! # The rule
//!
//! `docs/specification/draft/basic/index.mdx` § Error Codes — a section that is
//! **ABSENT at the `2026-07-28-RC` tag and was added after it** — states:
//!
//! > Implementations of this protocol version **MUST NOT** emit these codes:
//! > `-32002` … `-32042`.
//!
//! and, separately, that new implementations **SHOULD NOT** use the
//! implementation-defined range `-32000`..`-32019` at all.
//!
//! Recorded as Finding 11 of
//! `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK-ADDENDUM-2026-07-26.md`
//! with the status `untraced, actionable`.
//!
//! This is an **independent, semantics-agnostic** prohibition. It does NOT
//! contradict the phase's earlier `-32002`→`-32602` conclusion, which concerned
//! *resource-not-found semantics* and remains sound: that conclusion was about
//! what `-32002` MEANS, this file is about whether the NUMBER may appear on a v2
//! wire at all.
//!
//! # Why this file exists rather than a code review
//!
//! pmcp has two `-32002` emission sites and both are *commented* as v1-scoped:
//!
//! * `src/server/core.rs` — the server-not-initialized gate in
//!   `ServerCore::handle_request_internal`;
//! * `src/server/task_dispatch.rs` — the `tasks/result` "not completed yet"
//!   refusal on the no-router branch.
//!
//! Both comments are assertions nobody had tested. Phase 113 has repeatedly found
//! that "it looks v1-scoped" is exactly the reasoning that fails, so every
//! reachability claim in this file is settled by driving a real request at the
//! branch and reading the emitted `error.code` off the response. Nothing here is
//! decided by reading source and reasoning about it. The source scanner at the
//! bottom is a *change detector over emission sites*, deliberately kept separate
//! from — and never a substitute for — the executed probes.
//!
//! # Layout
//!
//! 1. The prohibition, declared as DATA (`MUST_NOT_EMIT`, `SHOULD_NOT_USE_RANGE`).
//! 2. Site A — `ServerCore`'s not-initialized gate: v1 control, then the v2 probe.
//! 3. Site B — `tasks/result` pending: the typed-surface trace, the REAL v2 HTTP
//!    probe, and the v1 controls for both.
//! 4. The inventory of every code a v2 request elicited here, asserted disjoint
//!    from `MUST_NOT_EMIT` and justified where it lands in the SHOULD-NOT range.
//! 5. The source tripwire over `V1_TASK_PENDING` emission sites.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::v2::{
    header, post, spawn_default_config, v1_body, v2_body, v2_headers, Resp, META_PROTOCOL_VERSION,
    V1, V2,
};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::{CallToolRequest, ClientRequest, JSONRPCResponse, Request, RequestId};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};

// ===========================================================================
// 1. The prohibition, as data.
// ===========================================================================

/// The codes protocol version 2026-07-28 **MUST NOT** emit.
///
/// Source: `docs/specification/draft/basic/index.mdx` § Error Codes. That section
/// is ABSENT at the `2026-07-28-RC` tag and was added to the draft afterwards, so
/// it is a post-RC obligation — which is why pmcp shipped two emission sites
/// before anyone had to think about it. Recorded as Finding 11 of
/// `113-SPEC-RECHECK-ADDENDUM-2026-07-26.md`.
///
/// `-32042` has never had a name or a call site in pmcp; it is carried here so
/// that the assertion is over the RULE rather than over the one number pmcp
/// happens to use today.
const MUST_NOT_EMIT: [i32; 2] = [-32002, -32042];

/// The range the same section says new implementations **SHOULD NOT** use.
///
/// This is a SHOULD NOT, not a MUST NOT, and this file does not change a single
/// code because of it. It inventories the residual instead, so that "we did not
/// look" and "we looked and decided" are distinguishable a year from now.
const SHOULD_NOT_USE_RANGE: std::ops::RangeInclusive<i32> = -32019..=-32000;

/// A code in [`SHOULD_NOT_USE_RANGE`] that a v2 request can genuinely elicit
/// from pmcp today, with the decision that keeps it.
struct ShouldNotEntry {
    code: i32,
    symbol: &'static str,
    justification: &'static str,
}

/// A justification shorter than this is a label, not a decision.
const MIN_JUSTIFICATION_CHARS: usize = 40;

/// Every `-32000`..`-32019` code a v2 request can elicit from pmcp, each with the
/// written reason it stays.
///
/// This list is the SHOULD-NOT residual. It is allowed to be non-empty — the spec
/// text is a SHOULD NOT — but it is not allowed to be unmeasured.
const SHOULD_NOT_ALLOWLIST: &[ShouldNotEntry] = &[ShouldNotEntry {
    code: -32005,
    symbol: "RATE_LIMITED",
    justification:
        "Plan 113-18 deliberately routed all three `subscriptions/listen` refusals (per-principal \
         stream cap, global stream cap, duplicate subscription id) to RATE_LIMITED at HTTP 200, \
         because the v2 transport has no spec-allocated code for a resource-exhaustion refusal and \
         inventing a `-3202x` value would spend a second exception against VERS-06. It is a SHOULD \
         NOT rather than a MUST NOT, the decision is recorded in that plan's summary, and \
         re-litigating it is outside this file's fence.",
}];

/// The codes the executed probes in this file actually observed on a v2 request.
///
/// Collected by the probes at runtime rather than declared, so the inventory
/// assertion is over MEASURED behaviour. A `None` entry means the v2 request
/// succeeded, which is itself a fact worth recording.
fn record(observed: &mut BTreeSet<i32>, response: &JSONRPCResponse) -> Option<i32> {
    match &response.payload {
        ResponsePayload::Error(err) => {
            observed.insert(err.code);
            Some(err.code)
        },
        ResponsePayload::Result(_) => None,
    }
}

/// The error message of a response, or `None` when it succeeded.
fn error_message(response: &JSONRPCResponse) -> Option<&str> {
    match &response.payload {
        ResponsePayload::Error(err) => Some(err.message.as_str()),
        ResponsePayload::Result(_) => None,
    }
}

/// The error code of a response, or `None` when it succeeded.
fn error_code(response: &JSONRPCResponse) -> Option<i32> {
    match &response.payload {
        ResponsePayload::Error(err) => Some(err.code),
        ResponsePayload::Result(_) => None,
    }
}

/// The `error.code` carried by an HTTP response body, or `None` when it carried a
/// result.
fn http_error_code(resp: &Resp) -> Option<i64> {
    resp.body.get("error")?.get("code")?.as_i64()
}

/// The `error.message` carried by an HTTP response body.
fn http_error_message(resp: &Resp) -> Option<&str> {
    resp.body.get("error")?.get("message")?.as_str()
}

fn assert_not_prohibited(code: Option<i32>, site: &str) {
    let Some(code) = code else {
        return;
    };
    assert!(
        !MUST_NOT_EMIT.contains(&code),
        "{site}: a v2 request elicited {code}, which protocol version 2026-07-28 MUST NOT emit \
         (docs/specification/draft/basic/index.mdx § Error Codes; Finding 11). Era-gate the site \
         — do not change the v1 wire value, which is frozen."
    );
}

// ===========================================================================
// 2. Site A — `ServerCore`'s server-not-initialized gate.
// ===========================================================================

/// The message `src/server/core.rs`'s not-initialized gate emits. Spelled here so
/// a probe that never reaches the gate fails on the MESSAGE rather than passing
/// vacuously on the code.
const NOT_INITIALIZED_MESSAGE: &str = "Server not initialized. Call initialize first.";

/// A tool that echoes the ingress-resolved era, so a SUCCESSFUL v2 probe proves
/// positively that the request was classified v2 and reached dispatch — the
/// anti-vacuity half of "the gate no longer fires".
struct EraEchoTool;

#[async_trait::async_trait]
impl ToolHandler for EraEchoTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "era": extra.era().map(|era| format!("{era:?}")) }))
    }
}

/// A `ServerCore` that is v2-opted-in, NOT stateless, and NOT initialised — the
/// exact configuration in which the not-initialized gate is live.
///
/// `stateless_mode(false)` is explicit rather than defaulted: the field is
/// resolved in `ServerCoreBuilder::build` as
/// `self.stateless_mode.unwrap_or_else(Self::detect_stateless_environment)`, i.e.
/// by ENVIRONMENT auto-detection. A test that let it default would silently pass
/// on a machine whose environment looked serverless.
fn uninitialised_v2_core() -> pmcp::server::core::ServerCore {
    ServerCoreBuilder::new()
        .name("prohibited-codes-probe")
        .version("1.0.0")
        .tool("probe", EraEchoTool)
        .stateless_mode(false)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .build()
        .expect("core builds")
}

/// A `tools/call` carrying the reserved v2 `_meta` era signal.
///
/// The reserved key comes from `pmcp::testing::META_PROTOCOL_VERSION` (re-exported
/// through the shared harness), which is the SHIPPED constant — not a re-spelled
/// literal that could drift from what the resolver reads.
fn v2_probe_call() -> Request {
    let mut request = CallToolRequest::new("probe", json!({}));
    request._meta =
        Some(pmcp::types::protocol::RequestMeta::new().with_meta(META_PROTOCOL_VERSION, json!(V2)));
    Request::Client(Box::new(ClientRequest::CallTool(request)))
}

/// The same `tools/call` with NO `_meta` at all — a v1 request.
fn v1_probe_call() -> Request {
    Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
        "probe",
        json!({}),
    ))))
}

/// NEGATIVE CONTROL A — the v1 wire at site A, byte-identical.
///
/// GREEN before the era guard and GREEN after it. If this ever goes red, the
/// guard caught v1 traffic and the guard is wrong, not this test.
#[tokio::test]
async fn site_a_v1_uninitialised_request_still_emits_minus_32002() {
    let core = uninitialised_v2_core();
    let response = core
        .handle_request(RequestId::from(1i64), v1_probe_call(), None)
        .await;

    assert_eq!(
        error_message(&response),
        Some(NOT_INITIALIZED_MESSAGE),
        "the v1 not-initialized gate must still fire with its existing message"
    );
    assert_eq!(
        error_code(&response),
        Some(-32002),
        "the v1 wire value is FROZEN at -32002"
    );
}

/// PROBE A — executed reachability trace at `src/server/core.rs`.
///
/// The v1 control is re-driven against the SAME core FIRST, so this test cannot
/// pass vacuously: if the gate were not live on this configuration (wrong
/// `stateless_mode`, an already-initialised core, a missing tool) the control
/// fails loudly on the message before anything is claimed about v2.
///
/// Then the v2 request. Before the era guard this emitted `-32002`; after it, the
/// gate skips and the request reaches the handler, which echoes `Era::V2` — the
/// positive proof that the v2 request was classified v2 rather than merely
/// avoiding the gate by taking some other path.
#[tokio::test]
async fn site_a_v2_request_must_not_elicit_a_prohibited_code() {
    let core = uninitialised_v2_core();

    // Branch arrival, proven positively on the SAME core before any v2 claim.
    let control = core
        .handle_request(RequestId::from(2i64), v1_probe_call(), None)
        .await;
    assert_eq!(
        error_message(&control),
        Some(NOT_INITIALIZED_MESSAGE),
        "the not-initialized gate is not live on this core — every v2 assertion below would be \
         vacuous"
    );

    let response = core
        .handle_request(RequestId::from(3i64), v2_probe_call(), None)
        .await;

    assert_not_prohibited(
        error_code(&response),
        "src/server/core.rs not-initialized gate",
    );

    // Positive proof the request really was a v2 request that reached dispatch.
    match &response.payload {
        ResponsePayload::Result(result) => {
            let text = result["content"][0]["text"]
                .as_str()
                .expect("probe result carries text content");
            let payload: Value = serde_json::from_str(text).expect("probe text content is JSON");
            assert_eq!(
                payload["era"], "V2",
                "the v2 probe reached the handler but was not classified v2 — the code assertion \
                 above would be vacuous"
            );
        },
        ResponsePayload::Error(err) => panic!(
            "the v2 probe was refused with {} ({}) — it did not reach dispatch, so the \
             prohibition assertion above proved nothing",
            err.code, err.message
        ),
    }
}

// ===========================================================================
// 3. Site B — `tasks/result` pending refusal.
// ===========================================================================

/// The message `src/server/task_dispatch.rs`'s no-router pending branch emits.
const TASK_PENDING_MESSAGE: &str = "task result not available: task not completed";

/// A `Server` with a `TaskStore` and NO `TaskRouter` — the configuration in which
/// the `tasks/result` pending refusal is live.
///
/// v2-opted-in, so the HTTP probe below can reach `Era::V2`.
fn task_store_v2_server(name: &str) -> Server {
    Server::builder()
        .name(name)
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", common::v2::SearchTool)
        .task_store(Arc::new(pmcp::InMemoryTaskStore::new()))
        .build()
        .expect("server builds")
}

/// The `ServerCore` twin of [`task_store_v2_server`], for the TYPED-surface trace.
///
/// `Server::handle_request` is private, so the typed dispatch surface an embedder
/// can actually reach is `ServerCore`'s public `ProtocolHandler`. `stateless_mode`
/// is TRUE here on purpose: site A's gate would otherwise refuse the request
/// before it ever reached the tasks route, and this probe is about site B.
fn task_store_v2_core() -> pmcp::server::core::ServerCore {
    ServerCoreBuilder::new()
        .name("task-pending-typed-surface")
        .version("1.0.0")
        .tool("probe", EraEchoTool)
        .stateless_mode(true)
        .task_store(Arc::new(pmcp::InMemoryTaskStore::new()))
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .build()
        .expect("core builds")
}

/// Mint a v1 session on a STATEFUL-config server, where sessions exist.
///
/// The default `StreamableHttpServerConfig` refuses a non-initialize v1 request
/// with `-32600 "Session ID required for non-initialization requests"` before
/// dispatch, so without this the v1 control below would never reach the tasks
/// route — it would measure the session gate instead of the pending branch.
///
/// Returns `None` on a `--no-default-features --features full-v2` build, where
/// the session-required gate does not exist either: `validate_non_init_session`
/// is the null twin, so the control leg reaches the same pending branch WITHOUT
/// a session id. Returning an `Option` rather than `#[cfg]`-ing the whole probe
/// away is what keeps the v2 half of this test — the part Finding 11 is actually
/// about — RUNNING on the severed build, with its non-vacuity control intact.
async fn v1_session(addr: std::net::SocketAddr) -> Option<String> {
    if !cfg!(feature = "v1-compat") {
        return None;
    }
    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(1),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    Some(
        init.mcp_session_id
            .unwrap_or_else(|| panic!("a v1 initialize mints a session; body was {}", init.raw)),
    )
}

/// A `tasks/result` request through the TYPED `ClientRequest` enum.
fn typed_tasks_result(task_id: &str) -> Request {
    Request::Client(Box::new(ClientRequest::TasksResult(
        pmcp::types::tasks::GetTaskPayloadRequest {
            task_id: task_id.to_string(),
        },
    )))
}

/// NEGATIVE CONTROL B — the v1 wire at site B, byte-identical.
#[tokio::test]
async fn site_b_v1_pending_tasks_result_still_emits_minus_32002() {
    let core = task_store_v2_core();
    let response = core
        .handle_request(RequestId::from(10i64), typed_tasks_result("absent"), None)
        .await;

    assert_eq!(
        error_message(&response),
        Some(TASK_PENDING_MESSAGE),
        "the v1 pending refusal must still fire with its existing message"
    );
    assert_eq!(
        error_code(&response),
        Some(-32002),
        "the v1 wire value is FROZEN at -32002"
    );
}

/// The TYPED-surface half of the site-B trace, executed rather than argued.
///
/// `ClientRequest::TasksResult` is one of the enumerated NON-`_meta`-bearing
/// variants in `extract_request_meta_value`, so the typed dispatch surfaces
/// (`ServerCore::handle_request` / `Server::handle_request`) cannot carry an era
/// signal on a `tasks/result` at all. That makes this surface v1-only BY
/// CONSTRUCTION — which is a real reachability answer, and also the reason the
/// decisive probe below has to cross the HTTP boundary, where `params._meta` is
/// read off the RAW body instead.
///
/// Executed, not inspected: the assertion is that a request carrying the reserved
/// era key in a place the typed variant has no field for still lands on the v1
/// branch.
#[tokio::test]
async fn site_b_typed_surface_cannot_carry_a_v2_era_signal() {
    let core = task_store_v2_core();

    // Anti-vacuity: the SAME core does resolve `Era::V2` for a request shape that
    // CAN carry `_meta`, so what follows measures the variant, not the fixture.
    let era_probe = core
        .handle_request(RequestId::from(11i64), v2_probe_call(), None)
        .await;
    let ResponsePayload::Result(result) = &era_probe.payload else {
        panic!("the era probe was refused: {:?}", era_probe.payload);
    };
    let text = result["content"][0]["text"].as_str().expect("text content");
    let payload: Value = serde_json::from_str(text).expect("probe text content is JSON");
    assert_eq!(
        payload["era"], "V2",
        "this core does not resolve Era::V2 at all — the tasks/result measurement below would be \
         about the fixture rather than about the variant"
    );

    let response = core
        .handle_request(RequestId::from(12i64), typed_tasks_result("absent"), None)
        .await;

    assert_eq!(
        error_message(&response),
        Some(TASK_PENDING_MESSAGE),
        "the typed surface must still reach the pending branch"
    );
    assert_eq!(
        error_code(&response),
        Some(-32002),
        "on the typed surface a tasks/result has nowhere to put an era signal, so it is v1 by \
         construction and keeps the frozen value"
    );
}

/// PROBE B — the decisive, executed reachability trace at
/// `src/server/task_dispatch.rs`, over a REAL v2 HTTP request.
///
/// The v1 control runs FIRST against the same live server, so the probe cannot
/// pass vacuously: it proves the pending branch is reachable on this fixture and
/// emits the message this file names.
///
/// Then the same request with the three v2 headers and the reserved `_meta` era
/// key. On this path the era comes from `params._meta` in the RAW body, which is
/// why it reaches `Era::V2` even though `GetTaskPayloadRequest` has no `_meta`
/// field.
#[tokio::test]
async fn site_b_v2_http_request_must_not_elicit_a_prohibited_code() {
    let (addr, handle) = spawn_default_config(task_store_v2_server("task-pending-http")).await;

    // Branch arrival, proven positively on the SAME live server. The default
    // config is STATEFUL, so the v1 leg has to mint a session first — v2 does
    // not, which is HTTP-01 and is why only this leg needs the handshake.
    let session = v1_session(addr).await;
    let control_headers: Vec<_> = session
        .as_deref()
        .map(|sid| vec![header(MCP_SESSION_ID, sid)])
        .unwrap_or_default();
    let control = post(
        addr,
        &control_headers,
        &v1_body("tasks/result", json!(2), json!({ "taskId": "absent" })),
    )
    .await;
    assert_eq!(
        http_error_message(&control),
        Some(TASK_PENDING_MESSAGE),
        "the v1 pending branch is not reachable on this fixture — every v2 assertion below would \
         be vacuous. body was {}",
        control.raw
    );
    assert_eq!(
        http_error_code(&control),
        Some(-32002),
        "the v1 wire value is FROZEN at -32002"
    );

    let probe = post(
        addr,
        &v2_headers("tasks/result", ""),
        &v2_body("tasks/result", json!(2), json!({ "taskId": "absent" })),
    )
    .await;
    handle.abort();

    let code = http_error_code(&probe);
    assert!(
        code.is_none_or(|code| !MUST_NOT_EMIT.contains(&i32::try_from(code).unwrap_or(0))),
        "src/server/task_dispatch.rs tasks/result: a v2 request elicited {code:?}, which protocol \
         version 2026-07-28 MUST NOT emit (Finding 11). body was {}",
        probe.raw
    );
    // Positive proof the probe crossed the v2 gate rather than being refused
    // before dispatch: an accepted v2 request echoes its three headers back.
    assert_eq!(
        probe.mcp_method.as_deref(),
        Some("tasks/result"),
        "the v2 probe did not pass the header gate, so the assertion above proved nothing. body \
         was {}",
        probe.raw
    );
    // And the replacement answer, pinned. `METHOD_NOT_FOUND` is the truthful
    // one: `tasks/result` is ABSENT from the vendored tasks-extension schema,
    // which declares only `tasks/get`, `tasks/update` and `tasks/cancel`. The
    // method does not exist on this protocol version.
    //
    // This comment used to say the refusal was `METHOD_NOT_FOUND` because pmcp
    // advertised no `io.modelcontextprotocol/tasks` entry. Plan 114-05 made
    // every backend-configured server advertise exactly that entry, and plan
    // 114-08 replaced the resulting untruthful "the tasks extension is not
    // negotiated" wire message — which told the caller to fix a negotiation
    // that had already succeeded — with the retirement message asserted below.
    // The assertion's INTENT is unchanged: the refusal must say WHY.
    assert_eq!(
        code,
        Some(i64::from(
            pmcp::types::protocol::error_codes::METHOD_NOT_FOUND
        )),
        "body was {}",
        probe.raw
    );
    assert!(
        http_error_message(&probe)
            .is_some_and(|m| m.contains(pmcp::testing::V2_TASKS_METHOD_RETIRED)),
        "the v2 refusal must say WHY, not just refuse. body was {}",
        probe.raw
    );
}

// ===========================================================================
// 4. The inventory.
// ===========================================================================

/// Every code a v2 request elicited from the paths this suite drives, asserted
/// disjoint from [`MUST_NOT_EMIT`] and justified where it falls in
/// [`SHOULD_NOT_USE_RANGE`].
///
/// The set is COLLECTED by re-driving the v2 probes, not declared, so it cannot
/// drift away from what the server does.
#[tokio::test]
async fn every_code_a_v2_request_elicits_here_is_inventoried() {
    let mut observed: BTreeSet<i32> = BTreeSet::new();

    // Site A, v2.
    let core = uninitialised_v2_core();
    let a = core
        .handle_request(RequestId::from(20i64), v2_probe_call(), None)
        .await;
    record(&mut observed, &a);

    // Site B, v2, over the real HTTP boundary.
    let (addr, handle) = spawn_default_config(task_store_v2_server("inventory-http")).await;
    let b = post(
        addr,
        &v2_headers("tasks/result", ""),
        &v2_body("tasks/result", json!(1), json!({ "taskId": "absent" })),
    )
    .await;
    handle.abort();
    if let Some(code) = http_error_code(&b) {
        observed.insert(i32::try_from(code).expect("a JSON-RPC error code fits in i32"));
    }

    for code in &observed {
        assert!(
            !MUST_NOT_EMIT.contains(code),
            "a v2 request elicited the prohibited code {code}"
        );
        if !SHOULD_NOT_USE_RANGE.contains(code) {
            continue;
        }
        let entry = SHOULD_NOT_ALLOWLIST.iter().find(|e| e.code == *code);
        assert!(
            entry.is_some(),
            "a v2 request elicited {code}, which is in the SHOULD-NOT range \
             {SHOULD_NOT_USE_RANGE:?} and is not in SHOULD_NOT_ALLOWLIST. Either move it out of \
             the range or record the decision to keep it."
        );
    }
}

/// The SHOULD-NOT residual is a decision, not a label.
#[test]
fn every_should_not_entry_carries_a_substantive_justification() {
    for entry in SHOULD_NOT_ALLOWLIST {
        assert!(
            SHOULD_NOT_USE_RANGE.contains(&entry.code),
            "SHOULD_NOT_ALLOWLIST entry {} ({}) is not in the SHOULD-NOT range",
            entry.symbol,
            entry.code
        );
        assert!(
            entry.justification.trim().len() >= MIN_JUSTIFICATION_CHARS,
            "SHOULD_NOT_ALLOWLIST entry {} needs a real decision, not {:?}",
            entry.symbol,
            entry.justification
        );
    }
}

/// `UNSUPPORTED_CAPABILITY` squats on the same prohibited number as
/// `V1_TASK_PENDING` (`-32002`). This measures the fact that makes it safe, which
/// was previously written down nowhere: **it is never emitted.**
///
/// The scan finds exactly two files, and both are DECLARATIONS rather than
/// emissions:
///
/// * `src/types/protocol/error_codes.rs` — `pub const UNSUPPORTED_CAPABILITY`;
/// * `src/error/mod.rs` — `ErrorCode::UNSUPPORTED_CAPABILITY`, a public
///   associated const that DELEGATES to that table entry.
///
/// The decisive half is the second assertion: the delegating const has zero USE
/// sites in compiled `src/`, so no code path anywhere constructs an error with
/// it. A third file naming the table symbol, or any use of the delegating const,
/// fails here — because either would be a `-32002` on a wire under a different
/// name.
#[test]
fn unsupported_capability_is_declared_twice_and_emitted_never() {
    let declaring: Vec<String> = emission_sites("UNSUPPORTED_CAPABILITY")
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        declaring,
        vec![
            "src/error/mod.rs".to_string(),
            "src/types/protocol/error_codes.rs".to_string(),
        ],
        "UNSUPPORTED_CAPABILITY (-32002) is named outside its two declaration sites. It shares \
         the prohibited number with V1_TASK_PENDING; emitting it on a v2 path is the same \
         conformance violation under a different name."
    );

    let uses = emission_sites("ErrorCode::UNSUPPORTED_CAPABILITY");
    assert!(
        uses.is_empty(),
        "ErrorCode::UNSUPPORTED_CAPABILITY is now used at {uses:?}. It resolves to -32002, which \
         protocol version 2026-07-28 MUST NOT emit; any use needs an era guard and an entry in \
         V1_TASK_PENDING_SITES' sibling reasoning."
    );
}

// ===========================================================================
// 5. The source tripwire over `V1_TASK_PENDING` emission sites.
// ===========================================================================
//
// Modelled on `tests/v2_bounded_reads_tripwire.rs` (plan 113-21): runtime source
// discovery from `CARGO_MANIFEST_DIR`, comment/literal stripping with a line map,
// `cfg(test)`-region exclusion, and a justified allowlist whose entries rot-check
// themselves. The scanner primitives are re-stated here rather than shared
// because a Rust integration test is its own crate and the two files cannot
// import each other; the IDIOM is deliberately identical so the repository has
// one source-scanning shape, not two.

/// The two shapes an allowlist entry can have, kept distinct so a DEFINITION site
/// can never be mistaken for an EMISSION site.
///
/// `src/types/protocol/error_codes.rs` is expected in this list as the
/// DEFINITION: it declares the constant and discusses it. Every other file that
/// names the symbol outside a comment is emitting or consuming it and must say
/// which era guard keeps it off the v2 wire.
enum SiteKind {
    /// The `pub const` declaration itself. Carries no era guard by definition.
    Definition,
    /// A site that writes the code onto a response, or reads it for comparison.
    /// `guard` is a substring that MUST appear in the same file — the era
    /// predicate that keeps this site off the v2 path.
    Emission { guard: &'static str },
}

struct SiteEntry {
    path: &'static str,
    kind: SiteKind,
    why: &'static str,
}

/// Every compiled `src/` file allowed to name `V1_TASK_PENDING` outside a comment.
///
/// Adding an unlisted file fails. Deleting a listed file's last non-comment
/// mention without deleting its entry fails. Deleting an emission site's era
/// guard fails. This is the structural half of Finding 11: nothing today stops a
/// future plan from reading this public constant into a v2 branch, and this list
/// is what makes that attempt loud.
const V1_TASK_PENDING_SITES: &[SiteEntry] = &[
    SiteEntry {
        path: "src/types/protocol/error_codes.rs",
        kind: SiteKind::Definition,
        why: "The DEFINITION site: `pub const V1_TASK_PENDING: i32 = -32002;` plus the \
              consistency tests that pin its value. It declares the number, it never writes it to \
              a wire, and it therefore carries no era guard.",
    },
    SiteEntry {
        path: "src/server/core.rs",
        kind: SiteKind::Emission {
            guard: "v1_initialize_gate_applies",
        },
        why: "The server-not-initialized gate in `ServerCore::handle_request_internal`. \
              `ProtocolHandler` is a PUBLIC trait, so this site is reachable without the \
              streamable-HTTP transport whose era gating Phase 113 built; probe A proved it \
              v2-reachable by execution. The named predicate is what keeps it v1-only.",
    },
    SiteEntry {
        path: "src/server/task_dispatch.rs",
        kind: SiteKind::Emission {
            guard: "is_v1_task_era",
        },
        why: "The `tasks/result` no-router pending refusal. Probe B proved it v2-reachable over \
              a real HTTP request, where the era rides in `params._meta` on the raw body rather \
              than on the typed `GetTaskPayloadRequest`. The named predicate routes the v2 \
              branch to METHOD_NOT_FOUND instead.",
    },
];

/// A file that names `V1_TASK_PENDING` ONLY inside a `cfg(test)` region.
///
/// It is not an allowlist — it is the anti-vacuity fixture for the exclusion:
/// `the_cfg_test_exclusion_is_load_bearing_on_a_real_file` asserts this file DOES
/// name the token when `cfg(test)` regions are included and does NOT when they
/// are excluded. Without that, an over-eager exclusion could empty the whole scan
/// and every check above would pass over nothing.
const TEST_ONLY_MENTION: &str = "src/server/streamable_http_server.rs";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `.rs` file under `src/`, discovered at runtime so a NEW file cannot
/// escape the scan by nobody remembering to add it.
fn src_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(&repo_root().join("src"), &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "src/ carries well over fifty files; discovering {} means the walk is broken and every \
         check below would pass vacuously",
        files.len()
    );
    files
}

// --- source stripping (comments and literal contents removed, line map kept) ---

/// Source with whitespace collapsed, comments removed and every string/char
/// literal's CONTENT removed (delimiters kept).
///
/// `lines[i]` is the 1-based source line of `text`'s i-th byte.
#[derive(Default)]
struct Stripped {
    text: String,
    lines: Vec<u32>,
}

impl Stripped {
    fn push_char(&mut self, ch: char, line: u32) {
        self.text.push(ch);
        for _ in 0..ch.len_utf8() {
            self.lines.push(line);
        }
    }

    fn push_delims(&mut self, delims: &str, line: u32) {
        for ch in delims.chars() {
            self.push_char(ch, line);
        }
    }
}

fn line_of(stripped: &Stripped, index: usize) -> u32 {
    stripped.lines.get(index).copied().unwrap_or(0)
}

struct Construct {
    end: usize,
    delims: &'static str,
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn line_numbers(chars: &[char]) -> Vec<u32> {
    let mut lines = Vec::with_capacity(chars.len());
    let mut line: u32 = 1;
    for &ch in chars {
        lines.push(line);
        if ch == '\n' {
            line += 1;
        }
    }
    lines
}

fn end_of_line(chars: &[char], from: usize) -> usize {
    let mut j = from;
    while j < chars.len() && chars[j] != '\n' {
        j += 1;
    }
    j
}

/// End of a block comment, honouring Rust's comment nesting.
fn end_of_block_comment(chars: &[char], from: usize) -> usize {
    let mut depth: usize = 0;
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
            depth += 1;
            j += 2;
        } else if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return j;
            }
        } else {
            j += 1;
        }
    }
    chars.len()
}

fn end_of_string(chars: &[char], from: usize) -> usize {
    let mut j = from + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '"' => return j + 1,
            _ => j += 1,
        }
    }
    chars.len()
}

/// End of an `r"..."` / `r#"..."#` raw string starting at `from`.
fn raw_string_end(chars: &[char], from: usize) -> Option<usize> {
    let mut hashes: usize = 0;
    let mut j = from + 1;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < chars.len() {
        if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(chars.len())
}

/// End of a char literal, or `None` when the tick opens a LIFETIME.
fn end_of_char_literal(chars: &[char], from: usize) -> Option<usize> {
    let c1 = *chars.get(from + 1)?;
    if c1 == '\\' {
        let mut j = from + 3;
        while j < chars.len() && chars[j] != '\'' {
            j += 1;
        }
        return Some((j + 1).min(chars.len()));
    }
    if chars.get(from + 2) == Some(&'\'') {
        return Some(from + 3);
    }
    None
}

fn skip_construct(chars: &[char], i: usize, prev_ident: bool) -> Option<Construct> {
    let next = chars.get(i + 1).copied();
    match chars[i] {
        '/' if next == Some('/') => Some(Construct {
            end: end_of_line(chars, i),
            delims: "",
        }),
        '/' if next == Some('*') => Some(Construct {
            end: end_of_block_comment(chars, i),
            delims: "",
        }),
        '"' => Some(Construct {
            end: end_of_string(chars, i),
            delims: "\"\"",
        }),
        '\'' => end_of_char_literal(chars, i).map(|end| Construct { end, delims: "''" }),
        'r' if !prev_ident => raw_string_end(chars, i).map(|end| Construct {
            end,
            delims: "\"\"",
        }),
        'b' if !prev_ident && next == Some('r') => {
            raw_string_end(chars, i + 1).map(|end| Construct {
                end,
                delims: "\"\"",
            })
        },
        _ => None,
    }
}

/// Strip `source` to scannable text plus a byte-to-line map.
///
/// # One deliberate divergence from `tests/v2_bounded_reads_tripwire.rs`
///
/// That scanner REMOVES whitespace entirely, because its needles are method
/// chains (`.collect().await`) that rustfmt breaks across lines, and removal is
/// what lets a broken chain match as one needle.
///
/// This scanner matches IDENTIFIERS, which need word boundaries, so whitespace
/// runs collapse to a single space instead of vanishing. Removing them entirely
/// turns `pub const V1_TASK_PENDING` into `pubconstV1_TASK_PENDING`, where the
/// character before the token is `t` — an identifier character — so the
/// whole-token filter rejects the DEFINITION site and the scan silently loses
/// coverage of the very file it is scanning for. That was measured, not
/// predicted: the first run of this file reported `error_codes.rs` as naming
/// `UNSUPPORTED_CAPABILITY` nowhere.
fn strip(source: &str) -> Stripped {
    let chars: Vec<char> = source.chars().collect();
    let lines = line_numbers(&chars);
    let mut out = Stripped::default();
    let mut i: usize = 0;
    let mut prev_ident = false;
    let mut pending_space = false;
    while i < chars.len() {
        if let Some(construct) = skip_construct(&chars, i, prev_ident) {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            out.push_delims(construct.delims, lines[i]);
            i = construct.end.max(i + 1);
            prev_ident = false;
            continue;
        }
        let ch = chars[i];
        if ch.is_whitespace() {
            prev_ident = false;
            pending_space = true;
        } else {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            out.push_char(ch, lines[i]);
            prev_ident = is_ident_char(ch);
        }
        i += 1;
    }
    out
}

// --- `cfg(test)` region exclusion ---

fn balanced_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (opener, closer) = match bytes.get(open)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'{' => (b'{', b'}'),
        _ => return None,
    };
    let mut depth: usize = 0;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        if *byte == opener {
            depth += 1;
        } else if *byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

fn split_top_level(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start: usize = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..idx].trim());
                start = idx + 1;
            },
            _ => {},
        }
    }
    parts.push(inner[start..].trim());
    parts
}

/// Whether a `cfg` predicate can only hold when `test` is enabled.
fn cfg_requires_test(predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate == "test" {
        return true;
    }
    let Some(inner) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    split_top_level(inner).into_iter().any(cfg_requires_test)
}

fn item_span(text: &str, from: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => i = balanced_end(text, i)? + 1,
            b';' | b',' => return Some(from..i + 1),
            b'{' => return balanced_end(text, i).map(|end| from..end + 1),
            _ => i += 1,
        }
    }
    None
}

/// Every region of `stripped` that only compiles under `cfg(test)`.
fn cfg_test_spans(stripped: &Stripped) -> Vec<Range<usize>> {
    let text = &stripped.text;
    let mut spans = Vec::new();
    let mut search: usize = 0;
    while let Some(found) = text[search..].find("#[cfg(") {
        let paren = search + found + "#[cfg".len();
        let Some(close) = balanced_end(text, paren) else {
            break;
        };
        let predicate = &text[paren + 1..close];
        search = close + 1;
        if !cfg_requires_test(predicate) {
            continue;
        }
        if let Some(span) = item_span(text, search) {
            search = span.end.max(search);
            spans.push(span);
        }
    }
    spans
}

fn is_excluded(spans: &[Range<usize>], index: usize) -> bool {
    spans.iter().any(|span| span.contains(&index))
}

fn occurrences(text: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from: usize = 0;
    while let Some(found) = text[from..].find(needle) {
        let at = from + found;
        out.push(at);
        from = at + 1;
    }
    out
}

/// A whole-token match: `V1_TASK_PENDING` must not be a substring of a longer
/// identifier such as `V1_TASK_PENDING_SITES`.
fn token_hits(text: &str, needle: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    occurrences(text, needle)
        .into_iter()
        .filter(|at| {
            let before_ok = *at == 0 || !is_ident_char(char::from(bytes[at - 1]));
            let after = at + needle.len();
            let after_ok = after >= bytes.len() || !is_ident_char(char::from(bytes[after]));
            before_ok && after_ok
        })
        .collect()
}

/// Every compiled-`src/` file naming `symbol` outside a comment, a literal and a
/// `cfg(test)` region, with the source lines of each hit.
fn emission_sites(symbol: &str) -> Vec<(String, Vec<u32>)> {
    let mut sites = Vec::new();
    for path in src_files() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let stripped = strip(&source);
        let excluded = cfg_test_spans(&stripped);
        let lines: Vec<u32> = token_hits(&stripped.text, symbol)
            .into_iter()
            .filter(|at| !is_excluded(&excluded, *at))
            .map(|at| line_of(&stripped, at))
            .collect();
        if !lines.is_empty() {
            sites.push((rel(&path), lines));
        }
    }
    sites
}

/// Every compiled-`src/` file naming `symbol` outside a comment and a literal,
/// INCLUDING `cfg(test)` regions — the counterpart used to prove the exclusion is
/// load-bearing rather than vacuous.
fn all_mentions(symbol: &str) -> Vec<String> {
    let mut files = Vec::new();
    for path in src_files() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let stripped = strip(&source);
        if !token_hits(&stripped.text, symbol).is_empty() {
            files.push(rel(&path));
        }
    }
    files
}

const V1_TASK_PENDING: &str = "V1_TASK_PENDING";

/// The population of SHIPPED `V1_TASK_PENDING` sites equals the declared
/// allowlist, and every EMISSION entry's named era guard is literally present in
/// its file.
///
/// An unlisted site fails. A missing guard fails. A stale entry fails.
#[test]
fn every_v1_task_pending_site_is_allowlisted_and_era_guarded() {
    let observed: Vec<String> = emission_sites(V1_TASK_PENDING)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let mut failures = String::new();

    for path in &observed {
        if !V1_TASK_PENDING_SITES.iter().any(|e| e.path == *path) {
            let _ = writeln!(
                failures,
                "\n  UNLISTED site: {path} names V1_TASK_PENDING (-32002).\n    \
                 Protocol version 2026-07-28 MUST NOT emit that code. Either era-gate the site and \
                 add a V1_TASK_PENDING_SITES entry naming the guard, or do not emit it."
            );
        }
    }

    for entry in V1_TASK_PENDING_SITES {
        if !observed.iter().any(|path| path == entry.path) {
            let _ = writeln!(
                failures,
                "\n  DEAD allowlist entry: {} no longer names V1_TASK_PENDING.\n    \
                 Delete the entry. A stale one is how a real new site hides under a number set \
                 for a site since removed.",
                entry.path
            );
            continue;
        }
        let SiteKind::Emission { guard } = entry.kind else {
            continue;
        };
        let source = fs::read_to_string(repo_root().join(entry.path))
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", entry.path));
        if !source.contains(guard) {
            let _ = writeln!(
                failures,
                "\n  MISSING era guard: {} no longer contains `{guard}`.\n    \
                 That expression is what keeps this -32002 emission off the v2 path. Removing it \
                 reopens Finding 11.",
                entry.path
            );
        }
    }

    assert!(
        failures.is_empty(),
        "Finding 11 tripwire — the V1_TASK_PENDING emission population changed:{failures}"
    );
}

/// ANTI-VACUITY — the `cfg(test)` exclusion is doing real work on a real file,
/// and the definition site survives it.
///
/// Two failure modes this catches, both of which would make the tripwire above
/// pass over an empty or wrong set:
///
/// * An exclusion that stopped working would pull [`TEST_ONLY_MENTION`] into the
///   shipped population, so the tripwire would fail for a bogus reason.
/// * An exclusion that over-reached (truncating at the FIRST `cfg(test)` marker,
///   which is the under-scan `tests/v2_bounded_reads_tripwire.rs` documents)
///   would drop the definition site and every emission site with it.
#[test]
fn the_cfg_test_exclusion_is_load_bearing_on_a_real_file() {
    let with_tests = all_mentions(V1_TASK_PENDING);
    let shipped: Vec<String> = emission_sites(V1_TASK_PENDING)
        .into_iter()
        .map(|(path, _)| path)
        .collect();

    assert!(
        with_tests.iter().any(|p| p == TEST_ONLY_MENTION),
        "{TEST_ONLY_MENTION} no longer names V1_TASK_PENDING at all, so it can no longer prove \
         the cfg(test) exclusion works. Point TEST_ONLY_MENTION at another test-only mention."
    );
    assert!(
        !shipped.iter().any(|p| p == TEST_ONLY_MENTION),
        "the cfg(test) exclusion stopped working: {TEST_ONLY_MENTION} names V1_TASK_PENDING only \
         inside its test module, yet it appears in the shipped population"
    );
    assert!(
        shipped.contains(&"src/types/protocol/error_codes.rs".to_string()),
        "the DEFINITION site vanished from the shipped population — the scan is over-stripping \
         and every check in this file would pass vacuously. Observed: {shipped:?}"
    );
}

/// The allowlist is a set of decisions, not a set of labels.
#[test]
fn every_v1_task_pending_entry_carries_a_substantive_justification() {
    let mut seen: Vec<&str> = Vec::new();
    for entry in V1_TASK_PENDING_SITES {
        let why = entry.why.trim();
        assert!(
            why.len() >= MIN_JUSTIFICATION_CHARS,
            "V1_TASK_PENDING_SITES entry {} needs a real justification, not {why:?}",
            entry.path
        );
        assert!(
            !seen.contains(&why),
            "V1_TASK_PENDING_SITES entry {} reuses another entry's justification verbatim; a \
             copy-pasted reason is not a reason",
            entry.path
        );
        seen.push(why);
        assert!(
            repo_root().join(entry.path).is_file(),
            "V1_TASK_PENDING_SITES entry {} does not exist",
            entry.path
        );
    }
    assert!(
        V1_TASK_PENDING_SITES
            .iter()
            .filter(|e| matches!(e.kind, SiteKind::Definition))
            .count()
            == 1,
        "there is exactly ONE definition site for V1_TASK_PENDING; more than one means the \
         constant was duplicated rather than referenced"
    );
}

// ===========================================================================
// Tests for the scanner itself — without these the tripwire can pass vacuously,
// which is the exact failure mode plan 113-09 found twice in this phase.
// ===========================================================================

mod scanner {
    use super::{
        cfg_requires_test, cfg_test_spans, is_excluded, line_of, src_files, strip, token_hits,
    };

    fn find_token(source: &str, needle: &str) -> Option<u32> {
        let stripped = strip(source);
        let at = *token_hits(&stripped.text, needle).first()?;
        Some(line_of(&stripped, at))
    }

    #[test]
    fn a_bare_emission_is_counted() {
        let source = "fn f() -> i32 {\n    error_codes::V1_TASK_PENDING\n}\n";
        assert_eq!(find_token(source, "V1_TASK_PENDING"), Some(2));
    }

    #[test]
    fn the_token_only_inside_a_comment_is_not_counted() {
        let line = "fn f() {\n    // returns V1_TASK_PENDING for a pending task\n    g();\n}\n";
        assert!(find_token(line, "V1_TASK_PENDING").is_none());

        let doc = "/// Locked to [`V1_TASK_PENDING`] by the regression test.\nfn f() {}\n";
        assert!(find_token(doc, "V1_TASK_PENDING").is_none());

        let inner = "//! - [`V1_TASK_PENDING`] — the FROZEN v1 task-pending code.\nfn f() {}\n";
        assert!(find_token(inner, "V1_TASK_PENDING").is_none());

        let block = "fn f() {\n/* V1_TASK_PENDING /* nested */ still comment */\nlet y = 1;\n}\n";
        assert!(find_token(block, "V1_TASK_PENDING").is_none());
        assert!(
            find_token(block, "let").is_some(),
            "a nested block comment must end where Rust says it ends"
        );
    }

    #[test]
    fn the_token_inside_a_string_literal_is_not_counted() {
        let source = "fn f() {\n    let msg = \"V1_TASK_PENDING\";\n}\n";
        assert!(find_token(source, "V1_TASK_PENDING").is_none());

        let raw = "fn f() {\n    let msg = r#\"V1_TASK_PENDING\"#;\n}\n";
        assert!(find_token(raw, "V1_TASK_PENDING").is_none());
    }

    #[test]
    fn the_token_inside_a_cfg_test_block_is_excluded_but_later_code_is_not() {
        let source = "#[cfg(test)]\nmod tests {\n    const A: i32 = V1_TASK_PENDING;\n}\n\n\
                      fn shipped() -> i32 {\n    V1_TASK_PENDING\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = token_hits(&stripped.text, "V1_TASK_PENDING");
        assert_eq!(hits.len(), 2, "both occurrences are lexically present");
        assert!(
            is_excluded(&spans, hits[0]),
            "the cfg(test) module body must be excluded"
        );
        assert!(
            !is_excluded(&spans, hits[1]),
            "production code AFTER a cfg(test) item must still be scanned — truncating at the \
             first marker is the under-scan this tripwire exists to prevent"
        );
    }

    #[test]
    fn a_longer_identifier_is_not_a_hit() {
        let source = "const V1_TASK_PENDING_SITES: &[u8] = &[];\n";
        assert!(
            find_token(source, "V1_TASK_PENDING").is_none(),
            "a substring of a longer identifier must not count as an emission site"
        );
    }

    #[test]
    fn cfg_requires_test_classifies_the_documented_predicate_shapes() {
        assert!(cfg_requires_test("test"));
        assert!(cfg_requires_test(
            "all(test, not(target_arch = \"wasm32\"), feature = \"streamable-http\")"
        ));
        assert!(
            !cfg_requires_test("any(feature = \"fuzzing\", test)"),
            "an any(...) item compiles WITHOUT test, so it ships and stays in scope"
        );
        assert!(!cfg_requires_test("feature = \"testing\""));
    }

    #[test]
    fn source_discovery_walks_the_whole_crate() {
        let files = src_files();
        let names: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        for required in [
            "src/server/core.rs",
            "src/server/task_dispatch.rs",
            "src/types/protocol/error_codes.rs",
        ] {
            assert!(
                names.iter().any(|n| n.ends_with(required)),
                "source discovery lost {required}"
            );
        }
    }
}
