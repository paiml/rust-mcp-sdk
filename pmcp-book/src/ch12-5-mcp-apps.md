# Chapter 12.5: MCP Apps Extension -- Interactive UIs

The MCP Apps Extension lets your server serve rich, interactive UIs -- charts, maps, games, dashboards -- as **widgets** alongside your tools. Widgets are plain HTML files that communicate with your Rust backend through a bridge API, and adapters transform them for different hosts (ChatGPT, MCP Apps, MCP-UI) without any changes to your widget code.

The modern developer experience is straightforward: write HTML files in a `widgets/` directory, point `WidgetDir` at that directory, and the server reads them from disk on every request. A browser refresh shows your latest changes instantly -- no server restart required.

This chapter covers:

1. **Widget authoring** with `WidgetDir` -- the file-based convention, API, and hot-reload workflow
2. **Bridge communication** -- the `window.mcpBridge` API that widgets use to call tools, read resources, and manage state
3. **Developer workflow** -- scaffolding, live preview, and building for distribution with `cargo pmcp`
4. **Adapter pattern** -- how a single widget works across ChatGPT, MCP Apps, and MCP-UI hosts (Chapter 12.5 continued)
5. **Example walkthroughs** -- the chess, map, and dataviz examples step by step (Chapter 12.5 continued)

**Feature flag requirement:** Enable the `mcp-apps` feature in your `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
```

---

## Quick Start: Your First Widget (5 Minutes)

Let's go from zero to a working interactive widget in three steps.

### Step 1: Scaffold the Project

```bash
cargo pmcp app new my-widget-app
cd my-widget-app
```

This creates a ready-to-run project:

```
my-widget-app/
  src/
    main.rs          # MCP server with tool handlers
  widgets/
    hello.html       # Starter widget (add more .html files here)
  mock-data/
    hello.json       # Mock data for landing page generation
  Cargo.toml
  README.md
```

### Step 2: Write a Widget

The scaffold includes `widgets/hello.html`, which demonstrates the bridge pattern. Here is a minimal widget that calls a tool and displays the result:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Hello Widget</title>
    <!-- The bridge script tag is auto-injected by the server.
         Do NOT add it manually. -->
    <style>
        body {
            font-family: system-ui, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: #f8f9fa;
        }
        .card {
            background: white;
            border-radius: 12px;
            box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
            padding: 32px;
            max-width: 400px;
            width: 100%;
        }
    </style>
</head>
<body>
    <div class="card">
        <h1>Say Hello</h1>
        <input type="text" id="name" placeholder="Enter a name..." />
        <button id="greet">Say Hello</button>
        <div id="result"></div>
    </div>

    <script>
        document.getElementById('greet').addEventListener('click', async () => {
            const name = document.getElementById('name').value.trim();
            if (!name) return;

            try {
                // Call the "hello" tool via the MCP bridge
                const response = await window.mcpBridge.callTool('hello', { name });
                document.getElementById('result').textContent = response.greeting;
            } catch (err) {
                document.getElementById('result').textContent = 'Error: ' + err.message;
            }
        });
    </script>
</body>
</html>
```

### Step 3: Run and Preview

```bash
# Build and start the server
cargo run &

# Open the browser-based preview
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser. Type a name, click the button, and the widget calls the `hello` tool on your MCP server, displaying the greeting it returns.

### What the Server Looks Like

The scaffolded `src/main.rs` uses `ServerBuilder` with `WidgetDir` and `ChatGptAdapter`:

```rust
use async_trait::async_trait;
use pmcp::server::mcp_apps::{ChatGptAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, WidgetMeta};
use pmcp::types::protocol::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

// Tool input type
#[derive(Deserialize, JsonSchema)]
struct HelloInput {
    name: String,
}

// Tool handler -- a pure function
fn hello_handler(input: HelloInput, _extra: RequestHandlerExtra) -> Result<serde_json::Value> {
    Ok(json!({
        "greeting": format!("Hello, {}!", input.name),
        "name": input.name
    }))
}

// Resource handler that serves widgets from the widgets/ directory
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}

impl AppResources {
    fn new(widgets_path: PathBuf) -> Self {
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("my widget app widget");
        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);
        Self { chatgpt_adapter, widget_dir }
    }
}

#[async_trait]
impl ResourceHandler for AppResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let transformed = self.chatgpt_adapter.transform(uri, widget_name, &html);

            Ok(ReadResourceResult::new(vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(ExtendedUIMimeType::HtmlSkybridge.to_string()),
                }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(ExtendedUIMimeType::HtmlSkybridge.to_string()),
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}
```

The server registers the tool and the resource handler with `ServerBuilder`, then runs on an HTTP transport:

```rust
let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

let server = ServerBuilder::new()
    .name("my-widget-app")
    .version("0.1.0")
    .tool_typed_sync_with_description("hello", "Greet someone by name", hello_handler)
    .resources(AppResources::new(widgets_path))
    .build()?;
```

This pattern -- `WidgetDir` for widget discovery, `ChatGptAdapter` for bridge injection, `ResourceHandler` for serving -- is identical across all three shipped examples (chess, map, and dataviz).

---

## Widget Authoring with WidgetDir

### Convention

Widgets are `.html` files in a `widgets/` directory. The filename maps directly to an MCP resource URI:

| File                   | MCP Resource URI    |
|------------------------|---------------------|
| `widgets/board.html`   | `ui://app/board`    |
| `widgets/map.html`     | `ui://app/map`      |
| `widgets/hello.html`   | `ui://app/hello`    |

Each widget is a single, self-contained HTML file. The server auto-injects the bridge script tag via the adapter -- widget authors never add bridge boilerplate manually.

### WidgetDir API

`WidgetDir` lives in `pmcp::server::mcp_apps` and provides three operations:

**Construction:**

```rust
use pmcp::server::mcp_apps::WidgetDir;

// Point at the widgets directory
let widget_dir = WidgetDir::new("widgets");

// The path does not need to exist at construction time.
// Errors are returned when discover() or read_widget() are called.
```

