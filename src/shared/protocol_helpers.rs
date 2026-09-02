//! Protocol helper functions for parsing and creating messages.

use crate::error::{Error, Result};
use crate::shared::simd_parsing::SimdJsonParser;
use crate::types::{
    ClientNotification, ClientRequest, JSONRPCNotification, JSONRPCRequest, Notification, Request,
    RequestId, ServerNotification, ServerRequest,
};
use serde_json::Value;
use std::sync::LazyLock;

/// Global SIMD JSON parser instance for high-performance parsing
static SIMD_PARSER: LazyLock<SimdJsonParser> = LazyLock::new(SimdJsonParser::new);

/// Crate-private ingress classification of a raw JSON-RPC request (Phase 112,
/// VERS-04).
///
/// A raw request is either a public typed [`Request`] (the existing exhaustive
/// enum path) or an internally-routed method that deliberately has NO public
/// enum variant (v2 `server/discover` and v2 `tasks/update`, both carried by
/// [`InternalClientRequest`](crate::types::protocol::InternalClientRequest)).
///
/// This is the single method-string interception seam: it consults
/// [`classify_internal_method`](crate::types::protocol::classify_internal_method)
/// BEFORE the public-enum conversion, so the routing decision lives in exactly
/// one place. The public [`parse_request`] delegates here and maps the internal
/// variant to `-32601` (v1 behavior, byte-identical); the era-gated server
/// dispatch consumes the [`IngressRequest::Internal`] variant directly to reach
/// the internal handler on the v2 path.
///
/// # Measured transport reach (Phase 114 plan 13)
///
/// The ONLY production consumer of [`IngressRequest::Internal`] is
/// `classify_http_ingress` in `src/server/streamable_http_server.rs`. Every other
/// transport reaches requests through the PUBLIC [`parse_request`], which maps
/// `Internal` to [`Error::method_not_found`] — so an internally-routed method is
/// served over streamable HTTP and answers `-32601` everywhere else, including
/// stdio. That is the reach `server/discover` has had since Phase 112, and
/// `tasks/update` inherits exactly it. The seam is transport-AGNOSTIC (it lives in
/// `shared/`), so a later plan can widen the reach without a semver break; nothing
/// in Phase 114 claims it already has.
pub(crate) enum IngressRequest {
    /// A public typed request (the existing exhaustive-enum dispatch path).
    Public(Request),
    /// An internally-routed method with no public enum variant (v2-only).
    ///
    /// The payload is read by the native server dispatch. That path is compiled
    /// out on wasm32 and in a transport-less build, so the field has no reader
    /// THERE and nowhere else.
    #[cfg_attr(
        any(target_arch = "wasm32", not(feature = "streamable-http")),
        allow(dead_code)
    )]
    Internal(crate::types::protocol::InternalClientRequest),
}

/// Parse a raw JSON-RPC request, intercepting internally-routed methods BEFORE
/// the public-enum conversion (Phase 112, VERS-04).
///
/// This is the crate-private routing seam. `server/discover` and `tasks/update`
/// (and any future internally-routed method) are classified via
/// [`classify_internal_method`](crate::types::protocol::classify_internal_method)
/// and returned as [`IngressRequest::Internal`]; every other method flows
/// through the existing public-enum conversion as [`IngressRequest::Public`].
/// Unknown methods still resolve to [`Error::method_not_found`].
///
/// The `RequestId` is read HERE and returned as the first tuple element. That is
/// why no `InternalClientRequest` variant carries an id: the classifier below is
/// never given one, and a routing site takes it from this tuple.
pub(crate) fn parse_request_or_internal(
    request: JSONRPCRequest<Value>,
) -> Result<(RequestId, IngressRequest)> {
    let id = request.id;
    let method = request.method;
    let params = request.params.unwrap_or(Value::Null);

    // Intercept internally-routed methods (`server/discover`, `tasks/update`)
    // BEFORE the public-enum conversion — one routing decision, one place. The
    // params are handed over RAW and are NOT deserialized here.
    if let Some(internal) = crate::types::protocol::classify_internal_method(&method, &params) {
        return Ok((id, IngressRequest::Internal(internal)));
    }

    // Try to parse as client request first
    if let Ok(client_req) = parse_client_request(&method, &params) {
        return Ok((
            id,
            IngressRequest::Public(Request::Client(Box::new(client_req))),
        ));
    }

    // Try to parse as server request
    if let Ok(server_req) = parse_server_request(&method, &params) {
        return Ok((
            id,
            IngressRequest::Public(Request::Server(Box::new(server_req))),
        ));
    }

    Err(Error::method_not_found(&method))
}

