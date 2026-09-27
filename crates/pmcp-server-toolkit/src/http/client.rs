//! reqwest-backed [`HttpConnector`] implementation (OAPI-01).
//!
//! Lifts the pmcp-run reference `HttpClient::execute_with_options` body into a
//! toolkit-owned [`HttpClient`] that implements [`HttpConnector`]. The concrete
//! shape mirrors `crate::sql::sqlite::SqliteConnector` (a concrete connector impl
//! + constructor). Construction is LAZY — `new` parses the base URL but contacts
//! no backend (CF-2). URL building uses the shared [`crate::http::join_url`]
//! helper so an API-Gateway stage prefix (`/v1`) survives (Pitfall 2 — explicit
//! path concatenation, never the RFC-3986 url-crate path merge). Error messages
//! never echo the URL or a credential (Pitfall 5).

use super::auth::HttpAuthProvider;
use super::{join_url, HttpConnector, HttpConnectorError, Operation};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// HTTP client configuration (OWNED here in `http`, mirroring [`super::AuthConfig`]
/// ownership so Plan 02 re-exports it rather than redefining).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Number of retry attempts on 5xx / connect / timeout.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Base backoff in milliseconds (exponential per attempt).
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff_ms: u64,
    /// `User-Agent` header for all requests.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Extra headers applied to every request.
    #[serde(default)]
    pub default_headers: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30
}
fn default_retries() -> u32 {
    3
}
fn default_retry_backoff() -> u64 {
    1000
}
fn default_user_agent() -> String {
    format!("pmcp-server-toolkit/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            retries: default_retries(),
            retry_backoff_ms: default_retry_backoff(),
            user_agent: default_user_agent(),
            default_headers: HashMap::new(),
        }
    }
}

/// reqwest-backed [`HttpConnector`].
pub struct HttpClient {
    client: reqwest::Client,
    base_url: url::Url,
    auth: Arc<dyn HttpAuthProvider>,
    http_config: HttpConfig,
}

impl HttpClient {
    /// Construct a client. LAZY: parses `base_url` but contacts no backend (CF-2).
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when `base_url` is unparseable or
    /// the reqwest client cannot be built. The error message does NOT echo the URL.
    pub fn new(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
    ) -> Result<Self, HttpConnectorError> {
        Self::with_config(client, base_url, auth, HttpConfig::default())
    }

