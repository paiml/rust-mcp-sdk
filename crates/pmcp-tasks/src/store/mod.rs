//! Task store trait, generic implementation, and supporting types.
//!
//! # Architecture
//!
//! The task storage system has three layers:
//!
//! 1. **[`TaskStore`]** -- A type-erasure interface for use with
//!    `Arc<dyn TaskStore>` in [`TaskContext`](crate::context::TaskContext)
//!    and [`TaskRouterImpl`](crate::router::TaskRouterImpl).
//!
//! 2. **[`GenericTaskStore<B>`](crate::store::generic::GenericTaskStore)** -- All domain
//!    logic (state machine, owner isolation, variable merge, TTL, CAS-based
//!    mutations, canonical serialization). Has a blanket `TaskStore` impl.
//!
//! 3. **[`StorageBackend`]** -- Dumb KV trait that backends implement
//!    (in-memory, DynamoDB, Redis). No domain logic.
//!
//! To create a store: `GenericTaskStore::new(backend)` and wrap in
//! `Arc<dyn TaskStore>` for use with `TaskContext` and `TaskRouterImpl`.
//!
//! # Backends
//!
//! - [`InMemoryBackend`](crate::store::memory::InMemoryBackend) -- Thread-safe
//!   in-memory backend using `DashMap`. Used by
//!   [`InMemoryTaskStore`](crate::store::memory::InMemoryTaskStore).
//! - `DynamoDbBackend` -- DynamoDB backend for production AWS/Lambda
//!   deployments. Available behind the `dynamodb` feature flag.
//! - `RedisBackend` -- Redis backend for long-running server deployments.
//!   Available behind the `redis` feature flag.
//!
//! # Supporting Types
//!
//! - [`StoreConfig`] - Configurable limits for variable size and TTL.
//! - [`ListTasksOptions`] - Parameters for cursor-based task listing.
//! - [`TaskPage`] - A page of task results with optional next cursor.

pub mod backend;
#[cfg(feature = "dynamodb")]
pub mod dynamodb;
pub mod generic;
pub mod memory;
#[cfg(feature = "redis")]
pub mod redis;

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

pub use backend::{StorageBackend, StorageError, VersionedRecord};

use crate::domain::TaskRecord;
use crate::error::TaskError;
use crate::types::task::TaskStatus;

/// Configuration for variable size limits, TTL enforcement, and variable
/// validation.
///
/// Applied at the trait level across all backends. Store implementations
/// should respect these limits when processing variable and TTL operations.
///
/// # Defaults
///
/// | Setting                   | Default      | Description                          |
/// |---------------------------|--------------|--------------------------------------|
/// | `max_variable_size_bytes` | 1,048,576    | 1 MB per variable payload            |
/// | `default_ttl_ms`          | 3,600,000    | 1 hour                               |
/// | `max_ttl_ms`              | 86,400,000   | 24 hours                             |
/// | `max_variable_depth`      | 10           | Max JSON nesting depth for variables |
/// | `max_string_length`       | 65,536       | Max bytes per string value (64 KB)   |
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::StoreConfig;
///
/// let config = StoreConfig::default();
/// assert_eq!(config.max_variable_size_bytes, 1_048_576);
/// assert_eq!(config.default_ttl_ms, Some(3_600_000));
/// assert_eq!(config.max_ttl_ms, Some(86_400_000));
/// assert_eq!(config.max_variable_depth, 10);
/// assert_eq!(config.max_string_length, 65_536);
///
/// let custom = StoreConfig {
///     max_variable_size_bytes: 512_000,
///     default_ttl_ms: Some(1_800_000), // 30 minutes
///     max_ttl_ms: Some(7_200_000),     // 2 hours
///     max_variable_depth: 5,
///     max_string_length: 32_768,
/// };
/// assert_eq!(custom.max_variable_size_bytes, 512_000);
/// assert_eq!(custom.max_variable_depth, 5);
/// ```
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Maximum size in bytes for a single variable payload.
    ///
    /// When a `set_variables` call would cause the serialized size of
    /// the variable map to exceed this limit, the store should return
    /// [`TaskError::VariableSizeExceeded`].
    pub max_variable_size_bytes: usize,

    /// Default TTL in milliseconds applied when a task is created without
    /// an explicit TTL. `None` means tasks do not expire by default.
    pub default_ttl_ms: Option<u64>,

    /// Maximum allowed TTL in milliseconds. The store should clamp or
    /// reject TTL values that exceed this limit. `None` means no upper
    /// bound on TTL.
    pub max_ttl_ms: Option<u64>,

    /// Maximum nesting depth for variable JSON values.
    ///
    /// Prevents depth bombs (deeply nested objects/arrays that can cause
    /// stack overflow during processing). Default: 10.
    pub max_variable_depth: usize,

    /// Maximum length in bytes for any single string value within variables.
    ///
    /// Prevents extremely long strings that could consume excessive memory
    /// or storage. Default: 65,536 (64 KB).
    pub max_string_length: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_variable_size_bytes: 1_048_576, // 1 MB
            default_ttl_ms: Some(3_600_000),    // 1 hour
            max_ttl_ms: Some(86_400_000),       // 24 hours
            max_variable_depth: 10,
            max_string_length: 65_536, // 64 KB
        }
    }
}

