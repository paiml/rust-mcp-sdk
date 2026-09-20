//! Per-request protocol context and W3C trace-context value types.
//!
//! These are the additive foundation value types the v2.5 milestone era-gates
//! off. [`ProtocolContext`] carries the once-resolved-at-ingress era plus the
//! negotiated version and optional client identity. [`TraceContext`] surfaces
//! the W3C distributed-tracing headers a client self-reports through the request
//! `_meta` object.

use super::version::{Era, SUPPORTED_PROTOCOL_VERSIONS};
use super::{Implementation, ProtocolVersion};
use crate::types::capabilities::ClientCapabilities;

/// The v1-only default protocol accept-list (Phase 112 D-02/D-04).
///
/// Maps the legacy [`SUPPORTED_PROTOCOL_VERSIONS`] string slice into owned
/// [`ProtocolVersion`] values. This is the accept-list a server carries when the
/// author never calls `.with_supported_protocol_versions(...)` — it deliberately
/// EXCLUDES `2026-07-28` (v2), so an un-opted-in server runs zero era-detection
/// and its v1 request path is byte-for-byte unchanged. It is also the safe
/// fallback for an explicitly-empty accept-list (never produce an all-reject
/// server).
#[must_use]
pub(crate) fn default_accept_list() -> Vec<ProtocolVersion> {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .map(|v| ProtocolVersion((*v).to_string()))
        .collect()
}

/// Normalize a caller-supplied accept-list, falling back to the v1-only
/// [`default_accept_list`] when it is empty.
///
/// The empty-means-v1 fallback is a documented safety invariant (D-02/D-04):
/// an explicitly-empty accept-list must never produce an all-reject server.
/// Shared by both server builders so the invariant lives in exactly one place.
#[must_use]
pub(crate) fn normalize_accept_list(
    versions: impl IntoIterator<Item = ProtocolVersion>,
) -> Vec<ProtocolVersion> {
    let collected: Vec<ProtocolVersion> = versions.into_iter().collect();
    if collected.is_empty() {
        default_accept_list()
    } else {
        collected
    }
}

/// Whether an accept-list opted into the v2 (`2026-07-28`) era.
///
/// The cheap "run era-detection at all" gate (D-04): when `false`, ingress
/// skips the resolver entirely and the v1 request path is byte-for-byte
/// unchanged. Classifies via the single [`protocol_era`](super::version::protocol_era)
/// source of truth so the era rule is never re-derived by hand.
#[must_use]
pub(crate) fn is_v2_opted_in(accept_list: &[ProtocolVersion]) -> bool {
    accept_list
        .iter()
        .any(|v| super::version::protocol_era(v.as_str()) == Era::V2)
}

/// Maximum accepted length, in bytes, for any single W3C trace value.
///
/// A legitimate `traceparent` is a fixed ~55-byte string; `tracestate` and
/// `baggage` are spec-capped but we allow a generous bound. Any value at
/// ingress exceeding this cap is rejected (see [`TraceContext::from_meta`]) so
/// an attacker-controlled oversized tracing value is never propagated to a
/// handler (threat T-112-09, bounded ingress).
const MAX_TRACE_VALUE_LEN: usize = 8192;

/// A DECRYPTED, cryptographically verified MRTR continuation (Phase 113,
/// HTTP-02/HTTP-03).
///
/// Produced ONLY by the dispatch layer, after the server-owned `requestState`
/// codec returned an authentic, live verdict for the token the client echoed.
/// It is therefore SERVER-MINTED and TRUSTED — unlike `inputResponses`, which is
/// client-supplied and must be schema-validated by the handler.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedContinuation {
    /// The handler-owned continuation state the server sealed on a prior round.
    pub state: serde_json::Value,
    /// Which multi-round-trip round this continuation belongs to (D-09).
    pub round: u8,
}