**Discovery:**

```rust
// Scan for .html files, returns Vec<WidgetEntry> sorted by filename
let entries = widget_dir.discover()?;

for entry in &entries {
    println!("{} -> {}", entry.filename, entry.uri);
    // "board" -> "ui://app/board"
    // "map"   -> "ui://app/map"
}
```

The `WidgetEntry` struct has three fields:

| Field      | Type       | Description                                    |
|------------|------------|------------------------------------------------|
| `filename` | `String`   | Stem of the HTML file (e.g., `"board"`)        |
| `uri`      | `String`   | MCP resource URI (e.g., `"ui://app/board"`)    |
| `path`     | `PathBuf`  | Absolute path to the `.html` file on disk      |

**Reading:**

```rust
// Read widget HTML from disk -- fresh on every call
let html = widget_dir.read_widget("board");
```

`read_widget` reads from disk on every call. There is no cache. This is intentional -- it enables the hot-reload development workflow described below.

If the file does not exist or cannot be read, `read_widget` returns a styled HTML error page showing the widget name, the file path that was attempted, and the error message. The error page includes a hint: "Create or fix the widget file and refresh the browser to retry."

**Bridge injection:**

```rust
// Insert a <script> tag into widget HTML
let html_with_bridge = WidgetDir::inject_bridge_script(
    &html,
    "/assets/widget-runtime.mjs",
);
```

The injection strategy inserts the script tag just before `</head>` if present, at the start of `<body>` otherwise, or at the very beginning of the document if neither tag is found. This is how the bridge script reaches the widget without the author adding it manually.

### Hot-Reload Development

Because `WidgetDir` reads from disk on every request, the development workflow feels like frontend development:

1. Start your server: `cargo run`
2. Open the preview: `cargo pmcp preview --url http://localhost:3000 --open`
3. Edit your HTML file in `widgets/`
4. Refresh the browser -- your changes appear instantly

No server restart is needed. The server re-reads the file from disk each time a client requests the widget resource. This is safe because widgets are small HTML files and disk I/O is negligible compared to network latency.

### The ResourceHandler Pattern

Every MCP Apps server needs a `ResourceHandler` implementation that connects `WidgetDir` to the MCP resource protocol. The pattern is the same across all shipped examples:

```rust
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}

#[async_trait]
impl ResourceHandler for AppResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra)
        -> Result<ReadResourceResult>
    {
        // 1. Extract widget name from URI
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            // 2. Read HTML from disk (hot-reload)
            let html = self.widget_dir.read_widget(widget_name);

            // 3. Transform for target host (injects bridge script)
            let transformed = self.chatgpt_adapter
                .transform(uri, widget_name, &html);

            Ok(ReadResourceResult::new(vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(
                        ExtendedUIMimeType::HtmlSkybridge.to_string()
                    ),
                }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Discover all widgets and map to ResourceInfo
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(
                    ExtendedUIMimeType::HtmlSkybridge.to_string()
                ),
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}
```

The three steps in `read()` are always the same:

1. **Extract widget name** from the `ui://app/{name}` URI
2. **Read from disk** via `widget_dir.read_widget(name)` -- no cache, hot-reload
3. **Transform** via `adapter.transform()` -- injects the bridge script for the target host

The `list()` method calls `widget_dir.discover()` and maps each `WidgetEntry` to a `ResourceInfo` for the MCP protocol.

This pattern is identical in the chess example (`ChessResources`), the map example, and the dataviz example. Once you understand it, you can build any widget-based MCP server.

---

## Bridge Communication

Widgets communicate with the MCP server through the `window.mcpBridge` API. The bridge script is auto-injected by the adapter -- widget authors never write `postMessage` code or manage JSON-RPC framing manually.

### window.mcpBridge API

The bridge exposes these methods:

**Core Operations:**

| Method                        | Returns     | Description                              |
|-------------------------------|-------------|------------------------------------------|
| `mcpBridge.callTool(name, args)` | `Promise`  | Call an MCP tool, get the result         |
| `mcpBridge.readResource(uri)`    | `Promise`  | Read an MCP resource                     |
| `mcpBridge.getPrompt(name, args)`| `Promise`  | Get a prompt                             |
| `mcpBridge.notify(method, params)` | `void`   | Send a notification (fire-and-forget)    |

**State Management (ChatGPT host only):**

| Method                        | Returns     | Description                              |
|-------------------------------|-------------|------------------------------------------|
| `mcpBridge.setState(state)`   | `void`      | Update widget state (persists in session)|
| `mcpBridge.getState()`        | `Object`    | Read current widget state                |

**Communication (ChatGPT host only):**

| Method                           | Returns  | Description                              |
|----------------------------------|----------|------------------------------------------|
| `mcpBridge.sendMessage(message)` | `void`   | Send a follow-up message to the conversation |
| `mcpBridge.openExternal(url)`    | `void`   | Open an external URL                     |

**Display Modes (ChatGPT host only):**

| Method                                | Returns   | Description                             |
|---------------------------------------|-----------|-----------------------------------------|
| `mcpBridge.requestDisplayMode(mode)`  | `Promise` | Request inline, pip, or fullscreen      |
| `mcpBridge.requestClose()`            | `void`    | Close the widget                        |
| `mcpBridge.notifyIntrinsicHeight(h)`  | `void`    | Report the widget's content height      |
| `mcpBridge.setOpenInAppUrl(href)`     | `void`    | Set the "Open in App" button URL        |

**Environment Context (read-only properties, ChatGPT host only):**

| Property                | Type     | Description                              |
|-------------------------|----------|------------------------------------------|
| `mcpBridge.theme`       | `string` | Current theme (`'light'` or `'dark'`)    |
| `mcpBridge.locale`      | `string` | Current locale (e.g., `'en-US'`)         |
| `mcpBridge.displayMode` | `string` | Current display mode                     |
| `mcpBridge.toolInput`   | `object` | Arguments supplied when the tool was invoked |
| `mcpBridge.toolOutput`  | `object` | The structuredContent returned by the tool   |

