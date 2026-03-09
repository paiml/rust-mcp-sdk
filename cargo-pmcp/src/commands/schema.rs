//! Schema discovery and management commands
//!
//! - `export`: Export schema from an MCP server endpoint
//! - `validate`: Validate a local schema file
//! - `diff`: Compare local schema with live server

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use console::style;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Subcommand)]
pub enum SchemaCommand {
    /// Export schema from an MCP server endpoint
    Export {
        /// MCP server endpoint URL (e.g., https://db-demo.us-east.true-mcp.com/mcp)
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Server ID on pmcp.run (alternative to --endpoint)
        #[arg(short, long)]
        server: Option<String>,

        /// Output file path (default: schemas/<server_id>.json)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Validate a schema file
    Validate {
        /// Schema file to validate
        schema: String,
    },

    /// Show diff between local schema and live server
    Diff {
        /// Local schema file
        schema: String,

        /// MCP server endpoint to compare against
        #[arg(short, long)]
        endpoint: String,
    },
}

impl SchemaCommand {
    pub fn execute(self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let quiet = global_flags.quiet;
        runtime.block_on(async {
            match self {
                SchemaCommand::Export {
                    endpoint,
                    server,
                    output,
                } => export(endpoint, server, output, quiet).await,
                SchemaCommand::Validate { schema } => validate(&schema, quiet).await,
                SchemaCommand::Diff { schema, endpoint } => diff(&schema, &endpoint, quiet).await,
            }
        })
    }
}

/// MCP server schema for code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSchema {
    /// JSON Schema identifier
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,

    /// Server ID (used for module naming)
    pub server_id: String,

    /// Display name
    pub name: String,

    /// Server description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Server version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Endpoint URL (for reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// Server tier (foundation/domain)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,

    /// Tool definitions
    #[serde(default)]
    pub tools: Vec<ToolSchema>,

    /// Resource definitions
    #[serde(default)]
    pub resources: Vec<ResourceSchema>,

    /// Prompt definitions
    #[serde(default)]
    pub prompts: Vec<PromptSchema>,

    /// Export metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Input schema - deserializes from MCP's "inputSchema" or our "input_schema"
    #[serde(
        default,
        alias = "inputSchema",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_schema: Option<Value>,
    /// Output schema - top-level per MCP spec 2025-06-18
    #[serde(
        default,
        alias = "outputSchema",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

/// Tool annotations supporting both MCP standard and PMCP extensions.
///
/// When deserializing from MCP responses, this maps:
/// - `readOnlyHint` -> `read_only`
/// - `destructiveHint` -> `destructive`
/// - `idempotentHint` -> `idempotent`
/// - `openWorldHint` -> `open_world`
/// - `pmcp:outputTypeName` -> `output_type_name` (PMCP extension)
///
/// Note: `outputSchema` is now a top-level field on `ToolSchema` per MCP spec 2025-06-18.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Tool does not modify state
    #[serde(alias = "read_only", skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    /// Tool may perform destructive operations
    #[serde(alias = "destructive", skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    /// Tool is idempotent (same args = same result)
    #[serde(alias = "idempotent", skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    /// Tool interacts with external systems
    #[serde(alias = "open_world", skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,

    // -------------------------------------------------------------------------
    // PMCP Extensions for Type-Safe Composition
    // -------------------------------------------------------------------------
    /// Name of the output type for code generation (PMCP extension).
    ///
    /// Example: "QueryResult" generates `struct QueryResult { ... }`
    #[serde(
        rename = "pmcp:outputTypeName",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_type_name: Option<String>,

    // Legacy/simplified fields for internal use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSchema {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<PromptArgSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// MCP JSON-RPC request
#[derive(Debug, Serialize)]
struct McpRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// MCP JSON-RPC response
#[derive(Debug, Deserialize)]
struct McpResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<Value>,
    error: Option<McpError>,
}

#[derive(Debug, Deserialize)]
struct McpError {
    code: i32,
    message: String,
}

