//! Transport layer abstraction for MCP.
//!
//! This module defines the core `Transport` trait that all transport
//! implementations must satisfy.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// A message that can be sent/received over a transport.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::TransportMessage;
/// use pmcp::types::{Request, RequestId, JSONRPCResponse, Notification, ProgressNotification, ProgressToken, ClientRequest};
///
/// // Create a request message
/// let request_msg = TransportMessage::Request {
///     id: RequestId::from(1i64),
///     request: Request::Client(Box::new(ClientRequest::Ping)),
/// };
///
/// // Create a response message
/// let response = JSONRPCResponse {
///     jsonrpc: "2.0".to_string(),
///     id: RequestId::from(1i64),
///     payload: pmcp::types::jsonrpc::ResponsePayload::Result(
///         serde_json::json!({"status": "ok"})
///     ),
/// };
/// let response_msg = TransportMessage::Response(response);
///
/// // Create a notification message
/// let notification = Notification::Progress(ProgressNotification::new(
///     ProgressToken::String("task-123".to_string()),
///     75.0,
///     Some("Processing nearly complete".to_string()),
/// ));
/// let notification_msg = TransportMessage::Notification(notification);
///
/// // Pattern matching on messages
/// match request_msg {
///     TransportMessage::Request { id, request } => {
///         println!("Received request with ID {:?}", id);
///         match &request {
///             Request::Client(client_req) => {
///                 println!("Client request: {:?}", client_req);
///             }
///             Request::Server(server_req) => {
///                 println!("Server request: {:?}", server_req);
///             }
///         }
///     }
///     TransportMessage::Response(resp) => {
///         println!("Received response for request {:?}", resp.id);
///     }
///     TransportMessage::Notification(notif) => {
///         println!("Received notification");
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransportMessage {
    /// Request message with ID
    Request {
        /// Request ID
        id: crate::types::RequestId,
        /// Request payload
        request: crate::types::Request,
    },
    /// Response message
    Response(crate::types::JSONRPCResponse),
    /// Notification message
    Notification(crate::types::Notification),
}

/// Serialize a [`TransportMessage`] into a JSON-RPC 2.0 wire frame.
///
/// This is the single source of truth for the on-the-wire encoding shared by
/// every transport (stdio, native HTTP, WASM Fetch, WebSocket). A `Request` is
/// flattened into a proper `{"jsonrpc":"2.0","id":…,"method":…,"params":…}`
/// frame via [`create_request`](crate::shared::create_request) — serializing the
/// untagged `TransportMessage` enum directly would instead emit
/// `{"id":…,"request":…}`, which no MCP server can parse ("Unknown message type").
pub fn serialize_message(message: &TransportMessage) -> Result<Vec<u8>> {
    use crate::error::TransportError;
    match message {
        TransportMessage::Request { id, request } => {
            let jsonrpc_request = crate::shared::create_request(id.clone(), request.clone());
            serde_json::to_vec(&jsonrpc_request).map_err(|e| {
                TransportError::InvalidMessage(format!("Failed to serialize request: {}", e)).into()
            })
        },
        TransportMessage::Response(response) => serde_json::to_vec(response).map_err(|e| {
            TransportError::InvalidMessage(format!("Failed to serialize response: {}", e)).into()
        }),
        TransportMessage::Notification(notification) => {
            let jsonrpc_notification = crate::shared::create_notification(notification.clone());
            serde_json::to_vec(&jsonrpc_notification).map_err(|e| {
                TransportError::InvalidMessage(format!("Failed to serialize notification: {}", e))
                    .into()
            })
        },
    }
}

/// Parse a JSON-RPC 2.0 wire frame into a [`TransportMessage`].
///
/// Classifies the frame as a request, notification, or response by inspecting the
/// `method`/`result`/`error` fields. Mirror of [`serialize_message`]; shared by all
/// transports.
pub fn parse_message(buffer: &[u8]) -> Result<TransportMessage> {
    use crate::error::TransportError;
    let json_value: serde_json::Value = serde_json::from_slice(buffer)
        .map_err(|e| TransportError::InvalidMessage(format!("Invalid JSON: {}", e)))?;

    if json_value.get("method").is_some() {
        parse_method_message(json_value)
    } else if json_value.get("result").is_some() || json_value.get("error").is_some() {
        parse_response_message(json_value)
    } else {
        Err(TransportError::InvalidMessage("Unknown message type".to_string()).into())
    }
}

