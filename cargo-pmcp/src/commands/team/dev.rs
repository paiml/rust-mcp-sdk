//! `cargo pmcp team dev` (CLI-03) — run the reference team servers locally.
//!
//! Three behaviors, one thin CLI over the Phase 108/109 primitives (D-02 — the
//! DEFAULT flow does NOT re-implement composition):
//!
//! - **default** — compose the two-member doc-review team in ONE process via
//!   [`TeamRuntimeBuilder`] over in-memory transports on an offline
//!   `FixedSource`, and print a labeled transcript of the 7-step doc-review flow.
//!   The [`TeamPackage`] is loaded from `--package` (+ `--data-dir`) or the
//!   built-in doc-review fixture (D-02 locked default).
//! - **`--serve`** — expose team-mcp over HTTP by reusing the shipped `team-mcp`
//!   binary's PUBLIC serve recipe ([`build_team_mcp_server`] over the
//!   member-wiring loop, then [`serve_streamable_http`]) on `127.0.0.1:<--port>`.
//!   NOT through [`TeamRuntime`](pmcp_team_servers::compose::wiring), which only
//!   exposes in-memory clients — and with no upstream API change.
//! - **`--llm <endpoint>`** — swap the offline `FixedSource` for an
//!   [`OpenAiCompatSource`] wrapped in the ALREADY-EXPORTED [`FixedSourceFactory`]
//!   (the `CompletionSourceFactory` trait is sync + infallible, so the source is
//!   constructed + validated ONCE in the CLI, then wrapped).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Args;
use colored::Colorize;
use serde_json::{json, Value};
use tempfile::TempDir;

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::tasks::RELATED_TASK_META_KEY;
use pmcp::types::{CallToolResult, Content, Role};
use pmcp::Client;

use pmcp_agent::{
    resolve_agent, CompletionError, CompletionSource, CompletionSourceFactory, EnvVarResolver,
    FixedSourceFactory,
};

use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::reference::ComponentType;
use pmcp_package::slot::SlotType;
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};

use pmcp_team_servers::compose::resolver::{LocalDirPackageResolver, PackageResolver};
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;
use pmcp_team_servers::dev_bin::serve_streamable_http;
use pmcp_team_servers::team::identity::{MemberId, MemberTaskForwarding};
use pmcp_team_servers::team::member::{resolve_member_factory, MemberHandle};
use pmcp_team_servers::team::server::build_team_mcp_server;

use pmcp_team_servers::transport::DuplexTransport;

use crate::commands::agent::sources::{build_openai_compat_source, resolve_api_key};
use crate::commands::GlobalFlags;

/// The default member-resolver root when a `--package` is supplied without a
/// `--data-dir` (matches the shipped `team-mcp` binary default).
const DEFAULT_DATA_DIR: &str = "./team-mcp-data";

/// Arguments for `cargo pmcp team dev`.
#[derive(Debug, Args)]
pub struct DevArgs {
    /// Serve the team over HTTP instead of running the in-process transcript.
    #[arg(long)]
    pub serve: bool,
    /// Port for the HTTP serve path.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// LLM endpoint for team members backed by a real model (swaps the offline
    /// FixedSource for an OpenAI-compatible source).
    #[arg(long)]
    pub llm: Option<String>,
    /// Environment variable holding the LLM API key (`--llm` only).
    #[arg(long)]
    pub llm_api_key_env: Option<String>,
    /// Model id passed to the LLM source (`--llm` only).
    #[arg(long, default_value = "llama3.2")]
    pub model: String,
    /// Path to the team package to run.
    #[arg(long)]
    pub package: Option<PathBuf>,
    /// Directory for team-fs / mem-mcp state and member `AgentPackage`s.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Allow a plain-HTTP (non-TLS) LLM endpoint (`--llm` only).
    #[arg(long)]
    pub allow_insecure_http: bool,
}

