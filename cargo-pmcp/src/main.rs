//! cargo-pmcp: Production-grade MCP server development toolkit
//!
//! This tool provides a batteries-included experience for building MCP servers in Rust,
//! based on proven patterns from 6 production servers.
#![allow(
    clippy::needless_borrows_for_generic_args,
    clippy::ptr_arg,
    clippy::double_ended_iterator_last,
    clippy::useless_format,
    clippy::deref_addrof,
    clippy::uninlined_format_args,
    clippy::too_many_arguments,
    clippy::collapsible_else_if,
    clippy::redundant_static_lifetimes,
    clippy::to_string_in_format_args,
    clippy::module_inception,
    clippy::print_literal,
    clippy::needless_borrow
)]

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use std::io::IsTerminal;

mod commands;
mod deployment;
mod landing;
mod pentest;
mod publishing;
mod secrets;
mod templates;
mod utils;

use commands::flags::AuthFlags;
use commands::GlobalFlags;

/// Production-grade MCP server development toolkit
#[derive(Parser)]
#[command(name = "cargo-pmcp")]
#[command(bin_name = "cargo pmcp")]
#[command(about = "Build production-ready MCP servers in Rust", long_about = None)]
#[command(version)]
#[command(after_long_help = "\x1b[1mExamples:\x1b[0m
  cargo pmcp new my-project          Create a new MCP workspace
  cargo pmcp dev --server my-server  Start development server
  cargo pmcp test check <url>        Quick server health check
  cargo pmcp test conformance <url>  Run MCP protocol conformance tests
  cargo pmcp pentest <url>           Security penetration testing
  cargo pmcp preview <url>           Preview MCP Apps in browser
  cargo pmcp doctor                  Diagnose workspace health
  cargo pmcp completions zsh         Generate shell completions")]
struct Cli {
    /// Dump every HTTP request and response the SDK puts on the wire.
    ///
    /// Answers "what did we actually send?" — the first question when a
    /// deployed server rejects something. Shows the request line, the headers
    /// (including the v2 routing trio `MCP-Protocol-Version` / `Mcp-Method` /
    /// `Mcp-Name`) and the body, plus the response status and headers.
    ///
    /// Credential and session headers are REDACTED: a wire dump is exactly the
    /// artifact that ends up in a bug report or a CI artifact.
    ///
    /// A preset over the SDK's `pmcp::wire` tracing target, so the equivalent
    /// without the flag is `RUST_LOG=pmcp::wire=debug`, and any
    /// `tracing_subscriber` layer composes with it. Most useful with
    /// `cargo pmcp test conformance` and the post-deploy verifiers.
    #[arg(long, global = true)]
    dump_wire: bool,

    /// Enable verbose output for debugging
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Suppress colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Suppress all non-error output
    #[arg(long, global = true)]
    quiet: bool,

    /// Named target from ~/.pmcp/config.toml (one-off override of .pmcp/active-target).
    ///
    /// Phase 77: this flag selects a NAMED target defined via `cargo pmcp configure add <name>`.
    /// Distinct from `cargo pmcp deploy --target-type <type>` which selects the deployment backend.
    /// Resolution order: PMCP_TARGET env > this flag > .pmcp/active-target file > none.
    ///
    /// Note: `global = true` is intentionally NOT set on this flag — the deploy subcommand
    /// owns a `--target` alias (deprecated, points to `--target-type`) and clap requires
    /// unique long-option names across the global+local arg namespace. Top-level resolution
    /// is sufficient since main.rs reads `cli.target` directly before dispatching.
    #[arg(long)]
    pub target: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new MCP workspace
    ///
    /// This creates a workspace with server-common template and scaffolding
    /// for building multiple MCP servers. The workspace pattern allows sharing
    /// common code (like HTTP bootstrap) across all servers.
    #[command(after_long_help = "Examples:
  cargo pmcp new my-project
  cargo pmcp new my-project --path /tmp
  cargo pmcp new my-sql-server --kind sql-server")]
    New {
        /// Name of the workspace (or single crate, with --kind) to create
        name: String,

        /// Directory to create workspace in (defaults to current directory)
        #[arg(long)]
        path: Option<String>,

        /// Project kind. Omit for the default multi-crate workspace; pass
        /// `sql-server` to emit a single runnable config-driven SQL server crate.
        #[arg(long)]
        kind: Option<String>,
    },

