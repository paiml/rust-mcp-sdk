# Widget Authoring and Developer Workflow

In this section, you'll build widgets using the `WidgetDir` file-based convention and learn the full `cargo pmcp` development cycle -- from scaffolding a new project to building distribution artifacts.

## Learning Objectives

By the end of this section, you will be able to:

- Create a new MCP Apps project with `cargo pmcp app new`
- Author widgets as standalone HTML files in the `widgets/` directory
- Use the hot-reload development workflow (edit HTML, refresh browser)
- Implement the `ResourceHandler` pattern that connects widgets to the MCP protocol
- Build distribution artifacts with `cargo pmcp app build`

## The WidgetDir Convention

Widgets are just HTML files. Drop them in a folder. The server does the rest.

### File-to-URI Mapping

Every `.html` file you place in the `widgets/` directory automatically becomes an MCP resource. The filename (without extension) maps directly to a `ui://app/{name}` URI:

| File on Disk             | MCP Resource URI      |
|--------------------------|-----------------------|
| `widgets/hello.html`    | `ui://app/hello`      |
| `widgets/board.html`    | `ui://app/board`      |
| `widgets/map.html`      | `ui://app/map`        |
| `widgets/dashboard.html`| `ui://app/dashboard`  |

That's the entire convention. No configuration files. No registration code. One HTML file equals one widget.

### WidgetDir API Walkthrough

The `WidgetDir` struct lives in `pmcp::server::mcp_apps` and provides three operations. Let's walk through each one.

**Construction:**

```rust
use pmcp::server::mcp_apps::WidgetDir;

// Point at the widgets directory
let widget_dir = WidgetDir::new("widgets");

// The path does not need to exist at construction time.
// Errors surface when you call discover() or read_widget().
```

**Discovery:**

```rust
// Scan for .html files -- returns Vec<WidgetEntry> sorted by filename
let entries = widget_dir.discover()?;

for entry in &entries {
    println!("{} -> {}", entry.filename, entry.uri);
    // "board" -> "ui://app/board"
    // "hello" -> "ui://app/hello"
    // "map"   -> "ui://app/map"
}
```

Each `WidgetEntry` has three fields:

| Field      | Type       | Description                                   |
|------------|------------|-----------------------------------------------|
| `filename` | `String`   | Stem of the HTML file (e.g., `"board"`)       |
| `uri`      | `String`   | MCP resource URI (e.g., `"ui://app/board"`)   |
| `path`     | `PathBuf`  | Absolute path to the `.html` file on disk     |

**Reading:**

```rust
// Read widget HTML from disk -- fresh on every call
let html = widget_dir.read_widget("board");
```

`read_widget` reads from disk on every call. There is no cache. This is intentional -- it enables the hot-reload development workflow you'll use throughout this chapter.

If the file does not exist, `read_widget` returns a styled HTML error page with a hint: "Create or fix the widget file and refresh the browser to retry."

**Bridge injection:**

```rust
// Insert a <script> tag into widget HTML
let html_with_bridge = WidgetDir::inject_bridge_script(
    &html,
    "/assets/widget-runtime.mjs",
);
```

The injection inserts the script tag just before `</head>` if present, at the start of `<body>` otherwise, or at the very beginning of the document if neither tag is found. In practice, you rarely call this directly -- the adapter handles injection for you.

**Try this:** Create a second `.html` file in your `widgets/` directory (even an empty `<html></html>` will work) and call `widget_dir.discover()`. Verify that it appears in the returned list alongside the original widget.

## Hands-On: Scaffold Your First MCP App

### Step 1: Create the Project

```bash
cargo pmcp app new my-widget-app
cd my-widget-app
```

You'll see the following project structure:

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

Each file has a purpose:

| File               | Purpose                                                    |
|--------------------|------------------------------------------------------------|
| `src/main.rs`      | Your MCP server -- registers tools and serves widgets      |
| `widgets/hello.html` | A starter widget that calls the `hello` tool via the bridge |
| `mock-data/hello.json` | Hardcoded response for the landing page demo            |
| `Cargo.toml`       | Dependencies including `pmcp` with `mcp-apps` feature      |

### Step 2: Explore the Generated Code

Open `src/main.rs`. You'll see the three-part pattern that every MCP Apps server uses. Let's walk through the key sections.

**The tool handler:**

```rust
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
```

This is a standard MCP tool. It takes a name and returns a greeting. Nothing widget-specific here.

**The resource handler:**

```rust
struct AppResources {
    chatgpt_adapter: ChatGptAdapter,   // Injects the bridge script
    widget_dir: WidgetDir,             // Discovers and reads widgets
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
```

This struct holds the adapter (which injects the bridge script) and the `WidgetDir` (which discovers and reads widget files). Together, they connect your HTML widgets to the MCP protocol.

