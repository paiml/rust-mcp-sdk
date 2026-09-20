// Why: `OptimizedSseTransport` is deprecated ON PURPOSE (plan 113.1-03, D-01)
// and is still SHIPPED for 2.x compatibility, so the crate must go on compiling
// its own retained transport. The `deprecated` lint fires on uses within the
// defining crate — including every `impl` block on the type — and `make lint`
// runs clippy with `-D warnings`, so without this the crate cannot build itself.
// Module-level rather than per-item: three `impl` blocks plus the test module
// name the type. The trade-off is that this also silences unrelated future
// deprecation warnings in this file — acceptable only because the whole module
// is slated for removal at 3.0.
#![allow(deprecated)]

//! Optimized SSE transport with advanced features.
//!
//! **DEPRECATED** — use
//! [`StreamableHttpTransport`](crate::shared::streamable_http::StreamableHttpTransport)
//! for new code. It bounds every peer-controlled read and carries a configurable
//! cap. This module is retained for 2.x compatibility only; retiring it removes
//! public items and is therefore a 3.0 action.
//!
//! PMCP-4002: High-performance SSE implementation with:
//! - Connection pooling and reuse
//! - Keep-alive mechanisms
//! - Streaming optimizations
//! - Buffered writing
//! - Event coalescing

use crate::error::{Error, Result};
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use bytes::BytesMut;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

/// Configuration for optimized SSE transport
#[derive(Debug, Clone)]
pub struct OptimizedSseConfig {
    /// Base URL for SSE endpoint
    pub url: String,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keepalive_interval: Duration,
    /// Maximum reconnect attempts
    pub max_reconnects: usize,
    /// Reconnect delay
    pub reconnect_delay: Duration,
    /// Buffer size for event coalescing
    pub buffer_size: usize,
    /// Flush interval for buffered events
    pub flush_interval: Duration,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Enable event compression
    pub enable_compression: bool,
}

impl Default for OptimizedSseConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080/sse".to_string(),
            connection_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(15),
            max_reconnects: 5,
            reconnect_delay: Duration::from_secs(1),
            buffer_size: 100,
            flush_interval: Duration::from_millis(100),
            enable_pooling: true,
            max_connections: 10,
            enable_compression: false,
        }
    }
}

/// Connection state for SSE
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// Optimized SSE transport implementation
#[deprecated(
    since = "2.18.0",
    note = "Use StreamableHttpTransport, which bounds every peer-controlled read; OptimizedSseTransport is retained for 2.x compatibility only"
)]
pub struct OptimizedSseTransport {
    config: OptimizedSseConfig,
    client: reqwest::Client,
    state: Arc<RwLock<ConnectionState>>,
    event_buffer: Arc<RwLock<VecDeque<TransportMessage>>>,
    send_tx: mpsc::Sender<TransportMessage>,
    recv_rx: Arc<RwLock<mpsc::Receiver<TransportMessage>>>,
    reconnect_count: Arc<RwLock<usize>>,
    last_event_id: Arc<RwLock<Option<String>>>,
}

impl std::fmt::Debug for OptimizedSseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OptimizedSseTransport")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("reconnect_count", &self.reconnect_count)
            .finish()
    }
}

