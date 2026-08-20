//! Outbound server-to-client request dispatcher with response correlation.
//!
//! Plumbing foundation for the peer back-channel. Adapts the
//! `ElicitationManager` pattern (`mpsc::Sender<ServerRequest>` +
//! pending-oneshot `HashMap` + `tokio::time::timeout`) into a generalized
//! dispatcher that can fulfill any server-to-client RPC — CreateMessage,
//! ListRoots, or any future addition. Keyed by correlation id so a single
//! dispatcher multiplexes many in-flight requests.
//!
//! This module is non-wasm only; wasm targets do not run the legacy
//! `Server` transport loop this integrates with.
//!
//! # Scope
//!
//! - `dispatch(request)` enqueues onto an outbound mpsc channel and awaits
//!   the correlated response via a pending-oneshot map.
//! - `handle_response(correlation_id, value)` fulfills the matching
//!   oneshot. Unknown correlation ids return `INVALID_REQUEST` without
//!   crashing the server loop.
//! - `spawn_server_request_drain(transport, outbound_rx)` wraps each
//!   outbound pair into a `TransportMessage::Request` and forwards to the
//!   transport — consumed by `Server::run`.
//!
//! # Threat model
//!
//! - Correlation ids are generated server-side from an `AtomicU64`
//!   counter. An attacker cannot predict live counter state from inbound
//!   responses alone.
//! - Timeout branches always remove the pending entry to prevent map leak
//!   (`test_dispatcher_timeout_cleans_pending`).
//! - The custom `Debug` impl never prints pending correlation ids.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::error::{Error, ErrorCode, Result};
use crate::types::ServerRequest;

/// Default timeout for a server-to-client RPC (60 seconds).
///
/// Shorter than `ElicitationManager`'s 5-minute default because
/// sampling/`list_roots` tend to be short and synchronous from the
/// client's perspective.
pub const DEFAULT_DISPATCH_TIMEOUT: Duration = Duration::from_mins(1);

static DISPATCH_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Dispatches server-initiated requests to the client and correlates
/// their responses back to the awaiting caller.
///
/// Held inside `Server` (and optionally `ServerCore`) as
/// `Arc<ServerRequestDispatcher>`. The enclosing `Server`'s `run` loop
/// is responsible for:
///   - draining `outbound_rx` and serializing each
///     `(correlation_id, ServerRequest)` onto the transport as a
///     `TransportMessage::Request { id, request }`;
///   - routing `TransportMessage::Response` back through
///     `handle_response`.
pub struct ServerRequestDispatcher {
    /// Outbound side. Kept on the dispatcher for `dispatch` to push onto.
    outbound_tx: mpsc::Sender<(String, ServerRequest)>,
    /// Pending requests awaiting response, keyed by correlation id.
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<Value>>>>,
    /// Correlation id -> the OPAQUE owner token that minted it.
    ///
    /// Empty on the in-process `Server::run` path, which dispatches through
    /// [`ServerRequestDispatcher::dispatch`] and records no owner: one transport
    /// with one client needs no ownership because there is only ever one place a
    /// server-to-client request can go. A multiplexed transport does — see
    /// [`ServerRequestDispatcher::dispatch_owned`].
    ///
    /// Kept STRICTLY in step with `pending`: every site that removes a `pending`
    /// entry removes the matching `owners` entry, `handle_response` — the SUCCESS
    /// path — included. A removal written only into the failure paths leaks one
    /// entry per completed round trip, on normal traffic, with no misbehaving
    /// client required (T-118.1-10-08).
    owners: Arc<RwLock<HashMap<String, String>>>,
    /// Per-request timeout.
    timeout_duration: Duration,
}

impl ServerRequestDispatcher {
    /// Construct with a pre-built outbound channel.
    ///
    /// The caller (`Server::run`) owns the matching receiver and is
    /// responsible for the drain-to-transport task.
    pub fn new_with_channel(outbound_tx: mpsc::Sender<(String, ServerRequest)>) -> Self {
        Self {
            outbound_tx,
            pending: Arc::new(RwLock::new(HashMap::new())),
            owners: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: DEFAULT_DISPATCH_TIMEOUT,
        }
    }

