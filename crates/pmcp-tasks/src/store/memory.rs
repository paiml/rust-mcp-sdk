//! In-memory storage backend and task store.
//!
//! [`InMemoryBackend`] provides a thread-safe [`StorageBackend`] implementation
//! using `DashMap<String, (Vec<u8>, u64)>` for concurrent key-value storage.
//! It is a dumb KV store with no domain logic.
//!
//! [`InMemoryTaskStore`] is a thin wrapper around
//! [`GenericTaskStore<InMemoryBackend>`](crate::store::generic::GenericTaskStore)
//! that preserves the existing zero-argument `new()` constructor, builder methods,
//! and `Default` impl. All domain logic (state machine validation, owner isolation,
//! variable merge, TTL enforcement, CAS-based mutations) is handled by
//! `GenericTaskStore`.
//!
//! # Security
//!
//! Owner isolation is enforced structurally by `GenericTaskStore`: every
//! operation that takes an `owner_id` verifies that the record's owner
//! matches. On mismatch, the store returns [`TaskError::NotFound`] -- never
//! revealing that a task exists but belongs to someone else.
//!
//! # Concurrency
//!
//! `InMemoryBackend` uses `DashMap` for fine-grained shard-level locking.
//! Mutation operations use CAS (`put_if_version`) through `GenericTaskStore`
//! for optimistic concurrency control.
//!
//! # Examples
//!
//! ```
//! use pmcp_tasks::store::memory::InMemoryTaskStore;
//! use pmcp_tasks::store::{StoreConfig, TaskStore};
//! use pmcp_tasks::security::TaskSecurityConfig;
//!
//! let store = InMemoryTaskStore::new()
//!     .with_config(StoreConfig::default())
//!     .with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
//!     .with_poll_interval(3000);
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;

use crate::domain::TaskRecord;
use crate::error::TaskError;
use crate::security::TaskSecurityConfig;
use crate::store::backend::{StorageBackend, StorageError, VersionedRecord};
use crate::store::generic::GenericTaskStore;
use crate::types::task::TaskStatus;

use super::{ListTasksOptions, StoreConfig, TaskPage, TaskStore};

// ---- InMemoryBackend: dumb KV store using DashMap ----

/// Thread-safe in-memory storage backend using [`DashMap`].
///
/// Stores serialized task records as `(Vec<u8>, u64)` tuples where the
/// `u64` is a monotonic version number starting at 1. Keys are composite
/// strings in the format `{owner_id}:{task_id}`.
///
/// This backend contains **no domain logic**. All intelligence (state machine
/// validation, owner isolation, variable merge, TTL enforcement) lives in
/// [`GenericTaskStore`].
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::memory::InMemoryBackend;
/// use pmcp_tasks::store::generic::GenericTaskStore;
/// use pmcp_tasks::security::TaskSecurityConfig;
///
/// let backend = InMemoryBackend::new();
/// let store = GenericTaskStore::new(backend)
///     .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
/// ```
#[derive(Debug)]
pub struct InMemoryBackend {
    data: DashMap<String, (Vec<u8>, u64)>,
}

impl InMemoryBackend {
    /// Creates an empty in-memory backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryBackend;
    ///
    /// let backend = InMemoryBackend::new();
    /// assert!(backend.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }

