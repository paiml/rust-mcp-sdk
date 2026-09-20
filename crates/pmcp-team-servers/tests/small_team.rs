//! "Small team, one process" integration tests for the [`TeamRuntime`] wiring
//! API (109-06, D-01/D-04).
//!
//! Every test binds a deterministic, CI-safe `FixedSource` completion override
//! (no live LLM / no network), a `LocalDirPackageResolver` rooted at a temp dir
//! holding the member `AgentPackage`s, and a stub `ProgrammaticBuilder`
//! `SlotResolver`. The member `AgentPackage.llm` slot is mandatory, so each
//! member carries a real (unused) llm `ConfigSlot`; the injected override means
//! no live slot resolution ever occurs.
//!
//! Proven here:
//! - A ≥2-member team with a human role + `built_in_servers` brings up team-mcp,
//!   approval-mcp, team-fs, AND mem-mcp in ONE process over in-memory transports.
//! - The team-of-one, zero-human degenerate case wires ONLY the single member
//!   `AgentServer` (no team-mcp/approval-mcp/opt-in).
//! - A requested opt-in that is unknown, or disabled by the enabled-server
//!   policy, FAILS CLOSED with [`RuntimeError::UnsupportedServer`].
//! - Explicit shutdown tears the hosted servers down cleanly (no leak).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use pmcp::types::protocol::RequestMeta;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::Role;

use pmcp_agent::{
    CompletionError, CompletionSource, CompletionSourceFactory, FixedSourceFactory,
    ProgrammaticBuilder, SlotResolver,
};

use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::reference::ComponentType;
use pmcp_package::slot::SlotType;
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};

use pmcp_team_servers::compose::resolver::LocalDirPackageResolver;
use pmcp_team_servers::compose::wiring::{EnabledServers, RuntimeError, TeamRuntimeBuilder};

// ---------------------------------------------------------------------------
// Deterministic, network-free fixtures.
// ---------------------------------------------------------------------------

/// A completion source that ends the turn immediately with a fixed answer — the
/// injected override that keeps every test CI-deterministic (no live LLM).
struct EndTurnMock;

#[async_trait]
impl CompletionSource for EndTurnMock {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "ok".to_string(),
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
        description: "A reviewing human role.".to_string(),
        responsibilities: vec![],
        channel_hints: vec![],
    }
}

fn member_pkg(name: &str) -> AgentPackage {
    AgentPackage {
        name: name.to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        instructions: "You are a helpful team member. Be brief.".to_string(),
        // Mandatory llm slot — resolved to its tested value by the stub
        // resolver, but never actually used because the FixedSource override is
        // injected.
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

/// Build a `TeamPackage` from member names, human-role labels, and built-in
/// (opt-in) server names.
fn team_package(members: &[&str], humans: &[&str], built_ins: &[&str]) -> TeamPackage {
    TeamPackage {
        name: "small-team".to_string(),
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
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 1,
            max_team_wall_clock_seconds: 1,
            poll_interval_ms: 1,
        },
        built_in_servers: built_ins.iter().map(|s| server_ref(s)).collect(),
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![],
    }
}

fn stub_slot_resolver() -> Arc<dyn SlotResolver> {
    Arc::new(ProgrammaticBuilder::new())
}

// ---------------------------------------------------------------------------
// (1) Small team, one process: all four servers reachable.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn small_team_one_process_brings_up_all_servers() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "triage");
    write_member(pkg_dir.path(), "formatter");
    let data_dir = tempfile::tempdir().unwrap();

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package(
        &["triage", "formatter"],
        &["reviewer"],
        &["team-fs", "mem-mcp"],
    );

    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .with_data_root(data_dir.path())
        .build(&pkg)
        .await
        .expect("small team starts in one process");

    // Derivation: team-mcp (2 members) + approval-mcp (1 human) + 2 opt-ins.
    let att = rt.attachment();
    assert!(att.team_mcp, "≥2 members ⇒ team-mcp");
    assert!(att.approval_mcp, "≥1 human role ⇒ approval-mcp");
    assert_eq!(att.opt_ins.len(), 2, "team-fs + mem-mcp opt-ins");

    // team-mcp: one dispatch tool per roster member.
    let team_tools = rt
        .team_mcp_client()
        .expect("team-mcp attached")
        .list_tools(None)
        .await
        .expect("team-mcp tools/list")
        .tools;
    assert_eq!(
        team_tools
            .iter()
            .filter(|t| t.name.starts_with("team_mcp__"))
            .count(),
        2,
        "one team_mcp__<member> tool per member"
    );

    // approval-mcp: the two unnamespaced tools + one ask tool per human role.
    let approval_names: Vec<String> = rt
        .approval_client()
        .expect("approval-mcp attached")
        .list_tools(None)
        .await
        .expect("approval-mcp tools/list")
        .tools
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert!(approval_names.iter().any(|n| n == "resolve_approval"));
    assert!(approval_names.iter().any(|n| n == "get_approval"));
    assert!(approval_names
        .iter()
        .any(|n| n.starts_with("team_approval__ask_")));

    // team-fs: exactly the 11 fs__* tools.
    let fs_tools = rt
        .team_fs_client()
        .expect("team-fs attached")
        .list_tools(None)
        .await
        .expect("team-fs tools/list")
        .tools;
    assert_eq!(fs_tools.len(), 11, "team-fs advertises 11 fs__* tools");
    assert!(fs_tools.iter().all(|t| t.name.starts_with("fs__")));

    // mem-mcp: exactly the 6 mem__* tools.
    let mem_tools = rt
        .mem_client()
        .expect("mem-mcp attached")
        .list_tools(None)
        .await
        .expect("mem-mcp tools/list")
        .tools;
    assert_eq!(mem_tools.len(), 6, "mem-mcp advertises 6 mem__* tools");
    assert!(mem_tools.iter().all(|t| t.name.starts_with("mem__")));

    rt.shutdown().await;
}

