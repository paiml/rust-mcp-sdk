//! JSON report serialization for load test results.
//!
//! Produces a schema-versioned JSON report file containing latency percentiles,
//! throughput, error classification, and the full resolved config for
//! reproducibility. Designed for CI/CD pipeline consumption.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::loadtest::config::LoadTestConfig;
use crate::loadtest::engine::LoadTestResult;

/// Schema version for the JSON report format.
///
/// Increment when making breaking changes to the report structure.
/// External tools key on this field to determine parser compatibility.
const SCHEMA_VERSION: &str = "1.1";

/// Top-level JSON report structure.
///
/// Contains all information needed to understand what test was run
/// and what the results were. Designed to be self-contained: anyone
/// reading just the JSON file should understand the test configuration
/// and outcomes.
#[derive(Debug, Serialize)]
pub struct LoadTestReport {
    /// Report format version for parser compatibility.
    pub schema_version: String,
    /// ISO-8601 timestamp when the report was generated.
    pub timestamp: String,
    /// Target MCP server URL that was tested.
    pub target_url: String,
    /// Actual test duration in seconds.
    pub duration_secs: f64,
    /// Full resolved configuration (with CLI overrides applied).
    pub config: ReportConfig,
    /// Aggregate performance metrics.
    pub metrics: ReportMetrics,
    /// Error counts by classification type.
    pub errors: HashMap<String, u64>,
    /// Per-tool metrics keyed by tool name.
    pub per_tool: HashMap<String, ToolReportMetrics>,
    /// Breaking point detection result.
    pub breaking_point: BreakingPointReport,
}

