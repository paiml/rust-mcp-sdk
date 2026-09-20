# Phase 58: #[mcp_tool] Proc Macro - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-21
**Phase:** 58-mcp-tool-proc-macro
**Areas discussed:** DX philosophy, naming, state injection, registration, router scope

---

## DX Philosophy

**User direction:** KPI is developer experience. Minimize DRY, potential errors, and Rust-specific complexities. Target audience finds Rust scary. Be opinionated about best practices (Streamable HTTP, prompts, stateless, Tasks, strong validation).

**Additional deliverable:** Design document (`docs/design/mcp-tool-macro-design.md`) for team feedback on DX.

---

## Macro Naming

| Option | Description | Selected |
|--------|-------------|----------|
| `#[mcp_tool]` | Distinguish from non-MCP tool patterns in agent frameworks | yes |
| `#[tool]` (keep existing) | Simpler, already exists | coexist (backward compat) |

**User's choice:** `#[mcp_tool]` explicitly — "to distinguish it from tools that are used in agents without the MCP standard"

---

## All Other Decisions

| Option | Description | Selected |
|--------|-------------|----------|
| Claude's discretion based on DX principles | Design all remaining choices to minimize boilerplate and Rust complexity | yes |

**User's choice:** "For the rest, please also create a design document" — delegated all technical decisions to Claude with DX as the north star.

---

## Claude's Design Choices (Rationale)

- **State<T> extractor pattern**: Borrowed from Axum — familiar to Rust web developers, hides Arc ceremony
- **Description mandatory**: LLMs can't use tools without descriptions — compile-time enforcement prevents silent failures in production
- **`#[mcp_server]` for impl blocks**: Natural `&self` access to state, all tools visible together, single registration call
- **Parameter matching by type**: More forgiving than positional — developers can reorder params without breaking
- **Typed output encouraged**: Generates outputSchema automatically — enables composition without extra work
- **Sync opt-in**: Async is the MCP default (network calls); sync is the exception, not the rule

## Deferred Ideas

- `#[mcp_prompt]` — Phase 59
- `#[mcp_resource]` — future phase
- WASM macro support
