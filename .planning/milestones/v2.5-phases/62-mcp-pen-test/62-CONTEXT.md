# Phase 62: MCP Pen Test - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Automated penetration testing for MCP server endpoints via `cargo pmcp pentest`. Extends the existing test/loadtest CLI pattern to security testing. Focuses on MCP-specific attacks first (prompt injection, tool poisoning, session security), with transport and general web attacks as follow-up. Developers can test their MCP endpoint automatically against MCP attacks and general cyber attacks.

</domain>

<decisions>
## Implementation Decisions

### Attack Categories
- **D-01:** MCP-specific attacks are the primary focus. Transport-level and general web attacks are future phases.
- **D-02:** Prompt injection testing uses a curated payload library plus fuzzing of tool arguments. Report if server echoes injected content or changes behavior.
- **D-03:** Tool poisoning detection: inspect tool responses for unexpected _meta keys, script injection in resourceUri, oversized payloads, and keys that don't match declared outputSchema.
- **D-04:** Full session security tests: session ID entropy, replay attacks, fixation, concurrent sessions, timeout enforcement. Reuses conformance engine's session tracking.

### CLI Integration
- **D-05:** New top-level subcommand: `cargo pmcp pentest <url>`. Separate from test (conformance) and loadtest (performance).
- **D-06:** Target specified as URL argument, consistent with test/loadtest. Reuses ServerFlags.
- **D-07:** Reuses AuthFlags from Phase 29 for testing authenticated endpoints. Same --api-key, --oauth-client-id, --bearer flags.

### Reporting & Output
- **D-08:** 5-level severity classification: Critical / High / Medium / Low / Info (OWASP-style). Each test has a predefined severity.
- **D-09:** Three output formats: terminal (rich colors/severity), JSON (programmatic/CI), SARIF (GitHub Security tab integration).
- **D-10:** CI pass/fail threshold via `--fail-on` flag. Default: fail on Critical or High. Configurable (e.g., `--fail-on medium`).

### Scope & Safety
- **D-11:** Explicit target only — tests only the URL provided. Built-in rate limiting (default 10 req/s, configurable via `--rate-limit`). No following redirects to other hosts. Clear banner before testing.
- **D-12:** Non-destructive by default. `--destructive` flag enables mutation-based tests (malicious tool call args, resource mutations). Clear warning when enabled.
- **D-13:** Auto-discover attack surface from MCP (tools/list, resources/list), then test each discovered endpoint. Mirrors real attacker behavior.

### Claude's Discretion
- Internal architecture: how to structure the pentest engine (test runner, payload library, result collection)
- Payload library format and content (curated injection patterns)
- SARIF schema mapping details
- Rate limiter implementation (token bucket, leaky bucket, etc.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Test Infrastructure
- `crates/mcp-tester/src/report.rs` — TestCategory enum, TestReport, TestResult, TestStatus (reuse for security findings)
- `crates/mcp-tester/src/diagnostics.rs` — Conformance test runner pattern (model for pen test runner)
- `cargo-pmcp/src/commands/test/conformance.rs` — CLI integration pattern for test subcommands

### Security Middleware (attack surface reference)
- `src/server/tower_layers/dns_rebinding.rs` — AllowedOrigins, DnsRebindingLayer (tests should verify this works)
- `src/types/auth.rs` — Auth types to understand attack vectors

### CLI Patterns
- `cargo-pmcp/src/commands/test/mod.rs` — Test subcommand structure (model for pentest subcommand)
- `cargo-pmcp/src/loadtest/` — Load test engine pattern (engine, metrics, report, config)
- `cargo-pmcp/src/commands/flags.rs` — AuthFlags, ServerFlags (reuse for pentest)

### Protocol Types
- `src/types/protocol/` — MCP protocol types (tools, resources, sessions) that define the attack surface

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `mcp-tester` crate: `ServerTester` for MCP session management, `TestReport`/`TestResult` for structured results
- `AuthFlags`/`AuthMethod` in `cargo-pmcp/src/commands/flags.rs` for authenticated testing
- `McpProxy` pattern from mcp-preview for raw HTTP request construction
- Conformance `TestCategory` enum — extend with `Security` variant or create parallel `SecurityCategory`

### Established Patterns
- Test subcommands follow `cargo-pmcp/src/commands/test/*.rs` pattern with per-file modules
- Load test uses engine/config/metrics/report separation — good model for pentest architecture
- `colored` crate for terminal output with severity coloring

### Integration Points
- New `cargo-pmcp/src/commands/pentest/` module alongside existing test/loadtest
- Register in `cargo-pmcp/src/commands/mod.rs` command dispatch
- Reuse `ServerTester` from `mcp-tester` for MCP session initialization and tool/resource discovery

</code_context>

<specifics>
## Specific Ideas

- Auto-discovery mirrors real attacker behavior: call tools/list and resources/list to map the attack surface before testing
- Payload library should include known MCP-specific attack patterns (prompt injection through tool arguments, delimiter confusion, instruction override)
- SARIF output enables GitHub Security tab integration for CI pipelines
- Rate limiting prevents accidentally DoS-ing the target during security testing

</specifics>

<deferred>
## Deferred Ideas

- Transport-level attacks (CORS bypass, TLS validation, header injection) — separate phase
- General web attacks (SSRF, path traversal) — separate phase
- Config file for multiple targets / recurring scans — CI workflow enhancement
- Pentest-specific auth flags (--stolen-token, --expired-token) — advanced auth attack scenarios
- Dual mode testing (authenticated + unauthenticated side-by-side) — advanced feature

</deferred>

---

*Phase: 62-mcp-pen-test*
*Context gathered: 2026-03-28*
