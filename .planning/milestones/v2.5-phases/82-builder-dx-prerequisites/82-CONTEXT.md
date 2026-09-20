# Phase 82: Builder DX Prerequisites - Context

**Gathered:** 2026-05-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Lift handler-registration parity to PMCP's public surface so external toolkit authors (Phase 83's `pmcp-server-toolkit` and downstream) can:

1. Share a single `Arc<dyn Handler>` between `pmcp::ServerBuilder` and their own in-process state without writing a 20-line delegating wrapper shim per handler type (spike 004 §"API surface discovery 1").
2. Drive a built `pmcp::Server` end-to-end in integration tests through a publicly documented, supported pattern — without poking at the crate-private `Server::handle_request` (spike 004 §"API surface discovery 3").

This phase is **pure upstream DX plumbing**. It ships no new functionality for end users, it changes no existing public API signatures, and it is unconditionally additive. It unblocks every subsequent v2.2 phase (83–89) that consumes `tool_arc` / `prompt_arc` or needs a documented test pattern for config-driven handlers.

**Out of scope:** any new test-driver runtime, any `Server::handle_request` exposure, any signature change to existing `tool()` / `prompt()` / `resources()` / `sampling()` / `auth_provider()` / `tool_authorizer()` methods, any toolkit code (Phase 83), any deferred Skills `extensions`-field cleanup work (already-shipped Phase 80 scope).

</domain>

<decisions>
## Implementation Decisions

### BLDR-03 testing surface

- **D-01: Handler-level testing pattern, not an in-process driver.** External toolkit integration tests will drive a built `pmcp::Server` by retrieving registered handlers via `Server::get_tool(name)` / `Server::get_prompt(name)` and calling `handler.handle(args, RequestHandlerExtra::default()).await`. **No new public dispatch surface** (no `Server::dispatch`, no paired `MemoryTransport`). This is the same pattern `tests/skills_integration.rs:146–164` already uses for prompts; this phase formalizes it as the supported testing contract and extends it to tools by adding the missing `get_tool` accessor (folded into BLDR-03 — see D-08).
- **D-02: Documented in three places** — (a) comprehensive doctest on `Server::get_tool` and `Server::get_prompt` showing the pattern end-to-end, (b) a numbered `tests/in_process_handler_pattern.rs` reference test that exercises the pattern against a real built `pmcp::Server` (regression anchor — if the pattern breaks, this test fails), (c) a short section in `pmcp-book` under the testing chapter giving narrative context and copy-paste-ready examples.
- **D-03: Explicit "what this skips" callout.** Both the doctest and the book section state plainly: "this pattern exercises handler logic only — `auth_provider`, `tool_authorizer`, and `tool_middleware` run in the JSONRPC dispatch path (`Server::handle_request`) and are NOT invoked. For full-pipeline tests, drive a real transport (stdio / streamable-http) with a `pmcp::Client`." This pre-empts the future support ticket where a toolkit author writes a test expecting the security pipeline to run and is surprised it doesn't.

### tool_arc / prompt_arc lift implementation

- **D-04: Pure additive copy from `ServerCoreBuilder` — no refactor of existing `tool()`/`prompt()` methods.** The new `_arc` methods are copied verbatim from `src/server/builder.rs:203` (`tool_arc`) and `:254` (`prompt_arc`) into the public `ServerBuilder` in `src/server/mod.rs:1741`. Existing `tool(impl ToolHandler + 'static)` and `prompt(...)` signatures are untouched. **Rejected: refactoring `tool()` to delegate through `tool_arc(Arc::new(handler))`** — `tool()` is in nearly every example/test/doc in the SDK, and the v2.2 release should not start by touching a load-bearing method when an additive lift achieves the same goal at zero blast radius.
- **D-05: Lift all six `_arc` methods at once**, not just the two named in BLDR-01/-02. `ServerCoreBuilder` exposes six `_arc` variants — `tool_arc` (`:203`), `prompt_arc` (`:254`), `resources_arc` (`:294`), `sampling_arc` (`:420`), `auth_provider_arc` (`:442`), `tool_authorizer_arc` (`:458`). All four extras are missing from the public `ServerBuilder` but are mechanically identical lifts. Closing the entire Arc-symmetry gap in Phase 82 prevents Phase 83/84 from re-tripping the same shim problem against `auth_provider` or `resources`. Each lift is ~10 lines, pure copy, same blast radius (additive only).
- **D-06: The four extra lifts get tracked as a single new umbrella requirement BLDR-04** in REQUIREMENTS.md: *"`pmcp::ServerBuilder` gains `_arc` variants for the remaining four handler types (`resources_arc`, `sampling_arc`, `auth_provider_arc`, `tool_authorizer_arc`) so all impl-or-Arc handler-registration paths reach parity with `ServerCoreBuilder`."* Phase 82's requirement count bumps from 3 to 4. The ROADMAP.md success-criteria block gains one bullet covering the four extras' parity claim.

