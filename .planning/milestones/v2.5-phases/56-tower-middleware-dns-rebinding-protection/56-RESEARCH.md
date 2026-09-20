# Phase 56: Tower Middleware + DNS Rebinding Protection - Research

**Researched:** 2026-03-20
**Domain:** Tower middleware / HTTP security / Axum integration
**Confidence:** HIGH

## Summary

This phase adds Tower Layer-based middleware to the PMCP SDK for DNS rebinding protection, security response headers, and origin-locked CORS. The existing `ServerHttpMiddleware` trait is an internal request/response pipeline; Tower Layers wrap the Axum Router externally, providing defense-in-depth that runs before any PMCP code touches the request.

The MCP specification (2025-03-26) explicitly REQUIRES servers to validate the `Origin` header to prevent DNS rebinding attacks (CVE-2025-66414 affected the TypeScript SDK; CVE-2025-66416 affected the Python SDK). This is not optional security hardening -- it is spec compliance. The current PMCP SDK ships `Access-Control-Allow-Origin: *` in `add_cors_headers()` (streamable_http_server.rs:1367), which is exactly the vulnerability pattern that earned CVEs in other SDKs.

**Primary recommendation:** Implement two Tower Layers (`DnsRebindingLayer` for Host+Origin validation, `SecurityHeadersLayer` for response hardening), compose them into the `pmcp::axum::router()` convenience function, and replace the hardcoded `Access-Control-Allow-Origin: *` with origin-locked CORS driven by a single `AllowedOrigins` config.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Tower Layers wrap OUTSIDE the existing ServerHttpMiddleware chain -- users of the existing middleware trait are unaffected
- **D-02:** Tower Layer usage is default via `pmcp::axum::router()` -- users who call StreamableHttpServer directly get no Tower layers unless they opt in
- **D-03:** No breaking changes to ServerHttpMiddleware trait or existing middleware implementations (example 55)
- **D-04:** Auto-detect allowed origins from bind address (localhost:port -> allow localhost, 127.0.0.1, [::1]) with explicit allow-list override for deployed servers
- **D-05:** Failed Host header check returns HTTP 403 Forbidden with explanation body, logged at WARN level
- **D-06:** Validate BOTH Host header (DNS rebinding) AND Origin header (CSRF) -- defense in depth
- **D-07:** Replace current `Access-Control-Allow-Origin: *` with origin-locked CORS -- same allow-list drives both DNS rebinding and CORS
- **D-08:** `pmcp::axum::router(server)` returns a full `axum::Router` with MCP routes + DNS rebinding Layer + security headers Layer applied
- **D-09:** Users call `.merge()` or `.nest()` to add their own routes, bring their own listener via `axum::serve`
- **D-10:** router() wraps StreamableHttpServer internally -- creates the same Axum routes but returns a Router instead of running a server. StreamableHttpServer still works for the `run_streamable_http()` convenience path
- **D-11:** Sensible defaults applied by the security headers Layer: X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store (for JSON-RPC responses)
- **D-12:** No HSTS (that's the reverse proxy's job). Users can override or disable individual headers
- **D-13:** CORS Allow-Origin reflects the request's Origin if it's in the allowed list -- no more wildcard *

### Claude's Discretion
- Tower Layer trait implementation details (how to compose DnsRebindingLayer + SecurityHeadersLayer)
- Whether to expose individual layers publicly or only via the `router()` convenience function
- How to integrate the allowed-origins config into StreamableHttpServerConfig
- Whether localhost auto-detection includes port matching or just hostname
- Error response format for 403 (plain text vs JSON)

