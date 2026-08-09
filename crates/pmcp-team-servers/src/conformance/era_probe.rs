//! The crate's single RAW streamable-HTTP wire seam for era observation.
//!
//! Ported from `crates/mcp-tester/src/tester.rs` (the Phase-117 raw-probe
//! region) under Phase 118 **D-16**, which says *reuse, do not reinvent*.
//!
//! # Why a raw HTTP client rather than the existing `ClientTarget`
//!
//! Two independent reasons, both MEASURED against this repository rather than
//! inferred:
//!
//! 1. **The in-process transport cannot carry v2 at all.**
//!    `crate::DuplexTransport` (`src/transport.rs:47`) implements only `send` /
//!    `receive` / `close` / `is_connected` / `transport_type`. It never
//!    overrides `supports_negotiated_protocol_version` (trait default `false`,
//!    `src/shared/transport.rs:351`), and `ClientBuilder::build`
//!    (`src/client/mod.rs:5213`) emits an explicit `tracing` record calling such
//!    a selection **INERT**. A matrix built on it would have compared v1 against
//!    v1 and reported green having measured nothing — the confirmed D-16 defect.
//!
//! 2. **`pmcp::HttpTransport` is a TRAP, and the runner's `ClientTarget::http`
//!    is built on it.** `impl Transport for HttpTransport`
//!    (`src/shared/http.rs:476`) ALSO does not override
//!    `supports_negotiated_protocol_version`, so it is v1-only for era purposes
//!    too. The transport that DOES carry v2 is `pmcp::StreamableHttpTransport`
//!    (`src/shared/streamable_http.rs:1779`, which returns `true` with the
//!    comment *"This transport DOES have a wire representation for the
//!    negotiated version"*).
//!
//! And even a correct typed client would not be enough: a typed client HIDES
//! the response header, the HTTP status and the raw envelope that most era
//! observations ARE. See
//! `.planning/phases/118-conformance-against-the-official-suite/118-REVIEWS.md`
//! for the cross-AI review consensus that established both facts against source.
//!
//! # Posture
//!
//! Everything here is BOUNDED and TOTAL. The client carries an explicit request
//! timeout so a hung or streaming server fails the probe rather than the run
//! (T-118-68); response bodies are truncated at `MAX_PROBE_BODY_BYTES` on a
//! CHARACTER boundary; and no function outside `#[cfg(test)]` can panic on what
//! a server sent it (T-118-69). A testing tool that panics on hostile input is
//! not a testing tool.
//!
//! Every header name comes from [`pmcp::shared::http_constants`] and every
//! protocol-version string from pmcp's own constants — never as a literal here.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use pmcp::shared::http_constants::{
    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, MCP_METHOD, MCP_NAME,
    MCP_PROTOCOL_VERSION, MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use pmcp::testing::{META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION};
use pmcp::types::protocol::{Era, PROTOCOL_VERSION_2026_07_28};
use serde_json::{json, Value};

/// The wall-clock ceiling on ONE probe request.
///
/// Explicit rather than `reqwest`'s default (which is none): an era run issues
/// roughly sixteen requests per era against a server it is deliberately sending
/// malformed and header-omitting requests to, and a server that answers one of
/// them by holding the socket open must fail THAT probe rather than wedge the
/// whole matrix (T-118-68).
const PROBE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum response bytes a raw probe reads. Bounds a streaming SSE server.
const MAX_PROBE_BODY_BYTES: usize = 64 * 1024;

/// The JSON-RPC id source for probe bodies.
///
/// A monotonically increasing counter rather than a random id: the era target
/// and every observation it produces must be byte-reproducible, and a random id
/// would put a fresh value into every request body for no benefit. It also keeps
/// `rand` out of this crate's dependency graph (T-118-SC: this plan adds no new
/// registry package).
static PROBE_ID: AtomicU64 = AtomicU64::new(1);

/// The `_meta` key a caller may pre-set on a probe's params.
///
/// Sourced through this module so a caller never re-spells it; see
/// [`build_probe_body`] for the merge rule that keeps a caller key and the
/// reserved era keys from clobbering each other.
const META_FIELD: &str = "_meta";

