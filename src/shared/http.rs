//! HTTP/SSE transport implementation for MCP.

use crate::error::Result;
use crate::shared::sse_parser::SseParser;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::Mutex as AsyncMutex;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use url::Url;

/// HTTP transport configuration.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Base URL for HTTP requests
    pub base_url: Url,
    /// SSE endpoint for receiving notifications
    pub sse_endpoint: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Additional headers to include in requests
    pub headers: Vec<(String, String)>,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Maximum idle connections in pool
    pub max_idle_per_host: usize,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".parse().expect("Valid default URL"),
            sse_endpoint: Some("/events".to_string()),
            timeout: Duration::from_secs(30),
            headers: vec![],
            enable_pooling: true,
            max_idle_per_host: 10,
        }
    }
}

/// HTTP/SSE transport implementation.
pub struct HttpTransport {
    config: HttpConfig,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
    message_queue: Arc<AsyncMutex<mpsc::Receiver<TransportMessage>>>,
    message_tx: mpsc::Sender<TransportMessage>,
    connected: Arc<RwLock<bool>>,
    /// In-flight ceiling for [`Self::connect_sse`]'s reader task, defaulted from
    /// [`DEFAULT_HTTP_SSE_BUFFERED_BYTES`] and overridable through
    /// [`Self::with_sse_buffered_bytes`].
    ///
    /// A PRIVATE field on the transport rather than a `pub` field on
    /// [`HttpConfig`]: `HttpConfig` is externally constructible, so adding a
    /// field to it fails `cargo semver-checks`'s `constructible_struct_adds_field`
    /// and would force pmcp to a MAJOR version. Measured, not assumed — see plan
    /// 113-17's `<config_surface_decision>`. Every field of this struct is
    /// already private, so adding one here is invisible to semver.
    sse_buffered_bytes: usize,
    /// Cap on ONE fully-collected response body — the POST response
    /// [`Self::send_request`] reads — defaulted from
    /// [`DEFAULT_HTTP_COLLECTED_BODY_BYTES`] and overridable through
    /// [`Self::with_max_collected_body_bytes`].
    ///
    /// A PRIVATE field for the same measured semver reason as
    /// [`Self::sse_buffered_bytes`].
    max_collected_body_bytes: usize,
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport")
            .field("config", &self.config)
            .field("connected", &self.connected)
            .field("sse_buffered_bytes", &self.sse_buffered_bytes)
            .field("max_collected_body_bytes", &self.max_collected_body_bytes)
            .finish_non_exhaustive()
    }
}

// The DEFINITION moved to `crate::shared::http_constants` in plan 113.1-03 so
// `sse_optimized.rs` can reach it: this module is gated on `feature = "http"`,
// which `feature = "sse"` does NOT enable, while `http_constants` is ungated.
// Re-exported here so the existing public path
// `pmcp::shared::http::DEFAULT_HTTP_SSE_BUFFERED_BYTES` is preserved
// byte-for-byte and every unqualified reference in this file keeps resolving.
pub use crate::shared::http_constants::DEFAULT_HTTP_SSE_BUFFERED_BYTES;

/// Default cap on ONE fully-collected response body on this transport, in bytes
/// (16 MiB).
///
/// `HttpTransport::send_request` reads its POST response with
/// `Full`-body semantics: the whole thing lands in memory before it is parsed,
/// and the PEER chooses how many bytes it sends. Without a cap that read was
/// unbounded — the same defect class 113-17 fixed on this file's sibling SSE
/// reader and 113-20 fixed on `StreamableHttpTransport`'s three whole-body reads
/// (review CR-03).
///
/// # Deliberately NOT the same quantity as the SSE in-flight ceiling
///
/// [`DEFAULT_HTTP_SSE_BUFFERED_BYTES`] bounds INCREMENTAL retention inside the
/// long-lived `connect_sse` reader — a running total across many chunks. This
/// bounds a ONE-SHOT collected body. They happen to share a number; they are not
/// the same knob and must not be unified.
///
/// # What breaks at this boundary
///
/// A response larger than the configured cap now fails with
/// [`TransportError::Request`](crate::error::TransportError::Request) instead of
/// being delivered. Base64 `image`/`audio` content expands by ~4/3, so a 12 MiB
/// binary is already 16 MiB encoded and does NOT fit under this default;
/// [`HttpTransport::with_max_collected_body_bytes`] is the escape hatch.
pub const DEFAULT_HTTP_COLLECTED_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Build the parser [`HttpTransport::connect_sse`]'s reader task feeds, bounded
/// at the transport's CONFIGURED ceiling.
///
/// Named rather than inlined, and taking the configured value rather than
/// reading a constant, so a test can assert on the bound this transport ACTUALLY
/// uses. Asserting on a separately-constructed parser would pass no matter what
/// the reader task were changed to.
fn sse_reader_parser(sse_buffered_bytes: usize) -> SseParser {
    SseParser::with_max_buffer_size(sse_buffered_bytes)
}

