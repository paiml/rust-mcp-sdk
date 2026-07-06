use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

pub struct DeployExecutor {
    project_root: PathBuf,
    /// Transient env vars (e.g., resolved secrets) passed to the CDK process.
    /// These are NEVER written to deploy.toml -- they exist only as process env
    /// vars for the CDK child process (per D-05: baked at deploy time,
    /// D-06: never persisted).
    extra_env: HashMap<String, String>,
    /// Runtime carrier for the `--regenerate-stack`/`--force` flag (Phase 98,
    /// DSTK-01). `execute()` re-loads `DeployConfig` from disk, which would
    /// drop the `#[serde(skip)]` `config.regenerate_stack` set by the CLI, so
    /// the flag is threaded onto the executor instead and re-applied to the
    /// freshly-loaded config before the stack.ts write.
    regenerate_stack: bool,
}

impl DeployExecutor {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            extra_env: HashMap::new(),
            regenerate_stack: false,
        }
    }

    /// Set transient environment variables to pass to the CDK child process.
    ///
    /// These are used for resolved secrets that must reach the Lambda
    /// configuration without being written to disk.
    pub fn with_extra_env(mut self, env: HashMap<String, String>) -> Self {
        self.extra_env = env;
        self
    }

    /// Set the `--regenerate-stack`/`--force` opt-in (Phase 98, DSTK-01).
    ///
    /// When `true`, an existing `deploy/lib/stack.ts` is overwritten; when
    /// `false` (default) a pre-existing curated file is preserved.
    pub fn with_regenerate_stack(mut self, regenerate_stack: bool) -> Self {
        self.regenerate_stack = regenerate_stack;
        self
    }

    pub fn execute(&self) -> Result<()> {
        let start = Instant::now();

        println!("🚀 Deploying to AWS Lambda...");
        println!();

        let mut config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        // Re-apply the runtime regeneration opt-in dropped by the disk reload
        // (`config.regenerate_stack` is `#[serde(skip)]`). Phase 98, DSTK-01.
        config.regenerate_stack = self.regenerate_stack;

        // Fail-closed IAM gate: hard errors block deploy before any AWS call;
        // warnings print to stderr and never block.
        let warnings = crate::deployment::iam::validate(&config.iam)
            .context("IAM validation failed — fix .pmcp/deploy.toml before deploying")?;
        crate::deployment::iam::emit_warnings(&warnings);

        println!("📋 Server: {}", config.server.name);
        println!("🌍 Region: {}", config.aws().region);
        println!();

        let builder = crate::deployment::builder::BinaryBuilder::new(self.project_root.clone());
        builder.build()?;
        println!();

        // Regenerate stack.ts from the loaded config so user-declared [iam]
        // permissions land in the CDK template. `init` scaffolds with an empty
        // IamConfig; the source of truth at deploy time is .pmcp/deploy.toml.
        self.regenerate_stack_ts(&config)?;

        self.run_cdk_deploy(&config)?;
        println!();

        let stack_name = format!("{}-stack", config.server.name);
        let outputs = crate::deployment::load_cdk_outputs(
            &self.project_root,
            &config.aws().region,
            &stack_name,
        )?;

        let elapsed = start.elapsed();
        println!("✅ Deployment complete in {:.1}s", elapsed.as_secs_f64());
        println!();

        outputs.display();

        Ok(())
    }

    fn regenerate_stack_ts(&self, config: &crate::deployment::config::DeployConfig) -> Result<()> {
        let lib_dir = self.project_root.join("deploy").join("lib");
        let stack_ts = crate::commands::deploy::init::render_stack_ts_for_deploy(
            &config.target.target_type,
            &config.server.name,
            &config.iam,
            &config.metadata,
        );
        // DSTK-01: preserve an operator-curated stack.ts unless
        // `--regenerate-stack`/`--force` was passed. IAM validation already ran
        // in `execute()`, so the guard never disables validation.
        let wrote = crate::deployment::config::write_stack_ts_guarded(
            &lib_dir,
            &stack_ts,
            config.regenerate_stack,
        )?;
        if !wrote {
            println!("{}", crate::deployment::config::STACK_TS_PRESERVED_NOTICE);
            // FIX #1 (deploy-toml-inert-for-preserved-stack): warn loudly when
            // the preserved stack.ts means declared [iam]/[environment] are not
            // auto-applied, so the silent no-op never surfaces only as a
            // runtime 500.
            if let Some(warning) = crate::deployment::config::stack_ts_preserved_inert_warning(
                config.iam.is_empty(),
                config.environment.is_empty(),
            ) {
                eprintln!("{warning}");
            }
        }
        Ok(())
    }

    fn run_cdk_deploy(&self, config: &crate::deployment::config::DeployConfig) -> Result<()> {
        println!("☁️  Deploying CloudFormation stack...");

        let deploy_dir = self.project_root.join("deploy");

        // Set environment variables for CDK app
        let mut cmd = Command::new("npx");
        cmd.args(&[
            "cdk",
            "deploy",
            "--require-approval",
            "never",
            "--outputs-file",
            "outputs.json",
        ])
        .current_dir(&deploy_dir)
        .env("SERVER_NAME", &config.server.name)
        .env("AWS_REGION", &config.aws().region);

        // If account ID is specified, set it
        if let Some(account_id) = &config.aws().account_id {
            cmd.env("CDK_DEFAULT_ACCOUNT", account_id);
        }

        // Pass transient env vars (resolved secrets) to CDK process.
        // These are NOT in deploy.toml -- they flow only as process env vars
        // so the CDK TypeScript stack reads them via process.env and sets
        // them on the Lambda function. Per D-05, secrets are "baked in" at
        // deploy time. Per D-06, they are never written to disk.
        for (key, value) in &self.extra_env {
            cmd.env(key, value);
        }

        print!("   Synthesizing template...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let status = cmd.status().context("Failed to run CDK deploy")?;

        if !status.success() {
            println!(" ❌");
            bail!("CDK deployment failed");
        }

        println!(" ✅");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_env_default_empty() {
        let executor = DeployExecutor::new(PathBuf::from("/tmp"));
        assert!(executor.extra_env.is_empty());
    }

    #[test]
    fn with_extra_env_builder() {
        let env: HashMap<String, String> = [
            ("SECRET_A".into(), "val_a".into()),
            ("SECRET_B".into(), "val_b".into()),
        ]
        .into();

        let executor = DeployExecutor::new(PathBuf::from("/tmp")).with_extra_env(env);

        assert_eq!(executor.extra_env.len(), 2);
        assert_eq!(executor.extra_env["SECRET_A"], "val_a");
        assert_eq!(executor.extra_env["SECRET_B"], "val_b");
    }

    #[test]
    fn with_regenerate_stack_builder() {
        let executor = DeployExecutor::new(PathBuf::from("/tmp"));
        assert!(
            !executor.regenerate_stack,
            "regenerate_stack defaults to false (preserve curated stack.ts)"
        );
        let executor = executor.with_regenerate_stack(true);
        assert!(executor.regenerate_stack);
    }

    /// Build an aws-lambda DeployConfig anchored at `project_root` with the
    /// given regeneration opt-in.
    fn aws_lambda_cfg(
        project_root: PathBuf,
        regenerate_stack: bool,
    ) -> crate::deployment::config::DeployConfig {
        let mut cfg = crate::deployment::config::DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-east-1".to_string(),
            project_root,
        );
        cfg.target.target_type = "aws-lambda".to_string();
        cfg.regenerate_stack = regenerate_stack;
        cfg
    }

    /// Seed a curated `deploy/lib/stack.ts` and return its path + content.
    fn seed_curated_stack_ts(project_root: &std::path::Path) -> (PathBuf, String) {
        let lib_dir = project_root.join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
        let path = lib_dir.join("stack.ts");
        let curated = "// operator-curated stack.ts — DO NOT CLOBBER\n".to_string();
        std::fs::write(&path, &curated).expect("seed curated stack.ts");
        (path, curated)
    }

    /// DSTK-01 (aws-lambda): a pre-existing curated stack.ts is preserved
    /// byte-for-byte when no `--regenerate-stack`/`--force` flag is set.
    #[test]
    fn aws_lambda_preserves_existing_stack_ts_without_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (path, curated) = seed_curated_stack_ts(tmp.path());

        let config = aws_lambda_cfg(tmp.path().to_path_buf(), false);
        let executor = DeployExecutor::new(tmp.path().to_path_buf());
        executor
            .regenerate_stack_ts(&config)
            .expect("guard succeeds");

        let after = std::fs::read_to_string(&path).expect("read stack.ts back");
        assert_eq!(
            after, curated,
            "curated stack.ts must be byte-identical when regenerate_stack is false"
        );
    }

    /// DSTK-01 (aws-lambda): with the flag, the curated file is re-rendered
    /// (overwritten) from the template.
    #[test]
    fn aws_lambda_overwrites_existing_stack_ts_with_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (path, curated) = seed_curated_stack_ts(tmp.path());

        let config = aws_lambda_cfg(tmp.path().to_path_buf(), true);
        let executor = DeployExecutor::new(tmp.path().to_path_buf());
        executor
            .regenerate_stack_ts(&config)
            .expect("regenerate succeeds");

        let after = std::fs::read_to_string(&path).expect("read stack.ts back");
        assert_ne!(
            after, curated,
            "stack.ts must be overwritten when regenerate_stack is true"
        );
    }
}
