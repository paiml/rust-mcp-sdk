# Phase 104: Task-Augmented Tool Results DX (SEP-1686 junction) - Pattern Map

**Mapped:** 2026-07-04
**Files analyzed:** 11 (7 source, 1 example, 1 test, 1 Cargo.toml, docs)
**Analogs found:** 11 / 11 (all in-tree; zero new external deps)

> This is a single-crate additive-refactor phase. Every "new" file is actually an
> EDIT to an existing file — the analog is almost always the SAME file's existing
> siblings (an adjacent trait method, an adjacent builder, an adjacent client
> method). The planner should copy the *local* convention at each edit site, not a
> distant file. Line numbers below are verified against live source at map time
> (pmcp 2.11.0).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/server/mod.rs` — `ToolOutput` enum + `ToolHandler::handle_output()` | trait / model | request-response | `ToolHandler::handle` (mod.rs:229-238); `Task`/`CreateTaskResult` `#[non_exhaustive]` (tasks.rs:91,203) | exact (same trait, same file) |
| `src/server/mod.rs` — `handle_call_tool` match + tripwire call | service (dispatch) | request-response | the wrap tail it replaces (mod.rs:1429-1500); `ToolRejected` match arm (mod.rs:1438) | exact (in-place edit) |
| `src/server/mod.rs` — `ServerBuilder::tool_with_result()` | builder | request-response | `ServerBuilder::tool_typed_with_output` (mod.rs:2657-2687) | exact (mirror registration) |
| `src/server/core.rs` — `ServerCore` match + tripwire (D-05 twin) | service (dispatch) | request-response | `ServerCore` wrap tail `ToolCallOutcome` (core.rs:594-628) | exact (in-place edit) |
| `src/server/task_dispatch.rs` — shared `looks_like_call_tool_result` + wrap helper | utility (free fn) | transform | `maybe_build_task_created` gate (task_dispatch.rs:311-336) | role-match (shared seam) |
| `src/server/cancellation.rs` — `RequestHandlerExtra::set_result_meta()` + slot | model / builder | event-driven (interior-mut) | `with_auth_context`/`with_progress_reporter` builders (cancellation.rs:262-274) | role-match (builder shape; slot mechanism is NEW — see Open Q1) |
| `src/types/tasks.rs` — `TaskMetadata` type | model | transform | `Task` struct + builders (tasks.rs:91-159); `RelatedTaskMetadata` (tasks.rs:193-199) | exact (adjacent type, same serde conventions) |
| `src/types/tools.rs` — `CallToolResult::{with_related_task, related_task}` | model (builder + accessor) | transform | `CallToolResult::with_meta`/`with_widget_enrichment` (tools.rs:605-633) | exact (same impl block) |
| `src/client/mod.rs` — `Client::wait_for_task()` | service (client) | request-response (poll loop) | `call_tool_with_task` (client.rs:508-542); `tasks_get`/`tasks_result` (client.rs:555-588) | exact (adjacent method, composes them) |
| `examples/s47_task_augmented_result.rs` | example | request-response | `examples/s45_tool_as_task_lifecycle.rs`, `examples/s46_http_tool_as_task.rs` | exact (numbered slot, same domain) |
| `tests/tool_output_result_http.rs` | test (integration) | request-response (HTTP loopback) | `tests/tool_as_task_lifecycle_http.rs` (whole file) | exact (extend this harness) |

## Pattern Assignments

### `src/server/mod.rs` — `ToolOutput` enum + `ToolHandler::handle_output()` (trait, TOUT-01 / D-01)

**Analog:** the `ToolHandler` trait itself + `#[non_exhaustive]` types in `tasks.rs`.

**Existing trait to extend** (mod.rs:226-238) — copy the `#[cfg(not(target_arch = "wasm32"))]` + `#[async_trait]` + default-method conventions verbatim:
```rust
/// Handler for tool execution.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Handle a tool call with the given arguments.
    async fn handle(&self, args: Value, extra: cancellation::RequestHandlerExtra) -> Result<Value>;

    /// Get tool metadata including description and schema.
    fn metadata(&self) -> Option<crate::types::ToolInfo> {
        None
    }
}
```
Add `handle_output` as a THIRD method with a default body that delegates — the
`metadata()` default (returns `None`) is the precedent for "default method that
existing impls never override."