// ---------------------------------------------------------------------------
// (2) Team-of-one, zero humans: only the single member AgentServer.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn team_of_one_wires_only_the_member() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "solo");

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package(&["solo"], &[], &[]);

    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .build(&pkg)
        .await
        .expect("team-of-one starts");

    // No dispatch/approval/opt-in servers attach.
    assert!(!rt.attachment().team_mcp);
    assert!(!rt.attachment().approval_mcp);
    assert!(rt.attachment().opt_ins.is_empty());
    assert!(rt.team_mcp_client().is_none());
    assert!(rt.approval_client().is_none());
    assert!(rt.team_fs_client().is_none());
    assert!(rt.mem_client().is_none());

    // The sole member AgentServer IS wired and reachable.
    let member = rt.solo_member().expect("the sole member is wired");
    let result = member
        .dispatch(json!({ "message": "hello" }), RequestMeta::new())
        .await
        .expect("sole member dispatch succeeds");
    assert!(
        result._meta.is_some(),
        "the live member returns a related-task envelope"
    );

    rt.shutdown().await;
}

// ---------------------------------------------------------------------------
// (3) Reduced-feature / fail-closed opt-in handling.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_optin_fails_closed() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "solo");

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    // A built-in server nobody knows how to wire.
    let pkg = team_package(&["solo"], &[], &["ghost-mcp"]);

    let err = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .build(&pkg)
        .await
        .err()
        .expect("an unknown opt-in must fail closed");
    assert!(
        matches!(err, RuntimeError::UnsupportedServer { name } if name == "ghost-mcp"),
        "unknown opt-in yields UnsupportedServer"
    );
}

#[tokio::test]
async fn policy_disabled_optin_fails_closed() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "solo");

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    // mem-mcp is compiled (all-features), but the policy disables it — the same
    // fail-closed path a reduced-feature build takes for an uncompiled server.
    let pkg = team_package(&["solo"], &[], &["mem-mcp"]);

    let err = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .with_enabled_servers(EnabledServers::all().without("mem-mcp"))
        .build(&pkg)
        .await
        .err()
        .expect("a policy-disabled opt-in must fail closed");
    assert!(
        matches!(err, RuntimeError::UnsupportedServer { name } if name == "mem-mcp"),
        "policy-disabled opt-in yields UnsupportedServer"
    );
}

// ---------------------------------------------------------------------------
// (4) Clean shutdown: hosted servers are torn down (no leak).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shutdown_tears_down_hosted_servers() {
    let pkg_dir = tempfile::tempdir().unwrap();
    write_member(pkg_dir.path(), "triage");
    write_member(pkg_dir.path(), "formatter");
    let data_dir = tempfile::tempdir().unwrap();

    let resolver = Arc::new(LocalDirPackageResolver::new(pkg_dir.path()));
    let pkg = team_package(&["triage", "formatter"], &["reviewer"], &["team-fs"]);

    let rt = TeamRuntimeBuilder::new(resolver, stub_slot_resolver())
        .with_completion_override(fixed_override())
        .with_data_root(data_dir.path())
        .build(&pkg)
        .await
        .expect("runtime starts");

    // Two servers attach (team-mcp + team-fs); approval-mcp too (1 human role).
    let hosted = rt.hosted_task_count();
    assert!(hosted >= 2, "hosting tasks tracked for teardown: {hosted}");

    // Sanity: a hosted server round-trips a real call while the runtime is up
    // (using the runtime's own client — no external clone keeping it alive).
    rt.team_fs_client()
        .expect("team-fs attached")
        .call_tool("fs__list".to_string(), json!({}))
        .await
        .expect("team-fs responds while the runtime is up");

    // A clean shutdown returns promptly (a leaked/hung task would block here) and
    // accounts for every tracked hosting task — none leaks.
    let joined = tokio::time::timeout(Duration::from_secs(5), rt.shutdown())
        .await
        .expect("shutdown completes promptly (no hung task)");
    assert_eq!(joined, hosted, "every hosting task was aborted and joined");
}
