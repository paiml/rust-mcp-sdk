//! The Phase-118 ERA TARGET: a dual-accept-list MCP server served over real
//! streamable HTTP on an ephemeral port.
//!
//! # What this is, and what it is not
//!
//! This server exists ONLY as the Phase-118 era-observation target (D-16) and
//! the CONF-03 evidence surface. It is **not** a reference server, nothing ships
//! it, and no binary in this crate serves it. The four reference servers
//! (`team-fs`, `mem-mcp`, `approval-mcp`, `team-mcp`) are deliberately left
//! v1-only and are NOT modified by Phase 118 — they are the v1 regression guard
//! (plan 118-10), not the era surface. The accept-list opt-in lives HERE, on
//! this server, via [`pmcp::Server::builder`]'s
//! `with_supported_protocol_versions` (`src/server/mod.rs:3087`), which is the
//! one call that makes v2 reachable without touching a single `build_*_server`
//! function.
//!
//! # The three `dep__*` tools and D-12
//!
//! The three tools named `dep__*` map one-to-one onto the deprecated
//! capabilities CONF-03 is about — **Roots**, **Sampling** and **Logging**.
//! D-12's settled reading is that each capability must be reachable **via its v2
//! mechanism**, NOT that the v1 RPC shapes answer under v2:
//!
//! | capability | v1 mechanism | v2 mechanism |
//! |---|---|---|
//! | Logging  | a preceding `logging/setLevel` RPC | the per-request `_meta` key `io.modelcontextprotocol/logLevel` |
//! | Sampling | a server-to-client `sampling/createMessage` mid-request | an `InputRequiredResult` continuation (SEP-2322) |
//! | Roots    | a server-to-client `roots/list` mid-request | the same continuation mechanism |
//!
//! # D-17: probes, not fixtures
//!
//! These capabilities are proved by PROBES, not by fixtures. The fixture grammar
//! supports only `tools_list` and a single `tool_call`, so it cannot express a
//! preceding `logging/setLevel`, a host-handler installation, a server-to-client
//! exchange, or an MRTR gather/resend. **Do not extend the fixture format** —
//! D-17 closed that option.
//!
//! # D-11: no runtime signal
//!
//! Nothing here emits a deprecation warning. Advisory-only deprecation over a
//! twelve-month window means no behavioural change and no new output: a warning
//! on a still-supported capability trains users to ignore warnings and would
//! fire for a year. The capabilities keep working and say nothing.
//!
//! This file therefore carries no Rust deprecation ATTRIBUTE, no
//! warning-level `tracing` emission, and no behavioural difference attributable
//! to deprecation (T-118-35). Those three absences are checked MECHANICALLY by
//! this plan's acceptance criteria, so neither the attribute nor the macro is
//! spelled out even inside prose here: a grep a doc comment can trip is a grep
//! nobody trusts.
//!
//! # Determinism
//!
//! No clocks, no randomness, no filesystem, no outbound network. Every probe and
//! every matrix run built on this target has to be byte-reproducible, so even
//! the `requestState` minting key is pinned rather than generated per process
//! (T-118-37).

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::mrtr::{InputRequest, InputRequests, MrtrSignal};
use pmcp::types::protocol::{Era, ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::sampling::{CreateMessageParams, SamplingMessage, SamplingMessageContent};
use pmcp::types::{
    CompletionCapabilities, LoggingCapabilities, Role, ServerCapabilities, ToolCapabilities,
    ToolInfo,
};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

// ===========================================================================
// The vocabulary. Every string the probes read is spelled ONCE, here, and
// imported by `era_observations.rs` — the two halves of an observation cannot
// drift if there is only one spelling of the token between them.
// ===========================================================================

/// The era target's advertised server name.
pub const ERA_TARGET_NAME: &str = "pmcp-era-target";

/// The era target's advertised version.
pub const ERA_TARGET_VERSION: &str = "0.0.0";

/// A trivial deterministic tool.
///
/// Every generic wire-fact probe (`result.result_type`, `result.server_info`,
/// `result.cache_scope`, `header.mcp_method_and_name`) drives `tools/list` or
/// this tool, so the target needs at least one real dispatch destination that is
/// not entangled with a deprecated capability.
pub const TOOL_ECHO: &str = "era__echo";

/// The Logging evidence tool. See [`LOG_LEVEL_META_KEY`].
pub const TOOL_LOG_EMIT: &str = "dep__log_emit";

/// The Sampling evidence tool.
pub const TOOL_REQUEST_SAMPLING: &str = "dep__request_sampling";

/// The Roots evidence tool.
pub const TOOL_LIST_ROOTS: &str = "dep__list_roots";

/// The per-request `_meta` key that REPLACES the `logging/setLevel` RPC on
/// 2026-07-28.
///
/// Spelled here because the SDK carries no constant for it: it is a v2 schema
/// key pmcp does not yet consume anywhere in `src/`. The 2026-07-28 schema's own
/// description of this key says verbatim *"Replaces the former
/// `logging/setLevel` RPC"* (`118-RESEARCH.md:900` quotes it), which is the
/// entire content of baseline rows ERA-11 and ERA-12.
pub const LOG_LEVEL_META_KEY: &str = "io.modelcontextprotocol/logLevel";

/// The result field carrying the level [`TOOL_LOG_EMIT`] actually observed.
pub const LOG_RESULT_LEVEL_FIELD: &str = "observedLevel";

/// The result field carrying WHERE that level came from.
///
/// The reported SOURCE is what makes `meta.log_level` observable at all: a
/// fixture asserting the mere PRESENCE of a level could not tell an honoured
/// `_meta` key from a server default, and would report the same token under both
/// eras — no observation, in exactly the way D-16 exists to prevent.
pub const LOG_RESULT_SOURCE_FIELD: &str = "levelSource";

/// [`LOG_RESULT_SOURCE_FIELD`] value when the level came from the v2 `_meta`
/// key.
pub const LOG_LEVEL_SOURCE_REQUEST_META: &str = "request-meta";

/// [`LOG_RESULT_SOURCE_FIELD`] value when the `_meta` key was not honoured and
/// the server fell back to its own default.
pub const LOG_LEVEL_SOURCE_SERVER_DEFAULT: &str = "server-default";

/// The level [`TOOL_LOG_EMIT`] reports when nothing set one.
pub const LOG_LEVEL_DEFAULT: &str = "info";

/// The result field the two continuation tools report their disposition in.
pub const DEP_RESULT_STATUS_FIELD: &str = "status";

/// [`DEP_RESULT_STATUS_FIELD`] value when the client declared no capability the
/// v1 mechanism could use.
pub const DEP_STATUS_CAPABILITY_NOT_OFFERED: &str = "capability-not-offered";

/// [`DEP_RESULT_STATUS_FIELD`] value when the operation finished.
pub const DEP_STATUS_COMPLETED: &str = "completed";

/// [`DEP_RESULT_STATUS_FIELD`] value when the peer WAS present, the
/// server-to-client request WAS issued, and it could not be delivered.
///
/// Introduced by phase 118.1 plan 11 because the outcome it names did not exist
/// before: until the peer reached the `StreamableHTTP` dispatch path there was
/// nothing between "the client declared no capability"
/// ([`DEP_STATUS_CAPABILITY_NOT_OFFERED`]) and "the round trip finished"
/// ([`DEP_STATUS_COMPLETED`]).
///
/// It is the honest reading of what the era matrix now measures. The SERVER half
/// of G-3 is closed — `tests/http_peer_roundtrip.rs` drives all three round trips
/// green over v1 HTTP with a client that holds a live SSE stream. But pmcp's OWN
/// `StreamableHttpTransport` client does not hold one: its `start_sse` collects
/// the GET body to completion and parses it in one piece (see that method's own
/// rustdoc: "this body was already read into memory in one piece, not a chunk of
/// a live stream"), so it has no reader registered when the server tries to
/// deliver. The server fails the correlation AT ONCE rather than parking the
/// handler for the dispatch timeout, which is exactly what plan 10's fail-fast
/// path exists to do.
///
/// Reporting it as a STATUS rather than propagating the error is the instrument
/// doing its job: an aborted tool call is not an observation.
pub const DEP_STATUS_NO_LIVE_STREAM: &str = "no-live-stream";

/// The result field carrying the transport error text behind
/// [`DEP_STATUS_NO_LIVE_STREAM`], so the evidence is visible rather than
/// summarised.
pub const DEP_RESULT_DETAIL_FIELD: &str = "detail";

/// [`DEP_RESULT_STATUS_FIELD`] value on the first leg of a v2 continuation.
pub const DEP_STATUS_AWAITING_INPUT: &str = "awaiting-input";

/// The single `inputRequests` key both continuation tools mint under.
const INPUT_REQUEST_KEY: &str = "era_target_input";

/// The loopback address the target binds.
///
/// Port `0` is an EPHEMERAL bind: the kernel picks a free port and
/// [`StreamableHttpServer::start`] reports the real one back AFTER binding. A
/// fixed port would make two runs in the same process — or two agents on the
/// same machine — collide, and would need a readiness sleep on top.
const EPHEMERAL_LOOPBACK: &str = "127.0.0.1:0";

/// The pinned `requestState` minting key.
///
/// Pinned rather than generated so this target is deterministic (T-118-37): an
/// unset key makes the SDK derive a fresh per-process key AND emit a startup
/// warning, and neither belongs in a byte-reproducible observation target. The
/// value is a constant in a test-only server that binds loopback and is torn
/// down at the end of the run; it protects nothing and is not a secret.
const ERA_TARGET_REQUEST_STATE_KEY: [u8; 32] = [0x11; 32];

// ===========================================================================
// The four tools.
// ===========================================================================

/// [`TOOL_ECHO`] — a trivial deterministic dispatch destination.
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // A PLAIN value, deliberately not shaped like a built `CallToolResult`,
        // so the SDK's double-wrap tripwire does not fire.
        Ok(json!({ "echoed": args }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            TOOL_ECHO,
            Some("Echo the arguments back; the era target's neutral dispatch target.".to_string()),
            json!({
                "type": "object",
                "properties": { "value": { "type": "string" } },
            }),
        ))
    }
}

