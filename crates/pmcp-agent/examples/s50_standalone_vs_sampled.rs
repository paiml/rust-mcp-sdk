//! # s50: one agent, two completion sources — standalone vs hosted-sampled
//!
//! Demonstrates the core `pmcp-agent` promise (AGNT-04/05/06): the SAME decision
//! loop (`AgentEngine` over the three seams, driven by the same
//! `ResolvedAgentConfig`) runs against DIFFERENT `CompletionSource`s with zero
//! loop changes.
//!
//! It runs two ways, both NETWORK-FREE and under DEFAULT features:
//!
//! 1. **Standalone** — the engine runs directly over a MOCK `CompletionSource`
//!    (a scripted `tool_use` then `end_turn`), dispatching a tool through the
//!    invoker seam. No server, no client.
//! 2. **Hosted-sampled** — the same agent is exposed via the `AgentServer`
//!    adapter and sampled through a real `pmcp::Client` (`on_sampling_with_tools`)
//!    over an in-process transport: the tool handler builds a request-scoped
//!    `SamplingSource` from `extra.peer()` and runs the identical loop.
//!
//! Two OPT-IN, feature-gated extensions are included at COMPILE level so the same
//! engine visibly satisfies the HTTP sources too:
//!
//! - `--features anthropic` constructs an `AnthropicSource` and binds it as
//!   `&dyn CompletionSource` (no live call).
//! - `--features openai-compat` enables a live-Ollama standalone path, gated
//!   behind the `PMCP_AGENT_LIVE_OLLAMA` env var (never runs by default).
//!
//! Run: `cargo run -p pmcp-agent --example s50_standalone_vs_sampled`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use pmcp::client::host::HostSamplingHandlerWithTools;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Role};
use pmcp::{ClientBuilder, Error, ToolCallResponse};
use tokio::sync::mpsc;

use pmcp_agent::{
    AgentEngine, AgentServer, CompletionError, CompletionSource, CompletionSourceFactory,
    InMemoryStore, ResolvedAgentConfig, RunOutcome, SamplingSourceFactory, ToolCall,
    ToolCallResult, ToolInvoker,
};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

/// Shared config for the agent, used by BOTH run styles.
fn agent_config() -> ResolvedAgentConfig {
    ResolvedAgentConfig::new(
        "You are a concise research assistant. Use tools when helpful.",
        "demo-model",
        100_000,
        5,
    )
}

/// The package that backs the hosted adapter (its name → the tool name).
fn agent_package() -> AgentPackage {
    AgentPackage {
        name: "research-agent".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
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

/// A tool invoker that records dispatches and echoes an ok result.
#[derive(Clone, Default)]
struct DemoInvoker {
    dispatched: Arc<AtomicUsize>,
}

#[async_trait]
impl ToolInvoker for DemoInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        self.dispatched.fetch_add(1, Ordering::SeqCst);
        ToolCallResult::ok(call.id, json!({ "result": format!("ran {}", call.name) }))
    }
}

/// A scripted MOCK completion source: `tool_use` first, then `end_turn`.
#[derive(Default)]
struct ScriptedSource {
    calls: AtomicUsize,
}

#[async_trait]
impl CompletionSource for ScriptedSource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(CreateMessageResultWithTools::new(
                "demo-model",
                Role::Assistant,
                vec![SamplingMessageContent::ToolUse {
                    name: "search".to_string(),
                    id: "call-1".to_string(),
                    input: json!({ "q": "pmcp" }),
                    meta: None,
                }],
            )
            .with_stop_reason("tool_use"))
        } else {
            Ok(CreateMessageResultWithTools::new(
                "demo-model",
                Role::Assistant,
                vec![SamplingMessageContent::Text {
                    text: "Done: pmcp is a Rust MCP SDK.".to_string(),
                    meta: None,
                }],
            )
            .with_stop_reason("end_turn"))
        }
    }
}

/// The host sampling handler used by the HOSTED path: same tool_use→end_turn
/// script, but answered over the real client sampling surface.
struct HostScript {
    calls: AtomicUsize,
}

