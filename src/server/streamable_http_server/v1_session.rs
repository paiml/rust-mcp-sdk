//! MCP 2025-11-25 (v1) session and SSE-resumability state — the `v1-compat` half.
//!
//! # What this module is
//!
//! This is the REAL half of a paired module. Its twin is
//! `v1_session_off.rs`, and exactly one of the two is compiled:
//!
//! ```text
//! #[cfg_attr(feature = "v1-compat", path = "streamable_http_server/v1_session.rs")]
//! #[cfg_attr(not(feature = "v1-compat"), path = "streamable_http_server/v1_session_off.rs")]
//! mod v1;
//! ```
//!
//! Every item declared here MUST also be declared, with an identical signature,
//! by the twin — otherwise `cargo build --no-default-features --features full-v2`
//! stops compiling. `tests/v1_severability_tripwire.rs` asserts the inclusion
//! direction that a build cannot: that the twin declares nothing this module
//! does not.
//!
//! # Why a paired module rather than scattered `#[cfg]`
//!
//! The alternative — sprinkling `#[cfg(feature = "v1-compat")]` through the
//! 6,000-line transport file — puts the severance boundary in dozens of places
//! that no reviewer can hold in their head at once. A pair puts the whole
//! boundary in two files: what v1 IS, and what v2 answers INSTEAD.
//!
//! # Operations, never borrows
//!
//! Every entry point below returns an OWNED answer or performs a whole
//! operation. None of them hands out a `&Arc<RwLock<HashMap<…>>>`, and that is a
//! hard rule rather than a style preference: a zero-sized twin has no map to
//! return a reference to, so a borrow-shaped accessor would be
//! *unimplementable* on the `full-v2` build and would force a `#[cfg]` back into
//! the transport. An `Option`-returning accessor is fine wherever the twin can
//! answer `None` honestly; a reference-to-collection accessor never is.
//!
//! # Why these take `&V1State` and the era chokepoints take `&ServerState`
//!
//! The state operations take `&V1State`, so a call site reads
//! `ServerState::v1` on BOTH feature sets. The era chokepoints keep their
//! `&ServerState` signature because it is the SHIPPED one and changing it would
//! invite a second era resolver (Phase 112 D-11 / 113 Pitfall 2).
//!
//! That split is not cosmetic. Give the operations `&ServerState` too and every
//! null twin ignores its `state` argument, so nothing reads `ServerState::v1` on
//! a `full-v2` build and `RUSTFLAGS="-D warnings"` fails the severance build
//! with `field `v1` is never read`. The only ways out are a blanket dead-code
//! `allow` on the seam field — which blunts the exact lint plan 117-05 wired the
//! CI gate around — or this signature. This is the signature.
//!
//! # Scope in this plan
//!
//! Plan 117-06 landed the mechanism on a small payload. Plan 117-09 collapsed
//! the three v1 fields off `ServerState` into [`V1State`] and moved the session
//! and resumability chokepoints here. Plans 117-12 and 117-13 move the
//! SSE-replay and header machinery, at which point several of the fine-grained
//! operations below fold into the function bodies that call them.
//!
//! # Removal, not just gating
//!
//! Gating is reversible and semver-safe; DELETING this pair is a major-version
//! change tracked as SMPL-F1 (pmcp 3.0). The (deliberately date-free) policy
//! that decides when that happens is `docs/v1-sunset-policy.md`.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution already used by `src/server/task_dispatch.rs` and
// `src/shared/http_body_cap.rs`. The twin carries the identical allow.
#![allow(clippy::redundant_pub_crate)]

use super::{create_error_response, EventStore, ServerState, StreamableHttpServerConfig};
use crate::shared::http_constants::{LAST_EVENT_ID, MCP_SESSION_ID};
use crate::shared::TransportMessage;
use crate::types::protocol::{error_codes, Era};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{sse::Event, IntoResponse, Response, Sse};
use axum::Json;
use futures_util::StreamExt;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

/// The event-store handle the transport actually uses for resumability.
///
/// Type-erased so every resumability helper is written against the
/// [`EventStore`](super::EventStore) TRAIT rather than the concrete
/// [`InMemoryEventStore`](super::InMemoryEventStore) that the public
/// `StreamableHttpServerConfig::event_store` field pins. That public field's type
/// is deliberately UNCHANGED — widening it would be a public-field type change,
/// i.e. a MAJOR semver break, which the milestone rules out (D-113-D discipline).
/// The indirection is what lets the crate's own tests substitute a spy and prove
/// zero v2 traffic directly instead of inferring it from a normal-looking 200.
///
/// # Why it lives in the REAL half and not in the transport
///
/// It was declared in `streamable_http_server.rs` until plan 117-13. Once the
/// GET body moved in here, the null twin's three uses of it went away and so did
/// the transport's own, leaving the alias dead on a `full-v2` build under
/// `RUSTFLAGS="-D warnings"`. It is a v1-only concept — resumability is a MCP
/// 2025-11-25 feature the 2026-07-28 transport does not have — so the honest
/// home is the half that only v1 compiles, rather than a `#[cfg]` on the alias
/// or an `allow(dead_code)` over it.
///
/// The twin declares no counterpart, and must not: `Arc<dyn EventStore` is in
/// `tests/v1_severability_tripwire.rs`'s `FORBIDDEN_STATE_TYPES`. The twin
/// declaring FEWER items than this half is the direction that test permits.
pub(crate) type EventStoreHandle = Arc<dyn EventStore>;

