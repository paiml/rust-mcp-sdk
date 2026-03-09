//! MCP protocol-specific types.
//!
//! This module contains all the protocol-specific request, response, and
//! notification types defined by the MCP specification.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};

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

/// Implementation information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name (e.g., "mcp-sdk-rust")
    pub name: String,
    /// Implementation version
    pub version: String,
}

/// Initialize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version the client wants to use
    pub protocol_version: String,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Client implementation info
    pub client_info: Implementation,
}

/// Initialize request parameters (legacy name).
pub type InitializeParams = InitializeRequest;

/// Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Pagination cursor.
pub type Cursor = Option<String>;

/// List tools request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List tools params (legacy name).
pub type ListToolsParams = ListToolsRequest;

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

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name (unique identifier)
    pub name: String,
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
    /// Optional metadata (e.g., for UI resource association in MCP Apps Extension)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<serde_json::Map<String, Value>>,
    /// Execution metadata declaring task support level (experimental MCP Tasks).
    ///
    /// Uses `serde_json::Value` to avoid circular crate dependency
    /// (`pmcp-tasks` depends on `pmcp`). Tools set this via
    /// `serde_json::to_value(ToolExecution { .. })`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<Value>,
}

impl ToolInfo {
    /// Create a new `ToolInfo` without metadata or annotations.
    pub fn new(name: impl Into<String>, description: Option<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            description,
            input_schema,
            output_schema: None,
            annotations: None,
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
            description,
            input_schema,
            output_schema: None,
            annotations: Some(annotations),
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
            description,
            input_schema,
            output_schema: None,
            annotations: None,
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
                || meta.contains_key("ui/resourceUri")
                || meta.get("ui").and_then(|v| v.get("resourceUri")).is_some()
        })
    }
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
}

impl ListToolsResult {
    /// Create a new list tools result.
    pub fn new(tools: Vec<ToolInfo>) -> Self {
        Self {
            tools,
            next_cursor: None,
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
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments (must match input schema)
    #[serde(default)]
    pub arguments: Value,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// Tool call parameters (legacy name).
pub type CallToolParams = CallToolRequest;

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
/// For struct initialization syntax, use `..Default::default()` to fill in the new optional fields:
///
/// ```rust
/// use pmcp::types::{CallToolResult, Content};
///
/// let result = CallToolResult {
///     content: vec![Content::Text { text: "Hello".to_string() }],
///     is_error: false,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Enrich with widget metadata from a [`ToolInfo`] if it has widget meta.
    ///
    /// Sets `structured_content` and `_meta` so widgets can access tool
    /// output data. No-op for non-widget tools. Only clones `_meta` when
    /// the tool actually has widget metadata.
    pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
        if let Some(meta) = info.widget_meta() {
            let filtered = crate::types::ui::filter_meta_by_prefix(meta, "openai/toolInvocation/");
            self.with_structured_content(structured_value)
                .with_meta(filtered)
        } else {
            self
        }
    }
}

/// Message content type alias.
pub type MessageContent = Content;

/// Content item in responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    #[serde(rename_all = "camelCase")]
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },
    /// Resource reference
    #[serde(rename_all = "camelCase")]
    Resource {
        /// Resource URI
        uri: String,
        /// Optional resource content
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// MIME type
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        /// Optional metadata for resource content (e.g., widget metadata for MCP Apps)
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Map<String, serde_json::Value>>,
    },
}

/// List prompts request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List prompts params (legacy name).
pub type ListPromptsParams = ListPromptsRequest;

/// Prompt information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    /// Prompt name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt arguments schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Type hint for prompt arguments.
///
/// This is a PMCP extension to the MCP protocol that helps:
/// - MCP clients display appropriate input widgets (number spinner vs text field)
/// - Validate user input before sending to the server
/// - Enable workflow tool chaining with properly typed parameters
/// - Future-proof for when the MCP protocol adds native type support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptArgumentType {
    /// String value (default)
    #[default]
    String,
    /// Floating-point number
    Number,
    /// Integer number
    Integer,
    /// Boolean true/false
    Boolean,
}

