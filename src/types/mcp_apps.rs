// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! MCP Apps Extension types for interactive UI support.
//!
//! This module provides types for building interactive widgets that work across
//! multiple MCP host platforms:
//!
//! - **ChatGPT Apps** (OpenAI Apps SDK) - Uses `text/html+skybridge` and `window.openai`
//! - **MCP Apps (SEP-1865)** - Standard MCP extension using `text/html+mcp`
//! - **MCP-UI** - Community standard supporting HTML, URLs, and Remote DOM
//!
//! # Architecture
//!
//! The types in this module follow a layered architecture:
//!
//! 1. **Core Types** - Host-agnostic abstractions (`UIAction`, `WidgetCSP`, etc.)
//! 2. **Platform-Specific Metadata** - ChatGPT-specific fields (`ChatGptToolMeta`, etc.)
//! 3. **Adapters** - Transform core types for specific hosts (see `server::mcp_apps`)
//!
//! # Example
//!
//! ```rust
//! use pmcp::types::mcp_apps::{WidgetCSP, ChatGptToolMeta, ToolVisibility};
//!
//! // Configure Content Security Policy
//! let csp = WidgetCSP::new()
//!     .connect("https://api.example.com")
//!     .resources("https://cdn.example.com");
//!
//! // Configure ChatGPT-specific tool metadata
//! let meta = ChatGptToolMeta::new()
//!     .output_template("ui://widget/my-widget.html")
//!     .invoking("Loading...")
//!     .invoked("Ready!")
//!     .widget_accessible(true);
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `mcp-apps` feature:
//!
//! ```toml
//! [dependencies]
//! pmcp = { version = "1.9", features = ["mcp-apps"] }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;

// =============================================================================
// Content Security Policy
// =============================================================================

/// Widget Content Security Policy for ChatGPT Apps.
///
/// Defines which domains the widget can interact with. ChatGPT enforces these
/// restrictions in the sandboxed iframe environment.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::WidgetCSP;
///
/// let csp = WidgetCSP::new()
///     .connect("https://api.example.com")      // Allow fetch/XHR
///     .resources("https://cdn.example.com")    // Allow images, scripts
///     .resources("https://*.oaistatic.com")    // OpenAI's CDN
///     .redirect("https://checkout.example.com"); // Allow external redirects
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetCSP {
    /// Domains the widget can fetch from (connect-src).
    ///
    /// Use for API endpoints that the widget calls directly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connect_domains: Vec<String>,

    /// Domains for static assets like images, fonts, and scripts.
    ///
    /// Use for CDN-hosted widget bundles and media.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_domains: Vec<String>,

    /// Domains for external redirects via `openExternal`.
    ///
    /// ChatGPT appends a `redirectUrl` query parameter to help external
    /// flows return to the conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_domains: Option<Vec<String>>,

    /// Domains allowed for iframes within the widget.
    ///
    /// **Warning:** Using `frame_domains` is discouraged and triggers
    /// extra scrutiny during ChatGPT App review. Only use when embedding
    /// iframes is essential to your experience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,

    /// Base URI domains for the widget.
    ///
    /// Maps to the spec's `McpUiResourceCsp.baseUriDomains` field.
    /// Restricts which domains can be used as the base URI for relative URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_uri_domains: Option<Vec<String>>,
}

impl WidgetCSP {
    /// Create an empty CSP configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connect domain (for fetch/XHR requests).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::mcp_apps::WidgetCSP;
    ///
    /// let csp = WidgetCSP::new()
    ///     .connect("https://api.example.com")
    ///     .connect("https://api2.example.com");
    /// ```
    pub fn connect(mut self, domain: impl Into<String>) -> Self {
        self.connect_domains.push(domain.into());
        self
    }

    /// Add a resource domain (for images, scripts, fonts, etc.).
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::mcp_apps::WidgetCSP;
    ///
    /// let csp = WidgetCSP::new()
    ///     .resources("https://cdn.example.com")
    ///     .resources("https://*.cloudfront.net");
    /// ```
    pub fn resources(mut self, domain: impl Into<String>) -> Self {
        self.resource_domains.push(domain.into());
        self
    }

