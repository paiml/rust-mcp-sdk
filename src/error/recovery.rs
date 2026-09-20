//! Advanced error handling and recovery mechanisms.
//!
//! PMCP-4005: This module provides enterprise-grade error recovery strategies including:
//! - Adaptive retry policies with jitter and backoff strategies
//! - Advanced circuit breakers with health monitoring
//! - Bulk operation recovery with partial failure handling
//! - Cascading failure detection and recovery
//! - Health monitoring and recovery coordination
//! - Deadline-aware recovery with timeout management

use crate::error::{Error, ErrorCode, Result};
use crate::runtime::{self, RwLock};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// Recovery operation result for bulk operations.
#[derive(Debug)]
pub enum BulkRecoveryResult {
    /// All operations succeeded
    AllSuccess(Vec<serde_json::Value>),
    /// Partial success with some failures
    PartialSuccess {
        /// Successful operations with their index and result
        successes: Vec<(usize, serde_json::Value)>,
        /// Failed operations with their index and error
        failures: Vec<(usize, Error)>,
    },
    /// All operations failed
    AllFailed(Vec<Error>),
}

/// Advanced jitter strategies for retry delays.
#[derive(Debug, Clone, Copy)]
pub enum JitterStrategy {
    /// No jitter
    None,
    /// Full jitter: random delay between 0 and calculated delay
    Full,
    /// Equal jitter: half calculated delay + random half
    Equal,
    /// Decorrelated jitter: randomly distributed around calculated delay
    Decorrelated,
}

/// Health status for components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component is degraded but functional
    Degraded,
    /// Component is unhealthy
    Unhealthy,
    /// Component health is unknown
    Unknown,
}

/// Health check result.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Component identifier
    pub component: String,
    /// Health status
    pub status: HealthStatus,
    /// Response time in microseconds
    pub response_time_us: u64,
    /// Last check timestamp
    pub timestamp: SystemTime,
    /// Optional error message
    pub message: Option<String>,
}

/// Recovery coordination events.
#[derive(Debug, Clone)]
pub enum RecoveryEvent {
    /// Component health changed
    HealthChanged {
        /// Name of the component that changed
        component: String,
        /// Previous health status
        old_status: HealthStatus,
        /// New health status
        new_status: HealthStatus,
    },
    /// Recovery operation started
    RecoveryStarted {
        /// Unique identifier for the recovery operation
        operation_id: String,
        /// Strategy being used for recovery
        strategy: String,
    },
    /// Recovery operation completed
    RecoveryCompleted {
        /// Unique identifier for the recovery operation
        operation_id: String,
        /// Whether the recovery was successful
        success: bool,
        /// Duration of the recovery operation
        duration: Duration,
    },
    /// Cascading failure detected
    CascadingFailure {
        /// Component that triggered the cascade
        trigger_component: String,
        /// Components affected by the cascade
        affected_components: Vec<String>,
    },
}

/// Advanced deadline management for recovery operations.
#[derive(Debug, Clone)]
pub struct RecoveryDeadline {
    /// Absolute deadline for the operation
    pub deadline: Instant,
    /// Remaining time budget
    pub remaining: Duration,
    /// Whether deadline has been exceeded
    pub exceeded: bool,
}

impl RecoveryDeadline {
    /// Create a new deadline with the given timeout.
    pub fn new(timeout: Duration) -> Self {
        let deadline = Instant::now() + timeout;
        Self {
            deadline,
            remaining: timeout,
            exceeded: false,
        }
    }

    /// Update the remaining time and check if deadline exceeded.
    pub fn update(&mut self) -> bool {
        let now = Instant::now();
        if now >= self.deadline {
            self.remaining = Duration::ZERO;
            self.exceeded = true;
        } else {
            self.remaining = self.deadline - now;
        }
        self.exceeded
    }

    /// Check if there's enough time left for an operation.
    pub fn has_time_for(&self, duration: Duration) -> bool {
        !self.exceeded && self.remaining >= duration
    }
}

/// Advanced recovery metrics for observability.
#[derive(Debug, Default)]
pub struct RecoveryMetrics {
    /// Total recovery attempts
    total_attempts: AtomicU64,
    /// Successful recoveries
    successful_recoveries: AtomicU64,
    /// Failed recoveries
    failed_recoveries: AtomicU64,
    /// Total recovery time in microseconds
    total_recovery_time_us: AtomicU64,
    /// Circuit breaker trips
    circuit_breaker_trips: AtomicU64,
    /// Fallback executions
    fallback_executions: AtomicU64,
    /// Cascade prevention count
    cascade_preventions: AtomicU64,
}

