//! The four v2 task result SHAPES, asserted against the VENDORED schema
//! (Phase 114, plan 11 — TASK-04, `114-SPEC-RECHECK.md` rows 4-24 and 29).
//!
//! # Why these assert against the schema rather than a restated field list
//!
//! Every "which keys are required" assertion in this file reads
//! `schema/vendored/ext-tasks/schema.json` at compile time and pulls the
//! relevant `$defs.<Variant>.required` array. A re-vendoring at the D-18 gate
//! therefore MOVES these tests automatically, instead of leaving a hand-copied
//! list asserting yesterday's contract while the artifact next to it says
//! something else.
//!
//! # Why so many assertions are on RAW BYTES
//!
//! `serde_json` is built with `preserve_order`, so a `serde_json::Map` is an
//! `IndexMap` whose `PartialEq` is ORDER-INDEPENDENT — a structural comparison
//! cannot see a key reorder. More importantly, plan 10 measured that the
//! reserved-field registry can DELETE a required key silently: a parsed-struct
//! assertion cannot distinguish "the server never emitted it" from "the egress
//! removed it on the way out", and both look like `None`. For a plan about
//! renames and flattening, the byte shape IS the contract.
//!
//! # The eleven properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `v2_create_task_result_is_flat_and_carries_all_required_fields` | `CreateTaskResult` is `Result & Task`, flat, `resultType: "task"`, FIVE required fields |
//! | 2 | `v2_tasks_get_on_a_working_task_is_flat` | `GetTaskResult` is flat, `resultType: "complete"`, no `task` wrapper |
//! | 3 | `v2_tasks_get_inlines_result_on_completed` | `CompletedTask.required` includes `result` |
//! | 4 | `v2_tasks_get_inlines_error_on_failed` | `FailedTask.required` includes `error` |
//! | 5 | `v2_tasks_get_inlines_input_requests_on_input_required` | row 23 end to end, on RAW bytes |
//! | 6 | `v2_tasks_cancel_is_an_empty_ack` | `CancelTaskResult = Result`, no task body |
//! | 7 | `task_status_wire_strings_match_the_extension_schema` | SET EQUALITY of the five status strings |
//! | 8 | `terminal_status_discipline` | `isError` -> `completed`; JSON-RPC error -> `failed` + `error` |
//! | 9 | `v1_shapes_are_still_nested` | the same server, a v1 caller, byte-compared to a golden literal |
//! | 10 | `tasks_get_never_carries_result_type_task` | the disposition boundary, from the negative side |
//! | 11 | `only_the_tool_call_create_path_mints_result_type_task` | the same boundary, from the positive side |

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    post, spawn_tasks_server_with_store, teardown, v1_body, v2_body_with_client_extensions,
    v2_headers, AuthPosture, Resp, COMPLETING_TOOL_NAME, TASKS_TOOL_NAME,
};
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::mrtr::{InputRequest, InputRequests};
use pmcp::types::tasks::TaskStatus;
use pmcp::types::CallToolResult;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

// ===========================================================================
// The vendored schema — read at compile time, never restated.
// ===========================================================================

/// The vendored tasks-extension JSON Schema.
const EXT_TASKS_SCHEMA_JSON: &str = include_str!("../schema/vendored/ext-tasks/schema.json");

/// The `required` array of a `$defs` entry.
fn schema_required(def: &str) -> BTreeSet<String> {
    let schema: Value =
        serde_json::from_str(EXT_TASKS_SCHEMA_JSON).expect("vendored schema parses");
    schema["$defs"][def]["required"]
        .as_array()
        .unwrap_or_else(|| panic!("$defs.{def}.required must be an array"))
        .iter()
        .map(|v| {
            v.as_str()
                .expect("a required entry is a string")
                .to_string()
        })
        .collect()
}

/// Assert `result` carries every key of `$defs.<def>.required`.
fn assert_carries_schema_required(result: &Value, def: &str, raw: &str) {
    let object = result
        .as_object()
        .unwrap_or_else(|| panic!("the {def} result must be a JSON object, got {result}"));
    for key in schema_required(def) {
        assert!(
            object.contains_key(&key),
            "$defs.{def}.required names `{key}` but the response omitted it.\n\
             result: {result}\nraw: {raw}"
        );
    }
}

// ===========================================================================
// Driving the server.
// ===========================================================================

