//! The CLIENT half of the v1 session stream — CONF-09, phase 118.2 plan 01.
//!
//! `tests/http_peer_roundtrip.rs` proves the SERVER half: a real
//! `StreamableHttpServer` does carry a server-to-client channel, measured by a
//! raw HTTP client. This file asks the mirror question and flips the direction:
//! does **pmcp's own client** open that channel at all, and once open, does it
//! READ it as a live stream rather than as a body it waits to end?
//!
//! Both halves of the answer were MEASURED against the unfixed tree before this
//! file existed, and both were "no":
//!
//! * A recording TCP listener driven through a real `pmcp::ClientBuilder`
//!   handshake observed the request lines `["POST / HTTP/1.1", "POST / HTTP/1.1"]`
//!   and a **GET count of 0**. The `start_sse(None)` call that should open the
//!   session stream sat inside `if !response.status().is_success()`, and `202
//!   Accepted` **is** a success status, so that branch was dead code (Defect A).
//! * Against a `text/event-stream` that delivers one complete frame and then
//!   holds the socket open, `start_sse(None)` did not return within 3 seconds and
//!   the already-delivered event never reached `Transport::receive()`: the reader
//!   awaited a whole-body `collect()`, which completes only at end-of-body, and a
//!   session stream has no end-of-body (Defect B).
//!
//! # The SECOND collect site (plan 03)
//!
//! Fences 8-10 ask the same question of the OTHER whole-body collect: the POST
//! that answers `text/event-stream`. In `StreamableHTTP` such a POST stays open for
//! the whole call and can carry notifications **and server-to-client requests**
//! before its result frame. Collecting it whole means progress arrives only after
//! the call ends — and an in-tool elicitation over a POST stream deadlocks
//! outright, because the client cannot answer a request it has not parsed yet.
//!
//! # Reliability doctrine
//!
//! EPHEMERAL PORT ([`RecordingServer::start`] binds `127.0.0.1:0` and reads the
//! address back), READINESS (`start()` binds before returning — never a fixed
//! sleep as a synchronization device), SHUTDOWN (the listener is dropped, then
//! the accept task is aborted, then awaited). EVERY await that crosses the wire
//! is wrapped in [`tokio::time::timeout`].
//!
//! That last rule is load-bearing here in a way it is not everywhere. Against the
//! unfixed tree these fences **hang** rather than fail — an unbounded await on a
//! stream that never ends does not return at all — and a hung test reads as a
//! slow test in CI rather than as a red one, which would silently defeat the
//! RED-first requirement this file exists to satisfy.
//!
//! The two deliberate sleeps in this file are [`QUIET`] waits used to prove that
//! NOTHING happened (no second GET; no further byte written by a stalled
//! producer). They are observation windows, not synchronization devices: every
//! wait for something to HAPPEN goes through [`wait_for`], which polls under
//! [`BOUND`].

#![cfg(all(
    feature = "streamable-http",
    feature = "v1-compat",
    not(target_arch = "wasm32")
))]

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::Transport;
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use pmcp::types::{
    CancelledNotification, ClientCapabilities, ClientNotification, ClientRequest, Notification,
    Request, RequestId, TransportMessage,
};
use pmcp::ClientBuilder;

// ===========================================================================
// Bounds. Every one of them is an upper bound on a wire operation or an
// observation window, never a synchronization device.
// ===========================================================================

/// Upper bound on any single operation that crosses the wire.
const BOUND: Duration = Duration::from_secs(5);

/// How long a listener is watched to prove NOTHING arrives.
const QUIET: Duration = Duration::from_millis(600);

/// How often [`wait_for`] re-evaluates its predicate.
///
/// A poll interval INSIDE a [`BOUND`]-limited wait, not a settle time: the test
/// proceeds the moment the condition holds, and fails at `BOUND` if it never
/// does. Nothing here depends on the interval's value.
const POLL: Duration = Duration::from_millis(10);

/// The parser bound fence 5 drives the peer past.
///
/// Small so the fence is fast: the bound is what the overflow message must NAME,
/// and a 16 MiB default would mean pushing 16 MiB to observe it.
const OVERSIZED_PARSER_BOUND: usize = 4096;

/// Payload marker that must never appear in an error message.
///
/// Embedded in the bytes fences 5 and 7 push. Both errors are UNTRUSTED-INPUT
/// errors: the overflow one names the limit and echoes nothing, and the
/// parse-failure one truncates any echo to a 200-character bound. Either way this
/// marker — placed past that bound — must not reach a client's logs (ASVS V7).
const SENTINEL: &str = "SENTINEL-0123456789abcdef-ABCDEF";

