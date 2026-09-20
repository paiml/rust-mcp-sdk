//! [`AgentServer`] — expose a package-driven agent as an MCP server (AGNT-07).
//!
//! NATIVE-ONLY (`cfg(not(target_arch = "wasm32"))`): the task lifecycle is
//! store-backed and the sampling path rides `pmcp::PeerHandle`, both of which are
//! native-only in `pmcp`. The wasm32 CI gate (D-13) proves the LOOP + SEAMS +
//! config path is target-clean, not this adapter.
//!
//! # What it builds
//!
//! An `AgentServer` wraps a single-tool [`pmcp::Server`]:
//!
//! - **One package-driven tool (D-11).** Its NAME is the [`AgentPackage`] name;
//!   its DESCRIPTION is derived by [`derive_tool_description`] (there is no
//!   `description` field on `AgentPackage`); its INPUT SCHEMA is the package's
//!   `input_schema` (or a default single-`message` object) EXTENDED with an
//!   optional `run_id`/`conversation_id` string for resume (D-12).
//! - **A REAL store-backed task lifecycle (D-10).** The tool is registered
//!   `with_task_support(TaskSupport::Required)` and the server is wired with a
//!   `task_store`, so a task-augmented `tools/call` mints a store task, runs the
//!   agent to completion synchronously, and persists the terminal result — a
//!   genuine create → working → completed lifecycle observable via `tasks/get`
//!   and `tasks/result`, NOT orphan `related_task` metadata.
//! - **A per-request completion source.** The handler asks a
//!   [`CompletionSourceFactory`] for THIS request's source (a request-scoped
//!   `SamplingSource` built from `extra.peer()`, or a fixed HTTP source).
//!
//! The adapter holds NO per-conversation mutable state: every call mints (or
//! resumes, via `run_id`) an independent run whose continuity lives entirely in
//! the shared [`ConversationStore`] (D-12).
//!
//! # Hosting
//!
//! [`AgentServer::run`] delegates to [`pmcp::Server::run`], which wires the
//! server-side peer used by [`SamplingSourceFactory`](super::SamplingSourceFactory)
//! AND serves the store-backed `tasks/*` endpoints — so a single served instance
//! supports both the task lifecycle and hosted sampling.

use std::sync::Arc;

use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::Transport;
use pmcp::types::sampling::{SamplingMessage, SamplingMessageContent};
use pmcp::types::{CallToolResult, Content, Role, TaskSupport, ToolExecution};
use pmcp::{RequestHandlerExtra, Server};

use pmcp_package::AgentPackage;
use serde_json::{json, Value};

use super::factory::CompletionSourceFactory;
use crate::config::ResolvedAgentConfig;
use crate::iteration::{AgentEngine, RunOutcome};
use crate::seams::{ConversationStore, RunPhase, ToolInvoker};

/// Derive the agent tool's description from an [`AgentPackage`].
///
/// `AgentPackage` carries NO `description` field (only `name` +
/// `instructions`), so the description is DERIVED by a documented rule:
///
/// 1. the first sentence of `instructions` (text up to the first `.` or
///    newline), trimmed and terminated with a period; else
/// 2. when `instructions` is empty, the fallback `"Run the {name} agent."`.
///
/// Keeping the rule in one function makes the derivation testable and stable.
#[must_use]
pub fn derive_tool_description(pkg: &AgentPackage) -> String {
    let trimmed = pkg.instructions.trim();
    if trimmed.is_empty() {
        return format!("Run the {} agent.", pkg.name);
    }
    let first = trimmed.split(['.', '\n']).next().unwrap_or(trimmed).trim();
    if first.is_empty() {
        format!("Run the {} agent.", pkg.name)
    } else {
        format!("{first}.")
    }
}

/// Build the tool input schema: the package schema (or a default single-`message`
/// object) EXTENDED with optional `run_id`/`conversation_id` string properties
/// for resume (D-12). Neither resume key is `required`.
fn build_input_schema(config: &ResolvedAgentConfig) -> Value {
    let mut schema = config.input_schema.clone().unwrap_or_else(|| {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The user message to send to the agent."
                }
            },
            "required": ["message"]
        })
    });

    // Inject the optional resume keys into `properties` without disturbing any
    // existing `required` list (resume is always optional).
    if let Some(obj) = schema.as_object_mut() {
        let props = obj.entry("properties").or_insert_with(|| json!({}));
        if let Some(props_obj) = props.as_object_mut() {
            props_obj.entry("run_id").or_insert_with(|| {
                json!({
                    "type": "string",
                    "description": "Optional prior run id to resume a stored conversation (D-12)."
                })
            });
            props_obj.entry("conversation_id").or_insert_with(|| {
                json!({
                    "type": "string",
                    "description": "Alias of run_id: resume a stored conversation."
                })
            });
        }
    }
    schema
}

/// An agent exposed as a single-tool MCP server (AGNT-07).
///
/// Construct via [`AgentServer::builder`], then host with [`AgentServer::run`].
pub struct AgentServer {
    server: Server,
    tool_name: String,
    description: String,
    input_schema: Value,
}