**`#[non_exhaustive]` enum convention** — copy from `tasks.rs` (Task at :91, CreateTaskResult at :203): every wire/API type in this crate carries `#[non_exhaustive]` for additive evolution. `ToolOutput` MUST too.

**Wasm twin (Pitfall 3):** the wasm `ToolHandler` is NOT in `traits.rs` — it is defined in `src/server/wasm_core.rs` / `wasm_server.rs` (module decls mod.rs:189-191); the wasm `RequestHandlerExtra` is a unit stub (mod.rs:155-161). Mirror `handle_output` there or the browser-served path silently keeps text-wrapping.

---

### `src/server/mod.rs` — `handle_call_tool` match + tripwire (service, TOUT-01/02 / D-04/D-05)

**Analog:** the exact tail this replaces + the `ToolRejected` arm right above it.

**The wrap site being changed** (mod.rs:1492-1500) — this becomes the `ToolOutput::Payload` arm; add the tripwire call BEFORE `result.to_string()`, and a sibling `ToolOutput::Result` arm that serializes verbatim:
```rust
// Build CallToolResult, adding structured_content for widget tools
let text = result.to_string();
let mut call_result = CallToolResult::new(vec![crate::types::Content::text(text)]);

if let Some(info) = self.tool_infos.get(&req.name) {
    call_result = call_result.with_widget_enrichment(info, result);
}

Ok(serde_json::to_value(call_result)?)
```

**The `Result`-early-return precedent** (mod.rs:1438-1442) — the `ToolOutput::Result` arm mirrors exactly this shape (build a `CallToolResult`, `serde_json::to_value`, early `return Ok(...)`, NO widget enrichment):
```rust
Err(Error::ToolRejected { message, details }) => {
    return Ok(serde_json::to_value(CallToolResult::rejected(
        message, details,
    ))?);
},
```

**Create-path gate that MUST keep precedence** (mod.rs:1463-1490) — do NOT move, do NOT route `ToolOutput::Result` through it. It runs, returns `None` for a non-task-shaped `CallToolResult`, and the `Result` arm fires after. The handler-call site to swap `handler.handle(...)` → `handler.handle_output(...)` is mod.rs:1394 (native, inside middleware) and mod.rs:1422 (wasm direct).

---

### `src/server/mod.rs` — `ServerBuilder::tool_with_result()` (builder, TOUT-01 / D-03.2)

**Analog:** `ServerBuilder::tool_typed_with_output` (mod.rs:2657-2687) — copy this registration shape exactly (the `impl Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output=Result<TOut>>>>` closure signature, the `self.tools.insert`, the capabilities auto-advertise tail):
```rust
#[cfg(feature = "schema-generation")]
pub fn tool_typed_with_output<TIn, TOut>(
    mut self,
    name: impl Into<String>,
    handler: impl Fn(TIn, crate::RequestHandlerExtra)
            -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<TOut>> + Send>>
        + Send + Sync + 'static,
) -> Self
where
    TIn: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
    TOut: serde::Serialize + schemars::JsonSchema + Send + Sync + 'static,
{
    use crate::server::typed_tool::TypedToolWithOutput;
    let name_str = name.into();
    let tool = TypedToolWithOutput::new(name_str.clone(), handler);
    self.tools.insert(name_str, Arc::new(tool));
    if self.capabilities.tools.is_none() {
        self.capabilities.tools = Some(crate::types::ToolCapabilities {
            list_changed: Some(false),
        });
    }
    self
}
```
For `tool_with_result`, `TOut` is fixed to `CallToolResult` and the wrapper tool's `handle_output` returns `ToolOutput::Result`. The per-tool `suppress_double_wrap_check()` opt-out (D-08) is a registration-time flag — model it on how `tool_typed_with_output` sets `capabilities`, i.e. a field mutation on the builder/ToolInfo at registration.

---

### `src/server/core.rs` — `ServerCore` match + tripwire (service, D-05 anti-drift twin)

