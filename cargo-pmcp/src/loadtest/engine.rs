//! Load test execution engine with metrics aggregation and graceful shutdown.
//!
//! [`LoadTestEngine`] is the top-level orchestrator that:
//! - Spawns N virtual user tasks via [`tokio_util::task::TaskTracker`]
//! - Collects metrics through a bounded mpsc channel
//! - Publishes snapshots through a watch channel for live display
//! - Coordinates graceful shutdown via [`CancellationToken`]
//!
//! The engine supports two execution modes:
//! - **Flat load** (no stages): all VUs start immediately with optional ramp-up
//! - **Staged load** (`[[stage]]` blocks): VU count ramps linearly through stages

use crate::loadtest::breaking::{BreakingPoint, BreakingPointDetector};
use crate::loadtest::config::LoadTestConfig;
use crate::loadtest::display::display_loop;
use crate::loadtest::error::LoadTestError;
use crate::loadtest::metrics::{MetricsRecorder, MetricsSnapshot, RequestSample};
use crate::loadtest::vu::{vu_loop, ActiveVuCounter};

use pmcp::client::http_middleware::HttpMiddlewareChain;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Compile-time Send bounds verification for channel-transported types.
///
/// These assertions fail at compile time if the types are not `Send`,
/// which would prevent them from being used in mpsc/watch channels.
fn _assert_send<T: Send>() {}
#[allow(dead_code)]
fn _check_send_bounds() {
    _assert_send::<RequestSample>();
    _assert_send::<MetricsSnapshot>();
    _assert_send::<DisplayState>();
}

/// Display state published through the watch channel to the live terminal display.
///
/// Wraps a [`MetricsSnapshot`] with an optional stage label so the display
/// can render `[stage 2/3]` prefix during staged load tests.
#[derive(Debug, Clone)]
pub struct DisplayState {
    /// Current metrics snapshot.
    pub snapshot: MetricsSnapshot,
    /// Current stage label (e.g., `"stage 2/3"`), or `None` for flat load.
    pub stage_label: Option<String>,
    /// Breaking point warning message, set once when degradation is detected.
    pub breaking_point: Option<String>,
}

/// Top-level load test engine configuration and entry point.
///
/// Spawns N virtual users as independent tokio tasks, collects metrics
/// through a bounded mpsc channel, and publishes snapshots through a
/// watch channel for live display consumption.
pub struct LoadTestEngine {
    config: LoadTestConfig,
    base_url: String,
    max_iterations: Option<u64>,
    ramp_up: Option<Duration>,
    no_color: bool,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
}

impl LoadTestEngine {
    /// Creates a new engine with the given configuration and target URL.
    pub fn new(config: LoadTestConfig, base_url: String) -> Self {
        Self {
            config,
            base_url,
            max_iterations: None,
            ramp_up: None,
            no_color: false,
            http_middleware_chain: None,
        }
    }

    /// Sets an iteration limit. The test stops after this many total iterations
    /// across all VUs (first-limit-wins with duration).
    pub fn with_iterations(mut self, n: u64) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Sets a ramp-up duration. VUs are spawned with uniform stagger over this
    /// period. Ramp-up metrics are excluded from the final report.
    /// Only applies to flat load mode (no stages).
    pub fn with_ramp_up(mut self, duration: Duration) -> Self {
        self.ramp_up = Some(duration);
        self
    }

    /// Disables colored output.
    pub fn with_no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    /// Sets an optional HTTP middleware chain for authentication.
    ///
    /// The middleware chain is shared (via `Arc`) across all virtual users.
    /// It is applied in [`McpClient::send_request`] before each HTTP POST,
    /// allowing transparent injection of `Authorization` headers.
    pub fn with_http_middleware(mut self, chain: Option<Arc<HttpMiddlewareChain>>) -> Self {
        self.http_middleware_chain = chain;
        self
    }

    /// Returns a reference to the engine's configuration.
    pub fn config(&self) -> &LoadTestConfig {
        &self.config
    }

    /// Returns the configured max iterations, if any.
    pub fn max_iterations(&self) -> Option<u64> {
        self.max_iterations
    }

