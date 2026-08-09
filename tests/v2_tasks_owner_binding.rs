//! Phase 114-09 (TASK-05): **the v2 task-owner binding and the ordered refusal
//! chain, driven over a REAL loopback socket.**
//!
//! v2 is session-free by design, so a task owner can only come from the
//! authenticated subject or not at all. `TaskDispatch::resolve_owner` implements
//! the three-row identity table Phase 113 minted for MRTR — the SAME function,
//! not a copy — and `route_tasks_endpoint` wraps it in a five-case refusal chain
//! whose ORDER is the contract:
//!
//! | case | condition | code |
//! |---|---|---|
//! | 1 | the method is RETIRED on this era | `-32601` |
//! | 2 | the server has no task backend | `-32601` |
//! | 3 | the client never declared the tasks extension | `-32021` |
//! | 4 | unauthenticated on a server that HAS an auth provider | `-32003` |
//! | 5 | the params, finally | — |
//!
//! | # | test | proves |
//! |---|------|--------|
//! | 1 | `a_retired_method_answers_32601_even_when_unauthenticated` | case 1 beats case 4 |
//! | 2 | `a_backendless_server_answers_32601_even_when_unauthenticated` | case 2 beats case 4 |
//! | 3 | `a_non_declaring_client_gets_32021_not_32003` | case 3 beats case 4, and the payload is an OBJECT |
//! | 4 | `an_unauthenticated_caller_is_refused_before_the_params_parse` | case 4 beats case 5 |
//! | 5 | `the_refusal_echoes_the_original_id_at_http_200` | the refusal's wire shape |
//! | 6 | `a_no_auth_provider_server_still_serves_tasks` | row 3 — fail-closed is not "refuse everyone" |
//! | 7 | `an_authenticated_caller_binds_its_own_subject` | row 1, the non-vacuity control |
//! | 8 | `the_refusals_are_mutually_distinguishable` | a caller can tell which case it hit |
//!
//! # Assertion discipline
//!
//! * **The ORDERING assertions are the load-bearing ones.** A test that only
//!   checks "an unauthenticated caller is refused" passes just as well when the
//!   auth refusal sits FIRST and swallows the three more-truthful answers above
//!   it. Each of tests 1–4 therefore constructs a request that satisfies TWO
//!   refusal conditions at once and asserts which one wins.
//! * **`AuthPosture::Optional`, not `Required`.** `BearerSubjects` returns `Err`
//!   for a missing token, so the transport answers `401` long before dispatch
//!   and case 4 is never reached. `OptionalBearer` is the only posture that
//!   makes the `(no subject, has_auth_provider = true)` row observable at all —
//!   114-02 added it to the shared harness for exactly this.
//! * Every assertion carries a reason string naming the decision it protects.
//! * Teardown is through the shared [`teardown`] helper (D-113-T).
//!
//! The live cross-caller proof — two authenticated callers cannot see each
//! other's tasks — is plan 114-15. This suite proves the REFUSALS.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, header, post, spawn_default_config, spawn_tasks_server, teardown,
    v2_body_with_client_extensions, v2_headers_for, AuthPosture, Resp,
};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::error_codes::{
    AUTHENTICATION_REQUIRED, INVALID_PARAMS, METHOD_NOT_FOUND, MISSING_REQUIRED_CLIENT_CAPABILITY,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::task::JoinHandle;

// ===========================================================================
// Harness.
// ===========================================================================

/// The subject the authenticated legs present.
const SUBJECT: &str = "owner-binding-subject";

/// A server with a task backend and NO auth provider at all — row 3 of the
/// identity table.
async fn spawn_backendless() -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(build_v2_server()).await
}

/// The `Authorization` header for [`SUBJECT`].
fn bearer() -> Vec<(String, String)> {
    vec![header("authorization", &format!("Bearer {SUBJECT}"))]
}

