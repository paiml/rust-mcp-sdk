//! Shared task-lifecycle dispatch unit used by BOTH `Server` and `ServerCore`.
//!
//! Phase 101 landed the complete `tasks/*` lifecycle on `ServerCore` /
//! `ServerCoreBuilder` only. Phase 102 extracts that machinery into ONE shared
//! place (this module — research Option A) so the high-level `Server` / HTTP
//! dispatcher can serve the same lifecycle without re-implementing it (drift).
//!
//! This module hosts:
//! - [`apply_tasks_capability_rule`] — the endpoint-backed `tasks`-capability
//!   rule, a free function over explicit params (the two builders hold
//!   `tool_infos` at different lifecycle points, so it cannot be a method).
//! - [`default_tasks_capability`] — the FROZEN advertised `ServerTasksCapability`
//!   shape (do not re-derive its JSON).
//! - [`TaskDispatch`] — a borrow-struct over `(&task_store, &task_router)` that
//!   owns owner-resolution, the create-path response (with the self-enforcing
//!   create gate), `tasks/result` precedence, and `tasks/get|list|cancel`
//!   routing.
//! - [`success_response`] / [`error_response`] — the SINGLE-SOURCE JSON-RPC
//!   envelope builders (`ServerCore` delegates to these; there is exactly one
//!   copy of the wrapping logic).
//!
//! The ENTIRE module is gated `#[cfg(not(target_arch = "wasm32"))]` because every
//! task item is non-wasm (mirrors `ServerCore`'s task fields/methods).

#![cfg(not(target_arch = "wasm32"))]
// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution (mirrors intent, keeps the API surface crate-private).
#![allow(clippy::redundant_pub_crate)]

use crate::error::{Error, Result};
use crate::server::auth::AuthContext;
use crate::server::core::DispatchEnvelopeClaim;
use crate::server::task_store::{TaskInputSnapshot, TaskStore, TaskStoreError};
use crate::server::tasks::TaskRouter;
use crate::types::capabilities::{
    ServerCapabilities, ServerTasksCapability, TasksExtensionCapability, TASKS_EXTENSION_KEY,
};
use crate::types::jsonrpc::ResponsePayload;
// The ONE `"tasks/update"` spelling in the crate. It lives in `types::mrtr`
// because that module owns `TASK_NAME_BEARING_METHODS`, the routing table this
// module's params gate resolves through; `types::protocol` re-exports the same
// item for the classifier. It is deliberately NOT a row of `MRTR_METHODS` — see
// that table's rustdoc, and `tests/v2_tasks_update_routing.rs` for the two
// independent guards that keep it out.
use crate::types::mrtr::TASKS_UPDATE_METHOD;
// The `tasks/update` delivery reads its bounds, its kind-directed decoder and its
// refusal vocabulary from `types::mrtr` — never from local re-definitions. That
// module owns them because the SAME four bounds and the SAME decode discipline
// already guard the MRTR ingress, and two servers' worth of limits on one process
// is how the halves of a bound drift apart (plan 114-14).
use crate::types::mrtr::{
    check_input_responses_map_bounds, InputResponse, InputResponseTypingError, InputResponses,
    INPUT_RESPONSES_KEY,
};
use crate::types::tasks::{
    DetailedTaskV2, Task, TaskDetailV2, TaskStatus, TaskV2, RELATED_TASK_META_KEY,
};
use crate::types::tools::TaskSupport;
use crate::types::{
    CallToolResult, ClientRequest, Content, JSONRPCError, JSONRPCResponse, RequestId, ToolInfo,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// `tasks/list` — RETIRED on protocol version 2026-07-28.
///
/// Spelled here rather than in `crate::types::mrtr`: that module's method
/// constants exist to key the routing-NAME table, and a retired method has no
/// row there. This constant exists so an era gate and its refusal message
/// cannot disagree about the spelling.
const TASKS_LIST_METHOD: &str = "tasks/list";

/// `tasks/result` — RETIRED on protocol version 2026-07-28.
///
/// See [`TASKS_LIST_METHOD`].
const TASKS_RESULT_METHOD: &str = "tasks/result";

/// The `-32601` message body for a `tasks/*` method protocol version 2026-07-28
/// RETIRED.
///
/// Emitted as `format!("{method} {V2_TASKS_METHOD_RETIRED}")` so the caller is
/// told WHICH method it asked for as well as why the answer is
/// `METHOD_NOT_FOUND`; the two gates share one builder (`retired_on_v2`) so
/// they cannot drift into two different sentences for one condition.
///
/// # Provenance
///
/// The vendored draft extension schema — `schema/vendored/ext-tasks/schema.ts`
/// at the commit pinned by `schema/vendored/ext-tasks/PROVENANCE.md` (plan
/// 114-01) — declares exactly THREE `tasks/*` request methods: `tasks/get`,
/// `tasks/update` and `tasks/cancel`. `tasks/list` and `tasks/result` are
/// ABSENT from it. They are not "unimplemented here"; they do not exist on that
/// protocol version:
///
/// * `tasks/list` was removed as a SECURITY improvement — with no enumeration
///   primitive a server cannot inadvertently leak the existence of one caller's
///   tasks to another. TASK-03 and TASK-05 are that one improvement seen from
///   two angles.
/// * `tasks/result` was removed because the v2 `tasks/get` inlines `result` /
///   `error` on the terminal task, so a second round trip has nothing left to
///   do (plan 114-11).
///
/// # Why this constant REPLACED `V2_TASKS_NOT_NEGOTIATED`
///
/// Until this plan the v2 `tasks/result` refusal read "the tasks extension is
/// not negotiated". That sentence was true only while pmcp advertised no entry
/// under [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY).
/// Plan 114-05 made [`apply_tasks_capability_rule`] advertise it on every
/// backend-configured server, at which point the message told the caller to fix
/// a negotiation that had already succeeded. A refusal message is the ONLY
/// signal a caller has for choosing its next move, so an untruthful one makes
/// the correct fix undiscoverable (T-114-33).
///
/// The old constant is GONE rather than reworded-and-kept: a second, unreachable
/// spelling of "no" is how two plans come to disagree about one wire string.
///
/// The server-side "the client did not declare the extension" refusal DOES now
/// exist, and it is deliberately not this constant: plans 114-12 and 114-13 gate
/// on `TaskDispatch::declares_tasks_extension` and answer `-32021` carrying
/// `MISSING_TASKS_DECLARATION_MESSAGE`. A missing DECLARATION and a RETIRED
/// method are opposite fixes — declare the extension versus stop calling the
/// method — so they stay two sentences. (This paragraph previously asserted that
/// no such gate existed; it was falsified one wave later, which is why the
/// conditions below are no longer counted in the heading.)
///
/// # The `-32601` conditions this module answers, kept distinguishable
///
/// | condition | message | when |
/// |-----------|---------|------|
/// | RETIRED | this constant, prefixed by the method | era is v2 AND a task backend exists |
/// | NO BACKEND | `TASKS_NOT_ENABLED` / `TASKS_RESULT_NOT_SUPPORTED` | neither a `TaskStore` nor a `TaskRouter`, on ANY era |
/// | NOT A `tasks/*` METHOD | `NOT_A_TASKS_METHOD` | the wildcard arm of `route_tasks_endpoint` |
/// | NOT YET A METHOD | `V1_TASKS_UPDATE_ABSENT`, prefixed by the method | `tasks/update` on v1 — the OPPOSITE direction from RETIRED |
///
/// Distinguishability is the mitigation, not a nicety: "this method was
/// retired", "this method does not exist yet" and "this server serves no tasks
/// at all" call for three different fixes.
///
/// # This is one of TWO homes for "does this method exist on this era"
///
/// The other is `V2_RETIRED_METHODS` in
/// [`crate::server::streamable_http_server`], which retires methods removed from
/// the CORE schema by matching the method STRING at the HTTP ingress. This one
/// covers EXTENSION-scoped methods and keys on a typed `ClientRequest` arm at
/// the shared dispatch layer. Both answer `METHOD_NOT_FOUND` and both map to
/// 404 — the same rule in two vocabularies, at two layers.
///
/// A new retirement belongs here if the method is extension-scoped, and in the
/// transport table if it was removed from the core `schema.ts` inventory. See
/// that table's rustdoc for why the two are not yet unified.
pub(crate) const V2_TASKS_METHOD_RETIRED: &str =
    "is not a method of the tasks extension on protocol version 2026-07-28: the extension \
     declares only tasks/get, tasks/update and tasks/cancel";

/// The owner a v1 task request from an UNAUTHENTICATED caller is bound to —
/// FROZEN.
///
/// A shared bucket: every unauthenticated caller on the server lands in it, and
/// `tests/v1_tasks_golden.rs` pins the resulting wire bytes. Spelled once so the
/// binding and the D-10 migration warn that names it cannot disagree.
///
/// It is a DIFFERENT key from v2's
/// [`ANONYMOUS_PRINCIPAL`](crate::server::core::ANONYMOUS_PRINCIPAL) (`""`).
/// `GenericTaskStore::is_anonymous_owner` treats the two IDENTICALLY for the
/// `allow_anonymous` decision, but `make_key` prefixes every record by owner, so
/// they are DISJOINT key spaces: a task created on v1 by an unauthenticated
/// caller is not reachable by an unauthenticated v2 caller on the same
/// no-auth-provider server, and vice versa. Those two facts are easy to
/// conflate; they are separate.
const V1_UNAUTHENTICATED_OWNER: &str = "local";

/// The FROZEN `-32601` message for a `tasks/*` method on a server with no task
/// backend at all.
///
/// Spelled once because two sites emit it: the per-endpoint handlers'
/// no-backend `else` arms. It is deliberately DIFFERENT from
/// [`V2_TASKS_METHOD_RETIRED`] — "this method was retired" and "this server
/// serves no tasks at all" call for opposite fixes (T-114-33).
const TASKS_NOT_ENABLED: &str = "Tasks not enabled";

/// The FROZEN `-32601` message `tasks/result` uses for the same no-backend
/// condition [`TASKS_NOT_ENABLED`] covers for the other three methods.
///
/// Deliberately a different sentence from its three siblings: `tests/…` and
/// `the_minus_32601_conditions_are_mutually_distinct` assert all four refusals
/// pairwise distinct, so a caller can always tell which one it hit.
const TASKS_RESULT_NOT_SUPPORTED: &str = "tasks/result not supported";

/// The `-32601` message for a request that is not a `tasks/*` method at all.
const NOT_A_TASKS_METHOD: &str = "Method not supported";

/// The message a v2 task-not-found refusal carries — ONE sentence, deliberately
/// content-free (T-114-50).
///
/// Two properties, both asserted rather than asserted-about:
///
/// 1. It is IDENTICAL for an absent id, another owner's id and an expired task.
///    Owner mismatch surfaces as [`TaskStoreError::NotFound`](crate::server::task_store::TaskStoreError::NotFound)
///    ON PURPOSE — the owner-prefixed key design is what closes the existence
///    oracle — so a message that varied between those cases would re-open it,
///    and moving the code from `-32603` to `-32602` would make the oracle
///    *sharper*, not the same. Expiry is folded onto the same answer for the same
///    reason: [`TaskStoreError`](crate::server::task_store::TaskStoreError)'s own
///    `From` impl already maps `Expired` onto `not_found` "to avoid leaking
///    existence of expired tasks", and this is that rule expressed on the wire.
/// 2. It does NOT render the requested task id back. A client already knows the
///    id it sent, so echoing it buys the caller nothing and buys a log-poisoning
///    attacker a free write into the operator's logs — the discipline
///    `MrtrParseError` already established.
const V2_TASK_NOT_FOUND_MESSAGE: &str = "task not found";

/// The client-facing message for case 3 of the ordered refusal chain.
///
/// It names the extension and the channel to declare it on, because the fix is
/// entirely the caller's to make. It says NOTHING about authentication state:
/// case 3 fires before the identity table is consulted, and a message that
/// hinted at credentials would turn a negotiation refusal into an
/// authentication oracle (T-114-40).
const MISSING_TASKS_DECLARATION_MESSAGE: &str =
    "the tasks extension was not declared on this request: send \
     _meta[\"io.modelcontextprotocol/clientCapabilities\"].extensions\
     [\"io.modelcontextprotocol/tasks\"]";

/// The `-32601` suffix a **v1** caller receives for `tasks/update` — the OPPOSITE
/// direction from [`V2_TASKS_METHOD_RETIRED`], and deliberately a different
/// sentence.
///
/// `tasks/update` is a 2026-07-28-ONLY method: it has no MCP 2025-11-25 spelling
/// at all, so on v1 the honest answer is "this method does not exist yet", not
/// "this method was retired". Those two call for opposite fixes — upgrade versus
/// migrate — and the `-32601` message is the only place a caller can tell them
/// apart, which is the distinguishability rule T-114-33 already established for
/// the other three `-32601` conditions in this module.
///
/// # This ALSO records a deliberate, benign v1 response change
///
/// Before Phase 114 plan 13 a `tasks/update` POST was an unrecognised method, so
/// the streamable-HTTP transport answered a `PARSE_ERROR` at HTTP 400 with
/// `id: null`. It now answers this `-32601` at HTTP 200 carrying the ORIGINAL id.
/// That is the same change, for the same reason, that Phase 112 made for
/// `server/discover` (D-10 finding #4): no conforming v1 client sends a v2-only
/// method, so no v1-relied-upon response byte moves. `tests/v1_tasks_golden.rs`
/// pins the v1 wire and never sends this method.
const V1_TASKS_UPDATE_ABSENT: &str =
    "is not a method of protocol version 2025-11-25: it is defined only by the tasks extension \
     on protocol version 2026-07-28";

/// The `-32602` message for a `tasks/update` whose params carry no addressable
/// task.
///
/// The addressing key is NOT re-spelled here: the gate resolves it through
/// [`logical_name_of`](crate::types::mrtr::logical_name_of), which reads
/// `TASK_NAME_BEARING_METHODS` — the SAME table the `Mcp-Name` routing header
/// derives from. One answer to "where does `tasks/update` keep its task id".
///
/// It names the required field and NOTHING else: no task state, no owner, and no
/// echo of whatever the caller actually sent (the log-poisoning discipline
/// [`V2_TASK_NOT_FOUND_MESSAGE`] records).
const TASKS_UPDATE_MALFORMED_PARAMS: &str = "tasks/update requires params.taskId to be a string";

/// The `-32602` message for a `tasks/update` whose `inputResponses` is absent or
/// is not a JSON object.
///
/// The vendored draft schema types `UpdateTaskRequest.params.inputResponses` as a
/// REQUIRED `InputResponses`, so an absent key and a non-object one are the same
/// defect from the caller's point of view and get one sentence. It names the
/// required field and NOTHING else — no state, no owner, no echo of what arrived
/// (the log-poisoning discipline [`V2_TASK_NOT_FOUND_MESSAGE`] records).
///
/// Deliberately a DIFFERENT sentence from [`TASKS_UPDATE_MALFORMED_PARAMS`]:
/// "your task id is not a string" and "your responses map is missing" are
/// different fixes, and the `-32602` message is the only place a caller can tell
/// them apart — the distinguishability rule T-114-33 established for the `-32601`
/// family, applied to this route's two params refusals.
const TASKS_UPDATE_MISSING_INPUT_RESPONSES: &str =
    "tasks/update requires params.inputResponses to be an object";

/// Build the `-32601` a v2 caller receives for a RETIRED `tasks/*` method.
///
/// The SINGLE builder both era gates use, so `tasks/list` and `tasks/result`
/// answer one condition with one sentence.
fn retired_on_v2(id: RequestId, method: &str) -> JSONRPCResponse {
    error_response(
        id,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        format!("{method} {V2_TASKS_METHOD_RETIRED}"),
    )
}

/// Does this request run under the v1 task lifecycle?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | the v1 task lifecycle is untouched |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | this request runs under the v2 task surface, which answers separately |
///
/// The v2 row deliberately no longer says "and not negotiated": since 114-05 a
/// tasks-backed server DOES advertise the tasks extension, so the reason this
/// predicate routes v2 away from the `-32002` refusal is not a missing
/// capability entry.
///
/// Nor does it still say "the v2 task surface is not implemented" — plans
/// 114-11, 114-12 and 114-13 implemented it (v2 result shapes, the
/// `V2ClientDeclaration` create trigger, and `tasks/update` routing). `false`
/// here means "answer this on the v2 path", not "there is nothing to answer
/// with".
///
/// # Why this predicate exists (Finding 11)
///
/// The `tasks/result` pending refusal emits
/// [`V1_TASK_PENDING`](crate::types::protocol::error_codes::V1_TASK_PENDING)
/// (`-32002`), which protocol version 2026-07-28 **MUST NOT** emit
/// (`docs/specification/draft/basic/index.mdx` § Error Codes). That site *looked*
/// v1-scoped — this module contains no era gating at all — but
/// `tests/v2_prohibited_error_codes.rs` drove a real v2 HTTP `tasks/result` at it
/// and read `-32002` off the response. It is reachable because the HTTP ingress
/// resolves the era from `params._meta` on the RAW body, so a `tasks/result`
/// arrives classified v2 even though the typed
/// [`GetTaskPayloadRequest`](crate::types::tasks::GetTaskPayloadRequest) has no
/// `_meta` field for it to ride on.
///
/// # What this predicate gates, and what it deliberately does NOT
///
/// It is the ONE era definition this module has, and every era-sensitive site
/// here delegates to it rather than re-deriving the answer — the `-32002`
/// pending emission, [`tasks_list_serves_on_era`] and
/// [`tasks_result_serves_on_era`] (both because plan 114-08 RETIRED those
/// methods on v2 — see [`V2_TASKS_METHOD_RETIRED`]), and the per-route v2 shape
/// and gate decisions added by plans 114-11 through 114-13.
///
/// That list is deliberately NOT written as a count. It was "three things" until
/// three more readers arrived in the following three waves, and a number in a
/// doc comment rots silently while a delegation rule does not.
///
/// It does NOT retire `tasks/get` or `tasks/cancel`: both still serve on BOTH
/// eras, because both survive in the v2 extension schema. Their v2 response
/// SHAPE changes (plan 114-11 flattens the result and remaps not-found), but a
/// shape change is not a retirement and this predicate must not be widened into
/// one.
///
/// This block previously claimed the predicate gated only the `-32002` emission
/// and that `tasks/list` was unchanged on every era. Both sentences were
/// falsified by plan 114-08 and are rewritten in the same commit that falsified
/// them: a stale "deliberately does NOT do X" comment actively misleads the next
/// reader, which is the failure class 113-29 recorded.
pub(crate) const fn is_v1_task_era(era: Option<crate::types::protocol::Era>) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2))
}

/// Does `tasks/list` serve on this era?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | v1 enumerates a caller's tasks exactly as it always has |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | `tasks/list` is ABSENT from the tasks extension — [`V2_TASKS_METHOD_RETIRED`] |
///
/// # Why this is its own predicate rather than a shared boolean
///
/// [`tasks_result_serves_on_era`] answers the same question for the other
/// retired method and currently returns the same value. They are deliberately
/// two functions: a negative control that disables ONE gate must fail ONLY that
/// gate's probe, which is the orthogonality discipline 113-29 established and
/// which a single shared boolean makes impossible.
///
/// The era answer itself is NOT re-derived here — it delegates to
/// [`is_v1_task_era`] — so the file still has exactly one definition of "which
/// era is this".
///
/// # This predicate alone does not decide the refusal
///
/// A `false` here means "not on this era"; the caller ALSO checks
/// `TaskDispatch::has_task_backend`, because a server with no task backend must
/// keep its existing "not enabled" answer rather than claim a retirement.
pub(crate) const fn tasks_list_serves_on_era(era: Option<crate::types::protocol::Era>) -> bool {
    is_v1_task_era(era)
}

/// Does `tasks/result` serve on this era?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | v1 serves the terminal payload, including the FROZEN `-32002` pending refusal |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | `tasks/result` is ABSENT from the tasks extension — [`V2_TASKS_METHOD_RETIRED`] |
///
/// Retiring the method on v2 also removes the LAST v2-reachable emission path
/// for `V1_TASK_PENDING` (`-32002`), the code protocol version 2026-07-28 MUST
/// NOT emit: `tests/v2_prohibited_error_codes.rs` proved that path reachable
/// over a real v2 HTTP request, and the gate now returns before the store is
/// ever consulted.
///
/// Separate from [`tasks_list_serves_on_era`] for the orthogonality reason
/// documented there.
pub(crate) const fn tasks_result_serves_on_era(era: Option<crate::types::protocol::Era>) -> bool {
    is_v1_task_era(era)
}

/// Build the default server-level `tasks` capability advertised when a task
/// backend (a [`TaskStore`] or a [`TaskRouter`]) is present.
///
/// This is the exact FROZEN [`ServerTasksCapability`] shape the client
/// `assert_capability` expects; it must not be hand-rolled at any call site.
/// Both [`apply_tasks_capability_rule`] and `ServerCoreBuilder` use this single
/// definition so the advertised capability shape can never drift.
pub(crate) fn default_tasks_capability() -> ServerTasksCapability {
    ServerTasksCapability {
        list: Some(serde_json::json!({})),
        cancel: Some(serde_json::json!({})),
        requests: Some(crate::types::capabilities::ServerTasksRequestCapability {
            tools: Some(crate::types::capabilities::ServerTasksToolsCapability {
                call: Some(serde_json::json!({})),
            }),
        }),
    }
}