/// The per-request server-to-client capability handles a TRANSPORT can hand to
/// dispatch (Phase 118.1 plan 10, CONF-07 / G-3).
///
/// # Why it rides on [`ProtocolContext`]
///
/// `RequestHandlerExtra` is built deep inside the dispatchers
/// (`Server::call_tool_with_context`, `ServerCore::handle_call_tool`) and the
/// peer is applied by `attach_peer` from a SINGLE global `peer_handle` field. A
/// transport cannot reach either. But `ProtocolContext` is already resolved once
/// at ingress, already threaded through `handle_request_with_context`, and
/// already moved onto `RequestHandlerExtra` by `.with_protocol_context(..)` at
/// both dispatch roots — so a request-scoped carrier riding here reaches the
/// handler with NO signature change at any call site.
///
/// # `Debug` is hand-written and reports PRESENCE only
///
/// Both fields are live capability handles: the peer can issue
/// `sampling/createMessage` and `elicitation/create` at the client, and the sink
/// can emit notifications. They follow the same redaction discipline as the MRTR
/// fields on the enclosing type — a single `tracing::debug!("{ctx:?}")` must not
/// publish a capability handle or let a reader correlate one request's context
/// with another's (T-118.1-10-09, the T-113-05 / T-113-31 class).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
pub(crate) struct TransportBackchannel {
    /// The peer handle bound to the ORIGINATING session, if the transport has
    /// a server-to-client channel for this request.
    peer: Option<std::sync::Arc<dyn crate::shared::peer::PeerHandle>>,
    /// A one-way notification sink for the same session.
    ///
    /// Typed EXACTLY as `ServerProgressReporter::new`'s second parameter so it
    /// can be handed straight in with no adapter.
    notification_sink: Option<std::sync::Arc<dyn Fn(crate::types::Notification) + Send + Sync>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for TransportBackchannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportBackchannel")
            .field("has_peer", &self.peer().is_some())
            .field("has_notification_sink", &self.notification_sink().is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Unwind safety, asserted DELIBERATELY.
//
// `PeerHandle` and the notification-sink `Fn` are public trait objects declared
// without `+ RefUnwindSafe`, so `Arc<dyn …>` of either is neither `UnwindSafe`
// nor `RefUnwindSafe`, and without these two impls the enclosing PUBLIC
// `ProtocolContext` silently stops implementing both. `cargo semver-checks`
// classifies that as `auto_trait_impl_removed` — a MAJOR break — which would
// turn an intentionally additive `pub(crate)` field into a 3.0 gate. It is not a
// theoretical concern: the check caught it on the first run of this plan.
//
// The assertion is honest rather than expedient. Both fields are opaque,
// immutable-after-construction `Arc` capability handles: this type never mutates
// through them and holds no invariant that a panic could tear. Every piece of
// mutable state they reach — the dispatcher's `pending` and `owners` maps — is
// behind `tokio::sync::RwLock`, which has no poisoning and therefore publishes
// no broken-invariant signal for a `catch_unwind` caller to observe. Adding
// `+ RefUnwindSafe` to the `PeerHandle` trait instead would be a MAJOR break of
// its own (a new supertrait every external implementor must satisfy), so this is
// the additive form of the same guarantee.
//
// Both are SAFE auto traits, so these are ordinary impls and assert nothing the
// compiler would otherwise have to trust us about for memory safety.
#[cfg(not(target_arch = "wasm32"))]
impl std::panic::UnwindSafe for TransportBackchannel {}

#[cfg(not(target_arch = "wasm32"))]
impl std::panic::RefUnwindSafe for TransportBackchannel {}

#[cfg(not(target_arch = "wasm32"))]
impl TransportBackchannel {
    /// An empty backchannel. Layer capabilities on with the `with_*` builders.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Attach the session-bound peer handle.
    #[must_use]
    pub(crate) fn with_peer(
        mut self,
        peer: std::sync::Arc<dyn crate::shared::peer::PeerHandle>,
    ) -> Self {
        self.peer = Some(peer);
        self
    }

    /// Attach the session-bound notification sink.
    #[must_use]
    pub(crate) fn with_notification_sink(
        mut self,
        sink: std::sync::Arc<dyn Fn(crate::types::Notification) + Send + Sync>,
    ) -> Self {
        self.notification_sink = Some(sink);
        self
    }

    /// The session-bound peer handle, if the transport supplied one.
    pub(crate) fn peer(&self) -> Option<&std::sync::Arc<dyn crate::shared::peer::PeerHandle>> {
        self.peer.as_ref()
    }

    /// The session-bound notification sink, if the transport supplied one.
    pub(crate) fn notification_sink(
        &self,
    ) -> Option<&std::sync::Arc<dyn Fn(crate::types::Notification) + Send + Sync>> {
        self.notification_sink.as_ref()
    }
}

/// The protocol context resolved once at request ingress and threaded through
/// dispatch.
///
/// This is an additive `#[non_exhaustive]` value type: construct it with
/// [`ProtocolContext::new`] and layer optional fields via the `with_*`
/// builders. The four negotiation fields are public and directly readable; the
/// MRTR fields are crate-private (see `ProtocolContext::with_mrtr_params`).
///
/// # `Debug` is hand-written and REDACTS the MRTR fields
///
/// A derived `Debug` printed `VerifiedContinuation::state` — the DECRYPTED,
/// server-minted continuation, which routinely holds partially collected tool
/// arguments — plus the raw `requestState` token and the client's
/// `inputResponses`, verbatim. This value is carried on
/// [`RequestHandlerExtra`](crate::RequestHandlerExtra), whose own `Debug` prints
/// it, so a single `tracing::debug!("{extra:?}")` in a handler or middleware
/// published exactly the material the AEAD token exists to seal (T-113-05 /
/// T-113-31). The impl below reports only PRESENCE for those three, matching the
/// redaction discipline `RequestHandlerExtra`'s `Debug` already applies to
/// `metadata` and `task_request`.
#[derive(Clone)]
#[non_exhaustive]
pub struct ProtocolContext {
    /// The behavioral era the negotiated version belongs to (v1 vs v2).
    pub era: Era,
    /// The exact protocol version negotiated for this session/request.
    pub negotiated_version: ProtocolVersion,
    /// The client's self-reported implementation info, if known.
    pub client_info: Option<Implementation>,
    /// The client's advertised capabilities, if known.
    pub client_capabilities: Option<ClientCapabilities>,
    /// The per-request MRTR continuation signals, extracted from the raw
    /// `params` at transport ingress; `None` on v1 and on any non-opted-in
    /// request (D-04's zero-era-code rule).
    ///
    /// Crate-private DELIBERATELY: `MrtrRequestParams` is `pub(crate)`
    /// (Phase-113 D-10 keeps the MRTR plumbing off the public API), so a `pub`
    /// field here would expose a private type from a public interface. Handlers
    /// read the values through the `RequestHandlerExtra` MRTR accessors.
    pub(crate) mrtr: Option<crate::types::mrtr::MrtrRequestParams>,
    /// The decrypted, verified continuation for this request.
    ///
    /// Set by the DISPATCH layer only (never at transport ingress) after the
    /// server-owned codec verified the echoed `requestState` against the live
    /// principal and originating request.
    pub(crate) mrtr_verified: Option<VerifiedContinuation>,
    /// The per-request server-to-client capability handles the TRANSPORT
    /// supplied, if it has a back-channel for this request (CONF-07 / G-3).
    ///
    /// Crate-private for the same reason as the MRTR fields: this is internal
    /// plumbing, and [`TransportBackchannel`] is `pub(crate)`, so a `pub` field
    /// would expose a private type from a public interface. Adding a
    /// `pub(crate)` field to a `#[non_exhaustive]` public struct is semver-MINOR.
    ///
    /// `#[cfg(not(target_arch = "wasm32"))]` because `src/shared/peer.rs` carries
    /// a module-level gate of the same shape, so `PeerHandle` does not exist on
    /// wasm at all.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) transport_backchannel: Option<TransportBackchannel>,
    /// The minimum [`LoggingLevel`](crate::types::LoggingLevel) this request's
    /// client asked to receive, resolved at HTTP ingress (Phase 118.2, CONF-10).
    ///
    /// # The problem this field exists to solve
    ///
    /// The level and the emitter live at opposite ends of dispatch. The HTTP
    /// ingress KNOWS the level — a v1 session's stored `logging/setLevel` value,
    /// or a v2 request's `io.modelcontextprotocol/logLevel` `_meta` key — but it
    /// never constructs a [`RequestHandlerExtra`](crate::RequestHandlerExtra):
    /// every construction site lives in `src/server/core.rs` and
    /// `src/server/mod.rs`. The dispatch roots construct the `extra` but know
    /// nothing about sessions. `ProtocolContext` is the value ALREADY threaded
    /// between the two, so it is the carrier that needs no new signature
    /// anywhere.
    ///
    /// Read by `server::core::attach_request_log_sink`, the one unit both native
    /// dispatch roots call, which applies it via
    /// `RequestHandlerExtra::with_log_level`. `None` means "nothing resolved",
    /// and the emitter then falls back to
    /// [`DEFAULT_LOG_LEVEL`](crate::server::cancellation::DEFAULT_LOG_LEVEL).
    ///
    /// # Rejected carrier, recorded so it is not re-proposed
    ///
    /// `server::streamable_http_server::peer_channel::attach_session_backchannel`
    /// looks like the natural place, but it returns EARLY when sessions are off
    /// or the request is an `initialize`. v2 has no sessions at all, so that
    /// route structurally cannot carry a v2 level — a carrier that works for one
    /// era and not the other is exactly the drift this phase exists to remove.
    ///
    /// # Isolation
    ///
    /// Per-request by construction: `ProtocolContext` is built at ingress and
    /// moved into THAT request's `RequestHandlerExtra`. It is never stored on the
    /// shared server, so one caller's level cannot leak into another's
    /// (T-118.2-06-05).
    ///
    /// Crate-private for the same reason as the MRTR fields — internal plumbing —
    /// and additive on an already-`#[non_exhaustive]` public struct.
    /// `Option<LoggingLevel>` is a plain `Copy` enum, so unlike
    /// [`TransportBackchannel`] it removes no auto-trait impl (see the
    /// `UnwindSafe`/`RefUnwindSafe` note above).
    pub(crate) resolved_log_level: Option<crate::types::notifications::LoggingLevel>,
}

impl std::fmt::Debug for ProtocolContext {
    /// Renders the negotiation fields in full and the MRTR fields as PRESENCE
    /// only — see the type-level note for why the derive was unsafe here.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut binding = f.debug_struct("ProtocolContext");
        let out = binding
            .field("era", &self.era)
            .field("negotiated_version", &self.negotiated_version)
            .field("client_info", &self.client_info)
            .field("client_capabilities", &self.client_capabilities)
            .field("has_input_responses", &self.input_responses().is_some())
            .field(
                "has_request_state_token",
                &self.request_state_token().is_some(),
            )
            .field("has_verified_continuation", &self.mrtr_verified.is_some())
            .field("mrtr_round", &self.mrtr_round())
            // Printed VERBATIM, not as presence: a severity threshold is not
            // sensitive, and "which level was resolved for this request" is
            // precisely the fact a developer chasing "my log records went
            // nowhere" needs (the same reasoning `RequestHandlerExtra`'s
            // `Debug` applies to its own `log_level`).
            .field("resolved_log_level", &self.resolved_log_level);
        // PRESENCE only. A peer handle and a notification sink are live
        // capability handles, so they follow the same redaction discipline as
        // the MRTR fields above (T-118.1-10-09).
        #[cfg(not(target_arch = "wasm32"))]
        out.field(
            "has_transport_backchannel",
            &self.transport_backchannel().is_some(),
        );
        out.finish()
    }
}

impl ProtocolContext {
    /// Construct a `ProtocolContext` from the resolved era and negotiated
    /// version. `client_info` and `client_capabilities` default to `None`.
    #[must_use]
    pub fn new(era: Era, negotiated_version: ProtocolVersion) -> Self {
        Self {
            era,
            negotiated_version,
            client_info: None,
            client_capabilities: None,
            mrtr: None,
            mrtr_verified: None,
            #[cfg(not(target_arch = "wasm32"))]
            transport_backchannel: None,
            resolved_log_level: None,
        }
    }

