//! Transport domain conformance scenarios.
//!
//! Validates the raw HTTP surface of a Streamable-HTTP MCP server beyond what
//! the JSON-RPC-over-POST `Core` domain can see:
//!
//! - `GET /mcp` — must return either an SSE stream (200 + `text/event-stream`)
//!   or `405 Method Not Allowed` with a JSON-RPC error body. Catches the
//!   common regression where a reverse proxy rewrites `GET /mcp` to a JSON
//!   health endpoint (`200 OK + application/json + {"ok":true,...}`), which
//!   silently breaks spec-compliant SSE clients.
//! - `OPTIONS /mcp` — must return CORS preflight headers (any 2xx with at
//!   least one `Access-Control-*` header) or `405`.
//! - `DELETE /mcp` — session termination. Spec-compliant responses include
//!   `200`/`204` (terminated), `405` (stateless), or any `4xx` carrying a
//!   JSON-RPC error envelope (stateful server correctly rejecting a probe
//!   that omits `Mcp-Session-Id`). Anything else is surfaced as a warning,
//!   not a failure, since session-termination semantics vary across servers.
//!
//! All probes go through the existing `HttpMiddlewareChain` produced by
//! `cargo pmcp auth`, so OAuth-protected servers are exercised with the
//! same bearer the rest of the conformance suite already negotiated. The
//! domain SKIPs cleanly on Stdio and JSON-RPC-only HTTP servers.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::{ServerTester, TransportType};
use pmcp::client::http_middleware::{HttpMiddlewareContext, HttpRequest};
use reqwest::{Client, StatusCode};
use std::time::{Duration, Instant};

/// Maximum bytes captured from a probe response body. Bounds the classifier
/// input and prevents real SSE servers from streaming indefinitely into our
/// buffer.
const MAX_BODY_BYTES: usize = 256;

/// Maximum body-prefix length embedded in failure-detail strings.
const MAX_BODY_PREFIX_IN_DETAIL: usize = 200;

/// Per-probe receive timeout. Hard upper bound; combined with `tester.timeout()`
/// via `min(...)` so the probe never out-runs the operator-configured budget.
const PROBE_RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);

const CT_JSON: &str = "application/json";
const CT_SSE: &str = "text/event-stream";

/// Run all transport-domain conformance scenarios.
///
/// SKIPs cleanly when the tester is not running against a Streamable-HTTP
/// endpoint (Stdio / JsonRpcHttp); otherwise runs three scenarios in order:
/// `GET /mcp`, `OPTIONS /mcp`, `DELETE /mcp`. A single `reqwest::Client` is
/// reused across all three probes so TLS/DNS work happens once.
pub async fn run_transport_conformance(tester: &ServerTester) -> Vec<TestResult> {
    if let Some(skip) = transport_skip_for_non_http(&tester.transport_type) {
        return vec![skip];
    }

    let client = match build_probe_client(tester) {
        Ok(c) => c,
        Err(e) => {
            return vec![TestResult::failed(
                "Transport: probe client",
                TestCategory::Transport,
                Duration::ZERO,
                e,
            )];
        },
    };

    vec![
        test_get_mcp_returns_sse_or_405(tester, &client).await,
        test_options_mcp_returns_cors_or_405(tester, &client).await,
        test_delete_mcp_returns_session_termination_or_405(tester, &client).await,
    ]
}

/// SKIP shim for non-Streamable-HTTP transports.
fn transport_skip_for_non_http(transport: &TransportType) -> Option<TestResult> {
    let label = match transport {
        TransportType::Http => return None,
        TransportType::Stdio => "Stdio",
        TransportType::JsonRpcHttp => "JsonRpcHttp",
    };
    Some(TestResult::skipped(
        "Transport: Streamable-HTTP-only",
        TestCategory::Transport,
        format!(
            "Transport: Streamable-HTTP-only tests skipped (transport={label}). \
             Re-run against an HTTP server to validate the GET/OPTIONS/DELETE surface."
        ),
    ))
}

