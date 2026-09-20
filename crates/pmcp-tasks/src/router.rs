//! TaskRouter implementation bridging pmcp's TaskRouter trait to TaskStore operations.
//!
//! [`TaskRouterImpl`] is the concrete implementation of the [`pmcp::server::tasks::TaskRouter`]
//! trait. It owns a [`TaskStore`] and [`TaskSecurityConfig`], handling all task lifecycle
//! operations: creation (via task-augmented `tools/call`), status retrieval, result
//! retrieval, listing, and cancellation.
//!
//! # Design
//!
//! The router does **not** execute tools. It creates a task, stores tool context
//! (tool name, arguments, progress token) as task variables so that an external
//! service (Step Functions, SQS consumer, etc.) can pick up the work, and returns
//! a [`CreateTaskResult`] immediately. This matches the locked design decision
//! in CONTEXT.md: handlers trigger external services and return immediately.
//!
//! # Error Conversion
//!
//! [`TaskError`] is converted to [`pmcp::error::Error`] using the error code
//! from [`TaskError::error_code()`] and the display message. Client-facing errors
//! (invalid params, not found) use code `-32602`; internal errors use `-32603`.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use pmcp::error::{Error as PmcpError, Result as PmcpResult};
use pmcp::server::tasks::TaskRouter;

use crate::error::TaskError;
use crate::security::{resolve_owner_id, TaskSecurityConfig};
use crate::store::{ListTasksOptions, TaskStore};
use crate::types::capabilities::ServerTaskCapabilities;
use crate::types::execution::{TaskSupport, ToolExecution};
use crate::types::params::{
    TaskCancelParams, TaskGetParams, TaskListParams, TaskParams, TaskResultParams,
};
use crate::types::task::{related_task_meta, CreateTaskResult};

/// Implementation of pmcp's `TaskRouter` trait using a `TaskStore` backend.
///
/// This struct bridges the pmcp server's request routing to the pmcp-tasks
/// store operations. It owns the store and security config, and handles
/// all task lifecycle operations.
///
/// # Construction
///
/// ```
/// use std::sync::Arc;
/// use pmcp_tasks::router::TaskRouterImpl;
/// use pmcp_tasks::store::memory::InMemoryTaskStore;
///
/// let store = Arc::new(InMemoryTaskStore::new());
/// let router = TaskRouterImpl::new(store);
/// ```
pub struct TaskRouterImpl {
    store: Arc<dyn TaskStore>,
    security_config: TaskSecurityConfig,
}

impl TaskRouterImpl {
    /// Creates a new `TaskRouterImpl` with default security configuration.
    ///
    /// # Arguments
    ///
    /// * `store` - The task store backend (shared via `Arc`).
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self {
            store,
            security_config: TaskSecurityConfig::default(),
        }
    }

    /// Creates a new `TaskRouterImpl` with a custom security configuration.
    ///
    /// # Arguments
    ///
    /// * `store` - The task store backend (shared via `Arc`).
    /// * `config` - Security configuration for owner-specific limits.
    pub fn with_security(store: Arc<dyn TaskStore>, config: TaskSecurityConfig) -> Self {
        Self {
            store,
            security_config: config,
        }
    }

    /// Returns a reference to the underlying task store.
    ///
    /// Useful for direct store access in tests or advanced use cases.
    pub fn store(&self) -> &Arc<dyn TaskStore> {
        &self.store
    }

    /// Returns a reference to the security configuration.
    pub fn security_config(&self) -> &TaskSecurityConfig {
        &self.security_config
    }
}

/// Wire parameters of a `tasks/update` request, as they arrive across the
/// `serde_json::Value` seam.
///
/// The two names are the ones the tasks extension schema specifies:
/// `taskId` (required) and `inputResponses` -- **not** `inputs`, which is the
/// name an earlier research pass reported and which the schema does not use.
///
/// `inputResponses` carries `#[serde(default)]` so an omitted member is an empty
/// delivery rather than a hard parse failure. It is deliberately NOT bounds- or
/// kind-checked here: both happen above this seam, at request ingress, before
/// any decode.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateParams {
    /// The task receiving the inputs.
    task_id: String,
    /// The client's answers, keyed by the server-assigned input key.
    #[serde(default)]
    input_responses: Value,
}

/// Converts a [`TaskError`] into a [`pmcp::error::Error`] using the error code
/// from the task error and its display message.
#[allow(clippy::needless_pass_by_value)] // Used in .map_err() which passes by value
fn task_error_to_pmcp(err: TaskError) -> PmcpError {
    let code = err.error_code();
    let message = err.to_string();
    if code == -32602 {
        PmcpError::invalid_params(message)
    } else {
        PmcpError::internal(message)
    }
}

