//! Phase 113-10 (HTTP-04): live-HTTP acceptance for `subscriptions/listen`.
//!
//! Both conformant configurations are proven over a REAL loopback socket:
//!
//! * **advertise nothing** — `server/discover` publishes no subscription-delivered
//!   capability AND `subscriptions/listen` answers `404` + `-32601`. The
//!   conformance rule is the CONJUNCTION: a `-32601` without an observed discover
//!   is recorded FAILURE, not SKIPPED, so both halves are asserted in one test.
//! * **advertise anything** — the stream is SERVED, ack-first, `subscriptionId`
//!   tagged, filter-respecting, collision-free across callers sharing a request
//!   id, and reclaiming its slot on disconnect.
//!
//! # Why a raw TCP client rather than `reqwest`
//!
//! Reading a long-lived SSE body requires a STREAMING read; the shared harness's
//! `post` helper reads to EOF and would hang forever on a stream that never ends.
//! `reqwest` is compiled here without its `stream` feature, so [`SseStream`]
//! below speaks HTTP/1.1 over a `tokio::net::TcpStream` directly. That also buys
//! the deterministic client-disconnect this file needs: dropping the socket IS
//! the disconnect, with no connection pool holding it open.
//!
//! EVERY stream read is wrapped in a [`tokio::time::timeout`], so a hung or
//! never-acknowledged stream fails the test instead of wedging CI (T-113-36).
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

// The in-process duplex transport, included per-crate exactly as its own module
// docs prescribe. Needed for the Finding 5 off-stream probe: on the HTTP
// transport the listen registry is the ONLY notification sink, so the
// non-listen delivery path can only be observed on a `Server::run` transport.
#[path = "common/duplex.rs"]
mod duplex;

use common::v2::{
    build_v2_server_with, post, spawn_default_config, spawn_shared, v2_body, v2_headers,
    BearerSubjects, GreetingPrompt, OptionalBearer, SearchTool, FRAME_TIMEOUT, V1, V2,
};
use pmcp::server::Server;
use pmcp::types::protocol::error_codes::{AUTHENTICATION_REQUIRED, METHOD_NOT_FOUND, RATE_LIMITED};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::subscriptions::{
    advertises_subscriptions, ACKNOWLEDGED_METHOD, MAX_AGREED_RESOURCE_SUBSCRIPTIONS,
    SUBSCRIPTION_ID_META_KEY,
};
use pmcp::types::{
    PromptCapabilities, ResourceCapabilities, ResourceUpdatedParams, ServerCapabilities,
    ServerNotification, ToolCapabilities,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// ===========================================================================
// Servers.
// ===========================================================================

/// The four subscription-delivered capabilities, by their conformance names.
const CAPABILITY_NAMES: [&str; 4] = [
    "tools.listChanged",
    "prompts.listChanged",
    "resources.listChanged",
    "resources.subscribe",
];

/// `ServerCapabilities` advertising exactly ONE of the four, or none.
///
/// Registering handlers AFTER `.capabilities(..)` only fills sub-capabilities
/// that are still `None`, and pmcp's registration defaults are `Some(false)`, so
/// the advertise-nothing server really does advertise nothing.
fn advertising(which: Option<&str>) -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    match which {
        Some("tools.listChanged") => {
            caps.tools = Some(ToolCapabilities {
                list_changed: Some(true),
            });
        },
        Some("prompts.listChanged") => {
            caps.prompts = Some(PromptCapabilities {
                list_changed: Some(true),
            });
        },
        Some("resources.listChanged") => {
            caps.resources = Some(ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(true),
            });
        },
        Some("resources.subscribe") => {
            caps.resources = Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(false),
            });
        },
        _ => {},
    }
    caps
}

/// A v2-opted-in server with the given capabilities and one handler per method.
fn server_with(caps: ServerCapabilities) -> Server {
    build_v2_server_with("v2-subscriptions", caps)
}

/// The two-principal server the `ListenKey` collision test drives.
fn server_with_two_principals() -> Server {
    let mut caps = ServerCapabilities::default();
    caps.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    caps.prompts = Some(PromptCapabilities {
        list_changed: Some(true),
    });
    Server::builder()
        .name("v2-subscriptions-auth")
        .version("1.0.0")
        .capabilities(caps)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .auth_provider(BearerSubjects)
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .build()
        .expect("server builds")
}

// `OptionalBearer` — the `Ok(None)` auth provider D-113-N's precondition needs —
// used to be defined right here. Plan 114-02 MOVED it into the shared harness
// (`tests/common/v2.rs`) because the Phase-114 tasks suites need the same
// precondition, and two divergent definitions of "this server admits anonymous
// callers" is how a security test comes to pass for the wrong reason. Its full
// rustdoc, including why `BearerSubjects` cannot serve this role, travelled with
// it. It is imported at the top of this file; nothing about the behaviour of the
// tests below changed.

/// A v2 server advertising `tools.listChanged` whose auth provider ADMITS
/// unauthenticated requests — the D-113-N configuration.
fn server_with_optional_auth() -> Server {
    Server::builder()
        .name("v2-subscriptions-optional-auth")
        .version("1.0.0")
        .capabilities(advertising(Some("tools.listChanged")))
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .auth_provider(OptionalBearer)
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .build()
        .expect("server builds")
}

/// Spawn a server this test does not need a handle to.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(server).await
}

// ===========================================================================
// Request bodies.
// ===========================================================================

/// A `subscriptions/listen` body requesting `filter`.
fn listen_body(id: Value, filter: &Value) -> String {
    // Built through a `Map` rather than the `json!` macro because the macro
    // BORROWS its interpolated values, which would leave `id` passed by value
    // but never consumed.
    let mut params = serde_json::Map::new();
    params.insert("notifications".to_string(), filter.clone());
    v2_body("subscriptions/listen", id, Value::Object(params))
}

/// The v2 routing headers for `subscriptions/listen`.
///
/// It is a name-less method, so since Phase 118 D-13 `Mcp-Name` is OPTIONAL on it
/// and the empty value emitted here is discarded by the gate. `Mcp-Method` and
/// `MCP-Protocol-Version` remain mandatory.
fn listen_headers() -> Vec<(String, String)> {
    v2_headers("subscriptions/listen", "")
}

// ===========================================================================
// Raw streaming SSE client.
// ===========================================================================

/// One parsed SSE event: either a `data:` payload or a comment line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SseEvent {
    Data(String),
    Comment(String),
}

