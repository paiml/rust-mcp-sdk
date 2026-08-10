//! MCP protocol-specific types.
//!
//! This module contains the core protocol types including initialization,
//! version negotiation, request routing, and completion types.

pub mod context;
pub mod error_codes;
pub mod version;

use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};
use serde::{Deserialize, Serialize};

// Re-export version constants and negotiation function.
pub use version::*;

// Re-export the additive protocol-context value types (Phase 112).
pub use context::{ProtocolContext, TraceContext};

// Re-export domain modules' types for backward compatibility.
// Types that were previously in this file are now in their own modules
// and re-exported via types/mod.rs. These re-exports preserve the
// `crate::types::protocol::X` import paths used throughout the codebase.
// `super::caching` is deliberately NOT globbed here. The rest of this list
// exists for BACKWARD compatibility — those types used to live in this file, so
// the `crate::types::protocol::X` paths predate the split. `types::caching` is
// new in Phase 115 and has no such history: `types/mod.rs` re-exports exactly
// its two public items (`CacheScope`, `DEFAULT_TTL_MS`) and documents that
// narrowness on purpose, so a glob here would mint a SECOND public path
// (`pmcp::types::protocol::CacheScope`) that nothing imports, and would also
// pull the module's `pub(crate)` projector plumbing into `types::protocol`.
pub use super::content::*;
pub use super::notifications::*;
pub use super::prompts::*;
pub use super::resources::*;
pub use super::sampling::*;
pub use super::tasks::*;
pub use super::tools::*;

/// Protocol version identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub String);

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self(crate::DEFAULT_PROTOCOL_VERSION.to_string())
    }
}

impl ProtocolVersion {
    /// Get the version as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Icon information for entities (MCP 2025-11-25).
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::IconInfo;
///
/// let icon = IconInfo::new("https://example.com/icon.png")
///     .with_mime_type("image/png")
///     .with_sizes(vec!["32x32".to_string()]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct IconInfo {
    /// Icon source URL.
    ///
    /// Serialized as `src` per the MCP 2025-11-25 spec. Accepts `url` as a
    /// deserialize alias for backwards compat with pre-2025-11-25 servers.
    #[serde(alias = "url")]
    pub src: String,
    /// Icon MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Icon sizes (e.g., `["16x16", "32x32"]`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
    /// Icon theme preference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<IconTheme>,
}

impl IconInfo {
    /// Create an `IconInfo` with the icon source URL.
    ///
    /// Optional fields (`mime_type`, `sizes`, `theme`) default to `None`.
    /// The argument is serialized as `src` per MCP 2025-11-25 spec.
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            mime_type: None,
            sizes: None,
            theme: None,
        }
    }

    /// Set the MIME type for the icon.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the icon sizes (e.g., \["16x16", "32x32"\]).
    pub fn with_sizes(mut self, sizes: Vec<String>) -> Self {
        self.sizes = Some(sizes);
        self
    }

    /// Set the icon theme preference.
    pub fn with_theme(mut self, theme: IconTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

/// Icon theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconTheme {
    /// Light theme icon
    Light,
    /// Dark theme icon
    Dark,
}

/// MCP-specific JSON-RPC error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorCode {
    /// Invalid request
    InvalidRequest = -32600,
    /// Method not found
    MethodNotFound = -32601,
    /// Invalid parameters
    InvalidParams = -32602,
    /// Internal error
    InternalError = -32603,
}

/// Implementation information.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor and fluent
/// methods to remain forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::Implementation;
///
/// let info = Implementation::new("my-server", "1.0.0")
///     .with_title("My Server")
///     .with_description("A great server");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name (e.g., "mcp-sdk-rust")
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Implementation version
    pub version: String,
    /// Optional website URL (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    /// Optional description (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<IconInfo>>,
}

impl Implementation {
    /// Create an `Implementation` with just name and version.
    ///
    /// The optional 2025-11-25 fields (title, website\_url, description, icons)
    /// default to `None`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            version: version.into(),
            website_url: None,
            description: None,
            icons: None,
        }
    }

    /// Set a human-readable title (MCP 2025-11-25).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the website URL (MCP 2025-11-25).
    pub fn with_website_url(mut self, url: impl Into<String>) -> Self {
        self.website_url = Some(url.into());
        self
    }

    /// Set a human-readable description (MCP 2025-11-25).
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set icons for the implementation (MCP 2025-11-25).
    pub fn with_icons(mut self, icons: Vec<IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }
}

/// Initialize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version the client wants to use
    pub protocol_version: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Client implementation info
    pub client_info: Implementation,
}

impl InitializeRequest {
    /// Create an initialize request with the latest protocol version.
    pub fn new(client_info: Implementation, capabilities: ClientCapabilities) -> Self {
        Self {
            protocol_version: crate::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info,
        }
    }
}

/// Initialize response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::{InitializeResult, Implementation};
/// use pmcp::ServerCapabilities;
///
/// let result = InitializeResult::new(
///     Implementation::new("my-server", "1.0.0"),
///     ServerCapabilities::tools_only(),
/// ).with_instructions("Use this server for ...");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Negotiated protocol version
    pub protocol_version: ProtocolVersion,
    /// Server capabilities
    pub capabilities: ServerCapabilities,
    /// Server implementation info
    pub server_info: Implementation,
    /// Optional instructions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