### Deferred Ideas (OUT OF SCOPE)
- Rate limiting Layer -- future phase, needs per-client tracking
- WebSocket transport Layer -- not in scope, MCP uses streamable HTTP
- mTLS / client certificate validation -- separate security phase
- Request size limiting -- could be added later as another Layer
- OpenTelemetry tracing Layer -- would be useful but separate concern
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TOWER-MIDDLEWARE | Build Tower Layer stack for MCP server hosting | Tower 0.5.2 already in dependency tree via axum 0.8; tower-http 0.6.8 in lockfile via mcp-preview. Custom Layer implementation follows standard tower::Layer + tower::Service pattern. |
| DNS-REBINDING | DNS rebinding protection via Host+Origin header validation | MCP spec 2025-03-26 REQUIRES Origin validation. CVE-2025-66414/66416 demonstrate the vulnerability. Implementation validates Host header against allowed origins, Origin header against same list, returns 403 on mismatch. |
| AXUM-ADAPTER | Axum convenience adapter `pmcp::axum::router()` for the 90% case | New `src/server/axum_router.rs` module gated behind `streamable-http` feature. Extracts Router construction from `StreamableHttpServer::start()` into a standalone function that applies Tower layers and returns `axum::Router`. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tower | 0.5.2 | Service/Layer traits | Already in dep tree via axum 0.8; THE standard middleware abstraction in Rust async ecosystem |
| tower-http | 0.6.8 | SetResponseHeaderLayer, CorsLayer | Already in lockfile (mcp-preview uses cors+fs features); provides battle-tested CORS and header middleware |
| axum | 0.8.5+ | Router, middleware::from_fn, ServiceBuilder | Already primary HTTP framework; 0.8 uses tower 0.5 |
| http | 1.1+ | HeaderValue, HeaderName, StatusCode, Method | Already in deps; provides typed HTTP primitives |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower-layer | 0.3.3 | Layer trait (re-exported by tower) | Already transitive; needed for custom Layer impl |
| tower-service | 0.3.3 | Service trait (re-exported by tower) | Already transitive; needed for custom Service impl |
| pin-project-lite | 0.2.16 | Safe future pinning | Already transitive via axum; needed for custom ResponseFuture |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom DnsRebindingLayer | axum::middleware::from_fn | from_fn is simpler but not composable outside axum; Tower Layer is reusable with any tower::Service and testable in isolation |
| Custom SecurityHeadersLayer | tower-http SetResponseHeaderLayer | SetResponseHeaderLayer handles individual headers; custom layer bundles sensible defaults as a single unit with override API |
| tower-http CorsLayer for all CORS | Custom CORS in DnsRebindingLayer | tower-http CorsLayer is battle-tested; use it for CORS, use custom layer only for Host header DNS rebinding check |

**Installation:**
```toml
# In root Cargo.toml [dependencies]
tower = { version = "0.5", optional = true }
tower-http = { version = "0.6", features = ["cors", "set-header"], optional = true }

# In [features]
streamable-http = ["dep:hyper", "dep:hyper-util", "dep:hyper-rustls", "dep:rustls", "dep:futures-util", "dep:bytes", "dep:axum", "dep:tower", "dep:tower-http"]
```

Note: `tower` is already a transitive dependency via axum. Making it explicit (optional) ensures the SDK can use `tower::Layer`, `tower::Service`, `tower::ServiceBuilder` directly. `tower-http` is already in the lockfile via mcp-preview; adding it to the root crate behind `streamable-http` feature is the right approach.

## Architecture Patterns

### Recommended Project Structure
```
src/
  server/
    axum_router.rs          # NEW: pmcp::axum::router() + AllowedOrigins config
    tower_layers/
      mod.rs                # NEW: pub mod dns_rebinding; pub mod security_headers;
      dns_rebinding.rs      # NEW: DnsRebindingLayer + DnsRebindingService
      security_headers.rs   # NEW: SecurityHeadersLayer + SecurityHeadersService
    streamable_http_server.rs  # MODIFIED: extract Router-building logic, replace add_cors_headers
    http_middleware.rs          # UNCHANGED: ServerHttpMiddleware trait preserved
    mod.rs                      # MODIFIED: add pub mod tower_layers; conditional pub mod axum_router;
```

### Pattern 1: Tower Layer + Service Pair (Custom Middleware)
**What:** Each middleware is a Layer struct (configuration) and Service struct (execution). The Layer creates the Service by wrapping an inner service.
**When to use:** For DnsRebindingLayer and SecurityHeadersLayer -- they need to short-circuit requests (403) or modify responses (add headers).