    /// Override the default timeout. Builder form.
    #[must_use]
    pub fn with_timeout(mut self, timeout_duration: Duration) -> Self {
        self.timeout_duration = timeout_duration;
        self
    }

    /// Generate a fresh correlation id. Monotonic + server-local.
    fn next_correlation_id() -> String {
        let id = DISPATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("dispatch-{id}")
    }

    /// Dispatch a server-to-client request and await its correlated
    /// response.
    ///
    /// Returns the raw JSON `Value` response; callers deserialize to the
    /// appropriate result type (`CreateMessageResult`, `ListRootsResult`,
    /// etc.).
    pub async fn dispatch(&self, request: ServerRequest) -> Result<Value> {
        self.dispatch_with_owner(None, request).await
    }

    /// [`dispatch`](Self::dispatch) that RECORDS which caller minted the
    /// correlation id, so a multiplexed transport can route the outbound frame
    /// back to the session that asked for it and can refuse an inbound response
    /// presented by any other session.
    ///
    /// `owner` is an opaque token — the streamable-HTTP transport passes the v1
    /// session id — and is never interpreted here beyond equality.
    ///
    /// # The in-process path is unaffected
    ///
    /// `Server::run` dispatches through [`dispatch`](Self::dispatch), which
    /// records no owner, so [`owner_of`](Self::owner_of) answers `None` there and
    /// the stock loop behaves byte-for-byte as before. Both entry points share
    /// [`dispatch_with_owner`](Self::dispatch_with_owner) precisely so the two
    /// can never drift in their pending-map bookkeeping.
    // Why: the only non-test caller is the streamable-HTTP peer channel
    // (`streamable_http_server/peer_channel.rs`), which is itself
    // `#[cfg(feature = "streamable-http")]`. A build of this crate WITHOUT that
    // feature — `pmcp-tasks --no-default-features` in CI's Feature Flag
    // Verification, and the Era Matrix job — therefore has no caller at all, and
    // `-D warnings` turns the dead_code lint into a hard error.
    //
    // An `allow` rather than a `#[cfg(feature = "streamable-http")]` on the
    // method: the unit tests below exercise `dispatch_owned` under DEFAULT
    // features, and `default = ["logging", "v1-compat"]` does not include
    // `streamable-http`, so cfg-gating the method would delete it out from under
    // its own tests.
    #[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
    pub(crate) async fn dispatch_owned(
        &self,
        owner: &str,
        request: ServerRequest,
    ) -> Result<Value> {
        self.dispatch_with_owner(Some(owner), request).await
    }

