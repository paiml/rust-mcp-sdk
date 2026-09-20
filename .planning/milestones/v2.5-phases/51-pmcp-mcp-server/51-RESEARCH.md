# Phase 51: PMCP MCP Server - Research

**Researched:** 2026-03-13
**Domain:** MCP server development, static content embedding, protocol testing APIs, streamable HTTP transport
**Confidence:** HIGH

## Summary

This phase builds a new `pmcp-server` crate (workspace member at `crates/pmcp-server/`) that exposes PMCP SDK functionality -- protocol testing, scaffold templates, schema export, app validation, and documentation -- as MCP tools, resources, and prompts over streamable HTTP transport. The server will be hosted at pmcp.run and released as a cross-platform binary alongside mcp-tester.

The mcp-tester library already provides a clean programmatic API (`ServerTester`, `AppValidator`, `ScenarioGenerator`, `TestReport`) that returns serializable results. These can be consumed directly as dependencies. The scaffold template content exists in `cargo-pmcp/src/templates/` but is tightly coupled to file-writing logic; the template *strings* need extracting into a shared module or the server can depend on cargo-pmcp as a library (it already exposes `lib.rs`). Book content totals 640KB across 47 markdown files -- selective embedding (~10-15 key chapters) via `include_str!` is the right approach over `rust-embed` (no directory structure needed, just curated text content).

The MCP Registry currently has ~30 servers, none of which provide MCP development tooling. This server will be the first developer-tools-for-MCP-builders offering in the registry, which is a strong positioning.

