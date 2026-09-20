//! Connection pooling and load balancing for MCP transports.
//!
//! Note: Connection pooling is not available in WASM environments
//! as they are single-threaded and typically use stateless connections.

#![cfg(not(target_arch = "wasm32"))]
//!
//! PMCP-4003: Advanced connection management with:
//! - Connection pooling for multiple transport types
//! - Load balancing strategies (round-robin, least connections, weighted)
//! - Health checking and automatic failover
//! - Connection lifecycle management

use crate::error::{Error, Result};
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unique identifier for a connection in the pool
pub type ConnectionId = Uuid;

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    /// Round-robin selection
    RoundRobin,
    /// Least connections first
    LeastConnections,
    /// Weighted round-robin based on capacity
    WeightedRoundRobin,
    /// Random selection
    Random,
}

/// Connection health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Connection is healthy and available
    Healthy,
    /// Connection is degraded but still usable
    Degraded,
    /// Connection is unhealthy and should not be used
    Unhealthy,
    /// Connection is being checked
    Checking,
}

/// Configuration for connection pool
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections in the pool
    pub max_connections: usize,
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Load balancing strategy
    pub strategy: LoadBalanceStrategy,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Connection timeout for operations
    pub operation_timeout: Duration,
    /// Maximum idle time before connection cleanup
    pub max_idle_time: Duration,
    /// Enable automatic connection scaling
    pub auto_scaling: bool,
    /// Connection retry attempts
    pub max_retries: usize,
    /// Retry delay
    pub retry_delay: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 2,
            strategy: LoadBalanceStrategy::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            operation_timeout: Duration::from_secs(10),
            max_idle_time: Duration::from_mins(5),
            auto_scaling: true,
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }
}

/// Connection metadata and statistics
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Unique connection ID
    pub id: ConnectionId,
    /// Current health status
    pub health: HealthStatus,
    /// Number of active requests
    pub active_requests: usize,
    /// Total requests handled
    pub total_requests: u64,
    /// Connection weight for load balancing
    pub weight: f64,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Connection establishment time
    pub created_at: Instant,
    /// Current latency (moving average)
    pub avg_latency: Duration,
    /// Error count in recent window
    pub recent_errors: usize,
}

/// Pooled connection wrapper
struct PooledConnection<T: Transport> {
    info: ConnectionInfo,
    transport: Box<T>,
    send_tx: mpsc::Sender<TransportMessage>,
    recv_rx: Arc<RwLock<mpsc::Receiver<TransportMessage>>>,
}

impl<T: Transport> std::fmt::Debug for PooledConnection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConnection")
            .field("info", &self.info)
            .finish()
    }
}

/// Connection pool with load balancing and health checking
pub struct ConnectionPool<T: Transport> {
    config: ConnectionPoolConfig,
    connections: Arc<RwLock<HashMap<ConnectionId, PooledConnection<T>>>>,
    round_robin_index: Arc<RwLock<usize>>,
    health_checker: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

impl<T: Transport + Clone + Send + Sync + 'static> std::fmt::Debug for ConnectionPool<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionPool")
            .field("config", &self.config)
            .finish()
    }
}