/// Type alias for the live v1 SSE stream map.
///
/// Named so the field type below reads at a glance and so the twin can be
/// compared against a short, stable signature rather than a nested generic.
type SseStreamMap = HashMap<String, mpsc::UnboundedSender<TransportMessage>>;

/// What the transport remembers about one MCP 2025-11-25 session.
///
/// All three fields are private to this module: every read and write goes
/// through an operation below, so the twin never has to model a shape it does
/// not hold.
#[derive(Debug, Clone)]
struct SessionInfo {
    initialized: bool,
    protocol_version: Option<String>,
    /// The level this session's `logging/setLevel` last asked for, if it ever
    /// asked (phase 118.2 plan 07, D-11).
    ///
    /// `None` means "this session never called `logging/setLevel`". The D-12
    /// default (`info`) is applied at RESOLUTION time, not stored here, so the
    /// two facts — "asked for nothing" and "asked for info" — stay
    /// distinguishable in the one place that can act on the difference.
    ///
    /// # Why the level lives on the SESSION and not on the server
    ///
    /// `ServerState::server` is one `Arc<tokio::sync::Mutex<Server>>` shared by
    /// every session. A level stored there would let client B's `setLevel`
    /// change client A's filtering — a cross-session information-disclosure
    /// defect (T-118.2-07-01). The per-session home is also the correct LIFETIME:
    /// `logging/setLevel` is retired in MCP 2026-07-28, which carries the level
    /// per request in `_meta` instead, so a v2 build allocates none of this.
    log_level: Option<crate::types::LoggingLevel>,
}

/// All state that exists ONLY for MCP 2025-11-25.
///
/// The three fields are the v1 session lifecycle (`sessions`), the v1 live SSE
/// fan-out those sessions address (`sse_streams`), and v1 SSE resumability
/// (`event_store`). None of the three has a v2 counterpart: the 2026-07-28
/// transport is handshake-free and session-free, and states that resumable SSE
/// via `Last-Event-ID` is not supported.
///
/// On a `full-v2` build this type is the zero-sized twin, so none of these
/// allocations happen — that is the structural half of the SMPL-02 claim, and
/// it is a property of the TYPE rather than of a runtime branch anyone could
/// forget to take.
#[derive(Clone, Default)]
pub(crate) struct V1State {
    /// Active v1 SSE streams, keyed by session id.
    sse_streams: Arc<RwLock<SseStreamMap>>,
    /// v1 session tracking: session id -> session info.
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    /// The v1 resumability event store, type-erased from `config.event_store`.
    ///
    /// Always derived from the config in production ([`V1State::new`] is the
    /// only constructor). It lives here rather than being read straight off the
    /// config so every resumability helper can be written against the
    /// [`EventStore`](super::EventStore) trait — see [`EventStoreHandle`] for
    /// why the public config field's concrete type must not change. Reach it
    /// ONLY through `resumability_store`, never directly.
    pub(crate) event_store: Option<EventStoreHandle>,
}

/// Hand-written because `EventStoreHandle` is `Arc<dyn EventStore>` and that
/// trait carries no `Debug` bound, so `#[derive(Debug)]` cannot be used here.
///
/// It is written anyway, and that is NOT cosmetic. The null twin derives
/// `Debug`; a `V1State` that implemented it on only one half of the pair would
/// let `#[derive(Debug)]` on `ServerState` — or any `tracing` field capture of
/// this field — compile on `full-v2` and FAIL on the DEFAULT build, i.e. break
/// in the configuration every consumer actually ships. The twin must declare
/// nothing this module does not, trait impls included.
///
/// Takes NO lock: a `Debug` impl that acquired the session or stream lock could
/// deadlock inside a panic formatter or a log line emitted while the lock is
/// already held. Cardinality is deliberately not reported for that reason.
impl std::fmt::Debug for V1State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V1State")
            .field("event_store", &self.event_store.is_some())
            .finish_non_exhaustive()
    }
}