impl RecoveryMetrics {
    /// Create new recovery metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a recovery attempt.
    pub fn record_attempt(&self) {
        self.total_attempts.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful recovery.
    pub fn record_success(&self, duration: Duration) {
        self.successful_recoveries.fetch_add(1, Ordering::Relaxed);
        self.total_recovery_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record a failed recovery.
    pub fn record_failure(&self, duration: Duration) {
        self.failed_recoveries.fetch_add(1, Ordering::Relaxed);
        self.total_recovery_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record a circuit breaker trip.
    pub fn record_circuit_trip(&self) {
        self.circuit_breaker_trips.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a fallback execution.
    pub fn record_fallback(&self) {
        self.fallback_executions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cascade prevention.
    pub fn record_cascade_prevention(&self) {
        self.cascade_preventions.fetch_add(1, Ordering::Relaxed);
    }

    /// Get success rate as a percentage.
    pub fn success_rate(&self) -> f64 {
        let total = self.total_attempts.load(Ordering::Relaxed);
        let successes = self.successful_recoveries.load(Ordering::Relaxed);
        if total > 0 {
            (successes as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Get average recovery time.
    pub fn average_recovery_time(&self) -> Duration {
        let total_time = self.total_recovery_time_us.load(Ordering::Relaxed);
        let attempts = self.total_attempts.load(Ordering::Relaxed);
        total_time
            .checked_div(attempts)
            .map_or(Duration::ZERO, Duration::from_micros)
    }

    /// Get circuit breaker trip count.
    pub fn circuit_breaker_trips(&self) -> u64 {
        self.circuit_breaker_trips.load(Ordering::Relaxed)
    }

    /// Get fallback execution count.
    pub fn fallback_executions(&self) -> u64 {
        self.fallback_executions.load(Ordering::Relaxed)
    }

    /// Get cascade prevention count.
    pub fn cascade_preventions(&self) -> u64 {
        self.cascade_preventions.load(Ordering::Relaxed)
    }
}

/// Error recovery strategy.
///
/// # Examples
///
/// ```rust
/// use pmcp::error::recovery::RecoveryStrategy;
/// use std::time::Duration;
///
/// // Fixed retry strategy
/// let fixed = RecoveryStrategy::RetryFixed {
///     attempts: 3,
///     delay: Duration::from_millis(500),
/// };
///
/// // Exponential backoff strategy
/// let exponential = RecoveryStrategy::RetryExponential {
///     attempts: 5,
///     initial_delay: Duration::from_millis(100),
///     max_delay: Duration::from_secs(30),
///     multiplier: 2.0,
/// };
///
/// // Circuit breaker strategy
/// let circuit = RecoveryStrategy::CircuitBreaker {
///     failure_threshold: 5,
///     success_threshold: 3,
///     timeout: Duration::from_mins(1),
/// };
/// ```
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with fixed delay.
    RetryFixed {
        /// Number of retry attempts
        attempts: u32,
        /// Delay between retries
        delay: Duration,
    },

    /// Retry with exponential backoff.
    RetryExponential {
        /// Number of retry attempts
        attempts: u32,
        /// Initial delay before first retry
        initial_delay: Duration,
        /// Maximum delay between retries
        max_delay: Duration,
        /// Backoff multiplier
        multiplier: f64,
    },

    /// Retry with exponential backoff and jitter.
    RetryAdaptive {
        /// Number of retry attempts
        attempts: u32,
        /// Initial delay before first retry
        initial_delay: Duration,
        /// Maximum delay between retries
        max_delay: Duration,
        /// Backoff multiplier
        multiplier: f64,
        /// Jitter strategy to prevent thundering herd
        jitter: JitterStrategy,
    },

    /// Fallback to alternative handler.
    Fallback,

    /// Circuit breaker pattern.
    CircuitBreaker {
        /// Number of failures before opening circuit
        failure_threshold: u32,
        /// Number of successes before closing circuit
        success_threshold: u32,
        /// Timeout duration for half-open state
        timeout: Duration,
    },

    /// Advanced circuit breaker with health monitoring.
    AdvancedCircuitBreaker {
        /// Number of failures before opening circuit
        failure_threshold: u32,
        /// Number of successes before closing circuit
        success_threshold: u32,
        /// Timeout duration for half-open state
        timeout: Duration,
        /// Health check interval for monitoring
        health_check_interval: Duration,
        /// Response time threshold for degraded status
        response_time_threshold_ms: u64,
    },

    /// Deadline-aware recovery with timeout management.
    DeadlineAware {
        /// Maximum total recovery time
        max_recovery_time: Duration,
        /// Base recovery strategy
        base_strategy: Box<Self>,
    },

    /// Bulk operation recovery strategy.
    BulkRecovery {
        /// Individual operation strategy
        individual_strategy: Box<Self>,
        /// Minimum success rate to consider bulk operation successful
        min_success_rate: f64,
        /// Whether to fail fast on first failure
        fail_fast: bool,
    },

    /// Cascade-aware recovery to prevent failure propagation.
    CascadeAware {
        /// Base recovery strategy
        base_strategy: Box<Self>,
        /// Component dependencies for cascade detection
        dependencies: Vec<String>,
        /// Isolation timeout to prevent cascade
        isolation_timeout: Duration,
    },

    /// No recovery, fail immediately.
    FailFast,
}

/// Error recovery policy.
///
/// # Examples
///
/// ```rust
/// use pmcp::error::{recovery::RecoveryPolicy, ErrorCode};
/// use pmcp::error::recovery::RecoveryStrategy;
/// use std::time::Duration;
///
/// // Create a default policy
/// let default_policy = RecoveryPolicy::default();
///
/// // Create custom policy
/// let mut policy = RecoveryPolicy::new(
///     RecoveryStrategy::RetryFixed {
///         attempts: 2,
///         delay: Duration::from_secs(1),
///     }
/// );
///
/// // Add strategy for specific error
/// policy.add_strategy(
///     ErrorCode::REQUEST_TIMEOUT,
///     RecoveryStrategy::RetryExponential {
///         attempts: 5,
///         initial_delay: Duration::from_millis(100),
///         max_delay: Duration::from_secs(10),
///         multiplier: 2.0,
///     }
/// );
///
/// // Get strategy for error code
/// let strategy = policy.get_strategy(&ErrorCode::REQUEST_TIMEOUT);
/// ```
#[derive(Debug, Clone)]
pub struct RecoveryPolicy {
    /// Recovery strategy for each error code.
    strategies: HashMap<ErrorCode, RecoveryStrategy>,

    /// Default strategy if no specific one is defined.
    default_strategy: RecoveryStrategy,

    /// Whether to log recovery attempts.
    log_attempts: bool,
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        let mut strategies = HashMap::new();

        // Network errors get exponential backoff
        strategies.insert(
            ErrorCode::INTERNAL_ERROR,
            RecoveryStrategy::RetryExponential {
                attempts: 3,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(5),
                multiplier: 2.0,
            },
        );

        // Request errors get fixed retry
        strategies.insert(
            ErrorCode::INVALID_REQUEST,
            RecoveryStrategy::RetryFixed {
                attempts: 2,
                delay: Duration::from_millis(500),
            },
        );

        Self {
            strategies,
            default_strategy: RecoveryStrategy::FailFast,
            log_attempts: true,
        }
    }
}

impl RecoveryPolicy {
    /// Create a new recovery policy.
    pub fn new(default_strategy: RecoveryStrategy) -> Self {
        Self {
            strategies: HashMap::new(),
            default_strategy,
            log_attempts: true,
        }
    }

    /// Add a strategy for a specific error code.
    pub fn add_strategy(&mut self, error_code: ErrorCode, strategy: RecoveryStrategy) {
        self.strategies.insert(error_code, strategy);
    }

    /// Get strategy for an error code.
    pub fn get_strategy(&self, error_code: &ErrorCode) -> &RecoveryStrategy {
        self.strategies
            .get(error_code)
            .unwrap_or(&self.default_strategy)
    }
}

/// Error recovery handler trait.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait RecoveryHandler: Send + Sync {
    /// Handle error recovery.
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value>;
}

/// Strategy for recovering from a transport or protocol error.
///
/// wasm32 mirror of the native trait; `?Send` because wasm futures are not
/// `Send`.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait RecoveryHandler {
    /// Handle error recovery.
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value>;
}

/// Default recovery handler.
#[derive(Debug)]
pub struct DefaultRecoveryHandler;

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl RecoveryHandler for DefaultRecoveryHandler {
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value> {
        Err(Error::internal(error_msg))
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl RecoveryHandler for DefaultRecoveryHandler {
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value> {
        Err(Error::internal(error_msg))
    }
}

/// Fallback recovery handler.
pub struct FallbackHandler<F> {
    fallback: F,
}

impl<F> std::fmt::Debug for FallbackHandler<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackHandler")
            .field("fallback", &"<function>")
            .finish()
    }
}

impl<F> FallbackHandler<F> {
    /// Create a new fallback handler.
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<F, Fut> RecoveryHandler for FallbackHandler<F>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<serde_json::Value>> + Send,
{
    async fn recover(&self, _error_msg: &str) -> Result<serde_json::Value> {
        (self.fallback)().await
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl<F, Fut> RecoveryHandler for FallbackHandler<F>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<serde_json::Value>>,
{
    async fn recover(&self, _error_msg: &str) -> Result<serde_json::Value> {
        (self.fallback)().await
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for error recovery.
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<std::time::Instant>>>,
    config: CircuitBreakerConfig,
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("state", &"Arc<RwLock<CircuitState>>")
            .field("failure_count", &"Arc<RwLock<u32>>")
            .field("success_count", &"Arc<RwLock<u32>>")
            .field("last_failure_time", &"Arc<RwLock<Option<Instant>>>")
            .field("config", &self.config)
            .finish()
    }
}

/// Circuit breaker configuration.
///
/// # Examples
///
/// ```rust
/// use pmcp::error::recovery::CircuitBreakerConfig;
/// use std::time::Duration;
///
/// let config = CircuitBreakerConfig {
///     failure_threshold: 5,
///     success_threshold: 3,
///     timeout: Duration::from_mins(1),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit.
    pub failure_threshold: u32,

    /// Number of successes before closing circuit.
    pub success_threshold: u32,

    /// Timeout before attempting half-open state.
    pub timeout: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::error::recovery::{CircuitBreaker, CircuitBreakerConfig};
    /// use std::time::Duration;
    ///
    /// let config = CircuitBreakerConfig {
    ///     failure_threshold: 3,
    ///     success_threshold: 2,
    ///     timeout: Duration::from_secs(30),
    /// };
    ///
    /// let circuit_breaker = CircuitBreaker::new(config);
    /// ```
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Check if the circuit allows requests.
    ///
    /// Returns `true` if requests are allowed, `false` if the circuit is open.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::error::recovery::{CircuitBreaker, CircuitBreakerConfig};
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let config = CircuitBreakerConfig {
    ///     failure_threshold: 3,
    ///     success_threshold: 2,
    ///     timeout: Duration::from_secs(30),
    /// };
    ///
    /// let circuit_breaker = CircuitBreaker::new(config);
    /// let can_proceed = circuit_breaker.allow_request().await;
    /// # }
    /// ```
    pub async fn allow_request(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_failure_opt = *self.last_failure_time.read().await;
                if let Some(last_failure) = last_failure_opt {
                    if last_failure.elapsed() >= self.config.timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.success_count.write().await = 0;
                        info!("Circuit breaker transitioning to half-open");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a success.
    pub async fn record_success(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                *self.failure_count.write().await = 0;
            },
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                if *success_count >= self.config.success_threshold {
                    *self.state.write().await = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    info!("Circuit breaker closed after successful recovery");
                }
            },
            CircuitState::Open => {
                // Shouldn't happen, but reset anyway
                *self.state.write().await = CircuitState::Closed;
                *self.failure_count.write().await = 0;
            },
        }
    }

    /// Record a failure.
    pub async fn record_failure(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                if *failure_count >= self.config.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                    *self.last_failure_time.write().await = Some(std::time::Instant::now());
                    warn!("Circuit breaker opened after {} failures", *failure_count);
                }
            },
            CircuitState::HalfOpen => {
                *self.state.write().await = CircuitState::Open;
                *self.last_failure_time.write().await = Some(std::time::Instant::now());
                *self.failure_count.write().await = 1;
                warn!("Circuit breaker reopened after failure in half-open state");
            },
            CircuitState::Open => {
                // Already open, update last failure time
                *self.last_failure_time.write().await = Some(std::time::Instant::now());
            },
        }
    }
}

/// Health monitor trait for component health checking.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait HealthMonitor: Send + Sync {
    /// Check the health of a component.
    async fn check_health(&self, component: &str) -> HealthCheckResult;

    /// Get the current health status of a component.
    async fn get_health_status(&self, component: &str) -> HealthStatus;

    /// Subscribe to health change events.
    async fn subscribe_health_changes(
        &self,
    ) -> Result<Box<dyn Future<Output = RecoveryEvent> + Send + Unpin>>;
}

/// Reports component health to the recovery machinery.
///
/// wasm32 mirror of the native trait; `?Send` because wasm futures are not
/// `Send`.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait HealthMonitor {
    /// Check the health of a component.
    async fn check_health(&self, component: &str) -> HealthCheckResult;

    /// Get the current health status of a component.
    async fn get_health_status(&self, component: &str) -> HealthStatus;

    /// Subscribe to health change events.
    async fn subscribe_health_changes(
        &self,
    ) -> Result<Box<dyn Future<Output = RecoveryEvent> + Unpin>>;
}

/// Type alias for event handlers to reduce complexity.
#[cfg(not(target_arch = "wasm32"))]
type EventHandlers = Arc<RwLock<Vec<Arc<dyn Fn(RecoveryEvent) + Send + Sync>>>>;

#[cfg(target_arch = "wasm32")]
type EventHandlers = Arc<RwLock<Vec<Arc<dyn Fn(RecoveryEvent)>>>>;

/// Advanced recovery coordinator for managing complex recovery scenarios.
pub struct RecoveryCoordinator {
    /// Health monitor for component monitoring
    health_monitor: Option<Arc<dyn HealthMonitor>>,
    /// Component dependency graph for cascade detection
    dependencies: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Recovery metrics
    metrics: Arc<RecoveryMetrics>,
    /// Event handlers for recovery coordination
    event_handlers: EventHandlers,
}

impl std::fmt::Debug for RecoveryCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryCoordinator")
            .field("health_monitor", &self.health_monitor.is_some())
            .field("dependencies", &"Arc<RwLock<HashMap<...>>>")
            .field("metrics", &self.metrics)
            .field("event_handlers", &"Arc<RwLock<Vec<...>>>")
            .finish()
    }
}

