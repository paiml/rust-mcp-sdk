//! Workspace template generator

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// The `pmcp` version REQUIREMENT the emitted workspace `Cargo.toml` declares —
/// a caret major.minor line, not a full version.
///
/// This MUST be a requirement that the PUBLISHED crates.io `pmcp` satisfies, and
/// must never be a filesystem path. Users of `cargo pmcp new` do not have — and
/// must never need — a local SDK checkout.
///
/// This shipped once as `pmcp = { path = "/Users/<maintainer>/..." }` in
/// cargo-pmcp 0.23.1 and broke every scaffolded workspace on every machine
/// except the maintainer's own: `crates/server-common` inherits this pin via
/// `pmcp = { workspace = true }`, so `cargo metadata` — and therefore
/// `cargo pmcp deploy init` — failed with "No such file or directory". It was
/// invisible in-house precisely because the hardcoded path resolves there, so a
/// green local run is not evidence for this class of defect.
///
/// MAJOR.MINOR, not the exact root version, deliberately — mirroring
/// `templates/agent.rs`'s `PMCP_PACKAGE_VERSION_REQ`. The workspace root is
/// routinely a patch ahead of crates.io during a release cycle, and an exact pin
/// on an unpublished patch makes every scaffolded project fail to resolve
/// (measured: `failed to select a version for the requirement pmcp = "^2.19.3"`,
/// exit 101). A caret major.minor line resolves against the currently published
/// crate AND admits the newer patch once it ships.
///
/// `emitted_pmcp_requirement_matches_workspace_major_minor_line` asserts this
/// tracks the workspace-root `pmcp` version so it cannot silently drift.
const PMCP_VERSION_REQ: &str = "2.19";

/// Generate workspace files (Cargo.toml, Makefile, README.md)
pub fn generate(workspace_dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(workspace_dir, name)?;
    generate_makefile(workspace_dir, name)?;
    generate_readme(workspace_dir, name)?;
    generate_gitignore(workspace_dir)?;

    println!("  {} Generated workspace files", "✓".green());
    Ok(())
}