```rust
// DnsRebindingLayer -- the configuration/factory
#[derive(Debug, Clone)]
pub struct DnsRebindingLayer {
    allowed_origins: AllowedOrigins,
}

impl<S> tower::Layer<S> for DnsRebindingLayer {
    type Service = DnsRebindingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DnsRebindingService {
            inner,
            allowed_origins: self.allowed_origins.clone(),
        }
    }
}

// DnsRebindingService -- the runtime service
#[derive(Debug, Clone)]
pub struct DnsRebindingService<S> {
    inner: S,
    allowed_origins: AllowedOrigins,
}

impl<S, B> tower::Service<http::Request<B>> for DnsRebindingService<S>
where
    S: tower::Service<http::Request<B>, Response = axum::response::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        // Validate Host and Origin headers BEFORE forwarding
        if !self.allowed_origins.is_allowed_host(req.headers()) {
            return Box::pin(async { Ok(forbidden_response("Host header not in allowed origins")) });
        }
        if let Some(origin) = req.headers().get(http::header::ORIGIN) {
            if !self.allowed_origins.is_allowed_origin(origin) {
                return Box::pin(async { Ok(forbidden_response("Origin not in allowed origins")) });
            }
        }
        let future = self.inner.call(req);
        Box::pin(future)
    }
}
```

### Pattern 2: AllowedOrigins -- Single Source of Truth
**What:** One config struct drives Host validation, Origin validation, and CORS allow-origin. Auto-detects localhost aliases when binding to 127.0.0.1 or [::1].
**When to use:** Configuration for both DnsRebindingLayer and CorsLayer.

```rust
#[derive(Debug, Clone)]
pub struct AllowedOrigins {
    /// Explicit list of allowed origins (e.g., "http://localhost:3000")
    origins: Vec<String>,
    /// Allowed hostnames extracted from origins (e.g., "localhost", "127.0.0.1")
    hostnames: HashSet<String>,
}

impl AllowedOrigins {
    /// Auto-detect from bind address.
    /// localhost:port -> allow "localhost", "127.0.0.1", "[::1]", "localhost:port", "127.0.0.1:port"
    pub fn from_bind_addr(addr: SocketAddr) -> Self { /* ... */ }

    /// Explicit allow-list for deployed servers.
    pub fn explicit(origins: Vec<String>) -> Self { /* ... */ }

    /// Check if a Host header value is allowed.
    pub fn is_allowed_host(&self, headers: &HeaderMap) -> bool { /* ... */ }

    /// Check if an Origin header value is allowed.
    pub fn is_allowed_origin(&self, origin: &HeaderValue) -> bool { /* ... */ }

    /// Convert to tower-http AllowOrigin for CorsLayer.
    pub fn to_cors_allow_origin(&self) -> tower_http::cors::AllowOrigin { /* ... */ }
}
```

### Pattern 3: Axum Router Convenience Function
**What:** `pmcp::axum::router()` builds MCP routes with security layers applied, returns `axum::Router`.
**When to use:** The 90% case for users who want a secure MCP server in one function call.

```rust
// src/server/axum_router.rs
pub fn router(server: Arc<tokio::sync::Mutex<Server>>) -> axum::Router {
    router_with_config(server, RouterConfig::default())
}

pub fn router_with_config(
    server: Arc<tokio::sync::Mutex<Server>>,
    config: RouterConfig,
) -> axum::Router {
    let state = ServerState { /* ... */ };
    let allowed = config.allowed_origins.unwrap_or_else(|| AllowedOrigins::localhost());

    Router::new()
        .route("/", post(handle_post_request))
        .route("/", get(handle_get_sse))
        .route("/", delete(handle_delete_session))
        .route("/", axum::routing::options(handle_options))
        .with_state(state)
        .layer(SecurityHeadersLayer::default())
        .layer(DnsRebindingLayer::new(allowed.clone()))
        .layer(CorsLayer::new()
            .allow_origin(allowed.to_cors_allow_origin())
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([/* MCP headers */])
            .expose_headers([/* MCP headers */])
            .max_age(Duration::from_secs(86400)))
}
```

