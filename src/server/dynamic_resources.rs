//! Dynamic resource provider system for pattern-based resource routing.
//!
//! This module provides a trait-based system for handling dynamic resources using URI templates,
//! eliminating the need for manual URI parsing and pattern matching code.
//!
//! # Architecture
//!
//! - **Static Resources**: Fast O(1) HashMap lookup for fixed URIs
//! - **Dynamic Providers**: Pattern-based routing using RFC 6570 URI templates
//! - **Priority System**: Multiple providers can handle the same pattern with configurable priority
//! - **Template Matching**: Automatic parameter extraction from URIs
//!
//! # Examples
//!
//! ```rust
//! use pmcp::server::dynamic_resources::{DynamicResourceProvider, UriParams, RequestContext};
//! use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};
//! use pmcp::Result;
//! use async_trait::async_trait;
//!
//! struct DatasetProvider;
//!
//! #[async_trait]
//! impl DynamicResourceProvider for DatasetProvider {
//!     fn templates(&self) -> Vec<ResourceTemplate> {
//!         vec![
//!             ResourceTemplate {
//!                 uri_template: "datasets://{id}/schema".parse().unwrap(),
//!                 name: "Dataset Schema".to_string(),
//!                 description: Some("Schema for a dataset".to_string()),
//!                 mime_type: Some("application/json".to_string()),
//!             }
//!         ]
//!     }
//!
//!     async fn fetch(
//!         &self,
//!         uri: &str,
//!         params: UriParams,
//!         _context: RequestContext,
//!     ) -> Result<ReadResourceResult> {
//!         let id = params.get("id").unwrap();
//!         Ok(ReadResourceResult::new(vec![Content::Text {
//!             text: format!("Schema for dataset {}", id),
//!         }]))
//!     }
//! }
//! ```

use crate::server::cancellation::RequestHandlerExtra;
use crate::shared::uri_template::UriTemplate;
use crate::types::{ReadResourceResult, ResourceTemplate};
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Parameters extracted from a URI template match.
///
/// Contains the variable bindings from matching a URI against a template.
/// For example, matching `datasets://{id}/schema` against `datasets://123/schema`
/// would produce `{"id": "123"}`.
#[derive(Debug, Clone)]
pub struct UriParams {
    variables: HashMap<String, String>,
}

impl UriParams {
    /// Create new URI parameters from a variable map.
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self { variables }
    }

    /// Get a parameter value by name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::dynamic_resources::UriParams;
    /// use std::collections::HashMap;
    ///
    /// let mut vars = HashMap::new();
    /// vars.insert("id".to_string(), "123".to_string());
    /// let params = UriParams::new(vars);
    ///
    /// assert_eq!(params.get("id"), Some(&"123".to_string()));
    /// assert_eq!(params.get("missing"), None);
    /// ```
    pub fn get(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    /// Get a parameter value with a default.
    pub fn get_or(&self, name: &str, default: &str) -> String {
        self.variables
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    /// Check if a parameter exists.
    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Get all parameter names.
    pub fn keys(&self) -> Vec<&String> {
        self.variables.keys().collect()
    }

    /// Get all parameter values.
    pub fn values(&self) -> Vec<&String> {
        self.variables.values().collect()
    }

    /// Get the number of parameters.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Check if parameters are empty.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Convert to inner `HashMap`.
    pub fn into_inner(self) -> HashMap<String, String> {
        self.variables
    }
}

/// Request context for dynamic resource providers.
///
/// Provides access to request metadata, authentication, and session information.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Session ID if available
    pub session_id: Option<String>,
    /// Request metadata from the MCP protocol
    pub metadata: HashMap<String, String>,
    /// Raw request handler extra data
    pub extra: RequestHandlerExtra,
}

