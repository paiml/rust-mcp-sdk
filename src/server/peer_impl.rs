//! Concrete [`PeerHandle`] implementation that delegates to the
//! [`ServerRequestDispatcher`].
//!
//! `DispatchPeerHandle` does NOT own a channel. It holds an
//! `Arc<ServerRequestDispatcher>` and delegates every outbound RPC to
//! `dispatcher.dispatch(...)`. The dispatcher owns the correlation layer
//! (pending oneshot map keyed by correlation id) and the drain-to-transport
//! task. This avoids the anti-pattern of ad-hoc per-site channel
//! construction: every peer handle shares the single correlation authority.
//!
//! That rule binds every method here, `elicit` (`elicitation/create`) included —
//! and it binds hardest there, because the payload correlated back is a person's
//! answer. A second registry could resolve one client's approval against another
//! client's prompt.
//!
//! Deserialization: the dispatcher returns `serde_json::Value`; the
//! `DispatchPeerHandle` parses into the typed result and surfaces malformed
//! responses as a protocol `INTERNAL_ERROR`.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{Error, ErrorCode, Result};
use crate::server::roots::ListRootsResult;
use crate::server::server_request_dispatcher::ServerRequestDispatcher;
use crate::shared::peer::PeerHandle;
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use crate::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools,
};
use crate::types::{ProgressToken, ServerRequest};
use serde::Deserialize;

/// [`PeerHandle`] that delegates outbound RPCs to a shared
/// [`ServerRequestDispatcher`].
///
/// Constructed fresh-per-request at each `ServerCore` dispatch site when
/// the enclosing `ServerCore` was built with
/// [`crate::server::core::ServerCore::with_server_request_dispatcher`].
/// The construction is near-zero-cost — the struct is a single `Arc`
/// clone — so per-request allocation is not a concern.
#[derive(Debug)]
pub struct DispatchPeerHandle {
    dispatcher: Arc<ServerRequestDispatcher>,
}

impl DispatchPeerHandle {
    /// Build a peer handle around a shared dispatcher.
    ///
    /// Pub (not `pub(crate)`) so the `#[doc(hidden)] __test_support`
    /// re-export in `src/lib.rs` can link from integration tests; the
    /// enclosing `peer_impl` module is `pub(crate)`, so this stays
    /// internal from a doc/discoverability standpoint.
    pub fn new(dispatcher: Arc<ServerRequestDispatcher>) -> Self {
        Self { dispatcher }
    }
}

