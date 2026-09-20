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

use std::collections::{HashSet, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use pmcp::shared::streamable_http::{
    AuthProvider, StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
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
/// Chosen so that [`BACKPRESSURE_FRAME_COUNT`] frames (32 MiB in total) exceed
/// the sum of everything that can absorb them without the consumer running: the
/// client's own bounded receive queue (64 messages, so 4 MiB at this frame size),
/// hyper's read buffer, and both kernel socket buffers. A frame small enough that
/// the whole push fits in those buffers makes the fence VACUOUS — the producer
/// never blocks and nothing proves the reader ever hit a full queue.
///
/// # Sized for LINUX, not for the machine it was written on
///
/// This was 16 KiB x 256 = 4 MiB, which stalls reliably on macOS and NOT on a
/// GitHub Actions Linux runner, where it wrote all 256 frames and tripped the
/// fence's own "measured nothing" guard on its first CI run. Loopback socket
/// buffers there are AUTO-TUNED and far larger than macOS's: `tcp_wmem` and
/// `tcp_rmem` maxima are commonly 4 MiB and 6 MiB RESPECTIVELY, so ~10 MiB of
/// kernel buffering alone could swallow the whole 4 MiB push before the server's
/// write ever blocked.
///
/// The budget now is 32 MiB pushed against roughly 15 MiB of absorbers (4 MiB
/// queue + ~10 MiB kernel + hyper), leaving ~17 MiB of slack rather than a
/// deficit. Do not shrink either constant without re-deriving that sum: the
/// failure mode is not a red test but a GREEN one that measured nothing, which
/// the guard at the assertion site exists to catch.
const BACKPRESSURE_FRAME_BYTES: usize = 64 * 1024;

/// How many frames the backpressure fence pushes.
///
/// Comfortably more than the receive-queue capacity the transport mints, so the
/// queue is provably saturated rather than merely filled. Raised from 256
/// alongside [`BACKPRESSURE_FRAME_BYTES`] — see that constant for the Linux
/// socket-buffer arithmetic that made the old total absorbable.
const BACKPRESSURE_FRAME_COUNT: usize = 512;

/// Upper bound on draining every backpressure frame.
///
/// Larger than [`BOUND`] because it covers 32 MiB of transfer plus
/// [`BACKPRESSURE_FRAME_COUNT`] channel hand-offs, not one wire operation.
/// Raised with the volume, and with headroom for a loaded shared CI runner.
const DRAIN_BOUND: Duration = Duration::from_secs(60);

/// The reconnect budget the transport ships, restated here (plan 04, D-03).
///
/// `MAX_SSE_RECONNECT_ATTEMPTS` is a PRIVATE constant on
/// `src/shared/streamable_http.rs` — every knob on that transport is private
/// precisely so that none of them is a semver event — so an integration test
/// cannot import it. Restating it is safe because a drift is caught by the very
/// fence that uses it: the observed GET count would stop being
/// `1 + SHIPPED_RECONNECT_BUDGET` and
/// [`reconnect_gives_up_after_the_retry_budget_with_a_named_error`] would fail
/// with both numbers in its message.
const SHIPPED_RECONNECT_BUDGET: usize = 2;

/// Upper bound on an operation that has to outlast the WHOLE reconnect schedule.
///
/// The shipped curve is the reference client's: 1000 ms, then 1500 ms
/// (`initialReconnectionDelay` x `reconnectionDelayGrowFactor^attempt`), i.e.
/// 2.5 s of deliberate sleeping before the budget is spent. [`BOUND`] is an upper
/// bound on ONE wire operation and is deliberately not stretched to cover that.
///
/// Deliberately NOT solved with a test-only "make the backoff shorter" knob on
/// the transport: such a knob would have to be `pub` to be reachable from an
/// integration test (`tests/` is a separate crate), which would put a
/// test-shaped affordance into pmcp's public API and into its
/// `cargo semver-checks` verdict. Waiting out the shipped curve measures the
/// SHIPPED curve.
const RECONNECT_BOUND: Duration = Duration::from_secs(20);

/// How long a listener is watched to prove NO further GET arrives.
///
/// Must outlast everything still scheduled at the moment of the close — the
/// remaining backoff plus the reconnect it would have issued — or the fence
/// would pass merely by looking too early. Longer than [`QUIET`] for exactly
/// that reason.
const RECONNECT_QUIET: Duration = Duration::from_secs(4);

/// The event id fence 11 pushes and then requires on the reconnect GET.
const RESUMED_FROM: &str = "e7";

/// The FLOOR the transport puts under any reconnect wait, restated here
/// (plan 14, CR-01).
///
/// `MIN_SSE_RECONNECT_DELAY` is a PRIVATE constant on
/// `src/shared/streamable_http.rs`, for the same never-a-semver-event reason
/// [`SHIPPED_RECONNECT_BUDGET`] is, so an integration test cannot import it.
/// Restating it follows that established precedent — and changing ONE without
/// the other reds [`reconnect_with_one_delivered_frame_and_zero_retry_stays_bounded`],
/// whose FLOOR arm measures the shipped spacing against this value and reports
/// both numbers.
const SHIPPED_MIN_RECONNECT_DELAY: Duration = Duration::from_millis(500);

/// Upper bound on the WHOLE zero-retry reconnect schedule (plan 14, CR-01).
///
/// Deliberately tighter than [`RECONNECT_BOUND`]. Against the UNFIXED tree this
/// fence's peer drives the client through a zero-delay reconnect loop — a
/// `retry: 0` that is honoured verbatim, plus a budget refunded on every
/// delivered frame — so the bound is what caps how many loopback connections the
/// RED capture is allowed to open (T-118.2-14-04). Post-fix the schedule is
/// `SHIPPED_RECONNECT_BUDGET` x [`SHIPPED_MIN_RECONNECT_DELAY`], i.e. about one
/// second, so 8 s is generous for the GREEN run and still short for the RED one.
const ZERO_RETRY_BOUND: Duration = Duration::from_secs(8);

/// How long the zero-retry fence watches to prove NO further GET arrives.
///
/// Deliberately tighter than [`RECONNECT_QUIET`], for the same RED-capture
/// reason as [`ZERO_RETRY_BOUND`]: every extra second of observation against the
/// unfixed tree is another second of unbounded loopback connections. It only has
/// to outlast what is still scheduled at the moment the budget is spent, and
/// post-fix that is one [`SHIPPED_MIN_RECONNECT_DELAY`] — so 1.5 s is three
/// times the wait it must outlast.
const ZERO_RETRY_QUIET: Duration = Duration::from_millis(1500);

/// How long fence 17 waits for a call whose answer bore the WRONG id
/// (plan 15, CR-02).
///
/// # This bound is NOT a synchronisation device, and the asymmetry is the reason
///
/// Post-fix the call NEVER completes: the correlation check discards the
/// mis-addressed frame and keeps waiting for the response that actually carries
/// this request's id, which this peer never sends. The bound therefore only
/// decides how long the fence waits before concluding so.
///
/// Pre-fix the call completes in MILLISECONDS — the receive loop returns the
/// first response frame it pops, whatever its id — so CI load makes this fence
/// SAFER rather than flakier: a slow machine cannot turn a returned wrong answer
/// into a timeout, it can only make the (already-passing) wait longer. That is
/// why two seconds is enough and why the fence never asserts on an upper time
/// bound as its SUBJECT.
const MISMATCH_BOUND: Duration = Duration::from_secs(2);

/// The id fence 17's peer answers with instead of the one it was asked with.
///
/// Deliberately self-describing rather than a plausible-looking `"1"`: it is a
/// literal no `pmcp` client request can ever have produced (`call_tool` mints a
/// `RequestId::String` holding a UUID), so a failure message naming it reads as
/// "this frame was addressed to nobody" rather than as an off-by-one.
const MISMATCHED_CALL_ID: &str = "an-id-no-pmcp-client-request-ever-produced";

/// The `id` prefix every SPRAYED response frame carries (plan 22, CR-02).
///
/// The same self-describing rule [`MISMATCHED_CALL_ID`] states, plus a per-frame
/// serial appended by [`sprayed_unowned_answer`]: a failure message naming
/// `…-17` and one naming `…-31` are telling the reader about DIFFERENT frames,
/// which is what makes "how far did this budget get" readable straight off the
/// assertion text.
const UNOWNED_SPRAY_ID_PREFIX: &str = "an-id-no-pmcp-client-request-ever-produced-spray-";

/// How long a SECOND operation on the same client gets to complete while a
/// first one is stuck (plan 20, BLOCKER 2 half A; now shared by the three
/// liveness fences `a_mismatched_frame_does_not_block_another_operation_on_the_same_client`,
/// `an_outstanding_call_does_not_block_another_operation` and
/// `a_peer_that_never_writes_response_headers_does_not_serialise_the_client`).
///
/// # Deliberately BELOW the ceiling, and that is the whole design
///
/// `src/client/mod.rs`'s ceiling on frames nobody awaits is
/// `UNMATCHED_RESPONSE_TIMEOUT`, 10 s. This bound is 5 s, so a pass CANNOT be
/// manufactured by the ceiling firing, returning the stuck call and releasing
/// the transport guard as a side effect. The fence's subject is the guard being
/// released *while* the first operation is still stuck — the pump takes it,
/// races one `receive()` against `PUMP_RECEIVE_SLICE` (250 ms), and drops it —
/// not at the end of it. If this constant were ever raised above the ceiling
/// these fences would silently stop measuring liveness and start measuring the
/// bounded failure, which `debris_from_a_dead_call_does_not_charge_the_next_calls_budget`
/// owns.
///
/// The count bound `MAX_UNMATCHED_RESPONSES` (32) cannot reach these fences
/// either: each sprays at most one un-owned frame, so only the clock is in play.
///
/// On the RED tree the second operation NEVER returns — the guard was held
/// across an unbounded `receive().await` and re-taken on every `continue`
/// (receive path, closed by the router), and later across the POST's response
/// head (send path, closed by `Client::send_frame`). So CI load makes these
/// fences safer rather than flakier: a slow machine cannot turn a wedged client
/// into a pass, only a passing one into a longer wait, and 5 s is twenty
/// `PUMP_RECEIVE_SLICE`s of headroom over the one slice the probe must outlast.
const LOCK_PROBE_BOUND: Duration = Duration::from_secs(5);

/// How long fence 22 waits for a `receive()` that must NOT answer (plan 19,
/// BLOCKER 1).
///
/// # This bound is NOT a synchronisation device, and the asymmetry is the reason
///
/// It is the same asymmetry [`MISMATCH_BOUND`] records. Post-fix the wait NEVER
/// completes: a successful `start_sse` re-open clears the terminal latch, the
/// queue is empty and no POST-response reader is in flight, so `receive()` parks
/// and this bound only decides how long the fence waits before concluding so.
///
/// Pre-fix the stale answer arrives in MICROSECONDS — the latch is written once,
/// `Arc`-shared across every clone, and cleared by no constructor, no
/// `start_sse` and no `close`, so `drain_or_latch` surfaces it on the first
/// `Empty` poll. CI load therefore makes this fence SAFER rather than flakier: a
/// slow machine cannot turn a returned stale reason into a timeout, it can only
/// make the (already-passing) wait longer. The fence never asserts on an upper
/// time bound as its SUBJECT.
const LATCH_RESET_BOUND: Duration = Duration::from_secs(2);

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
    /// The `Last-Event-ID` value each GET carried, in arrival order (plan 04).
    ///
    /// The ONE header this harness records, recorded by NAME rather than by
    /// capturing the header block: the module's rule is that a harness change
    /// must not be able to leak an `Authorization` header into a CI log, and an
    /// allow-list of exactly one non-credential header keeps that rule true
    /// while still giving fence 11 the actual wire value to assert on. `None`
    /// means the GET carried no such header at all, which is itself the
    /// assertion a first-open (and a `full-v2` build) has to satisfy.
    get_cursors: Mutex<Vec<Option<String>>>,
    /// When set, every GET is answered with a `text/event-stream` head and a
    /// zero-length body — a session stream that ends the instant it opens.
    ///
    /// The proxy-blink shape D-03 exists for, reduced to its limit: an idle
    /// timeout that fires immediately, every time.
    close_get_on_accept: AtomicBool,
    /// When `Some(retry_ms)`, every GET is answered with a chunked
    /// `text/event-stream` carrying exactly ONE complete event — whose `retry:`
    /// field is `retry_ms` — and then ended (plan 14, CR-01).
    ///
    /// The arm [`close_get_on_accept`](Shared::close_get_on_accept) cannot reach:
    /// that mode delivers NOTHING, so the reader's `delivered` flag is always
    /// `false` and the budget-refund branch is never taken. This mode sets
    /// `delivered == true` on EVERY body, which is the half of CR-01 that turns a
    /// bounded retry budget into an unbounded one.
    one_frame_then_close: Mutex<Option<u64>>,
    /// When each GET was ACCEPTED, server-side, in arrival order.
    ///
    /// Pushed in the same place [`get_cursors`](Shared::get_cursors) is, so there
    /// is exactly one timestamp slot per accepted GET and the two vectors index
    /// alike. Server-side rather than client-side deliberately: the fence asserts
    /// on the spacing the PEER observed, which is the spacing that matters for a
    /// request flood.
    get_instants: Mutex<Vec<std::time::Instant>>,
    /// Bumped to end every LIVE SSE body cleanly, mid-flight.
    ///
    /// A `watch` rather than a `Notify` deliberately: `Notify::notify_waiters`
    /// wakes only the tasks ALREADY parked on it, so a signal raised between two
    /// iterations of the pump loop would be lost and the fence would hang until
    /// its bound rather than fail on its subject.
    close_live: watch::Sender<u64>,
    /// When set, the `initialize` result advertises a `tools` capability instead
    /// of the bare `{}` (plan 15, CR-02).
    ///
    /// `Client::call_tool` runs `assert_capability("tools", "tools/call")` BEFORE
    /// it puts a byte on the wire, so against a `{}`-capability handshake a
    /// call-tool fence would fail without ever reaching the transport — vacuous,
    /// and vacuous in the direction that LOOKS like the fence is working.
    ///
    /// Default OFF, so fences 1-15 see exactly the `initialize` result they
    /// always did.
    advertise_tools: AtomicBool,
    /// When set, an id-bearing non-`initialize` POST is answered with a JSON-RPC
    /// success instead of the bare `202` (plan 15, CR-02).
    ///
    /// Default OFF, for the same reason [`advertise_tools`](Shared::advertise_tools)
    /// is: fences 1-15 assert on a listener that answers every non-`initialize`
    /// POST `202 Accepted`.
    answer_calls: AtomicBool,
    /// When set, an answered call's JSON-RPC success is delivered over a
    /// `text/event-stream` POST response rather than as an
    /// `application/json` body (plan 19, BLOCKER 1).
    ///
    /// The shape fence 16 cannot reach, and the whole reason BLOCKER 1 shipped
    /// green. A JSON-answered POST lands its response on the receive queue
    /// SYNCHRONOUSLY inside `send()`, so the queue-wins-over-latch rule always
    /// fires; an SSE-answered POST is read by a DETACHED reader that is spawned
    /// after `post_body` has already returned `Ok(())`, so the queue is
    /// legitimately, transiently EMPTY while a real answer is still on the wire.
    ///
    /// Default OFF, so fences 1-20 see byte-identical answers to the ones they
    /// always did.
    answer_calls_with_sse: AtomicBool,
    /// When set, the NEXT answered call is addressed to [`MISMATCHED_CALL_ID`]
    /// rather than to the id it was asked with (plan 15, CR-02).
    ///
    /// CONSUMED on use, so exactly one call is mis-addressed and every later call
    /// is answered correctly. That is what lets fence 17 measure the correlation
    /// decision without also changing what a second call would see.
    next_call_id_is_mismatched: AtomicBool,
    /// How many calls this harness has ANSWERED, so call *n* and call *n+1* are
    /// distinguishable by the VALUE of their result rather than by their order.
    ///
    /// The desync fence 16 measures is "call 2 was handed call 1's result", and a
    /// result that is byte-identical between the two cannot detect it.
    calls_answered: AtomicUsize,
    /// Hold the FIRST answerable call's response until a later one has been
    /// flushed, so two concurrent calls are answered newest-first (plan 22).
    ///
    /// Without this a two-caller fence is order-dependent: if the transport
    /// happens to hand the first caller its OWN answer first, the defect never
    /// fires and the fence passes while measuring nothing.
    stagger_answers: AtomicBool,
    /// How many answerable calls have reached the stagger gate.
    answerable_gated: AtomicUsize,
    /// How many call answers have been written AND flushed to a socket.
    ///
    /// Distinct from [`Self::calls_answered`], which counts answers BUILT — the
    /// gate needs "the other answer is on the wire", not "it exists".
    answers_flushed: AtomicUsize,
    /// How many response frames addressed to ids NO request awaits are written
    /// immediately before each answered call's own answer frame (plan 22, CR-02).
    ///
    /// One entry per answered call, in order; the front is popped as each
    /// SSE-answered call is served and an exhausted schedule sprays nothing. That
    /// per-call shape is what lets ONE fence give call 1, call 2 and call 3
    /// different spray counts, which is the whole arithmetic the chain fence
    /// turns on.
    ///
    /// EMPTY by default, so every existing fence observes byte-identical bytes to
    /// the ones it observes today — the same rule
    /// [`answer_calls_with_sse`](Shared::answer_calls_with_sse) and
    /// [`advertise_tools`](Shared::advertise_tools) state.
    unowned_spray_schedule: Mutex<VecDeque<usize>>,
    /// The serial appended to each sprayed frame's id, so no two sprayed frames
    /// anywhere in a run share an id.
    unowned_sprayed: AtomicUsize,
    /// How many further POSTs are answered with a bare `401 Unauthorized`
    /// instead of being served normally (plan 25).
    ///
    /// Decremented on each POST it consumes, so a fence arms exactly as many
    /// 401s as it has concurrent callers and every LATER POST — the retries —
    /// is served the way it always was. ZERO by default, so every existing
    /// fence observes byte-identical answers, the same rule
    /// [`answer_calls_with_sse`](Shared::answer_calls_with_sse) states.
    unauthorized_posts_remaining: AtomicUsize,
    /// How many bare `401` answers this harness has written AND flushed.
    ///
    /// The observable a concurrency fence gates on: "both callers are inside
    /// their 401 recovery" is not something the client exposes, but "both POSTs
    /// were answered 401" is, and it happens strictly before it.
    unauthorized_answers: AtomicUsize,
    /// When `Some(grace)`, a GET's response HEAD is withheld until a SECOND GET
    /// has arrived or `grace` elapses, whichever comes first (plan 25).
    ///
    /// The grace is load-bearing and is NOT a settle time. A pure
    /// two-arrival barrier would DEADLOCK the serialised tree: there the second
    /// `start_sse` cannot put its GET on the wire until the first has finished,
    /// so a gate that waits for two arrivals would be measuring the fix by
    /// hanging on it. Releasing on arrival-count OR grace makes BOTH trees
    /// terminate, and the fence then asserts on the END state rather than on
    /// the gate.
    ///
    /// `None` by default, so every existing GET fence sees the listener it
    /// always did.
    get_head_rendezvous: Mutex<Option<Duration>>,
    /// When set, the NEXT answerable JSON POST parks BEFORE its response HEAD is
    /// written, instead of answering at once (plan 23).
    ///
    /// # Before the head, not after it — and that is the whole distinction
    ///
    /// Parking AFTER the head is what
    /// [`stagger_answers`](Shared::stagger_answers) already does, and
    /// [`serve_post_sse_answer`]'s own comment states what that shape measures:
    /// holding the FRAME lets `send()` RETURN and the answer arrive
    /// asynchronously, which exercises the RECEIVE path. The same comment names
    /// the shape this flag produces — "holding a JSON-answered POST blocks the
    /// client INSIDE `send()`, still holding the transport lock and never
    /// reaching the receive path at all". That is the SEND-path wedge, and this
    /// is the only gate that reaches it.
    ///
    /// CONSUMED on use — see [`await_stalled_post_head`] — so exactly one POST
    /// is stalled and every later POST, the probe's included, is served the way
    /// it always was. Default OFF, so every existing fence observes
    /// byte-identical bytes, the same rule
    /// [`answer_calls_with_sse`](Shared::answer_calls_with_sse) states.
    stall_next_answerable_post: AtomicBool,
    /// How many POSTs have ENTERED that park, counted at the gate.
    ///
    /// The observable a send-path fence has to wait on before it probes: "the
    /// stalled POST has arrived and is being held" is what makes the transport
    /// guard genuinely contended, and a probe that ran before it would measure
    /// an uncontended lock and pass vacuously — the failure mode fence 26
    /// guards against by name.
    stalled_post_arrivals: AtomicUsize,
    /// Bumped to release a parked POST early.
    ///
    /// A `watch` rather than a `Notify` for exactly the reason
    /// [`close_live`](Shared::close_live) records: `notify_waiters` wakes only
    /// the tasks ALREADY parked, so a release raised between the arming and the
    /// park would be lost and the fence would hang on the park bound instead of
    /// finishing cleanly.
    release_stalled_post: watch::Sender<u64>,
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
        let (close_live, _) = watch::channel(0u64);
        let (release_stalled_post, _) = watch::channel(0u64);
        let shared = Arc::new(Shared {
            request_lines: Mutex::new(Vec::new()),
            post_bodies: Mutex::new(Vec::new()),
            open_gets: AtomicUsize::new(0),
            open_posts: AtomicUsize::new(0),
            frames_written: AtomicUsize::new(0),
            frames_rx: Mutex::new(Some(frames_rx)),
            post_frames_rx: Mutex::new(Some(post_frames_rx)),
            sse_post_methods: Mutex::new(HashSet::new()),
            get_cursors: Mutex::new(Vec::new()),
            close_get_on_accept: AtomicBool::new(false),
            one_frame_then_close: Mutex::new(None),
            get_instants: Mutex::new(Vec::new()),
            close_live,
            advertise_tools: AtomicBool::new(false),
            answer_calls: AtomicBool::new(false),
            answer_calls_with_sse: AtomicBool::new(false),
            next_call_id_is_mismatched: AtomicBool::new(false),
            calls_answered: AtomicUsize::new(0),
            stagger_answers: AtomicBool::new(false),
            answerable_gated: AtomicUsize::new(0),
            answers_flushed: AtomicUsize::new(0),
            unowned_spray_schedule: Mutex::new(VecDeque::new()),
            unowned_sprayed: AtomicUsize::new(0),
            unauthorized_posts_remaining: AtomicUsize::new(0),
            unauthorized_answers: AtomicUsize::new(0),
            get_head_rendezvous: Mutex::new(None),
            stall_next_answerable_post: AtomicBool::new(false),
            stalled_post_arrivals: AtomicUsize::new(0),
            release_stalled_post,
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

    /// Answer every GET with a `text/event-stream` that ends the instant it
    /// opens, instead of one held open forever (plan 04).
    ///
    /// Call BEFORE the client connects. This is the D-03 subject reduced to its
    /// limit: an idle-timeout proxy that blinks every single time.
    fn close_get_streams_on_accept(&self) {
        self.shared
            .close_get_on_accept
            .store(true, Ordering::SeqCst);
    }

    /// Answer every GET with a `text/event-stream` that delivers exactly ONE
    /// complete event carrying `retry: retry_ms`, and then ends (plan 14, CR-01).
    ///
    /// Call BEFORE the client connects, mirroring
    /// [`close_get_streams_on_accept`](RecordingServer::close_get_streams_on_accept).
    /// Together the two modes are the pair the equivalence arm needs: a body that
    /// delivers ZERO events and a body that delivers exactly ONE must settle at
    /// the SAME bounded GET count.
    fn deliver_one_frame_then_close_each_get(&self, retry_ms: u64) {
        *self.shared.one_frame_then_close.lock() = Some(retry_ms);
    }

    /// When each GET was accepted, server-side, in arrival order.
    fn get_instants(&self) -> Vec<std::time::Instant> {
        self.shared.get_instants.lock().clone()
    }

    /// Advertise a `tools` capability in the `initialize` result (plan 15, CR-02).
    ///
    /// Call BEFORE the client connects, mirroring
    /// [`close_get_streams_on_accept`](RecordingServer::close_get_streams_on_accept).
    /// Without it `Client::call_tool` refuses locally — see
    /// [`Shared::advertise_tools`].
    fn advertise_tools(&self) {
        self.shared.advertise_tools.store(true, Ordering::SeqCst);
    }

    /// Answer every id-bearing non-`initialize` POST with a JSON-RPC success
    /// carrying a per-call distinguishable marker (plan 15, CR-02).
    ///
    /// Call BEFORE the client connects. The marker is what makes "call 2 received
    /// call 1's result" observable — see [`Shared::calls_answered`].
    fn answer_calls_with_an_echoing_result(&self) {
        self.shared.answer_calls.store(true, Ordering::SeqCst);
    }

    /// Deliver an answered call's JSON-RPC success over a `text/event-stream`
    /// POST response instead of as an `application/json` body (plan 19).
    ///
    /// Call BEFORE the client connects, and TOGETHER with
    /// [`answer_calls_with_an_echoing_result`](RecordingServer::answer_calls_with_an_echoing_result)
    /// — this switch decides how an answer is FRAMED, not whether there is one.
    ///
    /// The same per-answer [`call_marker`] numbering is reused, which is what
    /// makes "call 2 got call 2's marker" assertable on this path too. See
    /// [`Shared::answer_calls_with_sse`] for why this framing is the one that
    /// reaches BLOCKER 1.
    fn answer_calls_with_sse(&self) {
        self.shared
            .answer_calls_with_sse
            .store(true, Ordering::SeqCst);
    }

    /// Stop ending every GET at once and go back to holding it open (plan 19).
    ///
    /// The inverse of
    /// [`close_get_streams_on_accept`](RecordingServer::close_get_streams_on_accept),
    /// and the ONLY way to prove a RECOVERED transport: a fence has to move a
    /// harness from the budget-spending shape to the ordinary held-open shape
    /// MID-TEST, because a transport whose session stream never failed has no
    /// latch to clear. That recovery is precisely what the `start_sse` reset seam
    /// exists for.
    fn reopen_get_streams_held_open(&self) {
        self.shared
            .close_get_on_accept
            .store(false, Ordering::SeqCst);
    }

    /// Address the NEXT answered call to [`MISMATCHED_CALL_ID`] instead of to the
    /// id it was asked with (plan 15, CR-02).
    ///
    /// Call BEFORE the client connects. Consumed on use, so exactly one call is
    /// mis-addressed.
    /// Answer two concurrent calls NEWEST-FIRST (plan 22).
    ///
    /// The first answerable call's response is held until a later call's
    /// response has been flushed. Call BEFORE the client connects, together with
    /// [`answer_calls_with_an_echoing_result`](RecordingServer::answer_calls_with_an_echoing_result).
    ///
    /// This exists because the defect it measures is order-dependent. A client
    /// that pops a frame belonging to another caller only misbehaves when it
    /// pops the OTHER caller's frame first; left to chance, the fence would pass
    /// roughly half the time while measuring nothing.
    fn answer_calls_newest_first(&self) {
        self.shared.stagger_answers.store(true, Ordering::SeqCst);
    }

    /// Write `schedule[n]` response frames addressed to ids NOBODY awaits
    /// immediately BEFORE answered call *n*'s own answer frame (plan 22, CR-02).
    ///
    /// Call BEFORE the client connects, together with
    /// [`answer_calls_with_an_echoing_result`](RecordingServer::answer_calls_with_an_echoing_result)
    /// and
    /// [`answer_calls_with_sse`](RecordingServer::answer_calls_with_sse) — the
    /// spray rides the SSE-answered POST body, which is the only framing on which
    /// a frame can precede an answer that is still on the wire.
    ///
    /// Per-CALL rather than a single count because the chain fence needs three
    /// different counts on one client. An exhausted schedule sprays nothing, so a
    /// fence naming fewer counts than it makes calls is stating "and then behave
    /// normally" rather than falling off an edge.
    fn spray_unowned_frames_before_each_answer(&self, schedule: &[usize]) {
        *self.shared.unowned_spray_schedule.lock() = schedule.iter().copied().collect();
    }

    /// Answer the next `count` POSTs with a bare `401 Unauthorized` (plan 25).
    ///
    /// Call BEFORE the client connects. Every POST after those `count` — the
    /// retries the transport's single-shot refresh sends — is served exactly the
    /// way it always was, so a fence arming two 401s observes two recoveries and
    /// then ordinary behaviour rather than an endless auth loop.
    fn answer_the_next_posts_unauthorized(&self, count: usize) {
        self.shared
            .unauthorized_posts_remaining
            .store(count, Ordering::SeqCst);
    }

    /// How many bare `401` answers have been written AND flushed.
    fn unauthorized_answers(&self) -> usize {
        self.shared.unauthorized_answers.load(Ordering::SeqCst)
    }

    /// Withhold each GET's response head until a second GET arrives or `grace`
    /// elapses (plan 25).
    ///
    /// Call BEFORE the client connects. See
    /// [`get_head_rendezvous`](Shared::get_head_rendezvous) for why the grace is
    /// part of the construction and not a settle time.
    fn hold_each_get_head_until_two_arrive(&self, grace: Duration) {
        *self.shared.get_head_rendezvous.lock() = Some(grace);
    }

    /// Park the NEXT answerable JSON POST BEFORE its response head is written
    /// (plan 23).
    ///
    /// Call together with
    /// [`answer_calls_with_an_echoing_result`](RecordingServer::answer_calls_with_an_echoing_result)
    /// — this decides WHEN an answer's head is written, not whether there is
    /// one. Consumed by the first POST that reaches the gate, so exactly one is
    /// stalled; release it with
    /// [`release_the_stalled_post`](RecordingServer::release_the_stalled_post).
    /// See [`Shared::stall_next_answerable_post`] for why parking before the
    /// head is the only shape that reaches the SEND path.
    fn stall_the_next_answerable_post_before_its_head(&self) {
        self.shared
            .stall_next_answerable_post
            .store(true, Ordering::SeqCst);
    }

    /// How many POSTs have entered that park.
    fn stalled_posts(&self) -> usize {
        self.shared.stalled_post_arrivals.load(Ordering::SeqCst)
    }

    /// Release a parked POST so the run finishes without waiting out the park
    /// bound.
    fn release_the_stalled_post(&self) {
        self.shared
            .release_stalled_post
            .send_modify(|generation| *generation += 1);
    }

    fn answer_the_next_call_with_a_mismatched_id(&self) {
        self.shared
            .next_call_id_is_mismatched
            .store(true, Ordering::SeqCst);
    }

    /// End every LIVE SSE body cleanly, mid-flight (plan 04).
    ///
    /// Writes the terminating zero-length chunk and shuts the socket down, so
    /// the client sees an ordinary end-of-body rather than a truncation — the
    /// shape an idle-timeout proxy produces, and the one a reconnect must
    /// survive.
    fn end_live_sse_streams(&self) {
        self.shared
            .close_live
            .send_modify(|generation| *generation += 1);
    }

    /// The `Last-Event-ID` each observed GET carried, in arrival order.
    fn get_cursors(&self) -> Vec<Option<String>> {
        self.shared.get_cursors.lock().clone()
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

    /// How many call answers this harness has WRITTEN (plan 20, BLOCKER 2).
    ///
    /// Distinct from [`calls_observed`], which counts `tools/call` POST bodies
    /// that ARRIVED. Fence 23 needs the stronger fact: the mis-addressed answer
    /// must already be on its way back, because only then has the client's
    /// receive loop had something to pop, mismatch on, and start discarding. A
    /// probe that contended for the transport write lock before anybody had
    /// taken it would pass against any tree at all.
    fn answers_written(&self) -> usize {
        self.shared.calls_answered.load(Ordering::SeqCst)
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

    let Some((content_length, cursor)) = read_headers(&mut reader).await else {
        return;
    };

    let mut body = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body).await.is_err() {
        return;
    }

    if request_line.starts_with("GET ") {
        shared.get_cursors.lock().push(cursor);
        // One timestamp slot per accepted GET, pushed in the SAME place the
        // cursor slot is so the two vectors index alike.
        let sequence = {
            let mut instants = shared.get_instants.lock();
            instants.push(std::time::Instant::now());
            instants.len()
        };
        let one_frame = *shared.one_frame_then_close.lock();
        if let Some(retry_ms) = one_frame {
            serve_get_that_delivers_one_frame_then_ends(&mut write_half, retry_ms, sequence).await;
            return;
        }
        if shared.close_get_on_accept.load(Ordering::SeqCst) {
            serve_get_that_ends_at_once(&mut write_half).await;
            return;
        }
        // The rendezvous gate (plan 25), placed BEFORE the head is written and
        // only when armed. `open_gets` is bumped inside `serve_sse_body`, i.e.
        // strictly after this, so a connection parked here is not yet counted as
        // an open stream — which is what keeps the END-state assertion honest.
        await_get_head_rendezvous(&shared).await;
        serve_get(reader, write_half, &shared).await;
        return;
    }

    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    shared.post_bodies.lock().push(value.clone());

    // The bare-401 mode (plan 25). Consumed BEFORE any other POST dispatch and
    // only while the arming count is non-zero, so the retry that follows falls
    // through to the ordinary paths and every unarmed fence is untouched.
    // `checked_sub` on zero returns `None`, which makes `fetch_update` fail —
    // the decrement and the decision are therefore one atomic step and two
    // concurrent POSTs cannot both consume the same unit.
    if shared
        .unauthorized_posts_remaining
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
            remaining.checked_sub(1)
        })
        .is_ok()
    {
        serve_post_unauthorized(&mut write_half, &shared).await;
        return;
    }

    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let streams = shared.sse_post_methods.lock().contains(&method);
    if streams {
        serve_post_sse(reader, write_half, &shared).await;
        return;
    }

    // The SSE-ANSWERED call path (plan 19). Checked before `serve_post` and only
    // when the flag is on, so `call_answer_body`'s per-answer sequence is
    // consumed exactly once per answered call on either framing — a speculative
    // call here would skew `call_marker` numbering for every existing fence.
    if shared.answer_calls_with_sse.load(Ordering::SeqCst) {
        if let Some(body) = call_answer_body(&value, &method, &shared) {
            serve_post_sse_answer(&mut write_half, &body, &shared).await;
            return;
        }
    }

    serve_post(&mut write_half, &value, &shared).await;
}

/// Read the request's header block, returning `(content-length, Last-Event-ID)`.
///
/// `None` means the peer closed mid-headers.
///
/// Exactly TWO header names are looked at, and only their values are returned;
/// nothing else about the block is retained anywhere. That is what keeps the
/// module's never-record-a-header rule true while still letting fence 11 assert
/// on the resumption cursor the client actually put on the wire.
async fn read_headers(
    reader: &mut BufReader<ReadHalf<TcpStream>>,
) -> Option<(usize, Option<String>)> {
    let mut content_length = 0usize;
    let mut cursor: Option<String> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            return Some((content_length, cursor));
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().unwrap_or(0);
        } else if name.trim().eq_ignore_ascii_case("last-event-id") {
            cursor = Some(value.trim().to_string());
        }
    }
}