### Pattern 4: Extracting Router Construction from StreamableHttpServer
**What:** The current `StreamableHttpServer::start()` builds the Router and binds a listener. The Router-building logic must be extracted into a shared function so both `StreamableHttpServer::start()` and `pmcp::axum::router()` can reuse it.
**When to use:** During refactoring of streamable_http_server.rs.

```rust
// Internal helper (not public API)
fn build_mcp_router(state: ServerState) -> Router<()> {
    Router::new()
        .route("/", post(handle_post_request))
        .route("/", get(handle_get_sse))
        .route("/", delete(handle_delete_session))
        .route("/", axum::routing::options(handle_options))
        .with_state(state)
}
```

### Anti-Patterns to Avoid
- **Modifying ServerHttpMiddleware for Tower integration:** Tower Layers wrap OUTSIDE. Never merge the two systems. They serve different purposes (Tower is transport-level, ServerHttpMiddleware is application-level).
- **Using `Access-Control-Allow-Origin: *` anywhere:** This is the CVE. Always use origin-locked CORS.
- **Port-matching in Host validation:** The Host header may or may not include port. Match hostname only, not hostname:port. Port-matching would break behind reverse proxies.
- **Blocking requests with no Origin header:** Non-browser clients (curl, CLI tools) do not send Origin headers. Only validate Origin when it is PRESENT. Always validate Host.
- **Making DnsRebindingLayer depend on Axum types:** The Layer should work with any `http::Request<B>`, not just Axum's Body type. This keeps it reusable.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CORS response headers | Custom CORS header insertion | `tower_http::cors::CorsLayer` | CORS has 7+ headers, preflight caching, credential modes, and browser-specific edge cases. tower-http handles all of them. |
| Individual response headers | Per-handler header insertion | `tower_http::set_header::SetResponseHeaderLayer` | Centralized, composable, no per-route duplication |
| Service composition | Manual nesting of service wrappers | `tower::ServiceBuilder` | Correct ordering, type inference, composable |
| Future pinning in middleware | Manual Pin<Box<dyn Future>> everywhere | `pin-project-lite` | Safe projection without unsafe code |

**Key insight:** tower-http 0.6.8 already provides `CorsLayer` with `AllowOrigin::list()` and `AllowOrigin::predicate()` for origin-locked CORS. Use it directly instead of reimplementing CORS. Only the Host-header DNS rebinding check needs a custom Layer.

## Common Pitfalls

### Pitfall 1: Origin Header Absence vs. Invalid Origin
**What goes wrong:** Middleware rejects all requests without an Origin header, breaking CLI clients (curl, mcp-tester) that never send Origin.
**Why it happens:** Conflating "browser-based CSRF protection" with "all request validation."
**How to avoid:** Only reject requests where Origin IS present but NOT in the allow-list. Missing Origin is acceptable -- DNS rebinding attacks come FROM browsers, which ALWAYS send Origin on cross-origin requests.
**Warning signs:** mcp-tester tests fail with 403 when connecting to a local server.

### Pitfall 2: Host Header Port Mismatch Behind Reverse Proxy
**What goes wrong:** Server binds to port 8080, reverse proxy forwards on port 443. Host header says `example.com` (no port) or `example.com:443`. Validation fails because the allow-list has `localhost:8080`.
**Why it happens:** Matching full host:port instead of just hostname.
**How to avoid:** Extract hostname from Host header (strip port), match against allowed hostnames. For localhost auto-detection, allow bare hostnames `localhost`, `127.0.0.1`, `[::1]` regardless of port.
**Warning signs:** Server works locally but fails behind nginx/cloudflare.