    /// Returns the configured ramp-up duration, if any.
    pub fn ramp_up(&self) -> Option<Duration> {
        self.ramp_up
    }

    /// Returns whether colored output is disabled.
    pub fn no_color(&self) -> bool {
        self.no_color
    }

    /// Run the load test. Returns the final metrics snapshot.
    ///
    /// This method branches on whether `[[stage]]` blocks are configured:
    /// - **Staged**: runs the stage scheduler with per-VU cancellation tokens
    /// - **Flat**: runs the original flat-load path with optional ramp-up
    pub async fn run(&self) -> Result<LoadTestResult, LoadTestError> {
        // Validate config
        self.config.validate()?;

        if self.config.has_stages() {
            self.run_staged().await
        } else {
            self.run_flat().await
        }
    }

    /// Run in flat load mode (no stages) -- original behavior preserved.
    async fn run_flat(&self) -> Result<LoadTestResult, LoadTestError> {
        let vu_count = self.config.settings.virtual_users;
        let cancel = CancellationToken::new();
        let tracker = TaskTracker::new();
        let active_vus = ActiveVuCounter::new();
        let iteration_counter = self.max_iterations.map(|_| Arc::new(AtomicU64::new(0)));
        let http_client = reqwest::Client::new();

        // Metrics channels
        let buffer_size = (vu_count as usize) * 100;
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(buffer_size);
        let expected_interval_ms = self.config.settings.expected_interval_ms;
        let initial_display_state = DisplayState {
            snapshot: MetricsRecorder::new(expected_interval_ms).snapshot(),
            stage_label: None,
            breaking_point: None,
        };
        let (display_tx, display_rx) = watch::channel(initial_display_state);

        // Spawn VU tasks with optional ramp-up
        let test_start = Instant::now();
        let config_arc = Arc::new(self.config.clone());

        let ramp_up_end: Instant;
        if let Some(ramp_duration) = self.ramp_up {
            let delay_per_vu = if vu_count > 1 {
                ramp_duration / vu_count
            } else {
                Duration::ZERO
            };
            for i in 0..vu_count {
                tracker.spawn(vu_loop(
                    i,
                    config_arc.clone(),
                    http_client.clone(),
                    self.base_url.clone(),
                    sample_tx.clone(),
                    cancel.clone(),
                    iteration_counter.clone(),
                    self.max_iterations,
                    active_vus.clone(),
                    self.http_middleware_chain.clone(),
                ));
                if i < vu_count - 1 {
                    tokio::time::sleep(delay_per_vu).await;
                }
            }
            ramp_up_end = Instant::now();
        } else {
            for i in 0..vu_count {
                tracker.spawn(vu_loop(
                    i,
                    config_arc.clone(),
                    http_client.clone(),
                    self.base_url.clone(),
                    sample_tx.clone(),
                    cancel.clone(),
                    iteration_counter.clone(),
                    self.max_iterations,
                    active_vus.clone(),
                    self.http_middleware_chain.clone(),
                ));
            }
            ramp_up_end = test_start; // No ramp-up, all metrics count
        }

        // Drop original sender -- VUs hold their own clones
        drop(sample_tx);

        // Shared breaking point holder -- aggregator writes, engine reads after completion
        let breaking_point_holder = Arc::new(std::sync::Mutex::new(None::<BreakingPoint>));

        // Spawn metrics aggregator (NOT on tracker -- must outlive VU tasks)
        let aggregator_cancel = cancel.clone();
        let bp_holder_clone = breaking_point_holder.clone();
        let aggregator_handle = tokio::spawn(metrics_aggregator(
            sample_rx,
            display_tx,
            aggregator_cancel,
            expected_interval_ms,
            ramp_up_end,
            None, // No stage label for flat mode
            bp_holder_clone,
            active_vus.clone(),
        ));

        // Spawn live display task
        let display_cancel = cancel.clone();
        let display_vus = active_vus.clone();
        let display_rx_clone = display_rx.clone();
        let target_vus = vu_count;
        let no_color = self.no_color;
        let display_handle = tokio::spawn(display_loop(
            display_rx_clone,
            display_vus,
            target_vus,
            display_cancel,
            no_color,
            test_start,
        ));

        // Run controller -- first-limit-wins between duration, iteration limit, Ctrl+C
        let duration = Duration::from_secs(self.config.settings.duration_secs);
        let controller_cancel = cancel.clone();

        tokio::select! {
            _ = tokio::time::sleep(duration) => {
                controller_cancel.cancel();
            }
            _ = controller_cancel.cancelled() => {
                // Iteration limit or other cancellation triggered by VU
            }
            _ = handle_ctrl_c(cancel.clone()) => {
                // Ctrl+C handled inside
            }
        }

        // Drain: close tracker and wait for all VU tasks
        tracker.close();
        tracker.wait().await;

        // Wait for aggregator to finish processing remaining samples
        let _ = aggregator_handle.await;

        // Wait for display to render final state
        let _ = display_handle.await;

        // Collect final snapshot and breaking point
        let final_snapshot = display_rx.borrow().snapshot.clone();
        let breaking_point = breaking_point_holder.lock().unwrap().clone();

        Ok(LoadTestResult {
            snapshot: final_snapshot,
            elapsed: test_start.elapsed(),
            final_active_vus: active_vus.get(),
            breaking_point,
        })
    }

