//! Active execution engine for task-backed workflow prompts.
//!
//! [`TaskWorkflowPromptHandler`] wraps a [`WorkflowPromptHandler`] to add
//! task lifecycle management with durable progress tracking. When a workflow
//! prompt is invoked, this handler:
//!
//! 1. Creates a task via the [`TaskRouter`]
//! 2. Runs its own step loop using the inner handler's `pub(crate)` helpers
//! 3. Accumulates step results in memory during execution
//! 4. Classifies failures into typed `PauseReason` variants
//! 5. Batch-writes all state (progress, results, pause reason) to the task store
//! 6. Auto-completes the task when all steps succeed
//! 7. Enriches the [`GetPromptResult`] with `_meta` containing task state
//!
//! The execution loop stops at the first unresolvable step without failing
//! the task -- the task stays Working with completed steps having results
//! and remaining steps staying Pending.
//!
//! # Graceful Degradation
//!
//! If task creation fails, the handler logs the error and falls back to
//! returning the inner handler's result without `_meta`. If the batch write
//! to the task store fails, the prompt result is returned anyway with `_meta`
//! constructed from in-memory state.
//!
//! # Architecture Note
//!
//! The typed `PauseReason`, `StepStatus`, and workflow progress types
//! are defined in the `pmcp-tasks` crate. Because `pmcp-tasks` depends on
//! `pmcp` (not the reverse), this module uses local mirror types that produce
//! identical JSON. The task variable key constants are duplicated here to
//! maintain the `_workflow.*` convention.

use super::data_source::DataSource;
use super::prompt_handler::{ExecutionContext, WorkflowPromptHandler};
use super::sequential::SequentialWorkflow;
use super::workflow_step::WorkflowStep;
use crate::error::Result;
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::tasks::TaskRouter;
use crate::server::PromptHandler;
#[cfg(test)]
use crate::types::Role;
use crate::types::{Content, GetPromptResult, PromptInfo, PromptMessage};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// === Task variable key constants (mirrors pmcp_tasks::types::workflow) ===

/// Task variable key for the structured workflow progress object.
const WORKFLOW_PROGRESS_KEY: &str = "_workflow.progress";

/// Task variable key for the pause reason when workflow execution pauses.
const WORKFLOW_PAUSE_REASON_KEY: &str = "_workflow.pause_reason";

/// Builds the task variable key for a step's tool result.
fn workflow_result_key(step_name: &str) -> String {
    format!("_workflow.result.{step_name}")
}

// === Step status (mirrors pmcp_tasks::types::workflow::StepStatus) ===

/// Runtime outcome of a workflow step.
///
/// Mirrors `pmcp_tasks::types::workflow::StepStatus` to avoid circular
/// dependency. Serializes to the same `snake_case` strings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum StepStatus {
    /// Step has not been attempted yet.
    #[default]
    Pending,
    /// Step completed successfully.
    Completed,
    /// Step failed (error recorded in variables).
    Failed,
    /// Step was skipped.
    Skipped,
}

impl StepStatus {
    /// Convert to the string representation used in JSON.
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

// === Pause reason (mirrors pmcp_tasks::types::workflow::PauseReason) ===

/// Describes why the partial execution engine paused before completing all
/// workflow steps.
///
/// Mirrors `pmcp_tasks::types::workflow::PauseReason`. Each variant produces
/// the same JSON shape with `"type"` tag and `camelCase` field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PauseReason {
    /// A step's parameters could not be resolved from available context.
    UnresolvableParams {
        blocked_step: String,
        missing_param: String,
        suggested_tool: String,
    },
    /// Resolved parameters miss required schema fields.
    SchemaMismatch {
        blocked_step: String,
        missing_fields: Vec<String>,
        suggested_tool: String,
    },
    /// Tool execution returned an error.
    ToolError {
        failed_step: String,
        error: String,
        retryable: bool,
        suggested_tool: String,
    },
    /// Step depends on output from a failed or skipped step.
    UnresolvedDependency {
        blocked_step: String,
        missing_output: String,
        producing_step: String,
        suggested_tool: String,
    },
}

impl PauseReason {
    /// Serialize to a JSON [`Value`] matching the `pmcp-tasks` serde format.
    fn to_value(&self) -> Value {
        match self {
            Self::UnresolvableParams {
                blocked_step,
                missing_param,
                suggested_tool,
            } => serde_json::json!({
                "type": "unresolvableParams",
                "blockedStep": blocked_step,
                "missingParam": missing_param,
                "suggestedTool": suggested_tool,
            }),
            Self::SchemaMismatch {
                blocked_step,
                missing_fields,
                suggested_tool,
            } => serde_json::json!({
                "type": "schemaMismatch",
                "blockedStep": blocked_step,
                "missingFields": missing_fields,
                "suggestedTool": suggested_tool,
            }),
            Self::ToolError {
                failed_step,
                error,
                retryable,
                suggested_tool,
            } => serde_json::json!({
                "type": "toolError",
                "failedStep": failed_step,
                "error": error,
                "retryable": retryable,
                "suggestedTool": suggested_tool,
            }),
            Self::UnresolvedDependency {
                blocked_step,
                missing_output,
                producing_step,
                suggested_tool,
            } => serde_json::json!({
                "type": "unresolvedDependency",
                "blockedStep": blocked_step,
                "missingOutput": missing_output,
                "producingStep": producing_step,
                "suggestedTool": suggested_tool,
            }),
        }
    }
}

/// Task-aware wrapper for [`WorkflowPromptHandler`].
///
/// Composes with the inner handler via its `pub(crate)` helpers: all prompt
/// metadata and step execution logic is reused from the inner handler, while
/// this handler owns the step loop, progress tracking, and task lifecycle.
///
/// # Construction
///
/// Created by the builder when a [`SequentialWorkflow`] has task support
/// enabled and a [`TaskRouter`] is configured on the server.
///
/// ```rust,ignore
/// let handler = TaskWorkflowPromptHandler::new(
///     inner_handler,
///     task_router.clone(),
///     workflow.clone(),
/// );
/// ```
pub struct TaskWorkflowPromptHandler {
    /// The inner workflow handler that provides step execution helpers.
    inner: WorkflowPromptHandler,
    /// Task router for creating/managing workflow tasks.
    task_router: Arc<dyn TaskRouter>,
    /// The workflow definition (needed for step metadata to build `WorkflowProgress`).
    workflow: SequentialWorkflow,
}