### Pitfall 3: CORS Preflight (OPTIONS) Blocked by DNS Rebinding Layer
**What goes wrong:** Browsers send OPTIONS preflight requests before cross-origin POST. If the DNS rebinding layer runs before CORS handling, and OPTIONS requests lack proper headers, they get 403.
**Why it happens:** Layer ordering is wrong -- DNS rebinding layer runs before CORS.
**How to avoid:** In `Router::layer()`, layers execute bottom-to-top for requests. Stack CorsLayer BELOW DnsRebindingLayer so CORS runs first on the request path. Actually, the correct approach: CorsLayer should be the outermost layer so it handles OPTIONS preflight BEFORE DnsRebindingLayer checks headers. In axum `.layer()` calls, the LAST `.layer()` added runs FIRST on requests.
**Warning signs:** Browser preflight requests fail with 403; non-browser clients work fine.

### Pitfall 4: Breaking the Existing add_cors_headers Function
**What goes wrong:** Removing `add_cors_headers()` from streamable_http_server.rs breaks the `StreamableHttpServer` path (used by users who don't use `pmcp::axum::router()`).
**Why it happens:** D-02 says Tower layers are only default via `router()`. StreamableHttpServer users get no layers unless they opt in.
**How to avoid:** Keep `add_cors_headers()` for the StreamableHttpServer path but update it to use AllowedOrigins-aware CORS (not wildcard `*`). Or add an `allowed_origins` field to `StreamableHttpServerConfig` that drives the existing `add_cors_headers()` function.
**Warning signs:** Example 22 (stateful HTTP server) and Example 23 (stateless) break.

### Pitfall 5: tower-http CorsLayer vs. PMCP Custom CORS
**What goes wrong:** Having TWO CORS systems (tower-http CorsLayer on the Tower layer AND add_cors_headers in handler code) results in duplicate/conflicting CORS headers.
**Why it happens:** Not cleaning up the old system when adding the new one.
**How to avoid:** For the `router()` path, the Tower CorsLayer handles ALL CORS. For the `StreamableHttpServer` path, update `add_cors_headers()` to respect AllowedOrigins. Never have both active simultaneously.
**Warning signs:** Browsers receive duplicate `Access-Control-Allow-Origin` headers.

### Pitfall 6: Layer Ordering with ServiceBuilder
**What goes wrong:** Applying layers in the wrong order via `ServiceBuilder::new().layer(A).layer(B)` -- ServiceBuilder applies top-to-bottom (A wraps B), meaning A runs FIRST for requests.
**Why it happens:** Confusion between `Router::layer()` (bottom-to-top) and `ServiceBuilder` (top-to-bottom).
**How to avoid:** Use `ServiceBuilder` for clarity. CorsLayer first (outermost), then DnsRebindingLayer, then SecurityHeadersLayer (innermost, closest to handler).
**Warning signs:** Tests pass individually but integration tests show wrong execution order.

## Code Examples

### Example 1: AllowedOrigins from Bind Address
```rust
// Source: Design based on CVE-2025-66414 fix pattern + Rails HostAuthorization
use std::collections::HashSet;
use std::net::SocketAddr;

impl AllowedOrigins {
    pub fn from_bind_addr(addr: SocketAddr) -> Self {
        let ip = addr.ip();
        let port = addr.port();
        let mut origins = Vec::new();
        let mut hostnames = HashSet::new();

        if ip.is_loopback() || ip.is_unspecified() {
            // Localhost aliases
            for host in &["localhost", "127.0.0.1", "[::1]"] {
                hostnames.insert(host.to_string());
                origins.push(format!("http://{}:{}", host, port));
            }
        } else {
            let host = ip.to_string();
            hostnames.insert(host.clone());
            origins.push(format!("http://{}:{}", host, port));
        }

        Self { origins, hostnames }
    }

    pub fn localhost() -> Self {
        Self::from_bind_addr(SocketAddr::from(([127, 0, 0, 1], 0)))
    }
}
```

### Example 2: Host Header Validation
```rust
// Source: MCP spec 2025-03-26 Security Warning + Rails HostAuthorization pattern
pub fn is_allowed_host(&self, headers: &http::HeaderMap) -> bool {
    let host = match headers.get(http::header::HOST) {
        Some(h) => match h.to_str() {
            Ok(s) => s,
            Err(_) => return false, // Non-ASCII host header
        },
        None => return false, // Missing Host header -- reject
    };

    // Strip port from Host header for matching
    let hostname = host.split(':').next().unwrap_or(host);
    self.hostnames.contains(hostname)
}
```

### Example 3: User-Facing Router API
```rust
// Source: Design per D-08, D-09
use pmcp::server::axum_router::{router, RouterConfig, AllowedOrigins};
use pmcp::Server;

#[tokio::main]
async fn main() {
    let server = Server::builder()
        .name("my-server")
        .version("1.0.0")
        .tool("echo", EchoTool)
        .build()
        .unwrap();

    let server = Arc::new(tokio::sync::Mutex::new(server));

    // One-liner: secure MCP server with localhost DNS rebinding protection
    let app = router(server);

    // Or with explicit config for deployed servers
    let app = router_with_config(server, RouterConfig {
        allowed_origins: Some(AllowedOrigins::explicit(vec![
            "https://myapp.example.com".to_string(),
        ])),
        ..Default::default()
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### Example 4: DnsRebindingLayer Tower Pattern
```rust
// Source: tower-rs/tower guide "building-a-middleware-from-scratch"
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

impl<S, B> tower::Service<http::Request<B>> for DnsRebindingService<S>
where
    S: tower::Service<http::Request<B>, Response = http::Response<axum::body::Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        // Check Host header
        if !self.allowed_origins.is_allowed_host(req.headers()) {
            tracing::warn!(
                host = ?req.headers().get(http::header::HOST),
                "DNS rebinding protection: rejected request with disallowed Host header"
            );
            return Box::pin(async {
                Ok(http::Response::builder()
                    .status(http::StatusCode::FORBIDDEN)
                    .body(axum::body::Body::from("Forbidden: Host header not in allowed origins"))
                    .unwrap())
            });
        }

        // Check Origin header (only when present -- non-browser clients may omit it)
        if let Some(origin) = req.headers().get(http::header::ORIGIN) {
            if !self.allowed_origins.is_allowed_origin(origin) {
                tracing::warn!(
                    origin = ?origin,
                    "DNS rebinding protection: rejected request with disallowed Origin header"
                );
                return Box::pin(async {
                    Ok(http::Response::builder()
                        .status(http::StatusCode::FORBIDDEN)
                        .body(axum::body::Body::from("Forbidden: Origin not in allowed origins"))
                        .unwrap())
                });
            }
        }

        // Clone inner service (required for Tower Service pattern)
        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}
