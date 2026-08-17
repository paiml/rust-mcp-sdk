//! UI resource builder for MCP Apps Extension (SEP-1865).
//!
//! This module provides a fluent API for creating and managing UI resources
//! in MCP servers. UI resources enable interactive user interfaces that can
//! be displayed by MCP hosts.
//!
//! # Example
//!
//! ```rust
//! use pmcp::server::ui::UIResourceBuilder;
//! use pmcp::types::ui::UIMimeType;
//!
//! let resource = UIResourceBuilder::new("ui://charts/sales", "Sales Dashboard")
//!     .description("Interactive sales analytics dashboard")
//!     .html_template(r#"
//!         <!DOCTYPE html>
//!         <html>
//!         <head><title>Sales Dashboard</title></head>
//!         <body>
//!             <h1>Sales Analytics</h1>
//!             <div id="chart"></div>
//!         </body>
//!         </html>
//!     "#)
//!     .build()
//!     .expect("Failed to build UI resource");
//! ```

use crate::types::ui::{UIMimeType, UIResource, UIResourceContents};
use crate::{Error, Result};

/// Builder for UI resources with fluent API.
///
/// Provides methods to construct UI resources with HTML content,
/// either from inline strings or external files.
#[derive(Debug, Clone)]
pub struct UIResourceBuilder {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: UIMimeType,
    content: Option<String>,
    /// `(connect_domains, resource_domains)` — set via [`Self::csp`].
    csp: Option<(Vec<String>, Vec<String>)>,
}