/// Parse a JSON-RPC request into a typed Request.
///
/// Internally-routed methods that have no public enum variant (e.g. v2
/// `server/discover`) resolve to [`Error::method_not_found`] on this PUBLIC
/// entrypoint — the v1 `-32601` behavior, byte-identical to before. The
/// era-gated live routing to the internal handler is performed by the server
/// dispatch via the crate-private `parse_request_or_internal` seam (D-10).
pub fn parse_request(request: JSONRPCRequest<Value>) -> Result<(RequestId, Request)> {
    let method = request.method.clone();
    match parse_request_or_internal(request)? {
        (id, IngressRequest::Public(req)) => Ok((id, req)),
        (_, IngressRequest::Internal(_)) => Err(Error::method_not_found(&method)),
    }
}

/// Parse a notification from JSON.
pub fn parse_notification(value: Value) -> Result<Notification> {
    let notification: JSONRPCNotification<Value> = serde_json::from_value(value)
        .map_err(|e| Error::parse(format!("Invalid notification: {}", e)))?;

    let method = &notification.method;
    let params = notification.params.unwrap_or(Value::Null);

    // Check for special notification types
    if method == "notifications/progress" {
        let progress = serde_json::from_value(params)
            .map_err(|e| Error::parse(format!("Invalid progress notification: {}", e)))?;
        return Ok(Notification::Progress(progress));
    }

    if method == "notifications/cancelled" {
        let cancelled = serde_json::from_value(params)
            .map_err(|e| Error::parse(format!("Invalid cancelled notification: {}", e)))?;
        return Ok(Notification::Cancelled(cancelled));
    }

    // Try to parse as client notification
    if let Ok(client_notif) = parse_client_notification(method, &params) {
        return Ok(Notification::Client(client_notif));
    }

    // Try to parse as server notification
    if let Ok(server_notif) = parse_server_notification(method, &params) {
        return Ok(Notification::Server(server_notif));
    }

    Err(Error::method_not_found(method))
}

/// Create a JSON-RPC request from typed request.
pub fn create_request(id: RequestId, request: Request) -> JSONRPCRequest<Value> {
    match request {
        Request::Client(boxed_req) => {
            let client_req = *boxed_req;
            let (method, params) = client_request_to_jsonrpc(client_req);
            JSONRPCRequest::new(id, method, params)
        },
        Request::Server(server_req) => {
            let (method, params) = server_request_to_jsonrpc(*server_req);
            JSONRPCRequest::new(id, method, params)
        },
    }
}

/// Create a JSON-RPC notification from typed notification.
///
/// # Panics
///
/// Panics if serialization to JSON fails (should never happen with valid MCP types).
pub fn create_notification(notification: Notification) -> JSONRPCNotification<Value> {
    match notification {
        Notification::Client(client_notif) => {
            let (method, params) = client_notification_to_jsonrpc(client_notif);
            JSONRPCNotification::new(method, params)
        },
        Notification::Server(server_notif) => {
            let (method, params) = server_notification_to_jsonrpc(server_notif);
            JSONRPCNotification::new(method, params)
        },
        Notification::Progress(progress) => JSONRPCNotification::new(
            "notifications/progress",
            Some(serde_json::to_value(progress).unwrap()),
        ),
        Notification::Cancelled(cancelled) => JSONRPCNotification::new(
            "notifications/cancelled",
            Some(serde_json::to_value(cancelled).unwrap()),
        ),
    }
}

// Helper functions for parsing