impl std::fmt::Debug for TaskWorkflowPromptHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskWorkflowPromptHandler")
            .field("workflow", &self.workflow.name())
            .field("inner", &"WorkflowPromptHandler")
            .finish()
    }
}

impl TaskWorkflowPromptHandler {
    /// Create a new task-aware workflow prompt handler.
    ///
    /// # Arguments
    ///
    /// * `inner` - The workflow prompt handler that provides step execution helpers.
    /// * `task_router` - Task router for creating and managing workflow tasks.
    /// * `workflow` - The workflow definition (used for step metadata).
    pub fn new(
        inner: WorkflowPromptHandler,
        task_router: Arc<dyn TaskRouter>,
        workflow: SequentialWorkflow,
    ) -> Self {
        Self {
            inner,
            task_router,
            workflow,
        }
    }

    /// Build the initial [`WorkflowProgress`] from the workflow definition.
    ///
    /// Returns a JSON [`Value`] with all steps set to `"pending"`, matching
    /// the `WorkflowProgress` schema in `pmcp-tasks`.
    fn build_initial_progress_typed(&self) -> Value {
        let steps: Vec<Value> = self
            .workflow
            .steps()
            .iter()
            .map(|step| {
                let mut step_obj = serde_json::Map::new();
                step_obj.insert("name".to_string(), Value::String(step.name().to_string()));
                if let Some(tool) = step.tool() {
                    step_obj.insert("tool".to_string(), Value::String(tool.name().to_string()));
                }
                step_obj.insert("status".to_string(), Value::String("pending".to_string()));
                Value::Object(step_obj)
            })
            .collect();

        let goal = format!("{}: {}", self.workflow.name(), self.workflow.description());

        serde_json::json!({
            "goal": goal,
            "steps": steps,
            "schemaVersion": 1
        })
    }

    /// Build the `_meta` map for the enriched [`GetPromptResult`].
    ///
    /// Contains `task_id`, `task_status`, a summary `steps` array, and
    /// optionally the `pause_reason` when execution paused.
    fn build_meta_map(
        task_id: &str,
        task_status: &str,
        step_names: &[String],
        step_statuses: &[StepStatus],
        pause_reason: Option<&PauseReason>,
    ) -> serde_json::Map<String, Value> {
        let steps: Vec<Value> = step_names
            .iter()
            .zip(step_statuses.iter())
            .map(|(name, status)| {
                serde_json::json!({
                    "name": name,
                    "status": status.as_str(),
                })
            })
            .collect();

        let mut meta = serde_json::Map::new();
        meta.insert("task_id".to_string(), Value::String(task_id.to_string()));
        meta.insert(
            "task_status".to_string(),
            Value::String(task_status.to_string()),
        );
        meta.insert("steps".to_string(), Value::Array(steps));

        if let Some(reason) = pause_reason {
            meta.insert("pause_reason".to_string(), reason.to_value());
        }

        meta
    }

    /// Build placeholder argument strings for a step whose parameters cannot
    /// be fully resolved.
    ///
    /// For each argument in the step:
    /// - `DataSource::PromptArg(name)` uses the actual prompt arg value if
    ///   available, otherwise `<prompt arg {name}>`.
    /// - `DataSource::StepOutput { step: binding, field: None }` produces
    ///   `<output from {binding}>`.
    /// - `DataSource::StepOutput { step: binding, field: Some(f) }` produces
    ///   `<field '{f}' from {binding}>`.
    /// - `DataSource::Constant(val)` serializes the value to a string.
    ///
    /// Returns a JSON-formatted string of the placeholder argument map.
    fn build_placeholder_args(step: &WorkflowStep, args: &HashMap<String, String>) -> String {
        let mut map = serde_json::Map::new();

        for (arg_name, data_source) in step.arguments() {
            let value = match data_source {
                DataSource::PromptArg(name) => {
                    if let Some(val) = args.get(name.as_str()) {
                        Value::String(val.clone())
                    } else {
                        Value::String(format!("<prompt arg {}>", name))
                    }
                },
                DataSource::StepOutput {
                    step: binding,
                    field: None,
                } => Value::String(format!("<output from {}>", binding)),
                DataSource::StepOutput {
                    step: binding,
                    field: Some(f),
                } => Value::String(format!("<field '{}' from {}>", f, binding)),
                DataSource::Constant(val) => val.clone(),
            };
            map.insert(arg_name.to_string(), value);
        }

        serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
    }