    /// Construct a client with an explicit [`HttpConfig`]. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn with_config(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let base_url = url::Url::parse(&base_url)
            .map_err(|_| HttpConnectorError::Backend("invalid base URL".to_string()))?;
        Ok(Self {
            client,
            base_url,
            auth,
            http_config,
        })
    }

    /// Build a client from an [`HttpConfig`], constructing the reqwest client with
    /// the configured timeout, user-agent, and default headers. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn from_config(
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        if let Ok(ua) = HeaderValue::from_str(&http_config.user_agent) {
            headers.insert(reqwest::header::USER_AGENT, ua);
        }
        for (key, value) in &http_config.default_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::try_from(key.as_str()),
                HeaderValue::try_from(value.as_str()),
            ) {
                headers.insert(name, val);
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http_config.timeout_seconds))
            .default_headers(headers)
            .build()
            .map_err(|_| HttpConnectorError::Backend("failed to build HTTP client".to_string()))?;
        Self::with_config(client, base_url, auth, http_config)
    }

    /// Substitute path parameters into the operation path template.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] (via [`render_scalar`]) when a path
    /// parameter value is a non-scalar (`Object`/`Array`) — such a value would
    /// otherwise be JSON-stringified into the URL (WR-03).
    fn substitute_path(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<String, HttpConnectorError> {
        let mut path = operation.path.clone();
        for param in operation.path_parameters() {
            let placeholder = format!("{{{}}}", param.name);
            if let Some(value) = args.get(&param.name) {
                let value_str = render_scalar(&param.name, value)?;
                path = path.replace(&placeholder, &value_str);
            }
        }
        Ok(path)
    }

    /// Render one query value: a scalar passes through; an array-of-scalars is
    /// comma-joined (OpenAPI `form`/`explode:false` style); an object or an array
    /// with any non-scalar member is rejected (each member is checked through
    /// [`render_scalar`]).
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] naming `param_name` when `value`
    /// (or any array member) is a non-scalar.
    fn render_query_value(
        param_name: &str,
        value: &serde_json::Value,
    ) -> Result<String, HttpConnectorError> {
        if let serde_json::Value::Array(arr) = value {
            // Comma-separate array members (OpenAPI `form`/`simple` style). A
            // nested non-scalar member is rejected by render_scalar.
            let mut csv = String::new();
            for (i, member) in arr.iter().enumerate() {
                if i > 0 {
                    csv.push(',');
                }
                csv.push_str(&render_scalar(param_name, member)?);
            }
            Ok(csv)
        } else {
            render_scalar(param_name, value)
        }
    }

    /// Build the query map from query-located params present in `args`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] naming the offending parameter when
    /// a query value is an object, or an array containing a non-scalar member
    /// (WR-03). A scalar or an array-of-scalars behaves exactly as before.
    fn build_query(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HashMap<String, String>, HttpConnectorError> {
        let mut query = HashMap::new();
        for param in operation.query_parameters() {
            if let Some(value) = args.get(&param.name) {
                query.insert(
                    param.name.clone(),
                    Self::render_query_value(&param.name, value)?,
                );
            }
        }
        Ok(query)
    }

    /// Build the header map from header-located params present in `args`.
    fn build_headers(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HeaderMap, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        for param in operation.header_parameters() {
            if let Some(value) = args.get(&param.name) {
                let name = HeaderName::try_from(param.name.as_str()).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header name".to_string())
                })?;
                // Reject a non-scalar header value (naming the param) before it
                // can be JSON-stringified into the header (WR-03).
                let rendered = render_scalar(&param.name, value)?;
                let val = HeaderValue::try_from(rendered).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header value".to_string())
                })?;
                headers.insert(name, val);
            }
        }
        Ok(headers)
    }

    /// Collect the request body: args that are NOT path/query/header params.
    fn build_body(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<serde_json::Value> {
        if !operation.has_request_body {
            return None;
        }
        if let Some(body) = args.get("body") {
            return Some(body.clone());
        }
        let declared: std::collections::HashSet<&str> = operation
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        let body: serde_json::Map<String, serde_json::Value> = args
            .iter()
            .filter(|(k, _)| !declared.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if body.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(body))
        }
    }

    fn convert_method(method: &str) -> Result<reqwest::Method, HttpConnectorError> {
        match method.to_uppercase().as_str() {
            "GET" => Ok(reqwest::Method::GET),
            "POST" => Ok(reqwest::Method::POST),
            "PUT" => Ok(reqwest::Method::PUT),
            "PATCH" => Ok(reqwest::Method::PATCH),
            "DELETE" => Ok(reqwest::Method::DELETE),
            "HEAD" => Ok(reqwest::Method::HEAD),
            "OPTIONS" => Ok(reqwest::Method::OPTIONS),
            _ => Err(HttpConnectorError::Backend(
                "unknown HTTP method".to_string(),
            )),
        }
    }

    /// Send the request, retrying on 5xx / connect / timeout with exponential backoff.
    async fn send_with_retries(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, HttpConnectorError> {
        let max_retries = self.http_config.retries;
        let mut last_status: Option<u16> = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = self.http_config.retry_backoff_ms * (1u64 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let Some(attempt_request) = request.try_clone() else {
                return Err(HttpConnectorError::Request(
                    "request body is not retryable".to_string(),
                ));
            };
            match attempt_request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() && attempt < max_retries {
                        last_status = Some(status.as_u16());
                        continue;
                    }
                    return Ok(response);
                },
                Err(e) => {
                    let retryable = e.is_connect() || e.is_timeout();
                    if retryable && attempt < max_retries {
                        continue;
                    }
                    // Redacted: never forward the reqwest error Display (echoes URL).
                    return Err(HttpConnectorError::Request(
                        "transport error contacting backend".to_string(),
                    ));
                },
            }
        }
        Err(HttpConnectorError::Status {
            status: last_status.unwrap_or(0),
        })
    }
}

