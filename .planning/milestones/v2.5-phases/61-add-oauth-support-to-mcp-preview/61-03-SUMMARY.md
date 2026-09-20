---
phase: 61-add-oauth-support-to-mcp-preview
plan: 03
subsystem: auth
tags: [oauth, forward_raw, 401, 403, wasm-bridge, gap-closure]

# Dependency graph
requires:
  - phase: 61-add-oauth-support-to-mcp-preview
    plan: 01
    provides: McpRequestError enum with AuthRequired variant
  - phase: 61-add-oauth-support-to-mcp-preview
    plan: 02
    provides: OAuthManager auth detection checking response.status === 401 || 403
provides:
  - forward_raw returns McpRequestError::AuthRequired for upstream 401/403
  - forward_mcp maps AuthRequired to upstream status code (not 502)
  - McpProxy::new() dead_code warning suppressed
affects: [61-add-oauth-support-to-mcp-preview]

# Tech tracking
stack: [rust, axum, reqwest]
patterns_used: [McpRequestError propagation, status code mapping]
---

# Plan 61-03 Summary: Fix WASM Bridge 401/403 Propagation

## One-liner
Changed forward_raw to detect 401/403 before check_response and return McpRequestError::AuthRequired, updated forward_mcp to map AuthRequired to upstream status code instead of 502 BAD_GATEWAY

## What Changed

### proxy.rs
- `forward_raw` signature changed from `Result<RawForwardResult>` to `Result<RawForwardResult, McpRequestError>`
- Added 401/403 detection before check_response (same pattern as send_request)
- Non-auth errors return `McpRequestError::Other` with descriptive message
- Added `#[allow(dead_code)]` on `McpProxy::new()` (public API for consumers)

### handlers/api.rs
- `forward_mcp` Err arm split into two: `AuthRequired` returns upstream status code, `Other` returns 502

## Verification
- `cargo check -p mcp-preview` — zero errors, zero warnings
- `cargo check --workspace` — zero errors
- McpRequestError::AuthRequired appears in both send_request and forward_raw (proxy.rs)
- McpRequestError::AuthRequired appears in list_tools, call_tool, list_resources, and forward_mcp (api.rs)
- BAD_GATEWAY only used for McpRequestError::Other arms

## Self-Check: PASSED

All acceptance criteria verified:
- [x] forward_raw returns McpRequestError::AuthRequired for 401/403
- [x] forward_mcp returns upstream status code for auth failures
- [x] Non-auth errors still produce 502 BAD_GATEWAY
- [x] McpProxy::new() compiles without dead_code warning
- [x] Zero compiler warnings in mcp-preview crate

## Deviations
None — plan executed as specified.

## Key Files

### Created
(none)

### Modified
- `crates/mcp-preview/src/proxy.rs` — forward_raw return type and 401/403 detection
- `crates/mcp-preview/src/handlers/api.rs` — forward_mcp AuthRequired matching
