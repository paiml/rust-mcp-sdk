//! Parallel batch processing with order preservation.
//!
//! This module provides utilities for processing batches of requests in parallel
//! while maintaining the original order of responses.

use crate::error::Result;
use crate::types::{JSONRPCRequest, JSONRPCResponse};
use futures::future::join_all;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::Semaphore;

/// Configuration for parallel batch processing
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::ParallelBatchConfig;
///
/// // Default configuration for moderate load
/// let default_config = ParallelBatchConfig::default();
/// assert_eq!(default_config.max_concurrency, 10);
/// assert_eq!(default_config.request_timeout_ms, Some(30_000));
/// assert!(!default_config.fail_fast);
///
/// // High throughput configuration
/// let high_throughput = ParallelBatchConfig {
///     max_concurrency: 50,
///     fail_fast: false,
///     request_timeout_ms: Some(60_000), // 1 minute timeout
/// };
///
/// // Low latency configuration with fail-fast
/// let low_latency = ParallelBatchConfig {
///     max_concurrency: 5,
///     fail_fast: true, // Stop on first error
///     request_timeout_ms: Some(5_000), // 5 second timeout
/// };
///
/// // No timeout configuration for long-running operations
/// let no_timeout = ParallelBatchConfig {
///     max_concurrency: 20,
///     fail_fast: false,
///     request_timeout_ms: None, // No timeout
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ParallelBatchConfig {
    /// Maximum number of concurrent requests
    pub max_concurrency: usize,

    /// Whether to stop on first error
    pub fail_fast: bool,

    /// Timeout for individual requests (in milliseconds)
    pub request_timeout_ms: Option<u64>,
}

impl Default for ParallelBatchConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 10,
            fail_fast: false,
            request_timeout_ms: Some(30_000), // 30 seconds
        }
    }
}

