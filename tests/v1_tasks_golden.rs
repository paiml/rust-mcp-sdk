//! Phase 114-02 (TASK-05/TASK-06, D-14 item 2): **byte-identity golden fixtures
//! for the v1 `tasks/*` wire, captured BEFORE Phase 114's v2 reshape lands.**
//!
//! # Read this before you change a literal in this file
//!
//! A diff in a golden literal here is a **v1 WIRE BREAK**, not a fixture that
//! drifted. These literals were captured from the unmodified tree on
//! 2026-07-28, ahead of every plan in this phase that touches
//! `src/server/task_dispatch.rs`, `src/types/tasks.rs` or the v2 egress
//! envelope. If a change you are making turns one of these tests red, the
//! correct response is to make your change v2-only — **not** to re-record the
//! golden. Re-recording is exactly the failure D-14 item 2 exists to prevent:
//! "the v1 suite still passes" is not byte-identity evidence, because a wire
//! reshape is precisely the change that alters bytes while leaving every
//! structural assertion true.
//!
//! # Why a RAW-STRING comparison, and what the ONLY permitted normalization is
//!
//! [`assert_v1_bytes`] compares the **raw response text**, not merely the parsed
//! JSON. A structural comparison of parsed JSON cannot detect
//!
//! * key **order** (this crate builds `serde_json` with `preserve_order`, so
//!   wire order follows struct declaration order and is observable),
//! * **whitespace**, or
//! * **omission versus explicit null** (`"ttl":null` versus no `ttl` key at
//!   all),
//!
//! and those three are precisely what a serde-level reshape changes while every
//! structural assertion stays green.
//!
//! The ONLY normalization permitted before that comparison is **placeholder
//! substitution of genuinely time- or randomness-dependent VALUES** — the
//! store-minted `taskId` and the two RFC-3339 timestamps. A key is never
//! deleted, and the helper proves it two ways on every call: a same-width
//! substitution whose output length must equal the input's, and a per-key
//! occurrence count that must be unchanged. Deleting a key is what would hide an
//! omission-versus-null change, so it is structurally impossible here.
//!
//! # Both backend paths are covered
//!
//! D-11 claims the v2 reshape sits ABOVE the `serde_json::Value`
//! [`TaskRouter`](pmcp::server::tasks::TaskRouter) seam and leaves
//! router-backed servers untouched. A golden suite exercising only
//! `InMemoryTaskStore` could not detect a v1 byte change on the router path —
//! the path `DynamoDB`- and Redis-backed deployments actually use. So every one of
//! the six fixtures is run twice, once per backend. Three of the six differ
//! LEGITIMATELY between the two backends today; each such difference is called
//! out at its fixture so a later diff is attributable rather than surprising.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{post, spawn_stateless_config, v1_body, Resp};
use pmcp::server::task_store::{InMemoryTaskStore, StoreConfig, TaskStore};
use pmcp::server::tasks::TaskRouter;
use pmcp::server::typed_tool::TypedTool;
use pmcp::server::Server;
use pmcp::types::protocol::error_codes::{INTERNAL_ERROR, V1_TASK_PENDING};
use pmcp::types::tasks::RELATED_TASK_META_KEY;
use pmcp::types::{TaskSupport, ToolExecution};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;

// ===========================================================================
// Dynamic-value normalization.
// ===========================================================================