impl RecoveryCoordinator {
    /// Create a new recovery coordinator.
    pub fn new() -> Self {
        Self {
            health_monitor: None,
            dependencies: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RecoveryMetrics::new()),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set health monitor for component monitoring.
    pub fn with_health_monitor(mut self, monitor: Arc<dyn HealthMonitor>) -> Self {
        self.health_monitor = Some(monitor);
        self
    }

    /// Add component dependency for cascade detection.
    pub async fn add_dependency(&self, component: String, dependencies: Vec<String>) {
        self.dependencies
            .write()
            .await
            .insert(component, dependencies);
    }

    /// Add event handler for recovery coordination.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add_event_handler(&self, handler: Arc<dyn Fn(RecoveryEvent) + Send + Sync>) {
        self.event_handlers.write().await.push(handler);
    }

    /// Register a callback invoked for every [`RecoveryEvent`].
    ///
    /// wasm32 counterpart of the native method: the handler is not required to
    /// be `Send`, since wasm32 runs single-threaded.
    #[cfg(target_arch = "wasm32")]
    pub async fn add_event_handler(&self, handler: Arc<dyn Fn(RecoveryEvent)>) {
        self.event_handlers.write().await.push(handler);
    }

    /// Emit a recovery event to all handlers.
    pub async fn emit_event(&self, event: RecoveryEvent) {
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            handler(event.clone());
        }
    }

    /// Detect cascading failures based on component dependencies.
    pub async fn detect_cascade(&self, failed_component: &str) -> Vec<String> {
        let mut affected = Vec::new();
        let dependencies = self.dependencies.read().await;

        // Find components that depend on the failed component
        for (component, deps) in dependencies.iter() {
            if deps.contains(&failed_component.to_string()) {
                affected.push(component.clone());
            }
        }

        if !affected.is_empty() {
            self.emit_event(RecoveryEvent::CascadingFailure {
                trigger_component: failed_component.to_string(),
                affected_components: affected.clone(),
            })
            .await;

            self.metrics.record_cascade_prevention();
        }

        affected
    }

    /// Get recovery metrics.
    pub fn get_metrics(&self) -> Arc<RecoveryMetrics> {
        self.metrics.clone()
    }
}

