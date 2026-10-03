use super::binary::{self, BuildTarget};
use crate::deployment::{DeployConfig, LayoutConfig};
use anyhow::{bail, Context, Result};

/// Generate the Cloud Run Dockerfile for `config.project_root`.
///
/// Layout selection precedence:
/// 1. `[layout].kind = "multi-crate-isolated"` (issue #258) — surgical
///    per-crate `COPY` lines + `cargo build --manifest-path`.
/// 2. Root `Cargo.toml` has a `[workspace]` table — workspace template
///    (`COPY . .`, then `cargo build -p <package> --bin <binary>`).
/// 3. Otherwise — simple binary crate template (`--bin <binary>`).
///
/// For 2 and 3 the binary is resolved by [`resolve_build_target`], and the
/// builder stage copies exactly that binary by name.
///
/// Issue #259's distroless default applies to all three layouts; see
/// `runtime_stage`.
///
/// # Errors
///
/// Returns an error if the project `Cargo.toml` cannot be read or parsed
/// (non multi-crate-isolated layouts), no binary can be chosen, or the
/// rendered Dockerfile cannot be written to disk.
// Why: lib API (the `cloud_run_local_build` integration test drives it through
// lib.rs). The bin's `deploy init` writes through the `render_*` functions so
// it can keep an existing file, which leaves this unused in the bin target.
#[allow(dead_code)]
pub fn generate_dockerfile(config: &DeployConfig) -> Result<()> {
    let dockerfile_content = render_dockerfile(config)?;
    let dockerfile_path = config.project_root.join("Dockerfile");
    std::fs::write(&dockerfile_path, dockerfile_content).context("Failed to write Dockerfile")?;
    println!("   ✓ Generated Dockerfile");
    Ok(())
}

/// The multi-crate isolated layout, when `[layout]` selects it.
fn multi_crate_layout(config: &DeployConfig) -> Option<&LayoutConfig> {
    config
        .layout
        .as_ref()
        .filter(|l| l.is_multi_crate_isolated())
}

/// The binary the workspace and simple-crate templates build.
///
/// `[server] binary` when set, else the project's single non-Lambda binary
/// (debug session `cargo-pmcp-deploy-targets`, findings #5/#6; see
/// [`binary::resolve_build_target`]).
///
/// `None` for the multi-crate isolated layout, which keeps its own rule
/// (`[server] binary`, else `[server] name`).
///
/// # Errors
///
/// Returns an error naming the project's binaries when none, or more than
/// one, could be the server, or when `[server] binary` is not one of them.
pub fn resolve_build_target(config: &DeployConfig) -> Result<Option<BuildTarget>> {
    if multi_crate_layout(config).is_some() {
        return Ok(None);
    }
    binary::resolve_build_target(&config.project_root, config.server.binary.as_deref()).map(Some)
}

/// Render the Dockerfile contents without writing to disk.
///
/// # Errors
///
/// As [`resolve_build_target`] and [`render_dockerfile_for`].
pub fn render_dockerfile(config: &DeployConfig) -> Result<String> {
    let target = resolve_build_target(config)?;
    render_dockerfile_for(config, target.as_ref())
}

/// Render the Dockerfile for an already-resolved build target (`deploy init`
/// resolves it once, to record it in a new deploy.toml as well).
///
/// # Errors
///
/// Returns an error if the project `Cargo.toml` cannot be read or parsed, or
/// when `target` is `None` (or holds a name that is not safe in a `RUN`
/// line) for a layout other than multi-crate isolated.
pub fn render_dockerfile_for(
    config: &DeployConfig,
    target: Option<&BuildTarget>,
) -> Result<String> {
    let builder = match (multi_crate_layout(config), target) {
        (Some(layout), _) => {
            builder_stage_multi_crate_isolated(layout, &resolve_binary_name(config))
        },
        (None, Some(target)) if is_workspace_root(&config.project_root)? => {
            builder_stage_workspace(target)?
        },
        (None, Some(target)) => builder_stage_simple(target)?,
        (None, None) => bail!(
            "no binary was chosen for the Dockerfile; set `binary = \"<name>\"` under [server] \
             in .pmcp/deploy.toml"
        ),
    };

    Ok(format!(
        "{builder}\n{runtime}",
        runtime = runtime_stage(config)
    ))
}

/// True when the project's root `Cargo.toml` has a `[workspace]` table. A
/// parsed check, not a substring one: a commented-out `# [workspace]` does not
/// count.
fn is_workspace_root(project_root: &std::path::Path) -> Result<bool> {
    let path = project_root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path).context("Failed to read Cargo.toml")?;
    let manifest: toml::Table =
        toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(manifest.contains_key("workspace"))
}

/// Resolve the binary name for `cargo build --bin <name>` and the runtime
/// `COPY --from=builder` line. Falls back to `config.server.name` when
/// `[server].binary` is unset.
fn resolve_binary_name(config: &DeployConfig) -> String {
    config
        .server
        .binary
        .clone()
        .unwrap_or_else(|| config.server.name.clone())
}