### Server::get_tool accessor parity

- **D-07: Add `pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn ToolHandler>>` to `pmcp::Server`** — perfect symmetry with the existing `pub fn get_prompt(&self, name: &str) -> Option<&Arc<dyn PromptHandler>>` at `src/server/mod.rs:385`. Returns a borrow of the `Arc`; callers `Arc::clone(...)` if they need owned. **Rejected: adding broader introspection** (`list_tools`, `list_prompts`, `get_sampling_handler`, etc.) — that's a separate "server introspection API" design space that opens its own decisions; out of scope for Phase 82.
- **D-08: `get_tool` is folded into BLDR-03's wording**, not a separate requirement number. BLDR-03 already says *"officially documented handler-level testing pattern"* — `get_tool` is the API surface that pattern requires to function for tools (it already works for prompts via `get_prompt`). BLDR-03 will be re-worded in REQUIREMENTS.md to explicitly mention `Server::get_tool(name) accessor symmetric with get_prompt` so the requirement is testable as written.

### Claude's Discretion

- Exact wording of the doctest example body — should illustrate (a) `Server::builder().tool_arc(name, Arc::new(handler)).build()`, (b) `server.get_tool(name).expect(...).handle(args, RequestHandlerExtra::default()).await`, (c) assertion on the result.
- Exact naming/structure of `tests/in_process_handler_pattern.rs` — researcher / planner picks based on existing tests/ naming conventions.
- Whether to add a property test asserting that for any `T: ToolHandler`, `builder.tool("x", T)` and `builder.tool_arc("x", Arc::new(T))` produce structurally identical post-build state (same `tool_infos` entry, same registered `Arc` resolves the same `handle(...)` call). Recommended yes — small additional test, catches future drift between the two registration paths. Final call to planner.
- MSRV / clippy implications of the lift — none expected (mechanical copy of code that already compiles in the same workspace), but planner should validate during plan-checker.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 82 source-of-truth specs
- `.planning/ROADMAP.md` §"Phase 82: Builder DX Prerequisites" (lines 1238–1247) — phase goal, success criteria, depends-on (Phase 81 / v2.1 close), additive minor-version-bump constraint. **NOTE for the planner:** the success-criteria bullets need extending to cover D-05 (all six `_arc` lifts) and D-07 (`get_tool` accessor). Add bullets 5 and 6.
- `.planning/REQUIREMENTS.md` lines 116–122 (BLDR-01, BLDR-02, BLDR-03) — current requirement text. **NOTE for the planner:** (a) BLDR-03 wording needs updating to explicitly mention `Server::get_tool(name) accessor symmetric with get_prompt`, (b) a new BLDR-04 umbrella requirement needs to be added covering `resources_arc / sampling_arc / auth_provider_arc / tool_authorizer_arc` lifts, (c) the traceability table at line 339+ needs a row for BLDR-04.

### Spike findings — these are the load-bearing design rationale
- `.planning/spikes/004-schema-server-thin-slice-sql/README.md` — §"API surface discovery 1: two `ServerBuilder`s" (lines 115–136) is the original problem statement for the `tool_arc`/`prompt_arc` gap; §"API surface discovery 3: `Server::handle_request` is private" (lines 148–158) is the original problem statement for the BLDR-03 testing-surface gap; §"Refined SDK lift shape" item 3 (lines 224–228) and §"Surprises" items 1–2 (lines 238–246) name the fixes Phase 82 ships.
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — auto-loaded blueprint; §requirements line 111–114 confirms `tool_arc` + `prompt_arc` as non-negotiable upstream requirements; §"Implementation Order" item 7 confirms "upstream DX gaps first" as the Phase 82 placement.