/// Parse a frame that carries a `method` field (request when `id` is present,
/// otherwise a notification).
fn parse_method_message(json_value: serde_json::Value) -> Result<TransportMessage> {
    use crate::error::TransportError;
    if json_value.get("id").is_some() {
        let request: crate::types::JSONRPCRequest<serde_json::Value> =
            serde_json::from_value(json_value)
                .map_err(|e| TransportError::InvalidMessage(format!("Invalid request: {}", e)))?;

        let parsed_request = crate::shared::parse_request(request)
            .map_err(|e| TransportError::InvalidMessage(format!("Invalid request: {}", e)))?;

        Ok(TransportMessage::Request {
            id: parsed_request.0,
            request: parsed_request.1,
        })
    } else {
        let parsed_notification = crate::shared::parse_notification(json_value)
            .map_err(|e| TransportError::InvalidMessage(format!("Invalid notification: {}", e)))?;

        Ok(TransportMessage::Notification(parsed_notification))
    }
}

/// Parse a frame that carries a `result`/`error` field into a response.
fn parse_response_message(json_value: serde_json::Value) -> Result<TransportMessage> {
    use crate::error::TransportError;
    let response: crate::types::JSONRPCResponse = serde_json::from_value(json_value)
        .map_err(|e| TransportError::InvalidMessage(format!("Invalid response: {}", e)))?;

    Ok(TransportMessage::Response(response))
}

/// Metadata associated with a transport message.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::{MessageMetadata, MessagePriority};
///
/// // Create default metadata
/// let default_meta = MessageMetadata::default();
/// assert!(default_meta.content_type.is_none());
/// assert!(!default_meta.flush);
///
/// // Create metadata with specific settings
/// let meta = MessageMetadata {
///     content_type: Some("application/json".to_string()),
///     priority: Some(MessagePriority::High),
///     flush: true,
/// };
///
/// // Use in transport implementations
/// fn should_flush_immediately(meta: &MessageMetadata) -> bool {
///     meta.flush || matches!(meta.priority, Some(MessagePriority::High))
/// }
///
/// assert!(should_flush_immediately(&meta));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
    /// Content type (e.g., "application/json")
    pub content_type: Option<String>,
    /// Message priority
    pub priority: Option<MessagePriority>,
    /// Whether this message should be flushed immediately
    pub flush: bool,
}

/// Message priority levels.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::MessagePriority;
///
/// // Priority levels are ordered
/// assert!(MessagePriority::Low < MessagePriority::Normal);
/// assert!(MessagePriority::Normal < MessagePriority::High);
///
/// // Default is Normal
/// let default_priority = MessagePriority::default();
/// assert_eq!(default_priority, MessagePriority::Normal);
///
/// // Use in message queue prioritization
/// let mut messages = vec![
///     ("msg1", MessagePriority::Low),
///     ("msg2", MessagePriority::High),
///     ("msg3", MessagePriority::Normal),
/// ];
///
/// // Sort by priority (highest first)
/// messages.sort_by_key(|(_, priority)| std::cmp::Reverse(*priority));
/// assert_eq!(messages[0].0, "msg2"); // High priority first
/// assert_eq!(messages[1].0, "msg3"); // Normal priority second
/// assert_eq!(messages[2].0, "msg1"); // Low priority last
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MessagePriority {
    /// Low priority
    Low,
    /// Normal priority (default)
    #[default]
    Normal,
    /// High priority
    High,
}