/// A response value that cannot be pinned because it is minted per run.
///
/// `token` replaces the value in the CANONICAL normalization (the one compared
/// against the golden literal). In the SAME-WIDTH normalization the token is
/// padded with `#` to the value's own byte width, so the normalized string is
/// exactly as long as the raw one — the check that proves the substitution
/// neither adds nor removes bytes and, in particular, never deletes a key.
struct DynamicField {
    /// The JSON object key whose STRING value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}

/// The store-minted task id: a v4 UUID, always 36 bytes.
const TASK_ID: DynamicField = DynamicField {
    key: "taskId",
    token: "<TASK-ID>",
    shape: is_uuid_shaped,
    shape_description: "a 36-byte hyphenated UUID",
};

/// `createdAt`: RFC 3339, UTC. NOT fixed width — `chrono`'s `to_rfc3339` prints
/// 0, 3, 6 or 9 fractional digits depending on the instant, which is why the
/// width-preserving substitution pads rather than assuming a constant length.
const CREATED_AT: DynamicField = DynamicField {
    key: "createdAt",
    token: "<TIMESTAMP>",
    shape: is_rfc3339_utc_shaped,
    shape_description: "an RFC-3339 UTC timestamp ending in +00:00",
};

/// `lastUpdatedAt`: same shape as [`CREATED_AT`], and deliberately the same
/// token — a fixture must not depend on the two differing, since `tasks/cancel`
/// advances one of them by microseconds.
const LAST_UPDATED_AT: DynamicField = DynamicField {
    key: "lastUpdatedAt",
    token: "<TIMESTAMP>",
    shape: is_rfc3339_utc_shaped,
    shape_description: "an RFC-3339 UTC timestamp ending in +00:00",
};

/// The three dynamic values on the store-backed path.
const STORE_DYNAMICS: &[DynamicField] = &[TASK_ID, CREATED_AT, LAST_UPDATED_AT];

/// The router-backed path returns values the test router chose, so NOTHING is
/// normalized there — those goldens are pinned verbatim, byte for byte.
const NO_DYNAMICS: &[DynamicField] = &[];

fn is_uuid_shaped(value: &str) -> bool {
    value.len() == 36
        && value
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            })
}

fn is_rfc3339_utc_shaped(value: &str) -> bool {
    value.len() >= 25
        && value.ends_with("+00:00")
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[..4].iter().all(u8::is_ascii_digit)
}

/// `token`, padded with `#` to exactly `width` bytes.
fn width_preserving(token: &str, width: usize) -> String {
    assert!(
        token.len() <= width,
        "placeholder `{token}` is wider than the {width}-byte value it replaces; \
         pick a shorter token rather than shortening the value"
    );
    let mut padded = String::with_capacity(width);
    padded.push_str(token);
    padded.push_str(&"#".repeat(width - token.len()));
    padded
}

/// Replace every dynamic value in `raw`.
///
/// With `same_width`, each value becomes a padded placeholder of its own width;
/// otherwise it becomes the bare canonical token. Both passes are pure string
/// operations, so key order, spacing and null-versus-absent all survive into the
/// comparison.
fn substitute(raw: &str, fields: &[DynamicField], same_width: bool) -> String {
    let mut out = raw.to_string();
    for field in fields {
        out = substitute_one(&out, field, same_width);
    }
    out
}

fn substitute_one(raw: &str, field: &DynamicField, same_width: bool) -> String {
    let needle = format!("\"{}\":\"", field.key);
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut hits = 0_usize;
    while let Some(position) = rest.find(needle.as_str()) {
        let value_start = position + needle.len();
        out.push_str(&rest[..value_start]);
        let tail = &rest[value_start..];
        let end = tail
            .find('"')
            .unwrap_or_else(|| panic!("unterminated `{}` string value in: {raw}", field.key));
        let value = &tail[..end];
        assert!(
            (field.shape)(value),
            "`{}` carried `{value}`, which is not {} — either the value shape \
             changed (a v1 wire break) or this fixture is normalizing the wrong key",
            field.key,
            field.shape_description
        );
        if same_width {
            out.push_str(&width_preserving(field.token, value.len()));
        } else {
            out.push_str(field.token);
        }
        rest = &tail[end..];
        hits += 1;
    }
    assert!(
        hits > 0,
        "declared dynamic key `{}` does not appear in the response — a golden \
         that normalizes an absent key proves nothing: {raw}",
        field.key
    );
    out.push_str(rest);
    out
}

fn key_occurrences(text: &str, key: &str) -> usize {
    text.matches(format!("\"{key}\":").as_str()).count()
}

// ===========================================================================
// The assertion helper.
// ===========================================================================