### Existing code Phase 82 modifies
- `src/server/builder.rs` (the **private** `ServerCoreBuilder`) — donor for all six `_arc` method bodies. Lines 203–222 (`tool_arc`), 251–272 (`prompt_arc`), 290+ (`resources_arc`), 420+ (`sampling_arc`), 442+ (`auth_provider_arc`), 458+ (`tool_authorizer_arc`).
- `src/server/mod.rs` (the **public** `ServerBuilder`) — recipient. Line 385 hosts the existing `get_prompt` to mirror at the same accessor block. Line 1741 starts the public `ServerBuilder` struct definition; the six new `_arc` methods land in its `impl` block (line 1816+) immediately after the corresponding non-Arc method (so `tool_arc` follows `tool`, `prompt_arc` follows `prompt`, etc., to keep rustdoc grouping clean).
- `tests/skills_integration.rs` lines 142–164 — the existing reference for the handler-level testing pattern (uses `get_prompt`); `tests/in_process_handler_pattern.rs` (new in this phase) extends the same shape to `get_tool` and is the regression anchor named in D-02.

### Conventions and quality contracts
- `CLAUDE.md` §"ALWAYS Requirements for New Features (MANDATORY)" — every feature MUST include unit tests, property tests, fuzz tests (where applicable), integration tests, doctests, and an example. For Phase 82's pure-DX surface, "example" is satisfied by the doctest + the `tests/in_process_handler_pattern.rs` reference; planner should not invent a numbered `examples/sNN_*.rs` if it would be redundant — confirm with reference to `spike-findings-rust-mcp-sdk` SKILL.md §"Implementation Order" item 5 which calls out where examples *do* belong.
- `CLAUDE.md` §"Quality Gate Enforcement" — every commit must pass `make quality-gate` (fmt + clippy + build + tests). PMAT cognitive-complexity gate (≤25 per function) is CI-enforced — none of the six lifted methods exceed it (each is ~10 lines).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`ServerCoreBuilder`'s six `_arc` methods** (`src/server/builder.rs:203 / :254 / :294 / :420 / :442 / :458`) — donor code. Each body handles cache-at-registration of `tool_infos` / `prompt_infos`, capability-flag toggling (e.g., `self.capabilities.tools = Some(ToolCapabilities { list_changed: Some(false) })`), and same-shape `HashMap::insert`. The lift literally copies these bodies into the public `ServerBuilder` impl block.
- **`Server::get_prompt(&self, name: &str) -> Option<&Arc<dyn PromptHandler>>`** at `src/server/mod.rs:385` — the exact signature template for the new `get_tool`. Same module, same `impl Server` block.
- **`RequestHandlerExtra::default()`** at `src/server/cancellation.rs:421` — already public; testing pattern callers use this for the second argument to `handler.handle(args, extra).await`. No API gap for handler-level testing.
- **`tests/skills_integration.rs:146–164` (`wire_level_prompt_text`)** — the working reference pattern. `Server::builder().bootstrap_skill_and_prompt(...).build()?` → `server.get_prompt(name).expect(...)` → `prompt.handle(HashMap::new(), RequestHandlerExtra::default()).await.unwrap()`. The new `tests/in_process_handler_pattern.rs` will follow the same skeleton, extended to `get_tool`.

### Established Patterns
- **Two-builder pattern in `src/server/`** — `ServerCoreBuilder` (private, in `builder.rs`) builds a `ServerCore`; the public `ServerBuilder` (in `mod.rs`) wraps that and adds public-API conveniences. The two have parallel but not identical method sets; Phase 82 closes the Arc-handler half of that parallelism gap.
- **Cache `ToolInfo` / `PromptInfo` at registration, not per-request** (`tool_infos` / `prompt_infos` HashMaps on the builder) — both `tool()`/`prompt()` and `tool_arc`/`prompt_arc` in `ServerCoreBuilder` do this. The lifted methods MUST preserve this behavior so registered handlers' metadata stays consistent regardless of which registration path was used.
- **Capabilities auto-enable on first registration** — e.g., the first `tool*()` call sets `capabilities.tools = Some(ToolCapabilities { list_changed: Some(false) })`. Lifted `_arc` methods must do this too (the `ServerCoreBuilder` bodies already include it; the copy preserves it).
- **`#[non_exhaustive]` on protocol types** (per `.planning/spikes/004-schema-server-thin-slice-sql/README.md` §"API surface discovery 2") — Phase 82 doesn't construct any `#[non_exhaustive]` types directly, but the doctest example body for `get_tool` should use `CallToolResult::new(...)` and friends per Phase 54.1's resolved constructor pattern (per `feedback_protocol_type_dx` memory).