/// Run the reference team servers, dispatching to the serve or transcript path.
pub async fn execute(args: DevArgs, global_flags: &GlobalFlags) -> Result<()> {
    let team = LoadedTeam::load(args.package.as_deref(), args.data_dir.as_deref())?;

    if args.serve {
        return serve_team_mcp(&team, args.port, global_flags).await;
    }
    let factory = completion_factory(&args)?;
    run_transcript(&team, factory, global_flags).await
}

// ---------------------------------------------------------------------------
// Package loading: explicit --package (+ --data-dir) or the built-in fixture.
// ---------------------------------------------------------------------------

/// A loaded team: its [`TeamPackage`], the member-resolver root, the team-fs /
/// mem-mcp data root, and any tempdir guards keeping the built-in fixture alive.
struct LoadedTeam {
    pkg: TeamPackage,
    member_root: PathBuf,
    data_root: PathBuf,
    // Held only to keep the built-in fixture's tempdirs alive for the run.
    _guards: Vec<TempDir>,
}

impl LoadedTeam {
    /// Load an explicit `--package` (members resolved from `--data-dir`, default
    /// `./team-mcp-data`), or synthesize the built-in doc-review fixture into
    /// tempdirs (the D-02 locked default).
    fn load(package: Option<&Path>, data_dir: Option<&Path>) -> Result<Self> {
        match package {
            Some(path) => Self::from_file(path, data_dir),
            None => Self::builtin_fixture(),
        }
    }

    /// Load a developer-supplied `TeamPackage` JSON; members resolve from the
    /// supplied (or default) data dir.
    fn from_file(path: &Path, data_dir: Option<&Path>) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read team package from {}", path.display()))?;
        let pkg: TeamPackage = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse {} as a TeamPackage", path.display()))?;
        let root = data_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
        Ok(Self {
            pkg,
            member_root: root.clone(),
            data_root: root,
            _guards: vec![],
        })
    }

    /// Synthesize the built-in two-member doc-review fixture into tempdirs.
    fn builtin_fixture() -> Result<Self> {
        let member_dir = tempfile::tempdir().context("create the fixture member dir")?;
        write_member(
            member_dir.path(),
            "drafter",
            "You draft documents for the team.",
        )?;
        write_member(
            member_dir.path(),
            "summarizer",
            "You summarize approved documents.",
        )?;
        let data_dir = tempfile::tempdir().context("create the fixture data dir")?;
        let member_root = member_dir.path().to_path_buf();
        let data_root = data_dir.path().to_path_buf();
        Ok(Self {
            pkg: builtin_team_package(),
            member_root,
            data_root,
            _guards: vec![member_dir, data_dir],
        })
    }
}

// ---------------------------------------------------------------------------
// The default transcript path (composition delegated to TeamRuntime — D-02).
// ---------------------------------------------------------------------------

