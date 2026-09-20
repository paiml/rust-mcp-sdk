use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[allow(dead_code)]
pub struct BinaryBuilder {
    project_root: PathBuf,
    oauth_enabled: bool,
    /// Whether to build local OAuth lambdas (false for pmcp-run which uses shared OAuth)
    build_oauth_lambdas: bool,
}

#[allow(dead_code)]
pub struct BuildResult {
    pub binary_path: PathBuf,
    pub binary_size: u64,
    pub oauth_proxy_path: Option<PathBuf>,
    pub authorizer_path: Option<PathBuf>,
    /// Path to deployment package (zip) if assets were bundled
    pub deployment_package: Option<PathBuf>,
}

/// Where `deploy` writes the Lambda bootstrap it uploads, under `project_root`.
///
/// THE one definition of that path. `cargo pmcp package save` defaults to it
/// (its `--binary-from`/`--binary` inputs), so producer and consumer must agree
/// — and the failure when they do not is silent, not loud: a STALE bootstrap
/// left behind by a path change is still found, still hashed, and still packed,
/// yielding a package that pins bytes the deployed server does not run. That is
/// an integrity failure in a format whose whole purpose is byte-accurate
/// identity, and no error is raised anywhere along the way.
///
/// A shared function rather than a drift test on purpose: the authority here is
/// Rust in the same binary, so the compiler can enforce it. This repo's
/// drift-guard idiom (`templates/workbook_server.rs`'s `PMCP_VERSION`) is for
/// authorities Rust cannot import — an external `Cargo.toml` — and is a
/// fallback for uncrossable boundaries, not the default.
pub fn bootstrap_path(project_root: &Path) -> PathBuf {
    project_root.join(BOOTSTRAP_RELATIVE)
}

/// [`bootstrap_path`]'s project-relative form, for messages that name it.
pub const BOOTSTRAP_RELATIVE: &str = "deploy/.build/bootstrap";

impl BinaryBuilder {
    pub fn new(project_root: PathBuf) -> Self {
        // Check if OAuth is enabled in config and if we should build local OAuth lambdas
        let config = crate::deployment::config::DeployConfig::load(&project_root);
        let oauth_enabled = config.as_ref().is_ok_and(|c| c.auth.enabled);

        // For pmcp-run target, OAuth lambdas are shared on the service side
        // Only build local OAuth lambdas for aws-lambda target
        let build_oauth_lambdas = config
            .as_ref()
            .is_ok_and(|c| c.auth.enabled && c.target.target_type == "aws-lambda");

        Self {
            project_root,
            oauth_enabled,
            build_oauth_lambdas,
        }
    }

    pub fn build(&self) -> Result<BuildResult> {
        println!("🔨 Building Rust binary for AWS Lambda...");

        // 1. Check for cargo-lambda
        self.ensure_cargo_lambda()?;

        // 2. Build release binary with cargo-lambda
        self.build_lambda_binary()?;

        // 3. Copy to deploy/.build/bootstrap
        let binary_path = self.copy_to_bootstrap()?;

        let binary_size = std::fs::metadata(&binary_path)
            .context("Failed to get binary size")?
            .len();

        println!(
            "✅ Binary built: {:.2} MB",
            binary_size as f64 / 1_048_576.0
        );

        // 4. Build OAuth Lambdas if enabled AND target requires local OAuth lambdas
        // (pmcp-run uses shared OAuth infrastructure, so we skip building local OAuth lambdas)
        let (oauth_proxy_path, authorizer_path) = if self.build_oauth_lambdas {
            println!("🔐 Building OAuth Lambdas...");
            let proxy_path = self.build_and_copy_oauth_lambda("oauth-proxy")?;
            let authorizer_path = self.build_and_copy_oauth_lambda("authorizer")?;
            (Some(proxy_path), Some(authorizer_path))
        } else {
            (None, None)
        };

        // 5. Bundle assets and create deployment package if assets are configured
        let deployment_package = self.bundle_assets_if_configured(&binary_path)?;

        Ok(BuildResult {
            binary_path,
            binary_size,
            oauth_proxy_path,
            authorizer_path,
            deployment_package,
        })
    }