impl UIResourceBuilder {
    /// Create a new UI resource builder.
    ///
    /// # Arguments
    ///
    /// * `uri` - The UI resource URI (must start with `ui://`)
    /// * `name` - Human-readable name for the UI resource
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let builder = UIResourceBuilder::new("ui://maps/venue", "Venue Map");
    /// ```
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            mime_type: UIMimeType::HtmlMcpApp,
            content: None,
            csp: None,
        }
    }

    /// Declare the widget's Content Security Policy domains.
    ///
    /// `connect_domains` are origins the widget may fetch/XHR/WebSocket to;
    /// `resource_domains` are origins it may load static assets from. Pass
    /// empty vectors to declare explicitly that the widget is fully
    /// self-contained — hosts treat a declared empty list as intent, whereas
    /// an absent declaration is an unknown.
    ///
    /// Emitted in the resource contents `_meta` in both shapes hosts consume:
    /// the MCP Apps standard `ui.csp` (`connectDomains`/`resourceDomains`)
    /// and the `ChatGPT` host key `openai/widgetCSP`
    /// (`connect_domains`/`resource_domains`).
    ///
    /// External *navigation* (link-outs) is not part of CSP — widgets should
    /// route those through the host bridge (e.g. `openExternal`).
    pub fn csp(mut self, connect_domains: Vec<String>, resource_domains: Vec<String>) -> Self {
        self.csp = Some((connect_domains, resource_domains));
        self
    }

    /// Set the description for this UI resource.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let builder = UIResourceBuilder::new("ui://maps/venue", "Venue Map")
    ///     .description("Interactive map showing conference venues");
    /// ```
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type for this UI resource.
    ///
    /// Currently only `UIMimeType::HtmlMcp` is supported.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    /// use pmcp::types::ui::UIMimeType;
    ///
    /// let builder = UIResourceBuilder::new("ui://test", "Test")
    ///     .mime_type(UIMimeType::HtmlMcp);
    /// ```
    pub fn mime_type(mut self, mime_type: UIMimeType) -> Self {
        self.mime_type = mime_type;
        self
    }

    /// Set HTML content from a string template.
    ///
    /// Use this for inline HTML content or dynamically generated templates.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let html = r#"
    ///     <!DOCTYPE html>
    ///     <html>
    ///     <body><h1>Hello World</h1></body>
    ///     </html>
    /// "#;
    ///
    /// let builder = UIResourceBuilder::new("ui://hello", "Hello World")
    ///     .html_template(html);
    /// ```
    pub fn html_template(mut self, html: impl Into<String>) -> Self {
        self.content = Some(html.into());
        self
    }

    /// Set HTML content from a file at compile time.
    ///
    /// This uses `include_str!()` macro to embed the file content.
    /// The path is relative to the crate root.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let builder = UIResourceBuilder::new("ui://dashboard", "Dashboard")
    ///     .html_file("ui/dashboard.html");
    /// ```
    ///
    /// Note: This method is typically used with `include_str!()` at the call site:
    ///
    /// ```rust,ignore
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let builder = UIResourceBuilder::new("ui://dashboard", "Dashboard")
    ///     .html_template(include_str!("../ui/dashboard.html"));
    /// ```
    pub fn html_file(mut self, content: impl Into<String>) -> Self {
        // Content is already loaded via include_str! at call site
        self.content = Some(content.into());
        self
    }

    /// Validate the URI format.
    fn validate_uri(&self) -> Result<()> {
        if !self.uri.starts_with("ui://") {
            return Err(Error::validation(format!(
                "UI resource URI must start with 'ui://', got: {}",
                self.uri
            )));
        }

        // Additional validation: ensure path is not empty
        let path = &self.uri[5..]; // Skip "ui://"
        if path.is_empty() {
            return Err(Error::validation(
                "UI resource URI must have a path after 'ui://'".to_string(),
            ));
        }

        Ok(())
    }

    /// Build the UI resource.
    ///
    /// Returns an error if:
    /// - The URI doesn't start with `ui://`
    /// - No content has been set
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let resource = UIResourceBuilder::new("ui://test", "Test")
    ///     .html_template("<html><body>Test</body></html>")
    ///     .build()
    ///     .expect("Failed to build resource");
    /// ```
    pub fn build(self) -> Result<UIResource> {
        // Validate URI
        self.validate_uri()?;

        // Ensure content is set
        if self.content.is_none() {
            return Err(Error::validation(
                "UI resource content must be set via html_template() or html_file()".to_string(),
            ));
        }

        let mut resource = UIResource::new(self.uri, self.name, self.mime_type);

        if let Some(description) = self.description {
            resource = resource.with_description(description);
        }

        Ok(resource)
    }

    /// Build the UI resource and its contents.
    ///
    /// Returns both the resource declaration and the contents that can be
    /// delivered to the MCP host when requested.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// let (resource, contents) = UIResourceBuilder::new("ui://test", "Test")
    ///     .html_template("<html><body>Test</body></html>")
    ///     .build_with_contents()
    ///     .expect("Failed to build");
    /// ```
    pub fn build_with_contents(self) -> Result<(UIResource, UIResourceContents)> {
        let uri = self.uri.clone();
        let content = self.content.clone().ok_or_else(|| {
            Error::validation(
                "UI resource content must be set via html_template() or html_file()".to_string(),
            )
        })?;

        let csp = self.csp.clone();
        let resource = self.build()?;

        let mut contents = UIResourceContents::html(uri.clone(), content);

        if let Some((connect, resources)) = csp {
            let mut meta = serde_json::Map::new();
            // The read-path merge inserts standard keys with entry().or_insert,
            // so a pre-populated "ui" object must carry resourceUri itself.
            meta.insert(
                "ui".to_string(),
                serde_json::json!({
                    "resourceUri": uri,
                    "csp": {
                        "connectDomains": connect,
                        "resourceDomains": resources,
                    }
                }),
            );
            meta.insert(
                "openai/widgetCSP".to_string(),
                serde_json::json!({
                    "connect_domains": connect,
                    "resource_domains": resources,
                }),
            );
            contents.meta = Some(meta);
        }

        Ok((resource, contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csp_declaration_lands_in_contents_meta() {
        let (_resource, contents) = UIResourceBuilder::new("ui://test/csp", "CSP Test")
            .html_template("<html><body>Test</body></html>")
            .csp(vec!["https://api.example.com".to_string()], Vec::new())
            .build_with_contents()
            .expect("Failed to build");

        let meta = contents.meta.expect("csp() must populate contents._meta");

        // MCP Apps standard shape — nested under "ui" WITH resourceUri, because
        // the read-path merge or_inserts per top-level key and must not clobber.
        let ui = meta.get("ui").expect("ui key present");
        assert_eq!(ui["resourceUri"], "ui://test/csp");
        assert_eq!(ui["csp"]["connectDomains"][0], "https://api.example.com");
        assert_eq!(
            ui["csp"]["resourceDomains"].as_array().map(Vec::len),
            Some(0),
            "empty list must be declared explicitly, not omitted"
        );

        // OpenAI host key — snake_case, top-level sibling of "ui".
        let openai = meta
            .get("openai/widgetCSP")
            .expect("openai/widgetCSP key present");
        assert_eq!(openai["connect_domains"][0], "https://api.example.com");
        assert_eq!(openai["resource_domains"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn test_no_csp_means_no_meta() {
        let (_resource, contents) = UIResourceBuilder::new("ui://test/plain", "Plain")
            .html_template("<html><body>Test</body></html>")
            .build_with_contents()
            .expect("Failed to build");
        assert!(
            contents.meta.is_none(),
            "builder without csp() must not invent contents meta"
        );
    }

    #[test]
    fn test_basic_builder() {
        let resource = UIResourceBuilder::new("ui://test", "Test Resource")
            .html_template("<html><body>Test</body></html>")
            .build()
            .expect("Failed to build");

        assert_eq!(resource.uri, "ui://test");
        assert_eq!(resource.name, "Test Resource");
        assert_eq!(resource.mime_type, "text/html;profile=mcp-app");
        assert_eq!(resource.description, None);
    }

    #[test]
    fn test_builder_with_description() {
        let resource = UIResourceBuilder::new("ui://test", "Test")
            .description("A test resource")
            .html_template("<html></html>")
            .build()
            .expect("Failed to build");

        assert_eq!(resource.description, Some("A test resource".to_string()));
    }

    #[test]
    fn test_builder_with_contents() {
        let (resource, contents) = UIResourceBuilder::new("ui://test", "Test")
            .html_template("<html><body>Hello</body></html>")
            .build_with_contents()
            .expect("Failed to build");

        assert_eq!(resource.uri, "ui://test");
        assert_eq!(contents.uri, "ui://test");
        assert_eq!(contents.mime_type, "text/html;profile=mcp-app");
        assert_eq!(
            contents.text,
            Some("<html><body>Hello</body></html>".to_string())
        );
    }

    #[test]
    fn test_invalid_uri_no_scheme() {
        let result = UIResourceBuilder::new("http://test", "Test")
            .html_template("<html></html>")
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must start with 'ui://'"));
    }

    #[test]
    fn test_invalid_uri_empty_path() {
        let result = UIResourceBuilder::new("ui://", "Test")
            .html_template("<html></html>")
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have a path after 'ui://'"));
    }

    #[test]
    fn test_missing_content() {
        let result = UIResourceBuilder::new("ui://test", "Test").build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("content must be set"));
    }

    #[test]
    fn test_html_file_method() {
        // Simulate include_str!() by passing string content
        let html = "<html><body>From file</body></html>";
        let resource = UIResourceBuilder::new("ui://from-file", "From File")
            .html_file(html)
            .build()
            .expect("Failed to build");

        assert_eq!(resource.uri, "ui://from-file");
    }

    #[test]
    fn test_mime_type_setting() {
        #[allow(deprecated)]
        let resource = UIResourceBuilder::new("ui://test", "Test")
            .mime_type(UIMimeType::HtmlMcp)
            .html_template("<html></html>")
            .build()
            .expect("Failed to build");

        assert_eq!(resource.mime_type, "text/html+mcp");
    }
}
