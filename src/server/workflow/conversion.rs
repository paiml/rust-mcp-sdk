//! Conversion from internal types to protocol types
//!
//! Handles expansion happens here - internal handles are converted to protocol-compliant
//! `MessageContent` at the edge.

use super::{
    error::WorkflowError,
    prompt_content::{InternalPromptMessage, PromptContent},
};
use crate::types::{MessageContent, PromptMessage};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Information about a registered tool
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for tool input
    pub input_schema: Value,
}

/// Information about a registered resource
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Optional resource name
    pub name: Option<String>,
    /// Optional MIME type
    pub mime_type: Option<String>,
}

/// Context needed for handle expansion
#[derive(Debug)]
pub struct ExpansionContext<'a> {
    /// Registered tools
    pub tools: &'a HashMap<Arc<str>, ToolInfo>,
    /// Registered resources
    pub resources: &'a HashMap<Arc<str>, ResourceInfo>,
}

impl PromptContent {
    /// Convert to protocol `MessageContent`
    /// Expands handles using server registry
    pub fn to_protocol(&self, ctx: &ExpansionContext<'_>) -> Result<MessageContent, WorkflowError> {
        match self {
            // Loose mode - direct passthrough
            Self::Text(text) => Ok(MessageContent::Text { text: text.clone() }),

            Self::Image { data, mime_type } => Ok(MessageContent::Image {
                data: data.clone(),
                mime_type: mime_type.clone(),
            }),

            Self::ResourceUri(uri) => Ok(MessageContent::Resource {
                uri: uri.clone(),
                text: None,
                mime_type: None,
                meta: None,
            }),

            // Strict mode - expand handles
            Self::ToolHandle(handle) => {
                // Look up tool in registry
                let tool_info =
                    ctx.tools
                        .get(handle.name())
                        .ok_or_else(|| WorkflowError::MissingTool {
                            workflow: "unknown".to_string(),
                            tool: handle.name().to_string(),
                        })?;

                // Embed tool schema as text (LLM can read it)
                Ok(MessageContent::Text {
                    text: format!(
                        "Tool: {}\nDescription: {}\nSchema: {}",
                        handle.name(),
                        tool_info.description,
                        serde_json::to_string_pretty(&tool_info.input_schema)
                            .unwrap_or_else(|_| "{}".to_string())
                    ),
                })
            },

            Self::ResourceHandle(handle) => {
                // Validate resource exists
                if !ctx.resources.contains_key(handle.uri()) {
                    return Err(WorkflowError::MissingResource {
                        workflow: "unknown".to_string(),
                        resource: handle.uri().to_string(),
                    });
                }

                // Return as resource reference (LLM will fetch)
                Ok(MessageContent::Resource {
                    uri: handle.uri().to_string(),
                    text: None,
                    mime_type: None,
                    meta: None,
                })
            },

            Self::Multi(parts) => {
                // Convert parts and concatenate text
                let mut text_parts = Vec::new();
                for part in parts {
                    let protocol = part.as_ref().to_protocol(ctx)?;
                    if let MessageContent::Text { text } = protocol {
                        text_parts.push(text);
                    }
                }
                Ok(MessageContent::Text {
                    text: text_parts.join("\n\n"),
                })
            },
        }
    }
}

impl InternalPromptMessage {
    /// Convert to protocol `PromptMessage`
    /// Expands handles using server registry
    pub fn to_protocol(&self, ctx: &ExpansionContext<'_>) -> Result<PromptMessage, WorkflowError> {
        Ok(PromptMessage {
            role: self.role,
            content: self.content.to_protocol(ctx)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::handles::{ResourceHandle, ToolHandle};
    use crate::types::Role;

    fn create_test_context() -> (HashMap<Arc<str>, ToolInfo>, HashMap<Arc<str>, ResourceInfo>) {
        let mut tools = HashMap::new();
        tools.insert(
            Arc::from("greet"),
            ToolInfo {
                name: "greet".to_string(),
                description: "Greet someone".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }),
            },
        );

        let mut resources = HashMap::new();
        resources.insert(
            Arc::from("resource://test/guide"),
            ResourceInfo {
                uri: "resource://test/guide".to_string(),
                name: Some("Test Guide".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
        );

        (tools, resources)
    }

    #[test]
    fn test_text_to_protocol() {
        let (tools, resources) = create_test_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        let content = PromptContent::Text("Hello".to_string());
        let protocol = content.to_protocol(&ctx).unwrap();

        assert!(matches!(protocol, MessageContent::Text { .. }));
    }

    #[test]
    fn test_tool_handle_to_protocol() {
        let (tools, resources) = create_test_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        let handle = ToolHandle::new("greet");
        let content = PromptContent::ToolHandle(handle);
        let protocol = content.to_protocol(&ctx).unwrap();

        if let MessageContent::Text { text } = protocol {
            assert!(text.contains("Tool: greet"));
            assert!(text.contains("Greet someone"));
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_tool_handle_missing() {
        let (tools, resources) = create_test_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        let handle = ToolHandle::new("nonexistent");
        let content = PromptContent::ToolHandle(handle);
        let result = content.to_protocol(&ctx);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WorkflowError::MissingTool { .. }
        ));
    }

    #[test]
    fn test_resource_handle_to_protocol() {
        let (tools, resources) = create_test_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        let handle = ResourceHandle::new("resource://test/guide").unwrap();
        let content = PromptContent::ResourceHandle(handle);
        let protocol = content.to_protocol(&ctx).unwrap();

        assert!(matches!(protocol, MessageContent::Resource { .. }));
    }

    #[test]
    fn test_internal_prompt_message_to_protocol() {
        let (tools, resources) = create_test_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        let msg = InternalPromptMessage::user("Hello");
        let protocol = msg.to_protocol(&ctx).unwrap();

        assert!(matches!(protocol.role, Role::User));
        assert!(matches!(protocol.content, MessageContent::Text { .. }));
    }
}
