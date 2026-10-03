//! Cloud Run `cargo pmcp deploy init` flow.
//!
//! Closes upstream issue #260: scaffolds `.pmcp/deploy.toml` with the
//! `[target]` + `[gcp]` + `[server]` + `[environment]` minimum-viable schema
//! alongside the existing `Dockerfile` / `.dockerignore` / `cloudbuild.yaml`
//! outputs.
//!
//! Idempotent: re-running `cargo pmcp deploy init --target-type
//! google-cloud-run` in a project directory keeps every file that already
//! exists — `.pmcp/deploy.toml` (so operators' filled-in `project_id`,
//! environment values, and any `[layout]` / `[runtime]` opt-ins are not
//! clobbered) and, since the debug session `cargo-pmcp-deploy-targets`
//! (finding A1), also `Dockerfile`, `.dockerignore` and `cloudbuild.yaml`,
//! which init used to overwrite. Files that are missing are written; files
//! that exist are left untouched, and init says which.

use std::path::Path;

use super::binary::{self, BuildTarget};
use super::dockerfile;
use crate::deployment::DeployConfig;
use anyhow::{Context, Result};

/// Initialise a Google Cloud Run deployment for `config.project_root`.
///
/// Writes (when missing):
/// - `.pmcp/deploy.toml` — Cloud Run-shaped schema (issue #260)
/// - `Dockerfile`
/// - `.dockerignore`
/// - `cloudbuild.yaml`
///
/// Pre-existing files are preserved verbatim.
///
/// When the Dockerfile is generated, its binary is resolved first (see
/// [`dockerfile::resolve_build_target`]) and recorded as `[server] binary` in
/// a NEW deploy.toml. If no binary can be chosen, the new deploy.toml is
/// still written (it is where `[server] binary` goes) and init stops before
/// any other file, with the candidates in the error.
///
/// # Errors
///
/// Returns an error if no binary can be chosen for a generated Dockerfile, or
/// if any of the deploy.toml / Dockerfile / .dockerignore / cloudbuild.yaml
/// artifacts cannot be written.
pub fn init_google_cloud_run(config: &DeployConfig) -> Result<()> {
    println!("🚀 Initializing Google Cloud Run deployment...");
    println!();

    let root = &config.project_root;
    let build_target = if root.join("Dockerfile").exists() {
        Ok(None)
    } else {
        dockerfile::resolve_build_target(config)
    };
    let resolved = build_target.as_ref().ok().and_then(Option::as_ref);
    let on_disk = config_on_disk(config, resolved);
    write_deploy_toml(&on_disk)?;
    let build_target = build_target?;

    let mut kept = Vec::new();
    let dockerfile_rendered = scaffold(root, "Dockerfile", &mut kept, || {
        dockerfile::render_dockerfile_for(config, build_target.as_ref())
    })?;
    if dockerfile_rendered {
        if let Some(target) = &build_target {
            println!(
                "     builds binary `{}`{}",
                target.binary,
                package_note(target)
            );
        }
    }
    scaffold(root, ".dockerignore", &mut kept, || {
        Ok(dockerfile::render_dockerignore(config))
    })?;
    scaffold(root, "cloudbuild.yaml", &mut kept, || {
        Ok(dockerfile::render_cloudbuild(config))
    })?;

    print_next_steps(&kept, &on_disk);
    Ok(())
}

fn package_note(target: &BuildTarget) -> String {
    target
        .package
        .as_ref()
        .map(|package| format!(" of package `{package}`"))
        .unwrap_or_default()
}

/// The config `.pmcp/deploy.toml` holds after init: an existing file is kept
/// byte for byte (so it is `config`, loaded from it); a new one records what
/// init resolved ([`with_recorded_binary`]). The next steps describe this one.
fn config_on_disk(config: &DeployConfig, resolved: Option<&BuildTarget>) -> DeployConfig {
    if config.project_root.join(".pmcp/deploy.toml").exists() {
        config.clone()
    } else {
        with_recorded_binary(config, resolved)
    }
}

/// `config` with what init resolved recorded in it: the binary as
/// `[server] binary` when it declares none, and `[server] mcp_path = "/"`
/// for a `cargo pmcp new` server. Only written when the deploy.toml is new
/// (an existing one is kept byte for byte).
fn with_recorded_binary(config: &DeployConfig, target: Option<&BuildTarget>) -> DeployConfig {
    let mut config = config.clone();
    if config.server.binary.is_none() {
        config.server.binary = target.map(|t| t.binary.clone());
    }
    if config.server.mcp_path.is_none() && target.is_some_and(|t| serves_at_root(&config, t)) {
        config.server.mcp_path = Some("/".to_string());
    }
    config
}

