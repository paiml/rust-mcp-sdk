//! Tool types for MCP protocol.
//!
//! This module contains tool-related types including tool information,
//! annotations, requests, and results.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::Content;
use super::protocol::Cursor;
use super::protocol::RequestMeta;

/// Tool annotations for metadata hints.
///
/// Standard MCP annotations plus PMCP extensions for type-safe composition.
/// Clients SHOULD ignore annotations they don't understand (per MCP spec).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Human-readable title for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// If true, the tool does not modify any state (read-only operation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    /// If true, the tool may perform destructive operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    /// If true, calling the tool multiple times with same args has same effect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    /// If true, the tool interacts with external systems (network, filesystem, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,

    // =========================================================================
    // PMCP Extensions for Type-Safe Composition
    // =========================================================================
    /// Name of the output type for code generation (PMCP extension).
    ///
    /// Used by code generators to name the generated struct.
    /// Example: `"QueryResult"` generates `pub struct QueryResult { ... }`
    #[serde(
        rename = "pmcp:outputTypeName",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_type_name: Option<String>,
}

impl ToolAnnotations {
    /// Create empty annotations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set human-readable title for the tool.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set read-only hint (tool does not modify any state).
    ///
    /// When `true`, the tool only reads data and never modifies it.
    /// Useful for clients that want to allow read operations without confirmation.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only_hint = Some(read_only);
        self
    }

    /// Set destructive hint (tool may perform destructive operations).
    ///
    /// When `true`, the tool may permanently delete or modify data.
    /// Clients should warn users before executing destructive tools.
    pub fn with_destructive(mut self, destructive: bool) -> Self {
        self.destructive_hint = Some(destructive);
        self
    }

    /// Set idempotent hint (multiple calls with same args have same effect).
    ///
    /// When `true`, calling the tool multiple times with identical arguments
    /// produces the same result as calling it once. Safe to retry on failure.
    pub fn with_idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent_hint = Some(idempotent);
        self
    }

    /// Set open-world hint (tool interacts with external systems).
    ///
    /// When `true`, the tool may make network requests, access filesystem,
    /// or interact with other external services. Results may vary based on
    /// external state.
    pub fn with_open_world(mut self, open_world: bool) -> Self {
        self.open_world_hint = Some(open_world);
        self
    }

    /// Set output type name (PMCP extension for code generation).
    ///
    /// Used by code generators to name the generated struct for the tool's
    /// output type (e.g., `"QueryResult"` becomes `struct QueryResult`).
    ///
    /// The actual output schema is set on [`ToolInfo::with_output_schema`]
    /// as a top-level field (MCP spec 2025-06-18).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::ToolAnnotations;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_type_name("SearchResult");
    /// ```
    pub fn with_output_type_name(mut self, name: impl Into<String>) -> Self {
        self.output_type_name = Some(name.into());
        self
    }

    /// Returns `true` if all fields are `None` (no meaningful content).
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.read_only_hint.is_none()
            && self.destructive_hint.is_none()
            && self.idempotent_hint.is_none()
            && self.open_world_hint.is_none()
            && self.output_type_name.is_none()
    }
}

/// Tool execution metadata declaring task support level (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    /// Task support level for this tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_support: Option<TaskSupport>,
}

impl ToolExecution {
    /// Create empty tool execution metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the task support level.
    ///
    /// Marking a tool with [`TaskSupport::Required`] and registering a
    /// [`TaskStore`](crate::server::task_store::TaskStore) on the server (via
    /// [`ServerCoreBuilder::task_store`](crate::server::builder::ServerCoreBuilder::task_store))
    /// is how you expose a tool as an async MCP Task: the SDK then serves the
    /// `tasks/*` surface typed from the store. Which methods that is depends on
    /// the negotiated era (Phase 114) — v1 (2025-11-25) serves `tasks/get`,
    /// `tasks/result`, `tasks/list` and `tasks/cancel`; v2 (2026-07-28) serves
    /// `tasks/get`, `tasks/update` and `tasks/cancel`, with `tasks/list` and
    /// `tasks/result` retired to `-32601`. See
    /// `examples/s45_tool_as_task_lifecycle.rs` (v1) and
    /// `examples/s50_v2_tasks_server.rs` + `examples/s51_v2_tasks_agent.rs`
    /// (v2) for the full pattern.
    ///
    /// A `Required` tool with no task backend makes the server's `build()`
    /// return an error (never a hollow `tasks` capability).
    pub fn with_task_support(mut self, support: TaskSupport) -> Self {
        self.task_support = Some(support);
        self
    }
}