/// Render a JSON scalar for use in a path / query / header position, REJECTING
/// non-scalar values (WR-03 / GAP 4).
///
/// # The decided rule (uniform)
///
/// The `http::schema::Parameter` model carries NO OpenAPI `style` / `explode` /
/// `type` hint, so there is no per-parameter serialization directive to honor;
/// the rule must therefore be uniform across every path / query / header
/// position:
///
/// - A scalar (`String`, `Number`, `Bool`, `Null`) renders to a bare string
///   (`Null` → `"null"`, matching the `code_mode::HttpCodeExecutor::scalar_str`
///   counterpart so the two HTTP surfaces stay consistent).
/// - A query parameter that is an **array of scalars** is comma-joined by the
///   caller ([`build_query`]); each member is rendered through this function so a
///   nested non-scalar member is rejected.
/// - An `Object`, an array containing any non-scalar member, or ANY non-scalar
///   in path / header position is **rejected** with a typed error that names the
///   parameter — it is NEVER JSON-stringified into the URL/header (which would
///   leak literal `{`/`[`/`"` that then percent-encode into a silently-wrong
///   request).
///
/// # Errors
///
/// Returns [`HttpConnectorError::Backend`] naming `param_name` when `value` is a
/// non-scalar (`Object` or `Array`). Per the module's redaction discipline
/// (Pitfall 5) the message names the PARAMETER ONLY — never the value.
fn render_scalar(
    param_name: &str,
    value: &serde_json::Value,
) -> Result<String, HttpConnectorError> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok("null".to_string()),
        // Object OR Array: non-scalar in a path/query/header position is rejected
        // rather than silently JSON-stringified. Name the param ONLY (Pitfall 5).
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            Err(HttpConnectorError::Backend(format!(
                "param '{param_name}' must be a scalar (non-scalar values are \
                 not supported in path/query/header position)"
            )))
        },
    }
}

#[async_trait]
impl HttpConnector for HttpClient {
    async fn execute(
        &self,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        let empty = serde_json::Map::new();
        let args_map = args.as_object().unwrap_or(&empty);

        // Build URL via the shared join_url helper (explicit concat, never the
        // url-crate RFC-3986 path merge) — preserves a stage prefix like /v1
        // (Pitfall 2 / T-90-01-05).
        let substituted = Self::substitute_path(operation, args_map)?;
        let joined = join_url(self.base_url.as_str(), &substituted);
        let mut url = url::Url::parse(&joined)
            .map_err(|_| HttpConnectorError::Backend("constructed URL is invalid".to_string()))?;

        let mut query = Self::build_query(operation, args_map)?;
        let mut headers = Self::build_headers(operation, args_map)?;

        // Single-call tools have no per-request passthrough token (Plan 04/06 carry
        // it through HttpCodeExecutor); pass None here.
        self.auth.apply(&mut headers, &mut query, None).await?;

        // Why: reqwest 0.13 gates `RequestBuilder::query` behind a `query` feature
        // (verified in reqwest-0.13.2 request.rs:`#[cfg(feature = "query")]`). The
        // toolkit deliberately does NOT enable that feature (Pitfall 4 / lean
        // build), so query params are appended to the URL via `url`'s built-in,
        // percent-encoding query-pair serializer instead.
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &query {
                pairs.append_pair(key, value);
            }
            drop(pairs);
        }

        let method = Self::convert_method(&operation.method)?;
        let mut request = self.client.request(method, url);
        request = request.headers(headers);
        if let Some(body) = Self::build_body(operation, args_map) {
            request = request.json(&body);
        }

        let response = self.send_with_retries(request).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(HttpConnectorError::Status {
                status: status.as_u16(),
            });
        }
        let body = response
            .text()
            .await
            .map_err(|_| HttpConnectorError::Request("failed to read response body".to_string()))?;
        if body.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&body).map_err(|_| {
            HttpConnectorError::Backend("response body was not valid JSON".to_string())
        })
    }

    fn base_url(&self) -> &str {
        self.base_url.as_str()
    }
}

// -----------------------------------------------------------------------------
// Phase 128 D4 test support + the two sibling test modules.
//
// `mod placeholder_floor` and `mod query_separator` are SIBLINGS of `mod tests`
// at the `client` module level, not children of it. Both are still selected by
// this plan's `--lib http::client` verify filter (a module prefix), and the names
// mirror `pmcp-code-mode`'s `executor::query_separator` so the two surfaces'
// boundary suites read alike. Being children of `client` is what gives them
// access to the private `HttpClient::substitute_path`.
// -----------------------------------------------------------------------------

