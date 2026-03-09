# Bridge Communication and Adapters

In this section, you'll learn how widgets talk to your server through the bridge API, and how adapters let you deploy the same widget to ChatGPT, standard MCP hosts, and MCP-UI hosts without changing a line of widget code.

## Learning Objectives

By the end of this section, you will be able to:

- Use `window.mcpBridge.callTool()` to call server tools from widget JavaScript
- Handle bridge initialization, errors, and async responses
- Understand how different hosts (ChatGPT, MCP Apps, MCP-UI) use different bridge mechanisms
- Choose the right adapter for your target host
- Use `MultiPlatformResource` to serve all hosts from a single server

## The Bridge API

Your widget needs to talk to your MCP server. The bridge makes this simple -- one API that works everywhere.

### window.mcpBridge

When the adapter injects the bridge script into your widget HTML, it creates a `window.mcpBridge` object with four core methods:

| Method                           | Returns    | Description                           |
|----------------------------------|------------|---------------------------------------|
| `mcpBridge.callTool(name, args)` | `Promise`  | Call an MCP tool, get the result      |
| `mcpBridge.readResource(uri)`    | `Promise`  | Read an MCP resource                  |
| `mcpBridge.getPrompt(name, args)`| `Promise`  | Get a prompt                          |
| `mcpBridge.notify(method, params)` | `void`   | Send a notification (fire-and-forget) |

These four methods work on ALL hosts. Write them once, they work everywhere -- ChatGPT, MCP Apps, MCP-UI.

### Hands-On: Making Your First Bridge Call

Here is a minimal widget that calls a tool and displays the result. You saw a version of this in the scaffolded `hello.html`, but let's break down exactly what's happening:

```html
<button id="greet">Say Hello</button>
<div id="result"></div>

<script>
    document.getElementById('greet').addEventListener('click', async () => {
        const name = document.getElementById('name').value.trim();
        if (!name) return;

        try {
            // Call the "hello" tool via the MCP bridge
            const result = await window.mcpBridge.callTool('hello', { name });

            // The result is the JSON object your tool handler returned
            document.getElementById('result').textContent = result.greeting;
        } catch (err) {
            document.getElementById('result').textContent = 'Error: ' + err.message;
        }
    });
</script>
```

The key line is:

```javascript
const result = await window.mcpBridge.callTool('hello', { name });
```

This calls the `hello` tool on your MCP server with `{ name: "World" }` as the argument. The bridge handles all the protocol plumbing -- you just get a Promise that resolves to the tool's return value.

**Try this:** Modify the hello widget from Ch 20-01 to make a second tool call. Add a `callTool('counter', { count: 1 })` call after the greeting and display both results.

### Error Handling

Always wrap bridge calls in try/catch. The bridge throws if the tool returns an error, the server is unreachable, or the request times out:

```javascript
async function greet(name) {
    try {
        const result = await window.mcpBridge.callTool('hello', { name });
        document.getElementById('result').textContent = result.greeting;
    } catch (err) {
        // err.message contains the error description
        document.getElementById('result').textContent =
            'Error: ' + (err.message || String(err));
    }
}
```

Common errors you'll encounter during development:

| Error                      | Cause                                | Fix                                  |
|----------------------------|--------------------------------------|--------------------------------------|
| Tool not found             | Tool name doesn't match server       | Check the name in `.tool()` registration |
| Server unreachable         | Server not running                   | Start the server with `cargo run`    |
| Timeout (30s)              | Tool takes too long                  | Optimize the tool or increase timeout |
| Invalid arguments          | Schema mismatch                      | Check the tool's input types         |

### Bridge Initialization

In most cases, the bridge is available immediately when your widget loads. But for widgets that load asynchronously (e.g., dynamic imports), use the `mcpBridgeReady` event:

```javascript
// Pattern: Wait for bridge, then initialize
window.addEventListener('mcpBridgeReady', () => {
    // Bridge is ready -- safe to call mcpBridge methods
    loadInitialData();
});

// If bridge is already ready (script loaded synchronously)
if (window.mcpBridge) {
    loadInitialData();
}
```

The `mcpBridgeReady` event is dispatched by the injected bridge script after it sets up `window.mcpBridge`. For most widgets, you can call `mcpBridge` methods directly in a `<script>` tag without listening for this event.