### Integration Points
- **Phase 83 (`pmcp-server-toolkit`)** is the first consumer — its `SqlToolHandler<C>` and `StaticPromptHandler` will register via `tool_arc(name, Arc::new(handler))` and skip the 20-line wrapper shim that spike 004 had to write.
- **Phase 84 (SQL connectors)** uses the same lift via Phase 83's toolkit — no direct dependency on Phase 82 changes, but transitively requires them.
- **Phase 87 (`pmcp-config-helper`)** uses `bootstrap_skill_and_prompt` (already shipped in Phase 80) on top of the public builder; doesn't directly need the new `_arc` methods, but does benefit from the documented handler-level testing pattern when validating its byte-equal dual-surface invariant.
- **Phase 88 (dogfood `crates/pmcp-server` on toolkit)** transitively consumes everything above; the documented testing pattern is what its in-binary regression tests use.

</code_context>

<specifics>
## Specific Ideas

- The doctest on `Server::get_tool` should literally show the spike-004 use-case shape: build a server with `tool_arc`, retrieve via `get_tool`, call `.handle(...)`, assert on the result. This is the "minimum demonstrable shape" the user wants downstream toolkit authors to recognize at a glance.
- The "skips auth/middleware" callout wording should be concrete: name the three things that don't run (`auth_provider`, `tool_authorizer`, `tool_middleware`) and name what to use instead for full-pipeline tests (`pmcp::Client` over stdio/streamable-http). Don't be vague — vague callouts produce the support ticket the callout was supposed to prevent.
- `tests/in_process_handler_pattern.rs` should test BOTH tools and prompts via the pattern, against a server registered with `tool_arc` and `prompt_arc` (i.e., exercise the new public surface end-to-end, not just the old `tool()`/`prompt()` paths).
- The user explicitly clarified during discussion that *production* security is not affected by the testing-pattern decision: in production, real MCP clients hit `Server::handle_request` and the full auth/authorizer/middleware pipeline runs. The handler-level pattern is a deliberate *testing* shortcut. Make sure this distinction is loud in the book section.

</specifics>

<deferred>
## Deferred Ideas

- **Public in-process driver** (e.g., `Server::dispatch(Request) -> JSONRPCResponse` or a paired `MemoryTransport`) — explicitly rejected for Phase 82. If a future phase finds the handler-level pattern insufficient (e.g., needs to test capabilities negotiation or auth-pipeline behavior end-to-end without a real transport), revisit then. Belongs in its own phase because publishing a wire-level dispatch API is a permanent public-API commitment that warrants its own design discussion.
- **Broader server introspection API** (`list_tools()`, `list_prompts()`, `get_sampling_handler()`, `get_resources()`) — explicitly rejected for Phase 82. `get_tool` parity with `get_prompt` is the only accessor the documented testing pattern needs. If/when a downstream phase needs reflective enumeration of registered handlers, scope it as a separate "server introspection" phase with its own design space.
- **Refactor existing `tool()`/`prompt()` to delegate through `tool_arc()`/`prompt_arc()`** (DRY) — explicitly rejected for Phase 82. The duplication between the impl-based and Arc-based registration paths is mechanical; collapsing it touches load-bearing existing methods that are exercised by every example/test/doc in the SDK. Cost > benefit during a milestone-opening phase. If the duplication becomes a maintenance burden in v2.3+, propose a dedicated refactor phase.
- **Shared registration helper across `ServerCoreBuilder` and `ServerBuilder`** (free function or trait) — rejected as scope creep; would introduce a new abstraction layer that isn't required by BLDR-01/-04.
- **Manual `ToolAuthorizer::authorize` invocation example in the doctest** — rejected. The "skips auth/middleware" callout is enough; showing how to manually invoke the authorizer in tests would couple the example to an evolving auth surface and double maintenance cost. Belongs in a dedicated "end-to-end testing" book chapter if anywhere.

### Reviewed Todos (not folded)
None — no pending todos matched Phase 82's scope during `cross_reference_todos`.

</deferred>

---

*Phase: 82-builder-dx-prerequisites*
*Context gathered: 2026-05-17*