/// Report an SSE in-flight overflow, returning whether the reader task must end.
///
/// [`HttpTransport::connect_sse`] is the SECOND incremental feeder of the shared
/// [`SseParser`] (the `subscriptions/listen` client is the other): it holds ONE
/// parser for the lifetime of its spawned reader task and feeds it frame by
/// frame. The parser BOUNDS what it retains, so without this observation the
/// discarded bytes would vanish SILENTLY here and the task would carry on as if
/// nothing had happened — strictly worse than the unbounded-but-correct
/// behaviour it replaced (T-113-78).
///
/// What trips the bound is the parser's RETAINED state plus the chunk being fed,
/// not one line and not one event: retained state is an unterminated line PLUS
/// every `data:` line accumulated into an event the peer has not yet ended with a
/// blank line, and one chunk carrying many small complete events can exceed the
/// limit on its total alone (T-113-86). The log message says exactly that.
///
/// The ceiling itself is [`DEFAULT_HTTP_SSE_BUFFERED_BYTES`] unless overridden
/// through [`HttpTransport::with_sse_buffered_bytes`].
///
/// A free function rather than an inline `if` so the condition is reachable from
/// a test — the reader task owns a live `hyper::body::Incoming`, which cannot be
/// constructed outside hyper.
fn report_sse_overflow(parser: &SseParser) -> bool {
    if !parser.overflowed() {
        return false;
    }
    error!(
        "an SSE chunk pushed the buffered stream state past the {}-byte parser \
         bound; the buffered bytes were discarded, so the stream is corrupt and \
         the connection is being closed",
        parser.max_buffer_size()
    );
    true
}

impl HttpTransport {
    /// Create a new HTTP transport with the given configuration.
    pub fn new(config: HttpConfig) -> Self {
        let connector = hyper_util::client::legacy::connect::HttpConnector::new();
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(config.max_idle_per_host)
            .build(connector);

        let (tx, rx) = mpsc::channel(100);

        Self {
            config,
            client,
            message_queue: Arc::new(AsyncMutex::new(rx)),
            message_tx: tx,
            connected: Arc::new(RwLock::new(false)),
            sse_buffered_bytes: DEFAULT_HTTP_SSE_BUFFERED_BYTES,
            max_collected_body_bytes: DEFAULT_HTTP_COLLECTED_BODY_BYTES,
        }
    }

    /// Create a new HTTP transport with default configuration.
    pub fn with_url(url: impl Into<Url>) -> Result<Self> {
        Ok(Self::new(HttpConfig {
            base_url: url.into(),
            ..Default::default()
        }))
    }

    /// Override how many SSE bytes [`Self::connect_sse`]'s reader task may hold
    /// in flight, in bytes.
    ///
    /// Defaults to [`DEFAULT_HTTP_SSE_BUFFERED_BYTES`] (16 MiB). Raise it for a
    /// deployment whose JSON-RPC results are legitimately larger — base64
    /// `image`/`audio` content expands by ~4/3, so a 12 MiB binary alone is
    /// already 16 MiB encoded, before the JSON envelope and the `data: ` prefix,
    /// and a payload past the ceiling is DISCARDED and ends the reader task
    /// (T-113-85). Lower it for a client talking to an untrusted peer whose
    /// payloads are known to be small.
    ///
    /// An inherent builder method rather than an [`HttpConfig`] field: that
    /// struct is externally constructible, so a new field on it is a MAJOR
    /// semver break (plan 113-17 `<config_surface_decision>`), while an added
    /// method is additive.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::shared::http::{HttpConfig, HttpTransport};
    ///
    /// let transport =
    ///     HttpTransport::new(HttpConfig::default()).with_sse_buffered_bytes(64 * 1024 * 1024);
    /// ```
    #[must_use]
    pub fn with_sse_buffered_bytes(mut self, sse_buffered_bytes: usize) -> Self {
        self.sse_buffered_bytes = sse_buffered_bytes;
        self
    }

