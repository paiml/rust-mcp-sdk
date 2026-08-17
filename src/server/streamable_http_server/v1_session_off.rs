//! The v1 null twin — what a `full-v2` build answers INSTEAD of MCP 2025-11-25.
//!
//! # Read this file to know what severance means
//!
//! This is the SMPL-02 deliverable. When the `v1-compat` feature is off, the MCP
//! 2025-11-25 session lifecycle and SSE resumability **do not exist** in the
//! compiled crate. Not "are skipped at runtime", not "return early" — the code
//! is not there. Every item below is the v2 constant answer to a question only
//! v1 ever asked.
//!
//! There is no session map here, no event store here, and nothing here reads the
//! `Last-Event-ID` request header. That is verifiable *by inspection*: the file
//! is short enough to read end to end, and what it does not contain is as
//! load-bearing as what it does. `tests/v1_severability_tripwire.rs` turns that
//! reading into an assertion, so the property survives the next edit.
//!
//! # How the pair is selected
//!
//! This file's twin is `v1_session.rs`, which holds the real v1 state. Exactly
//! one of the two is compiled, chosen by two `cfg_attr` path attributes on a
//! single `mod v1;` declaration in `src/server/streamable_http_server.rs`. The
//! transport therefore names `v1::…` unconditionally and never grows a
//! feature-gated call site.
//!
//! Because the compiler picks the half, a signature that drifts between the two
//! is a build failure on one feature set — the fastest possible feedback. The
//! tripwire covers the direction a build cannot see: that this file declares
//! nothing its twin does not, i.e. that severance never grows machinery of its
//! own.
//!
//! # Why the parameters are still here
//!
//! Every function below takes the same arguments as its real counterpart and
//! ignores them — most visibly the resolved `era`. Dropping a parameter the twin
//! does not read would make the two signatures diverge, and a caller that no
//! longer has to supply an era is a caller that is one edit away from resolving
//! one for itself. Phase 112 D-11 and Phase 113 Pitfall 2 forbid a second era
//! resolver in the transport: the era is resolved ONCE at ingress and CONSUMED
//! everywhere else. Identical parameter lists are how that stays true on both
//! builds.
//!
//! # This file is temporary
//!
//! Gating v1 off is reversible and semver-safe. DELETING the pair outright is a
//! major-version change, tracked as SMPL-F1 (pmcp 3.0) and gated on public
//! client adoption of the 2026-07-28 protocol. The policy — deliberately with no
//! date in it — is `docs/v1-sunset-policy.md`.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution already used by `src/server/task_dispatch.rs` and
// `src/shared/http_body_cap.rs`. The real half carries the identical allow.
#![allow(clippy::redundant_pub_crate)]

// The import list is itself a severance measurement, so read it before the code.
//
// Plan 117-13 removed SIX names from it — `create_error_response`,
// `EventStoreHandle`, `error_codes`, `StatusCode`, `sse::Event` and `mpsc` —
// when the GET body moved into the real half and took the last twins that used
// them. What is left is the vocabulary of an answer, not of a transport: this
// file now frames no error of its own (the one `405` it returns comes from the
// shared constructor in the parent), names no event-store handle, and cannot
// even mention an SSE event or a channel sender.
use super::{ServerState, StreamableHttpServerConfig};
use crate::shared::TransportMessage;
use crate::types::protocol::Era;
use axum::http::HeaderMap;
use axum::response::Response;

/// The zero-sized stand-in for the v1 session and resumability state.
///
/// A unit struct, not an empty-braced one: the absence of fields is the whole
/// point, and the unit form makes it impossible to add one without changing the
/// declaration that the tripwire reads. Nothing is allocated, nothing is locked,
/// and nothing is retained, because on this build there are no sessions to track
/// and no events to replay.
#[derive(Clone, Debug, Default)]
pub(crate) struct V1State;

impl V1State {
    /// The v2 constant answer to "give me v1 state": a value with no contents.
    ///
    /// Callable identically to its twin so the transport's construction site is
    /// written once. Allocating nothing here is not an optimisation; it is the
    /// observable difference between the two builds.
    pub(crate) const fn new(_config: &StreamableHttpServerConfig) -> Self {
        Self
    }
}

// ---------------------------------------------------------------------------
// v1 session-map operations — every answer is a constant.
// ---------------------------------------------------------------------------

