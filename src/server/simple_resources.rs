//! Simple resource implementations with builder pattern support.

use crate::types::{
    Content, ListResourcesResult, ReadResourceResult, ResourceInfo, UIResource, UIResourceContents,
};
use crate::Result;
use async_trait::async_trait;
use base64::Engine;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use super::cancellation::RequestHandlerExtra;
use super::dynamic_resources::{DynamicResourceProvider, RequestContext, ResourceRouter};
use super::ResourceHandler;

/// A static resource that returns fixed content.
#[derive(Debug, Clone)]
pub struct StaticResource {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    content: Content,
}

impl StaticResource {
    /// Create a new static resource with URI and text content.
    pub fn new_text(uri: impl Into<String>, content: impl Into<String>) -> Self {
        let uri = uri.into();
        let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();

        Self {
            uri: uri.clone(),
            name,
            description: None,
            mime_type: Some("text/plain".to_string()),
            content: Content::resource_with_text(uri, content, "text/plain"),
        }
    }

    /// Create a new static resource with URI and image content.
    pub fn new_image(uri: impl Into<String>, data: &[u8], mime_type: impl Into<String>) -> Self {
        let uri = uri.into();
        let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();
        let mime_type = mime_type.into();

        Self {
            uri: uri.clone(),
            name,
            description: None,
            mime_type: Some(mime_type.clone()),
            // NOTE: this encodes the image into `text`, not `blob`, which is what
            // this constructor did before `blob` existed. Preserved deliberately —
            // moving it to `Content::resource_with_blob` would change the wire
            // output for every existing caller and is a compatibility decision,
            // not a cleanup. See the phase 118.1 /simplify review (F2).
            content: Content::resource_with_text(
                uri,
                base64::prelude::BASE64_STANDARD.encode(data),
                mime_type,
            ),
        }
    }

    /// Set the resource name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the resource description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Get the resource info.
    pub fn info(&self) -> ResourceInfo {
        let mut info = ResourceInfo::new(&self.uri, &self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if let Some(mime) = &self.mime_type {
            info = info.with_mime_type(mime);
        }
        info
    }

    /// Get the resource URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

/// A collection of resources that can be managed together.
///
/// Supports both static resources (fixed URIs), UI resources (MCP Apps Extension),
/// and dynamic providers (URI templates). Static and UI resources are checked first
/// for O(1) lookup, then dynamic providers are tried in priority order.
pub struct ResourceCollection {
    resources: HashMap<String, Arc<StaticResource>>,
    router: ResourceRouter,
    ui_resources: HashMap<String, (UIResource, UIResourceContents)>,
}

impl fmt::Debug for ResourceCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceCollection")
            .field(
                "static_resources",
                &self.resources.keys().collect::<Vec<_>>(),
            )
            .field("dynamic_providers", &self.router.all_templates().len())
            .finish()
    }
}

