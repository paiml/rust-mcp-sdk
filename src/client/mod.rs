//! MCP client implementation.

use crate::error::{Error, Result};
use crate::shared::{
    EnhancedMiddlewareChain, MiddlewareContext, Protocol, ProtocolOptions, Transport,
};
use crate::types::mrtr::{
    InputRequests, InputResponses, CALL_TOOL_METHOD, COMPLETE_RESULT_TYPE, GET_PROMPT_METHOD,
    INPUT_RESPONSES_KEY, READ_RESOURCE_METHOD, RESULT_TYPE_KEY, TASKS_CANCEL_METHOD,
    TASKS_GET_METHOD, TASKS_UPDATE_METHOD, TASK_ID_KEY, TASK_RESULT_TYPE,
};
// `TaskStatus` is consumed only by the native task-polling path; on wasm32 that
// path is not compiled, so the import is unused THERE and nowhere else.
#[cfg_attr(target_arch = "wasm32", allow(unused_imports))]
use crate::types::tasks::{
    resolve_poll_interval, CancelTaskRequest, CancelTaskResult, CreateTaskResult, DetailedTaskV2,
    GetTaskPayloadRequest, GetTaskRequest, GetTaskResult, ListTasksRequest, ListTasksResult, Task,
    TaskDetailV2, TaskMetadata, TaskPollDecision, TaskStatus, TaskV2, MIN_POLL_MS,
};
use crate::types::{
    CallToolRequest, CallToolResult, CancelledNotification, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, CreateMessageParams, CreateMessageResult,
    GetPromptRequest, GetPromptResult, Implementation, InitializeRequest, InitializeResult,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest,
    ListToolsResult, LoggingLevel, Notification, ProgressNotification, PromptInfo,
    ReadResourceRequest, ReadResourceResult, Request, RequestId, ResourceInfo, ResourceTemplate,
    ServerCapabilities, SubscribeRequest, ToolInfo, UnsubscribeRequest,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, oneshot, RwLock};

#[cfg(target_arch = "wasm32")]
use futures::SinkExt;
#[cfg(target_arch = "wasm32")]
use futures_channel::{mpsc, oneshot};
#[cfg(target_arch = "wasm32")]
use futures_locks::RwLock;

#[cfg(all(not(target_arch = "wasm32"), feature = "http-client"))]
pub mod auth;
pub mod host;
pub mod http_logging_middleware;
pub mod http_middleware;
#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
pub mod oauth;
pub mod oauth_middleware;
mod options;
/// The client half of the v2 `subscriptions/listen` long-lived stream (HTTP-04).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub mod subscriptions;
pub mod transport;

pub use options::ClientOptions;

pub use host::{
    ApprovalDecision, ClientHostRegistry, HostElicitationHandler, HostSamplingHandler,
    PreflightApproval, RootsProvider, SamplingResultReview,
};

/// Response from a task-augmented `tools/call`.
///
/// When calling [`Client::call_tool_with_task`], the server may return either
/// an async task (poll with `tasks/get`) or a synchronous result.
///
/// # Both eras land in the SAME two variants (Phase 114, plan 19)
///
/// This enum is PUBLIC and not `#[non_exhaustive]`, so neither its variants nor
/// their payload types may change without a MAJOR semver bump. The `2026-07-28`
/// create result is a FLAT `{taskId,status,createdAt,lastUpdatedAt,ttlMs}` body
/// where v1's is a NESTED `{"task": {…}}` one, and the two eras also spell two
/// fields differently (`ttlMs`/`pollIntervalMs` vs `ttl`/`pollInterval`) — but
/// that difference is absorbed by
/// [`TaskV2::to_v1`](crate::types::tasks::TaskV2::to_v1) at the decode site, so
/// [`Task`] stays the one handle type a caller sees on either wire.
///
/// A v2 `tasks/get` carries STATUS-CONDITIONAL detail the v1 [`Task`] has no
/// field for (`result` on `completed`, `error` on `failed`, `inputRequests` on
/// `input_required`). That detail is reached through the ADDITIVE
/// [`Client::tasks_get_detailed`] rather than by widening a variant here.
#[derive(Debug, Clone)]
pub enum ToolCallResponse {
    /// The server returned a synchronous result (no task created).
    Result(CallToolResult),
    /// The server created an async task. Poll with [`Client::tasks_get`]
    /// until the task reaches a terminal status.
    ///
    /// - **v1** — then call [`Client::tasks_result`] for the final
    ///   `CallToolResult`.
    /// - **v2** — `tasks/result` does not exist; the terminal payload is INLINE
    ///   in the `tasks/get` result, read through [`Client::tasks_get_detailed`]
    ///   (or simply let [`Client::wait_for_task`] do both).
    Task(Task),
}

/// Options controlling [`Client::wait_for_task`] polling.
///
/// A caller who holds a [`TaskMetadata`] (e.g. from
/// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task))
/// composes options directly via [`WaitForTaskOptions::from_metadata`] /
/// [`From<TaskMetadata>`] — no hand-copying of poll fields.
#[derive(Debug, Clone, Default)]
pub struct WaitForTaskOptions {
    /// Override polling interval, in **milliseconds**. When `None`, the
    /// task-reported `pollInterval` (then a built-in default) is used. The
    /// effective interval is clamped to a small floor so a zero value cannot
    /// hot-spin the poll loop.
    pub poll_interval: Option<u64>,
    /// Maximum total time to poll before returning a timeout error, in
    /// **seconds**. When `None`, polling continues until the task is terminal
    /// (or enters `input_required`, which surfaces an error immediately — see
    /// [`Client::wait_for_task`]).
    pub max_poll_duration_secs: Option<u64>,
}

impl WaitForTaskOptions {
    /// Build options from a [`TaskMetadata`], copying its poll fields verbatim.
    pub fn from_metadata(meta: &TaskMetadata) -> Self {
        Self {
            poll_interval: meta.poll_interval,
            max_poll_duration_secs: meta.max_poll_duration_secs,
        }
    }

    /// Fill any unset fields from `meta`; existing `self` values take precedence.
    #[must_use]
    pub fn or_from_metadata(mut self, meta: &TaskMetadata) -> Self {
        self.poll_interval = self.poll_interval.or(meta.poll_interval);
        self.max_poll_duration_secs = self.max_poll_duration_secs.or(meta.max_poll_duration_secs);
        self
    }
}

impl From<TaskMetadata> for WaitForTaskOptions {
    fn from(meta: TaskMetadata) -> Self {
        Self::from_metadata(&meta)
    }
}

/// The reserved `_meta` key carrying the per-request self-reported protocol
/// version. Spelled `io.modelcontextprotocol/protocolVersion` on the wire.
///
/// Sourced from the ONE crate-level table (`types::protocol::context`) that the
/// SERVER resolver reads, so the two ends cannot drift.
/// [`Client::v2_request_meta`] emits it on every v2 request.
const META_PROTOCOL_VERSION: &str = crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;

/// The reserved `_meta` key carrying the client's self-reported
/// `io.modelcontextprotocol/clientInfo`.
const META_CLIENT_INFO: &str = crate::types::protocol::context::RESERVED_CLIENT_INFO_KEY;

/// The reserved `_meta` key carrying the client's
/// `io.modelcontextprotocol/clientCapabilities`.
const META_CLIENT_CAPABILITIES: &str =
    crate::types::protocol::context::RESERVED_CLIENT_CAPABILITIES_KEY;

/// The `_meta` object key inside `params` (the SPEC spelling).
///
/// Phase-113 D-113-A pinned `_meta` as pmcp's egress spelling on the typed
/// request structs; the raw v2 injection below uses the same key so a single
/// request never carries two spellings.
const PARAMS_META_KEY: &str = crate::types::mrtr::META_KEY;

/// The default MRTR gather→resend round bound (Phase 113, D-09).
///
/// Small on purpose: eight rounds is generous for a real multi-round
/// interaction and short enough that a buggy or hostile server cannot loop a
/// human (or an autonomous agent) for long. Override with
/// [`ClientBuilder::mrtr_round_limit`].
const DEFAULT_MRTR_ROUND_LIMIT: usize = 8;

/// The `sampling/createMessage` method name, sourced from the ONE MRTR kind
/// table so the v1 host path and the v2 fold cannot spell it differently.
const SAMPLING_METHOD: &str = crate::types::mrtr::InputRequestKind::Sampling.wire_method();

/// The `elicitation/create` method name. See [`SAMPLING_METHOD`].
const ELICITATION_METHOD: &str = crate::types::mrtr::InputRequestKind::Elicitation.wire_method();

/// The `roots/list` method name. See [`SAMPLING_METHOD`].
const ROOTS_METHOD: &str = crate::types::mrtr::InputRequestKind::Roots.wire_method();

/// `tasks/list` — RETIRED on the `2026-07-28` wire (Phase 114, TASK-03).
///
/// Deliberately NOT a row of either `types::mrtr` method table: those tables
/// decide MRTR eligibility and `Mcp-Name` derivation, and this method exists on
/// v1 ONLY — claiming a v2 routing name for it would be a claim pmcp cannot
/// support. It is spelled here because exactly two client sites need it: the v1
/// call and the v2 local refusal.
const TASKS_LIST_METHOD: &str = "tasks/list";

/// `tasks/result` — RETIRED on the `2026-07-28` wire. See [`TASKS_LIST_METHOD`].
const TASKS_RESULT_METHOD: &str = "tasks/result";

/// What a v2 caller should reach for instead of the retired `tasks/list`.
///
/// There is NO v2 list method — the enumeration primitive was removed as a
/// security improvement (a server with no list cannot leak the existence of one
/// caller's tasks to another), so the honest answer is that the client keeps its
/// own ids. Saying "use `tasks/get`" here would be a lie: `tasks/get` answers
/// about ONE id the caller already holds.
const TASKS_LIST_V2_REPLACEMENT: &str = "client-side task tracking";

/// The input-responder type [`Client::wait_for_task`] passes as `None`.
///
/// `Client::poll_task_to_terminal` is generic over its responder so that
/// [`Client::wait_for_task_with_inputs`] can reuse it WHOLESALE rather than
/// growing a second copy of the loop. A bare `None` leaves both type parameters
/// unconstrained, so the no-responder caller names this concrete, NEVER-CALLED
/// function-pointer type instead. `std::future::Ready` is simply the smallest
/// concrete `Future` that satisfies the bound.
type NoInputResponder = fn(InputRequests) -> std::future::Ready<Result<InputResponses>>;

/// How long ONE pump attempt waits for a frame before releasing the transport.
///
/// `Transport::send` and `Transport::receive` both take `&mut self`, so a client
/// holds ONE lock for both directions and whoever is receiving blocks every
/// sender. Slicing the receive is therefore not an optimisation — it is the only
/// reason a second operation on the same `Client` can make progress while a call
/// is outstanding.
///
/// Slicing drops the in-flight `receive()` future, which
/// [`Transport::receive`](crate::shared::Transport::receive) documents that
/// implementors MUST tolerate without losing already-consumed bytes.
const PUMP_RECEIVE_SLICE: std::time::Duration = std::time::Duration::from_millis(250);

/// The longest a request keeps waiting once the peer has begun answering with
/// ids NOBODY is awaiting (code review of Phase 118.2).
///
/// # Why a ceiling exists at all
///
/// [`RequestId`] equality is structural AND typed, as JSON-RPC 2.0 requires: a
/// peer that answers `"id": "7"` for a request this client minted as
/// `RequestId::Number(7)` produces a response that matches nothing. The router
/// drops it with a `warn!` and [`Client::dispatch_request`] goes on waiting —
/// forever, because pmcp has no request timeout (`RequestOptions::timeout` is
/// public but unreachable). A hang is the worst of the available answers: it is
/// indistinguishable from a slow tool call, and it holds an `active_requests`
/// entry and a caller task for the life of the process.
///
/// # What it bounds, and what it deliberately does not
///
/// It bounds frames addressed to ids THIS CLIENT NEVER MINTED: the re-typed id,
/// and the peer that sprays ids nobody here ever asked for. Those are the shapes
/// that answer nothing, forever, and they are counted by
/// [`MAX_UNMATCHED_RESPONSES`] as well as timed here — either bound alone is
/// sufficient.
///
/// It does NOT bound a frame that is the LATE ANSWER of a request this client
/// already stopped waiting for. Such a frame is our own debris rather than the
/// peer's misbehaviour, and charging it to whichever unrelated call happens to
/// be pumping was self-sustaining: one abandoned call plus a slow peer failed
/// every LATER call on the same `Client`, permanently. [`AbandonedRequestIds`]
/// separates the two classifications and absorbs the debris ONCE;
/// `debris_from_a_dead_call_does_not_charge_the_next_calls_budget` in
/// `tests/client_sse_stream.rs` is the fence that proves the chain is broken at
/// every link.
///
/// It fires ONLY once a mis-addressed frame has actually been seen. A request
/// whose peer has simply gone quiet is NOT bounded here, deliberately: that is
/// the pre-existing no-default-timeout behaviour, and giving every call a
/// blanket deadline would break long-running `tools/call` handlers, which is a
/// product decision rather than a review fix.
const UNMATCHED_RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How many unmatched response frames ONE request tolerates before failing.
///
/// A peer emitting a mis-addressed frame every few milliseconds keeps the wait
/// busy and productive-looking right up to [`UNMATCHED_RESPONSE_TIMEOUT`], and
/// the caller then sees only a timeout — true but undiagnosable. The cap turns
/// that shape into a named failure carrying the count and the offending id, so
/// "the peer went quiet" and "the peer is spraying wrong ids" are distinguishable
/// from the error alone.
///
/// Thirty-two is far above anything a correct peer produces (a correct peer
/// produces zero) and far below anything that could be mistaken for progress.
const MAX_UNMATCHED_RESPONSES: usize = 32;

/// The longest peer-chosen [`RequestId`] rendering this client will put into a
/// log line or an error (code review of Phase 118.2).
///
/// A response id is remote input of unbounded length: `RequestId::String` comes
/// straight off the wire, and a hostile or broken server can stream 1 MiB ids as
/// fast as it likes. The transport already bounds its own untrusted echo with
/// `MAX_ECHOED_SSE_FRAME` and states the rule — a hostile server must not be
/// able to push an unbounded string into a client's logs — and the router has to
/// follow it.
///
/// 128 bytes is far longer than any id a real peer mints (a counter, a UUID)
/// and short enough that a flood costs the log a bounded amount per line.
const MAX_ECHOED_REQUEST_ID: usize = 128;

/// Render a peer-supplied [`RequestId`] for a log line or an error, BOUNDED.
///
/// The typed `Debug` form deliberately, not the bare value: `RequestId` equality
/// is structural and typed, so `String("7")` and `Number(7)` are DIFFERENT ids.
/// Rendered as bare strings the two read identically and a re-typing report
/// looks like a contradiction; rendered as `String("7")` and `Number(7)` the
/// re-typing is the first thing the reader sees.
///
/// Truncation lands on a `char` boundary — `&str[..n]` PANICS mid-codepoint, and
/// a peer choosing a multi-byte id must not be able to panic a client's log line
/// — and names the number of bytes withheld so a truncated echo is never
/// mistaken for a short id.
fn echoed_request_id(id: &RequestId) -> String {
    let rendered = format!("{id:?}");
    if rendered.len() <= MAX_ECHOED_REQUEST_ID {
        return rendered;
    }
    let mut cut = MAX_ECHOED_REQUEST_ID;
    while cut > 0 && !rendered.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}… (+{} bytes withheld)",
        &rendered[..cut],
        rendered.len() - cut
    )
}

/// What ONE `dispatch_request` has spent on response frames addressed to nobody.
///
/// Its own type rather than two locals in the wait loop for two reasons: the
/// arming rule (the deadline is set on the FIRST unmatched frame and never
/// moves) is a correctness property worth unit-testing in isolation, and keeping
/// it out of [`Client::dispatch_request`] is what holds that function under the
/// repo's cognitive-complexity budget without an `#[allow]`.
#[derive(Debug, Default)]
struct UnmatchedBudget {
    /// How many unmatched frames this request has observed.
    seen: usize,
    /// When the wait expires — `None` until the FIRST unmatched frame.
    ///
    /// `None` is not "no deadline yet, use a default"; it is the load-bearing
    /// signal that the peer has not misbehaved, which is what keeps an ordinary
    /// slow call unbounded.
    deadline: Option<web_time::Instant>,
}

impl UnmatchedBudget {
    /// Book one unmatched frame.
    ///
    /// The deadline is armed on the FIRST one ONLY. Re-arming it per frame would
    /// let a peer emitting a steady drip of wrong ids extend the wait
    /// indefinitely — the ceiling would exist and never fire, which is the defect
    /// this type closes wearing a fix's clothes.
    fn record(&mut self) {
        self.seen += 1;
        if self.deadline.is_none() {
            self.deadline = Some(web_time::Instant::now() + UNMATCHED_RESPONSE_TIMEOUT);
        }
    }

    /// The failure this budget has reached, or `None` while it still has room.
    ///
    /// Either bound alone is sufficient: the count catches a fast sprayer and the
    /// deadline catches a slow drip, and a peer can choose which one to walk into.
    /// `last` is the already-bounded rendering of the most recent offending id —
    /// see [`echoed_request_id`] for why it is bounded and why it is typed.
    fn exhausted(&self, awaiting: &RequestId, last: Option<&str>) -> Option<Error> {
        let expired = self
            .deadline
            .is_some_and(|deadline| web_time::Instant::now() >= deadline);
        if self.seen < MAX_UNMATCHED_RESPONSES && !expired {
            return None;
        }
        let last = last.unwrap_or("<none>");
        Some(Error::Transport(crate::error::TransportError::Request(
            format!(
                "the peer answered with {seen} response id(s) no request is awaiting while \
                 {awaiting:?} was outstanding (most recently {last}); JSON-RPC 2.0 ids are typed, \
                 so a re-typed id — \"7\" for 7 — matches nothing and this request would \
                 otherwise wait forever",
                seen = self.seen,
            ),
        )))
    }
}

/// How many abandoned request ids ONE client remembers at a time.
///
/// Sized far above the number of requests a client abandons in the window
/// between an abandonment and the arrival of that request's answer — in practice
/// one, occasionally a handful under a peer that is spraying — and far below
/// anything that could matter for memory: a [`RequestId`] is a `u64` or a short
/// `String`, so sixty-four of them is measured in kilobytes at worst.
///
/// A PRIVATE constant, exactly as [`UNMATCHED_RESPONSE_TIMEOUT`],
/// [`MAX_UNMATCHED_RESPONSES`] and [`PUMP_RECEIVE_SLICE`] are: none of this
/// client's correlation knobs is a semver event.
const MAX_ABANDONED_REQUEST_IDS: usize = 64;

/// Ids this client MINTED whose owner stopped waiting before the answer landed.
///
/// # What it is for
///
/// [`Client::pump_once`] used to answer one question — "does any LIVE request
/// await this id?" — and treat both `no` answers alike. But they are not alike.
/// A frame the peer mis-addressed is genuine misbehaviour and must spend the
/// awaiting call's [`UnmatchedBudget`]; a frame this client asked for and then
/// stopped waiting for is our OWN debris and must charge nobody. Conflating them
/// is self-sustaining: a call that dies at its ceiling leaves its real answer on
/// the shared receive queue, the next call pops it, books it unmatched and arms
/// its own deadline on frame one — so one stray frame plus a slow peer failed
/// every later call on that client, permanently.
///
/// # The three properties this type is read against
///
/// 1. **Bounded by construction.** A fixed [`MAX_ABANDONED_REQUEST_IDS`] cap
///    with oldest-eviction, not a time-based expiry and not a reaper task: there
///    is no arrival pattern under which it grows.
/// 2. **Only locally-minted ids can enter it.** Every call site records an id
///    that came back from `active_requests::remove`, i.e. one this client minted
///    and registered. Nothing that arrived off the wire is ever recorded, so a
///    hostile peer cannot grow it at all — the memory bound is a property of the
///    type rather than a hope about peer behaviour.
/// 3. **An entry is consumed on FIRST use.** A peer that replays one abandoned
///    id N times is absorbed exactly once; the remaining N-1 replays are booked
///    against the budget as the misbehaviour they are.
#[derive(Debug, Default)]
struct AbandonedRequestIds {
    /// Oldest first, so eviction is a `pop_front`.
    ids: VecDeque<RequestId>,
}

impl AbandonedRequestIds {
    /// Remember one id whose owner stopped waiting, evicting the oldest when the
    /// cap is already reached.
    ///
    /// The caller MUST have obtained `id` from a live `active_requests`
    /// registration — see property 2 on the type.
    fn record(&mut self, id: RequestId) {
        // `if`, not `while`: this is the ONLY insertion path and it pushes
        // exactly one id, so the length can never exceed the cap by more than
        // one and a second eviction is unreachable. A loop here would suggest
        // the invariant is weaker than it is.
        if self.ids.len() >= MAX_ABANDONED_REQUEST_IDS {
            self.ids.pop_front();
        }
        self.ids.push_back(id);
    }

    /// Consume the entry for `id`, reporting whether there was one.
    ///
    /// `true` means "this is our own debris, absorb it"; `false` means "nobody
    /// here ever asked for this id", which is the peer misbehaviour the budget
    /// exists for. Removing on the way out is what makes a replay cost the peer.
    fn take(&mut self, id: &RequestId) -> bool {
        let Some(position) = self.ids.iter().position(|known| known == id) else {
            return false;
        };
        self.ids.remove(position);
        true
    }
}

/// One step of [`Client::pump_once`], from the waiting caller's point of view.
///
/// Three outcomes rather than `Result<()>` because the caller has to distinguish
/// them: its own answer ends the wait, an unmatched frame spends budget, and
/// everything else is ordinary progress.
enum PumpStep<T> {
    /// THIS caller's own answer arrived, raced inside the pump's own `select`.
    Answered(T),
    /// A response arrived that no request is awaiting, and was dropped. Carries
    /// the already-bounded rendering of its id (see [`echoed_request_id`]).
    Unmatched(String),
    /// A routed response, a notification, an inbound request, or an expired
    /// slice — anything that is neither of the above.
    Progressed,
}

/// What one in-flight client request is waiting on.
///
/// # Why the response channel lives here
///
/// `Transport::receive()` is a single, unaddressed FIFO, but JSON-RPC responses
/// are id-addressed. Before this type the last stage of demultiplexing was left
/// to every caller: each popped frames itself and, on finding a frame that was
/// not its own, had nowhere to put it — a `Transport` consumer holds no producer
/// handle — so it DISCARDED it, destroying some other caller's answer.
///
/// Registering the answer channel by id moves that demultiplexing to one place.
/// A frame is delivered to whoever awaits its id; a frame nobody awaits is
/// dropped. That is harmless in TWO distinct ways, and the split matters: no
/// caller is blocked on it, AND — since [`AbandonedRequestIds`] — a frame that
/// is the late answer of a request this client already stopped waiting for is
/// not CHARGED to one either. Only a frame addressed to an id nobody here ever
/// minted spends the awaiting call's [`UnmatchedBudget`].
struct Pending {
    /// Signalled by [`Client::cancel_request`].
    cancel: oneshot::Sender<()>,
    /// This request's answer, delivered by whichever task is pumping.
    ///
    /// Success only. A transport-level failure is deliberately NOT fanned out
    /// here: a terminal reason is sticky by contract, so every waiter that
    /// resumes pumping observes it and builds its OWN owned [`Error`]. That is
    /// what lets this channel stay `Clone`-free — [`Error`] is not `Clone`.
    response: oneshot::Sender<crate::types::JSONRPCResponse>,
}

/// Why a host request could not be answered.
///
/// Shared by the v1 server-initiated dispatch and the v2 MRTR fold so both
/// consume the SAME pipeline (approval gate, handler preference, result
/// review) and only differ in how they render the refusal.
#[derive(Debug)]
enum HostRefusal {
    /// No handler is registered for this kind.
    NoHandler,
    /// A policy gate (preflight approval or result review) denied it. The
    /// reason is logged at the denial site and deliberately not carried here —
    /// local host policy is never forwarded to the remote server.
    Denied,
    /// The registered handler or provider itself failed.
    Failed(Error),
    /// The handler's result could not be serialized.
    Serialization,
}

impl HostRefusal {
    /// A short, non-sensitive reason string for MRTR fold logging.
    fn reason(&self) -> &'static str {
        match self {
            Self::NoHandler => "no registered handler",
            Self::Denied => "denied by host policy",
            Self::Failed(_) => "handler returned an error",
            Self::Serialization => "handler result could not be serialized",
        }
    }
}

/// A sampling completion, still typed as the registered handler produced it.
///
/// The shared pipeline returns this rather than a serialized value because the
/// two entry points render it differently: the v1 host response carries the
/// tool-aware result in FULL, while an MRTR `inputResponses` value is
/// spec-typed as a `CreateMessageResult`.
#[derive(Debug)]
enum HostSamplingCompletion {
    /// From a [`host::HostSamplingHandler`].
    Legacy(CreateMessageResult),
    /// From a [`host::HostSamplingHandlerWithTools`].
    WithTools(crate::types::sampling::CreateMessageResultWithTools),
}

/// The outcome of folding an entire `inputRequests` map into `inputResponses`.
#[derive(Debug)]
enum FoldOutcome {
    /// EVERY entry was answered.
    Fulfilled(crate::types::mrtr::InputResponses),
    /// At least one entry could not be answered. All-or-nothing: no partial
    /// map, no fabricated response, and the client does NOT resend.
    CannotFulfil,
}

/// What ONE MRTR round decided.
#[derive(Debug)]
enum RoundOutcome {
    /// A non-`input_required` result — the operation is done.
    Terminal(serde_json::Value),
    /// The server needs input this client cannot supply — done, unfulfilled.
    /// Boxed because the variant is far larger than the others.
    Unfulfilled(Box<crate::types::mrtr::InputRequiredResult>),
    /// Resend the original request carrying these MRTR fields.
    Continue(crate::types::mrtr::MrtrRequestParams),
}

/// What the whole MRTR loop produced.
///
/// A two-variant enum rather than a struct with an `Option`: the struct form
/// needed six doc lines to state the invariant "`unfulfilled` is `Some` exactly
/// when `raw_result` is an `input_required` result", and paid a full deep clone
/// of the server's result body to populate a field that BOTH consumers
/// provably never read in that case (each returns early on `unfulfilled`).
/// As an enum the invariant is type-enforced and the clone is unrepresentable.
#[derive(Debug)]
enum MrtrLoopOutcome {
    /// The loop finished: the verbatim `result` object of the final response.
    Complete(serde_json::Value),
    /// The loop stopped because no registered handler could answer the server's
    /// `inputRequests`.
    Unfulfilled(Box<crate::types::mrtr::InputRequiredResult>),
}

/// MCP client for connecting to servers.
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Arc<RwLock<Protocol>>,
    middleware_chain: Arc<RwLock<EnhancedMiddlewareChain>>,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    server_version: Option<Implementation>,
    instructions: Option<String>,
    initialized: bool,
    info: Implementation,
    notification_tx: Option<mpsc::Sender<Notification>>,
    active_requests: Arc<RwLock<HashMap<RequestId, Pending>>>,
    /// Ids this client minted whose owner stopped waiting before the answer
    /// arrived — see [`AbandonedRequestIds`].
    ///
    /// Behind the SAME `Arc<RwLock<..>>` shape as `active_requests` and cloned
    /// alongside it, because the two are read together in [`Client::pump_once`]
    /// and a clone that shared one but not the other would be a split brain: a
    /// caller on one clone could abandon an id that a pump on another clone then
    /// booked as the peer's misbehaviour.
    abandoned_requests: Arc<RwLock<AbandonedRequestIds>>,
    /// The transport's owned send handle, resolved ONCE at construction.
    ///
    /// [`Transport::shared_sender`] answers a question that is constant for the
    /// transport's lifetime — no API swaps a `Client`'s transport — so asking
    /// it per frame bought nothing and cost real latency. The read guard it
    /// needed queues behind [`Client::pump_once`]'s writer, which holds the
    /// transport for up to `PUMP_RECEIVE_SLICE`; whenever any task on a cloned
    /// client was pumping, a send waited out a slice just to ask. Worse, a
    /// `None`-answering transport — stdio, WebSocket, every external one — paid
    /// that twice per send: once to ask, once to send. That was a REGRESSION
    /// against the pre-plan-23 path for the default transport.
    ///
    /// Resolved here, the opt-in path takes NO transport lock and the fallback
    /// takes exactly one, which is what the code did before plan 23.
    shared_sender: Option<Arc<dyn crate::shared::SharedSender>>,
    options: ClientOptions,
    /// Registered host handlers answering inbound server -> client requests
    /// (sampling / elicitation / roots). Immutable after construction.
    host_registry: crate::client::host::ClientHostRegistry,
    /// The EXPLICIT per-connection protocol-version selection made via
    /// [`ClientBuilder::with_protocol_version`] (Phase 113, CLNT-01).
    ///
    /// `None` — the default and the only state reachable without that builder
    /// call — means "behave exactly as pmcp always has": v1, full `initialize`
    /// handshake, no v2 headers, no per-request `_meta`.
    negotiated_protocol_version: Option<crate::types::ProtocolVersion>,
    /// Maximum MRTR gather→resend rounds before giving up (Phase 113, D-09).
    ///
    /// See [`ClientBuilder::mrtr_round_limit`]. Dead on a v1 connection.
    mrtr_round_limit: usize,
    /// The Extensions-Track capabilities this client DECLARES on v2 (Phase 114,
    /// D-04), merged into the `extensions` map of the `ClientCapabilities` that
    /// [`Self::v2_request_meta`] serializes into every request's
    /// `_meta["io.modelcontextprotocol/clientCapabilities"]`.
    ///
    /// `None` — the default and the only state reachable without
    /// [`ClientBuilder::with_tasks_extension`] — declares NOTHING, and the
    /// `extensions` key is then absent from the serialized capabilities
    /// entirely (the field carries `skip_serializing_if`).
    ///
    /// Deliberately NOT read on v1: the v1 `initialize` handshake advertises the
    /// `ClientCapabilities` the CALLER passed to [`Self::initialize`], and
    /// injecting an extension there would move the `initialize` bytes of every
    /// existing caller (Phase-114 D-02).
    declared_extensions: Option<HashMap<String, serde_json::Value>>,
}

impl<T: Transport> std::fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("transport", &"<Arc<RwLock<Transport>>>")
            .field("protocol", &"<Arc<RwLock<Protocol>>>")
            .field("capabilities", &self.capabilities)
            .field("server_capabilities", &self.server_capabilities)
            .field("initialized", &self.initialized)
            .field("info", &self.info)
            .field("host_registry", &self.host_registry)
            .finish()
    }
}

impl<T: Transport> Client<T> {
    /// Create a new client with the given transport.
    ///
    /// Uses default client information with the name "pmcp-client" and the
    /// current crate version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport};
    ///
    /// let transport = StdioTransport::new();
    /// let client = Client::new(transport);
    /// ```
    pub fn new(transport: T) -> Self {
        Self::with_info(
            transport,
            Implementation::new("pmcp-client", env!("CARGO_PKG_VERSION")),
        )
    }

