//! Benchmarks for MCP protocol serialization and deserialization
//!
//! These benchmarks measure the performance of converting between
//! Rust types and JSON for various MCP protocol messages.

use criterion::{criterion_group, criterion_main, Criterion};
use pmcp::types::*;
use serde_json::json;
use std::hint::black_box;

/// Benchmark serialization of different request types
fn bench_request_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_serialization");

    // Initialize request
    let init_request = ClientRequest::Initialize(InitializeParams {
        protocol_version: "2024-11-05".to_string(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "benchmark-client".to_string(),
            version: "1.0.0".to_string(),
        },
    });

    group.bench_function("initialize_request", |b| {
        b.iter(|| serde_json::to_string(&black_box(&init_request)).unwrap())
    });

    // List tools request
    let list_tools = ClientRequest::ListTools(ListToolsParams { cursor: None });

    group.bench_function("list_tools_request", |b| {
        b.iter(|| serde_json::to_string(&black_box(&list_tools)).unwrap())
    });

    // Call tool request with complex arguments
    let call_tool = ClientRequest::CallTool(CallToolParams {
        name: "complex_tool".to_string(),
        task: None,
        arguments: json!({
            "query": "rust programming language",
            "filters": {
                "type": "documentation",
                "level": "advanced",
                "tags": ["async", "tokio", "performance"]
            },
            "options": {
                "max_results": 100,
                "include_examples": true,
                "format": "markdown"
            }
        }),
        _meta: None,
    });

    group.bench_function("call_tool_request", |b| {
        b.iter(|| serde_json::to_string(&black_box(&call_tool)).unwrap())
    });

    group.finish();
}

/// Benchmark deserialization of different request types
fn bench_request_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_deserialization");

    // Pre-serialize JSON strings for deserialization benchmarks
    let init_json = r#"{
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "clientInfo": {
                "name": "benchmark-client",
                "version": "1.0.0"
            }
        }
    }"#;

    group.bench_function("initialize_request", |b| {
        b.iter(|| serde_json::from_str::<ClientRequest>(black_box(init_json)).unwrap())
    });

    let list_tools_json = r#"{
        "method": "tools/list",
        "params": {}
    }"#;

    group.bench_function("list_tools_request", |b| {
        b.iter(|| serde_json::from_str::<ClientRequest>(black_box(list_tools_json)).unwrap())
    });

    let call_tool_json = r#"{
        "method": "tools/call",
        "params": {
            "name": "complex_tool",
            "arguments": {
                "query": "rust programming language",
                "filters": {
                    "type": "documentation",
                    "level": "advanced",
                    "tags": ["async", "tokio", "performance"]
                },
                "options": {
                    "max_results": 100,
                    "include_examples": true,
                    "format": "markdown"
                }
            }
        }
    }"#;

    group.bench_function("call_tool_request", |b| {
        b.iter(|| serde_json::from_str::<ClientRequest>(black_box(call_tool_json)).unwrap())
    });

    group.finish();
}

/// Benchmark response serialization
fn bench_response_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_serialization");

    // Initialize response
    let init_response = InitializeResult {
        protocol_version: pmcp::ProtocolVersion("2024-11-05".to_string()),
        capabilities: ServerCapabilities::default(),
        server_info: Implementation {
            name: "benchmark-server".to_string(),
            version: "1.0.0".to_string(),
        },
        instructions: Some("A high-performance MCP server for benchmarking".to_string()),
    };

    group.bench_function("initialize_response", |b| {
        b.iter(|| serde_json::to_string(&black_box(&init_response)).unwrap())
    });

    // List tools response with multiple tools
    let tools_response = ListToolsResult::new(vec![
        ToolInfo::new(
            "search",
            Some("Search for information".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "number"}
                }
            }),
        ),
        ToolInfo::new(
            "analyze",
            Some("Analyze data".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "data": {"type": "array"},
                    "method": {"type": "string"}
                }
            }),
        ),
        ToolInfo::new(
            "generate",
            Some("Generate content".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "template": {"type": "string"},
                    "variables": {"type": "object"}
                }
            }),
        ),
    ]);

    group.bench_function("list_tools_response", |b| {
        b.iter(|| serde_json::to_string(&black_box(&tools_response)).unwrap())
    });

    // Call tool response with complex content
    let call_tool_response = CallToolResult {
        content: vec![
            Content::Text {
                text: "This is a comprehensive analysis of the data provided. The results show significant patterns in user behavior across multiple dimensions.".to_string(),
            },
            Content::Text {
                text: "Additional insights reveal performance improvements of up to 40% when using the optimized algorithms.".to_string(),
            },
        ],
        is_error: false,
        ..Default::default()
    };

    group.bench_function("call_tool_response", |b| {
        b.iter(|| serde_json::to_string(&black_box(&call_tool_response)).unwrap())
    });

    group.finish();
}