**Analog:** the `ServerCore` wrap tail (core.rs:594-628) — this is the SECOND copy that D-05/Pitfall 2 warns about. It already has its OWN task-shape gate (core.rs:594-616, mirroring `maybe_build_task_created`) and its own wrap tail using a `ToolCallOutcome` enum:
```rust
let call_result = if let Some(info) = tool_info.filter(|i| i.widget_meta().is_some()) {
    let summary = summarize_structured_output(&value);
    CallToolResult::new(vec![Content::text(summary)]).with_widget_enrichment(info, value)
} else {
    let text = serde_json::to_string_pretty(&value)?;
    CallToolResult::new(vec![Content::text(text)])
};
Ok(ToolCallOutcome::Result(call_result))
```
The handler-call sites are core.rs:519 / :549 / :852. **Both** this tail AND the
mod.rs:1493 tail must get the `ToolOutput` match + tripwire — the mitigation is to
extract the wrap+tripwire decision into ONE `task_dispatch.rs` free fn both call
(Open Q3). Note the tails already DIVERGE (`core.rs` uses `to_string_pretty` +
`summarize_structured_output`; mod.rs uses `to_string`) — the plan must decide
whether unification also reconciles that, or preserves each tail's existing
serialization and only shares the branch/tripwire.

---

### `src/server/task_dispatch.rs` — shared tripwire + wrap helper (utility, TOUT-02 / D-06/D-07)

**Analog:** `maybe_build_task_created` (task_dispatch.rs:311-336) — the model for "a
pure `pub(crate)` gate fn on `TaskDispatch`, structurally inspecting a `&Value`
with high-precision `value.get(...)` markers, no full deserialize":
```rust
pub(crate) async fn maybe_build_task_created(
    &self,
    id: RequestId,
    value: &Value,
    task_support: Option<TaskSupport>,
    task_requested: bool,
    auth_context: Option<&AuthContext>,
) -> Option<JSONRPCResponse> {
    let gate_open = task_requested
        && self.task_store.is_some()
        && task_support.is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));
    if !gate_open { return None; }
    let is_task_shaped =
        value.get("taskId").and_then(Value::as_str).is_some() && value.get("status").is_some();
    if !is_task_shaped { return None; }
    Some(self.build_task_created_response(id, value.clone(), auth_context).await)
}
```
`looks_like_call_tool_result(&Value) -> Option<DoubleWrapMarker>` copies this
`value.get(...).and_then(...).is_some_and(...)` idiom. Precision basis: `Content`
is `#[serde(tag = "type")]` internally-tagged (content.rs:62), and
`RELATED_TASK_META_KEY = "io.modelcontextprotocol/related-task"` (tasks.rs:9). The
`// Why:` production-reachability annotation on `maybe_build_task_created` (:308-310)
is the template for justifying the new fn to PMAT.

---

### `src/server/cancellation.rs` — `RequestHandlerExtra::set_result_meta()` (model, TOUT-01 / D-03.3)

**Analog:** the existing `RequestHandlerExtra` builders (cancellation.rs:262-274) for the setter shape:
```rust
pub fn with_auth_context(
    mut self,
    auth_context: Option<crate::server::auth::AuthContext>,
) -> Self {
    self.auth_context = auth_context;
    self
}
```
BUT the plumbing is NEW (Pitfall 4 / Open Q1): `RequestHandlerExtra` is `#[non_exhaustive]`, `#[derive(Clone)]`, moved BY VALUE into `handle`/`handle_output` (mod.rs:231), so a plain `mut self` setter is invisible to the dispatcher. The precedent for a shared-across-clone field is the existing `peer: Option<Arc<dyn PeerHandle>>` (cancellation.rs:227-228) and `extensions: http::Extensions` (:213) — an `Arc`-wrapped field survives the clone/move. Model `set_result_meta` on that: add an `Arc<Mutex<Option<Map<String,Value>>>>` result-meta slot the dispatcher pre-clones before moving `extra` in, reads back after `handle_output` returns. Remember to add the field to `RequestHandlerExtra::new` (cancellation.rs:233-247) AND the wasm unit-stub path (mod.rs:160) must still compile.

---

### `src/types/tasks.rs` — `TaskMetadata` type (model, TOUT-03 / D-09)

