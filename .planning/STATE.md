---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: CLI DX Overhaul
status: completed
stopped_at: Completed 44-02-PLAN.md
last_updated: "2026-03-08T23:00:10.904Z"
last_activity: 2026-03-08 -- Completed 44-02 Protocol tab and ChatGPT emulation
progress:
  total_phases: 18
  completed_phases: 11
  total_plans: 22
  completed_plans: 21
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-03)

**Core value:** Consistent, polished CLI experience for cargo pmcp ahead of course recording -- every command follows the same conventions for URLs, flags, auth, and output.
**Current focus:** Phase 44 complete -- mcp-preview supports ChatGPT mode

## Current Position

Phase: 44 (improving-mcp-preview-to-support-chatgpt-version)
Plan: 2 of 2 (44-02 complete -- phase done)
Status: Completed 44-02 -- Protocol tab, ChatGPT postMessage emulation, widget reload fix
Last activity: 2026-03-08 -- Completed 44-02 Protocol tab and ChatGPT emulation

Progress: [██████████] 100%

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v1.5 | Cloud Load Testing Upload | 25-26 | 2026-03-01 |

## Performance Metrics

**Velocity:**
- Total plans completed: 56 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6)
- Total phases completed: 26

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v1.6 decisions:
- 6 phases derived from 5 requirement categories: global flags, flag normalization, auth propagation, tester integration, new commands, help polish
- Phase 31 (New Commands) depends on Phase 28 (not 30) since doctor/completions don't need tester or auth
- Help polish is last phase since it touches every command and benefits from all prior changes being stable
- GlobalFlags defined in commands/mod.rs (not main.rs) to avoid circular imports
- no_color stores resolved effective value (CLI flag OR NO_COLOR env OR non-TTY)
- should_output() guard pattern for direct global_flags access; PMCP_QUIET env var for nested functions
- Secret module merges local --quiet with global --quiet via effective_quiet parameter
- Verbose field kept with allow(dead_code) -- used in precedence logic, not yet by individual commands
- [Phase 27]: Threaded not_quiet bool through validate.rs private functions rather than re-checking PMCP_QUIET env var in each function
- [Phase 34]: Axum 0.8 wildcard routes use {*path} syntax; mcp-preview bumped to 0.1.2
- [Phase 34-01]: Nested _meta.ui.resourceUri format with openai/outputTemplate for ChatGPT; HtmlMcpApp MIME type; dual-emit WidgetMeta prefersBorder
- [Phase 36]: Used explicit match arms (no wildcards) in From/TryFrom bridge for compile-time exhaustiveness
- [Phase 37]: Mirrored TypedTool::with_ui() exactly for TypedSyncTool and WasmTypedTool; WasmTypedTool tests wasm32-only gated
- [Phase 38]: Cache is sole source of truth for metadata; no fallback to handler.metadata() in hot paths; prompt_workflow() caches directly
- [Phase 39]: deep_merge in ui.rs for recursive JSON object merging; with_meta_entry on ToolInfo for composable _meta; arrays replaced not concatenated
- [Phase 39-02]: TypedToolWithOutput::with_ui() mirrors TypedTool::with_ui() for API consistency; all four tool types use identical deep_merge pattern
- [Phase 40-02]: redirect_domains excluded from nested ui.csp (ChatGPT-specific); nested csp uses spec camelCase field names; ModelOnly variant added to ToolVisibility
- [Phase 40-01]: Added legacy flat "ui/resourceUri" key to build_meta_map() matching official ext-apps dual-emit behavior
- [Phase 41-02]: AppBridge class in widget-runtime.mjs (not index.html); fall-through switch for backward compat; ui/notifications/initialized via setTimeout(0)
- [Phase 41]: Used field name meta with serde rename to _meta since leading underscores not idiomatic Rust
- [Phase 41-03]: Used TypedSyncTool::new().with_ui() in scaffold instead of tool_typed_sync_with_description() to enable tool-to-widget linking
- [Phase 42-01]: outputSchema is top-level on ToolInfo (MCP spec 2025-06-18); pmcp:outputTypeName remains in annotations as PMCP codegen extension
- [Phase 42-02]: cargo-pmcp local ToolSchema mirrors SDK ToolInfo with top-level output_schema; all docs and course content updated
- [Phase 43-01]: ResourceInfo._meta field with serde rename; URI-to-tool-meta index on ServerCore; with_widget_enrichment filtered to openai/toolInvocation/* only
- [Phase 43-02]: Post-process resources/list with clone and resources/read with deep_merge for _meta propagation from uri_to_tool_meta index
- [Phase 44-01]: Hard-coded ChatGPT descriptor/invocation keys in api.rs (mcp-preview doesn't depend on pmcp crate); derive(Default) with #[default] for PreviewMode
- [Phase 44-02]: AppBridge remains active in ChatGPT mode (postMessage is supplemental); skip iframe reload when same widget URI already loaded to preserve widget state

### Roadmap Evolution

- Phase 33 added: Fix mcp-tester failure with v1.12.0
- Phase 34 added: Fix MCP Apps ChatGPT compatibility
- Phases 35-39 added: MCP Apps code quality improvements (meta key constants, MIME type unification, TypedSyncTool UI, ToolInfo caching, ui meta merge)
- Phase 40 added: Review ChatGPT Compatibility for Apps
- Phase 41 added: ChatGPT MCP Apps Upgraded Version
- Phase 42 added: Add outputSchema top level support
- Phase 43 added: ChatGPT MCP Apps alignment
- Phase 44 added: Improving mcp-preview to support ChatGPT version

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-03-08T23:25:00.000Z
Stopped at: Completed 44-02-PLAN.md
Resume: Phase 44 complete. Next: Phase 27 (Global Flag Infrastructure) or other planned work