impl PromptArgumentType {
    /// Parse a string value according to this type hint.
    /// Returns a properly typed `serde_json::Value`.
    pub fn parse_value(&self, s: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::String => Ok(serde_json::Value::String(s.to_string())),
            Self::Number => s
                .parse::<f64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid number", s)),
            Self::Integer => s
                .parse::<i64>()
                .map(|n| serde_json::json!(n))
                .map_err(|_| format!("'{}' is not a valid integer", s)),
            Self::Boolean => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(serde_json::json!(true)),
                "false" | "0" | "no" => Ok(serde_json::json!(false)),
                _ => Err(format!("'{}' is not a valid boolean (use true/false)", s)),
            },
        }
    }
}

/// Prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
    /// Completion configuration for this argument
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<crate::types::completable::CompletionConfig>,
    /// Type hint for the argument value (PMCP extension).
    ///
    /// When set, the SDK will:
    /// - Validate that string arguments can be parsed to this type
    /// - Convert string arguments to the appropriate JSON type for tool calls
    ///
    /// This field is optional and defaults to "string" behavior if not specified.
    /// MCP clients that don't understand this field will safely ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg_type: Option<PromptArgumentType>,
}

/// List prompts response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListPromptsResult;
///
/// let result = ListPromptsResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts
    pub prompts: Vec<PromptInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListPromptsResult {
    /// Create a new list prompts result.
    pub fn new(prompts: Vec<PromptInfo>) -> Self {
        Self {
            prompts,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Get prompt request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptRequest {
    /// Prompt name
    pub name: String,
    /// Prompt arguments
    #[serde(default)]
    pub arguments: HashMap<String, String>,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// Get prompt params (legacy name).
pub type GetPromptParams = GetPromptRequest;

/// Get prompt result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::GetPromptResult;
///
/// let result = GetPromptResult::new(vec![], Some("A prompt".to_string()));
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
    /// Optional metadata for task-aware workflows (PMCP extension).
    ///
    /// When a workflow prompt is backed by a task, this field contains
    /// task state information (`task_id`, status, step plan) that
    /// task-aware MCP clients can use for structured continuation.
    /// Omitted from serialized JSON when `None`.
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl GetPromptResult {
    /// Create a new get prompt result.
    pub fn new(messages: Vec<PromptMessage>, description: Option<String>) -> Self {
        Self {
            description,
            messages,
            _meta: None,
        }
    }

    /// Add metadata to the prompt result.
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    pub fn with_meta(mut self, meta: serde_json::Map<String, serde_json::Value>) -> Self {
        self._meta = Some(meta);
        self
    }
}

/// Message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: MessageContent,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// System message
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
        }
    }
}

/// List resources request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// List resources params (legacy name).
pub type ListResourcesParams = ListResourcesRequest;

/// Resource information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional metadata (e.g., widget descriptor keys for `ChatGPT` MCP Apps)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

/// List resources response.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourcesResult;
///
/// let result = ListResourcesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources
    pub resources: Vec<ResourceInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListResourcesResult {
    /// Create a new list resources result.
    pub fn new(resources: Vec<ResourceInfo>) -> Self {
        Self {
            resources,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Read resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceRequest {
    /// Resource URI
    pub uri: String,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
}

/// Read resource params (legacy name).
pub type ReadResourceParams = ReadResourceRequest;

/// List resource templates request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Resource template.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    /// Template URI pattern
    pub uri_template: String,
    /// Template name
    pub name: String,
    /// Template description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type for resources created from this template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List resource templates result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ListResourceTemplatesResult;
///
/// let result = ListResourceTemplatesResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourceTemplatesResult {
    /// Available resource templates
    pub resource_templates: Vec<ResourceTemplate>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListResourceTemplatesResult {
    /// Create a new list resource templates result.
    pub fn new(resource_templates: Vec<ResourceTemplate>) -> Self {
        Self {
            resource_templates,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Subscribe to resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    /// Resource URI to subscribe to
    pub uri: String,
}

/// Unsubscribe from resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeRequest {
    /// Resource URI to unsubscribe from
    pub uri: String,
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

/// Completion result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    /// Completion options
    pub completion: CompletionResult,
}

/// Completion result.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Logging level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    /// Debug messages
    Debug,
    /// Informational messages
    Info,
    /// Warnings
    Warning,
    /// Errors
    Error,
    /// Critical errors
    Critical,
}

/// Read resource result.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::ReadResourceResult;
///
/// let result = ReadResourceResult::new(vec![]);
/// ```
///
/// Within the same crate, struct literal syntax with `..Default::default()` also works.
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Resource contents
    pub contents: Vec<Content>,
}

impl ReadResourceResult {
    /// Create a new read resource result.
    pub fn new(contents: Vec<Content>) -> Self {
        Self { contents }
    }
}

/// Model preferences for sampling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreferences {
    /// Hints for model selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    /// Cost priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f64>,
    /// Speed priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f64>,
    /// Intelligence priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f64>,
}

/// Model hint for sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHint {
    /// Model name/identifier hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Progress notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    /// Progress token from the original request
    pub progress_token: ProgressToken,
    /// Current progress value (must increase with each notification)
    ///
    /// This can represent percentage (0-100), count, or any increasing metric.
    pub progress: f64,
    /// Optional total value for the operation
    ///
    /// When combined with `progress`, allows expressing "5 of 10 items processed".
    /// Both `progress` and `total` may be floating point values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Optional human-readable progress message
    ///
    /// Should provide relevant context about the current operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ProgressNotification {
    /// Create a new progress notification with no total value.
    ///
    /// Convenience constructor to reduce boilerplate when the total is unknown.
    pub fn new(progress_token: ProgressToken, progress: f64, message: Option<String>) -> Self {
        Self {
            progress_token,
            progress,
            total: None,
            message,
        }
    }
}

/// Progress (legacy alias).
pub type Progress = ProgressNotification;

/// Progress token type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProgressToken {
    /// String token
    String(String),
    /// Numeric token
    Number(i64),
}

