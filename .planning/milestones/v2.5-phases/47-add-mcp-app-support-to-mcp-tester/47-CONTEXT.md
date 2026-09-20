# Phase 47: Add MCP App support to mcp-tester - Context

**Gathered:** 2026-03-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Add MCP App protocol validation to mcp-tester so developers can validate MCP App compliance via CLI (`cargo pmcp test apps`) without opening a browser. Must work with ANY MCP server (not just PMCP-built ones). Enables CI integration for App compliance checks.

</domain>

<decisions>
## Implementation Decisions

### Validation scope
- Metadata-only validation (no widget fetching, no browser rendering)
- Validate both tools AND resources for App compliance
- Tools: check for ui.resourceUri in _meta, valid URI format, MIME type declarations
- Resources: check for matching URI, correct MIME type (text/html or application/html+mcp-app), _meta structure
- outputSchema: validate if present (top-level, valid JSON Schema) but not required for App-capable tools
- Cross-reference: warn (not fail) if a tool's ui.resourceUri has no matching resource in resources/list

### Command structure
- New subcommand: `cargo pmcp test apps <url>`
- Mirror as standalone: `mcp-tester apps <url>`
- All App-capable tools tested by default; `--tool <name>` flag for specific tool
- `cargo pmcp test check` should detect App-capable tools and show hint: "N App-capable tools detected. Run `cargo pmcp test apps` for full validation."

### Host-specific modes
- Use `--mode` flag (consistent with `cargo pmcp preview --mode chatgpt`)
- Default (no --mode): standard MCP App compliance only (ui.resourceUri nested key)
- `--mode chatgpt`: standard checks PLUS ChatGPT-specific keys (openai/outputTemplate, openai/toolInvocation/*, legacy flat ui/resourceUri key)
- `--mode claude-desktop`: standard checks plus Claude Desktop format validation
- ChatGPT mode validates server emission directly (not enrichment layer) — checks that the server's actual responses contain the host-specific keys

### Output & reporting
- Reuse existing TestReport format (Pretty/JSON/Minimal/Verbose output modes)
- Results grouped per-tool: tool name → its checks (metadata, resource match, outputSchema, host-specific)
- Summary at bottom with total pass/warn/fail counts
- Exit code: 0 with warnings, 1 on failures only
- No App-capable tools found: exit 0 with info message ("No App-capable tools found")
- `--strict` flag available to promote warnings to failures

### Claude's Discretion
- Internal module organization (new file vs extending validators.rs)
- Exact check ordering within per-tool breakdown
- Pretty-format styling (colors, symbols, indentation)
- How to structure the App validator (trait, struct, functions)

</decisions>

<specifics>
## Specific Ideas

- Consistency with mcp-preview: use `--mode` flag (not `--host`) matching `cargo pmcp preview --mode chatgpt`
- The check command hint drives discoverability for the apps subcommand
- Must work with non-PMCP servers — validation checks raw protocol responses, not SDK internals

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Validator` struct in `crates/mcp-tester/src/validators.rs`: extensible validation with `strict_mode` toggle — can add App-specific methods
- `TestReport`/`TestResult` in `crates/mcp-tester/src/report.rs`: existing Pretty/JSON/Minimal/Verbose output — App results become TestResult entries
- `ServerTester` in `crates/mcp-tester/src/tester.rs`: orchestrates test execution, already calls tools/list and resources/list
- `PreviewMode` enum in mcp-preview: Standard vs ChatGPT — can share or mirror the enum for tester

### Established Patterns
- mcp-tester subcommands live in `crates/mcp-tester/src/main.rs` with clap derive
- cargo-pmcp test subcommands in `cargo-pmcp/src/commands/test/` — each subcommand is a separate module
- Validators return Vec<TestResult> with pass/warn/fail severity

### Integration Points
- `ServerTester::run_quick_test()` — where the check hint about App-capable tools would be added
- `cargo-pmcp/src/commands/test/mod.rs` — where apps subcommand would be registered
- `mcp-tester/src/main.rs` — where standalone apps subcommand would be added

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 47-add-mcp-app-support-to-mcp-tester*
*Context gathered: 2026-03-11*
