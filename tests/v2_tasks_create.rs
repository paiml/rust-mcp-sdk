//! The v2 task-creation TRIGGER, over a REAL `tools/call` (Phase 114, plan 12 —
//! TASK-01 / TASK-04, DQ1).
//!
//! # What this suite is about, and what it deliberately is NOT
//!
//! `tests/v2_tasks_shapes.rs` (114-11) owns the SHAPE of a create that already
//! fired. This file owns the question one layer earlier: WHETHER it fires at all.
//! On MCP 2025-11-25 the trigger is `CallToolRequest.task`, the client's
//! signal-that-I-want-a-task field. That field **does not exist in the 2026-07-28
//! tasks extension**, so on v2 the trigger is the client's per-request
//! declaration of `io.modelcontextprotocol/tasks` — which is also the extension's
//! own precondition: a server MUST NOT return a `CreateTaskResult` to a client
//! that never declared the extension, because such a client has no rule for
//! reading a task handle back.
//!
//! # Every test drives a REAL `tools/call`
//!
//! 114-RESEARCH names the warning sign explicitly: *"a 'TASK-04 complete' claim
//! demonstrated only by a hand-built `CreateTaskResult` unit test rather than a
//! real v2 `tools/call` round trip."* There is not one hand-built envelope in
//! this file. Every assertion is on bytes that crossed a loopback TCP socket
//! through the real `StreamableHttpServer`, and test 1 follows the returned id
//! with a real `tasks/get` so the handle is proven USABLE, not merely
//! well-shaped.
//!
//! # The six properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `a_declaring_v2_client_receives_a_task_handle` | the v2 trigger fires, and the handle is real (a `tasks/get` on it succeeds) |
//! | 1b | `a_handler_declared_input_request_is_recorded_against_the_minted_id` | create -> pause, end to end, against the STORE-minted id |
//! | 2 | `a_non_declaring_v2_client_receives_an_ordinary_result` | the spec's MUST NOT, asserted on bytes AND on the store |
//! | 3 | `a_v2_client_sending_the_v1_task_field_still_needs_the_declaration` | the v1 field is INERT on v2 |
//! | 4 | `a_v1_client_still_triggers_with_the_task_field` | v1 creation unchanged, against a golden envelope literal |
//! | 5 | `a_declaring_client_on_a_tool_with_no_task_support_receives_an_ordinary_result` | the `TaskSupport` half of the gate still closes |
//! | 6 | `a_declaring_client_on_a_server_with_no_store_receives_an_ordinary_result` | the backend half of the gate still closes, with no error leak |
//!
//! Within each pair the NON-creating case is fired first, so a fixture that
//! created something by accident cannot be mistaken for the property under test.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    post, spawn_default_config, spawn_tasks_server_with_store, teardown, v1_body,
    v2_body_with_caps, v2_body_with_client_extensions, v2_headers, AuthPosture, OptionalBearer,
    Resp, PAUSING_TOOL_NAME, PAUSING_TOOL_REQUEST_KEY, TASKS_TOOL_NAME,
};
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::{TaskSupport, ToolExecution};
use pmcp::Server;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

// ===========================================================================
// Driving the server.
// ===========================================================================

/// The principal every authenticated request in this suite binds to.
///
/// [`AuthPosture::Optional`] plus an explicit bearer means the v2 identity table
/// binds the owner to this subject, so the suite can read the SAME store records
/// the server writes without depending on the anonymous bucket.
const SUBJECT: &str = "alice";

fn auth_header() -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {SUBJECT}"))]
}

/// A v2 request that DECLARES the tasks extension.
async fn declaring(addr: SocketAddr, method: &str, name: &str, id: i64, params: Value) -> Resp {
    let mut headers = v2_headers(method, name);
    headers.extend(auth_header());
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &headers, &body).await
}

