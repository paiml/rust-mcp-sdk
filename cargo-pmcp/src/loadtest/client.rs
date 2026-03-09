//! MCP-aware HTTP client for load testing.
//!
//! Each virtual user owns one [`McpClient`] instance with its own session.
//! The client performs the full MCP initialize handshake, manages the
//! `mcp-session-id` header, and classifies errors into distinct categories.

use crate::loadtest::error::McpError;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// MCP protocol version used in the initialize handshake.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Client name sent in the initialize handshake.
const CLIENT_NAME: &str = "cargo-pmcp-loadtest";

/// HTTP header name for the MCP session identifier.
const SESSION_HEADER: &str = "mcp-session-id";

/// MCP-aware HTTP client for load testing.
///
/// Each virtual user owns one instance with its own session. The client
/// constructs JSON-RPC requests directly using [`serde_json::json!`] rather
/// than depending on the parent SDK's transport layer.
///
/// # Session lifecycle
///
/// 1. Call [`McpClient::initialize`] to perform the full MCP handshake.
/// 2. The `mcp-session-id` header is automatically extracted and stored.
/// 3. Subsequent calls to [`McpClient::call_tool`], [`McpClient::read_resource`],
///    and [`McpClient::get_prompt`] attach the session header automatically.
pub struct McpClient {
    http: Client,
    base_url: String,
    session_id: Option<String>,
    request_timeout: Duration,
    next_request_id: u64,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
}

impl McpClient {
    /// Creates a new MCP client.
    ///
    /// Takes a [`reqwest::Client`] by value (allows shared client via `Clone`
    /// for connection pool sharing in Phase 2), a base URL for the MCP server,
    /// and a per-request timeout duration.
    ///
    /// Starts with no session and request ID counter at 1.
    pub fn new(
        http: Client,
        base_url: String,
        timeout: Duration,
        http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,
    ) -> Self {
        Self {
            http,
            base_url,
            session_id: None,
            request_timeout: timeout,
            next_request_id: 1,
            http_middleware_chain,
        }
    }