/// Whether a v2 raw probe emits the v2 routing headers.
///
/// [`Self::OmitMethodAndName`] exists for exactly ONE observation —
/// `header.mcp_method_and_name` — which can only be established by SENDING a
/// request without those headers and seeing what happens. A header that is not
/// required looks exactly like one that is, until you leave it out.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_probe::V2HeaderMode;
///
/// assert_ne!(V2HeaderMode::Standard, V2HeaderMode::OmitMethodAndName);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2HeaderMode {
    /// Emit `Mcp-Method`, `Mcp-Name` and the protocol-version header (the
    /// conformant shape).
    ///
    /// `Mcp-Name` is emitted even when empty: since Phase 118 **D-13** a server
    /// requires it only on name-bearing methods and DISCARDS it elsewhere
    /// (D-20), so emitting it unconditionally is a valid superset.
    Standard,
    /// Emit only the protocol-version header, deliberately omitting
    /// `Mcp-Method` and `Mcp-Name`.
    OmitMethodAndName,
}

/// What a raw wire probe SAW. Pure data — no classification.
///
/// Classification is the caller's job, and deliberately so: a probe that
/// classified its own result would put the era rule in as many places as there
/// are probes.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_probe::RawProbeOutcome;
///
/// let served = RawProbeOutcome {
///     http_status: 200,
///     session_header: None,
///     result: Some(serde_json::json!({})),
///     error_code: None,
/// };
/// assert!(served.is_result());
///
/// let refused = RawProbeOutcome {
///     http_status: 404,
///     session_header: None,
///     result: None,
///     error_code: Some(-32601),
/// };
/// assert!(!refused.is_result());
/// ```
#[derive(Debug, Clone)]
pub struct RawProbeOutcome {
    /// HTTP status of the response.
    pub http_status: u16,
    /// The session-id response header, if the server sent one.
    pub session_header: Option<String>,
    /// The JSON-RPC `result` object, when the response carried one.
    pub result: Option<Value>,
    /// The JSON-RPC error code, when the response carried an error.
    pub error_code: Option<i64>,
}

impl RawProbeOutcome {
    /// Whether the response carried a JSON-RPC `result`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_probe::RawProbeOutcome;
    ///
    /// let empty = RawProbeOutcome {
    ///     http_status: 202,
    ///     session_header: None,
    ///     result: None,
    ///     error_code: None,
    /// };
    /// assert!(!empty.is_result());
    /// ```
    #[must_use]
    pub fn is_result(&self) -> bool {
        self.result.is_some()
    }
}

/// Truncate a probe response body to [`MAX_PROBE_BODY_BYTES`] on a CHARACTER
/// boundary.
///
/// Slicing a `str` at a raw byte index PANICS when that index lands inside a
/// multi-byte UTF-8 sequence, and a 64 KiB cut through a body containing any
/// non-ASCII text (a server name, an error message, a tool description) lands
/// there roughly three times in four. A testing tool must not panic on what a
/// server sent it, so the cut is walked back to the nearest boundary.
///
/// PURE, TOTAL, and both unit- and property-tested below.
fn truncate_probe_body(text: &str) -> &str {
    if text.len() <= MAX_PROBE_BODY_BYTES {
        return text;
    }
    let mut end = MAX_PROBE_BODY_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Extract the JSON-RPC envelope from a response body that may be SSE-framed.
///
/// A Streamable-HTTP server may answer a POST either with `application/json`
/// (the whole envelope) or with `text/event-stream` (the envelope inside one or
/// more `data:` lines). BOTH are conformant, so both are parsed here — a probe
/// that understood only one framing would misread half the servers it meets,
/// and would report every observation it drew from the other half as a finding
/// about the server rather than about itself.
///
/// PURE and TOTAL over arbitrary input: it returns `None` rather than failing.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_probe::extract_jsonrpc_envelope;
///
/// let plain = extract_jsonrpc_envelope("application/json", r#"{"result":{"ok":true}}"#);
/// assert_eq!(plain.and_then(|v| v["result"]["ok"].as_bool()), Some(true));
///
/// let framed = extract_jsonrpc_envelope(
///     "text/event-stream",
///     "event: message\ndata: {\"result\":{\"ok\":true}}\n\n",
/// );
/// assert_eq!(framed.and_then(|v| v["result"]["ok"].as_bool()), Some(true));
///
/// // Total over garbage.
/// assert!(extract_jsonrpc_envelope("application/json", "not json at all").is_none());
/// ```
#[must_use]
pub fn extract_jsonrpc_envelope(content_type: &str, body: &str) -> Option<Value> {
    if content_type.contains(TEXT_EVENT_STREAM) {
        for line in body.lines() {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
                return Some(value);
            }
        }
        return None;
    }
    serde_json::from_str::<Value>(body).ok()
}