/// Shared apt-install layer for the rust:slim builder stage. `pkg-config`
/// and `libssl-dev` cover the common native-build deps; everything else is
/// expected to come from crates.io.
const BUILDER_APT_LAYER: &str = "# Install build dependencies
RUN apt-get update && apt-get install -y \\
    pkg-config \\
    libssl-dev \\
    && rm -rf /var/lib/apt/lists/*";

/// Shared build-and-copy step for the workspace and simple-crate layouts:
/// build exactly the chosen binary (`-p <package>` only when `with_package`
/// and the package is known) and copy it BY NAME to the path the runtime
/// stage expects. (The step used to build every non-`lambda` package and copy
/// whichever executable `find target/release` met last.)
fn build_and_copy(target: &BuildTarget, with_package: bool) -> Result<String> {
    let names = std::iter::once(target.binary.as_str()).chain(target.package.as_deref());
    for name in names {
        if !binary::is_safe_name(name) {
            bail!(
                "`{name}` cannot be used in the generated Dockerfile: binary and package names \
                 must be ASCII letters, digits, `-` or `_`."
            );
        }
    }
    let package = match &target.package {
        Some(package) if with_package => format!(" -p {package}"),
        _ => String::new(),
    };
    Ok(format!(
        "# Build the server binary ([server] binary, resolved at `cargo pmcp deploy init`)
RUN cargo build --release{package} --bin {bin}

# Copy it by name to the path the runtime stage expects
RUN cp target/release/{bin} /app/mcp-server",
        bin = target.binary
    ))
}

fn builder_stage_workspace(target: &BuildTarget) -> Result<String> {
    let build = build_and_copy(target, true)?;
    Ok(format!(
        r"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Workspace project structure - builds inside Docker to handle path dependencies

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

{BUILDER_APT_LAYER}

# Create app directory
WORKDIR /app

# Copy the entire project (including path dependencies)
COPY . .

{build}
",
    ))
}

fn builder_stage_simple(target: &BuildTarget) -> Result<String> {
    let build = build_and_copy(target, false)?;
    Ok(format!(
        r"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Simple binary crate - builds inside Docker

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

{BUILDER_APT_LAYER}

# Create app directory
WORKDIR /app

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

{build}
",
    ))
}

/// Surgical builder for the multi-crate isolated layout (issue #258).
///
/// Emits one `COPY <crate>/Cargo.toml <crate>/Cargo.toml` + one
/// `COPY <crate>/src <crate>/src` pair for each entry in `primary` +
/// `path_deps`, then a single `cargo build --release --manifest-path
/// <primary>/Cargo.toml --bin <binary>` step. The release artifact is
/// then copied to `/app/mcp-server` so the runtime stage can rely on a
/// fixed path.
fn builder_stage_multi_crate_isolated(layout: &LayoutConfig, binary: &str) -> String {
    // `path_deps` entries are scaffolded into Dockerfile COPY lines. We
    // restrict to the Cargo crate-name character set (alphanumeric, `-`,
    // `_`) — `.` is intentionally excluded to block `..` path escapes,
    // and any other character that would inject Dockerfile syntax or
    // shell semantics (semicolons, quotes, newlines, whitespace, `/`)
    // is stripped.
    let sanitize = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    };

    let primary = sanitize(&layout.primary);
    let safe_binary = sanitize(binary);

    let mut copies = String::new();
    copies.push_str(&format!("COPY {primary}/Cargo.toml {primary}/Cargo.toml\n"));
    copies.push_str(&format!("COPY {primary}/src {primary}/src\n"));
    for dep in &layout.path_deps {
        let safe_dep = sanitize(dep);
        if safe_dep.is_empty() {
            continue;
        }
        copies.push_str(&format!(
            "COPY {safe_dep}/Cargo.toml {safe_dep}/Cargo.toml\n"
        ));
        copies.push_str(&format!("COPY {safe_dep}/src {safe_dep}/src\n"));
    }

    format!(
        r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Multi-crate isolated layout — sibling crates with path-dep relationships
# (issue 258). Only the primary crate and its declared path_deps are
# copied into the build context. Unrelated siblings (e.g. aws-lambda)
# are excluded to keep the image small and avoid cross-toolchain build
# failures.

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Surgical per-crate COPY lines (primary + declared path_deps)
{copies}
# Build the binary from the primary crate's manifest
RUN cargo build --release \
    --manifest-path {primary}/Cargo.toml \
    --bin {safe_binary}

# Cargo with `--manifest-path` places the release artifact under the
# primary crate's own target/release dir. Some setups put it at the
# top-level target/release dir; try both so the runtime stage gets a
# stable path.
RUN cp {primary}/target/release/{safe_binary} /app/mcp-server 2>/dev/null \
    || cp target/release/{safe_binary} /app/mcp-server
"#,
    )
}

/// Runtime stage of the Dockerfile.
///
/// Emits the runtime stage with the appropriate shape for the runtime
/// base. Closes upstream paiml/rust-mcp-sdk#259:
///
/// - Default base is `gcr.io/distroless/cc-debian12` — no shell, no apt,
///   ~20 MB vs `debian:bookworm-slim`'s ~80 MB. Cuts cold-start image-pull
///   time and the post-exploitation attack surface. Distroless runs as
///   `nonroot` (uid 65532) by default, so no `useradd` is needed.
/// - `[runtime].base = "..."` overrides verbatim.
/// - `[runtime].apt_packages = [...]` is honored only when `base` resolves
///   to a debian-family image. Empty list (the default) → no apt layer.
fn runtime_stage(config: &DeployConfig) -> String {
    let base = config
        .runtime
        .as_ref()
        .and_then(|r| r.base.as_deref())
        .unwrap_or("gcr.io/distroless/cc-debian12");

    if is_debian_family(base) {
        runtime_stage_debian(config, base)
    } else {
        runtime_stage_distroless(base)
    }
}

fn is_debian_family(base: &str) -> bool {
    base.starts_with("debian:") || base.starts_with("ubuntu:")
}

/// Distroless / non-shell runtime stage.
///
/// Contains no shell, no package manager, no `useradd`, no
/// `HEALTHCHECK CMD curl` (no curl in the image). Cloud Run health-checks
/// externally on `PORT`, so the in-image HEALTHCHECK is unnecessary.
fn runtime_stage_distroless(base: &str) -> String {
    format!(
        r#"# Stage 2: distroless runtime — minimal attack surface (issue #259)
# No shell, no apt, no package manager. Cloud Run health-checks externally
# on $PORT, so no in-image HEALTHCHECK directive is needed.
FROM {base}

# Copy the prebuilt binary from the builder stage. Distroless cc runs as
# `nonroot` (uid 65532) by default — no useradd / USER directive needed.
COPY --from=builder /app/mcp-server /usr/local/bin/mcp-server

# Cloud Run sets PORT (defaults to 8080); the server is expected to bind
# 0.0.0.0:$PORT.
ENV PORT=8080
EXPOSE $PORT

CMD ["/usr/local/bin/mcp-server"]
"#,
    )
}

/// Debian/Ubuntu runtime stage — emitted only when the operator opts back
/// to a shell-enabled base via `[runtime].base = "debian:..."` (or
/// `"ubuntu:..."`).
///
/// `[runtime].apt_packages` drives the apt-install layer; an empty list
/// (the default) emits no apt layer at all, leaving the base image
/// untouched.
fn runtime_stage_debian(config: &DeployConfig, base: &str) -> String {
    let pkgs: Vec<&str> = config
        .runtime
        .as_ref()
        .map(|r| {
            r.apt_packages
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let apt_layer = if pkgs.is_empty() {
        String::new()
    } else {
        format!(
            "# Install runtime apt packages declared in [runtime].apt_packages\nRUN apt-get update && apt-get install -y \\\n    {} \\\n    && rm -rf /var/lib/apt/lists/*\n\n",
            pkgs.join(" \\\n    ")
        )
    };

    format!(
        r#"# Stage 2: debian-family runtime — opted in via [runtime].base
FROM {base}

{apt_layer}# Create non-root user
RUN useradd -m -u 1000 mcpserver

# Copy binary from builder
COPY --from=builder /app/mcp-server /usr/local/bin/mcp-server

# Ensure binary is executable
RUN chmod +x /usr/local/bin/mcp-server

# Change to non-root user
USER mcpserver

# Set up working directory
WORKDIR /home/mcpserver

# Cloud Run will set the PORT environment variable
# Default to 8080 if not set
ENV PORT=8080

# Expose the port
EXPOSE $PORT

# Run the MCP server
# Cloud Run expects the server to bind to 0.0.0.0:$PORT
CMD ["/usr/local/bin/mcp-server"]
"#,
    )
}

/// Generate .dockerignore for optimal build context
///
/// # Errors
///
/// Returns an error if the `.dockerignore` file cannot be written.
// Why: lib API, see `generate_dockerfile`.
#[allow(dead_code)]
pub fn generate_dockerignore(config: &DeployConfig) -> Result<()> {
    let dockerignore_content = render_dockerignore(config);
    let dockerignore_path = config.project_root.join(".dockerignore");
    std::fs::write(&dockerignore_path, dockerignore_content)
        .context("Failed to write .dockerignore")?;

    println!("   ✓ Generated .dockerignore");

    Ok(())
}

/// Render `.dockerignore` for `config` without writing it.
///
/// The builder stage compiles from source, so the host's build output is
/// never needed: `target/` is ignored wholesale (debug session
/// `cargo-pmcp-deploy-targets`, finding #11 — only some `target/release`
/// subdirectories used to be ignored, and the field report's build context
/// carried 899 MB of host binaries). For the multi-crate isolated layout each
/// crate's own `target/` is ignored too.
///
/// Paths `cargo build` itself reads are NOT ignored, because the workspace
/// template copies the whole context: `tests/` and `benches/` (a declared
/// `[[test]]`/`[[bench]]` whose file is missing fails manifest parsing),
/// `README.md` (`#![doc = include_str!("../README.md")]`) and `vendor/` (a
/// `.cargo/config.toml` source replacement).
#[must_use]
pub fn render_dockerignore(config: &DeployConfig) -> String {
    let mut crate_targets = String::new();
    if let Some(layout) = multi_crate_layout(config) {
        let crates = std::iter::once(&layout.primary).chain(&layout.path_deps);
        for name in crates.filter(|name| binary::is_safe_name(name)) {
            crate_targets.push_str(&format!("{name}/target/\n"));
        }
    }
    format!(
        r"# Generated by `cargo pmcp deploy init --target-type google-cloud-run`.
# Init keeps an existing file; delete it and re-run init to regenerate it.

# Build output: the Dockerfile compiles from source, so nothing under the
# host's target/ is needed (it can be gigabytes).
target/
{crate_targets}**/*.rs.bk
*.pdb