**The server builder:**

```rust
let server = ServerBuilder::new()
    .name("my-widget-app")
    .version("0.1.0")
    .tool_typed_sync_with_description("hello", "Greet someone by name", hello_handler)
    .resources(AppResources::new(widgets_path))
    .build()?;
```

The builder registers the tool and the resource handler together. When a client requests `ui://app/hello`, the `AppResources` handler reads `widgets/hello.html` from disk and serves it with the bridge script injected.

**Now open `widgets/hello.html`.** Notice there is no bridge `<script>` tag in the file. The adapter injects it for you at serve time. Your widget code is clean HTML with `window.mcpBridge` calls:

```javascript
// Call the "hello" tool via the MCP bridge
const response = await window.mcpBridge.callTool('hello', { name });
document.getElementById('result').textContent = response.greeting;
```

### Step 3: Run and Preview

Open two terminal windows:

```bash
# Terminal 1: Start the MCP server
cargo run
```

```bash
# Terminal 2: Open the browser-based preview
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser. Type a name, click the button -- the widget calls your `hello` tool and displays the greeting response.

**Try this:** Edit the CSS in `widgets/hello.html` -- change the background color, the font size, anything. Refresh your browser. Your changes appear instantly. No server restart needed.

### Step 4: Add a Second Widget

Let's create a counter widget. Create `widgets/counter.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Counter</title>
    <style>
        body {
            font-family: system-ui, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: #f0f4f8;
        }
        .card {
            background: white;
            border-radius: 12px;
            padding: 32px;
            text-align: center;
            box-shadow: 0 2px 12px rgba(0,0,0,0.08);
        }
        #count { font-size: 3rem; margin: 16px 0; }
        button { padding: 8px 24px; font-size: 1rem; cursor: pointer; }
    </style>
</head>
<body>
    <div class="card">
        <h1>Counter</h1>
        <div id="count">0</div>
        <button id="increment">Increment</button>
        <div id="status"></div>
    </div>

    <script>
        let count = 0;

        document.getElementById('increment').addEventListener('click', async () => {
            count++;
            document.getElementById('count').textContent = count;

            try {
                // Call the counter tool to log the count
                const result = await window.mcpBridge.callTool('counter', { count });
                document.getElementById('status').textContent = result.message;
            } catch (err) {
                document.getElementById('status').textContent = 'Error: ' + err.message;
            }
        });
    </script>
</body>
</html>
```

Then add a `counter` tool handler in `src/main.rs`:

```rust
#[derive(Deserialize, JsonSchema)]
struct CounterInput {
    count: u64,
}