/// Options for listing tasks with cursor-based pagination.
///
/// All task listing is scoped to an owner. The cursor is opaque to the
/// caller -- it is produced by the store and should be passed back verbatim
/// for the next page.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::ListTasksOptions;
///
/// let options = ListTasksOptions {
///     owner_id: "session-abc".to_string(),
///     cursor: None,
///     limit: Some(25),
/// };
/// assert_eq!(options.owner_id, "session-abc");
/// assert!(options.cursor.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct ListTasksOptions {
    /// Tasks scoped to this owner.
    pub owner_id: String,

    /// Opaque cursor for pagination. `None` for the first page.
    pub cursor: Option<String>,

    /// Maximum number of tasks to return. `None` uses the backend default.
    pub limit: Option<usize>,
}

/// A page of task results from a list operation.
///
/// Contains the tasks for the current page and an optional cursor for
/// fetching the next page.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::TaskPage;
///
/// let page = TaskPage {
///     tasks: vec![],
///     next_cursor: None,
/// };
/// assert!(page.tasks.is_empty());
/// assert!(page.next_cursor.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct TaskPage {
    /// The tasks in this page of results.
    pub tasks: Vec<TaskRecord>,

    /// Cursor for the next page. `None` if there are no more results.
    pub next_cursor: Option<String>,
}