    /// Run in staged load mode -- ramps VU count through `[[stage]]` blocks.
    ///
    /// For each stage, the scheduler:
    /// 1. Computes how many VUs to add or remove to reach `target_vus`.
    /// 2. **Ramp up**: spawns VUs with linear stagger, each getting a `child_token()`.
    /// 3. **Ramp down**: cancels VU tokens in LIFO order (last spawned, first killed).
    /// 4. **Hold**: waits for remaining stage duration.
    async fn run_staged(&self) -> Result<LoadTestResult, LoadTestError> {
        let cancel = CancellationToken::new();
        let tracker = TaskTracker::new();
        let active_vus = ActiveVuCounter::new();
        let iteration_counter = self.max_iterations.map(|_| Arc::new(AtomicU64::new(0)));
        let http_client = reqwest::Client::new();

        // Use a generous buffer -- stages can have high VU counts
        let max_stage_vus = self
            .config
            .stage
            .iter()
            .map(|s| s.target_vus)
            .max()
            .unwrap_or(10);
        let buffer_size = (max_stage_vus as usize) * 100;
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(buffer_size);
        let expected_interval_ms = self.config.settings.expected_interval_ms;

        let total_stages = self.config.stage.len();
        let initial_label = Some(format!("stage 1/{total_stages}"));
        let initial_display_state = DisplayState {
            snapshot: MetricsRecorder::new(expected_interval_ms).snapshot(),
            stage_label: initial_label.clone(),
            breaking_point: None,
        };
        let (display_tx, display_rx) = watch::channel(initial_display_state);

        let test_start = Instant::now();
        let config_arc = Arc::new(self.config.clone());

        // For staged mode, ramp_up_end = test_start (all stage data is part of the test shape)
        let ramp_up_end = test_start;

        // Shared stage label for the aggregator to read
        let stage_label = Arc::new(std::sync::Mutex::new(initial_label));

        // Shared breaking point holder -- aggregator writes, engine reads after completion
        let breaking_point_holder = Arc::new(std::sync::Mutex::new(None::<BreakingPoint>));

        // Spawn metrics aggregator
        let aggregator_cancel = cancel.clone();
        let stage_label_clone = stage_label.clone();
        let bp_holder_clone = breaking_point_holder.clone();
        let aggregator_handle = tokio::spawn(metrics_aggregator_with_label(
            sample_rx,
            display_tx,
            aggregator_cancel,
            expected_interval_ms,
            ramp_up_end,
            stage_label_clone,
            bp_holder_clone,
            active_vus.clone(),
        ));

        // Spawn live display task
        let display_cancel = cancel.clone();
        let display_vus = active_vus.clone();
        let display_rx_clone = display_rx.clone();
        let no_color = self.no_color;
        // Target VUs starts at first stage target; display updates dynamically
        let display_target_vus = self.config.stage.first().map_or(0, |s| s.target_vus);
        let display_handle = tokio::spawn(display_loop(
            display_rx_clone,
            display_vus,
            display_target_vus,
            display_cancel,
            no_color,
            test_start,
        ));

        // Safety timeout: effective duration + 30s
        let safety_timeout = Duration::from_secs(self.config.effective_duration_secs() + 30);

        // Per-VU cancellation tokens for selective ramp-down (LIFO order)
        let mut vu_tokens: Vec<CancellationToken> = Vec::new();
        let mut next_vu_id: u32 = 0;

        // Stage scheduler loop
        let scheduler_result = tokio::select! {
            result = async {
                for (stage_idx, stage_config) in self.config.stage.iter().enumerate() {
                    let stage_start = Instant::now();
                    let stage_duration = Duration::from_secs(stage_config.duration_secs);
                    let target = stage_config.target_vus;
                    let current = active_vus.get();

                    // Update stage label
                    {
                        let label = format!("stage {}/{}", stage_idx + 1, total_stages);
                        *stage_label.lock().unwrap() = Some(label);
                    }

                    if target > current {
                        // Ramp up: spawn (target - current) VUs with linear stagger
                        let vus_to_spawn = target - current;
                        let delay_per_vu = if vus_to_spawn > 1 {
                            stage_duration / vus_to_spawn
                        } else {
                            Duration::ZERO
                        };

                        for spawn_idx in 0..vus_to_spawn {
                            if cancel.is_cancelled() {
                                return Ok(());
                            }

                            let child_token = cancel.child_token();
                            vu_tokens.push(child_token.clone());

                            tracker.spawn(vu_loop(
                                next_vu_id,
                                config_arc.clone(),
                                http_client.clone(),
                                self.base_url.clone(),
                                sample_tx.clone(),
                                child_token,
                                iteration_counter.clone(),
                                self.max_iterations,
                                active_vus.clone(),
                                self.http_middleware_chain.clone(),
                            ));
                            next_vu_id += 1;

                            // Stagger between spawns (not after last)
                            if spawn_idx < vus_to_spawn - 1 {
                                tokio::select! {
                                    _ = tokio::time::sleep(delay_per_vu) => {}
                                    _ = cancel.cancelled() => { return Ok(()); }
                                }
                            }
                        }
                    } else if target < current {
                        // Ramp down: cancel (current - target) VU tokens in LIFO order
                        let vus_to_remove = current - target;
                        for _ in 0..vus_to_remove {
                            if let Some(token) = vu_tokens.pop() {
                                token.cancel();
                            }
                        }
                    }
                    // else: hold at current level

                    // Wait for remaining stage time
                    let elapsed_in_stage = stage_start.elapsed();
                    if elapsed_in_stage < stage_duration {
                        let remaining = stage_duration - elapsed_in_stage;
                        tokio::select! {
                            _ = tokio::time::sleep(remaining) => {}
                            _ = cancel.cancelled() => { return Ok(()); }
                        }
                    }
                }
                Ok::<(), LoadTestError>(())
            } => result,
            _ = tokio::time::sleep(safety_timeout) => {
                eprintln!("Safety timeout reached, stopping test.");
                Ok(())
            }
            _ = handle_ctrl_c(cancel.clone()) => {
                Ok(())
            }
        };

        // Cancel all remaining VUs
        cancel.cancel();
        for token in &vu_tokens {
            token.cancel();
        }

        // Drop the scheduler's sender clone so aggregator can finish
        drop(sample_tx);

        // Drain: close tracker and wait for all VU tasks
        tracker.close();
        tracker.wait().await;

        // Wait for aggregator and display to finish
        let _ = aggregator_handle.await;
        let _ = display_handle.await;

        // Propagate any scheduler error
        scheduler_result?;

        // Collect final snapshot and breaking point
        let final_snapshot = display_rx.borrow().snapshot.clone();
        let breaking_point = breaking_point_holder.lock().unwrap().clone();

        Ok(LoadTestResult {
            snapshot: final_snapshot,
            elapsed: test_start.elapsed(),
            final_active_vus: active_vus.get(),
            breaking_point,
        })
    }
}