/// The level [`TOOL_LOG_EMIT`] observed, and where it came from.
///
/// The `_meta` key is a **2026-07-28 mechanism**: the v2 schema introduces it as
/// the replacement for the `logging/setLevel` RPC. A v1 request carrying it is a
/// request carrying a key its own era does not define, so it is IGNORED — which
/// is exactly what baseline row ERA-12 records (`ignored` on v1, `honored` on
/// v2), and exactly what makes the pair observable.
fn observed_log_level(extra: &RequestHandlerExtra) -> (String, &'static str) {
    if extra.era() == Some(Era::V2) {
        if let Some(level) = extra
            .request_meta
            .as_ref()
            .and_then(|meta| meta.get(LOG_LEVEL_META_KEY))
            .and_then(Value::as_str)
        {
            return (level.to_string(), LOG_LEVEL_SOURCE_REQUEST_META);
        }
    }
    (
        LOG_LEVEL_DEFAULT.to_string(),
        LOG_LEVEL_SOURCE_SERVER_DEFAULT,
    )
}

/// [`TOOL_LOG_EMIT`] — the Logging evidence tool.
struct LogEmitTool;

#[async_trait]
impl ToolHandler for LogEmitTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let (level, source) = observed_log_level(&extra);
        // ONE emission, at a fixed severity, carrying the observed level as a
        // FIELD. Emitting at the observed severity would mean writing a
        // severity-dispatch match in this file, and the only reason to avoid
        // that is worth stating: D-11 forbids any runtime deprecation signal
        // here, and this file is mechanically checked for the absence of a
        // warning-level emission. The log line is the tool's PURPOSE, not a
        // deprecation notice.
        tracing::info!(
            target: "pmcp.era_target",
            level = %level,
            source = source,
            "era target log emission"
        );
        Ok(json!({
            LOG_RESULT_LEVEL_FIELD: level,
            LOG_RESULT_SOURCE_FIELD: source,
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::new(
                TOOL_LOG_EMIT,
                Some(
                    "Emit one log record and report the level observed plus its source."
                        .to_string(),
                ),
                json!({
                    "type": "object",
                    "properties": { "message": { "type": "string" } },
                }),
            )
            // A declared output schema makes the SDK emit `structuredContent`
            // alongside the text voice, so the probe reads a typed field instead
            // of re-parsing a stringified payload out of `content[0].text`.
            .with_output_schema(json!({
                "type": "object",
                "properties": {
                    LOG_RESULT_LEVEL_FIELD: { "type": "string" },
                    LOG_RESULT_SOURCE_FIELD: { "type": "string" },
                },
                "required": [LOG_RESULT_LEVEL_FIELD, LOG_RESULT_SOURCE_FIELD],
            })),
        )
    }
}

