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

        /// Run in verbose mode (show all test output)
        #[arg(short, long)]
        verbose: bool,

        /// Server directory to validate (defaults to current directory)
        #[arg(long)]
        server: Option<String>,
    },
}

impl ValidateCommand {
    pub fn execute(self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        // quiet mode conveyed via PMCP_QUIET env var for downstream functions
        let _ = global_flags;
        match self {
            ValidateCommand::Workflows {
                generate,
                verbose,
                server,
            } => validate_workflows(generate, verbose, server),
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
    // Step 1: Run cargo check
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

    // Step 2: Check for workflow tests
    if not_quiet {
        println!(
            "\n{} Looking for workflow validation tests...",
            style("Step 2:").bold()
        );
    }

    let test_patterns = find_workflow_tests()?;

    if test_patterns.is_empty() {
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
        } else {
            if not_quiet {
                println!(
                    "  {} No workflow validation tests found",
                    style("!").yellow()
                );
                print_test_guidance(not_quiet);
            }
            return Ok(());
        }
    }

    if not_quiet {
        println!(
            "  {} Found {} workflow test pattern(s)",
            style("✓").green(),
            test_patterns.len()
        );
    }

    // Step 3: Run workflow tests
    if not_quiet {
        println!(
            "\n{} Running workflow validation tests...",
            style("Step 3:").bold()
        );
    }

    let mut all_passed = true;
    let mut total_tests = 0;
    let mut passed_tests = 0;

    for pattern in &test_patterns {
        let test_output = Command::new("cargo")
            .args(["test", pattern, "--", "--nocapture"])
            .output()
            .context("Failed to run cargo test")?;

        let stdout = String::from_utf8_lossy(&test_output.stdout);
        let stderr = String::from_utf8_lossy(&test_output.stderr);

        // Parse test results
        let (tests_run, tests_passed, tests_failed) = parse_test_output(&stdout, &stderr);
        total_tests += tests_run;
        passed_tests += tests_passed;

        if verbose {
            println!("{}", stdout);
            if !stderr.is_empty() {
                eprintln!("{}", stderr);
            }
        }

        if tests_failed > 0 {
            all_passed = false;
            if not_quiet {
                println!(
                    "  {} Pattern '{}': {} passed, {} failed",
                    style("✗").red(),
                    pattern,
                    tests_passed,
                    style(tests_failed).red()
                );
            }

            // Show failure details
            if !verbose {
                print_failure_summary(&stdout, &stderr);
            }
        } else if tests_run > 0 {
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
    }

    // Summary
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
    } else if total_tests == 0 {
        if not_quiet {
            println!(
                "{} No workflow tests were executed",
                style("!").yellow().bold()
            );
            print_test_guidance(not_quiet);
        }
    } else {
        println!(
            "{} Workflow validation failed: {} of {} tests passed",
            style("✗").red().bold(),
            passed_tests,
            total_tests
        );
        return Err(anyhow::anyhow!("Workflow validation failed"));
    }

    Ok(())
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
    // Look for "test result: ok. X passed; Y failed" pattern
    let combined = format!("{}\n{}", stdout, stderr);

    for line in combined.lines() {
        if line.starts_with("test result:") {
            let passed = line
                .split_whitespace()
                .find_map(|word| word.strip_suffix(" passed").and_then(|n| n.parse().ok()))
                .or_else(|| {
                    // Try another pattern: "X passed"
                    let parts: Vec<&str> = line.split(';').collect();
                    for part in parts {
                        if part.contains("passed") {
                            return part.split_whitespace().next().and_then(|n| n.parse().ok());
                        }
                    }
                    None
                })
                .unwrap_or(0);

            let failed = line
                .split_whitespace()
                .find_map(|word| word.strip_suffix(" failed").and_then(|n| n.parse().ok()))
                .or_else(|| {
                    let parts: Vec<&str> = line.split(';').collect();
                    for part in parts {
                        if part.contains("failed") {
                            return part.split_whitespace().next().and_then(|n| n.parse().ok());
                        }
                    }
                    None
                })
                .unwrap_or(0);

            return (passed + failed, passed, failed);
        }
    }

    // Alternative: count individual test lines
    let passed = combined.matches("... ok").count();
    let failed = combined.matches("... FAILED").count();

    (passed + failed, passed, failed)
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