/// The principal every request in this suite authenticates as.
///
/// [`AuthPosture::Optional`] plus an explicit bearer means the v2 identity table
/// binds the owner to this subject, so the suite can reach the SAME store
/// records the server does without depending on the anonymous-principal bucket
/// (which is `pub(crate)` and, on a no-auth-provider server, shared).
const SUBJECT: &str = "alice";

fn auth_header() -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {SUBJECT}"))]
}

/// A v2 request: the three required headers, the bearer, and a
/// `clientCapabilities` that DECLARES the tasks extension.
async fn v2_post(addr: SocketAddr, method: &str, name: &str, id: i64, params: Value) -> Resp {
    let mut headers = v2_headers(method, name);
    headers.extend(auth_header());
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &headers, &body).await
}

/// A task-augmented v2 `tools/call`.
///
/// `task: {}` is v1's client-signals-task field, which is what the create gate
/// still reads at the time of writing — plan 114-12 owns the v2 TRIGGER. This
/// plan owns the SHAPE of a create that already fires, so the fixture uses the
/// trigger that exists.
async fn v2_create(addr: SocketAddr, tool: &str, id: i64) -> Resp {
    v2_post(
        addr,
        "tools/call",
        tool,
        id,
        json!({ "name": tool, "arguments": {}, "task": {} }),
    )
    .await
}

/// The store-minted task id from a FLAT v2 create response.
fn v2_minted_id(response: &Resp) -> String {
    response.body["result"]["taskId"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "a v2 create result carries a TOP-LEVEL taskId; got {}",
                response.raw
            )
        })
        .to_string()
}

/// The `result` object of a success response, or a panic naming the error.
fn result_of(response: &Resp) -> &Value {
    response.body.get("result").unwrap_or_else(|| {
        panic!("expected a success result, got {}", response.raw);
    })
}

/// A real `InputRequests` map built from the PRODUCTION type.
fn input_requests() -> InputRequests {
    let mut requests = InputRequests::new();
    requests.insert("roots".to_string(), InputRequest::ListRoots);
    requests
}

/// Drive a freshly created task into `input_required` by recording the
/// server-authored input requests the way a server would.
async fn pause_for_input(store: &Arc<InMemoryTaskStore>, task_id: &str) {
    store
        .record_input_requests(task_id, SUBJECT, input_requests())
        .await
        .expect("the server records its own input requests");
}

/// Drive a freshly created task into `failed` with a JSON-RPC error object.
async fn fail_with_protocol_error(store: &Arc<InMemoryTaskStore>, task_id: &str) -> Value {
    let error = json!({ "code": -32603, "message": "the worker died mid-flight" });
    store
        .set_error(task_id, SUBJECT, error.clone())
        .await
        .expect("the error is persisted");
    store
        .update_status(task_id, SUBJECT, TaskStatus::Failed, None)
        .await
        .expect("working -> failed is a legal transition");
    error
}

// ===========================================================================
// 1-6: the four v2 shapes.
// ===========================================================================

/// A v2 `CreateTaskResult` is `Result & Task` — FLAT, with `resultType: "task"`.
///
/// All FIVE required `Task` fields are asserted INDIVIDUALLY, because TASK-04's
/// own requirement text enumerates only four and omits `ttlMs`. `ttlMs` is
/// asserted as KEY PRESENCE rather than as a value: it is required AND nullable,
/// so `"ttlMs": null` is the conformant shape for an unlimited task and a
/// value-based check would reject it.
#[tokio::test]
async fn v2_create_task_result_is_flat_and_carries_all_required_fields() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let response = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    teardown(handle, ()).await;

    let result = result_of(&response);
    assert_carries_schema_required(result, "Task", &response.raw);

    // The five, named one by one.
    assert!(
        result["taskId"].is_string(),
        "taskId must be a TOP-LEVEL string: {}",
        response.raw
    );
    assert_eq!(result["status"], json!("working"), "{}", response.raw);
    assert!(result["createdAt"].is_string(), "{}", response.raw);
    assert!(result["lastUpdatedAt"].is_string(), "{}", response.raw);
    assert!(
        result.as_object().expect("object").contains_key("ttlMs"),
        "ttlMs is REQUIRED and NULLABLE — the key must be present even when null: {}",
        response.raw
    );

    // Flat, renamed, and carrying the create discriminator.
    assert!(
        result.get("task").is_none(),
        "v2 must NOT wrap the task: {}",
        response.raw
    );
    assert_eq!(result["resultType"], json!("task"), "{}", response.raw);
    assert!(
        !response.raw.contains("\"ttl\":") && !response.raw.contains("\"pollInterval\":"),
        "a v1 key spelling leaked onto the v2 create wire: {}",
        response.raw
    );
}