impl AgentServer {
    /// Start building an `AgentServer` from a package, its resolved config, and
    /// the three request-time collaborators.
    #[must_use]
    pub fn builder(
        package: AgentPackage,
        config: ResolvedAgentConfig,
        factory: Arc<dyn CompletionSourceFactory>,
        invoker: Arc<dyn ToolInvoker>,
        store: Arc<dyn ConversationStore>,
    ) -> AgentServerBuilder {
        AgentServerBuilder {
            package,
            config,
            factory,
            invoker,
            store,
            name: None,
            version: None,
        }
    }

    /// The single tool's name (equal to the package name).
    #[must_use]
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// The single tool's derived description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The single tool's input schema (with the optional resume keys).
    #[must_use]
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    /// Host the agent over `transport`, wiring the peer (for sampling) and the
    /// store-backed `tasks/*` endpoints via [`pmcp::Server::run`].
    ///
    /// # Errors
    ///
    /// Propagates any transport/serving error from [`pmcp::Server::run`].
    pub async fn run<T: Transport + 'static>(self, transport: T) -> pmcp::Result<()> {
        self.server.run(transport).await
    }
}

/// Builder for [`AgentServer`].
pub struct AgentServerBuilder {
    package: AgentPackage,
    config: ResolvedAgentConfig,
    factory: Arc<dyn CompletionSourceFactory>,
    invoker: Arc<dyn ToolInvoker>,
    store: Arc<dyn ConversationStore>,
    name: Option<String>,
    version: Option<String>,
}

impl AgentServerBuilder {
    /// Override the advertised server name (defaults to the package name).
    #[must_use]
    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Override the advertised server version (defaults to the package version).
    #[must_use]
    pub fn server_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Build the single-tool [`AgentServer`].
    ///
    /// # Errors
    ///
    /// Returns any error from [`pmcp::Server`] construction (e.g. an invalid
    /// tasks-capability configuration).
    pub fn build(self) -> pmcp::Result<AgentServer> {
        let tool_name = self.package.name.clone();
        let description = derive_tool_description(&self.package);
        let input_schema = build_input_schema(&self.config);

        let server_name = self.name.unwrap_or_else(|| self.package.name.clone());
        let server_version = self
            .version
            .unwrap_or_else(|| self.package.version.to_string());

        // Shared, request-time collaborators captured by the tool handler. The
        // completion source is built PER request from the factory; the invoker
        // and store are shared (the store carries run continuity, D-12).
        let factory = self.factory;
        let invoker = self.invoker;
        let store = self.store;
        let config = self.config;

        let handler = move |args: Value, extra: RequestHandlerExtra| {
            let factory = factory.clone();
            let invoker = invoker.clone();
            let store = store.clone();
            let config = config.clone();
            Box::pin(
                async move { run_agent_tool(&args, &extra, factory, invoker, store, config).await },
            )
                as std::pin::Pin<Box<dyn std::future::Future<Output = pmcp::Result<Value>> + Send>>
        };

        let tool = TypedTool::new_with_schema(tool_name.clone(), input_schema.clone(), handler)
            .with_description(description.clone())
            .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
        let server = Server::builder()
            .name(server_name)
            .version(server_version)
            .tool(tool_name.clone(), tool)
            .task_store(task_store)
            .build()?;

        Ok(AgentServer {
            server,
            tool_name,
            description,
            input_schema,
        })
    }
}