/// Process a batch of requests in parallel while preserving order
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::{process_batch_parallel, ParallelBatchConfig};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create sample requests
/// let requests = vec![
///     JSONRPCRequest {
///         jsonrpc: "2.0".to_string(),
///         method: "tools.list".to_string(),
///         params: None,
///         id: RequestId::from(1i64),
///     },
///     JSONRPCRequest {
///         jsonrpc: "2.0".to_string(),
///         method: "resources.read".to_string(),
///         params: Some(json!({"uri": "file:///example.txt"})),
///         id: RequestId::from(2i64),
///     },
/// ];
///
/// // Define handler function
/// let handler = |request: JSONRPCRequest| async move {
///     // Simulate processing
///     JSONRPCResponse {
///         jsonrpc: "2.0".to_string(),
///         id: request.id,
///         payload: pmcp::types::jsonrpc::ResponsePayload::Result(
///             json!({"status": "processed", "method": request.method})
///         ),
///     }
/// };
///
/// // Process with custom config
/// let config = ParallelBatchConfig {
///     max_concurrency: 5,
///     fail_fast: false,
///     request_timeout_ms: Some(10_000),
/// };
///
/// let responses = process_batch_parallel(requests, handler, config).await?;
/// assert_eq!(responses.len(), 2);
/// # Ok(())
/// # }
/// ```
pub async fn process_batch_parallel<F, Fut>(
    requests: Vec<JSONRPCRequest>,
    handler: F,
    config: ParallelBatchConfig,
) -> Result<Vec<JSONRPCResponse>>
where
    F: Fn(JSONRPCRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = JSONRPCResponse> + Send + 'static,
{
    if requests.is_empty() {
        return Ok(vec![]);
    }

    // Create a semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(config.max_concurrency));

    // Create futures for each request with index tracking
    let indexed_futures: Vec<_> = requests
        .into_iter()
        .enumerate()
        .map(|(index, request)| {
            let handler = handler.clone();
            let semaphore = semaphore.clone();
            let timeout_ms = config.request_timeout_ms;

            async move {
                // Acquire semaphore permit
                let _permit = semaphore.acquire().await.unwrap();

                // Create the handler future
                let handler_future = handler(request);

                // Apply timeout if configured
                let response = if let Some(timeout_ms) = timeout_ms {
                    match tokio::time::timeout(
                        std::time::Duration::from_millis(timeout_ms),
                        handler_future,
                    )
                    .await
                    {
                        Ok(response) => response,
                        Err(_) => {
                            // Create timeout error response
                            JSONRPCResponse {
                                jsonrpc: "2.0".to_string(),
                                id: crate::types::RequestId::from("null"),
                                payload: crate::types::jsonrpc::ResponsePayload::Error(
                                    crate::types::jsonrpc::JSONRPCError {
                                        // Wire value -32603 preserved (byte-identical) via the
                                        // centralized table; a request timeout surfaces as an
                                        // internal error on the wire today (VERS-06).
                                        code: crate::types::protocol::error_codes::INTERNAL_ERROR,
                                        message: "Request timeout".to_string(),
                                        data: None,
                                    },
                                ),
                            }
                        },
                    }
                } else {
                    handler_future.await
                };

                (index, response)
            }
        })
        .collect();

    // Execute all futures concurrently
    let mut indexed_responses = join_all(indexed_futures).await;

    // Sort by original index to preserve order
    indexed_responses.sort_by_key(|(index, _)| *index);

    // Extract just the responses
    let responses: Vec<_> = indexed_responses
        .into_iter()
        .map(|(_, response)| response)
        .collect();

    Ok(responses)
}

/// Process a batch with a stateful handler that needs mutable access
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::{process_batch_parallel_stateful, ParallelBatchConfig};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// #[derive(Default)]
/// struct ProcessingState {
///     request_count: u64,
///     results: HashMap<String, String>,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let state = Arc::new(RwLock::new(ProcessingState::default()));
///
/// let requests = vec![
///     JSONRPCRequest {
///         jsonrpc: "2.0".to_string(),
///         method: "process".to_string(),
///         params: Some(json!({"data": "item1"})),
///         id: RequestId::from(1i64),
///     },
/// ];
///
/// let handler = |req: JSONRPCRequest, state: Arc<RwLock<ProcessingState>>| async move {
///     // Update shared state
///     {
///         let mut state = state.write().await;
///         state.request_count += 1;
///         state.results.insert(req.method.clone(), "processed".to_string());
///     }
///     
///     JSONRPCResponse {
///         jsonrpc: "2.0".to_string(),
///         id: req.id,
///         payload: pmcp::types::jsonrpc::ResponsePayload::Result(json!("ok")),
///     }
/// };
///
/// let responses = process_batch_parallel_stateful(
///     requests,
///     state.clone(),
///     handler,
///     ParallelBatchConfig::default()
/// ).await?;
///
/// // Check state was updated
/// let final_state = state.read().await;
/// assert_eq!(final_state.request_count, 1);
/// # Ok(())
/// # }
/// ```
pub async fn process_batch_parallel_stateful<S, F, Fut>(
    requests: Vec<JSONRPCRequest>,
    state: Arc<tokio::sync::RwLock<S>>,
    handler: F,
    config: ParallelBatchConfig,
) -> Result<Vec<JSONRPCResponse>>
where
    S: Send + Sync + 'static,
    F: Fn(JSONRPCRequest, Arc<tokio::sync::RwLock<S>>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = JSONRPCResponse> + Send + 'static,
{
    process_batch_parallel(
        requests,
        move |request| {
            let state = state.clone();
            handler(request, state)
        },
        config,
    )
    .await
}

/// Batch processor with advanced features
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::{BatchProcessor, ParallelBatchConfig};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create processor with custom config
/// let config = ParallelBatchConfig {
///     max_concurrency: 20,
///     fail_fast: false,
///     request_timeout_ms: Some(15_000),
/// };
/// let processor = BatchProcessor::new(config);
///
/// // Define requests
/// let requests = vec![
///     JSONRPCRequest {
///         jsonrpc: "2.0".to_string(),
///         method: "compute".to_string(),
///         params: Some(json!({"value": 42})),
///         id: RequestId::from(1i64),
///     },
/// ];
///
/// // Process with metrics tracking
/// let responses = processor.process(requests, |req| async move {
///     // Simulate processing
///     JSONRPCResponse {
///         jsonrpc: "2.0".to_string(),
///         id: req.id,
///         payload: pmcp::types::jsonrpc::ResponsePayload::Result(
///             json!({"result": "computed"})
///         ),
///     }
/// }).await?;
///
/// // Get metrics
/// let metrics = processor.metrics().await;
/// println!("Processed {} requests", metrics.total_requests);
/// println!("Success rate: {:.2}%",
///     (metrics.successful_responses as f64 / metrics.total_requests as f64) * 100.0
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BatchProcessor {
    config: ParallelBatchConfig,
    metrics: Arc<tokio::sync::RwLock<BatchMetrics>>,
}

/// Metrics for batch processing
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::BatchMetrics;
///
/// // Create and update metrics
/// let mut metrics = BatchMetrics::default();
/// metrics.total_requests = 100;
/// metrics.successful_responses = 95;
/// metrics.error_responses = 3;
/// metrics.timeout_responses = 2;
/// metrics.avg_processing_time_ms = 125.5;
/// metrics.max_processing_time_ms = 850;
///
/// // Calculate success rate
/// let success_rate = (metrics.successful_responses as f64 / metrics.total_requests as f64) * 100.0;
/// println!("Success rate: {:.2}%", success_rate);
///
/// // Check if performance is acceptable
/// if metrics.avg_processing_time_ms > 500.0 {
///     println!("Warning: Average processing time is high");
/// }
///
/// if metrics.timeout_responses > 0 {
///     println!("Warning: {} requests timed out", metrics.timeout_responses);
/// }
/// ```
#[derive(Debug, Default)]
pub struct BatchMetrics {
    /// Total requests processed
    pub total_requests: u64,

    /// Successful responses
    pub successful_responses: u64,

    /// Error responses
    pub error_responses: u64,

    /// Timeout responses
    pub timeout_responses: u64,

    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,

    /// Maximum processing time in milliseconds
    pub max_processing_time_ms: u64,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(config: ParallelBatchConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(tokio::sync::RwLock::new(BatchMetrics::default())),
        }
    }

    /// Process a batch of requests with metrics tracking
    pub async fn process<F, Fut>(
        &self,
        requests: Vec<JSONRPCRequest>,
        handler: F,
    ) -> Result<Vec<JSONRPCResponse>>
    where
        F: Fn(JSONRPCRequest) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = JSONRPCResponse> + Send + 'static,
    {
        let start_time = std::time::Instant::now();
        let request_count = requests.len() as u64;

        // Process the batch
        let responses = process_batch_parallel(requests, handler, self.config.clone()).await?;

        // Update metrics
        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        let mut metrics = self.metrics.write().await;

        metrics.total_requests += request_count;
        metrics.max_processing_time_ms = metrics.max_processing_time_ms.max(processing_time_ms);

        // Count response types
        for response in &responses {
            match &response.payload {
                crate::types::jsonrpc::ResponsePayload::Result(_) => {
                    metrics.successful_responses += 1;
                },
                crate::types::jsonrpc::ResponsePayload::Error(err) => {
                    if err.message.contains("timeout") {
                        metrics.timeout_responses += 1;
                    } else {
                        metrics.error_responses += 1;
                    }
                },
            }
        }

        // Update average processing time
        let total_time = metrics.avg_processing_time_ms.mul_add(
            (metrics.total_requests - request_count) as f64,
            processing_time_ms as f64,
        );
        metrics.avg_processing_time_ms = total_time / metrics.total_requests as f64;

        Ok(responses)
    }

    /// Get current metrics
    pub async fn metrics(&self) -> BatchMetrics {
        let metrics = self.metrics.read().await;
        BatchMetrics {
            total_requests: metrics.total_requests,
            successful_responses: metrics.successful_responses,
            error_responses: metrics.error_responses,
            timeout_responses: metrics.timeout_responses,
            avg_processing_time_ms: metrics.avg_processing_time_ms,
            max_processing_time_ms: metrics.max_processing_time_ms,
        }
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        *self.metrics.write().await = BatchMetrics::default();
    }
}

/// Type alias for a batch processing future
pub type BatchProcessingFuture = Pin<Box<dyn Future<Output = Result<Vec<JSONRPCResponse>>> + Send>>;

/// Create a rate-limited batch processor
///
/// # Examples
///
/// ```rust
/// use pmcp::utils::parallel_batch::{rate_limited_processor, ParallelBatchConfig};
/// use pmcp::types::{JSONRPCRequest, RequestId};
///
/// // Create processor that limits to 100 requests per second
/// let processor = rate_limited_processor(100, ParallelBatchConfig::default());
///
/// // Use with batch of requests
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let requests = vec![
///     JSONRPCRequest::new(1i64, "api.call", None::<serde_json::Value>),
///     // ... more requests  
/// ];
///
/// // Process will be rate-limited to 100 req/sec
/// // let future = processor(requests);
/// // Note: This example processor returns placeholder responses
/// // The actual usage would involve calling the processor function
/// # Ok(())
/// # }
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub fn rate_limited_processor(
    max_requests_per_second: usize,
    config: ParallelBatchConfig,
) -> impl Fn(Vec<JSONRPCRequest>) -> BatchProcessingFuture {
    let rate_limiter = Arc::new(tokio::sync::Semaphore::new(max_requests_per_second));
    let interval = std::time::Duration::from_secs(1) / max_requests_per_second as u32;

    move |requests| {
        let rate_limiter = rate_limiter.clone();
        let config = config.clone();

        Box::pin(async move {
            // Apply rate limiting
            let futures: Vec<_> = requests
                .into_iter()
                .map(|request| {
                    let rate_limiter = rate_limiter.clone();
                    async move {
                        let _permit = rate_limiter.acquire().await.unwrap();
                        tokio::time::sleep(interval).await;
                        request
                    }
                })
                .collect();

            let rate_limited_requests = join_all(futures).await;

            // Process with parallel batch
            process_batch_parallel(
                rate_limited_requests,
                |req| async move {
                    // Placeholder handler - in real use, this would be provided
                    JSONRPCResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        payload: crate::types::jsonrpc::ResponsePayload::Result(
                            serde_json::Value::Null,
                        ),
                    }
                },
                config,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RequestId;
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_parallel_batch_processing() {
        let requests = vec![
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "fast".to_string(),
                params: Some(json!({"delay": 10})),
                id: RequestId::from(1i64),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "slow".to_string(),
                params: Some(json!({"delay": 100})),
                id: RequestId::from(2i64),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "fast".to_string(),
                params: Some(json!({"delay": 10})),
                id: RequestId::from(3i64),
            },
        ];

        let handler = |req: JSONRPCRequest| async move {
            // Simulate processing delay
            if let Some(params) = req.params {
                if let Some(delay) = params.get("delay").and_then(|v| v.as_u64()) {
                    sleep(Duration::from_millis(delay)).await;
                }
            }

            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id.clone(),
                payload: crate::types::jsonrpc::ResponsePayload::Result(
                    json!({"method": req.method}),
                ),
            }
        };

        let start = std::time::Instant::now();
        let responses = process_batch_parallel(requests, handler, ParallelBatchConfig::default())
            .await
            .unwrap();
        let elapsed = start.elapsed();

        // Should complete faster than sequential (would take 120ms)
        assert!(elapsed.as_millis() < 120);

        // Check order is preserved
        assert_eq!(responses[0].id, RequestId::from(1i64));
        assert_eq!(responses[1].id, RequestId::from(2i64));
        assert_eq!(responses[2].id, RequestId::from(3i64));
    }

    #[tokio::test]
    async fn test_batch_processor_metrics() {
        let processor = BatchProcessor::new(ParallelBatchConfig::default());

        let requests = vec![
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "test".to_string(),
                params: None,
                id: RequestId::from(1i64),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "test".to_string(),
                params: None,
                id: RequestId::from(2i64),
            },
        ];

        let handler = |req: JSONRPCRequest| async move {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                payload: crate::types::jsonrpc::ResponsePayload::Result(json!(null)),
            }
        };

        processor.process(requests, handler).await.unwrap();

        let metrics = processor.metrics().await;
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.successful_responses, 2);
        assert_eq!(metrics.error_responses, 0);
    }

    #[tokio::test]
    async fn test_concurrent_limit() {
        let config = ParallelBatchConfig {
            max_concurrency: 2,
            ..Default::default()
        };

        let active = Arc::new(tokio::sync::RwLock::new(0));
        let max_active = Arc::new(tokio::sync::RwLock::new(0));

        let requests: Vec<_> = (1..=5)
            .map(|i| JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "test".to_string(),
                params: None,
                id: RequestId::from(i as i64),
            })
            .collect();

        let active_clone = active.clone();
        let max_active_clone = max_active.clone();

        let handler = move |req: JSONRPCRequest| {
            let active = active_clone.clone();
            let max_active = max_active_clone.clone();

            async move {
                // Increment active count
                {
                    let mut count = active.write().await;
                    *count += 1;
                    let current = *count;

                    let mut max = max_active.write().await;
                    if current > *max {
                        *max = current;
                    }
                }

                // Simulate work
                sleep(Duration::from_millis(50)).await;

                // Decrement active count
                {
                    let mut count = active.write().await;
                    *count -= 1;
                }

                JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    payload: crate::types::jsonrpc::ResponsePayload::Result(json!(null)),
                }
            }
        };

        process_batch_parallel(requests, handler, config)
            .await
            .unwrap();

        let max_concurrent = *max_active.read().await;
        assert_eq!(max_concurrent, 2);
    }
}