/// No session, so no version was ever negotiated against one.
///
/// The 2026-07-28 transport carries its version per request, so a session-scoped
/// version would be the wrong authority even if one existed.
pub(crate) const fn session_protocol_version(
    _state: &V1State,
    _session_id: &str,
) -> Option<String> {
    None
}

/// No session, so no level was ever recorded against one.
///
/// The 2026-07-28 transport RETIRED `logging/setLevel` and carries the level per
/// request in `params._meta` instead, so a session-scoped level would be the
/// wrong authority even if a session existed to hold one. The resolver at the
/// HTTP ingress reads the `_meta` key on this build and never consults this
/// answer for anything but the v1 arm it can no longer take.
///
/// `None` also means the D-12 default (`info`) applies to any request that sends
/// no `_meta` level — which is the same answer the real half gives a v1 session
/// that never called `setLevel`.
pub(crate) const fn session_log_level(
    _state: &V1State,
    _session_id: &str,
) -> Option<crate::types::LoggingLevel> {
    None
}

/// Recording a level is a no-op: there is no session row to record it against.
///
/// The RPC that would call this is one of the five the 2026-07-28 core schema
/// removes, so on this build the write can never be reached with a live session
/// id — and there is no map here for it to grow even if it were (T-118.2-07-02).
/// The parameters are taken so the signature matches the real half's and are
/// never read.
pub(crate) const fn set_session_log_level(
    _state: &V1State,
    _session_id: &str,
    _level: crate::types::LoggingLevel,
) {
}

// ---------------------------------------------------------------------------
// v1 SSE stream operations — every answer is a constant.
// ---------------------------------------------------------------------------

/// The message is always handed straight back, never routed anywhere.
///
/// There is no stream to deliver into, so the caller always frames the reply for
/// the caller that actually asked for it. That is the v2 rule stated positively:
/// a response goes to its requester and to nobody else.
pub(crate) const fn route_to_session_stream(
    _state: &V1State,
    _session_id: &str,
    message: TransportMessage,
) -> Option<TransportMessage> {
    Some(message)
}

// ---------------------------------------------------------------------------
// Session era gate — the v2 constant answers.
//
// Every signature below is its real counterpart's, arity and parameter types
// intact, `era` included. The twins ignore `era`; they do NOT drop it. A caller
// that no longer had to supply an era would be one edit away from resolving one
// for itself, and Phase 112 D-11 / Phase 113 Pitfall 2 forbid a second era
// resolver in the transport: the era is resolved ONCE at ingress and CONSUMED
// everywhere else. Keeping the parameter is how that stays true on this build
// too. (Unused names carry a leading underscore, which is Rust's marker for
// "deliberately ignored" — the types and the arity are what callers see.)
//
// The pure `_for` rule and the state-reading `fn` stay SEPARATE items here as
// well. Flattening them would leave the real half with a rule the twin has no
// counterpart for, and the rule is exactly what the truth-table and property
// tests exercise without constructing a live `ServerState`.
// ---------------------------------------------------------------------------

/// The session-era rule, collapsed to a constant.
///
/// There is no MCP 2025-11-25 session concept in this build, so a configured
/// generator is not evidence of anything and the era does not need consulting:
/// the answer is `false` for every input.
pub(crate) const fn sessions_active_for(_cfg_has_generator: bool, _era: Option<Era>) -> bool {
    false
}

/// Sessions are never live on this build.
///
/// Routed through [`sessions_active_for`] rather than returning `false` inline,
/// for two reasons: the rule stays the single place the answer comes from on
/// BOTH halves, and `era` is visibly CONSUMED here instead of discarded.
pub(crate) const fn sessions_active(_state: &ServerState, era: Option<Era>) -> bool {
    sessions_active_for(false, era)
}

/// No `Mcp-Session-Id` response header is ever emitted, on any request.
///
/// The real half is "the ONE place" that header is written; here there is no
/// such place at all, which is the stronger form of the same invariant. The
/// `headers` argument is taken and left untouched.
pub(crate) const fn apply_session_header(
    _headers: &mut HeaderMap,
    _response_session_id: Option<&String>,
    _sessions_on: bool,
) {
}

// ---------------------------------------------------------------------------
// Resumability era gate — the v2 constant answers.
//
// The 2026-07-28 transport spec is verbatim that resumable SSE streams via
// `Last-Event-ID` are not supported. On this build that is not a runtime
// refusal: there is no store to reach, no replay path compiled behind these
// functions, and nothing that reads the header.
// ---------------------------------------------------------------------------

