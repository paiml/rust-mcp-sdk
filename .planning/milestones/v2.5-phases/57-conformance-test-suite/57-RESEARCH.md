# Phase 57: Conformance Test Suite - Research

**Researched:** 2026-03-20
**Domain:** MCP protocol conformance testing, CLI subcommand design, test infrastructure
**Confidence:** HIGH

## Summary

This phase replaces the existing `mcp-tester compliance` subcommand with a new `conformance` subcommand that validates any MCP server against the MCP protocol spec (2025-11-25). The conformance tests are built-in hardcoded Rust scenarios (not YAML), grouped by MCP domain: Core (initialize handshake, protocol version), Tools (list, call, error handling), Resources (list, read, templates), Prompts (list, get, with arguments), and Tasks (lifecycle -- conditional on capability advertisement).

The existing mcp-tester infrastructure is mature and provides all the building blocks: `ServerTester` handles transport detection and all protocol operations; `TestReport`/`TestCategory`/`TestStatus` provide complete reporting with 4 output formats; `Validator` validates protocol version, JSON-RPC compliance, and tool schemas; `apply_strict_mode()` promotes warnings to failures. The `compliance` command currently runs 5 tests (initialize, protocol version, required methods, error codes, JSON-RPC compliance + cursor compatibility), but `test_error_codes()` and `test_json_rpc_compliance()` are stubs that always return Passed. The conformance command replaces all of this with substantive, spec-aligned scenarios.

The TypeScript SDK's `@modelcontextprotocol/conformance` CLI (analyzed in Phase 53) provides the reference catalog: 14 server-side tools, 4 resources, 4 prompts, and 23 client scenarios. For Phase 57, we port the server-side conformance patterns only (not auth scenarios, which are deferred per CONTEXT.md).

**Primary recommendation:** Implement conformance as a new module `crates/mcp-tester/src/conformance.rs` (or `conformance/` directory) that defines scenario groups using flat functions, each returning `Vec<TestResult>`. Replace the `Compliance` CLI variant with `Conformance`. Add `--domain` filter flag for running subsets. Surface through `cargo pmcp test conformance <url>`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: The existing `compliance` subcommand is replaced by the new `conformance` subcommand -- not a separate coexisting command
- D-02: All existing compliance checks (required methods, error codes, JSON-RPC 2.0 compliance, Cursor compatibility) are folded into conformance scenarios
- D-03: CLI signature: `mcp-tester conformance <url>` with `--strict` flag (existing pattern from compliance)
- D-04: Built-in hardcoded scenarios in Rust, not YAML files -- conformance is the canonical spec validation, not user-configurable
- D-05: Scenarios grouped by MCP domain: Core (init handshake, protocol version), Tools (list, call, error handling), Resources (list, read, templates), Prompts (list, get, with arguments), Tasks (lifecycle -- conditional on capability)
- D-06: Each domain group reports pass/fail independently -- a server with no resources still passes if it correctly reports empty capabilities
- D-07: Re-use existing `ServerTester` protocol operations (test_initialize, list_tools, test_tool, list_resources, read_resource, list_prompts, get_prompt, send_custom_request) as the underlying transport
- D-08: Task conformance scenarios are conditional -- only run if server advertises `tasks` capability in initialize response
- D-09: If tasks capability absent, task scenarios are reported as Skipped (not Failed)
- D-10: Task lifecycle test: create task -> poll until terminal status -> verify state machine transitions are valid
- D-11: Follow existing report infrastructure -- TestReport with TestCategory, TestStatus (Passed/Failed/Warning/Skipped), all output formats (Pretty/JSON/Minimal/Verbose)
- D-12: Add `TestCategory::Conformance` or reuse existing categories (Core, Protocol, Tools, Resources, Prompts) for domain grouping
- D-13: Summary line showing pass/fail per domain group for quick CI consumption
- D-14: `--strict` mode promotes warnings to failures (existing compliance pattern)
- D-15: Surface as `cargo pmcp test conformance <url>` through existing tester integration

