//! Builds the team-mcp [`pmcp::Server`]: one `team_mcp__<member>` tool per
//! roster member, each forwarding a `tools/call` to its member agent under the
//! [`super::guards`] checks, carrying guard state as namespaced `_meta`.
//!
//! # Per-member dynamic tool family
//!
//! [`build_team_mcp_server`] registers one tool per [`MemberHandle`], named
//! `team_mcp__<member>` (derived from the member's [`MemberId`], itself derived
//! from the member's `ComponentRef`). Each tool's handler OVERRIDES
//! [`ToolHandler::handle_output`] to:
//!
//! 1. read the guard state from `extra.request_meta` (109-00);
//! 2. enforce roster membership + depth + self-call + ancestor-cycle guards;
//! 3. build the OUTGOING `_meta` (depth + 1, this target as caller, target
//!    appended to the ancestor chain);
//! 4. dispatch on the member hop and return [`ToolOutput::Result`] carrying the
//!    related-task pointer under
//!    [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY).
//!
//! Returning [`ToolOutput::Result`] BYPASSES response middleware, so the handler
//! owns its own `_meta` hygiene: [`MemberHandle::dispatch`] already strips the
//! member envelope's `_meta` down to related-task only (Pitfall 5 / T-109-05-03).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::types::protocol::RequestMeta;
use pmcp::{Error, RequestHandlerExtra, Result, Server, ToolHandler, ToolInfo, ToolOutput};

use super::guards::{
    guard_ancestor_cycle, guard_depth, guard_self_call, lookup_member, read_guard_state,
    GuardError, GuardState, META_ANCESTORS, META_CALLER, META_DEPTH,
};
use super::identity::MemberId;
use super::member::MemberHandle;

/// The advertised tool name for a member: `team_mcp__<slug(member id)>`.
///
/// The `MemberId` wire form (`name@version`) is slugified (any character outside
/// `[a-z0-9_]` becomes `_`) so the result is a valid, stable tool name.
#[must_use]
pub fn team_tool_name(id: &MemberId) -> String {
    let slug: String = id
        .as_str()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("team_mcp__{slug}")
}

/// Map a guard rejection onto a protocol [`Error`] (never a panic).
fn guard_error(err: GuardError) -> Error {
    match err {
        GuardError::UnknownMember(_) => Error::not_found(err.to_string()),
        _ => Error::validation(err.to_string()),
    }
}

/// Build the OUTGOING guard `_meta` for the member hop: depth + 1, this target
/// as the new caller, and the target appended to the ancestor chain.
fn build_forward_meta(state: &GuardState, target: &MemberId) -> RequestMeta {
    let mut ancestors: Vec<String> = state
        .ancestors
        .iter()
        .map(|m| m.as_str().to_string())
        .collect();
    ancestors.push(target.as_str().to_string());

    RequestMeta::new()
        .with_meta(META_DEPTH, json!(state.depth + 1))
        .with_meta(META_CALLER, json!(target.as_str()))
        .with_meta(META_ANCESTORS, json!(ancestors))
}

/// One `team_mcp__<member>` tool: guards the incoming call, then forwards it to
/// the member hop.
struct MemberDispatchTool {
    target: MemberId,
    tool_name: String,
    description: String,
    handle: Arc<MemberHandle>,
    max_team_depth: i64,
    roster: Arc<Vec<MemberId>>,
}

#[async_trait]
impl ToolHandler for MemberDispatchTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // The dispatch owns a full `CallToolResult` envelope (related-task
        // `_meta`), so it runs through `handle_output` (the Result path). This
        // method is never used and must not silently drop that envelope.
        Err(Error::internal(
            "team_mcp member dispatch uses handle_output (ToolOutput::Result)",
        ))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            self.tool_name.clone(),
            Some(self.description.clone()),
            json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The task/message to forward to the team member."
                    }
                },
                "required": ["message"]
            }),
        ))
    }

    async fn handle_output(&self, args: Value, extra: RequestHandlerExtra) -> Result<ToolOutput> {
        // 1) Guard state from the request's namespaced _meta (untrusted input).
        let state = read_guard_state(&extra).map_err(guard_error)?;

        // 2) Enforce roster membership + all recursion guards (ids, not names).
        lookup_member(&self.target, &self.roster).map_err(guard_error)?;
        guard_depth(state.depth, self.max_team_depth).map_err(guard_error)?;
        if let Some(caller) = &state.caller {
            guard_self_call(&self.target, caller).map_err(guard_error)?;
        }
        guard_ancestor_cycle(&self.target, &state.ancestors).map_err(guard_error)?;

        // 3) Forward the incremented guard _meta + task augmentation on the hop.
        let forward_meta = build_forward_meta(&state, &self.target);
        let result = self.handle.dispatch(args, forward_meta).await?;

        // 4) Verbatim Result: dispatch already stripped member _meta to
        //    related-task only, so this owns a hygienic envelope.
        Ok(ToolOutput::Result(result))
    }
}