/// Task support level for a tool.
///
/// Set this via
/// [`ToolExecution::with_task_support`]. Pairing
/// [`TaskSupport::Required`] with a
/// [`TaskStore`](crate::server::task_store::TaskStore) on the server is the
/// recommended way to expose a tool as an async MCP Task — see
/// `examples/s45_tool_as_task_lifecycle.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSupport {
    /// Task creation is required for this tool
    Required,
    /// Task creation is optional for this tool
    Optional,
    /// Task creation is not supported for this tool
    Forbidden,
}

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name (unique identifier)
    pub name: String,
    /// Optional human-readable title (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for tool parameters
    pub input_schema: Value,
    /// JSON Schema for the tool's output type (MCP spec 2025-06-18).
    ///
    /// When present, clients can validate and type-check the tool's structured
    /// output. Code generators can create typed return structs instead of
    /// falling back to `serde_json::Value`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Tool annotations (hints and PMCP extensions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    /// Optional icons (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<super::protocol::IconInfo>>,
    /// Optional metadata (e.g., for UI resource association in MCP Apps Extension)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
    /// Execution metadata declaring task support level (MCP 2025-11-25).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ToolExecution>,
}

impl ToolInfo {
    /// Create a new `ToolInfo` without metadata or annotations.
    pub fn new(name: impl Into<String>, description: Option<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: None,
            execution: None,
        }
    }

    /// Create a new `ToolInfo` with annotations.
    ///
    /// Use this constructor when your tool has annotation hints. For output
    /// schema, chain [`ToolInfo::with_output_schema`] on the result.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::{ToolInfo, ToolAnnotations};
    /// use serde_json::json;
    ///
    /// let annotations = ToolAnnotations::new()
    ///     .with_read_only(true)
    ///     .with_output_type_name("MyResult");
    ///
    /// let tool = ToolInfo::with_annotations(
    ///     "my_tool",
    ///     Some("My tool description".to_string()),
    ///     json!({"type": "object"}),
    ///     annotations,
    /// ).with_output_schema(json!({"type": "object", "properties": {"result": {"type": "string"}}}));
    /// ```
    pub fn with_annotations(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        annotations: ToolAnnotations,
    ) -> Self {
        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: Some(annotations),
            icons: None,
            _meta: None,
            execution: None,
        }
    }

    /// Create a new `ToolInfo` with UI resource metadata.
    ///
    /// Produces nested `_meta` format compatible with both MCP standard and `ChatGPT`:
    /// - `_meta.ui.resourceUri` - MCP standard nested format
    /// - `_meta["openai/outputTemplate"]` - `ChatGPT` alias for the same URI
    pub fn with_ui(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: Value,
        ui_resource_uri: impl Into<String>,
    ) -> Self {
        let uri: String = ui_resource_uri.into();
        let meta = crate::types::ui::ToolUIMetadata::build_meta_map(&uri);

        Self {
            name: name.into(),
            title: None,
            description,
            input_schema,
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: Some(meta),
            execution: None,
        }
    }

    /// Set the output schema for this tool (MCP spec 2025-06-18).
    ///
    /// The output schema declares the JSON Schema that the tool's structured
    /// output conforms to, enabling clients to validate and type-check results.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::new("my_tool", None, json!({"type": "object"}))
    ///     .with_output_schema(json!({
    ///         "type": "object",
    ///         "properties": { "count": { "type": "integer" } }
    ///     }));
    /// ```
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Add widget metadata, deep-merging into existing `_meta`.
    ///
    /// This merges `WidgetMeta::to_meta_map()` into the tool's `_meta`,
    /// correctly combining nested `ui` objects so that `ui.resourceUri`
    /// and widget fields like `ui.prefersBorder` coexist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use pmcp::types::mcp_apps::WidgetMeta;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/app.html")
    ///     .with_widget_meta(WidgetMeta::new().prefers_border(true));
    /// // _meta.ui = { "resourceUri": "ui://w/app.html", "prefersBorder": true }
    /// ```
    #[cfg(feature = "mcp-apps")]
    #[allow(clippy::used_underscore_binding, clippy::needless_pass_by_value)]
    pub fn with_widget_meta(mut self, widget: crate::types::mcp_apps::WidgetMeta) -> Self {
        let meta = self._meta.get_or_insert_with(serde_json::Map::new);
        let overlay = widget.to_meta_map();
        crate::types::ui::deep_merge(meta, overlay);
        self
    }

    /// Add a single key-value pair to `_meta`, merging with existing entries.
    ///
    /// If the key already exists and both values are objects, they are
    /// deep-merged. Otherwise the new value replaces the old (last-in wins).
    ///
    /// This is the composable counterpart to [`ToolInfo::with_ui`] --
    /// multiple calls can be chained without overwriting each other's keys.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::types::ToolInfo;
    /// use serde_json::json;
    ///
    /// let tool = ToolInfo::new("my_tool", None, json!({"type": "object"}))
    ///     .with_meta_entry("ui", json!({"resourceUri": "ui://x"}))
    ///     .with_meta_entry("execution", json!({"mode": "async"}));
    /// ```
    #[allow(clippy::used_underscore_binding)]
    pub fn with_meta_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let meta = self._meta.get_or_insert_with(serde_json::Map::new);
        let mut overlay = serde_json::Map::with_capacity(1);
        overlay.insert(key.into(), value);
        crate::types::ui::deep_merge(meta, overlay);
        self
    }

    /// Return a reference to `_meta` if this tool has widget metadata.
    ///
    /// Single-pass check: returns `Some` only when `_meta` contains a
    /// recognised widget key, `None` otherwise.
    #[allow(clippy::used_underscore_binding)]
    pub fn widget_meta(&self) -> Option<&serde_json::Map<String, Value>> {
        self._meta.as_ref().filter(|meta| {
            meta.contains_key("openai/outputTemplate")
                || meta.contains_key(crate::types::ui::META_KEY_UI_RESOURCE_URI)
                || meta.get("ui").and_then(|v| v.get("resourceUri")).is_some()
        })
    }
}