#[async_trait]
impl HostSamplingHandlerWithTools for HostScript {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResultWithTools> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(CreateMessageResultWithTools::new(
                "host-model",
                Role::Assistant,
                vec![SamplingMessageContent::ToolUse {
                    name: "search".to_string(),
                    id: "h-1".to_string(),
                    input: json!({ "q": "pmcp" }),
                    meta: None,
                }],
            )
            .with_stop_reason("tool_use"))
        } else {
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
}

fn outcome_tag(outcome: &RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Completed { .. } => "Completed",
        RunOutcome::LimitReached => "LimitReached",
        RunOutcome::RetryRequired { .. } => "RetryRequired",
        RunOutcome::Failed { .. } => "Failed",
        _ => "Unknown",
    }
}

/// STANDALONE: run the engine directly over the mock source.
async fn run_standalone() {
    println!("== 1. STANDALONE (mock CompletionSource) ==");
    let source = ScriptedSource::default();
    let invoker = DemoInvoker::default();
    let engine = AgentEngine::new(
        source,
        invoker.clone(),
        InMemoryStore::new(),
        agent_config(),
    );
    let outcome = engine.run("standalone-run").await;
    println!(
        "   outcome = {}, tools dispatched = {}",
        outcome_tag(&outcome),
        invoker.dispatched.load(Ordering::SeqCst)
    );
}

/// HOSTED: expose the same agent via AgentServer and sample it through a client.
async fn run_hosted() {
    println!("== 2. HOSTED-SAMPLED (AgentServer + SamplingSource) ==");
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
    let agent = AgentServer::builder(
        agent_package(),
        agent_config(),
        factory,
        Arc::new(DemoInvoker::default()),
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
    let result = client.tasks_result(&task_id).await.expect("tasks/result");
    let text = result
        .content
        .iter()
        .find_map(|c| match c {
            pmcp::types::Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    println!(
        "   terminal task status = {}, result = {}",
        task.status, text
    );

    drop(client);
    server_handle.abort();
}

/// COMPILE-LEVEL: the SAME seam accepts the Anthropic HTTP source (no live call).
#[cfg(feature = "anthropic")]
fn assert_anthropic_satisfies_the_seam() {
    use pmcp_agent::sources::{AnthropicSource, SecretString};
    let src = AnthropicSource::new(
        "https://api.anthropic.com",
        "claude-3-5-sonnet",
        SecretString::new("sk-ant-not-used"),
    )
    .expect("construct AnthropicSource");
    let _seam: &dyn CompletionSource = &src;
    println!("   [anthropic] AnthropicSource satisfies CompletionSource (compile-level).");
}

/// OPT-IN live Ollama standalone path (never runs by default).
#[cfg(feature = "openai-compat")]
async fn run_live_ollama_if_requested() {
    if std::env::var("PMCP_AGENT_LIVE_OLLAMA").is_err() {
        return;
    }
    use pmcp_agent::sources::{OpenAiCompatSource, SecretString};
    println!("== (opt-in) LIVE OLLAMA standalone ==");
    let source = OpenAiCompatSource::new(
        "http://localhost:11434/v1",
        "llama3.2",
        SecretString::new("ollama"),
    )
    .expect("construct OpenAiCompatSource");
    let engine = AgentEngine::new(
        source,
        DemoInvoker::default(),
        InMemoryStore::new(),
        agent_config(),
    );
    let outcome = engine.run("live-ollama-run").await;
    println!("   live outcome = {}", outcome_tag(&outcome));
}

#[tokio::main]
async fn main() {
    println!("pmcp-agent s50: one loop, two sources\n");

    run_standalone().await;
    run_hosted().await;

    #[cfg(feature = "anthropic")]
    assert_anthropic_satisfies_the_seam();

    #[cfg(feature = "openai-compat")]
    run_live_ollama_if_requested().await;

    println!("\nDone — the same AgentEngine ran standalone and hosted-sampled.");
}

// ---- in-process duplex transport (self-contained, mirrors s45) ---------------

/// One half of an in-process duplex transport (client <-> server).
#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> pmcp::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}