    /// Create a new client with custom info.
    ///
    /// Allows specifying custom client name and version information that will
    /// be sent to the server during initialization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation::new("my-custom-client", "2.1.0");
    /// let client = Client::with_info(transport, client_info);
    /// ```
    pub fn with_info(transport: T, client_info: Implementation) -> Self {
        Self {
            shared_sender: transport.shared_sender(),
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            abandoned_requests: Arc::new(RwLock::new(AbandonedRequestIds::default())),
            options: ClientOptions::default(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
            negotiated_protocol_version: None,
            mrtr_round_limit: DEFAULT_MRTR_ROUND_LIMIT,
            declared_extensions: None,
        }
    }

    /// Create a new client with custom protocol options.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    /// use pmcp::shared::ProtocolOptions;
    ///
    /// // Custom options for high-throughput scenarios
    /// let options = ProtocolOptions {
    ///     enforce_strict_capabilities: false,
    ///     debounced_notification_methods: vec![
    ///         "notifications/progress".to_string(),
    ///         "notifications/message".to_string(),
    ///     ],
    /// };
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation::new("high-throughput-client", "1.0.0");
    ///
    /// let client = Client::with_options(transport, client_info, options);
    /// ```
    pub fn with_options(
        transport: T,
        client_info: Implementation,
        options: ProtocolOptions,
    ) -> Self {
        Self {
            shared_sender: transport.shared_sender(),
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(options))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            abandoned_requests: Arc::new(RwLock::new(AbandonedRequestIds::default())),
            options: ClientOptions::default(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
            negotiated_protocol_version: None,
            mrtr_round_limit: DEFAULT_MRTR_ROUND_LIMIT,
            declared_extensions: None,
        }
    }

    /// Construct a client with caller-supplied [`ClientOptions`].
    ///
    /// Mirrors [`Self::new`] but wires in a [`ClientOptions`] value so that
    /// [`Self::list_all_tools`] / [`Self::list_all_prompts`] / etc. honour a
    /// custom `max_iterations` cap.
    ///
    /// ## `ClientBuilder` parity
    ///
    /// [`ClientBuilder`] does not currently expose a `.client_options()` setter.
    /// If you need a custom [`ClientOptions`], construct the client via
    /// [`Self::with_client_options`] directly.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(transport: T) -> pmcp::Result<()> {
    /// use pmcp::{Client, ClientOptions};
    ///
    /// let opts = ClientOptions::default().with_max_iterations(50);
    /// let _client = Client::with_client_options(transport, opts);
    /// # Ok(()) }
    /// ```
    pub fn with_client_options(transport: T, options: ClientOptions) -> Self {
        Self {
            shared_sender: transport.shared_sender(),
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: Implementation::default(),
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            abandoned_requests: Arc::new(RwLock::new(AbandonedRequestIds::default())),
            options,
            host_registry: crate::client::host::ClientHostRegistry::default(),
            negotiated_protocol_version: None,
            mrtr_round_limit: DEFAULT_MRTR_ROUND_LIMIT,
            declared_extensions: None,
        }
    }

    /// Initialize the connection with the server.
    ///
    /// Performs the MCP initialization handshake, negotiating capabilities and
    /// receiving server information. This must be called before using other
    /// client methods.
    ///
    /// # Host capabilities are registry-derived (`sampling` / `elicitation` / `roots`)
    ///
    /// The three host-side capability fields — `sampling`, `elicitation`, and
    /// `roots` — are **derived from the handlers registered on
    /// [`ClientBuilder`]**, not from the value passed here. If no matching host
    /// handler is registered, the corresponding field is forced to `None` on the
    /// wire even when the caller set it (the anti-capability-lie rule: a client
    /// must not advertise a host capability it cannot service). Register handlers
    /// via [`ClientBuilder::on_sampling`], [`ClientBuilder::on_elicitation`], and
    /// [`ClientBuilder::on_roots`] to advertise these capabilities. When a
    /// handler *is* registered, any caller-configured detail for that field
    /// (e.g. `roots.list_changed`) is preserved. All other capability fields
    /// (`tasks`, `experimental`, ...) pass through unchanged.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// let capabilities = ClientCapabilities::default();
    /// let server_info = client.initialize(capabilities).await?;
    ///
    /// println!("Server: {} v{}",
    ///          server_info.server_info.name,
    ///          server_info.server_info.version);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Severability: this method is NOT gated, and that is measured
    ///
    /// SMPL-01 names "initialize/session lifecycle" as v1-only machinery that a
    /// `full-v2` build must not carry. Plan 117-14 MEASURED whether
    /// `#[cfg(feature = "v1-compat")]` could be applied here and took a
    /// documented fallback for two reasons, both recorded in
    /// `.planning/phases/117-agents-tester-v1-severability/117-14-SUMMARY.md`:
    ///
    /// 1. It is DUAL-era, not v1-only. The `is_v2()` branch below is a
    ///    deliberate Phase-113 compatibility affordance — it sends nothing and
    ///    exists so v1-shaped application code keeps compiling after opting
    ///    into v2. Gating this method would delete v2 behaviour, not just v1
    ///    behaviour.
    /// 2. `src/composition/mcp_client.rs` calls it, and `composition` is in the
    ///    `full-v2` feature list. Propagating the gate there would mean a
    ///    composition connection that reports itself initialized without ever
    ///    having handshaken — a semantic change to a subsystem this plan has no
    ///    mandate over.
    ///
    /// SMPL-01's "initialize" clause is therefore met on the SERVER side only,
    /// and even there it is the session BOOKKEEPING that is severed, not the
    /// handshake: plan 117-12 moved `process_init_session`,
    /// `update_session_after_init` and the rest of the session-lifecycle
    /// functions into `v1_session.rs`, while a `full-v2` server still answers an
    /// `initialize` POST statelessly (only GET and DELETE are refused `405`).
    /// The pure classifiers `is_initialize_request` /
    /// `extract_negotiated_version` therefore stay ungated in
    /// `streamable_http_server.rs`. `docs/v1-sunset-policy.md` MUST name this as
    /// a known limitation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is already initialized
    /// - The server rejects the initialization
    /// - Communication with the server fails
    pub async fn initialize(
        &mut self,
        mut capabilities: ClientCapabilities,
    ) -> Result<InitializeResult> {
        // v2 (2026-07-28) REMOVED the initialize handshake: every request carries
        // its own `_meta` era signal and there is no session to establish. This
        // stays callable so existing v1-shaped application code keeps compiling
        // when it opts into v2, but it sends NOTHING — no `initialize`, no
        // `notifications/initialized`.
        if self.is_v2() {
            self.initialized = true;
            return Ok(Self::v2_synthetic_initialize_result());
        }
        if self.initialized {
            return Err(Error::InvalidState("Client already initialized".into()));
        }

        // HOST-05: make the three host capability fields reflect the registry
        // (registry-authoritative anti-capability-lie) before advertising them.
        self.derive_host_capabilities(&mut capabilities);

        self.capabilities = Some(capabilities.clone());

        // Send initialize request
        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::types::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info: self.info.clone(),
        })));

        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        // Parse initialize result
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let init_result = serde_json::from_value::<InitializeResult>(result)
                    .map_err(|e| Error::parse(format!("Invalid initialize result: {e}")))?;

                // Validate protocol version
                if !crate::types::SUPPORTED_PROTOCOL_VERSIONS
                    .contains(&init_result.protocol_version.as_str())
                {
                    return Err(Error::protocol_msg(format!(
                        "Server protocol version {} not supported",
                        init_result.protocol_version
                    )));
                }

                self.server_capabilities = Some(init_result.capabilities.clone());
                self.server_version = Some(init_result.server_info.clone());
                self.instructions.clone_from(&init_result.instructions);
                self.initialized = true;

                // Send initialized notification
                self.send_notification(Notification::Client(ClientNotification::Initialized))
                    .await?;

                Ok(init_result)
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Apply the HOST-05 registry-authoritative rule to the three host
    /// capability fields (`sampling`/`elicitation`/`roots`), leaving every other
    /// field (`tasks`, `experimental`, ...) untouched.
    ///
    /// Per field:
    /// - **Handler absent** => force `None` (locked anti-capability-lie: a
    ///   caller-set value with no registered handler is discarded, closing the
    ///   spoofing hole where a client advertises a host capability it cannot
    ///   actually service).
    /// - **Handler present, caller left `None`** => insert `Some(default())`.
    /// - **Handler present, caller configured detail** => preserve the caller's
    ///   value unchanged (keeps configured sampling tool support / roots
    ///   `list_changed` / elicitation modes).
    ///
    /// There is deliberately no independent public setter for these three
    /// fields — advertisement is derived, never independently assertable.
    fn derive_host_capabilities(&self, capabilities: &mut ClientCapabilities) {
        // Apply the HOST-05 rule to one capability field:
        // - handler absent (`registered == false`) => force `None`,
        // - handler present, caller left `None` => insert `Some(default())`,
        // - handler present, caller configured detail => leave untouched.
        fn sync_cap<C: Default>(slot: &mut Option<C>, registered: bool) {
            if !registered {
                *slot = None;
            } else if slot.is_none() {
                *slot = Some(C::default());
            }
        }

        // EITHER sampling handler shape can service an inbound
        // `sampling/createMessage`: `on_sampling_with_tools` sets only
        // `sampling_with_tools`, and dispatch prefers it. Checking only
        // `sampling` made a `WithTools`-ONLY client advertise no sampling
        // capability at all — an under-claim that stops a server from ever
        // asking, and (on v2) makes the server answer `-32021` instead of
        // sending the sampling input request the client can in fact fulfil.
        sync_cap(
            &mut capabilities.sampling,
            self.host_registry.sampling.is_some()
                || self.host_registry.sampling_with_tools.is_some(),
        );
        sync_cap(
            &mut capabilities.elicitation,
            self.host_registry.elicitation.is_some(),
        );
        sync_cap(&mut capabilities.roots, self.host_registry.roots.is_some());
    }

    // =======================================================================
    // v2 (`2026-07-28`) era plumbing — Phase 113, CLNT-01.
    // =======================================================================

    /// The era this connection speaks, from the EXPLICIT
    /// [`ClientBuilder::with_protocol_version`] selection.
    ///
    /// A client that never made that call is [`Era::V1`](crate::types::protocol::Era::V1)
    /// and every v2 branch below is dead for it.
    fn era(&self) -> crate::types::protocol::Era {
        self.negotiated_protocol_version
            .as_ref()
            .map_or(crate::types::protocol::Era::V1, |version| {
                crate::types::protocol::protocol_era(version.as_str())
            })
    }

    /// Whether this connection speaks the v2 (`2026-07-28`) wire contract.
    fn is_v2(&self) -> bool {
        self.era() == crate::types::protocol::Era::V2
    }

    /// Refuse a method the 2026-07-28 schema RETIRED, on a v2 connection
    /// (Phase 113, HTTP-04).
    ///
    /// `resources/subscribe` and `resources/unsubscribe` no longer exist on the
    /// v2 wire, and plan 10 made pmcp's own server answer both with `404` +
    /// `-32601`. Sending them anyway costs a round trip and yields an opaque
    /// method-not-found; failing here yields a typed error that NAMES the
    /// replacement (T-113-68).
    ///
    /// On v1 this is a no-op, so the legacy path stays byte-identical.
    fn reject_if_retired_on_v2(&self, method: &str) -> Result<()> {
        if self.is_v2() {
            return Err(Error::retired_on_v2(
                method,
                crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD,
            ));
        }
        Ok(())
    }

    /// Fail fast and LOCALLY when a v2-only method is called on a v1 connection.
    ///
    /// The mirror image of [`Self::reject_if_retired_on_v2`], and the ONE place
    /// the remedy is spelled — naming `ClientBuilder::with_protocol_version` once
    /// rather than in every v2-only method, so renaming the builder does not
    /// leave stale guidance behind in N error strings.
    fn require_v2(&self, method: &str) -> Result<()> {
        if self.is_v2() {
            return Ok(());
        }
        Err(Error::InvalidState(format!(
            "{method} requires the 2026-07-28 era — select it with \
             ClientBuilder::with_protocol_version"
        )))
    }

    /// The `InitializeResult` a v2 client returns from its handshake-free
    /// [`Self::initialize`].
    ///
    /// It is LOCAL and SYNTHETIC: v2 removed `initialize`, so no byte of this
    /// came from the server. Deliberately it is NOT stored into
    /// `server_capabilities` — a v2 client learns the server's capabilities only
    /// from an explicit [`Self::server_discover`] call, and
    /// [`Self::assert_capability`] depends on that distinction.
    fn v2_synthetic_initialize_result() -> InitializeResult {
        InitializeResult {
            protocol_version: crate::types::ProtocolVersion(
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            ),
            capabilities: ServerCapabilities::default(),
            server_info: Implementation::new("unknown", "unknown"),
            instructions: None,
        }
    }

    /// The capabilities a v2 client declares in
    /// `_meta["io.modelcontextprotocol/clientCapabilities"]`.
    ///
    /// DERIVED FROM THE HANDLER REGISTRY, never from a caller-supplied value —
    /// the same registry-authoritative anti-capability-lie rule
    /// [`Self::derive_host_capabilities`] applies to the v1 handshake (HOST-05).
    ///
    /// This is capability HONESTY, and on v2 it is load-bearing in BOTH
    /// directions (spec MRTR obligation 7, conformance
    /// `input-required-result-capability-check`): a server may only put an
    /// `inputRequests` entry in an `input_required` result for a capability the
    /// client DECLARED, so an over-claiming client receives requests it cannot
    /// fulfil and an under-claiming one gets `-32021` where the round could have
    /// completed.
    /// # The `sampling.tools` sub-field is declared, not defaulted away
    ///
    /// `derive_host_capabilities` inserts `SamplingCapabilities::default()`,
    /// whose `tools` is `None`. On v1 that is fine — the caller passes its own
    /// `ClientCapabilities` into `initialize` and any configured detail is
    /// preserved. On v2 there is NO caller-supplied value at all, so the default
    /// IS the declaration: a client that registered `on_sampling_with_tools`
    /// would advertise `{"sampling": {}}`, and the server's
    /// `missing_client_capabilities` precheck would answer `-32021` for a
    /// tool-augmented `sampling/createMessage` the client can in fact service.
    /// That is the same under-claim `derive_host_capabilities` already fixes one
    /// level up for the `sampling` field itself.
    ///
    /// # The `extensions` map is DECLARED, not derived (Phase 114, D-04)
    ///
    /// The three host fields above are registry-derived because pmcp can check
    /// whether a handler exists. An Extensions-Track capability has no such
    /// local witness — it is a statement about what the APPLICATION does with a
    /// `resultType:"task"` response — so it is opted into explicitly through
    /// [`ClientBuilder::with_tasks_extension`] and merged here. A client that
    /// never opted in gets no `extensions` key at all, because the field carries
    /// `skip_serializing_if = "Option::is_none"`.
    fn v2_client_capabilities(&self) -> ClientCapabilities {
        let mut capabilities = ClientCapabilities::default();
        self.derive_host_capabilities(&mut capabilities);
        if self.host_registry.sampling_with_tools.is_some() {
            if let Some(sampling) = capabilities.sampling.as_mut() {
                if sampling.tools.is_none() {
                    sampling.tools = Some(serde_json::Value::Object(serde_json::Map::new()));
                }
            }
        }
        if let Some(declared) = self.declared_extensions.as_ref() {
            // MERGE rather than assign: `ClientCapabilities::default()` carries
            // `extensions: None` today, but a future default that pre-seeded the
            // map would otherwise be silently discarded here.
            let slot = capabilities.extensions.get_or_insert_with(HashMap::new);
            for (key, value) in declared {
                slot.insert(key.clone(), value.clone());
            }
        }
        capabilities
    }

    /// Build the reserved `_meta` object every v2 request carries.
    ///
    /// Exactly three keys, all read by the server's `resolve_protocol_context`:
    /// `io.modelcontextprotocol/protocolVersion` (the era channel — a stateless
    /// v2 request has no handshake, so this is the ONLY one),
    /// `io.modelcontextprotocol/clientInfo` and
    /// `io.modelcontextprotocol/clientCapabilities`.
    ///
    /// `clientInfo` is SELF-REPORTED and unverified by construction — a server
    /// must never derive authorization from it (T-113-21).
    fn v2_request_meta(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert(
            META_PROTOCOL_VERSION.to_string(),
            serde_json::Value::String(
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            ),
        );
        if let Ok(info) = serde_json::to_value(&self.info) {
            meta.insert(META_CLIENT_INFO.to_string(), info);
        }
        if let Ok(capabilities) = serde_json::to_value(self.v2_client_capabilities()) {
            meta.insert(META_CLIENT_CAPABILITIES.to_string(), capabilities);
        }
        meta
    }

    /// MERGE the v2 reserved `_meta` keys into an outgoing request's `params`.
    ///
    /// Merge, never replace (T-113-54): a caller-supplied `_meta` — W3C trace
    /// context (`traceparent` / `tracestate` / `baggage`), a progress token, a
    /// namespaced extension key — SURVIVES, so plan 07's MRTR retries keep their
    /// distributed-tracing spans linked. Only the three reserved
    /// `io.modelcontextprotocol/*` keys are authoritative and overwrite.
    ///
    /// A `params` that is absent or not an object is replaced with a fresh
    /// object carrying only `_meta`: on v2 a request with no `_meta` has no era
    /// signal at all and would be rejected by the server's header gate.
    fn splice_v2_meta(&self, params: &mut Option<serde_json::Value>) {
        let reserved = self.v2_request_meta();
        if !matches!(params, Some(serde_json::Value::Object(_))) {
            // Replacing a NON-NULL, non-object `params` DISCARDS it — a JSON-RPC
            // array (positional) `params` cannot carry a `_meta` sibling. No MCP
            // method uses that shape, so this is unreachable from this crate's
            // own request types, but a silent drop of caller data must be
            // observable rather than inferred from a missing field server-side.
            if matches!(params, Some(value) if !value.is_null()) {
                tracing::warn!(
                    "v2 request params were not a JSON object and have been replaced with one \
                     carrying only the reserved _meta keys; the original params were discarded"
                );
            }
            *params = Some(serde_json::Value::Object(serde_json::Map::new()));
        }
        let Some(serde_json::Value::Object(object)) = params.as_mut() else {
            return;
        };
        let meta = object
            .entry(PARAMS_META_KEY.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if !meta.is_object() {
            *meta = serde_json::Value::Object(serde_json::Map::new());
        }
        let Some(meta) = meta.as_object_mut() else {
            return;
        };
        for (key, value) in reserved {
            meta.insert(key, value);
        }
    }

    /// Ask a v2 server for its capability projection (`server/discover`).
    ///
    /// v2 has no `initialize`, so this is how a client learns what the server
    /// supports. It is EXPLICIT: pmcp never calls it implicitly, and never uses
    /// it to CHOOSE an era (Phase-113 D-08 forbids exactly that auto-probe).
    /// Populating capabilities from a call the USER made is a different thing
    /// from probing to decide which protocol to speak — do not "restore" the
    /// latter.
    ///
    /// Takes `&mut self` because it STORES the returned capabilities: after this
    /// call `Self::assert_capability` enforces on v2 exactly as it does on v1
    /// against initialize-learned ones.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection did not opt into `2026-07-28`
    /// (`server/discover` does not exist on v1 — a v1 server answers `-32601`),
    /// when the transport fails, or when the server returns a JSON-RPC error.
    pub async fn server_discover(
        &mut self,
    ) -> Result<crate::types::protocol::ServerDiscoverResult> {
        self.require_v2(crate::types::protocol::SERVER_DISCOVER_METHOD)?;
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self
            .send_untyped_request(
                request_id,
                crate::types::protocol::SERVER_DISCOVER_METHOD,
                serde_json::json!({}),
            )
            .await?;
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let discovered = serde_json::from_value::<
                    crate::types::protocol::ServerDiscoverResult,
                >(result)
                .map_err(|e| Error::parse(format!("Invalid server/discover result: {e}")))?;
                self.server_capabilities = Some(discovered.capabilities.clone());
                self.server_version = Some(discovered.server_info.clone());
                Ok(discovered)
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get server capabilities after initialization.
    pub fn get_server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Get server version information after initialization.
    pub fn get_server_version(&self) -> Option<&Implementation> {
        self.server_version.as_ref()
    }

    /// Get server instructions after initialization.
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Send a ping to the server.
    pub async fn ping(&self) -> Result<()> {
        self.ensure_initialized()?;
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Set the logging level on the server.
    pub async fn set_logging_level(&self, level: LoggingLevel) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("logging", "logging/setLevel")?;

        let request = Request::Client(Box::new(ClientRequest::SetLoggingLevel { level }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available tools.
    ///
    /// Retrieves information about all tools available on the server, including
    /// their names, descriptions, and input schemas.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all tools
    /// let tools = client.list_tools(None).await?;
    /// for tool in tools.tools {
    ///     println!("Tool: {} - {}",
    ///              tool.name,
    ///              tool.description.unwrap_or_else(|| "No description".to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional pagination cursor for retrieving additional results
    pub async fn list_tools(&self, cursor: Option<String>) -> Result<ListToolsResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool.
    ///
    /// Invokes a server-provided tool with the specified name and arguments.
    /// The server must have declared the tool via the tools capability during initialization.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `arguments` - JSON value containing the tool's arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Call a simple tool with no arguments
    /// let result = client.call_tool(
    ///     "list_files".to_string(),
    ///     json!({})
    /// ).await?;
    ///
    /// // Call a tool with specific arguments
    /// let search_result = client.call_tool(
    ///     "search".to_string(),
    ///     json!({
    ///         "query": "rust programming",
    ///         "limit": 10
    ///     })
    /// ).await?;
    ///
    /// // Tools can return structured data
    /// if let Some(content) = result.content.first() {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Tool result: {}", text);
    ///         }
    ///         _ => println!("Non-text tool result"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support tools
    /// - The tool name doesn't exist
    /// - The arguments are invalid for the tool
    /// - Network or protocol errors occur
    ///
    /// # v2 (`2026-07-28`) behavior
    ///
    /// On a connection that opted into `2026-07-28`, this method
    /// auto-orchestrates Multi-Round-Trip Elicitation: an `input_required`
    /// result is answered from the registered host handlers and the request is
    /// resent, up to [`ClientBuilder::mrtr_round_limit`] rounds.
    ///
    /// Two v2-only error outcomes are therefore possible, both programmatically
    /// distinguishable:
    ///
    /// - [`Error::is_input_required_unfulfilled`] — no handler could answer, so
    ///   the full result is handed back via [`Error::input_required_result`].
    ///   It is an ERROR here (rather than a value) because
    ///   [`CallToolResult::content`] carries `#[serde(default)]` and would
    ///   otherwise deserialize such a result into a silently EMPTY success. Use
    ///   [`Self::call_tool_mrtr`] to receive it as a value instead.
    /// - [`Error::is_mrtr_round_limit_exceeded`] — the server kept asking.
    ///
    /// On v1 this method is byte-identical to every prior release.
    pub async fn call_tool(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        if self.is_v2() {
            let params = Self::call_tool_params(name, arguments)?;
            return Self::mrtr_result_or_error(
                self.send_with_mrtr(CALL_TOOL_METHOD, params).await?,
            );
        }

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // =========================================================================
    // MCP Tasks (2025-11-25)
    // =========================================================================

    /// Call a tool with task augmentation.
    ///
    /// Sends a `tools/call` request with the `task` field set, signaling to the
    /// server that this client supports async task polling. The server may return
    /// either a `CreateTaskResult` (async task created) or a `CallToolResult`
    /// (sync result) depending on the tool's `taskSupport` declaration.
    ///
    /// Use [`call_tool`](Self::call_tool) instead if you don't need task support.
    ///
    /// # Returns
    ///
    /// - `Ok(ToolCallResponse::Task(task))` if the server created an async task.
    ///   Poll with [`tasks_get`](Self::tasks_get) until the task reaches a
    ///   terminal status.
    /// - `Ok(ToolCallResponse::Result(result))` if the server returned the
    ///   result synchronously.
    pub async fn call_tool_with_task(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResponse> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: Some(serde_json::json!({})),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;
        self.decode_task_augmented_response(response)
    }

    /// Call a tool with task augmentation AND custom request `_meta`.
    ///
    /// Identical to [`call_tool_with_task`](Self::call_tool_with_task) except the
    /// request carries the supplied [`RequestMeta`](crate::types::protocol::RequestMeta)
    /// as `_meta`, so namespaced/guard state (attached via
    /// [`RequestMeta::with_meta`](crate::types::protocol::RequestMeta::with_meta))
    /// travels alongside the task augmentation in a single `tools/call`.
    ///
    /// Passing an empty `RequestMeta` behaves like `call_tool_with_task`.
    ///
    /// # Returns
    ///
    /// - `Ok(ToolCallResponse::Task(task))` if the server created an async task.
    /// - `Ok(ToolCallResponse::Result(result))` if the server returned the
    ///   result synchronously.
    ///
    /// # Errors
    ///
    /// Returns an error if the client is not initialized, the server does not
    /// support tools, or a network/protocol error occurs.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use pmcp::{Client, types::protocol::RequestMeta};
    /// # async fn run(client: &Client<pmcp::StdioTransport>) -> pmcp::Result<()> {
    /// let meta = RequestMeta::new()
    ///     .with_meta("io.modelcontextprotocol/related-task", serde_json::json!({"taskId": "t-1"}));
    /// let resp = client
    ///     .call_tool_with_task_and_meta("member".to_string(), serde_json::json!({}), meta)
    ///     .await?;
    /// # let _ = resp;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_tool_with_task_and_meta(
        &self,
        name: String,
        arguments: serde_json::Value,
        meta: crate::types::protocol::RequestMeta,
    ) -> Result<ToolCallResponse> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: Some(meta),
            task: Some(serde_json::json!({})),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;
        self.decode_task_augmented_response(response)
    }

    /// Decode the response of a task-augmented `tools/call`, ERA-AWARE.
    ///
    /// The ONE decoder [`Self::call_tool_with_task`] and
    /// [`Self::call_tool_with_task_and_meta`] share. Those two carried a
    /// line-for-line identical copy of this arm before Phase 114 plan 19, and
    /// making them era-aware would have created a third.
    ///
    /// # v2 branches on the DISCRIMINATOR, never on the shape
    ///
    /// The v1 arm below is the historical "try `CreateTaskResult`, else
    /// `CallToolResult`" try-first-then-fall-back, kept byte-for-byte because v1
    /// wire behaviour must not move. Its own comment warns against key-name
    /// duck-typing, and on v2 the warning becomes load-bearing: the v2 create
    /// payload is FLAT, so it and an ordinary `CallToolResult` no longer have a
    /// reliably discriminating key — `CallToolResult::content` carries
    /// `#[serde(default)]`, so essentially any object decodes as one.
    ///
    /// v2 therefore reads [`RESULT_TYPE_KEY`] and compares it to
    /// [`TASK_RESULT_TYPE`], the SAME constant the server's
    /// `ResponseDisposition::as_wire_str` emits. An absent or unrecognised
    /// `resultType` is an ordinary complete result — the absent-means-complete
    /// rule Phase 112 established for this envelope (T-114-99: a server cannot
    /// steer a client's decode branch by crafting a task-SHAPED ordinary
    /// result, because the shape is not consulted).
    fn decode_task_augmented_response(
        &self,
        response: crate::types::JSONRPCResponse,
    ) -> Result<ToolCallResponse> {
        let result = match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => result,
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                return Err(Error::from_jsonrpc_error(error))
            },
        };
        if self.is_v2() {
            return Self::decode_v2_task_augmented_result(result);
        }
        Self::decode_v1_task_augmented_result(result)
    }

    /// The v2 arm of [`Self::decode_task_augmented_response`].
    ///
    /// Every arm is named, including the two that mean the same thing: the
    /// server's explicit `"complete"` and an ABSENT discriminator are one answer
    /// (absent-means-complete), and an unrecognised value is that answer too —
    /// but observably, because a value this client does not know is a protocol
    /// version skew a developer needs to see, not something to swallow.
    fn decode_v2_task_augmented_result(result: serde_json::Value) -> Result<ToolCallResponse> {
        let created_a_task = match result
            .get(RESULT_TYPE_KEY)
            .and_then(serde_json::Value::as_str)
        {
            Some(TASK_RESULT_TYPE) => true,
            Some(COMPLETE_RESULT_TYPE) | None => false,
            Some(unknown) => {
                tracing::debug!(
                    target: "mcp.tasks",
                    result_type = unknown,
                    "unrecognised {RESULT_TYPE_KEY} on a v2 tools/call result; treating it as a \
                     complete result (absent-means-complete)"
                );
                false
            },
        };
        if !created_a_task {
            let tool_result: CallToolResult =
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))?;
            return Ok(ToolCallResponse::Result(tool_result));
        }
        let created: TaskV2 =
            serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))?;
        Ok(ToolCallResponse::Task(created.to_v1()))
    }

    /// The v1 arm of [`Self::decode_task_augmented_response`] — FROZEN.
    fn decode_v1_task_augmented_result(result: serde_json::Value) -> Result<ToolCallResponse> {
        // Try CreateTaskResult first (more specific), fall back to CallToolResult.
        // This avoids brittle key-name duck-typing.
        if let Ok(task_result) = serde_json::from_value::<CreateTaskResult>(result.clone()) {
            return Ok(ToolCallResponse::Task(task_result.task));
        }
        let tool_result: CallToolResult =
            serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))?;
        Ok(ToolCallResponse::Result(tool_result))
    }

    /// Call a tool (non-task) with custom request `_meta`.
    ///
    /// Identical to [`call_tool`](Self::call_tool) except the request carries the
    /// supplied [`RequestMeta`](crate::types::protocol::RequestMeta) as `_meta`.
    /// Namespaced/guard state travels via
    /// [`RequestMeta::with_meta`](crate::types::protocol::RequestMeta::with_meta)
    /// and is visible to the server tool handler through `extra.request_meta`.
    ///
    /// Passing an empty `RequestMeta` behaves like `call_tool`.
    ///
    /// # Errors
    ///
    /// Returns an error if the client is not initialized, the server does not
    /// support tools, the tool name does not exist, or a network/protocol error
    /// occurs.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use pmcp::{Client, types::protocol::RequestMeta};
    /// # async fn run(client: &Client<pmcp::StdioTransport>) -> pmcp::Result<()> {
    /// let meta = RequestMeta::new().with_meta("x-pmcp-team-depth", serde_json::json!(1));
    /// let result = client
    ///     .call_tool_with_meta("echo".to_string(), serde_json::json!({}), meta)
    ///     .await?;
    /// # let _ = result;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_tool_with_meta(
        &self,
        name: String,
        arguments: serde_json::Value,
        meta: crate::types::protocol::RequestMeta,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: Some(meta),
            task: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get the current status of a task.
    ///
    /// Polls the server for the task's current state. Call this repeatedly
    /// (respecting `task.poll_interval`) until the task reaches a terminal
    /// status (`Completed`, `Failed`, or `Cancelled`).
    ///
    /// # Era awareness (Phase 114, plan 19)
    ///
    /// | Era | Wire shape | How it becomes a [`Task`] |
    /// |-----|-----------|---------------------------|
    /// | v1 | NESTED `{"task": {…, "ttl", "pollInterval"}}` | unchanged: decode `GetTaskResult`, return `.task` |
    /// | v2 | FLAT `{taskId, status, createdAt, lastUpdatedAt, ttlMs, …}` | decode `TaskV2`, then [`TaskV2::to_v1`](crate::types::tasks::TaskV2::to_v1) |
    ///
    /// The signature is unchanged on purpose: `ttlMs` lands on [`Task::ttl`] and
    /// `pollIntervalMs` on [`Task::poll_interval`], so an existing caller's poll
    /// logic keeps working verbatim against a v2 server.
    ///
    /// The v2 arm decodes only the flat BASE task, never the status-discriminated
    /// `DetailedTask`. That is deliberate: a backend that cannot supply a
    /// terminal task's `result` degrades to the bare flat `Task`, and a strict
    /// decode here would turn "I could not read the detail" into "I could not
    /// read the task at all". Use [`Self::tasks_get_detailed`] when you want the
    /// inlined detail and want a missing one to be an error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The server doesn't support tasks
    /// - The task ID doesn't exist or belongs to another owner. On v2 that is
    ///   ONE `-32602` answer for absent / wrong-owner / EXPIRED alike — the
    ///   three are deliberately indistinguishable (no existence oracle), so a
    ///   client must not try to tell them apart.
    pub async fn tasks_get(&self, task_id: &str) -> Result<Task> {
        if self.is_v2() {
            let raw = self.tasks_get_raw_v2(task_id).await?;
            return Ok(Self::decode_v2_task_base(&raw)?.to_v1());
        }

        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_GET_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
            task_id: task_id.to_string(),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        let task_result: GetTaskResult =
            self.parse_task_payload(response, TASKS_GET_METHOD).await?;
        Ok(task_result.task)
    }

    /// Get a task's status TOGETHER WITH its status-conditional detail — v2 only
    /// (Phase 114, plan 19).
    ///
    /// The additive sibling of [`Self::tasks_get`]. On `2026-07-28` a
    /// `tasks/get` result is a flat `DetailedTask`: one variant per status,
    /// each carrying exactly the key its schema variant marks required —
    /// `result` on `completed`, `error` on `failed`, `inputRequests` on
    /// `input_required`, nothing extra on `working` / `cancelled`.
    ///
    /// This is what removes the second round trip v1 needed: `tasks/result` does
    /// not exist on v2 because the terminal payload is already here.
    ///
    /// # Strict by design
    ///
    /// [`DetailedTaskV2::from_wire_value`](crate::types::tasks::DetailedTaskV2::from_wire_value)
    /// is STATUS-DIRECTED: it reads `status` first and then REQUIRES that
    /// status's key. A `completed` task with no `result` is an error here rather
    /// than a best-effort decode into a variant that happens to fit — the same
    /// discipline the server-side projection applies when it emits.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidState`] when the connection did not opt into
    ///   `2026-07-28` — NO request is sent.
    /// - The transport / JSON-RPC errors [`Self::tasks_get`] returns.
    /// - [`Error::Protocol`] carrying `ErrorCode::PARSE_ERROR` (built by
    ///   [`Error::parse`]) when the payload's status and detail disagree.
    pub async fn tasks_get_detailed(&self, task_id: &str) -> Result<DetailedTaskV2> {
        self.require_v2("Client::tasks_get_detailed")?;
        let raw = self.tasks_get_raw_v2(task_id).await?;
        DetailedTaskV2::from_wire_value(&raw).map_err(Error::parse)
    }

    /// Send one v2 `tasks/get` and hand back the RAW result object.
    ///
    /// The single v2 fetch site: [`Self::tasks_get`], [`Self::tasks_get_detailed`]
    /// and the poll loop all go through it, so the flat payload is fetched ONCE
    /// per tick and both the base task and the inlined detail are read from the
    /// SAME bytes. Decoding twice from one value is free; asking the server
    /// twice is a second round trip and a second chance for the two answers to
    /// disagree.
    async fn tasks_get_raw_v2(&self, task_id: &str) -> Result<serde_json::Value> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_GET_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
            task_id: task_id.to_string(),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;
        self.parse_task_payload::<serde_json::Value>(response, TASKS_GET_METHOD)
            .await
    }

    /// Decode the flat v2 base task out of a raw `tasks/get` payload.
    fn decode_v2_task_base(raw: &serde_json::Value) -> Result<TaskV2> {
        serde_json::from_value(raw.clone()).map_err(|e| Error::parse(e.to_string()))
    }

    /// Get the final result of a completed task — **v1 only**.
    ///
    /// For a task-augmented `tools/call`, this returns the `CallToolResult`
    /// that the tool would have returned synchronously. Only valid when
    /// the task status is `Completed`.
    ///
    /// # RETIRED on `2026-07-28` (Phase 114, TASK-03 / D-15)
    ///
    /// `tasks/result` is absent from the tasks extension: the v2 `tasks/get`
    /// INLINES the terminal payload, so a second round trip has nothing left to
    /// do. A v2 server answers this method `-32601`. Calling it on a v2
    /// connection therefore fails LOCALLY — no bytes leave the process — with an
    /// [`Error::retired_on_v2`] naming [`Self::tasks_get_detailed`]'s method as
    /// the replacement. A clear local error beats a round trip to an opaque
    /// method-not-found.
    pub async fn tasks_result(&self, task_id: &str) -> Result<CallToolResult> {
        self.reject_retired_tasks_method_on_v2(TASKS_RESULT_METHOD, TASKS_GET_METHOD)?;
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_RESULT_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest {
                task_id: task_id.to_string(),
            },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        self.parse_task_payload::<CallToolResult>(response, TASKS_RESULT_METHOD)
            .await
    }

    /// Refuse a `tasks/*` method the `2026-07-28` tasks extension does not
    /// declare, LOCALLY and before any bytes go on the wire.
    ///
    /// The tasks twin of [`Self::reject_if_retired_on_v2`], kept separate from it
    /// because the replacement differs per method and because the two families
    /// were retired by different specs. Both mint the SAME
    /// [`Error::retired_on_v2`] marker, so a caller has ONE typed check
    /// ([`Error::is_retired_on_v2`]) for "this method is gone on v2".
    ///
    /// On v1 it is a no-op, so the legacy path stays byte-identical.
    fn reject_retired_tasks_method_on_v2(&self, method: &str, replacement: &str) -> Result<()> {
        if self.is_v2() {
            return Err(Error::retired_on_v2(method, replacement));
        }
        Ok(())
    }

    /// Poll a task to terminal status, then return its final result.
    ///
    /// Drives `tasks/get` in a loop until [`TaskStatus::is_terminal`], honoring
    /// the polling interval (caller override, else the task-reported
    /// `pollInterval`, else a built-in default) and an optional overall timeout.
    ///
    /// # Where the terminal result comes from is ERA-SPLIT (Phase 114, plan 19)
    ///
    /// | Era | Terminal step |
    /// |-----|---------------|
    /// | v1 | a second round trip: [`Client::tasks_result`], exactly as before |
    /// | v2 | ZERO extra round trips — the result is INLINE in the `tasks/get` payload the loop already fetched |
    ///
    /// `tasks/result` does not exist on `2026-07-28` (a v2 server answers
    /// `-32601`), so the v2 arm must not call it — and does not need to, because
    /// a v2 `tasks/get` on a `completed` task carries `result` and on a `failed`
    /// task carries `error`. Nothing else in the loop is era-aware: the
    /// classifier, the floor, the budget clamp and the clock are shared.
    ///
    /// On v2 a task that reaches `failed` surfaces its inlined JSON-RPC `error`
    /// as a typed client error rather than an empty success, and a `cancelled`
    /// task is an error too — neither has a result to return.
    ///
    /// # Wasm safety
    ///
    /// The delay between polls uses [`crate::runtime::sleep`] (not
    /// `tokio::time::sleep` directly) and the timeout is measured with
    /// [`web_time::Instant`] (not `std::time::Instant`, which panics on
    /// `wasm32`), so this compiles and runs in the browser.
    ///
    /// # Hot-loop protection
    ///
    /// The effective interval is clamped to a small floor (50 ms), so a zero or
    /// absent `pollInterval` cannot turn the loop into a busy spin.
    ///
    /// # Errors
    ///
    /// - Propagates `tasks/get` / `tasks/result` transport and protocol errors.
    /// - Returns [`Error::Timeout`] when `opts.max_poll_duration_secs` elapses
    ///   before the task reaches a terminal status. Each sleep is clamped to
    ///   the remaining budget, so a large (possibly server-reported) poll
    ///   interval cannot overshoot the caller's budget by more than roughly
    ///   the 50 ms clamp floor.
    /// - Returns [`Error::Validation`] when the task enters
    ///   [`TaskStatus::InputRequired`]: that state is NOT terminal and needs
    ///   client-side action (elicitation) this poller cannot provide, so
    ///   polling on would hang forever under the default (unbounded) options.
    ///   Handle the required input, then resume polling — or use
    ///   [`Client::wait_for_task_with_inputs`], which is exactly this poller
    ///   with a responder attached and IS the answer to that message.
    ///
    /// # Durable and replay consumers
    ///
    /// Do **not** wrap `wait_for_task` inside a durable / replay workflow step.
    /// It sleeps, loops, and owns the whole polling lifecycle, which is
    /// non-deterministic under replay (each re-execution would re-sleep and
    /// re-poll). A durable consumer should instead call
    /// [`Task::poll_decision`](crate::types::tasks::Task::poll_decision) plus
    /// [`resolve_poll_interval`] once per tick inside its own memoized step and
    /// persist the decision
    /// between ticks — those are pure, replay-deterministic functions of the
    /// polled task, unlike this blocking poller (D-11 / D-16).
    ///
    /// See the pmcp-book "Durable and replay consumers" section
    /// (heading `## Durable and replay consumers` in
    /// `pmcp-book/src/ch12-7-tasks.md`) for the full per-poll pattern:
    /// <https://paiml.github.io/rust-mcp-sdk/ch12-7-tasks.html#durable-and-replay-consumers>.
    /// (This is a deliberate plain-text/URL reference, not a rustdoc intra-doc
    /// link, so it never fails `cargo doc` even before that page ships.)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::client::WaitForTaskOptions;
    ///
    /// // `result` came from a task-augmented tools/call.
    /// if let Some(meta) = result.related_task() {
    ///     let final_result = client
    ///         .wait_for_related_task(&meta, WaitForTaskOptions::default())
    ///         .await?;
    /// }
    /// ```
    pub async fn wait_for_task(
        &self,
        task_id: &str,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult> {
        // No responder: the `InputRequired` arm below returns its error, which is
        // the correct answer for a caller that supplied none. The turbofish names
        // a concrete never-called callback type because `None` alone leaves both
        // generic parameters unconstrained.
        self.poll_task_to_terminal(task_id, opts, None::<NoInputResponder>)
            .await
    }

    /// [`Client::wait_for_task`] WITH a responder for `input_required` — v2 only
    /// (Phase 114, TASK-02).
    ///
    /// The same poller, with one behaviour added: when the task pauses for
    /// input, `responder` is handed the task's `inputRequests` (read from the
    /// `tasks/get` payload the loop already fetched — no extra round trip), its
    /// answers are delivered with [`Client::tasks_update`], and polling resumes.
    ///
    /// Everything else is [`Client::wait_for_task`] VERBATIM — the same
    /// `poll_decision()` classifier matched with no wildcard arm, the same
    /// `MIN_POLL_MS` floor, the same remaining-budget clamp, the same
    /// `web_time::Instant` clock — because it is literally the same function
    /// with a responder passed in.
    ///
    /// # v2 only
    ///
    /// `tasks/update` does not exist on `2026-07-28`'s predecessor, so a v1 call
    /// fails LOCALLY with no bytes on the wire. Use
    /// [`Client::wait_for_task`] plus your own elicitation handling there.
    ///
    /// # The input rounds are BOUNDED
    ///
    /// A server that keeps re-requesting input cannot spin a client forever: the
    /// number of `input_required` rounds is capped by the SAME configured bound
    /// the MRTR gather->resend loop uses
    /// ([`ClientBuilder::mrtr_round_limit`], defaulting to
    /// `DEFAULT_MRTR_ROUND_LIMIT` = 8). It is deliberately the same knob and not
    /// a new constant: both bound "how many times will I answer this server's
    /// questions before I conclude it is not making progress", and two
    /// independently-tuned answers to one question is how they drift apart.
    /// Exceeding it returns [`Error::mrtr_round_limit_exceeded`].
    ///
    /// # A task is NOT a higher-trust channel
    ///
    /// The requests handed to `responder` are ordinary elicitation / sampling /
    /// roots requests that happen to arrive through a task. Apply the SAME
    /// consent and policy gates you would for a direct server-initiated request;
    /// this poller passes values through and executes nothing server-supplied.
    ///
    /// # Errors
    ///
    /// Everything [`Client::wait_for_task`] returns, plus whatever `responder`
    /// itself returns (propagated unchanged), plus the round-bound error above.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::client::WaitForTaskOptions;
    /// use pmcp::types::mrtr::InputResponses;
    ///
    /// let result = client
    ///     .wait_for_task_with_inputs("task-1", WaitForTaskOptions::default(), |requests| async move {
    ///         let mut answers = InputResponses::new();
    ///         for (key, request) in &requests {
    ///             answers.insert(key.clone(), answer_one(request).await?);
    ///         }
    ///         Ok(answers)
    ///     })
    ///     .await?;
    /// ```
    pub async fn wait_for_task_with_inputs<F, Fut>(
        &self,
        task_id: &str,
        opts: WaitForTaskOptions,
        responder: F,
    ) -> Result<CallToolResult>
    where
        F: FnMut(InputRequests) -> Fut,
        Fut: std::future::Future<Output = Result<InputResponses>>,
    {
        // `tasks/update` is v2-only, so a v1 call could never complete a round.
        // Refuse locally BEFORE the first `tasks/get` goes out.
        self.require_v2("Client::wait_for_task_with_inputs")?;
        self.poll_task_to_terminal(task_id, opts, Some(responder))
            .await
    }

    /// The ONE task poll loop, shared by [`Self::wait_for_task`] and
    /// [`Self::wait_for_task_with_inputs`].
    ///
    /// `responder` is the only difference between the two: `None` makes
    /// `input_required` the terminal error it has always been, `Some` makes it a
    /// round of the gather->update->resume loop.
    async fn poll_task_to_terminal<F, Fut>(
        &self,
        task_id: &str,
        opts: WaitForTaskOptions,
        mut responder: Option<F>,
    ) -> Result<CallToolResult>
    where
        F: FnMut(InputRequests) -> Fut,
        Fut: std::future::Future<Output = Result<InputResponses>>,
    {
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_GET_METHOD)?;

        // Wasm-safe monotonic clock: IS std::time::Instant on native, browser-safe on wasm.
        let start = web_time::Instant::now();
        let input_round_limit = self.mrtr_round_limit.max(1);
        let mut input_rounds = 0usize;
        loop {
            // ONE fetch per tick on both eras. On v2 the RAW payload is kept:
            // the terminal `result` / `error` and the pause's `inputRequests` are
            // INLINE in it, so re-reading it costs nothing while re-ASKING would
            // be a second round trip against a task that may have moved on.
            let (task, v2_payload) = if self.is_v2() {
                let raw = self.tasks_get_raw_v2(task_id).await?;
                (Self::decode_v2_task_base(&raw)?.to_v1(), Some(raw))
            } else {
                (self.tasks_get(task_id).await?, None)
            };

            // Single source of truth for the stop / ask / sleep decision: the
            // `poll_decision()` classifier in src/types/tasks.rs (D-13). No
            // parallel terminal-status or input-required comparison lives here,
            // so the poller and the classifier cannot drift. This matches the
            // `#[non_exhaustive]` `TaskPollDecision` exhaustively (no `_` arm)
            // because it is in-crate — a future variant becomes a compile error
            // here, forcing an explicit decision.
            match task.poll_decision() {
                // Terminal — the result comes from the payload already in hand on
                // v2, and from a second `tasks/result` round trip on v1.
                TaskPollDecision::Terminal { .. } => {
                    return match v2_payload {
                        Some(raw) => Self::terminal_result_from_v2_payload(task_id, &raw),
                        None => self.tasks_result(task_id).await,
                    };
                },
                // `input_required` is NOT terminal, and the task cannot progress
                // without client-side action. With no responder, surface it
                // (returning BEFORE any tasks/result fetch) instead of spinning
                // until a (possibly absent) timeout (CR-01).
                TaskPollDecision::InputRequired => {
                    let Some(callback) = responder.as_mut() else {
                        return Err(Error::validation(format!(
                            "task {task_id} is input_required; wait_for_task cannot provide \
                             input — handle the elicitation, then resume polling"
                        )));
                    };
                    input_rounds += 1;
                    if input_rounds > input_round_limit {
                        return Err(Error::mrtr_round_limit_exceeded(input_round_limit));
                    }
                    let requests =
                        Self::input_requests_from_v2_payload(task_id, v2_payload.as_ref())?;
                    let responses = callback(requests).await?;
                    self.tasks_update(task_id, responses).await?;
                    // Poll again immediately: the server has the answers now, so
                    // sleeping a full interval before asking would only add
                    // latency. The round bound above is what stops a server that
                    // keeps re-requesting.
                },
                // Still running — resolve the next sleep through the shared
                // resolver (D-02: caller override, else the server-reported
                // pollInterval hint, else the default, floored to MIN_POLL_MS).
                TaskPollDecision::InProgress { poll_hint } => {
                    let mut interval = resolve_poll_interval(opts.poll_interval, poll_hint);

                    // Enforce the overall polling budget (millisecond precision)
                    // and clamp the next sleep to the REMAINING budget — the
                    // interval may be server-chosen (task-reported pollInterval),
                    // and an unclamped sleep would overshoot a caller-specified
                    // budget by up to one arbitrary server interval. This clamp
                    // is loop state (not task state), so it stays INLINE here
                    // rather than moving into the classifier or resolver (WR-01 /
                    // D-09).
                    if let Some(max_secs) = opts.max_poll_duration_secs {
                        let budget_ms = max_secs.saturating_mul(1000);
                        let elapsed_ms =
                            u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let remaining_ms = budget_ms.saturating_sub(elapsed_ms);
                        if remaining_ms == 0 {
                            return Err(Error::timeout(budget_ms));
                        }
                        interval = interval.min(remaining_ms.max(MIN_POLL_MS));
                    }
                    crate::runtime::sleep(std::time::Duration::from_millis(interval)).await;
                },
            }
        }
    }

    /// Read a terminal task's final result out of the v2 `tasks/get` payload the
    /// poll loop already holds (Phase 114, plan 19).
    ///
    /// This is what replaces the v2-retired `tasks/result` round trip. The decode
    /// is STATUS-DIRECTED, so a `completed` task with no inlined `result` is an
    /// error rather than a silently EMPTY success — `CallToolResult::content`
    /// carries `#[serde(default)]`, which is exactly how that lie would slip
    /// through a permissive decode.
    fn terminal_result_from_v2_payload(
        task_id: &str,
        raw: &serde_json::Value,
    ) -> Result<CallToolResult> {
        let detailed = DetailedTaskV2::from_wire_value(raw).map_err(Error::parse)?;
        // `TaskDetailV2` is deliberately NOT `#[non_exhaustive]` (one variant per
        // `TaskStatus`), so this match is exhaustive with no wildcard arm.
        match detailed.detail() {
            TaskDetailV2::Completed { result } => {
                serde_json::from_value(serde_json::Value::Object(result.clone()))
                    .map_err(|e| Error::parse(e.to_string()))
            },
            // A failed task carries a JSON-RPC error object, and it must surface
            // AS an error: returning `Ok` with an empty result would report a
            // protocol failure as a successful tool call.
            TaskDetailV2::Failed { error } => {
                Err(Self::error_from_inlined_task_error(task_id, error.clone()))
            },
            TaskDetailV2::Cancelled => Err(Error::validation(format!(
                "task {task_id} was cancelled; a cancelled task has no result"
            ))),
            // Unreachable: `poll_decision()` returned `Terminal`, which only the
            // three statuses above produce. Reported rather than `unwrap`ed
            // because it would mean the payload's status and detail disagree.
            TaskDetailV2::Working | TaskDetailV2::InputRequired { .. } => Err(Error::internal(
                format!("task {task_id} reported a terminal status with non-terminal detail"),
            )),
        }
    }

    /// Turn a v2 task's INLINED JSON-RPC error object into a typed client error.
    ///
    /// Routed through [`Error::from_jsonrpc_error`] so a failed task is
    /// indistinguishable, to a caller matching on `code`, from the same failure
    /// delivered synchronously.
    fn error_from_inlined_task_error(
        task_id: &str,
        error: serde_json::Map<String, serde_json::Value>,
    ) -> Error {
        match serde_json::from_value::<crate::types::jsonrpc::JSONRPCError>(
            serde_json::Value::Object(error),
        ) {
            Ok(rpc_error) => Error::from_jsonrpc_error(rpc_error),
            Err(e) => Error::parse(format!(
                "task {task_id} failed, but its inlined error is malformed: {e}"
            )),
        }
    }

    /// Read a paused task's outstanding `inputRequests` out of the v2
    /// `tasks/get` payload the poll loop already holds.
    fn input_requests_from_v2_payload(
        task_id: &str,
        raw: Option<&serde_json::Value>,
    ) -> Result<InputRequests> {
        let raw = raw.ok_or_else(|| {
            Error::internal(format!(
                "task {task_id} paused for input on a connection with no v2 payload"
            ))
        })?;
        let detailed = DetailedTaskV2::from_wire_value(raw).map_err(Error::parse)?;
        match detailed.detail() {
            TaskDetailV2::InputRequired { input_requests } => Ok(input_requests.clone()),
            TaskDetailV2::Working
            | TaskDetailV2::Completed { .. }
            | TaskDetailV2::Failed { .. }
            | TaskDetailV2::Cancelled => Err(Error::internal(format!(
                "task {task_id} classified as input_required but its payload carries no \
                 inputRequests"
            ))),
        }
    }

    /// Poll a task referenced by [`TaskMetadata`] to terminal, then return its
    /// `tasks/result` — the zero-glue counterpart of [`Client::wait_for_task`].
    ///
    /// Any fields left unset in `opts` are filled from `meta`
    /// ([`WaitForTaskOptions::or_from_metadata`]) so a caller who holds a
    /// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task)
    /// result composes without hand-copying poll fields.
    ///
    /// # Errors
    ///
    /// Same as [`Client::wait_for_task`].
    pub async fn wait_for_related_task(
        &self,
        meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult> {
        self.wait_for_task(&meta.task_id, opts.or_from_metadata(meta))
            .await
    }

    /// List tasks owned by the current client — **v1 only**.
    ///
    /// # RETIRED on `2026-07-28` (Phase 114, TASK-03 / D-15)
    ///
    /// `tasks/list` is absent from the tasks extension, removed as a SECURITY
    /// improvement: with no enumeration primitive a server cannot inadvertently
    /// leak the existence of one caller's tasks to another. There is no
    /// replacement method — a v2 client keeps the ids it was handed. Calling
    /// this on a v2 connection fails LOCALLY with an [`Error::retired_on_v2`],
    /// with no bytes on the wire.
    pub async fn tasks_list(&self, cursor: Option<String>) -> Result<ListTasksResult> {
        self.reject_retired_tasks_method_on_v2(TASKS_LIST_METHOD, TASKS_LIST_V2_REPLACEMENT)?;
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_LIST_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksList(ListTasksRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        self.parse_task_payload::<ListTasksResult>(response, TASKS_LIST_METHOD)
            .await
    }

    /// Cancel a running task and report the task as the server now sees it.
    ///
    /// # Era awareness (Phase 114, plan 19)
    ///
    /// v1 answers a cancel with the NESTED `{"task": {…}}` envelope and this
    /// method returns that task, exactly as it always has.
    ///
    /// v2's `CancelTaskResult` is `Result` — an **EMPTY acknowledgement** with no
    /// task body at all (inventory row 20), so today's `CancelTaskResult` decode
    /// fails outright against it. Because this method's return type is `Task` and
    /// cannot change without a MAJOR semver bump, the v2 arm acknowledges the
    /// cancel through [`Self::tasks_cancel_ack`] and then performs ONE follow-up
    /// [`Self::tasks_get`]. It does NOT synthesise a `Task`: fabricating
    /// `status: cancelled` would be inventing status information the server
    /// deliberately did not send.
    ///
    /// # Cancellation is cooperative and eventually consistent
    ///
    /// That is the SEMANTICS of the empty ack, not a limitation of this client.
    /// The returned task MAY still be `working`, and MAY later settle on a
    /// terminal status other than `cancelled`. Callers that only need the
    /// acknowledgement — and do not want the extra round trip — should call
    /// [`Self::tasks_cancel_ack`] directly.
    pub async fn tasks_cancel(&self, task_id: &str) -> Result<Task> {
        if self.is_v2() {
            self.tasks_cancel_ack(task_id).await?;
            return self.tasks_get(task_id).await;
        }

        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_CANCEL_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksCancel(CancelTaskRequest {
            task_id: task_id.to_string(),
            result: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        let cancel_result: CancelTaskResult = self
            .parse_task_payload(response, TASKS_CANCEL_METHOD)
            .await?;
        Ok(cancel_result.task)
    }

    /// Request cancellation and read only the ACKNOWLEDGEMENT (Phase 114).
    ///
    /// The zero-invention primitive [`Self::tasks_cancel`] is built on: it
    /// accepts ANY successful result — including v2's bare `{}` — and returns
    /// `()`. One round trip, and nothing is claimed about the task's status.
    ///
    /// Works on BOTH eras. On v1 the response body carries a `Task` which this
    /// method DISCARDS; call [`Self::tasks_cancel`] when you want it.
    ///
    /// Cancellation is cooperative and eventually consistent: a successful
    /// acknowledgement means the request was accepted, NOT that the task has
    /// stopped.
    pub async fn tasks_cancel_ack(&self, task_id: &str) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_CANCEL_METHOD)?;

        let request = Request::Client(Box::new(ClientRequest::TasksCancel(CancelTaskRequest {
            task_id: task_id.to_string(),
            result: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Deliver responses to a paused task's outstanding `inputRequests` — v2
    /// only (Phase 114, TASK-02).
    ///
    /// `tasks/update` is how an `input_required` task is un-paused: each key of
    /// `responses` MUST correspond to a currently-outstanding `inputRequests`
    /// key from [`Self::tasks_get_detailed`]. The acknowledgement is EMPTY, so
    /// this returns `()`.
    ///
    /// # It is sent UNTYPED, on purpose
    ///
    /// There is no `ClientRequest::TasksUpdate` variant and there must not be
    /// one: [`ClientRequest`] is public and not `#[non_exhaustive]`, so adding a
    /// variant is a MAJOR semver break. This goes out through the same raw path
    /// [`Self::server_discover`] uses.
    ///
    /// # A task is NOT a higher-trust channel
    ///
    /// The spec is explicit that input requests delivered through a task carry
    /// exactly the trust of the elicitation / sampling they wrap. Whatever
    /// produces `responses` must apply the SAME consent and policy gates it
    /// would for a direct `elicitation/create`; this method transmits the values
    /// and executes nothing the server supplied.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidState`] on a v1 connection — `tasks/update` does not
    ///   exist there, and NO bytes are sent.
    /// - [`Error::UnsupportedCapability`] (built by [`Error::capability`]) when
    ///   the tasks extension was not negotiated — again with no bytes sent.
    /// - The server's own JSON-RPC error otherwise (e.g. an unknown or
    ///   already-answered input key).
    pub async fn tasks_update(&self, task_id: &str, responses: InputResponses) -> Result<()> {
        // Order matters only for WHICH local error you get; all three checks are
        // local, so every refusal below performs ZERO transport sends.
        self.require_v2(TASKS_UPDATE_METHOD)?;
        self.ensure_initialized()?;
        self.assert_capability("tasks", TASKS_UPDATE_METHOD)?;

        let mut params = serde_json::Map::new();
        params.insert(
            TASK_ID_KEY.to_string(),
            serde_json::Value::String(task_id.to_string()),
        );
        params.insert(
            INPUT_RESPONSES_KEY.to_string(),
            serde_json::to_value(&responses).map_err(|e| Error::parse(e.to_string()))?,
        );

        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self
            .send_untyped_request(
                request_id,
                TASKS_UPDATE_METHOD,
                serde_json::Value::Object(params),
            )
            .await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Deserialize a `tasks/*` response payload into `T`, emitting a structured
    /// WARN (method + transport identity + serde error) on a deserialize failure
    /// before surfacing it. Centralizes the four task endpoints' identical
    /// result-vs-error handling.
    ///
    /// Lock-on-error: the transport identity is read only on the cold failure
    /// path (D-LOCK-ON-ERROR — no cached field on `Client`).
    async fn parse_task_payload<D: serde::de::DeserializeOwned>(
        &self,
        response: crate::types::JSONRPCResponse,
        method: &'static str,
    ) -> Result<D> {
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                match serde_json::from_value::<D>(result) {
                    Ok(value) => Ok(value),
                    Err(e) => {
                        let transport = self.transport.read().await.transport_type();
                        Self::log_task_deserialize_error(
                            method,
                            std::any::type_name::<D>(),
                            transport,
                            &e,
                        );
                        Err(Error::parse(e.to_string()))
                    },
                }
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool and automatically poll until the task completes.
    ///
    /// This is a high-level convenience method that encapsulates the full
    /// task lifecycle:
    #[cfg(not(target_arch = "wasm32"))]
    /// 1. Calls the tool with task augmentation
    /// 2. If the server returns a task, polls `tasks/get` until terminal status
    /// 3. Returns the final `CallToolResult`
    ///
    /// If the server returns a sync result (no task), returns it immediately.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    /// * `arguments` - Tool arguments
    /// * `max_polls` - Maximum number of poll attempts before giving up (0 = unlimited)
    pub async fn call_tool_and_poll(
        &self,
        name: String,
        arguments: serde_json::Value,
        max_polls: usize,
    ) -> Result<CallToolResult> {
        /// Default polling interval when the server doesn't specify one.
        const DEFAULT_POLL_INTERVAL_MS: u64 = 5000;

        let response = self.call_tool_with_task(name, arguments).await?;

        match response {
            ToolCallResponse::Result(result) => Ok(result),
            ToolCallResponse::Task(initial_task) => {
                let task_id = initial_task.task_id.clone();
                let mut poll_ms = initial_task
                    .poll_interval
                    .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
                let mut polls = 0;

                loop {
                    polls += 1;

                    let task = self.tasks_get(&task_id).await?;

                    if task.status == TaskStatus::InputRequired {
                        return Err(Error::internal(format!(
                            "Task {} requires input — handle interactively via tasks_get/tasks_cancel",
                            task_id
                        )));
                    }

                    if task.status.is_terminal() {
                        if task.status == TaskStatus::Completed {
                            // Try to get the full result via tasks/result
                            match self.tasks_result(&task_id).await {
                                Ok(result) => return Ok(result),
                                // Only fall back for method-not-found (-32601); propagate real errors
                                Err(Error::Protocol { code, .. })
                                    if code == crate::error::ErrorCode::METHOD_NOT_FOUND =>
                                {
                                    let text = task
                                        .status_message
                                        .unwrap_or_else(|| "Task completed".to_string());
                                    return Ok(CallToolResult::new(vec![
                                        crate::types::Content::text(text),
                                    ]));
                                },
                                Err(e) => return Err(e),
                            }
                        } else {
                            // Failed or Cancelled
                            let text = task
                                .status_message
                                .unwrap_or_else(|| format!("Task {}", task.status));
                            return Ok(CallToolResult::error(vec![crate::types::Content::text(
                                text,
                            )]));
                        }
                    }

                    if max_polls > 0 && polls >= max_polls {
                        return Err(Error::internal(format!(
                            "Task {} did not complete after {} polls",
                            task_id, max_polls
                        )));
                    }

                    // Honor updated poll_interval from server (e.g., exponential backoff)
                    if let Some(interval) = task.poll_interval {
                        poll_ms = interval;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
                }
            },
        }
    }

    // =========================================================================
    // Prompts
    // =========================================================================

    /// List available prompts.
    ///
    /// Retrieves information about all prompts available on the server, including
    /// their names, descriptions, and required arguments.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large prompt lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all prompts
    /// let prompts = client.list_prompts(None).await?;
    /// for prompt in prompts.prompts {
    ///     println!("Prompt: {} - {}",
    ///              prompt.name,
    ///              prompt.description.unwrap_or_else(|| "No description".to_string()));
    ///     
    ///     // Show required arguments
    ///     if let Some(args) = prompt.arguments {
    ///         for arg in args {
    ///             println!("  - {}: {} (required: {})",
    ///                      arg.name,
    ///                      arg.description.unwrap_or_else(|| "No description".to_string()),
    ///                      arg.required);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - Network or protocol errors occur
    pub async fn list_prompts(&self, cursor: Option<String>) -> Result<ListPromptsResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get a prompt.
    ///
    /// Retrieves a specific prompt from the server with the provided arguments.
    /// The prompt is processed by the server and returned with filled-in content.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to retrieve
    /// * `arguments` - Key-value pairs for prompt arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use std::collections::HashMap;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Get a prompt with arguments
    /// let mut args = HashMap::new();
    /// args.insert("language".to_string(), "Rust".to_string());
    /// args.insert("topic".to_string(), "async programming".to_string());
    ///
    /// let prompt_result = client.get_prompt(
    ///     "code_review".to_string(),
    ///     args
    /// ).await?;
    ///
    /// println!("Prompt description: {}",
    ///          prompt_result.description.unwrap_or_else(|| "No description".to_string()));
    ///
    /// // Process the prompt messages
    /// for message in prompt_result.messages {
    ///     println!("Role: {}", message.role);
    ///     match &message.content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Content: {}", text);
    ///         }
    ///         _ => println!("Non-text content"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - The prompt name doesn't exist
    /// - Required arguments are missing
    /// - Network or protocol errors occur
    ///
    /// # v2 (`2026-07-28`) behavior
    ///
    /// Auto-orchestrates MRTR exactly as [`Self::call_tool`] documents,
    /// including the [`Error::is_input_required_unfulfilled`] and
    /// [`Error::is_mrtr_round_limit_exceeded`] outcomes. See
    /// [`Self::get_prompt_mrtr`] to receive an unfulfilled `input_required` as
    /// a value. v1 is byte-identical to every prior release.
    pub async fn get_prompt(
        &self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<GetPromptResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/get")?;

        if self.is_v2() {
            let params = Self::get_prompt_params(name, arguments)?;
            return Self::mrtr_result_or_error(
                self.send_with_mrtr(GET_PROMPT_METHOD, params).await?,
            );
        }

        let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name,
            arguments,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // === Typed call helpers ===

    /// Call a tool with typed, serializable arguments.
    ///
    /// Serializes `args` via `serde_json::to_value` and delegates to
    /// [`Self::call_tool`]. Serialization failures are mapped to
    /// [`Error::validation`] with the underlying serde error message.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Search { query: String, limit: u32 }
    ///
    /// let _ = client.call_tool_typed(
    ///     "search",
    ///     &Search { query: "rust mcp".into(), limit: 10 },
    /// ).await?;
    /// # Ok(()) }
    /// ```
    pub async fn call_tool_typed<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<CallToolResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed arguments: {e}")))?;
        self.call_tool(name.into(), value).await
    }

    /// Typed sibling of [`Self::call_tool_with_task`].
    ///
    /// Delegates to the two-argument [`Self::call_tool_with_task`]; there is no
    /// `TaskMetadata` parameter on the live client API, so none is exposed here.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    /// #[derive(Serialize)]
    /// struct Args { file: String }
    /// let _ = client.call_tool_typed_with_task("scan", &Args { file: "a.rs".into() }).await?;
    /// # Ok(()) }
    /// ```
    pub async fn call_tool_typed_with_task<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<ToolCallResponse> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed_with_task arguments: {e}")))?;
        self.call_tool_with_task(name.into(), value).await
    }

    /// Typed sibling of [`Self::call_tool_and_poll`].
    ///
    /// Delegates to the three-argument [`Self::call_tool_and_poll`]
    /// (`name, arguments, max_polls: usize`). There is no `poll_interval` or
    /// `TaskMetadata` parameter on the live client API — the server-supplied
    /// `poll_interval` is honoured internally by `call_tool_and_poll`.
    ///
    /// `max_polls = 0` means unlimited polls, matching the sibling's semantics.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    /// #[derive(Serialize)]
    /// struct Args { job: String }
    /// let _ = client.call_tool_typed_and_poll(
    ///     "build",
    ///     &Args { job: "nightly".into() },
    ///     30, // max_polls
    /// ).await?;
    /// # Ok(()) }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn call_tool_typed_and_poll<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
        max_polls: usize,
    ) -> Result<CallToolResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed_and_poll arguments: {e}")))?;
        self.call_tool_and_poll(name.into(), value, max_polls).await
    }

    /// Get a prompt with typed, serializable arguments.
    ///
    /// Serializes `args` to a JSON object, then coerces each leaf to a `String`
    /// for the wire-level `HashMap<String, String>` arguments:
    /// - `null` entries are omitted
    /// - `string` entries pass through unchanged (no JSON-quoting)
    /// - `number` and `bool` entries use `Display` (e.g. `42`, `true`)
    /// - `array` and `object` entries are re-serialized via
    ///   [`serde_json::to_string`]
    ///
    /// Non-object top-level serializations are rejected with
    /// [`Error::validation`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct SummaryArgs { topic: String, length: u32 }
    ///
    /// let _ = client.get_prompt_typed(
    ///     "summarize",
    ///     &SummaryArgs { topic: "rust async".into(), length: 200 },
    /// ).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_prompt_typed<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<GetPromptResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("get_prompt_typed arguments: {e}")))?;
        let serde_json::Value::Object(obj) = value else {
            return Err(Error::validation(
                "prompts/get arguments must serialize to a JSON object",
            ));
        };
        let mut out: HashMap<String, String> = HashMap::with_capacity(obj.len());
        for (k, v) in obj {
            match v {
                serde_json::Value::Null => {},
                serde_json::Value::String(s) => {
                    out.insert(k, s);
                },
                serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                    out.insert(k, v.to_string());
                },
                serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                    let nested = serde_json::to_string(&v).map_err(|e| {
                        Error::validation(format!("get_prompt_typed nested arg {k}: {e}"))
                    })?;
                    out.insert(k, nested);
                },
            }
        }
        self.get_prompt(name.into(), out).await
    }

    /// List available resources.
    ///
    /// Retrieves information about all resources available on the server, including
    /// their names, descriptions, URIs, and MIME types.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large resource lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resources
    /// let resources = client.list_resources(None).await?;
    /// for resource in resources.resources {
    ///     println!("Resource: {} ({})", resource.name, resource.uri);
    ///     if let Some(description) = resource.description {
    ///         println!("  Description: {}", description);
    ///     }
    ///     if let Some(mime_type) = resource.mime_type {
    ///         println!("  MIME Type: {}", mime_type);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - Network or protocol errors occur
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ListResourcesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List resource templates.
    ///
    /// Retrieves information about all resource templates available on the server.
    /// Resource templates define patterns for dynamically generated resources.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large template lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resource templates
    /// let templates = client.list_resource_templates(None).await?;
    /// for template in templates.resource_templates {
    ///     println!("Template: {} ({})", template.name, template.uri_template);
    ///     if let Some(description) = template.description {
    ///         println!("  Description: {}", description);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource templates
    /// - Network or protocol errors occur
    pub async fn list_resource_templates(
        &self,
        cursor: Option<String>,
    ) -> Result<ListResourceTemplatesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/templates/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResourceTemplates(
            ListResourceTemplatesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // === Auto-paginating list helpers ===

    /// List all tools across all pages, auto-paginating on `next_cursor`.
    ///
    /// Loops calling [`Self::list_tools`], terminating when the server returns
    /// `next_cursor: None`. Safety cap: if the loop runs more than
    /// `self.options.max_iterations` iterations (default `100`), returns
    /// [`Error::Validation`] instead of continuing or silently truncating.
    ///
    /// Empty-string cursors (`Some("")`) do NOT terminate the loop — only
    /// `None` does. This matches the MCP spec, which treats the cursor as an
    /// opaque server token and does not ascribe meaning to the empty string.
    ///
    /// # Memory
    ///
    /// This helper accumulates **all pages** in memory before returning. For
    /// very large servers, prefer the paginated single-page
    /// [`Self::list_tools`] and stream the output yourself — this helper is a
    /// convenience API and will amplify memory usage proportional to the
    /// total tool count.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_tools`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_tools exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let tools = client.list_all_tools().await?;
    /// println!("discovered {} tools", tools.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ToolInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_tools(cursor).await?;
            out.extend(page.tools);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_tools exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all prompts across all pages, auto-paginating on `next_cursor`.
    ///
    /// Semantics identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer [`Self::list_prompts`] for
    /// very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_prompts`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_prompts exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let prompts = client.list_all_prompts().await?;
    /// println!("discovered {} prompts", prompts.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_prompts(&self) -> Result<Vec<PromptInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<PromptInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_prompts(cursor).await?;
            out.extend(page.prompts);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_prompts exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all resources across all pages, auto-paginating on `next_cursor`.
    ///
    /// Semantics identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer [`Self::list_resources`] for
    /// very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_resources`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_resources exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let resources = client.list_all_resources().await?;
    /// println!("discovered {} resources", resources.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_resources(&self) -> Result<Vec<ResourceInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ResourceInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_resources(cursor).await?;
            out.extend(page.resources);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_resources exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all resource templates across all pages, auto-paginating on
    /// `next_cursor`.
    ///
    /// Uses the distinct `resources/templates/list` capability path (all
    /// other `list_all_*` helpers hit their own methods). Semantics otherwise
    /// identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer
    /// [`Self::list_resource_templates`] for very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_resource_templates`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_resource_templates exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let templates = client.list_all_resource_templates().await?;
    /// println!("discovered {} templates", templates.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_resource_templates(&self) -> Result<Vec<ResourceTemplate>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ResourceTemplate> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_resource_templates(cursor).await?;
            out.extend(page.resource_templates);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_resource_templates exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// Read a resource.
    ///
    /// Retrieves the content of a specific resource from the server by its URI.
    /// Resources can contain text, binary data, or structured content.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to read
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Read a text resource
    /// let resource = client.read_resource("file://readme.txt".to_string()).await?;
    /// for content in resource.contents {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Text content: {}", text);
    ///         }
    ///         pmcp::Content::Resource { uri, .. } => {
    ///             println!("Resource reference: {}", uri);
    ///         }
    ///         _ => println!("Other content type"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - The resource URI doesn't exist
    /// - Access to the resource is denied
    /// - Network or protocol errors occur
    ///
    /// # v2 (`2026-07-28`) behavior
    ///
    /// Auto-orchestrates MRTR exactly as [`Self::call_tool`] documents. This
    /// method is where the missing return type BIT the hardest:
    /// `ReadResourceResult.contents` has no serde default, so an
    /// `input_required` result cannot be deserialized into it at all and would
    /// surface as an opaque parse error. It now surfaces as an
    /// [`Error::is_input_required_unfulfilled`] carrying the full result, or —
    /// via [`Self::read_resource_mrtr`] — as a value. v1 is byte-identical to
    /// every prior release.
    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/read")?;

        if self.is_v2() {
            let params = Self::read_resource_params(uri)?;
            return Self::mrtr_result_or_error(
                self.send_with_mrtr(READ_RESOURCE_METHOD, params).await?,
            );
        }

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Subscribe to resource updates.
    ///
    /// Subscribes to receive notifications when a resource changes.
    /// The server will send notifications when the subscribed resource is modified.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to subscribe to
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a configuration file
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Now the client will receive notifications when settings.json changes
    /// // Handle notifications in your event loop
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # v2 behavior (2026-07-28)
    ///
    /// `resources/subscribe` was REMOVED from the 2026-07-28 schema and replaced
    /// by the `subscriptions/listen` stream. On a connection that opted into
    /// that version this method sends NOTHING and returns
    /// [`Error::retired_on_v2`](crate::Error::retired_on_v2) immediately — a
    /// v2 server answers the retired RPC with `404` + `-32601`, so the round
    /// trip can only fail. Use
    /// [`Client::subscriptions_listen`](Self::subscriptions_listen) with
    /// [`SubscriptionFilter::resource_subscriptions`](crate::types::subscriptions::SubscriptionFilter::resource_subscriptions)
    /// instead. The v1 path below is unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection speaks 2026-07-28 (see **v2 behavior** above)
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI doesn't exist
    /// - Network or protocol errors occur
    pub async fn subscribe_resource(&self, uri: String) -> Result<()> {
        // BEFORE `ensure_initialized` and before `assert_capability`: the era is
        // a property of the connection, and neither of those checks is
        // meaningful for a method the wire no longer defines.
        self.reject_if_retired_on_v2("resources/subscribe")?;
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/subscribe")?;

        // Check if server supports subscriptions
        if let Some(resources) = &self
            .server_capabilities
            .as_ref()
            .and_then(|c| c.resources.as_ref())
        {
            if !resources.subscribe.unwrap_or(false) {
                return Err(Error::capability(
                    "Server does not support resource subscriptions",
                ));
            }
        }

        let request = Request::Client(Box::new(ClientRequest::Subscribe(SubscribeRequest { uri })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Unsubscribe from resource updates.
    ///
    /// Unsubscribes from notifications for a previously subscribed resource.
    /// After unsubscribing, the client will no longer receive change notifications.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to unsubscribe from
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a resource
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Later, unsubscribe when no longer needed
    /// client.unsubscribe_resource("file://config/settings.json".to_string()).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # v2 behavior (2026-07-28)
    ///
    /// `resources/unsubscribe` was REMOVED from the 2026-07-28 schema along with
    /// `resources/subscribe`. On a connection that opted into that version this
    /// method sends NOTHING and returns
    /// [`Error::retired_on_v2`](crate::Error::retired_on_v2) immediately.
    /// Unsubscribing on v2 means DROPPING the
    /// [`SubscriptionStream`](crate::client::subscriptions::SubscriptionStream)
    /// returned by [`Client::subscriptions_listen`](Self::subscriptions_listen),
    /// which closes the connection and releases the server's registry slot. The
    /// v1 path below is unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection speaks 2026-07-28 (see **v2 behavior** above)
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI was not previously subscribed to
    /// - Network or protocol errors occur
    pub async fn unsubscribe_resource(&self, uri: String) -> Result<()> {
        self.reject_if_retired_on_v2("resources/unsubscribe")?;
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/unsubscribe")?;

        let request = Request::Client(Box::new(ClientRequest::Unsubscribe(UnsubscribeRequest {
            uri,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Request completion from the server.
    ///
    /// Requests auto-completion suggestions from the server for a given context.
    /// This is useful for implementing IDE-like features with contextual suggestions.
    ///
    /// # Arguments
    ///
    /// * `params` - The completion request parameters
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CompleteRequest};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Request completion for partial text
    /// let completion_request = CompleteRequest {
    ///     r#ref: pmcp::CompletionReference::Resource {
    ///         uri: "file://code.rs".to_string(),
    ///     },
    ///     argument: pmcp::CompletionArgument {
    ///         name: "function_name".to_string(),
    ///         value: "calc_".to_string(),
    ///     },
    /// };
    ///
    /// let completions = client.complete(completion_request).await?;
    /// for completion in completions.completion.values {
    ///     println!("Suggestion: {}", completion);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support completions
    /// - The completion context is invalid
    /// - Network or protocol errors occur
    pub async fn complete(&self, params: CompleteRequest) -> Result<CompleteResult> {
        self.ensure_initialized()?;
        self.assert_capability("completions", "completion/complete")?;

        let request = Request::Client(Box::new(ClientRequest::Complete(params)));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Create a message using sampling (for LLM providers).
    ///
    /// Requests the server to generate a message using its language model capabilities.
    /// This is typically used by servers that provide LLM functionality.
    ///
    /// # The "LLM-server pattern" (INVERSE of spec host sampling)
    ///
    /// This method is the **LLM-server pattern**: the *client* asks a *server*
    /// whose [`pmcp::SamplingHandler`](crate::SamplingHandler) runs the LLM. It
    /// is the **inverse** of MCP spec host sampling, where a server requests
    /// sampling and the client answers via a
    /// [`pmcp::client::host::HostSamplingHandler`](crate::client::host::HostSamplingHandler).
    /// Both directions are supported and neither is deprecated — pick the one
    /// that matches who owns the model. This path is unchanged by the client
    /// host surface.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CreateMessageParams, SamplingMessage};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let mut capabilities = ClientCapabilities::default();
    /// capabilities.sampling = Some(Default::default());
    ///
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(capabilities).await?;
    ///
    /// // Create a message with the LLM
    /// let msg = SamplingMessage::new(
    ///     pmcp::types::Role::User,
    ///     pmcp::types::SamplingMessageContent::Text {
    ///         text: "Explain how to implement a binary search tree".to_string(),
    ///         meta: None,
    ///     },
    /// );
    /// let prefs = pmcp::types::ModelPreferences::new()
    ///     .with_hints(vec![pmcp::types::ModelHint::new("gpt-4")])
    ///     .with_cost_priority(0.5)
    ///     .with_speed_priority(0.3)
    ///     .with_intelligence_priority(0.2);
    /// let mut request = CreateMessageParams::new(vec![msg])
    ///     .with_model_preferences(prefs)
    ///     .with_system_prompt("You are a helpful programming assistant")
    ///     .with_temperature(0.7)
    ///     .with_max_tokens(1000);
    /// request.include_context = pmcp::types::IncludeContext::ThisServer;
    ///
    /// let result = client.create_message(request).await?;
    /// println!("Model: {}", result.model);
    /// println!("Response: {:?}", result.content);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support sampling
    /// - The request parameters are invalid
    /// - Network or protocol errors occur
    pub async fn create_message(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        self.ensure_initialized()?;
        self.assert_capability("sampling", "sampling/createMessage")?;

        let request = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(params))));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Send roots list changed notification.
    ///
    /// Notifies the server that the client's root list has changed.
    /// This is typically sent when the workspace or project roots are modified.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport, ClientCapabilities};
    /// use pmcp::types::roots::{ListRootsResult, Root};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// // Roots advertisement is registry-derived (HOST-05): the client must
    /// // register a roots provider for the `roots` capability to reach the
    /// // wire. Build via `ClientBuilder` and register one with `on_roots`.
    /// let transport = StdioTransport::new();
    /// let mut client = ClientBuilder::new(transport)
    ///     .on_roots(|| async {
    ///         Ok(ListRootsResult {
    ///             roots: vec![Root {
    ///                 uri: "file:///workspace".to_string(),
    ///                 name: Some("workspace".to_string()),
    ///             }],
    ///         })
    ///     })
    ///     .build();
    ///
    /// // With a provider registered, a caller-set `list_changed` is preserved,
    /// // so the client advertises that it emits roots-list-changed notices.
    /// let mut capabilities = ClientCapabilities::default();
    /// capabilities.roots = Some(pmcp::RootsCapabilities { list_changed: true });
    /// client.initialize(capabilities).await?;
    ///
    /// // Notify server when project roots change
    /// client.send_roots_list_changed().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The client doesn't support roots list changed notifications
    /// - Network or protocol errors occur
    pub async fn send_roots_list_changed(&self) -> Result<()> {
        self.ensure_initialized()?;
        if let Some(roots) = &self.capabilities.as_ref().and_then(|c| c.roots.as_ref()) {
            if roots.list_changed {
                // OK, we support it
            } else {
                return Err(Error::capability(
                    "Client does not support roots list changed notifications",
                ));
            }
        }

        self.send_notification(Notification::Client(ClientNotification::RootsListChanged))
            .await
    }

    /// Authenticate with the server.
    ///
    /// Performs authentication using the provided authentication information.
    /// This should be called after initialization if the server requires authentication.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, AuthInfo, AuthScheme};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// // Initialize first
    /// client.initialize(pmcp::ClientCapabilities::default()).await?;
    ///
    /// // Authenticate with bearer token
    /// let auth = AuthInfo {
    ///     scheme: AuthScheme::Bearer,
    ///     token: Some("your-api-token".to_string()),
    ///     oauth: None,
    ///     params: Default::default(),
    /// };
    ///
    /// client.authenticate(&auth)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - Authentication fails
    /// - The server doesn't support authentication
    pub fn authenticate(&self, auth_info: &crate::types::AuthInfo) -> Result<()> {
        self.ensure_initialized()?;

        // In a real implementation, this would send an authentication request
        // For now, we'll just validate that we can authenticate
        match auth_info.scheme {
            crate::types::AuthScheme::None => Ok(()),
            crate::types::AuthScheme::Bearer => {
                if auth_info.token.is_none() {
                    return Err(Error::validation("Bearer token required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::OAuth2 => {
                if auth_info.oauth.is_none() {
                    return Err(Error::validation("OAuth information required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::Custom(_) => {
                // Custom auth schemes would be handled here
                Ok(())
            },
        }
    }

    /// Cancel a request.
    ///
    /// Sends a cancellation notification for an active request.
    /// This allows graceful termination of long-running operations.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to cancel
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, RequestId};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Start a long-running operation
    /// let request_id = RequestId::String("long-operation-123".to_string());
    ///
    /// // Later, cancel the request if needed
    /// client.cancel_request(&request_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the cancellation
    pub async fn cancel_request(&self, request_id: &RequestId) -> Result<()> {
        // Send cancellation notification
        self.send_notification(Notification::Cancelled(
            CancelledNotification::new(request_id.clone())
                .with_reason("User requested cancellation"),
        ))
        .await?;

        // Cancel any local tracking. Dropping the `Pending` also drops its
        // response sender, so a `dispatch_request` still awaiting this id sees
        // its channel close and stops waiting instead of blocking on a peer
        // that will never answer.
        let pending = self.active_requests.write().await.remove(request_id);
        if let Some(pending) = pending {
            let _ = pending.cancel.send(());
            // `Some` means the registration was still LIVE, so no answer had been
            // delivered and the peer may yet send one. Record the id so that late
            // answer is absorbed as our own debris rather than charged to whichever
            // unrelated call happens to be pumping when it lands — see
            // [`AbandonedRequestIds`]. A cancellation is a request to stop waiting,
            // not a claim that the peer will stop answering.
            self.abandoned_requests
                .write()
                .await
                .record(request_id.clone());
        }

        Ok(())
    }

    /// Send a progress notification.
    ///
    /// Sends a progress update for a long-running operation.
    /// This allows the server or client to track operation progress.
    ///
    /// # Arguments
    ///
    /// * `progress` - The progress notification to send
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, ProgressNotification, RequestId};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Send progress update for a file processing operation
    /// let progress = ProgressNotification::new(
    ///     pmcp::ProgressToken::String("file-processing".to_string()),
    ///     75.0,
    ///     Some("Processing files...".to_string()),
    /// );
    ///
    /// client.send_progress(progress).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the notification
    pub async fn send_progress(&self, progress: ProgressNotification) -> Result<()> {
        self.send_notification(Notification::Progress(progress))
            .await
    }

    /// Emit a `WARN` when a `tasks/*` response fails to deserialize.
    ///
    /// This is the single shared observability helper for all four task
    /// deserialize sites (`tasks/get`, `tasks/result`, `tasks/list`,
    /// `tasks/cancel`). It logs the originating `method`, the available
    /// transport identity, the deserialize `target` type, and the serde
    /// `error` — then the caller still returns `Err` (control flow is
    /// unchanged; this only adds observability, closing TASKDX-03).
    ///
    /// `transport` is [`Transport::transport_type`] (e.g. `"stdio"`,
    /// `"http"`) — the only server identity available here, because the
    /// `Transport` trait exposes no per-instance URL. TASKDX-03 logs this
    /// identity, not a genuine endpoint URL.
    fn log_task_deserialize_error(
        method: &'static str,
        target_type: &'static str,
        transport: &'static str,
        error: &serde_json::Error,
    ) {
        tracing::warn!(
            method = method,
            transport = transport,
            target = target_type,
            error = %error,
            "task response failed to deserialize",
        );
    }

    /// Check if client is initialized.
    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Error::InvalidState("Client not initialized".into()))
        }
    }

    /// Assert that the server has a specific capability.
    ///
    /// # Era awareness (Phase 113, CLNT-01)
    ///
    /// `server_capabilities` is populated ONLY by the `initialize` handshake,
    /// and v2 (`2026-07-28`) has no handshake. Left unguarded, a v2 client's
    /// `server_capabilities` is `None`, every `is_some_and(..)` below is `false`,
    /// and EVERY `call_tool` / `get_prompt` / `read_resource` fails locally
    /// before a byte leaves the process.
    ///
    /// So on v2 with nothing observed, this returns `Ok(())`: the client has not
    /// learned the server's capabilities and the SERVER is the authority. A v2
    /// server answers an unsupported method with `-32601` at HTTP 404, which is a
    /// truthful error from the party that knows, not a fabricated local one.
    ///
    /// Once an EXPLICIT [`Self::server_discover`] has stored a projection, v2
    /// enforcement is exactly as strict as v1. v1 is untouched and still fails
    /// closed.
    ///
    /// # `"tasks"` is era-SPLIT (Phase 114, D-04)
    ///
    /// The two eras spell the same capability in two different places, so the
    /// `"tasks"` arm reads a different field on each:
    ///
    /// | Era | Where the server advertises tasks |
    /// |-----|-----------------------------------|
    /// | v1 (`2025-11-25`) | `capabilities.tasks` |
    /// | v2 (`2026-07-28`) | `capabilities.extensions["io.modelcontextprotocol/tasks"]` |
    ///
    /// Reading `capabilities.tasks` on v2 would refuse EVERY conformant v2
    /// server: `core::project_capabilities_for_v2` strips that field from the
    /// v2 `server/discover` projection precisely because advertising it there
    /// would be a capability lie. See [`Self::tasks_capability_satisfied_by`].
    fn assert_capability(&self, capability: &str, method: &str) -> Result<()> {
        if self.is_v2() && self.server_capabilities.is_none() {
            return Ok(());
        }
        let has_capability = match capability {
            "tools" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.tools.is_some()),
            "prompts" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.prompts.is_some()),
            "resources" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.resources.is_some()),
            "logging" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.logging.is_some()),
            "completions" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.completions.is_some()),
            "tasks" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| self.tasks_capability_satisfied_by(c)),
            // The LLM-server pattern: `create_message` asks a server whose
            // `SamplingHandler` runs the LLM. A pmcp `Server` built with
            // `.sampling(handler)` advertises this by setting
            // `ServerCapabilities.sampling = Some(..)` (see
            // `src/server/mod.rs` `ServerBuilder::sampling`), so the check
            // mirrors that field. Without this arm every `create_message`
            // call fell through to `_ => false` and unconditionally errored.
            "sampling" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.sampling.is_some()),
            _ => {
                // A capability string reached here without a matching arm. This
                // is a programming error (a new capability was wired without
                // updating this match), not a server-side condition. Make it
                // loud in tests/debug builds while preserving the conservative
                // "not supported" behavior in release.
                tracing::error!(
                    "unknown capability string {capability:?} (required for {method}) — add an arm to assert_capability"
                );
                debug_assert!(
                    false,
                    "unknown capability string {capability:?} (required for {method}) — add an arm to assert_capability"
                );
                false
            },
        };

        if has_capability {
            Ok(())
        } else {
            Err(Error::capability(
                self.unsupported_capability_message(capability, method),
            ))
        }
    }

    /// Whether `capabilities` satisfies the `"tasks"` capability ON THIS ERA
    /// (Phase 114, D-04).
    ///
    /// - **v2** — the tasks extension is an Extensions-Track capability, so it
    ///   is satisfied iff the `extensions` map carries
    ///   [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY).
    ///   `capabilities.tasks` is deliberately NOT consulted: a v2 server
    ///   projects that field away, so reading it would refuse every conformant
    ///   v2 server.
    /// - **v1** — unchanged: `capabilities.tasks`.
    ///
    /// # PRESENCE, not `{}`-equality
    ///
    /// The check is `contains_key`. The draft schema types the value as
    /// `Record<string, never>` and pmcp's own server advertises exactly `{}`,
    /// but an operator may configure a richer value, and refusing to CALL a
    /// server that advertised support with a value we did not expect would be
    /// the mirror image of the over-removal the server-side v1 projection
    /// deliberately avoids. Presence is what the negotiation rule tests.
    fn tasks_capability_satisfied_by(&self, capabilities: &ServerCapabilities) -> bool {
        if self.is_v2() {
            return capabilities.extensions.as_ref().is_some_and(|extensions| {
                extensions.contains_key(crate::types::capabilities::TASKS_EXTENSION_KEY)
            });
        }
        capabilities.tasks.is_some()
    }

    /// Render the refusal [`assert_capability`](Self::assert_capability) returns.
    ///
    /// The v2 `"tasks"` refusal NAMES the extension key (T-114-23: a public,
    /// non-secret protocol identifier), because the remedy — the server has to
    /// advertise it, or the caller is talking to a server that does not support
    /// tasks at all — is not discoverable from "does not support tasks". No
    /// server state, task id or principal is rendered.
    fn unsupported_capability_message(&self, capability: &str, method: &str) -> String {
        let base = format!("Server does not support {capability} (required for {method})");
        if capability == "tasks" && self.is_v2() {
            return format!(
                "{base} — a 2026-07-28 server negotiates tasks through \
                 capabilities.extensions[\"{key}\"], and this server's server/discover \
                 projection carries no such entry",
                key = crate::types::capabilities::TASKS_EXTENSION_KEY,
            );
        }
        base
    }

    /// Send a TYPED request and wait for its response.
    async fn send_request(
        &self,
        request_id: RequestId,
        request: Request,
    ) -> Result<crate::types::JSONRPCResponse> {
        use crate::shared::protocol_helpers::create_request;

        // `create_request` CONSUMES its argument, so the typed value has to be
        // cloned to survive for the v1 branch — but the v2 branch of
        // `dispatch_request` never reads `typed`, and the clone is a full deep
        // copy of the request (arguments payload and all) held across the whole
        // network round trip. Branch first so v2 pays nothing; v1 is unchanged.
        if self.is_v2() {
            let jsonrpc_request = create_request(request_id.clone(), request);
            return self
                .dispatch_request(request_id, None, jsonrpc_request)
                .await;
        }
        let jsonrpc_request = create_request(request_id.clone(), request.clone());
        self.dispatch_request(request_id, Some(request), jsonrpc_request)
            .await
    }

    /// Send a request whose method has NO public [`ClientRequest`] variant.
    ///
    /// Today that is exactly `server/discover` (Phase-112 D-10 keeps it out of the
    /// exhaustive public enums, because adding a variant there is a MAJOR semver
    /// break). Only reachable on v2, where the raw transport frame is the normal
    /// path anyway.
    async fn send_untyped_request(
        &self,
        request_id: RequestId,
        method: &str,
        params: serde_json::Value,
    ) -> Result<crate::types::JSONRPCResponse> {
        let jsonrpc_request =
            crate::types::JSONRPCRequest::new(request_id.clone(), method, Some(params));
        self.dispatch_request(request_id, None, jsonrpc_request)
            .await
    }

    /// The ONE place a client request is put on the wire and its response awaited.
    ///
    /// `typed` carries the original [`Request`] for the v1 path, which sends the
    /// typed [`TransportMessage::Request`](crate::types::TransportMessage) exactly
    /// as it always has. On v2 the already-assembled JSON-RPC frame is stamped
    /// with the reserved `_meta` keys and sent RAW, which is what lets EVERY
    /// method — including the ones whose request struct has no `_meta` field —
    /// carry the era signal (Phase-113 D-113-D).
    async fn dispatch_request(
        &self,
        request_id: RequestId,
        typed: Option<Request>,
        mut jsonrpc_request: crate::types::JSONRPCRequest<serde_json::Value>,
    ) -> Result<crate::types::JSONRPCResponse> {
        // Register this request's ANSWER CHANNEL before it goes on the wire, so
        // whichever task ends up pumping can deliver the response to its owner
        // rather than to whoever happened to pop it.
        let (cancel_tx, _cancel_rx) = oneshot::channel();
        let (response_tx, mut response_rx) = oneshot::channel();
        // A LIVE registration must never be shadowed by debris from a previous
        // life of the same id: a client whose id counter wrapped, or a caller
        // that reused an explicit id, would otherwise have its real answer
        // absorbed as somebody else's leftovers. Taking the entry here costs one
        // scan of a bounded deque and removes the whole class.
        self.abandoned_requests.write().await.take(&request_id);
        self.active_requests.write().await.insert(
            request_id.clone(),
            Pending {
                cancel: cancel_tx,
                response: response_tx,
            },
        );

        // Create middleware context
        let context = MiddlewareContext::with_request_id(request_id.to_string());

        // Everything after the `active_requests` registration runs inside this
        // inner future so that EVERY error exit funnels through the single
        // cleanup point below (WR-04). On any `Err` — middleware, outbound
        // `send`, inbound `receive`, response middleware, or the host-dispatch
        // reply `send` — the pending entry (and its oneshot cancel sender) is
        // removed before the error propagates, so a `Client` that outlives a
        // failed request never leaks the id or collides with stale state on a
        // later reused id. The happy path still removes the entry inline when
        // the matching response arrives.
        let result = async {
            // Process request through middleware chain (read-only access)
            self.middleware_chain
                .read()
                .await
                .process_request_with_context(&mut jsonrpc_request, &context)
                .await?;

            if self.is_v2() {
                // v2: stamp the reserved `_meta` keys onto the assembled frame
                // and send it verbatim. The transport derives `Mcp-Method` /
                // `Mcp-Name` from these SAME bytes, so header and body cannot
                // desync (T-113-08).
                self.splice_v2_meta(&mut jsonrpc_request.params);
                let body = serde_json::to_vec(&jsonrpc_request)
                    .map_err(|e| Error::parse(format!("Failed to serialize v2 request: {e}")))?;
                // Through `send_frame`, so the transport guard is NOT held
                // across the round trip — see its rustdoc. Order is unchanged:
                // `_meta` is stamped and the frame serialized before it goes.
                self.send_frame(ClientFrame::Raw(body)).await?;
            } else {
                // v1: byte-identical to every prior release — the typed message
                // is re-serialized by the transport exactly as before.
                let request = typed.ok_or_else(|| {
                    Error::InvalidState("untyped requests require the 2026-07-28 era".to_string())
                })?;
                let message = crate::types::TransportMessage::Request {
                    id: request_id.clone(),
                    request,
                };
                // Through `send_frame`, for the same reason the v2 branch above
                // does: no transport guard across the round trip.
                self.send_frame(ClientFrame::Typed(message)).await?;
            }

            // Await THIS request's own answer, pumping the transport whenever
            // nobody else is.
            //
            // Every waiter queues for the transport lock; whoever gets it moves
            // exactly one sliced receive forward, routes the frame to its owner
            // and releases. So there is no pump to hand off and no pump to lose:
            // when the current pump returns with its own answer, the next waiter
            // in the queue simply becomes the pump.
            let mut budget = UnmatchedBudget::default();
            let mut last_unmatched: Option<String> = None;
            loop {
                // `now_or_never` rather than `try_recv`: the oneshot is
                // `tokio::sync` on native and `futures_channel` on wasm32 and
                // their `try_recv` signatures differ, but both `Receiver`s are
                // `Unpin` futures, so polling once is portable. Polling a
                // pending oneshot does not consume anything.
                //
                // Checked here AND raced inside the pump. This first poll is what
                // catches an answer another task's pump already delivered while
                // this one was queued for the transport lock; the pump's own arm
                // is what catches one delivered DURING a slice.
                let answered = match futures::FutureExt::now_or_never(&mut response_rx) {
                    Some(answer) => Some(answer),
                    None => {
                        // The answer is raced against the pump's RECEIVE, never
                        // against the pump as a whole: cancelling a pump between
                        // `receive()` returning and the frame being delivered
                        // would destroy that frame, which is the very defect this
                        // router exists to remove. Inside the `select` the
                        // receive arm is polled first, so a frame that is ready
                        // is always routed; only a PENDING receive is dropped,
                        // which `Transport::receive` documents implementors must
                        // tolerate.
                        match self.pump_once(&mut response_rx).await? {
                            PumpStep::Answered(answer) => Some(answer),
                            PumpStep::Unmatched(id) => {
                                budget.record();
                                last_unmatched = Some(id);
                                None
                            },
                            PumpStep::Progressed => None,
                        }
                    },
                };

                let Some(answer) = answered else {
                    // Checked on EVERY step that produced no answer, not only on
                    // the ones that booked an offence: a drip of wrong ids can go
                    // quiet after the last one, and a deadline that could only
                    // fire on the NEXT offence is a ceiling that exists and never
                    // fires. `exhausted` is `None` for an unarmed budget, so an
                    // honest but slow peer is still unbounded here.
                    if let Some(error) = budget.exhausted(&request_id, last_unmatched.as_deref()) {
                        return Err(error);
                    }
                    continue;
                };
                let mut response = answer.map_err(|_| {
                    // The registration was removed without an answer, which
                    // only `cancel_request` does. Stop waiting rather than
                    // blocking on a peer that will never reply.
                    Error::InvalidState(format!(
                        "request {request_id:?} was cancelled before it was answered"
                    ))
                })?;
                self.middleware_chain
                    .read()
                    .await
                    .process_response_with_context(&mut response, &context)
                    .await?;
                return Ok(response);
            }
        }
        .await;

        // Single WR-04 exit-cleanup invariant. On the happy path the pump has
        // already removed the entry to take ownership of the answer channel, so
        // this is a no-op; on every error path it is the one place the pending
        // id (and its cancel sender) is reaped, so a `Client` that outlives a
        // failed request never leaks the id or collides with a later reuse.
        //
        // And it is the one place the ABANDONMENT is recorded, for the same
        // reason it is the one place the removal happens: the `Option<Pending>`
        // this `remove` returns is exactly the distinction the ledger needs.
        // `Some` means the registration was still live, so no answer had been
        // delivered and the peer's real answer may still be on the wire behind
        // whatever killed this call — that is the debris. `None` means the pump
        // already took the registration to deliver the answer, so there is
        // nothing to absorb. Recording at each individual error site instead
        // would have to re-derive that distinction once per site and could not
        // see the happy path at all.
        if self
            .active_requests
            .write()
            .await
            .remove(&request_id)
            .is_some()
        {
            self.abandoned_requests.write().await.record(request_id);
        }
        result
    }

    /// Move the transport forward by at most one frame and route it to its owner.
    ///
    /// This is the whole of the per-id router. It takes the transport lock for a
    /// single SLICED receive and releases it before doing anything with what it
    /// got, so no other operation on this client is serialised behind either the
    /// peer's think-time or this task's routing work.
    ///
    /// A `Response` is delivered to whoever registered its id. A response nobody
    /// is awaiting is dropped with a `warn!` and REPORTED as
    /// [`PumpStep::Unmatched`] — harmless to the other waiters, because no caller
    /// is blocked on it, but the caller of this step has to know it happened:
    /// a peer that re-types an id answers nothing, forever, and
    /// [`UnmatchedBudget`] is what turns that into a named failure.
    ///
    /// # `answer` is raced INSIDE the select, not around this call
    ///
    /// The caller's own answer channel is polled in the same `select` as the
    /// receive, with the receive arm first. That is what removes a full
    /// [`PUMP_RECEIVE_SLICE`] of latency from the common two-caller case: with
    /// the answer polled only BETWEEN steps, a request whose response another
    /// task's pump had already delivered still sat through an entire 250 ms
    /// slice — holding the transport write lock, and so blocking that other task
    /// too — before it looked.
    ///
    /// Racing it here is safe where racing `pump_once` as a whole is not: a
    /// message the receive arm has already produced is always routed, because
    /// `futures::future::select` polls its first future first, and only a
    /// PENDING receive is ever dropped — which
    /// [`Transport::receive`](crate::shared::Transport::receive) documents
    /// implementors must tolerate without losing consumed bytes. Cancelling the
    /// whole step, by contrast, could drop a frame BETWEEN `receive()` returning
    /// it and this function routing it, which is the defect the router exists to
    /// remove.
    async fn pump_once<A>(&self, answer: &mut A) -> Result<PumpStep<A::Output>>
    where
        A: std::future::Future + Unpin,
    {
        let received = {
            let mut transport = self.transport.write().await;
            let receive = std::pin::pin!(transport.receive());
            let slice = std::pin::pin!(crate::runtime::sleep(PUMP_RECEIVE_SLICE));
            // Nested so the RECEIVE keeps first-poll priority: `select` polls its
            // first argument first, so a ready frame always wins over a ready
            // answer, and the answer wins over the slice.
            match futures::future::select(receive, futures::future::select(answer, slice)).await {
                futures::future::Either::Left((message, _)) => Some(message),
                futures::future::Either::Right((rest, _)) => match rest {
                    futures::future::Either::Left((answered, _)) => {
                        return Ok(PumpStep::Answered(answered))
                    },
                    futures::future::Either::Right(((), _)) => None,
                },
            }
        };

        // The slice expired with nothing delivered. Not an end of stream — the
        // point is that the lock has now been released, so anyone waiting to
        // send gets their turn before we ask again.
        let Some(message) = received else {
            return Ok(PumpStep::Progressed);
        };

        match message? {
            crate::types::TransportMessage::Response(response) => {
                let owner = self.active_requests.write().await.remove(&response.id);
                let Some(pending) = owner else {
                    // ORDER IS LOAD-BEARING. A LIVE owner is looked for FIRST,
                    // exactly as before, so nothing that is answered today stops
                    // being answered. Only when there is none does the ledger get
                    // asked, and it answers the question the live lookup cannot:
                    // is this our own debris, or the peer's misbehaviour?
                    if self.absorb_abandoned(&response.id).await {
                        return Ok(PumpStep::Progressed);
                    }
                    // Nobody here ever asked for this id. BOUNDED echo: the id is
                    // remote input of unbounded length. See `echoed_request_id`.
                    let id = echoed_request_id(&response.id);
                    tracing::warn!(%id, "dropping a JSON-RPC response no request is awaiting");
                    return Ok(PumpStep::Unmatched(id));
                };
                // The receiver is gone only if that caller has already stopped
                // waiting; dropping the answer is then correct.
                let _ = pending.response.send(response);
            },
            crate::types::TransportMessage::Notification(notification) => {
                use crate::shared::protocol_helpers::create_notification;
                let mut jsonrpc_notification = create_notification(notification.clone());
                let notif_context = MiddlewareContext::default();

                if let Err(e) = self
                    .middleware_chain
                    .write()
                    .await
                    .process_notification_with_context(&mut jsonrpc_notification, &notif_context)
                    .await
                {
                    // Log but do not terminate the pump - other requests are
                    // still waiting on frames behind this one.
                    tracing::warn!(
                        "Notification middleware processing failed for {}: {}",
                        jsonrpc_notification.method,
                        e
                    );
                }

                if let Some(tx) = &self.notification_tx {
                    #[allow(unused_mut)]
                    let mut tx_clone = tx.clone();
                    if let Err(e) = tx_clone.send(notification).await {
                        tracing::debug!("Notification channel closed: {}", e);
                    }
                }
            },
            crate::types::TransportMessage::Request { id, request } => {
                // Any inbound request at a client is server -> client by
                // definition. Answer it from the registered host handlers and
                // reply. The transport lock was released above, so this reply
                // does not have to wait for a receive to finish first.
                let response = self.dispatch_host_request(id, request).await;
                // The transport lock was released above, so this reply does not
                // have to wait for a receive to finish — and it goes through
                // `send_frame`, so it does not hold the lock across its own
                // round trip either. An in-tool elicitation answered against a
                // slow peer must not freeze the client that is answering it.
                self.send_frame(ClientFrame::Typed(
                    crate::types::TransportMessage::Response(response),
                ))
                .await?;
            },
        }
        Ok(PumpStep::Progressed)
    }

    /// Is this un-owned response frame OUR OWN debris, rather than the peer's
    /// misbehaviour?
    ///
    /// `true` when the id was recorded as abandoned — the answer to a request
    /// whose caller stopped waiting — in which case the entry is CONSUMED, the
    /// frame is ordinary progress and no budget moves. `false` when nothing here
    /// ever asked for this id, which is the case [`UnmatchedBudget`] exists for.
    ///
    /// Its own function rather than an inline branch in [`Client::pump_once`]
    /// purely to keep that function inside the repo's cognitive-complexity
    /// budget without an `#[allow]` — the same reason [`UnmatchedBudget`]'s own
    /// rustdoc gives for splitting the arming rule out of
    /// [`Client::dispatch_request`].
    ///
    /// The debug log is BOUNDED through [`echoed_request_id`] for the reason that
    /// function records: the id is remote input of unbounded length, and a
    /// hostile peer must not be able to push an unbounded string into a
    /// consumer's logs. Debug rather than the no-owner branch's `warn!` because
    /// this is not misbehaviour at all — it is the ordinary tail of a call that
    /// gave up.
    async fn absorb_abandoned(&self, id: &RequestId) -> bool {
        if !self.abandoned_requests.write().await.take(id) {
            return false;
        }
        // Built INSIDE the macro: `echoed_request_id` is a `format!`, and
        // `tracing` evaluates a field expression only when the callsite is
        // enabled. Hoisting it into a local would allocate on every absorbed
        // frame in every shipping configuration, where `debug` is off. The
        // no-owner branch's `warn!` is deliberately NOT written this way — there
        // the string is also the `PumpStep::Unmatched` return value, so it is
        // eager because it is USED, not merely logged.
        tracing::debug!(
            id = %echoed_request_id(id),
            "absorbing the late answer to a request this client already stopped waiting for"
        );
        true
    }

    /// Answer an inbound server -> client request from the host registry.
    ///
    /// Returns a [`JSONRPCResponse`](crate::types::JSONRPCResponse) that the
    /// caller sends back over the transport. A known request kind with no
    /// registered handler yields `-32601` (method-not-found); a
    /// handler/provider failure yields a sanitized `-32603` (the raw error is
    /// logged locally, never forwarded to the remote server). The connection is
    /// never dropped.
    async fn dispatch_host_request(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        use crate::client::host::{classify_host_request, HostRequestKind};
        match classify_host_request(&request) {
            HostRequestKind::Sampling => self.dispatch_host_sampling(id, request).await,
            HostRequestKind::Elicitation => self.dispatch_host_elicitation(id, request).await,
            HostRequestKind::Roots => self.dispatch_host_roots(id).await,
            // Spec MUST: answer inbound `ping` with an empty-object success
            // result so keepalive pings from servers/proxies do not fail (and
            // do not tear down the connection).
            HostRequestKind::Ping => {
                crate::types::JSONRPCResponse::success(id, serde_json::json!({}))
            },
            HostRequestKind::Unhandled => Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            ),
        }
    }

    /// Route a classified sampling request through the two-stage host approval
    /// model and the registered sampling handler.
    ///
    /// # Policy-denial taxonomy
    ///
    /// Sampling has two host-side access-control stages, both applied ONLY to
    /// the sampling path (never elicitation/roots):
    ///
    /// 1. **Preflight** ([`ClientBuilder::on_sampling_approval`]): an optional
    ///    gate (default-allow) that, when registered, runs BEFORE the handler. A
    ///    [`ApprovalDecision::Deny`] here prevents the LLM call entirely — no
    ///    tokens are billed — genuinely mitigating coerced / denial-of-wallet
    ///    sampling. When no preflight callback is registered, the handler runs
    ///    (default allow).
    /// 2. **Result review** ([`ClientBuilder::on_sampling_result_review`]): an
    ///    optional post-generation stage that sees the produced completion and
    ///    can deny after the fact. Its default (no callback) is pass-through.
    ///
    /// A denial from either stage returns a sanitized `-32603` response with the
    /// GENERIC message `"request denied by host policy"`. The callback's
    /// `Deny(reason)` is logged locally via `tracing::warn!` and is NEVER
    /// forwarded to the remote server (avoids leaking local host policy). The
    /// connection is kept alive — a denial is a normal JSON-RPC error response,
    /// not a transport failure.
    async fn dispatch_host_sampling(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        let Some(params) = Self::extract_sampling_params(request) else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        Self::host_response(id, SAMPLING_METHOD, self.answer_host_sampling(params).await)
    }

    /// The FULL host sampling pipeline: handler presence, the preflight
    /// approval gate, handler preference, and the result-review gate.
    ///
    /// ONE implementation with TWO entry points — the v1 server-initiated
    /// [`Self::answer_host_sampling`] and the v2 MRTR
    /// [`Self::answer_mrtr_sampling`]. Routing MRTR through here is what stops
    /// the v2 path from silently bypassing `on_sampling_approval` /
    /// `on_sampling_result_review` (T-113-57). The two entry points differ only
    /// in how they RENDER the completion, which is why this returns it typed.
    async fn run_host_sampling(
        &self,
        params: CreateMessageParams,
    ) -> std::result::Result<HostSamplingCompletion, HostRefusal> {
        // At least one sampling handler (legacy or WithTools) must be registered.
        if self.host_registry.sampling.is_none() && self.host_registry.sampling_with_tools.is_none()
        {
            return Err(HostRefusal::NoHandler);
        }

        // (1) PREFLIGHT approval gate — runs BEFORE any handler so a denial
        // prevents the LLM call entirely (no tokens billed). It operates on the
        // request params, so it is IDENTICAL for the legacy and WithTools paths
        // (the wallet gate is never weakened by the WithTools surface).
        if let Some(approval) = &self.host_registry.approval {
            if let ApprovalDecision::Deny(reason) = approval(params.clone()).await {
                tracing::warn!(%reason, "sampling denied by host preflight");
                return Err(HostRefusal::Denied);
            }
        }

        // Prefer the tool-aware handler; otherwise the legacy single-content one.
        // A client that registered only a legacy handler keeps its EXACT current
        // wire behavior (serializes a `CreateMessageResult`).
        if self.host_registry.sampling_with_tools.is_some() {
            self.answer_sampling_with_tools(params)
                .await
                .map(HostSamplingCompletion::WithTools)
        } else {
            self.answer_sampling_legacy(params)
                .await
                .map(HostSamplingCompletion::Legacy)
        }
    }

    /// The v1 host rendering of a sampling completion: the tool-aware result is
    /// serialized in FULL, exactly as before this pipeline was shared.
    async fn answer_host_sampling(
        &self,
        params: CreateMessageParams,
    ) -> std::result::Result<serde_json::Value, HostRefusal> {
        match self.run_host_sampling(params).await? {
            HostSamplingCompletion::Legacy(result) => Self::host_value(&result),
            HostSamplingCompletion::WithTools(result) => Self::host_value(&result),
        }
    }

    /// The MRTR rendering of a sampling completion.
    ///
    /// An `inputResponses` value for a `sampling/createMessage` entry is
    /// SPEC-TYPED as a `CreateMessageResult`, so a tool-aware completion is
    /// projected down through the SAME projection the result-review gate uses.
    /// Without this a `WithTools`-only client would advertise the `sampling`
    /// capability (it can service the request) and then fail to produce a
    /// decodable answer — an under-supply the server would re-request forever.
    async fn answer_mrtr_sampling(
        &self,
        params: CreateMessageParams,
    ) -> std::result::Result<serde_json::Value, HostRefusal> {
        match self.run_host_sampling(params).await? {
            HostSamplingCompletion::Legacy(result) => Self::host_value(&result),
            HostSamplingCompletion::WithTools(result) => {
                Self::host_value(&Self::project_with_tools_to_legacy(&result))
            },
        }
    }

    /// Legacy single-content sampling answer path (unchanged behavior).
    async fn answer_sampling_legacy(
        &self,
        params: CreateMessageParams,
    ) -> std::result::Result<CreateMessageResult, HostRefusal> {
        let handler = self
            .host_registry
            .sampling
            .as_ref()
            .expect("caller checked a legacy handler is present");

        // Capture an owned clone of the params for result review ONLY when a
        // review callback is registered; otherwise the handler consumes
        // `params` below with zero extra clones.
        let review_params = self
            .host_registry
            .result_review
            .is_some()
            .then(|| params.clone());

        let result = handler
            .handle_create_message(params)
            .await
            .map_err(HostRefusal::Failed)?;

        // Optional post-generation review (default pass-through). `review_params`
        // is `Some` exactly when `result_review` is `Some`, so the pair matches.
        if let (Some(review), Some(params)) = (&self.host_registry.result_review, review_params) {
            if let ApprovalDecision::Deny(reason) = review(params, result.clone()).await {
                tracing::warn!(%reason, "sampling denied by host result review");
                return Err(HostRefusal::Denied);
            }
        }

        Ok(result)
    }

    /// Tool-aware (`WithTools`) sampling answer path.
    ///
    /// The preflight gate has already run in `dispatch_host_sampling`. The
    /// optional result-review gate is NOT weakened: when a reviewer is
    /// registered, the `WithTools` completion is projected to a single-content
    /// [`CreateMessageResult`] (tool blocks rendered as a text summary) so the
    /// reviewer still sees the completion and can deny it. The returned wire
    /// value remains the full `CreateMessageResultWithTools`.
    async fn answer_sampling_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> std::result::Result<crate::types::sampling::CreateMessageResultWithTools, HostRefusal>
    {
        let handler = self
            .host_registry
            .sampling_with_tools
            .as_ref()
            .expect("caller checked a WithTools handler is present");

        let review_params = self
            .host_registry
            .result_review
            .is_some()
            .then(|| params.clone());

        let result = handler
            .handle_create_message_with_tools(params)
            .await
            .map_err(HostRefusal::Failed)?;

        if let (Some(review), Some(params)) = (&self.host_registry.result_review, review_params) {
            let projected = Self::project_with_tools_to_legacy(&result);
            if let ApprovalDecision::Deny(reason) = review(params, projected).await {
                tracing::warn!(%reason, "sampling denied by host result review");
                return Err(HostRefusal::Denied);
            }
        }

        Ok(result)
    }

    /// Project a [`CreateMessageResultWithTools`] into a single-content
    /// [`CreateMessageResult`]. Tool blocks have no single-`Content`
    /// counterpart, so they are rendered as a short text marker.
    ///
    /// TWO consumers: the optional result-review gate (which must still see the
    /// completion so it can deny it), and the MRTR fold (whose
    /// `inputResponses` value is spec-typed as a `CreateMessageResult`).
    fn project_with_tools_to_legacy(
        result: &crate::types::sampling::CreateMessageResultWithTools,
    ) -> crate::types::sampling::CreateMessageResult {
        use crate::types::sampling::SamplingMessageContent as Smc;
        let text = result
            .content
            .iter()
            .map(|c| match c {
                Smc::Text { text, .. } => text.clone(),
                Smc::Image { .. } => "[image]".to_string(),
                Smc::Audio { .. } => "[audio]".to_string(),
                Smc::ToolUse { name, id, .. } => format!("[tool_use {name} {id}]"),
                Smc::ToolResult { tool_use_id, .. } => format!("[tool_result {tool_use_id}]"),
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut cmr = crate::types::sampling::CreateMessageResult::new(
            crate::types::Content::text(text),
            &result.model,
        );
        if let Some(reason) = &result.stop_reason {
            cmr = cmr.with_stop_reason(reason.clone());
        }
        cmr
    }

    /// Route a classified elicitation request to the registered host handler.
    async fn dispatch_host_elicitation(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        // Extract the single elicitation parse variant inline (server-side
        // `elicitation/create`); anything else is not routable here.
        let Request::Server(server) = request else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        let crate::types::ServerRequest::ElicitationCreate(params) = *server else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        Self::host_response(
            id,
            ELICITATION_METHOD,
            self.answer_host_elicitation(*params).await,
        )
    }

    /// The host elicitation pipeline. ONE implementation, two entry points —
    /// the v1 server-initiated dispatch and the v2 MRTR fold (D-06: app authors
    /// write ONE elicitation callback that serves both eras).
    async fn answer_host_elicitation(
        &self,
        params: crate::types::elicitation::ElicitRequestParams,
    ) -> std::result::Result<serde_json::Value, HostRefusal> {
        let Some(handler) = &self.host_registry.elicitation else {
            return Err(HostRefusal::NoHandler);
        };
        let result = handler
            .handle_elicitation(params)
            .await
            .map_err(HostRefusal::Failed)?;
        Self::host_value(&result)
    }

    /// Answer a classified `roots/list` request from the registered provider.
    async fn dispatch_host_roots(&self, id: RequestId) -> crate::types::JSONRPCResponse {
        Self::host_response(id, ROOTS_METHOD, self.answer_host_roots().await)
    }

    /// The host roots pipeline. ONE implementation, two entry points.
    async fn answer_host_roots(&self) -> std::result::Result<serde_json::Value, HostRefusal> {
        let Some(provider) = &self.host_registry.roots else {
            return Err(HostRefusal::NoHandler);
        };
        let result = provider().await.map_err(HostRefusal::Failed)?;
        Self::host_value(&result)
    }

    // =======================================================================
    // MRTR `inputRequests` fold (Phase 113, CLNT-02).
    // =======================================================================

    /// Answer an entire `inputRequests` map from the registered host handlers.
    ///
    /// ALL-OR-NOTHING (T-113-26): either every entry is answered, or the result
    /// is [`FoldOutcome::CannotFulfil`] and the caller does NOT resend. A
    /// partially-filled map and a fabricated response are both forbidden — the
    /// former would let a server harvest partial answers, the latter would
    /// synthesize consent for a capability the client never registered.
    ///
    /// Every refusal path emits a `tracing::warn!` naming the entry key, so a
    /// handler failure is observable rather than swallowed into "the caller got
    /// the original result back".
    async fn fold_input_requests(
        &self,
        requests: &crate::types::mrtr::InputRequests,
    ) -> FoldOutcome {
        use crate::types::mrtr::{InputRequest, InputResponse};

        // PREFLIGHT FIRST: prove every kind is fulfillable BEFORE invoking
        // anything. Otherwise a map whose second entry has no handler would
        // first prompt a human (or spend an agent's tokens) on the fulfillable
        // first entry, and then discard that work.
        if let Err(kind) = self.host_registry.preflight_input_requests(requests) {
            tracing::warn!(
                ?kind,
                "MRTR: no registered handler for a requested input kind — not resending"
            );
            return FoldOutcome::CannotFulfil;
        }

        let mut responses = crate::types::mrtr::InputResponses::new();
        for (key, request) in requests {
            let kind = request.kind();
            // Routed through the SAME helpers the v1 host dispatch uses, so the
            // approval and result-review hooks apply identically (T-113-57).
            let answered = match request {
                InputRequest::Elicitation(params) => {
                    self.answer_host_elicitation((**params).clone()).await
                },
                InputRequest::Sampling(params) => {
                    self.answer_mrtr_sampling((**params).clone()).await
                },
                InputRequest::ListRoots => self.answer_host_roots().await,
            };
            let value = match answered {
                Ok(value) => value,
                Err(refusal) => {
                    tracing::warn!(
                        key = %key,
                        reason = refusal.reason(),
                        "MRTR: could not fulfil an inputRequests entry — not resending"
                    );
                    return FoldOutcome::CannotFulfil;
                },
            };
            // KIND-DIRECTED decode: the three response shapes overlap on the
            // wire, so decoding by the ORIGINATING kind is what stops a
            // misclassification (T-113-46).
            let response = match InputResponse::decode_for(kind, value) {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!(
                        key = %key,
                        %error,
                        "MRTR: a host handler produced a response that does not match the \
                         requested kind — not resending"
                    );
                    return FoldOutcome::CannotFulfil;
                },
            };
            // A declined/cancelled elicitation is a legitimate v1 answer but is
            // NOT a fulfilled MRTR input: the user said no, so the client must
            // not resend on their behalf (D-06).
            if let InputResponse::Elicitation(result) = &response {
                if result.action != crate::types::elicitation::ElicitAction::Accept {
                    tracing::warn!(
                        key = %key,
                        action = ?result.action,
                        "MRTR: elicitation was not accepted — not resending"
                    );
                    return FoldOutcome::CannotFulfil;
                }
            }
            // The server-assigned key is preserved VERBATIM: it is how the
            // server correlates the answer with its own continuation state.
            responses.insert(key.clone(), response);
        }
        FoldOutcome::Fulfilled(responses)
    }

    // =======================================================================
    // The bounded MRTR gather→resend loop (Phase 113, CLNT-02).
    // =======================================================================

    /// Drive one MRTR round: read the result, and decide what happens next.
    ///
    /// Extracted from [`Self::send_with_mrtr`] so the loop body stays small.
    async fn mrtr_round_step(&self, result: serde_json::Value) -> RoundOutcome {
        use crate::types::mrtr::{
            InputRequiredResult, MrtrRequestParams, INPUT_REQUIRED_RESULT_TYPE,
        };

        // Anything that is not `input_required` is TERMINAL — including a
        // `resultType` this build has never heard of (e.g. Phase 114's
        // `"task"`), and including a result with no `resultType` at all. That
        // is what lets later result types compose without touching this loop.
        if result
            .get(crate::types::mrtr::RESULT_TYPE_KEY)
            .and_then(serde_json::Value::as_str)
            != Some(INPUT_REQUIRED_RESULT_TYPE)
        {
            return RoundOutcome::Terminal(result);
        }

        let parsed = match <InputRequiredResult as serde::Deserialize>::deserialize(&result) {
            Ok(parsed) => parsed,
            Err(error) => {
                // A malformed `input_required` is not something a retry can
                // fix — hand the raw result back rather than resending blind.
                //
                // It must NOT go back as `Terminal`: the caller would then
                // deserialize it into the concrete result type, and
                // `CallToolResult::content` is `#[serde(default)]`, so a result
                // the server explicitly marked `input_required` would arrive as
                // a silently EMPTY success — the exact failure this whole type
                // exists to prevent. Carry the VERBATIM object instead, so the
                // caller receives `Error::input_required_unfulfilled` and can
                // read `raw`.
                tracing::warn!(%error, "MRTR: could not parse an input_required result");
                return RoundOutcome::Unfulfilled(Box::new(InputRequiredResult {
                    result_type: INPUT_REQUIRED_RESULT_TYPE.to_string(),
                    input_requests: None,
                    request_state: None,
                    meta: None,
                    raw: result,
                }));
            },
        };

        // `requestState` is ECHOED VERBATIM. The spec forbids the client
        // inspecting, parsing or modifying it (it is the server's sealed,
        // principal-bound continuation), so it is only ever moved — never read.
        let request_state = parsed.request_state.clone();

        // Borrowed, not cloned: `fold_input_requests` takes `&InputRequests` and
        // `parsed` keeps its own copy. Cloning here deep-copied every elicitation
        // schema and every `CreateMessageParams` once per round, purely to dodge
        // a borrow/move conflict with the `Unfulfilled(Box::new(parsed))` arm —
        // which binding the fold result to a local resolves under NLL.
        let Some(requests) = parsed.input_requests.as_ref() else {
            // Server-side load shedding: `requestState` only, no questions.
            // The client MAY retry immediately, and no handler is invoked.
            //
            // Only when a token is actually present. The spec requires an
            // `input_required` result to carry at least one of `inputRequests`
            // or `requestState`; a result with NEITHER gives the retry nothing
            // new to send, so resending the byte-identical request would just
            // burn the whole round budget on identical round trips before
            // reporting a misleading "round limit exceeded".
            if request_state.is_none() {
                tracing::warn!(
                    "MRTR: an input_required result carried neither inputRequests nor \
                     requestState — nothing to resend with"
                );
                return RoundOutcome::Unfulfilled(Box::new(parsed));
            }
            return RoundOutcome::Continue(MrtrRequestParams {
                input_responses: None,
                // EGRESS: `splice_mrtr_params` serializes the TYPED map, so the
                // raw retention has no meaning on the client's write path. It
                // exists only for the server's kind-directed re-decode at
                // ingress (D-113-O).
                input_responses_raw: None,
                request_state,
            });
        };

        match self.fold_input_requests(requests).await {
            FoldOutcome::Fulfilled(responses) => RoundOutcome::Continue(MrtrRequestParams {
                input_responses: Some(responses),
                // EGRESS — see the sibling arm above.
                input_responses_raw: None,
                request_state,
            }),
            // D-06: no handler, or a decline/error — do NOT resend, and do NOT
            // fabricate. The caller receives the result.
            FoldOutcome::CannotFulfil => RoundOutcome::Unfulfilled(Box::new(parsed)),
        }
    }

    /// Send `method` with `params`, auto-orchestrating MRTR until the server
    /// completes the operation, the client cannot fulfil, or the bound trips.
    ///
    /// Only reachable on v2: it sends RAW frames through
    /// [`Self::send_untyped_request`], which is the only path that can carry
    /// `params.inputResponses` / `params.requestState` (the typed request
    /// structs deliberately have no such fields — D-113-D).
    async fn send_with_mrtr(
        &self,
        method: &str,
        mut params: serde_json::Value,
    ) -> Result<MrtrLoopOutcome> {
        // A zero limit would send nothing at all and report a round-limit
        // breach for a request that never left, which is a confusing lie.
        let limit = self.mrtr_round_limit.max(1);
        for _round in 0..limit {
            // A FRESH id every iteration. Spec MUST: "the JSON-RPC id MUST be
            // different between the initial request and the retry" — they are
            // independent requests, and reusing one re-creates the id-replay
            // bug class HTTP-05 exists to close (T-113-07).
            let request_id = RequestId::String(Uuid::new_v4().to_string());
            let response = self
                .send_untyped_request(request_id, method, params.clone())
                .await?;
            let result = match response.payload {
                crate::types::jsonrpc::ResponsePayload::Result(result) => result,
                crate::types::jsonrpc::ResponsePayload::Error(error) => {
                    return Err(Error::from_jsonrpc_error(error))
                },
            };
            match self.mrtr_round_step(result).await {
                RoundOutcome::Terminal(raw_result) => {
                    return Ok(MrtrLoopOutcome::Complete(raw_result))
                },
                RoundOutcome::Unfulfilled(parsed) => {
                    return Ok(MrtrLoopOutcome::Unfulfilled(parsed))
                },
                RoundOutcome::Continue(mrtr) => {
                    // `splice_mrtr_params` REMOVES both keys before inserting,
                    // so no earlier round's `inputResponses` / `requestState`
                    // can survive into this one (T-113-28). Everything else on
                    // `params` — including the caller's `_meta` trace context —
                    // is untouched, so spans stay linked across rounds.
                    crate::types::mrtr::splice_mrtr_params(&mut params, &mrtr);
                },
            }
        }
        // The bound tripped. No handler ran for this round (the loop exits
        // BEFORE sending), and the error is programmatically distinguishable.
        Err(Error::mrtr_round_limit_exceeded(limit))
    }

    /// Deserialize a completed MRTR result, or convert an unfulfilled one into
    /// the typed client-local error the EXISTING methods return.
    fn mrtr_result_or_error<R: serde::de::DeserializeOwned>(outcome: MrtrLoopOutcome) -> Result<R> {
        match outcome {
            MrtrLoopOutcome::Unfulfilled(unfulfilled) => {
                Err(Error::input_required_unfulfilled(*unfulfilled))
            },
            MrtrLoopOutcome::Complete(raw) => {
                serde_json::from_value(raw).map_err(|e| Error::parse(e.to_string()))
            },
        }
    }

    /// Map a loop outcome onto the additive [`MrtrOutcome`] return type.
    fn mrtr_outcome<R: serde::de::DeserializeOwned>(
        outcome: MrtrLoopOutcome,
    ) -> Result<crate::types::mrtr::MrtrOutcome<R>> {
        match outcome {
            MrtrLoopOutcome::Unfulfilled(unfulfilled) => {
                Ok(crate::types::mrtr::MrtrOutcome::InputRequired(*unfulfilled))
            },
            MrtrLoopOutcome::Complete(raw) => serde_json::from_value(raw)
                .map(crate::types::mrtr::MrtrOutcome::Complete)
                .map_err(|e| Error::parse(e.to_string())),
        }
    }

    /// The `tools/call` params object, byte-identical to what the typed path
    /// would have serialized.
    fn call_tool_params(name: String, arguments: serde_json::Value) -> Result<serde_json::Value> {
        serde_json::to_value(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: None,
        })
        .map_err(|e| Error::parse(e.to_string()))
    }

    /// The `prompts/get` params object.
    fn get_prompt_params(
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<serde_json::Value> {
        serde_json::to_value(GetPromptRequest {
            name,
            arguments,
            _meta: None,
        })
        .map_err(|e| Error::parse(e.to_string()))
    }

    /// The `resources/read` params object.
    fn read_resource_params(uri: String) -> Result<serde_json::Value> {
        serde_json::to_value(ReadResourceRequest { uri, _meta: None })
            .map_err(|e| Error::parse(e.to_string()))
    }

    /// Call a tool, auto-orchestrating MRTR, and observe an unfulfilled
    /// `input_required` result instead of losing it (Phase 113, CLNT-02).
    ///
    /// The additive sibling of [`Self::call_tool`]. Use it whenever a
    /// `MrtrOutcome::InputRequired` is a normal outcome for your application
    /// rather than an error — for example when your client wants to surface the
    /// server's `inputRequests` in its own UI instead of registering a
    /// [`ClientBuilder::on_elicitation`] handler.
    ///
    /// On a v1 connection there is no MRTR, so this simply delegates to
    /// [`Self::call_tool`] and always returns `MrtrOutcome::Complete`.
    ///
    /// # Errors
    ///
    /// As [`Self::call_tool`], plus [`Error::mrtr_round_limit_exceeded`] when
    /// the server keeps asking for input past
    /// [`ClientBuilder::mrtr_round_limit`].
    pub async fn call_tool_mrtr(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<crate::types::mrtr::MrtrOutcome<CallToolResult>> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;
        if !self.is_v2() {
            return self
                .call_tool(name, arguments)
                .await
                .map(crate::types::mrtr::MrtrOutcome::Complete);
        }
        let params = Self::call_tool_params(name, arguments)?;
        Self::mrtr_outcome(self.send_with_mrtr(CALL_TOOL_METHOD, params).await?)
    }

    /// Get a prompt, auto-orchestrating MRTR. See [`Self::call_tool_mrtr`].
    ///
    /// # Errors
    ///
    /// As [`Self::get_prompt`], plus [`Error::mrtr_round_limit_exceeded`].
    pub async fn get_prompt_mrtr(
        &self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<crate::types::mrtr::MrtrOutcome<GetPromptResult>> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/get")?;
        if !self.is_v2() {
            return self
                .get_prompt(name, arguments)
                .await
                .map(crate::types::mrtr::MrtrOutcome::Complete);
        }
        let params = Self::get_prompt_params(name, arguments)?;
        Self::mrtr_outcome(self.send_with_mrtr(GET_PROMPT_METHOD, params).await?)
    }

    /// Read a resource, auto-orchestrating MRTR. See [`Self::call_tool_mrtr`].
    ///
    /// This one matters even more than the others: `ReadResourceResult.contents`
    /// has no serde default, so an `input_required` result cannot be
    /// deserialized into it at all — without this method (or
    /// [`Error::input_required_unfulfilled`]) the outcome would surface as an
    /// opaque parse error.
    ///
    /// # Errors
    ///
    /// As [`Self::read_resource`], plus [`Error::mrtr_round_limit_exceeded`].
    pub async fn read_resource_mrtr(
        &self,
        uri: String,
    ) -> Result<crate::types::mrtr::MrtrOutcome<ReadResourceResult>> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/read")?;
        if !self.is_v2() {
            return self
                .read_resource(uri)
                .await
                .map(crate::types::mrtr::MrtrOutcome::Complete);
        }
        let params = Self::read_resource_params(uri)?;
        Self::mrtr_outcome(self.send_with_mrtr(READ_RESOURCE_METHOD, params).await?)
    }

    /// Extract [`CreateMessageParams`] from either inbound sampling parse
    /// variant (client-alias or server), handling the parse ambiguity.
    fn extract_sampling_params(request: Request) -> Option<CreateMessageParams> {
        match request {
            Request::Client(client) => match *client {
                ClientRequest::CreateMessage(params) => Some(*params),
                _ => None,
            },
            Request::Server(server) => match *server {
                crate::types::ServerRequest::CreateMessage(params) => Some(*params),
                _ => None,
            },
        }
    }

    /// Serialize a handler result into the wire value both entry points use.
    fn host_value<S: serde::Serialize>(
        value: &S,
    ) -> std::result::Result<serde_json::Value, HostRefusal> {
        serde_json::to_value(value).map_err(|e| {
            tracing::error!("failed to serialize host response: {e}");
            HostRefusal::Serialization
        })
    }

    /// Turn a shared host-pipeline outcome into the v1 JSON-RPC response.
    ///
    /// The wire mapping is unchanged from before the pipeline was shared:
    /// no handler => `-32601`, a policy denial or a serialization failure =>
    /// a sanitized `-32603`, a handler failure => a sanitized `-32603` with the
    /// raw error logged locally.
    fn host_response(
        id: RequestId,
        method: &str,
        outcome: std::result::Result<serde_json::Value, HostRefusal>,
    ) -> crate::types::JSONRPCResponse {
        match outcome {
            Ok(value) => crate::types::JSONRPCResponse::success(id, value),
            Err(HostRefusal::NoHandler) => Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            ),
            Err(HostRefusal::Denied) => Self::host_error(
                id,
                crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
                "request denied by host policy",
            ),
            Err(HostRefusal::Failed(error)) => Self::host_handler_error(id, method, &error),
            Err(HostRefusal::Serialization) => Self::host_error(
                id,
                crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
                "Internal error handling host request",
            ),
        }
    }

    /// Build a JSON-RPC error response that keeps the connection alive.
    ///
    /// All host error responses are sanitized: only the generic `message`
    /// passed here crosses the wire. Raw handler errors and policy-denial
    /// reasons are logged locally by the caller (never forwarded to the remote
    /// server), so local host policy is not leaked. Callers pass the
    /// appropriate [`ErrorCode`](crate::error::ErrorCode) constant:
    /// - `METHOD_NOT_FOUND` for a known request kind with no registered handler,
    /// - `INTERNAL_ERROR` for a sanitized policy denial (a preflight or
    ///   result-review callback returning [`ApprovalDecision::Deny`]) or a
    ///   handler/provider/serialization failure.
    fn host_error(id: RequestId, code: i32, message: &str) -> crate::types::JSONRPCResponse {
        crate::types::JSONRPCResponse::error(
            id,
            crate::types::jsonrpc::JSONRPCError::new(code, message),
        )
    }

    /// Log a handler/provider failure locally and return a sanitized
    /// `INTERNAL_ERROR`.
    fn host_handler_error(
        id: RequestId,
        method: &str,
        err: &Error,
    ) -> crate::types::JSONRPCResponse {
        tracing::error!("host handler for {method} failed: {err}");
        Self::host_error(
            id,
            crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
            "Internal error handling host request",
        )
    }

    /// Send a notification.
    async fn send_notification(&self, notification: Notification) -> Result<()> {
        let message = crate::types::TransportMessage::Notification(notification);
        // Off the guard, like every other client-side send. `cancel_request` and
        // every notification path reach the wire through here.
        self.send_frame(ClientFrame::Typed(message)).await
    }

    /// Put ONE outbound frame on the wire WITHOUT holding the transport guard
    /// across the round trip (Phase 118.2, plan 23).
    ///
    /// # The invariant this whole plan rests on
    ///
    /// No transport guard on the SEND path in this file is held across an HTTP
    /// round trip. (The qualifier is load-bearing: [`Self::open_event_stream`]
    /// still holds a READ guard across the `subscriptions/listen` response
    /// head, recorded as a KNOWN RESIDUAL at that call site. Stating this
    /// invariant unqualified would tell the next reader of `send_frame` that
    /// the whole file is covered, which it is not.) The guard is taken for
    /// exactly as long as it takes to ASK the transport for a
    /// [`SharedSender`](crate::shared::SharedSender), dropped explicitly, and
    /// only then is the send awaited. Without that, a peer that accepts a POST
    /// and never writes its
    /// response HEAD holds the single transport lock forever and every other
    /// operation on this client — a second call, a notification, a
    /// cancellation, [`Transport::close`] — blocks at acquisition with nothing
    /// to bound it (T-118.2-23-01). Fenced by
    /// `a_peer_that_never_writes_response_headers_does_not_serialise_the_client`
    /// in `tests/client_sse_stream.rs`.
    ///
    /// A READ guard would NOT do: tokio's `RwLock` is fair, so a pending writer
    /// parks every later reader behind it, and a read guard held across a round
    /// trip wedges [`Self::pump_once`] and `close` exactly as a write guard
    /// does. The read guard here is momentary and contains no `await` of its
    /// own.
    ///
    /// # The fallback is today's path, unchanged
    ///
    /// A transport that answers `None` — every transport that has not opted in,
    /// including every external one — is sent through the exclusive `&mut`
    /// path exactly as before, byte for byte.
    async fn send_frame(&self, frame: ClientFrame) -> Result<()> {
        // Resolved at construction, so the opt-in path takes NO transport lock
        // here at all — see the field's docs for why asking per frame was both
        // pointless and a regression for `None`-answering transports.
        if let Some(handle) = self.shared_sender.as_ref() {
            return match frame {
                ClientFrame::Typed(message) => handle.send_shared(message).await,
                ClientFrame::Raw(body) => handle.send_raw_shared(body).await,
            };
        }

        let mut transport = self.transport.write().await;
        match frame {
            ClientFrame::Typed(message) => transport.send(message).await,
            ClientFrame::Raw(body) => transport.send_raw(body).await,
        }
    }
}

/// One outbound frame, in whichever of the two shapes the era produced it
/// (Phase 118.2, plan 23).
///
/// Exists so [`Client::send_frame`] is ONE function rather than four copies of
/// the same lock discipline — the shape in which three of the four sites were
/// left unfixed when the receive-path twin of this defect was closed.
enum ClientFrame {
    /// A typed message, re-serialized by the transport: the v1 path, and every
    /// notification and inbound-request reply on both eras.
    Typed(crate::types::TransportMessage),
    /// An already-encoded JSON-RPC frame, sent verbatim: the v2 request path,
    /// whose `params._meta` was stamped onto these very bytes.
    Raw(Vec<u8>),
}

// ===========================================================================
// `subscriptions/listen` — the v2 change-notification stream (HTTP-04).
// ===========================================================================

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
impl<T> Client<T>
where
    T: Transport + crate::client::subscriptions::EventStreamTransport,
{
    /// Open a v2 `subscriptions/listen` stream and receive change notifications
    /// (HTTP-04).
    ///
    /// The 2026-07-28 schema REMOVED `resources/subscribe` and
    /// `resources/unsubscribe` and replaced both with this single long-lived
    /// stream. The returned [`SubscriptionStream`](crate::client::subscriptions::SubscriptionStream)
    /// has already consumed the server's mandatory acknowledgement — read the
    /// AGREED filter from
    /// [`acknowledged()`](crate::client::subscriptions::SubscriptionStream::acknowledged)
    /// before polling — and then yields one item per delivered notification.
    ///
    /// Dropping the returned stream closes the underlying HTTP response, which
    /// is what releases the server's registry slot; there is no `close()` to
    /// forget.
    ///
    /// # Every call mints a FRESH subscription id
    ///
    /// The subscription id IS the JSON-RPC request id of this call, and this
    /// method mints a fresh `Uuid::new_v4()` for it every time. It is never
    /// derived from the transport, from a counter, or from a previous stream.
    ///
    /// That is a CONTRACT, not an implementation detail, and it is what makes a
    /// pmcp client structurally immune to the reconnect collision: the server
    /// refuses a second LIVE registration under a `(principal, subscriptionId)`
    /// pair it already holds, and it CANNOT tell an ungracefully disconnected
    /// peer from a live one (the receiver and the registry guard live in one
    /// stream-state tuple, so the entry survives until Hyper drops the response
    /// body — at which moment RAII reclaims it anyway). A client that reused its
    /// id when reconnecting would therefore be refused for the remainder of the
    /// server's keep-alive window. Because every call here mints a fresh id, a
    /// reconnect after ANY disconnect — graceful or not — can never collide with
    /// the incumbent the server may still consider live.
    ///
    /// The guard against a future refactor making ids sticky is the live
    /// tripwire `successive_listen_calls_mint_distinct_subscription_ids` in
    /// `tests/v2_subscriptions_client.rs`, which opens two streams from ONE
    /// client and asserts their acknowledged ids DIFFER.
    ///
    /// A third-party client that does reuse an id is refused with the RETRYABLE
    /// `RATE_LIMITED` (`-32005`, delivered at HTTP 200), so backing off and
    /// retrying is the correct response — but minting a fresh id, as this method
    /// does, is strictly better.
    ///
    /// # D-11: polling remains the RECOMMENDED enterprise mechanism
    ///
    /// Polling over the Tasks mechanism stays pmcp's recommended mechanism for
    /// enterprise remote deployments. This stream is the spec-conformant OPT-IN:
    /// its server side is documented single-instance / sticky-routed only,
    /// because the server's subscription registry is instance-local. Behind a
    /// non-sticky load balancer a subscriber silently under-receives.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use futures::StreamExt;
    /// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
    /// use pmcp::shared::StreamableHttpTransport;
    /// use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
    /// use pmcp::types::subscriptions::SubscriptionFilter;
    /// use pmcp::ClientBuilder;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let url = url::Url::parse("https://example.invalid/mcp").unwrap();
    /// let transport =
    ///     StreamableHttpTransport::new(StreamableHttpTransportConfigBuilder::new(url).build());
    /// let client = ClientBuilder::new(transport)
    ///     .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
    ///     .build();
    ///
    /// let filter = SubscriptionFilter {
    ///     tools_list_changed: Some(true),
    ///     ..SubscriptionFilter::default()
    /// };
    /// let mut stream = client.subscriptions_listen(filter).await?;
    /// println!("agreed: {:?}", stream.acknowledged().notifications);
    ///
    /// while let Some(notification) = stream.next().await {
    ///     println!("{:?}", notification?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// - the connection did not opt into `2026-07-28` — NO request is sent, and
    ///   the message names `ClientBuilder::with_protocol_version`;
    /// - the server rejected the request, in which case its own JSON-RPC error
    ///   is returned UNCHANGED (a server advertising no subscription-delivered
    ///   capability answers `-32601`, which is how "this server does not do
    ///   subscriptions" is distinguished from a transport fault);
    /// - the first frame on the stream is not the mandatory acknowledgement, or
    ///   is tagged with a different `subscriptionId`.
    pub async fn subscriptions_listen(
        &self,
        notifications: crate::types::subscriptions::SubscriptionFilter,
    ) -> Result<crate::client::subscriptions::SubscriptionStream> {
        use crate::types::subscriptions::{SubscriptionsListenParams, SUBSCRIPTIONS_LISTEN_METHOD};

        // Fail fast and LOCALLY: `subscriptions/listen` does not exist on v1, so
        // a request from a v1 client cannot succeed and must not be sent.
        self.require_v2(SUBSCRIPTIONS_LISTEN_METHOD)?;

        // A FRESH id per call, never a sticky or derived one — see this method's
        // docs. Making this constant, or reusing a previous stream's id, breaks
        // the reconnect contract and fails
        // `successive_listen_calls_mint_distinct_subscription_ids`.
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let params = serde_json::to_value(SubscriptionsListenParams::new(notifications))
            .map_err(|e| Error::parse(format!("Failed to serialize listen params: {e}")))?;
        let mut jsonrpc_request = crate::types::JSONRPCRequest::new(
            request_id.clone(),
            SUBSCRIPTIONS_LISTEN_METHOD,
            Some(params),
        );
        // The SAME reserved `_meta` every other v2 request carries: the transport
        // derives `Mcp-Method` / `Mcp-Name` from these very bytes, so the header
        // and the body cannot desync (T-113-08), and `Mcp-Name` comes out EMPTY
        // because `subscriptions/listen` is not name-bearing.
        self.splice_v2_meta(&mut jsonrpc_request.params);
        let body = serde_json::to_vec(&jsonrpc_request)
            .map_err(|e| Error::parse(format!("Failed to serialize v2 request: {e}")))?;

        let frames = {
            // A READ lock: the stream outlives this call and owns its own HTTP
            // response, so nothing here may hold the transport for the lifetime
            // of the subscription.
            //
            // KNOWN RESIDUAL (Phase 118.2, plan 23). This guard IS held across
            // one HTTP round trip — the `subscriptions/listen` POST's response
            // HEAD — and a read guard is no better than a write guard for that:
            // tokio's `RwLock` is fair, so a writer arriving meanwhile parks and
            // every later reader parks behind it. A peer that accepts this POST
            // and never answers therefore still serialises the client, exactly
            // as `dispatch_request` did before `send_frame`. It is NOT closed
            // here because the seam that closes it — an owned handle taken
            // before the await, see `Transport::shared_sender` — does not exist
            // on `EventStreamTransport`, whose `open_event_stream` is reached
            // through the generic `T` with no way to own one. Adding it is a
            // public-trait change and belongs to a decision, not to this
            // gap-closure round. Blast radius: callers of
            // `subscriptions/listen` only; every other client-side send is off
            // the guard.
            let transport = self.transport.read().await;
            transport.open_event_stream(body).await?
        };
        crate::client::subscriptions::SubscriptionStream::open(request_id, frames).await
    }
}

/// Builder for creating clients with custom configuration.
///
/// # Examples
///
/// ```rust
/// use pmcp::{ClientBuilder, StdioTransport};
///
/// # async fn example() -> Result<(), pmcp::Error> {
/// // Basic client builder
/// let transport = StdioTransport::new();
/// let client = ClientBuilder::new(transport)
///     .enforce_strict_capabilities(true)
///     .build();
///
/// // Client with debounced notifications
/// let transport2 = StdioTransport::new();
/// let debounced_client = ClientBuilder::new(transport2)
///     .debounced_notifications(vec![
///         "notifications/progress".to_string(),
///         "notifications/log".to_string(),
///     ])
///     .enforce_strict_capabilities(false)
///     .build();
///
/// // Chain multiple configurations
/// let transport3 = StdioTransport::new();
/// let configured_client = ClientBuilder::new(transport3)
///     .enforce_strict_capabilities(true)
///     .debounced_notifications(vec!["notifications/resources/changed".to_string()])
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder<T: Transport> {
    transport: T,
    options: ProtocolOptions,
    middleware_chain: EnhancedMiddlewareChain,
    host_registry: crate::client::host::ClientHostRegistry,
    /// The EXPLICIT per-connection protocol-version selection (Phase 113,
    /// CLNT-01). `None` with no [`ClientBuilder::with_protocol_version`] call.
    negotiated_protocol_version: Option<crate::types::ProtocolVersion>,
    /// The MRTR round bound (Phase 113, D-09). See
    /// [`ClientBuilder::mrtr_round_limit`].
    mrtr_round_limit: usize,
    /// The Extensions-Track capabilities the built client DECLARES on v2
    /// (Phase 114, D-04). See [`ClientBuilder::with_tasks_extension`].
    declared_extensions: Option<HashMap<String, serde_json::Value>>,
}

impl<T: Transport> std::fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("transport", &"<Transport>")
            .field("options", &self.options)
            .finish()
    }
}

