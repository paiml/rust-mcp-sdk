//! k6-style terminal summary renderer for load test results.
//!
//! Produces a colorized, human-readable summary with:
//! - ASCII art header with test configuration
//! - Dotted-line metric rows (metric.........: value)
//! - Latency percentile breakdown
//! - Throughput and error rate
//! - Error classification breakdown
//!
//! The renderer is a pure function: [`render_summary`] takes structured data
//! and returns a formatted [`String`]. No I/O, no side effects -- easy to
//! unit test without terminal access.
//!
//! Color coding is applied via the `colored` crate, which respects
//! the global override set by [`colored::control::set_override(false)`]
//! when `--no-color` is active or stdout is piped.

use colored::Colorize;

use crate::loadtest::config::LoadTestConfig;
use crate::loadtest::engine::LoadTestResult;

/// Width for dotted metric row padding.
const PAD_WIDTH: usize = 40;

/// Render a k6-style terminal summary from load test results.
///
/// This is a pure function: takes data, returns a formatted String.
/// Color coding is applied via the `colored` crate, which respects
/// the global override set by `colored::control::set_override(false)`
/// when `--no-color` is active or output is piped.
///
/// # Layout
///
/// ```text
///           /\      |  cargo-pmcp loadtest
///          /  \     |
///     /\  /    \    |  target:    http://localhost:3000/mcp
///    /  \/      \   |  vus:       10
///   /    \       \  |  duration:  60s
///  /      \       \ |  scenarios: 3 steps
///
///   mcp_req_duration............: p50=45ms  p95=200ms  p99=450ms
///   mcp_req_success_count.......: 950
///   mcp_req_error_count.........: 50
///   mcp_req_error_rate..........: 5.0%
///   mcp_req_throughput..........: 15.8 req/s
///   mcp_req_total...............: 1000
///   mcp_req_elapsed.............: 60.0s
///
///   errors:
///     timeout...................: 30
///     jsonrpc...................: 15
///     http......................: 5
/// ```
pub fn render_summary(result: &LoadTestResult, config: &LoadTestConfig, url: &str) -> String {
    let snap = &result.snapshot;
    let mut lines = Vec::new();

    // ASCII art header
    lines.push(render_header(
        url,
        config.settings.virtual_users,
        config.settings.duration_secs,
        config.scenario.len(),
    ));

    // Latency metrics
    let latency_value = format!("p50={}ms  p95={}ms  p99={}ms", snap.p50, snap.p95, snap.p99);
    let latency_colored = if snap.p99 < 1000 {
        latency_value.green().to_string()
    } else {
        latency_value.yellow().to_string()
    };
    lines.push(format_metric_row(
        "mcp_req_duration",
        &latency_colored,
        PAD_WIDTH,
    ));

    // Success count
    lines.push(format_metric_row(
        "mcp_req_success_count",
        &snap.success_count.to_string().green().to_string(),
        PAD_WIDTH,
    ));

    // Error count
    let error_count_str = if snap.error_count > 0 {
        snap.error_count.to_string().red().to_string()
    } else {
        snap.error_count.to_string()
    };
    lines.push(format_metric_row(
        "mcp_req_error_count",
        &error_count_str,
        PAD_WIDTH,
    ));

    // Error rate
    let error_rate_pct = snap.error_rate * 100.0;
    let error_rate_str = format!("{error_rate_pct:.1}%");
    let error_rate_colored = if error_rate_pct > 5.0 {
        error_rate_str.red().to_string()
    } else if error_rate_pct > 1.0 {
        error_rate_str.yellow().to_string()
    } else {
        error_rate_str.green().to_string()
    };
    lines.push(format_metric_row(
        "mcp_req_error_rate",
        &error_rate_colored,
        PAD_WIDTH,
    ));

    // Throughput
    let elapsed_secs = result.elapsed.as_secs_f64();
    let throughput = if elapsed_secs > 0.0 {
        snap.total_requests as f64 / elapsed_secs
    } else {
        0.0
    };
    let throughput_str = format!("{throughput:.1} req/s");
    lines.push(format_metric_row(
        "mcp_req_throughput",
        &throughput_str.green().to_string(),
        PAD_WIDTH,
    ));

    // Total requests
    lines.push(format_metric_row(
        "mcp_req_total",
        &snap.total_requests.to_string(),
        PAD_WIDTH,
    ));

    // Elapsed time
    lines.push(format_metric_row(
        "mcp_req_elapsed",
        &format!("{elapsed_secs:.1}s"),
        PAD_WIDTH,
    ));

    // Error breakdown (only when errors exist)
    if !snap.error_category_counts.is_empty() {
        lines.push(String::new());
        lines.push("  errors:".to_string());
        let mut categories: Vec<_> = snap.error_category_counts.iter().collect();
        categories.sort_by(|a, b| b.1.cmp(a.1));
        for (category, count) in categories {
            let row = format_metric_row(
                &format!("    {category}"),
                &count.to_string().red().to_string(),
                PAD_WIDTH,
            );
            lines.push(row);
        }
    }

    // Per-tool metrics table (only when tool-specific metrics exist)
    if !snap.per_tool.is_empty() {
        let elapsed_secs = result.elapsed.as_secs_f64();
        lines.push(String::new());
        lines.push("  per-tool metrics:".to_string());
        lines.push(String::new());
        lines.push(format!(
            "  {:<30} {:>6} {:>9} {:>6} {:>7} {:>7} {:>7}",
            "tool", "reqs", "rate", "err%", "p50", "p95", "p99"
        ));
        lines.push(format!("  {}", "\u{2500}".repeat(76)));

        for tool in &snap.per_tool {
            let rate = if elapsed_secs > 0.0 {
                tool.total_requests as f64 / elapsed_secs
            } else {
                0.0
            };
            let err_pct = tool.error_rate * 100.0;

            // Color coding: error rate
            let err_str = format!("{err_pct:.1}%");
            let err_colored = if err_pct > 5.0 {
                err_str.red().to_string()
            } else if err_pct > 1.0 {
                err_str.yellow().to_string()
            } else {
                err_str.green().to_string()
            };

            // Color coding: P99 latency
            let p99_str = format!("{}ms", tool.p99);
            let p99_colored = if tool.p99 > 1000 {
                p99_str.yellow().to_string()
            } else {
                p99_str.green().to_string()
            };

            // Truncate tool name to fit column width
            let display_name = if tool.name.len() > 30 {
                format!("{}...", &tool.name[..27])
            } else {
                tool.name.clone()
            };

            lines.push(format!(
                "  {:<30} {:>6} {:>9} {:>6} {:>7} {:>7} {:>7}",
                display_name,
                tool.total_requests,
                format!("{rate:.1}/s"),
                err_colored,
                format!("{}ms", tool.p50),
                format!("{}ms", tool.p95),
                p99_colored,
            ));
        }
    }

    lines.join("\n")
}

