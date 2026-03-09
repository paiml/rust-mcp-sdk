//! MCP Apps project template.
//!
//! Generates a complete MCP Apps project with server code, a starter widget,
//! Cargo.toml, and README. The scaffolded project uses `WidgetDir` from the
//! `pmcp` crate for file-based widget discovery and hot-reload.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate all files for a new MCP Apps project.
///
/// Creates `Cargo.toml`, `src/main.rs`, `widgets/hello.html`,
/// `mock-data/hello.json`, and `README.md` in the given project directory.
/// The directory structure (including `src/` and `widgets/`) must already
/// exist; `mock-data/` is created automatically.
pub fn generate(project_dir: &Path, name: &str) -> Result<()> {
    // Cargo.toml
    let cargo_toml = generate_cargo_toml(name);
    fs::write(project_dir.join("Cargo.toml"), cargo_toml).context("Failed to write Cargo.toml")?;
    println!("  {} Generated Cargo.toml", "ok".green());

    // src/main.rs
    let main_rs = generate_main_rs(name);
    fs::write(project_dir.join("src/main.rs"), main_rs).context("Failed to write src/main.rs")?;
    println!("  {} Generated src/main.rs", "ok".green());

    // widgets/hello.html
    let hello_html = generate_hello_widget();
    fs::write(project_dir.join("widgets/hello.html"), hello_html)
        .context("Failed to write widgets/hello.html")?;
    println!("  {} Generated widgets/hello.html", "ok".green());

    // mock-data/hello.json
    let mock_data_dir = project_dir.join("mock-data");
    fs::create_dir_all(&mock_data_dir).context("Failed to create mock-data/ directory")?;
    let mock_hello = generate_mock_hello();
    fs::write(mock_data_dir.join("hello.json"), mock_hello)
        .context("Failed to write mock-data/hello.json")?;
    println!("  {} Generated mock-data/hello.json", "ok".green());

    // README.md
    let readme = generate_readme(name);
    fs::write(project_dir.join("README.md"), readme).context("Failed to write README.md")?;
    println!("  {} Generated README.md", "ok".green());

    Ok(())
}

/// Generate the project's `Cargo.toml` with minimal dependencies.
fn generate_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "1.10", features = ["mcp-apps"] }}
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
schemars = "0.8"
async-trait = "0.1"
tracing-subscriber = "0.3"
"#
    )
}