impl RequestContext {
    /// Create a new request context.
    pub fn new(extra: RequestHandlerExtra) -> Self {
        Self {
            session_id: None,
            metadata: HashMap::new(),
            extra,
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Trait for dynamic resource providers using URI template matching.
///
/// Implement this trait to handle resources with dynamic URIs like `datasets://{id}/schema`.
/// The trait automatically handles URI parsing, parameter extraction, and routing.
///
/// # Priority
///
/// When multiple providers match the same URI, they are executed in priority order
/// (lower priority values execute first). The default priority is 50.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::dynamic_resources::{DynamicResourceProvider, UriParams, RequestContext};
/// use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};
/// use pmcp::Result;
/// use async_trait::async_trait;
///
/// struct FileProvider {
///     base_path: String,
/// }
///
/// #[async_trait]
/// impl DynamicResourceProvider for FileProvider {
///     fn templates(&self) -> Vec<ResourceTemplate> {
///         vec![
///             ResourceTemplate {
///                 uri_template: "file://{path}".parse().unwrap(),
///                 name: "File Resource".to_string(),
///                 description: Some("Access to file system".to_string()),
///                 mime_type: None,
///             }
///         ]
///     }
///
///     fn priority(&self) -> i32 {
///         100 // Lower priority than default
///     }
///
///     async fn fetch(
///         &self,
///         _uri: &str,
///         params: UriParams,
///         _context: RequestContext,
///     ) -> Result<ReadResourceResult> {
///         let path = params.get("path").unwrap();
///         let full_path = format!("{}/{}", self.base_path, path);
///
///         // Read file and return contents...
///         Ok(ReadResourceResult::new(vec![Content::Text {
///             text: format!("Contents of {}", full_path),
///         }]))
///     }
/// }
/// ```
#[async_trait]
pub trait DynamicResourceProvider: Send + Sync {
    /// Return URI templates this provider handles.
    ///
    /// The templates use RFC 6570 syntax for variable extraction.
    /// Common patterns:
    /// - `{var}` - Simple variable
    /// - `{/path}` - Path segment
    /// - `{?query}` - Query parameter
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::types::ResourceTemplate;
    ///
    /// let templates = vec![
    ///     ResourceTemplate {
    ///         uri_template: "datasets://{id}/schema".parse().unwrap(),
    ///         name: "Dataset Schema".to_string(),
    ///         description: Some("Schema for a dataset".to_string()),
    ///         mime_type: Some("application/json".to_string()),
    ///     },
    ///     ResourceTemplate {
    ///         uri_template: "datasets://{id}/preview".parse().unwrap(),
    ///         name: "Dataset Preview".to_string(),
    ///         description: Some("Preview of dataset contents".to_string()),
    ///         mime_type: Some("text/plain".to_string()),
    ///     },
    /// ];
    /// ```
    fn templates(&self) -> Vec<ResourceTemplate>;

    /// Fetch a resource with extracted URI parameters.
    ///
    /// Called when a URI matches one of this provider's templates.
    /// The `params` argument contains extracted template variables.
    ///
    /// # Arguments
    ///
    /// * `uri` - The full URI being requested
    /// * `params` - Extracted template variables (e.g., `{id}` -> `"123"`)
    /// * `context` - Request context with session and metadata
    ///
    /// # Errors
    ///
    /// Return an error if:
    /// - The resource doesn't exist
    /// - Parameters are invalid
    /// - An internal error occurs
    async fn fetch(
        &self,
        uri: &str,
        params: UriParams,
        context: RequestContext,
    ) -> Result<ReadResourceResult>;

    /// Priority for conflict resolution (lower = higher priority).
    ///
    /// When multiple providers match the same URI, they are tried in priority order.
    /// Default priority is 50. Use lower values for higher priority providers.
    fn priority(&self) -> i32 {
        50
    }
}

/// Internal helper for matched dynamic resources.
#[derive(Clone)]
pub(crate) struct MatchedProvider {
    pub(crate) provider: Arc<dyn DynamicResourceProvider>,
    pub(crate) params: UriParams,
    #[allow(dead_code)] // Reserved for future use (metadata, logging)
    pub(crate) template: ResourceTemplate,
}

/// Internal helper to store a template with its parsed `UriTemplate`.
struct ParsedTemplate {
    template: ResourceTemplate,
    parsed: UriTemplate,
}

/// Internal helper for routing resources to providers.
pub(crate) struct ResourceRouter {
    providers: Vec<Arc<dyn DynamicResourceProvider>>,
    // Cached parsed templates for efficient matching
    parsed_templates: Vec<(Arc<dyn DynamicResourceProvider>, ParsedTemplate)>,
}

impl ResourceRouter {
    /// Create a new resource router.
    pub(crate) fn new() -> Self {
        Self {
            providers: Vec::new(),
            parsed_templates: Vec::new(),
        }
    }

    /// Add a dynamic provider.
    pub(crate) fn add_provider(&mut self, provider: Arc<dyn DynamicResourceProvider>) {
        // Parse and cache templates for efficient matching
        for template in provider.templates() {
            if let Ok(parsed) = UriTemplate::new(&template.uri_template) {
                self.parsed_templates.push((
                    Arc::clone(&provider),
                    ParsedTemplate {
                        template: template.clone(),
                        parsed,
                    },
                ));
            } else {
                tracing::warn!("Failed to parse URI template: {}", template.uri_template);
            }
        }

        self.providers.push(provider);
        // Sort by priority (lower = higher priority)
        self.providers.sort_by_key(|p| p.priority());

        // Re-sort parsed templates by provider priority
        self.parsed_templates
            .sort_by_key(|(provider, _)| provider.priority());
    }

    /// Try to match a URI against all registered providers.
    ///
    /// Returns the first provider that matches, or None if no match.
    pub(crate) fn match_uri(&self, uri: &str) -> Option<MatchedProvider> {
        for (provider, parsed_template) in &self.parsed_templates {
            if let Some(variables) = parsed_template.parsed.match_uri(uri) {
                return Some(MatchedProvider {
                    provider: Arc::clone(provider),
                    params: UriParams::new(variables),
                    template: parsed_template.template.clone(),
                });
            }
        }
        None
    }

    /// Get all templates from all providers.
    pub(crate) fn all_templates(&self) -> Vec<ResourceTemplate> {
        self.providers.iter().flat_map(|p| p.templates()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Content;

    #[test]
    fn test_uri_params() {
        let mut vars = HashMap::new();
        vars.insert("id".to_string(), "123".to_string());
        vars.insert("type".to_string(), "schema".to_string());

        let params = UriParams::new(vars);

        assert_eq!(params.get("id"), Some(&"123".to_string()));
        assert_eq!(params.get("type"), Some(&"schema".to_string()));
        assert_eq!(params.get("missing"), None);
        assert_eq!(params.get_or("missing", "default"), "default");
        assert!(params.contains("id"));
        assert!(!params.contains("missing"));
        assert_eq!(params.len(), 2);
        assert!(!params.is_empty());
    }

    #[test]
    fn test_request_context() {
        use tokio_util::sync::CancellationToken;
        let extra = RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
        let context = RequestContext::new(extra)
            .with_session_id("session-123")
            .with_metadata("user", "alice");

        assert_eq!(context.session_id, Some("session-123".to_string()));
        assert_eq!(context.get_metadata("user"), Some(&"alice".to_string()));
        assert_eq!(context.get_metadata("missing"), None);
    }

    struct TestProvider {
        priority: i32,
    }

    #[async_trait]
    impl DynamicResourceProvider for TestProvider {
        fn templates(&self) -> Vec<ResourceTemplate> {
            vec![ResourceTemplate {
                uri_template: "test://{id}".to_string(),
                name: "Test Resource".to_string(),
                description: Some("A test resource".to_string()),
                mime_type: Some("text/plain".to_string()),
            }]
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        async fn fetch(
            &self,
            _uri: &str,
            params: UriParams,
            _context: RequestContext,
        ) -> Result<ReadResourceResult> {
            let id = params.get("id").unwrap();
            Ok(ReadResourceResult {
                contents: vec![Content::Text {
                    text: format!("Resource {}", id),
                }],
            })
        }
    }

    #[test]
    fn test_resource_router_matching() {
        let mut router = ResourceRouter::new();
        router.add_provider(Arc::new(TestProvider { priority: 50 }));

        let matched = router.match_uri("test://123");
        assert!(matched.is_some());

        let matched = matched.unwrap();
        assert_eq!(matched.params.get("id"), Some(&"123".to_string()));
        assert_eq!(matched.template.name, "Test Resource");

        let no_match = router.match_uri("other://123");
        assert!(no_match.is_none());
    }

    #[test]
    fn test_resource_router_priority() {
        let mut router = ResourceRouter::new();

        // Add providers in reverse priority order
        router.add_provider(Arc::new(TestProvider { priority: 100 }));
        router.add_provider(Arc::new(TestProvider { priority: 10 }));
        router.add_provider(Arc::new(TestProvider { priority: 50 }));

        // Verify they're sorted by priority
        assert_eq!(router.providers[0].priority(), 10);
        assert_eq!(router.providers[1].priority(), 50);
        assert_eq!(router.providers[2].priority(), 100);
    }

    #[tokio::test]
    async fn test_provider_fetch() {
        use tokio_util::sync::CancellationToken;
        let provider = TestProvider { priority: 50 };
        let mut vars = HashMap::new();
        vars.insert("id".to_string(), "456".to_string());
        let params = UriParams::new(vars);
        let extra = RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
        let context = RequestContext::new(extra);

        let result = provider.fetch("test://456", params, context).await.unwrap();

        match &result.contents[0] {
            Content::Text { text } => assert_eq!(text, "Resource 456"),
            _ => panic!("Expected text content"),
        }
    }
}
