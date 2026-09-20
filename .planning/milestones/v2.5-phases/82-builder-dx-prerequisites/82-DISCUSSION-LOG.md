# Phase 82: Builder DX Prerequisites - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-17
**Phase:** 82-builder-dx-prerequisites
**Areas discussed:** BLDR-03 testing surface, tool_arc / prompt_arc lift implementation, Server::get_tool accessor parity

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| BLDR-03 testing surface | In-process driver vs documented handler-level pattern vs both | ✓ |
| tool_arc / prompt_arc lift implementation | Additive copy vs refactor existing tool()/prompt() to delegate | ✓ |
| Server::get_tool accessor parity | Whether to add get_tool symmetric with existing get_prompt | ✓ |
| Scope confirmation only | Accept 3-requirement scope as-is, skip discussion | |

**User's choice:** All three real gray areas. Scope-confirmation-only was skipped — user wanted to discuss the design trade-offs, not just rubber-stamp.

---

## BLDR-03 testing surface

### Q1: How should external toolkit integration tests drive a built pmcp::Server?

| Option | Description | Selected |
|--------|-------------|----------|
| Documented handler-level pattern | Formalize what tests/skills_integration.rs already does (get_prompt + handler.handle); add get_tool; zero new public dispatch surface | ✓ |
| Public in-process driver | Add Server::dispatch / paired MemoryTransport; covers full wire path including JSONRPC + capabilities negotiation; permanent new public API | |
| Both | Handler-level for common case + in-process driver for full wire coverage | |

**User's choice:** Documented handler-level pattern.
**Notes:** Picked the recommended option — cheapest, smallest blast radius, already validated by spike 002 and spike 004. Avoids committing to a permanent wire-level public API for a use case the handler-level pattern covers.

### Q2: Where should the documented handler-level testing pattern live?

| Option | Description | Selected |
|--------|-------------|----------|
| Doctest + tests/ reference + book section | Comprehensive doctest on get_tool/get_prompt + tests/in_process_handler_pattern.rs regression anchor + pmcp-book testing chapter section | ✓ |
| Doctest only | Single doctest on get_tool/get_prompt; lower discoverability | |
| Numbered example + book section | examples/sNN_in_process_testing.rs + book section; skips doctest | |

**User's choice:** Doctest + tests/ reference + book section.
**Notes:** Three discoverability surfaces — toolkit authors find it via rustdoc, via examples grep, or via book TOC.

### Q3: Should the docs call out the testing-scope limitation (skips auth/middleware) explicitly?

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit "what this skips" callout | Doctest + book section both include short "this exercises handler logic only — auth/authorizer/middleware NOT invoked" callout | ✓ |
| Implicit — just show the pattern | No callout; users discover the limitation on their own | |
| Callout + show how to invoke authorizer manually | Callout PLUS follow-up doctest showing manual ToolAuthorizer::authorize invocation | |

**User's choice:** Explicit callout (after a clarification round). First response was "Other": *"I'm not sure I understand what is the handler-level pattern and what does it mean that is bypass the important security aspects. Does it mean that we can't build a more secure and complete MCP server from configuration, or can we enrich the configuration to support them?"*
**Notes:** Clarified that the "handler-level pattern" is a *testing* shortcut, not a production pattern. In production, real MCP clients hit `Server::handle_request` and the full auth/authorizer/middleware pipeline runs. The handler-level shortcut is what toolkit authors use in `cargo test` to verify their config-driven handlers produce the right output without spinning up a transport. Config-driven security is fully supported in Phase 83+ via `AuthProvider` / `SecretsProvider` / code-mode policy in `config.toml`. After clarification, user picked the explicit callout option.

---

## tool_arc / prompt_arc lift implementation

### Q1: How should we lift tool_arc / prompt_arc into the public pmcp::ServerBuilder?

| Option | Description | Selected |
|--------|-------------|----------|
| Pure additive copy | Copy the bodies from ServerCoreBuilder verbatim into public ServerBuilder; existing tool()/prompt() untouched | ✓ |
| Refactor tool()/prompt() to delegate to *_arc | Add *_arc, then refactor tool() to internally call tool_arc(Arc::new(handler)) for DRY | |
| Refactor both builders to share a single helper | Pull registration logic into shared helper that both builders use | |

**User's choice:** Pure additive copy.
**Notes:** Recommended option. Zero blast radius — `tool()` / `prompt()` are in nearly every example/test/doc and refactoring them at the opening of v2.2 risks subtle regressions.

