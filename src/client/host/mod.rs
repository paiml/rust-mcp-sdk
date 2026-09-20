//! Client host surface: answering server -> client requests.
//!
//! Historically a [`Client`](crate::Client) rejected any inbound request with an
//! "Unexpected message type" error. This module adds the **host** direction of
//! the MCP spec: a client can register handlers that answer inbound
//! `sampling/createMessage`, `elicitation/create`, and `roots/list` requests
//! while one of the client's own requests is in flight (the nested
//! sampling-during-`tools/call` flow).
//!
//! # Direction disambiguation (HOST-06)
//!
//! - [`HostSamplingHandler`] answers an INBOUND sampling request (server asks
//!   client — the spec host direction).
//! - [`pmcp::SamplingHandler`](crate::SamplingHandler) is the INVERSE
//!   "LLM-server pattern": a client calls
//!   [`Client::create_message`](crate::Client::create_message) to ask a server
//!   whose `SamplingHandler` runs the LLM. It is unchanged and not deprecated.
//!
//! Placing these traits under `pmcp::client::host::*` guarantees the public
//! paths are unmistakable even where names are similar.
//!
//! # Idle-host limitation
//!
//! The pmcp `Client` has no background receive loop; inbound requests are only
//! read while a client-initiated request awaits its response. Answering
//! sampling/elicitation/roots therefore works **during** an in-flight client
//! request (the agent/tool use case) but not while the client sits idle. This
//! is intentional for the current phase.

pub mod elicitation;
pub mod roots;
pub mod sampling;

use crate::types::mrtr::{InputRequest, InputRequestKind, InputRequests};
use crate::types::protocol::{ClientRequest, Request, ServerRequest};
use std::sync::Arc;

pub use elicitation::HostElicitationHandler;
pub use roots::RootsProvider;
pub use sampling::{
    lift_content_to_sampling, lift_result_to_with_tools, ApprovalDecision, HostSamplingHandler,
    HostSamplingHandlerWithTools, LegacyHostSamplingAdapter, PreflightApproval,
    SamplingResultReview,
};

/// Immutable set of host handlers a [`Client`](crate::Client) answers with.
///
/// Built once via [`ClientBuilder`](crate::ClientBuilder) and never mutated
/// afterwards, so dispatch stays lock-free (plain `Option<Arc<..>>` fields, no
/// interior mutability).
#[derive(Clone, Default)]
pub struct ClientHostRegistry {
    pub(crate) sampling: Option<Arc<dyn HostSamplingHandler>>,
    /// Tool-aware sampling handler (MCP 2025-11-25). Preferred over `sampling`
    /// when both are present; when only `sampling` is set, a `WithTools` code path
    /// adapts it via [`LegacyHostSamplingAdapter`].
    pub(crate) sampling_with_tools: Option<Arc<dyn HostSamplingHandlerWithTools>>,
    pub(crate) elicitation: Option<Arc<dyn HostElicitationHandler>>,
    pub(crate) roots: Option<RootsProvider>,
    pub(crate) approval: Option<PreflightApproval>,
    pub(crate) result_review: Option<SamplingResultReview>,
}

impl std::fmt::Debug for ClientHostRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientHostRegistry")
            .field("has_sampling", &self.sampling.is_some())
            .field(
                "has_sampling_with_tools",
                &self.sampling_with_tools.is_some(),
            )
            .field("has_elicitation", &self.elicitation.is_some())
            .field("has_roots", &self.roots.is_some())
            .field("has_approval", &self.approval.is_some())
            .field("has_result_review", &self.result_review.is_some())
            .finish()
    }
}

/// Classification of an inbound request at a client into a host-handler kind.
///
/// Pure, synchronous, and side-effect free so it can be property/fuzz tested
/// independently of the async dispatch path.
///
/// Exposed as `#[doc(hidden)] pub` (not part of the public API surface) solely
/// so the routing fuzz target can drive [`classify_host_request`] directly.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRequestKind {
    /// `sampling/createMessage` (either parse variant — see below).
    Sampling,
    /// `elicitation/create`.
    Elicitation,
    /// `roots/list`.
    Roots,
    /// Inbound `ping` (server -> client keepalive). Answered with an empty
    /// result per the MCP spec MUST, independent of the host registry.
    Ping,
    /// A known-but-unroutable request kind.
    Unhandled,
}