/// Answer a POST: the initialize result, an answered CALL, or a bare
/// `202 Accepted`.
///
/// Every answer carries `connection: close` so hyper cannot reuse the socket for
/// a later request — one connection per request keeps `request_lines()` an
/// exact, ordered record of what the client did.
async fn serve_post(write_half: &mut WriteHalf<TcpStream>, value: &Value, shared: &Shared) {
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");

    let mut is_call_answer = false;
    let response = if method == "initialize" {
        let result = json!({
            "jsonrpc": "2.0",
            "id": value.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "protocolVersion": LATEST_PROTOCOL_VERSION,
                "capabilities": advertised_capabilities(shared),
                "serverInfo": { "name": "recording-server", "version": "0.0.0" },
            },
        })
        .to_string();
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nMcp-Session-Id: s1\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{result}",
            result.len()
        )
    } else if let Some(answer) = call_answer(value, method, shared) {
        is_call_answer = true;
        answer
    } else {
        // Every notification — initialized or not — is acknowledged with the
        // bare 202 the spec prescribes. Which of them reopens the session stream
        // is the CLIENT's decision, and fence 2 is what measures it.
        "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
    };

    // The stalled-head gate (plan 23), placed AFTER the answer is BUILT and
    // strictly BEFORE its head is written, and only for an ANSWERABLE call.
    // Both facts matter: building first keeps `call_answer`'s per-answer
    // numbering identical for every existing fence, and gating only an
    // answerable call leaves the `202`-acknowledged notifications — which is
    // what a probe sends — served the way they always were.
    if is_call_answer {
        await_stalled_post_head(shared).await;
    }

    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.flush().await;
    if is_call_answer {
        shared.answers_flushed.fetch_add(1, Ordering::SeqCst);
    }
    let _ = write_half.shutdown().await;
}