/// The reserved `_meta` object a v2 probe body carries.
///
/// Split out of [`build_probe_body`] so the merge rule below reads as one
/// statement rather than as a nested literal.
fn reserved_v2_meta() -> Value {
    json!({
        META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
        META_CLIENT_INFO: {
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
        },
        // All three MRTR-fulfillable capabilities are declared, because the
        // server refuses to emit `inputRequests` for a capability the client did
        // not declare (`reject_undeclared_capabilities`,
        // `src/server/core.rs:2931`). A probe that under-declared would exercise
        // that refusal path and report the CONF-03 observations `absent` against
        // a server implementing them perfectly well.
        META_CLIENT_CAPABILITIES: { "elicitation": {}, "sampling": {}, "roots": {} },
    })
}

/// Build the JSON-RPC request body for `era`.
///
/// On v2 the reserved `_meta` keys are attached because the server's era gate
/// requires the header and the body to AGREE — a disagreement is rejected with
/// `HEADER_MISMATCH`. On v1 the body carries NO reserved keys at all, and that
/// absence is precisely what makes it a v1 request.
///
/// # The merge rule
///
/// A caller may pre-set its own `_meta` keys on `params` — the CONF-03
/// `meta.log_level` probe does exactly that. The reserved keys are therefore
/// MERGED INTO a caller-supplied `_meta` object rather than replacing it. The
/// port source replaced it outright, which was safe there only because no caller
/// in that crate set `_meta`; here it would silently delete the very key the
/// probe exists to send, and the observation would report `ignored` under BOTH
/// eras — a permanent false MISSING produced by the probe, not by the server.
/// The reserved keys win on a collision, because the era gate compares them
/// against the headers this module sets.
///
/// # Examples
///
/// ```
/// use pmcp::types::protocol::Era;
/// use pmcp_team_servers::conformance::era_probe::build_probe_body;
/// use serde_json::json;
///
/// // v1 carries no reserved keys, which is what makes it a v1 request.
/// let v1 = build_probe_body("tools/list", json!({}), Era::V1);
/// assert!(!v1.contains("io.modelcontextprotocol/protocolVersion"));
///
/// // v2 carries them, so header and body agree.
/// let v2 = build_probe_body("tools/list", json!({}), Era::V2);
/// assert!(v2.contains("io.modelcontextprotocol/protocolVersion"));
///
/// // A caller's own `_meta` key survives the merge.
/// let merged = build_probe_body(
///     "tools/call",
///     json!({ "_meta": { "example.test/key": "kept" } }),
///     Era::V2,
/// );
/// assert!(merged.contains("example.test/key"));
/// assert!(merged.contains("io.modelcontextprotocol/clientCapabilities"));
/// ```
#[must_use]
pub fn build_probe_body(method: &str, params: Value, era: Era) -> String {
    // Keep the MAP rather than rebuilding a `Value`: a non-object `params` is
    // replaced by an empty object, which is the only shape the reserved-key
    // merge below can be applied to.
    let mut params = match params {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if era == Era::V2 {
        let mut meta = match params.remove(META_FIELD) {
            Some(Value::Object(existing)) => existing,
            _ => serde_json::Map::new(),
        };
        if let Value::Object(reserved) = reserved_v2_meta() {
            for (key, value) in reserved {
                meta.insert(key, value);
            }
        }
        params.insert(META_FIELD.to_string(), Value::Object(meta));
    }
    json!({
        "jsonrpc": "2.0",
        "id": PROBE_ID.fetch_add(1, Ordering::Relaxed),
        "method": method,
        "params": params,
    })
    .to_string()
}

/// A raw streamable-HTTP client pointed at ONE MCP endpoint.
///
/// Holds one `reqwest::Client` and reuses it across probes, so an era run does
/// not rebuild a TLS configuration and a fresh connection pool sixteen times.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_probe::EraProbeClient;
///
/// let probe = EraProbeClient::new("http://127.0.0.1:1/");
/// assert!(probe.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct EraProbeClient {
    url: String,
    client: reqwest::Client,
}

