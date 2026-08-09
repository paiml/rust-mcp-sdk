//! Phase 114-08 (TASK-03): **the per-method era matrix for `tasks/list` and
//! `tasks/result`, driven over a REAL loopback socket in BOTH directions.**
//!
//! Both methods are ABSENT from the vendored tasks-extension schema
//! (`schema/vendored/ext-tasks/schema.ts`, pinned by plan 114-01, which declares
//! only `tasks/get`, `tasks/update` and `tasks/cancel`). On protocol version
//! 2026-07-28 they therefore answer `-32601`; on 2025-11-25 they serve exactly as
//! they always have.
//!
//! | # | test | proves |
//! |---|------|--------|
//! | 1 | `v1_tasks_list_still_serves` | v1 enumeration is untouched |
//! | 2 | `v2_tasks_list_is_gated` | v2 refuses AND enumerates nothing |
//! | 3 | `v1_tasks_result_still_serves_pending_minus_32002` | the FROZEN `-32002` still fires on v1 |
//! | 4 | `v2_tasks_result_is_gated` | v2 refuses AND emits no `-32002` anywhere |
//! | 5 | `v2_tasks_list_on_a_backendless_server_says_not_supported` | RETIRED ≠ NO-BACKEND |
//! | 6 | `v2_tasks_result_on_a_backendless_server_says_not_supported` | the same, for the other method |
//! | 7 | `v2_tasks_get_and_cancel_are_not_gated` | the scope fence: the two survivors still route |
//! | 8 | `the_minus_32601_conditions_are_mutually_distinct` | all four refusals are different strings |
//!
//! # Assertion discipline
//!
//! * **Both directions of every gate.** A gate tested only in the refusing
//!   direction cannot distinguish "gated" from "broken" — the shape 113-31
//!   caught as insufficient. The v1 (serving) case fires FIRST in each pair, and
//!   the paired v2 test re-establishes its own non-vacuity control on the SAME
//!   live server before probing.
//! * **Absence of enumeration, not merely presence of an error.** The whole
//!   point of removing `tasks/list` is that a v2 caller cannot learn a task
//!   exists, so test 2 asserts the response text carries neither the `tasks`
//!   array key nor the task id — against a server that demonstrably HAS that
//!   task.
//! * **The refusal text comes from the shipped constant**
//!   ([`pmcp::testing::V2_TASKS_METHOD_RETIRED`]), never a hand-copied sentence.
//! * Every assertion carries a reason string naming the decision it protects.
//! * Teardown is drop-sockets → `abort()` → `await`, through the shared
//!   [`teardown`] helper (D-113-T).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, header, post, spawn_default_config, spawn_tasks_server, teardown, v1_body,
    v2_body_with_client_extensions, v2_headers, v2_headers_for, AuthPosture, Resp, TASKS_TOOL_NAME,
    V1,
};
use pmcp::testing::V2_TASKS_METHOD_RETIRED;
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::error_codes::{METHOD_NOT_FOUND, V1_TASK_PENDING};
use serde_json::json;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

/// The `Mcp-Session-Id` header name, spelled once.
const SESSION_HEADER: &str = "Mcp-Session-Id";

/// The FROZEN v1 pending-refusal message. Phase 101 pinned it; Phase 114 must
/// not move a byte of it.
const V1_PENDING_MESSAGE: &str = "task result not available: task not completed";

/// The no-backend refusal `tasks/get`, `tasks/list` and `tasks/cancel` share.
const NO_BACKEND_LIST_MESSAGE: &str = "Tasks not enabled";

/// The no-backend refusal `tasks/result` gives. Deliberately a DIFFERENT string
/// from [`NO_BACKEND_LIST_MESSAGE`] in the shipped code, and this suite asserts
/// that rather than assuming it.
const NO_BACKEND_RESULT_MESSAGE: &str = "tasks/result not supported";

// ===========================================================================
// Harness.
// ===========================================================================

/// Mint a v1 session against a STATEFUL server.
///
/// [`spawn_tasks_server`] uses `StreamableHttpServerConfig::default()`, which
/// keeps the session machinery alive (the shared harness explains why a
/// build-time `stateless()` config would invalidate a per-request era test). So
/// the v1 legs — and only the v1 legs — need a handshake first; v2 has no
/// `initialize` at all, which is HTTP-01.
async fn v1_session(addr: SocketAddr) -> String {
    v1_session_as(addr, &[]).await
}