    /// Override the cap on ONE fully-collected POST response body, in bytes.
    ///
    /// Defaults to [`DEFAULT_HTTP_COLLECTED_BODY_BYTES`] (16 MiB). Raise it for a
    /// deployment whose responses are legitimately larger — base64 `image` /
    /// `audio` content expands by ~4/3, so a 12 MiB binary does NOT fit under the
    /// default once encoded.
    ///
    /// An inherent builder method rather than an [`HttpConfig`] field, for the
    /// same measured semver reason as [`Self::with_sse_buffered_bytes`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::shared::http::{HttpConfig, HttpTransport};
    ///
    /// let transport = HttpTransport::new(HttpConfig::default())
    ///     .with_max_collected_body_bytes(64 * 1024 * 1024);
    /// ```
    #[must_use]
    pub fn with_max_collected_body_bytes(mut self, max_collected_body_bytes: usize) -> Self {
        self.max_collected_body_bytes = max_collected_body_bytes;
        self
    }

    /// Collect a POST response body, refusing anything over `max_bytes`.
    ///
    /// The sibling of `StreamableHttpTransport::collect_body_within_cap`, with
    /// the same two independently-sufficient refusals: a declared
    /// `Content-Length` over the cap is refused before a byte is read, and the
    /// bytes actually delivered are read through `Limited`, which stops at the
    /// cap — so a peer that understates or omits `Content-Length` gains nothing.
    ///
    /// A body of exactly `max_bytes` is admitted; one byte over is refused.
    async fn collect_body_within_cap(
        response: hyper::Response<hyper::body::Incoming>,
        max_bytes: usize,
    ) -> Result<Bytes> {
        let declared = response
            .headers()
            .get(hyper::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok());
        if let Some(declared) = declared {
            if declared > max_bytes {
                return Err(crate::error::Error::Transport(
                    crate::error::TransportError::Request(format!(
                        "response body declares Content-Length {declared}, over this transport's \
                         {max_bytes}-byte collected-body cap (DEFAULT_HTTP_COLLECTED_BODY_BYTES); \
                         raise it with HttpTransport::with_max_collected_body_bytes"
                    )),
                ));
            }
        }
        match Limited::new(response.into_body(), max_bytes)
            .collect()
            .await
        {
            Ok(collected) => Ok(collected.to_bytes()),
            Err(error) if error.is::<LengthLimitError>() => Err(crate::error::Error::Transport(
                crate::error::TransportError::Request(format!(
                    "response body delivered more than this transport's {max_bytes}-byte \
                     collected-body cap (Content-Length absent or understated); raise it with \
                     HttpTransport::with_max_collected_body_bytes"
                )),
            )),
            Err(error) => Err(crate::error::Error::Transport(
                crate::error::TransportError::Request(error.to_string()),
            )),
        }
    }