impl V1State {
    /// Build the v1 state a server starts with, from its configuration.
    ///
    /// Type-erases the configured store ONCE, here, so every resumability helper
    /// is written against the [`EventStore`](super::EventStore) trait and never
    /// touches the concrete `InMemoryEventStore` the public config field pins.
    ///
    /// This is called from `make_server_state`, the transport's single
    /// `ServerState` construction site, with no `#[cfg]` around it — the twin
    /// takes the same argument and allocates nothing.
    pub(crate) fn new(config: &StreamableHttpServerConfig) -> Self {
        Self {
            sse_streams: Arc::new(RwLock::new(SseStreamMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_store: config
                .event_store
                .clone()
                .map(|store| store as EventStoreHandle),
        }
    }
}

// ---------------------------------------------------------------------------
// v1 session-map operations.
// ---------------------------------------------------------------------------

/// Is a v1 session with this id being tracked?
fn session_exists(state: &V1State, session_id: &str) -> bool {
    state.sessions.read().contains_key(session_id)
}

/// Start tracking a v1 session.
///
/// PRIVATE to this module since plan 117-12: both of its callers
/// ([`process_init_session`] and [`resolve_sse_session`]) now live on this side
/// of the pair, so it is no longer a seam the transport crosses and the twin
/// does not declare it. Kept as a helper rather than inlined twice because it
/// has two call sites and it is the only place [`SessionInfo`] is constructed.
fn insert_session(
    state: &V1State,
    session_id: String,
    initialized: bool,
    protocol_version: Option<String>,
) {
    state.sessions.write().insert(
        session_id,
        SessionInfo {
            initialized,
            protocol_version,
            // A freshly tracked session has asked for no level; `info` applies
            // by default until it does (D-12).
            log_level: None,
        },
    );
}

/// The protocol version recorded against a session, if it has one.
///
/// Deliberately collapses "no such session" and "session with no recorded
/// version" into the same `None`. Both callers already treated the two cases
/// identically — one falls back to `DEFAULT_PROTOCOL_VERSION`, the other skips
/// its comparison — so the collapse is behaviour-preserving and removes a
/// distinction the twin could not represent.
pub(crate) fn session_protocol_version(state: &V1State, session_id: &str) -> Option<String> {
    state
        .sessions
        .read()
        .get(session_id)
        .and_then(|info| info.protocol_version.clone())
}

/// The log level a session's `logging/setLevel` last asked for, if it asked.
///
/// Deliberately collapses "no such session" and "session with no recorded
/// level" into the same `None`, for exactly the reason
/// [`session_protocol_version`] does: both cases mean "nothing overrides the
/// default", every caller treats them identically, and the collapse removes a
/// distinction the zero-sized twin could not represent. The D-12 default
/// (`info`) is applied by the RESOLVER at the HTTP ingress, not here.
///
/// Returns an OWNED answer — [`crate::types::LoggingLevel`] is `Copy` — so the
/// twin can produce one without a map to borrow out of.
///
/// Its ONE caller is `super::resolve_request_log_level`, the named rule at the
/// HTTP ingress.
pub(crate) fn session_log_level(
    state: &V1State,
    session_id: &str,
) -> Option<crate::types::LoggingLevel> {
    state
        .sessions
        .read()
        .get(session_id)
        .and_then(|info| info.log_level)
}

/// Record the level a `logging/setLevel` request asked for, against ITS session.
///
/// THE single write path for [`SessionInfo::log_level`].
///
/// # A write for an unknown session id is a NO-OP, not an insert
///
/// That is a denial-of-service control (T-118.2-07-02), not a style choice.
/// Minting a session row from a `logging/setLevel` would let a caller grow the
/// session map without limit by guessing — or simply inventing — session ids,
/// on a request that the session pipeline has not authorised. `get_mut` rather
/// than `entry(..).or_insert(..)` is what makes that structural: there is no
/// insertion path in this function to reach.
///
/// In production the transport reaches this only with the session id that
/// `validate_non_init_session` already accepted, so the no-op arm is defence in
/// depth rather than the common case — but it is the arm that stays correct if a
/// future call site forgets the validation.
///
/// Its ONE caller is `super::capture_v1_set_level`, at the HTTP ingress.
pub(crate) fn set_session_log_level(
    state: &V1State,
    session_id: &str,
    level: crate::types::LoggingLevel,
) {
    if let Some(info) = state.sessions.write().get_mut(session_id) {
        info.log_level = Some(level);
    }
}

/// Stop tracking a v1 session.
fn remove_session(state: &V1State, session_id: &str) {
    state.sessions.write().remove(session_id);
}

// ---------------------------------------------------------------------------
// v1 SSE stream operations.
// ---------------------------------------------------------------------------

/// Is an SSE stream already open for this session?
fn sse_stream_exists(state: &V1State, session_id: &str) -> bool {
    state.sse_streams.read().contains_key(session_id)
}

/// Register the sending half of a newly opened v1 SSE stream.
fn register_sse_stream(
    state: &V1State,
    session_id: String,
    sender: mpsc::UnboundedSender<TransportMessage>,
) {
    state.sse_streams.write().insert(session_id, sender);
}

/// Close the SSE stream for a session, if one is open.
fn remove_sse_stream(state: &V1State, session_id: &str) {
    state.sse_streams.write().remove(session_id);
}

/// Try to hand a response to a session's live SSE stream.
///
/// Returns `None` when the message went into a live stream — the caller then
/// answers `202 Accepted` — and `Some(message)` when there was no stream to
/// take it, giving ownership back so the caller can frame it as a one-shot SSE
/// response instead.
///
/// This is the SSE-stream read that outlives every later plan in this phase
/// (`build_response` is in no plan's move list), so its shape is load-bearing.
/// A `&Arc<RwLock<SseStreamMap>>` accessor could not be implemented by the
/// zero-sized twin; moving ownership of `message` in and, on the
/// not-delivered path, back out keeps the whole lock scope on this side of the
/// seam.
pub(crate) fn route_to_session_stream(
    state: &V1State,
    session_id: &str,
    message: TransportMessage,
) -> Option<TransportMessage> {
    let streams = state.sse_streams.read();
    let Some(sender) = streams.get(session_id) else {
        return Some(message);
    };
    // Best-effort, exactly as before the move: a receiver that has gone away
    // still yields `202 Accepted` rather than a fallback body.
    let _ = sender.send(message);
    None
}

// ---------------------------------------------------------------------------
// Session era gate (Plan 113-04, HTTP-01; MOVED here by plan 117-09).
//
// `stateless()` is a BUILD-TIME config: it clears `session_id_generator` once,
// when the server is constructed. A dual-version server is built with
// `Default::default()`, which keeps a live generator — so every session decision
// that keys off the CONFIG would mint, demand and echo session ids for v2
// requests too (RESEARCH Pitfall 1). HTTP-01 requires the opposite: on v2 there
// is no handshake and no session at all.
//
// The fix is one predicate, not a transport fork. Every session decision routes
// through `sessions_active`, which makes the ERA the decider and leaves the v1
// path byte-for-byte unchanged.
// ---------------------------------------------------------------------------

/// The pure session-era rule: are sessions live for THIS request?
///
/// | `cfg_has_generator` | `era`            | result | why |
/// |---------------------|------------------|--------|-----|
/// | `true`              | `Some(Era::V2)`  | `false`| v2 is handshake-free and session-free (HTTP-01) |
/// | `true`              | `Some(Era::V1)`  | `true` | v1 session behavior is untouched |
/// | `true`              | `None`           | `true` | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `false`             | anything         | `false`| an explicitly `stateless()` server stays stateless |
///
/// Split out from [`sessions_active`] so the RULE is unit- and property-testable
/// without constructing a live [`ServerState`].
pub(crate) const fn sessions_active_for(cfg_has_generator: bool, era: Option<Era>) -> bool {
    !matches!(era, Some(Era::V2)) && cfg_has_generator
}

/// Are sessions live for this request? THE single reader of
/// `config.session_id_generator`'s presence.
///
/// `era` is the ALREADY-RESOLVED [`ProtocolContext::era`](crate::types::protocol::ProtocolContext)
/// being CONSUMED here — this layer never runs a second era resolver (Pitfall 2 /
/// D-11). The POST entrypoints resolve it once via the v2 header gate and thread
/// that same value into every session decision below.
///
/// `None` means the server is NOT opted into v2, so no era detection ran at all
/// and the v1 path executes with zero era code (D-04).
pub(crate) fn sessions_active(state: &ServerState, era: Option<Era>) -> bool {
    sessions_active_for(state.config.session_id_generator.is_some(), era)
}

/// The session-id generator to use for THIS request, or `None` when sessions are
/// not active for it.
///
/// The second (and last) permitted reader of `config.session_id_generator`: it
/// gates the borrow behind [`sessions_active`] so no caller can reach the
/// generator on a request whose era suppresses sessions.
///
/// PRIVATE to this module since plan 117-12, for the same reason as
/// [`insert_session`]: both of its callers ([`process_init_session`] and
/// [`resolve_sse_session`]) moved onto this side of the pair, so it is no longer
/// a seam the transport crosses. The twin does not declare it — a mirrored
/// declaration nothing can call would be dead code on the `full-v2` build, and
/// the only ways to keep it alive there are contrivances (a `debug_assert!`
/// whose real job is to defeat the lint, or a branch with two identical arms).
/// Narrowing the visibility is the honest form of the same fact.
fn active_session_generator(
    state: &ServerState,
    era: Option<Era>,
) -> Option<&(dyn Fn() -> String + Send + Sync)> {
    if !sessions_active(state, era) {
        return None;
    }
    state.config.session_id_generator.as_deref()
}

/// The ONE place a `Mcp-Session-Id` response header is emitted.
///
/// `response_session_id` is already `None` for a v2 request (both session
/// resolvers return `None` when [`sessions_active`] is false), so this is
/// defense in depth: even a future caller that manufactured a session id could
/// not leak it onto a v2 response. Non-panicking — an unrepresentable id is
/// skipped rather than unwrapped (T-112-13 discipline).
pub(crate) fn apply_session_header(
    headers: &mut HeaderMap,
    response_session_id: Option<&String>,
    sessions_on: bool,
) {
    if !sessions_on {
        return;
    }
    let Some(sid) = response_session_id else {
        return;
    };
    if let Ok(value) = HeaderValue::from_str(sid) {
        headers.insert(MCP_SESSION_ID, value);
    }
}

// ---------------------------------------------------------------------------
// Resumability era gate (Plan 113-08, HTTP-05; MOVED here by plan 117-09).
//
// The 2026-07-28 transport spec is verbatim: "Resumable SSE streams via
// `Last-Event-ID` are not supported", and a `Last-Event-ID` header "ignore it".
// The official conformance suite has already retired its `sse-polling` scenario
// for this revision.
//
// The gate mirrors [`sessions_active`] exactly: ONE predicate, consuming the
// ALREADY-RESOLVED era, routing every read / replay / store decision. It is
// deliberately INDEPENDENT of the session gate. Before plan 113-08 a v2 request
// happened not to reach the event store, but only INCIDENTALLY — the store write
// is conditioned on a `response_session_id`, which the session gate already
// zeroes on v2. An incidental guarantee is not a guarantee: the SSE-stream
// routing bug that plan fixed is exactly what happens when one of those two
// couplings is broken and the other is assumed to cover it.
//
// `EventStoreHandle` is declared at the top of THIS file (plan 117-13 moved it
// here once the GET body arrived and the transport's own uses went away). The
// twin deliberately declares no counterpart and must not: `Arc<dyn EventStore`
// is in `tests/v1_severability_tripwire.rs`'s `FORBIDDEN_STATE_TYPES`.
// ---------------------------------------------------------------------------

/// The pure resumability rule: is event replay/retention live for THIS request?
///
/// | `cfg_has_event_store` | `era`           | result | why |
/// |-----------------------|-----------------|--------|-----|
/// | `true`                | `Some(Era::V2)` | `false`| v2 does not offer resumability at all (HTTP-05) |
/// | `true`                | `Some(Era::V1)` | `true` | v1 resumability is untouched |
/// | `true`                | `None`          | `true` | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `false`               | anything        | `false`| no store configured, nothing to read or write |
///
/// Split out from [`resumability_active`] so the RULE is unit- and
/// property-testable without constructing a live [`ServerState`].
pub(crate) const fn resumability_active_for(cfg_has_event_store: bool, era: Option<Era>) -> bool {
    // The RULE is shared with `sessions_active_for` — both facilities are
    // "v1-only, and only when configured". Sharing the pure predicate does NOT
    // couple the two GATES (the point of keeping them independent): each still
    // reads its own config field, `event_store` here and `session_id_generator`
    // there. What it removes is a second copy of the era rule that had to be
    // edited in lockstep, along with a cloned truth table and a cloned proptest.
    sessions_active_for(cfg_has_event_store, era)
}

/// Is resumability live for this request? THE single reader of the event
/// store's presence.
///
/// `era` is the ALREADY-RESOLVED [`ProtocolContext::era`](crate::types::protocol::ProtocolContext)
/// being CONSUMED here — this layer never runs a second era resolver (Pitfall 2 /
/// D-11), exactly as [`sessions_active`] does not.
///
/// `None` means the server is NOT opted into v2, so no era detection ran at all
/// and the v1 path executes with zero era code (D-04).
pub(crate) fn resumability_active(state: &ServerState, era: Option<Era>) -> bool {
    resumability_active_for(state.v1.event_store.is_some(), era)
}

/// The event store to use for THIS request, or `None` when its era suppresses
/// resumability.
///
/// The second (and last) permitted reader of the v1 event store: it gates
/// the borrow behind [`resumability_active`], so no caller can reach the store —
/// to REPLAY from it or to WRITE to it — on a v2 request. Storing without
/// replaying would be dead retention of response envelopes, which is precisely
/// the material an id-replay bug feeds on (T-113-30).
pub(crate) fn resumability_store(
    state: &ServerState,
    era: Option<Era>,
) -> Option<&EventStoreHandle> {
    if !resumability_active(state, era) {
        return None;
    }
    state.v1.event_store.as_ref()
}

// ---------------------------------------------------------------------------
// v1 session LIFECYCLE (MOVED here by plan 117-12, SMPL-02).
//
// These are the pipeline stages that only MCP 2025-11-25 has: minting a session
// on `initialize`, requiring and validating one on every later request, reading
// the negotiated version out of the `initialize` RESULT, and recording it. The
// 2026-07-28 transport is handshake-free and session-free, so on a `full-v2`
// build the twin answers each of them with a constant and none of this code is
// compiled at all.
//
// Every signature below is the SHIPPED one, unchanged. In particular
// `session_id: Option<String>` keeps threading through the POST pipeline on both
// builds — it is simply always `None` on `full-v2`. That is deliberate: dropping
// the parameter would mean surgery on ~10 pipeline functions and would tempt a
// call site into making a second era decision of its own (Phase 112 D-11 /
// Phase 113 Pitfall 2).
// ---------------------------------------------------------------------------

/// Process session for initialization request.
///
/// `era` is the resolved per-request era (see [`sessions_active`]). A v2 request
/// never reaches `initialize` — v2 has no handshake — but the site is defensive:
/// with sessions inactive it mints nothing.
pub(crate) fn process_init_session(
    state: &ServerState,
    era: Option<Era>,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    if let Some(generator) = active_session_generator(state, era) {
        // Stateful mode
        if let Some(sid) = session_id {
            // Check if session already exists and is initialized
            // Inlined from plan 117-09's `session_is_initialized` seam: that
            // operation existed only so the transport could ask this question
            // across the pair boundary, and its one caller is now on this side
            // of it. `false` for an unknown id is what the re-initialization
            // guard wants — an unknown session cannot have been initialized, so
            // the guard falls through rather than rejecting. The answer is bound
            // first so the read lock is released before the error is built.
            let already_initialized = state
                .v1
                .sessions
                .read()
                .get(&sid)
                .is_some_and(|info| info.initialized);
            if already_initialized {
                // Session already initialized - reject re-initialization
                return Err(create_error_response(
                    StatusCode::BAD_REQUEST,
                    error_codes::INVALID_REQUEST,
                    "Session already initialized",
                ));
            }
            // Use existing session ID
            Ok((Some(sid), false))
        } else {
            // Generate new session ID
            let new_id = generator();
            // Create new session entry
            insert_session(&state.v1, new_id.clone(), false, protocol_version);
            if let Some(callback) = &state.config.on_session_initialized {
                callback(&new_id);
            }
            Ok((Some(new_id), true))
        }
    } else {
        // Sessions inactive (stateless config, or a v2 request) — mint nothing.
        Ok((None, false))
    }
}

/// Validate session for non-initialization request.
///
/// When sessions are inactive for this request — a `stateless()` server, or ANY
/// v2 request regardless of config — nothing is required and nothing is
/// validated. An inbound `Mcp-Session-Id` on a v2 request is IGNORED rather than
/// rejected, per the transport spec: "An `Mcp-Session-Id` header on a request:
/// ignore it, and do not mint or echo session IDs."
pub(crate) fn validate_non_init_session(
    state: &ServerState,
    era: Option<Era>,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if sessions_active(state, era) {
        // Stateful mode - require and validate session ID
        match session_id {
            None => {
                // Missing session ID
                Err(create_error_response(
                    StatusCode::BAD_REQUEST,
                    error_codes::INVALID_REQUEST,
                    "Session ID required for non-initialization requests",
                ))
            },
            Some(sid) => {
                // Validate session exists
                if session_exists(&state.v1, &sid) {
                    Ok(Some(sid))
                } else {
                    // Unknown session ID
                    Err(create_error_response(
                        StatusCode::NOT_FOUND,
                        error_codes::INVALID_REQUEST,
                        "Unknown session ID",
                    ))
                }
            },
        }
    } else {
        // Sessions inactive (stateless config, or a v2 request) — any inbound
        // `Mcp-Session-Id` is ignored, and none is echoed back.
        Ok(None)
    }
}

/// Update session info after initialization
pub(crate) fn update_session_after_init(
    state: &ServerState,
    session_id: Option<&String>,
    negotiated_version: Option<String>,
) {
    let Some(sid) = session_id else {
        return;
    };
    // Inlined from plan 117-09's `mark_session_initialized` seam, for the same
    // reason as `session_is_initialized` above. A session that negotiated
    // nothing explicit is recorded at `DEFAULT_PROTOCOL_VERSION`, unchanged from
    // before the move; an unknown id is a no-op.
    if let Some(info) = state.v1.sessions.write().get_mut(sid.as_str()) {
        info.initialized = true;
        info.protocol_version =
            negotiated_version.or_else(|| Some(crate::DEFAULT_PROTOCOL_VERSION.to_string()));
    }
}

/// In stateful mode, verify that a provided protocol version matches the
/// session's recorded negotiated version (if any). Pure early-return chain.
///
/// Short-circuits `Ok(())` whenever sessions are inactive for this request. On v2
/// that is not merely an optimization: there IS no session, and the PER-REQUEST
/// version is authoritative over any session state (the Phase-112 lock), so a
/// session-recorded version must never be consulted. The null twin returns that
/// same `Ok(())` unconditionally, which turns the early return below into a
/// compile-time fact on `full-v2` rather than a runtime one.
pub(crate) fn validate_protocol_version_matches_session(
    state: &ServerState,
    era: Option<Era>,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if !sessions_active(state, era) {
        return Ok(());
    }
    let Some(sid) = session_id else {
        return Ok(());
    };
    // Ordered so the CHEAP check runs first: `MCP-Protocol-Version` is an
    // optional header, and with nothing to compare against there is no point
    // taking the session read-lock and cloning the recorded version out of it
    // (`session_protocol_version` returns an owned `String` so the ZST twin can
    // implement it). Every request that omits the header now skips both.
    let Some(provided_version) = protocol_version else {
        return Ok(());
    };
    let Some(negotiated_version) = session_protocol_version(&state.v1, sid.as_str()) else {
        return Ok(());
    };
    if *provided_version == negotiated_version {
        return Ok(());
    }
    Err(create_error_response(
        StatusCode::BAD_REQUEST,
        error_codes::INVALID_REQUEST,
        &format!(
            "Protocol version mismatch: expected {}, got {}",
            negotiated_version, provided_version
        ),
    ))
}

/// Resolve the response session ID given the request type and incoming headers.
///
/// For initialize requests this delegates to [`process_init_session`]; for
/// subsequent requests to [`validate_non_init_session`]. Used by both POST
/// handlers.
pub(crate) fn resolve_session_for_request(
    state: &ServerState,
    era: Option<Era>,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if is_init_request {
        let (sid, _is_new) = process_init_session(state, era, session_id, protocol_version)?;
        Ok(sid)
    } else {
        validate_non_init_session(state, era, session_id)
    }
}

// ---------------------------------------------------------------------------
// v1 SSE RESUMABILITY and the GET-stream helpers (MOVED here by plan 117-12,
// SMPL-02).
//
// The 2026-07-28 transport spec is verbatim that "Resumable SSE streams via
// `Last-Event-ID` are not supported" and that a `Last-Event-ID` header on a
// request should be ignored. Before this move that was a runtime early return;
// now it is structural — the functions below are not compiled into a `full-v2`
// build at all, and with them goes the ONLY reader of `LAST_EVENT_ID` in the
// server. See the twin's `replay_sse_events_from_header` for why "not even
// parsed" is the property that matters (threats T-113-29 / T-113-30).
// ---------------------------------------------------------------------------

/// Persist the response event if resumability is live for THIS request.
///
/// Shared by both POST handlers — same condition (init OR non-init request
/// with a response session ID), same store-event call, same fire-and-forget
/// error handling.
///
/// The store is reached through [`resumability_store`], so a v2 request writes
/// NOTHING (HTTP-05 / T-113-30) independently of whether it happens to have a
/// response session id. Retaining v2 response envelopes that can never be
/// replayed is dead retention of exactly the material an id-replay bug feeds on.
pub(crate) async fn store_response_event(
    state: &ServerState,
    era: Option<Era>,
    response_session_id: Option<&String>,
    response_msg: &TransportMessage,
) {
    if let Some(event_store) = resumability_store(state, era) {
        if let Some(sid) = response_session_id {
            let event_id = Uuid::new_v4().to_string();
            let _ = event_store.store_event(sid, &event_id, response_msg).await;
        }
    }
}

/// Resolve the SSE session ID: validate an incoming one or mint a new one.
///
/// Returns `Ok(session_id)` on success, or an error response (404 unknown
/// session, 405 stateless-mode).
/// A GET carries no body and therefore no `_meta`, so the ONLY era signal is the
/// `MCP-Protocol-Version` header — and a v2 GET is already answered `405` by
/// [`handle_get_sse`](super::handle_get_sse) before this runs. Sessions are
/// therefore evaluated at `era = None`, which [`sessions_active`] resolves to
/// exactly the pre-113 config-only behavior for the v1 / non-opted-in traffic
/// that can reach here.
fn resolve_sse_session(
    state: &ServerState,
    incoming_session_id: Option<String>,
) -> std::result::Result<String, Response> {
    let sessions_on = sessions_active(state, None);
    if let Some(sid) = incoming_session_id {
        if sessions_on && !session_exists(&state.v1, &sid) {
            return Err(create_error_response(
                StatusCode::NOT_FOUND,
                error_codes::INVALID_REQUEST,
                "Unknown session ID",
            ));
        }
        return Ok(sid);
    }
    let Some(generator) = active_session_generator(state, None) else {
        return Err(create_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            error_codes::METHOD_NOT_FOUND,
            "SSE not supported in stateless mode",
        ));
    };
    let new_id = generator();
    // `true`: a GET SSE implicitly initializes the session it mints.
    insert_session(&state.v1, new_id.clone(), true, None);
    if let Some(callback) = &state.config.on_session_initialized {
        callback(&new_id);
    }
    Ok(new_id)
}

/// Replay events from the event store after a `Last-Event-ID` header value
/// into an SSE sender channel. Fire-and-forget on any intermediate failure.
///
/// `event_store` comes from [`resumability_store`], so on a v2 request it is
/// `None` and this function returns before it ever LOOKS at `Last-Event-ID` —
/// the spec's "ignore it" taken literally, at the only site in the transport that
/// reads that header (T-113-29). On a `full-v2` build the guarantee is stronger
/// still: this function does not exist, and the twin that replaces it names no
/// header at all.
async fn replay_sse_events_from_header(
    headers: &HeaderMap,
    tx: &mpsc::UnboundedSender<TransportMessage>,
    event_store: Option<&EventStoreHandle>,
) {
    // Deliberately FIRST: an era that suppresses resumability must not even parse
    // an attacker-supplied replay cursor.
    let Some(store) = event_store else {
        return;
    };
    let Some(last_event_id) = headers.get(LAST_EVENT_ID) else {
        return;
    };
    let Ok(last_id) = last_event_id.to_str() else {
        return;
    };
    if let Ok(events) = store.replay_events_after(last_id).await {
        for (_event_id, msg) in events {
            // A REPLAYED HISTORICAL EVENT is not a direct response: it keeps its
            // ORIGINAL id, which is correct and is asserted as such by
            // `v1_replayed_event_retains_original_id`. See the direct-response
            // audit block in `streamable_http_server.rs`, above
            // `envelope_for_live_request`.
            let _ = tx.send(msg);
        }
    }
}

/// Map a `TransportMessage` to an SSE `Event`, spawning a best-effort event
/// store write in parallel.
///
/// `event_store` comes from [`resumability_store`], so a v2 stream writes nothing.
///
/// # The payload is [`serialize_message`], NOT `serde_json::to_string`
///
/// [`TransportMessage`] is `#[serde(untagged)]`, so serializing it directly emits
/// the STRUCT, not a JSON-RPC frame. `serialize_message`'s own rustdoc records
/// this and it is not a style preference — the two encodings differ for two of
/// the three variants:
///
/// | variant | raw `serde_json` | [`serialize_message`] |
/// | ------- | ---------------- | --------------------- |
/// | `Response` | `{"jsonrpc":"2.0","id":…,"result":…}` | identical |
/// | `Request` | `{"id":…,"request":{"method":…}}` — unparseable | `{"jsonrpc":"2.0","id":…,"method":…,"params":…}` |
/// | `Notification` | the params object ALONE, with no `method` and no `jsonrpc` | `{"jsonrpc":"2.0","method":…,"params":…}` |
///
/// Until phase 118.1 only `Response` ever reached a v1 SSE stream, where the two
/// agree byte for byte — which is why the defect stayed latent. Plan 10 gave the
/// transport a server-to-client REQUEST path and plan 11 a progress-NOTIFICATION
/// path, and both are unparseable under the raw encoding: a client receiving
/// `{"id":…,"request":…}` cannot dispatch it, and `parse_message` rejects it with
/// "Unknown message type". Routing through the shared encoder means this stream
/// speaks the same wire language as stdio, WebSocket and the WASM fetch
/// transport, from ONE definition.
///
/// A serialization failure yields an EMPTY data payload rather than a panic. It
/// is unreachable in practice — the only serde failure modes for these types are
/// non-finite floats, and `ServerProgressReporter::validate_values` rejects those
/// before a notification is ever constructed — but a formatter on the response
/// path must not be able to abort the stream.
fn sse_event_for_message(
    msg: &TransportMessage,
    session_id: &str,
    event_store: Option<&EventStoreHandle>,
) -> Event {
    let event_id = Uuid::new_v4().to_string();
    if let Some(store) = event_store {
        let sid = session_id.to_string();
        let msg_clone = msg.clone();
        let store = store.clone();
        let event_id_clone = event_id.clone();
        tokio::spawn(async move {
            let _ = store.store_event(&sid, &event_id_clone, &msg_clone).await;
        });
    }
    let data = crate::shared::transport::serialize_message(msg)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    Event::default().id(event_id).event("message").data(data)
}

/// Read the inbound `Mcp-Session-Id` request header.
///
/// The v1 half of `super::extract_session_and_protocol_headers`, which is MIXED:
/// it also reads `MCP-Protocol-Version`, which 2026-07-28 REQUIRES (VERS-05), so
/// the function itself cannot move here. Only this read can, and it does — the
/// twin answers `None` without naming a header, so a `full-v2` build resolves no
/// inbound session id through the POST PIPELINE at all.
///
/// # One remaining ungated reader, named rather than hidden
///
/// `super::build_middleware_context` also reads `Mcp-Session-Id` — off the
/// middleware-adapted request, into `ServerHttpContext::session_id` — and it is
/// NOT gated, so on a `full-v2` build a POST that goes through the middleware
/// path still surfaces an inbound session id to user middleware. That is the same
/// measured exception `crate::shared::http_constants::MCP_SESSION_ID`'s own doc
/// records for why the CONSTANT stays ungated: `http_middleware` is a SHARED,
/// era-neutral config field, so the context it builds cannot move into this pair.
/// Nothing in the transport's own session pipeline consumes that value on a
/// severed build; it is observability handed to middleware, not state.
///
/// Returns the raw header value with no validation beyond UTF-8; every session
/// DECISION is made by `process_init_session` / `validate_non_init_session`
/// below, which is where it belongs.
pub(crate) fn incoming_session_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(std::string::ToString::to_string)
}

