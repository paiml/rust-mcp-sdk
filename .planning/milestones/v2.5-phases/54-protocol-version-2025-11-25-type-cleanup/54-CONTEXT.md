# Phase 54: Protocol Version 2025-11-25 + Type Cleanup - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Upgrade the Rust SDK from protocol 2025-06-18 to 2025-11-25 with version negotiation supporting the latest 3 versions. Add all new spec types. Aggressively clean up the type system — split protocol.rs into domain sub-modules, remove deprecated fields, rename inconsistent types. This is a semver major (v2.0.0) breaking change. Tasks implementation is Phase 55 — this phase only adds the types and capability fields.

</domain>

<decisions>
## Implementation Decisions

### Breaking change strategy
- Aggressive cleanup — rename inconsistent types, remove all deprecated fields, restructure modules
- This is a major version bump (v2.0.0) — take full advantage of the breaking change window
- No compatibility shims, no #[deprecated] bridge — clean break with MIGRATION.md
- Crate name stays `pmcp`

### Module restructure
- Split protocol.rs (2326 lines, 58 types) into domain sub-modules:
  - `types/protocol/mod.rs` — initialization, capabilities, version negotiation
  - `types/tools.rs` — ToolInfo, CallToolParams, CallToolResult
  - `types/resources.rs` — ResourceInfo, ResourceTemplate, ReadResourceResult
  - `types/prompts.rs` — PromptInfo, PromptMessage, GetPromptResult
  - `types/tasks.rs` — TaskSchema, TaskInfo, TaskStatus (types only — implementation in Phase 55)
  - `types/content.rs` — TextContent, ImageContent, AudioContent, ResourceLink, EmbeddedResource
- Re-export everything from `types/mod.rs` for flat import path: `use pmcp::types::ToolInfo`

### Version negotiation
- Support exactly 3 versions: 2025-11-25, 2025-06-18, 2025-03-26
- Drop all 2024 protocol versions — reject with clear JSON-RPC error including supported versions list
- Use highest common version: server checks client's supported versions, picks the highest both support
- No silent downgrade to unsupported versions — explicit rejection

### New type adoption scope
- Add ALL types from the 2025-11-25 spec, even for de-prioritized features (elicitation, notifications)
- Types are cheap (wire compatibility) — handlers are what we gate
- Hand-write types with `#[derive(Serialize, Deserialize)]` — continue current idiomatic Rust pattern
- No codegen from spec schema

### Backward compatibility
- Clean break with MIGRATION.md documenting every breaking change
- No deprecation bridge release
- v1.20.0 stays on crates.io for users who haven't migrated
- MIGRATION.md maps old type names → new, old module paths → new, removed fields → replacements

### Claude's Discretion
- Exact naming of new types (follow existing conventions: PascalCase, match spec names)
- Whether to use a `ProtocolVersion` enum vs string with validation
- Internal organization of the types/protocol/ sub-module
- Which deprecated fields to remove vs which are actually still needed

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol spec
- `~/Development/mcp/sdk/typescript-sdk/packages/core/src/types.ts` — TypeScript type definitions (reference for 2025-11-25 types)
- `.planning/phases/53-review-typescript-sdk-updates/53-GAP-ANALYSIS.md` — Gap analysis with all missing types enumerated
- `.planning/phases/53-review-typescript-sdk-updates/53-01-VERIFICATION-NOTES.md` — Source-verified type comparisons with file:line refs

### Current Rust types
- `src/types/protocol.rs` — Current 2025-06-18 protocol types (2326 lines, 58 types)
- `src/types/mod.rs` — Current type re-exports
- `src/types/capabilities.rs` — ServerCapabilities, ClientCapabilities
- `src/lib.rs` lines 263-308 — LATEST_PROTOCOL_VERSION, DEFAULT_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS constants

### Version negotiation
- `src/server/wasm_server.rs` line 130 — Current version negotiation in WASM server
- `src/client/mod.rs` line 213 — Client protocol version advertisement

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/types/protocol.rs` — All existing types being restructured; patterns to follow
- `src/types/capabilities.rs` — Capability structures that need expansion for tasks
- `serde` with `rename_all = "camelCase"` — established convention for all protocol types

### Established Patterns
- `#[derive(Debug, Clone, Serialize, Deserialize)]` on all protocol types
- `#[serde(rename_all = "camelCase")]` for JSON field mapping
- `Option<T>` for optional spec fields with `#[serde(skip_serializing_if = "Option::is_none")]`
- Builder pattern for complex types (ServerCoreBuilder)

### Integration Points
- `src/server/core.rs` — ServerCore uses protocol types for initialization
- `src/client/mod.rs` — Client uses protocol types for requests
- `src/shared/jsonrpc.rs` — JSON-RPC framing around protocol messages
- Every example in `examples/` — will need import path updates after module split

</code_context>

<specifics>
## Specific Ideas

- The TypeScript SDK's types.ts at ~/Development/mcp/sdk/typescript-sdk/packages/core/src/types.ts is the authoritative reference for 2025-11-25 type shapes
- The gap analysis enumerated 20+ specific missing types — use that as the checklist
- Version negotiation should follow the same algorithm as TypeScript: highest common version
- The MIGRATION.md should be structured as a find-and-replace guide, not prose

</specifics>

<deferred>
## Deferred Ideas

- Elicitation handler implementation — types added here but handlers belong in a future phase
- Notification subscription handlers — types added, handlers deferred
- SSE transport enhancements — de-prioritized per v2.0 direction
- Spec-schema codegen validation test — could add later to catch drift

</deferred>

---

*Phase: 54-protocol-version-2025-11-25-type-cleanup*
*Context gathered: 2026-03-19*
