//! Peer back-channel trait for server-to-client RPCs from inside request handlers.
//!
//! Implementations route outbound RPCs (`sampling/createMessage`, `roots/list`,
//! `elicitation/create`, `notifications/progress`) to the client that originated
//! the current request. The trait is object-safe so
//! [`crate::RequestHandlerExtra`] can hold `Option<Arc<dyn PeerHandle>>`.
//!
//! This module is non-wasm only; on wasm32 targets the server dispatch path does
//! not carry a peer, and handlers should treat `extra.peer()` returning `None`
//! as the normal case.
//!
//! # Session isolation
//!
//! Peer handles are constructed fresh-per-request by the dispatch site. Each
//! `Server` instance owns its own dispatcher (and therefore its own set of
//! peer handles) bound to its own transport. Cross-session confusion requires
//! cross-process access, which is out of threat model.
//!
//! The claim covers EVERY method on the trait, [`PeerHandle::elicit`](crate::shared::peer::PeerHandle::elicit) included,
//! and for the same reason: one transport, one dispatcher, one correlation
//! authority.
//!
//! ## Forward dependency: multi-session HTTP (Phase 118.1 plan 10)
//!
//! The paragraph above is trivially true today only because every transport that
//! carries a peer handle is single-session — a `Server::run` loop owns exactly
//! one transport. Plan 10 puts this same handle on `StreamableHttpServer`, where
//! ONE server process multiplexes MANY concurrent client sessions. There, "each
//! `Server` owns its own dispatcher" stops being sufficient on its own: the
//! dispatcher must ALSO route each answer back to the session that asked, and a
//! peer handle constructed for session A must never resolve against a response
//! delivered by session B. That is a real per-session routing requirement, not a
//! restatement of the sentence above. It is recorded here by name so plan 10
//! cannot land the transport without confronting it.
//!
//! # Authorization
//!
//! Peer calls inherit the originating tool's authorization context. Tool-level
//! authz runs BEFORE the dispatch site wires `peer` — an unauthorized caller
//! never reaches the handler body and therefore never sees `extra.peer()`. That
//! ordering is a property of the DISPATCH SITE rather than of any individual
//! method, so it covers [`PeerHandle::elicit`](crate::shared::peer::PeerHandle::elicit) exactly as it covers
//! [`PeerHandle::sample`](crate::shared::peer::PeerHandle::sample): a refused caller never reaches a handler body and so
//! never obtains a handle to elicit with.

#![cfg(not(target_arch = "wasm32"))]

use crate::error::{Error, ErrorCode, Result};
use crate::server::roots::ListRootsResult;
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use crate::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools,
};
use crate::types::ProgressToken;
use async_trait::async_trait;