/// A minimal HTTP/1.1 client that can read a response body INCREMENTALLY.
///
/// It handles BOTH framings the listen route produces: a `chunked` SSE stream
/// for a served subscription, and a `Content-Length` JSON body for every
/// rejection (`-32601`, `-32602`, the concurrency refusal). A test therefore
/// reads the first frame the same way regardless of which it got.
struct SseStream {
    reader: BufReader<TcpStream>,
    status: u16,
    headers: Vec<(String, String)>,
    /// Undelivered decoded body text.
    buffer: String,
    /// `true` when the body is `Transfer-Encoding: chunked`.
    chunked: bool,
    /// Remaining `Content-Length` bytes, for the non-chunked framing.
    remaining: usize,
    /// `true` once the body has signalled its end.
    finished: bool,
}

impl SseStream {
    /// POST `body` and read only as far as the response headers.
    async fn open(addr: SocketAddr, extra: &[(String, String)], body: &str) -> Self {
        let stream = TcpStream::connect(addr).await.expect("connects");
        let mut request = format!(
            "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);

        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .expect("request written");

        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .await
            .expect("status line");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("header line");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }

        let chunked = headers
            .iter()
            .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"));
        let remaining = headers
            .iter()
            .find(|(n, _)| n == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        Self {
            reader,
            status,
            headers,
            buffer: String::new(),
            chunked,
            remaining,
            finished: false,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Read more body bytes into [`Self::buffer`], in whichever framing applies.
    async fn pull(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if !self.chunked {
            // `Content-Length` framing: one read, then the body is complete.
            let mut payload = vec![0u8; self.remaining];
            let ok = self.remaining > 0 && self.reader.read_exact(&mut payload).await.is_ok();
            self.finished = true;
            if !ok {
                return false;
            }
            self.buffer.push_str(&String::from_utf8_lossy(&payload));
            return true;
        }
        let mut size_line = String::new();
        if self.reader.read_line(&mut size_line).await.unwrap_or(0) == 0 {
            self.finished = true;
            return false;
        }
        let size_token = size_line.trim().split(';').next().unwrap_or("").to_string();
        let Ok(size) = usize::from_str_radix(&size_token, 16) else {
            self.finished = true;
            return false;
        };
        if size == 0 {
            self.finished = true;
            return false;
        }
        let mut payload = vec![0u8; size];
        if self.reader.read_exact(&mut payload).await.is_err() {
            self.finished = true;
            return false;
        }
        let mut crlf = [0u8; 2];
        let _ = self.reader.read_exact(&mut crlf).await;
        self.buffer.push_str(&String::from_utf8_lossy(&payload));
        true
    }

    /// Pop one complete SSE block (`...\n\n`) from the buffer, if present.
    fn take_block(&mut self) -> Option<String> {
        let end = self.buffer.find("\n\n")?;
        let block = self.buffer[..end].to_string();
        self.buffer.drain(..end + 2);
        Some(block)
    }

    /// The next SSE event, or `None` at end of stream.
    ///
    /// ALWAYS call this through [`Self::expect_event`] / [`Self::expect_no_event`]
    /// so the read is bounded.
    async fn next_event(&mut self) -> Option<SseEvent> {
        loop {
            if let Some(block) = self.take_block() {
                let mut data = String::new();
                let mut comment = None;
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        data.push_str(rest.trim_start());
                    } else if let Some(rest) = line.strip_prefix(':') {
                        comment = Some(rest.trim().to_string());
                    }
                }
                if !data.is_empty() {
                    return Some(SseEvent::Data(data));
                }
                if let Some(comment) = comment {
                    return Some(SseEvent::Comment(comment));
                }
                continue;
            }
            if !self.pull().await {
                // A `Content-Length` JSON body has no `\n\n` block; flush it as
                // the single frame it is.
                if !self.buffer.trim().is_empty() {
                    let rest = std::mem::take(&mut self.buffer);
                    return Some(SseEvent::Data(rest.trim().to_string()));
                }
                return None;
            }
        }
    }

    /// The next `data:` payload, parsed as JSON. Bounded by [`FRAME_TIMEOUT`].
    async fn expect_json(&mut self) -> Value {
        loop {
            let event = tokio::time::timeout(FRAME_TIMEOUT, self.next_event())
                .await
                .expect("a frame arrived within the timeout")
                .expect("the stream did not end");
            if let SseEvent::Data(data) = event {
                return serde_json::from_str(&data).expect("the frame is JSON");
            }
            // A keep-alive comment is not a protocol frame; keep reading.
        }
    }

    /// Assert NO data frame arrives within `window`.
    async fn expect_no_json(&mut self, window: Duration) {
        if let Ok(Some(SseEvent::Data(data))) =
            tokio::time::timeout(window, self.next_event()).await
        {
            panic!("unexpected frame delivered to this stream: {data}");
        }
    }
}

/// The `subscriptionId` carried by any listen frame (a notification's
/// `params._meta` or a result's `result._meta`).
///
/// Indexed rather than read via `Value::pointer`: the reserved key CONTAINS a
/// `/`, which JSON Pointer would treat as a path separator unless escaped.
fn subscription_id_of(frame: &Value) -> Option<&Value> {
    ["params", "result"].into_iter().find_map(|section| {
        frame
            .get(section)?
            .get("_meta")?
            .get(SUBSCRIPTION_ID_META_KEY)
    })
}

// ===========================================================================
// Tests.
// ===========================================================================

/// The DEFAULT, conformant-by-absence configuration.
///
/// The conformance rule is a CONJUNCTION — a `-32601` is SKIPPED only when
/// `server/discover` was observed AND advertises nothing subscription-delivered
/// — so both halves are asserted here, in one test.
#[tokio::test]
async fn absent_capability_is_conformant() {
    let (addr, handle) = spawn(server_with(advertising(None))).await;

    let discover = post(
        addr,
        &v2_headers("server/discover", ""),
        &v2_body("server/discover", json!(1), json!({})),
    )
    .await;
    assert_eq!(discover.status, 200, "discover must be OBSERVED");
    let capabilities: ServerCapabilities =
        serde_json::from_value(discover.body["result"]["capabilities"].clone())
            .expect("the projection deserializes");
    assert!(
        !advertises_subscriptions(&capabilities),
        "the default advertises no subscription-delivered capability: {:?}",
        discover.body["result"]["capabilities"]
    );

    let listen = post(addr, &listen_headers(), &listen_body(json!(2), &json!({}))).await;
    assert_eq!(listen.status, 404, "spec: unimplemented method is 404");
    assert_eq!(listen.body["error"]["code"], json!(METHOD_NOT_FOUND));
    assert_eq!(listen.body["id"], json!(2), "the ORIGINAL id is echoed");

    handle.abort();
}

/// THE tripwire: advertising ANY of the four means the stream is SERVED.
///
/// Each capability is exercised INDIVIDUALLY, which is exactly the conformance
/// rule ("claims a feature it does not serve") encoded locally.
#[tokio::test]
async fn advertise_implies_serve() {
    for which in CAPABILITY_NAMES {
        let (addr, handle) = spawn(server_with(advertising(Some(which)))).await;

        let mut stream = SseStream::open(
            addr,
            &listen_headers(),
            &listen_body(json!(1), &json!({ "toolsListChanged": true })),
        )
        .await;

        assert_eq!(
            stream.status, 200,
            "{which} is advertised, so the stream must be served"
        );
        assert_eq!(
            stream.header("content-type"),
            Some("text/event-stream"),
            "{which}: the served response is an SSE stream"
        );
        let first = stream.expect_json().await;
        assert_ne!(
            first["error"]["code"],
            json!(METHOD_NOT_FOUND),
            "{which} is advertised, so -32601 here would be a conformance FAILURE"
        );
        assert_eq!(
            first["method"],
            json!(ACKNOWLEDGED_METHOD),
            "{which}: the first frame is the acknowledgement"
        );

        drop(stream);
        handle.abort();
    }
}

/// The served stream's wire protocol: SSE content type, ack first, matching
/// `subscriptionId` on the ack AND on every subsequent notification.
#[tokio::test]
async fn listen_stream_protocol() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "tools.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(11), &json!({ "toolsListChanged": true })),
    )
    .await;

    assert_eq!(stream.status, 200);
    assert_eq!(stream.header("content-type"), Some("text/event-stream"));
    assert_eq!(
        stream.header("x-accel-buffering"),
        Some("no"),
        "spec: servers SHOULD disable proxy buffering on the stream"
    );

    let ack = stream.expect_json().await;
    assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
    assert_eq!(
        ack["params"]["notifications"],
        json!({ "toolsListChanged": true }),
        "the ack reports the AGREED filter"
    );
    assert_eq!(
        subscription_id_of(&ack),
        Some(&json!(11)),
        "the subscriptionId equals the listen request's JSON-RPC id"
    );

    // Drive the server's REAL notification path.
    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let notification = stream.expect_json().await;
    assert_eq!(
        notification["method"],
        json!("notifications/tools/list_changed")
    );
    assert_eq!(
        subscription_id_of(&notification),
        Some(&json!(11)),
        "every subsequent frame carries the SAME subscriptionId"
    );

    drop(stream);
    handle.abort();
}