    /// Returns the number of records stored.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryBackend;
    ///
    /// let backend = InMemoryBackend::new();
    /// assert_eq!(backend.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the backend contains no records.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryBackend;
    ///
    /// let backend = InMemoryBackend::new();
    /// assert!(backend.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        let entry = self.data.get(key).ok_or_else(|| StorageError::NotFound {
            key: key.to_string(),
        })?;
        let (data, version) = entry.value();
        Ok(VersionedRecord {
            data: data.clone(),
            version: *version,
        })
    }

    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        let new_version = self.data.get(key).map_or(1, |entry| entry.value().1 + 1);
        self.data
            .insert(key.to_string(), (data.to_vec(), new_version));
        Ok(new_version)
    }

    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        let mut entry = self
            .data
            .get_mut(key)
            .ok_or_else(|| StorageError::NotFound {
                key: key.to_string(),
            })?;
        let (ref _current_data, current_version) = *entry.value();
        if current_version != expected_version {
            return Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: current_version,
            });
        }
        let new_version = current_version + 1;
        *entry.value_mut() = (data.to_vec(), new_version);
        Ok(new_version)
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        Ok(self.data.remove(key).is_some())
    }

    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        let results: Vec<(String, VersionedRecord)> = self
            .data
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| {
                let (data, version) = entry.value();
                (
                    entry.key().clone(),
                    VersionedRecord {
                        data: data.clone(),
                        version: *version,
                    },
                )
            })
            .collect();
        Ok(results)
    }

    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        let keys_to_remove: Vec<String> = self
            .data
            .iter()
            .filter_map(|entry| {
                let (data, _) = entry.value();
                let record: TaskRecord = serde_json::from_slice(data).ok()?;
                if record.is_expired() {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();
        for key in &keys_to_remove {
            self.data.remove(key);
        }
        Ok(keys_to_remove.len())
    }
}

// ---- InMemoryTaskStore: thin wrapper around GenericTaskStore<InMemoryBackend> ----

/// Thread-safe in-memory task store using [`GenericTaskStore`] with [`InMemoryBackend`].
///
/// This is a thin wrapper that preserves the existing builder API while
/// delegating all domain logic to `GenericTaskStore`. Every [`TaskStore`]
/// method is implemented as a forwarding call to the inner store -- including
/// the input-delivery methods that carry a default, where a missing forwarding
/// line would compile and then silently report "not supported".
///
/// # Construction
///
/// Use the builder pattern to configure the store:
///
/// ```
/// use pmcp_tasks::store::memory::InMemoryTaskStore;
/// use pmcp_tasks::store::StoreConfig;
/// use pmcp_tasks::security::TaskSecurityConfig;
///
/// let store = InMemoryTaskStore::new()
///     .with_config(StoreConfig {
///         max_ttl_ms: Some(7_200_000), // 2 hours
///         ..StoreConfig::default()
///     })
///     .with_security(TaskSecurityConfig::default()
///         .with_max_tasks_per_owner(50));
/// ```
#[derive(Debug)]
pub struct InMemoryTaskStore {
    inner: GenericTaskStore<InMemoryBackend>,
}

impl InMemoryTaskStore {
    /// Creates a new in-memory task store with default configuration.
    ///
    /// Defaults:
    /// - `StoreConfig::default()` (1MB variables, 1h default TTL, 24h max TTL)
    /// - `TaskSecurityConfig::default()` (100 tasks/owner, no anonymous access)
    /// - Poll interval: 5000ms
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    ///
    /// let store = InMemoryTaskStore::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: GenericTaskStore::new(InMemoryBackend::new()).with_poll_interval(5000),
        }
    }

    /// Sets the storage configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// use pmcp_tasks::store::StoreConfig;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_config(StoreConfig {
    ///         max_variable_size_bytes: 512_000,
    ///         ..StoreConfig::default()
    ///     });
    /// ```
    pub fn with_config(mut self, config: StoreConfig) -> Self {
        self.inner = self.inner.with_config(config);
        self
    }

    /// Sets the security configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// use pmcp_tasks::security::TaskSecurityConfig;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_security(TaskSecurityConfig::default()
    ///         .with_max_tasks_per_owner(25));
    /// ```
    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self {
        self.inner = self.inner.with_security(security);
        self
    }

    /// Sets the default poll interval suggested to clients.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_poll_interval(3000);
    /// ```
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.inner = self.inner.with_poll_interval(ms);
        self
    }

    /// Returns a reference to the underlying backend.
    ///
    /// Useful for test code that needs to inspect backend state (e.g., record
    /// count, force-writing expired records).
    #[cfg(test)]
    pub fn backend(&self) -> &InMemoryBackend {
        self.inner.backend()
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---- TaskStore delegation impl ----

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError> {
        self.inner.create(owner_id, request_method, ttl).await
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        self.inner.get(task_id, owner_id).await
    }

    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError> {
        self.inner
            .update_status(task_id, owner_id, new_status, status_message)
            .await
    }

    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError> {
        self.inner.set_variables(task_id, owner_id, variables).await
    }

    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError> {
        self.inner.set_result(task_id, owner_id, result).await
    }

    async fn get_result(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.inner.get_result(task_id, owner_id).await
    }

    async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError> {
        self.inner
            .complete_with_result(task_id, owner_id, status, status_message, result)
            .await
    }

    async fn list(&self, options: ListTasksOptions) -> Result<TaskPage, TaskError> {
        self.inner.list(options).await
    }

    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<TaskRecord, TaskError> {
        self.inner.cancel(task_id, owner_id).await
    }

    async fn cleanup_expired(&self) -> Result<usize, TaskError> {
        self.inner.cleanup_expired().await
    }

    fn config(&self) -> &StoreConfig {
        self.inner.config()
    }

    // The five input-delivery methods are delegated for the same reason as the
    // eleven above, and this is the site most easily forgotten: they are the
    // only `TaskStore` methods with a DEFAULT body, so omitting a delegation
    // line here does not fail to compile -- it silently inherits "store does not
    // support task input delivery", and every memory-backed deployment reports
    // that while the identical DynamoDB and Redis paths work. `tests/input_delivery.rs`
    // makes that structural: it reads this block and the trait at runtime and
    // fails when a trait method has no delegation line.

    async fn deliver_inputs(
        &self,
        task_id: &str,
        owner_id: &str,
        responses: Value,
    ) -> Result<Value, TaskError> {
        self.inner
            .deliver_inputs(task_id, owner_id, responses)
            .await
    }

    async fn task_input_snapshot(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.inner.task_input_snapshot(task_id, owner_id).await
    }

    async fn record_input_requests(
        &self,
        task_id: &str,
        owner_id: &str,
        requests: Value,
    ) -> Result<Value, TaskError> {
        self.inner
            .record_input_requests(task_id, owner_id, requests)
            .await
    }

    async fn set_error(
        &self,
        task_id: &str,
        owner_id: &str,
        error: Value,
    ) -> Result<(), TaskError> {
        self.inner.set_error(task_id, owner_id, error).await
    }

    async fn get_error(&self, task_id: &str, owner_id: &str) -> Result<Value, TaskError> {
        self.inner.get_error(task_id, owner_id).await
    }
}

