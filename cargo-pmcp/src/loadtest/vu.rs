//! Virtual user (VU) task loop for load test execution.
//!
//! Each VU owns its own [`McpClient`] session and independently executes
//! weighted-random scenario steps. Failed sessions are respawned with
//! exponential backoff. Metrics are emitted as [`RequestSample`] values
//! through a bounded mpsc channel.

use crate::loadtest::client::McpClient;
use crate::loadtest::config::{LoadTestConfig, ScenarioStep};
use crate::loadtest::error::McpError;
use crate::loadtest::metrics::{OperationType, RequestSample};

use pmcp::client::http_middleware::HttpMiddlewareChain;
use rand::distr::weighted::WeightedIndex;
use rand::prelude::*;
use reqwest::Client;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Maximum number of respawn attempts before a VU permanently dies.
const MAX_RESPAWN_ATTEMPTS: u32 = 3;

/// Base backoff duration for respawn attempts (milliseconds).
const BASE_BACKOFF_MS: u64 = 500;

/// Atomic counter tracking the number of currently active virtual users.
///
/// Lightweight wrapper around `Arc<AtomicU32>` for clone-friendly sharing
/// across VU tasks and the engine orchestrator.
#[derive(Clone)]
pub struct ActiveVuCounter(Arc<AtomicU32>);

impl ActiveVuCounter {
    /// Creates a new counter initialized to zero.
    pub fn new() -> Self {
        Self(Arc::new(AtomicU32::new(0)))
    }