/// POST a v2 `tasks/*` request DECLARING the tasks extension, with optional
/// credentials.
///
/// Declaring is the well-formed default here: every test but #3 wants to be past
/// case 3 so that whatever it observes is attributable to the case it names.
async fn tasks_post(
    addr: SocketAddr,
    method: &str,
    id: i64,
    params: Value,
    auth: &[(String, String)],
) -> Resp {
    // `Mcp-Name` is derived from the BODY, exactly as a conformant client derives
    // it. Since Phase 118 D-18 the server cross-checks it against `params.taskId`
    // for every `tasks/*` method, so a hard-coded `""` here would be a genuine
    // `-32020` header/body disagreement and would mask the gate this file measures.
    let mut headers = v2_headers_for(method, &params);
    headers.extend_from_slice(auth);
    post(
        addr,
        &headers,
        &v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]),
    )
    .await
}

/// POST a v2 `tasks/*` request that does NOT declare the tasks extension.
async fn undeclared_tasks_post(
    addr: SocketAddr,
    method: &str,
    id: i64,
    params: Value,
    auth: &[(String, String)],
) -> Resp {
    // `Mcp-Name` is derived from the BODY, exactly as a conformant client derives
    // it. Since Phase 118 D-18 the server cross-checks it against `params.taskId`
    // for every `tasks/*` method, so a hard-coded `""` here would be a genuine
    // `-32020` header/body disagreement and would mask the gate this file measures.
    let mut headers = v2_headers_for(method, &params);
    headers.extend_from_slice(auth);
    post(
        addr,
        &headers,
        &v2_body_with_client_extensions(method, json!(id), params, &[]),
    )
    .await
}

/// A well-formed `taskId` params object.
fn task_params(task_id: &str) -> Value {
    json!({ "taskId": task_id })
}

/// The JSON-RPC error code on a response, if it is an error.
fn error_code(response: &Resp) -> Option<i64> {
    response.body["error"]["code"].as_i64()
}

// ===========================================================================
// 1 — case 1 beats case 4.
// ===========================================================================

/// A RETIRED method answers "no such method" even to a caller who is also
/// unauthenticated.
///
/// The ordering matters for a reason beyond tidiness: if case 4 ran first, a v2
/// `tasks/list` would answer "authenticate yourself", which implies that
/// authenticating WOULD enumerate something. It would not — the method does not
/// exist on this era at all. Answering `-32601` first is what stops the auth
/// refusal from becoming an existence hint (T-114-32).
#[tokio::test]
async fn a_retired_method_answers_32601_even_when_unauthenticated() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    // No credentials AND a retired method: both case 1 and case 4 apply.
    let listed = tasks_post(addr, "tasks/list", 10, json!({}), &[]).await;
    let resulted = tasks_post(addr, "tasks/result", 11, task_params("absent"), &[]).await;
    teardown(handle, ()).await;

    for (method, response) in [("tasks/list", &listed), ("tasks/result", &resulted)] {
        assert_eq!(
            error_code(response),
            Some(i64::from(METHOD_NOT_FOUND)),
            "{method} is RETIRED on v2, so the retirement answers FIRST — an \
             unauthenticated caller must not be told to authenticate for a method that \
             does not exist: {}",
            response.raw
        );
        assert_ne!(
            error_code(response),
            Some(i64::from(AUTHENTICATION_REQUIRED)),
            "{method} must not answer the auth refusal: that would imply authenticating \
             makes the method reachable: {}",
            response.raw
        );
    }
}

// ===========================================================================
// 2 — case 2 beats case 4.
// ===========================================================================

