//! `cargo pmcp loadtest init` command implementation.

use anyhow::Result;
use std::time::Duration;

use cargo_pmcp::loadtest::client::McpClient;

use crate::commands::GlobalFlags;

/// Execute the `loadtest init` command.
///
/// Creates `.pmcp/loadtest.toml` with sensible defaults. If a server URL
/// is provided, connects to discover tools/resources/prompts and populates
/// the scenario with real names.
pub async fn execute_init(
    url: Option<String>,
    force: bool,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let config_dir = std::env::current_dir()?.join(".pmcp");
    let config_path = config_dir.join("loadtest.toml");

    // Check for existing file
    if config_path.exists() && !force {
        anyhow::bail!(
            "Config file already exists: {}\n\
             Use `--force` to overwrite.",
            config_path.display()
        );
    }

    // Generate config content
    let content = if let Some(server_url) = url {
        if global_flags.should_output() {
            eprintln!("Discovering server schema at {}...", server_url);
        }
        match discover_schema(&server_url).await {
            Ok(schema) => generate_discovered_template(&server_url, &schema),
            Err(e) => {
                if global_flags.should_output() {
                    eprintln!(
                        "Warning: Could not discover server schema: {}\n\
                         Generating default template instead.",
                        e
                    );
                }
                generate_default_template()
            },
        }
    } else {
        generate_default_template()
    };

    // Create directory if needed
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }

    // Write config file
    std::fs::write(&config_path, &content)?;
    if global_flags.should_output() {
        eprintln!("Created {}", config_path.display());
        eprintln!("Edit the file to customize your load test scenario.");
    }

    Ok(())
}

/// Discovered server schema for template generation.
struct DiscoveredSchema {
    tools: Vec<DiscoveredTool>,
    resources: Vec<DiscoveredResource>,
    prompts: Vec<DiscoveredPrompt>,
}

struct DiscoveredTool {
    name: String,
}

struct DiscoveredResource {
    uri: String,
}

struct DiscoveredPrompt {
    name: String,
}

/// Connect to a server and discover its available tools, resources, and prompts.
async fn discover_schema(url: &str) -> Result<DiscoveredSchema> {
    let http = reqwest::Client::new();
    let timeout = Duration::from_secs(10);
    let mut client = McpClient::new(http, url.to_owned(), timeout, None);

    // Initialize session
    client
        .initialize()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to server: {}", e))?;

    // Extract URL and session ID for direct HTTP requests
    let base_url = client.base_url().to_owned();
    let session_id = client.session_id().map(|s| s.to_owned());

    // Discover tools via tools/list
    let tools = discover_tools(&base_url, session_id.as_deref()).await;
    let resources = discover_resources(&base_url, session_id.as_deref()).await;
    let prompts = discover_prompts(&base_url, session_id.as_deref()).await;

    Ok(DiscoveredSchema {
        tools,
        resources,
        prompts,
    })
}

/// Send a JSON-RPC list request and extract items from the response.
///
/// Constructs the request body manually and sends via direct HTTP POST,
/// since [`McpClient`] does not expose list methods.
async fn send_list_request(
    url: &str,
    session_id: Option<&str>,
    _method: &str,
    body: &serde_json::Value,
) -> Option<serde_json::Value> {
    let http = reqwest::Client::new();

    let mut request = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .timeout(Duration::from_secs(10))
        .json(body);

    if let Some(sid) = session_id {
        request = request.header("mcp-session-id", sid);
    }

    let response = request.send().await.ok()?;
    let bytes = response.bytes().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    parsed.get("result").cloned()
}