/// The v2 arm shared by both continuation tools.
///
/// Builds the response from the SDK's own MRTR authoring surface — the dispatch
/// layer turns [`MrtrSignal`] into a wire `InputRequiredResult` (`resultType` +
/// `inputRequests` + `requestState`, `src/types/mrtr.rs:686`) and strips the
/// signal key before serialization. A hand-written `serde_json::json!` envelope
/// would drift from the schema silently, which is why none appears here.
fn v2_input_required(extra: &RequestHandlerExtra, request: InputRequest) -> pmcp::Result<Value> {
    // THE RESEND. The client answered the previous round's `inputRequests`, so
    // the operation completes. This second leg is what makes the v2 arm a
    // REPLACEMENT mechanism rather than a refusal.
    if extra.mrtr_continuation().is_some() {
        return Ok(json!({ DEP_RESULT_STATUS_FIELD: DEP_STATUS_COMPLETED }));
    }
    let mut input_requests = InputRequests::new();
    input_requests.insert(INPUT_REQUEST_KEY.to_string(), request);
    let (key, value) = MrtrSignal {
        input_requests,
        continuation: json!({ "round": 1 }),
    }
    .into_meta_entry()
    .map_err(|error| pmcp::Error::internal(error.to_string()))?;
    let mut meta = serde_json::Map::new();
    meta.insert(key, value);
    extra.set_result_meta(meta);
    Ok(json!({ DEP_RESULT_STATUS_FIELD: DEP_STATUS_AWAITING_INPUT }))
}

