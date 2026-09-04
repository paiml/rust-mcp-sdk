//! `PromptHandler` implementation for `SequentialWorkflow` with server-side execution
//!
//! Enables workflows to be registered as prompts in the server with full tool execution.
//!
//! # MCP-Compliant Workflow Execution
//!
//! This module implements server-side workflow execution during `prompts/get`. When a user
//! invokes a workflow prompt, the server:
//!
//! 1. Creates a user intent message from the workflow description and arguments
//! 2. Creates an assistant plan message listing all workflow steps
//! 3. Executes each step sequentially:
//!    - Announces the tool call (assistant message)
//!    - Executes the tool server-side
//!    - Returns the tool result (user message)
//!    - Stores the result in execution context (bindings)
//! 4. Returns the complete conversation trace to the client
//!
//! This approach provides:
//! - Complete execution context for the LLM
//! - Efficient single-round-trip execution
//! - Clear error handling and debugging
//! - Data flow via bindings between steps

use super::{
    conversion::ToolInfo, data_source::DataSource, newtypes::BindingName,
    sequential::SequentialWorkflow, workflow_step::WorkflowStep,
};
use crate::error::Result;
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::middleware_executor::MiddlewareExecutor;
use crate::server::{PromptHandler, ResourceHandler, ToolHandler};
#[cfg(test)]
use crate::types::Role;
use crate::types::{Content, GetPromptResult, PromptArgument, PromptInfo, PromptMessage};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Stores step execution results (bindings) during workflow execution
#[derive(Debug)]
pub(crate) struct ExecutionContext {
    bindings: HashMap<BindingName, Value>,
}

impl ExecutionContext {
    pub(crate) fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub(crate) fn store_binding(&mut self, name: BindingName, value: Value) {
        self.bindings.insert(name, value);
    }

    pub(crate) fn get_binding(&self, name: &BindingName) -> Option<&Value> {
        self.bindings.get(name)
    }
}

/// `PromptHandler` implementation for `SequentialWorkflow`
///
/// Executes workflow steps server-side during `prompts/get` and returns a conversation trace
/// showing the complete execution flow.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::Server;
/// use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
///
/// let workflow = SequentialWorkflow::new(
///     "add_task",
///     "Add a task to a project"
/// )
/// .argument("project", "Project name", true)
/// .argument("task", "Task description", true)
/// .step(
///     WorkflowStep::new("list_pages", ToolHandle::new("list_pages"))
///         .bind("pages")
/// );
///
/// let server = Server::builder()
///     .name("server")
///     .version("1.0.0")
///     .tool("list_pages", /* tool handler */)
///     .prompt_workflow(workflow)?
///     .build()?;
/// ```
pub struct WorkflowPromptHandler {
    /// The workflow definition
    workflow: SequentialWorkflow,
    /// Tool registry for handle expansion and metadata
    tools: HashMap<Arc<str>, ToolInfo>,
    /// Middleware executor for tool execution with middleware chain (preferred)
    middleware_executor: Option<Arc<dyn MiddlewareExecutor>>,
    /// Tool handlers for direct execution (fallback, for testing)
    tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>>,
    /// Resource handler for fetching resource content
    resource_handler: Option<Arc<dyn ResourceHandler>>,
    /// The D-04a projected-skill prepend, as an ALREADY-RENDERED body.
    ///
    /// `None` (the default) means the opt-in is off and the transcript is
    /// byte-identical to what it has always been. `Some(body)` holds the exact
    /// string that the body of [`SequentialWorkflow::as_skill`] held at the moment
    /// [`Self::with_projected_skill_prepend`] ran — it is a `String`, never a
    /// `bool` consulted per request, for three measured reasons:
    ///
    /// 1. The D-15 slug-fallback `tracing::warn!` inside the projection would
    ///    otherwise re-fire on **every `prompts/get`**, turning a build-time
    ///    diagnostic into a per-request log flood.
    /// 2. Message `[0]` would be re-derived from this handler's own clone of the
    ///    workflow, while the digest published in `skills/list` is a snapshot
    ///    taken at registry-build time. Two independently-derived strings that
    ///    are *supposed* to be identical is exactly the drift this projection
    ///    exists to prevent; caching makes them one string by construction.
    /// 3. Rendering is not free and [`Self::handle`] is a hot path.
    ///
    /// [`SequentialWorkflow::as_skill`]: super::sequential::SequentialWorkflow::as_skill
    #[cfg(feature = "skills")]
    projected_skill_prepend: Option<String>,
}

impl std::fmt::Debug for WorkflowPromptHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowPromptHandler")
            .field("workflow", &self.workflow.name())
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("middleware_executor", &self.middleware_executor.is_some())
            .field(
                "tool_handlers",
                &self.tool_handlers.keys().collect::<Vec<_>>(),
            )
            .field("resource_handler", &self.resource_handler.is_some())
            .finish()
    }
}

impl WorkflowPromptHandler {
    /// Create a new workflow prompt handler with tool handlers (for testing/legacy)
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow definition (should be validated before passing)
    /// * `tools` - Tool registry for metadata
    /// * `tool_handlers` - Actual tool handlers for execution (bypasses middleware)
    /// * `resource_handler` - Resource handler for fetching resource content
    pub fn new(
        workflow: SequentialWorkflow,
        tools: HashMap<Arc<str>, ToolInfo>,
        tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>>,
        resource_handler: Option<Arc<dyn ResourceHandler>>,
    ) -> Self {
        Self {
            workflow,
            tools,
            middleware_executor: None,
            tool_handlers,
            resource_handler,
            #[cfg(feature = "skills")]
            projected_skill_prepend: None,
        }
    }

    /// Create a new workflow prompt handler with middleware executor (production)
    ///
    /// This constructor uses the middleware executor, ensuring that all middleware
    /// (OAuth, logging, authorization, etc.) is applied to tool executions within workflows.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow definition (should be validated before passing)
    /// * `tools` - Tool registry for metadata
    /// * `middleware_executor` - Middleware executor for tool execution with full middleware chain
    /// * `resource_handler` - Resource handler for fetching resource content
    pub fn with_middleware_executor(
        workflow: SequentialWorkflow,
        tools: HashMap<Arc<str>, ToolInfo>,
        middleware_executor: Arc<dyn MiddlewareExecutor>,
        resource_handler: Option<Arc<dyn ResourceHandler>>,
    ) -> Self {
        Self {
            workflow,
            tools,
            middleware_executor: Some(middleware_executor),
            tool_handlers: HashMap::new(),
            resource_handler,
            #[cfg(feature = "skills")]
            projected_skill_prepend: None,
        }
    }

    /// Opt in to the D-04a projected-skill prepend (default: **off**).
    ///
    /// With the flag ON, the transcript [`Self::handle`] returns is:
    ///
    /// - `[0]` — the projected skill body, byte-equal to the body of the skill
    ///   [`SequentialWorkflow::as_skill`] returns and therefore byte-equal to the
    ///   bytes served at `skill://{slug}/SKILL.md` (D-05: one string, one digest,
    ///   no variant to keep in sync).
    /// - `[1]` — the user-intent message, **kept**. Its `Parameters:` block is
    ///   the only place the caller's actual argument VALUES appear; the projected
    ///   body carries argument SPECS. Suppressing it would remove information
    ///   from the transcript entirely.
    /// - `[2..]` — guidance, resource, tool-call-announcement and tool-result
    ///   messages, unchanged.
    ///
    /// The assistant-plan MESSAGE is suppressed, because the projected body's
    /// `## Procedure` section already carries its step-and-tool list. Only the
    /// message is dropped: [`Self::validate_tool_registry`] runs in its place, so
    /// the "tool not found in registry" validation raises the same error on the
    /// same step in both flag states — see [`Self::opening_messages`].
    ///
    /// With the flag OFF — the default — the transcript is byte-identical to
    /// what this handler has always produced, message for message and role for
    /// role.
    ///
    /// # Rendering happens HERE, exactly once
    ///
    /// The body is rendered by this call and stored. `prompts/get` never
    /// re-renders it. A caller that mutates the workflow value afterwards will
    /// therefore NOT see the change reflected in message `[0]`; re-run this
    /// setter to re-render.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use std::collections::HashMap;
    /// use pmcp::server::workflow::{SequentialWorkflow, WorkflowPromptHandler};
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund");
    /// let body = workflow.as_skill().body().to_string();
    ///
    /// let handler = WorkflowPromptHandler::new(
    ///     workflow,
    ///     HashMap::new(),
    ///     HashMap::new(),
    ///     None,
    /// )
    /// .with_projected_skill_prepend(true);
    ///
    /// assert!(body.starts_with("---\nname: \"refund-flow\"\n"));
    /// # let _ = handler;
    /// # }
    /// ```
    ///
    /// [`SequentialWorkflow::as_skill`]: super::sequential::SequentialWorkflow::as_skill
    #[cfg(feature = "skills")]
    #[must_use]
    pub fn with_projected_skill_prepend(mut self, on: bool) -> Self {
        self.projected_skill_prepend = if on {
            Some(self.workflow.as_skill().body().to_string())
        } else {
            None
        };
        self
    }

