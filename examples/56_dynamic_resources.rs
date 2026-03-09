//! Example 56: Dynamic Resource Providers
//!
//! Demonstrates the Dynamic Resource Provider pattern for handling resources
//! with URI templates instead of manual pattern matching.
//!
//! This example shows how to:
//! - Create a dynamic resource provider with URI templates
//! - Handle multiple URI patterns with a single provider
//! - Extract parameters from URIs automatically
//! - Combine static and dynamic resources in one collection
//! - Use priority to control provider ordering
//!
//! **Before**: Manual URI parsing (150+ lines)
//! ```ignore
//! async fn handle_resource(&self, uri: &str) -> Result<ResourceContent> {
//!     if uri.starts_with("datasets://") {
//!         let parts: Vec<&str> = uri.split('/').collect();
//!         if parts.len() >= 3 && parts[2] == "schema" {
//!             let id = parts[1];
//!             // validate id, fetch from database, handle errors...
//!         } else if parts.len() >= 3 && parts[2] == "preview" {
//!             // similar pattern repeated...
//!         }
//!     }
//! }
//! ```
//!
//! **After**: Declarative URI templates (clean and maintainable)
//! ```ignore
//! struct DatasetProvider;
//!
//! impl DynamicResourceProvider for DatasetProvider {
//!     fn templates(&self) -> Vec<ResourceTemplate> {
//!         vec![
//!             ResourceTemplate {
//!                 uri_template: "datasets://{id}/schema".to_string(),
//!                 ...
//!             }
//!         ]
//!     }
//!
//!     async fn fetch(&self, uri: &str, params: UriParams, ctx: RequestContext) -> Result<...> {
//!         let id = params.get("id").unwrap();
//!         // URI already validated, params extracted
//!     }
//! }
//! ```

use async_trait::async_trait;
use pmcp::server::dynamic_resources::{DynamicResourceProvider, RequestContext, UriParams};
use pmcp::server::simple_resources::{ResourceCollection, StaticResource};
use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};
use pmcp::{Result, Server};
use std::sync::Arc;

/// A dynamic resource provider for datasets.
///
/// Handles URIs like:
/// - datasets://{id}/schema - Returns the schema for a dataset
/// - datasets://{id}/preview - Returns a preview of the dataset
/// - datasets://{id}/stats - Returns statistics for the dataset
struct DatasetResourceProvider {
    // In a real application, this would be a database connection
    datasets: std::collections::HashMap<String, DatasetInfo>,
}

#[derive(Clone)]
struct DatasetInfo {
    id: String,
    name: String,
    row_count: usize,
    columns: Vec<String>,
}

impl DatasetResourceProvider {
    fn new() -> Self {
        let mut datasets = std::collections::HashMap::new();

        // Sample datasets
        datasets.insert(
            "users".to_string(),
            DatasetInfo {
                id: "users".to_string(),
                name: "User Database".to_string(),
                row_count: 1000,
                columns: vec![
                    "id".to_string(),
                    "name".to_string(),
                    "email".to_string(),
                    "created_at".to_string(),
                ],
            },
        );

        datasets.insert(
            "products".to_string(),
            DatasetInfo {
                id: "products".to_string(),
                name: "Product Catalog".to_string(),
                row_count: 5000,
                columns: vec![
                    "id".to_string(),
                    "name".to_string(),
                    "price".to_string(),
                    "category".to_string(),
                ],
            },
        );

        Self { datasets }
    }

    fn get_dataset(&self, id: &str) -> Result<&DatasetInfo> {
        self.datasets.get(id).ok_or_else(|| {
            pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("Dataset not found: {}", id),
            )
        })
    }
}

