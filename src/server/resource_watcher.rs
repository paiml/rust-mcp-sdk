//! Resource watcher for monitoring file system changes.

use crate::error::{Error, ErrorCode, Result};
use crate::types::protocol::{ResourceInfo, ServerNotification};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for resource watching.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::resource_watcher::WatchConfig;
/// use std::time::Duration;
///
/// // Default configuration
/// let config = WatchConfig::default();
///
/// // Custom configuration
/// let custom_config = WatchConfig {
///     debounce: Duration::from_millis(1000),
///     patterns: vec!["**/*.rs".to_string(), "**/*.md".to_string()],
///     ignore_patterns: vec!["**/target/**".to_string()],
///     max_resources: 5000,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Debounce duration for file changes.
    pub debounce: Duration,
    /// Patterns to watch (glob patterns).
    pub patterns: Vec<String>,
    /// Patterns to ignore.
    pub ignore_patterns: Vec<String>,
    /// Maximum number of resources to track.
    pub max_resources: usize,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(500),
            patterns: vec!["**/*".to_string()],
            ignore_patterns: vec![
                "**/.git/**".to_string(),
                "**/node_modules/**".to_string(),
                "**/.DS_Store".to_string(),
                "**/target/**".to_string(),
            ],
            max_resources: 10000,
        }
    }
}

/// File change event.
#[derive(Debug, Clone)]
struct FileEvent {
    /// Path that changed.
    path: PathBuf,
    /// Time of the event.
    timestamp: Instant,
    /// Type of change.
    kind: FileEventKind,
}

/// Type of file change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileEventKind {
    Created,
    Modified,
    Deleted,
}

/// Resource watcher for monitoring file system changes.
pub struct ResourceWatcher {
    /// Watch configuration.
    config: WatchConfig,
    /// Base directory to watch.
    base_dir: PathBuf,
    /// Tracked resources by URI.
    resources: Arc<RwLock<HashMap<String, ResourceInfo>>>,
    /// Pending file events for debouncing.
    pending_events: Arc<RwLock<HashMap<PathBuf, FileEvent>>>,
    /// Channel for sending notifications.
    notification_tx: mpsc::Sender<ServerNotification>,
    /// Channel for receiving file events.
    event_rx: Arc<RwLock<Option<mpsc::Receiver<FileEvent>>>>,
    /// Shutdown signal.
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl std::fmt::Debug for ResourceWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceWatcher")
            .field("config", &self.config)
            .field("base_dir", &self.base_dir)
            .field("resources_count", &"<async>")
            .field("has_shutdown_tx", &self.shutdown_tx.is_some())
            .finish()
    }
}

/// Process file events with debouncing (helper function).
#[allow(clippy::cognitive_complexity)]
async fn process_events(
    resources: Arc<RwLock<HashMap<String, ResourceInfo>>>,
    pending_events: Arc<RwLock<HashMap<PathBuf, FileEvent>>>,
    notification_tx: mpsc::Sender<ServerNotification>,
    debounce: Duration,
    event_rx: Arc<RwLock<Option<mpsc::Receiver<FileEvent>>>>,
) {
    let mut timer = interval(Duration::from_millis(100));

    loop {
        timer.tick().await;

        // Check for new events
        if let Some(rx) = &mut *event_rx.write().await {
            while let Ok(event) = rx.try_recv() {
                let mut pending = pending_events.write().await;
                pending.insert(event.path.clone(), event);
            }
        }

        // Process debounced events
        let now = Instant::now();
        let mut events_to_process = Vec::new();

        {
            let mut pending = pending_events.write().await;
            pending.retain(|path, event| {
                if now.duration_since(event.timestamp) >= debounce {
                    events_to_process.push((path.clone(), event.kind));
                    false
                } else {
                    true
                }
            });
        }

        // Send notifications for processed events
        for (path, kind) in events_to_process {
            let uri = format!("file://{}", path.display());

            let resources = resources.read().await;
            if resources.contains_key(&uri) {
                debug!("Resource {:?} changed: {}", kind, uri);

                // Send resource update notification
                let notification = ServerNotification::ResourceUpdated(
                    crate::types::protocol::ResourceUpdatedParams { uri: uri.clone() },
                );

                if let Err(e) = notification_tx.send(notification).await {
                    error!("Failed to send resource update notification: {}", e);
                }
            }
        }
    }
}

