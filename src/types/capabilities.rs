//! Capability definitions for MCP clients and servers.
//!
//! This module defines the capability structures that clients and servers
//! use to advertise their supported features.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Client capabilities advertised during initialization.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::ClientCapabilities;
///
/// let capabilities = ClientCapabilities {
///     experimental: Some([("custom-feature".to_string(), serde_json::json!(true))]
///         .into_iter()
///         .collect()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Elicitation capabilities (for user input)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<ElicitationCapabilities>,

    /// Roots capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapabilities>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Server capabilities advertised during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tool providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,

    /// Prompt providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptCapabilities>,

    /// Resource providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,

    /// Logging capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapabilities>,

    /// Completion capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<CompletionCapabilities>,

    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Tool-related capabilities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Prompt-related capabilities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resource-related capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCapabilities {
    /// Whether resource subscriptions are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,

    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Logging capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingCapabilities {
    /// Supported log levels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
}

/// Sampling capabilities for LLM operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCapabilities {
    /// Supported model families/providers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

/// Elicitation capabilities for user input.
///
/// This capability indicates that the client supports requesting user input
/// during tool execution or other operations. The structure is intentionally
/// minimal as per the MCP specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationCapabilities {
    // Empty object as per MCP spec - client just advertises support
}

/// Roots capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapabilities {
    /// Whether list changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Completion capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionCapabilities {
    /// Placeholder for completion capability options
    #[serde(skip)]
    _reserved: (),
}

impl ClientCapabilities {
    /// Create a minimal set of client capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create minimal capabilities (no features advertised)
    /// let capabilities = ClientCapabilities::minimal();
    /// assert!(!capabilities.supports_sampling());
    /// assert!(!capabilities.supports_elicitation());
    ///
    /// // Use in client initialization
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::minimal()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create a full set of client capabilities.
    ///
    /// Advertises all standard client capabilities defined in the MCP specification.
    /// Note: Client capabilities indicate what the CLIENT can do (e.g., handle sampling
    /// requests, provide user input). Server capabilities (tools, prompts, resources)
    /// are advertised by servers, not clients.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create full capabilities (all client features supported)
    /// let capabilities = ClientCapabilities::full();
    /// assert!(capabilities.supports_sampling());
    /// assert!(capabilities.supports_elicitation());
    ///
    /// // Inspect specific capabilities
    /// assert!(capabilities.roots.unwrap().list_changed);
    ///
    /// // Use in client that supports all MCP client features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::full()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn full() -> Self {
        Self {
            sampling: Some(SamplingCapabilities::default()),
            elicitation: Some(ElicitationCapabilities::default()),
            roots: Some(RootsCapabilities { list_changed: true }),
            experimental: None,
        }
    }

    /// Check if the client supports elicitation (user input requests).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::ElicitationCapabilities};
    ///
    /// // Check elicitation support
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_elicitation());
    ///
    /// // Build capabilities with elicitation
    /// let interactive_client = ClientCapabilities {
    ///     elicitation: Some(ElicitationCapabilities::default()),
    ///     ..Default::default()
    /// };
    /// assert!(interactive_client.supports_elicitation());
    ///
    /// // Use for interactive tools
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let caps = ClientCapabilities::full();
    /// if caps.supports_elicitation() {
    ///     println!("Client can handle user input requests");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn supports_elicitation(&self) -> bool {
        self.elicitation.is_some()
    }

    /// Check if the client supports sampling.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::SamplingCapabilities};
    ///
    /// // Check sampling support for LLM operations
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_sampling());
    ///
    /// // Build LLM client capabilities
    /// let llm_client = ClientCapabilities {
    ///     sampling: Some(SamplingCapabilities {
    ///         models: Some(vec![
    ///             "gpt-4".to_string(),
    ///             "claude-3".to_string(),
    ///             "llama-2".to_string(),
    ///         ]),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(llm_client.supports_sampling());
    ///
    /// // List supported models
    /// if let Some(sampling) = &llm_client.sampling {
    ///     if let Some(models) = &sampling.models {
    ///         println!("Supported models: {:?}", models);
    ///     }
    /// }
    /// ```
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }
}

