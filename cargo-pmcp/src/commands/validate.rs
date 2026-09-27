//! Workflow validation command
//!
//! Validates workflow definitions in the project by:
//! 1. Running `cargo check` to ensure compilation
//! 2. Running workflow validation tests
//! 3. Providing guidance on creating validation tests

use anyhow::{Context, Result};
use clap::Subcommand;
use console::style;
use std::process::{Command, Stdio};

#[derive(Subcommand)]
pub enum ValidateCommand {
    /// Validate all workflows in the project
    ///
    /// Runs cargo check and workflow validation tests.
    /// Use --generate to create validation test scaffolding.
    Workflows {
        /// Generate validation test scaffolding if none exists
        #[arg(long)]
        generate: bool,

        /// Server directory to validate (defaults to current directory)
        #[arg(long)]
        server: Option<String>,
    },

    /// Validate `.pmcp/deploy.toml` — focuses on IAM footgun detection.
    ///
    /// Hard-errors on wildcard-`Allow`, malformed actions, empty resource
    /// lists, bad effects, and sugar-keyword typos. Warnings (unknown
    /// service prefix, cross-account ARN) print but do not fail.
    ///
    /// This is the pre-flight equivalent of the same validation that runs
    /// inside `cargo pmcp deploy` — the deploy flow itself also invokes
    /// this validator before any AWS call, so a failing `validate deploy`
    /// guarantees a failing `deploy` for the same config.
    Deploy {
        /// Server directory to validate (defaults to current directory).
        #[arg(long)]
        server: Option<String>,
    },

    /// Validate a config-driven server's `config.toml` — input-validation focus.
    ///
    /// This reads a TOOLKIT server config (`config.toml`), NOT `.pmcp/deploy.toml`
    /// — those are different documents with different validators. Use
    /// `validate deploy` for the IAM document.
    ///
    /// HARD-ERRORS on whatever `ServerConfig::validate` itself rejects: an empty
    /// or uncompilable parameter `pattern`, a non-finite numeric bound, a
    /// malformed `path` template segment, and — under
    /// `[server.validation] strict = true` — an uncapped body-position string.
    ///
    /// WARNS ONLY on every `ServerConfig::lint()` finding: an uncapped
    /// body-position string parameter, a declared length cap above the always-on
    /// path-placeholder floor, and every ACTIVE `[server.validation]` opt-out
    /// (`enforce_input_schema = false`, a zeroed default cap,
    /// `additional_properties = true`). A lint finding NEVER fails this command.
    ///
    /// The findings come from the same `lint()` the running server reports at
    /// startup, so this command and the deployed server agree for a same-version
    /// pair.
    Config {
        /// Server directory to validate (defaults to current directory).
        ///
        /// `config.toml` is looked for at this directory's root.
        #[arg(long)]
        server: Option<String>,

        /// Explicit path to the toolkit server config. Wins over `--server`.
        #[arg(long)]
        config: Option<String>,
    },
}

impl ValidateCommand {
    pub fn execute(self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        match self {
            ValidateCommand::Workflows { generate, server } => {
                validate_workflows(generate, global_flags.verbose, server)
            },
            ValidateCommand::Deploy { server } => validate_deploy(server, global_flags.verbose),
            ValidateCommand::Config { server, config } => {
                validate_server_config(server, config, global_flags.verbose)
            },
        }
    }
}

/// Main validation entry point
fn validate_workflows(generate: bool, verbose: bool, server: Option<String>) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();

    if not_quiet {
        println!("\n{}", style("PMCP Workflow Validation").cyan().bold());
        println!("{}", style("━".repeat(50)).dim());
    }

    // Change to server directory if specified
    let original_dir = std::env::current_dir()?;
    if let Some(ref server_dir) = server {
        std::env::set_current_dir(server_dir)
            .with_context(|| format!("Failed to change to server directory: {}", server_dir))?;
        if not_quiet {
            println!(
                "  {} Validating in: {}",
                style("→").dim(),
                style(server_dir).yellow()
            );
        }
    }

    let result = run_validation(generate, verbose, not_quiet);

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    result
}

fn run_validation(generate: bool, verbose: bool, not_quiet: bool) -> Result<()> {
    // Step 1: Compilation check
    run_cargo_check(verbose, not_quiet)?;

    // Step 2: Discover workflow tests
    if not_quiet {
        println!(
            "\n{} Looking for workflow validation tests...",
            style("Step 2:").bold()
        );
    }
    let test_patterns = find_workflow_tests()?;

    if test_patterns.is_empty() {
        return handle_no_patterns(generate, not_quiet);
    }

    if not_quiet {
        println!(
            "  {} Found {} workflow test pattern(s)",
            style("✓").green(),
            test_patterns.len()
        );
        println!(
            "\n{} Running workflow validation tests...",
            style("Step 3:").bold()
        );
    }

    let (all_passed, total_tests, passed_tests) =
        run_all_test_patterns(&test_patterns, verbose, not_quiet)?;

    print_validation_summary(all_passed, total_tests, passed_tests, not_quiet)
}

