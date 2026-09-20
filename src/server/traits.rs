//! Core traits for handling server-side MCP requests.

use crate::error::Result;
use crate::shared::cancellation::RequestHandlerExtra;
use crate::types::{
    CallToolResult, CreateMessageParams, CreateMessageResult, GetPromptResult,
    ListResourcesResult, ListToolsResult, ReadResourceResult,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// A comprehensive handler for all MCP server operations.
///
/// This trait combines all the specific handler traits into a single interface.
/// It is the main entry point for implementing a custom MCP server.
#[async_trait]
pub trait RequestHandler:
    ToolHandler + PromptHandler + ResourceHandler + SamplingHandler + Send + Sync
{
}

/// Handler for tool execution.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// List available tools.
    async fn list_tools(&self, _req: crate::types::ListToolsRequest) -> Result<ListToolsResult> {
        Ok(ListToolsResult {
            tools: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }

    /// Handle a tool call with the given arguments.
    async fn call_tool(&self, req: crate::types::CallToolRequest) -> Result<CallToolResult>;
}

/// Handler for prompt generation.
#[async_trait]
pub trait PromptHandler: Send + Sync {
    /// Generate a prompt with the given arguments.
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult>;
}

/// Handler for resource access.
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    /// Read a resource at the given URI.
    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult>;

    /// List available resources.
    async fn list(&self, cursor: Option<String>, extra: RequestHandlerExtra)
        -> Result<ListResourcesResult>;
}

/// Handler for message sampling (LLM operations).
#[async_trait]
pub trait SamplingHandler: Send + Sync {
    /// Create a message using the language model.
    async fn create_message(
        &self,
        params: CreateMessageParams,
        extra: RequestHandlerExtra,
    ) -> Result<CreateMessageResult>;
}
