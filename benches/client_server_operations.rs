//! Benchmarks for MCP client and server operations
//!
//! These benchmarks measure the performance of high-level client/server
//! operations like initialization, tool calling, and request handling.

use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pmcp::types::capabilities::ElicitationCapabilities;
use pmcp::types::*;
use pmcp::{Server, ToolHandler};
use serde_json::{json, Value};
use std::hint::black_box;

/// Simple tool handler for benchmarking
struct BenchmarkTool;

#[async_trait]
impl ToolHandler for BenchmarkTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        // Simulate some processing
        let input = args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        Ok(json!({
            "result": format!("Processed: {}", input),
            "length": input.len(),
            "timestamp": chrono::Utc::now().timestamp()
        }))
    }
}

/// Complex tool handler that does more work
struct ComplexBenchmarkTool;

#[async_trait]
impl ToolHandler for ComplexBenchmarkTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        // Simulate more complex processing
        let empty_vec = Vec::new();
        let data = args
            .get("data")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("sum");

        let result = match operation {
            "sum" => data.iter().filter_map(|v| v.as_f64()).sum::<f64>(),
            "count" => data.len() as f64,
            "average" => {
                let sum: f64 = data.iter().filter_map(|v| v.as_f64()).sum();
                let count = data.iter().filter(|v| v.is_number()).count();
                if count > 0 {
                    sum / count as f64
                } else {
                    0.0
                }
            },
            _ => 0.0,
        };

        Ok(json!({
            "operation": operation,
            "result": result,
            "input_size": data.len(),
            "processed_items": data.iter().filter(|v| v.is_number()).count()
        }))
    }
}

/// Benchmark server builder operations
fn bench_server_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("server_creation");

    group.bench_function("basic_server_build", |b| {
        b.iter(|| {
            let server = black_box(
                Server::builder()
                    .name("benchmark-server")
                    .version("1.0.0")
                    .tool("simple_tool", BenchmarkTool)
                    .build()
                    .unwrap(),
            );
            black_box(server)
        })
    });

    group.bench_function("complex_server_build", |b| {
        b.iter(|| {
            let server = black_box(
                Server::builder()
                    .name("complex-benchmark-server")
                    .version("2.0.0")
                    .tool("simple_tool", BenchmarkTool)
                    .tool("complex_tool", ComplexBenchmarkTool)
                    .tool("another_tool", BenchmarkTool)
                    .capabilities(ServerCapabilities {
                        tools: Some(ToolCapabilities {
                            list_changed: Some(true),
                        }),
                        ..Default::default()
                    })
                    .build()
                    .unwrap(),
            );
            black_box(server)
        })
    });

    group.finish();
}