/// Per-frame payload size for the backpressure fence.
///
/// Chosen so that [`BACKPRESSURE_FRAME_COUNT`] frames (4 MiB in total) exceed the
/// sum of everything that can absorb them without the consumer running: the
/// client's own bounded receive queue (64 messages, so 1 MiB at this frame size),
/// hyper's read buffer, and both kernel socket buffers. A frame small enough that
/// the whole push fits in those buffers makes the fence VACUOUS — the producer
/// never blocks and nothing proves the reader ever hit a full queue.
///
/// Deliberately larger than the 8 KiB the plan sized this at: 128 x 8 KiB is
/// 1 MiB total against a queue that alone holds 512 KiB, which leaves too little
/// slack over an auto-tuned loopback socket buffer for the stall assertion to be
/// reliable.
const BACKPRESSURE_FRAME_BYTES: usize = 16 * 1024;

/// How many frames the backpressure fence pushes.
///
/// Comfortably more than the receive-queue capacity the transport mints, so the
/// queue is provably saturated rather than merely filled.
const BACKPRESSURE_FRAME_COUNT: usize = 256;

/// Upper bound on draining every backpressure frame.
///
/// Larger than [`BOUND`] because it covers 4 MiB of transfer plus
/// [`BACKPRESSURE_FRAME_COUNT`] channel hand-offs, not one wire operation.
const DRAIN_BOUND: Duration = Duration::from_secs(30);

// ===========================================================================
// The recording listener.
//
// Raw TCP rather than a served `StreamableHttpServer` because these fences must
// control the wire precisely: answer a notification POST with a bare `202`, hold
// a chunked `text/event-stream` open with no terminating chunk, and push frames
// on demand. Plans 03 and 04 extend this harness rather than rewriting it.
// ===========================================================================

/// Everything the accept loop and the fences share.
struct Shared {
    /// Every observed HTTP request line, in arrival order.
    ///
    /// The request LINE only — method, path and version. Never a header, so a
    /// harness change cannot leak an `Authorization` header into a CI log.
    request_lines: Mutex<Vec<String>>,
    /// Every POST body, parsed as JSON, in arrival order.
    ///
    /// A POST body on this harness is a JSON-RPC frame the TEST itself caused the
    /// client to emit — never a credential. Headers are still never recorded, so
    /// the `Authorization`-leak rule above is untouched; this records the one
    /// thing fence 9 has to assert on, namely the client's ANSWER to a
    /// server-to-client request. Stored parsed rather than raw so a malformed
    /// body cannot put arbitrary bytes into an assertion message.
    post_bodies: Mutex<Vec<Value>>,
    /// Accepted GET connections still open from the SERVER's side.
    open_gets: AtomicUsize,
    /// Accepted POST connections answered with `text/event-stream` and still open
    /// from the SERVER's side (plan 03).
    open_posts: AtomicUsize,
    /// Frames written to a socket AND flushed. A stalled producer is one whose
    /// count stops advancing.
    frames_written: AtomicUsize,
    /// The frame source, handed to whichever GET connection is live and put back
    /// when that connection ends.
    frames_rx: Mutex<Option<mpsc::Receiver<String>>>,
    /// The SECOND frame source, for the POST-answered stream. Deliberately
    /// separate from `frames_rx`: fence 9 runs a full handshake, so a GET session
    /// stream and a POST stream are open at the SAME time and a shared source
    /// would deliver each frame to whichever happened to poll first.
    post_frames_rx: Mutex<Option<mpsc::Receiver<String>>>,
    /// JSON-RPC methods whose POST is answered with a chunked
    /// `text/event-stream` that never ends, rather than with a bare `202`.
    ///
    /// Empty by default, so the GET fences see exactly the harness they always
    /// did.
    sse_post_methods: Mutex<HashSet<String>>,
}

/// A recording HTTP/1.1 listener on an ephemeral port.
struct RecordingServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    frames: mpsc::Sender<String>,
    post_frames: mpsc::Sender<String>,
    accept: JoinHandle<()>,
}

impl RecordingServer {
    /// Bind, spawn the accept loop, and return BEFORE any client connects.
    async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds an ephemeral loopback port");
        let addr = listener
            .local_addr()
            .expect("the bound address is readable back");

        // Capacity 1: the test-side producer must not be able to buffer the whole
        // push, or `frames_written` would measure the channel rather than the
        // socket.
        let (frames, frames_rx) = mpsc::channel::<String>(1);
        let (post_frames, post_frames_rx) = mpsc::channel::<String>(1);
        let shared = Arc::new(Shared {
            request_lines: Mutex::new(Vec::new()),
            post_bodies: Mutex::new(Vec::new()),
            open_gets: AtomicUsize::new(0),
            open_posts: AtomicUsize::new(0),
            frames_written: AtomicUsize::new(0),
            frames_rx: Mutex::new(Some(frames_rx)),
            post_frames_rx: Mutex::new(Some(post_frames_rx)),
            sse_post_methods: Mutex::new(HashSet::new()),
        });

