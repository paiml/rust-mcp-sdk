//! The team-mcp member hop: a per-member [`pmcp::Client`] over an in-process
//! [`DuplexTransport`](crate::transport::DuplexTransport) to a Phase 108
//! [`AgentServer`], plus the explicit task-forwarding contract and the shared
//! member-LLM resolver.
//!
//! # The member hop (D-13)
//!
//! Each roster member is a real `AgentServer` reached over a `DuplexTransport`
//! pair via its own `pmcp::Client` (initialized once). A dispatch forwards the
//! caller's arguments AND the incremented guard `_meta` in a single
//! [`Client::call_tool_with_task_and_meta`] (109-00), so task augmentation and
//! guard state travel together.
//!
//! # Explicit task-forwarding contract (Pitfall 5)
//!
//! A member's tool is built [`with_task_support(TaskSupport::Required)`](pmcp),
//! so a task-augmented call returns a [`ToolCallResponse::Task`]. Under the
//! default [`MemberTaskForwarding::Synthesize`] contract, team-mcp polls that
//! task to terminal ([`Client::wait_for_task`]) and SYNTHESIZES a synchronous
//! [`CallToolResult`] whose `_meta` carries ONLY the related-task pointer under
//! [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY). A
//! [`ToolCallResponse::Result`] is re-emitted with the member's `_meta` stripped
//! down to the related-task key only (a tight re-emit — the member envelope's
//! other `_meta` is never echoed unsanitized; T-109-05-03).
//!
//! # Member LLM resolution (D-15)
//!
//! [`resolve_member_factory`] returns an EXPLICITLY INJECTED factory override
//! when one is supplied (tests/CI/`FixedSource`); otherwise it resolves the
//! member's MANDATORY `AgentPackage.llm` `ConfigSlot` via a
//! [`SlotResolver`](pmcp_agent::SlotResolver) into a concrete
//! [`CompletionSourceFactory`]. There is no "no llm slot" fallback — the slot is
//! mandatory on the package.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::WaitForTaskOptions;
use pmcp::types::protocol::RequestMeta;
use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, ClientCapabilities};
use pmcp::{Client, ToolCallResponse};

use pmcp_agent::{
    AgentServer, CompletionSourceFactory, ConversationStore, InMemoryStore, ResolvedAgentConfig,
    SlotResolver, ToolCall, ToolCallResult, ToolInvoker,
};
use pmcp_package::AgentPackage;

use crate::transport::DuplexTransport;

use super::identity::{MemberId, MemberTaskForwarding};

/// The conventional dev endpoint for a local, OpenAI-compatible LLM (Ollama).
///
/// Used when a member's llm slot has no explicitly-resolved endpoint. Loopback,
/// so it passes the source's default (loopback-only) scheme policy.
#[cfg(feature = "member-llm")]
const DEFAULT_OPENAI_COMPAT_ENDPOINT: &str = "http://localhost:11434/v1";

/// The env var carrying an optional member-LLM API key (dev). Read into a
/// redacted secret; never logged, never an argv/CLI flag.
#[cfg(feature = "member-llm")]
const MEMBER_LLM_KEY_ENV: &str = "PMCP_TEAM_MEMBER_LLM_KEY";

/// An error building or dispatching a team-mcp member.
#[derive(Debug, thiserror::Error)]
pub enum MemberError {
    /// The member's mandatory llm slot could not be resolved to a usable value.
    #[error("resolving member llm slot failed: {0}")]
    Resolve(String),
    /// The resolved member LLM source could not be constructed.
    #[error("member llm source unavailable: {0}")]
    Source(String),
}

/// A no-op [`ToolInvoker`] — the reference members drive an end-turn source and
/// never dispatch downstream tools.
struct NoopInvoker;

#[async_trait]
impl ToolInvoker for NoopInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::ok(call.id, json!({}))
    }
}

/// A live handle to one team member: its identity, its own tool name, an
/// initialized [`Client`] over the in-process transport, and the forwarding
/// contract.
pub struct MemberHandle {
    id: MemberId,
    /// The member's OWN tool name (the `AgentServer`'s single tool == package
    /// name) — the name dispatched on the member hop (NOT the advertised
    /// `team_mcp__<member>` name).
    member_tool: String,
    client: Arc<Client<DuplexTransport>>,
    forwarding: MemberTaskForwarding,
    // Keep the member server task alive for the life of the handle.
    _server: Arc<tokio::task::JoinHandle<()>>,
}

impl MemberHandle {
    /// This member's identity (derived from its `ComponentRef`).
    #[must_use]
    pub fn id(&self) -> &MemberId {
        &self.id
    }

