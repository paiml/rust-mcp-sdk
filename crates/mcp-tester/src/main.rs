use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::time::Duration;

mod diagnostics;
mod report;
mod scenario;
mod scenario_executor;
mod scenario_generator;
mod tester;
mod validators;

use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};
use report::{OutputFormat, TestReport};
use tester::ServerTester;

#[derive(Parser)]
#[command(name = "mcp-tester")]
#[command(about = "Comprehensive MCP server testing and validation tool")]
#[command(
    long_about = "The MCP Server Tester is a powerful tool for testing, validating, and exploring MCP servers.

Key Features:
• Protocol compliance validation with detailed error reporting
• Cursor IDE & Claude Desktop compatibility testing
• Tool discovery with JSON schema validation and warnings
• Resource and prompt testing with metadata validation
• Automated test scenario generation from server capabilities
• Performance benchmarking and comparison between servers
• Health monitoring and diagnostics
• Support for multiple transport types (HTTP, stdio, JSON-RPC)"
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(short, long, global = true, default_value = "pretty")]
    format: OutputFormat,

    /// Verbosity level (0-3)
    #[arg(short, long, global = true, default_value = "0")]
    verbose: u8,

    /// Connection timeout in seconds
    #[arg(long, global = true, default_value = "30")]
    timeout: u64,

    /// Skip TLS certificate verification (insecure)
    #[arg(long, global = true)]
    insecure: bool,

    /// API key for authentication (if required)
    #[arg(long, global = true, env = "MCP_API_KEY")]
    api_key: Option<String>,

    /// Force specific transport type (http|stdio|jsonrpc)
    #[arg(long, global = true)]
    transport: Option<String>,

    /// OAuth issuer URL (optional - will auto-discover from server if not provided)
    #[arg(long, global = true, env = "MCP_OAUTH_ISSUER")]
    oauth_issuer: Option<String>,

    /// OAuth client ID (required for OAuth authentication)
    #[arg(long, global = true, env = "MCP_OAUTH_CLIENT_ID")]
    oauth_client_id: Option<String>,

    /// OAuth scopes (comma-separated, default: openid)
    #[arg(long, global = true, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    oauth_scopes: Option<Vec<String>>,

    /// Disable OAuth token caching
    #[arg(long, global = true)]
    oauth_no_cache: bool,

    /// OAuth redirect port for localhost callback (default: 8080)
    #[arg(
        long,
        global = true,
        env = "MCP_OAUTH_REDIRECT_PORT",
        default_value = "8080"
    )]
    oauth_redirect_port: u16,
}

#[derive(Subcommand)]
enum Commands {
    /// Run full test suite
    Test {
        /// Server URL (http://localhost:8080 or stdio)
        url: String,

        /// Test specific tools
        #[arg(long)]
        with_tools: bool,

        /// Specific tool to test
        #[arg(long)]
        tool: Option<String>,

        /// Tool arguments as JSON
        #[arg(long)]
        args: Option<String>,
    },

    /// Quick connectivity check
    Quick {
        /// Server URL
        url: String,
    },

    /// Protocol compliance validation
    Compliance {
        /// Server URL
        url: String,

        /// Strict mode (fail on warnings)
        #[arg(long)]
        strict: bool,
    },

    /// List and test available tools
    Tools {
        /// Server URL
        url: String,

        /// Test each tool with sample data
        #[arg(long)]
        test_all: bool,
    },

    /// List and test available resources
    Resources {
        /// Server URL
        url: String,
    },

    /// List and test available prompts
    Prompts {
        /// Server URL
        url: String,
    },

    /// Connection diagnostics
    Diagnose {
        /// Server URL
        url: String,

        /// Include network diagnostics
        #[arg(long)]
        network: bool,
    },

    /// Compare two servers
    Compare {
        /// First server URL
        server1: String,

        /// Second server URL
        server2: String,

        /// Include performance comparison
        #[arg(long)]
        with_perf: bool,
    },

    /// Server health check
    Health {
        /// Server URL
        url: String,
    },

    /// Run test scenarios from file
    Scenario {
        /// Server URL
        url: String,

        /// Path to scenario file (YAML or JSON)
        file: String,

        /// Show detailed output for scenario execution
        #[arg(long, alias = "verbose")]
        detailed: bool,
    },

    /// Generate a scenario file from server's tools
    GenerateScenario {
        /// Server URL
        url: String,

        /// Output file path (defaults to generated_scenario.yaml)
        #[arg(short, long, default_value = "generated_scenario.yaml")]
        output: String,

        /// Include all discovered tools (not just first few)
        #[arg(long)]
        all_tools: bool,

        /// Include resources in scenario
        #[arg(long)]
        with_resources: bool,

        /// Include prompts in scenario
        #[arg(long)]
        with_prompts: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging - respect RUST_LOG if set, otherwise use verbosity
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::from_default_env()
    } else {
        let log_level = match cli.verbose {
            0 => "error",
            1 => "warn",
            2 => "info",
            3 => "debug",
            _ => "trace",
        };
        tracing_subscriber::EnvFilter::new(format!(
            "mcp_server_tester={},pmcp={}",
            log_level, log_level
        ))
    };

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    // Print header
    if matches!(cli.format, OutputFormat::Pretty) {
        print_header();
    }

