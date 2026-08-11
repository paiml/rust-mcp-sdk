# SEP-1686 — Task-Augmented Tool Results (junction rationale + migration guide)

**Status**: Implemented (pmcp 2.12+)
**Protocol area**: MCP Tasks (2025-11-25) × tool results
**Phase**: 104 — Task-Augmented Tool Results DX
**Date**: 2026-07-04

> Companion to [`tasks-feature-design.md`](./tasks-feature-design.md) (the base
> MCP Tasks design). This document records ONLY the SEP-1686 junction — how a
> tool returns a full `CallToolResult` (`_meta` included) through the normal
> `Server` dispatch front door — plus the migration path off the three
> hand-rolled pre-2.12 `_meta` workarounds. It does not restate the tasks
> lifecycle; see the base doc for that.

---

## 1. The junction and the bug class it closes

Phase 101/102 froze the `tasks/*` wire contract and made a tool serve as an
async MCP Task via native `task_store()` machinery. What they deliberately did
NOT cover is the OTHER direction: a normal, synchronous `tools/call` whose
`CallToolResult` needs to carry `_meta` — most importantly an
`_meta["io.modelcontextprotocol/related-task"]` envelope pointing the caller at
a task the tool just kicked off.

Before 2.12 there was no supported way to do that. A `ToolHandler` returns a
`serde_json::Value`, and dispatch UNCONDITIONALLY stringified that value into
`content[0].text` (the text-wrap at `src/server/mod.rs`). A handler that
hand-built a `CallToolResult`-shaped `Value` (with its own `content` and
`_meta`) had that ENTIRE object serialized into a single text block. The
top-level `_meta` never reached the wire, so a `_meta`-sniffing client saw no
related task — silently.

The pmcp.run team's incident report catalogued **five variants of this one bug
class**, including:

- a hand-rolled `_meta` task tool whose related-task pointer vanished into
  `content[0].text`; and
- the **agent-lake double-wrap**: an outer result with NO `_meta` at all and the
  whole serialized `CallToolResult` living as a JSON string inside
  `content[0].text`.

One variant ran silently in production for **two weeks**. The verdict that framed
this phase: *"not wrong, but incomplete — and incomplete in a way that fails
silently, which is the worst kind."*

The junction is closed by four additive deliverables (no breaking change to
`ToolHandler`, `Server`/`ServerBuilder`, or wire shapes):

| ID | Deliverable |
|----|-------------|
| TOUT-01 | `ToolOutput::Result(CallToolResult)` reaches the wire verbatim via `ToolHandler::handle_output` (default delegates to `handle`); sugar: `CallToolResult::with_related_task`, `ServerBuilder::tool_with_result`, `RequestHandlerExtra::set_result_meta`. |
| TOUT-02 | Double-wrap tripwire: dispatch `warn!`s (all builds) and `debug_assert!`-fails (debug/CI) before text-wrapping a `Value` that structurally resembles a built `CallToolResult`; per-tool `suppress_double_wrap_check` opt-out. |
| TOUT-03 | Client detection owned by the SDK: `CallToolResult::related_task()` accessor + `Client::wait_for_task` / `wait_for_related_task` poller. |
| TOUT-04 | This guide + the `s47` runnable BEFORE/AFTER example + the D-14 live-HTTP wire-shape acceptance gate. |