/// Classify an inbound [`Request`] into the host-handler kind that should answer
/// it.
///
/// Handles the parse ambiguity where an inbound `sampling/createMessage` is
/// delivered as `Request::Client(ClientRequest::CreateMessage)` (because
/// `parse_request` tries the client grammar first) as well as the
/// `Request::Server(ServerRequest::CreateMessage)` shape. Both map to
/// [`HostRequestKind::Sampling`].
///
/// Exposed as `#[doc(hidden)] pub` (not part of the stable public API) so the
/// routing fuzz target can exercise the real dispatch classification path.
#[doc(hidden)]
pub fn classify_host_request(request: &Request) -> HostRequestKind {
    match request {
        Request::Client(client) => match client.as_ref() {
            ClientRequest::CreateMessage(_) => HostRequestKind::Sampling,
            // A server -> client `ping` parses via the client grammar
            // (`ClientRequest::Ping`). The MCP spec requires the receiver to
            // respond promptly with an empty result, so it must not fall
            // through to method-not-found.
            ClientRequest::Ping => HostRequestKind::Ping,
            _ => HostRequestKind::Unhandled,
        },
        Request::Server(server) => match server.as_ref() {
            ServerRequest::CreateMessage(_) => HostRequestKind::Sampling,
            ServerRequest::ElicitationCreate(_) => HostRequestKind::Elicitation,
            ServerRequest::ListRoots => HostRequestKind::Roots,
        },
    }
}

// ===========================================================================
// The MRTR (`inputRequests`) sibling — Phase 113, CLNT-02.
// ===========================================================================

/// Classify one MRTR [`InputRequest`] into the host-handler kind that answers it.
///
/// # Why there are TWO classifiers
///
/// On v1 the three host request kinds arrive as server-initiated JSON-RPC
/// requests and are classified by [`classify_host_request`]. On v2 the spec says
/// servers **MUST NOT** send independent requests: the SAME three kinds arrive
/// instead as entries of an `input_required` result's `inputRequests` map. The
/// Phase-106 host surface is therefore REPLACED by MRTR on v2, not supplemented
/// — which is exactly why both classifiers return the SAME
/// [`HostRequestKind`] and both feed the SAME dispatch helpers (D-06: one
/// elicitation callback serves both eras).
///
/// Pure, synchronous and side-effect free, so it is property/fuzz testable
/// independently of the async fold.
#[doc(hidden)]
#[must_use]
pub fn classify_input_request(request: &InputRequest) -> HostRequestKind {
    // `InputRequest::kind()` is already the total, exhaustive, const enum->kind
    // mapping. This used to route through a string classifier that took
    // `request.kind().wire_method()` and linearly scanned the same three kinds
    // to recover the kind it had just been handed — a typed value round-tripped
    // through its own wire spelling and back. The string half had no caller of
    // its own, and the detour manufactured an `Unhandled` outcome that is
    // structurally unreachable from a decoded `InputRequest`.
    host_kind_of(request.kind())
}

/// The ONE mapping from an MRTR kind to its host-handler kind.
const fn host_kind_of(kind: InputRequestKind) -> HostRequestKind {
    match kind {
        InputRequestKind::Elicitation => HostRequestKind::Elicitation,
        InputRequestKind::Sampling => HostRequestKind::Sampling,
        InputRequestKind::Roots => HostRequestKind::Roots,
    }
}

impl ClientHostRegistry {
    /// Whether a registered handler exists that COULD answer `kind`.
    ///
    /// Presence only — nothing is invoked, no approval gate runs.
    fn can_fulfil(&self, kind: HostRequestKind) -> bool {
        match kind {
            // Either sampling shape can service an inbound
            // `sampling/createMessage`; dispatch prefers `sampling_with_tools`.
            HostRequestKind::Sampling => {
                self.sampling.is_some() || self.sampling_with_tools.is_some()
            },
            HostRequestKind::Elicitation => self.elicitation.is_some(),
            HostRequestKind::Roots => self.roots.is_some(),
            // `ping` is never an MRTR input request, and `Unhandled` is by
            // definition unfulfillable.
            HostRequestKind::Ping | HostRequestKind::Unhandled => false,
        }
    }