impl ResourceWatcher {
    /// Create a new resource watcher.
    pub fn new(
        base_dir: impl AsRef<Path>,
        config: WatchConfig,
        notification_tx: mpsc::Sender<ServerNotification>,
    ) -> Self {
        Self {
            config,
            base_dir: base_dir.as_ref().to_path_buf(),
            resources: Arc::new(RwLock::new(HashMap::new())),
            pending_events: Arc::new(RwLock::new(HashMap::new())),
            notification_tx,
            event_rx: Arc::new(RwLock::new(None)),
            shutdown_tx: None,
        }
    }

    /// Start watching for changes.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting resource watcher for {:?}", self.base_dir);

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(1000);
        *self.event_rx.write().await = Some(event_rx);

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start file system watcher
        let base_dir = self.base_dir.clone();
        let patterns = self.config.patterns.clone();
        let ignore_patterns = self.config.ignore_patterns.clone();

        // Spawn file watcher task
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::watch_filesystem(
                base_dir,
                patterns,
                ignore_patterns,
                event_tx_clone,
                &mut shutdown_rx,
            )
            .await
            {
                error!("File system watcher error: {}", e);
            }
        });

        // Start event processor
        let resources = self.resources.clone();
        let pending_events = self.pending_events.clone();
        let notification_tx = self.notification_tx.clone();
        let debounce = self.config.debounce;
        let event_rx = self.event_rx.clone();

        tokio::spawn(async move {
            process_events(
                resources,
                pending_events,
                notification_tx,
                debounce,
                event_rx,
            )
            .await;
        });

        Ok(())
    }

    /// Stop watching for changes.
    pub async fn stop(&mut self) {
        info!("Stopping resource watcher");

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }

    /// Add a resource to watch.
    pub async fn add_resource(&self, uri: String, info: ResourceInfo) -> Result<()> {
        let mut resources = self.resources.write().await;

        if resources.len() >= self.config.max_resources {
            warn!("Resource limit reached, not adding {}", uri);
            return Ok(());
        }

        resources.insert(uri, info);
        Ok(())
    }

    /// Remove a resource from watching.
    pub async fn remove_resource(&self, uri: &str) -> Result<()> {
        let mut resources = self.resources.write().await;
        resources.remove(uri);
        Ok(())
    }

    /// Get all watched resources.
    pub async fn get_resources(&self) -> Vec<ResourceInfo> {
        let resources = self.resources.read().await;
        resources.values().cloned().collect()
    }

    /// Watch the file system for changes.
    async fn watch_filesystem(
        base_dir: PathBuf,
        patterns: Vec<String>,
        ignore_patterns: Vec<String>,
        event_tx: mpsc::Sender<FileEvent>,
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) -> Result<()> {
        #[cfg(feature = "resource-watcher")]
        {
            use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

            let (tx, mut rx) = mpsc::channel(1000);

            // Create watcher
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    if let Ok(event) = res {
                        let _ = tx.try_send(event);
                    }
                },
                Config::default(),
            )
            .map_err(|e| {
                Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to create watcher: {}", e),
                )
            })?;

            // Watch base directory
            watcher
                .watch(&base_dir, RecursiveMode::Recursive)
                .map_err(|e| {
                    Error::protocol(
                        ErrorCode::INTERNAL_ERROR,
                        format!("Failed to watch directory: {}", e),
                    )
                })?;

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("File watcher shutting down");
                        break;
                    }
                    Some(event) = rx.recv() => {
                        // Process notify event
                        let kind = match event.kind {
                            EventKind::Create(_) => FileEventKind::Created,
                            EventKind::Modify(_) => FileEventKind::Modified,
                            EventKind::Remove(_) => FileEventKind::Deleted,
                            _ => continue,
                        };

                        for path in event.paths {
                            // Check if path matches patterns
                            if !Self::matches_patterns(&path, &base_dir, &patterns, &ignore_patterns) {
                                continue;
                            }

                            let file_event = FileEvent {
                                path,
                                timestamp: Instant::now(),
                                kind,
                            };

                            if let Err(e) = event_tx.send(file_event).await {
                                error!("Failed to send file event: {}", e);
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        #[cfg(not(feature = "resource-watcher"))]
        {
            warn!("Resource watching is not enabled (requires 'resource-watcher' feature)");
            Ok(())
        }
    }

    /// Check if a path matches the watch patterns.
    #[cfg(feature = "resource-watcher")]
    fn matches_patterns(
        path: &Path,
        base_dir: &Path,
        patterns: &[String],
        ignore_patterns: &[String],
    ) -> bool {
        let Ok(relative_path) = path.strip_prefix(base_dir) else {
            return false;
        };

        let path_str = relative_path.to_string_lossy();

        // Check ignore patterns first
        for pattern in ignore_patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return false;
            }
        }

        // Check include patterns
        for pattern in patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return true;
            }
        }

        false
    }
}