/// An OWNED handle that sends on a transport WITHOUT holding that transport's
/// `&mut self` lock (Phase 118.2, plan 23).
///
/// Both operations take `&self`, which is the whole point: a consumer takes one
/// of these under a momentary guard, DROPS the guard, and only then awaits the
/// send. See [`Transport::shared_sender`] for why that matters and what goes
/// wrong without it.
///
/// A transport that offers one MUST route it through the SAME send core its
/// `&mut self` [`Transport::send`] uses. A handle with its own hand-rolled wire
/// path would be a second emission surface — different headers, a different
/// auth-refresh branch — which is a correctness hazard, not a performance one.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SharedSender: Send + Sync + Debug {
    /// Send a typed message, exactly as [`Transport::send`] would.
    ///
    /// # Errors
    ///
    /// Whatever the transport's own send path returns — synchronously, before
    /// this future resolves. A handle that returned early and reported failures
    /// out of band would break every caller that dispatches on the error.
    async fn send_shared(&self, message: TransportMessage) -> Result<()>;

    /// Send an ALREADY-ENCODED JSON-RPC frame verbatim, exactly as
    /// [`Transport::send_raw`] would.
    ///
    /// # Errors
    ///
    /// As [`Self::send_shared`].
    async fn send_raw_shared(&self, body: Vec<u8>) -> Result<()>;
}

/// wasm32 mirror of [`SharedSender`]; `?Send` because wasm futures are not
/// `Send`.
///
/// Kept identical to the native definition — the same rule
/// [`Transport::set_negotiated_protocol_version`] states — so the two cannot
/// drift.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait SharedSender: Debug {
    /// Send a typed message, exactly as [`Transport::send`] would.
    ///
    /// # Errors
    ///
    /// Whatever the transport's own send path returns.
    async fn send_shared(&self, message: TransportMessage) -> Result<()>;

    /// Send an ALREADY-ENCODED JSON-RPC frame verbatim.
    ///
    /// # Errors
    ///
    /// As [`Self::send_shared`].
    async fn send_raw_shared(&self, body: Vec<u8>) -> Result<()>;
}

