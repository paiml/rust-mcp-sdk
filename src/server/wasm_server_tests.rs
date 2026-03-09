//! Unit tests for WasmMcpServer

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::server::wasm_server::{SimpleTool, WasmMcpServer, WasmResource};
    use crate::shared::ClientInfo;
    use crate::types::{
        CallToolParams, CallToolResult, ClientRequest, Content, GetPromptParams, InitializeParams,
        InitializeResult, ListPromptsParams, ListPromptsResult, ListResourcesParams,
        ListResourcesResult, ListToolsParams, ListToolsResult, ReadResourceResult, Request,
        RequestId, ResourceInfo, ServerCapabilities,
    };
    use crate::{Error, ErrorCode, Result, SUPPORTED_PROTOCOL_VERSIONS};
    use serde_json::{json, Value};

    /// Create a test server with sample tools
    fn create_test_server() -> WasmMcpServer {
        WasmMcpServer::builder()
            .name("test-server")
            .version("1.0.0")
            .tool(
                "echo",
                SimpleTool::new("echo", "Echo back the input", |args| {
                    let message = args
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("empty");
                    Ok(json!({ "echo": message }))
                }),
            )
            .tool(
                "error_tool",
                SimpleTool::new("error_tool", "Always returns an error", |_args| {
                    Err(Error::protocol(
                        ErrorCode::INVALID_PARAMS,
                        "This tool always fails",
                    ))
                }),
            )
            .build()
    }

    #[tokio::test]
    async fn test_initialize_with_supported_version() {
        let server = create_test_server();
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            client_info: ClientInfo {
                name: "test-client".to_string(),
                version: Some("1.0.0".to_string()),
            },
        };

        let request = Request::Client(Box::new(ClientRequest::Initialize(params)));
        let response = server
            .handle_request(RequestId::String("1".to_string()), request)
            .await;

        // Check response is successful
        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: InitializeResult = serde_json::from_value(value).unwrap();
            assert_eq!(result.protocol_version.0, "2024-11-05");
            assert_eq!(result.server_info.name, "test-server");
            assert_eq!(result.server_info.version, "1.0.0");
        } else {
            panic!("Expected successful initialization");
        }
    }

    #[tokio::test]
    async fn test_initialize_with_unsupported_version() {
        let server = create_test_server();
        let params = InitializeParams {
            protocol_version: "1999-01-01".to_string(),
            client_info: ClientInfo {
                name: "test-client".to_string(),
                version: Some("1.0.0".to_string()),
            },
        };

        let request = Request::Client(Box::new(ClientRequest::Initialize(params)));
        let response = server
            .handle_request(RequestId::String("1".to_string()), request)
            .await;

        // Should negotiate to latest supported version
        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: InitializeResult = serde_json::from_value(value).unwrap();
            assert_eq!(result.protocol_version.0, SUPPORTED_PROTOCOL_VERSIONS[0]);
        } else {
            panic!("Expected successful initialization with negotiated version");
        }
    }

    #[tokio::test]
    async fn test_list_tools() {
        let server = create_test_server();
        let params = ListToolsParams { cursor: None };

        let request = Request::Client(Box::new(ClientRequest::ListTools(params)));
        let response = server
            .handle_request(RequestId::String("2".to_string()), request)
            .await;

        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: ListToolsResult = serde_json::from_value(value).unwrap();
            assert_eq!(result.tools.len(), 2);

            let tool_names: Vec<String> = result.tools.iter().map(|t| t.name.clone()).collect();
            assert!(tool_names.contains(&"echo".to_string()));
            assert!(tool_names.contains(&"error_tool".to_string()));
        } else {
            panic!("Expected successful tool listing");
        }
    }

    #[tokio::test]
    async fn test_call_existing_tool() {
        let server = create_test_server();
        let params = CallToolParams {
            name: "echo".to_string(),
            arguments: json!({ "message": "Hello, WASM!" }),
        };

        let request = Request::Client(Box::new(ClientRequest::CallTool(params)));
        let response = server
            .handle_request(RequestId::String("3".to_string()), request)
            .await;

        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: CallToolResult = serde_json::from_value(value).unwrap();
            assert!(!result.is_error);
            assert_eq!(result.content.len(), 1);

            if let Content::Text { text } = &result.content[0] {
                assert!(text.contains("Hello, WASM!"));
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected successful tool call");
        }
    }

    #[tokio::test]
    async fn test_call_nonexistent_tool() {
        let server = create_test_server();
        let params = CallToolParams {
            name: "nonexistent".to_string(),
            arguments: json!({}),
        };

        let request = Request::Client(Box::new(ClientRequest::CallTool(params)));
        let response = server
            .handle_request(RequestId::String("4".to_string()), request)
            .await;

        // Should return METHOD_NOT_FOUND error
        if let crate::types::jsonrpc::ResponsePayload::Error(error) = response.payload {
            assert_eq!(error.code, ErrorCode::METHOD_NOT_FOUND.0);
            assert!(error.message.contains("nonexistent"));
        } else {
            panic!("Expected error for nonexistent tool");
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_invalid_params() {
        let server = create_test_server();
        let params = CallToolParams {
            name: "error_tool".to_string(),
            arguments: json!({}),
        };

        let request = Request::Client(Box::new(ClientRequest::CallTool(params)));
        let response = server
            .handle_request(RequestId::String("5".to_string()), request)
            .await;

        // Tool should execute but return an error result
        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: CallToolResult = serde_json::from_value(value).unwrap();
            assert!(result.is_error);
            assert_eq!(result.content.len(), 1);

            if let Content::Text { text } = &result.content[0] {
                assert!(text.contains("always fails"));
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected result with error flag");
        }
    }

    #[tokio::test]
    async fn test_error_code_mapping() {
        // Test that error codes are mapped (simplified for WASM)
        // In WASM, we use a simplified error mapping
        assert_eq!(
            WasmMcpServer::map_error_code(&Error::protocol(ErrorCode::INVALID_PARAMS, "test")),
            ErrorCode::INTERNAL_ERROR
        );
        assert_eq!(
            WasmMcpServer::map_error_code(&Error::invalid_params("test")),
            ErrorCode::INTERNAL_ERROR
        );
        assert_eq!(
            WasmMcpServer::map_error_code(&Error::internal("test")),
            ErrorCode::INTERNAL_ERROR
        );
    }

    #[tokio::test]
    async fn test_resource_pagination() {
        // Create server with test resource
        struct TestResource;
        impl WasmResource for TestResource {
            fn read(&self, _uri: &str) -> Result<ReadResourceResult> {
                Ok(ReadResourceResult {
                    contents: vec![Content::Text {
                        text: "test".to_string(),
                    }],
                })
            }

            fn list(&self, cursor: Option<String>) -> Result<ListResourcesResult> {
                if cursor.is_none() {
                    Ok(ListResourcesResult {
                        resources: vec![ResourceInfo {
                            uri: "test://1".to_string(),
                            name: Some("Resource 1".to_string()),
                            mime_type: None,
                            description: None,
                            meta: None,
                        }],
                        next_cursor: Some("page2".to_string()),
                    })
                } else {
                    Ok(ListResourcesResult {
                        resources: vec![ResourceInfo {
                            uri: "test://2".to_string(),
                            name: Some("Resource 2".to_string()),
                            mime_type: None,
                            description: None,
                            meta: None,
                        }],
                        next_cursor: None,
                    })
                }
            }
        }

        let server = WasmMcpServer::builder()
            .name("test-server")
            .version("1.0.0")
            .resource("test", TestResource)
            .build();

        // First page
        let params = ListResourcesParams { cursor: None };
        let request = Request::Client(Box::new(ClientRequest::ListResources(params)));
        let response = server
            .handle_request(RequestId::String("6".to_string()), request)
            .await;

        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: ListResourcesResult = serde_json::from_value(value).unwrap();
            assert_eq!(result.resources.len(), 1);
            assert_eq!(result.resources[0].uri, "test://1");
            assert!(result.next_cursor.is_some());

            // Second page using cursor
            let params = ListResourcesParams {
                cursor: result.next_cursor,
            };
            let request = Request::Client(Box::new(ClientRequest::ListResources(params)));
            let response = server
                .handle_request(RequestId::String("7".to_string()), request)
                .await;

            if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
                let result: ListResourcesResult = serde_json::from_value(value).unwrap();
                assert_eq!(result.resources.len(), 1);
                assert_eq!(result.resources[0].uri, "test://2");
                assert!(result.next_cursor.is_none());
            }
        } else {
            panic!("Expected successful resource listing");
        }
    }

    #[tokio::test]
    async fn test_content_format_variations() {
        let server = WasmMcpServer::builder()
            .name("test-server")
            .version("1.0.0")
            .tool(
                "text_tool",
                SimpleTool::new("text_tool", "Returns plain text", |_args| {
                    Ok(json!("Plain text response"))
                }),
            )
            .tool(
                "object_tool",
                SimpleTool::new("object_tool", "Returns structured object", |_args| {
                    Ok(json!({ "field1": "value1", "nested": { "field2": 42 } }))
                }),
            )
            .build();

        // Test plain text response
        let params = CallToolParams {
            name: "text_tool".to_string(),
            arguments: json!({}),
        };
        let request = Request::Client(Box::new(ClientRequest::CallTool(params)));
        let response = server
            .handle_request(RequestId::String("8".to_string()), request)
            .await;

        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: CallToolResult = serde_json::from_value(value).unwrap();
            if let Content::Text { text } = &result.content[0] {
                assert_eq!(text, "Plain text response");
            }
        }

        // Test structured object response
        let params = CallToolParams {
            name: "object_tool".to_string(),
            arguments: json!({}),
        };
        let request = Request::Client(Box::new(ClientRequest::CallTool(params)));
        let response = server
            .handle_request(RequestId::String("9".to_string()), request)
            .await;

        if let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload {
            let result: CallToolResult = serde_json::from_value(value).unwrap();
            if let Content::Text { text } = &result.content[0] {
                // Should be pretty-printed JSON
                assert!(text.contains("field1"));
                assert!(text.contains("value1"));
                assert!(text.contains("nested"));
            }
        }
    }
}