/// Request metadata that can be attached to any request.
///
/// This follows the MCP protocol's `_meta` field specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMeta {
    /// Progress token for out-of-band progress notifications.
    ///
    /// If specified, the caller is requesting progress notifications for this request.
    /// The value is an opaque token that will be attached to subsequent progress notifications.
    /// The receiver is not obligated to provide these notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_token: Option<ProgressToken>,

    /// Task ID for workflow continuation (PMCP extension).
    ///
    /// When present on a `tools/call` request, the server records the tool
    /// result against the referenced workflow task after normal execution.
    /// The tool call itself proceeds as normal; the recording is best-effort.
    #[serde(skip_serializing_if = "Option::is_none", rename = "_task_id")]
    #[allow(clippy::pub_underscore_fields)]
    pub _task_id: Option<String>,
}

/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientRequest {
    /// Initialize the connection
    #[serde(rename = "initialize")]
    Initialize(InitializeParams),
    /// List available tools
    #[serde(rename = "tools/list")]
    ListTools(ListToolsParams),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(CallToolParams),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(ListPromptsParams),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(GetPromptParams),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(ListResourcesParams),
    /// List resource templates
    #[serde(rename = "resources/templates/list")]
    ListResourceTemplates(ListResourceTemplatesRequest),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(ReadResourceParams),
    /// Subscribe to resource updates
    #[serde(rename = "resources/subscribe")]
    Subscribe(SubscribeRequest),
    /// Unsubscribe from resource updates
    #[serde(rename = "resources/unsubscribe")]
    Unsubscribe(UnsubscribeRequest),
    /// Request completion
    #[serde(rename = "completion/complete")]
    Complete(CompleteRequest),
    /// Set logging level
    #[serde(rename = "logging/setLevel")]
    SetLoggingLevel {
        /// Logging level to set
        level: LoggingLevel,
    },
    /// Ping request
    #[serde(rename = "ping")]
    Ping,
    /// Create message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(CreateMessageRequest),
    /// Response to elicitation request
    #[serde(rename = "elicitation/response")]
    ElicitInputResponse(crate::types::elicitation::ElicitInputResponse),
    /// Get task status (experimental MCP Tasks).
    #[serde(rename = "tasks/get")]
    TasksGet(Value),
    /// Get task result (experimental MCP Tasks).
    #[serde(rename = "tasks/result")]
    TasksResult(Value),
    /// List tasks (experimental MCP Tasks).
    #[serde(rename = "tasks/list")]
    TasksList(Value),
    /// Cancel a task (experimental MCP Tasks).
    #[serde(rename = "tasks/cancel")]
    TasksCancel(Value),
}