impl Default for RecoveryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Advanced bulk operation recovery handler.
pub struct BulkRecoveryHandler {
    /// Recovery coordinator
    coordinator: Arc<RecoveryCoordinator>,
    /// Individual operation timeout
    operation_timeout: Duration,
}

impl std::fmt::Debug for BulkRecoveryHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BulkRecoveryHandler")
            .field("coordinator", &self.coordinator)
            .field("operation_timeout", &self.operation_timeout)
            .finish()
    }
}

impl BulkRecoveryHandler {
    /// Create a new bulk recovery handler.
    pub fn new(coordinator: Arc<RecoveryCoordinator>, operation_timeout: Duration) -> Self {
        Self {
            coordinator,
            operation_timeout,
        }
    }

    /// Execute bulk operations with recovery.
    pub async fn execute_bulk<F, Fut, T>(
        &self,
        operations: Vec<F>,
        min_success_rate: f64,
        fail_fast: bool,
    ) -> BulkRecoveryResult
    where
        F: Fn() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
        T: serde::Serialize + Send,
    {
        let total_operations = operations.len();
        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for (index, operation) in operations.into_iter().enumerate() {
            let start_time = Instant::now();

            match operation().await {
                Ok(result) => {
                    let value = serde_json::to_value(result).unwrap_or(serde_json::Value::Null);
                    successes.push((index, value));

                    self.coordinator
                        .metrics
                        .record_success(start_time.elapsed());
                },
                Err(error) => {
                    failures.push((index, error));
                    self.coordinator
                        .metrics
                        .record_failure(start_time.elapsed());

                    if fail_fast {
                        break;
                    }
                },
            }
        }

        let success_rate = successes.len() as f64 / total_operations as f64;

        if successes.is_empty() {
            BulkRecoveryResult::AllFailed(failures.into_iter().map(|(_, e)| e).collect())
        } else if failures.is_empty() {
            BulkRecoveryResult::AllSuccess(successes.into_iter().map(|(_, v)| v).collect())
        } else if success_rate >= min_success_rate {
            BulkRecoveryResult::PartialSuccess {
                successes,
                failures,
            }
        } else {
            BulkRecoveryResult::AllFailed(failures.into_iter().map(|(_, e)| e).collect())
        }
    }
}

