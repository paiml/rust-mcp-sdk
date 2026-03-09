//! Example showing ResourceWatcher for monitoring file system changes.

use async_trait::async_trait;
use pmcp::error::Result as PmcpResult;
#[cfg(feature = "resource-watcher")]
use pmcp::server::resource_watcher::{ResourceWatcher, ResourceWatcherBuilder};
use pmcp::server::{ResourceHandler, Server};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::protocol::{Content, ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::RequestHandlerExtra;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

/// File system resource handler with watching capabilities
struct FileSystemResourceHandler {
    base_dir: PathBuf,
    resources: Arc<RwLock<HashMap<String, ResourceInfo>>>,
    #[cfg(feature = "resource-watcher")]
    watcher: Arc<RwLock<Option<ResourceWatcher>>>,
}

impl FileSystemResourceHandler {
    fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            resources: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "resource-watcher")]
            watcher: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the resource watcher
    #[cfg(feature = "resource-watcher")]
    #[allow(dead_code)]
    async fn start_watching(
        &self,
        notification_tx: mpsc::Sender<pmcp::types::protocol::Notification>,
    ) -> PmcpResult<()> {
        let (tx, mut rx) = mpsc::channel(100);

        // Convert server notifications to general notifications
        let notification_tx_clone = notification_tx.clone();
        tokio::spawn(async move {
            while let Some(server_notif) = rx.recv().await {
                let notif = pmcp::types::protocol::Notification::Server(server_notif);
                let _ = notification_tx_clone.send(notif).await;
            }
        });

        // Create and start watcher
        let mut watcher = ResourceWatcherBuilder::new()
            .base_dir(&self.base_dir)
            .debounce(Duration::from_millis(500))
            .pattern("**/*.txt")
            .pattern("**/*.md")
            .pattern("**/*.json")
            .ignore("**/.*")
            .ignore("**/node_modules/**")
            .max_resources(1000)
            .build(tx)?;

        watcher.start().await?;

        // Store watcher
        *self.watcher.write().await = Some(watcher);

        info!("Started watching directory: {:?}", self.base_dir);
        Ok(())
    }

    async fn scan_directory(&self) -> PmcpResult<Vec<ResourceInfo>> {
        use std::fs;

        let mut resources = Vec::new();
        let mut resource_map = self.resources.write().await;
        resource_map.clear();

        // Scan for files
        if let Ok(entries) = fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // Check if it's a supported file type
                        let mime_type = match path.extension().and_then(|e| e.to_str()) {
                            Some("txt") => Some("text/plain".to_string()),
                            Some("md") => Some("text/markdown".to_string()),
                            Some("json") => Some("application/json".to_string()),
                            _ => continue,
                        };

                        let uri = format!("file://{}", path.display());
                        let info = ResourceInfo {
                            uri: uri.clone(),
                            name: name.to_string(),
                            description: Some(format!("File resource: {}", name)),
                            mime_type,
                            meta: None,
                        };

                        resources.push(info.clone());
                        resource_map.insert(uri.clone(), info.clone());

                        // Add to watcher
                        #[cfg(feature = "resource-watcher")]
                        if let Some(watcher) = &*self.watcher.read().await {
                            let _ = watcher.add_resource(uri, info).await;
                        }
                    }
                }
            }
        }

        Ok(resources)
    }
}

#[async_trait]
impl ResourceHandler for FileSystemResourceHandler {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> PmcpResult<ReadResourceResult> {
        // Convert URI to path
        let path = if let Some(path_str) = uri.strip_prefix("file://") {
            PathBuf::from(path_str)
        } else {
            return Err(pmcp::error::Error::not_found("Invalid URI scheme"));
        };

        // Read file content
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| pmcp::error::Error::not_found(format!("Failed to read file: {}", e)))?;

        let _mime_type = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| match ext {
                "txt" => Some("text/plain"),
                "md" => Some("text/markdown"),
                "json" => Some("application/json"),
                _ => None,
            })
            .unwrap_or("text/plain");

        Ok(ReadResourceResult::new(vec![Content::Text {
            text: content,
        }]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> PmcpResult<ListResourcesResult> {
        let resources = self.scan_directory().await?;

        Ok(ListResourcesResult::new(resources))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get directory to watch (current directory by default)
    let watch_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    info!("Starting resource watcher example");
    info!("Watching directory: {:?}", watch_dir);

    // Create resource handler
    let handler = FileSystemResourceHandler::new(watch_dir);

    // Create server
    let server = Server::builder()
        .name("resource-watcher-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(pmcp::types::capabilities::ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        })
        .resources(handler)
        .build()?;

    #[cfg(feature = "resource-watcher")]
    {
        info!("\nResourceWatcher features:");
        info!("- Monitors .txt, .md, and .json files");
        info!("- Sends notifications when files change");
        info!("- Debounces changes (500ms)");
        info!("- Ignores hidden files and node_modules");

        // Note: In a real implementation, you would get the notification channel
        // from the server and start the watcher. This is a simplified example.
        info!("\nTo test file watching:");
        info!("1. Create or modify .txt, .md, or .json files in the watched directory");
        info!("2. The server will send resource update notifications");
        info!("3. Clients subscribed to resources will receive updates");
    }

    #[cfg(not(feature = "resource-watcher"))]
    {
        info!("\nResourceWatcher is not enabled. To enable it, compile with:");
        info!("  cargo run --example 18_resource_watcher --features resource-watcher");
    }

    info!("\nStarting MCP server on stdio...");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