/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to create a message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<CreateMessageParams>),
    /// List roots request
    #[serde(rename = "roots/list")]
    ListRoots,
    /// Elicit input from user
    #[serde(rename = "elicitation/elicitInput")]
    ElicitInput(Box<crate::types::elicitation::ElicitInputRequest>),
}

/// Create message parameters (for server requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageParams {
    /// Messages to sample from
    pub messages: Vec<SamplingMessage>,
    /// Optional model preferences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Include context from MCP
    #[serde(default)]
    pub include_context: IncludeContext,
    /// Temperature (0-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Additional model-specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Create message request (for client requests).
pub type CreateMessageRequest = CreateMessageParams;

/// Create message result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    /// The content generated by the model
    pub content: Content,
    /// The model used for generation
    pub model: String,
    /// Token usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Input tokens used
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// Sampling message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: Content,
}

/// Context to include in sampling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IncludeContext {
    /// Include all context
    All,
    /// Include no context
    #[default]
    None,
    /// Include specific context types
    ThisServerOnly,
}

/// Client notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientNotification {
    /// Notification that client has been initialized
    #[serde(rename = "notifications/initialized")]
    Initialized,
    /// Notification that roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Notification that a request was cancelled
    #[serde(rename = "notifications/cancelled")]
    Cancelled(CancelledParams),
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
}

/// Cancelled notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotification {
    /// The request ID that was cancelled
    pub request_id: crate::types::RequestId,
    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Cancelled params (legacy alias).
pub type CancelledParams = CancelledNotification;

/// Server notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerNotification {
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
    /// Tools have changed
    #[serde(rename = "notifications/tools/list_changed")]
    ToolsChanged,
    /// Prompts have changed
    #[serde(rename = "notifications/prompts/list_changed")]
    PromptsChanged,
    /// Resources have changed
    #[serde(rename = "notifications/resources/list_changed")]
    ResourcesChanged,
    /// Roots have changed
    #[serde(rename = "notifications/roots/list_changed")]
    RootsListChanged,
    /// Resource was updated
    #[serde(rename = "notifications/resources/updated")]
    ResourceUpdated(ResourceUpdatedParams),
    /// Log message
    #[serde(rename = "notifications/message")]
    LogMessage(LogMessageParams),
}

/// Resource updated notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdatedParams {
    /// Resource URI that was updated
    pub uri: String,
}

/// Log message notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogMessageParams {
    /// Log level
    pub level: LogLevel,
    /// Logger name/category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// Log message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
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