```

### Example 5: SecurityHeadersLayer
```rust
// Source: OWASP Security Headers best practices
use tower_http::set_header::SetResponseHeaderLayer;
use http::HeaderValue;

/// Bundles security response headers as a single Tower Layer.
#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer {
    pub x_content_type_options: bool,  // default: true (nosniff)
    pub x_frame_options: bool,         // default: true (DENY)
    pub cache_control: bool,           // default: true (no-store)
}

// Implementation applies via ServiceBuilder composition of SetResponseHeaderLayer instances
// or via a custom Service that adds headers in on_response.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Access-Control-Allow-Origin: *` | Origin-locked CORS via allow-list | CVE-2025-66414 (2025) | Prevents DNS rebinding; spec requirement since MCP 2025-03-26 |
| No Host header validation | MUST validate Host header | MCP spec 2025-03-26 | Spec compliance for all HTTP MCP servers |
| Custom CORS in handler code | tower-http CorsLayer | tower-http 0.6 | Battle-tested, composable, handles preflight correctly |
| axum 0.7 + tower 0.4 | axum 0.8 + tower 0.5 | January 2025 | Current project already on axum 0.8.5 + tower 0.5.2 |

**Deprecated/outdated:**
- `Access-Control-Allow-Origin: *` on MCP servers: Security vulnerability per CVE-2025-66414/66416. Must be replaced.
- Manual CORS header insertion in handler functions: Should be handled by middleware layer for consistency.