/// The SAME v2 request with the extension NOT declared.
///
/// The three MRTR-fulfillable capabilities are still declared — this differs
/// from [`declaring`] in EXACTLY one key, the `extensions` map, so a difference
/// in outcome is attributable to the declaration and to nothing else.
async fn non_declaring(addr: SocketAddr, method: &str, name: &str, id: i64, params: Value) -> Resp {
    let mut headers = v2_headers(method, name);
    headers.extend(auth_header());
    let body = v2_body_with_caps(
        method,
        json!(id),
        params,
        json!({ "elicitation": {}, "sampling": {}, "roots": {} }),
    );
    post(addr, &headers, &body).await
}

/// A `tools/call` params object, optionally carrying the v1 `task` field.
fn call_params(tool: &str, with_v1_task_field: bool) -> Value {
    let mut params = json!({ "name": tool, "arguments": {} });
    if with_v1_task_field {
        params["task"] = json!({});
    }
    params
}

/// The `result` object of a success response, or a panic naming the error.
fn result_of(response: &Resp) -> &Value {
    response.body.get("result").unwrap_or_else(|| {
        panic!("expected a success result, got {}", response.raw);
    })
}

/// The store-minted task id from a FLAT v2 create response.
fn minted_id(response: &Resp) -> String {
    result_of(response)["taskId"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "a v2 create result carries a TOP-LEVEL taskId; got {}",
                response.raw
            )
        })
        .to_string()
}

/// Assert a response is an ORDINARY tool result, not a task handle.
///
/// Five independent facts, because "not a task" has five observable spellings
/// and a response could fail any one of them alone:
///
/// 1. the v2 envelope discriminator is `complete`, and `task` appears nowhere,
/// 2. the RAW bytes carry no PROTOCOL-level `"taskId":` key,
/// 3. there is no TOP-LEVEL `taskId` (the flat v2 handle),
/// 4. there is no `task` wrapper (the nested v1 handle),
/// 5. there is no `_meta.relatedTask` slot (the v1 envelope's id echo).
///
/// # Why fact 2 is spelled with its quotes
///
/// A bare `!raw.contains("taskId")` is UNSATISFIABLE for the pairs in this file,
/// and the reason is structural rather than incidental: the create gate only
/// fires on a value that is TASK-SHAPED (`taskId` + `status`), so "the same tool,
/// same server" forces the non-creating leg to text-wrap a payload that literally
/// spells `taskId`. MEASURED, on the real wire:
///
/// ```text
/// "result":{"content":[{"type":"text","text":"{\"taskId\":\"tool-fabricated\",…}"}],
///           "isError":false,"resultType":"complete", …}
/// ```
///
/// Inside the wrapped text the quotes are BACKSLASH-ESCAPED, so the nine-byte
/// protocol spelling `"taskId":` does not occur — while a real leaked handle,
/// which is a JSON key, always does. Checking the quoted spelling is therefore
/// both satisfiable and STRICTLY more precise than the bare substring: it
/// distinguishes a leaked protocol handle from a tool echoing its own payload,
/// which the bare check cannot do in either direction.
fn assert_ordinary_result(response: &Resp) {
    assert!(
        response.raw.contains("\"resultType\":\"complete\""),
        "an ordinary tool result carries resultType=complete: {}",
        response.raw
    );
    assert!(
        !response.raw.contains("\"resultType\":\"task\""),
        "the task disposition must NOT be minted here: {}",
        response.raw
    );
    assert!(
        !response.raw.contains("\"taskId\":"),
        "no PROTOCOL-level taskId key may appear anywhere in the bytes: {}",
        response.raw
    );
    let result = result_of(response);
    assert!(
        result.get("taskId").is_none(),
        "no TOP-LEVEL taskId may reach a caller that gets an ordinary result: {}",
        response.raw
    );
    assert!(
        result.get("task").is_none(),
        "no nested task wrapper either: {}",
        response.raw
    );
    assert!(
        result
            .get("_meta")
            .and_then(|meta| meta.get("io.modelcontextprotocol/related-task"))
            .is_none(),
        "no _meta.relatedTask id echo either: {}",
        response.raw
    );
}