    /// The minimum log level resolved for this request, if the ingress resolved
    /// one (Phase 118.2, CONF-10).
    ///
    /// Returns an OWNED `Option` rather than a borrow because `LoggingLevel` is
    /// `Copy`. Read by `server::core::attach_request_log_sink` — see the field's
    /// documentation for why the carrier lives here.
    // Why `allow(dead_code)`: the sole reader is `server::core::attach_request_log_sink`,
    // which is `#[cfg(not(target_arch = "wasm32"))]`; on wasm32 this accessor has no caller.
    #[allow(dead_code)]
    pub(crate) fn resolved_log_level(&self) -> Option<crate::types::notifications::LoggingLevel> {
        self.resolved_log_level
    }

    /// Attach the log level the HTTP ingress resolved for this request.
    ///
    /// Called by the transport ingress ONLY, once, after the era gate has run and
    /// the level source (a v1 session's stored `logging/setLevel` value, or a v2
    /// request's `_meta` key) is known. Never called by dispatch: dispatch READS
    /// this value, it does not mint one.
    // Why `allow(dead_code)`: the WRITER is the HTTP ingress
    // (`server::streamable_http_server::resolve_request_log_level`, plan 07, which landed it at
    // both POST ingress paths). That transport is not compiled on wasm32 or on a build without the
    // HTTP server features, so on those configurations this builder has no caller.
    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn with_resolved_log_level(
        mut self,
        level: crate::types::notifications::LoggingLevel,
    ) -> Self {
        self.resolved_log_level = Some(level);
        self
    }