    /// Add a redirect domain (for `openExternal` navigation).
    pub fn redirect(mut self, domain: impl Into<String>) -> Self {
        self.redirect_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Add a frame domain (for iframes - use with caution).
    ///
    /// **Warning:** Widgets with `frame_domains` are subject to higher
    /// scrutiny during review and may be rejected.
    pub fn frame(mut self, domain: impl Into<String>) -> Self {
        self.frame_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Add a base URI domain.
    ///
    /// Maps to the spec's `McpUiResourceCsp.baseUriDomains` field.
    pub fn base_uri(mut self, domain: impl Into<String>) -> Self {
        self.base_uri_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Serialize as a spec-compatible JSON map (ext-apps `McpUiResourceCsp` shape).
    ///
    /// Uses camelCase field names per spec. `redirect_domains` is excluded
    /// because it is ChatGPT-specific and not part of the ext-apps spec.
    pub fn to_spec_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut csp_obj = serde_json::Map::with_capacity(4);
        if !self.connect_domains.is_empty() {
            csp_obj.insert(
                "connectDomains".into(),
                serde_json::json!(self.connect_domains),
            );
        }
        if !self.resource_domains.is_empty() {
            csp_obj.insert(
                "resourceDomains".into(),
                serde_json::json!(self.resource_domains),
            );
        }
        if let Some(frames) = &self.frame_domains {
            if !frames.is_empty() {
                csp_obj.insert("frameDomains".into(), serde_json::json!(frames));
            }
        }
        if let Some(base_uris) = &self.base_uri_domains {
            if !base_uris.is_empty() {
                csp_obj.insert("baseUriDomains".into(), serde_json::json!(base_uris));
            }
        }
        csp_obj
    }

    /// Check if the CSP has any domains configured.
    pub fn is_empty(&self) -> bool {
        self.connect_domains.is_empty()
            && self.resource_domains.is_empty()
            && self.redirect_domains.as_ref().is_none_or(Vec::is_empty)
            && self.frame_domains.as_ref().is_none_or(Vec::is_empty)
            && self.base_uri_domains.as_ref().is_none_or(Vec::is_empty)
    }
}

// =============================================================================
// Widget Metadata (Resource-level)
// =============================================================================

/// Widget configuration metadata for ChatGPT Apps.
///
/// These fields are added to the resource's `_meta` field and control
/// how ChatGPT renders and configures the widget.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::{WidgetMeta, WidgetCSP};
///
/// // Complete _meta in one builder — no separate build_meta_map needed
/// let meta = WidgetMeta::new()
///     .resource_uri("ui://chess/board.html")
///     .prefers_border(true)
///     .domain("https://chatgpt.com")
///     .description("Interactive chess board")
///     .csp(WidgetCSP::new().connect("https://api.chess.com"))
///     .to_meta_map();
/// // => { "ui": { "resourceUri": "...", "prefersBorder": true, "domain": "...", "csp": {...} },
/// //      "ui/resourceUri": "...", "openai/outputTemplate": "...",
/// //      "openai/widgetPrefersBorder": true, "openai/widgetDomain": "...", ... }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetMeta {
    /// UI resource URI for this widget.
    ///
    /// When set, `to_meta_map()` emits the full triple-key format
    /// (`ui.resourceUri`, `ui/resourceUri`, `openai/outputTemplate`),
    /// eliminating the need for a separate `build_meta_map` call.
    #[serde(skip)]
    pub resource_uri: Option<String>,

    /// Whether widget prefers a border around it.
    #[serde(
        rename = "openai/widgetPrefersBorder",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefers_border: Option<bool>,

    /// Dedicated origin for the widget.
    ///
    /// When set, ChatGPT renders the widget under
    /// `<domain>.web-sandbox.oaiusercontent.com`, enabling features
    /// like API key allowlists and fullscreen punch-out.
    #[serde(
        rename = "openai/widgetDomain",
        skip_serializing_if = "Option::is_none"
    )]
    pub domain: Option<String>,

    /// Content Security Policy configuration.
    #[serde(rename = "openai/widgetCSP", skip_serializing_if = "Option::is_none")]
    pub csp: Option<WidgetCSP>,

    /// Widget self-description.
    ///
    /// Reduces redundant text beneath the widget by letting it describe itself.
    #[serde(
        rename = "openai/widgetDescription",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
}

impl WidgetMeta {
    /// Create empty widget metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the UI resource URI.
    ///
    /// When set, `to_meta_map()` emits the full triple-key format matching
    /// the official `registerAppTool` behavior:
    /// - `ui.resourceUri` (nested)
    /// - `ui/resourceUri` (legacy flat)
    /// - `openai/outputTemplate` (ChatGPT alias)
    ///
    /// This eliminates the need for a separate `build_meta_map` + merge step.
    pub fn resource_uri(mut self, uri: impl Into<String>) -> Self {
        self.resource_uri = Some(uri.into());
        self
    }

    /// Set border preference.
    pub fn prefers_border(mut self, prefers: bool) -> Self {
        self.prefers_border = Some(prefers);
        self
    }

    /// Set widget domain for dedicated origin.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set widget description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set Content Security Policy.
    pub fn csp(mut self, csp: WidgetCSP) -> Self {
        self.csp = Some(csp);
        self
    }

    /// Convert to a `serde_json::Map` for merging into resource `_meta`.
    ///
    /// Produces both flat `openai/*` keys (via serde) and a nested `"ui"` object
    /// containing MCP standard equivalents (`prefersBorder`, `domain`, `csp`).
    ///
    /// The nested `ui.csp` uses spec field names (`connectDomains`, `resourceDomains`,
    /// `frameDomains`, `baseUriDomains`). Note that `redirect_domains` is ChatGPT-specific
    /// and only appears in the flat `openai/widgetCSP` key, not in nested `ui.csp`.
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = match serde_json::to_value(self).ok() {
            Some(serde_json::Value::Object(m)) => m,
            _ => serde_json::Map::new(),
        };
        // Dual-emit: add nested ui object for MCP standard fields
        let mut ui_obj = serde_json::Map::new();
        if let Some(uri) = &self.resource_uri {
            crate::types::ui::emit_resource_uri_keys(&mut map, &mut ui_obj, uri);
        }
        if let Some(prefers) = self.prefers_border {
            ui_obj.insert(
                "prefersBorder".to_string(),
                serde_json::Value::Bool(prefers),
            );
        }
        if let Some(domain) = &self.domain {
            ui_obj.insert(
                "domain".to_string(),
                serde_json::Value::String(domain.clone()),
            );
        }
        if let Some(csp) = &self.csp {
            let csp_obj = csp.to_spec_map();
            if !csp_obj.is_empty() {
                ui_obj.insert("csp".to_string(), serde_json::Value::Object(csp_obj));
            }
        }
        if !ui_obj.is_empty() {
            map.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
        }
        map
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.resource_uri.is_none()
            && self.prefers_border.is_none()
            && self.domain.is_none()
            && self.csp.is_none()
            && self.description.is_none()
    }
}

// =============================================================================
// Tool Visibility
// =============================================================================

/// Tool visibility setting for ChatGPT Apps.
///
/// Controls whether a tool is visible to the model or only callable from widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ToolVisibility {
    /// Tool is visible to the model (default).
    ///
    /// The model can decide to call this tool based on user prompts.
    #[default]
    Public,

    /// Tool is hidden from the model.
    ///
    /// Only callable from widgets via `window.openai.callTool()`.
    /// Useful for internal widget operations that shouldn't be
    /// triggered by user prompts.
    Private,

    /// Tool is visible to the model only, not callable from widgets.
    ///
    /// The model can call this tool but widgets cannot invoke it via UI actions.
    ModelOnly,
}