/// Build the [`TeamRuntime`](pmcp_team_servers::compose::wiring) and drive the
/// 7-step doc-review transcript, printing one labeled line per step.
async fn run_transcript(
    team: &LoadedTeam,
    factory: Arc<dyn CompletionSourceFactory>,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let resolver = Arc::new(LocalDirPackageResolver::new(&team.member_root));
    let rt = TeamRuntimeBuilder::new(resolver, Arc::new(EnvVarResolver::new()))
        .with_completion_override(factory)
        .with_data_root(&team.data_root)
        .build(&team.pkg)
        .await
        .context("compose the team runtime (default flow, D-02)")?;

    let team_fs = require_client(rt.team_fs_client(), "team-fs")?;
    let approval = require_client(rt.approval_client(), "approval-mcp")?;
    let mem = require_client(rt.mem_client(), "mem-mcp")?;
    let team_mcp = require_client(rt.team_mcp_client(), "team-mcp")?;

    const DOC: &str = "design/review.md";
    let quiet = !global_flags.should_output();

    // Steps 1–2: the drafter writes + publishes the document.
    let written = team_fs
        .call_tool(
            "fs__write".to_string(),
            json!({ "path": DOC, "content": "# Q3 Launch Plan\n\nDraft for review.\n" }),
        )
        .await
        .context("step 1: fs__write")?;
    step(
        quiet,
        1,
        &format!("drafter writes {DOC} → {}", body(&written)),
    );
    team_fs
        .call_tool("fs__sync_to_review".to_string(), json!({ "path": DOC }))
        .await
        .context("step 2: fs__sync_to_review")?;
    step(quiet, 2, &format!("drafter publishes {DOC} for review"));

    // Steps 3–4: ask the human reviewer, then record the verdict.
    let approval_id = ask_reviewer(approval, DOC, quiet).await?;
    resolve_reviewer(approval, &approval_id, quiet).await?;

    // Steps 5–6: the summarizer reads the approved doc and stores a memory.
    let read = team_fs
        .call_tool("fs__read".to_string(), json!({ "path": DOC }))
        .await
        .context("step 5: fs__read")?;
    let len = body(&read)["content"].as_str().unwrap_or_default().len();
    step(quiet, 5, &format!("summarizer reads {DOC} ({len} bytes)"));
    mem.call_tool(
        "mem__add".to_string(),
        json!({ "text": format!("Approved '{DOC}' ({approval_id})"), "tags": ["doc-review"] }),
    )
    .await
    .context("step 6: mem__add")?;
    step(quiet, 6, "summarizer stores a memory of the approved doc");

    // Step 7: agent-facing dispatch surfaces the related-task pointer.
    dispatch_member(team_mcp, DOC, quiet).await?;

    let joined = rt.shutdown().await;
    if !quiet {
        println!(
            "\n{} doc-review flow complete — {joined} hosting task(s) torn down cleanly.",
            "✓".green().bold()
        );
    }
    Ok(())
}

/// Step 3: discover the per-role ask tool and request sign-off, returning the id.
async fn ask_reviewer(
    approval: &Client<DuplexTransport>,
    doc: &str,
    quiet: bool,
) -> Result<String> {
    let ask_tool = find_tool(approval, "team_approval__ask_")
        .await
        .context("discover the approval ask tool")?
        .context("no ask tool advertised (the team needs a human role)")?;
    let asked = approval
        .call_tool(
            ask_tool,
            json!({
                "question": format!("Approve '{doc}' for publication?"),
                "options": ["approve", "request-changes"],
                "subjectRef": doc
            }),
        )
        .await
        .context("step 3: ask the reviewer")?;
    let approval_id = body(&asked)["approvalId"]
        .as_str()
        .context("ask did not return an approvalId")?
        .to_string();
    step(
        quiet,
        3,
        &format!("drafter asks the reviewer → {approval_id}"),
    );
    Ok(approval_id)
}

/// Step 4: record the human verdict (approve).
async fn resolve_reviewer(
    approval: &Client<DuplexTransport>,
    approval_id: &str,
    quiet: bool,
) -> Result<()> {
    let resolved = approval
        .call_tool(
            "resolve_approval".to_string(),
            json!({ "approvalId": approval_id, "decision": "approve" }),
        )
        .await
        .context("step 4: resolve_approval")?;
    step(
        quiet,
        4,
        &format!("reviewer verdict recorded → {}", body(&resolved)["verdict"]),
    );
    Ok(())
}