/// Render the ASCII art header with test configuration details.
fn render_header(url: &str, vus: u32, duration_secs: u64, scenario_count: usize) -> String {
    format!(
        r#"
          /\      |  {}
         /  \     |
    /\  /    \    |  target:    {}
   /  \/      \   |  vus:       {}
  /    \       \  |  duration:  {}s
 /      \       \ |  scenarios: {} steps
"#,
        "cargo-pmcp loadtest".bold(),
        url,
        vus,
        duration_secs,
        scenario_count,
    )
}

/// Format a single metric row with dot-padding.
///
/// Produces: `"  metric_name..................: value_string"`
///
/// Uses Rust's fill-character formatting with `.` as fill and `<` alignment
/// to pad the metric name with dots up to `pad_width` characters.
fn format_metric_row(name: &str, value: &str, pad_width: usize) -> String {
    format!("  {name:.<pad_width$}: {value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};
    use crate::loadtest::engine::LoadTestResult;
    use crate::loadtest::metrics::{MetricsSnapshot, ToolSnapshot};
    use std::collections::HashMap;
    use std::time::Duration;

    /// Disable colors in tests for deterministic assertions.
    fn setup_no_color() {
        colored::control::set_override(false);
    }

    fn minimal_config() -> LoadTestConfig {
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

    fn success_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            p50: 42,
            p95: 120,
            p99: 350,
            error_p50: 0,
            error_p95: 0,
            error_p99: 0,
            success_count: 950,
            error_count: 50,
            total_requests: 1000,
            error_rate: 0.05,
            operation_counts: HashMap::new(),
            per_operation_errors: HashMap::new(),
            error_category_counts: HashMap::from([
                ("timeout".to_string(), 30),
                ("jsonrpc".to_string(), 15),
                ("http".to_string(), 5),
            ]),
            per_tool: Vec::new(),
        }
    }

    #[test]
    fn test_render_summary_contains_header() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(
            output.contains("cargo-pmcp loadtest"),
            "Missing header title"
        );
        assert!(
            output.contains("http://localhost:3000/mcp"),
            "Missing target URL"
        );
        assert!(output.contains("10"), "Missing VU count");
    }

    #[test]
    fn test_render_summary_contains_latency_metrics() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(output.contains("p50=42ms"), "Missing p50");
        assert!(output.contains("p95=120ms"), "Missing p95");
        assert!(output.contains("p99=350ms"), "Missing p99");
    }

    #[test]
    fn test_render_summary_contains_error_breakdown() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(output.contains("timeout"), "Missing timeout category");
        assert!(output.contains("jsonrpc"), "Missing jsonrpc category");
        assert!(output.contains("http"), "Missing http category");
        assert!(output.contains("30"), "Missing timeout count");
    }

    #[test]
    fn test_render_summary_no_errors_omits_error_section() {
        setup_no_color();
        let snapshot = MetricsSnapshot {
            p50: 42,
            p95: 120,
            p99: 350,
            error_p50: 0,
            error_p95: 0,
            error_p99: 0,
            success_count: 1000,
            error_count: 0,
            total_requests: 1000,
            error_rate: 0.0,
            operation_counts: HashMap::new(),
            per_operation_errors: HashMap::new(),
            error_category_counts: HashMap::new(),
            per_tool: Vec::new(),
        };
        let result = LoadTestResult {
            snapshot,
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(
            !output.contains("errors:"),
            "Should not have error section when no errors"
        );
    }

    #[test]
    fn test_format_metric_row_dot_padding() {
        let row = format_metric_row("test_metric", "42ms", 30);
        assert!(row.contains("test_metric"));
        assert!(row.contains("42ms"));
        assert!(row.contains(".."), "Should have dot padding");
    }

    #[test]
    fn test_render_summary_contains_throughput() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        // 1000 requests / 60 seconds = ~16.7 req/s
        assert!(output.contains("req/s"), "Missing throughput");
    }

    #[test]
    fn test_render_summary_error_categories_sorted_by_count_desc() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        // timeout (30) should appear before jsonrpc (15) which should appear before http (5)
        let timeout_pos = output.find("timeout").expect("timeout should appear");
        let jsonrpc_pos = output.find("jsonrpc").expect("jsonrpc should appear");
        let http_err_pos = output.rfind("http").expect("http should appear in errors");
        assert!(
            timeout_pos < jsonrpc_pos,
            "timeout should appear before jsonrpc"
        );
        assert!(
            jsonrpc_pos < http_err_pos,
            "jsonrpc should appear before http"
        );
    }

    #[test]
    fn test_render_summary_contains_elapsed_time() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(output.contains("60.0s"), "Missing elapsed time");
    }

    fn make_tool_snapshot(name: &str, reqs: u64, p50: u64, p95: u64, p99: u64) -> ToolSnapshot {
        ToolSnapshot {
            name: name.to_string(),
            p50,
            p95,
            p99,
            min: p50,
            max: p99,
            mean: p50 as f64,
            total_requests: reqs,
            success_count: reqs,
            error_count: 0,
            error_rate: 0.0,
            error_categories: HashMap::new(),
        }
    }

    #[test]
    fn test_render_summary_per_tool_section() {
        setup_no_color();
        let mut snapshot = success_snapshot();
        snapshot.per_tool = vec![
            make_tool_snapshot("calculate", 680, 42, 120, 350),
            make_tool_snapshot("search", 120, 85, 250, 800),
        ];
        let result = LoadTestResult {
            snapshot,
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(
            output.contains("per-tool metrics:"),
            "Missing per-tool section header"
        );
        assert!(
            output.contains("calculate"),
            "Missing tool name 'calculate'"
        );
        assert!(output.contains("search"), "Missing tool name 'search'");
        assert!(
            output.contains("680"),
            "Missing request count for calculate"
        );
        assert!(output.contains("120"), "Missing request count for search");
    }

    #[test]
    fn test_render_summary_no_per_tool_when_empty() {
        setup_no_color();
        let result = LoadTestResult {
            snapshot: success_snapshot(),
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        assert!(
            !output.contains("per-tool metrics:"),
            "Should not have per-tool section when per_tool is empty"
        );
    }

    #[test]
    fn test_render_summary_per_tool_sorted() {
        setup_no_color();
        let mut snapshot = success_snapshot();
        // Already sorted alphabetically by MetricsRecorder::snapshot()
        snapshot.per_tool = vec![
            make_tool_snapshot("alpha", 100, 10, 20, 30),
            make_tool_snapshot("beta", 200, 20, 40, 60),
            make_tool_snapshot("gamma", 50, 5, 10, 15),
        ];
        let result = LoadTestResult {
            snapshot,
            elapsed: Duration::from_secs(60),
            final_active_vus: 10,
            breaking_point: None,
        };
        let config = minimal_config();
        let output = render_summary(&result, &config, "http://localhost:3000/mcp");

        let alpha_pos = output.find("alpha").expect("alpha should appear");
        let beta_pos = output.find("beta").expect("beta should appear");
        let gamma_pos = output.find("gamma").expect("gamma should appear");
        assert!(alpha_pos < beta_pos, "alpha should appear before beta");
        assert!(beta_pos < gamma_pos, "beta should appear before gamma");
    }
}
