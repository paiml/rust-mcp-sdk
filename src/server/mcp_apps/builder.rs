// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! UI Resource builders for multi-platform support.

use super::adapter::{
    ChatGptAdapter, McpAppsAdapter, McpUiAdapter, TransformedResource, UIAdapter,
};
use crate::types::mcp_apps::{ChatGptToolMeta, HostType, WidgetCSP, WidgetMeta};
use std::collections::HashMap;

/// A UI resource that can be transformed for multiple platforms.
///
/// This allows a single tool implementation to return content that works
/// across ChatGPT Apps, MCP Apps, and MCP-UI hosts.
///
/// # Example
///
/// ```rust
/// use pmcp::server::mcp_apps::{MultiPlatformResource, ChatGptAdapter, McpAppsAdapter};
/// use pmcp::types::mcp_apps::HostType;
///
/// let html = "<html><body>Chess board here</body></html>";
///
/// let mut multi = MultiPlatformResource::new(
///     "ui://chess/board.html",
///     "Chess Board",
///     html,
/// )
/// .with_adapter(ChatGptAdapter::new())
/// .with_adapter(McpAppsAdapter::new());
///
/// // Get content for a specific host
/// if let Some(transformed) = multi.for_host(HostType::ChatGpt) {
///     // Use transformed.content and transformed.metadata
///     assert!(transformed.content.contains("mcpBridge"));
/// }
/// ```
pub struct MultiPlatformResource {
    /// Resource URI.
    uri: String,
    /// Display name.
    name: String,
    /// HTML content.
    html: String,
    /// Registered adapters.
    adapters: Vec<Box<dyn UIAdapter>>,
    /// Cached transformations.
    cache: HashMap<HostType, TransformedResource>,
}

impl std::fmt::Debug for MultiPlatformResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiPlatformResource")
            .field("uri", &self.uri)
            .field("name", &self.name)
            .field("html", &self.html)
            .field("adapters", &format!("[{} adapters]", self.adapters.len()))
            .field("cache", &self.cache)
            .finish()
    }
}

impl MultiPlatformResource {
    /// Create a new multi-platform resource.
    #[must_use]
    pub fn new(uri: impl Into<String>, name: impl Into<String>, html: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            html: html.into(),
            adapters: Vec::new(),
            cache: HashMap::new(),
        }
    }

    /// Add an adapter for a specific platform.
    #[must_use]
    pub fn with_adapter<A: UIAdapter + 'static>(mut self, adapter: A) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    /// Add all standard adapters (ChatGPT, MCP Apps, MCP-UI).
    #[must_use]
    pub fn with_all_adapters(self) -> Self {
        self.with_adapter(ChatGptAdapter::new())
            .with_adapter(McpAppsAdapter::new())
            .with_adapter(McpUiAdapter::new())
    }

    /// Get the transformed resource for a specific host type.
    ///
    /// Returns `None` if no adapter is registered for this host.
    pub fn for_host(&mut self, host: HostType) -> Option<&TransformedResource> {
        // Check cache first
        if self.cache.contains_key(&host) {
            return self.cache.get(&host);
        }

        // Find matching adapter
        let adapter = self.adapters.iter().find(|a| a.host_type() == host)?;
        let transformed = adapter.transform(&self.uri, &self.name, &self.html);
        self.cache.insert(host, transformed);
        self.cache.get(&host)
    }

    /// Get all transformed resources.
    pub fn all_transforms(&mut self) -> Vec<&TransformedResource> {
        // Transform with all adapters
        for adapter in &self.adapters {
            let host = adapter.host_type();
            if !self.cache.contains_key(&host) {
                let transformed = adapter.transform(&self.uri, &self.name, &self.html);
                self.cache.insert(host, transformed);
            }
        }
        self.cache.values().collect()
    }

    /// Get the original HTML content.
    #[must_use]
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Get the resource URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Get the resource name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Builder for creating UI resources with platform-specific configuration.
///
/// This builder provides a fluent API for configuring UI resources with
/// platform-specific metadata and CSP settings.
///
/// # Example
///
/// ```rust
/// use pmcp::server::mcp_apps::UIResourceBuilder;
///
/// let resource = UIResourceBuilder::new("ui://chess/board.html", "Chess Board")
///     .html("<html><body>Chess board</body></html>")
///     .chatgpt_invoking("Loading chess board...")
///     .chatgpt_widget_accessible(true)
///     .csp_connect("https://api.chess.com")
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct UIResourceBuilder {
    /// Resource URI.
    uri: String,
    /// Display name.
    name: String,
    /// HTML content (if inline).
    html: Option<String>,
    /// Description.
    description: Option<String>,
    /// ChatGPT-specific tool metadata.
    chatgpt_meta: ChatGptToolMeta,
    /// Widget metadata for ChatGPT.
    widget_meta: WidgetMeta,
    /// Content Security Policy.
    csp: WidgetCSP,
}