/// A v2 `tasks/get` on a `working` task is the flat `WorkingTask` variant with
/// `resultType: "complete"`.
#[tokio::test]
async fn v2_tasks_get_on_a_working_task_is_flat() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    let response = v2_post(addr, "tasks/get", &task_id, 2, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    let result = result_of(&response);
    assert_carries_schema_required(result, "WorkingTask", &response.raw);
    assert_eq!(result["taskId"], json!(task_id), "{}", response.raw);
    assert_eq!(result["status"], json!("working"), "{}", response.raw);
    assert!(
        result.get("task").is_none(),
        "v2 tasks/get must NOT wrap under `task`: {}",
        response.raw
    );
    assert_eq!(result["resultType"], json!("complete"), "{}", response.raw);
    assert!(
        response.raw.contains("\"ttlMs\""),
        "the renamed key must be on the wire: {}",
        response.raw
    );
    assert!(
        !response.raw.contains("\"ttl\":"),
        "the v1 spelling leaked: {}",
        response.raw
    );
}

/// A v2 `tasks/get` on a `completed` task inlines the terminal `result`.
#[tokio::test]
async fn v2_tasks_get_inlines_result_on_completed() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    store
        .set_result(
            &task_id,
            SUBJECT,
            CallToolResult::new(vec![pmcp::types::Content::Text {
                text: "the answer is 42".to_string(),
            }]),
        )
        .await
        .expect("the result is persisted");
    store
        .update_status(&task_id, SUBJECT, TaskStatus::Completed, None)
        .await
        .expect("working -> completed is legal");

    let response = v2_post(addr, "tasks/get", &task_id, 2, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    let result = result_of(&response);
    assert_carries_schema_required(result, "CompletedTask", &response.raw);
    assert_eq!(result["status"], json!("completed"), "{}", response.raw);
    assert!(
        result["result"].is_object(),
        "the terminal result must be inlined as an OBJECT: {}",
        response.raw
    );
    assert!(
        response.raw.contains("the answer is 42"),
        "the inlined result must be the persisted one: {}",
        response.raw
    );
}

/// A v2 `tasks/get` on a `failed` task inlines the JSON-RPC `error`.
#[tokio::test]
async fn v2_tasks_get_inlines_error_on_failed() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    let expected = fail_with_protocol_error(&store, &task_id).await;

    let response = v2_post(addr, "tasks/get", &task_id, 2, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    let result = result_of(&response);
    assert_carries_schema_required(result, "FailedTask", &response.raw);
    assert_eq!(result["status"], json!("failed"), "{}", response.raw);
    assert_eq!(result["error"], expected, "{}", response.raw);
}

/// A v2 `tasks/get` on an `input_required` task inlines a **TOP-LEVEL**
/// `inputRequests` — row 23, end to end.
///
/// This is the regression test for the defect plan 10 fixed at the registry:
/// under the old derived ownership flag the required key was DELETED from this
/// exact response, silently, with a `tracing::warn!` rather than an error. The
/// assertion is therefore on the RAW RESPONSE BYTES. A parsed-struct check
/// cannot see the difference between "the projection never emitted it" and "the
/// egress removed it on the way out" — both are an absent key.
#[tokio::test]
async fn v2_tasks_get_inlines_input_requests_on_input_required() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    pause_for_input(&store, &task_id).await;

    let response = v2_post(addr, "tasks/get", &task_id, 2, json!({ "taskId": task_id })).await;
    teardown(handle, ()).await;

    assert!(
        response.raw.contains("\"inputRequests\""),
        "the required key must survive egress to the WIRE: {}",
        response.raw
    );
    assert!(
        response.raw.contains("roots/list"),
        "the inlined map must be the server-recorded one: {}",
        response.raw
    );
    let result = result_of(&response);
    assert_carries_schema_required(result, "InputRequiredTask", &response.raw);
    assert_eq!(
        result["status"],
        json!("input_required"),
        "{}",
        response.raw
    );
    assert!(
        result.get("task").is_none(),
        "inputRequests is TOP-LEVEL, not nested under `task`: {}",
        response.raw
    );
}