fn generate_cargo_toml(workspace_dir: &Path, _name: &str) -> Result<()> {
    let content = format!(
        r#"[workspace]
resolver = "2"
members = [
    "crates/server-common",
    # Add server crates here via: cargo pmcp add server <name>
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
authors = ["Your Name <you@example.com>"]

[workspace.dependencies]
# MCP SDK
pmcp = {{ version = "{PMCP_VERSION_REQ}", features = ["streamable-http", "schema-generation"] }}

# HTTP transport
axum = "0.7"
tokio = {{ version = "1", features = ["full"] }}
tower-http = {{ version = "0.6", features = ["trace", "cors"] }}

# Serialization
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
schemars = {{ version = "1.0", features = ["preserve_order"] }}

# Logging
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter", "json"] }}
async-trait = "0.1"

# Error handling
anyhow = "1"
thiserror = "1"

# Validation
validator = {{ version = "0.18", features = ["derive"] }}

[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Enable link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip symbols for smaller binaries
"#
    );

    fs::write(workspace_dir.join("Cargo.toml"), &content).context("Failed to create Cargo.toml")?;

    Ok(())
}

fn generate_makefile(workspace_dir: &Path, _name: &str) -> Result<()> {
    let content = r#".PHONY: help build test quality-gate dev deploy clean

help:
	@echo "Available commands:"
	@echo "  make build         - Build all servers"
	@echo "  make test          - Run all tests"
	@echo "  make quality-gate  - Run format, clippy, and tests"
	@echo "  make dev           - Start development server with hot reload"
	@echo "  make deploy        - Deploy to production"
	@echo "  make clean         - Clean build artifacts"

build:
	cargo build --release

test:
	cargo test --all-features

quality-gate:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test --all-features

dev:
	@echo "Starting development server..."
	@echo "Use: cargo pmcp dev --server <name>"

deploy:
	@echo "Deploying to production..."
	@echo "Use: cargo pmcp deploy --server <name> --target lambda"

clean:
	cargo clean
"#;

    fs::write(workspace_dir.join("Makefile"), content).context("Failed to create Makefile")?;

    Ok(())
}

fn generate_readme(workspace_dir: &Path, name: &str) -> Result<()> {
    let content = format!(
        r#"# {}

Production MCP workspace built with [PMCP SDK](https://github.com/paiml/rust-mcp-sdk).

## Quick Start

### Prerequisites

Install mcp-tester for automated testing:
```bash
cargo install mcp-tester
```

### Development

```bash
# Add your first server
cargo-pmcp add server calculator --template minimal

# Generate and run tests
cargo-pmcp test --server calculator --generate-scenarios

# Start development server
cargo run --bin calculator-server

# Run quality checks
make quality-gate
```

## Project Structure

```
{}/
├── crates/
│   ├── server-common/     # Shared HTTP bootstrap (80 LOC)
│   ├── mcp-calculator-core/  # Calculator business logic
│   └── calculator-server/    # Calculator binary (6 LOC)
├── scenarios/             # Test scenarios (YAML)
├── lambda/                # Lambda deployment configs
├── Cargo.toml             # Workspace manifest
└── Makefile               # Build/test/deploy commands
```

## Development Workflow

### 1. Add a new server
```bash
cargo-pmcp add server myserver --template minimal
```

### 2. Generate and run tests
```bash
# Generate test scenarios using mcp-tester (requires: cargo install mcp-tester)
cargo-pmcp test --server myserver --generate-scenarios

# Or just run existing scenarios
cargo-pmcp test --server myserver
```

### 3. Run quality checks
```bash
make quality-gate  # fmt + clippy + tests
```

### 4. Build and run
```bash
# Development
cargo run --bin myserver-server

# Production
cargo build --release --bin myserver-server
```

## Server Pattern

Each server has two crates:

- **mcp-{name}-core** (library): Business logic, tools, resources, workflows
- **{name}-server** (binary): Just 6 lines calling `server_common::run_http()`

This pattern:
- Shares HTTP bootstrap across all servers (DRY)
- Makes binaries trivial (easy to audit)
- Enables unit testing without HTTP complexity
- Scales from 1 to 100 servers

## Configuration

Servers use environment variables:

```bash
RUST_LOG=info              # Logging level
MCP_HTTP_PORT=3000         # Port (or PORT)
MCP_ALLOWED_ORIGINS=*      # CORS origins
```

## Testing

### Automated Testing with mcp-tester

Install mcp-tester:
```bash
cargo install mcp-tester
```

Generate test scenarios automatically:
```bash
cargo-pmcp test --server calculator --generate-scenarios
```

This will:
1. Build your server
2. Start it temporarily
3. Use mcp-tester to discover all tools and capabilities
4. Generate comprehensive test scenarios in `scenarios/calculator/generated.yaml`
5. Run the scenarios and show results

### Manual Testing

Run unit tests:
```bash
cargo test -p mcp-calculator-core
```

Start server and test with curl:
```bash
cargo run --bin calculator-server &
curl -X POST http://0.0.0.0:3000 \\
  -H "Content-Type: application/json" \\
  -H "Accept: application/json" \\
  -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{{}}}}'
```

## Quality Standards

- **Zero tolerance for defects**: All commits pass quality gates
- **80%+ test coverage**: Property tests + unit tests + integration tests
- **Type safety**: schemars JsonSchema with validation
- **Production middleware**: Client tracking, redaction, request IDs

## Resources

- [PMCP SDK Documentation](https://github.com/paiml/rust-mcp-sdk)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Example Servers](https://github.com/paiml/rust-mcp-sdk/tree/main/examples)

## License

MIT OR Apache-2.0
"#,
        name, name
    );

    fs::write(workspace_dir.join("README.md"), content).context("Failed to create README.md")?;

    Ok(())
}

fn generate_gitignore(workspace_dir: &Path) -> Result<()> {
    let content = r#"# Rust
target/
Cargo.lock

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Environment
.env
.env.local

# Logs
*.log

# CDK
lambda/cdk.out/
lambda/.cdk.staging/
lambda/node_modules/
"#;

    fs::write(workspace_dir.join(".gitignore"), content).context("Failed to create .gitignore")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace-root `Cargo.toml`, embedded so the drift guard can compare
    /// the scaffold's pin against the real `pmcp` version.
    const ROOT_CARGO_TOML: &str = include_str!("../../../Cargo.toml");

    /// Render the emitted workspace `Cargo.toml` into a tempdir and read it back,
    /// exercising the real `generate_cargo_toml` rather than a copy of its string.
    fn render_workspace_cargo_toml() -> String {
        let tmp = tempfile::tempdir().expect("create tempdir");
        generate_cargo_toml(tmp.path(), "testws").expect("generate workspace Cargo.toml");
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read emitted Cargo.toml")
    }

    /// DIRECT REGRESSION GUARD for the `deploy-init-abs-path-dep` defect.
    ///
    /// `cargo pmcp new` once emitted a `pmcp = { path = "..." }` dependency
    /// carrying an ABSOLUTE path to the maintainer's own SDK checkout into every
    /// scaffolded workspace. `crates/server-common` inherits that pin via
    /// `pmcp = { workspace = true }`, so `cargo metadata` — and therefore
    /// `cargo pmcp deploy init` — failed on every machine except the
    /// maintainer's. The bug was invisible in-house precisely because the
    /// hardcoded path resolves fine there.
    ///
    /// Oracle: `derived` (contract) — asserts the structural property "no
    /// absolute filesystem path may appear in generated output", not one
    /// literal. Boundary neighbors cover the macOS, Linux and Windows shapes so
    /// re-introducing the defect under a different home directory still fails.
    #[test]
    fn emitted_cargo_toml_contains_no_absolute_path() {
        let content = render_workspace_cargo_toml();

        for needle in [
            "/Users/",
            "/home/",
            "/root/",
            "C:\\",
            "/private/var",
            "/tmp/",
        ] {
            assert!(
                !content.contains(needle),
                "emitted workspace Cargo.toml leaked an absolute filesystem path \
                 containing `{needle}` — scaffolded projects must never reference a \
                 local SDK checkout. Emitted:\n{content}"
            );
        }
    }

    /// The `pmcp` dependency must be pinned BY VERSION and must carry no `path`
    /// key at all — the shape that caused the original defect.
    ///
    /// Oracle: `derived` — parses the emitted TOML and asserts on the dependency
    /// table's structure rather than string-matching, so a reformatting of the
    /// template cannot make this pass vacuously.
    #[test]
    fn emitted_pmcp_dep_is_pinned_by_version_not_path() {
        let content = render_workspace_cargo_toml();
        let parsed: toml::Value = toml::from_str(&content).expect("emitted Cargo.toml must parse");

        let pmcp = parsed
            .get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|d| d.get("pmcp"))
            .expect("emitted [workspace.dependencies] must declare pmcp");

        assert!(
            pmcp.get("path").is_none(),
            "emitted pmcp dependency carries a `path` key — scaffolded workspaces \
             must resolve pmcp from crates.io, never from a local checkout. Got: {pmcp:?}"
        );
        assert_eq!(
            pmcp.get("version").and_then(toml::Value::as_str),
            Some(PMCP_VERSION_REQ),
            "emitted pmcp dependency must declare PMCP_VERSION_REQ. Got: {pmcp:?}"
        );
    }

    /// `PMCP_VERSION_REQ` must track the workspace-root `pmcp` MAJOR.MINOR line.
    ///
    /// This is a version emitter `cargo build` cannot see: the workspace resolves
    /// green while every project scaffolded by `cargo pmcp new` requests whatever
    /// this constant says. Without this guard a stale requirement ships silently.
    ///
    /// Compared at major.minor (not the full version) ON PURPOSE — see
    /// `PMCP_VERSION_REQ`'s docs. An exact-version guard here would force the
    /// scaffold to pin an unpublished patch during every release cycle.
    #[test]
    fn emitted_pmcp_requirement_matches_workspace_major_minor_line() {
        let parsed: toml::Value =
            toml::from_str(ROOT_CARGO_TOML).expect("parse workspace root Cargo.toml");
        let root_version = parsed
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .expect("root Cargo.toml has [package] version");
        let mut parts = root_version.split('.');
        let major = parts.next().unwrap_or("0");
        let minor = parts.next().unwrap_or("0");
        let expected = format!("{major}.{minor}");
        assert_eq!(
            PMCP_VERSION_REQ, expected,
            "the scaffold's pmcp requirement `{PMCP_VERSION_REQ}` drifted from the \
             workspace-root version `{root_version}` — bump PMCP_VERSION_REQ in \
             templates/workspace.rs"
        );
    }

    /// The generated `server-common` member inherits the root pin via
    /// `workspace = true`. This is the hop that turned one bad literal into a
    /// broken workspace, so assert the member list and the emitted workspace
    /// table stay consistent.
    #[test]
    fn emitted_workspace_declares_the_server_common_member() {
        let content = render_workspace_cargo_toml();
        let parsed: toml::Value = toml::from_str(&content).expect("emitted Cargo.toml must parse");
        let members = parsed
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(toml::Value::as_array)
            .expect("emitted [workspace] must declare members");
        assert!(
            members
                .iter()
                .any(|m| m.as_str() == Some("crates/server-common")),
            "emitted workspace must list crates/server-common — it is the member that \
             inherits the pmcp pin. Got: {members:?}"
        );
    }
}