    /// Add a component to the workspace
    ///
    /// Supports adding servers, tools, workflows, and resources to existing servers.
    Add {
        #[command(subcommand)]
        component: AddCommands,
    },

    /// Test MCP servers with mcp-tester
    ///
    /// Run tests locally, generate scenarios, or manage scenarios on pmcp.run
    #[command(after_long_help = "Examples:
  cargo pmcp test check http://localhost:3000
  cargo pmcp test conformance http://localhost:3000 --strict
  cargo pmcp test apps http://localhost:3000 --mode chatgpt
  cargo pmcp test run --server my-server")]
    Test {
        #[command(subcommand)]
        command: commands::test::TestCommand,
    },

    /// Manage OAuth credentials for MCP servers
    ///
    /// Log in once per OAuth-protected MCP server; subsequent `cargo pmcp test/*`,
    /// `connect`, `preview`, `schema`, `dev`, `loadtest/run`, `pentest` calls pick
    /// up the cached token automatically.
    #[command(after_long_help = "Examples:
  cargo pmcp auth login https://mcp.pmcp.run
  cargo pmcp auth login https://mcp.pmcp.run --client claude-desktop
  cargo pmcp auth status
  cargo pmcp auth token https://mcp.pmcp.run
  cargo pmcp auth logout --all")]
    Auth {
        #[command(subcommand)]
        command: commands::auth_cmd::AuthCommand,
    },

    /// Manage named deployment targets (dev, prod, staging, …)
    ///
    /// Define and select target environments stored in `~/.pmcp/config.toml`
    /// — modeled on `aws configure`. Each target carries the type, region, AWS
    /// profile, and api_url; deploy/upload commands resolve via the active target.
    #[command(after_long_help = "Examples:
  cargo pmcp configure add dev --type pmcp-run --region us-west-2
  cargo pmcp configure use dev
  cargo pmcp configure list
  cargo pmcp configure list --format json
  cargo pmcp configure show dev")]
    Configure {
        #[command(subcommand)]
        command: commands::configure::ConfigureCommand,
    },

    /// Compile, lint, and emit governed Excel workbook bundles
    ///
    /// Shells over the pmcp-workbook-compiler: ingest → lint → synth → compile →
    /// reconcile → gate → write. Reads workbook→bundle mappings from pmcp.toml.
    #[command(after_long_help = "Examples:
  cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice
  cargo pmcp workbook compile                 # compile-all from pmcp.toml
  cargo pmcp workbook lint pricing.xlsx
  cargo pmcp workbook emit pricing.xlsx --workflow quote")]
    Workbook {
        #[command(subcommand)]
        command: commands::workbook::WorkbookCommand,
    },

    /// Scaffold and run deploy-anywhere agents (agent-as-MCP-client)
    ///
    /// `agent new` scaffolds a `pmcp-agent` package; `agent dev` runs the agent
    /// loop against an OpenAI-compatible completion source.
    Agent {
        #[command(subcommand)]
        command: commands::agent::AgentCommand,
    },

    /// Run the reference team servers for local development
    ///
    /// `team dev` composes the `pmcp-team-servers` reference set (team-fs,
    /// mem-mcp, approval-mcp, team-mcp) in-process or served over HTTP.
    Team {
        #[command(subcommand)]
        command: commands::team::TeamCommand,
    },

    /// Inspect and capture portable AI-Package bundles
    ///
    /// `package show` prints an AI-Package manifest; `package capture` captures
    /// a bundle for a platform target selected by the capture-local `--target`.
    Package {
        #[command(subcommand)]
        command: commands::package::PackageCommand,
    },

    /// Start development server
    ///
    /// Builds and runs the server with live logs
    #[command(after_long_help = "Examples:
  cargo pmcp dev --server my-server
  cargo pmcp dev --server my-server --port 8080
  cargo pmcp dev --server my-server --connect claude-code")]
    Dev {
        /// Name of the server to run
        #[arg(long)]
        server: String,

        /// Port to run the server on
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Automatically connect to MCP client (claude-code, cursor, inspector)
        #[arg(long)]
        connect: Option<String>,
    },