/// Advanced jitter implementation for retry delays.
#[derive(Debug)]
pub struct JitterCalculator;

impl JitterCalculator {
    /// Calculate jittered delay based on strategy.
    #[allow(clippy::cast_sign_loss, clippy::too_many_arguments)]
    pub fn calculate_delay(base_delay: Duration, strategy: JitterStrategy) -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let base_millis = base_delay.as_millis() as f64;

        match strategy {
            JitterStrategy::None => base_delay,
            JitterStrategy::Full => {
                // Random delay between 0 and base_delay
                let mut hasher = DefaultHasher::new();
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
                    .hash(&mut hasher);
                let random = (hasher.finish() % 1000) as f64 / 1000.0;
                Duration::from_millis((base_millis * random).max(0.0) as u64)
            },
            JitterStrategy::Equal => {
                // Half base delay + random half
                let mut hasher = DefaultHasher::new();
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
                    .hash(&mut hasher);
                let random = (hasher.finish() % 1000) as f64 / 1000.0;
                let jittered_millis = base_millis.mul_add(0.5, base_millis * 0.5 * random);
                Duration::from_millis(jittered_millis.max(0.0) as u64)
            },
            JitterStrategy::Decorrelated => {
                // Randomly distributed around base delay (±25%)
                let mut hasher = DefaultHasher::new();
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
                    .hash(&mut hasher);
                let random = ((hasher.finish() % 1000) as f64 / 1000.0 - 0.5) * 0.5; // -0.25 to +0.25
                let jittered_millis = base_millis * (1.0 + random);
                Duration::from_millis(jittered_millis.max(0.0) as u64)
            },
        }
    }
}

