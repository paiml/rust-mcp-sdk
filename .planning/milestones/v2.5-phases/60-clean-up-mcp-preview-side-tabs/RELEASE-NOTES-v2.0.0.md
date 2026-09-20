# PMCP v2.0.0 — Aligned with MCP TypeScript SDK v2.0

We're excited to announce **PMCP v2.0** — the Rust implementation of the Model Context Protocol, now fully aligned with the MCP protocol v2025-03-26 and the TypeScript SDK v2.0 release.

PMCP v2.0 brings major new capabilities: **MCP Apps** for building rich interactive UIs served from MCP servers, **MCP Tasks** for shared client/server state management, a **conformance test suite** for protocol compliance, and a **production-grade HTTP security stack** with Tower middleware.

## Highlights

### MCP Apps — Rich Interactive UIs

Build HTML-based interactive widgets served directly from MCP servers. MCP Apps work with ChatGPT, Claude Desktop, and other MCP clients that support the Apps protocol.

- **Live Preview**: Browser-based development with the `mcp-preview` tool — resizable DevTools panel with Network, Events, Protocol, and Bridge tabs
- **Widget Authoring**: File-based `widgets/` directory with hot-reload, or scaffold with `cargo pmcp app new`
- **Publishing**: ChatGPT-compatible manifests, standalone demo landing pages, and production bundles
- **Examples**: Chess analyzer, interactive map explorer, data visualization dashboard

### MCP Tasks — Shared State (Experimental)

A new `pmcp-tasks` crate introduces task lifecycle management with shared client/server state:

- Create, update, complete, and cancel tasks with DynamoDB or in-memory backends
- Task variables enable shared state between client and server
- Designed for agent orchestration and multi-step workflows

### Conformance Test Suite

Validate MCP server implementations against the protocol specification:

- **19 test scenarios** across 5 domains: initialize, tools, resources, prompts, notifications
- `cargo pmcp test conformance` with `--strict` and `--domain` flags
- `mcp-tester conformance` for CI integration with per-domain summary

### Production-Ready HTTP Security

New Tower middleware stack for production deployments:

- **DNS Rebinding Protection**: Configurable allowed origins with localhost defaults
- **CORS**: Origin-locked headers (no wildcard `*` in production)
- **Security Headers**: Configurable layer for CSP, HSTS, X-Frame-Options, etc.
- **`AllowedOrigins`**: Simple API — `localhost()`, `any()`, or custom list

### Developer Experience

- **Uniform Constructor DX**: `Default`, builders, and constructors for all protocol types — no more field-by-field struct initialization
- **Protocol Compatibility**: Accepts both `2025-03-26` and `2024-11-05` protocol versions for backward compatibility
- **PMCP Server**: MCP server exposing SDK developer tools via Streamable HTTP — deployed on AWS Lambda

## What's in the Box

| Crate | Version | What it does |
|-------|---------|-------------|
| `pmcp` | 2.0.0 | Core SDK — types, transports, server builder, Tower middleware |
| `mcp-tester` | 0.4.0 | Protocol compliance testing and conformance suite |
| `mcp-preview` | 0.3.0 | MCP Apps live preview with browser DevTools |
| `cargo-pmcp` | 0.5.0 | CLI toolkit — scaffold, test, deploy, preview, conformance |
| `pmcp-macros` | 0.3.0 | Proc macros for `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]` |
| `pmcp-widget-utils` | 0.2.0 | Shared bridge injection for MCP Apps |
| `pmcp-tasks` | 0.1.0 | Experimental task lifecycle management |
| `pmcp-server` | 0.2.0 | MCP server with SDK developer tools |

## Performance

PMCP continues to deliver **16x faster** performance than the TypeScript SDK with **50x lower memory** usage. SIMD-accelerated SSE parsing achieves 336,921 events/sec.

## Getting Started

```toml
[dependencies]
pmcp = "2.0"
tokio = { version = "1", features = ["full"] }
```

- **Documentation**: https://paiml.github.io/rust-mcp-sdk/
- **API Reference**: https://docs.rs/pmcp
- **Course**: https://paiml.github.io/rust-mcp-sdk/course/
- **Examples**: 60+ working examples in the `examples/` directory

## Breaking Changes

- `StreamableHttpServerConfig` and `RouterConfig` now include an `allowed_origins` field
- Protocol version defaults to `2025-03-26` (older clients using `2024-11-05` are still accepted)

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for the complete list of changes.

---

**Thank you** to all contributors and early adopters who helped shape PMCP v2.0. Built with Rust, following Toyota Way quality principles.
