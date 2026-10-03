use super::auth;
use super::image::ImageRegistry;
use crate::deployment::{r#trait::DeploymentOutputs, DeployConfig};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// The programs the Cloud Run deploy runs. A seam: tests substitute recording
/// stand-ins (`fake_cloud`) and assert the argv, with no live GCP call.
pub(super) struct CloudTools {
    /// `docker`.
    pub docker: PathBuf,
    /// `gcloud`.
    pub gcloud: PathBuf,
}

impl CloudTools {
    /// The programs on `PATH`.
    fn system() -> Self {
        Self {
            docker: PathBuf::from("docker"),
            gcloud: PathBuf::from("gcloud"),
        }
    }
}

/// Run `program args` (in `dir` when given) and return its output.
fn run(program: &Path, args: &[String], dir: Option<&Path>) -> Result<std::process::Output> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command
        .output()
        .with_context(|| format!("Failed to run {} {}", program.display(), args.join(" ")))
}

/// Run `program args` and fail with `what` and its stderr unless it succeeds.
fn run_ok(program: &Path, args: &[String], dir: Option<&Path>, what: &str) -> Result<String> {
    let output = run(program, args, dir)?;
    if !output.status.success() {
        bail!("{what}:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

/// Deploy Rust MCP server to Google Cloud Run.
///
/// Flow:
/// 1. Verify authentication.
/// 2. Resolve deployment parameters from `.pmcp/deploy.toml`
///    (`[gcp]`, `[server]`, `[environment]`), with legacy env-var fallback
///    for projects that pre-date the schema. Closes upstream issue #260.
/// 3. Refuse absolute path dependencies.
/// 4. With `[gcp] repository`: create the Artifact Registry repository when
///    it does not exist (debug session `cargo-pmcp-deploy-targets`, #7).
/// 5. Build the Docker image (`docker buildx --platform linux/amd64`).
/// 6. Push it: Artifact Registry with `[gcp] repository`, else `gcr.io`.
/// 7. `gcloud run deploy` with `--set-env-vars` populated from
///    `[environment]`.
/// 8. Return the service URL (the CLI appends `[server] mcp_path`).
///
/// # Errors
///
/// Returns an error if authentication fails, `[gcp] repository` is not a
/// valid repository id, absolute path dependencies are detected, or any of
/// the `docker` / `gcloud` subprocess invocations fail.
pub async fn deploy_to_cloud_run(config: &DeployConfig) -> Result<DeploymentOutputs> {
    deploy_with(config, &CloudTools::system())
}

/// [`deploy_to_cloud_run`] with the programs it runs given.
fn deploy_with(config: &DeployConfig, tools: &CloudTools) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to Google Cloud Run...");
    println!();

    let project_id = verify_auth_and_project(config, tools)?;
    let params = resolve_params(config);
    print_configuration(config, &params);

    ensure_no_absolute_path_deps(config)?;
    let registry = image_registry(config, &params)?;
    let image_tag = registry.image(&project_id, &params.service_name);
    ensure_repository(tools, &registry, &project_id)?;

    build_image(tools, config, &image_tag)?;
    push_image(tools, &registry, &image_tag)?;
    deploy_service(tools, config, &params, &image_tag, &project_id)?;
    let url = service_url(tools, &params, &project_id)?;
    print_details(&params, &project_id, &url);

    Ok(DeploymentOutputs {
        url: Some(url),
        additional_urls: vec![],
        regions: vec![params.region],
        stack_name: Some(params.service_name),
        version: None,
        custom: {
            let mut custom = std::collections::HashMap::new();
            custom.insert(
                "project_id".to_string(),
                serde_json::Value::String(project_id),
            );
            custom.insert("image".to_string(), serde_json::Value::String(image_tag));
            custom
        },
    })
}

/// Step 1: authentication. The deploy.toml carries the *expected* project id;
/// the actual one is whatever gcloud is configured for. The deploy.toml value
/// wins when present so the CLI is reproducible across machines.
fn verify_auth_and_project(config: &DeployConfig, tools: &CloudTools) -> Result<String> {
    println!("🔐 Verifying authentication...");
    auth::check_gcloud_auth_with(&tools.gcloud).context("Not authenticated with Google Cloud")?;
    let project_id = config
        .gcp
        .as_ref()
        .map(|g| g.project_id.clone())
        .filter(|p| !p.is_empty() && p != "your-gcp-project-id")
        .map_or_else(|| auth::get_project_id_with(&tools.gcloud), Ok)?;
    println!("   ✓ Project: {project_id}");
    println!();
    Ok(project_id)
}

fn print_configuration(config: &DeployConfig, params: &CloudRunParams) {
    println!("📋 Deployment configuration:");
    println!("   Region: {}", params.region);
    println!("   Service: {}", params.service_name);
    println!("   Memory: {}", params.memory);
    println!("   CPU: {}", params.cpu);
    println!("   Max instances: {}", params.max_instances);
    println!("   Min instances: {}", params.min_instances);
    println!("   Allow unauthenticated: {}", params.allow_unauth);
    if let Some(ingress) = &params.ingress {
        println!("   Ingress: {ingress}");
    }
    if !config.environment.is_empty() {
        println!(
            "   Environment variables: {} entries from [environment]",
            config.environment.len()
        );
    }
    println!();
}

/// Where the image goes: the Artifact Registry repository `[gcp] repository`
/// (validated), else gcr.io as before 0.28.0, with a note on how to move.
fn image_registry(config: &DeployConfig, params: &CloudRunParams) -> Result<ImageRegistry> {
    let repository = config.gcp.as_ref().and_then(|g| g.repository.as_deref());
    if let Some(repository) = repository {
        super::image::validate_repository(repository).context("invalid .pmcp/deploy.toml")?;
    } else {
        println!(
            "ℹ️  [gcp] repository is not set, so the image goes to gcr.io (as before 0.28.0). \
             Container Registry stopped taking writes on 2025-03-18; gcr.io pushes need an \
             Artifact Registry gcr.io repository in the project. To push to Artifact Registry \
             instead, add `repository = \"{}\"` under [gcp] (the deploy creates it).\n",
            super::image::DEFAULT_REPOSITORY
        );
    }
    Ok(ImageRegistry::for_config(repository, &params.region))
}

/// Create the Artifact Registry repository unless it exists ("already
/// exists" counts as success). Nothing to do for `gcr.io`.
fn ensure_repository(tools: &CloudTools, registry: &ImageRegistry, project: &str) -> Result<()> {
    let Some(args) = registry.create_repository_args(project) else {
        return Ok(());
    };
    println!("📦 Ensuring the {} repository exists...", registry.label());
    let output = run(&tools.gcloud, &args, None)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        println!(
            "   ✓ Created {}",
            registry.image_name(project, "").trim_end_matches('/')
        );
    } else if super::image::is_already_exists(&stderr) {
        println!(
            "   ✓ Exists: {}",
            registry.image_name(project, "").trim_end_matches('/')
        );
    } else {
        bail!(
            "Could not create the Artifact Registry repository ({}). Creating it needs \
             roles/artifactregistry.admin (or create it once yourself: gcloud {}):\n{stderr}",
            registry.image_name(project, "").trim_end_matches('/'),
            args.join(" ")
        );
    }
    println!();
    Ok(())
}

fn build_image(tools: &CloudTools, config: &DeployConfig, image_tag: &str) -> Result<()> {
    println!("🔨 Building Docker image for linux/amd64...");
    let args = strings(&[
        "buildx",
        "build",
        "--platform",
        "linux/amd64",
        "-t",
        image_tag,
        "--load",
        ".",
    ]);
    run_ok(
        &tools.docker,
        &args,
        Some(&config.project_root),
        "Docker build failed",
    )?;
    println!("   ✓ Image built: {image_tag}");
    println!();
    Ok(())
}

fn push_image(tools: &CloudTools, registry: &ImageRegistry, image_tag: &str) -> Result<()> {
    println!("📤 Pushing image to {}...", registry.label());
    run_ok(
        &tools.gcloud,
        &registry.configure_docker_args(),
        None,
        "Failed to configure Docker authentication",
    )?;
    run_ok(
        &tools.docker,
        &strings(&["push", image_tag]),
        None,
        "Docker push failed",
    )?;
    println!("   ✓ Image pushed");
    println!();
    Ok(())
}

/// `gcloud run deploy` with `--set-env-vars` populated from
/// `config.environment` (closes the env-var-drift gap in #260).
fn deploy_service(
    tools: &CloudTools,
    config: &DeployConfig,
    params: &CloudRunParams,
    image_tag: &str,
    project_id: &str,
) -> Result<()> {
    println!("🚀 Deploying to Cloud Run...");
    let mut args = strings(&[
        "run",
        "deploy",
        &params.service_name,
        "--image",
        image_tag,
        "--region",
        &params.region,
        "--project",
        project_id,
        "--platform",
        "managed",
        "--memory",
        &params.memory,
        "--cpu",
        &params.cpu,
        "--max-instances",
        &params.max_instances.to_string(),
        "--min-instances",
        &params.min_instances.to_string(),
        "--port",
        "8080",
        "--quiet",
    ]);
    if let Some(ingress) = &params.ingress {
        args.extend(strings(&["--ingress", ingress]));
    }
    let env_vars_arg = super::env::render_set_env_vars(&config.environment);
    if !env_vars_arg.is_empty() {
        args.extend(strings(&["--set-env-vars", &env_vars_arg]));
    }
    args.push(
        if params.allow_unauth {
            "--allow-unauthenticated"
        } else {
            "--no-allow-unauthenticated"
        }
        .to_string(),
    );
    run_ok(&tools.gcloud, &args, None, "Cloud Run deployment failed")?;
    println!("   ✓ Service deployed successfully");
    println!();
    Ok(())
}

fn service_url(tools: &CloudTools, params: &CloudRunParams, project_id: &str) -> Result<String> {
    println!("🔍 Getting service URL...");
    let args = strings(&[
        "run",
        "services",
        "describe",
        &params.service_name,
        "--region",
        &params.region,
        "--project",
        project_id,
        "--format",
        "value(status.url)",
    ]);
    run_ok(&tools.gcloud, &args, None, "Failed to retrieve service URL")
}

fn print_details(params: &CloudRunParams, project_id: &str, url: &str) {
    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Project: {project_id}");
    println!("   Region: {}", params.region);
    println!("   Service: {}", params.service_name);
    println!("   Service URL: {url}");

    if !params.allow_unauth {
        println!();
        println!("🔒 Authentication required:");
        println!(
            "   gcloud run services proxy {} --region {}",
            params.service_name, params.region
        );
    }
}

/// Refuse the deploy when a dependency the Docker build would load has an
/// absolute `path` (it does not exist inside the build context).
///
/// Debug session `cargo-pmcp-deploy-targets`, finding #9: this used to be a
/// substring match (`path = "/`) over the primary manifest's raw text, which
/// refused a project for a commented-out `[patch]` line and missed
/// `path="/x"`, target-specific tables and member manifests. The manifests
/// are now parsed (see [`super::manifest::absolute_path_dependencies_in`]).
///
/// For multi-crate-isolated layouts, `project_root` is intentionally a
/// parent directory of the primary crate (so multiple sibling crates can
/// be COPY'd into the Docker build context) and has no `Cargo.toml` of
/// its own. The walk starts at the primary crate's manifest instead.
fn ensure_no_absolute_path_deps(config: &DeployConfig) -> Result<()> {
    let primary = match config
        .layout
        .as_ref()
        .filter(|l| l.is_multi_crate_isolated())
    {
        Some(layout) => config.project_root.join(&layout.primary).join("Cargo.toml"),
        None => config.project_root.join("Cargo.toml"),
    };
    let findings = super::manifest::absolute_path_dependencies_in(&primary, &config.project_root)?;
    if findings.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = findings
        .iter()
        .map(|f| format!("  {}: {}", f.manifest.display(), f.dependency))
        .collect();
    bail!(
        "Cannot deploy: these path dependencies are absolute, so they do not exist inside the \
         Docker build context:\n{}\nUse crates.io or git dependencies, or paths relative to the \
         manifest that stay inside {}.",
        lines.join("\n"),
        config.project_root.display()
    );
}

/// Resolved deployment parameters with deploy.toml-then-env-var precedence.
///
/// `config.server.*` is the new source of truth (issue #260). Env vars
/// remain as a fallback only for projects whose deploy.toml pre-dates the
/// Cloud Run schema additions.
struct CloudRunParams {
    region: String,
    service_name: String,
    memory: String,
    cpu: String,
    max_instances: u32,
    min_instances: u32,
    allow_unauth: bool,
    ingress: Option<String>,
}

fn resolve_params(config: &DeployConfig) -> CloudRunParams {
    let region = config
        .gcp
        .as_ref()
        .map(|g| g.region.clone())
        .filter(|r| !r.is_empty())
        .or_else(|| std::env::var("CLOUD_RUN_REGION").ok())
        .unwrap_or_else(auth::get_region);

    let memory = config
        .server
        .memory
        .clone()
        .or_else(|| std::env::var("CLOUD_RUN_MEMORY").ok())
        .unwrap_or_else(|| "512Mi".to_string());

    let cpu = config
        .server
        .cpu
        .clone()
        .or_else(|| std::env::var("CLOUD_RUN_CPU").ok())
        .unwrap_or_else(|| "1".to_string());

    let max_instances = config
        .server
        .max_instances
        .or_else(|| {
            std::env::var("CLOUD_RUN_MAX_INSTANCES")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(10);

    let min_instances = config.server.min_instances.unwrap_or(0);

    let allow_unauth = config
        .server
        .allow_unauthenticated
        .or_else(|| {
            std::env::var("CLOUD_RUN_ALLOW_UNAUTHENTICATED")
                .ok()
                .map(|v| v == "true")
        })
        .unwrap_or(true);

    CloudRunParams {
        region,
        service_name: config.server.name.clone(),
        memory,
        cpu,
        max_instances,
        min_instances,
        allow_unauth,
        ingress: config.server.ingress.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_params_prefers_config_over_env_var() {
        let config = DeployConfig::default_for_cloud_run_server(
            "test-server".to_string(),
            "test-project".to_string(),
            "europe-west1".to_string(),
            std::path::PathBuf::from("/tmp"),
        );
        let params = resolve_params(&config);
        assert_eq!(params.region, "europe-west1");
        assert_eq!(params.memory, "256Mi");
        assert_eq!(params.cpu, "1");
        assert_eq!(params.max_instances, 10);
        assert_eq!(params.min_instances, 0);
        assert!(params.allow_unauth);
        assert_eq!(params.ingress.as_deref(), Some("all"));
        assert_eq!(params.service_name, "test-server");
    }

    // ---------- Absolute-path guard (debug session cargo-pmcp-deploy-targets, #9) ----------

    use super::super::fixture;
    use crate::deployment::config::LayoutConfig;

    fn config_at(root: &std::path::Path) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "x".to_string(),
            "p".to_string(),
            "us-central1".to_string(),
            root.to_path_buf(),
        )
    }

    /// The field report's run 1 failed in 1 s on this manifest: the only
    /// absolute path is on a commented-out `[patch]` line.
    #[test]
    fn a_commented_out_absolute_patch_line_does_not_block_the_deploy() {
        let tmp = tempfile::tempdir().expect("tmp");
        fixture::write_x_lambda(tmp.path());
        ensure_no_absolute_path_deps(&config_at(tmp.path())).expect("nothing absolute");
    }

    /// `path="/x"` (no spaces) was missed by the substring match.
    #[test]
    fn an_unspaced_absolute_path_dependency_blocks_the_deploy_naming_it() {
        let tmp = tempfile::tempdir().expect("tmp");
        fixture::write_crate(
            tmp.path(),
            "svc",
            &["serve"],
            "[dependencies]\nshared = { path=\"/opt/src/shared\" }\n",
        );
        let message = format!(
            "{:#}",
            ensure_no_absolute_path_deps(&config_at(tmp.path())).expect_err("absolute")
        );
        assert!(
            message.contains("[dependencies] shared = { path = \"/opt/src/shared\" }"),
            "{message}"
        );
    }

    /// Member manifests were never read: only the root Cargo.toml was.
    #[test]
    fn an_absolute_path_in_a_workspace_member_blocks_the_deploy() {
        let tmp = tempfile::tempdir().expect("tmp");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .expect("root");
        fixture::write_crate(
            &tmp.path().join("crates/server"),
            "server",
            &["serve"],
            "[target.'cfg(unix)'.dependencies]\nghost = { path = \"/opt/ghost\" }\n",
        );
        let message = format!(
            "{:#}",
            ensure_no_absolute_path_deps(&config_at(tmp.path())).expect_err("absolute")
        );
        assert!(message.contains("crates/server/Cargo.toml"), "{message}");
        assert!(message.contains("ghost"), "{message}");
    }

    #[test]
    fn relative_path_dependencies_pass() {
        let tmp = tempfile::tempdir().expect("tmp");
        fixture::write_crate(&tmp.path().join("core"), "core", &[], "");
        std::fs::write(tmp.path().join("core/src/lib.rs"), "").expect("lib");
        fixture::write_crate(
            tmp.path(),
            "svc",
            &["serve"],
            "[dependencies]\ncore = { path = \"core\" }\n\n[workspace]\n",
        );
        ensure_no_absolute_path_deps(&config_at(tmp.path())).expect("relative is fine");
    }

    /// Multi-crate isolated layout: the primary crate's manifest is the
    /// entry point (the project root has no Cargo.toml).
    #[test]
    fn the_isolated_layout_checks_the_primary_crate() {
        let tmp = tempfile::tempdir().expect("tmp");
        fixture::write_crate(
            &tmp.path().join("gcp"),
            "gcp",
            &["server"],
            "[dependencies]\ncore = { path = \"../core\" }\n",
        );
        fixture::write_crate(
            &tmp.path().join("core"),
            "core",
            &[],
            "[dependencies]\nghost = { path = \"~/ghost\" }\n",
        );
        let mut config = config_at(tmp.path());
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp".to_string(),
            path_deps: vec!["core".to_string()],
        });
        let message = format!(
            "{:#}",
            ensure_no_absolute_path_deps(&config).expect_err("absolute in a path dep")
        );
        assert!(message.contains("core/Cargo.toml"), "{message}");
        assert!(message.contains("~/ghost"), "{message}");
    }

    // ---------- Image registry (debug session cargo-pmcp-deploy-targets, #7) ----------

    use super::super::fake_cloud::{self, FakeCloud, Reply};

    fn tools(fake: &FakeCloud) -> CloudTools {
        CloudTools {
            docker: fake.docker().to_path_buf(),
            gcloud: fake.gcloud().to_path_buf(),
        }
    }

    /// A deployable project: a crate at `root/app` and the config for it.
    fn deployable(root: &std::path::Path, repository: Option<&str>) -> DeployConfig {
        let app = root.join("app");
        fixture::write_crate(&app, "svc", &["serve"], "");
        let mut config = DeployConfig::default_for_cloud_run_server(
            "svc".to_string(),
            "acme-prod".to_string(),
            "us-central1".to_string(),
            app,
        );
        config.gcp.as_mut().expect("[gcp]").repository = repository.map(str::to_string);
        config
    }

    fn standard_replies() -> Vec<Reply> {
        vec![fake_cloud::ACTIVE_ACCOUNT, fake_cloud::DESCRIBE_URL]
    }

    /// The `gcloud run deploy` line, which only differs in the image.
    fn run_deploy_line(image: &str) -> String {
        format!(
            "gcloud run deploy svc --image {image} --region us-central1 --project acme-prod \
             --platform managed --memory 256Mi --cpu 1 --max-instances 10 --min-instances 0 \
             --port 8080 --quiet --ingress all --set-env-vars RUST_LOG=info --allow-unauthenticated"
        )
    }

    /// A deploy.toml with `[gcp] repository` (what a 0.28.0 `deploy init`
    /// writes): the repository is created, docker authenticates to the
    /// regional host, and the image is pushed and deployed from there.
    #[cfg(unix)]
    #[test]
    fn artifact_registry_deploy_creates_the_repository_and_pushes_there() {
        let tmp = tempfile::tempdir().expect("tmp");
        let fake = FakeCloud::new(&tmp.path().join("bin"), &standard_replies());
        let config = deployable(tmp.path(), Some("pmcp"));

        let outputs = deploy_with(&config, &tools(&fake)).expect("deploys");

        let image = "us-central1-docker.pkg.dev/acme-prod/pmcp/svc:latest";
        assert_eq!(
            fake.calls(),
            [
                "gcloud auth list --filter=status:ACTIVE --format=value(account)".to_string(),
                "gcloud artifacts repositories create pmcp --repository-format=docker \
                 --location=us-central1 --project=acme-prod \
                 --description=MCP server images deployed by cargo pmcp --quiet"
                    .to_string(),
                format!("docker buildx build --platform linux/amd64 -t {image} --load ."),
                "gcloud auth configure-docker us-central1-docker.pkg.dev --quiet".to_string(),
                format!("docker push {image}"),
                run_deploy_line(image),
                "gcloud run services describe svc --region us-central1 --project acme-prod \
                 --format value(status.url)"
                    .to_string(),
            ]
        );
        assert_eq!(outputs.url.as_deref(), Some(fake_cloud::SERVICE_URL));
        assert_eq!(
            outputs.custom.get("image"),
            Some(&serde_json::Value::String(image.to_string()))
        );
    }

    /// Re-deploys find the repository already there: "already exists" is
    /// success, not an error.
    #[cfg(unix)]
    #[test]
    fn an_existing_repository_is_not_an_error() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut replies = standard_replies();
        replies.push(Reply {
            program: "gcloud",
            prefix: "artifacts repositories create",
            stdout: "",
            stderr: "ERROR: (gcloud.artifacts.repositories.create) ALREADY_EXISTS: the repository already exists\n",
            exit: 1,
        });
        let fake = FakeCloud::new(&tmp.path().join("bin"), &replies);
        let config = deployable(tmp.path(), Some("pmcp"));

        deploy_with(&config, &tools(&fake)).expect("an existing repository is fine");
        assert!(fake.ran("docker push us-central1-docker.pkg.dev/acme-prod/pmcp/svc:latest"));
    }

    /// Any other create failure (permissions) stops the deploy before the
    /// image is built, naming the repository and the command.
    #[cfg(unix)]
    #[test]
    fn a_repository_that_cannot_be_created_stops_the_deploy_before_the_build() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut replies = standard_replies();
        replies.push(Reply {
            program: "gcloud",
            prefix: "artifacts repositories create",
            stdout: "",
            stderr: "ERROR: (gcloud.artifacts.repositories.create) PERMISSION_DENIED: denied\n",
            exit: 1,
        });
        let fake = FakeCloud::new(&tmp.path().join("bin"), &replies);
        let config = deployable(tmp.path(), Some("pmcp"));

        let message = format!(
            "{:#}",
            deploy_with(&config, &tools(&fake)).expect_err("cannot create")
        );
        assert!(message.contains("PERMISSION_DENIED"), "{message}");
        assert!(
            message.contains("us-central1-docker.pkg.dev/acme-prod/pmcp"),
            "{message}"
        );
        assert!(
            !fake.ran("docker"),
            "nothing may be built: {:?}",
            fake.calls()
        );
    }

    /// `[gcp] repository` is checked before any gcloud write or build.
    #[cfg(unix)]
    #[test]
    fn an_invalid_repository_is_refused_before_anything_runs_but_auth() {
        let tmp = tempfile::tempdir().expect("tmp");
        let fake = FakeCloud::new(&tmp.path().join("bin"), &standard_replies());
        let config = deployable(tmp.path(), Some("Bad_Repo"));

        let message = format!(
            "{:#}",
            deploy_with(&config, &tools(&fake)).expect_err("bad")
        );
        assert!(message.contains("\"Bad_Repo\""), "{message}");
        assert_eq!(
            fake.calls(),
            ["gcloud auth list --filter=status:ACTIVE --format=value(account)"]
        );
    }

    /// A deploy.toml from before 0.28.0 (no `[gcp] repository`) keeps the
    /// exact gcr.io argv it always ran.
    #[cfg(unix)]
    #[test]
    fn a_deploy_toml_without_repository_keeps_the_gcr_io_argv() {
        let tmp = tempfile::tempdir().expect("tmp");
        let fake = FakeCloud::new(&tmp.path().join("bin"), &standard_replies());
        let config = deployable(tmp.path(), None);

        let outputs = deploy_with(&config, &tools(&fake)).expect("deploys");

        let image = "gcr.io/acme-prod/svc:latest";
        assert_eq!(
            fake.calls(),
            [
                "gcloud auth list --filter=status:ACTIVE --format=value(account)".to_string(),
                format!("docker buildx build --platform linux/amd64 -t {image} --load ."),
                "gcloud auth configure-docker --quiet".to_string(),
                format!("docker push {image}"),
                run_deploy_line(image),
                "gcloud run services describe svc --region us-central1 --project acme-prod \
                 --format value(status.url)"
                    .to_string(),
            ]
        );
        assert_eq!(outputs.url.as_deref(), Some(fake_cloud::SERVICE_URL));
    }
}