/// Enhanced error recovery executor with advanced recovery strategies.
pub struct AdvancedRecoveryExecutor {
    policy: RecoveryPolicy,
    handlers: HashMap<String, Arc<dyn RecoveryHandler>>,
    #[allow(dead_code)]
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    coordinator: Arc<RecoveryCoordinator>,
    bulk_handler: Arc<BulkRecoveryHandler>,
}

impl std::fmt::Debug for AdvancedRecoveryExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdvancedRecoveryExecutor")
            .field("policy", &self.policy)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .field("circuit_breakers", &"Arc<RwLock<HashMap<...>>>")
            .field("coordinator", &self.coordinator)
            .field("bulk_handler", &self.bulk_handler)
            .finish()
    }
}

impl AdvancedRecoveryExecutor {
    /// Create a new advanced recovery executor.
    pub fn new(policy: RecoveryPolicy) -> Self {
        let coordinator = Arc::new(RecoveryCoordinator::new());
        let bulk_handler = Arc::new(BulkRecoveryHandler::new(
            coordinator.clone(),
            Duration::from_secs(30),
        ));

        Self {
            policy,
            handlers: HashMap::new(),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            coordinator,
            bulk_handler,
        }
    }

    /// Get recovery coordinator.
    pub fn coordinator(&self) -> Arc<RecoveryCoordinator> {
        self.coordinator.clone()
    }

    /// Get bulk recovery handler.
    pub fn bulk_handler(&self) -> Arc<BulkRecoveryHandler> {
        self.bulk_handler.clone()
    }

    /// Execute with adaptive retry including jitter.
    #[allow(clippy::too_many_arguments)]
    pub async fn retry_adaptive<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        jitter: JitterStrategy,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;
        let mut current_delay = initial_delay;

        for attempt in 1..=attempts {
            let jittered_delay = JitterCalculator::calculate_delay(current_delay, jitter);

            if self.policy.log_attempts {
                debug!(
                    "Adaptive retry attempt {} of {} after {:?} (jitter: {:?})",
                    attempt, attempts, jittered_delay, jitter
                );
            }

            runtime::sleep(jittered_delay).await;

            match operation().await {
                Ok(result) => {
                    self.coordinator.metrics.record_success(jittered_delay);
                    return Ok(result);
                },
                Err(e) => {
                    last_error = e;
                    self.coordinator.metrics.record_failure(jittered_delay);

                    if self.policy.log_attempts {
                        warn!("Adaptive retry attempt {} failed: {}", attempt, last_error);
                    }

                    // Calculate next delay with exponential backoff
                    let next_delay = Duration::from_secs_f64(
                        (current_delay.as_secs_f64() * multiplier).min(max_delay.as_secs_f64()),
                    );
                    current_delay = next_delay;
                },
            }
        }

        Err(last_error)
    }

    /// Execute with deadline-aware recovery.
    pub async fn execute_with_deadline<F, Fut>(
        &self,
        operation_id: &str,
        operation: F,
        deadline: &mut RecoveryDeadline,
        base_strategy: &RecoveryStrategy,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        if deadline.update() {
            return Err(Error::Timeout(0));
        }

        self.coordinator
            .emit_event(RecoveryEvent::RecoveryStarted {
                operation_id: operation_id.to_string(),
                strategy: "deadline_aware".to_string(),
            })
            .await;

        let start_time = Instant::now();

        // Use runtime timeout to enforce the deadline
        #[cfg(not(target_arch = "wasm32"))]
        let timeout_result = tokio::time::timeout(deadline.remaining, operation()).await;

        #[cfg(target_arch = "wasm32")]
        let timeout_result: std::result::Result<Result<serde_json::Value>, ()> = {
            // In WASM, we don't have tokio::time::timeout, so we use a simpler approach
            // This is a simplified version - in production, you'd want more sophisticated timeout handling
            let res: Result<serde_json::Value> = operation().await;
            match res {
                Ok(value) => Ok(Ok(value)),
                Err(e) => Ok(Err(e)),
            }
        };

        let result = match timeout_result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(error),
            Err(_) => {
                // Timeout occurred
                deadline.update();
                return Err(Error::Timeout(deadline.remaining.as_millis() as u64));
            },
        };

        let result = match result {
            Ok(value) => {
                let duration = start_time.elapsed();
                self.coordinator
                    .emit_event(RecoveryEvent::RecoveryCompleted {
                        operation_id: operation_id.to_string(),
                        success: true,
                        duration,
                    })
                    .await;
                Ok(value)
            },
            Err(error) => {
                // Check if we have enough time for recovery
                deadline.update();
                if !deadline.has_time_for(Duration::from_millis(100)) {
                    let duration = start_time.elapsed();
                    self.coordinator
                        .emit_event(RecoveryEvent::RecoveryCompleted {
                            operation_id: operation_id.to_string(),
                            success: false,
                            duration,
                        })
                        .await;
                    return Err(Error::Timeout(duration.as_millis() as u64));
                }

                // Execute base recovery strategy with remaining time
                match base_strategy {
                    RecoveryStrategy::RetryFixed { attempts, delay } => {
                        let max_attempts = (deadline.remaining.as_millis() / delay.as_millis())
                            .min(*attempts as u128)
                            as u32;
                        self.retry_fixed(error, max_attempts, *delay, operation.clone())
                            .await
                    },
                    _ => Err(error), // Simplified for now
                }
            },
        };

        result
    }

    /// Register a recovery handler.
    pub fn register_handler(&mut self, name: String, handler: Arc<dyn RecoveryHandler>) {
        self.handlers.insert(name, handler);
    }

    /// Re-implement `retry_fixed` for compatibility.
    async fn retry_fixed<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        delay: Duration,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;

        for attempt in 1..=attempts {
            if self.policy.log_attempts {
                debug!(
                    "Retry attempt {} of {} after {:?}",
                    attempt, attempts, delay
                );
            }

            runtime::sleep(delay).await;

            match operation().await {
                Ok(result) => {
                    self.coordinator.metrics.record_success(delay);
                    return Ok(result);
                },
                Err(e) => {
                    last_error = e;
                    self.coordinator.metrics.record_failure(delay);

                    if self.policy.log_attempts {
                        warn!("Retry attempt {} failed: {}", attempt, last_error);
                    }
                },
            }
        }

        Err(last_error)
    }
}