        let accept = tokio::spawn({
            let shared = Arc::clone(&shared);
            async move {
                while let Ok((stream, _)) = listener.accept().await {
                    tokio::spawn(serve(stream, Arc::clone(&shared)));
                }
            }
        });

        Self {
            addr,
            shared,
            frames,
            post_frames,
            accept,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// Answer the POST carrying `method` with a chunked `text/event-stream` that
    /// never ends, instead of with the bare `202`.
    ///
    /// Call BEFORE the client connects. This is the whole of plan 03's new
    /// harness behaviour: the GET side is untouched, so fences 1-7 see exactly
    /// the listener they always did.
    fn answer_post_with_sse(&self, method: &str) {
        self.shared
            .sse_post_methods
            .lock()
            .insert(method.to_string());
    }

    /// Every POST body observed so far, parsed as JSON.
    fn post_bodies(&self) -> Vec<Value> {
        self.shared.post_bodies.lock().clone()
    }

    /// POST connections answered with `text/event-stream` and still open.
    fn open_post_connections(&self) -> usize {
        self.shared.open_posts.load(Ordering::SeqCst)
    }

    /// Push one already-formatted SSE frame down the live POST body.
    async fn push_post_frame(&self, frame: String) {
        timeout(BOUND, self.post_frames.send(frame))
            .await
            .expect("the POST stream accepts a pushed frame within BOUND")
            .expect("the POST frame channel is open");
    }

    fn request_lines(&self) -> Vec<String> {
        self.shared.request_lines.lock().clone()
    }

    fn get_lines(&self) -> usize {
        self.request_lines()
            .iter()
            .filter(|line| line.starts_with("GET "))
            .count()
    }

    fn open_get_connections(&self) -> usize {
        self.shared.open_gets.load(Ordering::SeqCst)
    }

    fn frames_written(&self) -> usize {
        self.shared.frames_written.load(Ordering::SeqCst)
    }

    /// A sender the test can hand to its own producer task.
    fn frame_sender(&self) -> mpsc::Sender<String> {
        self.frames.clone()
    }

    /// Push one already-formatted SSE frame down the live GET body.
    async fn push_frame(&self, frame: String) {
        timeout(BOUND, self.frames.send(frame))
            .await
            .expect("a live GET connection accepts a pushed frame within BOUND")
            .expect("the frame channel is open");
    }

    async fn shutdown(self) {
        self.accept.abort();
        let _ = self.accept.await;
    }
}

/// Serve ONE accepted connection: record its request line, then answer by method.
async fn serve(stream: TcpStream, shared: Arc<Shared>) {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.unwrap_or(0) == 0 {
        return;
    }
    let request_line = request_line.trim_end().to_string();
    shared.request_lines.lock().push(request_line.clone());

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body).await.is_err() {
        return;
    }

    if request_line.starts_with("GET ") {
        serve_get(reader, write_half, &shared).await;
        return;
    }

    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    shared.post_bodies.lock().push(value.clone());

    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let streams = shared.sse_post_methods.lock().contains(&method);
    if streams {
        serve_post_sse(reader, write_half, &shared).await;
    } else {
        serve_post(&mut write_half, &value).await;
    }
}

/// Answer a POST: the initialize result, or a bare `202 Accepted`.
///
/// Every answer carries `connection: close` so hyper cannot reuse the socket for
/// a later request — one connection per request keeps `request_lines()` an
/// exact, ordered record of what the client did.
async fn serve_post(write_half: &mut WriteHalf<TcpStream>, value: &Value) {
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");

    let response = if method == "initialize" {
        let result = json!({
            "jsonrpc": "2.0",
            "id": value.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "protocolVersion": LATEST_PROTOCOL_VERSION,
                "capabilities": {},
                "serverInfo": { "name": "recording-server", "version": "0.0.0" },
            },
        })
        .to_string();
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nMcp-Session-Id: s1\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{result}",
            result.len()
        )
    } else {
        // Every notification — initialized or not — is acknowledged with the
        // bare 202 the spec prescribes. Which of them reopens the session stream
        // is the CLIENT's decision, and fence 2 is what measures it.
        "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
    };

    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.flush().await;
    let _ = write_half.shutdown().await;
}

/// Answer a GET with the never-ending `text/event-stream` session stream.
async fn serve_get(
    reader: BufReader<ReadHalf<TcpStream>>,
    write_half: WriteHalf<TcpStream>,
    shared: &Shared,
) {
    serve_sse_body(
        reader,
        write_half,
        shared,
        &shared.open_gets,
        &shared.frames_rx,
    )
    .await;
}

