# cargo pmcp app

MCP Apps project management.

## Usage

```
cargo pmcp app <SUBCOMMAND>
```

## Description

Scaffold and manage MCP Apps projects with interactive widgets. Create projects, generate ChatGPT-compatible manifests, and build landing pages.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `new` | Create a new MCP Apps project |
| `manifest` | Generate ChatGPT-compatible manifest JSON |
| `landing` | Generate a standalone demo landing page |
| `build` | Generate both manifest and landing page |

---

## app new

Create a new MCP Apps project with widget scaffolding.

```
cargo pmcp app new <NAME> [OPTIONS]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | Yes | Name of the project |

### Options

| Option | Description |
|--------|-------------|
| `--path <DIR>` | Directory to create project in (defaults to current directory) |

### Example

```bash
cargo pmcp app new my-widget-app
cd my-widget-app
cargo build
cargo run &
cargo pmcp preview --url http://localhost:3000 --open
```

---

## app manifest

Generate a ChatGPT-compatible action manifest.

```
cargo pmcp app manifest --url <URL> [OPTIONS]
```

Detects the MCP Apps project in the current directory, auto-discovers widgets, and writes `manifest.json`.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--url <URL>` | *(required)* | Server URL |
| `--logo <URL>` | - | Logo URL (overrides `[package.metadata.pmcp].logo`) |
| `--output <DIR>` | `dist` | Output directory |

---

## app landing

Generate a standalone demo landing page HTML.

```
cargo pmcp app landing [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--widget <NAME>` | first alphabetically | Widget to showcase |
| `--output <DIR>` | `dist` | Output directory |

---

## app build

Generate both manifest and landing page.

```
cargo pmcp app build --url <URL> [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--url <URL>` | *(required)* | Server URL |
| `--logo <URL>` | - | Logo URL |
| `--widget <NAME>` | first alphabetically | Widget to showcase in landing page |
| `--output <DIR>` | `dist` | Output directory |

### Example

```bash
cargo pmcp app build --url https://my-server.example.com
# Produces dist/manifest.json and dist/landing.html
```

## Related Commands

- [`cargo pmcp preview`](preview.md) - Preview widgets in browser
- [`cargo pmcp landing`](landing.md) - Landing pages for the server itself (not just apps)