/// A v2 `tasks/cancel` is an EMPTY acknowledgement.
///
/// `CancelTaskResult = Result` — no task body at all (inventory row 20). The
/// only keys the response may carry are the ones the v2 envelope itself owns
/// (`resultType`, `_meta`), which is what the key-set assertion below states.
///
/// The empty ack is the SEMANTICS, not a simplification: cancellation is
/// cooperative and eventually consistent, so the task MAY still be `working`
/// when this returns and MAY settle on a terminal status other than `cancelled`.
#[tokio::test]
async fn v2_tasks_cancel_is_an_empty_ack() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    let response = v2_post(
        addr,
        "tasks/cancel",
        &task_id,
        2,
        json!({ "taskId": task_id }),
    )
    .await;
    teardown(handle, ()).await;

    let result = result_of(&response);
    assert!(
        result.get("task").is_none(),
        "a v2 cancel ack carries no task wrapper: {}",
        response.raw
    );
    for key in ["taskId", "status", "createdAt", "lastUpdatedAt", "ttlMs"] {
        assert!(
            result.get(key).is_none(),
            "a v2 cancel ack carries no task field, but `{key}` was present: {}",
            response.raw
        );
    }
    let keys: BTreeSet<&str> = result
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    let envelope_only: BTreeSet<&str> = ["resultType", "_meta"].into_iter().collect();
    assert!(
        keys.is_subset(&envelope_only),
        "a v2 cancel ack may carry ONLY the envelope's own keys, got {keys:?}: {}",
        response.raw
    );
}

// ===========================================================================
// 7-8: the status locks.
// ===========================================================================

/// The five `TaskStatus` serde strings and the vendored schema's `TaskStatus`
/// union are the SAME SET — the TASK-04 "deterministic mapping" lock.
///
/// Research measured that the v1 five-state enum is already NAME-IDENTICAL to
/// the v2 status union, so what TASK-04 asks for is satisfied by a locking
/// tripwire rather than a conversion table. Building a table where none is
/// needed would be a second place for the mapping to drift.
///
/// The comparison is SET EQUALITY in both directions, never a subset: a subset
/// assertion passes when a sixth state is added on either side, which is the
/// exact drift this test exists to catch. The Rust side's own exhaustiveness is
/// enforced in-crate by the wildcard-free matches in
/// `TaskDetailV2::status()` and `Task::poll_decision()`, so a sixth `TaskStatus`
/// variant fails to compile before it can reach this assertion.
#[test]
fn task_status_wire_strings_match_the_extension_schema() {
    let from_code: BTreeSet<String> = [
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ]
    .into_iter()
    .map(|status| {
        serde_json::to_value(status)
            .expect("a TaskStatus serializes")
            .as_str()
            .expect("as a string")
            .to_string()
    })
    .collect();

    let schema: Value =
        serde_json::from_str(EXT_TASKS_SCHEMA_JSON).expect("vendored schema parses");
    let from_schema: BTreeSet<String> = schema["$defs"]["TaskStatus"]["anyOf"]
        .as_array()
        .expect("TaskStatus is an anyOf union")
        .iter()
        .map(|member| {
            member["const"]
                .as_str()
                .expect("each union member is a string const")
                .to_string()
        })
        .collect();

    assert_eq!(
        from_code, from_schema,
        "the code's status set and the vendored schema's union must be EQUAL, not merely \
         overlapping"
    );
    assert_eq!(
        from_code.len(),
        5,
        "the union is five states: {from_code:?}"
    );
    // The two spellings a re-verifier must read character by character.
    assert!(from_code.contains("input_required"), "{from_code:?}");
    assert!(from_code.contains("cancelled"), "{from_code:?}");
}