## Open Questions

1. **Error response format for 403**
   - What we know: D-05 says "403 Forbidden with explanation body." The MCP spec doesn't mandate a specific error format for transport-level rejections.
   - What's unclear: Should it be plain text (`"Forbidden: Host header not in allowed origins"`) or JSON-RPC error format (`{"jsonrpc":"2.0","error":{"code":-32600,"message":"..."},"id":null}`)?
   - Recommendation: Use plain text. DNS rebinding rejection happens BEFORE JSON-RPC parsing. The client may not even be a valid MCP client. Plain text is simpler, universal, and matches Rails/Express patterns.

2. **Feature flag for tower-http dependency**
   - What we know: tower-http is already in lockfile via mcp-preview. Adding it to root crate behind `streamable-http` feature is natural.
   - What's unclear: Should `tower` and `tower-http` be explicit deps or should we rely on axum re-exports?
   - Recommendation: Make them explicit optional deps gated behind `streamable-http`. Axum re-exports are unstable across versions. Explicit deps give version control.

3. **Module path: `pmcp::axum::router()` vs `pmcp::server::axum_router::router()`**
   - What we know: D-08 says `pmcp::axum::router()`. But `axum` is already a crate name, and having `pmcp::axum` as a module could be confusing.
   - What's unclear: Exact module path in the crate hierarchy.
   - Recommendation: Use `pmcp::server::axum_router::router()` internally, but add a `pub use` re-export at `pmcp::axum` (as a feature-gated module in lib.rs). This gives the clean API D-08 wants without namespace confusion.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio::test + proptest 1.7 |
