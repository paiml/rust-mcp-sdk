use crate::deployment::cdk_stack_guard::expected_stack_name;
use crate::deployment::scaffold_provenance::{self, StackTsState};
use crate::deployment::DeploymentOutputs;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::ffi::OsString;
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
    /// The program every CDK child process is spawned through. Always `npx`
    /// in production; tests point it at a recording stand-in so the exact
    /// `cdk` argv can be asserted without Node.js or AWS.
    npx_program: OsString,
}

impl DeployExecutor {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            extra_env: HashMap::new(),
            regenerate_stack: false,
            npx_program: OsString::from("npx"),
        }
    }

    /// Spawn CDK through `program` instead of `npx` (test seam only).
    #[cfg(test)]
    fn with_npx_program(mut self, program: impl Into<OsString>) -> Self {
        self.npx_program = program.into();
        self
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

    /// Build the Lambda binary, deploy it with `npx cdk deploy`, and return
    /// the stack's outputs. The caller prints them (once).
    pub fn execute(&self) -> Result<DeploymentOutputs> {
        let start = Instant::now();
        let config = self.load_validated_config()?;

        let builder = crate::deployment::builder::BinaryBuilder::new(self.project_root.clone());
        builder.build()?;
        println!();

        self.deploy_and_read_outputs(&config, start)
    }

    /// [`Self::execute`] without the build: deploy the binary this same
    /// `cargo pmcp deploy` run already built into `deploy/.build/` (debug
    /// session `cargo-pmcp-deploy-targets`, A5 — the legacy fallback used to
    /// build it a second time).
    pub fn execute_prebuilt(&self) -> Result<DeploymentOutputs> {
        let start = Instant::now();
        let config = self.load_validated_config()?;
        println!("🔨 Using the Lambda binary already built for this deploy (deploy/.build)");
        println!();
        self.deploy_and_read_outputs(&config, start)
    }

    /// Load `.pmcp/deploy.toml`, re-apply the runtime flags, and run the
    /// fail-closed IAM gate.
    fn load_validated_config(&self) -> Result<crate::deployment::config::DeployConfig> {
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
        Ok(config)
    }

    /// Deploy, then read the outputs of `{[server] name}-stack` back from
    /// `deploy/outputs.json`.
    fn deploy_and_read_outputs(
        &self,
        config: &crate::deployment::config::DeployConfig,
        start: Instant,
    ) -> Result<DeploymentOutputs> {
        self.deploy_after_build(config)?;
        println!();

        let stack_name = expected_stack_name(&config.server.name);
        let outputs = crate::deployment::load_cdk_outputs(
            &self.project_root,
            &config.aws().region,
            &stack_name,
        )?;

        let elapsed = start.elapsed();
        println!("✅ Deployment complete in {:.1}s", elapsed.as_secs_f64());
        println!();

        // Not printed here: `cargo pmcp deploy` prints the outputs it gets
        // back exactly once (A2 — this used to print them a second time).
        Ok(outputs)
    }

    /// Everything between the binary build and reading the stack outputs:
    /// the stack-identity guard, `stack.ts` regeneration, the `[server]`
    /// sizing warning, and `cdk deploy`. Split out of [`Self::execute`] so the
    /// CDK-facing half is testable without a cargo-lambda build.
    fn deploy_after_build(&self, config: &crate::deployment::config::DeployConfig) -> Result<()> {
        self.guard_and_regenerate_stack_ts(config)?;

        // `[server]` sizing divergence (debug session
        // `deploy-server-memory-timeout`). This is the ONLY route to
        // `npx cdk deploy` — reached both directly and via
        // `targets::aws_lambda::deploy::deploy_legacy`'s fallback — and it
        // uploads no template file, so unlike the pmcp-run target there is no
        // post-synth seam to merge the declared sizing into. Stay quiet when
        // the declaration already matches the scaffold literals (the pristine
        // case), and say so loudly when it does not.
        if let Some(warning) = crate::deployment::config::sizing_divergence_warning(
            "aws-lambda `npx cdk deploy`",
            (config.server.memory_mb, config.server.timeout_seconds),
            (
                Some(crate::commands::deploy::init::AWS_LAMBDA_SCAFFOLD_MEMORY_MB),
                Some(crate::commands::deploy::init::AWS_LAMBDA_SCAFFOLD_TIMEOUT_SECONDS),
            ),
            "Edit deploy/lib/stack.ts's memorySize/timeout literals, or deploy to the pmcp-run \
             target, which honors the declared sizing.",
        ) {
            eprintln!("{warning}");
            println!();
        }

        self.run_cdk_deploy(config)
    }

    /// Stack-identity guard (debug session `cargo-pmcp-deploy-targets`), then
    /// `stack.ts` regeneration.
    ///
    /// Refuses unless deploy/bin/app.ts declares `{[server] name}-stack`, so a
    /// rename can never silently update a different stack. The guard runs
    /// after the binary build (the CDK app's asset dir must exist to
    /// synthesize) and BEFORE `--regenerate-stack` may overwrite an existing
    /// stack.ts, so a refused deploy changes nothing locally either. A MISSING
    /// stack.ts is scaffolded first, because the app cannot be listed without
    /// it, and is removed again if the guard then refuses.
    ///
    /// When cargo-pmcp owns both scaffold files (stack.ts unmodified or
    /// missing, app.ts unmodified), app.ts is first pointed at the current
    /// `[server] name` (finding #1), so a rename deploys a NEW stack instead of
    /// being refused. A hand-modified stack.ts leaves app.ts alone: its own
    /// `serverName` would still name the old function, so creating a new stack
    /// from it would collide with the old one. On a refusal, app.ts is put
    /// back as it was.
    fn guard_and_regenerate_stack_ts(
        &self,
        config: &crate::deployment::config::DeployConfig,
    ) -> Result<()> {
        let state = scaffold_provenance::classify_stack_ts(config)?;
        let app_ts_before = if state == StackTsState::HandModified {
            None
        } else {
            scaffold_provenance::sync_app_ts_with_server_name(
                &self.project_root,
                &config.server.name,
            )?
        };

        let result = self.guard_then_regenerate(config, state);
        if let (Err(_), Some(before)) = (&result, app_ts_before) {
            let app_ts = self.project_root.join("deploy").join("bin").join("app.ts");
            std::fs::write(&app_ts, before).with_context(|| {
                format!(
                    "failed to restore {} after the refused deploy",
                    app_ts.display()
                )
            })?;
        }
        result
    }

    /// The guard + regeneration body of [`Self::guard_and_regenerate_stack_ts`].
    fn guard_then_regenerate(
        &self,
        config: &crate::deployment::config::DeployConfig,
        state: StackTsState,
    ) -> Result<()> {
        let stack_ts = self
            .project_root
            .join("deploy")
            .join("lib")
            .join("stack.ts");
        let stack_ts_existed = state != StackTsState::Missing;
        if stack_ts_existed {
            self.ensure_app_declares_expected_stack(config)?;
        }

        // Regenerate stack.ts from the loaded config so user-declared [iam]
        // permissions land in the CDK template. `init` scaffolds with an empty
        // IamConfig; the source of truth at deploy time is .pmcp/deploy.toml.
        Self::regenerate_stack_ts(config)?;

        if stack_ts_existed {
            return Ok(());
        }
        if let Err(refusal) = self.ensure_app_declares_expected_stack(config) {
            // Undo the scaffold: a stack.ts rendered for the refused name
            // would otherwise be deployed later, under a restored name, into
            // the old stack (replacing its function).
            std::fs::remove_file(&stack_ts).with_context(|| {
                format!(
                    "{refusal:#}\n(also failed to remove the deploy/lib/stack.ts \
                     scaffolded for this refused deploy: {})",
                    stack_ts.display()
                )
            })?;
            scaffold_provenance::forget_stack_ts(&self.project_root)?;
            return Err(refusal);
        }
        Ok(())
    }

    fn regenerate_stack_ts(config: &crate::deployment::config::DeployConfig) -> Result<()> {
        let stack_ts = crate::commands::deploy::init::render_stack_ts_for_deploy(
            &config.target.target_type,
            &config.server.name,
            &config.iam,
            &config.metadata,
        );
        // DSTK-01: preserve an operator-curated (hand-modified) stack.ts
        // unless `--regenerate-stack`/`--force` was passed; an unmodified
        // scaffold is cargo-pmcp's own output and is regenerated. IAM
        // validation already ran, so the guard never disables validation.
        let wrote = scaffold_provenance::write_scaffold_stack_ts(config, &stack_ts)?;
        if !wrote {
            println!("{}", crate::deployment::config::STACK_TS_PRESERVED_NOTICE);
            // FIX #1 (deploy-toml-inert-for-preserved-stack): warn loudly when
            // the preserved stack.ts means declared [iam]/[environment] are not
            // auto-applied, so the silent no-op never surfaces only as a
            // runtime 500.
            // `sizing_inert`: TRUE whenever `[server]` sizing is declared. This
            // path ends in `npx cdk deploy`, which uploads no template file, so
            // there is no post-synth seam to inject `Properties.MemorySize`/
            // `Timeout` through — the preserved stack.ts literals are
            // authoritative and the declared values cannot be honored
            // (debug session `deploy-server-memory-timeout`).
            let sizing_inert =
                config.server.memory_mb.is_some() || config.server.timeout_seconds.is_some();
            if let Some(warning) = crate::deployment::config::stack_ts_preserved_inert_warning(
                config.iam.is_empty(),
                config.environment.is_empty(),
                sizing_inert,
            ) {
                eprintln!("{warning}");
            }
        }
        Ok(())
    }

    /// A `npx cdk <args>` child in deploy/, carrying the environment every CDK
    /// step of this deploy shares. The stack guard's `cdk list` is built here
    /// too, so an app.ts that derives its stack id from this environment
    /// answers the guard exactly as it answers `cdk deploy`.
    fn cdk_command(
        &self,
        config: &crate::deployment::config::DeployConfig,
        args: &[&str],
    ) -> Command {
        let mut cmd = Command::new(&self.npx_program);
        cmd.arg("cdk")
            .args(args)
            .current_dir(self.project_root.join("deploy"))
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
        cmd
    }

    /// Refuse unless the CDK app declares `{[server] name}-stack` (see
    /// [`crate::deployment::cdk_stack_guard`]).
    fn ensure_app_declares_expected_stack(
        &self,
        config: &crate::deployment::config::DeployConfig,
    ) -> Result<()> {
        crate::deployment::cdk_stack_guard::ensure_app_declares_stack(
            self.cdk_command(config, &["list"]),
            &config.server.name,
            &config.aws().region,
        )
    }

    fn run_cdk_deploy(&self, config: &crate::deployment::config::DeployConfig) -> Result<()> {
        println!("☁️  Deploying CloudFormation stack...");

        // Name the stack explicitly: with no stack argument `cdk deploy`
        // deploys whatever single stack app.ts declares, which after a rename
        // is the OLD deployment.
        let stack_name = expected_stack_name(&config.server.name);
        let mut cmd = self.cdk_command(
            config,
            &[
                "deploy",
                &stack_name,
                "--require-approval",
                "never",
                "--outputs-file",
                "outputs.json",
            ],
        );

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
        DeployExecutor::regenerate_stack_ts(&config).expect("guard succeeds");

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
        DeployExecutor::regenerate_stack_ts(&config).expect("regenerate succeeds");

        let after = std::fs::read_to_string(&path).expect("read stack.ts back");
        assert_ne!(
            after, curated,
            "stack.ts must be overwritten when regenerate_stack is true"
        );
    }

    /// Stack-identity guard (debug session `cargo-pmcp-deploy-targets`, guard
    /// PR). These drive `deploy_after_build` through a recording `npx`
    /// stand-in, so they assert the exact `cdk` argv and that `cdk deploy`
    /// is never spawned when deploy/bin/app.ts declares a different stack.
    #[cfg(unix)]
    mod stack_guard {
        use super::*;
        use crate::deployment::fake_npx::FakeNpx;

        /// The `[server] name` every guard test deploys under.
        const NAME: &str = "acme";
        /// The stack the CLI reports, outputs and destroys for [`NAME`].
        const EXPECTED: &str = "acme-stack";
        /// The stack a stale, init-time deploy/bin/app.ts still declares.
        const STALE: &str = "old-server-stack";

        /// A project root with a `deploy/` dir and a `[server] name = acme`
        /// aws-lambda config.
        fn project(
            regenerate_stack: bool,
        ) -> (tempfile::TempDir, crate::deployment::config::DeployConfig) {
            let tmp = tempfile::tempdir().expect("tempdir");
            std::fs::create_dir_all(tmp.path().join("deploy")).expect("create deploy/");
            let mut cfg = aws_lambda_cfg(tmp.path().to_path_buf(), regenerate_stack);
            cfg.server.name = NAME.to_string();
            (tmp, cfg)
        }

        fn executor(root: &std::path::Path, npx: &FakeNpx) -> DeployExecutor {
            DeployExecutor::new(root.to_path_buf()).with_npx_program(npx.program())
        }

        /// The reported bug: after a `[server] name` rename, app.ts still
        /// declares the old stack and `cdk deploy` updated it in place. The
        /// deploy must be refused before `cdk deploy` runs, and the error must
        /// name both stacks.
        #[test]
        fn rename_refuses_cdk_deploy_when_app_declares_another_stack() {
            let (tmp, cfg) = project(false);
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{STALE}\n"), "", 0);

            let result = executor(tmp.path(), &npx).deploy_after_build(&cfg);

            let err = format!(
                "{:#}",
                result.expect_err("a stack mismatch must refuse the deploy")
            );
            assert!(
                err.contains(EXPECTED),
                "error must name the expected stack: {err}"
            );
            assert!(
                err.contains(STALE),
                "error must name the declared stack: {err}"
            );
            assert!(
                !npx.ran("cdk deploy"),
                "cdk deploy must never run on a mismatch; calls: {:?}",
                npx.calls()
            );
        }

        /// When app.ts declares the expected stack, `cdk deploy` runs and
        /// names that stack explicitly instead of deploying whatever single
        /// stack the app happens to declare.
        #[test]
        fn deploy_names_the_expected_stack_when_app_declares_it() {
            let (tmp, cfg) = project(false);
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{EXPECTED}\n"), "", 0);

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect("matching stack deploys");

            assert_eq!(
                npx.argv_lines(),
                vec![
                    "cdk list".to_string(),
                    format!(
                        "cdk deploy {EXPECTED} --require-approval never --outputs-file outputs.json"
                    ),
                ],
                "full call log: {:?}",
                npx.calls()
            );
        }

        /// A refused deploy changes nothing locally either: `--regenerate-stack`
        /// must not rewrite an existing stack.ts (and so its functionName)
        /// for a deploy the guard refuses.
        #[test]
        fn refused_regenerate_stack_rename_leaves_stack_ts_untouched() {
            let (tmp, cfg) = project(true);
            let (path, curated) = seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{STALE}\n"), "", 0);

            let result = executor(tmp.path(), &npx).deploy_after_build(&cfg);

            assert!(result.is_err(), "a stack mismatch must refuse the deploy");
            let after = std::fs::read_to_string(&path).expect("read stack.ts back");
            assert_eq!(after, curated, "a refused deploy must not rewrite stack.ts");
            assert!(!npx.ran("cdk deploy"), "calls: {:?}", npx.calls());
        }

        /// A MISSING stack.ts is scaffolded first (the CDK app cannot be
        /// listed without it), the guard still refuses the mismatch, and the
        /// scaffold is removed again so the refusal leaves no trace.
        #[test]
        fn missing_stack_ts_is_scaffolded_then_guarded() {
            let (tmp, cfg) = project(false);
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{STALE}\n"), "", 0);

            let result = executor(tmp.path(), &npx).deploy_after_build(&cfg);

            assert!(result.is_err(), "a stack mismatch must refuse the deploy");
            assert!(
                npx.calls().contains(&"probe stack.ts=present".to_string()),
                "cdk list must run after the missing stack.ts is scaffolded: {:?}",
                npx.calls()
            );
            assert!(!npx.ran("cdk deploy"), "calls: {:?}", npx.calls());
            assert!(
                !tmp.path().join("deploy/lib/stack.ts").exists(),
                "a refused deploy must remove the stack.ts it scaffolded"
            );
        }

        /// A MISSING stack.ts that passes the guard is kept and deployed.
        #[test]
        fn missing_stack_ts_is_scaffolded_and_kept_when_guard_passes() {
            let (tmp, cfg) = project(false);
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{EXPECTED}\n"), "", 0);

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect("matching stack deploys");

            assert!(tmp.path().join("deploy/lib/stack.ts").exists());
            assert!(
                npx.ran(&format!("cdk deploy {EXPECTED} ")),
                "{:?}",
                npx.calls()
            );
        }

        /// An existing stack.ts is guarded BEFORE any regeneration: `cdk list`
        /// sees the operator's file, not a freshly rendered one.
        #[test]
        fn existing_stack_ts_is_guarded_before_regeneration() {
            let (tmp, cfg) = project(true);
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{EXPECTED}\n"), "", 0);

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect("matching stack deploys");

            let calls = npx.calls();
            assert_eq!(
                calls.first().map(String::as_str),
                Some("cdk list"),
                "{calls:?}"
            );
            assert!(
                calls.contains(&"probe stack.ts=present".to_string()),
                "{calls:?}"
            );
        }

        /// If the CDK app cannot be listed, the guard cannot verify the stack
        /// and refuses (fail closed), surfacing cdk's own stderr.
        #[test]
        fn cdk_list_failure_refuses_deploy() {
            let (tmp, cfg) = project(false);
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(
                &tmp.path().join("bin"),
                "",
                "Error: Cannot find module '../lib/stack'\n",
                1,
            );

            let result = executor(tmp.path(), &npx).deploy_after_build(&cfg);

            let err = format!("{:#}", result.expect_err("an unlistable app must refuse"));
            assert!(
                err.contains("cdk list"),
                "error must name the failed step: {err}"
            );
            assert!(
                err.contains("Cannot find module"),
                "error must carry cdk's stderr: {err}"
            );
            assert!(!npx.ran("cdk deploy"), "calls: {:?}", npx.calls());
        }

        /// Seed the two init-time scaffold files for `name`: app.ts and an
        /// unmodified, recorded stack.ts.
        fn seed_init_scaffolds(root: &std::path::Path, name: &str) {
            let bin = root.join("deploy").join("bin");
            std::fs::create_dir_all(&bin).expect("create deploy/bin");
            std::fs::write(
                bin.join("app.ts"),
                crate::commands::deploy::init::render_app_ts(name),
            )
            .expect("write app.ts");
            let lib = root.join("deploy").join("lib");
            std::fs::create_dir_all(&lib).expect("create deploy/lib");
            let stack_ts = crate::commands::deploy::init::render_stack_ts_for_deploy(
                "aws-lambda",
                name,
                &crate::deployment::config::IamConfig::default(),
                &crate::deployment::config::MetadataConfig::default(),
            );
            std::fs::write(lib.join("stack.ts"), &stack_ts).expect("write stack.ts");
            scaffold_provenance::record_stack_ts(root, &stack_ts).expect("record");
        }

        fn read(root: &std::path::Path, rel: &str) -> String {
            std::fs::read_to_string(root.join(rel)).expect("read scaffold file")
        }

        /// Finding #1, end to end on the legacy path: with both scaffold files
        /// still cargo-pmcp's own, a `[server] name` rename deploys a NEW
        /// stack named from deploy.toml. app.ts and stack.ts follow the name.
        #[test]
        fn rename_with_untouched_scaffolds_deploys_a_new_stack() {
            let (tmp, cfg) = project(false);
            seed_init_scaffolds(tmp.path(), "old-server");
            let npx = FakeNpx::listing_scaffold_app_ts(&tmp.path().join("bin"));

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect("a rename of untouched scaffolds deploys");

            assert!(
                npx.ran(&format!("cdk deploy {EXPECTED} ")),
                "{:?}",
                npx.calls()
            );
            assert_eq!(
                read(tmp.path(), "deploy/bin/app.ts"),
                crate::commands::deploy::init::render_app_ts(NAME)
            );
            assert_eq!(
                read(tmp.path(), "deploy/lib/stack.ts"),
                crate::commands::deploy::init::render_stack_ts_for_deploy(
                    "aws-lambda",
                    NAME,
                    &cfg.iam,
                    &cfg.metadata
                )
            );
        }

        /// A hand-modified stack.ts still bakes the OLD function name, so a
        /// new stack built from it would collide with the old one: app.ts is
        /// left alone and the guard refuses, as before.
        #[test]
        fn rename_with_a_hand_modified_stack_ts_is_still_refused() {
            let (tmp, cfg) = project(false);
            seed_init_scaffolds(tmp.path(), "old-server");
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::listing_scaffold_app_ts(&tmp.path().join("bin"));

            let err = executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect_err("must refuse");

            assert!(format!("{err:#}").contains(STALE), "{err:#}");
            assert!(!npx.ran("cdk deploy"), "{:?}", npx.calls());
            assert_eq!(
                read(tmp.path(), "deploy/bin/app.ts"),
                crate::commands::deploy::init::render_app_ts("old-server")
            );
        }

        /// A refused deploy changes nothing locally: an app.ts that was
        /// pointed at the new name is put back.
        #[test]
        fn refused_deploy_restores_a_synced_app_ts() {
            let (tmp, cfg) = project(false);
            seed_init_scaffolds(tmp.path(), "old-server");
            let npx = FakeNpx::new(&tmp.path().join("bin"), "", "Error: synth failed\n", 1);

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect_err("an unlistable app must refuse");

            assert_eq!(
                read(tmp.path(), "deploy/bin/app.ts"),
                crate::commands::deploy::init::render_app_ts("old-server")
            );
            assert!(!npx.ran("cdk deploy"), "{:?}", npx.calls());
        }

        /// A2: the legacy executor returns the outputs and leaves printing
        /// them to the CLI, which prints them once. It used to print them as
        /// well, so a legacy deploy showed "Deployment Outputs" twice.
        #[test]
        fn legacy_deploy_returns_outputs_without_printing_them() {
            let (tmp, cfg) = project(false);
            seed_curated_stack_ts(tmp.path());
            std::fs::create_dir_all(tmp.path().join(".pmcp")).expect("create .pmcp");
            std::fs::write(
                tmp.path().join(".pmcp/deploy.toml"),
                toml::to_string_pretty(&cfg).expect("serialize config"),
            )
            .expect("write deploy.toml");
            std::fs::write(
                tmp.path().join("deploy/outputs.json"),
                format!("{{\"{EXPECTED}\": {{\"ApiUrl\": \"https://acme.example.com\"}}}}"),
            )
            .expect("write outputs.json");
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{EXPECTED}\n"), "", 0);
            crate::deployment::r#trait::DISPLAY_CALLS.with(|c| c.set(0));

            let outputs = executor(tmp.path(), &npx)
                .execute_prebuilt()
                .expect("legacy deploy succeeds");

            assert_eq!(outputs.url.as_deref(), Some("https://acme.example.com"));
            assert_eq!(outputs.stack_name.as_deref(), Some(EXPECTED));
            assert_eq!(
                crate::deployment::r#trait::DISPLAY_CALLS.with(std::cell::Cell::get),
                0,
                "the legacy executor must not print the outputs; the CLI does"
            );
        }

        /// `cdk list` runs with the same env as `cdk deploy`, so an app.ts
        /// that derives its stack id from `SERVER_NAME` answers the guard the
        /// same way it would answer the deploy.
        #[test]
        fn cdk_list_sees_the_deploy_env() {
            let (tmp, cfg) = project(false);
            seed_curated_stack_ts(tmp.path());
            let npx = FakeNpx::new(&tmp.path().join("bin"), &format!("{EXPECTED}\n"), "", 0);

            executor(tmp.path(), &npx)
                .deploy_after_build(&cfg)
                .expect("matching stack deploys");

            assert!(
                npx.calls()
                    .contains(&format!("probe SERVER_NAME={NAME} AWS_REGION=us-east-1")),
                "{:?}",
                npx.calls()
            );
        }
    }
}