fn parse_client_request(method: &str, params: &Value) -> Result<ClientRequest> {
    // For methods that don't accept params at all (like "ping"), we should not include
    // the params field. For methods that accept optional params, we convert null to empty object.
    let request_json = if method == "ping" {
        // Ping doesn't accept params at all
        serde_json::json!({
            "method": method,
        })
    } else if params.is_null() {
        // For methods with optional params, convert null to empty object
        serde_json::json!({
            "method": method,
            "params": {},
        })
    } else {
        // Include params as-is for methods that accept params
        serde_json::json!({
            "method": method,
            "params": params,
        })
    };

    serde_json::from_value(request_json)
        .map_err(|e| Error::parse(format!("Invalid client request: {}", e)))
}

fn parse_server_request(method: &str, params: &Value) -> Result<ServerRequest> {
    // For methods that don't accept params at all (like "roots/list"), we should not include
    // the params field. For methods that accept optional params, we convert null to empty object.
    let request_json = if method == "roots/list" {
        // roots/list doesn't accept params at all
        serde_json::json!({
            "method": method,
        })
    } else if params.is_null() {
        // For methods with optional params, convert null to empty object
        serde_json::json!({
            "method": method,
            "params": {},
        })
    } else {
        // Include params as-is for methods that accept params
        serde_json::json!({
            "method": method,
            "params": params,
        })
    };

    serde_json::from_value(request_json)
        .map_err(|e| Error::parse(format!("Invalid server request: {}", e)))
}

fn parse_client_notification(method: &str, params: &Value) -> Result<ClientNotification> {
    // For notifications that don't accept params, we should not include
    // the params field at all in the JSON object we construct for deserialization
    let notif_json = if matches!(
        method,
        "notifications/initialized" | "notifications/roots/list_changed"
    ) {
        // Don't include params field for parameterless notifications
        serde_json::json!({
            "method": method,
        })
    } else if params.is_null() {
        // For notifications with optional params, convert null to empty object
        serde_json::json!({
            "method": method,
            "params": {},
        })
    } else {
        // Include params field for notifications that accept params
        serde_json::json!({
            "method": method,
            "params": params,
        })
    };

    serde_json::from_value(notif_json)
        .map_err(|e| Error::parse(format!("Invalid client notification: {}", e)))
}

fn parse_server_notification(method: &str, params: &Value) -> Result<ServerNotification> {
    // For notifications that don't accept params, we should not include
    // the params field at all in the JSON object we construct for deserialization
    let notif_json = if matches!(
        method,
        "notifications/tools/list_changed"
            | "notifications/prompts/list_changed"
            | "notifications/resources/list_changed"
            | "notifications/roots/list_changed"
    ) {
        // Don't include params field for parameterless notifications
        serde_json::json!({
            "method": method,
        })
    } else if params.is_null() {
        // For notifications with optional params, convert null to empty object
        serde_json::json!({
            "method": method,
            "params": {},
        })
    } else {
        // Include params field for notifications that accept params
        serde_json::json!({
            "method": method,
            "params": params,
        })
    };

    serde_json::from_value(notif_json)
        .map_err(|e| Error::parse(format!("Invalid server notification: {}", e)))
}