/// Generate the server's `main.rs` using the builder pattern with `WidgetDir`.
fn generate_main_rs(name: &str) -> String {
    // Convert kebab-case project name to a display name
    let display_name = name.replace('-', " ");

    format!(
        r#"//! {display_name} -- MCP Apps server with interactive widgets.
//!
//! Run with:
//! ```bash
//! cargo run
//! ```
//!
//! Then preview:
//! ```bash
//! cargo pmcp preview --url http://localhost:3000 --open
//! ```

use async_trait::async_trait;
use pmcp::server::mcp_apps::{{ChatGptAdapter, UIAdapter, WidgetDir}};
use pmcp::server::streamable_http_server::{{StreamableHttpServer, StreamableHttpServerConfig}};
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{{ExtendedUIMimeType, WidgetMeta}};
use pmcp::types::protocol::Content;
use pmcp::types::{{ListResourcesResult, ReadResourceResult, ResourceInfo}};
use pmcp::{{RequestHandlerExtra, ResourceHandler, Result, TypedSyncTool}};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::net::{{Ipv4Addr, SocketAddr}};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

// === WidgetCSP Configuration ===
// Uncomment to restrict widget content security:
//
// use pmcp::types::mcp_apps::WidgetCSP;
//
// let csp = WidgetCSP::new()
//     .connect("https://api.example.com")    // Allow fetch/XHR to this domain
//     .resources("https://cdn.example.com")  // Allow images, scripts, fonts
//     .redirect("https://checkout.example.com"); // Allow external redirects
//
// Then pass to WidgetMeta:
//   WidgetMeta::new().csp(csp)

// =============================================================================
// Tool Input Types
// =============================================================================

/// Input for the hello tool.
#[derive(Deserialize, JsonSchema)]
struct HelloInput {{
    /// Name to greet
    name: String,
}}

// =============================================================================
// Tool Handlers
// =============================================================================

/// Greet someone by name.
fn hello_handler(input: HelloInput, _extra: RequestHandlerExtra) -> Result<serde_json::Value> {{
    Ok(json!({{
        "greeting": format!("Hello, {{}}!", input.name),
        "name": input.name
    }}))
}}

// =============================================================================
// Resource Handler
// =============================================================================

/// Widget resource handler that serves HTML files from the `widgets/` directory.
///
/// Uses `WidgetDir` for file-based widget discovery and hot-reload: widget HTML
/// is read from disk on every request, so a browser refresh shows the latest
/// content without server restart.
struct AppResources {{
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}}

impl AppResources {{
    fn new(widgets_path: PathBuf) -> Self {{
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("{display_name} widget");

        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);

        Self {{
            chatgpt_adapter,
            widget_dir,
        }}
    }}
}}

#[async_trait]
impl ResourceHandler for AppResources {{
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {{
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {{
            let html = self.widget_dir.read_widget(widget_name);
            let mut transformed = self.chatgpt_adapter.transform(uri, widget_name, &html);
            let meta = transformed.take_meta();

            Ok(ReadResourceResult::new(vec![Content::Resource {{
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                    meta,
                }}]))
        }} else {{
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {{}}", uri),
            ))
        }}
    }}

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {{
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {{
                uri: entry.uri,
                name: entry.filename.clone(),
                description: Some(format!("Interactive {{}} widget", entry.filename)),
                mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                meta: None,
            }})
            .collect();

        Ok(ListResourcesResult::new(resources))
    }}
}}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {{
    // Initialize logging for development
    tracing_subscriber::fmt::init();

    // Resolve widgets directory relative to the project root
    let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

    // Build the hello tool with UI resource link
    let hello_tool = TypedSyncTool::<HelloInput, _>::new("hello", hello_handler)
        .with_description("Greet someone by name. Returns a friendly greeting.")
        .with_ui("ui://app/hello.html");

    let server = ServerBuilder::new()
        .name("{name}")
        .version("0.1.0")
        .tool("hello", hello_tool)
        .resources(AppResources::new(widgets_path))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let server = Arc::new(Mutex::new(server));

    // Configure HTTP server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000u16);
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    let config = StreamableHttpServerConfig {{
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    }};

    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, server_handle) = http_server
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("{name} MCP Server running at http://{{}}", bound_addr);
    println!();
    println!("Available tools:");
    println!("  - hello: Greet someone by name");
    println!();
    println!(
        "Preview: cargo pmcp preview --url http://{{}} --open",
        bound_addr
    );
    println!();
    println!("Press Ctrl+C to stop");

    server_handle.await.map_err(|e| {{
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    }})?;

    Ok(())
}}
"#
    )
}

/// Generate mock data for the hello tool.
///
/// Returns a JSON string matching the response shape of the scaffolded
/// `hello_handler` tool, used by `cargo pmcp app landing` to demonstrate
/// the widget without a running server.
fn generate_mock_hello() -> String {
    r#"{
  "greeting": "Hello, World!",
  "name": "World"
}
"#
    .to_string()
}

/// Generate the starter `hello.html` widget demonstrating the bridge pattern.
fn generate_hello_widget() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hello Widget</title>
    <!-- This widget uses the stateless pattern: all state lives in the browser,
         tool calls are pure functions. The bridge script tag is auto-injected
         by the server (via WidgetDir) -- do NOT add it manually. -->
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f8f9fa;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 20px;
        }
        .card {
            background: white;
            border-radius: 12px;
            box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
            padding: 32px;
            max-width: 400px;
            width: 100%;
        }
        h1 {
            font-size: 1.4rem;
            color: #1a1a2e;
            margin-bottom: 16px;
        }
        .input-group {
            display: flex;
            gap: 8px;
            margin-bottom: 16px;
        }
        input[type="text"] {
            flex: 1;
            padding: 10px 14px;
            border: 1px solid #ddd;
            border-radius: 8px;
            font-size: 1rem;
            outline: none;
            transition: border-color 0.2s;
        }
        input[type="text"]:focus {
            border-color: #4a90d9;
        }
        button {
            padding: 10px 20px;
            background: #4a90d9;
            color: white;
            border: none;
            border-radius: 8px;
            font-size: 1rem;
            cursor: pointer;
            transition: background 0.2s;
        }
        button:hover {
            background: #357abd;
        }
        button:disabled {
            background: #ccc;
            cursor: not-allowed;
        }
        .result {
            padding: 16px;
            background: #f0f7ff;
            border-radius: 8px;
            color: #1a1a2e;
            font-size: 1.1rem;
            display: none;
        }
        .result.visible {
            display: block;
        }
        .error {
            background: #fff0f0;
            color: #c00;
        }
    </style>