    /// Returns the current request ID and increments the counter.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    /// Returns the current session ID, if one has been established.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Returns the base URL of the MCP server.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Builds the JSON-RPC request body for the `initialize` method.
    ///
    /// Includes the MCP protocol version and client identification info.
    pub fn build_initialize_body(&mut self) -> Value {
        let id = self.next_id();
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": CLIENT_NAME,
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        })
    }

    /// Builds the JSON-RPC notification body for `notifications/initialized`.
    ///
    /// This is a notification (no `id` field) sent after a successful
    /// initialize response to complete the handshake.
    pub fn build_initialized_notification(&self) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        })
    }

    /// Builds the JSON-RPC request body for `tools/call`.
    pub fn build_tool_call_body(&mut self, tool: &str, arguments: &Value) -> Value {
        let id = self.next_id();
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": arguments
            }
        })
    }

    /// Builds the JSON-RPC request body for `resources/read`.
    pub fn build_resource_read_body(&mut self, uri: &str) -> Value {
        let id = self.next_id();
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "resources/read",
            "params": {
                "uri": uri
            }
        })
    }

    /// Builds the JSON-RPC request body for `prompts/get`.
    pub fn build_prompt_get_body(
        &mut self,
        prompt: &str,
        arguments: &HashMap<String, String>,
    ) -> Value {
        let id = self.next_id();
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "prompts/get",
            "params": {
                "name": prompt,
                "arguments": arguments
            }
        })
    }

    /// Extracts the `mcp-session-id` header from response headers and stores it.
    ///
    /// If the header is not present or not valid UTF-8, the session ID is
    /// left unchanged.
    pub fn extract_session_id(&mut self, headers: &reqwest::header::HeaderMap) {
        if let Some(value) = headers.get(SESSION_HEADER) {
            if let Ok(s) = value.to_str() {
                self.session_id = Some(s.to_owned());
            }
        }
    }

    /// Parses a JSON-RPC response body.
    ///
    /// Returns the `result` field on success, or an [`McpError::JsonRpc`] if
    /// the response contains an `error` object.
    pub fn parse_response(body: &[u8]) -> Result<Value, McpError> {
        let parsed: Value = serde_json::from_slice(body).map_err(|e| McpError::Connection {
            message: format!("Invalid JSON response: {e}"),
        })?;

        if let Some(error_obj) = parsed.get("error") {
            let code = error_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
            let message = error_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_owned();
            return Err(McpError::JsonRpc { code, message });
        }

        if let Some(result) = parsed.get("result") {
            return Ok(result.clone());
        }

        // If there is neither result nor error, return the full response
        Ok(parsed)
    }

    /// Performs the full MCP initialize handshake.
    ///
    /// 1. Sends the `initialize` request.
    /// 2. Extracts the `mcp-session-id` from response headers.
    /// 3. Parses the JSON-RPC response.
    /// 4. Sends the `notifications/initialized` notification with the session
    ///    ID attached.
    ///
    /// Returns the initialize result value.
    pub async fn initialize(&mut self) -> Result<Value, McpError> {
        let body = self.build_initialize_body();
        let (headers, response_bytes) = self.send_request(&body).await?;

        self.extract_session_id(&headers);
        let result = Self::parse_response(&response_bytes)?;

        // Send the initialized notification to complete the handshake
        let notification = self.build_initialized_notification();
        let _ = self.send_request(&notification).await;

        Ok(result)
    }

    /// Sends a `tools/call` request to the MCP server.
    pub async fn call_tool(&mut self, tool: &str, arguments: &Value) -> Result<Value, McpError> {
        let body = self.build_tool_call_body(tool, arguments);
        let (_headers, response_bytes) = self.send_request(&body).await?;
        Self::parse_response(&response_bytes)
    }

    /// Sends a `resources/read` request to the MCP server.
    pub async fn read_resource(&mut self, uri: &str) -> Result<Value, McpError> {
        let body = self.build_resource_read_body(uri);
        let (_headers, response_bytes) = self.send_request(&body).await?;
        Self::parse_response(&response_bytes)
    }

    /// Sends a `prompts/get` request to the MCP server.
    pub async fn get_prompt(
        &mut self,
        prompt: &str,
        arguments: &HashMap<String, String>,
    ) -> Result<Value, McpError> {
        let body = self.build_prompt_get_body(prompt, arguments);
        let (_headers, response_bytes) = self.send_request(&body).await?;
        Self::parse_response(&response_bytes)
    }

    /// Executes a code_mode two-step flow: `validate_code` then `execute_code`.
    ///
    /// Step 1: Calls `validate_code` tool with the code and format.
    /// Step 2: Extracts the `approval_token` from the response and calls
    /// `execute_code` with the same code and the token.
    pub async fn execute_code_mode(&mut self, code: &str, format: &str) -> Result<Value, McpError> {
        // Step 1: validate_code
        let validate_args = json!({
            "code": code,
            "format": format
        });
        let validate_result = self.call_tool("validate_code", &validate_args).await?;

        // Extract approval_token from the validate_code response
        let approval_token = validate_result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|item| item.get("type") == Some(&json!("text")))
            })
            .and_then(|item| item.get("text"))
            .and_then(|text| serde_json::from_str::<Value>(text.as_str().unwrap_or("")).ok())
            .and_then(|parsed| parsed.get("approval_token").cloned())
            .and_then(|t| t.as_str().map(|s| s.to_string()))
            .ok_or_else(|| McpError::JsonRpc {
                code: -1,
                message: "validate_code response missing approval_token".to_string(),
            })?;

        // Step 2: execute_code
        let execute_args = json!({
            "code": code,
            "approval_token": approval_token
        });
        self.call_tool("execute_code", &execute_args).await
    }

    /// Sends an HTTP POST request with the given JSON-RPC body.
    ///
    /// Attaches the `mcp-session-id` header if a session has been established.
    /// Applies per-request timeout via [`reqwest::RequestBuilder::timeout`].
    /// Returns response headers and body bytes.
    ///
    /// **Timing boundary:** The caller captures `Instant::now()` before calling
    /// this method. The returned bytes represent the raw response -- JSON parsing
    /// happens after timing measurement is complete, ensuring parse time is not
    /// included in latency measurements.
    async fn send_request(
        &mut self,
        body: &Value,
    ) -> Result<(reqwest::header::HeaderMap, Vec<u8>), McpError> {
        if let Some(ref chain) = self.http_middleware_chain {
            use pmcp::client::http_middleware::{HttpMiddlewareContext, HttpRequest as MwRequest};

            let body_bytes = serde_json::to_vec(body).map_err(|e| McpError::Connection {
                message: format!("JSON serialize: {e}"),
            })?;
            let mut http_req =
                MwRequest::new("POST".to_string(), self.base_url.clone(), body_bytes);
            http_req.add_header("Content-Type", "application/json");
            http_req.add_header("Accept", "application/json, text/event-stream");
            if let Some(ref sid) = self.session_id {
                http_req.add_header(SESSION_HEADER, sid);
            }

            let context = HttpMiddlewareContext::new(self.base_url.clone(), "POST".to_string());
            chain
                .process_request(&mut http_req, &context)
                .await
                .map_err(|e| McpError::Connection {
                    message: format!("Middleware error: {e}"),
                })?;

            let mut request = self.http.post(&self.base_url).timeout(self.request_timeout);
            for (key, value) in &http_req.headers {
                request = request.header(key, value);
            }
            request = request.body(http_req.body);

            let response = request
                .send()
                .await
                .map_err(|e| McpError::classify_reqwest(&e))?;

            let status = response.status();
            let headers = response.headers().clone();
            let bytes = response
                .bytes()
                .await
                .map_err(|e| McpError::classify_reqwest(&e))?;

            if !status.is_success() {
                let body_text = String::from_utf8_lossy(&bytes).into_owned();
                return Err(McpError::Http {
                    status: status.as_u16(),
                    body: body_text,
                });
            }

            Ok((headers, bytes.to_vec()))
        } else {
            let mut request = self
                .http
                .post(&self.base_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .timeout(self.request_timeout)
                .json(body);

            if let Some(ref sid) = self.session_id {
                request = request.header(SESSION_HEADER, sid.as_str());
            }

            let response = request
                .send()
                .await
                .map_err(|e| McpError::classify_reqwest(&e))?;

            let status = response.status();
            let headers = response.headers().clone();

            // CRITICAL timing boundary: capture bytes BEFORE any JSON parsing.
            // The caller uses the time before send_request() and after it returns
            // to compute latency. JSON parse time must NOT be included.
            let bytes = response
                .bytes()
                .await
                .map_err(|e| McpError::classify_reqwest(&e))?;

            if !status.is_success() {
                let body_text = String::from_utf8_lossy(&bytes).into_owned();
                return Err(McpError::Http {
                    status: status.as_u16(),
                    body: body_text,
                });
            }

            Ok((headers, bytes.to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_client() -> McpClient {
        McpClient::new(
            Client::new(),
            "http://localhost:3000".to_string(),
            Duration::from_secs(5),
            None,
        )
    }

    #[test]
    fn test_new_client_has_no_session() {
        let client = make_client();
        assert!(client.session_id().is_none());
    }

    #[test]
    fn test_next_id_increments() {
        let mut client = make_client();
        assert_eq!(client.next_id(), 1);
        assert_eq!(client.next_id(), 2);
        assert_eq!(client.next_id(), 3);
    }

    #[test]
    fn test_build_initialize_request_body() {
        let mut client = make_client();
        let body = client.build_initialize_body();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "initialize");
        assert_eq!(body["params"]["protocolVersion"], "2025-06-18");
        assert_eq!(body["params"]["clientInfo"]["name"], "cargo-pmcp-loadtest");
        assert_eq!(
            body["params"]["clientInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_initialized_notification_body() {
        let client = make_client();
        let body = client.build_initialized_notification();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "notifications/initialized");
        assert!(body.get("id").is_none() || body["id"].is_null());
    }

    #[test]
    fn test_build_tool_call_body() {
        let mut client = make_client();
        let args = json!({"expression": "2+2"});
        let body = client.build_tool_call_body("calculate", &args);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "tools/call");
        assert_eq!(body["params"]["name"], "calculate");
        assert_eq!(body["params"]["arguments"]["expression"], "2+2");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_resource_read_body() {
        let mut client = make_client();
        let body = client.build_resource_read_body("file:///data.json");
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "resources/read");
        assert_eq!(body["params"]["uri"], "file:///data.json");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_build_prompt_get_body() {
        let mut client = make_client();
        let mut arguments = HashMap::new();
        arguments.insert("text".to_string(), "hello".to_string());
        let body = client.build_prompt_get_body("summarize", &arguments);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["method"], "prompts/get");
        assert_eq!(body["params"]["name"], "summarize");
        assert_eq!(body["params"]["arguments"]["text"], "hello");
        assert!(body["id"].is_u64());
    }

    #[test]
    fn test_parse_session_id_from_headers() {
        let mut client = make_client();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("mcp-session-id", "test-session-123".parse().unwrap());
        client.extract_session_id(&headers);
        assert_eq!(client.session_id(), Some("test-session-123"));
    }

    #[test]
    fn test_parse_jsonrpc_error_response() {
        let body =
            br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let result = McpClient::parse_response(body);
        match result {
            Err(McpError::JsonRpc { code, message }) => {
                assert_eq!(code, -32601);
                assert_eq!(message, "Method not found");
            },
            other => panic!("Expected McpError::JsonRpc, got: {:?}", other),
        }
    }

    #[test]
    fn test_parse_jsonrpc_success_response() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let result = McpClient::parse_response(body).expect("should parse ok");
        assert!(result.get("capabilities").is_some());
    }

    #[test]
    fn test_classify_reqwest_timeout() {
        // Test error_category on a manually constructed McpError::Timeout
        let err = McpError::Timeout;
        assert_eq!(err.error_category(), "timeout");
    }

    #[test]
    fn test_error_category_returns_correct_strings() {
        assert_eq!(
            McpError::JsonRpc {
                code: -32600,
                message: "Bad".to_string()
            }
            .error_category(),
            "jsonrpc"
        );
        assert_eq!(
            McpError::Http {
                status: 500,
                body: "err".to_string()
            }
            .error_category(),
            "http"
        );
        assert_eq!(McpError::Timeout.error_category(), "timeout");
        assert_eq!(
            McpError::Connection {
                message: "err".to_string()
            }
            .error_category(),
            "connection"
        );
    }

    #[test]
    fn test_client_accepts_middleware_chain() {
        let chain = Arc::new(HttpMiddlewareChain::new());
        let client = McpClient::new(
            Client::new(),
            "http://localhost:3000".to_string(),
            Duration::from_secs(5),
            Some(chain),
        );
        assert!(client.http_middleware_chain.is_some());
    }

    #[tokio::test]
    async fn test_timeout_fires_on_slow_server() {
        use tokio::net::TcpListener;

        // Start a local TCP listener that accepts but never responds
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a task that accepts connections but sleeps forever
        tokio::spawn(async move {
            loop {
                let (_socket, _) = listener.accept().await.unwrap();
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        let mut client = McpClient::new(
            Client::new(),
            format!("http://{}", addr),
            Duration::from_millis(200),
            None,
        );

        let start = std::time::Instant::now();
        let result = client.initialize().await;
        let elapsed = start.elapsed();

        // Must complete within 2 seconds
        assert!(
            elapsed < Duration::from_secs(2),
            "Timeout test took {:?}, expected < 2s",
            elapsed
        );

        // Must be an error (either Timeout or Connection is acceptable)
        assert!(result.is_err(), "Expected error from slow server");
        let err = result.unwrap_err();
        let cat = err.error_category();
        assert!(
            cat == "timeout" || cat == "connection",
            "Expected timeout or connection error, got: {}",
            cat
        );
    }
}
