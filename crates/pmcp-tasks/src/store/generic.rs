//! Generic task store with all domain logic delegating to a [`StorageBackend`].
//!
//! [`GenericTaskStore`] implements every domain operation (state machine
//! transitions, owner isolation, variable merge with null-deletion, size
//! limit enforcement, TTL validation, CAS-based mutations, canonical JSON
//! serialization) on top of any [`StorageBackend`] implementation.
//!
//! Backends remain dumb key-value stores; all intelligence lives here.
//!
//! # Construction
//!
//! Use the builder pattern:
//!
//! ```rust,no_run
//! # use pmcp_tasks::store::generic::GenericTaskStore;
//! # use pmcp_tasks::store::StoreConfig;
//! # use pmcp_tasks::security::TaskSecurityConfig;
//! // Assuming `backend` implements StorageBackend:
//! // let store = GenericTaskStore::new(backend)
//! //     .with_config(StoreConfig::default())
//! //     .with_security(TaskSecurityConfig::default())
//! //     .with_poll_interval(1000);
//! ```
//!
//! # CAS Semantics
//!
//! All mutation operations (except `create`) use
//! [`StorageBackend::put_if_version`] for optimistic concurrency. A version
//! mismatch surfaces as [`TaskError::ConcurrentModification`].
//!
//! # Owner Isolation
//!
//! Owner mismatch on any operation returns [`TaskError::NotFound`] -- the
//! store never reveals that a task exists for a different owner.

use std::collections::{BTreeSet, HashMap};

use pmcp::server::task_store::partition_input_delivery;
use serde_json::{json, Map, Value};

use crate::domain::record::{validate_variables, TaskRecord};
use crate::error::TaskError;
use crate::security::{TaskSecurityConfig, DEFAULT_LOCAL_OWNER};
use crate::store::backend::{make_key, make_prefix, StorageBackend, StorageError};
use crate::store::{ListTasksOptions, StoreConfig, TaskPage};
use crate::types::task::TaskStatus;

/// Generic task store that delegates all storage to a [`StorageBackend`].
///
/// All domain logic lives here: state machine validation, owner isolation,
/// variable merge with null-deletion, size/depth/string-length enforcement,
/// TTL validation with hard reject, CAS-based mutations, and canonical
/// JSON serialization at the storage boundary.
///
/// # Type Parameters
///
/// * `B` - A [`StorageBackend`] implementation (in-memory, `DynamoDB`, Redis, etc.)
#[derive(Debug)]
pub struct GenericTaskStore<B: StorageBackend> {
    backend: B,
    config: StoreConfig,
    security: TaskSecurityConfig,
    default_poll_interval: u64,
}

