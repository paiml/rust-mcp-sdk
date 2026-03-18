//! Advanced logging with correlation IDs and structured logging.
//!
//! This module provides enhanced logging capabilities including correlation IDs,
//! structured logging, and integration with request context.

use crate::shared::context::RequestContext;
use crate::types::RequestId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{field, span, Level, Span};
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
use tracing_subscriber::layer::SubscriberExt;
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
use tracing_subscriber::util::SubscriberInitExt;
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
use tracing_subscriber::{EnvFilter, Layer};

/// Logging configuration.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::logging::{LogConfig, LogLevel, LogFormat};
/// use std::collections::HashMap;
///
/// let config = LogConfig {
///     level: LogLevel::Info,
///     timestamps: true,
///     source_location: false,
///     correlation_ids: true,
///     custom_fields: HashMap::new(),
///     format: LogFormat::Json,
/// };
///
/// // With custom fields
/// let mut custom_fields = HashMap::new();
/// custom_fields.insert("service".to_string(), serde_json::json!("my-service"));
/// custom_fields.insert("version".to_string(), serde_json::json!("1.0.0"));
///
/// let config_with_fields = LogConfig {
///     level: LogLevel::Debug,
///     timestamps: true,
///     source_location: true,
///     correlation_ids: true,
///     custom_fields,
///     format: LogFormat::Pretty,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Log level.
    pub level: LogLevel,

    /// Whether to include timestamps.
    pub timestamps: bool,

    /// Whether to include file/line info.
    pub source_location: bool,

    /// Whether to include correlation IDs.
    pub correlation_ids: bool,

    /// Custom fields to include in all logs.
    pub custom_fields: HashMap<String, serde_json::Value>,

    /// Log format.
    pub format: LogFormat,
}

/// Log level.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::logging::LogLevel;
///
/// let levels = vec![
///     LogLevel::Trace,
///     LogLevel::Debug,
///     LogLevel::Info,
///     LogLevel::Warn,
///     LogLevel::Error,
/// ];
///
/// // Use the levels
/// for level in levels {
///     println!("Log level: {:?}", level);
/// }
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Trace level logging
    Trace,
    /// Debug level logging
    Debug,
    /// Info level logging
    Info,
    /// Warning level logging
    Warn,
    /// Error level logging
    Error,
}