/// The resumability rule, collapsed to a constant.
///
/// `false` for every input: this build offers no resumability to gate.
pub(crate) const fn resumability_active_for(_cfg_has_event_store: bool, _era: Option<Era>) -> bool {
    false
}

/// Resumability is never live on this build.
///
/// Routed through [`resumability_active_for`] for the same two reasons
/// [`sessions_active`] is routed through its rule.
pub(crate) const fn resumability_active(_state: &ServerState, era: Option<Era>) -> bool {
    resumability_active_for(false, era)
}

/// No inbound session id is ever read, because no header is ever looked at.
///
/// The v1 half of `super::extract_session_and_protocol_headers` — a MIXED
/// function that also reads `MCP-Protocol-Version`, which this build REQUIRES
/// (VERS-05). That is why the function stays in the transport and only this read
/// is paired.
///
/// This makes the POST PIPELINE session-id-free on this build. It does NOT make
/// the whole build header-blind: `super::build_middleware_context` is ungated and
/// still reads `Mcp-Session-Id` into `ServerHttpContext::session_id` on the
/// middleware POST path, because `http_middleware` is a shared, era-neutral
/// config field. See the real half's counterpart doc — the exception is named
/// there rather than papered over here.
///
/// `headers` is taken so the signature matches its real counterpart and is never
/// touched. Do not "improve" this by reading the header to log or reject an
/// unexpected session id: `tests/v1_severability_tripwire.rs` fails on the
/// `MCP_SESSION_ID` and `headers.get` tokens for exactly that reason, and a
/// build that reads the header in order to complain about it is still a build
/// that reads the header.
pub(crate) const fn incoming_session_header(_headers: &HeaderMap) -> Option<String> {
    None
}

// ---------------------------------------------------------------------------
// v1 session LIFECYCLE — the v2 constant answers (plan 117-12, SMPL-02).
//
// The 2026-07-28 transport is handshake-free and session-free: there is no
// `initialize`, no `Mcp-Session-Id` to mint, demand or echo, and no session
// record to hold a negotiated version. Every stage below therefore collapses to
// the answer "there is no session", and the v1 bodies that computed those
// answers are not compiled into this build at all.
//
// # Why the parameters are still here — `session_id: Option<String>` in particular
//
// The POST pipeline threads `session_id: Option<String>` through roughly ten
// functions. Keeping it means it is simply always `None` on this build; dropping
// it would mean rewriting all ten, in a plan whose whole point is that the v1
// wire stays byte-identical. Worse, a call site that no longer had to supply a
// session id would be one edit away from deciding for itself whether sessions
// apply — a SECOND era decision, which Phase 112 D-11 and Phase 113 Pitfall 2
// exist to forbid. Identical signatures are what keep the single decision
// single.
// ---------------------------------------------------------------------------

// Four of the seven twins below are plain `fn` rather than `const fn`: they take
// an OWNED `Option<String>`, whose destructor cannot be evaluated at compile
// time (E0493). Constness follows the SIGNATURE, which is fixed by the real
// half; it is never bought by changing a parameter type.

/// No session is ever minted, because there is no `initialize` to mint one for.
///
/// `(None, false)` is exactly what the real function answers when sessions are
/// inactive — "no session id, and nothing was newly created".
pub(crate) fn process_init_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<String>,
    _protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    Ok((None, false))
}

/// Nothing is required and nothing is validated, so nothing can be rejected.
///
/// An inbound `Mcp-Session-Id` is IGNORED rather than rejected, which is the
/// transport spec taken literally — and here it is structural: there is no
/// session map to look the id up in, so it cannot be consulted by accident.
pub(crate) fn validate_non_init_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    Ok(None)
}

/// Recording the outcome of an initialization is a no-op: nothing was recorded
/// to update.
pub(crate) fn update_session_after_init(
    _state: &ServerState,
    _session_id: Option<&String>,
    _negotiated_version: Option<String>,
) {
}

/// A per-request version can never disagree with a session-recorded one,
/// because no session records one.
///
/// This is the same `Ok(())` the real function returns from its own first line
/// when sessions are inactive — so the twin is not a behaviour change but the
/// compile-time realisation of a behaviour that already held. On this build the
/// per-request `MCP-Protocol-Version` is the sole authority, which is the
/// Phase-112 lock stated positively.
pub(crate) const fn validate_protocol_version_matches_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<&String>,
    _protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    Ok(())
}