impl EraProbeClient {
    /// Build a probe client for `url`.
    ///
    /// # Errors
    ///
    /// Returns the client-construction failure as a `String` — this module
    /// never panics outside its tests, and a caller that could not build a
    /// client has learned nothing about any server.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_probe::EraProbeClient;
    ///
    /// let probe = EraProbeClient::new("http://127.0.0.1:1/");
    /// assert!(probe.is_ok());
    /// ```
    pub fn new(url: impl Into<String>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(PROBE_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("could not build the era probe client: {error}"))?;
        Ok(Self {
            url: url.into(),
            client,
        })
    }

    /// The endpoint this client probes.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_probe::EraProbeClient;
    ///
    /// let probe = EraProbeClient::new("http://127.0.0.1:1/");
    /// assert_eq!(probe.map(|p| p.url().to_string()).ok(), Some("http://127.0.0.1:1/".to_string()));
    /// ```
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Send ONE raw JSON-RPC request with era-appropriate framing and report
    /// what came back.
    ///
    /// On `Era::V2` this sets the protocol-version header, and in
    /// [`V2HeaderMode::Standard`] also the method and name routing headers. On
    /// `Era::V1` it sets NONE of them, which is what makes it a v1 request.
    /// Both `application/json` and `text/event-stream` responses are parsed.
    ///
    /// `name` is the logical routing name: pass `""` for a method that has none.
    /// Since Phase 118 D-13 a server requires the header only on name-bearing
    /// methods and discards it elsewhere, so an unconditional empty value is a
    /// valid superset.
    ///
    /// # Why the session parameter exists
    ///
    /// MEASURED: a STATEFUL v1 server rejects every non-initialization request
    /// that arrives without a session id, with `400` and
    /// *"Session ID required for non-initialization requests"*
    /// (`validate_non_init_session`,
    /// `src/server/streamable_http_server/v1_session.rs`). A v1 probe carrying
    /// no session would therefore be refused for a SESSION reason while looking
    /// exactly like a refusal for the reason the probe is about, and several
    /// observations would mis-report at once. v2 never mints a session (ERA-03),
    /// so on that path this is always `None` and costs nothing.
    ///
    /// # Errors
    ///
    /// Returns the transport failure as a `String`. That is a DIFFERENT fact
    /// from *"the server answered with an error"*, and the callers depend on the
    /// distinction: the first is `Unavailable`, the second is an observation.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::types::protocol::Era;
    /// use pmcp_team_servers::conformance::era_probe::{EraProbeClient, V2HeaderMode};
    /// use serde_json::json;
    ///
    /// # async fn demo() {
    /// let probe = EraProbeClient::new("http://127.0.0.1:1/").expect("client builds");
    /// // Nothing is listening on port 1, so this is a TRANSPORT failure, which is
    /// // reported as `Err` and never confused with a served refusal.
    /// let outcome = probe
    ///     .raw_jsonrpc_probe_with_session(
    ///         "tools/list",
    ///         "",
    ///         json!({}),
    ///         Era::V1,
    ///         V2HeaderMode::Standard,
    ///         None,
    ///     )
    ///     .await;
    /// assert!(outcome.is_err());
    /// # }
    /// ```
    pub async fn raw_jsonrpc_probe_with_session(
        &self,
        method: &str,
        name: &str,
        params: Value,
        era: Era,
        header_mode: V2HeaderMode,
        session_id: Option<&str>,
    ) -> Result<RawProbeOutcome, String> {
        let body = build_probe_body(method, params, era);
        let mut request = self
            .client
            .post(&self.url)
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(ACCEPT, ACCEPT_STREAMABLE);
        if era == Era::V2 {
            request = request.header(MCP_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28);
            if header_mode == V2HeaderMode::Standard {
                request = request.header(MCP_METHOD, method).header(MCP_NAME, name);
            }
        }
        if let Some(session) = session_id {
            request = request.header(MCP_SESSION_ID, session);
        }

        let response = request
            .body(body)
            .send()
            .await
            .map_err(|error| format!("{method} probe transport failure: {error}"))?;
        Ok(Self::read_outcome(response).await)
    }