    /// Build the handoff assistant message narrating what happened and what
    /// steps remain for the client to execute.
    ///
    /// The message has two sections:
    /// 1. **What happened** -- derived from the [`PauseReason`] variant.
    /// 2. **Remaining steps** -- a numbered list of pending steps with tool
    ///    names, resolved (or placeholder) arguments, and guidance text.
    ///
    /// # Important Invariants
    ///
    /// - The task ID never appears in the narrative text (it belongs in `_meta`).
    /// - Completed steps are not re-summarized (they are already in the
    ///   conversation trace as tool-call/result message pairs).
    /// - When a step's arguments cannot be resolved, placeholder syntax is
    ///   used (e.g., `<output from deploy_result>`).
    /// - If the pause is a retryable `ToolError`, the failed step appears as
    ///   the first item in the remaining steps list.
    fn build_handoff_message(
        &self,
        step_statuses: &[StepStatus],
        pause_reason: &PauseReason,
        args: &HashMap<String, String>,
        execution_context: &ExecutionContext,
    ) -> PromptMessage {
        let mut text = String::new();

        // Section 1: What happened (from pause_reason)
        match pause_reason {
            PauseReason::ToolError {
                failed_step,
                error,
                retryable,
                ..
            } => {
                text.push_str(&format!("Step '{}' failed: {}.", failed_step, error));
                if *retryable {
                    text.push_str(" This step is retryable.");
                }
            },
            PauseReason::UnresolvableParams {
                blocked_step,
                missing_param,
                ..
            } => {
                text.push_str(&format!(
                    "Could not resolve parameter '{}' for step '{}'.",
                    missing_param, blocked_step
                ));
            },
            PauseReason::SchemaMismatch {
                blocked_step,
                missing_fields,
                ..
            } => {
                let fields = missing_fields.join(", ");
                text.push_str(&format!(
                    "Step '{}' has missing required fields: {}.",
                    blocked_step, fields
                ));
            },
            PauseReason::UnresolvedDependency {
                blocked_step,
                missing_output,
                producing_step,
                ..
            } => {
                text.push_str(&format!(
                    "Step '{}' depends on output '{}' from step '{}', which did not complete.",
                    blocked_step, missing_output, producing_step
                ));
            },
        }

        // Section 2: Remaining steps
        text.push_str("\n\nTo continue the workflow, make these tool calls:\n\n");

        let mut step_num = 1;

        // If the pause is a retryable ToolError, include the failed step first
        if let PauseReason::ToolError {
            failed_step,
            retryable: true,
            ..
        } = pause_reason
        {
            for (idx, step) in self.workflow.steps().iter().enumerate() {
                if step_statuses.get(idx) == Some(&StepStatus::Failed)
                    && step.name().as_str() == failed_step.as_str()
                {
                    let tool_name = step
                        .tool()
                        .map_or_else(|| "unknown".to_string(), |t| t.name().to_string());

                    let args_str =
                        match self
                            .inner
                            .resolve_tool_parameters(step, args, execution_context)
                        {
                            Ok(resolved) => serde_json::to_string(&resolved)
                                .unwrap_or_else(|_| "{}".to_string()),
                            Err(_) => Self::build_placeholder_args(step, args),
                        };

                    text.push_str(&format!(
                        "{}. Call {} with {}\n",
                        step_num, tool_name, args_str
                    ));

                    if let Some(guidance) = step.guidance() {
                        let guidance_text =
                            WorkflowPromptHandler::substitute_arguments(guidance, args);
                        text.push_str(&format!("   Note: {}\n", guidance_text));
                    }

                    step_num += 1;
                    break;
                }
            }
        }

        // List pending steps
        for (idx, step) in self.workflow.steps().iter().enumerate() {
            if step_statuses.get(idx) != Some(&StepStatus::Pending) {
                continue;
            }

            let tool_name = step
                .tool()
                .map_or_else(|| "unknown".to_string(), |t| t.name().to_string());

            let args_str = match self
                .inner
                .resolve_tool_parameters(step, args, execution_context)
            {
                Ok(resolved) => {
                    serde_json::to_string(&resolved).unwrap_or_else(|_| "{}".to_string())
                },
                Err(_) => Self::build_placeholder_args(step, args),
            };

            text.push_str(&format!(
                "{}. Call {} with {}\n",
                step_num, tool_name, args_str
            ));

            if let Some(guidance) = step.guidance() {
                let guidance_text = WorkflowPromptHandler::substitute_arguments(guidance, args);
                text.push_str(&format!("   Note: {}\n", guidance_text));
            }

            step_num += 1;
        }

        PromptMessage::assistant(Content::text(text))
    }

    /// Resolve the owner ID from the request's auth context.
    ///
    /// Follows the same pattern as `ServerCore::resolve_task_owner`:
    /// delegates to `TaskRouter::resolve_owner` with available auth fields.
    fn resolve_owner(&self, extra: &RequestHandlerExtra) -> String {
        match &extra.auth_context {
            Some(ctx) => {
                self.task_router
                    .resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)
            },
            None => self.task_router.resolve_owner(None, None, None),
        }
    }
}

/// Find a producing step by binding name and return it with its status.
fn find_producing_step<'a>(
    binding_name: &str,
    all_steps: &'a [WorkflowStep],
    step_statuses: &[StepStatus],
) -> Option<(&'a WorkflowStep, StepStatus)> {
    for (idx, producing_step) in all_steps.iter().enumerate() {
        let Some(binding) = producing_step.binding() else {
            continue;
        };
        if binding.as_str() != binding_name {
            continue;
        }
        let status = step_statuses.get(idx).copied()?;
        return Some((producing_step, status));
    }
    None
}

/// Inspect a single `(arg_name, data_source)` entry for a `StepOutput`
/// dependency whose producer failed or was skipped, returning a matching
/// `UnresolvedDependency` reason if so.
fn dependency_failure_for_arg(
    data_source: &DataSource,
    all_steps: &[WorkflowStep],
    step_statuses: &[StepStatus],
    blocked_step: &str,
) -> Option<PauseReason> {
    let DataSource::StepOutput {
        step: binding_name, ..
    } = data_source
    else {
        return None;
    };
    let (producing_step, status) =
        find_producing_step(binding_name.as_str(), all_steps, step_statuses)?;
    if status != StepStatus::Failed && status != StepStatus::Skipped {
        return None;
    }
    let producing_tool = producing_step
        .tool()
        .map(|t| t.name().to_string())
        .unwrap_or_default();
    Some(PauseReason::UnresolvedDependency {
        blocked_step: blocked_step.to_string(),
        missing_output: binding_name.to_string(),
        producing_step: producing_step.name().to_string(),
        suggested_tool: producing_tool,
    })
}

/// Classify a parameter resolution failure into a typed [`PauseReason`].
///
/// When parameter resolution fails for a step, this function inspects the
/// step's arguments to determine if the failure is due to a dependency on
/// a failed or skipped producing step ([`PauseReason::UnresolvedDependency`])
/// or a generic resolution failure ([`PauseReason::UnresolvableParams`]).
///
/// Refactored in 75-01 Task 1a-B (P1): extracted [`find_producing_step`]
/// and [`dependency_failure_for_arg`] so the outer function is a flat scan
/// with a fallback.
fn classify_resolution_failure(
    step: &WorkflowStep,
    all_steps: &[WorkflowStep],
    step_statuses: &[StepStatus],
) -> PauseReason {
    let blocked_step = step.name().to_string();
    let suggested_tool = step
        .tool()
        .map(|t| t.name().to_string())
        .unwrap_or_default();

    for (_arg_name, data_source) in step.arguments() {
        if let Some(reason) =
            dependency_failure_for_arg(data_source, all_steps, step_statuses, &blocked_step)
        {
            return reason;
        }
    }

    let missing_param = step
        .arguments()
        .keys()
        .next()
        .map_or_else(|| "unknown".to_string(), |k| k.to_string());

    PauseReason::UnresolvableParams {
        blocked_step,
        missing_param,
        suggested_tool,
    }
}

