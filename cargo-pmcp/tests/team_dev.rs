//! CLI-03 behavioral tests for `cargo pmcp team dev`.
//!
//! Three mechanisms, each exercised end-to-end WITHOUT any external network or
//! live LLM:
//!
//! 1. **Default transcript** — a two-member doc-review team composed in ONE
//!    process via [`TeamRuntimeBuilder`] over in-memory transports, driven
//!    through the 7-step doc-review flow on an injected `FixedSource`
//!    ([`fixed_override`]). Deterministic; asserts the step ordering and a clean
//!    `rt.shutdown()` count.
//! 2. **`--serve`** — the PUBLIC `team-mcp` binary recipe (`build_team_mcp_server`
//!    over the member-wiring loop, then a streamable-HTTP server) bound to an
//!    ephemeral loopback port; an MCP client completes a `tools/list` over HTTP
//!    and sees the `team_mcp__<member>` tools, then the server is aborted.
//! 3. **`--llm`** — an [`OpenAiCompatSource`] pointed at a local `mockito`
//!    endpoint (canned end-turn chat/completions response) wrapped in the
//!    exported [`FixedSourceFactory`], driving a member dispatch; asserts the
//!    mock endpoint was hit and a terminal related-task pointer surfaced.
//!
//! These mirror the primitives the `team dev` handler uses (D-02 default flow,
//! Codex 110-04 HIGH `--serve`/`--llm` shapes), so a green suite proves the
//! wiring the CLI depends on — not compile-only.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::tasks::RELATED_TASK_META_KEY;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, Role};
use pmcp::Client;

use pmcp_agent::sources::{HttpSourceOptions, OpenAiCompatSource, SecretString};
use pmcp_agent::{
    resolve_agent, CompletionError, CompletionSource, CompletionSourceFactory, EnvVarResolver,
    FixedSourceFactory, ProgrammaticBuilder, SlotResolver,
};

use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::reference::ComponentType;
use pmcp_package::slot::SlotType;
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};

use pmcp_team_servers::compose::resolver::{LocalDirPackageResolver, PackageResolver};
use pmcp_team_servers::compose::wiring::TeamRuntimeBuilder;
use pmcp_team_servers::team::identity::{MemberId, MemberTaskForwarding};
use pmcp_team_servers::team::member::{resolve_member_factory, MemberHandle};
use pmcp_team_servers::team::server::build_team_mcp_server;

use url::Url;

// ---------------------------------------------------------------------------
// Deterministic, network-free fixtures (mirroring the reference
// example/tests in pmcp-team-servers).
// ---------------------------------------------------------------------------

/// A completion source that ends every turn immediately — the injected override
/// that keeps the default transcript fully offline (no live LLM).
struct EndTurnMock;

#[async_trait]
impl CompletionSource for EndTurnMock {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "doc-review-mock",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "acknowledged".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

/// The injected completion override (a `FixedSource` bound to [`EndTurnMock`]).
fn fixed_override() -> Arc<dyn CompletionSourceFactory> {
    Arc::new(FixedSourceFactory::new(
        Arc::new(EndTurnMock) as Arc<dyn CompletionSource>
    ))
}

fn agent_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Agent,
    }
}

fn server_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Server,
    }
}

fn human_role(label: &str) -> HumanRole {
    HumanRole {
        role: label.to_string(),
        description: "A human who signs off on the drafted document.".to_string(),
        responsibilities: vec![],
        channel_hints: vec![],
    }
}

fn member_pkg(name: &str) -> AgentPackage {
    AgentPackage {
        name: name.to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        instructions: "You are a helpful team member. Be brief.".to_string(),
        // Mandatory llm slot — resolved to its tested value, never actually used
        // for the default flow (the FixedSource override is injected).
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "test-model".to_string(),
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

/// Write a member `AgentPackage` to `<dir>/<name>.json` so the
/// `LocalDirPackageResolver` can resolve its bare `ComponentRef`.
fn write_member(dir: &std::path::Path, name: &str) {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&member_pkg(name)).unwrap(),
    )
    .unwrap();
}