**Analog:** `Task` struct + its builders (tasks.rs:91-159) and the adjacent minimal `RelatedTaskMetadata` (tasks.rs:193-199):
```rust
/// Task metadata for related-task references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTaskMetadata {
    /// The referenced task ID
    pub task_id: String,
}
```
`TaskMetadata` extends this shape with `#[non_exhaustive]` (copy from `Task` at :92) and optional `poll_interval` / `max_poll_duration_secs` using the `Task.poll_interval` serde pattern verbatim (tasks.rs:107-108):
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub poll_interval: Option<u64>,
```
Keep `#[serde(rename_all = "camelCase")]` → wire keys `taskId` / `pollInterval` /
`maxPollDurationSecs` (the exact keys `_meta`-sniffing pmcp.run clients already
read). `related_task()` must tolerate the minimal `{taskId}` native-emit shape
(all extra fields `Option` → default `None`) — Pitfall 6.

---

### `src/types/tools.rs` — `CallToolResult::{with_related_task, related_task}` (model, TOUT-03 / D-03.1/D-09)

**Analog:** the existing `CallToolResult` impl block (tools.rs:605-633) — `with_meta` is the builder template, `with_widget_enrichment` shows the `_meta` `serde_json::Map` manipulation idiom:
```rust
/// Add widget-only metadata.
#[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
    self._meta = Some(meta);
    self
}
```
`_meta` is `Option<serde_json::Map<String, Value>>` (tools.rs:556-558) with the
`#[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]` +
`#[allow(clippy::pub_underscore_fields)]` attributes — `with_related_task` inserts
`RELATED_TASK_META_KEY → serde_json::to_value(meta)` into that map (init to
`Some(Map::new())` when `None`), and `related_task()` reads it back with
`.and_then(|v| serde_json::from_value(v.clone()).ok())`. Carry the
`#[allow(clippy::used_underscore_binding)]` on any fn touching `_meta` (the
existing block-level `#[allow]` at tools.rs:637 is test-only).

---

### `src/client/mod.rs` — `Client::wait_for_task()` (service, TOUT-03 / D-09/D-10)