impl ToolVisibility {
    /// Convert to a spec-compatible visibility array.
    ///
    /// Returns the visibility as an array of audience strings matching
    /// the ext-apps spec format:
    /// - `Public` -> `["model", "app"]`
    /// - `Private` -> `["app"]`
    /// - `ModelOnly` -> `["model"]`
    pub fn to_visibility_array(&self) -> &'static [&'static str] {
        match self {
            Self::Public => &["model", "app"],
            Self::Private => &["app"],
            Self::ModelOnly => &["model"],
        }
    }
}

// =============================================================================
// Tool Metadata (Tool-level)
// =============================================================================

/// ChatGPT-specific tool metadata.
///
/// These fields are added to the tool's `_meta` field and control
/// how ChatGPT presents and executes the tool.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::{ChatGptToolMeta, ToolVisibility};
///
/// let meta = ChatGptToolMeta::new()
///     .output_template("ui://widget/kanban-board.html")
///     .invoking("Preparing the board...")
///     .invoked("Board ready!")
///     .widget_accessible(true)
///     .visibility(ToolVisibility::Public);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct ChatGptToolMeta {
    /// UI template URI that ChatGPT loads when this tool is called.
    ///
    /// Must point to a resource with `text/html;profile=mcp-app` MIME type.
    #[serde(
        rename = "openai/outputTemplate",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_template: Option<String>,

    /// Message shown while the tool is running.
    ///
    /// Example: "Loading data from server..."
    #[serde(
        rename = "openai/toolInvocation/invoking",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoking: Option<String>,

    /// Message shown when the tool completes.
    ///
    /// Example: "Data loaded successfully!"
    #[serde(
        rename = "openai/toolInvocation/invoked",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoked: Option<String>,

    /// Whether the widget can call this tool via `window.openai.callTool()`.
    ///
    /// Set to `true` to enable widget-initiated tool calls (e.g., refresh button).
    #[serde(
        rename = "openai/widgetAccessible",
        skip_serializing_if = "Option::is_none"
    )]
    pub widget_accessible: Option<bool>,

    /// Tool visibility to the model.
    #[serde(rename = "openai/visibility", skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ToolVisibility>,

    /// Names of parameters that accept file uploads.
    ///
    /// Each named parameter must be an object with `download_url` and `file_id` fields.
    #[serde(rename = "openai/fileParams", skip_serializing_if = "Option::is_none")]
    pub file_params: Option<Vec<String>>,
}

impl ChatGptToolMeta {
    /// Create empty tool metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the UI template URI.
    pub fn output_template(mut self, uri: impl Into<String>) -> Self {
        self.output_template = Some(uri.into());
        self
    }

    /// Set the message shown while the tool is running.
    pub fn invoking(mut self, msg: impl Into<String>) -> Self {
        self.invoking = Some(msg.into());
        self
    }

    /// Set the message shown when the tool completes.
    pub fn invoked(mut self, msg: impl Into<String>) -> Self {
        self.invoked = Some(msg.into());
        self
    }

    /// Set whether the widget can call this tool.
    pub fn widget_accessible(mut self, accessible: bool) -> Self {
        self.widget_accessible = Some(accessible);
        self
    }

    /// Set tool visibility.
    pub fn visibility(mut self, visibility: ToolVisibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Set file parameter names.
    pub fn file_params(mut self, params: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.file_params = Some(params.into_iter().map(Into::into).collect());
        self
    }

    /// Convert to a `serde_json::Map` for merging into tool `_meta`.
    ///
    /// Produces both flat `openai/*` keys (via serde) and a nested `"ui"` object
    /// containing the spec-compatible `visibility` array (e.g., `["model", "app"]`).
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        // Dual-emit: nested ui.visibility array for MCP standard
        if let Some(vis) = &self.visibility {
            let mut ui_obj = serde_json::Map::with_capacity(1);
            ui_obj.insert(
                "visibility".to_string(),
                serde_json::json!(vis.to_visibility_array()),
            );
            map.insert("ui".to_string(), serde_json::Value::Object(ui_obj));
        }
        map
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.output_template.is_none()
            && self.invoking.is_none()
            && self.invoked.is_none()
            && self.widget_accessible.is_none()
            && self.visibility.is_none()
            && self.file_params.is_none()
    }
}

// =============================================================================
// Widget Response Metadata (Server-to-Widget Communication)
// =============================================================================

/// Metadata for tool responses in ChatGPT Apps.
///
/// These fields control widget behavior after a tool response is received.
/// They are included in the `_meta` field of `CallToolResult`.
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::WidgetResponseMeta;
///
/// // Close the widget after this response
/// let meta = WidgetResponseMeta::new()
///     .close_widget(true)
///     .widget_session_id("session-abc123");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct WidgetResponseMeta {
    /// Close the widget when this response arrives.
    ///
    /// When set to `true`, ChatGPT will hide the widget after processing
    /// this tool response. Useful for "done" or "cancel" actions.
    #[serde(rename = "openai/closeWidget", skip_serializing_if = "Option::is_none")]
    pub close_widget: Option<bool>,

    /// Widget session ID for correlating tool calls.
    ///
    /// This ID is unique per widget instance and can be used to correlate
    /// multiple tool calls from the same widget session for logging or analytics.
    #[serde(
        rename = "openai/widgetSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub widget_session_id: Option<String>,
}

impl WidgetResponseMeta {
    /// Create empty response metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to close the widget.
    pub fn close_widget(mut self, close: bool) -> Self {
        self.close_widget = Some(close);
        self
    }

    /// Set the widget session ID.
    pub fn widget_session_id(mut self, id: impl Into<String>) -> Self {
        self.widget_session_id = Some(id.into());
        self
    }