    /// The single producer of the D-04a prepended message.
    ///
    /// Returns `None` when the opt-in is off. Otherwise clones the STORED body
    /// into a user-role [`PromptMessage`]; it performs no rendering. Both
    /// [`Self::handle`] and
    /// [`TaskWorkflowPromptHandler`](super::TaskWorkflowPromptHandler)'s
    /// independent message-building branch call this one method, which is what
    /// makes it impossible for the two handlers to produce a different message
    /// `[0]` for the same workflow.
    #[cfg(feature = "skills")]
    pub(crate) fn projected_prepend(&self) -> Option<PromptMessage> {
        self.projected_skill_prepend
            .as_ref()
            .map(|body| PromptMessage::user(Content::text(body.clone())))
    }

    /// Skills-off twin of [`Self::projected_prepend`].
    ///
    /// Why it exists: with this always-`None` counterpart, neither `handle` nor
    /// the task handler's branch needs a `#[cfg]` around its statement sequence,
    /// so the flag-off code path is literally the same code in both feature
    /// configurations. That is what D-04's unchanged-execution property asks
    /// for, and a `#[cfg]`'d statement block would only approximate it.
    #[cfg(not(feature = "skills"))]
    #[allow(clippy::unused_self)]
    pub(crate) fn projected_prepend(&self) -> Option<PromptMessage> {
        None
    }

    /// Substitute argument values into template text
    ///
    /// Replaces `{arg_name}` patterns with actual argument values.
    ///
    /// # Example
    /// ```ignore
    /// let template = "Find page matching '{project}' in the list";
    /// let mut args = HashMap::new();
    /// args.insert("project".to_string(), "MCP Tester".to_string());
    /// let result = substitute_arguments(template, &args);
    /// // result: "Find page matching 'MCP Tester' in the list"
    /// ```
    pub(crate) fn substitute_arguments(template: &str, args: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in args {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }

    /// Resolve template bindings from execution context
    ///
    /// Takes template variables and their `DataSource` definitions,
    /// resolves them to actual values from the execution context.
    fn resolve_template_bindings(
        bindings: &HashMap<String, DataSource>,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, String>> {
        let mut resolved = HashMap::new();

        for (var_name, data_source) in bindings {
            let value = Self::resolve_data_source_to_string(data_source, args, ctx)?;
            resolved.insert(var_name.clone(), value);
        }

        Ok(resolved)
    }

    /// Resolve a `DataSource` to a string value
    ///
    /// Handles all `DataSource` variants and converts them to strings suitable
    /// for template interpolation.
    fn resolve_data_source_to_string(
        source: &DataSource,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        match source {
            DataSource::PromptArg(arg_name) => {
                args.get(arg_name.as_str()).cloned().ok_or_else(|| {
                    crate::Error::validation(format!("Missing prompt argument: {}", arg_name))
                })
            },

            DataSource::StepOutput { step, field } => {
                let step_result = ctx.get_binding(step).ok_or_else(|| {
                    crate::Error::validation(format!("Step binding not found: {}", step))
                })?;

                if let Some(field_name) = field {
                    // Extract field from step result
                    Self::extract_field_as_string(step_result, field_name)
                } else {
                    // Use entire step result
                    Ok(Self::value_to_string(step_result))
                }
            },

            DataSource::Constant(value) => Ok(Self::value_to_string(value)),
        }
    }

    /// Get the type name of a JSON value for error messages
    fn value_type_name(v: &Value) -> &'static str {
        match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    /// Navigate a JSON value using dot-notation path with array indexing support.
    ///
    /// Supports:
    /// - Object fields: `"user.name"`
    /// - Array indices: `"games.0.id"`
    /// - Nested paths: `"data.items.0.field.1.value"`
    ///
    /// Precedence: Object fields take priority over array indices for ambiguous
    /// numeric keys like `{"0": "value"}`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let data = json!({"users": [{"name": "Alice"}, {"name": "Bob"}]});
    /// let result = navigate_json_path(&data, "users.1.name")?;
    /// assert_eq!(result, &json!("Bob"));
    /// ```
    fn navigate_json_path<'a>(value: &'a Value, field_path: &str) -> Result<&'a Value> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = value;

        for (idx, part) in parts.iter().enumerate() {
            let path_so_far = parts[..=idx].join(".");

            current = if let Some(val) = current.get(part) {
                // Object field access (handles {"0": "value"})
                val
            } else if let Ok(index) = part.parse::<usize>() {
                // Array index access
                current
                    .as_array()
                    .ok_or_else(|| {
                        crate::Error::validation(format!(
                            "Cannot index into non-array at '{}' in path '{}'. Found: {}",
                            part,
                            field_path,
                            Self::value_type_name(current)
                        ))
                    })?
                    .get(index)
                    .ok_or_else(|| {
                        let len = current.as_array().unwrap().len();
                        crate::Error::validation(format!(
                            "Array index {} out of bounds at '{}' in path '{}'. Array has {} element{}",
                            index,
                            path_so_far,
                            field_path,
                            len,
                            if len == 1 { "" } else { "s" }
                        ))
                    })?
            } else {
                return Err(crate::Error::validation(format!(
                    "Field '{}' not found at '{}' in path '{}'",
                    part, path_so_far, field_path
                )));
            };
        }

        Ok(current)
    }

    /// Extract a field from a JSON value and convert to string
    ///
    /// Supports dot notation for nested fields: "user.profile.id"
    /// Supports array indexing: "items.0.name", "data.users.1.email"
    fn extract_field_as_string(value: &Value, field_path: &str) -> Result<String> {
        let extracted = Self::navigate_json_path(value, field_path)?;
        Ok(Self::value_to_string(extracted))
    }