/// List tools request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List tools response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListToolsResult;
///
/// let result = ListToolsResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Available tools
    pub tools: Vec<ToolInfo>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,

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
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
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
    /// **No builder by design.** `ListToolsResult` is built by the dispatcher
    /// from the registered tool set, with no handler seam, so a builder method
    /// here would be public API no server author can reach through normal
    /// configuration — unlike `ListResourcesResult` and `ReadResourceResult`,
    /// which a [`ResourceHandler`](crate::server::ResourceHandler) returns from
    /// `list` and `read` and which therefore do carry builders. The field stays
    /// `pub`, so a caller constructing the struct directly can still set it.
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
    /// section before setting `Public`: it authorizes a shared gateway to serve
    /// this body across authorization contexts.
    ///
    /// **v2 only.** On a `2025-11-25` wire the key is never emitted, and a
    /// value set here is actively STRIPPED (D-11).
    ///
    /// **Why `Option` when the wire says REQUIRED (D-07):** see
    /// [`ttl_ms`](Self::ttl_ms).
    ///
    /// **No builder by design** — see [`ttl_ms`](Self::ttl_ms). Additive under
    /// semver for the same `#[non_exhaustive]` reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<crate::types::caching::CacheScope>,
}

impl ListToolsResult {
    /// Create a new list tools result.
    pub fn new(tools: Vec<ToolInfo>) -> Self {
        Self {
            tools,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments (must match input schema)
    #[serde(default)]
    pub arguments: Value,
    /// Request metadata (e.g., progress token, per-request protocol context).
    ///
    /// # Wire spelling
    ///
    /// The explicit `rename` is load-bearing: the struct-level
    /// `#[serde(rename_all = "camelCase")]` above would otherwise rename this
    /// FIELD to `meta`, which is not the MCP spelling. `alias = "meta"` keeps
    /// ingress compatible with pmcp peers built before Phase 113, which emitted
    /// the renamed spelling. See `src/types/protocol/mod.rs` §
    /// `every_meta_bearing_request_uses_the_spec_spelling_and_accepts_the_legacy_alias`.
    #[serde(
        rename = "_meta",
        alias = "meta",
        skip_serializing_if = "Option::is_none",
        default
    )]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
    /// Task augmentation parameters (experimental MCP Tasks).
    ///
    /// When present, the server creates a task and returns `CreateTaskResult`
    /// instead of `CallToolResult`. Uses `serde_json::Value` to avoid circular
    /// crate dependency (`pmcp-tasks` depends on `pmcp`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task: Option<Value>,
}

impl CallToolRequest {
    /// Create a tool call request.
    ///
    /// `_meta` and `task` default to `None`.
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
            _meta: None,
            task: None,
        }
    }
}

