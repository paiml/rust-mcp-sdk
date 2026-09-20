# Phase 112 — Public-API Additive-Guarantee Baseline (Plan 01)

This is the "what-would-force-3.0" reference the phase-end semver gate (Plan 07/08,
wave 5) compares against. The **authoritative** `cargo semver-checks check-release`
MINOR assertion runs over the FULL phase diff at phase end — this file is the Plan-01
foundation-types reference only.

## Pinned tooling (record for CI reuse)

| Tool | Pinned version | Install |
|------|----------------|---------|
| `cargo-semver-checks` | `0.49.0` | `cargo install cargo-semver-checks --version 0.49.0 --locked` |
| `cargo-public-api` | `0.52.0` | `cargo install cargo-public-api --version 0.52.0 --locked` |

## Baseline

- **Semver comparison baseline:** published `pmcp 2.17.0` on crates.io.
- **Surface snapshot method:** `cargo public-api --no-default-features -p pmcp`
  (the full default-feature rustdoc-JSON build OOM-killed `rustdoc` with SIGKILL on
  this machine; the `--no-default-features` surface is captured here as the reference).
- **Total public items in `--no-default-features` surface:** 42,702.

## Phase-112 Plan-01 additive delta (all NEW `pub`, no removals/modifications → MINOR)

```
pub enum pmcp::types::protocol::Era
pub pmcp::types::protocol::Era::V1
pub pmcp::types::protocol::Era::V2
#[non_exhaustive] pub struct pmcp::types::protocol::ProtocolContext
pub pmcp::types::protocol::ProtocolContext::era: pmcp::types::Era
pub pmcp::types::protocol::ProtocolContext::negotiated_version: pmcp::types::ProtocolVersion
pub pmcp::types::protocol::ProtocolContext::client_info: core::option::Option<pmcp::types::Implementation>
pub pmcp::types::protocol::ProtocolContext::client_capabilities: core::option::Option<pmcp::types::capabilities::ClientCapabilities>
#[non_exhaustive] pub struct pmcp::types::protocol::TraceContext
pub pmcp::types::protocol::TraceContext::traceparent: alloc::string::String
pub pmcp::types::protocol::TraceContext::tracestate: core::option::Option<alloc::string::String>
pub pmcp::types::protocol::TraceContext::baggage: core::option::Option<alloc::string::String>
pub const pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28: &str
pub fn pmcp::types::protocol::protocol_era(&str) -> pmcp::types::Era
```

Every item above is a **new** public symbol. No public symbol was removed or had its
signature changed, so this delta classifies as an additive **minor (2.x)** change.

## DX note (pre-existing name collision, NOT a semver break)

A `TraceContext` already exists at `pmcp::server::observability::TraceContext` — a
DIFFERENT concept (internal span tracking: `trace_id`/`span_id`/`parent_span_id`/`depth`).
The new `pmcp::types::protocol::TraceContext` is a distinct W3C-header passthrough type
(`traceparent`/`tracestate`/`baggage`) in a distinct module, named per the Plan-01 spec.
The two never collide at a path level, but downstream glob-imports of both modules would
need to disambiguate. Flagged for the phase verifier; no action required in Plan 01.