    /// Connect to SSE endpoint for receiving notifications.
    pub async fn connect_sse(&self) -> Result<()> {
        if let Some(sse_path) = &self.config.sse_endpoint {
            let sse_url = self
                .config
                .base_url
                .join(sse_path)
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;
            info!("Connecting to SSE endpoint: {}", sse_url);

            let req = Request::builder()
                .method(Method::GET)
                .uri(sse_url.as_str())
                .header("Accept", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .body(Full::new(Bytes::new()))
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

            let response = self
                .client
                .request(req)
                .await
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

            if response.status() != StatusCode::OK {
                return Err(crate::error::Error::Transport(
                    crate::error::TransportError::InvalidMessage(format!(
                        "SSE connection failed with status: {}",
                        response.status()
                    )),
                ));
            }

            // Spawn SSE reader task
            let message_tx = self.message_tx.clone();
            let connected = self.connected.clone();
            let sse_buffered_bytes = self.sse_buffered_bytes;

            tokio::spawn(async move {
                *connected.write() = true;

                let mut body = response.into_body();
                let mut sse_parser = sse_reader_parser(sse_buffered_bytes);
                // Bytes received but not yet decodable as complete UTF-8. A body
                // frame boundary can fall in the MIDDLE of a multi-byte
                // character, so decoding each chunk with `from_utf8_lossy` would
                // corrupt any non-ASCII payload that straddles two frames. The
                // shared incremental decoder retains the (≤3 byte) tail instead.
                let mut undecoded: Vec<u8> = Vec::new();

                while let Some(chunk) = body.frame().await {
                    match chunk {
                        Ok(frame) => {
                            if let Some(data) = frame.data_ref() {
                                undecoded.extend_from_slice(data);
                                let text =
                                    crate::shared::sse_parser::take_utf8_prefix(&mut undecoded);
                                let events = sse_parser.feed(&text);

                                // Observed BEFORE the events are drained, ENDED
                                // after: the events this chunk completed are
                                // legitimate and already decoded, so discarding
                                // them would lose good frames on top of the ones
                                // the parser discarded. Same order the
                                // `subscriptions/listen` client uses, which
                                // drains its `pending` queue before honouring the
                                // latch.
                                let overflowed = report_sse_overflow(&sse_parser);

                                for event in events {
                                    // Process SSE event data as JSON-RPC message
                                    match crate::shared::stdio::StdioTransport::parse_message(
                                        event.data.as_bytes(),
                                    ) {
                                        Ok(msg) => {
                                            if message_tx.send(msg).await.is_err() {
                                                error!("Failed to send SSE message");
                                                break;
                                            }
                                        },
                                        Err(e) => {
                                            error!("Failed to parse SSE message: {}", e);
                                        },
                                    }
                                }

                                if overflowed {
                                    // The parser DISCARDED buffered bytes, so the
                                    // byte stream is no longer trustworthy — stop
                                    // reading a peer already established as
                                    // hostile or broken.
                                    break;
                                }
                            }
                        },
                        Err(e) => {
                            error!("SSE stream error: {}", e);
                            break;
                        },
                    }
                }

                *connected.write() = false;
                warn!("SSE connection closed");
            });
        } else {
            // No SSE endpoint configured, mark as connected for request/response only
            *self.connected.write() = true;
        }
        Ok(())
    }

    async fn send_request(&self, message: &TransportMessage) -> Result<()> {
        let json_bytes = crate::shared::stdio::StdioTransport::serialize_message(message)?;
        let json = String::from_utf8(json_bytes).map_err(|e| {
            crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(format!(
                "Invalid UTF-8: {}",
                e
            )))
        })?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(self.config.base_url.as_str())
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

        let response = timeout(self.config.timeout, self.client.request(req))
            .await
            .map_err(|_| crate::error::Error::Timeout(self.config.timeout.as_secs() * 1000))?
            .map_err(|e| {
                crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(
                    e.to_string(),
                ))
            })?;

        if response.status() != StatusCode::OK {
            return Err(crate::error::Error::Transport(
                crate::error::TransportError::InvalidMessage(format!(
                    "HTTP request failed with status: {}",
                    response.status()
                )),
            ));
        }

