# Phase 57: Conformance Test Suite - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `mcp-tester conformance <url>` command that validates any MCP server against the MCP protocol spec (2025-11-25). Replaces the existing `compliance` subcommand with a spec-aligned conformance command. Core scenarios: initialize handshake, tools CRUD, resources CRUD, prompts CRUD, task lifecycle. Modeled after TypeScript SDK's `@modelcontextprotocol/conformance` infrastructure.

</domain>

<decisions>
## Implementation Decisions

### Conformance replaces compliance
- **D-01:** The existing `compliance` subcommand is replaced by the new `conformance` subcommand — not a separate coexisting command
- **D-02:** All existing compliance checks (required methods, error codes, JSON-RPC 2.0 compliance, Cursor compatibility) are folded into conformance scenarios
- **D-03:** CLI signature: `mcp-tester conformance <url>` with `--strict` flag (existing pattern from compliance)

### Scenario design
- **D-04:** Built-in hardcoded scenarios in Rust, not YAML files — conformance is the canonical spec validation, not user-configurable
- **D-05:** Scenarios grouped by MCP domain: Core (init handshake, protocol version), Tools (list, call, error handling), Resources (list, read, templates), Prompts (list, get, with arguments), Tasks (lifecycle — conditional on capability)
- **D-06:** Each domain group reports pass/fail independently — a server with no resources still passes if it correctly reports empty capabilities
- **D-07:** Re-use existing `ServerTester` protocol operations (test_initialize, list_tools, test_tool, list_resources, read_resource, list_prompts, get_prompt, send_custom_request) as the underlying transport

### Task lifecycle testing
- **D-08:** Task conformance scenarios are conditional — only run if server advertises `tasks` capability in initialize response
- **D-09:** If tasks capability absent, task scenarios are reported as Skipped (not Failed)
- **D-10:** Task lifecycle test: create task → poll until terminal status → verify state machine transitions are valid

### Output and reporting
- **D-11:** Follow existing report infrastructure — TestReport with TestCategory, TestStatus (Passed/Failed/Warning/Skipped), all output formats (Pretty/JSON/Minimal/Verbose)
- **D-12:** Add `TestCategory::Conformance` or reuse existing categories (Core, Protocol, Tools, Resources, Prompts) for domain grouping
- **D-13:** Summary line showing pass/fail per domain group for quick CI consumption
- **D-14:** `--strict` mode promotes warnings to failures (existing compliance pattern)

### Integration with cargo pmcp test
- **D-15:** Surface as `cargo pmcp test conformance <url>` through existing tester integration (same pattern as other mcp-tester subcommands)

### Claude's Discretion
- Internal scenario implementation (trait-based, enum dispatch, or flat function list)
- Exact set of error-handling scenarios (invalid method, malformed params, unknown tool name)
- Whether to add a `--domain` filter flag (e.g., `--domain tools,resources` to run subset)
- Conformance report preamble text and formatting details
- Whether `run_full_suite` incorporates conformance or stays separate

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing mcp-tester infrastructure
- `crates/mcp-tester/src/main.rs` — CLI entry point with all subcommands (Compliance at line 117)
- `crates/mcp-tester/src/tester.rs` — ServerTester core (~2900 lines) with all protocol operations
- `crates/mcp-tester/src/validators.rs` — ValidationResult, protocol/capability/schema validators
- `crates/mcp-tester/src/report.rs` — TestReport, TestCategory, TestStatus, OutputFormat
- `crates/mcp-tester/src/scenario_executor.rs` — Existing scenario execution engine (reference for conformance runner)

### TypeScript conformance reference
- `.planning/phases/53-review-typescript-sdk-updates/53-RESEARCH.md` lines 131-159 — TypeScript conformance infrastructure analysis, everythingServer/everythingClient patterns
- `.planning/phases/53-review-typescript-sdk-updates/53-01-VERIFICATION-NOTES.md` line 138 — Client conformance scenario pattern

### Protocol types (Phase 54)
- `src/types/tools.rs` — ToolInfo, CallToolParams, CallToolResult
- `src/types/resources.rs` — ResourceInfo, ResourceTemplate, ReadResourceResult
- `src/types/prompts.rs` — PromptInfo, PromptMessage, GetPromptResult
- `src/types/tasks.rs` — Task, TaskStatus, CreateTaskResult
- `src/types/capabilities.rs` — ServerCapabilities, ClientCapabilities (for capability-conditional testing)

### Prior phase context
- `.planning/phases/55-tasks-with-polling/55-CONTEXT.md` — Task lifecycle decisions (create, get, list, cancel)
- `.planning/phases/56-tower-middleware-dns-rebinding-protection/56-CONTEXT.md` — Tower middleware (servers under test may use this)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ServerTester` — Full protocol operation support (initialize, tools/list, tools/call, resources/list, resources/read, prompts/list, prompts/get, custom requests, send_custom_request)
- `run_compliance_tests()` — Existing compliance logic to fold into conformance
- `TestReport` / `TestCategory` / `TestStatus` — Complete reporting infrastructure
- `Validator` class in `validators.rs` — Protocol version, initialize response, server info, capabilities, tool schema validation
- `ScenarioExecutor` — Reference for sequential test step execution with assertions

### Established Patterns
- Subcommand pattern: URL as positional arg, `--strict` flag, `--format` global, `--verbose` global
- `ServerTester::new()` handles transport detection, OAuth, TLS, timeouts
- Test methods return `TestResult` with category, status, duration, error details
- Capability-conditional testing (apps validator checks for app-capable tools before validating)

### Integration Points
- `main.rs` Commands enum — add `Conformance` variant, remove `Compliance`
- `tester.rs` — add `run_conformance_tests()` method
- `report.rs` — may need conformance-specific category or domain grouping
- `cargo-pmcp/src/commands/test.rs` — surface conformance through `cargo pmcp test conformance`

</code_context>

<specifics>
## Specific Ideas

- Conformance replaces compliance as the standard spec validation command — "compliance" is a weaker term; "conformance" aligns with the official MCP SDK terminology
- Follow the TypeScript SDK pattern: `@modelcontextprotocol/conformance` is the reference implementation for what scenarios to test
- Task lifecycle testing must handle the reality that most servers today don't support tasks — conditional testing is essential
- The existing scenario infrastructure (YAML/JSON) remains for custom user tests; conformance is the built-in canonical validation

</specifics>

<deferred>
## Deferred Ideas

- Auth conformance scenarios (OAuth flows, CIMD scenarios) — separate phase, large surface area
- SSE resumability testing — de-prioritized per v2.0 direction
- "Everything server" reference implementation — could be useful but separate scope
- Conformance certification/badge output — future enhancement
- Transport-specific conformance (stdio vs HTTP behavioral differences) — future phase

</deferred>

---

*Phase: 57-conformance-test-suite*
*Context gathered: 2026-03-20*
