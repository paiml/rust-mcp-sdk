//! AGNT-07 / D-10/11/12: the [`AgentServer`] exposes ONE package-driven,
//! task-supported tool backed by a REAL store lifecycle, driven end-to-end
//! through a live `pmcp::Client`.
//!
//! Asserts:
//! - (a) the single tool's name == the package name, description == the derived
//!   rule, and the input schema carries the optional `run_id`;
//! - (b) a task-augmented `tools/call` mints a store-backed task that reaches a
//!   terminal status and whose `tasks/result` carries the persisted content (a
//!   REAL create → completed lifecycle, not orphan metadata);
//! - (c) two calls with distinct auto-minted run ids run independently;
//! - (d) a call passing an existing `run_id` RESUMES stored history (the mock
//!   source observes the prior turns — D-12 continuity).

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Content, Role};
use pmcp::{Client, ToolCallResponse};

use pmcp_agent::{
    AgentServer, CompletionSource, CompletionSourceFactory, FixedSourceFactory, InMemoryStore,
    ResolvedAgentConfig, ToolCall, ToolCallResult, ToolInvoker,
};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

/// A no-op tool invoker (the end-turn source never dispatches tools).
struct NoopInvoker;

#[async_trait]
impl ToolInvoker for NoopInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::ok(call.id, json!({}))
    }
}

fn test_package() -> AgentPackage {
    AgentPackage {
        name: "echo-agent".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        instructions: "You echo the user politely. Stay brief.".to_string(),
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

fn config() -> ResolvedAgentConfig {
    ResolvedAgentConfig::new("You echo the user politely.", "test-model", 100_000, 5)
}

/// Poll `tasks/get` until the task reaches a terminal status (bounded).
async fn drive_to_terminal<T>(client: &mut Client<T>, task_id: &str)
where
    T: pmcp::shared::Transport + Send + Sync,
{
    let mut polls = 0;
    let mut task = client.tasks_get(task_id).await.expect("tasks/get");
    while !task.status.is_terminal() && polls < 50 {
        tokio::time::sleep(Duration::from_millis(2)).await;
        task = client.tasks_get(task_id).await.expect("tasks/get");
        polls += 1;
    }
    assert!(task.status.is_terminal(), "task must reach terminal status");
    assert_eq!(task.task_id, task_id, "polled id == store-minted id");
}

/// Call the agent tool as a task and return `(run_id_from_result, result)`.
async fn call_agent<T>(
    client: &mut Client<T>,
    tool: &str,
    args: serde_json::Value,
) -> (String, pmcp::types::CallToolResult)
where
    T: pmcp::shared::Transport + Send + Sync,
{
    let task_id = match client
        .call_tool_with_task(tool.to_string(), args)
        .await
        .expect("call_tool_with_task")
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => panic!("expected a created task, got a sync result"),
    };
    drive_to_terminal(client, &task_id).await;
    let result = client.tasks_result(&task_id).await.expect("tasks/result");
    let run_id = result
        .structured_content
        .as_ref()
        .and_then(|v| v.get("runId"))
        .and_then(|v| v.as_str())
        .expect("result carries a runId")
        .to_string();
    (run_id, result)
}

#[tokio::test]
async fn agent_server_real_task_lifecycle_and_resume() {
    let source = Arc::new(EndTurnMock::default());
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(
        source.clone() as Arc<dyn CompletionSource>,
    ));
    let store: Arc<dyn pmcp_agent::ConversationStore> = Arc::new(InMemoryStore::new());

    let agent = AgentServer::builder(
        test_package(),
        config(),
        factory,
        Arc::new(NoopInvoker),
        store,
    )
    .build()
    .expect("agent server builds");

    let tool_name = agent.tool_name().to_string();
    let expected_desc = agent.description().to_string();

    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server_handle = tokio::spawn(async move {
        let _ = agent.run(server_t).await;
    });

    let mut client = Client::new(client_t);
    let init = client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    // (b, prerequisite) the store-backed server auto-advertises `tasks`.
    assert!(
        init.capabilities.tasks.is_some(),
        "task_store server must advertise tasks capability"
    );

    // (a) exactly one tool; name == package name; description == derived rule;
    // schema carries the optional run_id.
    let tools = client.list_tools(None).await.expect("list_tools");
    assert_eq!(tools.tools.len(), 1, "adapter exposes exactly one tool");
    let tool = &tools.tools[0];
    assert_eq!(tool.name, "echo-agent");
    assert_eq!(tool.name, tool_name);
    assert_eq!(tool.description.as_deref(), Some(expected_desc.as_str()));
    assert_eq!(
        expected_desc, "You echo the user politely.",
        "description derived from the first sentence of instructions"
    );
    assert!(
        tool.input_schema["properties"].get("run_id").is_some(),
        "input schema carries the optional run_id"
    );

    // (b) REAL lifecycle: a task-augmented call reaches terminal and the
    // persisted result is non-empty.
    let (run_a, result_a) = call_agent(&mut client, &tool_name, json!({ "message": "hi" })).await;
    assert!(
        !result_a.content.is_empty(),
        "tasks/result carries persisted terminal content"
    );

    // (c) a second auto-minted run is independent (distinct run id).
    let (run_b, _result_b) =
        call_agent(&mut client, &tool_name, json!({ "message": "hello again" })).await;
    assert_ne!(run_a, run_b, "auto-minted run ids are distinct");

    // Before resume, capture how many messages the source saw on a FRESH run.
    let fresh_max = source.max_messages_seen.load(Ordering::SeqCst);

    // (d) resume: pass run_a back as run_id; the stored history (userA +
    // assistant reply) is loaded, so the source sees strictly more messages.
    let (run_c, _result_c) = call_agent(
        &mut client,
        &tool_name,
        json!({ "message": "continue", "run_id": run_a }),
    )
    .await;
    assert_eq!(run_c, run_a, "resume reuses the provided run id");
    let resumed_max = source.max_messages_seen.load(Ordering::SeqCst);
    assert!(
        resumed_max >= 3,
        "resumed run must load prior history (saw {resumed_max} messages, fresh peak {fresh_max})"
    );

    drop(client);
    server_handle.abort();
}