/// Server-to-client back-channel accessible from inside request handlers.
///
/// Implementations delegate outbound RPCs to the client session that
/// originated the current inbound request. The trait is object-safe so
/// [`crate::RequestHandlerExtra`] can hold `Option<Arc<dyn PeerHandle>>`.
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::PeerHandle;
/// use std::sync::Arc;
/// # async fn demo(peer: Arc<dyn PeerHandle>) -> pmcp::Result<()> {
/// let _roots = peer.list_roots().await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait PeerHandle: Send + Sync {
    /// Request the client to sample its LLM (`sampling/createMessage`).
    ///
    /// Delegates through the enclosing `Server`'s outbound request
    /// dispatcher. The response is deserialized into the typed
    /// [`CreateMessageResult`]; malformed responses surface as a protocol
    /// error (`INTERNAL_ERROR`).
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult>;

    /// Request the client to sample its LLM with tool support
    /// (`sampling/createMessage`, MCP 2025-11-25).
    ///
    /// Returns a [`CreateMessageResultWithTools`], whose `content` is an array
    /// that can carry `tool_use` / `tool_result` blocks — unlike the
    /// single-`Content` [`CreateMessageResult`] from [`PeerHandle::sample`](crate::shared::peer::PeerHandle::sample).
    ///
    /// This is an ADDITIVE trait method with a default body that delegates to
    /// [`PeerHandle::sample`](crate::shared::peer::PeerHandle::sample) and lifts the single result into the `WithTools`
    /// shape (via [`CreateMessageResultWithTools::from_single`]). Existing
    /// `PeerHandle` implementors therefore keep compiling unchanged; the
    /// dispatch-backed [`crate::server::peer_impl::DispatchPeerHandle`] overrides
    /// it to decode a real `CreateMessageResultWithTools` (with a legacy
    /// single-content fallback).
    async fn sample_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        let legacy = self.sample(params).await?;
        Ok(CreateMessageResultWithTools::from_single(legacy))
    }

    /// Request the client's root list (`roots/list`).
    ///
    /// Delegates through the enclosing `Server`'s outbound request
    /// dispatcher. The response is deserialized into the typed
    /// [`ListRootsResult`].
    async fn list_roots(&self) -> Result<ListRootsResult>;

    /// Ask the client to collect input from its user (`elicitation/create`).
    ///
    /// This is an ADDITIVE trait method with a default body — the same semver
    /// mechanism [`PeerHandle::sample_with_tools`] uses. Existing `PeerHandle`
    /// implementors therefore keep compiling unchanged, so shipping it is a
    /// MINOR release; the dispatch-backed
    /// [`DispatchPeerHandle`](crate::server::peer_impl::DispatchPeerHandle)
    /// overrides it to dispatch `ServerRequest::ElicitationCreate` through the
    /// shared outbound dispatcher and decode the client's [`ElicitResult`].
    ///
    /// # The default body FAILS, deliberately
    ///
    /// Unlike `sample_with_tools`, `elicit` has no sensible fallback: no other
    /// method's answer can be lifted into an [`ElicitResult`]. The default
    /// therefore returns [`ErrorCode::METHOD_NOT_FOUND`] naming the missing
    /// capability instead of synthesizing a result. A default that returned
    /// `Ok` — with ANY
    /// [`ElicitAction`](crate::types::elicitation::ElicitAction), including a
    /// bare `Decline` — would let a server record a decision no human ever made.
    /// That is a correctness and a security failure rather than a convenience:
    /// elicitation exists precisely to put a person in the loop, so a silent
    /// success forges the one signal the method is for. Failing loudly makes an
    /// unimplemented back-channel impossible to mistake for a real user's answer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams};
    /// use pmcp::PeerHandle;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// # async fn demo(peer: Arc<dyn PeerHandle>) -> pmcp::Result<()> {
    /// let answer = peer
    ///     .elicit(ElicitRequestParams::Form {
    ///         message: "Which environment should I deploy to?".to_string(),
    ///         requested_schema: json!({
    ///             "type": "object",
    ///             "properties": { "env": { "type": "string" } },
    ///             "required": ["env"],
    ///         }),
    ///     })
    ///     .await?;
    ///
    /// // Only `Accept` carries content; `Decline` and `Cancel` are NOT approval.
    /// match answer.action {
    ///     ElicitAction::Accept => {
    ///         let env = answer
    ///             .content
    ///             .as_ref()
    ///             .and_then(|c| c.get("env"))
    ///             .and_then(|v| v.as_str())
    ///             .unwrap_or("staging");
    ///         println!("deploying to {env}");
    ///     },
    ///     ElicitAction::Decline | ElicitAction::Cancel => println!("the user said no"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn elicit(&self, params: ElicitRequestParams) -> Result<ElicitResult> {
        let _ = params;
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "elicitation/create is unsupported by this peer handle: \
             PeerHandle::elicit was never implemented, so no user was asked",
        ))
    }

    /// Send a progress notification (`notifications/progress`).
    ///
    /// Best-effort: returns `Ok(())` silently when no progress channel is
    /// configured — matches the existing
    /// [`crate::RequestHandlerExtra::report_progress`] no-op guard. This
    /// phase does NOT attempt to surface transport errors on the progress
    /// path; a follow-on phase may plumb `notification_tx` through the peer
    /// implementation for live progress reporting.
    async fn progress_notify(
        &self,
        token: ProgressToken,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    /// A minimal EXTERNAL-style implementor: it provides ONLY the methods with
    /// no default body, exactly as a downstream crate written before `elicit`
    /// existed does. That this compiles at all is the semver proof (an additive
    /// default body is MINOR); what it DOES on `elicit` is the security proof.
    struct BarePeer;

    #[async_trait]
    impl PeerHandle for BarePeer {
        async fn sample(&self, _params: CreateMessageParams) -> Result<CreateMessageResult> {
            Err(Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                "no sampling here",
            ))
        }

        async fn list_roots(&self) -> Result<ListRootsResult> {
            Ok(ListRootsResult { roots: Vec::new() })
        }

        async fn progress_notify(
            &self,
            _token: ProgressToken,
            _progress: f64,
            _total: Option<f64>,
            _message: Option<String>,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn form_params() -> ElicitRequestParams {
        ElicitRequestParams::Form {
            message: "approve?".to_string(),
            requested_schema: json!({ "type": "object" }),
        }
    }

    /// T-118.1-09-01: the inherited default must FAIL LOUDLY. An `Ok` here — with
    /// any action, `Decline` included — would let a server record a decision no
    /// human ever made.
    #[tokio::test]
    async fn bare_implementor_elicit_is_err_not_ok() {
        let result = BarePeer.elicit(form_params()).await;

        let error = match result {
            Err(error) => error,
            Ok(answer) => panic!(
                "the default elicit body must NOT synthesize a user decision, got action {:?}",
                answer.action
            ),
        };

        match error {
            Error::Protocol {
                code, ref message, ..
            } => {
                assert_eq!(code, ErrorCode::METHOD_NOT_FOUND);
                assert!(
                    message.contains("elicitation/create"),
                    "the error must name the missing capability, got: {message}"
                );
            },
            other => panic!("expected a protocol error, got: {other:?}"),
        }
    }

    /// The same guard through the ERASED form the dispatch site actually stores
    /// (`Option<Arc<dyn PeerHandle>>`): a default reachable only via the concrete
    /// type would prove nothing about production call sites.
    #[tokio::test]
    async fn bare_implementor_elicit_is_err_through_dyn_peer_handle() {
        let peer: Arc<dyn PeerHandle> = Arc::new(BarePeer);
        assert!(
            peer.elicit(form_params()).await.is_err(),
            "the loud default must survive erasure to `dyn PeerHandle`"
        );
    }
}