#[async_trait]
impl PeerHandle for DispatchPeerHandle {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        let value = self
            .dispatcher
            .dispatch(ServerRequest::CreateMessage(Box::new(params)))
            .await?;
        serde_json::from_value::<CreateMessageResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid sample response: {e}"),
            )
        })
    }

    async fn sample_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        // Dispatches the SAME `sampling/createMessage` request as `sample`; the
        // hosting client answers with either a `CreateMessageResultWithTools`
        // (tool-aware host) or a legacy `CreateMessageResult` (older host). We
        // decode the WithTools shape first, then fall back to decoding the
        // legacy single-content shape and lifting it — so an older client can
        // never crash the tool call (Gemini legacy-decode fallback).
        let value = self
            .dispatcher
            .dispatch(ServerRequest::CreateMessage(Box::new(params)))
            .await?;
        // Borrowing deserializer: the strict shape is TRIED first, so a clone
        // here would copy the whole completion payload on every call just to
        // fall back on the legacy shape. `&Value` leaves `value` owned for the
        // fallback below.
        if let Ok(with_tools) = CreateMessageResultWithTools::deserialize(&value) {
            return Ok(with_tools);
        }
        let legacy = serde_json::from_value::<CreateMessageResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid sample_with_tools response: {e}"),
            )
        })?;
        Ok(CreateMessageResultWithTools::from_single(legacy))
    }

    async fn list_roots(&self) -> Result<ListRootsResult> {
        let value = self.dispatcher.dispatch(ServerRequest::ListRoots).await?;
        serde_json::from_value::<ListRootsResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid list_roots response: {e}"),
            )
        })
    }

    async fn elicit(&self, params: ElicitRequestParams) -> Result<ElicitResult> {
        // Same shape as `list_roots`, and deliberately so: dispatch through the
        // SHARED `Arc<ServerRequestDispatcher>`, then decode. No channel and no
        // pending map are constructed here — the module doc names ad-hoc
        // per-site correlation as the anti-pattern this type exists to avoid,
        // and for `elicit` the stakes are a user's answer being matched to the
        // wrong request.
        let value = self
            .dispatcher
            .dispatch(ServerRequest::ElicitationCreate(Box::new(params)))
            .await?;
        serde_json::from_value::<ElicitResult>(value).map_err(|e| {
            // A malformed answer is peer-supplied JSON, so it must FAIL rather
            // than degrade to a default: a synthesized action would report a
            // decision the user never made.
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid elicit response: {e}"),
            )
        })
    }

    /// A no-op on THIS handle, and the reason is structural rather than pending.
    ///
    /// `DispatchPeerHandle` holds only the request/response correlation
    /// authority. Progress is a NOTIFICATION — one-way, uncorrelated — and its
    /// vehicle on the in-process path is `Server::notification_tx`, which is
    /// assigned inside `Server::run()` and never handed to this type. Plumbing
    /// it here would duplicate a channel the dispatch site already owns: the
    /// in-process `Server` builds a `ServerProgressReporter` from that same
    /// `notification_tx` and puts it on `RequestHandlerExtra`, so a handler
    /// calling `extra.report_progress(..)` on the in-process path already emits.
    ///
    /// # What DOES emit, and where
    ///
    /// | path | `extra.report_progress(..)` | `peer.progress_notify(..)` |
    /// | ---- | --------------------------- | -------------------------- |
    /// | in-process `Server::run` | emits via `notification_tx` | no-op (this impl) |
    /// | `StreamableHTTP` v1 session | emits via the transport's session sink | emits via the same sink (`SessionPeerHandle`) |
    /// | `StreamableHTTP` v2 | no reporter, silent `Ok(())` | no sink, silent `Ok(())` |
    ///
    /// The v2 column is plan 12's work: its vehicle is a multi-frame SSE POST
    /// response body, not a session stream, so it cannot reuse the v1 sink.
    ///
    /// Returning `Ok(())` is deliberate and must not change: callers treat
    /// progress as infallible, matching `RequestHandlerExtra::report_progress`'s
    /// own `None`-reporter guard.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn build_dispatcher_with_short_timeout() -> (
        Arc<ServerRequestDispatcher>,
        mpsc::Receiver<(String, ServerRequest)>,
    ) {
        let (tx, rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_millis(40)),
        );
        (dispatcher, rx)
    }

    #[tokio::test]
    async fn test_peer_handle_trait_shape() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer: Arc<dyn PeerHandle> = Arc::new(DispatchPeerHandle::new(dispatcher));
        // Trait-shape smoke: casts to Arc<dyn PeerHandle>. The Arc itself
        // can be cloned and stored — no ?Sized errors.
        let _clone = peer.clone();
    }

    #[tokio::test]
    async fn test_peer_progress_notify_always_ok() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer = DispatchPeerHandle::new(dispatcher);
        let result = peer
            .progress_notify(
                ProgressToken::String("tok-1".to_string()),
                0.5,
                Some(1.0),
                None,
            )
            .await;
        assert!(
            result.is_ok(),
            "progress_notify is infallible on this handle: the in-process path emits through \
             `RequestHandlerExtra::report_progress`, not through the peer"
        );
    }

    #[tokio::test]
    async fn test_peer_sample_propagates_dispatcher_timeout() {
        let (dispatcher, _rx) = build_dispatcher_with_short_timeout();
        let peer = DispatchPeerHandle::new(dispatcher);
        // Use REAL constructor — CreateMessageParams has no Default impl.
        let params = CreateMessageParams::new(Vec::new());
        let start = std::time::Instant::now();
        let result = peer.sample(params).await;
        let elapsed = start.elapsed();
        assert!(
            result.is_err(),
            "sample must return Err when dispatcher times out"
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "timeout must fire within 500ms (was {:?})",
            elapsed
        );
    }

    fn build_dispatcher_with_long_timeout() -> (
        Arc<ServerRequestDispatcher>,
        mpsc::Receiver<(String, ServerRequest)>,
    ) {
        let (tx, rx) = mpsc::channel::<(String, ServerRequest)>(4);
        let dispatcher = Arc::new(
            ServerRequestDispatcher::new_with_channel(tx).with_timeout(Duration::from_secs(2)),
        );
        (dispatcher, rx)
    }

    #[tokio::test]
    async fn test_sample_with_tools_decodes_with_tools_response() {
        let (dispatcher, mut rx) = build_dispatcher_with_long_timeout();
        let peer = DispatchPeerHandle::new(dispatcher.clone());

        let fut = tokio::spawn(async move {
            peer.sample_with_tools(CreateMessageParams::new(Vec::new()))
                .await
        });

        let (cid, _req) = rx.recv().await.expect("outbound dispatch");
        // A tool-aware client answers with a CreateMessageResultWithTools that
        // carries a tool_use block.
        let response = serde_json::json!({
            "model": "host-model",
            "role": "assistant",
            "content": [
                { "type": "tool_use", "name": "search", "id": "call-1", "input": {"q": "rust"} }
            ]
        });
        dispatcher
            .handle_response(&cid, response)
            .await
            .expect("handle_response");

        let result = fut.await.unwrap().expect("sample_with_tools succeeds");
        assert_eq!(result.model, "host-model");
        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            crate::types::sampling::SamplingMessageContent::ToolUse { name, id, .. } => {
                assert_eq!(name, "search");
                assert_eq!(id, "call-1");
            },
            other => panic!("tool_use block must survive decode, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // D-07: `elicit` over the SHARED dispatcher.
    //
    // These drive the SAME `ServerRequestDispatcher` harness the sampling cases
    // use — deliberately, because the threat here (T-118.1-09-03) is a SECOND
    // correlation authority: a peer method that built its own channel or pending
    // map could resolve one client's answer against another client's request.
    // If `elicit` ever stops riding this dispatcher, the `rx.recv()` in these
    // tests goes silent and they fail.
    //
    // Every await is timeout-bounded: this whole cluster is about a back-channel
    // that can park a tool handler, and an unbounded await in the test for a
    // deadlock in the code just moves the hang into CI.
    // -----------------------------------------------------------------------

    /// Bound for awaits that must resolve promptly; generous enough not to be
    /// load-flaky, short enough that a hang fails rather than stalls the suite.
    const AWAIT_BOUND: Duration = Duration::from_secs(5);

    fn elicit_params() -> crate::types::elicitation::ElicitRequestParams {
        crate::types::elicitation::ElicitRequestParams::Form {
            message: "approve the deploy?".to_string(),
            requested_schema: serde_json::json!({
                "type": "object",
                "properties": { "approved": { "type": "boolean" } },
                "required": ["approved"],
            }),
        }
    }

    /// Receive the next outbound dispatch, asserting it is an
    /// `elicitation/create` and that it arrived on the SHARED dispatcher channel.
    async fn recv_elicitation(
        rx: &mut mpsc::Receiver<(String, ServerRequest)>,
    ) -> (String, Box<crate::types::elicitation::ElicitRequestParams>) {
        let (cid, req) = tokio::time::timeout(AWAIT_BOUND, rx.recv())
            .await
            .expect("the outbound elicitation must reach the shared dispatcher, not hang")
            .expect("dispatcher channel stays open");
        match req {
            ServerRequest::ElicitationCreate(params) => (cid, params),
            other => panic!("elicit must dispatch ServerRequest::ElicitationCreate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_elicit_decodes_answered_response() {
        use crate::types::elicitation::ElicitAction;

        let (dispatcher, mut rx) = build_dispatcher_with_long_timeout();
        let peer = DispatchPeerHandle::new(dispatcher.clone());

        let fut = tokio::spawn(async move { peer.elicit(elicit_params()).await });

        let (cid, params) = recv_elicitation(&mut rx).await;
        match *params {
            crate::types::elicitation::ElicitRequestParams::Form { ref message, .. } => {
                assert_eq!(message, "approve the deploy?", "the params must ride along");
            },
            crate::types::elicitation::ElicitRequestParams::Url { ref url, .. } => {
                panic!("form params must survive the dispatch, got a url elicitation for {url}")
            },
        }

        dispatcher
            .handle_response(
                &cid,
                serde_json::json!({
                    "action": "accept",
                    "content": { "approved": true }
                }),
            )
            .await
            .expect("handle_response");

        let result = tokio::time::timeout(AWAIT_BOUND, fut)
            .await
            .expect("elicit must not hang once answered")
            .unwrap()
            .expect("elicit succeeds on a well-formed answer");

        assert_eq!(result.action, ElicitAction::Accept);
        assert_eq!(
            result
                .content
                .as_ref()
                .and_then(|c| c.get("approved"))
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "the accepted form content must decode"
        );
    }

    #[tokio::test]
    async fn test_elicit_malformed_answer_is_internal_error() {
        let (dispatcher, mut rx) = build_dispatcher_with_long_timeout();
        let peer = DispatchPeerHandle::new(dispatcher.clone());

        let fut = tokio::spawn(async move { peer.elicit(elicit_params()).await });

        let (cid, _params) = recv_elicitation(&mut rx).await;
        // A client answering with a non-spec `action` is peer-supplied garbage:
        // it must surface as a protocol error, never as a silent Accept.
        dispatcher
            .handle_response(
                &cid,
                serde_json::json!({ "action": "definitely-not-an-action" }),
            )
            .await
            .expect("handle_response");

        let error = tokio::time::timeout(AWAIT_BOUND, fut)
            .await
            .expect("elicit must not hang on a malformed answer")
            .unwrap()
            .expect_err("a malformed answer must NOT decode into a user decision");

        match error {
            Error::Protocol { code, .. } => {
                assert_eq!(
                    code,
                    ErrorCode::INTERNAL_ERROR,
                    "malformed peer answers map to INTERNAL_ERROR, matching list_roots"
                );
            },
            other => panic!("expected a protocol error, got: {other:?}"),
        }
    }

    /// T-118.1-09-02: an UNANSWERED elicitation must error on the dispatcher's
    /// own timeout rather than parking the handler forever. The dispatch is
    /// asserted to have gone out first, so this cannot pass by never trying.
    #[tokio::test]
    async fn test_elicit_unanswered_hits_the_dispatcher_timeout() {
        let (dispatcher, mut rx) = build_dispatcher_with_short_timeout();
        let peer = DispatchPeerHandle::new(dispatcher);

        let fut = tokio::spawn(async move { peer.elicit(elicit_params()).await });

        // The request went out on the shared channel...
        let (_cid, _params) = recv_elicitation(&mut rx).await;

        // ...and is never answered, so the dispatcher's timeout must fire.
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(AWAIT_BOUND, fut)
            .await
            .expect("an unanswered elicit must TIME OUT, not hang")
            .unwrap();
        assert!(
            result.is_err(),
            "an unanswered elicitation must return Err, never a synthesized answer"
        );
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "the dispatcher timeout must fire promptly (was {:?})",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn test_sample_with_tools_falls_back_to_legacy_result() {
        let (dispatcher, mut rx) = build_dispatcher_with_long_timeout();
        let peer = DispatchPeerHandle::new(dispatcher.clone());

        let fut = tokio::spawn(async move {
            peer.sample_with_tools(CreateMessageParams::new(Vec::new()))
                .await
        });

        let (cid, _req) = rx.recv().await.expect("outbound dispatch");
        // An OLDER client answers with a legacy single-content CreateMessageResult
        // (content is an object, no `role`) — must NOT crash the tool call.
        let response = serde_json::json!({
            "content": { "type": "text", "text": "legacy answer" },
            "model": "old-model",
            "stopReason": "endTurn"
        });
        dispatcher
            .handle_response(&cid, response)
            .await
            .expect("handle_response");

        let result = fut.await.unwrap().expect("legacy fallback succeeds");
        assert_eq!(result.model, "old-model");
        assert_eq!(result.stop_reason.as_deref(), Some("endTurn"));
        assert_eq!(
            result.content.len(),
            1,
            "single content lifts to one element"
        );
        match &result.content[0] {
            crate::types::sampling::SamplingMessageContent::Text { text, .. } => {
                assert_eq!(text, "legacy answer");
            },
            other => panic!("legacy content must lift to Text, got {other:?}"),
        }
    }
}