/// Build an updated progress JSON value from step statuses.
///
/// Takes the initial progress (with goal and step metadata) and updates
/// each step's status from the actual execution results.
fn build_updated_progress(initial: &Value, step_statuses: &[StepStatus]) -> Value {
    let mut progress = initial.clone();
    if let Some(steps) = progress.get_mut("steps").and_then(|s| s.as_array_mut()) {
        for (i, step) in steps.iter_mut().enumerate() {
            if let Some(status) = step_statuses.get(i) {
                if let Some(obj) = step.as_object_mut() {
                    obj.insert(
                        "status".to_string(),
                        Value::String(status.as_str().to_string()),
                    );
                }
            }
        }
    }
    progress
}

#[async_trait]
impl PromptHandler for TaskWorkflowPromptHandler {
    /// Execute the workflow with active step tracking and task lifecycle.
    ///
    /// Orchestration flow:
    /// 1. Resolve owner from auth context
    /// 2. Build initial progress from workflow definition
    /// 3. Create task via task router (graceful degradation on failure)
    /// 4. Run active step loop using inner handler's helpers
    /// 5. Batch-write progress, results, and pause reason to task store
    /// 6. Auto-complete if all steps succeeded
    /// 7. Enrich result with `_meta`
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // 1. Resolve owner
        let owner_id = self.resolve_owner(&extra);

        // 2. Build initial progress (typed)
        let initial_progress = self.build_initial_progress_typed();

        // 3. Create task (graceful degradation on failure)
        let task_id = match self
            .task_router
            .create_workflow_task(self.workflow.name(), &owner_id, initial_progress.clone())
            .await
        {
            Ok(value) => value
                .get("task")
                .and_then(|t| t.get("taskId"))
                .and_then(|v| v.as_str())
                .map(String::from),
            Err(e) => {
                tracing::warn!(
                    "Task creation failed for workflow '{}', proceeding without task tracking: {}",
                    self.workflow.name(),
                    e
                );
                None
            },
        };

        // If no task was created, delegate to inner handler (graceful degradation)
        let Some(task_id) = task_id else {
            let result = self.inner.handle(args, extra).await?;
            return Ok(result);
        };

        // 4. Active execution loop
        let step_count = self.workflow.steps().len();
        let total_steps = step_count;
        let mut messages: Vec<PromptMessage> = Vec::new();
        let mut execution_context = ExecutionContext::new();
        let mut step_results: Vec<(String, Value)> = Vec::new();
        let mut step_statuses: Vec<StepStatus> = vec![StepStatus::Pending; step_count];
        let mut pause_reason: Option<PauseReason> = None;

        // Add header messages.
        //
        // This branch rebuilds the message list itself rather than delegating to
        // `WorkflowPromptHandler::handle`, so it must reproduce that handler's
        // header sequence exactly — otherwise a `has_task_support(true)` workflow
        // would get a different transcript from a plain one. The D-04a prepend is
        // read through the ONE crate-private producer on `inner`, never
        // re-rendered here: `prepend.is_some()` IS the flag, so the two handlers
        // cannot drift.
        let prepend = self.inner.projected_prepend();
        let suppress_plan = prepend.is_some();
        messages.extend(prepend);
        messages.push(self.inner.create_user_intent(&args));
        // Why: this call is the tool-registry validation — it is where
        // `Error::Internal("Tool '{}' not found in registry")` is raised. It runs
        // UNCONDITIONALLY and its `?` propagates in both flag states; only the
        // push of its message is suppressed when the projected body already
        // carries the step-and-tool list. Guarding the call instead of the push
        // would silently delete a validation that flag-off callers have today.
        let plan_message = self.inner.create_assistant_plan()?;
        if !suppress_plan {
            messages.push(plan_message);
        }

        for (idx, step) in self.workflow.steps().iter().enumerate() {
            // Check cancellation
            if extra.is_cancelled() {
                tracing::warn!("Workflow cancelled at step: {}", step.name());
                return Err(crate::Error::internal(format!(
                    "Workflow '{}' cancelled at step {}",
                    self.workflow.name(),
                    step.name()
                )));
            }

            // Report progress
            let progress_message = format!("Step {}/{}: {}", idx + 1, total_steps, step.name());
            if let Err(e) = extra
                .report_count(idx + 1, total_steps, Some(progress_message))
                .await
            {
                tracing::warn!("Failed to report workflow progress: {}", e);
            }

            // Add guidance message if step has guidance
            if let Some(guidance_template) = step.guidance() {
                let guidance_text =
                    WorkflowPromptHandler::substitute_arguments(guidance_template, &args);
                messages.push(PromptMessage::assistant(Content::text(guidance_text)));
            }

            // Fetch pre-tool resources (those not depending on step outputs)
            let fetch_resources_after_tool =
                WorkflowPromptHandler::template_bindings_use_step_outputs(step.template_bindings());

            if !fetch_resources_after_tool
                && !step.resources().is_empty()
                && self
                    .inner
                    .fetch_step_resources(step, &args, &execution_context, &extra, &mut messages)
                    .await
                    .is_err()
            {
                break;
            }

            // Handle resource-only steps
            if step.is_resource_only() {
                messages.push(PromptMessage::assistant(Content::text(format!(
                    "I'll fetch the required resources for {}...",
                    step.name()
                ))));

                if fetch_resources_after_tool
                    && self
                        .inner
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
                    break;
                }

                step_statuses[idx] = StepStatus::Completed;
                step_results.push((step.name().to_string(), Value::Null));
                continue;
            }

