//! Core protocol types for MCP.
//!
//! This module contains all the type definitions for the Model Context Protocol,
//! including requests, responses, notifications, and capability definitions.

pub mod auth;
// Dead on wasm32 by CONFIGURATION, not by disuse: this module's consumers are the
// native server/client tier (`src/server/core.rs`, `src/server/task_dispatch.rs`,
// `src/client/mod.rs`), all of which are `#[cfg(not(target_arch = "wasm32"))]`. The
// items are `pub(crate)` and very much alive natively, so they must NOT be deleted;
// the wasm build simply has no callers for them. Scoped to wasm32 so genuine dead
// code is still caught on every other target.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod caching;
pub mod capabilities;
pub mod completable;
pub mod content;
pub mod elicitation;
pub mod jsonrpc;
// Dead on wasm32 by CONFIGURATION, not by disuse: this module's consumers are the
// native server/client tier (`src/server/core.rs`, `src/server/task_dispatch.rs`,
// `src/client/mod.rs`), all of which are `#[cfg(not(target_arch = "wasm32"))]`. The
// items are `pub(crate)` and very much alive natively, so they must NOT be deleted;
// the wasm build simply has no callers for them. Scoped to wasm32 so genuine dead
// code is still caught on every other target.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod mrtr;
pub mod notifications;
pub mod prompts;
// Dead on wasm32 by CONFIGURATION, not by disuse: this module's consumers are the
// native server/client tier (`src/server/core.rs`, `src/server/task_dispatch.rs`,
// `src/client/mod.rs`), all of which are `#[cfg(not(target_arch = "wasm32"))]`. The
// items are `pub(crate)` and very much alive natively, so they must NOT be deleted;
// the wasm build simply has no callers for them. Scoped to wasm32 so genuine dead
// code is still caught on every other target.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod protocol;
pub mod resources;
pub mod roots;
pub mod sampling;
// Dead on wasm32 by CONFIGURATION, not by disuse: this module's consumers are the
// native server/client tier (`src/server/core.rs`, `src/server/task_dispatch.rs`,
// `src/client/mod.rs`), all of which are `#[cfg(not(target_arch = "wasm32"))]`. The
// items are `pub(crate)` and very much alive natively, so they must NOT be deleted;
// the wasm build simply has no callers for them. Scoped to wasm32 so genuine dead
// code is still caught on every other target.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod subscriptions;
pub mod tasks;
pub mod tools;

/// UI resources for MCP Apps Extension (SEP-1865)
pub mod ui;

/// MCP Apps Extension types for interactive UI support (ChatGPT Apps, MCP-UI)
#[cfg(feature = "mcp-apps")]
pub mod mcp_apps;

// Re-export transport message type
pub use crate::shared::transport::TransportMessage;

// Re-export protocol version constants
pub use crate::{DEFAULT_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS};

// Re-export commonly used types for flat access.
// protocol/mod.rs re-exports all domain module types, so a single
// `pub use protocol::*` provides `types::X` for every type.
pub use protocol::*;

pub use auth::{AuthInfo, AuthScheme};
// The 2026-07-28 client-side caching vocabulary. NARROW re-export: only the two
// PUBLIC items. The projector and its classification enum are `pub(crate)`
// dispatcher plumbing and deliberately stay off the public surface.
pub use caching::{CacheScope, DEFAULT_TTL_MS};
pub use capabilities::{
    ClientCapabilities, ClientTasksCapability, CompletionCapabilities, ElicitationCapabilities,
    FormElicitationCapability, LoggingCapabilities, PromptCapabilities, ResourceCapabilities,
    RootsCapabilities, SamplingCapabilities, ServerCapabilities, ServerTasksCapability,
    ToolCapabilities,
};
pub use elicitation::{
    ElicitAction, ElicitRequestParams, ElicitResult, ElicitationCompleteNotification,
};
pub use jsonrpc::{JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId};
// NARROW re-export: only the handler-AUTHORING and client-facing RESULT types.
// Every MRTR parsing/plumbing helper stays `pub(crate)` (Phase-113 D-10).
pub use mrtr::{
    InputRequest, InputRequestKind, InputRequests, InputRequiredResult, InputResponse,
    InputResponses, MrtrOutcome, MrtrSignal,
};
// `subscriptions/listen` wire types (HTTP-04). The classification helpers
// (`subscription_kind_of`, `SubscriptionFilter::covers`) stay `pub(crate)` —
// only the WIRE nouns, the two reserved-name constants and the one shared
// capability predicate are public surface.
pub use subscriptions::{
    advertises_subscriptions, SubscriptionAcknowledgedParams, SubscriptionFilter,
    SubscriptionsListenParams, SubscriptionsListenResult, ACKNOWLEDGED_METHOD,
    SUBSCRIPTIONS_LISTEN_METHOD, SUBSCRIPTION_ID_META_KEY,
};
pub use ui::{ToolUIMetadata, UIMimeType, UIResource, UIResourceContents};

// MCP Apps Extension re-exports
#[cfg(feature = "mcp-apps")]
pub use mcp_apps::{
    ChatGptToolMeta, ExtendedUIMimeType, HostType, NotifyLevel, RemoteDomFramework, ToolVisibility,
    UIAction, UIContent, UIDimensions, UIMetadata, WidgetCSP, WidgetMeta, WidgetResponseMeta,
};
