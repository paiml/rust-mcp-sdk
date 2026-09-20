//! Example: the offline doc-review team transcript (CLI-03).
//!
//! Mirrors the DEFAULT flow of `cargo pmcp team dev` (plan 110-04): it composes
//! the built-in two-member doc-review team in ONE process via
//! [`TeamRuntimeBuilder`] over in-memory transports, injects an offline
//! `FixedSource` completion override (no live LLM, no sockets, no network), and
//! walks the 7-step doc-review flow — printing one labeled transcript line per
//! step. Composition is driven ENTIRELY through [`TeamRuntime`] (D-02 — the
//! example does NOT hand-roll server spin-up), the same primitive the `team dev`
//! default path uses.
//!
//! Determinism: an injected [`FixedSourceFactory`] (an end-turn mock) replaces
//! any live LLM, so the run is fully offline and CI-reproducible. All state lives
//! under auto-cleaned [`tempfile::tempdir`] roots.
//!
//! This satisfies the CLAUDE.md ALWAYS EXAMPLE requirement for CLI-03.
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example team_dev_transcript

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::tasks::RELATED_TASK_META_KEY;
use pmcp::types::{CallToolResult, Content, Role};

use pmcp_agent::{
    CompletionError, CompletionSource, CompletionSourceFactory, FixedSourceFactory,
    ProgrammaticBuilder, SlotResolver,
};

use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::reference::ComponentType;
use pmcp_package::slot::SlotType;
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};

use pmcp_team_servers::compose::resolver::LocalDirPackageResolver;
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;

// ---------------------------------------------------------------------------
// Deterministic, network-free fixtures (mirror team dev's built-in default).
// ---------------------------------------------------------------------------

/// A completion source that ends every turn immediately — keeps the transcript
/// fully offline (no live LLM).
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

/// The injected offline completion override (a `FixedSource` bound to
/// [`EndTurnMock`]).
fn fixed_override() -> Arc<dyn CompletionSourceFactory> {
    Arc::new(FixedSourceFactory::new(
        Arc::new(EndTurnMock) as Arc<dyn CompletionSource>
    ))
}

fn stub_slot_resolver() -> Arc<dyn SlotResolver> {
    Arc::new(ProgrammaticBuilder::new())
}

fn agent_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").expect("valid range"),
        component_type: ComponentType::Agent,
    }
}

fn server_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").expect("valid range"),
        component_type: ComponentType::Server,
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
fn write_member(dir: &Path, name: &str, instructions: &str) {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&member_pkg(name, instructions)).expect("serialize member"),
    )
    .expect("write member fixture");
}

