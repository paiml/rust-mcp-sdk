//! Connector-client factory seam + the object-safe connector client it produces.
//!
//! A [`pmcp::Client<T>`] is transport-generic, so it is NOT object-safe: an
//! agent that talks to several heterogeneous endpoints cannot hold a
//! `Vec<Client<T>>`. The [`ConnectorClient`] trait erases the transport behind
//! the minimal surface [`ClientToolInvoker`](super::client::ClientToolInvoker)
//! needs (`call_tool` + `wait_for_related_task`), and [`ConnectorClientFactory`]
//! mints one per resolved endpoint.
//!
//! Both traits are UNCONDITIONAL (they compile on `wasm32`) so the seam holds
//! everywhere. The one concrete impl this plan ships,
//! [`UrlConnectorClientFactory`], wraps a `pmcp::Client<StreamableHttpTransport>`
//! and is therefore gated behind the crate's `url-connector` feature (that
//! transport is native-only and pulls a real HTTP client). Command/stdio
//! transports are a narrow, justified follow-up: URL endpoints already cover the
//! AGNT-05/AGNT-09 targets and a command transport adds no new loop behavior —
//! only a second `ConnectorClient` impl behind the same seam.

use async_trait::async_trait;
use std::sync::Arc;

use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, ToolInfo};
use pmcp::WaitForTaskOptions;

/// An error establishing or using a connector client.
///
/// Distinct from a per-call tool error (which is carried as data in a
/// [`ToolCallResult`](crate::seams::ToolCallResult)): this is a
/// transport/configuration failure of the connection itself. Messages never
/// contain secret material.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InvokerError {
    /// Transport / connection / protocol failure — transient.
    #[error("connector transport error: {0}")]
    Transport(String),
    /// The endpoint scheme is not supported by this factory (e.g. a `file://`
    /// URL handed to the URL/HTTP factory).
    #[error("unsupported endpoint scheme: {0}")]
    UnsupportedScheme(String),
    /// The endpoint or client could not be configured.
    #[error("connector configuration error: {0}")]
    Config(String),
}

/// The minimal, object-safe MCP client surface the tool invoker needs.
///
/// Implementors erase a concrete `pmcp::Client<T>` so the invoker can hold an
/// `Arc<dyn ConnectorClient>` over heterogeneous transports.
#[async_trait]
pub trait ConnectorClient: Send + Sync {
    /// Call a tool, returning the raw [`CallToolResult`] (which may carry a
    /// related-task envelope under `_meta`).
    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, InvokerError>;

    /// Drive a related task to terminal and return its FINAL result.
    ///
    /// The caller supplies a hard `max_poll_duration_secs` cap in `opts`, so an
    /// implementation that delegates to
    /// [`pmcp::Client::wait_for_related_task`] can never poll forever.
    ///
    /// # ⚠ THIS METHOD IS THE ENTIRE `tasks/*` SEAM (Phase 117, D-09)
    ///
    /// `pmcp-agent`'s coupling to the tasks surface is bounded to exactly THREE
    /// places: this trait method, its ONE caller
    /// ([`ClientToolInvoker::dispatch`](super::client::ClientToolInvoker), which
    /// applies the hard poll cap) and its ONE impl (`UrlConnectorClient` below,
    /// which delegates to `pmcp::Client`). No other method on this trait names a
    /// `tasks/*` wire method, and none may be added: a Phase-114 tasks
    /// sign-off change must have exactly one place to look. Widening this seam
    /// is the failure D-09 exists to prevent.
    async fn wait_for_related_task(
        &self,
        meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult, InvokerError>;

    /// Discover the tools this connector advertises (`tools/list`), so the loop
    /// can tell the model what it may call.
    ///
    /// DEFAULT: `Ok(empty)` — a connector that does not support discovery
    /// advertises nothing, which is backward-compatible (existing impls keep
    /// compiling) and makes the loop simply send no `tools` to the model.
    async fn list_tools(&self) -> Result<Vec<ToolInfo>, InvokerError> {
        Ok(Vec::new())
    }

    /// The protocol version this connector NEGOTIATED with its endpoint, as the
    /// wire string (e.g. `"2026-07-28"` or `"2025-11-25"`).
    ///
    /// Classify it with [`pmcp::types::protocol::protocol_era`] rather than
    /// comparing strings: that classifier's unknown-to-`V1` conservative
    /// fallback gives the right answer for any value, including one a future
    /// server invents. This is the value
    /// [`EffectTrace::with_negotiated_version`](crate::trace::EffectTrace::with_negotiated_version)
    /// records, so a replay can detect that a trace was recorded under a
    /// different era than it is being replayed under (D-08).
    ///
    /// DEFAULT: `None` — a connector that tracks no era reports none, which
    /// keeps every existing implementor compiling unchanged (the same
    /// backward-compatible-default discipline [`Self::list_tools`] uses). It is
    /// NOT a claim that the connection is era-less.
    fn negotiated_protocol_version(&self) -> Option<&str> {
        None
    }
}

/// Produces a [`ConnectorClient`] for a resolved endpoint (URL or, in a
/// follow-up, command transport).
#[async_trait]
pub trait ConnectorClientFactory: Send + Sync {
    /// Establish a connector client for `endpoint`.
    async fn client_for(&self, endpoint: &str) -> Result<Arc<dyn ConnectorClient>, InvokerError>;
}

// The concrete URL/HTTP connector. Native-only + behind `url-connector`
// (BLOCKER-2): `StreamableHttpTransport` is `#[cfg(not(target_arch = "wasm32"))]`
// and pulls a real HTTP client, so the default + wasm32 build never sees it.
#[cfg(feature = "url-connector")]
pub use url_impl::UrlConnectorClientFactory;

#[cfg(feature = "url-connector")]
mod url_impl {
    use super::{ConnectorClient, ConnectorClientFactory, InvokerError};
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::time::Duration;

