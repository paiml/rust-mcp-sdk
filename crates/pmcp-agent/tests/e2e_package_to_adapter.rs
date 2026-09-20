//! AGNT-09 end-to-end composition: `AgentPackage` → `resolve_agent` →
//! `ResolvedAgentConfig` → (mock `CompletionSource` + `ClientToolInvoker` over a
//! mock connector + `AgentEngine`) → `AgentServer` → one live `tools/call`
//! reaching a terminal, store-backed task result.
//!
//! This is the single "package → resolved runtime → engine/source/invoker →
//! adapter round-trip" acceptance test: it wires every seam the phase built and
//! drives it through a real `pmcp::Client`.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, Role};
use pmcp::{Client, ToolCallResponse, WaitForTaskOptions};

use pmcp_agent::{
    resolve_agent, AgentServer, ClientToolInvoker, CompletionError, CompletionSource,
    CompletionSourceFactory, ConnectorClient, ConversationStore, FixedSourceFactory, InMemoryStore,
    InvokerError, ProgrammaticBuilder, ToolInvoker,
};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

/// A completion source that ends the turn immediately (no tool calls).
struct EndTurnSource;

#[async_trait]
impl CompletionSource for EndTurnSource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "resolved-and-composed".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

/// A minimal mock connector for the `ClientToolInvoker` — part of the
/// composition even though the end-turn source never dispatches a tool.
struct MockConnector;

#[async_trait]
impl ConnectorClient for MockConnector {
    async fn call_tool(
        &self,
        _name: &str,
        _arguments: serde_json::Value,
    ) -> Result<CallToolResult, InvokerError> {
        Ok(CallToolResult::new(vec![Content::text("ok")]))
    }

    async fn wait_for_related_task(
        &self,
        _meta: &TaskMetadata,
        _opts: WaitForTaskOptions,
    ) -> Result<CallToolResult, InvokerError> {
        Ok(CallToolResult::new(vec![Content::text("done")]))
    }
}

/// A package with one behavior slot and no connectors (endpoints resolve empty).
fn sample_package() -> AgentPackage {
    AgentPackage {
        name: "compose-agent".to_string(),
        version: semver::Version::parse("2.0.0").unwrap(),
        instructions: "You compose the whole pipeline end to end.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "test-model".to_string(),
        }),
        max_tokens: 2048,
        max_iterations: 4,
        connectors: vec![],
        tool_selection: Some(json!({ "search": ["query"] })),
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

#[tokio::test]
async fn package_resolves_and_composes_through_the_adapter() {
    // 1) package → resolve_agent → ResolvedAgentConfig (AGNT-09 resolve step).
    let pkg = sample_package();
    let resolver = ProgrammaticBuilder::new().with_value("primary-llm", "test-model");
    let config = resolve_agent(&pkg, &resolver)
        .await
        .expect("resolve_agent composes the package");
    assert_eq!(config.model, "test-model");
    assert_eq!(config.tools, vec!["query".to_string()]);
    assert_eq!(config.max_iterations, 4);

    // 2) compose the runtime seams: mock source + ClientToolInvoker(mock) + store.
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(Arc::new(
        EndTurnSource,
    )
        as Arc<dyn CompletionSource>));
    let invoker: Arc<dyn ToolInvoker> =
        Arc::new(ClientToolInvoker::new(Arc::new(MockConnector), 5));
    let store: Arc<dyn ConversationStore> = Arc::new(InMemoryStore::new());

    // 3) expose via the AgentServer adapter.
    let agent = AgentServer::builder(pkg, config, factory, invoker, store)
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

    // 4) drive ONE tool call to a terminal, store-backed task result.
    let task_id = match client
        .call_tool_with_task(tool_name, json!({ "message": "go" }))
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
        "composed run reaches terminal task"
    );

    let result = client.tasks_result(&task_id).await.expect("tasks/result");
    assert!(
        !result.content.is_empty(),
        "the composed adapter yields a persisted terminal result"
    );

    drop(client);
    server_handle.abort();
}