### ChatGPT-Only Extras

When your widget runs inside ChatGPT, the bridge provides additional capabilities beyond the four core methods. These are not available on other hosts.

**State management:**

| Method                      | Returns   | Description                                |
|-----------------------------|-----------|--------------------------------------------|
| `mcpBridge.setState(state)` | `void`    | Update widget state (persists in session)  |
| `mcpBridge.getState()`      | `Object`  | Read current widget state                  |

**Communication:**

| Method                            | Returns  | Description                                 |
|-----------------------------------|----------|---------------------------------------------|
| `mcpBridge.sendMessage(message)`  | `void`   | Send a follow-up message to the conversation |
| `mcpBridge.openExternal(url)`     | `void`   | Open an external URL                         |

**Display modes:**

| Method                                | Returns   | Description                            |
|---------------------------------------|-----------|----------------------------------------|
| `mcpBridge.requestDisplayMode(mode)`  | `Promise` | Request inline, pip, or fullscreen     |
| `mcpBridge.requestClose()`            | `void`    | Close the widget                       |
| `mcpBridge.notifyIntrinsicHeight(h)`  | `void`    | Report the widget's content height     |
| `mcpBridge.setOpenInAppUrl(href)`     | `void`    | Set the "Open in App" button URL       |

**Environment properties (read-only):**

| Property                 | Type     | Description                                   |
|--------------------------|----------|-----------------------------------------------|
| `mcpBridge.theme`        | `string` | Current theme (`'light'` or `'dark'`)         |
| `mcpBridge.locale`       | `string` | Current locale (e.g., `'en-US'`)              |
| `mcpBridge.displayMode`  | `string` | Current display mode                          |
| `mcpBridge.toolInput`    | `object` | Arguments supplied when the tool was invoked  |
| `mcpBridge.toolOutput`   | `object` | The structuredContent returned by the tool    |

These extras are available when running inside ChatGPT. On other hosts, stick to the four core methods (`callTool`, `readResource`, `getPrompt`, `notify`).

## Communication Flow

Let's trace exactly what happens when your widget calls `mcpBridge.callTool('hello', { name: 'World' })`:

```
Widget (iframe)                    Host                      MCP Server
     |                              |                            |
     |  mcpBridge.callTool(         |                            |
     |    'hello', { name: 'World' }|                            |
     |  )                           |                            |
     |                              |                            |
     |  ---- bridge script ---->    |                            |
     |       (postMessage or        |                            |
     |        window.openai)        |                            |
     |                              |                            |
     |                              |  -- tools/call -------->   |
     |                              |     { name: 'hello',       |
     |                              |       arguments: {...} }   |
     |                              |                            |
     |                              |  <-- result -----------    |
     |                              |     { greeting: '...' }    |
     |                              |                            |
     |  <-- response ----------     |                            |
     |      Promise resolves        |                            |
     |      with { greeting: '...' }|                            |
     v                              v                            v
```

Here is each step in detail:

1. **Widget calls bridge:** Your JavaScript calls `window.mcpBridge.callTool('hello', { name: 'World' })`. The bridge creates a Promise and assigns a unique request ID.

2. **Bridge script forwards to host:** The bridge script translates the call into the host's native mechanism. On ChatGPT, it calls `window.openai.callTool()`. On MCP Apps and MCP-UI hosts, it sends a `postMessage` with JSON-RPC 2.0 framing.

3. **Host sends MCP request:** The host sends a `tools/call` request to your MCP server using the standard MCP protocol.

4. **Server processes and responds:** Your tool handler runs, returns a result, and the response flows back through the host to the bridge.

5. **Promise resolves:** The bridge matches the response to the pending Promise by request ID, and your widget's `await` completes with the tool's return value.

The key insight is that platform differences are hidden by the bridge. ChatGPT uses `window.openai` under the hood. MCP Apps uses `postMessage` with JSON-RPC. You don't need to know this -- the bridge handles it.

## The Adapter Pattern

One widget. Three hosts. Zero code changes. Here is how.

### Why Adapters?

Different hosts use different communication mechanisms to talk to widgets:

| Host         | Native Mechanism      | MIME Type             | Bridge Script Wraps              |
|--------------|-----------------------|-----------------------|----------------------------------|
| ChatGPT      | `window.openai`       | `text/html+skybridge` | `window.openai` -> `mcpBridge`  |
| MCP Apps     | `postMessage` JSON-RPC| `text/html+mcp`       | `postMessage` -> `mcpBridge`    |
| MCP-UI       | `postMessage` JSON-RPC| `text/html`           | `postMessage` -> `mcpBridge`    |

Without adapters, you'd need three versions of every widget. With adapters, you write ONE widget using `window.mcpBridge`, and the adapter injects the right bridge script at serve time.

### The UIAdapter Trait

The `UIAdapter` trait defines what every adapter must do:

```rust
pub trait UIAdapter: Send + Sync {
    /// Which host platform this adapter targets
    fn host_type(&self) -> HostType;

    /// The MIME type this adapter produces
    fn mime_type(&self) -> ExtendedUIMimeType;

    /// Transform HTML content for this host platform
    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource;

    /// Inject platform-specific bridge script into HTML
    fn inject_bridge(&self, html: &str) -> String;

    /// Get CSP headers required by this platform
    fn required_csp(&self) -> Option<WidgetCSP>;
}
```

The `transform()` method is what you call in your `ResourceHandler`. It reads the widget HTML, calls `inject_bridge()` to insert the platform-specific bridge script, and returns a `TransformedResource` with the correct MIME type and metadata.

The `TransformedResource` struct carries everything needed to serve the widget:

| Field      | Type                       | Description                                  |
|------------|----------------------------|----------------------------------------------|
| `uri`      | `String`                   | Original resource URI                        |
| `name`     | `String`                   | Display name                                 |
| `mime_type`| `ExtendedUIMimeType`       | Platform-specific MIME type                  |
| `content`  | `String`                   | Transformed HTML with bridge script injected |
| `metadata` | `HashMap<String, Value>`   | Platform-specific metadata (e.g., WidgetMeta)|

### ChatGptAdapter

The `ChatGptAdapter` targets ChatGPT Apps (OpenAI Apps SDK). It injects a bridge script that wraps `window.openai` behind the universal `window.mcpBridge` API.

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

This is what all three shipped examples (chess, map, dataviz) use -- ChatGPT is the most mature host for interactive widgets.

**WidgetMeta fields:**

| Field            | Serde Key                    | Description                                    |
|------------------|------------------------------|------------------------------------------------|
| `prefers_border` | `openai/widgetPrefersBorder` | Whether the widget should have a border        |
| `domain`         | `openai/widgetDomain`        | Dedicated origin for the widget sandbox        |
| `description`    | `openai/widgetDescription`   | Widget self-description                        |
| `csp`            | `openai/widgetCSP`           | Content Security Policy configuration          |

### McpAppsAdapter

The `McpAppsAdapter` targets the SEP-1865 standard for MCP Apps. It injects a `postMessage`-based JSON-RPC 2.0 bridge.

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

The McpAppsAdapter bridge is intentionally minimal -- it supports the four core MCP operations (`callTool`, `readResource`, `getPrompt`, `notify`) without platform-specific features like state management or display modes. The bridge automatically sends a `ui/ready` notification when the widget loads.

### McpUiAdapter

The `McpUiAdapter` targets MCP-UI community hosts (Nanobot, MCPJam, and others). It supports three output formats, not just HTML.

```rust
use pmcp::server::mcp_apps::{McpUiAdapter, McpUiFormat};

// HTML format with postMessage bridge (default)
let adapter = McpUiAdapter::new();

// URL format -- returns a URL reference instead of HTML
let adapter = McpUiAdapter::new().with_format(McpUiFormat::Url);

// Remote DOM -- non-iframe rendering
let adapter = McpUiAdapter::new().with_format(McpUiFormat::RemoteDom);
```

**Three output formats:**

| Format      | MIME Type                                          | Description                         |
|-------------|----------------------------------------------------|-------------------------------------|
| `Html`      | `text/html`                                        | HTML with postMessage bridge        |
| `Url`       | `text/uri-list`                                    | URL reference (CDN-hosted widget)   |
| `RemoteDom` | `application/vnd.mcp-ui.remote-dom+javascript`     | Non-iframe JavaScript rendering     |