fn counter_handler(input: CounterInput, _extra: RequestHandlerExtra) -> Result<serde_json::Value> {
    Ok(json!({
        "message": format!("Count is now {}", input.count),
        "count": input.count
    }))
}
```

Register it with the server builder:

```rust
.tool_typed_sync_with_description("counter", "Track a counter value", counter_handler)
```

Restart the server (you changed Rust code, so a restart is required), then refresh the preview. Your new counter widget appears in the widget selector.

## The ResourceHandler Pattern

Every MCP Apps server needs this pattern. Learn it once, use it everywhere.

The `ResourceHandler` trait connects `WidgetDir` and the adapter to the MCP protocol. It has two methods: `read()` for serving a specific widget, and `list()` for discovering all available widgets.

### The read() Method

The `read()` method follows three steps every time:

```rust
#[async_trait]
impl ResourceHandler for AppResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra)
        -> Result<ReadResourceResult>
    {
        // Step 1: Extract widget name from URI
        let name = uri
            .strip_prefix("ui://app/")
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            // Step 2: Read HTML from disk (fresh every time -- hot-reload)
            let html = self.widget_dir.read_widget(widget_name);

            // Step 3: Transform for target host (injects bridge script)
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
```

The three steps are always the same:

| Step | What Happens                                | Code                                    |
|------|---------------------------------------------|-----------------------------------------|
| 1    | Extract widget name from `ui://app/{name}`  | `uri.strip_prefix("ui://app/")`         |
| 2    | Read HTML from disk (no cache, hot-reload)   | `widget_dir.read_widget(widget_name)`   |
| 3    | Transform via adapter (inject bridge script) | `adapter.transform(uri, name, &html)`  |

### The list() Method

The `list()` method calls `widget_dir.discover()` and maps each `WidgetEntry` to a `ResourceInfo`:

```rust
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
                mime_type: Some(
                    ExtendedUIMimeType::HtmlSkybridge.to_string()
                ),
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}
```

This exact `ResourceHandler` pattern appears in the chess, map, and dataviz examples. Once you understand it, you can build any widget-based MCP server.

## Hot-Reload Development

MCP Apps development feels like frontend development. Here is the workflow:

```
  1. Start server        2. Open preview         3. Edit HTML         4. Refresh browser
  ================       =================       ==============       ==================
  cargo run              cargo pmcp preview      vim widgets/         Browser refresh
                         --url ... --open        hello.html           -> instant update
```

1. Start your server: `cargo run`
2. Open the preview: `cargo pmcp preview --url http://localhost:3000 --open`
3. Edit your HTML file in `widgets/`
4. Refresh the browser -- your changes appear instantly

Why does this work? `WidgetDir` reads from disk on every request. There is no cache. The server re-reads the file each time a client requests the widget resource.

**When do you need to restart the server?** Only when you change Rust code (tool handlers, `main.rs`). Widget HTML changes are always instant -- just refresh the browser.

**Try this:** With the server running and preview open, add a `<p>` tag to your widget. Refresh the browser. Then delete it and refresh again. Get a feel for the instant feedback loop.

## Building for Distribution

Ready to share your widget? The build command produces two deployment artifacts.

### cargo pmcp app build

```bash
cargo pmcp app build --url https://my-server.example.com
```

This generates:

```
dist/
  manifest.json    # ChatGPT-compatible app directory listing
  landing.html     # Standalone demo page with mock bridge
```

**What each artifact is for:**

| Artifact         | Purpose                                                                  |
|------------------|--------------------------------------------------------------------------|
| `manifest.json`  | ChatGPT-compatible app directory listing following the `ai-plugin.json` schema. Upload this to make your server discoverable. |
| `landing.html`   | Standalone demo page that embeds your widget with a mock bridge. Uses hardcoded responses from `mock-data/*.json`. Works without a running server. |

### Individual Commands

If you only need one artifact, use the subcommands directly:

```bash
# Generate only manifest.json
cargo pmcp app manifest --url https://my-server.example.com

# Generate only manifest.json with a logo
cargo pmcp app manifest --url https://my-server.example.com --logo https://example.com/logo.png

# Generate only landing.html
cargo pmcp app landing

# Generate landing.html for a specific widget
cargo pmcp app landing --widget board --output build
```

### CLI Flags Reference

| Flag              | Description                                     | Default              |
|-------------------|-------------------------------------------------|----------------------|
| `--url <URL>`     | Server URL for manifest (required for build/manifest) | --              |
| `--logo <URL>`    | Logo URL for the manifest                       | --                   |
| `--widget <NAME>` | Widget to showcase in landing page              | First alphabetically |
| `--output <DIR>`  | Output directory for generated files            | `dist`               |

**Try this:** Run `cargo pmcp app build --url http://localhost:3000` and then open `dist/landing.html` in your browser. The landing page shows your widget running with mock data -- no server needed.

## Preview Command Deep Dive

The `cargo pmcp preview` command connects to a running MCP server and renders widgets in a browser-based testing environment.

```bash
cargo pmcp preview --url http://localhost:3000 --open
```

### Flags Reference

| Flag                     | Description                                      | Default   |
|--------------------------|--------------------------------------------------|-----------|
| `--url <URL>`            | URL of the running MCP server (required)         | --        |
| `--port <PORT>`          | Port for the preview server                      | `8765`    |
| `--open`                 | Open browser automatically                       | `false`   |
| `--tool <NAME>`          | Auto-select this tool on start                   | --        |
| `--theme <light\|dark>`  | Initial theme for the preview environment        | `light`   |
| `--locale <LOCALE>`      | Initial locale (e.g., `en-US`, `ja-JP`)          | `en-US`   |
| `--widgets-dir <PATH>`   | Path to widgets directory for hot-reload         | --        |

**Environment simulation:** The `--theme` and `--locale` flags let you test how your widget behaves in different environments without switching hosts. The `--tool` flag auto-selects a specific tool when the preview loads, which is useful when your server has many tools.

**Try this:** Test your widget with dark mode and Japanese locale:

```bash
cargo pmcp preview --url http://localhost:3000 --open --theme dark --locale ja-JP
```

## Summary and Next Steps

Let's recap what you've learned:

- **WidgetDir** maps `.html` files in `widgets/` to `ui://app/{name}` URIs automatically
- **`cargo pmcp app new`** scaffolds a complete project with server, widget, and mock data
- **The ResourceHandler pattern** connects widgets to MCP: extract name, read from disk, transform with adapter
- **Hot-reload** works because `WidgetDir` reads from disk on every request -- no cache
- **`cargo pmcp app build`** produces `manifest.json` and `landing.html` for distribution
- **`cargo pmcp preview`** opens a browser-based testing environment with theme and locale simulation

In the next section, you'll learn how widgets communicate with your server through the bridge API, and how adapters make your widgets work across different hosts.

---

*Continue to [Bridge Communication and Adapters](./ch20-02-tool-ui-association.md) ->*
