//! Conformance helpers for proving the `tasks/*` wire surface round-trips
//! through the SDK's real client deserialization types.
//!
//! **Why this module exists.** The tools-as-tasks incident (Phase 101) was
//! caused by hand-written `tasks/*` JSON diverging from the typed structs the
//! client deserializes into. A regression test fed the helper an
//! *author-written fixture* and passed green while the live wire path failed.
//! The lesson, adopted as an acceptance gate: for protocol-shape requirements,
//! "resolved" is gated on feeding the **actual server dispatch output** through
//! the real client type, never a hand-built value.
//!
//! [`assert_roundtrips_through_client`] is the primitive that enforces this. It
//! is feature-gated behind `testing` (folded into the `full` feature set) so it
//! is available to the integration tests / examples and the quality gate, but
//! omitted from lean default release builds.

use serde::de::DeserializeOwned;

/// Assert that real server dispatch output deserializes into the client type
/// `T`, panicking with a diagnostic message if it does not.
///
/// Feed this the `serde_json::Value` carried by an actual
/// `ResponsePayload::Result(..)` produced by
/// [`ServerCore::handle_request`](crate::server::core::ProtocolHandler::handle_request)
/// — **never** an author-written fixture. If `real_dispatch_output` does not
/// deserialize into `T`, the call panics with a message naming `T`, the serde
/// error, and the pretty-printed offending output.
///
/// # Type Parameters
///
/// - `T`: a client-facing wire type such as
///   [`GetTaskResult`](crate::types::tasks::GetTaskResult),
///   [`CallToolResult`](crate::types::CallToolResult), or
///   [`CreateTaskResult`](crate::types::tasks::CreateTaskResult). Because serde
///   ignores unknown fields by default, the extra `_meta` carried by the
///   create envelope deserializes cleanly into `CreateTaskResult`.
///
/// # Panics
///
/// Panics if `real_dispatch_output` cannot be deserialized into `T`. This is
/// the intended behavior in a test context: a deliberately-wrong wire shape
/// (e.g. a flat `Task` where a `{ "task": ... }` wrapper is expected) makes the
/// helper fail loudly.
///
/// # Examples
///
/// ```rust
/// use pmcp::testing::assert_roundtrips_through_client;
/// use pmcp::types::tasks::{GetTaskResult, Task, TaskStatus};
///
/// // A correctly-shaped `tasks/get` payload wraps the task under `task`.
/// let task = Task::new("t-1", TaskStatus::Working)
///     .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z");
/// let dispatch_output = serde_json::to_value(GetTaskResult::new(task)).unwrap();
///
/// // Deserializes cleanly into the client type — returns normally.
/// assert_roundtrips_through_client::<GetTaskResult>(dispatch_output);
/// ```
pub fn assert_roundtrips_through_client<T>(real_dispatch_output: serde_json::Value)
where
    T: DeserializeOwned,
{
    // Pre-render the diagnostic before moving the owned value into `from_value`
    // (the value is consumed by the deserialize, so it is genuinely owned — not
    // merely borrowed).
    let pretty = serde_json::to_string_pretty(&real_dispatch_output)
        .unwrap_or_else(|_| real_dispatch_output.to_string());
    if let Err(error) = serde_json::from_value::<T>(real_dispatch_output) {
        panic!(
            "dispatch output does not deserialize into `{}`: {error}\noffending output was:\n{pretty}",
            std::any::type_name::<T>(),
        );
    }
}

/// Reserved `_meta` key carrying the per-request protocol version.
///
/// Re-exported so tests read the crate's constant instead of re-spelling the
/// `io.modelcontextprotocol/*` string and silently drifting from it.
pub const META_PROTOCOL_VERSION: &str =
    crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;

/// Reserved `_meta` key carrying the per-request client identity.
pub const META_CLIENT_INFO: &str = crate::types::protocol::context::RESERVED_CLIENT_INFO_KEY;