/// The payload half of the JSON-RPC frame a fixture pins.
enum Frame {
    /// A success frame: `{"jsonrpc":"2.0","id":…,"result":…}`.
    Result(Value),
    /// An error frame: `{"jsonrpc":"2.0","id":…,"error":…}`.
    Error(Value),
}

/// What `_meta` must look like on this frame.
///
/// The asymmetry is deliberate and is the one place this file departs from
/// `tests/v2_required_headers.rs`'s `assert_v1_byte_identical`, whose third
/// assertion is a blanket `!raw.contains("_meta")`. That would fail here for the
/// WRONG reason: `build_task_created_response`
/// (`src/server/task_dispatch.rs`, around lines 527-532) deliberately emits an
/// `_meta.relatedTask` envelope on the v1 create response, linking the
/// synchronous `tools/call` to the async task. So `_meta` is asserted ABSENT on
/// `tasks/get` / `tasks/list` / `tasks/cancel` / `tasks/result`, and asserted
/// PRESENT-and-exactly-`relatedTask` on the create envelope.
enum MetaExpectation {
    /// `_meta` must not appear anywhere in the raw response.
    Absent,
    /// `_meta` must carry exactly one key, the related-task slot, whose value is
    /// exactly `{"taskId": …}`.
    RelatedTaskOnly,
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays on one line: this is the
/// assertion a reviewer greps for when asking "does this file actually compare
/// bytes, or only parsed JSON?", and a macro split across four lines by
/// `rustfmt` answers that question much less clearly.
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 tasks wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — \
         make the change v2-only instead of re-recording the golden. Raw response was: {raw}"
    )
}

/// One pinned v1 response.
struct V1Golden<'a> {
    /// The JSON-RPC request id the frame must echo.
    id: i64,
    /// The full frame, byte for byte, after canonical normalization.
    raw: &'a str,
    /// The same frame's payload, for a readable structural failure message.
    frame: Frame,
    /// Values normalized before comparison (see [`DynamicField`]).
    dynamics: &'a [DynamicField],
    /// The `_meta` rule for this frame.
    meta: MetaExpectation,
}

/// Assert `raw` is byte-identical to `golden` once dynamic values are replaced.
///
/// Four things happen, in this order:
///
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged and every dynamic key's occurrence count unchanged. This is what
///    makes "the normalization never deletes a key" a checked property rather
///    than a comment.
/// 2. **RAW-STRING comparison** against the canonical golden — the load-bearing
///    assertion, and the only one that sees key order, spacing and
///    omission-versus-null.
/// 3. **Structural comparison** of the parsed frame. Note this crate's
///    `serde_json::Map` is an `IndexMap`, whose `PartialEq` is
///    order-INDEPENDENT, so step 3 is genuinely structural — it exists for the
///    readable message, and step 2 is what carries ordering.
/// 4. **v2 leak guards**: neither `resultType` nor `serverInfo` may appear on a
///    v1 wire, plus the `_meta` rule.
fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
    let same_width = substitute(raw, golden.dynamics, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the placeholder substitution changed the response length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in golden.dynamics {
        assert_eq!(
            key_occurrences(&same_width, field.key),
            key_occurrences(raw, field.key),
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));

    let parsed: Value = serde_json::from_str(&normalized).expect("v1 response must be valid JSON");
    let expected = match &golden.frame {
        Frame::Result(result) => json!({ "jsonrpc": "2.0", "id": golden.id, "result": result }),
        Frame::Error(error) => json!({ "jsonrpc": "2.0", "id": golden.id, "error": error }),
    };
    assert_eq!(
        parsed, expected,
        "the full JSON-RPC frame (jsonrpc + id + payload) must match the golden"
    );

    assert!(
        !raw.contains("resultType"),
        "v1 raw must not contain resultType: {raw}"
    );
    assert!(
        !raw.contains("serverInfo"),
        "v1 raw must not contain serverInfo: {raw}"
    );
    assert_meta(raw, &normalized, &golden.meta);
}

