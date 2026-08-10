//! Conversion from internal types to protocol types
//!
//! Handles expansion happens here - internal handles are converted to protocol-compliant
//! `Content` at the edge.

use super::{
    error::WorkflowError,
    prompt_content::{InternalPromptMessage, PromptContent},
};
use crate::types::{Content, PromptMessage};
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
    /// Convert to protocol `Content`
    /// Expands handles using server registry
    pub fn to_protocol(&self, ctx: &ExpansionContext<'_>) -> Result<Content, WorkflowError> {
        match self {
            // Loose mode - direct passthrough
            Self::Text(text) => Ok(Content::text(text)),

            Self::Image { data, mime_type } => Ok(Content::image(data, mime_type)),

            // A bare URI is `ResourceLink` in the spec, NOT `EmbeddedResource`:
            // `EmbeddedResource.resource` is
            // `TextResourceContents | BlobResourceContents` (schema.ts:1734-1748)
            // and both arms require content this arm does not have. The LLM
            // fetches it, which is exactly `ResourceLink` semantics.
            Self::ResourceUri(uri) => Ok(Content::resource_link(uri.as_str(), uri.as_str())),

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
                Ok(Content::text(format!(
                    "Tool: {}\nDescription: {}\nSchema: {}",
                    handle.name(),
                    tool_info.description,
                    serde_json::to_string_pretty(&tool_info.input_schema)
                        .unwrap_or_else(|_| "{}".to_string())
                )))
            },

            Self::ResourceHandle(handle) => {
                // Validate resource exists
                if !ctx.resources.contains_key(handle.uri()) {
                    return Err(WorkflowError::MissingResource {
                        workflow: "unknown".to_string(),
                        resource: handle.uri().to_string(),
                    });
                }

                // Return as resource reference (LLM will fetch) -- a spec
                // `ResourceLink`, for the same reason as `Self::ResourceUri`.
                Ok(Content::resource_link(handle.uri(), handle.uri()))
            },

            Self::Multi(parts) => {
                // Convert parts and concatenate text
                let mut text_parts = Vec::new();
                for part in parts {
                    let protocol = part.as_ref().to_protocol(ctx)?;
                    if let Content::Text { text } = protocol {
                        text_parts.push(text);
                    }
                }
                Ok(Content::text(text_parts.join("\n\n")))
            },
        }
    }
}

impl InternalPromptMessage {
    /// Convert to protocol `PromptMessage`
    /// Expands handles using server registry
    pub fn to_protocol(&self, ctx: &ExpansionContext<'_>) -> Result<PromptMessage, WorkflowError> {
        Ok(PromptMessage::new(
            self.role,
            self.content.to_protocol(ctx)?,
        ))
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

        assert!(matches!(protocol, Content::Text { .. }));
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

        if let Content::Text { text } = protocol {
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

        // A handle expands to a spec `ResourceLink`, not an `EmbeddedResource`:
        // the LLM fetches the URI rather than receiving inline content.
        match protocol {
            Content::ResourceLink(link) => assert_eq!(link.uri, "resource://test/guide"),
            other => panic!("expected Content::ResourceLink, got {other:?}"),
        }
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
        assert!(matches!(protocol.content, Content::Text { .. }));
    }
}