/// Assert the STORE minted nothing for `SUBJECT`.
///
/// The byte assertions above prove no handle reached the CALLER. This proves no
/// task was created at all, which is the property T-114-60 actually cares about
/// and the one a "we returned the right envelope but wrote a record anyway"
/// regression would still violate.
async fn assert_store_is_empty(store: &Arc<InMemoryTaskStore>) {
    let (tasks, _cursor) = store
        .list(SUBJECT, None)
        .await
        .expect("listing an owner's tasks succeeds");
    assert!(
        tasks.is_empty(),
        "a closed create gate must mint NOTHING; the store held {tasks:?}"
    );
}

// ===========================================================================
// 1 / 1b / 2 / 3: the trigger, on one server, one tool.
// ===========================================================================

/// A DECLARING v2 client receives a real, POLLABLE task handle.
///
/// The non-creating leg fires FIRST, so the store is known empty at the moment
/// the declaring leg runs and the minted id cannot be a leftover.
#[tokio::test]
async fn a_declaring_v2_client_receives_a_task_handle() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    // Non-creating leg first.
    let ignored = non_declaring(
        addr,
        "tools/call",
        TASKS_TOOL_NAME,
        1,
        call_params(TASKS_TOOL_NAME, false),
    )
    .await;
    assert_ordinary_result(&ignored);
    assert_store_is_empty(&store).await;

    let created = declaring(
        addr,
        "tools/call",
        TASKS_TOOL_NAME,
        2,
        call_params(TASKS_TOOL_NAME, false),
    )
    .await;
    let task_id = minted_id(&created);
    // The handle must be REAL, not merely well-shaped: poll it.
    let polled = declaring(addr, "tasks/get", &task_id, 3, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    assert!(
        created.raw.contains("\"resultType\":\"task\""),
        "a declaring v2 client's create earns the task disposition: {}",
        created.raw
    );
    assert_eq!(
        result_of(&created)["status"],
        json!("working"),
        "{}",
        created.raw
    );
    assert_eq!(
        result_of(&polled)["taskId"],
        json!(task_id),
        "the returned handle must resolve through a real tasks/get: {}",
        polled.raw
    );
    assert_eq!(
        result_of(&polled)["status"],
        json!("working"),
        "{}",
        polled.raw
    );
}

/// The create -> pause loop, end to end.
///
/// A tool whose task-shaped value declares `inputRequests` and
/// `status: input_required` produces a handle whose `tasks/get` shows
/// `input_required` AND carries the SAME requests the handler declared.
///
/// This is the assertion that proves 114-14's `tasks/update` and 114-17's paired
/// example are reachable AT ALL. `store.create()` mints the canonical id AFTER
/// the handler has returned, so without this the handler's requests would be
/// recorded against an id no client ever sees — or, as before this plan, not
/// recorded at all — and both later plans would fail at wave 9 and wave 11
/// rather than here.
#[tokio::test]
async fn a_handler_declared_input_request_is_recorded_against_the_minted_id() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    // Non-creating leg first: the pausing tool must ALSO obey the trigger.
    let ignored = non_declaring(
        addr,
        "tools/call",
        PAUSING_TOOL_NAME,
        1,
        call_params(PAUSING_TOOL_NAME, false),
    )
    .await;
    assert_ordinary_result(&ignored);
    assert_store_is_empty(&store).await;

    let created = declaring(
        addr,
        "tools/call",
        PAUSING_TOOL_NAME,
        2,
        call_params(PAUSING_TOOL_NAME, false),
    )
    .await;
    let task_id = minted_id(&created);
    let polled = declaring(addr, "tasks/get", &task_id, 3, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    // The id the pause was recorded against is the STORE-minted one, never the
    // tool's fabricated `"tool-fabricated"`.
    assert_ne!(
        task_id, "tool-fabricated",
        "the wire id must be store-minted: {}",
        created.raw
    );
    assert_eq!(
        result_of(&polled)["status"],
        json!("input_required"),
        "the returned handle must already be PAUSED: {}",
        polled.raw
    );
    assert_eq!(
        result_of(&polled)["inputRequests"][PAUSING_TOOL_REQUEST_KEY]["method"],
        json!("roots/list"),
        "the inlined map must be the one the HANDLER declared: {}",
        polled.raw
    );
    assert!(
        polled.raw.contains("\"inputRequests\""),
        "the required key must survive egress to the WIRE: {}",
        polled.raw
    );
}

