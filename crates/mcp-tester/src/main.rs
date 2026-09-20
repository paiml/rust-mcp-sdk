use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::time::Duration;

mod app_validator;
mod diagnostics;
// The dual-run pair (Phase 117): `conformance::ConformanceRunner::run_dual`
// needs both, so the binary must declare them alongside the library barrel.
//
// Why the allow: a Rust binary and its sibling library are SEPARATE crates, so
// this module is compiled twice and dead-code analysis runs independently on
// each. The library exports the whole surface (`load_baseline`,
// `default_baseline_path`, the `EraBaseline` accessors); the binary reaches
// only the `--dual-run` path through it. Without the allow the binary reports
// every library-only item as dead — which is not a defect, it is the other
// crate's entry point. Scoped to these two modules, never crate-wide.
#[allow(dead_code)]
mod era_diff;
#[allow(dead_code)]
mod era_observations;
mod report;
mod scenario;
mod scenario_executor;
mod scenario_generator;
mod tester;
mod validators;

mod conformance;

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

    /// Dump every HTTP request and response this tool puts on the wire.
    ///
    /// Answers "what did we actually send?" — the first question in any
    /// conformance dispute, and one this tool previously could not answer about
    /// itself. Shows the request line, the headers (including the v2 routing
    /// trio `MCP-Protocol-Version` / `Mcp-Method` / `Mcp-Name`) and the body,
    /// plus the response status and headers.
    ///
    /// Credential and session headers are REDACTED — a wire dump is exactly the
    /// artifact that ends up pasted into a bug report.
    ///
    /// This is a preset over the SDK's `pmcp::wire` tracing target, not a
    /// separate logger, so the equivalent without the flag is
    /// `RUST_LOG=pmcp::wire=debug`, and any `tracing_subscriber` layer (JSON for
    /// CI, for instance) composes with it.
    #[arg(long, global = true)]
    dump_wire: bool,

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

    /// MCP protocol conformance validation
    ///
    /// Validates a server against the MCP protocol spec (2025-11-25).
    /// Tests 5 domains: Core (initialize, version, errors), Tools (list, call),
    /// Resources (list, read), Prompts (list, get), Tasks (lifecycle).
    /// Each domain reports independently -- a server with no resources still
    /// passes if it correctly reports empty capabilities.
    Conformance {
        /// Server URL
        url: String,

        /// Strict mode (promote warnings to failures)
        #[arg(long)]
        strict: bool,

        /// Run only specific domains (comma-separated: core,tools,resources,prompts,tasks)
        #[arg(long, value_delimiter = ',')]
        domain: Option<Vec<String>>,

        /// Detect whether the server serves BOTH MCP eras and, if so, run the
        /// suite twice and print a v1-vs-v2 comparison.
        ///
        /// OFF by default. With the flag absent, nothing about the output
        /// changes — single-run output stays byte-identical to 0.7.0, which is
        /// the additivity contract `tests/report_compat.rs` pins.
        ///
        /// Differences are classified against the checked-in expected-difference
        /// baseline (`baselines/era-deltas.yaml`): a listed delta is correct by
        /// design, an unlisted one is a finding, and a listed one that no longer
        /// reproduces is also a finding. Against a server that serves only one
        /// era this degrades to a single run and says so.
        #[arg(long)]
        dual_run: bool,

        /// Make `--dual-run`'s findings GATE the exit code.
        ///
        /// OFF by default, and that default is a deliberate contract: without
        /// this flag the exit code keeps meaning "did the v1 suite pass", so
        /// adding `--dual-run` to an existing CI job cannot change its verdict.
        ///
        /// With it, a v2 suite failure or an UNEXPECTED era difference exits
        /// non-zero. Use it when you WANT the era comparison to be a gate rather
        /// than a report — an opt-in flag whose findings cannot fail a job
        /// cannot gate anything.
        ///
        /// Requires `--dual-run`; on its own it does nothing.
        #[arg(long, requires = "dual_run")]
        fail_on_era_findings: bool,
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

    /// Validate MCP App metadata compliance
    Apps {
        /// Server URL
        url: String,

        /// Validation mode: standard, chatgpt, claude-desktop
        #[arg(long, default_value = "standard")]
        mode: String,

        /// Test specific tool only
        #[arg(long)]
        tool: Option<String>,

        /// Strict mode (promote warnings to failures)
        #[arg(long)]
        strict: bool,
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
    init_tracing(cli.verbose, cli.dump_wire);

    if matches!(cli.format, OutputFormat::Pretty) {
        print_header();
    }

    let oauth_config = (
        cli.oauth_issuer.clone(),
        cli.oauth_client_id.clone(),
        cli.oauth_scopes.clone(),
        cli.oauth_no_cache,
        cli.oauth_redirect_port,
    );

    let result = dispatch_command(&cli, oauth_config).await;
    handle_command_result(result, cli.format);
    Ok(())
}

/// Initialize tracing/logging from RUST_LOG, `--verbose`, or `--dump-wire`.
///
/// `--dump-wire` is a PRESET, not a parallel logger: it adds the SDK's
/// `pmcp::wire` target at `debug` to whatever filter is already in force. That
/// composition is the point —
///
/// - `--dump-wire` alone gives wire frames and nothing else, so the signal is
///   not buried under unrelated SDK debug output;
/// - `RUST_LOG=...` still wins for anything else, and a user who already knows
///   `RUST_LOG=pmcp::wire=debug` needs no flag at all;
/// - a CI job can swap in `tracing_subscriber`'s JSON layer and archive the
///   frames as a machine-readable artifact, because these are ordinary tracing
///   events rather than `println!`.
fn init_tracing(verbose: u8, dump_wire: bool) {
    let wire_directive = format!("{}=debug", pmcp::shared::wire_trace::WIRE_TARGET);
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        let base = tracing_subscriber::EnvFilter::from_default_env();
        if dump_wire {
            // ADD to the user's filter rather than replacing it: an explicit
            // RUST_LOG is a deliberate choice and the flag is additive to it.
            base.add_directive(
                wire_directive
                    .parse()
                    .expect("the wire directive is a compile-time constant"),
            )
        } else {
            base
        }
    } else if dump_wire {
        // Wire frames ONLY. Turning on `pmcp=debug` wholesale here would bury
        // the frames the user asked for under every other SDK debug line.
        tracing_subscriber::EnvFilter::new(wire_directive)
    } else {
        let log_level = match verbose {
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
}

/// Dispatch the parsed `Commands` variant to its `run_*` handler.
///
/// The command match arms collapse to one helper per command — the common
/// pattern is "build OAuth middleware then call the run_<cmd> async fn".
/// Diagnose has its own non-OAuth shape (handled in run_diagnose_command).
async fn dispatch_command(cli: &Cli, oauth_config: OAuthConfigTuple) -> Result<TestReport> {
    match &cli.command {
        Commands::Test {
            url,
            with_tools,
            tool,
            args,
        } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_full_test(
                url,
                *with_tools,
                tool.clone(),
                args.clone(),
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth,
            )
            .await
        },
        Commands::Quick { url } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_quick_test(
                url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth,
            )
            .await
        },
        Commands::Conformance {
            url,
            strict,
            domain,
            dual_run,
            fail_on_era_findings,
        } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_conformance_command(
                cli,
                url,
                *strict,
                domain.clone(),
                *dual_run,
                *fail_on_era_findings,
                oauth,
            )
            .await
        },
        Commands::Tools { url, test_all } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_tools_test(
                url,
                *test_all,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth.clone(),
            )
            .await
        },
        Commands::Resources { url } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_resources_test(
                url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth.clone(),
            )
            .await
        },
        Commands::Prompts { url } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_prompts_test(
                url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                cli.verbose > 0,
                oauth.clone(),
            )
            .await
        },
        Commands::Diagnose { url, network } => run_diagnose_command(cli, url, *network).await,
        Commands::Compare {
            server1,
            server2,
            with_perf,
        } => {
            let oauth = create_oauth_from_config(server1, &oauth_config).await?;
            run_comparison(
                server1,
                server2,
                *with_perf,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth.clone(),
            )
            .await
        },
        Commands::Health { url } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_health_check(
                url,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth.clone(),
            )
            .await
        },
        Commands::Scenario {
            url,
            file,
            detailed,
        } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_scenario(
                url,
                file,
                *detailed,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth.clone(),
            )
            .await
        },
        Commands::Apps {
            url,
            mode,
            tool,
            strict,
        } => {
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            run_apps_validation(
                url,
                mode,
                tool.clone(),
                *strict,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth,
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
            let oauth = create_oauth_from_config(url, &oauth_config).await?;
            generate_scenario(
                url,
                output,
                *all_tools,
                *with_resources,
                *with_prompts,
                cli.timeout,
                cli.insecure,
                cli.api_key.as_deref(),
                cli.transport.as_deref(),
                oauth.clone(),
            )
            .await
        },
    }
}