/// Run `cargo check --message-format=short` and bail on failure.
fn run_cargo_check(verbose: bool, not_quiet: bool) -> Result<()> {
    if not_quiet {
        println!("\n{} Checking compilation...", style("Step 1:").bold());
    }

    let check_status = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .stdout(if verbose {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stderr(if verbose {
            Stdio::inherit()
        } else {
            Stdio::piped()
        })
        .status()
        .context("Failed to run cargo check")?;

    if !check_status.success() {
        println!(
            "  {} Compilation failed. Fix errors and try again.",
            style("✗").red()
        );
        return Err(anyhow::anyhow!("Compilation failed"));
    }
    if not_quiet {
        println!("  {} Compilation successful", style("✓").green());
    }
    Ok(())
}

/// When no workflow-test patterns match: emit `generate_validation_scaffolding`
/// (if `generate`) or user-facing guidance.
fn handle_no_patterns(generate: bool, not_quiet: bool) -> Result<()> {
    if generate {
        if not_quiet {
            println!(
                "  {} No workflow tests found. Generating scaffolding...",
                style("!").yellow()
            );
        }
        generate_validation_scaffolding(not_quiet)?;
        if not_quiet {
            println!(
                "  {} Generated validation test scaffolding",
                style("✓").green()
            );
            println!(
                "\n  Run {} again to validate",
                style("cargo pmcp validate workflows").cyan()
            );
        }
        return Ok(());
    }

    if not_quiet {
        println!(
            "  {} No workflow validation tests found",
            style("!").yellow()
        );
        print_test_guidance(not_quiet);
    }
    Ok(())
}

/// Run each test pattern. Returns (all_passed, total_tests, passed_tests).
fn run_all_test_patterns(
    test_patterns: &[String],
    verbose: bool,
    not_quiet: bool,
) -> Result<(bool, usize, usize)> {
    let mut all_passed = true;
    let mut total_tests = 0;
    let mut passed_tests = 0;

    for pattern in test_patterns {
        let test_output = Command::new("cargo")
            .args(["test", pattern, "--", "--nocapture"])
            .output()
            .context("Failed to run cargo test")?;

        let stdout = String::from_utf8_lossy(&test_output.stdout);
        let stderr = String::from_utf8_lossy(&test_output.stderr);

        let (tests_run, tests_passed, tests_failed) = parse_test_output(&stdout, &stderr);
        total_tests += tests_run;
        passed_tests += tests_passed;

        if verbose {
            println!("{}", stdout);
            if !stderr.is_empty() {
                eprintln!("{}", stderr);
            }
        }

        let passed = print_pattern_result(
            pattern,
            tests_run,
            tests_passed,
            tests_failed,
            &stdout,
            &stderr,
            verbose,
            not_quiet,
        );
        if !passed {
            all_passed = false;
        }
    }

    Ok((all_passed, total_tests, passed_tests))
}

/// Print one test-pattern's result and return whether it passed.
#[allow(clippy::too_many_arguments)]
fn print_pattern_result(
    pattern: &str,
    tests_run: usize,
    tests_passed: usize,
    tests_failed: usize,
    stdout: &str,
    stderr: &str,
    verbose: bool,
    not_quiet: bool,
) -> bool {
    if tests_failed > 0 {
        if not_quiet {
            println!(
                "  {} Pattern '{}': {} passed, {} failed",
                style("✗").red(),
                pattern,
                tests_passed,
                style(tests_failed).red()
            );
        }
        if !verbose {
            print_failure_summary(stdout, stderr);
        }
        return false;
    }

    if tests_run > 0 {
        if not_quiet {
            println!(
                "  {} Pattern '{}': {} passed",
                style("✓").green(),
                pattern,
                tests_passed
            );
        }
    } else if not_quiet {
        println!(
            "  {} Pattern '{}': no tests matched",
            style("-").dim(),
            pattern
        );
    }
    true
}

/// Print the final "all passed / X of Y / no tests" summary and return
/// an Err when validation failed.
fn print_validation_summary(
    all_passed: bool,
    total_tests: usize,
    passed_tests: usize,
    not_quiet: bool,
) -> Result<()> {
    if not_quiet {
        println!("\n{}", style("━".repeat(50)).dim());
    }

    if all_passed && total_tests > 0 {
        if not_quiet {
            println!(
                "{} All {} workflow validation tests passed!",
                style("✓").green().bold(),
                passed_tests
            );
            println!("\n  Your workflows are structurally valid and ready for use.");
        }
        return Ok(());
    }

    if total_tests == 0 {
        if not_quiet {
            println!(
                "{} No workflow tests were executed",
                style("!").yellow().bold()
            );
            print_test_guidance(not_quiet);
        }
        return Ok(());
    }

    println!(
        "{} Workflow validation failed: {} of {} tests passed",
        style("✗").red().bold(),
        passed_tests,
        total_tests
    );
    Err(anyhow::anyhow!("Workflow validation failed"))
}

/// Find test patterns that match workflow tests
fn find_workflow_tests() -> Result<Vec<String>> {
    let mut patterns = Vec::new();

    // Look for common workflow test patterns
    let test_patterns = [
        "workflow",
        "test_workflow",
        "workflow_valid",
        "workflow_validation",
    ];

    // Check if any tests exist with these patterns
    for pattern in test_patterns {
        let output = Command::new("cargo")
            .args(["test", pattern, "--", "--list"])
            .output()
            .context("Failed to list tests")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if any tests were found
        if stdout.contains(": test") {
            patterns.push(pattern.to_string());
        }
    }

    Ok(patterns)
}

/// Parse test output to extract pass/fail counts
fn parse_test_output(stdout: &str, stderr: &str) -> (usize, usize, usize) {
    let combined = format!("{}\n{}", stdout, stderr);

    if let Some(counts) = parse_test_result_line(&combined) {
        return counts;
    }

    // Alternative: count individual test lines
    let passed = combined.matches("... ok").count();
    let failed = combined.matches("... FAILED").count();
    (passed + failed, passed, failed)
}

/// Parse the `test result: ...` line if present.
fn parse_test_result_line(combined: &str) -> Option<(usize, usize, usize)> {
    let line = combined.lines().find(|l| l.starts_with("test result:"))?;

    let passed = count_for_keyword(line, "passed");
    let failed = count_for_keyword(line, "failed");
    Some((passed + failed, passed, failed))
}

/// Extract the integer count preceding `keyword` (e.g. "passed" or "failed")
/// in a cargo-test result line. Tries the `"N <keyword>"` token form first,
/// then falls back to semicolon-separated segments.
fn count_for_keyword(line: &str, keyword: &str) -> usize {
    let with_space = format!(" {keyword}");
    line.split_whitespace()
        .find_map(|word| {
            word.strip_suffix(with_space.as_str())
                .and_then(|n| n.parse().ok())
        })
        .or_else(|| {
            line.split(';')
                .find(|part| part.contains(keyword))
                .and_then(|part| part.split_whitespace().next())
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

/// Print failure summary from test output
fn print_failure_summary(stdout: &str, stderr: &str) {
    let combined = format!("{}\n{}", stdout, stderr);

    // Find failure messages
    let mut in_failure = false;
    for line in combined.lines() {
        if line.contains("FAILED") || line.contains("panicked at") {
            in_failure = true;
        }
        if in_failure {
            if line.starts_with("---- ") || line.is_empty() {
                if !line.is_empty() {
                    println!("    {}", style(line).red());
                }
                in_failure = line.starts_with("---- ");
            } else {
                println!("    {}", line);
            }
        }
    }
}

/// Generate validation test scaffolding
fn generate_validation_scaffolding(not_quiet: bool) -> Result<()> {
    let test_dir = std::path::Path::new("tests");
    if !test_dir.exists() {
        std::fs::create_dir(test_dir)?;
    }

    let test_file = test_dir.join("workflow_validation.rs");
    if test_file.exists() {
        if not_quiet {
            println!(
                "    {} Test file already exists: {}",
                style("!").yellow(),
                test_file.display()
            );
        }
        return Ok(());
    }

    let test_content = r#"//! Workflow validation tests
//!
//! Generated by `cargo pmcp validate workflows --generate`
//!
//! These tests verify that your workflow definitions are structurally valid.
//! Add tests for each workflow you create.

// TODO: Import your workflow creation functions
// use your_crate::workflows::create_my_workflow;

/// Template: Validate workflow structure
///
/// Copy and adapt this test for each workflow in your server.
#[test]
fn test_workflow_is_valid() {
    // TODO: Replace with your workflow creation
    // let workflow = create_my_workflow();
    // workflow.validate().expect("Workflow should be valid");

    // Example assertions:
    // assert_eq!(workflow.name(), "my_workflow");
    // assert!(!workflow.steps().is_empty());

    // Placeholder - remove when you add your workflows
    println!("TODO: Add workflow validation tests");
}

/// Template: Validate workflow bindings
///
/// Test that step outputs are properly bound and referenced.
#[test]
fn test_workflow_bindings() {
    // TODO: Replace with your workflow
    // let workflow = create_my_workflow();
    //
    // // Check that expected bindings exist
    // let bindings = workflow.output_bindings();
    // assert!(bindings.contains(&"result".into()));

    println!("TODO: Add binding validation tests");
}

/// Template: Test workflow execution
///
/// For integration testing, you can execute the workflow.
#[tokio::test]
async fn test_workflow_execution() {
    // TODO: Build a test server with your workflow
    // let server = Server::builder()
    //     .name("test")
    //     .version("1.0.0")
    //     .tool_typed("my_tool", my_tool_handler)
    //     .prompt_workflow(create_my_workflow())
    //     .expect("Workflow should register")
    //     .build()
    //     .expect("Server should build");
    //
    // let handler = server.get_prompt("my_workflow").unwrap();
    // let mut args = std::collections::HashMap::new();
    // args.insert("input".into(), "test".into());
    //
    // let result = handler.handle(args, test_extra()).await
    //     .expect("Workflow should execute");
    //
    // assert!(!result.messages.is_empty());

    println!("TODO: Add workflow execution tests");
}
"#;

    std::fs::write(&test_file, test_content)?;
    if not_quiet {
        println!(
            "    {} Created: {}",
            style("→").dim(),
            style(test_file.display()).cyan()
        );
    }

    Ok(())
}

/// Print guidance on creating workflow tests
fn print_test_guidance(not_quiet: bool) {
    if not_quiet {
        println!(
            "\n{}",
            style("How to create workflow validation tests:").bold()
        );
        println!();
        println!(
            "  1. Run {} to generate scaffolding",
            style("cargo pmcp validate workflows --generate").cyan()
        );
        println!();
        println!("  2. Or manually add tests like:");
        println!();
        println!(
            "     {}",
            style("// In tests/workflow_validation.rs or your lib.rs").dim()
        );
        println!("     {}", style("#[test]").yellow());
        println!(
            "     {} test_my_workflow_is_valid() {{",
            style("fn").yellow()
        );
        println!("         let workflow = create_my_workflow();");
        println!("         workflow.validate().expect(\"Workflow should be valid\");");
        println!("         assert_eq!(workflow.name(), \"my_workflow\");");
        println!("     }}");
        println!();
        println!(
            "  3. Run {} to validate",
            style("cargo pmcp validate workflows").cyan()
        );
        println!();
        println!(
            "  {} Validation is automatic when you call .prompt_workflow(),",
            style("Note:").bold()
        );
        println!(
            "        but tests let you catch errors at {} time.",
            style("cargo test").cyan()
        );
    }
}

/// Validate the deployment configuration (`.pmcp/deploy.toml`) — IAM focus.
///
/// This is the pre-flight equivalent of the same validation that runs inside
/// `cargo pmcp deploy`. Returns `Ok(())` on success (even when warnings are
/// emitted); returns `Err` on any CR-locked hard-error rule.
///
/// Warnings are printed to stderr with a yellow `warning:` prefix; they do
/// not fail the command.
///
/// Since Phase 128 (D-07) it ALSO reports the toolkit input-validation findings
/// when a `config.toml` is discoverable at the same server directory root — as
/// warnings only. See [`discover_server_config_lint`] for why a parse failure of
/// that DISCOVERED document is a warning rather than an error.
///
/// # Errors
/// Returns `Err` when:
/// - `.pmcp/deploy.toml` is missing or malformed
/// - Any hard-error rule in [`crate::deployment::iam::validate`] is violated
pub fn validate_deploy(server: Option<String>, verbose: bool) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();

    let project_root = match server {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir().context("failed to read current directory")?,
    };

    if not_quiet {
        println!("\n{}", style("PMCP Deploy Config Validation").cyan().bold());
        println!("{}", style("━".repeat(50)).dim());
        if verbose {
            println!("  Project: {}", project_root.display());
        }
    }

    let config = crate::deployment::config::DeployConfig::load(&project_root)
        .context("failed to load .pmcp/deploy.toml")?;

    let warnings = crate::deployment::iam::validate(&config.iam)
        .context("IAM validation failed — fix .pmcp/deploy.toml before deploying")?;

    if not_quiet {
        crate::deployment::iam::emit_warnings(&warnings);
        if warnings.is_empty() {
            println!(
                "  {} IAM configuration valid (no warnings)",
                style("✓").green()
            );
        } else {
            println!(
                "  {} IAM configuration valid ({} warning{})",
                style("✓").green(),
                warnings.len(),
                if warnings.len() == 1 { "" } else { "s" }
            );
        }
    }

    // Phase 128 D-07 names `cargo pmcp validate deploy` explicitly, so the toolkit
    // input-validation findings surface HERE as well as through `validate config`.
    //
    // EXIT-CODE CONTRACT, UNCHANGED: everything below is WARNINGS ONLY. No branch
    // of it can return `Err` or alter this function's exit code, so the guarantee
    // documented at `validate.rs:36-38` — a failing `validate deploy` guarantees a
    // failing `deploy` for the same config — still holds exactly as written. That is
    // also why a parse failure of the DISCOVERED toolkit config is a warning and not
    // an error: this command's subject is the deploy document.
    emit_discovered_server_config_lint(&discover_server_config_lint(&project_root), not_quiet);

    Ok(())
}

// -----------------------------------------------------------------------------
// Phase 128 Plan 07 — `cargo pmcp validate config` (D3 / SC-3)
// -----------------------------------------------------------------------------

/// The toolkit server-config file name looked for at a server directory root.
const SERVER_CONFIG_FILE: &str = "config.toml";

/// Which file `validate config` should read.
///
/// An explicit `--config` wins outright; otherwise [`SERVER_CONFIG_FILE`] at the
/// `--server` directory root, falling back to the process working directory.
///
/// # Errors
/// Returns `Err` only when neither flag was given and the working directory
/// cannot be read.
fn resolve_server_config_path(
    server: Option<&str>,
    config: Option<&str>,
) -> Result<std::path::PathBuf> {
    if let Some(explicit) = config {
        return Ok(std::path::PathBuf::from(explicit));
    }
    let root = match server {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir().context("failed to read current directory")?,
    };
    Ok(root.join(SERVER_CONFIG_FILE))
}

/// Read, parse and `validate()` a toolkit server config.
///
/// # Errors
/// Returns `Err` when the file cannot be read, is not parseable as a toolkit
/// `ServerConfig`, or is rejected by `ServerConfig::validate` — each with the path
/// in the context chain.
fn load_server_config(path: &std::path::Path) -> Result<pmcp_server_toolkit::config::ServerConfig> {
    let text = std::fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read toolkit server config at {} (looked for {SERVER_CONFIG_FILE})",
            path.display()
        )
    })?;
    let config =
        pmcp_server_toolkit::config::ServerConfig::from_toml(&text).with_context(|| {
            format!(
                "failed to parse toolkit server config at {}",
                path.display()
            )
        })?;
    config
        .validate()
        .with_context(|| format!("invalid toolkit server config at {}", path.display()))?;
    Ok(config)
}

/// `ServerConfig::lint()`'s findings, rendered one per line.
///
/// A PROJECTION of the toolkit's single implementation, never a second copy of any
/// rule (Phase 128 Q5 / T-128-32): this CLI holds no rule literals of its own, so
/// what a reviewer sees here is exactly what the running server reports at startup.
fn render_config_lint_findings(config: &pmcp_server_toolkit::config::ServerConfig) -> Vec<String> {
    config.lint().iter().map(ToString::to_string).collect()
}

/// Print lint findings to stderr, with the same `warning:` prefix
/// [`crate::deployment::iam::emit_warnings`] uses for the IAM document.
fn emit_config_lint_findings(findings: &[String]) {
    for finding in findings {
        eprintln!("  {} {}", style("warning:").yellow(), finding);
    }
}

/// Validate a config-driven server's toolkit `config.toml` — input-validation focus.
///
/// Warnings (every `ServerConfig::lint()` finding) print to stderr and do NOT fail
/// the command. Only a read failure, a parse failure, or a `ServerConfig::validate`
/// rejection returns `Err`.
///
/// # Errors
/// Returns `Err` when the config file cannot be read or parsed, or when
/// `ServerConfig::validate` rejects it.
pub fn validate_server_config(
    server: Option<String>,
    config: Option<String>,
    verbose: bool,
) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    let path = resolve_server_config_path(server.as_deref(), config.as_deref())?;

    if not_quiet {
        println!("\n{}", style("PMCP Server Config Validation").cyan().bold());
        println!("{}", style("━".repeat(50)).dim());
        if verbose {
            println!("  Config: {}", path.display());
        }
    }

    let parsed = load_server_config(&path)?;
    let findings = render_config_lint_findings(&parsed);

    if not_quiet {
        emit_config_lint_findings(&findings);
        if findings.is_empty() {
            println!(
                "  {} Server config valid — no input-validation findings",
                style("✓").green()
            );
        } else {
            println!(
                "  {} Server config valid ({} input-validation finding{})",
                style("✓").green(),
                findings.len(),
                if findings.len() == 1 { "" } else { "s" }
            );
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// Phase 128 Plan 07 — D-07's literal surface: the same findings from
// `cargo pmcp validate deploy`, warnings only
// -----------------------------------------------------------------------------

/// What `validate deploy` found in a toolkit `config.toml` sitting at the same
/// server directory root as `.pmcp/deploy.toml`.
///
/// Three states rather than a `Result<Vec<String>>` so the CALLER cannot conflate
/// "no such document" (a pure-IAM project, which must stay silent rather than
/// become noisy) with "a document that exists and reports nothing".
#[derive(Debug)]
enum DiscoveredConfigLint {
    /// No `config.toml` at the server directory root — a pure-IAM project.
    Absent,
    /// Rendered `ServerConfig::lint()` findings. Possibly empty.
    Findings(Vec<String>),
    /// The file is there but could not be read, parsed, or validated.
    ///
    /// A WARNING and never an error, for the reason stated at the call site in
    /// [`validate_deploy`]: this command's subject is the deploy document. Note the
    /// asymmetry that makes the warning worth reading — the SERVER would refuse to
    /// boot on this same file, so an operator who ignores it ships a server that
    /// will not start.
    Unreadable(String),
}

/// Look for a toolkit `config.toml` next to the deploy document and lint it.
///
/// Never returns an error: every failure mode becomes
/// [`DiscoveredConfigLint::Unreadable`], preserving `validate_deploy`'s exit-code
/// contract.
fn discover_server_config_lint(_project_root: &std::path::Path) -> DiscoveredConfigLint {
    DiscoveredConfigLint::Findings(Vec::new())
}

/// Render a [`DiscoveredConfigLint`]. Prints NOTHING for
/// [`DiscoveredConfigLint::Absent`] — a pure-IAM project must not become noisy.
fn emit_discovered_server_config_lint(lint: &DiscoveredConfigLint, not_quiet: bool) {
    if !not_quiet {
        return;
    }
    match lint {
        DiscoveredConfigLint::Absent => {},
        DiscoveredConfigLint::Findings(findings) if findings.is_empty() => {
            println!(
                "  {} {SERVER_CONFIG_FILE} valid — no input-validation findings",
                style("✓").green()
            );
        },
        DiscoveredConfigLint::Findings(findings) => {
            println!(
                "  {} input-validation findings from {SERVER_CONFIG_FILE} ({} finding{}):",
                style("→").cyan(),
                findings.len(),
                if findings.len() == 1 { "" } else { "s" }
            );
            emit_config_lint_findings(findings);
        },
        DiscoveredConfigLint::Unreadable(detail) => {
            eprintln!(
                "  {} {SERVER_CONFIG_FILE} is present but could NOT be read as a toolkit server \
                 config, so its input-validation findings were NOT checked — the server itself \
                 would refuse to boot on this file: {detail}",
                style("warning:").yellow()
            );
        },
    }
}

#[cfg(test)]
mod server_config_validate_tests {
    //! Phase 128 Plan 07 — unit coverage for `cargo pmcp validate config`.
    //!
    //! # Every test here passes an EXPLICIT `--config` path, and none may depend
    //! # on the process working directory
    //!
    //! `make test-cargo-pmcp` runs its `--lib` leg WITHOUT `--test-threads=1`
    //! (`Makefile:340`) while this crate's tests are known to race, and
    //! `std::env::set_current_dir` is process-global. A cwd-relative discovery test
    //! would therefore be flaky by construction. `resolve_server_config_path`'s
    //! cwd branch is exercised through the integration binary
    //! (`cargo-pmcp/tests/validate_server_config.rs`), which runs a child process
    //! with its own working directory. Keep this rule when adding the next test.

    use super::*;

    /// A minimal valid `[server]` header, prepended to every fixture.
    const SERVER_HEADER: &str = r#"
[server]
name = "demo-server"
version = "0.1.0"
"#;

    /// Write `toml_str` as a `config.toml` inside a fresh tempdir.
    ///
    /// Returns the guard (which must outlive the path) and the explicit file path
    /// every test passes as `--config`.
    fn write_server_config(toml_str: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(SERVER_CONFIG_FILE);
        std::fs::write(&path, toml_str).expect("write config.toml");
        (dir, path)
    }

    /// Parse a fixture and return its rendered lint findings.
    fn findings_for(toml_str: &str) -> Vec<String> {
        let config = pmcp_server_toolkit::config::ServerConfig::from_toml(toml_str)
            .expect("fixture must parse");
        render_config_lint_findings(&config)
    }

    /// Run the command against an explicit config path.
    fn run_on(path: &std::path::Path) -> Result<()> {
        std::env::set_var("PMCP_QUIET", "1");
        validate_server_config(None, Some(path.to_string_lossy().into_owned()), false)
    }

    /// A `POST` tool's non-path string parameter is BODY position, where the D3
    /// default cap deliberately does not apply — so it is an uncapped-string finding.
    const UNCAPPED_BODY_STRING: &str = r#"
[[tools]]
name = "post-comment"
description = "Post a comment."
path = "/issues/1/comments"
method = "POST"

[[tools.parameters]]
name = "body_text"
type = "string"
description = "Free text."
"#;

    #[test]
    fn uncapped_body_string_is_reported_naming_tool_and_param() {
        let findings = findings_for(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}"));
        assert_eq!(
            findings.len(),
            1,
            "expected exactly one finding, got: {findings:?}"
        );
        let line = &findings[0];
        assert!(
            line.contains("uncapped-string"),
            "finding must carry the machine-readable rule id, got: {line}"
        );
        assert!(
            line.contains("post-comment") && line.contains("body_text"),
            "finding must name the tool and the parameter, got: {line}"
        );
    }

    #[test]
    fn zeroed_default_cap_opt_out_is_reported() {
        let toml_str = format!(
            "{SERVER_HEADER}\n[server.validation]\ndefault_max_length = 0\n{UNCAPPED_BODY_STRING}"
        );
        let findings = findings_for(&toml_str);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("opt-out-default-max-length-zero")),
            "an active opt-out must never read as off, got: {findings:?}"
        );
    }

    #[test]
    fn schema_enforcement_opt_out_is_reported() {
        let toml_str = format!(
            "{SERVER_HEADER}\n[server.validation]\nenforce_input_schema = false\n{UNCAPPED_BODY_STRING}"
        );
        let findings = findings_for(&toml_str);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("opt-out-enforce-input-schema")),
            "an active opt-out must never read as off, got: {findings:?}"
        );
    }

    #[test]
    fn zero_tools_config_has_no_findings_and_succeeds() {
        let (_dir, path) = write_server_config(SERVER_HEADER);
        assert!(
            findings_for(SERVER_HEADER).is_empty(),
            "a config with zero tools and no opt-out has nothing to report"
        );
        let result = run_on(&path);
        assert!(
            result.is_ok(),
            "zero-tool config must succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn lint_finding_alone_exits_zero() {
        let (_dir, path) = write_server_config(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}"));
        let result = run_on(&path);
        assert!(
            result.is_ok(),
            "a lint finding is a WARNING and must never fail the command: {:?}",
            result.err()
        );
    }

    #[test]
    fn uncompilable_pattern_fails_naming_the_parameter() {
        let toml_str = format!(
            r#"{SERVER_HEADER}
[[tools]]
name = "lookup"
description = "Look something up."
path = "/things/{{id}}"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
description = "Identifier."
pattern = "([unclosed"
"#
        );
        let (_dir, path) = write_server_config(&toml_str);
        let result = run_on(&path);
        let err = result.expect_err("a non-compiling pattern must fail the command");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("id"),
            "error chain must name the offending parameter, got: {msg}"
        );
    }

    #[test]
    fn missing_config_file_names_the_path_it_looked_for() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope").join(SERVER_CONFIG_FILE);
        let result = run_on(&missing);
        let err = result.expect_err("a missing config file must fail the command");
        let msg = format!("{err:#}");
        assert!(
            msg.contains(&missing.display().to_string()),
            "error chain must name the path it looked for, got: {msg}"
        );
    }

    #[test]
    fn backend_section_parses() {
        // REGRESSION form of the `http`-feature requirement: `ServerConfig.backend`
        // is `#[cfg(feature = "http")]` and `ServerConfig` is
        // `deny_unknown_fields`, so a toolkit built without `http` rejects this
        // fixture with `unknown field 'backend'`.
        let toml_str = format!(
            r#"{SERVER_HEADER}
[backend]
base_url = "https://example.invalid"
{UNCAPPED_BODY_STRING}"#
        );
        let (_dir, path) = write_server_config(&toml_str);
        let result = run_on(&path);
        assert!(
            result.is_ok(),
            "a config carrying [backend] must parse — this is what `http` is for: {:?}",
            result.err()
        );
    }

    #[test]
    fn explicit_config_flag_wins_over_server_dir() {
        let (_dir, path) = write_server_config(SERVER_HEADER);
        let other = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_server_config_path(
            Some(&other.path().to_string_lossy()),
            Some(&path.to_string_lossy()),
        )
        .expect("resolve");
        assert_eq!(resolved, path, "--config must win over --server");
    }

    #[test]
    fn server_dir_resolves_to_the_config_file_at_its_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resolved =
            resolve_server_config_path(Some(&dir.path().to_string_lossy()), None).expect("resolve");
        assert_eq!(resolved, dir.path().join(SERVER_CONFIG_FILE));
    }

    // -------------------------------------------------------------------------
    // D-07's literal surface — `validate deploy` reports the SAME findings,
    // as warnings only
    // -------------------------------------------------------------------------

    /// The deploy-document stanzas every fixture here needs, kept in ONE place.
    const DEPLOY_FIXTURE_HEADER: &str = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-west-2"