impl InitializeResult {
    /// Create an initialize result with the default protocol version.
    ///
    /// Instructions default to `None`.
    pub fn new(server_info: Implementation, capabilities: ServerCapabilities) -> Self {
        Self {
            protocol_version: ProtocolVersion::default(),
            capabilities,
            server_info,
            instructions: None,
        }
    }

    /// Set optional instructions for the client.
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
}

/// Pagination cursor.
pub type Cursor = Option<String>;

/// Request metadata that can be attached to any request.
///
/// This follows the MCP protocol's `_meta` field specification.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::protocol::RequestMeta;
/// use pmcp::types::notifications::ProgressToken;
///
/// let meta = RequestMeta::new()
///     .with_progress_token(ProgressToken::String("tok-1".to_string()))
///     .with_task_id("task-abc");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct RequestMeta {
    /// Progress token for out-of-band progress notifications.
    ///
    /// If specified, the caller is requesting progress notifications for this request.
    /// The value is an opaque token that will be attached to subsequent progress notifications.
    /// The receiver is not obligated to provide these notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_token: Option<super::notifications::ProgressToken>,

    /// Task ID for workflow continuation (PMCP extension).
    ///
    /// When present on a `tools/call` request, the server records the tool
    /// result against the referenced workflow task after normal execution.
    /// The tool call itself proceeds as normal; the recording is best-effort.
    #[serde(skip_serializing_if = "Option::is_none", rename = "_task_id")]
    #[allow(clippy::pub_underscore_fields)]
    pub _task_id: Option<String>,

    /// Arbitrary namespaced `_meta` keys (extensible catch-all).
    ///
    /// Any key on the `_meta` object that is not `progressToken` or `_task_id`
    /// flows here on deserialize and is emitted back on serialize. This lets
    /// callers carry namespaced extension state (e.g.
    /// `io.modelcontextprotocol/*` keys or team-guard depth/ancestor state) on a
    /// `tools/call` request without a typed dependency.
    ///
    /// Because this is `#[serde(flatten)]` over a map that is empty by default,
    /// an empty `other` emits NO keys — existing `RequestMeta` serialization for
    /// `progressToken`/`_task_id` is byte-for-byte unchanged (additive only).
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

impl RequestMeta {
    /// Create an empty `RequestMeta`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the progress token.
    pub fn with_progress_token(mut self, token: super::notifications::ProgressToken) -> Self {
        self.progress_token = Some(token);
        self
    }

    /// Set the task ID for workflow continuation (PMCP extension).
    #[allow(clippy::used_underscore_binding)]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self._task_id = Some(task_id.into());
        self
    }

    /// Attach an arbitrary namespaced `_meta` key/value.
    ///
    /// The key is inserted into the flattened [`other`](Self::other) map and
    /// round-trips through serialize/deserialize. Use namespaced keys (e.g.
    /// `io.modelcontextprotocol/related-task`) to avoid collisions with the
    /// typed `progressToken`/`_task_id` fields.
    #[must_use]
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.other.insert(key.into(), value);
        self
    }

    /// Read a namespaced `_meta` key previously set via [`with_meta`](Self::with_meta)
    /// or populated on deserialize.
    #[must_use]
    pub fn get_meta(&self, key: &str) -> Option<&serde_json::Value> {
        self.other.get(key)
    }
}

/// Completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteRequest {
    /// The reference to complete from
    pub r#ref: CompletionReference,
    /// The argument to complete
    pub argument: CompletionArgument,
}

/// Completion reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CompletionReference {
    /// Complete from a resource
    #[serde(rename = "ref/resource")]
    Resource {
        /// Resource URI
        uri: String,
    },
    /// Complete from a prompt
    #[serde(rename = "ref/prompt")]
    Prompt {
        /// Prompt name
        name: String,
    },
}

/// Completion argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionArgument {
    /// Argument name
    pub name: String,
    /// Argument value
    pub value: String,
}

/// Completion result wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    /// Completion options
    pub completion: CompletionResult,
}

/// Completion result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CompletionResult {
    /// Suggested values
    pub values: Vec<String>,
    /// Total number of completions available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    /// Whether there are more completions available
    #[serde(default)]
    pub has_more: bool,
}

impl CompletionResult {
    /// Create a completion result with the given values.
    ///
    /// `has_more` defaults to `false`, `total` defaults to `None`.
    pub fn new(values: Vec<String>) -> Self {
        Self {
            values,
            total: None,
            has_more: false,
        }
    }

    /// Set the total number of completions available.
    pub fn with_total(mut self, total: usize) -> Self {
        self.total = Some(total);
        self
    }

    /// Set whether there are more completions available.
    pub fn with_has_more(mut self, has_more: bool) -> Self {
        self.has_more = has_more;
        self
    }
}