    /// The per-request transport back-channel, if the transport supplied one.
    ///
    /// Read by `attach_peer` at both dispatch roots, which prefers a
    /// REQUEST-SCOPED peer over the server's single global `peer_handle` — that
    /// is what makes a server-to-client request reach the session that issued it
    /// on a multiplexed transport (T-118.1-10-04).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn transport_backchannel(&self) -> Option<&TransportBackchannel> {
        self.transport_backchannel.as_ref()
    }

    /// Attach the transport's per-request server-to-client capability handles.
    ///
    /// Called by the STREAMABLE-HTTP transport, once, after the era gate has run
    /// and the originating session is known. Never called by dispatch: the value
    /// is transport-owned by construction, so a handler cannot mint itself one.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub(crate) fn with_transport_backchannel(mut self, backchannel: TransportBackchannel) -> Self {
        self.transport_backchannel = Some(backchannel);
        self
    }

    /// Attach the client's implementation info.
    #[must_use]
    pub fn with_client_info(mut self, client_info: Implementation) -> Self {
        self.client_info = Some(client_info);
        self
    }

    /// Attach the client's advertised capabilities.
    #[must_use]
    pub fn with_client_capabilities(mut self, client_capabilities: ClientCapabilities) -> Self {
        self.client_capabilities = Some(client_capabilities);
        self
    }

    /// The client-supplied `inputResponses` map, if any.
    pub(crate) fn input_responses(&self) -> Option<&crate::types::mrtr::InputResponses> {
        self.mrtr.as_ref()?.input_responses.as_ref()
    }

    /// The DECRYPTED continuation state from a verified token.
    pub(crate) fn mrtr_continuation(&self) -> Option<&serde_json::Value> {
        Some(&self.mrtr_verified.as_ref()?.state)
    }

    /// The round counter carried by a verified token.
    pub(crate) fn mrtr_round(&self) -> Option<u8> {
        Some(self.mrtr_verified.as_ref()?.round)
    }
}

/// The MRTR WRITE surface (Phase 113, HTTP-02/HTTP-03).
///
/// Split into its own block because these four are reached only from the
/// `streamable-http` MRTR path (D-14 confines the AEAD `requestState` codec to
/// that feature on native), while the three READ accessors above are consumed
/// unconditionally by `RequestHandlerExtra`. The conditional `allow` is
/// feature-scoped, not blanket: with `streamable-http` on — which is what every
/// lint and build gate uses — dead code here is still an error.
#[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
impl ProtocolContext {
    /// Attach the MRTR continuation signals read off the raw request `params`
    /// at transport ingress (Phase 113, HTTP-03).
    ///
    /// `inputResponses` / `requestState` are top-level `params` SIBLINGS of
    /// `name`/`arguments`/`uri`, not `_meta` keys, and the typed request structs
    /// deliberately do not model them (D-113-D: adding a field to a
    /// constructible `pub` struct is a MAJOR semver break). The transport
    /// therefore reads them from the raw body and rides them here, on the value
    /// already threaded end-to-end into `RequestHandlerExtra`.
    #[must_use]
    pub(crate) fn with_mrtr_params(mut self, mrtr: crate::types::mrtr::MrtrRequestParams) -> Self {
        self.mrtr = Some(mrtr);
        self
    }

    /// Attach the DECRYPTED, verified continuation and its round counter.
    #[must_use]
    pub(crate) fn with_verified_continuation(
        mut self,
        state: serde_json::Value,
        round: u8,
    ) -> Self {
        self.mrtr_verified = Some(VerifiedContinuation { state, round });
        self
    }

    /// Clear EVERY MRTR signal, so the handler sees a pristine FIRST call.
    ///
    /// This is the context half of the D-15 `UnknownKey`/`Expired` strip-and-
    /// re-run mechanic: the re-run handler must observe no continuation, no
    /// round and no input responses, exactly as if the client had never
    /// presented a token.
    #[must_use]
    pub(crate) fn without_mrtr(mut self) -> Self {
        self.mrtr = None;
        self.mrtr_verified = None;
        self
    }

    /// The client-echoed `requestState` token, if any (still UNVERIFIED).
    pub(crate) fn request_state_token(&self) -> Option<&str> {
        self.mrtr.as_ref()?.request_state.as_deref()
    }