/// Step 7: discover a member dispatch tool and route a follow-up task; surface
/// the related-task pointer.
async fn dispatch_member(team_mcp: &Client<DuplexTransport>, doc: &str, quiet: bool) -> Result<()> {
    let dispatch_tool = find_tool(team_mcp, "team_mcp__")
        .await
        .context("discover a member dispatch tool")?
        .context("no team_mcp__<member> tool advertised")?;
    let dispatched = team_mcp
        .call_tool(
            dispatch_tool,
            json!({ "message": format!("Announce '{doc}' is approved.") }),
        )
        .await
        .context("step 7: team_mcp dispatch")?;
    let related = dispatched
        ._meta
        .as_ref()
        .and_then(|m| m.get(RELATED_TASK_META_KEY))
        .map_or_else(|| "<none>".to_string(), Value::to_string);
    step(
        quiet,
        7,
        &format!("agent-facing dispatch → related-task {related}"),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The --serve path (the PUBLIC team-mcp binary recipe — no TeamRuntime).
// ---------------------------------------------------------------------------

/// Expose team-mcp over HTTP via the shipped `team-mcp` binary's public serve
/// recipe on `127.0.0.1:<port>`, running until Ctrl-C.
async fn serve_team_mcp(team: &LoadedTeam, port: u16, global_flags: &GlobalFlags) -> Result<()> {
    use pmcp::server::streamable_http_server::StreamableHttpServerConfig;

    let resolver = LocalDirPackageResolver::new(&team.member_root);
    let slot_resolver = EnvVarResolver::new();

    // The SAME member-wiring loop the shipped team-mcp binary uses.
    let mut members = Vec::with_capacity(team.pkg.members.len());
    let mut roster = Vec::with_capacity(team.pkg.members.len());
    for member in &team.pkg.members {
        let id = MemberId::from_ref(&member.agent);
        let agent_pkg = resolver
            .resolve_agent(&member.agent)
            .await
            .with_context(|| format!("resolve member {id}"))?;
        let config = resolve_agent(&agent_pkg, &slot_resolver)
            .await
            .with_context(|| format!("resolve member {id} config"))?;
        let factory = resolve_member_factory(&agent_pkg, &slot_resolver, None)
            .await
            .with_context(|| format!("resolve member {id} llm factory"))?;
        let handle = MemberHandle::spawn_from_package(
            id.clone(),
            agent_pkg,
            config,
            factory,
            MemberTaskForwarding::Synthesize,
        )
        .await
        .with_context(|| format!("spawn member {id}"))?;
        roster.push(id);
        members.push(handle);
    }

    let server = build_team_mcp_server(members, team.pkg.limits.max_team_depth, roster)
        .context("build the team-mcp server")?;

    if global_flags.should_output() {
        let url = format!("http://127.0.0.1:{port}");
        println!("Serving team-mcp over HTTP at {}", url.bright_cyan());
        println!("Point an MCP client at {url} (Ctrl-C to stop).");
    }

    // Run until the serve future ends or Ctrl-C arrives (graceful stop).
    tokio::select! {
        res = serve_streamable_http(server, "team-mcp", port, StreamableHttpServerConfig::default()) => {
            res.map_err(|e| anyhow::anyhow!("team-mcp HTTP serve failed: {e}"))?;
        },
        _ = tokio::signal::ctrl_c() => {
            if global_flags.should_output() {
                println!("\nreceived Ctrl-C — stopping team-mcp.");
            }
        },
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Completion override: offline FixedSource (default) or --llm OpenAiCompatSource.
// ---------------------------------------------------------------------------

/// Choose the member completion override: the offline [`EndTurnMock`] by default,
/// or an [`OpenAiCompatSource`] (validated once, wrapped in the exported
/// [`FixedSourceFactory`]) when `--llm <endpoint>` is set.
fn completion_factory(args: &DevArgs) -> Result<Arc<dyn CompletionSourceFactory>> {
    match args.llm.as_deref() {
        None => Ok(fixed_override()),
        Some(endpoint) => {
            // Validate the endpoint shape up front (actionable, not a panic).
            url::Url::parse(endpoint)
                .with_context(|| format!("invalid --llm endpoint URL: {endpoint}"))?;
            let key = resolve_api_key(args.llm_api_key_env.as_deref())?;
            let source = build_openai_compat_source(
                endpoint,
                &args.model,
                key,
                args.allow_insecure_http,
                "--llm (or drop it for the offline default)",
            )?;
            Ok(Arc::new(FixedSourceFactory::new(
                Arc::new(source) as Arc<dyn CompletionSource>
            )))
        },
    }
}

/// The injected offline completion override (a `FixedSource` bound to
/// [`EndTurnMock`]).
fn fixed_override() -> Arc<dyn CompletionSourceFactory> {
    Arc::new(FixedSourceFactory::new(
        Arc::new(EndTurnMock) as Arc<dyn CompletionSource>
    ))
}

/// A completion source that ends every turn immediately — keeps the default
/// transcript fully offline (no live LLM).
struct EndTurnMock;

#[async_trait]
impl CompletionSource for EndTurnMock {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "team-dev-mock",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "acknowledged".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

// ---------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------

/// Require a runtime client the transcript flow depends on, or bail with an
/// actionable message naming the missing server.
fn require_client<'a>(
    client: Option<&'a Arc<Client<DuplexTransport>>>,
    name: &str,
) -> Result<&'a Client<DuplexTransport>> {
    client.map(AsRef::as_ref).with_context(|| {
        format!(
            "the team package does not attach {name}; the built-in doc-review flow needs \
             ≥2 members, a human role, and team-fs + mem-mcp opt-ins"
        )
    })
}

/// Find the first advertised tool whose name starts with `prefix`.
async fn find_tool(client: &Client<DuplexTransport>, prefix: &str) -> Result<Option<String>> {
    Ok(client
        .list_tools(None)
        .await
        .context("tools/list")?
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with(prefix)))
}

/// Extract the JSON body a `TypedTool` returns from its first text content block.
fn body(res: &CallToolResult) -> Value {
    match res.content.first() {
        Some(Content::Text { text }) => {
            serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
        },
        _ => Value::Null,
    }
}

/// Print one labeled transcript line (unless quiet).
fn step(quiet: bool, n: u8, msg: &str) {
    if !quiet {
        println!("{} {msg}", format!("[step {n}]").bright_blue().bold());
    }
}

// ---------------------------------------------------------------------------
// The built-in doc-review fixture (the D-02 locked default).
// ---------------------------------------------------------------------------

/// A `^1` component reference of the given kind (the doc-review fixture's members
/// and built-in servers differ only by [`ComponentType`]).
fn component_ref(name: &str, component_type: ComponentType) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").expect("valid range"),
        component_type,
    }
}