    /// The one dispatch body. `owner` decides only whether an `owners` entry is
    /// recorded alongside the `pending` one.
    async fn dispatch_with_owner(
        &self,
        owner: Option<&str>,
        request: ServerRequest,
    ) -> Result<Value> {
        if self.outbound_tx.is_closed() {
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "ServerRequestDispatcher outbound channel closed",
            ));
        }

        let (tx, rx) = oneshot::channel::<Value>();
        let correlation_id = Self::next_correlation_id();
        self.pending
            .write()
            .await
            .insert(correlation_id.clone(), tx);
        // ORDERING IS LOAD-BEARING: the owner is recorded BEFORE the send, and
        // the drain pops the pair only AFTER the send, so an owner recorded here
        // is always visible to the drain that will look it up. Recorded after the
        // send it would race, and the drain would see an ownerless dispatch.
        if let Some(owner) = owner {
            self.owners
                .write()
                .await
                .insert(correlation_id.clone(), owner.to_string());
        }

        if let Err(e) = self
            .outbound_tx
            .send((correlation_id.clone(), request))
            .await
        {
            self.forget(&correlation_id).await;
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to enqueue server request: {e}"),
            ));
        }

        debug!("Dispatched server request: {}", correlation_id);

        match timeout(self.timeout_duration, rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                // oneshot sender dropped without sending — either a dropped
                // receiver or a deliberate `fail_pending` from the drain.
                self.forget(&correlation_id).await;
                Err(Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    "Dispatch oneshot channel closed",
                ))
            },
            Err(_) => {
                // Timeout — remove pending entry to prevent leak.
                self.forget(&correlation_id).await;
                Err(Error::protocol(
                    ErrorCode::REQUEST_TIMEOUT,
                    format!("Server request {correlation_id} timed out"),
                ))
            },
        }
    }

    /// Drop every trace of a correlation id from BOTH maps.
    ///
    /// The single removal primitive, so `pending` and `owners` cannot fall out of
    /// step. The two locks are always taken in this order and never held at the
    /// same time, so no caller can invert them into a deadlock.
    async fn forget(&self, correlation_id: &str) {
        self.pending.write().await.remove(correlation_id);
        self.owners.write().await.remove(correlation_id);
    }

    /// Which owner minted this correlation id, if any.
    ///
    /// The transport calls this TWICE per server-to-client round trip: once on
    /// the outbound drain, to resolve which session's stream the request belongs
    /// on, and once on the inbound response POST, to refuse a response presented
    /// by a session that never received the request (T-118.1-10-02).
    ///
    /// Answers `None` for the in-process `Server::run` path, which records no
    /// owners.
    #[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
    pub(crate) async fn owner_of(&self, correlation_id: &str) -> Option<String> {
        self.owners.read().await.get(correlation_id).cloned()
    }

    /// Abandon a pending correlation that can never be answered.
    ///
    /// Dropping the `oneshot::Sender` closes the channel, so the awaiting
    /// `dispatch_with_owner` returns its `INTERNAL_ERROR` arm IMMEDIATELY rather
    /// than stranding the caller until [`DEFAULT_DISPATCH_TIMEOUT`]. Used by the
    /// transport drain when an outbound request has no recorded owner or the
    /// owning session has no live stream to deliver it on.
    ///
    /// `reason` is logged next to the correlation id and NEVER next to the
    /// request body: an outbound `sampling/createMessage` carries conversation
    /// content.
    #[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
    pub(crate) async fn fail_pending(&self, correlation_id: &str, reason: &str) {
        let was_pending = self.pending.write().await.remove(correlation_id).is_some();
        self.owners.write().await.remove(correlation_id);
        if was_pending {
            warn!("Abandoned server request {}: {}", correlation_id, reason);
        }
    }

    /// Route a client response back to the awaiting `dispatch` caller.
    ///
    /// Called by `Server::run` when `TransportMessage::Response(...)`
    /// arrives. The `correlation_id` parameter must match the one the
    /// dispatcher assigned when sending the matching request. Unknown
    /// ids return `INVALID_REQUEST` without crashing the server loop.
    ///
    /// # This is where a SUCCESSFUL round trip releases its `owners` entry
    ///
    /// `dispatch_with_owner` returns `Ok(value)` WITHOUT removing anything on the
    /// happy path, because the removal already happened HERE — this is the
    /// function that pops `pending`. So the matching `owners` removal has to land
    /// here too. Written only into the failure arms it would leak one entry per
    /// successful server-to-client RPC, forever, on entirely normal traffic
    /// (T-118.1-10-08). `test_dispatcher_owners_cleared_after_success` is the
    /// fence for exactly that.
    ///
    /// Ownership is verified by the CALLER (the transport's inbound response
    /// path) via [`owner_of`](Self::owner_of) BEFORE this runs, so removing the
    /// owner here cannot race that check.
    pub async fn handle_response(&self, correlation_id: &str, response: Value) -> Result<()> {
        let mut pending = self.pending.write().await;
        if let Some(tx) = pending.remove(correlation_id) {
            // Released before taking the `owners` lock: the two are always taken
            // in the order pending -> owners and never held together.
            drop(pending);
            self.owners.write().await.remove(correlation_id);
            if tx.send(response).is_err() {
                warn!("Dispatch response receiver dropped: {}", correlation_id);
            }
            Ok(())
        } else {
            warn!(
                "Received response for unknown correlation: {}",
                correlation_id
            );
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Unknown correlation id: {correlation_id}"),
            ))
        }
    }

    /// Number of pending dispatches. Used by integration tests; no in-tree
    /// library call sites yet, hence the `dead_code` allow.
    #[allow(dead_code)]
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }

    /// Number of recorded correlation owners. The leak fence's instrument.
    #[cfg(test)]
    async fn owners_count(&self) -> usize {
        self.owners.read().await.len()
    }
}

