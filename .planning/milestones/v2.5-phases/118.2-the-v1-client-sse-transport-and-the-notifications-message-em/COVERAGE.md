# Phase 118.2 — API coverage declaration

No external API integration: this phase fixes pmcp's own client SSE transport
(`src/shared/streamable_http.rs`) and its response-correlation code (`src/client/mod.rs`), plus the v1
server-side `notifications/message` emitter — the MCP protocol is *implemented* here rather than
consumed as a third-party API, so there is no vendor endpoint, SDK or service surface to cover.

## Why the detector fired, and why the answer is a declaration rather than a matrix

The deterministic api-coverage detector returned `detected: true` on the terms "wire/MCP" and "API
surface". Both terms appear throughout this phase's planning record for the same reason: pmcp *is* an
MCP implementation. "Wire" refers to the JSON-RPC frames pmcp itself encodes and decodes; "API
surface" refers to pmcp's **own** public Rust API and the `cargo semver-checks` verdict the phase's
Success Criterion 3 demands of it. Neither term denotes an outbound integration.

Checked against the phase scope before writing this, not after. `.planning/ROADMAP.md`'s Phase 118.2
entry and `118.2-CONTEXT.md`'s Phase Boundary both scope the work to two missing surfaces at opposite
ends of pmcp's own server-to-client channel: the client-side live SSE read, and a handler-facing log
emitter. The gap closure's four in-scope defects (CR-01, CR-02, WR-01, WR-02) are all in that same
client code — a reconnect bound, a request/response correlation check, a reader shutdown signal and a
per-stream resumption cursor. None of them touches an outbound integration with any third-party
service.

The only network peer involved anywhere in this phase is an MCP server, which is the protocol
counterparty rather than an integrated API — and in this phase's tests that peer is a local recording
TCP listener in `tests/client_sse_stream.rs`, not a hosted service. The conformance measurement runs
against the official MCP validator suite, pinned by version, which is a test harness rather than a
consumed API.

No coverage matrix, endpoint table or per-method checklist appears in this document, deliberately. In
the no-integration case the capability contract asks for a reasoned declaration; a fabricated matrix
would be worse than an empty file, because a later reviewer would read it as evidence that endpoints
were enumerated and verified when none exist to enumerate.