### Q2: Lift only the two required (BLDR-01/-02), or all six _arc methods at once?

| Option | Description | Selected |
|--------|-------------|----------|
| Lift all six at once | tool_arc, prompt_arc, resources_arc, sampling_arc, auth_provider_arc, tool_authorizer_arc | ✓ |
| Lift only the two required | Stick strictly to BLDR-01/-02 scope; other four wait for a phase that needs them | |
| Lift two + tool_authorizer_arc | tool_arc, prompt_arc, plus tool_authorizer_arc (config-driven auth in v2.2 scope) | |

**User's choice:** Lift all six at once.
**Notes:** Recommended. Closes the entire Arc-symmetry gap in one phase so Phase 83/84 don't trip over a missing one. Each lift is mechanically identical (~10 lines, pure copy, same additive blast radius).

### Q3: How should the four extra _arc lifts be tracked in REQUIREMENTS.md?

| Option | Description | Selected |
|--------|-------------|----------|
| Single umbrella BLDR-04 "Arc-handler symmetry" | One new requirement covering all four extras; Phase 82 reqs bump from 3 to 4 | ✓ |
| Four separate requirements BLDR-04..07 | One requirement per lifted method; most granular tracking | |
| No new requirement — fold into BLDR-01/-02 | Treat the four extras as implementation details of BLDR-01/-02 spirit | |

**User's choice:** Single umbrella BLDR-04.
**Notes:** Balances auditability (paper trail exists) with proportionality (the four lifts are mechanically one decision, not four).

---

## Server::get_tool accessor parity

### Q1: How should we add Server::get_tool to match the existing get_prompt accessor?

| Option | Description | Selected |
|--------|-------------|----------|
| Add get_tool with same signature as get_prompt | pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn ToolHandler>> | ✓ |
| Add get_tool + list_tools / list_prompts accessors | Broader server-introspection surface (list_tools, list_prompts iterators) | |
| Add get_tool plus alias get_tool_handler / get_prompt_handler | Rename for explicit "this returns the HANDLER" intent | |

**User's choice:** Add get_tool with same signature as get_prompt.
**Notes:** Recommended. Perfect symmetry, ~5 lines. Broader introspection is its own design space and out of scope for Phase 82.

### Q2: Where does Server::get_tool fit in REQUIREMENTS.md?

| Option | Description | Selected |
|--------|-------------|----------|
| Fold into BLDR-03 as implementation detail | Update BLDR-03 wording to explicitly name get_tool as the API surface the documented testing pattern needs | ✓ |
| New separate BLDR-05 requirement | One row per method; more auditable but BLDR-03 reads weirdly without it | |
| Fold into BLDR-04 Arc-handler symmetry | Stretch BLDR-04 to cover server-side accessor symmetry too | |

**User's choice:** Fold into BLDR-03 as implementation detail.
**Notes:** Recommended. `get_tool` is the enabler for BLDR-03's documented pattern, not a standalone requirement.

---

## Claude's Discretion

- Exact wording of the doctest example body (build → tool_arc → get_tool → handle → assert).
- Exact naming/structure of `tests/in_process_handler_pattern.rs`.
- Whether to add a property test asserting `tool("x", T)` and `tool_arc("x", Arc::new(T))` produce structurally identical post-build state (recommended yes; final call to planner).
- MSRV / clippy implications (expected none; planner validates).

## Deferred Ideas

- Public in-process driver (`Server::dispatch` / `MemoryTransport`) — explicitly rejected for Phase 82. Belongs in its own phase if a future need surfaces; publishing a wire-level dispatch API is a permanent commitment that warrants its own design discussion.
- Broader server introspection API (`list_tools`, `list_prompts`, `get_sampling_handler`, `get_resources`) — explicitly rejected. Out of scope; would open its own decisions.
- Refactor `tool()`/`prompt()` to delegate through `tool_arc()`/`prompt_arc()` (DRY) — explicitly rejected. Cost > benefit during a milestone-opening phase; revisit if duplication becomes a maintenance burden in v2.3+.
- Shared registration helper across `ServerCoreBuilder` and `ServerBuilder` — rejected as scope creep.
- Manual `ToolAuthorizer::authorize` invocation example in the doctest — rejected. Callout is sufficient; full example would couple the doctest to an evolving auth surface and double maintenance cost.
