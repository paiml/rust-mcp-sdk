// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! UI Adapter implementations for different MCP host platforms.

use crate::types::mcp_apps::{ExtendedUIMimeType, HostType, WidgetCSP, WidgetMeta};
use serde_json::Value;
use std::collections::HashMap;

/// Trait for adapting UI resources to specific MCP host platforms.
///
/// Adapters transform HTML content into platform-specific formats,
/// enabling a single tool implementation to work across multiple hosts.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::mcp_apps::{UIAdapter, ChatGptAdapter};
///
/// let adapter = ChatGptAdapter::new();
/// let html = "<html><body>Chess Board</body></html>";
/// let transformed = adapter.transform("ui://chess/board", "Chess Board", html);
/// ```
pub trait UIAdapter: Send + Sync {
    /// Get the host type this adapter targets.
    fn host_type(&self) -> HostType;

    /// Get the MIME type this adapter produces.
    fn mime_type(&self) -> ExtendedUIMimeType;

    /// Transform HTML content for this host platform.
    ///
    /// Returns the transformed resource with platform-specific metadata.
    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource;

    /// Inject platform-specific communication bridge into HTML content.
    ///
    /// For ChatGPT Apps, this wraps the HTML with `window.openai` bridge code.
    /// For MCP Apps, this adds postMessage bridge code.
    fn inject_bridge(&self, html: &str) -> String;

    /// Get CSP headers required by this platform.
    fn required_csp(&self) -> Option<WidgetCSP>;
}

/// A UI resource transformed for a specific host platform.
#[derive(Debug, Clone)]
pub struct TransformedResource {
    /// Original resource URI.
    pub uri: String,
    /// Display name.
    pub name: String,
    /// MIME type for this platform.
    pub mime_type: ExtendedUIMimeType,
    /// Transformed HTML content with platform bridge injected.
    pub content: String,
    /// Platform-specific metadata.
    pub metadata: HashMap<String, Value>,
}

impl TransformedResource {
    /// Take platform metadata as the `_meta` map format used by `Content::Resource`.
    ///
    /// Returns `None` if metadata is empty, `Some(map)` otherwise.
    /// Drains the metadata from this resource.
    pub fn take_meta(&mut self) -> Option<serde_json::Map<String, Value>> {
        if self.metadata.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.metadata).into_iter().collect())
        }
    }
}

/// Adapter for ChatGPT Apps (OpenAI Apps SDK).
///
/// Transforms resources to use `text/html;profile=mcp-app` MIME type and
/// injects the `window.openai` bridge for widget communication.
#[derive(Debug, Clone, Default)]
pub struct ChatGptAdapter {
    /// Optional widget metadata for ChatGPT.
    pub widget_meta: Option<WidgetMeta>,
}

impl ChatGptAdapter {
    /// Create a new ChatGPT adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set widget metadata for this adapter.
    #[must_use]
    pub fn with_widget_meta(mut self, meta: WidgetMeta) -> Self {
        self.widget_meta = Some(meta);
        self
    }
}