fn client_request_to_jsonrpc(req: ClientRequest) -> (String, Option<Value>) {
    match req {
        // Core protocol requests
        ClientRequest::Initialize(params) => create_method_params("initialize", params),
        ClientRequest::Ping => ("ping".to_string(), None),
        ClientRequest::SetLoggingLevel { level } => (
            "logging/setLevel".to_string(),
            Some(serde_json::json!({"level": level})),
        ),
        // Tool requests
        ClientRequest::ListTools(params) => create_method_params("tools/list", params),
        ClientRequest::CallTool(params) => create_method_params("tools/call", params),
        // Prompt requests
        ClientRequest::ListPrompts(params) => create_method_params("prompts/list", params),
        ClientRequest::GetPrompt(params) => create_method_params("prompts/get", params),
        // Resource requests
        ClientRequest::ListResources(params) => create_method_params("resources/list", params),
        ClientRequest::ListResourceTemplates(params) => {
            create_method_params("resources/templates/list", params)
        },
        ClientRequest::ReadResource(params) => create_method_params("resources/read", params),
        ClientRequest::Subscribe(params) => create_method_params("resources/subscribe", params),
        ClientRequest::Unsubscribe(params) => create_method_params("resources/unsubscribe", params),
        // Completion requests
        ClientRequest::Complete(params) => create_method_params("completion/complete", params),
        // Sampling requests
        ClientRequest::CreateMessage(params) => {
            create_method_params("sampling/createMessage", params)
        },
        // Task requests (MCP 2025-11-25)
        ClientRequest::TasksGet(params) => create_method_params("tasks/get", params),
        ClientRequest::TasksResult(params) => create_method_params("tasks/result", params),
        ClientRequest::TasksList(params) => create_method_params("tasks/list", params),
        ClientRequest::TasksCancel(params) => create_method_params("tasks/cancel", params),
    }
}

/// Helper function to create method and params tuple.
fn create_method_params<T: serde::Serialize>(method: &str, params: T) -> (String, Option<Value>) {
    (
        method.to_string(),
        Some(serde_json::to_value(params).unwrap()),
    )
}