/// Breaking point detection result for the JSON report.
///
/// Included in every report. When no breaking point was detected,
/// `detected` is `false` and all other fields are `None`.
#[derive(Debug, Serialize)]
pub struct BreakingPointReport {
    /// Whether a breaking point was detected during the test.
    pub detected: bool,
    /// VU count at which the breaking point was detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vus: Option<u32>,
    /// Reason category (e.g., `"error_rate_spike"`, `"latency_degradation"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Human-readable detail string explaining the detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// ISO-8601 timestamp when the breaking point was detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// Resolved test configuration embedded in the report.
///
/// Captures VUs, duration, timeout, scenario steps -- everything needed
/// to reproduce the test.
#[derive(Debug, Serialize)]
pub struct ReportConfig {
    /// Number of virtual users configured.
    pub virtual_users: u32,
    /// Test duration in seconds (from config, not actual).
    pub duration_secs: u64,
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Expected interval for coordinated omission correction.
    pub expected_interval_ms: u64,
    /// Scenario steps as JSON values for stable serialization.
    pub scenario: Vec<serde_json::Value>,
}

/// Aggregate performance metrics in the report.
#[derive(Debug, Serialize)]
pub struct ReportMetrics {
    /// Total number of requests made.
    pub total_requests: u64,
    /// Number of successful requests.
    pub success_count: u64,
    /// Number of failed requests.
    pub error_count: u64,
    /// Error rate as a fraction (0.0..=1.0).
    pub error_rate: f64,
    /// Throughput in requests per second.
    pub throughput_rps: f64,
    /// Latency percentile breakdown.
    pub latency: LatencyMetrics,
    /// Per-operation request counts (keys are operation type strings like "tools/call").
    pub operation_counts: HashMap<String, u64>,
    /// Per-operation error counts.
    pub operation_errors: HashMap<String, u64>,
}

/// Latency percentile metrics in milliseconds.
#[derive(Debug, Serialize)]
pub struct LatencyMetrics {
    /// 50th percentile (median) latency in milliseconds.
    pub p50_ms: u64,
    /// 95th percentile latency in milliseconds.
    pub p95_ms: u64,
    /// 99th percentile latency in milliseconds.
    pub p99_ms: u64,
    /// Error latency 50th percentile in milliseconds.
    pub error_p50_ms: u64,
    /// Error latency 95th percentile in milliseconds.
    pub error_p95_ms: u64,
    /// Error latency 99th percentile in milliseconds.
    pub error_p99_ms: u64,
}

/// Per-tool metrics for JSON report output.
///
/// Provides extended latency and error detail for a single tool, resource,
/// or prompt. Keyed by tool name in the `per_tool` HashMap of [`LoadTestReport`].
#[derive(Debug, Serialize)]
pub struct ToolReportMetrics {
    /// Total requests for this tool.
    pub total_requests: u64,
    /// Number of successful requests for this tool.
    pub success_count: u64,
    /// Number of failed requests for this tool.
    pub error_count: u64,
    /// Error rate as a fraction (0.0..=1.0).
    pub error_rate: f64,
    /// Latency breakdown for this tool.
    pub latency: ToolLatencyMetrics,
    /// Error counts by classification for this tool.
    pub errors: HashMap<String, u64>,
}

/// Per-tool latency metrics for JSON report output.
#[derive(Debug, Serialize)]
pub struct ToolLatencyMetrics {
    /// 50th percentile latency in milliseconds.
    pub p50_ms: u64,
    /// 95th percentile latency in milliseconds.
    pub p95_ms: u64,
    /// 99th percentile latency in milliseconds.
    pub p99_ms: u64,
    /// Minimum latency in milliseconds.
    pub min_ms: u64,
    /// Maximum latency in milliseconds.
    pub max_ms: u64,
    /// Mean latency in milliseconds.
    pub mean_ms: f64,
}

impl LoadTestReport {
    /// Build a report from load test results, config, and target URL.
    ///
    /// Converts `OperationType` enum keys to strings for JSON compatibility
    /// and computes throughput from total requests and elapsed time.
    pub fn from_result(result: &LoadTestResult, config: &LoadTestConfig, url: &str) -> Self {
        let snap = &result.snapshot;
        let elapsed_secs = result.elapsed.as_secs_f64();
        let throughput_rps = if elapsed_secs > 0.0 {
            snap.total_requests as f64 / elapsed_secs
        } else {
            0.0
        };

        // Convert OperationType keys to String keys for JSON serialization
        let operation_counts: HashMap<String, u64> = snap
            .operation_counts
            .iter()
            .map(|(op, count)| (op.to_string(), *count))
            .collect();

        let operation_errors: HashMap<String, u64> = snap
            .per_operation_errors
            .iter()
            .map(|(op, count)| (op.to_string(), *count))
            .collect();

        // Convert scenario steps to JSON values for stable serialization
        let scenario_values: Vec<serde_json::Value> = config
            .scenario
            .iter()
            .map(|step| serde_json::to_value(step).unwrap_or(serde_json::Value::Null))
            .collect();

        let timestamp = chrono::Utc::now().to_rfc3339();

        // Convert per-tool snapshots to report format
        let per_tool: HashMap<String, ToolReportMetrics> = snap
            .per_tool
            .iter()
            .map(|tool| {
                (
                    tool.name.clone(),
                    ToolReportMetrics {
                        total_requests: tool.total_requests,
                        success_count: tool.success_count,
                        error_count: tool.error_count,
                        error_rate: tool.error_rate,
                        latency: ToolLatencyMetrics {
                            p50_ms: tool.p50,
                            p95_ms: tool.p95,
                            p99_ms: tool.p99,
                            min_ms: tool.min,
                            max_ms: tool.max,
                            mean_ms: tool.mean,
                        },
                        errors: tool.error_categories.clone(),
                    },
                )
            })
            .collect();

        // Convert breaking point to report format
        let breaking_point_report = match &result.breaking_point {
            Some(bp) => BreakingPointReport {
                detected: true,
                vus: Some(bp.vus),
                reason: Some(bp.reason.clone()),
                detail: Some(bp.detail.clone()),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
            },
            None => BreakingPointReport {
                detected: false,
                vus: None,
                reason: None,
                detail: None,
                timestamp: None,
            },
        };

        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            timestamp,
            target_url: url.to_string(),
            duration_secs: elapsed_secs,
            config: ReportConfig {
                virtual_users: config.settings.virtual_users,
                duration_secs: config.settings.duration_secs,
                timeout_ms: config.settings.timeout_ms,
                expected_interval_ms: config.settings.expected_interval_ms,
                scenario: scenario_values,
            },
            metrics: ReportMetrics {
                total_requests: snap.total_requests,
                success_count: snap.success_count,
                error_count: snap.error_count,
                error_rate: snap.error_rate,
                throughput_rps,
                latency: LatencyMetrics {
                    p50_ms: snap.p50,
                    p95_ms: snap.p95,
                    p99_ms: snap.p99,
                    error_p50_ms: snap.error_p50,
                    error_p95_ms: snap.error_p95,
                    error_p99_ms: snap.error_p99,
                },
                operation_counts,
                operation_errors,
            },
            errors: snap.error_category_counts.clone(),
            per_tool,
            breaking_point: breaking_point_report,
        }
    }
}