/// Fixtures shared by the two Phase 128 D4 test modules.
#[cfg(all(test, feature = "input-validation"))]
mod d4_support {
    use super::{HttpClient, HttpConnectorError, Operation};
    use crate::http::{Parameter, ParameterLocation};

    /// A `GET` operation on `path` carrying `parameters`.
    pub fn op(path: &str, parameters: Vec<Parameter>) -> Operation {
        Operation {
            method: "GET".to_string(),
            path: path.to_string(),
            parameters,
            has_request_body: false,
            base_url: None,
        }
    }

    /// A required path parameter with no declared narrowing (floor + cap only).
    pub fn path_param(name: &str) -> Parameter {
        Parameter::new(name, ParameterLocation::Path, true)
    }

    /// Substitute `pairs` into `path`, treating every named key as a path
    /// parameter with no declared narrowing.
    pub fn substitute(
        path: &str,
        pairs: &[(&str, serde_json::Value)],
    ) -> Result<String, HttpConnectorError> {
        let parameters = pairs.iter().map(|(k, _)| path_param(k)).collect();
        let mut args = serde_json::Map::new();
        for (k, v) in pairs {
            args.insert((*k).to_string(), v.clone());
        }
        HttpClient::substitute_path(&op(path, parameters), &args)
    }

    /// Substitute a single string `value` for `{name}` in `path`.
    pub fn substitute_one(
        path: &str,
        name: &str,
        value: &str,
    ) -> Result<String, HttpConnectorError> {
        substitute(
            path,
            &[(name, serde_json::Value::String(value.to_string()))],
        )
    }
}

/// The curated surface's D4 floor: every rendered placeholder value faces
/// `validate_path_placeholder` before ANY substitution is applied, and the
/// composed result faces `validate_resolved_path` before dispatch.
#[cfg(all(test, feature = "input-validation"))]
mod placeholder_floor {
    use super::d4_support::{op, path_param, substitute, substitute_one};
    use super::{HttpClient, HttpConnectorError};
    use crate::http::{Parameter, ParameterLocation};
    use pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH;

    /// Assert a refusal names the parameter and carries no byte of the value and
    /// no fragment of the resolved path.
    fn assert_value_free(err: &HttpConnectorError, param: &str, value: &str, path_fragment: &str) {
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
        let rendered = err.to_string();
        assert!(
            rendered.contains(param),
            "the refusal must name the declared parameter: {rendered}"
        );
        assert!(
            !rendered.contains(value),
            "the refusal must carry no byte of the value: {rendered}"
        );
        assert!(
            !rendered.contains(path_fragment),
            "the refusal must never contain the resolved path: {rendered}"
        );
    }

    /// CR-01 row: a query separator inside a placeholder value.
    #[test]
    fn placeholder_floor_refuses_a_query_separator_in_a_value() {
        let value = "current?string=x";
        let err = substitute_one("/content/{version}/CUI", "version", value).unwrap_err();
        assert_value_free(&err, "version", value, "/content/");
    }

    /// CR-01 row: parent-directory traversal inside a placeholder value.
    #[test]
    fn placeholder_floor_refuses_traversal_in_a_value() {
        let value = "current/../../search/current";
        let err = substitute_one("/content/{version}/CUI", "version", value).unwrap_err();
        assert_value_free(&err, "version", value, "/content/");
    }

    /// Percent-encoded traversal in UPPER-case hex — the decode-once pass is what
    /// has to catch it, not an enumerated denylist of spellings.
    #[test]
    fn placeholder_floor_refuses_upper_case_encoded_traversal() {
        let err = substitute_one("/content/{version}/CUI", "version", "a%2E%2Eb").unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// Adjacency edge: a value that is EXACTLY a denied character. The check is a
    /// character rule, not a substring-position heuristic.
    #[test]
    fn placeholder_floor_refuses_a_value_that_is_exactly_a_denied_character() {
        let err = substitute_one("/content/{version}/CUI", "version", "?").unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// Encoding edge: a literal NUL byte, and separately its percent-encoded form.
    #[test]
    fn placeholder_floor_refuses_a_nul_byte_in_both_forms() {
        assert!(substitute_one("/x/{v}", "v", "a\u{0}b").is_err());
        assert!(substitute_one("/x/{v}", "v", "a%00b").is_err());
    }

    /// An empty value is refused — it would otherwise compose an empty segment.
    #[test]
    fn placeholder_floor_refuses_an_empty_value() {
        assert!(substitute_one("/x/{v}", "v", "").is_err());
    }

    /// The always-on cap holds at exactly `PLACEHOLDER_MAX_LENGTH`.
    #[test]
    fn placeholder_floor_accepts_the_cap_and_refuses_one_more() {
        let at_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH);
        assert_eq!(
            substitute_one("/x/{v}", "v", &at_cap).expect("at the cap"),
            format!("/x/{at_cap}")
        );
        let over_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH + 1);
        assert!(substitute_one("/x/{v}", "v", &over_cap).is_err());
    }