/// `failed` MUST NOT represent a tool result that completed with
/// `isError: true`; BOTH directions are asserted.
///
/// * A tool that RAN and reported a failure to its caller is `completed`, with
///   the error detail inside `result`.
/// * A JSON-RPC protocol error during execution is `failed`, and a `failed` task
///   MUST carry `error`.
///
/// Both directions are required because the two are indistinguishable from a
/// "the tool failed" mindset and opposite on the wire: a one-directional test
/// would pass against an implementation that mapped every failure onto one
/// status.
#[tokio::test]
async fn terminal_status_discipline() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    // --- direction A: isError: true -> completed, error detail inside result.
    let created = v2_create(addr, COMPLETING_TOOL_NAME, 1).await;
    let error_task = v2_minted_id(&created);
    let a = v2_post(
        addr,
        "tasks/get",
        &error_task,
        2,
        json!({ "taskId": error_task }),
    )
    .await;

    // --- direction B: a JSON-RPC error -> failed, and failed carries `error`.
    let created = v2_create(addr, TASKS_TOOL_NAME, 3).await;
    let failed_task = v2_minted_id(&created);
    fail_with_protocol_error(&store, &failed_task).await;
    let b = v2_post(
        addr,
        "tasks/get",
        &failed_task,
        4,
        json!({ "taskId": failed_task }),
    )
    .await;
    teardown(handle, ()).await;

    let a_result = result_of(&a);
    assert_eq!(
        a_result["status"],
        json!("completed"),
        "a tool outcome carrying isError: true is COMPLETED, never failed: {}",
        a.raw
    );
    assert_eq!(
        a_result["result"]["isError"],
        json!(true),
        "the error detail lives INSIDE result: {}",
        a.raw
    );
    assert!(
        a_result.get("error").is_none(),
        "a completed task carries no top-level error: {}",
        a.raw
    );

    let b_result = result_of(&b);
    assert_eq!(
        b_result["status"],
        json!("failed"),
        "a JSON-RPC error during execution is FAILED: {}",
        b.raw
    );
    assert!(
        b_result["error"].is_object(),
        "a failed task MUST carry `error`: {}",
        b.raw
    );
    assert!(
        b_result.get("result").is_none(),
        "a failed task carries no terminal result: {}",
        b.raw
    );
}

// ===========================================================================
// 9: the v1 freeze, on the SAME server.
// ===========================================================================

/// Complete a REAL v1 handshake and return the headers a v1 caller must then
/// send.
///
/// The shared harness spawns with `StreamableHttpServerConfig::default()`, which
/// is STATEFUL on purpose (RESEARCH Pitfall 1: the build-time `stateless()`
/// config removes the session machinery before a request is ever seen). So a v1
/// caller against this fixture has to do what a real v1 client does — negotiate,
/// then carry `Mcp-Session-Id` — rather than being handed a server that never
/// asks. That is also what makes this test's claim the strong one: the v1 freeze
/// holds on a server SIMULTANEOUSLY serving the v2 shapes, which is the only
/// configuration a real deployment has.
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

/// The FROZEN v1 `tasks/get` frame for a `working` task on this fixture.
///
/// A diff here is a **v1 WIRE BREAK**, not a fixture that drifted.
///
/// `ttl` is the store's `StoreConfig::default().default_ttl` (3 600 000 ms), NOT
/// `null`: `tests/v1_tasks_golden.rs` spawns a store configured with no default,
/// this fixture uses `InMemoryTaskStore::new()`. Same wire SHAPE, different
/// fixture — which is why this literal lives next to the fixture that produces
/// it rather than being shared with that suite.
const V1_GET_WORKING: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"<TASK-ID>","status":"working","ttl":3600000,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000}}}"#;

/// The dynamic tokens a v1 golden literal cannot pin.
fn normalize_v1(raw: &str, task_id: &str) -> String {
    let mut normalized = raw.replace(task_id, "<TASK-ID>");
    // Two ISO 8601 timestamps of the shape the store emits.
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

/// The SAME server, a v1 caller: still nested, still `ttl` and `pollInterval`.
///
/// A diff in the literal below is a **v1 WIRE BREAK**, not a fixture that
/// drifted — the same rule `tests/v1_tasks_golden.rs` states. It is repeated
/// here because that suite spawns its OWN server: this one proves the freeze
/// holds on a server that is simultaneously serving the v2 shapes, which is the
/// only configuration a real deployment has.
#[tokio::test]
async fn v1_shapes_are_still_nested() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let headers = v1_session_headers(addr).await;
    let created = post(
        addr,
        &headers,
        &v1_body(
            "tools/call",
            json!(1),
            json!({ "name": TASKS_TOOL_NAME, "arguments": {}, "task": {} }),
        ),
    )
    .await;
    let task_id = created.body["result"]["task"]["taskId"]
        .as_str()
        .unwrap_or_else(|| panic!("a v1 create envelope nests under `task`: {}", created.raw))
        .to_string();
    let response = post(
        addr,
        &headers,
        &v1_body("tasks/get", json!(2), json!({ "taskId": task_id })),
    )
    .await;
    teardown(handle, ()).await;

    let body = serde_json::to_string(&response.body).expect("the body re-serializes");
    assert_eq!(
        normalize_v1(&body, &task_id),
        V1_GET_WORKING,
        "the v1 tasks/get wire moved. raw: {}",
        response.raw
    );
    // The v2 spellings must be nowhere on this wire.
    for v2_only in ["ttlMs", "pollIntervalMs", "resultType"] {
        assert!(
            !response.raw.contains(v2_only),
            "`{v2_only}` leaked onto the v1 wire: {}",
            response.raw
        );
    }
}

