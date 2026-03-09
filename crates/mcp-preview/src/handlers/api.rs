//! API handlers for tool and resource operations

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::proxy::{ToolCallResult, ToolInfo};
use crate::server::AppState;

/// Configuration response
#[derive(Serialize)]
pub struct ConfigResponse {
    pub mcp_url: String,
    pub theme: String,
    pub locale: String,
    pub initial_tool: Option<String>,
    pub mode: String,
    pub descriptor_keys: Vec<String>,
    pub invocation_keys: Vec<String>,
}

/// Get preview configuration
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        mcp_url: state.config.mcp_url.clone(),
        theme: state.config.theme.clone(),
        locale: state.config.locale.clone(),
        initial_tool: state.config.initial_tool.clone(),
        mode: state.config.mode.to_string(),
        // Mirror of pmcp::types::ui::CHATGPT_DESCRIPTOR_KEYS
        descriptor_keys: vec![
            "openai/outputTemplate".into(),
            "openai/toolInvocation/invoking".into(),
            "openai/toolInvocation/invoked".into(),
            "openai/widgetAccessible".into(),
        ],
        invocation_keys: vec![
            "openai/toolInvocation/invoking".into(),
            "openai/toolInvocation/invoked".into(),
        ],
    })
}

/// Tools list response
#[derive(Serialize)]
pub struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List available tools from the MCP server
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolsResponse>, (StatusCode, String)> {
    match state.proxy.list_tools().await {
        Ok(tools) => Ok(Json(ToolsResponse { tools, error: None })),
        Err(e) => Ok(Json(ToolsResponse {
            tools: vec![],
            error: Some(e.to_string()),
        })),
    }
}

/// Tool call request
#[derive(Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// Call a tool on the MCP server
pub async fn call_tool(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CallToolRequest>,
) -> Result<Json<ToolCallResult>, (StatusCode, String)> {
    let result = state
        .proxy
        .call_tool(&request.name, request.arguments)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

/// Query parameters for resource read requests
#[derive(Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
}

/// List UI resources from the MCP server and any file-based widgets.
///
/// When `widgets_dir` is configured, discovered `.html` files are merged with
/// proxy-fetched resources. Proxy resources are filtered to those whose MIME type
/// contains "html" (case-insensitive).
pub async fn list_resources(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut all_resources: Vec<serde_json::Value> = Vec::new();

    // Add file-based widgets from widgets_dir (if configured)
    if let Some(ref widgets_dir) = state.config.widgets_dir {
        match std::fs::read_dir(widgets_dir) {
            Ok(entries) => {
                let mut widget_entries: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("html"))
                    .collect();
                widget_entries.sort_by_key(|e| e.file_name());

                for entry in widget_entries {
                    if let Some(stem) = entry
                        .path()
                        .file_stem()
                        .and_then(|s| s.to_str().map(String::from))
                    {
                        all_resources.push(json!({
                            "uri": format!("ui://app/{}", stem),
                            "name": stem,
                            "description": format!("Widget from {}", entry.path().display()),
                            "mimeType": "text/html"
                        }));
                    }
                }

                tracing::debug!(
                    "Discovered {} widget(s) from {}",
                    all_resources.len(),
                    widgets_dir.display()
                );
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to read widgets directory {}: {}",
                    widgets_dir.display(),
                    e
                );
            },
        }
    }

    // Also fetch proxy resources (from the MCP server)
    match state.proxy.list_resources().await {
        Ok(resources) => {
            let ui_resources = resources.into_iter().filter(|r| {
                r.mime_type
                    .as_deref()
                    .is_some_and(|m| m.to_lowercase().contains("html"))
            });
            for r in ui_resources {
                // Avoid duplicates: skip proxy resources whose URI matches a disk widget
                let dominated = all_resources
                    .iter()
                    .any(|existing| existing.get("uri").and_then(|v| v.as_str()) == Some(&r.uri));
                if !dominated {
                    all_resources.push(json!({
                        "uri": r.uri,
                        "name": r.name,
                        "description": r.description,
                        "mimeType": r.mime_type,
                        "_meta": r.meta
                    }));
                }
            }
        },
        Err(e) => {
            tracing::warn!("Proxy list_resources failed: {}", e);
            if all_resources.is_empty() {
                return json_response(json!({ "resources": [], "error": e.to_string() }));
            }
        },
    }

    json_response(json!({ "resources": all_resources }))
}