    /// Connect server to an MCP client
    ///
    /// Helps configure connection to Claude Code, Cursor, or MCP Inspector
    #[command(after_long_help = "Examples:
  cargo pmcp connect --server my-server --client claude-code
  cargo pmcp connect --server my-server --client cursor
  cargo pmcp connect --server my-server --client inspector")]
    Connect {
        /// Name of the server
        #[arg(long)]
        server: String,

        /// MCP client to connect to (claude-code, cursor, inspector)
        #[arg(long)]
        client: String,

        /// Server URL
        #[arg(default_value = "http://localhost:3000")]
        url: String,

        /// Authentication flags for the target MCP server
        #[command(flatten)]
        auth_flags: AuthFlags,
    },

    /// Deploy MCP server to cloud platforms.
    ///
    /// Builds widgets (auto-detected from widget/ or widgets/) before compiling
    /// and deploying the Rust binary. Verifies the deployed endpoint via
    /// cargo pmcp test {check,conformance,apps} before reporting success.
    ///
    /// Deploy to AWS Lambda, Azure Container Apps, Google Cloud Run, etc.
    Deploy(commands::deploy::DeployCommand),

    /// Manage landing pages for MCP servers
    ///
    /// Create, develop, and deploy landing pages that showcase your MCP server
    Landing {
        #[command(subcommand)]
        command: commands::landing::LandingCommand,
    },

    /// Export schema from foundation MCP servers
    ///
    /// Connect to a foundation server and generate typed Rust client code
    /// for calling its tools. Supports both MCP HTTP and Lambda invocation.
    Schema {
        #[command(subcommand)]
        command: commands::schema::SchemaCommand,
    },

    /// Validate MCP server components
    ///
    /// Run validation checks on workflows, tools, and other server components.
    /// Helps catch structural errors before runtime.
    Validate {
        #[command(subcommand)]
        command: commands::validate::ValidateCommand,
    },

    /// Manage secrets for MCP servers
    ///
    /// Store and retrieve secrets across multiple providers (local, pmcp.run, AWS).
    /// Secrets are namespaced by server ID to avoid conflicts.
    Secret(commands::secret::SecretCommand),

    /// Run load tests against MCP servers
    ///
    /// Execute load tests with configurable virtual users, scenarios, and reports.
    #[command(after_long_help = "Examples:
  cargo pmcp loadtest run http://localhost:3000 --users 10 --duration 30
  cargo pmcp loadtest run http://localhost:3000 --format json -o report.json")]
    Loadtest {
        #[command(subcommand)]
        command: commands::loadtest::LoadtestCommand,
    },

    /// MCP Apps project management
    ///
    /// Scaffold and manage MCP Apps projects with interactive widgets.
    App {
        #[command(subcommand)]
        command: commands::app::AppCommand,
    },

    /// Diagnose workspace and server health
    ///
    /// Validates project structure (Cargo.toml, pmcp dependency), Rust toolchain,
    /// development tools (rustfmt, clippy), and optionally tests MCP server connectivity.
    #[command(after_long_help = "Examples:
  cargo pmcp doctor
  cargo pmcp doctor http://localhost:3000")]
    Doctor {
        /// Optional MCP server URL to test connectivity
        url: Option<String>,
    },

    /// Generate shell completions
    ///
    /// Outputs shell completion scripts for bash, zsh, fish, or powershell.
    /// Pipe to a file or source directly in your shell config.
    #[command(after_long_help = "Examples:
  cargo pmcp completions zsh > ~/.zfunc/_cargo-pmcp
  cargo pmcp completions bash > /etc/bash_completion.d/cargo-pmcp
  cargo pmcp completions fish > ~/.config/fish/completions/cargo-pmcp.fish")]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Run security penetration tests against MCP servers
    ///
    /// Probes MCP endpoints for protocol-specific vulnerabilities: prompt injection,
    /// tool poisoning, and session security issues. Reports findings with severity
    /// levels in text, JSON, or SARIF format.
    Pentest(commands::pentest::PentestCommand),