/// Attach SSE-specific hardening headers (session, cache-control, connection)
/// to the given axum response.
fn attach_sse_response_headers(response: &mut Response, session_id: &str) {
    response
        .headers_mut()
        .insert(MCP_SESSION_ID, session_id.parse().unwrap());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
}

// ---------------------------------------------------------------------------
// The v1 HTTP verb bodies.
//
// `handle_get_sse` and `handle_delete_session` are SPLIT across the pair, not
// moved into it: their `v2_verb_rejection` heads stay in the transport, always
// compiled, and only these bodies are gated. Plan 117-13.
// ---------------------------------------------------------------------------

/// Everything a `GET /` does once the v2 405 rejection has declined to fire.
///
/// Verbatim the post-rejection half of the shipped `handle_get_sse`, including
/// the `validate_headers` call: on a `v1-compat` build the wire answer for every
/// GET is byte-identical to what it was before the split, which
/// `tests/v1_byte_identity_after_cut.rs` pins.
///
/// It takes `&ServerState` and `&HeaderMap` rather than axum extractors because
/// the extractors belong to the always-compiled head; the pair only ever sees
/// already-extracted values, so the twin never has to model an axum handler
/// signature.
pub(crate) async fn handle_get_sse_body(state: &ServerState, headers: &HeaderMap) -> Response {
    if let Err(error_response) = super::validate_headers(headers, "GET") {
        return error_response;
    }

    let incoming_session_id = incoming_session_header(headers);

    let session_id = match resolve_sse_session(state, incoming_session_id) {
        Ok(sid) => sid,
        Err(response) => return response,
    };

    if sse_stream_exists(&state.v1, &session_id) {
        return create_error_response(
            StatusCode::CONFLICT,
            error_codes::INVALID_REQUEST,
            "SSE stream already exists for this session",
        );
    }

    let (tx, rx) = mpsc::unbounded_channel();
    register_sse_stream(&state.v1, session_id.clone(), tx.clone());

    // A GET carries no body and therefore no `_meta`, so `era = None` — and a v2
    // GET is already answered `405` by the head in the transport, so only v1 /
    // non-opted-in traffic reaches here. `resumability_store(state, None)` is
    // therefore exactly the pre-113 config-only read, the same reasoning
    // [`resolve_sse_session`] records for its `sessions_active(state, None)`.
    let resumability = resumability_store(state, None).cloned();

    replay_sse_events_from_header(headers, &tx, resumability.as_ref()).await;

    let stream = UnboundedReceiverStream::new(rx);
    let session_id_for_stream = session_id.clone();
    let event_store = resumability;

    let sse = Sse::new(stream.map(move |msg| {
        Ok::<_, Infallible>(sse_event_for_message(
            &msg,
            &session_id_for_stream,
            event_store.as_ref(),
        ))
    }));

    let mut response = sse.into_response();
    attach_sse_response_headers(&mut response, &session_id);
    response
}