/// How long a parked POST holds its response head before giving up (plan 23).
///
/// Deliberately far ABOVE [`LOCK_PROBE_BOUND`] — two orders of magnitude of the
/// probe's own bound — for the reason the stagger gate records: "the probe never
/// completed" must surface as a FENCE failure rather than be papered over by the
/// harness answering first. Bounded rather than infinite only so a regression
/// cannot hang CI.
const STALLED_POST_PARK: Duration = Duration::from_secs(30);

/// Withhold this answer's response HEAD until a fence releases it, or until
/// [`STALLED_POST_PARK`] elapses (plan 23).
///
/// A no-op unless
/// [`RecordingServer::stall_the_next_answerable_post_before_its_head`] armed it.
/// The arming is CONSUMED with a `swap`, so the decision and the consumption are
/// one atomic step and two concurrent POSTs cannot both be stalled by one
/// arming — the same rule the bare-401 gate's `fetch_update` states.
///
/// Subscribed BEFORE the arrival is counted, so a release raised the instant a
/// fence sees `stalled_posts() >= 1` is observed rather than missed.
async fn await_stalled_post_head(shared: &Shared) {
    if !shared
        .stall_next_answerable_post
        .swap(false, Ordering::SeqCst)
    {
        return;
    }
    let mut release = shared.release_stalled_post.subscribe();
    shared.stalled_post_arrivals.fetch_add(1, Ordering::SeqCst);
    let _ = timeout(STALLED_POST_PARK, release.changed()).await;
}

/// Answer a POST with a bare `401 Unauthorized` (plan 25).
///
/// No `WWW-Authenticate`, no body: the transport's refresh branch keys on the
/// STATUS alone, and adding a challenge header would make this fence depend on
/// header parsing it does not measure. `connection: close` for the reason every
/// other answer on this harness carries it — one connection per request keeps
/// `request_lines()` an exact, ordered record.
async fn serve_post_unauthorized(write_half: &mut WriteHalf<TcpStream>, shared: &Shared) {
    let response = "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.flush().await;
    // Counted AFTER the flush, so `unauthorized_answers()` means "the client can
    // already see this" rather than "the harness intends to send it".
    shared.unauthorized_answers.fetch_add(1, Ordering::SeqCst);
    let _ = write_half.shutdown().await;
}

/// How many GETs release the rendezvous gate (plan 25).
///
/// Two, because the hazard is two overlapping restarts and nothing larger adds
/// anything: a third would only widen the window the second already opens.
const GET_RENDEZVOUS_PARTY: usize = 2;

/// Withhold this GET's response head until a second GET has arrived, or until
/// the armed grace elapses — whichever happens FIRST (plan 25).
///
/// A no-op unless [`RecordingServer::hold_each_get_head_until_two_arrive`] armed
/// it. `get_instants` is the arrival record — pushed once per accepted GET,
/// before this is reached — so its length IS the arrival count and no second
/// counter can drift from it.
///
/// The OR is the whole design. On a tree where two restarts overlap, both GETs
/// arrive and the gate opens at once. On a tree where the restart is
/// serialised, the second GET cannot exist until the first restart has
/// finished, so only the grace can open the gate — a pure barrier would hang
/// the fixed tree forever and report the fix as a timeout.
async fn await_get_head_rendezvous(shared: &Shared) {
    let Some(grace) = *shared.get_head_rendezvous.lock() else {
        return;
    };
    // `wait_for_within` IS this loop, and it already backs fence 28's
    // identically OR-shaped wait. Hand-rolling it here re-implemented the
    // deadline arithmetic on `tokio::time::Instant`, putting a second timing
    // primitive in a file whose fences are sensitive to exactly that.
    let _ = wait_for_within(grace, || {
        shared.get_instants.lock().len() >= GET_RENDEZVOUS_PARTY
    })
    .await;
}

/// The `capabilities` member of this harness's `initialize` result.
///
/// `{}` unless a fence asked for [`RecordingServer::advertise_tools`], so fences
/// 1-15 see the byte-identical result they always did.
fn advertised_capabilities(shared: &Shared) -> Value {
    if shared.advertise_tools.load(Ordering::SeqCst) {
        json!({ "tools": {} })
    } else {
        json!({})
    }
}

/// The JSON-RPC success answering ONE id-bearing, non-`initialize` POST, in the
/// `application/json` HTTP envelope [`serve_post`] writes, or `None` when this
/// harness is not in answering mode (plan 15, CR-02).
///
/// Split from [`call_answer_body`] by plan 19 so the SAME per-answer numbering
/// can be delivered over a `text/event-stream` POST response instead — see
/// [`serve_post_sse_answer`]. The head written here is byte-identical to the one
/// this function wrote before the split, so every existing fence observes
/// exactly the bytes it always did.
fn call_answer(value: &Value, method: &str, shared: &Shared) -> Option<String> {
    let body = call_answer_body(value, method, shared)?;
    Some(format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    ))
}

/// The JSON-RPC success BODY answering one id-bearing, non-`initialize` POST,
/// with no HTTP envelope around it.
///
/// Owns the whole of the answering decision — the mode guard, the `initialize`
/// exclusion, the id-bearing filter, the per-answer sequence and its
/// [`call_marker`], and the single-shot mis-addressing consumption — so the two
/// framings ([`call_answer`] and [`serve_post_sse_answer`]) cannot drift over
/// WHAT is answered while differing over how it is delivered.
///
/// Two properties the fences depend on:
///
/// 1. The `result` is a `CallToolResult` shape whose single text item carries
///    [`call_marker`] of a per-answer sequence number, so call *n* and call *n+1*
///    differ BY VALUE. A constant result cannot detect a desync.
/// 2. The `id` echoes the request's own, EXCEPT when
///    [`RecordingServer::answer_the_next_call_with_a_mismatched_id`] armed a
///    single mis-addressed answer — consumed here on use.
fn call_answer_body(value: &Value, method: &str, shared: &Shared) -> Option<String> {
    if !shared.answer_calls.load(Ordering::SeqCst) {
        return None;
    }
    // Restated here rather than relied upon from the caller's `else if`: the
    // handshake result is built by its own branch, and an answering mode that
    // could ever shadow it would break every fence in this file at once.
    if method == "initialize" {
        return None;
    }
    // A notification carries no `id`, and answering one would be a protocol
    // violation the fences would then have to reason about.
    let asked_with = value.get("id").cloned().filter(|id| !id.is_null())?;
    let sequence = shared.calls_answered.fetch_add(1, Ordering::SeqCst) + 1;
    let addressed_to = if shared
        .next_call_id_is_mismatched
        .swap(false, Ordering::SeqCst)
    {
        Value::String(MISMATCHED_CALL_ID.to_string())
    } else {
        asked_with
    };
    Some(
        json!({
            "jsonrpc": "2.0",
            "id": addressed_to,
            "result": { "content": [ { "type": "text", "text": call_marker(sequence) } ] },
        })
        .to_string(),
    )
}

/// The marker answer *n* carries, so answer *n* is distinguishable from answer
/// *n+1* by value.
fn call_marker(sequence: usize) -> String {
    format!("call-answer-{sequence}")
}

/// One well-formed JSON-RPC 2.0 success addressed to an id nobody awaits
/// (plan 22, CR-02).
///
/// A SUCCESS rather than an error deliberately: the client's correlation
/// decision is made on the frame's `id` alone, before anything looks at
/// `result`/`error`, so an error frame would add a second variable to a fence
/// whose subject is classification. The `id` is
/// [`UNOWNED_SPRAY_ID_PREFIX`] plus `serial`, unique across a whole run.
fn sprayed_unowned_answer(serial: usize) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": format!("{UNOWNED_SPRAY_ID_PREFIX}{serial}"),
        "result": { "content": [ { "type": "text", "text": "addressed to nobody" } ] },
    })
    .to_string()
}

/// Answer a GET with a `text/event-stream` whose body is already over (plan 04).
///
/// A well-formed, successful response — the client's open SUCCEEDS — that then
/// delivers end-of-body immediately. That is the distinction D-03 turns on: a
/// stream that DROPPED is retryable, whereas a failed open or a corrupt frame is
/// not, and only a response that opens cleanly exercises the retryable path.
///
/// `content-length: 0` rather than a chunked body with a terminating chunk, and
/// `connection: close` rather than a poolable socket: both make "this stream is
/// over" unambiguous to hyper, so the reconnect that follows opens a NEW
/// connection instead of racing a pooled one this task is about to drop.
async fn serve_get_that_ends_at_once(write_half: &mut WriteHalf<TcpStream>) {
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
    let _ = write_half.write_all(head.as_bytes()).await;
    let _ = write_half.flush().await;
    let _ = write_half.shutdown().await;
}

/// Answer a GET with ONE complete SSE event and then end the body (plan 14).
///
/// The delivered-arm twin of [`serve_get_that_ends_at_once`], and the shape
/// CR-01 turns on: a peer that hands the client one frame per GET — so the
/// reader's `delivered` flag is `true` on every body — while telling it, through
/// the SSE `retry:` field, to come straight back with no wait at all.
///
/// Chunked rather than `content-length`-framed because a frame has to be written
/// after the head; the terminating zero-length chunk plus `connection: close`
/// keep "this stream is over" as unambiguous to hyper as the zero-delivery mode
/// makes it, so the reconnect opens a NEW connection rather than racing a pooled
/// one.
///
/// `sequence` gives the event a per-GET `id:`, so the ids advance the way a real
/// stream's do and the reconnect carries a moving cursor rather than a constant
/// one.
async fn serve_get_that_delivers_one_frame_then_ends(
    write_half: &mut WriteHalf<TcpStream>,
    retry_ms: u64,
    sequence: usize,
) {
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\
                connection: close\r\n\r\n";
    if write_half.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": { "progressToken": "zero-retry", "progress": sequence },
    })
    .to_string();
    let frame = format!("retry: {retry_ms}\nid: zr{sequence}\nevent: message\ndata: {payload}\n\n");
    let _ = write_chunk(write_half, &frame).await;
    let _ = write_half.write_all(b"0\r\n\r\n").await;
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