The core operations (`callTool`, `readResource`, `getPrompt`, `notify`) work across all hosts. The state management, communication, and display mode methods are available when running inside ChatGPT.

### Communication Flow

Here is how a tool call flows through the system:

```
Widget (iframe)                    Host                      MCP Server
     │                              │                            │
     │  mcpBridge.callTool(         │                            │
     │    'hello', { name: 'World' }│                            │
     │  )                           │                            │
     │                              │                            │
     │  ──── bridge script ────►    │                            │
     │       (postMessage or        │                            │
     │        window.openai)        │                            │
     │                              │                            │
     │                              │  ── tools/call ──────►     │
     │                              │     { name: 'hello',       │
     │                              │       arguments: {...} }   │
     │                              │                            │
     │                              │  ◄── result ──────────     │
     │                              │     { greeting: '...' }    │
     │                              │                            │
     │  ◄── response ─────────     │                            │
     │      Promise resolves        │                            │
     │      with { greeting: '...' }│                            │
     ▼                              ▼                            ▼
```

The bridge script handles all the protocol plumbing:

- **ChatGPT host:** Uses `window.openai.callTool()` under the hood
- **MCP Apps host:** Uses `postMessage` with JSON-RPC 2.0 framing
- **MCP-UI host:** Uses `postMessage` with JSON-RPC 2.0 framing

Widget authors write the same `mcpBridge.callTool()` call regardless of which host runs their widget. The adapter selects the correct bridge implementation at serve time.

### Error Handling in Widgets

Always wrap bridge calls in try/catch:

```javascript
async function greet(name) {
    try {
        const result = await window.mcpBridge.callTool('hello', { name });
        document.getElementById('result').textContent = result.greeting;
    } catch (err) {
        document.getElementById('result').textContent =
            'Error: ' + (err.message || String(err));
    }
}
```

If your widget needs to wait for the bridge to initialize before making calls, listen for the `mcpBridgeReady` event:

```javascript
// Wait for bridge initialization
window.addEventListener('mcpBridgeReady', () => {
    // Bridge is ready -- safe to call mcpBridge methods
    loadInitialData();
});

// If bridge is already ready (script loaded synchronously)
if (window.mcpBridge) {
    loadInitialData();
}
```

The `mcpBridgeReady` event is dispatched by the injected bridge script after it sets up `window.mcpBridge`. In most cases the bridge is available immediately, but the event pattern is useful for widgets that load asynchronously.

---

## Developer Workflow

The full development cycle for an MCP Apps project follows five stages:

```
  scaffold          author           run            preview          build
  ────────►  ──────────────►  ──────────►  ──────────────►  ──────────►
  cargo pmcp   Edit HTML in    cargo run    cargo pmcp       cargo pmcp
  app new      widgets/                     preview          app build
               (hot-reload)                 --url ... --open --url ...
```

1. **Scaffold** -- generate a project with `cargo pmcp app new`
2. **Author widgets** -- write HTML files in `widgets/`, iterate with browser refresh
3. **Run the server** -- `cargo run` starts the MCP server
4. **Preview** -- `cargo pmcp preview` opens a browser-based testing environment
5. **Build** -- `cargo pmcp app build` produces `manifest.json` and `landing.html` for distribution

Each stage is covered in detail below.

### Scaffolding with `cargo pmcp app new`

```bash
cargo pmcp app new my-widget-app
```

This creates a complete project directory:

```
my-widget-app/
  src/
    main.rs          # MCP server with tool handlers and ResourceHandler
  widgets/
    hello.html       # Starter widget demonstrating bridge pattern
  mock-data/
    hello.json       # Mock tool response for landing page generation
  Cargo.toml         # pmcp dependency with mcp-apps feature enabled
  README.md          # Getting started guide with bridge API docs
```

**Flags:**

| Flag            | Description                              | Default            |
|-----------------|------------------------------------------|--------------------|
| `<name>`        | Project name (required, positional)      | --                 |
| `--path <DIR>`  | Parent directory to create project in    | Current directory  |

If the target directory already exists, the command errors with a message matching `cargo new` semantics:

```
Error: directory 'my-widget-app' already exists.
Use a different name or remove the existing directory.
```

The generated `Cargo.toml` includes the required dependency:

```toml
[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
async-trait = "0.1"
tracing-subscriber = "0.3"
```

After scaffolding, the next steps are printed to the terminal:

```
  Next steps:
    cd my-widget-app
    cargo build
    cargo run &
    cargo pmcp preview --url http://localhost:3000 --open
```

### Live Preview with `cargo pmcp preview`

The preview command connects to a running MCP server and renders widgets in a browser-based testing environment that simulates the ChatGPT Apps runtime.

```bash
cargo pmcp preview --url http://localhost:3000 --open
```

**Flags:**

| Flag                    | Description                                       | Default    |
|-------------------------|---------------------------------------------------|------------|
| `--url <URL>`           | URL of the running MCP server (required)          | --         |
| `--port <PORT>`         | Port for the preview server                       | `8765`     |
| `--open`                | Open browser automatically                        | `false`    |
| `--tool <NAME>`         | Auto-select this tool on start                    | --         |
| `--theme <light\|dark>` | Initial theme for the preview environment         | `light`    |
| `--locale <LOCALE>`     | Initial locale (e.g., `en-US`, `ja-JP`)           | `en-US`    |
| `--widgets-dir <PATH>`  | Path to widgets directory for hot-reload          | --         |

The preview server starts on `http://localhost:{port}` and connects to your MCP server via the MCP protocol. When `--open` is set, the browser opens automatically after a short delay.

**Typical development loop:**

```bash
# Terminal 1: Start the MCP server
cargo run

# Terminal 2: Start the preview with hot-reload
cargo pmcp preview --url http://localhost:3000 --open --widgets-dir ./widgets
```