/// Whether a v1 server-to-client request may be issued at all.
///
/// Consulting the client's DECLARED capabilities before issuing one is
/// load-bearing, not defensive style. A raw era probe declares no client
/// capabilities on v1 and never reads a server-to-client stream, so an issued
/// request would be answered by nobody: the handler would hold open until the
/// probe's request timeout, the observation would come back `Unavailable`, and
/// `Unavailable` is not a difference — it would surface as a permanent false
/// MISSING finding about a server that is behaving correctly.
fn v1_peer(
    extra: &RequestHandlerExtra,
    declared: bool,
) -> Option<&Arc<dyn pmcp::shared::peer::PeerHandle>> {
    if !declared {
        return None;
    }
    extra.peer()
}

/// Report a peer call that was ISSUED but could not be delivered.
///
/// An instrument that aborts is an instrument that measures nothing: a
/// propagated `Err` here surfaces as a failed tool call, which is
/// indistinguishable at the matrix level from a broken target. Returning a
/// STATUS keeps the observation joinable against the baseline and keeps the
/// error text visible under [`DEP_RESULT_DETAIL_FIELD`].
///
/// See [`DEP_STATUS_NO_LIVE_STREAM`] for why this outcome exists at all and why
/// it is not a server defect.
fn undelivered(error: &pmcp::Error) -> Value {
    json!({
        DEP_RESULT_STATUS_FIELD: DEP_STATUS_NO_LIVE_STREAM,
        DEP_RESULT_DETAIL_FIELD: error.to_string(),
    })
}

/// [`TOOL_REQUEST_SAMPLING`] — the Sampling evidence tool.
struct RequestSamplingTool;

#[async_trait]
impl ToolHandler for RequestSamplingTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        if extra.era() == Some(Era::V2) {
            return v2_input_required(&extra, sampling_input_request());
        }
        let declared = extra
            .client_capabilities()
            .is_some_and(|caps| caps.sampling.is_some());
        let Some(peer) = v1_peer(&extra, declared) else {
            return Ok(json!({ DEP_RESULT_STATUS_FIELD: DEP_STATUS_CAPABILITY_NOT_OFFERED }));
        };
        match peer.sample(sampling_params()).await {
            Ok(completion) => Ok(json!({
                DEP_RESULT_STATUS_FIELD: DEP_STATUS_COMPLETED,
                "model": completion.model,
            })),
            Err(error) => Ok(undelivered(&error)),
        }
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            TOOL_REQUEST_SAMPLING,
            Some(
                "Reach the Sampling capability: a v1 server-to-client request, or a v2 \
                 input-required continuation."
                    .to_string(),
            ),
            json!({ "type": "object", "properties": {} }),
        ))
    }
}