/// Core transport trait for MCP communication.
///
/// All transport implementations (stdio, WebSocket, HTTP) must implement
/// this trait to be usable with the MCP client/server.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{Transport, TransportMessage};
/// use async_trait::async_trait;
///
/// #[derive(Debug)]
/// struct MyTransport;
///
/// #[async_trait]
/// impl Transport for MyTransport {
///     async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
///         // Send implementation
///         Ok(())
///     }
///
///     async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
///         // Receive implementation  
///         Ok(TransportMessage::Notification(
///             pmcp::types::Notification::Progress(pmcp::types::ProgressNotification::new(
///                 pmcp::types::ProgressToken::String("example".to_string()),
///                 50.0,
///                 Some("Processing...".to_string()),
///             ))
///         ))
///     }
///
///     async fn close(&mut self) -> pmcp::Result<()> {
///         Ok(())
///     }
/// }
/// ```
// On native targets, transports must be Send + Sync so they can be used from
// multi-threaded runtimes. In WASM (single-threaded), we relax this to avoid
// forcing Send/Sync on Web APIs (e.g., web_sys::WebSocket).
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait Transport: Send + Sync + Debug {
    /// Send a message over the transport.
    ///
    /// This method should handle framing and ensure the entire message
    /// is sent atomically.
    async fn send(&mut self, message: TransportMessage) -> Result<()>;

    /// Receive a message from the transport.
    ///
    /// This method should block until a complete message is available.
    /// It should handle any necessary buffering and framing internally.
    ///
    /// # Cancellation
    ///
    /// Implementors MUST make `receive` cancel-safe: if the returned future is
    /// dropped before it resolves (for example when the server's single-owner
    /// transport actor's `select!` drops the in-flight receive to service an
    /// outbound send), NO already-consumed bytes may be lost. The next call to
    /// `receive` must resume and return the next complete message intact, as if
    /// the dropped call had never happened.
    ///
    /// Stream-based transports (`StreamExt::next`, `mpsc::Receiver::recv`) are
    /// cancel-safe by construction. Byte-oriented transports that read into a
    /// local buffer (e.g. `AsyncBufReadExt::read_line`) are NOT — they must
    /// persist their partial-read buffer across calls to satisfy this contract
    /// (see [`crate::shared::StdioTransport`]).
    async fn receive(&mut self) -> Result<TransportMessage>;

    /// Close the transport.
    ///
    /// After calling this method, the transport should not accept any
    /// more messages for sending or receiving.
    async fn close(&mut self) -> Result<()>;

    /// Check if the transport is still connected.
    ///
    /// Default implementation always returns true.
    fn is_connected(&self) -> bool {
        true
    }

    /// Get the transport type name for debugging.
    fn transport_type(&self) -> &'static str {
        "unknown"
    }

    /// Receive the per-connection NEGOTIATED protocol version (Phase 113, CLNT-01).
    ///
    /// The client pushes its
    /// [`ClientBuilder::with_protocol_version`](crate::ClientBuilder::with_protocol_version)
    /// selection here EXACTLY ONCE at build time; a transport that has no wire
    /// representation for it ignores the call (the default body). This is the
    /// mode-propagation seam: `Client<T>` is generic over `T`, but emitting the
    /// `MCP-Protocol-Version` / `Mcp-Method` / `Mcp-Name` headers is the
    /// transport's job, so the selection has to cross that boundary explicitly
    /// rather than by assumption.
    ///
    /// Deliberately NOT named `set_protocol_version`:
    /// [`StreamableHttpTransport`](crate::shared::StreamableHttpTransport) already
    /// has an INHERENT `set_protocol_version(&self, Option<String>)` that the
    /// v1 handshake path writes, and an identically-named trait method would
    /// shadow it confusingly.
    fn set_negotiated_protocol_version(&mut self, _version: Option<String>) {}

    /// Whether this transport has a wire representation for the negotiated
    /// protocol version (Phase 113, CLNT-01).
    ///
    /// `false` by default. `ClientBuilder::build` emits a `tracing::warn!` when a
    /// caller selects `2026-07-28` on a transport that answers `false`, so an
    /// INERT v2 selection is visible instead of silently producing requests a v2
    /// server rejects (T-113-53). v2-over-stdio is explicitly out of scope for
    /// Phase 113.
    fn supports_negotiated_protocol_version(&self) -> bool {
        false
    }

    /// Send an ALREADY-ENCODED JSON-RPC frame verbatim (Phase 113, CLNT-01).
    ///
    /// The v2 (`2026-07-28`) client path needs this for two reasons the typed
    /// [`TransportMessage::Request`] cannot serve:
    ///
    /// 1. Every v2 request must carry `params._meta` with the reserved
    ///    `io.modelcontextprotocol/*` keys, and only three typed request structs
    ///    have a `_meta` field — adding one to the rest is a MAJOR semver break
    ///    (Phase-113 D-113-D).
    /// 2. `server/discover` has no [`ClientRequest`](crate::types::ClientRequest)
    ///    variant and must not gain one (adding a variant to an exhaustive public
    ///    enum is also a MAJOR break, Phase-112 D-10).
    ///
    /// The default body errors, so a transport that does not implement it simply
    /// cannot speak v2 — which is exactly what
    /// [`Self::supports_negotiated_protocol_version`] reports up front.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidState`](crate::Error::InvalidState) by default.
    /// Implementors return whatever their normal send path returns.
    async fn send_raw(&mut self, _body: Vec<u8>) -> Result<()> {
        Err(crate::Error::InvalidState(format!(
            "transport {} cannot send raw JSON-RPC frames (required by the 2026-07-28 era)",
            self.transport_type()
        )))
    }

    /// An OWNED handle that can send WITHOUT this transport's `&mut self` lock
    /// (Phase 118.2, plan 23). `None` by default.
    ///
    /// # Why this exists
    ///
    /// [`Self::send`] and [`Self::receive`] both take `&mut self`, so a consumer
    /// holding one transport for many callers — [`Client`](crate::Client) holds
    /// it behind a single `RwLock` — necessarily serialises them. That is
    /// harmless for a lock held across a local operation and catastrophic for
    /// one held across a NETWORK round trip: an HTTP transport's `send` awaits
    /// the peer's response HEADERS, so a peer that accepts a POST and never
    /// answers holds the one lock forever and every other operation on that
    /// client — a second call, a notification, a cancellation, even
    /// [`Self::close`] — blocks at acquisition with nothing to bound it.
    ///
    /// # The handle is OWNED, and that is the point
    ///
    /// It is returned behind an [`Arc`](std::sync::Arc) precisely so the caller
    /// can DROP its guard and only then await the send. A borrowed handle would
    /// keep the guard alive for the caller's whole round trip, which is the
    /// state of affairs this seam removes.
    ///
    /// # A READ guard is NOT an acceptable substitute
    ///
    /// tokio's `RwLock` is fair: a writer that arrives while a reader holds the
    /// lock parks, and every reader arriving after that writer parks BEHIND it.
    /// A read guard held across a round trip therefore blocks the next `receive`
    /// and the next `close` exactly as completely as a write guard would. The
    /// guard must be released, not merely downgraded.
    ///
    /// # The default `None` is what keeps every existing implementor unchanged
    ///
    /// A transport that answers `None` — which is every transport that does not
    /// opt in, including every external one, without editing a line — is used
    /// through the exclusive `&mut` path exactly as it is today, byte for byte.
    /// Answering `Some` is an assertion that concurrent sends on this transport
    /// are safe; do not make it lightly.
    fn shared_sender(&self) -> Option<std::sync::Arc<dyn SharedSender>> {
        None
    }
}

