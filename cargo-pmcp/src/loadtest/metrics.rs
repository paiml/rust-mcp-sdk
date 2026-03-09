//! HdrHistogram-based metrics pipeline with coordinated omission correction.
//!
//! Provides [`MetricsRecorder`] for recording MCP request latency samples into
//! separate success/error HdrHistogram buckets, with per-operation-type tracking.
//! Coordinated omission correction is applied at recording time via
//! [`hdrhistogram::Histogram::record_correct`].
//!
//! # Design
//!
//! - **Single-owner**: No `Arc<Mutex>` -- designed for Phase 2's mpsc channel
//!   aggregation pattern where one thread owns the recorder.
//! - **Separate buckets**: Success and error latencies are tracked in independent
//!   histograms so error spikes don't pollute success percentiles.
//! - **Coordinated omission correction**: Applied at recording time via
//!   `record_correct()`, not post-hoc. This fills in synthetic samples for
//!   intervals missed when the system was stalled.
//! - **Millisecond resolution**: Matches how users think about latency.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use hdrhistogram::Histogram;

use crate::loadtest::error::McpError;

/// Type of MCP operation being measured.
///
/// Each variant maps to an MCP protocol method. The [`fmt::Display`] impl
/// produces the wire-format string (e.g., `"tools/call"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// MCP initialize handshake.
    Initialize,
    /// tools/call request.
    ToolsCall,
    /// resources/read request.
    ResourcesRead,
    /// prompts/get request.
    PromptsGet,
    /// tools/list discovery request.
    ToolsList,
    /// resources/list discovery request.
    ResourcesList,
    /// prompts/list discovery request.
    PromptsList,
    /// code_mode two-step flow (validate_code + execute_code).
    CodeMode,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Initialize => "initialize",
            Self::ToolsCall => "tools/call",
            Self::ResourcesRead => "resources/read",
            Self::PromptsGet => "prompts/get",
            Self::ToolsList => "tools/list",
            Self::ResourcesList => "resources/list",
            Self::PromptsList => "prompts/list",
            Self::CodeMode => "code_mode",
        };
        f.write_str(s)
    }
}

/// A single request measurement sample.
///
/// Created via [`RequestSample::success`] or [`RequestSample::error`] convenience
/// constructors. Passed to [`MetricsRecorder::record`] for ingestion.
pub struct RequestSample {
    /// The MCP operation type that was measured.
    pub operation: OperationType,
    /// Wall-clock duration of the request.
    pub duration: Duration,
    /// `Ok(())` for success, `Err(McpError)` for failure.
    pub result: Result<(), McpError>,
    /// When the sample was taken.
    pub timestamp: Instant,
    /// Optional tool/resource/prompt name for per-tool metrics tracking.
    pub tool_name: Option<String>,
}

impl RequestSample {
    /// Create a success sample with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `operation` - The MCP operation type that was measured.
    /// * `duration` - Wall-clock duration of the request.
    /// * `tool_name` - Optional tool, resource URI, or prompt name.
    pub fn success(
        operation: OperationType,
        duration: Duration,
        tool_name: Option<String>,
    ) -> Self {
        Self {
            operation,
            duration,
            result: Ok(()),
            timestamp: Instant::now(),
            tool_name,
        }
    }

    /// Create an error sample with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `operation` - The MCP operation type that was measured.
    /// * `duration` - Wall-clock duration of the request.
    /// * `err` - The MCP error that occurred.
    /// * `tool_name` - Optional tool, resource URI, or prompt name.
    pub fn error(
        operation: OperationType,
        duration: Duration,
        err: McpError,
        tool_name: Option<String>,
    ) -> Self {
        Self {
            operation,
            duration,
            result: Err(err),
            timestamp: Instant::now(),
            tool_name,
        }
    }
}