/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum ClientRequest {
    /// Initialize the connection
    #[serde(rename = "initialize")]
    Initialize(InitializeRequest),
    /// List available tools
    #[serde(rename = "tools/list")]
    ListTools(super::tools::ListToolsRequest),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(super::tools::CallToolRequest),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(super::prompts::ListPromptsRequest),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(super::prompts::GetPromptRequest),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(super::resources::ListResourcesRequest),
    /// List resource templates
    #[serde(rename = "resources/templates/list")]
    ListResourceTemplates(super::resources::ListResourceTemplatesRequest),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(super::resources::ReadResourceRequest),
    /// Subscribe to resource updates
    #[serde(rename = "resources/subscribe")]
    Subscribe(super::resources::SubscribeRequest),
    /// Unsubscribe from resource updates
    #[serde(rename = "resources/unsubscribe")]
    Unsubscribe(super::resources::UnsubscribeRequest),
    /// Request completion
    #[serde(rename = "completion/complete")]
    Complete(CompleteRequest),
    /// Set logging level
    #[serde(rename = "logging/setLevel")]
    SetLoggingLevel {
        /// Logging level to set
        level: super::notifications::LoggingLevel,
    },
    /// Ping request
    #[serde(rename = "ping")]
    Ping,
    /// Create message (sampling).
    /// Boxed to match `ServerRequest::CreateMessage` and avoid inflating the enum.
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<super::sampling::CreateMessageParams>),
    /// Get task status (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/get")]
    TasksGet(crate::types::tasks::GetTaskRequest),
    /// Get task result (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/result")]
    TasksResult(crate::types::tasks::GetTaskPayloadRequest),
    /// List tasks (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/list")]
    TasksList(crate::types::tasks::ListTasksRequest),
    /// Cancel a task (MCP 2025-11-25 Tasks).
    #[serde(rename = "tasks/cancel")]
    TasksCancel(crate::types::tasks::CancelTaskRequest),
}

/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to create a message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<super::sampling::CreateMessageParams>),
    /// List roots request
    #[serde(rename = "roots/list")]
    ListRoots,
    /// Request to elicit user input (spec method: elicitation/create)
    #[serde(rename = "elicitation/create")]
    ElicitationCreate(Box<crate::types::elicitation::ElicitRequestParams>),
}

/// Combined request types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Request {
    /// Client request
    Client(Box<ClientRequest>),
    /// Server request
    Server(Box<ServerRequest>),
}

/// Parameters for the v2 `server/discover` request (VERS-04, MCP 2026-07-28).
///
/// `server/discover` takes no required parameters today. This struct is
/// `#[non_exhaustive]` so future spec-defined fields can be added without a
/// breaking change. Adding a new public STRUCT is a non-breaking minor addition
/// — it introduces no new variant to any exhaustive public enum.
///
/// # Routing
///
/// `server/discover` is deliberately NOT a variant of the public exhaustive
/// [`ClientRequest`] / [`Request`] enums: adding one would break downstream
/// exhaustive `match` arms in the workspace crates (a source-level break that
/// `cargo-semver-checks` classifies as "minor" but that violates the milestone's
/// hard 2.x-minor promise). Instead it is carried by the crate-private
/// `InternalClientRequest` and routed by matching the raw method string via
/// `classify_internal_method` BEFORE conversion into the public enum. Plan 05
/// wires this classifier into the server request path so v2 `server/discover`
/// reaches the era-gated handler while v1 / non-opted-in requests fall through
/// to the existing `parse_request` → `method_not_found` → `-32601` (D-10).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ServerDiscoverRequest {}

impl ServerDiscoverRequest {
    /// Create an empty `server/discover` request.
    pub fn new() -> Self {
        Self {}
    }
}