/// Builder for creating a resource watcher.
#[derive(Debug)]
pub struct ResourceWatcherBuilder {
    base_dir: Option<PathBuf>,
    config: WatchConfig,
}

impl ResourceWatcherBuilder {
    /// Create a new resource watcher builder.
    pub fn new() -> Self {
        Self {
            base_dir: None,
            config: WatchConfig::default(),
        }
    }

    /// Set the base directory to watch.
    pub fn base_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.base_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Set the debounce duration.
    pub fn debounce(mut self, duration: Duration) -> Self {
        self.config.debounce = duration;
        self
    }

    /// Add a watch pattern.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.config.patterns.push(pattern.into());
        self
    }

    /// Add an ignore pattern.
    pub fn ignore(mut self, pattern: impl Into<String>) -> Self {
        self.config.ignore_patterns.push(pattern.into());
        self
    }

    /// Set the maximum number of resources to track.
    pub fn max_resources(mut self, max: usize) -> Self {
        self.config.max_resources = max;
        self
    }

    /// Build the resource watcher.
    pub fn build(
        self,
        notification_tx: mpsc::Sender<ServerNotification>,
    ) -> Result<ResourceWatcher> {
        let base_dir = self.base_dir.ok_or_else(|| {
            crate::error::Error::protocol(
                crate::error::ErrorCode::INVALID_PARAMS,
                "Base directory not specified",
            )
        })?;

        Ok(ResourceWatcher::new(base_dir, self.config, notification_tx))
    }
}

impl Default for ResourceWatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let base_dir = Path::new("/home/user/project");
        let patterns = vec!["**/*.rs".to_string(), "*.toml".to_string()];
        let ignore_patterns = vec!["**/target/**".to_string()];

        // Should match
        assert!(ResourceWatcher::matches_patterns(
            &base_dir.join("src/main.rs"),
            base_dir,
            &patterns,
            &ignore_patterns,
        ));

        assert!(ResourceWatcher::matches_patterns(
            &base_dir.join("Cargo.toml"),
            base_dir,
            &patterns,
            &ignore_patterns,
        ));

        // Should not match (wrong extension)
        assert!(!ResourceWatcher::matches_patterns(
            &base_dir.join("README.md"),
            base_dir,
            &patterns,
            &ignore_patterns,
        ));

        // Should not match (ignored)
        assert!(!ResourceWatcher::matches_patterns(
            &base_dir.join("target/debug/main.rs"),
            base_dir,
            &patterns,
            &ignore_patterns,
        ));
    }

    #[tokio::test]
    async fn test_resource_management() {
        let (tx, _rx) = mpsc::channel(10);
        let watcher = ResourceWatcher::new("/tmp", WatchConfig::default(), tx);

        // Add resource
        let info = ResourceInfo {
            uri: "file:///tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            description: None,
            mime_type: Some("text/plain".to_string()),
            meta: None,
        };

        watcher
            .add_resource("file:///tmp/test.txt".to_string(), info.clone())
            .await
            .unwrap();

        // Check it was added
        let resources = watcher.get_resources().await;
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "file:///tmp/test.txt");

        // Remove resource
        watcher
            .remove_resource("file:///tmp/test.txt")
            .await
            .unwrap();

        // Check it was removed
        let resources = watcher.get_resources().await;
        assert_eq!(resources.len(), 0);
    }
}