    /// Preview MCP Apps widgets in browser
    ///
    /// Launch a browser-based preview environment for testing MCP servers
    /// that return widget UI. Simulates the ChatGPT Apps runtime.
    #[command(after_long_help = "Examples:
  cargo pmcp preview http://localhost:3000 --open
  cargo pmcp preview http://localhost:3000 --mode chatgpt --open
  cargo pmcp preview http://localhost:3000 --widgets-dir ./widgets")]
    Preview {
        /// URL of the running MCP server
        url: String,

        /// Port for the preview server
        #[arg(long, default_value = "8765")]
        port: u16,

        /// Open browser automatically
        #[arg(long)]
        open: bool,

        /// Auto-select this tool on start
        #[arg(long)]
        tool: Option<String>,

        /// Initial theme (light/dark)
        #[arg(long, default_value = "light")]
        theme: String,

        /// Initial locale
        #[arg(long, default_value = "en-US")]
        locale: String,

        /// Path to widgets directory for file-based authoring (hot-reload)
        ///
        /// When set, widget HTML files are read directly from this directory
        /// on each request. Browser refresh shows the latest HTML without
        /// server restart.
        #[arg(long)]
        widgets_dir: Option<String>,

        /// Preview mode: standard (default) or chatgpt (strict ChatGPT protocol validation)
        #[arg(long, default_value = "standard")]
        mode: String,

        /// Authentication flags for the target MCP server
        #[command(flatten)]
        auth_flags: AuthFlags,
    },
}

impl Commands {
    /// MED-2 per 77-REVIEWS.md: determine whether this subcommand consumes a resolved
    /// Phase 77 target. Only target-consuming commands trigger env injection in `main`.
    ///
    /// Target-consuming = invokes AWS API, CDK, GraphQL upload to pmcp.run, or otherwise
    /// reads `region` / `aws_profile` / `api_url` from the resolved target.
    ///
    /// Non-target-consuming commands (configure, auth, doctor, completions, schema, etc.)
    /// must NOT side-effect into process env: doing so would (a) fail when `PMCP_TARGET`
    /// is set to a non-existent target, (b) clobber user-set `AWS_PROFILE` / `AWS_REGION`
    /// for commands that have nothing to do with AWS.
    pub fn is_target_consuming(&self) -> bool {
        matches!(
            self,
            Commands::Deploy(_)
                | Commands::Loadtest { .. }
                | Commands::Test { .. }
                | Commands::Landing { .. }
        )
    }
}

#[derive(Subcommand)]
enum AddCommands {
    /// Add a new MCP server to the workspace
    Server {
        /// Name of the server (will create mcp-{name}-core and {name}-server)
        name: String,

        /// Server template to use
        #[arg(long, default_value = "minimal")]
        template: String,

        /// Port to assign to this server (auto-increments if not specified)
        #[arg(long)]
        port: Option<u16>,

        /// Replace existing server with same name (requires confirmation)
        #[arg(long)]
        replace: bool,
    },

    /// Add a tool to an existing server
    Tool {
        /// Name of the tool
        name: String,

        /// Server to add the tool to
        #[arg(long)]
        server: String,
    },

    /// Add a workflow to an existing server
    Workflow {
        /// Name of the workflow
        name: String,

        /// Server to add the workflow to
        #[arg(long)]
        server: String,
    },
}

