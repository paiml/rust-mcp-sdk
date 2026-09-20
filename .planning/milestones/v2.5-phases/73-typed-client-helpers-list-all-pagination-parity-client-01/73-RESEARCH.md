# Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01) - Research

**Researched:** 2026-04-21
**Domain:** pmcp `Client` ergonomics — typed-arg serialization helpers + auto-paginating list helpers
**Confidence:** HIGH (all findings verified against HEAD of `main`, pmcp 2.5.0)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (D-01 … D-15)

**Typed Call API Shape**
- **D-01:** `call_tool_typed<T: Serialize>(&self, name: impl Into<String>, args: &T) -> Result<CallToolResult>` — args borrowed (`&T`).
- **D-02:** Serialize failure → `Error::Validation` with a message naming the offending field path if serde surfaces one.
- **D-03:** Method names: `call_tool_typed`, `call_tool_typed_with_task`.
- **D-04:** Integration tests reuse the existing in-process / `MockTransport` test harness.

**Typed Prompt Helper**
- **D-05:** Name is `get_prompt_typed` (not `call_prompt_typed`); fix REQUIREMENTS.md §55 as doc correction in this phase.
- **D-06:** `T: Serialize` → `HashMap<String, String>` coercion rules:
  1. `serde_json::to_value(args)`.
  2. Require result is `Value::Object` — non-object → `Error::Validation("prompts/get arguments must serialize to a JSON object")`.
  3. Per entry: `Value::String(s)` passthrough; `Number`/`Bool` → `to_string()`; `Null` → skip; `Array`/`Object` → `serde_json::to_string(&value)?`.
- **D-07:** Doctest uses `#[derive(Serialize)] struct SummaryArgs { topic: String, length: u32 }`.