/// The wire result of a v2 `server/discover` request (VERS-04, MCP 2026-07-28).
///
/// A read-only projection of the server's ALREADY-COMPUTED capabilities plus its
/// implementation info. It reuses the existing [`ServerCapabilities`] /
/// [`Implementation`] types — it does NOT invent a parallel capability model — and
/// is produced ONLY through the server's isolated
/// `discover_result_from_capabilities` conversion fn so a final-spec wire
/// adjustment stays localized.
///
/// It is `#[non_exhaustive]` (spec-defined fields may be added without a break).
///
/// # Why it lives in `types::protocol` and not in the server
///
/// Phase 113 (CLNT-01) makes this the return type of
/// [`Client::server_discover`](crate::Client::server_discover), and the pmcp
/// `Client` compiles on `wasm32` where the whole `server::core` module is
/// `cfg`-ed out. Keeping the shared wire type in the `cfg`-agnostic protocol
/// module is what lets ONE type serve both ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ServerDiscoverResult {
    /// The negotiated protocol version this projection was produced under.
    pub protocol_version: String,
    /// The server's already-computed capabilities (incl. the `extensions` map).
    pub capabilities: crate::types::ServerCapabilities,
    /// The server's self-reported implementation info.
    ///
    /// SELF-REPORTED and unverified — never derive authorization from it.
    pub server_info: Implementation,

    /// The protocol versions this server accepts — the REQUIRED
    /// `2026-07-28` `DiscoverResult.supportedVersions` field.
    ///
    /// "MCP Protocol Versions this server supports. The client should choose a
    /// version from this list for use in subsequent requests"
    /// (`schema/vendored/core-2026-07-28/schema.ts:678-696`).
    ///
    /// # ONE accept list, read twice (Phase 118.1, G-7)
    ///
    /// This is the SAME list an unsupported-version rejection reports as
    /// `error.data.supported`: both read the server's configured accept list
    /// (`Server::supported_protocol_versions()`), which reaches the discover
    /// projection through the `DiscoverSource` bundle rather than through a
    /// second list assembled here. That is a structural requirement, not a
    /// stylistic one — the official conformance suite's
    /// `ServerUnsupportedVersionError` check asserts that every element of
    /// `error.data.supported` also appears in this field, so two sources would
    /// be two chances to disagree. `tests/v2_discover_supported_versions.rs`
    /// fences the correlation against a SINGLE spawned server, where two
    /// independent lists cannot both pass.
    ///
    /// # Why not `Option`, and why no `skip_serializing_if`
    ///
    /// The spec makes the field REQUIRED, and unlike [`ttl_ms`](Self::ttl_ms)
    /// there is no "the handler expressed no preference" state to model: a
    /// server always has an accept list. Omitting the key would fail the suite
    /// outright, so it is emitted unconditionally.
    ///
    /// `#[serde(default)]` affects DESERIALIZATION only and is the repo's
    /// tolerant-reader / strict-emitter rule (Phase 118.1 D-03) applied to the
    /// client end: pmcp always EMITS the field, but
    /// [`Client::server_discover`](crate::Client::server_discover) must not hard
    /// fail against a peer that predates it or omits it.
    ///
    /// # Semver
    ///
    /// Adding this field is additive rather than a major bump because this
    /// struct is `#[non_exhaustive]`, so `cargo semver-checks`'
    /// `constructible_struct_adds_field` does not fire.
    ///
    /// That verdict is the OPPOSITE of the one plan 118.1-03 recorded for
    /// `Content::Resource`, and the asymmetry is entirely explained by that one
    /// attribute: `Content` is a public EXHAUSTIVE enum whose `Resource`
    /// variant was not `#[non_exhaustive]`, so every representation of a new
    /// spec field there was a major lint (`enum_struct_variant_field_added`)
    /// and shipped as a documented one-time delta (D-15). Here the attribute
    /// was already in place, so the same class of change costs nothing.
    #[serde(default)]
    pub supported_versions: Vec<String>,

    /// How long (in milliseconds) a client MAY cache this response — the
    /// `2026-07-28` `CacheableResult.ttlMs` hint.
    ///
    /// `u64` is the MEASURED mapping: the vendored artifact declares
    /// `$defs.CacheableResult.properties.ttlMs` as
    /// `{"type": "integer", "minimum": 0}` (asserted by
    /// `tests/v2_core_schema_facts.rs`), so integrality and non-negativity are
    /// contract. The one residual is the absent upper bound — JSON Schema
    /// `integer` is unbounded while `u64` is not — which at millisecond
    /// resolution is roughly 584 million years and is an ACCEPTED risk.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`DEFAULT_TTL_MS`](crate::types::DEFAULT_TTL_MS)
    /// (`0`, "immediately stale") — D-08.
    ///
    /// **v2 only.** `server/discover` is itself a v2-only method, but the
    /// projection rule is the same: a value set here is emitted only on the
    /// `2026-07-28` wire and actively STRIPPED otherwise (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07).** The field is
    /// required on the `2026-07-28` projection, but modelling it as `Option`
    /// plus inject-on-v2 fails CLOSED (a missed path merely omits a hint),
    /// whereas a non-`Option` field plus strip-on-v1 fails OPEN (a missed path
    /// leaks a v2 key onto the v1 wire).
    ///
    /// Not to be confused with
    /// [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
    /// LIFETIME rather than a cache-freshness hint (D-10).
    ///
    /// **No builder by design.** This struct's only producer is the server's
    /// `discover_result_from_capabilities` conversion, with no handler seam, so
    /// a builder method here would be public API no server author can reach
    /// through normal configuration — unlike `ListResourcesResult` and
    /// `ReadResourceResult`, which a
    /// [`ResourceHandler`](crate::server::ResourceHandler) returns from `list`
    /// and `read` and which therefore do carry builders. The field stays `pub`,
    /// so a caller constructing the struct directly can still set it.
    ///
    /// (`ListResourceTemplatesResult` carries builders too, but is NOT
    /// handler-reachable — see its own note. Two of the six cacheable results
    /// are settable through a handler, not three; 115-10 corrected an earlier
    /// version of this paragraph that said three.)
    ///
    /// Adding this field is additive rather than a major bump because this
    /// struct is `#[non_exhaustive]`, so `cargo semver-checks`'
    /// `constructible_struct_adds_field` does not fire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,

    /// The intended sharing scope of the cached response — the `2026-07-28`
    /// `CacheableResult.cacheScope` hint.
    ///
    /// `None` means the handler expressed no preference; the v2 projection then
    /// emits the safe default [`CacheScope::Private`](crate::types::CacheScope)
    /// (D-08). Read [`CacheScope`](crate::types::CacheScope)'s `# Security`
    /// section before setting `Public`: a discover projection can be
    /// authorization-filtered, in which case sharing it across authorization
    /// contexts would disclose capabilities one caller may not hold.
    ///
    /// **v2 only** and **no builder by design** — see [`ttl_ms`](Self::ttl_ms).
    /// Additive under semver for the same `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