    use pmcp::shared::streamable_http::{
        StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
    };
    use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
    use pmcp::types::tasks::TaskMetadata;
    use pmcp::types::{CallToolResult, ToolInfo};
    use pmcp::{Client, ClientBuilder, ClientCapabilities, WaitForTaskOptions};
    use url::Url;

    /// How long [`endpoint_is_reachable`] waits for a TCP connection before it
    /// declares the endpoint "did NOT answer".
    ///
    /// EXPLICIT and bounded: an unbounded probe would turn a black-holed host
    /// into a hang, and the whole point of the probe is that its failure is a
    /// fast, unambiguous fact.
    const REACHABILITY_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Why the v2 era attempt did not yield a client.
    ///
    /// This enum deliberately does NOT encode the era-rejection-vs-infrastructure
    /// split. That split is decided by the reachability fact established before
    /// the attempt ran (see [`endpoint_is_reachable`]) and is applied in ONE
    /// place — the match in `client_for` — so there is a single site that answers
    /// "who decides reachability". The error below is never read to classify.
    #[derive(Debug)]
    enum V2Failure {
        /// The v2 attempt was SENT and the server did not accept the era.
        /// Whether this means "era rejection, fall back to v1" or
        /// "infrastructure, propagate" is the caller's reachability call.
        Rejected(Box<pmcp::Error>),
        /// The v2 client could not be CONSTRUCTED, so nothing was ever sent.
        /// A local configuration failure — it says nothing about the server, so
        /// it is neither a rejection nor a reachability signal.
        NotAttempted(InvokerError),
    }