fn assert_meta(raw: &str, normalized: &str, expectation: &MetaExpectation) {
    match expectation {
        MetaExpectation::Absent => assert!(
            !raw.contains("_meta"),
            "this v1 tasks response must carry no _meta: {raw}"
        ),
        MetaExpectation::RelatedTaskOnly => {
            let parsed: Value = serde_json::from_str(normalized).expect("valid JSON");
            let meta = parsed["result"]["_meta"]
                .as_object()
                .unwrap_or_else(|| panic!("the create envelope must carry _meta: {raw}"));
            let keys: Vec<&String> = meta.keys().collect();
            assert_eq!(
                keys,
                vec![&RELATED_TASK_META_KEY.to_string()],
                "the create envelope's _meta must carry EXACTLY the relatedTask \
                 slot it carries today and nothing else: {raw}"
            );
            assert_eq!(
                meta[RELATED_TASK_META_KEY],
                json!({ "taskId": TASK_ID.token }),
                "the relatedTask slot must carry exactly the store-minted taskId: {raw}"
            );
        },
    }
}

// ===========================================================================
// Fixtures: tools.
// ===========================================================================

/// A task-capable tool that completes synchronously: it returns a task-shaped
/// value carrying a nested terminal `result`, so the create path records
/// create + `set_result` + `Completed` before responding.
fn completing_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "complete_now",
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "completed",
                    "ttl": 60000,
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z",
                    "result": { "content": [ { "type": "text", "text": "terminal payload" } ] }
                }))
            })
        },
    )
    .with_description("a task tool that completes synchronously")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// A task-capable tool that stays pending, and deliberately requests **no**
/// TTL — combined with [`store_backed_server`]'s `default_ttl_ms: None`, that
/// puts an explicit `"ttl":null` on the wire. `Task::ttl` is the one
/// `Option` field with no `skip_serializing_if`, so this fixture is what makes
/// an omission-versus-explicit-null regression detectable at all.
fn pending_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "stay_pending",
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z"
                }))
            })
        },
    )
    .with_description("a task tool that stays pending")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

// ===========================================================================
// Fixtures: the router-backed backend.
// ===========================================================================

/// The task id the test router reports as `working`.
const ROUTER_WORKING_TASK_ID: &str = "router-task-0001";
/// The task id the test router reports as `completed`.
const ROUTER_TERMINAL_TASK_ID: &str = "router-task-0002";

/// A [`TaskRouter`] returning FIXED `serde_json::Value` payloads.
///
/// Fixed on purpose: on this path the dispatcher is supposed to pass the
/// router's value through verbatim, so pinning constant bytes is exactly the
/// assertion D-11 needs — any envelope injection, field rename or re-projection
/// added above the `Value` seam shows up as a diff with nothing else moving.
struct GoldenRouter;

fn router_task(task_id: &str, status: &str) -> Value {
    json!({
        "taskId": task_id,
        "status": status,
        "ttl": 60000,
        "createdAt": "2026-01-01T00:00:00Z",
        "lastUpdatedAt": "2026-01-01T00:00:01Z",
        "pollInterval": 5000,
    })
}

#[async_trait]
impl TaskRouter for GoldenRouter {
    async fn handle_task_call(
        &self,
        _tool_name: &str,
        _arguments: Value,
        _task_params: Value,
        _owner_id: &str,
        _progress_token: Option<Value>,
    ) -> pmcp::Result<Value> {
        Ok(router_task(ROUTER_WORKING_TASK_ID, "working"))
    }

    async fn handle_tasks_get(&self, params: Value, _owner_id: &str) -> pmcp::Result<Value> {
        let requested = params.get("taskId").and_then(Value::as_str).unwrap_or("");
        let status = if requested == ROUTER_TERMINAL_TASK_ID {
            "completed"
        } else {
            "working"
        };
        Ok(json!({ "task": router_task(requested, status) }))
    }

    async fn handle_tasks_result(&self, _params: Value, _owner_id: &str) -> pmcp::Result<Value> {
        Err(pmcp::Error::internal(
            "router has no terminal result for this task",
        ))
    }

    async fn handle_tasks_list(&self, _params: Value, _owner_id: &str) -> pmcp::Result<Value> {
        Ok(json!({ "tasks": [router_task(ROUTER_WORKING_TASK_ID, "working")] }))
    }