    /// Convert to a serde_json::Map for merging into tool result `_meta`.
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Check if the metadata is empty (all fields are None).
    pub fn is_empty(&self) -> bool {
        self.close_widget.is_none() && self.widget_session_id.is_none()
    }
}

// =============================================================================
// UI Actions (Widget-to-Host Communication)
// =============================================================================

/// Notification severity level for UI notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum NotifyLevel {
    /// Informational message.
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

/// UI actions that widgets can emit to communicate with the host.
///
/// This is a superset of actions supported across all platforms:
/// - **MCP Apps**: `Tool` only
/// - **ChatGPT Apps**: `Tool`, `SetState`, `SendMessage`
/// - **MCP-UI**: `Tool`, `Prompt`, `Intent`, `Notify`, `Link`
///
/// # Example
///
/// ```rust
/// use pmcp::types::mcp_apps::UIAction;
/// use serde_json::json;
///
/// // Call an MCP tool
/// let action = UIAction::Tool {
///     name: "chess_move".to_string(),
///     arguments: json!({ "from": "e2", "to": "e4" }),
///     message_id: Some("req-123".to_string()),
/// };
///
/// // Update widget state (ChatGPT)
/// let action = UIAction::SetState {
///     state: json!({ "selectedPiece": "e2" }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIAction {
    /// Call an MCP tool.
    ///
    /// Supported by: MCP Apps, ChatGPT Apps, MCP-UI
    #[serde(rename_all = "camelCase")]
    Tool {
        /// Tool name to invoke.
        name: String,
        /// Tool arguments.
        arguments: serde_json::Value,
        /// Optional message ID for async response correlation.
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },

    /// Send a message to the AI (MCP-UI).
    ///
    /// Supported by: MCP-UI, ChatGPT Apps (as `sendFollowUpMessage`)
    Prompt {
        /// Text to send to the AI.
        text: String,
    },

    /// High-level intent action (MCP-UI).
    ///
    /// Supported by: MCP-UI only
    Intent {
        /// Intent action name.
        action: String,
        /// Intent data.
        data: serde_json::Value,
    },

    /// Notification to the user (MCP-UI).
    ///
    /// Supported by: MCP-UI only
    Notify {
        /// Notification severity.
        level: NotifyLevel,
        /// Notification message.
        message: String,
    },

    /// Navigation link (MCP-UI, limited support).
    ///
    /// Supported by: MCP-UI (limited), ChatGPT Apps (via `openExternal`)
    Link {
        /// URL to navigate to.
        url: String,
    },

    /// Update widget state (ChatGPT Apps).
    ///
    /// Supported by: ChatGPT Apps only (via `setWidgetState`)
    SetState {
        /// New state object.
        state: serde_json::Value,
    },

    /// Send follow-up message (ChatGPT Apps).
    ///
    /// Supported by: ChatGPT Apps only
    SendMessage {
        /// Message text.
        message: String,
    },
}

// =============================================================================
// Extended UI MIME Types
// =============================================================================

/// Extended MIME types for MCP Apps.
///
/// Includes support for ChatGPT's Skybridge format and MCP-UI's Remote DOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendedUIMimeType {
    /// Standard MCP Apps HTML (`text/html+mcp`).
    ///
    /// Widgets communicate via postMessage with JSON-RPC protocol.
    HtmlMcp,

    /// ChatGPT Apps Skybridge HTML (`text/html+skybridge`).
    ///
    /// Required by ChatGPT for widget resource recognition in the Templates section.
    /// ChatGPT injects `window.openai` API for widget communication.
    HtmlSkybridge,

    /// HTML with MCP App profile (`text/html;profile=mcp-app`).
    ///
    /// Alternative profile-based MIME type. ChatGPT currently requires
    /// `HtmlSkybridge` instead for Templates section rendering.
    HtmlMcpApp,

    /// Plain HTML for MCP-UI hosts (`text/html`).
    HtmlPlain,

    /// URL list for MCP-UI (`text/uri-list`).
    ///
    /// First valid HTTP(S) URL is loaded in an iframe.
    UriList,

    /// Remote DOM for MCP-UI (`application/vnd.mcp-ui.remote-dom+javascript`).
    ///
    /// JavaScript-based UI using Shopify's Remote DOM library.
    RemoteDom,

    /// Remote DOM with React framework.
    RemoteDomReact,
}

impl ExtendedUIMimeType {
    /// Get the MIME type string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HtmlMcp => "text/html+mcp",
            Self::HtmlSkybridge => "text/html+skybridge",
            Self::HtmlMcpApp => "text/html;profile=mcp-app",
            Self::HtmlPlain => "text/html",
            Self::UriList => "text/uri-list",
            Self::RemoteDom => "application/vnd.mcp-ui.remote-dom+javascript",
            Self::RemoteDomReact => "application/vnd.mcp-ui.remote-dom+javascript; framework=react",
        }
    }

    /// Check if this MIME type is for ChatGPT Apps.
    pub fn is_chatgpt(&self) -> bool {
        matches!(self, Self::HtmlSkybridge | Self::HtmlMcpApp)
    }

    /// Check if this MIME type is for standard MCP Apps.
    pub fn is_mcp_apps(&self) -> bool {
        matches!(self, Self::HtmlMcp)
    }

    /// Check if this MIME type is for MCP-UI.
    pub fn is_mcp_ui(&self) -> bool {
        matches!(
            self,
            Self::HtmlPlain | Self::UriList | Self::RemoteDom | Self::RemoteDomReact
        )
    }
}