/// A notification type the client did not request is never delivered.
#[tokio::test]
async fn no_unrequested_notification_types() {
    let mut caps = ServerCapabilities::default();
    caps.tools = Some(ToolCapabilities {
        list_changed: Some(true),
    });
    caps.prompts = Some(PromptCapabilities {
        list_changed: Some(true),
    });
    let server = Arc::new(Mutex::new(server_with(caps)));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    // TWO advertised, only ONE requested.
    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(21), &json!({ "toolsListChanged": true })),
    )
    .await;
    let ack = stream.expect_json().await;
    assert_eq!(
        ack["params"]["notifications"],
        json!({ "toolsListChanged": true }),
        "the agreed filter is never a superset of the request"
    );

    // Trigger BOTH change notifications.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::PromptsChanged)
            .await;
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "only the REQUESTED type appears, and it appears FIRST despite prompts \
         being triggered first"
    );
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
}

/// No notification frame may precede the acknowledgement.
///
/// The change notification is fired IMMEDIATELY after the request goes out and
/// before anything is read, which is the tightest race this can be put under.
#[tokio::test]
async fn ack_is_first_frame() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "tools.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(31), &json!({ "toolsListChanged": true })),
    )
    .await;
    for _ in 0..5 {
        server
            .lock()
            .await
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    let first = stream.expect_json().await;
    assert_eq!(
        first["method"],
        json!(ACKNOWLEDGED_METHOD),
        "the acknowledgement MUST be the first message on the stream"
    );

    drop(stream);
    handle.abort();
}

// ===========================================================================
// Addendum Finding 14(b) — the RESOURCES half of HTTP-08's four opt-ins.
//
// `toolsListChanged` and `promptsListChanged` are proven over a real socket by
// the tests above. `resourcesListChanged` and `resourceSubscriptions` were, until
// this section existed, exercised ONLY by `#[cfg(test)]` unit tests inside
// `src/types/subscriptions.rs` — never over the wire. That asymmetry is invisible
// in a green suite, because the tests that would fail did not exist.
//
// `resourceSubscriptions` is also the only one of the four that is not a boolean:
// it is a `string[]` of URIs, its delivery decision is a linear EXACT-STRING scan
// (`SubscriptionFilter::covers`'s `ResourceUpdated` arm — no prefix matching, no
// normalisation), and it carries a per-stream truncation bound. None of that had
// ever been proven to compose over live HTTP.
// ===========================================================================

/// The resource URI the tests below SUBSCRIBE to.
const SUBSCRIBED_URI: &str = "mem://a";

/// A DIFFERENT resource URI that no test below ever subscribes to.
///
/// Deliberately one character from [`SUBSCRIBED_URI`] in the same scheme, so a
/// mix-up shows up as a legible diff in the failure message rather than as two
/// unrelated strings.
const UNSUBSCRIBED_URI: &str = "mem://b";

/// A `notifications/resources/updated` reaches a stream that named its URI, and
/// a stream that did not name a URI never sees it.
///
/// This is the live-socket proof of `SubscriptionFilter::covers`'s
/// `ResourceUpdated` arm — the one arm that consults a client-supplied list
/// rather than reading a flag, and therefore the one arm whose over-broad failure
/// mode leaks another resource's change notification to a subscriber that never
/// asked for it (T-113-150).
///
/// The unsubscribed URI is fired FIRST and the subscribed one second, exactly as
/// [`no_unrequested_notification_types`] orders its two triggers: a test that
/// fired only the subscribed URI could not distinguish "filtered out" from
/// "merely slower", and would still pass against a `covers` that returned `true`
/// unconditionally.
#[tokio::test]
async fn resource_subscriptions_deliver_the_subscribed_uri_and_not_another() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "resources.subscribe",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(
            json!(51),
            &json!({ "resourceSubscriptions": [SUBSCRIBED_URI] }),
        ),
    )
    .await;

    assert_eq!(stream.status, 200, "resources.subscribe is advertised");
    assert_eq!(
        stream.header("content-type"),
        Some("text/event-stream"),
        "the served response is a stream, not a refusal body"
    );

    // The acknowledgement is asserted BEFORE anything is fired. It is the only
    // observable proof that `intersect_with_capabilities` KEPT the requested
    // list; asserting delivery alone would still pass on a server that agreed to
    // something else entirely and delivered by coincidence.
    let ack = stream.expect_json().await;
    assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
    assert_eq!(
        ack["params"]["notifications"],
        json!({ "resourceSubscriptions": [SUBSCRIBED_URI] }),
        "the agreed filter echoes the requested URI list EXACTLY — the whole \
         object is compared, so an extra agreed field would fail here too: {ack}"
    );
    assert_eq!(
        subscription_id_of(&ack),
        Some(&json!(51)),
        "the subscriptionId equals the listen request's JSON-RPC id: {ack}"
    );

    // The UNSUBSCRIBED URI goes out FIRST — under one lock, so nothing can
    // reorder them between the two sends.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(UNSUBSCRIBED_URI),
            ))
            .await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(SUBSCRIBED_URI),
            ))
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/resources/updated"),
        "the subscribed URI is delivered as a resources/updated frame: {delivered}"
    );
    assert_eq!(
        delivered["params"]["uri"],
        json!(SUBSCRIBED_URI),
        "and it is the SUBSCRIBED URI that arrives first, despite \
         {UNSUBSCRIBED_URI} having been fired before it: {delivered}"
    );
    assert_eq!(
        subscription_id_of(&delivered),
        Some(&json!(51)),
        "a delivered resources/updated carries the stream's subscriptionId: {delivered}"
    );

    // `covers` is EXACT string equality, so the unsubscribed URI must never
    // arrive — proven by a bounded wait rather than by the absence of a further
    // assertion.
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
    let _ = handle.await;
}