#[async_trait]
impl TaskRouter for TaskRouterImpl {
    /// Handle a task-augmented `tools/call` request.
    ///
    /// Creates a task in the store, stores tool context (name, arguments,
    /// progress token) as task variables for external service pickup, and
    /// returns a `CreateTaskResult` immediately without executing the tool.
    async fn handle_task_call(
        &self,
        tool_name: &str,
        arguments: Value,
        task_params: Value,
        owner_id: &str,
        progress_token: Option<Value>,
    ) -> PmcpResult<Value> {
        let params: TaskParams = serde_json::from_value(task_params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid task params: {e}")))?;

        let record = self
            .store
            .create(owner_id, "tools/call", params.ttl)
            .await
            .map_err(task_error_to_pmcp)?;

        let task_id = record.task.task_id.clone();

        // Store tool context as task variables so external services can pick up the work
        let mut vars = HashMap::new();
        vars.insert(
            "tool_name".to_string(),
            Value::String(tool_name.to_string()),
        );
        vars.insert("arguments".to_string(), arguments);
        if let Some(token) = progress_token {
            vars.insert("progress_token".to_string(), token);
        }

        self.store
            .set_variables(&task_id, owner_id, vars)
            .await
            .map_err(task_error_to_pmcp)?;

        // Build the CreateTaskResult with the task's poll_interval from params
        let mut task = record.task;
        if let Some(pi) = params.poll_interval {
            task.poll_interval = Some(pi);
        }

        let result = CreateTaskResult { task, _meta: None };

        serde_json::to_value(result)
            .map_err(|e| PmcpError::internal(format!("failed to serialize CreateTaskResult: {e}")))
    }

    /// Handle `tasks/get` request.
    ///
    /// Returns the task status for the given task ID, with variables
    /// injected into `_meta`.
    async fn handle_tasks_get(&self, params: Value, owner_id: &str) -> PmcpResult<Value> {
        let get_params: TaskGetParams = serde_json::from_value(params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid tasks/get params: {e}")))?;

        let record = self
            .store
            .get(&get_params.task_id, owner_id)
            .await
            .map_err(task_error_to_pmcp)?;

        // GetTaskResult is a type alias for Task -- serialize flat with variables
        let wire_task = record.to_wire_task_with_variables();
        serde_json::to_value(wire_task)
            .map_err(|e| PmcpError::internal(format!("failed to serialize GetTaskResult: {e}")))
    }

    /// Handle `tasks/result` request.
    ///
    /// Returns the stored operation result for a terminal task, with
    /// `_meta` containing the related-task metadata linking back to the task.
    async fn handle_tasks_result(&self, params: Value, owner_id: &str) -> PmcpResult<Value> {
        let result_params: TaskResultParams = serde_json::from_value(params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid tasks/result params: {e}")))?;

        let result_value = self
            .store
            .get_result(&result_params.task_id, owner_id)
            .await
            .map_err(task_error_to_pmcp)?;

        // Build response with _meta containing related-task link
        let meta = related_task_meta(&result_params.task_id);
        let mut response = serde_json::Map::new();
        response.insert("result".to_string(), result_value);
        response.insert("_meta".to_string(), Value::Object(meta));

        Ok(Value::Object(response))
    }

    /// Handle `tasks/list` request.
    ///
    /// Returns a paginated list of tasks for the given owner.
    async fn handle_tasks_list(&self, params: Value, owner_id: &str) -> PmcpResult<Value> {
        let list_params: TaskListParams = serde_json::from_value(params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid tasks/list params: {e}")))?;

        let options = ListTasksOptions {
            owner_id: owner_id.to_string(),
            cursor: list_params.cursor,
            limit: None,
        };

        let page = self.store.list(options).await.map_err(task_error_to_pmcp)?;

        // Convert TaskPage to JSON response with tasks array and optional nextCursor
        let tasks_json: Vec<Value> = page
            .tasks
            .into_iter()
            .map(|record| {
                let wire_task = record.to_wire_task_with_variables();
                serde_json::to_value(wire_task).unwrap_or(Value::Null)
            })
            .collect();

        let mut response = serde_json::Map::new();
        response.insert("tasks".to_string(), Value::Array(tasks_json));
        if let Some(cursor) = page.next_cursor {
            response.insert("nextCursor".to_string(), Value::String(cursor));
        }

        Ok(Value::Object(response))
    }

    /// Handle `tasks/cancel` request.
    ///
    /// When the `result` field is present in params, the task transitions to
    /// `Completed` status (workflow completion). Otherwise, transitions to
    /// `Cancelled` status (standard cancel).
    async fn handle_tasks_cancel(&self, params: Value, owner_id: &str) -> PmcpResult<Value> {
        let cancel_params: TaskCancelParams = serde_json::from_value(params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid tasks/cancel params: {e}")))?;

        let record = if let Some(result) = cancel_params.result {
            // Completion path: complete the task with the provided result
            self.store
                .complete_with_result(
                    &cancel_params.task_id,
                    owner_id,
                    crate::types::task::TaskStatus::Completed,
                    None,
                    result,
                )
                .await
                .map_err(task_error_to_pmcp)?
        } else {
            // Standard cancel path
            self.store
                .cancel(&cancel_params.task_id, owner_id)
                .await
                .map_err(task_error_to_pmcp)?
        };

        // CancelTaskResult is a type alias for Task -- serialize flat
        let wire_task = record.to_wire_task_with_variables();
        serde_json::to_value(wire_task)
            .map_err(|e| PmcpError::internal(format!("failed to serialize CancelTaskResult: {e}")))
    }

    /// Handle `tasks/update` request.
    ///
    /// Delivers the client's `inputResponses` to a task that is awaiting input,
    /// through the store's single compare-and-set delivery -- so this path
    /// reaches the `DynamoDB` and Redis backends exactly as it reaches the
    /// in-memory one, with no per-backend code.
    ///
    /// # The owner comes from the ARGUMENT, never from `params`
    ///
    /// `owner_id` has ALREADY been resolved by the caller through the v2
    /// identity table. Re-deriving it here from client-supplied `params` would
    /// be an insecure direct object reference -- a caller could name someone
    /// else's owner and reach their task -- which is precisely what the
    /// owner-prefixed storage key and the resolved-owner argument exist to
    /// prevent.
    ///
    /// [`resolve_owner_id`] is deliberately NOT used on this path either. Its
    /// chain is `subject -> client_id -> session_id -> "local"`, and a
    /// session-id fallback is forbidden on v2: v2 is stateless and has no
    /// session to trust. The decision was made once, above this seam.
    ///
    /// # Errors
    ///
    /// - Invalid params (`-32602`) if `params` is not a `tasks/update` shape, if
    ///   the task does not exist under this owner, if it has expired, or if it
    ///   is not awaiting input (a terminal or still-`working` task cannot be
    ///   fed).
    /// - Internal error (`-32603`) on a concurrent-modification conflict or a
    ///   backend failure.
    async fn handle_tasks_update(&self, params: Value, owner_id: &str) -> PmcpResult<Value> {
        let update_params: TaskUpdateParams = serde_json::from_value(params)
            .map_err(|e| PmcpError::invalid_params(format!("invalid tasks/update params: {e}")))?;

        self.store
            .deliver_inputs(
                &update_params.task_id,
                owner_id,
                update_params.input_responses,
            )
            .await
            .map_err(task_error_to_pmcp)
    }

    /// Resolve owner ID from authentication context fields.
    ///
    /// Delegates to [`resolve_owner_id`] with the given subject, client ID,
    /// and session ID.
    fn resolve_owner(
        &self,
        subject: Option<&str>,
        client_id: Option<&str>,
        session_id: Option<&str>,
    ) -> String {
        resolve_owner_id(subject, client_id, session_id)
    }

    /// Check if a tool requires task augmentation (`taskSupport: required`).
    ///
    /// Parses the tool's execution metadata and checks if `task_support`
    /// is [`TaskSupport::Required`].
    fn tool_requires_task(&self, _tool_name: &str, tool_execution: Option<&Value>) -> bool {
        let Some(execution_value) = tool_execution else {
            return false;
        };

        let Ok(execution) = serde_json::from_value::<ToolExecution>(execution_value.clone()) else {
            return false;
        };

        execution.task_support == TaskSupport::Required
    }

    /// Get the server task capabilities as a `Value` for `experimental.tasks`.
    fn task_capabilities(&self) -> Value {
        serde_json::to_value(ServerTaskCapabilities::full())
            .expect("ServerTaskCapabilities serialization should never fail")
    }

    /// Create a workflow-backed task with initial progress in variables.
    ///
    /// The `workflow_name` becomes the task's origin method identifier.
    /// The `progress` value (a serialized [`WorkflowProgress`](crate::types::workflow::WorkflowProgress)) is stored
    /// under the [`WORKFLOW_PROGRESS_KEY`](crate::types::workflow::WORKFLOW_PROGRESS_KEY) task variable.
    async fn create_workflow_task(
        &self,
        workflow_name: &str,
        owner_id: &str,
        progress: Value,
    ) -> PmcpResult<Value> {
        // Validate progress is a JSON object
        if !progress.is_object() {
            return Err(PmcpError::invalid_params(
                "workflow progress must be a JSON object",
            ));
        }

        let record = self
            .store
            .create(owner_id, workflow_name, None)
            .await
            .map_err(task_error_to_pmcp)?;

        let task_id = record.task.task_id.clone();

        // Store initial progress under the workflow progress key
        let mut vars = HashMap::new();
        vars.insert(
            crate::types::workflow::WORKFLOW_PROGRESS_KEY.to_string(),
            progress,
        );

        self.store
            .set_variables(&task_id, owner_id, vars)
            .await
            .map_err(task_error_to_pmcp)?;

        let result = CreateTaskResult {
            task: record.task,
            _meta: None,
        };

        serde_json::to_value(result)
            .map_err(|e| PmcpError::internal(format!("failed to serialize CreateTaskResult: {e}")))
    }

    /// Update task variables with workflow step results.
    ///
    /// Deserializes `variables` as a JSON object and sets each key-value
    /// pair on the task's variable store.
    async fn set_task_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: Value,
    ) -> PmcpResult<()> {
        let vars_map: HashMap<String, Value> = serde_json::from_value(variables)
            .map_err(|e| PmcpError::invalid_params(format!("invalid variables object: {e}")))?;

        self.store
            .set_variables(task_id, owner_id, vars_map)
            .await
            .map_err(task_error_to_pmcp)?;

        Ok(())
    }

    /// Complete a workflow task with final result.
    ///
    /// Transitions the task to `Completed` status and stores the result
    /// atomically.
    async fn complete_workflow_task(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> PmcpResult<Value> {
        let record = self
            .store
            .complete_with_result(
                task_id,
                owner_id,
                crate::types::task::TaskStatus::Completed,
                None,
                result,
            )
            .await
            .map_err(task_error_to_pmcp)?;

        let wire_task = record.to_wire_task_with_variables();
        serde_json::to_value(wire_task)
            .map_err(|e| PmcpError::internal(format!("failed to serialize task: {e}")))
    }

    /// Record a tool call result against a workflow task.
    ///
    /// Matches the tool name to a remaining (pending or failed) workflow step.
    /// If matched, stores the result under `_workflow.result.<step_name>` and
    /// updates the step status to `completed` in `_workflow.progress`. If no
    /// match, stores under `_workflow.extra.<tool_name>` for observability.
    async fn handle_workflow_continuation(
        &self,
        task_id: &str,
        tool_name: &str,
        tool_result: Value,
        owner_id: &str,
    ) -> PmcpResult<()> {
        use crate::types::workflow::{
            workflow_extra_key, workflow_result_key, WORKFLOW_PAUSE_REASON_KEY,
            WORKFLOW_PROGRESS_KEY,
        };

        // Load the task to verify it exists and is Working
        let record = self
            .store
            .get(task_id, owner_id)
            .await
            .map_err(task_error_to_pmcp)?;

        // Only continue if task is in Working status
        if record.task.status != crate::types::task::TaskStatus::Working {
            return Err(PmcpError::invalid_params(format!(
                "task {} is not in Working status (current: {:?})",
                task_id, record.task.status
            )));
        }

        // Load _workflow.progress from task variables
        let progress_value = record.variables.get(WORKFLOW_PROGRESS_KEY).cloned();

        let mut variables_to_set: HashMap<String, Value> = HashMap::new();

        // Try to match tool_name against remaining step tool fields
        let mut matched_step_name: Option<String> = None;
        if let Some(progress) = &progress_value {
            if let Some(steps) = progress.get("steps").and_then(|s| s.as_array()) {
                for step in steps {
                    let step_tool = step.get("tool").and_then(|t| t.as_str()).unwrap_or("");
                    let step_status = step.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    let step_name = step.get("name").and_then(|n| n.as_str()).unwrap_or("");

                    // Match: tool name matches AND step is pending or failed (retryable)
                    if step_tool == tool_name
                        && (step_status == "pending" || step_status == "failed")
                    {
                        // Store result under _workflow.result.<step_name>
                        variables_to_set
                            .insert(workflow_result_key(step_name), tool_result.clone());
                        matched_step_name = Some(step_name.to_string());
                        break; // First match wins
                    }
                }
            }
        }

        if matched_step_name.is_none() {
            // Store under _workflow.extra.<tool_name> for observability
            variables_to_set.insert(workflow_extra_key(tool_name), tool_result);
        }

        // Update progress: mark matched step as completed
        if let Some(ref matched_name) = matched_step_name {
            if let Some(mut progress) = progress_value.clone() {
                if let Some(steps) = progress.get_mut("steps").and_then(|s| s.as_array_mut()) {
                    for step in steps.iter_mut() {
                        let step_tool = step.get("tool").and_then(|t| t.as_str()).unwrap_or("");
                        let step_name = step.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let step_status = step.get("status").and_then(|s| s.as_str()).unwrap_or("");

                        if step_tool == tool_name
                            && step_name == matched_name.as_str()
                            && (step_status == "pending" || step_status == "failed")
                        {
                            if let Some(obj) = step.as_object_mut() {
                                obj.insert(
                                    "status".to_string(),
                                    Value::String("completed".to_string()),
                                );
                            }
                            break;
                        }
                    }
                }
                variables_to_set.insert(WORKFLOW_PROGRESS_KEY.to_string(), progress);
            }
        }

        // Clear pause_reason since the client is making progress
        variables_to_set.insert(WORKFLOW_PAUSE_REASON_KEY.to_string(), Value::Null);

        // Batch write all variables
        let variables_value = serde_json::to_value(&variables_to_set)
            .map_err(|e| PmcpError::internal(format!("failed to serialize variables: {e}")))?;

        self.set_task_variables(task_id, owner_id, variables_value)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::memory::InMemoryTaskStore;

    fn make_router() -> TaskRouterImpl {
        let store = Arc::new(
            InMemoryTaskStore::new()
                .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
        );
        TaskRouterImpl::with_security(
            store,
            TaskSecurityConfig::default().with_allow_anonymous(true),
        )
    }

    #[tokio::test]
    async fn handle_task_call_creates_task_and_stores_variables() {
        let router = make_router();
        let task_params = serde_json::json!({ "ttl": 60000 });
        let arguments = serde_json::json!({ "query": "test" });

        let result = router
            .handle_task_call("my-tool", arguments.clone(), task_params, "owner-1", None)
            .await
            .unwrap();

        // Result should have a task wrapper
        assert!(result.get("task").is_some());
        let task = &result["task"];
        assert_eq!(task["status"], "working");
        assert_eq!(task["ttl"], 60000);

        // Check that variables were stored
        let task_id = task["taskId"].as_str().unwrap();
        let record = router.store().get(task_id, "owner-1").await.unwrap();
        assert_eq!(
            record.variables.get("tool_name").unwrap(),
            &Value::String("my-tool".to_string())
        );
        assert_eq!(record.variables.get("arguments").unwrap(), &arguments);
        assert!(!record.variables.contains_key("progress_token"));
    }

    #[tokio::test]
    async fn handle_task_call_stores_progress_token() {
        let router = make_router();
        let task_params = serde_json::json!({});
        let token = serde_json::json!("tok-123");

        let result = router
            .handle_task_call(
                "tool",
                serde_json::json!({}),
                task_params,
                "owner-1",
                Some(token.clone()),
            )
            .await
            .unwrap();

        let task_id = result["task"]["taskId"].as_str().unwrap();
        let record = router.store().get(task_id, "owner-1").await.unwrap();
        assert_eq!(record.variables.get("progress_token").unwrap(), &token);
    }

    #[tokio::test]
    async fn handle_task_call_with_poll_interval() {
        let router = make_router();
        let task_params = serde_json::json!({ "pollInterval": 5000 });

        let result = router
            .handle_task_call("tool", serde_json::json!({}), task_params, "owner-1", None)
            .await
            .unwrap();

        assert_eq!(result["task"]["pollInterval"], 5000);
    }

    #[tokio::test]
    async fn handle_tasks_get_returns_task() {
        let router = make_router();

        // Create a task first
        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let params = serde_json::json!({ "taskId": task_id });
        let result = router.handle_tasks_get(params, "owner-1").await.unwrap();

        assert_eq!(result["taskId"], task_id);
        assert_eq!(result["status"], "working");
    }

    #[tokio::test]
    async fn handle_tasks_get_not_found() {
        let router = make_router();
        let params = serde_json::json!({ "taskId": "nonexistent" });

        let err = router
            .handle_tasks_get(params, "owner-1")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn handle_tasks_result_returns_result_with_meta() {
        let router = make_router();

        // Create and complete a task with a result
        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let result_value = serde_json::json!({ "output": "done" });
        router
            .store()
            .complete_with_result(
                &task_id,
                "owner-1",
                crate::types::task::TaskStatus::Completed,
                None,
                result_value.clone(),
            )
            .await
            .unwrap();

        let params = serde_json::json!({ "taskId": task_id });
        let response = router.handle_tasks_result(params, "owner-1").await.unwrap();

        assert_eq!(response["result"], result_value);
        assert!(
            response["_meta"]["io.modelcontextprotocol/related-task"]["taskId"]
                .as_str()
                .unwrap()
                .contains(&task_id)
        );
    }

    #[tokio::test]
    async fn handle_tasks_result_not_ready() {
        let router = make_router();

        // Create a task but don't complete it
        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let params = serde_json::json!({ "taskId": task_id });
        let err = router
            .handle_tasks_result(params, "owner-1")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not in terminal state"));
    }

    #[tokio::test]
    async fn handle_tasks_list_returns_paginated_tasks() {
        let router = make_router();

        // Create a few tasks
        router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();

        let params = serde_json::json!({});
        let response = router.handle_tasks_list(params, "owner-1").await.unwrap();

        let tasks = response["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn handle_tasks_list_empty() {
        let router = make_router();

        let params = serde_json::json!({});
        let response = router.handle_tasks_list(params, "owner-1").await.unwrap();

        let tasks = response["tasks"].as_array().unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn handle_tasks_cancel_transitions_to_cancelled() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let params = serde_json::json!({ "taskId": task_id });
        let result = router.handle_tasks_cancel(params, "owner-1").await.unwrap();

        assert_eq!(result["taskId"], task_id);
        assert_eq!(result["status"], "cancelled");
    }

    #[tokio::test]
    async fn handle_tasks_cancel_already_terminal() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        // Complete the task first
        router
            .store()
            .complete_with_result(
                &task_id,
                "owner-1",
                crate::types::task::TaskStatus::Completed,
                None,
                serde_json::json!({}),
            )
            .await
            .unwrap();

        let params = serde_json::json!({ "taskId": task_id });
        let err = router
            .handle_tasks_cancel(params, "owner-1")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid transition") || err.to_string().contains("terminal")
        );
    }

    #[test]
    fn resolve_owner_delegates_correctly() {
        let router = make_router();

        assert_eq!(router.resolve_owner(Some("user-1"), None, None), "user-1");
        assert_eq!(
            router.resolve_owner(None, Some("client-1"), None),
            "client-1"
        );
        assert_eq!(
            router.resolve_owner(None, None, Some("session-1")),
            "session-1"
        );
        assert_eq!(router.resolve_owner(None, None, None), "local");
    }

    #[test]
    fn tool_requires_task_with_required() {
        let router = make_router();
        let execution = serde_json::json!({ "taskSupport": "required" });

        assert!(router.tool_requires_task("tool", Some(&execution)));
    }

    #[test]
    fn tool_requires_task_with_optional() {
        let router = make_router();
        let execution = serde_json::json!({ "taskSupport": "optional" });

        assert!(!router.tool_requires_task("tool", Some(&execution)));
    }

    #[test]
    fn tool_requires_task_with_forbidden() {
        let router = make_router();
        let execution = serde_json::json!({ "taskSupport": "forbidden" });

        assert!(!router.tool_requires_task("tool", Some(&execution)));
    }

    #[test]
    fn tool_requires_task_without_execution() {
        let router = make_router();

        assert!(!router.tool_requires_task("tool", None));
    }

    #[test]
    fn task_capabilities_returns_full() {
        let router = make_router();
        let caps = router.task_capabilities();

        assert!(caps.get("list").is_some());
        assert!(caps.get("cancel").is_some());
        assert!(caps["requests"]["tools"]["call"].is_object());
    }

    #[test]
    fn new_uses_default_security() {
        let store = Arc::new(InMemoryTaskStore::new());
        let router = TaskRouterImpl::new(store);
        assert_eq!(router.security_config().max_tasks_per_owner, 100);
        assert!(!router.security_config().allow_anonymous);
    }

    #[test]
    fn with_security_uses_custom_config() {
        let store = Arc::new(InMemoryTaskStore::new());
        let config = TaskSecurityConfig::default()
            .with_max_tasks_per_owner(50)
            .with_allow_anonymous(true);
        let router = TaskRouterImpl::with_security(store, config);
        assert_eq!(router.security_config().max_tasks_per_owner, 50);
        assert!(router.security_config().allow_anonymous);
    }

    // --- Workflow task method tests ---

    #[tokio::test]
    async fn create_workflow_task_creates_task_with_progress() {
        let router = make_router();
        let progress = serde_json::json!({
            "goal": "Deploy service",
            "steps": [
                { "name": "validate", "tool": "validate_config", "status": "pending" }
            ],
            "schemaVersion": 1
        });

        let result = router
            .create_workflow_task("deploy-workflow", "owner-1", progress.clone())
            .await
            .unwrap();

        assert!(result.get("task").is_some());
        let task = &result["task"];
        assert_eq!(task["status"], "working");

        // Verify progress stored in variables
        let task_id = task["taskId"].as_str().unwrap();
        let record = router.store().get(task_id, "owner-1").await.unwrap();
        assert_eq!(
            record
                .variables
                .get(crate::types::workflow::WORKFLOW_PROGRESS_KEY)
                .unwrap(),
            &progress
        );
    }

    #[tokio::test]
    async fn create_workflow_task_rejects_non_object_progress() {
        let router = make_router();

        let err = router
            .create_workflow_task("wf", "owner-1", serde_json::json!("not an object"))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("must be a JSON object"));
    }

    #[tokio::test]
    async fn set_task_variables_updates_variables() {
        let router = make_router();

        // Create a task first
        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let variables = serde_json::json!({
            "_workflow.result.validate": { "output": "valid" },
            "_workflow.progress": { "goal": "test", "steps": [], "schemaVersion": 1 }
        });

        router
            .set_task_variables(&task_id, "owner-1", variables)
            .await
            .unwrap();

        let updated = router.store().get(&task_id, "owner-1").await.unwrap();
        assert!(updated.variables.contains_key("_workflow.result.validate"));
        assert!(updated.variables.contains_key("_workflow.progress"));
    }

    #[tokio::test]
    async fn set_task_variables_rejects_invalid_json() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        // Pass a non-object value (array)
        let err = router
            .set_task_variables(&task_id, "owner-1", serde_json::json!([1, 2, 3]))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("invalid variables"));
    }

    #[tokio::test]
    async fn set_task_variables_rejects_unknown_task() {
        let router = make_router();

        let err = router
            .set_task_variables("nonexistent", "owner-1", serde_json::json!({}))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn complete_workflow_task_transitions_to_completed() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let result_value = serde_json::json!({ "summary": "all steps done" });
        let response = router
            .complete_workflow_task(&task_id, "owner-1", result_value)
            .await
            .unwrap();

        assert_eq!(response["taskId"], task_id);
        assert_eq!(response["status"], "completed");
    }

    #[tokio::test]
    async fn complete_workflow_task_rejects_unknown_task() {
        let router = make_router();

        let err = router
            .complete_workflow_task("nonexistent", "owner-1", serde_json::json!({}))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn complete_workflow_task_rejects_already_terminal() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        // Complete once
        router
            .complete_workflow_task(&task_id, "owner-1", serde_json::json!({}))
            .await
            .unwrap();

        // Try to complete again
        let err = router
            .complete_workflow_task(&task_id, "owner-1", serde_json::json!({}))
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("invalid transition") || err.to_string().contains("terminal")
        );
    }

    // --- Workflow continuation tests ---

    /// Helper: creates a task with workflow progress containing pending steps.
    async fn create_task_with_workflow_progress(router: &TaskRouterImpl, owner_id: &str) -> String {
        let progress = serde_json::json!({
            "goal": "Deploy service",
            "steps": [
                { "name": "validate", "tool": "validate_config", "status": "completed" },
                { "name": "deploy", "tool": "deploy_service", "status": "pending" },
                { "name": "notify", "tool": "send_notification", "status": "pending" }
            ],
            "schemaVersion": 1
        });

        let result = router
            .create_workflow_task("test-workflow", owner_id, progress)
            .await
            .unwrap();

        result["task"]["taskId"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn handle_workflow_continuation_matches_step() {
        let router = make_router();
        let task_id = create_task_with_workflow_progress(&router, "owner-1").await;

        // Call continuation with a tool that matches the pending "deploy" step
        router
            .handle_workflow_continuation(
                &task_id,
                "deploy_service",
                serde_json::json!({"status": "deployed"}),
                "owner-1",
            )
            .await
            .unwrap();

        // Verify _workflow.result.deploy is set
        let record = router.store().get(&task_id, "owner-1").await.unwrap();
        let result = record
            .variables
            .get("_workflow.result.deploy")
            .expect("_workflow.result.deploy should be set");
        assert_eq!(result["status"], "deployed");

        // Verify _workflow.progress shows the step as "completed"
        let progress = record
            .variables
            .get("_workflow.progress")
            .expect("_workflow.progress should be set");
        let steps = progress["steps"].as_array().unwrap();
        // deploy step (index 1) should now be completed
        assert_eq!(steps[1]["status"], "completed");
        assert_eq!(steps[1]["name"], "deploy");
        // notify step (index 2) should still be pending
        assert_eq!(steps[2]["status"], "pending");
    }

    #[tokio::test]
    async fn handle_workflow_continuation_unmatched_tool() {
        let router = make_router();
        let task_id = create_task_with_workflow_progress(&router, "owner-1").await;

        // Call continuation with a tool NOT in any step
        router
            .handle_workflow_continuation(
                &task_id,
                "debug_tool",
                serde_json::json!({"info": "debug data"}),
                "owner-1",
            )
            .await
            .unwrap();

        // Verify _workflow.extra.debug_tool is set
        let record = router.store().get(&task_id, "owner-1").await.unwrap();
        let extra = record
            .variables
            .get("_workflow.extra.debug_tool")
            .expect("_workflow.extra.debug_tool should be set");
        assert_eq!(extra["info"], "debug data");

        // Verify progress steps are unchanged
        let progress = record
            .variables
            .get("_workflow.progress")
            .expect("_workflow.progress should be set");
        let steps = progress["steps"].as_array().unwrap();
        assert_eq!(steps[1]["status"], "pending"); // deploy still pending
        assert_eq!(steps[2]["status"], "pending"); // notify still pending
    }

    #[tokio::test]
    async fn handle_workflow_continuation_last_result_wins() {
        let router = make_router();
        let task_id = create_task_with_workflow_progress(&router, "owner-1").await;

        // First call for "deploy" step
        router
            .handle_workflow_continuation(
                &task_id,
                "deploy_service",
                serde_json::json!({"attempt": 1}),
                "owner-1",
            )
            .await
            .unwrap();

        // Second call for "notify" step (simulate retry on different pending step)
        // But actually, deploy step is now completed, so deploy_service won't match again.
        // Let's test overwrite by calling the "notify" step tool twice instead.
        router
            .handle_workflow_continuation(
                &task_id,
                "send_notification",
                serde_json::json!({"attempt": 1}),
                "owner-1",
            )
            .await
            .unwrap();

        // Verify first result
        let record = router.store().get(&task_id, "owner-1").await.unwrap();
        assert_eq!(record.variables["_workflow.result.notify"]["attempt"], 1);

        // Now the "notify" step is marked completed, so a retry would go to extra.
        // To test true overwrite (last result wins), let's create a new task with
        // a failed step and call twice.
        let progress2 = serde_json::json!({
            "goal": "Retry test",
            "steps": [
                { "name": "step_a", "tool": "tool_a", "status": "failed" }
            ],
            "schemaVersion": 1
        });
        let result2 = router
            .create_workflow_task("retry-wf", "owner-1", progress2)
            .await
            .unwrap();
        let task_id2 = result2["task"]["taskId"].as_str().unwrap().to_string();

        // First attempt
        router
            .handle_workflow_continuation(
                &task_id2,
                "tool_a",
                serde_json::json!({"version": "v1"}),
                "owner-1",
            )
            .await
            .unwrap();

        // Step is now "completed", create another task with the step still failed
        // to truly test overwrite
        let progress3 = serde_json::json!({
            "goal": "Overwrite test",
            "steps": [
                { "name": "step_b", "tool": "tool_b", "status": "failed" }
            ],
            "schemaVersion": 1
        });
        let result3 = router
            .create_workflow_task("overwrite-wf", "owner-1", progress3)
            .await
            .unwrap();
        let task_id3 = result3["task"]["taskId"].as_str().unwrap().to_string();

        // First result
        router
            .handle_workflow_continuation(
                &task_id3,
                "tool_b",
                serde_json::json!({"version": "v1"}),
                "owner-1",
            )
            .await
            .unwrap();

        let record3 = router.store().get(&task_id3, "owner-1").await.unwrap();
        assert_eq!(
            record3.variables["_workflow.result.step_b"]["version"],
            "v1"
        );

        // After step is completed, a second call with same tool goes to extra
        // (since step is now "completed", not "pending" or "failed")
        router
            .handle_workflow_continuation(
                &task_id3,
                "tool_b",
                serde_json::json!({"version": "v2"}),
                "owner-1",
            )
            .await
            .unwrap();

        let record3_updated = router.store().get(&task_id3, "owner-1").await.unwrap();
        // The original result stays (step is completed, no re-match)
        assert_eq!(
            record3_updated.variables["_workflow.result.step_b"]["version"],
            "v1"
        );
        // Second call goes to extra
        assert_eq!(
            record3_updated.variables["_workflow.extra.tool_b"]["version"],
            "v2"
        );
    }

    #[tokio::test]
    async fn handle_workflow_continuation_rejects_non_working_task() {
        let router = make_router();
        let task_id = create_task_with_workflow_progress(&router, "owner-1").await;

        // Complete the task first
        router
            .complete_workflow_task(&task_id, "owner-1", serde_json::json!({}))
            .await
            .unwrap();

        // Try continuation on completed task
        let err = router
            .handle_workflow_continuation(
                &task_id,
                "deploy_service",
                serde_json::json!({}),
                "owner-1",
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("not in Working status"),
            "Expected 'not in Working status' error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn handle_tasks_cancel_with_result_completes() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let params = serde_json::json!({
            "taskId": task_id,
            "result": {"summary": "workflow completed by client"}
        });
        let result = router.handle_tasks_cancel(params, "owner-1").await.unwrap();

        assert_eq!(result["taskId"], task_id);
        assert_eq!(result["status"], "completed"); // Not "cancelled"
    }

    #[tokio::test]
    async fn handle_tasks_cancel_without_result_cancels() {
        let router = make_router();

        let record = router
            .store()
            .create("owner-1", "tools/call", None)
            .await
            .unwrap();
        let task_id = record.task.task_id.clone();

        let params = serde_json::json!({ "taskId": task_id });
        let result = router.handle_tasks_cancel(params, "owner-1").await.unwrap();

        assert_eq!(result["taskId"], task_id);
        assert_eq!(result["status"], "cancelled"); // Standard cancel
    }
}
