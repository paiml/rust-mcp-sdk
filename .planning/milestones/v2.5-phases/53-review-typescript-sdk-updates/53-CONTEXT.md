# Phase 53: Review TypeScript SDK Updates - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Compare the TypeScript MCP SDK (v2.0.0-alpha, packages split) against the Rust SDK (v1.20.0) to identify gaps worth adopting. Produce a gap analysis report with prioritized recommendations. This phase is research/analysis only — implementation happens in subsequent phases.

</domain>

<decisions>
## Implementation Decisions

### Protocol version negotiation
- Support the latest 3 protocol versions: 2025-11-25, 2025-06-18, 2025-03-26
- TypeScript supports 5 versions with fallback negotiation; Rust currently uses only the latest
- Add version negotiation during initialize handshake so older clients can still connect
- Don't support 2024 protocol versions — they're pre-production

### Conformance test suite
- Extend mcp-tester with a `conformance` command for MCP spec compliance testing
- Reuses existing transport/connection infrastructure in mcp-tester
- TypeScript has a dedicated conformance/ test directory — study its test cases as reference
- Goal: any MCP server (Rust or not) can validate spec compliance via `mcp-tester conformance <url>`

### MCP Apps (UI) gaps
- Full gap analysis needed across three dimensions:
  1. Spec alignment with ext-apps SDK v1.2.2 (method names, capabilities, types)
  2. Host-side APIs for rendering MCP Apps (not just server-side tool registration)
  3. DX ergonomics — reduce boilerplate for registering App-capable tools
- The Rust SDK already has outputSchema, structuredContent, widget-runtime, mcp-preview
- Need to identify what's missing vs what's already ahead of TypeScript

### Tasks comparison
- Rust SDK has: design doc (docs/design/tasks-feature-design.md), planned crate (pmcp-tasks), and existing implementation
- TypeScript SDK has: experimental task support with polling, resumption, message queuing
- Compare both implementations to identify alignment opportunities and gaps
- Focus on API shape compatibility (task states, polling, cancellation) for interop

### Framework adapters
- Claude's Discretion on which frameworks to recommend
- Pros/cons analysis needed:
  - **Axum**: Already used internally for streamable-http transport. Lowest effort to expose as clean adapter.
  - **Tower middleware**: Framework-agnostic, works with Axum/Hyper/any Tower-compatible server. Most flexible.
  - **Actix-web**: Popular alternative but requires separate integration effort.
- Recommendation: Tower middleware layer (framework-agnostic) + Axum convenience adapter

### Package structure
- Keep feature flags approach (not crate split)
- Feature flags already reduce compile times effectively (phase 52 proved this: 249→134 deps)
- Crate split adds complexity without proportional benefit for a Rust workspace
- TypeScript split was driven by ESM/CJS concerns that don't apply to Rust

### Claude's Discretion
- Exact format of the gap analysis report (markdown table vs prose vs both)
- Priority scoring methodology for recommendations
- Which TypeScript conformance tests to port first

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### TypeScript SDK (external, read for comparison)
- `~/Development/mcp/sdk/typescript-sdk/packages/core/src/types.ts` — Protocol types and version definitions
- `~/Development/mcp/sdk/typescript-sdk/packages/server/src/server.ts` — Server registration patterns
- `~/Development/mcp/sdk/typescript-sdk/test/conformance/` — Conformance test structure and cases
- `~/Development/mcp/sdk/typescript-sdk/packages/server/src/experimental/tasks/` — Tasks implementation

### Rust SDK (current state)
- `docs/design/tasks-feature-design.md` — Tasks feature design document
- `src/types/protocol.rs` — Protocol version and type definitions
- `src/server/traits.rs` — Server handler traits
- `src/server/mcp_apps/` — MCP Apps extension module
- `crates/mcp-tester/src/` — mcp-tester crate for conformance extension
- `crates/pmcp-tasks/` — Tasks crate (if exists)

### MCP Spec
- ext-apps SDK spec: https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `mcp-tester` crate: Already has `compliance` command — extend with `conformance` for full spec compliance
- `src/server/workflow/` module: Existing workflow system that may overlap with Tasks
- `src/server/mcp_apps/` module: Existing MCP Apps support with GUIDE.md
- `pmcp-widget-utils` crate: Shared widget utilities for bridge script injection

### Established Patterns
- Feature flags for optional functionality (http-client, logging, etc.)
- Builder pattern for server construction (ServerCoreBuilder)
- async_trait for all async traits
- serde with rename_all = "camelCase" for protocol types

### Integration Points
- `src/types/protocol.rs` — Where protocol version negotiation would be added
- `crates/mcp-tester/src/main.rs` — Where conformance command would be added
- `src/server/transport/` — Where framework adapter layer would connect

</code_context>

<specifics>
## Specific Ideas

- The TypeScript SDK removed server-side auth entirely in v2, recommending dedicated auth libraries. The Rust SDK still has JWT + Cognito/OIDC providers. This is a divergence worth analyzing — the Rust approach may be more practical for enterprise deployments.
- TypeScript's conformance test suite structure (test/conformance/) is a good model for the mcp-tester extension.
- The TypeScript SDK supports Web Standard APIs (Cloudflare Workers, Deno, Bun) which is not relevant for Rust but interesting for understanding portability trade-offs.

</specifics>

<deferred>
## Deferred Ideas

- WebSocket transport for Rust SDK — TypeScript has client-side WebSocket, Rust has it for server. Full bidirectional WebSocket may be needed.
- Cross-runtime support (WASM target for browser clients) — existing wasm feature exists but limited.

</deferred>

---

*Phase: 53-review-typescript-sdk-updates*
*Context gathered: 2026-03-19*