/// [`v1_session`] carrying EXTRA headers (an `Authorization` bearer, in
/// practice).
///
/// The single implementation; `v1_session` is the no-extra-headers spelling of
/// it. One body rather than two so an authenticated leg and an anonymous leg
/// cannot drift apart in anything but the headers they send.
async fn v1_session_as(addr: SocketAddr, extra: &[(String, String)]) -> String {
    // A `--no-default-features --features full-v2` build mints nothing and
    // validates nothing: the transport spec's "an inbound Mcp-Session-Id is
    // IGNORED" is structural there. Carrying a placeholder id exercises exactly
    // that and keeps every v1 leg below RUNNING on the severed build.
    if !cfg!(feature = "v1-compat") {
        return "no-session-on-a-severed-build".to_string();
    }
    let init = post(
        addr,
        extra,
        &v1_body(
            "initialize",
            json!(1),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "era-gate-v1-client", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    init.mcp_session_id
        .unwrap_or_else(|| panic!("a v1 initialize mints a session; body was {}", init.raw))
}

/// POST a v1 request on `session`.
async fn v1_post(addr: SocketAddr, session: &str, body: &str) -> Resp {
    v1_post_as(addr, session, body, &[]).await
}

/// [`v1_post`] carrying EXTRA headers. The single implementation.
async fn v1_post_as(
    addr: SocketAddr,
    session: &str,
    body: &str,
    extra: &[(String, String)],
) -> Resp {
    let mut headers = vec![header(SESSION_HEADER, session)];
    headers.extend_from_slice(extra);
    post(addr, &headers, body).await
}

/// POST a v2 request: three required headers plus the `_meta` era signal, and a
/// `clientCapabilities` that DECLARES the tasks extension.
///
/// The declaration is deliberate. Plan 114-09 added case 3 of the ordered
/// refusal chain: a v2 caller that never declared
/// `io.modelcontextprotocol/tasks` is refused `-32021` before the identity table
/// is consulted. This suite measures the ERA gates, so every probe must clear
/// the declaration gate first or a `-32021` would masquerade as a retirement —
/// the same reason the unit-level `context_for` fixture declares.
async fn v2_post(addr: SocketAddr, method: &str, id: i64, params: serde_json::Value) -> Resp {
    post(
        addr,
        &v2_headers(method, ""),
        &v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]),
    )
    .await
}

/// POST a v2 `tasks/*` request carrying a `taskId`.
async fn v2_task_post(addr: SocketAddr, method: &str, id: i64, task_id: &str) -> Resp {
    v2_task_post_as(addr, method, id, task_id, &[]).await
}

/// [`v2_task_post`] carrying EXTRA headers. The single implementation.
///
/// Declares the tasks extension for the reason [`v2_post`] gives.
async fn v2_task_post_as(
    addr: SocketAddr,
    method: &str,
    id: i64,
    task_id: &str,
    extra: &[(String, String)],
) -> Resp {
    let params = json!({ "taskId": task_id });
    // `Mcp-Name` is derived from the BODY, exactly as a conformant client derives
    // it. Since Phase 118 D-18 the server cross-checks it against `params.taskId`
    // for every `tasks/*` method, so a hard-coded `""` here would be a genuine
    // `-32020` header/body disagreement and would mask the era gate this file
    // measures.
    let mut headers = v2_headers_for(method, &params);
    headers.extend_from_slice(extra);
    post(
        addr,
        &headers,
        &v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]),
    )
    .await
}

/// Create a genuinely PENDING task over v1 and return its store-minted id.
///
/// The harness's `long_task_tool` returns a task-shaped value with no nested
/// terminal result, so the created task stays `working` — which is what makes
/// the `-32002` pending row reachable at all.
async fn create_v1_task(addr: SocketAddr, session: &str) -> String {
    create_v1_task_as(addr, session, &[]).await
}

/// [`create_v1_task`] carrying EXTRA headers. The single implementation.
///
/// The headers matter: with an `Authorization` bearer the created task is owned
/// by that SUBJECT and is therefore reachable from either era; without one it
/// lands in v1's `"local"` bucket, which no v2 caller can address (TASK-05).
async fn create_v1_task_as(addr: SocketAddr, session: &str, extra: &[(String, String)]) -> String {
    let created = v1_post_as(
        addr,
        session,
        &v1_body(
            "tools/call",
            json!(10),
            json!({ "name": TASKS_TOOL_NAME, "arguments": {}, "task": {} }),
        ),
        extra,
    )
    .await;
    created.body["result"]["task"]["taskId"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "the v1 create envelope must carry a store-minted taskId; body was {}",
                created.raw
            )
        })
        .to_string()
}