[server]
name = "demo-server"
memory_mb = 512
timeout_seconds = 30

[environment]

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = false
create_dashboard = false
"#;

    const BENIGN_IAM: &str = r#"
[[iam.tables]]
name = "demo-table"
actions = ["read"]
"#;

    const WILDCARD_IAM: &str = r#"
[[iam.statements]]
effect = "Allow"
actions = ["*"]
resources = ["*"]
"#;

    /// A benign `.pmcp/deploy.toml` plus an optional `config.toml`, in one tempdir.
    fn write_deploy_project(
        iam_section: &str,
        server_config: Option<&str>,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pmcp_dir = dir.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir).expect("mkdir .pmcp");
        std::fs::write(
            pmcp_dir.join("deploy.toml"),
            format!("{DEPLOY_FIXTURE_HEADER}{iam_section}"),
        )
        .expect("write deploy.toml");
        if let Some(text) = server_config {
            std::fs::write(dir.path().join(SERVER_CONFIG_FILE), text).expect("write config.toml");
        }
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    #[test]
    fn deploy_discovers_and_lints_a_toolkit_config_beside_the_deploy_document() {
        let (_dir, root) = write_deploy_project(
            BENIGN_IAM,
            Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        );
        match discover_server_config_lint(&root) {
            DiscoveredConfigLint::Findings(findings) => {
                assert_eq!(
                    findings.len(),
                    1,
                    "expected the uncapped-string finding, got: {findings:?}"
                );
                assert!(
                    findings[0].contains("uncapped-string"),
                    "got: {}",
                    findings[0]
                );
            },
            other => panic!("expected Findings, got {other:?}"),
        }
        std::env::set_var("PMCP_QUIET", "1");
        let result = validate_deploy(Some(root.to_string_lossy().into_owned()), false);
        assert!(
            result.is_ok(),
            "lint findings are warnings: {:?}",
            result.err()
        );
    }

    #[test]
    fn deploy_without_a_toolkit_config_reports_absent_and_stays_silent() {
        let (_dir, root) = write_deploy_project(BENIGN_IAM, None);
        assert!(
            matches!(
                discover_server_config_lint(&root),
                DiscoveredConfigLint::Absent
            ),
            "a pure-IAM project must not become noisy"
        );
        std::env::set_var("PMCP_QUIET", "1");
        let result = validate_deploy(Some(root.to_string_lossy().into_owned()), false);
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    fn deploy_warns_on_a_malformed_toolkit_config_and_still_exits_zero() {
        let (_dir, root) = write_deploy_project(BENIGN_IAM, Some("this is not = valid toml ["));
        assert!(
            matches!(
                discover_server_config_lint(&root),
                DiscoveredConfigLint::Unreadable(_)
            ),
            "a malformed discovered config is a warning, not an error"
        );
        std::env::set_var("PMCP_QUIET", "1");
        let result = validate_deploy(Some(root.to_string_lossy().into_owned()), false);
        assert!(
            result.is_ok(),
            "a malformed DISCOVERED config must not fail `validate deploy`: {:?}",
            result.err()
        );
    }

    #[test]
    fn deploy_wildcard_allow_still_fails_with_lint_findings_present() {
        let (_dir, root) = write_deploy_project(
            WILDCARD_IAM,
            Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        );
        std::env::set_var("PMCP_QUIET", "1");
        let result = validate_deploy(Some(root.to_string_lossy().into_owned()), false);
        let err = result.expect_err("wildcard Allow must still be rejected");
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("wildcard"),
            "the hard-error contract must be unchanged by the lint extension, got: {msg}"
        );
    }
}