/// Benchmark JSONRPC message handling
fn bench_jsonrpc_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonrpc_messages");

    let request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(42),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "test_tool",
            "arguments": {
                "input": "benchmark data",
                "options": {"format": "json"}
            }
        })),
    };

    group.bench_function("jsonrpc_request_serialize", |b| {
        b.iter(|| serde_json::to_string(&black_box(&request)).unwrap())
    });

    let response = JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(42),
        payload: pmcp::types::jsonrpc::ResponsePayload::<
            serde_json::Value,
            pmcp::types::jsonrpc::JSONRPCError,
        >::Result(json!({
            "content": [{
                "type": "text",
                "text": "Benchmark result data with performance metrics"
            }],
            "is_error": false
        })),
    };

    group.bench_function("jsonrpc_response_serialize", |b| {
        b.iter(|| serde_json::to_string(&black_box(&response)).unwrap())
    });

    let request_json = r#"{
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "test_tool",
            "arguments": {
                "input": "benchmark data",
                "options": {"format": "json"}
            }
        }
    }"#;

    group.bench_function("jsonrpc_request_deserialize", |b| {
        b.iter(|| serde_json::from_str::<JSONRPCRequest>(black_box(request_json)).unwrap())
    });

    group.finish();
}

/// Benchmark large message handling
fn bench_large_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_messages");

    // Create a large tool response with lots of content
    let large_content: Vec<Content> = (0..1000)
        .map(|i| Content::Text {
            text: format!("This is content item number {} with some additional text to make it realistic. It contains information about data processing, analysis results, and performance metrics that would typically be found in a real MCP response.", i),
        })
        .collect();

    let large_response = CallToolResult {
        content: large_content,
        is_error: false,
        ..Default::default()
    };

    group.bench_function("large_tool_response_serialize", |b| {
        b.iter(|| serde_json::to_string(&black_box(&large_response)).unwrap())
    });

    // Create a large list of tools
    let many_tools: Vec<ToolInfo> = (0..100)
        .map(|i| ToolInfo::new(
            format!("tool_{}", i),
            Some(format!("Description for tool number {} with comprehensive details about its functionality and usage patterns.", i)),
            json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": format!("Input parameter for tool {}", i)},
                    "options": {
                        "type": "object",
                        "properties": {
                            "format": {"type": "string"},
                            "limit": {"type": "number"},
                            "detailed": {"type": "boolean"}
                        }
                    }
                }
            }),
        ))
        .collect();

    let large_tools_response =
        ListToolsResult::new(many_tools).with_next_cursor("next_page_token_12345".to_string());

    group.bench_function("large_tools_list_serialize", |b| {
        b.iter(|| serde_json::to_string(&black_box(&large_tools_response)).unwrap())
    });

    group.finish();
}

criterion_group!(
    protocol_benches,
    bench_request_serialization,
    bench_request_deserialization,
    bench_response_serialization,
    bench_jsonrpc_messages,
    bench_large_messages
);

criterion_main!(protocol_benches);