/// The JSON-RPC error code on a response, if it is an error.
fn error_code(response: &Resp) -> Option<i64> {
    response.body["error"]["code"].as_i64()
}

/// The JSON-RPC error message on a response, if it is an error.
fn error_message(response: &Resp) -> Option<String> {
    response.body["error"]["message"]
        .as_str()
        .map(str::to_string)
}

/// Assert a response is the v2 RETIRED refusal for `method`.
fn assert_retired(response: &Resp, method: &str) {
    assert_eq!(
        error_code(response),
        Some(i64::from(METHOD_NOT_FOUND)),
        "a RETIRED v2 method answers -32601 — the method does not exist on this protocol \
         version, so method-not-found is the truthful code. body was {}",
        response.raw
    );
    let message = error_message(response)
        .unwrap_or_else(|| panic!("a refusal must carry a message; body was {}", response.raw));
    assert!(
        message.starts_with(method),
        "the refusal must name the method the caller asked for, so a caller multiplexing \
         several requests can attribute it: {message}"
    );
    assert!(
        message.contains(V2_TASKS_METHOD_RETIRED),
        "the refusal must say WHY — a -32601 message is the ONLY signal distinguishing \
         'retired' from 'this server has no tasks backend', and an untruthful one makes the \
         correct fix undiscoverable (T-114-33): {message}"
    );
}

/// Spawn a v2-opted-in server with NO task backend at all.
async fn spawn_backendless() -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(build_v2_server()).await
}

// ===========================================================================
// 1 + 2 — `tasks/list`: serves on v1, retired on v2.
// ===========================================================================

/// v1 `tasks/list` enumerates the caller's tasks exactly as it always has.
#[tokio::test]
async fn v1_tasks_list_still_serves() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let session = v1_session(addr).await;
    let task_id = create_v1_task(addr, &session).await;

    let listed = v1_post(addr, &session, &v1_body("tasks/list", json!(20), json!({}))).await;
    teardown(handle, ()).await;

    let tasks = listed.body["result"]["tasks"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "a v1 tasks/list serves a tasks array; body was {}",
                listed.raw
            )
        });
    assert!(
        tasks
            .iter()
            .any(|task| task["taskId"].as_str() == Some(task_id.as_str())),
        "the v1 enumeration must still contain the created task — this plan retires the method \
         on v2 ONLY, and a v1 regression here is the failure mode T-114-35 names: {}",
        listed.raw
    );
}

/// v2 `tasks/list` is `-32601` and enumerates NOTHING.
///
/// Its own non-vacuity control fires first: the same live server is shown to
/// hold a task that a v1 `tasks/list` DOES return, so the absence asserted after
/// the gate is attributable to the gate rather than to an empty store.
#[tokio::test]
async fn v2_tasks_list_is_gated() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let session = v1_session(addr).await;
    let task_id = create_v1_task(addr, &session).await;

    // Non-vacuity: without the gate this id WOULD appear in a list response.
    let v1_listed = v1_post(addr, &session, &v1_body("tasks/list", json!(21), json!({}))).await;
    assert!(
        v1_listed.raw.contains(&task_id),
        "the control leg must show the task id IS enumerable on this fixture, or the v2 \
         absence assertions below prove nothing: {}",
        v1_listed.raw
    );

    let gated = v2_post(addr, "tasks/list", 22, json!({})).await;
    teardown(handle, ()).await;

    assert_retired(&gated, "tasks/list");
    assert!(
        gated.body["result"].is_null(),
        "a refusal carries no result: {}",
        gated.raw
    );
    assert!(
        !gated.raw.contains(&task_id),
        "the v2 response must not carry a task id ANYWHERE. `tasks/list` was removed as a \
         SECURITY improvement — with no enumeration primitive a server cannot inadvertently \
         leak the existence of one caller's tasks to another (T-114-32) — so a partial-serve \
         regression must fail here, not merely a wrong error code: {}",
        gated.raw
    );
    assert!(
        !gated.raw.contains("\"tasks\""),
        "and it must not carry a `tasks` array key at all: {}",
        gated.raw
    );
}

// ===========================================================================
// 3 + 4 — `tasks/result`: serves on v1 (including -32002), retired on v2.
// ===========================================================================