**Analog:** `call_tool_with_task` (client.rs:508-542) composes `send_request`; `tasks_get` (:555-567) and `tasks_result` (:574-588) are the exact methods `wait_for_task` loops over:
```rust
pub async fn tasks_get(&self, task_id: &str) -> Result<Task> {
    self.ensure_initialized()?;
    self.assert_capability("tasks", "tasks/get")?;
    let request = Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
        task_id: task_id.to_string(),
    })));
    let request_id = RequestId::String(Uuid::new_v4().to_string());
    let response = self.send_request(request_id, request).await?;
    let task_result: GetTaskResult = self.parse_task_payload(response, "tasks/get").await?;
    Ok(task_result.task)
}
```
`wait_for_task` = `loop { tasks_get; if task.status.is_terminal() break; sleep }`
then `tasks_result`. `TaskStatus::is_terminal()` (tasks.rs:44) is the loop exit;
`task.poll_interval` (tasks.rs:108) is the delay fallback. Copy the
`ensure_initialized()` + `assert_capability("tasks", ...)` preamble from every
sibling. Wasm delay (D-10): the in-tree precedent is `web-time` (Cargo.toml:97,
used `src/shared/middleware.rs:25` for `Instant`); `#[cfg(not(wasm32))] tokio::time::sleep`
/ `#[cfg(wasm32)]` gloo/wasm-bindgen future — verify the wasm timer is already
transitive before adding (Package Legitimacy). Client swallow audit (deferred
ask #5): `parse_task_payload` (client.rs:629-640) ALREADY WARNs on deserialize
failure — SDK client is clean; note in the guide.

---

### `examples/s47_task_augmented_result.rs` (example, TOUT-04 / D-13)

**Analog:** `examples/s45_tool_as_task_lifecycle.rs` + `examples/s46_http_tool_as_task.rs` (next free slot is `s47`; highest existing is `s46`). Copy the file-header doc-comment style, the `TypedTool`/`with_task_store()` server setup, and the in-process-then-assert structure. The BEFORE/AFTER diff (hand-rolled `_meta` tool vs native `with_task_store()` + `ToolOutput`) IS the migration guide. Add the `[[example]]` block to `Cargo.toml` mirroring the s46 entry with `required-features = ["full"]`.

---

### `tests/tool_output_result_http.rs` (test, TOUT-04 / D-14)

**Analog:** `tests/tool_as_task_lifecycle_http.rs` (whole file) — the Phase 102 live HTTP loopback harness. Copy its structure verbatim:
```rust
#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::{Client, ErrorCode, Server, ToolCallResponse};
```
Reuse its reliability conventions (documented in its header, lines 19-31):
EPHEMERAL PORT (`127.0.0.1:0`, read `local_addr` back from `start()`),
readiness = listener bound before `start()` returns (no fixed sleep), SHUTDOWN =
`JoinHandle::abort()` after the client completes. Register a tool returning
`ToolOutput::Result(CallToolResult::new(..).with_related_task(..))`, drive it over
the real transport, and assert on the RAW wire JSON that `result._meta` is present
at TOP LEVEL and that the envelope is NOT stringified into `content[0].text` (the
agent-lake bug). Consumes REAL dispatch output — never a hand-authored fixture.
Sibling `tests/tool_as_task_lifecycle.rs` (in-process duplex) is the analog for the
D-05 byte-identical `Server` vs `ServerCore` parity test.

---

## Shared Patterns

### `#[non_exhaustive]` + camelCase serde on every wire/API type
**Source:** `src/types/tasks.rs:91-93` (`Task`), tools.rs:523-525 (`CallToolResult`)
**Apply to:** `ToolOutput`, `TaskMetadata`
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
```
Non-wire enums (`ToolOutput`) skip `Serialize/Deserialize`/`camelCase` but KEEP `#[non_exhaustive]` (additive-evolution rule, CONTEXT.md §Established Patterns).

### `#[cfg(not(target_arch = "wasm32"))]` gating on server task machinery
**Source:** `src/server/mod.rs:227` (trait), :1363-1490 (create-path), cancellation.rs:227
**Apply to:** every new server-side fn touching the task store / create-path. Client task calls (`wait_for_task`) ARE wasm-reachable (Phase 103 browser client) — gate only the delay mechanism, not the method. The wasm `RequestHandlerExtra` unit stub (mod.rs:160) must still compile any new `set_result_meta` field.

### Builder-by-`mut self` + `with_*` naming
**Source:** `cancellation.rs:250-274`, `tasks.rs:132-158`, `tools.rs:597-608`
**Apply to:** `with_related_task`, `TaskMetadata::with_*`, `set_result_meta` (note: `set_` prefix — it mutates interior state, not a chainable builder — document the naming departure).

### Client method preamble: `ensure_initialized()` + `assert_capability("tasks", ...)`
**Source:** `src/client/mod.rs:556-557` (and every `tasks_*` sibling)
**Apply to:** `wait_for_task` (composes already-guarded calls, so a top-level `assert_capability("tasks", "tasks/get")` is optional but consistent).

### `// Why:` PMAT reachability annotation
**Source:** `src/server/task_dispatch.rs:308-310`
**Apply to:** any new `pub(crate)` fn that a naive dead-code/complexity scan might flag (the tripwire fn, the shared wrap helper). Keep every new fn ≤ cog 25.

## No Analog Found

None. Every deliverable has a same-crate, same-role precedent. The two mechanisms
with the WEAKEST analog (still role-matched, but genuinely new plumbing) are called
out explicitly rather than listed here:

| Concern | Role | Data Flow | Note |
|------|------|-----------|------|
| `set_result_meta` interior-mutable slot (D-03.3) | model | event-driven | No existing `RequestHandlerExtra` field round-trips a handler mutation back to the dispatcher; the `Arc`-wrapped `peer`/`extensions` fields are the closest structural precedent but are dispatcher→handler, not handler→dispatcher. Spike first (Open Q1). |
| wasm timer for `wait_for_task` delay (D-10) | utility | request-response | `web-time` gives `Instant` on wasm but NOT a `sleep`; the async-delay wasm crate (gloo-timers / wasm-bindgen-futures) is a Claude's-discretion choice — verify it is already transitive (`cargo tree -e features -i`) before adding, else it hits the Package Legitimacy Gate. |

## Metadata

**Analog search scope:** `src/server/{mod,core,task_dispatch,cancellation}.rs`, `src/types/{tasks,tools,content}.rs`, `src/client/mod.rs`, `examples/s4*`, `tests/tool_as_task_lifecycle*.rs`
**Files scanned:** 11 read + 1 grep sweep (core.rs / wasm module map / example+test inventory)
**Pattern extraction date:** 2026-07-04
