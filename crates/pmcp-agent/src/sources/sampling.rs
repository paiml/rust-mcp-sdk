//! [`SamplingSource`] — a zero-dependency [`CompletionSource`] that drives the
//! model over the agent's **server-side peer** (AGNT-04).
//!
//! When a `pmcp-agent` loop runs *inside* a `tools/call` handler of a hosted
//! MCP server, the handler receives a request-scoped
//! [`PeerHandle`](pmcp::PeerHandle) via
//! [`RequestHandlerExtra::peer`](pmcp::RequestHandlerExtra::peer). That peer
//! speaks spec sampling back to the hosting client. `SamplingSource` wraps that
//! handle and implements [`CompletionSource`] by delegating to
//! [`PeerHandle::sample_with_tools`], so the completion (including any
//! `tool_use` blocks the host chooses) flows through the standard MCP sampling
//! surface with **no extra dependencies** — no reqwest, no provider SDK.
//!
//! This rides the Phase 108-01 Transport Actor fix (D-106-A): an in-tool
//! `peer.sample_with_tools()` now completes on the stock `Server::run` loop.
//! Host-side approval (Phase 106 `PreflightApproval`) is enforced by the
//! hosting client, not reimplemented here (T-108-04-06: accept).

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};
use pmcp::PeerHandle;

use crate::seams::{CompletionError, CompletionSource};

/// A [`CompletionSource`] backed by the server-side peer's sampling surface.
///
/// Holds an `Arc<dyn PeerHandle>` (the request-scoped peer from
/// [`RequestHandlerExtra::peer`](pmcp::RequestHandlerExtra::peer)) and forwards
/// [`create_message`](CompletionSource::create_message) to
/// [`PeerHandle::sample_with_tools`]. Zero new dependencies.
///
/// The peer is request-scoped, so the `SamplingSource` is typically constructed
/// per tool invocation (the request-scoped factory that mints one per call
/// lands in the 108-06 adapter).
#[derive(Clone)]
pub struct SamplingSource {
    peer: Arc<dyn PeerHandle>,
}

impl SamplingSource {
    /// Wrap a peer handle (e.g. `extra.peer().expect(..).clone()`).
    #[must_use]
    pub fn new(peer: Arc<dyn PeerHandle>) -> Self {
        Self { peer }
    }
}

impl std::fmt::Debug for SamplingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplingSource").finish_non_exhaustive()
    }
}

/// Map a `pmcp::Error` from the peer round-trip into a [`CompletionError`].
///
/// Transient failures (transport, timeout, cancellation, protocol round-trip
/// faults, circuit breaker) map to a non-`Fatal` class so the loop can retry;
/// serialization/decode faults are `Fatal`; auth is `Auth`. No secret material
/// is ever echoed — only the coarse category and the error's own message
/// (which the SDK does not populate with keys).
fn map_peer_error(err: &pmcp::Error) -> CompletionError {
    use pmcp::Error;
    match err {
        Error::Authentication(_) => CompletionError::Auth,
        Error::RateLimited => CompletionError::Capacity("rate limited".to_string()),
        Error::Serialization(e) => CompletionError::Decode(e.to_string()),
        Error::Timeout(_)
        | Error::Transport(_)
        | Error::Cancelled
        | Error::CircuitBreakerOpen
        | Error::Protocol { .. } => CompletionError::Transport(err.to_string()),
        other => CompletionError::Transport(other.to_string()),
    }
}

#[async_trait]
impl CompletionSource for SamplingSource {
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        self.peer
            .sample_with_tools(params)
            .await
            .map_err(|e| map_peer_error(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::{map_peer_error, CompletionError, CompletionSource, SamplingSource};
    use crate::seams::RetryClass;
    use async_trait::async_trait;
    use pmcp::types::roots::ListRootsResult;
    use pmcp::types::sampling::{
        CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools, SamplingMessage,
        SamplingMessageContent,
    };
    use pmcp::types::{Content, ProgressToken, Role};
    use pmcp::{PeerHandle, Result};
    use std::sync::Arc;

    /// A stub peer that answers `sample_with_tools` with a canned ToolUse block,
    /// or fails with a supplied error, to exercise `SamplingSource` in isolation
    /// (the end-to-end real-loop proof lives in `tests/sampling_source.rs`).
    struct StubPeer {
        fail: Option<fn() -> pmcp::Error>,
    }

    #[async_trait]
    impl PeerHandle for StubPeer {
        async fn sample(&self, _params: CreateMessageParams) -> Result<CreateMessageResult> {
            Ok(CreateMessageResult::new(Content::text("legacy"), "stub"))
        }

        async fn sample_with_tools(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools> {
            if let Some(make) = self.fail {
                return Err(make());
            }
            Ok(CreateMessageResultWithTools::new(
                "stub-model",
                Role::Assistant,
                vec![SamplingMessageContent::ToolUse {
                    name: "search".to_string(),
                    id: "tu-9".to_string(),
                    input: serde_json::json!({"q": "rust"}),
                    meta: None,
                }],
            ))
        }

        async fn list_roots(&self) -> Result<ListRootsResult> {
            Ok(ListRootsResult { roots: vec![] })
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

    fn params() -> CreateMessageParams {
        CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hi".to_string(),
                meta: None,
            },
        )])
    }

    #[tokio::test]
    async fn forwards_tool_use_from_peer() {
        let peer: Arc<dyn PeerHandle> = Arc::new(StubPeer { fail: None });
        let source = SamplingSource::new(peer);
        let result = source.create_message(params()).await.expect("ok");
        assert_eq!(result.model, "stub-model");
        let has_tool_use = result.content.iter().any(|c| {
            matches!(c, SamplingMessageContent::ToolUse { id, name, .. } if id == "tu-9" && name == "search")
        });
        assert!(has_tool_use, "ToolUse (id+name) must survive to the source");
    }

    #[tokio::test]
    async fn transport_error_is_transient_not_fatal() {
        let peer: Arc<dyn PeerHandle> = Arc::new(StubPeer {
            fail: Some(|| pmcp::Error::internal("boom")),
        });
        let source = SamplingSource::new(peer);
        let err = source.create_message(params()).await.expect_err("must err");
        assert_ne!(err.retry_class(), RetryClass::Fatal);
    }

    #[test]
    fn error_mapping_classes() {
        assert!(matches!(
            map_peer_error(&pmcp::Error::Authentication("x".into())),
            CompletionError::Auth
        ));
        assert!(matches!(
            map_peer_error(&pmcp::Error::RateLimited),
            CompletionError::Capacity(_)
        ));
        assert!(matches!(
            map_peer_error(&pmcp::Error::Timeout(5)),
            CompletionError::Transport(_)
        ));
        assert!(matches!(
            map_peer_error(&pmcp::Error::Cancelled),
            CompletionError::Transport(_)
        ));
    }
}