impl UIAdapter for ChatGptAdapter {
    fn host_type(&self) -> HostType {
        HostType::ChatGpt
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        ExtendedUIMimeType::HtmlSkybridge
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        let injected_html = self.inject_bridge(html);

        // Only emit the 4 descriptor keys ChatGPT expects.
        // Display keys (widgetCSP, widgetPrefersBorder, widgetDescription) cause
        // ChatGPT's Templates section to fail, so they are deliberately excluded.
        let metadata: HashMap<String, Value> = self
            .widget_meta
            .as_ref()
            .map(|wm| {
                crate::types::ui::filter_to_descriptor_keys(&wm.to_meta_map())
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();

        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content: injected_html,
            metadata,
        }
    }

    fn inject_bridge(&self, html: &str) -> String {
        // ChatGPT Apps bridge script - Full OpenAI Apps SDK alignment
        let bridge_script = r#"
<script>
// ChatGPT Apps Bridge - Full window.openai API wrapper
// Aligned with OpenAI Apps SDK documentation
(function() {
    'use strict';

    // State management
    var widgetState = {};
    var _toolOutput = null;

    // Initialize bridge when ready
    if (window.openai) {
        initBridge();
    } else {
        window.addEventListener('load', function() {
            if (window.openai) initBridge();
        });
    }

    function initBridge() {
        // Notify ChatGPT that widget is ready
        window.openai.onWidgetReady?.();

        // Listen for state updates from ChatGPT
        window.openai.onStateUpdate?.(function(newState) {
            widgetState = Object.assign({}, widgetState, newState);
            window.dispatchEvent(new CustomEvent('widgetStateUpdate', { detail: widgetState }));
        });
    }

    // Listen for tool results via postMessage from ChatGPT host
    window.addEventListener('message', function(event) {
        if (event.source !== window.parent) return;
        var msg = event.data;
        if (!msg || msg.jsonrpc !== '2.0') return;
        if (msg.method === 'ui/notifications/tool-result') {
            var data = msg.params && msg.params.structuredContent;
            if (data) {
                _toolOutput = data;
                window.dispatchEvent(new CustomEvent('widgetStateUpdate', {
                    detail: { toolOutput: data }
                }));
            }
        }
    }, { passive: true });

    // Listen for openai:set_globals (alternate data delivery)
    window.addEventListener('openai:set_globals', function(event) {
        var data = event.detail && event.detail.globals && event.detail.globals.toolOutput;
        if (data) {
            _toolOutput = data;
            window.dispatchEvent(new CustomEvent('widgetStateUpdate', {
                detail: { toolOutput: data }
            }));
        }
    }, { passive: true });

    // Expose bridge API - aligned with window.openai
    window.mcpBridge = {
        // ========================================
        // Core Operations
        // ========================================

        // Call an MCP tool
        callTool: async (name, args) => {
            if (window.openai?.callTool) {
                return window.openai.callTool(name, args);
            }
            throw new Error('ChatGPT bridge not available');
        },

        // ========================================
        // State Management
        // ========================================

        // Get current widget state
        getState: () => window.openai?.widgetState ?? widgetState,

        // Update widget state
        setState: (newState) => {
            widgetState = { ...widgetState, ...newState };
            window.openai?.setWidgetState?.(widgetState);
        },

        // ========================================
        // Tool Context (Read-only)
        // ========================================

        // Arguments supplied when the tool was invoked
        get toolInput() {
            return window.openai?.toolInput ?? {};
        },

        // The structuredContent returned by the tool
        get toolOutput() {
            return _toolOutput ?? window.openai?.toolOutput;
        },

        // The _meta payload (widget-only, never sent to model)
        get toolResponseMetadata() {
            return window.openai?.toolResponseMetadata ?? {};
        },

        // ========================================
        // Communication
        // ========================================

        // Send follow-up message
        sendMessage: (message) => {
            window.openai?.sendFollowUpMessage?.({ prompt: message });
        },

        // Open external URL
        openExternal: (url) => {
            window.openai?.openExternal?.({ href: url });
        },

        // ========================================
        // File Operations
        // ========================================

        // Upload a file and get a file ID
        uploadFile: async (file) => {
            if (window.openai?.uploadFile) {
                return window.openai.uploadFile(file);
            }
            return null;
        },

        // Get a temporary download URL for a file
        getFileDownloadUrl: async (fileId) => {
            if (window.openai?.getFileDownloadUrl) {
                return window.openai.getFileDownloadUrl({ fileId });
            }
            return null;
        },

        // ========================================
        // Display Modes
        // ========================================

        // Request a display mode change (inline, pip, fullscreen)
        requestDisplayMode: async (mode) => {
            if (window.openai?.requestDisplayMode) {
                return window.openai.requestDisplayMode({ mode });
            }
        },

        // Close the widget
        requestClose: () => {
            window.openai?.requestClose?.();
        },

        // Report the widget's intrinsic height
        notifyIntrinsicHeight: (height) => {
            window.openai?.notifyIntrinsicHeight?.(height);
        },

        // Set the URL for the "Open in App" button
        setOpenInAppUrl: (href) => {
            window.openai?.setOpenInAppUrl?.({ href });
        },

        // ========================================
        // Environment Context (Read-only)
        // ========================================

        // Current theme ('light' or 'dark')
        get theme() {
            return window.openai?.theme ?? 'light';
        },

        // Current locale (e.g., 'en-US')
        get locale() {
            return window.openai?.locale ?? 'en-US';
        },

        // Current display mode ('inline', 'pip', 'fullscreen')
        get displayMode() {
            return window.openai?.displayMode ?? 'inline';
        },

        // Maximum widget height in pixels
        get maxHeight() {
            return window.openai?.maxHeight;
        },

        // Safe area insets
        get safeArea() {
            return window.openai?.safeArea;
        },

        // User agent string
        get userAgent() {
            return window.openai?.userAgent;
        },

        // Widget view type ('default' or 'compact')
        get view() {
            return window.openai?.view ?? 'default';
        }
    };

    // Dispatch ready event for widgets waiting on bridge
    window.dispatchEvent(new Event('mcpBridgeReady'));
})();
</script>
"#;

        // Inject bridge script before </head> or at the beginning
        if html.contains("</head>") {
            html.replace("</head>", &format!("{bridge_script}</head>"))
        } else if html.contains("<body") {
            // Find <body> tag and inject before it
            let pos = html.find("<body").unwrap_or(0);
            format!(
                "{}<head>{}</head>{}",
                &html[..pos],
                bridge_script,
                &html[pos..]
            )
        } else {
            format!("{bridge_script}{html}")
        }
    }

    fn required_csp(&self) -> Option<WidgetCSP> {
        // ChatGPT has its own CSP management
        None
    }
}

/// Adapter for MCP Apps (SEP-1865 standard).
///
/// Transforms resources to use `text/html+mcp` MIME type and
/// injects postMessage bridge for MCP JSON-RPC communication.
#[derive(Debug, Clone, Default)]
pub struct McpAppsAdapter {
    /// Optional Content Security Policy.
    pub csp: Option<WidgetCSP>,
}

impl McpAppsAdapter {
    /// Create a new MCP Apps adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set CSP for this adapter.
    #[must_use]
    pub fn with_csp(mut self, csp: WidgetCSP) -> Self {
        self.csp = Some(csp);
        self
    }
}

impl UIAdapter for McpAppsAdapter {
    fn host_type(&self) -> HostType {
        HostType::Generic
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        ExtendedUIMimeType::HtmlMcp
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        let injected_html = self.inject_bridge(html);

        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content: injected_html,
            metadata: HashMap::new(),
        }
    }

