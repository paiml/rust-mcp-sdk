# Phase 56: Tower Middleware + DNS Rebinding Protection - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a Tower Layer stack for MCP server hosting: DNS rebinding protection (Host + Origin header validation), security response headers, and JSON-RPC routing. Axum convenience adapter (`pmcp::axum::router()`) returns a ready-to-serve Router with security layers applied. Enterprise security focus. The existing `ServerHttpMiddleware` trait is preserved — Tower Layers wrap the outside, custom middleware runs on the inside.

</domain>

<decisions>
## Implementation Decisions

### Tower vs existing middleware
- **D-01:** Tower Layers wrap OUTSIDE the existing ServerHttpMiddleware chain — users of the existing middleware trait are unaffected
- **D-02:** Tower Layer usage is default via `pmcp::axum::router()` — users who call StreamableHttpServer directly get no Tower layers unless they opt in
- **D-03:** No breaking changes to ServerHttpMiddleware trait or existing middleware implementations (example 55)

### DNS rebinding protection
- **D-04:** Auto-detect allowed origins from bind address (localhost:port → allow localhost, 127.0.0.1, [::1]) with explicit allow-list override for deployed servers
- **D-05:** Failed Host header check returns HTTP 403 Forbidden with explanation body, logged at WARN level
- **D-06:** Validate BOTH Host header (DNS rebinding) AND Origin header (CSRF) — defense in depth
- **D-07:** Replace current `Access-Control-Allow-Origin: *` with origin-locked CORS — same allow-list drives both DNS rebinding and CORS

### Axum convenience API
- **D-08:** `pmcp::axum::router(server)` returns a full `axum::Router` with MCP routes + DNS rebinding Layer + security headers Layer applied
- **D-09:** Users call `.merge()` or `.nest()` to add their own routes, bring their own listener via `axum::serve`
- **D-10:** router() wraps StreamableHttpServer internally — creates the same Axum routes but returns a Router instead of running a server. StreamableHttpServer still works for the `run_streamable_http()` convenience path

### Security headers
- **D-11:** Sensible defaults applied by the security headers Layer: X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store (for JSON-RPC responses)
- **D-12:** No HSTS (that's the reverse proxy's job). Users can override or disable individual headers
- **D-13:** CORS Allow-Origin reflects the request's Origin if it's in the allowed list — no more wildcard *

### Claude's Discretion
- Tower Layer trait implementation details (how to compose DnsRebindingLayer + SecurityHeadersLayer)
- Whether to expose individual layers publicly or only via the `router()` convenience function
- How to integrate the allowed-origins config into StreamableHttpServerConfig
- Whether localhost auto-detection includes port matching or just hostname
- Error response format for 403 (plain text vs JSON)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Current HTTP middleware system
- `src/server/http_middleware.rs` — ServerHttpMiddleware trait (line 184), ServerHttpMiddlewareChain (line 288), Axum adapters from_axum/into_axum (lines 914-991)
- `src/server/streamable_http_server.rs` — Axum Router setup (line 300), handler dispatch (line 645), CORS headers (line 1366), session management (line 242)
- `examples/55_server_middleware.rs` — Example of custom ServerHttpMiddleware (must not break)

### Security
- `src/server/streamable_http_server.rs:1366-1382` — Current CORS implementation (Access-Control-Allow-Origin: *)
- `src/server/streamable_http_server.rs:665-668` — Auth header extraction path

### Dependencies
- `Cargo.toml:84` — axum 0.8.5 (already present, behind streamable-http feature)
- `Cargo.toml:147` — streamable-http feature flag gates axum, hyper, etc.

### Builder integration
- `src/server/builder.rs` — ServerCoreBuilder for .task_store(), .with_http_middleware() patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ServerHttpMiddleware` trait with priority ordering and chain execution — Tower Layers sit outside this
- `adapters::from_axum()` / `into_axum()` — conversion between Axum and PMCP HTTP types
- Axum Router already configured with MCP routes at `streamable_http_server.rs:300-305`
- `StreamableHttpServerConfig` — existing config struct for HTTP server settings

### Established Patterns
- Feature flags for optional functionality (`streamable-http` gates all HTTP code)
- Builder pattern for server construction
- Two-path dispatch: fast path (no middleware) and middleware path
- Session management via `mcp-session-id` header

### Integration Points
- `streamable_http_server.rs:300-305` — Where Axum Router is built (Tower Layers wrap here)
- `streamable_http_server.rs:1366-1382` — Where CORS headers are set (replace with Layer)
- `StreamableHttpServerConfig` — Where allowed_origins config would be added
- `src/lib.rs` — Where `pub mod axum` would be re-exported

</code_context>

<specifics>
## Specific Ideas

- The `pmcp::axum::router()` function should feel like one line of code to get a secure MCP server running
- DNS rebinding is the primary security concern for locally-running MCP servers accessed by browser-based clients
- The allow-list should be the single source of truth for Host validation, Origin validation, and CORS
- Tower Layers should be composable — users can take DnsRebindingLayer without SecurityHeadersLayer if they want

</specifics>

<deferred>
## Deferred Ideas

- Rate limiting Layer — future phase, needs per-client tracking
- WebSocket transport Layer — not in scope, MCP uses streamable HTTP
- mTLS / client certificate validation — separate security phase
- Request size limiting — could be added later as another Layer
- OpenTelemetry tracing Layer — would be useful but separate concern

</deferred>

---

*Phase: 56-tower-middleware-dns-rebinding-protection*
*Context gathered: 2026-03-21*