    /// Spawn a member `AgentServer` over a fresh [`DuplexTransport`] pair and
    /// return a handle whose [`Client`] has completed `initialize`.
    ///
    /// # Errors
    /// Propagates any `initialize` transport/protocol error.
    pub async fn spawn(
        id: MemberId,
        agent: AgentServer,
        forwarding: MemberTaskForwarding,
    ) -> pmcp::Result<Self> {
        let member_tool = agent.tool_name().to_string();
        let (client_t, server_t) = DuplexTransport::pair();
        let server = tokio::spawn(async move {
            let _ = agent.run(server_t).await;
        });
        let mut client = Client::new(client_t);
        client.initialize(ClientCapabilities::default()).await?;
        Ok(Self {
            id,
            member_tool,
            client: Arc::new(client),
            forwarding,
            _server: Arc::new(server),
        })
    }

    /// Build a member `AgentServer` from its package + resolved config +
    /// completion factory, then spawn it (the SAME member-wiring path the dev
    /// binary and `TeamRuntime` use).
    ///
    /// # Errors
    /// Propagates `AgentServer` build errors and `initialize` errors.
    pub async fn spawn_from_package(
        id: MemberId,
        agent_pkg: AgentPackage,
        config: ResolvedAgentConfig,
        factory: Arc<dyn CompletionSourceFactory>,
        forwarding: MemberTaskForwarding,
    ) -> pmcp::Result<Self> {
        let store: Arc<dyn ConversationStore> = Arc::new(InMemoryStore::new());
        let agent = AgentServer::builder(agent_pkg, config, factory, Arc::new(NoopInvoker), store)
            .build()?;
        Self::spawn(id, agent, forwarding).await
    }

    /// Dispatch a `tools/call` to this member, forwarding the incremented guard
    /// `_meta` AND task augmentation, then apply the explicit task-forwarding
    /// contract. The returned [`CallToolResult`] always carries related-task
    /// under [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY)
    /// (and NOTHING else in `_meta`).
    ///
    /// # Errors
    /// Propagates the member hop's transport/protocol errors and any
    /// `wait_for_task` polling error.
    pub async fn dispatch(
        &self,
        args: Value,
        forward_meta: RequestMeta,
    ) -> pmcp::Result<CallToolResult> {
        let resp = self
            .client
            .call_tool_with_task_and_meta(self.member_tool.clone(), args, forward_meta)
            .await?;
        match resp {
            // Re-emit path: strip the member envelope's _meta to related-task only.
            ToolCallResponse::Result(result) => Ok(reemit_related_only(result)),
            // Task path: honor the forwarding contract.
            ToolCallResponse::Task(task) => match self.forwarding {
                MemberTaskForwarding::Synthesize => {
                    let terminal = self
                        .client
                        .wait_for_task(&task.task_id, WaitForTaskOptions::default())
                        .await?;
                    Ok(synthesize_from_task(&task.task_id, terminal))
                },
                MemberTaskForwarding::ReturnEnvelope => Ok(related_only(&task.task_id)),
            },
        }
    }
}

/// Tight re-emit: keep the member's content/structured output but replace its
/// `_meta` with ONLY the related-task pointer (never echo the rest — Pitfall 5).
fn reemit_related_only(result: CallToolResult) -> CallToolResult {
    let related = result.related_task();
    let mut out = CallToolResult::new(result.content);
    out.is_error = result.is_error;
    out.structured_content = result.structured_content;
    // `out._meta` is already `None` from `::new`.
    match related {
        Some(meta) => out.with_related_task(meta),
        None => out,
    }
}

/// Synthesize a synchronous result from a polled-terminal member task: the
/// member content + ONLY a related-task pointer to the member task.
fn synthesize_from_task(member_task_id: &str, terminal: CallToolResult) -> CallToolResult {
    let mut out = CallToolResult::new(terminal.content);
    out.is_error = terminal.is_error;
    out.structured_content = terminal.structured_content;
    out.with_related_task(TaskMetadata::new(member_task_id))
}

/// A result carrying only the related-task pointer (the `ReturnEnvelope`
/// contract: hand the caller the member task id without polling it).
fn related_only(member_task_id: &str) -> CallToolResult {
    CallToolResult::new(vec![]).with_related_task(TaskMetadata::new(member_task_id))
}