# Node.js dependencies (e.g. widget builds); cargo never reads them
**/node_modules/

# IDE files
.vscode/
.idea/
*.swp
*.swo
*~

# Git
.git/
.gitignore

# Documentation
docs/

# CI/CD
.github/
.gitlab-ci.yml

# Deploy artifacts (CDK, Lambda, etc.)
deploy/
cdk.out/
.pmcp/
bootstrap

# OS files
.DS_Store
Thumbs.db

# Logs
*.log

# Environment files
.env
.env.local
"
    )
}

/// Generate `cloudbuild.yaml` from `DeployConfig`.
///
/// Drives `gcloud run deploy` flags from the persisted schema (`[gcp]`,
/// `[server]`, `[environment]`) rather than hard-coded literals — closes
/// the env-var-drift gap from upstream issue #260. Memory / CPU / instance
/// counts / ingress / `allow-unauthenticated` are all sourced from
/// `[server]`; the `[environment]` table becomes the `--set-env-vars`
/// argument.
///
/// # Errors
///
/// Returns an error if the `cloudbuild.yaml` file cannot be written.
// Why: lib API, see `generate_dockerfile`.
#[allow(dead_code)]
pub fn generate_cloudbuild(config: &DeployConfig) -> Result<()> {
    let cloudbuild_content = render_cloudbuild(config);
    let cloudbuild_path = config.project_root.join("cloudbuild.yaml");
    std::fs::write(&cloudbuild_path, cloudbuild_content)
        .context("Failed to write cloudbuild.yaml")?;

    println!("   ✓ Generated cloudbuild.yaml");

    Ok(())
}

