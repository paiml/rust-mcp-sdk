# Phase 104: Task-Augmented Tool Results DX (SEP-1686 junction) - Research

**Researched:** 2026-07-04
**Domain:** Rust SDK internal architecture — tool-dispatch / tasks-layer junction (`pmcp` 2.11.0)
**Confidence:** HIGH — code-archaeology phase; every claim below is verified against live source at the cited line. Zero external dependencies; no web research required.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Typed-output API shape (TOUT-01)**
- **D-01:** Core mechanism is a `ToolOutput` enum — `ToolOutput::Payload(Value) | ToolOutput::Result(CallToolResult)` — exposed via a NEW default-implemented trait method (e.g. `ToolHandler::handle_output()`) that delegates to `handle()`. Fully additive; existing handlers untouched; works for hand-written `ToolHandler` impls (the pmcp.run case), not just builder closures.
- **D-02:** Implicit "returned Value parses as `CallToolResult` → pass through" sniffing is **REJECTED**. `CallToolResult`'s serde is fully defaulted (`#[serde(default)]` on `content`/`is_error`, unknown fields ignored), so ANY JSON object parses — implicit detection would silently swallow arbitrary payloads, recreating the bug class.
- **D-03:** Three sugar layers ALL ship alongside the enum:
  1. `CallToolResult::with_related_task(TaskMetadata)` builder method keyed by `RELATED_TASK_META_KEY` (server-emit twin of the client accessor);
  2. typed closure registration `ServerBuilder::tool_with_result(name, |args: T, extra| -> Result<CallToolResult>)` mirroring the existing `tool_typed()` precedent;
  3. `RequestHandlerExtra::set_result_meta()` — merges `_meta` onto the dispatch-built result (lowest-friction retrofit for existing handlers).
