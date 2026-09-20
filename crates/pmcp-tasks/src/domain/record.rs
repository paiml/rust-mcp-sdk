//! Task record -- the store's internal representation of a task.
//!
//! [`TaskRecord`] wraps the wire-format [`Task`] with additional fields
//! needed for storage, ownership, variables, and TTL expiration.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::types::task::{Task, TaskStatus};

/// Internal storage representation of a task.
///
/// A `TaskRecord` holds the wire-format [`Task`] along with domain-specific
/// fields that are not part of the MCP wire protocol: `owner_id` for access
/// control, `variables` for shared state, `result` for the operation outcome,
/// `request_method` for auditing, and `expires_at` for efficient TTL checks.
///
/// All fields are public so that store implementors have full access.
///
/// # Construction
///
/// Use [`TaskRecord::new`] to create a new record with a generated UUID task
/// ID and computed expiry:
///
/// ```
/// use pmcp_tasks::domain::TaskRecord;
///
/// let record = TaskRecord::new(
///     "session-abc".to_string(),
///     "tools/call".to_string(),
///     Some(60_000),
/// );
/// assert!(!record.task.task_id.is_empty());
/// assert_eq!(record.owner_id, "session-abc");
/// assert!(record.expires_at.is_some());
/// ```
///
/// # Source compatibility
///
/// This type is `#[non_exhaustive]`. Fields are added to it as the protocol
/// grows (the v2 tasks extension added `input_requests`, `input_responses` and
/// `error`), and every such addition would otherwise be a source break for any
/// downstream crate that built a `TaskRecord` with a struct literal. Construct
/// it through [`TaskRecord::new`] and mutate the fields you need; that route is
/// stable across field additions. `#[serde(default)]` on the added fields keeps
/// the PERSISTED representation compatible in the other direction — a record
/// written by a pre-v2 build still deserializes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TaskRecord {
    /// The wire-format task (serialized as-is for MCP responses).
    pub task: Task,

    /// Identifier of the session or client that owns this task.
    pub owner_id: String,

    /// Shared variable store for this task. Both client and server can
    /// read and write variables. Variables are injected into the wire
    /// task's `_meta` field at the serialization boundary.
    pub variables: HashMap<String, Value>,

    /// The operation result, set when the task reaches a terminal state.
    pub result: Option<Value>,

    /// The MCP method that created this task (e.g., `"tools/call"`).
    pub request_method: String,

    /// Computed absolute expiry time based on TTL. `None` means the task
    /// does not expire (unlimited TTL). Serialized as ISO 8601 via
    /// chrono's serde support.
    pub expires_at: Option<DateTime<Utc>>,

    /// The inputs the SERVER asked the client for, keyed by the
    /// server-assigned key. Written only by
    /// [`GenericTaskStore::record_input_requests`](crate::store::generic::GenericTaskStore::record_input_requests),
    /// never from anything a client sent: this is the only trustworthy record
    /// of which KIND was asked for under each key, and a kind-directed decode
    /// of the client's answers reads it back. If a client could influence it, a
    /// client could choose how its own answer is typed.
    ///
    /// `None` and `Some(empty)` are distinct: `None` means the server has never
    /// asked for anything, `Some(empty)` means it recorded a round that asked
    /// for nothing.
    ///
    /// Absent in records written before the v2 tasks extension landed, so it
    /// carries `#[serde(default)]` (absent means empty) and is omitted from the
    /// serialized bytes while untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_requests: Option<Map<String, Value>>,

    /// The responses delivered against [`Self::input_requests`], keyed
    /// identically. Absent until the first delivery.
    ///
    /// Absent in records written before the v2 tasks extension landed; see
    /// [`Self::input_requests`] for the compatibility rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_responses: Option<Map<String, Value>>,

    /// The JSON-RPC error object (`code`/`message`/`data`) for a `failed` task,
    /// carried verbatim as a `Value` so it can be inlined on a v2 `tasks/get`
    /// without being re-typed and re-serialized on the way through.
    ///
    /// Absent in records written before the v2 tasks extension landed; see
    /// [`Self::input_requests`] for the compatibility rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,

    /// Monotonic version for CAS operations. Not part of the serialized
    /// record -- managed by the storage backend.
    #[serde(skip)]
    pub version: u64,
}