### Claude's Discretion
- Internal scenario implementation (trait-based, enum dispatch, or flat function list)
- Exact set of error-handling scenarios (invalid method, malformed params, unknown tool name)
- Whether to add a `--domain` filter flag (e.g., `--domain tools,resources` to run subset)
- Conformance report preamble text and formatting details
- Whether `run_full_suite` incorporates conformance or stays separate

### Deferred Ideas (OUT OF SCOPE)
- Auth conformance scenarios (OAuth flows, CIMD scenarios) -- separate phase, large surface area
- SSE resumability testing -- de-prioritized per v2.0 direction
- "Everything server" reference implementation -- could be useful but separate scope
- Conformance certification/badge output -- future enhancement
- Transport-specific conformance (stdio vs HTTP behavioral differences) -- future phase
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONFORMANCE-CLI | `mcp-tester conformance <url>` command that replaces `compliance`, with `--strict`, `--domain`, and all existing global flags (format, verbose, timeout, insecure, api-key, OAuth) | Existing CLI pattern in main.rs Commands enum; replace Compliance variant with Conformance; surface through cargo-pmcp TestCommand enum |
| CONFORMANCE-SCENARIOS | Built-in hardcoded conformance scenarios covering 5 MCP domains (Core, Tools, Resources, Prompts, Tasks) with capability-conditional testing | ServerTester already supports all protocol operations; conformance module implements scenario groups as functions returning Vec<TestResult>; task scenarios gated on ServerCapabilities.tasks |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pmcp | 1.20.0 | MCP protocol types, Client, ServerTester transport | Already a dependency of mcp-tester |
| clap | 4 | CLI argument parsing with derive macros | Already used for all mcp-tester subcommands |
| serde_json | 1 | JSON manipulation for custom requests/assertions | Already used throughout mcp-tester |
| tokio | 1 | Async runtime for all protocol operations | Already used |
| chrono | 0.4 | Timestamp formatting in reports | Already used in TestReport |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| colored | 3 | Terminal output formatting | Already used for Pretty/Verbose output |
| anyhow | 1 | Error handling in test execution | Already used throughout |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Flat function groups | Trait-based scenarios | Traits add complexity for no benefit -- scenarios are not user-extensible (D-04) |
| Enum dispatch per domain | Module per domain | Module organization is cleaner for 5 domains with 3-8 tests each |

**Installation:** No new dependencies needed. All libraries are already in mcp-tester's Cargo.toml.

## Architecture Patterns

### Recommended Project Structure
```
crates/mcp-tester/src/
  conformance/
    mod.rs            # ConformanceRunner + domain orchestration + --domain filtering
    core.rs           # Core domain: initialize handshake, protocol version, server info
    tools.rs          # Tools domain: list, call, error handling, schema validation
    resources.rs      # Resources domain: list, read, templates (capability-conditional)
    prompts.rs        # Prompts domain: list, get, with arguments (capability-conditional)
    tasks.rs          # Tasks domain: lifecycle (capability-conditional, Skipped if absent)
  main.rs             # Commands::Conformance variant (replaces Commands::Compliance)
  tester.rs           # run_conformance_tests() added (delegates to conformance module)
  report.rs           # No structural changes needed; reuse existing TestCategory variants
  lib.rs              # pub mod conformance; re-export ConformanceRunner
```

### Pattern 1: Domain Scenario Group (flat functions returning Vec<TestResult>)
**What:** Each domain module exposes a single async function that takes `&mut ServerTester` and returns `Vec<TestResult>`.
**When to use:** For all 5 conformance domain groups.
**Why:** Simple, testable, no trait boilerplate. Each function is a self-contained test sequence.
**Example:**
```rust
// crates/mcp-tester/src/conformance/tools.rs
use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;

/// Run all tools conformance scenarios.
/// Assumes server is already initialized (ServerTester has capabilities cached).
pub async fn run_tools_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    let mut results = Vec::new();

    // Check if tools capability is advertised
    let has_tools = tester.server_capabilities()
        .map(|c| c.tools.is_some())
        .unwrap_or(false);

    if !has_tools {
        results.push(TestResult {
            name: "Tools: capability advertised".to_string(),
            category: TestCategory::Tools,
            status: TestStatus::Skipped,
            duration: Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise tools capability".to_string()),
        });
        return results;
    }

    results.push(test_tools_list(tester).await);
    results.push(test_tools_call_existing(tester).await);
    results.push(test_tools_call_unknown(tester).await);
    results.push(test_tools_call_invalid_params(tester).await);
    results
}
```

