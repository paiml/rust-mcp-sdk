//! MCP Server Testing Library
//!
//! This crate provides comprehensive testing capabilities for MCP servers.
//! It can be used as both a standalone CLI tool (`mcp-tester`) and as a library.
//!
//! # Examples
//!
//! ## Generate Test Scenarios
//!
//! ```no_run
//! use mcp_tester::{generate_scenarios, GenerateOptions};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let options = GenerateOptions {
//!         all_tools: true,
//!         with_resources: true,
//!         with_prompts: true,
//!     };
//!
//!     generate_scenarios(
//!         "http://localhost:3000",
//!         "test_scenario.yaml",
//!         options
//!     ).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Run Test Scenarios
//!
//! ```no_run
//! use mcp_tester::run_scenario;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     run_scenario(
//!         "test_scenario.yaml",
//!         "http://localhost:3000",
//!         false // detailed output
//!     ).await?;
//!
//!     Ok(())
//! }
//! ```

// Re-export key modules
pub mod diagnostics;
pub mod report;
pub mod scenario;
pub mod scenario_executor;
pub mod scenario_generator;
pub mod tester;
pub mod validators;

// OAuth support -- re-exported from core SDK
pub use pmcp::client::oauth;
pub use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
pub use report::{OutputFormat, TestReport, TestResult, TestStatus};
pub use scenario::TestScenario;
pub use scenario_executor::ScenarioExecutor;
pub use scenario_generator::ScenarioGenerator;
pub use tester::ServerTester;

use anyhow::{Context, Result};
use std::time::Duration;

/// Options for generating test scenarios
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Include all discovered tools in scenarios
    pub all_tools: bool,
    /// Include resource testing
    pub with_resources: bool,
    /// Include prompt testing
    pub with_prompts: bool,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            all_tools: true,
            with_resources: false,
            with_prompts: false,
        }
    }
}

/// Generate test scenarios from server capabilities
///
/// This function connects to an MCP server, discovers its capabilities,
/// and generates comprehensive test scenarios in YAML format.
///
/// # Arguments
///
/// * `server_url` - URL of the MCP server to test
/// * `output_path` - Path where the generated scenario file will be written
/// * `options` - Configuration for scenario generation
///
/// # Example
///
/// ```no_run
/// use mcp_tester::{generate_scenarios, GenerateOptions};
///
/// # async fn example() -> anyhow::Result<()> {
/// generate_scenarios(
///     "http://localhost:3000",
///     "scenarios/my_server.yaml",
///     GenerateOptions::default()
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn generate_scenarios(
    server_url: &str,
    output_path: &str,
    options: GenerateOptions,
) -> Result<()> {
    generate_scenarios_with_transport(server_url, output_path, options, None).await
}

/// Generate test scenarios with explicit transport type
///
/// # Arguments
///
/// * `server_url` - URL of the MCP server to test
/// * `output_path` - Path where the generated scenario file will be written
/// * `options` - Configuration for scenario generation
/// * `transport` - Transport type: "http" (SSE), "jsonrpc" (POST), "stdio", or None for auto-detect
pub async fn generate_scenarios_with_transport(
    server_url: &str,
    output_path: &str,
    options: GenerateOptions,
    transport: Option<&str>,
) -> Result<()> {
    // Create a server tester
    let mut tester = ServerTester::new(
        server_url,
        Duration::from_secs(30),
        false,     // insecure
        None,      // api_key
        transport, // transport type
        None,      // http_middleware_chain
    )?;

    // Create generator
    let generator = ScenarioGenerator::new(
        server_url.to_string(),
        options.all_tools,
        options.with_resources,
        options.with_prompts,
    );

    // Generate scenarios
    generator.generate(&mut tester, output_path).await
}

/// Run a test scenario from a file
///
/// # Arguments
///
/// * `scenario_path` - Path to the scenario YAML/JSON file
/// * `server_url` - URL of the MCP server to test
/// * `detailed` - Whether to show detailed step-by-step output
///
/// # Example
///
/// ```no_run
/// use mcp_tester::run_scenario;
///
/// # async fn example() -> anyhow::Result<()> {
/// run_scenario(
///     "scenarios/my_test.yaml",
///     "http://localhost:3000",
///     true // show detailed output
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_scenario(scenario_path: &str, server_url: &str, detailed: bool) -> Result<()> {
    run_scenario_with_transport(scenario_path, server_url, detailed, None).await
}

/// Run a test scenario with explicit transport type
///
/// # Arguments
///
/// * `scenario_path` - Path to the scenario YAML/JSON file
/// * `server_url` - URL of the MCP server to test
/// * `detailed` - Whether to show detailed step-by-step output
/// * `transport` - Transport type: "http" (SSE), "jsonrpc" (POST), "stdio", or None for auto-detect
pub async fn run_scenario_with_transport(
    scenario_path: &str,
    server_url: &str,
    detailed: bool,
    transport: Option<&str>,
) -> Result<()> {
    use colored::*;

    // Load scenario
    let scenario_content = std::fs::read_to_string(scenario_path)
        .with_context(|| format!("Failed to read scenario file: {}", scenario_path))?;

    let scenario: TestScenario = if scenario_path.ends_with(".json") {
        serde_json::from_str(&scenario_content)?
    } else {
        serde_yaml::from_str(&scenario_content)?
    };

    // Create a server tester
    let mut tester = ServerTester::new(
        server_url,
        Duration::from_secs(30),
        false,     // insecure
        None,      // api_key
        transport, // transport type
        None,      // http_middleware_chain
    )?;

    // Execute scenario
    let mut executor = ScenarioExecutor::new(&mut tester, detailed);

    if detailed {
        println!(
            "\n{}",
            "Running scenario with detailed output..."
                .bright_cyan()
                .bold()
        );
    }

    let result = executor.execute(scenario).await?;

    // Report results
    if result.success {
        println!("\n{} Scenario passed!", "✓".green().bold());
        Ok(())
    } else {
        println!("\n{} Scenario failed!", "✗".red().bold());
        anyhow::bail!("Scenario execution failed");
    }
}

/// Create a server tester instance
///
/// This is useful for more advanced testing scenarios where you want
/// full control over the testing process.
///
/// # Example
///
/// ```no_run
/// use mcp_tester::create_tester;
///
/// # fn example() -> anyhow::Result<()> {
/// let tester = create_tester(
///     "http://localhost:3000",
///     30, // timeout in seconds
///     false, // don't skip TLS verification
///     None, // no API key
/// )?;
///
/// // Use tester for custom testing
/// # Ok(())
/// # }
/// ```
pub fn create_tester(
    url: &str,
    timeout_secs: u64,
    insecure: bool,
    api_key: Option<String>,
) -> Result<ServerTester> {
    ServerTester::new(
        url,
        Duration::from_secs(timeout_secs),
        insecure,
        api_key.as_deref(),
        None, // transport
        None, // http_middleware_chain
    )
}