impl TaskRecord {
    /// Creates a new task record in the `Working` state.
    ///
    /// Generates a `UUIDv4` task ID, sets timestamps to the current UTC time,
    /// and computes `expires_at` from `ttl` (milliseconds from now). If `ttl`
    /// is `None`, the task does not expire.
    ///
    /// # Arguments
    ///
    /// * `owner_id` - The session or client identifier that owns this task.
    /// * `request_method` - The MCP method name that created this task.
    /// * `ttl` - Time-to-live in milliseconds, or `None` for unlimited.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    ///
    /// let record = TaskRecord::new(
    ///     "session-1".to_string(),
    ///     "tools/call".to_string(),
    ///     Some(30_000),
    /// );
    ///
    /// assert_eq!(record.task.status, pmcp_tasks::TaskStatus::Working);
    /// assert_eq!(record.owner_id, "session-1");
    /// assert_eq!(record.request_method, "tools/call");
    /// assert!(record.variables.is_empty());
    /// assert!(record.result.is_none());
    /// assert!(record.expires_at.is_some());
    /// ```
    pub fn new(owner_id: String, request_method: String, ttl: Option<u64>) -> Self {
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        // Use checked arithmetic to avoid panics on extremely large TTL values.
        // If the TTL overflows i64 milliseconds or the resulting DateTime overflows,
        // we treat it as "never expires" (None).
        let expires_at = ttl.and_then(|ms| {
            let ms_i64 = i64::try_from(ms).ok()?;
            let duration = Duration::try_milliseconds(ms_i64)?;
            now.checked_add_signed(duration)
        });

        let task = Task {
            task_id: Uuid::new_v4().to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: now_str.clone(),
            last_updated_at: now_str,
            ttl,
            poll_interval: None,
            _meta: None,
        };

        Self {
            task,
            owner_id,
            variables: HashMap::new(),
            result: None,
            request_method,
            expires_at,
            input_requests: None,
            input_responses: None,
            error: None,
            version: 0,
        }
    }

    /// Returns `true` if the task has expired based on its TTL.
    ///
    /// A task with no `expires_at` (unlimited TTL) never expires.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    ///
    /// // Task with no TTL never expires
    /// let record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// assert!(!record.is_expired());
    /// ```
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Utc::now() > expiry,
            None => false,
        }
    }

    /// Returns a clone of the wire-format task without variable injection.
    ///
    /// Use this when you need the raw task without PMCP extensions.
    pub fn to_wire_task(&self) -> Task {
        self.task.clone()
    }

    /// Returns a clone of the wire-format task with variables injected
    /// into the `_meta` field.
    ///
    /// Variables are placed at the top level of `_meta` (not nested under
    /// a PMCP-specific key), per the locked design decision. If the task
    /// already has `_meta` entries, variables are merged in (variables take
    /// precedence on key conflict).
    ///
    /// If the variables map is empty, the existing `_meta` is returned
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    /// use serde_json::json;
    ///
    /// let mut record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// record.variables.insert("progress".to_string(), json!(42));
    ///
    /// let wire = record.to_wire_task_with_variables();
    /// let meta = wire._meta.expect("_meta should be present");
    /// assert_eq!(meta["progress"], json!(42));
    /// ```
    pub fn to_wire_task_with_variables(&self) -> Task {
        let mut task = self.task.clone();

        if self.variables.is_empty() {
            return task;
        }

        let meta = task._meta.get_or_insert_with(serde_json::Map::new);
        for (key, value) in &self.variables {
            meta.insert(key.clone(), value.clone());
        }

        task
    }
}

/// Validates that a JSON value does not exceed the maximum nesting depth.
///
/// Recursively walks the value. Arrays and objects increment the current
/// depth. Returns an error string if the depth exceeds `max_depth`.
///
/// # Arguments
///
/// * `value` - The JSON value to validate.
/// * `max_depth` - Maximum allowed nesting depth (e.g., 10).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::domain::record::validate_variable_depth;
/// use serde_json::json;
///
/// assert!(validate_variable_depth(&json!(42), 10).is_ok());
/// assert!(validate_variable_depth(&json!({"a": {"b": 1}}), 10).is_ok());
/// ```
pub fn validate_variable_depth(value: &Value, max_depth: usize) -> Result<(), String> {
    check_depth(value, 0, max_depth)
}