    /// The client's `inputResponses` entries UNDECODED, as they arrived.
    ///
    /// The input to the kind-directed re-decode (D-113-O). Read only by the
    /// dispatch layer, and only after a continuation has verified — see
    /// [`with_kind_directed_input_responses`](Self::with_kind_directed_input_responses).
    pub(crate) fn input_responses_raw(
        &self,
    ) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.mrtr.as_ref()?.input_responses_raw.as_ref()
    }

    /// REPLACE the guessed `inputResponses` typing with the kind-directed one
    /// (D-113-O).
    ///
    /// At transport ingress the entries were typed by
    /// [`InputResponse::try_from_value_untagged`](crate::types::mrtr::InputResponse::try_from_value_untagged),
    /// which takes the first of three overlapping shapes that fits. Once the
    /// dispatch layer has opened the sealed continuation it knows which kind was
    /// requested under each key, and this is where the corrected map lands.
    ///
    /// Handlers see NO difference in shape: they still read
    /// `Option<&InputResponses>` through
    /// [`RequestHandlerExtra::input_responses`](crate::RequestHandlerExtra::input_responses),
    /// whose signature and behaviour are unchanged. Only the CORRECTNESS of the
    /// typing changes — which is the point: a handler matching on
    /// [`InputResponse::Elicitation`](crate::types::mrtr::InputResponse) now
    /// matches when the server asked for an elicitation, instead of falling
    /// through to a re-elicit because the value happened to also satisfy
    /// `CreateMessageResult`.
    ///
    /// No-op when no MRTR params were attached, which cannot happen on the path
    /// that calls this: a verified continuation implies a presented
    /// `requestState`, which implies `mrtr` is `Some`.
    #[must_use]
    pub(crate) fn with_kind_directed_input_responses(
        mut self,
        responses: crate::types::mrtr::InputResponses,
    ) -> Self {
        if let Some(mrtr) = self.mrtr.as_mut() {
            mrtr.input_responses = Some(responses);
            // The raw map's ONLY consumer is the kind-directed retype that just
            // ran, so from here it is dead weight — and `ProtocolContext` is
            // cloned on the dispatch path, which would deep-copy it (up to the
            // 256 KiB `inputResponses` bound) for the rest of the request.
            mrtr.input_responses_raw = None;
        }
        self
    }
}

/// Reserved `_meta` key carrying the per-request self-reported protocol version
/// (Phase 112, D-11). Read at ingress by [`resolve_protocol_context`].
pub(crate) const RESERVED_PROTOCOL_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

/// Reserved `_meta` key carrying the per-request self-reported `clientInfo`.
pub(crate) const RESERVED_CLIENT_INFO_KEY: &str = "io.modelcontextprotocol/clientInfo";

/// Reserved `_meta` key carrying the per-request self-reported `clientCapabilities`.
pub(crate) const RESERVED_CLIENT_CAPABILITIES_KEY: &str =
    "io.modelcontextprotocol/clientCapabilities";

/// The outcome of protocol negotiation failing at ingress (Phase 112, VERS-01).
///
/// Produced by [`resolve_protocol_context`] when a per-request `_meta` signal
/// cannot be honored. The caller (the native dispatch ingress) maps each variant
/// to a structured JSON-RPC rejection rather than silently disagreeing with the
/// wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolNegotiationError {
    /// A per-request version was present but is not in the server's configured
    /// accept-list (or a v2-only server received no v2 signal). Carries the
    /// offending/absent version string.
    UnsupportedVersion(String),
    /// A RESERVED `_meta` key was present but malformed (non-string
    /// `protocolVersion`, non-deserializable `clientInfo`/`clientCapabilities`,
    /// or a non-object `_meta`). Carries a static description.
    MalformedMeta(&'static str),
}

/// Resolve the per-request [`ProtocolContext`] ONCE at ingress from the server's
/// configured accept-list and the request's `_meta` (Phase 112, VERS-01; the
/// load-bearing spine).
///
/// This is a PURE, deterministic, `cfg`-agnostic function — the single source of
/// era resolution. The native dispatch sites (`core.rs`, `server/mod.rs`) call it
/// once and thread the result; the HTTP layer (Plan 06) resolves once for its
/// header gate and passes the SAME value in — it is never re-derived downstream
/// (D-11: `_meta`-authoritative, transport-agnostic). It compiles on wasm32 (no
/// wasm caller this phase) so the wasm build stays green.
///
/// # Behavior
///
/// - A per-request `protocolVersion` present and in `accept_list` → classified
///   via [`protocol_era`](super::version::protocol_era).
/// - A per-request version present but NOT in `accept_list` →
///   [`ProtocolNegotiationError::UnsupportedVersion`].
/// - No per-request version → falls back to the first v1 version in
///   `accept_list`; a v2-only accept-list with no v2 signal →
///   `UnsupportedVersion("")` (a v2-only server never silently serves v1).
/// - A malformed RESERVED `_meta` key → [`ProtocolNegotiationError::MalformedMeta`];
///   unrelated/unknown extension keys are IGNORED.
///
/// The per-request signal is authoritative over any session-stored version
/// (Pitfall 2) — this function never consults session state.
pub(crate) fn resolve_protocol_context(
    accept_list: &[ProtocolVersion],
    meta: Option<&serde_json::Value>,
) -> Result<Option<ProtocolContext>, ProtocolNegotiationError> {
    // A present `_meta` MUST be an object; a non-object reserved carrier can never
    // be reconciled with the wire, so fail closed rather than silently ignore it.
    let meta_obj =
        match meta {
            Some(value) => Some(value.as_object().ok_or(
                ProtocolNegotiationError::MalformedMeta("_meta is not an object"),
            )?),
            None => None,
        };

    // Resolve the negotiated version + era from the per-request signal, enforcing
    // the accept-list. The per-request signal is authoritative over any session
    // state (Pitfall 2) — session is never consulted here.
    let negotiated_version = resolve_negotiated_version(accept_list, meta_obj)?;
    let era = super::version::protocol_era(negotiated_version.as_str());

    let mut ctx = ProtocolContext::new(era, negotiated_version);
    if let Some(info) = parse_reserved_object::<Implementation>(
        meta_obj,
        RESERVED_CLIENT_INFO_KEY,
        "clientInfo is not deserializable",
    )? {
        ctx = ctx.with_client_info(info);
    }
    if let Some(caps) = parse_reserved_object::<ClientCapabilities>(
        meta_obj,
        RESERVED_CLIENT_CAPABILITIES_KEY,
        "clientCapabilities is not deserializable",
    )? {
        ctx = ctx.with_client_capabilities(caps);
    }
    Ok(Some(ctx))
}