impl UIResourceBuilder {
    /// Create a new UI resource builder.
    #[must_use]
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set inline HTML content.
    #[must_use]
    pub fn html(mut self, content: impl Into<String>) -> Self {
        self.html = Some(content.into());
        self
    }

    /// Set resource description.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    // =========================================================================
    // ChatGPT-specific configuration
    // =========================================================================

    /// Set ChatGPT output template URI.
    #[must_use]
    pub fn chatgpt_output_template(mut self, uri: impl Into<String>) -> Self {
        self.chatgpt_meta = self.chatgpt_meta.output_template(uri);
        self
    }

    /// Set ChatGPT invoking message.
    #[must_use]
    pub fn chatgpt_invoking(mut self, message: impl Into<String>) -> Self {
        self.chatgpt_meta = self.chatgpt_meta.invoking(message);
        self
    }

    /// Set ChatGPT invoked message.
    #[must_use]
    pub fn chatgpt_invoked(mut self, message: impl Into<String>) -> Self {
        self.chatgpt_meta = self.chatgpt_meta.invoked(message);
        self
    }

    /// Set whether the widget can call tools.
    #[must_use]
    pub fn chatgpt_widget_accessible(mut self, accessible: bool) -> Self {
        self.chatgpt_meta = self.chatgpt_meta.widget_accessible(accessible);
        self
    }

    /// Set widget border preference.
    #[must_use]
    pub fn widget_prefers_border(mut self, border: bool) -> Self {
        self.widget_meta = self.widget_meta.prefers_border(border);
        self
    }

    // =========================================================================
    // CSP configuration
    // =========================================================================

    /// Add allowed connect domain for CSP.
    #[must_use]
    pub fn csp_connect(mut self, domain: impl Into<String>) -> Self {
        self.csp = self.csp.connect(domain);
        self
    }

    /// Add allowed resource domain for CSP.
    #[must_use]
    pub fn csp_resource(mut self, domain: impl Into<String>) -> Self {
        self.csp = self.csp.resources(domain);
        self
    }

    /// Add allowed frame domain for CSP.
    #[must_use]
    pub fn csp_frame(mut self, domain: impl Into<String>) -> Self {
        self.csp = self.csp.frame(domain);
        self
    }

    // =========================================================================
    // Build methods
    // =========================================================================

    /// Build a `MultiPlatformResource`.
    ///
    /// Returns the resource info (uri, name) and the multi-platform resource.
    #[must_use]
    pub fn build(self) -> MultiPlatformResource {
        let html = self.html.unwrap_or_default();
        MultiPlatformResource::new(&self.uri, &self.name, html)
    }

    /// Build a `MultiPlatformResource` for ChatGPT only.
    #[must_use]
    pub fn build_chatgpt(self) -> MultiPlatformResource {
        let widget_meta = self.widget_meta.clone();
        let html = self.html.clone().unwrap_or_default();

        MultiPlatformResource::new(&self.uri, &self.name, html)
            .with_adapter(ChatGptAdapter::new().with_widget_meta(widget_meta))
    }