/// Reserved `_meta` key carrying the per-request client capabilities.
pub const META_CLIENT_CAPABILITIES: &str =
    crate::types::protocol::context::RESERVED_CLIENT_CAPABILITIES_KEY;

/// Reserved `result._meta` key carrying the server's identity on a v2 response.
///
/// The RESPONSE-side sibling of the three request-side keys above. Re-exported
/// for the same reason: a live-HTTP test asserting the v2 envelope placement must
/// read the crate's constant, not re-spell `io.modelcontextprotocol/serverInfo`
/// and drift from it.
#[cfg(all(not(target_arch = "wasm32"), feature = "streamable-http"))]
pub const META_SERVER_INFO: &str = crate::server::core::RESERVED_SERVER_INFO_KEY;

/// The opening marker of the `Mcp-Name` base64 sentinel form.
pub const HEADER_SENTINEL_PREFIX: &str = crate::types::mrtr::HEADER_SENTINEL_PREFIX;

/// The closing marker of the `Mcp-Name` base64 sentinel form.
pub const HEADER_SENTINEL_SUFFIX: &str = crate::types::mrtr::HEADER_SENTINEL_SUFFIX;

/// Encode a value for the `Mcp-Name` header, using the PRODUCTION codec.
///
/// **Why this wrapper exists.** The codec in `crate::types::mrtr` is `pub(crate)`
/// (Phase-113 D-10 keeps the MRTR plumbing off the public API), so the v2 test
/// harness previously carried a hand-copied mirror of it — and the mirror had
/// already drifted, omitting the `MAX_HEADER_VALUE_LEN` term from its passthrough
/// predicate. That meant the harness could emit a raw oversized header the real
/// encoder would have sentinel-encoded, so every test built on it was validating
/// the harness against itself rather than against the shipped encoder.
///
/// A `pub use` of a `pub(crate)` item does not compile (E0365), so this thin
/// wrapper is the seam. Six Phase-113 plans build their requests through it.
#[must_use]
pub fn encode_mcp_name(value: &str) -> String {
    crate::types::mrtr::encode_header_value(value)
}

/// Decode an `Mcp-Name` header value, using the PRODUCTION codec.
///
/// Returns `None` for a malformed sentinel or an over-long value. See
/// [`encode_mcp_name`] for why this wrapper exists.
#[must_use]
pub fn decode_mcp_name(raw: &str) -> Option<String> {
    crate::types::mrtr::decode_header_value(raw)
}

/// Where `method` keeps its `Mcp-Name` routing value, from the PRODUCTION
/// combined lookup (Phase 114, DQ4).
///
/// `Some("name")` for `tools/call` / `prompts/get`, `Some("uri")` for
/// `resources/read`, `Some("taskId")` for `tasks/get` / `tasks/update` /
/// `tasks/cancel`, and `None` for every other method (whose `Mcp-Name` is the
/// empty string).
///
/// **Why this wrapper exists.** Both method tables are `pub(crate)` (Phase-113
/// D-10 keeps the MRTR plumbing off the public API) and a `pub use` of a
/// `pub(crate)` item does not compile (E0365). Paired with
/// [`method_is_mrtr_eligible`] it lets an integration test state the ONE
/// property that keeps the two tables from being merged back together: the
/// tasks methods are name-bearing and NOT MRTR-eligible.
#[must_use]
pub fn routing_name_key(method: &str) -> Option<&'static str> {
    crate::types::mrtr::name_bearing_key(method)
}

/// Whether `method` may carry an `input_required` result, from the PRODUCTION
/// `MRTR_METHODS` table.
///
/// See [`routing_name_key`] for why this wrapper exists and what the pair is
/// for. This reads `MRTR_METHODS` and ONLY `MRTR_METHODS`: making a tasks method
/// eligible here would route `tasks/update` through `splice_mrtr_params`, which
/// strips `inputResponses` unconditionally — i.e. deletes that request's entire
/// payload.
#[must_use]
pub fn method_is_mrtr_eligible(method: &str) -> bool {
    crate::types::mrtr::mrtr_eligible(method)
}