/// The value pmcp auto-advertises under
/// [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY):
/// the empty object.
///
/// Built through [`TasksExtensionCapability`] rather than a bare
/// `serde_json::json!({})` so there is ONE canonical spelling of the value, in
/// the same way `TASKS_EXTENSION_KEY` gives one canonical spelling of the key.
///
/// `default_tasks_capability()`'s `list` / `cancel` / `requests.tools.call`
/// flags are deliberately NOT projected in here (D-03). Advertising
/// `list: true` on an era where `tasks/list` answers `-32601` is exactly the
/// capability lie the endpoint-backed rule exists to prevent, and the vendored
/// draft schema types this capability as `Record<string, never>` — a value
/// admitting no properties at all.
///
/// Serializing a field-less struct cannot fail; the fallback restates the SAME
/// `{}` rather than introducing a panic path.
pub(crate) fn tasks_extension_value() -> Value {
    serde_json::to_value(TasksExtensionCapability::default())
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
}

/// Apply the endpoint-backed `tasks`-capability rule (D-CAPABILITY-ENDPOINT-BACKED).
///
/// This is the SINGLE shared rule both `ServerCoreBuilder` and (Plan 02)
/// `ServerBuilder` call. It is a free function over explicit params rather than a
/// builder method because the two builders hold `tool_infos` at different
/// lifecycle points (`ServerCoreBuilder` fills it at `.tool()`; `ServerBuilder`
/// builds it locally inside `build()`).
///
/// The `tasks` capability advertised in `initialize` represents REAL endpoint
/// support, never tool metadata alone:
/// - It is auto-advertised only when a backend exists (`has_backend`) and the
///   author has not already configured a custom `tasks` capability (additive-only
///   — an explicit value is preserved verbatim).
/// - A tool declaring [`TaskSupport::Required`] with NO backend is a build-time
///   validation error (rather than a hollow capability whose `tasks/*` endpoints
///   cannot work).
/// - An `Optional`/`Forbidden` task tool with no backend is NOT an error and does
///   NOT by itself trigger advertisement.
///
/// # ONE knob, TWO eras (plan 114-05, D-01)
///
/// The same `has_backend` fact drives BOTH advertisements:
///
/// | era | where it lands | value |
/// |-----|----------------|-------|
/// | MCP 2025-11-25 | `capabilities.tasks` | [`default_tasks_capability()`] |
/// | MCP 2026-07-28 | `capabilities.extensions["io.modelcontextprotocol/tasks"]` | `{}` ([`tasks_extension_value()`]) |
///
/// So no existing tasks server needs a code change to serve a v2 client, and no
/// v2 server with a working store can silently serve nothing. Both writes are
/// ADDITIVE in both directions: an explicitly configured value — `tasks` or the
/// extensions entry — is preserved VERBATIM, an absent `extensions` map is
/// created, and an existing one gains the entry alongside its other keys without
/// disturbing them.
///
/// This rule runs at BUILD time, where no era exists. Era-awareness is NOT its
/// job: the struct carries everything both eras could want, and the
/// serialization boundary decides what each era SEES
/// (`core::discover_result_from_capabilities` for v2 `server/discover`). That
/// split is D-02, and collapsing it — making this rule era-conditional — is what
/// would move v1 `initialize` bytes.
///
/// # Errors
///
/// Returns a validation error if any registered tool declares
/// [`TaskSupport::Required`] but no `TaskStore` or `TaskRouter` backs the
/// `tasks/*` endpoints.
pub(crate) fn apply_tasks_capability_rule(
    capabilities: &mut ServerCapabilities,
    tool_infos: &HashMap<String, ToolInfo>,
    has_backend: bool,
) -> Result<()> {
    let has_required_task_tool = tool_infos.values().any(|info| {
        info.execution
            .as_ref()
            .and_then(|e| e.task_support)
            .is_some_and(|ts| matches!(ts, TaskSupport::Required))
    });

    if has_required_task_tool && !has_backend {
        return Err(Error::validation(
            "a tool declares TaskSupport::Required but no TaskStore or TaskRouter \
             is configured to back the tasks/* endpoints",
        ));
    }

    if capabilities.tasks.is_none() && has_backend {
        capabilities.tasks = Some(default_tasks_capability());
    }

    // The v2 arm of the SAME endpoint-backed rule. `entry(..).or_insert_with(..)`
    // is the additive-only discipline the `tasks.is_none()` guard above expresses
    // for v1: an operator-configured value is never overwritten.
    if has_backend {
        capabilities
            .extensions
            .get_or_insert_with(HashMap::new)
            .entry(TASKS_EXTENSION_KEY.to_string())
            .or_insert_with(tasks_extension_value);
    }

    Ok(())
}

/// Create a success JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::success_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Result(result),
    }
}

/// Create an error JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::error_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Error(JSONRPCError {
            code,
            message,
            data: None,
        }),
    }
}

/// Resolution of a [`ToolHandler`](crate::server::ToolHandler)'s
/// [`ToolOutput`](crate::server::ToolOutput) at a NATIVE dispatch tail.
///
/// This is the SINGLE place (D-05 anti-drift) where the `Payload`-vs-`Result`
/// decision AND the response-middleware-bypass rule live. BOTH native dispatchers
/// (`Server::handle_call_tool` and `ServerCore::handle_call_tool`) resolve their
/// handler's `Result<ToolOutput>` through [`resolve_tool_output`] and branch on
/// this enum identically, so the two dispatchers can never drift on the rule.
pub(crate) enum DispatchOutput {
    /// `ToolOutput::Result` — send this `CallToolResult` to the wire VERBATIM.
    ///
    /// The dispatcher must BYPASS response middleware, the create-path gate, and
    /// text-wrap / widget enrichment for this arm (D-04 + D-04a, USER-APPROVED and
    /// LOCKED — the handler owns the full envelope, including its own redaction).
    /// REQUEST middleware and the handler-error path are unaffected: they run
    /// before this resolution, so only the SUCCESSFUL `Result` arm is verbatim.
    Verbatim(CallToolResult),

    /// `ToolOutput::Payload(v)` OR a handler `Err(_)` — coerced back into the
    /// existing `Result<Value>` middleware variable and fed through the UNCHANGED
    /// tail: response middleware, `handle_tool_error`, the create-path gate, and
    /// the text-wrap / widget enrichment, exactly as before this feature existed.
    Middleware(Result<Value>),
}

/// Build the `-32003` a caller receives for case 4 of the ordered refusal chain.
///
/// The message shape is `subscriptions/listen`'s verbatim, because it is the
/// same condition on the same server and a caller that hits both should not have
/// to learn two sentences for it.
///
/// It deliberately answers at HTTP **200** with a JSON-RPC error body:
/// [`AUTHENTICATION_REQUIRED`](crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED)
/// is not in `v2_status_for_code`'s 400 arm, and putting it there would change
/// the status of every other emitter of that code across the transport
/// (T-114-43). The transport file is untouched by this plan.
///
/// It is not an authentication ORACLE: it fires only for a method that EXISTS on
/// a server that ADVERTISES it, so all it reveals is "this server wants
/// authentication" — already public from the server's `WWW-Authenticate` posture
/// (T-114-40).
/// Build the `-32021` a caller receives for case 3 of the ordered refusal chain:
/// it never declared the tasks extension on this request.
///
/// The code is read from the NAMED constant
/// [`MISSING_REQUIRED_CLIENT_CAPABILITY`](crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY)
/// and is never spelled as a numeric literal anywhere in this module. Research
/// measured that the ext-tasks PROSE uses `-32003` for a missing client
/// capability while the core draft SCHEMA uses `-32021`; DQ3 locked `-32021`.
/// Reading the value from one named constant at the single emission site is what
/// makes a schema-driven change at the D-18 gate a one-line edit rather than an
/// archaeology exercise.
///
/// `error.data.requiredCapabilities` is a `ClientCapabilities` **OBJECT**
/// (`{"extensions":{"io.modelcontextprotocol/tasks":{}}}`), never an array —
/// emitting an array is a wire-contract violation the official conformance suite
/// grades. It is built by SERIALIZING the real capability types rather than by
/// hand-writing the JSON, so the key and the empty-object value cannot drift
/// from [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY).
///
/// The payload carries the required capabilities and NOTHING else — no server
/// configuration, no task state, no hint about authentication (T-114-41).
fn missing_tasks_declaration_refusal(id: RequestId) -> JSONRPCResponse {
    let mut extensions = std::collections::HashMap::new();
    extensions.insert(
        crate::types::capabilities::TASKS_EXTENSION_KEY.to_string(),
        serde_json::to_value(crate::types::capabilities::TasksExtensionCapability::default())
            .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
    );
    let required = crate::types::ClientCapabilities {
        extensions: Some(extensions),
        ..Default::default()
    };

    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Error(JSONRPCError {
            code: crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            message: MISSING_TASKS_DECLARATION_MESSAGE.to_string(),
            data: Some(serde_json::json!({
                "requiredCapabilities": serde_json::to_value(&required)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
            })),
        }),
    }
}

pub(crate) fn authentication_required(id: RequestId, method: &str) -> JSONRPCResponse {
    error_response(
        id,
        crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
        format!("{method} requires an authenticated caller on this server"),
    )
}

// ===========================================================================
// The v2 result surface: store-error mapping and the flat shape projections.
//
// Everything below sits ABOVE the `serde_json::Value` seam. No `TaskStore` and
// no `crates/pmcp-tasks` backend changes to serve v2 (D-11) — which is what
// makes DynamoDB/Redis-backed tasks work on v2 from day one.
// ===========================================================================

/// Map a [`TaskStoreError`] onto a JSON-RPC error response, ERA-AWARE.
///
/// # v1 — FROZEN
///
/// EVERY error becomes `-32603` carrying `error.to_string()`, byte-for-byte what
/// it has always been. `tests/v1_tasks_golden.rs` is the gate.
///
/// # v2 — the extension's own not-found code
///
/// [`TaskStoreError::NotFound`] and [`TaskStoreError::Expired`] become `-32602`
/// (`INVALID_PARAMS`) carrying [`V2_TASK_NOT_FOUND_MESSAGE`]; every other variant
/// stays `-32603` with its own message. The extension's error-handling section
/// makes `-32602` a MUST for `tasks/get` and a SHOULD for `tasks/update` and
/// `tasks/cancel` (inventory row 29).
///
/// `Expired` is folded onto the SAME answer as `NotFound` deliberately: the
/// anti-oracle constraint on row 29 names absent / wrong-owner / expired
/// together, and `TaskStoreError`'s own `From<TaskStoreError> for Error` already
/// maps `Expired` onto `not_found` for exactly that reason. Splitting them here
/// would tell a caller "that id existed until recently", which is precisely the
/// fact the owner-prefixed key design refuses to disclose.
///
/// # This is NOT the frozen `-32002` question
///
/// [`V1_TASK_PENDING`](crate::types::protocol::error_codes::V1_TASK_PENDING) is
/// pmcp's *resource*-not-found / task-pending squat, which the ROADMAP forbids
/// re-litigating. This function does not read it, does not emit it and does not
/// change it; `-32602` here is the tasks extension's own independent allocation
/// for *task*-not-found on v2.
fn store_error_response(
    id: RequestId,
    error: &TaskStoreError,
    era: Option<crate::types::protocol::Era>,
) -> JSONRPCResponse {
    if is_v1_task_era(era) {
        return error_response(
            id,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            error.to_string(),
        );
    }
    match error {
        TaskStoreError::NotFound { .. } | TaskStoreError::Expired { .. } => error_response(
            id,
            crate::types::protocol::error_codes::INVALID_PARAMS,
            V2_TASK_NOT_FOUND_MESSAGE.to_string(),
        ),
        TaskStoreError::InvalidTransition { .. } | TaskStoreError::Internal { .. } => {
            error_response(
                id,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                error.to_string(),
            )
        },
    }
}

/// A JSON value as an owned object map, or `None` when it is not an object.
fn as_object(value: Value) -> Option<serde_json::Map<String, Value>> {
    match value {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// The v2 `_meta.relatedTask` envelope every create response carries.
///
/// Its key is a KNOWN property of `CreateTaskResult._meta` in the vendored
/// schema, so it stays on v2 rather than being dropped with the `task` wrapper.
fn related_task_meta(task_id: &str) -> Value {
    serde_json::json!({ RELATED_TASK_META_KEY: { "taskId": task_id } })
}

/// The v2 create body: a FLAT `Result & Task` (`resultType: "task"` is supplied
/// by [`DispatchEnvelopeClaim::TASK_CREATED`], not written here).
///
/// The envelope discriminator is deliberately NOT hand-written into this object:
/// `own_reserved_result_fields` OWNS `resultType` and overwrites whatever a
/// producer put there, so writing it here would be a value that silently never
/// reaches the wire. The claim is the only way to state it.
fn v2_create_result_value(task: &Task, store_id: &str) -> Value {
    let mut object = as_object(serde_json::to_value(TaskV2::from_v1(task)).unwrap_or_default())
        .unwrap_or_default();
    object.insert("_meta".to_string(), related_task_meta(store_id));
    Value::Object(object)
}

/// The v1 create body: the FROZEN nested `{ "task": …, "_meta": … }` envelope.
fn v1_create_result_value(task: &Task, store_id: &str) -> Value {
    let create_result = crate::types::tasks::CreateTaskResult::new(task.clone());
    let mut envelope = serde_json::to_value(create_result).unwrap_or_default();
    if let Some(object) = envelope.as_object_mut() {
        object.insert("_meta".to_string(), related_task_meta(store_id));
    }
    envelope
}

/// Project a `TaskRouter`'s `tasks/get` `Value` into the v2 flat shape.
///
/// A router is out-of-tree code returning an untyped `Value`, so this reads the
/// SAME two things the store path reads — the task body and its status detail —
/// from wherever the router put them, and passes the value through UNCHANGED
/// when it cannot be understood. Silently emitting a half-projected object would
/// be worse than emitting the router's own shape and saying so.
fn v2_project_router_task(value: Value) -> Value {
    let Some(object) = value.as_object() else {
        return value;
    };
    // A router may return the v1 nested `{"task": …}` or a bare task body.
    let body = object.get("task").unwrap_or(&value);
    let Ok(task) = serde_json::from_value::<Task>(body.clone()) else {
        tracing::warn!(
            target: "mcp.tasks",
            "a TaskRouter returned a tasks/get value that is not a Task; passing it through \
             unprojected on protocol version 2026-07-28"
        );
        return value;
    };
    // The detail keys may sit at the top level or inside the nested body.
    let detail_source = |key: &str| -> Option<Value> {
        object
            .get(key)
            .or_else(|| body.as_object().and_then(|inner| inner.get(key)))
            .cloned()
    };
    let detail = match task.status {
        TaskStatus::Working => Some(TaskDetailV2::Working),
        TaskStatus::Cancelled => Some(TaskDetailV2::Cancelled),
        TaskStatus::InputRequired => detail_source(crate::types::tasks::DETAIL_KEY_INPUT_REQUESTS)
            .and_then(|v| serde_json::from_value(v).ok())
            .map(|input_requests| TaskDetailV2::InputRequired { input_requests }),
        TaskStatus::Completed => detail_source(crate::types::tasks::DETAIL_KEY_RESULT)
            .and_then(as_object)
            .map(|result| TaskDetailV2::Completed { result }),
        TaskStatus::Failed => detail_source(crate::types::tasks::DETAIL_KEY_ERROR)
            .and_then(as_object)
            .map(|error| TaskDetailV2::Failed { error }),
    };
    v2_detailed_task_value(&task, detail)
}

/// The v2 `tasks/get` body for `task`.
///
/// With a `detail` the shape is the full `DetailedTask` variant. WITHOUT one —
/// a backend that cannot supply the terminal result, the stored error or the
/// recorded input requests — the shape degrades to the bare flat `Task` rather
/// than inventing an empty field: an `inputRequests: {}` on an `input_required`
/// task is a schema-VALID lie, and a client that trusted it would wait forever
/// for requests it was told there were none of.
fn v2_detailed_task_value(task: &Task, detail: Option<TaskDetailV2>) -> Value {
    if let Some(detail) = detail {
        return Value::Object(DetailedTaskV2::new(TaskV2::from_v1(task), detail).to_wire_object());
    }
    tracing::warn!(
        target: "mcp.tasks",
        status = %task.status,
        "no backend could supply this task's status detail; emitting the bare flat Task \
         rather than an empty required field"
    );
    serde_json::to_value(TaskV2::from_v1(task)).unwrap_or_default()
}

/// Resolve a handler's `Result<ToolOutput>` into the shared [`DispatchOutput`]
/// decision (D-05: one copy of the Payload-vs-Result + bypass rule).
///
/// - `Ok(ToolOutput::Result(r))` → [`DispatchOutput::Verbatim`] (wire-verbatim,
///   bypasses RESPONSE middleware + create-path + wrap);
/// - `Ok(ToolOutput::Payload(v))` → [`DispatchOutput::Middleware(Ok(v))`];
/// - `Err(e)` → [`DispatchOutput::Middleware(Err(e))`] (handler errors STILL flow
///   through `process_response` / `handle_tool_error` — the bypass is scoped to
///   the `Ok(Result(_))` arm only).
///
/// Matching a `#[non_exhaustive]` enum from WITHIN the defining crate is exhaustive
/// (the attribute only constrains downstream crates), so no wildcard arm is needed.
// Why: called by both native dispatch tails (mod.rs + core.rs handle_call_tool);
// production-reachable, no dead_code allow needed.
pub(crate) fn resolve_tool_output(output: Result<crate::server::ToolOutput>) -> DispatchOutput {
    match output {
        Ok(crate::server::ToolOutput::Result(call_result)) => DispatchOutput::Verbatim(call_result),
        Ok(crate::server::ToolOutput::Payload(value)) => DispatchOutput::Middleware(Ok(value)),
        Err(e) => DispatchOutput::Middleware(Err(e)),
    }
}

/// Which high-precision structural marker tripped the double-wrap detector.
///
/// Reported in the TOUT-02 WARN / `debug_assert!` so an author immediately sees
/// WHY a `Value` about to be text-wrapped looked like an already-built
/// [`CallToolResult`]. `Copy` (two field-less variants); it never escapes the
/// server crate (exposed to integration tests only via the hidden
/// `pmcp::__test_support` seam, mirroring `ServerRequestDispatcher`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleWrapMarker {
    /// The value carries a `_meta` object holding [`RELATED_TASK_META_KEY`] — the
    /// envelope key only a built task-augmented `CallToolResult` sets.
    RelatedTaskMeta,
    /// The value is a `CallToolResult`-envelope-shaped object (ONLY envelope
    /// keys: `content`/`isError`/`structuredContent`/`_meta`) carrying a
    /// NON-EMPTY `content` array whose every element deserializes as
    /// [`Content`] (the internally `#[serde(tag = "type")]` enum), i.e. it is
    /// already a wire-shaped result body.
    ContentArray,
}

/// Structural, high-precision detector for an about-to-be-double-wrapped result.
///
/// Detects "this `Value` is ALREADY a built [`CallToolResult`] and is about to
/// be WRONGLY text-wrapped a second time" (TOUT-02 — the exact silent bug
/// behind the agent-lake 2-week outage).
///
/// Returns `Some(marker)` only for a value carrying an unambiguous built-result
/// marker; `None` otherwise. Deliberately NOT a full
/// `from_value::<CallToolResult>` parse (D-02): it checks two cheap, precise
/// structural markers in cost order, so a benign tool payload almost never trips.
///
/// Precision rationale (near-zero false positives):
/// - The content-array marker only fires on a `CallToolResult` *envelope*: an
///   object whose keys are ALL envelope keys (`content`, `isError`,
///   `structuredContent`, `_meta`). A hand-built double-wrap was authored to
///   BE a `CallToolResult`, so only envelope keys accompany its `content`; a
///   chat-message-style payload (`role`, `model`, `stopReason`, ... — common
///   for tools that proxy LLM/sampling APIs) carries foreign keys and must
///   NOT trip.
/// - [`Content`] is internally tagged (`#[serde(tag = "type")]`), so an object
///   lacking a valid `"type"` NEVER deserializes as `Content` — the content-array
///   marker is high precision.
/// - An empty `content: []` is NOT a built-result marker (a benign payload can
///   carry an empty array), so it must NOT fire — hence the `!arr.is_empty()`
///   guard.
///
/// Order matters: the single-lookup `_meta` key check runs first (cheapest and
/// also short-circuits pathological large `content` arrays, T-104-03-02).
// Why: called at BOTH Payload wrap sites (mod.rs + core.rs) through
// `double_wrap_tripwire`; production-reachable, so no `dead_code` allow needed.
pub fn looks_like_call_tool_result(v: &Value) -> Option<DoubleWrapMarker> {
    /// Only these `CallToolResult` wire keys may accompany the `content` array
    /// for the envelope-shaped marker to fire (WR-02 precision fix).
    const RESULT_ENVELOPE_KEYS: [&str; 4] = ["content", "isError", "structuredContent", "_meta"];

    let obj = v.as_object()?;
    // Cheapest first: the task-envelope meta key — a single map lookup.
    if obj
        .get("_meta")
        .and_then(Value::as_object)
        .is_some_and(|meta| meta.contains_key(RELATED_TASK_META_KEY))
    {
        return Some(DoubleWrapMarker::RelatedTaskMeta);
    }
    // An envelope-shaped object (only `CallToolResult` keys) with a NON-EMPTY
    // `content` array whose every element parses as `Content`. The
    // `!arr.is_empty()` guard keeps a benign empty array from firing; the
    // envelope-keys guard keeps chat-message payloads from firing.
    if obj
        .keys()
        .all(|k| RESULT_ENVELOPE_KEYS.contains(&k.as_str()))
        && obj
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|arr| {
                !arr.is_empty()
                    && arr
                        .iter()
                        .all(|e| serde_json::from_value::<Content>(e.clone()).is_ok())
            })
    {
        return Some(DoubleWrapMarker::ContentArray);
    }
    None
}