</head>
<body>
    <div class="card">
        <h1>Say Hello</h1>
        <div class="input-group">
            <input type="text" id="name-input" placeholder="Enter a name..." />
            <button id="greet-btn">Say Hello</button>
        </div>
        <div id="result" class="result"></div>
    </div>

    <script>
        const nameInput = document.getElementById('name-input');
        const greetBtn = document.getElementById('greet-btn');
        const resultDiv = document.getElementById('result');

        greetBtn.addEventListener('click', async () => {
            const name = nameInput.value.trim();
            if (!name) return;

            greetBtn.disabled = true;
            resultDiv.className = 'result';
            resultDiv.textContent = 'Calling tool...';
            resultDiv.classList.add('visible');

            try {
                // Call the "hello" tool via the MCP bridge.
                // The bridge is auto-injected by the server -- no script tag needed.
                const response = await window.mcpBridge.callTool('hello', { name });
                resultDiv.textContent = response.greeting;
                resultDiv.classList.add('visible');
                resultDiv.classList.remove('error');
            } catch (err) {
                resultDiv.textContent = 'Error: ' + (err.message || err);
                resultDiv.classList.add('visible', 'error');
            } finally {
                greetBtn.disabled = false;
            }
        });

        // Allow pressing Enter to trigger the greeting
        nameInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') greetBtn.click();
        });
    </script>
</body>
</html>
"#
    .to_string()
}

