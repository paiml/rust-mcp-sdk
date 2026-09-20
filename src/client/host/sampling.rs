//! Host-side sampling handler + approval seam.
//!
//! This module defines the trait a [`Client`](crate::Client) implements to
//! **answer** an inbound, spec-direction `sampling/createMessage` request — the
//! MCP *host* direction, where a connected server asks the client to run an LLM
//! completion on its behalf.
//!
//! # Not to be confused with the LLM-server pattern
//!
//! This is the **inverse** of [`pmcp::SamplingHandler`](crate::SamplingHandler)
//! (defined at `crate::server::SamplingHandler`). That server-side trait powers
//! the legacy "LLM-server pattern", where a *client* calls
//! [`Client::create_message`](crate::Client::create_message) to ask a *server*
//! whose `SamplingHandler` runs the LLM. Here the roles are reversed: the
//! **server** requests sampling and the **client host** answers it. See
//! [`crate::client::host`] for the full picture.

use crate::error::Result;
use crate::types::content::Content;
use crate::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools, SamplingMessageContent,
};
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;

/// Answers an inbound spec-direction `sampling/createMessage` request.
///
/// A [`Client`](crate::Client) registers an implementation via
/// [`ClientBuilder::on_sampling`](crate::ClientBuilder::on_sampling). When a
/// connected server calls `extra.peer().sample(..)` while one of the client's
/// own requests is in flight, the inbound request is routed to this handler and
/// the produced [`CreateMessageResult`] is returned to the server.
///
/// Distinct from [`pmcp::SamplingHandler`](crate::SamplingHandler), which is the
/// inverted LLM-server pattern (client asks server). The unambiguous public path
/// for *this* trait is `pmcp::client::host::HostSamplingHandler`.
#[async_trait]
pub trait HostSamplingHandler: Send + Sync {
    /// Produce a completion for the given inbound sampling request.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion cannot be produced. The client maps a
    /// handler error to a sanitized JSON-RPC `-32603` response (the raw error is
    /// logged locally and never forwarded to the remote server).
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResult>;
}

/// Answers an inbound `sampling/createMessage` request with a **tool-aware**
/// completion (MCP 2025-11-25).
///
/// This is the `WithTools` counterpart of [`HostSamplingHandler`]: it returns a
/// [`CreateMessageResultWithTools`] whose `content` is a
/// `Vec<SamplingMessageContent>`, so the host can answer with `tool_use` /
/// `tool_result` blocks that the legacy single-`Content` [`CreateMessageResult`]
/// cannot express. Register one via
/// [`ClientBuilder::on_sampling_with_tools`](crate::ClientBuilder::on_sampling_with_tools).
///
/// A client that registers only a legacy [`HostSamplingHandler`] still answers
/// `WithTools`-shaped needs through [`LegacyHostSamplingAdapter`], which lifts the
/// single `Content` into a one-element `Vec<SamplingMessageContent>`.
#[async_trait]
pub trait HostSamplingHandlerWithTools: Send + Sync {
    /// Produce a tool-aware completion for the given inbound sampling request.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion cannot be produced. As with the legacy
    /// handler, the client maps a handler error to a sanitized JSON-RPC `-32603`
    /// response (the raw error is logged locally, never forwarded upstream).
    async fn handle_create_message_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools>;
}

/// Lift a single response [`Content`] into its [`SamplingMessageContent`]
/// counterpart for the `WithTools` shape. Thin wrapper over the canonical
/// [`SamplingMessageContent::from_content`].
#[must_use]
pub fn lift_content_to_sampling(content: Content) -> SamplingMessageContent {
    SamplingMessageContent::from_content(content)
}

/// Lift a legacy [`CreateMessageResult`] into a [`CreateMessageResultWithTools`].
/// Thin wrapper over the canonical [`CreateMessageResultWithTools::from_single`].
#[must_use]
pub fn lift_result_to_with_tools(result: CreateMessageResult) -> CreateMessageResultWithTools {
    CreateMessageResultWithTools::from_single(result)
}

/// Adapts any legacy [`HostSamplingHandler`] to the
/// [`HostSamplingHandlerWithTools`] interface by lifting its single-`Content`
/// result via [`lift_result_to_with_tools`].
///
/// Used so a client registered with only a legacy handler can still satisfy a
/// `WithTools` code path (e.g. a server that dispatches `sample_with_tools`)
/// without changing the legacy handler's own behavior.
pub struct LegacyHostSamplingAdapter {
    inner: Arc<dyn HostSamplingHandler>,
}