With `--widgets-dir` set, the preview reads widget HTML directly from the specified directory on each request, enabling the hot-reload workflow. Edit a widget file, refresh the browser, and see your changes immediately.

**Environment simulation:** The `--theme` and `--locale` flags let you test how your widget behaves in different environments without switching hosts. The `--tool` flag auto-selects a specific tool when the preview loads, which is useful when your server has many tools and you want to jump straight to the one you are developing.

### Building with `cargo pmcp app build`

When your widgets are ready for distribution, the build command produces deployment artifacts:

```bash
cargo pmcp app build --url https://my-server.example.com
```

This generates two files in the output directory:

```
dist/
  manifest.json    # ChatGPT-compatible app directory listing
  landing.html     # Standalone demo page with embedded widget
```

**Flags:**

| Flag              | Description                                     | Default  |
|-------------------|-------------------------------------------------|----------|
| `--url <URL>`     | Server URL for manifest (required)              | --       |
| `--logo <URL>`    | Logo URL for the manifest                       | --       |
| `--widget <NAME>` | Widget to showcase in landing page              | First alphabetically |
| `--output <DIR>`  | Output directory for generated files            | `dist`   |

**What each artifact is for:**

- **`manifest.json`** -- A ChatGPT-compatible app directory listing following the `ai-plugin.json` schema (v1). Contains your server URL, package name, description, logo, and auto-discovered widget-to-tool mappings. Upload this to a ChatGPT Apps directory to make your server discoverable.

- **`landing.html`** -- A standalone demo page that embeds your widget in an iframe with a mock bridge. The mock bridge returns hardcoded responses from `mock-data/*.json` files, so the page works without a running server. Use this as a product page or share it for quick demos.

**Individual artifact commands:**

If you only need one artifact, use the subcommands directly:

```bash
# Generate only manifest.json
cargo pmcp app manifest --url https://my-server.example.com
cargo pmcp app manifest --url https://my-server.example.com --logo https://example.com/logo.png

# Generate only landing.html
cargo pmcp app landing
cargo pmcp app landing --widget board --output build
```

The `manifest` subcommand requires `--url` (the server URL is embedded in the manifest). The `landing` subcommand does not require `--url` because it uses mock data.

Both subcommands accept `--output <DIR>` (default: `dist`).

### Project Detection

The `cargo pmcp app` commands auto-detect your project by reading `Cargo.toml` in the current directory. The detection verifies:

1. A `pmcp` dependency exists with either `mcp-apps` or `full` feature enabled
2. A `widgets/` directory exists with at least one `.html` file

If detection fails, you get a descriptive error:

```
Error: Not an MCP Apps project.
The `pmcp` dependency does not enable `mcp-apps` or `full` features.
Run `cargo pmcp app new` first.
```

**Optional metadata:** Add a `[package.metadata.pmcp]` section to your `Cargo.toml` for additional customization:

```toml
[package.metadata.pmcp]
logo = "https://example.com/my-logo.png"
```

The `logo` field is used by `cargo pmcp app manifest` as the default logo URL. You can override it at build time with the `--logo` flag.

---

## Multi-Platform Adapter Pattern

### The Problem

MCP widgets need to communicate with the host application that embeds them, but different hosts use different communication mechanisms:

- **ChatGPT Apps:** The host injects a `window.openai` JavaScript API. Widgets use MIME type `text/html+skybridge`. The host manages state, display modes, and file operations natively.
- **MCP Apps (SEP-1865):** Widgets run in an iframe and communicate via `postMessage` using JSON-RPC 2.0 framing. MIME type is `text/html+mcp`.
- **MCP-UI (community):** Also uses `postMessage` with JSON-RPC 2.0, but supports additional output formats beyond HTML -- URL references for CDN-hosted widgets and Remote DOM for non-iframe rendering. MIME type is `text/html` for HTML format.

If widget authors had to write platform-specific code for each host, they would need to maintain three separate implementations of the same UI. The adapter pattern solves this.

### Architecture Overview

The adapter architecture follows a simple principle: widget authors write ONE HTML file using `window.mcpBridge` as their API. At serve time, an adapter injects the correct bridge script that translates `mcpBridge` calls into the platform's native communication mechanism.

```text
+----------------------------------------------------------------+
|                       UIResource (Core)                        |
|                                                                |
|   +--------------+    +--------------+    +----------------+   |
|   | ChatGptAdapter|    | McpAppsAdapter|    |  McpUiAdapter  |   |
|   +--------------+    +--------------+    +----------------+   |
|           |                   |                    |           |
|           v                   v                    v           |
|   text/html+skybridge   text/html+mcp        text/html        |
|   window.openai         postMessage          postMessage       |
+----------------------------------------------------------------+
```

The `UIAdapter` trait defines the interface. Three concrete adapters implement it. Each adapter's `transform()` method reads the widget HTML, calls `inject_bridge()` to insert the platform-specific bridge script, and returns a `TransformedResource` with the correct MIME type and metadata.

### The UIAdapter Trait

The `UIAdapter` trait is defined in `pmcp::server::mcp_apps` and has five methods:

```rust
pub trait UIAdapter: Send + Sync {
    /// Which host platform this adapter targets.
    fn host_type(&self) -> HostType;

    /// The MIME type this adapter produces.
    fn mime_type(&self) -> ExtendedUIMimeType;

    /// Transform HTML content for this host platform.
    /// Returns the transformed resource with platform-specific metadata.
    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource;

    /// Inject platform-specific communication bridge into HTML content.
    fn inject_bridge(&self, html: &str) -> String;

    /// Get CSP headers required by this platform.
    fn required_csp(&self) -> Option<WidgetCSP>;
}
```

The `TransformedResource` returned by `transform()` carries everything needed to serve the widget:

| Field      | Type                 | Description                                     |
|------------|----------------------|-------------------------------------------------|
| `uri`      | `String`             | Original resource URI                           |
| `name`     | `String`             | Display name                                    |
| `mime_type`| `ExtendedUIMimeType` | Platform-specific MIME type                     |
| `content`  | `String`             | Transformed HTML with bridge script injected    |
| `metadata` | `HashMap<String, Value>` | Platform-specific metadata (e.g., WidgetMeta) |

The `HostType` enum identifies known MCP hosts:

| Variant   | Description                        | Preferred MIME type        |
|-----------|------------------------------------|----------------------------|
| `ChatGpt` | OpenAI ChatGPT                     | `text/html+skybridge`      |
| `Claude`  | Anthropic Claude                   | `text/html+mcp`            |
| `Nanobot` | MCP-UI host (Nanobot)              | `text/html`                |
| `McpJam`  | MCP-UI host (MCPJam)               | `text/html`                |
| `Generic` | Any standard MCP host              | `text/html+mcp`            |

### ChatGptAdapter

The `ChatGptAdapter` targets ChatGPT Apps (OpenAI Apps SDK). It injects a bridge script that wraps `window.openai` behind the universal `window.mcpBridge` API.

**Construction:**

```rust
use pmcp::server::mcp_apps::ChatGptAdapter;
use pmcp::types::mcp_apps::WidgetMeta;

let adapter = ChatGptAdapter::new()
    .with_widget_meta(
        WidgetMeta::new()
            .prefers_border(true)
            .description("Interactive chess board")
    );
```

**MIME type:** `text/html+skybridge` (`ExtendedUIMimeType::HtmlSkybridge`)

**Bridge mechanism:** The injected script checks for `window.openai` and wraps it behind `window.mcpBridge`. When the widget calls `mcpBridge.callTool()`, the bridge forwards to `window.openai.callTool()`. When it calls `mcpBridge.setState()`, the bridge calls `window.openai.setWidgetState()`.

**Bridge capabilities:** The ChatGptAdapter bridge is the most feature-rich, providing the full OpenAI Apps SDK surface:

| Category            | Method / Property                  | Description                                |
|---------------------|------------------------------------|--------------------------------------------|
| Core                | `callTool(name, args)`             | Call an MCP tool                           |
| State               | `getState()`                       | Read current widget state                  |
| State               | `setState(state)`                  | Update widget state (persists in session)  |
| Context             | `toolInput` (getter)               | Arguments supplied when the tool was invoked|
| Context             | `toolOutput` (getter)              | The structuredContent returned by the tool |
| Context             | `toolResponseMetadata` (getter)    | The `_meta` payload (widget-only)          |
| Communication       | `sendMessage(message)`             | Send a follow-up message to the conversation|
| Communication       | `openExternal(url)`                | Open an external URL                       |
| Files               | `uploadFile(file)`                 | Upload a file and get a file ID            |
| Files               | `getFileDownloadUrl(fileId)`       | Get a temporary download URL for a file    |
| Display             | `requestDisplayMode(mode)`         | Request inline, pip, or fullscreen         |
| Display             | `requestClose()`                   | Close the widget                           |
| Display             | `notifyIntrinsicHeight(height)`    | Report the widget's content height         |
| Display             | `setOpenInAppUrl(href)`            | Set the "Open in App" button URL           |
| Environment         | `theme` (getter)                   | Current theme (`'light'` or `'dark'`)      |
| Environment         | `locale` (getter)                  | Current locale (e.g., `'en-US'`)           |
| Environment         | `displayMode` (getter)             | Current display mode                       |
| Environment         | `maxHeight` (getter)               | Maximum widget height in pixels            |
| Environment         | `safeArea` (getter)                | Safe area insets                           |
| Environment         | `userAgent` (getter)               | User agent string                          |
| Environment         | `view` (getter)                    | Widget view type (`'default'` or `'compact'`)|

**WidgetMeta:** ChatGPT-specific metadata that controls how the host renders the widget. Added to the resource's `_meta` field:

| Field            | Serde key                    | Description                                    |
|------------------|------------------------------|------------------------------------------------|
| `prefers_border` | `openai/widgetPrefersBorder` | Whether the widget should have a border        |
| `domain`         | `openai/widgetDomain`        | Dedicated origin for the widget sandbox        |
| `description`    | `openai/widgetDescription`   | Widget self-description (reduces redundant text)|
| `csp`            | `openai/widgetCSP`           | Content Security Policy configuration          |

**CSP (Content Security Policy):** None by default. ChatGPT has its own CSP management. Use the `csp` field on `WidgetMeta` if your widget needs to fetch from external APIs or load external resources.

### McpAppsAdapter

The `McpAppsAdapter` targets the SEP-1865 standard for MCP Apps. It injects a `postMessage`-based JSON-RPC 2.0 bridge.

**Construction:**

```rust
use pmcp::server::mcp_apps::McpAppsAdapter;
use pmcp::types::mcp_apps::WidgetCSP;

let adapter = McpAppsAdapter::new()
    .with_csp(
        WidgetCSP::new()
            .connect("https://api.example.com")
            .resources("https://cdn.example.com")
    );
```

**MIME type:** `text/html+mcp` (`ExtendedUIMimeType::HtmlMcp`)

**Bridge mechanism:** The injected script uses `window.parent.postMessage()` to send JSON-RPC 2.0 requests to the host and listens for responses via `window.addEventListener('message', ...)`. Each request gets a unique ID, and the bridge matches responses to pending promises with a 30-second timeout.

**Bridge capabilities:**

| Method                      | Description                              |
|-----------------------------|------------------------------------------|
| `callTool(name, args)`      | Call an MCP tool via JSON-RPC            |
| `readResource(uri)`         | Read an MCP resource                     |
| `getPrompt(name, args)`     | Get a prompt                             |
| `notify(method, params)`    | Send a notification (fire-and-forget)    |

The McpAppsAdapter bridge is intentionally minimal -- it supports the core MCP operations without platform-specific features like state management or display modes. The bridge automatically sends a `ui/ready` notification when the widget loads.