impl ServerCapabilities {
    /// Create a minimal set of server capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create minimal server with no advertised features
    /// let capabilities = ServerCapabilities::minimal();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in server that implements custom protocol extensions
    /// # use pmcp::Server;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("minimal-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::minimal())
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create capabilities for a tool server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides tools
    /// let capabilities = ServerCapabilities::tools_only();
    /// assert!(capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a tool-focused server
    /// # use pmcp::{Server, ToolHandler};
    /// # use async_trait::async_trait;
    /// # struct CalculatorTool;
    /// # #[async_trait]
    /// # impl ToolHandler for CalculatorTool {
    /// #     async fn handle(&self, args: serde_json::Value, _extra: pmcp::RequestHandlerExtra) -> Result<serde_json::Value, pmcp::Error> {
    /// #         Ok(serde_json::json!({"result": 42}))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("calculator-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::tools_only())
    ///     .tool("calculate", CalculatorTool)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn tools_only() -> Self {
        Self {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a prompt server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides prompts
    /// let capabilities = ServerCapabilities::prompts_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a prompt template server
    /// # use pmcp::{Server, PromptHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::protocol::{GetPromptResult, PromptMessage, Role, Content};
    /// # struct GreetingPrompt;
    /// # #[async_trait]
    /// # impl PromptHandler for GreetingPrompt {
    /// #     async fn handle(&self, args: std::collections::HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> Result<GetPromptResult, pmcp::Error> {
    /// #         Ok(GetPromptResult::new(
    /// #             vec![PromptMessage {
    /// #                 role: Role::System,
    /// #                 content: Content::Text { text: "Hello!".to_string() },
    /// #             }],
    /// #             Some("Greeting prompt".to_string()),
    /// #         ))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("prompt-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::prompts_only())
    ///     .prompt("greeting", GreetingPrompt)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prompts_only() -> Self {
        Self {
            prompts: Some(PromptCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a resource server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides resources
    /// let capabilities = ServerCapabilities::resources_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(capabilities.provides_resources());
    ///
    /// // Check subscription support
    /// let resource_caps = capabilities.resources.unwrap();
    /// assert!(resource_caps.subscribe.unwrap());
    /// assert!(resource_caps.list_changed.unwrap());
    ///
    /// // Use in a file system resource server
    /// # use pmcp::{Server, ResourceHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::protocol::{ReadResourceResult, ListResourcesResult, ResourceInfo, Content};
    /// # struct FileResource;
    /// # #[async_trait]
    /// # impl ResourceHandler for FileResource {
    /// #     async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra) -> Result<ReadResourceResult, pmcp::Error> {
    /// #         Ok(ReadResourceResult::new(vec![Content::Text { text: "File contents".to_string() }]))
    /// #     }
    /// #     async fn list(&self, _path: Option<String>, _extra: pmcp::RequestHandlerExtra) -> Result<ListResourcesResult, pmcp::Error> {
    /// #         Ok(ListResourcesResult::new(vec![]))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("filesystem-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::resources_only())
    ///     .resources(FileResource)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn resources_only() -> Self {
        Self {
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Check if the server provides tools.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check different server configurations
    /// let tool_server = ServerCapabilities::tools_only();
    /// assert!(tool_server.provides_tools());
    ///
    /// let minimal_server = ServerCapabilities::minimal();
    /// assert!(!minimal_server.provides_tools());
    ///
    /// // Use in server logic
    /// fn validate_server(caps: &ServerCapabilities) {
    ///     if caps.provides_tools() {
    ///         println!("Server can handle tool calls");
    ///     } else {
    ///         println!("Server does not provide tools");
    ///     }
    /// }
    ///
    /// // Combine multiple capabilities
    /// use pmcp::types::capabilities::{ToolCapabilities, PromptCapabilities};
    /// let multi_server = ServerCapabilities {
    ///     tools: Some(ToolCapabilities::default()),
    ///     prompts: Some(PromptCapabilities::default()),
    ///     ..Default::default()
    /// };
    /// assert!(multi_server.provides_tools());
    /// assert!(multi_server.provides_prompts());
    /// ```
    pub fn provides_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the server provides prompts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check prompt server
    /// let prompt_server = ServerCapabilities::prompts_only();
    /// assert!(prompt_server.provides_prompts());
    /// assert!(!prompt_server.provides_tools());
    ///
    /// // Use in client code to check server features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let transport = StdioTransport::new();
    /// # let mut client = Client::new(transport);
    /// # let server_info = client.initialize(pmcp::ClientCapabilities::default()).await?;
    /// if server_info.capabilities.provides_prompts() {
    ///     // Server supports prompts, we can list them
    ///     let prompts = client.list_prompts(None).await?;
    ///     println!("Available prompts: {}", prompts.prompts.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn provides_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the server provides resources.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check resource server capabilities
    /// let resource_server = ServerCapabilities::resources_only();
    /// assert!(resource_server.provides_resources());
    ///
    /// // Check if subscriptions are supported
    /// if resource_server.provides_resources() {
    ///     if let Some(res_caps) = &resource_server.resources {
    ///         if res_caps.subscribe.unwrap_or(false) {
    ///             println!("Server supports resource subscriptions");
    ///         }
    ///     }
    /// }
    ///
    /// // Build a full-featured server
    /// use pmcp::types::capabilities::*;
    /// let full_server = ServerCapabilities {
    ///     tools: Some(ToolCapabilities::default()),
    ///     prompts: Some(PromptCapabilities::default()),
    ///     resources: Some(ResourceCapabilities {
    ///         subscribe: Some(true),
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(full_server.provides_tools());
    /// assert!(full_server.provides_prompts());
    /// assert!(full_server.provides_resources());
    /// ```
    pub fn provides_resources(&self) -> bool {
        self.resources.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_capabilities_helpers() {
        let minimal = ClientCapabilities::minimal();
        assert!(!minimal.supports_sampling());
        assert!(!minimal.supports_elicitation());

        let full = ClientCapabilities::full();
        assert!(full.supports_sampling());
        assert!(full.supports_elicitation());
    }

    #[test]
    fn server_capabilities_helpers() {
        let tools_only = ServerCapabilities::tools_only();
        assert!(tools_only.provides_tools());
        assert!(!tools_only.provides_prompts());
        assert!(!tools_only.provides_resources());

        let prompts_only = ServerCapabilities::prompts_only();
        assert!(!prompts_only.provides_tools());
        assert!(prompts_only.provides_prompts());
        assert!(!prompts_only.provides_resources());
    }

    #[test]
    fn capabilities_serialization() {
        let caps = ClientCapabilities {
            sampling: Some(SamplingCapabilities::default()),
            elicitation: Some(ElicitationCapabilities::default()),
            roots: Some(RootsCapabilities { list_changed: true }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("sampling").is_some());
        assert!(json.get("elicitation").is_some());
        assert_eq!(json["roots"]["listChanged"], true);
        // Verify invalid fields are not present
        assert!(json.get("tools").is_none());
        assert!(json.get("prompts").is_none());
        assert!(json.get("resources").is_none());
    }

    #[test]
    fn server_capabilities_auto_set_serialization() {
        // Test that auto-set capabilities (with Some(false)) serialize correctly
        let caps = ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(false),
            }),
            prompts: Some(PromptCapabilities {
                list_changed: Some(false),
            }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        println!(
            "Serialized capabilities: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Verify tools, prompts, resources are present
        assert!(json.get("tools").is_some(), "tools should be present");
        assert!(json.get("prompts").is_some(), "prompts should be present");
        assert!(
            json.get("resources").is_some(),
            "resources should be present"
        );

        // Verify the listChanged fields are present
        assert_eq!(json["tools"]["listChanged"], false);
        assert_eq!(json["prompts"]["listChanged"], false);
        assert_eq!(json["resources"]["listChanged"], false);
        assert_eq!(json["resources"]["subscribe"], false);
    }

    #[test]
    fn server_capabilities_with_none_fields_serialization() {
        // Test that capabilities with None fields still have the parent object
        let caps = ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: None }),
            prompts: Some(PromptCapabilities { list_changed: None }),
            resources: Some(ResourceCapabilities {
                subscribe: None,
                list_changed: None,
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        println!(
            "Serialized capabilities with None: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Verify tools, prompts, resources are present (even if empty objects)
        assert!(
            json.get("tools").is_some(),
            "tools should be present even with None fields"
        );
        assert!(
            json.get("prompts").is_some(),
            "prompts should be present even with None fields"
        );
        assert!(
            json.get("resources").is_some(),
            "resources should be present even with None fields"
        );
    }
}