/// The two resources opt-ins are INDEPENDENT: a `resources.subscribe`-only
/// server neither agrees to nor delivers `resourcesListChanged`.
///
/// The client here asks for BOTH halves. `advertising("resources.subscribe")`
/// sets `list_changed: Some(false)`, so `agreed_flag` OMITS the requested
/// `resourcesListChanged` — and the omission has to be observable as an ABSENT
/// key, because `skip_serializing_if = "Option::is_none"` means an omitted field
/// never reaches the wire at all. Accepting `false` or `null` here would pass on
/// a future change that started agreeing to unsupported types.
///
/// `ResourcesChanged` is fired FIRST for the same reason the test above fires the
/// unsubscribed URI first.
#[tokio::test]
async fn a_resource_subscriptions_stream_is_not_a_resources_list_changed_stream() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "resources.subscribe",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(
            json!(52),
            &json!({
                "resourceSubscriptions": [SUBSCRIBED_URI],
                "resourcesListChanged": true,
            }),
        ),
    )
    .await;

    let ack = stream.expect_json().await;
    let agreed = &ack["params"]["notifications"];
    assert!(
        agreed.get("resourcesListChanged").is_none(),
        "an unsupported requested type is OMITTED from the agreed filter, not \
         agreed as `false` and not emitted as `null`: {ack}"
    );
    assert_eq!(
        *agreed,
        json!({ "resourceSubscriptions": [SUBSCRIBED_URI] }),
        "only the supported half survives the intersection: {ack}"
    );

    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::ResourcesChanged)
            .await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(SUBSCRIBED_URI),
            ))
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/resources/updated"),
        "the FIRST frame is the resources/updated, even though \
         resources/list_changed was fired before it — the list-changed half was \
         never agreed to, so it is not merely late: {delivered}"
    );
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
    let _ = handle.await;
}

/// The MIRROR of the test above: a `resources.listChanged`-only server agrees to
/// and delivers `resourcesListChanged`, and refuses `resourceSubscriptions`.
///
/// Together the two tests are the cross-product that proves the two resources
/// opt-ins are independent rather than one capability wearing two names: each
/// server advertises exactly one half, each client requests BOTH halves, and each
/// acknowledgement keeps exactly the advertised half.
///
/// The omitted half is asserted as an ABSENT KEY. `agreed_flag` and
/// `intersect_with_capabilities`'s `_ => None` arm both produce `None`, and
/// `skip_serializing_if = "Option::is_none"` keeps `None` off the wire entirely —
/// so accepting `null` or `[]` here would let a future change that started
/// emitting an empty agreed list pass, and that is a DIFFERENT contract (a server
/// agreeing to a capability it does not advertise, T-113-152).
#[tokio::test]
async fn resources_list_changed_is_agreed_and_delivered_when_subscriptions_are_not() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "resources.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(
            json!(53),
            &json!({
                "resourcesListChanged": true,
                "resourceSubscriptions": [SUBSCRIBED_URI],
            }),
        ),
    )
    .await;

    assert_eq!(stream.status, 200, "resources.listChanged is advertised");
    let ack = stream.expect_json().await;
    let agreed = &ack["params"]["notifications"];
    assert!(
        agreed.get("resourceSubscriptions").is_none(),
        "`resources.subscribe` is NOT advertised, so the requested URI list is \
         OMITTED from the agreed filter — the key must be ABSENT, not present as \
         `[]` and not present as `null`: {ack}"
    );
    assert_eq!(
        *agreed,
        json!({ "resourcesListChanged": true }),
        "only the advertised half survives: {ack}"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(53)));

    // The requested-but-NOT-agreed URI goes out first.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(SUBSCRIBED_URI),
            ))
            .await;
        server
            .send_notification(ServerNotification::ResourcesChanged)
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/resources/list_changed"),
        "the agreed half is delivered, and it arrives FIRST despite the \
         un-agreed resources/updated having been fired before it: {delivered}"
    );
    assert_eq!(
        subscription_id_of(&delivered),
        Some(&json!(53)),
        "a delivered resources/list_changed carries the stream's \
         subscriptionId: {delivered}"
    );
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
    let _ = handle.await;
}