/// Log format.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::logging::LogFormat;
///
/// let formats = vec![
///     LogFormat::Json,
///     LogFormat::Pretty,
///     LogFormat::Compact,
/// ];
///
/// for format in formats {
///     println!("Format: {:?}", format);
/// }
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// JSON formatted logs
    Json,
    /// Pretty formatted logs
    Pretty,
    /// Compact formatted logs
    Compact,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            timestamps: true,
            source_location: true,
            correlation_ids: true,
            custom_fields: HashMap::new(),
            format: LogFormat::Pretty,
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
/// Initialize logging with configuration.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::logging::{init_logging, LogConfig, LogLevel, LogFormat};
/// use std::collections::HashMap;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = LogConfig {
///     level: LogLevel::Info,
///     timestamps: true,
///     source_location: true,
///     correlation_ids: true,
///     custom_fields: HashMap::new(),
///     format: LogFormat::Json,
/// };
///
/// init_logging(config)?;
/// # Ok(())
/// # }
/// ```
pub fn init_logging(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = match config.level {
        LogLevel::Trace => EnvFilter::new("trace"),
        LogLevel::Debug => EnvFilter::new("debug"),
        LogLevel::Info => EnvFilter::new("info"),
        LogLevel::Warn => EnvFilter::new("warn"),
        LogLevel::Error => EnvFilter::new("error"),
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(config.source_location)
        .with_thread_ids(true)
        .with_thread_names(true);

    let fmt_layer = match config.format {
        LogFormat::Json => fmt_layer.with_ansi(false).boxed(),
        LogFormat::Pretty => fmt_layer.pretty().boxed(),
        LogFormat::Compact => fmt_layer.compact().boxed(),
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(CorrelationLayer::new(config))
        .init();

    Ok(())
}

/// Correlation layer for adding context to spans.
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
pub struct CorrelationLayer {
    config: LogConfig,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
impl std::fmt::Debug for CorrelationLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorrelationLayer")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
impl CorrelationLayer {
    /// Create a new correlation layer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::logging::{CorrelationLayer, LogConfig};
    ///
    /// let config = LogConfig::default();
    /// let layer = CorrelationLayer::new(config);
    /// ```
    pub fn new(config: LogConfig) -> Self {
        Self { config }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
impl<S> Layer<S> for CorrelationLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        _attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if self.config.correlation_ids {
            if let Some(context) = RequestContext::current() {
                if let Some(span) = ctx.span(id) {
                    let mut extensions = span.extensions_mut();
                    extensions.insert(context);
                }
            }
        }
    }
}

/// Logger with correlation ID support.
pub struct CorrelatedLogger {
    span: Span,
}

impl std::fmt::Debug for CorrelatedLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorrelatedLogger")
            .field("span", &"Span")
            .finish()
    }
}

impl CorrelatedLogger {
    /// Create a new correlated logger.
    pub fn new(operation: &str) -> Self {
        let span = if let Some(context) = RequestContext::current() {
            span!(
                Level::INFO,
                "operation",
                name = operation,
                request_id = %context.request_id,
                trace_id = %context.trace_id,
                span_id = %context.span_id,
                user_id = field::Empty,
                session_id = field::Empty,
            )
        } else {
            span!(
                Level::INFO,
                "operation",
                name = operation,
                request_id = field::Empty,
                trace_id = field::Empty,
                span_id = field::Empty,
                user_id = field::Empty,
                session_id = field::Empty,
            )
        };

        // Set user and session IDs if available
        if let Some(context) = RequestContext::current() {
            if let Some(user_id) = &context.user_id {
                span.record("user_id", field::display(user_id));
            }
            if let Some(session_id) = &context.session_id {
                span.record("session_id", field::display(session_id));
            }
        }

        Self { span }
    }

    /// Create from existing request context.
    pub fn from_context(operation: &str, context: &RequestContext) -> Self {
        let span = span!(
            Level::INFO,
            "operation",
            name = operation,
            request_id = %context.request_id,
            trace_id = %context.trace_id,
            span_id = %context.span_id,
            user_id = field::Empty,
            session_id = field::Empty,
        );

        if let Some(user_id) = &context.user_id {
            span.record("user_id", field::display(user_id));
        }
        if let Some(session_id) = &context.session_id {
            span.record("session_id", field::display(session_id));
        }

        Self { span }
    }

    /// Enter the span context.
    pub fn enter(&self) -> span::Entered<'_> {
        self.span.enter()
    }

    /// Log with correlation context.
    pub fn in_scope<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.span.in_scope(f)
    }
}

/// Structured log entry.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Log level.
    pub level: String,

    /// Message.
    pub message: String,

    /// Request ID.
    pub request_id: Option<RequestId>,

    /// Trace ID.
    pub trace_id: Option<String>,

    /// Span ID.
    pub span_id: Option<String>,

    /// User ID.
    pub user_id: Option<String>,

    /// Session ID.
    pub session_id: Option<String>,

    /// Additional fields.
    pub fields: HashMap<String, serde_json::Value>,

    /// Error details if applicable.
    pub error: Option<ErrorDetails>,
}