fn member_pkg(name: &str, instructions: &str) -> AgentPackage {
    AgentPackage {
        name: name.to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: instructions.to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "team-dev-mock".to_string(),
        }),
        max_tokens: 4096,
        max_iterations: 5,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

/// Write a member `AgentPackage` to `<dir>/<name>.json` for the local resolver.
fn write_member(dir: &Path, name: &str, instructions: &str) -> Result<()> {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&member_pkg(name, instructions)).expect("serialize member"),
    )
    .with_context(|| format!("write member fixture {name}"))
}

/// The built-in two-member doc-review team + one human reviewer + team-fs &
/// mem-mcp opt-ins (mirrors the `doc_review_team` example).
fn builtin_team_package() -> TeamPackage {
    TeamPackage {
        name: "doc-review-team".to_string(),
        version: semver::Version::new(1, 0, 0),
        entry_point: component_ref("drafter", ComponentType::Agent),
        members: vec![
            TeamMember {
                agent: component_ref("drafter", ComponentType::Agent),
                role: TeamRole::EntryPoint,
            },
            TeamMember {
                agent: component_ref("summarizer", ComponentType::Agent),
                role: TeamRole::Member,
            },
        ],
        human_roles: vec![HumanRole {
            role: "reviewer".to_string(),
            description: "A human who signs off on the drafted document.".to_string(),
            responsibilities: vec![],
            channel_hints: vec![],
        }],
        // Dev-harness limits: generous enough that `--llm` runs against a real
        // model are not killed instantly (the offline transcript ends every turn
        // immediately, so it never approaches these).
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 2_000_000,
            max_team_wall_clock_seconds: 300,
            poll_interval_ms: 50,
        },
        built_in_servers: vec![
            component_ref("team-fs", ComponentType::Server),
            component_ref("mem-mcp", ComponentType::Server),
        ],
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![],
    }
}
