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
    CancelledNotification, ClientCapabilities, ClientNotification, Notification, RequestId,
    TransportMessage,
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
    /// The request LINE only — method, path and version. Never a header and
    /// never a body, so a harness change cannot leak an `Authorization` header
    /// into a CI log.
    request_lines: Mutex<Vec<String>>,
    /// Accepted GET connections still open from the SERVER's side.
    open_gets: AtomicUsize,
    /// Frames written to a socket AND flushed. A stalled producer is one whose
    /// count stops advancing.
    frames_written: AtomicUsize,
    /// The frame source, handed to whichever GET connection is live and put back
    /// when that connection ends.
    frames_rx: Mutex<Option<mpsc::Receiver<String>>>,
}

/// A recording HTTP/1.1 listener on an ephemeral port.
struct RecordingServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    frames: mpsc::Sender<String>,
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
        let shared = Arc::new(Shared {
            request_lines: Mutex::new(Vec::new()),
            open_gets: AtomicUsize::new(0),
            frames_written: AtomicUsize::new(0),
            frames_rx: Mutex::new(Some(frames_rx)),
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
            accept,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.addr)
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
    } else {
        serve_post(&mut write_half, &body).await;
    }
}

/// Answer a POST: the initialize result, or a bare `202 Accepted`.
///
/// Every answer carries `connection: close` so hyper cannot reuse the socket for
/// a later request — one connection per request keeps `request_lines()` an
/// exact, ordered record of what the client did.
async fn serve_post(write_half: &mut WriteHalf<TcpStream>, body: &[u8]) {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
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

/// Answer a GET with a chunked `text/event-stream` that NEVER ends.
///
/// No terminating zero-length chunk is ever written. The connection ends only
/// when the client closes it — which is exactly what makes this the shape a
/// whole-body `collect()` cannot read.
async fn serve_get(
    mut reader: BufReader<ReadHalf<TcpStream>>,
    mut write_half: WriteHalf<TcpStream>,
    shared: &Shared,
) {
    shared.open_gets.fetch_add(1, Ordering::SeqCst);

    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\r\n";
    if write_half.write_all(head.as_bytes()).await.is_ok() && write_half.flush().await.is_ok() {
        let mut frames_rx = shared.frames_rx.lock().take();
        pump_frames(&mut reader, &mut write_half, shared, frames_rx.as_mut()).await;
        // Hand the source back so a LATER GET on this same harness can use it.
        if let Some(rx) = frames_rx {
            *shared.frames_rx.lock() = Some(rx);
        }
    }

    shared.open_gets.fetch_sub(1, Ordering::SeqCst);
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