/// An over-bound `resourceSubscriptions` list is ACCEPTED and TRUNCATED, and the
/// truncation is reported back in the acknowledgement the client actually reads.
///
/// The spec allows the agreed set to omit entries precisely so a server can bound
/// a client-supplied list without failing the request, on the condition that the
/// omission is *reported* rather than silent. That contract is only meaningful if
/// it is observed on the wire: `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` exists because
/// the agreed list is retained per live stream and rescanned on every
/// `notifications/resources/updated` fan-out under the registry read lock
/// (T-113-151), and a bound nobody can see is a bound nobody can rely on.
///
/// The bound is IMPORTED, never spelled `1024`: a future change to the production
/// constant must move this test with it rather than quietly making it vacuous.
/// The two probe URIs are likewise chosen BY INDEX — one from the head of the
/// list, one at the first index past the bound — so the test states the
/// truncation semantics rather than a coincidence about particular strings.
///
/// This test's request body is deliberately the largest in the file (roughly
/// 16 KB of URIs, far under the 4 MB `DEFAULT_MAX_REQUEST_BYTES`); it is the only
/// one here that exercises the whole-body path at size, and the size is the point
/// rather than sloppiness.
#[tokio::test]
async fn an_over_bound_resource_subscriptions_list_is_truncated_and_reported() {
    let uris: Vec<String> = (0..=MAX_AGREED_RESOURCE_SUBSCRIPTIONS)
        .map(|index| format!("mem://r/{index}"))
        .collect();
    let kept = uris[0].clone();
    let truncated_away = uris[MAX_AGREED_RESOURCE_SUBSCRIPTIONS].clone();

    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "resources.subscribe",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(54), &json!({ "resourceSubscriptions": uris })),
    )
    .await;

    assert_eq!(
        stream.status, 200,
        "an over-bound list is TRUNCATED, not rejected: truncating keeps the \
         operation conformant because the agreed set is allowed to omit entries"
    );
    assert_eq!(
        stream.header("content-type"),
        Some("text/event-stream"),
        "the stream is served, not refused"
    );

    let ack = stream.expect_json().await;
    let agreed = ack["params"]["notifications"]["resourceSubscriptions"]
        .as_array()
        .expect("the agreed filter reports the URI list it kept");
    assert_eq!(
        agreed.len(),
        MAX_AGREED_RESOURCE_SUBSCRIPTIONS,
        "{} URIs were requested; the acknowledgement reports exactly \
         MAX_AGREED_RESOURCE_SUBSCRIPTIONS of them",
        uris.len()
    );
    assert_eq!(
        agreed[0],
        json!(kept),
        "truncation keeps the HEAD of the requested list (`.take(..)`), so index \
         0 survives"
    );
    assert!(
        !agreed.contains(&json!(truncated_away)),
        "the URI at index MAX_AGREED_RESOURCE_SUBSCRIPTIONS is past the bound and \
         must not appear in the agreed list"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(54)));

    // The TRUNCATED-AWAY URI is fired first: it must be filtered, not merely late.
    {
        let server = server.lock().await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(truncated_away.clone()),
            ))
            .await;
        server
            .send_notification(ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new(kept.clone()),
            ))
            .await;
    }

    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/resources/updated"),
        "the kept URI still delivers after truncation: {delivered}"
    );
    assert_eq!(
        delivered["params"]["uri"],
        json!(kept),
        "a URI that survived truncation delivers; the one truncated away does \
         not, even though it was fired first: {delivered}"
    );
    stream.expect_no_json(Duration::from_millis(300)).await;

    drop(stream);
    handle.abort();
    let _ = handle.await;
}

/// Two DIFFERENT principals both using JSON-RPC id `1` must not cross-deliver.
///
/// This is the live half of the `ListenKey { principal, request_id }` fix: an
/// id-keyed registry would have bob's registration EVICT alice's, and alice —
/// the only caller who asked for `toolsListChanged` — would receive nothing.
#[tokio::test]
async fn two_callers_same_request_id_do_not_cross() {
    let server = Arc::new(Mutex::new(server_with_two_principals()));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut alice_headers = listen_headers();
    alice_headers.push(("authorization".to_string(), "Bearer alice".to_string()));
    let mut bob_headers = listen_headers();
    bob_headers.push(("authorization".to_string(), "Bearer bob".to_string()));

    // BOTH use id 1.
    let mut alice = SseStream::open(
        addr,
        &alice_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    let mut bob = SseStream::open(
        addr,
        &bob_headers,
        &listen_body(json!(1), &json!({ "promptsListChanged": true })),
    )
    .await;

    for stream in [&mut alice, &mut bob] {
        let ack = stream.expect_json().await;
        assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
        assert_eq!(subscription_id_of(&ack), Some(&json!(1)));
    }

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let delivered = alice.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "alice's entry survived bob's registration under the SAME request id"
    );
    bob.expect_no_json(Duration::from_millis(300)).await;

    drop(alice);
    drop(bob);
    handle.abort();
}

/// The SAME-principal twin of the test above — the half that shipped UNTESTED.
///
/// Plan 113-10 proved id reuse only ACROSS principals (both tests that claimed
/// to cover it used two different subjects), and `113-VERIFICATION.md` gap item
/// 4 recorded that omission after independently reproducing the defect: two
/// connections authenticated as ONE subject — several tabs, a shared service
/// account, a token with a constant `sub` — collapse onto ONE principal and can
/// still choose the same JSON-RPC id.
///
/// Before the plan-14 fix the second registration EVICTED the first (dropping
/// its `mpsc::Sender`, ending that stream with no terminal frame), so the
/// closing assertion here — that the FIRST stream still receives a fanned-out
/// `tools/list_changed` — is the load-bearing one: alice-1 was already
/// disconnected at that point and the read would time out.
///
/// Plan 113-18 changed only the SHAPE of the refusal, never its existence: the
/// duplicate now answers the RETRYABLE `-32005` at HTTP 200 instead of `-32600`
/// at HTTP 400, because the condition is transient server state rather than a
/// malformed request. The status and code assertions below moved with it; the
/// two MESSAGE assertions did not, and are now the only thing distinguishing
/// this refusal from a capacity refusal.
#[tokio::test]
async fn same_principal_id_reuse_rejects_the_second_and_spares_the_first() {
    let server = Arc::new(Mutex::new(server_with_two_principals()));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    // The ONE difference from the cross-principal twin above: the second caller
    // presents alice's subject too, so both resolve to `AuthContext.subject ==
    // "alice"` and share ONE principal.
    let mut first_headers = listen_headers();
    first_headers.push(("authorization".to_string(), "Bearer alice".to_string()));
    let mut second_headers = listen_headers();
    second_headers.push(("authorization".to_string(), "Bearer alice".to_string()));

    let mut first = SseStream::open(
        addr,
        &first_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    let ack = first.expect_json().await;
    assert_eq!(
        ack["method"],
        json!(ACKNOWLEDGED_METHOD),
        "the first stream is served, ack first"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(1)));

    // The SAME principal, the SAME id, a second connection.
    let mut second = SseStream::open(
        addr,
        &second_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    assert_eq!(
        second.status, 200,
        "a duplicate is a transient, RETRYABLE condition: RATE_LIMITED is not in \
         v2_status_for_code's 400 arm, so it answers at 200 with a JSON-RPC error \
         body, exactly as both capacity refusals already do"
    );
    let refusal = second.expect_json().await;
    assert!(
        refusal["error"].is_object(),
        "the second stream is refused, not served: {refusal}"
    );
    assert_eq!(
        refusal["error"]["code"],
        json!(RATE_LIMITED),
        "the refusal is the RETRYABLE -32005, not the non-retryable -32600 it \
         answered with before 113-18: {refusal}"
    );
    let message = refusal["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("already open for this subscription id"),
        "the refusal names the real reason: {refusal}"
    );
    assert!(
        !message.contains("too many concurrent"),
        "this is a DUPLICATE refusal, not a cap refusal: {refusal}"
    );

    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;

    let delivered = first.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed"),
        "the FIRST stream survived the duplicate registration"
    );
    assert_eq!(
        subscription_id_of(&delivered),
        Some(&json!(1)),
        "and is still tagged with its own subscriptionId"
    );

    drop(first);
    drop(second);
    handle.abort();
}