/// A NON-declaring v2 client never receives a task handle.
///
/// The extension's `MUST NOT return CreateTaskResult to a non-declaring client`,
/// asserted on the response bytes AND on the store.
///
/// # A measured correction to this plan's acceptance text
///
/// The plan asked for `!raw.contains("taskId")`. That is UNSATISFIABLE for this
/// pair and the reason is structural: the create gate only fires on a value that
/// is TASK-SHAPED (`taskId` + `status`), so "the same tool, same server" forces
/// the non-creating leg to text-wrap a payload that literally spells `taskId`
/// inside its `content` string. A byte-substring check therefore cannot
/// distinguish a leaked protocol handle from the tool's own echoed payload. The
/// assertions below measure the real property instead — no protocol-level handle
/// in ANY of its four spellings, and no store record — which a leak would fail
/// and the echoed text does not.
#[tokio::test]
async fn a_non_declaring_v2_client_receives_an_ordinary_result() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let response = non_declaring(
        addr,
        "tools/call",
        TASKS_TOOL_NAME,
        1,
        call_params(TASKS_TOOL_NAME, false),
    )
    .await;
    assert_ordinary_result(&response);
    assert_store_is_empty(&store).await;

    // And the tool's own fabricated id is not a usable handle either.
    let probe = declaring(
        addr,
        "tasks/get",
        "tool-fabricated",
        2,
        json!({ "taskId": "tool-fabricated" }),
    )
    .await;
    teardown(handle, ()).await;

    assert!(
        probe.body.get("error").is_some(),
        "the tool-fabricated id must not resolve to a task: {}",
        probe.raw
    );
}

/// The v1 `task` field is INERT on v2 — it does not substitute for the
/// declaration.
///
/// The field does not exist in the v2 extension at all, so carrying it must buy
/// the caller exactly nothing. The declaring twin at the end is the non-vacuity
/// guard: it proves the request is otherwise creatable, so the refusal above is
/// attributable to the missing declaration rather than to some unrelated
/// malformation of the body.
#[tokio::test]
async fn a_v2_client_sending_the_v1_task_field_still_needs_the_declaration() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    let with_v1_field = non_declaring(
        addr,
        "tools/call",
        TASKS_TOOL_NAME,
        1,
        call_params(TASKS_TOOL_NAME, true),
    )
    .await;
    assert_ordinary_result(&with_v1_field);
    assert_store_is_empty(&store).await;

    // Non-vacuity: the SAME body, plus the declaration, DOES create.
    let declared = declaring(
        addr,
        "tools/call",
        TASKS_TOOL_NAME,
        2,
        call_params(TASKS_TOOL_NAME, true),
    )
    .await;
    teardown(handle, ()).await;

    assert!(
        declared.raw.contains("\"resultType\":\"task\""),
        "the declaration is what makes this body creatable: {}",
        declared.raw
    );
}

// ===========================================================================
// 4: v1 is unchanged, against a golden envelope literal.
// ===========================================================================

/// The FROZEN v1 create envelope on this fixture.
///
/// A diff here is a **v1 WIRE BREAK**, not a fixture that drifted.
///
/// `ttl` is `InMemoryTaskStore::new()`'s `StoreConfig::default().default_ttl`
/// (3 600 000 ms), not `null` — `tests/v1_tasks_golden.rs` spawns a store
/// configured with no default and therefore pins `null`. Same wire SHAPE,
/// different fixture, which is why this literal lives next to the fixture that
/// produces it.
const V1_CREATE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"task":{"taskId":"<TASK-ID>","status":"working","ttl":3600000,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000},"_meta":{"io.modelcontextprotocol/related-task":{"taskId":"<TASK-ID>"}}}}"#;