/// The TOUT-02 double-wrap tripwire decision function.
///
/// The SINGLE decision fn both Payload wrap sites (`Server::handle_call_tool`
/// in mod.rs and `ServerCore::handle_call_tool` in core.rs) call BEFORE
/// stringifying a produced `Value` into a `CallToolResult`'s text content.
///
/// Behavior:
/// - `suppressed == true` → returns `None`, emits NOTHING (the tool opted out of
///   the check via `suppress_double_wrap_check`; D-08).
/// - otherwise, if [`looks_like_call_tool_result`] returns `Some(marker)`:
///   emits a `tracing::warn!` (EVERY build) AND `debug_assert!(false, ..)`
///   (debug/CI builds hard-fail; D-06: release compiles the assert out and NEVER
///   panics), then returns `Some(marker)`.
/// - benign value → returns `None`, emits nothing.
///
/// Returning the `Option<DoubleWrapMarker>` makes the decision unit-testable in
/// isolation: a RELEASE test asserts the return value (no panic), a DEBUG test
/// asserts the `debug_assert!` panic via `catch_unwind` — NEITHER spins up a
/// dispatch that the assert would abort mid-call (Codex MEDIUM: such end-to-end
/// debug-assert tests are brittle).
///
/// The identical helper is called from BOTH dispatchers, so the WARN/panic rule
/// can never drift between the high-level `Server` and `ServerCore`.
// Why: called at both Payload wrap sites (mod.rs + core.rs) guarded by the
// per-tool suppression check; production-reachable, no dead_code allow needed.
pub fn double_wrap_tripwire(
    tool_name: &str,
    value: &Value,
    suppressed: bool,
) -> Option<DoubleWrapMarker> {
    if suppressed {
        return None;
    }
    let marker = looks_like_call_tool_result(value)?;
    tracing::warn!(
        tool = %tool_name,
        ?marker,
        "value being text-wrapped structurally resembles a built CallToolResult \
         — did you mean ToolOutput::Result? (TOUT-02)"
    );
    // D-06: `debug_assert!` (NOT `assert!`) so release builds compile this out and
    // never panic in production; debug/CI builds hard-fail so the double-wrap is
    // caught by "one local run".
    debug_assert!(
        false,
        "double-wrap tripwire (TOUT-02): tool `{tool_name}` produced a value that \
         structurally resembles a built CallToolResult ({marker:?}); return \
         ToolOutput::Result to send it verbatim, or call \
         suppress_double_wrap_check(\"{tool_name}\") if this payload is legitimate"
    );
    Some(marker)
}

/// The outcome of binding a task request to a task OWNER.
///
/// A two-variant enum rather than `Option<String>` because the two answers mean
/// opposite things and one of them has to reach the wire: `None` used to mean
/// "no task backend", and reusing it for "refused" would make the fail-closed
/// row indistinguishable from a configuration fact at every call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnerBinding {
    /// The owner id every downstream store/router call is scoped to.
    Owner(String),
    /// Row 2 of the v2 identity table: an unauthenticated caller on a server
    /// that HAS an auth provider. The caller receives `-32003`
    /// [`AUTHENTICATION_REQUIRED`](crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED)
    /// and no task is read, minted or enumerated (T-114-37).
    Refused,
}

/// WHICH era's create trigger this `tools/call` is being gated by, and whether
/// it FIRED (plan 114-12, DQ1).
///
/// The two eras signal "make this a task" through completely different channels,
/// and this enum exists so a call site cannot pass the wrong one:
///
/// | era | trigger | source |
/// |-----|---------|--------|
/// | MCP 2025-11-25 (and any request carrying no era code) | the request carried a `task` field | `CallToolRequest.task` |
/// | MCP 2026-07-28 | the client DECLARED `io.modelcontextprotocol/tasks` on this request | `_meta.clientCapabilities.extensions`, already resolved onto [`ProtocolContext`](crate::types::protocol::ProtocolContext) |
///
/// # Why an enum and not a second `bool`
///
/// The alternative was `maybe_build_task_created(.., task_requested: bool,
/// client_declared: bool, ..)`. Two adjacent booleans at a call site is precisely
/// how the wrong one gets passed, and the compiler cannot tell them apart. A
/// variant carrying its own fact makes the era EXPLICIT at every call site and
/// makes "v1 consulted the declaration" a shape that does not exist.
///
/// # The v2 trigger is a CONFORMANCE requirement, not a pmcp preference
///
/// The extension states that a server MUST NOT return a `CreateTaskResult` to a
/// client that did not declare the tasks extension — a client that never
/// declared it has no rule for reading a task handle back and would break. The
/// v2 arm below IS that precondition. It is also the reason the v1 `task` field
/// is not consulted on v2: that field does not exist in the v2 extension at all,
/// and requiring it would make v2 task creation unreachable.
///
/// # It decides WHETHER, never WHO
///
/// The declaration is CLIENT-SUPPLIED and trivially forgeable. It gates only
/// whether a task is minted; the task's OWNER comes from [`TaskDispatch::resolve_owner`]
/// via the `AuthContext` identity table (T-114-57), which no client input reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateTrigger {
    /// MCP 2025-11-25: the request carried a `task` field. BYTE-FROZEN.
    V1TaskField {
        /// Whether `CallToolRequest.task` was present on this request.
        task_field_present: bool,
    },
    /// MCP 2026-07-28: the client declared the tasks extension on this request.
    V2ClientDeclaration {
        /// Whether `io.modelcontextprotocol/tasks` appeared in this request's
        /// declared `clientCapabilities.extensions`.
        client_declared_tasks: bool,
    },
}

impl CreateTrigger {
    /// Resolve the trigger for `era` from this request's RAW facts.
    ///
    /// The ONE place the era chooses a trigger. The declaration is read through
    /// [`TaskDispatch::declares_tasks_extension`] — the SAME predicate
    /// `route_tasks_endpoint`'s case-3 refusal uses — off the already-resolved
    /// [`ProtocolContext`](crate::types::protocol::ProtocolContext), never by
    /// re-parsing `params._meta` (resolving once at ingress is Phase 112's whole
    /// point).
    pub(crate) fn resolve(
        era: Option<crate::types::protocol::Era>,
        task_field_present: bool,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> Self {
        if is_v1_task_era(era) {
            return Self::V1TaskField { task_field_present };
        }
        Self::V2ClientDeclaration {
            client_declared_tasks: TaskDispatch::declares_tasks_extension(protocol_context, era),
        }
    }

    /// Did this era's trigger fire?
    const fn fired(self) -> bool {
        match self {
            Self::V1TaskField { task_field_present } => task_field_present,
            Self::V2ClientDeclaration {
                client_declared_tasks,
            } => client_declared_tasks,
        }
    }
}

/// The verdict of the SHARED create gate — the single expression both
/// dispatchers ask (plan 114-12, T-114-58).
///
/// Three answers rather than a `bool` because `ServerCore` logs the middle one:
/// a tool that declares task support but returns a non-task-shaped value is a
/// tool-authoring mistake worth a `debug!`, while a closed gate is the ordinary
/// "this is just a tool call" path and must stay silent. Collapsing them would
/// have forced `ServerCore` to keep its own copy of the task-shape check, which
/// is the divergent second copy this plan exists to delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateGate {
    /// Every precondition holds — mint a task.
    Create,
    /// Trigger, backend and `TaskSupport` all hold, but the tool's value is not
    /// task-shaped. Fall through to a normal `CallToolResult`; the caller MAY
    /// log it.
    NotTaskShaped,
    /// A precondition is absent. Fall through SILENTLY and with NO error leak
    /// (T-102-11) — this covers `TaskSupport::Forbidden`/absent, no backend, and
    /// an untriggered request.
    Closed,
}

/// A `tasks/update`'s params in their RAW, UNDECODED form (plan 114-14).
///
/// A BORROW-struct: `input_responses` points into the caller's `params` value
/// rather than owning a copy, because a copy taken here would duplicate up to the
/// 256 KiB [`MAX_INPUT_RESPONSES_TOTAL_BYTES`](crate::types::mrtr) budget BEFORE
/// that budget has been checked.
///
/// `input_responses` is a `serde_json::Map<String, Value>` — `serde_json`'s
/// `BTreeMap<String, Value>` — and emphatically NOT an
/// [`InputResponses`]. See [`TaskDispatch::parse_tasks_update_params`] for why
/// that distinction is the whole point of this type existing.
struct TasksUpdateParams<'a> {
    /// The task this delivery addresses, resolved through the routing-name table.
    task_id: String,
    /// The caller's `inputResponses`, UNDECODED and UNBOUNDED at construction
    /// time. Both of those are fixed by the two steps that follow, in that order.
    input_responses: &'a serde_json::Map<String, Value>,
}

/// The `UpdateTaskResult` wire body: an EMPTY acknowledgement.
///
/// `UpdateTaskResult = Result` in the vendored draft extension schema — the ack
/// carries no task fields at all, and the `resultType: "complete"` discriminator
/// is written by the envelope rather than here (`own_reserved_result_fields` owns
/// that key and overwrites whatever a producer puts in it).
///
/// Spelled once, and beside [`Self`](update_ack)'s only two call sites, so the
/// store leg and the router leg cannot come to disagree about what an
/// acknowledgement looks like. The v2 client landed by 114-19 decodes exactly
/// this shape as SUCCESS (`v2_empty_update_ack_is_not_a_decode_error`).
fn update_ack(id: RequestId) -> JSONRPCResponse {
    success_response(id, Value::Object(serde_json::Map::new()))
}

/// Borrow-struct holding the task backend handles and the identity inputs a
/// dispatcher owns.
///
/// Both `Server` and `ServerCore` construct a `TaskDispatch` borrowing their own
/// `task_store`/`task_router` fields and call into it — the task-lifecycle logic
/// lives HERE, once, never as a divergent second copy.
pub(crate) struct TaskDispatch<'a> {
    /// Standard task backend (polling path). Presence flips `tasks` capability on.
    pub(crate) task_store: &'a Option<Arc<dyn TaskStore>>,
    /// Legacy experimental router backend (fall-through path).
    pub(crate) task_router: &'a Option<Arc<dyn TaskRouter>>,
    /// Whether this server has an auth provider configured — the FAIL-CLOSED
    /// input to the v2 identity table (TASK-05, D-07).
    ///
    /// A PER-SERVER fact, so it lives on the borrow-struct alongside the two
    /// backend handles rather than being threaded through every route. Both
    /// dispatchers read it from their EXISTING auth-provider accessor
    /// (`Server::get_auth_provider` / `ServerCore`'s own field, the same read
    /// `MrtrRound::begin` already makes) — no new field is added to either
    /// server, exactly as `listen_server_view` does for
    /// `subscriptions/listen`.
    pub(crate) has_auth_provider: bool,
}

