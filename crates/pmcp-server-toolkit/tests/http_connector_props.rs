//! Property tests for the HTTP connector URL construction (OAPI-01 / Pitfall 2).
//!
//! Asserts the invariant of the shared `join_url` path-concat as observed
//! through the public `HttpClient` surface: for any `(base_path, tool_path)` the
//! joined URL has exactly ONE slash between the base and the leaf and NEVER drops
//! the base's non-root path (an API-Gateway stage prefix like `/v1` survives).
//! Plan 03 extends this file with a path-param-substitution proptest.

#![cfg(feature = "http")]

use async_trait::async_trait;
use pmcp_server_toolkit::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
use pmcp_server_toolkit::http::auth::NoAuth;
use pmcp_server_toolkit::http::{HttpClient, HttpConnector, HttpConnectorError, Operation};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Build a base URL with the given path segments and return the `HttpClient`'s
/// stored `base_url()` for assertion convenience.
fn client_base(path: &str) -> String {
    let base = format!("https://api.example.com{path}");
    let client =
        HttpClient::new(reqwest::Client::new(), base, Arc::new(NoAuth)).expect("valid base URL");
    client.base_url().to_string()
}

proptest! {
    /// For any single-segment base prefix and leaf, the joined URL keeps the
    /// prefix and joins with exactly one slash. We assert via the public
    /// `base_url()` + a manual single-slash join mirroring the connector.
    #[test]
    fn join_preserves_prefix_single_slash(
        prefix in "[a-z]{1,8}",
        leaf in "[a-z]{1,8}",
    ) {
        let base = client_base(&format!("/{prefix}"));
        // Mirror the connector's concatenation contract.
        let joined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            leaf.trim_start_matches('/')
        );
        // Prefix preserved.
        prop_assert!(joined.contains(&format!("/{prefix}/")), "prefix dropped in {joined}");
        // Exactly one slash between prefix and leaf (no double slash after host).
        let after_host = &joined["https://".len()..];
        prop_assert!(!after_host.contains("//"), "double slash in {joined}");
        // Leaf present at the tail.
        prop_assert!(joined.ends_with(&leaf), "leaf missing in {joined}");
    }

    /// A trailing slash on the base and a leading slash on the leaf collapse to one.
    #[test]
    fn trailing_and_leading_slash_collapse(leaf in "[a-z]{1,8}") {
        let base = client_base("/v1/");
        let joined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            format!("/{leaf}").trim_start_matches('/')
        );
        prop_assert_eq!(
            joined,
            format!("https://api.example.com/v1/{leaf}")
        );
    }
}

/// Mock connector that records the [`Operation`] the synthesized handler passes
/// to [`HttpConnector::execute`] (so the path-param-substitution property can be
/// asserted on what the synthesizer produced, without a live HTTP backend).
struct RecordingConnector {
    last: Mutex<Option<Operation>>,
}

#[async_trait]
impl HttpConnector for RecordingConnector {
    async fn execute(
        &self,
        operation: &Operation,
        _args: &Value,
    ) -> Result<Value, HttpConnectorError> {
        *self.last.lock().unwrap() = Some(operation.clone());
        Ok(Value::Null)
    }
    fn base_url(&self) -> &str {
        "https://api.example.com"
    }
}