        // Collect the response body under this transport's collected-body cap.
        //
        // The PEER chooses how many bytes it sends and this read buffers all of
        // them before parsing, so an uncapped `collect()` here was the one
        // unbounded whole-body read left on this transport — the same defect
        // class 113-17 fixed on the sibling `connect_sse` reader in this very
        // file (review CR-03). See `DEFAULT_HTTP_COLLECTED_BODY_BYTES`.
        let body_bytes = Self::collect_body_within_cap(response, self.max_collected_body_bytes)
            .await
            .map_err(|e| {
                crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(
                    e.to_string(),
                ))
            })?;
        let response_msg = crate::shared::stdio::StdioTransport::parse_message(&body_bytes)?;

        // Send response through message queue
        self.message_tx.send(response_msg).await.map_err(|_| {
            crate::error::Error::Transport(crate::error::TransportError::ConnectionClosed)
        })?;

        Ok(())
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        debug!("Sending HTTP message: {:?}", message);
        self.send_request(&message).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.message_queue.lock().await;
        rx.recv().await.ok_or_else(|| {
            crate::error::Error::Transport(crate::error::TransportError::ConnectionClosed)
        })
    }

    async fn close(&mut self) -> Result<()> {
        *self.connected.write() = false;
        info!("HTTP transport closed");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ClientRequest, Request, RequestId};

    #[test]
    fn test_http_config_default() {
        let config = HttpConfig::default();
        assert!(config.enable_pooling);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.sse_endpoint, Some("/events".to_string()));
        assert_eq!(config.max_idle_per_host, 10);
        assert_eq!(config.headers.len(), 0);
    }

    #[test]
    fn test_http_config_custom() {
        let config = HttpConfig {
            base_url: "http://example.com:3000".parse().unwrap(),
            sse_endpoint: None,
            timeout: Duration::from_mins(1),
            headers: vec![("X-Custom".to_string(), "value".to_string())],
            enable_pooling: false,
            max_idle_per_host: 5,
        };
        assert_eq!(config.base_url.as_str(), "http://example.com:3000/");
        assert!(config.sse_endpoint.is_none());
        assert_eq!(config.timeout, Duration::from_mins(1));
        assert_eq!(config.headers.len(), 1);
        assert!(!config.enable_pooling);
        assert_eq!(config.max_idle_per_host, 5);
    }

    #[test]
    fn test_http_transport_creation() {
        let config = HttpConfig::default();
        let transport = HttpTransport::new(config);
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_http_transport_with_url() {
        let transport =
            HttpTransport::with_url("http://localhost:9000".parse::<Url>().unwrap()).unwrap();
        assert!(!transport.is_connected());
        assert_eq!(transport.config.base_url.as_str(), "http://localhost:9000/");
    }

    #[test]
    fn test_http_transport_debug() {
        let config = HttpConfig::default();
        let transport = HttpTransport::new(config);
        let debug_str = format!("{:?}", transport);
        assert!(debug_str.contains("HttpTransport"));
        assert!(debug_str.contains("config"));
        assert!(debug_str.contains("connected"));
    }

    #[tokio::test]
    async fn test_http_transport_close() {
        let config = HttpConfig::default();
        let mut transport = HttpTransport::new(config);

        // Mark as connected first
        *transport.connected.write() = true;
        assert!(transport.is_connected());

        // Close should mark as disconnected
        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[tokio::test]
    async fn test_connect_sse_no_endpoint() {
        let config = HttpConfig {
            base_url: "http://localhost:8080".parse().unwrap(),
            sse_endpoint: None,
            ..Default::default()
        };
        let transport = HttpTransport::new(config);

        // Should mark as connected even without SSE endpoint
        transport.connect_sse().await.unwrap();
        assert!(transport.is_connected());
    }

    #[tokio::test]
    async fn test_send_request_not_connected() {
        let config = HttpConfig::default();
        let mut transport = HttpTransport::new(config);

        let message = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        // This will fail since we're not connected to a real server
        let result = transport.send(message).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_http_config_with_headers() {
        let config = HttpConfig {
            base_url: "http://localhost:8080".parse().unwrap(),
            headers: vec![
                ("Authorization".to_string(), "Bearer token".to_string()),
                ("X-API-Key".to_string(), "secret".to_string()),
            ],
            ..Default::default()
        };
        assert_eq!(config.headers.len(), 2);
        assert_eq!(config.headers[0].0, "Authorization");
        assert_eq!(config.headers[0].1, "Bearer token");
    }

    #[test]
    fn test_http_config_clone() {
        let config = HttpConfig::default();
        let cloned = config.clone();
        assert_eq!(config.base_url, cloned.base_url);
        assert_eq!(config.timeout, cloned.timeout);
        assert_eq!(config.enable_pooling, cloned.enable_pooling);
    }

    /// One SSE frame of exactly `len` bytes, carrying a complete event.
    fn sse_frame_of_len(len: usize) -> String {
        // "data: " + payload + "\n\n" — the 8 fixed bytes of framing. Asserted
        // rather than subtracted blind: `len - 8` underflow-PANICS for any
        // caller that picks a smaller ceiling, which would report a bound bug as
        // an arithmetic crash inside the helper.
        assert!(
            len >= 8,
            "an SSE frame cannot be shorter than its 8 bytes of framing (asked for {len})"
        );
        format!("data: {}\n\n", "A".repeat(len - 8))
    }

    /// The reader task's overflow arm, exercised on the predicate it actually
    /// calls. A deliberately tiny parser stands in for the 16 MiB production
    /// ceiling so the test allocates bytes rather than megabytes.
    #[test]
    fn an_oversized_sse_line_ends_the_reader_task() {
        let mut parser = SseParser::with_max_buffer_size(64);
        assert!(
            !report_sse_overflow(&parser),
            "a fresh parser has lost nothing, so the task keeps reading"
        );

        assert!(
            parser.feed(&"x".repeat(256)).is_empty(),
            "an unterminated line completes no event"
        );
        assert!(
            report_sse_overflow(&parser),
            "the discarded bytes end the task instead of being silently swallowed"
        );
    }

    /// The realistic flood, on the input class every other bound test in this
    /// module avoids (review IN-03): perfectly ordinary NEWLINE-TERMINATED
    /// `data:` lines that the peer simply never ends with a blank line.
    ///
    /// Drives `report_sse_overflow`, the predicate the reader task calls, not a
    /// reconstruction of it.
    #[test]
    fn a_newline_carrying_flood_ends_the_reader_task_too() {
        let mut parser = sse_reader_parser(64);
        let mut ended = false;
        for _ in 0..1_000 {
            assert!(
                parser.feed("data: AAAAAAAA\n").is_empty(),
                "a `data:` line with no blank line after it completes no event"
            );
            if report_sse_overflow(&parser) {
                ended = true;
                break;
            }
        }
        assert!(ended, "accumulated `data:` lines must end the reader task");
    }

    /// `connect_sse` bounds its reader at its OWN named, configurable ceiling.
    ///
    /// The tripwire that used to guard "this site keeps the shared 1 MiB
    /// default" now guards the named constant instead: it asserts on
    /// `sse_reader_parser`, the function the reader task actually calls, and on
    /// the value `HttpTransport` actually passes it, so ANY future change to
    /// either still fails here.
    #[test]
    fn connect_sse_uses_its_own_named_bound() {
        let transport = HttpTransport::new(HttpConfig::default());
        assert_eq!(
            transport.sse_buffered_bytes, DEFAULT_HTTP_SSE_BUFFERED_BYTES,
            "the transport defaults its ceiling from the named constant"
        );

        let mut parser = sse_reader_parser(transport.sse_buffered_bytes);
        assert_eq!(parser.max_buffer_size(), DEFAULT_HTTP_SSE_BUFFERED_BYTES);
        let _ = parser.feed(&"x".repeat(256));
        assert!(
            !report_sse_overflow(&parser),
            "256 bytes is nowhere near the {DEFAULT_HTTP_SSE_BUFFERED_BYTES}-byte default"
        );
    }

    /// Where the ceiling cuts, pinned on both sides and ON it — the comparison
    /// is `>`, so a payload of EXACTLY the ceiling is admitted (review HIGH-4).
    ///
    /// Uses a small configured ceiling so the test costs bytes rather than the
    /// 16 MiB the production default would.
    #[test]
    fn the_configured_ceiling_admits_up_to_and_including_itself() {
        let ceiling = 256;

        let mut under = sse_reader_parser(ceiling);
        let events = under.feed(&sse_frame_of_len(ceiling - 1));
        assert_eq!(events.len(), 1, "one byte under the ceiling parses");
        assert!(!under.overflowed());

        let mut exact = sse_reader_parser(ceiling);
        let events = exact.feed(&sse_frame_of_len(ceiling));
        assert_eq!(events.len(), 1, "exactly the ceiling parses");
        assert!(!exact.overflowed(), "the comparison is `>`, not `>=`");

        let mut over = sse_reader_parser(ceiling);
        assert!(
            over.feed(&sse_frame_of_len(ceiling + 1)).is_empty(),
            "one byte over the ceiling is refused whole"
        );
        assert!(over.overflowed(), "and the refusal is observable");
        assert!(report_sse_overflow(&over), "so the reader task ends");
    }

    /// The escape hatch is WIRED, not decorative: the same payload that the
    /// lower ceiling refuses parses once the ceiling is raised through the
    /// public builder method, and the raised value reaches the reader's parser.
    ///
    /// Expressed at a scaled-down ceiling rather than at the 16 MiB default so
    /// the test does not allocate 16 MiB to prove a wiring property.
    #[test]
    fn raising_the_ceiling_admits_a_payload_the_lower_one_refuses() {
        let base = 256;
        let payload = sse_frame_of_len(base + 1);

        let mut low = sse_reader_parser(base);
        assert!(low.feed(&payload).is_empty());
        assert!(report_sse_overflow(&low), "refused at the lower ceiling");

        let raised = HttpTransport::new(HttpConfig::default()).with_sse_buffered_bytes(base * 4);
        assert_eq!(
            raised.sse_buffered_bytes,
            base * 4,
            "the builder overrides the default"
        );

        let mut parser = sse_reader_parser(raised.sse_buffered_bytes);
        let events = parser.feed(&payload);
        assert_eq!(events.len(), 1, "the same bytes now parse");
        assert!(!report_sse_overflow(&parser));
    }

    /// base64 expands by ~4/3, which is exactly why a FIXED 16 MiB ceiling is
    /// indefensible and why "16 MiB comfortably fits a 12 MiB image" is false.
    ///
    /// Scaled down by 2^10 from the real numbers — 12 KiB of raw binary against
    /// a 16 KiB ceiling stands in for 12 MiB against 16 MiB — so the arithmetic
    /// is identical and the test allocates kilobytes. A future reader who wants
    /// to re-introduce the "media is unaffected" claim has to delete this first.
    #[test]
    fn base64_expansion_puts_a_12_to_16_binary_over_the_ceiling() {
        use base64::Engine as _;

        let raw_len = 12 * 1024;
        let ceiling = 16 * 1024;

        let encoded = base64::engine::general_purpose::STANDARD.encode(vec![0u8; raw_len]);

        // The ~4/3 expansion, asserted rather than assumed: 3 raw bytes become
        // 4 encoded characters, rounded up to a whole group.
        assert_eq!(
            encoded.len(),
            raw_len.div_ceil(3) * 4,
            "base64 expands 3 raw bytes into 4"
        );
        assert_eq!(
            encoded.len(),
            ceiling,
            "a '12 MiB' binary is ALREADY the whole '16 MiB' ceiling once encoded, \
             with nothing left for JSON, the `data: ` prefix or the MIME type"
        );

        // And so the SSE framing alone pushes it over.
        let frame = format!("data: {encoded}\n\n");
        assert!(frame.len() > ceiling, "the envelope is what tips it");

        let mut parser = sse_reader_parser(ceiling);
        assert!(
            parser.feed(&frame).is_empty(),
            "so the payload is refused at a ceiling sized for its RAW bytes"
        );
        assert!(parser.overflowed());
    }

    #[tokio::test]
    async fn test_message_queue_receive_closed() {
        let config = HttpConfig::default();
        let transport = HttpTransport::new(config);

        // Create a new receiver that's already closed
        let (_, rx) = mpsc::channel::<TransportMessage>(1);
        let mut transport = HttpTransport {
            config: transport.config,
            client: transport.client,
            message_queue: Arc::new(AsyncMutex::new(rx)),
            message_tx: transport.message_tx,
            connected: transport.connected,
            sse_buffered_bytes: transport.sse_buffered_bytes,
            max_collected_body_bytes: transport.max_collected_body_bytes,
        };

        // Receive should error with ConnectionClosed
        let result = transport.receive().await;
        assert!(result.is_err());
        if let Err(crate::error::Error::Transport(e)) = result {
            assert!(matches!(e, crate::error::TransportError::ConnectionClosed));
        }
    }
}
