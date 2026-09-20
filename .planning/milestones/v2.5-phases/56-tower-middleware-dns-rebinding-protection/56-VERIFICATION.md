---
phase: 56-tower-middleware-dns-rebinding-protection
verified: 2026-03-20T00:00:00Z
status: passed
score: 5/5 success criteria verified
re_verification: false
---

# Phase 56: Tower Middleware + DNS Rebinding Protection Verification Report

**Phase Goal:** Build a Tower Layer stack for MCP server hosting: DNS rebinding protection (Host + Origin header validation against allowed origins), security response headers, and origin-locked CORS. Axum convenience adapter (`pmcp::axum::router()`) for the 90% case. Enterprise security focus — fix CVE-pattern wildcard CORS and achieve MCP spec 2025-03-26 Origin validation compliance.
**Verified:** 2026-03-20
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | DnsRebindingLayer validates Host header (always) and Origin header (when present), returns 403 on mismatch | VERIFIED | `impl<S> Layer<S> for DnsRebindingLayer` in dns_rebinding.rs; 4 occurrences of `StatusCode::FORBIDDEN`; 15 unit tests including `test_reject_bad_host`, `test_reject_bad_origin`, `test_accept_good_host_no_origin` |
| 2 | SecurityHeadersLayer adds X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store | VERIFIED | All three headers inserted in `SecurityHeadersService::call()`; `impl<S> Layer<S> for SecurityHeadersLayer` present; 6 tests including `test_default_headers`, `test_no_hsts` |
| 3 | `pmcp::axum::router(server)` returns axum::Router with DNS rebinding + security headers + origin-locked CORS | VERIFIED | `src/server/axum_router.rs` wires `.layer(config.security_headers).layer(DnsRebindingLayer::new(allowed)).layer(cors)`; `pub mod axum` in lib.rs re-exports `router`, `router_with_config`, `AllowedOrigins`, `RouterConfig` |
| 4 | StreamableHttpServer no longer uses wildcard `Access-Control-Allow-Origin: *` | VERIFIED | Zero matches for `Allow-Origin.*\*` in streamable_http_server.rs; `add_cors_headers` now reflects request `Origin` only when allowed via `AllowedOrigins::is_allowed_origin()`; `resolve_allowed_origins()` helper defaults to `AllowedOrigins::localhost()` |
| 5 | Example 55 (ServerHttpMiddleware) still compiles unchanged | VERIFIED | `examples/55_server_middleware.rs` uses `..Default::default()` for `StreamableHttpServerConfig` — the new `allowed_origins: None` field is covered by the default; example structure unchanged |