impl OptimizedSseTransport {
    /// Create new optimized SSE transport
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(config: OptimizedSseConfig) -> Self {
        let (send_tx, send_rx) = mpsc::channel(config.buffer_size);
        let (recv_tx, recv_rx) = mpsc::channel(config.buffer_size);

        let client = reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(config.max_connections)
            .tcp_keepalive(Some(Duration::from_mins(1)))
            .timeout(config.connection_timeout)
            .build()
            .expect("Failed to build HTTP client");

        let transport = Self {
            config: config.clone(),
            client,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            event_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.buffer_size))),
            send_tx,
            recv_rx: Arc::new(RwLock::new(recv_rx)),
            reconnect_count: Arc::new(RwLock::new(0)),
            last_event_id: Arc::new(RwLock::new(None)),
        };

        // Start background tasks
        transport.start_background_tasks(send_rx, recv_tx);

        transport
    }

    /// Start background tasks for SSE handling
    fn start_background_tasks(
        &self,
        mut send_rx: mpsc::Receiver<TransportMessage>,
        recv_tx: mpsc::Sender<TransportMessage>,
    ) {
        let config = self.config.clone();
        let config2 = self.config.clone();
        let config3 = self.config.clone();
        let client = self.client.clone();
        let client2 = self.client.clone();
        let client3 = self.client.clone();
        let state = self.state.clone();
        let state2 = self.state.clone();
        let state3 = self.state.clone();
        let _event_buffer = self.event_buffer.clone();
        let event_buffer2 = self.event_buffer.clone();
        let reconnect_count = self.reconnect_count.clone();
        let last_event_id = self.last_event_id.clone();

        // Spawn SSE connection handler
        tokio::spawn(async move {
            loop {
                match Self::connect_sse(&config, &client, &state, &recv_tx, &last_event_id).await {
                    Ok(()) => {
                        info!("SSE connection closed normally");
                        *reconnect_count.write().await = 0;
                    },
                    Err(e) => {
                        error!("SSE connection error: {}", e);
                        let mut count = reconnect_count.write().await;
                        *count += 1;

                        if *count >= config.max_reconnects {
                            error!("Max reconnect attempts reached");
                            break;
                        }

                        *state.write().await = ConnectionState::Reconnecting;
                        tokio::time::sleep(config.reconnect_delay).await;
                    },
                }
            }
        });

        // Spawn event sender task

        tokio::spawn(async move {
            let mut flush_ticker = interval(config2.flush_interval);

            loop {
                tokio::select! {
                    Some(msg) = send_rx.recv() => {
                        event_buffer2.write().await.push_back(msg);

                        // Flush if buffer is full
                        if event_buffer2.read().await.len() >= config2.buffer_size {
                            Self::flush_events(
                                &event_buffer2,
                                &client2,
                                &config2,
                                &state2,
                            ).await;
                        }
                    }
                    _ = flush_ticker.tick() => {
                        // Periodic flush
                        if !event_buffer2.read().await.is_empty() {
                            Self::flush_events(
                                &event_buffer2,
                                &client2,
                                &config2,
                                &state2,
                            ).await;
                        }
                    }
                }
            }
        });

        // Spawn keepalive task
        tokio::spawn(async move {
            let mut ticker = interval(config3.keepalive_interval);

            loop {
                ticker.tick().await;

                if *state3.read().await == ConnectionState::Connected {
                    // Send keepalive ping
                    if let Err(e) = Self::send_keepalive(&client3, &config3).await {
                        warn!("Keepalive failed: {}", e);
                    }
                }
            }
        });
    }

    /// Collect a response body as text, refusing anything over `max_bytes`.
    ///
    /// The ONE place this transport turns a peer-controlled response into an
    /// in-memory buffer. Two independently-sufficient refusals, the same doctrine
    /// [`crate::shared::streamable_http`]'s `collect_body_within_cap` applies:
    ///
    /// 1. A declared `Content-Length` over the cap is refused before a single
    ///    body byte is read. The header is a peer-controlled OPTIMISATION, never
    ///    the authority.
    /// 2. The bytes actually delivered are accumulated through
    ///    [`reqwest::Response::chunk`] with a running total checked BEFORE each
    ///    append, so the read stops mid-flight. A peer that understates or omits
    ///    `Content-Length` therefore gains nothing (T-113-93), and the allocation
    ///    is bounded DURING the read rather than measured after it.
    ///
    /// A body of exactly `max_bytes` is ADMITTED; one byte over is refused.
    ///
    /// `max_bytes` is a parameter so the tests can drive a small cap and cost
    /// bytes rather than megabytes. Production has exactly ONE call site, and it
    /// passes [`crate::shared::http_constants::DEFAULT_HTTP_SSE_BUFFERED_BYTES`].
    ///
    /// # Why `chunk()` and not `bytes_stream()`
    ///
    /// `Response::chunk` carries no `cfg`, while `bytes_stream` is behind
    /// `#[cfg(feature = "stream")]`, which this crate does not enable
    /// (`Cargo.toml` pins reqwest with `default-features = false`, features
    /// `["json", "rustls", "form"]`). Accumulating through `chunk()` therefore
    /// costs zero dependency-surface change (D-02).
    ///
    /// Decoding is `String::from_utf8_lossy`, matching the lossy tolerance the
    /// `.text()` call this replaced already had — so this change is a bound and
    /// not also a strictness change.
    ///
    /// Added in plan 113.1-03 (D-113-Q): the previous `response.text().await`
    /// accepted no limit argument, so a remote peer chose the allocation.
    async fn collect_sse_text_within_cap(
        mut response: reqwest::Response,
        max_bytes: usize,
    ) -> Result<String> {
        // Refusal 1 — advisory, and only ever an early exit.
        if let Some(declared) = response.content_length() {
            if declared > max_bytes as u64 {
                return Err(Self::sse_body_over_cap(max_bytes, Some(declared)));
            }
        }

        // Refusal 2 — authoritative, over the bytes actually delivered.
        let mut accumulated: Vec<u8> = Vec::new();
        loop {
            let next = response.chunk().await;
            let Some(chunk) =
                next.map_err(|e| Error::internal(format!("SSE body read failed: {}", e)))?
            else {
                break;
            };
            // Overflow-safe by construction: `accumulated.len() <= max_bytes` is
            // the loop invariant, so `max_bytes - accumulated.len()` cannot
            // underflow, and no unguarded `a + b` is ever computed.
            if chunk.len() > max_bytes - accumulated.len() {
                return Err(Self::sse_body_over_cap(max_bytes, None));
            }
            accumulated.extend_from_slice(&chunk);
        }

        // `from_utf8` REUSES the Vec's allocation on the valid-UTF-8 path, which
        // is every real SSE body. The obvious
        // `String::from_utf8_lossy(&accumulated).into_owned()` returns a
        // `Cow::Borrowed` there and `into_owned()` then copies the whole buffer —
        // a second 16 MiB allocation and memcpy at the production cap, with both
        // copies live at once, so a function whose job is to bound in-flight
        // bytes at 16 MiB would transiently hold 32. The lossy fallback keeps
        // the tolerance `.text()` had for invalid bytes.
        Ok(String::from_utf8(accumulated)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
    }

    /// Build the over-cap refusal for [`Self::collect_sse_text_within_cap`].
    ///
    /// Names the LIMIT and the observed size, and deliberately echoes no body
    /// content: the refusal must not become a channel for the very bytes it
    /// refused. `declared` is `Some` only when the peer's `Content-Length` was
    /// itself over the cap; when the peer understated or omitted it the read is
    /// stopped mid-flight and no total is knowable, so the message says so rather
    /// than inventing one.
    ///
    /// Uses [`Error::internal`], the family `connect_sse`'s other four failure
    /// sites already use, so a caller matching on the error family sees no new
    /// shape.
    fn sse_body_over_cap(max_bytes: usize, declared: Option<u64>) -> Error {
        let observed = match declared {
            Some(bytes) => format!("declares Content-Length {bytes}"),
            None => {
                "delivered more than the cap (Content-Length absent or understated)".to_string()
            },
        };
        Error::internal(format!(
            "SSE response body {observed}, over the {max_bytes}-byte SSE buffered-bytes cap \
             (DEFAULT_HTTP_SSE_BUFFERED_BYTES); OptimizedSseTransport is deprecated — use \
             StreamableHttpTransport, which carries a configurable cap"
        ))
    }

    /// Connect to SSE endpoint
    async fn connect_sse(
        config: &OptimizedSseConfig,
        client: &reqwest::Client,
        state: &Arc<RwLock<ConnectionState>>,
        recv_tx: &mpsc::Sender<TransportMessage>,
        last_event_id: &Arc<RwLock<Option<String>>>,
    ) -> Result<()> {
        *state.write().await = ConnectionState::Connecting;

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));

        // Add Last-Event-ID header if we have one
        if let Some(ref id) = *last_event_id.read().await {
            headers.insert(
                "Last-Event-ID",
                HeaderValue::from_str(id).unwrap_or_else(|_| HeaderValue::from_static("0")),
            );
        }

        let response = timeout(
            config.connection_timeout,
            client.get(&config.url).headers(headers).send(),
        )
        .await
        .map_err(|_| Error::internal("SSE connection timeout"))?
        .map_err(|e| Error::internal(format!("SSE connection failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "SSE connection failed with status: {}",
                response.status()
            )));
        }

        *state.write().await = ConnectionState::Connected;
        info!("SSE connection established");

        // Process event stream - simplified for now
        // In a real implementation, this would use eventsource or similar
        //
        // The read is BOUNDED (D-113-Q, plan 113.1-03): it used to be
        // `response.text().await`, which accepts no limit argument, so the peer
        // chose the allocation. `collect_sse_text_within_cap` applies the crate's
        // single SSE ceiling through a running total.
        match Self::collect_sse_text_within_cap(
            response,
            crate::shared::http_constants::DEFAULT_HTTP_SSE_BUFFERED_BYTES,
        )
        .await
        {
            Ok(text) => {
                // Parse SSE events from text
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(msg) = serde_json::from_str::<TransportMessage>(data) {
                            if let Err(e) = recv_tx.send(msg).await {
                                error!("Failed to queue received message: {}", e);
                                return Err(Error::internal("Receiver channel closed"));
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Response error: {}", e);
                return Err(Error::internal("Response error"));
            },
        }

        *state.write().await = ConnectionState::Disconnected;
        Ok(())
    }

    /// Parse SSE event from buffer
    #[allow(dead_code, clippy::unnecessary_wraps)]
    fn parse_sse_event(buffer: &mut BytesMut) -> Result<Option<SseEvent>> {
        // Look for double newline (event boundary)
        if let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
            let event_data = buffer.split_to(pos + 2);
            let event_str = String::from_utf8_lossy(&event_data);

            let mut event = SseEvent::default();

            for line in event_str.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    event.data.push_str(data);
                    event.data.push('\n');
                } else if let Some(event_type) = line.strip_prefix("event: ") {
                    event.event = Some(event_type.to_string());
                } else if let Some(id) = line.strip_prefix("id: ") {
                    event.id = Some(id.to_string());
                } else if let Some(retry) = line.strip_prefix("retry: ") {
                    if let Ok(ms) = retry.parse::<u64>() {
                        event.retry = Some(Duration::from_millis(ms));
                    }
                }
            }

            // Trim trailing newline from data
            if event.data.ends_with('\n') {
                event.data.pop();
            }

            if !event.data.is_empty() {
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    /// Parse `TransportMessage` from SSE event
    #[allow(dead_code, clippy::unnecessary_wraps)]
    fn parse_message(event: &SseEvent) -> Result<Option<TransportMessage>> {
        if event.data.is_empty() {
            return Ok(None);
        }

        match serde_json::from_str::<TransportMessage>(&event.data) {
            Ok(msg) => Ok(Some(msg)),
            Err(e) => {
                warn!("Failed to parse SSE message: {}", e);
                Ok(None)
            },
        }
    }

    /// Flush buffered events
    async fn flush_events(
        buffer: &Arc<RwLock<VecDeque<TransportMessage>>>,
        client: &reqwest::Client,
        config: &OptimizedSseConfig,
        state: &Arc<RwLock<ConnectionState>>,
    ) {
        if *state.read().await != ConnectionState::Connected {
            return;
        }

        let mut events = buffer.write().await;
        if events.is_empty() {
            return;
        }

        // Batch events for sending
        let batch: Vec<TransportMessage> = events.drain(..).collect();

        // Send batch
        for msg in batch {
            if let Err(e) = Self::send_event(client, config, &msg).await {
                error!("Failed to send event: {}", e);
                // Re-queue failed message
                events.push_back(msg);
            }
        }
    }

    /// Send single event
    async fn send_event(
        client: &reqwest::Client,
        config: &OptimizedSseConfig,
        msg: &TransportMessage,
    ) -> Result<()> {
        let json = serde_json::to_string(msg)
            .map_err(|e| Error::internal(format!("Failed to serialize message: {}", e)))?;

        let response = client
            .post(&config.url)
            .header(CONTENT_TYPE, "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to send event: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Event send failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Send keepalive ping
    async fn send_keepalive(client: &reqwest::Client, config: &OptimizedSseConfig) -> Result<()> {
        let response = client
            .get(format!("{}/ping", config.url))
            .send()
            .await
            .map_err(|e| Error::internal(format!("Keepalive failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal("Keepalive ping failed"));
        }

        debug!("Keepalive ping successful");
        Ok(())
    }
}

/// SSE event structure
#[derive(Debug, Default)]
#[allow(dead_code)]
struct SseEvent {
    data: String,
    event: Option<String>,
    id: Option<String>,
    retry: Option<Duration>,
}

#[async_trait]
impl Transport for OptimizedSseTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.send_tx
            .send(message)
            .await
            .map_err(|_| Error::internal("Send channel closed"))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.recv_rx.write().await;
        rx.recv()
            .await
            .ok_or_else(|| Error::internal("Receive channel closed"))
    }

    async fn close(&mut self) -> Result<()> {
        *self.state.write().await = ConnectionState::Disconnected;
        info!("SSE transport closed");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        futures::executor::block_on(async {
            *self.state.read().await == ConnectionState::Connected
        })
    }

    fn transport_type(&self) -> &'static str {
        "sse-optimized"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = OptimizedSseConfig::default();
        assert_eq!(config.buffer_size, 100);
        assert_eq!(config.max_connections, 10);
        assert!(config.enable_pooling);
    }

    #[test]
    fn test_sse_event_parsing() {
        use bytes::BufMut;
        let mut buffer = BytesMut::new();
        buffer.put(&b"data: test message\nid: 123\n\n"[..]);

        let event = OptimizedSseTransport::parse_sse_event(&mut buffer)
            .unwrap()
            .unwrap();

        assert_eq!(event.data, "test message");
        assert_eq!(event.id, Some("123".to_string()));
    }

    // ------------------------------------------------------------------
    // D-113-Q: the bounded whole-body read (plan 113.1-03).
    //
    // A small cap so the tests cost BYTES, not megabytes — which is exactly
    // why `collect_sse_text_within_cap` takes `max_bytes` as a parameter
    // rather than reading the 16 MiB constant directly. Mirrors
    // `streamable_http.rs`'s `const CAP: usize = 512;` and its over/at pair.
    // ------------------------------------------------------------------

    /// The cap under test. 512 bytes, not 16 MiB.
    const CAP: usize = 512;

    /// A distinctive token the refusal must NOT echo back.
    const FILLER: &str = "pppppppp";

    /// Assert the refusal names the limit and leaks no body content.
    fn assert_over_cap_refusal(error: &Error, cap: usize) {
        let text = error.to_string();
        assert!(
            text.contains(&cap.to_string()),
            "the refusal must NAME the limit: {text}"
        );
        assert!(
            !text.contains(FILLER) && !text.contains("jsonrpc"),
            "the refusal must not echo body content: {text}"
        );
    }

    /// Build a body of exactly `bytes` bytes made of the filler token.
    ///
    /// `FILLER` is a run of one repeated character, so repeating that character
    /// gives byte-identical output to repeat-then-truncate while allocating
    /// once — and every window of it still contains `FILLER`, which is what
    /// [`assert_over_cap_refusal`]'s "no body content echoed" check relies on.
    fn filler_body(bytes: usize) -> String {
        debug_assert!(
            FILLER.as_bytes().iter().all(|b| *b == FILLER.as_bytes()[0]),
            "filler_body's single-char shortcut assumes FILLER is one repeated byte"
        );
        String::from_utf8(vec![FILLER.as_bytes()[0]; bytes]).expect("FILLER is ASCII")
    }

    /// One byte over the cap is refused — with NO `Content-Length` at all.
    ///
    /// `with_chunked_body` means the advisory header path cannot be what
    /// produces the pass: only the authoritative running total over delivered
    /// bytes can refuse this body (T-113-93).
    #[tokio::test]
    async fn connect_sse_one_byte_over_the_cap_is_refused() {
        let mut server = mockito::Server::new_async().await;
        let body = filler_body(CAP + 1);
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_chunked_body(move |w| w.write_all(body.as_bytes()))
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let response = client.get(server.url()).send().await.unwrap();
        let error = OptimizedSseTransport::collect_sse_text_within_cap(response, CAP)
            .await
            .expect_err("a body one byte over the cap must be refused");
        assert_over_cap_refusal(&error, CAP);
    }

    /// A body of EXACTLY the cap is admitted, and its content survives.
    ///
    /// This is what pins the comparison as `>` and not `>=`, and it also
    /// prevents a "fast because it read less" regression: the returned text
    /// must still carry the `data:` line.
    #[tokio::test]
    async fn connect_sse_at_exactly_the_cap_is_admitted() {
        let mut server = mockito::Server::new_async().await;
        // A real `data:` line, padded out to exactly CAP bytes.
        let line = r#"data: {"jsonrpc":"2.0","method":"x","params":{}}"#;
        let mut body = format!("{line}\n");
        body.push_str(&filler_body(CAP - body.len()));
        assert_eq!(body.len(), CAP, "the fixture must be exactly at the cap");

        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_chunked_body(move |w| w.write_all(body.as_bytes()))
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let response = client.get(server.url()).send().await.unwrap();
        let text = OptimizedSseTransport::collect_sse_text_within_cap(response, CAP)
            .await
            .expect("a body of exactly the cap must be admitted");
        assert_eq!(text.len(), CAP, "every admitted byte is returned");
        assert!(
            text.contains(line),
            "the admitted body still carries its data line: {text:?}"
        );
    }

    /// A DECLARED `Content-Length` over the cap is refused before the body is
    /// read.
    ///
    /// `mockito`'s default `.with_body(..)` sets a real `Content-Length`, so
    /// 513 bytes against a 512-byte cap exercises the early-refusal branch
    /// exactly. No 16 MiB transfer is needed — the branch is about the header
    /// value versus `max_bytes`.
    #[tokio::test]
    async fn declared_content_length_over_the_cap_is_refused_without_reading_the_body() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(filler_body(CAP + 1))
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let response = client.get(server.url()).send().await.unwrap();
        assert_eq!(
            response.content_length(),
            Some((CAP + 1) as u64),
            "this test is only meaningful if the peer actually declared a length"
        );
        let error = OptimizedSseTransport::collect_sse_text_within_cap(response, CAP)
            .await
            .expect_err("a declared Content-Length over the cap must be refused");
        assert_over_cap_refusal(&error, CAP);
    }

    /// The WIRING test: an under-cap body still flows through `connect_sse`
    /// itself into `recv_tx`.
    ///
    /// The three tests above exercise the collector in isolation; none of them
    /// proves that an ADMITTED body still reaches the channel. This is the one
    /// that would catch the cap being wired in a way that swallows the body.
    #[tokio::test]
    async fn connect_sse_under_the_cap_still_delivers_a_message_to_recv_tx() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body("data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n\n")
            .create_async()
            .await;

        let config = OptimizedSseConfig {
            url: server.url(),
            ..Default::default()
        };
        let client = reqwest::Client::new();
        let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        let (tx, mut rx) = mpsc::channel(4);
        let last_event_id = Arc::new(RwLock::new(None));

        OptimizedSseTransport::connect_sse(&config, &client, &state, &tx, &last_event_id)
            .await
            .expect("an under-cap body must be served normally");

        let message = rx
            .try_recv()
            .expect("the admitted body's data line must reach recv_tx");
        assert!(
            matches!(message, TransportMessage::Notification(_)),
            "the delivered message is the notification the body carried: {message:?}"
        );
    }
}