/// A bidirectional MCP message transport.
///
/// wasm32 mirror of the native trait; `?Send` because wasm futures are not
/// `Send`.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait Transport: Debug {
    /// Send a message over the transport.
    async fn send(&mut self, message: TransportMessage) -> Result<()>;

    /// Receive a message from the transport.
    async fn receive(&mut self) -> Result<TransportMessage>;

    /// Close the transport.
    async fn close(&mut self) -> Result<()>;

    /// Check if the transport is still connected.
    fn is_connected(&self) -> bool {
        true
    }

    /// Get the transport type name for debugging.
    fn transport_type(&self) -> &'static str {
        "unknown"
    }

    /// Receive the per-connection NEGOTIATED protocol version (Phase 113, CLNT-01).
    ///
    /// Parity mirror of the native definition — see that one for the full
    /// contract. Kept identical so the two trait definitions cannot drift.
    fn set_negotiated_protocol_version(&mut self, _version: Option<String>) {}

    /// Whether this transport has a wire representation for the negotiated
    /// protocol version (Phase 113, CLNT-01).
    ///
    /// Parity mirror of the native definition.
    fn supports_negotiated_protocol_version(&self) -> bool {
        false
    }

    /// Send an ALREADY-ENCODED JSON-RPC frame verbatim (Phase 113, CLNT-01).
    ///
    /// Parity mirror of the native definition.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidState`](crate::Error::InvalidState) by default.
    async fn send_raw(&mut self, _body: Vec<u8>) -> Result<()> {
        Err(crate::Error::InvalidState(format!(
            "transport {} cannot send raw JSON-RPC frames (required by the 2026-07-28 era)",
            self.transport_type()
        )))
    }

    /// An OWNED handle that can send WITHOUT this transport's `&mut self` lock
    /// (Phase 118.2, plan 23). `None` by default.
    ///
    /// Parity mirror of the native definition — see that one for the full
    /// contract, including why a READ guard is not an acceptable substitute and
    /// why the default `None` leaves every existing implementor unchanged. Kept
    /// identical so the two trait definitions cannot drift.
    fn shared_sender(&self) -> Option<std::sync::Arc<dyn SharedSender>> {
        None
    }
}