    /// Increments the active VU count by one.
    pub fn increment(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrements the active VU count by one.
    pub fn decrement(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    /// Returns the current number of active VUs.
    pub fn get(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for ActiveVuCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the [`OperationType`] corresponding to a [`ScenarioStep`] variant.
///
/// This is a pure mapping function used for metrics classification.
pub fn step_to_operation_type(step: &ScenarioStep) -> OperationType {
    match step {
        ScenarioStep::ToolCall { .. } => OperationType::ToolsCall,
        ScenarioStep::ResourceRead { .. } => OperationType::ResourcesRead,
        ScenarioStep::PromptGet { .. } => OperationType::PromptsGet,
        ScenarioStep::CodeMode { .. } => OperationType::CodeMode,
    }
}

/// Executes a single scenario step against the MCP server.
///
/// Returns the operation type and the result (success or error).
async fn execute_step(
    client: &mut McpClient,
    step: &ScenarioStep,
) -> (OperationType, Result<(), McpError>) {
    match step {
        ScenarioStep::ToolCall {
            tool, arguments, ..
        } => {
            let result = client.call_tool(tool, arguments).await;
            (OperationType::ToolsCall, result.map(|_| ()))
        },
        ScenarioStep::ResourceRead { uri, .. } => {
            let result = client.read_resource(uri).await;
            (OperationType::ResourcesRead, result.map(|_| ()))
        },
        ScenarioStep::PromptGet {
            prompt, arguments, ..
        } => {
            let result = client.get_prompt(prompt, arguments).await;
            (OperationType::PromptsGet, result.map(|_| ()))
        },
        ScenarioStep::CodeMode { code, format, .. } => {
            let result = client.execute_code_mode(code, format).await;
            (OperationType::CodeMode, result.map(|_| ()))
        },
    }
}

/// Waits with exponential backoff before a respawn attempt.
///
/// Returns `true` if the backoff completed normally, `false` if cancelled
/// (e.g., by shutdown signal). Returns `false` immediately if the attempt
/// count has reached [`MAX_RESPAWN_ATTEMPTS`].
///
/// Backoff formula: `base * 2^attempt` with +/-25% jitter.
async fn respawn_with_backoff(attempt: u32, cancel: &CancellationToken) -> bool {
    if attempt >= MAX_RESPAWN_ATTEMPTS {
        return false;
    }
    let backoff = Duration::from_millis(BASE_BACKOFF_MS * 2u64.pow(attempt));
    let jitter_range = backoff.as_millis() as u64 / 4;
    let jitter = if jitter_range > 0 {
        rand::rng().random_range(0..=jitter_range * 2) as i64 - jitter_range as i64
    } else {
        0
    };
    let actual = Duration::from_millis((backoff.as_millis() as i64 + jitter).max(0) as u64);
    tokio::select! {
        _ = tokio::time::sleep(actual) => true,
        _ = cancel.cancelled() => false,
    }
}

/// Returns `true` if the error indicates a session-fatal condition requiring respawn.
fn is_session_fatal(err: &McpError) -> bool {
    matches!(err, McpError::Connection { .. } | McpError::Timeout)
}

/// Attempts to create a new MCP client and perform the initialize handshake.
///
/// Returns the initialized client and the request sample for the initialize call,
/// or `None` if initialization failed after all respawn attempts.
#[allow(clippy::too_many_arguments)]
async fn try_initialize(
    vu_id: u32,
    http_client: &Client,
    base_url: &str,
    timeout: Duration,
    sample_tx: &mpsc::Sender<RequestSample>,
    cancel: &CancellationToken,
    max_attempts: u32,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
) -> Option<McpClient> {
    for attempt in 0..max_attempts {
        if cancel.is_cancelled() {
            return None;
        }

        let mut client = McpClient::new(
            http_client.clone(),
            base_url.to_owned(),
            timeout,
            http_middleware_chain.clone(),
        );
        let start = Instant::now();
        let result = client.initialize().await;
        let duration = start.elapsed();

        match result {
            Ok(_) => {
                let sample = RequestSample::success(OperationType::Initialize, duration, None);
                let _ = sample_tx.send(sample).await;
                return Some(client);
            },
            Err(err) => {
                let sample =
                    RequestSample::error(OperationType::Initialize, duration, err.clone(), None);
                let _ = sample_tx.send(sample).await;
                eprintln!(
                    "VU {vu_id}: initialize failed (attempt {}/{}): {err}",
                    attempt + 1,
                    max_attempts
                );
                if !respawn_with_backoff(attempt, cancel).await {
                    return None;
                }
            },
        }
    }
    None
}

/// Main virtual user task loop.
///
/// Each VU:
/// 1. Initializes its own MCP session via [`McpClient::initialize`].
/// 2. Loops executing weighted-random scenario steps until cancellation or
///    iteration limit.
/// 3. On session-fatal errors (connection/timeout), respawns with exponential
///    backoff (base 500ms, x2, +/-25% jitter), max 3 attempts before death.
/// 4. Sends [`RequestSample`] values through the bounded mpsc channel.
///
/// The `active_vus` counter is incremented on entry and decremented on all
/// exit paths.
#[allow(clippy::too_many_arguments)]
pub async fn vu_loop(
    vu_id: u32,
    config: Arc<LoadTestConfig>,
    http_client: Client,
    base_url: String,
    sample_tx: mpsc::Sender<RequestSample>,
    cancel: CancellationToken,
    iteration_counter: Option<Arc<AtomicU64>>,
    max_iterations: Option<u64>,
    active_vus: ActiveVuCounter,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
) {
    active_vus.increment();

    // Use a closure-like pattern: all early returns go through the cleanup block
    let result = vu_loop_inner(
        vu_id,
        &config,
        &http_client,
        &base_url,
        &sample_tx,
        &cancel,
        iteration_counter.as_ref(),
        max_iterations,
        http_middleware_chain,
    )
    .await;

    if let Err(reason) = result {
        eprintln!("VU {vu_id}: permanently dead -- {reason}");
    }

    active_vus.decrement();
}

/// Inner VU loop logic, separated for clean exit handling.
///
/// Returns `Ok(())` on normal shutdown, `Err(reason)` on permanent death.
#[allow(clippy::too_many_arguments)]
async fn vu_loop_inner(
    vu_id: u32,
    config: &LoadTestConfig,
    http_client: &Client,
    base_url: &str,
    sample_tx: &mpsc::Sender<RequestSample>,
    cancel: &CancellationToken,
    iteration_counter: Option<&Arc<AtomicU64>>,
    max_iterations: Option<u64>,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
) -> Result<(), String> {
    let timeout = config.settings.timeout_as_duration();

    // Initialize phase
    let mut client = try_initialize(
        vu_id,
        http_client,
        base_url,
        timeout,
        sample_tx,
        cancel,
        MAX_RESPAWN_ATTEMPTS,
        http_middleware_chain.clone(),
    )
    .await
    .ok_or_else(|| "all initialize attempts failed".to_string())?;

    // Build weighted distribution for step selection
    let weights: Vec<u32> = config.scenario.iter().map(|s| s.weight()).collect();
    let dist = WeightedIndex::new(&weights)
        .map_err(|e| format!("failed to build weighted distribution: {e}"))?;
    let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());

    // Load generation loop
    loop {
        // Pre-flight cancellation check
        if cancel.is_cancelled() {
            return Ok(());
        }

        // Iteration limit check (first-limit-wins with minor overshoot acceptable)
        if let (Some(counter), Some(max)) = (iteration_counter, max_iterations) {
            let prev = counter.fetch_add(1, Ordering::Relaxed);
            if prev >= max {
                cancel.cancel();
                return Ok(());
            }
        }

        // Select and execute a weighted-random step
        let step_idx = dist.sample(&mut rng);
        let step = &config.scenario[step_idx];

        let start = Instant::now();
        let (op_type, result) = execute_step(&mut client, step).await;
        let duration = start.elapsed();

        // Extract tool_name from the scenario step for per-tool metrics
        let tool_name = match step {
            ScenarioStep::ToolCall { tool, .. } => Some(tool.clone()),
            ScenarioStep::ResourceRead { uri, .. } => Some(uri.clone()),
            ScenarioStep::PromptGet { prompt, .. } => Some(prompt.clone()),
            ScenarioStep::CodeMode { format, .. } => Some(format!("code_mode/{format}")),
        };

        // Build and send the metrics sample
        let sample = match &result {
            Ok(()) => RequestSample::success(op_type, duration, tool_name),
            Err(err) => RequestSample::error(op_type, duration, err.clone(), tool_name),
        };

        if sample_tx.send(sample).await.is_err() {
            // Receiver dropped -- metrics aggregator is gone
            return Ok(());
        }

        // Handle session-fatal errors with respawn
        if let Err(ref err) = result {
            if is_session_fatal(err) {
                client = try_initialize(
                    vu_id,
                    http_client,
                    base_url,
                    timeout,
                    sample_tx,
                    cancel,
                    MAX_RESPAWN_ATTEMPTS,
                    http_middleware_chain.clone(),
                )
                .await
                .ok_or_else(|| "all respawn attempts failed".to_string())?;
            }
        }

        // Pace requests when request_interval_ms is configured
        if let Some(interval_ms) = config.settings.request_interval_ms {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(interval_ms)) => {},
                _ = cancel.cancelled() => return Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::config::ScenarioStep;

    #[test]
    fn test_active_vu_counter_increment_decrement() {
        let counter = ActiveVuCounter::new();
        assert_eq!(counter.get(), 0);

        counter.increment();
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 3);

        counter.decrement();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_step_to_operation_type_tool_call() {
        let step = ScenarioStep::ToolCall {
            weight: 10,
            tool: "echo".to_string(),
            arguments: serde_json::Value::Null,
        };
        assert_eq!(step_to_operation_type(&step), OperationType::ToolsCall);
    }

    #[test]
    fn test_step_to_operation_type_resource_read() {
        let step = ScenarioStep::ResourceRead {
            weight: 10,
            uri: "file:///data".to_string(),
        };
        assert_eq!(step_to_operation_type(&step), OperationType::ResourcesRead);
    }

    #[test]
    fn test_step_to_operation_type_prompt_get() {
        let step = ScenarioStep::PromptGet {
            weight: 10,
            prompt: "summarize".to_string(),
            arguments: std::collections::HashMap::new(),
        };
        assert_eq!(step_to_operation_type(&step), OperationType::PromptsGet);
    }

    #[tokio::test]
    async fn test_respawn_with_backoff_returns_false_at_max() {
        let cancel = CancellationToken::new();
        let result = respawn_with_backoff(MAX_RESPAWN_ATTEMPTS, &cancel).await;
        assert!(
            !result,
            "Should return false at max attempts without sleeping"
        );
    }

    #[tokio::test]
    async fn test_respawn_with_backoff_respects_cancellation() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = respawn_with_backoff(0, &cancel).await;
        assert!(
            !result,
            "Should return false when cancel token is already cancelled"
        );
    }

    #[test]
    fn test_is_session_fatal_connection() {
        let err = McpError::Connection {
            message: "refused".to_string(),
        };
        assert!(is_session_fatal(&err));
    }

    #[test]
    fn test_is_session_fatal_timeout() {
        assert!(is_session_fatal(&McpError::Timeout));
    }

    #[test]
    fn test_is_not_session_fatal_jsonrpc() {
        let err = McpError::JsonRpc {
            code: -32600,
            message: "Bad request".to_string(),
        };
        assert!(!is_session_fatal(&err));
    }

    #[test]
    fn test_is_not_session_fatal_http() {
        let err = McpError::Http {
            status: 500,
            body: "Internal".to_string(),
        };
        assert!(!is_session_fatal(&err));
    }
}