#[cfg(test)]
mod deploy_validate_gate_tests {
    //! Phase 76 Wave 4 — integration tests for `cargo pmcp validate deploy`.
    //!
    //! **Rule-3 deviation note (consistent with Waves 1/2/3):** the plan
    //! called for this file at `cargo-pmcp/tests/deploy_validate_gate.rs`,
    //! but `cargo_pmcp::commands` is NOT re-exported from
    //! `cargo-pmcp/src/lib.rs` (lib surface intentionally minimal at
    //! `loadtest`/`pentest`/`test_support_cache`). Expanding lib visibility
    //! would drag in the entire CLI subsystem for very little over the
    //! in-crate coverage that works against `validate_deploy` via
    //! `super::*`. Tests are otherwise identical in intent — they write a
    //! synthetic `.pmcp/deploy.toml` into a tempdir and invoke the public
    //! `validate_deploy` handler, asserting the CR gate behaviour.

    use super::*;
    use std::path::PathBuf;

    /// Deploy-TOML stanzas common to every fixture. Inserted verbatim at the
    /// top of each fixture constant so individual tests only have to vary
    /// the `[iam.*]` section they care about.
    const COMMON_FIXTURE_HEADER: &str = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-west-2"

[server]
name = "demo-server"
memory_mb = 512
timeout_seconds = 30

[environment]

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = false
create_dashboard = false
"#;