/// Read a resource by URI.
///
/// When `widgets_dir` is configured and the URI matches `ui://app/{name}`,
/// reads the widget HTML directly from disk and auto-injects the bridge
/// script tag. Every browser refresh reads fresh HTML from disk (hot-reload).
///
/// For all other URIs, falls through to the MCP proxy.
pub async fn read_resource(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ReadResourceParams>,
) -> Json<Value> {
    // Check if this is a file-based widget (ui://app/{name})
    if let Some(ref widgets_dir) = state.config.widgets_dir {
        if let Some(widget_name) = params.uri.strip_prefix("ui://app/") {
            let file_path = widgets_dir.join(format!("{}.html", widget_name));
            let html = match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    tracing::debug!(
                        "Reading widget file: {} ({} bytes)",
                        file_path.display(),
                        content.len()
                    );
                    // Auto-inject bridge script
                    inject_bridge_script(&content, "/assets/widget-runtime.mjs")
                },
                Err(err) => {
                    tracing::warn!("Failed to read widget {}: {}", file_path.display(), err);
                    widget_error_html(widget_name, &file_path, &err.to_string())
                },
            };

            return json_response(json!({
                "contents": [{
                    "uri": params.uri,
                    "text": html,
                    "mimeType": "text/html"
                }]
            }));
        }
    }

    // Fall through to proxy for non-widget or non-configured resources
    match state.proxy.read_resource(&params.uri).await {
        Ok(result) => json_response(json!({
            "contents": result.contents,
            "_meta": result.meta
        })),
        Err(e) => json_response(json!({ "contents": null, "error": e.to_string() })),
    }
}

/// Insert a bridge script tag into widget HTML.
///
/// Delegates to the shared `pmcp-widget-utils` crate (single source of truth).
fn inject_bridge_script(html: &str, bridge_url: &str) -> String {
    pmcp_widget_utils::inject_bridge_script(html, bridge_url)
}

/// Generate a styled HTML error page for a widget that failed to load from disk.
fn widget_error_html(name: &str, path: &std::path::Path, error: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Widget Error: {name}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            padding: 20px;
        }}
        .error-card {{
            background: #4a1515;
            border: 1px solid #ff6b6b;
            border-radius: 12px;
            padding: 24px 32px;
            max-width: 560px;
            width: 100%;
        }}
        h2 {{ color: #ff6b6b; margin: 0 0 12px 0; font-size: 1.2rem; }}
        .file-path {{
            font-family: monospace;
            font-size: 0.85rem;
            color: #ffcc00;
            background: rgba(0,0,0,0.3);
            padding: 6px 10px;
            border-radius: 6px;
            word-break: break-all;
            margin-bottom: 12px;
        }}
        .error-message {{ font-family: monospace; font-size: 0.85rem; color: #ff9999; }}
        .hint {{ margin-top: 16px; font-size: 0.85rem; color: #888; }}
    </style>
</head>
<body>
    <div class="error-card">
        <h2>Widget Load Error</h2>
        <div class="file-path">{path}</div>
        <div class="error-message">{error}</div>
        <div class="hint">Create or fix the widget file and refresh the browser to retry.</div>
    </div>
</body>
</html>"#,
        name = name,
        path = path.display(),
        error = error,
    )
}

/// Reconnect to the MCP server by resetting the session and
/// re-initializing via a tool list request.
pub async fn reconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.proxy.reset_session().await;
    match state.proxy.list_tools().await {
        Ok(tools) => json_response(json!({
            "success": true,
            "toolCount": tools.len()
        })),
        Err(e) => json_response(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Check whether the MCP session is currently connected.
pub async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let connected = state.proxy.is_connected().await;
    json_response(json!({ "connected": connected }))
}

/// Wrap a `serde_json::Value` in an Axum `Json` response.
fn json_response(value: Value) -> Json<Value> {
    Json(value)
}
