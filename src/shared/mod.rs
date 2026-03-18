//! Shared components used by both client and server.

pub mod batch;
pub mod context;
pub mod event_store;
pub mod http_utils;
pub mod logging;
pub mod middleware;
pub mod middleware_presets;
pub mod protocol;
pub mod protocol_helpers;
#[cfg(not(target_arch = "wasm32"))]
pub mod reconnect;
pub mod session;
pub mod simd_parsing;
pub mod sse_parser;

#[cfg(feature = "sse")]
pub mod sse_optimized;

#[cfg(not(target_arch = "wasm32"))]
pub mod connection_pool;
#[cfg(not(target_arch = "wasm32"))]
pub mod stdio;
pub mod transport;
pub mod uri_template;

// Cross-platform runtime abstraction
pub mod runtime;

// Platform-specific WebSocket modules
#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub mod websocket;

#[cfg(all(feature = "websocket-wasm", target_arch = "wasm32"))]
pub mod wasm_websocket;

#[cfg(target_arch = "wasm32")]
pub mod wasm_http;

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub mod http;
pub mod http_constants;

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
/// Streamable HTTP transport implementation for MCP.
pub mod streamable_http;

// Re-export commonly used types
pub use batch::{BatchRequest, BatchResponse};
pub use context::{ClientInfo, ContextPropagator, RequestContext};
pub use event_store::{
    EventStore, EventStoreConfig, InMemoryEventStore, MessageDirection, ResumptionManager,
    ResumptionState, ResumptionToken, StoredEvent,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "logging"))]
pub use logging::init_logging;
pub use logging::{CorrelatedLogger, LogConfig, LogEntry, LogFormat, LogLevel};
pub use middleware::{
    AdvancedMiddleware, AuthMiddleware, CircuitBreakerMiddleware, CompressionMiddleware,
    CompressionType, EnhancedMiddlewareChain, LoggingMiddleware, MetricsMiddleware, Middleware,
    MiddlewareChain, MiddlewareContext, MiddlewarePriority, PerformanceMetrics,
    RateLimitMiddleware, RetryMiddleware,
};
pub use protocol::{ProgressCallback, Protocol, ProtocolOptions, RequestOptions};
pub use protocol_helpers::{
    create_notification, create_request, parse_notification, parse_request,
};
#[cfg(not(target_arch = "wasm32"))]
pub use reconnect::{ReconnectConfig, ReconnectGuard, ReconnectManager};
pub use session::{Session, SessionConfig, SessionManager};
#[cfg(not(target_arch = "wasm32"))]
pub use stdio::StdioTransport;
pub use transport::{Transport, TransportMessage};
pub use uri_template::UriTemplate;

#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub use websocket::{WebSocketConfig, WebSocketTransport};

#[cfg(all(feature = "websocket-wasm", target_arch = "wasm32"))]
pub use wasm_websocket::{WasmWebSocketConfig, WasmWebSocketTransport};

#[cfg(target_arch = "wasm32")]
pub use wasm_http::{WasmHttpClient, WasmHttpConfig, WasmHttpTransport};

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use http::{HttpConfig, HttpTransport};

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub use streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};

#[cfg(feature = "sse")]
pub use sse_optimized::{OptimizedSseConfig, OptimizedSseTransport};

#[cfg(not(target_arch = "wasm32"))]
pub use connection_pool::{
    ConnectionId, ConnectionPool, ConnectionPoolConfig, HealthStatus, LoadBalanceStrategy,
    PoolStats, PooledTransport,
};

pub use simd_parsing::{
    CpuFeatures, ParsingMetrics, SimdBase64, SimdHttpHeaderParser, SimdJsonParser, SimdSseParser,
};