    fn inject_bridge(&self, html: &str) -> String {
        // MCP Apps bridge script using postMessage
        let bridge_script = r"
<script>
// MCP Apps Bridge - postMessage JSON-RPC
(function() {
    'use strict';

    let requestId = 0;
    const pendingRequests = new Map();

    // Listen for messages from host
    window.addEventListener('message', (event) => {
        const msg = event.data;

        if (msg.jsonrpc !== '2.0') return;

        // Handle responses
        if (msg.id !== undefined && pendingRequests.has(msg.id)) {
            const { resolve, reject } = pendingRequests.get(msg.id);
            pendingRequests.delete(msg.id);

            if (msg.error) {
                reject(new Error(msg.error.message || 'Unknown error'));
            } else {
                resolve(msg.result);
            }
        }

        // Handle notifications
        if (msg.method && !msg.id) {
            window.dispatchEvent(new CustomEvent('mcpNotification', { detail: msg }));
        }
    });

    // Send JSON-RPC request
    function sendRequest(method, params) {
        return new Promise((resolve, reject) => {
            const id = ++requestId;
            pendingRequests.set(id, { resolve, reject });

            window.parent.postMessage({
                jsonrpc: '2.0',
                id,
                method,
                params
            }, '*');

            // Timeout after 30 seconds
            setTimeout(() => {
                if (pendingRequests.has(id)) {
                    pendingRequests.delete(id);
                    reject(new Error('Request timeout'));
                }
            }, 30000);
        });
    }

    // Expose bridge API
    window.mcpBridge = {
        // Call an MCP tool
        callTool: (name, args) => sendRequest('tools/call', { name, arguments: args }),

        // Read a resource
        readResource: (uri) => sendRequest('resources/read', { uri }),

        // Get a prompt
        getPrompt: (name, args) => sendRequest('prompts/get', { name, arguments: args }),

        // Send notification
        notify: (method, params) => {
            window.parent.postMessage({
                jsonrpc: '2.0',
                method,
                params
            }, '*');
        }
    };

    // Notify host that widget is initializing
    window.mcpBridge.notify('ui/initialize', {});
})();
</script>
";

        if html.contains("</head>") {
            html.replace("</head>", &format!("{bridge_script}</head>"))
        } else if html.contains("<body") {
            let pos = html.find("<body").unwrap_or(0);
            format!(
                "{}<head>{}</head>{}",
                &html[..pos],
                bridge_script,
                &html[pos..]
            )
        } else {
            format!("{bridge_script}{html}")
        }
    }