/// Type-erasure interface for task storage.
///
/// This trait serves as the dynamic dispatch interface for
/// [`GenericTaskStore<B>`](crate::store::generic::GenericTaskStore). Domain logic lives
/// in `GenericTaskStore`, not in trait implementations. Use
/// `GenericTaskStore::new(backend)` to create a store, then wrap in
/// `Arc<dyn TaskStore>` for use with
/// [`TaskContext`](crate::context::TaskContext) and
/// [`TaskRouterImpl`](crate::router::TaskRouterImpl).
///
/// A blanket implementation is provided for `GenericTaskStore<B>` where
/// `B: StorageBackend + 'static`, so any `GenericTaskStore` automatically
/// satisfies this trait.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent access
/// from multiple request handlers.
///
/// # Atomicity
///
/// The [`complete_with_result`](TaskStore::complete_with_result) method
/// must be atomic: if either the status transition or the result storage
/// fails, neither operation should be applied. Other methods should also
/// be implemented with appropriate concurrency controls.
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Creates a new task in the `Working` state.
    ///
    /// Generates a unique task ID and initializes all task fields.
    /// The `ttl` parameter overrides `StoreConfig::default_ttl_ms`; if
    /// `None`, the store's default TTL is applied.
    ///
    /// # Errors
    ///
    /// - [`TaskError::ResourceExhausted`] if the store has reached its
    ///   capacity limit for active tasks.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError>;

    /// Retrieves a task by ID, scoped to the given owner.
    ///
    /// Returns the current state of the task, including expired tasks
    /// (callers can check [`TaskRecord::is_expired()`] to detect expiry).
    /// Expired tasks remain readable until [`cleanup_expired`](TaskStore::cleanup_expired)
    /// removes them, allowing clients to inspect state and retry with a
    /// longer TTL if needed.
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists
    ///   (or if the task belongs to a different owner -- owner mismatch
    ///   is indistinguishable from not found for security).
    /// - [`TaskError::StoreError`] on backend failures.
    async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError>;

    /// Transitions a task to a new status.
    ///
    /// Validates the transition against the state machine before applying.
    /// Updates `last_updated_at` and optionally sets a status message.
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::InvalidTransition`] if the transition is not allowed
    ///   by the state machine (e.g., terminal to any, self-transitions).
    /// - [`TaskError::StoreError`] on backend failures.
    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError>;

    /// Merges variables into the task's variable store.
    ///
    /// For each entry in `variables`:
    /// - If the value is not `null`, upsert the key-value pair.
    /// - If the value is `null`, delete that key from the store.
    ///
    /// The total variable payload size (after merge) must not exceed
    /// `StoreConfig::max_variable_size_bytes`.
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::VariableSizeExceeded`] if the merged payload exceeds
    ///   the configured limit.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError>;

    /// Stores the operation result for a task.
    ///
    /// The result is the outcome of the long-running operation and is
    /// returned via `tasks/result`. This can be called independently of
    /// status transitions, though typically the result is set when
    /// transitioning to a terminal state (see [`complete_with_result`]).
    ///
    /// [`complete_with_result`]: TaskStore::complete_with_result
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError>;

    /// Retrieves the stored result for a task.
    ///
    /// The result is only available after the task has reached a terminal
    /// state (`Completed`, `Failed`). Returns `NotReady` if the task is
    /// still in a non-terminal state.
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::NotReady`] if the task has not reached a terminal state.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn get_result(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError>;

    /// Atomically transitions to a terminal status AND stores the result.
    ///
    /// This is the preferred way to complete a task. If either the status
    /// transition or the result storage fails, **neither operation is
    /// applied** -- the task remains in its original state.
    ///
    /// # Atomicity Guarantee
    ///
    /// Implementations must ensure that:
    /// - The status transition and result storage happen as a single
    ///   atomic operation.
    /// - On failure, the task is not left in a partial state (e.g., status
    ///   changed but result not stored).
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::InvalidTransition`] if the transition is not allowed
    ///   by the state machine.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError>;

    /// Lists tasks for an owner with cursor-based pagination.
    ///
    /// Returns a page of tasks matching the given options. Results are
    /// ordered by creation time (newest first). Expired tasks may or may
    /// not be included depending on the backend implementation -- callers
    /// should check `is_expired()` if needed.
    ///
    /// # Errors
    ///
    /// - [`TaskError::StoreError`] on backend failures.
    async fn list(&self, options: ListTasksOptions) -> Result<TaskPage, TaskError>;

    /// Cancels a non-terminal task.
    ///
    /// Transitions the task to `Cancelled` status. Equivalent to
    /// `update_status(task_id, owner_id, TaskStatus::Cancelled, None)` but
    /// provided as a convenience method for the `tasks/cancel` endpoint.
    ///
    /// # Errors
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
    /// - [`TaskError::InvalidTransition`] if the task is already in a
    ///   terminal state.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError>;

    /// Removes expired tasks from storage.
    ///
    /// Scans for tasks whose TTL has elapsed and removes them. Returns
    /// the count of tasks removed. Implementations should be efficient
    /// for periodic background cleanup.
    ///
    /// # Errors
    ///
    /// - [`TaskError::StoreError`] on backend failures.
    async fn cleanup_expired(&self) -> Result<usize, TaskError>;

    /// Returns a reference to the store's configuration.
    ///
    /// This method is synchronous (not async) since it returns a reference
    /// to a configuration value that does not require I/O.
    fn config(&self) -> &StoreConfig;

    // ---- v2 tasks extension: input delivery (additive, defaulted) ----
    //
    // Every method below is ADDITIVE WITH A DEFAULT, specifically so that
    // out-of-tree `TaskStore` implementations keep compiling unchanged. Unlike
    // the required methods above, omitting them is legal; the default then says
    // so explicitly rather than silently succeeding.
    //
    // All five cross the `serde_json::Value` seam in both directions. This crate
    // sits BELOW that seam: the typed `InputRequests` / `InputResponses` /
    // `TaskInputDelivery` / `TaskInputSnapshot` shapes live in `pmcp` above it,
    // and re-declaring them here would put the same wire contract in two places.

    /// Delivers a client's `inputResponses` against the task's SERVER-recorded
    /// `inputRequests`, scoped to `owner_id`.
    ///
    /// `responses` is a JSON object keyed by the server-assigned input key. The
    /// return value is a JSON object with three members, mirroring `pmcp`'s
    /// `TaskInputDelivery` field for field so the layer above can build the
    /// typed value without a second read:
    ///
    /// | Key | Type | Meaning |
    /// |-----|------|---------|
    /// | `accepted` | `string[]` | Keys that were outstanding and unanswered, and whose responses this call persisted |
    /// | `ignored`  | `string[]` | Keys that were NOT outstanding — never issued, already answered, or superseded |
    /// | `complete` | `bool`     | Whether every outstanding request now has a response |
    ///
    /// # Partial delivery is legitimate
    ///
    /// A key that is not currently outstanding is IGNORED rather than turned
    /// into an error, and a caller MAY deliver a subset of the outstanding keys.
    /// A partial delivery persists its responses and the task REMAINS
    /// [`TaskStatus::InputRequired`] until the rest arrive.
    ///
    /// # Bounds are NOT enforced here
    ///
    /// The four `inputResponses` denial-of-service bounds — entry COUNT, ONE
    /// entry's serialized size, TOTAL serialized size, and one entry's nesting
    /// DEPTH — are enforced above this seam at request ingress, before any
    /// decode, so an oversized payload never reaches a store. An implementation
    /// must not re-check them and must not mint limits of its own. (The fifth,
    /// adjacent bound on the MRTR `requestState` continuation token does NOT
    /// apply: a `tasks/update` carries no continuation token. Four, not five.)
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskError::StoreError`]
    /// ("store does not support task input delivery"). Overriding
    /// implementations return:
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists, or it
    ///   belongs to a different owner (owner mismatch is indistinguishable from
    ///   not found for security).
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::InvalidTransition`] if the task is not awaiting input — a
    ///   terminal or still-`working` task cannot be fed.
    /// - [`TaskError::ConcurrentModification`] if another writer modified the
    ///   task between this call's read and its write.
    /// - [`TaskError::StoreError`] on backend failures, or if `responses` is not
    ///   a JSON object.
    async fn deliver_inputs(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _responses: Value,
    ) -> Result<Value, TaskError> {
        Err(TaskError::StoreError(
            "store does not support task input delivery".to_string(),
        ))
    }

    /// Reads the task's input state — the server-recorded requests, the
    /// delivered responses and the current status — scoped to `owner_id`.
    ///
    /// Returns a JSON object with `inputRequests`, `inputResponses` and
    /// `status`. The *outstanding* (recorded-but-unanswered) subset is derived
    /// above this seam from those two maps rather than duplicated here, so the
    /// two layers cannot disagree about what "outstanding" means.
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskError::NotFound`] — a
    /// store that records no input requests has no snapshot to return.
    /// Overriding implementations return:
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists, it belongs
    ///   to a different owner, or it has no recorded input requests.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn task_input_snapshot(
        &self,
        task_id: &str,
        _owner_id: &str,
    ) -> Result<Value, TaskError> {
        Err(TaskError::NotFound {
            task_id: task_id.to_string(),
        })
    }

    /// Records the inputs the server needs before this task can continue and
    /// transitions it to [`TaskStatus::InputRequired`] in the SAME write, so a
    /// task is never observable as `input_required` with nothing to answer.
    ///
    /// # `requests` is SERVER-AUTHORED
    ///
    /// `requests` MUST be authored by the server and MUST NOT be sourced from
    /// anything a client sent. What is written here is the only trustworthy
    /// record of which KIND was asked for under each key; letting a client
    /// influence it would let the client choose how its own answer is typed.
    ///
    /// Returns the paused task as a JSON object.
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskError::StoreError`]
    /// ("store does not support recording task input requests"). Overriding
    /// implementations return:
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists, or it
    ///   belongs to a different owner.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::InvalidTransition`] if the task cannot move to
    ///   [`TaskStatus::InputRequired`] — it is terminal, or it is already
    ///   awaiting input.
    /// - [`TaskError::ConcurrentModification`] on a CAS conflict.
    /// - [`TaskError::StoreError`] on backend failures, if `requests` is not a
    ///   JSON object, or if a key is already recorded (which would orphan or
    ///   erase a response already delivered against it).
    async fn record_input_requests(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _requests: Value,
    ) -> Result<Value, TaskError> {
        Err(TaskError::StoreError(
            "store does not support recording task input requests".to_string(),
        ))
    }

    /// Persists the JSON-RPC error object for a failed task, scoped to
    /// `owner_id`, so a v2 `tasks/get` can inline it.
    ///
    /// The value is the JSON-RPC error OBJECT (`code`/`message`/`data`) exactly
    /// as it will appear on the wire, carried verbatim rather than re-typed.
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskError::StoreError`]
    /// ("store does not support task errors"). Overriding implementations
    /// return:
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists, or it
    ///   belongs to a different owner.
    /// - [`TaskError::Expired`] if the task's TTL has elapsed.
    /// - [`TaskError::ConcurrentModification`] on a CAS conflict.
    /// - [`TaskError::StoreError`] on backend failures.
    async fn set_error(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _error: Value,
    ) -> Result<(), TaskError> {
        Err(TaskError::StoreError(
            "store does not support task errors".to_string(),
        ))
    }

    /// Retrieves the persisted JSON-RPC error object for a task, scoped to
    /// `owner_id`.
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskError::NotFound`] — a
    /// store that persists no errors has none to return. Overriding
    /// implementations return:
    ///
    /// - [`TaskError::NotFound`] if no task with the given ID exists, it belongs
    ///   to a different owner, or it recorded no error (it did not fail, or has
    ///   not failed yet).
    /// - [`TaskError::StoreError`] on backend failures.
    async fn get_error(&self, task_id: &str, _owner_id: &str) -> Result<Value, TaskError> {
        Err(TaskError::NotFound {
            task_id: task_id.to_string(),
        })
    }
}