impl std::fmt::Debug for LegacyHostSamplingAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyHostSamplingAdapter").finish()
    }
}

impl LegacyHostSamplingAdapter {
    /// Wrap a legacy handler.
    #[must_use]
    pub fn new(inner: Arc<dyn HostSamplingHandler>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl HostSamplingHandlerWithTools for LegacyHostSamplingAdapter {
    async fn handle_create_message_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        let legacy = self.inner.handle_create_message(params).await?;
        Ok(lift_result_to_with_tools(legacy))
    }
}

/// Outcome of a human-in-the-loop approval callback.
///
/// The approval seam is a host-side access-control hook on sampling. Both hooks
/// ([`PreflightApproval`] and [`SamplingResultReview`]) are invoked by
/// `dispatch_host_sampling` as of this phase — an [`ApprovalDecision::Deny`]
/// from either gates the LLM call / suppresses the completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Allow the sampling call to proceed / the completion to be returned.
    Allow,
    /// Deny the sampling call, carrying a human-readable reason.
    Deny(String),
}

/// Optional pre-handler approval gate for sampling.
///
/// Invoked with owned [`CreateMessageParams`] (owned so the value moves cleanly
/// into a `'static` future) BEFORE the [`HostSamplingHandler`] runs. Returning
/// [`ApprovalDecision::Deny`] prevents the LLM call.
///
/// The gate is optional with a default-allow posture: when no callback is
/// registered, every inbound sampling request reaches the handler unchallenged.
/// Register one via
/// [`ClientBuilder::on_sampling_approval`](crate::ClientBuilder::on_sampling_approval)
/// to require human/policy approval before any tokens are billed. It is invoked
/// by `dispatch_host_sampling` as of this phase.
pub type PreflightApproval =
    Arc<dyn Fn(CreateMessageParams) -> BoxFuture<'static, ApprovalDecision> + Send + Sync>;

/// Optional post-handler review of a produced completion.
///
/// Invoked with the owned request params and the produced
/// [`CreateMessageResult`] AFTER the [`HostSamplingHandler`] runs, letting an
/// approver inspect the actual completion before it is returned. Returning
/// [`ApprovalDecision::Deny`] suppresses the completion.
///
/// Optional with a default pass-through posture: when no callback is
/// registered the produced completion is returned as-is. It is invoked by
/// `dispatch_host_sampling` as of this phase.
pub type SamplingResultReview = Arc<
    dyn Fn(CreateMessageParams, CreateMessageResult) -> BoxFuture<'static, ApprovalDecision>
        + Send
        + Sync,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::content::Role;

    /// Legacy handler returning a single text content.
    struct LegacyText;

    #[async_trait]
    impl HostSamplingHandler for LegacyText {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            Ok(
                CreateMessageResult::new(Content::text("hello"), "legacy-model")
                    .with_stop_reason("endTurn"),
            )
        }
    }

    #[tokio::test]
    async fn legacy_adapter_lifts_single_content_into_with_tools() {
        let adapter = LegacyHostSamplingAdapter::new(Arc::new(LegacyText));
        let result = adapter
            .handle_create_message_with_tools(CreateMessageParams::new(Vec::new()))
            .await
            .expect("adapter must succeed");

        assert_eq!(result.model, "legacy-model");
        assert_eq!(result.stop_reason.as_deref(), Some("endTurn"));
        assert_eq!(result.role, Role::Assistant);
        assert_eq!(
            result.content.len(),
            1,
            "single content lifts to one element"
        );
        match &result.content[0] {
            SamplingMessageContent::Text { text, .. } => assert_eq!(text, "hello"),
            other => panic!("expected lifted Text content, got {other:?}"),
        }
    }

    #[test]
    fn lift_content_maps_each_variant() {
        // Text/image/audio map 1:1; resource renders as text.
        assert!(matches!(
            lift_content_to_sampling(Content::text("t")),
            SamplingMessageContent::Text { .. }
        ));
        assert!(matches!(
            lift_content_to_sampling(Content::image("d", "image/png")),
            SamplingMessageContent::Image { .. }
        ));
        assert!(matches!(
            lift_content_to_sampling(Content::resource_with_text(
                "file:///x",
                "body",
                "text/plain"
            )),
            SamplingMessageContent::Text { .. }
        ));
    }
}