    fn ensure_cargo_lambda(&self) -> Result<()> {
        print!("   Checking cargo-lambda...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let output = Command::new("cargo")
            .args(&["lambda", "--version"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                println!(" ✅");
                Ok(())
            },
            _ => {
                println!(" ❌");
                println!();
                println!("cargo-lambda is required for building Lambda binaries.");
                println!("Install with:");
                println!("  cargo install cargo-lambda");
                println!();
                bail!("cargo-lambda not installed");
            },
        }
    }

    fn build_lambda_binary(&self) -> Result<()> {
        print!("   Building Lambda binary (this may take a few minutes)...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Load config to get server name
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;

        // Use cargo lambda build with --arm64 for cross-compilation.
        // ARM64 is cheaper and faster on Lambda than x86_64.
        //
        // We cd into the lambda package directory and run without --package/--bin
        // to ensure cargo-lambda configures Zig wrappers correctly for build scripts.
        //
        // Three env vars fix Zig 0.15+ cross-compilation of C dependencies:
        // - AWS_LC_SYS_CMAKE_BUILDER=1: aws-lc-sys uses cmake (not cc crate)
        // - AWS_LC_SYS_NO_JITTER_ENTROPY=1: skip jitterentropy (Zig rejects -U flag)
        // - CC_aarch64_unknown_linux_gnu=zigcc wrapper: ring's cc crate uses the
        //   wrapper which handles the --target triple Zig can't parse
        let lambda_pkg_dir = self.find_lambda_package_dir(&config.server.name)?;

        let status = Command::new("cargo")
            .args(["lambda", "build", "--release", "--arm64"])
            .current_dir(&lambda_pkg_dir)
            // Force aws-lc-sys to use cmake builder (bypasses cc crate target conflict)
            .env("AWS_LC_SYS_CMAKE_BUILDER", "1")
            // Disable jitter entropy: Zig rejects -U_FORTIFY_SOURCE preprocessor flag
            // that cmake adds. Lambda uses OS-provided entropy, not CPU jitter.
            .env("AWS_LC_SYS_NO_JITTER_ENTROPY", "1")
            // Set CC to cargo-lambda's zigcc wrapper for ring and other cc-crate
            // based builds. The cc crate appends --target=aarch64-unknown-linux-gnu
            // which bare zig rejects (UnknownOperatingSystem), but the zigcc wrapper
            // handles it by routing through `cargo-lambda zig cc` first.
            .envs(Self::find_zigcc_env_vars())
            .status()
            .context("Failed to run cargo lambda build")?;

        if !status.success() {
            println!(" ❌");
            bail!("Failed to build Lambda binary");
        }

        println!(" ✅");
        Ok(())
    }

    fn copy_to_bootstrap(&self) -> Result<PathBuf> {
        print!("   Copying to Lambda bootstrap...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Get package name from config
        let package_name = self.get_package_name()?;

        // Resolve the target directory: respect CARGO_TARGET_DIR, .cargo/config.toml,
        // and workspace target-dir settings. Falls back to {project_root}/target.
        let target_dir = self.resolve_target_dir();

        // cargo-lambda outputs to {target_dir}/lambda/{binary-name}/bootstrap
        let src = target_dir.join(format!("lambda/{}/bootstrap", package_name));

        if !src.exists() {
            println!(" ❌");
            bail!(
                "Binary not found at: {}\n\
                 Hint: If using a shared target directory (CARGO_TARGET_DIR or \
                 [build] target-dir in .cargo/config.toml), the binary will be \
                 in that directory, not in the project root.",
                src.display()
            );
        }

        // Destination path for CDK
        let dst = bootstrap_path(&self.project_root);
        let deploy_build_dir = dst.parent().expect("bootstrap_path always has a parent");
        std::fs::create_dir_all(deploy_build_dir)
            .context("Failed to create deploy/.build directory")?;

        // Copy binary
        std::fs::copy(&src, &dst).context("Failed to copy binary to deploy/.build/bootstrap")?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dst, perms)?;
        }

        println!(" ✅");
        Ok(dst)
    }

    fn get_package_name(&self) -> Result<String> {
        // cargo-lambda outputs to target/lambda/{binary-name}/bootstrap
        // Since AWS Lambda requires binary name "bootstrap", the output is in target/lambda/bootstrap/
        Ok("bootstrap".to_string())
    }

    /// Resolve the effective target directory.
    ///
    /// Checks (in priority order):
    /// 1. `CARGO_TARGET_DIR` env var
    /// 2. `CARGO_BUILD_TARGET_DIR` env var
    /// 3. Workspace root's target/ (walks up to 5 ancestors looking for Cargo.toml with [workspace])
    /// 4. `{project_root}/target`
    fn resolve_target_dir(&self) -> PathBuf {
        // Env vars take priority (matches Cargo's own resolution)
        if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
            return PathBuf::from(dir);
        }
        if let Ok(dir) = std::env::var("CARGO_BUILD_TARGET_DIR") {
            return PathBuf::from(dir);
        }

        // Walk up to find workspace root (Cargo.toml with [workspace])
        let mut dir = self.project_root.clone();
        for _ in 0..5 {
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[workspace]") {
                        return dir.join("target");
                    }
                }
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Fallback: project root
        self.project_root.join("target")
    }

    /// Find the Lambda wrapper package in the workspace.
    /// First tries {server_name}-lambda, then looks for any *-lambda package with bootstrap binary.
    /// Find the directory of the Lambda package for cd-based builds.
    ///
    /// Returns the absolute path to the Lambda package directory.
    /// Find cargo-zigbuild's zigcc wrapper script and return CC/AR env vars.
    ///
    /// cargo-lambda caches zigcc wrappers in ~/Library/Caches/cargo-zigbuild/
    /// (macOS) or ~/.cache/cargo-zigbuild/ (Linux). The wrapper correctly
    /// handles the cc crate's --target flag that bare zig rejects.
    fn find_zigcc_env_vars() -> Vec<(String, String)> {
        let mut vars = Vec::new();

        // dirs::cache_dir() resolves to ~/Library/Caches (macOS) or ~/.cache (Linux)
        let cache_dirs: Vec<PathBuf> = dirs::cache_dir()
            .map(|d| vec![d.join("cargo-zigbuild")])
            .unwrap_or_default();

        for cache_base in &cache_dirs {
            if !cache_base.exists() {
                continue;
            }
            // Find the most recent version directory
            if let Ok(entries) = std::fs::read_dir(cache_base) {
                let mut versions: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .collect();
                versions.sort_by_key(|e| e.file_name());

                if let Some(version_dir) = versions.last() {
                    let dir = version_dir.path();
                    // Find zigcc-aarch64-unknown-linux-gnu-*.sh
                    if let Ok(files) = std::fs::read_dir(&dir) {
                        for file in files.filter_map(|f| f.ok()) {
                            let name = file.file_name().to_string_lossy().to_string();
                            if name.starts_with("zigcc-aarch64-unknown-linux-gnu-")
                                && name.ends_with(".sh")
                            {
                                vars.push((
                                    "CC_aarch64_unknown_linux_gnu".to_string(),
                                    file.path().to_string_lossy().to_string(),
                                ));
                            }
                        }
                    }
                    // AR is just the plain 'ar' in the same directory
                    let ar = dir.join("ar");
                    if ar.exists() {
                        vars.push((
                            "AR_aarch64_unknown_linux_gnu".to_string(),
                            ar.to_string_lossy().to_string(),
                        ));
                    }
                }
            }
        }

        vars
    }

    fn find_lambda_package_dir(&self, server_name: &str) -> Result<PathBuf> {
        let preferred_package = format!("{}-lambda", server_name);

        // Check if the preferred package exists as a direct subdirectory
        let preferred_dir = self.project_root.join(&preferred_package);
        if preferred_dir.exists() && preferred_dir.join("Cargo.toml").exists() {
            return Ok(preferred_dir);
        }

        // Search workspace members for a *-lambda package exposing a `bootstrap`
        // binary. A metadata error here (e.g. a single-crate config-driven project
        // whose manifest does not resolve as a workspace) is NOT fatal: we fall
        // through to the single-crate fallback below rather than bailing early (H3).
        if let Some(pkg_dir) = self.find_workspace_lambda_package_dir() {
            return Ok(pkg_dir);
        }

        // H3 (Phase 86 Plan 05): single-crate config-driven fallback.
        //
        // A config-driven project (`cargo pmcp new --kind sql-server`) is a SINGLE
        // crate where the project ROOT *is* the Lambda package — there is no
        // `<server>-lambda` subdir and no `*-lambda` workspace member. When the root
        // is such a project, return the project root itself so `cargo lambda build`
        // runs in the crate that defines the deployable binary. This is an ADDITIVE
        // final fallback: the two resolution paths above are always tried first, so
        // existing `*-lambda` layouts are unaffected (no regression).
        if self.is_single_crate_config_root() {
            return Ok(self.project_root.clone());
        }

        bail!(
            "No Lambda wrapper package found. Expected '{}' or any '*-lambda' package with 'bootstrap' binary.\n\
             Run 'cargo pmcp deploy init --target pmcp-run' to create one.",
            preferred_package
        );
    }

    /// Search workspace members for a `*-lambda` package that exposes a `bootstrap`
    /// binary and return its package directory.
    ///
    /// Returns `None` (rather than an error) when workspace metadata cannot be read
    /// or no such package exists, so the caller can fall through to the single-crate
    /// fallback (H3). Extracted from `find_lambda_package_dir` to keep that fn's
    /// cognitive complexity low.
    fn find_workspace_lambda_package_dir(&self) -> Option<PathBuf> {
        let binaries =
            crate::deployment::naming::detect_workspace_binaries(&self.project_root).ok()?;

        let pkg_name = binaries.iter().find_map(|binary| {
            (binary.binary_name == "bootstrap" && binary.package_name.ends_with("-lambda"))
                .then(|| binary.package_name.clone())
        })?;

        let metadata = cargo_metadata::MetadataCommand::new()
            .current_dir(&self.project_root)
            .exec()
            .ok()?;

        metadata
            .packages
            .iter()
            .find(|p| p.name == pkg_name)
            .and_then(|pkg| pkg.manifest_path.parent().map(|p| p.into()))
    }

    /// Whether the project root is a single-crate config-driven project whose root
    /// crate IS the Lambda package (H3).
    ///
    /// True when the root `Cargo.toml` declares a `[package]` (i.e. is NOT a virtual
    /// workspace manifest) AND both `config.toml` and `schema.sql` are present at the
    /// root — the same config-driven heuristic the deploy seam uses (D-09). Kept tiny
    /// so `find_lambda_package_dir` stays well under the cog-25 budget.
    fn is_single_crate_config_root(&self) -> bool {
        let cargo_toml = self.project_root.join("Cargo.toml");
        let is_package_crate = std::fs::read_to_string(&cargo_toml)
            .map(|c| c.contains("[package]"))
            .unwrap_or(false);
        is_package_crate
            && self.project_root.join("config.toml").exists()
            && self.project_root.join("schema.sql").exists()
    }

    fn find_lambda_package(&self, server_name: &str) -> Result<String> {
        let preferred_package = format!("{}-lambda", server_name);

        // Check if the preferred package exists
        let preferred_dir = self.project_root.join(&preferred_package);
        if preferred_dir.exists() {
            return Ok(preferred_package);
        }

        // Look for any *-lambda package with bootstrap binary in the workspace
        let binaries = crate::deployment::naming::detect_workspace_binaries(&self.project_root)?;

        for binary in binaries {
            if binary.binary_name == "bootstrap" && binary.package_name.ends_with("-lambda") {
                println!();
                println!(
                    "   ℹ️  Using existing Lambda wrapper: {}",
                    binary.package_name
                );
                return Ok(binary.package_name);
            }
        }

        // No Lambda wrapper found
        bail!(
            "No Lambda wrapper package found. Expected '{}' or any '*-lambda' package with 'bootstrap' binary.\n\
             Run 'cargo pmcp deploy init --target pmcp-run' to create one.",
            preferred_package
        );
    }

    /// Build and copy an OAuth Lambda (proxy or authorizer)
    fn build_and_copy_oauth_lambda(&self, lambda_type: &str) -> Result<PathBuf> {
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        let package_name = format!("{}-{}", config.server.name, lambda_type);
        let output_dir = format!(".build-{}", lambda_type);

        print!("   Building {} Lambda...", lambda_type);
        std::io::Write::flush(&mut std::io::stdout())?;

        // Build with cargo-lambda
        let status = Command::new("cargo")
            .args([
                "lambda",
                "build",
                "--release",
                "--package",
                &package_name,
                "--output-format",
                "binary",
                "--target",
                "aarch64-unknown-linux-gnu",
            ])
            .current_dir(&self.project_root)
            .status()
            .context(format!(
                "Failed to run cargo lambda build for {}",
                lambda_type
            ))?;

        if !status.success() {
            println!(" ❌");
            bail!("Failed to build {} Lambda binary", lambda_type);
        }

        // Copy to deploy/{output_dir}/bootstrap
        let target_dir = self.resolve_target_dir();
        let src = target_dir.join("lambda/bootstrap/bootstrap");

        let src = if src.exists() {
            src
        } else {
            let alt_src = target_dir.join(format!("lambda/{}/bootstrap", package_name));
            if alt_src.exists() {
                alt_src
            } else {
                println!(" ❌");
                bail!(
                    "{} binary not found at {} or {}",
                    lambda_type,
                    src.display(),
                    alt_src.display()
                );
            }
        };

        let deploy_build_dir = self.project_root.join(format!("deploy/{}", output_dir));
        std::fs::create_dir_all(&deploy_build_dir)
            .context(format!("Failed to create deploy/{} directory", output_dir))?;

        let dst = deploy_build_dir.join("bootstrap");
        std::fs::copy(&src, &dst).context(format!(
            "Failed to copy {} binary to deploy/{}/bootstrap",
            lambda_type, output_dir
        ))?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dst, perms)?;
        }

        println!(" ✅");
        Ok(dst)
    }

    /// Bundle assets and create a deployment package (zip) if assets are configured.
    ///
    /// Returns the path to the zip file if assets were bundled, None otherwise.
    fn bundle_assets_if_configured(&self, binary_path: &Path) -> Result<Option<PathBuf>> {
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;

        // Resolve asset files (empty vec if no assets configured)
        let asset_files = if config.assets.has_assets() {
            config.assets.resolve_files(&self.project_root)?
        } else {
            Vec::new()
        };

        // Resolve config.toml once (for Code Mode operation extraction)
        let config_toml_path = self.resolve_config_toml();

        // Only create ZIP if there's something to bundle beyond bootstrap
        if asset_files.is_empty() && config_toml_path.is_none() {
            return Ok(None);
        }

        println!("📦 Bundling deployment package...");
        if !asset_files.is_empty() {
            println!("   Found {} asset file(s)", asset_files.len());
        }

        // Create deployment package directory
        let package_dir = self.project_root.join("deploy/.build");
        std::fs::create_dir_all(&package_dir)
            .context("Failed to create deployment package directory")?;

        // Create zip file
        let zip_path = package_dir.join("deployment.zip");
        let zip_file =
            std::fs::File::create(&zip_path).context("Failed to create deployment.zip")?;
        let mut zip = ZipWriter::new(zip_file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // Add bootstrap binary
        print!("   Adding bootstrap binary...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let bootstrap_data =
            std::fs::read(binary_path).context("Failed to read bootstrap binary")?;
        zip.start_file("bootstrap", options)
            .context("Failed to add bootstrap to zip")?;
        zip.write_all(&bootstrap_data)
            .context("Failed to write bootstrap to zip")?;
        println!(" ✅");

        let file_options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // H4 (Phase 86 Plan 05): for a config-driven project, the BUNDLED config's
        // inline DEV `token_secret` MUST be replaced with the `${CODE_MODE_SECRET}`
        // env ref so the deployed artifact never ships the dev literal. The on-disk
        // local config.toml is untouched (D-06 out-of-box `cargo run`).
        let config_driven = crate::commands::deploy::is_config_driven_project(&self.project_root);

        // Add config.toml if found (for Code Mode operation extraction by pmcp.run)
        let config_toml_included = self.add_config_toml_to_zip(
            &mut zip,
            file_options,
            config_toml_path.as_deref(),
            config_driven,
        )?;

        // Add assets to assets/ subdirectory in the zip
        // Lambda will extract to $LAMBDA_TASK_ROOT/assets/
        let base_dir = config
            .assets
            .base_dir
            .as_ref()
            .map(|d| self.project_root.join(d))
            .unwrap_or_else(|| self.project_root.clone());

        for asset_path in &asset_files {
            // Get relative path from base directory
            let relative_path = pathdiff::diff_paths(asset_path, &base_dir)
                .unwrap_or_else(|| asset_path.file_name().unwrap().into());

            // Put assets in assets/ subdirectory
            let zip_path = format!("assets/{}", relative_path.display());

            print!("   Adding {}...", relative_path.display());
            std::io::Write::flush(&mut std::io::stdout())?;

            let asset_data = std::fs::read(asset_path)
                .context(format!("Failed to read {}", asset_path.display()))?;
            // H4: this `assets/config.toml` copy is the one the runtime reads via
            // `pmcp::assets::load_string("config.toml")` (/var/task/assets/) — so it
            // MUST carry the env-ref secret, never the inline dev literal.
            let asset_data = if config_driven
                && asset_path.file_name().and_then(|n| n.to_str()) == Some("config.toml")
            {
                Self::sanitize_config_bytes_for_deploy(&asset_data)
            } else {
                asset_data
            };
            zip.start_file(&zip_path, file_options)
                .context(format!("Failed to add {} to zip", zip_path))?;
            zip.write_all(&asset_data)
                .context(format!("Failed to write {} to zip", zip_path))?;

            println!(" ✅");
        }

        zip.finish().context("Failed to finalize zip file")?;

        let zip_size = std::fs::metadata(&zip_path)
            .context("Failed to get zip size")?
            .len();

        let extra_files = 1 + if config_toml_included { 1 } else { 0 }; // bootstrap + optional config.toml
        println!(
            "✅ Deployment package created: {:.2} MB ({} files)",
            zip_size as f64 / 1_048_576.0,
            asset_files.len() + extra_files
        );

        Ok(Some(zip_path))
    }

    /// Add config.toml to the deploy ZIP for Code Mode operation extraction.
    ///
    /// Takes a pre-resolved config path (from `resolve_config_toml`) to avoid
    /// redundant filesystem scanning. Returns true if a config file was added.
    ///
    /// When `config_driven` is true the bundled bytes are passed through
    /// [`Self::sanitize_config_bytes_for_deploy`] so the inline DEV `token_secret`
    /// is replaced with the `${CODE_MODE_SECRET}` env ref (H4). The on-disk source
    /// file is never modified.
    fn add_config_toml_to_zip<W: std::io::Write + std::io::Seek>(
        &self,
        zip: &mut ZipWriter<W>,
        options: SimpleFileOptions,
        config_path: Option<&Path>,
        config_driven: bool,
    ) -> Result<bool> {
        let config_path = match config_path {
            Some(path) => path,
            None => return Ok(false),
        };

        print!("   Adding config.toml...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let config_data = std::fs::read(config_path).context("Failed to read config.toml")?;
        let config_data = if config_driven {
            Self::sanitize_config_bytes_for_deploy(&config_data)
        } else {
            config_data
        };
        zip.start_file("config.toml", options)
            .context("Failed to add config.toml to zip")?;
        zip.write_all(&config_data)
            .context("Failed to write config.toml to zip")?;
        println!(" ✅ ({})", config_path.display());
        Ok(true)
    }

    /// Rewrite a bundled `config.toml`'s inline DEV `token_secret` to the
    /// `${CODE_MODE_SECRET}` env ref (H4, Phase 86 Plan 05).
    ///
    /// Replaces any `token_secret = "..."` assignment with
    /// `token_secret = "${CODE_MODE_SECRET}"` (env-expanded by the toolkit config
    /// parser at runtime) so the DEPLOYED artifact never carries the inline dev
    /// literal. Operates on bytes-in / bytes-out; if the input is not valid UTF-8
    /// or has no `token_secret` line, the bytes are returned unchanged. The local
    /// on-disk config is untouched — only the bundled copy is rewritten (D-06).
    fn sanitize_config_bytes_for_deploy(bytes: &[u8]) -> Vec<u8> {
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => return bytes.to_vec(),
        };
        let rewritten: String = text
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("token_secret") && line.contains('=') {
                    let indent_len = line.len() - line.trim_start().len();
                    format!(
                        "{}token_secret = \"${{CODE_MODE_SECRET}}\"",
                        &line[..indent_len]
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Preserve a trailing newline if the original had one.
        let mut out = rewritten;
        if text.ends_with('\n') {
            out.push('\n');
        }
        out.into_bytes()
    }

    /// Resolve the server's config.toml path.
    ///
    /// Resolution order (same as Rust servers using `include_str!()`):
    /// 1. `config.toml` in the server crate root
    /// 2. Single file in `instances/*.toml`
    fn resolve_config_toml(&self) -> Option<PathBuf> {
        let direct = self.project_root.join("config.toml");
        if direct.exists() {
            return Some(direct);
        }
        self.find_single_instance_toml()
    }

    /// Find a single .toml file in the `instances/` directory.
    fn find_single_instance_toml(&self) -> Option<PathBuf> {
        let instances_dir = self.project_root.join("instances");
        if !instances_dir.is_dir() {
            return None;
        }
        let toml_files: Vec<_> = std::fs::read_dir(&instances_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
            })
            .collect();
        if toml_files.len() == 1 {
            Some(toml_files[0].path())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct a `BinaryBuilder` for a given root WITHOUT touching the filesystem
    /// for config (the `new()` constructor calls `DeployConfig::load`, which requires
    /// `.pmcp/deploy.toml`). Tests only exercise path resolution, so we build the
    /// struct directly (same-module private-field access).
    fn builder_for(root: PathBuf) -> BinaryBuilder {
        BinaryBuilder {
            project_root: root,
            oauth_enabled: false,
            build_oauth_lambdas: false,
        }
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    /// (ii) H3: a single-crate config-driven root (root `[package]` + config.toml +
    /// schema.sql, no `*-lambda` package) resolves to the project root itself.
    #[test]
    fn find_lambda_package_dir_resolves_single_crate_config_root() {
        let tmp = std::env::temp_dir().join(format!("pmcp-h3-single-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        write(
            &tmp.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        write(&tmp.join("config.toml"), "[server]\nname = \"demo\"\n");
        write(
            &tmp.join("schema.sql"),
            "CREATE TABLE IF NOT EXISTS t(id INTEGER);\n",
        );

        let b = builder_for(tmp.clone());
        assert!(b.is_single_crate_config_root());
        let resolved = b.find_lambda_package_dir("demo").unwrap();
        assert_eq!(
            resolved, tmp,
            "single-crate config root must resolve to the project root (H3)"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// (i) No regression: a `<server>-lambda` subdir with a Cargo.toml still resolves
    /// FIRST (before the single-crate fallback is even considered).
    #[test]
    fn find_lambda_package_dir_prefers_server_lambda_subdir() {
        let tmp = std::env::temp_dir().join(format!("pmcp-h3-wrapper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // Root is also a single-crate config root, to prove the `*-lambda` path wins.
        write(
            &tmp.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        write(&tmp.join("config.toml"), "[server]\nname = \"demo\"\n");
        write(
            &tmp.join("schema.sql"),
            "CREATE TABLE IF NOT EXISTS t(id INTEGER);\n",
        );
        let wrapper = tmp.join("demo-lambda");
        write(
            &wrapper.join("Cargo.toml"),
            "[package]\nname = \"demo-lambda\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );

        let b = builder_for(tmp.clone());
        let resolved = b.find_lambda_package_dir("demo").unwrap();
        assert_eq!(
            resolved, wrapper,
            "the <server>-lambda subdir must resolve first (no regression)"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// (iii) A bare root (no `*-lambda`, no config.toml/schema.sql) still bails.
    #[test]
    fn find_lambda_package_dir_bails_on_bare_root() {
        let tmp = std::env::temp_dir().join(format!("pmcp-h3-bare-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        write(
            &tmp.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );

        let b = builder_for(tmp.clone());
        assert!(!b.is_single_crate_config_root());
        assert!(
            b.find_lambda_package_dir("demo").is_err(),
            "a bare root (no *-lambda, no config-driven markers) must bail"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// H4 (Phase 86 Plan 05) — NON-CLOUD packaging + secret-posture test.
    ///
    /// Drives `bundle_assets_if_configured` against a scaffold-shaped tempdir
    /// (config.toml + schema.sql + `.pmcp/deploy.toml` with the `[assets]` include)
    /// using a DUMMY bootstrap file (only paths/contents are inspected — no real
    /// deploy, no cloud), then inspects the produced `deployment.zip`:
    ///
    /// - it contains `assets/config.toml` and `assets/schema.sql` (H1 — where
    ///   `pmcp::assets` reads at `/var/task/assets/`);
    /// - the BUNDLED config bytes (both the zip-root `config.toml` and
    ///   `assets/config.toml`) do NOT contain the inline DEV secret literal and DO
    ///   contain `${CODE_MODE_SECRET}` (H4 — the deployed artifact never ships the
    ///   dev secret).
    ///
    /// Lives in-module because `bundle_assets_if_configured` is a private method
    /// (the plan's fallback when the bundler is unreachable from an integration
    /// test, mirroring the M3 in-module approach).
    #[test]
    fn bundled_artifact_paths_and_secret_posture() {
        let tmp = std::env::temp_dir().join(format!("pmcp-h4-bundle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        // Scaffold-shaped single-crate config-driven project.
        write(
            &tmp.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\npmcp-server-toolkit = \"0.1.0\"\n",
        );
        write(
            &tmp.join("config.toml"),
            "[code_mode]\nenabled = true\n\
             token_secret = \"dev-only-insecure-secret-min-16-bytes\"\n\
             allow_inline_token_secret_for_dev = true\n",
        );
        write(
            &tmp.join("schema.sql"),
            "CREATE TABLE IF NOT EXISTS books(id INTEGER);\n",
        );
        // Minimal parseable deploy descriptor with the [assets] include list.
        write(
            &tmp.join(".pmcp/deploy.toml"),
            "[target]\ntype = \"pmcp-run\"\nversion = \"1.0.0\"\n\n\
             [aws]\nregion = \"us-east-1\"\n\n\
             [server]\nname = \"demo\"\nmemory_mb = 512\ntimeout_seconds = 30\n\n\
             [environment]\nRUST_LOG = \"info\"\n\n\
             [auth]\nenabled = false\nprovider = \"none\"\n\n\
             [observability]\nlog_retention_days = 30\nenable_xray = false\ncreate_dashboard = false\n\n\
             [assets]\ninclude = [\"config.toml\", \"schema.sql\"]\n",
        );
        // Dummy bootstrap (only its bytes are copied into the zip).
        let dummy_bootstrap = tmp.join("dummy-bootstrap");
        write(&dummy_bootstrap, "ELF-PLACEHOLDER");

        let b = builder_for(tmp.clone());
        // Sanity: the project is detected as config-driven (drives the H4 rewrite).
        assert!(crate::commands::deploy::is_config_driven_project(&tmp));

        let zip_path = b
            .bundle_assets_if_configured(&dummy_bootstrap)
            .expect("bundling must succeed")
            .expect("a deployment.zip must be produced (assets are configured)");

        // Read the zip and collect entry name -> contents.
        let file = std::fs::File::open(&zip_path).expect("open deployment.zip");
        let mut archive = zip::ZipArchive::new(file).expect("parse deployment.zip");
        let mut entries: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            let mut buf = String::new();
            // bootstrap is binary; ignore read errors for non-UTF8 entries.
            use std::io::Read;
            if entry.read_to_string(&mut buf).is_ok() {
                entries.insert(name, buf);
            } else {
                entries.insert(name, String::new());
            }
        }

        // H1: assets land under assets/ where pmcp::assets reads (/var/task/assets/).
        assert!(
            entries.contains_key("assets/config.toml"),
            "zip must contain assets/config.toml (H1); entries: {:?}",
            entries.keys().collect::<Vec<_>>()
        );
        assert!(
            entries.contains_key("assets/schema.sql"),
            "zip must contain assets/schema.sql (H1); entries: {:?}",
            entries.keys().collect::<Vec<_>>()
        );

        // H4: NEITHER bundled config copy may carry the inline dev secret literal,
        // and BOTH must carry the env ref.
        for key in ["config.toml", "assets/config.toml"] {
            let body = entries
                .get(key)
                .unwrap_or_else(|| panic!("zip must contain {key}"));
            assert!(
                !body.contains("dev-only-insecure-secret-min-16-bytes"),
                "H4: bundled {key} must NOT contain the inline dev secret literal"
            );
            assert!(
                body.contains("${CODE_MODE_SECRET}"),
                "H4: bundled {key} must reference ${{CODE_MODE_SECRET}}"
            );
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
