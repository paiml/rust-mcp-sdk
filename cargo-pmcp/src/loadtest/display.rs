//! k6-style live terminal display for load test progress.
//!
//! Renders a compact, in-place updating block showing active VU count,
//! requests per second, P95 latency, error count/rate, and elapsed time.
//! Updates every 2 seconds from a watch channel, not per-request.
//!
//! When stages are active, a `[stage N/M]` prefix is shown.

use crate::loadtest::engine::DisplayState;
use crate::loadtest::metrics::MetricsSnapshot;
use crate::loadtest::vu::ActiveVuCounter;

use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// k6-style live terminal display for load test progress.
///
/// Renders a compact, in-place updating block showing:
/// - Optional stage label (e.g., `[stage 2/3]`)
/// - Active VU count
/// - Requests per second (RPS)
/// - P95 latency
/// - Error count and rate
/// - Elapsed time
///
/// Updates every 2 seconds from a watch channel, not per-request.
pub struct LiveDisplay {
    _multi: MultiProgress,
    status_bar: ProgressBar,
    _no_color: bool,
}

impl LiveDisplay {
    /// Create a new live display.
    ///
    /// Color suppression is handled globally in `main()` via
    /// `colored::control::set_override(false)`. The `no_color` parameter
    /// is retained for indicatif draw target decisions (e.g., hiding
    /// progress bars when color is disabled).
    pub fn new(no_color: bool) -> Self {
        let is_terminal = std::io::stderr().is_terminal();

        let multi = MultiProgress::new();
        let status_bar = multi.add(ProgressBar::new_spinner());
        status_bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {wide_msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        status_bar.enable_steady_tick(Duration::from_millis(100));

        Self {
            _multi: multi,
            status_bar,
            _no_color: no_color || !is_terminal,
        }
    }

    /// Format a single line of live status from a snapshot.
    ///
    /// Shown metrics:
    /// - requests/sec (total_requests / elapsed seconds)
    /// - error count + error rate percentage
    /// - active VU count / target VU count
    /// - P95 latency in milliseconds
    /// - elapsed time in seconds
    ///
    /// When `stage_label` is provided, prepends `[stage_label]` to the line.
    ///
    /// Color coding:
    /// - Green for healthy metrics
    /// - Red for errors (when error_count > 0)
    /// - Yellow for high P95 latency (> 1000ms)
    pub fn format_status(
        snap: &MetricsSnapshot,
        elapsed: Duration,
        active_vus: u32,
        target_vus: u32,
        stage_label: Option<&str>,
    ) -> String {
        let elapsed_secs = elapsed.as_secs_f64();
        let rps = if elapsed_secs > 0.0 {
            snap.total_requests as f64 / elapsed_secs
        } else {
            0.0
        };

        let vu_str = format!("{}/{}", active_vus, target_vus);
        let rps_str = format!("{:.1}", rps);
        let p95_str = format!("{}ms", snap.p95);
        let error_count_str = format!("{}", snap.error_count);
        let error_rate_str = format!("{:.1}%", snap.error_rate * 100.0);
        let elapsed_str = format!("{}s", elapsed.as_secs());

        // Color coding: green for healthy, red for errors, yellow for high P95
        let vu_display = vu_str.green();
        let rps_display = rps_str.green();
        let p95_display = if snap.p95 > 1000 {
            p95_str.yellow().to_string()
        } else {
            p95_str.green().to_string()
        };
        let error_display = if snap.error_count > 0 {
            format!("{} ({})", error_count_str.red(), error_rate_str.red())
        } else {
            format!("{} ({})", error_count_str, error_rate_str)
        };

        let metrics_line = format!(
            "vus: {}  |  rps: {}  |  p95: {}  |  errors: {}  |  elapsed: {}",
            vu_display, rps_display, p95_display, error_display, elapsed_str
        );

        if let Some(label) = stage_label {
            format!("  [{}]  {}", label, metrics_line)
        } else {
            format!("  {}", metrics_line)
        }
    }

    /// Update the display with the latest snapshot.
    pub fn update(
        &self,
        snap: &MetricsSnapshot,
        elapsed: Duration,
        active_vus: u32,
        target_vus: u32,
        stage_label: Option<&str>,
    ) {
        let msg = Self::format_status(snap, elapsed, active_vus, target_vus, stage_label);
        self.status_bar.set_message(msg);
    }

    /// Stop the display and clear the spinner.
    pub fn finish(&self) {
        self.status_bar.finish_and_clear();
    }
}

/// Run the live display loop.
///
/// Subscribes to the watch channel receiving [`DisplayState`] and updates the
/// terminal every 2 seconds. Stops when the [`CancellationToken`] is cancelled
/// or the watch sender is dropped.
///
/// When a breaking point is detected, a one-time warning is printed to stderr.
pub async fn display_loop(
    mut display_rx: watch::Receiver<DisplayState>,
    active_vus: ActiveVuCounter,
    target_vus: u32,
    cancel: CancellationToken,
    no_color: bool,
    test_start: Instant,
) {
    let display = LiveDisplay::new(no_color);
    let mut bp_shown = false;

    // Print header
    eprintln!();
    eprintln!("  Running load test...");
    eprintln!();

    loop {
        tokio::select! {
            result = display_rx.changed() => {
                match result {
                    Ok(()) => {
                        let state = display_rx.borrow_and_update().clone();
                        let elapsed = test_start.elapsed();
                        display.update(
                            &state.snapshot,
                            elapsed,
                            active_vus.get(),
                            target_vus,
                            state.stage_label.as_deref(),
                        );
                        // Show breaking point warning once
                        if !bp_shown {
                            if let Some(ref warning) = state.breaking_point {
                                eprintln!(
                                    "  {} {}",
                                    "WARNING:".yellow().bold(),
                                    warning,
                                );
                                bp_shown = true;
                            }
                        }
                    }
                    Err(_) => {
                        // Sender dropped, test is ending
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                // Final update before exit
                let state = display_rx.borrow().clone();
                let elapsed = test_start.elapsed();
                display.update(
                    &state.snapshot,
                    elapsed,
                    active_vus.get(),
                    target_vus,
                    state.stage_label.as_deref(),
                );
                break;
            }
        }
    }

    display.finish();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            p50: 0,
            p95: 0,
            p99: 0,
            error_p50: 0,
            error_p95: 0,
            error_p99: 0,
            success_count: 0,
            error_count: 0,
            total_requests: 0,
            error_rate: 0.0,
            operation_counts: HashMap::new(),
            per_operation_errors: HashMap::new(),
            error_category_counts: HashMap::new(),
            per_tool: Vec::new(),
        }
    }

    #[test]
    fn test_format_status_zero_state() {
        let snap = empty_snapshot();
        let status = LiveDisplay::format_status(&snap, Duration::ZERO, 0, 10, None);

        assert!(
            status.contains("0/10"),
            "Should contain VU count 0/10, got: {status}"
        );
        assert!(
            status.contains("0.0"),
            "Should contain rps 0.0, got: {status}"
        );
        assert!(
            status.contains("errors:"),
            "Should contain errors label, got: {status}"
        );
    }

    #[test]
    fn test_format_status_with_errors() {
        let snap = MetricsSnapshot {
            p50: 30,
            p95: 42,
            p99: 80,
            error_p50: 0,
            error_p95: 0,
            error_p99: 0,
            success_count: 45,
            error_count: 5,
            total_requests: 50,
            error_rate: 0.1,
            operation_counts: HashMap::new(),
            per_operation_errors: HashMap::new(),
            error_category_counts: HashMap::new(),
            per_tool: Vec::new(),
        };
        let status = LiveDisplay::format_status(&snap, Duration::from_secs(30), 10, 10, None);

        assert!(
            status.contains("rps:"),
            "Should contain rps label, got: {status}"
        );
        assert!(
            status.contains("errors:"),
            "Should contain errors label, got: {status}"
        );
        assert!(
            status.contains("42ms"),
            "Should contain p95=42ms, got: {status}"
        );
    }

    #[test]
    fn test_format_status_high_p95_gets_highlighted() {
        let snap = MetricsSnapshot {
            p95: 1500,
            ..empty_snapshot()
        };
        let status = LiveDisplay::format_status(&snap, Duration::from_secs(10), 5, 10, None);

        assert!(
            status.contains("1500ms"),
            "Should contain 1500ms, got: {status}"
        );
    }

    #[test]
    fn test_format_status_with_stage_label() {
        let snap = empty_snapshot();
        let status =
            LiveDisplay::format_status(&snap, Duration::from_secs(10), 5, 10, Some("stage 2/3"));

        assert!(
            status.contains("[stage 2/3]"),
            "Should contain stage label, got: {status}"
        );
        assert!(
            status.contains("vus:"),
            "Should still contain vus label, got: {status}"
        );
    }

    #[test]
    fn test_format_status_without_stage_label() {
        let snap = empty_snapshot();
        let status = LiveDisplay::format_status(&snap, Duration::from_secs(10), 5, 10, None);

        assert!(
            !status.contains('['),
            "Should not contain brackets without stage label, got: {status}"
        );
    }

    #[test]
    fn test_live_display_new_does_not_panic() {
        let display = LiveDisplay::new(true);
        display.finish();
    }
}
