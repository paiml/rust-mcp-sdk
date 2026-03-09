//! MCP-based foundation client implementation.
//!
//! This module provides `McpFoundationClient`, which uses the existing MCP
//! client infrastructure to connect to foundation servers over HTTP.

use super::{
    CompositionError, FoundationClient, FoundationConfig, FoundationEndpoint, PromptContent,
    PromptMessage, PromptResult, ResourceContent,
};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

use crate::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use crate::types::ClientCapabilities;
use crate::Client;

/// Connection state for a foundation server.
struct FoundationConnection {
    /// The MCP client for this connection.
    client: Arc<tokio::sync::RwLock<Client<StreamableHttpTransport>>>,
    /// Whether the connection is initialized.
    initialized: bool,
}

/// MCP-based foundation client that connects to servers over HTTP.
///
/// This client maintains persistent connections to foundation servers,
/// initializing them once and reusing them for subsequent calls.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::composition::{McpFoundationClient, FoundationConfig};
///
/// // Create configuration
/// let config = FoundationConfig::with_foundation("calculator", "http://localhost:8080");
///
/// // Create client
/// let client = McpFoundationClient::new(config).await?;
///
/// // Call a tool
/// let result = client.call_tool(
///     "calculator",
///     "add",
///     &serde_json::json!({"a": 5, "b": 3})
/// ).await?;
/// ```
pub struct McpFoundationClient {
    /// Configuration for foundation servers.
    config: FoundationConfig,
    /// Active connections to foundation servers.
    connections: RwLock<HashMap<String, Arc<FoundationConnection>>>,
}