/// Answer a POST with a chunked `text/event-stream` that NEVER ends (plan 03).
///
/// The SAME shape as [`serve_get`], on the SECOND stream source and the SECOND
/// open-connection counter. This is the response a `StreamableHTTP` server gives
/// to a `tools/call` it wants to narrate: notifications and server-to-client
/// requests ride it, and the result frame closes it.
async fn serve_post_sse(
    reader: BufReader<ReadHalf<TcpStream>>,
    write_half: WriteHalf<TcpStream>,
    shared: &Shared,
) {
    serve_sse_body(
        reader,
        write_half,
        shared,
        &shared.open_posts,
        &shared.post_frames_rx,
    )
    .await;
}

/// Write the `text/event-stream` head, then pump pushed frames until the peer
/// closes — counting the connection as open for exactly that span.
///
/// No terminating zero-length chunk is ever written. The connection ends only
/// when the client closes it — which is exactly what makes this the shape a
/// whole-body `collect()` cannot read.
async fn serve_sse_body(
    mut reader: BufReader<ReadHalf<TcpStream>>,
    mut write_half: WriteHalf<TcpStream>,
    shared: &Shared,
    open: &AtomicUsize,
    source: &Mutex<Option<mpsc::Receiver<String>>>,
) {
    open.fetch_add(1, Ordering::SeqCst);

    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\r\n";
    if write_half.write_all(head.as_bytes()).await.is_ok() && write_half.flush().await.is_ok() {
        let mut frames_rx = source.lock().take();
        pump_frames(&mut reader, &mut write_half, shared, frames_rx.as_mut()).await;
        // Hand the source back so a LATER stream on this same harness can use it.
        if let Some(rx) = frames_rx {
            *source.lock() = Some(rx);
        }
    }

    open.fetch_sub(1, Ordering::SeqCst);
}

/// Write pushed frames until the peer closes the connection.
///
/// The read half is polled alongside the frame source purely as a CLOSE
/// detector: a client that aborts its reader sends nothing further, so a `read`
/// of zero bytes is the only signal that the connection is gone. Without it a
/// connection parked on `recv()` would never notice, and
/// `open_get_connections()` would never fall.
async fn pump_frames(
    reader: &mut BufReader<ReadHalf<TcpStream>>,
    write_half: &mut WriteHalf<TcpStream>,
    shared: &Shared,
    mut frames_rx: Option<&mut mpsc::Receiver<String>>,
) {
    let mut scratch = [0u8; 1024];
    loop {
        match frames_rx.as_mut() {
            Some(rx) => {
                tokio::select! {
                    frame = rx.recv() => {
                        let Some(frame) = frame else { return };
                        if write_chunk(write_half, &frame).await.is_err() {
                            return;
                        }
                        shared.frames_written.fetch_add(1, Ordering::SeqCst);
                    },
                    read = reader.read(&mut scratch) => {
                        if !matches!(read, Ok(bytes) if bytes > 0) {
                            return;
                        }
                    },
                }
            },
            None => {
                if !matches!(reader.read(&mut scratch).await, Ok(bytes) if bytes > 0) {
                    return;
                }
            },
        }
    }
}

/// Write one HTTP/1.1 chunked-transfer chunk and flush it.
async fn write_chunk(write_half: &mut WriteHalf<TcpStream>, payload: &str) -> std::io::Result<()> {
    write_half
        .write_all(format!("{:X}\r\n", payload.len()).as_bytes())
        .await?;
    write_half.write_all(payload.as_bytes()).await?;
    write_half.write_all(b"\r\n").await?;
    write_half.flush().await
}

// ===========================================================================
// Fence helpers.
// ===========================================================================

/// A transport pointed at the recording listener, at the default body cap.
fn transport_for(server: &RecordingServer) -> StreamableHttpTransport {
    StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(
            url::Url::parse(&server.url()).expect("the harness URL parses"),
        )
        .build(),
    )
}