### Pattern 2: ConformanceRunner orchestrator
**What:** A struct that holds configuration (strict mode, domain filter) and orchestrates all domain groups.
**When to use:** The top-level entry point called from main.rs and tester.rs.
**Example:**
```rust
// crates/mcp-tester/src/conformance/mod.rs
pub struct ConformanceRunner {
    strict: bool,
    domains: Option<Vec<ConformanceDomain>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceDomain {
    Core,
    Tools,
    Resources,
    Prompts,
    Tasks,
}

impl ConformanceRunner {
    pub fn new(strict: bool, domains: Option<Vec<ConformanceDomain>>) -> Self {
        Self { strict, domains }
    }

    pub async fn run(&self, tester: &mut ServerTester) -> TestReport {
        let mut report = TestReport::new();
        let start = Instant::now();

        // Always run core first (initializes the server connection)
        if self.should_run(ConformanceDomain::Core) {
            for result in core::run_core_conformance(tester).await {
                report.add_test(result);
            }
        }

        // Only proceed if initialization succeeded
        if !report.has_failures() || !self.should_run_only_core() {
            if self.should_run(ConformanceDomain::Tools) {
                for result in tools::run_tools_conformance(tester).await {
                    report.add_test(result);
                }
            }
            // ... resources, prompts, tasks
        }

        if self.strict {
            report.apply_strict_mode();
        }

        report.duration = start.elapsed();
        report
    }

    fn should_run(&self, domain: ConformanceDomain) -> bool {
        self.domains.as_ref().map_or(true, |d| d.contains(&domain))
    }
}
```

### Pattern 3: Capability-conditional testing (existing pattern from compliance)
**What:** Check `server_capabilities` before running domain-specific tests. Report Skipped (not Failed) when capability is absent.
**When to use:** For all non-core domains (Tools, Resources, Prompts, Tasks).
**Example from existing code (line 496-509 of tester.rs):**
```rust
if let Some(caps) = &self.server_capabilities {
    if caps.resources.is_none() {
        report.add_test(TestResult {
            name: "Resources support".to_string(),
            category: TestCategory::Resources,
            status: TestStatus::Skipped,
            duration: Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise resource capabilities".to_string()),
        });
        return results;
    }
}
```

### Pattern 4: Error handling scenarios via send_custom_request
**What:** Use `send_custom_request` for negative testing (unknown methods, malformed params).
**When to use:** Core and Tools domain error-handling conformance tests.
**Example:**
```rust
async fn test_unknown_method(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let response = tester.send_custom_request(
        "nonexistent/method",
        serde_json::json!({}),
    ).await;

    match response {
        Ok(value) => {
            // Should get a JSON-RPC error with code -32601 (Method not found)
            if let Some(error) = value.get("error") {
                if error.get("code").and_then(|c| c.as_i64()) == Some(-32601) {
                    TestResult {
                        name: "Core: unknown method returns -32601".to_string(),
                        category: TestCategory::Protocol,
                        status: TestStatus::Passed,
                        // ...
                    }
                } else {
                    // Wrong error code -- warning or failure
                }
            } else {
                // No error returned -- unexpected success
            }
        }
        Err(e) => { /* Transport error */ }
    }
}
```