/// Replace the two dynamic tokens a golden literal cannot pin.
///
/// Lifted verbatim from `tests/v2_tasks_shapes.rs`, which normalizes the same
/// two tokens on the same fixture.
fn normalize_v1(raw: &str, task_id: &str) -> String {
    let mut normalized = raw.replace(task_id, "<TASK-ID>");
    while let Some(start) = normalized.find("20") {
        let candidate = &normalized[start..];
        if candidate.len() >= 20 && candidate.as_bytes()[4] == b'-' && candidate.contains('T') {
            let end = start + candidate.find('"').unwrap_or(0);
            if end > start {
                normalized.replace_range(start..end, "<TIMESTAMP>");
                continue;
            }
        }
        break;
    }
    normalized
}

/// Complete a REAL v1 handshake and return the headers a v1 caller must carry.
///
/// D-114-J: the shared harness spawns with `StreamableHttpServerConfig::default()`,
/// which is STATEFUL on purpose, so a v1 caller has to negotiate and then carry
/// `Mcp-Session-Id` — otherwise it is answered `-32600`, which looks like a tasks
/// bug and is not one.
async fn v1_session_headers(addr: SocketAddr) -> Vec<(String, String)> {
    // On `--no-default-features --features full-v2` there is no session
    // machinery at all: `validate_non_init_session` is the null twin, so a v1
    // caller needs no handshake and carries no `Mcp-Session-Id`. Returning the
    // auth headers alone keeps every caller below RUNNING on the severed build,
    // instead of this whole file being gated away for one handshake.
    if !cfg!(feature = "v1-compat") {
        return auth_header();
    }
    let initialized = post(
        addr,
        &auth_header(),
        &v1_body(
            "initialize",
            json!(0),
            json!({
                "protocolVersion": common::v2::V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "0.0.0" }
            }),
        ),
    )
    .await;
    let session = initialized.mcp_session_id.unwrap_or_else(|| {
        panic!(
            "a stateful v1 handshake must mint a session id: {}",
            initialized.raw
        )
    });
    let mut headers = auth_header();
    headers.push((
        pmcp::shared::http_constants::MCP_SESSION_ID.to_string(),
        session,
    ));
    headers
}

/// v1 creation is UNCHANGED: the `task` field still triggers, and the envelope is
/// byte-for-byte the frozen nested `CreateTaskResult`.
///
/// The non-creating leg — a v1 call with NO `task` field — fires first, so the
/// v1 arm is proven to still REQUIRE the field rather than to have quietly
/// become unconditional.
#[tokio::test]
async fn a_v1_client_still_triggers_with_the_task_field() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let headers = v1_session_headers(addr).await;

    let without_field = post(
        addr,
        &headers,
        &v1_body("tools/call", json!(9), call_params(TASKS_TOOL_NAME, false)),
    )
    .await;
    assert!(
        without_field.body["result"].get("task").is_none(),
        "v1 without a `task` field must still fall through: {}",
        without_field.raw
    );
    assert_store_is_empty(&store).await;

    let created = post(
        addr,
        &headers,
        &v1_body("tools/call", json!(1), call_params(TASKS_TOOL_NAME, true)),
    )
    .await;
    teardown(handle, ()).await;

    let task_id = created.body["result"]["task"]["taskId"]
        .as_str()
        .unwrap_or_else(|| panic!("a v1 create envelope nests under `task`: {}", created.raw))
        .to_string();
    let body = serde_json::to_string(&created.body).expect("the body re-serializes");
    assert_eq!(
        normalize_v1(&body, &task_id),
        V1_CREATE,
        "the v1 create wire moved. raw: {}",
        created.raw
    );
    // The v2 spellings must be nowhere on this wire.
    for v2_only in ["ttlMs", "pollIntervalMs", "resultType"] {
        assert!(
            !created.raw.contains(v2_only),
            "`{v2_only}` leaked onto the v1 wire: {}",
            created.raw
        );
    }
}