/// The PRODUCTION `-32601` message body a v2 caller receives for a `tasks/*`
/// method protocol version 2026-07-28 RETIRED (Phase 114, TASK-03).
///
/// The wire message is `format!("{method} {V2_TASKS_METHOD_RETIRED}")`, so a
/// test asserts the method prefix and this suffix separately.
///
/// **Why this re-export exists.** The constant lives in the `pub(crate)`
/// `server::task_dispatch` module, and the suites that assert on the refusal
/// cross a real HTTP boundary — so without it every one of them would hand-copy
/// the sentence. This file already records what a hand-copied mirror costs (the
/// `Mcp-Name` encoder mirror had silently drifted from the shipped codec), and a
/// refusal message that drifts is worse than most: it is the ONLY signal telling
/// a caller which of the three `-32601` conditions it hit.
///
/// `#[cfg(not(target_arch = "wasm32"))]` because the whole task subsystem is.
#[cfg(not(target_arch = "wasm32"))]
pub const V2_TASKS_METHOD_RETIRED: &str = crate::server::task_dispatch::V2_TASKS_METHOD_RETIRED;

/// Mint a `requestState` continuation token with the PRODUCTION codec
/// (Phase 113, HTTP-02).
///
/// **Why this wrapper exists.** `RequestStateCodec` is `pub(crate)` (Phase-113
/// D-10 keeps the MRTR plumbing off the public API), so an integration test
/// cannot construct one — and a test that hand-rolled the token layout would be
/// validating itself rather than the shipped codec. This is the same one-hop
/// seam [`encode_mcp_name`] provides for the `Mcp-Name` codec.
///
/// `key` is the SAME 32 bytes the server under test was built with via
/// [`ServerBuilder::with_request_state_key`](crate::ServerBuilder::with_request_state_key),
/// so tests configure the key through the builder and never mutate
/// `PMCP_REQUEST_STATE_KEY` (which is process-global and therefore
/// order-dependent under parallel test threads).
///
/// `params` MUST be the params DISPATCH derives from the typed request, because
/// that is what the AEAD binds to:
///
/// | method | params |
/// |--------|--------|
/// | `tools/call` | `{"name": …, "arguments": …}` |
/// | `prompts/get` | `{"name": …, "arguments": …}` |
/// | `resources/read` | `{"uri": …}` |
///
/// A `ttl` of zero mints an ALREADY-EXPIRED token (`exp == now`), which is how a
/// test exercises the expiry verdict deterministically instead of sleeping.
///
/// Returns `None` if the codec refuses the key, cannot seal the state, or cannot
/// BIND the request — the last being params nested past the canonicalization depth
/// cap, which the production mint path refuses for the same reason (D-113-M).
///
/// # This seam mints a KINDS-LESS continuation, deliberately
///
/// The production mint path seals the server's record of which
/// [`InputRequestKind`](crate::types::mrtr::InputRequestKind) it requested under
/// each `inputRequests` key, so ingress can type the client's answers
/// kind-directed (D-113-O). This helper has no `inputRequests` to derive that
/// from — it mints a bare continuation for a test that wants to control `state`,
/// `round` and `ttl` — so it passes `None`, which selects the documented
/// untagged-decode degradation on
/// [`Continuation::kinds`](crate::server::request_state::Continuation).
///
/// That keeps this function's PUBLIC SIGNATURE byte-unchanged and keeps every
/// existing caller's behaviour byte-identical. It is not a hole: a token minted
/// here is minted with the caller's OWN key, so it grants no capability the
/// caller did not already hold. A test that wants to exercise kind-directed
/// typing drives a real server and answers the `inputRequests` it actually
/// returned.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[must_use]
pub fn mint_request_state(
    key: &[u8; 32],
    ttl: std::time::Duration,
    principal: &str,
    method: &str,
    params: &serde_json::Value,
    state: &serde_json::Value,
    round: u8,
) -> Option<String> {
    let codec = crate::server::request_state::RequestStateCodec::new(key, ttl).ok()?;
    let binding =
        crate::server::request_state::RequestBinding::from_request(principal, method, params)
            .ok()?;
    codec.mint(state, &binding, round, None).ok()
}