impl<T: Transport> ClientBuilder<T> {
    /// Create a new client builder.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            options: ProtocolOptions::default(),
            middleware_chain: EnhancedMiddlewareChain::new(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
            negotiated_protocol_version: None,
            mrtr_round_limit: DEFAULT_MRTR_ROUND_LIMIT,
            declared_extensions: None,
        }
    }

    /// Opt into an EXPLICIT per-connection protocol version (Phase 113, CLNT-01).
    ///
    /// The client twin of
    /// [`Server::with_supported_protocol_versions`](crate::ServerBuilder::with_supported_protocol_versions).
    /// **With no call, the client behaves exactly as it does today** — v1, full
    /// `initialize` handshake, no v2 headers, no per-request `_meta`. There is no
    /// auto-detection: the selection is EXPLICIT and PER-CONNECTION, and the
    /// client NEVER probes `server/discover` to CHOOSE an era (Phase-113 D-08).
    ///
    /// Selecting [`PROTOCOL_VERSION_2026_07_28`](crate::types::protocol::PROTOCOL_VERSION_2026_07_28)
    /// switches the connection to the v2 wire contract:
    ///
    /// - no `initialize` / `notifications/initialized` (v2 has no handshake),
    /// - every request carries `params._meta` with the reserved
    ///   `io.modelcontextprotocol/*` keys,
    /// - every request carries `MCP-Protocol-Version`, `Mcp-Method` and
    ///   `Mcp-Name` (empty for a name-less method),
    /// - no `Mcp-Session-Id`, in either direction.
    ///
    /// The selection is pushed into the transport EXACTLY ONCE at
    /// [`ClientBuilder::build`] time via
    /// [`Transport::set_negotiated_protocol_version`]. A transport with no wire
    /// representation for it (stdio, WebSocket) logs a `tracing::warn!` at build
    /// time — v2-over-stdio is out of scope for this phase.
    ///
    /// # Errors
    ///
    /// Returns [`Error::validation`](crate::Error::validation) when `version` is
    /// neither a member of [`SUPPORTED_PROTOCOL_VERSIONS`](crate::types::SUPPORTED_PROTOCOL_VERSIONS)
    /// nor `2026-07-28`. Validating here (rather than silently emitting an
    /// arbitrary `MCP-Protocol-Version` header) closes T-113-52.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
    ///
    /// # fn main() -> Result<(), pmcp::Error> {
    /// let client = ClientBuilder::new(StdioTransport::new())
    ///     .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
    ///     .build();
    /// # Ok(()) }
    /// ```
    pub fn with_protocol_version(mut self, version: crate::types::ProtocolVersion) -> Result<Self> {
        if !Self::is_selectable_protocol_version(version.as_str()) {
            return Err(Error::validation(format!(
                "unsupported protocol version {:?}: pmcp clients may select one of {:?} or {:?}",
                version.as_str(),
                crate::types::SUPPORTED_PROTOCOL_VERSIONS,
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28,
            )));
        }
        self.negotiated_protocol_version = Some(version);
        Ok(self)
    }

    /// The versions [`Self::with_protocol_version`] accepts.
    ///
    /// `SUPPORTED_PROTOCOL_VERSIONS` deliberately does NOT list `2026-07-28`
    /// (Phase-112 Pitfall 1: v2 is reachable only via explicit opt-in, so it must
    /// never be picked by the v1 negotiation fallback), which is why the v2
    /// constant is unioned in here rather than added to that table.
    fn is_selectable_protocol_version(version: &str) -> bool {
        // ONE membership authority, shared with the server's outbound-header
        // echo — see `known_protocol_version`. It reads the same two version
        // tables `protocol_era` classifies against, so a second v2-generation
        // version added to `V2_PROTOCOL_VERSIONS` becomes selectable
        // automatically instead of silently failing opt-in.
        crate::types::protocol::known_protocol_version(version).is_some()
    }

    /// Bound the MRTR gather→resend loop (Phase 113, CLNT-02 / D-09).
    ///
    /// **With no call, the client behaves exactly as today**: the default is
    /// `8`, and on a v1 connection the bound is dead code because MRTR does not
    /// exist there.
    ///
    /// # Why a bound exists at all
    ///
    /// The spec tells a server to RE-REQUEST rather than error when a client
    /// under-supplies (`input_required` obligation 9), so a buggy or hostile
    /// server can answer `input_required` forever. The bound protects BOTH
    /// first-class client shapes (D-07):
    ///
    /// - an **AI-chat client** with a human behind the handler, who would
    ///   otherwise be re-prompted indefinitely;
    /// - an **autonomous agent client** whose handler answers programmatically
    ///   from other MCP servers, which would otherwise spin (and spend) forever.
    ///
    /// Exceeding it returns [`Error::mrtr_round_limit_exceeded`] — a
    /// programmatically distinguishable error — **without** invoking any
    /// handler for the round that trips it. Rounds are counted per LOGICAL
    /// round, so an `inputRequests` map with five entries still costs one.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport};
    ///
    /// let client = ClientBuilder::new(StdioTransport::new())
    ///     .mrtr_round_limit(3)
    ///     .build();
    /// ```
    #[must_use]
    pub fn mrtr_round_limit(mut self, limit: usize) -> Self {
        self.mrtr_round_limit = limit;
        self
    }

    /// DECLARE the tasks extension (`io.modelcontextprotocol/tasks`) on v2
    /// (Phase 114, D-04 / TASK-01).
    ///
    /// Inserts [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY)
    /// → `{}` into the client capabilities' `extensions` map. **With no call the
    /// client behaves exactly as today** and declares nothing.
    ///
    /// # This is a PER-REQUEST declaration, not a handshake
    ///
    /// v2 (`2026-07-28`) removed `initialize`, so there is no negotiation round
    /// to carry it. The declaration therefore travels on EVERY request, inside
    /// `params._meta["io.modelcontextprotocol/clientCapabilities"].extensions`,
    /// and the server reads it out of the request it is answering. On a v1
    /// connection this setter changes nothing on the wire: v1 advertises the
    /// `ClientCapabilities` the caller passed to [`Client::initialize`], and
    /// injecting here would move the `initialize` bytes of every existing
    /// caller.
    ///
    /// # What declaring it means
    ///
    /// It is what the spec requires before a server may answer a task-capable
    /// tool call with a task handle: the declaration is the server's CREATE
    /// trigger. A client that declares it is announcing that it can handle a
    /// `resultType:"task"` response — poll `tasks/get`, fetch `tasks/result` —
    /// instead of the ordinary synchronous result. A client that cannot do that
    /// must NOT declare it, or it will receive task handles it cannot follow.
    ///
    /// # It is SELF-REPORTED (T-114-22)
    ///
    /// The `extensions` map says what this client can HANDLE. It is unverified
    /// and forgeable by construction, exactly like
    /// `io.modelcontextprotocol/clientInfo`. A server may read it to decide what
    /// may be SERVED; it must never read it as identity, and never derive
    /// authorization from it. Owner binding reads the authenticated principal
    /// (`AuthContext`), never this map.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
    ///
    /// # fn main() -> Result<(), pmcp::Error> {
    /// let client = ClientBuilder::new(StdioTransport::new())
    ///     .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
    ///     .with_tasks_extension()
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_tasks_extension(mut self) -> Self {
        self.declared_extensions
            .get_or_insert_with(HashMap::new)
            .insert(
                crate::types::capabilities::TASKS_EXTENSION_KEY.to_string(),
                serde_json::to_value(
                    crate::types::capabilities::TasksExtensionCapability::default(),
                )
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new())),
            );
        self
    }

    /// Set whether to enforce strict capabilities.
    pub fn enforce_strict_capabilities(mut self, enforce: bool) -> Self {
        self.options.enforce_strict_capabilities = enforce;
        self
    }

    /// Set debounced notification methods.
    pub fn debounced_notifications(mut self, methods: Vec<String>) -> Self {
        self.options.debounced_notification_methods = methods;
        self
    }

    /// Add middleware to the client.
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_middleware(
        mut self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.middleware_chain.add(middleware);
        self
    }

    /// Add protocol-level middleware to the client.
    ///
    /// This is an alias for `with_middleware()` that provides explicit naming to distinguish
    /// protocol middleware (operates on JSON-RPC messages) from HTTP middleware
    /// (operates on HTTP requests/responses via `StreamableHttpTransportConfigBuilder`).
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_protocol_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_protocol_middleware(
        self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.with_middleware(middleware)
    }

    /// Set the entire middleware chain.
    ///
    /// This replaces any previously configured middleware.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::EnhancedMiddlewareChain;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let mut chain = EnhancedMiddlewareChain::new();
    /// // Add middleware to chain...
    ///
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .middleware_chain(chain)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn middleware_chain(mut self, chain: EnhancedMiddlewareChain) -> Self {
        self.middleware_chain = chain;
        self
    }

    /// Register a host sampling handler answering inbound
    /// `sampling/createMessage` requests (the MCP host direction).
    ///
    /// This is the INVERSE of [`Client::create_message`] (the LLM-server
    /// pattern). See [`crate::client::host`] for the full disambiguation.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::client::host::HostSamplingHandler;
    /// use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
    /// use pmcp::types::Content;
    /// use async_trait::async_trait;
    ///
    /// struct MyHost;
    /// #[async_trait]
    /// impl HostSamplingHandler for MyHost {
    ///     async fn handle_create_message(
    ///         &self,
    ///         _params: CreateMessageParams,
    ///     ) -> pmcp::Result<CreateMessageResult> {
    ///         Ok(CreateMessageResult::new(Content::text("hi"), "my-model"))
    ///     }
    /// }
    ///
    /// let client = ClientBuilder::new(StdioTransport::new())
    ///     .on_sampling(MyHost)
    ///     .build();
    /// ```
    pub fn on_sampling(mut self, handler: impl host::HostSamplingHandler + 'static) -> Self {
        self.host_registry.sampling = Some(Arc::new(handler));
        self
    }

    /// Register a **tool-aware** host sampling handler answering inbound
    /// `sampling/createMessage` requests with a
    /// [`CreateMessageResultWithTools`](crate::types::sampling::CreateMessageResultWithTools).
    ///
    /// This is the `WithTools` counterpart of [`ClientBuilder::on_sampling`]: the
    /// handler can return `tool_use` / `tool_result` content blocks that the
    /// legacy single-`Content` result cannot express (MCP 2025-11-25). When both
    /// a legacy and a `WithTools` handler are registered, the `WithTools` handler is
    /// preferred. The preflight approval gate
    /// ([`ClientBuilder::on_sampling_approval`]) still runs unchanged; the
    /// optional result-review gate sees a single-content projection of the
    /// completion (tool blocks rendered as a text marker) so it is never
    /// silently bypassed.
    pub fn on_sampling_with_tools(
        mut self,
        handler: impl host::HostSamplingHandlerWithTools + 'static,
    ) -> Self {
        self.host_registry.sampling_with_tools = Some(Arc::new(handler));
        self
    }

    /// Register a host elicitation handler answering inbound
    /// `elicitation/create` requests.
    pub fn on_elicitation(mut self, handler: impl host::HostElicitationHandler + 'static) -> Self {
        self.host_registry.elicitation = Some(Arc::new(handler));
        self
    }

    /// Register a roots provider answering inbound `roots/list` requests.
    ///
    /// The provider is generic over any closure returning a future that yields
    /// `Result<ListRootsResult>`, so callers never construct the
    /// [`RootsProvider`] alias by hand.
    pub fn on_roots<F, Fut>(mut self, provider: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<crate::types::roots::ListRootsResult>>
            + Send
            + 'static,
    {
        self.host_registry.roots = Some(Arc::new(move || Box::pin(provider())));
        self
    }

    /// Register an optional pre-handler sampling approval gate.
    ///
    /// The callback is generic over any closure taking owned
    /// [`CreateMessageParams`] and returning a future that yields an
    /// [`ApprovalDecision`]. It is invoked by `dispatch_host_sampling` BEFORE
    /// the sampling handler runs as of this phase, so an
    /// [`ApprovalDecision::Deny`] prevents the LLM
    /// call entirely. The gate is optional and default-allow: when none is
    /// registered, inbound sampling reaches the handler unchallenged.
    pub fn on_sampling_approval<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(CreateMessageParams) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = host::ApprovalDecision> + Send + 'static,
    {
        self.host_registry.approval = Some(Arc::new(move |params| Box::pin(callback(params))));
        self
    }

    /// Register an optional post-handler sampling result review.
    ///
    /// The callback receives the owned request params and the produced
    /// [`CreateMessageResult`] and returns a future yielding an
    /// [`ApprovalDecision`]. It is invoked by `dispatch_host_sampling` AFTER the
    /// sampling handler runs as of this phase, so an
    /// [`ApprovalDecision::Deny`] suppresses the
    /// completion. It is optional and default pass-through: when none is
    /// registered the completion is returned as-is.
    pub fn on_sampling_result_review<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(CreateMessageParams, CreateMessageResult) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = host::ApprovalDecision> + Send + 'static,
    {
        self.host_registry.result_review = Some(Arc::new(move |params, result| {
            Box::pin(callback(params, result))
        }));
        self
    }

    /// Build the client.
    pub fn build(self) -> Client<T> {
        let mut transport = self.transport;
        // The mode-propagation seam (Phase 113, CLNT-01): the selection crosses
        // into the transport EXACTLY ONCE, and only when the caller actually made
        // one — a non-opted-in build never touches the transport at all, so its
        // wire bytes are byte-identical to every prior release.
        if let Some(version) = self.negotiated_protocol_version.as_ref() {
            transport.set_negotiated_protocol_version(Some(version.as_str().to_string()));
            if version.as_str() == crate::types::protocol::PROTOCOL_VERSION_2026_07_28
                && !transport.supports_negotiated_protocol_version()
            {
                // T-113-53: an inert v2 selection would otherwise emit requests
                // no v2 server accepts, with no local signal at all.
                tracing::warn!(
                    transport = transport.transport_type(),
                    "protocol version 2026-07-28 was selected but this transport has no wire \
                     representation for it — the selection is INERT (v2 is streamable-HTTP only \
                     in this release)"
                );
            }
        }

        let mut client = Client::with_options(
            transport,
            Implementation::new("pmcp-client", env!("CARGO_PKG_VERSION")),
            self.options,
        );
        // Replace the default middleware chain with the configured one
        client.middleware_chain = Arc::new(RwLock::new(self.middleware_chain));
        // Thread the configured host registry onto the client.
        client.host_registry = self.host_registry;
        client.negotiated_protocol_version = self.negotiated_protocol_version;
        client.mrtr_round_limit = self.mrtr_round_limit;
        client.declared_extensions = self.declared_extensions;
        // v2 has NO handshake, so a v2 client is ready the moment it is built.
        // `ensure_initialized` therefore passes without an `initialize` round
        // trip, which is the whole point of the stateless era.
        if client.is_v2() {
            client.initialized = true;
        }
        client
    }
}