/// Build the shared `reqwest::Client` honouring the tester's TLS posture and
/// timeout budget. The probe-level `tokio::time::timeout` shield is a separate
/// hard upper bound — see `raw_probe_with_headers`.
pub(crate) fn build_probe_client(tester: &ServerTester) -> Result<Client, String> {
    let overall_timeout = std::cmp::min(tester.timeout(), PROBE_RECEIVE_TIMEOUT);
    let mut builder = reqwest::ClientBuilder::new().timeout(overall_timeout);
    if tester.insecure() {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder
        .build()
        .map_err(|e| format!("client build error: {e}"))
}

/// Pure classifier for `GET /mcp` responses. Total over (u16, &str, &str).
///
/// Truth table (status × content-type → status):
///
/// | status     | content-type        | body             | classification |
/// |------------|---------------------|------------------|----------------|
/// | 405        | `application/json*` | JSON-RPC -32601  | Passed         |
/// | 200        | `text/event-stream*`| any              | Passed         |
/// | 401 / 403  | any                 | any              | Warning (auth) |
/// | 200        | `application/json*` | non-JSON-RPC     | Failed         |
/// | other      | any                 | any              | Failed         |
pub fn classify_get_mcp(status: u16, content_type: &str, body_prefix: &str) -> TestStatus {
    let ct = content_type.to_ascii_lowercase();
    if is_auth_status(status) {
        return TestStatus::Warning;
    }
    if status == StatusCode::METHOD_NOT_ALLOWED.as_u16()
        && ct.starts_with(CT_JSON)
        && looks_like_jsonrpc_error(body_prefix)
    {
        return TestStatus::Passed;
    }
    if status == StatusCode::OK.as_u16() && ct.starts_with(CT_SSE) {
        return TestStatus::Passed;
    }
    TestStatus::Failed
}

fn is_auth_status(status: u16) -> bool {
    status == StatusCode::UNAUTHORIZED.as_u16() || status == StatusCode::FORBIDDEN.as_u16()
}

/// Detect a JSON-RPC error envelope in a (potentially-truncated) body prefix.
///
/// Parse failure is treated as "not JSON-RPC error" — never panics.
fn looks_like_jsonrpc_error(body_prefix: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body_prefix) else {
        return false;
    };
    value
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(serde_json::Value::as_i64)
        .is_some()
}

/// `Transport: GET /mcp returns SSE stream OR 405`
async fn test_get_mcp_returns_sse_or_405(tester: &ServerTester, client: &Client) -> TestResult {
    let start = Instant::now();
    let name = "Transport: GET /mcp returns SSE stream OR 405";
    let extra = [("Accept", CT_SSE)];

    match raw_probe_with_headers(tester, client, "GET", &extra, PROBE_RECEIVE_TIMEOUT).await {
        Ok((status, ct, body_prefix, _has_cors)) => {
            match classify_get_mcp(status, &ct, &body_prefix) {
                TestStatus::Passed => TestResult::passed(
                    name,
                    TestCategory::Transport,
                    start.elapsed(),
                    format!("status={status} content-type={ct}"),
                ),
                TestStatus::Warning => auth_warning(name, start, status),
                TestStatus::Failed | TestStatus::Skipped => TestResult::failed(
                    name,
                    TestCategory::Transport,
                    start.elapsed(),
                    format_unexpected(status, &ct, &body_prefix),
                ),
            }
        },
        Err(e) => TestResult::failed(name, TestCategory::Transport, start.elapsed(), e),
    }
}