/// Everything a `DELETE /` does once the v2 405 rejection has declined to fire.
///
/// Verbatim the post-rejection half of the shipped `handle_delete_session`.
///
/// Not `async`: the shipped body never awaited, and making it `async` purely to
/// match the GET twin would add a future to a synchronous teardown for symmetry
/// alone. The pair only requires that the two HALVES of *this* function agree,
/// which they do.
///
/// This is also the only remaining reader of
/// [`StreamableHttpServerConfig::on_session_closed`](super::StreamableHttpServerConfig)
/// outside the config's own construction sites. Because it lives here, plan
/// 117-13's gating of that field needed no `#[cfg]` at a call site.
pub(crate) fn handle_delete_body(state: &ServerState, headers: &HeaderMap) -> Response {
    // Extract session ID
    let session_id = incoming_session_header(headers);

    if let Some(sid) = session_id {
        // Check if session exists
        let exists = session_exists(&state.v1, &sid);

        // A DELETE carries no body, so `era = None` — and a v2 DELETE is already
        // answered `405` by the head in the transport, so only v1 / non-opted-in
        // traffic reaches here. `sessions_active(state, None)` is exactly the
        // pre-113 config-only read.
        if !exists && sessions_active(state, None) {
            // Unknown session in stateful mode
            return create_error_response(
                StatusCode::NOT_FOUND,
                error_codes::INVALID_REQUEST,
                "Unknown session ID",
            );
        }

        // Remove SSE stream if exists
        remove_sse_stream(&state.v1, &sid);

        // Remove session from tracking
        remove_session(&state.v1, &sid);

        // Notify callback
        if let Some(callback) = &state.config.on_session_closed {
            callback(&sid);
        }

        (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
    } else {
        // No session to delete
        create_error_response(
            StatusCode::NOT_FOUND,
            error_codes::INVALID_REQUEST,
            "No session ID provided",
        )
    }
}