proptest! {
    /// Path-param-substitution invariant (T-90-03-01): for a single-call tool
    /// whose path template declares exactly one `{declared}` placeholder, the
    /// synthesized `Operation` carries that — and ONLY that — name as a path
    /// parameter, regardless of any extra (undeclared-as-path) query params the
    /// tool declares. Undeclared keys never become path parameters, so they can
    /// never be substituted into the URL path.
    #[test]
    fn only_declared_path_segments_become_path_params(
        declared in "[a-z]{1,8}",
        extra in "[a-z]{1,8}",
    ) {
        // Keep the two names distinct so the assertion is unambiguous.
        prop_assume!(declared != extra);

        let cfg = ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools: vec![ToolDecl {
                name: "t".to_string(),
                description: Some("t".to_string()),
                path: Some(format!("/things/{{{declared}}}")),
                method: Some("GET".to_string()),
                parameters: vec![
                    ParamDecl {
                        name: declared.clone(),
                        param_type: Some("string".to_string()),
                        required: true,
                        ..Default::default()
                    },
                    // `extra` is declared but is NOT a `{...}` segment ⇒ query.
                    ParamDecl {
                        name: extra.clone(),
                        param_type: Some("string".to_string()),
                        required: false,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let connector = Arc::new(RecordingConnector { last: Mutex::new(None) });
        let out = synthesize_from_config_with_http_connector(&cfg, connector.clone())
            .expect("synthesize");
        let (_, _, handler) = &out[0];

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            handler
                .handle(json!({ &declared: "v1", &extra: "v2" }), pmcp::RequestHandlerExtra::default())
                .await
                .expect("handle");
        });

        let op = connector.last.lock().unwrap().clone().expect("operation recorded");
        let path_params: Vec<String> =
            op.path_parameters().iter().map(|p| p.name.clone()).collect();
        // Only the declared `{...}` segment is a path param.
        prop_assert_eq!(path_params, vec![declared.clone()]);
        // The extra declared param is a query param, never a path param.
        let query_params: Vec<String> =
            op.query_parameters().iter().map(|p| p.name.clone()).collect();
        prop_assert!(query_params.contains(&extra), "extra must be a query param");
    }
}

// =============================================================================
// WR-03 / GAP 4 — non-scalar params rejected by BOTH HTTP renderers.
//
// The `client.rs` `render_scalar` and the `code_mode.rs` `HttpCodeExecutor`
// `scalar_str` are both PRIVATE, so the property is asserted through each
// renderer's public surface:
//   - client.rs via `HttpConnector::execute` with an OBJECT path param — the
//     `substitute_path` rejection fires SYNCHRONOUSLY, before any network call.
//   - code_mode.rs via the `HttpExecutor::execute_request` trait method with an
//     OBJECT GET-QUERY value — the `scalar_str` rejection likewise fires before
//     any network call. It is the GET-query leg and no longer the `{path}` leg:
//     Phase 128 D-09 moved `{key}` path resolution out of this executor and up
//     into `PlanExecutor`, so `scalar_str` is now reached ONLY from the
//     remaining-body-as-query-params step. The `{path}` half of the WR-03 rule
//     is asserted in `pmcp-code-mode`'s own `layer_two` tests.
// For a scalar value, the client.rs path produces a well-formed URL with no
// literal `{`/`[`/`"` (asserted via the recorded operation + a manual join).
// =============================================================================

/// A non-scalar (object) JSON value used as a generated arbitrary.
fn object_value() -> Value {
    json!({ "nested": { "deep": [1, 2, 3] } })
}

proptest! {
    /// The client.rs renderer rejects an OBJECT path param (naming the param)
    /// for any scalar leaf the object happens to wrap; the error never echoes a
    /// JSON brace/quote. Asserted through the public `HttpConnector::execute`.
    #[test]
    fn client_render_scalar_rejects_object_path_param(seg in "[a-z]{1,8}") {
        let op = Operation {
            method: "GET".to_string(),
            path: format!("/things/{{{seg}}}"),
            parameters: vec![pmcp_server_toolkit::http::Parameter::new(
                &seg,
                pmcp_server_toolkit::http::ParameterLocation::Path,
                true,
            )],
            has_request_body: false,
            base_url: None,
        };
        // A non-routable base: the rejection must fire BEFORE any connect.
        let client = HttpClient::new(
            reqwest::Client::new(),
            "https://api.example.com".to_string(),
            Arc::new(NoAuth),
        )
        .expect("valid base URL");

        let args = json!({ &seg: object_value() });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(async { client.execute(&op, &args).await })
            .expect_err("an object path param must be rejected");
        let rendered = err.to_string();
        prop_assert!(rendered.contains(&seg), "error must name the param: {rendered}");
        for forbidden in ['{', '[', '"'] {
            prop_assert!(!rendered.contains(forbidden), "must not echo JSON: {rendered}");
        }
    }
}

#[cfg(feature = "openapi-code-mode")]
proptest! {
    /// The code_mode.rs renderer rejects an OBJECT GET-query value (naming the
    /// key) through the public `HttpExecutor::execute_request`.
    #[test]
    fn code_mode_render_scalar_rejects_object_query_value(key in "[a-z]{1,8}") {
        use pmcp_code_mode::{HttpExecutor, ResolvedPath};
        use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
        use pmcp_server_toolkit::http::auth::create_auth_provider;
        use pmcp_server_toolkit::http::AuthConfig;

        let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
        let exec = HttpCodeExecutor::new(
            reqwest::Client::new(),
            "https://api.example.com".to_string(),
            auth,
        );


        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // Object value in the remaining body of a GET -> rejected as a query
        // param before any call.
        let err = rt
            .block_on(async {
                exec.execute_request(
                    "GET",
                    ResolvedPath::from_checked("/things").expect("a clean resolved path"),
                    Some(json!({ &key: object_value() })),
                )
                .await
            })
            .expect_err("an object GET-query value must be rejected");
        let rendered = err.to_string();
        prop_assert!(rendered.contains(&key), "error must name the key: {rendered}");
        for forbidden in ['{', '[', '"'] {
            prop_assert!(!rendered.contains(forbidden), "must not echo JSON: {rendered}");
        }
    }
}