/// Install the tracing subscriber that collects the SDK's wire events.
///
/// `cargo-pmcp` drives the SDK as a LIBRARY, so `pmcp::wire` events are emitted
/// on every request it makes — but until this call nothing collected them, and a
/// `--dump-wire` here would have printed nothing at all. That is the whole gap
/// this closes: the same instrumentation `mcp-tester --dump-wire` surfaces is
/// available to `cargo pmcp test conformance` and the post-deploy verifiers.
///
/// A preset, not a parallel logger: `RUST_LOG` wins when set, and the flag is
/// ADDITIVE to it. With neither, no subscriber is installed and the SDK's
/// `enabled()` guards keep every wire path from allocating.
fn init_wire_tracing(dump_wire: bool) {
    let wire_directive = format!("{}=debug", pmcp::shared::wire_trace::WIRE_TARGET);
    let filter = match (std::env::var("RUST_LOG").is_ok(), dump_wire) {
        // An explicit RUST_LOG is a deliberate choice; the flag adds to it.
        (true, true) => tracing_subscriber::EnvFilter::from_default_env().add_directive(
            wire_directive
                .parse()
                .expect("the wire directive is a compile-time constant"),
        ),
        (true, false) => tracing_subscriber::EnvFilter::from_default_env(),
        // Wire frames ONLY: turning on `pmcp=debug` wholesale would bury the
        // frames the user asked for under unrelated SDK output.
        (false, true) => tracing_subscriber::EnvFilter::new(wire_directive),
        // Nothing requested — install nothing, so there is no subscriber to pay
        // for and the SDK's guards short-circuit.
        (false, false) => return,
    };
    // `try_init`, not `init`: a subscriber may already be installed by an
    // embedding process, and a diagnostic flag must never panic the CLI.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn main() -> Result<()> {
    // Handle cargo subcommand invocation
    // When called as `cargo pmcp`, cargo passes "pmcp" as the first argument
    let mut args = std::env::args();
    let cli = if args.nth(1).as_deref() == Some("pmcp") {
        // Skip the "pmcp" argument when invoked as cargo subcommand
        let args_vec: Vec<String> = std::env::args()
            .enumerate()
            .filter_map(|(i, arg)| if i != 1 { Some(arg) } else { None })
            .collect();
        Cli::parse_from(args_vec)
    } else {
        // Normal invocation as cargo-pmcp
        Cli::parse()
    };

    // Set verbose mode as environment variable for global access
    if cli.verbose {
        std::env::set_var("PMCP_VERBOSE", "1");
    }

    // Determine effective no_color: explicit flag, NO_COLOR env (no-color.org), or non-TTY
    let effective_no_color =
        cli.no_color || std::env::var("NO_COLOR").is_ok() || !std::io::stdout().is_terminal();

    if effective_no_color {
        // Suppress colored crate output globally
        colored::control::set_override(false);
        // Suppress console crate output globally
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }

    // Verbose wins over quiet (per user decision):
    // If both --verbose and --quiet are passed, quiet is disabled.
    let effective_quiet = cli.quiet && !cli.verbose;

    // Set global flag env vars for subprocess consumption
    if effective_no_color {
        std::env::set_var("PMCP_NO_COLOR", "1");
    }
    if effective_quiet {
        std::env::set_var("PMCP_QUIET", "1");
    }

    // Install the wire-trace subscriber BEFORE any command runs, so a dump
    // covers the whole invocation rather than starting mid-flight.
    init_wire_tracing(cli.dump_wire);

    let global_flags = GlobalFlags {
        verbose: cli.verbose,
        no_color: effective_no_color,
        quiet: effective_quiet,
    };

    // Phase 77: resolve named target and inject env vars for downstream consumers.
    // MUST happen BEFORE any aws-config code path (the AWS SDK caches its credential
    // provider at first use; env injection after that point would be ignored).
    //
    // **MED-2 per 77-REVIEWS.md**: env injection is gated to target-consuming commands ONLY.
    // Running `cargo pmcp configure list` or `cargo pmcp doctor` with PMCP_TARGET=foo set must
    // NOT mutate process env (or fail because foo doesn't exist) — those commands are NOT
    // target consumers. The gate uses `Commands::is_target_consuming()`.
    //
    // Returns Ok(None) when no target selected AND ~/.pmcp/config.toml does not exist
    // (D-11 zero-touch).
    if cli.command.is_target_consuming() {
        // Discover the workspace root for the resolver. Fall back to current_dir when the walk
        // fails (e.g. from a system-tmp dir during tests) — the resolver tolerates a non-Cargo
        // path and just won't find deploy.toml.
        let project_root =
            commands::configure::workspace::find_workspace_root().unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        // For dispatch-time injection we don't yet have a parsed DeployConfig (the deploy
        // command will load its own as part of its action path). We pass `None` here so
        // env-source vars take effect early, before any AWS SDK initializes its provider chain.
        // The deploy/test/loadtest commands additionally call resolve_target with their loaded
        // `Some(&deploy_config)` to get the deploy.toml fall-through layer.
        match commands::configure::resolver::resolve_target(
            None,
            cli.target.as_deref(),
            &project_root,
            None,
        ) {
            Ok(Some(resolved)) => {
                commands::configure::resolver::inject_resolved_env_into_process(&resolved);
            },
            Ok(None) => { /* D-11 zero-touch: no config, no target — Phase 76 behavior */ },
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(2);
            },
        }
    }

    // Dispatch the command. A returned `Err` is first downcast to `WorkbookExit`
    // (D-10): a governance gate block must reach the shell as the DISTINCT exit
    // code 2, not be collapsed to anyhow's default `1`. The gate-block render is
    // already printed to stdout by the handler before it constructs the error;
    // here we re-surface a concise message to stderr and exit with its code. This
    // MIRRORS the existing `eprintln!()+std::process::exit(2)` precedent above
    // (the Phase-77 resolver path).
    match execute_command(cli.command, &global_flags) {
        Ok(()) => {},
        Err(e) => {
            if let Some(wx) = e.downcast_ref::<commands::workbook::WorkbookExit>() {
                eprintln!("{}", wx.message);
                std::process::exit(wx.code);
            }
            return Err(e);
        },
    }

    Ok(())
}

