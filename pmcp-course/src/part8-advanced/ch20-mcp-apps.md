# MCP Apps: Interactive Widgets

```
+-----------------------------------------------------------------------+
|                    EXPERIMENTAL FEATURE                                |
+-----------------------------------------------------------------------+
|                                                                       |
|  MCP Apps and UI Resources are EXPERIMENTAL and subject to change.    |
|                                                                       |
|  What This Means for You:                                             |
|  ========================                                             |
|                                                                       |
|  1. APIs MAY CHANGE                                                   |
|     The types, methods, and patterns shown here may evolve            |
|     significantly as the specification matures.                       |
|                                                                       |
|  2. LIMITED CLIENT SUPPORT                                            |
|     Most MCP clients (Claude Desktop, IDE plugins, etc.) do NOT      |
|     yet support UI resources. Your UIs may not render in all hosts.   |
|                                                                       |
|  3. PMCP SDK WILL EVOLVE                                              |
|     As the MCP specification changes, PMCP SDK and cargo-pmcp        |
|     will update their APIs accordingly.                               |
|                                                                       |
|  Recommendations:                                                     |
|  ================                                                     |
|  - Learn these patterns to understand where MCP is heading            |
|  - Experiment in development environments                             |
|  - Check client compatibility before production deployment            |
|  - Design servers to gracefully degrade when UI is not supported      |
|  - Follow MCP specification updates for changes                       |
|                                                                       |
+-----------------------------------------------------------------------+
```

In this chapter, you'll learn to build rich, interactive widgets -- charts, maps, games, dashboards -- that run alongside your MCP tools. Instead of returning plain JSON that an AI describes in words, your server will serve full HTML interfaces that users can see and interact with directly.

## Learning Objectives

By the end of this chapter, you will be able to:

- Scaffold an MCP Apps project with `cargo pmcp app new` and preview it in your browser
- Author widgets as HTML files using the `WidgetDir` file-based convention
- Communicate between widgets and your server using the `window.mcpBridge` bridge API
- Use adapters to deploy the same widget to ChatGPT, MCP Apps, and MCP-UI hosts
- Follow the chess, map, and dataviz example walkthroughs hands-on

## Why Widgets?

Traditional MCP interactions are text-based: the AI calls a tool, gets JSON back, and describes the results in natural language. That works well for simple data, but some information is inherently visual.

```
+-----------------------------------------------------------------------+
|                    Text vs Visual Information                          |
+-----------------------------------------------------------------------+
|                                                                       |
|  Text works well for:           Visual works better for:              |
|  =====================          =========================             |
|  - Status messages              - Image galleries                     |
|  - Simple data retrieval        - Interactive maps                    |
|  - Configuration values         - Charts and dashboards               |
|  - Error messages               - Forms with validation               |
|  - Lists of items               - Data grids with sorting             |
|                                 - Real-time visualizations            |
|                                 - Game boards                         |
|                                                                       |
+-----------------------------------------------------------------------+
```

When your tool returns coordinates for ten conference venues, an interactive map beats a list of latitude/longitude pairs every time. MCP Apps let you serve that map as an HTML widget right alongside the tool that provides the data.

## Architecture at a Glance

The MCP Apps stack has four layers:

```
Widget (HTML)  --->  Bridge (mcpBridge)  --->  Host (ChatGPT/MCP/MCP-UI)  --->  Server (tools)
```

Here is how each layer works:

| Layer    | What It Is                                  | What It Does                                         |
|----------|---------------------------------------------|------------------------------------------------------|
| Widget   | A plain `.html` file in your `widgets/` dir | Renders the UI the user sees and interacts with      |
| Bridge   | `window.mcpBridge` JavaScript API           | Translates widget calls into the host's native protocol |
| Host     | ChatGPT, MCP Apps runtime, or MCP-UI        | Embeds the widget in a sandboxed iframe              |
| Server   | Your Rust MCP server                        | Processes tool calls and returns results             |

Widgets are plain HTML files. The bridge script handles communication. The host renders the widget. The server processes tool calls. You write the widget and the server -- the bridge and hosting are handled for you.

## Quick Start: Your First Widget

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

### Step 2: Run the Server

```bash
cargo run &
```

The generated `src/main.rs` uses `WidgetDir` to discover widgets and `ChatGptAdapter` to inject the bridge script. You don't need to modify anything yet -- it works out of the box.

### Step 3: Preview in Your Browser

```bash
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser. Type a name in the input field, click the button, and the widget calls the `hello` tool on your MCP server, displaying the greeting it returns.

**Try this:** Edit `widgets/hello.html` -- change a color, update the heading text, anything. Refresh your browser. Your changes appear instantly. No server restart needed. This is the hot-reload development workflow.

## Feature Flag

MCP Apps requires the `mcp-apps` feature flag in your `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
```

The `full` feature also includes `mcp-apps`. If you scaffolded with `cargo pmcp app new`, this is already configured.

## Chapter Contents

This chapter is split into three hands-on sections:

1. **[Widget Authoring and Developer Workflow](./ch20-01-ui-resources.md)** -- Learn the `WidgetDir` file-based convention, scaffold projects with `cargo pmcp app new`, implement the `ResourceHandler` pattern, and use hot-reload development with `cargo pmcp preview` and `cargo pmcp app build`

2. **[Bridge Communication and Adapters](./ch20-02-tool-ui-association.md)** -- Master the `window.mcpBridge` API for calling tools from widgets, understand the communication flow, and use adapters (`ChatGptAdapter`, `McpAppsAdapter`, `McpUiAdapter`) for multi-platform support

3. **[Example Walkthroughs](./ch20-03-postmessage.md)** -- Walk through the chess, map, and dataviz examples hands-on to see these patterns in real-world applications

> **Reference:** For complete API documentation covering every method, type, and edge case, see [Chapter 12.5: MCP Apps Extension](../../pmcp-book/src/ch12-5-mcp-apps.md) in the PMCP book.

## Knowledge Check

{{#quiz ../quizzes/ch20-mcp-apps.toml}}

---

*Continue to [Widget Authoring and Developer Workflow](./ch20-01-ui-resources.md) ->*