/// The crate `cargo pmcp new` generates for every server's HTTP entry point;
/// its `run_http` serves MCP at `/` (`StreamableHttpServer`).
const SERVER_COMMON_CRATE: &str = "server-common";

/// True when the binary is a `cargo pmcp new` server: its package depends on
/// the generated `server-common` crate, whose `run_http` mounts MCP at `/`.
/// Recording `mcp_path = "/"` for it keeps cargo-pmcp's own scaffold and the
/// Cloud Run `/mcp` default in agreement (debug session
/// `cargo-pmcp-deploy-targets`, #8).
fn serves_at_root(config: &DeployConfig, target: &BuildTarget) -> bool {
    target.package.as_deref().is_some_and(|package| {
        binary::package_depends_on(&config.project_root, package, SERVER_COMMON_CRATE)
    })
}

/// Write `root/file` from `render` when it does not exist; record it in
/// `kept` otherwise. Returns whether the file was written.
fn scaffold(
    root: &Path,
    file: &'static str,
    kept: &mut Vec<&'static str>,
    render: impl FnOnce() -> Result<String>,
) -> Result<bool> {
    let path = root.join(file);
    if path.exists() {
        println!("   ⏭  {file} already exists — preserving");
        kept.push(file);
        return Ok(false);
    }
    std::fs::write(&path, render()?).with_context(|| format!("Failed to write {file}"))?;
    println!("   ✓ Generated {file}");
    Ok(true)
}

fn print_next_steps(kept: &[&str], config: &DeployConfig) {
    print!("{}", next_steps_text(kept, config));
}

/// What the server must do behind the Cloud Run ingress, and where the deploy
/// looks for MCP (debug session `cargo-pmcp-deploy-targets`, #12 and #8; the
/// azure-container-apps init prints the same origin warning).
fn serving_requirements_text(config: &DeployConfig) -> String {
    let path = crate::deployment::mcp_endpoint::endpoint_path(
        "google-cloud-run",
        config.server.mcp_path.as_deref(),
    )
    .unwrap_or(crate::deployment::mcp_endpoint::DEFAULT_MCP_PATH);
    let root = path.trim_end_matches('/').is_empty();
    let probed = if root {
        "the service URL itself".to_string()
    } else {
        format!("<service URL>{path}")
    };
    let other = if root {
        "set mcp_path = \"/mcp\" if the server serves MCP there"
    } else {
        "set mcp_path = \"/\" for a server mounted at the root"
    };
    format!(
        "\n⚠  The server MUST listen on 0.0.0.0:$PORT (Cloud Run sets PORT=8080) and accept the\n\
         \x20  public Host header: with pmcp::axum::router_with_config, set\n\
         \x20  `allowed_origins: Some(AllowedOrigins::any())` (or an explicit list naming the\n\
         \x20  service URL). `allowed_origins: None` means localhost only, so the DNS-rebinding\n\
         \x20  guard answers 403 to every request through the Cloud Run ingress.\n\
         \n🔗 The deploy verifies, and prints as the MCP endpoint, {probed}\n\
         \x20  ([server] mcp_path = \"{path}\"; {other}).\n"
    )
}

/// What init prints after scaffolding: kept files, next steps, the files.
fn next_steps_text(kept: &[&str], config: &DeployConfig) -> String {
    let mut text = String::from("\n✅ Google Cloud Run deployment initialized!\n");
    if !kept.is_empty() {
        text.push_str(&format!(
            "\nℹ️  Kept existing {}. To regenerate one from .pmcp/deploy.toml, delete it and re-run init.\n",
            kept.join(", ")
        ));
    }
    text.push_str(
        "\n📝 Next steps:\n\
         \x20  1. Edit .pmcp/deploy.toml: set [gcp].project_id, [server].name,\n\
         \x20     and any [environment] keys your server requires\n\
         \x20  2. Authenticate: gcloud auth login\n\
         \x20  3. Set project: gcloud config set project PROJECT_ID\n\
         \x20  4. Deploy: cargo pmcp deploy --target google-cloud-run\n\
         \n💡 Files:\n\
         \x20  • .pmcp/deploy.toml - IaC source of truth\n\
         \x20  • Dockerfile - Multi-stage Rust build of [server] binary\n\
         \x20  • .dockerignore - Optimize build context\n\
         \x20  • cloudbuild.yaml - Optional Cloud Build configuration\n",
    );
    text.push_str(&serving_requirements_text(config));
    text
}