    async fn handle_tasks_cancel(&self, params: Value, _owner_id: &str) -> pmcp::Result<Value> {
        let requested = params.get("taskId").and_then(Value::as_str).unwrap_or("");
        Ok(json!({ "task": router_task(requested, "cancelled") }))
    }

    fn resolve_owner(
        &self,
        subject: Option<&str>,
        _client_id: Option<&str>,
        _session_id: Option<&str>,
    ) -> String {
        subject.unwrap_or("local").to_string()
    }

    fn tool_requires_task(&self, _tool_name: &str, _tool_execution: Option<&Value>) -> bool {
        false
    }

    fn task_capabilities(&self) -> Value {
        json!({})
    }
}

// ===========================================================================
// Fixtures: servers and requests.
// ===========================================================================

/// A v1-ONLY server (no `2026-07-28` in its accept-list) backed by the in-crate
/// [`InMemoryTaskStore`].
///
/// `default_ttl_ms: None` is deliberate: it lets [`pending_task_tool`] produce a
/// task whose `ttl` is genuinely `None` and therefore appears on the wire as an
/// explicit `null`, while `complete_now` still carries a numeric TTL. Both
/// shapes are pinned.
fn store_backed_server() -> Server {
    let store = Arc::new(InMemoryTaskStore::with_config(StoreConfig {
        default_ttl_ms: None,
        max_ttl_ms: None,
        default_poll_interval_ms: 5000,
        max_tasks_per_owner: 100,
    })) as Arc<dyn TaskStore>;
    Server::builder()
        .name("v1-tasks-golden")
        .version("1.0.0")
        .tool("complete_now", completing_task_tool())
        .tool("stay_pending", pending_task_tool())
        .task_store(store)
        .build()
        .expect("store-backed server builds")
}

/// The same v1-only server backed by a [`TaskRouter`] instead of a store.
fn router_backed_server() -> Server {
    Server::builder()
        .name("v1-tasks-golden")
        .version("1.0.0")
        .tool("complete_now", completing_task_tool())
        .tool("stay_pending", pending_task_tool())
        .with_task_store(Arc::new(GoldenRouter) as Arc<dyn TaskRouter>)
        .build()
        .expect("router-backed server builds")
}

/// Spawn over real loopback HTTP with `enable_json_response: true`, so the raw
/// response text IS the JSON-RPC frame rather than an SSE-framed copy of it.
/// The framing is not what this file pins; the frame is.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_stateless_config(server).await
}

/// Shut the spawned server down through the shared harness's one teardown order
/// (drop sockets → `abort()` → `await`, D-113-T).
///
/// `()` is passed for the sockets because this file owns none of its own — every
/// request goes through the harness's pooled `reqwest` client.
async fn shutdown(handle: JoinHandle<()>) {
    common::v2::teardown(handle, ()).await;
}

/// A v1 `tools/call` carrying a `task` field (no v2 headers, no v2 `_meta`).
fn call_body(id: i64, tool: &str) -> String {
    v1_body(
        "tools/call",
        json!(id),
        json!({ "name": tool, "arguments": {}, "task": {} }),
    )
}

/// A v1 `tasks/*` request body.
fn tasks_body(id: i64, method: &str, params: Value) -> String {
    v1_body(method, json!(id), params)
}

/// POST a task-augmented `tools/call` and return the raw response.
async fn create_task(addr: SocketAddr, id: i64, tool: &str) -> Resp {
    post(addr, &[], &call_body(id, tool)).await
}

/// The store-minted task id from a create response.
fn minted_id(response: &Resp) -> String {
    response.body["result"]["task"]["taskId"]
        .as_str()
        .expect("the create envelope carries a store-minted taskId")
        .to_string()
}

// ===========================================================================
// Golden bodies — store-backed.
// ===========================================================================

const STORE_GET_WORKING: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"<TASK-ID>","status":"working","ttl":null,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000}}}"#;

const STORE_GET_TERMINAL: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"<TASK-ID>","status":"completed","ttl":60000,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000}}}"#;

