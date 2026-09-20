//! Comprehensive unit tests for the `ServerCore` implementation.

#[cfg(test)]
#[allow(clippy::match_wildcard_for_single_variants)]
mod tests {
    use crate::error::{Error, Result};
    use crate::server::builder::ServerCoreBuilder;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::core::{ProtocolHandler, ServerCore};
    use crate::server::{PromptHandler, ResourceHandler, ToolHandler};
    use crate::types::ResourceInfo;
    use crate::types::*;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Test fixtures

    /// Mock tool that tracks invocations
    struct MockTool {
        invocation_count: Arc<AtomicUsize>,
        should_fail: bool,
        return_value: Value,
    }

    impl MockTool {
        fn new() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: false,
                return_value: json!({"status": "ok"}),
            }
        }

        fn with_return(value: Value) -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: false,
                return_value: value,
            }
        }

        fn failing() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
                should_fail: true,
                return_value: json!({}),
            }
        }

        fn invocation_count(&self) -> usize {
            self.invocation_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ToolHandler for MockTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            self.invocation_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(Error::internal("Mock tool error"))
            } else {
                Ok(self.return_value.clone())
            }
        }
    }

    /// Mock prompt handler
    struct MockPromptHandler {
        invocation_count: Arc<AtomicUsize>,
    }

    impl MockPromptHandler {
        fn new() -> Self {
            Self {
                invocation_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl PromptHandler for MockPromptHandler {
        async fn handle(
            &self,
            args: HashMap<String, String>,
            _extra: RequestHandlerExtra,
        ) -> Result<GetPromptResult> {
            self.invocation_count.fetch_add(1, Ordering::SeqCst);
            Ok(GetPromptResult {
                description: Some("Test prompt".to_string()),
                messages: vec![PromptMessage::user(Content::text(format!(
                    "Prompt with args: {:?}",
                    args
                )))],
                _meta: None,
            })
        }
    }

    /// Mock resource handler
    struct MockResourceHandler {
        resources: Vec<ResourceInfo>,
    }

    impl MockResourceHandler {
        fn new() -> Self {
            Self {
                resources: vec![ResourceInfo::new("test://resource1", "Resource 1")
                    .with_description("Test resource 1")
                    .with_mime_type("text/plain")],
            }
        }
    }

    #[async_trait]
    impl ResourceHandler for MockResourceHandler {
        async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
            if uri == "test://resource1" {
                Ok(ReadResourceResult::new(vec![Content::text(
                    "Resource content",
                )]))
            } else {
                Err(Error::internal(format!("Resource not found: {}", uri)))
            }
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<ListResourcesResult> {
            Ok(ListResourcesResult {
                resources: self.resources.clone(),
                next_cursor: None,
                ttl_ms: None,
                cache_scope: None,
            })
        }
    }

    // Helper functions

    fn create_test_server() -> ServerCore {
        ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap()
    }

    fn create_init_request() -> Request {
        Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2025-06-18".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })))
    }

    // Test cases

    #[tokio::test]
    async fn test_server_initialization() {
        let server = create_test_server();

        // Server should not be initialized initially
        assert!(!server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_none());

        // Send initialization request
        let response = server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Verify successful response
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let init_result: InitializeResult = serde_json::from_value(result).unwrap();
                assert_eq!(
                    init_result.protocol_version,
                    ProtocolVersion("2025-06-18".to_string())
                );
                assert_eq!(init_result.server_info.name, "test-server");
                assert_eq!(init_result.server_info.version, "1.0.0");
            },
            _ => panic!("Expected successful initialization"),
        }

        // Server should be initialized now
        assert!(server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_some());
    }

    #[tokio::test]
    async fn test_request_before_initialization() {
        let server = create_test_server();

        // Try to call a tool before initialization
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Should get an error
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert_eq!(error.code, -32002);
                assert!(error.message.contains("not initialized"));
            },
            _ => panic!("Expected error for uninitialized server"),
        }
    }

    #[tokio::test]
    async fn test_tool_listing() {
        let tool1 = MockTool::new();
        let tool2 = MockTool::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("tool1", tool1)
            .tool("tool2", tool2)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List tools
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 2);

                let tool_names: Vec<&str> =
                    tools_result.tools.iter().map(|t| t.name.as_str()).collect();
                assert!(tool_names.contains(&"tool1"));
                assert!(tool_names.contains(&"tool2"));
            },
            _ => panic!("Expected successful tools list"),
        }
    }

    #[tokio::test]
    async fn test_tool_schema_in_list() {
        use crate::server::simple_tool::SyncTool;

        let tool_with_schema = SyncTool::new("math_tool", |args| {
            let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!({ "result": a + b }))
        })
        .with_description("Adds two numbers")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "First number" },
                "b": { "type": "number", "description": "Second number" }
            },
            "required": ["a", "b"]
        }));

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("math_tool", tool_with_schema)
            .tool("plain_tool", MockTool::new())
            .build()
            .unwrap();

        // Initialize
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List tools
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 2);

                // Check tool with schema
                let math_tool = tools_result
                    .tools
                    .iter()
                    .find(|t| t.name == "math_tool")
                    .expect("math_tool not found");

                assert_eq!(math_tool.description.as_deref(), Some("Adds two numbers"));
                assert_eq!(math_tool.input_schema["type"], "object");
                assert_eq!(math_tool.input_schema["required"], json!(["a", "b"]));
                assert_eq!(math_tool.input_schema["properties"]["a"]["type"], "number");

                // Check plain tool has default empty schema
                let plain_tool = tools_result
                    .tools
                    .iter()
                    .find(|t| t.name == "plain_tool")
                    .expect("plain_tool not found");

                assert_eq!(plain_tool.description, None);
                assert_eq!(plain_tool.input_schema, json!({}));
            },
            _ => panic!("Expected successful tools list"),
        }
    }

    #[tokio::test]
    async fn test_tool_invocation() {
        let tool = Arc::new(MockTool::with_return(json!({
            "result": "computed",
            "value": 42
        })));
        let tool_clone = tool.clone();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool_arc("calculator", tool_clone)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "calculator".to_string(),
            arguments: json!({
                "operation": "add",
                "a": 5,
                "b": 3
            }),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(!call_result.is_error);
                assert_eq!(call_result.content.len(), 1);
            },
            _ => panic!("Expected successful tool call"),
        }

        // Verify tool was invoked
        assert_eq!(tool.invocation_count(), 1);
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let server = create_test_server();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call non-existent tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "nonexistent".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            _ => panic!("Expected error for non-existent tool"),
        }
    }

    #[tokio::test]
    async fn test_tool_error_handling() {
        let tool = MockTool::failing();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("failing_tool", tool)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Call the failing tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "failing_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("Mock tool error"));
            },
            _ => panic!("Expected error from failing tool"),
        }
    }

    #[tokio::test]
    async fn test_tool_rejected_maps_to_iserror_result_not_protocol_error() {
        // A handler returning `Error::tool_rejected` must surface as a
        // SUCCESSFUL `CallToolResult { isError: true }` carrying the message as
        // text content and the detail as `structuredContent` — NOT a JSON-RPC
        // protocol error. This is the Code Mode policy-rejection envelope
        // (e.g. "SELECT missing LIMIT"): the model reads it and retries with a
        // corrected query rather than treating the call as a server fault.
        struct RejectingTool;
        #[async_trait]
        impl ToolHandler for RejectingTool {
            async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
                Err(Error::tool_rejected(
                    "SELECT statements must declare a LIMIT",
                    Some(json!({ "violations": [{ "rule": "missing_limit" }] })),
                ))
            }
        }

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("reject", RejectingTool)
            .build()
            .unwrap();
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "reject".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));
        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(
                    call_result.is_error,
                    "tool_rejected must produce isError: true"
                );
                let text = call_result
                    .content
                    .iter()
                    .find_map(|c| match c {
                        Content::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                assert!(
                    text.contains("must declare a LIMIT"),
                    "content must carry the rejection message, got: {text}"
                );
                let sc = call_result
                    .structured_content
                    .expect("structuredContent must carry the violation detail");
                assert_eq!(sc["violations"][0]["rule"], "missing_limit");
            },
            crate::types::jsonrpc::ResponsePayload::Error(e) => panic!(
                "tool_rejected must NOT be a protocol error, got {}: {}",
                e.code, e.message
            ),
        }
    }

    #[tokio::test]
    async fn test_prompt_handling() {
        let prompt = MockPromptHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .prompt("test_prompt", prompt)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List prompts
        let list_request =
            Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
                cursor: None,
            })));

        let list_response = server
            .handle_request(RequestId::from(2i64), list_request, None)
            .await;

        match list_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let prompts_result: ListPromptsResult = serde_json::from_value(result).unwrap();
                assert_eq!(prompts_result.prompts.len(), 1);
                assert_eq!(prompts_result.prompts[0].name, "test_prompt");
            },
            _ => panic!("Expected successful prompts list"),
        }

        // Get prompt
        let get_request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name: "test_prompt".to_string(),
            arguments: HashMap::from([("key".to_string(), "value".to_string())]),
            _meta: None,
        })));

        let get_response = server
            .handle_request(RequestId::from(3i64), get_request, None)
            .await;

        match get_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let prompt_result: GetPromptResult = serde_json::from_value(result).unwrap();
                assert_eq!(prompt_result.description, Some("Test prompt".to_string()));
                assert_eq!(prompt_result.messages.len(), 1);
            },
            _ => panic!("Expected successful prompt get"),
        }
    }

    #[tokio::test]
    async fn test_resource_handling() {
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .resources(resources)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // List resources
        let list_request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor: None },
        )));

        let list_response = server
            .handle_request(RequestId::from(2i64), list_request, None)
            .await;

        match list_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let resources_result: ListResourcesResult = serde_json::from_value(result).unwrap();
                assert_eq!(resources_result.resources.len(), 1);
                assert_eq!(resources_result.resources[0].uri, "test://resource1");
            },
            _ => panic!("Expected successful resources list"),
        }

        // Read resource
        let read_request =
            Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
                uri: "test://resource1".to_string(),
                _meta: None,
            })));

        let read_response = server
            .handle_request(RequestId::from(3i64), read_request, None)
            .await;

        match read_response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let read_result: ReadResourceResult = serde_json::from_value(result).unwrap();
                assert_eq!(read_result.contents.len(), 1);
                // Check that we got content back
                assert_eq!(read_result.contents.len(), 1);
            },
            _ => panic!("Expected successful resource read"),
        }
    }

    #[tokio::test]
    async fn test_resource_not_found() {
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .resources(resources)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(1i64), create_init_request(), None)
            .await;

        // Read non-existent resource
        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "test://nonexistent".to_string(),
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(2i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                assert!(error.message.contains("Resource not found"));
            },
            _ => panic!("Expected error for non-existent resource"),
        }
    }

    #[tokio::test]
    async fn test_capabilities_reporting() {
        let tool = MockTool::new();
        let prompt = MockPromptHandler::new();
        let resources = MockResourceHandler::new();

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("tool1", tool)
            .prompt("prompt1", prompt)
            .resources(resources)
            .build()
            .unwrap();

        // Check capabilities through ProtocolHandler trait
        let caps = server.capabilities();
        assert!(caps.tools.is_some());
        assert!(caps.prompts.is_some());
        assert!(caps.resources.is_some());

        // Check info through ProtocolHandler trait
        let info = server.info();
        assert_eq!(info.name, "test-server");
        assert_eq!(info.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_notification_handling() {
        let server = create_test_server();

        // Send a notification
        let notification = Notification::Progress(ProgressNotification::new(
            ProgressToken::String("test".to_string()),
            50.0,
            Some("Processing".to_string()),
        ));

        // Should handle without error
        let result = server.handle_notification(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        use futures::future::join_all;

        let tool = Arc::new(MockTool::new());
        let tool_clone = tool.clone();

        let server = Arc::new(
            ServerCoreBuilder::new()
                .name("test-server")
                .version("1.0.0")
                .tool_arc("concurrent_tool", tool_clone)
                .build()
                .unwrap(),
        );

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Create multiple concurrent requests
        let mut futures = Vec::new();
        for i in 1..=10 {
            let server_clone = server.clone();
            let future = async move {
                let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
                    name: "concurrent_tool".to_string(),
                    arguments: json!({ "id": i }),
                    _meta: None,
                    task: None,
                })));
                server_clone
                    .handle_request(RequestId::from(i as i64), request, None)
                    .await
            };
            futures.push(future);
        }

        // Execute all requests concurrently
        let results = join_all(futures).await;

        // All should succeed
        for response in results {
            match response.payload {
                crate::types::jsonrpc::ResponsePayload::Result(_) => {
                    // Success
                },
                _ => panic!("Expected successful concurrent tool calls"),
            }
        }

        // Verify all invocations
        assert_eq!(tool.invocation_count(), 10);
    }

    #[tokio::test]
    async fn test_builder_validation() {
        // Missing name
        let result = ServerCoreBuilder::new().version("1.0.0").build();
        assert!(result.is_err());

        // Missing version
        let result = ServerCoreBuilder::new().name("test").build();
        assert!(result.is_err());

        // Valid configuration
        let result = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_custom_capabilities() {
        let custom_caps = ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            prompts: None,
            resources: None,
            logging: None,
            completions: None,
            sampling: None,
            tasks: None,
            experimental: None,
            extensions: None,
        };

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .capabilities(custom_caps.clone())
            .build()
            .unwrap();

        assert_eq!(server.capabilities().tools, custom_caps.tools);
        assert_eq!(server.capabilities().prompts, custom_caps.prompts);
    }

    #[tokio::test]
    async fn test_call_tool_with_task_support_returns_create_task_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::tasks::RELATED_TASK_META_KEY;
        use crate::types::{TaskSupport, ToolExecution};

        // Create a TypedTool that returns Task-shaped JSON and declares taskSupport
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-test-123",
                        "status": "working",
                        "ttl": 60000,
                        "createdAt": "2026-03-22T00:00:00Z",
                        "lastUpdatedAt": "2026-03-22T00:00:00Z",
                        "pollInterval": 5000
                    }))
                })
            },
        )
        .with_description("A task-creating tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the task tool WITH task field (client requests task-augmented response)
        let mut call_req = CallToolRequest::new("task_tool", json!({}));
        call_req.task = Some(json!({})); // Client signals task-augmented mode
        let request = Request::Client(Box::new(ClientRequest::CallTool(call_req)));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Verify it's a CreateTaskResult (has "task" key, no "content" key)
                assert!(
                    result.get("task").is_some(),
                    "Response should have 'task' field for CreateTaskResult, got: {}",
                    serde_json::to_string_pretty(&result).unwrap()
                );
                assert!(
                    result.get("content").is_none(),
                    "Response should NOT have 'content' field (that would be CallToolResult)"
                );

                // D-STORE-MINTS-ID (finding #3): the wire task.taskId is the
                // STORE-minted id, NOT the tool's fabricated "t-test-123".
                let wire_task_id = result["task"]["taskId"]
                    .as_str()
                    .expect("task.taskId must be a string");
                assert_ne!(
                    wire_task_id, "t-test-123",
                    "wire id must be the store-minted id, not the tool's fabricated id"
                );
                // The tool's value carried no result content, so the task stays
                // Working (pending) -- no synchronous completion.
                assert_eq!(result["task"]["status"], "working");

                // Verify _meta with related-task reference (D-08, D-09) and that
                // it equals the store-minted wire id.
                assert!(
                    result.get("_meta").is_some(),
                    "Response should have '_meta' with related-task"
                );
                let related = &result["_meta"][RELATED_TASK_META_KEY];
                assert_eq!(
                    related["taskId"].as_str(),
                    Some(wire_task_id),
                    "_meta task id must equal the wire task.taskId (store-minted)"
                );
            },
            _ => panic!("Expected successful tool call with CreateTaskResult"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_without_task_support_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;

        // Tool returns Task-shaped JSON but does NOT declare taskSupport
        let normal_tool = MockTool::with_return(json!({
            "taskId": "t-sneaky",
            "status": "working"
        }));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("normal_tool", normal_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "normal_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Should be CallToolResult with content, NOT CreateTaskResult with task
                assert!(
                    result.get("content").is_some(),
                    "Should be CallToolResult with 'content' field"
                );
                assert!(
                    result.get("task").is_none(),
                    "Should NOT be CreateTaskResult -- tool doesn't declare taskSupport"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_task_support_but_no_task_field_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::{TaskSupport, ToolExecution};

        // Tool declares taskSupport=Required but client does NOT send task field
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-test-456",
                        "status": "working"
                    }))
                })
            },
        )
        .with_description("A task tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call WITHOUT task field — client doesn't support task-augmented calls
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "task_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Should be CallToolResult (backward compat for non-task-aware clients)
                assert!(
                    result.get("content").is_some(),
                    "Should be CallToolResult when client doesn't send task field"
                );
                assert!(
                    result.get("task").is_none(),
                    "Should NOT be CreateTaskResult — client didn't request task mode"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }

    /// Helper: extract the `Result` payload Value or panic.
    fn expect_result_payload(
        response: crate::types::jsonrpc::JSONRPCResponse,
    ) -> serde_json::Value {
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(v) => v,
            other => panic!("expected Result payload, got: {other:?}"),
        }
    }

    /// Helper: extract the error code from an `Error` payload or panic.
    fn expect_error_code(response: crate::types::jsonrpc::JSONRPCResponse) -> i32 {
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Error(e) => e.code,
            other => panic!("expected Error payload, got: {other:?}"),
        }
    }

    /// Finding #1 + #3 acceptance: a synchronously-completing task tool drives
    /// create -> the wire id is the store-minted id reflected in BOTH
    /// `task.taskId` AND `_meta` -> `tasks/get` finds it (not `NotFound`) and
    /// shows `Completed` -> `tasks/result` returns non-empty
    /// `CallToolResult.content`.
    #[tokio::test]
    async fn test_task_create_roundtrip_store_minted_id_and_typed_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::tasks::{GetTaskPayloadRequest, GetTaskRequest, RELATED_TASK_META_KEY};
        use crate::types::{TaskSupport, ToolExecution};

        // Tool returns a Task-shaped value that ALSO carries terminal `content`
        // (synchronous completion per D-TERMINAL-RESULT-CONTRACT).
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "tool-fabricated-id",
                        "status": "working",
                        "content": [{ "type": "text", "text": "the answer is 42" }]
                    }))
                })
            },
        )
        .with_description("A synchronously-completing task tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // create
        let mut call_req = CallToolRequest::new("task_tool", json!({}));
        call_req.task = Some(json!({}));
        let request = Request::Client(Box::new(ClientRequest::CallTool(call_req)));
        let create = expect_result_payload(
            server
                .handle_request(RequestId::from(1i64), request, None)
                .await,
        );

        // finding #3: wire task.taskId == _meta id == store-minted (not the tool's)
        let wire_id = create["task"]["taskId"]
            .as_str()
            .expect("task.taskId string")
            .to_string();
        assert_ne!(wire_id, "tool-fabricated-id");
        assert_eq!(
            create["_meta"][RELATED_TASK_META_KEY]["taskId"].as_str(),
            Some(wire_id.as_str())
        );
        // synchronous completion visible on the create envelope
        assert_eq!(create["task"]["status"], "completed");

        // tasks/get with the store-minted id finds the task (NOT NotFound)
        let get_req = Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
            task_id: wire_id.clone(),
        })));
        let got = expect_result_payload(
            server
                .handle_request(RequestId::from(2i64), get_req, None)
                .await,
        );
        assert_eq!(got["task"]["taskId"].as_str(), Some(wire_id.as_str()));
        assert_eq!(got["task"]["status"], "completed");

        // finding #1: tasks/result returns a typed, NON-EMPTY CallToolResult
        let result_req = Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest {
                task_id: wire_id.clone(),
            },
        )));
        let result = expect_result_payload(
            server
                .handle_request(RequestId::from(3i64), result_req, None)
                .await,
        );
        let content = result["content"]
            .as_array()
            .expect("CallToolResult.content array");
        assert!(
            !content.is_empty(),
            "tasks/result must return non-empty terminal content"
        );
        assert_eq!(content[0]["text"], "the answer is 42");
    }

    /// MED refinement: a pending task (no synchronous result) with a store but
    /// NO router yields the SPECIFIED not-completed error (-32002), not
    /// `NotFound` and not the no-backend -32601.
    #[tokio::test]
    async fn test_tasks_result_pending_task_returns_specified_error() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::tasks::GetTaskPayloadRequest;
        use crate::types::{TaskSupport, ToolExecution};

        // Tool returns a Task-shaped value with NO content -> task stays pending.
        let task_tool = TypedTool::new_with_schema(
            "task_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async { Ok(json!({ "taskId": "pending", "status": "working" })) })
            },
        )
        .with_description("A pending task tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("task_tool", task_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        let mut call_req = CallToolRequest::new("task_tool", json!({}));
        call_req.task = Some(json!({}));
        let create = expect_result_payload(
            server
                .handle_request(
                    RequestId::from(1i64),
                    Request::Client(Box::new(ClientRequest::CallTool(call_req))),
                    None,
                )
                .await,
        );
        let wire_id = create["task"]["taskId"].as_str().unwrap().to_string();
        assert_eq!(create["task"]["status"], "working");

        let result_req = Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest { task_id: wire_id },
        )));
        let code = expect_error_code(
            server
                .handle_request(RequestId::from(2i64), result_req, None)
                .await,
        );
        assert_eq!(
            code, -32002,
            "pending task must return the specified not-completed error"
        );
    }

    /// Finding #2: with BOTH a `TaskStore` (which has no result for the polled
    /// id) AND a `TaskRouter`, `tasks/result` FALLS THROUGH to the router
    /// (router serves it) rather than returning a hard error.
    #[tokio::test]
    async fn test_tasks_result_falls_through_to_router_on_store_miss() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::tasks::TaskRouter;
        use crate::types::tasks::GetTaskPayloadRequest;

        // Minimal router that serves tasks/result with a sentinel payload.
        struct FallbackRouter;
        #[async_trait]
        impl TaskRouter for FallbackRouter {
            async fn handle_task_call(
                &self,
                _tool_name: &str,
                _arguments: serde_json::Value,
                _task_params: serde_json::Value,
                _owner_id: &str,
                _progress_token: Option<serde_json::Value>,
            ) -> Result<serde_json::Value> {
                Ok(Value::Null)
            }
            async fn handle_tasks_get(
                &self,
                _params: serde_json::Value,
                _owner_id: &str,
            ) -> Result<serde_json::Value> {
                Ok(json!({}))
            }
            async fn handle_tasks_result(
                &self,
                _params: serde_json::Value,
                _owner_id: &str,
            ) -> Result<serde_json::Value> {
                Ok(json!({ "content": [{ "type": "text", "text": "from-router" }] }))
            }
            async fn handle_tasks_list(
                &self,
                _params: serde_json::Value,
                _owner_id: &str,
            ) -> Result<serde_json::Value> {
                Ok(json!({ "tasks": [] }))
            }
            async fn handle_tasks_cancel(
                &self,
                _params: serde_json::Value,
                _owner_id: &str,
            ) -> Result<serde_json::Value> {
                Ok(json!({}))
            }
            fn resolve_owner(
                &self,
                _subject: Option<&str>,
                _client_id: Option<&str>,
                _session_id: Option<&str>,
            ) -> String {
                "local".to_string()
            }
            fn tool_requires_task(
                &self,
                _tool_name: &str,
                _tool_execution: Option<&Value>,
            ) -> bool {
                false
            }
            fn task_capabilities(&self) -> Value {
                json!({ "supported": true })
            }
        }

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;
        let router = Arc::new(FallbackRouter) as Arc<dyn crate::server::tasks::TaskRouter>;

        // `with_task_store(Arc<dyn TaskRouter>)` is the (legacy-named) router
        // setter; `task_store(..)` sets the store. Configure BOTH.
        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .task_store(task_store)
            .with_task_store(router)
            .build()
            .unwrap();

        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // The store has no result for this id -> NotFound -> fall through.
        let result_req = Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest {
                task_id: "unknown-to-store".to_string(),
            },
        )));
        let result = expect_result_payload(
            server
                .handle_request(RequestId::from(1i64), result_req, None)
                .await,
        );
        assert_eq!(
            result["content"][0]["text"], "from-router",
            "tasks/result must fall through to the router on store miss"
        );
    }

    #[tokio::test]
    async fn test_call_tool_with_forbidden_task_support_returns_call_tool_result() {
        use crate::server::task_store::InMemoryTaskStore;
        use crate::server::typed_tool::TypedTool;
        use crate::types::{TaskSupport, ToolExecution};

        // Create TypedTool with Forbidden task support
        let forbidden_tool = TypedTool::new_with_schema(
            "forbidden_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "t-should-not-detect",
                        "status": "working"
                    }))
                })
            },
        )
        .with_description("Forbidden task support")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden));

        let task_store =
            Arc::new(InMemoryTaskStore::new()) as Arc<dyn crate::server::task_store::TaskStore>;

        let server = ServerCoreBuilder::new()
            .name("test-server")
            .version("1.0.0")
            .tool("forbidden_tool", forbidden_tool)
            .task_store(task_store)
            .build()
            .unwrap();

        // Initialize server
        server
            .handle_request(RequestId::from(0i64), create_init_request(), None)
            .await;

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            "forbidden_tool",
            json!({}),
        ))));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                assert!(result.get("content").is_some(), "Should be CallToolResult");
                assert!(
                    result.get("task").is_none(),
                    "Should NOT detect task -- Forbidden"
                );
            },
            _ => panic!("Expected successful tool call with CallToolResult"),
        }
    }
}