impl<B: StorageBackend> GenericTaskStore<B> {
    /// Creates a new generic task store backed by the given backend.
    ///
    /// Uses default configuration:
    /// - `StoreConfig::default()` (1 MB variables, 1h default TTL, 24h max TTL)
    /// - `TaskSecurityConfig::default()` (100 tasks/owner, no anonymous access)
    /// - Poll interval: 500ms
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            config: StoreConfig::default(),
            security: TaskSecurityConfig::default(),
            default_poll_interval: 500,
        }
    }

    /// Sets the storage configuration.
    pub fn with_config(mut self, config: StoreConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the security configuration.
    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self {
        self.security = security;
        self
    }

    /// Sets the default poll interval in milliseconds.
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.default_poll_interval = ms;
        self
    }

    // ---- Serialization helpers (private) ----

    fn serialize_record(record: &TaskRecord) -> Result<Vec<u8>, TaskError> {
        serde_json::to_vec(record)
            .map_err(|e| TaskError::StoreError(format!("failed to serialize TaskRecord: {e}")))
    }

    fn deserialize_record(data: &[u8]) -> Result<TaskRecord, TaskError> {
        serde_json::from_slice(data)
            .map_err(|e| TaskError::StoreError(format!("failed to deserialize TaskRecord: {e}")))
    }

    fn map_storage_error(err: StorageError, task_id: &str) -> TaskError {
        match err {
            StorageError::NotFound { .. } => TaskError::NotFound {
                task_id: task_id.to_string(),
            },
            StorageError::VersionConflict {
                expected, actual, ..
            } => TaskError::ConcurrentModification {
                task_id: task_id.to_string(),
                expected_version: expected,
                actual_version: actual,
            },
            StorageError::CapacityExceeded { message } => TaskError::StorageFull { message },
            StorageError::Backend { message, .. } => TaskError::StoreError(message),
        }
    }

    /// Checks if the given owner ID represents anonymous/local access.
    fn is_anonymous_owner(owner_id: &str) -> bool {
        owner_id.is_empty() || owner_id == DEFAULT_LOCAL_OWNER
    }

    /// Validates that the owner has permission to create tasks.
    fn check_anonymous_access(&self, owner_id: &str) -> Result<(), TaskError> {
        if !self.security.allow_anonymous && Self::is_anonymous_owner(owner_id) {
            return Err(TaskError::StoreError(
                "anonymous access is not allowed; configure OAuth or enable allow_anonymous"
                    .to_string(),
            ));
        }
        Ok(())
    }

    // ---- Domain operations (public) ----

    /// Creates a new task in the `Working` state.
    ///
    /// Enforces anonymous access check, max tasks per owner, TTL maximum
    /// (hard reject, no silent clamping), and default TTL application.
    pub async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError> {
        // Check anonymous access
        self.check_anonymous_access(owner_id)?;

        // Count owner tasks via list_by_prefix
        let prefix = make_prefix(owner_id);
        let owner_records = self
            .backend
            .list_by_prefix(&prefix)
            .await
            .map_err(|e| Self::map_storage_error(e, ""))?;
        if owner_records.len() >= self.security.max_tasks_per_owner {
            return Err(TaskError::ResourceExhausted {
                suggested_action: Some("Cancel or wait for existing tasks to expire".to_string()),
            });
        }

        // Validate TTL against maximum (hard reject, no clamping)
        if let (Some(requested_ttl), Some(max_ttl)) = (ttl, self.config.max_ttl_ms) {
            if requested_ttl > max_ttl {
                return Err(TaskError::StoreError(format!(
                    "TTL {requested_ttl}ms exceeds maximum allowed {max_ttl}ms"
                )));
            }
        }

        // Apply default TTL if none provided
        let effective_ttl = ttl.or(self.config.default_ttl_ms);

        // Create the task record
        let mut record = TaskRecord::new(
            owner_id.to_string(),
            request_method.to_string(),
            effective_ttl,
        );
        record.task.poll_interval = Some(self.default_poll_interval);

        // Serialize to canonical JSON and store
        let key = make_key(owner_id, &record.task.task_id);
        let bytes = Self::serialize_record(&record)?;
        let version = self
            .backend
            .put(&key, &bytes)
            .await
            .map_err(|e| Self::map_storage_error(e, &record.task.task_id))?;
        record.version = version;

        Ok(record)
    }

    /// Retrieves a task by ID, scoped to the given owner.
    ///
    /// Returns the task even if expired (callers check `is_expired()`).
    /// Owner mismatch returns `NotFound` for security.
    pub async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        // Defense in depth: verify owner_id even though key is scoped
        Self::check_owner(&record, task_id, owner_id, "get")?;

        Ok(record)
    }

    /// Transitions a task to a new status with CAS-based atomicity.
    ///
    /// Validates owner, expiry, and state machine transition. Uses
    /// `put_if_version` so concurrent modifications are detected.
    pub async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        // Owner isolation
        Self::check_owner(&record, task_id, owner_id, "update_status")?;

        // Reject mutations on expired tasks
        Self::check_not_expired(&record, task_id)?;

        // Validate state machine transition
        record
            .task
            .status
            .validate_transition(task_id, &new_status)?;

        // Apply transition
        record.task.status = new_status;
        record.task.status_message = status_message;
        Self::touch(&mut record);

        // CAS write
        let bytes = Self::serialize_record(&record)?;
        let new_version = self
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;
        record.version = new_version;

        Ok(record)
    }

    /// Merges variables with null-deletion semantics and size/schema validation.
    ///
    /// Uses a clone-check-commit pattern: merges on a clone first, validates
    /// depth/string-length/total-size, and only commits if within limits.
    pub async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        // Owner isolation
        Self::check_owner(&record, task_id, owner_id, "set_variables")?;

        // Reject mutations on expired tasks
        Self::check_not_expired(&record, task_id)?;

        // Validate incoming variables for depth bombs and long strings
        validate_variables(
            &variables,
            self.config.max_variable_depth,
            self.config.max_string_length,
        )
        .map_err(|e| TaskError::StoreError(format!("variable validation failed: {e}")))?;

        // Clone-check-commit: merge on a clone first, validate size, then commit
        let mut merged = record.variables.clone();
        for (key_name, value) in &variables {
            if value.is_null() {
                merged.remove(key_name);
            } else {
                merged.insert(key_name.clone(), value.clone());
            }
        }

        // Check merged size against limit
        let serialized = serde_json::to_vec(&merged)
            .map_err(|e| TaskError::StoreError(format!("failed to serialize variables: {e}")))?;

        if serialized.len() > self.config.max_variable_size_bytes {
            return Err(TaskError::VariableSizeExceeded {
                limit_bytes: self.config.max_variable_size_bytes,
                actual_bytes: serialized.len(),
            });
        }

        // Commit the merged variables
        record.variables = merged;
        Self::touch(&mut record);

        // CAS write
        let bytes = Self::serialize_record(&record)?;
        let new_version = self
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;
        record.version = new_version;

        Ok(record)
    }

    /// Stores the operation result for a task.
    pub async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        // Owner isolation
        Self::check_owner(&record, task_id, owner_id, "set_result")?;

        // Reject mutations on expired tasks
        Self::check_not_expired(&record, task_id)?;

        record.result = Some(result);
        Self::touch(&mut record);

        // CAS write
        let bytes = Self::serialize_record(&record)?;
        self.backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        Ok(())
    }

    /// Retrieves the stored result for a completed task.
    ///
    /// Returns `NotReady` if the task has not reached a terminal state.
    pub async fn get_result(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        let record = self.get(task_id, owner_id).await?;

        // Must be in a terminal state to retrieve result
        if !record.task.status.is_terminal() {
            return Err(TaskError::NotReady {
                task_id: task_id.to_string(),
                current_status: record.task.status,
            });
        }

        record.result.ok_or_else(|| TaskError::NotReady {
            task_id: task_id.to_string(),
            current_status: record.task.status,
        })
    }

    /// Atomically transitions to a terminal status AND stores the result.
    ///
    /// Both the status transition and result storage are applied in a single
    /// CAS write via `put_if_version`, guaranteeing atomicity.
    pub async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        // Owner isolation
        Self::check_owner(&record, task_id, owner_id, "complete_with_result")?;

        // Reject mutations on expired tasks
        Self::check_not_expired(&record, task_id)?;

        // Validate state machine transition
        record.task.status.validate_transition(task_id, &status)?;

        // Apply atomically: status + result in a single CAS write
        record.task.status = status;
        record.task.status_message = status_message;
        record.result = Some(result);
        Self::touch(&mut record);

        // CAS write
        let bytes = Self::serialize_record(&record)?;
        let new_version = self
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;
        record.version = new_version;

        Ok(record)
    }

    /// Lists tasks for an owner with cursor-based pagination.
    ///
    /// Results are sorted by creation time (newest first). The cursor is
    /// the task ID of the last item in the previous page.
    pub async fn list(&self, options: ListTasksOptions) -> Result<TaskPage, TaskError> {
        let prefix = make_prefix(&options.owner_id);
        let entries = self
            .backend
            .list_by_prefix(&prefix)
            .await
            .map_err(|e| Self::map_storage_error(e, ""))?;

        // Deserialize all records
        let mut tasks: Vec<TaskRecord> = entries
            .iter()
            .filter_map(|(_, versioned)| {
                let mut record = Self::deserialize_record(&versioned.data).ok()?;
                record.version = versioned.version;
                Some(record)
            })
            .collect();

        // Sort by creation time, newest first
        tasks.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));

        // Cursor-based pagination: cursor = task_id of last item in previous page
        let start_idx = if let Some(ref cursor) = options.cursor {
            tasks
                .iter()
                .position(|t| t.task.task_id == *cursor)
                .map_or(0, |i| i + 1)
        } else {
            0
        };

        let limit = options.limit.unwrap_or(50);
        let page_tasks: Vec<TaskRecord> = tasks
            .get(start_idx..)
            .unwrap_or(&[])
            .iter()
            .take(limit)
            .cloned()
            .collect();

        let next_cursor = if start_idx + limit < tasks.len() {
            page_tasks.last().map(|t| t.task.task_id.clone())
        } else {
            None
        };

        Ok(TaskPage {
            tasks: page_tasks,
            next_cursor,
        })
    }

    /// Cancels a non-terminal task.
    ///
    /// Delegates to [`update_status`](Self::update_status) with
    /// `TaskStatus::Cancelled`.
    pub async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        self.update_status(task_id, owner_id, TaskStatus::Cancelled, None)
            .await
    }

    /// Removes expired tasks from storage.
    ///
    /// Delegates to [`StorageBackend::cleanup_expired`].
    pub async fn cleanup_expired(&self) -> Result<usize, TaskError> {
        self.backend
            .cleanup_expired()
            .await
            .map_err(|e| Self::map_storage_error(e, ""))
    }

    // -----------------------------------------------------------------------
    // Shared record guards
    //
    // Every domain operation above and below runs the same three steps against a
    // freshly-read record: prove the owner, refuse an expired task, stamp the
    // update time. They live here ONCE. The owner rule in particular is
    // security-critical — six hand-inlined copies is six places for a `NotFound`
    // to become something that reveals another owner's task exists.
    // -----------------------------------------------------------------------

    /// Owner isolation, defence in depth.
    ///
    /// The owner-prefixed key already makes another owner's record unreachable
    /// (a wrong owner produces a different key, so the read misses). This second
    /// check catches a record whose stored `ownerId` disagrees with the key it
    /// was found under, and reports it as `NotFound` so the store never reveals
    /// that a task exists for someone else.
    ///
    /// `op` names the calling operation and travels as a structured log field,
    /// so one message text serves every call site without any of them drifting.
    fn check_owner(
        record: &TaskRecord,
        task_id: &str,
        owner_id: &str,
        op: &str,
    ) -> Result<(), TaskError> {
        if record.owner_id == owner_id {
            return Ok(());
        }
        tracing::warn!(
            task_id = task_id,
            expected_owner = owner_id,
            actual_owner = record.owner_id,
            operation = op,
            "owner mismatch on task operation (returning NotFound)"
        );
        Err(TaskError::NotFound {
            task_id: task_id.to_string(),
        })
    }

    /// Rejects mutations on an expired task, matching the existing write paths.
    fn check_not_expired(record: &TaskRecord, task_id: &str) -> Result<(), TaskError> {
        if record.is_expired() {
            return Err(TaskError::Expired {
                task_id: task_id.to_string(),
                expired_at: record.expires_at.map(|e| e.to_rfc3339()),
            });
        }
        Ok(())
    }

    /// Stamps `lastUpdatedAt` with the store's canonical millisecond-precision
    /// RFC 3339 encoding.
    fn touch(record: &mut TaskRecord) {
        record.task.last_updated_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    }

    // -----------------------------------------------------------------------
    // v2 tasks extension: input delivery
    //
    // These live here, once, for the same reason every other domain operation
    // does: `GenericTaskStore<B>` is the ONE implementation and the backends are
    // dumb KV stores, so the in-memory, DynamoDB and Redis deployments share
    // identical semantics with zero divergence. Each operation below is a single
    // `put_if_version` compare-and-set. There is deliberately NO internal retry
    // and NO mutex around the read-then-write: a mutex would be local to one
    // process and would not prevent a lost update across two Lambda invocations
    // or two server instances, which is exactly the failure the CAS primitive
    // exists to prevent. A conflict propagates to the caller as
    // `ConcurrentModification` — first writer wins, second writer is told.
    // -----------------------------------------------------------------------

    /// Interprets a seam value as a JSON object. `null` and an absent value both
    /// mean "empty", which is what lets a pre-v2 record and an omitted parameter
    /// read the same way.
    fn as_object(value: Value, field: &str) -> Result<Map<String, Value>, TaskError> {
        match value {
            Value::Object(map) => Ok(map),
            Value::Null => Ok(Map::new()),
            other => Err(TaskError::StoreError(format!(
                "{field} must be a JSON object, got {}",
                json_type_name(&other)
            ))),
        }
    }

    /// Delivers a client's `inputResponses` against the task's server-recorded
    /// `inputRequests`, in ONE compare-and-set write.
    ///
    /// The atomic unit is *(persist the accepted responses [+ transition to
    /// `Working` iff the outstanding set is now complete])*: both mutations land
    /// in the same `put_if_version`, guaranteeing atomicity, so a task can never
    /// be observed as resumed with its answers missing, nor answered while still
    /// paused. A PARTIAL delivery persists and the task stays `input_required`.
    ///
    /// Keys that are not currently outstanding — never issued, already answered,
    /// or superseded — are IGNORED and reported, not turned into errors. An
    /// already-answered key is never re-accepted, so a delivered response cannot
    /// be replayed over.
    ///
    /// See [`TaskStore::deliver_inputs`](crate::store::TaskStore::deliver_inputs)
    /// for the returned shape and for why the payload bounds are NOT checked here.
    ///
    /// # Errors
    ///
    /// See [`TaskStore::deliver_inputs`](crate::store::TaskStore::deliver_inputs).
    pub async fn deliver_inputs(
        &self,
        task_id: &str,
        owner_id: &str,
        responses: Value,
    ) -> Result<Value, TaskError> {
        self.check_anonymous_access(owner_id)?;
        let delivered = Self::as_object(responses, "inputResponses")?;

        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        Self::check_owner(&record, task_id, owner_id, "deliver_inputs")?;
        Self::check_not_expired(&record, task_id)?;

        // Only a task that is AWAITING input may be fed, and the check runs
        // through the SHARED state machine rather than a hand-written match.
        // The predicate is exact: a transition to `Working` is legal from
        // `InputRequired` ALONE -- `Working -> Working` is rejected as a
        // self-transition and every terminal state is rejected outright. One
        // call to the transition validator therefore expresses the whole rule,
        // with no second predicate that could drift away from it.
        record
            .task
            .status
            .validate_transition(task_id, &TaskStatus::Working)?;

        let outstanding: BTreeSet<String> = record
            .input_requests
            .as_ref()
            .map(|requests| requests.keys().cloned().collect())
            .unwrap_or_default();

        // The accept/ignore/complete decision is the SHARED policy, imported from
        // `pmcp` rather than restated here. It reads key sets only, never values,
        // which is what lets this `serde_json`-shaped store and the typed in-crate
        // `InMemoryTaskStore` run the SAME decision procedure — the two copies had
        // already begun to drift before it was extracted. The closure's borrow of
        // `input_responses` ends with the call, leaving the persistence loop free
        // to take `&mut`.
        let delivery = partition_input_delivery(
            &outstanding,
            |wanted| {
                record
                    .input_responses
                    .as_ref()
                    .is_some_and(|answered| answered.contains_key(wanted))
            },
            delivered.keys().cloned(),
        );

        // Persist ONLY the accepted values; the ignored ones are reported back
        // and dropped.
        for (delivered_key, response) in delivered {
            if delivery.accepted.contains(&delivered_key) {
                record
                    .input_responses
                    .get_or_insert_with(Map::new)
                    .insert(delivered_key, response);
            }
        }

        // The `accepted` guard means a delivery that changed nothing cannot
        // resume a paused task: vacuous completeness would let a caller restart
        // a task using only keys the server never issued.
        if delivery.complete && !delivery.accepted.is_empty() {
            record.task.status = TaskStatus::Working;
        }
        Self::touch(&mut record);

        let bytes = Self::serialize_record(&record)?;
        self.backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        Ok(json!({
            "accepted": delivery.accepted,
            "ignored": delivery.ignored,
            "complete": delivery.complete,
        }))
    }

    /// Reads this task's input state — server-recorded requests, delivered
    /// responses and current status — owner-scoped.
    ///
    /// Delegates its read and its owner rule to [`Self::get`], so a mismatch
    /// reports `NotFound` through exactly the same branch as every other read.
    ///
    /// # Errors
    ///
    /// See
    /// [`TaskStore::task_input_snapshot`](crate::store::TaskStore::task_input_snapshot).
    pub async fn task_input_snapshot(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<Value, TaskError> {
        let record = self.get(task_id, owner_id).await?;

        // No recorded requests means there is no input state to snapshot. This
        // is deliberately NOT an empty snapshot: "the server never asked for
        // anything" and "the server asked for nothing in particular" are
        // different facts and the caller needs to tell them apart.
        let requests = record
            .input_requests
            .clone()
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        Ok(json!({
            "inputRequests": requests,
            "inputResponses": record.input_responses.clone().unwrap_or_default(),
            "status": record.task.status,
        }))
    }

    /// Records the SERVER-authored inputs this task needs and transitions it to
    /// `InputRequired`, in ONE compare-and-set write.
    ///
    /// # Multiple rounds
    ///
    /// A second round is permitted, but only once the previous round has been
    /// fully answered — the transition validator refuses while the task is still
    /// `input_required`, because `InputRequired -> InputRequired` is a rejected
    /// self-transition. New keys are MERGED into the recorded set; a key that is
    /// already recorded is REFUSED rather than overwritten, since overwriting it
    /// would orphan (or, on a superseding write, erase) the response already
    /// delivered against it. The spec requires keys to be unique over a task's
    /// lifetime, so a collision is a server bug, not a client-reachable state.
    ///
    /// This is the documented relaxation of the in-crate `pmcp`
    /// `InMemoryTaskStore`, which records exactly one round per task and refuses
    /// a second outright. That store is a dev/test store; this one backs
    /// production deployments where a multi-round elicitation is ordinary.
    ///
    /// # Errors
    ///
    /// See
    /// [`TaskStore::record_input_requests`](crate::store::TaskStore::record_input_requests).
    pub async fn record_input_requests(
        &self,
        task_id: &str,
        owner_id: &str,
        requests: Value,
    ) -> Result<Value, TaskError> {
        self.check_anonymous_access(owner_id)?;
        let new_requests = Self::as_object(requests, "inputRequests")?;

        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        Self::check_owner(&record, task_id, owner_id, "record_input_requests")?;
        Self::check_not_expired(&record, task_id)?;

        // The shared state machine IS the refusal for a terminal task and for a
        // task that is already awaiting input; there is no second predicate.
        record
            .task
            .status
            .validate_transition(task_id, &TaskStatus::InputRequired)?;

        if let Some(recorded) = record.input_requests.as_ref() {
            if let Some(duplicate) = new_requests.keys().find(|k| recorded.contains_key(*k)) {
                return Err(TaskError::StoreError(format!(
                    "input request key '{duplicate}' is already recorded for task {task_id}; \
                     keys must be unique over a task's lifetime"
                )));
            }
        }

        // Requests and transition land in the SAME write, so a task is never
        // observable as `input_required` with nothing recorded to answer.
        record
            .input_requests
            .get_or_insert_with(Map::new)
            .extend(new_requests);
        record.task.status = TaskStatus::InputRequired;
        Self::touch(&mut record);

        let bytes = Self::serialize_record(&record)?;
        let new_version = self
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;
        record.version = new_version;

        serde_json::to_value(record.to_wire_task_with_variables())
            .map_err(|e| TaskError::StoreError(format!("failed to serialize task: {e}")))
    }

    /// Persists the JSON-RPC error object for a failed task, in ONE
    /// compare-and-set write.
    ///
    /// The status is NOT changed here: a caller that is failing a task uses
    /// [`Self::complete_with_result`] or [`Self::update_status`] for the
    /// transition and this method for the error object, so the two concerns stay
    /// independently callable.
    ///
    /// # Errors
    ///
    /// See [`TaskStore::set_error`](crate::store::TaskStore::set_error).
    pub async fn set_error(
        &self,
        task_id: &str,
        owner_id: &str,
        error: Value,
    ) -> Result<(), TaskError> {
        let key = make_key(owner_id, task_id);
        let versioned = self
            .backend
            .get(&key)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        let mut record = Self::deserialize_record(&versioned.data)?;
        record.version = versioned.version;

        Self::check_owner(&record, task_id, owner_id, "set_error")?;
        Self::check_not_expired(&record, task_id)?;

        record.error = Some(error);
        Self::touch(&mut record);

        let bytes = Self::serialize_record(&record)?;
        self.backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .map_err(|e| Self::map_storage_error(e, task_id))?;

        Ok(())
    }

    /// Retrieves the persisted JSON-RPC error object for a task, owner-scoped.
    ///
    /// # Errors
    ///
    /// See [`TaskStore::get_error`](crate::store::TaskStore::get_error).
    pub async fn get_error(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        let record = self.get(task_id, owner_id).await?;

        // A task that exists but recorded no error has none to return.
        record.error.ok_or_else(|| TaskError::NotFound {
            task_id: task_id.to_string(),
        })
    }

    /// Returns a reference to the store's configuration.
    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    /// Returns a reference to the underlying storage backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }
}