impl Default for ResourceCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceCollection {
    /// Create a new empty resource collection.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::ResourceCollection;
    ///
    /// let collection = ResourceCollection::new();
    /// ```
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            router: ResourceRouter::new(),
            ui_resources: HashMap::new(),
        }
    }

    /// Add a static resource to the collection.
    ///
    /// Static resources are checked first for O(1) lookup performance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::{ResourceCollection, StaticResource};
    ///
    /// let collection = ResourceCollection::new()
    ///     .add_resource(StaticResource::new_text("file://readme.txt", "Hello world"));
    /// ```
    pub fn add_resource(mut self, resource: StaticResource) -> Self {
        self.resources
            .insert(resource.uri.clone(), Arc::new(resource));
        self
    }

    /// Add a static resource by reference (avoids cloning).
    pub fn add_static(mut self, resource: StaticResource) -> Self {
        self.resources
            .insert(resource.uri.clone(), Arc::new(resource));
        self
    }

    /// Add multiple static resources to the collection.
    pub fn add_resources(mut self, resources: Vec<StaticResource>) -> Self {
        for resource in resources {
            self.resources
                .insert(resource.uri.clone(), Arc::new(resource));
        }
        self
    }

    /// Add a UI resource to the collection (MCP Apps Extension).
    ///
    /// UI resources use the `ui://` URI scheme and provide HTML-based interactive
    /// interfaces for tools. They are part of the MCP Apps Extension (SEP-1865).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::ResourceCollection;
    /// use pmcp::server::ui::UIResourceBuilder;
    ///
    /// # fn main() -> pmcp::Result<()> {
    /// let (ui_resource, ui_contents) = UIResourceBuilder::new(
    ///     "ui://charts/sales",
    ///     "Sales Chart",
    /// )
    /// .description("Interactive sales data visualization")
    /// .html_template("<html><body>Chart goes here</body></html>")
    /// .build_with_contents()?;
    ///
    /// let collection = ResourceCollection::new()
    ///     .add_ui_resource(ui_resource, ui_contents);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_ui_resource(mut self, resource: UIResource, contents: UIResourceContents) -> Self {
        // Apply ext-apps CDN shim so widgets work in hosts that block external
        // script loading (e.g. Claude Desktop's Electron iframe CSP).
        #[cfg(feature = "mcp-apps")]
        let contents = {
            if let Some(text) = contents.text {
                let shimmed = crate::server::mcp_apps::inline_ext_apps_shim(&text);
                UIResourceContents {
                    text: Some(shimmed.into_owned()),
                    ..contents
                }
            } else {
                contents
            }
        };
        self.ui_resources
            .insert(resource.uri.clone(), (resource, contents));
        self
    }

    /// Add a dynamic resource provider to the collection.
    ///
    /// Dynamic providers handle URI patterns using RFC 6570 templates.
    /// They are checked after static resources, in priority order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::ResourceCollection;
    /// use pmcp::server::dynamic_resources::{DynamicResourceProvider, UriParams, RequestContext};
    /// use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DatasetProvider;
    ///
    /// #[async_trait]
    /// impl DynamicResourceProvider for DatasetProvider {
    ///     fn templates(&self) -> Vec<ResourceTemplate> {
    ///         vec![
    ///             ResourceTemplate::new("datasets://{id}/schema", "Dataset Schema")
    ///                 .with_description("Schema for a dataset")
    ///                 .with_mime_type("application/json"),
    ///         ]
    ///     }
    ///
    ///     async fn fetch(
    ///         &self,
    ///         _uri: &str,
    ///         params: UriParams,
    ///         _context: RequestContext,
    ///     ) -> pmcp::Result<ReadResourceResult> {
    ///         let id = params.get("id").unwrap();
    ///         Ok(ReadResourceResult::new(vec![Content::text(format!(
    ///             "Schema for dataset {}",
    ///             id
    ///         ))]))
    ///     }
    /// }
    ///
    /// let collection = ResourceCollection::new()
    ///     .add_dynamic_provider(Arc::new(DatasetProvider));
    /// ```
    pub fn add_dynamic_provider(mut self, provider: Arc<dyn DynamicResourceProvider>) -> Self {
        self.router.add_provider(provider);
        self
    }

    /// Get a static resource by URI.
    pub fn get(&self, uri: &str) -> Option<&Arc<StaticResource>> {
        self.resources.get(uri)
    }

    /// List all resources (static and dynamic templates).
    pub fn list(&self) -> Vec<ResourceInfo> {
        let mut infos: Vec<ResourceInfo> = self
            .resources
            .values()
            .map(|resource| resource.info())
            .collect();

        // Add UI resources with `_meta.ui` descriptor (same format as tools/list)
        for (ui_resource, _contents) in self.ui_resources.values() {
            let mut info = ResourceInfo::new(&ui_resource.uri, &ui_resource.name)
                .with_mime_type(&ui_resource.mime_type);
            if let Some(desc) = &ui_resource.description {
                info = info.with_description(desc);
            }
            info.meta = crate::types::ui::build_ui_meta(Some(&ui_resource.uri));
            infos.push(info);
        }

        // Add dynamic templates as resources
        for template in self.router.all_templates() {
            let mut info = ResourceInfo::new(&template.uri_template, &template.name);
            if let Some(desc) = &template.description {
                info = info.with_description(desc);
            }
            if let Some(mime) = &template.mime_type {
                info = info.with_mime_type(mime);
            }
            infos.push(info);
        }

        infos
    }
}

#[async_trait]
impl ResourceHandler for ResourceCollection {
    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // Try static resources first (fast O(1) lookup)
        if let Some(resource) = self.resources.get(uri) {
            return Ok(ReadResourceResult {
                contents: vec![resource.content.clone()],
                _meta: None,
                ttl_ms: None,
                cache_scope: None,
            });
        }

        // Try UI resources (ui:// scheme)
        if let Some((resource, contents)) = self.ui_resources.get(uri) {
            // Merge contents._meta with the UI descriptor metadata so the
            // resources/read response includes `_meta.ui` — required by
            // Claude Desktop to recognise this as a renderable MCP App.
            let meta = contents
                .meta
                .clone()
                .or_else(|| Some(serde_json::Map::new()))
                .map(|mut m| {
                    // Ensure _meta.ui.resourceUri is present (same as resources/list)
                    let ui_meta = crate::types::ui::build_ui_meta(Some(&resource.uri));
                    if let Some(ui_map) = ui_meta {
                        for (k, v) in ui_map {
                            m.entry(k).or_insert(v);
                        }
                    }
                    m
                });
            return Ok(ReadResourceResult {
                contents: vec![Content::Resource {
                    uri: contents.uri.clone(),
                    text: contents.text.clone(),
                    blob: None,
                    mime_type: Some(contents.mime_type.clone()),
                    annotations: None,
                    meta,
                }],
                _meta: None,
                ttl_ms: None,
                cache_scope: None,
            });
        }

        // Try dynamic providers (pattern matching)
        if let Some(matched) = self.router.match_uri(uri) {
            let context = RequestContext::new(extra);
            return matched.provider.fetch(uri, matched.params, context).await;
        }