/// Tight, offline-friendly limits (matching the reference example): the default
/// FixedSource flow ends the turn before any budget matters.
fn tight_limits() -> TeamLimits {
    TeamLimits {
        max_team_depth: 3,
        max_team_total_tokens: 1,
        max_team_wall_clock_seconds: 1,
        poll_interval_ms: 1,
    }
}

/// Generous limits so a real (mock) LLM call is actually made before any budget
/// trip — used by the `--llm` mock-endpoint test.
fn generous_limits() -> TeamLimits {
    TeamLimits {
        max_team_depth: 3,
        max_team_total_tokens: 1_000_000,
        max_team_wall_clock_seconds: 30,
        poll_interval_ms: 5,
    }
}

/// Build a `TeamPackage` from member names, human-role labels, opt-in server
/// names, and explicit limits.
fn team_package(
    members: &[&str],
    humans: &[&str],
    built_ins: &[&str],
    limits: TeamLimits,
) -> TeamPackage {
    TeamPackage {
        name: "doc-review-team".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        entry_point: agent_ref(members[0]),
        members: members
            .iter()
            .enumerate()
            .map(|(i, n)| TeamMember {
                agent: agent_ref(n),
                role: if i == 0 {
                    TeamRole::EntryPoint
                } else {
                    TeamRole::Member
                },
            })
            .collect(),
        human_roles: humans.iter().map(|h| human_role(h)).collect(),
        limits,
        built_in_servers: built_ins.iter().map(|s| server_ref(s)).collect(),
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![],
    }
}

fn stub_slot_resolver() -> Arc<dyn SlotResolver> {
    Arc::new(ProgrammaticBuilder::new())
}