/// `Transport: OPTIONS /mcp returns CORS or 405`
async fn test_options_mcp_returns_cors_or_405(
    tester: &ServerTester,
    client: &Client,
) -> TestResult {
    let start = Instant::now();
    let name = "Transport: OPTIONS /mcp returns CORS or 405";
    let extra = [
        ("Origin", "https://example.invalid"),
        ("Access-Control-Request-Method", "POST"),
    ];

    match raw_probe_with_headers(tester, client, "OPTIONS", &extra, PROBE_RECEIVE_TIMEOUT).await {
        Ok((status, ct, body_prefix, has_cors)) => {
            let s = StatusCode::from_u16(status).ok();
            let is_2xx = s.is_some_and(|c| c.is_success());
            let is_405 = status == StatusCode::METHOD_NOT_ALLOWED.as_u16();
            if is_405 || (is_2xx && has_cors) {
                TestResult::passed(
                    name,
                    TestCategory::Transport,
                    start.elapsed(),
                    format!("status={status} cors_headers={has_cors}"),
                )
            } else if is_auth_status(status) {
                auth_warning(name, start, status)
            } else {
                TestResult::failed(
                    name,
                    TestCategory::Transport,
                    start.elapsed(),
                    format_unexpected(status, &ct, &body_prefix),
                )
            }
        },
        Err(e) => TestResult::failed(name, TestCategory::Transport, start.elapsed(), e),
    }
}

/// `Transport: DELETE /mcp returns 200/204/405 OR JSON-RPC rejection`
///
/// Spec-compliant DELETE responses, given the probe sends no `Mcp-Session-Id`:
/// - `200` / `204` — session terminated (lenient stateful server).
/// - `405` — stateless server: DELETE not supported.
/// - any `4xx` with a JSON-RPC error envelope — stateful server correctly
///   rejecting a session-less DELETE.
///
/// Anything else is surfaced as a warning, not a failure, since session-
/// termination semantics vary across implementations.
async fn test_delete_mcp_returns_session_termination_or_405(
    tester: &ServerTester,
    client: &Client,
) -> TestResult {
    let start = Instant::now();
    let name = "Transport: DELETE /mcp returns 200/204/405 OR JSON-RPC rejection";
    let extra: [(&str, &str); 0] = [];

    match raw_probe_with_headers(tester, client, "DELETE", &extra, PROBE_RECEIVE_TIMEOUT).await {
        Ok((status, ct, body_prefix, _has_cors)) => match classify_delete_mcp(status, &body_prefix)
        {
            TestStatus::Passed => TestResult::passed(
                name,
                TestCategory::Transport,
                start.elapsed(),
                format!("status={status}"),
            ),
            TestStatus::Warning if is_auth_status(status) => auth_warning(name, start, status),
            TestStatus::Warning | TestStatus::Failed | TestStatus::Skipped => TestResult::warning(
                name,
                TestCategory::Transport,
                start.elapsed(),
                format_unexpected(status, &ct, &body_prefix),
            ),
        },
        Err(e) => TestResult::warning(name, TestCategory::Transport, start.elapsed(), e),
    }
}

/// Pure classifier for `DELETE /mcp` responses. Returns `Passed` for any
/// spec-compliant shape; `Warning` for everything else (auth or unexpected).
pub fn classify_delete_mcp(status: u16, body_prefix: &str) -> TestStatus {
    if status == StatusCode::OK.as_u16()
        || status == StatusCode::NO_CONTENT.as_u16()
        || status == StatusCode::METHOD_NOT_ALLOWED.as_u16()
    {
        return TestStatus::Passed;
    }
    if is_4xx(status) && looks_like_jsonrpc_error(body_prefix) {
        return TestStatus::Passed;
    }
    TestStatus::Warning
}

fn is_4xx(status: u16) -> bool {
    (400..500).contains(&status)
}

fn auth_warning(name: &'static str, start: Instant, status: u16) -> TestResult {
    TestResult::warning(
        name,
        TestCategory::Transport,
        start.elapsed(),
        format!("auth required (status={status}); run `cargo pmcp auth login` to authenticate"),
    )
}

