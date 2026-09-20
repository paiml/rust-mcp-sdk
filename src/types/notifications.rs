//! Notification types for MCP protocol.
//!
//! This module contains notification-related types including progress,
//! cancellation, logging, and resource update notifications.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Progress notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    /// Progress token from the original request
    pub progress_token: ProgressToken,
    /// Current progress value (must increase with each notification)
    ///
    /// This can represent percentage (0-100), count, or any increasing metric.
    pub progress: f64,
    /// Optional total value for the operation
    ///
    /// When combined with `progress`, allows expressing "5 of 10 items processed".
    /// Both `progress` and `total` may be floating point values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Optional human-readable progress message
    ///
    /// Should provide relevant context about the current operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ProgressNotification {
    /// Create a new progress notification with no total value.
    ///
    /// Convenience constructor to reduce boilerplate when the total is unknown.
    pub fn new(progress_token: ProgressToken, progress: f64, message: Option<String>) -> Self {
        Self {
            progress_token,
            progress,
            total: None,
            message,
        }
    }

    /// Set the total value for the operation.
    pub fn with_total(mut self, total: f64) -> Self {
        self.total = Some(total);
        self
    }
}

/// Progress token type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProgressToken {
    /// String token
    String(String),
    /// Numeric token
    Number(i64),
}

/// Client notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientNotification {
    /// Notification that client has been initialized
    #[serde(rename = "notifications/initialized")]
    Initialized,
    /// Notification that roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Notification that a request was cancelled
    #[serde(rename = "notifications/cancelled")]
    Cancelled(CancelledNotification),
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(ProgressNotification),
}

/// Cancelled notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotification {
    /// The request ID that was cancelled
    pub request_id: super::RequestId,
    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl CancelledNotification {
    /// Create a cancelled notification for the given request ID.
    ///
    /// `reason` defaults to `None`.
    pub fn new(request_id: super::RequestId) -> Self {
        Self {
            request_id,
            reason: None,
        }
    }

    /// Set the reason for cancellation.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Server notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerNotification {
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(ProgressNotification),
    /// Tools have changed
    #[serde(rename = "notifications/tools/list_changed")]
    ToolsChanged,
    /// Prompts have changed
    #[serde(rename = "notifications/prompts/list_changed")]
    PromptsChanged,
    /// Resources have changed
    #[serde(rename = "notifications/resources/list_changed")]
    ResourcesChanged,
    /// Roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Resource was updated
    #[serde(rename = "notifications/resources/updated")]
    ResourceUpdated(ResourceUpdatedParams),
    /// Log message
    #[serde(rename = "notifications/message")]
    LogMessage(LogMessageParams),
    /// Task status changed (MCP 2025-11-25)
    #[serde(rename = "notifications/tasks/status")]
    TaskStatus(super::tasks::TaskStatusNotification),
}

/// Resource updated notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdatedParams {
    /// Resource URI that was updated
    pub uri: String,
}

impl ResourceUpdatedParams {
    /// Create a resource updated notification for the given URI.
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

/// Log message notification.
///
/// # The wire contract pmcp emits
///
/// The MCP schema declares `LoggingMessageNotificationParams` as
/// `{ level, logger?, data }` — **`data` is REQUIRED and there is no `message`
/// member at all** (`schema/vendored/core-2026-07-28/schema.ts`). This Rust type
/// is the reverse shape: a required `message` and an optional `data`. The two are
/// reconciled at the EMITTER, not here:
/// [`RequestHandlerExtra::log`](crate::RequestHandlerExtra::log) defaults `data`
/// to the message string when the caller supplied none, so **every frame pmcp
/// emits carries `data`**, and `message` rides alongside it as a pmcp extension.
/// When the caller used
/// [`log_with_data`](crate::RequestHandlerExtra::log_with_data), their value is
/// passed through verbatim.
///
/// The reason, so this is not "simplified" back: the official reference client's
/// `LoggingMessageNotificationParamsSchema` uses `z.unknown()`, which is
/// **non-optional under the bundled zod v4**. A frame with no `data` fails to
/// parse (`invalid_type at params.data: expected nonoptional, received
/// undefined`) and the client drops it silently — measured against the pinned
/// conformance suite, where it cost `2025-11-25:tools-call-with-logging` its
/// score and tripped the suite's `WireSchemaValid` check with
/// `LoggingMessageNotification/params: must have required property 'data'`.
///
/// Keeping `message` is legal: the schema does not close
/// `additionalProperties`, and the reference client strips unknown members rather
/// than rejecting them. It is also what keeps this a non-breaking fix — removing
/// `message` would be a breaking change to a public type, and was rejected in
/// favour of defaulting `data`.
///
/// Constructing this type directly (rather than through the emitter) does NOT get
/// the default: [`new`](Self::new) leaves `data` as `None`, so a caller building
/// the params by hand should call [`with_data`](Self::with_data).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct LogMessageParams {
    /// Log level
    pub level: LoggingLevel,
    /// Logger name/category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// Log message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl LogMessageParams {
    /// Create a log message with level and message.
    ///
    /// `logger` and `data` default to `None`.
    pub fn new(level: LoggingLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            logger: None,
            message: message.into(),
            data: None,
        }
    }

    /// Set the logger name/category.
    pub fn with_logger(mut self, logger: impl Into<String>) -> Self {
        self.logger = Some(logger.into());
        self
    }

    /// Set additional data.
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Combined notification types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Notification {
    /// Client notification
    Client(ClientNotification),
    /// Server notification
    Server(ServerNotification),
    /// Progress notification
    Progress(ProgressNotification),
    /// Cancelled notification
    Cancelled(CancelledNotification),
}

