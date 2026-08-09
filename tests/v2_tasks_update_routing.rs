//! `tasks/update` ROUTING, and the substitute for the compile-time protection
//! that routing choice structurally loses (Phase 114, plan 13 — TASK-02).
//!
//! # Two halves, and they are not the same kind of test
//!
//! **Half one — routing and ORDERING.** `tasks/update` is a 2026-07-28-only
//! method with no public [`ClientRequest`] variant, so it rides the crate-private
//! `InternalClientRequest` route that Phase 112 built for `server/discover`. These
//! tests drive REAL bytes over a loopback socket through the real
//! `StreamableHttpServer` and assert which of the five ordered gates answered.
//!
//! **Half two — the LOST GUARD.** `client_request_mrtr_eligible`
//! (`src/server/core.rs`) is an EXHAUSTIVE no-wildcard match over `ClientRequest`;
//! a new variant there is a compile error that forces its author to classify the
//! method. Because this design adds NOTHING to that enum, that compile-time
//! protection does not apply to `tasks/update` at all. Pitfall 4 is what it was
//! protecting against, and it is severe: `splice_mrtr_params` removes
//! `inputResponses` **unconditionally** from an MRTR-eligible method's params, and
//! `inputResponses` IS the entire `tasks/update` payload — so an eligible
//! `tasks/update` would have its request body deleted in flight and would return a
//! successful-looking empty acknowledgement while the task never left
//! `input_required`. Tests 5, 6 and 7 replace that one compile error with three
//! runtime/source assertions, each catching a DIFFERENT mistake.
//!
//! # The properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `tasks_update_reaches_dispatch_on_v2` | a negotiated, declaring, authenticated request reaches the router — NOT `-32601` |
//! | 2 | `tasks_update_is_method_not_found_on_v1` | the v1 freeze: the method does not exist on 2025-11-25 |
//! | 3 | `malformed_tasks_update_params_yield_32602_not_a_parse_error` | a garbage body becomes a structured `-32602` AFTER the gates |
//! | 3b | `an_undeclaring_v2_caller_is_refused_before_the_params_parse` | gate 3 (`-32021`) precedes the params |
//! | 4 | `malformed_params_from_an_unauthenticated_caller_yield_32003` | gate 4 (`-32003`) precedes the params |
//! | 5 | `tasks_update_is_not_mrtr_eligible` | the TABLE mistake — runtime, through the production predicate |
//! | 6 | `no_source_site_routes_tasks_update_through_the_mrtr_ingress` | the CALL-SITE mistake — source, with an anti-vacuity check |
//! | 7 | `client_request_has_no_tasks_update_variant` | the SEMVER mistake, as a test rather than only as CI |
//!
//! Tests 5 and 6 are deliberately NOT redundant and the difference is measurable:
//! adding a `tasks/update` row to `MRTR_METHODS` fails 5 and NOT 6 (the row names
//! no ingress function), while writing `splice_mrtr_params(.., TASKS_UPDATE_METHOD)`
//! at a call site fails 6 and NOT 5 (the table is untouched). Both controls were
//! run; see `114-13-SUMMARY.md`.
//!
//! # Why the scanner primitives are restated here
//!
//! A Rust integration test is its own crate, so this file cannot import
//! `tests/v2_prohibited_error_codes.rs`'s scanner. The IDIOM is deliberately
//! identical — runtime `read_dir` discovery, comment/literal stripping with a line
//! map, `cfg(test)`-region exclusion, a justified allowlist that rot-checks itself,
//! and an anti-vacuity fixture — so the repository has one source-scanning shape
//! rather than two.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    header, post, spawn_tasks_server, teardown, v1_body, v2_body_with_caps,
    v2_body_with_client_extensions, v2_headers, AuthPosture, Resp, PAUSING_TOOL_NAME, V1,
};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::fs;
use std::net::SocketAddr;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The wire method under test. Spelled once here; the crate's own single-sourced
/// constant is `pub(crate)` and therefore unreachable from an integration crate.
const TASKS_UPDATE: &str = "tasks/update";

/// The `Mcp-Session-Id` request header, for the v1 legs (D-114-J: a v1 caller
/// against the shared harness must complete a real handshake or it gets `-32600`,
/// which looks like a tasks bug and is not one).
const SESSION_HEADER: &str = "Mcp-Session-Id";

/// The principal every authenticated request in this suite binds to.
const SUBJECT: &str = "alice";

// ===========================================================================
// Driving the server.
// ===========================================================================

fn auth_header() -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {SUBJECT}"))]
}

/// A v2 request that DECLARES the tasks extension, from an AUTHENTICATED caller.
async fn declaring(addr: SocketAddr, method: &str, name: &str, id: i64, params: Value) -> Resp {
    let mut headers = v2_headers(method, name);
    headers.extend(auth_header());
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &headers, &body).await
}