- **D-04:** Precedence: the Phase 102 create-path gate (`maybe_build_task_created`) keeps running FIRST — native task machinery and D-STORE-MINTS-ID remain un-bypassable (a `Result` output isn't task-shaped, so the gate naturally passes). Then `ToolOutput::Result(...)` goes to the wire verbatim: NO text-wrap, NO widget enrichment. The handler owns the full envelope.
- **D-05:** The change lands in the SHARED task-dispatch seam (Phase 102 anti-drift rule): `Server` and `ServerCore` dispatch must honor `ToolOutput` identically — no divergent second copy of the pass-through logic.

**Tripwire semantics (TOUT-02)**
- **D-06:** Loudness: `tracing::warn!` in ALL builds PLUS `debug_assert!` hard-fail in debug builds. Release builds never panic.
- **D-07:** Heuristic (high-precision structural markers, NOT full deserialize): fire when the `Value` is an object with (a) a `content` array whose elements ALL deserialize as valid `Content` items, OR (b) `_meta` containing `RELATED_TASK_META_KEY`.
- **D-08:** Escape hatch: per-tool registration-time opt-out flag (e.g. `.suppress_double_wrap_check()`) for tools whose legitimate payload trips the heuristic. Explicit and reviewable; no env-var global kill switch.

**Client accessor scope (TOUT-03)**
- **D-09:** Ship BOTH the typed accessor `CallToolResult::related_task() -> Option<TaskMetadata>` (SEP-1686 detection of `_meta["io.modelcontextprotocol/related-task"]`) AND a `wait_for_task(task_id, opts)` client convenience that drives `tasks/get` polling until a terminal status, then fetches `tasks/result`. Honors `pollInterval`/`maxPollDurationSecs` from `TaskMetadata` with caller overrides.
- **D-10:** `wait_for_task` must be wasm32-compatible: platform-abstracted delay (`tokio::time::sleep` native / wasm-bindgen-futures- or gloo-timers-based delay on wasm — precedent: `web-time` already adopted for `Instant`). The Phase 103 web-channel browser client is a direct consumer.
- **D-11:** A `Stream`-based poll API is rejected for now — can be added additively later.

**Migration guide shape (TOUT-04)**
- **D-12:** Canonical guide = pmcp-book chapter + a `docs/design/` companion recording the SEP-1686 junction rationale and the D-08/D-09 wire-compat confirmation. README gets a short pointer. Course chapter deferred.
- **D-13:** One new numbered runnable example (next free slot, `s47`) showing BEFORE (hand-rolled `_meta` task tool) and AFTER (same tool on native `with_task_store()` + `ToolOutput`) — the diff IS the migration guide; doubles as the ALWAYS-required example and a regression harness.
- **D-14:** Acceptance gate for wire-shape correctness: extend the Phase 102 live HTTP loopback harness with a `ToolOutput::Result` tool, asserting the RAW wire JSON carries `_meta` at result top-level. In-repo, CI-enforced. A coordinated pmcp.run-side UAT is NOT a phase-closure gate.

### Claude's Discretion
- Exact names (`ToolOutput`, `handle_output`, `tool_with_result`, `wait_for_task`, `suppress_double_wrap_check`) and module placement within `src/server/` / `src/client/` — provided the shared-seam rule (D-05) holds.
- Internal signature of the pass-through in the shared dispatch unit.
- WARN message contents (should include tool name and which marker fired).
- Wasm timer mechanism selection for D-10.
- Whether `set_result_meta` merges or overwrites on key collision (document whichever is chosen).
- Test file organization (mirror `tests/tool_as_task_lifecycle.rs` / Phase 102 HTTP harness conventions).

### Deferred Ideas (OUT OF SCOPE)
- `Stream`-based task-status polling API on the client (D-11) — additive later.
- pmcp-course chapter for the migration guide (D-12) — after the API stabilizes.
- Coordinated pmcp.run-side UAT as a formal gate (D-14) — invited, not required for phase closure.
- The June asks #1–#5 already shipped (Phase 101/102) or out of SDK scope (their durable client's WARN-on-deserialize, ask #5) — though pmcp's OWN client error paths should be checked for the same swallow pattern (note: `parse_task_payload` already WARNs on deserialize failure — client/mod.rs:629-640; verified present).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TOUT-01 | Typed, explicit `ToolOutput`; `ToolOutput::Result(CallToolResult)` lands un-re-wrapped through normal `Server` dispatch | The wrap site is `src/server/mod.rs:1493` (`result.to_string()` → `Content::text`). The `ToolHandler` trait to extend is `src/server/mod.rs:229` (`handle(&self, args, extra) -> Result<Value>`) — NOT `traits.rs:25`. The seam that must own the branch is `src/server/task_dispatch.rs` (Phase 102, `TaskDispatch`). See §Architecture Patterns / Pattern 1-2. |
| TOUT-02 | Dispatch emits WARN + debug-fail when about to text-wrap a `Value` structurally shaped like a built `CallToolResult` (valid `content` array OR `_meta` with `RELATED_TASK_META_KEY`) | `Content` is `#[serde(tag = "type")]` internally-tagged (src/types/content.rs:62) → a stray object with no `"type"` never deserializes as `Content`, giving the (a) marker near-zero false positives. `RELATED_TASK_META_KEY = "io.modelcontextprotocol/related-task"` (src/types/tasks.rs:9). Tripwire fires at the mod.rs:1493 branch (and the `ServerCore` twin). See Pattern 3. |
| TOUT-03 | Client `related_task()` accessor (`_meta[related-task]` → `TaskMetadata`) + `wait_for_task` polling convenience | `TaskMetadata` type does NOT yet exist — must be defined (only `RelatedTaskMetadata { task_id }` exists at src/types/tasks.rs:196). Client task surface to compose: `tasks_get`/`tasks_result`/`tasks_cancel` (src/client/mod.rs:555-620). `web-time` precedent for wasm time (Cargo.toml:97). See Pattern 4 + Open Question 2. |
| TOUT-04 | Migration guide (pmcp-book + docs/design) + BEFORE/AFTER runnable example (`s47`) + wire-compat confirmation | Native create-path `_meta` emission proven at src/server/core_tests.rs:855-896 (D-STORE-MINTS-ID + `_meta[related-task]` == store id). `s47` is the next free example slot (highest is `s46`). See §Code Examples + D-13/D-14. |
</phase_requirements>

## Summary

This is a **pure code-architecture phase** inside one crate (`pmcp` 2.11.0). Phase 101 landed the `tasks/*` lifecycle on `ServerCore`; Phase 102 lifted it into a SHARED `src/server/task_dispatch.rs` unit (`TaskDispatch`) and wired the high-level `Server`/HTTP path through it. Phase 104 closes the **remaining junction**: a tool that produces a full `CallToolResult` — `_meta` and all — currently has that entire envelope stringified into `content[0].text` at `src/server/mod.rs:1493`, because `Server::handle_call_tool` treats the handler's returned `Value` as an opaque payload. This is the fifth documented variant of a silent wire-shape bug class (a 2-week silent production outage in agent-lake).

The fix is **additive and surgical**: (1) a `ToolOutput` enum returned by a new default-impl `ToolHandler::handle_output()` method whose default delegates to `handle()` (so existing handlers are untouched), (2) a dispatch branch — placed in or alongside the shared `TaskDispatch` seam AFTER the Phase-102 create-path gate — that sends `ToolOutput::Result(r)` to the wire verbatim and text-wraps `ToolOutput::Payload(v)` exactly as today, (3) a tripwire at the wrap site that WARNs + debug-asserts when it is about to wrap a `Value` that structurally looks like a `CallToolResult`, and (4) client-side `related_task()` + `wait_for_task`. All wire shapes are FROZEN (Phase 101); no breaking change to `ToolHandler`, `Server`/`ServerBuilder`, or the client API.

The single sharpest design constraint discovered: **`RequestHandlerExtra` is moved by value into `handle()` and is not shared with the dispatcher**, so `set_result_meta()` (D-03.3) cannot round-trip through the current signature without a shared interior-mutable slot — this materially shapes the plan (see Open Question 1 / Pitfall 4). The `handle_output` mechanism (D-01) sidesteps this cleanly and should be the primary path; `set_result_meta` is the one sugar layer that needs an explicit plumbing decision.

**Primary recommendation:** Add `ToolOutput` + `ToolHandler::handle_output()` (default → `Payload`); branch on it inside the SHARED dispatch flow immediately after `maybe_build_task_created` returns `None`; make the tripwire a free `fn looks_like_call_tool_result(&Value) -> Option<Marker>` called at the wrap site in BOTH dispatchers; add `TaskMetadata` + `CallToolResult::{with_related_task, related_task}` + `Client::wait_for_task` (platform-abstracted delay). Land D-14's live-HTTP `_meta`-at-top-level assertion RED first.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Return a full `CallToolResult` from a tool | API/Backend — `ToolHandler` impl via `ToolOutput` | — | The handler owns the envelope (D-04); dispatch must not re-wrap. |
| Verbatim pass-through vs text-wrap decision | API/Backend — shared `TaskDispatch` seam / `handle_call_tool` | — | D-05: single copy honored identically by `Server` + `ServerCore`. |
| Task create-path precedence | API/Backend — `maybe_build_task_created` (Phase 102) | — | D-04: runs FIRST, un-bypassable; a `Result` output isn't task-shaped so it passes. |
| Double-wrap tripwire | API/Backend — free fn at the wrap site | — | Structural inspection of the produced `Value`; no I/O, no auth. |
| SEP-1686 task detection on a result | Client — `CallToolResult::related_task()` | — | Pure accessor over `_meta`; reachable on wasm (Phase 103 browser client). |
| Poll-until-terminal convenience | Client — `Client::wait_for_task` | Browser (wasm delay) | Composes `tasks_get`/`tasks_result`; needs platform-abstracted sleep (D-10). |
| Migration narrative | Docs — pmcp-book + docs/design + `s47` example | — | D-12/D-13; the example diff IS the guide. |

## Standard Stack

This phase adds **no new external dependencies.** All work is internal to `pmcp` using in-tree types and crates already present.

### Core (already in-tree — verified present)
| Item | Location | Purpose in this phase |
|------|----------|------------------------|
| `ToolHandler` trait (`handle -> Result<Value>`) | `src/server/mod.rs:229` | Gains the default-impl `handle_output()` (D-01). NOTE: this is the `Server` trait, distinct from `traits.rs:25`. |
| `CallToolResult` (`content`/`is_error`/`structured_content`/`_meta`) | `src/types/tools.rs:526` | Gains `with_related_task()` (D-03.1) + `related_task()` (D-09). Its fully-defaulted serde (`:531`,`:535`) is the reason D-02 rejects sniffing. |
| `Content` enum (`#[serde(tag="type")]`) | `src/types/content.rs:62` | The tripwire's (a) marker deserializes candidate `content` items as `Content` — internal tag ⇒ high precision (D-07). |
| `RELATED_TASK_META_KEY` | `src/types/tasks.rs:9` | The `_meta` key for D-03.1/D-07(b)/D-09. |
| `RelatedTaskMetadata { task_id }` | `src/types/tasks.rs:196` | Present but MINIMAL — carries only `task_id`. `TaskMetadata` (with `pollInterval`/`maxPollDurationSecs`) must be ADDED for D-09/D-10. |
| `TaskDispatch` + `maybe_build_task_created` | `src/server/task_dispatch.rs:158,311` | The shared seam (D-05). Create-path gate that keeps precedence (D-04). |
| `TaskDispatch::route_tasks_endpoint` / `handle_tasks_result` | `src/server/task_dispatch.rs:500,346` | Client `wait_for_task` drives these over the wire. |
| `Server::handle_call_tool` (wrap at `:1493`, create-gate at `:1465`) | `src/server/mod.rs:1493`,`:1463-1490` | The exact TOUT-01/TOUT-02 change site. |
| `client.tasks_get/tasks_result/tasks_cancel` | `src/client/mod.rs:555,574,606` | Composed by `wait_for_task`. `assert_capability("tasks", …)` guards each. |
| `TypedTool` / `TypedToolWithOutput` + `tool_typed_with_output` | `src/server/typed_tool.rs`, `src/server/mod.rs:2657` | The registration precedent `tool_with_result` (D-03.2) mirrors. |
| `web-time` (`web_time::Instant`) | `Cargo.toml:97`, used `src/shared/middleware.rs:25` | Precedent for platform-abstracted time on wasm (D-10). |
| `RequestHandlerExtra` (has `task_request`, `is_task_request()`) | `src/server/cancellation.rs:179,350` | `req.task` plumbing already present. `set_result_meta` (D-03.3) needs a NEW field + a retrieval mechanism (see Open Question 1). |

**Installation:** none. `cargo` workspace; task path features `["streamable-http"]` (in `full`).

## Package Legitimacy Audit

> **Not applicable.** This phase installs **zero** external packages. All work uses crates already in the `pmcp` workspace. slopcheck / registry verification is moot — no `cargo add`. If the wasm-delay choice for D-10 (Claude's discretion) reaches for a NEW crate (e.g. `gloo-timers`), that ONE crate must pass the Package Legitimacy Gate before the plan adopts it; the preferred path reuses `web-time` + `wasm-bindgen-futures` which are already transitive (verify with `cargo tree -e features -i web-time`).

## Architecture Patterns

### System Architecture Diagram

```
   tool author returns ToolOutput
   ┌───────────────────────────────────────────────┐
   │  impl ToolHandler {                            │
   │    handle_output() -> ToolOutput               │   default impl:
   │      Payload(Value)   ← existing handlers      │   handle().map(ToolOutput::Payload)
   │      Result(CallToolResult) ← NEW, _meta owned │
   │  }                                             │
   └───────────────────────┬───────────────────────┘
                           │ handler.handle_output(args, extra).await
                           ▼
   ┌───────────────────────────────────────────────────────────────┐
   │  Server::handle_call_tool  (mod.rs ~1360-1500)  == ServerCore  │  D-05: identical in both
   │                                                                │
   │  1. run middleware + handler.handle_output()                   │
   │  2. ToolRejected → CallToolResult::rejected  (mod.rs:1438)     │
   │  3. match ToolOutput:                                          │
   │      Payload(v) ──► maybe_build_task_created(v, …)  [D-04]     │  ← create-path FIRST
   │                       Some(resp) → task envelope (verbatim)    │
   │                       None → TRIPWIRE(v) [TOUT-02] ─┐          │
   │                              then text-wrap (:1493) │          │
   │                              + widget enrichment    ▼          │
   │      Result(r)  ──► [gate passes: not task-shaped]             │
   │                     serde_json::to_value(r) VERBATIM           │  ← TOUT-01: NO wrap, NO widget
   └───────────────────────────────────────────────────────────────┘
                           │  raw JSON-RPC result (result._meta preserved)
                           ▼
   ┌───────────────────────────────────────────────┐
   │  wire: { "result": { "content":[…],           │
   │            "_meta": { related-task: {…} } } }  │  ← D-14 asserts _meta at TOP LEVEL
   └───────────────────────┬───────────────────────┘
                           ▼  client side
   ┌───────────────────────────────────────────────┐
   │  CallToolResult::related_task() -> TaskMetadata│  [TOUT-03 / D-09]
   │  Client::wait_for_task(id, opts):              │
   │    loop { tasks_get → terminal? } → tasks_result│  platform-abstracted delay [D-10]
   └───────────────────────────────────────────────┘
```

### Recommended Project Structure (new/changed files)
```
src/server/
├── mod.rs              # ToolHandler::handle_output() default method (:229 trait);
│                       #   ToolOutput enum (or in a new small module);
│                       #   handle_call_tool: match ToolOutput, verbatim pass-through,
│                       #   tripwire call before the :1493 wrap;
│                       #   ServerBuilder::tool_with_result() (mirror tool_typed_with_output :2657);
│                       #   per-tool suppress_double_wrap_check flag (on ToolInfo/registration)
├── task_dispatch.rs    # (optional) house the tripwire fn + pass-through helper so BOTH
│                       #   dispatchers call ONE copy (D-05 anti-drift)
├── core.rs             # ServerCore twin: same match + tripwire (no divergent copy)
└── cancellation.rs     # RequestHandlerExtra::set_result_meta() + retrieval slot (Open Q 1)
src/types/
├── tasks.rs            # NEW TaskMetadata (taskId + pollInterval + maxPollDurationSecs, camelCase)
└── tools.rs            # CallToolResult::with_related_task() + related_task()
src/client/
└── mod.rs              # Client::wait_for_task(task_id, WaitForTaskOptions) — wasm-safe delay
examples/
└── s47_task_augmented_result.rs   # BEFORE/AFTER migration example (D-13)
tests/
└── tool_output_result_http.rs     # D-14 live-HTTP _meta-at-top-level (extend Phase 102 harness)
docs/
├── design/…            # SEP-1686 junction rationale + wire-compat note (D-12)
└── pmcp-book chapter   # migration guide (D-12)
Cargo.toml              # [[example]] s47, required-features = ["full"]
```

### Pattern 1: Additive default-impl trait method (TOUT-01, D-01)
**What:** Add `handle_output` to the `ToolHandler` trait at `mod.rs:229` with a default body that preserves today's behavior.
**When:** The dispatcher calls `handle_output` instead of `handle`; hand-written handlers override it to return `Result`.
```rust
// Source target: src/server/mod.rs:229 (the Server-side ToolHandler; NOT traits.rs:25)
pub enum ToolOutput {
    Payload(Value),          // existing behavior: dispatch wraps/enriches
    Result(CallToolResult),  // NEW: handler owns the full envelope, verbatim to wire
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value>;

    /// Default preserves existing dispatch: delegate to `handle`, wrap as Payload.
    async fn handle_output(&self, args: Value, extra: RequestHandlerExtra) -> Result<ToolOutput> {
        self.handle(args, extra).await.map(ToolOutput::Payload)
    }

    fn metadata(&self) -> Option<crate::types::ToolInfo> { None }
}
```
- Existing impls compile unchanged (default method). The dispatcher swaps `handler.handle(...)` (mod.rs:1394 native / :1422 wasm) → `handler.handle_output(...)`.
- `ToolOutput` should be `#[non_exhaustive]` (established additive-evolution pattern, tools.rs/tasks.rs precedent).

### Pattern 2: Verbatim pass-through AFTER the create-path gate (TOUT-01, D-04/D-05)
**What:** In `handle_call_tool`, after `ToolRejected` mapping and the `maybe_build_task_created` gate, branch on `ToolOutput`.
```rust
// Conceptual — lands in BOTH Server (mod.rs) and ServerCore (core.rs); D-05.
let output = handler.handle_output(args, extra).await?; // ToolRejected handled as today
match output {
    ToolOutput::Payload(value) => {
        // 1. create-path gate keeps precedence (Phase 102) — UNCHANGED
        if let Some(resp) = task_dispatch.maybe_build_task_created(id, &value, ts, task_requested, auth).await {
            return /* task envelope */;
        }
        // 2. TOUT-02 tripwire, then existing text-wrap + widget enrichment (mod.rs:1493-1498)
        double_wrap_tripwire(&req.name, &value, suppress_flag);
        let text = value.to_string();
        let mut call_result = CallToolResult::new(vec![Content::text(text)]);
        if let Some(info) = self.tool_infos.get(&req.name) { call_result = call_result.with_widget_enrichment(info, value); }
        Ok(serde_json::to_value(call_result)?)
    }
    ToolOutput::Result(result) => {
        // TOUT-01: verbatim. NO text-wrap, NO widget enrichment. Handler owns _meta.
        // create-path gate naturally passes: a CallToolResult is not task-shaped
        // (no top-level taskId+status), so D-04 precedence is preserved for free.
        Ok(serde_json::to_value(result)?)
    }
}
```
**Critical D-04 subtlety:** `maybe_build_task_created` gates on `value.get("taskId") && value.get("status")` (task_dispatch.rs:327). A `CallToolResult` has neither at top level, so `ToolOutput::Result` does NOT need to be routed through the gate at all — but the plan MUST confirm that a task-augmented `Result` (one carrying `_meta[related-task]`) is still emitted verbatim and NOT double-created. The task create-path is for the *Payload* task-shaped value; the `Result` path is the SEP-1686 "my result *references* a task" case — they are complementary, not competing.

### Pattern 3: Tripwire as a high-precision free function (TOUT-02, D-06/D-07)
**What:** A pure `fn` inspecting the `Value` about to be text-wrapped; returns which marker fired (for the WARN message).
```rust
// Source basis: Content is #[serde(tag="type")] (content.rs:62); RELATED_TASK_META_KEY (tasks.rs:9)
enum DoubleWrapMarker { ContentArray, RelatedTaskMeta }

fn looks_like_call_tool_result(v: &Value) -> Option<DoubleWrapMarker> {
    let obj = v.as_object()?;
    // (b) high-precision: _meta contains the SEP-1686 related-task key
    if obj.get("_meta").and_then(Value::as_object)
          .is_some_and(|m| m.contains_key(RELATED_TASK_META_KEY)) {
        return Some(DoubleWrapMarker::RelatedTaskMeta);
    }
    // (a) content array whose elements ALL deserialize as Content (internal "type" tag ⇒ precise)
    if let Some(arr) = obj.get("content").and_then(Value::as_array) {
        if !arr.is_empty()
           && arr.iter().all(|e| serde_json::from_value::<Content>(e.clone()).is_ok()) {
            return Some(DoubleWrapMarker::ContentArray);
        }
    }
    None
}
```
- At the wrap site: if `!suppress_flag` and `looks_like_call_tool_result(&value)` is `Some(marker)` → `tracing::warn!(tool=%name, ?marker, "value being text-wrapped structurally resembles a built CallToolResult — did you mean ToolOutput::Result? (TOUT-02)")` **plus** `debug_assert!(false, …)` so any debug/CI run fails hard (D-06).
- **False-positive note:** a legitimate `Payload` that happens to be `{ "content": [ {"type":"text", "text":"…"} ] }` WILL trip (a). That is exactly what the D-08 per-tool `suppress_double_wrap_check()` opt-out exists for. Empty `content: []` must NOT trip (guarded by `!arr.is_empty()`), since `CallToolResult::new(vec![])` is a real shape but an empty payload array is common and benign — confirm this choice with the planner (matches "high-precision, near-zero false positive").

### Pattern 4: Client accessor + wasm-safe poll (TOUT-03, D-09/D-10)
```rust
// src/types/tasks.rs — NEW (RelatedTaskMetadata at :196 carries only task_id; insufficient for polling)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TaskMetadata {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub poll_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub max_poll_duration_secs: Option<u64>,
}

// src/types/tools.rs — on CallToolResult
pub fn with_related_task(mut self, meta: TaskMetadata) -> Self { /* insert into _meta[RELATED_TASK_META_KEY] */ }
pub fn related_task(&self) -> Option<TaskMetadata> {
    self._meta.as_ref()?.get(RELATED_TASK_META_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

// src/client/mod.rs — composes tasks_get (:555) + tasks_result (:574); platform-abstracted delay
pub async fn wait_for_task(&self, task_id: &str, opts: WaitForTaskOptions) -> Result<CallToolResult> {
    loop {
        let task = self.tasks_get(task_id).await?;
        if task.status.is_terminal() { break; }       // TaskStatus::is_terminal (tasks.rs:44)
        sleep_platform(opts.poll_interval.or(task.poll_interval)).await; // web-time precedent
    }
    self.tasks_result(task_id).await
}
```
**Wasm delay (D-10):** mirror the `web-time` adoption pattern (Cargo.toml:97) — `#[cfg(not(target_arch="wasm32"))] tokio::time::sleep` / `#[cfg(target_arch="wasm32")]` a `wasm-bindgen-futures`/`gloo-timers` future. Verify whichever wasm timer is chosen is already a transitive dep before adding (Package Legitimacy note above). The Phase 103 web-channel client (`examples/web-channel-client/`) is the direct consumer whose hand-rolled JS poll loop this shrinks.

### Anti-Patterns to Avoid
- **Implicit "parses as CallToolResult" sniffing (D-02, explicitly rejected).** `CallToolResult` serde is fully defaulted (tools.rs:531/535) — ANY object parses. Detection MUST be the explicit `ToolOutput::Result` variant, never a `from_value::<CallToolResult>` probe on the payload.
- **A second copy of the pass-through in `mod.rs` vs `core.rs` (D-05 / Phase-102 Pitfall 1).** The two `handle_call_tool` bodies drifted once. Put the tripwire + pass-through decision in ONE place (a `task_dispatch` free fn or a shared helper) called identically.
- **Routing `ToolOutput::Result` through `maybe_build_task_created`.** It isn't task-shaped so the gate returns `None` anyway; do not add a special case that could bypass D-STORE-MINTS-ID.
- **Widget-enriching a `ToolOutput::Result`.** D-04: the handler owns the full envelope — no `with_widget_enrichment` on the `Result` arm.
- **Adding `handle_output` only to the native trait.** The wasm dispatch (mod.rs:1422) also calls `handler.handle`; the wasm `ToolHandler` (in `wasm_core`/`wasm_server`, `RequestHandlerExtra` is a unit stub at mod.rs:160) must mirror the default method or the browser-served path silently keeps text-wrapping (Pitfall 3).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Task detection on a result | Ad-hoc `result["_meta"]["io.model…"]` string reads (the pmcp.run `detect_task_response` pattern) | `CallToolResult::related_task()` (D-09) | Owned by SDK; server-emit twin (`with_related_task`) guarantees both ends agree by construction. |
| `TaskMetadata` shape | Re-derive `{taskId, pollInterval, maxPollDurationSecs}` JSON | New typed `TaskMetadata` (Pattern 4) | Wire compat: `_meta`-sniffing clients (pmcp.run) already read these exact keys. |
| Poll loop | Hand-rolled `loop { get; sleep }` in each consumer | `Client::wait_for_task` | Phase 103 browser client + pmcp.run durable agent both hand-roll it today; consolidate. |
| Wasm delay | New timer crate | `web-time` pattern already in-tree (Cargo.toml:97) | Precedent set in Phase 103; avoids a new dependency + slopcheck. |
| Verbatim result serialization | Manual JSON assembly of `content`/`_meta` | `serde_json::to_value(result)` on the handler's `CallToolResult` | Ask #1 lesson: always serialize FROM the typed struct. |
| Structural CallToolResult detection | Full `from_value::<CallToolResult>` (always succeeds — D-02) | Marker fn (Pattern 3) using `Content`'s internal `type` tag | Fully-defaulted serde makes full-parse useless as a discriminator. |

**Key insight:** Phase 101/102 already wrote correct task machinery and froze the wire. Phase 104 is a *junction* phase — the failure mode is (a) drifting the two dispatchers again (D-05) or (b) recreating the swallow via implicit detection (D-02). Both are explicitly fenced.

## Runtime State Inventory

> Additive internal API + docs phase. A grep finds files; it does not find behavioral divergence between the two dispatchers or moved-value plumbing gaps. The "state" to inventory is the divergence surface and the by-value `extra` constraint.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — wire contract (`src/types/tasks.rs`, `CallToolResult`) FROZEN; `InMemoryTaskStore` is per-process; no renamed keys. `TaskMetadata` is ADDITIVE (new type, `RelatedTaskMetadata` unchanged). | None (verified: no rename, no migration). |
| Live service config | None — example/test stand up loopback servers only. The pmcp.run migration is on THEIR side (delete hand-rolled intercepts), out of this repo. | None in-repo; migration guide (D-12) documents their path. |
| OS-registered state | None. | None. |
| Secrets/env vars | None. D-08 opt-out is an explicit per-tool registration flag, NOT an env var (no global kill switch). | None. |
| Build artifacts | New `[[example]] s47` in Cargo.toml; possibly `pub mod` / new pub items. No stale artifacts. | Add Cargo.toml example block (mirror the s46 entry). |

**Divergence surface (the real "runtime state" of this refactor):** the two `handle_call_tool` bodies (`Server` mod.rs:1360-1500 vs `ServerCore` core.rs) already share the `maybe_build_task_created` gate (Phase 102) but each still owns its own text-wrap tail. TOUT-01/TOUT-02 add a `match ToolOutput` + tripwire to BOTH tails — the drift risk (D-05) is that only one gets the tripwire, or the two disagree on the empty-`content` false-positive rule. Mitigation: extract the wrap+tripwire tail into ONE `task_dispatch` (or `mod`-level) free fn both call.

**By-value `extra` constraint (drives D-03.3):** `RequestHandlerExtra` is passed BY VALUE into `handle`/`handle_output` (mod.rs:231, moved) and dropped inside the handler. A `set_result_meta()` mutation on that moved value is invisible to the dispatcher. See Open Question 1 — this is the one decision the plan cannot leave implicit.

## Common Pitfalls

### Pitfall 1: Recreating the swallow via implicit detection (D-02)
**What goes wrong:** A tempting shortcut — "if the returned `Value` deserializes as `CallToolResult`, pass it through" — silently swallows arbitrary payloads.
**Why:** `CallToolResult` serde is fully defaulted (tools.rs:531/535, `#[non_exhaustive]`, unknown fields ignored) → every JSON object parses.
**How to avoid:** ONLY the explicit `ToolOutput::Result` variant triggers pass-through. The tripwire (Pattern 3) uses high-precision structural markers, never full deserialize.
**Warning signs:** A plan task says "detect CallToolResult by trying to parse it" — reject on sight.

### Pitfall 2: The two dispatchers drift again (D-05 / Phase-102 Pitfall 1)
**What goes wrong:** The `ToolOutput` match + tripwire lands in `Server::handle_call_tool` but not the `ServerCore` twin (or with a different false-positive rule).
**Why:** The tails were never unified; only the create-path gate is shared.
**How to avoid:** Put the wrap+tripwire+pass-through decision in a single free fn (in `task_dispatch.rs` or `mod`) both call. Add a test asserting `ServerCore` and `Server` produce byte-identical output for the same `ToolOutput::Result`.
**Warning signs:** A `ToolOutput::Result` over HTTP carries top-level `_meta` but the in-process `ServerCore` path text-wraps it (or vice versa).

### Pitfall 3: The wasm dispatch path is left text-wrapping
**What goes wrong:** `handle_output` is added to the native `ToolHandler` (mod.rs:229) only; the wasm dispatch (mod.rs:1422 `handler.handle`) and the wasm `ToolHandler` (in `wasm_core`/`wasm_server`) keep calling `handle` and re-wrapping.
**Why:** There are TWO tool-handler surfaces — native (`#[cfg(not(wasm32))]` mod.rs:229) and wasm (`RequestHandlerExtra` is a unit stub at mod.rs:160; wasm modules at mod.rs:186-191). The task create-path is non-wasm, but `ToolOutput::Result` pass-through is meaningful on wasm (Phase 103 browser-served tools).
**How to avoid:** Mirror `handle_output` + the `ToolOutput` match on the wasm dispatch too, OR scope TOUT-01 explicitly and document that browser-served tools defer to a follow-up. Decide during planning; verify with `cargo check --target wasm32-unknown-unknown` (project wasm gate).
**Warning signs:** wasm build errors on `ToolOutput`, or a browser-served task-augmented result still lands in `content[0].text`.

### Pitfall 4: `set_result_meta` (D-03.3) cannot round-trip through a moved `extra`
**What goes wrong:** `RequestHandlerExtra::set_result_meta()` is added, but the `_meta` the handler sets never reaches the wire — because `extra` was moved into `handle_output` and dropped.
**Why:** `handle(&self, args, extra)` takes `extra` by value (mod.rs:231); the dispatcher has no handle to the mutated copy afterward. `http::Extensions` and `metadata` are inside that moved value.
**How to avoid:** Give the dispatcher a shared interior-mutable slot it clones BEFORE moving `extra` in — e.g. `extra` holds `Arc<Mutex<Option<Map<String,Value>>>>` for result-meta; `set_result_meta` writes it; the dispatcher reads the clone after `handle_output` returns and merges it onto the built `CallToolResult`. See Open Question 1.
**Warning signs:** A `set_result_meta` doctest passes at compile but the emitted result has no `_meta`.

### Pitfall 5: Empty-`content` and widget payloads tripping the tripwire
**What goes wrong:** Over-eager (a)-marker fires on benign `Payload` values that carry a `content`/`_meta` key, spamming WARN or failing CI via `debug_assert`.
**Why:** Some tools legitimately return `{content:[…]}` or structured `_meta`-bearing payloads.
**How to avoid:** (1) require the `content` array non-empty AND all elements valid `Content` (Pattern 3); (2) restrict (b) to the exact `RELATED_TASK_META_KEY` (not any `_meta`); (3) honor the per-tool `suppress_double_wrap_check()` opt-out (D-08). Add a test with a benign `{content:["not-a-content-item"]}` asserting NO fire.
**Warning signs:** CI `debug_assert` failures on unrelated tools; WARN noise in existing example runs.

### Pitfall 6: `RelatedTaskMetadata` vs new `TaskMetadata` confusion
**What goes wrong:** Reusing the existing `RelatedTaskMetadata { task_id }` (tasks.rs:196) for `related_task()` loses `pollInterval`/`maxPollDurationSecs` that `wait_for_task` needs.
**Why:** The existing type is minimal (only `task_id`).
**How to avoid:** Add a distinct `TaskMetadata` (Pattern 4). Confirm the SEP-1686 `_meta[related-task]` value shape the native create-path emits (core_tests.rs:881 emits `{ "taskId": store_id }`) — decide whether native emission should ALSO carry `pollInterval`, and ensure `related_task()` tolerates the minimal `{taskId}` shape (optional fields default to `None`). This is a wire-compat point the D-12 guide must state.
**Warning signs:** `related_task()` returns `None` against a native `CreateTaskResult` because deserialization required a missing field.

## Code Examples

### The wrap site to change (TOUT-01/TOUT-02 target)
```rust
// Source: src/server/mod.rs:1492-1500 (CURRENT — unconditional wrap)
// Build CallToolResult, adding structured_content for widget tools
let text = result.to_string();
let mut call_result = CallToolResult::new(vec![crate::types::Content::text(text)]);
if let Some(info) = self.tool_infos.get(&req.name) {
    call_result = call_result.with_widget_enrichment(info, result);
}
Ok(serde_json::to_value(call_result)?)
// AFTER: this is the ToolOutput::Payload arm; add the tripwire before the wrap,
//        and a sibling ToolOutput::Result arm that serializes `result` verbatim.
```

### The create-path gate that keeps precedence (D-04 — do NOT bypass)
```rust
// Source: src/server/mod.rs:1463-1490 + task_dispatch.rs:311-336
// maybe_build_task_created returns Some ONLY when task_requested && store present
// && task_support ∈ {Required,Optional} && value has taskId+status.
// A CallToolResult (ToolOutput::Result) has no top-level taskId+status ⇒ gate passes,
// so ToolOutput::Result flows to the verbatim arm without special-casing.
```

### Native `_meta[related-task]` emission — the wire-compat fact for the guide (TOUT-04)
```rust
// Source: src/server/core_tests.rs:881-892 (PROVEN)
// result["_meta"][RELATED_TASK_META_KEY]["taskId"] == result["task"]["taskId"] == store-minted id.
// ⇒ A pmcp.run-style detect_task_response (reads result._meta[related-task]) works UNCHANGED
//   against a native with_task_store() server. This is the D-12 "delete your hand-rolled path" proof.
```

### D-14 live-HTTP assertion (extend Phase 102 harness)
```rust
// Source pattern: tests/workflow_prompt_e2e_test.rs:54-97 + Phase 102 s46 harness
// Register a tool returning ToolOutput::Result(CallToolResult::new(..).with_related_task(..)),
// drive it over real StreamableHttpServer + StreamableHttpTransport, then assert on the RAW wire JSON:
//   assert!(raw["result"].get("_meta").is_some(), "_meta must survive at result TOP LEVEL");
//   assert!(raw["result"]["content"][0].get("text").map_or(true, |t| !t.as_str().unwrap().contains("\\\"_meta\\\"")),
//           "envelope must NOT be stringified into content[0].text (the agent-lake bug)");
// Consumes REAL dispatch output, never a hand-authored fixture (the note's ask #4).
```

## State of the Art

| Old Approach | Current Approach (this phase) | When Changed | Impact |
|--------------|-------------------------------|--------------|--------|
| Tool returns `Result<Value>`; dispatch stringifies any built `CallToolResult` into `content[0].text` (mod.rs:1493) | `ToolOutput::Result` reaches the wire verbatim; `_meta` preserved | Phase 104 | Closes SEP-1686 junction; pmcp.run's 3 hand-rolled servers migrate; no more pre-dispatch `tools/call` intercepts |
| Servers bypass `pmcp::Server` with a pre-dispatch intercept to preserve `_meta` (approval-mcp, team-mcp, agent-lake) | Native front-door path is correct | Phase 104 | The bypass pattern is deleted |
| Client integrators hand-roll `result._meta[related-task]` detection + poll loops | `related_task()` + `wait_for_task()` owned by SDK | Phase 104 | Both ends agree by construction |

**Deprecated/outdated:** the pmcp.run "pre-dispatch intercept before `build_per_request_server`" pattern (their PATTERNS.md §4) becomes unnecessary once TOUT-01 ships.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `ToolHandler` to extend is `src/server/mod.rs:229` (returns `Result<Value>`, backs `Server.tools`), NOT `src/server/traits.rs:25` (returns `CallToolResult`, part of `RequestHandler`) | Standard Stack / Pattern 1 | If a hand-rolled pmcp.run server implements the `traits.rs` variant instead, the plan targets the wrong trait. LOW — `Server.tools: HashMap<_, Arc<dyn ToolHandler>>` (mod.rs:323) resolves to the mod.rs:229 trait; verified. |
| A2 | `ToolOutput::Result` needs NO routing through `maybe_build_task_created` because a `CallToolResult` is not top-level task-shaped (no `taskId`+`status`) | Pattern 2 / D-04 | If some `CallToolResult` legitimately carries a top-level `taskId`, the gate could mis-fire. LOW — `CallToolResult` fields are `content/is_error/structured_content/_meta` only (tools.rs:526); no `taskId`/`status` at top level. |
| A3 | `set_result_meta` (D-03.3) requires a shared interior-mutable slot on `RequestHandlerExtra` because `extra` is moved by value | Pitfall 4 / Open Q 1 | If a simpler mechanism exists (e.g. returning `extra`), the plan over-engineers. MEDIUM — confirmed `handle` takes `extra` by value (mod.rs:231); some plumbing IS required. |
| A4 | The wasm `ToolHandler` surface needs the same `handle_output` default to avoid leaving browser-served tools text-wrapping | Pitfall 3 | If wasm tools never return `_meta`-bearing results, mirroring is unnecessary scope. MEDIUM — Phase 103 browser client is a stated consumer; confirm wasm ToolHandler location during planning. |
| A5 | Empty `content: []` should NOT trip the tripwire; non-empty all-valid-`Content` should | Pattern 3 / Pitfall 5 | If the desired policy differs, false-positive rate changes. LOW-MEDIUM — "near-zero false positive" (D-07) implies conservative firing; confirm with planner. |
| A6 | The wasm delay for `wait_for_task` (D-10) can reuse already-transitive `web-time`/`wasm-bindgen-futures` without a new crate | Standard Stack / Pattern 4 | If a new timer crate is needed, it must pass the Package Legitimacy Gate. LOW — `web-time` precedent exists (Cargo.toml:97). |

## Open Questions (RESOLVED)

> All four resolved during planning. Inline `RESOLVED:` markers below cite the plan/task that carries each decision.


1. **How does `set_result_meta` (D-03.3) reach the dispatcher across the by-value `extra` move?**
   - **RESOLVED: 104-04 Task 2** — shared `Arc<Mutex<Option<Map>>>` result-meta slot on `RequestHandlerExtra`, pre-cloned by the dispatcher before the by-value move, read+merged after `handle_output` on the Payload path only (documented merge-not-overwrite).
   - What we know: `handle(&self, args, extra)` takes `extra` by value (mod.rs:231); mutations are dropped inside the handler.
   - What's unclear: exact mechanism — shared `Arc<Mutex<Option<Map>>>` slot the dispatcher pre-clones, vs. a returned-`extra` signature change (breaking), vs. reading back via `http::Extensions` (also moved).
   - Recommendation: shared interior-mutable result-meta slot on `RequestHandlerExtra`, set by the handler, read+merged by the dispatcher after `handle_output` returns. Because `handle_output`'s DEFAULT calls `handle`, this slot must be populated before the default delegates, or `set_result_meta` only works on the `Payload` (default) path — document the interaction. This is the sharpest plan-shaping decision; spike it first.

2. **What is the canonical `TaskMetadata` shape, and should the native create-path emit `pollInterval`/`maxPollDurationSecs` (not just `taskId`)?**
   - **RESOLVED: 104-01 Task 1** — new `TaskMetadata { task_id, poll_interval?, max_poll_duration_secs? }` (camelCase, `#[non_exhaustive]`, skip-if-none); `related_task()` tolerates the minimal `{taskId}` shape; native emission stays minimal.
   - What we know: native emission is `_meta[related-task] = { "taskId": store_id }` (core_tests.rs:881); `Task` already carries `poll_interval` (tasks.rs:108). `RelatedTaskMetadata` has only `task_id`.
   - What's unclear: whether `related_task()`/`wait_for_task` should read `pollInterval` from `_meta` (needs the create-path to emit it) or fall back to `tasks_get`'s `task.poll_interval`.
   - Recommendation: define `TaskMetadata` with optional `poll_interval`/`max_poll_duration_secs`; `related_task()` tolerates the minimal `{taskId}` shape; `wait_for_task` prefers `_meta` value, falls back to the polled `Task.poll_interval`, then a default. Keep native emission minimal (`{taskId}`) unless the guide needs richer `_meta` — a wire-compat decision for D-12.

3. **One shared wrap+tripwire tail, or per-dispatcher with a shared helper?**
   - **RESOLVED: 104-02 Task 2 (+ 104-03 Task 2)** — single shared decision helper in `src/server/task_dispatch.rs` called by both dispatchers (Payload-vs-Result + middleware-bypass rule); tripwire wired into the same shared Payload wrap site; parity test asserts byte-identical output.
   - What we know: the two `handle_call_tool` tails are still separate (only the create-path gate is shared).
   - Recommendation: extract the `match ToolOutput { … } + tripwire + wrap` into ONE free fn in `task_dispatch.rs` (D-05), called by both. Test byte-identical output across dispatchers.

4. **Does pmcp's own client swallow task deserialize errors elsewhere (deferred ask #5 check)?**
   - **RESOLVED: 104-05 Task 3** — audit conclusion documented in the migration guide: SDK client is clean (`parse_task_payload` WARNs; `call_tool_with_task` maps via `Error::parse`); the ask #5 gap is on the pmcp.run durable-client side.
   - What we know: `parse_task_payload` already WARNs on deserialize failure (client/mod.rs:629-640) — the SDK side is already good.
   - Recommendation: quick audit of `call_tool_with_task` (mod.rs:508-542) confirms it maps parse errors via `Error::parse` (not swallowed). Note in the guide that the SDK client is clean; the ask #5 gap is in THEIR durable client.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (stable) | build/test | ✓ (assumed local) | latest stable (`rustup update stable` before release per CLAUDE.md) | — |
| `pmcp` workspace, `full` features (incl. `streamable-http`) | HTTP round-trip + example | ✓ in-tree | 2.11.0 (Cargo.toml:3) | — |
| `web-time` | wasm delay for `wait_for_task` (D-10) | ✓ in-tree | 1 (Cargo.toml:97) | — |
| `tokio` runtime | native sleep + async tests | ✓ in-tree | workspace | — |
| wasm target (`wasm32-unknown-unknown`) | verify wasm ToolHandler + client `wait_for_task` compile | project wasm gate | — | `cargo check --target wasm32-unknown-unknown` |
| `pmat` 3.15.0 | CI cognitive-complexity gate (≤25) | CI-only | 3.15.0 | local `pmat analyze complexity --max-cognitive 25` |
| `make` (quality-gate, doc-check) | verification | ✓ | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** PMAT is CI-only; keep new fns ≤ cog 25 (CLAUDE.md). The `match ToolOutput` + tripwire branch is small; the risk is the wasm-gated delay helper — keep the `#[cfg]` split trivial.

## Validation Architecture

> `.planning/config.json` → `workflow.nyquist_validation: true`. Section included. This is protocol-shape work — the acceptance gate is the D-14 live HTTP `_meta`-at-top-level assertion consuming REAL dispatch output (the note's "resolved only via a live round-trip" rule).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` (integration tests in `tests/`) + `proptest` (in-tree) + `make` targets |
| Config file | none — `cargo test` + `Makefile` (`test-unit`/`test-integration`/`test-examples`/`test-property`/`test-fuzz`) |
| Quick run command | `cargo test --features full tool_output` |
| Full suite command | `make quality-gate` (fmt + clippy pedantic/nursery + build + test + audit) AND `make doc-check` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TOUT-01 | `ToolOutput::Result` reaches wire verbatim; `_meta` preserved; no widget enrichment | integration | `cargo test --features full tool_output_result_passthrough` | ❌ Wave 0 |
| TOUT-01 | `handle_output` default delegates to `handle` → `Payload` (existing handlers unchanged) | unit | `cargo test --features full tool_output_default_delegation` | ❌ Wave 0 |
| TOUT-01 | `ServerCore` and `Server` emit byte-identical output for the same `ToolOutput::Result` (D-05) | integration | `cargo test --features full tool_output_dispatch_parity` | ❌ Wave 0 |
| TOUT-02 | tripwire fires (WARN + debug_assert) on `{content:[valid]}` and `{_meta:{related-task}}`; does NOT fire on empty content / benign payload / after `suppress_double_wrap_check` | unit | `cargo test --features full double_wrap_tripwire` | ❌ Wave 0 |
| TOUT-02 | property: no `Payload` value ever both trips the tripwire AND is a legitimate non-CallToolResult (fuzz the marker fn) | property/fuzz | `cargo test --features full proptest_tripwire_precision` | ❌ Wave 0 |
| TOUT-03 | `related_task()` returns `Some` for native `_meta[related-task]` (minimal `{taskId}`) and `None` otherwise; `with_related_task` round-trips | unit | `cargo test --features full related_task_accessor` | ❌ Wave 0 |
| TOUT-03 | `wait_for_task` polls to terminal then returns `tasks_result`; honors `pollInterval`; wasm compiles | integration | `cargo test --features full wait_for_task_lifecycle` + `cargo check --target wasm32-unknown-unknown` | ❌ Wave 0 |
| TOUT-04 | live HTTP: `ToolOutput::Result` carries `_meta` at result top level; NOT stringified into content[0].text | integration (HTTP loopback) | `cargo test --features full tool_output_result_http` | ❌ Wave 0 |
| TOUT-04 | BEFORE/AFTER example compiles & runs; migration doc links resolve | example/doctest | `cargo run --example s47_task_augmented_result --features full`; `make doc-check` | ❌ Wave 0 |
| TOUT-* | no regression to Phase 101/102 task lifecycle | integration | `cargo test --features full tool_as_task_lifecycle` | ✅ exists |

### Sampling Rate
- **Per task commit:** `cargo test --features full tool_output` + `cargo test --features full tool_as_task_lifecycle` (Phase 101/102 no-regression).
- **Per wave merge:** `cargo test --features full` (full integration incl. HTTP round-trip) + `cargo check --target wasm32-unknown-unknown`.
- **Phase gate:** `make quality-gate` AND `make doc-check` green; PMAT complexity ≤ cog 25 (CI); ALWAYS coverage (unit + property + fuzz + `cargo run --example s47`).

### Wave 0 Gaps
- [ ] `tests/tool_output_result_http.rs` — D-14 live HTTP `_meta`-at-top-level (extend Phase 102 harness / s46 pattern)
- [ ] `tests/tool_output_dispatch_parity.rs` (or in-module) — D-05 byte-identical `Server` vs `ServerCore`
- [ ] Unit tests for `looks_like_call_tool_result` (fire / no-fire / suppressed) + proptest precision
- [ ] Unit tests for `related_task()` / `with_related_task()` round-trip + minimal-shape tolerance
- [ ] `examples/s47_task_augmented_result.rs` + Cargo.toml `[[example]]` block
- [ ] Fuzz target or proptest for the tripwire marker fn (ALWAYS requirement)
- [ ] No framework install needed (Rust built-in harness + existing `proptest`)

## Security Domain

> `security_enforcement` not explicitly `false` in config → treat as enabled. This phase is additive: it adds NO new auth surface, NO new network input parsing (client `wait_for_task` reuses existing typed `tasks/*` calls), NO crypto. The controls are *preserving* existing semantics.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (preserve) | The `ToolOutput` branch runs AFTER the existing auth/middleware path (mod.rs:1367-1394); the verbatim `Result` arm does not touch auth. Do not relocate the branch above auth validation. |
| V3 Session Management | no | No session state added. |
| V4 Access Control | yes (preserve) | Create-path owner-scoping (`resolve_owner`, task_dispatch.rs:168) keeps precedence (D-04). `ToolOutput::Result` is handler-owned content — the handler is responsible for not leaking cross-owner data in `_meta`, same as today's `Value` return. |
| V5 Input Validation | yes | `wait_for_task` sends typed `GetTaskRequest`/`GetTaskPayloadRequest` (no hand-rolled params). The tripwire only READS the produced `Value` (no deserialization side effects beyond a discardable `from_value::<Content>` probe). `TaskMetadata` deserialize is fallible + returns `None` on bad shape (no panic). |
| V6 Cryptography | no | None. |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Handler smuggles a task envelope to bypass D-STORE-MINTS-ID via `ToolOutput::Result` | Spoofing/Tampering | `ToolOutput::Result` is NOT the create-path — it emits a `CallToolResult`, never mints a store task. A tool that wants a real task must go through `maybe_build_task_created` (task-shaped `Payload`), which still mints the store id. The `_meta[related-task].taskId` in a `Result` is a client-side hint only; `tasks/get` still enforces owner scoping server-side. |
| `debug_assert!` DoS in production | Availability | D-06: release builds NEVER panic — only `tracing::warn!` in release; `debug_assert!` is compiled out. |
| Tripwire probe cost on large payloads | Availability | Marker fn is O(content array len); short-circuits on the `_meta` key check first; no full-document deserialize (D-02/D-07). Guard against pathological arrays if needed. |
| Cross-owner task read via `wait_for_task` | Information Disclosure | Reuses `tasks_get`/`tasks_result` which resolve owner server-side (task_dispatch.rs:352, `-32002`/owner-scoped store). Client convenience adds no new trust. |

## Sources

### Primary (HIGH confidence — live source, this session, pmcp 2.11.0)
- `src/server/mod.rs` — `ToolHandler` trait (229-238), `Server` struct + `tools` field (320-323), native/wasm dispatch of `handler.handle` (1394/1422), `ToolRejected` mapping (1438-1442), create-path gate call (1463-1490), unconditional text-wrap (1492-1500), `ServerBuilder` tool registration (2265, 2286, 2373, 2657 `tool_typed_with_output`), wasm module gating (152-217)
- `src/server/task_dispatch.rs` — `TaskDispatch` (158), `resolve_owner` (168), `maybe_build_task_created` gate (311-336), `handle_tasks_result` (346-396), `route_tasks_*` (399-499), `route_tasks_endpoint` (500)
- `src/types/tools.rs` — `CallToolResult` (523-559, fully-defaulted serde 531/535, `_meta` 556), constructors/builders (561-634, `with_widget_enrichment` 615)
- `src/types/tasks.rs` — `RELATED_TASK_META_KEY` (9), `TaskStatus::is_terminal` (44), `Task.poll_interval` (108), `RelatedTaskMetadata { task_id }` (193-199), `CreateTaskResult` (205)
- `src/types/content.rs` — `Content` enum `#[serde(tag="type")]` (62-80) — the tripwire precision basis
- `src/client/mod.rs` — `call_tool_with_task` (508-542), `tasks_get` (555-567), `tasks_result` (574-588), `tasks_list` (591-603), `tasks_cancel` (606-620), `parse_task_payload` WARN-on-failure (629-640)
- `src/server/cancellation.rs` — `RequestHandlerExtra` fields incl. `task_request` (179-224), builders + `is_task_request`/`set_metadata` (231-374), by-value move constraint
- `src/server/core_tests.rs` — native `_meta[related-task]` == store-minted id proof (855-896)
- `src/server/traits.rs` — the OTHER `ToolHandler` (25-36, `call_tool -> CallToolResult`) — confirmed NOT the target (A1)
- `Cargo.toml` — version 2.11.0 (3), `web-time = "1"` (97); `examples/` highest slot = `s46` ⇒ `s47` free
- `.planning/phases/104-task-augmented-tool-results-dx/104-CONTEXT.md` — locked decisions D-01..D-14

### Secondary (MEDIUM confidence)
- `/Users/guy/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-tool-as-task-dx.md` — pmcp.run incident report (5 variants, asks 6-9, agent-lake wire probe, "live round-trip not code reading" lesson). THE spec; cross-checked against the cited pmcp source lines.
- `.planning/phases/102-http-task-dispatch/102-RESEARCH.md` — shared-seam map, two-dispatcher-drift pitfall, HTTP loopback harness that D-14 extends.

### Tertiary (LOW confidence)
- None — no web research required for this internal-refactor phase.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all types/sites verified in-tree at cited lines; zero new deps.
- Architecture (ToolOutput + shared seam): HIGH — the wrap site, create-path gate precedence, and shared `TaskDispatch` unit are verified; the branch is additive.
- `set_result_meta` plumbing (D-03.3): MEDIUM — the by-value `extra` constraint is confirmed but the exact retrieval mechanism is a plan decision (Open Q 1).
- wasm ToolHandler mirroring: MEDIUM — the wasm dispatch path exists (mod.rs:1422) but the exact wasm trait location needs a planning-time confirm.
- Pitfalls: HIGH — each derived from a concrete source fact (serde defaults, internal tag, by-value move, two tails).

**Research date:** 2026-07-04
**Valid until:** stable internal architecture — ~30 days, or until `src/server/{mod,core,task_dispatch}.rs`, `src/types/{tasks,tools,content}.rs`, or `src/client/mod.rs` materially change.