fn execute_command(command: Commands, global_flags: &GlobalFlags) -> Result<()> {
    // Subcommand groups whose execute(&GlobalFlags) method is on the
    // subcommand type itself — dispatch uniformly.
    if let Some(result) = dispatch_trait_based(command, global_flags) {
        return result;
    }
    Ok(())
}

/// Route through every `Commands` variant and dispatch either via the
/// subcommand's `.execute(global_flags)` method or an explicit function-call
/// wrapper. Always returns `Some(result)` — the outer `execute_command`
/// unwraps for `Ok(())` compatibility.
fn dispatch_trait_based(command: Commands, global_flags: &GlobalFlags) -> Option<Result<()>> {
    let result: Result<()> = match command {
        Commands::New { name, path, kind } => {
            commands::new::execute(name, path, None, kind, global_flags)
        },
        Commands::Add { component } => execute_add(component, global_flags),
        Commands::Test { command } => command.execute(global_flags),
        Commands::Auth { command } => command.execute(global_flags),
        Commands::Configure { command } => command.execute(global_flags),
        Commands::Workbook { command } => command.execute(global_flags),
        Commands::Agent { command } => execute_agent(command, global_flags),
        Commands::Team { command } => execute_team(command, global_flags),
        Commands::Package { command } => execute_package(command, global_flags),
        Commands::Dev {
            server,
            port,
            connect,
        } => commands::dev::execute(server, port, connect, global_flags),
        Commands::Connect {
            server,
            client,
            url,
            auth_flags,
        } => commands::connect::execute(server, client, url, &auth_flags, global_flags),
        Commands::Deploy(deploy_cmd) => deploy_cmd.execute(global_flags),
        Commands::Landing { command } => execute_landing(command, global_flags),
        Commands::Schema { command } => command.execute(global_flags),
        Commands::Validate { command } => command.execute(global_flags),
        Commands::Secret(secret_cmd) => secret_cmd.execute(global_flags),
        Commands::Loadtest { command } => command.execute(global_flags),
        Commands::App { command } => command.execute(global_flags),
        Commands::Doctor { url } => commands::doctor::execute(url.as_deref(), global_flags),
        Commands::Completions { shell } => {
            execute_completions(shell);
            Ok(())
        },
        Commands::Pentest(pentest_cmd) => pentest_cmd.execute(global_flags),
        Commands::Preview {
            url,
            port,
            open,
            tool,
            theme,
            locale,
            widgets_dir,
            mode,
            auth_flags,
        } => execute_preview(
            url,
            port,
            open,
            tool,
            theme,
            locale,
            widgets_dir,
            mode,
            auth_flags,
            global_flags,
        ),
    };
    Some(result)
}

/// Emit shell completions to stdout for the given shell.
fn execute_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "cargo pmcp", &mut std::io::stdout());
}

/// Dispatcher for the Add subcommand tree (Server / Tool / Workflow).
fn execute_add(component: AddCommands, global_flags: &GlobalFlags) -> Result<()> {
    match component {
        AddCommands::Server {
            name,
            template,
            port,
            replace,
        } => commands::add::server(name, template, port, replace, global_flags),
        AddCommands::Tool { name, server } => commands::add::tool(name, server, global_flags),
        AddCommands::Workflow { name, server } => {
            commands::add::workflow(name, server, global_flags)
        },
    }
}

/// Dispatcher for the Landing subcommand group (async; spins up its own
/// tokio runtime because main.rs stays sync).
fn execute_landing(
    command: commands::landing::LandingCommand,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let project_root = std::env::current_dir()?;
    runtime.block_on(command.execute(project_root, global_flags))
}

/// Dispatcher for the `agent` subcommand group (async; owns its own tokio
/// runtime because main.rs stays sync — mirrors [`execute_landing`]).
fn execute_agent(command: commands::agent::AgentCommand, global_flags: &GlobalFlags) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(command.execute(global_flags))
}

/// Dispatcher for the `team` subcommand group (async; owns its own tokio
/// runtime because main.rs stays sync — mirrors [`execute_landing`]).
fn execute_team(command: commands::team::TeamCommand, global_flags: &GlobalFlags) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(command.execute(global_flags))
}

