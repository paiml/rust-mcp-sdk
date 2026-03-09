//! Example: Server with resource support
//!
//! This example demonstrates:
//! - Creating a server that provides resources
//! - Implementing resource handlers
//! - Resource listing and reading
//! - URI template support

use async_trait::async_trait;
use pmcp::{
    types::{
        capabilities::ServerCapabilities, Content, ListResourcesResult, ReadResourceResult,
        ResourceInfo,
    },
    ResourceHandler, Server,
};
use std::collections::HashMap;

// Mock file system resource handler
struct FileSystemResources {
    files: HashMap<String, String>,
}

impl FileSystemResources {
    fn new() -> Self {
        let mut files = HashMap::new();

        // Simulate some files
        files.insert(
            "file://config/app.json".to_string(),
            r#"{
  "name": "Example App",
  "version": "1.0.0",
  "features": {
    "auth": true,
    "logging": true
  }
}"#
            .to_string(),
        );

        files.insert(
            "file://data/users.csv".to_string(),
            "id,name,email\n1,Alice,alice@example.com\n2,Bob,bob@example.com".to_string(),
        );

        files.insert(
            "file://logs/app.log".to_string(),
            "[2025-01-15 10:00:00] INFO: Application started\n[2025-01-15 10:00:05] DEBUG: Connected to database\n".to_string(),
        );

        Self { files }
    }
}

#[async_trait]
impl ResourceHandler for FileSystemResources {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        match self.files.get(uri) {
            Some(content) => Ok(ReadResourceResult::new(vec![Content::Text {
                text: content.clone(),
            }])),
            None => Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            )),
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let resources: Vec<ResourceInfo> = self
            .files
            .keys()
            .map(|uri| ResourceInfo {
                uri: uri.clone(),
                name: uri.rsplit('/').next().unwrap_or("").to_string(),
                description: Some(format!("Mock file at {}", uri)),
                mime_type: Some(guess_mime_type(uri)),
                meta: None,
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}

fn guess_mime_type(uri: &str) -> String {
    if uri.ends_with(".json") {
        "application/json".to_string()
    } else if uri.ends_with(".csv") {
        "text/csv".to_string()
    } else if uri.ends_with(".log") {
        "text/plain".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

// Template-based resource handler
struct TemplateResources;

#[async_trait]
impl ResourceHandler for TemplateResources {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        // Example: Handle parameterized URIs like "template://greeting/{name}"
        if uri.starts_with("template://greeting/") {
            let name = uri.strip_prefix("template://greeting/").unwrap_or("World");

            Ok(ReadResourceResult::new(vec![Content::Text {
                text: format!("Hello, {}! Welcome to MCP resources.", name),
            }]))
        } else if uri.starts_with("template://time/") {
            let timezone = uri.strip_prefix("template://time/").unwrap_or("UTC");

            Ok(ReadResourceResult::new(vec![Content::Text {
                text: format!(
                    "Current time in {}: {}",
                    timezone,
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                ),
            }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo {
                uri: "template://greeting/{name}".to_string(),
                name: "Greeting Template".to_string(),
                description: Some("Personalized greeting message".to_string()),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            },
            ResourceInfo {
                uri: "template://time/{timezone}".to_string(),
                name: "Time Template".to_string(),
                description: Some("Current time in specified timezone".to_string()),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            },
        ]))
    }
}

// Combine multiple resource handlers
struct CombinedResources {
    filesystem: FileSystemResources,
    templates: TemplateResources,
}

#[async_trait]
impl ResourceHandler for CombinedResources {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        if uri.starts_with("file://") {
            self.filesystem.read(uri, _extra).await
        } else if uri.starts_with("template://") {
            self.templates.read(uri, _extra).await
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        // Simple pagination: use cursor to determine which handler to list from
        match cursor.as_deref() {
            None | Some("") => {
                // List filesystem resources first
                let mut result = self.filesystem.list(None, _extra).await?;
                result.next_cursor = Some("templates".to_string());
                Ok(result)
            },
            Some("templates") => {
                // Then list template resources
                self.templates.list(None, _extra).await
            },
            _ => Ok(ListResourcesResult::new(vec![])),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Server Resources Example ===");
    println!("Starting server with file and template resources...\n");

    // Build server with resource support
    let server = Server::builder()
        .name("resource-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::resources_only())
        .resources(CombinedResources {
            filesystem: FileSystemResources::new(),
            templates: TemplateResources,
        })
        .build()?;

    println!("Server ready! Available resources:");
    println!("\n📁 File Resources:");
    println!("  - file://config/app.json");
    println!("  - file://data/users.csv");
    println!("  - file://logs/app.log");
    println!("\n🔗 Template Resources:");
    println!("  - template://greeting/{{name}}");
    println!("  - template://time/{{timezone}}");
    println!("\nListening on stdio...");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