/// Result of a completed load test run.
#[derive(Debug)]
pub struct LoadTestResult {
    /// Final metrics snapshot (excludes ramp-up if applicable).
    pub snapshot: MetricsSnapshot,
    /// Total elapsed time of the test.
    pub elapsed: Duration,
    /// Number of VUs that were still active at test end.
    pub final_active_vus: u32,
    /// Breaking point event, if degradation was detected during the run.
    pub breaking_point: Option<BreakingPoint>,
}

/// Metrics aggregator task for flat load mode.
///
/// Consumes [`RequestSample`] values from the mpsc channel, records them
/// into dual recorders (live + report), and publishes [`DisplayState`] via the
/// watch channel every 2 seconds.
///
/// Uses `biased;` select to ensure the tick branch is checked first,
/// preventing display starvation when the mpsc channel is busy.
///
/// The dual-recorder pattern excludes ramp-up samples from the final report:
/// - `live` records ALL samples (for live display)
/// - `report` records only post-ramp-up samples (for final result)
#[allow(clippy::too_many_arguments)]
async fn metrics_aggregator(
    mut sample_rx: mpsc::Receiver<RequestSample>,
    display_tx: watch::Sender<DisplayState>,
    cancel: CancellationToken,
    expected_interval_ms: u64,
    ramp_up_end: Instant,
    stage_label: Option<String>,
    bp_holder: Arc<std::sync::Mutex<Option<BreakingPoint>>>,
    active_vus: ActiveVuCounter,
) {
    let mut live = MetricsRecorder::new(expected_interval_ms);
    let mut report = MetricsRecorder::new(expected_interval_ms);
    let mut detector = BreakingPointDetector::with_default_window();
    let mut bp_warning: Option<String> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;

            _ = tick.tick() => {
                // Drain all available samples before publishing snapshot
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let snapshot = live.snapshot();
                // Run breaking point detection on each tick
                if let Some(bp) = detector.observe(&snapshot, active_vus.get()) {
                    let warning = format!(
                        "Breaking point detected at {} VUs ({}: {})",
                        bp.vus, bp.reason, bp.detail
                    );
                    bp_warning = Some(warning);
                    *bp_holder.lock().unwrap() = Some(bp);
                }
                let _ = display_tx.send(DisplayState {
                    snapshot,
                    stage_label: stage_label.clone(),
                    breaking_point: bp_warning.clone(),
                });
            }
            result = sample_rx.recv() => {
                match result {
                    Some(sample) => {
                        if sample.timestamp >= ramp_up_end {
                            report.record(&sample);
                        }
                        live.record(&sample);
                    }
                    None => {
                        // All senders dropped -- VUs are done
                        let _ = display_tx.send(DisplayState {
                            snapshot: report.snapshot(),
                            stage_label: stage_label.clone(),
                            breaking_point: bp_warning.clone(),
                        });
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                // Drain remaining samples
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let _ = display_tx.send(DisplayState {
                    snapshot: report.snapshot(),
                    stage_label: stage_label.clone(),
                    breaking_point: bp_warning.clone(),
                });
                break;
            }
        }
    }
}