/// [`TOOL_LIST_ROOTS`] — the Roots evidence tool.
struct ListRootsTool;

#[async_trait]
impl ToolHandler for ListRootsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        if extra.era() == Some(Era::V2) {
            return v2_input_required(&extra, InputRequest::ListRoots);
        }
        let declared = extra
            .client_capabilities()
            .is_some_and(|caps| caps.roots.is_some());
        let Some(peer) = v1_peer(&extra, declared) else {
            return Ok(json!({ DEP_RESULT_STATUS_FIELD: DEP_STATUS_CAPABILITY_NOT_OFFERED }));
        };
        match peer.list_roots().await {
            Ok(roots) => Ok(json!({
                DEP_RESULT_STATUS_FIELD: DEP_STATUS_COMPLETED,
                "rootCount": roots.roots.len(),
            })),
            Err(error) => Ok(undelivered(&error)),
        }
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            TOOL_LIST_ROOTS,
            Some(
                "Reach the Roots capability: a v1 server-to-client request, or a v2 \
                 input-required continuation."
                    .to_string(),
            ),
            json!({ "type": "object", "properties": {} }),
        ))
    }
}

/// The fixed sampling params both arms use.
///
/// Deterministic by construction: one user message with fixed text, no
/// temperature, no tools, no metadata.
fn sampling_params() -> CreateMessageParams {
    CreateMessageParams::new(vec![SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: "era target sampling probe".to_string(),
            meta: None,
        },
    )])
}

/// The Sampling-kind [`InputRequest`], built from the SDK's typed variant.
fn sampling_input_request() -> InputRequest {
    InputRequest::Sampling(Box::new(sampling_params()))
}

// ===========================================================================
// Construction and spawn.
// ===========================================================================

/// The capability structure the era target advertises.
fn era_target_capabilities() -> ServerCapabilities {
    let mut capabilities = ServerCapabilities::default();
    // `list_changed` stays unset: advertising it opts a v2-capable server into
    // serving `subscriptions/listen` and makes `build()` emit an instance-local
    // delivery warning, neither of which this target wants.
    capabilities.tools = Some(ToolCapabilities { list_changed: None });
    capabilities.logging = Some(LoggingCapabilities::default());
    capabilities.completions = Some(CompletionCapabilities::default());
    capabilities
}