/// Crate-private internal dispatch representation for methods that must be
/// routable WITHOUT appearing in the public exhaustive [`ClientRequest`] /
/// [`Request`] enums.
///
/// This enum is `pub(crate)`, so it is invisible to `cargo-semver-checks` /
/// `cargo-public-api` and can grow variants freely without any public API or
/// downstream exhaustive-match impact.
// Consumed in production via `classify_internal_method` →
// `IngressRequest::Internal` (the crate-private `parse_request_or_internal`
// routing seam in `src/shared/protocol_helpers.rs`), which classifies
// `server/discover` into this internal representation BEFORE the public-enum
// conversion. On the HTTP transport the streamable-HTTP `HttpIngress::Discover`
// classifier then routes it to `Server::handle_discover`, which era-gates it via
// the shared `build_discover_response` projection.
#[derive(Debug, Clone)]
pub(crate) enum InternalClientRequest {
    /// The v2 `server/discover` request (VERS-04).
    ServerDiscover(ServerDiscoverRequest),
    /// The v2 `tasks/update` request (Phase 114, TASK-02), carrying its **RAW**
    /// `params` and NOTHING else.
    ///
    /// # Why it is here and NOT a [`ClientRequest`] variant
    ///
    /// MEASURED, not assumed: [`ClientRequest`] carries
    /// `#[derive(Debug, Clone, Serialize, Deserialize)]` and
    /// `#[serde(tag = "method", content = "params", rename_all = "camelCase")]`
    /// with **no `#[non_exhaustive]`**. Adding a variant to a public exhaustive
    /// enum is `enum_variant_added`, a semver-**MAJOR** break, and would fail this
    /// milestone's hard 2.x-minor promise for a reason unrelated to the tasks
    /// surface. `cargo semver-checks check-release` is what catches a regression;
    /// `client_request_has_no_tasks_update_variant` in
    /// `tests/v2_tasks_update_routing.rs` is the in-repo guard that fails with an
    /// explanation rather than only failing CI.
    ///
    /// Adding `#[non_exhaustive]` to [`ClientRequest`] is NOT the escape hatch
    /// either: that is itself a source break for every downstream exhaustive
    /// `match`. This enum is `pub(crate)`, so it is invisible to
    /// `cargo-semver-checks` / `cargo-public-api` and may grow freely.
    ///
    /// The `server/discover` precedent (the sibling variant above) established
    /// exactly this route in Phase 112 and is followed here by name.
    ///
    /// # Why the params stay RAW, and why there is no id field
    ///
    /// RAW because [`classify_internal_method`] **must never reject a body**: a
    /// malformed `params` has to become a structured `-32602` in the SERVED
    /// branch — after the era gate, the backend gate, the client-declaration
    /// `-32021` gate and the `-32003` auth refusal have all run (114-09's
    /// documented order) — not a parse error before them. A classifier that
    /// deserialized would hand an UNAUTHENTICATED caller a params error instead of
    /// `-32003`, inverting that ordering guarantee.
    ///
    /// No request id: the classifier never receives one. `parse_request_or_internal`
    /// reads `request.id` itself and returns it as the FIRST element of its
    /// `(RequestId, IngressRequest)` tuple, so the routing site takes the id from
    /// there — exactly as the `ServerDiscover` arm does. A field the classifier
    /// cannot populate would either be a lie or force a signature change on the
    /// shared classifier.
    TasksUpdate {
        /// The request's `params`, verbatim and undecoded (`Value::Null` when the
        /// frame carried none).
        params: serde_json::Value,
    },
}

/// The wire method string of the v2 `server/discover` request (VERS-04).
///
/// Single-sourced here so the classifier and the streamable-HTTP transport's
/// header cross-check (which pins this method rather than reading it from the
/// body) can never disagree on the spelling.
pub(crate) const SERVER_DISCOVER_METHOD: &str = "server/discover";

/// The wire method string of the v2 `tasks/update` request (Phase 114, TASK-02).
///
/// Single-sourced so the classifier, the streamable-HTTP ingress fast-reject and
/// the routing-header table can never disagree on the spelling.
///
/// # This is a RE-EXPORT, deliberately — the plan asked for a new `const`
///
/// 114-13's action text said to declare a fresh
/// `TASKS_UPDATE_METHOD: &str = "tasks/update"` beside [`SERVER_DISCOVER_METHOD`].
/// That premise was false: the spelling ALREADY existed, at
/// [`crate::types::mrtr::TASKS_UPDATE_METHOD`], as a row of
/// `TASK_NAME_BEARING_METHODS` (Phase 114, DQ4). Minting a second constant with
/// the same name and value is precisely the "two spellings that can disagree"
/// failure the single-sourcing rustdoc on [`SERVER_DISCOVER_METHOD`] exists to
/// prevent, so this re-exports the ONE definition instead.
///
/// MEASURED: after this change `src/` contains exactly ONE non-test
/// `"tasks/update"` string literal — the definition at
/// `src/types/mrtr.rs`. Every other occurrence is a doc comment or a
/// `#[cfg(test)]` fixture.
pub(crate) use crate::types::mrtr::TASKS_UPDATE_METHOD;