impl std::fmt::Debug for McpFoundationClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpFoundationClient")
            .field("config", &self.config)
            .field(
                "connected_servers",
                &self.connections.read().keys().cloned().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl McpFoundationClient {
    /// Create a new MCP foundation client with the given configuration.
    ///
    /// This does not immediately connect to any servers. Connections are
    /// established lazily when first used.
    pub fn new(config: FoundationConfig) -> Self {
        Self {
            config,
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new client from a TOML configuration file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, CompositionError> {
        let config = FoundationConfig::from_file(path)?;
        Ok(Self::new(config))
    }

    /// Create a new client from environment variables.
    pub fn from_env() -> Self {
        let config = FoundationConfig::from_env();
        Self::new(config)
    }

    /// Create a client for a single foundation server.
    ///
    /// This is a convenience method for simple setups.
    pub fn for_server(server_id: impl Into<String>, url: impl Into<String>) -> Self {
        let config = FoundationConfig::with_foundation(server_id, url);
        Self::new(config)
    }

    /// Get or create a connection to a foundation server.
    async fn get_connection(
        &self,
        server_id: &str,
    ) -> Result<Arc<FoundationConnection>, CompositionError> {
        // Fast path: check if we already have a connection
        {
            let connections = self.connections.read();
            if let Some(conn) = connections.get(server_id) {
                if conn.initialized {
                    return Ok(conn.clone());
                }
            }
        }

        // Slow path: create a new connection
        let endpoint = self
            .config
            .get_endpoint(server_id)
            .ok_or_else(|| CompositionError::ServerNotFound(server_id.to_string()))?
            .clone();

        let conn = self.create_connection(server_id, &endpoint).await?;
        let conn = Arc::new(conn);

        // Store the connection
        {
            let mut connections = self.connections.write();
            connections.insert(server_id.to_string(), conn.clone());
        }

        Ok(conn)
    }

    /// Create a new connection to a foundation server.
    async fn create_connection(
        &self,
        server_id: &str,
        endpoint: &FoundationEndpoint,
    ) -> Result<FoundationConnection, CompositionError> {
        // Parse the URL
        let url = Url::parse(&endpoint.url).map_err(|e| {
            CompositionError::Configuration(format!("Invalid URL for {}: {}", server_id, e))
        })?;

        // Build transport configuration
        let mut transport_config = StreamableHttpTransportConfig {
            url,
            extra_headers: endpoint
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            auth_provider: None,
            session_id: None,
            enable_json_response: endpoint.enable_json_response,
            on_resumption_token: None,
            http_middleware_chain: None,
        };

        // Add auth header if configured
        if let Some(token) = &endpoint.auth_token {
            transport_config
                .extra_headers
                .push(("Authorization".to_string(), format!("Bearer {}", token)));
        }

        // Create transport
        let transport = StreamableHttpTransport::new(transport_config);

        // Create MCP client
        let mut client = Client::new(transport);

        // Initialize the connection
        let capabilities = ClientCapabilities::minimal();
        client.initialize(capabilities).await.map_err(|e| {
            CompositionError::ConnectionFailed(format!(
                "Failed to initialize connection to {}: {}",
                server_id, e
            ))
        })?;

        Ok(FoundationConnection {
            client: Arc::new(tokio::sync::RwLock::new(client)),
            initialized: true,
        })
    }

    /// Extract text from tool result content.
    fn extract_tool_result_text(
        result: &crate::types::CallToolResult,
    ) -> Result<String, CompositionError> {
        // If the result is an error, return it as an error
        if result.is_error {
            let error_text = result
                .content
                .first()
                .and_then(|c| match c {
                    crate::types::Content::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(CompositionError::ServerError {
                code: -1,
                message: error_text,
            });
        }

        // Extract text content
        for content in &result.content {
            if let crate::types::Content::Text { text } = content {
                return Ok(text.clone());
            }
        }

        // If no text content, try to serialize the whole result
        serde_json::to_string(&result.content)
            .map_err(|e| CompositionError::Serialization(e.to_string()))
    }
}

#[async_trait]
impl FoundationClient for McpFoundationClient {
    async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, CompositionError> {
        let conn = self.get_connection(server_id).await?;
        let client = conn.client.read().await;

        let result = client
            .call_tool(tool_name.to_string(), arguments.clone())
            .await
            .map_err(|e| CompositionError::ToolCallFailed {
                server_id: server_id.to_string(),
                tool_name: tool_name.to_string(),
                message: e.to_string(),
            })?;

        Self::extract_tool_result_text(&result)
    }

    async fn read_resource(
        &self,
        server_id: &str,
        uri: &str,
    ) -> Result<ResourceContent, CompositionError> {
        let conn = self.get_connection(server_id).await?;
        let client = conn.client.read().await;

        let result = client.read_resource(uri.to_string()).await.map_err(|e| {
            CompositionError::ResourceReadFailed {
                server_id: server_id.to_string(),
                uri: uri.to_string(),
                message: e.to_string(),
            }
        })?;

        // Convert MCP resource content to our ResourceContent type
        if let Some(content) = result.contents.first() {
            match content {
                crate::types::Content::Text { text } => Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: None,
                    text: Some(text.clone()),
                    blob: None,
                }),
                crate::types::Content::Resource {
                    uri,
                    text,
                    mime_type,
                    ..
                } => Ok(ResourceContent {
                    uri: uri.clone(),
                    mime_type: mime_type.clone(),
                    text: text.clone(),
                    blob: None,
                }),
                crate::types::Content::Image { data, mime_type } => Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some(mime_type.clone()),
                    text: None,
                    blob: Some(data.clone()),
                }),
            }
        } else {
            Err(CompositionError::InvalidResponse(
                "Empty resource content".to_string(),
            ))
        }
    }

    async fn get_prompt(
        &self,
        server_id: &str,
        prompt_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<PromptResult, CompositionError> {
        let conn = self.get_connection(server_id).await?;
        let client = conn.client.read().await;

        // Convert JSON arguments to HashMap<String, String>
        let args: HashMap<String, String> = if arguments.is_object() {
            arguments
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let value = if v.is_string() {
                        v.as_str().unwrap().to_string()
                    } else {
                        v.to_string()
                    };
                    (k.clone(), value)
                })
                .collect()
        } else {
            HashMap::new()
        };

        let result = client
            .get_prompt(prompt_name.to_string(), args)
            .await
            .map_err(|e| CompositionError::PromptFailed {
                server_id: server_id.to_string(),
                prompt_name: prompt_name.to_string(),
                message: e.to_string(),
            })?;

        // Convert MCP prompt result to our PromptResult type
        let messages = result
            .messages
            .into_iter()
            .map(|msg| {
                let role = match msg.role {
                    crate::types::Role::User => "user",
                    crate::types::Role::Assistant => "assistant",
                    crate::types::Role::System => "system",
                }
                .to_string();

                let content = match msg.content {
                    crate::types::Content::Text { text } => PromptContent::Text { text },
                    crate::types::Content::Image { data, mime_type } => {
                        PromptContent::Image { data, mime_type }
                    },
                    crate::types::Content::Resource {
                        uri,
                        text,
                        mime_type,
                        ..
                    } => PromptContent::Resource {
                        resource: super::types::EmbeddedResource {
                            uri,
                            mime_type,
                            text,
                            blob: None,
                        },
                    },
                };

                PromptMessage { role, content }
            })
            .collect();

        Ok(PromptResult {
            description: result.description,
            messages,
        })
    }

    async fn is_available(&self, server_id: &str) -> bool {
        // Check if we have a configuration for this server
        if self.config.get_endpoint(server_id).is_none() {
            return false;
        }

        // Try to get or create a connection
        match self.get_connection(server_id).await {
            Ok(conn) => conn.initialized,
            Err(_) => false,
        }
    }

    fn foundation_ids(&self) -> Vec<String> {
        self.config.foundations.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_server() {
        let client = McpFoundationClient::for_server("calculator", "http://localhost:8080");
        assert_eq!(client.foundation_ids(), vec!["calculator".to_string()]);
    }

    #[test]
    fn test_from_config() {
        let mut config = FoundationConfig::default();
        config.add_foundation("server1", FoundationEndpoint::new("http://localhost:8080"));
        config.add_foundation("server2", FoundationEndpoint::new("http://localhost:8081"));

        let client = McpFoundationClient::new(config);
        let ids = client.foundation_ids();
        assert!(ids.contains(&"server1".to_string()));
        assert!(ids.contains(&"server2".to_string()));
    }
}