/// Metrics aggregator task for staged load mode.
///
/// Like [`metrics_aggregator`] but reads the current stage label from a shared
/// `Arc<Mutex<Option<String>>>` on each tick, so the display reflects the
/// current stage as the scheduler progresses.
#[allow(clippy::too_many_arguments)]
async fn metrics_aggregator_with_label(
    mut sample_rx: mpsc::Receiver<RequestSample>,
    display_tx: watch::Sender<DisplayState>,
    cancel: CancellationToken,
    expected_interval_ms: u64,
    ramp_up_end: Instant,
    stage_label: Arc<std::sync::Mutex<Option<String>>>,
    bp_holder: Arc<std::sync::Mutex<Option<BreakingPoint>>>,
    active_vus: ActiveVuCounter,
) {
    let mut live = MetricsRecorder::new(expected_interval_ms);
    let mut report = MetricsRecorder::new(expected_interval_ms);
    let mut detector = BreakingPointDetector::with_default_window();
    let mut bp_warning: Option<String> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;

            _ = tick.tick() => {
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let label = stage_label.lock().unwrap().clone();
                let snapshot = live.snapshot();
                // Run breaking point detection on each tick
                if let Some(bp) = detector.observe(&snapshot, active_vus.get()) {
                    let warning = format!(
                        "Breaking point detected at {} VUs ({}: {})",
                        bp.vus, bp.reason, bp.detail
                    );
                    bp_warning = Some(warning);
                    *bp_holder.lock().unwrap() = Some(bp);
                }
                let _ = display_tx.send(DisplayState {
                    snapshot,
                    stage_label: label,
                    breaking_point: bp_warning.clone(),
                });
            }
            result = sample_rx.recv() => {
                match result {
                    Some(sample) => {
                        if sample.timestamp >= ramp_up_end {
                            report.record(&sample);
                        }
                        live.record(&sample);
                    }
                    None => {
                        let label = stage_label.lock().unwrap().clone();
                        let _ = display_tx.send(DisplayState {
                            snapshot: report.snapshot(),
                            stage_label: label,
                            breaking_point: bp_warning.clone(),
                        });
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let label = stage_label.lock().unwrap().clone();
                let _ = display_tx.send(DisplayState {
                    snapshot: report.snapshot(),
                    stage_label: label,
                    breaking_point: bp_warning.clone(),
                });
                break;
            }
        }
    }
}