/// Per-tool metrics snapshot with latency percentiles and error breakdown.
///
/// Produced by [`MetricsRecorder::snapshot`] for each distinct tool name
/// encountered during recording. Sorted alphabetically by name for
/// deterministic terminal output.
#[derive(Debug, Clone)]
pub struct ToolSnapshot {
    /// Tool name, resource URI, or prompt name.
    pub name: String,
    /// Success latency P50 (milliseconds).
    pub p50: u64,
    /// Success latency P95 (milliseconds).
    pub p95: u64,
    /// Success latency P99 (milliseconds).
    pub p99: u64,
    /// Minimum latency across all requests (milliseconds).
    pub min: u64,
    /// Maximum latency across all requests (milliseconds).
    pub max: u64,
    /// Mean latency across all requests (milliseconds).
    pub mean: f64,
    /// Total requests for this tool (success + error).
    pub total_requests: u64,
    /// Total successful requests for this tool.
    pub success_count: u64,
    /// Total failed requests for this tool.
    pub error_count: u64,
    /// Error rate as a fraction (0.0..=1.0).
    pub error_rate: f64,
    /// Error counts by classification for this tool.
    pub error_categories: HashMap<String, u64>,
}

/// Point-in-time snapshot of all metrics state.
///
/// Captured via [`MetricsRecorder::snapshot`]. All percentile values are in
/// milliseconds.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Success latency P50 (milliseconds).
    pub p50: u64,
    /// Success latency P95 (milliseconds).
    pub p95: u64,
    /// Success latency P99 (milliseconds).
    pub p99: u64,
    /// Error latency P50 (milliseconds).
    pub error_p50: u64,
    /// Error latency P95 (milliseconds).
    pub error_p95: u64,
    /// Error latency P99 (milliseconds).
    pub error_p99: u64,
    /// Total successful requests recorded.
    pub success_count: u64,
    /// Total failed requests recorded.
    pub error_count: u64,
    /// Total requests (success + error).
    pub total_requests: u64,
    /// Fraction of requests that were errors (0.0..=1.0).
    pub error_rate: f64,
    /// Per-operation total request counts.
    pub operation_counts: HashMap<OperationType, u64>,
    /// Per-operation error counts.
    pub per_operation_errors: HashMap<OperationType, u64>,
    /// Error counts by classification (jsonrpc, http, timeout, connection).
    pub error_category_counts: HashMap<String, u64>,
    /// Per-tool metrics, sorted alphabetically by tool name.
    pub per_tool: Vec<ToolSnapshot>,
}

/// Per-tool HdrHistogram pair tracking success and error latencies independently.
///
/// Created on-demand inside [`MetricsRecorder`] when a sample with a non-`None`
/// `tool_name` is recorded. Uses the same histogram configuration (3 significant
/// figures, auto-resize) as the main recorder.
struct ToolMetrics {
    /// Histogram for successful request latencies for this tool.
    success_histogram: Histogram<u64>,
    /// Histogram for failed request latencies for this tool.
    error_histogram: Histogram<u64>,
    /// Running total of successful requests for this tool.
    total_success: u64,
    /// Running total of failed requests for this tool.
    total_errors: u64,
    /// Per-error-category counts for this tool.
    error_category_counts: HashMap<String, u64>,
}

impl ToolMetrics {
    /// Create a new per-tool metrics tracker with auto-resizing histograms.
    fn new() -> Self {
        let mut success_histogram = Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        success_histogram.auto(true);

        let mut error_histogram = Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        error_histogram.auto(true);

        Self {
            success_histogram,
            error_histogram,
            total_success: 0,
            total_errors: 0,
            error_category_counts: HashMap::new(),
        }
    }
}