/// Build the era target [`Server`].
///
/// Advertises BOTH [`pmcp::LATEST_PROTOCOL_VERSION`] and
/// [`PROTOCOL_VERSION_2026_07_28`], so v2 is reachable by negotiation without
/// any reference server being touched.
///
/// # Errors
///
/// Propagates a [`pmcp::Server::builder`] build failure.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_target::build_era_target_server;
///
/// let server = build_era_target_server();
/// assert!(server.is_ok());
/// ```
pub fn build_era_target_server() -> pmcp::Result<Server> {
    Server::builder()
        .name(ERA_TARGET_NAME)
        .version(ERA_TARGET_VERSION)
        .capabilities(era_target_capabilities())
        // THE ONE CALL that makes v2 reachable. `src/server/mod.rs:3087`.
        .with_supported_protocol_versions([
            ProtocolVersion(pmcp::LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .with_request_state_key(ERA_TARGET_REQUEST_STATE_KEY)
        .tool_arc(TOOL_ECHO, Arc::new(EchoTool) as Arc<dyn ToolHandler>)
        .tool_arc(TOOL_LOG_EMIT, Arc::new(LogEmitTool) as Arc<dyn ToolHandler>)
        .tool_arc(
            TOOL_REQUEST_SAMPLING,
            Arc::new(RequestSamplingTool) as Arc<dyn ToolHandler>,
        )
        .tool_arc(
            TOOL_LIST_ROOTS,
            Arc::new(ListRootsTool) as Arc<dyn ToolHandler>,
        )
        .build()
}

/// A running era target, with its bound address and a teardown handle.
///
/// Tearing down is automatic: [`Drop`] aborts the serve task, so a test that
/// panics cannot leak a listening socket into the next test in the same process
/// (T-118-70).
#[derive(Debug)]
pub struct EraTargetHandle {
    addr: SocketAddr,
    url: url::Url,
    task: Option<JoinHandle<()>>,
}

impl EraTargetHandle {
    /// The BOUND address, with the kernel-assigned port already resolved.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The MCP endpoint URL.
    #[must_use]
    pub fn url(&self) -> &url::Url {
        &self.url
    }

    /// Tear the target down explicitly.
    ///
    /// Consumes the handle, so [`Drop`] does the abort; the method exists so a
    /// caller can say when rather than relying on scope.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for EraTargetHandle {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Serve [`build_era_target_server`] over streamable HTTP on an ephemeral
/// loopback port.
///
/// Uses [`StreamableHttpServerConfig::default()`] — a STATEFUL config with a
/// live session-id generator — and NOT `::stateless()`. `::stateless()` is a
/// BUILD-TIME config, so a target built with it never exercises the per-request
/// era gate or the v1 session path at all, and `header.mcp_session_id` would
/// observe the same thing under both eras.
///
/// Binding completes BEFORE the handle is returned, so a caller needs no
/// readiness sleep.
///
/// # Errors
///
/// Returns the build, bind or URL-parse failure as a `String`.
///
/// # Examples
///
/// ```
/// # async fn demo() {
/// use pmcp_team_servers::conformance::era_target::spawn_era_target;
///
/// let target = spawn_era_target().await.expect("the era target binds");
/// assert_ne!(target.addr().port(), 0);
/// target.shutdown();
/// # }
/// ```
pub async fn spawn_era_target() -> Result<EraTargetHandle, String> {
    let server =
        build_era_target_server().map_err(|error| format!("era target build failed: {error}"))?;
    let addr: SocketAddr = EPHEMERAL_LOOPBACK
        .parse()
        .map_err(|error| format!("era target bind address is unparseable: {error}"))?;
    let shared = Arc::new(tokio::sync::Mutex::new(server));
    let http =
        StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    let (bound, task) = http
        .start()
        .await
        .map_err(|error| format!("era target failed to bind: {error}"))?;
    let url = url::Url::parse(&format!("http://{bound}"))
        .map_err(|error| format!("era target endpoint is not a URL: {error}"))?;
    Ok(EraTargetHandle {
        addr: bound,
        url,
        task: Some(task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_builds_with_both_eras_in_its_accept_list() {
        let server = build_era_target_server();
        assert!(server.is_ok(), "the era target must build");
    }

    #[test]
    fn the_capability_structure_declares_tools_logging_and_completions() {
        let capabilities = era_target_capabilities();
        assert!(capabilities.tools.is_some());
        assert!(capabilities.logging.is_some());
        assert!(capabilities.completions.is_some());
        assert!(
            capabilities
                .tools
                .as_ref()
                .and_then(|tools| tools.list_changed)
                .is_none(),
            "listChanged must stay unset: it opts a v2 server into subscriptions/listen"
        );
    }

    #[test]
    fn the_sampling_input_request_is_of_the_sampling_kind() {
        assert_eq!(
            sampling_input_request().kind(),
            pmcp::types::mrtr::InputRequestKind::Sampling
        );
        assert_eq!(
            InputRequest::ListRoots.kind(),
            pmcp::types::mrtr::InputRequestKind::Roots
        );
    }

    #[test]
    fn the_sampling_params_are_deterministic() {
        let first = serde_json::to_value(sampling_params()).expect("params serialize");
        let second = serde_json::to_value(sampling_params()).expect("params serialize");
        assert_eq!(first, second, "the era target must be byte-reproducible");
    }

    /// The ephemeral bind is REAL: the kernel picked the port, and the handle
    /// reports the port that was actually bound.
    #[tokio::test]
    async fn spawning_binds_an_ephemeral_port_and_tears_down() {
        let target = spawn_era_target().await.expect("the era target binds");
        assert_ne!(
            target.addr().port(),
            0,
            "start() must report the KERNEL-ASSIGNED port, not the requested 0"
        );
        assert_eq!(target.addr().ip().to_string(), "127.0.0.1");
        assert_eq!(target.url().scheme(), "http");
        assert_eq!(target.url().port(), Some(target.addr().port()));
        target.shutdown();
    }

    /// Two targets in one process get DIFFERENT ports, which is the property a
    /// fixed port would destroy.
    #[tokio::test]
    async fn two_targets_do_not_collide() {
        let first = spawn_era_target().await.expect("the first target binds");
        let second = spawn_era_target().await.expect("the second target binds");
        assert_ne!(first.addr().port(), second.addr().port());
        first.shutdown();
        second.shutdown();
    }

    /// THE RESERVED-KEY TRIPWIRE, and it is non-circular.
    ///
    /// `era_probe::build_probe_body` must spell the three reserved `_meta` keys
    /// the way the SDK's own era gate reads them: the gate requires the
    /// `MCP-Protocol-Version` HEADER and the `_meta` value to AGREE and rejects a
    /// header-only claim with `HEADER_MISMATCH`. A misspelled key would still
    /// produce two DIFFERING observation maps (every v2 row would just become the
    /// same rejection token), so the Task-3 anti-vacuity control cannot catch it.
    /// This test can: it compares the probe's body against the SDK SERVER's
    /// behaviour, not against itself.
    #[tokio::test]
    async fn a_v2_probe_body_is_accepted_by_the_live_target() {
        use crate::conformance::era_probe::{EraProbeClient, V2HeaderMode};

        let target = spawn_era_target().await.expect("the era target binds");
        let probe = EraProbeClient::new(target.url().as_str()).expect("the probe client builds");
        let outcome = probe
            .raw_jsonrpc_probe_with_session(
                "tools/list",
                "",
                json!({}),
                Era::V2,
                V2HeaderMode::Standard,
                None,
            )
            .await
            .expect("the target answers");
        assert!(
            outcome.is_result(),
            "a conformant v2 tools/list must be SERVED. error_code={:?} status={} — \
             a HEADER_MISMATCH here means the reserved `_meta` key spellings in \
             era_probe::build_probe_body no longer match the SDK's era gate.",
            outcome.error_code,
            outcome.http_status
        );
        target.shutdown();
    }

    /// Both continuation tools answer v1 DETERMINISTICALLY rather than issuing a
    /// server-to-client request nobody will answer.
    #[tokio::test]
    async fn the_v1_arms_never_block_on_an_unanswerable_request() {
        use crate::conformance::era_probe::{EraProbeClient, V2HeaderMode};

        let target = spawn_era_target().await.expect("the era target binds");
        let probe = EraProbeClient::new(target.url().as_str()).expect("the probe client builds");
        // Establish the v1 session first: a stateful v1 server refuses every
        // non-initialization request that arrives without one.
        let init = probe
            .raw_jsonrpc_probe_with_session(
                "initialize",
                "",
                json!({
                    "protocolVersion": pmcp::LATEST_PROTOCOL_VERSION,
                    "clientInfo": { "name": "era-target-test", "version": "0" },
                    "capabilities": {},
                }),
                Era::V1,
                V2HeaderMode::Standard,
                None,
            )
            .await
            .expect("the target answers initialize");
        let session = init.session_header.clone();

        for tool in [TOOL_REQUEST_SAMPLING, TOOL_LIST_ROOTS] {
            let outcome = probe
                .raw_jsonrpc_probe_with_session(
                    "tools/call",
                    tool,
                    json!({ "name": tool, "arguments": {} }),
                    Era::V1,
                    V2HeaderMode::Standard,
                    session.as_deref(),
                )
                .await
                .expect("the target answers the tool call");
            let rendered = serde_json::to_string(&outcome.result).unwrap_or_default();
            assert!(
                rendered.contains(DEP_STATUS_CAPABILITY_NOT_OFFERED),
                "{tool} must answer a capability-less v1 caller promptly; got {rendered}"
            );
        }
        target.shutdown();
    }
}