    /// Establish, at the HOST layer, whether `url`'s endpoint ANSWERS at all.
    ///
    /// `true` means the endpoint accepted a TCP connection, so any subsequent
    /// protocol-level failure is "the server ANSWERED". `false` is
    /// unambiguously "the server did NOT answer" — including the degenerate
    /// cases of a URL with no host and a URL with no resolvable port, which
    /// cannot answer either.
    ///
    /// # THE CLASSIFICATION CONTRACT
    ///
    /// *(Plan 117-11 Task 1 is instructed to CITE this block verbatim rather
    /// than re-derive it. Two independently written classifiers, drifting apart,
    /// is the failure this paragraph prevents.)*
    ///
    /// ```text
    /// The endpoint ANSWERED (any HTTP response, any JSON-RPC error) => era rejection => FALL BACK to v1.
    /// The endpoint did NOT answer (DNS / TCP / timeout)             => infrastructure => PROPAGATE.
    /// ```
    ///
    /// ## Why a host-layer probe instead of inspecting the attempt's error
    ///
    /// MEASURED, and it is why neither the error VARIANT nor the message can do
    /// this job:
    ///
    /// - `src/shared/streamable_http.rs` turns a connect/send failure into
    ///   `Error::Transport(TransportError::Request(String))`.
    /// - the same file turns a NON-2xx HTTP status into the SAME
    ///   `Error::Transport(TransportError::Request(String))`.
    ///
    /// (Cited by SYMBOL, not by line: the two sites move whenever that file is
    /// edited, and a stale line number is worse than none — it sends a reader to
    /// unrelated code and quietly discredits the whole contract.)
    ///
    /// A third-party v1 server answering a plain `404` and a refused TCP
    /// connection are therefore one variant carrying different prose.
    /// `TransportError::Request` carries only a `String`, and
    /// `InvokerError::Transport` stringifies again — so classifying on the
    /// variant is wrong, and classifying on the prose is string matching, which
    /// § Q4.3 forbids (it measured FOUR distinct rejection signatures across
    /// implementations, none of them stable).
    ///
    /// ## Two known imprecisions, recorded rather than hidden
    ///
    /// 1. A **TLS handshake failure** on `https` passes this TCP probe and is
    ///    classified `Answered`, so a pointless v1 attempt is made — which then
    ///    fails with the same TLS error and propagates.
    /// 2. A server that **accepts TCP but never responds** is classified
    ///    `Answered`; the bounded timeout on the attempt itself then surfaces as
    ///    an error either way.
    ///
    /// ## Why both are acceptable — the STRUCTURAL invariant
    ///
    /// Era V1 is reported ONLY when `try_v1`'s `initialize` actually SUCCEEDED.
    /// A misclassification can therefore cost a wasted round trip, or change
    /// WHICH error is reported — it can never produce the Pitfall-7 silent
    /// downgrade in which an agent claims "connected via v1" against a host that
    /// never answered.
    ///
    /// This returns a plain `bool` rather than a `Result` ON PURPOSE: every
    /// failure mode here means exactly one thing to the only caller — "did not
    /// answer" — so an error type would carry prose that nothing may read
    /// without violating the contract above.
    async fn endpoint_is_reachable(url: &Url) -> bool {
        let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
            return false;
        };
        // `Url::host_str` returns an IPv6 literal WITH its URL brackets
        // (`http://[::1]:8080/` -> `"[::1]"`), and `ToSocketAddrs` would try to
        // resolve that as a DNS name and fail — classifying a live IPv6
        // endpoint as "did not answer" and suppressing the legitimate v1
        // fallback. The brackets are URL syntax, not part of the address.
        let host = host.trim_start_matches('[').trim_end_matches(']');
        // The stream is dropped immediately: the ANSWER is the whole fact.
        matches!(
            tokio::time::timeout(
                REACHABILITY_PROBE_TIMEOUT,
                tokio::net::TcpStream::connect((host, port)),
            )
            .await,
            Ok(Ok(_stream))
        )
    }

    /// Attempt 1 — the `2026-07-28` era, PINNED.
    ///
    /// The pin is what makes this legal under A-D08: the HOST makes an explicit
    /// era choice and `server_discover` only CONFIRMS it. `pmcp::Client` is not
    /// probing to decide (Phase-113 D-08 forbids exactly that, and
    /// `src/client/mod.rs` carries the lock as a literal "do not restore the
    /// latter" comment). A client built without
    /// `ClientBuilder::with_protocol_version` is never in v2 mode at all, which
    /// is why `initialize` used to run its full v1 path here.
    ///
    /// This attempt does NOT classify its own failure. The reachability fact
    /// from [`endpoint_is_reachable`] is captured by `client_for` BEFORE this
    /// runs, and applied there — so the error below is never read to decide an
    /// era.
    async fn try_v2(url: &Url) -> Result<Client<StreamableHttpTransport>, V2Failure> {
        let config = StreamableHttpTransportConfigBuilder::new(url.clone()).build();
        let transport = StreamableHttpTransport::new(config);
        let builder = match ClientBuilder::new(transport)
            .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        {
            Ok(builder) => builder.with_tasks_extension(),
            Err(err) => {
                return Err(V2Failure::NotAttempted(InvokerError::Config(
                    err.to_string(),
                )))
            },
        };
        // `ClientBuilder::build` marks a v2 client initialized, so this sends
        // ZERO handshake bytes: `server/discover` is the first and only request.
        let mut client = builder.build();
        match client.server_discover().await {
            Ok(_projection) => Ok(client),
            Err(err) => Err(V2Failure::Rejected(Box::new(err))),
        }
    }

    /// Attempt 2 — v1, byte-identical to the pre-117 agent.
    ///
    /// Returns the connected client together with the version the server
    /// NEGOTIATED in its `initialize` result (not a hardcoded guess).
    async fn try_v1(url: &Url) -> Result<(Client<StreamableHttpTransport>, String), InvokerError> {
        let config = StreamableHttpTransportConfigBuilder::new(url.clone()).build();
        let transport = StreamableHttpTransport::new(config);
        let mut client = Client::new(transport);
        let initialized = client
            .initialize(ClientCapabilities::default())
            .await
            .map_err(|e| InvokerError::Transport(e.to_string()))?;
        let version = initialized.protocol_version.0;
        Ok((client, version))
    }