    /// A conforming value matching its DECLARED pattern is accepted and the path
    /// is fully substituted.
    #[test]
    fn placeholder_floor_accepts_a_value_matching_its_declared_pattern() {
        let parameters = vec![
            Parameter::new("cui", ParameterLocation::Path, true).with_rules(
                Some("^C[0-9]+$".to_string()),
                Some(32),
                false,
            ),
        ];
        let mut args = serde_json::Map::new();
        args.insert("cui".to_string(), serde_json::json!("C0018787"));
        let resolved = HttpClient::substitute_path(&op("/CUI/{cui}/content", parameters), &args)
            .expect("a conforming value must be accepted");
        assert_eq!(resolved, "/CUI/C0018787/content");
    }

    /// D-10: the DECLARED pattern narrows on top of the floor.
    #[test]
    fn placeholder_floor_refuses_a_value_failing_its_declared_pattern() {
        let parameters = vec![
            Parameter::new("cui", ParameterLocation::Path, true).with_rules(
                Some("^C[0-9]+$".to_string()),
                None,
                false,
            ),
        ];
        let mut args = serde_json::Map::new();
        args.insert("cui".to_string(), serde_json::json!("notacui"));
        let err = HttpClient::substitute_path(&op("/CUI/{cui}", parameters), &args).unwrap_err();
        assert!(err.to_string().contains("cui"), "{err}");
        assert!(!err.to_string().contains("notacui"), "{err}");
    }

    /// Empty edge: a template with NO placeholders is returned unchanged and gains
    /// zero new refusals. The composed check still runs, and passes.
    #[test]
    fn placeholder_floor_leaves_a_placeholder_free_template_untouched() {
        let resolved = HttpClient::substitute_path(
            &op("/Line/Mode/tube/Status", vec![]),
            &serde_json::Map::new(),
        )
        .expect("a placeholder-free template must be unaffected");
        assert_eq!(resolved, "/Line/Mode/tube/Status");
    }

    /// A refusal on the SECOND of two placeholders aborts with no
    /// partially-substituted path in existence — the first value is rendered and
    /// checked but nothing is applied until every value has passed.
    #[test]
    fn placeholder_floor_refuses_the_second_of_two_placeholders_without_substituting() {
        let err = substitute(
            "/a/{first}/b/{second}",
            &[
                ("first", serde_json::json!("ok")),
                ("second", serde_json::json!("../escape")),
            ],
        )
        .unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("second"), "{rendered}");
        assert!(
            !rendered.contains("/a/ok/b/"),
            "no partially-substituted path may appear anywhere: {rendered}"
        );
    }

    /// A path parameter ABSENT from `args` is refused, naming the parameter only —
    /// rather than leaving the literal `{name}` in the outbound URL.
    #[test]
    fn placeholder_floor_refuses_an_absent_path_argument() {
        let err = HttpClient::substitute_path(
            &op("/users/{id}/profile", vec![path_param("id")]),
            &serde_json::Map::new(),
        )
        .unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("id"), "{rendered}");
        assert!(
            !rendered.contains('{') && !rendered.contains('}'),
            "the refusal must not echo the template: {rendered}"
        );
        assert!(
            !rendered.contains("/users/"),
            "the refusal must not echo the path: {rendered}"
        );
    }

    /// The COMPOSED check is the only mechanism that can see this: a template
    /// literal prefix plus a value that each pass on their own compose a segment
    /// over the cap. Spec-derived operations carry mid-segment placeholders, so
    /// this shape is reachable without any curated config.
    #[test]
    fn placeholder_floor_refuses_a_composed_segment_over_the_cap() {
        let prefix = "p".repeat(100);
        let value = "v".repeat(200);
        let err = substitute_one(&format!("/x/{prefix}{{id}}"), "id", &value).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// The composed check also refuses a residual `{`/`}` arriving from a template
    /// the curated config parser would not recognize — the spec-derived route that
    /// config validation cannot reach.
    #[test]
    fn placeholder_floor_refuses_a_residual_brace_from_an_unrecognized_template() {
        let err = HttpClient::substitute_path(&op("/x/{a}/y/{b}", vec![path_param("a")]), &{
            let mut args = serde_json::Map::new();
            args.insert("a".to_string(), serde_json::json!("ok"));
            args
        })
        .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }

    /// A template literal carrying traversal is refused by the composed check
    /// alone: no per-value check ever sees a literal, so this row isolates the
    /// composed mechanism.
    #[test]
    fn placeholder_floor_refuses_traversal_written_into_the_template_literal() {
        let err = HttpClient::substitute_path(&op("/a/../b", vec![]), &serde_json::Map::new())
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)), "{err}");
    }
}