The mapping back to the pmcp.run asks: **asks 6/7/8/9 → TOUT-01/02/03/04**.
Ask-6-option-(b) (implicit "returned `Value` parses as `CallToolResult` → pass
it through" sniffing) was **rejected**: `CallToolResult`'s serde is fully
defaulted (`#[serde(default)]` on `content`/`is_error`, unknown fields ignored),
so ANY JSON object parses — implicit detection would silently swallow arbitrary
payloads and recreate the bug class. Detection is therefore EXPLICIT
(`ToolOutput::Result` / `tool_with_result`), never inferred.

---

## 2. Migration: hand-rolled `_meta` → native `with_task_store()` + `ToolOutput::Result`

The three pre-2.12 hand-rolled patterns and their native replacements:

### Pattern A — hand-built `CallToolResult`-shaped `Value` on the Payload path

**Before** (the anti-pattern — `_meta` is lost to text-wrap):

```rust,ignore
// Returns a Value that LOOKS like a CallToolResult. Dispatch stringifies the
// whole thing into content[0].text; the top-level _meta never reaches the wire.
Ok(serde_json::json!({
    "content": [{ "type": "text", "text": "processing started" }],
    "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "hand-rolled" } }
}))
```

**After** — return a real `CallToolResult` via `tool_with_result`; it reaches the
wire VERBATIM:

```rust,ignore
use pmcp::types::{CallToolResult, Content};
use pmcp::types::tasks::TaskMetadata;

server.tool_with_result::<StartArgs>("start_job", move |_args, _extra| {
    let task_id = store_minted_id.clone(); // from TaskStore::create — NOT hand-written
    Box::pin(async move {
        Ok(CallToolResult::new(vec![Content::text("processing started")])
            .with_related_task(TaskMetadata::new(task_id)))
    })
})
```

The related-task id MUST be a **store-minted** id (from
`TaskStore::create` / the native `task_store()` create-path), never a fabricated
literal — that is what makes it resolvable via `tasks/get` / `tasks/result`.

### Pattern B — a hand-written `ToolHandler` impl

Override `handle_output` to return `ToolOutput::Result`:

```rust,ignore
async fn handle_output(&self, args: Value, extra: RequestHandlerExtra)
    -> pmcp::Result<pmcp::ToolOutput>
{
    Ok(pmcp::ToolOutput::Result(
        CallToolResult::new(vec![Content::text("done")])
            .with_related_task(TaskMetadata::new(task_id)),
    ))
}
```

`handle()` stays as a serialize fallback for non-dispatch callers; the default
`handle_output` delegates to it, so existing handlers are untouched.

### Pattern C — an existing handler that only needs to ADD `_meta`

If the handler is otherwise happy on the Payload path and just needs to stamp a
related-task pointer, one call retrofits it — no `handle_output` impl needed:

```rust,ignore
extra.set_result_meta(related_task_meta_map); // merges onto the outgoing result
```

`set_result_meta` MERGES with **handler-key-wins** precedence (unrelated
widget/native `_meta` keys are preserved).

**Path scope (amended by D-06, Phase 118.1 plan 09).** The merge now applies on
BOTH output paths and BOTH dispatchers. It originally affected the Payload path
only and was deliberately ignored on the `ToolOutput::Result` path, on the
reasoning that a verbatim handler already owns its full envelope. That drop
turned out to be load-bearing rather than cosmetic — the server-to-client
elicitation wiring runs straight through the verbatim arm, so a handler that
retrofitted `_meta` with one call silently shipped none. The verbatim arm still
bypasses response middleware, the task create-path gate and the text-wrap tail
(D-04 / D-04a); the bypass covers the response *pipeline*, not the handler's own
`_meta`, which is authored by the same handler at the same trust level as the
envelope it returns. When a verbatim handler sets `_meta` both ways, the
`set_result_meta` key wins the collision and the envelope's unrelated keys
survive.

### The runnable diff

`examples/s47_task_augmented_result.rs` shows Pattern A BEFORE (registered with
`suppress_double_wrap_check` ONLY so the anti-pattern can run without tripping the
Plan 03 `debug_assert`) and AFTER (native `task_store()` + `tool_with_result`
with a store-minted id) over a live HTTP loopback. The diff IS the migration.

---

## 3. Wire-compat confirmation — `_meta`-sniffing clients detect native tasks unchanged

The good-news fact the pmcp.run team needs before deleting their hand-rolled
intercepts: **the native create-path already emits
`_meta[related-task]` with the store-minted id.**

`src/server/core_tests.rs` (`test_...task...`, lines ~855-896) proves it against
real dispatch output: a task-augmented `tools/call` returns a `CreateTaskResult`
whose `result._meta["io.modelcontextprotocol/related-task"]["taskId"]` equals the
store-minted `task.taskId`. So pmcp.run's durable-agent `detect_task_response`
(which reads `result._meta[related-task]`) works **UNCHANGED** against a native
`with_task_store()` server — no client change required.

Two related SDK guarantees:

- **Required-without-store is a build error.** A tool that declares
  `TaskSupport::Required` with NO `TaskStore` makes `build()` return an error,
  never a hollow advertised capability.
- **The SDK client already WARNs on task-payload deserialize failure**
  (`src/client/mod.rs`, `parse_task_payload`, ~lines 629-640) — it never silently
  swallows a malformed task payload. The pmcp.run "ask #5" swallow gap is on
  THEIR durable client, not in pmcp's client.

The D-14 acceptance gate (`tests/tool_output_result_http.rs`) locks the
`ToolOutput::Result` wire shape over a real `StreamableHttpServer` +
`StreamableHttpTransport` round-trip: `result._meta` is present at the result TOP
LEVEL and `content[0].text` is NOT a stringified envelope. It consumes REAL
dispatch output, never a hand-authored fixture, and runs in CI.

---

## 4. ⚠️ D-04a — the response-middleware bypass (a `ToolOutput::Result` tool owns its own redaction)

`ToolOutput::Result` (and its sugar `tool_with_result`) sends the handler's
`CallToolResult` to the wire **VERBATIM**. That path **BYPASSES response
middleware** — redaction, sanitization, and audit hooks (`ToolMiddleware`
`on_response`) DO NOT run for this variant — as well as text-wrapping and widget
enrichment. **The handler is therefore responsible for its OWN redaction and
sanitization of both `content` and `_meta`**, at the same trust level as
returning a raw `Value` today.

This is a **deliberate, user-approved, LOCKED decision** (D-04a — *"keep the
bypass, harden it"*). There is intentionally NO result-aware response-middleware
hook. The bypass ships WITH these hardening mitigations:

1. **Loud rustdoc** on the `ToolOutput::Result` variant AND on `tool_with_result`
   (not only this guide) stating the value goes to the wire verbatim and bypasses
   response middleware.
2. A **request-middleware-still-runs** guarantee: `ToolMiddleware::on_request`
   still fires before a `ToolOutput::Result` tool executes — only RESPONSE
   middleware is bypassed (regression-tested).
3. A **handler-error-path** guarantee: a handler that returns `Err(_)` from
   `handle_output` still routes through the normal error path
   (`handle_tool_error`); the bypass is scoped to the successful `Result` arm only
   (regression-tested).
4. This callout, surfaced where authors read it (guide + book chapter + rustdoc),
   not buried.

What is NOT bypassed: **request** middleware still runs, unconditional token
cleanup still runs, and handler errors still route through error handling.

What IS bypassed — including the create-path gate: in BOTH dispatchers the
`DispatchOutput::Verbatim` arm returns BEFORE the Phase 102 task create-path
gate ever executes; the gate is structurally bypassed, never consulted.
Client-visible consequence: a **task-augmented** `tools/call` to a
`tool_with_result` / `handle_output`-overriding tool receives the plain
`CallToolResult` verbatim and NO task is minted — even if the tool declares
`TaskSupport::Required` (the two contracts conflict; do not combine them).
This is regression-tested in `tests/tool_output_passthrough.rs`
(`task_augmented_call_to_verbatim_tool_returns_plain_result`).

**Rule of thumb:** if your tool returns `ToolOutput::Result` / uses
`tool_with_result`, redact/sanitize inside the handler BEFORE building the
`CallToolResult`. Do not rely on a response-middleware layer that will not run.

---

## 5. References

- `examples/s47_task_augmented_result.rs` — the runnable BEFORE/AFTER migration.
- `tests/tool_output_result_http.rs` — the D-14 live-HTTP wire-shape gate.
- `src/server/core_tests.rs` (~855-896) — native `_meta[related-task]` emission proof.
- `src/client/mod.rs` (~629-640) — client WARNs on task-payload deserialize failure.
- `docs/design/tasks-feature-design.md` — the base MCP Tasks design (not restated here).
- pmcp-book: *Task-Augmented Tool Results (SEP-1686)* chapter — the user-facing guide.
```
