//! Server-side batch request handling.

use crate::error::Result;
use crate::server::Server;
use crate::shared::batch::{BatchRequest, BatchResponse};
use crate::types::{JSONRPCRequest, JSONRPCResponse};
use std::sync::Arc;

impl Server {
    /// Handle a batch request.
    ///
    /// Processes multiple JSON-RPC requests and returns their responses
    /// in the same order.
    pub async fn handle_batch_request(
        self: &Arc<Self>,
        batch: BatchRequest,
    ) -> Result<BatchResponse> {
        let server = self.clone();
        let handler = move |req: JSONRPCRequest| {
            let server = server.clone();
            async move {
                // Convert JSONRPCRequest to our internal request type
                match crate::shared::protocol_helpers::parse_request(req.clone()) {
                    Ok((id, request)) => server.handle_request(id, request, None).await,
                    Err(e) => {
                        // Return error response for unparseable requests
                        JSONRPCResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id.clone(),
                            payload: crate::types::jsonrpc::ResponsePayload::Error(
                                crate::types::jsonrpc::JSONRPCError {
                                    code: crate::types::protocol::error_codes::PARSE_ERROR,
                                    message: format!("Parse error: {}", e),
                                    data: None,
                                },
                            ),
                        }
                    },
                }
            }
        };

        crate::shared::batch::process_batch_request(batch, handler).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ServerBuilder;
    use crate::shared::batch::BatchRequest;
    use crate::types::{JSONRPCRequest, RequestId};
    use serde_json::json;

    #[tokio::test]
    async fn test_batch_request_handling() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        // Create a batch of requests
        let batch = BatchRequest::Batch(vec![
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "tools/list".to_string(),
                params: None,
                id: RequestId::from("batch-1"),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "prompts/list".to_string(),
                params: None,
                id: RequestId::from("batch-2"),
            },
        ]);

        let response = server.handle_batch_request(batch).await.unwrap();
        let responses = response.into_responses();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].id, RequestId::from("batch-1"));
        assert_eq!(responses[1].id, RequestId::from("batch-2"));
    }

    #[tokio::test]
    async fn test_single_request_as_batch() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        let batch = BatchRequest::Single(JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: RequestId::from(1i64),
        });

        let response = server.handle_batch_request(batch).await.unwrap();
        match response {
            BatchResponse::Single(resp) => {
                assert_eq!(resp.id, RequestId::from(1i64));
            },
            BatchResponse::Batch(_) => panic!("Expected single response"),
        }
    }

    #[tokio::test]
    async fn test_empty_batch() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        let batch = BatchRequest::Batch(vec![]);
        let response = server.handle_batch_request(batch).await.unwrap();

        match response {
            BatchResponse::Batch(responses) => {
                assert!(responses.is_empty());
            },
            BatchResponse::Single(_) => panic!("Expected batch response"),
        }
    }

    #[tokio::test]
    async fn test_batch_with_invalid_request() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        let batch = BatchRequest::Batch(vec![
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "tools/list".to_string(),
                params: None,
                id: RequestId::from(1i64),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "invalid/method".to_string(),
                params: Some(json!({"bad": "params"})),
                id: RequestId::from(2i64),
            },
        ]);

        let response = server.handle_batch_request(batch).await.unwrap();
        let responses = response.into_responses();

        assert_eq!(responses.len(), 2);

        // First response should be successful
        assert_eq!(responses[0].id, RequestId::from(1i64));

        // Second response should be an error
        assert_eq!(responses[1].id, RequestId::from(2i64));
        match &responses[1].payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found") || error.message.contains("invalid"));
            },
            crate::types::jsonrpc::ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }
}