/// The `?` narrowing inherited from plan 05, mirrored on the CURATED surface and
/// pinned in BOTH directions.
///
/// A `[[tools]]` `path` is operator-authored configuration, exactly as a Code Mode
/// script's literal path text is operator-authored script text — so the same
/// asymmetry applies: refusing `..` from it catches a traversal bug, while refusing
/// `?` from it rejects legitimate authoring. `substitute_path` therefore splits the
/// composed path at the FIRST `?` and applies the full, unmodified rule set to each
/// side. Nine of the twelve rows below assert what did NOT change, because a
/// narrowing pinned only by accept-rows is indistinguishable from a deleted check.
#[cfg(all(test, feature = "input-validation"))]
mod query_separator {
    use super::d4_support::substitute_one;
    use super::{HttpClient, Operation};
    use pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH;

    /// Substitute nothing — a placeholder-free template, checked as composed.
    fn literal(path: &str) -> Result<String, super::HttpConnectorError> {
        HttpClient::substitute_path(
            &Operation {
                method: "GET".to_string(),
                path: path.to_string(),
                parameters: vec![],
                has_request_body: false,
                base_url: None,
            },
            &serde_json::Map::new(),
        )
    }

    // ---- ACCEPTED: the separator an operator wrote into the config ----

    /// The row the narrowing exists for: a `?` in a curated `[[tools]]` path.
    #[test]
    fn query_separator_accepts_an_author_written_query_string() {
        assert_eq!(
            literal("/Line/Mode/tube/Status?detail=true").expect("author query accepted"),
            "/Line/Mode/tube/Status?detail=true"
        );
    }

    /// The change-request's own curated shape: an author query alongside a floored
    /// placeholder. Both mechanisms coexist on one path.
    #[test]
    fn query_separator_accepts_a_literal_query_alongside_a_floored_placeholder() {
        assert_eq!(
            substitute_one("/content/{version}/CUI?string=x", "version", "current")
                .expect("author query plus conforming placeholder accepted"),
            "/content/current/CUI?string=x"
        );
    }

    /// The Graph-style `$select` projection shape in-tree consumers author.
    #[test]
    fn query_separator_accepts_a_graph_style_dollar_projection() {
        let resolved = literal(
            "/drives/D/items/I/workbook/worksheets/C/range(address='A2:D7')?$select=values",
        )
        .expect("a Graph $select projection must be accepted");
        assert!(resolved.ends_with("?$select=values"), "{resolved}");
    }

    // ---- STILL REFUSED: everything the split does not relax ----

    #[test]
    fn query_separator_still_refuses_traversal_in_the_path_portion() {
        assert!(
            literal("/a/../b?x=1").is_err(),
            "appending a query must not launder a traversal"
        );
    }

    #[test]
    fn query_separator_still_refuses_traversal_in_the_query_portion() {
        let err = literal("/search?next=../../etc/passwd").unwrap_err();
        assert!(!err.to_string().contains("passwd"), "{err}");
    }

    #[test]
    fn query_separator_still_refuses_a_control_byte_in_the_query_portion() {
        assert!(literal("/search?x=a%00b").is_err());
    }

    #[test]
    fn query_separator_still_refuses_an_over_cap_query_portion() {
        let long = "z".repeat(PLACEHOLDER_MAX_LENGTH + 1);
        assert!(literal(&format!("/search?q={long}")).is_err());
    }