/// Combined notification types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Notification {
    /// Client notification
    Client(ClientNotification),
    /// Server notification  
    Server(ServerNotification),
    /// Progress notification
    Progress(ProgressNotification),
    /// Cancelled notification
    Cancelled(CancelledNotification),
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warning,
    /// Error level
    Error,
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)] // MCP protocol fields use underscore prefix (_meta, _task_id)
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(ListToolsParams::default());
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/list");
    }

    #[test]
    fn serialize_content() {
        let content = Content::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

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
    fn test_all_notification_types() {
        let progress = ServerNotification::Progress(ProgressNotification {
            progress_token: ProgressToken::String("token123".to_string()),
            progress: 50.0,
            total: None,
            message: Some("Processing...".to_string()),
        });
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["method"], "notifications/progress");

        let tools_changed = ServerNotification::ToolsChanged;
        let json = serde_json::to_value(&tools_changed).unwrap();
        assert_eq!(json["method"], "notifications/tools/list_changed");

        let prompts_changed = ServerNotification::PromptsChanged;
        let json = serde_json::to_value(&prompts_changed).unwrap();
        assert_eq!(json["method"], "notifications/prompts/list_changed");

        let resources_changed = ServerNotification::ResourcesChanged;
        let json = serde_json::to_value(&resources_changed).unwrap();
        assert_eq!(json["method"], "notifications/resources/list_changed");

        let roots_changed = ServerNotification::RootsListChanged;
        let json = serde_json::to_value(&roots_changed).unwrap();
        assert_eq!(json["method"], "notifications/roots/list_changed");

        let resource_updated = ServerNotification::ResourceUpdated(ResourceUpdatedParams {
            uri: "file://test.txt".to_string(),
        });
        let json = serde_json::to_value(&resource_updated).unwrap();
        assert_eq!(json["method"], "notifications/resources/updated");

        let log_msg = ServerNotification::LogMessage(LogMessageParams {
            level: LogLevel::Info,
            logger: None,
            message: "Test log message".to_string(),
            data: Some(json!({"extra": "data"})),
        });
        let json = serde_json::to_value(&log_msg).unwrap();
        assert_eq!(json["method"], "notifications/message");
    }

    #[test]
    fn test_resource_types() {
        let resource = ResourceInfo {
            uri: "file://test.txt".to_string(),
            name: "test.txt".to_string(),
            description: Some("Test file".to_string()),
            mime_type: Some("text/plain".to_string()),
            meta: None,
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["uri"], "file://test.txt");
        assert_eq!(json["name"], "test.txt");
        assert_eq!(json["description"], "Test file");
        assert_eq!(json["mimeType"], "text/plain");
    }

    #[test]
    fn test_prompt_types() {
        let prompt = PromptInfo {
            name: "test_prompt".to_string(),
            description: Some("A test prompt".to_string()),
            arguments: Some(vec![PromptArgument {
                name: "arg1".to_string(),
                description: Some("First argument".to_string()),
                required: true,
                completion: None,
                arg_type: None,
            }]),
        };

        let json = serde_json::to_value(&prompt).unwrap();
        assert_eq!(json["name"], "test_prompt");
        assert_eq!(json["arguments"][0]["name"], "arg1");
        assert_eq!(json["arguments"][0]["required"], true);
    }

    #[test]
    fn test_log_levels() {
        assert_eq!(serde_json::to_value(LogLevel::Debug).unwrap(), "debug");
        assert_eq!(serde_json::to_value(LogLevel::Info).unwrap(), "info");
        assert_eq!(serde_json::to_value(LogLevel::Warning).unwrap(), "warning");
        assert_eq!(serde_json::to_value(LogLevel::Error).unwrap(), "error");
    }

    #[test]
    fn test_cancelled_notification() {
        use crate::types::RequestId;

        let cancelled = CancelledNotification {
            request_id: RequestId::Number(123),
            reason: Some("User cancelled".to_string()),
        };

        let json = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(json["requestId"], 123);
        assert_eq!(json["reason"], "User cancelled");
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
        // Demonstrates the full three-tier response model for `ChatGPT` Apps
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

        // Content: narration for model
        assert!(json["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Chess game started"));

        // Structured content: data for model + widget
        assert_eq!(json["structuredContent"]["turn"], "white");

        // Meta: widget-only
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
    #[allow(clippy::used_underscore_binding)] // _meta is valid MCP protocol field name
    fn test_call_tool_result_deserialization() {
        // Test deserializing a `ChatGPT` Apps-style response
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
    fn test_task_client_request_variants() {
        // TasksGet
        let json_str = r#"{"method": "tasks/get", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksGet(_)));

        // TasksResult
        let json_str = r#"{"method": "tasks/result", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksResult(_)));

        // TasksList
        let json_str = r#"{"method": "tasks/list", "params": {}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksList(_)));

        // TasksCancel
        let json_str = r#"{"method": "tasks/cancel", "params": {"taskId": "abc"}}"#;
        let req: ClientRequest = serde_json::from_str(json_str).unwrap();
        assert!(matches!(req, ClientRequest::TasksCancel(_)));
    }

    #[test]
    fn test_task_client_request_roundtrip() {
        // Verify serialization round-trip for task variants
        let req = ClientRequest::TasksGet(json!({"taskId": "t-123"}));
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tasks/get");
        assert_eq!(json["params"]["taskId"], "t-123");

        let deserialized: ClientRequest = serde_json::from_value(json).unwrap();
        assert!(matches!(deserialized, ClientRequest::TasksGet(_)));
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
        // Existing JSON without the task field still works
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
        tool.execution = Some(json!({"taskSupport": "required"}));

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "task-tool");
        assert_eq!(json["execution"]["taskSupport"], "required");
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
    fn get_prompt_result_without_meta_omits_field() {
        let result = GetPromptResult {
            description: Some("Test".to_string()),
            messages: vec![],
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("_meta").is_none(),
            "_meta should be omitted when None"
        );
        assert_eq!(json["description"], "Test");
    }

    #[test]
    fn get_prompt_result_with_meta_includes_field() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-123".to_string()),
        );

        let result = GetPromptResult {
            description: None,
            messages: vec![],
            _meta: Some(meta),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("_meta").is_some(), "_meta should be present");
        assert_eq!(json["_meta"]["taskId"], "task-123");
    }

    #[test]
    fn get_prompt_result_deserialize_without_meta_backward_compat() {
        let json_str = r#"{"messages": [], "description": "Test"}"#;
        let result: GetPromptResult = serde_json::from_str(json_str).unwrap();
        assert!(
            result._meta.is_none(),
            "Missing _meta should deserialize as None"
        );
        assert_eq!(result.description.as_deref(), Some("Test"));
    }

    #[test]
    fn request_meta_task_id_serializes_as_underscore() {
        let meta = RequestMeta {
            progress_token: None,
            _task_id: Some("abc".to_string()),
        };
        let json = serde_json::to_value(&meta).unwrap();
        // Must serialize as "_task_id" (underscore-separated), not "_taskId" (camelCase)
        assert_eq!(json["_task_id"], "abc");
        assert!(
            json.get("_taskId").is_none(),
            "_task_id must not be camelCased"
        );
    }

    #[test]
    fn request_meta_task_id_omitted_when_none() {
        let meta = RequestMeta {
            progress_token: None,
            _task_id: None,
        };
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

    #[test]
    fn test_tool_info_with_ui_nested_format() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();

        // Must use nested format: {"ui": {"resourceUri": "..."}}
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://w/x.html");

        // Must also have legacy flat "ui/resourceUri" key for backward compat
        assert_eq!(
            meta.get("ui/resourceUri"),
            Some(&json!("ui://w/x.html")),
            "must have legacy flat ui/resourceUri key"
        );
    }

    #[test]
    fn test_tool_info_with_ui_openai_output_template() {
        let tool = ToolInfo::with_ui("my_tool", None, json!({"type": "object"}), "ui://w/x.html");

        let meta = tool._meta.as_ref().unwrap();

        // Must have openai/outputTemplate as ChatGPT alias
        assert_eq!(
            meta.get("openai/outputTemplate").unwrap(),
            &serde_json::Value::String("ui://w/x.html".to_string())
        );
    }

    #[test]
    fn get_prompt_result_serde_round_trip_with_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "taskId".to_string(),
            serde_json::Value::String("task-456".to_string()),
        );
        meta.insert(
            "status".to_string(),
            serde_json::Value::String("working".to_string()),
        );

        let result = GetPromptResult {
            description: Some("Workflow result".to_string()),
            messages: vec![PromptMessage {
                role: Role::User,
                content: Content::Text {
                    text: "Hello".to_string(),
                },
            }],
            _meta: Some(meta),
        };

        let json = serde_json::to_value(&result).unwrap();
        let round_trip: GetPromptResult = serde_json::from_value(json).unwrap();

        assert_eq!(round_trip.description.as_deref(), Some("Workflow result"));
        assert_eq!(round_trip.messages.len(), 1);
        assert!(round_trip._meta.is_some());
        let rt_meta = round_trip._meta.unwrap();
        assert_eq!(
            rt_meta.get("taskId").unwrap(),
            &serde_json::Value::String("task-456".to_string())
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
        // Ensure the existing with_ui constructor still works (replace-all semantics)
        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://y");
        let meta = tool._meta.unwrap();
        assert_eq!(meta["ui"]["resourceUri"], "ui://y");
        assert!(meta.contains_key("openai/outputTemplate"));
    }

    #[test]
    #[cfg(feature = "mcp-apps")]
    fn test_with_widget_meta_merges_with_ui() {
        use crate::types::mcp_apps::WidgetMeta;

        let tool = ToolInfo::with_ui("t", None, json!({"type": "object"}), "ui://w/app.html")
            .with_widget_meta(WidgetMeta::new().prefers_border(true).domain("x.com"));
        let meta = tool._meta.unwrap();

        // URI keys preserved from with_ui
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert_eq!(meta["openai/outputTemplate"], "ui://w/app.html");

        // Widget fields deep-merged into the same ui object
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui"]["domain"], "x.com");

        // Flat widget keys also present
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

        // All keys produced from WidgetMeta alone
        assert_eq!(meta["ui"]["resourceUri"], "ui://w/app.html");
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui/resourceUri"], "ui://w/app.html");
        assert_eq!(meta["openai/outputTemplate"], "ui://w/app.html");
        assert_eq!(meta["openai/widgetPrefersBorder"], true);
    }

    #[test]
    fn test_content_resource_meta_serialization() {
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(
            "widgetDescription".to_string(),
            serde_json::Value::String("A chess board widget".to_string()),
        );
        let content = Content::Resource {
            uri: "ui://chess/board".to_string(),
            text: Some("<html>chess</html>".to_string()),
            mime_type: Some("text/html;profile=mcp-app".to_string()),
            meta: Some(meta_map),
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["_meta"]["widgetDescription"], "A chess board widget");
        assert_eq!(json["uri"], "ui://chess/board");
    }

    #[test]
    fn test_content_resource_no_meta_serialization() {
        let content = Content::Resource {
            uri: "file:///test.txt".to_string(),
            text: Some("hello".to_string()),
            mime_type: Some("text/plain".to_string()),
            meta: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert!(json.get("_meta").is_none());
        assert_eq!(json["uri"], "file:///test.txt");
    }

    #[test]
    fn test_content_resource_meta_deserialization() {
        let json = json!({
            "type": "resource",
            "uri": "ui://widget",
            "text": "<html></html>",
            "mimeType": "text/html",
            "_meta": {
                "widgetDescription": "test widget",
                "csp": { "connectDomains": ["https://api.example.com"] }
            }
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "ui://widget");
                let meta = meta.unwrap();
                assert_eq!(meta["widgetDescription"], "test widget");
                assert!(meta.contains_key("csp"));
            },
            _ => panic!("Expected Content::Resource"),
        }
    }

    #[test]
    fn test_content_resource_backward_compat() {
        let json = json!({
            "type": "resource",
            "uri": "file:///old.txt",
            "text": "old content",
            "mimeType": "text/plain"
        });
        let content: Content = serde_json::from_value(json).unwrap();
        match content {
            Content::Resource { uri, meta, .. } => {
                assert_eq!(uri, "file:///old.txt");
                assert!(meta.is_none());
            },
            _ => panic!("Expected Content::Resource"),
        }
    }
}