/// A server with NO task backend answers "no such method", not "authenticate
/// yourself".
///
/// Such a server advertises no tasks extension at all, so telling the caller to
/// authenticate would send it to fix the wrong thing (T-114-33). This is also
/// the guard that keeps the owner binding from leaking a server-configuration
/// fact: a backendless server answers identically to every caller, authenticated
/// or not.
#[tokio::test]
async fn a_backendless_server_answers_32601_even_when_unauthenticated() {
    let (addr, handle) = spawn_backendless().await;

    let got = tasks_post(addr, "tasks/get", 20, task_params("absent"), &[]).await;
    let cancelled = tasks_post(addr, "tasks/cancel", 21, task_params("absent"), &[]).await;
    teardown(handle, ()).await;

    for (method, response) in [("tasks/get", &got), ("tasks/cancel", &cancelled)] {
        assert_eq!(
            error_code(response),
            Some(i64::from(METHOD_NOT_FOUND)),
            "{method} on a backendless server is a no-such-method, whatever the caller's \
             credentials: {}",
            response.raw
        );
        assert_ne!(
            error_code(response),
            Some(i64::from(AUTHENTICATION_REQUIRED)),
            "{method} must not answer the auth refusal on a server that serves no tasks \
             at all: {}",
            response.raw
        );
    }
}

// ===========================================================================
// 3 — case 3 beats case 4, and the payload is an OBJECT.
// ===========================================================================

/// A client that never declared the tasks extension is refused `-32021`, NOT the
/// auth code — even though it is also unauthenticated.
///
/// Two refusals, two codes, two meanings: on a tasks method a single `-32003`
/// would be ambiguous between "you did not declare the extension" and "you are
/// not authenticated", which is the exact undiscoverability the distinct codes
/// exist to avoid (DQ3). The `data` payload's SHAPE is asserted too: the
/// official conformance suite grades `requiredCapabilities` as a
/// `ClientCapabilities` object, and emitting an array is a wire-contract
/// violation (T-114-41).
#[tokio::test]
async fn a_non_declaring_client_gets_32021_not_32003() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let refused = undeclared_tasks_post(addr, "tasks/get", 30, task_params("absent"), &[]).await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&refused),
        Some(i64::from(MISSING_REQUIRED_CLIENT_CAPABILITY)),
        "an under-declaring client hears about its DECLARATION, not its credentials: {}",
        refused.raw
    );
    assert_ne!(
        error_code(&refused),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "collapsing case 3 onto the auth code would leave the caller unable to tell \
         which of the two things it must fix: {}",
        refused.raw
    );

    let required = &refused.body["error"]["data"]["requiredCapabilities"];
    assert!(
        required.is_object(),
        "requiredCapabilities is a ClientCapabilities OBJECT, which the official \
         conformance suite grades: {}",
        refused.raw
    );
    assert!(
        !required.is_array(),
        "emitting an ARRAY here is a wire-contract violation, not a stylistic choice: {}",
        refused.raw
    );
    assert_eq!(
        required["extensions"][TASKS_EXTENSION_KEY],
        json!({}),
        "the payload names the extension the caller must declare, with the spec-literal \
         empty-object value: {}",
        refused.raw
    );
    assert!(
        !refused.raw.contains("authenticat"),
        "a negotiation refusal must reveal NOTHING about authentication state \
         (T-114-40): {}",
        refused.raw
    );
}

// ===========================================================================
// 4 — case 4 beats case 5.
// ===========================================================================