fn server_request_to_jsonrpc(req: ServerRequest) -> (String, Option<Value>) {
    match req {
        ServerRequest::CreateMessage(params) => (
            "sampling/createMessage".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerRequest::ListRoots => ("roots/list".to_string(), None),
        ServerRequest::ElicitationCreate(params) => (
            "elicitation/create".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

fn client_notification_to_jsonrpc(notif: ClientNotification) -> (String, Option<Value>) {
    match notif {
        ClientNotification::Initialized => ("notifications/initialized".to_string(), None),
        ClientNotification::RootsListChanged => {
            ("notifications/roots/list_changed".to_string(), None)
        },
        ClientNotification::Cancelled(params) => (
            "notifications/cancelled".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientNotification::Progress(params) => (
            "notifications/progress".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

fn server_notification_to_jsonrpc(notif: ServerNotification) -> (String, Option<Value>) {
    match notif {
        ServerNotification::Progress(params) => (
            "notifications/progress".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerNotification::ToolsChanged => ("notifications/tools/list_changed".to_string(), None),
        ServerNotification::PromptsChanged => {
            ("notifications/prompts/list_changed".to_string(), None)
        },
        ServerNotification::ResourcesChanged => {
            ("notifications/resources/list_changed".to_string(), None)
        },
        ServerNotification::RootsListChanged => {
            ("notifications/roots/list_changed".to_string(), None)
        },
        ServerNotification::ResourceUpdated(params) => (
            "notifications/resources/updated".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerNotification::LogMessage(params) => (
            "notifications/message".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerNotification::TaskStatus(params) => (
            "notifications/tasks/status".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

/// SIMD-accelerated JSON-RPC parsing functions
///
/// These functions provide high-performance alternatives to standard parsing
/// by leveraging SIMD optimizations when available on the target CPU.
/// Parse a JSON-RPC request from raw bytes using SIMD optimization.
pub fn parse_request_bytes(data: &[u8]) -> Result<(RequestId, Request)> {
    let request = SIMD_PARSER
        .parse_request(data)
        .map_err(|e| Error::parse(format!("SIMD JSON parsing failed: {}", e)))?;
    parse_request(request)
}

/// Parse a JSON-RPC response from raw bytes using SIMD optimization.
pub fn parse_response_bytes(data: &[u8]) -> Result<crate::types::jsonrpc::JSONRPCResponse> {
    SIMD_PARSER
        .parse_response(data)
        .map_err(|e| Error::parse(format!("SIMD JSON response parsing failed: {}", e)))
}

/// Parse a batch of JSON-RPC requests from raw bytes using SIMD optimization with parallel processing.
pub fn parse_batch_requests_bytes(data: &[u8]) -> Result<Vec<(RequestId, Request)>> {
    let requests = SIMD_PARSER
        .parse_batch_requests(data)
        .map_err(|e| Error::parse(format!("SIMD batch parsing failed: {}", e)))?;

    requests
        .into_iter()
        .map(parse_request)
        .collect::<Result<Vec<_>>>()
}

/// Parse a batch of JSON-RPC responses from raw bytes using SIMD optimization.
pub fn parse_batch_responses_bytes(
    data: &[u8],
) -> Result<Vec<crate::types::jsonrpc::JSONRPCResponse>> {
    SIMD_PARSER
        .parse_batch_responses(data)
        .map_err(|e| Error::parse(format!("SIMD batch response parsing failed: {}", e)))
}

/// Get SIMD parsing performance metrics.
pub fn get_simd_parsing_metrics() -> crate::shared::simd_parsing::ParsingMetrics {
    SIMD_PARSER.get_metrics()
}

/// Check if SIMD features are available on the current CPU.
pub fn get_cpu_features() -> crate::shared::simd_parsing::CpuFeatures {
    SIMD_PARSER.get_cpu_features()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CallToolRequest, CancelledNotification, ClientCapabilities, CompleteRequest,
        CompletionArgument, CompletionReference, GetPromptRequest, Implementation,
        InitializeRequest, ListPromptsRequest, ListResourceTemplatesRequest, ListResourcesRequest,
        ListToolsRequest, LoggingLevel, ProgressNotification, ProgressToken, ReadResourceRequest,
        SubscribeRequest, UnsubscribeRequest,
    };
    use serde_json::json;

    #[test]
    fn test_parse_client_request_initialize() {
        let id = RequestId::from(1i64);
        let method = "initialize";
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ref boxed) if matches!(**boxed, ClientRequest::Initialize(_)) => (),
            _ => panic!("Expected Initialize request"),
        }
    }

    #[test]
    fn test_parse_client_request_list_tools() {
        let id = RequestId::from(2i64);
        let method = "tools/list";
        let params = json!({ "cursor": null });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ref boxed) if matches!(**boxed, ClientRequest::ListTools(_)) => (),
            _ => panic!("Expected ListTools request"),
        }
    }

    #[test]
    fn test_parse_client_request_call_tool() {
        let id = RequestId::from(3i64);
        let method = "tools/call";
        let params = json!({
            "name": "test-tool",
            "arguments": {"input": "test"}
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ref boxed) if matches!(**boxed, ClientRequest::CallTool(_)) => (),
            _ => panic!("Expected CallTool request"),
        }
    }

    #[test]
    fn test_parse_client_request_ping() {
        let id = RequestId::from(4i64);
        let method = "ping";

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), None);
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ref boxed) if matches!(**boxed, ClientRequest::Ping) => (),
            _ => panic!("Expected Ping request"),
        }
    }

    #[test]
    fn test_parse_server_request_create_message() {
        let id = RequestId::from(5i64);
        let method = "sampling/createMessage";
        let params = json!({
            "messages": [],
            "includeContext": "none"
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ref boxed) if matches!(**boxed, ClientRequest::CreateMessage(_)) => (),
            _ => panic!("Expected CreateMessage request"),
        }
    }

    #[test]
    fn test_parse_request_or_internal_routes_server_discover() {
        // `server/discover` is intercepted as an Internal ingress request BEFORE
        // the public-enum conversion (VERS-04 routing seam).
        let req = JSONRPCRequest::new(
            RequestId::from(1i64),
            "server/discover".to_string(),
            Some(json!({})),
        );
        let (_id, ingress) = parse_request_or_internal(req).unwrap();
        assert!(matches!(ingress, IngressRequest::Internal(_)));

        // A normal method flows through as Public.
        let req2 = JSONRPCRequest::new(
            RequestId::from(2i64),
            "tools/list".to_string(),
            Some(json!({})),
        );
        let (_id2, ingress2) = parse_request_or_internal(req2).unwrap();
        assert!(matches!(ingress2, IngressRequest::Public(_)));

        // On the PUBLIC entrypoint, `server/discover` still resolves to -32601.
        let req3 = JSONRPCRequest::new(
            RequestId::from(3i64),
            "server/discover".to_string(),
            Some(json!({})),
        );
        assert!(parse_request(req3)
            .unwrap_err()
            .to_string()
            .contains("Method not found"));
    }

    /// `tasks/update` classifies as Internal, carries its params VERBATIM, and
    /// still answers `-32601` on the public entrypoint (Phase 114, TASK-02).
    ///
    /// The verbatim half is the load-bearing one: it is what proves the classifier
    /// does not deserialize, which is what keeps a malformed body from becoming a
    /// parse error ahead of the `-32003` auth refusal. The params below are
    /// deliberately NOT a well-formed `tasks/update` payload.
    #[test]
    fn test_parse_request_or_internal_routes_tasks_update_with_raw_params() {
        let garbage = json!({ "taskId": 17, "wat": [1, 2, 3] });
        let req = JSONRPCRequest::new(
            RequestId::from(7i64),
            "tasks/update".to_string(),
            Some(garbage.clone()),
        );
        let (id, ingress) = parse_request_or_internal(req).unwrap();
        assert_eq!(
            id,
            RequestId::from(7i64),
            "the id comes off the OUTER tuple"
        );
        match ingress {
            IngressRequest::Internal(
                crate::types::protocol::InternalClientRequest::TasksUpdate { params },
            ) => {
                assert_eq!(
                    params, garbage,
                    "the classifier must pass params through RAW"
                );
            },
            // Enumerated rather than `_`: this match is a compile-time tripwire
            // over `InternalClientRequest` too, and a wildcard would silently
            // absorb a future internally-routed method.
            IngressRequest::Public(_)
            | IngressRequest::Internal(
                crate::types::protocol::InternalClientRequest::ServerDiscover(_)
                | crate::types::protocol::InternalClientRequest::SkillsList { .. },
            ) => {
                panic!("tasks/update must classify as InternalClientRequest::TasksUpdate")
            },
        }

        // A frame with NO params at all still classifies — rejecting it here would
        // be the classifier judging a body.
        let bare = JSONRPCRequest::new(RequestId::from(8i64), "tasks/update".to_string(), None);
        let (_id, bare_ingress) = parse_request_or_internal(bare).unwrap();
        assert!(matches!(
            bare_ingress,
            IngressRequest::Internal(
                crate::types::protocol::InternalClientRequest::TasksUpdate { .. }
            )
        ));

        // The PUBLIC entrypoint keeps the v1-byte-identical `-32601`.
        let public = JSONRPCRequest::new(
            RequestId::from(9i64),
            "tasks/update".to_string(),
            Some(json!({})),
        );
        assert!(parse_request(public)
            .unwrap_err()
            .to_string()
            .contains("Method not found"));
    }

    #[test]
    fn test_parse_request_unknown_method() {
        let id = RequestId::from(6i64);
        let method = "unknown/method";

        let request = JSONRPCRequest::new(id, method.to_string(), None);
        let result = parse_request(request);

        assert!(result.is_err());
        let error_str = result.unwrap_err().to_string();
        assert!(error_str.contains("Method not found"));
    }

    #[test]
    fn test_parse_notification_progress() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "progressToken": "test-token",
                "progress": 50.0,
                "message": "Processing..."
            }
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Progress(progress) => {
                assert_eq!(
                    progress.progress_token,
                    ProgressToken::String("test-token".to_string())
                );
                assert!((progress.progress - 50.0).abs() < f64::EPSILON);
                assert_eq!(progress.message, Some("Processing...".to_string()));
            },
            _ => panic!("Expected Progress notification"),
        }
    }

    #[test]
    fn test_parse_notification_cancelled() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {
                "requestId": "test-request",
                "reason": "User cancelled"
            }
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Cancelled(cancelled) => {
                assert_eq!(
                    cancelled.request_id,
                    RequestId::String("test-request".to_string())
                );
                assert_eq!(cancelled.reason, Some("User cancelled".to_string()));
            },
            _ => panic!("Expected Cancelled notification"),
        }
    }

    #[test]
    fn test_parse_client_notification_initialized() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Client(ClientNotification::Initialized) => (),
            _ => panic!("Expected Initialized notification"),
        }
    }

    #[test]
    fn test_parse_server_notification_tools_changed() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed"
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Server(ServerNotification::ToolsChanged) => (),
            _ => panic!("Expected ToolsChanged notification"),
        }
    }

    #[test]
    fn test_parse_notification_unknown_method() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "unknown/notification"
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[test]
    fn test_parse_notification_invalid_json() {
        let invalid_json = json!("not a notification");
        let result = parse_notification(invalid_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid notification"));
    }

    #[test]
    fn test_create_client_request_initialize() {
        let id = RequestId::from(1i64);
        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "initialize");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_list_tools() {
        let id = RequestId::from(2i64);
        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "tools/list");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_call_tool() {
        let id = RequestId::from(3i64);
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test-tool".to_string(),
            arguments: json!({"input": "test"}),
            _meta: None,
            task: None,
        })));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "tools/call");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_ping() {
        let id = RequestId::from(4i64);
        let request = Request::Client(Box::new(ClientRequest::Ping));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "ping");
        assert!(jsonrpc_request.params.is_none());
    }

    #[test]
    fn test_create_client_request_set_logging_level() {
        let id = RequestId::from(5i64);
        let request = Request::Client(Box::new(ClientRequest::SetLoggingLevel {
            level: LoggingLevel::Debug,
        }));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "logging/setLevel");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_server_request_create_message() {
        let id = RequestId::from(6i64);
        let request = Request::Server(Box::new(ServerRequest::CreateMessage(Box::new(
            crate::types::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
                tools: None,
                tool_choice: None,
            },
        ))));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "sampling/createMessage");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_notification_client_initialized() {
        let notification = Notification::Client(ClientNotification::Initialized);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/initialized");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_client_roots_list_changed() {
        let notification = Notification::Client(ClientNotification::RootsListChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/roots/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_progress() {
        let progress = ProgressNotification::new(
            ProgressToken::String("test".to_string()),
            75.0,
            Some("Almost done".to_string()),
        );
        let notification = Notification::Progress(progress);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/progress");
        assert!(jsonrpc_notif.params.is_some());
    }

    #[test]
    fn test_create_notification_cancelled() {
        let cancelled = CancelledNotification::new(RequestId::String("test-req".to_string()))
            .with_reason("Timeout");
        let notification = Notification::Cancelled(cancelled);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/cancelled");
        assert!(jsonrpc_notif.params.is_some());
    }

    #[test]
    fn test_create_notification_server_tools_changed() {
        let notification = Notification::Server(ServerNotification::ToolsChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/tools/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_server_prompts_changed() {
        let notification = Notification::Server(ServerNotification::PromptsChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/prompts/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_server_resources_changed() {
        let notification = Notification::Server(ServerNotification::ResourcesChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/resources/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_client_request_to_jsonrpc_all_variants() {
        // Test all ClientRequest variants to ensure complete coverage
        let test_cases = vec![
            (
                ClientRequest::ListPrompts(ListPromptsRequest { cursor: None }),
                "prompts/list",
            ),
            (
                ClientRequest::GetPrompt(GetPromptRequest {
                    name: "test".to_string(),
                    arguments: std::collections::HashMap::new(),
                    _meta: None,
                }),
                "prompts/get",
            ),
            (
                ClientRequest::ListResources(ListResourcesRequest { cursor: None }),
                "resources/list",
            ),
            (
                ClientRequest::ListResourceTemplates(ListResourceTemplatesRequest { cursor: None }),
                "resources/templates/list",
            ),
            (
                ClientRequest::ReadResource(ReadResourceRequest {
                    uri: "test://uri".to_string(),
                    _meta: None,
                }),
                "resources/read",
            ),
            (
                ClientRequest::Subscribe(SubscribeRequest {
                    uri: "test://uri".to_string(),
                }),
                "resources/subscribe",
            ),
            (
                ClientRequest::Unsubscribe(UnsubscribeRequest {
                    uri: "test://uri".to_string(),
                }),
                "resources/unsubscribe",
            ),
            (
                ClientRequest::Complete(CompleteRequest {
                    r#ref: CompletionReference::Resource {
                        uri: "test://uri".to_string(),
                    },
                    argument: CompletionArgument {
                        name: "test".to_string(),
                        value: "val".to_string(),
                    },
                }),
                "completion/complete",
            ),
        ];

        for (request, expected_method) in test_cases {
            let (method, params) = client_request_to_jsonrpc(request);
            assert_eq!(method, expected_method);
            assert!(params.is_some());
        }
    }

    #[test]
    fn test_client_notification_to_jsonrpc_all_variants() {
        let cancelled = CancelledNotification::new(RequestId::String("test".to_string()));
        let progress =
            ProgressNotification::new(ProgressToken::String("test".to_string()), 50.0, None);

        let test_cases = vec![
            (
                ClientNotification::Cancelled(cancelled),
                "notifications/cancelled",
                true,
            ),
            (
                ClientNotification::Progress(progress),
                "notifications/progress",
                true,
            ),
        ];

        for (notification, expected_method, should_have_params) in test_cases {
            let (method, params) = client_notification_to_jsonrpc(notification);
            assert_eq!(method, expected_method);
            assert_eq!(params.is_some(), should_have_params);
        }
    }

    #[test]
    fn test_server_notification_to_jsonrpc_all_variants() {
        let progress =
            ProgressNotification::new(ProgressToken::String("test".to_string()), 25.0, None);
        let resource_updated = crate::types::ResourceUpdatedParams::new("test://uri");
        let log_message = crate::types::LogMessageParams::new(crate::types::LoggingLevel::Info, "");

        let test_cases = vec![
            (
                ServerNotification::Progress(progress),
                "notifications/progress",
                true,
            ),
            (
                ServerNotification::ResourceUpdated(resource_updated),
                "notifications/resources/updated",
                true,
            ),
            (
                ServerNotification::LogMessage(log_message),
                "notifications/message",
                true,
            ),
        ];

        for (notification, expected_method, should_have_params) in test_cases {
            let (method, params) = server_notification_to_jsonrpc(notification);
            assert_eq!(method, expected_method);
            assert_eq!(params.is_some(), should_have_params);
        }
    }

    #[test]
    fn test_parse_invalid_progress_notification() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "invalid": "data"
            }
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid progress notification"));
    }

    #[test]
    fn test_parse_invalid_cancelled_notification() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {
                "invalid": "data"
            }
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid cancelled notification"));
    }

    #[test]
    fn test_roundtrip_request_parsing() {
        // Test that we can create a request and parse it back
        let original_id = RequestId::from(42i64);
        let original_request = Request::Client(Box::new(ClientRequest::Ping));

        let jsonrpc_request = create_request(original_id.clone(), original_request.clone());
        let (parsed_id, parsed_request) = parse_request(jsonrpc_request).unwrap();

        assert_eq!(parsed_id, original_id);
        match (&original_request, &parsed_request) {
            (Request::Client(boxed1), Request::Client(boxed2)) => {
                match (boxed1.as_ref(), boxed2.as_ref()) {
                    (ClientRequest::Ping, ClientRequest::Ping) => (),
                    _ => panic!("Roundtrip failed - request type mismatch"),
                }
            },
            _ => panic!("Roundtrip failed - request category mismatch"),
        }
    }

    #[test]
    fn test_roundtrip_notification_parsing() {
        // Test that we can create a notification and parse it back
        let original_notification = Notification::Client(ClientNotification::Initialized);

        let jsonrpc_notif = create_notification(original_notification.clone());
        let notification_value = serde_json::to_value(&jsonrpc_notif).unwrap();
        let parsed_notification = parse_notification(notification_value).unwrap();

        match (original_notification, parsed_notification) {
            (
                Notification::Client(ClientNotification::Initialized),
                Notification::Client(ClientNotification::Initialized),
            ) => (),
            _ => panic!("Roundtrip failed"),
        }
    }
}