/// Legacy error recovery executor for backward compatibility.
pub struct RecoveryExecutor {
    policy: RecoveryPolicy,
    handlers: HashMap<String, Arc<dyn RecoveryHandler>>,
    #[allow(dead_code)]
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
}

impl std::fmt::Debug for RecoveryExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryExecutor")
            .field("policy", &self.policy)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .field("circuit_breakers", &"Arc<RwLock<HashMap<...>>>")
            .finish()
    }
}

impl RecoveryExecutor {
    /// Create a new recovery executor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::error::recovery::{RecoveryExecutor, RecoveryPolicy};
    ///
    /// let policy = RecoveryPolicy::default();
    /// let executor = RecoveryExecutor::new(policy);
    /// ```
    pub fn new(policy: RecoveryPolicy) -> Self {
        Self {
            policy,
            handlers: HashMap::new(),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a recovery handler.
    pub fn register_handler(&mut self, name: String, handler: Arc<dyn RecoveryHandler>) {
        self.handlers.insert(name, handler);
    }

    /// Execute with recovery.
    pub async fn execute_with_recovery<F, Fut>(
        &self,
        operation_id: &str,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        match operation().await {
            Ok(result) => {
                // Record success if using circuit breaker
                if let Some(cb) = self.circuit_breakers.read().await.get(operation_id) {
                    cb.record_success().await;
                }
                Ok(result)
            },
            Err(error) => {
                let error_code = error.error_code().unwrap_or(ErrorCode::INTERNAL_ERROR);
                let strategy = self.policy.get_strategy(&error_code);

                match strategy {
                    RecoveryStrategy::RetryFixed { attempts, delay } => {
                        self.retry_fixed(error, *attempts, *delay, operation).await
                    },
                    RecoveryStrategy::RetryExponential {
                        attempts,
                        initial_delay,
                        max_delay,
                        multiplier,
                    } => {
                        self.retry_exponential(
                            error,
                            *attempts,
                            *initial_delay,
                            *max_delay,
                            *multiplier,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::Fallback => {
                        self.fallback(&error.to_string(), operation_id).await
                    },
                    RecoveryStrategy::CircuitBreaker {
                        failure_threshold,
                        success_threshold,
                        timeout,
                    } => {
                        self.circuit_breaker(
                            error,
                            operation_id,
                            *failure_threshold,
                            *success_threshold,
                            *timeout,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::FailFast => Err(error),
                    // Advanced strategies - simplified implementation for legacy executor
                    RecoveryStrategy::RetryAdaptive {
                        attempts,
                        initial_delay,
                        max_delay,
                        multiplier,
                        ..
                    } => {
                        // Use exponential backoff without jitter for legacy compatibility
                        self.retry_exponential(
                            error,
                            *attempts,
                            *initial_delay,
                            *max_delay,
                            *multiplier,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::AdvancedCircuitBreaker {
                        failure_threshold,
                        success_threshold,
                        timeout,
                        ..
                    } => {
                        // Use basic circuit breaker for legacy compatibility
                        self.circuit_breaker(
                            error,
                            operation_id,
                            *failure_threshold,
                            *success_threshold,
                            *timeout,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::DeadlineAware { base_strategy, .. } => {
                        // Extract base strategy and use that (simplified)
                        match base_strategy.as_ref() {
                            RecoveryStrategy::RetryFixed { attempts, delay } => {
                                self.retry_fixed(error, *attempts, *delay, operation).await
                            },
                            _ => Err(error), // Simplified for legacy compatibility
                        }
                    },
                    RecoveryStrategy::BulkRecovery { .. } => {
                        // Not supported in legacy executor
                        Err(error)
                    },
                    RecoveryStrategy::CascadeAware { base_strategy, .. } => {
                        // Extract base strategy and use that (simplified)
                        match base_strategy.as_ref() {
                            RecoveryStrategy::RetryFixed { attempts, delay } => {
                                self.retry_fixed(error, *attempts, *delay, operation).await
                            },
                            _ => Err(error), // Simplified for legacy compatibility
                        }
                    },
                }
            },
        }
    }

    async fn retry_fixed<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        delay: Duration,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;

        for attempt in 1..=attempts {
            if self.policy.log_attempts {
                debug!(
                    "Retry attempt {} of {} after {:?}",
                    attempt, attempts, delay
                );
            }

            runtime::sleep(delay).await;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if self.policy.log_attempts {
                        warn!("Retry attempt {} failed: {}", attempt, last_error);
                    }
                },
            }
        }

        Err(last_error)
    }

    async fn retry_exponential<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;
        let mut current_delay = initial_delay;

        for attempt in 1..=attempts {
            if self.policy.log_attempts {
                debug!(
                    "Exponential retry attempt {} of {} after {:?}",
                    attempt, attempts, current_delay
                );
            }

            runtime::sleep(current_delay).await;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if self.policy.log_attempts {
                        warn!(
                            "Exponential retry attempt {} failed: {}",
                            attempt, last_error
                        );
                    }

                    // Calculate next delay
                    let next_delay = Duration::from_secs_f64(
                        (current_delay.as_secs_f64() * multiplier).min(max_delay.as_secs_f64()),
                    );
                    current_delay = next_delay;
                },
            }
        }

        Err(last_error)
    }

    async fn fallback(&self, error_msg: &str, operation_id: &str) -> Result<serde_json::Value> {
        if let Some(handler) = self.handlers.get(operation_id) {
            if self.policy.log_attempts {
                info!("Using fallback handler for operation: {}", operation_id);
            }
            handler.recover(error_msg).await
        } else {
            if self.policy.log_attempts {
                error!(
                    "No fallback handler registered for operation: {}",
                    operation_id
                );
            }
            Err(Error::internal(error_msg))
        }
    }

    async fn circuit_breaker<F, Fut>(
        &self,
        _error: Error,
        operation_id: &str,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        // Get or create circuit breaker
        let cb = {
            let mut breakers = self.circuit_breakers.write().await;
            breakers
                .entry(operation_id.to_string())
                .or_insert_with(|| {
                    Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
                        failure_threshold,
                        success_threshold,
                        timeout,
                    }))
                })
                .clone()
        };

        if !cb.allow_request().await {
            if self.policy.log_attempts {
                warn!("Circuit breaker open for operation: {}", operation_id);
            }
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "Circuit breaker is open",
            ));
        }