            // Tool steps: attempt to resolve parameters and execute
            match self
                .inner
                .create_tool_call_announcement(step, &args, &execution_context)
            {
                Err(_) => {
                    pause_reason = Some(classify_resolution_failure(
                        step,
                        self.workflow.steps(),
                        &step_statuses,
                    ));
                    break;
                },
                Ok(announcement) => {
                    let Ok(params) =
                        self.inner
                            .resolve_tool_parameters(step, &args, &execution_context)
                    else {
                        tracing::warn!(
                            "resolve_tool_parameters failed unexpectedly for step '{}' \
                             after announcement succeeded",
                            step.name()
                        );
                        pause_reason = Some(classify_resolution_failure(
                            step,
                            self.workflow.steps(),
                            &step_statuses,
                        ));
                        break;
                    };

                    match self.inner.params_satisfy_tool_schema(step, &params) {
                        Err(e) => {
                            tracing::warn!(
                                "params_satisfy_tool_schema error for step '{}': {}",
                                step.name(),
                                e
                            );
                            pause_reason = Some(PauseReason::UnresolvableParams {
                                blocked_step: step.name().to_string(),
                                missing_param: "unknown".to_string(),
                                suggested_tool: step
                                    .tool()
                                    .map(|t| t.name().to_string())
                                    .unwrap_or_default(),
                            });
                            break;
                        },
                        Ok(ref missing) if !missing.is_empty() => {
                            let suggested_tool = step
                                .tool()
                                .map(|t| t.name().to_string())
                                .unwrap_or_default();
                            pause_reason = Some(PauseReason::SchemaMismatch {
                                blocked_step: step.name().to_string(),
                                missing_fields: missing.clone(),
                                suggested_tool,
                            });
                            break;
                        },
                        Ok(_) => {
                            messages.push(announcement);

                            match self
                                .inner
                                .execute_tool_step(step, &args, &execution_context, &extra)
                                .await
                            {
                                Ok(result) => {
                                    messages.push(PromptMessage::user(Content::text(format!(
                                        "Tool result:\n{}",
                                        serde_json::to_string_pretty(&result)
                                            .unwrap_or_else(|_| format!("{:?}", result))
                                    ))));

                                    step_results.push((step.name().to_string(), result.clone()));
                                    step_statuses[idx] = StepStatus::Completed;

                                    if let Some(binding) = step.binding() {
                                        execution_context.store_binding(binding.clone(), result);
                                    }

                                    if fetch_resources_after_tool
                                        && self
                                            .inner
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
                                        break;
                                    }
                                },
                                Err(e) => {
                                    messages.push(PromptMessage::user(Content::text(format!(
                                        "Error executing tool: {}",
                                        e
                                    ))));

                                    let step_name = step.name().to_string();
                                    step_results.push((
                                        step_name.clone(),
                                        serde_json::json!({"error": e.to_string()}),
                                    ));
                                    step_statuses[idx] = StepStatus::Failed;

                                    let suggested_tool = step
                                        .tool()
                                        .map(|t| t.name().to_string())
                                        .unwrap_or_default();
                                    pause_reason = Some(PauseReason::ToolError {
                                        failed_step: step_name,
                                        error: e.to_string(),
                                        retryable: step.is_retryable(),
                                        suggested_tool,
                                    });
                                    break;
                                },
                            }
                        },
                    }
                },
            }
        }

        // 4b. Append handoff message when execution paused
        if let Some(ref reason) = pause_reason {
            let handoff =
                self.build_handoff_message(&step_statuses, reason, &args, &execution_context);
            messages.push(handoff);
        }

        // 5. Batch write to task store
        let updated_progress = build_updated_progress(&initial_progress, &step_statuses);

        let mut batch: HashMap<String, Value> = HashMap::new();
        batch.insert(WORKFLOW_PROGRESS_KEY.to_string(), updated_progress);

        for (step_name, result) in &step_results {
            batch.insert(workflow_result_key(step_name), result.clone());
        }

        if let Some(ref reason) = pause_reason {
            batch.insert(WORKFLOW_PAUSE_REASON_KEY.to_string(), reason.to_value());
        }

        let batch_value = serde_json::to_value(&batch).unwrap_or_else(|_| serde_json::json!({}));

        if let Err(e) = self
            .task_router
            .set_task_variables(&task_id, &owner_id, batch_value)
            .await
        {
            tracing::warn!(
                "Failed to batch-write task variables for workflow '{}': {}",
                self.workflow.name(),
                e
            );
        }

        // 6. Auto-complete if all steps succeeded
        let mut task_status = "working";
        let all_completed =
            pause_reason.is_none() && step_statuses.iter().all(|s| *s == StepStatus::Completed);

        if all_completed {
            let completion_result = serde_json::json!({
                "completed": true,
                "steps_completed": step_count,
            });

            match self
                .task_router
                .complete_workflow_task(&task_id, &owner_id, completion_result)
                .await
            {
                Ok(_) => {
                    task_status = "completed";
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to auto-complete task for workflow '{}': {}",
                        self.workflow.name(),
                        e
                    );
                },
            }
        }

        // 7. Build _meta and return
        let step_names: Vec<String> = self
            .workflow
            .steps()
            .iter()
            .map(|s| s.name().to_string())
            .collect();

        let meta = Self::build_meta_map(
            &task_id,
            task_status,
            &step_names,
            &step_statuses,
            pause_reason.as_ref(),
        );

        // Report final workflow completion
        let _ = extra
            .report_count(
                total_steps,
                total_steps,
                Some("Workflow execution complete".to_string()),
            )
            .await;

        let mut result = GetPromptResult {
            description: Some(self.workflow.description().to_string()),
            messages,
            _meta: None,
        };
        result._meta = Some(meta);

        Ok(result)
    }

    /// Delegate metadata to the inner handler unchanged.
    fn metadata(&self) -> Option<PromptInfo> {
        self.inner.metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_meta_map_working_with_pause_reason() {
        let step_names = vec!["validate".to_string(), "deploy".to_string()];
        let step_statuses = vec![StepStatus::Completed, StepStatus::Pending];
        let pause = PauseReason::ToolError {
            failed_step: "deploy".to_string(),
            error: "timeout".to_string(),
            retryable: true,
            suggested_tool: "deploy_service".to_string(),
        };

        let meta = TaskWorkflowPromptHandler::build_meta_map(
            "task-123",
            "working",
            &step_names,
            &step_statuses,
            Some(&pause),
        );

        assert_eq!(meta["task_id"], "task-123");
        assert_eq!(meta["task_status"], "working");

        let pr = meta.get("pause_reason").expect("should have pause_reason");
        assert_eq!(pr["type"], "toolError");
        assert_eq!(pr["failedStep"], "deploy");
        assert_eq!(pr["retryable"], true);

        let steps = meta["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["status"], "completed");
        assert_eq!(steps[1]["status"], "pending");
    }

    #[test]
    fn build_meta_map_completed_no_pause_reason() {
        let step_names = vec![
            "validate".to_string(),
            "deploy".to_string(),
            "notify".to_string(),
        ];
        let step_statuses = vec![
            StepStatus::Completed,
            StepStatus::Completed,
            StepStatus::Completed,
        ];

        let meta = TaskWorkflowPromptHandler::build_meta_map(
            "task-456",
            "completed",
            &step_names,
            &step_statuses,
            None,
        );

        assert_eq!(meta["task_id"], "task-456");
        assert_eq!(meta["task_status"], "completed");
        assert!(
            meta.get("pause_reason").is_none(),
            "completed task should not have pause_reason"
        );

        let steps = meta["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        for step in steps {
            assert_eq!(step["status"], "completed");
        }
    }

    #[test]
    fn build_meta_map_with_step_statuses() {
        let step_names = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let step_statuses = vec![
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Skipped,
            StepStatus::Pending,
        ];

        let meta = TaskWorkflowPromptHandler::build_meta_map(
            "task-789",
            "working",
            &step_names,
            &step_statuses,
            None,
        );

        let steps = meta["steps"].as_array().unwrap();
        assert_eq!(steps[0]["status"], "completed");
        assert_eq!(steps[1]["status"], "failed");
        assert_eq!(steps[2]["status"], "skipped");
        assert_eq!(steps[3]["status"], "pending");
    }

    #[test]
    fn build_meta_map_empty_steps() {
        let meta = TaskWorkflowPromptHandler::build_meta_map("task-000", "working", &[], &[], None);

        assert_eq!(meta["task_id"], "task-000");
        assert_eq!(meta["task_status"], "working");

        let steps = meta["steps"].as_array().expect("steps should be an array");
        assert!(steps.is_empty());
    }

    #[test]
    fn build_initial_progress_typed_all_pending() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("test_wf", "Test workflow")
            .step(WorkflowStep::new("validate", ToolHandle::new("checker")))
            .step(WorkflowStep::new("deploy", ToolHandle::new("deployer")))
            .step(
                WorkflowStep::fetch_resources("read_guide")
                    .with_resource("docs://guide")
                    .expect("valid URI"),
            );

        let inner = WorkflowPromptHandler::new(
            SequentialWorkflow::new("dummy", "dummy"),
            HashMap::new(),
            HashMap::new(),
            None,
        );
        let task_router: Arc<dyn TaskRouter> = Arc::new(DummyTaskRouter);
        let handler = TaskWorkflowPromptHandler::new(inner, task_router, workflow);

        let progress = handler.build_initial_progress_typed();

        assert_eq!(progress["goal"], "test_wf: Test workflow");
        assert_eq!(progress["schemaVersion"], 1);

        let steps = progress["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);

        // All steps should be Pending
        for step in steps {
            assert_eq!(step["status"], "pending");
        }

        // Verify step names and tools
        assert_eq!(steps[0]["name"], "validate");
        assert_eq!(steps[0]["tool"], "checker");
        assert_eq!(steps[1]["name"], "deploy");
        assert_eq!(steps[1]["tool"], "deployer");
        assert_eq!(steps[2]["name"], "read_guide");
        assert!(
            steps[2].get("tool").is_none(),
            "resource-only step should not have tool"
        );
    }

    #[test]
    fn classify_resolution_failure_unresolved_dependency() {
        use super::super::handles::ToolHandle;

        let step_a = WorkflowStep::new("produce", ToolHandle::new("fetcher")).bind("data_out");
        let step_b = WorkflowStep::new("consume", ToolHandle::new("processor"))
            .arg("input", DataSource::from_step("data_out"));

        let all_steps = vec![step_a, step_b.clone()];
        let statuses = vec![StepStatus::Failed, StepStatus::Pending];

        let result = classify_resolution_failure(&step_b, &all_steps, &statuses);

        match result {
            PauseReason::UnresolvedDependency {
                blocked_step,
                missing_output,
                producing_step,
                suggested_tool,
            } => {
                assert_eq!(blocked_step, "consume");
                assert_eq!(missing_output, "data_out");
                assert_eq!(producing_step, "produce");
                assert_eq!(suggested_tool, "fetcher");
            },
            other => panic!("Expected UnresolvedDependency, got: {:?}", other),
        }
    }

    #[test]
    fn classify_resolution_failure_generic_unresolvable() {
        use super::super::handles::ToolHandle;

        let step = WorkflowStep::new("do_thing", ToolHandle::new("tool_x"))
            .arg("name", DataSource::prompt_arg("missing_arg"));

        let all_steps = vec![step.clone()];
        let statuses = vec![StepStatus::Pending];

        let result = classify_resolution_failure(&step, &all_steps, &statuses);

        match result {
            PauseReason::UnresolvableParams {
                blocked_step,
                missing_param,
                suggested_tool,
            } => {
                assert_eq!(blocked_step, "do_thing");
                assert_eq!(missing_param, "name");
                assert_eq!(suggested_tool, "tool_x");
            },
            other => panic!("Expected UnresolvableParams, got: {:?}", other),
        }
    }

    #[test]
    fn classify_resolution_failure_skipped_producer() {
        use super::super::handles::ToolHandle;

        let step_a = WorkflowStep::new("gather", ToolHandle::new("gatherer")).bind("info");
        let step_b = WorkflowStep::new("analyze", ToolHandle::new("analyzer"))
            .arg("data", DataSource::from_step("info"));

        let all_steps = vec![step_a, step_b.clone()];
        let statuses = vec![StepStatus::Skipped, StepStatus::Pending];

        let result = classify_resolution_failure(&step_b, &all_steps, &statuses);

        match result {
            PauseReason::UnresolvedDependency {
                blocked_step,
                missing_output,
                producing_step,
                suggested_tool,
            } => {
                assert_eq!(blocked_step, "analyze");
                assert_eq!(missing_output, "info");
                assert_eq!(producing_step, "gather");
                assert_eq!(suggested_tool, "gatherer");
            },
            other => panic!("Expected UnresolvedDependency, got: {:?}", other),
        }
    }

    #[test]
    fn pause_reason_to_value_all_variants() {
        // UnresolvableParams
        let reason = PauseReason::UnresolvableParams {
            blocked_step: "step_a".to_string(),
            missing_param: "param_x".to_string(),
            suggested_tool: "tool_y".to_string(),
        };
        let val = reason.to_value();
        assert_eq!(val["type"], "unresolvableParams");
        assert_eq!(val["blockedStep"], "step_a");
        assert_eq!(val["missingParam"], "param_x");
        assert_eq!(val["suggestedTool"], "tool_y");

        // SchemaMismatch
        let reason = PauseReason::SchemaMismatch {
            blocked_step: "step_b".to_string(),
            missing_fields: vec!["f1".to_string(), "f2".to_string()],
            suggested_tool: "tool_z".to_string(),
        };
        let val = reason.to_value();
        assert_eq!(val["type"], "schemaMismatch");
        assert_eq!(val["blockedStep"], "step_b");
        assert_eq!(val["missingFields"], serde_json::json!(["f1", "f2"]));

        // ToolError
        let reason = PauseReason::ToolError {
            failed_step: "deploy".to_string(),
            error: "connection refused".to_string(),
            retryable: false,
            suggested_tool: "deploy_service".to_string(),
        };
        let val = reason.to_value();
        assert_eq!(val["type"], "toolError");
        assert_eq!(val["failedStep"], "deploy");
        assert_eq!(val["error"], "connection refused");
        assert_eq!(val["retryable"], false);

        // UnresolvedDependency
        let reason = PauseReason::UnresolvedDependency {
            blocked_step: "step_c".to_string(),
            missing_output: "data".to_string(),
            producing_step: "step_a".to_string(),
            suggested_tool: "fetch_data".to_string(),
        };
        let val = reason.to_value();
        assert_eq!(val["type"], "unresolvedDependency");
        assert_eq!(val["blockedStep"], "step_c");
        assert_eq!(val["missingOutput"], "data");
        assert_eq!(val["producingStep"], "step_a");
        assert_eq!(val["suggestedTool"], "fetch_data");
    }

    #[test]
    fn workflow_result_key_produces_correct_keys() {
        assert_eq!(workflow_result_key("validate"), "_workflow.result.validate");
        assert_eq!(workflow_result_key("deploy"), "_workflow.result.deploy");
    }

    #[test]
    fn build_updated_progress_applies_statuses() {
        let initial = serde_json::json!({
            "goal": "Test",
            "steps": [
                {"name": "a", "tool": "tool_a", "status": "pending"},
                {"name": "b", "tool": "tool_b", "status": "pending"},
                {"name": "c", "status": "pending"}
            ],
            "schemaVersion": 1
        });

        let statuses = vec![
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Pending,
        ];

        let updated = build_updated_progress(&initial, &statuses);

        let steps = updated["steps"].as_array().unwrap();
        assert_eq!(steps[0]["status"], "completed");
        assert_eq!(steps[1]["status"], "failed");
        assert_eq!(steps[2]["status"], "pending");

        // Goal and schemaVersion unchanged
        assert_eq!(updated["goal"], "Test");
        assert_eq!(updated["schemaVersion"], 1);
    }

    // --- Dummy TaskRouter for tests ---

    struct DummyTaskRouter;

    #[async_trait]
    impl TaskRouter for DummyTaskRouter {
        async fn handle_task_call(
            &self,
            _tool_name: &str,
            _arguments: Value,
            _task_params: Value,
            _owner_id: &str,
            _progress_token: Option<Value>,
        ) -> Result<Value> {
            unimplemented!()
        }

        async fn handle_tasks_get(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            unimplemented!()
        }

        async fn handle_tasks_result(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            unimplemented!()
        }

        async fn handle_tasks_list(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            unimplemented!()
        }

        async fn handle_tasks_cancel(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            unimplemented!()
        }

        fn resolve_owner(
            &self,
            _subject: Option<&str>,
            _client_id: Option<&str>,
            _session_id: Option<&str>,
        ) -> String {
            "owner".to_string()
        }

        fn tool_requires_task(&self, _tool_name: &str, _tool_execution: Option<&Value>) -> bool {
            false
        }

        fn task_capabilities(&self) -> Value {
            serde_json::json!({})
        }
    }

    /// Helper: build a handler with a given workflow for handoff tests.
    fn make_handler(workflow: SequentialWorkflow) -> TaskWorkflowPromptHandler {
        let inner =
            WorkflowPromptHandler::new(workflow.clone(), HashMap::new(), HashMap::new(), None);
        let task_router: Arc<dyn TaskRouter> = Arc::new(DummyTaskRouter);
        TaskWorkflowPromptHandler::new(inner, task_router, workflow)
    }

    // === Handoff message tests ===

    #[test]
    fn handoff_message_tool_error_retryable() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("deploy_wf", "Deploy workflow")
            .step(WorkflowStep::new("validate", ToolHandle::new("checker")))
            .step(
                WorkflowStep::new("deploy", ToolHandle::new("deploy_service"))
                    .arg("region", DataSource::prompt_arg("region"))
                    .retryable(true),
            )
            .step(
                WorkflowStep::new("notify", ToolHandle::new("notify_team"))
                    .arg("result", DataSource::from_step("deploy_out")),
            );

        let handler = make_handler(workflow);

        let step_statuses = vec![
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Pending,
        ];
        let pause = PauseReason::ToolError {
            failed_step: "deploy".to_string(),
            error: "connection timeout".to_string(),
            retryable: true,
            suggested_tool: "deploy_service".to_string(),
        };

        let mut args = HashMap::new();
        args.insert("region".to_string(), "us-east-1".to_string());

        let ctx = ExecutionContext::new();

        let msg = handler.build_handoff_message(&step_statuses, &pause, &args, &ctx);

        assert_eq!(msg.role, Role::Assistant);
        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        // Section 1: What happened
        assert!(
            text.contains("Step 'deploy' failed: connection timeout."),
            "should contain failure description"
        );
        assert!(
            text.contains("This step is retryable."),
            "should note retryable"
        );

        // Section 2: Remaining steps
        assert!(
            text.contains("To continue the workflow, make these tool calls:"),
            "should contain continuation header"
        );

        // The failed retryable step should appear as item 1
        assert!(
            text.contains("1. Call deploy_service with"),
            "retryable failed step should be first"
        );

        // The pending step should appear as item 2
        assert!(
            text.contains("2. Call notify_team with"),
            "pending step should follow"
        );
    }

    #[test]
    fn handoff_message_unresolvable_params() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("wf", "Workflow").step(
            WorkflowStep::new("step_a", ToolHandle::new("tool_a"))
                .arg("x", DataSource::prompt_arg("missing")),
        );

        let handler = make_handler(workflow);

        let step_statuses = vec![StepStatus::Pending];
        let pause = PauseReason::UnresolvableParams {
            blocked_step: "step_a".to_string(),
            missing_param: "x".to_string(),
            suggested_tool: "tool_a".to_string(),
        };

        let msg = handler.build_handoff_message(
            &step_statuses,
            &pause,
            &HashMap::new(),
            &ExecutionContext::new(),
        );

        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        assert!(
            text.contains("Could not resolve parameter 'x' for step 'step_a'."),
            "should describe unresolvable param. Got: {}",
            text
        );
        assert!(
            text.contains("1. Call tool_a with"),
            "should list remaining step"
        );
    }

    #[test]
    fn handoff_message_unresolved_dependency() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("wf", "Workflow")
            .step(WorkflowStep::new("produce", ToolHandle::new("fetcher")).bind("data"))
            .step(
                WorkflowStep::new("consume", ToolHandle::new("processor"))
                    .arg("input", DataSource::from_step("data")),
            );

        let handler = make_handler(workflow);

        let step_statuses = vec![StepStatus::Failed, StepStatus::Pending];
        let pause = PauseReason::UnresolvedDependency {
            blocked_step: "consume".to_string(),
            missing_output: "data".to_string(),
            producing_step: "produce".to_string(),
            suggested_tool: "fetcher".to_string(),
        };

        let msg = handler.build_handoff_message(
            &step_statuses,
            &pause,
            &HashMap::new(),
            &ExecutionContext::new(),
        );

        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        assert!(
            text.contains("Step 'consume' depends on output 'data' from step 'produce', which did not complete."),
            "should mention dependency. Got: {}",
            text
        );
        assert!(text.contains("produce"), "should mention producing step");
    }

    #[test]
    fn handoff_message_schema_mismatch() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("wf", "Workflow")
            .step(WorkflowStep::new("step_b", ToolHandle::new("tool_z")));

        let handler = make_handler(workflow);

        let step_statuses = vec![StepStatus::Pending];
        let pause = PauseReason::SchemaMismatch {
            blocked_step: "step_b".to_string(),
            missing_fields: vec!["field_1".to_string(), "field_2".to_string()],
            suggested_tool: "tool_z".to_string(),
        };

        let msg = handler.build_handoff_message(
            &step_statuses,
            &pause,
            &HashMap::new(),
            &ExecutionContext::new(),
        );

        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        assert!(
            text.contains("Step 'step_b' has missing required fields: field_1, field_2."),
            "should list missing fields. Got: {}",
            text
        );
    }

    #[test]
    fn handoff_message_no_task_id_in_text() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("wf", "Workflow")
            .step(WorkflowStep::new("s1", ToolHandle::new("t1")))
            .step(WorkflowStep::new("s2", ToolHandle::new("t2")));

        let handler = make_handler(workflow);

        let step_statuses = vec![StepStatus::Completed, StepStatus::Pending];
        let pause = PauseReason::ToolError {
            failed_step: "s1".to_string(),
            error: "oops".to_string(),
            retryable: false,
            suggested_tool: "t1".to_string(),
        };

        let msg = handler.build_handoff_message(
            &step_statuses,
            &pause,
            &HashMap::new(),
            &ExecutionContext::new(),
        );

        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        // The narrative should never contain "task_id", "task-", or any UUID-like pattern
        assert!(
            !text.contains("task_id"),
            "narrative should not contain task_id"
        );
        assert!(
            !text.contains("task-"),
            "narrative should not contain task- prefix"
        );
    }

    #[test]
    fn placeholder_args_step_output() {
        use super::super::handles::ToolHandle;

        let step = WorkflowStep::new("consume", ToolHandle::new("processor"))
            .arg("data", DataSource::from_step("result_binding"))
            .arg("name", DataSource::prompt_arg("user_name"))
            .arg("flag", DataSource::constant(serde_json::json!(true)))
            .arg(
                "detail",
                DataSource::from_step_field("other_binding", "nested_field"),
            );

        let mut args = HashMap::new();
        args.insert("user_name".to_string(), "Alice".to_string());

        let result = TaskWorkflowPromptHandler::build_placeholder_args(&step, &args);

        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        let obj = parsed.as_object().expect("should be an object");

        assert_eq!(
            obj["data"], "<output from result_binding>",
            "StepOutput without field should use placeholder"
        );
        assert_eq!(
            obj["name"], "Alice",
            "PromptArg with available value should resolve"
        );
        assert_eq!(obj["flag"], true, "Constant should serialize as-is");
        assert_eq!(
            obj["detail"], "<field 'nested_field' from other_binding>",
            "StepOutput with field should use field placeholder"
        );
    }

    #[test]
    fn handoff_includes_guidance() {
        use super::super::handles::ToolHandle;

        let workflow = SequentialWorkflow::new("wf", "Workflow")
            .step(WorkflowStep::new("validate", ToolHandle::new("checker")))
            .step(
                WorkflowStep::new("deploy", ToolHandle::new("deploy_service"))
                    .arg("region", DataSource::prompt_arg("region"))
                    .with_guidance("Deploy to the '{region}' region with validated config"),
            );

        let handler = make_handler(workflow);

        let step_statuses = vec![StepStatus::Completed, StepStatus::Pending];
        let pause = PauseReason::ToolError {
            failed_step: "validate".to_string(),
            error: "check failed".to_string(),
            retryable: false,
            suggested_tool: "checker".to_string(),
        };

        let mut args = HashMap::new();
        args.insert("region".to_string(), "us-west-2".to_string());

        let msg =
            handler.build_handoff_message(&step_statuses, &pause, &args, &ExecutionContext::new());

        let text = match &msg.content {
            Content::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        };

        assert!(
            text.contains("Note: Deploy to the 'us-west-2' region with validated config"),
            "should include guidance with substituted args. Got: {}",
            text
        );
    }
}