    /// Drain a response into a [`RawProbeOutcome`].
    ///
    /// Split out so [`Self::raw_jsonrpc_probe_with_session`] stays one
    /// statement per wire concern and neither function approaches the
    /// cognitive-complexity ceiling.
    async fn read_outcome(response: reqwest::Response) -> RawProbeOutcome {
        let http_status = response.status().as_u16();
        let content_type = header_str(response.headers(), CONTENT_TYPE);
        let session_header = response
            .headers()
            .get(MCP_SESSION_ID)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        // A body this client could not read is an EMPTY body, never a panic: the
        // status and headers above are already observations in their own right,
        // and discarding them because the payload was malformed would turn a
        // server-side fact into `Unavailable`.
        let text = response.text().await.unwrap_or_default();
        let envelope = extract_jsonrpc_envelope(&content_type, truncate_probe_body(&text));

        let result = envelope
            .as_ref()
            .and_then(|value| value.get("result"))
            .filter(|value| !value.is_null())
            .cloned();
        let error_code = envelope
            .as_ref()
            .and_then(|value| value.get("error"))
            .and_then(|error| error.get("code"))
            .and_then(Value::as_i64);

        RawProbeOutcome {
            http_status,
            session_header,
            result,
            error_code,
        }
    }

    /// Send a raw HTTP VERB (`GET` / `DELETE`) at the MCP endpoint and report
    /// its status and content type.
    ///
    /// This is how the HTTP-surface observations that no JSON-RPC request can
    /// see are made: `http.verb.get_delete` and `header.last_event_id` are
    /// facts about the endpoint, not about any method.
    ///
    /// # Errors
    ///
    /// Returns the transport failure — or an unparseable verb — as a `String`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::types::protocol::Era;
    /// use pmcp_team_servers::conformance::era_probe::EraProbeClient;
    ///
    /// # async fn demo() {
    /// let probe = EraProbeClient::new("http://127.0.0.1:1/").expect("client builds");
    /// assert!(probe.raw_verb_probe("GET", Era::V1, &[]).await.is_err());
    /// # }
    /// ```
    pub async fn raw_verb_probe(
        &self,
        verb: &str,
        era: Era,
        extra_headers: &[(&str, &str)],
    ) -> Result<(u16, String), String> {
        let request_method = reqwest::Method::from_bytes(verb.as_bytes())
            .map_err(|error| format!("invalid HTTP verb {verb}: {error}"))?;
        let mut request = self
            .client
            .request(request_method, &self.url)
            .header(ACCEPT, ACCEPT_STREAMABLE);
        if era == Era::V2 {
            request = request.header(MCP_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28);
        }
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("{verb} probe transport failure: {error}"))?;
        let status = response.status().as_u16();
        Ok((status, header_str(response.headers(), CONTENT_TYPE)))
    }
}