/// Dropping a client connection reclaims BOTH the registry entry and the
/// concurrency permit, with no explicit unregister call anywhere.
#[tokio::test]
async fn disconnect_releases_registry_slot() {
    let (addr, handle) = spawn(server_with_two_principals()).await;

    let mut headers = listen_headers();
    headers.push(("authorization".to_string(), "Bearer capped".to_string()));

    // Open streams up to the per-principal cap. The cap is a private constant,
    // so this walks up until the server refuses.
    let mut held = Vec::new();
    let mut refusal = None;
    for id in 0..16 {
        let mut stream = SseStream::open(
            addr,
            &headers,
            &listen_body(json!(id), &json!({ "toolsListChanged": true })),
        )
        .await;
        let first = stream.expect_json().await;
        if first["error"].is_object() {
            refusal = Some(first);
            break;
        }
        held.push(stream);
    }
    let refusal = refusal.expect("the per-principal cap must refuse an N+1th stream");
    assert!(
        refusal["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("too many concurrent"),
        "the refusal names the concurrency bound: {refusal}"
    );
    assert!(!held.is_empty(), "some streams were accepted first");

    // Disconnect ONE client and let the server observe the closed socket.
    drop(held.pop().expect("at least one open stream"));

    // The RAII guard releases asynchronously (the server notices the dropped
    // connection), so poll for the reclaimed slot rather than sleeping blindly.
    let mut accepted = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut probe = SseStream::open(
            addr,
            &headers,
            &listen_body(json!(99), &json!({ "toolsListChanged": true })),
        )
        .await;
        let first = probe.expect_json().await;
        if first["error"].is_object() {
            drop(probe);
            continue;
        }
        assert_eq!(first["method"], json!(ACKNOWLEDGED_METHOD));
        held.push(probe);
        accepted = true;
        break;
    }
    assert!(
        accepted,
        "a disconnect must release the registry entry AND the permit"
    );

    drop(held);
    handle.abort();
}

// ===========================================================================
// D-113-N — the listen route agrees with the MRTR ingress about what an
// unauthenticated caller is.
// ===========================================================================

/// An unauthenticated `subscriptions/listen` on an auth-configured server is
/// REFUSED, not handed a private anonymous identity.
///
/// This is the row `resolve_mrtr_principal` has always answered `None` for
/// (`(None, has_auth_provider = true)`), and which the listen route answered
/// `Some(anon#N)` for until plan 113-23. Two ingress paths on ONE server
/// disagreeing about identity is the defect; agreeing is the fix.
#[tokio::test]
async fn unauthenticated_listen_is_refused_on_an_auth_configured_server() {
    let (addr, handle) = spawn(server_with_optional_auth()).await;

    // No `authorization` header at all — the provider returns `Ok(None)`, so the
    // request reaches the listen route with `auth_context: None`.
    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(41), &json!({ "toolsListChanged": true })),
    )
    .await;

    assert_eq!(
        stream.status, 200,
        "-32003 is DELIBERATELY unremapped: it is not in v2_status_for_code's \
         400 arm, so it answers at HTTP 200 with a JSON-RPC error body exactly \
         like the three RATE_LIMITED listen refusals. Remapping it to 401 would \
         change the status of every other emitter of that code on this transport"
    );
    assert_ne!(
        stream.header("content-type"),
        Some("text/event-stream"),
        "no stream body is opened for a refused caller"
    );

    let refusal = stream.expect_json().await;
    assert_eq!(
        refusal["error"]["code"],
        json!(AUTHENTICATION_REQUIRED),
        "the refusal is -32003, the same fail-closed answer the MRTR ingress \
         gives on this server: {refusal}"
    );
    assert_eq!(
        refusal["id"],
        json!(41),
        "the ORIGINAL request id is echoed: {refusal}"
    );
    assert!(
        refusal["result"].is_null(),
        "a refusal carries no result: {refusal}"
    );

    drop(stream);
    handle.abort();
}

/// The DELIBERATE divergence: a server with NO auth provider still serves an
/// unauthenticated listen, and still keeps its full global capacity.
///
/// This is the regression guard for the third row of `resolve_listen_principal`.
/// Without it, a later "cleanup" collapses both `None` rows onto the MRTR
/// ingress's single shared `ANONYMOUS_PRINCIPAL` and quietly caps a no-auth
/// server at `MAX_LISTEN_STREAMS_PER_PRINCIPAL` (4) concurrent streams instead
/// of `MAX_LISTEN_STREAMS_TOTAL` (64) — the local/dev configuration the shipped
/// `s47_v2_stateless_mrtr` / `s48_v2_mrtr_client` examples use. The AAD-binding
/// reason MRTR needs a stable principal simply does not apply to a
/// concurrency-accounting key.
#[tokio::test]
async fn unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider() {
    let (addr, handle) = spawn(server_with(advertising(Some("tools.listChanged")))).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(42), &json!({ "toolsListChanged": true })),
    )
    .await;

    assert_eq!(
        stream.status, 200,
        "a no-auth server still serves the stream"
    );
    assert_eq!(
        stream.header("content-type"),
        Some("text/event-stream"),
        "and it really is a stream, not a refusal body"
    );
    let ack = stream.expect_json().await;
    assert_eq!(
        ack["method"],
        json!(ACKNOWLEDGED_METHOD),
        "the acknowledgement arrives exactly as before the D-113-N fix: {ack}"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(42)));

    // Five concurrent anonymous streams — one MORE than
    // MAX_LISTEN_STREAMS_PER_PRINCIPAL. All are served, which is the capacity
    // claim above stated as an assertion rather than a comment: had the two rows
    // been unified onto one shared principal, the fifth would be refused.
    let mut held = vec![stream];
    for id in 43..47 {
        let mut extra = SseStream::open(
            addr,
            &listen_headers(),
            &listen_body(json!(id), &json!({ "toolsListChanged": true })),
        )
        .await;
        let frame = extra.expect_json().await;
        assert_eq!(
            frame["method"],
            json!(ACKNOWLEDGED_METHOD),
            "anonymous stream {id} must be served: a per-stream principal means \
             MAX_LISTEN_STREAMS_PER_PRINCIPAL does not bind here: {frame}"
        );
        held.push(extra);
    }

    // Sockets first, then the accept loop, then WAIT for it: several concurrent
    // streams make runtime teardown slow enough to trip nextest's 100 ms leak
    // timeout otherwise, which is noise rather than signal.
    drop(held);
    handle.abort();
    let _ = handle.await;
}