// ===========================================================================
// 5 / 6: the OTHER two halves of the gate still close, with no error leak.
// ===========================================================================

/// The name of the tool registered by [`spawn_gate_probe_server`].
const PROBE_TOOL_NAME: &str = "probe";

/// A tool that returns a TASK-SHAPED value, with a caller-chosen `TaskSupport`.
///
/// Task-shaped on purpose: it makes the value half of the gate satisfied, so the
/// ONLY thing that can close the gate in tests 5 and 6 is the row under test.
fn probe_tool(task_support: Option<TaskSupport>) -> impl pmcp::ToolHandler {
    let tool = pmcp::server::typed_tool::TypedTool::new_with_schema(
        PROBE_TOOL_NAME,
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
    .with_description("a probe tool that returns a task-shaped value");
    match task_support {
        Some(support) => tool.with_execution(ToolExecution::new().with_task_support(support)),
        None => tool,
    }
}

/// Spawn a v2-opted-in server carrying ONE probe tool, with or without a store.
///
/// Built locally rather than through `spawn_tasks_server_with_store` because
/// both rows below need a server SHAPE the shared tasks fixture cannot have by
/// definition: a tool with no `taskSupport`, and a server with no `TaskStore`.
/// Everything else — the v2 accept-list, the auth posture, the spawn helper — is
/// the shared harness's.
async fn spawn_gate_probe_server(
    task_support: Option<TaskSupport>,
    with_store: bool,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let mut builder = Server::builder()
        .name("v2-create-gate-probe")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(common::v2::V1.to_string()),
            ProtocolVersion(common::v2::V2.to_string()),
        ])
        .tool(PROBE_TOOL_NAME, probe_tool(task_support))
        .auth_provider(OptionalBearer);
    if with_store {
        builder = builder.task_store(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>);
    }
    let server = builder.build().expect("probe server builds");
    spawn_default_config(server).await
}

/// A DECLARING client on a tool with NO task support gets an ordinary result.
///
/// The `TaskSupport::Forbidden`/absent rows of the gate still close on v2, and
/// no error leaks (T-102-11) — the existing `gate_rejects_when_task_support_forbidden`
/// unit behaviour, now observed over a socket.
#[tokio::test]
async fn a_declaring_client_on_a_tool_with_no_task_support_receives_an_ordinary_result() {
    for support in [None, Some(TaskSupport::Forbidden)] {
        let (addr, handle) = spawn_gate_probe_server(support, true).await;
        let response = declaring(
            addr,
            "tools/call",
            PROBE_TOOL_NAME,
            1,
            call_params(PROBE_TOOL_NAME, false),
        )
        .await;
        teardown(handle, ()).await;

        assert!(
            response.body.get("error").is_none(),
            "a closed gate must NOT leak an error (support={support:?}): {}",
            response.raw
        );
        assert_ordinary_result(&response);
    }
}

/// A DECLARING client on a server with NO store gets an ordinary result.
///
/// No backend, no handle, no error. `TaskSupport::Optional` rather than
/// `Required` because `apply_tasks_capability_rule` REFUSES to build a server
/// whose tool declares `Required` with no backend at all — that configuration is
/// rejected at build time, so the reachable no-backend row is the optional one.
#[tokio::test]
async fn a_declaring_client_on_a_server_with_no_store_receives_an_ordinary_result() {
    let (addr, handle) = spawn_gate_probe_server(Some(TaskSupport::Optional), false).await;
    let response = declaring(
        addr,
        "tools/call",
        PROBE_TOOL_NAME,
        1,
        call_params(PROBE_TOOL_NAME, false),
    )
    .await;
    teardown(handle, ()).await;

    assert!(
        response.body.get("error").is_none(),
        "a backendless server must answer the tool call normally, not with an error: {}",
        response.raw
    );
    assert_ordinary_result(&response);
}