/// One response header, lowercased, or the empty string.
///
/// Lowercased because every caller matches a content type against a lowercase
/// literal, and a server is free to send `Application/JSON`.
fn header_str(headers: &reqwest::header::HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_leaves_a_short_body_alone() {
        assert_eq!(truncate_probe_body("hello"), "hello");
        assert_eq!(truncate_probe_body(""), "");
    }

    /// The measured failure mode: a 64 KiB cut through non-ASCII text.
    ///
    /// The multi-byte character straddles the cap, so a raw byte slice would
    /// panic. The walked-back cut must land BEFORE it.
    #[test]
    fn truncate_walks_back_off_a_multi_byte_boundary() {
        // One 3-byte character repeated: no multiple of 3 equals 65536, so the
        // cap necessarily lands mid-character.
        let body = "\u{4e16}".repeat(MAX_PROBE_BODY_BYTES);
        let cut = truncate_probe_body(&body);
        assert!(cut.len() <= MAX_PROBE_BODY_BYTES);
        assert!(MAX_PROBE_BODY_BYTES - cut.len() < 3);
        assert!(body.starts_with(cut));
    }

    #[test]
    fn extract_reads_an_application_json_envelope() {
        let value = extract_jsonrpc_envelope("application/json", r#"{"result":{"a":1}}"#)
            .expect("a well-formed JSON body parses");
        assert_eq!(value["result"]["a"], 1);
    }

    #[test]
    fn extract_reads_a_single_data_line_sse_envelope() {
        let value = extract_jsonrpc_envelope(
            "text/event-stream; charset=utf-8",
            "data: {\"result\":{\"a\":1}}\n\n",
        )
        .expect("a single-data SSE body parses");
        assert_eq!(value["result"]["a"], 1);
    }

    #[test]
    fn extract_reads_a_multi_line_sse_envelope() {
        let body = "event: message\nid: 7\nretry: 100\ndata: {\"result\":{\"a\":2}}\n\n";
        let value = extract_jsonrpc_envelope("text/event-stream", body)
            .expect("a multi-line SSE body parses");
        assert_eq!(value["result"]["a"], 2);
    }

    #[test]
    fn extract_is_total_over_garbage() {
        assert!(extract_jsonrpc_envelope("application/json", "}{").is_none());
        assert!(extract_jsonrpc_envelope("text/event-stream", "data: }{").is_none());
        assert!(extract_jsonrpc_envelope("text/event-stream", "no data lines").is_none());
        assert!(extract_jsonrpc_envelope("", "").is_none());
    }

    #[test]
    fn v1_bodies_carry_no_reserved_keys() {
        let body = build_probe_body("tools/list", json!({}), Era::V1);
        assert!(!body.contains(META_PROTOCOL_VERSION));
        assert!(!body.contains(META_CLIENT_INFO));
        assert!(!body.contains(META_CLIENT_CAPABILITIES));
        assert!(!body.contains(PROTOCOL_VERSION_2026_07_28));
    }

    #[test]
    fn v2_bodies_carry_every_reserved_key() {
        let body = build_probe_body("tools/list", json!({}), Era::V2);
        for key in [
            META_PROTOCOL_VERSION,
            META_CLIENT_INFO,
            META_CLIENT_CAPABILITIES,
        ] {
            assert!(body.contains(key), "the v2 body must carry `{key}`");
        }
        let parsed: Value = serde_json::from_str(&body).expect("the body is JSON");
        assert_eq!(
            parsed["params"]["_meta"][META_PROTOCOL_VERSION],
            PROTOCOL_VERSION_2026_07_28
        );
    }

    /// The merge rule, which the port source did not have: a caller's `_meta`
    /// key must survive, or the `meta.log_level` probe silently sends nothing.
    #[test]
    fn v2_bodies_merge_rather_than_replace_a_caller_meta() {
        let body = build_probe_body(
            "tools/call",
            json!({ "name": "t", "_meta": { "example.test/key": "kept" } }),
            Era::V2,
        );
        let parsed: Value = serde_json::from_str(&body).expect("the body is JSON");
        assert_eq!(parsed["params"]["_meta"]["example.test/key"], "kept");
        assert_eq!(
            parsed["params"]["_meta"][META_PROTOCOL_VERSION],
            PROTOCOL_VERSION_2026_07_28
        );
        assert_eq!(parsed["params"]["name"], "t");
    }

    /// A v1 body leaves a caller's `_meta` alone AND adds nothing to it.
    #[test]
    fn v1_bodies_keep_a_caller_meta_untouched() {
        let body = build_probe_body(
            "tools/call",
            json!({ "_meta": { "example.test/key": "kept" } }),
            Era::V1,
        );
        let parsed: Value = serde_json::from_str(&body).expect("the body is JSON");
        assert_eq!(parsed["params"]["_meta"]["example.test/key"], "kept");
        assert_eq!(
            parsed["params"]["_meta"]
                .as_object()
                .map(serde_json::Map::len),
            Some(1)
        );
    }

    #[test]
    fn non_object_params_become_an_empty_object() {
        let body = build_probe_body("ping", json!("not an object"), Era::V1);
        let parsed: Value = serde_json::from_str(&body).expect("the body is JSON");
        assert_eq!(parsed["params"], json!({}));
    }

    #[test]
    fn probe_ids_are_distinct_and_monotonic() {
        let first: Value = serde_json::from_str(&build_probe_body("ping", json!({}), Era::V1))
            .expect("the body is JSON");
        let second: Value = serde_json::from_str(&build_probe_body("ping", json!({}), Era::V1))
            .expect("the body is JSON");
        let (Some(a), Some(b)) = (first["id"].as_u64(), second["id"].as_u64()) else {
            panic!("probe ids are numbers");
        };
        assert!(
            b > a,
            "ids must advance so an MRTR resend differs from its first call"
        );
    }

    #[test]
    fn a_client_carries_its_url() {
        let probe = EraProbeClient::new("http://127.0.0.1:1/").expect("client builds");
        assert_eq!(probe.url(), "http://127.0.0.1:1/");
    }

    #[test]
    fn an_outcome_distinguishes_a_result_from_a_refusal() {
        let served = RawProbeOutcome {
            http_status: 200,
            session_header: Some("s".to_string()),
            result: Some(json!({})),
            error_code: None,
        };
        assert!(served.is_result());
        let refused = RawProbeOutcome {
            http_status: 404,
            session_header: None,
            result: None,
            error_code: Some(-32601),
        };
        assert!(!refused.is_result());
    }

    // CLAUDE.md ALWAYS / PROPERTY + FUZZ arms. Both pure helpers are TOTAL over
    // arbitrary input, which is the whole reason a testing tool may be pointed
    // at a server it does not trust (T-118-69).
    proptest::proptest! {
        /// `truncate_probe_body` never panics and always returns a valid `&str`
        /// that is a PREFIX of its input.
        #[test]
        fn truncate_is_total_over_arbitrary_text(raw in ".*") {
            let cut = truncate_probe_body(&raw);
            proptest::prop_assert!(cut.len() <= raw.len());
            proptest::prop_assert!(cut.len() <= MAX_PROBE_BODY_BYTES);
            proptest::prop_assert!(raw.starts_with(cut));
        }

        /// The same, over bodies built to STRADDLE the cap with multi-byte
        /// characters — the case a naive byte slice panics on.
        #[test]
        fn truncate_is_total_over_oversized_multi_byte_text(pad in 0usize..8, ch in "[\u{80}-\u{10ffff}]") {
            let unit = ch.chars().next().map_or(1, char::len_utf8);
            let repeats = MAX_PROBE_BODY_BYTES / unit + pad + 1;
            let body = ch.repeat(repeats);
            let cut = truncate_probe_body(&body);
            proptest::prop_assert!(cut.len() <= MAX_PROBE_BODY_BYTES);
            proptest::prop_assert!(body.starts_with(cut));
        }

        /// `extract_jsonrpc_envelope` is TOTAL over arbitrary
        /// `(content_type, body)` pairs — it answers, it never fails.
        #[test]
        fn extract_is_total_over_arbitrary_pairs(content_type in ".*", body in ".*") {
            let _ = extract_jsonrpc_envelope(&content_type, &body);
        }

        /// `build_probe_body` always emits parseable JSON-RPC, whatever the
        /// method string and params it is handed.
        #[test]
        fn build_probe_body_always_emits_parseable_jsonrpc(method in ".*", flag: bool) {
            let era = if flag { Era::V2 } else { Era::V1 };
            let body = build_probe_body(&method, json!({ "a": 1 }), era);
            let parsed: Value = serde_json::from_str(&body)
                .map_err(|e| proptest::test_runner::TestCaseError::fail(e.to_string()))?;
            proptest::prop_assert_eq!(parsed["jsonrpc"].as_str(), Some("2.0"));
            proptest::prop_assert_eq!(parsed["method"].as_str(), Some(method.as_str()));
            proptest::prop_assert_eq!(
                parsed["params"]["_meta"][META_PROTOCOL_VERSION].is_string(),
                era == Era::V2
            );
        }
    }
}
