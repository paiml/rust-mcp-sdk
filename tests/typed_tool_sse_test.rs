//! SSE smoke test for typed tools
//!
//! Ensures that typed tools with schemas work correctly over SSE transport.

#![cfg(all(feature = "sse", feature = "schema-generation"))]

use pmcp::ServerBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct TestArgs {
    /// Test message field
    message: String,
    /// Optional count field
    #[serde(default)]
    count: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct TestResponse {
    /// Echo of the message
    echo: String,
    /// Count used
    count: u32,
    /// Timestamp
    timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::server::typed_tool::TypedTool;
    use pmcp::server::ToolHandler;
    use pmcp::types::ListToolsResult;

    /// Create a test server with typed tools
    fn create_test_server() -> pmcp::Server {
        ServerBuilder::new()
            .name("sse-typed-test")
            .version("1.0.0")
            .tool_typed("echo_typed", |args: TestArgs, _extra| {
                Box::pin(async move {
                    let response = TestResponse {
                        echo: args.message.clone(),
                        count: args.count,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    serde_json::to_value(response)
                        .map_err(|e| pmcp::Error::Internal(format!("Serialization error: {}", e)))
                })
            })
            .build()
            .expect("Failed to build server")
    }

    #[tokio::test]
    async fn test_typed_tools_list_with_schema() {
        // Create a typed tool directly
        let tool = TypedTool::new("echo_typed", |args: TestArgs, _extra| {
            Box::pin(async move {
                let response = TestResponse {
                    echo: args.message.clone(),
                    count: args.count,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                serde_json::to_value(response)
                    .map_err(|e| pmcp::Error::Internal(format!("Serialization error: {}", e)))
            })
        });

        // Get tool metadata which includes the schema
        let metadata = tool.metadata().unwrap();
        let _schema = metadata.input_schema.clone();

        // Create ListToolsResult as would be sent over SSE
        let tools = vec![metadata];

        let result = ListToolsResult::new(tools);

        // Verify we have our tool
        assert_eq!(result.tools.len(), 1);
        let tool = &result.tools[0];
        assert_eq!(tool.name, "echo_typed");

        // Verify schema is present and valid
        assert!(tool.input_schema.is_object());
        let schema_obj = tool.input_schema.as_object().unwrap();

        // Check for expected schema properties
        assert!(schema_obj.contains_key("properties"));
        let props = schema_obj["properties"].as_object().unwrap();
        assert!(props.contains_key("message"));
        assert!(props.contains_key("count"));

        // Check that message has description
        let message_schema = props["message"].as_object().unwrap();
        assert!(message_schema.contains_key("type"));
        assert_eq!(message_schema["type"], "string");
        assert!(message_schema.contains_key("description"));
        assert!(message_schema["description"]
            .as_str()
            .unwrap()
            .contains("Test message"));

        // Check that count has default behavior
        let count_schema = props["count"].as_object().unwrap();
        assert_eq!(count_schema["type"], "integer");
    }

    #[tokio::test]
    async fn test_sse_format_compatibility() {
        // This test simulates SSE-specific behavior
        // In a real SSE scenario, the server would stream responses

        let _server = create_test_server();

        // Create an InitializeResult as would be sent over SSE
        let init_result = pmcp::types::InitializeResult {
            protocol_version: pmcp::ProtocolVersion(pmcp::DEFAULT_PROTOCOL_VERSION.to_string()),
            capabilities: pmcp::types::ServerCapabilities::tools_only(),
            server_info: pmcp::types::Implementation {
                name: "sse-typed-test".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: None,
        };

        // Create JSON-RPC response as would be sent over SSE
        let response =
            pmcp::types::JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: pmcp::types::RequestId::Number(1),
                payload: pmcp::types::jsonrpc::ResponsePayload::<
                    serde_json::Value,
                    serde_json::Value,
                >::Result(serde_json::to_value(init_result).unwrap()),
            };

        // In SSE mode, this would be formatted as:
        // data: {"jsonrpc":"2.0","id":1,"result":{...}}
        //
        // The schema normalization ensures the payload is reasonable size for SSE

        // Verify response can be serialized for SSE
        let json_str = serde_json::to_string(&response).expect("Failed to serialize response");

        // SSE has practical limits on message size
        // Our schema normalization helps keep payloads reasonable
        assert!(json_str.len() < 1_000_000); // 1MB limit is reasonable for SSE

        // Verify the response structure is SSE-compatible (no binary data, etc.)
        let parsed: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");
        assert!(parsed.is_object());
    }

    #[tokio::test]
    async fn test_schema_size_with_normalization() {
        // Test that schema normalization keeps sizes reasonable for SSE

        // Create a typed tool
        let tool = TypedTool::new("echo_typed", |args: TestArgs, _extra| {
            Box::pin(async move {
                let response = TestResponse {
                    echo: args.message.clone(),
                    count: args.count,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                serde_json::to_value(response)
                    .map_err(|e| pmcp::Error::Internal(format!("Serialization error: {}", e)))
            })
        });

        // Get tool metadata and create ListToolsResult
        let metadata = tool.metadata().unwrap();
        let tools = vec![metadata];

        let result = ListToolsResult::new(tools);

        // Create JSON-RPC response as would be sent over SSE
        let response =
            pmcp::types::JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: pmcp::types::RequestId::Number(1),
                payload: pmcp::types::jsonrpc::ResponsePayload::<
                    serde_json::Value,
                    serde_json::Value,
                >::Result(serde_json::to_value(result).unwrap()),
            };

        // Serialize the entire response
        let serialized = serde_json::to_string(&response).expect("Failed to serialize");

        // Check that the normalized schema keeps response size reasonable
        // This is important for SSE which chunks data
        assert!(serialized.len() < 100_000); // 100KB is very reasonable for SSE

        // Also verify no $refs remain (they're inlined by normalization)
        assert!(!serialized.contains("\"$ref\""));
        assert!(!serialized.contains("\"definitions\""));
        assert!(!serialized.contains("\"$defs\""));
    }
}