#[cfg(test)]
mod backend_tests {
    use super::*;
    use crate::store::backend::{StorageBackend, StorageError};

    // ---- get tests ----

    #[tokio::test]
    async fn get_missing_key_returns_not_found() {
        let backend = InMemoryBackend::new();
        let result = backend.get("nonexistent").await;
        assert!(
            matches!(&result, Err(StorageError::NotFound { key }) if key == "nonexistent"),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn get_returns_stored_data() {
        let backend = InMemoryBackend::new();
        let data = b"hello world";
        let version = backend.put("key-1", data).await.unwrap();

        let record = backend.get("key-1").await.unwrap();
        assert_eq!(record.data, data);
        assert_eq!(record.version, version);
    }

    // ---- put tests ----

    #[tokio::test]
    async fn put_new_key_returns_version_1() {
        let backend = InMemoryBackend::new();
        let version = backend.put("key-1", b"data").await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn put_existing_key_increments_version() {
        let backend = InMemoryBackend::new();
        let v1 = backend.put("key-1", b"first").await.unwrap();
        let v2 = backend.put("key-1", b"second").await.unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);

        let record = backend.get("key-1").await.unwrap();
        assert_eq!(record.data, b"second");
        assert_eq!(record.version, 2);
    }

    #[tokio::test]
    async fn put_overwrites_data() {
        let backend = InMemoryBackend::new();
        backend.put("key-1", b"original").await.unwrap();
        backend.put("key-1", b"updated").await.unwrap();

        let record = backend.get("key-1").await.unwrap();
        assert_eq!(record.data, b"updated");
    }

    // ---- put_if_version tests ----

    #[tokio::test]
    async fn put_if_version_succeeds_on_match() {
        let backend = InMemoryBackend::new();
        let v1 = backend.put("key-1", b"data-v1").await.unwrap();
        let v2 = backend
            .put_if_version("key-1", b"data-v2", v1)
            .await
            .unwrap();
        assert_eq!(v2, v1 + 1);
    }

    #[tokio::test]
    async fn put_if_version_fails_on_mismatch() {
        let backend = InMemoryBackend::new();
        backend.put("key-1", b"data").await.unwrap();

        let result = backend.put_if_version("key-1", b"new-data", 999).await;
        match result {
            Err(StorageError::VersionConflict {
                key,
                expected,
                actual,
            }) => {
                assert_eq!(key, "key-1");
                assert_eq!(expected, 999);
                assert_eq!(actual, 1);
            },
            other => panic!("expected VersionConflict, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn put_if_version_fails_on_missing_key() {
        let backend = InMemoryBackend::new();
        let result = backend.put_if_version("nonexistent", b"data", 1).await;
        assert!(
            matches!(&result, Err(StorageError::NotFound { key }) if key == "nonexistent"),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn put_if_version_updates_data() {
        let backend = InMemoryBackend::new();
        let v1 = backend.put("key-1", b"original").await.unwrap();
        backend
            .put_if_version("key-1", b"cas-updated", v1)
            .await
            .unwrap();

        let record = backend.get("key-1").await.unwrap();
        assert_eq!(record.data, b"cas-updated");
    }

    // ---- delete tests ----

    #[tokio::test]
    async fn delete_existing_returns_true() {
        let backend = InMemoryBackend::new();
        backend.put("key-1", b"data").await.unwrap();
        let deleted = backend.delete("key-1").await.unwrap();
        assert!(deleted);
    }

    #[tokio::test]
    async fn delete_missing_returns_false() {
        let backend = InMemoryBackend::new();
        let deleted = backend.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn delete_then_get_returns_not_found() {
        let backend = InMemoryBackend::new();
        backend.put("key-1", b"data").await.unwrap();
        backend.delete("key-1").await.unwrap();

        let result = backend.get("key-1").await;
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    // ---- list_by_prefix tests ----

    #[tokio::test]
    async fn list_by_prefix_returns_matching() {
        let backend = InMemoryBackend::new();
        backend.put("owner:a", b"data-a").await.unwrap();
        backend.put("owner:b", b"data-b").await.unwrap();
        backend.put("other:c", b"data-c").await.unwrap();

        let results = backend.list_by_prefix("owner:").await.unwrap();
        assert_eq!(results.len(), 2);
        let keys: Vec<&str> = results.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"owner:a"));
        assert!(keys.contains(&"owner:b"));
    }

    #[tokio::test]
    async fn list_by_prefix_empty_on_no_match() {
        let backend = InMemoryBackend::new();
        backend.put("owner:a", b"data").await.unwrap();

        let results = backend.list_by_prefix("nomatch:").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn list_by_prefix_returns_correct_data_and_versions() {
        let backend = InMemoryBackend::new();
        backend.put("owner:a", b"data-a").await.unwrap();
        let v2 = backend.put("owner:b", b"data-b").await.unwrap();
        // Update owner:b to get version 2
        let v2b = backend.put("owner:b", b"data-b-v2").await.unwrap();

        let results = backend.list_by_prefix("owner:").await.unwrap();
        assert_eq!(results.len(), 2);

        for (key, record) in &results {
            match key.as_str() {
                "owner:a" => {
                    assert_eq!(record.data, b"data-a");
                    assert_eq!(record.version, 1);
                },
                "owner:b" => {
                    assert_eq!(record.data, b"data-b-v2");
                    assert_eq!(record.version, v2b);
                },
                other => panic!("unexpected key: {other}"),
            }
        }
        // Suppress unused variable warning
        let _ = v2;
    }

    // ---- cleanup_expired tests ----

    #[tokio::test]
    async fn cleanup_expired_removes_expired_records() {
        let backend = InMemoryBackend::new();

        let mut record =
            TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        record.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        let bytes = serde_json::to_vec(&record).unwrap();
        backend.put("owner:task-expired", &bytes).await.unwrap();

        let removed = backend.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        let result = backend.get("owner:task-expired").await;
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_non_expired_records() {
        let backend = InMemoryBackend::new();

        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            Some(3_600_000),
        );
        let bytes = serde_json::to_vec(&record).unwrap();
        backend.put("owner:task-alive", &bytes).await.unwrap();

        let removed = backend.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);

        let result = backend.get("owner:task-alive").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cleanup_expired_returns_zero_on_empty() {
        let backend = InMemoryBackend::new();
        let removed = backend.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::backend::make_key;
    use serde_json::json;

    /// Helper: creates a store with anonymous access enabled.
    fn test_store() -> InMemoryTaskStore {
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
    }

    /// Helper: creates a store with a specific max tasks limit.
    fn store_with_max_tasks(max: usize) -> InMemoryTaskStore {
        InMemoryTaskStore::new().with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(max)
                .with_allow_anonymous(true),
        )
    }

    /// Helper: forces a task to be expired by rewriting the backend record
    /// with a past `expires_at` timestamp.
    async fn force_expire(store: &InMemoryTaskStore, owner_id: &str, task_id: &str) {
        let key = make_key(owner_id, task_id);
        let versioned = store.backend().get(&key).await.unwrap();
        let mut record: TaskRecord = serde_json::from_slice(&versioned.data).unwrap();
        record.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        let bytes = serde_json::to_vec(&record).unwrap();
        store
            .backend()
            .put_if_version(&key, &bytes, versioned.version)
            .await
            .unwrap();
    }

    // --- Constructor and builder tests ---

    #[test]
    fn new_creates_empty_store() {
        let store = InMemoryTaskStore::new();
        assert!(store.backend().is_empty());
    }

    #[test]
    fn default_delegates_to_new() {
        let store = InMemoryTaskStore::default();
        assert!(store.backend().is_empty());
    }

    #[tokio::test]
    async fn new_creates_store_with_5000ms_poll_interval() {
        let store = InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.poll_interval, Some(5000));
    }

    #[tokio::test]
    async fn default_creates_store_with_5000ms_poll_interval() {
        let store = InMemoryTaskStore::default()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.poll_interval, Some(5000));
    }

    #[test]
    fn with_config_sets_config() {
        let store = InMemoryTaskStore::new().with_config(StoreConfig {
            max_variable_size_bytes: 512_000,
            ..StoreConfig::default()
        });
        assert_eq!(store.config().max_variable_size_bytes, 512_000);
    }

    #[tokio::test]
    async fn with_security_sets_security() {
        // Verify the security config takes effect by checking behavior:
        // set max_tasks_per_owner to 2, create 2 tasks, third should fail
        let store = InMemoryTaskStore::new().with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(2)
                .with_allow_anonymous(true),
        );
        store.create("owner-1", "tools/call", None).await.unwrap();
        store.create("owner-1", "tools/call", None).await.unwrap();
        let result = store.create("owner-1", "tools/call", None).await;
        assert!(matches!(result, Err(TaskError::ResourceExhausted { .. })));
    }

    #[test]
    fn with_poll_interval_sets_interval() {
        // Verified via create in the async test below
        let _store = InMemoryTaskStore::new().with_poll_interval(3000);
    }

    #[tokio::test]
    async fn with_poll_interval_applied_to_created_tasks() {
        let store = InMemoryTaskStore::new()
            .with_poll_interval(3000)
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.poll_interval, Some(3000));
    }

    // --- Create tests ---

    #[tokio::test]
    async fn create_returns_working_task() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Working);
        assert_eq!(record.owner_id, "owner-1");
        assert_eq!(record.request_method, "tools/call");
        assert_eq!(record.task.poll_interval, Some(5000));
    }

    #[tokio::test]
    async fn create_applies_default_ttl() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        // Default TTL from StoreConfig is 3_600_000 (1 hour)
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
        let store = InMemoryTaskStore::new(); // allow_anonymous = false (default)
        let result = store.create("local", "tools/call", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("anonymous access"));
    }