/// Determine the negotiated [`ProtocolVersion`] from the per-request signal +
/// accept-list, or the v1 fallback when no signal is present.
fn resolve_negotiated_version(
    accept_list: &[ProtocolVersion],
    meta_obj: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<ProtocolVersion, ProtocolNegotiationError> {
    match meta_obj.and_then(|m| m.get(RESERVED_PROTOCOL_VERSION_KEY)) {
        Some(raw) => {
            let requested = raw.as_str().ok_or(ProtocolNegotiationError::MalformedMeta(
                "protocolVersion is not a string",
            ))?;
            if accept_list.iter().any(|v| v.as_str() == requested) {
                Ok(ProtocolVersion(requested.to_string()))
            } else {
                Err(ProtocolNegotiationError::UnsupportedVersion(
                    requested.to_string(),
                ))
            }
        },
        // Absent signal: fall back to the first v1 version in the accept-list.
        // A v2-only accept-list (no v1 version) never silently serves v1.
        None => first_v1_version(accept_list)
            .ok_or(ProtocolNegotiationError::UnsupportedVersion(String::new())),
    }
}

/// The first v1 version in an accept-list, or `None` for a v2-only list.
///
/// The absent-signal fallback rule, as ONE unit. [`resolve_negotiated_version`]
/// applies it when a request carries no era signal; `server::core`'s v1
/// handshake capability fold applies it when it has to synthesise a context a
/// non-opted-in server never resolved (Phase 118.1-08, G-9). Both must name the
/// SAME version, and a shared function is what makes that structural rather
/// than a comment claiming two copies "mirror each other exactly".
pub(crate) fn first_v1_version(accept_list: &[ProtocolVersion]) -> Option<ProtocolVersion> {
    accept_list
        .iter()
        .find(|v| super::version::protocol_era(v.as_str()) == Era::V1)
        .cloned()
}

/// Deserialize a present RESERVED `_meta` object key into `T`, mapping a
/// present-but-malformed value to [`ProtocolNegotiationError::MalformedMeta`].
/// Absent keys return `Ok(None)`.
fn parse_reserved_object<T: serde::de::DeserializeOwned>(
    meta_obj: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    malformed: &'static str,
) -> Result<Option<T>, ProtocolNegotiationError> {
    match meta_obj.and_then(|m| m.get(key)) {
        Some(raw) => serde_json::from_value::<T>(raw.clone())
            .map(Some)
            .map_err(|_| ProtocolNegotiationError::MalformedMeta(malformed)),
        None => Ok(None),
    }
}

/// W3C trace-context values extracted from a request `_meta` object.
///
/// # Security: values are RAW, UNVALIDATED, and self-reported
///
/// The `traceparent`, `tracestate`, and `baggage` strings are **untrusted**
/// data taken verbatim from the client-supplied `_meta` JSON. They are only
/// **length-bounded** (see `MAX_TRACE_VALUE_LEN`); no W3C syntax validation
/// is performed. These values MUST NOT be treated as trusted, authenticated,
/// or safe to interpolate into logs/queries without independent sanitization.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TraceContext {
    /// RAW, UNVALIDATED, self-reported W3C `traceparent` (length-bounded only).
    pub traceparent: String,
    /// RAW, UNVALIDATED, self-reported W3C `tracestate` (length-bounded only).
    pub tracestate: Option<String>,
    /// RAW, UNVALIDATED, self-reported W3C `baggage` (length-bounded only).
    pub baggage: Option<String>,
}

impl TraceContext {
    /// Extract a `TraceContext` from a request `_meta` JSON value.
    ///
    /// Returns `Some` only when the `_meta` object carries a `traceparent`
    /// string within `MAX_TRACE_VALUE_LEN`; returns `None` when it is absent,
    /// not a string, or over the bound. The optional `tracestate`/`baggage`
    /// keys are surfaced when present and in-bounds, and silently dropped when
    /// over the bound. Never panics on arbitrary untrusted input.
    ///
    /// The returned values are RAW/UNVALIDATED — see the type-level security
    /// note.
    #[must_use]
    pub fn from_meta(meta: &serde_json::Value) -> Option<Self> {
        // `traceparent` is required and gates the whole extraction: absent,
        // non-string, or over-bound => no trace context at all.
        let traceparent = bounded_trace_value(meta, "traceparent")?;
        // `tracestate`/`baggage` are optional: an over-bound value is dropped
        // (treated as absent for that field), never propagated.
        let tracestate = bounded_trace_value(meta, "tracestate");
        let baggage = bounded_trace_value(meta, "baggage");
        Some(Self {
            traceparent,
            tracestate,
            baggage,
        })
    }
}