impl<T: Transport + Clone + Send + Sync + 'static> ConnectionPool<T> {
    /// Create a new connection pool
    pub fn new(config: ConnectionPoolConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            round_robin_index: Arc::new(RwLock::new(0)),
            health_checker: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the connection pool and initialize minimum connections
    pub async fn start<F>(&mut self, connection_factory: F) -> Result<()>
    where
        F: Fn() -> Result<T> + Send + Sync + 'static,
    {
        info!(
            "Starting connection pool with {} min connections",
            self.config.min_connections
        );

        // Create initial connections
        for _ in 0..self.config.min_connections {
            match connection_factory() {
                Ok(transport) => {
                    if let Err(e) = self.add_connection(transport).await {
                        warn!("Failed to add initial connection: {}", e);
                    }
                },
                Err(e) => {
                    warn!("Failed to create initial connection: {}", e);
                },
            }
        }

        // Start health checker
        self.start_health_checker().await;

        Ok(())
    }

    /// Add a new connection to the pool
    pub async fn add_connection(&self, transport: T) -> Result<ConnectionId> {
        let mut connections = self.connections.write().await;

        if connections.len() >= self.config.max_connections {
            return Err(Error::internal("Connection pool at maximum capacity"));
        }

        let id = Uuid::new_v4();
        let now = Instant::now();

        let (send_tx, mut send_rx) = mpsc::channel(100);
        let (recv_tx, recv_rx) = mpsc::channel(100);

        // Start message forwarding tasks
        let mut transport_send = transport.clone();
        tokio::spawn(async move {
            while let Some(msg) = send_rx.recv().await {
                if let Err(e) = transport_send.send(msg).await {
                    error!("Failed to send message through transport: {}", e);
                    break;
                }
            }
        });

        let mut transport_recv = transport.clone();
        tokio::spawn(async move {
            loop {
                match transport_recv.receive().await {
                    Ok(msg) => {
                        if let Err(e) = recv_tx.send(msg).await {
                            error!("Failed to queue received message: {}", e);
                            break;
                        }
                    },
                    Err(e) => {
                        error!("Transport receive error: {}", e);
                        break;
                    },
                }
            }
        });

        let info = ConnectionInfo {
            id,
            health: HealthStatus::Healthy,
            active_requests: 0,
            total_requests: 0,
            weight: 1.0,
            last_activity: now,
            created_at: now,
            avg_latency: Duration::from_millis(10),
            recent_errors: 0,
        };

        let pooled = PooledConnection {
            info,
            transport: Box::new(transport),
            send_tx,
            recv_rx: Arc::new(RwLock::new(recv_rx)),
        };

        connections.insert(id, pooled);
        info!("Added connection {} to pool", id);

        Ok(id)
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, id: ConnectionId) -> Result<()> {
        let mut connections = self.connections.write().await;

        if let Some(mut conn) = connections.remove(&id) {
            info!("Removing connection {} from pool", id);
            let _ = conn.transport.close().await;
            Ok(())
        } else {
            Err(Error::internal("Connection not found in pool"))
        }
    }

    /// Get the best connection based on load balancing strategy
    pub async fn get_connection(&self) -> Result<ConnectionId> {
        let connections = self.connections.read().await;

        if connections.is_empty() {
            return Err(Error::internal("No connections available in pool"));
        }

        let healthy_connections: Vec<&PooledConnection<T>> = connections
            .values()
            .filter(|conn| conn.info.health == HealthStatus::Healthy)
            .collect();

        if healthy_connections.is_empty() {
            // Fall back to degraded connections if no healthy ones
            let degraded: Vec<&PooledConnection<T>> = connections
                .values()
                .filter(|conn| conn.info.health == HealthStatus::Degraded)
                .collect();

            if degraded.is_empty() {
                return Err(Error::internal("No healthy connections available"));
            }

            warn!("No healthy connections, using degraded connection");
            return Ok(degraded[0].info.id);
        }

        let selected_id = match self.config.strategy {
            LoadBalanceStrategy::RoundRobin => self.select_round_robin(&healthy_connections).await,
            LoadBalanceStrategy::LeastConnections => {
                Self::select_least_connections(self, &healthy_connections)
            },
            LoadBalanceStrategy::WeightedRoundRobin => {
                Self::select_weighted_round_robin(self, &healthy_connections)
            },
            LoadBalanceStrategy::Random => Self::select_random(self, &healthy_connections),
        };

        Ok(selected_id)
    }

    /// Send message through the pool (automatically selects best connection)
    pub async fn send_message(&self, message: TransportMessage) -> Result<()> {
        let connection_id = self.get_connection().await?;
        self.send_to_connection(connection_id, message).await
    }

    /// Send message to specific connection
    pub async fn send_to_connection(
        &self,
        connection_id: ConnectionId,
        message: TransportMessage,
    ) -> Result<()> {
        let mut connections = self.connections.write().await;

        let conn = connections
            .get_mut(&connection_id)
            .ok_or_else(|| Error::internal("Connection not found"))?;

        let start_time = Instant::now();
        conn.info.active_requests += 1;
        conn.info.last_activity = start_time;

        let result = conn.send_tx.send(message).await;

        // Update statistics
        conn.info.active_requests = conn.info.active_requests.saturating_sub(1);
        conn.info.total_requests += 1;

        if result.is_err() {
            conn.info.recent_errors += 1;
            conn.info.health = if conn.info.recent_errors > 5 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            return Err(Error::internal("Failed to send message"));
        }

        // Update latency (simplified moving average)
        let latency = start_time.elapsed();
        #[allow(clippy::cast_sign_loss)]
        {
            conn.info.avg_latency = Duration::from_nanos(
                (conn.info.avg_latency.as_nanos() as f64)
                    .mul_add(0.9, latency.as_nanos() as f64 * 0.1) as u64,
            );
        }

        Ok(())
    }

    /// Receive message from any connection in the pool
    pub async fn receive_message(&self) -> Result<(ConnectionId, TransportMessage)> {
        let connections = self.connections.read().await;

        if connections.is_empty() {
            return Err(Error::internal("No connections in pool"));
        }

        // Try each connection's receiver in round-robin fashion
        let connection_ids: Vec<ConnectionId> = connections.keys().copied().collect();

        for connection_id in connection_ids {
            if let Some(conn) = connections.get(&connection_id) {
                let mut rx = conn.recv_rx.try_write();
                if let Ok(ref mut receiver) = rx {
                    if let Ok(msg) = receiver.try_recv() {
                        return Ok((connection_id, msg));
                    }
                }
            }
        }

        // If no immediate messages, wait on the first available
        if let Some((connection_id, conn)) = connections.iter().next() {
            let mut rx = conn.recv_rx.write().await;
            let msg = rx
                .recv()
                .await
                .ok_or_else(|| Error::internal("Connection closed"))?;
            Ok((*connection_id, msg))
        } else {
            Err(Error::internal("No connections available"))
        }
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        let connections = self.connections.read().await;

        let total_connections = connections.len();
        let healthy_count = connections
            .values()
            .filter(|c| c.info.health == HealthStatus::Healthy)
            .count();
        let degraded_count = connections
            .values()
            .filter(|c| c.info.health == HealthStatus::Degraded)
            .count();
        let unhealthy_count = connections
            .values()
            .filter(|c| c.info.health == HealthStatus::Unhealthy)
            .count();

        let total_requests: u64 = connections.values().map(|c| c.info.total_requests).sum();

        let active_requests: usize = connections.values().map(|c| c.info.active_requests).sum();

        PoolStats {
            total_connections,
            healthy_connections: healthy_count,
            degraded_connections: degraded_count,
            unhealthy_connections: unhealthy_count,
            total_requests,
            active_requests,
            strategy: self.config.strategy,
        }
    }

    /// Start health checking background task
    async fn start_health_checker(&self) {
        let (tx, mut rx) = mpsc::channel(1);
        *self.health_checker.write().await = Some(tx);

        let connections = self.connections.clone();
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        Self::perform_health_check(&connections).await;
                    }
                    _ = rx.recv() => {
                        info!("Health checker shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Perform health check on all connections
    async fn perform_health_check(
        connections: &Arc<RwLock<HashMap<ConnectionId, PooledConnection<T>>>>,
    ) {
        let mut connections_guard = connections.write().await;
        let now = Instant::now();

        for (id, conn) in connections_guard.iter_mut() {
            // Check if connection is responsive
            if conn.transport.is_connected() {
                // Reset error count for healthy connections
                if conn.info.recent_errors > 0 {
                    conn.info.recent_errors = conn.info.recent_errors.saturating_sub(1);
                }

                // Update health status based on recent errors
                conn.info.health = match conn.info.recent_errors {
                    0 => HealthStatus::Healthy,
                    1..=3 => HealthStatus::Degraded,
                    _ => HealthStatus::Unhealthy,
                };
            } else {
                conn.info.health = HealthStatus::Unhealthy;
                conn.info.recent_errors += 1;
            }

            debug!("Health check for {}: {:?}", id, conn.info.health);
        }

        // Remove connections that have been unhealthy for too long
        let unhealthy_ids: Vec<ConnectionId> = connections_guard
            .iter()
            .filter(|(_, conn)| {
                conn.info.health == HealthStatus::Unhealthy
                    && now.duration_since(conn.info.last_activity) > Duration::from_mins(5)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in unhealthy_ids {
            warn!("Removing persistently unhealthy connection: {}", id);
            if let Some(mut conn) = connections_guard.remove(&id) {
                let _ = conn.transport.close().await;
            }
        }
    }

    /// Round-robin selection
    async fn select_round_robin(&self, connections: &[&PooledConnection<T>]) -> ConnectionId {
        let mut index = self.round_robin_index.write().await;
        let selected = connections[*index % connections.len()];
        *index = (*index + 1) % connections.len();
        selected.info.id
    }

    /// Least connections selection
    fn select_least_connections(
        _self: &Self,
        connections: &[&PooledConnection<T>],
    ) -> ConnectionId {
        #![allow(clippy::unused_self)]
        connections
            .iter()
            .min_by_key(|conn| conn.info.active_requests)
            .unwrap()
            .info
            .id
    }

    /// Weighted round-robin selection
    fn select_weighted_round_robin(
        _self: &Self,
        connections: &[&PooledConnection<T>],
    ) -> ConnectionId {
        #![allow(clippy::unused_self)]
        // Simple weighted selection based on inverse of active requests
        let best = connections
            .iter()
            .min_by(|a, b| {
                let score_a = a.info.active_requests as f64 / a.info.weight;
                let score_b = b.info.active_requests as f64 / b.info.weight;
                score_a.partial_cmp(&score_b).unwrap()
            })
            .unwrap();

        best.info.id
    }

    /// Random selection
    fn select_random(_self: &Self, connections: &[&PooledConnection<T>]) -> ConnectionId {
        #![allow(clippy::unused_self)]
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        let index = (hasher.finish() as usize) % connections.len();

        connections[index].info.id
    }

    /// Shutdown the connection pool
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down connection pool");

        // Stop health checker
        let value = self.health_checker.write().await.take();
        if let Some(tx) = value {
            let _ = tx.send(()).await;
        }

        // Close all connections
        let mut connections = self.connections.write().await;
        for (id, mut conn) in connections.drain() {
            info!("Closing connection {}", id);
            let _ = conn.transport.close().await;
        }

        Ok(())
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of connections in the pool
    pub total_connections: usize,
    /// Number of healthy connections
    pub healthy_connections: usize,
    /// Number of degraded connections
    pub degraded_connections: usize,
    /// Number of unhealthy connections
    pub unhealthy_connections: usize,
    /// Total requests processed
    pub total_requests: u64,
    /// Currently active requests
    pub active_requests: usize,
    /// Current load balancing strategy
    pub strategy: LoadBalanceStrategy,
}

/// Pooled transport that implements the Transport trait
pub struct PooledTransport<T: Transport> {
    pool: Arc<ConnectionPool<T>>,
}

impl<T: Transport + Clone + Send + Sync + 'static> PooledTransport<T> {
    /// Create a new pooled transport
    pub fn new(pool: ConnectionPool<T>) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    /// Get reference to underlying pool for management
    pub fn pool(&self) -> &ConnectionPool<T> {
        &self.pool
    }
}

impl<T: Transport + Clone + Send + Sync + 'static> std::fmt::Debug for PooledTransport<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledTransport").finish()
    }
}

// Why: this is the ONE wrapping `impl Transport for ...` in the tree, so it is
// the one place where `Transport::shared_sender`'s default `None` could be
// SILENTLY inherited rather than decided (Phase 118.2, plan 23). The decision is
// to keep the default, and the reason is structural rather than incidental:
// `send` does not speak to any single inner transport. `ConnectionPool::
// send_message` load-balances a CONNECTION per send and then hands the message
// to that connection's mpsc channel; a per-connection worker task, owning its
// own clone of the inner transport, performs the actual I/O. There is therefore
// no inner transport whose handle would be correct for a future send, and
// forwarding one would silently pin every shared send to whichever connection
// happened to be chosen when the handle was taken — defeating the pool.
//
// NAMED RESIDUAL, with an owner in plan 24's ledger rather than left implicit: a
// consumer who pools a `StreamableHttpTransport` keeps the exclusive `&mut`
// path. That is materially narrower than the wedge plan 23 closes, because
// `send_to_connection` awaits an mpsc push rather than the peer's response
// HEAD — a stalled peer does not hold the consumer's transport guard through
// this wrapper. It is not ZERO: with the connection's channel full (capacity
// 100) the push blocks until the worker drains, and the worker is itself blocked
// on the stalled peer, so sustained traffic against a silent peer can still
// serialise a client over a pooled transport.
#[async_trait]
impl<T: Transport + Clone + Send + Sync + 'static> Transport for PooledTransport<T> {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.pool.send_message(message).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let (_connection_id, message) = self.pool.receive_message().await?;
        Ok(message)
    }

    async fn close(&mut self) -> Result<()> {
        // Note: This creates a mutable reference issue with Arc
        // In a real implementation, we'd need a different shutdown approach
        warn!("PooledTransport close called - pool remains active");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Pool is connected if we have at least one healthy connection
        futures::executor::block_on(async {
            let stats = self.pool.get_stats().await;
            stats.healthy_connections > 0 || stats.degraded_connections > 0
        })
    }

    fn transport_type(&self) -> &'static str {
        "pooled"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = ConnectionPoolConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.strategy, LoadBalanceStrategy::RoundRobin);
        assert!(config.auto_scaling);
    }

    #[test]
    fn test_load_balance_strategies() {
        assert_eq!(
            LoadBalanceStrategy::RoundRobin,
            LoadBalanceStrategy::RoundRobin
        );
        assert_ne!(
            LoadBalanceStrategy::RoundRobin,
            LoadBalanceStrategy::LeastConnections
        );
    }

    #[test]
    fn test_health_status() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Degraded);
    }
}