**CSP:** Optional via `with_csp()`. The CSP is returned by `required_csp()` and can be used by the host to configure iframe sandbox permissions.

### McpUiAdapter

The `McpUiAdapter` targets MCP-UI community hosts (Nanobot, MCPJam, and others). It supports three output formats, not just HTML.

**Construction:**

```rust
use pmcp::server::mcp_apps::{McpUiAdapter, McpUiFormat};

// HTML format with postMessage bridge (default)
let adapter = McpUiAdapter::new();

// URL format -- returns a URL reference instead of HTML
let adapter = McpUiAdapter::new().with_format(McpUiFormat::Url);

// Remote DOM -- non-iframe rendering via Shopify Remote DOM
let adapter = McpUiAdapter::new().with_format(McpUiFormat::RemoteDom);
```

**Output formats:**

| Format      | MIME type                                              | Description                          |
|-------------|--------------------------------------------------------|--------------------------------------|
| `Html`      | `text/html`                                            | HTML with postMessage bridge         |
| `Url`       | `text/uri-list`                                        | URL reference (CDN-hosted widget)    |
| `RemoteDom` | `application/vnd.mcp-ui.remote-dom+javascript`         | Non-iframe JavaScript rendering      |

**Bridge capabilities (Html format):**

| Method                          | Description                              |
|---------------------------------|------------------------------------------|
| `callTool(name, args)`          | Call an MCP tool via JSON-RPC            |
| `readResource(uri)`             | Read an MCP resource                     |
| `getPrompt(name, args)`         | Get a prompt                             |
| `sendIntent(action, data)`      | Send a high-level intent to the host     |
| `notify(level, message)`        | Send a notification with severity level  |
| `openLink(url)`                 | Open a URL in the host                   |

The MCP-UI bridge includes `sendIntent` and `openLink` methods not present in the MCP Apps bridge, reflecting MCP-UI's richer host integration. The `notify` method takes a severity level (`info`, `success`, `warning`, `error`) rather than raw JSON-RPC method/params.

When using the `Url` format, `transform()` returns the URI string directly instead of HTML -- the host fetches the widget from the URL. When using `RemoteDom`, `transform()` returns a JSON descriptor with the resource URI and name.

**CSP:** None. MCP-UI hosts manage their own security policies.

### Choosing an Adapter

| Adapter          | Host          | MIME Type              | Bridge API               | Best For                    |
|------------------|---------------|------------------------|---------------------------|-----------------------------|
| `ChatGptAdapter` | ChatGPT       | `text/html+skybridge`  | `window.openai` wrapper   | ChatGPT Apps deployment     |
| `McpAppsAdapter` | Generic MCP   | `text/html+mcp`        | postMessage JSON-RPC      | Standard MCP hosts          |
| `McpUiAdapter`   | MCP-UI hosts  | `text/html` (or URL/RemoteDom) | postMessage JSON-RPC | Community MCP-UI hosts      |

All three shipped examples (chess, map, dataviz) use `ChatGptAdapter` because ChatGPT is the most mature host for interactive widgets. To support multiple hosts from a single server, use `MultiPlatformResource`:

```rust
use pmcp::server::mcp_apps::{
    MultiPlatformResource, ChatGptAdapter, McpAppsAdapter, McpUiAdapter,
};

let mut multi = MultiPlatformResource::new(
    "ui://app/board.html",
    "Chess Board",
    html,
)
.with_adapter(ChatGptAdapter::new())
.with_adapter(McpAppsAdapter::new())
.with_adapter(McpUiAdapter::new());

// Or add all standard adapters at once:
let mut multi = MultiPlatformResource::new(uri, name, html)
    .with_all_adapters();

// Get transformed content for a specific host:
use pmcp::types::mcp_apps::HostType;
if let Some(transformed) = multi.for_host(HostType::ChatGpt) {
    // transformed.content has the bridge-injected HTML
    // transformed.mime_type is text/html+skybridge
}
```

`MultiPlatformResource` caches transformations -- calling `for_host()` with the same host type twice reuses the cached result. Call `all_transforms()` to get every platform's version at once.

For most projects today, using `ChatGptAdapter` directly (as all the shipped examples do) is the simplest path. Switch to `MultiPlatformResource` when you need to serve the same widgets to multiple hosts from a single server.

---

## Example Walkthroughs

Three shipped examples demonstrate different MCP Apps patterns. Each follows the same structure: tool handlers + `WidgetDir` + `ChatGptAdapter` + `StreamableHttpServer`. What varies is the domain logic and the widget's visualization approach.

### Chess: Stateless Game Widget

The chess example (`examples/mcp-apps-chess/`) demonstrates the stateless widget pattern -- the widget holds ALL game state in memory, and each tool call includes the full `GameState` struct. The server validates and processes moves without storing any state between requests.

**Why stateless?** Stateless tools do not require server-side sessions, session ID generators, or state cleanup. The widget is the source of truth. If the user refreshes the page, the widget reloads with default state. This simplicity is the recommended default for MCP Apps widgets.

**Architecture:**

```text
Widget (board.html)
    |
    |  mcpBridge.callTool("chess_move", { state: {...}, move: "e2e4" })
    |
    v
Server
    |  Validates move against GameState
    |  Returns new GameState or error
    v
Widget updates board from new state
```

**Tools:**

| Tool               | Input                                | Output                             |
|--------------------|--------------------------------------|------------------------------------|
| `chess_new_game`   | (none)                               | Initial `GameState` with pieces    |
| `chess_move`       | `GameState` + move (e.g., `"e2e4"`) | New `GameState` or error           |
| `chess_valid_moves`| `GameState` + position (e.g., `"e2"`)| List of valid destination squares  |

**Key types:**

The `GameState` struct carries the full board state with every request:

```rust
pub struct GameState {
    pub board: [[Option<Piece>; 8]; 8],  // 8x8 board
    pub turn: Color,                      // Whose turn
    pub history: Vec<String>,             // Move history (algebraic)
    pub castling: CastlingRights,         // Castling availability
    pub en_passant: Option<Position>,     // En passant target
    pub status: GameStatus,               // InProgress, Check, etc.
}
```