/// The SAME v2 request with the extension NOT declared.
///
/// Differs from [`declaring`] in EXACTLY one key — the `extensions` map — so a
/// difference in outcome is attributable to the declaration and nothing else.
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

/// A DECLARING v2 request from an UNAUTHENTICATED caller.
///
/// Only [`AuthPosture::Optional`] can produce this: its provider returns
/// `Ok(None)` for a missing token, so the request reaches dispatch with
/// `auth_context: None` and `has_auth_provider: true` — the `(None, true)` row of
/// the fail-closed identity table. `AuthPosture::Required` would be answered `401`
/// by the transport long before the router's gate chain ran.
async fn declaring_anonymous(
    addr: SocketAddr,
    method: &str,
    name: &str,
    id: i64,
    params: Value,
) -> Resp {
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &v2_headers(method, name), &body).await
}

/// Mint a v1 session against the STATEFUL shared harness (D-114-J).
async fn v1_session(addr: SocketAddr) -> String {
    // A `--no-default-features --features full-v2` build mints nothing and
    // validates nothing: the transport spec's "an inbound Mcp-Session-Id is
    // IGNORED" is structural there. Carrying a placeholder id exercises exactly
    // that and keeps every v1 leg below RUNNING on the severed build.
    if !cfg!(feature = "v1-compat") {
        return "no-session-on-a-severed-build".to_string();
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
                "clientInfo": { "name": "tasks-update-v1-client", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    init.mcp_session_id
        .unwrap_or_else(|| panic!("a v1 initialize mints a session; body was {}", init.raw))
}

/// The distinctive value the malformed body carries, so the "does the refusal
/// echo the caller back into the operator's logs?" assertion has something
/// unmistakable to look for.
const MALFORMED_MARKER: &str = "gate-ordering-marker-17";

/// The addressable task id [`malformed_update_params`] carries.
///
/// It must be a well-formed STRING even though the body is deliberately
/// malformed: since Phase 118 D-18 the v2 header gate cross-checks `Mcp-Name`
/// against `params.taskId` for every `tasks/*` method, so a body with no
/// string task id can never reach the gates this file measures — it is answered
/// `-32020` at the transport boundary instead. Keeping the id valid and breaking
/// `inputResponses` is what preserves what these tests exist to measure.
const MALFORMED_TASK_ID: &str = "malformed-update-task";

/// Params that are structurally a `tasks/update` but whose `inputResponses` is
/// not an object, so the router answers `-32602`.
///
/// It is deliberately not "obviously broken JSON": a body that failed to PARSE
/// would prove nothing about gate ORDERING, because a parse failure is exactly
/// the outcome these tests exist to rule out.
fn malformed_update_params() -> Value {
    json!({ "taskId": MALFORMED_TASK_ID, "inputResponses": MALFORMED_MARKER })
}

/// The `error` object of a JSON-RPC response, or a panic naming the raw bytes.
fn error_of(response: &Resp) -> &Value {
    response.body.get("error").unwrap_or_else(|| {
        panic!("expected a JSON-RPC error, got {}", response.raw);
    })
}

fn code_of(response: &Resp) -> i64 {
    error_of(response)["code"]
        .as_i64()
        .unwrap_or_else(|| panic!("an error carries a numeric code; got {}", response.raw))
}

fn message_of(response: &Resp) -> String {
    error_of(response)["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Every JSON-RPC error code this suite distinguishes between, read from the
/// PUBLIC table so no number is hand-typed.
mod codes {
    use pmcp::types::protocol::error_codes as ec;
    pub const METHOD_NOT_FOUND: i64 = ec::METHOD_NOT_FOUND as i64;
    pub const INVALID_PARAMS: i64 = ec::INVALID_PARAMS as i64;
    pub const INTERNAL_ERROR: i64 = ec::INTERNAL_ERROR as i64;
    pub const AUTHENTICATION_REQUIRED: i64 = ec::AUTHENTICATION_REQUIRED as i64;
    pub const MISSING_REQUIRED_CLIENT_CAPABILITY: i64 =
        ec::MISSING_REQUIRED_CLIENT_CAPABILITY as i64;
}

// ===========================================================================
// 1-4. Routing and gate ORDERING, over a real socket.
// ===========================================================================

/// A fully negotiated, DECLARING, AUTHENTICATED v2 `tasks/update` on a REAL
/// paused task reaches `TaskDispatch` — it is NOT `-32601`.
///
/// The task is created through a real `tools/call` on the shared harness's
/// `pausing_task_tool`, which is the only client-reachable way to produce an
/// `input_required` task (114-12). So this addresses a task that genuinely exists
/// and is genuinely awaiting input, rather than an id the suite invented.
///
/// The answer asserted was plan 13's placeholder `-32603`; since **plan 114-14**
/// landed the delivery body it is the real `UpdateTaskResult` — an EMPTY
/// acknowledgement with no `error` at all. This test's job is unchanged: prove
/// the request REACHES `TaskDispatch` rather than falling out at one of the four
/// gates above it. It asserts that by naming every refusal that must NOT happen,
/// so it stays a routing test rather than becoming a second copy of
/// `tests/v2_tasks_update.rs`'s delivery-semantics suite.
///
/// The `inputResponses` fixture is a real `ListRootsResult` (`{"roots": []}`),
/// because the harness's `pausing_task_tool` records its one request under the
/// `roots` key as `roots/list` and 114-14 decodes KIND-DIRECTED against that
/// record. The value this test used to send — `{"action": …, "content": …}` — is
/// an `ElicitResult` shape and is now correctly REFUSED under a roots key; that
/// refusal is exactly what `tests/v2_tasks_update.rs` exists to pin, and
/// asserting it here too would make one property two tests' business.
///
/// The `Mcp-Name` sent is the CONFORMANT `params.taskId` the ext-tasks routing
/// rule requires, so this also proves the server accepts it (server-side
/// name-bearing enforcement for `tasks/*` is deliberately off this phase).
#[tokio::test]
async fn tasks_update_reaches_dispatch_on_v2() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let created = declaring(
        addr,
        "tools/call",
        PAUSING_TOOL_NAME,
        1,
        json!({ "name": PAUSING_TOOL_NAME, "arguments": {} }),
    )
    .await;
    let task_id = created
        .body
        .get("result")
        .and_then(|result| result.get("taskId"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "a declaring v2 tools/call on the pausing tool mints a task handle; got {}",
                created.raw
            )
        })
        .to_string();

    let updated = declaring(
        addr,
        TASKS_UPDATE,
        &task_id,
        2,
        json!({
            "taskId": task_id,
            "inputResponses": { "roots": { "roots": [] } },
        }),
    )
    .await;

    assert!(
        updated.body.get("error").is_none(),
        "a fully gated tasks/update on a real paused task is answered, not refused: {}",
        updated.raw
    );
    // Every refusal that must NOT have happened, named individually so a
    // regression says WHICH gate started firing rather than only that one did.
    for (code, why) in [
        (
            codes::METHOD_NOT_FOUND,
            "tasks/update must REACH dispatch on v2, not fall through to -32601",
        ),
        (
            codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            "this request DECLARED the tasks extension",
        ),
        (
            codes::AUTHENTICATION_REQUIRED,
            "this request carried a bearer the OptionalBearer provider accepts",
        ),
        (
            codes::INVALID_PARAMS,
            "these params carry a string taskId and a kind-correct inputResponses map",
        ),
        (
            codes::INTERNAL_ERROR,
            "the delivery body landed in 114-14; a -32603 here means it did not run",
        ),
    ] {
        assert_ne!(
            updated.body["error"]["code"].as_i64(),
            Some(code),
            "{why}: {}",
            updated.raw
        );
    }
    // The full response tail ran — this is not a pipeline bypass.
    assert_eq!(
        updated.mcp_version.as_deref(),
        Some(common::v2::V2),
        "a v2 response echoes the negotiated version: {}",
        updated.raw
    );
    assert_eq!(updated.body["id"], json!(2), "the ORIGINAL id is preserved");

    teardown(handle, ()).await;
}

/// `tasks/update` does not exist on MCP 2025-11-25 — the v1 freeze for this
/// method.
///
/// v1 is deliberately UNCHANGED by this plan in every respect that a v1 client can
/// observe except one, recorded here on purpose: the method now answers a
/// structured `-32601` at HTTP 200 with the ORIGINAL id, where an unrecognised
/// method previously produced a transport `PARSE_ERROR` at HTTP 400 with
/// `id: null`. That is the same deliberate, benign change Phase 112 made for
/// `server/discover` (D-10 finding #4), and it is justified by the same fact: no
/// conforming v1 client sends a v2-only method. `tests/v1_tasks_golden.rs` pins
/// the v1 wire and never sends this one.
///
/// The refusal must NOT be the v2 RETIREMENT sentence: "this method was retired"
/// and "this method does not exist yet" call for opposite fixes, and the message
/// is the only place a caller can tell them apart (T-114-33).
#[tokio::test]
async fn tasks_update_is_method_not_found_on_v1() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let session = v1_session(addr).await;

    let refused = post(
        addr,
        &[header(SESSION_HEADER, &session)],
        &v1_body(TASKS_UPDATE, json!(10), json!({ "taskId": "whatever" })),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::METHOD_NOT_FOUND,
        "tasks/update is a v2-only method: {}",
        refused.raw
    );
    let message = message_of(&refused);
    assert!(
        message.contains("2025-11-25"),
        "the v1 refusal names the protocol version that lacks the method: {message}"
    );
    assert!(
        !message.contains(pmcp::testing::V2_TASKS_METHOD_RETIRED),
        "a v1 caller must NOT be told the method was RETIRED — the opposite fix: {message}"
    );
    assert_eq!(
        refused.body["id"],
        json!(10),
        "the ORIGINAL id is preserved"
    );
    assert_eq!(
        refused.status, 200,
        "a structured JSON-RPC refusal, not a transport parse error: {}",
        refused.raw
    );

    teardown(handle, ()).await;
}

/// A DECLARING, AUTHENTICATED v2 caller sending garbage params gets a structured
/// `-32602` — not a transport parse error.
///
/// This is the half of the RAW-params contract that says the body IS eventually
/// judged. Tests 3b and 4 are the half that says it is judged LAST.
#[tokio::test]
async fn malformed_tasks_update_params_yield_32602_not_a_parse_error() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let refused = declaring(
        addr,
        TASKS_UPDATE,
        MALFORMED_TASK_ID,
        20,
        malformed_update_params(),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::INVALID_PARAMS,
        "a malformed tasks/update body is a structured -32602: {}",
        refused.raw
    );
    assert_eq!(
        refused.body["id"],
        json!(20),
        "a structured -32602 preserves the id; a transport parse error would send null: {}",
        refused.raw
    );
    assert!(
        !message_of(&refused).contains(MALFORMED_MARKER),
        "the refusal must not echo the caller's value back into the operator's logs: {}",
        refused.raw
    );

    teardown(handle, ()).await;
}