/// The D-113-N harm, reproduced and then denied: one unauthenticated caller can
/// no longer take the whole global listen budget.
///
/// Before the fix each of these attempts minted its OWN `anon#N`, so the
/// per-principal cap never bound and the first `MAX_LISTEN_STREAMS_TOTAL` (64)
/// of them were SERVED — after which the authenticated subscriber below was
/// refused `RATE_LIMITED`. The second half of this test is the point: the harm
/// D-113-N names is starvation of authenticated subscribers, so the test has to
/// show them un-starved, not merely show the anonymous caller refused.
#[tokio::test]
async fn one_unauthenticated_caller_cannot_exhaust_the_global_listen_budget() {
    // `MAX_LISTEN_STREAMS_TOTAL` is 64 and `pub(crate)`, so it is spelled out
    // here; the count deliberately EXCEEDS it, because a run that stopped at the
    // per-principal cap (4) would not reproduce the global exhaustion the defect
    // is about.
    const ATTEMPTS: i64 = 68;

    let (addr, handle) = spawn(server_with_optional_auth()).await;

    // Held, not dropped: under the pre-fix behaviour these connections really do
    // occupy registry slots, and releasing them as we went would hide the
    // exhaustion this test exists to reproduce.
    let mut held = Vec::new();
    for id in 0..ATTEMPTS {
        let mut stream = SseStream::open(
            addr,
            &listen_headers(),
            &listen_body(json!(id), &json!({ "toolsListChanged": true })),
        )
        .await;
        let frame = stream.expect_json().await;
        assert_eq!(
            frame["error"]["code"],
            json!(AUTHENTICATION_REQUIRED),
            "unauthenticated attempt {id} must be REFUSED, never granted a \
             private uncapped anon#N principal: {frame}"
        );
        held.push(stream);
    }

    // The load-bearing half: an authenticated subscriber is un-starved.
    let mut authenticated_headers = listen_headers();
    authenticated_headers.push(("authorization".to_string(), "Bearer carol".to_string()));
    let mut authenticated = SseStream::open(
        addr,
        &authenticated_headers,
        &listen_body(json!(1), &json!({ "toolsListChanged": true })),
    )
    .await;
    assert_eq!(authenticated.status, 200);
    assert_eq!(
        authenticated.header("content-type"),
        Some("text/event-stream"),
        "the authenticated subscriber gets a real stream, not a refusal body"
    );
    let ack = authenticated.expect_json().await;
    assert_eq!(
        ack["method"],
        json!(ACKNOWLEDGED_METHOD),
        "an authenticated subscriber still registers after {ATTEMPTS} \
         unauthenticated attempts — the global budget was never consumed: {ack}"
    );
    assert_eq!(subscription_id_of(&ack), Some(&json!(1)));

    // See the teardown note on the no-auth-provider twin: 68+ sockets make
    // runtime teardown slow enough to trip nextest's leak timeout otherwise.
    drop(authenticated);
    drop(held);
    handle.abort();
    let _ = handle.await;
}

// ===========================================================================
// Finding 5 (113-SPEC-RECHECK-ADDENDUM-2026-07-26) — what pmcp ACTUALLY emits
// for `io.modelcontextprotocol/subscriptionId`.
//
// The draft schema makes the key REQUIRED on `SubscriptionsListenResultMeta`
// (the teardown result) but OPTIONAL on `NotificationMetaObject` — absent for
// notifications not delivered via a subscription. HTTP-07's wording ("every
// delivered notification carries `subscriptionId` tagging") therefore has to be
// MEASURED against both halves, not assumed. These two tests are the
// measurement; the verdict is recorded in the addendum.
// ===========================================================================

/// All THREE listen frame classes carry the tag, and all three carry the SAME
/// id as the `subscriptions/listen` request that opened the stream.
///
/// Equality with the request id is asserted everywhere, not mere presence: a
/// frame tagged with the WRONG subscription id is worse than an untagged one —
/// it routes a client's notification onto the wrong subscription.
///
/// The terminal result is reached through
/// [`Server::close_subscription_streams`], which is the ONLY one of the three
/// closure triggers that can emit one (a client disconnect has no peer left to
/// send to, and the overflow policy has no buffer slot left).
#[tokio::test]
async fn subscription_id_is_emitted_on_all_three_listen_frame_classes() {
    let server = Arc::new(Mutex::new(server_with(advertising(Some(
        "tools.listChanged",
    )))));
    let (addr, handle) = spawn_shared(Arc::clone(&server)).await;

    let mut stream = SseStream::open(
        addr,
        &listen_headers(),
        &listen_body(json!(77), &json!({ "toolsListChanged": true })),
    )
    .await;

    // (a) The mandatory acknowledgement — `SubscriptionAcknowledgedParams`.
    let ack = stream.expect_json().await;
    assert_eq!(ack["method"], json!(ACKNOWLEDGED_METHOD));
    assert_eq!(
        subscription_id_of(&ack),
        Some(&json!(77)),
        "class (a) acknowledgement: params._meta carries the request's own id: {ack}"
    );

    // (b) A delivered change notification — `NotificationMetaObject`, the half
    // the schema makes OPTIONAL.
    server
        .lock()
        .await
        .send_notification(ServerNotification::ToolsChanged)
        .await;
    let delivered = stream.expect_json().await;
    assert_eq!(
        delivered["method"],
        json!("notifications/tools/list_changed")
    );
    assert_eq!(
        subscription_id_of(&delivered),
        Some(&json!(77)),
        "class (b) delivered notification: params._meta carries the SAME id: {delivered}"
    );

    // (c) The terminal `SubscriptionsListenResult` — `_meta` is REQUIRED here.
    server.lock().await.close_subscription_streams();
    let terminal = stream.expect_json().await;
    assert!(
        terminal["result"].is_object(),
        "class (c) is a RESULT, not a notification: {terminal}"
    );
    assert_eq!(
        terminal["id"],
        json!(77),
        "the terminal result answers the original listen request: {terminal}"
    );
    assert_eq!(
        terminal["result"]["_meta"][SUBSCRIPTION_ID_META_KEY],
        json!(77),
        "class (c) teardown result: _meta is REQUIRED and carries the id: {terminal}"
    );
    assert_eq!(subscription_id_of(&terminal), Some(&json!(77)));

    drop(stream);
    handle.abort();
    let _ = handle.await;
}

/// A tool that reports progress, so the off-stream probe has a real
/// server-originated notification to observe.
struct ProgressTool;

#[async_trait::async_trait]
impl pmcp::ToolHandler for ProgressTool {
    async fn handle(&self, _args: Value, extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        extra
            .report_progress(1.0, Some(2.0), Some("halfway".to_string()))
            .await?;
        Ok(json!({ "answer": "ok" }))
    }
}