impl std::fmt::Debug for ServerRequestDispatcher {
    /// CARDINALITY only, never contents: the existing rule is that this impl
    /// never prints a pending correlation id, and an owner token is a session id,
    /// which is a bearer-shaped value in its own right.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerRequestDispatcher")
            .field("timeout_duration", &self.timeout_duration)
            .field("outbound_tx_closed", &self.outbound_tx.is_closed())
            .field("owners_len", &self.owners.try_read().map(|m| m.len()).ok())
            .finish()
    }
}

/// Drain outbound server-to-client requests and forward them to the transport
/// actor as JSON-RPC `Request` frames.
///
/// Spawned by `Server::run` once per server lifetime. Since Phase 108 the drain
/// no longer holds a transport write-lock across a send — it forwards each frame
/// onto the actor's unbounded `send_tx`, so a server-initiated request can never
/// starve behind an in-flight `receive()`. Exits cleanly when the outbound
/// channel is closed (dispatcher dropped) or when the actor's send channel is
/// gone (actor shut down).
pub fn spawn_server_request_drain(
    send_tx: mpsc::UnboundedSender<crate::shared::TransportMessage>,
    mut outbound_rx: mpsc::Receiver<(String, ServerRequest)>,
) {
    tokio::spawn(async move {
        while let Some((correlation_id, server_request)) = outbound_rx.recv().await {
            let request = crate::types::Request::Server(Box::new(server_request));
            let id = crate::types::RequestId::from(correlation_id.clone());

            if send_tx
                .send(crate::shared::TransportMessage::Request { id, request })
                .is_err()
            {
                warn!(
                    "Failed to forward server request {}: actor send channel closed",
                    correlation_id
                );
                // The actor is gone; nothing more can be sent.
                break;
            }
        }
        debug!("Server-request drain task exited");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatcher_enqueues_on_outbound_channel() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher =
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(100));
        let dispatch_fut =
            tokio::spawn(async move { dispatcher.dispatch(ServerRequest::ListRoots).await });

        // Drain the outbound channel — same role as
        // Server::spawn_server_request_drain.
        let (correlation_id, req) = tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("recv deadline")
            .expect("channel closed unexpectedly");
        assert!(
            !correlation_id.is_empty(),
            "correlation id must be non-empty"
        );
        assert!(matches!(req, ServerRequest::ListRoots));
        // Let dispatch time out (we never fulfill); drain the spawned future.
        let _ = dispatch_fut.await;
    }

    #[tokio::test]
    async fn test_dispatcher_fulfills_on_handle_response() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
        );

        let dispatch_fut = {
            let d = dispatcher.clone();
            tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await })
        };

        let (correlation_id, _req) = rx.recv().await.expect("outbound must receive");
        let response = serde_json::json!({"roots": []});
        dispatcher
            .handle_response(&correlation_id, response.clone())
            .await
            .expect("handle_response must succeed");

        let result = dispatch_fut.await.unwrap().expect("dispatch must succeed");
        assert_eq!(result, response);
        assert_eq!(dispatcher.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_dispatcher_timeout_cleans_pending() {
        let (tx, mut _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher =
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40));
        let result = dispatcher.dispatch(ServerRequest::ListRoots).await;
        assert!(result.is_err(), "dispatch must timeout");
        assert_eq!(
            dispatcher.pending_count().await,
            0,
            "timeout must clean pending"
        );
    }

    // -----------------------------------------------------------------------
    // The `owners` map must not leak. TWO fences, not one: the timeout path and
    // the SUCCESS path remove from different functions, and a removal written
    // only into `dispatch_with_owner` passes the first and fails the second
    // (T-118.1-10-08).
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_dispatcher_owners_cleared_after_timeout() {
        let (tx, mut _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher =
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40));
        let result = dispatcher
            .dispatch_owned("session-a", ServerRequest::ListRoots)
            .await;
        assert!(result.is_err(), "dispatch_owned must timeout");
        assert_eq!(
            dispatcher.pending_count().await,
            0,
            "timeout must clean pending"
        );
        assert_eq!(
            dispatcher.owners_count().await,
            0,
            "timeout must clean owners"
        );
    }

    #[tokio::test]
    async fn test_dispatcher_owners_cleared_after_success() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
        );

        let dispatch_fut = {
            let d = dispatcher.clone();
            tokio::spawn(async move {
                d.dispatch_owned("session-a", ServerRequest::ListRoots)
                    .await
            })
        };

        let (correlation_id, _req) = rx.recv().await.expect("outbound must receive");
        assert_eq!(
            dispatcher.owner_of(&correlation_id).await.as_deref(),
            Some("session-a"),
            "the owner is visible to the drain BEFORE the response arrives"
        );
        dispatcher
            .handle_response(&correlation_id, serde_json::json!({"roots": []}))
            .await
            .expect("handle_response must succeed");
        dispatch_fut
            .await
            .unwrap()
            .expect("dispatch_owned must succeed");

        assert_eq!(
            dispatcher.pending_count().await,
            0,
            "success must clean pending"
        );
        // THE one that catches the normal-path leak: on success `dispatch_owned`
        // removes nothing — `handle_response` does — so a removal written only
        // into `dispatch_owned` leaves this entry behind forever.
        assert_eq!(
            dispatcher.owners_count().await,
            0,
            "success must clean owners — handle_response is the removal site"
        );
        assert_eq!(dispatcher.owner_of(&correlation_id).await, None);
    }

    #[tokio::test]
    async fn test_dispatch_records_no_owner() {
        // The in-process `Server::run` path: `dispatch` records nothing, so a
        // multiplexing transport's ownership check cannot be fooled by it and the
        // stock loop is byte-for-byte unchanged.
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40)),
        );
        let d = dispatcher.clone();
        let fut = tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await });
        let (correlation_id, _req) = rx.recv().await.expect("outbound must receive");
        assert_eq!(dispatcher.owner_of(&correlation_id).await, None);
        assert_eq!(dispatcher.owners_count().await, 0);
        let _ = fut.await;
    }

    #[tokio::test]
    async fn test_fail_pending_releases_the_caller_immediately() {
        // The drain's answer to "no owner" / "no live stream": the caller must
        // return AT ONCE rather than waiting out the dispatch timeout, which is
        // set generously here precisely so a slow return would be visible.
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(30)),
        );
        let d = dispatcher.clone();
        let fut = tokio::spawn(async move {
            d.dispatch_owned("session-a", ServerRequest::ListRoots)
                .await
        });
        let (correlation_id, _req) = rx.recv().await.expect("outbound must receive");
        dispatcher
            .fail_pending(&correlation_id, "no live stream for the owning session")
            .await;

        let result = tokio::time::timeout(Duration::from_secs(2), fut)
            .await
            .expect("the caller must be released without waiting out the dispatch timeout")
            .expect("task joins");
        assert!(result.is_err(), "an abandoned correlation is an error");
        assert_eq!(dispatcher.pending_count().await, 0);
        assert_eq!(dispatcher.owners_count().await, 0);
    }

    #[tokio::test]
    async fn test_dispatcher_debug_does_not_leak_owner_tokens() {
        let (tx, mut rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(5)),
        );
        let d = dispatcher.clone();
        let _fut = tokio::spawn(async move {
            d.dispatch_owned("secret-session", ServerRequest::ListRoots)
                .await
        });
        let _ = rx.recv().await.expect("outbound must receive");

        let debug_str = format!("{:?}", dispatcher);
        assert!(
            !debug_str.contains("secret-session"),
            "debug must not leak the owner token: {debug_str}"
        );
        assert!(debug_str.contains("owners_len"));
    }

    #[tokio::test]
    async fn test_dispatcher_handle_response_unknown_id_returns_err() {
        let (tx, _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = ServerRequestDispatcher::new_with_channel(tx);
        let result = dispatcher
            .handle_response("does-not-exist", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dispatcher_debug_does_not_leak_correlation_ids() {
        let (tx, _rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(5)),
        );
        // Kick off a dispatch to populate a pending entry.
        let d = dispatcher.clone();
        let _fut = tokio::spawn(async move { d.dispatch(ServerRequest::ListRoots).await });
        // Give the task a moment to insert into `pending`.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let debug_str = format!("{:?}", dispatcher);
        assert!(
            !debug_str.contains("dispatch-"),
            "debug must not leak correlation id: {debug_str}"
        );
        assert!(debug_str.contains("ServerRequestDispatcher"));
    }
}