const STORE_RESULT_PENDING: &str = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32002,"message":"task result not available: task not completed"}}"#;

const STORE_LIST: &str = r#"{"jsonrpc":"2.0","id":4,"result":{"tasks":[{"taskId":"<TASK-ID>","status":"working","ttl":null,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000}]}}"#;

const STORE_CANCEL: &str = r#"{"jsonrpc":"2.0","id":5,"result":{"task":{"taskId":"<TASK-ID>","status":"cancelled","ttl":null,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000}}}"#;

const STORE_CREATE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"task":{"taskId":"<TASK-ID>","status":"working","ttl":null,"createdAt":"<TIMESTAMP>","lastUpdatedAt":"<TIMESTAMP>","pollInterval":5000},"_meta":{"io.modelcontextprotocol/related-task":{"taskId":"<TASK-ID>"}}}}"#;

/// The pinned `Task` object, as a structural value.
fn store_task(status: &str, ttl: &Value) -> Value {
    json!({
        "taskId": TASK_ID.token,
        "status": status,
        "ttl": ttl,
        "createdAt": CREATED_AT.token,
        "lastUpdatedAt": LAST_UPDATED_AT.token,
        "pollInterval": 5000,
    })
}

// ===========================================================================
// Golden bodies — router-backed.
// ===========================================================================

const ROUTER_GET_WORKING: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"router-task-0001","status":"working","ttl":60000,"createdAt":"2026-01-01T00:00:00Z","lastUpdatedAt":"2026-01-01T00:00:01Z","pollInterval":5000}}}"#;

const ROUTER_GET_TERMINAL: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"router-task-0002","status":"completed","ttl":60000,"createdAt":"2026-01-01T00:00:00Z","lastUpdatedAt":"2026-01-01T00:00:01Z","pollInterval":5000}}}"#;

const ROUTER_RESULT_PENDING: &str = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32603,"message":"Internal error: router has no terminal result for this task"}}"#;

const ROUTER_LIST: &str = r#"{"jsonrpc":"2.0","id":4,"result":{"tasks":[{"taskId":"router-task-0001","status":"working","ttl":60000,"createdAt":"2026-01-01T00:00:00Z","lastUpdatedAt":"2026-01-01T00:00:01Z","pollInterval":5000}]}}"#;

const ROUTER_CANCEL: &str = r#"{"jsonrpc":"2.0","id":5,"result":{"task":{"taskId":"router-task-0001","status":"cancelled","ttl":60000,"createdAt":"2026-01-01T00:00:00Z","lastUpdatedAt":"2026-01-01T00:00:01Z","pollInterval":5000}}}"#;

const ROUTER_CREATE_FALLTHROUGH: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"taskId\":\"tool-fabricated\",\"status\":\"working\",\"createdAt\":\"2026-06-21T00:00:00Z\",\"lastUpdatedAt\":\"2026-06-21T00:00:00Z\"}"}],"isError":false}}"#;