/// Run the `Conformance` subcommand, choosing the single-era or the 117-11
/// `--dual-run` orchestrator.
///
/// Extracted from [`dispatch_command`] for the reason `run_diagnose_command`
/// already was: the era branch is the one arm with its own control flow, and
/// inline it put the dispatcher at cognitive **25** against CI's PR-blocking
/// `pmat quality-gate --checks complexity` threshold of 23 (pmat 3.15.0). Note
/// `make quality-gate` does NOT run pmat — per Phase 75 D-07 pmat is CI-only to
/// keep the dev loop fast — so this class of regression is invisible locally and
/// is caught only on a PR. Do not inline it back.
async fn run_conformance_command(
    cli: &Cli,
    url: &str,
    strict: bool,
    domain: Option<Vec<String>>,
    dual_run: bool,
    fail_on_era_findings: bool,
    oauth: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    if dual_run {
        run_dual_conformance_test(
            url,
            strict,
            domain,
            cli.timeout,
            cli.insecure,
            cli.api_key.as_deref(),
            oauth,
            cli.format,
            fail_on_era_findings,
        )
        .await
    } else {
        run_conformance_test(
            url,
            strict,
            domain,
            cli.timeout,
            cli.insecure,
            cli.api_key.as_deref(),
            cli.transport.as_deref(),
            oauth,
        )
        .await
    }
}