/// HdrHistogram-backed metrics recorder with coordinated omission correction.
///
/// Records MCP request latency samples into separate success/error histograms.
/// Designed for single-owner usage -- no internal locking. Phase 2 will use
/// mpsc channels to feed samples from worker tasks to a single recorder.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use cargo_pmcp::loadtest::metrics::{MetricsRecorder, RequestSample, OperationType};
///
/// let mut recorder = MetricsRecorder::new(100);
/// let sample = RequestSample::success(OperationType::ToolsCall, Duration::from_millis(42), None);
/// recorder.record(&sample);
///
/// assert_eq!(recorder.success_count(), 1);
/// assert_eq!(recorder.p50(), 42);
/// ```
pub struct MetricsRecorder {
    /// Histogram for successful request latencies.
    success_histogram: Histogram<u64>,
    /// Histogram for failed request latencies.
    error_histogram: Histogram<u64>,
    /// Expected interval between requests in milliseconds.
    /// Used by `record_correct()` for coordinated omission correction.
    expected_interval_ms: u64,
    /// Per-operation total request counts (success + error).
    operation_counts: HashMap<OperationType, u64>,
    /// Per-operation error counts.
    error_counts: HashMap<OperationType, u64>,
    /// Running total of successful requests (logical, not histogram entries).
    total_success: u64,
    /// Running total of failed requests (logical, not histogram entries).
    total_errors: u64,
    /// Per-error-category counts (jsonrpc, http, timeout, connection).
    error_category_counts: HashMap<String, u64>,
    /// Per-tool metrics keyed by tool name.
    per_tool: HashMap<String, ToolMetrics>,
}

impl MetricsRecorder {
    /// Create a new recorder with the given expected request interval.
    ///
    /// The `expected_interval_ms` is used for coordinated omission correction:
    /// when a request takes much longer than expected, synthetic samples are
    /// filled in to account for requests that *would have* been sent during
    /// the stall.
    ///
    /// Histograms are created with 3 significant figures of precision and
    /// auto-resize enabled.
    pub fn new(expected_interval_ms: u64) -> Self {
        let mut success_histogram = Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        success_histogram.auto(true);

        let mut error_histogram = Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        error_histogram.auto(true);

        Self {
            success_histogram,
            error_histogram,
            expected_interval_ms,
            operation_counts: HashMap::new(),
            error_counts: HashMap::new(),
            total_success: 0,
            total_errors: 0,
            error_category_counts: HashMap::new(),
            per_tool: HashMap::new(),
        }
    }

    /// Record a request sample with coordinated omission correction.
    ///
    /// The sample is routed to the success or error histogram based on its
    /// `result` field. `record_correct()` is used instead of plain `record()`
    /// to apply coordinated omission correction.
    ///
    /// If the sample has a `tool_name`, it is also recorded into per-tool
    /// histograms for tool-level metrics breakdown.
    pub fn record(&mut self, sample: &RequestSample) {
        let ms = sample.duration.as_millis() as u64;

        // Increment per-operation total count
        *self.operation_counts.entry(sample.operation).or_insert(0) += 1;

        match &sample.result {
            Ok(()) => {
                let _ = self
                    .success_histogram
                    .record_correct(ms, self.expected_interval_ms);
                self.total_success += 1;
            },
            Err(ref err) => {
                let _ = self
                    .error_histogram
                    .record_correct(ms, self.expected_interval_ms);
                self.total_errors += 1;
                *self.error_counts.entry(sample.operation).or_insert(0) += 1;
                *self
                    .error_category_counts
                    .entry(err.error_category().to_owned())
                    .or_insert(0) += 1;
            },
        }

        // Record into per-tool histograms if tool_name is present
        if let Some(ref name) = sample.tool_name {
            let tool = self
                .per_tool
                .entry(name.clone())
                .or_insert_with(ToolMetrics::new);

            match &sample.result {
                Ok(()) => {
                    let _ = tool
                        .success_histogram
                        .record_correct(ms, self.expected_interval_ms);
                    tool.total_success += 1;
                },
                Err(ref err) => {
                    let _ = tool
                        .error_histogram
                        .record_correct(ms, self.expected_interval_ms);
                    tool.total_errors += 1;
                    *tool
                        .error_category_counts
                        .entry(err.error_category().to_owned())
                        .or_insert(0) += 1;
                },
            }
        }
    }

    /// Success latency P50 in milliseconds. Returns 0 if no samples recorded.
    pub fn p50(&self) -> u64 {
        if self.success_histogram.is_empty() {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.50)
    }