/// Open a `requestState` token with the PRODUCTION codec, returning
/// `(continuation state, round)`.
///
/// The inverse of [`mint_request_state`], and the only way an integration test
/// can assert that a token the SERVER minted carries the round it should — the
/// D-15 expiry path must PRESERVE the round rather than reset it (T-113-49).
///
/// Returns `None` for any verdict other than "authentic and live": an
/// unknown key, a failed tag check, or an expired token all yield `None`, as does
/// a request too deeply nested to bind (D-113-M).
/// `params` follows the same rule as [`mint_request_state`].
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[must_use]
pub fn open_request_state(
    key: &[u8; 32],
    principal: &str,
    method: &str,
    params: &serde_json::Value,
    token: &str,
) -> Option<(serde_json::Value, u8)> {
    let codec = crate::server::request_state::RequestStateCodec::new(
        key,
        std::time::Duration::from_secs(1),
    )
    .ok()?;
    let binding =
        crate::server::request_state::RequestBinding::from_request(principal, method, params)
            .ok()?;
    match codec.verify(token, &binding) {
        crate::server::request_state::Verdict::Ok(continuation) => {
            Some((continuation.state, continuation.round))
        },
        _ => None,
    }
}

/// The principal a server with NO auth provider binds continuations to.
///
/// Re-exported so a test spells the anonymous principal through the crate's own
/// constant instead of hard-coding the empty string.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub const ANONYMOUS_PRINCIPAL: &str = crate::server::core::ANONYMOUS_PRINCIPAL;

/// The reserved TOP-LEVEL result key carrying an `inputRequests` map.
///
/// Re-exported for the same reason as [`META_SERVER_INFO`]: a test asserting on
/// raw response bytes must read the crate's own constant rather than re-spell
/// the wire key and drift from it. `INPUT_REQUESTS_KEY` is `pub(crate)` in
/// `crate::types::mrtr`, which is where BOTH legitimate minters of this key —
/// the MRTR egress and the v2 tasks dispatch — read it from.
pub const RESERVED_INPUT_REQUESTS: &str = crate::types::mrtr::INPUT_REQUESTS_KEY;

/// The reserved TOP-LEVEL result key carrying an MRTR continuation token.
///
/// Unlike [`RESERVED_INPUT_REQUESTS`] this key has exactly ONE legitimate
/// minter — the MRTR egress. The tasks surface has no continuation token (the
/// persisted task record replaces the sealed continuation, D-17), so a tasks
/// result carrying it is stripped.
pub const RESERVED_REQUEST_STATE: &str = crate::types::mrtr::REQUEST_STATE_KEY;

// ===========================================================================
// The FOUR `inputResponses` denial-of-service bounds (Phase 114, plan 14).
//
// Re-exported for one reason: a test that asserts "the bound fires" must build
// its payload FROM the production limit, never from a number typed into the
// test. A hand-typed `65` is correct only until someone changes the constant,
// and the failure mode is silent — the payload stops crossing the bound and the
// test keeps passing while asserting nothing.
//
// All four are `pub(crate)` in `crate::types::mrtr`, which owns them because the
// SAME four guard both the MRTR ingress and the `tasks/update` route. There is
// deliberately NO re-export of the fifth, `MAX_REQUEST_STATE_LEN`: it bounds the
// continuation TOKEN, `tasks/update` carries none, and exporting it beside these
// four is how a fifth test gets written asserting a bound the route correctly
// does not enforce.
// ===========================================================================

/// Upper bound on the number of `inputResponses` entries one request may carry.
pub const MAX_INPUT_RESPONSES: usize = crate::types::mrtr::MAX_INPUT_RESPONSES;

