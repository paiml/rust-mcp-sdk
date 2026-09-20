# Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01) - Context

**Gathered:** 2026-04-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship **additive, non-breaking** `Client` ergonomics that close the client-side DX gap surfaced by the rmcp parity research (CLIENT-02 / 69-PROPOSALS.md Proposal 2):

1. **Typed call helpers** — `call_tool_typed<T: Serialize>`, `call_tool_typed_with_task<T: Serialize>`, `call_tool_typed_and_poll<T: Serialize>`, and `get_prompt_typed<T: Serialize>` that internally serialize `T` and delegate to the existing Value-based methods.
2. **Auto-paginating list helpers** — `list_all_tools`, `list_all_prompts`, `list_all_resources`, `list_all_resource_templates` that internally loop on `next_cursor` with a safety cap.
3. **`ClientOptions` config struct** — new type holding `max_iterations` (default `100`) wired in via a new `Client::with_options` constructor. No `page_size` knob (MCP spec has no request-side limit field).

The existing `call_tool` / `call_tool_with_task` / `get_prompt` / `list_tools` / `list_prompts` / `list_resources` / `list_resource_templates` methods stay exactly as they are — low-level cursor access is preserved. Minor semver bump.

Out of scope (belong to other phases):
- `ClientNotificationHandler` trait (CLIENT-03)
- client-side `ProgressDispatcher` (CLIENT-04)
- Tower / service integration (already shipped in Phase 56)
- Transport construction API reshape

</domain>

<decisions>
## Implementation Decisions

### Typed Call API Shape
- **D-01:** `call_tool_typed<T: Serialize>(&self, name: impl Into<String>, args: &T) -> Result<CallToolResult>` — args are **borrowed** (`&T`). `serde_json::to_value` takes `&T` internally; non-consuming is idiomatic for serialization helpers and keeps `T: Serialize` as the only bound.
- **D-02:** Serialize failure → `Error::Validation` with a message naming the offending field path if serde surfaces one. Rationale: treated as client-side input-shape validation, consistent with other pre-send checks.
- **D-03:** Method names exactly as stated in REQUIREMENTS.md / 69-PROPOSALS.md Scope: `call_tool_typed`, `call_tool_typed_with_task`. The `_typed` suffix is a clear DX marker and matches traceability artifacts verbatim.
- **D-04:** Integration tests **reuse the existing in-process test server harness** used by current `client::*` integration tests — add typed round-trip cases there rather than building a new fixture.

### Typed Prompt Helper
- **D-05:** Name is `get_prompt_typed` (matches existing `get_prompt` + MCP method `prompts/get`). REQUIREMENTS.md row 55 currently says `call_prompt_typed` — this is a wording mismatch; fix the REQUIREMENTS.md row as a doc correction during this phase (see "Requirements doc-fix" below).
- **D-06:** Coercion from `T: Serialize` to `HashMap<String, String>`:
  1. Call `serde_json::to_value(args)`.
  2. Require result is `Value::Object` — non-object → `Error::Validation("prompts/get arguments must serialize to a JSON object")`.
  3. For each `(key, value)` entry in the object:
     - `Value::String(s)` → pass through
     - `Value::Number` / `Value::Bool` → `value.to_string()`
     - `Value::Null` → skip entry (treat as omitted)
     - `Value::Array` / `Value::Object` → `serde_json::to_string(&value)?` (nested values JSON-stringified; rare for prompt args but unambiguous behavior)
- **D-07:** Doctest demonstrates a `#[derive(Serialize)] struct SummaryArgs { topic: String, length: u32 }` to showcase non-string leaf stringification — the headline DX win over raw `HashMap<String, String>`.