impl std::fmt::Display for ExtendedUIMimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ExtendedUIMimeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text/html+mcp" => Ok(Self::HtmlMcp),
            "text/html+skybridge" => Ok(Self::HtmlSkybridge),
            "text/html;profile=mcp-app" => Ok(Self::HtmlMcpApp),
            "text/html" => Ok(Self::HtmlPlain),
            "text/uri-list" => Ok(Self::UriList),
            "application/vnd.mcp-ui.remote-dom+javascript" => Ok(Self::RemoteDom),
            s if s.starts_with("application/vnd.mcp-ui.remote-dom+javascript") => {
                if s.contains("framework=react") {
                    Ok(Self::RemoteDomReact)
                } else {
                    Ok(Self::RemoteDom)
                }
            },
            _ => Err(format!("Unknown UI MIME type: {}", s)),
        }
    }
}

// =============================================================================
// UIMimeType <-> ExtendedUIMimeType Bridge
// =============================================================================

/// Infallible conversion from [`UIMimeType`](crate::types::ui::UIMimeType) to
/// [`ExtendedUIMimeType`].
///
/// Every `UIMimeType` variant has a matching `ExtendedUIMimeType` variant,
/// so this conversion always succeeds.
impl From<crate::types::ui::UIMimeType> for ExtendedUIMimeType {
    fn from(value: crate::types::ui::UIMimeType) -> Self {
        match value {
            crate::types::ui::UIMimeType::HtmlMcp => Self::HtmlMcp,
            crate::types::ui::UIMimeType::HtmlSkybridge => Self::HtmlSkybridge,
            crate::types::ui::UIMimeType::HtmlMcpApp => Self::HtmlMcpApp,
        }
    }
}

/// Fallible conversion from [`ExtendedUIMimeType`] to
/// [`UIMimeType`](crate::types::ui::UIMimeType).
///
/// Only the three shared variants (`HtmlMcp`, `HtmlSkybridge`, `HtmlMcpApp`)
/// can be converted. Extended-only variants (`HtmlPlain`, `UriList`,
/// `RemoteDom`, `RemoteDomReact`) return a descriptive error.
impl TryFrom<ExtendedUIMimeType> for crate::types::ui::UIMimeType {
    type Error = String;

    fn try_from(value: ExtendedUIMimeType) -> Result<Self, Self::Error> {
        match value {
            ExtendedUIMimeType::HtmlMcp => Ok(Self::HtmlMcp),
            ExtendedUIMimeType::HtmlSkybridge => Ok(Self::HtmlSkybridge),
            ExtendedUIMimeType::HtmlMcpApp => Ok(Self::HtmlMcpApp),
            other => Err(format!(
                "Cannot convert {} to UIMimeType (extended-only variant)",
                other
            )),
        }
    }
}

// =============================================================================
// UI Content Types
// =============================================================================

/// Content types for UI resources.
///
/// Represents the different ways UI content can be delivered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIContent {
    /// Inline HTML content.
    Html {
        /// The HTML content.
        html: String,
    },

    /// External URL to load.
    Url {
        /// The URL to load in an iframe.
        url: String,
    },

    /// Remote DOM JavaScript (MCP-UI).
    #[cfg(feature = "mcp-apps")]
    RemoteDom {
        /// JavaScript code defining the UI.
        script: String,
        /// Framework to use for rendering.
        framework: RemoteDomFramework,
    },
}

/// Framework for Remote DOM rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum RemoteDomFramework {
    /// Web Components (default).
    #[default]
    WebComponents,
    /// React.
    React,
}

// =============================================================================
// Universal UI Resource
// =============================================================================

/// Widget dimensions configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct UIDimensions {
    /// Preferred width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Preferred height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Minimum width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u32>,
    /// Minimum height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<u32>,
    /// Maximum width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    /// Maximum height in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
}

/// Universal UI metadata (merged from all platforms).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
pub struct UIMetadata {
    /// Widget description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Preferred dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<UIDimensions>,

    /// Initial data to pass to the widget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_data: Option<serde_json::Value>,

    /// Content Security Policy (ChatGPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<WidgetCSP>,

    /// Additional host-specific metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// =============================================================================
// Host Type Detection
// =============================================================================

/// Known MCP host types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostType {
    /// OpenAI ChatGPT.
    ChatGpt,
    /// Anthropic Claude.
    Claude,
    /// Nanobot (MCP-UI).
    Nanobot,
    /// MCPJam (MCP-UI).
    McpJam,
    /// Generic MCP host.
    Generic,
}

impl HostType {
    /// Get the preferred MIME type for this host.
    pub fn preferred_mime_type(&self) -> ExtendedUIMimeType {
        match self {
            Self::ChatGpt => ExtendedUIMimeType::HtmlSkybridge,
            Self::Claude | Self::Generic => ExtendedUIMimeType::HtmlMcp,
            Self::Nanobot | Self::McpJam => ExtendedUIMimeType::HtmlPlain,
        }
    }