/// Resolve the [`CompletionSourceFactory`] for a member (D-15).
///
/// - `override_factory = Some(f)` → return `f` verbatim (the explicit injection
///   for tests/CI/`FixedSource`). This is NOT a "no-slot" fallback — it is a
///   first-class dependency-injection seam.
/// - otherwise → resolve the member's MANDATORY `agent_pkg.llm` `ConfigSlot` via
///   `resolver` and construct the concrete factory (an OpenAI-compatible /
///   Ollama source; the resolved endpoint + a redacted key). Requires the
///   `member-llm` feature; without it the concrete branch errors, directing the
///   caller to inject an override or enable the feature.
///
/// # Errors
/// [`MemberError::Resolve`] when the slot resolves to a secret or fails to
/// resolve; [`MemberError::Source`] when the concrete source cannot be built (or
/// the `member-llm` feature is not compiled).
pub async fn resolve_member_factory(
    agent_pkg: &AgentPackage,
    resolver: &dyn SlotResolver,
    override_factory: Option<Arc<dyn CompletionSourceFactory>>,
) -> Result<Arc<dyn CompletionSourceFactory>, MemberError> {
    if let Some(factory) = override_factory {
        return Ok(factory);
    }

    // Mandatory llm slot → concrete model/provider string (behavior-relevant, so
    // it resolves to Plain; a Secret here is a misconfiguration).
    let model = match resolver
        .resolve_slot(&agent_pkg.llm)
        .await
        .map_err(|e| MemberError::Resolve(e.to_string()))?
    {
        pmcp_agent::ResolvedValue::Plain(value) => value,
        pmcp_agent::ResolvedValue::Secret(_) => {
            return Err(MemberError::Resolve(
                "llm slot resolved to a secret value (expected a model id)".to_string(),
            ))
        },
    };

    build_llm_factory(&agent_pkg.llm, &model, resolver).await
}

/// Construct the concrete OpenAI-compatible member LLM factory (feature-gated).
#[cfg(feature = "member-llm")]
async fn build_llm_factory(
    llm: &pmcp_package::ConfigSlot,
    model: &str,
    resolver: &dyn SlotResolver,
) -> Result<Arc<dyn CompletionSourceFactory>, MemberError> {
    use pmcp_agent::sources::{OpenAiCompatSource, SecretString};
    use pmcp_agent::{CompletionSource, FixedSourceFactory};

    // Endpoint: the slot's resolved endpoint, else the local Ollama default.
    let (_, slot_name) = llm.slot.key();
    let base_url = resolver
        .resolve_endpoint(slot_name)
        .await
        .unwrap_or_else(|_| DEFAULT_OPENAI_COMPAT_ENDPOINT.to_string());

    // Optional API key: env-only (never argv), wrapped so it never logs.
    let key = SecretString::new(std::env::var(MEMBER_LLM_KEY_ENV).unwrap_or_default());

    let source = OpenAiCompatSource::new(base_url, model, key)
        .map_err(|e| MemberError::Source(e.to_string()))?;
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(Arc::new(
        source,
    )
        as Arc<dyn CompletionSource>));
    Ok(factory)
}

/// Feature-absent stub: without `member-llm` there is no concrete HTTP source, so
/// callers must inject a factory override (tests/CI) or build with `member-llm`.
#[cfg(not(feature = "member-llm"))]
#[allow(clippy::unused_async)]
async fn build_llm_factory(
    _llm: &pmcp_package::ConfigSlot,
    _model: &str,
    _resolver: &dyn SlotResolver,
) -> Result<Arc<dyn CompletionSourceFactory>, MemberError> {
    Err(MemberError::Source(
        "built without the `member-llm` feature: inject a factory override or \
         rebuild with --features member-llm"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};
    use pmcp::types::Role;
    use pmcp_agent::{CompletionError, CompletionSource, FixedSourceFactory, ProgrammaticBuilder};
    use pmcp_package::slot::SlotType;
    use pmcp_package::ConfigSlot;

    struct DummySource;

    #[async_trait]
    impl CompletionSource for DummySource {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools, CompletionError> {
            Ok(CreateMessageResultWithTools::new(
                "dummy",
                Role::Assistant,
                vec![],
            ))
        }
    }

    fn pkg() -> AgentPackage {
        AgentPackage {
            name: "triage".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You triage.".to_string(),
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

    /// The injected override is returned verbatim (same Arc), never re-resolved.
    #[tokio::test]
    async fn override_factory_is_returned_as_is() {
        let injected: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(
            Arc::new(DummySource) as Arc<dyn CompletionSource>,
        ));
        let resolver = ProgrammaticBuilder::new();
        let got = resolve_member_factory(&pkg(), &resolver, Some(injected.clone()))
            .await
            .expect("override returned");
        assert!(Arc::ptr_eq(&injected, &got), "override returned verbatim");
    }

    /// With no override, the mandatory llm slot is resolved and a concrete
    /// factory constructed (only meaningful when `member-llm` is compiled).
    #[cfg(feature = "member-llm")]
    #[tokio::test]
    async fn slot_resolved_factory_is_constructed() {
        // A stub resolver that resolves the llm model + a loopback endpoint.
        let resolver = ProgrammaticBuilder::new()
            .with_value("primary-llm", "gpt-4o-mini")
            .with_endpoint("primary-llm", "http://localhost:1234/v1");
        let got = resolve_member_factory(&pkg(), &resolver, None).await;
        // `Arc<dyn CompletionSourceFactory>` is not Debug, so report the error side only.
        assert!(
            got.is_ok(),
            "slot-resolved factory constructed: {:?}",
            got.err()
        );
    }
}