/// Extract the JSON body a `TypedTool` returns from the first text content block.
fn body(res: &CallToolResult) -> Value {
    match res.content.first() {
        Some(Content::Text { text }) => {
            serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
        },
        _ => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// (1) Default transcript: the 7-step doc-review flow offline, in order.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn transcript_drives_seven_step_doc_review_offline() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "drafter");
    write_member(pkg_dir.path(), "summarizer");
    let data_dir = tempfile::tempdir().unwrap();

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package(
        &["drafter", "summarizer"],
        &["reviewer"],
        &["team-fs", "mem-mcp"],
        tight_limits(),
    );

    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .with_data_root(data_dir.path())
        .build(&pkg)
        .await
        .expect("doc-review team composes in one process");

    let hosted = rt.hosted_task_count();
    assert!(
        hosted >= 4,
        "team-mcp + approval-mcp + team-fs + mem-mcp: {hosted}"
    );

    let team_fs = rt.team_fs_client().expect("team-fs attached");
    let approval = rt.approval_client().expect("approval-mcp attached");
    let mem = rt.mem_client().expect("mem-mcp attached");
    let team_mcp = rt.team_mcp_client().expect("team-mcp attached");

    const DOC: &str = "design/review.md";

    // Step 1: drafter writes the document into the shared workspace.
    let written = team_fs
        .call_tool(
            "fs__write".to_string(),
            json!({ "path": DOC, "content": "# Q3 Launch Plan\n\nDraft.\n" }),
        )
        .await
        .expect("fs__write");
    assert!(
        !body(&written).is_null(),
        "step 1: fs__write returns a body"
    );

    // Step 2: publish the draft for review.
    team_fs
        .call_tool("fs__sync_to_review".to_string(), json!({ "path": DOC }))
        .await
        .expect("step 2: fs__sync_to_review");

    // Step 3: ask the human reviewer for sign-off (dynamic per-role ask tool).
    let ask_tool = approval
        .list_tools(None)
        .await
        .expect("approval tools/list")
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with("team_approval__ask_"))
        .expect("one ask tool per human role");
    let asked = approval
        .call_tool(
            ask_tool,
            json!({
                "question": format!("Approve '{DOC}'?"),
                "options": ["approve", "request-changes"],
                "subjectRef": DOC
            }),
        )
        .await
        .expect("step 3: ask");
    let approval_id = body(&asked)["approvalId"]
        .as_str()
        .expect("step 3: ask returns an approvalId")
        .to_string();
    assert!(!approval_id.is_empty(), "step 3: approvalId is non-empty");

    // Step 4: record the human verdict.
    let resolved = approval
        .call_tool(
            "resolve_approval".to_string(),
            json!({ "approvalId": approval_id, "decision": "approve" }),
        )
        .await
        .expect("step 4: resolve_approval");
    assert_eq!(
        body(&resolved)["verdict"],
        "approve",
        "step 4: the recorded verdict is approve"
    );

    // Step 5: summarizer reads the approved document.
    let read = team_fs
        .call_tool("fs__read".to_string(), json!({ "path": DOC }))
        .await
        .expect("step 5: fs__read");
    let content_len = body(&read)["content"].as_str().unwrap_or_default().len();
    assert!(
        content_len > 0,
        "step 5: fs__read returns the document body"
    );

    // Step 6: store a memory of the reviewed document.
    let remembered = mem
        .call_tool(
            "mem__add".to_string(),
            json!({ "text": format!("Approved '{DOC}'"), "tags": ["doc-review"] }),
        )
        .await
        .expect("step 6: mem__add");
    assert!(
        !body(&remembered).is_null(),
        "step 6: mem__add returns a body"
    );

    // Step 7: agent-facing dispatch surfaces the related-task pointer.
    let dispatch_tool = team_mcp
        .list_tools(None)
        .await
        .expect("team-mcp tools/list")
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with("team_mcp__"))
        .expect("one team_mcp__<member> tool per member");
    let dispatched = team_mcp
        .call_tool(
            dispatch_tool,
            json!({ "message": format!("Announce '{DOC}' is approved.") }),
        )
        .await
        .expect("step 7: team_mcp dispatch");
    assert!(
        dispatched
            ._meta
            .as_ref()
            .and_then(|m| m.get(RELATED_TASK_META_KEY))
            .is_some(),
        "step 7: dispatch surfaces the related-task _meta pointer"
    );

    // Clean shutdown: every hosting task is aborted and joined.
    let joined = tokio::time::timeout(Duration::from_secs(5), rt.shutdown())
        .await
        .expect("shutdown completes promptly (no hung task)");
    assert_eq!(joined, hosted, "every hosting task torn down cleanly");
}

// ---------------------------------------------------------------------------
// (2) --serve: the public team-mcp binary recipe over an ephemeral loopback port.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn serve_exposes_team_mcp_over_http() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "drafter");
    write_member(pkg_dir.path(), "summarizer");

    let pkg = team_package(
        &["drafter", "summarizer"],
        &["reviewer"],
        &["team-fs", "mem-mcp"],
        tight_limits(),
    );

    // The SAME member-wiring loop the shipped `team-mcp` binary (and the `--serve`
    // branch) uses: resolve each member's AgentPackage + llm slot, spawn it, and
    // build the team-mcp server. No injected override — the concrete member LLM
    // source is constructed but never invoked by `tools/list`.
    let resolver = LocalDirPackageResolver::new(pkg_dir.path());
    let slot_resolver = EnvVarResolver::new();
    let mut members = Vec::with_capacity(pkg.members.len());
    let mut roster = Vec::with_capacity(pkg.members.len());
    for member in &pkg.members {
        let id = MemberId::from_ref(&member.agent);
        let agent_pkg = resolver.resolve_agent(&member.agent).await.unwrap();
        let config = resolve_agent(&agent_pkg, &slot_resolver).await.unwrap();
        let factory = resolve_member_factory(&agent_pkg, &slot_resolver, None)
            .await
            .unwrap();
        let handle = MemberHandle::spawn_from_package(
            id.clone(),
            agent_pkg,
            config,
            factory,
            MemberTaskForwarding::Synthesize,
        )
        .await
        .unwrap();
        roster.push(id);
        members.push(handle);
    }
    let server = build_team_mcp_server(members, pkg.limits.max_team_depth, roster).unwrap();

    // Ephemeral loopback: bind 127.0.0.1:0 and read back the OS-assigned address.
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let http = StreamableHttpServer::new(bind_addr, Arc::new(tokio::sync::Mutex::new(server)));
    let (bound, handle) = http.start().await.expect("streamable HTTP server binds");

    let config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{bound}")).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };
    let mut client = Client::new(StreamableHttpTransport::new(config));
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("MCP initialize over HTTP");
    let tools = client
        .list_tools(None)
        .await
        .expect("tools/list over HTTP")
        .tools;
    let team_tools = tools
        .iter()
        .filter(|t| t.name.starts_with("team_mcp__"))
        .count();
    assert_eq!(
        team_tools, 2,
        "one team_mcp__<member> tool per member over HTTP"
    );

    // Clean shutdown: abort the serve task, no lingering listener.
    handle.abort();
}