/// Run the `Diagnose` subcommand, choosing quiet vs full mode by output format.
async fn run_diagnose_command(cli: &Cli, url: &str, network: bool) -> Result<TestReport> {
    let use_quiet = !matches!(cli.format, OutputFormat::Pretty | OutputFormat::Verbose);
    if use_quiet {
        diagnostics::run_diagnostics_quiet(
            url,
            network,
            Duration::from_secs(cli.timeout),
            cli.insecure,
            cli.api_key.as_deref(),
        )
        .await
    } else {
        diagnostics::run_diagnostics(
            url,
            network,
            Duration::from_secs(cli.timeout),
            cli.insecure,
            cli.api_key.as_deref(),
        )
        .await
    }
}

/// Print the result, exit non-zero on failure (CI-friendly).
fn handle_command_result(result: Result<TestReport>, format: OutputFormat) {
    match result {
        Ok(report) => {
            report.print(format);
            if report.has_failures() {
                std::process::exit(1);
            }
        },
        Err(e) => {
            if format == OutputFormat::Json {
                let error_report = TestReport::from_error(e);
                error_report.print(OutputFormat::Json);
            } else {
                eprintln!("{} {:#}", "Error:".red().bold(), e);
            }
            std::process::exit(1);
        },
    }
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
        client_id: Some(client_id),
        client_name: None,
        dcr_enabled: false,
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

#[allow(clippy::too_many_arguments)]
async fn run_conformance_test(
    url: &str,
    strict: bool,
    domain: Option<Vec<String>>,
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

    tester.run_conformance_tests(strict, domain).await
}

/// `conformance --dual-run`: detect the eras, then run the suite once per era
/// and print the baseline-keyed comparison.
///
/// # Degradation is explicit, never silent
///
/// A dual run is only possible against a server that serves BOTH eras. Against
/// a single-era server this returns that era's ordinary `TestReport` and SAYS
/// SO on stderr, rather than failing — a v1-only server is not
/// non-conformant, it simply has not opted into `2026-07-28`.
///
/// An UNREACHABLE endpoint routes through `TestReport::from_error`, which is the
/// existing connectivity-failure path and exits non-zero. A REACHABLE endpoint
/// that speaks no era we know is a different thing — a conformance finding — and
/// gets its own Core-domain failure instead.
///
/// # The comparison is rendered only for HUMAN formats
///
/// `format` is threaded in for one reason: `handle_command_result` renders the
/// returned report to the SAME stdout this function writes the comparison to, so
/// under `--format json` a prepended text block makes the whole stream
/// unparseable. Machine formats therefore get the comparison on stderr instead;
/// nothing is dropped, and stdout stays exactly one document.
///
/// # The exit code reports the v1 suite ONLY — a KNOWN limitation
///
/// The returned report is the v1 one, so `--dual-run` exits 0 even when the v2
/// suite failed every test and the comparison reported UNEXPECTED findings. That
/// is the contract plan 117-11 chose (the flag must not change what the exit code
/// MEANS) and `the_binary_runs_in_both_modes_against_a_live_server` pins it.
///
/// The cost is real and worth stating: an opt-in flag whose findings cannot fail
/// a CI job cannot gate anything, so `--dual-run` is a reporting tool, not a gate.
/// Changing that is a deliberate contract change — it needs the pinning test
/// updated in the same commit, not a silent roll-up.
#[allow(clippy::too_many_arguments)]
async fn run_dual_conformance_test(
    url: &str,
    strict: bool,
    domain: Option<Vec<String>>,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
    format: OutputFormat,
    fail_on_era_findings: bool,
) -> Result<TestReport> {
    use conformance::{ConformanceDomain, ConformanceRunner};
    use pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28;
    use pmcp::types::ProtocolVersion;
    use tester::{EraProbeAuth, EraSupport};

    let budget = Duration::from_secs(timeout);
    let parsed_domains = domain.map(|ds| {
        ds.iter()
            .filter_map(|s| ConformanceDomain::from_str_loose(s))
            .collect::<Vec<_>>()
    });
    let runner = ConformanceRunner::new(strict, parsed_domains);

    // The dual-run path is Streamable-HTTP only: both era probes and every raw
    // wire observation are HTTP facts, so the transport is pinned rather than
    // auto-detected here.
    let build = |pin_v2: bool| -> Result<ServerTester> {
        let tester = ServerTester::new(
            url,
            budget,
            insecure,
            api_key,
            Some("http"),
            oauth_middleware.clone(),
        )?;
        Ok(if pin_v2 {
            tester.with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        } else {
            tester
        })
    };

    // The detector opens REAL connections, so it must carry the SAME
    // credentials and TLS posture the two suite runs will. Detecting with none
    // of them made `--dual-run --api-key …` (or `--insecure`, or OAuth) report
    // every authenticated endpoint as unreachable and silently degrade to a
    // single run.
    let probe_auth = EraProbeAuth {
        api_key: api_key.map(str::to_string),
        insecure,
        oauth_middleware: oauth_middleware.clone(),
    };
    let era_support = tester::detect_eras_with_auth(url, budget, &probe_auth).await;
    match era_support {
        EraSupport::Dual => {
            eprintln!(
                "{} {url} serves BOTH eras ({}); running the suite twice.",
                "note:".cyan(),
                era_support.label()
            );
            let mut v1 = build(false)?;
            let mut v2 = build(true)?;
            let report = runner.run_dual(&mut v1, &mut v2).await;
            // Best-effort, exactly as `TestReport::print` is: the CLI entry
            // point cannot act on a broken pipe at the report layer. Routed to
            // stderr for machine formats so stdout stays a single parseable
            // document (see this function's rustdoc).
            if matches!(format, OutputFormat::Pretty | OutputFormat::Verbose) {
                let _ = report.print_to_writer(&mut std::io::stdout());
            } else {
                let _ = report.print_to_writer(&mut std::io::stderr());
            }
            // The v1 report is what the caller receives, so the process exit
            // code keeps meaning "did the suite pass" and does not silently
            // change meaning when --dual-run is passed. This contract is PINNED
            // by `the_binary_runs_in_both_modes_against_a_live_server` in
            // `tests/dual_run.rs`; see this function's rustdoc for the open
            // question about whether it should hold.
            //
            // `--fail-on-era-findings` is the OPT-IN answer to that question. It
            // does not change the default contract — absent the flag this is the
            // same v1 report as before, byte for byte — but when the caller asks
            // for the comparison to be a GATE, the v2 failures and UNEXPECTED
            // differences are folded in as named Core failures so the existing
            // `handle_command_result` exit path reports them. Each carries the
            // era in its name, so a red never leaves the reader guessing which
            // suite produced it.
            let mut out = report.v1_report.clone();
            if fail_on_era_findings {
                for test in report
                    .v2_report
                    .tests
                    .iter()
                    .filter(|t| t.status == report::TestStatus::Failed)
                {
                    out.add_test(report::TestResult::failed(
                        format!("[v2 suite] {}", test.name),
                        report::TestCategory::Core,
                        test.duration,
                        test.error
                            .clone()
                            .unwrap_or_else(|| "v2 suite failure (no reason recorded)".to_string()),
                    ));
                }
                for finding in report
                    .differences
                    .iter()
                    .filter(|d| d.class == era_diff::DifferenceClass::Unexpected)
                {
                    out.add_test(report::TestResult::failed(
                        format!("[era] unexpected difference: {}", finding.observation_id),
                        report::TestCategory::Core,
                        Duration::from_secs(0),
                        finding.detail.clone(),
                    ));
                }
            }
            Ok(out)
        },
        EraSupport::V1Only => {
            eprintln!(
                "{} {url} serves only MCP 2025-11-25; --dual-run degraded to a single v1 run.",
                "note:".yellow()
            );
            Ok(runner.run(&mut build(false)?).await)
        },
        EraSupport::V2Only => {
            eprintln!(
                "{} {url} serves only MCP 2026-07-28; --dual-run degraded to a single v2 run.",
                "note:".yellow()
            );
            Ok(runner.run(&mut build(true)?).await)
        },
        // Reachable but speaking no era we know is a CONFORMANCE finding about
        // whatever answered, so it is a Core-domain failure.
        EraSupport::NoEraSpoken => Ok(tester::no_era_spoken_report(url)),
        // Unreachable is an INFRASTRUCTURE fault, so it routes through
        // `TestReport::from_error` — the existing connectivity-failure path.
        // The report carries a failure, so `handle_command_result` still exits
        // non-zero.
        EraSupport::Unreachable => Ok(tester::unreachable_report(url)),
    }
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
async fn run_apps_validation(
    url: &str,
    mode: &str,
    tool_filter: Option<String>,
    strict: bool,
    timeout: u64,
    insecure: bool,
    api_key: Option<&str>,
    transport: Option<&str>,
    oauth_middleware: Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
) -> Result<TestReport> {
    use app_validator::{AppValidationMode, AppValidator};

    let validation_mode: AppValidationMode =
        mode.parse().map_err(|e: String| anyhow::anyhow!(e))?;

    let mut tester = ServerTester::new(
        url,
        Duration::from_secs(timeout),
        insecure,
        api_key,
        transport,
        oauth_middleware,
    )?;

    // Initialize connection
    let init_report = tester.run_quick_test().await?;
    if init_report.has_failures() {
        return Ok(init_report);
    }

    // Discover tools and resources
    let tools_result = tester.list_tools().await?;
    let tools = tools_result.tools;
    let resources = match tester.list_resources().await {
        Ok(result) => result.resources,
        Err(_) => Vec::new(),
    };

    // Check if any App-capable tools exist
    let has_app_tools = tools.iter().any(AppValidator::is_app_capable);

    if !has_app_tools && tool_filter.is_none() {
        let mut report = TestReport::new();
        report.add_test(report::TestResult {
            name: "App-capable tools discovery".to_string(),
            category: report::TestCategory::Apps,
            status: report::TestStatus::Passed,
            duration: Duration::from_secs(0),
            error: None,
            details: Some(format!(
                "No App-capable tools found on server ({} tools total)",
                tools.len()
            )),
        });
        return Ok(report);
    }

    // Run validation
    let validator = AppValidator::new(validation_mode, tool_filter);
    let results = validator.validate_tools(&tools, &resources);

    let mut report = TestReport::new();
    for result in results {
        report.add_test(result);
    }

    if strict {
        report.apply_strict_mode();
    }

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