/// Names a JSON value's type for a diagnostic message.
///
/// Only the TYPE is named, never the value: a rejected seam payload may carry
/// client-supplied content and an error message is not the place to echo it.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::backend::VersionedRecord;
    use crate::store::memory::InMemoryBackend;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;

    /// Helper: creates a store with anonymous access enabled.
    fn test_store() -> GenericTaskStore<InMemoryBackend> {
        GenericTaskStore::new(InMemoryBackend::new())
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
    }

    /// Helper: creates a store with a specific max tasks limit.
    fn store_with_max_tasks(max: usize) -> GenericTaskStore<InMemoryBackend> {
        GenericTaskStore::new(InMemoryBackend::new()).with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(max)
                .with_allow_anonymous(true),
        )
    }

    // ---- Create tests ----

    #[tokio::test]
    async fn create_returns_working_task() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Working);
        assert_eq!(record.owner_id, "owner-1");
        assert_eq!(record.request_method, "tools/call");
        assert_eq!(record.task.poll_interval, Some(500));
        assert!(record.version > 0);
    }

    #[tokio::test]
    async fn create_applies_default_ttl() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.ttl, Some(3_600_000));
        assert!(record.expires_at.is_some());
    }

    #[tokio::test]
    async fn create_uses_explicit_ttl() {
        let store = test_store();
        let record = store
            .create("owner-1", "tools/call", Some(30_000))
            .await
            .unwrap();
        assert_eq!(record.task.ttl, Some(30_000));
    }

    #[tokio::test]
    async fn create_rejects_ttl_above_max() {
        let store = test_store();
        let result = store
            .create("owner-1", "tools/call", Some(100_000_000))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("TTL"),
            "error should mention TTL: {err}"
        );
    }

    #[tokio::test]
    async fn create_rejects_anonymous_when_disabled() {
        let store = GenericTaskStore::new(InMemoryBackend::new()); // allow_anonymous = false
        let result = store.create("local", "tools/call", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("anonymous access"));
    }

    #[tokio::test]
    async fn create_enforces_max_tasks_per_owner() {
        let store = store_with_max_tasks(3);
        for i in 0..3 {
            store
                .create("owner-1", &format!("tools/call-{i}"), None)
                .await
                .unwrap();
        }
        let result = store.create("owner-1", "tools/call-extra", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskError::ResourceExhausted { suggested_action } => {
                assert!(suggested_action.is_some());
            },
            other => panic!("expected ResourceExhausted, got: {other}"),
        }
    }

    // ---- Get + owner isolation tests ----

    #[tokio::test]
    async fn get_returns_created_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let fetched = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert_eq!(fetched.task.task_id, created.task.task_id);
        assert_eq!(fetched.owner_id, "owner-1");
    }

    #[tokio::test]
    async fn owner_isolation_get_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B tries to access Owner A's task
        let result = store.get(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_returns_not_found_for_missing_task() {
        let store = test_store();
        let result = store.get("nonexistent", "owner-1").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // ---- State machine tests ----

    #[tokio::test]
    async fn update_status_valid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let updated = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("Done".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(updated.task.status, TaskStatus::Completed);
        assert_eq!(updated.task.status_message.as_deref(), Some("Done"));
    }

    #[tokio::test]
    async fn state_machine_completed_to_working_fails() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        let result = store
            .update_status(&created.task.task_id, "owner-1", TaskStatus::Working, None)
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    // ---- Variable merge tests ----

    #[tokio::test]
    async fn set_variables_upsert_and_null_deletion() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Set initial variables
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        vars.insert("key2".to_string(), json!(42));
        let updated = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();
        assert_eq!(updated.variables.get("key1").unwrap(), &json!("value1"));
        assert_eq!(updated.variables.get("key2").unwrap(), &json!(42));

        // Delete key1 via null, upsert key3
        let mut vars2 = HashMap::new();
        vars2.insert("key1".to_string(), Value::Null);
        vars2.insert("key3".to_string(), json!("new"));
        let updated2 = store
            .set_variables(&created.task.task_id, "owner-1", vars2)
            .await
            .unwrap();
        assert!(!updated2.variables.contains_key("key1"));
        assert_eq!(updated2.variables.get("key2").unwrap(), &json!(42));
        assert_eq!(updated2.variables.get("key3").unwrap(), &json!("new"));
    }

    #[tokio::test]
    async fn set_variables_size_exceeded() {
        let store = GenericTaskStore::new(InMemoryBackend::new())
            .with_config(StoreConfig {
                max_variable_size_bytes: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let mut vars = HashMap::new();
        vars.insert("big".to_string(), json!("x".repeat(200)));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(
            result,
            Err(TaskError::VariableSizeExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn set_variables_depth_bomb_rejected() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Build a depth-11 nested object (exceeds default max_depth of 10)
        let mut value = json!(1);
        for _ in 0..11 {
            value = json!({"nested": value});
        }
        let mut vars = HashMap::new();
        vars.insert("bomb".to_string(), value);
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("depth"));
    }

    #[tokio::test]
    async fn set_variables_long_string_rejected() {
        let store = GenericTaskStore::new(InMemoryBackend::new())
            .with_config(StoreConfig {
                max_string_length: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let mut vars = HashMap::new();
        vars.insert("long".to_string(), json!("x".repeat(200)));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("string value length"));
    }

    // ---- TTL rejection test ----

    #[tokio::test]
    async fn ttl_rejection_not_clamping() {
        let store = GenericTaskStore::new(InMemoryBackend::new())
            .with_config(StoreConfig {
                max_ttl_ms: Some(60_000),
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let result = store.create("owner-1", "tools/call", Some(120_000)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("TTL"));
    }

    // ---- CAS conflict test ----

    #[tokio::test]
    async fn cas_conflict_returns_concurrent_modification() {
        let backend = Arc::new(InMemoryBackend::new());
        let store = GenericTaskStore {
            backend: CasConflictBackend {
                inner: backend.clone(),
            },
            config: StoreConfig::default(),
            security: TaskSecurityConfig::default().with_allow_anonymous(true),
            default_poll_interval: 500,
        };

        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // CasConflictBackend always returns VersionConflict on put_if_version
        let result = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(
            matches!(result, Err(TaskError::ConcurrentModification { .. })),
            "expected ConcurrentModification, got: {result:?}"
        );
    }

    /// Backend wrapper that makes put_if_version always fail with VersionConflict.
    #[derive(Debug)]
    struct CasConflictBackend {
        inner: Arc<InMemoryBackend>,
    }

    #[async_trait]
    impl StorageBackend for CasConflictBackend {
        async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
            self.inner.get(key).await
        }
        async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
            self.inner.put(key, data).await
        }
        async fn put_if_version(
            &self,
            key: &str,
            _data: &[u8],
            expected_version: u64,
        ) -> Result<u64, StorageError> {
            Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: expected_version + 1,
            })
        }
        async fn delete(&self, key: &str) -> Result<bool, StorageError> {
            self.inner.delete(key).await
        }
        async fn list_by_prefix(
            &self,
            prefix: &str,
        ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
            self.inner.list_by_prefix(prefix).await
        }
        async fn cleanup_expired(&self) -> Result<usize, StorageError> {
            self.inner.cleanup_expired().await
        }
    }

    // ---- Complete with result tests ----

    #[tokio::test]
    async fn complete_with_result_atomic() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let completed = store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("All done".to_string()),
                json!({"data": true}),
            )
            .await
            .unwrap();
        assert_eq!(completed.task.status, TaskStatus::Completed);
        assert_eq!(completed.task.status_message.as_deref(), Some("All done"));
        assert_eq!(completed.result, Some(json!({"data": true})));
    }

    #[tokio::test]
    async fn complete_with_result_rejects_invalid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
                json!("first"),
            )
            .await
            .unwrap();

        let result = store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Failed,
                None,
                json!("second"),
            )
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    // ---- List pagination tests ----

    #[tokio::test]
    async fn list_pagination() {
        let store = test_store();

        // Create 3 tasks with small delays for ordering
        let mut task_ids = Vec::new();
        for _ in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            let record = store.create("owner-1", "tools/call", None).await.unwrap();
            task_ids.push(record.task.task_id);
        }

        // First page: limit 2
        let page1 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: None,
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page1.tasks.len(), 2);
        assert!(page1.next_cursor.is_some());

        // Second page using cursor
        let page2 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: page1.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page2.tasks.len(), 1);
        assert!(page2.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_scoped_to_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-b", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-a".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(page.tasks.len(), 2);
        assert!(page.tasks.iter().all(|t| t.owner_id == "owner-a"));
    }

    // ---- Cleanup expired test ----

    #[tokio::test]
    async fn cleanup_expired_delegates_to_backend() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force the record to be expired by rewriting with past expiry
        let key = make_key("owner-1", &created.task.task_id);
        let versioned = store.backend.get(&key).await.unwrap();
        let mut record =
            GenericTaskStore::<InMemoryBackend>::deserialize_record(&versioned.data).unwrap();
        record.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        let bytes = GenericTaskStore::<InMemoryBackend>::serialize_record(&record).unwrap();
        store
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .unwrap();

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
    }

    // ---- Serialization round-trip test ----

    #[tokio::test]
    async fn serialization_round_trip() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Set some variables
        let mut vars = HashMap::new();
        vars.insert("progress".to_string(), json!(42));
        store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();

        // Retrieve and verify all fields match
        let fetched = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert_eq!(fetched.task.task_id, created.task.task_id);
        assert_eq!(fetched.owner_id, created.owner_id);
        assert_eq!(fetched.request_method, created.request_method);
        assert_eq!(fetched.variables.get("progress").unwrap(), &json!(42));
        assert_eq!(fetched.task.ttl, Some(60_000));
        assert!(fetched.expires_at.is_some());
    }

    // ---- get_result tests ----

    #[tokio::test]
    async fn get_result_from_completed_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
                json!({"output": "done"}),
            )
            .await
            .unwrap();

        let result = store
            .get_result(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(result, json!({"output": "done"}));
    }

    #[tokio::test]
    async fn get_result_not_ready_for_working_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let result = store.get_result(&created.task.task_id, "owner-1").await;
        assert!(matches!(result, Err(TaskError::NotReady { .. })));
    }

    // ---- Cancel test ----

    #[tokio::test]
    async fn cancel_transitions_to_cancelled() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let cancelled = store
            .cancel(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    }

    // ---- Config accessor test ----

    #[test]
    fn config_returns_store_config() {
        let store = GenericTaskStore::new(InMemoryBackend::new()).with_config(StoreConfig {
            max_variable_size_bytes: 999,
            ..StoreConfig::default()
        });
        assert_eq!(store.config().max_variable_size_bytes, 999);
    }

    // ---- set_result test ----

    #[tokio::test]
    async fn set_result_stores_value() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        store
            .set_result(&created.task.task_id, "owner-1", json!({"answer": 42}))
            .await
            .unwrap();

        let record = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert_eq!(record.result, Some(json!({"answer": 42})));
    }

    // ---- Expired task mutation rejection tests ----

    #[tokio::test]
    async fn update_status_rejects_expired_task() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry by rewriting
        let key = make_key("owner-1", &created.task.task_id);
        let versioned = store.backend.get(&key).await.unwrap();
        let mut record =
            GenericTaskStore::<InMemoryBackend>::deserialize_record(&versioned.data).unwrap();
        record.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        let bytes = GenericTaskStore::<InMemoryBackend>::serialize_record(&record).unwrap();
        store
            .backend
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .unwrap();

        let result = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    // ---- Type-erasure test: GenericTaskStore as Arc<dyn TaskStore> ----

    #[tokio::test]
    async fn generic_task_store_as_dyn_task_store() {
        use crate::store::TaskStore;

        let backend = InMemoryBackend::new();
        let store = GenericTaskStore::new(backend)
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
        let dyn_store: Arc<dyn TaskStore> = Arc::new(store);

        // Use through trait interface
        let record = dyn_store
            .create("owner", "tools/call", Some(60_000))
            .await
            .unwrap();
        let fetched = dyn_store.get(&record.task.task_id, "owner").await.unwrap();
        assert_eq!(fetched.task.task_id, record.task.task_id);

        // Set variables through trait
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("value"));
        let updated = dyn_store
            .set_variables(&record.task.task_id, "owner", vars)
            .await
            .unwrap();
        assert_eq!(updated.variables.get("key").unwrap(), &json!("value"));

        // Complete through trait
        let completed = dyn_store
            .complete_with_result(
                &record.task.task_id,
                "owner",
                TaskStatus::Completed,
                None,
                json!({"done": true}),
            )
            .await
            .unwrap();
        assert_eq!(completed.task.status, TaskStatus::Completed);

        // Get result through trait
        let result = dyn_store
            .get_result(&record.task.task_id, "owner")
            .await
            .unwrap();
        assert_eq!(result, json!({"done": true}));

        // Config through trait
        assert_eq!(dyn_store.config().max_variable_size_bytes, 1_048_576);
    }
}