/// Upper bound on ONE serialized `inputResponses` entry, in bytes.
pub const MAX_INPUT_RESPONSE_BYTES: usize = crate::types::mrtr::MAX_INPUT_RESPONSE_BYTES;

/// Upper bound on the TOTAL serialized size of all `inputResponses` entries.
pub const MAX_INPUT_RESPONSES_TOTAL_BYTES: usize =
    crate::types::mrtr::MAX_INPUT_RESPONSES_TOTAL_BYTES;

/// Upper bound on the nesting DEPTH of ONE `inputResponses` entry.
pub const MAX_INPUT_RESPONSE_DEPTH: usize = crate::types::mrtr::MAX_INPUT_RESPONSE_DEPTH;

#[cfg(not(target_arch = "wasm32"))]
pub use reserved_fields::{
    v1_result_envelope, v2_result_envelope, CapturedWarning, EnvelopeOutcome, ReservedFieldEgress,
};

/// The seam an integration test uses to drive the PRODUCTION v2 result envelope
/// and observe BOTH halves of what it did — the bytes it emitted and the
/// `tracing` warnings it logged (Phase 114, plan 10).
///
/// **Why this seam exists.** `server::core::inject_v2_result_envelope` and its
/// reserved-field registry `own_reserved_result_fields` are `pub(crate)`, and a
/// `pub use` of a `pub(crate)` item does not compile (E0365) — the same wall
/// [`encode_mcp_name`] and [`routing_name_key`] already work around. A test that
/// hand-rolled the registry's strip rules would be validating itself rather than
/// the shipped egress.
///
/// **Why the warnings are captured rather than left to a global subscriber.**
/// The registry's removals are DELIBERATELY silent on the wire — a
/// `tracing::warn!`, never an error — which is precisely how a spec-required
/// field could be deleted from every v2 `tasks/get` while every integration test
/// asserting "the request succeeded" stayed green. A test that cannot see the
/// warning cannot tell "the key was never there" from "the key was removed", so
/// the two facts are returned together.
#[cfg(not(target_arch = "wasm32"))]
mod reserved_fields {
    use std::sync::{Arc, Mutex};

