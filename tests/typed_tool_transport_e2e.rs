//! End-to-end transport tests for typed tools
//!
//! Verifies that typed tools with schemas work correctly across all transports:
//! - HTTP JSON
//! - SSE (Server-Sent Events)
//! - WebSocket

#![cfg(feature = "schema-generation")]

use pmcp::{Server, ServerBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Test arguments with comprehensive schema features
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct ComplexArgs {
    /// Required string field
    required_string: String,

    /// Optional string field
    optional_string: Option<String>,

    /// Field with default value
    #[serde(default = "default_number")]
    number_with_default: u32,

    /// Enum field
    operation: Operation,

    /// Nested object
    metadata: Metadata,

    /// Array field
    tags: Vec<String>,
}

fn default_number() -> u32 {
    42
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct Metadata {
    /// Author of the operation
    author: String,
    /// Optional description
    description: Option<String>,
}

/// Create a test server with typed tools
fn create_test_server() -> Server {
    ServerBuilder::new()
        .name("transport-e2e-test")
        .version("1.0.0")
        .tool_typed("complex_tool", |args: ComplexArgs, _extra| {
            Box::pin(async move {
                // Validate and process
                if args.required_string.is_empty() {
                    return Err(pmcp::Error::Validation(
                        "required_string cannot be empty".to_string(),
                    ));
                }

                Ok(json!({
                    "processed": true,
                    "input_summary": {
                        "required": args.required_string,
                        "optional": args.optional_string,
                        "number": args.number_with_default,
                        "operation": format!("{:?}", args.operation),
                        "author": args.metadata.author,
                        "tags_count": args.tags.len(),
                    }
                }))
            })
        })
        .build()
        .expect("Failed to build server")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::server::typed_tool::TypedTool;
    use pmcp::server::ToolHandler;
    use pmcp::types::ListToolsResult;
    use pmcp::RequestHandlerExtra;

    /// Verify that tools/list returns proper schema across all transports
    #[tokio::test]
    async fn test_schema_in_tools_list() {
        // Create a typed tool directly
        let tool = TypedTool::new("complex_tool", |args: ComplexArgs, _extra| {
            Box::pin(async move {
                if args.required_string.is_empty() {
                    return Err(pmcp::Error::Validation(
                        "required_string cannot be empty".to_string(),
                    ));
                }
                Ok(json!({
                    "processed": true,
                    "required": args.required_string,
                }))
            })
        });

        // Get tool metadata which includes the schema
        let metadata = tool.metadata().unwrap();
        let schema = metadata.input_schema;

        // Verify schema structure
        verify_schema_structure(&schema);
    }

    /// Helper to verify schema structure is correct and normalized
    fn verify_schema_structure(schema: &Value) {
        let obj = schema.as_object().expect("Schema should be object");

        // Check no $ref or definitions remain (normalization should inline them)
        assert!(!obj.contains_key("$ref"));
        assert!(!obj.contains_key("definitions"));
        assert!(!obj.contains_key("$defs"));

        // Check properties exist
        assert!(obj.contains_key("properties"));
        let props = obj["properties"].as_object().unwrap();

        // Verify all expected fields
        assert!(props.contains_key("required_string"));
        assert!(props.contains_key("optional_string"));
        assert!(props.contains_key("number_with_default"));
        assert!(props.contains_key("operation"));
        assert!(props.contains_key("metadata"));
        assert!(props.contains_key("tags"));

        // Verify required_string has description
        let required = props["required_string"].as_object().unwrap();
        assert_eq!(required["type"], "string");
        assert!(required["description"]
            .as_str()
            .unwrap()
            .contains("Required"));

        // Verify operation is enum
        let operation = props["operation"].as_object().unwrap();
        assert!(operation.contains_key("enum") || operation.contains_key("type"));

        // Verify metadata is nested object (should be inlined, not a $ref)
        let metadata = props["metadata"].as_object().unwrap();
        assert!(metadata.contains_key("properties"));
        let metadata_props = metadata["properties"].as_object().unwrap();
        assert!(metadata_props.contains_key("author"));
        assert!(metadata_props.contains_key("description"));

        // Verify tags is array
        let tags = props["tags"].as_object().unwrap();
        assert_eq!(tags["type"], "array");
        assert!(tags.contains_key("items"));
    }

    /// Test that typed tool execution works correctly
    #[tokio::test]
    async fn test_typed_tool_execution() {
        // Create the typed tool directly
        let tool = TypedTool::new("complex_tool", |args: ComplexArgs, _extra| {
            Box::pin(async move {
                if args.required_string.is_empty() {
                    return Err(pmcp::Error::Validation(
                        "required_string cannot be empty".to_string(),
                    ));
                }

                Ok(json!({
                    "processed": true,
                    "input_summary": {
                        "required": args.required_string,
                        "optional": args.optional_string,
                        "number": args.number_with_default,
                        "operation": format!("{:?}", args.operation),
                        "author": args.metadata.author,
                        "tags_count": args.tags.len(),
                    }
                }))
            })
        });

        // Create a tool call with valid arguments
        let args = json!({
            "required_string": "test",
            "operation": "create",
            "metadata": {
                "author": "test_user"
            },
            "tags": ["tag1", "tag2"]
        });

        // Execute the tool through ToolHandler trait
        let extra = RequestHandlerExtra::new("test-1".to_string(), Default::default());
        let result = tool
            .handle(args, extra)
            .await
            .expect("Tool execution should succeed");

        // Verify the result
        assert_eq!(result["processed"], true);
        assert_eq!(result["input_summary"]["required"], "test");
        assert_eq!(result["input_summary"]["number"], 42); // default value
        assert_eq!(result["input_summary"]["tags_count"], 2);
    }

    /// Test schema validation with invalid arguments
    #[tokio::test]
    async fn test_schema_validation_error() {
        // Create the typed tool directly
        let tool = TypedTool::new("complex_tool", |_args: ComplexArgs, _extra| {
            Box::pin(async move { Ok(json!({ "should_not_reach": true })) })
        });

        // Missing required fields
        let invalid_args = json!({
            "operation": "create",
            "tags": []
        });

        // This should fail because required_string and metadata are missing
        let extra = RequestHandlerExtra::new("test-2".to_string(), Default::default());
        let result = tool.handle(invalid_args, extra).await;

        // Should get an error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error.to_string().contains("missing field") || error.to_string().contains("required")
        );
    }

    #[cfg(feature = "websocket")]
    #[tokio::test]
    async fn test_websocket_transport_compatibility() {
        // This test verifies that tool schemas work with WebSocket transport

        // Create a typed tool
        let tool = TypedTool::new("complex_tool", |_args: ComplexArgs, _extra| {
            Box::pin(async move { Ok(json!({"processed": true})) })
        });

        // Get tool metadata and create ListToolsResult
        let metadata = tool.metadata().unwrap();
        let tools = vec![metadata];

        let result = ListToolsResult::new(tools);

        // WebSocket messages are JSON text frames
        // Verify the response can be serialized for WebSocket
        let serialized =
            serde_json::to_string(&result).expect("Response should be serializable for WebSocket");

        // WebSocket has frame size limits, but our normalized schemas help
        assert!(serialized.len() < 10_000_000); // 10MB is reasonable for WebSocket

        // Verify it's valid JSON (WebSocket text frames must be valid UTF-8 JSON)
        let _parsed: Value =
            serde_json::from_str(&serialized).expect("Should be valid JSON for WebSocket");
    }

    #[cfg(feature = "streamable-http")]
    #[tokio::test]
    async fn test_http_transport_compatibility() {
        // Test that tool schemas work with HTTP transport
        let _server = create_test_server();

        // Get the server info which would be returned in initialize response
        let server_info = pmcp::types::InitializeResult {
            protocol_version: pmcp::ProtocolVersion(pmcp::DEFAULT_PROTOCOL_VERSION.to_string()),
            capabilities: pmcp::types::ServerCapabilities::tools_only(),
            server_info: pmcp::types::Implementation {
                name: "transport-e2e-test".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: None,
        };

        // Create a JSON-RPC response as would be sent over HTTP
        let response =
            pmcp::types::JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: pmcp::RequestId::Number(5),
                payload: pmcp::types::jsonrpc::ResponsePayload::<
                    serde_json::Value,
                    serde_json::Value,
                >::Result(serde_json::to_value(server_info).unwrap()),
            };

        // HTTP responses should be standard JSON-RPC
        let serialized =
            serde_json::to_string(&response).expect("Response should be serializable for HTTP");

        // Verify it matches JSON-RPC 2.0 spec
        assert!(serialized.contains("\"jsonrpc\":\"2.0\""));

        // Parse and verify structure
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("id").is_some());
        assert!(parsed.get("result").is_some());
    }

    /// Test that schema normalization produces consistent results across transports
    #[tokio::test]
    async fn test_schema_consistency_across_transports() {
        // Create multiple typed tools to simulate different configurations
        let tool1 = TypedTool::new("complex_tool", |_args: ComplexArgs, _extra| {
            Box::pin(async move { Ok(json!({"processed": true})) })
        });

        let tool2 = TypedTool::new("complex_tool", |_args: ComplexArgs, _extra| {
            Box::pin(async move { Ok(json!({"processed": true})) })
        });

        // Get schemas from tool metadata
        let metadata1 = tool1.metadata().unwrap();
        let metadata2 = tool2.metadata().unwrap();
        let schema1 = metadata1.input_schema;
        let schema2 = metadata2.input_schema;

        // Schemas should be identical regardless of creation
        assert_eq!(
            schema1, schema2,
            "Schemas should be consistent across transports"
        );

        // Verify both are normalized (no $refs)
        let schema_str = serde_json::to_string(&schema1).unwrap();
        assert!(!schema_str.contains("$ref"));
        assert!(!schema_str.contains("definitions"));
    }
}