**Score:** 5/5 success criteria verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/tower_layers/mod.rs` | Module declarations and re-exports | VERIFIED | Declares `pub mod dns_rebinding`, `pub mod security_headers`; re-exports `AllowedOrigins`, `DnsRebindingLayer`, `DnsRebindingService`, `SecurityHeadersLayer`, `SecurityHeadersService` |
| `src/server/tower_layers/dns_rebinding.rs` | AllowedOrigins, DnsRebindingLayer, DnsRebindingService | VERIFIED | 459 lines; all three types present with full Tower `Layer` + `Service` impls; 15 inline tests |
| `src/server/tower_layers/security_headers.rs` | SecurityHeadersLayer, SecurityHeadersService | VERIFIED | 269 lines; both types present with full Tower `Layer` + `Service` impls; 6 inline tests; no HSTS per D-12 |
| `src/server/axum_router.rs` | router(), router_with_config(), RouterConfig | VERIFIED | 185 lines; all three exported; `AllowedOrigins` re-exported from tower_layers; 3 tests |
| `src/lib.rs` | pmcp::axum re-export module | VERIFIED | `pub mod axum` at lines 99-105; crate-root re-export `pub use server::tower_layers::{AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer}` at line 141 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `dns_rebinding.rs` | `tower::Layer + tower::Service` | `impl<S> Layer<S> for DnsRebindingLayer` | WIRED | Pattern found at line 194 |
| `security_headers.rs` | `tower::Layer + tower::Service` | `impl<S> Layer<S> for SecurityHeadersLayer` | WIRED | Pattern found at line 77 |
| `dns_rebinding.rs` | `tower_http::cors::AllowOrigin` | `to_cors_allow_origin()` using `AllowOrigin::list()` | WIRED | Lines 127-134 |
| `axum_router.rs` | `tower_layers::DnsRebindingLayer` | `DnsRebindingLayer::new(allowed)` | WIRED | Line 136 |
| `axum_router.rs` | `tower_layers::SecurityHeadersLayer` | `config.security_headers` (default) applied via `.layer()` | WIRED | Line 135 |
| `axum_router.rs` | `tower_http::cors::CorsLayer` | `CorsLayer::new().allow_origin(allowed.to_cors_allow_origin())` | WIRED | Lines 112-126 |
| `axum_router.rs` | `streamable_http_server` | `build_mcp_router(state)` + `make_server_state(server, config)` | WIRED | Lines 109-110 |
| `streamable_http_server.rs` | `tower_layers::AllowedOrigins` | `resolve_allowed_origins()` + `add_cors_headers()` throughout | WIRED | `use crate::server::tower_layers::AllowedOrigins` at line 7; `resolve_allowed_origins()` helper at line 864 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TOWER-MIDDLEWARE | 56-01, 56-02 | Tower Layer/Service implementation for MCP HTTP security | SATISFIED | `DnsRebindingLayer` and `SecurityHeadersLayer` implement `tower::Layer`; gated behind `streamable-http` feature in Cargo.toml |
| DNS-REBINDING | 56-01, 56-02 | Host + Origin header validation returning 403 on mismatch | SATISFIED | `DnsRebindingService::call()` validates Host (always) and Origin (when present); wildcard CORS eliminated from `StreamableHttpServer` |
| AXUM-ADAPTER | 56-02 | `pmcp::axum::router()` one-liner for 90% use case | SATISFIED | `router()` and `router_with_config()` in `axum_router.rs`; re-exported via `pmcp::axum` module in lib.rs |

Note: These requirement IDs (TOWER-MIDDLEWARE, DNS-REBINDING, AXUM-ADAPTER) are scoped to phase 56 and do not appear in the global `.planning/REQUIREMENTS.md` (which covers v1.6 CLI DX requirements). No orphaned requirements found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `security_headers.rs` | 1 | `Strict-Transport-Security` appears in test assertion (assert `is_none()`) | INFO | Correctly absent — the test verifies no HSTS is added per D-12. Not a SATD pattern. |

No SATD comments (TODO/FIXME/XXX/HACK), no stub implementations (`return null`, empty handlers), no console-only functions found in any of the 3 new source files.

### Human Verification Required

None. All success criteria are verifiable programmatically:
- Tower Layer/Service impls confirmed by code inspection
- 403 rejection confirmed by test assertions in `test_reject_bad_host` and `test_reject_bad_origin`
- Security headers confirmed by test assertions in `test_default_headers`
- Wildcard CORS absence confirmed by grep (zero matches)
- Example 55 compatibility confirmed by `..Default::default()` usage pattern

### Gaps Summary

None. All 5 success criteria from ROADMAP.md are fully satisfied:

1. `DnsRebindingLayer` is a real Tower `Layer` + `Service` with Host/Origin validation and 403 rejection — not a stub.
2. `SecurityHeadersLayer` inserts all three OWASP headers with per-header opt-out — not a stub.
3. `pmcp::axum::router()` composes all three layers (`SecurityHeadersLayer` + `DnsRebindingLayer` + `CorsLayer`) over the MCP handler router — fully wired.
4. `StreamableHttpServer` uses `AllowedOrigins`-based CORS throughout — no wildcard `*` remains.
5. Example 55 uses `..Default::default()` so the new `allowed_origins` field does not require changes.

All 4 commits verified in git history: `9b702b1`, `bcd487b`, `f550146`, `14ff873`.

---

_Verified: 2026-03-20_
_Verifier: Claude (gsd-verifier)_