/// Discover tools via `tools/list`.
async fn discover_tools(url: &str, session_id: Option<&str>) -> Vec<DiscoveredTool> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "tools/list",
        "params": {}
    });

    let result = match send_list_request(url, session_id, "tools/list", &body).await {
        Some(r) => r,
        None => return Vec::new(),
    };

    result
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| DiscoveredTool { name: n.to_owned() })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Discover resources via `resources/list`.
async fn discover_resources(url: &str, session_id: Option<&str>) -> Vec<DiscoveredResource> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "resources/list",
        "params": {}
    });

    let result = match send_list_request(url, session_id, "resources/list", &body).await {
        Some(r) => r,
        None => return Vec::new(),
    };

    result
        .get("resources")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("uri")
                        .and_then(|u| u.as_str())
                        .map(|u| DiscoveredResource { uri: u.to_owned() })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Discover prompts via `prompts/list`.
async fn discover_prompts(url: &str, session_id: Option<&str>) -> Vec<DiscoveredPrompt> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "prompts/list",
        "params": {}
    });

    let result = match send_list_request(url, session_id, "prompts/list", &body).await {
        Some(r) => r,
        None => return Vec::new(),
    };

    result
        .get("prompts")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| DiscoveredPrompt { name: n.to_owned() })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Default ramp-up stage block appended to all generated loadtest configs.
///
/// Provides a 3-phase profile: ramp to 5 VUs (10s), ramp to 10 VUs (10s),
/// hold at 10 VUs (60s). Prevents cold-start thundering herd on cloud execution.
const DEFAULT_STAGES_TOML: &str = r#"
# Ramp-up stages: gradual VU increase avoids cold-start thundering herd.
# Critical for cloud load testing (cargo pmcp loadtest upload) where distributed
# workers amplify cold-start effects across multiple service layers.
# Without stages, all VUs launch simultaneously (flat load).
# Modify target_vus/duration_secs to match your capacity, or delete to use flat load.

[[stage]]
target_vus = 5
duration_secs = 10

[[stage]]
target_vus = 10
duration_secs = 10

# Hold at peak
[[stage]]
target_vus = 10
duration_secs = 60
"#;

/// Generate a default TOML template without server discovery.
fn generate_default_template() -> String {
    let mut content = r#"# Load test configuration for cargo-pmcp
# See: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp#load-testing

[settings]
# Number of concurrent virtual users
virtual_users = 10

# Test duration in seconds
duration_secs = 60

# Per-request timeout in milliseconds
timeout_ms = 5000

# Expected interval between requests (ms) for coordinated omission correction
# expected_interval_ms = 100

# Optional delay between requests per VU (ms). Omit for closed-loop (fire as fast as possible).
# Example: 15000 = one request every 15s per VU (50 VUs x 4 req/min = ~200 req/min)
# request_interval_ms = 15000

# Define your scenario steps below. Each step has a type, weight, and parameters.
# Weights determine the relative frequency of each operation.

[[scenario]]
type = "tools/call"
weight = 70
tool = "your-tool-name"
# arguments = { key = "value" }

# [[scenario]]
# type = "resources/read"
# weight = 20
# uri = "file:///your/resource/uri"

# [[scenario]]
# type = "prompts/get"
# weight = 10
# prompt = "your-prompt-name"
# arguments = { key = "value" }
"#
    .to_string();
    content.push_str(DEFAULT_STAGES_TOML);
    content
}