impl<T: Transport> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            shared_sender: self.shared_sender.clone(),
            protocol: self.protocol.clone(),
            middleware_chain: self.middleware_chain.clone(),
            capabilities: self.capabilities.clone(),
            server_capabilities: self.server_capabilities.clone(),
            server_version: self.server_version.clone(),
            instructions: self.instructions.clone(),
            initialized: self.initialized,
            info: self.info.clone(),
            notification_tx: self.notification_tx.clone(),
            active_requests: self.active_requests.clone(),
            abandoned_requests: self.abandoned_requests.clone(),
            options: self.options.clone(),
            host_registry: self.host_registry.clone(),
            negotiated_protocol_version: self.negotiated_protocol_version.clone(),
            mrtr_round_limit: self.mrtr_round_limit,
            declared_extensions: self.declared_extensions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::{JSONRPCError, ResponsePayload},
        JSONRPCResponse, ProgressNotification, ProgressToken, TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_responses(responses: Vec<TransportMessage>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, response: TransportMessage) {
            self.responses.lock().unwrap().push(response);
        }

        /// Re-address a canned RESPONSE to the id of the request still awaiting
        /// one (Phase 118.2, CR-02).
        ///
        /// Every canned response below carries a hand-written id (`1`, `2`, ...)
        /// while `Client` mints its own — a `RequestId::String` holding a UUID for
        /// `call_tool`, a counter elsewhere. Until CR-02, `dispatch_request`
        /// returned the first `Response` frame it popped WITHOUT comparing ids, so
        /// that mismatch was invisible; it is now refused, exactly as a fabricated
        /// id from a real peer is (T-118.2-15-02).
        ///
        /// Echoing the id is what a CONFORMANT server does — JSON-RPC 2.0 requires
        /// the response id to "be the same as the value of the id member in the
        /// Request Object" — so this makes the mock conformant rather than working
        /// around the check. The canned `payload` is untouched, and that is what
        /// every test here actually asserts on.
        ///
        /// A mock with no recorded request yet is left alone, so a test that
        /// exercises an UNSOLICITED frame keeps the id it wrote.
        ///
        /// # What this mock CANNOT prove, and where that coverage lives
        ///
        /// Because it forces every id to match, a client that routes by id and a
        /// client that simply returns the next `Response` frame behave
        /// identically under it — so no suite built on this mock can tell the two
        /// apart. (A BROKEN lookup is still caught: a response that matches
        /// nothing is never delivered, and the awaiting call fails.) The
        /// discriminating arm is
        /// [`a_re_typed_response_id_fails_the_call_rather_than_hanging`], which
        /// answers with a deliberately mis-typed id and asserts the call refuses
        /// it. Keep that arm alive if this helper is ever changed.
        fn addressed_to_the_pending_request(&self, message: TransportMessage) -> TransportMessage {
            match message {
                TransportMessage::Response(mut response) => {
                    if let Some(id) = self.last_request_id() {
                        response.id = id;
                    }
                    TransportMessage::Response(response)
                },
                other => other,
            }
        }

        /// The id of the most recent REQUEST this mock was sent.
        fn last_request_id(&self) -> Option<RequestId> {
            self.sent_messages
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|sent| match sent {
                    TransportMessage::Request { id, .. } => Some(id.clone()),
                    _ => None,
                })
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let message = self
                .responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| Error::protocol_msg("No more responses"))?;
            Ok(self.addressed_to_the_pending_request(message))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// A transport that answers every request with a RE-TYPED id.
    ///
    /// JSON-RPC 2.0 ids are typed, so `String("1")` is not `Number(1)`. This is
    /// the peer shape the router cannot route and must therefore refuse.
    #[derive(Debug, Default)]
    struct RetypingTransport {
        answered: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl Transport for RetypingTransport {
        async fn send(&mut self, _message: TransportMessage) -> Result<()> {
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let mut answered = self.answered.lock().unwrap();
            *answered += 1;
            Ok(TransportMessage::Response(
                crate::types::JSONRPCResponse::success(
                    // A STRING id, whatever the client minted. A conformant peer
                    // echoes the request's id verbatim, type included.
                    RequestId::String(format!("{answered}")),
                    serde_json::json!({}),
                ),
            ))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// THE ceiling claim (code review of Phase 118.2).
    ///
    /// `pump_once` drops a response no request is awaiting. With no ceiling —
    /// and pmcp has no request timeout — a peer that re-types an id leaves
    /// `dispatch_request` polling at 4 Hz forever, holding an `active_requests`
    /// entry and a caller task for the life of the process. The commit that
    /// introduced the router deleted the bound that covered exactly this.
    ///
    /// It is ALSO the arm that proves the client routes BY id at all: the
    /// re-addressing `MockTransport` above forces every id to match, so no suite
    /// built on it can distinguish routing from "return the next frame".
    ///
    /// Bounded by `timeout`, because the defect is a HANG: without the bound a
    /// regression would wedge the suite rather than fail it.
    #[tokio::test]
    async fn a_re_typed_response_id_fails_the_call_rather_than_hanging() {
        let client = Client::new(RetypingTransport::default());
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.send_request(
                RequestId::Number(1),
                Request::Client(Box::new(crate::types::ClientRequest::ListTools(
                    crate::types::ListToolsRequest { cursor: None },
                ))),
            ),
        )
        .await
        .expect("the call must FAIL, not hang — hanging is the defect this arm fences")
        .expect_err("a peer that answers nothing this client asked for cannot succeed");

        let rendered = error.to_string();
        assert!(
            rendered.contains("no request is awaiting"),
            "the failure must NAME the defect so a re-typed id is not mistaken for a slow peer; \
             got {rendered}"
        );
    }

    /// The budget arms on the FIRST unmatched frame and never re-arms.
    ///
    /// Re-arming per frame would let a peer emitting a steady drip of wrong ids
    /// extend the wait indefinitely: the ceiling would exist and never fire.
    #[test]
    fn the_unmatched_budget_arms_once_and_never_moves() {
        let mut budget = UnmatchedBudget::default();
        assert!(
            budget.deadline.is_none(),
            "an untouched budget must NOT be armed — a slow but honest peer is not bounded here"
        );
        assert!(
            budget.exhausted(&RequestId::Number(1), None).is_none(),
            "and an unarmed budget can never be exhausted"
        );

        budget.record();
        let armed = budget.deadline.expect("the first unmatched frame arms it");
        for _ in 0..8 {
            budget.record();
        }
        assert_eq!(
            budget.deadline,
            Some(armed),
            "later frames must not push the deadline out; that is how a ceiling exists and never \
             fires"
        );
        assert_eq!(budget.seen, 9, "every frame is counted");
    }

    /// The COUNT bound fires on its own, so a fast sprayer does not get to run
    /// out the full timeout first.
    #[test]
    fn the_unmatched_budget_fails_on_the_count_alone() {
        let mut budget = UnmatchedBudget::default();
        for _ in 0..MAX_UNMATCHED_RESPONSES {
            budget.record();
        }
        let error = budget
            .exhausted(&RequestId::Number(1), Some("String(\"1\")"))
            .expect("the count bound must fire without waiting for the deadline");
        let rendered = error.to_string();
        assert!(
            rendered.contains("String(\"1\")"),
            "the offending id must be named — it is the whole diagnosis; got {rendered}"
        );
    }

    /// A recorded id is absorbed as our own debris.
    #[test]
    fn an_abandoned_id_is_absorbed() {
        let mut ledger = AbandonedRequestIds::default();
        ledger.record(RequestId::Number(7));
        assert!(
            ledger.take(&RequestId::Number(7)),
            "the late answer to a request this client abandoned is OUR debris and must be \
             absorbed, not charged to whichever unrelated call happens to be pumping"
        );
    }

    /// And absorbed ONCE: the entry is consumed on first use.
    ///
    /// The spoofing bound (T-118.2-22-03). Without it a peer could replay one
    /// abandoned id indefinitely and have every replay absorbed silently; with
    /// it the first replay is absorbed and every later one is booked against the
    /// budget as the misbehaviour it is.
    #[test]
    fn an_abandoned_id_is_absorbed_only_once() {
        let mut ledger = AbandonedRequestIds::default();
        ledger.record(RequestId::Number(7));
        assert!(ledger.take(&RequestId::Number(7)), "the first use absorbs");
        assert!(
            !ledger.take(&RequestId::Number(7)),
            "the entry is CONSUMED on first use, so a peer that replays one abandoned id N times \
             is absorbed once and the remaining N-1 replays still spend budget"
        );
    }

    /// An id that was never recorded is still the peer's misbehaviour.
    ///
    /// The half of the classification that must NOT move: the re-typed id and
    /// the id-spraying peer still spend budget at the same two bounds. This fix
    /// removes one classification, not the ceiling.
    #[test]
    fn an_id_this_client_never_minted_is_not_absorbed() {
        let mut ledger = AbandonedRequestIds::default();
        ledger.record(RequestId::Number(7));
        assert!(
            !ledger.take(&RequestId::from("an-id-no-request-here-ever-minted")),
            "an id nobody here asked for must stay unmatched, or the ceiling stops firing for \
             the peer misbehaviour it was written for"
        );
        assert!(
            !ledger.take(&RequestId::String("7".to_string())),
            "and ids are TYPED: String(\"7\") is not Number(7), so a re-typed answer is not \
             absorbed by the entry its correctly-typed twin left"
        );
    }

    /// The container is bounded BY CONSTRUCTION: oldest-evicted at a fixed cap.
    ///
    /// This is what makes the memory bound a property of the type rather than a
    /// hope about peer behaviour — and it holds even though no peer can reach
    /// this path at all, because only locally-minted ids are ever recorded.
    #[test]
    fn the_abandoned_ledger_evicts_the_oldest_at_its_cap() {
        let mut ledger = AbandonedRequestIds::default();
        let overflow: i64 = 16;
        let cap = i64::try_from(MAX_ABANDONED_REQUEST_IDS).expect("the cap fits in an i64");
        for id in 0..(cap + overflow) {
            ledger.record(RequestId::Number(id));
        }
        assert_eq!(
            ledger.ids.len(),
            MAX_ABANDONED_REQUEST_IDS,
            "the deque must never exceed its cap; an unbounded holding pen would be memory growth"
        );
        assert!(
            !ledger.take(&RequestId::Number(0)),
            "the OLDEST entries are the ones evicted — id 0 was recorded first and is gone"
        );
        assert!(
            ledger.take(&RequestId::Number(cap + overflow - 1)),
            "and the newest is retained, which is the entry a live abandonment actually needs"
        );
    }

    /// Registering a request CLEARS any stale entry for its id.
    ///
    /// Drives the real registration path — `dispatch_request` — rather than the
    /// struct alone, because the property is about a live registration never
    /// being SHADOWED by debris from a previous life of the same id: without the
    /// take, that request's own answer would be absorbed as somebody else's
    /// leftovers instead of delivered.
    ///
    /// The observation is a COUNT, so it needs no racing and no clock. The mock
    /// answers nothing, so each dispatch fails and its exit cleanup records one
    /// abandonment. Two dispatches of the SAME id therefore leave exactly ONE
    /// entry if the second registration took the first one, and TWO if it did
    /// not.
    #[tokio::test]
    async fn registering_a_request_clears_stale_debris_for_its_id() {
        let transport = MockTransport::with_responses(vec![]);
        let client = Client::new(transport);
        let reused = RequestId::from("reused-across-two-lives");

        for _ in 0..2 {
            let failed = client
                .dispatch_request(
                    reused.clone(),
                    Some(Request::Client(Box::new(ClientRequest::ListTools(
                        ListToolsRequest { cursor: None },
                    )))),
                    crate::types::JSONRPCRequest {
                        jsonrpc: "2.0".to_string(),
                        id: reused.clone(),
                        method: "tools/list".to_string(),
                        params: Some(serde_json::json!({})),
                    },
                )
                .await;
            assert!(
                failed.is_err(),
                "the mock answers nothing, so each dispatch must fail and leave an abandonment — \
                 a success here means this fence is counting something else entirely"
            );
        }

        let entries = client
            .abandoned_requests
            .read()
            .await
            .ids
            .iter()
            .filter(|id| *id == &reused)
            .count();
        assert_eq!(
            entries, 1,
            "the second registration must TAKE the entry the first abandonment left. Two entries \
             mean a live registration can be shadowed by debris from a previous life of the same \
             id, and that request's real answer would be absorbed rather than delivered"
        );
    }

    /// THE echo bound (code review of Phase 118.2).
    ///
    /// A response id is remote input of unbounded length. The transport bounds
    /// its own untrusted echo with `MAX_ECHOED_SSE_FRAME` and states the rule;
    /// the router has to follow it.
    #[test]
    fn an_echoed_request_id_is_bounded_and_typed() {
        let short = echoed_request_id(&RequestId::Number(7));
        assert_eq!(
            short, "Number(7)",
            "a short id is rendered in its TYPED form, so a re-typing report does not read as a \
             contradiction"
        );
        assert_eq!(
            echoed_request_id(&RequestId::String("7".to_string())),
            "String(\"7\")",
            "and the string twin is visibly a different id"
        );

        let hostile = echoed_request_id(&RequestId::String("A".repeat(1024 * 1024)));
        assert!(
            hostile.len() < MAX_ECHOED_REQUEST_ID + 64,
            "a 1 MiB id must not reach a log line verbatim; got {} bytes",
            hostile.len()
        );
        assert!(
            hostile.contains("withheld"),
            "and a truncated echo must SAY it was truncated, or it reads as a short id"
        );
    }

    /// Truncation lands on a `char` boundary: `&str[..n]` panics mid-codepoint,
    /// and a peer choosing a multi-byte id must not be able to panic a client.
    #[test]
    fn an_echoed_request_id_never_splits_a_codepoint() {
        for pad in 0..8 {
            let id = RequestId::String(format!("{}{}", "x".repeat(pad), "é".repeat(512)));
            let rendered = echoed_request_id(&id);
            assert!(
                rendered.contains("withheld"),
                "the fixture must actually exceed the cap at pad {pad}"
            );
        }
    }

    #[test]
    fn test_client_creation() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert!(!client.initialized);
        assert_eq!(client.info.name, "pmcp-client");
    }

    #[test]
    fn test_client_with_info() {
        let transport = MockTransport::new();
        let info = Implementation::new("test-client", "1.0.0");
        let client = Client::with_info(transport, info);
        assert_eq!(client.info.name, "test-client");
        assert_eq!(client.info.version, "1.0.0");
    }

    // === ClientOptions wiring tests ===

    #[test]
    fn test_client_new_uses_default_options() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert_eq!(client.options.max_iterations, 100);
    }

    #[test]
    fn test_client_with_client_options_threads_value() {
        let transport = MockTransport::new();
        let opts = ClientOptions {
            max_iterations: 7,
            ..Default::default()
        };
        let client = Client::with_client_options(transport, opts);
        assert_eq!(client.options.max_iterations, 7);
    }

    #[test]
    fn test_client_with_options_preserves_default_client_options() {
        let transport = MockTransport::new();
        let client = Client::with_options(
            transport,
            Implementation::default(),
            ProtocolOptions::default(),
        );
        assert_eq!(client.options.max_iterations, 100);
    }

    // === Client host dispatch unit tests (HOST-01/HOST-05) ===

    struct MockHostSampling;

    #[async_trait]
    impl host::HostSamplingHandler for MockHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            Ok(CreateMessageResult::new(
                crate::types::Content::text("mock host completion"),
                "mock-host-model",
            ))
        }
    }

    struct FailingHostSampling;

    #[async_trait]
    impl host::HostSamplingHandler for FailingHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            Err(Error::protocol_msg(
                "secret path /etc/passwd leaked in error",
            ))
        }
    }

    fn sampling_client_alias_request() -> Request {
        // Inbound sampling parses as the CLIENT variant (parse ambiguity).
        Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))))
    }

    #[tokio::test]
    async fn test_dispatch_sampling_alias_reaches_handler() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let id = RequestId::from(1i64);
        let response = client
            .dispatch_host_request(id, sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "inbound sampling (client-alias parse) must reach the host handler, got: {response:?}"
        );
    }

    #[tokio::test]
    async fn test_dispatch_known_unhandled_returns_method_not_found() {
        // No handlers registered; a KNOWN roots/list request must yield -32601.
        let client = Client::new(MockTransport::new());
        let id = RequestId::from(2i64);
        let request = Request::Server(Box::new(crate::types::ServerRequest::ListRoots));
        let response = client.dispatch_host_request(id, request).await;
        match response.payload {
            ResponsePayload::Error(e) => assert_eq!(e.code, -32601),
            ResponsePayload::Result(r) => panic!("expected -32601 error, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_handler_error_is_sanitized_32603() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(FailingHostSampling)
            .build();
        let id = RequestId::from(3i64);
        let response = client
            .dispatch_host_request(id, sampling_client_alias_request())
            .await;
        match response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32603);
                // Sanitized: the raw handler error text must NOT cross the wire.
                assert!(
                    !e.message.contains("/etc/passwd"),
                    "handler error text must be sanitized, got: {}",
                    e.message
                );
            },
            ResponsePayload::Result(r) => panic!("expected -32603 error, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_roots_provider_answers() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_roots(|| async { Ok(crate::types::roots::ListRootsResult { roots: Vec::new() }) })
            .build();
        let id = RequestId::from(4i64);
        let request = Request::Server(Box::new(crate::types::ServerRequest::ListRoots));
        let response = client.dispatch_host_request(id, request).await;
        assert!(
            response.is_success(),
            "roots provider must answer roots/list"
        );
    }

    // === Sampling approval (preflight + result-review) unit tests (HOST-04) ===

    /// Host sampling handler that flips an `AtomicBool` when invoked, so tests
    /// can prove whether the LLM call happened.
    struct TrackingHostSampling {
        invoked: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl host::HostSamplingHandler for TrackingHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            self.invoked
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(CreateMessageResult::new(
                crate::types::Content::text("tracked completion"),
                "tracked-model",
            ))
        }
    }

    fn assert_policy_denied(response: &JSONRPCResponse) {
        match &response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32603, "policy denial must be -32603");
                assert_eq!(
                    e.message, "request denied by host policy",
                    "policy denial message must be the generic sanitized string"
                );
            },
            ResponsePayload::Result(r) => panic!("expected -32603 denial, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_sampling_no_preflight_runs_handler() {
        // (a) No preflight callback => handler runs, completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(10i64), sampling_client_alias_request())
            .await;
        assert!(response.is_success(), "default (no preflight) must allow");
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must run when no preflight is registered"
        );
    }

    #[tokio::test]
    async fn test_sampling_preflight_deny_skips_handler() {
        // (b) Preflight Deny => handler is NOT called (denial-of-wallet fix) and
        // the raw deny reason must NOT cross the wire.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async {
                host::ApprovalDecision::Deny("local-secret-reason".to_string())
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(11i64), sampling_client_alias_request())
            .await;
        assert_policy_denied(&response);
        assert!(
            !invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must NOT run when preflight denies (no LLM call, no tokens)"
        );
        // The raw deny reason must never be forwarded.
        if let ResponsePayload::Error(e) = &response.payload {
            assert!(
                !e.message.contains("local-secret-reason"),
                "deny reason must be logged locally, not forwarded: {}",
                e.message
            );
        }
    }

    #[tokio::test]
    async fn test_sampling_preflight_allow_runs_handler() {
        // (c) Preflight Allow => completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async { host::ApprovalDecision::Allow })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(12i64), sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "preflight Allow must return completion"
        );
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must run after preflight Allow"
        );
    }

    #[tokio::test]
    async fn test_sampling_result_review_deny_after_handler() {
        // (d) result_review Deny after an allowed preflight => -32603, but the
        // handler WAS called (generation happened, then was suppressed).
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_result_review(|_params, _result| async {
                host::ApprovalDecision::Deny("post-gen-reason".to_string())
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(13i64), sampling_client_alias_request())
            .await;
        assert_policy_denied(&response);
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler runs before result review can deny"
        );
    }

    #[tokio::test]
    async fn test_sampling_result_review_absent_is_passthrough() {
        // (e) result_review absent => pass-through (Allow), completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async { host::ApprovalDecision::Allow })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(14i64), sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "absent result_review must pass through"
        );
    }

    // === Capability derivation unit tests (HOST-05) ===

    struct MockHostElicit;

    #[async_trait]
    impl host::HostElicitationHandler for MockHostElicit {
        async fn handle_elicitation(
            &self,
            _params: crate::types::elicitation::ElicitRequestParams,
        ) -> Result<crate::types::elicitation::ElicitResult> {
            Ok(crate::types::elicitation::ElicitResult {
                action: crate::types::elicitation::ElicitAction::Accept,
                content: None,
            })
        }
    }

    #[test]
    fn test_capability_sampling_registered_is_present() {
        // (a) handler registered + default caps => sampling present.
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(caps.sampling.is_some(), "registered sampling => present");
    }

    #[test]
    fn test_capability_sampling_unregistered_default_is_absent() {
        // (b) no handler + default caps => sampling absent.
        let client = Client::new(MockTransport::new());
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(caps.sampling.is_none(), "unregistered sampling => absent");
    }

    #[test]
    fn test_capability_sampling_anti_lie_discards_caller_value() {
        // (c) ANTI-LIE: no handler + caller-set sampling => forced None.
        let client = Client::new(MockTransport::new());
        let mut caps = ClientCapabilities {
            sampling: Some(crate::types::capabilities::SamplingCapabilities::default()),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        assert!(
            caps.sampling.is_none(),
            "caller-set sampling with no handler must be discarded (anti-capability-lie)"
        );
    }

    #[test]
    fn test_capability_sampling_preserves_caller_detail() {
        // (d) PRESERVATION: handler present + caller-configured sub-field =>
        // that exact detail is preserved (not reset to default()).
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let mut caps = ClientCapabilities {
            sampling: Some(crate::types::capabilities::SamplingCapabilities {
                models: Some(vec!["gpt-4o".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        let sampling = caps.sampling.expect("handler present => sampling kept");
        assert_eq!(
            sampling.models,
            Some(vec!["gpt-4o".to_string()]),
            "caller-configured models must be preserved, not reset to default"
        );
    }

    #[test]
    fn test_capability_elicitation_and_roots_parallel() {
        // (e) elicitation + roots follow the same rule.
        // Registered => present.
        let client = ClientBuilder::new(MockTransport::new())
            .on_elicitation(MockHostElicit)
            .on_roots(|| async { Ok(crate::types::roots::ListRootsResult { roots: Vec::new() }) })
            .build();
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(
            caps.elicitation.is_some(),
            "registered elicitation => present"
        );
        assert!(caps.roots.is_some(), "registered roots => present");

        // Unregistered + caller-set => discarded (anti-lie) for both.
        let bare = Client::new(MockTransport::new());
        let mut caps2 = ClientCapabilities {
            elicitation: Some(crate::types::capabilities::ElicitationCapabilities::default()),
            roots: Some(crate::types::capabilities::RootsCapabilities::default()),
            ..Default::default()
        };
        bare.derive_host_capabilities(&mut caps2);
        assert!(caps2.elicitation.is_none(), "elicitation anti-lie");
        assert!(caps2.roots.is_none(), "roots anti-lie");
    }

    #[test]
    fn test_capability_derivation_leaves_tasks_and_experimental_untouched() {
        // (f) tasks / experimental are never modified by host derivation.
        let client = Client::new(MockTransport::new());
        let mut experimental = HashMap::new();
        experimental.insert("custom".to_string(), serde_json::json!(true));
        let mut caps = ClientCapabilities {
            tasks: Some(crate::types::capabilities::ClientTasksCapability::default()),
            experimental: Some(experimental),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        assert!(caps.tasks.is_some(), "tasks must be preserved");
        assert_eq!(
            caps.experimental.and_then(|e| e.get("custom").cloned()),
            Some(serde_json::json!(true)),
            "experimental must be preserved"
        );
    }

    // =======================================================================
    // MRTR `inputRequests` fold (Phase 113, CLNT-02).
    // =======================================================================

    mod mrtr_fold {
        use super::*;
        use crate::types::content::Role;
        use crate::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
        use crate::types::mrtr::{InputRequest, InputRequests, InputResponse};
        use crate::types::roots::{ListRootsResult, Root};
        use crate::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// An elicitation handler that counts invocations and returns a
        /// configurable action.
        struct CountingElicitation {
            calls: Arc<AtomicUsize>,
            action: ElicitAction,
        }

        #[async_trait]
        impl host::HostElicitationHandler for CountingElicitation {
            async fn handle_elicitation(
                &self,
                _params: ElicitRequestParams,
            ) -> Result<ElicitResult> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let mut content = HashMap::new();
                content.insert("user_name".to_string(), serde_json::json!("ada"));
                Ok(ElicitResult {
                    action: self.action,
                    content: matches!(self.action, ElicitAction::Accept).then_some(content),
                })
            }
        }

        /// A sampling handler that counts invocations.
        struct CountingSampling {
            calls: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl host::HostSamplingHandler for CountingSampling {
            async fn handle_create_message(
                &self,
                _params: CreateMessageParams,
            ) -> Result<CreateMessageResult> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(CreateMessageResult::new(
                    crate::types::Content::text("sampled"),
                    "test-model",
                ))
            }
        }

        struct WithToolsSampling;

        #[async_trait]
        impl host::HostSamplingHandlerWithTools for WithToolsSampling {
            async fn handle_create_message_with_tools(
                &self,
                _params: CreateMessageParams,
            ) -> Result<CreateMessageResultWithTools> {
                Ok(CreateMessageResultWithTools::new(
                    "with-tools-model",
                    Role::Assistant,
                    vec![SamplingMessageContent::Text {
                        text: "sampled with tools".to_string(),
                        meta: None,
                    }],
                ))
            }
        }

        fn elicitation_request() -> InputRequest {
            InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
                message: "who?".to_string(),
                requested_schema: serde_json::json!({}),
            }))
        }

        fn sampling_request() -> InputRequest {
            InputRequest::Sampling(Box::new(CreateMessageParams::new(Vec::new())))
        }

        fn requests(entries: Vec<(&str, InputRequest)>) -> InputRequests {
            entries
                .into_iter()
                .map(|(key, request)| (key.to_string(), request))
                .collect()
        }

        /// All three kinds are answered from the already-registered Phase-106
        /// registry, and the server-assigned keys are preserved VERBATIM.
        #[tokio::test]
        async fn folds_all_three_kinds_preserving_keys() {
            let elicit_calls = Arc::new(AtomicUsize::new(0));
            let sample_calls = Arc::new(AtomicUsize::new(0));
            let client = ClientBuilder::new(MockTransport::new())
                .on_elicitation(CountingElicitation {
                    calls: elicit_calls.clone(),
                    action: ElicitAction::Accept,
                })
                .on_sampling(CountingSampling {
                    calls: sample_calls.clone(),
                })
                .on_roots(|| async {
                    Ok(ListRootsResult {
                        roots: vec![Root {
                            uri: "file:///tmp".to_string(),
                            name: None,
                        }],
                    })
                })
                .build();

            let map = requests(vec![
                ("server_key_a", elicitation_request()),
                ("server_key_b", sampling_request()),
                ("server_key_c", InputRequest::ListRoots),
            ]);
            let FoldOutcome::Fulfilled(responses) = client.fold_input_requests(&map).await else {
                panic!("every kind has a registered handler");
            };

            assert_eq!(responses.len(), 3);
            // Keys are the SERVER's, verbatim — never re-derived.
            assert!(matches!(
                responses.get("server_key_a"),
                Some(InputResponse::Elicitation(_))
            ));
            assert!(matches!(
                responses.get("server_key_b"),
                Some(InputResponse::Sampling(_))
            ));
            assert!(matches!(
                responses.get("server_key_c"),
                Some(InputResponse::Roots(_))
            ));
            assert_eq!(elicit_calls.load(Ordering::SeqCst), 1);
            assert_eq!(sample_calls.load(Ordering::SeqCst), 1);
        }

        /// `on_sampling_with_tools` alone can answer a sampling entry — the
        /// same precedence the v1 dispatch applies.
        #[tokio::test]
        async fn folds_a_with_tools_only_sampling_handler() {
            let client = ClientBuilder::new(MockTransport::new())
                .on_sampling_with_tools(WithToolsSampling)
                .build();
            let map = requests(vec![("k", sampling_request())]);
            let FoldOutcome::Fulfilled(responses) = client.fold_input_requests(&map).await else {
                panic!("a WithTools handler must satisfy a sampling entry");
            };
            assert!(matches!(
                responses.get("k"),
                Some(InputResponse::Sampling(_))
            ));
        }

        /// PREFLIGHT: the map's FIRST entry is fulfillable and the SECOND is
        /// not, so ZERO handlers may run — otherwise a human is prompted (or an
        /// agent's tokens are spent) for work the all-or-nothing fold discards.
        #[tokio::test]
        async fn preflight_failure_invokes_zero_handlers() {
            let elicit_calls = Arc::new(AtomicUsize::new(0));
            let client = ClientBuilder::new(MockTransport::new())
                .on_elicitation(CountingElicitation {
                    calls: elicit_calls.clone(),
                    action: ElicitAction::Accept,
                })
                .build();

            // BTreeMap ordering: "a" (fulfillable) is visited before "b".
            let map = requests(vec![
                ("a", elicitation_request()),
                ("b", sampling_request()),
            ]);
            assert!(matches!(
                client.fold_input_requests(&map).await,
                FoldOutcome::CannotFulfil
            ));
            assert_eq!(
                elicit_calls.load(Ordering::SeqCst),
                0,
                "no handler may run once ANY kind is known unfulfillable"
            );
        }

        /// A rejecting `on_sampling_approval` gate reaches the MRTR path — the
        /// fold must not bypass the wallet gate (T-113-57).
        #[tokio::test]
        async fn rejecting_approval_yields_cannot_fulfil() {
            let sample_calls = Arc::new(AtomicUsize::new(0));
            let client = ClientBuilder::new(MockTransport::new())
                .on_sampling(CountingSampling {
                    calls: sample_calls.clone(),
                })
                .on_sampling_approval(|_params| async {
                    host::ApprovalDecision::Deny("policy".to_string())
                })
                .build();

            let map = requests(vec![("k", sampling_request())]);
            assert!(matches!(
                client.fold_input_requests(&map).await,
                FoldOutcome::CannotFulfil
            ));
            assert_eq!(
                sample_calls.load(Ordering::SeqCst),
                0,
                "the preflight approval gate must prevent the LLM call"
            );
        }

        /// The post-generation `on_sampling_result_review` gate also runs on
        /// the MRTR path.
        #[tokio::test]
        async fn result_review_runs_on_a_sampling_result() {
            let reviewed = Arc::new(AtomicUsize::new(0));
            let seen = reviewed.clone();
            let client = ClientBuilder::new(MockTransport::new())
                .on_sampling(CountingSampling {
                    calls: Arc::new(AtomicUsize::new(0)),
                })
                .on_sampling_result_review(move |_params, result| {
                    let seen = seen.clone();
                    async move {
                        assert_eq!(
                            result.model, "test-model",
                            "the reviewer sees the completion"
                        );
                        seen.fetch_add(1, Ordering::SeqCst);
                        host::ApprovalDecision::Deny("no".to_string())
                    }
                })
                .build();

            let map = requests(vec![("k", sampling_request())]);
            assert!(matches!(
                client.fold_input_requests(&map).await,
                FoldOutcome::CannotFulfil
            ));
            assert_eq!(
                reviewed.load(Ordering::SeqCst),
                1,
                "the result review must have run on the MRTR path"
            );
        }

        /// A declined (or cancelled) elicitation is a legitimate v1 answer but
        /// is NOT a fulfilled MRTR input — the client must not resend.
        #[tokio::test]
        async fn declined_elicitation_yields_cannot_fulfil() {
            for action in [ElicitAction::Decline, ElicitAction::Cancel] {
                let client = ClientBuilder::new(MockTransport::new())
                    .on_elicitation(CountingElicitation {
                        calls: Arc::new(AtomicUsize::new(0)),
                        action,
                    })
                    .build();
                let map = requests(vec![("k", elicitation_request())]);
                assert!(
                    matches!(
                        client.fold_input_requests(&map).await,
                        FoldOutcome::CannotFulfil
                    ),
                    "{action:?} must not be treated as a fulfilled input"
                );
            }
        }

        /// A handler that ERRORS yields `CannotFulfil` — never a partial map
        /// and never a fabricated response.
        #[tokio::test]
        async fn handler_error_yields_cannot_fulfil() {
            let client = ClientBuilder::new(MockTransport::new())
                .on_sampling(FailingHostSampling)
                .build();
            let map = requests(vec![("k", sampling_request())]);
            assert!(matches!(
                client.fold_input_requests(&map).await,
                FoldOutcome::CannotFulfil
            ));
        }

        /// An empty map folds to an empty (but fulfilled) response map.
        #[tokio::test]
        async fn an_empty_map_is_trivially_fulfilled() {
            let client = ClientBuilder::new(MockTransport::new()).build();
            let FoldOutcome::Fulfilled(responses) =
                client.fold_input_requests(&InputRequests::new()).await
            else {
                panic!("nothing to fulfil");
            };
            assert!(responses.is_empty());
        }
    }

    // === Typed-helper unit tests ===

    #[tokio::test]
    async fn test_call_tool_typed_serialize_error_maps_to_validation() {
        use serde::Serialize;
        // A type whose Serialize impl always errors.
        struct Bad;
        impl Serialize for Bad {
            fn serialize<S: serde::Serializer>(
                &self,
                _: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("nope"))
            }
        }
        let transport = MockTransport::new();
        let client = Client::new(transport);
        let err = client.call_tool_typed("any", &Bad).await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("call_tool_typed arguments"), "got: {msg}");
        assert!(msg.contains("nope"), "serde error must surface: {msg}");
        assert!(matches!(err, Error::Validation(_)));
    }

    #[tokio::test]
    async fn test_get_prompt_typed_non_object_rejected() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        // Vec<i32> serializes to Value::Array, which is non-object.
        let err = client
            .get_prompt_typed("p", &vec![1, 2, 3])
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must serialize to a JSON object"),
            "got: {msg}"
        );
        assert!(matches!(err, Error::Validation(_)));
    }

    #[tokio::test]
    async fn test_get_prompt_typed_string_values_not_quoted() {
        use serde::Serialize;
        #[derive(Serialize)]
        struct Args {
            topic: String,
            length: u32,
            verbose: bool,
            ignored: Option<String>,
        }
        // Unit-test the coercion directly by building the intermediate HashMap
        // in-situ. Wire round-trip is covered in tests/list_all_pagination.rs;
        // here we only care that the leaf-coercion rules are honoured.
        let args = Args {
            topic: "rust".into(),
            length: 200,
            verbose: true,
            ignored: None,
        };
        let value = serde_json::to_value(&args).unwrap();
        let obj = value.as_object().unwrap().clone();
        assert_eq!(
            obj.get("topic").unwrap(),
            &serde_json::Value::String("rust".into())
        );
        assert_eq!(obj.get("length").unwrap().to_string(), "200");
        assert_eq!(obj.get("verbose").unwrap().to_string(), "true");
        assert!(matches!(
            obj.get("ignored").unwrap(),
            serde_json::Value::Null
        ));
    }

    #[test]
    fn test_client_builder() {
        let transport = MockTransport::new();
        let client = ClientBuilder::new(transport)
            .enforce_strict_capabilities(true)
            .debounced_notifications(vec!["test".to_string()])
            .build();
        assert!(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(client.protocol.read())
                .options()
                .enforce_strict_capabilities
        );
    }

    #[tokio::test]
    async fn test_client_initialization() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        let caps = ClientCapabilities::minimal();

        let result = client.initialize(caps).await;
        assert!(result.is_ok());
        assert!(client.initialized);
        assert_eq!(client.server_version.as_ref().unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_ping() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let ping_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({})),
        });

        let transport = MockTransport::with_responses(vec![ping_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize first
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Ping
        let result = client.ping().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let tools_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "tools": [{
                    "name": "test-tool",
                    "description": "Test tool",
                    "inputSchema": {}
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![tools_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize with tools capability
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // List tools
        let result = client.list_tools(None).await;
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_error_response() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let error_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Error(JSONRPCError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        });

        let transport = MockTransport::with_responses(vec![error_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should get error
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[tokio::test]
    async fn test_uninitialized_error() {
        let transport = MockTransport::new();
        let client = Client::new(transport);

        // Try to call method without initialization
        let result = client.ping().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_capability_enforcement() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    // No tools capability
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize without tools capability
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should fail
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[test]
    fn test_assert_capability_sampling_present_when_server_advertises() {
        // Regression for CR-01: a server advertising `sampling` must satisfy
        // the capability assertion `create_message` performs.
        let mut client = Client::new(MockTransport::new());
        client.server_capabilities = Some(ServerCapabilities {
            sampling: Some(crate::types::SamplingCapabilities::default()),
            ..Default::default()
        });
        assert!(
            client
                .assert_capability("sampling", "sampling/createMessage")
                .is_ok(),
            "sampling capability must be recognized when the server advertises it"
        );
    }

    #[test]
    fn test_assert_capability_sampling_absent_errors() {
        // Negative half of CR-01: no `sampling` advertised => capability error.
        let mut client = Client::new(MockTransport::new());
        client.server_capabilities = Some(ServerCapabilities::default());
        let err = client
            .assert_capability("sampling", "sampling/createMessage")
            .expect_err("missing sampling capability must error");
        assert!(
            err.to_string().contains("does not support sampling"),
            "unexpected error message: {err}"
        );
    }

    /// Transport whose first `send` (the outgoing request) succeeds and whose
    /// second `send` (the host response) fails, returning a single inbound
    /// request from `receive` in between. Drives the WR-04 leak path.
    #[derive(Debug)]
    struct FailSecondSend {
        sends: Arc<Mutex<usize>>,
        inbound: Arc<Mutex<Option<TransportMessage>>>,
    }

    #[async_trait]
    impl Transport for FailSecondSend {
        async fn send(&mut self, _message: TransportMessage) -> Result<()> {
            let mut n = self.sends.lock().unwrap();
            *n += 1;
            if *n >= 2 {
                Err(Error::internal("host response send failed"))
            } else {
                Ok(())
            }
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let msg = self.inbound.lock().unwrap().take();
            if let Some(msg) = msg {
                Ok(msg)
            } else {
                Err(Error::internal("no more messages"))
            }
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_host_response_send_failure_cleans_active_requests() {
        // WR-04: when sending the host response fails, the in-flight request's
        // entry (and its oneshot cancel sender) must be removed from
        // active_requests before the error propagates, matching the Response
        // arm's cleanup.
        let inbound = TransportMessage::Request {
            id: RequestId::from("inbound-1".to_string()),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };
        let client = Client::new(FailSecondSend {
            sends: Arc::new(Mutex::new(0)),
            inbound: Arc::new(Mutex::new(Some(inbound))),
        });

        let req_id = RequestId::from("outgoing-1".to_string());
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let result = client.send_request(req_id.clone(), request).await;

        assert!(
            result.is_err(),
            "host-response send failure must propagate as an error"
        );
        assert!(
            !client.active_requests.read().await.contains_key(&req_id),
            "pending entry must be removed when the host response send fails"
        );
    }

    #[tokio::test]
    async fn test_receive_failure_cleans_active_requests() {
        // WR-04: a failure in the inbound `receive` (before any response
        // arrives) must also funnel through the single exit-cleanup point, so
        // the pending entry does not leak. `FailSecondSend` with no inbound
        // message sends the outgoing request successfully (send #1) and then
        // errors on `receive` ("no more messages").
        let client = Client::new(FailSecondSend {
            sends: Arc::new(Mutex::new(0)),
            inbound: Arc::new(Mutex::new(None)),
        });

        let req_id = RequestId::from("outgoing-recv".to_string());
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let result = client.send_request(req_id.clone(), request).await;

        assert!(
            result.is_err(),
            "transport receive failure must propagate as an error"
        );
        assert!(
            !client.active_requests.read().await.contains_key(&req_id),
            "pending entry must be removed when the transport receive fails"
        );
    }

    #[tokio::test]
    async fn test_send_progress() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Send progress
        let progress = ProgressNotification::new(
            ProgressToken::String("test".to_string()),
            50.0,
            Some("Halfway done".to_string()),
        );

        let result = client.send_progress(progress).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "completions": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let complete_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "completion": {
                    "values": ["test1", "test2"]
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![complete_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Complete
        let result = client
            .complete(CompleteRequest {
                r#ref: crate::types::CompletionReference::Resource {
                    uri: "test://test".to_string(),
                },
                argument: crate::types::CompletionArgument {
                    name: "test".to_string(),
                    value: "t".to_string(),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_resource() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "resources": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let read_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "contents": [{
                    "type": "text",
                    "text": "Hello, world!"
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![read_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // Read resource
        let result = client.read_resource("test://test".to_string()).await;
        if let Err(e) = &result {
            tracing::error!("Read resource error: {:?}", e);
        }
        assert!(result.is_ok());
        let contents = result.unwrap();
        assert_eq!(contents.contents.len(), 1);
    }

    // === list_all_* in-module tests ===

    fn list_all_init_response() -> TransportMessage {
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {},
                    "prompts": {},
                    "resources": {},
                },
                "serverInfo": { "name": "test-server", "version": "1.0.0" }
            })),
        })
    }

    fn page_response<V: Into<serde_json::Value>>(
        id: i64,
        items_field: &str,
        items: V,
        next_cursor: Option<&str>,
    ) -> TransportMessage {
        let mut payload = serde_json::Map::new();
        payload.insert(items_field.to_string(), items.into());
        if let Some(c) = next_cursor {
            payload.insert("nextCursor".to_string(), json!(c));
        }
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(serde_json::Value::Object(payload)),
        })
    }

    #[tokio::test]
    async fn test_list_all_tools_single_page() {
        let page = page_response(
            2,
            "tools",
            json!([{"name": "only", "description": "t", "inputSchema": {}}]),
            None,
        );
        // MockTransport pops from tail; push reversed + init last.
        let transport = MockTransport::with_responses(vec![page, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "only");
    }

    #[tokio::test]
    async fn test_list_all_tools_three_pages_in_order() {
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "tools",
            json!([{"name": "c", "description": "t", "inputSchema": {}}]),
            None,
        );
        // Reverse-push: pages last-to-first, init last.
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_list_all_tools_cap_enforced() {
        // max_iterations=3, server emits 4 pages all with Some(_).
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "tools",
            json!([{"name": "c", "description": "t", "inputSchema": {}}]),
            Some("p4"),
        );
        let p4 = page_response(
            5,
            "tools",
            json!([{"name": "d", "description": "t", "inputSchema": {}}]),
            Some("p5"),
        );
        let transport =
            MockTransport::with_responses(vec![p4, p3, p2, p1, list_all_init_response()]);
        let opts = ClientOptions {
            max_iterations: 3,
            ..Default::default()
        };
        let mut client = Client::with_client_options(transport, opts);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let err = client.list_all_tools().await.unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(err, Error::Validation(_)), "got: {msg}");
        assert!(msg.contains("list_all_tools"), "method name missing: {msg}");
        assert!(msg.contains('3'), "cap value missing: {msg}");
    }

    #[tokio::test]
    async fn test_list_all_tools_empty_string_cursor_continues() {
        // First page has next_cursor: Some("") — MUST continue the loop.
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some(""),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec!["a", "b"],
            "Some(\"\") must continue the loop (Pitfall 2)"
        );
    }

    #[tokio::test]
    async fn test_list_all_tools_max_iterations_zero_errors_immediately() {
        // max_iterations=0: loop body must not execute; no tools/list sent.
        // Only the init response is pre-loaded — if the loop ever called
        // list_tools, receive() would then fail with "No more responses"
        // (a Protocol error), not a Validation error.
        let transport = MockTransport::with_responses(vec![list_all_init_response()]);
        let sent_ref = Arc::clone(&transport.sent_messages);
        let opts = ClientOptions {
            max_iterations: 0,
            ..Default::default()
        };
        let mut client = Client::with_client_options(transport, opts);
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // Snapshot sent count BEFORE the call — initialize() sent its init
        // request. We assert no ADDITIONAL tools/list request is sent.
        let sent_before = sent_ref.lock().unwrap().len();

        let err = client.list_all_tools().await.unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(err, Error::Validation(_)), "got: {msg}");
        assert!(msg.contains('0'), "cap value missing: {msg}");
        assert!(msg.contains("list_all_tools"), "method name missing: {msg}");

        let sent_after = sent_ref.lock().unwrap().clone();
        assert_eq!(
            sent_after.len(),
            sent_before,
            "transport must not receive any tools/list request when max_iterations=0; sent: {sent_after:?}"
        );
    }

    #[tokio::test]
    async fn test_list_all_prompts_three_pages_in_order() {
        let p1 = page_response(2, "prompts", json!([{"name": "p1"}]), Some("p2"));
        let p2 = page_response(3, "prompts", json!([{"name": "p2"}]), Some("p3"));
        let p3 = page_response(3, "prompts", json!([{"name": "p3"}]), None);
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_prompts().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["p1", "p2", "p3"]);
    }

    #[tokio::test]
    async fn test_list_all_resources_three_pages_in_order() {
        let p1 = page_response(
            2,
            "resources",
            json!([{"uri": "file://a", "name": "a"}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "resources",
            json!([{"uri": "file://b", "name": "b"}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "resources",
            json!([{"uri": "file://c", "name": "c"}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_resources().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_list_all_resource_templates_two_pages_in_order() {
        let p1 = page_response(
            2,
            "resourceTemplates",
            json!([{"uriTemplate": "file://{a}", "name": "ta"}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "resourceTemplates",
            json!([{"uriTemplate": "file://{b}", "name": "tb"}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_resource_templates().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["ta", "tb"]);
    }

    // === TASKDX-03: WARN on task deserialize failure ===

    /// A single captured tracing event's structured fields.
    #[derive(Debug, Clone, Default)]
    struct CapturedEvent {
        level: String,
        fields: std::collections::HashMap<String, String>,
        message: String,
    }

    /// Minimal in-test recording subscriber that captures events' structured
    /// fields into a shared `Vec`, with NO dependency on `tracing-subscriber`.
    ///
    /// Installed via `tracing::subscriber::with_default` (scoped, never a global
    /// `init()`), so it cannot leak across tests run under `--test-threads=1`.
    #[derive(Clone, Default)]
    struct RecordingSubscriber {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    struct FieldCollector<'a>(&'a mut CapturedEvent);

    impl tracing::field::Visit for FieldCollector<'_> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            let rendered = format!("{value:?}");
            if field.name() == "message" {
                // Debug-rendered messages are wrapped in quotes; strip them.
                self.0.message = rendered.trim_matches('"').to_string();
            } else {
                self.0.fields.insert(
                    field.name().to_string(),
                    rendered.trim_matches('"').to_string(),
                );
            }
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                self.0.message = value.to_string();
            } else {
                self.0
                    .fields
                    .insert(field.name().to_string(), value.to_string());
            }
        }
    }

    impl tracing::Subscriber for RecordingSubscriber {
        fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            let mut captured = CapturedEvent {
                level: event.metadata().level().to_string(),
                ..Default::default()
            };
            event.record(&mut FieldCollector(&mut captured));
            self.events.lock().unwrap().push(captured);
        }

        fn enter(&self, _span: &tracing::span::Id) {}
        fn exit(&self, _span: &tracing::span::Id) {}
    }

    /// Build an init response advertising the `tasks` capability.
    fn tasks_init_response() -> TransportMessage {
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tasks": {} },
                "serverInfo": { "name": "test-server", "version": "1.0.0" }
            })),
        })
    }

    #[test]
    fn test_tasks_get_malformed_response_emits_warn_and_errs() {
        // A flat Task missing the required `task` wrapper — the deliberately
        // wrong shape for GetTaskResult (incident bug #3).
        let malformed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "taskId": "abc",
                "status": "completed"
            })),
        });
        let transport = MockTransport::with_responses(vec![malformed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_get("abc"))
        });

        // Control flow unchanged: still returns Err (a parse error).
        assert!(result.is_err(), "malformed tasks/get must still return Err");

        // Structural WARN assertion (not a substring of the message text).
        let events = sink.lock().unwrap();
        let warn = events
            .iter()
            .find(|e| e.fields.get("method").map(String::as_str) == Some("tasks/get"))
            .expect("a WARN naming method=tasks/get must be captured");
        assert_eq!(warn.level, "WARN", "must be a WARN level event");
        assert!(
            warn.fields.contains_key("error"),
            "WARN must carry the serde error field, got: {:?}",
            warn.fields
        );
        assert!(
            warn.fields.contains_key("transport"),
            "WARN must carry the transport identity field, got: {:?}",
            warn.fields
        );
    }

    #[test]
    fn test_tasks_result_malformed_response_emits_warn_and_errs() {
        // CallToolResult requires `content`; a bare bool is the wrong shape.
        let malformed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!(true)),
        });
        let transport = MockTransport::with_responses(vec![malformed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_result("abc"))
        });

        assert!(
            result.is_err(),
            "malformed tasks/result must still return Err"
        );

        let events = sink.lock().unwrap();
        let warn = events
            .iter()
            .find(|e| e.fields.get("method").map(String::as_str) == Some("tasks/result"))
            .expect("a WARN naming method=tasks/result must be captured");
        assert_eq!(warn.level, "WARN");
        assert!(warn.fields.contains_key("error"), "got: {:?}", warn.fields);
        assert!(
            warn.fields.contains_key("transport"),
            "got: {:?}",
            warn.fields
        );
    }

    #[test]
    fn test_tasks_get_well_formed_response_emits_no_warn() {
        let well_formed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "task": {
                    "taskId": "abc",
                    "status": "completed",
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z"
                }
            })),
        });
        let transport = MockTransport::with_responses(vec![well_formed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_get("abc"))
        });

        assert!(
            result.is_ok(),
            "well-formed tasks/get must succeed: {result:?}"
        );
        let events = sink.lock().unwrap();
        assert!(
            !events
                .iter()
                .any(|e| e.fields.get("method").map(String::as_str) == Some("tasks/get")),
            "no task-deserialize WARN must fire on a well-formed response"
        );
    }

    // =======================================================================
    // Phase 113 / CLNT-01 — the v2 (`2026-07-28`) client era.
    // =======================================================================

    mod v2_era {
        use super::*;
        use crate::types::protocol::{
            Era, ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
        };
        use crate::types::ServerCapabilities;

        /// A transport that RECORDS the mode-propagation seam calls and every
        /// raw frame, so the wiring can be asserted without a socket.
        #[derive(Debug, Default, Clone)]
        struct ModeRecordingTransport {
            mode_calls: Arc<Mutex<Vec<Option<String>>>>,
            raw_bodies: Arc<Mutex<Vec<Vec<u8>>>>,
            typed_sends: Arc<Mutex<usize>>,
            supports_mode: bool,
        }

        impl ModeRecordingTransport {
            fn http_like() -> Self {
                Self {
                    supports_mode: true,
                    ..Self::default()
                }
            }
        }

        #[async_trait]
        impl Transport for ModeRecordingTransport {
            async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                *self.typed_sends.lock().unwrap() += 1;
                Ok(())
            }

            async fn receive(&mut self) -> Result<TransportMessage> {
                Err(Error::protocol_msg("no responses"))
            }

            async fn close(&mut self) -> Result<()> {
                Ok(())
            }

            fn transport_type(&self) -> &'static str {
                "mode-recording"
            }

            fn set_negotiated_protocol_version(&mut self, version: Option<String>) {
                self.mode_calls.lock().unwrap().push(version);
            }

            fn supports_negotiated_protocol_version(&self) -> bool {
                self.supports_mode
            }

            async fn send_raw(&mut self, body: Vec<u8>) -> Result<()> {
                self.raw_bodies.lock().unwrap().push(body);
                Ok(())
            }
        }

        fn v2() -> ProtocolVersion {
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string())
        }

        fn v2_client() -> Client<ModeRecordingTransport> {
            ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_protocol_version(v2())
                .expect("2026-07-28 is selectable")
                .build()
        }

        // ---- the propagation seam -----------------------------------------

        #[test]
        fn v2_selection_reaches_the_transport_exactly_once() {
            let transport = ModeRecordingTransport::http_like();
            let calls = transport.mode_calls.clone();
            let client = ClientBuilder::new(transport)
                .with_protocol_version(v2())
                .expect("2026-07-28 is selectable")
                .build();

            let calls = calls.lock().unwrap();
            assert_eq!(
                &*calls,
                &[Some(PROTOCOL_VERSION_2026_07_28.to_string())],
                "the selected version must cross into the transport exactly once"
            );
            assert_eq!(client.era(), Era::V2);
        }

        #[test]
        fn a_non_opted_in_build_never_touches_the_transport_mode_seam() {
            let transport = ModeRecordingTransport::http_like();
            let calls = transport.mode_calls.clone();
            let client = ClientBuilder::new(transport).build();

            assert!(
                calls.lock().unwrap().is_empty(),
                "a client that never called with_protocol_version must be byte-identical to today"
            );
            assert_eq!(client.era(), Era::V1);
            assert!(!client.initialized, "v1 still requires the handshake");
        }

        #[test]
        fn a_v2_client_is_ready_without_a_handshake() {
            assert!(
                v2_client().initialized,
                "v2 has no initialize, so the client is ready on construction"
            );
        }

        // ---- version validation (T-113-52) --------------------------------

        #[test]
        fn with_protocol_version_accepts_the_two_documented_versions() {
            for accepted in [PROTOCOL_VERSION_2026_07_28, LATEST_PROTOCOL_VERSION] {
                assert!(
                    ClientBuilder::new(MockTransport::new())
                        .with_protocol_version(ProtocolVersion(accepted.to_string()))
                        .is_ok(),
                    "{accepted} must be selectable"
                );
            }
        }

        #[test]
        fn with_protocol_version_rejects_an_unsupported_version() {
            let error = ClientBuilder::new(MockTransport::new())
                .with_protocol_version(ProtocolVersion("1999-01-01".to_string()))
                .expect_err("an unknown version must be rejected, never silently emitted");
            let rendered = error.to_string();
            assert!(
                rendered.contains("1999-01-01") && rendered.contains(PROTOCOL_VERSION_2026_07_28),
                "the error must name both the offending and the accepted values: {rendered}"
            );
        }

        // ---- reserved `_meta` emission -------------------------------------

        /// The literal wire spellings, restated here on purpose: this test is the
        /// drift guard between the client emitter and the server resolver.
        #[test]
        fn v2_meta_carries_exactly_the_three_reserved_keys() {
            let client = v2_client();
            let mut params = Some(json!({}));
            client.splice_v2_meta(&mut params);

            let meta = params.as_ref().unwrap()["_meta"].clone();
            assert_eq!(
                meta["io.modelcontextprotocol/protocolVersion"],
                "2026-07-28"
            );
            assert_eq!(
                meta["io.modelcontextprotocol/clientInfo"]["name"],
                "pmcp-client"
            );
            assert!(meta
                .get("io.modelcontextprotocol/clientCapabilities")
                .is_some());
        }

        #[test]
        fn v2_meta_injection_preserves_caller_trace_context() {
            let client = v2_client();
            let mut params = Some(json!({
                "name": "search",
                "_meta": { "traceparent": "00-abc-def-01", "progressToken": 7 },
            }));
            client.splice_v2_meta(&mut params);

            let params = params.unwrap();
            assert_eq!(params["_meta"]["traceparent"], "00-abc-def-01");
            assert_eq!(params["_meta"]["progressToken"], 7);
            assert_eq!(
                params["_meta"]["io.modelcontextprotocol/protocolVersion"],
                "2026-07-28"
            );
            assert_eq!(params["name"], "search", "sibling params are untouched");
        }

        #[test]
        fn v2_meta_injection_creates_params_when_absent() {
            let client = v2_client();
            let mut params = None;
            client.splice_v2_meta(&mut params);
            assert_eq!(
                params.unwrap()["_meta"]["io.modelcontextprotocol/protocolVersion"],
                "2026-07-28"
            );
        }

        // ---- extension declaration (Phase 114, D-04) -----------------------

        /// The declared `extensions` map reaches the per-request `_meta`.
        ///
        /// Asserted as EQUALITY with `{}` rather than presence: a presence-only
        /// check passes on precisely the regression that would matter (a value
        /// that is `null`, `true`, or a populated settings object none of which
        /// the draft schema admits).
        #[test]
        fn a_declaring_v2_client_emits_the_tasks_extension_on_every_request() {
            let client = ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_protocol_version(v2())
                .expect("2026-07-28 is selectable")
                .with_tasks_extension()
                .build();

            // TWO different requests, because the v2 declaration is per-request
            // and a mechanism that only stamped the first would still pass a
            // single-frame assertion.
            for params in [json!({}), json!({ "name": "search" })] {
                let mut params = Some(params);
                client.splice_v2_meta(&mut params);
                let declared = params.as_ref().unwrap()["_meta"]
                    ["io.modelcontextprotocol/clientCapabilities"]["extensions"]
                    [crate::types::capabilities::TASKS_EXTENSION_KEY]
                    .clone();
                assert_eq!(
                    declared,
                    json!({}),
                    "the tasks extension must be declared as EXACTLY {{}}"
                );
            }
        }

        /// Absence is asserted as key ABSENCE, never as a falsy value.
        ///
        /// `ClientCapabilities::extensions` carries
        /// `skip_serializing_if = "Option::is_none"`, so a regression that
        /// started emitting `"extensions": null` would satisfy any check written
        /// against the VALUE.
        #[test]
        fn a_non_declaring_v2_client_emits_no_extensions_key_at_all() {
            let client = v2_client();
            let mut params = Some(json!({}));
            client.splice_v2_meta(&mut params);

            let capabilities = params.as_ref().unwrap()["_meta"]
                ["io.modelcontextprotocol/clientCapabilities"]
                .clone();
            assert!(
                capabilities.get("extensions").is_none(),
                "a client that never opted in must emit NO extensions key, got {capabilities}"
            );
        }

        /// The declaration is threaded through the emission that SERIALIZES
        /// `ClientCapabilities`, not through a hand-built `json!` object.
        ///
        /// If `v2_request_meta` ever hand-builds the capabilities value, this
        /// test fails: a field added to `ClientCapabilities` would then be
        /// invisible on the wire, which is exactly how the client and the
        /// server's `ProtocolContext::client_capabilities` come to disagree.
        #[test]
        fn the_emitted_capabilities_deserialize_back_into_client_capabilities() {
            let client = ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_protocol_version(v2())
                .expect("2026-07-28 is selectable")
                .with_tasks_extension()
                .build();
            let mut params = Some(json!({}));
            client.splice_v2_meta(&mut params);

            let raw = params.as_ref().unwrap()["_meta"]
                ["io.modelcontextprotocol/clientCapabilities"]
                .clone();
            let round_tripped: ClientCapabilities =
                serde_json::from_value(raw).expect("the emitted value IS a ClientCapabilities");
            assert_eq!(
                round_tripped
                    .extensions
                    .as_ref()
                    .and_then(|e| e.get(crate::types::capabilities::TASKS_EXTENSION_KEY)),
                Some(&json!({})),
            );
        }

        /// A v1 client's `initialize` bytes do not move (Phase-114 D-02).
        #[test]
        fn the_declaration_never_reaches_a_v1_initialize() {
            let client = ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_tasks_extension()
                .build();
            assert_eq!(client.era(), Era::V1);

            // v1 advertises the CALLER's capabilities verbatim (modulo the
            // registry-derived host fields), so the declaration is inert.
            let mut capabilities = ClientCapabilities::default();
            client.derive_host_capabilities(&mut capabilities);
            let serialized = serde_json::to_string(&capabilities).expect("serializes");
            assert!(
                !serialized.contains("extensions"),
                "a v1 initialize must carry no extensions key, got {serialized}"
            );
        }

        // ---- capability honesty (T-113-12) ---------------------------------

        struct NoopElicitation;

        #[async_trait]
        impl host::HostElicitationHandler for NoopElicitation {
            async fn handle_elicitation(
                &self,
                _params: crate::types::elicitation::ElicitRequestParams,
            ) -> Result<crate::types::elicitation::ElicitResult> {
                Ok(crate::types::elicitation::ElicitResult {
                    action: crate::types::elicitation::ElicitAction::Cancel,
                    content: None,
                })
            }
        }

        #[test]
        fn client_capabilities_are_empty_for_an_empty_registry() {
            let capabilities = v2_client().v2_client_capabilities();
            let value = serde_json::to_value(capabilities).unwrap();
            assert_eq!(
                value,
                json!({}),
                "a client with no host handlers must claim nothing"
            );
        }

        #[test]
        fn client_capabilities_declare_elicitation_once_registered() {
            let client = ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_protocol_version(v2())
                .expect("selectable")
                .on_elicitation(NoopElicitation)
                .build();
            let value = serde_json::to_value(client.v2_client_capabilities()).unwrap();
            assert!(value.get("elicitation").is_some(), "got {value}");
            assert!(value.get("sampling").is_none(), "must not over-claim");
            assert!(value.get("roots").is_none(), "must not over-claim");
        }

        #[test]
        fn client_capabilities_declare_sampling_for_the_with_tools_handler() {
            struct ToolAwareHost;
            #[async_trait]
            impl host::HostSamplingHandlerWithTools for ToolAwareHost {
                async fn handle_create_message_with_tools(
                    &self,
                    _params: crate::types::sampling::CreateMessageParams,
                ) -> Result<crate::types::sampling::CreateMessageResultWithTools> {
                    Err(Error::internal("unused"))
                }
            }

            let client = ClientBuilder::new(ModeRecordingTransport::http_like())
                .with_protocol_version(v2())
                .expect("selectable")
                .on_sampling_with_tools(ToolAwareHost)
                .build();
            let value = serde_json::to_value(client.v2_client_capabilities()).unwrap();
            assert!(
                value.get("sampling").is_some(),
                "a WithTools-only client CAN service sampling and must say so: {value}"
            );
        }

        // ---- era-aware capability enforcement ------------------------------

        #[test]
        fn v2_without_discovery_does_not_block_locally() {
            let client = v2_client();
            assert!(client.server_capabilities.is_none());
            assert!(
                client.assert_capability("tools", "tools/call").is_ok(),
                "a v2 client never learned capabilities — the server is the authority"
            );
        }

        #[test]
        fn v1_without_capabilities_still_fails_closed() {
            let client = ClientBuilder::new(MockTransport::new()).build();
            assert!(
                client.assert_capability("tools", "tools/call").is_err(),
                "v1 enforcement must be unchanged"
            );
        }

        #[test]
        fn v2_enforces_once_discovery_has_stored_a_projection() {
            let mut client = v2_client();
            client.server_capabilities = Some(ServerCapabilities::default());
            assert!(
                client.assert_capability("tools", "tools/call").is_err(),
                "after server_discover stored a projection, v2 is as strict as v1"
            );

            client.server_capabilities = Some(ServerCapabilities::tools_only());
            assert!(client.assert_capability("tools", "tools/call").is_ok());
        }

        // ---- tasks negotiation, era-split (Phase 114, D-04) -----------------

        /// A `ServerCapabilities` advertising the tasks extension the v2 way.
        fn v2_tasks_capabilities() -> ServerCapabilities {
            let mut extensions = HashMap::new();
            extensions.insert(
                crate::types::capabilities::TASKS_EXTENSION_KEY.to_string(),
                json!({}),
            );
            ServerCapabilities {
                extensions: Some(extensions),
                ..ServerCapabilities::default()
            }
        }

        #[test]
        fn v2_tasks_capability_is_satisfied_by_the_extensions_entry() {
            let mut client = v2_client();
            client.server_capabilities = Some(v2_tasks_capabilities());
            assert!(
                client.assert_capability("tasks", "tasks/get").is_ok(),
                "a v2 server advertising the extension must be callable"
            );
        }

        /// The refusal NAMES the key, or the remedy is undiscoverable.
        #[test]
        fn v2_tasks_capability_is_refused_when_the_extensions_entry_is_absent() {
            let mut client = v2_client();
            client.server_capabilities = Some(ServerCapabilities::default());
            let error = client
                .assert_capability("tasks", "tasks/get")
                .expect_err("a v2 server that did not advertise the extension must be refused");
            let rendered = error.to_string();
            assert!(
                rendered.contains(crate::types::capabilities::TASKS_EXTENSION_KEY),
                "the refusal must name the extension key: {rendered}"
            );
        }

        /// The v1 field is NOT the v2 signal.
        ///
        /// Non-vacuity guard for the arm above: a v2 server projects
        /// `capabilities.tasks` away, so an implementation that kept reading it
        /// would refuse every conformant v2 server. This is the fixture that
        /// would pass under the old arm and must now fail.
        #[test]
        fn v2_tasks_capability_ignores_the_v1_tasks_field() {
            let mut client = v2_client();
            client.server_capabilities = Some(ServerCapabilities {
                tasks: Some(crate::types::capabilities::ServerTasksCapability::default()),
                ..ServerCapabilities::default()
            });
            assert!(
                client.assert_capability("tasks", "tasks/get").is_err(),
                "capabilities.tasks is the v1 spelling and must not satisfy v2"
            );
        }

        /// The escape hatch is NOT narrowed for tasks.
        #[test]
        fn v2_tasks_capability_passes_without_a_stored_projection() {
            let client = v2_client();
            assert!(client.server_capabilities.is_none());
            assert!(
                client.assert_capability("tasks", "tasks/get").is_ok(),
                "a v2 client that never called server_discover has no basis to refuse"
            );
        }

        /// v1 still gates on `capabilities.tasks`, in BOTH directions.
        #[test]
        fn v1_tasks_capability_still_gates_on_the_tasks_field() {
            let mut client = ClientBuilder::new(MockTransport::new()).build();
            client.server_capabilities = Some(ServerCapabilities::default());
            assert!(
                client.assert_capability("tasks", "tasks/get").is_err(),
                "v1 with no tasks field must still fail closed"
            );

            client.server_capabilities = Some(ServerCapabilities {
                tasks: Some(crate::types::capabilities::ServerTasksCapability::default()),
                ..ServerCapabilities::default()
            });
            assert!(
                client.assert_capability("tasks", "tasks/get").is_ok(),
                "v1 behaviour is untouched"
            );

            // And the v2 spelling must NOT satisfy v1 — the two tables stay
            // era-separated in both directions.
            client.server_capabilities = Some(v2_tasks_capabilities());
            assert!(
                client.assert_capability("tasks", "tasks/get").is_err(),
                "an extensions entry is the v2 spelling and must not satisfy v1"
            );
        }

        /// "Fails fast" is MEASURED, not assumed: zero bytes leave the process.
        #[test]
        fn an_un_negotiated_v2_tasks_call_sends_nothing() {
            let transport = ModeRecordingTransport::http_like();
            let typed = transport.typed_sends.clone();
            let raw = transport.raw_bodies.clone();
            let mut client = ClientBuilder::new(transport)
                .with_protocol_version(v2())
                .expect("selectable")
                .with_tasks_extension()
                .build();
            client.server_capabilities = Some(ServerCapabilities::default());

            let error = futures::executor::block_on(client.tasks_get("task-1"))
                .expect_err("an un-negotiated tasks call must be refused locally");
            assert!(error
                .to_string()
                .contains(crate::types::capabilities::TASKS_EXTENSION_KEY));
            assert_eq!(
                *typed.lock().unwrap(),
                0,
                "the refusal must precede the round trip"
            );
            assert!(
                raw.lock().unwrap().is_empty(),
                "the refusal must precede the round trip"
            );
        }

        // ---- no handshake on the wire ---------------------------------------

        #[test]
        fn initialize_on_v2_sends_nothing() {
            let transport = ModeRecordingTransport::http_like();
            let typed = transport.typed_sends.clone();
            let raw = transport.raw_bodies.clone();
            let mut client = ClientBuilder::new(transport)
                .with_protocol_version(v2())
                .expect("selectable")
                .build();

            let result =
                futures::executor::block_on(client.initialize(ClientCapabilities::default()))
                    .expect("v2 initialize is a local no-op");

            assert_eq!(
                result.protocol_version.as_str(),
                PROTOCOL_VERSION_2026_07_28
            );
            assert_eq!(*typed.lock().unwrap(), 0, "no typed frame may be sent");
            assert!(
                raw.lock().unwrap().is_empty(),
                "no initialize and no notifications/initialized on v2"
            );
        }

        #[test]
        fn a_v2_request_travels_as_a_raw_frame_carrying_meta() {
            let transport = ModeRecordingTransport::http_like();
            let raw = transport.raw_bodies.clone();
            let typed = transport.typed_sends.clone();
            let client = ClientBuilder::new(transport)
                .with_protocol_version(v2())
                .expect("selectable")
                .build();

            // `receive` errors, so the call fails AFTER the frame was sent —
            // which is exactly the observation this test wants.
            let _ = futures::executor::block_on(client.list_tools(None));

            assert_eq!(*typed.lock().unwrap(), 0, "v2 never uses the typed path");
            let bodies = raw.lock().unwrap();
            let body: serde_json::Value =
                serde_json::from_slice(&bodies[0]).expect("valid JSON frame");
            assert_eq!(body["method"], "tools/list");
            assert_eq!(
                body["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"], "2026-07-28",
                "tools/list has no typed _meta field, so the raw frame is the only era channel"
            );
        }

        #[test]
        fn server_discover_is_refused_on_v1() {
            let mut client = ClientBuilder::new(MockTransport::new()).build();
            let error = futures::executor::block_on(client.server_discover())
                .expect_err("server/discover does not exist on v1");
            assert!(error.to_string().contains("2026-07-28"), "{error}");
        }

        // ---- the retired subscription RPCs (Phase 113-13, HTTP-04) ---------

        /// A v2 `subscribe_resource` fails LOCALLY: nothing reaches the wire.
        ///
        /// The send counters are the proof. The typed counter is `0` because v2
        /// never uses the typed path, and the RAW counter is `0` because the
        /// gate returns before `dispatch_request` is ever reached — an
        /// un-gated method WOULD have pushed a body there (see
        /// `a_v2_request_travels_as_a_raw_frame_carrying_meta` above).
        #[test]
        fn v2_subscribe_resource_is_retired_and_sends_nothing() {
            for method in ["resources/subscribe", "resources/unsubscribe"] {
                let transport = ModeRecordingTransport::http_like();
                let raw = transport.raw_bodies.clone();
                let typed = transport.typed_sends.clone();
                let client = ClientBuilder::new(transport)
                    .with_protocol_version(v2())
                    .expect("selectable")
                    .build();

                let uri = "mem://greeting".to_string();
                let error = if method == "resources/subscribe" {
                    futures::executor::block_on(client.subscribe_resource(uri))
                } else {
                    futures::executor::block_on(client.unsubscribe_resource(uri))
                }
                .expect_err("the method is gone from the 2026-07-28 schema");

                assert!(error.is_retired_on_v2(), "{method}: {error}");
                assert_eq!(error.retired_method(), Some(method));
                assert!(
                    error.to_string().contains("subscriptions/listen"),
                    "{method}: the error names the replacement: {error}"
                );
                assert_eq!(
                    raw.lock().unwrap().len(),
                    0,
                    "{method}: NO raw frame may reach the transport"
                );
                assert_eq!(
                    *typed.lock().unwrap(),
                    0,
                    "{method}: NO typed frame may reach the transport either"
                );
            }
        }

        /// A v1 `subscribe_resource` is byte-identical to today: it still sends
        /// exactly one typed request, with the same capability assertion.
        #[test]
        fn v1_subscribe_resource_still_sends_exactly_one_request() {
            for method in ["resources/subscribe", "resources/unsubscribe"] {
                let transport = ModeRecordingTransport::default();
                let raw = transport.raw_bodies.clone();
                let typed = transport.typed_sends.clone();
                let mut client = ClientBuilder::new(transport).build();

                // A v1 client learns capabilities from `initialize`; short-circuit
                // that here so the capability assertion passes and the send path
                // is reached.
                client.initialized = true;
                client.server_capabilities = Some(ServerCapabilities {
                    resources: Some(crate::types::ResourceCapabilities {
                        subscribe: Some(true),
                        list_changed: Some(true),
                    }),
                    ..ServerCapabilities::default()
                });

                let uri = "mem://greeting".to_string();
                // `receive` errors, so the call fails AFTER the send — which is
                // exactly the observation this test wants.
                let result = if method == "resources/subscribe" {
                    futures::executor::block_on(client.subscribe_resource(uri))
                } else {
                    futures::executor::block_on(client.unsubscribe_resource(uri))
                };

                assert!(
                    !result.as_ref().err().is_some_and(Error::is_retired_on_v2),
                    "{method}: v1 must NOT be gated: {result:?}"
                );
                assert_eq!(
                    *typed.lock().unwrap(),
                    1,
                    "{method}: v1 still sends exactly one typed request"
                );
                assert!(
                    raw.lock().unwrap().is_empty(),
                    "{method}: v1 never uses the raw path"
                );
            }
        }
    }
}