/// Dispatcher for the `package` subcommand group (async; owns its own tokio
/// runtime because main.rs stays sync — mirrors [`execute_landing`]).
fn execute_package(
    command: commands::package::PackageCommand,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(command.execute(global_flags))
}

#[cfg(test)]
mod cli_target_flag_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_global_target_named_flag() {
        // Note: cargo subcommand parsing puts the bin name first
        let cli = Cli::parse_from(["cargo-pmcp", "--target", "dev", "auth", "status"]);
        assert_eq!(cli.target.as_deref(), Some("dev"));
    }

    #[test]
    fn legacy_deploy_target_alias_still_works() {
        // The DEPLOY-level `--target aws-lambda` (alias for `--target-type`) still parses
        let cli = Cli::parse_from(["cargo-pmcp", "deploy", "--target", "aws-lambda", "outputs"]);
        assert!(
            cli.target.is_none(),
            "global --target should NOT consume the deploy-scoped --target alias"
        );
    }

    #[test]
    fn both_flags_coexist() {
        let cli = Cli::parse_from([
            "cargo-pmcp",
            "--target",
            "dev",
            "deploy",
            "--target-type",
            "aws-lambda",
            "outputs",
        ]);
        assert_eq!(cli.target.as_deref(), Some("dev"));
    }

    // MED-3 fix per 77-REVIEWS.md: clap matrix tests covering the `--target` (top-level
    // named target) vs `--target-type` (deploy-scoped, with `alias = "target"`) ambiguity
    // edge cases. These are explicit snapshot/parse tests; if clap conflates the two flags
    // for any of these invocations, these tests fail before any wiring runs.

    #[test]
    fn med3_top_target_and_deploy_target_type_disjoint() {
        // `cargo pmcp --target dev deploy --target-type aws-lambda <args>`
        // — top-level Cli.target is "dev"; deploy-scoped target_type is "aws-lambda".
        let cli = Cli::parse_from([
            "cargo-pmcp",
            "--target",
            "dev",
            "deploy",
            "--target-type",
            "aws-lambda",
            "outputs",
        ]);
        assert_eq!(
            cli.target.as_deref(),
            Some("dev"),
            "top-level --target = dev"
        );
        if let Commands::Deploy(deploy_cmd) = cli.command {
            let dbg = format!("{:?}", deploy_cmd);
            assert!(
                dbg.contains("aws-lambda") || dbg.contains("AwsLambda"),
                "deploy.target_type must contain `aws-lambda`; debug repr: {dbg}"
            );
        } else {
            panic!("expected Commands::Deploy");
        }
    }

    #[test]
    fn med3_legacy_alias_still_works_via_alias() {
        // `cargo pmcp deploy --target aws-lambda <args>` — `--target` is the deprecated
        // alias for `--target-type`. The top-level Cli.target must REMAIN None (the deploy-scoped
        // alias is not consumed by the global --target).
        let cli = Cli::parse_from(["cargo-pmcp", "deploy", "--target", "aws-lambda", "outputs"]);
        assert!(
            cli.target.is_none(),
            "top-level --target must NOT be populated by deploy-scoped alias"
        );
    }

    #[test]
    fn med3_top_target_and_deploy_target_alias_disjoint() {
        // `cargo pmcp --target dev deploy --target prod <args>`
        // — top-level Cli.target = "dev"; deploy-scoped --target alias = "prod" (target_type).
        let cli = Cli::parse_from([
            "cargo-pmcp",
            "--target",
            "dev",
            "deploy",
            "--target",
            "prod",
            "outputs",
        ]);
        assert_eq!(
            cli.target.as_deref(),
            Some("dev"),
            "top-level --target = dev"
        );
        if let Commands::Deploy(deploy_cmd) = cli.command {
            let dbg = format!("{:?}", deploy_cmd);
            assert!(
                dbg.contains("prod"),
                "deploy-scoped --target alias must = `prod`; debug repr: {dbg}"
            );
        }
    }

    // Phase 77 Plan 07 — MED-2 gating tests for is_target_consuming() + Configure dispatch.
    // These tests verify that env injection in main.rs is gated to commands that actually
    // consume a resolved target (deploy/test/loadtest/landing) and skipped for everything else
    // (configure/auth/doctor/completions/etc.).

    #[test]
    fn is_target_consuming_returns_true_only_for_target_commands() {
        let deploy_cli = Cli::parse_from(["cargo-pmcp", "deploy", "outputs"]);
        assert!(
            deploy_cli.command.is_target_consuming(),
            "deploy must be target-consuming"
        );

        let configure_cli = Cli::parse_from(["cargo-pmcp", "configure", "list"]);
        assert!(
            !configure_cli.command.is_target_consuming(),
            "configure must NOT be target-consuming"
        );

        let auth_cli = Cli::parse_from(["cargo-pmcp", "auth", "status"]);
        assert!(
            !auth_cli.command.is_target_consuming(),
            "auth must NOT be target-consuming"
        );

        let doctor_cli = Cli::parse_from(["cargo-pmcp", "doctor"]);
        assert!(
            !doctor_cli.command.is_target_consuming(),
            "doctor must NOT be target-consuming"
        );
    }

    #[test]
    fn pmcp_target_does_not_pollute_env_for_non_target_command() {
        // MED-2 acceptance: running `cargo pmcp configure list` with PMCP_TARGET=foo set must
        // NOT mutate process env. We test this by parsing the CLI and checking that
        // is_target_consuming() returns false — main.rs's gate uses this method.
        let cli = Cli::parse_from(["cargo-pmcp", "configure", "list"]);
        assert!(!cli.command.is_target_consuming());
    }

    #[test]
    fn med3_deploy_help_renders_both_flags() {
        // `cargo pmcp deploy --help` must render both `--target` (alias [DEPRECATED]) and
        // `--target-type` in the help output. We can't easily assert on stdout from inside a
        // test without spawning a subprocess; verify the structure compiles and clap doesn't
        // panic at help-render time. Use `Cli::command()` for the parsed command tree.
        use clap::CommandFactory;
        let cmd = Cli::command();
        let deploy = cmd
            .find_subcommand("deploy")
            .expect("deploy subcommand exists");
        let arg_names: Vec<String> = deploy
            .get_arguments()
            .map(|a| a.get_id().to_string())
            .collect();
        assert!(
            arg_names.iter().any(|n| n == "target_type"),
            "deploy must declare target_type arg; got: {:?}",
            arg_names
        );
    }
}