The widget sends this entire struct with `chess_move` and `chess_valid_moves` calls. The server validates the move against the state, applies it, and returns the updated `GameState`. The widget then re-renders the board.

**Tool registration:**

```rust
let server = ServerBuilder::new()
    .name("chess-server")
    .version("1.0.0")
    .tool_typed_sync_with_description(
        "chess_new_game",
        "Start a new chess game. Returns the initial game state.",
        new_game_handler,
    )
    .tool_typed_sync_with_description(
        "chess_move",
        "Make a chess move. Requires current game state and move in algebraic notation.",
        move_handler,
    )
    .tool_typed_sync_with_description(
        "chess_valid_moves",
        "Get all valid moves for a piece at the given position.",
        valid_moves_handler,
    )
    .resources(ChessResources::new(widgets_path))
    .build()?;
```

Each tool handler is a synchronous function registered with `tool_typed_sync_with_description`. The input type (`MoveInput`, `ValidMovesInput`) derives `Deserialize` and `JsonSchema` for automatic schema generation. The server automatically validates incoming JSON against the schema before calling the handler.

**Server configuration:**

```rust
let config = StreamableHttpServerConfig {
    session_id_generator: None,   // No sessions needed -- stateless
    enable_json_response: true,
    event_store: None,
    on_session_initialized: None,
    on_session_closed: None,
    http_middleware: None,
};

let http_server = StreamableHttpServer::with_config(addr, server, config);
```

Note `session_id_generator: None` -- because the chess server is fully stateless, it does not need session tracking. This is the simplest `StreamableHttpServer` configuration.

**Widget:** `widgets/board.html` renders an interactive chess board. Clicking a piece calls `chess_valid_moves` to highlight valid destinations. Clicking a destination calls `chess_move` to apply the move. All state lives in the widget's JavaScript -- the server never stores game state.

**Running:**

```bash
cd examples/mcp-apps-chess
cargo run
# Server starts on http://localhost:3000

# In another terminal:
cargo pmcp preview --url http://localhost:3000 --open
```

### Map: Geographic Data Explorer

The map example (`examples/mcp-apps-map/`) demonstrates a data exploration widget with Leaflet.js for interactive map rendering.

**Architecture:** The server holds a mock database of world cities with geographic coordinates, categories, and descriptions. The widget renders a Leaflet.js map with markers and popups. Search queries and category filters flow from the widget to the server, and city data flows back for map display.

**Tools:**

| Tool               | Input                                             | Output                               |
|--------------------|----------------------------------------------------|--------------------------------------|
| `search_cities`    | Optional query, optional category, optional `MapState` | Matching cities with coordinates  |
| `get_city_details` | City ID                                            | Full city details + suggested zoom   |
| `get_nearby_cities`| Center coordinates + radius in km                  | Cities within radius + distances     |

**Context-aware queries:** The `search_cities` tool accepts an optional `MapState` parameter with the current map view (center coordinates, zoom level, selected city, active filter). This lets the server return context-aware responses -- for example, prioritizing cities visible in the current viewport.

```rust
pub struct MapState {
    pub center: Coordinates,
    pub zoom: u8,
    pub selected_city: Option<String>,
    pub filter: Option<CityCategory>,
}
```

**City categories:** Cities are tagged with a `CityCategory` enum (`Capital`, `Tech`, `Cultural`, `Financial`, `Historical`) that the widget can use for filtering.

**Distance calculation:** The `get_nearby_cities` tool uses the Haversine formula to calculate great-circle distances between geographic points:

```rust
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();
    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_KM * c
}
```

**Widget:** `widgets/map.html` loads Leaflet.js from CDN (`https://unpkg.com/leaflet@1.9.4/`) and renders an interactive map. The search bar calls `search_cities`, markers show city locations, and clicking a marker shows details via `get_city_details`. Category filter buttons narrow results by type.

**Running:**

```bash
cd examples/mcp-apps-map
cargo run
# Server starts on http://localhost:3001

cargo pmcp preview --url http://localhost:3001 --open
```

### Dataviz: SQL Dashboard

The dataviz example (`examples/mcp-apps-dataviz/`) demonstrates a database exploration widget with Chart.js for interactive data visualization.

**Architecture:** The server opens a local Chinook SQLite database (a standard sample database with music store data -- artists, albums, tracks, invoices) and exposes SQL query tools. The widget renders query results as bar, line, and pie charts plus a sortable data table.

**Tools:**

| Tool              | Input          | Output                                    |
|-------------------|----------------|-------------------------------------------|
| `execute_query`   | SQL string     | Columns, rows (as JSON arrays), row count |
| `list_tables`     | (none)         | List of table names                       |
| `describe_table`  | Table name     | Column metadata (name, type, nullable, PK)|

**SQL injection prevention:** The `describe_table` handler validates the table name to allow only alphanumeric characters and underscores before using it in a `PRAGMA` query:

```rust
if !input.table_name.chars()
    .all(|c| c.is_alphanumeric() || c == '_')
{
    return Ok(json!({
        "error": "Invalid table name: only alphanumeric characters \
                  and underscores are allowed"
    }));
}
```

The `execute_query` tool accepts arbitrary SQL and returns structured results. In a production server you would add authorization checks and query whitelisting, but for a demo the open query interface lets users explore the data freely.

**Structured error handling:** All three tool handlers return JSON error objects rather than panicking. If the database file is missing, the handler returns a helpful error message with download instructions:

```rust
fn open_db() -> Result<Connection, String> {
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Chinook.db");
    if !db_path.exists() {
        return Err(format!(
            "Chinook.db not found at {}. Please download it:\n\
             cd examples/mcp-apps-dataviz\n\
             curl -L -o Chinook.db https://github.com/lerocha/chinook-database/\
             releases/download/v1.4.5/Chinook_Sqlite.sqlite",
            db_path.display()
        ));
    }
    Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))
}
```