/// Write a JSON report file to the `.pmcp/reports/` directory.
///
/// Creates the reports directory if it does not exist. The filename
/// is timestamped: `loadtest-YYYY-MM-DDTHH-MM-SS.json` (hyphens, not
/// colons, for Windows compatibility).
///
/// Returns the path to the written report file.
pub fn write_report(report: &LoadTestReport, base_dir: &Path) -> Result<PathBuf, std::io::Error> {
    let reports_dir = base_dir.join(".pmcp").join("reports");

    // Auto-create reports directory
    if !reports_dir.exists() {
        std::fs::create_dir_all(&reports_dir)?;
    }

    // Generate filename with timestamp (hyphens, not colons)
    let filename = format!(
        "loadtest-{}.json",
        chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S")
    );
    let report_path = reports_dir.join(&filename);

    // Serialize and write
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    std::fs::write(&report_path, json)?;

    Ok(report_path)
}

/// Generate the report filename for a given timestamp.
///
/// Exposed for testing. Uses hyphens instead of colons for
/// cross-platform filename compatibility.
pub fn report_filename(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    format!("loadtest-{}.json", timestamp.format("%Y-%m-%dT%H-%M-%S"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};
    use crate::loadtest::engine::LoadTestResult;
    use crate::loadtest::metrics::{MetricsSnapshot, OperationType, ToolSnapshot};
    use std::collections::HashMap;
    use std::time::Duration;

    fn test_config() -> LoadTestConfig {
        LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        }
    }

    fn test_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            p50: 42,
            p95: 120,
            p99: 350,
            error_p50: 100,
            error_p95: 200,
            error_p99: 500,
            success_count: 950,
            error_count: 50,
            total_requests: 1000,
            error_rate: 0.05,
            operation_counts: HashMap::from([(OperationType::ToolsCall, 1000)]),
            per_operation_errors: HashMap::from([(OperationType::ToolsCall, 50)]),
            error_category_counts: HashMap::from([
                ("timeout".to_string(), 30),
                ("jsonrpc".to_string(), 15),
                ("http".to_string(), 5),
            ]),
            per_tool: Vec::new(),
        }
    }

    fn test_result() -> LoadTestResult {
        LoadTestResult {
            snapshot: test_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        }
    }

    #[test]
    fn test_report_schema_version() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert_eq!(report.schema_version, "1.1");
    }

    #[test]
    fn test_report_target_url() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert_eq!(report.target_url, "http://localhost:3000/mcp");
    }

    #[test]
    fn test_report_embeds_config() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert_eq!(report.config.virtual_users, 10);
        assert_eq!(report.config.duration_secs, 60);
        assert_eq!(report.config.timeout_ms, 5000);
        assert_eq!(report.config.scenario.len(), 1);
    }

    #[test]
    fn test_report_metrics() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert_eq!(report.metrics.total_requests, 1000);
        assert_eq!(report.metrics.success_count, 950);
        assert_eq!(report.metrics.error_count, 50);
        assert!((report.metrics.error_rate - 0.05).abs() < 0.001);
        assert!((report.metrics.throughput_rps - 16.666).abs() < 0.1);
        assert_eq!(report.metrics.latency.p50_ms, 42);
        assert_eq!(report.metrics.latency.p95_ms, 120);
        assert_eq!(report.metrics.latency.p99_ms, 350);
    }

    #[test]
    fn test_report_operation_counts_use_string_keys() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        // OperationType::ToolsCall.to_string() = "tools/call"
        assert_eq!(
            report.metrics.operation_counts.get("tools/call"),
            Some(&1000)
        );
    }

    #[test]
    fn test_report_error_categories() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert_eq!(report.errors.get("timeout"), Some(&30));
        assert_eq!(report.errors.get("jsonrpc"), Some(&15));
        assert_eq!(report.errors.get("http"), Some(&5));
    }

    #[test]
    fn test_report_serializes_to_valid_json() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        let json = serde_json::to_string_pretty(&report).expect("should serialize");

        // Verify it parses back to a Value
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse back");

        assert_eq!(parsed["schema_version"], "1.1");
        assert_eq!(parsed["target_url"], "http://localhost:3000/mcp");
        assert!(parsed["timestamp"].is_string());
        assert!(parsed["metrics"]["latency"]["p50_ms"].is_u64());
    }

    #[test]
    fn test_report_filename_format() {
        let ts = chrono::Utc::now();
        let filename = report_filename(&ts);
        assert!(filename.starts_with("loadtest-"));
        assert!(filename.ends_with(".json"));
        // Verify no colons (Windows incompatible)
        assert!(
            !filename.contains(':'),
            "Filename must not contain colons: {}",
            filename
        );
    }

    #[test]
    fn test_write_report_creates_file() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );

        let tmp_dir = tempfile::tempdir().expect("should create temp dir");
        let result = write_report(&report, tmp_dir.path());
        assert!(result.is_ok(), "write_report failed: {:?}", result.err());

        let path = result.unwrap();
        assert!(path.exists(), "Report file should exist at {:?}", path);

        // Verify the content is valid JSON
        let content = std::fs::read_to_string(&path).expect("should read file");
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("should be valid JSON");
        assert_eq!(parsed["schema_version"], "1.1");
    }

    #[test]
    fn test_write_report_creates_reports_directory() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );

        let tmp_dir = tempfile::tempdir().expect("should create temp dir");
        // The .pmcp/reports/ directory does not exist yet
        let reports_dir = tmp_dir.path().join(".pmcp").join("reports");
        assert!(!reports_dir.exists());

        let result = write_report(&report, tmp_dir.path());
        assert!(result.is_ok());
        assert!(
            reports_dir.exists(),
            "Reports directory should be auto-created"
        );
    }

    #[test]
    fn test_report_duration_from_elapsed() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert!((report.duration_secs - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_report_per_tool_metrics() {
        let mut result = test_result(); // already has breaking_point: None
        result.snapshot.per_tool = vec![
            ToolSnapshot {
                name: "calculate".to_string(),
                p50: 42,
                p95: 120,
                p99: 350,
                min: 5,
                max: 1200,
                mean: 68.3,
                total_requests: 680,
                success_count: 666,
                error_count: 14,
                error_rate: 14.0 / 680.0,
                error_categories: HashMap::from([
                    ("timeout".to_string(), 10),
                    ("jsonrpc".to_string(), 4),
                ]),
            },
            ToolSnapshot {
                name: "search".to_string(),
                p50: 85,
                p95: 250,
                p99: 800,
                min: 10,
                max: 2000,
                mean: 150.0,
                total_requests: 120,
                success_count: 110,
                error_count: 10,
                error_rate: 10.0 / 120.0,
                error_categories: HashMap::from([("timeout".to_string(), 10)]),
            },
        ];

        let report =
            LoadTestReport::from_result(&result, &test_config(), "http://localhost:3000/mcp");

        // Verify per_tool has both tools
        assert_eq!(report.per_tool.len(), 2);

        // Verify calculate tool
        let calc = report.per_tool.get("calculate").expect("calculate tool");
        assert_eq!(calc.total_requests, 680);
        assert_eq!(calc.success_count, 666);
        assert_eq!(calc.error_count, 14);
        assert_eq!(calc.latency.p50_ms, 42);
        assert_eq!(calc.latency.p95_ms, 120);
        assert_eq!(calc.latency.p99_ms, 350);
        assert_eq!(calc.latency.min_ms, 5);
        assert_eq!(calc.latency.max_ms, 1200);
        assert!((calc.latency.mean_ms - 68.3).abs() < 0.1);
        assert_eq!(calc.errors.get("timeout"), Some(&10));
        assert_eq!(calc.errors.get("jsonrpc"), Some(&4));

        // Verify serialized JSON structure
        let json = serde_json::to_string_pretty(&report).expect("should serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
        assert!(parsed["per_tool"]["calculate"]["latency"]["p50_ms"].is_u64());
        assert!(parsed["per_tool"]["search"]["total_requests"].is_u64());
    }

    #[test]
    fn test_report_per_tool_empty_when_no_tools() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );
        assert!(
            report.per_tool.is_empty(),
            "per_tool should be empty when no tool-specific samples"
        );

        // Verify empty object in JSON
        let json = serde_json::to_string_pretty(&report).expect("should serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
        assert!(parsed["per_tool"].is_object());
        assert_eq!(
            parsed["per_tool"].as_object().unwrap().len(),
            0,
            "per_tool should be empty object"
        );
    }

    #[test]
    fn test_report_breaking_point_when_detected() {
        use crate::loadtest::breaking::BreakingPoint;
        use std::time::Instant;

        let mut result = test_result();
        result.breaking_point = Some(BreakingPoint {
            vus: 25,
            reason: "error_rate_spike".to_string(),
            detail: "Error rate 15.0% exceeds threshold".to_string(),
            detected_at: Instant::now(),
        });

        let report =
            LoadTestReport::from_result(&result, &test_config(), "http://localhost:3000/mcp");

        assert!(report.breaking_point.detected);
        assert_eq!(report.breaking_point.vus, Some(25));
        assert_eq!(
            report.breaking_point.reason,
            Some("error_rate_spike".to_string())
        );
        assert!(report.breaking_point.detail.is_some());
        assert!(report.breaking_point.timestamp.is_some());

        // Verify JSON serialization
        let json = serde_json::to_string_pretty(&report).expect("should serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
        assert_eq!(parsed["breaking_point"]["detected"], true);
        assert_eq!(parsed["breaking_point"]["vus"], 25);
        assert_eq!(parsed["breaking_point"]["reason"], "error_rate_spike");
    }

    #[test]
    fn test_report_breaking_point_when_not_detected() {
        let report = LoadTestReport::from_result(
            &test_result(),
            &test_config(),
            "http://localhost:3000/mcp",
        );

        assert!(!report.breaking_point.detected);
        assert!(report.breaking_point.vus.is_none());
        assert!(report.breaking_point.reason.is_none());
        assert!(report.breaking_point.detail.is_none());
        assert!(report.breaking_point.timestamp.is_none());

        // Verify JSON serialization -- null optionals should be omitted
        let json = serde_json::to_string_pretty(&report).expect("should serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
        assert_eq!(parsed["breaking_point"]["detected"], false);
        assert!(
            parsed["breaking_point"]["vus"].is_null(),
            "vus should be null/absent when not detected"
        );
    }
}