    /// Build a `MultiPlatformResource` for MCP Apps only.
    #[must_use]
    pub fn build_mcp_apps(self) -> MultiPlatformResource {
        let csp = self.csp.clone();
        let html = self.html.clone().unwrap_or_default();

        MultiPlatformResource::new(&self.uri, &self.name, html)
            .with_adapter(McpAppsAdapter::new().with_csp(csp))
    }

    /// Build a `MultiPlatformResource` for all platforms.
    #[must_use]
    pub fn build_all(self) -> MultiPlatformResource {
        let widget_meta = self.widget_meta.clone();
        let csp = self.csp.clone();
        let html = self.html.clone().unwrap_or_default();

        MultiPlatformResource::new(&self.uri, &self.name, html)
            .with_adapter(ChatGptAdapter::new().with_widget_meta(widget_meta))
            .with_adapter(McpAppsAdapter::new().with_csp(csp))
            .with_adapter(McpUiAdapter::new())
    }

    /// Get the ChatGPT tool metadata for use in tool definitions.
    #[must_use]
    pub fn chatgpt_tool_meta(&self) -> &ChatGptToolMeta {
        &self.chatgpt_meta
    }

    /// Get the widget metadata.
    #[must_use]
    pub fn widget_meta(&self) -> &WidgetMeta {
        &self.widget_meta
    }

    /// Get the CSP configuration.
    #[must_use]
    pub fn csp(&self) -> &WidgetCSP {
        &self.csp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mcp_apps::ExtendedUIMimeType;

    #[test]
    fn test_ui_resource_builder_basic() {
        let multi = UIResourceBuilder::new("ui://test/widget.html", "Test Widget")
            .html("<html><body>Test</body></html>")
            .description("A test widget")
            .build();

        assert_eq!(multi.uri(), "ui://test/widget.html");
        assert_eq!(multi.name(), "Test Widget");
    }

    #[test]
    fn test_ui_resource_builder_chatgpt_config() {
        let builder = UIResourceBuilder::new("ui://chess/board.html", "Chess Board")
            .html("<html><body>Board</body></html>")
            .chatgpt_invoking("Loading...")
            .chatgpt_invoked("Ready!")
            .chatgpt_widget_accessible(true);

        let meta = builder.chatgpt_tool_meta();
        // The meta should have the values set
        assert!(serde_json::to_value(meta).is_ok());
    }

    #[test]
    fn test_ui_resource_builder_csp() {
        let builder = UIResourceBuilder::new("ui://test/widget.html", "Test")
            .html("<html></html>")
            .csp_connect("https://api.example.com")
            .csp_resource("https://cdn.example.com");

        let csp = builder.csp();
        assert!(!csp.connect_domains.is_empty());
        assert!(!csp.resource_domains.is_empty());
    }

    #[test]
    fn test_multi_platform_resource() {
        let mut multi = MultiPlatformResource::new(
            "ui://test/widget.html",
            "Test Widget",
            "<html><body>Test</body></html>",
        )
        .with_all_adapters();

        let chatgpt = multi.for_host(HostType::ChatGpt);
        assert!(chatgpt.is_some());
        assert_eq!(
            chatgpt.unwrap().mime_type,
            ExtendedUIMimeType::HtmlSkybridge
        );

        let generic = multi.for_host(HostType::Generic);
        assert!(generic.is_some());
        assert_eq!(generic.unwrap().mime_type, ExtendedUIMimeType::HtmlMcp);
    }

    #[test]
    fn test_build_chatgpt() {
        let mut multi = UIResourceBuilder::new("ui://test/widget.html", "Test")
            .html("<html></html>")
            .chatgpt_invoking("Loading...")
            .build_chatgpt();

        let transformed = multi.for_host(HostType::ChatGpt);
        assert!(transformed.is_some());
    }

    #[test]
    fn test_build_all() {
        let mut multi = UIResourceBuilder::new("ui://test/widget.html", "Test")
            .html("<html></html>")
            .build_all();

        let transforms = multi.all_transforms();
        assert!(!transforms.is_empty());
    }
}