    #[tokio::test]
    async fn create_rejects_empty_owner_when_anonymous_disabled() {
        let store = InMemoryTaskStore::new();
        let result = store.create("", "tools/call", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_allows_anonymous_when_enabled() {
        let store = test_store();
        let result = store.create("local", "tools/call", None).await;
        assert!(result.is_ok());
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

    #[tokio::test]
    async fn create_max_tasks_scoped_to_owner() {
        let store = store_with_max_tasks(2);
        // Owner A fills their quota
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B can still create
        let result = store.create("owner-b", "tools/call", None).await;
        assert!(result.is_ok());
    }

    // --- Get tests ---

    #[tokio::test]
    async fn get_returns_created_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let fetched = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert_eq!(fetched.task.task_id, created.task.task_id);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_missing_task() {
        let store = test_store();
        let result = store.get("nonexistent-id", "owner-1").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_returns_not_found_on_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B tries to access Owner A's task
        let result = store.get(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_returns_expired_task() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force the task to be expired
        force_expire(&store, "owner-1", &created.task.task_id).await;

        // Get should still succeed (expired tasks are readable)
        let fetched = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert!(fetched.is_expired());
    }

    // --- Update status tests ---

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
    async fn update_status_invalid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        // Complete the task first
        store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        // Try to transition from terminal state
        let result = store
            .update_status(&created.task.task_id, "owner-1", TaskStatus::Working, None)
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    #[tokio::test]
    async fn update_status_rejects_expired_task() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        force_expire(&store, "owner-1", &created.task.task_id).await;

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

    #[tokio::test]
    async fn update_status_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .update_status(
                &created.task.task_id,
                "owner-b",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Set variables tests ---

    #[tokio::test]
    async fn set_variables_upserts_values() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        vars.insert("key2".to_string(), json!(42));

        let updated = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();
        assert_eq!(updated.variables.get("key1").unwrap(), &json!("value1"));
        assert_eq!(updated.variables.get("key2").unwrap(), &json!(42));
    }

    #[tokio::test]
    async fn set_variables_null_deletes_key() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Set initial variable
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();

        // Delete via null
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), Value::Null);
        let updated = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();
        assert!(!updated.variables.contains_key("key1"));
    }

    #[tokio::test]
    async fn set_variables_rejects_oversized_payload() {
        let store = InMemoryTaskStore::new()
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
    async fn set_variables_checks_merged_size() {
        let store = InMemoryTaskStore::new()
            .with_config(StoreConfig {
                max_variable_size_bytes: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // First set: small enough
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!("x".repeat(30)));
        store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();

        // Second set: combined exceeds limit
        let mut vars = HashMap::new();
        vars.insert("b".to_string(), json!("y".repeat(60)));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(
            result,
            Err(TaskError::VariableSizeExceeded { .. })
        ));

        // Verify original variables are unchanged (clone-check-commit)
        let record = store.get(&created.task.task_id, "owner-1").await.unwrap();
        assert!(record.variables.contains_key("a"));
        assert!(!record.variables.contains_key("b"));
    }

    #[tokio::test]
    async fn set_variables_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("val"));
        let result = store
            .set_variables(&created.task.task_id, "owner-b", vars)
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn set_variables_rejects_expired() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        force_expire(&store, "owner-1", &created.task.task_id).await;

        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("val"));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    // --- Set result tests ---

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

