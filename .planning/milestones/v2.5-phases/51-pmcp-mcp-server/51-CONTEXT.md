# Phase 51: PMCP MCP Server - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Build an MCP server named "pmcp" for the MCP registry that serves as both a practical developer tool (build + test MCP servers) and a showcase of PMCP SDK capabilities. The server provides full lifecycle support: scaffolding instructions, protocol validation, test generation, schema export, and MCP Apps compliance checking — plus rich documentation resources and guided workflow prompts. Deployed via HTTP transport, hosted on pmcp.run.

</domain>

<decisions>
## Implementation Decisions

### Tool surface area
- Full lifecycle coverage: both build and test capabilities
- Server connects directly to remote MCP servers for testing (given a URL)
- Build tools return instructions + code snippets (not file generation) — AI agents or users apply them
- Tools to expose:
  1. **test check** — connect to an MCP server URL and run protocol compliance checks (mcp-tester core)
  2. **test generate** — generate test scenarios from a server's tool/resource listing
  3. **test apps** — MCP Apps metadata validation (cargo pmcp test apps equivalent)
  4. **scaffold** — return code templates for workspaces, servers, tools, resources, workflows
  5. **schema export** — connect to a server and export tool schemas as typed Rust code or JSON schema

### Resource design
- All content statically embedded in the binary (include_str! or similar) — no runtime file dependencies
- Content types to serve:
  1. SDK API reference (key types, traits, builder patterns)
  2. cargo-pmcp CLI guide (commands, flags, workflows)
  3. Best practices & patterns (tool structure, error handling, auth, MCP Apps)
  4. Example code from SDK examples
  5. pmcp-book chapters
- Broad topic URIs (e.g., pmcp://docs/typed-tools, pmcp://docs/auth, pmcp://book/chapter-12)
- One resource per major topic — fewer resources, more content each

### Prompt design
- Include guided workflow prompt templates for common MCP development scenarios
- Examples: 'create-mcp-server', 'add-tool', 'debug-protocol-error', 'setup-auth', 'quickstart', 'diagnose', 'migrate'

### Registry positioning
- Name: **pmcp**
- Primary audience: PMCP SDK users, but testing tools work for any MCP server regardless of language
- Positioned as both a practical tool AND a showcase/reference implementation of PMCP SDK patterns
- Researcher agent should scan the MCP registry for competing/complementary dev-tool servers during planning

### Deployment & packaging
- Lives in `crates/pmcp-server/` as a new workspace member
- HTTP-only transport (streamable HTTP) — no stdio
- Hosted on pmcp.run for immediate use without installation
- Binary releases alongside mcp-tester and mcp-preview in the existing release workflow (cross-platform: macOS ARM/x64, Linux ARM/x64, Windows)
- Published to crates.io

### Claude's Discretion
- Exact prompt template content and structure
- How to organize embedded documentation content (modules, const strings, etc.)
- API reference granularity — what to include vs. what's too detailed
- HTTP server configuration (port, auth, rate limiting for hosted instance)

</decisions>

<specifics>
## Specific Ideas

- The server dogfoods the PMCP SDK — it should demonstrate typed tools, resources, prompts, middleware, and other SDK patterns in its own implementation
- Registry listing should mention it's built with PMCP SDK and serves as a reference implementation
- Include the MCP Apps tester (cargo pmcp test apps) as a first-class tool alongside protocol compliance checks
- Book chapters from pmcp-book should be available as resources for AI agents to reference

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `mcp-tester` crate: `create_tester()`, `generate_scenarios()`, `run_scenario()` APIs for protocol validation
- `cargo-pmcp` commands: scaffold templates, schema export, app validation logic
- `pmcp` SDK: ServerCoreBuilder, TypedTool, PromptHandler, ResourceHandler, middleware chain
- MCP Apps tester in `cargo-pmcp/src/commands/test/` — apps metadata validation

### Established Patterns
- Typed tools with `TypedTool<Input, Output>` and `TypedToolWithOutput` for schema-driven tools
- Dynamic resources with pattern-based routing
- Workflow-based prompts via `SequentialWorkflow`
- Feature-gated optional functionality
- Builder pattern for server construction

### Integration Points
- Depends on `pmcp` (core SDK) and `mcp-tester` (testing library)
- May need to extract scaffold template logic from cargo-pmcp into a shared library
- Binary release workflow in `.github/workflows/release.yml` needs a new target
- Registry publishing via mcp-publisher in CI

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 51-pmcp-mcp-server*
*Context gathered: 2026-03-13*