/// Poll `predicate` until it holds, or [`BOUND`] elapses.
async fn wait_for(mut predicate: impl FnMut() -> bool) -> bool {
    timeout(BOUND, async {
        loop {
            if predicate() {
                return;
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .is_ok()
}

/// The `notifications/initialized` frame, as a typed transport message.
fn initialized() -> TransportMessage {
    TransportMessage::Notification(Notification::Client(ClientNotification::Initialized))
}

/// A notification that is NOT `notifications/initialized`.
fn cancelled() -> TransportMessage {
    TransportMessage::Notification(Notification::Cancelled(CancelledNotification::new(
        RequestId::from("not-initialized"),
    )))
}

/// One complete `message` SSE frame carrying a progress notification.
fn progress_frame(id: &str, token: &str, progress: usize, pad: Option<String>) -> String {
    let mut params = json!({ "progressToken": token, "progress": progress });
    if let Some(pad) = pad {
        params["message"] = Value::String(pad);
    }
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": params,
    })
    .to_string();
    format!("id: {id}\nevent: message\ndata: {payload}\n\n")
}

/// A `ping` REQUEST, as a typed transport message.
///
/// `ping` because it is the smallest spec-defined request that carries an id, and
/// because the harness routes a POST to its streaming answer by METHOD — so the
/// fences below name exactly one method and nothing else changes shape.
fn ping_request(id: &str) -> TransportMessage {
    TransportMessage::Request {
        id: RequestId::from(id),
        request: Request::Client(Box::new(ClientRequest::Ping)),
    }
}

/// One complete `message` SSE frame carrying a JSON-RPC RESULT for `id`.
fn result_frame(id: &Value) -> String {
    let payload = json!({ "jsonrpc": "2.0", "id": id, "result": {} }).to_string();
    format!("event: message\ndata: {payload}\n\n")
}

/// One complete `message` SSE frame carrying a SERVER-TO-CLIENT `ping` request.
///
/// The shape `tests/http_peer_roundtrip.rs` proves a real server emits on its
/// server-to-client channel: a JSON-RPC frame with BOTH an `id` and a `method`.
/// `ping` is answered by `Client`'s host dispatch with no registered handler at
/// all (spec MUST: an inbound ping gets an empty-object success), so the fence
/// measures the transport's parse-and-deliver path rather than a handler
/// registration.
fn server_request_frame(id: &str) -> String {
    let payload = json!({ "jsonrpc": "2.0", "id": id, "method": "ping" }).to_string();
    format!("event: message\ndata: {payload}\n\n")
}

/// The id of the client's own outbound `ping` request, if it has been `POSTed`.
fn outbound_ping_id(server: &RecordingServer) -> Option<Value> {
    server.post_bodies().into_iter().find_map(|body| {
        (body.get("method").and_then(Value::as_str) == Some("ping"))
            .then(|| body.get("id").cloned())
            .flatten()
    })
}

/// Whether the client has `POSTed` a JSON-RPC RESULT for `id`.
fn answered(server: &RecordingServer, id: &str) -> bool {
    server.post_bodies().iter().any(|body| {
        body.get("id").and_then(Value::as_str) == Some(id) && body.get("result").is_some()
    })
}

/// The `progress` value of a received progress notification.
fn progress_of(message: &TransportMessage) -> Option<f64> {
    match message {
        TransportMessage::Notification(Notification::Progress(progress)) => Some(progress.progress),
        _ => None,
    }
}

/// Handshake a real `pmcp` client against the harness, keeping an OBSERVER
/// clone of the transport.
///
/// The transport is `Clone` and every field behind the clone is an `Arc`, so the
/// observer shares the receive queue with the clone the client owns — which is
/// what lets a fence call `Transport::receive()` directly while the client sits
/// idle. Both are returned because dropping the client would drop its transport
/// clone.
async fn handshake(
    server: &RecordingServer,
) -> (
    pmcp::Client<StreamableHttpTransport>,
    StreamableHttpTransport,
) {
    let transport = transport_for(server);
    let observer = transport.clone();
    let mut client = ClientBuilder::new(transport).build();
    timeout(BOUND, client.initialize(ClientCapabilities::default()))
        .await
        .expect("initialize returns within BOUND — a hang here IS the defect")
        .expect("the recording harness answers initialize");
    (client, observer)
}

/// Open the session stream WITHOUT a full handshake, by sending the initialized
/// notification directly.
///
/// The handshake is only one of two ways to reach the 202 branch, and the fences
/// that measure the READER do not need a client at all. Driving the transport
/// keeps them free of the client's own `receive()` loop, which would otherwise
/// compete for the very queue they read.
async fn open_stream(transport: &mut StreamableHttpTransport) {
    timeout(BOUND, transport.send(initialized()))
        .await
        .expect("the initialized notification completes within BOUND")
        .expect("the harness answers it 202");
}

// ===========================================================================
// Fence 1 — Defect A: the stream is opened at all.
// ===========================================================================

#[tokio::test]
async fn client_issues_a_get_sse_stream_after_the_initialized_notification() {
    let server = RecordingServer::start().await;
    let (_client, _observer) = handshake(&server).await;

    let opened = wait_for(|| server.get_lines() >= 1).await;
    assert!(
        opened,
        "pmcp's own client answered a 202 to notifications/initialized and never opened the \
         session stream. Observed request lines: {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 2 — the predicate is `notifications/initialized`, not "is a
// notification".
// ===========================================================================

#[tokio::test]
async fn a_non_initialized_notification_does_not_reopen_the_stream() {
    let server = RecordingServer::start().await;
    let (_client, mut observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the handshake must open the stream first. Observed: {:?}",
        server.request_lines()
    );

    timeout(BOUND, observer.send(cancelled()))
        .await
        .expect("the cancelled notification completes within BOUND")
        .expect("the harness answers it 202");

    tokio::time::sleep(QUIET).await;
    assert_eq!(
        server.get_lines(),
        1,
        "a notification that is NOT notifications/initialized must not tear down and re-open the \
         session stream — start_sse aborts the live reader as its first act, so a broad predicate \
         is an active regression rather than mere waste. Observed request lines: {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 3 — Defect B: an event arrives while the stream is STILL OPEN.
// ===========================================================================

#[tokio::test]
async fn a_frame_delivered_on_a_live_stream_reaches_receive_before_the_stream_ends() {
    let server = RecordingServer::start().await;
    // `handshake` itself asserts that `initialize` returned inside BOUND: on the
    // unfixed tree the initialized notification's `start_sse` never returns, so
    // the handshake is where the hang surfaces.
    let (_client, mut observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the session stream must be open before a frame can be delivered on it. Observed: {:?}",
        server.request_lines()
    );

    server
        .push_frame(progress_frame("e1", "fence-3", 1, None))
        .await;

    let message = timeout(BOUND, observer.receive())
        .await
        .expect("a frame delivered on a LIVE stream reaches receive() within BOUND")
        .expect("the frame parses into a transport message");

    assert_eq!(
        progress_of(&message),
        Some(1.0),
        "expected the pushed progress notification, got {message:?}"
    );
    assert!(
        server.open_get_connections() >= 1,
        "the event must arrive while the GET body is still open — no terminating chunk was ever \
         written"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 4 — no orphan readers survive a re-open.
// ===========================================================================

#[tokio::test]
async fn repeated_initialized_notifications_leave_exactly_one_live_reader() {
    let server = RecordingServer::start().await;
    let mut transport = transport_for(&server);

    for _ in 0..3 {
        open_stream(&mut transport).await;
    }

    assert!(
        wait_for(|| server.get_lines() >= 3).await,
        "three initialized notifications must open three session streams. Observed: {:?}",
        server.request_lines()
    );
    assert_eq!(
        server.get_lines(),
        3,
        "exactly three, not more. Observed: {:?}",
        server.request_lines()
    );
    assert!(
        wait_for(|| server.open_get_connections() == 1).await,
        "start_sse aborts the previous reader as its first act, so exactly ONE GET connection may \
         remain open; {} were still open",
        server.open_get_connections()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 5 — D-02: an oversized chunk ends the stream, naming the limit and
// echoing nothing.
// ===========================================================================

#[tokio::test]
async fn an_oversized_chunk_ends_the_stream_with_an_error_naming_the_limit() {
    let server = RecordingServer::start().await;
    let mut transport =
        transport_for(&server).with_max_collected_body_bytes(OVERSIZED_PARSER_BOUND);
    open_stream(&mut transport).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the session stream must be open. Observed: {:?}",
        server.request_lines()
    );

    // No terminating blank line: the peer streams an unbounded `data:` line, the
    // realistic shape of the attack the parser bound exists to refuse.
    let oversized = format!("data: {SENTINEL}{}", "A".repeat(OVERSIZED_PARSER_BOUND * 2));
    server.push_frame(oversized).await;

    let error = timeout(BOUND, transport.receive())
        .await
        .expect("the terminal error reaches receive() within BOUND")
        .expect_err("an over-bound chunk must END the stream, not be silently dropped");

    let text = error.to_string();
    assert!(
        text.contains(&OVERSIZED_PARSER_BOUND.to_string()),
        "the overflow error must NAME the limit it enforced; got {text:?}"
    );
    assert!(
        !text.contains(SENTINEL),
        "the overflow error must echo NO body content — the bytes that tripped the bound are \
         exactly the untrusted input the rule keeps out of a client's logs; got {text:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 6 — D-04: a slow consumer stalls the producer and nothing is dropped.
// ===========================================================================

#[tokio::test]
async fn a_slow_consumer_stalls_the_producer_and_no_frame_is_dropped() {
    let server = RecordingServer::start().await;
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the session stream must be open. Observed: {:?}",
        server.request_lines()
    );

    let pad = "A".repeat(BACKPRESSURE_FRAME_BYTES);
    let sender = server.frame_sender();
    let producer = tokio::spawn(async move {
        for seq in 0..BACKPRESSURE_FRAME_COUNT {
            if sender
                .send(progress_frame(
                    &format!("bp-{seq}"),
                    "backpressure",
                    seq,
                    Some(pad.clone()),
                ))
                .await
                .is_err()
            {
                return seq;
            }
        }
        BACKPRESSURE_FRAME_COUNT
    });

    // THE STALL. Nothing calls `receive()` yet, so the reader fills its bounded
    // queue, stops polling the body, the TCP window closes and the SERVER's write
    // blocks. Two samples QUIET apart prove it stopped advancing rather than
    // merely running slowly — without this the fence would pass without ever
    // exercising backpressure.
    tokio::time::sleep(QUIET).await;
    let first = server.frames_written();
    tokio::time::sleep(QUIET).await;
    let second = server.frames_written();

    assert!(
        first >= 1,
        "the producer never started: wrote {first} frames"
    );
    assert!(
        second < BACKPRESSURE_FRAME_COUNT,
        "the producer wrote all {BACKPRESSURE_FRAME_COUNT} frames with nothing draining them, so \
         no queue was ever saturated and this fence measured nothing"
    );
    assert_eq!(
        first, second,
        "the producer must be BLOCKED, not merely slow: it wrote {first} frames and was still at \
         {second} a full QUIET later"
    );

    // Now drain. Every frame must arrive, in order, with none lost.
    let mut seen: Vec<f64> = Vec::with_capacity(BACKPRESSURE_FRAME_COUNT);
    timeout(DRAIN_BOUND, async {
        while seen.len() < BACKPRESSURE_FRAME_COUNT {
            let message = transport
                .receive()
                .await
                .expect("every queued frame is delivered");
            seen.push(progress_of(&message).expect("each frame is a progress notification"));
        }
    })
    .await
    .expect("the whole backlog drains within DRAIN_BOUND");

    let written = timeout(BOUND, producer)
        .await
        .expect("the producer completes once the consumer drains")
        .expect("the producer task did not panic");
    assert_eq!(written, BACKPRESSURE_FRAME_COUNT);
    assert_eq!(server.frames_written(), BACKPRESSURE_FRAME_COUNT);

    let expected: Vec<f64> = (0..BACKPRESSURE_FRAME_COUNT)
        .map(|seq| seq as f64)
        .collect();
    assert_eq!(
        seen, expected,
        "the receive queue carries sampling / roots / elicit REQUESTS: a dropped or reordered one \
         strands a correlation until it times out"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 7 — D-05: a frame the client cannot parse ends the stream with a named,
// bounded-echo error rather than being silently dropped.
// ===========================================================================

#[tokio::test]
async fn an_unparseable_frame_ends_the_stream_with_a_named_error() {
    let server = RecordingServer::start().await;
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the session stream must be open. Observed: {:?}",
        server.request_lines()
    );

    // Well-formed JSON, but no `method`, `result` or `error` member — so it is
    // not a JSON-RPC frame at all. The sentinel sits past the 200-character echo
    // bound.
    let payload = json!({ "jsonrpc": "2.0", "pad": format!("{}{SENTINEL}", "A".repeat(500)) });
    server
        .push_frame(format!("id: bad\nevent: message\ndata: {payload}\n\n"))
        .await;

    let error = timeout(BOUND, transport.receive())
        .await
        .expect("the terminal error reaches receive() within BOUND")
        .expect_err(
            "an unparseable frame must END the stream: it may be a server-to-client REQUEST, and \
             swallowing it hangs both ends with no signal",
        );

    let text = error.to_string();
    assert!(
        !text.contains(SENTINEL),
        "any echoed frame text must be truncated to the 200-character bound; got {text:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 8 — D-01 at the SECOND collect site: a notification carried on a POST
// `text/event-stream` response arrives BEFORE the result frame.
// ===========================================================================

#[tokio::test]
async fn a_notification_on_a_post_sse_body_arrives_before_the_result_frame() {
    let server = RecordingServer::start().await;
    server.answer_post_with_sse("ping");
    let mut transport = transport_for(&server);

    // The send itself is the first measurement. Against the whole-body collect
    // this never returns: `post_body` awaits a body whose end the server has
    // deliberately not written.
    timeout(BOUND, transport.send(ping_request("fence-8")))
        .await
        .expect(
            "a POST answered with text/event-stream must not block the send until the body ends — \
             the response stays open for the whole call by construction, so a hang here IS the \
             defect",
        )
        .expect("the harness answers the POST with a 200 text/event-stream");

    assert!(
        wait_for(|| server.open_post_connections() >= 1).await,
        "the POST stream must be open. Observed request lines: {:?}",
        server.request_lines()
    );

    server
        .push_post_frame(progress_frame("p1", "fence-8", 1, None))
        .await;

    let message = timeout(BOUND, transport.receive())
        .await
        .expect("a notification carried on a LIVE POST stream reaches receive() within BOUND")
        .expect("the frame parses into a transport message");
    assert_eq!(
        progress_of(&message),
        Some(1.0),
        "expected the pushed progress notification, got {message:?}"
    );
    assert!(
        server.open_post_connections() >= 1,
        "the notification must arrive while the POST body is STILL OPEN — no result frame has \
         been written yet, and no terminating chunk ever is"
    );

    // ONLY NOW the result frame, so "before" above is an ordering fact rather
    // than a coincidence of timing.
    server
        .push_post_frame(result_frame(&json!("fence-8")))
        .await;
    let response = timeout(BOUND, transport.receive())
        .await
        .expect("the result frame reaches receive() within BOUND")
        .expect("the result frame parses into a transport message");
    assert!(
        matches!(response, TransportMessage::Response(_)),
        "expected the result frame after the notification, got {response:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 9 — the in-tool elicitation deadlock: a server-to-client REQUEST on a
// POST body is parsed, and answered, before the call's result frame.
// ===========================================================================

/// The id the harness's server-to-client request carries.
const SERVER_REQUEST_ID: &str = "srv-to-client-1";

#[tokio::test]
async fn a_server_to_client_request_on_a_post_sse_body_is_parsed_before_the_result_frame() {
    let server = RecordingServer::start().await;
    server.answer_post_with_sse("ping");
    let (client, _observer) = handshake(&server).await;

    // A real `Client` call, because the answer is produced by `Client`'s host
    // dispatch loop — the loop that only runs while a request is outstanding.
    // That is precisely the in-tool situation: the server asks a question in the
    // middle of answering one.
    let calling = tokio::spawn(async move { client.ping().await });

    assert!(
        wait_for(|| outbound_ping_id(&server).is_some()).await,
        "the client's own request must reach the harness. Observed bodies: {:?}",
        server.post_bodies()
    );
    let outbound_id = outbound_ping_id(&server).expect("the outbound request carries an id");
    assert!(
        wait_for(|| server.open_post_connections() >= 1).await,
        "the POST stream answering that request must be open. Observed request lines: {:?}",
        server.request_lines()
    );

    server
        .push_post_frame(server_request_frame(SERVER_REQUEST_ID))
        .await;

    assert!(
        wait_for(|| answered(&server, SERVER_REQUEST_ID)).await,
        "the client must ANSWER a server-to-client request carried on a POST stream while that \
         stream is still open. A whole-body collect deadlocks here outright: the answer is what \
         lets the server finish, and the server finishing is what ends the body the client is \
         waiting to read. Observed POST bodies: {:?}",
        server.post_bodies()
    );
    assert!(
        server.open_post_connections() >= 1,
        "and it must answer while the POST body is still open — the result frame has not been \
         written"
    );

    // Now let the call finish, so the fence proves the ordering rather than
    // merely that an answer eventually appeared.
    server.push_post_frame(result_frame(&outbound_id)).await;
    let result = timeout(BOUND, calling)
        .await
        .expect("the call completes once its result frame arrives")
        .expect("the calling task did not panic");
    assert!(
        result.is_ok(),
        "the result frame must complete the outstanding call; got {result:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 10 — no orphan POST reader survives its owning transport.
// ===========================================================================

#[tokio::test]
async fn a_post_sse_reader_terminates_when_its_owning_transport_is_dropped() {
    let server = RecordingServer::start().await;
    server.answer_post_with_sse("ping");
    let mut transport = transport_for(&server);

    timeout(BOUND, transport.send(ping_request("fence-10")))
        .await
        .expect("the POST returns as soon as the stream is open — a hang here IS the defect")
        .expect("the harness answers the POST with a 200 text/event-stream");
    assert!(
        wait_for(|| server.open_post_connections() >= 1).await,
        "the POST stream must be open before it can be orphaned. Observed request lines: {:?}",
        server.request_lines()
    );

    // Drop EVERY transport clone. There is deliberately no `Drop` impl to rely
    // on: `StreamableHttpTransport` is `Clone` and shares its abort handle, so a
    // `Drop` impl would let one clone's drop kill the original's stream. The
    // contract is the send-failure signal instead — dropping the last clone
    // drops the receive queue's `Receiver`, so the reader's next
    // `sender.send(..).await` returns `Err` and the reader RETURNS.
    drop(transport);

    // One frame, purely so the reader HAS a next send to fail. A reader that
    // ignores its send result keeps the body alive and this fence stays red.
    server
        .push_post_frame(progress_frame("orphan", "fence-10", 1, None))
        .await;

    assert!(
        wait_for(|| server.open_post_connections() == 0).await,
        "a returned reader drops the response body, which closes the connection — {} POST \
         stream(s) were still open at the SERVER, i.e. a task is still reading a peer-controlled \
         socket with nothing left to deliver to",
        server.open_post_connections()
    );

    // And it STAYS closed: the client writes nothing on a POST connection after
    // its request body, so the only thing that could move either counter is a
    // surviving reader.
    let written = server.frames_written();
    tokio::time::sleep(QUIET).await;
    assert_eq!(
        server.open_post_connections(),
        0,
        "no POST stream may re-open after the transport is gone"
    );
    assert_eq!(
        server.frames_written(),
        written,
        "nothing may still be consuming the orphaned stream a full QUIET later"
    );

    server.shutdown().await;
}