/// Tool call result.
///
/// Supports three-tier response model for MCP Apps:
/// - `content`: Model-focused narration (goes to model, optionally to widget)
/// - `structured_content`: Structured data for both model and widget
/// - `_meta`: Widget-only metadata (never sent to model)
///
/// # `ChatGPT` Apps Example
///
/// ```rust
/// use pmcp::types::CallToolResult;
/// use serde_json::json;
///
/// let result = CallToolResult::new(vec![])
///     .with_structured_content(json!({
///         "boardState": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR",
///         "lastMove": { "from": "e2", "to": "e4" }
///     }))
///     .with_meta(json!({
///         "widgetState": { "selectedSquare": null }
///     }).as_object().unwrap().clone());
/// ```
///
/// # Backward Compatibility
///
/// Use constructors for clean, future-proof initialization:
///
/// ```rust
/// use pmcp::types::{CallToolResult, Content};
///
/// let result = CallToolResult::new(vec![Content::text("Hello")]);
/// assert!(!result.is_error);
///
/// let error = CallToolResult::error(vec![Content::text("Something went wrong")]);
/// assert!(error.is_error);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result (model-focused narration).
    ///
    /// This content is primarily for the model to understand the result.
    /// In `ChatGPT` Apps, this appears as text below the widget.
    #[serde(default)]
    pub content: Vec<Content>,

    /// Whether the tool call represents an error.
    #[serde(default)]
    pub is_error: bool,

    /// Structured data for both model and widget (`ChatGPT` Apps / MCP Apps Extension).
    ///
    /// Use this for data that should be accessible to both the AI model
    /// (for reasoning) and the widget (for display). Examples:
    /// - Game board state (chess position, game score)
    /// - Query results (database rows, search results)
    /// - Form data (user selections, validated input)
    ///
    /// # Era: what shape is allowed
    ///
    /// The 2026-07-28 schema declares this field as `structuredContent?: unknown`
    /// — *"An optional JSON value that represents the structured result of the
    /// tool call. This can be any JSON value (object, array, string, number, boolean, or null)
    /// that conforms to the tool's `outputSchema` if one is defined."*
    /// (`CallToolResult` in the vendored `schema/vendored/core-2026-07-28/schema.ts`.)
    ///
    /// The 2025-11-25 (v1) schema text was narrower on BOTH halves:
    /// `structuredContent?: { [key: string]: unknown }`, and `outputSchema` was
    /// *"Currently restricted to `type: "object"` at the root level"*. v2 lifts
    /// both restrictions.
    ///
    /// [`CallToolResult::structured_value`](Self::structured_value) is the
    /// constructor that names a non-object payload; use it rather than
    /// [`structured`](Self::structured) so the choice is greppable at the call
    /// site.
    ///
    /// # pmcp's v1 permissiveness is FROZEN here, not corrected
    ///
    /// This field has always been `Option<Value>`, and neither native dispatcher
    /// (`ServerCore` in `src/server/core.rs`, the high-level `Server` in
    /// `src/server/mod.rs`) shape-checks the handler's value on the way out — so
    /// pmcp already emits non-object `structuredContent` on **v1** today, which is
    /// more permissive than v1's own spec text allows. Phase 115 decision D-05
    /// freezes v1 behaviour byte-identically, so that over-permissiveness is
    /// FROZEN rather than fixed: tightening v1 to reject scalars would ITSELF be a
    /// v1 wire change, and is forbidden. Do not add a shape guard here "for
    /// correctness" — `tests/structured_tool_output.rs` fences the v1 half on both
    /// dispatchers precisely so a later tightening fails loudly.
    ///
    /// `skip_serializing_if` distinguishes the two absences that matter:
    /// `None` omits the key entirely, while `Some(Value::Null)` emits an explicit
    /// `"structuredContent": null` — a value v2 permits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,

    /// Widget-only metadata (`ChatGPT` Apps / MCP Apps Extension).
    ///
    /// Metadata that goes only to the widget, never to the model.
    /// Use for widget display hints, UI state, and internal widget data.
    /// Examples:
    /// - `widgetState`: Persisted widget state (`ChatGPT` manages this)
    /// - Display hints: colors, animations, layout preferences
    /// - Internal IDs that the model doesn't need
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
}