### Anti-Patterns to Avoid
- **Trait-based scenario abstraction:** D-04 explicitly says scenarios are hardcoded, not user-configurable. Traits add extension points that are not needed and increase complexity.
- **YAML/JSON scenario files for conformance:** Conformance is the canonical spec validation. User-configurable scenarios already exist via the `scenario` command. Don't conflate the two.
- **Modifying TestCategory for per-domain conformance:** Reuse existing TestCategory variants (Core, Protocol, Tools, Resources, Prompts). Adding TestCategory::Conformance would lose domain granularity. If domain grouping in the summary is needed, build it from the existing categories.
- **Re-implementing transport logic:** All protocol operations go through ServerTester. Never bypass it with raw HTTP calls in conformance tests.
- **Tight coupling to specific tool/resource names:** Conformance tests should work with any server. Don't hardcode "tools must have a tool named X". Instead, test that tools/list returns valid ToolInfo shapes, and test call behavior on whatever tools exist.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Transport detection & connection | Custom HTTP/stdio handlers | `ServerTester::new()` | Handles all 3 transport types, OAuth, TLS, API keys |
| Protocol operations | Raw JSON-RPC construction | `tester.list_tools()`, `tester.read_resource()`, etc. | Already handles transport-specific serialization, deserialization, error mapping |
| Report formatting | Custom output formatting | `TestReport::print(format)` | Already supports Pretty/JSON/Minimal/Verbose with color, tables, summaries |
| Strict mode enforcement | Manual warning-to-failure promotion | `report.apply_strict_mode()` | Existing one-liner that mutates all Warning -> Failed |
| Protocol version validation | Custom version checks | `Validator::validate_protocol_version()` | Already knows supported versions (needs update for 2025-11-25 which was added in Phase 54) |
| JSON-RPC error validation | Custom error parsing | `Validator::validate_json_rpc_error()` | Validates code, message, standard codes |
| Tool schema validation | Custom JSON Schema checks | `Validator::validate_tool_definition()` | Validates name, description, inputSchema |

**Key insight:** mcp-tester's `ServerTester` is a fully-featured MCP client with 2900+ lines of protocol operations. The conformance module should be a thin orchestration layer on top of it, not a reimplementation.

## Common Pitfalls

### Pitfall 1: Testing server capabilities vs server behavior
**What goes wrong:** Conformance test fails because server doesn't have tools, when the real test is "does the server correctly report it has no tools?"
**Why it happens:** Conflating "capability advertisement" with "feature implementation."
**How to avoid:** D-06 explicitly states: each domain group reports independently. A server with no resources passes if it correctly reports empty capabilities. Test the capability advertisement first, then test behavior only if the capability is present.
**Warning signs:** Test names like "Server has tools" -- should be "Server correctly reports tools capability."

### Pitfall 2: Stale server_capabilities reference
**What goes wrong:** Conformance tests read server_capabilities before initialize() completes, getting None.
**Why it happens:** ServerTester stores capabilities in `self.server_capabilities` only after successful `test_initialize()`.
**How to avoid:** Core domain must run first and succeed before any other domain. ConformanceRunner must check initialization result before proceeding.
**Warning signs:** All domain tests returning Skipped with "Server not initialized."

### Pitfall 3: Task conformance on servers without task support
**What goes wrong:** Task lifecycle test tries `tasks/get` on a server that doesn't support tasks, gets -32601 and reports failure.
**Why it happens:** D-08/D-09 require conditional testing based on capability advertisement, but the check was missed.
**How to avoid:** Explicitly check `server_capabilities.tasks.is_some()` before any task protocol operation. Report Skipped, not Failed.
**Warning signs:** All task tests failing on basic MCP servers.

### Pitfall 4: Removing compliance without preserving its test coverage
**What goes wrong:** Old compliance tests covered JSON-RPC error codes and Cursor compatibility. New conformance drops them, reducing coverage.
**Why it happens:** D-02 says "fold into conformance" but implementer treats it as "replace."
**How to avoid:** Audit each existing compliance test and ensure it has a conformance equivalent:
- `test_protocol_version()` -> Core domain: protocol version validation
- `test_required_methods()` -> Core domain: method availability
- `test_error_codes()` -> Core domain: JSON-RPC error codes (currently a stub -- make it real)
- `test_json_rpc_compliance()` -> Core domain: JSON-RPC 2.0 format (currently a stub -- make it real)
- `test_cursor_compatibility()` -> Core domain: Cursor IDE compat or drop (it was a bonus check)
**Warning signs:** Conformance has fewer tests than old compliance.