/// The real mock: ends the turn and records the max history length observed.
#[derive(Default)]
struct EndTurnMock {
    max_messages_seen: AtomicUsize,
}

#[async_trait]
impl CompletionSource for EndTurnMock {
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, pmcp_agent::CompletionError> {
        self.max_messages_seen
            .fetch_max(params.messages.len(), Ordering::SeqCst);
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

/// A NON-task-aware client (plain `tools/call`, no `task` field) must receive the
/// agent's ANSWER as normal tool output — NOT the `{taskId,status,ttl,result}`
/// task-creation envelope. Before the `is_task_request()` branch, the SDK
/// text-wrapped the whole envelope (a JSON blob) because its `TaskCreated` path
/// only fires when `req.task.is_some()`. Regression test for that path.
#[tokio::test]
async fn agent_server_non_task_call_returns_answer_not_envelope() {
    let source = Arc::new(EndTurnMock::default());
    let factory: Arc<dyn CompletionSourceFactory> =
        Arc::new(FixedSourceFactory::new(source as Arc<dyn CompletionSource>));
    let store: Arc<dyn pmcp_agent::ConversationStore> = Arc::new(InMemoryStore::new());

    let agent = AgentServer::builder(
        test_package(),
        config(),
        factory,
        Arc::new(NoopInvoker),
        store,
    )
    .build()
    .expect("agent server builds");
    let tool_name = agent.tool_name().to_string();

    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server_handle = tokio::spawn(async move {
        let _ = agent.run(server_t).await;
    });

    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    // Plain `tools/call`, no task augmentation.
    let result = client
        .call_tool(tool_name, json!({ "message": "hi" }))
        .await
        .expect("plain call_tool");

    let text = result
        .content
        .iter()
        .find_map(|c| match c {
            Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .expect("tool output carries text content");

    // The agent's answer ("ok") reaches the client, with the runId for D-12 resume.
    assert!(
        text.contains("ok"),
        "non-task client gets the answer text: {text}"
    );
    assert!(
        text.contains("runId"),
        "answer carries a resumable runId: {text}"
    );
    // ...and it is NOT the task-creation envelope (the pre-fix bug).
    assert!(
        !text.contains("taskId") && !text.contains("ttl"),
        "non-task output must not be the task-creation envelope: {text}"
    );

    drop(client);
    let _ = server_handle.await;
}