/// Render `cloudbuild.yaml` from `config` without writing it.
#[must_use]
pub fn render_cloudbuild(config: &DeployConfig) -> String {
    let region = config
        .gcp
        .as_ref()
        .map(|g| g.region.as_str())
        .filter(|r| !r.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string())
        });

    let memory = config
        .server
        .memory
        .clone()
        .unwrap_or_else(|| "512Mi".to_string());
    let cpu = config.server.cpu.clone().unwrap_or_else(|| "1".to_string());
    let max_instances = config.server.max_instances.unwrap_or(10);
    let min_instances = config.server.min_instances.unwrap_or(0);
    let allow_unauth = config.server.allow_unauthenticated.unwrap_or(true);

    let mut steps_tail = String::new();
    if let Some(ingress) = &config.server.ingress {
        steps_tail.push_str(&format!("      - '--ingress'\n      - '{ingress}'\n"));
    }
    let env_vars = super::env::render_set_env_vars(&config.environment);
    if !env_vars.is_empty() {
        steps_tail.push_str(&format!("      - '--set-env-vars'\n      - '{env_vars}'\n"));
    }
    let auth_flag = if allow_unauth {
        "--allow-unauthenticated"
    } else {
        "--no-allow-unauthenticated"
    };

    format!(
        r#"# Cloud Build configuration for automated deployments
# Build and deploy Rust MCP server to Cloud Run
#
# Usage: gcloud builds submit --config cloudbuild.yaml
#
# Or set up a trigger in Cloud Build to auto-deploy on git push.
#
# Generated by `cargo pmcp deploy init --target-type google-cloud-run` from
# .pmcp/deploy.toml ([gcp].region, [server].*, [environment].*). Init keeps
# an existing file: after changing deploy.toml, delete this file and re-run
# init to regenerate it.

steps:
  # Build the Docker image
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'build'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{name}:latest'
      - '.'

  # Push the Docker image to Google Container Registry
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'push'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'

  # Deploy to Cloud Run
  - name: 'gcr.io/google.com/cloudsdktool/cloud-sdk'
    entrypoint: gcloud
    args:
      - 'run'
      - 'deploy'
      - '{name}'
      - '--image'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
      - '--region'
      - '{region}'
      - '--platform'
      - 'managed'
      - '{auth_flag}'
      - '--memory'
      - '{memory}'
      - '--cpu'
      - '{cpu}'
      - '--max-instances'
      - '{max_instances}'
      - '--min-instances'
      - '{min_instances}'
      - '--port'
      - '8080'
{tail}
# Store images in GCR
images:
  - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
  - 'gcr.io/$PROJECT_ID/{name}:latest'

# Build timeout
timeout: '1200s'

# Build options
options:
  machineType: 'E2_HIGHCPU_8'
  logging: CLOUD_LOGGING_ONLY
"#,
        name = config.server.name,
        tail = steps_tail,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(tmp: &TempDir) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "auth-echo-cloud-run".to_string(),
            "your-gcp-project-id".to_string(),
            "us-central1".to_string(),
            tmp.path().to_path_buf(),
        )
    }

    /// A simple binary crate (`src/main.rs`, so its one binary is
    /// `auth-echo-cloud-run`) that `cargo metadata` can load.
    fn write_cargo_toml(tmp: &TempDir) {
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"auth-echo-cloud-run\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write cargo.toml");
        std::fs::create_dir_all(tmp.path().join("src")).expect("mkdir src");
        std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").expect("main.rs");
    }

    /// cloudbuild.yaml splices [server].memory / cpu / max_instances /
    /// min_instances from deploy.toml — closes upstream #260's env-var
    /// drift problem.
    #[test]
    fn cloudbuild_yaml_drives_server_fields_from_config() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.memory = Some("1Gi".to_string());
        config.server.cpu = Some("2".to_string());
        config.server.max_instances = Some(50);
        config.server.min_instances = Some(2);

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("- '1Gi'"), "memory must come from config: {cb}");
        assert!(cb.contains("- '2'"), "cpu must come from config");
        assert!(cb.contains("- '50'"), "max_instances must come from config");
        assert!(cb.contains("- '2'"), "min_instances must come from config");
    }

    /// [environment] entries become a deterministic `--set-env-vars` arg.
    /// Closes upstream #260: post-deploy
    /// `gcloud run services update --set-env-vars` patches are no longer
    /// required for app-level env vars.
    #[test]
    fn cloudbuild_yaml_includes_environment_set_env_vars() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config
            .environment
            .insert("EXPECTED_AUDIENCE".to_string(), "abc.apps.x".to_string());
        // RUST_LOG=info is already in environment from the default ctor.

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("'--set-env-vars'"));
        assert!(
            cb.contains("EXPECTED_AUDIENCE=abc.apps.x,RUST_LOG=info")
                || cb.contains("RUST_LOG=info,EXPECTED_AUDIENCE=abc.apps.x")
                || cb.contains("EXPECTED_AUDIENCE=abc.apps.x"),
            "deterministic env-vars rendering: {cb}"
        );
    }

    /// `[server].allow_unauthenticated = false` flips the gcloud auth flag.
    #[test]
    fn cloudbuild_yaml_honors_allow_unauthenticated_false() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.allow_unauthenticated = Some(false);

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("--no-allow-unauthenticated"));
        assert!(!cb.contains("'--allow-unauthenticated'\n"));
    }

    /// `[server].ingress = "internal"` produces an `--ingress` arg pair.
    #[test]
    fn cloudbuild_yaml_honors_ingress() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.ingress = Some("internal".to_string());

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("'--ingress'"));
        assert!(cb.contains("'internal'"));
    }

    // ---------- Layout / Dockerfile tests (issue #258) ----------

    use crate::deployment::config::LayoutConfig;

    /// Default simple-crate layout: no [workspace] in Cargo.toml, no
    /// [layout] block. Dockerfile uses the simple template.
    #[test]
    fn dockerfile_simple_layout_emits_simple_template() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("Simple binary crate"));
        assert!(dockerfile.contains("COPY Cargo.toml Cargo.lock ./"));
        assert!(!dockerfile.contains("multi-crate-isolated"));
    }

    /// Workspace layout detected via `[workspace]` table at the project
    /// root. Dockerfile uses the workspace template.
    #[test]
    fn dockerfile_workspace_layout_emits_workspace_template() {
        let tmp = TempDir::new().expect("tmpdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]\n",
        )
        .expect("write cargo");
        super::super::fixture::write_crate(&tmp.path().join("crate-a"), "crate-a", &["a"], "");
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("Workspace project structure"));
        assert!(dockerfile.contains("COPY . ."));
    }

    // ---------- Binary selection (debug session cargo-pmcp-deploy-targets, #5/#6) ----------

    use super::super::fixture;

    fn directive_lines(dockerfile: &str) -> Vec<&str> {
        dockerfile
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect()
    }

    /// The field report's acceptance shape: package `x-lambda`, bins
    /// `bootstrap` + `serve`, a `lambda_http` dependency, a `[workspace]`
    /// table. The workspace template used to `--exclude` every name a grep
    /// matched (`x-lambda` itself and `lambda_http`) -> "no packages to
    /// compile", then copy whichever executable `find` met last.
    #[test]
    fn x_lambda_workspace_builds_and_copies_serve_by_name() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        let directives = directive_lines(&dockerfile).join("\n");
        assert!(
            directives.contains("RUN cargo build --release -p x-lambda --bin serve\n"),
            "{dockerfile}"
        );
        assert!(
            directives.contains("RUN cp target/release/serve /app/mcp-server\n"),
            "{dockerfile}"
        );
        for gone in [
            "--exclude",
            "cargo metadata",
            "grep",
            "find target/release",
            "--workspace",
        ] {
            assert!(
                !directives.contains(gone),
                "`{gone}` must not be in: {dockerfile}"
            );
        }
    }

    /// `[server] binary` is honoured by the simple-crate template too.
    #[test]
    fn a_declared_binary_is_built_by_name_in_the_simple_template() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_crate(tmp.path(), "svc", &["api", "worker"], "");
        let mut config = make_config(&tmp);
        config.server.binary = Some("api".to_string());

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("Simple binary crate"), "{dockerfile}");
        let directives = directive_lines(&dockerfile).join("\n");
        assert!(
            directives.contains("RUN cargo build --release --bin api\n"),
            "{dockerfile}"
        );
        assert!(directives.contains("RUN cp target/release/api /app/mcp-server\n"));
        assert!(!directives.contains("worker"), "{dockerfile}");
    }

    /// forecast-coach's real shape (`bootstrap`, `local`, `serve`): no
    /// silent pick; the error names both candidates and the fix.
    #[test]
    fn several_candidate_binaries_fail_naming_them() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda_with_local(tmp.path());
        let config = make_config(&tmp);

        let err = render_dockerfile(&config).expect_err("ambiguous");
        let message = format!("{err:#}");
        assert!(
            message.contains("`local` (package `x-lambda`)"),
            "{message}"
        );
        assert!(
            message.contains("`serve` (package `x-lambda`)"),
            "{message}"
        );
        assert!(message.contains("binary = \"<name>\""), "{message}");
    }

    #[test]
    fn a_declared_binary_that_does_not_exist_fails() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());
        let mut config = make_config(&tmp);
        config.server.binary = Some("x-lambda".to_string());

        let message = format!("{:#}", render_dockerfile(&config).expect_err("not a bin"));
        assert!(
            message.contains("is not a binary of this project"),
            "{message}"
        );
    }

    /// `render_dockerfile_for` is public: a build target handed to it directly
    /// is checked again before any name reaches a `RUN` line.
    #[test]
    fn an_unsafe_build_target_is_refused_before_rendering() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());
        let config = make_config(&tmp);
        for (binary, package) in [("serve; rm -rf /", None), ("serve", Some("x $(id)"))] {
            let target = BuildTarget {
                binary: binary.to_string(),
                package: package.map(str::to_string),
            };
            let err = render_dockerfile_for(&config, Some(&target)).expect_err("unsafe");
            assert!(err.to_string().contains("cannot be used"), "{err}");
        }
        assert!(
            render_dockerfile_for(&config, None).is_err(),
            "no target, no Dockerfile"
        );
    }

    /// `[workspace]` is a table, not a substring: a commented-out header does
    /// not switch to the workspace template.
    #[test]
    fn a_commented_out_workspace_header_is_not_a_workspace() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_crate(tmp.path(), "svc", &["serve"], "# [workspace]\n");
        let dockerfile = render_dockerfile(&make_config(&tmp)).expect("render");
        assert!(dockerfile.contains("Simple binary crate"), "{dockerfile}");
    }

    // ---------- .dockerignore (debug session cargo-pmcp-deploy-targets, #11) ----------

    fn ignore_lines(text: &str) -> Vec<&str> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    }

    /// The builder stage compiles from source, so nothing under the host's
    /// `target/` belongs in the build context (899 MB in the field report).
    #[test]
    fn dockerignore_excludes_target_wholesale() {
        let tmp = TempDir::new().expect("tmpdir");
        let text = render_dockerignore(&make_config(&tmp));
        let lines = ignore_lines(&text);
        assert!(lines.contains(&"target/"), "{lines:?}");
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("target/") && *l != "target/"),
            "partial target/ entries are redundant: {lines:?}"
        );
        for kept in [".git/", "**/node_modules/", ".env", "deploy/", ".pmcp/"] {
            assert!(lines.contains(&kept), "{kept} must be ignored: {lines:?}");
        }
    }

    /// Paths `cargo build` reads must stay in the context: a declared
    /// `[[test]]`/`[[bench]]` whose file is missing fails manifest parsing,
    /// `#![doc = include_str!("../README.md")]` needs the README, and a
    /// `.cargo/config.toml` source replacement needs `vendor/`.
    #[test]
    fn dockerignore_keeps_what_cargo_build_reads() {
        let tmp = TempDir::new().expect("tmpdir");
        let text = render_dockerignore(&make_config(&tmp));
        let lines = ignore_lines(&text);
        for needed in ["tests/", "benches/", "README.md", "vendor/"] {
            assert!(
                !lines.contains(&needed),
                "{needed} must not be ignored: {lines:?}"
            );
        }
    }

    /// Multi-crate isolated layout: each crate keeps its own `target/`.
    #[test]
    fn dockerignore_excludes_each_isolated_crate_target() {
        let tmp = TempDir::new().expect("tmpdir");
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp-cloud-run".to_string(),
            path_deps: vec!["auth-echo-core".to_string(), "../escape".to_string()],
        });
        let text = render_dockerignore(&config);
        let lines = ignore_lines(&text);
        for expected in ["target/", "gcp-cloud-run/target/", "auth-echo-core/target/"] {
            assert!(lines.contains(&expected), "{expected}: {lines:?}");
        }
        assert!(!lines.iter().any(|l| l.contains("..")), "{lines:?}");
    }

    /// Multi-crate isolated layout (#258): per-crate COPY pairs for
    /// primary + each path_dep, then `cargo build --manifest-path
    /// <primary>/Cargo.toml --bin <binary>`. Crucially, no `COPY . .`
    /// (which would over-bundle sibling lambda crates).
    #[test]
    fn dockerfile_multi_crate_isolated_emits_surgical_copy_pairs() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp-cloud-run".to_string(),
            path_deps: vec!["auth-echo-core".to_string()],
        });
        config.server.binary = Some("server".to_string());

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("COPY gcp-cloud-run/Cargo.toml gcp-cloud-run/Cargo.toml"));
        assert!(dockerfile.contains("COPY gcp-cloud-run/src gcp-cloud-run/src"));
        assert!(dockerfile.contains("COPY auth-echo-core/Cargo.toml auth-echo-core/Cargo.toml"));
        assert!(dockerfile.contains("COPY auth-echo-core/src auth-echo-core/src"));
        assert!(
            dockerfile.contains("--manifest-path gcp-cloud-run/Cargo.toml")
                || dockerfile.contains("--manifest-path gcp-cloud-run/Cargo.toml \\"),
            "expected manifest-path build step: {dockerfile}"
        );
        assert!(dockerfile.contains("--bin server"));
        // Critically: must NOT include `COPY . .` which would over-bundle.
        assert!(
            !dockerfile.contains("COPY . ."),
            "multi-crate isolated must not COPY . ."
        );
        // Sibling lambda crates must NOT appear in any COPY line. (The
        // file may mention `aws-lambda` in an explanatory header comment;
        // we only assert it does not appear in a Dockerfile directive.)
        for line in dockerfile.lines().filter(|l| l.starts_with("COPY ")) {
            assert!(
                !line.contains("aws-lambda") && !line.contains("lambda"),
                "lambda crate leaked into COPY directive: {line}"
            );
        }
    }

    /// `[layout]` falls back to `config.server.name` when `binary` is
    /// unset — preserves backward-compat with users who don't set the
    /// new optional field.
    #[test]
    fn dockerfile_multi_crate_isolated_binary_falls_back_to_server_name() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp".to_string(),
            path_deps: vec![],
        });
        // No config.server.binary set — should fall back to server.name.
        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("--bin auth-echo-cloud-run"));
    }

    /// Crate-name sanitization rejects injection attempts. A path_dep
    /// containing shell-meaningful characters must not propagate them
    /// into the generated Dockerfile.
    #[test]
    fn dockerfile_multi_crate_isolated_sanitizes_crate_names() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp-run".to_string(),
            path_deps: vec!["evil; rm -rf /".to_string(), "../escape".to_string()],
        });
        let dockerfile = render_dockerfile(&config).expect("render");
        // The sanitized form ("evilrm-rf" / "escape") may appear as crate
        // names, but no dangerous chars / path-escape segments must
        // propagate into the COPY/RUN lines.
        for line in dockerfile.lines().filter(|l| {
            l.starts_with("COPY")
                || l.starts_with("RUN")
                || l.starts_with("CMD")
                || l.starts_with("    --")
                || l.starts_with("    cp ")
                || l.starts_with("    || ")
        }) {
            assert!(!line.contains(';'), "directive contains semicolon: {line}");
            assert!(
                !line.contains(".."),
                "directive contains `..` path escape: {line}"
            );
            assert!(
                !line.contains("rm -rf"),
                "directive contains shell injection: {line}"
            );
        }
        // Positive check: sanitized form is present where path_deps used
        // to be.
        assert!(dockerfile.contains("evilrm-rf/Cargo.toml"));
        assert!(dockerfile.contains("escape/Cargo.toml"));
    }

    // ---------- Runtime / distroless tests (issue #259) ----------

    use crate::deployment::config::RuntimeConfig;

    /// Extract just the runtime (Stage 2) portion of a rendered
    /// Dockerfile. The builder (Stage 1) uses `apt-get` to install
    /// pkg-config + libssl-dev regardless of runtime base, so any
    /// runtime-stage assertions about apt / useradd / etc. need to be
    /// scoped to Stage 2.
    fn runtime_portion(dockerfile: &str) -> &str {
        let start = dockerfile
            .find("# Stage 2:")
            .expect("runtime stage marker present");
        &dockerfile[start..]
    }

    /// Default runtime base is distroless. No apt layer, no useradd, no
    /// HEALTHCHECK curl. Closes upstream #259.
    #[test]
    fn dockerfile_runtime_defaults_to_distroless() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(
            runtime.contains("FROM gcr.io/distroless/cc-debian12"),
            "default runtime FROM must be distroless: {runtime}"
        );
        assert!(!runtime.contains("apt-get install"));
        assert!(!runtime.contains("RUN useradd"));
        // Assert no HEALTHCHECK *directive* (a directive starts at the
        // line start; the explanatory comment may still mention the
        // keyword).
        assert!(
            !runtime
                .lines()
                .any(|l| l.trim_start().starts_with("HEALTHCHECK")),
            "no HEALTHCHECK directive in distroless runtime"
        );
        assert!(!runtime.contains("RUN chmod +x"));
        assert!(runtime.contains("CMD [\"/usr/local/bin/mcp-server\"]"));
    }

    /// `[runtime].base = "debian:bookworm-slim"` opts back to the
    /// shell-enabled runtime stage. With no `apt_packages` declared, no
    /// apt layer is emitted (operator opted out without declaring any
    /// packages — they get bare debian).
    #[test]
    fn dockerfile_runtime_debian_opt_out_no_apt_layer_by_default() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("debian:bookworm-slim".to_string()),
            apt_packages: vec![],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM debian:bookworm-slim"));
        // No apt layer in the runtime stage.
        assert!(!runtime.contains("apt-get install"));
        // Shell-enabled scaffolding is present.
        assert!(runtime.contains("useradd -m -u 1000 mcpserver"));
        assert!(runtime.contains("USER mcpserver"));
    }

    /// `[runtime].apt_packages = ["ca-certificates"]` with a debian base
    /// produces an apt-install layer with exactly those packages.
    #[test]
    fn dockerfile_runtime_debian_with_apt_packages_emits_apt_layer() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("debian:bookworm-slim".to_string()),
            apt_packages: vec!["ca-certificates".to_string(), "libssl3".to_string()],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM debian:bookworm-slim"));
        assert!(runtime.contains("apt-get install -y"));
        assert!(runtime.contains("ca-certificates"));
        assert!(runtime.contains("libssl3"));
        // Cleanup line for apt cache.
        assert!(runtime.contains("rm -rf /var/lib/apt/lists/*"));
    }

    /// `apt_packages` is ignored on non-debian bases (distroless,
    /// alpine, scratch, etc.). Issue #259 explicitly scopes the
    /// apt-packages knob to debian-family bases.
    #[test]
    fn dockerfile_runtime_apt_packages_ignored_on_distroless() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: None,
            apt_packages: vec!["ca-certificates".to_string()],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM gcr.io/distroless/cc-debian12"));
        assert!(!runtime.contains("apt-get install"));
    }

    /// `[runtime].base = "<arbitrary image>"` uses the value verbatim
    /// and falls through to the distroless-shaped (no-shell) runtime
    /// stage. Operator-managed bases that aren't debian/ubuntu get the
    /// minimal scaffolding so the operator is in control of any
    /// additional layers.
    #[test]
    fn dockerfile_runtime_custom_base_uses_distroless_shape() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("gcr.io/distroless/static-debian12".to_string()),
            apt_packages: vec![],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM gcr.io/distroless/static-debian12"));
        assert!(!runtime.contains("RUN useradd"));
    }

    // ---------- Property tests ----------
    //
    // Per CLAUDE.md ALWAYS requirements, every new feature must include
    // property-based invariants. These cover the two attack surfaces of
    // the new code: arbitrary user input flowing into Dockerfile
    // directives via [layout].path_deps, and arbitrary user input
    // flowing into the `gcloud --set-env-vars` argument via
    // [environment].

    use proptest::prelude::*;

    proptest! {
        /// INVARIANT: any string fed through the crate-name sanitizer
        /// produces output that is safe to splice into a Dockerfile
        /// COPY directive — only `[A-Za-z0-9_-]`, no path separators,
        /// no shell metacharacters, no newlines.
        #[test]
        fn prop_sanitize_path_dep_yields_safe_chars_only(input in ".{0,200}") {
            let mut config =
                DeployConfig::default_for_cloud_run_server(
                    "s".to_string(),
                    "p".to_string(),
                    "us-central1".to_string(),
                    std::env::temp_dir(),
                );
            config.layout = Some(LayoutConfig {
                kind: "multi-crate-isolated".to_string(),
                primary: "primary-crate".to_string(),
                path_deps: vec![input.clone()],
            });
            // Synthesize a Cargo.toml in temp so render_dockerfile
            // doesn't fail on the simple/workspace detection path. We
            // route through the multi-crate-isolated branch so the
            // sanitization invariant is what we measure.
            let tmp = tempfile::TempDir::new().expect("tmpdir");
            std::fs::write(
                tmp.path().join("Cargo.toml"),
                "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
            )
            .expect("seed cargo");
            config.project_root = tmp.path().to_path_buf();

            let dockerfile = render_dockerfile(&config).expect("render");
            // The COPY lines for path_deps must not contain dangerous
            // characters regardless of input. (The input may also
            // sanitize to empty — in which case no COPY line is emitted
            // for that dep, which is also safe.)
            for line in dockerfile.lines().filter(|l| l.starts_with("COPY ")) {
                for ch in line.chars() {
                    prop_assert!(
                        ch.is_ascii() && (ch != ';' && ch != '`' && ch != '$' && ch != '\\'
                            && ch != '"' && ch != '\''),
                        "COPY line contains dangerous char {:?}: {}", ch, line
                    );
                }
                prop_assert!(!line.contains(".."), "COPY line contains `..`: {}", line);
            }
        }

        /// INVARIANT: render_set_env_vars output is sorted by key and
        /// every entry has the form KEY=VALUE. Determinism matters —
        /// re-running deploy with no schema change must produce the
        /// exact same gcloud invocation so Cloud Run does not create a
        /// pointless new revision.
        #[test]
        fn prop_render_set_env_vars_is_sorted_and_well_formed(
            entries in prop::collection::vec(
                ("[A-Z][A-Z0-9_]{0,15}", "[a-zA-Z0-9._-]{0,40}"),
                0..16,
            )
        ) {
            let mut env = std::collections::HashMap::new();
            for (k, v) in entries {
                env.insert(k, v);
            }
            let rendered = super::super::env::render_set_env_vars(&env);
            if env.is_empty() {
                prop_assert_eq!(rendered, "");
                return Ok(());
            }
            let pairs: Vec<&str> = rendered.split(',').collect();
            prop_assert_eq!(pairs.len(), env.len());
            // Every pair is KEY=VALUE.
            for pair in &pairs {
                prop_assert!(pair.contains('='), "missing `=` in pair: {}", pair);
            }
            // Keys (the part before the first `=`) are sorted ascending.
            // We assert on keys — not on full pairs — because key sort
            // order can differ from full-string sort when values
            // contain bytes lower than `=` (0x3D), e.g. digits (0x30).
            let keys: Vec<&str> = pairs
                .iter()
                .map(|p| p.split('=').next().unwrap_or(""))
                .collect();
            let mut sorted_keys = keys.clone();
            sorted_keys.sort();
            prop_assert_eq!(keys, sorted_keys);
        }
    }
}