// ---- Blanket impl for GenericTaskStore<B> ----

#[async_trait]
impl<B: StorageBackend + 'static> TaskStore for generic::GenericTaskStore<B> {
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError> {
        self.create(owner_id, request_method, ttl).await
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        self.get(task_id, owner_id).await
    }

    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError> {
        self.update_status(task_id, owner_id, new_status, status_message)
            .await
    }

    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError> {
        self.set_variables(task_id, owner_id, variables).await
    }

    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError> {
        self.set_result(task_id, owner_id, result).await
    }

    async fn get_result(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.get_result(task_id, owner_id).await
    }

    async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError> {
        self.complete_with_result(task_id, owner_id, status, status_message, result)
            .await
    }

    async fn list(&self, options: ListTasksOptions) -> Result<TaskPage, TaskError> {
        self.list(options).await
    }

    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        self.cancel(task_id, owner_id).await
    }

    async fn cleanup_expired(&self) -> Result<usize, TaskError> {
        self.cleanup_expired().await
    }

    fn config(&self) -> &StoreConfig {
        self.config()
    }

    // The five input-delivery methods are forwarded here for the same reason as
    // the eleven above: without a forwarding line the blanket impl would inherit
    // the trait's not-supported DEFAULT, and every `Arc<dyn TaskStore>` built
    // from a `GenericTaskStore` would report "store does not support task input
    // delivery" while the inherent method right next to it worked.

    async fn deliver_inputs(
        &self,
        task_id: &str,
        owner_id: &str,
        responses: Value,
    ) -> Result<Value, TaskError> {
        self.deliver_inputs(task_id, owner_id, responses).await
    }

    async fn task_input_snapshot(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.task_input_snapshot(task_id, owner_id).await
    }

    async fn record_input_requests(
        &self,
        task_id: &str,
        owner_id: &str,
        requests: Value,
    ) -> Result<Value, TaskError> {
        self.record_input_requests(task_id, owner_id, requests)
            .await
    }

    async fn set_error(
        &self,
        task_id: &str,
        owner_id: &str,
        error: Value,
    ) -> Result<(), TaskError> {
        self.set_error(task_id, owner_id, error).await
    }

    async fn get_error(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.get_error(task_id, owner_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_config_default() {
        let config = StoreConfig::default();
        assert_eq!(config.max_variable_size_bytes, 1_048_576);
        assert_eq!(config.default_ttl_ms, Some(3_600_000));
        assert_eq!(config.max_ttl_ms, Some(86_400_000));
    }

    #[test]
    fn store_config_custom() {
        let config = StoreConfig {
            max_variable_size_bytes: 512_000,
            default_ttl_ms: None,
            max_ttl_ms: Some(7_200_000),
            max_variable_depth: 5,
            max_string_length: 32_768,
        };
        assert_eq!(config.max_variable_size_bytes, 512_000);
        assert!(config.default_ttl_ms.is_none());
        assert_eq!(config.max_ttl_ms, Some(7_200_000));
        assert_eq!(config.max_variable_depth, 5);
        assert_eq!(config.max_string_length, 32_768);
    }

    #[test]
    fn store_config_clone() {
        let config = StoreConfig::default();
        let cloned = config.clone();
        assert_eq!(
            config.max_variable_size_bytes,
            cloned.max_variable_size_bytes
        );
    }

    #[test]
    fn store_config_debug() {
        let config = StoreConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("StoreConfig"));
        assert!(debug.contains("1048576"));
    }

    #[test]
    fn list_tasks_options_construction() {
        let options = ListTasksOptions {
            owner_id: "session-abc".to_string(),
            cursor: None,
            limit: Some(25),
        };
        assert_eq!(options.owner_id, "session-abc");
        assert!(options.cursor.is_none());
        assert_eq!(options.limit, Some(25));
    }

    #[test]
    fn list_tasks_options_with_cursor() {
        let options = ListTasksOptions {
            owner_id: "owner".to_string(),
            cursor: Some("cursor-token-123".to_string()),
            limit: None,
        };
        assert_eq!(options.cursor.as_deref(), Some("cursor-token-123"));
        assert!(options.limit.is_none());
    }

    #[test]
    fn task_page_empty() {
        let page = TaskPage {
            tasks: vec![],
            next_cursor: None,
        };
        assert!(page.tasks.is_empty());
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn task_page_with_records() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        let page = TaskPage {
            tasks: vec![record],
            next_cursor: Some("next-page-cursor".to_string()),
        };
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.next_cursor.as_deref(), Some("next-page-cursor"));
    }

    #[test]
    fn task_page_clone() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let page = TaskPage {
            tasks: vec![record],
            next_cursor: None,
        };
        let cloned = page.clone();
        assert_eq!(cloned.tasks.len(), 1);
        assert_eq!(cloned.tasks[0].task.task_id, page.tasks[0].task.task_id);
    }
}