    #[test]
    fn query_separator_still_refuses_a_second_question_mark() {
        assert!(
            literal("/search?a=1?b=2").is_err(),
            "only the FIRST `?` is split off; one exemption, not a licence"
        );
    }

    #[test]
    fn query_separator_still_refuses_an_empty_query_portion() {
        assert!(
            literal("/search?").is_err(),
            "a dangling `?` is the same class as a trailing `/`"
        );
    }

    #[test]
    fn query_separator_still_refuses_a_fragment_marker() {
        assert!(literal("/search#frag").is_err());
    }

    /// THE row that proves the narrowing is not a hole: the template carries an
    /// author-written `?` (legal) AND a placeholder value carries an injected one
    /// (still refused by the per-value floor, which is the mechanism the narrowing
    /// relies on for its safety argument).
    #[test]
    fn query_separator_still_refuses_an_injected_separator_from_a_value() {
        let payload = "2026AA?string=x";
        let err = substitute_one("/search/{v}?detail=true", "v", payload).unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains('v'), "{rendered}");
        assert!(
            !rendered.contains("2026AA") && !rendered.contains('?'),
            "the refusal must carry no byte of the value: {rendered}"
        );
    }

    /// The second value route: an injected TRAVERSAL alongside an author query.
    #[test]
    fn query_separator_still_refuses_an_injected_traversal_from_a_value() {
        assert!(substitute_one("/search/{v}?detail=true", "v", "../../etc/passwd").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::auth::NoAuth;
    use crate::http::{Parameter, ParameterLocation};

    fn get_user_op() -> Operation {
        Operation {
            method: "GET".to_string(),
            path: "/users/{id}".to_string(),
            parameters: vec![
                Parameter::new("id", ParameterLocation::Path, true),
                Parameter::new("verbose", ParameterLocation::Query, false),
            ],
            has_request_body: false,
            base_url: None,
        }
    }

    #[test]
    fn test_build_url_with_path_prefix() {
        // Regression: an API-Gateway stage prefix /v1 survives via join_url.
        let client = HttpClient::new(
            reqwest::Client::new(),
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/".to_string(),
            Arc::new(NoAuth),
        )
        .unwrap();
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        let substituted = HttpClient::substitute_path(&op, &args).unwrap();
        let joined = join_url(client.base_url(), &substituted);
        assert_eq!(
            joined,
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/users/42"
        );
    }

    #[test]
    fn test_substitute_path_replaces_placeholder() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!(7));
        assert_eq!(HttpClient::substitute_path(&op, &args).unwrap(), "/users/7");
    }

    #[test]
    fn test_build_query_skips_path_params() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        args.insert("verbose".to_string(), serde_json::json!(true));
        let query = HttpClient::build_query(&op, &args).unwrap();
        assert_eq!(query.get("verbose"), Some(&"true".to_string()));
        assert!(!query.contains_key("id"));
    }

    // -- WR-03 / GAP 4: fallible scalar renderer (reject non-scalar params) -----

    /// An array-of-scalars query param comma-joins (unchanged OpenAPI
    /// `form`/`explode:false` behavior).
    #[test]
    fn render_query_value_comma_joins_scalar_array() {
        let rendered =
            HttpClient::render_query_value("tags", &serde_json::json!(["a", 2, true])).unwrap();
        assert_eq!(rendered, "a,2,true");
    }

    /// A scalar query param renders bare (unchanged).
    #[test]
    fn render_query_value_scalar_passthrough() {
        assert_eq!(
            HttpClient::render_query_value("q", &serde_json::json!("hi")).unwrap(),
            "hi"
        );
        assert_eq!(
            HttpClient::render_query_value("n", &serde_json::json!(7)).unwrap(),
            "7"
        );
    }

    /// `render_scalar` renders Null as the bare string `"null"` (matches the
    /// code_mode `scalar_str` counterpart).
    #[test]
    fn render_scalar_null_is_bare_null() {
        assert_eq!(
            render_scalar("x", &serde_json::Value::Null).unwrap(),
            "null"
        );
    }

    /// An OBJECT path param is rejected, naming the param; the error never echoes
    /// the value and never produces a JSON-stringified `{`/`[`/`"`.
    #[test]
    fn substitute_path_rejects_object_param() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!({"nested": "x"}));
        let err = HttpClient::substitute_path(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        let rendered = err.to_string();
        assert!(
            rendered.contains("id"),
            "error must name the param: {rendered}"
        );
        for forbidden in ['{', '[', '"'] {
            assert!(
                !rendered.contains(forbidden),
                "must not echo JSON: {rendered}"
            );
        }
        // Pitfall 5: never echo the value.
        assert!(
            !rendered.contains("nested"),
            "must not echo the value: {rendered}"
        );
    }

    /// An OBJECT query param is rejected, naming the param.
    #[test]
    fn build_query_rejects_object_param() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("verbose".to_string(), serde_json::json!({"k": "v"}));
        let err = HttpClient::build_query(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("verbose"));
    }

    /// An array CONTAINING a non-scalar member is rejected (the scalar comma-join
    /// is preserved only for scalar-only arrays).
    #[test]
    fn render_query_value_rejects_array_with_object_member() {
        let err = HttpClient::render_query_value("tags", &serde_json::json!(["ok", {"bad": 1}]))
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("tags"));
    }

    /// A non-scalar HEADER param is rejected, naming the param.
    #[test]
    fn build_headers_rejects_non_scalar_param() {
        let op = Operation {
            method: "GET".to_string(),
            path: "/x".to_string(),
            parameters: vec![Parameter::new("x-trace", ParameterLocation::Header, false)],
            has_request_body: false,
            base_url: None,
        };
        // An ARRAY in header position is non-scalar (arrays comma-join ONLY in
        // query position) and is rejected.
        let mut args = serde_json::Map::new();
        args.insert("x-trace".to_string(), serde_json::json!(["a", "b"]));
        let err = HttpClient::build_headers(&op, &args).unwrap_err();
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        assert!(err.to_string().contains("x-trace"));
        // An OBJECT in header position is likewise rejected.
        let mut args2 = serde_json::Map::new();
        args2.insert("x-trace".to_string(), serde_json::json!({"k": "v"}));
        let err2 = HttpClient::build_headers(&op, &args2).unwrap_err();
        assert!(matches!(err2, HttpConnectorError::Backend(_)));
        assert!(err2.to_string().contains("x-trace"));
        // A scalar header value still succeeds.
        let mut args3 = serde_json::Map::new();
        args3.insert("x-trace".to_string(), serde_json::json!("abc"));
        let headers = HttpClient::build_headers(&op, &args3).unwrap();
        assert_eq!(headers.get("x-trace").unwrap(), "abc");
    }

    #[test]
    fn test_new_is_lazy_and_rejects_bad_url() {
        // Lazy: a bad URL fails synchronously without any network (CF-2).
        let err = HttpClient::new(
            reqwest::Client::new(),
            "not a url".to_string(),
            Arc::new(NoAuth),
        )
        .err()
        .expect("bad URL should error");
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        let rendered = err.to_string();
        assert!(!rendered.contains("not a url"), "must not echo the bad URL");
    }

    #[tokio::test]
    async fn http_connector_get_returns_json() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 42, "name": "Ada"})),
            )
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["id"], 42);
        assert_eq!(result["name"], "Ada");
    }

    #[tokio::test]
    async fn http_connector_post_sends_body_and_auth() {
        use wiremock::matchers::{body_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/items"))
            .and(header("authorization", "Bearer tok"))
            .and(body_json(serde_json::json!({"name": "widget"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let auth = crate::http::auth::create_auth_provider(&crate::http::AuthConfig::Bearer {
            token: "tok".to_string(),
            required: true,
        })
        .unwrap();
        let client = HttpClient::new(reqwest::Client::new(), server.uri(), auth).unwrap();
        let op = Operation {
            method: "POST".to_string(),
            path: "/items".to_string(),
            parameters: vec![],
            has_request_body: true,
            base_url: None,
        };
        let args = serde_json::json!({"name": "widget"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["ok"], true);
    }

    #[tokio::test]
    async fn http_connector_maps_non_2xx_to_status_without_url() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let err = client.execute(&op, &args).await.unwrap_err();
        assert!(matches!(err, HttpConnectorError::Status { status: 404 }));
        let rendered = err.to_string();
        assert!(rendered.contains("404"));
        assert!(
            !rendered.contains("http://"),
            "status error must not echo the URL"
        );
    }
}