fn check_depth(value: &Value, current_depth: usize, max_depth: usize) -> Result<(), String> {
    if current_depth > max_depth {
        return Err(format!(
            "variable nesting depth {current_depth} exceeds maximum {max_depth}"
        ));
    }
    match value {
        Value::Array(arr) => {
            for item in arr {
                check_depth(item, current_depth + 1, max_depth)?;
            }
        },
        Value::Object(map) => {
            for v in map.values() {
                check_depth(v, current_depth + 1, max_depth)?;
            }
        },
        _ => {},
    }
    Ok(())
}

/// Validates that no string value within a JSON structure exceeds the
/// maximum byte length.
///
/// Recursively walks the value. Returns an error string if any string
/// value exceeds `max_length` bytes.
///
/// # Arguments
///
/// * `value` - The JSON value to validate.
/// * `max_length` - Maximum allowed string length in bytes (e.g., 65536).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::domain::record::validate_variable_string_lengths;
/// use serde_json::json;
///
/// assert!(validate_variable_string_lengths(&json!("short"), 65536).is_ok());
/// ```
pub fn validate_variable_string_lengths(value: &Value, max_length: usize) -> Result<(), String> {
    match value {
        Value::String(s) if s.len() > max_length => Err(format!(
            "string value length {} bytes exceeds maximum {max_length} bytes",
            s.len()
        )),
        Value::Array(arr) => {
            for item in arr {
                validate_variable_string_lengths(item, max_length)?;
            }
            Ok(())
        },
        Value::Object(map) => {
            for v in map.values() {
                validate_variable_string_lengths(v, max_length)?;
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

/// Validates all variable values for safety against depth bombs and
/// excessively long strings.
///
/// Iterates over each value in the `variables` map and applies both
/// [`validate_variable_depth`] and [`validate_variable_string_lengths`].
///
/// # Arguments
///
/// * `variables` - The variable map to validate.
/// * `max_depth` - Maximum allowed JSON nesting depth.
/// * `max_string_length` - Maximum allowed string value length in bytes.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::domain::record::validate_variables;
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// let mut vars = HashMap::new();
/// vars.insert("key".to_string(), json!({"nested": "value"}));
/// assert!(validate_variables(&vars, 10, 65536).is_ok());
/// ```
pub fn validate_variables(
    variables: &HashMap<String, Value>,
    max_depth: usize,
    max_string_length: usize,
) -> Result<(), String> {
    for (key, value) in variables {
        validate_variable_depth(value, max_depth).map_err(|e| format!("variable '{key}': {e}"))?;
        validate_variable_string_lengths(value, max_string_length)
            .map_err(|e| format!("variable '{key}': {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_record_has_uuid_task_id() {
        let record = TaskRecord::new(
            "owner-1".to_string(),
            "tools/call".to_string(),
            Some(60_000),
        );
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(record.task.task_id.len(), 36);
        assert!(record.task.task_id.contains('-'));
    }

    #[test]
    fn new_record_is_working_status() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.task.status, TaskStatus::Working);
    }

    #[test]
    fn new_record_timestamps_are_set() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(!record.task.created_at.is_empty());
        assert!(!record.task.last_updated_at.is_empty());
        assert_eq!(record.task.created_at, record.task.last_updated_at);
    }

    #[test]
    fn new_record_with_ttl_has_expiry() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        assert!(record.expires_at.is_some());
        assert_eq!(record.task.ttl, Some(60_000));
    }

    #[test]
    fn new_record_without_ttl_has_no_expiry() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(record.expires_at.is_none());
        assert!(record.task.ttl.is_none());
    }

    #[test]
    fn new_record_has_empty_variables_and_no_result() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(record.variables.is_empty());
        assert!(record.result.is_none());
    }

    #[test]
    fn is_expired_returns_false_for_no_ttl() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(!record.is_expired());
    }

    #[test]
    fn is_expired_returns_false_for_future_expiry() {
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            Some(60_000), // 60 seconds from now
        );
        assert!(!record.is_expired());
    }

    #[test]
    fn is_expired_returns_true_for_past_expiry() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(1));
        // Force the expiry into the past
        record.expires_at = Some(Utc::now() - Duration::seconds(10));
        assert!(record.is_expired());
    }

    #[test]
    fn to_wire_task_returns_clone() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let wire = record.to_wire_task();
        assert_eq!(wire.task_id, record.task.task_id);
        assert_eq!(wire.status, record.task.status);
    }

    #[test]
    fn to_wire_task_with_variables_empty_vars() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let wire = record.to_wire_task_with_variables();
        // No variables, so _meta should be unchanged (None)
        assert!(wire._meta.is_none());
    }

    #[test]
    fn to_wire_task_with_variables_injects_at_top_level() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record
            .variables
            .insert("server.progress".to_string(), json!(75));
        record
            .variables
            .insert("client.note".to_string(), json!("test note"));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        assert_eq!(meta["server.progress"], json!(75));
        assert_eq!(meta["client.note"], json!("test note"));
    }

    #[test]
    fn to_wire_task_with_variables_merges_existing_meta() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        // Set existing _meta
        let mut existing = serde_json::Map::new();
        existing.insert("existing_key".to_string(), json!("existing_value"));
        record.task._meta = Some(existing);

        // Add variable
        record
            .variables
            .insert("server.count".to_string(), json!(10));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        assert_eq!(meta["existing_key"], json!("existing_value"));
        assert_eq!(meta["server.count"], json!(10));
    }

    #[test]
    fn to_wire_task_with_variables_overwrites_on_conflict() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        // Set existing _meta with key that will conflict
        let mut existing = serde_json::Map::new();
        existing.insert("conflict_key".to_string(), json!("old_value"));
        record.task._meta = Some(existing);

        // Add variable with same key
        record
            .variables
            .insert("conflict_key".to_string(), json!("new_value"));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        // Variable takes precedence
        assert_eq!(meta["conflict_key"], json!("new_value"));
    }

    #[test]
    fn owner_id_and_request_method_preserved() {
        let record = TaskRecord::new("session-xyz".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.owner_id, "session-xyz");
        assert_eq!(record.request_method, "tools/call");
    }

    #[test]
    fn new_record_version_is_zero() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.version, 0);
    }

    // ---- Serialization round-trip tests ----

    #[test]
    fn serialization_round_trip() {
        let mut record = TaskRecord::new(
            "owner-1".to_string(),
            "tools/call".to_string(),
            Some(60_000),
        );
        record.variables.insert("progress".to_string(), json!(42));
        record.result = Some(json!({"status": "done"}));
        record.version = 5; // Set non-zero version to verify skip

        let bytes = serde_json::to_vec(&record).expect("serialization should succeed");
        let deserialized: TaskRecord =
            serde_json::from_slice(&bytes).expect("deserialization should succeed");

        assert_eq!(deserialized.task.task_id, record.task.task_id);
        assert_eq!(deserialized.owner_id, record.owner_id);
        assert_eq!(deserialized.request_method, record.request_method);
        assert_eq!(deserialized.task.status, record.task.status);
        assert_eq!(deserialized.variables, record.variables);
        assert_eq!(deserialized.result, record.result);
        assert_eq!(deserialized.task.ttl, record.task.ttl);
        // expires_at should round-trip (chrono serde)
        assert!(deserialized.expires_at.is_some());
    }

    #[test]
    fn version_not_in_serialized_json() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record.version = 42;

        let json_value = serde_json::to_value(&record).expect("serialization should succeed");
        assert!(
            json_value.get("version").is_none(),
            "version should not be present in serialized JSON"
        );
    }

    #[test]
    fn deserialized_version_defaults_to_zero() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let bytes = serde_json::to_vec(&record).expect("serialization should succeed");
        let deserialized: TaskRecord =
            serde_json::from_slice(&bytes).expect("deserialization should succeed");
        assert_eq!(
            deserialized.version, 0,
            "version should default to 0 on deserialization"
        );
    }

    #[test]
    fn serialization_uses_camel_case() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        let json_value = serde_json::to_value(&record).expect("serialization should succeed");
        // Check camelCase field names
        assert!(
            json_value.get("ownerId").is_some(),
            "should use camelCase ownerId"
        );
        assert!(
            json_value.get("requestMethod").is_some(),
            "should use camelCase requestMethod"
        );
        assert!(
            json_value.get("expiresAt").is_some(),
            "should use camelCase expiresAt"
        );
        // Verify snake_case is NOT used
        assert!(
            json_value.get("owner_id").is_none(),
            "should not use snake_case"
        );
        assert!(
            json_value.get("request_method").is_none(),
            "should not use snake_case"
        );
    }

    // ---- Variable validation tests ----

    #[test]
    fn validate_variable_depth_normal_values_pass() {
        assert!(validate_variable_depth(&json!(42), 10).is_ok());
        assert!(validate_variable_depth(&json!("hello"), 10).is_ok());
        assert!(validate_variable_depth(&json!(null), 10).is_ok());
        assert!(validate_variable_depth(&json!(true), 10).is_ok());
        assert!(validate_variable_depth(&json!({"a": 1}), 10).is_ok());
        assert!(validate_variable_depth(&json!([1, 2, 3]), 10).is_ok());
    }

    #[test]
    fn validate_variable_depth_nested_within_limit() {
        // depth 3: {a: {b: {c: 1}}}
        let value = json!({"a": {"b": {"c": 1}}});
        assert!(validate_variable_depth(&value, 10).is_ok());
    }

    #[test]
    fn validate_variable_depth_bomb_rejected() {
        // Build a depth-11 nested object
        let mut value = json!(1);
        for _ in 0..11 {
            value = json!({"nested": value});
        }
        let result = validate_variable_depth(&value, 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds maximum 10"));
    }

    #[test]
    fn validate_variable_depth_at_exact_limit_passes() {
        // Build exactly depth-10 nested object
        let mut value = json!(1);
        for _ in 0..10 {
            value = json!({"n": value});
        }
        assert!(validate_variable_depth(&value, 10).is_ok());
    }

    #[test]
    fn validate_variable_string_lengths_normal_pass() {
        assert!(validate_variable_string_lengths(&json!("short"), 65536).is_ok());
        assert!(validate_variable_string_lengths(&json!(42), 65536).is_ok());
        assert!(validate_variable_string_lengths(&json!(null), 65536).is_ok());
    }

    #[test]
    fn validate_variable_string_lengths_long_string_rejected() {
        let long_string = "x".repeat(65537);
        let value = json!(long_string);
        let result = validate_variable_string_lengths(&value, 65536);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds maximum 65536"));
    }

    #[test]
    fn validate_variable_string_lengths_nested_long_string_rejected() {
        let long_string = "x".repeat(65537);
        let value = json!({"nested": {"deep": long_string}});
        let result = validate_variable_string_lengths(&value, 65536);
        assert!(result.is_err());
    }

    #[test]
    fn validate_variables_mixed_valid_pass() {
        let mut vars = HashMap::new();
        vars.insert("int_val".to_string(), json!(42));
        vars.insert("str_val".to_string(), json!("hello"));
        vars.insert("obj_val".to_string(), json!({"a": 1}));
        vars.insert("arr_val".to_string(), json!([1, 2, 3]));
        vars.insert("null_val".to_string(), json!(null));
        assert!(validate_variables(&vars, 10, 65536).is_ok());
    }

    #[test]
    fn validate_variables_empty_map_passes() {
        let vars: HashMap<String, Value> = HashMap::new();
        assert!(validate_variables(&vars, 10, 65536).is_ok());
    }

    #[test]
    fn validate_variables_null_values_pass() {
        let mut vars = HashMap::new();
        vars.insert("null_key".to_string(), json!(null));
        assert!(validate_variables(&vars, 10, 65536).is_ok());
    }

    #[test]
    fn validate_variables_depth_bomb_detected() {
        let mut value = json!(1);
        for _ in 0..11 {
            value = json!({"n": value});
        }
        let mut vars = HashMap::new();
        vars.insert("bad".to_string(), value);
        let result = validate_variables(&vars, 10, 65536);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad"));
    }

    #[test]
    fn validate_variables_long_string_detected() {
        let long_string = "x".repeat(65537);
        let mut vars = HashMap::new();
        vars.insert("long".to_string(), json!(long_string));
        let result = validate_variables(&vars, 10, 65536);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("long"));
    }
}