    /// Convert a JSON value to a string representation
    ///
    /// Handles different JSON types appropriately for template substitution.
    fn value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            // For arrays and objects, serialize as JSON
            _ => serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value)),
        }
    }

    /// Check if template bindings reference any step outputs
    ///
    /// Returns true if any binding uses `DataSource::StepOutput`, meaning the
    /// resource must be fetched AFTER tool execution.
    pub(crate) fn template_bindings_use_step_outputs(
        bindings: &HashMap<String, DataSource>,
    ) -> bool {
        bindings
            .values()
            .any(|source| matches!(source, DataSource::StepOutput { .. }))
    }

    /// Fetch and embed resources for a workflow step
    ///
    /// Resolves template bindings, interpolates URIs, fetches resource content,
    /// and adds resource messages to the message list.
    ///
    /// Returns `Ok(())` if all resources fetched successfully, or `Err` if any fetch failed.
    pub(crate) async fn fetch_step_resources(
        &self,
        step: &WorkflowStep,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
        extra: &RequestHandlerExtra,
        messages: &mut Vec<PromptMessage>,
    ) -> Result<()> {
        // Resolve template bindings for resource URI interpolation
        let template_vars = if !step.template_bindings().is_empty() {
            Self::resolve_template_bindings(step.template_bindings(), args, ctx)?
        } else {
            HashMap::new()
        };

        for resource_handle in step.resources() {
            let uri = resource_handle.uri();

            // Apply template substitution if needed
            let interpolated_uri = if !template_vars.is_empty() {
                Self::substitute_arguments(uri, &template_vars)
            } else {
                uri.to_string()
            };

            match self.fetch_resource_content(&interpolated_uri, extra).await {
                Ok(content) => {
                    // Embed resource content as user message
                    messages.push(PromptMessage::user(Content::text(format!(
                        "Resource content from {}:\n{}",
                        interpolated_uri, content
                    ))));
                },
                Err(e) => {
                    // Resource fetch failed - add error message
                    messages.push(PromptMessage::user(Content::text(format!(
                        "Error fetching resource {}: {}",
                        interpolated_uri, e
                    ))));
                    return Err(e);
                },
            }
        }

        Ok(())
    }

    /// Create user intent message from workflow description and arguments
    pub(crate) fn create_user_intent(&self, args: &HashMap<String, String>) -> PromptMessage {
        let description = self.workflow.description();

        // Format arguments nicely
        let args_display = if args.is_empty() {
            String::new()
        } else {
            format!(
                "\nParameters:\n{}",
                args.iter()
                    .map(|(k, v)| format!("  - {}: \"{}\"", k, v))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        PromptMessage::user(Content::text(format!(
            "I want to {}.{}",
            description, args_display
        )))
    }

    /// Create assistant plan message listing all workflow steps
    pub(crate) fn create_assistant_plan(&self) -> Result<PromptMessage> {
        let mut plan = String::from("Here's my plan:\n");

        for (idx, step) in self.workflow.steps().iter().enumerate() {
            if let Some(tool_handle) = step.tool() {
                // Tool execution step
                let tool_info = self.tools.get(tool_handle.name()).ok_or_else(|| {
                    crate::Error::Internal(format!(
                        "Tool '{}' not found in registry",
                        tool_handle.name()
                    ))
                })?;

                plan.push_str(&format!(
                    "{}. {} - {}\n",
                    idx + 1,
                    tool_handle.name(),
                    tool_info.description
                ));
            } else {
                // Resource-only step
                let resource_count = step.resources().len();
                plan.push_str(&format!(
                    "{}. {} - Fetch {} resource{}\n",
                    idx + 1,
                    step.name(),
                    resource_count,
                    if resource_count == 1 { "" } else { "s" }
                ));
            }
        }

        Ok(PromptMessage::assistant(Content::text(plan)))
    }

    /// The tool-registry validation half of [`Self::create_assistant_plan`],
    /// without the rendering.
    ///
    /// `create_assistant_plan` does two things: it raises
    /// `Error::Internal("Tool '{}' not found in registry")` for the first step
    /// whose tool is absent, and it renders the plan text. When the projected
    /// skill body already carries the step-and-tool list the rendered message is
    /// dropped — but the validation must still run, because flag-off callers have
    /// that error today. This is that validation on its own.
    ///
    /// The error value, its message and the step it fires on are identical to
    /// [`Self::create_assistant_plan`]'s: same iteration order, same lookup, same
    /// `Error::Internal` construction.
    fn validate_tool_registry(&self) -> Result<()> {
        for step in self.workflow.steps() {
            if let Some(tool_handle) = step.tool() {
                self.tools.get(tool_handle.name()).ok_or_else(|| {
                    crate::Error::Internal(format!(
                        "Tool '{}' not found in registry",
                        tool_handle.name()
                    ))
                })?;
            }
        }
        Ok(())
    }

    /// The header messages every workflow transcript opens with.
    ///
    /// THE single source of the opening sequence — the D-04a projected prepend,
    /// the user-intent message, and the assistant plan unless the prepend already
    /// carries it. [`Self::handle`] and
    /// [`TaskWorkflowPromptHandler`](super::TaskWorkflowPromptHandler)'s
    /// independent message-building branch both push exactly this vector, which
    /// is what makes it impossible for the two handlers to diverge anywhere in
    /// the header, not just at message `[0]`.
    ///
    /// # Why the plan is suppressed rather than skipped
    ///
    /// When the prepend is present the plan MESSAGE is dropped, but the
    /// tool-registry validation it carries is not: `validate_tool_registry()`
    /// runs in its place and its `?` propagates identically. Guarding the whole
    /// `create_assistant_plan()` call instead would silently delete a validation
    /// that flag-off callers have today. Splitting it this way also stops the
    /// plan text — one `format!` temporary per step into a growing `String`, plus
    /// a `PromptMessage` — from being rendered and immediately dropped on a hot
    /// path.
    ///
    /// The prepend is inserted FIRST because the five later early-return /
    /// `break` exits in `handle` each build a `GetPromptResult` from whatever
    /// `messages` holds; prepending here is what makes the projected body message
    /// `[0]` on every one of those paths.
    pub(crate) fn opening_messages(
        &self,
        args: &HashMap<String, String>,
    ) -> Result<Vec<PromptMessage>> {
        let prepend = self.projected_prepend();
        let suppress_plan = prepend.is_some();

        let mut messages = Vec::with_capacity(3);
        messages.extend(prepend);
        messages.push(self.create_user_intent(args));
        if suppress_plan {
            self.validate_tool_registry()?;
        } else {
            messages.push(self.create_assistant_plan()?);
        }
        Ok(messages)
    }

    /// Create assistant message announcing the tool call with resolved parameters
    pub(crate) fn create_tool_call_announcement(
        &self,
        step: &WorkflowStep,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<PromptMessage> {
        let tool_handle = step.tool().ok_or_else(|| {
            crate::Error::Internal(format!(
                "Cannot create tool call announcement for resource-only step '{}'",
                step.name()
            ))
        })?;

        let params = self.resolve_tool_parameters(step, args, ctx)?;

        Ok(PromptMessage::assistant(Content::text(format!(
            "Calling tool '{}' with parameters:\n{}",
            tool_handle.name(),
            serde_json::to_string_pretty(&params).unwrap_or_else(|_| format!("{:?}", params))
        ))))
    }

    /// Fetch a resource and extract its text content
    ///
    /// Returns the text content from the resource, or an error if fetching fails
    /// or the resource doesn't contain text content.
    async fn fetch_resource_content(
        &self,
        uri: &str,
        extra: &RequestHandlerExtra,
    ) -> Result<String> {
        // Check if resource handler is available
        let handler = self.resource_handler.as_ref().ok_or_else(|| {
            crate::Error::validation(
                "No resource handler configured - cannot fetch resources in workflows".to_string(),
            )
        })?;

        // Fetch the resource
        let result = handler.read(uri, extra.clone()).await?;

        // Extract text content from the result
        let mut text_content = String::new();
        for content in result.contents {
            match content {
                Content::Text { text } => {
                    if !text_content.is_empty() {
                        text_content.push('\n');
                    }
                    text_content.push_str(&text);
                },
                Content::Resource { uri, text, .. } => {
                    // Add newline before content
                    if !text_content.is_empty() {
                        text_content.push('\n');
                    }
                    // If resource has embedded text, use it; otherwise include the URI reference
                    if let Some(text) = text {
                        text_content.push_str(&text);
                    } else {
                        text_content.push_str(&format!("[Resource: {}]", uri));
                    }
                },
                Content::Image { .. } | Content::Audio { .. } | Content::ResourceLink { .. } => {
                    // Skip non-text content - we only embed text
                },
            }
        }

        if text_content.is_empty() {
            return Err(crate::Error::validation(format!(
                "Resource {} contains no text content",
                uri
            )));
        }

        Ok(text_content)
    }

    /// Check if resolved parameters satisfy the tool's input schema
    ///
    /// Returns a list of missing required field names. An empty vec means all
    /// required fields are present. This collects ALL missing fields rather
    /// than short-circuiting on the first one, giving callers complete
    /// diagnostic information.
    pub(crate) fn params_satisfy_tool_schema(
        &self,
        step: &WorkflowStep,
        params: &Value,
    ) -> Result<Vec<String>> {
        let tool_handle = step.tool().ok_or_else(|| {
            crate::Error::Internal(format!(
                "Cannot check schema for resource-only step '{}'",
                step.name()
            ))
        })?;

        // Get the tool info (includes schema)
        let tool_info = self.tools.get(tool_handle.name()).ok_or_else(|| {
            crate::Error::Internal(format!(
                "Tool '{}' not found in registry",
                tool_handle.name()
            ))
        })?;

        let mut missing_fields = Vec::new();

        // Check if params object has all required fields from schema
        if let Some(schema_obj) = tool_info.input_schema.as_object() {
            if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
                if let Some(params_obj) = params.as_object() {
                    // Check each required field
                    for req_field in required {
                        if let Some(field_name) = req_field.as_str() {
                            if !params_obj.contains_key(field_name) {
                                missing_fields.push(field_name.to_string());
                            }
                        }
                    }
                } else if !required.is_empty() {
                    // Params is not an object but schema requires fields -- all are missing
                    for req_field in required {
                        if let Some(field_name) = req_field.as_str() {
                            missing_fields.push(field_name.to_string());
                        }
                    }
                }
            }
        }

        Ok(missing_fields)
    }

    /// Execute a workflow step by calling the actual tool handler
    ///
    /// If a middleware executor is available, routes through it to ensure consistent
    /// middleware application (OAuth, logging, etc.). Otherwise, calls tool handler directly.
    pub(crate) async fn execute_tool_step(
        &self,
        step: &WorkflowStep,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
        extra: &RequestHandlerExtra,
    ) -> Result<Value> {
        let tool_handle = step.tool().ok_or_else(|| {
            crate::Error::Internal(format!(
                "Cannot execute tool for resource-only step '{}'",
                step.name()
            ))
        })?;

        // Resolve parameters using bindings and arguments
        let params = self.resolve_tool_parameters(step, args, ctx)?;

        // Debug: Check auth_context before passing to middleware executor
        tracing::debug!(
            "WorkflowPromptHandler.execute_tool_step() - Before clone: auth_context present: {}, has_token: {}",
            extra.auth_context.is_some(),
            extra.auth_context.as_ref().and_then(|ctx| ctx.token.as_ref()).is_some()
        );

        // Execute through middleware executor if available (production mode)
        if let Some(middleware_executor) = &self.middleware_executor {
            let cloned_extra = extra.clone();

            // Debug: Check auth_context after clone
            tracing::debug!(
                "WorkflowPromptHandler.execute_tool_step() - After clone: auth_context present: {}, has_token: {}",
                cloned_extra.auth_context.is_some(),
                cloned_extra.auth_context.as_ref().and_then(|ctx| ctx.token.as_ref()).is_some()
            );

            return middleware_executor
                .execute_tool_with_middleware(tool_handle.name(), params, cloned_extra)
                .await;
        }

        // Fallback: Direct tool handler execution (testing/legacy mode)
        let handler = self.tool_handlers.get(tool_handle.name()).ok_or_else(|| {
            crate::Error::Internal(format!("Tool handler '{}' not found", tool_handle.name()))
        })?;

        handler.handle(params, extra.clone()).await
    }

    /// Resolve tool parameters from `DataSources` (prompt args, bindings, constants)
    pub(crate) fn resolve_tool_parameters(
        &self,
        step: &WorkflowStep,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        let mut params = serde_json::Map::new();

        for (arg_name, data_source) in step.arguments() {
            let value = match data_source {
                DataSource::PromptArg(arg_name) => {
                    // Get from prompt arguments
                    if let Some(value) = args.get(arg_name.as_str()) {
                        // Check if this argument has a type hint
                        if let Some(spec) = self.workflow.arguments().get(arg_name) {
                            if let Some(arg_type) = &spec.arg_type {
                                // Convert string to typed value
                                arg_type.parse_value(value).map_err(|e| {
                                    crate::Error::validation(format!(
                                        "Invalid value for argument '{}': {}",
                                        arg_name, e
                                    ))
                                })?
                            } else {
                                // No type hint - use as string
                                Value::String(value.clone())
                            }
                        } else {
                            // Argument not in workflow spec - use as string
                            Value::String(value.clone())
                        }
                    } else {
                        // Check if this argument is optional in the workflow
                        let is_required = self
                            .workflow
                            .arguments()
                            .get(arg_name)
                            .is_none_or(|spec| spec.required); // Default to required if not found

                        if is_required {
                            // Required argument missing - error
                            return Err(crate::Error::validation(format!(
                                "Missing required argument '{}' for step '{}'",
                                arg_name,
                                step.name()
                            )));
                        }
                        // Optional argument missing - skip it (don't add to params)
                        continue;
                    }
                },

                DataSource::Constant(val) => {
                    // Apply template substitution to constant strings
                    // This allows SQL queries and other constant strings to use {placeholder} syntax
                    match val {
                        Value::String(s) => {
                            let substituted = Self::substitute_arguments(s, args);
                            Value::String(substituted)
                        },
                        _ => val.clone(),
                    }
                },

                DataSource::StepOutput {
                    step: binding_name,
                    field: None,
                } => {
                    // Get entire output from previous step
                    ctx.get_binding(binding_name).cloned().ok_or_else(|| {
                        crate::Error::validation(format!(
                            "Binding '{}' not found (step may not have executed yet)",
                            binding_name
                        ))
                    })?
                },

                DataSource::StepOutput {
                    step: binding_name,
                    field: Some(field_name),
                } => {
                    // Extract specific field from previous step output
                    // Supports dot notation with array indexing: "games.0.id"
                    let binding_value = ctx.get_binding(binding_name).ok_or_else(|| {
                        crate::Error::validation(format!("Binding '{}' not found", binding_name))
                    })?;

                    Self::navigate_json_path(binding_value, field_name.as_str())?.clone()
                },
            };

            params.insert(arg_name.to_string(), value);
        }

        Ok(Value::Object(params))
    }
}

#[async_trait]
impl PromptHandler for WorkflowPromptHandler {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Debug: Check if auth_context is present at handler entry
        tracing::debug!(
            "WorkflowPromptHandler.handle() - auth_context present: {}, has_token: {}",
            extra.auth_context.is_some(),
            extra
                .auth_context
                .as_ref()
                .and_then(|ctx| ctx.token.as_ref())
                .is_some()
        );

        // 0️⃣ 1️⃣ 2️⃣ The header sequence: D-04a projected prepend, user intent,
        // assistant plan (suppressed when the prepend already carries it, but
        // never at the cost of its tool-registry validation). Built by the ONE
        // producer both this handler and `TaskWorkflowPromptHandler` share.
        let mut messages = self.opening_messages(&args)?;
        let mut execution_context = ExecutionContext::new();

        // 3️⃣ Execute workflow steps sequentially with progress reporting
        let total_steps = self.workflow.steps().len();

        for (step_index, step) in self.workflow.steps().iter().enumerate() {
            // Check for cancellation before each step
            if extra.is_cancelled() {
                tracing::warn!("Workflow cancelled at step: {}", step.name());
                return Err(crate::Error::internal(format!(
                    "Workflow '{}' cancelled at step {}",
                    self.workflow.name(),
                    step.name()
                )));
            }

            // Report progress at the start of each step
            // Use the step name for a more descriptive message
            let progress_message =
                format!("Step {}/{}: {}", step_index + 1, total_steps, step.name());
            if let Err(e) = extra
                .report_count(step_index + 1, total_steps, Some(progress_message))
                .await
            {
                tracing::warn!("Failed to report workflow progress: {}", e);
                // Continue execution - progress reporting is non-critical
            }
            // Add guidance message (if present) - BEFORE attempting execution
            // Guidance helps LLM understand the step's intent, especially for hybrid execution
            if let Some(guidance_template) = step.guidance() {
                let guidance_text = Self::substitute_arguments(guidance_template, &args);
                messages.push(PromptMessage::assistant(Content::text(guidance_text)));
            }

            // Fetch resources that DON'T depend on step outputs (pre-tool phase)
            // Resources that depend on step outputs will be fetched after tool execution
            let fetch_resources_after_tool =
                Self::template_bindings_use_step_outputs(step.template_bindings());

            if !fetch_resources_after_tool && !step.resources().is_empty() {
                // Fetch resources now (before tool execution)
                if self
                    .fetch_step_resources(step, &args, &execution_context, &extra, &mut messages)
                    .await
                    .is_err()
                {
                    // Resource fetch failed - stop execution
                    return Ok(GetPromptResult {
                        description: Some(self.workflow.description().to_string()),
                        messages,
                        _meta: None,
                    });
                }
            }

            // Handle resource-only steps (no tool execution)
            if step.is_resource_only() {
                // For resource-only steps, just fetch resources (already done above or will be done below)
                // Add an assistant message to explain what we're doing
                messages.push(PromptMessage::assistant(Content::text(format!(
                    "I'll fetch the required resources for {}...",
                    step.name()
                ))));

                // If resources depend on step outputs, fetch them now
                if fetch_resources_after_tool
                    && self
                        .fetch_step_resources(
                            step,
                            &args,
                            &execution_context,
                            &extra,
                            &mut messages,
                        )
                        .await
                        .is_err()
                {
                    // Resource fetch failed - stop execution
                    return Ok(GetPromptResult {
                        description: Some(self.workflow.description().to_string()),
                        messages,
                        _meta: None,
                    });
                }

                // Continue to next step
                continue;
            }

            // Tool execution step - Try to resolve parameters and announce tool call
            match self.create_tool_call_announcement(step, &args, &execution_context) {
                Ok(announcement) => {
                    // Parameters resolved - but do they satisfy the tool's schema?
                    let Ok(params) = self.resolve_tool_parameters(step, &args, &execution_context)
                    else {
                        // Resolution failed (shouldn't happen if announcement succeeded)
                        break;
                    };

                    // Check if resolved params satisfy tool's required fields
                    let Ok(ref missing) = self.params_satisfy_tool_schema(step, &params) else {
                        // Schema check error (tool not found, etc.)
                        break;
                    };

                    if !missing.is_empty() {
                        // Params resolved but incomplete (missing required fields)
                        // This is a graceful handoff - client should provide missing params
                        // Guidance message (if present) was already added above
                        break;
                    }

                    // Params complete - execute tool server-side
                    messages.push(announcement);

                    match self
                        .execute_tool_step(step, &args, &execution_context, &extra)
                        .await
                    {
                        Ok(result) => {
                            // User message with successful result
                            messages.push(PromptMessage::user(Content::text(format!(
                                "Tool result:\n{}",
                                serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| format!("{:?}", result))
                            ))));

                            // Store binding for next steps
                            if let Some(binding) = step.binding() {
                                execution_context.store_binding(binding.clone(), result);
                            }

                            // Fetch resources that depend on step outputs (post-tool phase)
                            // These resources can now access the tool's result via template bindings
                            if fetch_resources_after_tool
                                && self
                                    .fetch_step_resources(
                                        step,
                                        &args,
                                        &execution_context,
                                        &extra,
                                        &mut messages,
                                    )
                                    .await
                                    .is_err()
                            {
                                // Resource fetch failed - stop execution
                                break;
                            }
                        },
                        Err(e) => {
                            // Execution error - STOP with error
                            messages.push(PromptMessage::user(Content::text(format!(
                                "Error executing tool: {}",
                                e
                            ))));
                            break; // Let LLM handle recovery
                        },
                    }
                },
                Err(_) => {
                    // Cannot resolve parameters deterministically
                    // This is NOT an error - it's a handoff to client LLM for hybrid execution
                    // The guidance message (if present) was already added above
                    // Client can continue using the context provided
                    break; // Graceful handoff - return partial trace
                },
            }
        }

        // Report final workflow completion
        // This bypasses rate limiting and confirms the workflow finished
        let _ = extra
            .report_count(
                total_steps,
                total_steps,
                Some("Workflow execution complete".to_string()),
            )
            .await;

        Ok(GetPromptResult {
            description: Some(self.workflow.description().to_string()),
            messages,
            _meta: None,
        })
    }

    fn metadata(&self) -> Option<PromptInfo> {
        // Convert workflow arguments to prompt arguments
        let arguments = if self.workflow.arguments().is_empty() {
            None
        } else {
            Some(
                self.workflow
                    .arguments()
                    .iter()
                    .map(|(name, spec)| {
                        let mut arg = PromptArgument::new(name.to_string())
                            .with_description(&spec.description);
                        if spec.required {
                            arg = arg.required();
                        }
                        if let Some(arg_type) = spec.arg_type {
                            arg.arg_type = Some(arg_type);
                        }
                        arg
                    })
                    .collect(),
            )
        };

        let mut info =
            PromptInfo::new(self.workflow.name()).with_description(self.workflow.description());
        if let Some(args) = arguments {
            info = info.with_arguments(args);
        }
        Some(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::dsl::{from_step, prompt_arg};
    use crate::server::workflow::{
        InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
    };
    use crate::SimpleTool;
    use serde_json::json;

    #[tokio::test]
    async fn test_workflow_prompt_handler_basic() {
        let workflow = SequentialWorkflow::new("test_workflow", "A test workflow").instruction(
            InternalPromptMessage::new(Role::System, "Process the request"),
        );

        let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new(), None);

        // Test metadata
        let metadata = handler.metadata().expect("Should have metadata");
        assert_eq!(metadata.name, "test_workflow");
        assert_eq!(metadata.description, Some("A test workflow".to_string()));
        assert!(metadata.arguments.is_none());
    }

    #[tokio::test]
    async fn test_workflow_execution_with_tools() {
        let workflow = SequentialWorkflow::new("add_project_task", "add a task to a project")
            .argument("project", "Project name", true)
            .argument("task", "Task description", true)
            .step(WorkflowStep::new("list_pages", ToolHandle::new("list_pages")).bind("pages"));

        // Create simple tool for testing
        let list_pages_tool = SimpleTool::new("list_pages", |_args, _extra| {
            Box::pin(async move { Ok(serde_json::json!({"pages": ["Website", "Mobile"]})) })
        })
        .with_description("List all pages")
        .with_schema(serde_json::json!({"type": "object"}));

        // Get tool metadata
        let mut tools = HashMap::new();
        let tool_metadata = list_pages_tool.metadata().unwrap();
        tools.insert(
            Arc::from("list_pages"),
            ToolInfo {
                name: tool_metadata.name.clone(),
                description: tool_metadata.description.unwrap_or_default(),
                input_schema: tool_metadata.input_schema,
            },
        );

        // Create tool handlers map
        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("list_pages"), Arc::new(list_pages_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("project".to_string(), "Website".to_string());
        args.insert("task".to_string(), "Fix bug".to_string());

        let extra = RequestHandlerExtra::new("test-1".to_string(), Default::default());

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute successfully");

        // Should have: user intent, assistant plan, assistant call, user result
        assert_eq!(result.messages.len(), 4, "Should have 4 messages");

        // First message: user intent
        assert_eq!(result.messages[0].role, Role::User);
        if let Content::Text { text } = &result.messages[0].content {
            assert!(text.contains("add a task to a project"));
            assert!(text.contains("Website"));
            assert!(text.contains("Fix bug"));
        }

        // Second message: assistant plan
        assert_eq!(result.messages[1].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[1].content {
            assert!(text.contains("Here's my plan"));
            assert!(text.contains("list_pages"));
        }

        // Third message: assistant tool call announcement
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert!(text.contains("Calling tool 'list_pages'"));
        }

        // Fourth message: user tool result
        assert_eq!(result.messages[3].role, Role::User);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("Tool result"));
            assert!(text.contains("Website"));
            assert!(text.contains("Mobile"));
        }
    }

    #[tokio::test]
    async fn test_complete_workflow_execution_with_bindings() {
        use crate::server::workflow::dsl::*;

        // Define a workflow that adds a task to a project with validation
        let workflow = SequentialWorkflow::new(
            "add_project_task",
            "add a task to a project with validation",
        )
        .argument("project", "Project name", true)
        .argument("task", "Task description", true)
        // Step 1: List all available pages (read-only operation)
        .step(WorkflowStep::new("list_pages", ToolHandle::new("list_pages")).bind("pages"))
        // Step 2: Verify project exists (uses output from step 1)
        .step(
            WorkflowStep::new("verify_project", ToolHandle::new("verify_project"))
                .arg("project", prompt_arg("project"))
                .arg("available_pages", from_step("pages")) // Use binding from step 1
                .bind("project_info"), // Bind output as "project_info"
        )
        // Step 3: Add task (uses outputs from previous steps)
        .step(
            WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
                .arg("project", prompt_arg("project"))
                .arg("task", prompt_arg("task"))
                .arg("project_path", field("project_info", "path")), // Extract field from step 2
        );

        // Create mock tools
        let list_pages_tool = SimpleTool::new("list_pages", |_args, _extra| {
            Box::pin(async move {
                Ok(serde_json::json!({
                    "pages": ["Website", "Mobile", "Backend"]
                }))
            })
        })
        .with_description("List all available pages")
        .with_schema(serde_json::json!({"type": "object"}));

        let verify_project_tool = SimpleTool::new("verify_project", |args, _extra| {
            Box::pin(async move {
                let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("");
                let empty_vec = vec![];
                let pages = args
                    .get("available_pages")
                    .and_then(|v| v.get("pages"))
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty_vec);

                let exists = pages.iter().any(|p| p.as_str() == Some(project));

                if exists {
                    Ok(serde_json::json!({
                        "exists": true,
                        "path": format!("/projects/{}", project)
                    }))
                } else {
                    Err(crate::Error::validation(format!(
                        "Project '{}' not found",
                        project
                    )))
                }
            })
        })
        .with_description("Verify project exists")
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "project": {"type": "string"},
                "available_pages": {"type": "object"}
            },
            "required": ["project", "available_pages"]
        }));

        let add_task_tool = SimpleTool::new("add_journal_task", |args, _extra| {
            Box::pin(async move {
                let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("");
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let path = args
                    .get("project_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(serde_json::json!({
                    "success": true,
                    "task_id": "task-123",
                    "project": project,
                    "task": task,
                    "location": format!("{}/tasks", path)
                }))
            })
        })
        .with_description("Add a task to a journal")
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "project": {"type": "string"},
                "task": {"type": "string"},
                "project_path": {"type": "string"}
            },
            "required": ["project", "task", "project_path"]
        }));

        // Build tool registries
        let mut tools = HashMap::new();
        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();

        for (name, tool) in [
            (
                "list_pages",
                Arc::new(list_pages_tool) as Arc<dyn ToolHandler>,
            ),
            ("verify_project", Arc::new(verify_project_tool)),
            ("add_journal_task", Arc::new(add_task_tool)),
        ] {
            if let Some(metadata) = tool.metadata() {
                tools.insert(
                    Arc::from(name),
                    ToolInfo {
                        name: metadata.name.clone(),
                        description: metadata.description.unwrap_or_default(),
                        input_schema: metadata.input_schema,
                    },
                );
            }
            tool_handlers.insert(Arc::from(name), tool);
        }

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("project".to_string(), "Website".to_string());
        args.insert("task".to_string(), "Fix login bug".to_string());

        let extra = RequestHandlerExtra::new("test-integration".to_string(), Default::default());

        let result = handler
            .handle(args, extra)
            .await
            .expect("Workflow should execute successfully");

        // Should have 8 messages total:
        // 1. User intent
        // 2. Assistant plan
        // 3. Assistant call (list_pages)
        // 4. User result (list_pages)
        // 5. Assistant call (verify_project)
        // 6. User result (verify_project)
        // 7. Assistant call (add_task)
        // 8. User result (add_task)
        assert_eq!(result.messages.len(), 8, "Should have 8 messages in trace");

        // Verify message 1: User intent
        assert_eq!(result.messages[0].role, Role::User);
        if let Content::Text { text } = &result.messages[0].content {
            assert!(text.contains("add a task to a project"));
            assert!(text.contains("Website"));
            assert!(text.contains("Fix login bug"));
        }

        // Verify message 2: Assistant plan
        assert_eq!(result.messages[1].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[1].content {
            assert!(text.contains("Here's my plan"));
            assert!(text.contains("list_pages"));
            assert!(text.contains("verify_project"));
            assert!(text.contains("add_journal_task"));
        }

        // Verify message 8: Final result contains data from all steps
        assert_eq!(result.messages[7].role, Role::User);
        if let Content::Text { text } = &result.messages[7].content {
            assert!(text.contains("Tool result"));
            assert!(text.contains("success"));
            assert!(text.contains("task-123"));
            assert!(text.contains("/projects/Website/tasks"));
        }
    }

    #[tokio::test]
    async fn test_optional_arguments_handling() {
        // Test that optional arguments work correctly when not provided
        let workflow = SequentialWorkflow::new("add_task", "add a task with optional priority")
            .argument("task", "Task name", true) // required
            .argument("priority", "Priority level", false) // optional
            .step(
                WorkflowStep::new("add", ToolHandle::new("add_task"))
                    .arg("task", prompt_arg("task"))
                    .arg("priority", prompt_arg("priority")) // Optional, may not be provided
                    .bind("result"),
            );

        let add_task_tool = SimpleTool::new("add_task", |args, _extra| {
            Box::pin(async move {
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let priority = args.get("priority").and_then(|v| v.as_str());

                Ok(json!({
                    "success": true,
                    "task": task,
                    "priority": priority, // Will be null if not provided
                }))
            })
        })
        .with_description("Add a task")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "task": {"type": "string"},
                "priority": {"type": "string"}
            },
            "required": ["task"]
        }));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("add_task"),
            ToolInfo {
                name: "add_task".to_string(),
                description: "Add a task".to_string(),
                input_schema: json!({"type": "object"}),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("add_task"), Arc::new(add_task_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        // Test 1: Provide only required argument (task), omit optional (priority)
        let mut args = HashMap::new();
        args.insert("task".to_string(), "Fix bug".to_string());
        // Note: priority is NOT provided

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args.clone(), extra.clone())
            .await
            .expect("Should execute successfully with optional arg missing");

        // Should have 4 messages: user intent, assistant plan, tool call, tool result
        assert_eq!(result.messages.len(), 4);

        // Verify message 3: Tool call should NOT include priority parameter
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert!(text.contains("add_task"));
            assert!(text.contains("Fix bug"));
            // Priority should NOT be in parameters since it wasn't provided
            assert!(!text.contains("priority"));
        }

        // Verify message 4: Result should show priority as null
        assert_eq!(result.messages[3].role, Role::User);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("Tool result"));
            assert!(text.contains("success"));
        }

        // Test 2: Provide both required and optional arguments
        let mut args_with_priority = HashMap::new();
        args_with_priority.insert("task".to_string(), "Write docs".to_string());
        args_with_priority.insert("priority".to_string(), "high".to_string());

        let result2 = handler
            .handle(args_with_priority, extra)
            .await
            .expect("Should execute successfully with optional arg provided");

        // Verify message 3: Tool call SHOULD include priority parameter
        if let Content::Text { text } = &result2.messages[2].content {
            assert!(text.contains("add_task"));
            assert!(text.contains("Write docs"));
            assert!(text.contains("priority"));
            assert!(text.contains("high"));
        }
    }

    #[tokio::test]
    async fn test_error_messages_appear_as_user_role() {
        // Test that tool execution errors appear as user messages, not assistant messages
        let workflow = SequentialWorkflow::new("test", "test workflow")
            .argument("input", "Input value", true)
            .step(
                WorkflowStep::new("step1", ToolHandle::new("process"))
                    .arg("value", prompt_arg("input"))
                    .bind("result"),
            );

        // Tool that always fails
        let process_tool = SimpleTool::new("process", |_args, _extra| {
            Box::pin(async move {
                Err(crate::Error::validation(
                    "Tool execution failed: invalid input",
                ))
            })
        })
        .with_description("Process data")
        .with_schema(json!({"type": "object"}));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("process"),
            ToolInfo {
                name: "process".to_string(),
                description: "Process data".to_string(),
                input_schema: json!({"type": "object"}),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("process"), Arc::new(process_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("input".to_string(), "test".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should return partial trace even with tool error");

        // Should have 4 messages: user intent, assistant plan, tool call announcement, error message
        assert_eq!(result.messages.len(), 4);

        // Verify message 1: User intent
        assert_eq!(result.messages[0].role, Role::User);

        // Verify message 2: Assistant plan
        assert_eq!(result.messages[1].role, Role::Assistant);

        // Verify message 3: Tool call announcement
        assert_eq!(result.messages[2].role, Role::Assistant);

        // Verify message 4: Error should be a USER message (not assistant)
        assert_eq!(
            result.messages[3].role,
            Role::User,
            "Tool execution error should appear as user message"
        );
        if let Content::Text { text } = &result.messages[3].content {
            assert!(
                text.contains("Error executing tool"),
                "Error message should indicate tool execution error"
            );
            assert!(
                text.contains("invalid input"),
                "Error message should contain the tool error details"
            );
        }
    }

    #[tokio::test]
    async fn test_hybrid_execution_with_guidance() {
        // Test hybrid execution where server executes deterministic steps
        // and provides guidance for steps requiring LLM reasoning
        let workflow = SequentialWorkflow::new(
            "add_project_task",
            "add a task to a Logseq project with intelligent matching",
        )
        .argument("project", "Project name (can be fuzzy)", true)
        .argument("task", "Task description", true)
        // Step 1: Server executes (deterministic)
        .step(
            WorkflowStep::new("list_pages", ToolHandle::new("list_pages"))
                .with_guidance("I'll first get all available page names from Logseq")
                .bind("pages"),
        )
        // Step 2: Client continues (needs LLM reasoning for fuzzy matching)
        .step(
            WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
                .with_guidance(
                    "I'll now:\n\
                     1. Find the page name from the list above that best matches '{project}'\n\
                     2. Format the task as: [[matched-page-name]] {task}\n\
                     3. Call add_journal_task with the formatted task",
                )
                // No .arg() mappings - server will detect params don't satisfy schema
                // and gracefully hand off to client LLM
                .bind("result"),
        );

        let list_pages_tool = SimpleTool::new("list_pages", |_args, _extra| {
            Box::pin(async move {
                Ok(json!({
                    "page_names": ["mcp-tester", "MCP Rust SDK", "Test Page"]
                }))
            })
        })
        .with_description("List all pages")
        .with_schema(json!({"type": "object"}));

        let add_task_tool = SimpleTool::new("add_journal_task", |_args, _extra| {
            Box::pin(async move { Ok(json!({"success": true})) })
        })
        .with_description("Add a task")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "formatted_task": {"type": "string"}
            },
            "required": ["formatted_task"]  // ← Required field triggers handoff
        }));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("list_pages"),
            ToolInfo {
                name: "list_pages".to_string(),
                description: "List all pages".to_string(),
                input_schema: json!({"type": "object"}),
            },
        );
        tools.insert(
            Arc::from("add_journal_task"),
            ToolInfo {
                name: "add_journal_task".to_string(),
                description: "Add a task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "formatted_task": {"type": "string"}
                    },
                    "required": ["formatted_task"]
                }),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("list_pages"), Arc::new(list_pages_tool));
        tool_handlers.insert(Arc::from("add_journal_task"), Arc::new(add_task_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("project".to_string(), "MCP Tester".to_string());
        args.insert("task".to_string(), "Fix workflow bug".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute with hybrid execution");

        // Expected message structure:
        // 1. User intent
        // 2. Assistant plan
        // 3. Guidance for step 1 (list_pages)
        // 4. Assistant tool call (list_pages)
        // 5. User tool result (list_pages)
        // 6. Guidance for step 2 (add_task) - with argument substitution
        // Then handoff to client (no more messages - can't execute step 2)

        assert_eq!(
            result.messages.len(),
            6,
            "Should have 6 messages before handoff"
        );

        // Verify message 1: User intent
        assert_eq!(result.messages[0].role, Role::User);
        if let Content::Text { text } = &result.messages[0].content {
            assert!(text.contains("add a task to a Logseq project"));
            assert!(text.contains("MCP Tester"));
            assert!(text.contains("Fix workflow bug"));
        }

        // Verify message 2: Assistant plan
        assert_eq!(result.messages[1].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[1].content {
            assert!(text.contains("Here's my plan"));
        }

        // Verify message 3: Guidance for step 1
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert_eq!(
                text, "I'll first get all available page names from Logseq",
                "Guidance should be rendered as-is"
            );
        }

        // Verify message 4: Tool call announcement
        assert_eq!(result.messages[3].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("list_pages"));
        }

        // Verify message 5: Tool result
        assert_eq!(result.messages[4].role, Role::User);
        if let Content::Text { text } = &result.messages[4].content {
            assert!(text.contains("Tool result"));
            assert!(text.contains("mcp-tester"));
            assert!(text.contains("MCP Rust SDK"));
        }

        // Verify message 6: Guidance for step 2 (with argument substitution)
        assert_eq!(result.messages[5].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[5].content {
            assert!(
                text.contains(
                    "Find the page name from the list above that best matches 'MCP Tester'"
                ),
                "Guidance should have {{project}} replaced with 'MCP Tester'"
            );
            assert!(
                text.contains("[[matched-page-name]] Fix workflow bug"),
                "Guidance should have {{task}} replaced with 'Fix workflow bug'"
            );
            assert!(text.contains("Call add_journal_task"));
        }

        // No message 7 - execution stopped (handoff to client for fuzzy matching)
        // Client LLM can now:
        // 1. See the page list from step 1
        // 2. Read the guidance on how to proceed
        // 3. Match "MCP Tester" to "mcp-tester"
        // 4. Call add_journal_task with formatted task
    }

    #[tokio::test]
    async fn test_argument_substitution_in_guidance() {
        // Test that {arg_name} patterns are substituted correctly in guidance
        let workflow = SequentialWorkflow::new("test", "test workflow")
            .argument("name", "User name", true)
            .argument("action", "Action to perform", true)
            .step(
                WorkflowStep::new("step1", ToolHandle::new("process"))
                    .with_guidance("Processing '{action}' for user '{name}'")
                    // Reference non-existent binding to force handoff
                    .arg("data", from_step("nonexistent"))
                    .bind("result"),
            );

        let process_tool = SimpleTool::new("process", |_args, _extra| {
            Box::pin(async move { Ok(json!({"ok": true})) })
        })
        .with_description("Process")
        .with_schema(json!({"type": "object"}));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("process"),
            ToolInfo {
                name: "process".to_string(),
                description: "Process".to_string(),
                input_schema: json!({"type": "object"}),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("process"), Arc::new(process_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());
        args.insert("action".to_string(), "login".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute with guidance");

        // Should have: user intent, plan, guidance (then handoff)
        assert_eq!(result.messages.len(), 3);

        // Verify guidance has substituted arguments
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert_eq!(
                text, "Processing 'login' for user 'Alice'",
                "All argument placeholders should be substituted"
            );
        }
    }

    #[tokio::test]
    async fn test_full_execution_with_guidance() {
        // Test that guidance messages appear even when server can execute fully
        let workflow = SequentialWorkflow::new("greet", "greet a user")
            .argument("name", "User name", true)
            .step(
                WorkflowStep::new("greet", ToolHandle::new("greet_user"))
                    .with_guidance("I'll greet the user '{name}'")
                    .arg("name", prompt_arg("name"))
                    .bind("greeting"),
            );

        let greet_tool = SimpleTool::new("greet_user", |args, _extra| {
            Box::pin(async move {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Guest");
                Ok(json!({"message": format!("Hello, {}!", name)}))
            })
        })
        .with_description("Greet user")
        .with_schema(json!({"type": "object"}));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("greet_user"),
            ToolInfo {
                name: "greet_user".to_string(),
                description: "Greet user".to_string(),
                input_schema: json!({"type": "object"}),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("greet_user"), Arc::new(greet_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Bob".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute fully");

        // Should have: user intent, plan, guidance, tool call, tool result
        assert_eq!(result.messages.len(), 5);

        // Verify message 3 is guidance
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert_eq!(text, "I'll greet the user 'Bob'");
        }

        // Verify execution completed
        assert_eq!(result.messages[4].role, Role::User);
        if let Content::Text { text } = &result.messages[4].content {
            assert!(text.contains("Hello, Bob!"));
        }
    }

    #[tokio::test]
    async fn test_automatic_handoff_for_incomplete_params() {
        // Test that steps with guidance but insufficient args trigger graceful handoff
        // (not validation errors) when params don't satisfy tool schema
        let workflow = SequentialWorkflow::new("workflow", "test workflow")
            .argument("input", "Input value", true)
            .step(
                WorkflowStep::new("process", ToolHandle::new("process_data"))
                    .with_guidance("Process the data using '{input}'"), // No .arg() mappings, but tool requires 'data' parameter
            );

        let process_tool = SimpleTool::new("process_data", |_args, _extra| {
            Box::pin(async move { Ok(json!({"result": "ok"})) })
        })
        .with_description("Process data")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "data": {"type": "string"}
            },
            "required": ["data"]  // ← Required field not provided by workflow
        }));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("process_data"),
            ToolInfo {
                name: "process_data".to_string(),
                description: "Process data".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "data": {"type": "string"}
                    },
                    "required": ["data"]
                }),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("process_data"), Arc::new(process_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("input".to_string(), "test".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should handoff gracefully without error");

        // Should have: user intent, plan, guidance
        // Should NOT have: tool call announcement or error message
        assert_eq!(
            result.messages.len(),
            3,
            "Should have 3 messages (intent, plan, guidance) before handoff"
        );

        // Message 1: User intent
        assert_eq!(result.messages[0].role, Role::User);

        // Message 2: Assistant plan
        assert_eq!(result.messages[1].role, Role::Assistant);

        // Message 3: Guidance (NOT an error)
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert!(
                text.contains("Process the data"),
                "Last message should be guidance"
            );
            assert!(
                !text.contains("Error"),
                "Should NOT contain error message - this is a graceful handoff"
            );
            assert!(
                !text.contains("validation"),
                "Should NOT contain validation error"
            );
        }
    }

    #[tokio::test]
    async fn test_partial_params_trigger_handoff() {
        // Test that steps with some (but not all) required params trigger handoff
        let workflow = SequentialWorkflow::new("workflow", "test workflow")
            .argument("input", "Input value", true)
            .step(
                WorkflowStep::new("process", ToolHandle::new("multi_param_tool"))
                    .with_guidance("Process data with '{input}' and additional context")
                    .arg("field1", prompt_arg("input")), // field2 is missing but required by tool schema
            );

        let multi_param_tool = SimpleTool::new("multi_param_tool", |_args, _extra| {
            Box::pin(async move { Ok(json!({"result": "ok"})) })
        })
        .with_description("Tool with multiple required params")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "field1": {"type": "string"},
                "field2": {"type": "string"}
            },
            "required": ["field1", "field2"]  // ← Both required, but only field1 provided
        }));

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("multi_param_tool"),
            ToolInfo {
                name: "multi_param_tool".to_string(),
                description: "Tool with multiple required params".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "field1": {"type": "string"},
                        "field2": {"type": "string"}
                    },
                    "required": ["field1", "field2"]
                }),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("multi_param_tool"), Arc::new(multi_param_tool));

        let handler = WorkflowPromptHandler::new(workflow, tools, tool_handlers, None);

        let mut args = HashMap::new();
        args.insert("input".to_string(), "test".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should handoff gracefully");

        // Should handoff before tool call (partial params)
        assert_eq!(
            result.messages.len(),
            3,
            "Should have 3 messages before handoff (intent, plan, guidance)"
        );

        // Last message should be guidance, not error
        assert_eq!(result.messages[2].role, Role::Assistant);
        if let Content::Text { text } = &result.messages[2].content {
            assert!(text.contains("Process data"));
            assert!(!text.contains("Error"));
        }
    }

    #[tokio::test]
    async fn test_workflow_with_resource_fetching() {
        use crate::server::ResourceHandler;
        use crate::types::{Content, ReadResourceResult};
        use async_trait::async_trait;

        // Create mock resource handler
        struct MockResourceHandler;

        #[async_trait]
        impl ResourceHandler for MockResourceHandler {
            async fn read(
                &self,
                uri: &str,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<ReadResourceResult> {
                if uri == "docs://task-format" {
                    Ok(ReadResourceResult::new(vec![Content::text(
                        "Task Format Guide:\n- Use [[page-name]] for links\n- Add TASK prefix for action items",
                    )]))
                } else {
                    Err(crate::Error::validation(format!(
                        "Unknown resource: {}",
                        uri
                    )))
                }
            }

            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult {
                    resources: vec![],
                    next_cursor: None,
                    ttl_ms: None,
                    cache_scope: None,
                })
            }
        }

        // Create workflow with resource
        let workflow = SequentialWorkflow::new("add_task", "Add a task with formatting guide")
            .argument("task", "Task description", true)
            .step(
                WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
                    .with_guidance("Format the task according to the guide")
                    .with_resource("docs://task-format")
                    .expect("Valid resource URI")
                    .arg("task", DataSource::prompt_arg("task"))
                    .bind("result"),
            );

        // Create mock tool
        let add_task_tool = SimpleTool::new("add_journal_task", |args, _extra| {
            Box::pin(async move {
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::json!({
                    "success": true,
                    "task": task
                }))
            })
        })
        .with_description("Add a task")
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "task": {"type": "string"}
            },
            "required": ["task"]
        }));

        // Create tool info and handlers
        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("add_journal_task"),
            ToolInfo {
                name: "add_journal_task".to_string(),
                description: "Add a task".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string"}
                    },
                    "required": ["task"]
                }),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("add_journal_task"), Arc::new(add_task_tool));

        // Create handler with resource handler
        let handler = WorkflowPromptHandler::new(
            workflow,
            tools,
            tool_handlers,
            Some(Arc::new(MockResourceHandler)),
        );

        let mut args = HashMap::new();
        args.insert("task".to_string(), "Fix bug".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute successfully");

        // Should have messages:
        // 1. User intent
        // 2. Assistant plan
        // 3. Guidance message
        // 4. Resource content (User)
        // 5. Tool call announcement
        // 6. Tool result
        assert_eq!(result.messages.len(), 6);

        // Check resource was embedded
        assert_eq!(result.messages[3].role, Role::User);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("Resource content from docs://task-format"));
            assert!(text.contains("Task Format Guide"));
            assert!(text.contains("[[page-name]]"));
        } else {
            panic!("Expected text message for resource content");
        }
    }

    #[tokio::test]
    async fn test_workflow_with_multiple_resources() {
        use crate::server::ResourceHandler;
        use crate::types::{Content, ReadResourceResult};
        use async_trait::async_trait;

        // Create mock resource handler
        struct MockResourceHandler;

        #[async_trait]
        impl ResourceHandler for MockResourceHandler {
            async fn read(
                &self,
                uri: &str,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<ReadResourceResult> {
                match uri {
                    "docs://format" => Ok(ReadResourceResult::new(vec![Content::text(
                        "Format: [[link]]",
                    )])),
                    "docs://examples" => Ok(ReadResourceResult::new(vec![Content::text(
                        "Examples:\n- [[project]] Task 1\n- [[project]] Task 2",
                    )])),
                    _ => Err(crate::Error::validation(format!(
                        "Unknown resource: {}",
                        uri
                    ))),
                }
            }

            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult {
                    resources: vec![],
                    next_cursor: None,
                    ttl_ms: None,
                    cache_scope: None,
                })
            }
        }

        // Create workflow with multiple resources
        let workflow = SequentialWorkflow::new("add_task", "Add a task with guides")
            .argument("task", "Task description", true)
            .step(
                WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
                    .with_guidance("Use the format and examples provided")
                    .with_resource("docs://format")
                    .expect("Valid resource URI")
                    .with_resource("docs://examples")
                    .expect("Valid resource URI")
                    .arg("task", DataSource::prompt_arg("task"))
                    .bind("result"),
            );

        // Create mock tool
        let add_task_tool = SimpleTool::new("add_journal_task", |args, _extra| {
            Box::pin(async move {
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::json!({
                    "success": true,
                    "task": task
                }))
            })
        })
        .with_description("Add a task")
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "task": {"type": "string"}
            },
            "required": ["task"]
        }));

        // Create tool info and handlers
        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("add_journal_task"),
            ToolInfo {
                name: "add_journal_task".to_string(),
                description: "Add a task".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string"}
                    },
                    "required": ["task"]
                }),
            },
        );

        let mut tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();
        tool_handlers.insert(Arc::from("add_journal_task"), Arc::new(add_task_tool));

        // Create handler with resource handler
        let handler = WorkflowPromptHandler::new(
            workflow,
            tools,
            tool_handlers,
            Some(Arc::new(MockResourceHandler)),
        );

        let mut args = HashMap::new();
        args.insert("task".to_string(), "Fix bug".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should execute successfully");

        // Should have messages:
        // 1. User intent
        // 2. Assistant plan
        // 3. Guidance message
        // 4. First resource content (User)
        // 5. Second resource content (User)
        // 6. Tool call announcement
        // 7. Tool result
        assert_eq!(result.messages.len(), 7);

        // Check both resources were embedded
        assert_eq!(result.messages[3].role, Role::User);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("docs://format"));
            assert!(text.contains("Format: [[link]]"));
        } else {
            panic!("Expected first resource content");
        }

        assert_eq!(result.messages[4].role, Role::User);
        if let Content::Text { text } = &result.messages[4].content {
            assert!(text.contains("docs://examples"));
            assert!(text.contains("Examples:"));
        } else {
            panic!("Expected second resource content");
        }
    }

    #[tokio::test]
    async fn test_workflow_resource_fetch_error() {
        use crate::server::ResourceHandler;
        use crate::types::ReadResourceResult;
        use async_trait::async_trait;

        // Create mock resource handler that always fails
        struct FailingResourceHandler;

        #[async_trait]
        impl ResourceHandler for FailingResourceHandler {
            async fn read(
                &self,
                uri: &str,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<ReadResourceResult> {
                Err(crate::Error::validation(format!(
                    "Resource not found: {}",
                    uri
                )))
            }

            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult {
                    resources: vec![],
                    next_cursor: None,
                    ttl_ms: None,
                    cache_scope: None,
                })
            }
        }

        // Create workflow with resource
        let workflow = SequentialWorkflow::new("add_task", "Add a task with guide")
            .argument("task", "Task description", true)
            .step(
                WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
                    .with_guidance("Format the task")
                    .with_resource("docs://missing")
                    .expect("Valid resource URI")
                    .arg("task", DataSource::prompt_arg("task")),
            );

        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("add_journal_task"),
            ToolInfo {
                name: "add_journal_task".to_string(),
                description: "Add a task".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string"}
                    },
                    "required": ["task"]
                }),
            },
        );

        let handler = WorkflowPromptHandler::new(
            workflow,
            tools,
            HashMap::new(),
            Some(Arc::new(FailingResourceHandler)),
        );

        let mut args = HashMap::new();
        args.insert("task".to_string(), "Fix bug".to_string());

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            "test".to_string(),
            Default::default(),
        );

        let result = handler
            .handle(args, extra)
            .await
            .expect("Should return result with error message");

        // Should have messages:
        // 1. User intent
        // 2. Assistant plan
        // 3. Guidance message
        // 4. Resource fetch error (User)
        assert_eq!(result.messages.len(), 4);

        // Last message should be error
        assert_eq!(result.messages[3].role, Role::User);
        if let Content::Text { text } = &result.messages[3].content {
            assert!(text.contains("Error fetching resource"));
            assert!(text.contains("docs://missing"));
        } else {
            panic!("Expected error message");
        }
    }

    #[test]
    fn test_navigate_json_path_basic_array_access() {
        let data = json!({
            "games": [
                {"id": "game1", "score": 10},
                {"id": "game2", "score": 5}
            ]
        });

        // Test basic array access
        let result = WorkflowPromptHandler::navigate_json_path(&data, "games.0.id").unwrap();
        assert_eq!(result, &json!("game1"));

        let result = WorkflowPromptHandler::navigate_json_path(&data, "games.1.score").unwrap();
        assert_eq!(result, &json!(5));
    }

    #[test]
    fn test_navigate_json_path_nested_arrays() {
        let data = json!({
            "matrix": [
                [{"value": 1}, {"value": 2}],
                [{"value": 3}, {"value": 4}]
            ]
        });

        // Test nested array access
        let result = WorkflowPromptHandler::navigate_json_path(&data, "matrix.0.1.value").unwrap();
        assert_eq!(result, &json!(2));

        let result = WorkflowPromptHandler::navigate_json_path(&data, "matrix.1.0.value").unwrap();
        assert_eq!(result, &json!(3));
    }

    #[test]
    fn test_navigate_json_path_mixed_structures() {
        let data = json!({
            "data": {
                "users": [
                    {
                        "name": "Alice",
                        "games": [
                            {"id": "a1", "score": 100},
                            {"id": "a2", "score": 200}
                        ]
                    },
                    {
                        "name": "Bob",
                        "games": [
                            {"id": "b1", "score": 50}
                        ]
                    }
                ]
            }
        });

        // Deep nested access
        let result =
            WorkflowPromptHandler::navigate_json_path(&data, "data.users.0.games.1.score").unwrap();
        assert_eq!(result, &json!(200));

        let result = WorkflowPromptHandler::navigate_json_path(&data, "data.users.1.name").unwrap();
        assert_eq!(result, &json!("Bob"));
    }

    #[test]
    fn test_navigate_json_path_numeric_object_keys() {
        // Edge case: numeric object keys (object field should win)
        let data = json!({
            "0": "object_value",
            "items": [1, 2, 3]
        });

        // Numeric key as object field
        let result = WorkflowPromptHandler::navigate_json_path(&data, "0").unwrap();
        assert_eq!(result, &json!("object_value"));

        // Array indexing still works
        let result = WorkflowPromptHandler::navigate_json_path(&data, "items.0").unwrap();
        assert_eq!(result, &json!(1));

        let result = WorkflowPromptHandler::navigate_json_path(&data, "items.2").unwrap();
        assert_eq!(result, &json!(3));
    }

    #[test]
    fn test_navigate_json_path_errors() {
        let data = json!({
            "items": [1, 2, 3],
            "obj": {"field": "value"}
        });

        // Out of bounds
        let result = WorkflowPromptHandler::navigate_json_path(&data, "items.5");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("out of bounds"));
        assert!(err_msg.contains("Array has 3 elements"));

        // Index into non-array
        let result = WorkflowPromptHandler::navigate_json_path(&data, "obj.0");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Cannot index into non-array"));

        // Non-existent field
        let result = WorkflowPromptHandler::navigate_json_path(&data, "missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_extract_field_as_string_with_arrays() {
        let data = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ],
            "count": 2
        });

        // Extract from array
        let result = WorkflowPromptHandler::extract_field_as_string(&data, "users.0.name").unwrap();
        assert_eq!(result, "Alice");

        let result = WorkflowPromptHandler::extract_field_as_string(&data, "users.1.age").unwrap();
        assert_eq!(result, "25");

        // Extract non-array field
        let result = WorkflowPromptHandler::extract_field_as_string(&data, "count").unwrap();
        assert_eq!(result, "2");
    }

    #[test]
    fn test_extract_field_as_string_array_to_json() {
        let data = json!({
            "items": [1, 2, 3],
            "nested": {
                "arrays": [[1, 2], [3, 4]]
            }
        });

        // Extract entire array (should serialize as JSON)
        let result = WorkflowPromptHandler::extract_field_as_string(&data, "items").unwrap();
        assert_eq!(result, "[1,2,3]");

        // Extract nested array
        let result =
            WorkflowPromptHandler::extract_field_as_string(&data, "nested.arrays.0").unwrap();
        assert_eq!(result, "[1,2]");
    }

    #[test]
    fn test_value_type_name() {
        assert_eq!(WorkflowPromptHandler::value_type_name(&json!(null)), "null");
        assert_eq!(
            WorkflowPromptHandler::value_type_name(&json!(true)),
            "boolean"
        );
        assert_eq!(WorkflowPromptHandler::value_type_name(&json!(42)), "number");
        assert_eq!(
            WorkflowPromptHandler::value_type_name(&json!("text")),
            "string"
        );
        assert_eq!(
            WorkflowPromptHandler::value_type_name(&json!([1, 2, 3])),
            "array"
        );
        assert_eq!(
            WorkflowPromptHandler::value_type_name(&json!({"key": "value"})),
            "object"
        );
    }
}