**Pagination Config Surface**
- **D-08:** Add `ClientOptions` struct (new file `src/client/options.rs` recommended or inline — see Claude's Discretion). Fields: `max_iterations: usize` (default `100`). Derives `Debug, Clone, Default`. **`#[non_exhaustive]`** so a future PARITY-CLIENT-02 can add `StrictMode`/typed-output knobs non-breaking. Construct via `ClientOptions::default()` + field-update idiom.
- **D-09:** Add `Client::with_options(transport: T, options: ClientOptions) -> Self`. Existing `Client::new` internally delegates to `with_options(…, ClientOptions::default())`. **⚠️ See "Landmines" — the name `with_options` is already taken by a different signature.**
- **D-10:** `max_iterations` exceeded → `Error::Validation` naming the cap (`"list_all_tools exceeded max_iterations cap of 100 pages"`). No silent partial return.
- **D-11:** No `page_size` field, no per-request `limit`. MCP spec request types only carry `cursor`.

**Method Coverage**
- **D-12:** Four `list_all_*` methods: `list_all_tools`, `list_all_prompts`, `list_all_resources`, `list_all_resource_templates`.
- **D-13:** Both `call_tool_typed_with_task` AND `call_tool_typed_and_poll` — full typed parity with the non-typed trio.
- **D-14:** Example footprint: update `examples/c02_client_tools.rs`; add new pagination demo example. **⚠️ See "Landmines" — the name `c08_client_list_all` conflicts with the already-landed `c08_oauth_dcr.rs` from Phase 74.**

**Requirements Doc-Fix**
- **D-15:** Update `.planning/REQUIREMENTS.md:55` to rename `call_prompt_typed` → `get_prompt_typed` in the PARITY-CLIENT-01 row.

### Claude's Discretion
- Exact error message wording for `Error::Validation` cases (as long as cap number + method name appear).
- Whether `src/client/options.rs` is new file or inline.
- Whether `list_all_*` accept an optional per-call `max_iterations` override (default: no override).
- Exact rustdoc structure per method (each must carry a `rust,no_run` doctest per Phase 66 convention).
- Whether property/fuzz tests live under existing `tests/property_tests.rs` or a new dedicated file.

### Deferred Ideas (OUT OF SCOPE)
- Additional `ClientOptions` tunables (timeout, retry, headers).
- `c09_client_prompts_typed` dedicated example (prompt-typed DX lives in the `get_prompt_typed` rustdoc).
- **Typed result / Strict / Trust client modes** (candidate PARITY-CLIENT-02). Phase 73 keeps `ClientOptions` `#[non_exhaustive]` to preserve this path.
- Per-call `max_iterations` override.
- `ClientNotificationHandler` trait (CLIENT-03).
- Client-side `ProgressDispatcher` (CLIENT-04).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PARITY-CLIENT-01 | Ship typed-input `call_tool_typed` / `get_prompt_typed` helpers and auto-paginating `list_all_tools` / `list_all_prompts` / `list_all_resources` / `list_all_resource_templates` convenience methods on `Client`, reducing client boilerplate to one call per operation. | Exact current delegate signatures confirmed at `src/client/mod.rs` lines 339-357 / 416-441 / 463-497 / 620-697 / 749-767 / 825-849 / 891-909 / 948-969. `Cursor = Option<String>` consistent across all 4 list request/result pairs. `Error::Validation` available via `Error::validation(msg)` constructor. |

This phase implements PARITY-CLIENT-01 exactly as scoped by `.planning/phases/69-…/69-PROPOSALS.md` Proposal 2, with the four tweaks captured in CONTEXT.md (D-05 `get_prompt_typed`, D-12 resource_templates symmetry, D-13 full typed task-aware parity, D-11 no `page_size`).
</phase_requirements>

## Summary

Phase 73 is a textbook additive-ergonomics phase. The pmcp `Client` is a shared-reference (`&self`) async API built on `Arc<RwLock<Transport>>` interior mutability. Every existing `call_*` / `list_*` / `get_*` method takes `&self`, returns `Result<SomeResult>`, and ends with the same `ResponsePayload::{Result, Error}` match. That means all nine new helpers (`call_tool_typed`, `call_tool_typed_with_task`, `call_tool_typed_and_poll`, `get_prompt_typed`, `list_all_tools`, `list_all_prompts`, `list_all_resources`, `list_all_resource_templates`, + a new `ClientOptions` config struct) are **pure thin wrappers** — no new wire-level plumbing, no locking concerns across awaits, no `&mut self` collisions.

The existing codebase already provides every primitive needed: `Error::validation(msg)`, `Cursor = Option<String>` consistent across all four list request/result pairs, a `MockTransport` inside `src/client/mod.rs` `mod tests` that can drive paginated-server unit tests, and `serde_json` with `raw_value`/`preserve_order` features already in the workspace.

**Primary recommendation:** Three plans align naturally — (Plan 1) typed-call helpers + `ClientOptions` scaffold + typed path unit/property tests; (Plan 2) `list_all_*` auto-pagination family + integration tests against paginated `MockTransport`; (Plan 3) examples, rustdoc, README index, REQUIREMENTS.md doc-fix, CHANGELOG. **Resolve two landmines before plans are written**: (a) rename the proposed `Client::with_options(transport, options)` to something else since the name is already taken by `Client::with_options(transport, client_info, ProtocolOptions)`; (b) pick `c09_client_list_all.rs` instead of `c08_client_list_all.rs` because `c08` is now `c08_oauth_dcr.rs` (shipped in Phase 74).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Typed call helpers (`call_tool_typed*`, `get_prompt_typed`) | pmcp core — `src/client/mod.rs` `impl<T: Transport> Client<T>` | `serde_json::to_value` (workspace dep) | Client ergonomics live on `Client`; serialization is a pre-send validation step that reuses existing serde infrastructure. |
| Auto-pagination (`list_all_*`) | pmcp core — same `impl<T: Transport> Client<T>` block | — | Loops purely over existing single-page methods; no new transport, no new RPC, no new types. |
| `ClientOptions` configuration | pmcp core — new type (either `src/client/options.rs` or inline in `src/client/mod.rs`), re-exported from `lib.rs` | `ClientBuilder` (potential follow-on wiring) | Mirrors the existing `ProtocolOptions` pattern — a plain `Debug + Clone + Default` config struct owned by `Client`. |
| Examples | `examples/c02_client_tools.rs` (update) + `examples/cNN_client_list_all.rs` (new) | `examples/README.md` index | Phase 66 convention: numbered `cNN_` files under `examples/` registered in `examples/README.md`. |
| Docs (`rustdoc`) | Inline on each new `pub async fn`, each carries a `rust,no_run` doctest | — | Phase 66 doctest convention is repo-wide. |

## Standard Stack

No new external dependencies. Everything is already in the workspace.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` | 1.0 (workspace) | `T: Serialize` bound on typed helpers | [VERIFIED: workspace Cargo.toml] already used throughout `src/types/`. |
| `serde_json` | 1.0 (features: `raw_value`, `preserve_order`) | `to_value(&T)` / `to_string(&Value)` for D-01 and D-06 | [VERIFIED: `Cargo.toml:51`] already used by every existing `list_*` / `call_*` method. |
| `thiserror` | — | `Error::Validation` variant already defined | [VERIFIED: `src/error/mod.rs:37-38`] |
| `async_trait` | — | Already used for `Transport`, `ToolHandler`, etc. | [VERIFIED: repo convention] |
| `tokio` / `futures-locks` | — | `RwLock` for interior mutability on `Client` | [VERIFIED: `src/client/mod.rs:26-33`] already wired per-target (`cfg(target_arch)`). |

### Supporting
Nothing new. New helpers reuse the existing `Client.send_request` path via the single-page methods they delegate to.

### Alternatives Considered
None — locked by CONTEXT.md.

**Installation:**
No changes to `Cargo.toml` dependencies required. Version bump only:
- pmcp `2.5.0` → `2.6.0` (minor, additive-only public API)
- Downstream `mcp-tester`, `mcp-preview`, `cargo-pmcp`: no dep-pin bump needed unless they exercise the new APIs (they don't).

**Version verification:**
- pmcp at HEAD: `2.5.0` [VERIFIED: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml:3`]

## Architecture Patterns

### System Architecture Diagram

```
Caller
  │
  ├─► call_tool_typed(name, &T) ───► serde_json::to_value(&T) ──► call_tool(name, Value) ───┐
  │                                       │                                                  │
  │                                       └─ Err → Error::Validation                         │
  │                                                                                          │
  ├─► call_tool_typed_with_task(name, &T) ──► serde_json::to_value ──► call_tool_with_task ──┤
  │                                                                                          │
  ├─► call_tool_typed_and_poll(name, &T, max_polls) ──► serde_json::to_value ──► call_tool_and_poll ──┤
  │                                                                                          │
  ├─► get_prompt_typed(name, &T) ──► to_value → require Object                               │
  │                                   → stringify leaves → HashMap<String,String> ──► get_prompt ──┤
  │                                                                                          │
  ├─► list_all_tools() ───────┐                                                             │
  ├─► list_all_prompts() ─────┤                                                             │
  ├─► list_all_resources() ───┼─► loop { call single-page; append; break if next_cursor.is_none();
  └─► list_all_resource_templates() ──┘  abort if iteration > max_iterations }               │
                                                                                             │
                                                                                             ▼
                                                                                      Existing Client
                                                                                    (send_request pipeline)
                                                                                             │
                                                                                             ▼
                                                                                        Transport
                                                                                       (wire-level I/O)
```

**Data flow:** Caller-typed input → serde serialization (typed helpers) OR in-process cursor loop (list_all helpers) → existing untyped `Client` method → `send_request` → `Protocol` → `Transport` → server. Response path unchanged.

