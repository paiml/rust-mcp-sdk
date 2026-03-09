# cargo pmcp preview

Preview MCP Apps widgets in a browser.

## Usage

```
cargo pmcp preview --url <URL> [OPTIONS]
```

## Description

Launches a browser-based preview environment for testing MCP servers that return widget UI. Simulates the ChatGPT Apps runtime with dual proxy/WASM bridge modes.

When `--widgets-dir` is set, widget HTML files are read directly from disk on each request, enabling hot-reload during development (just refresh the browser).

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--url <URL>` | *(required)* | URL of the running MCP server |
| `--port <PORT>` | `8765` | Port for the preview server |
| `--open` | - | Open browser automatically |
| `--tool <TOOL>` | - | Auto-select this tool on start |
| `--theme <THEME>` | `light` | Initial theme (`light` or `dark`) |
| `--locale <LOCALE>` | `en-US` | Initial locale |
| `--widgets-dir <DIR>` | - | Path to widgets directory for file-based authoring (hot-reload) |

## Examples

**Basic preview with auto-open:**
```bash
cargo pmcp preview --url http://localhost:3000 --open
```

**File-based widget authoring with hot-reload:**
```bash
cargo pmcp preview --url http://localhost:3000 --widgets-dir ./widgets --open
```

**Dark theme, auto-select a specific tool:**
```bash
cargo pmcp preview --url http://localhost:3000 --theme dark --tool chess_board
```

## Related Commands

- [`cargo pmcp app`](app.md) - Scaffold and manage MCP Apps projects
- [`cargo pmcp dev`](dev.md) - Start the MCP server before previewing