// ---------------------------------------------------------------------------
// (3) --llm: an OpenAiCompatSource wrapped in FixedSourceFactory against a
//     local mockito endpoint drives a member dispatch (no real LLM).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn llm_drives_against_mock_endpoint() {
    // A local OpenAI-compat mock returning a canned end-turn completion.
    let mut mock_server = mockito::Server::new_async().await;
    let mock = mock_server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"model":"test-model","choices":[{"message":{"content":"acknowledged"},"finish_reason":"stop"}]}"#,
        )
        .expect_at_least(1)
        .create_async()
        .await;
    let endpoint = mock_server.url();

    // Construct + VALIDATE the source ONCE (mock is plain-http loopback), then
    // wrap it in the EXPORTED FixedSourceFactory (the correct sync/infallible
    // shape — NOT a custom fallible factory).
    let source = OpenAiCompatSource::with_options(
        &endpoint,
        "test-model",
        SecretString::new("test"),
        HttpSourceOptions {
            allow_insecure_http: true,
            ..Default::default()
        },
    )
    .expect("mock endpoint is loopback plain-http");
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(Arc::new(
        source,
    )
        as Arc<dyn CompletionSource>));

    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "drafter");
    write_member(pkg_dir.path(), "summarizer");
    let data_dir = tempfile::tempdir().unwrap();

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package(
        &["drafter", "summarizer"],
        &["reviewer"],
        &["team-fs", "mem-mcp"],
        generous_limits(),
    );

    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(factory)
        .with_data_root(data_dir.path())
        .build(&pkg)
        .await
        .expect("team composes with the mock-backed override");

    // A member dispatch runs the member agent loop, which calls the (mock) LLM
    // and — on the end-turn response — completes with a related-task pointer.
    let team_mcp = rt.team_mcp_client().expect("team-mcp attached");
    let dispatch_tool = team_mcp
        .list_tools(None)
        .await
        .expect("team-mcp tools/list")
        .tools
        .into_iter()
        .map(|t| t.name)
        .find(|n| n.starts_with("team_mcp__"))
        .expect("one team_mcp__<member> tool per member");
    let dispatched = team_mcp
        .call_tool(dispatch_tool, json!({ "message": "Announce the plan." }))
        .await
        .expect("member dispatch drives the mock-backed loop");

    // Terminal step reached: a related-task pointer surfaced.
    assert!(
        dispatched
            ._meta
            .as_ref()
            .and_then(|m| m.get(RELATED_TASK_META_KEY))
            .is_some(),
        "dispatch reaches a terminal step (related-task pointer)"
    );
    // The mock LLM endpoint was actually hit — no real network/LLM.
    mock.assert_async().await;

    rt.shutdown().await;
}