/// Build the operator-facing failure-detail string. Truncates the body to
/// `MAX_BODY_PREFIX_IN_DETAIL` characters before embedding.
fn format_unexpected(status: u16, content_type: &str, body_prefix: &str) -> String {
    let truncated = truncate_chars(body_prefix, MAX_BODY_PREFIX_IN_DETAIL);
    format!(
        "unexpected response: status={status} content-type={content_type} body_prefix={truncated}"
    )
}

/// Truncate a string to at most `max_chars` characters, on a char boundary.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Raw HTTP probe shared by all scenarios. Builds an `HttpRequest`, applies
/// the existing `HttpMiddlewareChain` (auth reuse) when present, sends via
/// the shared `reqwest::Client`, and returns
/// `(status, content_type, body_prefix, has_cors)` — body streamed and
/// hard-capped at `MAX_BODY_BYTES` so SSE servers can't pin the connection.
async fn raw_probe_with_headers(
    tester: &ServerTester,
    client: &Client,
    method: &str,
    extra_headers: &[(&str, &str)],
    receive_timeout: Duration,
) -> Result<(u16, String, String, bool), String> {
    let url = tester.url();

    // The middleware chain MUST be borrowed from the tester (auth-reuse
    // contract) — never construct a new auth provider here.
    let mut http_req = HttpRequest::new(method.to_string(), url.to_string(), Vec::new());
    for (name, value) in extra_headers {
        http_req.add_header(name, value);
    }
    if let Some(chain) = tester.http_middleware_chain() {
        let context = HttpMiddlewareContext::new(url.to_string(), method.to_string());
        chain
            .process_request(&mut http_req, &context)
            .await
            .map_err(|e| format!("middleware error: {e}"))?;
    }

    let method_obj = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|e| format!("invalid method `{method}`: {e}"))?;
    let mut req = client.request(method_obj, url);
    for (k, v) in http_req.headers.iter() {
        if let Ok(value_str) = v.to_str() {
            req = req.header(k.as_str(), value_str);
        }
    }
    if !http_req.body.is_empty() {
        req = req.body(http_req.body);
    }

    let send_fut = req.send();
    let mut response = tokio::time::timeout(receive_timeout, send_fut)
        .await
        .map_err(|_| format!("transport error: probe timed out after {receive_timeout:?}"))?
        .map_err(|e| format!("transport error: {e}"))?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let has_cors = response.headers().keys().any(|name| {
        name.as_str()
            .to_ascii_lowercase()
            .starts_with("access-control-")
    });

    let body_prefix = read_capped_body(&mut response).await?;
    drop(response);

    Ok((status, content_type, body_prefix, has_cors))
}