### Component Responsibilities

| Component | Location | Responsibility |
|-----------|----------|----------------|
| `Client::call_tool_typed` / `_with_task` / `_and_poll` | `src/client/mod.rs` (new methods in `impl<T: Transport> Client<T>`) | Serialize `&T` → `Value` via `serde_json::to_value`; map failure → `Error::validation`; delegate to existing non-typed sibling. |
| `Client::get_prompt_typed` | `src/client/mod.rs` (new method) | Serialize `&T` → `Value`; require `Value::Object`; coerce each leaf (per D-06); delegate to `get_prompt(name, HashMap<String,String>)`. |
| `Client::list_all_{tools,prompts,resources,resource_templates}` | `src/client/mod.rs` (new methods) | Read `self.options.max_iterations`; loop calling the existing single-page method, appending items from the result's `.tools` / `.prompts` / `.resources` / `.resource_templates` field; break when `next_cursor.is_none()`; error if iterations ≥ cap before cursor drains. |
| `ClientOptions` | `src/client/options.rs` **new file (recommended)** or inline | `#[non_exhaustive]` config struct carrying `max_iterations: usize`. `Default::default()` returns `max_iterations = 100`. |
| `Client::options` field | `src/client/mod.rs` (new struct field) | Owns the `ClientOptions` instance. Threaded in by whichever constructor is used. |
| `Client::new` / `Client::with_info` / `ClientBuilder::build` | `src/client/mod.rs` (existing — modified) | Each continues to initialize `options` to `ClientOptions::default()`. No signature change on `Client::new` / `Client::with_info` (both stay backward-compatible). |
| **New constructor** carrying `ClientOptions` | `src/client/mod.rs` | **Do NOT name it `with_options` — collides. See Landmines.** Recommendation: `Client::with_client_options(transport: T, options: ClientOptions)` or extend `ClientBuilder` with `.client_options(opts)`. |
| `lib.rs` re-export | `src/lib.rs:54` | Add `ClientOptions` to the `pub use client::{…}` line. |

### Recommended Project Structure

```
src/client/
├── mod.rs              # Existing — all new methods added here
├── options.rs          # NEW — ClientOptions struct (recommended placement; inline is also acceptable per D-08 discretion)
├── auth.rs             # unchanged
├── oauth.rs            # unchanged
├── http_middleware.rs  # unchanged
└── transport/          # unchanged

examples/
├── c02_client_tools.rs          # UPDATED — showcase call_tool_typed + list_all_tools
└── cNN_client_list_all.rs       # NEW — dedicated pagination demo (NN ≥ 09, see Landmines)

tests/
└── property_tests.rs   # EXTEND — delegation-equivalence + cap-enforcement properties
                        # (new file also acceptable per Claude's Discretion)

fuzz/fuzz_targets/
└── (no new fuzz target strictly required; see Validation Architecture discussion)
```

### Pattern 1: Existing single-page method skeleton (the template)

Every new single-page method (and thus every `list_all_*` inner body) already follows this template in `src/client/mod.rs`:

```rust
// Source: src/client/mod.rs:339-357 [VERIFIED]
pub async fn list_tools(&self, cursor: Option<String>) -> Result<ListToolsResult> {
    self.ensure_initialized()?;
    self.assert_capability("tools", "tools/list")?;

    let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
        cursor,
    })));
    let request_id = RequestId::String(Uuid::new_v4().to_string());
    let response = self.send_request(request_id, request).await?;

    match response.payload {
        crate::types::jsonrpc::ResponsePayload::Result(result) => {
            serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
        },
        crate::types::jsonrpc::ResponsePayload::Error(error) => {
            Err(Error::from_jsonrpc_error(error))
        },
    }
}
```

`list_all_tools` does NOT duplicate this — it calls `self.list_tools(cursor).await` in a loop.

### Pattern 2: Typed-call delegation (the recommended shape for D-01/D-13)

```rust
// Pattern for call_tool_typed — each arm delegates to its non-typed sibling
pub async fn call_tool_typed<Arg: Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &Arg,
) -> Result<CallToolResult> {
    let value = serde_json::to_value(args)
        .map_err(|e| Error::validation(format!("call_tool_typed arguments: {e}")))?;
    self.call_tool(name.into(), value).await
}
```

The `?Sized` bound lets callers pass `&SomeUnsizedType` (e.g., `&dyn Serialize`) but is optional; simpler `T: Serialize` is acceptable. Confirm clippy pedantic doesn't bikeshed here.

### Pattern 3: Auto-paginating loop (the recommended shape for D-12)

```rust
pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>> {
    let cap = self.options.max_iterations;
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for iteration in 0..cap {
        let page = self.list_tools(cursor).await?;
        out.extend(page.tools);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(out),
        }
        let _ = iteration; // iteration counter reserved for future per-page instrumentation
    }
    Err(Error::validation(format!(
        "list_all_tools exceeded max_iterations cap of {cap} pages"
    )))
}
```

Same shape for `list_all_prompts` (field `.prompts: Vec<PromptInfo>`), `list_all_resources` (field `.resources: Vec<ResourceInfo>`), `list_all_resource_templates` (field `.resource_templates: Vec<ResourceTemplate>`).