### Pitfall 5: Hardcoding tool/resource expectations
**What goes wrong:** Conformance test expects `tools/call` to succeed, but the server has tools that require complex inputs.
**Why it happens:** Can't call arbitrary tools with empty args and expect success.
**How to avoid:** For "call existing tool" scenarios, either: (a) use the first tool with `{}` args and accept either success or a well-formed error (valid isError=true response), or (b) validate only that the call returns a valid CallToolResult structure regardless of success/error content.
**Warning signs:** Conformance failing on servers with tools that have required parameters.

### Pitfall 6: cargo pmcp test integration not wired
**What goes wrong:** `mcp-tester conformance <url>` works but `cargo pmcp test conformance <url>` doesn't.
**Why it happens:** Forgot to add Conformance variant to cargo-pmcp's TestCommand enum and wire the execution.
**How to avoid:** Plan includes explicit step to update cargo-pmcp/src/commands/test/mod.rs and add conformance.rs handler module.
**Warning signs:** `cargo pmcp test --help` doesn't list conformance.

## Code Examples

### Existing compliance command entry point (to be replaced)
```rust
// crates/mcp-tester/src/main.rs line 117-124
Commands::Compliance {
    url: String,
    #[arg(long)]
    strict: bool,
}

// Line 304-316: Handler dispatches to tester.run_compliance_tests(strict)
```

### Existing compliance test method (to be folded into conformance)
```rust
// crates/mcp-tester/src/tester.rs line 439-467
pub async fn run_compliance_tests(&mut self, strict: bool) -> Result<TestReport> {
    let mut report = TestReport::new();
    let start = Instant::now();

    let init_result = self.test_initialize().await;
    report.add_test(init_result.clone());

    if init_result.status != TestStatus::Passed {
        return Ok(report);
    }

    report.add_test(self.test_protocol_version().await);
    report.add_test(self.test_required_methods().await);
    report.add_test(self.test_error_codes().await);      // STUB - always passes
    report.add_test(self.test_json_rpc_compliance().await); // STUB - always passes
    report.add_test(self.test_cursor_compatibility().await);

    if strict { report.apply_strict_mode(); }
    report.duration = start.elapsed();
    Ok(report)
}
```

### ServerTester capability access (existing pattern)
```rust
// Line 496-509: Capability check before domain testing
if let Some(caps) = &self.server_capabilities {
    if caps.resources.is_none() {
        // Skip with TestStatus::Skipped
    }
}
```

### TestResult construction (existing pattern)
```rust
TestResult {
    name: "Tools: list returns valid ToolInfo".to_string(),
    category: TestCategory::Tools,
    status: TestStatus::Passed,
    duration: start.elapsed(),
    error: None,
    details: Some(format!("Found {} tools", tools.len())),
}
```

### Task lifecycle conformance (new pattern)
```rust
// Use send_custom_request for task operations that aren't wrapped in Client
let create_response = tester.send_custom_request(
    "tools/call",
    json!({
        "name": tool_name,
        "arguments": {},
        "_meta": {
            "task": { "ttl": 60000 }
        }
    }),
).await?;

// Verify response contains a task with valid status
if let Some(task) = create_response.get("task") {
    let status = task.get("status").and_then(|s| s.as_str());
    // Verify status is a valid TaskStatus value
    let valid_statuses = ["working", "input_required", "completed", "failed", "cancelled"];
    if !valid_statuses.contains(&status.unwrap_or("")) {
        // Report failure
    }

    // Poll tasks/get until terminal
    if let Some(task_id) = task.get("taskId").and_then(|id| id.as_str()) {
        let get_response = tester.send_custom_request(
            "tasks/get",
            json!({ "taskId": task_id }),
        ).await?;
        // Validate state machine transitions
    }
}
```