/// Generate the project README with bridge API documentation.
fn generate_readme(name: &str) -> String {
    format!(
        r#"# {name}

An MCP Apps project with interactive widgets powered by [pmcp](https://crates.io/crates/pmcp).

## Getting Started

```bash
# Build the project
cargo build

# Start the server
cargo run

# In another terminal, open the preview
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser with the hello widget ready to use.

## Project Structure

```
{name}/
  src/
    main.rs          # MCP server with tool handlers
  widgets/
    hello.html       # Starter widget (add more .html files here)
  Cargo.toml
  README.md
```

**Widget auto-discovery:** Every `.html` file in `widgets/` is automatically
registered as an MCP resource. To add a new widget, drop an `.html` file in
`widgets/` and refresh the browser -- no server restart required.

**URI mapping:** `widgets/hello.html` becomes the resource `ui://app/hello`.

## Bridge API

Widgets communicate with the server through the MCP bridge. The bridge script
is auto-injected by the server -- you never add a `<script>` tag for it.

### `callTool(name, args)`

Call a server-side tool and get the result.

```javascript
const result = await window.mcpBridge.callTool('hello', {{ name: 'World' }});
console.log(result.greeting); // "Hello, World!"
```

### `getState()` / `setState(state)`

Read and write widget-local state. State is held in the browser and persists
across tool calls within the same session.

```javascript
// Save state
window.mcpBridge.setState({{ count: 42 }});

// Restore state later
const state = window.mcpBridge.getState();
console.log(state.count); // 42
```

### Lifecycle Events

The bridge dispatches events on `window` for connection status:

```javascript
window.addEventListener('mcp:connected', () => {{
    console.log('Bridge connected to MCP server');
}});

window.addEventListener('mcp:disconnected', () => {{
    console.log('Bridge disconnected');
}});
```

## Stateless Widget Pattern

Widgets in MCP Apps follow a **stateless server** pattern:

1. **All state lives in the browser.** The widget keeps its own state in
   JavaScript variables or `setState()`.
2. **Tool calls include full state.** When calling a tool that needs context,
   pass the relevant state as part of the arguments.
3. **Server processes without sessions.** Tool handlers are pure functions --
   they receive input, return output, and store nothing.

This design means widgets work in any MCP host (ChatGPT, Claude, etc.)
without server-side session management.

## CSP Configuration

Content Security Policy controls what external resources your widget can access.
Use `WidgetCSP` in `main.rs` to configure policies:

```rust
use pmcp::types::mcp_apps::WidgetCSP;

let csp = WidgetCSP::new()
    .connect("https://api.example.com")    // Allow fetch/XHR
    .resources("https://cdn.example.com")  // Allow images, scripts, fonts
    .redirect("https://checkout.example.com"); // Allow external redirects
```

Then attach it to your widget metadata:

```rust
use pmcp::types::mcp_apps::{{WidgetMeta, WidgetCSP}};

let meta = WidgetMeta::new()
    .csp(WidgetCSP::new().connect("https://api.example.com"));
```

**Default policy:** When no CSP is configured, the host applies its default
policy. In ChatGPT, this restricts widgets to same-origin requests only.
Configure CSP when your widget needs to call external APIs or load external
resources.

## Adding Tools

Define new tools in `src/main.rs`:

```rust
#[derive(Deserialize, JsonSchema)]
struct MyInput {{
    value: String,
}}

fn my_handler(input: MyInput, _extra: RequestHandlerExtra) -> Result<serde_json::Value> {{
    Ok(json!({{ "result": input.value.to_uppercase() }}))
}}
```

Then register with the server builder:

```rust
.tool_typed_sync_with_description(
    "my_tool",
    "Description of what this tool does",
    my_handler,
)
```

Widgets call it with `window.mcpBridge.callTool('my_tool', {{ value: 'hello' }})`.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_cargo_toml_contains_name() {
        let toml = generate_cargo_toml("my-app");
        assert!(toml.contains(r#"name = "my-app""#));
        assert!(toml.contains(r#"pmcp = { version = "1.10""#));
        assert!(toml.contains(r#"features = ["mcp-apps"]"#));
    }

    #[test]
    fn test_generate_main_rs_contains_server_setup() {
        let main = generate_main_rs("my-app");
        assert!(main.contains("WidgetDir"));
        assert!(main.contains("hello_handler"));
        assert!(main.contains("StreamableHttpServer"));
        assert!(main.contains("WidgetCSP"));
        // Must use HtmlMcpApp (not HtmlSkybridge)
        assert!(main.contains("HtmlMcpApp"));
        assert!(!main.contains("HtmlSkybridge"));
        // Must use .with_ui() on tool registration
        assert!(main.contains(".with_ui("));
        assert!(main.contains("TypedSyncTool"));
        // Must forward _meta from adapter via take_meta()
        assert!(main.contains("transformed.take_meta()"));
    }

    #[test]
    fn test_generate_hello_widget_uses_bridge() {
        let html = generate_hello_widget();
        assert!(html.contains("window.mcpBridge.callTool"));
        assert!(html.contains("stateless pattern"));
        // Should NOT contain a bridge script tag
        assert!(!html.contains(r#"src="widget-runtime"#));
    }

    #[test]
    fn test_generate_readme_documents_bridge_api() {
        let readme = generate_readme("my-app");
        assert!(readme.contains("callTool"));
        assert!(readme.contains("getState"));
        assert!(readme.contains("setState"));
        assert!(readme.contains("Stateless Widget Pattern"));
        assert!(readme.contains("CSP Configuration"));
        assert!(readme.contains("WidgetCSP"));
    }

    #[test]
    fn test_generate_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("test-app");
        std::fs::create_dir_all(project_dir.join("src")).unwrap();
        std::fs::create_dir_all(project_dir.join("widgets")).unwrap();

        generate(&project_dir, "test-app").unwrap();

        assert!(project_dir.join("Cargo.toml").exists());
        assert!(project_dir.join("src/main.rs").exists());
        assert!(project_dir.join("widgets/hello.html").exists());
        assert!(project_dir.join("mock-data/hello.json").exists());
        assert!(project_dir.join("README.md").exists());
    }

    #[test]
    fn test_generate_creates_mock_data() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("mock-test");
        std::fs::create_dir_all(project_dir.join("src")).unwrap();
        std::fs::create_dir_all(project_dir.join("widgets")).unwrap();

        generate(&project_dir, "mock-test").unwrap();

        let mock_path = project_dir.join("mock-data/hello.json");
        assert!(mock_path.exists(), "mock-data/hello.json should exist");

        let content = std::fs::read_to_string(&mock_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["greeting"], "Hello, World!");
        assert_eq!(parsed["name"], "World");
    }

    #[test]
    fn test_generate_mock_hello_is_valid_json() {
        let content = generate_mock_hello();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("greeting").is_some());
        assert!(parsed.get("name").is_some());
    }
}
