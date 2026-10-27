// MCP protocol uses underscore-prefixed fields (_meta, _task_id) that are required by spec.
#![allow(clippy::used_underscore_binding)]

//! MCP Tasks support for the PMCP SDK (experimental).
//!
//! This crate implements the MCP 2025-11-25 Tasks specification, providing
//! spec-compliant wire types, state machine validation, error types, and
//! constants for managing long-running task lifecycles in the Model Context
//! Protocol.
//!
//! # Overview
//!
//! Tasks are coordination primitives for long-running activities between
//! client and server. A task progresses through a state machine
//! (`working` -> `completed`/`failed`/`cancelled`, with `input_required`
//! as an intermediate state) and supports TTL-based expiration, polling,
//! and status notifications.
//!
//! # Which server path this crate serves
//!
//! The stores here, including `DynamoDbBackend` and `RedisBackend`, implement
//! this crate's own [`store::TaskStore`] trait. They reach a pmcp server through
//! [`TaskRouterImpl`] and `with_task_store(...)`, the legacy experimental
//! router path. They do **not** implement `pmcp::server::task_store::TaskStore`,
//! the trait the recommended `task_store(...)` builder method takes. A server
//! on that path that needs task state to survive a restart, or to be shared
//! between instances, implements that trait over its own database.
//!
//! # Module Organization
//!
//! - [`types`] - Spec-compliant wire types (Task, params, capabilities, etc.)
//! - [`domain`] - Internal domain types (`TaskRecord`, `TaskWithVariables`)
//! - [`store`] - `TaskStore` async trait, supporting types, and backend implementations
//! - [`security`] - Security configuration and owner resolution
//! - [`error`] - Rich error types with JSON-RPC error code mapping
//! - [`constants`] - Meta key and method name constants

pub mod constants;
pub mod error;
pub mod types;

/// Domain types (TaskRecord, TaskWithVariables) for internal task representation.
pub mod domain;

/// Task store trait and supporting types (StoreConfig, ListTasksOptions, TaskPage).
pub mod store;

/// Security configuration and owner resolution.
pub mod security;

/// Ergonomic task context for tool handlers.
pub mod context;

/// TaskRouter implementation bridging pmcp's TaskRouter trait to TaskStore.
pub mod router;

// Re-exports for ergonomic access
pub use constants::*;
pub use context::TaskContext;
pub use domain::{TaskRecord, TaskWithVariables};
pub use error::TaskError;
pub use router::TaskRouterImpl;
pub use security::{resolve_owner_id, TaskSecurityConfig, DEFAULT_LOCAL_OWNER};
#[cfg(feature = "dynamodb")]
pub use store::dynamodb::DynamoDbBackend;
pub use store::generic::GenericTaskStore;
pub use store::memory::InMemoryBackend;
pub use store::memory::InMemoryTaskStore;
#[cfg(feature = "redis")]
pub use store::redis::RedisBackend;
pub use store::{
    ListTasksOptions, StorageBackend, StorageError, StoreConfig, TaskPage, TaskStore,
    VersionedRecord,
};
pub use types::*;