/// An unauthenticated caller is refused BEFORE its params are used — proven by
/// the store never answering.
///
/// # The probe this test does NOT use, and why
///
/// The plan specified sending MALFORMED params with no credentials and asserting
/// `-32003` rather than `-32602`. Measurement says that probe cannot observe
/// this ordering on this transport: `ClientRequest` is deserialized at INGRESS,
/// before any dispatch runs, and a `taskId` of the wrong JSON type simply fails
/// to match the `tasks/get` variant — so the request is answered
/// `-32601 "Method not found: tasks/get"` and never reaches
/// `route_tasks_endpoint` at all. Leg C below pins that measured fact so it stays
/// a recorded caveat rather than folklore. (That a known method with bad params
/// answers `-32601` rather than `-32602` is a pre-existing transport-level
/// behaviour, out of this plan's scope; it is logged in `deferred-items.md`.)
///
/// # The probe this test uses instead
///
/// Identical, WELL-FORMED params sent twice to the same server, differing only
/// in the credential. The authenticated leg reaches the store and hears its
/// "task not found"; the unauthenticated leg hears `-32003` and never learns
/// whether the task exists. That is a strictly stronger statement than the
/// original probe: it proves not merely that the params were not PARSED but that
/// no store or router was consulted with them, which is what T-114-37 actually
/// requires — and it doubles as the guard against the refusal becoming an
/// existence oracle.
#[tokio::test]
async fn an_unauthenticated_caller_is_refused_before_the_params_parse() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;
    let absent = task_params("a-task-that-does-not-exist");

    // Leg A — no credentials.
    let refused = tasks_post(addr, "tasks/get", 40, absent.clone(), &[]).await;
    // Leg B — the SAME params, with credentials. The non-vacuity control: it
    // proves the store WOULD have answered had the chain got that far.
    let reached_store = tasks_post(addr, "tasks/get", 42, absent, &bearer()).await;
    // Leg C — the measured caveat about malformed params.
    let malformed = tasks_post(addr, "tasks/get", 43, json!({ "taskId": 12345 }), &[]).await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&refused),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "the auth refusal fires BEFORE the params are used — no store, no router: {}",
        refused.raw
    );
    assert!(
        !refused.raw.contains("not found"),
        "the refused caller must not learn whether the task exists: a refusal that \
         leaked the store's answer would be an existence oracle (T-114-40): {}",
        refused.raw
    );
    assert_ne!(
        error_code(&reached_store),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "the control: with a credential the SAME request reaches the store, so leg A's \
         refusal is attributable to the credential and not to the params: {}",
        reached_store.raw
    );
    assert!(
        reached_store.raw.contains("not found"),
        "and the store genuinely answered for the authenticated caller — without this \
         the ordering claim above would be vacuous: {}",
        reached_store.raw
    );
    assert_ne!(
        error_code(&malformed),
        Some(i64::from(INVALID_PARAMS)),
        "measured caveat: malformed params are rejected at INGRESS, so `-32602` is not \
         the answer here either — see this test's rustdoc: {}",
        malformed.raw
    );
}

// ===========================================================================
// 5 — the refusal's wire shape.
// ===========================================================================

/// The `-32003` answers at HTTP 200, echoes the original id, and carries no
/// result.
///
/// `AUTHENTICATION_REQUIRED` is DELIBERATELY absent from `v2_status_for_code`'s
/// 400 arm: remapping it would change the status of every other emitter of that
/// code across the transport, so the transport file is untouched by this plan
/// (T-114-43). The four assertions each name the decision they protect, the
/// shape `tests/v2_subscriptions.rs` established for the sibling
/// `subscriptions/listen` refusal.
#[tokio::test]
async fn the_refusal_echoes_the_original_id_at_http_200() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let refused = tasks_post(addr, "tasks/get", 41, task_params("absent"), &[]).await;
    teardown(handle, ()).await;

    assert_eq!(
        refused.status, 200,
        "-32003 is deliberately unremapped: it is not in v2_status_for_code's 400 arm, \
         and putting it there would move every other emitter of that code on this \
         transport: {}",
        refused.raw
    );
    assert_eq!(
        error_code(&refused),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "the refusal is -32003, the same fail-closed answer the MRTR ingress and \
         subscriptions/listen give on this server: {}",
        refused.raw
    );
    assert_eq!(
        refused.body["id"],
        json!(41),
        "the ORIGINAL request id is echoed: {}",
        refused.raw
    );
    assert!(
        refused.body["result"].is_null(),
        "a refusal carries no result: {}",
        refused.raw
    );
}

// ===========================================================================
// 6 — row 3: fail-closed is not "refuse everyone".
// ===========================================================================