impl TaskDispatch<'_> {
    /// Does this server have ANY task backend — a [`TaskStore`], a
    /// [`TaskRouter`], or both?
    ///
    /// The two v2 era gates consult it so a backend-LESS server keeps its
    /// existing "not enabled" / "not supported" refusal on EVERY era. "This
    /// method was retired" and "this server serves no tasks at all" are
    /// different facts calling for opposite fixes, and the `-32601` message is
    /// the only place a caller can tell them apart (T-114-33).
    const fn has_task_backend(&self) -> bool {
        self.task_store.is_some() || self.task_router.is_some()
    }

    /// Bind this request to a task owner, ERA-AWARE and FAIL-CLOSED on v2
    /// (TASK-05, D-07).
    ///
    /// Owner is ALWAYS derived from the `AuthContext`/router, NEVER from client
    /// params (IDOR mitigation, T-102-01) — on both eras.
    ///
    /// # v1 (and a request carrying no era code at all) — FROZEN
    ///
    /// Byte-identical to what it has always been. With a [`TaskRouter`],
    /// delegates to [`TaskRouter::resolve_owner`] (priority chain: OAuth subject,
    /// then client id, then session id, then the shared
    /// [`V1_UNAUTHENTICATED_OWNER`] bucket); with only a [`TaskStore`], the owner
    /// IS the OAuth subject; with no backend at all, the value is inert and
    /// collapses onto the same fallback every pre-114-09 caller already applied
    /// with `.unwrap_or_else(|| "local")`. The ONLY addition is the D-10
    /// migration `tracing::warn!` on the unauthenticated row.
    ///
    /// # v2 — the three-row identity table, REUSED not re-derived
    ///
    /// | authenticated subject | `has_auth_provider` | owner |
    /// |---|---|---|
    /// | `Some(sub)` | any | `sub` |
    /// | `None` | `true` | [`OwnerBinding::Refused`] |
    /// | `None` | `false` | [`ANONYMOUS_PRINCIPAL`](crate::server::core::ANONYMOUS_PRINCIPAL) |
    ///
    /// The decision is [`resolve_mrtr_principal`](crate::server::core::resolve_mrtr_principal)
    /// itself — the same function, not a copy of its match — because a task
    /// record and an MRTR continuation are both server-held state a later
    /// request redeems, and "who may redeem it" must have exactly one answer per
    /// server. See that function for why one table rather than two.
    ///
    /// ## The v2 arm never calls [`TaskRouter::resolve_owner`]
    ///
    /// Deliberate, and NOT an oversight to be tidied away by a later "unify the
    /// two paths" change. That method's chain reaches:
    ///
    /// * a **session id**, which TASK-05 forbids outright — v2 is stateless by
    ///   design and has no session, so binding an owner to one would either fail
    ///   or (worse) collide callers who happen to share a synthesised id; and
    /// * a **`client_id`**, which is per-APPLICATION (the OAuth `azp` claim), so
    ///   using it would collapse per-USER isolation into per-APP isolation —
    ///   every user of the same client application would share one task bucket
    ///   (T-114-38).
    ///
    /// ## D-07's caveat, stated plainly rather than implied
    ///
    /// Row 3 means that on a server with **no auth provider at all**, every v2
    /// caller shares ONE bucket. Fail-closed therefore applies to
    /// **auth-configured deployments** (row 2); a no-auth-provider server runs v2
    /// tasks in a single shared bucket BY DESIGN. That is a development / stdio
    /// affordance, NOT per-caller isolation, and it is defensible only because
    /// such a server has no notion of caller identity to separate in the first
    /// place. The production backends refuse that bucket independently:
    /// `TaskSecurityConfig::default()` sets `allow_anonymous: false`
    /// (`crates/pmcp-tasks/src/security.rs:89`), so `GenericTaskStore` rejects an
    /// anonymous owner unless an operator opts in (T-114-39).
    ///
    /// TASK-05's own wording says owner binding "fails closed" when no stable
    /// identity exists, which row 3 does not do; that gap is recorded as its own
    /// row in `114-SPEC-RECHECK.md` rather than left to be inferred, with the
    /// deferred configurable proxy-header identity source named as its future
    /// closure.
    pub(crate) fn resolve_owner(
        &self,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> OwnerBinding {
        if is_v1_task_era(era) {
            return OwnerBinding::Owner(self.resolve_v1_owner(auth_context));
        }
        // v2: the SHARED table, not a second match over the same two inputs.
        let principal = crate::server::core::MrtrPrincipal {
            authenticated_subject: auth_context.map(|ctx| ctx.subject.as_str()),
            has_auth_provider: self.has_auth_provider,
        };
        crate::server::core::resolve_mrtr_principal(principal)
            .map_or(OwnerBinding::Refused, |owner| {
                OwnerBinding::Owner(owner.to_string())
            })
    }

    /// The FROZEN v1 owner binding, plus D-10's migration warn.
    ///
    /// Split out only so the v2 arm of [`Self::resolve_owner`] reads as one
    /// decision; the body is byte-for-byte the pre-114-09 logic with the three
    /// former `None` outcomes collapsed onto [`V1_UNAUTHENTICATED_OWNER`] — which
    /// is what every caller already did with `.unwrap_or_else(|| "local")`.
    fn resolve_v1_owner(&self, auth_context: Option<&AuthContext>) -> String {
        if auth_context.is_none() {
            // D-10 migration warn. Emitted once per unauthenticated v1 owner
            // resolution, and it names the shared bucket rather than describing
            // it, so an operator can grep for the string that is actually in
            // their store.
            tracing::warn!(
                target: "mcp.tasks",
                owner = V1_UNAUTHENTICATED_OWNER,
                "an unauthenticated v1 task request was bound to the shared \"local\" owner \
                 bucket, which every other unauthenticated caller on this server also shares; \
                 protocol version 2026-07-28 binds the owner to the authenticated subject \
                 instead and refuses the request outright when an auth provider is configured"
            );
        }
        // Legacy path: TaskRouter has its own resolve_owner logic.
        if let Some(router) = self.task_router {
            return match auth_context {
                Some(ctx) => {
                    router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)
                },
                None => router.resolve_owner(None, None, None),
            };
        }
        // Standard path: derive owner from auth context when task_store is configured.
        // Key on the OAuth subject (the authenticated principal), matching the
        // router's subject-first priority — NOT client_id, which is per-application
        // (OAuth `azp`) and would collapse per-user isolation to per-app isolation.
        //
        // With NO backend at all the value is inert: every route reaches its
        // frozen `-32601` without reading it.
        match auth_context {
            Some(ctx) => ctx.subject.clone(),
            None => V1_UNAUTHENTICATED_OWNER.to_string(),
        }
    }

    /// Extract the terminal [`CallToolResult`] from a task-shaped tool value.
    ///
    /// Per `D-TERMINAL-RESULT-CONTRACT`: if the value carries a `result` object or
    /// a `content` array, deserialize it into a [`CallToolResult`]; otherwise the
    /// task is genuinely pending and there is no synchronous terminal result.
    pub(crate) fn extract_terminal_result(value: &Value) -> Option<CallToolResult> {
        if let Some(result_value) = value.get("result") {
            return serde_json::from_value::<CallToolResult>(result_value.clone()).ok();
        }
        if value.get("content").is_some() {
            return serde_json::from_value::<CallToolResult>(value.clone()).ok();
        }
        None
    }

    /// Extract a handler-declared PAUSE from a task-shaped tool value.
    ///
    /// Returns the server-authored `inputRequests` map IFF the tool's value
    /// declares BOTH:
    ///
    /// - a `status` that deserializes to [`TaskStatus::InputRequired`] — read
    ///   through the TYPE, never compared against a re-spelled wire literal, and
    /// - an `inputRequests` OBJECT that parses as a real
    ///   [`InputRequests`](crate::types::mrtr::InputRequests).
    ///
    /// Both are required, so a tool that wrote one without the other gets the
    /// ordinary `working` handle rather than a half-applied pause. A malformed
    /// map yields `None` (fall through to `working`) rather than an error: the
    /// tool already ran and its task already exists, so failing the whole call
    /// here would discard work that succeeded.
    ///
    /// # This map is SERVER-AUTHORED
    ///
    /// `value` is the TOOL HANDLER's output — server code, not anything a client
    /// sent. That is the precondition
    /// [`TaskStore::record_input_requests`](crate::server::task_store::TaskStore::record_input_requests)
    /// states: what is written becomes the only trustworthy record of which KIND
    /// was asked for under each key, and a kind-directed decode of the client's
    /// answers reads it back.
    pub(crate) fn extract_input_requests(
        value: &Value,
    ) -> Option<crate::types::mrtr::InputRequests> {
        let status = value.get("status")?;
        if serde_json::from_value::<TaskStatus>(status.clone()).ok()? != TaskStatus::InputRequired {
            return None;
        }
        let requests = value.get("inputRequests")?;
        if !requests.is_object() {
            return None;
        }
        serde_json::from_value::<crate::types::mrtr::InputRequests>(requests.clone()).ok()
    }

    /// Build the `tools/call` create-task response.
    ///
    /// Per `D-STORE-MINTS-ID`: when a [`TaskStore`] is configured the store mints
    /// the canonical task id via `store.create()`; that store-minted id is
    /// reflected on the WIRE in BOTH `CreateTaskResult.task.taskId` AND the
    /// `_meta.relatedTask.taskId` envelope (never the tool's fabricated id). When
    /// the terminal result is present (synchronous completion) it is persisted via
    /// `store.set_result()` and the task is transitioned `Working -> Completed`
    /// BEFORE the response returns, so a subsequent `tasks/get` shows `Completed`.
    ///
    /// SIGNATURE NOTE: this fn does NOT take `task_id`, the terminal `result`, or
    /// the handler's `inputRequests` as params — it RE-EXTRACTS all three from
    /// `value` internally (the store-minted id comes back from `store.create`,
    /// [`Self::extract_terminal_result`] recovers the terminal result for
    /// persistence, and [`Self::extract_input_requests`] recovers a
    /// handler-declared pause). A future refactor that stops re-extracting MUST
    /// add explicit params instead — never silently drop either write. Dropping
    /// the terminal-result persistence regresses synchronous completion; dropping
    /// the `inputRequests` persistence returns a handle that LOOKS pausable and is
    /// not, which fails two waves downstream (`tasks/update` and the paired
    /// example both assume the pause is already recorded).
    ///
    /// # Why the pause is recorded HERE and not by the handler
    ///
    /// `store.create()` mints the canonical id AFTER the tool handler has already
    /// returned, discarding the tool's fabricated `taskId`. A handler therefore
    /// CANNOT associate its input requests with the id the client will poll. This
    /// is the one place both ids exist at once, so it is the only place the
    /// association can be made — against the STORE-minted id, never the
    /// fabricated one.
    ///
    /// Falls back to the legacy tool-fabricated envelope only when no store is
    /// configured (preserves prior behavior for router-only servers).
    ///
    /// `era` reaches the SAME [`Self::resolve_owner`] table every `tasks/*` route
    /// uses, so a task's owner is bound at CREATE by exactly the rule that later
    /// governs who may read it. On the v2 refuse row this answers `-32003` and
    /// mints nothing. WHETHER a v2 `tools/call` becomes a task at all is plan
    /// 114-12's decision (DQ1); this plan only decides WHOSE task it is.
    pub(crate) async fn build_task_created_response(
        &self,
        id: RequestId,
        value: Value,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> (JSONRPCResponse, DispatchEnvelopeClaim) {
        let v1 = is_v1_task_era(era);
        let Some(store) = self.task_store.as_ref() else {
            // No store: preserve the legacy tool-fabricated envelope. The
            // tool-fabricated task id is only needed on THIS path; with a store
            // the store-minted id wins, so don't allocate it otherwise.
            let tool_task_id = value
                .get("taskId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            // On v2 the same flat projection applies when the tool's value is a
            // parseable task; otherwise the legacy nested envelope is emitted
            // unchanged rather than half-projected.
            if !v1 {
                if let Ok(task) = serde_json::from_value::<Task>(value.clone()) {
                    return (
                        success_response(id, v2_create_result_value(&task, &tool_task_id)),
                        DispatchEnvelopeClaim::TASK_CREATED,
                    );
                }
            }
            let result_value = serde_json::json!({
                "task": value,
                "_meta": { RELATED_TASK_META_KEY: { "taskId": tool_task_id } }
            });
            return (
                success_response(id, result_value),
                DispatchEnvelopeClaim::NONE,
            );
        };

        let OwnerBinding::Owner(owner_id) = self.resolve_owner(auth_context, era) else {
            return (
                authentication_required(id, crate::types::mrtr::CALL_TOOL_METHOD),
                DispatchEnvelopeClaim::NONE,
            );
        };

        // Carry the tool's requested TTL onto the store-minted task, if present.
        let ttl = value.get("ttl").and_then(serde_json::Value::as_u64);

        let created = match store.create(&owner_id, ttl).await {
            Ok(task) => task,
            Err(e) => {
                return (
                    store_error_response(id, &e, era),
                    DispatchEnvelopeClaim::NONE,
                )
            },
        };
        let store_id = created.task_id.clone();

        // The two post-create writes are MUTUALLY EXCLUSIVE, and the source SAYS
        // so — one `if let … else if let …`, not two independent `if`s: a task is
        // either already terminal or awaiting input, never both.
        //
        // - Synchronous completion: persist the terminal result and complete.
        // - Handler-declared pause: record the server-authored input requests
        //   against the STORE-minted id, so the handle this call returns is
        //   ALREADY paused and pollable (a later `tasks/get` shows
        //   `input_required` and inlines the same set the handler declared).
        let final_task = if let Some(call_result) = Self::extract_terminal_result(&value) {
            if let Err(e) = store.set_result(&store_id, &owner_id, call_result).await {
                return (
                    store_error_response(id, &e, era),
                    DispatchEnvelopeClaim::NONE,
                );
            }
            match store
                .update_status(&store_id, &owner_id, TaskStatus::Completed, None)
                .await
            {
                Ok(task) => task,
                Err(e) => {
                    return (
                        store_error_response(id, &e, era),
                        DispatchEnvelopeClaim::NONE,
                    )
                },
            }
        } else if let Some(requests) = Self::extract_input_requests(&value) {
            match store
                .record_input_requests(&store_id, &owner_id, requests)
                .await
            {
                Ok(task) => task,
                Err(e) => {
                    return (
                        store_error_response(id, &e, era),
                        DispatchEnvelopeClaim::NONE,
                    )
                },
            }
        } else {
            created
        };

        // Build the wire envelope from the STORE-minted task (typed, no
        // hand-written task JSON) so task.taskId == _meta id == store id.
        //
        // v1: the FROZEN nested `CreateTaskResult`. v2: the flat `Result & Task`
        // the extension declares, and the ONE response in the whole surface that
        // earns `resultType: "task"` — a `tasks/get`, a `tasks/cancel` and a
        // `tasks/update` are all ordinary complete results ABOUT a task.
        if v1 {
            return (
                success_response(id, v1_create_result_value(&final_task, &store_id)),
                DispatchEnvelopeClaim::NONE,
            );
        }
        (
            success_response(id, v2_create_result_value(&final_task, &store_id)),
            DispatchEnvelopeClaim::TASK_CREATED,
        )
    }

    /// The COMPLETE create gate, as ONE expression, reached from BOTH dispatch
    /// sites (plan 114-12, T-114-58).
    ///
    /// `Server` reaches it through [`Self::maybe_build_task_created`];
    /// `ServerCore` calls it directly, because that dispatcher returns a
    /// `ToolCallOutcome` rather than a `JSONRPCResponse` and builds its envelope
    /// one frame up. The RESPONSE building is therefore NOT shared — the
    /// PREDICATE is. Adding a future era's trigger means editing this one
    /// expression and [`CreateTrigger`], and nothing else.
    ///
    /// Returns [`CreateGate::Create`] IFF ALL of:
    /// - the era's `trigger` FIRED (see [`CreateTrigger`] for the per-era table), AND
    /// - a backend is present (`self.task_store.is_some()`), AND
    /// - `task_support ∈ {Required, Optional}`, AND
    /// - `value` carries BOTH a `taskId` and a `status` (task-shaped).
    ///
    /// The caller passes RAW facts and this enforces the whole rule; no caller
    /// pre-computes any part of it.
    pub(crate) fn create_gate(
        &self,
        trigger: CreateTrigger,
        task_support: Option<TaskSupport>,
        value: &Value,
    ) -> CreateGate {
        let gate_open = trigger.fired()
            && self.task_store.is_some()
            && task_support
                .is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));
        if !gate_open {
            return CreateGate::Closed;
        }
        // Task-shaped value check: must carry BOTH a taskId and a status.
        let is_task_shaped =
            value.get("taskId").and_then(Value::as_str).is_some() && value.get("status").is_some();
        if is_task_shaped {
            CreateGate::Create
        } else {
            CreateGate::NotTaskShaped
        }
    }

    /// Self-enforcing create-path gate: decide whether a `tools/call` becomes a
    /// task and, if so, build the create response.
    ///
    /// The `Server` dispatcher's entry point to the SINGLE source of truth for
    /// "should this `tools/call` become a task?" — [`Self::create_gate`]. The
    /// helper enforces the COMPLETE gate INTERNALLY: the caller passes raw facts
    /// (the era's [`CreateTrigger`], the tool's `task_support`, the produced
    /// `value`), never a pre-checked precondition.
    ///
    /// # The trigger is ERA-AWARE and each era IGNORES the other's signal
    ///
    /// | era | trigger | the other era's signal |
    /// |-----|---------|------------------------|
    /// | MCP 2025-11-25 / no era code | `CallToolRequest.task` is present | a declaration is IGNORED |
    /// | MCP 2026-07-28 | the client declared `io.modelcontextprotocol/tasks` on this request | the `task` field is IGNORED |
    ///
    /// On v2 the declaration is the extension's OWN precondition: a server MUST
    /// NOT return a `CreateTaskResult` to a non-declaring client, which would
    /// hand a task handle to a client that has no rule for reading one. So this
    /// gate is a conformance requirement, not a pmcp preference. And the v1
    /// `task` field is not consulted on v2 because it does not exist in the v2
    /// extension at all — requiring it would make v2 task creation unreachable.
    ///
    /// A closed gate, a `TaskSupport::Forbidden`/`None`, an absent backend, or a
    /// non-task-shaped value ALL return `None` ("fall through to a normal
    /// `CallToolResult`") with NO error leak (T-102-11).
    // Why: proven by the in-module `gate_tests` truth-table in Plan 01 and wired
    // into the `Server` create-path in Plan 02 (`handle_call_tool`), so it is
    // production-reachable — no `dead_code` allow is needed.
    pub(crate) async fn maybe_build_task_created(
        &self,
        id: RequestId,
        value: &Value,
        task_support: Option<TaskSupport>,
        trigger: CreateTrigger,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<(JSONRPCResponse, DispatchEnvelopeClaim)> {
        match self.create_gate(trigger, task_support, value) {
            CreateGate::Create => Some(
                self.build_task_created_response(id, value.clone(), auth_context, era)
                    .await,
            ),
            CreateGate::NotTaskShaped | CreateGate::Closed => None,
        }
    }

    /// Handle a `tasks/result` request (store-first → router → -32002 → -32601).
    ///
    /// On protocol version 2026-07-28 the method is RETIRED. That gate is case 1
    /// of [`Self::route_tasks_endpoint`]'s ordered refusal chain and fires before
    /// this function is entered at all — see [`tasks_result_serves_on_era`] and
    /// [`V2_TASKS_METHOD_RETIRED`]. The tail `match` below still reads the SAME
    /// predicate, deliberately: ONE era definition, N call sites (114-08).
    ///
    /// On v1 (and on a request carrying no era code at all) the behaviour is
    /// byte-for-byte what it has always been: serves from the configured
    /// [`TaskStore`] FIRST when it `supports_results()`, but FALLS THROUGH to the
    /// [`TaskRouter`] on store `NotFound`/unsupported — never a hard error when a
    /// router can serve it. When the store has no result and NO router is
    /// configured, returns the SPECIFIED "task not completed" error (`-32002`),
    /// distinct from the truly-no-backend `-32601` (FROZEN by Phase 101;
    /// T-102-03); see [`is_v1_task_era`].
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`],
    /// resolved once per request by the caller. This function does not — and
    /// must not — bind a second one.
    pub(crate) async fn handle_tasks_result(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskPayloadRequest,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> JSONRPCResponse {
        // Store-first: serve a typed CallToolResult when the store persists one.
        if let Some(store) = self.task_store {
            if store.supports_results() {
                match store.get_result(&params.task_id, owner_id).await {
                    Ok(call_result) => {
                        return success_response(
                            id,
                            serde_json::to_value(call_result).unwrap_or_default(),
                        );
                    },
                    // NotFound = store doesn't have it (absent / pending / owner
                    // mismatch): fall through to the router below.
                    Err(crate::server::task_store::TaskStoreError::NotFound { .. }) => {},
                    Err(e) => {
                        return error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        )
                    },
                }
            }
        }

        // Router fallback — behavior UNCHANGED for router-backed servers.
        if let Some(task_router) = self.task_router {
            return match task_router
                .handle_tasks_result(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            };
        }

        // No router: distinguish "store exists but task not completed yet"
        // (specified error) from "no task backend at all".
        //
        // The era axis reads the SAME predicate as case 1 of
        // `route_tasks_endpoint`'s chain, deliberately — not a second,
        // independently-disable-able copy of the era question. A negative control
        // measured why: with this arm keyed on `is_v1_task_era` directly,
        // disabling `tasks_result_serves_on_era` left this arm still refusing v2
        // with an identical body, so the retirement gate could not be proven
        // load-bearing by any test. ONE predicate, two call sites: disable it and
        // the whole gate opens, which is what a negative control has to be able
        // to do.
        match (self.task_store.is_some(), tasks_result_serves_on_era(era)) {
            (true, true) => error_response(
                id,
                // FROZEN wire value -32002 (byte-identical); read by name from the
                // centralized table (Pitfall 6). The
                // pending_tasks_result_preserves_minus_32002 test is the guard.
                // Unreachable on v2 by the arm below, which is what keeps this
                // spec-prohibited code off the v2 wire (Finding 11).
                crate::types::protocol::error_codes::V1_TASK_PENDING,
                "task result not available: task not completed".to_string(),
            ),
            // Required for exhaustiveness, and unreachable in production: case 1
            // already returned for every era-v2 request that has a backend, and
            // `task_store.is_some()` implies one. It answers IDENTICALLY so the
            // two spellings of the refusal cannot diverge.
            (true, false) => retired_on_v2(id, TASKS_RESULT_METHOD),
            (false, _) => error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_RESULT_NOT_SUPPORTED.to_string(),
            ),
        }
    }

    /// Read the status-specific detail of `task` from the STORE, through
    /// 114-04's accessors and never through the private record.
    ///
    /// The two values that are not on the wire [`Task`] at all come from the two
    /// seams that exist for them: `inputRequests` from
    /// [`TaskStore::task_input_snapshot`] and a failed task's `error` from
    /// [`TaskStore::get_error`]. `TaskStore::get` returns only the wire `Task`
    /// and `TaskRecord` is private, so neither is reachable any other way.
    ///
    /// Returns `None` when the backend cannot supply the detail — see
    /// [`v2_detailed_task_value`] for why that degrades rather than fabricates.
    async fn v2_task_detail(&self, task: &Task, owner_id: &str) -> Option<TaskDetailV2> {
        let store = self.task_store.as_ref()?;
        match task.status {
            TaskStatus::Working => Some(TaskDetailV2::Working),
            TaskStatus::Cancelled => Some(TaskDetailV2::Cancelled),
            TaskStatus::InputRequired => store
                .task_input_snapshot(&task.task_id, owner_id)
                .await
                .ok()
                .map(|snapshot| TaskDetailV2::InputRequired {
                    input_requests: snapshot.input_requests,
                }),
            TaskStatus::Completed => {
                if !store.supports_results() {
                    return None;
                }
                store
                    .get_result(&task.task_id, owner_id)
                    .await
                    .ok()
                    .and_then(|result| as_object(serde_json::to_value(result).ok()?))
                    .map(|result| TaskDetailV2::Completed { result })
            },
            TaskStatus::Failed => store
                .get_error(&task.task_id, owner_id)
                .await
                .ok()
                .and_then(as_object)
                .map(|error| TaskDetailV2::Failed { error }),
        }
    }

    /// The v2 `tasks/get` success response, plus the envelope claim it earns.
    ///
    /// An `input_required` task inlines a TOP-LEVEL `inputRequests`, which the
    /// reserved-result-field registry strips from any result whose owner is not
    /// [`ReservedFieldOwner::TasksDispatch`](crate::server::core::ReservedFieldOwner::TasksDispatch)
    /// — so the claim is returned alongside the response rather than left to be
    /// re-derived at the envelope (DQ2). Every other status claims nothing.
    async fn v2_get_response(
        &self,
        id: RequestId,
        task: &Task,
        owner_id: &str,
    ) -> (JSONRPCResponse, DispatchEnvelopeClaim) {
        let detail = self.v2_task_detail(task, owner_id).await;
        let claims_input_requests = matches!(detail, Some(TaskDetailV2::InputRequired { .. }));
        let response = success_response(id, v2_detailed_task_value(task, detail));
        let claim = if claims_input_requests {
            DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED
        } else {
            DispatchEnvelopeClaim::NONE
        };
        (response, claim)
    }

    /// Route a `tasks/get` request (store-first, router fall-through).
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    ///
    /// # The era decides the SHAPE, not the routing
    ///
    /// v1 keeps the nested `{"task": {…}}` `GetTaskResult` with `ttl` and
    /// `pollInterval`, byte-for-byte. v2 emits the flat `DetailedTask` variant —
    /// `ttlMs`, `pollIntervalMs`, and the status-conditional `result` / `error` /
    /// `inputRequests` inlined at the TOP LEVEL. Both eras take the same
    /// store-first / router-fall-through / no-backend path to get there.
    async fn route_tasks_get(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskRequest,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> (JSONRPCResponse, DispatchEnvelopeClaim) {
        let v1 = is_v1_task_era(era);
        if let Some(store) = self.task_store {
            return match store.get(&params.task_id, owner_id).await {
                Ok(task) if v1 => {
                    let result = crate::types::tasks::GetTaskResult::new(task);
                    (
                        success_response(id, serde_json::to_value(result).unwrap_or_default()),
                        DispatchEnvelopeClaim::NONE,
                    )
                },
                Ok(task) => self.v2_get_response(id, &task, owner_id).await,
                Err(e) => (
                    store_error_response(id, &e, era),
                    DispatchEnvelopeClaim::NONE,
                ),
            };
        }
        if let Some(task_router) = self.task_router {
            return match task_router
                .handle_tasks_get(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) if v1 => (success_response(id, result), DispatchEnvelopeClaim::NONE),
                Ok(result) => {
                    let projected = v2_project_router_task(result);
                    let claims = projected
                        .get(crate::types::tasks::DETAIL_KEY_INPUT_REQUESTS)
                        .is_some();
                    (
                        success_response(id, projected),
                        if claims {
                            DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED
                        } else {
                            DispatchEnvelopeClaim::NONE
                        },
                    )
                },
                Err(e) => (
                    error_response(
                        id,
                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                        e.to_string(),
                    ),
                    DispatchEnvelopeClaim::NONE,
                ),
            };
        }
        (
            error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            ),
            DispatchEnvelopeClaim::NONE,
        )
    }

    /// Route a `tasks/list` request (store-first, router fall-through).
    ///
    /// On protocol version 2026-07-28 the method is RETIRED and answers
    /// `-32601` WITHOUT enumerating anything. That gate is case 1 of
    /// [`Self::route_tasks_endpoint`]'s ordered chain and fires before this
    /// function is entered — which is what makes enumeration impossible rather
    /// than merely refused: no store `list`, no router call, and not even an
    /// owner binding, so nothing can leak the existence of a task into the
    /// response body. See [`tasks_list_serves_on_era`] and
    /// [`V2_TASKS_METHOD_RETIRED`]. On v1 the store/router behaviour below is
    /// unchanged.
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    async fn route_tasks_list(
        &self,
        id: RequestId,
        params: &crate::types::tasks::ListTasksRequest,
        owner_id: &str,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            match store.list(owner_id, params.cursor.as_deref()).await {
                Ok((tasks, next_cursor)) => {
                    let mut result = crate::types::tasks::ListTasksResult::new(tasks);
                    if let Some(cursor) = next_cursor {
                        result = result.with_next_cursor(cursor);
                    }
                    success_response(id, serde_json::to_value(result).unwrap_or_default())
                },
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else if let Some(task_router) = self.task_router {
            match task_router
                .handle_tasks_list(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else {
            error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            )
        }
    }

    /// Route a `tasks/cancel` request (store-first, router fall-through).
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    ///
    /// # v2 answers an EMPTY acknowledgement
    ///
    /// `CancelTaskResult = Result` in the extension schema — no task body at all
    /// (inventory row 20). v1 keeps its nested `{"task": {…}}`.
    ///
    /// The empty ack is not a lossy simplification, it is the semantics:
    /// **cancellation is cooperative and eventually consistent.** The task MAY
    /// still be `working` when the ack arrives and MAY reach a terminal status
    /// other than `cancelled`. Returning a task body would invite a client to
    /// treat the ack as the final state; deliberately NO wait and NO poll is
    /// added here to make the ack look synchronous. A client that wants the
    /// settled state issues `tasks/get`.
    async fn route_tasks_cancel(
        &self,
        id: RequestId,
        params: &crate::types::tasks::CancelTaskRequest,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> JSONRPCResponse {
        let v1 = is_v1_task_era(era);
        if let Some(store) = self.task_store {
            return match store.cancel(&params.task_id, owner_id).await {
                Ok(task) if v1 => {
                    let result = crate::types::tasks::CancelTaskResult::new(task);
                    success_response(id, serde_json::to_value(result).unwrap_or_default())
                },
                Ok(_) => success_response(id, Value::Object(serde_json::Map::new())),
                Err(e) => store_error_response(id, &e, era),
            };
        }
        if let Some(task_router) = self.task_router {
            return match task_router
                .handle_tasks_cancel(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) if v1 => success_response(id, result),
                Ok(_) => success_response(id, Value::Object(serde_json::Map::new())),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            };
        }
        error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            TASKS_NOT_ENABLED.to_string(),
        )
    }

    /// Route any `tasks/*` endpoint request to its handler.
    ///
    /// Dispatches `TasksGet`/`TasksList`/`TasksCancel` to their per-endpoint
    /// helpers and `TasksResult` to [`Self::handle_tasks_result`]. Non-`tasks/*`
    /// variants return the FROZEN `-32601 "Method not supported"` (callers only
    /// pass `tasks/*` variants here).
    ///
    /// `protocol_context` is the ALREADY-RESOLVED
    /// [`ProtocolContext`](crate::types::protocol::ProtocolContext) being
    /// CONSUMED here — this module never runs an era resolver of its own and
    /// never re-reads `params._meta`. Two things are read off it: the
    /// [`era`](crate::types::protocol::ProtocolContext::era), by the
    /// `tasks/result` pending refusal (see [`is_v1_task_era`]) and the two v2
    /// retirement gates ([`tasks_list_serves_on_era`],
    /// [`tasks_result_serves_on_era`]); and the client's declared
    /// [`client_capabilities`](crate::types::protocol::ProtocolContext::client_capabilities),
    /// resolved once at ingress by Phase 112.
    ///
    /// `TasksGet` and `TasksCancel` are not era-GATED on purpose: both survive
    /// in the v2 extension schema. Their v2 response SHAPE is plan 114-11's,
    /// not this router's.
    ///
    /// # Rejection cases, IN ORDER
    ///
    /// The order is the contract, not an implementation detail — each case says
    /// something different to the caller, and the wrong order either leaks or
    /// misdirects. The shape mirrors `subscriptions/listen`'s ordered chain
    /// (D-08), which this reuses down to the `-32003` placement.
    ///
    /// 1. **RETIRED on this era → `-32601`.** A method that does not exist on
    ///    protocol version 2026-07-28 answers "no such method" FIRST, so a
    ///    `tasks/list` cannot be answered "authenticate yourself" and thereby
    ///    imply that authenticating would enumerate anything (T-114-32).
    /// 2. **No task backend → `-32601`.** Answered by the per-endpoint handlers
    ///    below, where each method's FROZEN message lives
    ///    ([`TASKS_NOT_ENABLED`] / [`TASKS_RESULT_NOT_SUPPORTED`]). Cases 3 and 4
    ///    are therefore SKIPPED for a backendless server: it advertises no tasks
    ///    extension at all, so telling such a caller to declare one — or to
    ///    authenticate — would send it to fix the wrong thing (T-114-33).
    /// 3. **Client did not declare the extension → `-32021`.** A
    ///    method-availability-class refusal like cases 1 and 2, and placed with
    ///    them, because it says "this method is not available to you as
    ///    configured" and reveals NOTHING about authentication state. See
    ///    [`missing_tasks_declaration_refusal`].
    /// 4. **Unauthenticated on an auth-configured server → `-32003`.** Row 2 of
    ///    the identity table. Placed AFTER cases 1–3 so a retired method, a
    ///    backendless server or an under-declaring client each keeps its own
    ///    truthful answer rather than being told to authenticate; and BEFORE the
    ///    params are used, so a refused caller's body is never read and no
    ///    store or router is ever consulted (T-114-37).
    /// 5. **The params, finally.** Everything below this line consumes
    ///    `request`'s typed params. Nothing above it does.
    ///
    /// The owner is bound EXACTLY ONCE here and passed down as a `&str`; no
    /// handler resolves a second one.
    pub(crate) async fn route_tasks_endpoint(
        &self,
        id: RequestId,
        request: &ClientRequest,
        auth_context: Option<&AuthContext>,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> (JSONRPCResponse, DispatchEnvelopeClaim) {
        let era = protocol_context.map(|context| context.era);

        if self.has_task_backend() {
            // --- case 1 -----------------------------------------------------
            if let Some(method) = Self::retired_method(request, era) {
                return (retired_on_v2(id, method), DispatchEnvelopeClaim::NONE);
            }

            // --- case 3 -----------------------------------------------------
            // Inside the backend guard on purpose: case 2 (below, in the
            // per-endpoint handlers) owns the answer for a backendless server,
            // which advertises no tasks extension at all — telling such a caller
            // to DECLARE one would send it to fix the wrong thing (T-114-33).
            if !Self::declares_tasks_extension(protocol_context, era) {
                return (
                    missing_tasks_declaration_refusal(id),
                    DispatchEnvelopeClaim::NONE,
                );
            }
        }

        // --- case 4 ---------------------------------------------------------
        let owner_id = match self.resolve_owner(auth_context, era) {
            OwnerBinding::Owner(owner) => owner,
            // Case 2 owns the answer for a backendless server, and every handler
            // below reaches its frozen `-32601` WITHOUT reading the owner, so
            // this value is inert. It is spelled as the v1 fallback rather than
            // as the v2 anonymous principal so that no reader mistakes it for a
            // bucket a task could ever land in: no backend means no task.
            OwnerBinding::Refused if !self.has_task_backend() => {
                V1_UNAUTHENTICATED_OWNER.to_string()
            },
            OwnerBinding::Refused => {
                return (
                    authentication_required(id, Self::method_of(request)),
                    DispatchEnvelopeClaim::NONE,
                );
            },
        };

        // --- case 5 ---------------------------------------------------------
        //
        // `tasks/get` is the ONLY route that can earn a non-default envelope
        // claim: it is the only one that inlines a reserved top-level field.
        // Every other arm returns `NONE` explicitly rather than by omission, so
        // a route added later has to state its own claim.
        match request {
            ClientRequest::TasksGet(params) => {
                self.route_tasks_get(id, params, &owner_id, era).await
            },
            ClientRequest::TasksResult(params) => (
                self.handle_tasks_result(id, params, &owner_id, era).await,
                DispatchEnvelopeClaim::NONE,
            ),
            ClientRequest::TasksList(params) => (
                self.route_tasks_list(id, params, &owner_id).await,
                DispatchEnvelopeClaim::NONE,
            ),
            ClientRequest::TasksCancel(params) => (
                self.route_tasks_cancel(id, params, &owner_id, era).await,
                DispatchEnvelopeClaim::NONE,
            ),
            _ => (
                error_response(
                    id,
                    crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                    NOT_A_TASKS_METHOD.to_string(),
                ),
                DispatchEnvelopeClaim::NONE,
            ),
        }
    }

    /// Route a v2 `tasks/update` request — the ordered GATE chain only (Phase
    /// 114, plan 13, TASK-02).
    ///
    /// The sibling of [`Self::route_tasks_endpoint`] for the one `tasks/*` method
    /// that has no [`ClientRequest`] variant. It cannot go through that function,
    /// which matches on `&ClientRequest`; it takes the RAW `params` instead. Every
    /// other input is IDENTICAL, and every gate is the SAME helper — this function
    /// contains no era definition, no backend predicate, no declaration predicate
    /// and no identity table of its own.
    ///
    /// # Why `tasks/update` is not a `ClientRequest`
    ///
    /// See [`InternalClientRequest::TasksUpdate`](crate::types::protocol::InternalClientRequest):
    /// [`ClientRequest`] is a PUBLIC EXHAUSTIVE enum, so a variant there is a
    /// semver-MAJOR break. It rides the crate-private `InternalClientRequest`
    /// route instead, following the `server/discover` precedent.
    ///
    /// # Rejection cases, IN ORDER — the same contract, one gate wider
    ///
    /// 1. **Not v2 → `-32601`** ([`V1_TASKS_UPDATE_ABSENT`]). Read through
    ///    [`is_v1_task_era`], this module's ONE era definition. Placed FIRST and,
    ///    unlike `route_tasks_endpoint`'s case 1, OUTSIDE the backend guard: a
    ///    method that does not exist on the negotiated protocol version does not
    ///    become conditional on how the server is configured.
    /// 2. **No task backend → `-32601`** ([`TASKS_NOT_ENABLED`], the FROZEN
    ///    message its three sibling methods already emit). Cases 3 and 4 are
    ///    SKIPPED for a backendless server for T-114-33's reason: it advertises no
    ///    tasks extension at all, so telling such a caller to declare one — or to
    ///    authenticate — would send it to fix the wrong thing.
    /// 3. **Client did not declare the extension → `-32021`.** The SAME
    ///    [`Self::declares_tasks_extension`] predicate `route_tasks_endpoint`'s
    ///    case 3 and `CreateTrigger`'s v2 arm both read, off the ALREADY-RESOLVED
    ///    [`ProtocolContext`](crate::types::protocol::ProtocolContext). There is
    ///    no second `params._meta` read on this path.
    /// 4. **Unauthenticated on an auth-configured server → `-32003`.** Row 2 of
    ///    the identity table, via [`Self::resolve_owner`].
    /// 5. **The params, finally.** Nothing above this line reads `params`. That
    ///    ordering is the whole reason
    ///    [`classify_internal_method`](crate::types::protocol::classify_internal_method)
    ///    keeps them RAW, and
    ///    `malformed_params_from_an_unauthenticated_caller_yield_32003` is what
    ///    measures it: an unauthenticated caller sending garbage gets `-32003`,
    ///    not `-32602`.
    ///
    /// 6. **The bounds, over the RAW map, before any decode.** The FOUR existing
    ///    `inputResponses` MRTR bounds via [`check_input_responses_map_bounds`].
    /// 7. **The kind-directed decode, then the delivery, then an EMPTY ack.** See
    ///    [`Self::deliver_tasks_update`].
    ///
    /// # Cases 5-7 are plan 114-14's, and their ORDER is the security property
    ///
    /// The params are read into a RAW map ([`TasksUpdateParams`]) and NOT into the
    /// typed `InputResponses`, because that type's `Deserialize` impl runs
    /// [`InputResponse::try_from_value_untagged`] — the overlapping guess that
    /// mis-typed an elicitation answer as sampling and re-elicited sixteen times
    /// (D-113-O). Deserializing straight into it would re-introduce that bug class
    /// one layer EARLIER than the route exists to prevent it, and would do so
    /// before the bounds had run.
    ///
    /// So: parse raw → bound → decode against the kinds the SERVER recorded. Each
    /// step is refusable and none of them trusts the step's own input to describe
    /// itself.
    ///
    /// # `async` since plan 114-14
    ///
    /// Cases 1-5 touch no store and no router, and until the delivery body landed
    /// this function was deliberately synchronous — an `async fn` that never awaits
    /// is a false promise of I/O to every caller that has to decide where to hold a
    /// lock across it. Case 7 reads the task record and writes the delivery, so the
    /// promise is now real and the single call site
    /// ([`Server::handle_tasks_update`](crate::server::Server::handle_tasks_update))
    /// awaits it.
    pub(crate) async fn route_tasks_update(
        &self,
        id: RequestId,
        params: &Value,
        auth_context: Option<&AuthContext>,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        let era = protocol_context.map(|context| context.era);

        // --- case 1 ---------------------------------------------------------
        if is_v1_task_era(era) {
            return error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                format!("{TASKS_UPDATE_METHOD} {V1_TASKS_UPDATE_ABSENT}"),
            );
        }

        // --- case 2 ---------------------------------------------------------
        if !self.has_task_backend() {
            return error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            );
        }

        // --- case 3 ---------------------------------------------------------
        if !Self::declares_tasks_extension(protocol_context, era) {
            return missing_tasks_declaration_refusal(id);
        }

        // --- case 4 ---------------------------------------------------------
        // The owner every read and write below is scoped to. It comes from the
        // identity table and NEVER from `params` — a client-supplied owner would
        // be a write-side IDOR straight into another caller's paused task
        // (T-114-73).
        let owner_id = match self.resolve_owner(auth_context, era) {
            OwnerBinding::Owner(owner) => owner,
            OwnerBinding::Refused => {
                return authentication_required(id, TASKS_UPDATE_METHOD);
            },
        };
        debug_assert!(
            !owner_id.is_empty() || !self.has_auth_provider,
            "an empty owner is the anonymous principal, which only a server with no auth \
             provider may bind"
        );

        // --- case 5 ---------------------------------------------------------
        // The FIRST params read on this path, and it resolves the task id through
        // the routing-name table rather than spelling `taskId` here.
        let update = match Self::parse_tasks_update_params(params) {
            Ok(update) => update,
            Err(message) => {
                return error_response(
                    id,
                    crate::types::protocol::error_codes::INVALID_PARAMS,
                    message.to_string(),
                )
            },
        };

        // --- case 6 ---------------------------------------------------------
        // BEFORE any decode. Bounding after decoding means the decoder already did
        // the work the bound exists to prevent, and the `Display` of every one of
        // these violations names only the BOUND — never the key, never the value
        // (T-114-68, T-114-69).
        if let Err(violation) = check_input_responses_map_bounds(update.input_responses) {
            return error_response(
                id,
                crate::types::protocol::error_codes::INVALID_PARAMS,
                violation.to_string(),
            );
        }

        // --- case 7 ---------------------------------------------------------
        self.deliver_tasks_update(id, params, &update, &owner_id, era)
            .await
    }

    /// Read a `tasks/update`'s params into their RAW form — a task id and an
    /// UNDECODED `inputResponses` map.
    ///
    /// # Why the map stays raw
    ///
    /// `inputResponses` is typed [`InputResponses`] — a `BTreeMap<String,
    /// InputResponse>` — and [`InputResponse`]'s `Deserialize` impl is the
    /// UNTAGGED guess: it tries `ListRootsResult`, then `CreateMessageResult`,
    /// then `ElicitResult`, and takes the first that fits. `ElicitResult` and
    /// `CreateMessageResult` structurally OVERLAP, which is how D-113-O silently
    /// reclassified an elicitation answer as sampling, never matched the handler's
    /// `Elicitation` arm, and re-elicited sixteen times with no error raised
    /// anywhere.
    ///
    /// So this function deliberately deserializes into
    /// `&serde_json::Map<String, Value>` — `serde_json`'s `BTreeMap<String, Value>`
    /// — and NOT into [`InputResponses`]. Doing otherwise would run that guess at
    /// ingress, one layer before the kind-directed decode this route exists to
    /// perform, and before the bounds had run at all.
    ///
    /// It BORROWS the map rather than cloning it: a clone here would copy up to
    /// the 256 KiB total bound BEFORE that bound has been checked.
    ///
    /// # Errors
    ///
    /// The `-32602` message to emit: [`TASKS_UPDATE_MALFORMED_PARAMS`] for a
    /// missing or non-string `taskId`, [`TASKS_UPDATE_MISSING_INPUT_RESPONSES`]
    /// for an absent or non-object `inputResponses`. Neither echoes anything the
    /// caller sent.
    fn parse_tasks_update_params(
        params: &Value,
    ) -> std::result::Result<TasksUpdateParams<'_>, &'static str> {
        // Resolved through `TASK_NAME_BEARING_METHODS`, the SAME table the
        // `Mcp-Name` routing header derives from: one answer to "where does
        // tasks/update keep its task id".
        let Some(task_id) = crate::types::mrtr::logical_name_of(TASKS_UPDATE_METHOD, params) else {
            return Err(TASKS_UPDATE_MALFORMED_PARAMS);
        };
        let Some(input_responses) = params.get(INPUT_RESPONSES_KEY).and_then(Value::as_object)
        else {
            return Err(TASKS_UPDATE_MISSING_INPUT_RESPONSES);
        };
        Ok(TasksUpdateParams {
            task_id,
            input_responses,
        })
    }

    /// Decode a RAW `inputResponses` map against the kinds the SERVER recorded.
    ///
    /// # This is the D-113-O fix applied to the tasks surface
    ///
    /// Every value is typed with [`InputResponse::decode_for`] using the kind read
    /// from `snapshot.input_requests` — the server's own record, which no client
    /// input reaches. [`InputResponse::try_from_value_untagged`] is never called on
    /// this path and must never be: it is the overlapping guess, and a client that
    /// could choose which variant its answer became would be choosing the server's
    /// control flow (T-114-74).
    ///
    /// The persisted task record is the tasks analogue of Phase 113's AEAD-sealed
    /// continuation. Both hold server-minted kinds; neither is client-writable.
    ///
    /// # Ignore vs refuse — the two are not the same answer
    ///
    /// | the record… | the value… | outcome |
    /// |---|---|---|
    /// | does NOT hold the key | anything | IGNORED (never issued / already answered / superseded) |
    /// | HOLDS the key | decodes as the recorded kind | accepted |
    /// | HOLDS the key | does NOT decode as that kind | REFUSED |
    ///
    /// The ignore row is the extension's own rule: a server SHOULD ignore a key
    /// that is not currently outstanding rather than fail the delivery. Turning it
    /// into an error would break a client that legitimately re-sent an answer.
    ///
    /// # Message provenance
    ///
    /// The refused key is taken from the RECORD via `get_key_value`, not from the
    /// caller's map, so the rendered string is provably server-assigned even
    /// though the two are equal by construction here. An IGNORED key is
    /// CLIENT-chosen by definition and is never rendered anywhere — echoing it
    /// both amplifies and poisons logs (T-114-69). No value is ever rendered.
    ///
    /// # Errors
    ///
    /// [`InputResponseTypingError::KindMismatch`] for the first recorded key whose
    /// value does not decode as its recorded kind.
    fn decode_inputs_against_record(
        raw: &serde_json::Map<String, Value>,
        snapshot: &TaskInputSnapshot,
    ) -> std::result::Result<InputResponses, InputResponseTypingError> {
        let mut typed = InputResponses::new();
        for (key, value) in raw {
            let Some((recorded_key, request)) = snapshot.input_requests.get_key_value(key) else {
                // IGNORED, per the extension's prose rule. Not an error, and the
                // client-chosen key is not carried anywhere it could be rendered.
                continue;
            };
            let kind = request.kind();
            let response = InputResponse::decode_for(kind, value.clone()).map_err(|_| {
                InputResponseTypingError::KindMismatch {
                    key: recorded_key.clone(),
                    expected: kind,
                }
            })?;
            typed.insert(recorded_key.clone(), response);
        }
        Ok(typed)
    }

    /// The `tasks/update` delivery: store-first, router fall-through, EMPTY ack.
    ///
    /// The same store-first / router-fall-through precedence every other `tasks/*`
    /// route uses ([`Self::route_tasks_get`], [`Self::handle_tasks_result`]) — a
    /// store that cannot serve this request is never a hard error while a router
    /// could serve it.
    ///
    /// # The transition is the BACKEND's, atomically
    ///
    /// `InputRequired → Working` iff the delivered set COMPLETES the outstanding
    /// set; a PARTIAL set persists its responses and the task STAYS
    /// `input_required`. That rule lives in
    /// [`TaskStore::deliver_task_inputs`] and is applied there under one write
    /// guard (114-04 / 114-07), so this function does NOT read-then-write around
    /// it and does not re-derive it. Two concurrent deliveries therefore cannot
    /// interleave and lose an update (T-114-70).
    ///
    /// # The acknowledgement is EMPTY and EVENTUALLY CONSISTENT
    ///
    /// `UpdateTaskResult = Result` in the vendored schema: no task body at all.
    /// The `resultType: "complete"` discriminator is supplied by the envelope
    /// (`own_reserved_result_fields` OWNS that key), never written here.
    ///
    /// The server MAY acknowledge before a subsequent `tasks/get` reflects the
    /// change, so — exactly as for [`Self::route_tasks_cancel`] — NO wait and NO
    /// re-read is inserted to make the ack look synchronous. A client that wants
    /// the settled state issues `tasks/get`.
    ///
    /// # `raw_params` for the router leg
    ///
    /// A [`TaskRouter`] is out-of-tree code holding its own record, so it receives
    /// the params VERBATIM and performs its own decode against its own kinds — the
    /// same pass-through [`Self::route_tasks_get`] applies to a router's `Value`.
    /// The four bounds have already fired, so what it receives is bounded.
    async fn deliver_tasks_update(
        &self,
        id: RequestId,
        raw_params: &Value,
        update: &TasksUpdateParams<'_>,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> JSONRPCResponse {
        if let Some(response) = self
            .deliver_update_through_store(id.clone(), update, owner_id, era)
            .await
        {
            return response;
        }
        let Some(task_router) = self.task_router else {
            // Reachable only for a store that does not accept inputs on a server
            // with no router: case 2 already answered for a server with NO task
            // backend at all. The FROZEN sibling message rather than a fifth
            // `-32601` sentence, so `the_minus_32601_conditions_are_mutually_distinct`
            // keeps describing the whole population.
            return error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            );
        };
        match task_router
            .handle_tasks_update(raw_params.clone(), owner_id)
            .await
        {
            Ok(_) => update_ack(id),
            Err(e) => error_response(
                id,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                e.to_string(),
            ),
        }
    }

    /// The STORE leg of [`Self::deliver_tasks_update`].
    ///
    /// `Some(response)` — the store answered, for better or worse. `None` — fall
    /// through to the router: either there is no store, or it does not accept
    /// inputs, or it answered `NotFound` while a router is configured.
    ///
    /// Split out so each of the route's seven steps is one short function: this
    /// file is where the phase's cognitive complexity concentrates and the
    /// PR-blocking gate is at 25.
    async fn deliver_update_through_store(
        &self,
        id: RequestId,
        update: &TasksUpdateParams<'_>,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<JSONRPCResponse> {
        let store = self.task_store.as_ref()?;
        if !store.supports_inputs() {
            return None;
        }
        // The OWNER-SCOPED read, before anything on the request is trusted. It
        // goes through 114-04's snapshot accessor because that is the ONLY
        // supported way to reach the server-recorded kinds: `TaskStore::get`
        // returns the wire `Task` alone and `TaskRecord` is private.
        let snapshot = match store.task_input_snapshot(&update.task_id, owner_id).await {
            Ok(snapshot) => snapshot,
            Err(e) => return self.store_error_or_fall_through(id, &e, era),
        };
        let typed = match Self::decode_inputs_against_record(update.input_responses, &snapshot) {
            Ok(typed) => typed,
            Err(refusal) => {
                return Some(error_response(
                    id,
                    crate::types::protocol::error_codes::INVALID_PARAMS,
                    refusal.to_string(),
                ))
            },
        };
        match store
            .deliver_task_inputs(&update.task_id, owner_id, typed)
            .await
        {
            Ok(_delivery) => Some(update_ack(id)),
            Err(e) => self.store_error_or_fall_through(id, &e, era),
        }
    }

    /// A store error either ANSWERS or falls through to the router.
    ///
    /// `NotFound` with a router configured is the store saying "not mine" — the
    /// same signal [`Self::handle_tasks_result`] falls through on. Everything else
    /// (including `NotFound` with no router) is answered through the shared
    /// era-aware [`store_error_response`], so a not-found task on v2 gets the ONE
    /// oracle-free `-32602` that is identical for absent, wrong-owner and expired
    /// (T-114-73), and a terminal task's `InvalidTransition` keeps its `-32603`.
    fn store_error_or_fall_through(
        &self,
        id: RequestId,
        error: &TaskStoreError,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<JSONRPCResponse> {
        if matches!(error, TaskStoreError::NotFound { .. }) && self.task_router.is_some() {
            return None;
        }
        Some(store_error_response(id, error, era))
    }

    /// Did this request DECLARE the tasks extension — case 3's predicate?
    ///
    /// Read from the ALREADY-RESOLVED [`ProtocolContext`] that Phase 112 binds
    /// once at ingress, never by re-parsing `params._meta` here. A second read
    /// is the drift Phase 113 kept finding.
    ///
    /// v1 is exempt and always answers `true`: the declaration mechanism is the
    /// v2 `_meta.clientCapabilities` channel, and v1 clients have no way to send
    /// one. Requiring it there would retire `tasks/*` on v1, which is precisely
    /// what this phase promises not to do.
    ///
    /// The declared capabilities are CLIENT-SUPPLIED and trivially forgeable, so
    /// this is a NEGOTIATION check, never an access decision — it answers "is
    /// this method available to you as configured", and the identity table
    /// (case 4) is the only control that answers "who are you".
    fn declares_tasks_extension(
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> bool {
        if is_v1_task_era(era) {
            return true;
        }
        protocol_context
            .and_then(|context| context.client_capabilities.as_ref())
            .and_then(|capabilities| capabilities.extensions.as_ref())
            .is_some_and(|extensions| {
                extensions.contains_key(crate::types::capabilities::TASKS_EXTENSION_KEY)
            })
    }

    /// The `tasks/*` method name IFF this request's method is RETIRED on `era`
    /// — case 1 of [`Self::route_tasks_endpoint`]'s chain.
    ///
    /// A dispatch TABLE over the two EXISTING era predicates, not a third era
    /// decision: `tasks/list` and `tasks/result` each keep exactly one predicate
    /// (so a negative control that disables one fails only that method's
    /// probes), and `tasks/get`/`tasks/cancel` survive on both eras so neither
    /// has one at all.
    fn retired_method(
        request: &ClientRequest,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<&'static str> {
        match request {
            ClientRequest::TasksList(_) if !tasks_list_serves_on_era(era) => {
                Some(TASKS_LIST_METHOD)
            },
            ClientRequest::TasksResult(_) if !tasks_result_serves_on_era(era) => {
                Some(TASKS_RESULT_METHOD)
            },
            _ => None,
        }
    }

    /// The method string a `tasks/*` request names, for a refusal message.
    ///
    /// Every spelling is read from an existing constant, never re-typed here.
    fn method_of(request: &ClientRequest) -> &'static str {
        match request {
            ClientRequest::TasksGet(_) => crate::types::mrtr::TASKS_GET_METHOD,
            ClientRequest::TasksResult(_) => TASKS_RESULT_METHOD,
            ClientRequest::TasksList(_) => TASKS_LIST_METHOD,
            ClientRequest::TasksCancel(_) => crate::types::mrtr::TASKS_CANCEL_METHOD,
            _ => NOT_A_TASKS_METHOD,
        }
    }
}

#[cfg(test)]
// Test-ergonomic helpers: `///` summaries name gate-table inputs by their literal
// arg/enum spelling (clippy::doc_markdown), and the `store_backend()` helper always
// returns `Some` by design so each test reads as a backend-present row
// (clippy::unnecessary_wraps). Both are noise in a truth-table test module.
#[allow(clippy::doc_markdown, clippy::unnecessary_wraps)]
mod gate_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::protocol::{Era, ProtocolContext};
    use crate::types::RequestId;

    fn store_backend() -> Option<Arc<dyn TaskStore>> {
        Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>)
    }

    fn task_shaped_value() -> Value {
        serde_json::json!({
            "taskId": "tool-fabricated",
            "status": "completed",
            "result": { "content": [{ "type": "text", "text": "done" }] }
        })
    }

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    /// The v1 trigger — the request carried a `task` field, or it did not.
    const fn v1_trigger(task_field_present: bool) -> CreateTrigger {
        CreateTrigger::V1TaskField { task_field_present }
    }

    /// The already-resolved [`ProtocolContext`] a real ingress hands the create
    /// path, at `era`, with or without the tasks-extension declaration.
    ///
    /// `declares` maps onto `clientCapabilities.extensions` through the SHARED
    /// key constant and the SHARED value helper, so a fixture cannot declare the
    /// extension by a spelling production would not recognise.
    fn context(era: Era, declares: bool) -> ProtocolContext {
        let version = match era {
            Era::V2 => crate::types::protocol::PROTOCOL_VERSION_2026_07_28,
            Era::V1 => crate::LATEST_PROTOCOL_VERSION,
        };
        let context = ProtocolContext::new(era, crate::types::ProtocolVersion(version.to_string()));
        if !declares {
            return context;
        }
        let mut extensions = HashMap::new();
        extensions.insert(TASKS_EXTENSION_KEY.to_string(), tasks_extension_value());
        context.with_client_capabilities(crate::types::ClientCapabilities {
            extensions: Some(extensions),
            ..crate::types::ClientCapabilities::default()
        })
    }

    /// Resolve the trigger the way BOTH dispatchers do: through
    /// [`CreateTrigger::resolve`], never by hand-picking a variant.
    ///
    /// That is the point of these five rows — a test that constructed the
    /// variant directly would prove the gate reads its input, not that the ERA
    /// picks the right input.
    fn resolved_trigger(era: Era, task_field_present: bool, declares: bool) -> CreateTrigger {
        let context = context(era, declares);
        CreateTrigger::resolve(Some(era), task_field_present, Some(&context))
    }

    /// task_requested == false → None regardless of other inputs.
    #[tokio::test]
    async fn gate_rejects_when_not_task_requested() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Required),
                v1_trigger(false),
                None,
                None,
            )
            .await;
        assert!(out.is_none(), "task_requested=false must yield None");
    }

    /// task_requested == true but no backend → None.
    #[tokio::test]
    async fn gate_rejects_when_no_backend() {
        let store = None;
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Required),
                v1_trigger(true),
                None,
                None,
            )
            .await;
        assert!(out.is_none(), "no backend must yield None");
    }

    /// task_requested, backend, TaskSupport::Forbidden → None (no error leak).
    #[tokio::test]
    async fn gate_rejects_forbidden_no_error_leak() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Forbidden),
                v1_trigger(true),
                None,
                None,
            )
            .await;
        assert!(out.is_none(), "Forbidden must yield None, never an error");
    }

    /// task_requested, backend, TaskSupport::None → None.
    #[tokio::test]
    async fn gate_rejects_no_task_support() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, None, v1_trigger(true), None, None)
            .await;
        assert!(out.is_none(), "no task_support must yield None");
    }

    /// Required-with-backend, value missing taskId/status → None.
    #[tokio::test]
    async fn gate_rejects_non_task_shaped_value() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = serde_json::json!({ "foo": "bar" });
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Required),
                v1_trigger(true),
                None,
                None,
            )
            .await;
        assert!(out.is_none(), "non-task-shaped value must yield None");
    }

    /// Assert the Some-case three-way store-minted-id invariant on an envelope.
    fn assert_store_minted(resp: &JSONRPCResponse) {
        let ResponsePayload::Result(value) = &resp.payload else {
            panic!("expected a success result envelope");
        };
        let wire_task_id = value
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("task.taskId present");
        let meta_id = value
            .get("_meta")
            .and_then(|m| m.get(RELATED_TASK_META_KEY))
            .and_then(|r| r.get("taskId"))
            .and_then(Value::as_str)
            .expect("_meta.relatedTask.taskId present");
        assert_eq!(
            wire_task_id, meta_id,
            "three-way invariant: task.taskId == _meta.relatedTask.taskId"
        );
        assert_ne!(
            wire_task_id, "tool-fabricated",
            "wire id must be store-minted, not the tool-fabricated id"
        );
    }

    /// task_requested, backend, TaskSupport::Optional, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_optional_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Optional),
                v1_trigger(true),
                None,
                None,
            )
            .await;
        let (resp, claim) = out.expect("Optional + task-shaped must yield Some");
        // These fixtures carry NO era, so the v1 create shape and its NONE claim
        // are what must come back (the v2 claim is proven in `create_claim_tests`).
        assert_eq!(claim, DispatchEnvelopeClaim::NONE);
        assert_store_minted(&resp);
    }

    /// task_requested, backend, TaskSupport::Required, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_required_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(
                id(),
                &value,
                Some(TaskSupport::Required),
                v1_trigger(true),
                None,
                None,
            )
            .await;
        let (resp, claim) = out.expect("Required + task-shaped must yield Some");
        assert_eq!(claim, DispatchEnvelopeClaim::NONE);
        assert_store_minted(&resp);
    }

    // =======================================================================
    // The ERA-AWARE trigger (plan 114-12, DQ1). One row per behaviour, named
    // for the row it proves. Each asserts the SHARED `create_gate` verdict, so
    // the assertion is on the predicate BOTH dispatchers reach — not on one
    // dispatcher's rendering of it.
    // =======================================================================

    /// Ask the SHARED gate with a store, `TaskSupport::Required` and a
    /// task-shaped value, varying ONLY the trigger.
    fn gate_with(trigger: CreateTrigger) -> CreateGate {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        dispatch.create_gate(trigger, Some(TaskSupport::Required), &task_shaped_value())
    }

    /// v2: the client DECLARED the tasks extension → the gate OPENS.
    ///
    /// This is the row that makes v2 task creation reachable at all. Without it
    /// no v2 `tools/call` could ever produce a `CreateTaskResult`.
    #[test]
    fn v2_gate_opens_on_a_client_declaration() {
        assert_eq!(
            gate_with(resolved_trigger(Era::V2, false, true)),
            CreateGate::Create,
            "a declaring v2 client must be able to receive a task handle"
        );
    }

    /// v2: the client did NOT declare → the gate stays CLOSED.
    ///
    /// The extension's own precondition: a server MUST NOT return a
    /// `CreateTaskResult` to a client that never declared the extension.
    #[test]
    fn v2_gate_rejects_a_non_declaring_client() {
        assert_eq!(
            gate_with(resolved_trigger(Era::V2, false, false)),
            CreateGate::Closed,
            "a non-declaring v2 client must never receive a task handle"
        );
    }

    /// v2: a `task` field WITHOUT a declaration does NOT create.
    ///
    /// The v1 trigger is INERT on v2 — the field does not exist in the v2
    /// extension, so carrying it must buy the caller nothing.
    #[test]
    fn v2_gate_ignores_the_v1_task_field() {
        assert_eq!(
            gate_with(resolved_trigger(Era::V2, true, false)),
            CreateGate::Closed,
            "the v1 `task` field must not open the v2 gate"
        );
    }

    /// v1: the `task` field is still REQUIRED. BYTE-FROZEN behaviour.
    #[test]
    fn v1_gate_still_requires_the_task_field() {
        assert_eq!(
            gate_with(resolved_trigger(Era::V1, true, false)),
            CreateGate::Create,
            "v1 creation is triggered by the `task` field, exactly as before"
        );
        assert_eq!(
            gate_with(resolved_trigger(Era::V1, false, false)),
            CreateGate::Closed,
            "v1 without a `task` field must still fall through"
        );
    }

    /// v1: a declaration WITHOUT a `task` field does NOT create.
    ///
    /// The mirror of [`v2_gate_ignores_the_v1_task_field`]: each era ignores the
    /// other's trigger, so a v1 client that somehow carried a v2-style
    /// declaration cannot gain a task it did not ask for.
    #[test]
    fn v1_gate_ignores_a_client_declaration() {
        assert_eq!(
            gate_with(resolved_trigger(Era::V1, false, true)),
            CreateGate::Closed,
            "a declaration must not open the v1 gate"
        );
    }

    /// The gate distinguishes "not task-shaped" from "closed", which is what
    /// lets `ServerCore` keep its tool-authoring `debug!` without keeping its
    /// own copy of the shape check.
    #[test]
    fn a_gate_open_but_unshaped_value_is_distinguishable_from_a_closed_gate() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let unshaped = serde_json::json!({ "foo": "bar" });
        assert_eq!(
            dispatch.create_gate(v1_trigger(true), Some(TaskSupport::Required), &unshaped),
            CreateGate::NotTaskShaped
        );
        assert_eq!(
            dispatch.create_gate(v1_trigger(false), Some(TaskSupport::Required), &unshaped),
            CreateGate::Closed
        );
    }
}

/// The era-aware owner binding (plan 114-09, TASK-05, D-07).
///
/// One test per ROW of the v2 identity table, each named for the row it proves,
/// plus the v1 freeze and its D-10 migration warn. These are the UNIT half; the
/// ordered refusal chain is measured over a real socket in
/// `tests/v2_tasks_owner_binding.rs`, and the cross-caller proof is 114-15.
#[cfg(test)]
mod owner_binding_tests {
    use super::*;
    use crate::server::auth::AuthContext;
    use crate::server::core::ANONYMOUS_PRINCIPAL;
    use crate::types::protocol::Era;

    /// Bind an owner at `era` with the given identity inputs.
    ///
    /// Deliberately backend-LESS: the v2 table reads only
    /// `authenticated_subject` and `has_auth_provider`, and the v1 arm's
    /// store/router branches are already covered by `gate_tests` and
    /// `era_gate_tests`. Keeping the fixture minimal is what makes each
    /// assertion below attributable to exactly one row.
    fn bind(subject: Option<&str>, has_auth_provider: bool, era: Option<Era>) -> OwnerBinding {
        let store = None;
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider,
        };
        let auth = subject.map(AuthContext::new);
        dispatch.resolve_owner(auth.as_ref(), era)
    }

    /// Row 1: an authenticated subject IS the owner, on either auth posture.
    ///
    /// `has_auth_provider` is the "any" column of the table, so both values are
    /// asserted — a row-1 implementation that accidentally read the flag would
    /// pass a single-value test.
    #[test]
    fn v2_owner_is_the_authenticated_subject() {
        for has_auth_provider in [true, false] {
            assert_eq!(
                bind(Some("user-alice"), has_auth_provider, Some(Era::V2)),
                OwnerBinding::Owner("user-alice".to_string()),
                "row 1 must bind the OAuth subject verbatim, \
                 has_auth_provider={has_auth_provider}"
            );
        }
    }

    /// Row 2, the FAIL-CLOSED row: unauthenticated + an auth provider → refused.
    ///
    /// This is TASK-05's central control. If it ever returns an `Owner`, an
    /// anonymous caller can mint and read tasks on a server that expects
    /// authentication (T-114-37).
    #[test]
    fn v2_unauthenticated_with_auth_provider_is_refused() {
        assert_eq!(
            bind(None, true, Some(Era::V2)),
            OwnerBinding::Refused,
            "row 2 must refuse: no subject on an auth-configured server binds NO owner"
        );
    }

    /// Row 3: unauthenticated on a server with NO auth provider → the shared
    /// anonymous bucket, NOT a refusal.
    ///
    /// The counterweight to row 2: a fail-closed change must not be satisfiable
    /// by refusing everyone, which would break every stdio/dev server.
    #[test]
    fn v2_unauthenticated_without_auth_provider_is_anonymous() {
        assert_eq!(
            bind(None, false, Some(Era::V2)),
            OwnerBinding::Owner(ANONYMOUS_PRINCIPAL.to_string()),
            "row 3 must bind the NAMED anonymous principal, not refuse"
        );
    }

    /// The v2 anonymous bucket and the v1 `"local"` bucket are DISJOINT keys.
    ///
    /// `GenericTaskStore::is_anonymous_owner` treats the two identically for the
    /// `allow_anonymous` decision, but `make_key` prefixes every record by owner
    /// — so these two facts are separate and easy to conflate. Asserting the
    /// inequality here stops a future "simplify the two fallbacks" change from
    /// silently merging two key spaces.
    #[test]
    fn the_v1_and_v2_unauthenticated_buckets_are_different_keys() {
        assert_ne!(
            ANONYMOUS_PRINCIPAL, V1_UNAUTHENTICATED_OWNER,
            "the v1 and v2 unauthenticated owners must remain distinct key prefixes"
        );
    }

    /// The v1 arm is FROZEN: an unauthenticated caller still binds `"local"`,
    /// on an explicit v1 era AND on a request carrying no era code at all.
    #[test]
    fn v1_unauthenticated_owner_is_still_local() {
        for era in [Some(Era::V1), None] {
            for has_auth_provider in [true, false] {
                assert_eq!(
                    bind(None, has_auth_provider, era),
                    OwnerBinding::Owner(V1_UNAUTHENTICATED_OWNER.to_string()),
                    "v1 owner binding is frozen and NEVER refuses: \
                     era={era:?}, has_auth_provider={has_auth_provider}"
                );
            }
        }
    }

    /// v1 with a subject still binds that subject — the freeze is not "always
    /// local".
    #[test]
    fn v1_authenticated_owner_is_still_the_subject() {
        assert_eq!(
            bind(Some("user-bob"), true, Some(Era::V1)),
            OwnerBinding::Owner("user-bob".to_string()),
            "v1 store-path owner binding is the OAuth subject, unchanged"
        );
    }

    /// D-10's migration warn fires EXACTLY once per unauthenticated v1
    /// resolution, and NOT for an authenticated one.
    ///
    /// Counted with a hand-rolled `tracing::Subscriber` rather than
    /// `tracing-subscriber`, which is an OPTIONAL dependency behind the
    /// `logging` feature — this assertion must hold under every feature set the
    /// gate builds.
    #[test]
    fn the_v1_migration_warn_fires_once_per_unauthenticated_resolution() {
        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);

        tracing::subscriber::with_default(counter, || {
            assert_eq!(
                bind(None, false, Some(Era::V1)),
                OwnerBinding::Owner(V1_UNAUTHENTICATED_OWNER.to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "exactly one migration warn per unauthenticated v1 owner resolution"
        );

        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);
        tracing::subscriber::with_default(counter, || {
            assert_eq!(
                bind(Some("user-carol"), false, Some(Era::V1)),
                OwnerBinding::Owner("user-carol".to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "an AUTHENTICATED v1 caller is not in the shared bucket and must not be warned about"
        );
    }

    /// A v2 resolution never emits the v1 migration warn — the two arms are
    /// genuinely separate, not one arm with a flag.
    #[test]
    fn the_migration_warn_is_v1_only() {
        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);
        tracing::subscriber::with_default(counter, || {
            assert_eq!(bind(None, true, Some(Era::V2)), OwnerBinding::Refused);
            assert_eq!(
                bind(None, false, Some(Era::V2)),
                OwnerBinding::Owner(ANONYMOUS_PRINCIPAL.to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the D-10 migration warn is about v1's shared bucket and must not fire on v2"
        );
    }

    /// Counts WARN-level events, and nothing else.
    ///
    /// Hand-rolled against `tracing`'s core `Subscriber` trait so the assertion
    /// has no optional-dependency footprint (see the test above).
    #[derive(Default)]
    struct WarnCounter {
        warnings: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl tracing::Subscriber for WarnCounter {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() == tracing::Level::WARN
        }
        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::Id {
            tracing::Id::from_u64(1)
        }
        fn record(&self, _span: &tracing::Id, _values: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _span: &tracing::Id, _follows: &tracing::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            if *event.metadata().level() == tracing::Level::WARN {
                self.warnings
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        fn enter(&self, _span: &tracing::Id) {}
        fn exit(&self, _span: &tracing::Id) {}
    }
}

/// The v2 retirement of `tasks/list` and `tasks/result` (plan 114-08, TASK-03).
///
/// One `#[tokio::test]` per row of the per-method era matrix, each named for the
/// row it proves — the shape `gate_tests` above established. The live-socket
/// half, with a negative control per gate, is `tests/v2_tasks_era_gates.rs`.
#[cfg(test)]
// Why: `store_backend()` always returns `Some` BY DESIGN, so each caller reads
// as a backend-present row (clippy::unnecessary_wraps); and `route()` takes
// `&Option<Arc<dyn TaskStore>>` because that is the type
// `TaskDispatch::task_store` borrows — a helper taking `Option<&T>` could not
// construct the production struct at all (clippy::ref_option). Both are the same
// truth-table-test noise `gate_tests` above already allows.
#[allow(clippy::unnecessary_wraps, clippy::ref_option)]
mod era_gate_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::protocol::error_codes::{METHOD_NOT_FOUND, V1_TASK_PENDING};
    use crate::types::protocol::Era;
    use crate::types::RequestId;

    /// Every era value a request can carry, in truth-table order.
    const ERAS: [(Option<Era>, bool); 3] =
        [(Some(Era::V1), true), (None, true), (Some(Era::V2), false)];

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    fn store_backend() -> Option<Arc<dyn TaskStore>> {
        Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>)
    }

    fn list_request() -> ClientRequest {
        ClientRequest::TasksList(crate::types::tasks::ListTasksRequest { cursor: None })
    }

    fn result_request() -> ClientRequest {
        ClientRequest::TasksResult(crate::types::tasks::GetTaskPayloadRequest {
            task_id: "absent".to_string(),
        })
    }

    /// A [`ClientCapabilities`] that DECLARES the tasks extension, spelled
    /// through the shared key constant.
    fn tasks_declaring_capabilities() -> crate::types::ClientCapabilities {
        let mut extensions = HashMap::new();
        extensions.insert(TASKS_EXTENSION_KEY.to_string(), tasks_extension_value());
        crate::types::ClientCapabilities {
            extensions: Some(extensions),
            ..crate::types::ClientCapabilities::default()
        }
    }

    /// The already-resolved [`ProtocolContext`] for `era`, DECLARING the tasks
    /// extension.
    ///
    /// The declaration is deliberate: this module measures the ERA gates, so
    /// every fixture must clear the extension-declaration gate
    /// ([`missing_tasks_declaration_refusal`]) or a `-32021` would masquerade as
    /// a retirement. `None` reproduces the "no era code at all" row.
    fn context_for(era: Era) -> crate::types::protocol::ProtocolContext {
        let version = match era {
            Era::V2 => crate::types::protocol::PROTOCOL_VERSION_2026_07_28,
            Era::V1 => crate::LATEST_PROTOCOL_VERSION,
        };
        crate::types::protocol::ProtocolContext::new(
            era,
            crate::types::ProtocolVersion(version.to_string()),
        )
        .with_client_capabilities(tasks_declaring_capabilities())
    }

    /// Drive one `tasks/*` request through the real router at one era.
    async fn route(
        store: &Option<Arc<dyn TaskStore>>,
        request: &ClientRequest,
        era: Option<Era>,
    ) -> JSONRPCResponse {
        let router = None;
        let dispatch = TaskDispatch {
            task_store: store,
            task_router: &router,
            has_auth_provider: false,
        };
        let context = era.map(context_for);
        // These rows are about the ERA GATES, not the envelope claim; the claim
        // is asserted by its own module below.
        dispatch
            .route_tasks_endpoint(id(), request, None, context.as_ref())
            .await
            .0
    }

    /// The `(code, message)` of an error response, or `None` for a success.
    fn error_of(response: &JSONRPCResponse) -> Option<(i32, String)> {
        match &response.payload {
            ResponsePayload::Error(error) => Some((error.code, error.message.clone())),
            ResponsePayload::Result(_) => None,
        }
    }

    /// `tasks/list` serves on v1 and on an era-less request, and not on v2.
    #[test]
    fn tasks_list_era_truth_table() {
        for (era, expected) in ERAS {
            assert_eq!(
                tasks_list_serves_on_era(era),
                expected,
                "tasks/list serving decision for era {era:?}"
            );
        }
    }

    /// `tasks/result` serves on v1 and on an era-less request, and not on v2.
    #[test]
    fn tasks_result_era_truth_table() {
        for (era, expected) in ERAS {
            assert_eq!(
                tasks_result_serves_on_era(era),
                expected,
                "tasks/result serving decision for era {era:?}"
            );
        }
    }

    /// A v2 `tasks/list` is `-32601` with the RETIRED message and enumerates
    /// nothing.
    #[tokio::test]
    async fn v2_tasks_list_is_retired() {
        let store = store_backend();
        let response = route(&store, &list_request(), Some(Era::V2)).await;

        let (code, message) = error_of(&response).expect("a v2 tasks/list must be refused");
        assert_eq!(code, METHOD_NOT_FOUND, "message was {message}");
        assert!(
            message.starts_with(TASKS_LIST_METHOD) && message.contains(V2_TASKS_METHOD_RETIRED),
            "the refusal must name the method AND the retirement: {message}"
        );
    }

    /// A v2 `tasks/result` is `-32601` with the RETIRED message and never the
    /// spec-prohibited `-32002`.
    #[tokio::test]
    async fn v2_tasks_result_is_retired() {
        let store = store_backend();
        let response = route(&store, &result_request(), Some(Era::V2)).await;

        let (code, message) = error_of(&response).expect("a v2 tasks/result must be refused");
        assert_eq!(code, METHOD_NOT_FOUND, "message was {message}");
        assert_ne!(
            code, V1_TASK_PENDING,
            "protocol version 2026-07-28 MUST NOT emit -32002: {message}"
        );
        assert!(
            message.starts_with(TASKS_RESULT_METHOD) && message.contains(V2_TASKS_METHOD_RETIRED),
            "the refusal must name the method AND the retirement: {message}"
        );
    }

    /// The v1 side of the same two gates is untouched: `tasks/list` still
    /// enumerates and `tasks/result` still emits the FROZEN `-32002` with its
    /// existing message.
    #[tokio::test]
    async fn v1_list_and_result_are_unchanged() {
        let store = store_backend();

        let listed = route(&store, &list_request(), Some(Era::V1)).await;
        let ResponsePayload::Result(value) = &listed.payload else {
            panic!("a v1 tasks/list must still serve: {:?}", listed.payload);
        };
        assert!(
            value.get("tasks").is_some_and(Value::is_array),
            "a v1 tasks/list result still carries the tasks array: {value}"
        );

        let pending = route(&store, &result_request(), Some(Era::V1)).await;
        assert_eq!(
            error_of(&pending),
            Some((
                V1_TASK_PENDING,
                "task result not available: task not completed".to_string()
            )),
            "the v1 pending refusal is FROZEN, code and message"
        );
    }

    /// A server with NO backend keeps its "not enabled" / "not supported"
    /// answers on v2, and they are DIFFERENT strings from the RETIRED message.
    ///
    /// This is what makes the two-message split observable rather than
    /// cosmetic: a caller that hits the no-backend answer must not be told a
    /// method was retired, because the fix is to configure a backend.
    #[tokio::test]
    async fn a_backendless_v2_server_is_not_told_the_methods_were_retired() {
        let store = None;

        let listed = route(&store, &list_request(), Some(Era::V2)).await;
        let (list_code, list_message) = error_of(&listed).expect("no backend refuses tasks/list");
        assert_eq!(list_code, METHOD_NOT_FOUND, "message was {list_message}");
        assert_eq!(list_message, "Tasks not enabled");

        let resulted = route(&store, &result_request(), Some(Era::V2)).await;
        let (result_code, result_message) =
            error_of(&resulted).expect("no backend refuses tasks/result");
        assert_eq!(
            result_code, METHOD_NOT_FOUND,
            "message was {result_message}"
        );
        assert_eq!(result_message, "tasks/result not supported");

        for message in [&list_message, &result_message] {
            assert!(
                !message.contains(V2_TASKS_METHOD_RETIRED),
                "a no-backend refusal must not claim a retirement: {message}"
            );
        }
        assert_ne!(
            list_message, result_message,
            "the two no-backend refusals are themselves distinguishable"
        );
    }
}

/// The v2 arm of [`apply_tasks_capability_rule`] (plan 114-05, D-01/D-03).
///
/// One test per row of the additive-only truth table, each named for the row it
/// proves. Every test uses NO tools, so the `TaskSupport::Required` validation
/// branch is out of the way and each assertion is about the capability writes
/// alone.
#[cfg(test)]
mod capability_rule_tests {
    use super::*;

    fn no_tools() -> HashMap<String, ToolInfo> {
        HashMap::new()
    }

    /// Read the tasks-extension entry, if any.
    fn tasks_entry(capabilities: &ServerCapabilities) -> Option<&Value> {
        capabilities
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(TASKS_EXTENSION_KEY))
    }

    /// A backend-configured server gains the extension entry, and its value is
    /// EXACTLY `{}` — not merely present.
    ///
    /// Equality with `{}` rather than `is_some()` is the assertion that fails if
    /// a future change starts projecting `default_tasks_capability()`'s
    /// `list`/`cancel`/`requests` flags into the extension value: advertising
    /// `list: true` on an era where `tasks/list` answers `-32601` is the
    /// capability lie D-03 forbids.
    #[test]
    fn capability_rule_advertises_the_tasks_extension_when_a_backend_exists() {
        let mut capabilities = ServerCapabilities::default();
        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        assert_eq!(
            tasks_entry(&capabilities),
            Some(&serde_json::json!({})),
            "a backend-configured server must advertise the tasks extension as \
             the EMPTY OBJECT (D-03): {capabilities:?}"
        );
        // The v1 arm is unchanged by the v2 arm — one knob, two advertisements.
        assert!(
            capabilities.tasks.is_some(),
            "the v1 tasks capability must still be auto-advertised: {capabilities:?}"
        );
    }

    /// An EXPLICITLY configured extension value survives the rule byte-unchanged.
    ///
    /// This is the extensions-map twin of the `capabilities.tasks.is_none()`
    /// guard: an operator-supplied value is the operator's, and silently
    /// rewriting it would be worse than serving it.
    #[test]
    fn capability_rule_preserves_an_explicitly_configured_tasks_extension_value() {
        let explicit = serde_json::json!({ "io.example/nonconformant": true });
        let mut capabilities = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(TASKS_EXTENSION_KEY.to_string(), explicit.clone());
        capabilities.extensions = Some(extensions);

        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        assert_eq!(
            serde_json::to_string(tasks_entry(&capabilities).expect("entry present")).unwrap(),
            serde_json::to_string(&explicit).unwrap(),
            "an explicitly configured extension value must survive the rule \
             byte-unchanged: {capabilities:?}"
        );
    }

    /// A server with NO task backend gains neither the v1 capability nor the v2
    /// extension entry.
    ///
    /// The endpoint-backed rule's whole point: presence of the key is a promise
    /// that `tasks/*` works, so a backend-less server must make no such promise
    /// on either era.
    #[test]
    fn capability_rule_advertises_nothing_without_a_backend() {
        let mut capabilities = ServerCapabilities::default();
        apply_tasks_capability_rule(&mut capabilities, &no_tools(), false).unwrap();

        assert!(
            capabilities.tasks.is_none(),
            "no backend must mean no v1 tasks capability: {capabilities:?}"
        );
        assert_eq!(
            tasks_entry(&capabilities),
            None,
            "no backend must mean no v2 extension entry: {capabilities:?}"
        );
        assert!(
            capabilities.extensions.is_none(),
            "and the rule must not manufacture an empty extensions map: {capabilities:?}"
        );
    }

    /// An unrelated pre-existing extensions key is still present afterwards.
    ///
    /// The insert is alongside, never a replacement of the map.
    #[test]
    fn capability_rule_leaves_an_unrelated_extensions_key_intact() {
        let mut capabilities = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(
            "io.example/experimental".to_string(),
            serde_json::json!({ "enabled": true }),
        );
        capabilities.extensions = Some(extensions);

        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        let extensions = capabilities.extensions.as_ref().expect("map present");
        assert_eq!(
            extensions.get("io.example/experimental"),
            Some(&serde_json::json!({ "enabled": true })),
            "an unrelated extensions key must survive: {extensions:?}"
        );
        assert_eq!(
            extensions.get(TASKS_EXTENSION_KEY),
            Some(&serde_json::json!({})),
            "and the tasks entry lands alongside it: {extensions:?}"
        );
    }
}

/// The v2 result surface: shapes and the era-aware store-error mapping
/// (plan 114-11, TASK-04, inventory rows 16/18/20/29).
///
/// The UNIT half. Every one of these facts is ALSO proven over a real socket in
/// `tests/v2_tasks_shapes.rs`; these exist because two of them — a non-NotFound
/// store error, and the identity of the absent-vs-wrong-owner message — cannot
/// be provoked from outside the process without a fault-injecting store.
#[cfg(test)]
mod v2_shape_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::protocol::error_codes::{INTERNAL_ERROR, INVALID_PARAMS};
    use crate::types::protocol::Era;

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    /// The four `TaskStoreError` variants, each carrying the SAME task id so a
    /// message that echoed it would be visible in every row.
    fn all_store_errors() -> Vec<TaskStoreError> {
        vec![
            TaskStoreError::NotFound {
                task_id: "task-abc".to_string(),
            },
            TaskStoreError::Expired {
                task_id: "task-abc".to_string(),
            },
            TaskStoreError::InvalidTransition {
                task_id: "task-abc".to_string(),
                from: TaskStatus::Completed,
                to: TaskStatus::Working,
            },
            TaskStoreError::Internal {
                message: "backend unavailable".to_string(),
            },
        ]
    }

    fn error_of(response: &JSONRPCResponse) -> (i32, String) {
        match &response.payload {
            ResponsePayload::Error(error) => (error.code, error.message.clone()),
            ResponsePayload::Result(value) => {
                panic!("expected an error response, got a result: {value}")
            },
        }
    }

    fn result_of(response: &JSONRPCResponse) -> Value {
        match &response.payload {
            ResponsePayload::Result(value) => value.clone(),
            ResponsePayload::Error(error) => {
                panic!(
                    "expected a success result, got {}: {}",
                    error.code, error.message
                )
            },
        }
    }

    /// v1 is FROZEN: every store error is `-32603` carrying the error's own
    /// message, on both the explicit-v1 and the no-era-code rows.
    #[test]
    fn v1_store_errors_are_all_internal_error() {
        for era in [Some(Era::V1), None] {
            for error in all_store_errors() {
                let (code, message) = error_of(&store_error_response(id(), &error, era));
                assert_eq!(code, INTERNAL_ERROR, "{era:?} / {error}");
                assert_eq!(message, error.to_string(), "{era:?} / {error}");
            }
        }
    }

    /// v2 maps not-found and expired onto `-32602`; everything else stays
    /// `-32603` (inventory row 29).
    #[test]
    fn v2_maps_only_not_found_and_expired_to_invalid_params() {
        let expected = [
            (INVALID_PARAMS, true),
            (INVALID_PARAMS, true),
            (INTERNAL_ERROR, false),
            (INTERNAL_ERROR, false),
        ];
        for (error, (code, is_not_found)) in all_store_errors().into_iter().zip(expected) {
            let (actual_code, message) =
                error_of(&store_error_response(id(), &error, Some(Era::V2)));
            assert_eq!(actual_code, code, "{error}");
            if is_not_found {
                assert_eq!(message, V2_TASK_NOT_FOUND_MESSAGE, "{error}");
            } else {
                assert_eq!(message, error.to_string(), "{error}");
            }
        }
    }

    /// The `-32602` answer is IDENTICAL for an absent id and for another owner's
    /// id — both surface as `NotFound`, and a message that distinguished them
    /// would be the existence oracle the owner-prefixed key design closes
    /// (T-114-50).
    #[tokio::test]
    async fn the_v2_not_found_answer_is_identical_for_absent_and_wrong_owner() {
        let store = InMemoryTaskStore::new();
        let owned = store.create("owner-a", None).await.expect("creates");

        // Another owner asking for a task that DOES exist.
        let wrong_owner = store
            .get(&owned.task_id, "owner-b")
            .await
            .expect_err("another owner must not read it");
        // Anyone asking for a task that does not exist.
        let absent = store
            .get("no-such-task", "owner-b")
            .await
            .expect_err("an absent task must not be found");

        let a = error_of(&store_error_response(id(), &wrong_owner, Some(Era::V2)));
        let b = error_of(&store_error_response(id(), &absent, Some(Era::V2)));
        assert_eq!(a, b, "the two refusals must be indistinguishable");
        assert!(
            !a.1.contains(&owned.task_id) && !a.1.contains("no-such-task"),
            "the refusal must not render a task id back: {}",
            a.1
        );
    }

    /// The `-32602` message never renders the requested id back, for EVERY id it
    /// could be handed — including one that looks like a log-injection payload.
    #[test]
    fn the_v2_not_found_message_never_echoes_the_task_id() {
        for task_id in [
            "task-abc",
            "\n2026-01-01 ERROR forged log line",
            "../../etc/passwd",
        ] {
            let error = TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            };
            let (_, message) = error_of(&store_error_response(id(), &error, Some(Era::V2)));
            assert!(
                !message.contains(task_id),
                "the id leaked into the refusal: {message}"
            );
            assert_eq!(message, V2_TASK_NOT_FOUND_MESSAGE);
        }
    }

    fn sample_task() -> Task {
        Task::new("t-1", TaskStatus::Working)
            .with_timestamps("2026-07-28T00:00:00Z", "2026-07-28T00:00:01Z")
            .with_ttl(60_000)
            .with_poll_interval(2500)
    }

    /// The v2 create body is FLAT: `taskId` at the top level, no `task` wrapper,
    /// the renamed keys, and the `_meta.relatedTask` envelope retained.
    #[test]
    fn the_v2_create_body_is_flat() {
        let value = v2_create_result_value(&sample_task(), "t-1");
        assert_eq!(value.get("taskId").and_then(Value::as_str), Some("t-1"));
        assert!(value.get("task").is_none(), "v2 must not wrap: {value}");
        assert_eq!(value.get("ttlMs").and_then(Value::as_u64), Some(60_000));
        assert_eq!(
            value.get("pollIntervalMs").and_then(Value::as_u64),
            Some(2500)
        );
        assert!(
            value.get("_meta").is_some(),
            "the relatedTask envelope stays"
        );
    }

    /// The v1 create body is the FROZEN nested envelope with the v1 key spellings.
    #[test]
    fn the_v1_create_body_is_still_nested() {
        let value = v1_create_result_value(&sample_task(), "t-1");
        let task = value.get("task").expect("v1 wraps under `task`");
        assert_eq!(task.get("taskId").and_then(Value::as_str), Some("t-1"));
        assert_eq!(task.get("ttl").and_then(Value::as_u64), Some(60_000));
        assert_eq!(task.get("pollInterval").and_then(Value::as_u64), Some(2500));
        let raw = value.to_string();
        assert!(
            !raw.contains("ttlMs"),
            "a v2 spelling leaked into v1: {raw}"
        );
        assert!(
            !raw.contains("pollIntervalMs"),
            "a v2 spelling leaked into v1: {raw}"
        );
    }

    /// The create path's ENVELOPE CLAIM is era-split: `resultType: "task"` on v2,
    /// nothing at all on v1.
    #[tokio::test]
    async fn the_create_claim_is_era_split() {
        let store = Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>);
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = serde_json::json!({
            "taskId": "tool-fabricated",
            "status": "working",
            "createdAt": "2026-07-28T00:00:00Z",
            "lastUpdatedAt": "2026-07-28T00:00:00Z"
        });
        let (_, v1_claim) = dispatch
            .build_task_created_response(id(), value.clone(), None, Some(Era::V1))
            .await;
        assert_eq!(v1_claim, DispatchEnvelopeClaim::NONE);
        let (response, v2_claim) = dispatch
            .build_task_created_response(id(), value, None, Some(Era::V2))
            .await;
        assert_eq!(v2_claim, DispatchEnvelopeClaim::TASK_CREATED);
        assert_eq!(
            v2_claim.disposition.as_wire_str(),
            "task",
            "the create claim is the ONLY source of `resultType: \"task\"`"
        );
        assert!(result_of(&response).get("taskId").is_some());
    }

    /// A v2 `tasks/get` on an `input_required` task inlines a TOP-LEVEL
    /// `inputRequests` AND claims ownership of it, so the reserved-field registry
    /// does not strip it (114-10 row 23). Every other status claims nothing.
    #[tokio::test]
    async fn the_get_claim_is_input_required_only() {
        let store_impl = Arc::new(InMemoryTaskStore::new());
        let store = Some(store_impl.clone() as Arc<dyn TaskStore>);
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let task = store_impl.create("owner-a", None).await.expect("creates");
        let params = crate::types::tasks::GetTaskRequest {
            task_id: task.task_id.clone(),
        };

        // working -> no claim.
        let (response, claim) = dispatch
            .route_tasks_get(id(), &params, "owner-a", Some(Era::V2))
            .await;
        assert_eq!(claim, DispatchEnvelopeClaim::NONE);
        assert!(result_of(&response).get("inputRequests").is_none());

        // input_required -> the TasksDispatch claim and the inlined key.
        let mut requests = crate::types::mrtr::InputRequests::new();
        requests.insert(
            "roots".to_string(),
            crate::types::mrtr::InputRequest::ListRoots,
        );
        store_impl
            .record_input_requests(&task.task_id, "owner-a", requests)
            .await
            .expect("records");
        let (response, claim) = dispatch
            .route_tasks_get(id(), &params, "owner-a", Some(Era::V2))
            .await;
        assert_eq!(claim, DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED);
        assert_eq!(
            claim.owner,
            crate::server::core::ReservedFieldOwner::TasksDispatch
        );
        assert_eq!(
            claim.disposition.as_wire_str(),
            "complete",
            "the REQUEST completed; it is the TASK that is waiting"
        );
        let value = result_of(&response);
        assert!(
            value.get("inputRequests").is_some(),
            "inputRequests must be TOP-LEVEL: {value}"
        );
        assert!(value.get("task").is_none(), "v2 must not wrap: {value}");
    }

    /// A router's `tasks/get` value is projected flat on v2, from either the
    /// nested or the bare shape, and passed through when it is not a task.
    #[test]
    fn the_router_get_value_is_projected_on_v2() {
        let nested = serde_json::json!({
            "task": {
                "taskId": "r-1",
                "status": "completed",
                "ttl": 1000,
                "createdAt": "2026-07-28T00:00:00Z",
                "lastUpdatedAt": "2026-07-28T00:00:01Z"
            },
            "result": { "content": [] }
        });
        let projected = v2_project_router_task(nested);
        assert_eq!(projected.get("taskId").and_then(Value::as_str), Some("r-1"));
        assert!(projected.get("task").is_none());
        assert_eq!(projected.get("ttlMs").and_then(Value::as_u64), Some(1000));
        assert!(projected.get("result").is_some(), "{projected}");

        let opaque = serde_json::json!({ "something": "else" });
        assert_eq!(
            v2_project_router_task(opaque.clone()),
            opaque,
            "an unparseable router value passes through rather than half-projecting"
        );
    }

    /// A backend that cannot supply the detail degrades to the BARE flat task
    /// rather than fabricating an empty required field.
    #[tokio::test]
    async fn a_detail_less_backend_degrades_rather_than_fabricating() {
        // `InMemoryTaskStore` records no input requests for this task, so the
        // snapshot read fails and there is genuinely nothing to inline.
        let mut task = Task::new("t-1", TaskStatus::InputRequired)
            .with_timestamps("2026-07-28T00:00:00Z", "2026-07-28T00:00:01Z");
        task.ttl = None;
        let value = v2_detailed_task_value(&task, None);
        assert_eq!(
            value.get("status").and_then(Value::as_str),
            Some("input_required")
        );
        assert!(
            value.get("inputRequests").is_none(),
            "an empty inputRequests would be a schema-valid lie: {value}"
        );
        assert!(
            value.get("ttlMs").is_some_and(Value::is_null),
            "the five required fields survive the degradation: {value}"
        );
    }
}

/// Minimal seam for `fuzz/fuzz_targets/fuzz_tasks_update.rs` (plan 114-14).
///
/// # ⚠️ Not stable API
///
/// The gate is `#[cfg(any(feature = "fuzzing", test))]` and that is the FENCE,
/// not a decoration. `#[doc(hidden)]` alone would not do: plan 113-19 measured
/// that `cargo public-api` OMITS `doc(hidden)` items, so a `doc(hidden)`-only
/// seam passes an absence check VACUOUSLY while remaining callable by every
/// downstream crate and still counting for semver. `fuzzing` is in neither
/// `default` nor `full`, so nothing a dependent can enable reaches this module.
///
/// # What it exposes, and why that is the boundary worth fuzzing
///
/// The RAW `tasks/update` params — the bytes an untrusted client supplies. That
/// is the whole attack surface of this route: `inputResponses` is the entire
/// request payload, it is the only large client-supplied structure on the path,
/// and its decode is the one place in the tasks surface where guessing at
/// overlapping shapes is actively wrong (D-113-O). It is the same reasoning that
/// put `fuzz_request_state` at the continuation-token boundary.
///
/// [`judge_update_params`] runs EXACTLY the route's pure prefix — parse, bound,
/// kind-directed decode — against a FIXED synthetic record, so a crash artifact
/// replays deterministically regardless of ambient process state. Everything
/// after that prefix is a store write, which no fuzz target should perform.
#[cfg(any(feature = "fuzzing", test))]
// Why: under `cfg(test)` ALONE — i.e. the ordinary `make lint --lib --tests` build
// — the parent module is still `pub(crate)`, so every `pub` item here is
// unreachable from outside the crate, and the four bound re-exports have no
// in-crate caller at all. Both are CORRECT: that half of the seam exists for the
// fuzz target, which builds with `feature = "fuzzing"`, where the parent IS `pub`
// and the target IS the caller. The allow is scoped to `not(fuzzing)` precisely so
// the real fuzz configuration still gets full dead-code and reachability analysis
// — the 114-10 discipline of scoping such an allow to the build where the
// condition genuinely holds, rather than to `not(test)`.
#[cfg_attr(not(feature = "fuzzing"), allow(unreachable_pub, dead_code))]
pub mod fuzz_support {
    use super::{TaskDispatch, TaskInputSnapshot};
    use crate::types::mrtr::{InputRequest, InputRequests, InputResponses};
    use crate::types::tasks::TaskStatus;
    use serde_json::Value;

    /// The params parsed, bounded and decoded cleanly.
    pub const VERDICT_ACCEPTED: u8 = 0;
    /// The bytes were not JSON, or were not a well-formed `tasks/update` params
    /// object (`-32602` before anything else looked at them).
    pub const VERDICT_MALFORMED: u8 = 1;
    /// A bound was exceeded. Reached BEFORE any decode, which is the property
    /// invariant 2 exists to keep true.
    pub const VERDICT_BOUNDED: u8 = 2;
    /// A recorded key's value did not decode as the kind the record holds.
    pub const VERDICT_REFUSED: u8 = 3;

    /// The synthetic record's `roots/list` key.
    pub const RECORDED_ROOTS_KEY: &str = "roots";
    /// The synthetic record's `elicitation/create` key.
    pub const RECORDED_ELICITATION_KEY: &str = "form";
    /// The synthetic record's `sampling/createMessage` key.
    pub const RECORDED_SAMPLING_KEY: &str = "sample";

    /// The entry-count bound, re-exported so the target re-derives it rather
    /// than spelling a number that can drift from production.
    pub const MAX_ENTRIES: usize = crate::types::mrtr::MAX_INPUT_RESPONSES;
    /// The per-entry serialized-size bound.
    pub const MAX_ENTRY_BYTES: usize = crate::types::mrtr::MAX_INPUT_RESPONSE_BYTES;
    /// The total serialized-size bound.
    pub const MAX_TOTAL_BYTES: usize = crate::types::mrtr::MAX_INPUT_RESPONSES_TOTAL_BYTES;
    /// The per-entry nesting-depth bound.
    pub const MAX_DEPTH: usize = crate::types::mrtr::MAX_INPUT_RESPONSE_DEPTH;

    /// What the pipeline decided, plus the keys it ACCEPTED.
    ///
    /// The accepted keys travel back because invariants 3 and 4 are about them
    /// specifically, and a bare verdict byte cannot express "it succeeded, but
    /// on a key the server never issued" — which is exactly the break those two
    /// invariants exist to detect.
    #[derive(Debug)]
    pub struct UpdateVerdict {
        /// One of the four `VERDICT_*` discriminants.
        pub verdict: u8,
        /// The keys that were typed against the record. Empty for every verdict
        /// other than [`VERDICT_ACCEPTED`].
        pub accepted: Vec<String>,
    }

    impl UpdateVerdict {
        /// A verdict with no accepted keys.
        const fn plain(verdict: u8) -> Self {
            Self {
                verdict,
                accepted: Vec::new(),
            }
        }
    }

    /// The FIXED record every fuzz run — and every in-module unit test — decodes
    /// against.
    ///
    /// One key of EACH kind, so a generated or fuzzed key that collides with a
    /// recorded one exercises a real kind-directed decode instead of always
    /// taking the ignore path. It is the SINGLE record fixture in this file: a
    /// second copy in the test module would let a change to one silently pass
    /// the other.
    ///
    /// # Panics
    ///
    /// Never in practice: the two `params` fixtures are compile-time literals
    /// that the corresponding request types accept. The `expect` is the repo's
    /// `check-unwraps`-compatible spelling of that fact.
    #[must_use]
    pub fn synthetic_snapshot() -> TaskInputSnapshot {
        let mut input_requests = InputRequests::new();
        input_requests.insert(RECORDED_ROOTS_KEY.to_string(), InputRequest::ListRoots);
        input_requests.insert(
            RECORDED_ELICITATION_KEY.to_string(),
            InputRequest::Elicitation(Box::new(
                crate::types::elicitation::ElicitRequestParams::Form {
                    message: "which city?".to_string(),
                    requested_schema: serde_json::json!({ "type": "object" }),
                },
            )),
        );
        input_requests.insert(
            RECORDED_SAMPLING_KEY.to_string(),
            InputRequest::Sampling(Box::new(
                serde_json::from_value(serde_json::json!({ "messages": [] }))
                    .expect("a minimal CreateMessageParams parses"),
            )),
        );
        TaskInputSnapshot {
            input_requests,
            input_responses: InputResponses::new(),
            status: TaskStatus::InputRequired,
        }
    }

    /// Drive raw `tasks/update` params bytes through the route's pure prefix.
    ///
    /// Parse -> bound -> kind-directed decode, in that order and through the
    /// PRODUCTION functions, so a campaign that stays green is evidence about the
    /// shipped code rather than about a fuzz-only reimplementation of it.
    ///
    /// Invalid UTF-8 and non-JSON bytes reach the parser rather than being
    /// filtered out before it, and both answer [`VERDICT_MALFORMED`].
    #[must_use]
    pub fn judge_update_params(input: &[u8]) -> UpdateVerdict {
        let Ok(params) = serde_json::from_slice::<Value>(input) else {
            return UpdateVerdict::plain(VERDICT_MALFORMED);
        };
        let Ok(update) = TaskDispatch::parse_tasks_update_params(&params) else {
            return UpdateVerdict::plain(VERDICT_MALFORMED);
        };
        // BEFORE the decode. Removing this line is the falsifiability control:
        // the target's invariant 2 then fires on an over-bound payload whose keys
        // are all unrecorded, because those are IGNORED and the decode succeeds.
        if crate::types::mrtr::check_input_responses_map_bounds(update.input_responses).is_err() {
            return UpdateVerdict::plain(VERDICT_BOUNDED);
        }
        let snapshot = synthetic_snapshot();
        match TaskDispatch::decode_inputs_against_record(update.input_responses, &snapshot) {
            Ok(typed) => UpdateVerdict {
                verdict: VERDICT_ACCEPTED,
                accepted: typed.keys().cloned().collect(),
            },
            Err(_) => UpdateVerdict::plain(VERDICT_REFUSED),
        }
    }
}

/// The `tasks/update` delivery pipeline — deterministic unit tests plus the
/// PROPERTY tests that generalize them (plan 114-14, TASK-02).
///
/// The pipeline under test is the pure prefix of the route: parse the RAW params,
/// bound the map, then decode it KIND-DIRECTED against the record. Everything
/// after that is the backend's single atomic write, which
/// `crates/pmcp-tasks/tests/input_delivery.rs` and `tests/v2_tasks_update.rs`
/// own; testing it again here would be testing the store through a second door.
///
/// The `proptest!` block sits beside the deterministic tests it generalizes,
/// following the `types::mrtr` precedent, and its strategy's recursion depth
/// stays INSIDE the depth bound it is generalizing over — a strategy that could
/// build a 33-deep value would make "a bounded map is never refused by the
/// bounds" fail for a reason that has nothing to do with the code.
#[cfg(test)]
mod update_delivery_tests {
    use super::fuzz_support::{
        judge_update_params, synthetic_snapshot, VERDICT_ACCEPTED, VERDICT_BOUNDED,
        VERDICT_MALFORMED, VERDICT_REFUSED,
    };
    use super::*;
    use crate::types::mrtr::{
        MAX_CANONICAL_DEPTH, MAX_INPUT_RESPONSES, MAX_INPUT_RESPONSES_TOTAL_BYTES,
        MAX_INPUT_RESPONSE_BYTES, MAX_INPUT_RESPONSE_DEPTH,
    };
    use proptest::prelude::*;
    use serde_json::Map;

    /// The deepest value the generator below can build.
    ///
    /// `prop_recursive(depth, ..)` counts RECURSION levels, and each level adds
    /// one container around a leaf, so a value from this strategy nests at most
    /// `STRATEGY_RECURSION + 1` levels.
    const STRATEGY_RECURSION: u32 = 4;

    /// The generator can never build a value the depth bound would refuse.
    ///
    /// A compile-time lock rather than a runtime assertion: if someone raises
    /// `STRATEGY_RECURSION` past the bound, the property
    /// `a_bounded_map_is_never_refused_by_the_bounds` would start failing for a
    /// reason that has nothing to do with the production code, and a
    /// `const _: () = assert!` says so at the point of the change instead.
    const _: () = assert!(
        STRATEGY_RECURSION as usize + 1 < MAX_INPUT_RESPONSE_DEPTH,
        "the inputResponses generator must stay strictly inside MAX_INPUT_RESPONSE_DEPTH, \
         or the bounded-map property fails for a reason unrelated to the code under test"
    );

    /// The `inputResponses` depth bound is strictly tighter than the AAD
    /// canonicalization depth cap.
    ///
    /// Both bound how deep a peer-supplied JSON value may be, and this route's
    /// values are a SUBSET of what the canonicalizer may later see. If the
    /// relationship inverted, a value could pass the ingress bound and then be
    /// refused deeper in as uncanonicalizable — a refusal at the wrong layer,
    /// with the wrong message, after the work had already been done.
    const _: () = assert!(
        MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH,
        "MAX_INPUT_RESPONSE_DEPTH must stay strictly below MAX_CANONICAL_DEPTH"
    );

    /// A depth-bounded arbitrary JSON value.
    fn arb_response_value() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i32>().prop_map(|n| serde_json::json!(n)),
            "[ -~]{0,12}".prop_map(Value::String),
        ];
        leaf.prop_recursive(STRATEGY_RECURSION, 24, 3, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..3).prop_map(Value::Array),
                prop::collection::btree_map("[a-z]{1,6}", inner, 0..3)
                    .prop_map(|map| Value::Object(map.into_iter().collect())),
            ]
        })
    }

    /// An `inputResponses` map of at most `max_entries` entries.
    ///
    /// The key alphabet deliberately includes the three RECORDED keys as
    /// generatable strings (`roots`, `form`, `sample` are all `[a-z]{1,6}`), so
    /// the decode path is reachable rather than always short-circuiting on
    /// ignore.
    fn arb_input_responses(max_entries: usize) -> impl Strategy<Value = Map<String, Value>> {
        prop::collection::btree_map(
            prop_oneof![
                Just("roots".to_string()),
                Just("form".to_string()),
                Just("sample".to_string()),
                "[a-z]{1,6}",
            ],
            arb_response_value(),
            0..=max_entries,
        )
        .prop_map(|map| map.into_iter().collect())
    }

    /// Every value this generator produces is small and shallow, so the ONLY
    /// bound a generated map can cross is the entry COUNT.
    ///
    /// Stated as its own test because the two properties below depend on it: if
    /// the generator could produce a 64 KiB entry, "a bounded map is never
    /// refused" would be false and the failure would look like a production bug.
    #[test]
    fn the_generator_cannot_cross_a_size_or_depth_bound() {
        proptest!(|(entries in arb_input_responses(MAX_INPUT_RESPONSES))| {
            let mut total = 0usize;
            for (key, value) in &entries {
                let bytes = serde_json::to_string(value).map_or(usize::MAX, |s| s.len());
                prop_assert!(bytes <= MAX_INPUT_RESPONSE_BYTES, "{key} is too large to be generatable");
                total += bytes;
            }
            prop_assert!(total <= MAX_INPUT_RESPONSES_TOTAL_BYTES);
        });
    }

    proptest! {
        /// A map inside every bound is never refused BY the bounds.
        #[test]
        fn a_bounded_map_is_never_refused_by_the_bounds(
            entries in arb_input_responses(MAX_INPUT_RESPONSES)
        ) {
            prop_assert!(check_input_responses_map_bounds(&entries).is_ok());
        }

        /// A map over the entry-count bound is ALWAYS refused, whatever it holds.
        #[test]
        fn an_over_count_map_is_always_refused(
            entries in arb_input_responses(MAX_INPUT_RESPONSES)
        ) {
            let mut entries = entries;
            // Fill past the bound with keys the generator cannot produce, so the
            // count crosses regardless of how many the generator gave us.
            for i in 0..=MAX_INPUT_RESPONSES {
                entries.insert(format!("PAD-{i:04}"), Value::Null);
            }
            prop_assert!(entries.len() > MAX_INPUT_RESPONSES);
            prop_assert!(check_input_responses_map_bounds(&entries).is_err());
        }

        /// The kind-directed decode never panics, and never accepts a key the
        /// record does not hold.
        ///
        /// The second half is the one that matters: an ACCEPTED key that the
        /// server never issued would mean the client chose its own kind, which is
        /// the "spoof the decode" break (T-114-74).
        #[test]
        fn the_decode_accepts_only_recorded_keys_and_never_panics(
            entries in arb_input_responses(MAX_INPUT_RESPONSES)
        ) {
            let snapshot = synthetic_snapshot();
            if let Ok(typed) = TaskDispatch::decode_inputs_against_record(&entries, &snapshot) {
                for key in typed.keys() {
                    prop_assert!(
                        snapshot.input_requests.contains_key(key),
                        "accepted `{key}`, which the record never held"
                    );
                }
            }
        }

        /// Parsing never panics over arbitrary params, and a parse that SUCCEEDS
        /// always yields a string `taskId` plus an object `inputResponses`.
        #[test]
        fn parsing_params_never_panics(
            task_id in prop_oneof![
                Just(Value::Null),
                any::<i32>().prop_map(|n| serde_json::json!(n)),
                "[ -~]{0,16}".prop_map(Value::String),
            ],
            entries in arb_input_responses(4),
        ) {
            let params = serde_json::json!({
                "taskId": task_id.clone(),
                "inputResponses": Value::Object(entries),
            });
            match TaskDispatch::parse_tasks_update_params(&params) {
                Ok(update) => {
                    prop_assert_eq!(Some(update.task_id.as_str()), task_id.as_str());
                    prop_assert!(params["inputResponses"].is_object());
                },
                Err(message) => prop_assert!(!message.is_empty()),
            }
        }
    }

    /// An UNRECORDED key is ignored and a RECORDED one whose value does not
    /// decode is refused — the deterministic pair the property above generalizes.
    #[test]
    fn ignore_and_refuse_are_different_answers() {
        let snapshot = synthetic_snapshot();

        let mut ignored = Map::new();
        ignored.insert(
            "never-issued".to_string(),
            serde_json::json!({ "nothing": true }),
        );
        let typed = TaskDispatch::decode_inputs_against_record(&ignored, &snapshot)
            .expect("an unrecorded key is IGNORED, never an error");
        assert!(
            typed.is_empty(),
            "and it contributes nothing to the delivery"
        );

        let mut refused = Map::new();
        // A `CreateMessageResult` under the ELICITATION key: the D-113-O shape.
        refused.insert(
            "form".to_string(),
            serde_json::json!({ "content": { "type": "text", "text": "x" }, "model": "m" }),
        );
        let error = TaskDispatch::decode_inputs_against_record(&refused, &snapshot)
            .expect_err("a recorded key's value must decode as the RECORDED kind");
        let rendered = error.to_string();
        assert!(
            rendered.contains("form"),
            "names the record's key: {rendered}"
        );
        for from_the_value in ["model", "content", "text"] {
            assert!(
                !rendered.contains(from_the_value),
                "the refusal must never render the value; it leaked `{from_the_value}`: {rendered}"
            );
        }
    }

    /// The fuzz seam cannot rot silently.
    ///
    /// `fuzz/` is not built by `make quality-gate` (and `cargo fuzz` needs a
    /// nightly toolchain, which this repo does not default to), so a seam that
    /// stopped compiling — or, worse, started answering the wrong verdict —
    /// would go unnoticed until someone ran a campaign. One test per verdict,
    /// driving the SAME entry point the target drives, is what makes the seam
    /// part of the ordinary gate. This mirrors `request_state`'s
    /// `fuzz_support_seam_rejects_garbage`.
    #[test]
    fn the_fuzz_seam_answers_every_verdict() {
        let body = |responses: Value| {
            serde_json::to_vec(&serde_json::json!({
                "taskId": "t-1",
                "inputResponses": responses,
            }))
            .expect("the fixture serializes")
        };

        assert_eq!(
            judge_update_params(b"not json at all").verdict,
            VERDICT_MALFORMED
        );
        assert_eq!(
            judge_update_params(b"{\"inputResponses\":{}}").verdict,
            VERDICT_MALFORMED,
            "a params object with no string taskId is malformed"
        );
        assert_eq!(
            judge_update_params(&body(serde_json::json!({ "roots": { "roots": [] } }))).verdict,
            VERDICT_ACCEPTED
        );
        assert_eq!(
            judge_update_params(&body(serde_json::json!({
                "form": { "content": { "type": "text", "text": "x" }, "model": "m" }
            })))
            .verdict,
            VERDICT_REFUSED,
            "the D-113-O shape under an elicitation key"
        );
        let over_count: Map<String, Value> = (0..=MAX_INPUT_RESPONSES)
            .map(|i| (format!("pad-{i:04}"), Value::Null))
            .collect();
        assert_eq!(
            judge_update_params(&body(Value::Object(over_count))).verdict,
            VERDICT_BOUNDED
        );

        // Invariant 3, stated here too: an ACCEPTED verdict never names a key
        // the record does not hold.
        let mixed = judge_update_params(&body(serde_json::json!({
            "roots": { "roots": [] },
            "never-issued": { "anything": true },
        })));
        assert_eq!(mixed.verdict, VERDICT_ACCEPTED);
        assert_eq!(mixed.accepted, vec!["roots".to_string()]);
    }

    /// The empty acknowledgement is EMPTY.
    #[test]
    fn the_update_ack_carries_no_fields() {
        let response = update_ack(RequestId::from(1i64));
        let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload else {
            panic!("an acknowledgement is a success result");
        };
        assert_eq!(value, Value::Object(Map::new()));
    }
}