### Anti-Patterns to Avoid
- **Don't** re-implement `send_request` / `ResponsePayload` match inside `list_all_*` — always delegate to the existing single-page method so behavior stays in one place.
- **Don't** hold any lock across the await in the loop — none of the existing methods do, and the delegation pattern preserves this automatically.
- **Don't** swallow the serialize error — always map it to `Error::validation(…)` with a string that includes the serde error message (call sites want to see which field failed).
- **Don't** treat `next_cursor: Some("")` as "no more pages" — the MCP spec says only `None` terminates. Empty string is a legal (if unusual) cursor value.
- **Don't** silently truncate on cap-hit — D-10 mandates `Error::Validation`.
- **Don't** add `page_size` to `ClientOptions` — D-11.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Serializing `T: Serialize` to a JSON value | A manual match on `Value` variants | `serde_json::to_value(&T)` | Already in the workspace; handles all edge cases. |
| Cursor-loop termination | A `while let Some(_) = …` with manual state | The bounded `for` template above | The `for 0..cap` form self-documents the cap and avoids one bug class (forgotten break). |
| Error wrapping for serialize failures | New `Error::Serialization` variant | `Error::validation(msg)` | CONTEXT.md D-02 locks this. No new enum variants. |
| Mock transport for unit tests | New fixture | The `MockTransport` already inside `src/client/mod.rs` `mod tests` at line 1847 | [VERIFIED: `src/client/mod.rs:1847-1891`] already implements `Transport` with a `Vec<TransportMessage>` response queue. |
| Prompt-arg stringification helper | Custom recursive stringifier | Direct `match` on `Value` per D-06 (strings passthrough, numbers/bools via `to_string()`, arrays/objects via `serde_json::to_string(&value)`) | Simpler and explicit; avoids coercing strings into their quoted JSON form. |