/// A server with NO auth provider still serves v2 `tasks/*` to an
/// unauthenticated caller.
///
/// The counterweight to the whole suite. Without it, "make v2 fail closed" is
/// satisfiable by refusing every anonymous caller, which would break every
/// stdio/dev server — the configuration the shipped examples use. Row 3 of the
/// identity table maps this case onto `ANONYMOUS_PRINCIPAL`: one SHARED bucket,
/// by design, on a server that has no notion of caller identity to separate.
/// That is a development affordance and NOT per-caller isolation, and the
/// production backends bound it independently (`allow_anonymous: false`,
/// T-114-39).
#[tokio::test]
async fn a_no_auth_provider_server_still_serves_tasks() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;

    let got = tasks_post(addr, "tasks/get", 50, task_params("absent"), &[]).await;
    teardown(handle, ()).await;

    assert_ne!(
        error_code(&got),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "a server with NO auth provider has no authentication to demand — refusing here \
         would break every stdio/dev deployment: {}",
        got.raw
    );
    assert_ne!(
        error_code(&got),
        Some(i64::from(MISSING_REQUIRED_CLIENT_CAPABILITY)),
        "the request DID declare the tasks extension, so case 3 must not fire: {}",
        got.raw
    );
}

// ===========================================================================
// 7 — row 1, the non-vacuity control.
// ===========================================================================

/// An AUTHENTICATED caller is served, on the same server that refuses an
/// unauthenticated one.
///
/// The non-vacuity control for tests 4 and 5: without it, a server that refused
/// EVERY caller — a broken auth wiring, say — would satisfy them both. The pair
/// runs against the same `AuthPosture::Optional` server so the only difference
/// between the two outcomes is the credential.
#[tokio::test]
async fn an_authenticated_caller_binds_its_own_subject() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let anonymous = tasks_post(addr, "tasks/get", 60, task_params("absent"), &[]).await;
    let authenticated = tasks_post(addr, "tasks/get", 61, task_params("absent"), &bearer()).await;
    teardown(handle, ()).await;

    assert_eq!(
        error_code(&anonymous),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "the control: the SAME server refuses an unauthenticated caller: {}",
        anonymous.raw
    );
    assert_ne!(
        error_code(&authenticated),
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "a caller presenting a bearer binds its subject and is NOT refused — otherwise \
         the refusal above proves nothing about the credential: {}",
        authenticated.raw
    );
}

// ===========================================================================
// 8 — the refusals stay mutually distinguishable.
// ===========================================================================

/// Every refusal this chain can produce is DIFFERENT, collected by re-driving
/// the real paths rather than declared.
///
/// Distinguishability is the mitigation, not a nicety: a caller has to be able
/// to tell "this method was retired" from "this server serves no tasks" from
/// "declare the extension" from "authenticate", because each calls for a
/// different fix.
#[tokio::test]
async fn the_refusals_are_mutually_distinguishable() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;
    let retired = error_code(&tasks_post(addr, "tasks/list", 70, json!({}), &[]).await);
    let undeclared =
        error_code(&undeclared_tasks_post(addr, "tasks/get", 71, task_params("absent"), &[]).await);
    let unauthenticated =
        error_code(&tasks_post(addr, "tasks/get", 72, task_params("absent"), &[]).await);
    teardown(handle, ()).await;

    let (bare_addr, bare_handle) = spawn_backendless().await;
    let no_backend =
        error_code(&tasks_post(bare_addr, "tasks/get", 73, task_params("absent"), &[]).await);
    teardown(bare_handle, ()).await;

    assert_eq!(
        retired,
        Some(i64::from(METHOD_NOT_FOUND)),
        "case 1 is a -32601"
    );
    assert_eq!(
        no_backend,
        Some(i64::from(METHOD_NOT_FOUND)),
        "case 2 is a -32601 too — the pair is distinguished by MESSAGE, which \
         tests/v2_tasks_era_gates.rs asserts pairwise"
    );
    assert_eq!(
        undeclared,
        Some(i64::from(MISSING_REQUIRED_CLIENT_CAPABILITY)),
        "case 3 is a -32021"
    );
    assert_eq!(
        unauthenticated,
        Some(i64::from(AUTHENTICATION_REQUIRED)),
        "case 4 is a -32003"
    );
    assert_ne!(
        undeclared, unauthenticated,
        "cases 3 and 4 must stay DIFFERENT codes: collapsing them is the ambiguity \
         DQ3 chose two codes to avoid"
    );
}