/// The agent tool handler: parse input, resume/mint a run, drive one full
/// [`AgentEngine`] run, and return a TASK-SHAPED value the SDK persists as the
/// store task's terminal result.
async fn run_agent_tool(
    args: &Value,
    extra: &RequestHandlerExtra,
    factory: Arc<dyn CompletionSourceFactory>,
    invoker: Arc<dyn ToolInvoker>,
    store: Arc<dyn ConversationStore>,
    config: ResolvedAgentConfig,
) -> pmcp::Result<Value> {
    let message = args
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| pmcp::Error::validation("missing required 'message' string"))?;

    // Resume an existing run (run_id/conversation_id) or mint a fresh,
    // collision-safe one. Minting a uuid is an EFFECT at the adapter boundary —
    // never inside the pure decision core.
    let run_id = args
        .get("run_id")
        .or_else(|| args.get("conversation_id"))
        .and_then(Value::as_str)
        .map_or_else(
            || format!("run-{}", uuid::Uuid::new_v4()),
            ToString::to_string,
        );

    // Seed the run: load prior history (resume) or start fresh, append the new
    // user turn, and reset the phase so the engine begins a fresh completion.
    let mut state = store
        .load(&run_id)
        .await
        .map_err(|e| pmcp::Error::internal(format!("conversation store load failed: {e}")))?
        .unwrap_or_default();
    state.history.push(SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: message.to_string(),
            meta: None,
        },
    ));
    state.phase = RunPhase::ReadyForCompletion;
    state.pending_tool_calls.clear();
    store
        .save(&run_id, &state)
        .await
        .map_err(|e| pmcp::Error::internal(format!("conversation store save failed: {e}")))?;

    // Build the per-request completion source and drive one full run. The engine
    // loads the state we just seeded (including the resumed history).
    let source = factory.create(extra);
    let engine = AgentEngine::new(source, invoker, store, config);
    let outcome = engine.run(&run_id).await;

    let call_result = outcome_to_result(&run_id, outcome);

    // A non-task-aware client (no `task` field in the request) must receive the
    // agent's ANSWER as normal tool output. Returning the task-shaped envelope
    // below would make the SDK text-wrap `{taskId,status,ttl,result}` as the
    // whole tool output (verified: `ServerCore::on_tool_call` only takes the
    // TaskCreated path when `req.task.is_some()`), so the client would get a
    // JSON blob instead of the answer. Return the structured answer directly;
    // the `runId` rides along for D-12 resume.
    if !extra.is_task_request() {
        return Ok(call_result
            .structured_content
            .clone()
            .unwrap_or_else(|| json!({ "runId": run_id })));
    }

    let result_value = serde_json::to_value(&call_result)
        .map_err(|e| pmcp::Error::internal(format!("serialize agent result: {e}")))?;

    // TASK-SHAPED return (taskId + status + nested result): the SDK's
    // store-backed create path mints its own task id, persists this `result` as
    // the terminal CallToolResult, and completes the task — a REAL lifecycle.
    Ok(json!({
        "taskId": run_id,
        "status": "completed",
        "ttl": 60_000,
        "result": result_value,
    }))
}

/// Map a terminal [`RunOutcome`] into the tool's [`CallToolResult`].
///
/// The `run_id` is echoed in `structuredContent` so a client can pass it back as
/// `run_id` to resume the stored conversation (D-12).
fn outcome_to_result(run_id: &str, outcome: RunOutcome) -> CallToolResult {
    match outcome {
        RunOutcome::Completed { result } => {
            let text = render_turn_text(&result.assistant_message);
            CallToolResult::structured_with_text(json!({ "runId": run_id, "text": text }), text)
        },
        RunOutcome::LimitReached => {
            error_result(run_id, "agent stopped: iteration/token limit reached")
        },
        RunOutcome::RetryRequired { class } => {
            error_result(run_id, &format!("agent paused: retry required ({class:?})"))
        },
        RunOutcome::Failed { error } => error_result(run_id, &format!("agent failed: {error}")),
    }
}

/// Build an error [`CallToolResult`] that still carries the resumable `run_id`.
///
/// The message is mirrored into `structuredContent.text` so a non-task-aware
/// client (which receives the structured content directly, not the task
/// envelope) still sees why the run stopped.
fn error_result(run_id: &str, message: &str) -> CallToolResult {
    let mut result = CallToolResult::error(vec![Content::text(message.to_string())]);
    result.structured_content = Some(json!({ "runId": run_id, "text": message }));
    result
}

/// Concatenate the text blocks of a turn into a single string (`tool_use` and
/// other non-textual blocks are skipped).
fn render_turn_text(turn: &crate::iteration::TurnMessage) -> String {
    let mut parts = Vec::new();
    for block in &turn.content {
        if let SamplingMessageContent::Text { text, .. } = block {
            parts.push(text.as_str());
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{build_input_schema, derive_tool_description};
    use crate::config::ResolvedAgentConfig;
    use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

    fn pkg(name: &str, instructions: &str) -> AgentPackage {
        AgentPackage {
            name: name.to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: instructions.to_string(),
            llm: ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "test-model".to_string(),
            }),
            max_tokens: 4096,
            max_iterations: 10,
            connectors: vec![],
            tool_selection: None,
            input_schema: None,
            output_schema: None,
            importance: None,
            finalizer_role: None,
            budget_defaults: vec![],
        }
    }

    #[test]
    fn description_uses_first_sentence_of_instructions() {
        let p = pkg("triage", "You triage support tickets. Be terse.");
        assert_eq!(derive_tool_description(&p), "You triage support tickets.");
    }

    #[test]
    fn description_falls_back_when_instructions_empty() {
        let p = pkg("triage", "   ");
        assert_eq!(derive_tool_description(&p), "Run the triage agent.");
    }

    #[test]
    fn input_schema_injects_optional_resume_keys() {
        let config = ResolvedAgentConfig::new("be helpful", "test-model", 100, 4);
        let schema = build_input_schema(&config);
        let props = &schema["properties"];
        assert!(props.get("message").is_some(), "default message property");
        assert!(props.get("run_id").is_some(), "optional run_id injected");
        assert!(
            props.get("conversation_id").is_some(),
            "optional conversation_id injected"
        );
        // Resume keys are NOT required.
        let required = schema["required"].as_array().cloned().unwrap_or_default();
        assert!(!required.iter().any(|v| v == "run_id"));
    }
}