/// Drive a `tools/call` carrying a progress token over an IN-PROCESS DUPLEX
/// transport and return the raw wire frame of the first notification the server
/// emits.
///
/// # Why this transport and not the HTTP one
///
/// On `StreamableHttpServer` the listen registry is the ONLY server→client
/// notification sink: that transport never calls `Server::run`, so
/// `notification_tx` stays `None` and `Server::send_notification` reaches
/// nothing else. The non-listen delivery path therefore has to be observed on a
/// `Server::run` transport, where `notification_tx` IS wired — and
/// `notifications/progress` is a good probe precisely because
/// `subscription_kind_of` classifies it as request-scoped, so the listen
/// registry excludes it STRUCTURALLY.
///
/// The frame is re-encoded through `pmcp::shared::transport::serialize_message`,
/// the crate's own single source of truth for the on-the-wire JSON-RPC encoding,
/// so this measures what a peer would actually receive rather than an ad-hoc
/// re-serialization of the enum.
async fn off_stream_notification_frame() -> Value {
    use duplex::DuplexTransport;
    use pmcp::shared::transport::serialize_message;
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::notifications::ProgressToken;
    use pmcp::types::tools::CallToolRequest;
    use pmcp::types::{
        ClientCapabilities, ClientNotification, ClientRequest, Implementation, InitializeRequest,
        Notification, Request, RequestId, RequestMeta,
    };

    let server = Server::builder()
        .name("v2-subscriptions-off-stream")
        .version("1.0.0")
        .capabilities(advertising(Some("tools.listChanged")))
        .tool("progress", ProgressTool)
        .build()
        .expect("server builds");

    let (mut client, server_transport) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_transport).await;
    });

    client
        .send(TransportMessage::Request {
            id: RequestId::from(1i64),
            // `InitializeRequest::new` defaults `protocol_version` to
            // `LATEST_PROTOCOL_VERSION`, which IS `V1`; the struct is
            // `#[non_exhaustive]`, so the constructor is the forward-compatible
            // way to build it.
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("off-stream-probe", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        })
        .await
        .expect("initialize sent");
    let _initialize_result = receive_bounded(&mut client).await;
    client
        .send(TransportMessage::Notification(Notification::Client(
            ClientNotification::Initialized,
        )))
        .await
        .expect("initialized sent");

    let mut call = CallToolRequest::new("progress", json!({}));
    call._meta = Some(
        RequestMeta::new().with_progress_token(ProgressToken::String("off-stream".to_string())),
    );
    client
        .send(TransportMessage::Request {
            id: RequestId::from(2i64),
            request: Request::Client(Box::new(ClientRequest::CallTool(call))),
        })
        .await
        .expect("tools/call sent");

    loop {
        let message = receive_bounded(&mut client).await;
        if matches!(message, TransportMessage::Notification(_)) {
            let bytes = serialize_message(&message).expect("the frame serializes");
            return serde_json::from_slice(&bytes).expect("the frame is JSON");
        }
    }
}

/// One duplex read, bounded by [`FRAME_TIMEOUT`] so a wedged probe FAILS rather
/// than hangs — the same doctrine every `SseStream` read in this file follows.
async fn receive_bounded(client: &mut duplex::DuplexTransport) -> pmcp::shared::TransportMessage {
    use pmcp::shared::Transport;

    tokio::time::timeout(FRAME_TIMEOUT, client.receive())
        .await
        .expect("a frame arrived within the timeout")
        .expect("the duplex peer is alive")
}

/// The OTHER half of Finding 5: a notification NOT delivered over a listen
/// stream carries NO `subscriptionId`.
///
/// The schema makes the key OPTIONAL on `NotificationMetaObject` precisely so a
/// notification with no subscription can omit it, so a pmcp that stamped the tag
/// universally would be emitting a subscription id for a notification that
/// belongs to no subscription. It does not: the tag is written in exactly one
/// place (`tag_notification_with_subscription_id`, called only from the listen
/// registry's fan-out), so it reaches only frames delivered on a stream.
///
/// If this assertion ever fails, that is a FINDING to record — not something to
/// "fix" by changing the emission. Over-tagging would be a wire-behaviour change
/// and belongs to a decision, not to plan 113-23's fence.
#[tokio::test]
async fn a_notification_not_delivered_over_a_listen_stream_carries_no_subscription_id() {
    let frame = off_stream_notification_frame().await;

    assert_eq!(
        frame["method"],
        json!("notifications/progress"),
        "the probe observed the request-scoped notification it drove: {frame}"
    );
    assert_eq!(
        subscription_id_of(&frame),
        None,
        "a notification with no subscription must carry NO subscriptionId — the \
         key is OPTIONAL on NotificationMetaObject, and pmcp writes it in exactly \
         one place (the listen registry's fan-out): {frame}"
    );
    assert!(
        !frame.to_string().contains(SUBSCRIPTION_ID_META_KEY),
        "the key must not appear ANYWHERE in the off-stream frame, not merely \
         outside `params._meta`: {frame}"
    );
}

/// `resources/subscribe` and `resources/unsubscribe` are GONE on v2.
#[tokio::test]
async fn v2_resources_subscribe_gone() {
    let (addr, handle) = spawn(server_with(advertising(Some("resources.subscribe")))).await;

    for method in ["resources/subscribe", "resources/unsubscribe"] {
        let response = post(
            addr,
            &v2_headers(method, ""),
            &v2_body(method, json!(1), json!({ "uri": "mem://greeting" })),
        )
        .await;
        assert_eq!(response.status, 404, "{method} is retired on v2");
        assert_eq!(
            response.body["error"]["code"],
            json!(METHOD_NOT_FOUND),
            "{method}: -32601"
        );
        assert_eq!(
            response.body["id"],
            json!(1),
            "{method}: original id echoed"
        );
    }

    handle.abort();
}

/// The v1 `resources/subscribe` flow still works on the SAME server.
///
/// v1 CONTROL — gated behind `v1-compat` (Phase 117). It mints a session id to
/// prove the v2 retirement above is additive rather than a blanket removal; on a
/// `--no-default-features --features full-v2` build there are no sessions to
/// mint, which is the severance itself. Gated per-TEST so the 18 v2
/// subscription tests in this file keep RUNNING on the severed build.
#[cfg(feature = "v1-compat")]
#[tokio::test]
async fn v1_subscribe_unchanged() {
    use common::v2::v1_body;

    let (addr, handle) = spawn(server_with(advertising(Some("resources.subscribe")))).await;

    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(1),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "0.0.0" },
            }),
        ),
    )
    .await;
    assert_eq!(init.status, 200, "the v1 handshake still works");
    let session = init
        .mcp_session_id
        .clone()
        .expect("v1 mints a session id (HTTP-01 leaves v1 untouched)");

    let subscribe = post(
        addr,
        &[
            ("mcp-session-id".to_string(), session.clone()),
            ("mcp-protocol-version".to_string(), V1.to_string()),
        ],
        &v1_body(
            "resources/subscribe",
            json!(2),
            json!({ "uri": "mem://greeting" }),
        ),
    )
    .await;
    assert_eq!(subscribe.status, 200, "v1 subscribe is untouched");
    assert!(
        subscribe.body["error"].is_null(),
        "v1 subscribe must not be retired: {}",
        subscribe.body
    );

    handle.abort();
}