// ===========================================================================
// 10-11: the `resultType: "task"` disposition boundary, both directions.
// ===========================================================================

/// `resultType: "task"` NEVER appears on `tasks/get`, `tasks/cancel` or
/// `tasks/update`.
///
/// The discriminator is a TOOL-CALL disposition: it means "this tool call
/// returned a handle instead of a result". A `tasks/get` is an ordinary complete
/// result ABOUT a task, so it carries `"complete"` even though its body is full
/// of task fields — and the `completed` row is the most tempting to mislabel,
/// because that one inlines a `result`.
///
/// Asserted on RAW BYTES so a re-ordered or re-nested `resultType` cannot slip
/// past a structural lookup.
///
/// `tasks/update` is included even though it answers `-32601` at the time of
/// writing (plan 114-13 lands the method): the assertion holds for an error
/// response too, and it becomes a live tripwire the moment the method exists.
#[tokio::test]
async fn tasks_get_never_carries_result_type_task() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    let working = v2_minted_id(&v2_create(addr, TASKS_TOOL_NAME, 1).await);
    let paused = v2_minted_id(&v2_create(addr, TASKS_TOOL_NAME, 2).await);
    pause_for_input(&store, &paused).await;
    let done = v2_minted_id(&v2_create(addr, COMPLETING_TOOL_NAME, 3).await);
    let cancelling = v2_minted_id(&v2_create(addr, TASKS_TOOL_NAME, 4).await);

    let mut responses = Vec::new();
    for (index, task_id) in [&working, &paused, &done].into_iter().enumerate() {
        responses.push((
            format!("tasks/get({task_id})"),
            #[allow(clippy::cast_possible_wrap)]
            v2_post(
                addr,
                "tasks/get",
                task_id,
                10 + index as i64,
                json!({ "taskId": task_id }),
            )
            .await,
        ));
    }
    responses.push((
        "tasks/cancel".to_string(),
        v2_post(
            addr,
            "tasks/cancel",
            &cancelling,
            20,
            json!({ "taskId": cancelling }),
        )
        .await,
    ));
    responses.push((
        "tasks/update".to_string(),
        v2_post(
            addr,
            "tasks/update",
            &paused,
            21,
            json!({ "taskId": paused, "inputResponses": {} }),
        )
        .await,
    ));
    teardown(handle, ()).await;

    for (label, response) in &responses {
        assert!(
            !response.raw.contains("\"resultType\":\"task\""),
            "{label} must never carry the tool-call disposition: {}",
            response.raw
        );
    }
    // Non-vacuity: the three `tasks/get` rows really were served, and really did
    // carry the OTHER discriminator. Without this, a suite whose requests all
    // failed would pass the assertion above for the wrong reason.
    for (label, response) in responses.iter().take(3) {
        assert!(
            response.raw.contains("\"resultType\":\"complete\""),
            "{label} must carry the complete disposition: {}",
            response.raw
        );
    }
    // The three statuses really were distinct — `completed` is the row most
    // tempting to mislabel, so it must actually have been reached.
    let statuses: Vec<&str> = responses
        .iter()
        .take(3)
        .map(|(_, r)| r.body["result"]["status"].as_str().unwrap_or("<none>"))
        .collect();
    assert_eq!(
        statuses,
        vec!["working", "input_required", "completed"],
        "the three tasks/get rows must cover three DIFFERENT statuses"
    );
}

