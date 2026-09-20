//! Benchmarks for MCP transport layer performance
//!
//! These benchmarks measure the performance of message formatting,
//! content-length parsing, and transport-level operations.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pmcp::types::*;
use std::hint::black_box;

/// Benchmark content-length header parsing
fn bench_content_length_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_length_parsing");

    let test_cases = vec![
        ("small", "Content-Length: 123\r\n\r\n"),
        ("medium", "Content-Length: 12345\r\n\r\n"),
        ("large", "Content-Length: 1234567\r\n\r\n"),
        (
            "with_extra_headers",
            "Content-Type: application/json\r\nContent-Length: 456\r\nUser-Agent: test\r\n\r\n",
        ),
    ];

    for (name, header) in test_cases {
        group.bench_with_input(BenchmarkId::new("parse", name), &header, |b, header| {
            b.iter(|| {
                let lines: Vec<&str> = black_box(header).lines().collect();
                for line in lines {
                    if line.starts_with("Content-Length:") {
                        let length_str = line.trim_start_matches("Content-Length:").trim();
                        let _length: usize = length_str.parse().unwrap_or(0);
                        break;
                    }
                }
            })
        });
    }

    group.finish();
}

/// Benchmark message formatting for stdio transport
fn bench_message_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_formatting");

    // Create different sized messages
    let small_message = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let medium_message = serde_json::to_string(&ClientRequest::Initialize(InitializeRequest::new(
        Implementation::new("benchmark-client", "1.0.0"),
        ClientCapabilities::default(),
    )))
    .unwrap();

    let large_message = serde_json::to_string(&CallToolResult::new(
        (0..100)
            .map(|i| {
                Content::text(format!("This is a long piece of content for item {} that simulates a realistic response from an MCP tool with substantial data.", i))
            })
            .collect(),
    ))
    .unwrap();

    let test_messages = [
        ("small", small_message),
        ("medium", medium_message.as_str()),
        ("large", large_message.as_str()),
    ];

    for (name, message) in test_messages.iter() {
        group.bench_with_input(BenchmarkId::new("format", name), message, |b, message| {
            b.iter(|| {
                let content_length = black_box(message).len();
                let formatted = format!("Content-Length: {}\r\n\r\n{}", content_length, message);
                black_box(formatted)
            })
        });
    }

    group.finish();
}

/// Benchmark message size calculations
fn bench_message_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_sizes");

    let text_content = Content::text("A".repeat(1000)); // 1KB of text

    let image_content = Content::Image {
        data: "A".repeat(10000), // 10KB of base64 data
        mime_type: "image/png".to_string(),
    };

    let resource_content = Content::resource_with_text(
        "file://large-document.pdf",
        "This is a large document with extensive content...".repeat(100),
        "application/pdf",
    );

    let contents = vec![
        ("text_1kb", text_content),
        ("image_10kb", image_content),
        ("resource_large", resource_content),
    ];

    for (name, content) in contents {
        group.bench_with_input(
            BenchmarkId::new("serialize", name),
            &content,
            |b, content| {
                b.iter(|| {
                    let serialized = serde_json::to_string(&black_box(content)).unwrap();
                    black_box(serialized.len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark protocol helper operations
fn bench_protocol_helpers(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_helpers");

    group.bench_function("create_list_tools_request", |b| {
        b.iter(|| {
            let request = black_box(ClientRequest::ListTools(ListToolsRequest::default()));
            black_box(request)
        })
    });

    group.bench_function("create_call_tool_request", |b| {
        b.iter(|| {
            let request = black_box(ClientRequest::CallTool(CallToolRequest::new(
                "test_tool",
                serde_json::json!({
                    "input": "test data",
                    "options": {"format": "json"}
                }),
            )));
            black_box(request)
        })
    });

    group.bench_function("create_progress_notification", |b| {
        b.iter(|| {
            let notification = black_box(ProgressNotification::new(
                ProgressToken::String("task_123".to_string()),
                75.0,
                Some("Processing...".to_string()),
            ));
            black_box(notification)
        })
    });

    group.finish();
}

/// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    let json_error = JSONRPCError {
        code: -32600,
        message: "Invalid Request".to_string(),
        data: Some(serde_json::json!({
            "details": "The request was malformed and could not be processed",
            "request_id": "12345",
            "timestamp": "2024-01-01T00:00:00Z"
        })),
    };

    group.bench_function("create_jsonrpc_error", |b| {
        b.iter(|| {
            let error = black_box(JSONRPCError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: Some(serde_json::json!({"details": "Test error"})),
            });
            black_box(error)
        })
    });

    group.bench_function("serialize_jsonrpc_error", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&black_box(&json_error)).unwrap();
            black_box(serialized)
        })
    });

    group.bench_function("create_error_response", |b| {
        b.iter(|| {
            let response: pmcp::types::JSONRPCResponse<serde_json::Value> =
                black_box(JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id: RequestId::Number(42),
                    payload: pmcp::types::jsonrpc::ResponsePayload::Error(json_error.clone()),
                });
            black_box(response)
        })
    });

    group.finish();
}

/// Benchmark concurrent message processing
fn bench_concurrent_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_processing");

    let messages: Vec<String> = (0..100)
        .map(|i| {
            serde_json::to_string(&ClientRequest::CallTool(CallToolRequest::new(
                format!("tool_{}", i),
                serde_json::json!({
                    "id": i,
                    "data": format!("Message data for request {}", i)
                }),
            )))
            .unwrap()
        })
        .collect();

    group.bench_function("sequential_parsing", |b| {
        b.iter(|| {
            for message in &messages {
                let parsed: ClientRequest = serde_json::from_str(black_box(message)).unwrap();
                black_box(parsed);
            }
        })
    });

    let requests: Vec<ClientRequest> = (0..100)
        .map(|i| {
            ClientRequest::CallTool(CallToolRequest::new(
                format!("tool_{}", i),
                serde_json::json!({"id": i}),
            ))
        })
        .collect();

    group.bench_function("batch_serialization", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for request in &requests {
                let serialized = serde_json::to_string(black_box(request)).unwrap();
                results.push(serialized);
            }
            black_box(results)
        })
    });

    group.finish();
}

criterion_group!(
    transport_benches,
    bench_content_length_parsing,
    bench_message_formatting,
    bench_message_sizes,
    bench_protocol_helpers,
    bench_error_handling,
    bench_concurrent_processing
);

criterion_main!(transport_benches);