/// Options for sending messages.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::{SendOptions, MessagePriority};
/// use std::time::Duration;
///
/// // Default options
/// let default_opts = SendOptions::default();
/// assert!(default_opts.priority.is_none());
/// assert!(!default_opts.flush);
/// assert!(default_opts.timeout.is_none());
///
/// // High priority message with immediate flush
/// let urgent_opts = SendOptions {
///     priority: Some(MessagePriority::High),
///     flush: true,
///     timeout: Some(Duration::from_secs(5)),
/// };
///
/// // Builder pattern for options
/// fn build_send_options(urgent: bool) -> SendOptions {
///     SendOptions {
///         priority: if urgent {
///             Some(MessagePriority::High)
///         } else {
///             Some(MessagePriority::Normal)
///         },
///         flush: urgent,
///         timeout: Some(Duration::from_secs(if urgent { 5 } else { 30 })),
///     }
/// }
///
/// let opts = build_send_options(true);
/// assert_eq!(opts.priority, Some(MessagePriority::High));
/// assert!(opts.flush);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Message priority
    pub priority: Option<MessagePriority>,
    /// Whether to flush immediately after sending
    pub flush: bool,
    /// Timeout for the send operation
    pub timeout: Option<std::time::Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(MessagePriority::Low < MessagePriority::Normal);
        assert!(MessagePriority::Normal < MessagePriority::High);
    }

    // Plan 23: the accessor's DEFAULT body is what keeps every existing
    // implementor unchanged. `NoSharedSend` implements exactly the three methods
    // an implementor had to implement before this plan — nothing about
    // `shared_sender` — and answers `None` without saying so, which is the whole
    // of the additive claim.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_transport_that_does_not_opt_in_answers_none() {
        #[derive(Debug)]
        struct NoSharedSend;

        #[async_trait]
        impl Transport for NoSharedSend {
            async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                Ok(())
            }

            async fn receive(&mut self) -> Result<TransportMessage> {
                Err(crate::Error::InvalidState(
                    "no receive in this fence".to_string(),
                ))
            }

            async fn close(&mut self) -> Result<()> {
                Ok(())
            }
        }

        assert!(
            NoSharedSend.shared_sender().is_none(),
            "a transport that implements only the pre-existing trait surface must answer `None` \
             from the DEFAULT body — that default is what makes this addition invisible to every \
             external implementor"
        );
    }

    // Regression (Phase 103 UAT, F2): a Request must serialize to a flat JSON-RPC
    // frame, NOT the untagged `{"id":…,"request":…}` shape. Serializing the
    // `TransportMessage` enum directly produced the latter, which every MCP server
    // rejected with -32700 "Unknown message type".
    #[test]
    fn serialize_request_is_flat_jsonrpc_frame() {
        use crate::types::{Request, RequestId};

        let msg = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(crate::types::ClientRequest::Ping)),
        };
        let bytes = serialize_message(&msg).expect("serialize");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("valid json");

        assert_eq!(v["jsonrpc"], "2.0", "must carry the JSON-RPC version");
        assert!(v.get("method").is_some(), "must have a top-level method");
        assert!(
            v.get("request").is_none(),
            "must NOT emit the untagged enum wrapper: {v}"
        );
    }

    // Regression (Phase 103 UAT, F2): parse_message classifies the three frame
    // kinds and round-trips a request through serialize→parse.
    #[test]
    fn parse_message_roundtrips_request_and_classifies() {
        use crate::types::{Request, RequestId};

        let msg = TransportMessage::Request {
            id: RequestId::from(7i64),
            request: Request::Client(Box::new(crate::types::ClientRequest::Ping)),
        };
        let bytes = serialize_message(&msg).expect("serialize");
        match parse_message(&bytes).expect("parse") {
            TransportMessage::Request { id, .. } => assert_eq!(id, RequestId::from(7i64)),
            other => panic!("expected Request, got {other:?}"),
        }

        // A response frame classifies as Response.
        let resp = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        assert!(matches!(
            parse_message(resp).expect("parse response"),
            TransportMessage::Response(_)
        ));

        // A frame with none of method/result/error is rejected.
        assert!(parse_message(br#"{"id":1,"request":{}}"#).is_err());
    }
}