    /// Check if this host supports the given MIME type.
    pub fn supports_mime_type(&self, mime_type: ExtendedUIMimeType) -> bool {
        match self {
            Self::ChatGpt => matches!(
                mime_type,
                ExtendedUIMimeType::HtmlSkybridge | ExtendedUIMimeType::HtmlMcpApp
            ),
            Self::Claude | Self::Generic => matches!(mime_type, ExtendedUIMimeType::HtmlMcp),
            Self::Nanobot | Self::McpJam => mime_type.is_mcp_ui(),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_widget_csp_builder() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .connect("https://api2.example.com")
            .resources("https://cdn.example.com")
            .redirect("https://checkout.example.com")
            .frame("https://embed.example.com");

        assert_eq!(csp.connect_domains.len(), 2);
        assert_eq!(csp.resource_domains.len(), 1);
        assert_eq!(csp.redirect_domains.as_ref().unwrap().len(), 1);
        assert_eq!(csp.frame_domains.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_widget_csp_serialization() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .resources("https://cdn.example.com");

        let json = serde_json::to_value(&csp).unwrap();
        assert_eq!(json["connect_domains"][0], "https://api.example.com");
        assert_eq!(json["resource_domains"][0], "https://cdn.example.com");
        assert!(json.get("redirect_domains").is_none());
        assert!(json.get("frame_domains").is_none());
    }

    #[test]
    fn test_widget_meta_builder() {
        let meta = WidgetMeta::new()
            .prefers_border(true)
            .domain("https://chatgpt.com")
            .description("Test widget")
            .csp(WidgetCSP::new().connect("https://api.example.com"));

        assert_eq!(meta.prefers_border, Some(true));
        assert_eq!(meta.domain, Some("https://chatgpt.com".to_string()));
        assert_eq!(meta.description, Some("Test widget".to_string()));
        assert!(meta.csp.is_some());
    }

    #[test]
    fn test_widget_meta_serialization() {
        let meta = WidgetMeta::new().prefers_border(true).description("Test");

        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["openai/widgetPrefersBorder"], true);
        assert_eq!(json["openai/widgetDescription"], "Test");
    }

    #[test]
    fn test_chatgpt_tool_meta_builder() {
        let meta = ChatGptToolMeta::new()
            .output_template("ui://widget/test.html")
            .invoking("Loading...")
            .invoked("Done!")
            .widget_accessible(true)
            .visibility(ToolVisibility::Public)
            .file_params(vec!["imageFile", "documentFile"]);

        assert_eq!(
            meta.output_template,
            Some("ui://widget/test.html".to_string())
        );
        assert_eq!(meta.invoking, Some("Loading...".to_string()));
        assert_eq!(meta.invoked, Some("Done!".to_string()));
        assert_eq!(meta.widget_accessible, Some(true));
        assert_eq!(meta.visibility, Some(ToolVisibility::Public));
        assert_eq!(meta.file_params.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chatgpt_tool_meta_serialization() {
        let meta = ChatGptToolMeta::new()
            .output_template("ui://widget/test.html")
            .invoking("Loading...")
            .widget_accessible(true)
            .visibility(ToolVisibility::Private);

        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["openai/outputTemplate"], "ui://widget/test.html");
        assert_eq!(json["openai/toolInvocation/invoking"], "Loading...");
        assert_eq!(json["openai/widgetAccessible"], true);
        assert_eq!(json["openai/visibility"], "private");
    }

    #[test]
    fn test_tool_visibility_serialization() {
        assert_eq!(
            serde_json::to_value(ToolVisibility::Public).unwrap(),
            "public"
        );
        assert_eq!(
            serde_json::to_value(ToolVisibility::Private).unwrap(),
            "private"
        );
    }

    #[test]
    fn test_ui_action_tool() {
        let action = UIAction::Tool {
            name: "test_tool".to_string(),
            arguments: json!({ "param": "value" }),
            message_id: Some("123".to_string()),
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "tool");
        assert_eq!(json["name"], "test_tool");
        assert_eq!(json["arguments"]["param"], "value");
        assert_eq!(json["messageId"], "123");
    }

    #[test]
    fn test_ui_action_set_state() {
        let action = UIAction::SetState {
            state: json!({ "selected": "item1" }),
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "setState");
        assert_eq!(json["state"]["selected"], "item1");
    }

    #[test]
    fn test_extended_ui_mime_type() {
        assert_eq!(ExtendedUIMimeType::HtmlMcp.as_str(), "text/html+mcp");
        assert_eq!(
            ExtendedUIMimeType::HtmlSkybridge.as_str(),
            "text/html+skybridge"
        );
        assert_eq!(
            ExtendedUIMimeType::HtmlMcpApp.as_str(),
            "text/html;profile=mcp-app"
        );
        assert_eq!(ExtendedUIMimeType::HtmlPlain.as_str(), "text/html");

        assert!(ExtendedUIMimeType::HtmlSkybridge.is_chatgpt());
        assert!(ExtendedUIMimeType::HtmlMcpApp.is_chatgpt());
        assert!(ExtendedUIMimeType::HtmlMcp.is_mcp_apps());
        assert!(ExtendedUIMimeType::HtmlPlain.is_mcp_ui());
        assert!(!ExtendedUIMimeType::HtmlMcpApp.is_mcp_apps());
    }

    #[test]
    fn test_extended_ui_mime_type_from_str() {
        assert_eq!(
            "text/html+mcp".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlMcp
        );
        assert_eq!(
            "text/html+skybridge".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlSkybridge
        );
        assert_eq!(
            "text/html;profile=mcp-app"
                .parse::<ExtendedUIMimeType>()
                .unwrap(),
            ExtendedUIMimeType::HtmlMcpApp
        );
        assert_eq!(
            "text/html".parse::<ExtendedUIMimeType>().unwrap(),
            ExtendedUIMimeType::HtmlPlain
        );
        assert!("invalid".parse::<ExtendedUIMimeType>().is_err());
    }

    #[test]
    fn test_host_type_mime_type() {
        assert_eq!(
            HostType::ChatGpt.preferred_mime_type(),
            ExtendedUIMimeType::HtmlSkybridge
        );
        assert_eq!(
            HostType::Claude.preferred_mime_type(),
            ExtendedUIMimeType::HtmlMcp
        );
        assert_eq!(
            HostType::Nanobot.preferred_mime_type(),
            ExtendedUIMimeType::HtmlPlain
        );
    }

    #[test]
    fn test_to_meta_map() {
        let widget_meta = WidgetMeta::new().prefers_border(true).description("Test");

        let map = widget_meta.to_meta_map();
        assert_eq!(
            map.get("openai/widgetPrefersBorder"),
            Some(&serde_json::Value::Bool(true))
        );

        let tool_meta = ChatGptToolMeta::new()
            .output_template("ui://test")
            .widget_accessible(true);

        let map = tool_meta.to_meta_map();
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&serde_json::Value::String("ui://test".to_string()))
        );
    }