/// Answer a POST with a `text/event-stream` carrying ONE complete `message`
/// frame — the call's own JSON-RPC result — and then end the body (plan 19).
///
/// Deliberately NOT [`serve_post_sse`], which fences 8-10 and 19 depend on:
/// that mode holds the body open forever and pumps frames a fence pushes, and
/// this one delivers the answer and finishes. Both are `text/event-stream` POST
/// responses, which is the property BLOCKER 1 turns on — `post_body` spawns a
/// DETACHED reader and returns `Ok(())` before either has delivered anything, so
/// the receive queue is transiently empty while a real answer is on the wire.
///
/// Chunked rather than `content-length`-framed for the reason
/// [`serve_get_that_delivers_one_frame_then_ends`] records: a frame has to be
/// written after the head. The terminating zero-length chunk plus
/// `connection: close` make "this answer is complete" unambiguous to hyper.
///
/// `open_posts` is incremented for exactly the span the body is live, mirroring
/// [`serve_sse_body`], so `open_post_connections()` keeps meaning the same thing
/// on both POST-SSE modes.
async fn serve_post_sse_answer(write_half: &mut WriteHalf<TcpStream>, body: &str, shared: &Shared) {
    shared.open_posts.fetch_add(1, Ordering::SeqCst);

    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\
                connection: close\r\n\r\n";
    if write_half.write_all(head.as_bytes()).await.is_ok() && write_half.flush().await.is_ok() {
        // Newest-first staggering (plan 22), placed AFTER the head is flushed
        // and BEFORE the data frame — which is the only seam that produces a
        // genuinely outstanding call. `StreamableHttpTransport::send()` awaits
        // the HTTP response, so holding a JSON-answered POST blocks the client
        // INSIDE `send()`, still holding the transport lock and never reaching
        // the receive path at all. Holding the FRAME instead lets `send()`
        // return and the answer arrive asynchronously on the queue, which is
        // the shape per-id routing exists for.
        if shared.stagger_answers.load(Ordering::SeqCst)
            && shared.answerable_gated.fetch_add(1, Ordering::SeqCst) == 0
        {
            // 20 s of patience — deliberately ABOVE both fence bounds that use
            // this gate, so "the other caller never got its answer" surfaces as
            // a fence failure rather than being papered over by the gate giving
            // up first. Bounded rather than infinite only so a regression
            // cannot hang CI.
            for _ in 0..2000 {
                if shared.answers_flushed.load(Ordering::SeqCst) >= 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        // The unowned spray (plan 22, CR-02), placed AFTER the stagger gate and
        // BEFORE this call's own answer. Both facts matter: the sprayed frames
        // must reach the client while the call is still waiting (so they are
        // booked against ITS budget), and the call's real answer must still be
        // behind them on the wire (so a call that dies at the count bound leaves
        // its own answer as un-owned debris — the chain under test).
        let spray = shared
            .unowned_spray_schedule
            .lock()
            .pop_front()
            .unwrap_or(0);
        for _ in 0..spray {
            let serial = shared.unowned_sprayed.fetch_add(1, Ordering::SeqCst) + 1;
            let frame = format!(
                "event: message\ndata: {}\n\n",
                sprayed_unowned_answer(serial)
            );
            // The SAME `write_chunk` the answer uses, so the framing a sprayed
            // frame arrives in is byte-identical to the framing a real answer
            // arrives in and nothing can pass by telling the two apart.
            let _ = write_chunk(write_half, &frame).await;
        }

        let frame = format!("event: message\ndata: {body}\n\n");
        let _ = write_chunk(write_half, &frame).await;
        shared.answers_flushed.fetch_add(1, Ordering::SeqCst);
        let _ = write_half.write_all(b"0\r\n\r\n").await;
        let _ = write_half.flush().await;
        let _ = write_half.shutdown().await;
    }

    shared.open_posts.fetch_sub(1, Ordering::SeqCst);
}

/// Write the `text/event-stream` head, then pump pushed frames until the peer
/// closes — counting the connection as open for exactly that span.
///
/// No terminating zero-length chunk is written while the stream is LIVE. The
/// connection ends only when the client closes it, or when a fence asks for it
/// through [`RecordingServer::end_live_sse_streams`] — which is exactly what
/// makes this the shape a whole-body `collect()` cannot read.
///
/// On the way out the terminating chunk IS written, so a fence-requested close
/// reaches the client as an ordinary end-of-body rather than as a truncation.
/// When the peer already went away the write simply fails and is ignored.
async fn serve_sse_body(
    mut reader: BufReader<ReadHalf<TcpStream>>,
    mut write_half: WriteHalf<TcpStream>,
    shared: &Shared,
    open: &AtomicUsize,
    source: &Mutex<Option<mpsc::Receiver<String>>>,
) {
    open.fetch_add(1, Ordering::SeqCst);

    // Subscribed BEFORE the head is written, so a close raised at any point
    // after this connection was accepted is seen rather than missed.
    let mut close_live = shared.close_live.subscribe();

    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\
                connection: close\r\n\r\n";
    if write_half.write_all(head.as_bytes()).await.is_ok() && write_half.flush().await.is_ok() {
        let mut frames_rx = source.lock().take();
        pump_frames(
            &mut reader,
            &mut write_half,
            shared,
            frames_rx.as_mut(),
            &mut close_live,
        )
        .await;
        // Hand the source back so a LATER stream on this same harness can use it.
        if let Some(rx) = frames_rx {
            *source.lock() = Some(rx);
        }
        let _ = write_half.write_all(b"0\r\n\r\n").await;
        let _ = write_half.flush().await;
        let _ = write_half.shutdown().await;
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
    close_live: &mut watch::Receiver<u64>,
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
                    _ = close_live.changed() => return,
                }
            },
            None => {
                tokio::select! {
                    read = reader.read(&mut scratch) => {
                        if !matches!(read, Ok(bytes) if bytes > 0) {
                            return;
                        }
                    },
                    _ = close_live.changed() => return,
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
async fn wait_for(predicate: impl FnMut() -> bool) -> bool {
    wait_for_within(BOUND, predicate).await
}

/// Poll `predicate` until it holds, or `bound` elapses.
///
/// The reconnect fences wait out a schedule measured in seconds of deliberate
/// backoff, which is not a wire operation and so is not [`BOUND`]'s business.
/// Still a POLL inside a bound, never a fixed sleep used as a synchronisation
/// device: the fence proceeds the moment the condition holds.
async fn wait_for_within(bound: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    timeout(bound, async {
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

/// How many `tools/call` POST bodies this harness has observed.
///
/// A vacuity guard for fences 16 and 17: a correlation fence that never reached
/// the wire proves nothing, and would pass against any tree at all.
fn calls_observed(server: &RecordingServer) -> usize {
    server
        .post_bodies()
        .iter()
        .filter(|body| body.get("method").and_then(Value::as_str) == Some("tools/call"))
        .count()
}

/// The text of a call result's first content item.
fn text_of(result: &pmcp::types::CallToolResult) -> Option<String> {
    result.content.first().and_then(|content| match content {
        pmcp::types::Content::Text { text } => Some(text.clone()),
        _ => None,
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
    assert!(
        !text.contains(RECONNECT_PHRASE),
        "the D-02 corruption error must be DISCRIMINABLE from D-03's reconnect-budget exhaustion: \
         a consumer has to be able to tell a corrupted stream (do not retry) from a lifecycle end \
         (the peer went away); got {text:?}"
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
    assert!(
        !text.contains(RECONNECT_PHRASE),
        "the D-05 corruption error must be DISCRIMINABLE from D-03's reconnect-budget exhaustion, \
         for the same reason the D-02 one must; got {text:?}"
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
    // contract is the receive queue instead — dropping the last clone drops the
    // queue's `Receiver`, which BOTH fails the reader's next
    // `sender.send(..).await` AND resolves the `sender.closed()` arm the reader
    // races against its parked body read (WR-01).
    drop(transport);

    // NOTHING is pushed. This fence used to write one frame here "purely so the
    // reader HAS a next send to fail", and that crutch WAS the measurement gap
    // WR-01 names: a reader must stop on a stream that is open and IDLE, not only
    // on one that hands it a send to fail. A server holding this body open with
    // SSE keep-alive comments — the standard idle keepalive — produces no events
    // at all, so the send-failure signal never fires and the reader stays parked
    // in `body.frame()` holding a live socket. The shutdown race added for WR-01
    // is what makes the assertion below true with no frame at all.
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

// ===========================================================================
// Fences 11-14 — D-03: bounded auto-reconnect with the resumption cursor, and
// the two ways a client can go away while the loop is asleep in its backoff.
//
// Why this exists at all: real deployments idle-drop long-lived streams as a
// matter of course (ALB at ~60 s, Cloudflare, most API gateways). Without a
// reconnect the session stream this phase just opened silently regresses to
// "no live stream" the first time a proxy blinks — the exact symptom the phase
// exists to remove.
// ===========================================================================

/// The phrase D-03's exhaustion error carries and the D-02 / D-05 corruption
/// errors must not.
///
/// The three are checked against each other in BOTH directions — fences 5 and 7
/// assert its absence, fence 12 asserts its presence — because "the consumer can
/// tell corruption from a lifecycle end" is a property of the SET of messages,
/// not of any one of them.
const RECONNECT_PHRASE: &str = "reconnect budget";

// ===========================================================================
// Fence 11 — a dropped stream is re-opened, carrying the last event id.
// ===========================================================================

#[tokio::test]
async fn a_dropped_stream_is_reopened_with_the_last_event_id() {
    let server = RecordingServer::start().await;
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the session stream must be open before it can be dropped. Observed: {:?}",
        server.request_lines()
    );

    // A frame carrying an id, RECEIVED — so the cursor is recorded by the same
    // production path a live stream records it on, rather than planted.
    server
        .push_frame(progress_frame(RESUMED_FROM, "fence-11", 1, None))
        .await;
    let message = timeout(BOUND, transport.receive())
        .await
        .expect("the id-carrying frame reaches receive() within BOUND")
        .expect("the frame parses into a transport message");
    assert_eq!(progress_of(&message), Some(1.0), "got {message:?}");

    // The proxy blinks.
    server.end_live_sse_streams();

    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= 2).await,
        "a session stream dropped mid-flight must be RE-OPENED. Without this the phase's fix \
         regresses to no-live-stream the first time an idle-timeout proxy closes the socket, \
         which every real deployment does. Observed request lines: {:?}",
        server.request_lines()
    );

    let cursors = server.get_cursors();
    assert!(
        cursors.len() >= 2,
        "the harness must have recorded a cursor slot per GET; got {cursors:?}"
    );
    assert_eq!(
        cursors[0], None,
        "the FIRST open has nothing to resume from and must carry no cursor at all; got {cursors:?}"
    );
    assert_eq!(
        cursors[1].as_deref(),
        Some(RESUMED_FROM),
        "the re-opened GET must carry the last event id ON THE WIRE, so the server can replay from \
         it — a reconnect that starts from nothing loses every frame emitted during the gap. \
         Observed cursors: {cursors:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 12 — the retry budget is bounded, and its exhaustion is LOUD.
// ===========================================================================

#[tokio::test]
async fn reconnect_gives_up_after_the_retry_budget_with_a_named_error() {
    let server = RecordingServer::start().await;
    // Every GET opens successfully and ends immediately: a peer that will never
    // hold a stream, which is what a bounded budget exists to stop chasing.
    server.close_get_streams_on_accept();
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;

    let expected_gets = 1 + SHIPPED_RECONNECT_BUDGET;
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= expected_gets).await,
        "the initial open plus {SHIPPED_RECONNECT_BUDGET} budgeted retries must all be issued. \
         Observed request lines: {:?}",
        server.request_lines()
    );

    let error = timeout(RECONNECT_BOUND, transport.receive())
        .await
        .expect("the exhaustion error reaches receive() within RECONNECT_BOUND")
        .expect_err(
            "an exhausted reconnect budget must SURFACE at Transport::receive(). A reader that \
             simply stops is indistinguishable from a healthy idle stream, and an application \
             that cannot tell those apart waits forever for a peer that is gone",
        );

    let text = error.to_string();
    assert!(
        text.contains(RECONNECT_PHRASE),
        "the exhaustion error must NAME the budget it spent, not merely report a closed channel — \
         a generic channel-closed error is the false-green shape this phase's validation strategy \
         exists to prevent; got {text:?}"
    );
    assert!(
        text.contains(&SHIPPED_RECONNECT_BUDGET.to_string()),
        "and it must carry the attempt count, so an operator can tell a spent budget from a \
         never-started one; got {text:?}"
    );
    assert!(
        !text.contains("parser bound"),
        "it must be textually distinct from the D-02 overflow error; got {text:?}"
    );
    assert!(
        !text.contains("did not parse as a JSON-RPC message"),
        "and from the D-05 parse error; got {text:?}"
    );

    // And it STOPS. A budget that is announced and then exceeded is not a budget.
    tokio::time::sleep(RECONNECT_QUIET).await;
    assert_eq!(
        server.get_lines(),
        expected_gets,
        "exactly the initial open plus {SHIPPED_RECONNECT_BUDGET} retries and no more — an \
         unbounded loop against a peer that keeps closing is a self-inflicted denial of service \
         (T-118.2-04-01). Observed request lines: {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 13 — close() during the backoff sleep issues no further GET.
// ===========================================================================

#[tokio::test]
async fn closing_during_reconnect_backoff_issues_no_further_get() {
    let server = RecordingServer::start().await;
    server.close_get_streams_on_accept();
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;

    // Wait for the FIRST reconnect before closing, so the fence closes a loop
    // that is demonstrably running rather than one that never started.
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= 2).await,
        "the reconnect loop must be running before this fence can measure its cancellation. \
         Observed request lines: {:?}",
        server.request_lines()
    );
    let observed = server.get_lines();
    assert!(
        observed < 1 + SHIPPED_RECONNECT_BUDGET,
        "the budget was already spent before the close, so this fence would prove nothing: {} \
         GET(s) observed",
        observed
    );

    timeout(BOUND, transport.close())
        .await
        .expect("close() returns within BOUND")
        .expect("close() succeeds against the harness");

    tokio::time::sleep(RECONNECT_QUIET).await;
    assert_eq!(
        server.get_lines(),
        observed,
        "close() during a backoff sleep must prevent the GET that sleep was scheduling. A loop \
         that wakes up and reconnects anyway is talking to a peer nobody is listening to, and \
         keeps a socket open against a transport its owner has already shut. Observed request \
         lines: {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 14 — a DROPPED transport during the backoff sleep does the same.
// ===========================================================================

#[tokio::test]
async fn dropping_the_transport_during_backoff_issues_no_further_get() {
    let server = RecordingServer::start().await;
    server.close_get_streams_on_accept();
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;

    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= 2).await,
        "the reconnect loop must be running before this fence can measure its cancellation. \
         Observed request lines: {:?}",
        server.request_lines()
    );
    let observed = server.get_lines();
    assert!(
        observed < 1 + SHIPPED_RECONNECT_BUDGET,
        "the budget was already spent before the drop, so this fence would prove nothing: {} \
         GET(s) observed",
        observed
    );

    // No `close()` here, and deliberately so: there is no transport left to call
    // it on, so the abort handle is not available as a mechanism. What must stop
    // the loop instead is its own sender going closed the moment the last clone
    // drops the receive queue's `Receiver` — which is exactly why the reader's
    // owned context must consult its sender rather than rely on the abort handle.
    drop(transport);

    tokio::time::sleep(RECONNECT_QUIET).await;
    assert_eq!(
        server.get_lines(),
        observed,
        "a reconnect loop must not outlive the transport that owns it — it would hold a socket \
         open and reconnect to a peer with nothing left to deliver to. Observed request lines: \
         {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 15 — a peer answering every GET with `retry: 0` PLUS one delivered
// frame is still bounded (plan 14, CR-01).
//
// The arm fence 12 cannot reach. Fence 12's peer uses
// `close_get_streams_on_accept()`, a stream that delivers NOTHING, so the
// reader's `delivered` flag is always `false` and the budget-refund branch is
// never taken. A peer that hands over exactly ONE frame per body takes that
// branch on EVERY iteration — and, with `retry: 0` honoured verbatim, converts
// the bounded retry budget into an unbounded, zero-delay request flood that also
// re-mints an access token per iteration (T-118.2-14-01, T-118.2-14-02).
// ===========================================================================

#[tokio::test]
async fn reconnect_with_one_delivered_frame_and_zero_retry_stays_bounded() {
    let server = RecordingServer::start().await;
    // Both halves of CR-01 at once: a delivered frame per body, and a
    // peer-supplied `retry:` of zero.
    server.deliver_one_frame_then_close_each_get(0);
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;

    // EQUIVALENCE (the CONF-09 `empty` probe). This is the SAME expression
    // fence 12 pins for a body that delivers ZERO events. Pinning the one
    // expression in both fences is what makes "an empty body and a
    // single-element body terminate at the same bounded GET count" an assertion
    // rather than a coincidence: the delivered arm must not buy extra attempts.
    let expected_gets = 1 + SHIPPED_RECONNECT_BUDGET;

    // --- COUNT: the CR-01 subject. ---
    assert!(
        wait_for_within(ZERO_RETRY_BOUND, || server.get_lines() >= expected_gets).await,
        "the initial open plus {SHIPPED_RECONNECT_BUDGET} budgeted retries must all be issued. \
         Observed request lines: {:?}",
        server.request_lines()
    );
    tokio::time::sleep(ZERO_RETRY_QUIET).await;
    assert_eq!(
        server.get_lines(),
        expected_gets,
        "a peer that answers every GET with ONE frame and `retry: 0` must still get exactly the \
         initial open plus {SHIPPED_RECONNECT_BUDGET} retries. Refunding the whole budget on any \
         single delivered event makes the loop unbounded, and honouring `retry: 0` verbatim makes \
         it tight — together they are a remote-triggerable denial of service against the CLIENT: \
         a burned core, a request flood at the peer, and a fresh access-token fetch per iteration \
         (CR-01, T-118.2-14-01/02). Observed request lines: {:?}",
        server.request_lines()
    );

    // --- FLOOR: a lower bound on the spacing, measured SERVER-side. ---
    let instants = server.get_instants();
    assert!(
        instants.len() >= expected_gets,
        "the harness must have recorded a timestamp slot per accepted GET; got {} for {} GET \
         line(s)",
        instants.len(),
        server.get_lines()
    );
    let budget = u32::try_from(SHIPPED_RECONNECT_BUDGET)
        .expect("the shipped reconnect budget fits in a u32");
    let floor = SHIPPED_MIN_RECONNECT_DELAY * budget;
    let spanned = instants[expected_gets - 1].duration_since(instants[0]);
    assert!(
        spanned >= floor,
        "the {expected_gets} GETs arrived {spanned:?} apart end to end, under the {floor:?} floor \
         that {SHIPPED_RECONNECT_BUDGET} waits of at least {SHIPPED_MIN_RECONNECT_DELAY:?} \
         guarantee. A peer-supplied `retry:` is remote input in BOTH directions: uncapped above it \
         parks the reader, unfloored below it turns the reconnect loop into a request flood. A \
         LOWER bound only — CI load can only make this larger, never smaller."
    );

    // --- TERMINAL: the named exhaustion error still surfaces, after the frames. ---
    let mut delivered = 0usize;
    let error = loop {
        let next = timeout(ZERO_RETRY_BOUND, transport.receive())
            .await
            .expect("each delivered frame, and then the exhaustion error, arrives within bound");
        match next {
            Ok(message) => {
                delivered += 1;
                assert!(
                    delivered <= expected_gets,
                    "the peer wrote one frame per GET and issued {expected_gets} GET(s), so the \
                     drain must not exceed that count; got {delivered}"
                );
                assert!(
                    progress_of(&message).is_some(),
                    "every frame this peer writes is a progress notification; got {message:?}"
                );
            },
            Err(error) => break error,
        }
    };
    assert_eq!(
        delivered, expected_gets,
        "one frame per GET must actually have been DELIVERED — otherwise this fence never took \
         the `delivered` arm it exists to bound, and would pass for the same reason fence 12 does"
    );

    let text = error.to_string();
    assert!(
        text.contains(RECONNECT_PHRASE),
        "the exhaustion error must still NAME the budget it spent even when every body delivered. \
         A reader that simply goes quiet is indistinguishable from a healthy idle stream; got \
         {text:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fences 16-17 — CR-02: a terminal reason raised while the application was IDLE
// must not fail the next, unrelated call, and a response bearing someone else's
// id must never be returned as this call's answer (plan 15).
//
// The two halves are one defect. Every terminal reason phase 118.2 introduced is
// delivered by pushing `Err(..)` onto the SAME queue the responses ride, and the
// only consumer is `Client::dispatch_request` — so a reason raised while nobody
// is asking sits in the FIFO and is handed to whoever asks next. That same
// receive loop returned the first `Response` frame it popped with NO comparison
// of `response.id` against the id it was awaiting, so one out-of-band entry
// desynchronises the queue permanently and call *n+1* silently receives call
// *n*'s result (T-118.2-15-01).
//
// Fence 16 measures the poisoning and the desync it causes; fence 17 measures
// the correlation decision in isolation, with no terminal reason anywhere.
// ===========================================================================

// ===========================================================================
// Fence 16 — an idle terminal error does not fail the next unrelated call.
//
// The automated replacement for the second `human_verification` item in
// `118.2-VERIFICATION.md`, exercising exactly the interleaving that report
// described: the session stream dies while the application is idle, and the
// application then makes two ordinary tool calls.
// ===========================================================================

#[tokio::test]
async fn an_idle_terminal_error_does_not_fail_the_next_unrelated_call() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    // Every GET opens successfully and ends at once, so the session stream spends
    // its WHOLE reconnect budget and raises its named terminal reason while the
    // application is doing nothing at all.
    server.close_get_streams_on_accept();

    // The observer clone is bound only to keep the transport alive for the whole
    // fence. `receive()` is NEVER called on it: it shares the receive queue with
    // the clone the client owns, so a read here would STEAL the very response
    // these assertions are about and the fence would measure the harness.
    let (client, _observer) = handshake(&server).await;

    let expected_gets = 1 + SHIPPED_RECONNECT_BUDGET;
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= expected_gets).await,
        "the session stream must spend its budget before the fence calls anything — otherwise \
         there is no idle terminal reason to be poisoned by. Observed request lines: {:?}",
        server.request_lines()
    );
    // An OBSERVATION window, not a synchronisation device: it proves the
    // reconnect loop has STOPPED, which is what establishes both halves of the
    // precondition — the terminal reason has been raised, and it was raised with
    // NO request in flight. Every wait after this point goes through a bound.
    tokio::time::sleep(RECONNECT_QUIET).await;
    assert_eq!(
        server.get_lines(),
        expected_gets,
        "the budget must be spent and the loop finished before the first call. Observed request \
         lines: {:?}",
        server.request_lines()
    );

    // --- Call 1: whatever it reports, it must describe ITSELF. ---
    let first = timeout(
        BOUND,
        client.call_tool("fence-16-one".to_string(), json!({})),
    )
    .await
    .expect("the first call completes within BOUND");
    if let Err(error) = &first {
        let text = error.to_string();
        assert!(
            !text.contains(RECONNECT_PHRASE),
            "a tools/call that succeeded on the wire came back reporting a SESSION-STREAM failure \
             raised while the application was idle. An out-of-band terminal reason on the shared \
             response FIFO is handed to whichever request asks next, so an unrelated call fails \
             with a diagnosis of something that happened minutes earlier and the real answer is \
             left in the queue to desynchronise every later call (CR-02). Got: {text:?}"
        );
    }

    // --- Call 2: its OWN answer, not call 1's. ---
    //
    // This is the desync assertion the verification report demanded. An
    // id-correlation unit test does not satisfy it: the subject is that TWO
    // consecutive calls through a real `pmcp::Client` each receive their own
    // result.
    let second = timeout(
        BOUND,
        client.call_tool("fence-16-two".to_string(), json!({})),
    )
    .await
    .expect("the second call completes within BOUND")
    .expect("the harness answers every id-bearing call with a JSON-RPC success");
    let observed = text_of(&second);
    assert_eq!(
        observed.as_deref(),
        Some(call_marker(2).as_str()),
        "call 2 must receive call 2's OWN result. Once one out-of-band entry desynchronises the \
         response FIFO, every later call is handed the PREVIOUS call's answer — a cross-request \
         data leak between two callers of one client (T-118.2-15-01), and silent, because a \
         well-formed result for the wrong request is indistinguishable from a correct one at the \
         call site. Expected {:?}, got {observed:?}",
        call_marker(2)
    );

    assert_eq!(
        calls_observed(&server), 2,
        "both calls must actually have reached the wire — a fence that asserts on results it never \
         asked for would pass against any tree. Observed POST bodies: {:?}",
        server.post_bodies()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 17 — a response bearing an id that is not this call's is not returned
// as this call's answer.
//
// No terminal reason exists anywhere here: the GET side is the ordinary
// held-open session stream. The ONLY thing under test is the correlation
// decision, so a failure cannot be blamed on the latch.
// ===========================================================================

#[tokio::test]
async fn a_response_whose_id_does_not_match_is_not_returned_as_this_calls_answer() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_the_next_call_with_a_mismatched_id();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so this fence differs from fence 16 in exactly \
         one thing: there is no terminal reason anywhere. Observed request lines: {:?}",
        server.request_lines()
    );

    // The bounded wait CANCELS `dispatch_request` mid-loop, so the client's
    // `active_requests` entry for this id is not reaped by the WR-04 exit
    // cleanup — that path runs on an `Err` return, and a cancelled future
    // returns nothing at all. A known consequence of cancelling a pending
    // request, not what this fence measures; the client is dropped immediately
    // below.
    let outcome = timeout(
        MISMATCH_BOUND,
        client.call_tool("fence-17".to_string(), json!({})),
    )
    .await;

    match outcome {
        // The correct post-fix shape: the mis-addressed frame was discarded and
        // the call is still waiting for the response that carries ITS id, which
        // this peer never sends.
        Err(_still_waiting) => {},
        Ok(Ok(result)) => panic!(
            "the client returned a server response addressed to {MISMATCHED_CALL_ID:?} as the \
             answer to a request it never identified. A receive loop that returns the first \
             `Response` frame it pops, without comparing `response.id` against the id it is \
             awaiting, accepts a fabricated or re-typed id from a hostile or merely buggy peer \
             (T-118.2-15-02) and mis-pairs concurrent callers of one client \
             (T-118.2-15-01). Returned result: {:?}",
            text_of(&result)
        ),
        // WR-05, closed by plan 20. This arm used to accept the WHOLE error
        // space on the reasoning that "an error is not this call's answer
        // either". True, but not sufficient: a regression that fails the call
        // early — for any reason at all, before the receive loop ever pops a
        // frame — kept this fence green while the correlation branch never
        // executed. A fence that passes when its subject never runs measures
        // nothing.
        //
        // At MISMATCH_BOUND (2 s) the intended outcome is STILL WAITING, and
        // it stays that way at this HEAD: the ceiling on frames nobody awaits
        // (UNMATCHED_RESPONSE_TIMEOUT, 10 s) is five times this bound, and its
        // sibling count bound (MAX_UNMATCHED_RESPONSES, 32) needs 32 un-owned
        // frames where this fence sends one — so the bounded failure, owned by
        // debris_from_a_dead_call_does_not_charge_the_next_calls_budget, cannot
        // reach this arm either.
        Ok(Err(not_an_answer)) => panic!(
            "call_tool returned an ERROR at {MISMATCH_BOUND:?}, where this fence's subject \
             requires it to be STILL WAITING — the mis-addressed frame discarded and the \
             correlation branch holding for the response that carries this call's own id. Any \
             error here means the receive loop was left before the correlation decision was \
             reached, so accepting it would keep this fence green on a tree where the branch it \
             exists to pin never ran (WR-05). Got: {not_an_answer}"
        ),
    }

    assert_eq!(
        calls_observed(&server),
        1,
        "the call must actually have reached the wire — a fence that concludes 'still waiting' \
         about a request that was never sent would pass against any tree. Observed POST bodies: \
         {:?}",
        server.post_bodies()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fences 18-20 — WR-01 (a reader parked on an IDLE-but-open stream never
// observes shutdown, so a task and a socket leak) and WR-02 (the resumption
// cursor is shared across streams, so a reconnect can resume from ANOTHER
// stream's event id).
//
// Why the existing 17 fences cannot reach either:
//
// * Every termination signal this phase documents needs the reader to ACT. The
//   failing `sender.send(..)` needs a frame to send; the two `is_closed()`
//   checks need the loop to reach its backoff sleep. A server holding the
//   stream open with SSE keep-alive comments — the standard idle keepalive —
//   produces no events at all, so the reader parks in `body.frame()` and
//   neither signal ever fires. Fence 10 hid this by pushing a frame on purpose;
//   that crutch is gone as of this plan.
// * `close()` aborts exactly ONE `JoinHandle` — the GET session reader's. Every
//   reader spawned per streaming POST is DETACHED (`drop(spawn_sse_reader(..))`)
//   and there is no bound on how many exist, so each one survives `close()`
//   outright.
// * Fence 11 proves the re-opened GET carries A cursor, but a GET-only scenario
//   has exactly one writer, so the shared cursor and the per-stream one agree.
//   Only a scenario with a GET and a streaming POST live at the SAME TIME can
//   tell them apart.
// ===========================================================================

/// The event id the GET session stream delivers in fence 20.
///
/// Distinguishable from [`POST_STREAM_EVENT_ID`] by VALUE, because the whole
/// subject of that fence is WHICH stream's id reached the reconnect's
/// `Last-Event-ID` header.
const GET_STREAM_EVENT_ID: &str = "get-stream-e11";

/// The event id the streaming POST response delivers in fence 20 — the id that
/// must NEVER become the session stream's resume point.
const POST_STREAM_EVENT_ID: &str = "post-stream-e97";

/// How many `receive()` pops [`drain_until_progress`] will make while looking for
/// the frame it was asked about.
///
/// ONE queue serves both of fence 20's streams, so the fence matches on the
/// PAYLOAD rather than on arrival order. The cap exists so a tree that delivers
/// nothing fails on the fence's subject rather than looping forever; it is not a
/// synchronisation device — each pop is itself bounded by [`BOUND`].
const CROSS_STREAM_POPS: usize = 6;

/// Pop `receive()` until a progress notification carrying `progress` arrives, or
/// [`CROSS_STREAM_POPS`] pops have been made.
///
/// Identifies the frame by its own payload, never by arrival order: fence 20 has
/// a GET session stream and a streaming POST response feeding one queue.
async fn drain_until_progress(transport: &mut StreamableHttpTransport, progress: f64) -> bool {
    for _ in 0..CROSS_STREAM_POPS {
        let message = timeout(BOUND, transport.receive())
            .await
            .expect("a pushed frame reaches receive() within BOUND")
            .expect("the frame parses into a transport message");
        if progress_of(&message).is_some_and(|value| (value - progress).abs() < f64::EPSILON) {
            return true;
        }
    }
    false
}

// ===========================================================================
// Fence 18 — WR-01, the dropped-transport half: a reader parked on an
// idle-but-open GET session stream stops when the last transport clone drops.
// ===========================================================================

#[tokio::test]
async fn a_reader_parked_on_an_idle_open_stream_stops_when_the_transport_is_dropped() {
    let server = RecordingServer::start().await;
    let mut transport = transport_for(&server);
    open_stream(&mut transport).await;

    assert!(
        wait_for(|| server.open_get_connections() >= 1).await,
        "the session stream must be OPEN server-side before it can be leaked. Observed request \
         lines: {:?}",
        server.request_lines()
    );

    // NOTHING is ever pushed on this stream, and that is the whole point: the
    // reader never attempts a send, so the send-failure signal stays silent, and
    // it never reaches a backoff sleep, so both `is_closed()` checks stay
    // unreached. This is the peer shape WR-01 names — a server that holds the
    // session stream open and sends only keep-alive comments, which the shared
    // parser drops without producing an event.
    assert_eq!(
        server.frames_written(),
        0,
        "this fence measures an IDLE stream; a frame written here would restore exactly the crutch \
         fence 10 just lost"
    );

    drop(transport);

    assert!(
        wait_for(|| server.open_get_connections() == 0).await,
        "a reader parked on an idle-but-open body must STOP when the last transport clone is \
         dropped — {} GET stream(s) were still open at the SERVER. `abort_handle` holds a \
         `JoinHandle`, and dropping a `JoinHandle` DETACHES rather than aborts, and there is no \
         `Drop` impl on the transport, so a dropped transport otherwise leaves a live task holding \
         a live TCP connection until the SERVER times it out. In a process that creates and drops \
         transports in a loop — a pool, a CLI, a test suite — that is an unbounded task and \
         file-descriptor leak whose duration the PEER chooses (T-118.2-17-01)",
        server.open_get_connections()
    );

    // And nothing comes BACK: a reader that stopped must not have been mistaken
    // for a dropped stream worth reconnecting.
    let gets = server.get_lines();
    tokio::time::sleep(QUIET).await;
    assert_eq!(
        server.open_get_connections(),
        0,
        "no session stream may re-open after the transport is gone"
    );
    assert_eq!(
        server.get_lines(),
        gets,
        "and no further GET may be issued — a shutdown that surfaced as a retryable DROP would send \
         the reconnect loop chasing a transport nobody owns. Observed request lines: {:?}",
        server.request_lines()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 19 — WR-01, the `close()` half: a DETACHED POST-response reader is
// stopped by `close()`, which aborts only the GET session reader's handle.
// ===========================================================================

#[tokio::test]
async fn close_stops_a_detached_post_response_reader() {
    let server = RecordingServer::start().await;
    server.answer_post_with_sse("ping");
    let mut transport = transport_for(&server);

    timeout(BOUND, transport.send(ping_request("fence-19")))
        .await
        .expect("the POST returns as soon as the stream is open — a hang here IS the defect")
        .expect("the harness answers the POST with a 200 text/event-stream");
    assert!(
        wait_for(|| server.open_post_connections() >= 1).await,
        "the POST stream must be open before close() can be asked to stop it. Observed request \
         lines: {:?}",
        server.request_lines()
    );

    // `close()`, NOT drop. The transport stays alive for the whole assertion
    // below, so the receive queue's `Receiver` is still held and the
    // send-failure/`sender.closed()` path is NOT what could stop this reader —
    // an explicit close is.
    timeout(BOUND, transport.close())
        .await
        .expect("close() returns within BOUND")
        .expect("close() succeeds against this harness");

    assert!(
        wait_for(|| server.open_post_connections() == 0).await,
        "close() must stop EVERY reader, not only the GET session reader whose `JoinHandle` it \
         aborts — {} POST stream(s) were still open at the SERVER. Each streaming POST spawns a \
         DETACHED reader (`drop(spawn_sse_reader(..))`) with no bound on how many exist, so on the \
         unfixed tree every one of them survives close() outright and keeps reading a \
         peer-controlled socket that the application has explicitly finished with \
         (T-118.2-17-02)",
        server.open_post_connections()
    );

    // It stays closed, and the transport is still alive while we check — so this
    // is close()'s doing and not a drop's.
    let written = server.frames_written();
    tokio::time::sleep(QUIET).await;
    assert_eq!(
        server.open_post_connections(),
        0,
        "no POST stream may re-open after close()"
    );
    assert_eq!(
        server.frames_written(),
        written,
        "nothing may still be consuming the stream a full QUIET after close()"
    );
    assert!(
        transport.is_connected(),
        "the transport itself is still alive here — a fence that had dropped it would be measuring \
         fence 10's signal instead of close()'s"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 20 — WR-02: a cursor minted on a streaming POST response must never
// become the session stream's `Last-Event-ID` on reconnect.
// ===========================================================================

#[tokio::test]
async fn a_post_stream_cursor_never_becomes_the_session_streams_last_event_id() {
    let server = RecordingServer::start().await;
    server.answer_post_with_sse("ping");
    let mut transport = transport_for(&server);

    // Both streams live at once: the GET session stream, and a POST answered with
    // a held-open `text/event-stream`. That co-existence is what no earlier fence
    // arranges, and it is the only shape in which a shared cursor and a
    // per-stream one can disagree.
    open_stream(&mut transport).await;
    assert!(
        wait_for(|| server.open_get_connections() >= 1).await,
        "the session stream must be open. Observed request lines: {:?}",
        server.request_lines()
    );
    timeout(BOUND, transport.send(ping_request("fence-20")))
        .await
        .expect("the POST returns as soon as its stream is open")
        .expect("the harness answers the POST with a 200 text/event-stream");
    assert!(
        wait_for(|| server.open_post_connections() >= 1).await,
        "the POST stream must be open TOO. Observed request lines: {:?}",
        server.request_lines()
    );

    // An id-carrying frame on EACH stream, delivered through the production path
    // rather than planted — and delivered ONE AT A TIME, each observed before the
    // next is pushed.
    //
    // The sequencing is load-bearing, not tidiness. Both readers write the ONE
    // shared `last_event_id` slot, and nothing orders those two writes against
    // each other: pushing both frames and then draining lets the GET reader's
    // write land last under CI load, so an assertion about "the most recent id"
    // would be measuring the scheduler. Observing each delivery before pushing the
    // next makes the shared slot's value a fact rather than a race, because the
    // write happens before the send that `receive()` pops.
    server
        .push_frame(progress_frame(GET_STREAM_EVENT_ID, "fence-20", 1, None))
        .await;
    assert!(
        drain_until_progress(&mut transport, 1.0).await,
        "the GET session stream's id-carrying frame must be DELIVERED, so its cursor is recorded by \
         the same production path a live stream records it on"
    );
    assert_eq!(
        transport.last_event_id().as_deref(),
        Some(GET_STREAM_EVENT_ID),
        "with only the GET having delivered, the transport-wide accessor reports the GET's id"
    );

    server
        .push_post_frame(progress_frame(POST_STREAM_EVENT_ID, "fence-20", 2, None))
        .await;
    assert!(
        drain_until_progress(&mut transport, 2.0).await,
        "the streaming POST response's id-carrying frame must be DELIVERED too — a cursor that was \
         never minted cannot be the one that leaks"
    );

    // The transport-wide accessor's meaning is deliberately UNCHANGED: it is
    // "the most recent id seen on ANY stream", and the most recent here is the
    // POST's. This assertion is what pins that only the RECONNECT cursor was
    // promoted to per-reader state — a fix that "corrected" this accessor to
    // report the session stream's id would change a public, documented behaviour.
    assert_eq!(
        transport.last_event_id().as_deref(),
        Some(POST_STREAM_EVENT_ID),
        "StreamableHttpTransport::last_event_id() must still report the most recent id from ANY \
         stream — the POST's. Only the reconnect cursor moves"
    );

    // Now the proxy blinks on BOTH bodies, and the session stream reconnects.
    server.end_live_sse_streams();
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= 2).await,
        "the session stream must be RE-OPENED after it is dropped. Observed request lines: {:?}",
        server.request_lines()
    );

    let cursors = server.get_cursors();
    assert!(
        cursors.len() >= 2,
        "the harness must have recorded a cursor slot per GET; got {cursors:?}"
    );
    assert_eq!(
        cursors[0], None,
        "the FIRST open has nothing to resume from and must carry no cursor at all; got {cursors:?}"
    );
    assert_eq!(
        cursors[1].as_deref(),
        Some(GET_STREAM_EVENT_ID),
        "the re-opened GET must resume from the id THIS STREAM delivered, never from one minted on \
         a streaming POST response. MCP resumability is per-stream: replaying another stream's \
         cursor asks the server to resume from a position belonging to a different stream, which \
         is a rejection, a replay of the wrong frames, or a SILENT GAP — and a silent gap is \
         exactly the failure the reconnect exists to prevent, and is indistinguishable from a \
         healthy resume (T-118.2-17-03). Observed cursors: {cursors:?}"
    );

    server.shutdown().await;
}

// ===========================================================================
// Fences 21 and 22 — BLOCKER 1: the terminal latch pre-empts an in-flight,
// SSE-delivered POST response, permanently.
//
// `118.2-15` moved terminal stream reasons off the response FIFO onto a
// write-once latch, and `drain_or_latch` surfaces that latch the moment
// `try_recv()` reports `Empty`. On the POST-answered-with-`text/event-stream`
// path this phase introduced for D-01, `post_body` spawns a DETACHED reader and
// returns `Ok(())` BEFORE the answer lands on the queue — so the queue is
// legitimately, transiently empty while a real answer is still on the wire, and
// the latch wins instantly with a stale reason belonging to a DIFFERENT stream.
//
// Fence 16 cannot reach it: its harness answers `content-type:
// application/json`, so the response lands on the queue synchronously inside
// `send()` and the queue-wins-over-latch rule always fires. That is precisely
// why this defect shipped green, and why fence 21 re-runs fence 16's scenario
// against an SSE-ANSWERED POST.
// ===========================================================================

// ===========================================================================
// Fence 21 — a latched session-stream reason does not pre-empt a caller whose
// own POST-response stream is still in flight, and does not do so forever.
// ===========================================================================

#[tokio::test]
async fn a_latched_session_stream_does_not_pre_empt_an_sse_answered_call() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    // The whole difference from fence 16: the SAME answers, delivered over a
    // `text/event-stream` POST response by a detached reader, AFTER `post_body`
    // has already returned.
    server.answer_calls_with_sse();
    // Fence 16's preamble verbatim in structure: every GET opens successfully and
    // ends at once, so the session stream spends its WHOLE reconnect budget and
    // latches its named terminal reason while the application is idle.
    server.close_get_streams_on_accept();

    // Bound only to keep the transport alive for the whole fence. `receive()` is
    // NEVER called on it: it shares the receive queue with the clone the client
    // owns, so a read here would STEAL the very response these assertions are
    // about.
    let (client, _observer) = handshake(&server).await;

    let expected_gets = 1 + SHIPPED_RECONNECT_BUDGET;
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= expected_gets).await,
        "the session stream must spend its budget before the fence calls anything — otherwise \
         there is no latched reason to be pre-empted by. Observed request lines: {:?}",
        server.request_lines()
    );
    // An OBSERVATION window, not a synchronisation device: it proves the
    // reconnect loop has STOPPED, which establishes both halves of the
    // precondition — the terminal reason has been latched, and it was latched
    // with NO request in flight. Every wait after this point goes through a bound.
    tokio::time::sleep(RECONNECT_QUIET).await;
    assert_eq!(
        server.get_lines(),
        expected_gets,
        "the budget must be spent and the loop finished before the first call. Observed request \
         lines: {:?}",
        server.request_lines()
    );

    // --- Call 1: its OWN answer, delivered off-queue by a detached reader. ---
    let first = timeout(
        BOUND,
        client.call_tool("fence-21-one".to_string(), json!({})),
    )
    .await
    .expect("the first call completes within BOUND")
    .unwrap_or_else(|error| {
        panic!(
            "a tools/call answered over text/event-stream came back reporting a SESSION-STREAM \
             failure raised while the application was idle. `post_body` spawns a DETACHED reader \
             for a `text/event-stream` response and returns Ok(()) before the answer lands on the \
             queue, so `drain_or_latch` sees an EMPTY queue with a real answer still on the wire \
             and surfaces another stream's diagnosis as this caller's result. The latch is \
             Arc-shared across every clone, written once, and cleared by no constructor, no \
             start_sse and no close — so the FIRST trip fails every later call against an \
             SSE-answering server for the life of the process (BLOCKER 1). Got: {error}"
        )
    });
    let first_text = text_of(&first);
    assert_eq!(
        first_text.as_deref(),
        Some(call_marker(1).as_str()),
        "call 1 must receive call 1's OWN result. Expected {:?}, got {first_text:?}",
        call_marker(1)
    );

    // --- Call 2: proves the failure is not merely delayed, but not PERMANENT. ---
    //
    // A latch with no reset seam converts one transient session-stream event into
    // a process-lifetime failure, which is a severity WORSENING over the
    // pre-closure behaviour where a bad terminal reason could poison at most one
    // call. A second call with its own distinct marker is what measures that.
    let second = timeout(
        BOUND,
        client.call_tool("fence-21-two".to_string(), json!({})),
    )
    .await
    .expect("the second call completes within BOUND")
    .unwrap_or_else(|error| {
        panic!(
            "the SECOND SSE-answered call also failed with a latched reason. One latched \
             session-stream reason must not poison the transport for the life of the process \
             (BLOCKER 1). Got: {error}"
        )
    });
    let second_text = text_of(&second);
    assert_eq!(
        second_text.as_deref(),
        Some(call_marker(2).as_str()),
        "call 2 must receive call 2's OWN distinct result, not call 1's. Expected {:?}, got \
         {second_text:?}",
        call_marker(2)
    );

    assert_eq!(
        calls_observed(&server),
        2,
        "both calls must actually have reached the wire — a fence that asserts on results it \
         never asked for would pass against any tree. Observed POST bodies: {:?}",
        server.post_bodies()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 22 — a successful session-stream re-open CLEARS the terminal latch.
//
// Drives the transport directly rather than through a `Client`: a client receive
// loop would compete for the very queue this fence reads.
// ===========================================================================

#[tokio::test]
async fn a_reopened_session_stream_clears_the_terminal_latch() {
    let server = RecordingServer::start().await;
    server.close_get_streams_on_accept();
    let mut transport = transport_for(&server);

    open_stream(&mut transport).await;

    let expected_gets = 1 + SHIPPED_RECONNECT_BUDGET;
    assert!(
        wait_for_within(RECONNECT_BOUND, || server.get_lines() >= expected_gets).await,
        "the session stream must spend its budget, so there IS a latched reason to clear. \
         Observed request lines: {:?}",
        server.request_lines()
    );
    // No quiet window is needed here (unlike fence 21, whose `assert_eq!` on the
    // GET count requires one). The assertion below is CAUSAL: `receive()` parks
    // until the latch is written, and the latch is only written once the budget
    // is spent and `run_session_stream` returns. BOUND covers that.

    // With nothing in flight and no recovery yet, answering the latched reason is
    // CORRECT behaviour — that is the CR-02 contract and this fence does not
    // weaken it. It is asserted here so the fence's second half cannot pass
    // merely because no reason was ever raised.
    let latched = timeout(BOUND, transport.receive())
        .await
        .expect("with the budget spent and nothing in flight, receive() answers within BOUND")
        .expect_err("a spent reconnect budget is a terminal reason, not a message");
    let latched_text = latched.to_string();
    assert!(
        latched_text.contains(RECONNECT_PHRASE),
        "the latched reason must be the spent reconnect budget, or this fence's second half is \
         measuring something else. Got: {latched_text:?}"
    );

    // Now the peer recovers: GETs are held open again, and the transport re-opens
    // its session stream successfully.
    server.reopen_get_streams_held_open();
    timeout(BOUND, transport.start_sse(None))
        .await
        .expect("the re-open completes within BOUND")
        .expect("the harness answers the re-opened GET 200 text/event-stream");
    assert!(
        wait_for(|| server.open_get_connections() >= 1).await,
        "the re-opened session stream must actually be LIVE — a re-open that never connected \
         proves nothing about the reset seam. Observed request lines: {:?}",
        server.request_lines()
    );

    // The subject: with the queue empty, no reader in flight and a RECOVERED
    // session stream, receive() must WAIT rather than answer the stale reason.
    //
    // `started` is a DIAGNOSTIC, reported only on the failing arms. It is never
    // asserted on: an elapse here is a cancellation ceiling, not the thing under
    // test, and a fence that asserted an upper time bound as its subject would be
    // measuring CI load. Reporting it is what makes the RED capture quantitative
    // — a stale answer arrives in microseconds where a correct wait cannot.
    let started = std::time::Instant::now();
    match timeout(LATCH_RESET_BOUND, transport.receive()).await {
        Err(_elapsed) => {},
        Ok(Ok(message)) => panic!(
            "receive() returned a message on a stream nothing was pushed down after {:?}; this \
             fence's subject is that it WAITS. Got: {message:?}",
            started.elapsed()
        ),
        Ok(Err(error)) => {
            let text = error.to_string();
            let elapsed = started.elapsed();
            assert!(
                !text.contains(RECONNECT_PHRASE),
                "a transport whose session stream RECOVERED still answered the stale terminal \
                 reason from before the recovery, after {elapsed:?}. The latch is written once and \
                 cleared by no constructor, no start_sse and no close, so a transient network \
                 event becomes a permanent, process-lifetime failure with no way back (BLOCKER 1). \
                 A successful start_sse re-open must clear it. Got: {text:?}"
            );
            panic!(
                "receive() answered an error after {elapsed:?} on a recovered transport with \
                 nothing in flight. Got: {text:?}"
            );
        },
    }

    server.shutdown().await;
}

// ===========================================================================
// Fence 23 — BLOCKER 2, half A: a mis-addressed frame must not wedge every
// OTHER operation on the same client.
//
// The defect is a STATEMENT SHAPE, not a logic error:
//
//     let response_message = self.transport.write().await.receive().await?;
//
// The `RwLockWriteGuard` temporary lives to the end of the `let` STATEMENT, so
// it is held across `.receive().await`. The correlation branch `continue`s
// rather than returning, so the guard is re-taken and re-held on every
// mis-addressed frame — and `send_notification`, `call_tool` and `close` all
// need that same lock. One frame with a re-typed id therefore takes the whole
// client down: no error, no log after the first `warn!`, no recovery.
//
// This fence's subject is the LOCK, and it deliberately says nothing about the
// call's return value — that is fence 24's subject. One fence cannot stand for
// both: a fence that watched the call would pass on a tree that released the
// lock only by failing the call, and a fence that watched only the failure would
// pass on a tree that still serialised every other caller behind it.
// ===========================================================================

#[tokio::test]
async fn a_mismatched_frame_does_not_block_another_operation_on_the_same_client() {
    /// Which `select!` arm finished first. Named rather than a bool so the
    /// failure message can say what actually happened.
    enum Arm {
        /// The `tools/call` returned — this fence is then measuring nothing,
        /// because there is no discard in flight to contend with.
        Call,
        /// The second operation completed while the discard was running.
        Probe(pmcp::Result<()>),
    }

    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_the_next_call_with_a_mismatched_id();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so nothing here can be blamed on a terminal \
         reason. Observed request lines: {:?}",
        server.request_lines()
    );

    let started = std::time::Instant::now();
    let outcome = timeout(LOCK_PROBE_BOUND, async {
        tokio::select! {
            _called = client.call_tool("fence-23".to_string(), json!({})) => Arm::Call,
            probed = async {
                // Wait for the mis-addressed answer to have been WRITTEN, not
                // merely for the POST to have arrived. Only then has the client
                // had a frame to pop, mismatch on and begin discarding — so only
                // then is there a held lock to contend for.
                assert!(
                    wait_for(|| server.answers_written() >= 1).await,
                    "the harness must have written the mis-addressed answer, or this fence would \
                     be probing a lock nobody has taken. Observed POST bodies: {:?}",
                    server.post_bodies()
                );
                // An observation window, not a synchronisation device: it proves
                // the client has had time to pop the frame and settle into the
                // discard loop, so a pass cannot come from probing before the
                // loop ever started.
                tokio::time::sleep(QUIET).await;
                client
                    .cancel_request(&RequestId::from("fence-23-probe"))
                    .await
            } => Arm::Probe(probed),
        }
    })
    .await;

    let Ok(finished) = outcome else {
        panic!(
            "a SECOND operation on the same client never completed in {LOCK_PROBE_BOUND:?} while \
             a mis-addressed response frame was in flight. What can fail this at THIS head is a \
             pump step that does not release the transport write guard between slices: \
             `pump_once` takes the guard, races ONE `receive()` against `PUMP_RECEIVE_SLICE`, \
             routes whatever arrived, and drops the guard before looping. A guard held across an \
             unsliced receive — or re-taken without being released — serialises every other \
             `send`, `call_tool` and `close` on this client behind a wait that never ends, so one \
             frame from a hostile or merely lenient peer takes the whole client down \
             (T-118.2-20-01). Observed POST bodies at the elapse: {:?}",
            server.post_bodies()
        )
    };
    match finished {
        Arm::Call => panic!(
            "the `tools/call` returned after {:?}, before the probe ran. This fence's subject is \
             the lock being released DURING the discard, so a run in which the discard was no \
             longer in flight measures nothing. LOCK_PROBE_BOUND ({LOCK_PROBE_BOUND:?}) must stay \
             below the client's discard ceiling for that reason. Observed POST bodies: {:?}",
            started.elapsed(),
            server.post_bodies()
        ),
        Arm::Probe(result) => {
            result.expect("the second operation reached the wire and was answered 202");
        },
    }

    // Vacuity guards. Both are needed: the call must have been sent (or there
    // was no discard), and the probe must have reached the WIRE (or "the second
    // operation completed" is a statement about a no-op).
    assert_eq!(
        calls_observed(&server),
        1,
        "the `tools/call` must actually have been sent, or there is no discard for the probe to \
         contend with. Observed POST bodies: {:?}",
        server.post_bodies()
    );
    let cancelled_frames = server
        .post_bodies()
        .iter()
        .filter(|body| {
            body.get("method").and_then(Value::as_str) == Some("notifications/cancelled")
        })
        .count();
    assert_eq!(
        cancelled_frames,
        1,
        "the probe's notification must be observable on the wire — a fence concluding 'the second \
         operation completed' about an operation that never left the process would pass against \
         any tree. Observed POST bodies: {:?}",
        server.post_bodies()
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 24 — per-id routing: a frame nobody is awaiting is nobody's answer.
//
// This fence previously asserted `Error::Timeout` within a bound — the error
// KIND of plan 20's private discard ceiling. That ceiling existed only because a
// popped frame had nowhere to go, so whichever caller popped it had to decide
// what to do with a frame that was not its own. Per-id routing removes the
// decision: the pump delivers a frame to whoever registered its id, and drops
// one nobody registered. Asserting the workaround's error kind would now pin
// machinery that is deliberately gone.
//
// WHAT THIS FENCE DELIBERATELY DOES NOT ASSERT: that the call eventually fails.
// The harness mis-addresses this call's ONLY answer, so no answer for it is ever
// sent, and `pmcp::Client` applies no request timeout — `RequestOptions::timeout`
// is public but unreachable (no API accepts a `RequestOptions`) and
// `dispatch_request` has never read it. A call whose answer never arrives
// therefore waits, exactly as it did before plan 20 and exactly as any other
// unanswered request does. That gap is REAL and OPEN; it is recorded rather than
// papered over, and its fix is a per-request deadline, not a mismatch-specific
// ceiling.
//
// Liveness of the REST of the client under this same condition is fences 25 and
// 26's job, so a pass here cannot come from the client being wedged.
// ===========================================================================

#[tokio::test]
async fn a_mismatched_frame_is_not_delivered_as_this_calls_answer() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_the_next_call_with_a_mismatched_id();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so nothing here can be blamed on a terminal \
         reason. Observed request lines: {:?}",
        server.request_lines()
    );

    let outcome = timeout(
        MISMATCH_STILL_WAITING,
        client.call_tool("fence-24".to_string(), json!({})),
    )
    .await;

    match outcome {
        Err(_still_waiting) => {
            // Routed to nobody; this call is still waiting for an answer that
            // was never sent. Correct.
        },
        Ok(Ok(result)) => panic!(
            "the client returned a server response addressed to {MISMATCHED_CALL_ID:?} as the \
             answer to a request it never identified. Per-id routing must deliver a frame only \
             to the caller that registered its id. Returned result: {:?}",
            text_of(&result)
        ),
        Ok(Err(error)) => panic!(
            "the call FAILED rather than continuing to wait. A frame addressed to another id is \
             not this call's business and must not fail it — that would let one peer's \
             mis-addressed frame break an unrelated in-flight request. Error: {error}"
        ),
    }
}

/// How long fence 24 watches a call that must still be WAITING (plan 22).
///
/// Short on purpose: it is proving a negative ("has not returned"), and a longer
/// watch buys no extra confidence while costing wall clock on every run.
const MISMATCH_STILL_WAITING: Duration = Duration::from_secs(3);

/// How long two concurrent calls get to BOTH complete (plan 22, fence 25).
///
/// Generous on purpose: the point is to separate "both answered" from "one
/// caller's answer was destroyed", not to measure latency. It sits above
/// `UNMATCHED_RESPONSE_TIMEOUT` (10 s), the ceiling a call can die at, so a run
/// that fails does so because an answer is gone, not because this bound is
/// tighter than the mechanism under test.
///
/// (This doc block previously sat glued to the END of `MISMATCH_STILL_WAITING`'s,
/// so rustdoc attributed it to that constant and `CONCURRENT_CALLS_BOUND` had
/// none at all. Reattached by plan 24 in the same pass that corrected the
/// constant it names.)
const CONCURRENT_CALLS_BOUND: Duration = Duration::from_secs(15);

/// Fence 25 (plan 22, CR-02): two concurrent calls on ONE client each receive
/// their OWN answer.
///
/// The harness answers newest-first, so the client is guaranteed to pop a frame
/// belonging to the OTHER caller before it pops its own. Before per-id routing
/// there was nowhere to put that frame — a `Transport` consumer holds no
/// producer handle — so it was DISCARDED, and the caller it belonged to waited
/// for an answer that had already been destroyed, until the then-current ceiling
/// (10 s, today `UNMATCHED_RESPONSE_TIMEOUT`) failed it. That is the whole of
/// CR-02: not a wrong answer, a deleted one.
///
/// The assertion is deliberately on the SET of markers rather than on which
/// caller got which. Answer numbering follows arrival order at the server, which
/// is not the property under test; "two distinct real answers, one per caller"
/// is.
///
/// # It covers ONE half of CR-02
///
/// This fence covers the CONCURRENT half: two callers alive at once, and neither
/// answer destroyed. The SEQUENTIAL half — a caller that already DIED, whose own
/// answer then arrives with no owner and is charged to the next call's unmatched
/// budget — is [`debris_from_a_dead_call_does_not_charge_the_next_calls_budget`].
/// Neither implies the other, and concurrency coverage alone does not close the
/// chain: nothing here has a dead call in it.
#[tokio::test]
async fn two_concurrent_calls_each_receive_their_own_answer() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_calls_with_sse();
    server.answer_calls_newest_first();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so nothing here can be blamed on a terminal \
         reason. Observed request lines: {:?}",
        server.request_lines()
    );

    let both = timeout(CONCURRENT_CALLS_BOUND, async {
        tokio::join!(
            client.call_tool("fence-25-a".to_string(), json!({})),
            client.call_tool("fence-25-b".to_string(), json!({})),
        )
    })
    .await;

    let Ok((first, second)) = both else {
        panic!(
            "two concurrent calls on one client did not BOTH complete in \
             {CONCURRENT_CALLS_BOUND:?}. One caller's answer was popped by the other caller and \
             discarded, because a popped frame had no owner to be delivered to. Observed POST \
             bodies: {:?}",
            server.post_bodies()
        )
    };

    let first = first.expect("the first concurrent call must be answered, not destroyed");
    let second = second.expect("the second concurrent call must be answered, not destroyed");

    let mut markers = vec![
        text_of(&first).expect("the first call's result must carry its marker"),
        text_of(&second).expect("the second call's result must carry its marker"),
    ];
    assert_ne!(
        markers[0], markers[1],
        "each caller must receive its OWN answer; identical markers mean one answer was \
         delivered twice and the other was lost"
    );
    markers.sort();
    assert_eq!(
        markers,
        vec![call_marker(1), call_marker(2)],
        "between them the two callers must hold exactly the two answers the server wrote"
    );
}

/// Fence 26 (plan 22, CR-01): an outstanding call does not serialise every other
/// operation on the same client.
///
/// The transport is ONE object behind ONE lock — `send` and `receive` both take
/// `&mut self` — so whoever is receiving holds the write guard. Before this plan
/// the guard was held across an UNBOUNDED `receive()` on the ordinary path, so a
/// perfectly well-behaved long-running call blocked every other `send`,
/// `call_tool` and `close` on the client for as long as the peer chose.
///
/// Here the harness holds the first call's answer, so a call IS outstanding, and
/// the fence asserts a second, unrelated operation still completes. It asserts
/// nothing about the call itself — fence 25 owns that — so a pass cannot come
/// from the call finishing early.
///
/// This fence covers the RECEIVE path and fence 30 covers the SEND path;
/// NEITHER subsumes the other. The harness here answers the POST head promptly
/// and holds the answer FRAME, which lets `send()` return, so a tree that
/// released the guard around the receive but kept it across the send passes this
/// fence and fails fence 30.
#[tokio::test]
async fn an_outstanding_call_does_not_block_another_operation() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_calls_with_sse();
    server.answer_calls_newest_first();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open. Observed request lines: {:?}",
        server.request_lines()
    );

    let probed = timeout(LOCK_PROBE_BOUND, async {
        tokio::select! {
            _call = client.call_tool("fence-26-held".to_string(), json!({})) => None,
            probe = async {
                // Wait until the held call is genuinely in flight — its POST has
                // arrived and the harness has begun holding its answer — so the
                // probe cannot pass by running before there is any contention.
                assert!(
                    wait_for(|| server.post_bodies().len() >= 3).await,
                    "the held call's POST must have arrived, or this fence probes an \
                     uncontended lock. Observed POST bodies: {:?}",
                    server.post_bodies()
                );
                Some(client.cancel_request(&RequestId::from("fence-26-probe")).await)
            } => probe,
        }
    })
    .await;

    let Ok(outcome) = probed else {
        panic!(
            "a second operation on the same client never completed in {LOCK_PROBE_BOUND:?} while \
             an ordinary call was outstanding. What can fail this at THIS head is a pump step \
             that does not release the transport write guard between slices: `pump_once` takes \
             the guard, races one `receive()` against `PUMP_RECEIVE_SLICE`, and must DROP it \
             before looping, so a waiter gets its turn once per slice. A guard re-taken without \
             being released — or a receive awaited outside the slice race — serialises every \
             other operation on this client behind however long the peer takes to answer. (The \
             SEND-path twin of this is fence 30's subject, not this one's.) Observed POST bodies \
             at the elapse: {:?}",
            server.post_bodies()
        )
    };
    assert!(
        outcome.is_some(),
        "the outstanding call completed before the probe could run, so this fence measured \
         nothing. The harness must hold that answer until the probe has finished."
    );
}

// ===========================================================================
// Fence 27 — CR-02's SURVIVING half: a dead call's own answer must not be
// charged to the NEXT call's unmatched budget.
// ===========================================================================

/// The unmatched-response COUNT bound the client ships, restated here
/// (plan 22, CR-02).
///
/// `MAX_UNMATCHED_RESPONSES` is a PRIVATE constant in `src/client/mod.rs` — for
/// the same never-a-semver-event reason [`SHIPPED_RECONNECT_BUDGET`] records
/// about the transport's knobs — so an integration test cannot import it.
/// Restating it is safe because a drift is caught by the very fence that uses
/// it: call one's failure names the count it actually reached, and
/// [`debris_from_a_dead_call_does_not_charge_the_next_calls_budget`] asserts on
/// that number.
const SHIPPED_UNMATCHED_RESPONSE_BUDGET: usize = 32;

/// Upper bound on the WHOLE three-call chain (plan 22, CR-02).
///
/// Generous, and NOT the subject: nothing in this fence is timed. Its only job
/// is to keep a regression from hanging CI, which is why the panic text says a
/// timeout here means a call never returned at all rather than that a budget
/// fired.
const DEBRIS_CHAIN_BOUND: Duration = Duration::from_secs(45);

/// Fence 27 (plan 22, CR-02 surviving half): the answer to a call that already
/// stopped waiting must not spend the NEXT call's unmatched budget.
///
/// # The mechanism
///
/// A call that dies at its ceiling is removed from `active_requests` by
/// `dispatch_request`'s exit cleanup, but its real answer is still on the wire
/// behind the frames that killed it. That answer lands on the transport's
/// SHARED receive queue with no owner. The next call pops it, finds no live
/// owner, and — before the fix — booked it `PumpStep::Unmatched`, spending one
/// unit of its OWN budget and arming its OWN `UNMATCHED_RESPONSE_TIMEOUT` on
/// frame one. That is CR-02's shape one rename later: one stray frame plus a
/// slow peer fails every LATER call on the client, permanently.
///
/// # The arithmetic that makes RED deterministic
///
/// Call one is sprayed exactly [`SHIPPED_UNMATCHED_RESPONSE_BUDGET`] frames
/// addressed to nobody, so it dies ON THE LAST ONE — by COUNT — while its own
/// answer is still behind them. Call two is sprayed exactly one FEWER. On a tree
/// that books call one's leftover answer as unmatched, call two reaches
/// `31 + 1 == 32` and fails; on a tree that absorbs it, call two stays at `31`,
/// one under the bound, and is answered. Call three is sprayed nothing at all
/// and must simply be answered, which is what makes this a fence on the CHAIN
/// rather than on a single failure.
///
/// # No wall clock appears anywhere in it
///
/// `UnmatchedBudget` fires on `seen >= MAX_UNMATCHED_RESPONSES` **or** on an
/// elapsed deadline, and only the COUNT is used here. The deadline is built from
/// `web_time::Instant::now()`, which on native IS `std::time::Instant`:
/// `tokio::time::pause()` does not move it, so a paused-clock fence could never
/// expire it and a real-time fence would need eleven seconds of wall clock and
/// would be load-sensitive. [`DEBRIS_CHAIN_BOUND`] is a hang guard, not a
/// measurement.
#[tokio::test]
async fn debris_from_a_dead_call_does_not_charge_the_next_calls_budget() {
    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();
    server.answer_calls_with_sse();
    server.spray_unowned_frames_before_each_answer(&[
        SHIPPED_UNMATCHED_RESPONSE_BUDGET,
        SHIPPED_UNMATCHED_RESPONSE_BUDGET - 1,
        0,
    ]);

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so nothing here can be blamed on a terminal \
         reason. Observed request lines: {:?}",
        server.request_lines()
    );

    let chain = timeout(DEBRIS_CHAIN_BOUND, async {
        let first = client
            .call_tool("fence-27-dies".to_string(), json!({}))
            .await;
        let second = client
            .call_tool("fence-27-next".to_string(), json!({}))
            .await;
        let third = client
            .call_tool("fence-27-after".to_string(), json!({}))
            .await;
        (first, second, third)
    })
    .await;

    let Ok((first, second, third)) = chain else {
        panic!(
            "the three-call chain did not complete in {DEBRIS_CHAIN_BOUND:?}. This bound is a \
             HANG guard, not the subject: a timeout here means one of the three calls never \
             RETURNED AT ALL, which is a different defect from a budget firing — a budget that \
             fires returns a named error promptly. Observed POST bodies: {:?}",
            server.post_bodies()
        )
    };

    // Call one must die the way the construction intends. Without this a later
    // refactor could make it die some other way and the fence would stop
    // measuring the chain while still passing.
    let died = first.expect_err(
        "call one is sprayed exactly the shipped unmatched-response budget, so it MUST fail at \
         the COUNT bound. A success here means the spray never reached the client and every \
         later assertion in this fence is vacuous",
    );
    let died = died.to_string();
    assert!(
        died.contains(&format!(
            "{SHIPPED_UNMATCHED_RESPONSE_BUDGET} response id(s) no request is awaiting"
        )),
        "call one must fail at the COUNT bound, naming {SHIPPED_UNMATCHED_RESPONSE_BUDGET}. A \
         failure naming a different number means the restated \
         SHIPPED_UNMATCHED_RESPONSE_BUDGET has drifted from `MAX_UNMATCHED_RESPONSES` in \
         src/client/mod.rs; a failure naming none means it died of something other than the \
         budget and this fence is no longer measuring the chain. Observed error: {died}"
    );

    let second = second.unwrap_or_else(|error| {
        panic!(
            "call two must be ANSWERED. It was sprayed {} frames — one UNDER the bound — so the \
             only way it can reach {SHIPPED_UNMATCHED_RESPONSE_BUDGET} is by also being charged \
             for call one's leftover answer, which is our OWN debris and belongs to nobody's \
             budget. That is CR-02's surviving half: one stray frame plus a peer this client \
             already stopped waiting for fails every LATER call on the same client. Observed \
             error: {error}",
            SHIPPED_UNMATCHED_RESPONSE_BUDGET - 1,
        )
    });
    let third = third.expect(
        "call three was sprayed NOTHING and must simply be answered. A failure here means the \
         charge did not stop at call two — the budget damage is chaining forward, which is the \
         permanence half of the defect",
    );

    let second = text_of(&second).expect("call two's result must carry its marker");
    let third = text_of(&third).expect("call three's result must carry its marker");
    assert_ne!(
        second, third,
        "each surviving call must receive its OWN answer; identical markers mean one answer was \
         delivered twice"
    );
    assert_eq!(
        (second.as_str(), third.as_str()),
        (call_marker(2).as_str(), call_marker(3).as_str()),
        "the three calls are SEQUENTIAL, so the harness answers them 1, 2, 3. Call two must hold \
         answer 2 and call three answer 3. Either holding {} would mean the dead call's debris \
         was DELIVERED to a live caller rather than absorbed, which is a routing defect and not \
         a budget one",
        call_marker(1),
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 28 — plan 25: two concurrent 401s on ONE transport must refresh the
// token ONCE, and must VEND it once.
// ===========================================================================

/// Upper bound on the rendezvous that puts both 401 recoveries in flight at the
/// same time (plan 25).
///
/// NOT a settle time and NOT the subject: nothing in fence 28 is timed. It is
/// the same OR-shaped construction
/// [`get_head_rendezvous`](Shared::get_head_rendezvous) documents, applied to
/// the token vend. On a tree where both callers vend, both park and the gate
/// opens the instant the second arrives — this bound is never reached. On a
/// tree where exactly one caller vends, a pure two-arrival barrier would hang
/// forever, so the grace is what lets the FIXED tree terminate and be asserted
/// on. Deliberately far below [`BOUND`], so the sends it delays cannot time out
/// because of it.
const VEND_RENDEZVOUS_GRACE: Duration = Duration::from_millis(750);

/// The token [`CachingAuthProvider`] starts life holding.
///
/// Primed rather than empty so the FIRST request build on each caller is a
/// cache HIT. An empty cache would put two vends on the wire before any 401 had
/// happened, and the fence's subject — vends performed during the 401 recovery
/// — would be buried under them.
const PRIMED_TOKEN: &str = "primed-token";

/// An `AuthProvider` double that MODELS A CACHE, and records concurrency inside
/// the VEND path (plan 25).
///
/// The cache is not decoration. `on_unauthorized` only EVICTS; the network round
/// trip that presents a rotating refresh token happens in the SUBSEQUENT
/// `get_access_token`, which the transport reaches while REBUILDING the retry
/// request — the trait's own rustdoc states that division. A double whose
/// `get_access_token` merely returns a `String` therefore measures the operation
/// ADJACENT to the one that breaks, and would go green with the hazard live.
#[derive(Debug)]
struct CachingAuthProvider {
    /// The cached access token, `None` once evicted.
    cached: Mutex<Option<String>>,
    /// How many times `get_access_token` took the VEND path (cache miss).
    vends: AtomicUsize,
    /// Vends currently INSIDE the vend path.
    vends_live: AtomicUsize,
    /// The high-water mark of [`Self::vends_live`] — the number this fence
    /// exists to hold at one.
    vends_peak: AtomicUsize,
    /// How many times `on_unauthorized` was called.
    purges: AtomicUsize,
    /// Purges currently inside `on_unauthorized`.
    purges_live: AtomicUsize,
    /// The high-water mark of [`Self::purges_live`].
    purges_peak: AtomicUsize,
    /// The serial of the next minted token, so two vends are distinguishable.
    minted: AtomicUsize,
    /// The gate every vend parks on until the fence releases it.
    ///
    /// A `watch` rather than a `Notify` for the reason
    /// [`close_live`](Shared::close_live) records: `notify_waiters` wakes only
    /// tasks ALREADY parked, so a release raised before the second vend arrived
    /// would be lost and the fence would hang instead of failing on its subject.
    release: watch::Sender<bool>,
}

impl CachingAuthProvider {
    fn primed(token: &str) -> Self {
        let (release, _) = watch::channel(false);
        Self {
            cached: Mutex::new(Some(token.to_string())),
            vends: AtomicUsize::new(0),
            vends_live: AtomicUsize::new(0),
            vends_peak: AtomicUsize::new(0),
            purges: AtomicUsize::new(0),
            purges_live: AtomicUsize::new(0),
            purges_peak: AtomicUsize::new(0),
            minted: AtomicUsize::new(0),
            release,
        }
    }

    /// A provider whose cache is EMPTY — the state every process starts in.
    ///
    /// [`Self::primed`]'s warm cache is what makes fence 28 measure the `401`
    /// RECOVERY specifically. Fence 30 needs the opposite: the cold cache a
    /// first request meets, where the vend happens during the ORDINARY request
    /// build and no `401` is involved at all.
    fn cold() -> Self {
        let (release, _) = watch::channel(false);
        Self {
            cached: Mutex::new(None),
            vends: AtomicUsize::new(0),
            vends_live: AtomicUsize::new(0),
            vends_peak: AtomicUsize::new(0),
            purges: AtomicUsize::new(0),
            purges_live: AtomicUsize::new(0),
            purges_peak: AtomicUsize::new(0),
            minted: AtomicUsize::new(0),
            release,
        }
    }

    fn vends(&self) -> usize {
        self.vends.load(Ordering::SeqCst)
    }

    fn vends_peak(&self) -> usize {
        self.vends_peak.load(Ordering::SeqCst)
    }

    fn purges(&self) -> usize {
        self.purges.load(Ordering::SeqCst)
    }

    fn purges_peak(&self) -> usize {
        self.purges_peak.load(Ordering::SeqCst)
    }

    /// Let every parked vend proceed, and every later one through unhindered.
    fn release_vends(&self) {
        self.release.send_replace(true);
    }
}

#[async_trait]
impl AuthProvider for CachingAuthProvider {
    async fn get_access_token(&self) -> pmcp::Result<String> {
        // A cache HIT is the loser's path on a fixed tree: no identity provider
        // is touched, so nothing is recorded and no gate is entered.
        //
        // Read into a local FIRST so the guard is dropped before the branch —
        // a lock guard living in an `if let` scrutinee outlives the whole
        // arm, and this one is held across the vend's await if it does.
        let cached = self.cached.lock().clone();
        if let Some(token) = cached {
            return Ok(token);
        }

        // THE VEND. Everything this fence measures happens here.
        self.vends.fetch_add(1, Ordering::SeqCst);
        let live = self.vends_live.fetch_add(1, Ordering::SeqCst) + 1;
        self.vends_peak.fetch_max(live, Ordering::SeqCst);

        // Park INSIDE the vend, so the peak above is observed while both callers
        // are still holding it rather than after one has already left.
        let mut release = self.release.subscribe();
        if !*release.borrow() {
            let _ = release.wait_for(|open| *open).await;
        }

        let serial = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
        let token = format!("vended-{serial}");
        *self.cached.lock() = Some(token.clone());
        self.vends_live.fetch_sub(1, Ordering::SeqCst);
        Ok(token)
    }

    async fn on_unauthorized(&self) -> pmcp::Result<()> {
        self.purges.fetch_add(1, Ordering::SeqCst);
        let live = self.purges_live.fetch_add(1, Ordering::SeqCst) + 1;
        self.purges_peak.fetch_max(live, Ordering::SeqCst);
        *self.cached.lock() = None;
        // A yield, so two purges that genuinely overlap are OBSERVED to overlap
        // rather than being flattened by a single-threaded runtime running each
        // to completion. The purge peak is a secondary assertion for exactly
        // this reason — see the fence's doc comment.
        tokio::task::yield_now().await;
        self.purges_live.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Fence 28 (plan 25): two concurrent POSTs on ONE transport that both receive a
/// `401` produce exactly ONE `on_unauthorized` call and exactly ONE token VEND.
///
/// # Why this is reachable TODAY, before plan 23
///
/// `StreamableHttpTransport` is `#[derive(Clone)]` with every field an `Arc`, so
/// two clones share one config and therefore ONE `AuthProvider`. A durable agent
/// holding one transport across concurrent tasks has exactly this shape. The
/// fence drives two clones directly rather than a `Client`, so nothing about the
/// client's outer transport guard is in the picture.
///
/// # THE TWO PURGE ASSERTIONS ALONE ARE INSUFFICIENT
///
/// Do not simplify this fence back to counting `on_unauthorized`. That call only
/// EVICTS the cached token. The rotating refresh token is presented to the identity provider
/// by the SUBSEQUENT `get_access_token`, which the transport reaches while
/// rebuilding the retry request. A serialisation that ended at the purge would
/// let both callers reach `get_access_token` concurrently one step later —
/// against a cache the winner had just emptied, so both would vend — which is
/// the ORIGINAL defect with one extra step and is INVISIBLE to any fence that
/// counts purges. The vend assertions are the subject; the purge assertions are
/// corroboration.
///
/// # What a failure MEANS
///
/// Two callers presented a refresh token at the same time. Against a rotating
/// refresh token the identity provider accepts the first and REJECTS the second, and the
/// second's rejection also destroys the token the first just cached — so the
/// transport's auth fails permanently until an out-of-band re-auth.
///
/// # Why the RED is by construction and not by racing
///
/// Every vend parks on a gate the fence releases, and the fence releases it only
/// after BOTH 401s are on the wire AND either two vends are parked or
/// [`VEND_RENDEZVOUS_GRACE`] has elapsed. On a tree with no single-flight both
/// callers reach the vend and park, so the peak is two before anything is
/// released. On a serialised tree only one caller can vend, the grace opens the
/// gate, and the loser's `get_access_token` is a cache hit. Neither tree can
/// deadlock: the gate has an escape on the count AND on the grace, and the
/// serialisation the fix adds is released before the retry POST is sent.
#[tokio::test]
async fn two_concurrent_401s_refresh_the_token_once() {
    let server = RecordingServer::start().await;
    // Exactly two, one per caller's FIRST POST. Both retries fall through to the
    // ordinary 202, so a passing recovery is observable as a successful send.
    server.answer_the_next_posts_unauthorized(2);

    let provider = Arc::new(CachingAuthProvider::primed(PRIMED_TOKEN));
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(
            url::Url::parse(&server.url()).expect("the harness URL parses"),
        )
        .with_auth_provider(Arc::clone(&provider) as Arc<dyn AuthProvider>)
        .build(),
    );

    // Two CLONES of one transport — the durable-agent shape, and the reason both
    // callers share one credential cache.
    let mut one = transport.clone();
    let mut two = transport;

    // A notification that is NOT `notifications/initialized`, so neither send
    // reaches `start_sse` and this fence measures the auth path only.
    let first = tokio::spawn(async move { timeout(BOUND, one.send(cancelled())).await });
    let second = tokio::spawn(async move { timeout(BOUND, two.send(cancelled())).await });

    assert!(
        wait_for(|| server.unauthorized_answers() >= 2).await,
        "both concurrent POSTs must be answered 401 before the recovery is measured; only {} \
         were. Observed request lines: {:?}",
        server.unauthorized_answers(),
        server.request_lines(),
    );

    // Release on arrival-count OR grace. See the doc comment: the count arm is
    // what makes the unfixed tree's peak observable, the grace arm is what keeps
    // the fixed tree from hanging on its own fix.
    let _ = wait_for_within(VEND_RENDEZVOUS_GRACE, || provider.vends() >= 2).await;
    provider.release_vends();

    let first = first.await.expect("the first sender task does not panic");
    let second = second.await.expect("the second sender task does not panic");

    assert!(
        provider.vends_peak() <= 1,
        "at most ONE caller may be inside get_access_token's VEND path at a time; {} were. Two \
         callers presented the rotating refresh token to the IdP simultaneously — the second is \
         rejected and its failure destroys the token the first had just cached, so the \
         transport's auth fails permanently. NOTE: this is the assertion a purge-only \
         single-flight cannot satisfy; on_unauthorized merely EVICTS, and the vend happens one \
         step later in the retry's request rebuild",
        provider.vends_peak(),
    );
    assert_eq!(
        provider.vends(),
        1,
        "exactly ONE vend may occur across the whole 401 recovery — the loser's retry token must \
         come from the cache the winner warmed, not from a second round trip to the IdP. \
         Observed {} vends, {} purges",
        provider.vends(),
        provider.purges(),
    );
    assert!(
        provider.purges_peak() <= 1,
        "at most ONE caller may be inside on_unauthorized at a time; {} were",
        provider.purges_peak(),
    );
    assert_eq!(
        provider.purges(),
        1,
        "exactly ONE on_unauthorized call may happen: the loser's token generation was already \
         superseded by the winner's refresh, so it has nothing to purge and purging anyway would \
         evict the token the winner just cached",
    );

    first
        .expect("the first send returns within BOUND — a hang here is the fix deadlocking")
        .expect("the first send must SUCCEED after its 401 recovery; a caller may not be starved");
    second
        .expect("the second send returns within BOUND — a hang here is the fix deadlocking")
        .expect(
            "the second send must SUCCEED after its 401 recovery. A failure here means the \
             generation check skipped a refresh the caller genuinely needed, which turns a \
             token-rotation fix into a permanent auth failure",
        );

    server.shutdown().await;
}

// ===========================================================================
// Fence 29 — plan 25: two concurrent session-stream restarts leave ONE reader,
// and `close()` reaches it.
// ===========================================================================

/// How long the harness withholds the FIRST GET's head while waiting for a
/// second (plan 25).
///
/// Paid exactly once, on the serialised tree, and far below [`BOUND`] so the
/// `start_sse` it delays cannot time out because of it. On the unfixed tree it
/// is never reached at all: the second GET arrives immediately.
const GET_RENDEZVOUS_GRACE: Duration = Duration::from_millis(500);

/// Fence 29 (plan 25): two CONCURRENT session-stream restarts must leave exactly
/// one live reader, and the transport's `close()` must reach it.
///
/// # Not a duplicate of fence 4
///
/// [`repeated_initialized_notifications_leave_exactly_one_live_reader`] asserts
/// a near-identical postcondition and passes today, but it restarts the stream
/// SEQUENTIALLY — three opens in a loop, each completing before the next begins
/// — so what it proves is that the abort-then-respawn ORDERING is correct when
/// there is no overlap. This is the CONCURRENT case: two restarts whose awaits
/// interleave, which fence 4 cannot construct and does not cover. Neither
/// subsumes the other; deleting this one because "fence 4 already checks the
/// reader count" would delete the only coverage of the interleave.
///
/// # The defect
///
/// `start_sse` takes-and-aborts the abort handle, opens a GET ACROSS AN AWAIT,
/// clears the terminal latch and only then stores the new handle — a
/// transport-wide read-modify-write with an await in the middle. Two overlapping
/// calls each take a `None` handle, each open, and the second's store orphans
/// the first's reader. `close()` aborts exactly ONE `JoinHandle`, so the orphan
/// outlives the transport holding a live TCP connection nothing can reach.
///
/// # Why the interleave is by construction
///
/// The harness withholds each GET's response head until a second GET arrives OR
/// [`GET_RENDEZVOUS_GRACE`] elapses, which pins both callers inside the await
/// that the defect lives across. The OR is what keeps the SERIALISED tree from
/// deadlocking on the gate — there the second GET cannot exist until the first
/// restart has returned, so only the grace can open it.
///
/// # Why both assertions settle rather than snap
///
/// On the fixed tree the aborted reader's TCP close is observed ASYNCHRONOUSLY
/// from the server's side, so an instantaneous `assert_eq!` can transiently read
/// two and fail on a loaded machine. `wait_for` turns "eventually one" into a
/// bounded settle and reports the last observed value.
#[tokio::test]
async fn two_concurrent_session_stream_restarts_leave_one_reader() {
    let server = RecordingServer::start().await;
    server.hold_each_get_head_until_two_arrive(GET_RENDEZVOUS_GRACE);

    let transport = transport_for(&server);
    let one = transport.clone();
    let two = transport.clone();

    let first = tokio::spawn(async move { timeout(BOUND, one.start_sse(None)).await });
    let second = tokio::spawn(async move { timeout(BOUND, two.start_sse(None)).await });

    first
        .await
        .expect("the first restart task does not panic")
        .expect("the first start_sse returns within BOUND — a hang here is the fix deadlocking")
        .expect("the harness answers the first GET");
    second
        .await
        .expect("the second restart task does not panic")
        .expect("the second start_sse returns within BOUND — a hang here is the fix deadlocking")
        .expect("the harness answers the second GET");

    assert_eq!(
        server.get_lines(),
        2,
        "both restarts must actually put a GET on the wire, or this fence measures nothing. \
         Observed: {:?}",
        server.request_lines(),
    );
    assert!(
        wait_for(|| server.open_get_connections() == 1).await,
        "exactly ONE session-stream reader may survive two concurrent restarts; {} GET \
         connections were still open. Two overlapping restarts each aborted a handle the other \
         had not yet stored, so one reader is live with NO handle pointing at it — and the \
         transport's close() reaches exactly one handle, which makes that reader unreachable \
         rather than merely redundant",
        server.open_get_connections(),
    );

    let mut closer = transport;
    timeout(BOUND, closer.close())
        .await
        .expect("close returns within BOUND")
        .expect("close succeeds");

    assert!(
        wait_for(|| server.open_get_connections() == 0).await,
        "close() must reach the SURVIVING reader — that is what proves it is reachable rather \
         than merely alone. {} GET connections were still open after close",
        server.open_get_connections(),
    );

    server.shutdown().await;
}

// ===========================================================================
// Fence 30 — plan 23: a peer that accepts a POST and never writes its response
// HEAD must block only ITS OWN call, not every operation on the client.
//
// The defect is the SEND path's twin of the one fence 23 owns on the receive
// path, and no existing fence can see it. `Client::dispatch_request` takes the
// single transport write guard and awaits the send INSIDE it; the send reaches
// `StreamableHttpTransport::post_once`, which awaits the peer's response
// HEADERS; and `grep -n "timeout" src/shared/streamable_http.rs` shows only a
// pool idle timeout — there is no request timeout on any path. So the guard is
// held across a round trip whose duration the PEER chooses, and `send`,
// `cancel_request`, `call_tool` and `Transport::close` all queue behind it.
//
// Fence 26 answers POST heads promptly and holds the answer FRAME, which lets
// `send()` return and measures the RECEIVE path. Neither fence subsumes the
// other: a tree that released the guard around the receive but not around the
// send passes fence 26 and fails this one.
// ===========================================================================

/// Fence 30 (plan 23): a stalled POST HEAD serialises only its own call.
///
/// # Why the RED is deterministic rather than load-sensitive
///
/// For the whole park the harness writes NOTHING — no head, no body, no byte.
/// The two outcomes being distinguished are therefore "the probe completed in
/// milliseconds" and "the probe did not complete within
/// [`LOCK_PROBE_BOUND`]", separated by three orders of magnitude rather than by
/// a race, and [`STALLED_POST_PARK`] is six times the probe bound so a slow
/// machine cannot turn a wedged client into a pass. CI load can only make an
/// already-passing probe slower.
///
/// # Two probe operations, of two different kinds, in order
///
/// A notification send (`cancel_request`, which is
/// `Client::send_notification`'s path) and then a full request/response round
/// trip (`call_tool`, which is `dispatch_request`'s). One alone would leave the
/// pass attributable to that one path happening to be lock-free; both together
/// say the transport is genuinely available while a peer is silent.
#[tokio::test]
async fn a_peer_that_never_writes_response_headers_does_not_serialise_the_client() {
    /// Which `select!` arm finished first. Named rather than a bool so the
    /// failure message can say what actually happened.
    enum Arm {
        /// The stalled `tools/call` returned — this fence is then measuring
        /// nothing, because there is no held POST to contend with.
        Call,
        /// Both probe operations completed while the POST was still parked.
        ///
        /// The call's result is BOXED: a `CallToolResult` is ~280 bytes and the
        /// other variant carries none, which `clippy::large_enum_variant`
        /// rejects. The indirection is invisible to the assertion.
        Probe(
            pmcp::Result<()>,
            Box<pmcp::Result<pmcp::types::CallToolResult>>,
        ),
    }

    let server = RecordingServer::start().await;
    server.advertise_tools();
    server.answer_calls_with_an_echoing_result();

    let (client, _observer) = handshake(&server).await;
    assert!(
        wait_for(|| server.get_lines() >= 1).await,
        "the ordinary session stream must be open, so nothing here can be blamed on a terminal \
         reason. Observed request lines: {:?}",
        server.request_lines()
    );

    // Armed AFTER the handshake, so the `initialize` POST and the `initialized`
    // notification are served exactly as they always are and the stall lands on
    // the call this fence makes.
    server.stall_the_next_answerable_post_before_its_head();

    let started = std::time::Instant::now();
    let outcome = timeout(LOCK_PROBE_BOUND, async {
        tokio::select! {
            _call = client.call_tool("fence-30-stalled".to_string(), json!({})) => Arm::Call,
            probe = async {
                // Wait until the stalled POST has genuinely ARRIVED at the gate.
                // A probe that ran before it would contend for a lock nobody had
                // taken and pass against any tree at all.
                assert!(
                    wait_for(|| server.stalled_posts() >= 1).await,
                    "the stalled call's POST must have reached the harness gate, or this fence \
                     probes an uncontended lock. Observed POST bodies: {:?}",
                    server.post_bodies()
                );
                let notified = client
                    .cancel_request(&RequestId::from("fence-30-probe"))
                    .await;
                let called = client
                    .call_tool("fence-30-probe".to_string(), json!({}))
                    .await;
                Arm::Probe(notified, Box::new(called))
            } => probe,
        }
    })
    .await;

    let Ok(finished) = outcome else {
        server.release_the_stalled_post();
        panic!(
            "a SECOND operation on the same client never completed in {LOCK_PROBE_BOUND:?} while \
             a POST was parked BEFORE its response head. `dispatch_request` takes the transport \
             write guard and awaits the send INSIDE it; that send reaches `post_once`, which \
             awaits the peer's response HEADERS, and no request timeout exists on any path in \
             `src/shared/streamable_http.rs`. So a peer that accepts a POST and never answers \
             holds the one transport guard for as long as it likes, and every other operation on \
             this client — a second call, a notification, a cancellation, `Transport::close` — is \
             serialised behind it with nothing to bound it (T-118.2-23-01). Observed POST bodies \
             at the elapse: {:?}",
            server.post_bodies()
        )
    };

    match finished {
        Arm::Call => {
            server.release_the_stalled_post();
            panic!(
                "the stalled `tools/call` returned after {:?}, before the probe ran. This fence's \
                 subject is the guard being free WHILE a POST is unanswered, so a run in which \
                 that POST was already answered measures nothing. Observed POST bodies: {:?}",
                started.elapsed(),
                server.post_bodies()
            )
        },
        Arm::Probe(notified, called) => {
            notified.expect("the probe's notification must reach the wire and be answered 202");
            (*called)
                .expect("the probe's own call must be answered while the other POST is stalled");
        },
    }

    // Vacuity guards. Both are needed: the stalled call must have been SENT (or
    // there is nothing held), and both probe operations must have reached the
    // WIRE (or "a second operation completed" is a statement about a no-op).
    assert_eq!(
        calls_observed(&server),
        2,
        "both the stalled call and the probe's call must be observable on the wire. Observed \
         POST bodies: {:?}",
        server.post_bodies()
    );
    let cancelled_frames = server
        .post_bodies()
        .iter()
        .filter(|body| {
            body.get("method").and_then(Value::as_str) == Some("notifications/cancelled")
        })
        .count();
    assert_eq!(
        cancelled_frames,
        1,
        "the probe's notification must be observable on the wire. Observed POST bodies: {:?}",
        server.post_bodies()
    );

    // Release the park so the run finishes without waiting out STALLED_POST_PARK.
    server.release_the_stalled_post();
    server.shutdown().await;
}

/// Fence 31 (T-118.2-25-01): a COLD-START fan-out on one transport vends once.
///
/// # What this measures that fence 28 does not
///
/// Fence 28 drives the `401` RECOVERY, which `refresh_lock` single-flights.
/// This one drives the path a process reaches FIRST — several concurrent
/// requests on one cloned transport against an empty credential cache, with no
/// `401` anywhere in the run. That path had no gate at all: the recovery was
/// guarded while the ordinary request build called `get_access_token` directly,
/// so the mitigation existed only on the surface the threat row named and not
/// on the path that reaches the consequence it named.
///
/// # Why the consequence is permanent, not transient
///
/// Against a ROTATING refresh token the identity provider accepts exactly one
/// presentation and rejects the rest, and each rejection invalidates the token
/// the winner just cached. The transport is then holding a dead credential with
/// no way back except an out-of-band re-auth. That is why the assertion is
/// `vends_peak() <= 1` and not merely "the requests succeeded": every request
/// here succeeds on the unfixed tree too, because this harness's provider mints
/// happily. The defect is invisible in the responses and visible only in the
/// concurrency of the vend.
///
/// # The release strategy, and why it is not a sleep
///
/// Every vend parks inside the provider until released. On the UNFIXED tree all
/// `COLD_FANOUT` callers reach the vend, so the count arm fires quickly and the
/// peak is observed while they are all still inside it. On the FIXED tree only
/// one ever gets in, the count arm never fires, and the grace arm releases so
/// the fence does not hang on its own fix. Same two-armed shape fence 28 uses.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cold_start_fanout_on_one_transport_vends_once() {
    /// Enough concurrent first requests that an ungated tree cannot flatten
    /// them by scheduling luck alone.
    const COLD_FANOUT: usize = 4;

    let server = RecordingServer::start().await;

    // COLD, not primed: no token cached, so the very first request build is
    // what vends. No 401 is issued anywhere in this fence.
    let provider = Arc::new(CachingAuthProvider::cold());
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(
            url::Url::parse(&server.url()).expect("the harness URL parses"),
        )
        .with_auth_provider(Arc::clone(&provider) as Arc<dyn AuthProvider>)
        .build(),
    );

    // Clones of ONE transport — the durable-agent shape, and the reason all
    // these callers share one credential cache.
    let senders: Vec<_> = (0..COLD_FANOUT)
        .map(|_| {
            let mut clone = transport.clone();
            // Not `notifications/initialized`, so no send reaches `start_sse`
            // and this fence measures the auth path only.
            tokio::spawn(async move { timeout(BOUND, clone.send(cancelled())).await })
        })
        .collect();

    // Count arm: on an ungated tree every caller piles into the vend.
    // Grace arm: on the gated tree only one does, so release anyway.
    let _ = wait_for_within(VEND_RENDEZVOUS_GRACE, || provider.vends() >= COLD_FANOUT).await;
    provider.release_vends();

    for sender in senders {
        let _ = sender.await.expect("no cold-start sender task panics");
    }

    assert!(
        provider.vends_peak() <= 1,
        "at most ONE caller may be inside get_access_token's VEND path at a time on a COLD \
         cache; {} were. {COLD_FANOUT} concurrent first requests on one cloned transport each \
         presented the rotating refresh token to the IdP, which accepts one and rejects the \
         rest — and each rejection invalidates the token the winner cached, so this transport's \
         auth fails PERMANENTLY before a single 401 has ever been seen. NOTE: fence 28 cannot \
         catch this; it drives the 401 recovery, which refresh_lock already guarded",
        provider.vends_peak(),
    );
    assert_eq!(
        provider.vends(),
        1,
        "exactly ONE vend may reach the identity provider across a cold-start fan-out — every \
         later caller's token must come from the cache the winner warmed. Observed {} vends \
         across {COLD_FANOUT} concurrent first requests",
        provider.vends(),
    );
    assert_eq!(
        provider.purges(),
        0,
        "this fence must not involve a 401 at all: {} purges were observed, which means it \
         drifted onto the recovery path fence 28 already covers and stopped measuring the \
         cold-start path",
        provider.purges(),
    );

    server.shutdown().await;
}