| Config file | Cargo.toml `[dev-dependencies]` section |
| Quick run command | `cargo test --features streamable-http -p pmcp -- tower_layers` |
| Full suite command | `cargo test --features full -p pmcp` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DNS-REBINDING-01 | Reject request with disallowed Host header | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::reject_bad_host -x` | Wave 0 |
| DNS-REBINDING-02 | Accept request with allowed Host header | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::accept_good_host -x` | Wave 0 |
| DNS-REBINDING-03 | Reject request with present but disallowed Origin | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::reject_bad_origin -x` | Wave 0 |
| DNS-REBINDING-04 | Accept request with missing Origin (non-browser) | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::accept_missing_origin -x` | Wave 0 |
| DNS-REBINDING-05 | AllowedOrigins auto-detect from localhost bind | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::auto_detect_localhost -x` | Wave 0 |
| DNS-REBINDING-06 | AllowedOrigins explicit list | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::explicit_origins -x` | Wave 0 |
| SECURITY-HEADERS-01 | Response includes X-Content-Type-Options: nosniff | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::security_headers::tests -x` | Wave 0 |
| CORS-01 | CORS allows origin from allow-list, rejects others | unit | `cargo test --features streamable-http -p pmcp -- tower_layers::dns_rebinding::tests::cors_origin_locked -x` | Wave 0 |
| AXUM-ROUTER-01 | router() returns working Router with security layers | integration | `cargo test --features streamable-http -p pmcp -- axum_router::tests -x` | Wave 0 |
| AXUM-ROUTER-02 | StreamableHttpServer path still works (no regression) | integration | `cargo test --features streamable-http -p pmcp -- streamable_http_server_tests -x` | Exists: tests/streamable_http_server_tests.rs |
| COMPAT-01 | Example 55 (ServerHttpMiddleware) still compiles and runs | smoke | `cargo build --example 55_server_middleware --features streamable-http` | Exists: examples/55_server_middleware.rs |

### Sampling Rate
- **Per task commit:** `cargo test --features streamable-http -p pmcp -- tower_layers`
- **Per wave merge:** `cargo test --features full -p pmcp`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/server/tower_layers/dns_rebinding.rs` -- unit tests for Host/Origin validation (inline #[cfg(test)] mod)
- [ ] `src/server/tower_layers/security_headers.rs` -- unit tests for response headers (inline #[cfg(test)] mod)
- [ ] `src/server/axum_router.rs` -- integration tests for router() function (inline #[cfg(test)] mod)
- [ ] `tests/dns_rebinding_integration.rs` -- end-to-end test: start server via router(), connect client, verify 403 on bad Host

## Discretion Recommendations

Based on research, here are recommendations for areas left to Claude's discretion:

1. **Expose individual layers publicly.** Both `DnsRebindingLayer` and `SecurityHeadersLayer` should be `pub` in `pmcp::server::tower_layers`. Power users who bring their own Axum Router need these. The `router()` function is convenience, not the only path.

2. **Add `allowed_origins` to `StreamableHttpServerConfig`.** This is the integration point for users who use `StreamableHttpServer` directly. Type: `Option<AllowedOrigins>`. When `Some`, it drives the existing `add_cors_headers()` function (replacing `*` with the specific origin). When `None`, default to localhost auto-detect (matching D-04).

3. **Localhost auto-detection: hostname only, no port matching.** Match `localhost`, `127.0.0.1`, `[::1]` as hostnames. Do not include port in the comparison. Reason: Host headers behind proxies often strip port. Port matching would cause false rejections.

4. **Error response: plain text for 403.** DNS rebinding rejection is transport-level, before JSON-RPC parsing. Use `"Forbidden: Host header not in allowed origins\n"` as the body. StatusCode::FORBIDDEN (403). Content-Type: text/plain.

## Sources

### Primary (HIGH confidence)
- MCP Specification 2025-03-26 Transports section -- Security Warning for Streamable HTTP: "Servers MUST validate the Origin header on all incoming connections to prevent DNS rebinding attacks" (https://modelcontextprotocol.io/specification/2025-03-26/basic/transports)
- CVE-2025-66414 -- MCP TypeScript SDK DNS rebinding vulnerability and fix pattern (https://github.com/advisories/GHSA-w48q-cv73-mx4w)
- tower-rs/tower guide "building-a-middleware-from-scratch" -- canonical Layer+Service+Future pattern (https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md)
- tower-http 0.6.8 docs -- CorsLayer, SetResponseHeaderLayer, AllowOrigin API (https://docs.rs/tower-http/0.6.2/tower_http/)
- axum 0.8 middleware docs -- Router::layer(), ServiceBuilder integration (https://docs.rs/axum/latest/axum/middleware/index.html)
- Existing codebase: `streamable_http_server.rs:1366-1382` -- current `add_cors_headers()` with wildcard `*`
- Existing codebase: `http_middleware.rs:184` -- ServerHttpMiddleware trait definition
- Existing codebase: `Cargo.toml:84` -- axum 0.8.5; `Cargo.lock` -- tower 0.5.2, tower-http 0.6.8

### Secondary (MEDIUM confidence)
- Axum 0.8.0 announcement -- tower 0.5 compatibility confirmed (https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0)
- GitHub blog on DNS rebinding attacks -- attack vector explanation (https://github.blog/security/application-security/dns-rebinding-attacks-explained-the-lookup-is-coming-from-inside-the-house/)
- Rails HostAuthorization middleware -- prior art for Host header validation (https://github.com/rails/rails/commit/07ec8062e605ba4e9bd153e1d264b02ac4ab8a0f)
- brannondorsey/host-validation npm -- Express.js prior art (https://github.com/brannondorsey/host-validation)

### Tertiary (LOW confidence)
- None. All findings verified against primary or secondary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- tower 0.5.2, tower-http 0.6.8, axum 0.8 all already in dependency tree. Versions verified from Cargo.lock.
- Architecture: HIGH -- Tower Layer+Service pattern is canonical and well-documented. Code patterns verified against official tower guide.
- Pitfalls: HIGH -- CVE-2025-66414 provides real-world evidence of the exact vulnerability being addressed. Host/Origin validation pitfalls documented in multiple framework implementations.

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable ecosystem; tower/axum versions unlikely to change within 30 days)