    /// Prove EVERY entry of an `inputRequests` map is fulfillable, BEFORE any
    /// handler runs.
    ///
    /// Returns `Err(kind)` naming the FIRST unfulfillable kind. The ordering
    /// matters: a map whose second entry has no handler must not first prompt a
    /// human (or spend an agent's tokens) on the fulfillable first entry, since
    /// the fold is all-or-nothing and that work would be discarded (T-113-26 /
    /// T-113-27).
    pub(crate) fn preflight_input_requests(
        &self,
        requests: &InputRequests,
    ) -> std::result::Result<(), HostRequestKind> {
        for request in requests.values() {
            let kind = classify_input_request(request);
            if !self.can_fulfil(kind) {
                return Err(kind);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::sampling::CreateMessageParams;

    #[test]
    fn registry_default_is_all_none() {
        let reg = ClientHostRegistry::default();
        assert!(reg.sampling.is_none());
        assert!(reg.elicitation.is_none());
        assert!(reg.roots.is_none());
        assert!(reg.approval.is_none());
        assert!(reg.result_review.is_none());
    }

    #[test]
    fn registry_debug_reports_has_flags() {
        let reg = ClientHostRegistry::default();
        let dbg = format!("{reg:?}");
        assert!(dbg.contains("has_sampling: false"));
        assert!(dbg.contains("has_elicitation: false"));
        assert!(dbg.contains("has_roots: false"));
    }

    #[test]
    fn classify_sampling_server_variant() {
        let req = Request::Server(Box::new(ServerRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))));
        assert_eq!(classify_host_request(&req), HostRequestKind::Sampling);
    }

    #[test]
    fn classify_sampling_client_alias_variant() {
        // Inbound sampling parses as the CLIENT variant (parse ambiguity).
        let req = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))));
        assert_eq!(classify_host_request(&req), HostRequestKind::Sampling);
    }

    #[test]
    fn classify_roots_and_elicitation() {
        let roots = Request::Server(Box::new(ServerRequest::ListRoots));
        assert_eq!(classify_host_request(&roots), HostRequestKind::Roots);

        let elicit = Request::Server(Box::new(ServerRequest::ElicitationCreate(Box::new(
            crate::types::elicitation::ElicitRequestParams::Form {
                message: "please".to_string(),
                requested_schema: serde_json::json!({}),
            },
        ))));
        assert_eq!(classify_host_request(&elicit), HostRequestKind::Elicitation);
    }

    #[test]
    fn classify_unhandled_client_request() {
        let req = Request::Client(Box::new(ClientRequest::ListTools(Default::default())));
        assert_eq!(classify_host_request(&req), HostRequestKind::Unhandled);
    }

    #[test]
    fn classify_inbound_ping_is_ping_not_unhandled() {
        // WR-01: server -> client ping parses as ClientRequest::Ping and must
        // classify as Ping (spec MUST: answered with an empty result), never
        // fall through to Unhandled / method-not-found.
        let req = Request::Client(Box::new(ClientRequest::Ping));
        assert_eq!(classify_host_request(&req), HostRequestKind::Ping);
    }

    // =======================================================================
    // MRTR `inputRequests` classification + preflight (Phase 113, CLNT-02).
    // =======================================================================

    mod input_requests {
        use super::*;
        use crate::types::elicitation::ElicitRequestParams;

        fn elicitation_entry() -> InputRequest {
            InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
                message: "who?".to_string(),
                requested_schema: serde_json::json!({}),
            }))
        }

        fn sampling_entry() -> InputRequest {
            InputRequest::Sampling(Box::new(CreateMessageParams::new(Vec::new())))
        }

        fn registry_with_elicitation() -> ClientHostRegistry {
            struct Handler;
            #[async_trait::async_trait]
            impl HostElicitationHandler for Handler {
                async fn handle_elicitation(
                    &self,
                    _params: ElicitRequestParams,
                ) -> crate::Result<crate::types::elicitation::ElicitResult> {
                    unreachable!("preflight must not invoke anything")
                }
            }
            ClientHostRegistry {
                elicitation: Some(Arc::new(Handler)),
                ..ClientHostRegistry::default()
            }
        }

        #[test]
        fn classify_maps_the_three_kinds() {
            assert_eq!(
                classify_input_request(&elicitation_entry()),
                HostRequestKind::Elicitation
            );
            assert_eq!(
                classify_input_request(&sampling_entry()),
                HostRequestKind::Sampling
            );
            assert_eq!(
                classify_input_request(&InputRequest::ListRoots),
                HostRequestKind::Roots
            );
        }

        #[test]
        fn preflight_passes_when_every_kind_has_a_handler() {
            let registry = registry_with_elicitation();
            let mut requests = InputRequests::new();
            requests.insert("a".to_string(), elicitation_entry());
            assert!(registry.preflight_input_requests(&requests).is_ok());
        }

        /// The FIRST unfulfillable kind is named, and an empty map trivially
        /// passes.
        #[test]
        fn preflight_names_the_first_unfulfillable_kind() {
            let registry = registry_with_elicitation();
            let mut requests = InputRequests::new();
            requests.insert("a".to_string(), elicitation_entry());
            requests.insert("b".to_string(), sampling_entry());
            assert_eq!(
                registry.preflight_input_requests(&requests),
                Err(HostRequestKind::Sampling)
            );

            assert!(registry
                .preflight_input_requests(&InputRequests::new())
                .is_ok());
        }

        /// `on_sampling_with_tools` sets only `sampling_with_tools`, and
        /// dispatch prefers it — preflight must accept either shape or a
        /// `WithTools`-only client would refuse a sampling entry it can answer.
        #[test]
        fn preflight_accepts_either_sampling_handler_shape() {
            struct WithTools;
            #[async_trait::async_trait]
            impl HostSamplingHandlerWithTools for WithTools {
                async fn handle_create_message_with_tools(
                    &self,
                    _params: CreateMessageParams,
                ) -> crate::Result<crate::types::sampling::CreateMessageResultWithTools>
                {
                    unreachable!("preflight must not invoke anything")
                }
            }
            let registry = ClientHostRegistry {
                sampling_with_tools: Some(Arc::new(WithTools)),
                ..ClientHostRegistry::default()
            };
            let mut requests = InputRequests::new();
            requests.insert("a".to_string(), sampling_entry());
            assert!(registry.preflight_input_requests(&requests).is_ok());
        }
    }
}