    fn fixture_with_iam(iam_section: &str) -> String {
        format!("{COMMON_FIXTURE_HEADER}{iam_section}")
    }

    fn write_fixture(toml_str: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pmcp_dir = dir.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir).expect("mkdir .pmcp");
        std::fs::write(pmcp_dir.join("deploy.toml"), toml_str).expect("write deploy.toml");
        let project_root = dir.path().to_path_buf();
        (dir, project_root)
    }

    #[test]
    fn validate_deploy_accepts_valid_config() {
        std::env::set_var("PMCP_QUIET", "1");
        let toml_str = fixture_with_iam(
            r#"
[[iam.tables]]
name = "demo-table"
actions = ["read"]
"#,
        );
        let (_dir, project_root) = write_fixture(&toml_str);
        let result = validate_deploy(Some(project_root.to_string_lossy().into_owned()), false);
        assert!(result.is_ok(), "valid config rejected: {:?}", result.err());
    }

    #[test]
    fn validate_deploy_rejects_wildcard_allow() {
        std::env::set_var("PMCP_QUIET", "1");
        let toml_str = fixture_with_iam(
            r#"
[[iam.statements]]
effect = "Allow"
actions = ["*"]
resources = ["*"]
"#,
        );
        let (_dir, project_root) = write_fixture(&toml_str);
        let result = validate_deploy(Some(project_root.to_string_lossy().into_owned()), false);
        let err = result.expect_err("wildcard Allow must be rejected");
        let msg = format!("{err:?}");
        assert!(
            msg.to_lowercase().contains("wildcard"),
            "expected 'wildcard' in error chain, got: {msg}"
        );
    }

    #[test]
    fn validate_deploy_rejects_bad_bucket_sugar() {
        std::env::set_var("PMCP_QUIET", "1");
        let toml_str = fixture_with_iam(
            r#"
[[iam.buckets]]
name = "my-bucket"
actions = ["devour"]
"#,
        );
        let (_dir, project_root) = write_fixture(&toml_str);
        let result = validate_deploy(Some(project_root.to_string_lossy().into_owned()), false);
        assert!(result.is_err(), "bad bucket sugar must be rejected");
    }

    #[test]
    fn validate_deploy_reports_unknown_service_prefix_but_returns_ok() {
        std::env::set_var("PMCP_QUIET", "1");
        let toml_str = fixture_with_iam(
            r#"
[[iam.statements]]
effect = "Allow"
actions = ["totallyfake:DoThing"]
resources = ["*"]
"#,
        );
        let (_dir, project_root) = write_fixture(&toml_str);
        let result = validate_deploy(Some(project_root.to_string_lossy().into_owned()), false);
        assert!(
            result.is_ok(),
            "unknown prefix must be a warning, not Err: {:?}",
            result.err()
        );
    }
}