/// Classify a raw JSON-RPC method string into a crate-private internal request,
/// if it is one of the internally-routed (non-public-enum) methods.
///
/// Returns `Some(InternalClientRequest::ServerDiscover(..))` for the exact
/// method string [`SERVER_DISCOVER_METHOD`], `Some(InternalClientRequest::TasksUpdate { .. })`
/// for [`TASKS_UPDATE_METHOD`], and `None` for every other method (which then
/// flows through the normal public-enum dispatch path). Plan 05 calls this from
/// the server request path BEFORE the public-enum conversion. Consumed in
/// production by [`parse_request_or_internal`](crate::shared::protocol_helpers)
/// (Plan 05).
///
/// # It does not, and must not, deserialize `params`
///
/// `params` is passed through verbatim into the variant. See
/// [`InternalClientRequest::TasksUpdate`] for the ordering guarantee that depends
/// on it.
///
/// The parameter's ARITY and TYPES are unchanged by Phase 114 — the second
/// parameter was already `&serde_json::Value`, spelled `_params` because the
/// `server/discover` arm ignores it. It is renamed to `params` here because the
/// `tasks/update` arm READS it, and an underscore prefix that claims "unused" on a
/// binding that is used is both a `clippy::pedantic` violation
/// (`used_underscore_binding`) and the stale-marker failure class 113-29 recorded.
/// No parameter was added, removed or retyped — in particular the classifier still
/// does NOT receive the request id.
pub(crate) fn classify_internal_method(
    method: &str,
    params: &serde_json::Value,
) -> Option<InternalClientRequest> {
    match method {
        SERVER_DISCOVER_METHOD => Some(InternalClientRequest::ServerDiscover(
            ServerDiscoverRequest::new(),
        )),
        TASKS_UPDATE_METHOD => Some(InternalClientRequest::TasksUpdate {
            params: params.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;

    #[test]
    fn request_meta_empty_other_serialization_unchanged() {
        // A RequestMeta carrying only progress_token/_task_id must serialize to
        // EXACTLY the pre-change JSON — no `other`/flatten key emitted when empty.
        let meta = RequestMeta::new()
            .with_progress_token(super::super::notifications::ProgressToken::String(
                "tok-1".to_string(),
            ))
            .with_task_id("task-abc");
        let json = serde_json::to_value(&meta).unwrap();
        let expected = serde_json::json!({
            "progressToken": "tok-1",
            "_task_id": "task-abc",
        });
        assert_eq!(json, expected, "empty `other` must emit no extra keys");

        // A fully-empty RequestMeta is an empty object.
        let empty = serde_json::to_value(RequestMeta::new()).unwrap();
        assert_eq!(empty, serde_json::json!({}));
    }

    #[test]
    fn request_meta_custom_key_round_trips_via_other() {
        let meta = RequestMeta::new().with_meta("x-pmcp-team-depth", serde_json::json!(3));
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["x-pmcp-team-depth"], serde_json::json!(3));

        let back: RequestMeta = serde_json::from_value(json).unwrap();
        assert_eq!(
            back.get_meta("x-pmcp-team-depth"),
            Some(&serde_json::json!(3))
        );
        // Typed fields stay empty; custom key does NOT leak into them.
        assert!(back.progress_token.is_none());
        assert!(back._task_id.is_none());
    }

    #[test]
    fn request_meta_typed_fields_do_not_leak_into_other() {
        // Deserializing progressToken/_task_id must land in the typed fields,
        // NOT in the `other` catch-all; unknown namespaced keys populate `other`.
        let json = serde_json::json!({
            "progressToken": "p1",
            "_task_id": "t1",
            "io.modelcontextprotocol/related-task": {"taskId": "abc"},
        });
        let meta: RequestMeta = serde_json::from_value(json).unwrap();
        assert!(meta.progress_token.is_some());
        assert_eq!(meta._task_id.as_deref(), Some("t1"));
        assert!(!meta.other.contains_key("progressToken"));
        assert!(!meta.other.contains_key("_task_id"));
        assert!(meta
            .other
            .contains_key("io.modelcontextprotocol/related-task"));
    }

    // -----------------------------------------------------------------------
    // The `_meta` wire-spelling contract (Phase-113 D-113-A / D-113-B).
    //
    // Every request type that carries a per-request `_meta` object MUST spell it
    // `_meta` on the wire (the MCP spec spelling) and MUST also ACCEPT the legacy
    // `meta` spelling pmcp emitted before this phase, so an older pmcp peer keeps
    // interoperating on ingress. These tests are the binding guard for both
    // halves.
    // -----------------------------------------------------------------------

    /// The reserved `_meta` payload every spelling test round-trips.
    fn meta_probe() -> serde_json::Value {
        serde_json::json!({ "ns/key": "v" })
    }

    /// `base` with `key` set to [`meta_probe`].
    fn with_meta_key(base: &serde_json::Value, key: &str) -> serde_json::Value {
        let mut out = base.clone();
        out.as_object_mut()
            .expect("base is an object")
            .insert(key.to_string(), meta_probe());
        out
    }

    /// Assert the full `_meta` wire contract for one request type, driven
    /// entirely from JSON so the test never depends on a Rust field NAME.
    ///
    /// 1. EGRESS — a request carrying `_meta` re-serializes under the SPEC
    ///    spelling `_meta`, never the camelCase-renamed `meta`.
    /// 2. INGRESS (spec) — a spec-spelled `_meta` on the wire survives a
    ///    deserialize → serialize round trip (i.e. it was actually READ, not
    ///    silently dropped by an unknown-field skip).
    /// 3. INGRESS (legacy) — the `meta` spelling pmcp emitted before Phase 113
    ///    still deserializes, via `#[serde(alias = "meta")]`, and is re-emitted
    ///    under the spec spelling.
    fn assert_meta_spelling<T>(base: &serde_json::Value)
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        for (label, incoming) in [
            ("spec spelling `_meta`", with_meta_key(base, "_meta")),
            ("legacy spelling `meta`", with_meta_key(base, "meta")),
        ] {
            let typed: T = serde_json::from_value(incoming.clone())
                .unwrap_or_else(|e| panic!("{label} must deserialize ({incoming}): {e}"));
            let wire = serde_json::to_value(&typed).expect("serializes");
            assert_eq!(
                wire.get("_meta"),
                Some(&meta_probe()),
                "{label}: the reserved object must be READ and re-emitted under \
                 the spec spelling `_meta`; got {wire}"
            );
            assert!(
                wire.get("meta").is_none(),
                "{label}: the camelCase-renamed `meta` spelling must NOT be \
                 emitted; got {wire}"
            );
        }
    }

    /// An absent `_meta` must emit NO key at all, so v1 wire bytes are unchanged
    /// for every request that does not opt into the per-request signal.
    fn assert_absent_meta_emits_no_key<T>(base: &serde_json::Value)
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let typed: T = serde_json::from_value(base.clone()).expect("base deserializes");
        let wire = serde_json::to_value(&typed).expect("serializes");
        assert!(
            wire.get("_meta").is_none() && wire.get("meta").is_none(),
            "an absent _meta must emit neither spelling; got {wire}"
        );
    }

    /// `(base params, type)` for every request type that carries a typed `_meta`.
    ///
    /// Deliberately just the three name/uri-bearing methods. The list-shaped
    /// requests do NOT carry a typed `_meta` — adding a `pub` field to those
    /// constructible `pub` structs is a MAJOR semver break, so the v2 era signal
    /// for those methods is read from the RAW body at HTTP ingress instead
    /// (Phase-113 D-113-B / D-113-D resolution).
    macro_rules! for_each_meta_bearing_request {
        ($assertion:ident) => {
            $assertion::<super::super::tools::CallToolRequest>(
                &serde_json::json!({ "name": "t", "arguments": {} }),
            );
            $assertion::<super::super::prompts::GetPromptRequest>(
                &serde_json::json!({ "name": "p", "arguments": {} }),
            );
            $assertion::<super::super::resources::ReadResourceRequest>(
                &serde_json::json!({ "uri": "mem://x" }),
            );
        };
    }

    #[test]
    fn every_meta_bearing_request_uses_the_spec_spelling_and_accepts_the_legacy_alias() {
        for_each_meta_bearing_request!(assert_meta_spelling);
    }

    #[test]
    fn absent_meta_emits_no_key_on_any_request_type() {
        for_each_meta_bearing_request!(assert_absent_meta_emits_no_key);
    }

    #[test]
    fn server_discover_request_round_trips() {
        // Empty-but-extensible struct serializes to `{}` and round-trips.
        let req = ServerDiscoverRequest::new();
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json, serde_json::json!({}));
        let _back: ServerDiscoverRequest = serde_json::from_value(json).unwrap();
        // Deserializing an object with unknown fields still succeeds (extensible).
        let _back2: ServerDiscoverRequest = serde_json::from_value(serde_json::json!({})).unwrap();
    }

    #[test]
    fn classify_internal_method_routes_server_discover() {
        // Exact "server/discover" → Some(ServerDiscover).
        let out = classify_internal_method("server/discover", &serde_json::json!({}));
        assert!(matches!(
            out,
            Some(InternalClientRequest::ServerDiscover(_))
        ));

        // Any other method → None (falls through to public-enum dispatch).
        assert!(classify_internal_method("tools/list", &serde_json::json!({})).is_none());
        assert!(classify_internal_method("initialize", &serde_json::json!({})).is_none());
        // Near-miss method names are NOT matched.
        assert!(classify_internal_method("server/discovery", &serde_json::json!({})).is_none());
    }

    /// `tasks/update` classifies as its own internal variant and carries its
    /// params VERBATIM (Phase 114 plan 13, TASK-02).
    ///
    /// The verbatim half is the property. A classifier that decoded would turn a
    /// malformed body into a parse error BEFORE the `-32003` auth refusal that
    /// `TaskDispatch::route_tasks_update` places ahead of the params — so the
    /// inputs below are deliberately not a well-formed `tasks/update` payload, and
    /// the assertion is that they arrive unchanged rather than that they are
    /// accepted.
    #[test]
    fn classify_internal_method_routes_tasks_update_with_raw_params() {
        let garbage = serde_json::json!({ "taskId": 1, "inputResponses": "not-an-object" });
        match classify_internal_method("tasks/update", &garbage) {
            Some(InternalClientRequest::TasksUpdate { params }) => {
                assert_eq!(params, garbage, "params must pass through undecoded");
            },
            other => panic!("tasks/update must classify as TasksUpdate, got {other:?}"),
        }

        // `Value::Null` (a frame with no params at all) is still classified — the
        // classifier judges the METHOD, never the body.
        assert!(matches!(
            classify_internal_method("tasks/update", &serde_json::Value::Null),
            Some(InternalClientRequest::TasksUpdate { .. })
        ));

        // Near-miss method names are NOT matched, and the three surviving v2
        // `tasks/*` methods keep their public-enum route.
        assert!(classify_internal_method("tasks/updates", &serde_json::json!({})).is_none());
        assert!(classify_internal_method("tasks/get", &serde_json::json!({})).is_none());
        assert!(classify_internal_method("tasks/cancel", &serde_json::json!({})).is_none());
    }

    /// The one spelling: [`TASKS_UPDATE_METHOD`] is the re-exported
    /// `types::mrtr` constant, not a second literal.
    #[test]
    fn the_tasks_update_method_spelling_is_single_sourced() {
        assert_eq!(TASKS_UPDATE_METHOD, "tasks/update");
        assert_eq!(
            TASKS_UPDATE_METHOD,
            crate::types::mrtr::TASKS_UPDATE_METHOD,
            "these must be the SAME item, not two constants that happen to agree"
        );
    }

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(super::super::tools::ListToolsRequest::default());
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/list");
    }

    #[test]
    fn test_task_client_request_variants() {
        let json_str = r#"{"method": "tasks/get", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksGet(_)));

        let json_str = r#"{"method": "tasks/result", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksResult(_)));

        let json_str = r#"{"method": "tasks/list", "params": {}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksList(_)));

        let json_str = r#"{"method": "tasks/cancel", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksCancel(_)));
    }

    #[test]
    fn test_task_client_request_roundtrip() {
        let req = ClientRequest::TasksGet(crate::types::tasks::GetTaskRequest {
            task_id: "t-123".to_string(),
        });
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tasks/get");
        assert_eq!(json["params"]["taskId"], "t-123");

        let deserialized: ClientRequest = serde_json::from_value(json).unwrap();
        assert!(matches!(deserialized, ClientRequest::TasksGet(_)));
    }

    #[test]
    fn request_meta_task_id_serializes_as_underscore() {
        let meta = RequestMeta::new().with_task_id("abc");
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["_task_id"], "abc");
        assert!(
            json.get("_taskId").is_none(),
            "_task_id must not be camelCased"
        );
    }

    #[test]
    fn request_meta_task_id_omitted_when_none() {
        let meta = RequestMeta::new();
        let json = serde_json::to_value(&meta).unwrap();
        assert!(
            json.get("_task_id").is_none(),
            "_task_id should be omitted when None"
        );
    }

    #[test]
    fn request_meta_task_id_deserialization() {
        let json_str = r#"{"_task_id": "task-xyz"}"#;
        let meta: RequestMeta = serde_json::from_str(json_str).unwrap();
        assert_eq!(meta._task_id.as_deref(), Some("task-xyz"));
        assert!(meta.progress_token.is_none());
    }

    /// Per MCP 2025-11-25 spec, `IconInfo` must serialize its source URL
    /// as `src` (not `url`). Regression test for CR-002 — `ChatGPT`'s pydantic
    /// validator rejects responses where the field is named `url`.
    #[test]
    fn icon_info_serializes_as_src() {
        let icon = IconInfo::new("https://example.com/icon.png").with_mime_type("image/png");
        let json = serde_json::to_value(&icon).unwrap();
        assert_eq!(json["src"].as_str(), Some("https://example.com/icon.png"));
        assert_eq!(json["mimeType"].as_str(), Some("image/png"));
        assert!(
            json.get("url").is_none(),
            "IconInfo must not emit `url` — MCP spec requires `src`"
        );
    }

    #[test]
    fn icon_info_deserializes_src() {
        let j = serde_json::json!({"src": "https://example.com/a.png"});
        let icon: IconInfo = serde_json::from_value(j).unwrap();
        assert_eq!(icon.src, "https://example.com/a.png");
    }

    /// Backwards compat: legacy `url` key must still deserialize via the alias.
    #[test]
    fn icon_info_deserializes_legacy_url_alias() {
        let j = serde_json::json!({"url": "https://example.com/b.png"});
        let icon: IconInfo = serde_json::from_value(j).unwrap();
        assert_eq!(icon.src, "https://example.com/b.png");
    }

    #[test]
    fn icon_info_round_trip_preserves_value() {
        let original = IconInfo::new("https://example.com/c.png")
            .with_mime_type("image/svg+xml")
            .with_sizes(vec!["32x32".to_string(), "64x64".to_string()]);
        let json = serde_json::to_value(&original).unwrap();
        let restored: IconInfo = serde_json::from_value(json).unwrap();
        assert_eq!(restored.src, original.src);
        assert_eq!(restored.mime_type, original.mime_type);
        assert_eq!(restored.sizes, original.sizes);
    }
}