/// Stream up to `MAX_BODY_BYTES` from the response body, then stop. Caps memory
/// and ensures SSE responses don't pin the connection for the full timeout.
async fn read_capped_body(response: &mut reqwest::Response) -> Result<String, String> {
    let mut body = Vec::with_capacity(MAX_BODY_BYTES);
    while body.len() < MAX_BODY_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let take = std::cmp::min(chunk.len(), MAX_BODY_BYTES - body.len());
                body.extend_from_slice(&chunk[..take]);
            },
            Ok(None) => break,
            Err(e) => return Err(format!("transport error: {e}")),
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Truth-table tests for `classify_get_mcp`.

    #[test]
    fn classify_get_mcp_405_jsonrpc_passes() {
        let body = r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"SSE not supported in stateless mode"},"id":null}"#;
        assert_eq!(
            classify_get_mcp(405, "application/json", body),
            TestStatus::Passed
        );
        // charset suffix on content-type still passes
        assert_eq!(
            classify_get_mcp(405, "application/json; charset=utf-8", body),
            TestStatus::Passed
        );
    }

    #[test]
    fn classify_get_mcp_200_sse_passes() {
        assert_eq!(
            classify_get_mcp(200, "text/event-stream", ""),
            TestStatus::Passed
        );
        assert_eq!(
            classify_get_mcp(200, "text/event-stream; charset=utf-8", ""),
            TestStatus::Passed
        );
    }

    #[test]
    fn classify_get_mcp_200_json_non_sse_fails() {
        // Regression case: 200 + JSON health body where SSE was expected.
        let body = r#"{"ok":true,"service":"example","version":"1.2.3"}"#;
        assert_eq!(
            classify_get_mcp(200, "application/json", body),
            TestStatus::Failed
        );
        assert_eq!(
            classify_get_mcp(200, "application/json; charset=utf-8", body),
            TestStatus::Failed
        );
    }

    #[test]
    fn classify_get_mcp_401_warns() {
        assert_eq!(
            classify_get_mcp(401, "application/json", ""),
            TestStatus::Warning
        );
        assert_eq!(
            classify_get_mcp(403, "text/html", "<html>forbidden</html>"),
            TestStatus::Warning
        );
    }

    #[test]
    fn classify_get_mcp_other_fails() {
        assert_eq!(
            classify_get_mcp(500, "text/html", "Server Error"),
            TestStatus::Failed
        );
        assert_eq!(classify_get_mcp(502, "text/plain", ""), TestStatus::Failed);
        assert_eq!(
            classify_get_mcp(404, "application/json", "{}"),
            TestStatus::Failed
        );
        // 405 + non-JSON-RPC body must fail.
        assert_eq!(
            classify_get_mcp(405, "application/json", "not even json"),
            TestStatus::Failed
        );
        // 405 + JSON-RPC error body but wrong content-type must fail.
        assert_eq!(
            classify_get_mcp(
                405,
                "text/plain",
                r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"x"},"id":null}"#
            ),
            TestStatus::Failed
        );
    }

    /// Total-classifier exhaustive sweep: classifier never panics across a
    /// representative cross-product of (status, content-type, body) inputs
    /// and always returns one of {Passed, Failed, Warning, Skipped}.
    #[test]
    fn classify_get_mcp_total_no_panic_exhaustive() {
        let statuses: &[u16] = &[
            0, 1, 100, 101, 200, 201, 204, 301, 302, 400, 401, 403, 404, 405, 408, 418, 429, 500,
            502, 503, 504, 599, 999,
        ];
        let content_types: &[&str] = &[
            "",
            "application/json",
            "application/json; charset=utf-8",
            "text/event-stream",
            "text/event-stream; charset=utf-8",
            "text/html",
            "text/plain",
            "application/octet-stream",
            "APPLICATION/JSON",
            "TEXT/EVENT-STREAM",
        ];
        let bodies: &[&str] = &[
            "",
            "{}",
            "not even json",
            r#"{"ok":true,"service":"example","version":"1.2.3"}"#,
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"x"},"id":null}"#,
            r#"{"jsonrpc":"2.0","error":{"code":"not-a-number"},"id":null}"#,
            "\u{0000}\u{FFFF}🦀{",
            "<html>oops",
        ];
        for &s in statuses {
            for &ct in content_types {
                for &body in bodies {
                    let result = classify_get_mcp(s, ct, body);
                    match result {
                        TestStatus::Passed
                        | TestStatus::Failed
                        | TestStatus::Warning
                        | TestStatus::Skipped => {},
                    }
                }
            }
        }
    }

    /// Truncated JSON-RPC error envelopes must not panic the classifier.
    #[test]
    fn classify_get_mcp_truncated_jsonrpc_fails_safely() {
        let body = r#"{"jsonrpc":"2.0","error":{"code":-326"#;
        assert_eq!(
            classify_get_mcp(405, "application/json", body),
            TestStatus::Failed
        );
    }

    // Truth-table tests for `classify_delete_mcp`.

    #[test]
    fn classify_delete_mcp_200_204_405_pass() {
        assert_eq!(classify_delete_mcp(200, ""), TestStatus::Passed);
        assert_eq!(classify_delete_mcp(204, ""), TestStatus::Passed);
        assert_eq!(classify_delete_mcp(405, ""), TestStatus::Passed);
    }

    #[test]
    fn classify_delete_mcp_4xx_with_jsonrpc_error_passes() {
        // A stateful server rejecting a session-less DELETE with a JSON-RPC
        // error envelope is spec-compliant — this is the cost-coach case.
        let body = r#"{"jsonrpc":"2.0","error":{"code":-32600,"message":"No session ID provided"},"id":null}"#;
        assert_eq!(classify_delete_mcp(404, body), TestStatus::Passed);
        assert_eq!(classify_delete_mcp(400, body), TestStatus::Passed);
        assert_eq!(classify_delete_mcp(409, body), TestStatus::Passed);
    }

    #[test]
    fn classify_delete_mcp_4xx_without_jsonrpc_warns() {
        assert_eq!(classify_delete_mcp(404, ""), TestStatus::Warning);
        assert_eq!(
            classify_delete_mcp(404, r#"{"message":"Not Found"}"#),
            TestStatus::Warning
        );
        assert_eq!(
            classify_delete_mcp(400, "<html>Bad</html>"),
            TestStatus::Warning
        );
    }

    #[test]
    fn classify_delete_mcp_other_warns() {
        assert_eq!(classify_delete_mcp(500, ""), TestStatus::Warning);
        assert_eq!(classify_delete_mcp(502, ""), TestStatus::Warning);
        assert_eq!(classify_delete_mcp(301, ""), TestStatus::Warning);
        // 5xx with JSON-RPC body is still warning — server-side failure.
        let body = r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"x"},"id":null}"#;
        assert_eq!(classify_delete_mcp(500, body), TestStatus::Warning);
    }

    /// Failure-detail strings only embed response-side data; the function
    /// signature has no access to request headers.
    #[test]
    fn format_unexpected_does_not_leak_request_secrets() {
        let body = "BEARER-TOKEN-LOOKING-VALUE-IN-RESPONSE-BODY";
        let detail = format_unexpected(500, "application/json", body);
        assert!(detail.contains("status=500"));
        assert!(detail.contains("content-type=application/json"));
        assert!(!detail.contains("Authorization"));
        assert!(!detail.contains("Bearer "));
    }

    /// Body truncation cap holds at `MAX_BODY_PREFIX_IN_DETAIL` characters.
    #[test]
    fn format_unexpected_truncates_body_prefix() {
        let big = "x".repeat(10_000);
        let detail = format_unexpected(200, "application/json", &big);
        let body_prefix_part = detail
            .split("body_prefix=")
            .nth(1)
            .expect("body_prefix= present");
        assert!(body_prefix_part.chars().count() <= MAX_BODY_PREFIX_IN_DETAIL);
    }

    #[test]
    fn truncate_chars_handles_multibyte_safely() {
        // Crab emoji is multi-byte; truncation must operate on char boundaries.
        let s = "🦀🦀🦀🦀🦀";
        let out = truncate_chars(s, 2);
        assert_eq!(out.chars().count(), 2);
        assert_eq!(out, "🦀🦀");
    }

    /// Skipped-shim emits a single result with category Transport when the
    /// transport is Stdio or JsonRpcHttp; emits None for HTTP.
    #[test]
    fn transport_skip_for_non_http_classifies_correctly() {
        assert!(transport_skip_for_non_http(&TransportType::Http).is_none());

        let stdio = transport_skip_for_non_http(&TransportType::Stdio).expect("skipped result");
        assert_eq!(stdio.category, TestCategory::Transport);
        assert_eq!(stdio.status, TestStatus::Skipped);
        assert!(stdio
            .details
            .as_deref()
            .unwrap_or("")
            .contains("transport=Stdio"));

        let jsonrpc =
            transport_skip_for_non_http(&TransportType::JsonRpcHttp).expect("skipped result");
        assert_eq!(jsonrpc.category, TestCategory::Transport);
        assert_eq!(jsonrpc.status, TestStatus::Skipped);
        assert!(jsonrpc
            .details
            .as_deref()
            .unwrap_or("")
            .contains("transport=JsonRpcHttp"));
    }
}