/// Export schema from an MCP server
async fn export(
    endpoint: Option<String>,
    server: Option<String>,
    output: Option<String>,
    quiet: bool,
) -> Result<()> {
    // Determine endpoint URL
    let endpoint_url = match (&endpoint, &server) {
        (Some(url), _) => url.clone(),
        (None, Some(server_id)) => {
            // Construct pmcp.run endpoint
            format!("https://api.pmcp.run/{}/mcp", server_id)
        },
        (None, None) => {
            return Err(anyhow!(
                "Either --endpoint or --server must be specified\n\n\
                 Examples:\n  \
                 cargo pmcp schema export --endpoint https://mcp.example.com\n  \
                 cargo pmcp schema export --server db-demo"
            ));
        },
    };

    if !quiet {
        println!(
            "{} Exporting schema from {}",
            style("->").cyan().bold(),
            style(&endpoint_url).yellow()
        );
    }

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    // Initialize MCP session
    if !quiet {
        println!("  {} Initializing MCP session...", style("*").dim());
    }
    let init_response = send_mcp_request(
        &client,
        &endpoint_url,
        "initialize",
        Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "cargo-pmcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
    )
    .await?;

    // Extract server info
    let server_info = init_response
        .get("serverInfo")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let server_name = server_info
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let server_version = server_info
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Derive server_id from name or endpoint
    let server_id = server.clone().unwrap_or_else(|| slugify(&server_name));

    // Send initialized notification (fire and forget)
    let _ = client
        .post(&endpoint_url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .send()
        .await;

    // Fetch tools
    if !quiet {
        println!("  {} Fetching tools...", style("*").dim());
    }
    let tools_response = send_mcp_request(&client, &endpoint_url, "tools/list", None).await?;
    let tools: Vec<ToolSchema> = tools_response
        .get("tools")
        .and_then(|t| serde_json::from_value(t.clone()).ok())
        .unwrap_or_default();
    if !quiet {
        println!(
            "    {} Found {} tools",
            style("OK").green(),
            style(tools.len()).bold()
        );
    }

    // Fetch resources
    if !quiet {
        println!("  {} Fetching resources...", style("*").dim());
    }
    let resources_response = send_mcp_request(&client, &endpoint_url, "resources/list", None).await;
    let resources: Vec<ResourceSchema> = resources_response
        .ok()
        .and_then(|r| r.get("resources").cloned())
        .and_then(|r| serde_json::from_value(r).ok())
        .unwrap_or_default();
    if !quiet {
        println!(
            "    {} Found {} resources",
            style("OK").green(),
            style(resources.len()).bold()
        );
    }

    // Fetch prompts
    if !quiet {
        println!("  {} Fetching prompts...", style("*").dim());
    }
    let prompts_response = send_mcp_request(&client, &endpoint_url, "prompts/list", None).await;
    let prompts: Vec<PromptSchema> = prompts_response
        .ok()
        .and_then(|r| r.get("prompts").cloned())
        .and_then(|r| serde_json::from_value(r).ok())
        .unwrap_or_default();
    if !quiet {
        println!(
            "    {} Found {} prompts",
            style("OK").green(),
            style(prompts.len()).bold()
        );
    }

    // Build schema
    let schema = McpSchema {
        schema_url: Some("https://pmcp.run/schemas/mcp-foundation-v1.json".to_string()),
        server_id: server_id.clone(),
        name: server_name,
        description: None,
        version: server_version,
        endpoint: Some(endpoint_url),
        tier: Some("foundation".to_string()),
        tools,
        resources,
        prompts,
        exported_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    // Determine output path
    let output_path = output.unwrap_or_else(|| format!("schemas/{}.json", server_id));

    // Create parent directories
    if let Some(parent) = Path::new(&output_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write schema
    let schema_json =
        serde_json::to_string_pretty(&schema).context("Failed to serialize schema")?;
    std::fs::write(&output_path, &schema_json)
        .with_context(|| format!("Failed to write schema to {}", output_path))?;

    if !quiet {
        println!();
        println!(
            "{} Schema exported to {}",
            style("OK").green().bold(),
            style(&output_path).cyan()
        );
        println!();
        println!(
            "  Server: {} v{}",
            style(&schema.name).bold(),
            style(schema.version.as_deref().unwrap_or("?")).dim()
        );
        println!("  Tools: {}", style(schema.tools.len()).bold());
        println!("  Resources: {}", style(schema.resources.len()).bold());
        println!("  Prompts: {}", style(schema.prompts.len()).bold());
        println!();
        println!("Next steps:");
        println!(
            "  1. Review and customize: {}",
            style(&output_path).yellow()
        );
        println!(
            "  2. Generate typed client: {}",
            style(format!("cargo pmcp generate foundation {}", output_path)).yellow()
        );
    }

    Ok(())
}

/// Validate a schema file
async fn validate(schema_path: &str, quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{} Validating schema: {}",
            style("->").cyan().bold(),
            style(schema_path).yellow()
        );
    }

    // Read schema file
    let content = std::fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file: {}", schema_path))?;

    // Parse as JSON
    let schema: McpSchema =
        serde_json::from_str(&content).with_context(|| "Failed to parse schema JSON")?;

    // Validate required fields
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if schema.server_id.is_empty() {
        errors.push("server_id is required".to_string());
    }
    if schema.name.is_empty() {
        errors.push("name is required".to_string());
    }
    if schema.tools.is_empty() && schema.resources.is_empty() && schema.prompts.is_empty() {
        warnings.push("Schema has no tools, resources, or prompts".to_string());
    }

    // Check tools
    for (i, tool) in schema.tools.iter().enumerate() {
        if tool.name.is_empty() {
            errors.push(format!("tools[{}].name is required", i));
        }
    }

    // Report results -- validation errors are important output (always show)
    if !warnings.is_empty() {
        for warning in &warnings {
            println!("  {} {}", style("WARN").yellow(), warning);
        }
    }

    if !errors.is_empty() {
        for error in &errors {
            println!("  {} {}", style("ERR").red(), error);
        }
        return Err(anyhow!(
            "Schema validation failed with {} errors",
            errors.len()
        ));
    }

    // Success output is decorative
    if !quiet {
        println!("{} Schema is valid", style("OK").green().bold());
        println!(
            "  Server: {} ({})",
            style(&schema.name).bold(),
            schema.server_id
        );
        println!(
            "  Tools: {}, Resources: {}, Prompts: {}",
            schema.tools.len(),
            schema.resources.len(),
            schema.prompts.len()
        );
    }

    Ok(())
}

/// Compare local schema with live server
async fn diff(schema_path: &str, endpoint: &str, quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{} Comparing {} with {}",
            style("->").cyan().bold(),
            style(schema_path).yellow(),
            style(endpoint).yellow()
        );
    }

    // Read local schema
    let local_content = std::fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file: {}", schema_path))?;
    let local: McpSchema =
        serde_json::from_str(&local_content).with_context(|| "Failed to parse local schema")?;

    // Export remote schema
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Initialize and fetch remote
    send_mcp_request(
        &client,
        endpoint,
        "initialize",
        Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "cargo-pmcp", "version": env!("CARGO_PKG_VERSION") }
        })),
    )
    .await?;

    let tools_response = send_mcp_request(&client, endpoint, "tools/list", None).await?;
    let remote_tools: Vec<ToolSchema> = tools_response
        .get("tools")
        .and_then(|t| serde_json::from_value(t.clone()).ok())
        .unwrap_or_default();

    // Compare tools
    let local_tool_names: std::collections::HashSet<_> =
        local.tools.iter().map(|t| &t.name).collect();
    let remote_tool_names: std::collections::HashSet<_> =
        remote_tools.iter().map(|t| &t.name).collect();

    let added: Vec<_> = remote_tool_names.difference(&local_tool_names).collect();
    let removed: Vec<_> = local_tool_names.difference(&remote_tool_names).collect();

    println!();
    if added.is_empty() && removed.is_empty() {
        println!("{} No differences found", style("OK").green().bold());
    } else {
        if !added.is_empty() {
            println!("{} Added tools:", style("+").green());
            for name in added {
                println!("  {} {}", style("+").green(), name);
            }
        }
        if !removed.is_empty() {
            println!("{} Removed tools:", style("-").red());
            for name in removed {
                println!("  {} {}", style("-").red(), name);
            }
        }
        if !quiet {
            println!();
            println!(
                "Run {} to update local schema",
                style(format!(
                    "cargo pmcp schema export --endpoint {} --output {}",
                    endpoint, schema_path
                ))
                .yellow()
            );
        }
    }

    Ok(())
}

/// Send an MCP JSON-RPC request
async fn send_mcp_request(
    client: &reqwest::Client,
    endpoint: &str,
    method: &str,
    params: Option<Value>,
) -> Result<Value> {
    let request = McpRequest {
        jsonrpc: "2.0",
        id: 1,
        method: method.to_string(),
        params,
    };

    let response = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to send request to {}", endpoint))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Server returned error: {} {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let mcp_response: McpResponse = response
        .json()
        .await
        .with_context(|| "Failed to parse MCP response")?;

    if let Some(error) = mcp_response.error {
        return Err(anyhow!("MCP error {}: {}", error.code, error.message));
    }

    mcp_response
        .result
        .ok_or_else(|| anyhow!("Empty result from MCP server"))
}

/// Convert a string to a URL-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
