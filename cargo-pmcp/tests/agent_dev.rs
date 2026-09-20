//! Phase 110-03 CLI-02 — `cargo pmcp agent dev` behavioral tests.
//!
//! Two fully-offline, deterministic proofs of the two runnable agent-dev modes
//! (no network, no real sockets, no Ollama):
//!
//!   1. `fixed_source_runs_offline` — the `--source fixed` path drives the
//!      `pmcp-agent` loop through the lib-safe runner seam
//!      (`cargo_pmcp::agent_run::run_fixed_source`) to a terminal
//!      `RunOutcome::Completed`, AND the REAL built binary
//!      `cargo pmcp agent dev --source fixed` exits 0. The seam is the SAME
//!      production path the CLI fixed arm and the 110-06 example both call.
//!   2. `sampling_hosted_run_in_process` — the `--source sampling` server shape
//!      (`AgentServer` + `SamplingSourceFactory`) is exercised over an in-process
//!      `DuplexTransport` by a real `pmcp::Client` whose `on_sampling_with_tools`
//!      host scripts an immediate `end_turn`. The tool call is driven to a
//!      TERMINAL task status and a non-empty result — no sockets, no LLM.
//!
//! The seam module `cargo_pmcp::agent_run` is `commands/agent/run.rs` mounted via
//! a `#[path]` seam in `lib.rs` (the `commands::*` tree is bin-only; this mirrors
//! the established `templates_agent` / `workbook_explain` convention).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use pmcp::client::host::HostSamplingHandlerWithTools;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Content, Role};
use pmcp::{ClientBuilder, ToolCallResponse};

use cargo_pmcp::agent_run::NoopInvoker;
use pmcp_agent::{
    AgentServer, CompletionSourceFactory, InMemoryStore, ResolvedAgentConfig, RunOutcome,
    SamplingSourceFactory,
};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};
use pmcp_team_servers::transport::DuplexTransport;

/// A demo resolved config shared by the tests (mirrors the built-in demo the CLI
/// falls back to when no package is supplied).
fn demo_config() -> ResolvedAgentConfig {
    ResolvedAgentConfig::new(
        "You are a concise research assistant. Use tools when helpful.",
        "demo-model",
        100_000,
        5,
    )
}

/// The package that backs the hosted adapter (its name → the tool name).
fn demo_package() -> AgentPackage {
    AgentPackage {
        name: "demo-agent".to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You are a concise research assistant.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "demo-model".to_string(),
        }),
        max_tokens: 100_000,
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

// ---- 1. fixed-source offline run -------------------------------------------

#[tokio::test]
async fn fixed_source_runs_offline() {
    // Via the lib-safe seam (the production fixed path).
    let outcome = cargo_pmcp::agent_run::run_fixed_source(demo_config()).await;
    assert!(
        matches!(outcome, RunOutcome::Completed { .. }),
        "fixed source must drive the loop to a terminal Completed outcome, got {outcome:?}"
    );

    // Via the REAL built binary — `agent dev --source fixed` must exit 0 with no
    // network access (the fixed source is scripted end-turn).
    let mut cmd =
        assert_cmd::Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    cmd.args(["agent", "dev", "--source", "fixed"]);
    cmd.assert().success();
}

// ---- 2. sampling-hosted in-process run -------------------------------------

#[tokio::test]
async fn sampling_hosted_run_in_process() {
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
    let agent = AgentServer::builder(
        demo_package(),
        demo_config(),
        factory,
        Arc::new(NoopInvoker::default()),
        Arc::new(InMemoryStore::new()),
    )
    .build()
    .expect("agent server builds");
    let tool_name = agent.tool_name().to_string();

    let (client_t, server_t) = DuplexTransport::pair();
    let server_handle = tokio::spawn(async move {
        let _ = agent.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling_with_tools(HostScript {
            calls: AtomicUsize::new(0),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let task_id = match client
        .call_tool_with_task(tool_name, json!({ "message": "what is pmcp?" }))
        .await
        .expect("call_tool_with_task")
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => panic!("expected a created task"),
    };

    let mut task = client.tasks_get(&task_id).await.expect("tasks/get");
    let mut polls = 0;
    while !task.status.is_terminal() && polls < 50 {
        tokio::time::sleep(Duration::from_millis(2)).await;
        task = client.tasks_get(&task_id).await.expect("tasks/get");
        polls += 1;
    }
    assert!(
        task.status.is_terminal(),
        "hosted task must reach a terminal status within the poll budget (status={})",
        task.status
    );

    let result = client.tasks_result(&task_id).await.expect("tasks/result");
    let text = result
        .content
        .iter()
        .find_map(|c| match c {
            Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    assert!(
        !text.is_empty(),
        "the hosted run must produce a non-empty result"
    );

    drop(client);
    server_handle.abort();
}

// ---- inline test double -----------------------------------------------------

/// A host sampling handler that answers with an immediate `end_turn`, so the
/// hosted loop terminates in one iteration.
struct HostScript {
    calls: AtomicUsize,
}

#[async_trait]
impl HostSamplingHandlerWithTools for HostScript {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResultWithTools> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CreateMessageResultWithTools::new(
            "host-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "Done (hosted): pmcp is a Rust MCP SDK.".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}