/// Benchmark request processing patterns
fn bench_request_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_processing");

    // Different types of requests to benchmark
    let requests = vec![
        ("ping", ClientRequest::Ping),
        (
            "list_tools",
            ClientRequest::ListTools(ListToolsParams { cursor: None }),
        ),
        (
            "simple_call_tool",
            ClientRequest::CallTool(CallToolParams {
                name: "simple_tool".to_string(),
                arguments: json!({"input": "test"}),
                _meta: None,
                task: None,
            }),
        ),
        (
            "complex_call_tool",
            ClientRequest::CallTool(CallToolParams {
                name: "complex_tool".to_string(),
                arguments: json!({
                    "data": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                    "operation": "average"
                }),
                _meta: None,
                task: None,
            }),
        ),
        (
            "initialize",
            ClientRequest::Initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "benchmark-client".to_string(),
                    version: "1.0.0".to_string(),
                },
            }),
        ),
    ];

    for (name, request) in requests {
        group.bench_with_input(
            BenchmarkId::new("serialize_request", name),
            &request,
            |b, request| {
                b.iter(|| {
                    let serialized = serde_json::to_string(black_box(request)).unwrap();
                    black_box(serialized)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("roundtrip_request", name),
            &request,
            |b, request| {
                b.iter(|| {
                    let serialized = serde_json::to_string(black_box(request)).unwrap();
                    let deserialized: ClientRequest = serde_json::from_str(&serialized).unwrap();
                    black_box(deserialized)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark response generation
fn bench_response_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_generation");

    // Different types of responses
    let responses = vec![
        (
            "simple_success",
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Number(1),
                payload: pmcp::types::jsonrpc::ResponsePayload::Result(json!("pong")),
            },
        ),
        (
            "tool_list",
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Number(2),
                payload: pmcp::types::jsonrpc::ResponsePayload::Result(
                    serde_json::to_value(ListToolsResult::new(vec![
                        ToolInfo::new(
                            "tool1",
                            Some("First tool".to_string()),
                            json!({"type": "object"}),
                        ),
                        ToolInfo::new(
                            "tool2",
                            Some("Second tool".to_string()),
                            json!({"type": "object"}),
                        ),
                    ]))
                    .unwrap(),
                ),
            },
        ),
        (
            "tool_call_result",
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Number(3),
                payload: pmcp::types::jsonrpc::ResponsePayload::Result(
                    serde_json::to_value(CallToolResult {
                        content: vec![Content::Text {
                            text: "This is the result of a tool call with substantial output data."
                                .to_string(),
                        }],
                        is_error: false,
                        ..Default::default()
                    })
                    .unwrap(),
                ),
            },
        ),
        (
            "error_response",
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Number(4),
                payload: pmcp::types::jsonrpc::ResponsePayload::Error(JSONRPCError {
                    code: -32602,
                    message: "Invalid params".to_string(),
                    data: Some(json!({"details": "Missing required parameter"})),
                }),
            },
        ),
    ];

    for (name, response) in responses {
        group.bench_with_input(
            BenchmarkId::new("serialize_response", name),
            &response,
            |b, response| {
                b.iter(|| {
                    let serialized = serde_json::to_string(black_box(response)).unwrap();
                    black_box(serialized)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark capabilities handling
fn bench_capabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("capabilities");

    let full_client_capabilities = ClientCapabilities {
        sampling: Some(SamplingCapabilities { models: None }),
        elicitation: Some(ElicitationCapabilities::default()),
        roots: Some(RootsCapabilities { list_changed: true }),
        experimental: None,
    };

    group.bench_function("serialize_full_client_capabilities", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(black_box(&full_client_capabilities)).unwrap();
            black_box(serialized)
        })
    });

    let full_server_capabilities = ServerCapabilities {
        tools: Some(ToolCapabilities {
            list_changed: Some(true),
        }),
        prompts: Some(PromptCapabilities {
            list_changed: Some(true),
        }),
        resources: Some(ResourceCapabilities {
            subscribe: Some(true),
            list_changed: Some(true),
        }),
        logging: Some(LoggingCapabilities { levels: None }),
        sampling: None,
        completions: None,
        experimental: None,
    };

    group.bench_function("serialize_full_server_capabilities", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(black_box(&full_server_capabilities)).unwrap();
            black_box(serialized)
        })
    });

    group.bench_function("roundtrip_capabilities", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(black_box(&full_client_capabilities)).unwrap();
            let deserialized: ClientCapabilities = serde_json::from_str(&serialized).unwrap();
            black_box(deserialized)
        })
    });

    group.finish();
}

/// Benchmark notification handling
fn bench_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("notifications");

    let notifications = vec![
        (
            "progress",
            Notification::Progress(ProgressNotification {
                progress_token: ProgressToken::String("task_123".to_string()),
                progress: 75.0,
                total: None,
                message: Some("Processing data...".to_string()),
            }),
        ),
        (
            "cancelled",
            Notification::Cancelled(CancelledNotification {
                request_id: RequestId::Number(42),
                reason: Some("User requested cancellation".to_string()),
            }),
        ),
        (
            "client_initialized",
            Notification::Client(ClientNotification::Initialized),
        ),
        (
            "server_tools_changed",
            Notification::Server(ServerNotification::ToolsChanged),
        ),
    ];

    for (name, notification) in notifications {
        group.bench_with_input(
            BenchmarkId::new("serialize_notification", name),
            &notification,
            |b, notification| {
                b.iter(|| {
                    let serialized = serde_json::to_string(black_box(notification)).unwrap();
                    black_box(serialized)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    // Create a batch of tool call requests
    let tool_calls: Vec<ClientRequest> = (0..50)
        .map(|i| {
            ClientRequest::CallTool(CallToolParams {
                name: "batch_tool".to_string(),
                arguments: json!({
                    "id": i,
                    "data": format!("Batch item {}", i),
                    "index": i
                }),
                _meta: None,
                task: None,
            })
        })
        .collect();

    group.bench_function("serialize_batch_tool_calls", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for request in &tool_calls {
                let serialized = serde_json::to_string(black_box(request)).unwrap();
                results.push(serialized);
            }
            black_box(results)
        })
    });

    // Create a batch of responses
    let responses: Vec<CallToolResult> = (0..50)
        .map(|i| CallToolResult {
            content: vec![Content::Text {
                text: format!("Result for batch item {}: processed successfully", i),
            }],
            is_error: false,
            ..Default::default()
        })
        .collect();

    group.bench_function("serialize_batch_responses", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for response in &responses {
                let serialized = serde_json::to_string(black_box(response)).unwrap();
                results.push(serialized);
            }
            black_box(results)
        })
    });

    group.finish();
}

criterion_group!(
    client_server_benches,
    bench_server_creation,
    bench_request_processing,
    bench_response_generation,
    bench_capabilities,
    bench_notifications,
    bench_batch_operations
);

criterion_main!(client_server_benches);