/// Ctrl+C handler with two-phase shutdown.
///
/// First Ctrl+C triggers graceful drain via the cancellation token.
/// Second Ctrl+C performs a hard abort via `std::process::exit(1)`.
async fn handle_ctrl_c(cancel: CancellationToken) {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl_c handler");
    eprintln!("\nReceived Ctrl+C, stopping gracefully...");
    cancel.cancel();

    // Second Ctrl+C: hard abort
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl_c handler");
    eprintln!("\nReceived second Ctrl+C, aborting immediately.");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::config::{LoadTestConfig, ScenarioStep, Settings, Stage};
    use crate::loadtest::metrics::{OperationType, RequestSample};

    fn minimal_config() -> LoadTestConfig {
        LoadTestConfig {
            settings: Settings {
                virtual_users: 2,
                duration_secs: 10,
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

    #[test]
    fn test_load_test_engine_builder() {
        let config = minimal_config();
        let engine = LoadTestEngine::new(config, "http://localhost:3000".to_string())
            .with_iterations(1000)
            .with_ramp_up(Duration::from_secs(10))
            .with_no_color(true);

        assert_eq!(engine.max_iterations(), Some(1000));
        assert_eq!(engine.ramp_up(), Some(Duration::from_secs(10)));
        assert!(engine.no_color());
        assert_eq!(engine.config().settings.virtual_users, 2);
    }

    #[tokio::test]
    async fn test_metrics_aggregator_processes_samples() {
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(100);
        let initial = DisplayState {
            snapshot: MetricsRecorder::new(100).snapshot(),
            stage_label: None,
            breaking_point: None,
        };
        let (display_tx, display_rx) = watch::channel(initial);
        let cancel = CancellationToken::new();
        let ramp_up_end = Instant::now();
        let bp_holder = Arc::new(std::sync::Mutex::new(None));

        // Send 5 success samples
        for _ in 0..5 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(10), None);
            sample_tx.send(sample).await.unwrap();
        }

        // Drop sender to signal completion
        drop(sample_tx);

        // Run aggregator to completion
        metrics_aggregator(
            sample_rx,
            display_tx,
            cancel,
            100,
            ramp_up_end,
            None,
            bp_holder,
            ActiveVuCounter::new(),
        )
        .await;

        let state = display_rx.borrow().clone();
        assert_eq!(
            state.snapshot.total_requests, 5,
            "Expected 5 total requests, got {}",
            state.snapshot.total_requests
        );
        assert!(state.stage_label.is_none(), "Flat mode has no stage label");
    }

    #[tokio::test]
    async fn test_metrics_aggregator_dual_recorder_excludes_ramp_up() {
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(100);
        let initial = DisplayState {
            snapshot: MetricsRecorder::new(10_000).snapshot(),
            stage_label: None,
            breaking_point: None,
        };
        let (display_tx, display_rx) = watch::channel(initial);
        let cancel = CancellationToken::new();
        let bp_holder = Arc::new(std::sync::Mutex::new(None));

        // Set ramp_up_end 50ms in the future
        let ramp_up_end = Instant::now() + Duration::from_millis(50);

        // Send 3 samples immediately (before ramp_up_end -- ramp-up period)
        for _ in 0..3 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(10), None);
            sample_tx.send(sample).await.unwrap();
        }

        // Wait past the ramp-up boundary
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Send 2 samples after ramp_up_end (post-ramp-up)
        for _ in 0..2 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(20), None);
            sample_tx.send(sample).await.unwrap();
        }

        // Drop sender to signal completion
        drop(sample_tx);

        // Run aggregator -- final snapshot uses the report recorder (post-ramp-up only)
        metrics_aggregator(
            sample_rx,
            display_tx,
            cancel,
            10_000,
            ramp_up_end,
            None,
            bp_holder,
            ActiveVuCounter::new(),
        )
        .await;

        let state = display_rx.borrow().clone();
        // The report recorder should only have the 2 post-ramp-up samples
        assert_eq!(
            state.snapshot.total_requests, 2,
            "Expected 2 post-ramp-up requests, got {}",
            state.snapshot.total_requests
        );
    }

    #[tokio::test]
    async fn test_engine_run_with_no_color_does_not_panic() {
        // Smoke test: engine handles VU connection failures gracefully.
        // VUs will fail to connect (no server), die after respawn attempts,
        // and engine returns a result with 0 successful requests.
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 1,
                duration_secs: 5,
                timeout_ms: 500,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };
        let engine = LoadTestEngine::new(config, "http://127.0.0.1:1".to_string())
            .with_no_color(true)
            .with_iterations(1);

        let result = engine.run().await;
        assert!(
            result.is_ok(),
            "Engine should not panic or error: {result:?}"
        );
        let result = result.unwrap();
        assert_eq!(
            result.snapshot.success_count, 0,
            "No server, so no successes"
        );
    }

    #[test]
    fn test_load_test_result_fields() {
        let snapshot = MetricsRecorder::new(100).snapshot();
        let result = LoadTestResult {
            snapshot,
            elapsed: Duration::from_secs(30),
            final_active_vus: 5,
            breaking_point: None,
        };
        assert_eq!(result.elapsed, Duration::from_secs(30));
        assert_eq!(result.final_active_vus, 5);
        assert_eq!(result.snapshot.total_requests, 0);
    }

    #[test]
    fn test_load_test_engine_builder_with_stages() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 1,
                duration_secs: 10,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![
                Stage {
                    target_vus: 5,
                    duration_secs: 10,
                },
                Stage {
                    target_vus: 10,
                    duration_secs: 20,
                },
                Stage {
                    target_vus: 0,
                    duration_secs: 10,
                },
            ],
        };
        let engine =
            LoadTestEngine::new(config, "http://localhost:3000".to_string()).with_no_color(true);

        assert!(engine.config().has_stages());
        assert_eq!(engine.config().stage.len(), 3);
        assert_eq!(engine.config().effective_duration_secs(), 40);
    }

    #[test]
    fn test_display_state_clone_and_debug() {
        let snapshot = MetricsRecorder::new(100).snapshot();
        let state = DisplayState {
            snapshot,
            stage_label: Some("stage 2/3".to_string()),
            breaking_point: None,
        };
        let cloned = state.clone();
        assert_eq!(cloned.stage_label, Some("stage 2/3".to_string()));

        // Verify Debug impl exists (compile-time check + runtime format)
        let debug_str = format!("{:?}", cloned);
        assert!(
            debug_str.contains("DisplayState"),
            "Debug should contain struct name"
        );
    }

    #[test]
    fn test_display_state_no_stage_label() {
        let snapshot = MetricsRecorder::new(100).snapshot();
        let state = DisplayState {
            snapshot,
            stage_label: None,
            breaking_point: None,
        };
        assert!(state.stage_label.is_none());
    }
}