/// There is never a response session id, on any request, ever.
///
/// Both branches — mint on `initialize`, validate otherwise — collapse to the
/// same `Ok(None)`. They are still WRITTEN as two branches, mirroring the real
/// half, for the reason plan 117-09 recorded for the era twins: routing through
/// the two stage functions keeps them the single place the answer comes from on
/// BOTH halves, and keeps `is_init_request` visibly CONSUMED rather than
/// discarded.
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
// v1 SSE RESUMABILITY — the v2 constant answers (plan 117-12, SMPL-02).
//
// The 2026-07-28 transport spec is verbatim: "Resumable SSE streams via
// `Last-Event-ID` are not supported", and a `Last-Event-ID` header on a request
// should be ignored. On this build that is not a runtime refusal and not an
// early return — there is no store to reach, no replay loop compiled behind
// these functions, and NOTHING IN THIS FILE NAMES THAT HEADER.
//
// That last property is the one to check when editing this block: a twin that
// took a header map and looked inside it, even to decide to do nothing, would
// re-open a threat this build closes by construction.
// ---------------------------------------------------------------------------

/// Nothing is ever persisted, because there is no store to persist into.
///
/// Retaining response envelopes that can never be replayed would be dead
/// retention of exactly the material an id-replay bug feeds on (T-113-30); here
/// the retention is not merely gated off, it is not compiled.
///
/// Stays `async` because the signature is the real half's and both POST
/// handlers `.await` it.
pub(crate) async fn store_response_event(
    _state: &ServerState,
    _era: Option<Era>,
    _response_session_id: Option<&String>,
    _response_msg: &TransportMessage,
) {
}

// ---------------------------------------------------------------------------
// The v1 HTTP verb bodies — the constant `405` a `full-v2` build answers.
// ---------------------------------------------------------------------------

/// `GET /` is never served: this build has no SSE stream to open.
///
/// SSE is a MCP 2025-11-25 transport feature; 2026-07-28 replaced it with
/// per-request responses. Its real counterpart validates headers, resolves a
/// session, registers a stream and replays an event log — none of which exists
/// here, so there is nothing to answer but `405`.
///
/// # Refused, not missing
///
/// The verb stays ROUTED (`build_mcp_router` is identical on both feature sets).
/// Dropping the route would answer `404`, which says "no such endpoint" rather
/// than "this endpoint does not take this verb" — a different wire answer that
/// `tests/v2_verbs_405_on_severed_build.rs` explicitly rejects.
///
/// # One 405, not two
///
/// The response comes from [`super::method_not_allowed_for_verb`], the SAME
/// constructor the v2 rejection head uses. A locally hand-rolled `405` would be
/// a second answer to one question, free to drift on the next edit.
///
/// # `405` PREEMPTS `406` — a deliberate status difference
///
/// The real half calls `validate_headers(headers, "GET")` first, so a GET with a
/// wrong `Accept` is answered `406 Not Acceptable` there. This twin answers `405`
/// for EVERY GET, whatever the headers say, and that is intended rather than an
/// oversight: content negotiation is a question about how to serve a request, and
/// this endpoint does not serve GET at all on `2026-07-28`. Answering `406` would
/// tell the caller to fix its `Accept` header and try again, which is advice that
/// cannot work.
///
/// It also keeps the answer INPUT-INDEPENDENT, which is the same property the
/// DELETE twin's doc names: one status for all inputs discloses nothing about
/// what this build does or does not hold.
///
/// `headers` is taken so the signature matches the real half's and is never
/// read: no `Mcp-Session-Id`, no `Accept`, no replay cursor. Stays `async`
/// because the real half awaits its SSE replay and the shared head `.await`s the
/// call.
pub(crate) async fn handle_get_sse_body(_state: &ServerState, _headers: &HeaderMap) -> Response {
    super::method_not_allowed_for_verb("GET")
}

/// `DELETE /` is never served: there is no session to terminate.
///
/// The real half looks a session up, removes its SSE stream, forgets it and
/// fires `on_session_closed`. On this build no session is ever created, so the
/// only honest answer is that the endpoint does not take this verb — and,
/// deliberately, the same `405` as the GET twin rather than the real half's
/// `404 Unknown session ID`, which would leak a session-existence oracle out of
/// a build that tracks no sessions.
pub(crate) fn handle_delete_body(_state: &ServerState, _headers: &HeaderMap) -> Response {
    super::method_not_allowed_for_verb("DELETE")
}