### Domain summary in minimal output (new pattern)
```
PASS: Core 5/5, Tools 4/4, Resources 3/3, Prompts 3/3, Tasks 0/0 (skipped) in 1.23s
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `mcp-tester compliance` (stubs) | `mcp-tester conformance` (real tests) | Phase 57 | Spec-aligned validation |
| Custom compliance checks | Domain-grouped scenarios | Phase 57 | Modular, extensible per domain |
| No task conformance | Capability-conditional task lifecycle | Phase 55 (tasks) + 57 | Full MCP 2025-11-25 coverage |
| TypeScript-only conformance CLI | Rust mcp-tester conformance | Phase 57 | Cross-ecosystem validation tool |

**Deprecated/outdated:**
- `Commands::Compliance` -- replaced by `Commands::Conformance`
- `tester.run_compliance_tests()` -- replaced by `ConformanceRunner::run()`
- `test_error_codes()` stub -- replaced by real JSON-RPC error code scenarios
- `test_json_rpc_compliance()` stub -- replaced by real JSON-RPC 2.0 validation
- `test_cursor_compatibility()` -- content folded into Core domain or dropped (it's cursor-specific, not spec conformance)

## Open Questions

1. **Should `run_full_suite` incorporate conformance?**
   - What we know: `run_full_suite` currently runs connectivity, initialize, protocol version, capabilities, tools, resources, prompts, and error handling. Conformance covers similar ground but more thoroughly.
   - What's unclear: Whether running both would be redundant or whether they serve different audiences (quick check vs spec validation).
   - Recommendation: Keep them separate. `run_full_suite` is a quick sanity check; `conformance` is the thorough spec validation. They can share test logic but have different orchestration.

2. **Task lifecycle: how to trigger task creation on an arbitrary server?**
   - What we know: Tasks are created via `tools/call` with `_meta.task` parameters. But we need a tool to call, and we don't know what tools the server has or what args they accept.
   - What's unclear: Whether any tool can be called with empty args and `_meta.task` to trigger task creation, or if the tool must be task-aware.
   - Recommendation: If server advertises tasks capability, try calling the first available tool with `_meta.task: { ttl: 60000 }` and empty args. If the tool errors on args, the test validates that the error response is well-formed. If the tool succeeds with a task, validate the task lifecycle. Report as Warning if no tool produces a task.

3. **TestCategory for conformance: add new variant or reuse existing?**
   - What we know: D-12 says "Add `TestCategory::Conformance` or reuse existing categories." Existing categories map well to domains (Core, Protocol, Tools, Resources, Prompts).
   - What's unclear: Whether the domain summary (D-13) needs a way to distinguish conformance tests from other tests of the same category.
   - Recommendation: Reuse existing categories. The conformance command runs in isolation (its own TestReport), so there's no mixing with other test types. Add TestCategory::Tasks if one doesn't exist (it doesn't currently).

## Conformance Scenario Catalog

Based on the TypeScript SDK reference and MCP 2025-11-25 spec, here is the complete scenario catalog for Phase 57.

### Core Domain (5-7 scenarios)
| # | Scenario | Tests | Pass Criteria |
|---|----------|-------|---------------|
| C-01 | Initialize handshake | initialize + notifications/initialized | Server returns valid InitializeResult with protocolVersion, capabilities, serverInfo |
| C-02 | Protocol version | protocolVersion field | Version is in supported set (2025-11-25, 2025-06-18, 2025-03-26) |
| C-03 | Server info | serverInfo.name, serverInfo.version | Both present, both strings |
| C-04 | Capabilities structure | capabilities object | Valid JSON object, known keys only |
| C-05 | Unknown method | send "nonexistent/method" | Returns JSON-RPC error -32601 (Method not found) |
| C-06 | Malformed request | send invalid JSON-RPC | Returns JSON-RPC error -32600 (Invalid request) or connection error |

### Tools Domain (3-5 scenarios)
| # | Scenario | Tests | Pass Criteria |
|---|----------|-------|---------------|
| T-01 | List tools | tools/list | Returns valid array of ToolInfo (each has name as string) |
| T-02 | Tool schema validation | Each tool's inputSchema | Valid JSON Schema object or null |
| T-03 | Call existing tool | tools/call with first tool | Returns valid CallToolResult (content array or isError=true with content) |
| T-04 | Call unknown tool | tools/call "___nonexistent___" | Returns error (JSON-RPC error or isError=true response) |

### Resources Domain (2-4 scenarios, capability-conditional)
| # | Scenario | Tests | Pass Criteria |
|---|----------|-------|---------------|
| R-01 | List resources | resources/list | Returns valid array of ResourceInfo (each has name, uri) |
| R-02 | Read first resource | resources/read with first resource URI | Returns valid ReadResourceResult with contents array |
| R-03 | Read invalid URI | resources/read with nonexistent URI | Returns error (not crash) |

### Prompts Domain (2-4 scenarios, capability-conditional)
| # | Scenario | Tests | Pass Criteria |
|---|----------|-------|---------------|
| P-01 | List prompts | prompts/list | Returns valid array of PromptInfo (each has name) |
| P-02 | Get first prompt | prompts/get with first prompt name | Returns valid GetPromptResult with messages array |
| P-03 | Get unknown prompt | prompts/get "___nonexistent___" | Returns error (not crash) |

### Tasks Domain (2-4 scenarios, capability-conditional, Skipped if no tasks capability)
| # | Scenario | Tests | Pass Criteria |
|---|----------|-------|---------------|
| K-01 | Tasks capability advertised | capabilities.tasks present | ServerTasksCapability with valid structure |
| K-02 | Task creation via tools/call | tools/call with _meta.task | Response includes task field with valid taskId, status |
| K-03 | Task get | tasks/get with taskId from K-02 | Returns Task with valid status value |
| K-04 | Task status transitions | Poll tasks/get, validate transitions | All transitions follow state machine (Working/InputRequired can transition; terminal states don't) |

## Sources

### Primary (HIGH confidence)
- `crates/mcp-tester/src/main.rs` -- Full CLI structure with all subcommands, Commands::Compliance at line 117
- `crates/mcp-tester/src/tester.rs` -- ServerTester implementation (~2900 lines), run_compliance_tests at line 439, all protocol operations
- `crates/mcp-tester/src/report.rs` -- TestReport, TestCategory (8 variants), TestStatus (4 variants), OutputFormat (4 variants)
- `crates/mcp-tester/src/validators.rs` -- ValidationResult, Validator with protocol/tool/JSON-RPC validation
- `crates/mcp-tester/src/scenario_executor.rs` -- ScenarioExecutor pattern for sequential test execution
- `crates/mcp-tester/src/lib.rs` -- Public API surface, re-exports
- `crates/mcp-tester/Cargo.toml` -- Dependency set (no new deps needed)
- `cargo-pmcp/src/commands/test/mod.rs` -- TestCommand enum for cargo pmcp test integration
- `src/types/tasks.rs` -- Task, TaskStatus with is_terminal() and can_transition_to()
- `src/types/capabilities.rs` -- ServerCapabilities.tasks (ServerTasksCapability), ClientCapabilities.tasks
- `.planning/phases/57-conformance-test-suite/57-CONTEXT.md` -- All locked decisions D-01 through D-15

### Secondary (MEDIUM confidence)
- `.planning/phases/53-review-typescript-sdk-updates/53-RESEARCH.md` -- TypeScript conformance infrastructure analysis
- `.planning/phases/53-review-typescript-sdk-updates/53-01-VERIFICATION-NOTES.md` -- Detailed scenario catalog (14 server tools, 4 resources, 4 prompts, 23 client scenarios)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies; entirely uses existing mcp-tester infrastructure verified by reading source
- Architecture: HIGH -- pattern follows existing AppValidator and compliance patterns, directly observed in source code
- Pitfalls: HIGH -- derived from actual code inspection of ServerTester lifecycle, capability caching, and existing stub tests
- Scenario catalog: MEDIUM -- derived from TypeScript reference and MCP spec, but exact error codes and edge cases need validation during implementation

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain -- conformance spec changes slowly)