    /// One `tracing` event the production envelope emitted.
    ///
    /// The registry logs `target: "mcp.v2"` with a `field` naming the reserved
    /// key it acted on, so a test can attribute a warning to a specific key
    /// rather than matching on prose.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedWarning {
        /// The `tracing` target — `mcp.v2` for every registry event.
        pub target: String,
        /// The `field = …` value naming the reserved key, when the event set one.
        pub field: Option<String>,
        /// The event message, verbatim.
        pub message: String,
    }

    /// What the production envelope produced: the response BYTES plus every
    /// warning it logged while producing them.
    #[derive(Debug, Clone)]
    pub struct EnvelopeOutcome {
        /// The serialized JSON-RPC response.
        ///
        /// Assert on THIS, not on a parsed struct: a struct with
        /// `skip_serializing_if` would hide a deleted key behind a `None`.
        pub bytes: String,
        /// Every warning the envelope logged, in emission order.
        pub warnings: Vec<CapturedWarning>,
    }

    impl EnvelopeOutcome {
        /// Whether a warning naming `field` fired.
        #[must_use]
        pub fn warned_about(&self, field: &str) -> bool {
            self.warnings
                .iter()
                .any(|warning| warning.field.as_deref() == Some(field))
        }
    }

    /// Which egress minted the reserved result fields on the response under test.
    ///
    /// This is the TEST-side spelling of the production
    /// `server::core::ReservedFieldOwner`, paired with the disposition that
    /// egress really selects, so a test cannot construct a pairing production
    /// never produces:
    ///
    /// | variant | disposition | owner |
    /// |---|---|---|
    /// | [`NoEgress`](Self::NoEgress) | `complete` | none — both reserved keys stripped |
    /// | [`Mrtr`](Self::Mrtr) | `input_required` | the MRTR egress |
    /// | [`TasksDispatch`](Self::TasksDispatch) | `complete` | the v2 tasks dispatch |
    ///
    /// [`TasksDispatch`](Self::TasksDispatch) pairs with `complete` and not with
    /// `input_required` because a v2 `tasks/get` on an `input_required` TASK is
    /// still a COMPLETE JSON-RPC result — the task is what is waiting, not the
    /// request. That is the pairing the reserved-field registry got wrong.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ReservedFieldEgress {
        /// No egress minted the reserved fields.
        NoEgress,
        /// The MRTR egress minted `requestState` + `inputRequests`.
        Mrtr,
        /// The v2 tasks dispatch minted `inputRequests`.
        TasksDispatch,
    }

    /// Run the PRODUCTION v2 result envelope over `result`.
    ///
    /// `result` is the raw result value a dispatch produced — for the tasks
    /// cases, the flat `tasks/get` shape the ext-tasks schema specifies
    /// (`GetTaskResult` is `Result & (WorkingTask | InputRequiredTask | …)`, so
    /// `inputRequests` is a TOP-LEVEL result key, not a nested one).
    #[must_use]
    pub fn v2_result_envelope(
        result: serde_json::Value,
        egress: ReservedFieldEgress,
    ) -> EnvelopeOutcome {
        run_envelope(result, Some(crate::types::protocol::Era::V2), egress)
    }

    /// Run the PRODUCTION envelope over a **v1** result.
    ///
    /// The envelope is v2-only, so this must leave the result byte-identical —
    /// including a `inputRequests` key a v1 tasks path put there. Pins the early
    /// return in `inject_v2_result_envelope`.
    #[must_use]
    pub fn v1_result_envelope(result: serde_json::Value) -> EnvelopeOutcome {
        run_envelope(result, Some(crate::types::protocol::Era::V1), egress_none())
    }

    fn egress_none() -> ReservedFieldEgress {
        ReservedFieldEgress::NoEgress
    }

    fn run_envelope(
        result: serde_json::Value,
        era: Option<crate::types::protocol::Era>,
        egress: ReservedFieldEgress,
    ) -> EnvelopeOutcome {
        // The two facts travel to the registry exactly as production pairs them:
        // MRTR selects `input_required` and owns both reserved keys; the tasks
        // dispatch selects `complete` and owns `inputRequests` alone.
        let (disposition, owner) = match egress {
            ReservedFieldEgress::NoEgress => (
                crate::server::core::ResponseDisposition::Complete,
                crate::server::core::ReservedFieldOwner::None,
            ),
            ReservedFieldEgress::Mrtr => (
                crate::server::core::ResponseDisposition::InputRequired,
                crate::server::core::ReservedFieldOwner::Mrtr,
            ),
            ReservedFieldEgress::TasksDispatch => (
                crate::server::core::ResponseDisposition::Complete,
                crate::server::core::ReservedFieldOwner::TasksDispatch,
            ),
        };
        let context = era.map(|era| {
            let version = match era {
                crate::types::protocol::Era::V2 => {
                    crate::types::protocol::PROTOCOL_VERSION_2026_07_28
                },
                crate::types::protocol::Era::V1 => crate::LATEST_PROTOCOL_VERSION,
            };
            crate::types::protocol::ProtocolContext::new(
                era,
                crate::types::protocol::ProtocolVersion(version.to_string()),
            )
        });
        let server_info =
            crate::types::Implementation::new("reserved-field-registry-probe", "1.0.0");
        let mut response = crate::types::jsonrpc::JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: crate::types::RequestId::from(1i64),
            payload: crate::types::jsonrpc::ResponsePayload::Result(result),
        };

        let events = Arc::new(Mutex::new(Vec::new()));
        {
            let collector = capture::Collector {
                events: Arc::clone(&events),
            };
            tracing::subscriber::with_default(collector, || {
                crate::server::core::inject_v2_result_envelope(
                    &mut response,
                    context.as_ref(),
                    &server_info,
                    disposition,
                    owner,
                    // This probe measures the RESERVED-FIELD registry, not the
                    // caching projection (which `types::caching`'s own unit
                    // tests and `mod inject_v2_result_envelope` cover), so it
                    // deliberately drives the non-cacheable arm.
                    crate::types::caching::Cacheable::No,
                );
            });
        }
        let warnings = std::mem::take(&mut *events.lock().unwrap_or_else(|e| e.into_inner()));
        EnvelopeOutcome {
            bytes: serde_json::to_string(&response).unwrap_or_default(),
            warnings,
        }
    }

    /// A minimal thread-local `tracing` subscriber.
    ///
    /// Hand-written against `tracing` itself rather than pulled from
    /// `tracing-subscriber`, so this seam carries no feature gate beyond the one
    /// `server::core` already has, and no new dependency.
    /// [`tracing::subscriber::with_default`] scopes it to one closure on one
    /// thread, so parallel test threads cannot observe each other's events.
    mod capture {
        use super::{Arc, CapturedWarning, Mutex};

        pub(super) struct Collector {
            pub(super) events: Arc<Mutex<Vec<CapturedWarning>>>,
        }

        #[derive(Default)]
        struct Fields {
            message: String,
            field: Option<String>,
        }

        impl tracing::field::Visit for Fields {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.assign(field.name(), value.to_string());
            }

            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.assign(field.name(), format!("{value:?}"));
            }
        }

        impl Fields {
            fn assign(&mut self, name: &str, value: String) {
                match name {
                    "message" => self.message = value,
                    "field" => self.field = Some(value),
                    _ => {},
                }
            }
        }

        impl tracing::Subscriber for Collector {
            fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }

            fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {
            }

            fn event(&self, event: &tracing::Event<'_>) {
                let mut fields = Fields::default();
                event.record(&mut fields);
                let captured = CapturedWarning {
                    target: event.metadata().target().to_string(),
                    field: fields.field,
                    message: fields.message,
                };
                if let Ok(mut events) = self.events.lock() {
                    events.push(captured);
                }
            }

            fn enter(&self, _span: &tracing::span::Id) {}

            fn exit(&self, _span: &tracing::span::Id) {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::assert_roundtrips_through_client;
    use crate::types::tasks::{CreateTaskResult, GetTaskResult, Task, TaskStatus};

    fn sample_task() -> Task {
        Task::new("t-positive", TaskStatus::Working)
            .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z")
    }

    #[test]
    fn passes_on_valid_get_task_result() {
        let value = serde_json::to_value(GetTaskResult::new(sample_task())).unwrap();
        assert_roundtrips_through_client::<GetTaskResult>(value);
    }

    #[test]
    fn passes_on_valid_create_task_result() {
        // The create envelope serializes as `{ "task": { .. } }`; serde ignores
        // any extra `_meta` the live dispatch envelope also carries.
        let mut value = serde_json::to_value(CreateTaskResult::new(sample_task())).unwrap();
        value.as_object_mut().unwrap().insert(
            "_meta".to_string(),
            serde_json::json!({ "io.modelcontextprotocol/related-task": { "taskId": "t-positive" } }),
        );
        assert_roundtrips_through_client::<CreateTaskResult>(value);
    }

    #[test]
    #[should_panic(expected = "does not deserialize into")]
    fn panics_on_exact_historical_flat_task_shape() {
        // The EXACT historical bug: a serialized `Task` with top-level `taskId`
        // / `status`, NOT wrapped in `{ "task": ... }`. Feeding this into
        // `GetTaskResult` (which requires the `task` wrapper) must panic.
        let flat_task = serde_json::to_value(Task::new("t-1", TaskStatus::Working)).unwrap();
        // Sanity: the flat shape really does have a top-level `taskId`.
        assert!(flat_task.get("taskId").is_some());
        assert!(flat_task.get("task").is_none());
        assert_roundtrips_through_client::<GetTaskResult>(flat_task);
    }
}