**Primary recommendation:** Build `crates/pmcp-server/` depending on `pmcp` (streamable-http feature) and `mcp-tester`. Extract scaffold template content from cargo-pmcp or depend on cargo-pmcp as a library. Embed curated documentation via `include_str!`. Use the existing `StreamableHttpServer` for transport. Add binary target to release-binary.yml workflow.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Full lifecycle coverage: both build and test capabilities
- Server connects directly to remote MCP servers for testing (given a URL)
- Build tools return instructions + code snippets (not file generation) -- AI agents or users apply them
- Tools to expose: test check, test generate, test apps, scaffold, schema export
- All content statically embedded in binary (include_str! or similar) -- no runtime file dependencies
- Content types: SDK API reference, CLI guide, best practices, example code, pmcp-book chapters
- Broad topic URIs (e.g., pmcp://docs/typed-tools, pmcp://docs/auth, pmcp://book/chapter-12)
- One resource per major topic -- fewer resources, more content each
- Include guided workflow prompt templates (create-mcp-server, add-tool, debug-protocol-error, setup-auth, quickstart, diagnose, migrate)
- Name: pmcp
- Positioned as both practical tool AND showcase/reference implementation
- Lives in crates/pmcp-server/ as new workspace member
- HTTP-only transport (streamable HTTP) -- no stdio
- Hosted on pmcp.run
- Binary releases alongside mcp-tester and mcp-preview (cross-platform: macOS ARM/x64, Linux ARM/x64, Windows)
- Published to crates.io

### Claude's Discretion
- Exact prompt template content and structure
- How to organize embedded documentation content (modules, const strings, etc.)
- API reference granularity -- what to include vs. what's too detailed
- HTTP server configuration (port, auth, rate limiting for hosted instance)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pmcp | 1.18.2 | Core MCP SDK -- ServerCoreBuilder, StreamableHttpServer, tool/resource/prompt traits | Self-hosting; the server IS a PMCP showcase |
| mcp-tester | 0.3.3 | Protocol testing library -- ServerTester, AppValidator, ScenarioGenerator | Already exposes clean library API with serializable TestReport |
| tokio | 1.46 | Async runtime | Already used across all workspace crates |
| axum | 0.8.5 | HTTP framework (via pmcp streamable-http feature) | Already the transport layer in pmcp |
| serde / serde_json | 1.0 | Serialization for tool inputs/outputs | Standard across project |
| clap | 4.5 | CLI argument parsing for binary | Standard across project binaries |
| tracing | 0.1 | Structured logging | Standard across project |
| anyhow | 1.0 | Error handling | Standard across project |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing-subscriber | 0.3.20 | Log formatting for binary | Server startup/runtime logging |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| include_str! for docs | rust-embed | rust-embed adds dev-mode FS reads, but we don't need directory traversal -- just curated text files. include_str! is simpler, zero-dep, sufficient |
| cargo-pmcp as library dep | Extracting template strings to shared crate | cargo-pmcp already has lib.rs, but pulling it as a dep brings heavy deps (zip, oauth2, reqwest, etc.). Better to extract just the template content strings into a shared module |

**Installation:**
```toml
[dependencies]
pmcp = { version = "1.18.2", path = "../../", features = ["streamable-http"] }
mcp-tester = { version = "0.3.3", path = "../mcp-tester" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }
anyhow = "1"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/pmcp-server/
  src/
    main.rs              # Binary entry point, CLI args, server startup
    lib.rs               # Library root (for testing and reuse)
    tools/
      mod.rs             # Tool module exports
      test_check.rs      # test_check tool -- wraps ServerTester::run_compliance_tests()
      test_generate.rs   # test_generate tool -- wraps ScenarioGenerator
      test_apps.rs       # test_apps tool -- wraps AppValidator::validate_tools()
      scaffold.rs        # scaffold tool -- returns code templates as instructions
      schema_export.rs   # schema_export tool -- wraps ServerTester for schema discovery
    resources/
      mod.rs             # Resource handler with URI routing
      docs.rs            # SDK documentation resources
      book.rs            # pmcp-book chapter resources
    prompts/
      mod.rs             # Prompt handler routing
      workflows.rs       # Guided workflow prompt templates
    content/
      mod.rs             # Embedded content organization
      sdk_reference.rs   # const &str blocks for SDK docs
      cli_guide.rs       # const &str blocks for CLI docs
      best_practices.rs  # const &str blocks for patterns/practices
      examples.rs        # const &str blocks for example code
  Cargo.toml
  content/               # Source markdown files for include_str!
    sdk-typed-tools.md
    sdk-resources.md
    sdk-prompts.md
    sdk-auth.md
    sdk-middleware.md
    sdk-mcp-apps.md
    cli-guide.md
    best-practices.md
    ...
```

### Pattern 1: Tool Implementation via ToolHandler trait
**What:** Each MCP tool is a struct implementing `ToolHandler` with `handle()` and `metadata()`.
**When to use:** All five tools (test_check, test_generate, test_apps, scaffold, schema_export).
**Example:**
```rust
// Source: examples/22_streamable_http_server_stateful.rs
struct TestCheckTool;

#[async_trait]
impl ToolHandler for TestCheckTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: TestCheckInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        // Create ServerTester and run compliance tests
        let mut tester = ServerTester::new(
            &params.url,
            Duration::from_secs(params.timeout.unwrap_or(30)),
            false,
            None,
            None,
            None,
        ).map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        let report = tester.run_compliance_tests(params.strict.unwrap_or(false))
            .await
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        // TestReport implements Serialize -- return as JSON
        Ok(serde_json::to_value(&report)?)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "test_check",
            Some("Run MCP protocol compliance checks against a remote server".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "MCP server URL to test" },
                    "strict": { "type": "boolean", "description": "Enable strict compliance mode" },
                    "timeout": { "type": "integer", "description": "Timeout in seconds (default: 30)" }
                },
                "required": ["url"]
            }),
        ))
    }
}
```

### Pattern 2: Resource Handler with URI Routing
**What:** Single ResourceHandler implementation that routes `pmcp://docs/*` and `pmcp://book/*` URIs to embedded content.
**When to use:** All documentation resources.
**Example:**
```rust
// Source: src/server/traits.rs ResourceHandler trait
struct DocsResourceHandler;

#[async_trait]
impl ResourceHandler for DocsResourceHandler {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> pmcp::Result<ReadResourceResult> {
        let content = match uri {
            "pmcp://docs/typed-tools" => include_str!("../content/sdk-typed-tools.md"),
            "pmcp://docs/auth" => include_str!("../content/sdk-auth.md"),
            "pmcp://book/chapter-05" => include_str!("../content/book-ch05-tools.md"),
            // ... more URIs
            _ => return Err(pmcp::Error::resource_not_found(uri)),
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(uri, content, Some("text/markdown"))],
        })
    }

    async fn list(&self, _cursor: Option<String>, _extra: RequestHandlerExtra)
        -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult {
            resources: vec![
                ResourceInfo { uri: "pmcp://docs/typed-tools".into(), name: "Typed Tools Guide".into(), .. },
                // ... more resources
            ],
            next_cursor: None,
        })
    }
}
```

### Pattern 3: Server Assembly via Builder
**What:** Use `Server::builder()` to assemble tools, resources, and prompts, then wrap in StreamableHttpServer.
**When to use:** Server main.rs entry point.
**Example:**
```rust
// Source: examples/22_streamable_http_server_stateful.rs
let server = Server::builder()
    .name("pmcp")
    .version(env!("CARGO_PKG_VERSION"))
    .capabilities(ServerCapabilities {
        tools: Some(ToolCapabilities { list_changed: Some(false) }),
        resources: Some(ResourceCapabilities { subscribe: Some(false), list_changed: Some(false) }),
        prompts: Some(PromptCapabilities { list_changed: Some(false) }),
        ..Default::default()
    })
    .tool("test_check", TestCheckTool)
    .tool("test_generate", TestGenerateTool)
    .tool("test_apps", TestAppsTool)
    .tool("scaffold", ScaffoldTool)
    .tool("schema_export", SchemaExportTool)
    .resources(DocsResourceHandler)
    .prompt("create-mcp-server", CreateMcpServerPrompt)
    .prompt("add-tool", AddToolPrompt)
    .prompt("quickstart", QuickstartPrompt)
    // ... more prompts
    .build()
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

let server = Arc::new(Mutex::new(server));
let addr = SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), port);
let http_server = StreamableHttpServer::new(addr, server);
let (bound_addr, server_handle) = http_server.start().await?;
```

### Pattern 4: Scaffold as Instructions (not file writes)
**What:** Return structured code snippets and instructions, not write files.
**When to use:** scaffold tool.
**Example:**
```rust
// Instead of cargo-pmcp's generate_calculator() which writes files,
// return the template content as structured instructions
Ok(json!({
    "instructions": "Create the following files in your project:",
    "files": [
        {
            "path": "crates/mcp-{name}-core/src/lib.rs",
            "content": "// Core library for {name}\nuse pmcp::...",
            "description": "Core library with tool implementations"
        },
        {
            "path": "{name}-server/src/main.rs",
            "content": "// Server binary\n#[tokio::main]\nasync fn main() { ... }",
            "description": "Server binary entry point"
        }
    ],
    "next_steps": [
        "Run `cargo build` to verify compilation",
        "Run `cargo pmcp test check http://localhost:8080` to validate"
    ]
}))
```

### Anti-Patterns to Avoid
- **Depending on cargo-pmcp crate directly:** Brings 15+ transitive deps (zip, oauth2, hdrhistogram, etc.) that the server doesn't need. Extract only template content strings.
- **Embedding all 47 book chapters (640KB):** Binary bloat and context overload for AI agents. Curate 10-15 most relevant chapters.
- **Using stdio transport:** The server is HTTP-only (hosted at pmcp.run). No stdio path needed.
- **Writing files from tool handlers:** The scaffold tool must return content as structured JSON, not create files on the server's filesystem.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Protocol compliance testing | Custom MCP protocol validator | `mcp_tester::ServerTester::run_compliance_tests()` | Already handles connection, initialization, capability negotiation, tool listing, resource listing, prompt listing |
| Test scenario generation | Custom scenario builder | `mcp_tester::ScenarioGenerator::generate()` | Already handles server discovery, tool schema analysis, edge case generation |
| App metadata validation | Custom _meta inspector | `mcp_tester::AppValidator::validate_tools()` | Already handles standard/ChatGPT/Claude modes, resource cross-references |
| HTTP transport server | Custom axum router for MCP | `pmcp::server::streamable_http_server::StreamableHttpServer` | Already handles SSE, sessions, protocol version headers, CORS |
| Schema export discovery | Custom JSON-RPC client | `mcp_tester::ServerTester` (initialize + test_tools_list + list_resources) | Already handles transport negotiation and capability discovery |
| Server builder assembly | Manual server construction | `pmcp::Server::builder()` | Already handles capability inference, tool metadata caching, middleware chains |

**Key insight:** The entire test and schema surface area already exists as library code in mcp-tester. The server's tools are thin wrappers that deserialize input, call library functions, and serialize output. The complexity is in content curation, prompt design, and deployment -- not in reimplementing testing logic.

## Common Pitfalls

### Pitfall 1: ServerTester Prints to Stdout
**What goes wrong:** Several `ServerTester` and `ScenarioGenerator` methods print directly to stdout using `println!` and `colored` formatting, which pollutes MCP tool responses.
**Why it happens:** These were designed as CLI tools first.
**How to avoid:** Use `run_compliance_tests()`, `run_quick_test()`, `run_full_suite()` which return `TestReport` (serializable). For `ScenarioGenerator`, the `generate()` method writes to a file AND prints -- need to either: (a) capture its output via an in-memory buffer, or (b) use the internal `generate_scenario_struct()` approach to get the `TestScenario` struct directly without file I/O. Check if ScenarioGenerator can return the scenario struct instead of writing a file.
**Warning signs:** Colored text appearing in tool response content.

### Pitfall 2: Circular Dependencies
**What goes wrong:** pmcp-server depends on mcp-tester which depends on pmcp. Adding pmcp-server as a workspace member could create issues if any crate tries to depend on pmcp-server.
**Why it happens:** Diamond dependency patterns in workspace.
**How to avoid:** pmcp-server is a leaf crate (nothing depends on it). Keep it that way. Dependencies flow: pmcp -> mcp-tester -> pmcp-server (terminal).
**Warning signs:** Cargo compilation errors about circular dependencies.

### Pitfall 3: Binary Size from Embedded Content
**What goes wrong:** Embedding too much content inflates the binary, slowing downloads and CI builds.
**Why it happens:** Each `include_str!` adds file content to the binary at compile time.
**How to avoid:** Budget ~200-300KB total for embedded content. Select 10-15 book chapters (the most practical ones: tools, resources, prompts, auth, transports, MCP Apps, testing). Summarize API reference rather than embedding full rustdoc output.
**Warning signs:** Binary size exceeding 30MB (current mcp-tester is ~15MB).

### Pitfall 4: TestReport Contains Duration (not JSON-friendly by default)
**What goes wrong:** `std::time::Duration` serializes as `{"secs": N, "nanos": N}` which is awkward for consumers.
**Why it happens:** Default serde serialization of Duration.
**How to avoid:** The `TestReport` already derives `Serialize` so it works, but consider post-processing to convert durations to human-readable strings or milliseconds in the tool response.
**Warning signs:** Consumers confused by duration format.

### Pitfall 5: Stateful vs Stateless HTTP Configuration
**What goes wrong:** Using stateful (default) config when deploying behind a load balancer or Lambda.
**Why it happens:** `StreamableHttpServerConfig::default()` creates sessions.
**How to avoid:** For pmcp.run deployment, use stateful config (default) since it's likely a single process. Add a `--stateless` CLI flag for serverless deployments. The server should default to stateful.
**Warning signs:** Session ID errors behind load balancers.

### Pitfall 6: mcp-tester ScenarioGenerator Writes Files
**What goes wrong:** `ScenarioGenerator::generate()` writes a YAML file to disk and returns `Result<()>`.
**Why it happens:** Designed for CLI use where file output is the goal.
**How to avoid:** For the `test_generate` tool, we need the generated `TestScenario` struct, not a file. Either: (1) build the scenario manually using the same logic (tools iteration, schema analysis), or (2) use a temp file and read it back, or (3) add a new method to ScenarioGenerator that returns the struct. Option (3) is best but requires modifying mcp-tester's API.
**Warning signs:** Tool trying to write files to server filesystem.

## Code Examples

### Server Main Entry Point
```rust
// Source: pattern from examples/22_streamable_http_server_stateful.rs
use clap::Parser;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::Server;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "pmcp-server", about = "PMCP SDK developer tools MCP server")]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value = "8080", env = "PMCP_SERVER_PORT")]
    port: u16,

    /// Bind address
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("pmcp_server=info,pmcp=warn")
        .init();

    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let server = build_server()?;
    let server = Arc::new(Mutex::new(server));
    let http_server = StreamableHttpServer::new(addr, server);
    let (bound_addr, handle) = http_server.start().await?;

    tracing::info!("PMCP server listening on http://{}", bound_addr);
    handle.await?;
    Ok(())
}
```

### Scaffold Tool Returning Instructions
```rust
// Pattern: return structured JSON with file templates, not write files
fn generate_scaffold_response(template: &str, name: &str) -> Value {
    match template {
        "minimal" => json!({
            "template": "minimal",
            "name": name,
            "instructions": format!(
                "Create a minimal MCP server workspace named '{}'.\n\
                 The workspace has two crates: a core library and a server binary.",
                name
            ),
            "files": [
                {
                    "path": format!("Cargo.toml"),
                    "content": WORKSPACE_CARGO_TOML_TEMPLATE.replace("{name}", name),
                },
                {
                    "path": format!("crates/mcp-{}-core/src/lib.rs", name),
                    "content": MINIMAL_CORE_TEMPLATE.replace("{name}", name),
                },
                {
                    "path": format!("{}-server/src/main.rs", name),
                    "content": MINIMAL_SERVER_TEMPLATE.replace("{name}", name),
                },
            ]
        }),
        // ... other templates
        _ => json!({"error": format!("Unknown template: {}", template)})
    }
}
```

### Using AppValidator from Tool Handler
```rust
// Source: crates/mcp-tester/src/app_validator.rs API
async fn handle_test_apps(url: &str, mode: &str, tool_filter: Option<&str>) -> pmcp::Result<Value> {
    let validation_mode: AppValidationMode = mode.parse()
        .map_err(|e: String| pmcp::Error::validation(e))?;

    let mut tester = ServerTester::new(
        url, Duration::from_secs(30), false, None, None, None,
    ).map_err(|e| pmcp::Error::Internal(e.to_string()))?;

    // Initialize and discover
    tester.run_quick_test().await
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

    let tools = tester.get_tools().cloned().unwrap_or_default();
    let resources = tester.list_resources().await
        .map(|r| r.resources)
        .unwrap_or_default();

    let validator = AppValidator::new(validation_mode, tool_filter.map(String::from));
    let results = validator.validate_tools(&tools, &resources);

    Ok(serde_json::to_value(&results)?)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SSE transport | Streamable HTTP | MCP spec 2024-11-05 | Server uses StreamableHttpServer, not old SSE adapter |
| Flat _meta keys | Nested ui.resourceUri + openai/* | Phase 45 (2026-03) | AppValidator already handles both modes |
| outputSchema in annotations | outputSchema top-level on ToolInfo | Phase 42 (2026-03) | Schema export tool should emit top-level outputSchema |
| Single binary (mcp-tester only) | Multi-binary release workflow | Phase 50 (2026-03) | release-binary.yml is parameterized, just add pmcp-server |

**Deprecated/outdated:**
- SSE-only transport: Use streamable HTTP
- Flat ui/resourceUri metadata: Nested ui.resourceUri is now standard

## Open Questions

1. **ScenarioGenerator File-Write Problem**
   - What we know: `ScenarioGenerator::generate()` writes YAML to disk and prints to stdout.
   - What's unclear: Whether we should modify mcp-tester to add a `generate_to_struct()` method, or work around it with temp files.
   - Recommendation: Add a `pub fn generate_scenario(&self, tester: &mut ServerTester) -> Result<TestScenario>` method to ScenarioGenerator in mcp-tester. This is a small API addition that benefits both the server and future library consumers.

2. **Scaffold Template Content Extraction**
   - What we know: Templates exist as Rust format strings in cargo-pmcp/src/templates/*.rs. They use `format!()` with name substitution.
   - What's unclear: Whether to copy the template strings into pmcp-server or create a shared `pmcp-templates` crate.
   - Recommendation: Copy the template content strings directly into pmcp-server's `content/` directory as markdown/rust files with `{name}` placeholders. The templates are stable and small. Avoid creating another crate for ~500 lines of template strings.

3. **Book Content Selection**
   - What we know: 47 chapters, 640KB total. Need to select 10-15 most useful.
   - What's unclear: Exact chapter list to embed.
   - Recommendation: Include chapters most useful for AI agent context: ch02 (first server), ch05 (tools), ch06 (resources), ch07 (prompts), ch08 (error handling), ch09 (auth), ch10-03 (streamable HTTP), ch11 (middleware), ch12-5 (MCP Apps), ch15 (testing), ch18 (patterns). ~12 chapters, estimated ~180KB.

4. **Server Registry Configuration**
   - What we know: server.json at repo root configures registry publishing. The server needs its own registry entry.
   - What's unclear: Whether the same mcp-publisher workflow handles multiple servers from one repo, or if we need a separate server.json.
   - Recommendation: Create a separate server.json for pmcp-server or add it to the existing publishing config. Research mcp-publisher multi-server support during implementation.

5. **Hosted Instance Configuration**
   - What we know: Server runs on pmcp.run. Need port, host binding, and potentially rate limiting.
   - What's unclear: Deployment infrastructure (Lambda? EC2? Fly.io?).
   - Recommendation: Build the binary to listen on configurable port via `--port` / `PMCP_SERVER_PORT` env var, bind `0.0.0.0`. Deployment infrastructure is separate from the binary -- don't embed deployment-specific logic. Keep it a simple HTTP server binary.

## Content Embedding Strategy

### Recommended Approach: `include_str!` with Curated Content

Create a `content/` directory alongside `src/` in the pmcp-server crate with curated markdown files. Use `include_str!` to embed them at compile time.

**Rationale:**
- `include_str!` is zero-dependency, compile-time, and the files are individual curated documents (not a directory tree).
- `rust-embed` (used by mcp-preview) is better for directories of assets with MIME type guessing -- overkill for text documents.
- The content is curated per-topic, not a dump of the full book.

### Content Organization
```rust
// src/content/mod.rs
pub mod sdk {
    pub const TYPED_TOOLS: &str = include_str!("../../content/sdk-typed-tools.md");
    pub const RESOURCES: &str = include_str!("../../content/sdk-resources.md");
    pub const PROMPTS: &str = include_str!("../../content/sdk-prompts.md");
    pub const AUTH: &str = include_str!("../../content/sdk-auth.md");
    pub const MIDDLEWARE: &str = include_str!("../../content/sdk-middleware.md");
    pub const MCP_APPS: &str = include_str!("../../content/sdk-mcp-apps.md");
    pub const ERROR_HANDLING: &str = include_str!("../../content/sdk-error-handling.md");
}

pub mod cli {
    pub const GUIDE: &str = include_str!("../../content/cli-guide.md");
}

pub mod book {
    pub const CH02_FIRST_SERVER: &str = include_str!("../../content/book-ch02.md");
    pub const CH05_TOOLS: &str = include_str!("../../content/book-ch05.md");
    // ... selected chapters
}

pub mod practices {
    pub const BEST_PRACTICES: &str = include_str!("../../content/best-practices.md");
    pub const EXAMPLES: &str = include_str!("../../content/example-patterns.md");
}
```

### Resource URI Mapping
| URI | Content Source | Description |
|-----|---------------|-------------|
| `pmcp://docs/typed-tools` | sdk-typed-tools.md | TypedTool, TypedSyncTool, TypedToolWithOutput |
| `pmcp://docs/resources` | sdk-resources.md | ResourceHandler, dynamic resources |
| `pmcp://docs/prompts` | sdk-prompts.md | PromptHandler, workflow prompts |
| `pmcp://docs/auth` | sdk-auth.md | OAuth, JWT, API key auth |
| `pmcp://docs/middleware` | sdk-middleware.md | Tool and protocol middleware |
| `pmcp://docs/mcp-apps` | sdk-mcp-apps.md | MCP Apps extension, widgets |
| `pmcp://docs/error-handling` | sdk-error-handling.md | Error types, handling patterns |
| `pmcp://docs/cli` | cli-guide.md | cargo-pmcp commands reference |
| `pmcp://docs/best-practices` | best-practices.md | Patterns and conventions |
| `pmcp://book/first-server` | book-ch02.md | Getting started tutorial |
| `pmcp://book/tools` | book-ch05.md | Tools deep dive |
| `pmcp://book/resources` | book-ch06.md | Resources deep dive |
| `pmcp://book/prompts` | book-ch07.md | Prompts deep dive |
| `pmcp://book/auth` | book-ch09.md | Auth and security |
| `pmcp://book/streamable-http` | book-ch10-03.md | Streamable HTTP transport |
| `pmcp://book/middleware` | book-ch11.md | Middleware patterns |
| `pmcp://book/mcp-apps` | book-ch12-5.md | MCP Apps extension |
| `pmcp://book/testing` | book-ch15.md | Testing strategies |
| `pmcp://book/patterns` | book-ch18.md | Production patterns |

## Binary Release Integration

### Current release-binary.yml
The workflow is already parameterized with `package_name` input. Adding pmcp-server requires:

1. Add to `release.yml` a new workflow call:
```yaml
build-pmcp-server:
  name: Build PMCP Server Binaries
  needs: create-release
  uses: ./.github/workflows/release-binary.yml
  with:
    tag_name: ${{ needs.create-release.outputs.version }}
    package_name: pmcp-server
  secrets: inherit
```

2. Update `release-binary.yml` package choices for manual dispatch:
```yaml
options:
  - mcp-tester
  - pmcp-server
```

3. Add binstall metadata to pmcp-server's Cargo.toml:
```toml
[package.metadata.binstall]
pkg-url = "https://github.com/paiml/rust-mcp-sdk/releases/download/v{ version }/{ name }-{ target }{ binary-ext }"
pkg-fmt = "bin"
```

### Workspace Integration
Add to root Cargo.toml workspace members:
```toml
members = ["...", "crates/pmcp-server"]
```

Add to publish order in CLAUDE.md:
```
1. pmcp-widget-utils
2. pmcp
3. mcp-tester
4. mcp-preview
5. pmcp-server (depends on pmcp, mcp-tester)
6. cargo-pmcp (depends on pmcp, mcp-tester, mcp-preview)
```

## Prompt Template Design

### Recommended Prompts
| Name | Description | Arguments |
|------|-------------|-----------|
| `create-mcp-server` | Step-by-step guide to create a new MCP server | `name` (required), `template` (optional: minimal/calculator/complete) |
| `add-tool` | Guide to add a new tool to an existing server | `tool_name` (required), `description` (optional) |
| `add-resource` | Guide to add a resource handler | `resource_type` (optional: static/dynamic) |
| `debug-protocol-error` | Diagnose MCP protocol issues | `error_message` (optional), `server_url` (optional) |
| `setup-auth` | Configure authentication for an MCP server | `auth_type` (optional: oauth/api-key/jwt) |
| `quickstart` | Get started with PMCP SDK in 5 minutes | none |
| `diagnose` | Diagnose a server that's not working | `server_url` (required) |
| `migrate` | Migrate from TypeScript SDK to PMCP | `migration_type` (optional: tools/resources/full) |

### Prompt Implementation Pattern
```rust
struct QuickstartPrompt;

#[async_trait]
impl PromptHandler for QuickstartPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult {
            description: Some("Get started with PMCP SDK in 5 minutes".to_string()),
            messages: vec![
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: "I want to create a new MCP server using the PMCP Rust SDK. \
                               Guide me through the quickstart process.".to_string(),
                    },
                },
                PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Text {
                        text: QUICKSTART_INSTRUCTIONS.to_string(), // embedded content
                    },
                },
            ],
        })
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "quickstart".to_string(),
            description: Some("Get started with PMCP SDK in 5 minutes".to_string()),
            arguments: None,
        })
    }
}
```

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/mcp-tester/src/lib.rs` -- ServerTester, AppValidator, ScenarioGenerator public API
- Codebase: `crates/mcp-tester/src/tester.rs` -- Full testing API surface (run_compliance_tests, run_full_suite, etc.)
- Codebase: `crates/mcp-tester/src/app_validator.rs` -- AppValidator::validate_tools() and AppValidationMode
- Codebase: `crates/mcp-tester/src/report.rs` -- TestReport, TestResult (Serialize/Deserialize)
- Codebase: `src/server/streamable_http_server.rs` -- StreamableHttpServer::new(), start(), config
- Codebase: `src/server/builder.rs` -- ServerCoreBuilder API (.tool(), .prompt(), .resources())
- Codebase: `src/server/traits.rs` -- ToolHandler, ResourceHandler, PromptHandler traits
- Codebase: `examples/22_streamable_http_server_stateful.rs` -- Complete server setup pattern
- Codebase: `cargo-pmcp/src/commands/schema.rs` -- Schema export types (McpSchema, ToolSchema)
- Codebase: `cargo-pmcp/src/templates/` -- Scaffold template modules
- Codebase: `.github/workflows/release-binary.yml` -- Parameterized binary release workflow
- Codebase: `.github/workflows/release.yml` -- Full release pipeline with crate publishing

### Secondary (MEDIUM confidence)
- [Official MCP Registry](https://registry.modelcontextprotocol.io/) -- ~30 servers listed, no developer tools category
- [rust-embed crate](https://crates.io/crates/rust-embed) -- v8, directory embedding with dev-mode FS reads
- [include_str! docs](https://doc.rust-lang.org/std/macro.include_str.html) -- Compile-time file embedding

### Tertiary (LOW confidence)
- [MCP Registry GitHub](https://github.com/modelcontextprotocol/registry) -- mcp-publisher configuration format
- [PulseMCP Server Scaffold](https://www.pulsemcp.com/servers/maoxiaoke-create-server-scaffold) -- Existing scaffold server (npx-based, TypeScript, limited scope)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries are already used in the workspace
- Architecture: HIGH -- patterns directly from existing codebase examples and traits
- Pitfalls: HIGH -- identified from reading actual source code (stdout printing, file writes, etc.)
- Content strategy: MEDIUM -- book chapter selection is a judgment call, may need adjustment
- Registry positioning: MEDIUM -- registry scan was limited by dynamic content loading

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- core APIs unlikely to change)
