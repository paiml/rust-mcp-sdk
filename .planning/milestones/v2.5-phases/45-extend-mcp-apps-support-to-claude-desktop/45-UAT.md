---
status: testing
phase: 45-extend-mcp-apps-support-to-claude-desktop
source: 45-01-SUMMARY.md, 45-02-SUMMARY.md, 45-03-SUMMARY.md
started: 2026-03-09T20:00:00Z
updated: 2026-03-09T20:15:00Z
---

## Current Test
<!-- OVERWRITE each test - shows where we are -->

number: 1
name: Examples updated to standard MCP Apps pattern
expected: |
  All 3 MCP Apps examples (chess, dataviz, map) use McpAppsAdapter instead of ChatGptAdapter,
  TypedSyncTool with .with_ui() for tool-widget association, and no ChatGPT-specific code.
  All compile and pass clippy with zero warnings.
awaiting: user response

## Tests

### 1. Examples updated to standard MCP Apps pattern
expected: All 3 MCP Apps examples (chess, dataviz, map) use McpAppsAdapter instead of ChatGptAdapter, TypedSyncTool with .with_ui() for tool-widget association, and no ChatGPT-specific code. All compile and pass clippy with zero warnings.
result: [pending]

### 2. mcp-preview Standard mode rendering
expected: Run `cargo pmcp preview` on an MCP Apps example. The preview launches in Standard MCP Apps mode by default. Widget iframe loads and renders correctly.
result: [pending]

### 3. mcp-preview ChatGPT mode metadata enrichment
expected: Run `cargo pmcp preview --mode chatgpt` on an MCP Apps example. Protocol diagnostics tab shows openai/* keys PRESENT (enriched preview-side from standard ui.resourceUri).
result: [pending]

### 4. Widget renders in ChatGPT preview mode
expected: Same example in `--mode chatgpt` — widget iframe loads and renders correctly, same as standard mode.
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0

## Gaps

[none yet]