#[async_trait]
impl DynamicResourceProvider for DatasetResourceProvider {
    fn templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                uri_template: "datasets://{id}/schema".to_string(),
                name: "Dataset Schema".to_string(),
                description: Some("Schema definition for a dataset".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "datasets://{id}/preview".to_string(),
                name: "Dataset Preview".to_string(),
                description: Some("Preview of dataset contents (first 10 rows)".to_string()),
                mime_type: Some("text/plain".to_string()),
            },
            ResourceTemplate {
                uri_template: "datasets://{id}/stats".to_string(),
                name: "Dataset Statistics".to_string(),
                description: Some("Statistical summary of dataset".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    async fn fetch(
        &self,
        uri: &str,
        params: UriParams,
        _context: RequestContext,
    ) -> Result<ReadResourceResult> {
        // Extract the dataset ID from the URI template match
        let id = params.get("id").ok_or_else(|| {
            pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing dataset ID")
        })?;

        let dataset = self.get_dataset(id)?;

        // Determine which resource is being requested
        let content = if uri.contains("/schema") {
            // Return schema
            let schema = serde_json::json!({
                "id": dataset.id,
                "name": dataset.name,
                "columns": dataset.columns.iter().map(|col| {
                    serde_json::json!({
                        "name": col,
                        "type": "string"  // Simplified for demo
                    })
                }).collect::<Vec<_>>(),
            });

            Content::Text {
                text: serde_json::to_string_pretty(&schema).unwrap(),
            }
        } else if uri.contains("/preview") {
            // Return preview
            let preview = format!(
                "Dataset: {} ({})\nColumns: {}\nShowing first 10 of {} rows:\n\n{}\n...",
                dataset.name,
                dataset.id,
                dataset.columns.join(", "),
                dataset.row_count,
                dataset.columns.join(" | ")
            );

            Content::Text { text: preview }
        } else if uri.contains("/stats") {
            // Return statistics
            let stats = serde_json::json!({
                "id": dataset.id,
                "name": dataset.name,
                "row_count": dataset.row_count,
                "column_count": dataset.columns.len(),
            });

            Content::Text {
                text: serde_json::to_string_pretty(&stats).unwrap(),
            }
        } else {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("Unknown dataset resource: {}", uri),
            ));
        };

        Ok(ReadResourceResult::new(vec![content]))
    }

    fn priority(&self) -> i32 {
        50 // Default priority
    }
}

/// A file system resource provider with lower priority.
///
/// Demonstrates priority ordering - this provider only handles
/// URIs if no higher-priority provider matches.
struct FileSystemProvider {
    base_path: String,
}

impl FileSystemProvider {
    fn new(base_path: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
}

#[async_trait]
impl DynamicResourceProvider for FileSystemProvider {
    fn templates(&self) -> Vec<ResourceTemplate> {
        vec![ResourceTemplate {
            uri_template: "file://{path}".to_string(),
            name: "File Resource".to_string(),
            description: Some("Access to local files".to_string()),
            mime_type: None,
        }]
    }

    async fn fetch(
        &self,
        _uri: &str,
        params: UriParams,
        _context: RequestContext,
    ) -> Result<ReadResourceResult> {
        let path = params.get("path").ok_or_else(|| {
            pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing file path")
        })?;

        // In a real application, read the file here
        let content = format!("File contents from: {}/{}", self.base_path, path);

        Ok(ReadResourceResult::new(vec![Content::Text {
            text: content,
        }]))
    }

    fn priority(&self) -> i32 {
        100 // Lower priority than datasets (higher number = lower priority)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Dynamic Resource Provider Example ===\n");

    // Create resource collection with both static and dynamic resources
    let resources = ResourceCollection::new()
        // Add static resources (fixed URIs)
        .add_static(StaticResource::new_text(
            "config://readme",
            "This is a static resource with a fixed URI",
        ))
        // Add dynamic providers (URI templates)
        .add_dynamic_provider(Arc::new(DatasetResourceProvider::new()))
        .add_dynamic_provider(Arc::new(FileSystemProvider::new("/data")));

    // Create server with the resource collection
    let _server = Server::builder()
        .name("dynamic-resources-demo")
        .version("1.0.0")
        .resources(resources)
        .build()?;

    println!("Server created with dynamic resource providers!");
    println!("\nAvailable resource patterns:");
    println!("  - datasets://{{id}}/schema");
    println!("  - datasets://{{id}}/preview");
    println!("  - datasets://{{id}}/stats");
    println!("  - file://{{path}}");
    println!("  - config://readme (static)");

    println!("\nExample URIs:");
    println!("  - datasets://users/schema");
    println!("  - datasets://products/preview");
    println!("  - file://logs/app.log");

    println!("\n✅ Dynamic resources eliminate manual URI parsing!");
    println!("   - Automatic parameter extraction");
    println!("   - Priority-based routing");
    println!("   - Type-safe provider interface");
    println!("   - Template discovery for clients");

    Ok(())
}