/// Write `.pmcp/deploy.toml` only when it doesn't already exist.
///
/// Delegates to [`DeployConfig::save_if_missing`] for the actual
/// serialize-and-write logic; this wrapper exists to print the
/// scaffolder-status line the operator sees.
fn write_deploy_toml(config: &DeployConfig) -> Result<()> {
    if config.save_if_missing(&config.project_root)? {
        println!("   ✓ Generated .pmcp/deploy.toml");
        if let Some(binary) = &config.server.binary {
            println!("     [server] binary = \"{binary}\"");
        }
        if let Some(path) = &config.server.mcp_path {
            println!("     [server] mcp_path = \"{path}\"");
        }
    } else {
        println!("   ⏭  .pmcp/deploy.toml already exists — preserving");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_cloud_run_config(project_root: PathBuf) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "auth-echo-cloud-run".to_string(),
            "your-gcp-project-id".to_string(),
            "us-central1".to_string(),
            project_root,
        )
    }

    /// Cloud Run init must persist a roundtrippable deploy.toml — closes
    /// the core of upstream issue #260 (the existing AWS Lambda path is the
    /// reference precedent at built-in/test-harness/.../aws-lambda/.pmcp/
    /// deploy.toml).
    #[test]
    fn init_writes_cloud_run_shaped_deploy_toml() {
        let tmp = TempDir::new().expect("tmpdir");
        let config = make_cloud_run_config(tmp.path().to_path_buf());

        write_deploy_toml(&config).expect("write succeeds");

        let written =
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read back");
        assert!(written.contains("type = \"google-cloud-run\""));
        assert!(written.contains("[gcp]"));
        assert!(written.contains("project_id = \"your-gcp-project-id\""));
        assert!(written.contains("region = \"us-central1\""));
        assert!(written.contains("memory = \"256Mi\""));
        assert!(
            !written.contains("[aws]"),
            "Cloud Run deploy.toml must not contain an [aws] block"
        );

        // Roundtrip: the written file must parse back into the same shape.
        let reloaded: DeployConfig = toml::from_str(&written).expect("reload");
        assert_eq!(reloaded.target.target_type, "google-cloud-run");
        assert!(reloaded.aws.is_none());
        let gcp = reloaded.gcp.as_ref().expect("gcp present");
        assert_eq!(gcp.project_id, "your-gcp-project-id");
    }

    /// Re-running init on a project where deploy.toml exists must not
    /// clobber operator edits. This is the scaffolder-immunity invariant
    /// from upstream #260.
    #[test]
    fn init_preserves_existing_deploy_toml() {
        let tmp = TempDir::new().expect("tmpdir");
        let pmcp = tmp.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp).expect("mkdir");
        let sentinel = "# operator edits go here\n[target]\ntype = \"google-cloud-run\"\nversion = \"1.0.0\"\n";
        std::fs::write(pmcp.join("deploy.toml"), sentinel).expect("seed file");

        let config = make_cloud_run_config(tmp.path().to_path_buf());
        write_deploy_toml(&config).expect("re-init succeeds");

        let after =
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read back");
        assert_eq!(after, sentinel, "existing deploy.toml must be preserved");
    }

    // ---------- Debug session cargo-pmcp-deploy-targets (PR-C) ----------

    use super::super::fixture;

    fn read(root: &std::path::Path, file: &str) -> String {
        std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
    }

    /// The acceptance shape inits cleanly: the Dockerfile builds `serve` by
    /// name, and a NEW deploy.toml records the chosen binary.
    #[test]
    fn x_lambda_init_scaffolds_a_dockerfile_for_serve_and_records_it() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());
        let config = make_cloud_run_config(tmp.path().to_path_buf());

        init_google_cloud_run(&config).expect("init");

        let dockerfile = read(tmp.path(), "Dockerfile");
        assert!(
            dockerfile.contains("RUN cargo build --release -p x-lambda --bin serve\n"),
            "{dockerfile}"
        );
        assert!(dockerfile.contains("RUN cp target/release/serve /app/mcp-server\n"));
        let saved = crate::deployment::DeployConfig::load(tmp.path()).expect("deploy.toml");
        assert_eq!(saved.server.binary.as_deref(), Some("serve"));
        assert!(read(tmp.path(), ".dockerignore")
            .lines()
            .any(|l| l == "target/"));
        assert!(read(tmp.path(), "cloudbuild.yaml").contains("'auth-echo-cloud-run'"));
    }

    /// A1: re-running init keeps an existing Dockerfile, .dockerignore and
    /// cloudbuild.yaml byte for byte, like deploy.toml.
    #[test]
    fn reinit_keeps_existing_dockerfile_dockerignore_and_cloudbuild() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());
        let curated = [
            ("Dockerfile", "FROM scratch\n# curated\n"),
            (".dockerignore", "# curated\nsecret/\n"),
            ("cloudbuild.yaml", "# curated\nsteps: []\n"),
        ];
        for (file, text) in curated {
            std::fs::write(tmp.path().join(file), text).expect("seed");
        }

        init_google_cloud_run(&make_cloud_run_config(tmp.path().to_path_buf())).expect("init");

        for (file, text) in curated {
            assert_eq!(read(tmp.path(), file), text, "{file} must be kept");
        }
    }

    /// A kept Dockerfile is the operator's: init does not need (or try) to
    /// choose a binary for it, even when the choice would be ambiguous.
    #[test]
    fn a_kept_dockerfile_needs_no_binary_choice() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda_with_local(tmp.path());
        std::fs::write(tmp.path().join("Dockerfile"), "FROM scratch\n").expect("seed");

        init_google_cloud_run(&make_cloud_run_config(tmp.path().to_path_buf())).expect("init");

        assert_eq!(read(tmp.path(), "Dockerfile"), "FROM scratch\n");
        let saved = crate::deployment::DeployConfig::load(tmp.path()).expect("deploy.toml");
        assert_eq!(saved.server.binary, None);
    }

    /// Two candidate binaries: init writes the new deploy.toml (where the
    /// fix goes), refuses to guess, and writes no Dockerfile. After
    /// `[server] binary` is set, the re-run builds that binary.
    #[test]
    fn ambiguous_binaries_stop_init_until_server_binary_is_set() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda_with_local(tmp.path());
        let config = make_cloud_run_config(tmp.path().to_path_buf());

        let message = format!(
            "{:#}",
            init_google_cloud_run(&config).expect_err("ambiguous")
        );
        assert!(
            message.contains("`local` (package `x-lambda`)"),
            "{message}"
        );
        assert!(
            message.contains("`serve` (package `x-lambda`)"),
            "{message}"
        );
        assert!(tmp.path().join(".pmcp/deploy.toml").exists());
        assert!(!tmp.path().join("Dockerfile").exists());

        let path = tmp.path().join(".pmcp/deploy.toml");
        let text = std::fs::read_to_string(&path).expect("read");
        let edited = text.replacen("[server]\n", "[server]\nbinary = \"serve\"\n", 1);
        assert_ne!(
            edited, text,
            "the [server] header is in the scaffolded file"
        );
        std::fs::write(&path, edited).expect("write");
        let kept = crate::deployment::DeployConfig::load(tmp.path()).expect("reload");

        init_google_cloud_run(&kept).expect("re-init");
        assert!(read(tmp.path(), "Dockerfile").contains("--bin serve\n"));
    }

    /// cloudbuild.yaml no longer claims init regenerates it.
    #[test]
    fn cloudbuild_header_tells_how_to_regenerate() {
        let tmp = TempDir::new().expect("tmpdir");
        let config = make_cloud_run_config(tmp.path().to_path_buf());
        let text = dockerfile::render_cloudbuild(&config);
        assert!(
            !text.contains("will be\n# overwritten on the next init"),
            "{text}"
        );
        assert!(text.contains("delete this file"), "{text}");
    }

    // ---------- Debug session cargo-pmcp-deploy-targets (PR-D) ----------

    /// #7: a new init pushes to Artifact Registry, in deploy.toml and in the
    /// Cloud Build file alike.
    #[test]
    fn a_new_init_records_the_repository_and_cloudbuild_uses_it() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());

        init_google_cloud_run(&make_cloud_run_config(tmp.path().to_path_buf())).expect("init");

        let deploy_toml = read(tmp.path(), ".pmcp/deploy.toml");
        assert!(
            deploy_toml.contains("repository = \"pmcp\""),
            "{deploy_toml}"
        );
        let cloudbuild = read(tmp.path(), "cloudbuild.yaml");
        assert!(
            cloudbuild.contains("us-central1-docker.pkg.dev/$PROJECT_ID/pmcp/auth-echo-cloud-run"),
            "{cloudbuild}"
        );
    }

    /// #12 + #8: init says what the server must do behind the Cloud Run
    /// ingress (bind 0.0.0.0:$PORT, an origin policy that is not
    /// localhost-only) and where the deploy will look for MCP.
    #[test]
    fn next_steps_name_the_origin_policy_and_the_endpoint_path() {
        let tmp = TempDir::new().expect("tmpdir");
        let config = make_cloud_run_config(tmp.path().to_path_buf());
        let text = next_steps_text(&[], &config);
        assert!(text.contains("0.0.0.0:$PORT"), "{text}");
        assert!(text.contains("AllowedOrigins::any()"), "{text}");
        assert!(text.contains("allowed_origins"), "{text}");
        assert!(text.contains("403"), "{text}");
        assert!(text.contains("/mcp"), "{text}");
        assert!(text.contains("mcp_path"), "{text}");

        let mut root = config;
        root.server.mcp_path = Some("/".to_string());
        let text = next_steps_text(&[], &root);
        assert!(text.contains("the service URL itself"), "{text}");
    }

    /// A `cargo pmcp new` workspace: the server binary depends on the
    /// generated `server-common` crate.
    fn write_cargo_pmcp_new_workspace(root: &std::path::Path) {
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
        )
        .expect("root");
        let common = root.join("crates/server-common");
        std::fs::create_dir_all(common.join("src")).expect("mkdir");
        std::fs::write(
            common.join("Cargo.toml"),
            "[package]\nname = \"server-common\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("server-common");
        std::fs::write(common.join("src/lib.rs"), "").expect("lib.rs");
        fixture::write_crate(
            &root.join("crates/acme-server"),
            "acme-server",
            &["acme-server"],
            "[dependencies]\nserver-common = { path = \"../server-common\" }\n",
        );
    }

    /// #8: the `cargo pmcp new` server (`server_common::run_http`) serves MCP
    /// at the root, so a new deploy.toml for it records `mcp_path = "/"`:
    /// the deploy's default and cargo-pmcp's own scaffold agree.
    #[test]
    fn a_cargo_pmcp_new_server_records_the_root_mcp_path() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_pmcp_new_workspace(tmp.path());

        init_google_cloud_run(&make_cloud_run_config(tmp.path().to_path_buf())).expect("init");

        let saved = crate::deployment::DeployConfig::load(tmp.path()).expect("deploy.toml");
        assert_eq!(saved.server.binary.as_deref(), Some("acme-server"));
        assert_eq!(saved.server.mcp_path.as_deref(), Some("/"));
    }

    /// An existing deploy.toml is kept byte for byte: init records nothing in
    /// it, even for a `cargo pmcp new` server (the next steps then describe
    /// the `/mcp` default the deploy will use).
    #[test]
    fn a_kept_deploy_toml_gets_no_recorded_mcp_path() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_pmcp_new_workspace(tmp.path());
        let config = make_cloud_run_config(tmp.path().to_path_buf());
        write_deploy_toml(&config).expect("seed deploy.toml");
        let before = read(tmp.path(), ".pmcp/deploy.toml");

        init_google_cloud_run(&config).expect("init");

        assert_eq!(read(tmp.path(), ".pmcp/deploy.toml"), before);
        let target = dockerfile::resolve_build_target(&config).expect("resolves");
        let described = config_on_disk(&config, target.as_ref());
        assert_eq!(described.server.mcp_path, None);
        assert!(next_steps_text(&[], &described).contains("<service URL>/mcp"));

        std::fs::remove_file(tmp.path().join(".pmcp/deploy.toml")).expect("rm");
        let described = config_on_disk(&config, target.as_ref());
        assert_eq!(described.server.mcp_path.as_deref(), Some("/"));
        assert!(next_steps_text(&[], &described).contains("the service URL itself"));
    }

    /// Any other server gets no `mcp_path` line: the `/mcp` default applies.
    #[test]
    fn other_servers_leave_mcp_path_to_the_default() {
        let tmp = TempDir::new().expect("tmpdir");
        fixture::write_x_lambda(tmp.path());

        init_google_cloud_run(&make_cloud_run_config(tmp.path().to_path_buf())).expect("init");

        let saved = crate::deployment::DeployConfig::load(tmp.path()).expect("deploy.toml");
        assert_eq!(saved.server.mcp_path, None);
        assert!(!read(tmp.path(), ".pmcp/deploy.toml").contains("mcp_path"));
    }
}