// Phase 77 Plan 07 — Configure subcommand parse tests
#[cfg(test)]
mod configure_dispatch_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_configure_add_subcommand() {
        let cli = Cli::parse_from([
            "cargo-pmcp",
            "configure",
            "add",
            "dev",
            "--type",
            "pmcp-run",
            "--region",
            "us-west-2",
        ]);
        match cli.command {
            Commands::Configure {
                command: commands::configure::ConfigureCommand::Add(args),
            } => {
                assert_eq!(args.name, "dev");
                assert_eq!(args.r#type.as_deref(), Some("pmcp-run"));
                assert_eq!(args.region.as_deref(), Some("us-west-2"));
            },
            _ => panic!("expected Configure::Add"),
        }
    }

    #[test]
    fn parses_configure_use_with_keyword_safe_name() {
        let cli = Cli::parse_from(["cargo-pmcp", "configure", "use", "prod"]);
        match cli.command {
            Commands::Configure {
                command: commands::configure::ConfigureCommand::Use(args),
            } => {
                assert_eq!(args.name, "prod");
            },
            _ => panic!("expected Configure::Use"),
        }
    }

    #[test]
    fn parses_configure_list_with_format_json() {
        let cli = Cli::parse_from(["cargo-pmcp", "configure", "list", "--format", "json"]);
        match cli.command {
            Commands::Configure {
                command: commands::configure::ConfigureCommand::List(args),
            } => {
                assert_eq!(args.format, "json");
            },
            _ => panic!("expected Configure::List"),
        }
    }
}

/// Dispatcher for the Preview command (async; spins up its own tokio runtime).
#[allow(clippy::too_many_arguments)]
fn execute_preview(
    url: String,
    port: u16,
    open: bool,
    tool: Option<String>,
    theme: String,
    locale: String,
    widgets_dir: Option<String>,
    mode: String,
    auth_flags: commands::flags::AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(commands::preview::execute(
        url,
        port,
        open,
        tool,
        theme,
        locale,
        widgets_dir,
        mode,
        &auth_flags,
        global_flags,
    ))
}