/// v1 `tasks/result` on a pending task still emits the FROZEN `-32002` with its
/// exact message.
#[tokio::test]
async fn v1_tasks_result_still_serves_pending_minus_32002() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let session = v1_session(addr).await;
    let task_id = create_v1_task(addr, &session).await;

    let pending = v1_post(
        addr,
        &session,
        &v1_body("tasks/result", json!(30), json!({ "taskId": task_id })),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&pending),
        Some(i64::from(V1_TASK_PENDING)),
        "the v1 pending wire value is FROZEN at -32002. The -32002 -> -32602 rename in the \
         draft targets RESOURCE-not-found, not task-pending, and the ROADMAP states verbatim \
         that Phase 114 must not re-litigate it: {}",
        pending.raw
    );
    assert_eq!(
        error_message(&pending).as_deref(),
        Some(V1_PENDING_MESSAGE),
        "and its message string is frozen too: {}",
        pending.raw
    );
}

/// v2 `tasks/result` is `-32601` and the raw response carries `-32002` nowhere.
///
/// The v1 control fires first against the SAME live server, so the absence of
/// the prohibited code below is attributable to the gate and not to a fixture
/// that could never have produced it.
#[tokio::test]
async fn v2_tasks_result_is_gated() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let session = v1_session(addr).await;
    let task_id = create_v1_task(addr, &session).await;

    // Non-vacuity: this fixture DOES reach the -32002 branch on v1.
    let control = v1_post(
        addr,
        &session,
        &v1_body("tasks/result", json!(31), json!({ "taskId": task_id })),
    )
    .await;
    assert!(
        control.raw.contains("-32002"),
        "the control leg must reach the pending branch, or the v2 assertion below is vacuous: {}",
        control.raw
    );

    let gated = v2_task_post(addr, "tasks/result", 32, &task_id).await;
    teardown(handle, ()).await;

    assert_retired(&gated, "tasks/result");
    assert!(
        !gated.raw.contains("-32002"),
        "retiring `tasks/result` on v2 removes the LAST v2-reachable emission path for the \
         code protocol version 2026-07-28 MUST NOT emit (T-114-34); the raw response must not \
         contain it in ANY position, including inside a message: {}",
        gated.raw
    );
}

// ===========================================================================
// 5 + 6 — the third arm: no backend is NOT a retirement.
// ===========================================================================

/// A v2 server with NO task backend answers `-32601` with the NO-BACKEND
/// message, which is a DIFFERENT string from the RETIRED message.
///
/// This is what makes the two-message split observable rather than cosmetic:
/// the two conditions call for opposite fixes (configure a backend vs. stop
/// calling a method that no longer exists).
#[tokio::test]
async fn v2_tasks_list_on_a_backendless_server_says_not_supported() {
    let (addr, handle) = spawn_backendless().await;
    let refused = v2_post(addr, "tasks/list", 40, json!({})).await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&refused),
        Some(i64::from(METHOD_NOT_FOUND)),
        "a backend-less server still answers -32601: {}",
        refused.raw
    );
    let message = error_message(&refused).unwrap_or_default();
    assert_eq!(
        message, NO_BACKEND_LIST_MESSAGE,
        "the no-backend refusal is unchanged by this plan, on every era: {}",
        refused.raw
    );
    assert_ne!(
        message, V2_TASKS_METHOD_RETIRED,
        "NO-BACKEND and RETIRED must be different strings"
    );
    assert!(
        !message.contains(V2_TASKS_METHOD_RETIRED),
        "a server that simply has no tasks backend must NOT be told the method was retired — \
         that would send the operator to fix the wrong thing (T-114-33): {message}"
    );
}

/// The `tasks/result` twin of the row above.
#[tokio::test]
async fn v2_tasks_result_on_a_backendless_server_says_not_supported() {
    let (addr, handle) = spawn_backendless().await;
    let refused = v2_task_post(addr, "tasks/result", 41, "absent").await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&refused),
        Some(i64::from(METHOD_NOT_FOUND)),
        "a backend-less server still answers -32601: {}",
        refused.raw
    );
    let message = error_message(&refused).unwrap_or_default();
    assert_eq!(
        message, NO_BACKEND_RESULT_MESSAGE,
        "the no-backend refusal is unchanged by this plan, on every era: {}",
        refused.raw
    );
    assert!(
        !message.contains(V2_TASKS_METHOD_RETIRED),
        "and it must not claim a retirement either (T-114-33): {message}"
    );
    assert!(
        !refused.raw.contains("-32002"),
        "nor may the backend-less path emit the prohibited code on v2: {}",
        refused.raw
    );
}

// ===========================================================================
// 7 — the scope fence.
// ===========================================================================