MCP-UI hosts have extra bridge methods not available on other platforms:

| Method                          | Description                              |
|---------------------------------|------------------------------------------|
| `sendIntent(action, data)`      | Send a high-level intent to the host     |
| `openLink(url)`                 | Open a URL in the host                   |
| `notify(level, message)`        | Send a notification with severity level  |

### Choosing an Adapter

Here is a comparison to help you decide:

| Adapter          | Host          | MIME Type              | Bridge API               | Best For                    |
|------------------|---------------|------------------------|---------------------------|-----------------------------|
| `ChatGptAdapter` | ChatGPT       | `text/html+skybridge`  | `window.openai` wrapper   | ChatGPT Apps deployment     |
| `McpAppsAdapter` | Generic MCP   | `text/html+mcp`        | postMessage JSON-RPC      | Standard MCP hosts          |
| `McpUiAdapter`   | MCP-UI hosts  | `text/html` (or URL/RemoteDom) | postMessage JSON-RPC | Community MCP-UI hosts      |

**Start with `ChatGptAdapter`.** It's the most mature, and all three shipped examples use it. Switch to `MultiPlatformResource` when you need to support multiple hosts from a single server.

## Hands-On: Multi-Platform Support

Let's serve the same widget to multiple hosts. The `MultiPlatformResource` struct wraps your widget HTML with all three adapters and serves the right version based on the requesting host.

### Construction

```rust
use pmcp::server::mcp_apps::{
    MultiPlatformResource, ChatGptAdapter, McpAppsAdapter, McpUiAdapter,
};

// Add adapters individually
let multi = MultiPlatformResource::new(
    "ui://app/board.html",
    "Chess Board",
    html,
)
.with_adapter(ChatGptAdapter::new())
.with_adapter(McpAppsAdapter::new())
.with_adapter(McpUiAdapter::new());

// Or add all standard adapters at once
let multi = MultiPlatformResource::new(uri, name, html)
    .with_all_adapters();
```

### Getting Host-Specific Content

Use `for_host()` to get the transformed content for a specific host:

```rust
use pmcp::types::mcp_apps::HostType;

if let Some(transformed) = multi.for_host(HostType::ChatGpt) {
    // transformed.content has the bridge-injected HTML
    // transformed.mime_type is text/html+skybridge
    println!("MIME: {}", transformed.mime_type);
}

// Or get all transformations at once
let all = multi.all_transforms();
for t in &all {
    println!("{}: {}", t.mime_type, t.uri);
}
```

`MultiPlatformResource` caches transformations -- calling `for_host()` with the same host type twice reuses the cached result.

### Using MultiPlatformResource in a ResourceHandler

In your `ResourceHandler`, you can detect the requesting host from the request headers or URI parameters, then call `for_host()` with the appropriate `HostType`. For most projects today, using `ChatGptAdapter` directly (as all the shipped examples do) is the simplest path.

**Try this:** Modify the hello server from Ch 20-01 to use `McpAppsAdapter` instead of `ChatGptAdapter`. Change the adapter construction in `AppResources::new()` and update the MIME type in the `ResourceHandler`. Run the preview and notice that the widget still works -- the bridge API (`window.mcpBridge`) is identical. The only difference is the MIME type and the underlying transport mechanism.

## Summary and Next Steps

Let's recap what you've learned:

- **`window.mcpBridge`** provides four core methods (`callTool`, `readResource`, `getPrompt`, `notify`) that work on all hosts
- **Error handling** requires try/catch around every bridge call
- **Bridge initialization** is usually immediate; use `mcpBridgeReady` for async-loading widgets
- **ChatGPT extras** add state management, display modes, and environment properties
- **The communication flow** goes: widget -> bridge script -> host -> MCP server -> response
- **Three adapters** target different hosts: `ChatGptAdapter` (ChatGPT), `McpAppsAdapter` (generic MCP), `McpUiAdapter` (community hosts)
- **`MultiPlatformResource`** serves the same widget to all hosts with host-specific bridge injection

In the next section, you'll walk through the chess, map, and dataviz examples hands-on to see these patterns in real-world applications.

---

*Continue to [Example Walkthroughs](./ch20-03-postmessage.md) ->*