**Widget:** `widgets/dashboard.html` loads Chart.js from CDN (`https://cdn.jsdelivr.net/npm/chart.js@4`) and renders an interactive dashboard. Users type SQL queries in a text area, click "Run", and the results appear as both a chart and a sortable data table. The widget auto-detects numeric columns for charting and supports bar, line, and pie chart types.

**Prerequisites:**

```bash
cd examples/mcp-apps-dataviz
# Download the Chinook sample database
curl -L -o Chinook.db \
  https://github.com/lerocha/chinook-database/releases/download/v1.4.5/Chinook_Sqlite.sqlite
```

**Running:**

```bash
cargo run
# Server starts on http://localhost:3002

cargo pmcp preview --url http://localhost:3002 --open
```

### Common Architecture Pattern

All three examples share the same structure. Once you understand this pattern, you can build any widget-based MCP server:

**Step 1: Define tool input types**

```rust
#[derive(Deserialize, JsonSchema)]
struct MyToolInput {
    query: String,
    filter: Option<String>,
}
```

Derive `Deserialize` for JSON parsing and `JsonSchema` for automatic schema generation. The server advertises the schema to clients so they know what arguments each tool accepts.

**Step 2: Write synchronous tool handlers**

```rust
fn my_tool_handler(input: MyToolInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Process input, return JSON result
    Ok(json!({ "result": "data" }))
}
```

Handlers are pure functions: input in, JSON out. No `async` needed for most tools.

**Step 3: Create a ResourceHandler with ChatGptAdapter + WidgetDir**

```rust
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,
    widget_dir: WidgetDir,
}
```

This struct holds the adapter and widget directory. The `ResourceHandler` implementation follows the three-step pattern from the Widget Authoring section: extract name from URI, read HTML from disk, transform with adapter.

**Step 4: Build and run the server**

```rust
let server = ServerBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool_typed_sync_with_description("my_tool", "Description", my_tool_handler)
    .resources(AppResources::new(widgets_path))
    .build()?;

let http_server = StreamableHttpServer::with_config(addr, Arc::new(Mutex::new(server)), config);
let (bound_addr, handle) = http_server.start().await?;
```

`StreamableHttpServer` provides HTTP access to the MCP server. The `with_config` constructor accepts a `StreamableHttpServerConfig` where you can optionally enable sessions, event stores, and middleware.

This four-step pattern is the recommended way to build MCP Apps. All three shipped examples follow it exactly.

---

## Best Practices

- **Keep widgets as single self-contained HTML files.** External CDN libraries (Leaflet.js, Chart.js) are fine -- they are loaded at runtime and do not affect the server. Avoid multi-file widget bundles; the `WidgetDir` convention is one `.html` file per widget.

- **Use `window.mcpBridge.callTool()` as the universal bridge API.** This works on all platforms. Do not call `window.openai` or `window.parent.postMessage` directly -- the bridge handles platform differences for you.

- **Design stateless tools when possible.** Let the widget own state and send it with each request. The server validates and processes without storing anything. This eliminates session management, simplifies scaling, and makes widgets portable between hosts.

- **Use hot-reload during development.** Start the server once with `cargo run`, then edit widget HTML and refresh the browser. `WidgetDir` re-reads files from disk on every request -- no server restart needed.

- **Test with `cargo pmcp preview` before deploying.** The preview environment simulates the ChatGPT Apps runtime, including theme switching, locale testing, and tool parameter editing. Use `--theme dark` and `--locale ja-JP` to verify your widget handles different environments.

- **Use `cargo pmcp app build` to produce distribution artifacts.** The manifest and landing page are generated from your running server and mock data, ready for upload to a ChatGPT Apps directory or sharing as a demo.

- **Handle errors gracefully in both server and widget.** Server handlers should return JSON error objects, not panic. Widget code should wrap `mcpBridge` calls in try/catch and show user-friendly error messages.

- **Validate inputs on the server side.** Even though the widget sends structured data, treat all input as untrusted. The dataviz example shows table name validation to prevent SQL injection; apply similar patterns to your domain.

---

## Summary

This chapter covered the MCP Apps Extension -- the system for building interactive UIs served from MCP servers:

- **WidgetDir** provides file-based widget authoring with hot-reload. Widgets are `.html` files in a `widgets/` directory, read from disk on every request for instant iteration.

- **`window.mcpBridge`** is the universal bridge API for widget-server communication. Write `mcpBridge.callTool()` once, and the adapter injects the correct platform-specific bridge script.

- **`cargo pmcp`** provides the full developer workflow: `app new` to scaffold, `preview` for live testing with theme/locale simulation, and `app build` to produce `manifest.json` and `landing.html` for distribution.

- **The adapter pattern** enables write-once deployment across ChatGPT Apps (`ChatGptAdapter`, `text/html+skybridge`), MCP Apps (`McpAppsAdapter`, `text/html+mcp`), and MCP-UI hosts (`McpUiAdapter`, `text/html`). `MultiPlatformResource` serves all three from a single server.

- **Three shipped examples** demonstrate real-world patterns:
  - **Chess** -- stateless game state, move validation, interactive board widget
  - **Map** -- geographic data with Leaflet.js, Haversine distance, category filtering
  - **Dataviz** -- SQL query execution with Chart.js dashboard, table name validation

The common architecture pattern across all examples is: define tool input types with `Deserialize` + `JsonSchema`, write synchronous handlers, create a `ResourceHandler` with `ChatGptAdapter` + `WidgetDir`, build with `ServerBuilder`, and run with `StreamableHttpServer`.

For production deployment, see [Chapter 13: Building Production Servers](ch13-production.md) for server hardening, [Chapter 14: Performance and Load Testing](ch14-performance.md) for benchmarking widget-heavy workloads, and [Chapter 15: Testing MCP Servers](ch15-testing.md) for integration testing MCP Apps servers.
