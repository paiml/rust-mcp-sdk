//! Tests for batch request handling.

use pmcp::types::{
    Content, JSONRPCRequest, ListResourcesResult, ReadResourceResult, RequestId, ResourceInfo,
};
use pmcp::{BatchRequest, BatchResponse, ResourceHandler, ServerBuilder, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;

/// Mock tool handler for testing
#[derive(Clone)]
struct TestTool;

#[async_trait::async_trait]
impl ToolHandler for TestTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "result": "Tool executed",
            "args": args
        }))
    }
}

/// Mock resource handler for testing
#[derive(Clone)]
struct TestResourceHandler;

#[async_trait::async_trait]
impl ResourceHandler for TestResourceHandler {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        Ok(ReadResourceResult::new(vec![Content::Text {
            text: format!("Content of {}", uri),
        }]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo {
                uri: "file:///test1.txt".to_string(),
                name: "test1.txt".to_string(),
                description: Some("Test file 1".to_string()),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            },
            ResourceInfo {
                uri: "file:///test2.txt".to_string(),
                name: "test2.txt".to_string(),
                description: Some("Test file 2".to_string()),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            },
        ]))
    }
}

#[tokio::test]
async fn test_batch_multiple_requests() {
    let server = Arc::new(
        ServerBuilder::new()
            .name("test-batch-server")
            .version("1.0.0")
            .tool("test-tool", TestTool)
            .resources(TestResourceHandler)
            .build()
            .unwrap(),
    );

    // Initialize the server first
    let init_request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {"subscribe": true, "unsubscribe": true}
            },
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        })),
        id: RequestId::from("init"),
    };

    let _init_response = server
        .handle_batch_request(BatchRequest::Single(init_request))
        .await
        .unwrap();

    // Create a batch with different request types
    let batch = BatchRequest::Batch(vec![
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: Some(json!({})),
            id: RequestId::from("batch-1"),
        },
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/list".to_string(),
            params: Some(json!({})),
            id: RequestId::from("batch-2"),
        },
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "test-tool",
                "arguments": {"value": 42}
            })),
            id: RequestId::from("batch-3"),
        },
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/read".to_string(),
            params: Some(json!({
                "uri": "file:///test1.txt"
            })),
            id: RequestId::from("batch-4"),
        },
    ]);

    let response = server.handle_batch_request(batch).await.unwrap();
    let responses = response.into_responses();

    assert_eq!(responses.len(), 4);

    // Check tools/list response
    assert_eq!(responses[0].id, RequestId::from("batch-1"));
    match &responses[0].payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(value) => {
            assert!(value.get("tools").is_some(), "Response: {:?}", value);
        },
        pmcp::types::jsonrpc::ResponsePayload::Error(e) => {
            panic!("Got error for tools/list: {:?}", e);
        },
    }

    // Check resources/list response
    assert_eq!(responses[1].id, RequestId::from("batch-2"));
    match &responses[1].payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(value) => {
            assert!(value.get("resources").is_some());
        },
        pmcp::types::jsonrpc::ResponsePayload::Error(_) => {
            panic!("Expected successful result for resources/list")
        },
    }

    // Check tools/call response
    assert_eq!(responses[2].id, RequestId::from("batch-3"));
    match &responses[2].payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(value) => {
            assert!(value.get("content").is_some());
        },
        pmcp::types::jsonrpc::ResponsePayload::Error(_) => {
            panic!("Expected successful result for tools/call")
        },
    }

    // Check resources/read response
    assert_eq!(responses[3].id, RequestId::from("batch-4"));
    match &responses[3].payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(value) => {
            assert!(value.get("contents").is_some());
        },
        pmcp::types::jsonrpc::ResponsePayload::Error(_) => {
            panic!("Expected successful result for resources/read")
        },
    }
}

#[tokio::test]
async fn test_batch_error_handling() {
    let server = Arc::new(
        ServerBuilder::new()
            .name("test-batch-server")
            .version("1.0.0")
            .build()
            .unwrap(),
    );

    let batch = BatchRequest::Batch(vec![
        // Valid request
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id: RequestId::from(1i64),
        },
        // Invalid method
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "invalid/method".to_string(),
            params: None,
            id: RequestId::from(2i64),
        },
        // Call non-existent tool
        JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "non-existent-tool",
                "arguments": {}
            })),
            id: RequestId::from(3i64),
        },
    ]);

    let response = server.handle_batch_request(batch).await.unwrap();
    let responses = response.into_responses();

    assert_eq!(responses.len(), 3);

    // First should succeed (ping)
    assert_eq!(responses[0].id, RequestId::from(1i64));
    match &responses[0].payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(_) => {},
        pmcp::types::jsonrpc::ResponsePayload::Error(_) => panic!("Expected successful ping"),
    }

    // Second should be parse error (invalid method format)
    assert_eq!(responses[1].id, RequestId::from(2i64));
    match &responses[1].payload {
        pmcp::types::jsonrpc::ResponsePayload::Error(error) => {
            assert_eq!(error.code, -32700, "Error: {:?}", error); // Parse error
        },
        pmcp::types::jsonrpc::ResponsePayload::Result(_) => {
            panic!("Expected error for invalid method")
        },
    }

    // Third should be not found error for non-existent tool
    assert_eq!(responses[2].id, RequestId::from(3i64));
    match &responses[2].payload {
        pmcp::types::jsonrpc::ResponsePayload::Error(error) => {
            assert_eq!(error.code, -32603); // Internal error (not found)
        },
        pmcp::types::jsonrpc::ResponsePayload::Result(_) => {
            panic!("Expected error for non-existent tool")
        },
    }
}

#[tokio::test]
async fn test_batch_mixed_single_and_batch() {
    let server = Arc::new(
        ServerBuilder::new()
            .name("test-batch-server")
            .version("1.0.0")
            .build()
            .unwrap(),
    );

    // Test single request wrapped in BatchRequest
    let single = BatchRequest::Single(JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: RequestId::from("single"),
    });

    let response = server.handle_batch_request(single).await.unwrap();
    match response {
        BatchResponse::Single(resp) => {
            assert_eq!(resp.id, RequestId::from("single"));
        },
        BatchResponse::Batch(_) => panic!("Expected single response"),
    }

    // Test batch with single item
    let batch_single = BatchRequest::Batch(vec![JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: RequestId::from("batch-single"),
    }]);

    let response = server.handle_batch_request(batch_single).await.unwrap();
    match response {
        BatchResponse::Single(resp) => {
            assert_eq!(resp.id, RequestId::from("batch-single"));
        },
        BatchResponse::Batch(_) => panic!("Expected single response for batch with one item"),
    }
}

#[tokio::test]
async fn test_batch_preserve_order() {
    let server = Arc::new(
        ServerBuilder::new()
            .name("test-batch-server")
            .version("1.0.0")
            .build()
            .unwrap(),
    );

    // Create batch with specific order
    let ids: Vec<RequestId> = (1..=10).map(|i| RequestId::from(i64::from(i))).collect();
    let requests: Vec<JSONRPCRequest> = ids
        .iter()
        .cloned()
        .map(|id| JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id,
        })
        .collect();

    let batch = BatchRequest::Batch(requests);
    let response = server.handle_batch_request(batch).await.unwrap();
    let responses = response.into_responses();

    // Verify order is preserved
    assert_eq!(responses.len(), 10);
    for (i, response) in responses.iter().enumerate() {
        assert_eq!(response.id, ids[i]);
    }
}