/// Generate a TOML template populated from discovered server schema.
fn generate_discovered_template(url: &str, schema: &DiscoveredSchema) -> String {
    let mut content = format!(
        r#"# Load test configuration for cargo-pmcp
# Generated from server: {}
# See: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp#load-testing

[settings]
# Number of concurrent virtual users
virtual_users = 10

# Test duration in seconds
duration_secs = 60

# Per-request timeout in milliseconds
timeout_ms = 5000

# Expected interval between requests (ms) for coordinated omission correction
# expected_interval_ms = 100

# Optional delay between requests per VU (ms). Omit for closed-loop (fire as fast as possible).
# Example: 15000 = one request every 15s per VU (50 VUs x 4 req/min = ~200 req/min)
# request_interval_ms = 15000

# Scenario steps discovered from server capabilities.
# Adjust weights to control the mix of operations.
"#,
        url
    );

    // Calculate weights: tools get majority, resources and prompts split remainder
    let has_tools = !schema.tools.is_empty();
    let has_resources = !schema.resources.is_empty();
    let has_prompts = !schema.prompts.is_empty();

    let tool_weight: u32 = if has_tools { 70 } else { 0 };
    let resource_weight: u32 = if has_resources { 20 } else { 0 };
    let prompt_weight: u32 = if has_prompts { 10 } else { 0 };

    // Normalize weights if some categories are empty
    let total = tool_weight + resource_weight + prompt_weight;
    let (tw, rw, pw) = if total > 0 {
        (
            tool_weight * 100 / total,
            resource_weight * 100 / total,
            prompt_weight * 100 / total,
        )
    } else {
        (100, 0, 0)
    };

    // Add tool steps
    for tool in &schema.tools {
        let per_tool_weight = tw / schema.tools.len().max(1) as u32;
        content.push_str(&format!(
            r#"
[[scenario]]
type = "tools/call"
weight = {}
tool = "{}"
# arguments = {{}}
"#,
            per_tool_weight, tool.name
        ));
    }

    // Add resource steps
    for resource in &schema.resources {
        let per_resource_weight = rw / schema.resources.len().max(1) as u32;
        content.push_str(&format!(
            r#"
[[scenario]]
type = "resources/read"
weight = {}
uri = "{}"
"#,
            per_resource_weight, resource.uri
        ));
    }

    // Add prompt steps
    for prompt in &schema.prompts {
        let per_prompt_weight = pw / schema.prompts.len().max(1) as u32;
        content.push_str(&format!(
            r#"
[[scenario]]
type = "prompts/get"
weight = {}
prompt = "{}"
# arguments = {{}}
"#,
            per_prompt_weight, prompt.name
        ));
    }

    // Add default ramp-up stages
    content.push_str(DEFAULT_STAGES_TOML);

    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_default_template_is_valid_toml_comment_structure() {
        let template = generate_default_template();
        assert!(template.contains("[settings]"));
        assert!(template.contains("virtual_users"));
        assert!(template.contains("duration_secs"));
        assert!(template.contains("timeout_ms"));
        assert!(template.contains("[[scenario]]"));
        assert!(template.contains("tools/call"));
        assert!(template.contains("[[stage]]"));
        assert!(template.contains("target_vus = 5"));
        assert!(template.contains("target_vus = 10"));
        assert!(template.contains("cold-start thundering herd"));
    }

    #[test]
    fn test_generate_discovered_template_with_tools() {
        let schema = DiscoveredSchema {
            tools: vec![
                DiscoveredTool {
                    name: "echo".to_string(),
                },
                DiscoveredTool {
                    name: "calculate".to_string(),
                },
            ],
            resources: vec![],
            prompts: vec![],
        };
        let template = generate_discovered_template("http://localhost:3000/mcp", &schema);
        assert!(template.contains("echo"));
        assert!(template.contains("calculate"));
        assert!(template.contains("[settings]"));
        assert!(template.contains("[[stage]]"));
        assert!(template.contains("target_vus = 5"));
    }

    #[test]
    fn test_generate_discovered_template_with_all_types() {
        let schema = DiscoveredSchema {
            tools: vec![DiscoveredTool {
                name: "search".to_string(),
            }],
            resources: vec![DiscoveredResource {
                uri: "file:///data.json".to_string(),
            }],
            prompts: vec![DiscoveredPrompt {
                name: "summarize".to_string(),
            }],
        };
        let template = generate_discovered_template("http://localhost:3000/mcp", &schema);
        assert!(template.contains("search"));
        assert!(template.contains("file:///data.json"));
        assert!(template.contains("summarize"));
        assert!(template.contains("tools/call"));
        assert!(template.contains("resources/read"));
        assert!(template.contains("prompts/get"));
        assert!(template.contains("[[stage]]"));
        assert!(template.contains("cold-start thundering herd"));
    }
}