impl CallToolResult {
    /// Create a new tool result with content.
    pub fn new(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: false,
            structured_content: None,
            _meta: None,
        }
    }

    /// Create an error result.
    pub fn error(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: true,
            structured_content: None,
            _meta: None,
        }
    }

    /// Create a tool-level rejection result.
    ///
    /// An `isError: true` result whose `content` is the model-readable
    /// `message` and whose `structuredContent` is `details` (when present).
    /// This is the envelope the server's `tools/call` dispatch produces for
    /// [`Error::ToolRejected`](crate::Error::ToolRejected) — an application
    /// rejection the caller should correct and retry, NOT a protocol fault.
    pub fn rejected(message: impl Into<String>, details: Option<Value>) -> Self {
        let result = Self::error(vec![Content::text(message.into())]);
        match details {
            Some(details) => result.with_structured_content(details),
            None => result,
        }
    }

    /// Create a structured success result — the success-side counterpart of
    /// [`rejected`](Self::rejected).
    ///
    /// One value, one call, both voices: `structuredContent` carries `value`
    /// verbatim for structured-aware clients, and `content` carries the
    /// canonical JSON serialization of the same value as text, so text-only
    /// clients (older hosts, log pipelines) keep working. Per the MCP spec, a
    /// tool that declares an `outputSchema` SHOULD return `structuredContent`
    /// conforming to it — this constructor is the one-call way to do that
    /// from handlers that own their [`CallToolResult`] envelope.
    ///
    /// Use [`structured_with_text`](Self::structured_with_text) when the
    /// human-readable voice should differ from the raw serialization, and
    /// [`structured_value`](Self::structured_value) when the payload is NOT an
    /// object (a scalar, an array or `null`) — this constructor keeps its
    /// object-shaped intent so every existing call site reads the same way.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use serde_json::json;
    ///
    /// let value = json!({ "rows": [1, 2, 3] });
    /// let result = CallToolResult::structured(value.clone());
    ///
    /// assert!(!result.is_error);
    /// assert_eq!(result.structured_content, Some(value.clone()));
    /// // The text voice round-trips to the same value.
    /// let pmcp::types::Content::Text { text } = &result.content[0] else {
    ///     unreachable!()
    /// };
    /// assert_eq!(serde_json::from_str::<serde_json::Value>(text).unwrap(), value);
    /// ```
    pub fn structured(value: Value) -> Self {
        let text = value.to_string();
        Self::new(vec![Content::text(text)]).with_structured_content(value)
    }

    /// Create a structured success result with a distinct human-readable voice.
    ///
    /// Like [`structured`](Self::structured), but `content` carries `text`
    /// instead of the raw JSON serialization — mirroring the two-voice
    /// separation [`rejected`](Self::rejected) has on the error side.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use serde_json::json;
    ///
    /// let result = CallToolResult::structured_with_text(
    ///     json!({ "matches": 42 }),
    ///     "Found 42 matches.",
    /// );
    /// assert_eq!(result.structured_content, Some(json!({ "matches": 42 })));
    /// ```
    pub fn structured_with_text(value: Value, text: impl Into<String>) -> Self {
        Self::new(vec![Content::text(text.into())]).with_structured_content(value)
    }

    /// Create a structured success result whose payload is NOT a JSON object —
    /// the widening sibling of [`structured`](Self::structured).
    ///
    /// The body is identical to [`structured`](Self::structured); the two differ
    /// only in what they SAY. Reaching for this name is the deliberate, greppable
    /// record at the call site that the payload is a scalar, an array or `null`
    /// rather than the object shape most tools return. [`structured`](Self::structured)
    /// keeps its exact signature and its object-shaped intent, so every existing
    /// call site compiles and behaves identically (Phase 115 decision D-06).
    ///
    /// # Era
    ///
    /// The 2026-07-28 schema declares `structuredContent?: unknown` — *"An
    /// optional JSON value that represents the structured result of the tool call.
    /// This can be any JSON value (object, array, string, number, boolean, or null)
    /// that conforms to the tool's `outputSchema` if one is defined."*
    /// The 2025-11-25 schema text restricted it to
    /// `{ [key: string]: unknown }`. pmcp's v1 wire behaviour is FROZEN as-is
    /// rather than tightened — see the note on
    /// [`structured_content`](Self::structured_content).
    ///
    /// # A declared `outputSchema` still applies (D-04)
    ///
    /// Widening the payload does not weaken the contract: if the tool declares an
    /// `outputSchema`, that schema must DESCRIBE the scalar. `{"type":
    /// "integer"}` accepts `42`; an object-shaped schema such as
    /// `{"type": "object", "required": ["n"]}` does not.
    ///
    /// "Does not accept" here means a `tracing` **warning is logged at emit
    /// time** — `src/server/output_validation.rs` is warn-only on BOTH eras, so a
    /// mismatch never turns the call into an error result and never adds a
    /// production failure mode. The tool call still succeeds and the value still
    /// reaches the wire.
    ///
    /// # `Some(null)` is not `None`
    ///
    /// A `null` payload is a PRESENT value: it serializes as an explicit
    /// `"structuredContent": null`, whereas a result that never set the field
    /// omits the key entirely.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use serde_json::json;
    ///
    /// // A tool whose declared outputSchema is `{"type": "integer"}`.
    /// let result = CallToolResult::structured_value(json!(42));
    ///
    /// assert!(!result.is_error);
    /// assert_eq!(result.structured_content, Some(json!(42)));
    ///
    /// // The text voice carries the same value, for text-only clients.
    /// let pmcp::types::Content::Text { text } = &result.content[0] else {
    ///     unreachable!()
    /// };
    /// assert_eq!(text, "42");
    ///
    /// // A null payload is present, not absent.
    /// let null_result = CallToolResult::structured_value(json!(null));
    /// assert_eq!(null_result.structured_content, Some(json!(null)));
    /// let wire = serde_json::to_string(&null_result).unwrap();
    /// assert!(wire.contains(r#""structuredContent":null"#));
    /// ```
    pub fn structured_value(value: Value) -> Self {
        // Delegates rather than restating `structured`'s body: the two are
        // documented as behaviourally identical, and a copy would let that
        // claim silently rot the first time one of them changed.
        Self::structured(value)
    }

    /// Add structured content for both model and widget.
    pub fn with_structured_content(mut self, content: Value) -> Self {
        self.structured_content = Some(content);
        self
    }

    /// Add widget-only metadata.
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_meta(mut self, meta: serde_json::Map<String, Value>) -> Self {
        self._meta = Some(meta);
        self
    }

    /// Attach related-task metadata under
    /// [`RELATED_TASK_META_KEY`](crate::types::tasks::RELATED_TASK_META_KEY) (SEP-1686).
    ///
    /// This is the server-emit twin of [`CallToolResult::related_task`]: it
    /// records a [`TaskMetadata`](crate::types::tasks::TaskMetadata) into
    /// `_meta` so a client can recover it and
    /// drive [`Client::wait_for_task`](crate::Client::wait_for_task) without
    /// hand-reading `_meta`. Existing `_meta` entries are preserved.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use pmcp::types::tasks::TaskMetadata;
    ///
    /// let meta = TaskMetadata::new("t9").with_poll_interval(1000);
    /// let result = CallToolResult::new(vec![]).with_related_task(meta);
    /// assert_eq!(result.related_task().unwrap().task_id, "t9");
    /// ```
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_related_task(mut self, meta: crate::types::tasks::TaskMetadata) -> Self {
        let map = self._meta.get_or_insert_with(serde_json::Map::new);
        // Serialization of TaskMetadata (a plain struct of String/Option<u64>)
        // is infallible; fall back to Null rather than panicking on the
        // impossible error path.
        let value = serde_json::to_value(meta).unwrap_or(Value::Null);
        map.insert(
            crate::types::tasks::RELATED_TASK_META_KEY.to_string(),
            value,
        );
        self
    }

    /// Read related-task metadata from `_meta` under
    /// [`RELATED_TASK_META_KEY`](crate::types::tasks::RELATED_TASK_META_KEY).
    ///
    /// Returns `Some(TaskMetadata)` when the result carries a well-formed
    /// related-task entry, `None` when `_meta` is absent, the key is missing,
    /// or the value does not deserialize (tamper-tolerant: never panics). The
    /// minimal native shape `{ "taskId": "t9" }` yields `Some` with the poll
    /// fields defaulting to `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    ///
    /// let result = CallToolResult::new(vec![]);
    /// assert!(result.related_task().is_none());
    /// ```
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn related_task(&self) -> Option<crate::types::tasks::TaskMetadata> {
        self._meta
            .as_ref()?
            .get(crate::types::tasks::RELATED_TASK_META_KEY)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Enrich with widget metadata from a [`ToolInfo`] if it has widget meta.
    ///
    /// Sets `structured_content` and `_meta` so widgets can access tool
    /// output data. No-op for non-widget tools. Only clones `_meta` when
    /// the tool actually has widget metadata.
    pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
        if let Some(meta) = info.widget_meta() {
            let enriched = self.with_structured_content(structured_value);
            // Copy all openai/* descriptor keys from the tool's _meta to the
            // CallToolResult._meta so ChatGPT can match the result to its widget.
            let filtered: serde_json::Map<String, Value> = meta
                .iter()
                .filter(|(k, _)| k.starts_with("openai/toolInvocation/"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !filtered.is_empty() {
                enriched.with_meta(filtered)
            } else {
                enriched
            }
        } else {
            self
        }
    }
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)] // MCP protocol fields use underscore prefix (_meta, _task_id)
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_info_serialization() {
        let tool = ToolInfo::new(
            "test-tool",
            Some("A test tool".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
        );

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "test-tool");
        assert_eq!(json["description"], "A test tool");
        assert_eq!(json["inputSchema"]["type"], "object");
    }

    #[test]
    fn test_call_tool_result_basic() {
        let result = CallToolResult::new(vec![Content::Text {
            text: "Move accepted".to_string(),
        }]);

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["content"][0]["text"], "Move accepted");
        assert_eq!(json["isError"], false);
        assert!(json.get("structuredContent").is_none());
        assert!(json.get("_meta").is_none());
    }

    #[test]
    fn test_call_tool_result_with_structured_content() {
        let result = CallToolResult::new(vec![Content::Text {
            text: "Move e2-e4 played".to_string(),
        }])
        .with_structured_content(json!({
            "boardState": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR",
            "lastMove": { "from": "e2", "to": "e4" }
        }));

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json["structuredContent"]["boardState"],
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR"
        );
        assert_eq!(json["structuredContent"]["lastMove"]["from"], "e2");
        assert_eq!(json["structuredContent"]["lastMove"]["to"], "e4");
    }

    #[test]
    fn test_call_tool_result_with_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert("widgetState".to_string(), json!({ "selectedSquare": "e4" }));
        meta.insert("displayHints".to_string(), json!({ "animate": true }));

        let result = CallToolResult::new(vec![]).with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["_meta"]["widgetState"]["selectedSquare"], "e4");
        assert_eq!(json["_meta"]["displayHints"]["animate"], true);
    }

    #[test]
    fn test_call_tool_result_full_three_tier() {
        let mut meta = serde_json::Map::new();
        meta.insert("widgetState".to_string(), json!({ "theme": "dark" }));

        let result = CallToolResult::new(vec![Content::Text {
            text: "Chess game started. White to move.".to_string(),
        }])
        .with_structured_content(json!({
            "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "turn": "white",
            "legalMoves": ["e2e4", "d2d4", "Nf3", "Nc3"]
        }))
        .with_meta(meta);

        let json = serde_json::to_value(&result).unwrap();
        assert!(json["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Chess game started"));
        assert_eq!(json["structuredContent"]["turn"], "white");
        assert_eq!(json["_meta"]["widgetState"]["theme"], "dark");
    }

    #[test]
    fn test_call_tool_result_error() {
        let result = CallToolResult::error(vec![Content::Text {
            text: "Invalid move: e2-e5 is not legal".to_string(),
        }]);

        assert!(result.is_error);
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["isError"], true);
    }

    #[test]
    fn test_call_tool_result_deserialization() {
        let json_str = r#"{
            "content": [{"type": "text", "text": "Move played"}],
            "isError": false,
            "structuredContent": {"position": "e4"},
            "_meta": {"widgetState": {"selected": true}}
        }"#;

        let result: CallToolResult = serde_json::from_str(json_str).unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert!(result.structured_content.is_some());
        assert!(result._meta.is_some());

        let meta_value = result._meta.unwrap();
        assert_eq!(meta_value["widgetState"]["selected"], true);
    }

    #[test]
    fn test_call_tool_request_with_task() {
        let json_str = r#"{"name": "my_tool", "arguments": {}, "task": {"ttl": 60000}}"#;
        let req: CallToolRequest = serde_json::from_str(json_str).unwrap();
        assert!(req.task.is_some());
        assert_eq!(req.task.unwrap()["ttl"], 60000);
    }

    #[test]
    fn test_call_tool_request_without_task_backward_compat() {
        let json_str = r#"{"name": "my_tool", "arguments": {}}"#;
        let req: CallToolRequest = serde_json::from_str(json_str).unwrap();
        assert!(req.task.is_none());
        assert_eq!(req.name, "my_tool");
    }

    #[test]
    fn test_tool_info_with_execution() {
        let mut tool = ToolInfo::new(
            "task-tool",
            Some("A task-enabled tool".to_string()),
            json!({"type": "object"}),
        );
        tool.execution = Some(ToolExecution::new().with_task_support(TaskSupport::Required));

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "task-tool");
        assert_eq!(json["execution"]["taskSupport"], "required");
    }

    #[test]
    fn test_tool_execution_serialization() {
        let exec = ToolExecution::new().with_task_support(TaskSupport::Required);
        let json = serde_json::to_value(&exec).unwrap();
        assert_eq!(json["taskSupport"], "required");

        let exec2 = ToolExecution::new().with_task_support(TaskSupport::Optional);
        let json2 = serde_json::to_value(&exec2).unwrap();
        assert_eq!(json2["taskSupport"], "optional");

        let exec3 = ToolExecution::new().with_task_support(TaskSupport::Forbidden);
        let json3 = serde_json::to_value(&exec3).unwrap();
        assert_eq!(json3["taskSupport"], "forbidden");
    }

    #[test]
    fn test_tool_info_without_execution_omits_field() {
        let tool = ToolInfo::new(
            "normal-tool",
            Some("A normal tool".to_string()),
            json!({"type": "object"}),
        );

        let json = serde_json::to_value(&tool).unwrap();
        assert!(json.get("execution").is_none());
    }

    #[test]
    fn test_tool_info_with_ui_dual_format() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://w/x.html");
        assert_eq!(
            meta.get("ui/resourceUri"),
            Some(&serde_json::Value::String("ui://w/x.html".to_string())),
            "must have legacy flat ui/resourceUri key for Claude Desktop/ChatGPT"
        );
    }

    #[test]
    fn test_tool_info_with_ui_no_openai_keys() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );
        assert_eq!(
            meta.len(),
            2,
            "_meta should have exactly 2 keys (ui + ui/resourceUri)"
        );
    }

    #[test]
    fn test_with_meta_entry_on_empty_meta() {
        let tool = ToolInfo::new("t", None, json!({"type": "object"}))
            .with_meta_entry("ui", json!({"resourceUri": "ui://x"}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
    }

    #[test]
    fn test_with_meta_entry_merges_with_existing() {
        let mut initial = serde_json::Map::new();
        initial.insert("ui".into(), json!({"resourceUri": "ui://x"}));
        let tool = ToolInfo::new("t", None, json!({"type": "object"}));
        let tool = ToolInfo {
            _meta: Some(initial),
            ..tool
        };
        let tool = tool.with_meta_entry("execution", json!({"mode": "async"}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
        assert_eq!(meta["execution"]["mode"], "async");
    }

    #[test]
    fn test_with_meta_entry_deep_merges_nested() {
        let mut initial = serde_json::Map::new();
        initial.insert("ui".into(), json!({"resourceUri": "ui://x"}));
        let tool = ToolInfo::new("t", None, json!({"type": "object"}));
        let tool = ToolInfo {
            _meta: Some(initial),
            ..tool
        };
        let tool = tool.with_meta_entry("ui", json!({"prefersBorder": true}));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://x");
        assert_eq!(meta["ui"]["prefersBorder"], true);
    }

    #[test]
    fn test_with_meta_entry_chained() {
        let tool = ToolInfo::new("t", None, json!({"type": "object"}))
            .with_meta_entry("a", json!(1))
            .with_meta_entry("b", json!(2));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["a"], 1);
        assert_eq!(meta["b"], 2);
    }

    #[test]
    fn test_existing_with_meta_replace_all_unchanged() {
        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://y");
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://y");
        assert!(!meta.contains_key("openai/outputTemplate"));
    }

    #[test]
    #[cfg(feature = "mcp-apps")]
    fn test_with_widget_meta_merges_with_ui() {
        use crate::types::mcp_apps::WidgetMeta;

        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://w/app.html")
            .with_widget_meta(WidgetMeta::new().prefers_border(true).domain("x.com"));
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert!(!meta.contains_key("openai/outputTemplate"));
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui"]["domain"], "x.com");
        assert_eq!(meta["openai/widgetPrefersBorder"], true);
        assert_eq!(meta["openai/widgetDomain"], "x.com");
    }

    #[test]
    #[cfg(feature = "mcp-apps")]
    fn test_with_widget_meta_on_empty_meta() {
        use crate::types::mcp_apps::WidgetMeta;

        let tool = ToolInfo::new("t", None, json!({"type": "object"})).with_widget_meta(
            WidgetMeta::new()
                .resource_uri("ui://w/app.html")
                .prefers_border(true),
        );
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert!(!meta.contains_key("openai/outputTemplate"));
        assert_eq!(meta["openai/widgetPrefersBorder"], true);
    }
}