    /// A [`ConnectorClientFactory`] that connects `http(s)://` endpoints over
    /// `StreamableHttpTransport`.
    #[derive(Debug, Default, Clone)]
    pub struct UrlConnectorClientFactory;

    impl UrlConnectorClientFactory {
        /// Create a URL connector factory.
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl ConnectorClientFactory for UrlConnectorClientFactory {
        /// Establish a connector, PREFERRING the `2026-07-28` era and falling
        /// back to v1 only when the endpoint ANSWERED (D-07 / CLNT-03).
        ///
        /// The full rule, its two known imprecisions and the structural
        /// invariant that makes them harmless are documented ONCE, on
        /// [`endpoint_is_reachable`].
        async fn client_for(
            &self,
            endpoint: &str,
        ) -> Result<Arc<dyn ConnectorClient>, InvokerError> {
            let url = url::Url::parse(endpoint)
                .map_err(|e| InvokerError::Config(format!("invalid endpoint URL: {e}")))?;
            // T-108-05-05: only http(s) endpoints are dispatched (mirrors the
            // 108-04 scheme policy) — never a `file://`/`data:`/etc. scheme.
            match url.scheme() {
                "http" | "https" => {},
                other => return Err(InvokerError::UnsupportedScheme(other.to_string())),
            }
            match try_v2(&url).await {
                Ok(client) => Ok(Arc::new(UrlConnectorClient::new(
                    client,
                    PROTOCOL_VERSION_2026_07_28.to_string(),
                ))),
                // The ONE site that turns the reachability fact into an era
                // decision. The error is logged, never inspected.
                //
                // The probe is paid LAZILY, only here. Establishing it ahead of
                // attempt 1 added a full TCP handshake to every SUCCESSFUL v2
                // connection for a fact only this failure path reads. The
                // contract on `endpoint_is_reachable` is about not deriving
                // reachability from an ERROR STRING; a TCP connect run at this
                // point still stringifies nothing, so it is preserved — the same
                // lazy shape `mcp_tester::detect_eras` uses. (An `if` inside the
                // arm rather than a match guard, because guards cannot `.await`.)
                Err(V2Failure::Rejected(rejection)) => {
                    if !endpoint_is_reachable(&url).await {
                        // No v1 attempt: a host that never answered is
                        // infrastructure, not a protocol signal (T-117-21 /
                        // Pitfall 7).
                        return Err(InvokerError::Transport(rejection.to_string()));
                    }
                    // The v2 client is DROPPED here; a fresh v1 client is built.
                    tracing::debug!(
                        endpoint,
                        rejection = %rejection,
                        "endpoint answered and declined the 2026-07-28 era; falling back to v1"
                    );
                    let (client, version) = try_v1(&url).await?;
                    Ok(Arc::new(UrlConnectorClient::new(client, version)))
                },
                Err(V2Failure::NotAttempted(err)) => Err(err),
            }
        }
    }

    /// A [`ConnectorClient`] backed by a connected `Client<StreamableHttpTransport>`.
    struct UrlConnectorClient {
        client: Client<StreamableHttpTransport>,
        /// The wire version this connection negotiated. Set to the pinned
        /// `2026-07-28` constant on the v2 path, and to the version the server
        /// echoed in its `initialize` result on the v1 fallback.
        negotiated_version: String,
    }

    impl UrlConnectorClient {
        fn new(client: Client<StreamableHttpTransport>, negotiated_version: String) -> Self {
            Self {
                client,
                negotiated_version,
            }
        }
    }

    #[async_trait]
    impl ConnectorClient for UrlConnectorClient {
        async fn call_tool(
            &self,
            name: &str,
            arguments: serde_json::Value,
        ) -> Result<CallToolResult, InvokerError> {
            self.client
                .call_tool(name.to_string(), arguments)
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }

        async fn wait_for_related_task(
            &self,
            meta: &TaskMetadata,
            opts: WaitForTaskOptions,
        ) -> Result<CallToolResult, InvokerError> {
            self.client
                .wait_for_related_task(meta, opts)
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }

        async fn list_tools(&self) -> Result<Vec<ToolInfo>, InvokerError> {
            self.client
                .list_tools(None)
                .await
                .map(|result| result.tools)
                .map_err(|e| InvokerError::Transport(e.to_string()))
        }

        fn negotiated_protocol_version(&self) -> Option<&str> {
            Some(&self.negotiated_version)
        }
    }
}