/// Build the team-mcp [`Server`] from live member handles.
///
/// Registers one `team_mcp__<member>` tool per handle (a dynamic family keyed by
/// [`MemberId`], derived from the member's `ComponentRef`); `roster` is the set
/// of valid member ids the guards check membership against, and
/// `max_team_depth` bounds recursion.
///
/// # Errors
/// Propagates any [`Server`] construction error, and fails loudly with a
/// validation error if two distinct members slugify to the same
/// `team_mcp__<member>` tool name (which would otherwise silently overwrite one
/// handler in the builder's tool map, dropping a member from dispatch).
pub fn build_team_mcp_server(
    members: Vec<MemberHandle>,
    max_team_depth: i64,
    roster: Vec<MemberId>,
) -> Result<Server> {
    let roster = Arc::new(roster);
    let mut builder = Server::builder()
        .name("team-mcp")
        .version(env!("CARGO_PKG_VERSION"));

    // Detect tool-name collisions at build time: two distinct `MemberId`s can
    // slugify to the same `team_mcp__<member>` name, and the builder's
    // `HashMap::insert` would silently overwrite (dropping a member handler).
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for handle in members {
        let target = handle.id().clone();
        let tool_name = team_tool_name(&target);
        if !seen.insert(tool_name.clone()) {
            return Err(Error::validation(format!(
                "team tool-name collision: '{tool_name}' derived from two distinct members \
                 (member '{target}' slugifies to an already-registered tool name)"
            )));
        }
        let tool: Arc<dyn ToolHandler> = Arc::new(MemberDispatchTool {
            target: target.clone(),
            tool_name: tool_name.clone(),
            description: format!("Dispatch a task to team member {target}."),
            handle: Arc::new(handle),
            max_team_depth,
            roster: roster.clone(),
        });
        builder = builder.tool_arc(&tool_name, tool);
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::resolver::{LocalDirPackageResolver, PackageResolver};
    use crate::team::member::resolve_member_factory;
    use crate::transport::DuplexTransport;

    use async_trait::async_trait;
    use pmcp::types::protocol::RequestMeta;
    use pmcp::types::sampling::{
        CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
    };
    use pmcp::types::tasks::RELATED_TASK_META_KEY;
    use pmcp::types::{ClientCapabilities, Role};
    use pmcp::Client;

    use pmcp_agent::{
        CompletionError, CompletionSource, CompletionSourceFactory, FixedSourceFactory,
        ProgrammaticBuilder, ResolvedAgentConfig,
    };
    use pmcp_package::reference::ComponentType;
    use pmcp_package::slot::SlotType;
    use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot};

    use super::super::identity::MemberTaskForwarding;

    /// A completion source that ends the turn immediately with a fixed answer.
    struct EndTurnMock;

    #[async_trait]
    impl CompletionSource for EndTurnMock {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> std::result::Result<CreateMessageResultWithTools, CompletionError> {
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

    fn member_ref(name: &str) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    fn member_pkg(name: &str) -> AgentPackage {
        AgentPackage {
            name: name.to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You are a helpful team member. Be brief.".to_string(),
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

    /// Build a live member handle for `name` backed by an injected FixedSource.
    async fn live_member(name: &str) -> (MemberId, MemberHandle) {
        let r = member_ref(name);
        let id = MemberId::from_ref(&r);
        let pkg = member_pkg(name);

        // Injected override (D-15): the FixedSource path — no slot resolution.
        let injected: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(
            Arc::new(EndTurnMock) as Arc<dyn CompletionSource>,
        ));
        let resolver = ProgrammaticBuilder::new();
        let factory = resolve_member_factory(&pkg, &resolver, Some(injected))
            .await
            .expect("override factory");

        let config = ResolvedAgentConfig::new("Be a helpful team member.", "test-model", 10_000, 5);
        // Prove the shared build path works even though we could pass the agent
        // directly: reuse spawn_from_package (the same path the binary uses).
        let handle = MemberHandle::spawn_from_package(
            id.clone(),
            pkg,
            config,
            factory,
            MemberTaskForwarding::Synthesize,
        )
        .await
        .expect("member spawns");
        (id, handle)
    }

    async fn connect(server: Server) -> (Client<DuplexTransport>, tokio::task::JoinHandle<()>) {
        let (client_t, server_t) = DuplexTransport::pair();
        let handle = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });
        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");
        (client, handle)
    }

    #[tokio::test]
    async fn dispatch_surfaces_related_task_under_the_correct_key() {
        let (id, handle) = live_member("triage").await;
        let tool_name = team_tool_name(&id);
        let server = build_team_mcp_server(vec![handle], 3, vec![id]).unwrap();
        let (client, server_task) = connect(server).await;

        // Root call (absent depth => depth 0).
        let result = client
            .call_tool(tool_name, json!({ "message": "hi" }))
            .await
            .expect("dispatch ok");

        // Top-level _meta[RELATED_TASK_META_KEY].taskId is present + non-null.
        let meta = result._meta.as_ref().expect("result carries _meta");
        let related = meta
            .get(RELATED_TASK_META_KEY)
            .expect("related-task under the correct key");
        let task_id = related.get("taskId").and_then(Value::as_str);
        assert!(
            task_id.is_some_and(|s| !s.is_empty()),
            "related-task taskId present + non-null: {related:?}"
        );
        // And no bare `related_task` key leaked.
        assert!(meta.get("related_task").is_none());

        server_task.abort();
    }

    async fn error_on_meta(
        depth_meta: RequestMeta,
        tool: &str,
        id: MemberId,
        handle: MemberHandle,
    ) {
        let server = build_team_mcp_server(vec![handle], 3, vec![id]).unwrap();
        let (client, server_task) = connect(server).await;
        let res = client
            .call_tool_with_meta(tool.to_string(), json!({ "message": "x" }), depth_meta)
            .await;
        assert!(res.is_err(), "guarded dispatch must error");
        server_task.abort();
    }

    #[tokio::test]
    async fn malformed_depth_is_rejected() {
        let (id, handle) = live_member("triage").await;
        let tool = team_tool_name(&id);
        let meta = RequestMeta::new().with_meta(META_DEPTH, json!("not-an-integer"));
        error_on_meta(meta, &tool, id, handle).await;
    }

    #[tokio::test]
    async fn excessive_depth_is_rejected() {
        let (id, handle) = live_member("triage").await;
        let tool = team_tool_name(&id);
        // max_team_depth is 3; depth 4 exceeds it.
        let meta = RequestMeta::new().with_meta(META_DEPTH, json!(4));
        error_on_meta(meta, &tool, id, handle).await;
    }

    #[tokio::test]
    async fn self_call_is_rejected() {
        let (id, handle) = live_member("triage").await;
        let tool = team_tool_name(&id);
        // Caller id == target id -> self-call.
        let meta = RequestMeta::new()
            .with_meta(META_DEPTH, json!(1))
            .with_meta(META_CALLER, json!(id.as_str()));
        error_on_meta(meta, &tool, id, handle).await;
    }

    #[tokio::test]
    async fn ancestor_cycle_is_rejected() {
        let (id, handle) = live_member("triage").await;
        let tool = team_tool_name(&id);
        // Target already in the ancestor chain -> cycle.
        let meta = RequestMeta::new()
            .with_meta(META_DEPTH, json!(1))
            .with_meta(META_ANCESTORS, json!([id.as_str()]));
        error_on_meta(meta, &tool, id, handle).await;
    }

    #[tokio::test]
    async fn unknown_member_tool_is_not_advertised() {
        let (id, handle) = live_member("triage").await;
        let server = build_team_mcp_server(vec![handle], 3, vec![id]).unwrap();
        let (client, server_task) = connect(server).await;
        // A member that was never registered -> pmcp "tool not found", never panic.
        let res = client
            .call_tool(
                "team_mcp__ghost_9_9_9".to_string(),
                json!({ "message": "x" }),
            )
            .await;
        assert!(res.is_err(), "unknown member tool must error");
        server_task.abort();
    }

    #[tokio::test]
    async fn member_identity_is_the_component_ref() {
        // Distinct ComponentRefs => distinct MemberIds => distinct tool names.
        let a = MemberId::from_ref(&member_ref("triage"));
        let b = MemberId::from_ref(&member_ref("formatter"));
        assert_ne!(a, b);
        assert_ne!(team_tool_name(&a), team_tool_name(&b));
        // "triage@^1" slugifies '@' and '^' to '_' each.
        assert_eq!(team_tool_name(&a), "team_mcp__triage__1");
    }

    #[tokio::test]
    async fn colliding_member_tool_names_fail_the_build() {
        // Two distinct members whose ids slugify to the SAME tool name:
        // `triage-1@^1` and `triage_1@^1` both -> `team_mcp__triage_1__1`.
        let (id_a, handle_a) = live_member("triage-1").await;
        let (id_b, handle_b) = live_member("triage_1").await;
        // Distinct identities, colliding derived tool names.
        assert_ne!(id_a, id_b);
        assert_eq!(team_tool_name(&id_a), team_tool_name(&id_b));

        let err = build_team_mcp_server(vec![handle_a, handle_b], 3, vec![id_a, id_b])
            .expect_err("colliding members must fail the build, not silently drop a handler");
        let msg = err.to_string();
        assert!(
            msg.contains("collision"),
            "expected a tool-name collision build error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn local_dir_resolver_round_trips_a_member_package() {
        // The binary's ComponentRef -> AgentPackage seam.
        let dir = tempfile::tempdir().unwrap();
        let pkg = member_pkg("triage");
        std::fs::write(
            dir.path().join("triage.json"),
            serde_json::to_vec(&pkg).unwrap(),
        )
        .unwrap();
        let resolver = LocalDirPackageResolver::new(dir.path());
        let loaded = resolver.resolve_agent(&member_ref("triage")).await.unwrap();
        assert_eq!(loaded, pkg);
    }
}