        match operation().await {
            Ok(result) => {
                cb.record_success().await;
                Ok(result)
            },
            Err(e) => {
                cb.record_failure().await;
                Err(e)
            },
        }
    }
}

/// Helper function to create a retry handler.
///
/// Retries an operation with fixed delay between attempts.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::error::recovery::with_retry;
/// use std::time::Duration;
/// use serde_json::json;
///
/// # async fn example() -> pmcp::Result<()> {
/// let result = with_retry(3, Duration::from_millis(500), || {
///     async {
///         // Simulated operation that might fail
///         Ok(json!({"success": true}))
///     }
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn with_retry<F, Fut>(
    attempts: u32,
    delay: Duration,
    operation: F,
) -> Result<serde_json::Value>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<serde_json::Value>>,
{
    let mut last_error = None;

    for attempt in 0..attempts {
        if attempt > 0 {
            runtime::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
            },
        }
    }

    Err(last_error.unwrap_or_else(|| Error::internal("No attempts made")))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_fixed() {
        let policy = RecoveryPolicy::default();
        let executor = RecoveryExecutor::new(policy);

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = executor
            .retry_fixed(
                Error::internal("test"),
                3,
                Duration::from_millis(10),
                || {
                    let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed);
                    async move {
                        if count < 2 {
                            Err(Error::internal("retry"))
                        } else {
                            Ok(serde_json::json!({"success": true}))
                        }
                    }
                },
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
        };

        let cb = CircuitBreaker::new(config);

        // Initially closed
        assert!(cb.allow_request().await);

        // Record failures to open circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Should be open now
        assert!(!cb.allow_request().await);

        // Wait for timeout
        runtime::sleep(Duration::from_millis(150)).await;

        // Should be half-open
        assert!(cb.allow_request().await);

        // Success should start closing
        cb.record_success().await;
        cb.record_success().await;

        // Should be closed again
        assert!(cb.allow_request().await);
    }

    #[tokio::test]
    async fn test_with_retry() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = with_retry(3, Duration::from_millis(10), || {
            let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed);
            async move {
                if count < 2 {
                    Err(Error::internal("retry"))
                } else {
                    Ok(serde_json::json!({"success": true}))
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
    }
}