// ===========================================================================
// Fixture 1 — tasks/get on a `working` task.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_get_working_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let task_id = minted_id(&create_task(addr, 1, "stay_pending").await);
    let got = post(
        addr,
        &[],
        &tasks_body(2, "tasks/get", json!({ "taskId": task_id })),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(got.status, 200, "v1 tasks/get must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 2,
            raw: STORE_GET_WORKING,
            frame: Frame::Result(json!({ "task": store_task("working", &Value::Null) })),
            dynamics: STORE_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_get_working_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let got = post(
        addr,
        &[],
        &tasks_body(2, "tasks/get", json!({ "taskId": ROUTER_WORKING_TASK_ID })),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(got.status, 200, "v1 tasks/get must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 2,
            raw: ROUTER_GET_WORKING,
            frame: Frame::Result(json!({
                "task": router_task(ROUTER_WORKING_TASK_ID, "working"),
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 2 — tasks/get on a `completed` task.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_get_terminal_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let task_id = minted_id(&create_task(addr, 1, "complete_now").await);
    let got = post(
        addr,
        &[],
        &tasks_body(2, "tasks/get", json!({ "taskId": task_id })),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(got.status, 200, "v1 tasks/get must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 2,
            raw: STORE_GET_TERMINAL,
            frame: Frame::Result(json!({ "task": store_task("completed", &json!(60000)) })),
            dynamics: STORE_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_get_terminal_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let got = post(
        addr,
        &[],
        &tasks_body(2, "tasks/get", json!({ "taskId": ROUTER_TERMINAL_TASK_ID })),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(got.status, 200, "v1 tasks/get must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 2,
            raw: ROUTER_GET_TERMINAL,
            frame: Frame::Result(json!({
                "task": router_task(ROUTER_TERMINAL_TASK_ID, "completed"),
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 3 — tasks/result while pending.
//
// LEGITIMATE BACKEND DIFFERENCE. The store-backed path emits the FROZEN
// `-32002` `V1_TASK_PENDING` body; the router-backed path never reaches that
// branch, because `handle_tasks_result`'s router fall-through returns first and
// a router `Err` becomes `-32603 INTERNAL_ERROR`. Both are today's bytes and
// both are pinned, so a later plan can tell which path it moved.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_result_pending_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let task_id = minted_id(&create_task(addr, 1, "stay_pending").await);
    let pending = post(
        addr,
        &[],
        &tasks_body(3, "tasks/result", json!({ "taskId": task_id })),
    )
    .await;
    shutdown(handle).await;

    assert_v1_bytes(
        &pending.raw,
        &V1Golden {
            id: 3,
            raw: STORE_RESULT_PENDING,
            frame: Frame::Error(json!({
                "code": V1_TASK_PENDING,
                "message": "task result not available: task not completed",
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_result_pending_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let pending = post(
        addr,
        &[],
        &tasks_body(
            3,
            "tasks/result",
            json!({ "taskId": ROUTER_WORKING_TASK_ID }),
        ),
    )
    .await;
    shutdown(handle).await;

    assert_v1_bytes(
        &pending.raw,
        &V1Golden {
            id: 3,
            raw: ROUTER_RESULT_PENDING,
            frame: Frame::Error(json!({
                "code": INTERNAL_ERROR,
                "message": "Internal error: router has no terminal result for this task",
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 4 — tasks/list.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_list_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let _ = create_task(addr, 1, "stay_pending").await;
    let listed = post(addr, &[], &tasks_body(4, "tasks/list", json!({}))).await;
    shutdown(handle).await;

    assert_eq!(listed.status, 200, "v1 tasks/list must still be served");
    assert_v1_bytes(
        &listed.raw,
        &V1Golden {
            id: 4,
            raw: STORE_LIST,
            frame: Frame::Result(json!({ "tasks": [store_task("working", &Value::Null)] })),
            dynamics: STORE_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_list_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let listed = post(addr, &[], &tasks_body(4, "tasks/list", json!({}))).await;
    shutdown(handle).await;

    assert_eq!(listed.status, 200, "v1 tasks/list must still be served");
    assert_v1_bytes(
        &listed.raw,
        &V1Golden {
            id: 4,
            raw: ROUTER_LIST,
            frame: Frame::Result(json!({
                "tasks": [router_task(ROUTER_WORKING_TASK_ID, "working")],
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 5 — tasks/cancel.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_cancel_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let task_id = minted_id(&create_task(addr, 1, "stay_pending").await);
    let cancelled = post(
        addr,
        &[],
        &tasks_body(5, "tasks/cancel", json!({ "taskId": task_id })),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        cancelled.status, 200,
        "v1 tasks/cancel must still be served"
    );
    assert_v1_bytes(
        &cancelled.raw,
        &V1Golden {
            id: 5,
            raw: STORE_CANCEL,
            frame: Frame::Result(json!({ "task": store_task("cancelled", &Value::Null) })),
            dynamics: STORE_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_cancel_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let cancelled = post(
        addr,
        &[],
        &tasks_body(
            5,
            "tasks/cancel",
            json!({ "taskId": ROUTER_WORKING_TASK_ID }),
        ),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        cancelled.status, 200,
        "v1 tasks/cancel must still be served"
    );
    assert_v1_bytes(
        &cancelled.raw,
        &V1Golden {
            id: 5,
            raw: ROUTER_CANCEL,
            frame: Frame::Result(json!({
                "task": router_task(ROUTER_WORKING_TASK_ID, "cancelled"),
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 6 — the create envelope from a task-augmented `tools/call`.
//
// LEGITIMATE BACKEND DIFFERENCE, and the larger of the two. On the store-backed
// server the create gate mints a task and answers with the NESTED
// `CreateTaskResult` envelope plus its `_meta.relatedTask` slot. On a
// router-ONLY server the gate in `maybe_build_task_created` requires
// `task_store.is_some()`, so it never opens: the call falls through to an
// ordinary `CallToolResult`. That fall-through IS today's v1 behaviour for a
// router-only high-level `Server`, so it is pinned as such — the router's own
// create envelope is only reachable through `ServerCore`, which this file does
// not drive.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_create_task_result_store_backed() {
    let (addr, handle) = spawn(store_backed_server()).await;
    let created = create_task(addr, 1, "stay_pending").await;
    shutdown(handle).await;

    assert_eq!(created.status, 200, "v1 tools/call must still be served");
    assert_v1_bytes(
        &created.raw,
        &V1Golden {
            id: 1,
            raw: STORE_CREATE,
            frame: Frame::Result(json!({
                "task": store_task("working", &Value::Null),
                "_meta": { RELATED_TASK_META_KEY: { "taskId": TASK_ID.token } },
            })),
            dynamics: STORE_DYNAMICS,
            meta: MetaExpectation::RelatedTaskOnly,
        },
    );
}

#[tokio::test]
async fn v1_tasks_golden_create_task_result_router_backed() {
    let (addr, handle) = spawn(router_backed_server()).await;
    let created = create_task(addr, 1, "stay_pending").await;
    shutdown(handle).await;

    assert_eq!(created.status, 200, "v1 tools/call must still be served");
    assert_v1_bytes(
        &created.raw,
        &V1Golden {
            id: 1,
            raw: ROUTER_CREATE_FALLTHROUGH,
            frame: Frame::Result(json!({
                "content": [{
                    "type": "text",
                    "text": "{\"taskId\":\"tool-fabricated\",\"status\":\"working\",\
                             \"createdAt\":\"2026-06-21T00:00:00Z\",\
                             \"lastUpdatedAt\":\"2026-06-21T00:00:00Z\"}",
                }],
                "isError": false,
            })),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Self-tests for the normalizer.
//
// The normalizer is the one piece of machinery standing between a raw response
// and the golden literal, so it gets its own falsifiability guards: if it
// silently dropped a key or changed a length, every fixture above would keep
// passing while proving less.
// ===========================================================================

#[tokio::test]
async fn v1_tasks_golden_normalizer_preserves_width_and_every_key() {
    let raw = r#"{"taskId":"f28148d5-3039-4437-a871-eca80f36d721","createdAt":"2026-07-28T05:44:44.456710+00:00","ttl":null}"#;
    let same_width = substitute(raw, &[TASK_ID, CREATED_AT], true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the width-preserving substitution must not change the byte count"
    );
    assert_eq!(key_occurrences(&same_width, "taskId"), 1);
    assert_eq!(key_occurrences(&same_width, "createdAt"), 1);
    assert!(
        same_width.contains(r#""ttl":null"#),
        "an explicit null must survive normalization untouched: {same_width}"
    );

    let canonical = substitute(raw, &[TASK_ID, CREATED_AT], false);
    assert_eq!(
        canonical,
        r#"{"taskId":"<TASK-ID>","createdAt":"<TIMESTAMP>","ttl":null}"#
    );
}

#[tokio::test]
#[should_panic(expected = "does not appear in the response")]
async fn v1_tasks_golden_normalizer_rejects_an_absent_dynamic_key() {
    // A response missing a declared dynamic key must FAIL loudly. Silently
    // normalizing nothing is how a golden goes quiet.
    let _ = substitute(r#"{"status":"working"}"#, &[TASK_ID], false);
}