/// Logging level (MCP 2025-11-25 -- full syslog severity).
///
/// # The ordering derive is load-bearing
///
/// The variants below are declared in SYSLOG SEVERITY ORDER, least severe
/// first. `#[derive(PartialOrd, Ord)]` on a fieldless enum compares by
/// declaration order, so the derive yields the correct severity comparison for
/// free: `Emergency > Alert > Critical > Error > Warning > Notice > Info >
/// Debug`. Level filtering (see
/// [`RequestHandlerExtra::log`](crate::RequestHandlerExtra::log)) is therefore
/// spelled `emitted >= configured` on the typed value.
///
/// **Never compare the serialized strings.** Lexically `"critical" < "debug"`,
/// so a string-based `>= "debug"` filter would SUPPRESS `debug` records while
/// letting `critical` through — exactly backwards. `tests/log_emitter.rs`
/// fences all 64 `(configured, emitted)` pairs, including that inversion.
///
/// **Do not reorder the variants.** The declaration order IS the semantics; a
/// reorder silently changes every level filter in every downstream server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    /// Debug messages
    Debug,
    /// Informational messages
    Info,
    /// Notice-level messages
    Notice,
    /// Warnings
    Warning,
    /// Errors
    Error,
    /// Critical errors
    Critical,
    /// Alerts requiring immediate action
    Alert,
    /// System emergency
    Emergency,
}

/// Deprecated: Use [`LoggingLevel`] instead.
///
/// This type alias is provided for backward compatibility during
/// the v2.0 transition. It will be removed in a future release.
pub type LogLevel = LoggingLevel;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_all_notification_types() {
        let progress = ServerNotification::Progress(ProgressNotification::new(
            ProgressToken::String("token123".to_string()),
            50.0,
            Some("Processing...".to_string()),
        ));
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["method"], "notifications/progress");

        let tools_changed = ServerNotification::ToolsChanged;
        let json = serde_json::to_value(&tools_changed).unwrap();
        assert_eq!(json["method"], "notifications/tools/list_changed");

        let prompts_changed = ServerNotification::PromptsChanged;
        let json = serde_json::to_value(&prompts_changed).unwrap();
        assert_eq!(json["method"], "notifications/prompts/list_changed");

        let resources_changed = ServerNotification::ResourcesChanged;
        let json = serde_json::to_value(&resources_changed).unwrap();
        assert_eq!(json["method"], "notifications/resources/list_changed");

        let roots_changed = ServerNotification::RootsListChanged;
        let json = serde_json::to_value(&roots_changed).unwrap();
        assert_eq!(json["method"], "notifications/roots/list_changed");

        let resource_updated =
            ServerNotification::ResourceUpdated(ResourceUpdatedParams::new("file://test.txt"));
        let json = serde_json::to_value(&resource_updated).unwrap();
        assert_eq!(json["method"], "notifications/resources/updated");

        let log_msg = ServerNotification::LogMessage(
            LogMessageParams::new(LoggingLevel::Info, "Test log message")
                .with_data(json!({"extra": "data"})),
        );
        let json = serde_json::to_value(&log_msg).unwrap();
        assert_eq!(json["method"], "notifications/message");
    }

    #[test]
    fn test_logging_level_all_8_values() {
        assert_eq!(serde_json::to_value(LoggingLevel::Debug).unwrap(), "debug");
        assert_eq!(serde_json::to_value(LoggingLevel::Info).unwrap(), "info");
        assert_eq!(
            serde_json::to_value(LoggingLevel::Notice).unwrap(),
            "notice"
        );
        assert_eq!(
            serde_json::to_value(LoggingLevel::Warning).unwrap(),
            "warning"
        );
        assert_eq!(serde_json::to_value(LoggingLevel::Error).unwrap(), "error");
        assert_eq!(
            serde_json::to_value(LoggingLevel::Critical).unwrap(),
            "critical"
        );
        assert_eq!(serde_json::to_value(LoggingLevel::Alert).unwrap(), "alert");
        assert_eq!(
            serde_json::to_value(LoggingLevel::Emergency).unwrap(),
            "emergency"
        );
    }

    #[test]
    fn test_log_level_alias_works() {
        // LogLevel is now a type alias for LoggingLevel
        let level: LogLevel = LoggingLevel::Warning;
        assert_eq!(serde_json::to_value(level).unwrap(), "warning");
    }

    #[test]
    fn test_cancelled_notification() {
        use crate::types::RequestId;

        let cancelled =
            CancelledNotification::new(RequestId::Number(123)).with_reason("User cancelled");

        let json = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(json["requestId"], 123);
        assert_eq!(json["reason"], "User cancelled");
    }

    #[test]
    fn test_task_status_notification() {
        use crate::types::tasks::{Task, TaskStatus as TStatus, TaskStatusNotification};

        let notif = ServerNotification::TaskStatus(TaskStatusNotification {
            task: Task::new("t-789", TStatus::Completed)
                .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:05:00Z")
                .with_status_message("Done"),
        });
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["method"], "notifications/tasks/status");
        assert_eq!(json["params"]["task"]["taskId"], "t-789");
        assert_eq!(json["params"]["task"]["status"], "completed");
    }
}