/// EXACTLY ONE response in this suite carries `resultType: "task"`, and it is
/// the `tools/call` create response.
///
/// The positive counterpart of the test above. Together they pin the boundary
/// from both sides: one proves the discriminator is absent everywhere it must
/// be, the other proves it is not absent EVERYWHERE — which a server that simply
/// never emitted it would also satisfy.
#[tokio::test]
async fn only_the_tool_call_create_path_mints_result_type_task() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;

    let mut responses: Vec<(&str, Resp)> = Vec::new();
    let created = v2_create(addr, TASKS_TOOL_NAME, 1).await;
    let task_id = v2_minted_id(&created);
    responses.push(("tools/call (create)", created));
    pause_for_input(&store, &task_id).await;
    responses.push((
        "tasks/get",
        v2_post(addr, "tasks/get", &task_id, 2, json!({ "taskId": task_id })).await,
    ));
    responses.push((
        "tasks/cancel",
        v2_post(
            addr,
            "tasks/cancel",
            &task_id,
            3,
            json!({ "taskId": task_id }),
        )
        .await,
    ));
    responses.push((
        "tools/list",
        v2_post(addr, "tools/list", "", 4, json!({})).await,
    ));
    teardown(handle, ()).await;

    let minting: Vec<&str> = responses
        .iter()
        .filter(|(_, r)| r.raw.contains("\"resultType\":\"task\""))
        .map(|(label, _)| *label)
        .collect();
    assert_eq!(
        minting,
        vec!["tools/call (create)"],
        "exactly one response may mint the task disposition; the full set was {:#?}",
        responses
            .iter()
            .map(|(label, r)| (*label, r.raw.clone()))
            .collect::<Vec<_>>()
    );
}

// ===========================================================================
// The not-found code (inventory row 29), over the real socket.
// ===========================================================================

/// On v2, an unknown task id answers `-32602` with a message that is neither an
/// existence oracle nor an id echo.
///
/// The absent / wrong-owner pair is compared for EQUALITY: owner mismatch
/// surfaces as `NotFound` deliberately, so a message that varied between the two
/// would make the sharper `-32602` code an existence oracle.
#[tokio::test]
async fn v2_task_not_found_is_invalid_params_and_not_an_oracle() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    // A task that exists — but under a DIFFERENT owner.
    let mut headers = v2_headers("tools/call", TASKS_TOOL_NAME);
    headers.push(("authorization".to_string(), "Bearer bob".to_string()));
    let bobs = post(
        addr,
        &headers,
        &v2_body_with_client_extensions(
            "tools/call",
            json!(1),
            json!({ "name": TASKS_TOOL_NAME, "arguments": {}, "task": {} }),
            &[TASKS_EXTENSION_KEY],
        ),
    )
    .await;
    let bobs_task = v2_minted_id(&bobs);

    // Alice asks for Bob's task, and for a task nobody has.
    let wrong_owner = v2_post(
        addr,
        "tasks/get",
        &bobs_task,
        2,
        json!({ "taskId": bobs_task }),
    )
    .await;
    let absent = v2_post(
        addr,
        "tasks/get",
        "no-such-task",
        3,
        json!({ "taskId": "no-such-task" }),
    )
    .await;
    teardown(handle, ()).await;

    for (label, response) in [("wrong owner", &wrong_owner), ("absent", &absent)] {
        assert_eq!(
            response.body["error"]["code"],
            json!(-32602),
            "{label} must answer -32602 on v2: {}",
            response.raw
        );
    }
    assert_eq!(
        wrong_owner.body["error"]["message"], absent.body["error"]["message"],
        "the two refusals must be INDISTINGUISHABLE, or -32602 becomes an existence oracle.\n\
         wrong owner: {}\nabsent: {}",
        wrong_owner.raw, absent.raw
    );
    let message = wrong_owner.body["error"]["message"]
        .as_str()
        .expect("a message string");
    assert!(
        !message.contains(&bobs_task) && !message.contains("no-such-task"),
        "the refusal must not render the requested task id back: {message}"
    );
}

/// The v1 answer for the SAME condition is unchanged: `-32603`, not `-32602`.
#[tokio::test]
async fn v1_task_not_found_is_still_internal_error() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let headers = v1_session_headers(addr).await;
    let response = post(
        addr,
        &headers,
        &v1_body("tasks/get", json!(1), json!({ "taskId": "no-such-task" })),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        response.body["error"]["code"],
        json!(-32603),
        "v1 keeps its frozen internal-error answer: {}",
        response.raw
    );
}