    #[tokio::test]
    async fn set_result_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .set_result(&created.task.task_id, "owner-b", json!("data"))
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Get result tests ---

    #[tokio::test]
    async fn get_result_from_completed_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Complete with result
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

    #[tokio::test]
    async fn get_result_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-a",
                TaskStatus::Completed,
                None,
                json!("result"),
            )
            .await
            .unwrap();
        let result = store.get_result(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Complete with result tests ---

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

        // Complete first
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

        // Try again from terminal state
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

    // --- List tests ---

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

    #[tokio::test]
    async fn list_sorted_newest_first() {
        let store = test_store();
        let first = store.create("owner-1", "tools/call", None).await.unwrap();

        // Ensure second task has a strictly later timestamp
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let second = store.create("owner-1", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(page.tasks.len(), 2);
        // Newest first
        assert!(page.tasks[0].task.created_at >= page.tasks[1].task.created_at);
        // The second task was created later so should appear first
        assert_eq!(page.tasks[1].task.task_id, first.task.task_id);
        assert_eq!(page.tasks[0].task.task_id, second.task.task_id);
    }

    #[tokio::test]
    async fn list_pagination() {
        let store = test_store();

        // Create 5 tasks
        let mut task_ids = Vec::new();
        for _ in 0..5 {
            let record = store.create("owner-1", "tools/call", None).await.unwrap();
            task_ids.push(record.task.task_id);
        }

        // First page (limit 2)
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
        assert_eq!(page2.tasks.len(), 2);
        assert!(page2.next_cursor.is_some());

        // Third page (last)
        let page3 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: page2.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page3.tasks.len(), 1);
        assert!(page3.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_empty_for_unknown_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-b".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert!(page.tasks.is_empty());
        assert!(page.next_cursor.is_none());
    }

    // --- Cancel tests ---

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

    #[tokio::test]
    async fn cancel_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store.cancel(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Cleanup expired tests ---

    #[tokio::test]
    async fn cleanup_expired_removes_expired_tasks() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        force_expire(&store, "owner-1", &created.task.task_id).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.backend().len(), 0);
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_non_expired() {
        let store = test_store();
        store
            .create("owner-1", "tools/call", Some(3_600_000))
            .await
            .unwrap();
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.backend().len(), 1);
    }

    // --- Config accessor test ---

    #[tokio::test]
    async fn config_returns_store_config() {
        let store = InMemoryTaskStore::new().with_config(StoreConfig {
            max_variable_size_bytes: 999,
            ..StoreConfig::default()
        });
        assert_eq!(store.config().max_variable_size_bytes, 999);
    }
}