    #[test]
    fn test_widget_meta_dual_emit_prefers_border() {
        let meta = WidgetMeta::new().prefers_border(true);
        let map = meta.to_meta_map();

        // Flat openai/* key
        assert_eq!(
            map.get("openai/widgetPrefersBorder"),
            Some(&serde_json::Value::Bool(true))
        );

        // Nested ui object
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["prefersBorder"], true);
    }

    #[test]
    fn test_widget_meta_dual_emit_with_domain() {
        let meta = WidgetMeta::new().prefers_border(true).domain("x.com");
        let map = meta.to_meta_map();

        // prefersBorder in nested ui object
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["prefersBorder"], true);
        // domain is now dual-emitted: both flat and nested
        assert_eq!(ui_obj["domain"], "x.com");
        assert_eq!(
            map.get("openai/widgetDomain"),
            Some(&serde_json::Value::String("x.com".to_string()))
        );
    }

    #[test]
    fn test_widget_meta_empty_no_ui_key() {
        let meta = WidgetMeta::new();
        let map = meta.to_meta_map();

        // No spurious ui key when no fields set
        assert!(
            map.get("ui").is_none(),
            "empty WidgetMeta should not have 'ui' key"
        );
        assert!(map.is_empty(), "empty WidgetMeta should produce empty map");
    }

    #[test]
    fn test_from_ui_mime_type() {
        use crate::types::ui::UIMimeType;

        // Each UIMimeType variant converts infallibly to its ExtendedUIMimeType counterpart
        let html_mcp: ExtendedUIMimeType = UIMimeType::HtmlMcp.into();
        assert_eq!(html_mcp, ExtendedUIMimeType::HtmlMcp);
        assert_eq!(html_mcp.as_str(), UIMimeType::HtmlMcp.as_str());

        let html_skybridge: ExtendedUIMimeType = UIMimeType::HtmlSkybridge.into();
        assert_eq!(html_skybridge, ExtendedUIMimeType::HtmlSkybridge);
        assert_eq!(html_skybridge.as_str(), UIMimeType::HtmlSkybridge.as_str());

        let html_mcp_app: ExtendedUIMimeType = UIMimeType::HtmlMcpApp.into();
        assert_eq!(html_mcp_app, ExtendedUIMimeType::HtmlMcpApp);
        assert_eq!(html_mcp_app.as_str(), UIMimeType::HtmlMcpApp.as_str());
    }

    #[test]
    fn test_try_from_extended_shared() {
        use crate::types::ui::UIMimeType;
        use std::convert::TryFrom;

        // Shared variants convert back successfully
        let result = UIMimeType::try_from(ExtendedUIMimeType::HtmlMcp);
        assert_eq!(result, Ok(UIMimeType::HtmlMcp));
        assert_eq!(
            result.unwrap().as_str(),
            ExtendedUIMimeType::HtmlMcp.as_str()
        );

        let result = UIMimeType::try_from(ExtendedUIMimeType::HtmlSkybridge);
        assert_eq!(result, Ok(UIMimeType::HtmlSkybridge));
        assert_eq!(
            result.unwrap().as_str(),
            ExtendedUIMimeType::HtmlSkybridge.as_str()
        );

        let result = UIMimeType::try_from(ExtendedUIMimeType::HtmlMcpApp);
        assert_eq!(result, Ok(UIMimeType::HtmlMcpApp));
        assert_eq!(
            result.unwrap().as_str(),
            ExtendedUIMimeType::HtmlMcpApp.as_str()
        );
    }

    #[test]
    fn test_try_from_extended_fails() {
        use crate::types::ui::UIMimeType;
        use std::convert::TryFrom;

        // Extended-only variants fail conversion with descriptive error
        let extended_only = [
            ExtendedUIMimeType::HtmlPlain,
            ExtendedUIMimeType::UriList,
            ExtendedUIMimeType::RemoteDom,
            ExtendedUIMimeType::RemoteDomReact,
        ];

        for ext in &extended_only {
            let result = UIMimeType::try_from(*ext);
            assert!(result.is_err(), "Expected Err for {:?}", ext);
            let err = result.unwrap_err();
            assert!(
                err.contains("extended-only variant"),
                "Error for {:?} should contain 'extended-only variant', got: {}",
                ext,
                err
            );
        }
    }

    #[test]
    fn test_widget_meta_dual_emit_domain() {
        let meta = WidgetMeta::new().domain("x.com");
        let map = meta.to_meta_map();

        // Flat key preserved
        assert_eq!(
            map.get("openai/widgetDomain"),
            Some(&serde_json::Value::String("x.com".to_string()))
        );

        // Nested ui.domain also emitted
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["domain"], "x.com");
    }

    #[test]
    fn test_widget_meta_dual_emit_csp() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .resources("https://cdn.example.com")
            .frame("https://embed.example.com")
            .base_uri("https://base.example.com")
            .redirect("https://redirect.example.com");

        let meta = WidgetMeta::new().csp(csp);
        let map = meta.to_meta_map();

        // Flat key preserved
        assert!(map.get("openai/widgetCSP").is_some());

        // Nested ui.csp with spec field names
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        let nested_csp = ui_obj.get("csp").expect("must have nested csp");
        assert_eq!(nested_csp["connectDomains"][0], "https://api.example.com");
        assert_eq!(nested_csp["resourceDomains"][0], "https://cdn.example.com");
        assert_eq!(nested_csp["frameDomains"][0], "https://embed.example.com");
        assert_eq!(nested_csp["baseUriDomains"][0], "https://base.example.com");

        // redirect_domains is ChatGPT-specific, must NOT appear in nested ui.csp
        assert!(
            nested_csp.get("redirect_domains").is_none()
                && nested_csp.get("redirectDomains").is_none(),
            "redirect_domains should not be in nested ui.csp (ChatGPT-specific)"
        );
    }

    #[test]
    fn test_widget_meta_dual_emit_all_fields() {
        let meta = WidgetMeta::new()
            .prefers_border(true)
            .domain("x.com")
            .csp(WidgetCSP::new().connect("https://api.example.com"));

        let map = meta.to_meta_map();
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");

        // All three fields in nested ui
        assert_eq!(ui_obj["prefersBorder"], true);
        assert_eq!(ui_obj["domain"], "x.com");
        assert!(ui_obj.get("csp").is_some());
    }

    #[test]
    fn test_chatgpt_tool_meta_dual_emit_visibility_public() {
        let meta = ChatGptToolMeta::new().visibility(ToolVisibility::Public);
        let map = meta.to_meta_map();

        // Flat key preserved
        assert_eq!(map.get("openai/visibility"), Some(&json!("public")));

        // Nested ui.visibility as array
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["visibility"], json!(["model", "app"]));
    }

    #[test]
    fn test_chatgpt_tool_meta_dual_emit_visibility_private() {
        let meta = ChatGptToolMeta::new().visibility(ToolVisibility::Private);
        let map = meta.to_meta_map();

        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["visibility"], json!(["app"]));
    }

    #[test]
    fn test_chatgpt_tool_meta_dual_emit_visibility_model_only() {
        let meta = ChatGptToolMeta::new().visibility(ToolVisibility::ModelOnly);
        let map = meta.to_meta_map();

        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["visibility"], json!(["model"]));
    }

    #[test]
    fn test_widget_csp_base_uri_builder() {
        let csp = WidgetCSP::new().base_uri("https://example.com");
        assert_eq!(
            csp.base_uri_domains,
            Some(vec!["https://example.com".to_string()])
        );
    }

    #[test]
    fn test_widget_csp_base_uri_serialization() {
        let csp = WidgetCSP::new()
            .connect("https://api.example.com")
            .base_uri("https://base.example.com");

        let json = serde_json::to_value(&csp).unwrap();
        assert_eq!(json["base_uri_domains"][0], "https://base.example.com");
    }

    #[test]
    fn test_widget_csp_is_empty_with_base_uri() {
        let csp = WidgetCSP::new().base_uri("https://example.com");
        assert!(
            !csp.is_empty(),
            "CSP with base_uri_domains should not be empty"
        );
    }

    #[test]
    fn test_tool_visibility_model_only_serialization() {
        assert_eq!(
            serde_json::to_value(ToolVisibility::ModelOnly).unwrap(),
            "modelonly"
        );
    }

    #[test]
    fn test_tool_visibility_has_three_variants() {
        // Verify all 3 variants exist and serialize correctly
        let variants = vec![
            (ToolVisibility::Public, "public"),
            (ToolVisibility::Private, "private"),
            (ToolVisibility::ModelOnly, "modelonly"),
        ];
        for (variant, expected) in variants {
            assert_eq!(
                serde_json::to_value(variant).unwrap(),
                expected,
                "ToolVisibility::{:?} should serialize to {:?}",
                variant,
                expected
            );
        }
    }

    #[test]
    fn test_tool_visibility_to_visibility_array() {
        assert_eq!(
            ToolVisibility::Public.to_visibility_array(),
            &["model", "app"]
        );
        assert_eq!(ToolVisibility::Private.to_visibility_array(), &["app"]);
        assert_eq!(ToolVisibility::ModelOnly.to_visibility_array(), &["model"]);
    }

    #[test]
    fn test_widget_meta_resource_uri_emits_triple_keys() {
        let meta = WidgetMeta::new().resource_uri("ui://chess/board.html");
        let map = meta.to_meta_map();

        // Nested ui.resourceUri
        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://chess/board.html");

        // Legacy flat key
        assert_eq!(
            map.get("ui/resourceUri"),
            Some(&json!("ui://chess/board.html"))
        );

        // ChatGPT alias
        assert_eq!(
            map.get("openai/outputTemplate"),
            Some(&json!("ui://chess/board.html"))
        );
    }

    #[test]
    fn test_widget_meta_resource_uri_with_widget_fields() {
        let meta = WidgetMeta::new()
            .resource_uri("ui://chess/board.html")
            .prefers_border(true)
            .domain("chess.com");
        let map = meta.to_meta_map();

        let ui_obj = map.get("ui").expect("must have nested 'ui' key");
        // All fields coexist in the same nested ui object
        assert_eq!(ui_obj["resourceUri"], "ui://chess/board.html");
        assert_eq!(ui_obj["prefersBorder"], true);
        assert_eq!(ui_obj["domain"], "chess.com");

        // Flat keys also present
        assert!(map.get("ui/resourceUri").is_some());
        assert!(map.get("openai/outputTemplate").is_some());
        assert!(map.get("openai/widgetPrefersBorder").is_some());
        assert!(map.get("openai/widgetDomain").is_some());
    }

    #[test]
    fn test_widget_meta_resource_uri_not_in_serde_output() {
        // resource_uri should not appear as a raw key in the map
        let meta = WidgetMeta::new().resource_uri("ui://x");
        let map = meta.to_meta_map();
        assert!(
            map.get("resource_uri").is_none(),
            "resource_uri should be removed from serde output"
        );
    }

    #[test]
    fn test_widget_meta_is_empty_with_resource_uri() {
        let meta = WidgetMeta::new().resource_uri("ui://x");
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_mime_type_round_trip() {
        use crate::types::ui::UIMimeType;
        use std::convert::TryFrom;

        // Round-trip: UIMimeType -> ExtendedUIMimeType -> UIMimeType preserves value
        let variants = [
            UIMimeType::HtmlMcp,
            UIMimeType::HtmlSkybridge,
            UIMimeType::HtmlMcpApp,
        ];

        for original in &variants {
            let extended: ExtendedUIMimeType = (*original).into();
            let round_tripped = UIMimeType::try_from(extended)
                .unwrap_or_else(|e| panic!("Round-trip failed for {:?}: {}", original, e));
            assert_eq!(
                *original, round_tripped,
                "Round-trip failed: {:?} != {:?}",
                original, round_tripped
            );
        }
    }
}