/// Error details for structured logging.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetails {
    /// Error type.
    pub error_type: String,

    /// Error message.
    pub message: String,

    /// Stack trace if available.
    pub stack_trace: Option<Vec<String>>,

    /// Error code.
    pub code: Option<String>,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: impl Into<String>, message: impl Into<String>) -> Self {
        let mut entry = Self {
            timestamp: chrono::Utc::now(),
            level: level.into(),
            message: message.into(),
            request_id: None,
            trace_id: None,
            span_id: None,
            user_id: None,
            session_id: None,
            fields: HashMap::new(),
            error: None,
        };

        // Populate from current context
        if let Some(context) = RequestContext::current() {
            entry.request_id = Some(context.request_id.clone());
            entry.trace_id = Some(context.trace_id.clone());
            entry.span_id = Some(context.span_id.clone());
            entry.user_id.clone_from(&context.user_id);
            entry.session_id.clone_from(&context.session_id);
        }

        entry
    }

    /// Add a field to the log entry.
    pub fn with_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Add error details.
    pub fn with_error(mut self, error: ErrorDetails) -> Self {
        self.error = Some(error);
        self
    }

    /// Log the entry.
    #[allow(clippy::cognitive_complexity)]
    pub fn log(self) {
        let json = serde_json::to_string(&self).unwrap_or_else(|_| self.message.clone());

        match self.level.to_lowercase().as_str() {
            "trace" => tracing::trace!("{}", json),
            "debug" => tracing::debug!("{}", json),
            "info" => tracing::info!("{}", json),
            "warn" => tracing::warn!("{}", json),
            "error" => tracing::error!("{}", json),
            _ => tracing::info!("{}", json),
        }
    }
}

/// Helper macros for correlated logging.
#[macro_export]
macro_rules! log_correlated {
    ($level:expr, $msg:expr) => {
        $crate::shared::logging::LogEntry::new($level, $msg).log()
    };
    ($level:expr, $msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        {
            let mut entry = $crate::shared::logging::LogEntry::new($level, $msg);
            $(
                entry = entry.with_field($key, serde_json::json!($value));
            )*
            entry.log()
        }
    };
}

/// Log info with correlation.
#[macro_export]
macro_rules! info_correlated {
    ($msg:expr) => {
        $crate::log_correlated!("info", $msg)
    };
    ($msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        $crate::log_correlated!("info", $msg, $($key => $value),*)
    };
}

/// Log error with correlation.
#[macro_export]
macro_rules! error_correlated {
    ($msg:expr) => {
        $crate::log_correlated!("error", $msg)
    };
    ($msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        $crate::log_correlated!("error", $msg, $($key => $value),*)
    };
}

/// Log warning with correlation.
#[macro_export]
macro_rules! warn_correlated {
    ($msg:expr) => {
        $crate::log_correlated!("warn", $msg)
    };
    ($msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        $crate::log_correlated!("warn", $msg, $($key => $value),*)
    };
}

/// Log debug with correlation.
#[macro_export]
macro_rules! debug_correlated {
    ($msg:expr) => {
        $crate::log_correlated!("debug", $msg)
    };
    ($msg:expr, $($key:expr => $value:expr),* $(,)?) => {
        $crate::log_correlated!("debug", $msg, $($key => $value),*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new("info", "Test message")
            .with_field("key1", serde_json::json!("value1"))
            .with_field("count", serde_json::json!(42));

        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.fields.get("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(entry.fields.get("count"), Some(&serde_json::json!(42)));
    }

    #[tokio::test]
    async fn test_correlated_logger() {
        let context =
            RequestContext::new(RequestId::from(123i64)).with_user_id("user123".to_string());

        context
            .run(async {
                let logger = CorrelatedLogger::new("test_operation");
                logger.in_scope(|| {
                    tracing::info!("Test log message");
                });
            })
            .await;
    }

    #[test]
    fn test_error_details() {
        let error = ErrorDetails {
            error_type: "ValidationError".to_string(),
            message: "Invalid input".to_string(),
            stack_trace: Some(vec!["line1".to_string(), "line2".to_string()]),
            code: Some("E001".to_string()),
        };

        let entry = LogEntry::new("error", "Validation failed").with_error(error);

        assert!(entry.error.is_some());
        let err = entry.error.unwrap();
        assert_eq!(err.error_type, "ValidationError");
        assert_eq!(err.code, Some("E001".to_string()));
    }
}