    /// Success latency P95 in milliseconds. Returns 0 if no samples recorded.
    pub fn p95(&self) -> u64 {
        if self.success_histogram.is_empty() {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.95)
    }

    /// Success latency P99 in milliseconds. Returns 0 if no samples recorded.
    pub fn p99(&self) -> u64 {
        if self.success_histogram.is_empty() {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.99)
    }

    /// Error latency P50 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p50(&self) -> u64 {
        if self.error_histogram.is_empty() {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.50)
    }

    /// Error latency P95 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p95(&self) -> u64 {
        if self.error_histogram.is_empty() {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.95)
    }

    /// Error latency P99 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p99(&self) -> u64 {
        if self.error_histogram.is_empty() {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.99)
    }

    /// Total number of successful requests recorded.
    ///
    /// Note: this returns histogram entry count which includes synthetic
    /// fill-ins from coordinated omission correction. Use this for accurate
    /// percentile denominators.
    pub fn success_count(&self) -> u64 {
        self.success_histogram.len()
    }

    /// Total number of failed requests recorded.
    ///
    /// Note: this returns histogram entry count which includes synthetic
    /// fill-ins from coordinated omission correction.
    pub fn error_count(&self) -> u64 {
        self.error_histogram.len()
    }

    /// Total requests recorded (success + error histogram entries).
    pub fn total_requests(&self) -> u64 {
        self.success_histogram.len() + self.error_histogram.len()
    }

    /// Error rate as a fraction (0.0..=1.0). Returns 0.0 if no requests recorded.
    pub fn error_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.error_histogram.len() as f64 / total as f64
    }

    /// Total requests for a specific operation type (success + error).
    ///
    /// This is the *logical* count (one per `record()` call), not inflated
    /// by coordinated omission correction.
    pub fn operation_count(&self, op: OperationType) -> u64 {
        self.operation_counts.get(&op).copied().unwrap_or(0)
    }

    /// Capture a point-in-time snapshot of all metrics.
    ///
    /// The snapshot is a self-contained value that can be sent across threads
    /// or serialized without holding a reference to the recorder.
    ///
    /// Per-tool snapshots are sorted alphabetically by tool name for
    /// deterministic terminal and JSON output.
    pub fn snapshot(&self) -> MetricsSnapshot {
        // Build per-tool snapshots sorted by name
        let mut per_tool: Vec<ToolSnapshot> = self
            .per_tool
            .iter()
            .map(|(name, tool)| {
                let success_count = tool.success_histogram.len();
                let error_count = tool.error_histogram.len();
                let total = success_count + error_count;
                let error_rate = if total == 0 {
                    0.0
                } else {
                    error_count as f64 / total as f64
                };

                // Combine min/max/mean across both histograms
                let (min, max, mean) = if success_count > 0 && error_count > 0 {
                    let min = tool.success_histogram.min().min(tool.error_histogram.min());
                    let max = tool.success_histogram.max().max(tool.error_histogram.max());
                    let mean = (tool.success_histogram.mean() * success_count as f64
                        + tool.error_histogram.mean() * error_count as f64)
                        / total as f64;
                    (min, max, mean)
                } else if success_count > 0 {
                    (
                        tool.success_histogram.min(),
                        tool.success_histogram.max(),
                        tool.success_histogram.mean(),
                    )
                } else if error_count > 0 {
                    (
                        tool.error_histogram.min(),
                        tool.error_histogram.max(),
                        tool.error_histogram.mean(),
                    )
                } else {
                    (0, 0, 0.0)
                };

                // P50/P95/P99 from success histogram (primary latency view)
                let p50 = if tool.success_histogram.is_empty() {
                    0
                } else {
                    tool.success_histogram.value_at_quantile(0.50)
                };
                let p95 = if tool.success_histogram.is_empty() {
                    0
                } else {
                    tool.success_histogram.value_at_quantile(0.95)
                };
                let p99 = if tool.success_histogram.is_empty() {
                    0
                } else {
                    tool.success_histogram.value_at_quantile(0.99)
                };

                ToolSnapshot {
                    name: name.clone(),
                    p50,
                    p95,
                    p99,
                    min,
                    max,
                    mean,
                    total_requests: total,
                    success_count,
                    error_count,
                    error_rate,
                    error_categories: tool.error_category_counts.clone(),
                }
            })
            .collect();
        per_tool.sort_by(|a, b| a.name.cmp(&b.name));

        MetricsSnapshot {
            p50: self.p50(),
            p95: self.p95(),
            p99: self.p99(),
            error_p50: self.error_p50(),
            error_p95: self.error_p95(),
            error_p99: self.error_p99(),
            success_count: self.success_histogram.len(),
            error_count: self.error_histogram.len(),
            total_requests: self.total_requests(),
            error_rate: self.error_rate(),
            operation_counts: self.operation_counts.clone(),
            per_operation_errors: self.error_counts.clone(),
            error_category_counts: self.error_category_counts.clone(),
            per_tool,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::error::McpError;

    #[test]
    fn test_new_recorder_has_zero_counts() {
        let recorder = MetricsRecorder::new(100);
        assert_eq!(recorder.total_requests(), 0);
        assert_eq!(recorder.success_count(), 0);
        assert_eq!(recorder.error_count(), 0);
    }

    #[test]
    fn test_record_success_increments_count() {
        let mut recorder = MetricsRecorder::new(100);
        for _ in 0..5 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(50), None);
            recorder.record(&sample);
        }
        assert_eq!(recorder.success_count(), 5);
        assert_eq!(recorder.error_count(), 0);
        assert_eq!(recorder.total_requests(), 5);
    }

    #[test]
    fn test_record_error_increments_count() {
        let mut recorder = MetricsRecorder::new(100);
        for _ in 0..3 {
            let sample = RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                McpError::Timeout,
                None,
            );
            recorder.record(&sample);
        }
        assert_eq!(recorder.error_count(), 3);
        assert_eq!(recorder.success_count(), 0);
    }

    #[test]
    fn test_percentiles_single_value() {
        let mut recorder = MetricsRecorder::new(10_000);
        let sample =
            RequestSample::success(OperationType::ToolsCall, Duration::from_millis(50), None);
        recorder.record(&sample);
        assert_eq!(recorder.p50(), 50);
        assert_eq!(recorder.p95(), 50);
        assert_eq!(recorder.p99(), 50);
    }

    #[test]
    fn test_percentiles_known_distribution() {
        let mut recorder = MetricsRecorder::new(10_000);
        for i in 1..=100 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(i), None);
            recorder.record(&sample);
        }
        let p50 = recorder.p50() as i64;
        let p95 = recorder.p95() as i64;
        let p99 = recorder.p99() as i64;
        assert!((p50 - 50).abs() <= 1, "p50 was {p50}, expected ~50");
        assert!((p95 - 95).abs() <= 1, "p95 was {p95}, expected ~95");
        assert!((p99 - 99).abs() <= 1, "p99 was {p99}, expected ~99");
    }

    #[test]
    fn test_coordinated_omission_correction() {
        let mut recorder = MetricsRecorder::new(10);
        let sample =
            RequestSample::success(OperationType::ToolsCall, Duration::from_millis(100), None);
        recorder.record(&sample);
        // With correction, HdrHistogram fills in synthetic samples at
        // 90ms, 80ms, 70ms, ..., 10ms. So total count > 1.
        assert!(
            recorder.success_count() > 1,
            "Expected synthetic fills, got count={}",
            recorder.success_count()
        );
        // The corrected median should be well below 100ms.
        assert!(
            recorder.p50() < 100,
            "p50 was {}, expected < 100 with correction",
            recorder.p50()
        );
    }

    #[test]
    fn test_success_and_error_separate_buckets() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..10 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(10), None);
            recorder.record(&sample);
        }
        for _ in 0..10 {
            let sample = RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(500),
                McpError::Timeout,
                None,
            );
            recorder.record(&sample);
        }
        assert_eq!(recorder.p99(), 10, "success p99 should be ~10ms");
        assert_eq!(recorder.error_p99(), 500, "error p99 should be ~500ms");
    }

    #[test]
    fn test_per_operation_counts() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..3 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
                None,
            ));
        }
        for _ in 0..2 {
            recorder.record(&RequestSample::success(
                OperationType::ResourcesRead,
                Duration::from_millis(10),
                None,
            ));
        }
        recorder.record(&RequestSample::error(
            OperationType::PromptsGet,
            Duration::from_millis(10),
            McpError::Timeout,
            None,
        ));
        assert_eq!(recorder.operation_count(OperationType::ToolsCall), 3);
        assert_eq!(recorder.operation_count(OperationType::ResourcesRead), 2);
        assert_eq!(recorder.operation_count(OperationType::PromptsGet), 1);
    }

    #[test]
    fn test_snapshot_captures_current_state() {
        let mut recorder = MetricsRecorder::new(10_000);
        for i in 1..=100 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(i),
                None,
            ));
        }
        recorder.record(&RequestSample::error(
            OperationType::PromptsGet,
            Duration::from_millis(200),
            McpError::Timeout,
            None,
        ));
        let snap = recorder.snapshot();
        assert_eq!(snap.success_count, 100);
        assert_eq!(snap.error_count, 1);
        assert_eq!(snap.total_requests, 101);
        assert!((snap.p50 as i64 - 50).abs() <= 1);
        assert_eq!(snap.error_p99, 200);
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(OperationType::ToolsCall.to_string(), "tools/call");
        assert_eq!(OperationType::ResourcesRead.to_string(), "resources/read");
        assert_eq!(OperationType::PromptsGet.to_string(), "prompts/get");
        assert_eq!(OperationType::Initialize.to_string(), "initialize");
        assert_eq!(OperationType::ToolsList.to_string(), "tools/list");
        assert_eq!(OperationType::ResourcesList.to_string(), "resources/list");
        assert_eq!(OperationType::PromptsList.to_string(), "prompts/list");
    }

    #[test]
    fn test_error_rate_calculation() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..7 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
                None,
            ));
        }
        for _ in 0..3 {
            recorder.record(&RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(10),
                McpError::Timeout,
                None,
            ));
        }
        let rate = recorder.error_rate();
        assert!(
            (rate - 0.3).abs() < 0.001,
            "error_rate was {rate}, expected 0.3"
        );
    }

    #[test]
    fn test_empty_percentiles_return_zero() {
        let recorder = MetricsRecorder::new(100);
        assert_eq!(recorder.p50(), 0);
        assert_eq!(recorder.p95(), 0);
        assert_eq!(recorder.p99(), 0);
    }

    #[test]
    fn test_error_category_counts_tracked() {
        let mut recorder = MetricsRecorder::new(10_000);
        recorder.record(&RequestSample::error(
            OperationType::ToolsCall,
            Duration::from_millis(10),
            McpError::Timeout,
            None,
        ));
        recorder.record(&RequestSample::error(
            OperationType::ToolsCall,
            Duration::from_millis(10),
            McpError::Timeout,
            None,
        ));
        recorder.record(&RequestSample::error(
            OperationType::ResourcesRead,
            Duration::from_millis(10),
            McpError::JsonRpc {
                code: -32601,
                message: "Not found".to_string(),
            },
            None,
        ));
        recorder.record(&RequestSample::error(
            OperationType::ToolsCall,
            Duration::from_millis(10),
            McpError::Http {
                status: 500,
                body: "Internal error".to_string(),
            },
            None,
        ));

        let snap = recorder.snapshot();
        assert_eq!(snap.error_category_counts.get("timeout"), Some(&2));
        assert_eq!(snap.error_category_counts.get("jsonrpc"), Some(&1));
        assert_eq!(snap.error_category_counts.get("http"), Some(&1));
        assert_eq!(snap.error_category_counts.get("connection"), None);
    }

    #[test]
    fn test_error_category_counts_empty_for_success_only() {
        let mut recorder = MetricsRecorder::new(10_000);
        recorder.record(&RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(10),
            None,
        ));
        let snap = recorder.snapshot();
        assert!(snap.error_category_counts.is_empty());
    }

    #[test]
    fn test_per_tool_metrics_recorded() {
        let mut recorder = MetricsRecorder::new(10_000);
        // Record samples for two different tools
        for _ in 0..5 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                Some("calculate".to_string()),
            ));
        }
        for _ in 0..3 {
            recorder.record(&RequestSample::success(
                OperationType::ResourcesRead,
                Duration::from_millis(20),
                Some("file:///data".to_string()),
            ));
        }
        let snap = recorder.snapshot();
        assert_eq!(snap.per_tool.len(), 2);

        let calc = snap
            .per_tool
            .iter()
            .find(|t| t.name == "calculate")
            .unwrap();
        assert_eq!(calc.success_count, 5);
        assert_eq!(calc.error_count, 0);
        assert_eq!(calc.total_requests, 5);
        assert_eq!(calc.p50, 50);

        let file = snap
            .per_tool
            .iter()
            .find(|t| t.name == "file:///data")
            .unwrap();
        assert_eq!(file.success_count, 3);
        assert_eq!(file.total_requests, 3);
        assert_eq!(file.p50, 20);
    }

    #[test]
    fn test_per_tool_metrics_empty_when_no_tool_name() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..5 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                None,
            ));
        }
        let snap = recorder.snapshot();
        assert!(
            snap.per_tool.is_empty(),
            "per_tool should be empty when no tool_name is set"
        );
    }

    #[test]
    fn test_per_tool_snapshot_sorted_by_name() {
        let mut recorder = MetricsRecorder::new(10_000);
        recorder.record(&RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(50),
            Some("beta".to_string()),
        ));
        recorder.record(&RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(50),
            Some("alpha".to_string()),
        ));
        recorder.record(&RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(50),
            Some("gamma".to_string()),
        ));
        let snap = recorder.snapshot();
        assert_eq!(snap.per_tool.len(), 3);
        assert_eq!(snap.per_tool[0].name, "alpha");
        assert_eq!(snap.per_tool[1].name, "beta");
        assert_eq!(snap.per_tool[2].name, "gamma");
    }

    #[test]
    fn test_per_tool_error_categories() {
        let mut recorder = MetricsRecorder::new(10_000);
        // Tool "calc" gets 2 successes and 1 timeout error
        for _ in 0..2 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                Some("calc".to_string()),
            ));
        }
        recorder.record(&RequestSample::error(
            OperationType::ToolsCall,
            Duration::from_millis(100),
            McpError::Timeout,
            Some("calc".to_string()),
        ));
        // Tool "search" gets 1 jsonrpc error
        recorder.record(&RequestSample::error(
            OperationType::ToolsCall,
            Duration::from_millis(200),
            McpError::JsonRpc {
                code: -32601,
                message: "Not found".to_string(),
            },
            Some("search".to_string()),
        ));

        let snap = recorder.snapshot();
        assert_eq!(snap.per_tool.len(), 2);

        let calc = snap.per_tool.iter().find(|t| t.name == "calc").unwrap();
        assert_eq!(calc.success_count, 2);
        assert_eq!(calc.error_count, 1);
        assert_eq!(calc.error_categories.get("timeout"), Some(&1));
        assert!(calc.error_rate > 0.0);

        let search = snap.per_tool.iter().find(|t| t.name == "search").unwrap();
        assert_eq!(search.success_count, 0);
        assert_eq!(search.error_count, 1);
        assert_eq!(search.error_categories.get("jsonrpc"), Some(&1));
        // search has no timeout errors
        assert_eq!(search.error_categories.get("timeout"), None);
    }
}