/// Gate 3 (`-32021`) runs BEFORE the params are read.
///
/// The request below is byte-identical to test 3's except that it omits the
/// `extensions` declaration. If the params were judged first it would answer
/// `-32602`; the declaration refusal is what it must answer, because "this method
/// is not available to you as configured" is a different — and earlier — fact than
/// "your body is wrong".
#[tokio::test]
async fn an_undeclaring_v2_caller_is_refused_before_the_params_parse() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let refused = non_declaring(
        addr,
        TASKS_UPDATE,
        MALFORMED_TASK_ID,
        21,
        malformed_update_params(),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
        "the declaration gate precedes the params parse: {}",
        refused.raw
    );
    assert!(
        error_of(&refused)["data"]["requiredCapabilities"]["extensions"][TASKS_EXTENSION_KEY]
            .is_object(),
        "the -32021 payload names the capability to declare, as an OBJECT: {}",
        refused.raw
    );

    teardown(handle, ()).await;
}

/// Gate 4 (`-32003`) runs BEFORE the params are read — the ordering guarantee.
///
/// **This is the assertion that would catch a future classifier change that
/// started deserializing params.** The request carries the same garbage body as
/// test 3; the only difference is the absent bearer. If the body were judged
/// anywhere upstream of the identity table — in `classify_internal_method`, in
/// `classify_http_ingress`, or by reordering the router's own chain — an
/// UNAUTHENTICATED caller would learn `-32602` instead of `-32003`, i.e. would get
/// a free parse of its own choosing on an unauthenticated path (T-114-63,
/// T-114-64).
#[tokio::test]
async fn malformed_params_from_an_unauthenticated_caller_yield_32003() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::Optional).await;

    let refused = declaring_anonymous(
        addr,
        TASKS_UPDATE,
        MALFORMED_TASK_ID,
        22,
        malformed_update_params(),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::AUTHENTICATION_REQUIRED,
        "the auth refusal precedes the params parse even though the classifier saw the body: {}",
        refused.raw
    );
    assert_ne!(
        code_of(&refused),
        codes::INVALID_PARAMS,
        "stated separately because -32602 here is the SPECIFIC regression this test exists for: {}",
        refused.raw
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// 5. The runtime half of the substitute guard — the TABLE mistake.
// ===========================================================================

/// `tasks/update` is name-bearing and NOT MRTR-eligible, read through the
/// PRODUCTION predicates.
///
/// # Pitfall 4, in full, because this assertion looks trivial and is not
///
/// A `tasks/update` row in `MRTR_METHODS` would make [`mrtr_eligible`] answer
/// `true`, and `splice_mrtr_params` strips `inputResponses` from an eligible
/// method's params **unconditionally** — the method gate is upstream, in
/// `mrtr_eligible`, not in the strip. `inputResponses` IS the entire `tasks/update`
/// payload. So the request would arrive at the router carrying nothing, the router
/// would find no inputs to deliver, and the caller would receive a
/// successful-looking empty acknowledgement while the task sat in
/// `input_required` forever. Nothing in the response would say so.
///
/// Before this plan the enum match `client_request_mrtr_eligible` would have
/// forced a decision when a `ClientRequest::TasksUpdate` variant was added. This
/// design adds no such variant, so that compile error can never fire for this
/// method — which is why the property is asserted here instead.
///
/// The two production wrappers this reads (`pmcp::testing::method_is_mrtr_eligible`
/// and `routing_name_key`) exist precisely so an integration crate can state this
/// pair; both method tables are `pub(crate)`.
#[test]
fn tasks_update_is_not_mrtr_eligible() {
    assert!(
        !pmcp::testing::method_is_mrtr_eligible(TASKS_UPDATE),
        "tasks/update must NOT be MRTR-eligible: splice_mrtr_params strips inputResponses \
         unconditionally, which for this method is the whole request body"
    );
    // Its two name-bearing siblings, so a control that widened eligibility to the
    // tasks family as a group is caught rather than only a control that added one
    // row.
    for method in ["tasks/get", "tasks/cancel"] {
        assert!(
            !pmcp::testing::method_is_mrtr_eligible(method),
            "{method} must NOT be MRTR-eligible"
        );
    }
    // The OTHER half of the pair: it is still name-bearing. An "obvious fix" that
    // removed the tasks rows from BOTH tables would satisfy the assertions above
    // and silently stop emitting the routing header the ext-tasks spec REQUIRES.
    assert_eq!(
        pmcp::testing::routing_name_key(TASKS_UPDATE),
        Some("taskId"),
        "tasks/update routes on params.taskId (TASK_NAME_BEARING_METHODS)"
    );
    // The three genuinely eligible methods are unchanged — this pins the set from
    // the other side, so a control that emptied MRTR_METHODS also fails here.
    for method in ["tools/call", "prompts/get", "resources/read"] {
        assert!(
            pmcp::testing::method_is_mrtr_eligible(method),
            "{method} is one of the three MRTR methods and must stay eligible"
        );
    }
}

// ===========================================================================
// 6-7. The source half of the substitute guard.
// ===========================================================================
//
// Modelled on `tests/v2_prohibited_error_codes.rs` §5 (plan 113-21 lineage).

/// The minimum length of an allowlist justification, in characters.
const MIN_JUSTIFICATION_CHARS: usize = 40;

/// The symbol every routing site names. The crate constant is `pub(crate)`, so
/// the scanner matches the IDENTIFIER rather than importing it.
const TASKS_UPDATE_SYMBOL: &str = "TASKS_UPDATE_METHOD";

/// The MRTR ingress surface `tasks/update` must never touch.
///
/// Three FUNCTIONS, deliberately not the `MRTR_METHODS` table: the table mistake
/// is test 5's, and keeping the two guards keyed on different things is what makes
/// a control that fails one fail only that one. `splice_mrtr_params` is the strip
/// itself; `mrtr_ingest` and `mrtr_egress` are the two ends that call into it.
const MRTR_INGRESS_TOKENS: &[&str] = &["splice_mrtr_params", "mrtr_ingest", "mrtr_egress"];

/// The two shapes an allowlist entry can have, kept distinct so a DEFINITION site
/// can never be mistaken for a ROUTING site.
enum SiteKind {
    /// The `pub(crate) const` declaration and the routing-name table row.
    Definition,
    /// A site that classifies or gates `tasks/update`. `must_contain` is a
    /// substring that MUST appear in the same file — the thing that makes this
    /// site's mention legitimate.
    Route { must_contain: &'static str },
}

struct SiteEntry {
    path: &'static str,
    kind: SiteKind,
    why: &'static str,
}

/// Every compiled `src/` file allowed to name [`TASKS_UPDATE_SYMBOL`] outside a
/// comment, a literal and a `cfg(test)` region.
///
/// Adding an unlisted file fails. Deleting a listed file's last non-comment
/// mention without deleting its entry fails. Removing a route entry's named
/// anchor fails. The population is small ON PURPOSE: `tasks/update` is a method
/// whose payload one wrong table row would delete, so every place that names it
/// should be a place someone chose.
const TASKS_UPDATE_SITES: &[SiteEntry] = &[
    SiteEntry {
        path: "src/types/mrtr.rs",
        kind: SiteKind::Definition,
        why: "The DEFINITION: `pub(crate) const TASKS_UPDATE_METHOD` plus its row in \
              TASK_NAME_BEARING_METHODS, the routing-name table. It declares the spelling and \
              the params key; it routes nothing and gates nothing.",
    },
    SiteEntry {
        path: "src/types/protocol/mod.rs",
        kind: SiteKind::Route {
            must_contain: "classify_internal_method",
        },
        why: "The CLASSIFIER. It re-exports the one constant and matches the raw method string \
              into InternalClientRequest::TasksUpdate, carrying params RAW. The named anchor is \
              the classifier itself: if it goes, this mention is no longer a routing decision.",
    },
    SiteEntry {
        path: "src/server/task_dispatch.rs",
        kind: SiteKind::Route {
            must_contain: "route_tasks_update",
        },
        why: "The ROUTER: the ordered gate chain (era, backend, declaration, auth, params) and \
              the two refusal messages that name the method. The named anchor is the entry \
              point every one of those gates lives inside.",
    },
    SiteEntry {
        path: "src/server/streamable_http_server.rs",
        kind: SiteKind::Route {
            must_contain: "HttpIngress::TasksUpdate",
        },
        why: "The TRANSPORT ingress fast-reject, which skips the typed parse for every method \
              that is not internally routed. The named anchor is the ingress variant this \
              method is classified into on the only transport that serves it.",
    },
    SiteEntry {
        path: "src/client/mod.rs",
        kind: SiteKind::Route {
            must_contain: "send_untyped_request",
        },
        why: "The CLIENT half (114-19): `Client::tasks_update` era-gates, asserts the tasks \
              capability, and emits the request. The named anchor is the untyped send it must \
              use — there is no ClientRequest variant to serialize, which is the same semver \
              fact the server side is built on. FOUND BY THIS SCANNER rather than predicted.",
    },
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

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

/// Every `.rs` file under `src/`, discovered at RUNTIME so a new file cannot
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

// --- source stripping (comments and literal CONTENTS removed, line map kept) ---

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
/// Whitespace runs COLLAPSE to a single space rather than vanishing: the needles
/// here are identifiers, which need word boundaries. Removing whitespace entirely
/// turns `const TASKS_UPDATE_METHOD` into `constTASKS_UPDATE_METHOD`, whose
/// preceding character is an identifier character — so the whole-token filter
/// would reject the definition site and the scan would silently lose coverage of
/// the very file it exists to watch. That was measured in the sibling file, not
/// predicted.
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

/// A whole-token match, so `TASKS_UPDATE_METHOD` never matches inside a longer
/// identifier such as `TASKS_UPDATE_METHODS`.
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
fn shipped_sites(symbol: &str) -> Vec<(String, Vec<u32>)> {
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

/// The statement-sized chunks of `text`, split on `;` and on either brace.
///
/// A "statement" here is deliberately coarse: what matters is that a tasks token
/// and an MRTR-ingress token cannot end up in one chunk unless they are genuinely
/// adjacent in one expression, one call, one table row or one `use`.
fn statements(text: &str) -> Vec<&str> {
    text.split([';', '{', '}']).collect()
}

/// The population of shipped `TASKS_UPDATE_METHOD` sites equals the declared
/// allowlist, and every ROUTE entry's named anchor is literally present.
///
/// An unlisted site fails. A missing anchor fails. A stale entry fails.
#[test]
fn every_tasks_update_site_is_allowlisted() {
    let observed: Vec<String> = shipped_sites(TASKS_UPDATE_SYMBOL)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let mut failures = String::new();

    for path in &observed {
        if !TASKS_UPDATE_SITES.iter().any(|e| e.path == *path) {
            let _ = writeln!(
                failures,
                "\n  UNLISTED site: {path} names {TASKS_UPDATE_SYMBOL}.\n    \
                 Every place that names this method should be a place someone chose: one wrong \
                 table row deletes its entire request payload. Add a TASKS_UPDATE_SITES entry \
                 saying what this site does."
            );
        }
    }

    for entry in TASKS_UPDATE_SITES {
        if !observed.iter().any(|path| path == entry.path) {
            let _ = writeln!(
                failures,
                "\n  DEAD allowlist entry: {} no longer names {TASKS_UPDATE_SYMBOL}.\n    \
                 Delete the entry. A stale one is how a real new site hides inside a population \
                 sized for a site since removed.",
                entry.path
            );
            continue;
        }
        let SiteKind::Route { must_contain } = entry.kind else {
            continue;
        };
        let source = fs::read_to_string(repo_root().join(entry.path))
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", entry.path));
        if !source.contains(must_contain) {
            let _ = writeln!(
                failures,
                "\n  MISSING anchor: {} no longer contains `{must_contain}`.\n    \
                 That is what made this file's mention of {TASKS_UPDATE_SYMBOL} a routing \
                 decision rather than a stray reference.",
                entry.path
            );
        }
    }

    assert!(
        failures.is_empty(),
        "the tasks/update routing-site population changed:{failures}"
    );
}

/// **The substitute for the lost compile-time guard.** No compiled `src/` site
/// puts `tasks/update` and the MRTR ingress in one statement.
///
/// Test 5 catches the TABLE mistake (a `MRTR_METHODS` row). This catches the
/// CALL-SITE mistake — someone reaching for `splice_mrtr_params`, `mrtr_ingest` or
/// `mrtr_egress` on a `tasks/update` path directly, which no table assertion can
/// see. Both are needed and neither subsumes the other; the measured proof is that
/// each of the two negative controls fails exactly one of them.
///
/// The consequence, once more because it is not obvious from the code: the strip
/// removes `inputResponses` UNCONDITIONALLY, and `inputResponses` is the whole
/// `tasks/update` payload — so a hit here means a request body deleted in flight
/// and an empty ack returned for a task that never resumed.
///
/// ANTI-VACUITY: a scanner that matches nothing is not a guard, so this FAILS if
/// the tasks symbol is found nowhere in `src/`, and if any MRTR ingress token is
/// found nowhere. Either would make the emptiness below meaningless.
#[test]
fn no_source_site_routes_tasks_update_through_the_mrtr_ingress() {
    // --- anti-vacuity, before the property ---
    let tasks_population = shipped_sites(TASKS_UPDATE_SYMBOL);
    assert!(
        !tasks_population.is_empty(),
        "the scanner found {TASKS_UPDATE_SYMBOL} NOWHERE in src/. Either the walk, the stripper \
         or the token filter is broken, and the emptiness check below would pass over nothing."
    );
    for token in MRTR_INGRESS_TOKENS {
        assert!(
            !shipped_sites(token).is_empty(),
            "the scanner found the MRTR ingress token `{token}` nowhere in src/; the proximity \
             check below would then be vacuous for it"
        );
    }

    // --- the property ---
    let mut failures = String::new();
    for path in src_files() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let stripped = strip(&source);
        let excluded = cfg_test_spans(&stripped);
        let mut offset: usize = 0;
        for statement in statements(&stripped.text) {
            let base = offset;
            offset += statement.len() + 1;
            let names_tasks = !token_hits(statement, TASKS_UPDATE_SYMBOL).is_empty()
                || statement.contains("tasks/update");
            if !names_tasks {
                continue;
            }
            if is_excluded(&excluded, base) {
                continue;
            }
            for token in MRTR_INGRESS_TOKENS {
                if token_hits(statement, token).is_empty() {
                    continue;
                }
                let _ = writeln!(
                    failures,
                    "\n  {} line {}: `{token}` and tasks/update in ONE statement.\n    \
                     splice_mrtr_params strips `inputResponses` unconditionally and that key IS \
                     the tasks/update payload, so this deletes the request body in flight and \
                     returns a successful-looking empty ack for a task that never resumed. If \
                     this is genuinely intended, it needs its own written-down exception here.",
                    rel(&path),
                    line_of(&stripped, base)
                );
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Pitfall 4 tripwire — tasks/update reached the MRTR ingress:{failures}"
    );
}

/// **The semver half of the substitute guard.** The public `ClientRequest` enum
/// has no `TasksUpdate` variant.
///
/// Adding one is `enum_variant_added` on a public EXHAUSTIVE enum — a semver-MAJOR
/// break that would fail this milestone's 2.x-minor promise and every downstream
/// exhaustive `match`. `cargo semver-checks check-release` catches it in CI; this
/// catches it in `cargo test`, with a message that says WHY, which is the
/// difference between a developer fixing the design and a developer adding
/// `#[non_exhaustive]` (itself a source break).
///
/// It is scoped to the `pub enum ClientRequest` BLOCK, not to the file: the file
/// legitimately names `TasksUpdate` several times, on the crate-private
/// `InternalClientRequest` that carries the method instead.
#[test]
fn client_request_has_no_tasks_update_variant() {
    let path = repo_root().join("src/types/protocol/mod.rs");
    let source = fs::read_to_string(&path).expect("protocol/mod.rs is readable");

    let start = source
        .find("\npub enum ClientRequest {")
        .expect("the `pub enum ClientRequest` declaration still exists");
    let rest = &source[start + 1..];
    let end = rest
        .find("\n}\n")
        .expect("the ClientRequest block is brace-terminated at column 0");
    let block = &rest[..end];

    assert!(
        !block.contains("TasksUpdate"),
        "`pub enum ClientRequest` gained a TasksUpdate variant. That is enum_variant_added on a \
         PUBLIC EXHAUSTIVE enum — a semver-MAJOR break, and every downstream exhaustive match \
         stops compiling. Adding #[non_exhaustive] instead is ALSO a source break. tasks/update \
         is carried by the crate-private InternalClientRequest::TasksUpdate and classified by \
         classify_internal_method; route it there.\n\nBlock was:\n{block}"
    );
    // Anti-vacuity: the block must be the real one, i.e. carry the variants that
    // ARE there. A `find` that matched a comment would slice something inert.
    for variant in ["TasksGet(", "TasksResult(", "TasksList(", "TasksCancel("] {
        assert!(
            block.contains(variant),
            "the extracted block is not the real ClientRequest enum — it lacks {variant}"
        );
    }
    assert!(
        !block.contains("non_exhaustive"),
        "ClientRequest must stay exhaustive: adding #[non_exhaustive] breaks every downstream \
         exhaustive match, which is the same harm by another route"
    );
}

/// The allowlist is a set of decisions, not a set of labels.
#[test]
fn every_tasks_update_site_entry_carries_a_substantive_justification() {
    let mut seen: Vec<&str> = Vec::new();
    for entry in TASKS_UPDATE_SITES {
        let why = entry.why.trim();
        assert!(
            why.len() >= MIN_JUSTIFICATION_CHARS,
            "TASKS_UPDATE_SITES entry {} needs a real justification, not {why:?}",
            entry.path
        );
        assert!(
            !seen.contains(&why),
            "TASKS_UPDATE_SITES entry {} reuses another entry's justification verbatim; a \
             copy-pasted reason is not a reason",
            entry.path
        );
        seen.push(why);
        assert!(
            repo_root().join(entry.path).is_file(),
            "TASKS_UPDATE_SITES entry {} does not exist",
            entry.path
        );
    }
    assert_eq!(
        TASKS_UPDATE_SITES
            .iter()
            .filter(|e| matches!(e.kind, SiteKind::Definition))
            .count(),
        1,
        "there is exactly ONE definition site for the tasks/update spelling; more than one means \
         the constant was duplicated rather than re-exported"
    );
}

// ===========================================================================
// Tests for the scanner itself — without these the tripwires above can pass
// vacuously, which is the failure mode plan 113-09 found twice in this phase.
// ===========================================================================

mod scanner {
    use super::{cfg_requires_test, line_of, statements, strip, token_hits, MRTR_INGRESS_TOKENS};

    fn find_token(source: &str, needle: &str) -> Option<u32> {
        let stripped = strip(source);
        let at = *token_hits(&stripped.text, needle).first()?;
        Some(line_of(&stripped, at))
    }

    #[test]
    fn a_bare_mention_is_counted() {
        let source = "fn f() -> &'static str {\n    TASKS_UPDATE_METHOD\n}\n";
        assert_eq!(find_token(source, "TASKS_UPDATE_METHOD"), Some(2));
    }

    #[test]
    fn a_mention_only_inside_a_comment_is_not_counted() {
        let source = "fn f() {\n    // never route TASKS_UPDATE_METHOD through MRTR\n    g();\n}\n";
        assert!(find_token(source, "TASKS_UPDATE_METHOD").is_none());
    }

    #[test]
    fn a_mention_only_inside_a_string_literal_is_not_counted() {
        let source = "fn f() {\n    panic!(\"TASKS_UPDATE_METHOD is not eligible\");\n}\n";
        assert!(find_token(source, "TASKS_UPDATE_METHOD").is_none());
    }

    #[test]
    fn a_longer_identifier_is_not_a_whole_token_match() {
        let source = "const TASKS_UPDATE_METHODS: [&str; 0] = [];\n";
        assert!(find_token(source, "TASKS_UPDATE_METHOD").is_none());
    }

    #[test]
    fn the_definition_site_survives_whitespace_collapse() {
        // The measured regression the sibling scanner documents: removing
        // whitespace entirely makes `const TASKS_UPDATE_METHOD` unmatchable.
        let source = "pub(crate) const TASKS_UPDATE_METHOD: &str = \"x\";\n";
        assert_eq!(find_token(source, "TASKS_UPDATE_METHOD"), Some(1));
    }

    /// The statement splitter separates two things that are merely NEARBY from
    /// two things that are in one expression.
    #[test]
    fn statements_separate_adjacent_items_but_not_one_call() {
        let one_call = strip("splice_mrtr_params(&mut p, TASKS_UPDATE_METHOD);");
        let chunks = statements(&one_call.text);
        assert!(
            chunks.iter().any(|s| s.contains("splice_mrtr_params")
                && !token_hits(s, "TASKS_UPDATE_METHOD").is_empty()),
            "one call expression must land in ONE chunk: {chunks:?}"
        );

        let adjacent = strip("const A: &str = TASKS_UPDATE_METHOD; fn splice_mrtr_params() {}");
        let chunks = statements(&adjacent.text);
        assert!(
            !chunks.iter().any(|s| s.contains("splice_mrtr_params")
                && !token_hits(s, "TASKS_UPDATE_METHOD").is_empty()),
            "two separate items must NOT share a chunk: {chunks:?}"
        );
    }

    #[test]
    fn cfg_predicates_requiring_test_are_recognised() {
        assert!(cfg_requires_test("test"));
        assert!(cfg_requires_test(
            "all(test, not(target_arch = \"wasm32\"))"
        ));
        assert!(!cfg_requires_test("not(target_arch = \"wasm32\")"));
        assert!(!cfg_requires_test("feature = \"streamable-http\""));
    }

    #[test]
    fn the_ingress_token_list_is_not_empty() {
        assert_eq!(
            MRTR_INGRESS_TOKENS.len(),
            3,
            "the three MRTR ingress functions; shrinking this list silently narrows the guard"
        );
    }
}