### Pagination Config Surface
- **D-08:** Add `ClientOptions` struct — new file `src/client/options.rs` (or inline if planner prefers; see Claude's Discretion). Fields for this phase: `max_iterations: usize` (default `100`). Derives: `Debug`, `Clone`, `Default`. **Mark the struct `#[non_exhaustive]`** so the PARITY-CLIENT-02 follow-on (see Deferred Ideas — Strict / Trust client modes) can add a `StrictMode` variant and typed-output knobs without a breaking change. Construct via `ClientOptions::default()` + field-update idiom since `#[non_exhaustive]` forbids struct-literal construction from external crates.
- **D-09:** Client construction: add `Client::with_options(transport: T, options: ClientOptions) -> Self`. Existing `Client::new` delegates to `Client::with_options(t, ClientOptions::default())` so no call sites break.
- **D-10:** On `max_iterations` exceeded before server reports `next_cursor: None` → return `Error::Validation` with message naming the cap (e.g., `"list_all_tools exceeded max_iterations cap of 100 pages"`). Matches Proposal 2 Success Criteria bullet 3 verbatim. No silent partial-return.
- **D-11:** **No `page_size` field** on `ClientOptions` and no per-request `limit`: MCP `ListToolsRequest` / `ListPromptsRequest` / `ListResourcesRequest` / `ListResourceTemplatesRequest` only carry `cursor`. Server dictates page size. list_all_* honors whatever `next_cursor` the server emits.

### Method Coverage
- **D-12:** Auto-paginate family = **four** methods, not three:
  - `list_all_tools() -> Result<Vec<ToolInfo>>`
  - `list_all_prompts() -> Result<Vec<PromptInfo>>`
  - `list_all_resources() -> Result<Vec<ResourceInfo>>`
  - `list_all_resource_templates() -> Result<Vec<ResourceTemplate>>` *(extension beyond Proposal 2 for symmetry with existing `list_resource_templates` at `src/client/mod.rs:948`)*
- **D-13:** Typed task-aware parity = **both** `call_tool_typed_with_task` AND `call_tool_typed_and_poll` — full parity with the existing non-typed task trio (`call_tool` / `call_tool_with_task` / `call_tool_and_poll`). Each delegates to its non-typed sibling after serialization.
- **D-14:** Examples exactly as Proposal 2 scope: update `examples/c02_client_tools.rs` to showcase typed call_tool + list_all_tools; add new `examples/c08_client_list_all.rs` as the dedicated pagination demo. Both registered in `examples/README.md`.

### Requirements Doc-Fix
- **D-15:** During this phase, update `.planning/REQUIREMENTS.md:55` to change `call_prompt_typed` → `get_prompt_typed` in the PARITY-CLIENT-01 row. This is a one-line doc correction that aligns the requirement text with the actual MCP method name (`prompts/get`) and the existing `get_prompt` method it parallels. Commit as part of the documentation plan.

### Claude's Discretion
- Exact error message wording for `Error::Validation` cases (as long as the cap number and method name are present).
- Whether `src/client/options.rs` is a new module file or `ClientOptions` lives inline in `src/client/mod.rs` — planner decides based on file size / module hygiene.
- Whether `list_all_*` take no args or accept an optional `max_iterations` override per call (default: no per-call override — read from `self.options`; revisit if fuzz tests show need).
- Exact rustdoc structure per method as long as each carries a working `rust,no_run` doctest per Phase 66 macro-doc convention.
- Whether property/fuzz tests live under `tests/property_tests.rs` (existing) or a new dedicated file.

### Folded Todos
None — no matching pending todos surfaced for Phase 73 scope.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements & Proposal Source
- `.planning/REQUIREMENTS.md` §55 — PARITY-CLIENT-01 checkbox row (pending the D-15 `get_prompt_typed` rename)
- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md` §"Proposal 2" — authoritative Goal / In-scope / Out-of-scope / Success Criteria / Rationale for PARITY-CLIENT-01
- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-RESEARCH.md` — CLIENT-02 gap analysis (pmcp-vs-rmcp evidence rows for typed call and list_all)

### Dependency Phase
- `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RECOMMENDATION.md` — Phase 72 concluded pmcp stays on its own foundations (recommendation D); PARITY-CLIENT-01 is the client-ergonomics follow-up

### Codebase Surfaces to Extend
- `src/client/mod.rs:339-357` — existing `list_tools(cursor: Option<String>)` — loop template for `list_all_tools`
- `src/client/mod.rs:416-441` — existing `call_tool(name: String, arguments: Value)` — delegate target for `call_tool_typed`
- `src/client/mod.rs:463-498` — existing `call_tool_with_task` — delegate target for `call_tool_typed_with_task`
- `src/client/mod.rs:620-...` — existing `call_tool_and_poll` — delegate target for `call_tool_typed_and_poll`
- `src/client/mod.rs:749-...` — existing `list_prompts(cursor)` — loop template for `list_all_prompts`
- `src/client/mod.rs:825-854` — existing `get_prompt(name, HashMap<String,String>)` — delegate target for `get_prompt_typed`
- `src/client/mod.rs:891-...` — existing `list_resources(cursor)` — loop template for `list_all_resources`
- `src/client/mod.rs:948-...` — existing `list_resource_templates(cursor)` — loop template for `list_all_resource_templates`
- `src/error.rs` — `Error::Validation` variant (target for D-02 serialize failures and D-10 cap-hit)
- `examples/c02_client_tools.rs:35` — current manual-serialization pattern being superseded
- `examples/README.md` — examples index; needs update to register c08

### Doc / Convention Refs
- `.planning/phases/66-macros-documentation-rewrite/` — doctest convention for new `Client` methods (every public method carries a `rust,no_run` doctest)
- `CLAUDE.md` — ALWAYS requirements: fuzz + property + unit + `cargo run --example` demonstration

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`Client::send_request`** — low-level request dispatch used by every existing `call_*` / `list_*`. Typed helpers delegate to existing non-typed methods, which already call this. No new plumbing.
- **`Error::Validation`** — existing error variant covers both serialize-failure (D-02) and cap-hit (D-10); no new Error enum variants needed.
- **`serde_json::to_value` / `serde_json::to_string`** — already a workspace dep. Powers D-01 and D-06 leaf coercion.
- **`CallToolResult` / `GetPromptResult` / `ListToolsResult` / `ListPromptsResult` / `ListResourcesResult` / `ListResourceTemplatesResult`** types — returned as-is; typed helpers don't change result shapes.
- **In-process test server harness** — existing client integration tests already spin this up (see `tests/` dir); reuse for typed round-trips and paginated-server fixtures (D-04).

### Established Patterns
- **Request → Result → match on `ResponsePayload::{Result,Error}`** — every existing client method follows this. New list_all loops will call the existing single-page methods and accumulate, so they don't re-implement this pattern at the wire level.
- **`impl Into<String>` for names** — some of the codebase uses this, others use `String`. D-01 picks `impl Into<String>` for the name arg to match Proposal 2 Scope wording and reduce `.to_string()` at call sites.
- **Per-method rustdoc with runnable doctest** — Phase 66 convention; all 4 typed + 4 list_all + 1 get_prompt_typed methods must carry one.

### Integration Points
- New `ClientOptions` type needs to thread through `Client` struct — add `options: ClientOptions` field (see Claude's Discretion on module placement).
- `examples/README.md` index has to register `c08_client_list_all` alongside the existing c01-c07 entries.
- `pmcp` version bump: minor (additive capability). `mcp-tester` / `cargo-pmcp` consumers only need a dep pin bump if they exercise the new APIs — likely not for this phase.

</code_context>

<specifics>
## Specific Ideas

- The user explicitly chose the pmcp-native naming (`get_prompt_typed`) over the REQUIREMENTS.md-mirrored naming (`call_prompt_typed`) even though it means updating REQUIREMENTS.md. This matches the project's "pmcp-native framing" rule in REQUIREMENTS.md that forbids "adopt rmcp's" phrasing — local method naming should be internally consistent (`get_` matches `get_prompt`), not imported verbatim from a research doc.
- Full task-aware typed parity (both `_with_task` AND `_and_poll` typed variants) was preferred over minimum-viable — the user wants the typed surface to feel first-class, not a stopgap.
- `list_all_resource_templates` was explicitly requested beyond the 3-method proposal scope. Treat as a small expansion, not scope creep — it's the same pagination loop applied to an existing single-page method.

</specifics>

<deferred>
## Deferred Ideas

- **Additional `ClientOptions` tunables** — request timeout, retry policy, default headers, etc. Out of scope for this phase; revisit when a concrete need surfaces.
- **`c09_client_prompts_typed` dedicated example** — user chose proposal-exact example scope (c02 update + c08 new only). Prompt-typed DX is demonstrated via the rustdoc doctest on `get_prompt_typed`.
- **Typed result + Strict / Trust client modes (candidate PARITY-CLIENT-02 follow-on)** — symmetric output-side typing and a client-wide validation policy. Three behaviors to cover in the follow-on:
  1. **Strict** — validate responses against each tool/prompt/resource's declared schema; reject on mismatch with `Error::Validation`. For trusted internal servers where schema drift = bug.
  2. **Trust** — best-effort `serde_json::from_value::<R>(…)` when caller requests a typed result; fall back to raw `CallToolResult` on failure. For external/third-party servers where schemas lag implementations.
  3. **Off (current)** — hand back raw `CallToolResult`; caller decides.
  Lands a `StrictMode` enum on the existing Phase 73 `ClientOptions` plus typed-output variants like `call_tool_typed_for::<Args, Result>` and `get_prompt_typed_for::<Args, Result>`. **Kept out of Phase 73 on purpose** — doubles plan count and brings a non-trivial schema-validation test matrix. Mark `ClientOptions` as `#[non_exhaustive]` in Phase 73 so the follow-on adds variants non-breaking.
- **Per-call `max_iterations` override** in list_all_* method signatures — not added this phase; callers get the `ClientOptions` value from the Client. Revisit only if fuzz/property tests show need.
- **`ClientNotificationHandler` trait (CLIENT-03)** — explicit Medium-severity future work per 69-PROPOSALS.md D-16.
- **Client-side `ProgressDispatcher` (CLIENT-04)** — explicit Medium-severity future work per 69-PROPOSALS.md D-16.

### Reviewed Todos (not folded)
None — no pending todos were reviewed.

</deferred>

---

*Phase: 73-typed-client-helpers-list-all-pagination-parity-client-01*
*Context gathered: 2026-04-20*