**Key insight:** There is nothing complex to implement here — the phase is 100% existing-primitive composition. The only judgment calls are method naming (see Landmines) and test organization (Claude's Discretion).

## Common Pitfalls

### Pitfall 1: `Value::String(s)` must NOT go through `to_string()`
**What goes wrong:** `Value::String("hello").to_string()` returns `"\"hello\""` (with embedded quotes), not `"hello"`. A naive single-arm `value.to_string()` coercion for every entry would wrap every string arg in quotes, breaking prompt servers.
**Why it happens:** `serde_json::Value`'s `Display` impl renders as valid JSON, which means string values get re-quoted.
**How to avoid:** Match on `Value` variant explicitly per D-06 — `Value::String(s)` extracts the inner `String`; only non-string variants go through `to_string()` or `serde_json::to_string`.
**Warning signs:** Integration test against a prompt server sees arguments wrapped in quotes; prompt arg validation fails.

### Pitfall 2: `next_cursor: Some("")` vs `next_cursor: None`
**What goes wrong:** Loop terminates early if you check `.is_some_and(|s| !s.is_empty())` or similar truthy-string pattern.
**Why it happens:** Rust habits from other languages where empty string ≈ falsy.
**How to avoid:** Terminate *only* on `None`. An empty-string cursor is legal and the server may require it to be echoed back.
**Warning signs:** Paginated tests that return `next_cursor: Some("")` as a non-terminal cursor don't exercise more than one iteration.

### Pitfall 3: Method name collision on `with_options`
**What goes wrong:** The name `Client::with_options` is already defined with signature `(transport, client_info, options: ProtocolOptions) -> Self`. Adding `Client::with_options(transport, options: ClientOptions) -> Self` per D-09 literal wording would be a compile error on duplicate method name.
**Why it happens:** CONTEXT.md was written before the existing `with_options` was re-examined.
**How to avoid:** See Landmines section below. Pick a different constructor name or route configuration through `ClientBuilder`.
**Warning signs:** Planner writes D-09 verbatim → compile error at implementation time.

### Pitfall 4: Example filename collision with Phase 74
**What goes wrong:** CONTEXT.md D-14 says "add new `examples/c08_client_list_all.rs`", but Phase 74 already shipped `examples/c08_oauth_dcr.rs` on main (commit `87a1d100`).
**Why it happens:** CONTEXT.md drafted 2026-04-20 before Phase 74 landed 2026-04-21.
**How to avoid:** Use `c09_client_list_all.rs`. The c01–c08 numeric prefixes are in-order-taken in `examples/README.md`.
**Warning signs:** `git status` shows untracked `c08_client_list_all.rs` but `c08_oauth_dcr.rs` already exists.

### Pitfall 5: Typed helper default-generic bound is too loose
**What goes wrong:** Writing `pub async fn call_tool_typed<T: Serialize>(&self, name: …, args: &T)` without `?Sized` is fine for 99% of calls but rejects `args: &dyn Serialize`. Adding `?Sized` costs nothing but silently expands accept-set.
**Why it happens:** Forgetting that `Serialize` is dyn-compatible and users occasionally want to pass trait objects.
**How to avoid:** Add `T: Serialize + ?Sized` unless clippy pedantic complains; both forms work, the former is more permissive.
**Warning signs:** Rare; user feedback post-release would be the trigger.

### Pitfall 6: Clippy pedantic/nursery on `impl Into<String>` vs `String`
**What goes wrong:** CI runs clippy with pedantic + nursery lint groups (see `CLAUDE.md` release section). The new typed helpers must not introduce `clippy::impl_trait_in_params` / `clippy::needless_pass_by_value` warnings that existing similar methods avoid.
**Why it happens:** The existing client methods mix both styles (`call_tool(name: String, …)` vs examples' `name.to_string()`). Clippy warnings depend on version.
**How to avoid:** Run `make quality-gate` locally on the Plan 1 branch before PR; match whichever style the clippy version on CI accepts for neighboring code.
**Warning signs:** CI red on lint group that local `cargo clippy` missed.

## Code Examples

Verified patterns from the existing codebase:

### Existing `call_tool` — the delegate target for D-01

```rust
// Source: src/client/mod.rs:416-441 [VERIFIED 2026-04-21]
pub async fn call_tool(
    &self,
    name: String,
    arguments: serde_json::Value,
) -> Result<CallToolResult> {
    self.ensure_initialized()?;
    self.assert_capability("tools", "tools/call")?;
    let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
        name,
        arguments,
        _meta: None,
        task: None,
    })));
    let request_id = RequestId::String(Uuid::new_v4().to_string());
    let response = self.send_request(request_id, request).await?;
    match response.payload {
        crate::types::jsonrpc::ResponsePayload::Result(result) => {
            serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
        },
        crate::types::jsonrpc::ResponsePayload::Error(error) => {
            Err(Error::from_jsonrpc_error(error))
        },
    }
}
```

### Existing `get_prompt` — delegate target for D-05/D-06

```rust
// Source: src/client/mod.rs:825-849 [VERIFIED 2026-04-21]
pub async fn get_prompt(
    &self,
    name: String,
    arguments: HashMap<String, String>,
) -> Result<GetPromptResult> {
    self.ensure_initialized()?;
    self.assert_capability("prompts", "prompts/get")?;
    let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
        name,
        arguments,
        _meta: None,
    })));
    let request_id = RequestId::String(Uuid::new_v4().to_string());
    let response = self.send_request(request_id, request).await?;
    match response.payload { /* same match as above */ }
}
```

### Existing `Error::validation` constructor

```rust
// Source: src/error/mod.rs:215-217 [VERIFIED]
pub fn validation(message: impl Into<String>) -> Self {
    Self::Validation(message.into())
}
```

### Existing `MockTransport` reusable for unit tests

```rust
// Source: src/client/mod.rs:1847-1891 [VERIFIED]
#[derive(Debug)]
struct MockTransport {
    responses: Arc<Mutex<Vec<TransportMessage>>>,
    sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> { /* push */ Ok(()) }
    async fn receive(&mut self) -> Result<TransportMessage> { /* pop */ }
    async fn close(&mut self) -> Result<()> { Ok(()) }
}
```

### Current Client field layout — where `options` slots in

```rust
// Source: src/client/mod.rs:59-71 [VERIFIED]
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Arc<RwLock<Protocol>>,
    middleware_chain: Arc<RwLock<EnhancedMiddlewareChain>>,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    server_version: Option<Implementation>,
    instructions: Option<String>,
    initialized: bool,
    info: Implementation,
    notification_tx: Option<mpsc::Sender<Notification>>,
    active_requests: Arc<RwLock<HashMap<RequestId, oneshot::Sender<()>>>>,
    // NEW: options: ClientOptions,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Caller hand-serializes `Args` → `Value` with `serde_json::json!()` or `to_value` | Caller passes `&Args` directly to `call_tool_typed` | Phase 73 | Removes one line + one failure mode per call site (existing callers unchanged). |
| Caller writes manual `while let Some(cursor)` loops around `list_tools` | Caller calls `list_all_tools()` | Phase 73 | One call per full listing; safety cap prevents infinite loops on buggy servers. |
| Prompt args built as `HashMap<String, String>` by caller | Caller passes a `#[derive(Serialize)]` struct; library stringifies leaves | Phase 73 | Removes per-field `.to_string()` boilerplate; numeric / bool fields stop being string-typed in user code. |

**Deprecated/outdated:** Nothing. All existing methods stay. Proposal 2's original "(`&mut self`)" signature wording is superseded by CONTEXT.md D-01's `&self` to match the actual codebase convention.

## Assumptions Log

Every claim in this research was verified against HEAD of `main` (pmcp 2.5.0) with direct file reads. No `[ASSUMED]` claims remain. The Assumptions Log is intentionally empty.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | (none) | — | — |

**All claims verified:** no user confirmation needed.

## Open Questions

1. **What is the new constructor named, given `with_options` collision?**
   - What we know: `Client::with_options(transport: T, client_info: Implementation, options: ProtocolOptions) -> Self` already exists at `src/client/mod.rs:159-177`.
   - What's unclear: CONTEXT.md D-09 says "add `Client::with_options(transport: T, options: ClientOptions) -> Self`" — that literal name is taken.
   - Recommendation: Planner picks one of these three paths in Plan 1 and documents it:
     - **Option A (preferred):** `Client::with_client_options(transport: T, options: ClientOptions) -> Self`. Unambiguous; parallels `with_info`.
     - **Option B:** Extend `ClientBuilder` with `.client_options(opts: ClientOptions) -> Self` and plumb through `ClientBuilder::build()`. Zero new `Client::*` constructors, fully backward-compatible.
     - **Option C:** Rename the existing `with_options` to `with_protocol_options` (a breaking change for anyone calling it — check callers first; `grep -r "Client::with_options" src/ examples/ tests/` shows internal uses only, so the blast radius is small).
   - **This is not a research gap — CONTEXT.md gave us the design intent; only the literal name needs a spelling fix.** Plan 1 author picks the name at task-authoring time.

2. **Where does `ClientOptions` live (file vs inline)?**
   - What we know: D-08 leaves this to Claude's Discretion.
   - What's unclear: Which is the house style — new `options.rs` file or inline struct in `mod.rs` next to `Client`?
   - Recommendation: Prefer a new `src/client/options.rs` (mirrors the `auth.rs`/`oauth.rs`/`http_middleware.rs` sub-module layout of `src/client/`). Keep `src/client/mod.rs` focused on the `Client` struct + methods. If a future PARITY-CLIENT-02 adds `StrictMode` and output-type knobs, having its own file avoids cluttering the 2231-line `mod.rs` further.

3. **Fuzz target — required or optional?**
   - What we know: `CLAUDE.md` says "ALWAYS" requirements include fuzz.
   - What's unclear: CONTEXT.md does not explicitly call for a fuzz target. The phase's input-validation surface is `serde_json::to_value(&T)` which is already fuzz-tested upstream by `serde_json` maintainers; the cursor loop takes strings from the server which are also already covered by existing JSON fuzz targets.
   - Recommendation: Add **one small fuzz target** `fuzz_list_all_cursor_loop` that drives `list_all_tools` against a `MockTransport` seeded with adversarial cursor sequences (empty strings, very long strings, repeated cursors, cycle-back cursors). This directly exercises the cap-enforcement and termination properties introduced by this phase without redundantly fuzzing serde_json. (Alternative per Claude's Discretion: satisfy ALWAYS via property tests alone and explicitly note fuzz coverage is deferred to the in-loop property tests — but adding the target is cheap.)

4. **Does `ClientOptions` need to be `Copy`?**
   - What we know: `Debug + Clone + Default` per D-08. `#[non_exhaustive]` per D-08.
   - What's unclear: Future `StrictMode` enum may not be `Copy`; locking in `Copy` now would box the follow-on phase.
   - Recommendation: Stay with just `Debug + Clone + Default`. Do not derive `Copy`.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (workspace toolchain) | Build / clippy / test | ✓ | Latest stable (match CI `dtolnay/rust-toolchain@stable`) | — |
| `make quality-gate` | Pre-commit + pre-PR gate per CLAUDE.md | ✓ (existing target) | — | — |
| `serde_json` | Typed-arg serialization | ✓ (already workspace dep) | 1.0 + `raw_value`, `preserve_order` | — |
| `proptest` / `quickcheck` | Property tests | ✓ (existing; see `tests/property_tests.rs`) | — | — |
| `cargo-fuzz` | Fuzz target (if added per Open Question 3) | ✓ (existing `fuzz/` dir with 10+ targets) | — | — |
| `pmat quality-gate proxy` | Development-time quality enforcement per CLAUDE.md | Assumed available | — | Local `make quality-gate` is the gate CI enforces; pmat proxy is advisory. |
| Provable-contracts (`../provable-contracts/contracts/pmcp/`) | Contract-first workflow per CLAUDE.md | ✗ (sibling dir absent) | — | **No contract for the `Client` surface exists.** Ship phase 73 without contract; flag to the planner that contract-first may be a no-op for this phase. |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** provable-contracts — flagged above; this is pre-existing condition, not a phase blocker.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` workspace runner + `proptest` (existing) + `cargo-fuzz` (existing) |
| Config file | `Cargo.toml` + `tests/property_tests.rs` + `fuzz/Cargo.toml` |
| Quick run command | `cargo test --lib -p pmcp client::tests::` (existing in-module `MockTransport` tests) |
| Full suite command | `make quality-gate` (fmt + pedantic+nursery clippy + build + test + audit) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PARITY-CLIENT-01 | `call_tool_typed` serializes `&T` and delegates to `call_tool` with identical wire result | unit | `cargo test -p pmcp client::tests::test_call_tool_typed_delegation` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `call_tool_typed` serialize failure returns `Error::Validation` carrying the serde error | unit | `cargo test -p pmcp client::tests::test_call_tool_typed_serialize_error` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `call_tool_typed_with_task` delegates to `call_tool_with_task` (`ToolCallResponse` passthrough) | unit | `cargo test -p pmcp client::tests::test_call_tool_typed_with_task_delegation` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `call_tool_typed_and_poll` delegates to `call_tool_and_poll` and honors `max_polls` | unit | `cargo test -p pmcp client::tests::test_call_tool_typed_and_poll_delegation` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `get_prompt_typed` with `#[derive(Serialize)] struct` → stringified `HashMap<String,String>` matches D-06 coercion rules | unit | `cargo test -p pmcp client::tests::test_get_prompt_typed_coercion` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `get_prompt_typed` with a non-object `T` (e.g., `Vec<i32>`) returns `Error::Validation` | unit | `cargo test -p pmcp client::tests::test_get_prompt_typed_non_object_rejected` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `list_all_tools` aggregates across ≥3 paginated pages against `MockTransport` | integration (in-module) | `cargo test -p pmcp client::tests::test_list_all_tools_multi_page` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `list_all_tools` terminates on `next_cursor: None` after first page | integration | `cargo test -p pmcp client::tests::test_list_all_tools_single_page` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `list_all_tools` returns `Error::Validation` when server emits `max_iterations` pages all with `next_cursor: Some(_)` | integration | `cargo test -p pmcp client::tests::test_list_all_tools_cap_enforced` | ❌ Wave 0 |
| PARITY-CLIENT-01 | Same cap/termination/aggregation properties hold for `list_all_prompts`, `list_all_resources`, `list_all_resource_templates` | integration | `cargo test -p pmcp client::tests::test_list_all_{prompts,resources,resource_templates}_multi_page` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `ClientOptions::default().max_iterations == 100` contract | unit | `cargo test -p pmcp client::tests::test_client_options_default` | ❌ Wave 0 |
| PARITY-CLIENT-01 | `Client::new` yields a client whose `options == ClientOptions::default()` | unit | `cargo test -p pmcp client::tests::test_client_new_default_options` | ❌ Wave 0 |
| PARITY-CLIENT-01 | New constructor (name TBD per Open Q1) wires through caller-supplied `ClientOptions` | unit | `cargo test -p pmcp client::tests::test_client_with_client_options` | ❌ Wave 0 |
| PARITY-CLIENT-01 | **Property:** for any `Arg: Serialize` that serializes to a non-error `Value`, `call_tool_typed(name, &arg)` produces the same wire request as `call_tool(name, serde_json::to_value(&arg).unwrap())` | proptest | `cargo test -p pmcp --test property_tests prop_call_tool_typed_delegation` | ⚠ extend `tests/property_tests.rs` |
| PARITY-CLIENT-01 | **Property:** for any `Vec<Vec<ToolInfo>>` paginated across N pages (N ≤ cap), `list_all_tools` returns the flat concatenation in order | proptest | `cargo test -p pmcp --test property_tests prop_list_all_tools_flat_concatenation` | ⚠ extend `tests/property_tests.rs` |
| PARITY-CLIENT-01 | **Property:** `list_all_tools` cap enforcement — for any `max_iterations = k`, a server emitting `k+1` pages with `Some(_)` cursors always produces `Error::Validation` | proptest | `cargo test -p pmcp --test property_tests prop_list_all_tools_cap_enforced` | ⚠ extend `tests/property_tests.rs` |
| PARITY-CLIENT-01 | **Fuzz:** cursor-loop termination on adversarial cursor sequences (empty strings, repeated values, cycles, very long strings) | fuzz | `cargo fuzz run list_all_cursor_loop` | ❌ Wave 0 (optional per Open Q3; see Discretion) |
| PARITY-CLIENT-01 | Example compiles + runs (mocked server loopback) | smoke | `cargo check --example c02_client_tools && cargo check --example cNN_client_list_all` | ❌ Wave 0 |
| PARITY-CLIENT-01 | Public API doc examples compile | doctest | `cargo test --doc -p pmcp` (existing gate, extends automatically) | ✓ existing infra |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp client::` (≈ 5 seconds)
- **Per wave merge:** `cargo test --workspace` + `cargo test --doc -p pmcp`
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] **No new test file strictly required** — all unit + integration tests can live in the existing `src/client/mod.rs` `mod tests` block (which already hosts `MockTransport`).
- [ ] Property tests: extend `tests/property_tests.rs` (existing file) with three new `proptest!` macros. Alternatively create `tests/client_parity_properties.rs` per Claude's Discretion.
- [ ] Fuzz target: `fuzz/fuzz_targets/list_all_cursor_loop.rs` (new) — optional per Open Q3. Register in `fuzz/Cargo.toml`.
- [ ] `MockTransport` extensions: may need a helper to build paginated response sequences (`MockTransport::with_paginated_tools(vec![vec![tool_a], vec![tool_b, tool_c]])`) — small addition to the existing harness. Scope into the first pagination test task.

*(No test framework install needed — `proptest`, `cargo-fuzz`, and `tokio::test` are already wired.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase surface is purely client-side helpers; auth unchanged. |
| V3 Session Management | no | Same. |
| V4 Access Control | no | Same. |
| V5 Input Validation | **yes** | `serde_json::to_value(&T)` validates caller-provided types at the type-system level; runtime `Value::Object` check gates prompt args per D-06. |
| V6 Cryptography | no | No crypto in this phase. |

### Known Threat Patterns for pmcp Client

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Caller injects untrusted type that serializes to a very large `Value` | DoS (via memory) | Unchanged from existing `call_tool(Value)`; `serde_json::to_value` will OOM on adversarial input the same way the existing pre-serialized path does. Document as "do not call with untrusted `&T`" — no new attack surface introduced. |
| Malicious server emits an infinite paginated cursor chain | DoS (via CPU / memory) | **Mitigated by D-10 `max_iterations` cap.** Phase 73's explicit cap-enforcement property test (prop_list_all_tools_cap_enforced) directly covers this. |
| Malicious server emits a cursor that causes a back-pointer cycle (A → B → A) | DoS | Mitigated by `max_iterations` — the cap terminates regardless of cursor semantics. Cycle-detection is *not* required; bounded-iteration is sufficient and matches MCP spec (which says nothing about cursor uniqueness). |
| Prompt arg stringification of nested `Object`/`Array` leaks structure to the wire | Information disclosure | Low risk — caller controls the input. Document D-06 stringification rule in the rustdoc so callers know nested leaves are JSON-stringified. |

Phase scope is narrow: client-side type system + pagination. The only security-relevant addition is the `max_iterations` cap itself, which is positive (closes a pre-existing minor DoS vector).

## Landmines

Anything in the current `Client` that could make delegation tricky:

1. **`with_options` name collision — HIGH impact on Plan 1.**
   - Existing: `Client::with_options(transport, client_info: Implementation, options: ProtocolOptions)` at `src/client/mod.rs:159-177`. [VERIFIED]
   - Proposed: `Client::with_options(transport, options: ClientOptions)` per D-09. **Collides.**
   - Resolution: See Open Question 1. Planner picks: `Client::with_client_options` (preferred), route through `ClientBuilder.client_options()`, or rename-existing-with-deprecation.

2. **Example filename `c08_client_list_all.rs` — MEDIUM impact on Plan 3.**
   - Existing: `examples/c08_oauth_dcr.rs` (shipped in Phase 74, commit `87a1d100`, 2026-04-21). [VERIFIED]
   - Proposed: `examples/c08_client_list_all.rs` per D-14.
   - Resolution: Use `c09_client_list_all.rs` and register under "Tools, Resources, Prompts" section in `examples/README.md` (next to c02/c03/c04 entries).

3. **`&self` everywhere — LOW impact, actually a positive.**
   - Existing: All `Client::call_*` / `list_*` / `get_*` methods take `&self`. `Arc<RwLock<Transport>>` interior mutability absorbs the await-across-lock concern inside `send_request`. [VERIFIED]
   - Impact: The new helpers just use `&self`. No borrow conflicts, no re-entrancy issues in the `list_all_*` loops — each iteration calls the single-page method on `&self` cleanly.

4. **`ClientBuilder::build()` already calls `Client::with_options` — LOW impact.**
   - Existing: `ClientBuilder::build()` at `src/client/mod.rs:1803-1812` calls `Client::with_options(self.transport, default-impl, self.options)`. [VERIFIED]
   - Impact: If Plan 1 renames `Client::with_options` (Option C above), `ClientBuilder::build()` must be updated in the same commit. Internal call; not a breaking change.

5. **`lib.rs` re-export list — LOW impact.**
   - Existing: `src/lib.rs:54` — `pub use client::{Client, ClientBuilder, ToolCallResponse};` [VERIFIED]
   - Impact: Add `ClientOptions` to this line so callers can `use pmcp::ClientOptions;`.

6. **Proposal 2 literal text says `(&mut self)` — HAS BEEN superseded by D-01.**
   - 69-PROPOSALS.md line 83 says `&mut self`. CONTEXT.md D-01 overrides to `&self` matching codebase convention. [CITED: `.planning/phases/69-…/69-PROPOSALS.md:83` vs CONTEXT.md D-01]
   - Impact: Planner should ignore the `&mut self` wording in the upstream proposal and follow D-01 strictly.

7. **`#[non_exhaustive]` + struct-literal construction from outside the crate — MEDIUM impact on doctests/examples.**
   - From outside the `pmcp` crate, `ClientOptions { max_iterations: 50 }` is a compile error. Callers must do `ClientOptions { max_iterations: 50, ..Default::default() }`.
   - Impact: Every rustdoc example AND `cNN_client_list_all.rs` AND tests-in-examples must use the `..Default::default()` idiom OR add a builder method `ClientOptions::default().with_max_iterations(50)`. The latter is an optional DX add; the former is mandatory spelling.

## Breaking-Change Check

Confirm `ClientOptions` as a new struct + new constructor is genuinely additive:

| Addition | Breaking? | Why |
|----------|-----------|-----|
| `ClientOptions` struct (new `#[non_exhaustive]`) | **No** | Pure new type. Nothing external depended on its name. |
| `ClientOptions` re-export in `lib.rs` | **No** | Pure new re-export. |
| `Client::call_tool_typed` / `_with_task` / `_and_poll` / `get_prompt_typed` | **No** | Pure new methods on `impl<T: Transport> Client<T>`. Does not alter existing method signatures or visibility. Adding methods is always additive in Rust. |
| `Client::list_all_tools` / `_prompts` / `_resources` / `_resource_templates` | **No** | Same — pure new methods. |
| New constructor `Client::with_client_options` (or equivalent) | **No** (preferred) | Pure new method; no collision. |
| Renaming existing `Client::with_options` → `with_protocol_options` (Option C) | **YES — breaking for downstream callers** | If any external code calls `Client::with_options(transport, info, proto_opts)` directly, this rename breaks it. **Verified internally-only in this repo via `grep "Client::with_options"` — no hits outside `src/`, `examples/`, `tests/` (i.e., no downstream consumer known to use it) — but it is still a public-API rename.** |
| `impl` changes to `Client::new` / `Client::with_info` / `ClientBuilder::build` to initialize `options` field | **No** | Internal impl changes; signatures preserved. |
| New `options: ClientOptions` field on `Client<T>` | **No** (struct is not publicly constructible via literal; no `pub` fields today) | `Client` is constructed only via `new` / `with_info` / `with_options` / `ClientBuilder::build`. Adding an internal field does not break anything. |
| REQUIREMENTS.md §55 doc-fix (`call_prompt_typed` → `get_prompt_typed`) | **No** | Documentation change. |

**Recommendation:** Avoid Option C (rename existing `with_options`). Options A and B are fully additive. If planner chooses Option A, we can issue pmcp 2.6.0 (minor bump) with confidence. If Option C, we must issue 3.0.0 (major) — **not recommended** for this phase scope.

## Sources

### Primary (HIGH confidence — all HEAD-of-main file reads, pmcp 2.5.0)
- `src/client/mod.rs` — full read of method signatures, field layout, `MockTransport` helper, existing `with_options`/`ClientBuilder::build` collision points
- `src/error/mod.rs` — `Error` enum variants + `validation()` / `parse()` constructors
- `src/types/tools.rs`, `src/types/prompts.rs`, `src/types/resources.rs` — all four list request/result types; `Cursor = Option<String>` confirmed
- `src/types/protocol/mod.rs:292-293` — `pub type Cursor = Option<String>`
- `src/lib.rs:54` — existing `pub use client::{Client, ClientBuilder, ToolCallResponse}` re-export line
- `Cargo.toml:3` — pmcp version `2.5.0`
- `examples/c02_client_tools.rs` — current manual `json!({...})` pattern (update target)
- `examples/c04_client_prompts.rs` — current `HashMap<String, String>` pattern (showcase target for `get_prompt_typed` rustdoc)
- `examples/README.md` — c-series index (confirms c01-c08 taken; next slot is c09)
- `.planning/REQUIREMENTS.md:52-55, 164-170` — PARITY-CLIENT-01 row + status table
- `.planning/STATE.md:29-30, 85-89` — Phase 73 / 74 ordering swap + summary
- `.planning/ROADMAP.md:992-997` — Phase 73 entry
- `.planning/phases/69-.../69-PROPOSALS.md` §"Proposal 2" — original in-scope / out-of-scope / success criteria
- `.planning/phases/73-…/73-CONTEXT.md` — locked decisions D-01 through D-15

### Secondary (MEDIUM confidence)
- None relied upon — all claims anchor to primary HEAD-of-main sources above.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new deps; everything already in workspace and verified against `Cargo.toml` + source.
- Architecture: HIGH — delegation pattern is forced by the shape of existing methods, all read and confirmed.
- Pitfalls: HIGH — all six pitfalls anchor to code constructs verified in-tree (`with_options` collision, `c08_*` collision, `Value::String` display quoting, `next_cursor: Some("")` semantics, `#[non_exhaustive]` literal-construction rules).
- Validation Architecture: HIGH — existing `MockTransport`, `proptest` harness, and `cargo-fuzz` infra all present and re-usable.
- Security: HIGH — `max_iterations` cap is the only new security-relevant surface; property test covers it.

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — pmcp `Client` surface is stable; only risk is rebase conflict if another phase touches `src/client/mod.rs` in the meantime)