        // Not found
        Err(crate::Error::protocol(
            crate::ErrorCode::INVALID_PARAMS,
            format!("Resource not found: {}", uri),
        ))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        Ok(ListResourcesResult {
            resources: self.list(),
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }
}

/// A dynamic resource handler that uses callbacks.
pub struct DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    read_handler: R,
    list_handler: L,
}

impl<R, L> fmt::Debug for DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicResourceHandler")
            .field("read_handler", &"<function>")
            .field("list_handler", &"<function>")
            .finish()
    }
}

impl<R, L> DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    /// Create a new dynamic resource handler.
    pub fn new(read_handler: R, list_handler: L) -> Self {
        Self {
            read_handler,
            list_handler,
        }
    }
}

#[async_trait]
impl<R, L> ResourceHandler for DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        (self.read_handler)(uri, extra).await
    }

    async fn list(
        &self,
        cursor: Option<String>,
        extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        (self.list_handler)(cursor, extra).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_resource_new_text_returns_resource_content() {
        let resource = StaticResource::new_text("test://doc", "Hello world");

        // Verify content is Content::Resource, not Content::Text
        match &resource.content {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "test://doc");
                assert_eq!(text.as_ref().unwrap(), "Hello world");
                assert_eq!(mime_type.as_ref().unwrap(), "text/plain");
            },
            _ => panic!("Expected Content::Resource, got {:?}", resource.content),
        }

        assert_eq!(resource.uri(), "test://doc");
    }

    #[test]
    fn test_static_resource_new_image_returns_resource_content() {
        let image_data = b"fake image data";
        let resource = StaticResource::new_image("test://image.png", image_data, "image/png");

        // Verify content is Content::Resource with base64 encoded data
        match &resource.content {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "test://image.png");

                // Verify base64 encoding
                let expected_base64 = base64::prelude::BASE64_STANDARD.encode(image_data);
                assert_eq!(text.as_ref().unwrap(), &expected_base64);
                assert_eq!(mime_type.as_ref().unwrap(), "image/png");
            },
            _ => panic!("Expected Content::Resource, got {:?}", resource.content),
        }
    }

    #[test]
    fn test_static_resource_with_custom_mime_type() {
        let resource = StaticResource::new_text("test://data", "{ \"key\": \"value\" }")
            .with_mime_type("application/json");

        match &resource.content {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "test://data");
                assert_eq!(text.as_ref().unwrap(), "{ \"key\": \"value\" }");
                // Note: mime_type in struct is updated, but content still has original
                assert_eq!(mime_type.as_ref().unwrap(), "text/plain");
            },
            _ => panic!("Expected Content::Resource"),
        }

        // Verify the struct's mime_type field was updated
        assert_eq!(resource.mime_type.as_ref().unwrap(), "application/json");
    }

    #[tokio::test]
    async fn test_resource_collection_read_returns_resource_content() {
        use tokio_util::sync::CancellationToken;

        let collection = ResourceCollection::new().add_resource(StaticResource::new_text(
            "maps://instructions",
            "Game instructions",
        ));

        let extra = RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());
        let result = collection.read("maps://instructions", extra).await.unwrap();

        assert_eq!(result.contents.len(), 1);

        // Verify the returned content has URI (MCP protocol compliance)
        match &result.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "maps://instructions");
                assert_eq!(text.as_ref().unwrap(), "Game instructions");
                assert_eq!(mime_type.as_ref().unwrap(), "text/plain");
            },
            _ => panic!("Expected Content::Resource with URI field"),
        }
    }

    #[tokio::test]
    async fn test_resource_collection_read_image_returns_resource_content() {
        use tokio_util::sync::CancellationToken;

        let image_data = b"\x89PNG\r\n\x1a\n";
        let collection = ResourceCollection::new().add_resource(StaticResource::new_image(
            "test://logo.png",
            image_data,
            "image/png",
        ));

        let extra = RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());
        let result = collection.read("test://logo.png", extra).await.unwrap();

        assert_eq!(result.contents.len(), 1);

        match &result.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "test://logo.png");
                let expected_base64 = base64::prelude::BASE64_STANDARD.encode(image_data);
                assert_eq!(text.as_ref().unwrap(), &expected_base64);
                assert_eq!(mime_type.as_ref().unwrap(), "image/png");
            },
            _ => panic!("Expected Content::Resource with URI field"),
        }
    }

    #[test]
    fn test_static_resource_builder_pattern() {
        let resource = StaticResource::new_text("test://readme", "README content")
            .with_name("Project README")
            .with_description("Main documentation file")
            .with_mime_type("text/markdown");

        assert_eq!(resource.name, "Project README");
        assert_eq!(
            resource.description.as_ref().unwrap(),
            "Main documentation file"
        );
        assert_eq!(resource.mime_type.as_ref().unwrap(), "text/markdown");

        // Content should still have Resource type with URI
        match &resource.content {
            Content::Resource { uri, .. } => {
                assert_eq!(uri, "test://readme");
            },
            _ => panic!("Expected Content::Resource"),
        }
    }
}