    // OAuth config will be created per-command with the actual URL
    let oauth_config = (
        cli.oauth_issuer.clone(),
        cli.oauth_client_id.clone(),
        cli.oauth_scopes.clone(),
        cli.oauth_no_cache,
        cli.oauth_redirect_port,
    );

    // Execute command
    let result = match cli.command {
        Commands::Test {
            url,
            with_tools,
            tool,
            args,
        } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_full_test(
                &url,
                with_tools,
                tool,
                args,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware,
            )
            .await
        },

        Commands::Quick { url } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_quick_test(
                &url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware,
            )
            .await
        },

        Commands::Compliance { url, strict } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_compliance_test(
                &url,
                strict,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Tools { url, test_all } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_tools_test(
                &url,
                test_all,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Resources { url } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_resources_test(
                &url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Prompts { url } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_prompts_test(
                &url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Diagnose { url, network } => {
            // Keep JSON/minimal outputs clean by running diagnostics in quiet mode
            let use_quiet = !matches!(cli.format, OutputFormat::Pretty | OutputFormat::Verbose);
            let report = if use_quiet {
                diagnostics::run_diagnostics_quiet(
                    &url,
                    network,
                    Duration::from_secs(cli.timeout),
                    cli.insecure,
                    cli.api_key.as_deref(),
                )
                .await?
            } else {
                diagnostics::run_diagnostics(
                    &url,
                    network,
                    Duration::from_secs(cli.timeout),
                    cli.insecure,
                    cli.api_key.as_deref(),
                )
                .await?
            };
            Ok(report)
        },

        Commands::Compare {
            server1,
            server2,
            with_perf,
        } => {
            let oauth_middleware = create_oauth_from_config(&server1, &oauth_config).await?;
            run_comparison(
                &server1,
                &server2,
                with_perf,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Health { url } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_health_check(
                &url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::Scenario {
            url,
            file,
            detailed,
        } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            run_scenario(
                &url,
                &file,
                detailed,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware.clone(),
            )
            .await
        },

        Commands::GenerateScenario {
            url,
            output,
            all_tools,
            with_resources,
            with_prompts,
        } => {
            let oauth_middleware = create_oauth_from_config(&url, &oauth_config).await?;
            generate_scenario(
                &url,
                &output,
                all_tools,
                with_resources,
                with_prompts,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth_middleware.clone(),
            )
            .await
        },
    };

    // Handle results and output
    match result {
        Ok(report) => {
            report.print(cli.format);
            // Always use non-zero exit code on failures for all formats (CI-friendly)
            if report.has_failures() {
                std::process::exit(1);
            }
        },
        Err(e) => {
            if cli.format == OutputFormat::Json {
                let error_report = TestReport::from_error(e);
                error_report.print(OutputFormat::Json);
            } else {
                eprintln!("{} {:#}", "Error:".red().bold(), e);
            }
            std::process::exit(1);
        },
    }

    Ok(())
}

fn print_header() {
    println!(
        "{}",
        "╔════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║              MCP SERVER TESTING TOOL v0.1.0               ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
}

/// OAuth configuration tuple type
type OAuthConfigTuple = (
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    bool,
    u16,
);

/// Helper to create OAuth middleware from config tuple
async fn create_oauth_from_config(
    url: &str,
    config: &OAuthConfigTuple,
) -> Result<Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>> {
    create_oauth_middleware(
        url,
        config.0.clone(),
        config.1.clone(),
        config.2.clone(),
        config.3,
        config.4,
    )
    .await
}

/// Create OAuth middleware chain from CLI configuration
async fn create_oauth_middleware(
    mcp_server_url: &str,
    oauth_issuer: Option<String>,
    oauth_client_id: Option<String>,
    oauth_scopes: Option<Vec<String>>,
    no_cache: bool,
    redirect_port: u16,
) -> Result<Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>> {
    // Check if OAuth is configured (requires at minimum client_id)
    let client_id = match oauth_client_id {
        Some(id) => id,
        None => {
            // No OAuth configured
            if oauth_issuer.is_some() {
                eprintln!(
                    "{}",
                    "Warning: --oauth-issuer provided but --oauth-client-id missing. OAuth disabled."
                        .yellow()
                );
            }
            return Ok(None);
        },
    };

    let scopes = oauth_scopes.unwrap_or_else(|| vec!["openid".to_string()]);

    let cache_file = if no_cache {
        None
    } else {
        Some(default_cache_path())
    };

    let config = OAuthConfig {
        issuer: oauth_issuer,
        mcp_server_url: Some(mcp_server_url.to_string()),
        client_id,
        scopes,
        cache_file,
        redirect_port,
    };

    let oauth_helper = OAuthHelper::new(config)?;
    let middleware_chain = oauth_helper.create_middleware_chain().await?;

    Ok(Some(middleware_chain))
}

#[allow(clippy::too_many_arguments)]
async fn run_full_test(
    url: &str,
    with_tools: bool,
    tool: Option<String>,
    args: Option<String>,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Intentionally no unconditional prints here to keep JSON/minimal output clean

    // Run all test categories
    let mut report = tester.run_full_suite(with_tools).await?;

    // Test specific tool if requested
    if let Some(tool_name) = tool {
        // Optional: testing a specific tool (no unconditional prints)
        let tool_args = if let Some(args_str) = args {
            serde_json::from_str(&args_str).context("Invalid JSON arguments")?
        } else {
            serde_json::Value::Null
        };

        let tool_result = tester.test_tool(&tool_name, tool_args).await?;
        report.add_test(tool_result);
    }

    Ok(report)
}

async fn run_quick_test(
    url: &str,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Intentionally no unconditional prints here to keep JSON/minimal output clean

    tester.run_quick_test().await
}

async fn run_compliance_test(
    url: &str,
    strict: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Intentionally no unconditional prints here to keep JSON/minimal output clean

    tester.run_compliance_tests(strict).await
}

#[allow(clippy::too_many_arguments)]
async fn run_tools_test(
    url: &str,
    test_all: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    verbose: bool,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Intentionally no unconditional prints here to keep JSON/minimal output clean
    println!();

    // Pass verbose flag to the tester for detailed output
    tester
        .run_tools_discovery_with_verbose(test_all, verbose)
        .await
}

#[allow(clippy::too_many_arguments)]
async fn run_resources_test(
    url: &str,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    verbose: bool,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    if verbose {
        println!("{}", "Discovering and testing resources...".green());
        println!();
        println!("Connecting to {}...", url);
    }

    tester.run_resources_discovery_with_verbose(verbose).await
}

#[allow(clippy::too_many_arguments)]
async fn run_prompts_test(
    url: &str,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    verbose: bool,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    if verbose {
        println!("{}", "Discovering and testing prompts...".green());
        println!();
        println!("Connecting to {}...", url);
    }

    tester.run_prompts_discovery().await
}

#[allow(dead_code)]
async fn run_diagnostics(
    url: &str,
    network: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
) -> Result<TestReport> {
    // No unconditional prints to keep JSON/minimal output clean

    let report = diagnostics::run_diagnostics(
        url,
        network,
        Duration::from_secs(timeout),
        insecure,
        api_key,
    )
    .await?;

    Ok(report)
}

#[allow(clippy::too_many_arguments)]
async fn run_comparison(
    server1: &str,
    server2: &str,
    with_perf: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    // No unconditional prints to keep JSON/minimal output clean

    let mut tester1 = ServerTester::new(
        server1,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware.clone(),
    )?;
    let mut tester2 = ServerTester::new(
        server2,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    let report = tester1.compare_with(&mut tester2, with_perf).await?;

    Ok(report)
}

async fn run_health_check(
    url: &str,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // No unconditional prints to keep JSON/minimal output clean

    tester.run_health_check().await
}

#[allow(clippy::too_many_arguments)]
async fn generate_scenario(
    url: &str,
    output: &str,
    all_tools: bool,
    with_resources: bool,
    with_prompts: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    use scenario_generator::ScenarioGenerator;

    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    let generator =
        ScenarioGenerator::new(url.to_string(), all_tools, with_resources, with_prompts);

    generator.generate(&mut tester, output).await?;

    // Return a simple success report
    let mut report = TestReport::new();
    report.add_test(crate::report::TestResult {
        name: "Generate Scenario".to_string(),
        category: crate::report::TestCategory::Core,
        status: crate::report::TestStatus::Passed,
        duration: Duration::from_secs(0),
        error: None,
        details: Some(format!("Scenario generated successfully: {}", output)),
    });

    Ok(report)
}

#[allow(clippy::too_many_arguments)]
async fn run_scenario(
    url: &str,
    file: &str,
    verbose: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    use scenario::TestScenario;
    use scenario_executor::ScenarioExecutor;

    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Initialize the server first (no unconditional prints)
    let init_report = tester.run_quick_test().await?;
    if init_report.has_failures() {
        return Ok(init_report);
    }

    // Load the scenario file
    // No unconditional prints to keep JSON/minimal output clean
    let scenario = TestScenario::from_file(file).context("Failed to load scenario file")?;

    // Execute the scenario
    let mut executor = ScenarioExecutor::new(&mut tester, verbose);
    let scenario_result = executor.execute(scenario).await?;

    // Convert scenario result to test report
    let mut report = TestReport::new();

    for step_result in scenario_result.step_results {
        let test_result = crate::report::TestResult {
            name: step_result.step_name,
            category: crate::report::TestCategory::Tools,
            status: if step_result.success {
                crate::report::TestStatus::Passed
            } else {
                crate::report::TestStatus::Failed
            },
            duration: step_result.duration,
            error: step_result.error,
            details: step_result.response.map(|r| r.to_string()),
        };
        report.add_test(test_result);
    }

    if let Some(error) = scenario_result.error {
        report.add_test(crate::report::TestResult {
            name: "Scenario Execution".to_string(),
            category: crate::report::TestCategory::Core,
            status: crate::report::TestStatus::Failed,
            duration: scenario_result.duration,
            error: Some(error),
            details: None,
        });
    }

    Ok(report)
}