    fn required_csp(&self) -> Option<WidgetCSP> {
        self.csp.clone()
    }
}

/// Adapter for MCP-UI (community standard).
///
/// Supports multiple output formats including HTML, URLs, and Remote DOM.
#[derive(Debug, Clone, Default)]
pub struct McpUiAdapter {
    /// Preferred output format.
    pub preferred_format: McpUiFormat,
}

/// MCP-UI output formats.
#[derive(Debug, Clone, Default)]
pub enum McpUiFormat {
    /// HTML with postMessage bridge.
    #[default]
    Html,
    /// URL reference (for CDN-hosted widgets).
    Url,
    /// Remote DOM (for non-iframe rendering).
    RemoteDom,
}

impl McpUiAdapter {
    /// Create a new MCP-UI adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set preferred output format.
    #[must_use]
    pub fn with_format(mut self, format: McpUiFormat) -> Self {
        self.preferred_format = format;
        self
    }
}

impl UIAdapter for McpUiAdapter {
    fn host_type(&self) -> HostType {
        HostType::Nanobot // MCP-UI hosts like Nanobot
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        match self.preferred_format {
            McpUiFormat::Html => ExtendedUIMimeType::HtmlPlain,
            McpUiFormat::Url => ExtendedUIMimeType::UriList,
            McpUiFormat::RemoteDom => ExtendedUIMimeType::RemoteDom,
        }
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        let content = match self.preferred_format {
            McpUiFormat::Html => self.inject_bridge(html),
            McpUiFormat::Url => uri.to_string(),
            McpUiFormat::RemoteDom => {
                format!(
                    r#"{{"type":"remote-dom","uri":"{}","name":"{}"}}"#,
                    uri, name
                )
            },
        };

        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content,
            metadata: HashMap::new(),
        }
    }

    fn inject_bridge(&self, html: &str) -> String {
        // MCP-UI bridge script
        let bridge_script = r"
<script>
// MCP-UI Bridge
(function() {
    'use strict';

    let requestId = 0;
    const pendingRequests = new Map();

    window.addEventListener('message', (event) => {
        const msg = event.data;
        if (msg.jsonrpc !== '2.0') return;

        if (msg.id !== undefined && pendingRequests.has(msg.id)) {
            const { resolve, reject } = pendingRequests.get(msg.id);
            pendingRequests.delete(msg.id);
            if (msg.error) reject(new Error(msg.error.message));
            else resolve(msg.result);
        }

        if (msg.method && !msg.id) {
            window.dispatchEvent(new CustomEvent('mcpNotification', { detail: msg }));
        }
    });

    function sendRequest(method, params) {
        return new Promise((resolve, reject) => {
            const id = ++requestId;
            pendingRequests.set(id, { resolve, reject });
            window.parent.postMessage({ jsonrpc: '2.0', id, method, params }, '*');
            setTimeout(() => {
                if (pendingRequests.has(id)) {
                    pendingRequests.delete(id);
                    reject(new Error('Request timeout'));
                }
            }, 30000);
        });
    }

    // MCP-UI specific actions
    window.mcpBridge = {
        callTool: (name, args) => sendRequest('tools/call', { name, arguments: args }),
        readResource: (uri) => sendRequest('resources/read', { uri }),
        getPrompt: (name, args) => sendRequest('prompts/get', { name, arguments: args }),

        // MCP-UI specific
        sendIntent: (action, data) => sendRequest('ui/intent', { action, data }),
        notify: (level, message) => {
            window.parent.postMessage({
                jsonrpc: '2.0',
                method: 'ui/notify',
                params: { level, message }
            }, '*');
        },
        openLink: (url) => {
            window.parent.postMessage({
                jsonrpc: '2.0',
                method: 'ui/link',
                params: { url }
            }, '*');
        }
    };

    window.mcpBridge.notify('info', 'Widget ready');
})();
</script>
";

        if html.contains("</head>") {
            html.replace("</head>", &format!("{bridge_script}</head>"))
        } else {
            format!("{bridge_script}{html}")
        }
    }

    fn required_csp(&self) -> Option<WidgetCSP> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chatgpt_adapter_transform() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><head></head><body>Hello</body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert_eq!(transformed.mime_type, ExtendedUIMimeType::HtmlSkybridge);
        assert!(transformed.content.contains("window.mcpBridge"));
        assert!(transformed.content.contains("window.openai"));
    }

    #[test]
    fn test_mcp_apps_adapter_transform() {
        let adapter = McpAppsAdapter::new();
        let html = "<html><body></body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert_eq!(transformed.mime_type, ExtendedUIMimeType::HtmlMcp);
        assert!(transformed.content.contains("window.mcpBridge"));
        assert!(transformed.content.contains("postMessage"));
    }

    #[test]
    fn test_mcp_ui_adapter_transform() {
        let adapter = McpUiAdapter::new();
        let html = "<html><body></body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert!(transformed.content.contains("mcpBridge"));
    }

    #[test]
    fn test_chatgpt_adapter_with_widget_meta_strips_display_keys() {
        // Display-only keys (prefers_border, description) should be stripped
        let display_only = WidgetMeta::new()
            .prefers_border(true)
            .description("A test widget");

        let adapter = ChatGptAdapter::new().with_widget_meta(display_only);
        let html = "<html><body></body></html>";
        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert!(
            transformed.metadata.is_empty(),
            "display keys should be stripped from ChatGPT metadata"
        );
    }

    #[test]
    fn test_chatgpt_adapter_with_widget_meta_keeps_descriptor_keys() {
        // resource_uri produces openai/outputTemplate — a descriptor key
        let meta = WidgetMeta::new()
            .resource_uri("ui://test/widget.html")
            .prefers_border(true);

        let adapter = ChatGptAdapter::new().with_widget_meta(meta);
        let html = "<html><body></body></html>";
        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert!(
            transformed.metadata.contains_key("openai/outputTemplate"),
            "descriptor keys should be kept"
        );
        assert!(
            !transformed
                .metadata
                .contains_key("openai/widgetPrefersBorder"),
            "display keys should be stripped"
        );
    }

    #[test]
    fn test_bridge_injection_with_head() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><head><title>Test</title></head><body></body></html>";
        let result = adapter.inject_bridge(html);

        assert!(result.contains("window.mcpBridge"));
        assert!(result.contains("</head>"));
    }

    #[test]
    fn test_bridge_injection_without_head() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><body>Content</body></html>";
        let result = adapter.inject_bridge(html);

        assert!(result.contains("window.mcpBridge"));
    }
}
