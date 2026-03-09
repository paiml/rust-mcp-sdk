# Phase 43: ChatGPT MCP Apps alignment - Context

**Gathered:** 2026-03-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets. Align `_meta` propagation across `resources/list`, `resources/read`, and `tools/call` with ChatGPT's expected protocol as documented in the Pizzaz reference server comparison.

Reference: `/Users/guy/Development/mcp/sdk/pmcp-run/built-in/sql-api/servers/.planning/phases/07-chatgpt-mcp-apps-alignment-take-2/CHATGPT_WIDGET_PROTOCOL_ANALYSIS.md`

</domain>

<decisions>
## Implementation Decisions

### ResourceInfo _meta propagation (Gap 1)
- Add `_meta: Option<Map>` field to `ResourceInfo` struct
- Auto-populate from the linked tool's `_meta` — developer writes _meta once on the tool, SDK propagates to resources/list
- Propagate all `openai/*` prefixed keys (future-proof without leaking internal `ui.*` keys)
- Auto-set `mimeType` on ResourceInfo from the registered resource's MIME type
- When multiple tools share the same widget resource URI, first tool registered wins for _meta resolution

### tools/call _meta filtering (Gap 3)
- Modify `with_widget_enrichment()` to filter _meta before applying — only `openai/toolInvocation/*` keys pass through
- Strip everything: no `openai/outputTemplate`, no `openai/widgetAccessible`, no `ui.*`, no custom developer keys
- tools/call _meta becomes exactly `{openai/toolInvocation/invoking, openai/toolInvocation/invoked}`
- Always filter regardless of host type — start with ChatGPT compatibility, extend later
- Filter happens inside `with_widget_enrichment()` — single place, automatic for all tool calls

### resources/read _meta alignment (Gap 2)
- Merge both key sets: ChatGPT descriptor keys (outputTemplate, invoking, invoked, widgetAccessible) PLUS existing display keys (widgetCSP, prefersBorder, etc.)
- Descriptor keys come from the linked tool's `_meta` — same source as resources/list, tool is single source of truth
- Merge happens at the handler level (core.rs or resource handler), not inside the ChatGptAdapter — adapter stays focused on HTML transformation
- Use deep_merge (from Phase 39) to combine adapter display keys with tool descriptor keys

### title field on ToolInfo (Gap 4)
- Skip for now — ChatGPT may read title from annotations
- Test without top-level title first; add dual-emit later if widgets still don't render

### Design principles (user-stated)
- DRY: tool _meta is the single source of truth, auto-propagated everywhere, filtered per context
- Keep it simple: expect frequent changes and additional hosts in this extension area
- Start with ChatGPT compatibility, extend to other hosts later

### Claude's Discretion
- Internal implementation of the openai/* key filter (regex, prefix match, or hardcoded list)
- How to look up the linked tool's _meta from a resource URI during resources/list and resources/read
- Whether to cache the propagated _meta or compute it on each request
- Test strategy for verifying _meta content across all 3 protocol methods

</decisions>

<specifics>
## Specific Ideas

- The Pizzaz reference server (OpenAI's sample) is the gold standard for what ChatGPT expects
- The same 4 `_meta` keys appear identically in 3 places (tools/list, resources/list, resources/read) — redundant by design so ChatGPT can discover widget metadata from any entry point
- tools/call is the exception: only 2 invocation keys (no outputTemplate, no widgetAccessible)
- `text/html+skybridge` is already supported as `UIMimeType::HtmlSkybridge` — MIME type gap from analysis is TBD, not blocking

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ResourceInfo` struct: `src/types/protocol.rs:898-909` — needs `_meta` field addition
- `with_widget_enrichment()`: `src/types/protocol.rs:589-601` — needs _meta filtering
- `ToolInfo::widget_meta()`: `src/types/protocol.rs:402-413` — detects widget tools by checking for outputTemplate/resourceUri keys
- `deep_merge()`: `src/types/ui.rs` — recursive JSON object merging from Phase 39
- `ChatGptAdapter`: `src/server/mcp_apps/adapter.rs` — transforms HTML, produces display-key _meta via WidgetMeta
- `WidgetMeta::to_meta_map()`: `src/types/mcp_apps.rs:246-383` — produces display keys
- `ChatGptToolMeta::to_meta_map()`: `src/types/mcp_apps.rs:453-575` — produces tool openai/* keys
- `build_meta_map()` in `src/types/ui.rs` — emits three URI key variants (nested, flat, openai/)

### Established Patterns
- Dual-emit pattern: nested `ui.*` + flat `openai/*` keys (Phase 34, 40)
- `meta` field with `#[serde(rename = "_meta")]` for Rust-idiomatic naming (Phase 41)
- `with_meta_entry()` on ToolInfo for composable _meta (Phase 39)
- `HostType` enum exists for per-host branching but NOT used for filtering (user decided: always filter)

### Integration Points
- `core.rs:355` — where with_widget_enrichment is called after handler execution
- Resource handler in core.rs — where resources/read response is built
- UIResourceBuilder in `src/server/ui.rs` — where resources are registered
- ServerCoreBuilder tool/resource registration — where tool-to-resource URI linking happens

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 43-chatgpt-mcp-apps-alignment*
*Context gathered: 2026-03-08*