/// Read a string-valued key out of a `_meta` object, enforcing the
/// [`MAX_TRACE_VALUE_LEN`] ingress bound.
///
/// Returns `None` when the key is absent, not a string, or over the bound so an
/// attacker-controlled oversized value is never surfaced (threat T-112-09).
fn bounded_trace_value(meta: &serde_json::Value, key: &str) -> Option<String> {
    let value = meta.get(key)?.as_str()?;
    if value.len() > MAX_TRACE_VALUE_LEN {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dual_accept_list() -> Vec<ProtocolVersion> {
        vec![
            ProtocolVersion("2025-11-25".to_string()),
            ProtocolVersion("2026-07-28".to_string()),
        ]
    }

    #[test]
    fn resolve_in_list_v2_signal_classifies_v2() {
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28" });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("v2 in accept-list => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V2);
        assert_eq!(ctx.negotiated_version.as_str(), "2026-07-28");
    }

    #[test]
    fn resolve_absent_signal_falls_back_to_v1() {
        // No per-request version + v1 present in the accept-list => v1 fallback.
        let ctx = resolve_protocol_context(&dual_accept_list(), None)
            .expect("v1 in accept-list => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V1);
        assert_eq!(ctx.negotiated_version.as_str(), "2025-11-25");
    }

    #[test]
    fn resolve_unsupported_version_errors() {
        // A per-request version not in the accept-list => UnsupportedVersion.
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: "1999-01-01" });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect_err("version not in accept-list => Err");
        assert_eq!(
            err,
            ProtocolNegotiationError::UnsupportedVersion("1999-01-01".to_string())
        );
    }

    #[test]
    fn resolve_v2_only_no_signal_errors() {
        // v2-only accept-list + no v2 signal => never silently serve v1.
        let v2_only = vec![ProtocolVersion("2026-07-28".to_string())];
        let err = resolve_protocol_context(&v2_only, None).expect_err("v2-only + no signal => Err");
        assert_eq!(
            err,
            ProtocolNegotiationError::UnsupportedVersion(String::new())
        );
    }

    #[test]
    fn resolve_malformed_reserved_key_errors() {
        // protocolVersion present but not a string => MalformedMeta.
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: 42 });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect_err("non-string protocolVersion => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));

        // _meta present but not an object => MalformedMeta.
        let non_object = json!("not-an-object");
        let err = resolve_protocol_context(&dual_accept_list(), Some(&non_object))
            .expect_err("non-object _meta => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));

        // clientInfo present but not deserializable => MalformedMeta.
        let bad_info = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            RESERVED_CLIENT_INFO_KEY: "should-be-an-object",
        });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&bad_info))
            .expect_err("malformed clientInfo => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));
    }

    #[test]
    fn resolve_unknown_extension_key_is_ignored() {
        // An unrelated extension key must NOT trip the resolver.
        let meta = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            "com.example/whatever": { "anything": [1, 2, 3] },
        });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("unknown key ignored => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V2);
    }

    #[test]
    fn resolve_populates_client_identity_when_well_formed() {
        let meta = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            RESERVED_CLIENT_INFO_KEY: { "name": "acme-client", "version": "1.2.3" },
            RESERVED_CLIENT_CAPABILITIES_KEY: {},
        });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("well-formed => Ok")
            .expect("resolved => Some");
        let info = ctx.client_info.expect("client_info populated");
        assert_eq!(info.name, "acme-client");
        assert_eq!(info.version, "1.2.3");
        assert!(ctx.client_capabilities.is_some());
    }

    #[test]
    fn protocol_context_new_defaults_optionals_to_none() {
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()));
        assert_eq!(ctx.era, Era::V2);
        assert_eq!(ctx.negotiated_version.as_str(), "2026-07-28");
        assert!(ctx.client_info.is_none());
        assert!(ctx.client_capabilities.is_none());
    }

    /// A context built WITHOUT the MRTR builder carries no MRTR signal at all —
    /// the Phase-112 construction path is unchanged (`mrtr == None`).
    #[test]
    fn protocol_context_without_the_mrtr_builder_has_no_mrtr() {
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()));
        assert!(ctx.mrtr.is_none());
        assert!(ctx.mrtr_verified.is_none());
        assert!(ctx.input_responses().is_none());
        assert!(ctx.request_state_token().is_none());
        assert!(ctx.mrtr_continuation().is_none());
        assert!(ctx.mrtr_round().is_none());

        // Every resolver-built context is equally MRTR-free: the resolver reads
        // `_meta`, and MRTR fields are top-level `params` siblings.
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28" });
        let resolved = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("resolves")
            .expect("some");
        assert!(resolved.mrtr.is_none());
    }

    #[test]
    fn protocol_context_with_mrtr_params_round_trips() {
        let mrtr = crate::types::mrtr::MrtrRequestParams {
            input_responses: None,
            input_responses_raw: None,
            request_state: Some("opaque-token".to_string()),
        };
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()))
            .with_mrtr_params(mrtr);
        assert_eq!(ctx.request_state_token(), Some("opaque-token"));
        // Not yet verified — the transport only READS the token, dispatch verifies.
        assert!(ctx.mrtr_continuation().is_none());
        assert!(ctx.mrtr_round().is_none());
    }

    #[test]
    fn protocol_context_verified_continuation_surfaces_state_and_round() {
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()))
            .with_verified_continuation(json!({ "step": 2 }), 3);
        assert_eq!(ctx.mrtr_continuation(), Some(&json!({ "step": 2 })));
        assert_eq!(ctx.mrtr_round(), Some(3));
    }

    /// The D-15 strip-and-re-run mechanic: after `without_mrtr` a handler cannot
    /// observe ANY MRTR signal.
    #[test]
    fn protocol_context_without_mrtr_clears_every_signal() {
        let mrtr = crate::types::mrtr::MrtrRequestParams {
            input_responses: Some(crate::types::mrtr::InputResponses::new()),
            input_responses_raw: None,
            request_state: Some("opaque-token".to_string()),
        };
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()))
            .with_mrtr_params(mrtr)
            .with_verified_continuation(json!({ "step": 9 }), 4)
            .without_mrtr();
        assert!(ctx.input_responses().is_none());
        assert!(ctx.request_state_token().is_none());
        assert!(ctx.mrtr_continuation().is_none());
        assert!(ctx.mrtr_round().is_none());
    }

    #[test]
    fn protocol_context_builders_set_optional_fields() {
        let ctx = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()))
            .with_client_info(Implementation::new("acme-client", "1.2.3"))
            .with_client_capabilities(ClientCapabilities::default());
        assert_eq!(ctx.era, Era::V1);
        let info = ctx.client_info.expect("client_info set");
        assert_eq!(info.name, "acme-client");
        assert_eq!(info.version, "1.2.3");
        assert!(ctx.client_capabilities.is_some());
    }

    #[test]
    fn trace_context_from_meta_extracts_all_fields() {
        let meta = json!({
            "traceparent": "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            "tracestate": "rojo=00f067aa0ba902b7",
            "baggage": "userId=alice"
        });
        let tc = TraceContext::from_meta(&meta).expect("traceparent present => Some");
        assert_eq!(
            tc.traceparent,
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
        assert_eq!(tc.tracestate.as_deref(), Some("rojo=00f067aa0ba902b7"));
        assert_eq!(tc.baggage.as_deref(), Some("userId=alice"));
    }

    #[test]
    fn trace_context_from_meta_traceparent_only() {
        let meta = json!({ "traceparent": "00-abc-def-01" });
        let tc = TraceContext::from_meta(&meta).expect("traceparent present => Some");
        assert_eq!(tc.traceparent, "00-abc-def-01");
        assert!(tc.tracestate.is_none());
        assert!(tc.baggage.is_none());
    }

    #[test]
    fn trace_context_from_meta_absent_returns_none() {
        assert!(TraceContext::from_meta(&json!({})).is_none());
        assert!(TraceContext::from_meta(&json!({ "tracestate": "a=1" })).is_none());
        // Non-string traceparent is treated as absent.
        assert!(TraceContext::from_meta(&json!({ "traceparent": 42 })).is_none());
        // Arbitrary non-object values never panic and yield None.
        assert!(TraceContext::from_meta(&json!("just a string")).is_none());
        assert!(TraceContext::from_meta(&json!([1, 2, 3])).is_none());
        assert!(TraceContext::from_meta(&json!(null)).is_none());
    }

    #[test]
    fn trace_context_over_bound_traceparent_yields_none() {
        let huge = "a".repeat(MAX_TRACE_VALUE_LEN + 1);
        let meta = json!({ "traceparent": huge });
        assert!(TraceContext::from_meta(&meta).is_none());
    }

    #[test]
    fn trace_context_over_bound_tracestate_and_baggage_are_dropped() {
        let huge = "b".repeat(MAX_TRACE_VALUE_LEN + 1);
        let meta = json!({
            "traceparent": "00-abc-def-01",
            "tracestate": huge,
            "baggage": huge,
        });
        let tc = TraceContext::from_meta(&meta).expect("in-bounds traceparent => Some");
        assert_eq!(tc.traceparent, "00-abc-def-01");
        // The oversized values are not surfaced.
        assert!(tc.tracestate.is_none());
        assert!(tc.baggage.is_none());
    }

    proptest::proptest! {
        /// `from_meta` parses UNTRUSTED `_meta` JSON: it must never panic, must
        /// return `None` when `traceparent` is absent, must round-trip an
        /// in-bounds `traceparent` exactly, and must never surface a field over
        /// the bound (threat T-112-09).
        #[test]
        fn from_meta_holds_invariants_over_arbitrary_meta(
            has_traceparent in proptest::prelude::any::<bool>(),
            traceparent in ".*",
            tracestate in proptest::option::of(".*"),
            baggage in proptest::option::of(".*"),
        ) {
            let mut map = serde_json::Map::new();
            if has_traceparent {
                map.insert("traceparent".into(), serde_json::Value::String(traceparent.clone()));
            }
            if let Some(ref ts) = tracestate {
                map.insert("tracestate".into(), serde_json::Value::String(ts.clone()));
            }
            if let Some(ref bg) = baggage {
                map.insert("baggage".into(), serde_json::Value::String(bg.clone()));
            }
            let value = serde_json::Value::Object(map);

            // (a) never panics
            let result = TraceContext::from_meta(&value);

            if !has_traceparent {
                // (b) absent traceparent => None
                proptest::prop_assert!(result.is_none());
            } else if traceparent.len() <= MAX_TRACE_VALUE_LEN {
                // (c) in-bounds traceparent present => Some carrying it exactly
                let tc = result.expect("in-bounds traceparent present => Some");
                proptest::prop_assert_eq!(&tc.traceparent, &traceparent);
                // (d) no surfaced field exceeds the bound
                proptest::prop_assert!(tc.traceparent.len() <= MAX_TRACE_VALUE_LEN);
                if let Some(ref ts) = tc.tracestate {
                    proptest::prop_assert!(ts.len() <= MAX_TRACE_VALUE_LEN);
                }
                if let Some(ref bg) = tc.baggage {
                    proptest::prop_assert!(bg.len() <= MAX_TRACE_VALUE_LEN);
                }
            }
        }
    }
}