/// The built-in two-member doc-review team + one human reviewer + team-fs &
/// mem-mcp opt-ins (mirrors `team dev`'s D-02 locked default fixture).
fn team_package() -> TeamPackage {
    TeamPackage {
        name: "doc-review-team".to_string(),
        version: semver::Version::new(1, 0, 0),
        entry_point: agent_ref("drafter"),
        members: vec![
            TeamMember {
                agent: agent_ref("drafter"),
                role: TeamRole::EntryPoint,
            },
            TeamMember {
                agent: agent_ref("summarizer"),
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
        built_in_servers: vec![server_ref("team-fs"), server_ref("mem-mcp")],
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![],
    }
}

// ---------------------------------------------------------------------------
// Small helpers for the narrative.
// ---------------------------------------------------------------------------

/// Extract the JSON body a `TypedTool` returns from its first text content block.
fn body(res: &CallToolResult) -> Value {
    match res.content.first() {
        Some(Content::Text { text }) => {
            serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
        },
        _ => Value::Null,
    }
}

fn step(n: u8, msg: &str) {
    println!("\n── Step {n} ──────────────────────────────────────────");
    println!("{msg}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║  cargo pmcp team dev — offline doc-review transcript     ║");
    println!("║  (team-fs · mem-mcp · approval-mcp · team-mcp)           ║");
    println!("╚════════════════════════════════════════════════════════╝");

    // --- Compose the team in one process (in-memory transports) -------------
    let pkg_dir = tempfile::tempdir()?;
    write_member(
        pkg_dir.path(),
        "drafter",
        "You draft documents for the team.",
    );
    write_member(
        pkg_dir.path(),
        "summarizer",
        "You summarize approved documents.",
    );
    let data_dir = tempfile::tempdir()?;

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package();

    println!(
        "\nComposing 2 members ('drafter', 'summarizer') + 1 human role \
         ('reviewer') + team-fs & mem-mcp opt-ins over in-memory transports…"
    );
    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override()) // offline, deterministic
        .with_data_root(data_dir.path())
        .build(&pkg)
        .await?;

    let team_fs = rt.team_fs_client().expect("team-fs attached (opt-in)");
    let approval = rt
        .approval_client()
        .expect("approval-mcp attached (1 human)");
    let mem = rt.mem_client().expect("mem-mcp attached (opt-in)");
    let team_mcp = rt.team_mcp_client().expect("team-mcp attached (2 members)");

    const DOC: &str = "design/review.md";

    // --- Step 1: the drafter writes the document into the workspace ---------
    step(
        1,
        "Drafter writes the document into the shared workspace via team-fs (fs__write).",
    );
    let written = team_fs
        .call_tool(
            "fs__write".to_string(),
            json!({ "path": DOC, "content": "# Q3 Launch Plan\n\nDraft for review.\n" }),
        )
        .await?;
    println!("   fs__write → {}", body(&written));

    // --- Step 2: publish the draft into the review/ tree --------------------
    step(
        2,
        "Drafter publishes the draft for review via team-fs (fs__sync_to_review).",
    );
    let synced = team_fs
        .call_tool("fs__sync_to_review".to_string(), json!({ "path": DOC }))
        .await?;
    println!("   fs__sync_to_review → {}", body(&synced));

    // --- Step 3: ask the human reviewer for sign-off ------------------------
    let ask_tool = approval
        .list_tools(None)
        .await?
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with("team_approval__ask_"))
        .expect("one ask tool per human role");

    step(
        3,
        &format!(
            "Drafter asks the human reviewer for sign-off via approval-mcp ({ask_tool}), \
             linking the draft as the subject."
        ),
    );
    let asked = approval
        .call_tool(
            ask_tool,
            json!({
                "question": format!("Approve '{DOC}' for publication?"),
                "options": ["approve", "request-changes"],
                "subjectRef": DOC
            }),
        )
        .await?;
    let approval_id = body(&asked)["approvalId"]
        .as_str()
        .expect("ask returns an approvalId")
        .to_string();
    println!("   ask → approvalId={approval_id}");

    // --- Step 4: the human verdict is recorded ------------------------------
    step(
        4,
        "The human reviewer's verdict is recorded via resolve_approval (decision: approve).",
    );
    let resolved = approval
        .call_tool(
            "resolve_approval".to_string(),
            json!({ "approvalId": approval_id, "decision": "approve" }),
        )
        .await?;
    println!(
        "   resolve_approval → verdict={}",
        body(&resolved)["verdict"]
    );

    // --- Step 5: the summarizer reads the approved document -----------------
    step(
        5,
        "Summarizer reads the approved document via team-fs (fs__read).",
    );
    let read = team_fs
        .call_tool("fs__read".to_string(), json!({ "path": DOC }))
        .await?;
    let len = body(&read)["content"].as_str().unwrap_or_default().len();
    println!("   fs__read → {len} bytes");

    // --- Step 6: store a memory of the reviewed document --------------------
    step(
        6,
        "Summarizer stores a summary of the approved document via mem-mcp (mem__add).",
    );
    let remembered = mem
        .call_tool(
            "mem__add".to_string(),
            json!({
                "text": format!("Reviewed & approved '{DOC}' (approval {approval_id})."),
                "tags": ["doc-review", "approved"]
            }),
        )
        .await?;
    println!("   mem__add → {}", body(&remembered));

    // --- Step 7: agent-facing dispatch through team-mcp ---------------------
    let dispatch_tool = team_mcp
        .list_tools(None)
        .await?
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with("team_mcp__"))
        .expect("one team_mcp__<member> tool per member");

    step(
        7,
        &format!(
            "Agent-facing dispatch: route a follow-up task to a member via team-mcp \
             ({dispatch_tool}) and surface the related-task _meta."
        ),
    );
    let dispatched = team_mcp
        .call_tool(
            dispatch_tool,
            json!({ "message": format!("Announce that '{DOC}' is approved.") }),
        )
        .await?;
    let related = dispatched
        ._meta
        .as_ref()
        .and_then(|m| m.get(RELATED_TASK_META_KEY))
        .map_or_else(|| "<none>".to_string(), |v| v.to_string());
    println!("   team_mcp dispatch → related-task _meta[{RELATED_TASK_META_KEY}] = {related}");

    // --- Clean shutdown -----------------------------------------------------
    let joined = rt.shutdown().await;
    println!("\n✅ doc-review flow complete — {joined} hosting task(s) torn down cleanly.");
    Ok(())
}