/// The subject BOTH legs of the scope-fence test authenticate as.
///
/// The fence creates on v1 and reads on v2, so it can only find its own task if
/// the two eras agree on the owner. An AUTHENTICATED caller is the only identity
/// that satisfies that: plan 114-09 binds the OAuth subject on both eras, while
/// an UNAUTHENTICATED caller binds the v1 `"local"` bucket and the v2
/// `ANONYMOUS_PRINCIPAL` (`""`) bucket, which `GenericTaskStore::make_key`
/// prefixes into DISJOINT key spaces by design (TASK-05). Before 114-09 both
/// eras used `"local"`, so this test could reach its task anonymously; that it
/// no longer can is the owner binding working, not a regression, and pinning the
/// subject here keeps this test measuring the RETIREMENT fence it is named for
/// rather than the owner binding 114-15 measures.
const FENCE_SUBJECT: &str = "era-fence-subject";

/// The `Authorization` header both fence legs carry.
fn fence_bearer() -> (String, String) {
    header("authorization", &format!("Bearer {FENCE_SUBJECT}"))
}

/// `tasks/get` and `tasks/cancel` SURVIVE in the v2 extension schema, so neither
/// is gated.
///
/// Their v2 response SHAPE (the flat projection, the inlined result, the
/// not-found remap) is plan 114-11's; this test asserts ONLY that they still
/// route, which is the fence that keeps a future "tidy up the era gates" change
/// from widening the retirement to methods that were not retired.
#[tokio::test]
async fn v2_tasks_get_and_cancel_are_not_gated() {
    // `Optional`, not `None`: the fence needs a STABLE identity on both eras
    // (see [`FENCE_SUBJECT`]), and `OptionalBearer` is the posture that maps a
    // bearer onto a subject while still admitting unauthenticated callers.
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;
    let auth = [fence_bearer()];
    let session = v1_session_as(addr, &auth).await;
    let task_id = create_v1_task_as(addr, &session, &auth).await;

    let got = v2_task_post_as(addr, "tasks/get", 50, &task_id, &auth).await;
    let cancelled = v2_task_post_as(addr, "tasks/cancel", 51, &task_id, &auth).await;
    teardown(handle, ()).await;

    for (method, response) in [("tasks/get", &got), ("tasks/cancel", &cancelled)] {
        assert_ne!(
            error_code(response),
            Some(i64::from(METHOD_NOT_FOUND)),
            "{method} survives in the v2 extension schema and must NOT be retired: {}",
            response.raw
        );
        assert!(
            !response.raw.contains(V2_TASKS_METHOD_RETIRED),
            "{method} must not carry the retirement message: {}",
            response.raw
        );
        assert!(
            !response.body["result"].is_null(),
            "{method} serves a real result on v2 today; plan 114-11 owns its SHAPE, not \
             whether it serves at all: {}",
            response.raw
        );
    }
}

// ===========================================================================
// 8 — the conditions stay mutually distinguishable.
// ===========================================================================

/// Every `-32601` this phase can produce for a `tasks/*` method is a DIFFERENT
/// string, collected by re-driving the real paths rather than declared.
#[tokio::test]
async fn the_minus_32601_conditions_are_mutually_distinct() {
    let (backed_addr, backed_handle) = spawn_tasks_server(AuthPosture::None).await;
    let retired_list = error_message(&v2_post(backed_addr, "tasks/list", 60, json!({})).await);
    let retired_result =
        error_message(&v2_task_post(backed_addr, "tasks/result", 61, "absent").await);
    teardown(backed_handle, ()).await;

    let (bare_addr, bare_handle) = spawn_backendless().await;
    let no_backend_list = error_message(&v2_post(bare_addr, "tasks/list", 62, json!({})).await);
    let no_backend_result =
        error_message(&v2_task_post(bare_addr, "tasks/result", 63, "absent").await);
    teardown(bare_handle, ()).await;

    let observed = [
        ("retired tasks/list", retired_list),
        ("retired tasks/result", retired_result),
        ("no-backend tasks/list", no_backend_list),
        ("no-backend tasks/result", no_backend_result),
    ];
    for (name, message) in &observed {
        assert!(
            message.is_some(),
            "{name} did not produce a refusal message at all"
        );
    }
    for (i, (left_name, left)) in observed.iter().enumerate() {
        for (right_name, right) in observed.iter().skip(i + 1) {
            assert_ne!(
                left, right,
                "{left_name} and {right_name} answer with the SAME string, so a caller cannot \
                 tell which condition it hit — the distinguishability is the mitigation, not a \
                 nicety"
            );
        }
    }
}
