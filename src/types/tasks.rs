//! MCP Task protocol types (2025-11-25).
//!
//! This module contains the wire types for MCP Tasks as defined
//! in the 2025-11-25 protocol version.

use serde::{Deserialize, Serialize};

/// Related task metadata key per MCP spec.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Task status (5-value enum).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being worked on
    #[default]
    Working,
    /// Task requires user input to continue
    InputRequired,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Working => write!(f, "working"),
            Self::InputRequired => write!(f, "input_required"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TaskStatus {
    /// Returns `true` if this status is terminal (no further transitions allowed).
    ///
    /// Terminal states are `Completed`, `Failed`, and `Cancelled`.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` if transitioning from this status to `next` is valid.
    ///
    /// The MCP spec defines these valid transitions:
    /// - `Working` -> `InputRequired`, `Completed`, `Failed`, `Cancelled`
    /// - `InputRequired` -> `Working`, `Completed`, `Failed`, `Cancelled`
    /// - Terminal states -> no transitions allowed
    ///
    /// Self-transitions (e.g., `Working` -> `Working`) are rejected per spec.
    pub fn can_transition_to(&self, next: &Self) -> bool {
        if self == next {
            return false;
        }

        match self {
            Self::Working => matches!(
                next,
                Self::InputRequired | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::InputRequired => matches!(
                next,
                Self::Working | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }
}

/// The classification of a polled [`Task`], derived purely from its
/// already-deserialized [`TaskStatus`] — the single decision primitive every
/// task poller consumes so the branch logic cannot drift.
///
/// Produced by [`Task::poll_decision`]. It answers the one question a poll loop
/// asks each tick: *stop, ask the user, or sleep and poll again?* — without a
/// network round-trip, a [`CallToolResult`](crate::types::CallToolResult)
/// fetch, or any I/O. It is a pure, replay-deterministic function of the polled
/// `Task`, so it is safe to call inside a memoized durable/replay step (D-01,
/// D-03).
///
/// # Non-exhaustive
///
/// This enum is `#[non_exhaustive]` for future-proofing (D-04): adding a variant
/// later is a non-breaking change, so external `match` sites must carry a
/// wildcard `_ =>` arm. This is a distinct claim from [`TaskStatus`], which is
/// deliberately **exhaustive** today (D-15): `#[non_exhaustive]` here does NOT
/// imply that unknown or future wire statuses are handled gracefully at runtime.
/// An unknown status fails at serde deserialization during `tasks/get`, BEFORE
/// classification ever runs — `poll_decision()` only ever sees one of the five
/// known `TaskStatus` values.
///
/// Unlike every other type in this module, `TaskPollDecision` intentionally
/// derives neither `Serialize` nor `Deserialize`: it is a returned classifier
/// value consumed in-process, never a wire type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskPollDecision {
    /// The task has reached a terminal status ([`TaskStatus::Completed`],
    /// [`TaskStatus::Failed`], or [`TaskStatus::Cancelled`]) and will not
    /// transition further — the poll loop should stop.
    ///
    /// The classifier carries the terminal [`TaskStatus`] by value (it is
    /// `Copy`) but deliberately does NOT carry the final
    /// [`CallToolResult`](crate::types::CallToolResult): to retrieve the
    /// result the caller issues a **second call** (D-06/D-16). This keeps
    /// classification a pure, I/O-free decision.
    ///
    /// Which second call is ERA-DEPENDENT: on v1 (2025-11-25) it is
    /// `tasks/result`; on v2 (2026-07-28) that method is retired and the
    /// terminal `tasks/get` already inlines `result` / `error` on the
    /// [`TaskDetailV2`] body, so the follow-up is another `tasks/get`.
    Terminal {
        /// The terminal status that ended the task.
        status: TaskStatus,
    },
    /// The task is still running ([`TaskStatus::Working`]) — the poll loop
    /// should sleep and poll again.
    ///
    /// `poll_hint` is the raw server-reported `pollInterval` in **milliseconds**
    /// ([`Task::poll_interval`]), passed through verbatim including `None`
    /// (D-07). Feed it to [`resolve_poll_interval`] to obtain the concrete sleep
    /// duration honoring the caller override, this hint, the default, and the
    /// hot-loop floor.
    InProgress {
        /// The raw server-reported `pollInterval` in ms, verbatim (`None` when
        /// the server suggested no interval).
        poll_hint: Option<u64>,
    },
    /// The task is blocked on user input ([`TaskStatus::InputRequired`]).
    ///
    /// This is a unit variant carrying no payload (D-05): the caller already
    /// holds the polled `Task`, and a blocking poller cannot supply the input,
    /// so it must route to elicitation rather than continue spinning.
    InputRequired,
}

/// The default poll interval, in **milliseconds**, used when neither the caller
/// nor the server-reported `pollInterval` specifies one.
///
/// This is a **stable, supported public default** — the documented fallback in
/// the poll-interval policy (1000 ms), not an internal tunable. Its value is a
/// public API contract: changing it is a semver-relevant change, not a silent
/// implementation detail. It is the single source of truth shared by
/// [`resolve_poll_interval`] and every task poller (D-08).
pub const DEFAULT_POLL_MS: u64 = 1000;

/// The floor, in **milliseconds**, applied to any resolved poll interval so a
/// zero or very small value cannot hot-spin the poll loop.
///
/// This is a **stable, supported public default** — the documented 50 ms
/// hot-loop-protection floor in the poll-interval policy, not an internal
/// tunable. Its value is a public API contract: changing it is a
/// semver-relevant change. It is the single source of truth shared by
/// [`resolve_poll_interval`] and the budget clamp in blocking pollers (D-08,
/// T-105-01 mitigation).
pub const MIN_POLL_MS: u64 = 50;

/// Resolve the concrete poll interval, in **milliseconds**, from a caller
/// override and a server-reported hint, applying the documented precedence and
/// the hot-loop-protection floor.
///
/// Precedence: `caller_override` wins if present, else the server `hint`, else
/// [`DEFAULT_POLL_MS`] (1000 ms); the result is then floored to at least
/// [`MIN_POLL_MS`] (50 ms) so a zero or tiny value cannot busy-spin the poll
/// loop (T-105-01 mitigation). This is the single source of truth for interval
/// resolution, consumed by every task poller so the policy cannot drift (D-08).
///
/// Returns `u64` milliseconds — NOT a [`Duration`](std::time::Duration) — to
/// stay symmetric with the `Option<u64>` inputs and consistent with
/// [`Task::poll_interval`] and [`TaskMetadata::poll_interval`] (D-12). Callers
/// wrap with `Duration::from_millis` at the sleep site.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::tasks::resolve_poll_interval;
///
/// // Caller override always wins.
/// assert_eq!(resolve_poll_interval(Some(200), Some(999)), 200);
/// // Server hint used when there is no override.
/// assert_eq!(resolve_poll_interval(None, Some(300)), 300);
/// // Falls back to the 1000 ms default when neither is set.
/// assert_eq!(resolve_poll_interval(None, None), 1000);
/// // A zero (or tiny) value is floored to 50 ms so it cannot hot-spin.
/// assert_eq!(resolve_poll_interval(Some(0), None), 50);
/// ```
pub fn resolve_poll_interval(caller_override: Option<u64>, hint: Option<u64>) -> u64 {
    caller_override
        .or(hint)
        .unwrap_or(DEFAULT_POLL_MS)
        .max(MIN_POLL_MS)
}

/// A task resource representing an in-progress or completed operation.
///
/// # This is the **v1 (2025-11-25)** wire shape
///
/// `Task` is the storage-and-v1-wire type. The v2 (2026-07-28)
/// `io.modelcontextprotocol/tasks` extension renames two of its fields
/// (`ttl` → `ttlMs`, `pollInterval` → `pollIntervalMs`) and makes `ttlMs`
/// required-and-nullable, so it has its own projection type: [`TaskV2`],
/// produced by [`TaskV2::from_v1`] — the ONLY site where those renames happen.
/// The richer per-status v2 variants live on [`TaskDetailV2`].
///
/// Serializing a `Task` onto a v2 response is a schema-invalid answer. Project
/// it first.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::tasks::{Task, TaskStatus};
///
/// let task = Task::new("t-123", TaskStatus::Working)
///     .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
///     .with_ttl(60000)
///     .with_poll_interval(5000)
///     .with_status_message("Processing...");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique task identifier
    pub task_id: String,
    /// Current task status
    pub status: TaskStatus,
    /// Time-to-live in milliseconds. Required but nullable per MCP spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
    pub ttl: Option<u64>,
    /// ISO 8601 creation timestamp
    pub created_at: String,
    /// ISO 8601 last-updated timestamp
    pub last_updated_at: String,
    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    /// Human-readable status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// Full operator-facing diagnostic detail (step ids, URLs, internal error
    /// text) for this task.
    ///
    /// **PMCP EXTENSION — not an MCP-spec field (D-17).** The MCP `Task` type
    /// carries a single [`status_message`](Task::status_message) voice, which
    /// PMCP treats as the business-friendly, user-facing voice (D-17/D-18).
    /// `diagnostic_detail` is a second, separate voice this SDK adds for
    /// operator/developer consumption — full detail that would be
    /// inappropriate to show a business user by default (see the
    /// Information-Disclosure disposition on this field: producers MUST
    /// redact secrets/tokens before setting it). Consuming UIs typically
    /// render it behind an expandable "details" affordance.
    ///
    /// Because `Task` has no `deny_unknown_fields` (see the module's serde
    /// round-trip and consumer-tolerance tests), existing/strict consumers
    /// that don't know this field simply ignore the extra `diagnosticDetail`
    /// key on the wire — this is additive and non-breaking. When absent
    /// (`None`), it is skip-serialized, so callers that never set it produce
    /// byte-identical JSON to before this field existed.
    ///
    /// **Future migration note:** if/when the MCP spec grows a `_meta`
    /// extension slot on `Task` (mirroring `CallToolResult._meta` /
    /// [`RELATED_TASK_META_KEY`]), this field is a candidate to migrate under
    /// that slot instead of a top-level struct field, to keep `Task` itself
    /// spec-pure. Until then, this is the pragmatic wire slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_detail: Option<String>,
}

impl Task {
    /// Create a task with the given ID and status.
    ///
    /// Timestamps default to empty strings (caller should set via
    /// [`Task::with_timestamps`]). Optional fields default to `None`.
    pub fn new(task_id: impl Into<String>, status: TaskStatus) -> Self {
        Self {
            task_id: task_id.into(),
            status,
            ttl: None,
            created_at: String::new(),
            last_updated_at: String::new(),
            poll_interval: None,
            status_message: None,
            diagnostic_detail: None,
        }
    }

    /// Set the time-to-live in milliseconds.
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set both creation and last-updated timestamps.
    pub fn with_timestamps(
        mut self,
        created_at: impl Into<String>,
        last_updated_at: impl Into<String>,
    ) -> Self {
        self.created_at = created_at.into();
        self.last_updated_at = last_updated_at.into();
        self
    }

    /// Set the suggested polling interval in milliseconds.
    pub fn with_poll_interval(mut self, interval: u64) -> Self {
        self.poll_interval = Some(interval);
        self
    }

    /// Set a human-readable status message.
    pub fn with_status_message(mut self, message: impl Into<String>) -> Self {
        self.status_message = Some(message.into());
        self
    }

    /// Set the full operator-facing diagnostic detail (PMCP extension — D-17;
    /// see the field doc comment on [`Task::diagnostic_detail`]).
    pub fn with_diagnostic_detail(mut self, detail: impl Into<String>) -> Self {
        self.diagnostic_detail = Some(detail.into());
        self
    }

    /// Classify this already-polled task into a [`TaskPollDecision`] without any
    /// I/O — the single decision primitive every task poller consumes.
    ///
    /// This is a pure, total function of the polled `Task`'s [`TaskStatus`]:
    /// there is no `_` wildcard arm because `TaskStatus` is exhaustive (five
    /// known variants), so the mapping cannot silently drift. Because it touches
    /// nothing but the in-hand `Task`, it is replay-deterministic and safe to
    /// call inside a memoized durable/replay step — provided the `tasks/get`
    /// network call and its serde decode sit INSIDE the memoized step, so an
    /// unknown/future status fails at deserialization before this runs (D-01,
    /// D-04, D-14, D-15).
    ///
    /// Mapping:
    /// - [`TaskStatus::Working`] → [`TaskPollDecision::InProgress`] carrying the
    ///   task's `poll_interval` verbatim as `poll_hint` (D-07).
    /// - [`TaskStatus::InputRequired`] → [`TaskPollDecision::InputRequired`].
    /// - [`TaskStatus::Completed`] / [`TaskStatus::Failed`] /
    ///   [`TaskStatus::Cancelled`] → [`TaskPollDecision::Terminal`] carrying the
    ///   terminal status. The final
    ///   [`CallToolResult`](crate::types::CallToolResult) is NOT fetched here —
    ///   the caller issues a separate call (D-06/D-16): `tasks/result` on v1,
    ///   and on v2 a `tasks/get`, which inlines the result (`tasks/result` is
    ///   retired there).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::types::tasks::{Task, TaskStatus, TaskPollDecision};
    ///
    /// let task = Task::new("t-123", TaskStatus::Working).with_poll_interval(2000);
    /// match task.poll_decision() {
    ///     TaskPollDecision::InProgress { poll_hint } => assert_eq!(poll_hint, Some(2000)),
    ///     // `TaskPollDecision` is `#[non_exhaustive]`, so external callers
    ///     // need a wildcard arm.
    ///     _ => panic!("a Working task must classify as InProgress"),
    /// }
    /// ```
    pub fn poll_decision(&self) -> TaskPollDecision {
        match self.status {
            TaskStatus::Working => TaskPollDecision::InProgress {
                poll_hint: self.poll_interval,
            },
            TaskStatus::InputRequired => TaskPollDecision::InputRequired,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                TaskPollDecision::Terminal {
                    status: self.status,
                }
            },
        }
    }
}

/// Parameters for task creation (augments tools/call).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskCreationParams {
    /// Time-to-live in milliseconds. Required but nullable per MCP spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
    pub ttl: Option<u64>,
    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

impl TaskCreationParams {
    /// Create empty task creation parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the time-to-live in milliseconds.
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the suggested polling interval in milliseconds.
    pub fn with_poll_interval(mut self, interval: u64) -> Self {
        self.poll_interval = Some(interval);
        self
    }
}

/// Task metadata for related-task references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTaskMetadata {
    /// The referenced task ID
    pub task_id: String,
}

/// Typed metadata carried on a `CallToolResult` under
/// [`RELATED_TASK_META_KEY`], linking a synchronous tool result to the async
/// task that produced (or is producing) it (SEP-1686).
///
/// This is the richer twin of [`RelatedTaskMetadata`]: it carries the optional
/// polling hints a client needs to drive
/// [`Client::wait_for_task`](crate::Client::wait_for_task) without hand-copying
/// fields. Server code builds it via
/// [`CallToolResult::with_related_task`](crate::types::CallToolResult::with_related_task);
/// client code reads it via
/// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task).
///
/// The two `Option` polling fields default to `None`, so the minimal native
/// emit shape `{ "taskId": "t1" }` deserializes cleanly (extra fields absent).
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Construct it via [`TaskMetadata::new`]
/// and the builder methods to stay forward-compatible:
///
/// ```rust
/// use pmcp::types::tasks::TaskMetadata;
///
/// let meta = TaskMetadata::new("t-123")
///     .with_poll_interval(5000)
///     .with_max_poll_duration_secs(300);
/// let json = serde_json::to_value(&meta).unwrap();
/// assert_eq!(json["taskId"], "t-123");
/// assert_eq!(json["pollInterval"], 5000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskMetadata {
    /// The referenced task ID.
    pub task_id: String,
    /// Suggested polling interval, in **milliseconds**.
    ///
    /// This is the same unit as [`Task::poll_interval`]. A client poller uses
    /// it as the delay between `tasks/get` calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    /// Maximum total time to poll before giving up, in **seconds**.
    ///
    /// Note the unit differs from [`TaskMetadata::poll_interval`] (which is
    /// milliseconds): this is a coarse overall budget expressed in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_poll_duration_secs: Option<u64>,
}

impl TaskMetadata {
    /// Create related-task metadata referencing `task_id`.
    ///
    /// Both polling hints default to `None`; set them via
    /// [`TaskMetadata::with_poll_interval`] and
    /// [`TaskMetadata::with_max_poll_duration_secs`].
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            poll_interval: None,
            max_poll_duration_secs: None,
        }
    }

    /// Set the suggested polling interval, in **milliseconds**.
    pub fn with_poll_interval(mut self, interval_ms: u64) -> Self {
        self.poll_interval = Some(interval_ms);
        self
    }

    /// Set the maximum total polling duration, in **seconds**.
    pub fn with_max_poll_duration_secs(mut self, secs: u64) -> Self {
        self.max_poll_duration_secs = Some(secs);
        self
    }
}

/// Result of creating a task — the **v1 (2025-11-25)** wire shape.
///
/// v1 NESTS the task under a `task` key. The v2 `CreateTaskResult` is FLAT
/// (`Result & Task`) and carries the discriminator `resultType: "task"`;
/// nothing constructs this struct on the v2 path. The v2 body is projected
/// through [`TaskV2`] and emitted by
/// `pmcp::server::task_dispatch`'s `v2_create_result_value` (inventory rows
/// 16-17, plans 114-11 / 114-12).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// The created task
    pub task: Task,
}

impl CreateTaskResult {
    /// Create a task creation result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
}

/// Task status notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotification {
    /// Task with updated status
    pub task: Task,
}

/// Get task request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskRequest {
    /// Task ID to retrieve
    pub task_id: String,
}

/// Get task result — the **v1 (2025-11-25)** wire shape.
///
/// v1 NESTS the task under a `task` key and carries no result or error body.
/// The v2 `GetTaskResult` is FLAT (`Result & DetailedTask`) with
/// `resultType: "complete"`, and it INLINES the terminal `result` / `error` and
/// the outstanding `inputRequests` — which is why v2 needs no `tasks/result`.
/// That shape is [`TaskDetailV2`] over a [`TaskV2`] (inventory rows 18, 21-24,
/// plan 114-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct GetTaskResult {
    /// The requested task
    pub task: Task,
}

impl GetTaskResult {
    /// Create a get task result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
}

/// Get task payload request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskPayloadRequest {
    /// Task ID whose payload to retrieve
    pub task_id: String,
}

/// List tasks request (paginated).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// List tasks result — **v1-only (2025-11-25)**.
///
/// There is no v2 counterpart and no projection type: `tasks/list` is ABSENT
/// from the `io.modelcontextprotocol/tasks` extension and answers `-32601` on a
/// v2-negotiated request (inventory row 37, plan 114-08). The removal is a
/// SECURITY improvement — with no enumeration primitive a server cannot leak
/// the existence of one caller's tasks to another. Kept unchanged for v1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ListTasksResult {
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl ListTasksResult {
    /// Create a list tasks result.
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Cancel task request.
///
/// When `result` is `Some`, the task transitions to `Completed` status
/// (workflow completion). When `result` is `None`, the task transitions to
/// `Cancelled` status (standard cancel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    /// Task ID to cancel
    pub task_id: String,
    /// Optional result value for workflow completion.
    ///
    /// When present, completes the task (transitions to `Completed` status)
    /// instead of cancelling it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Cancel task result — the **v1 (2025-11-25)** wire shape.
///
/// v1 echoes the cancelled task back under a `task` key. The v2
/// `CancelTaskResult` is an **empty acknowledgement** — `Result` only, with
/// `resultType: "complete"` and NO task body — because v2 cancellation is
/// cooperative and eventually consistent, so echoing a status would assert a
/// synchrony the server does not have. Poll [`TaskDetailV2`] via `tasks/get` to
/// observe the transition (inventory row 20, plan 114-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResult {
    /// The cancelled task
    pub task: Task,
}

impl CancelTaskResult {
    /// Create a cancel task result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
}

// ===========================================================================
// v2 (2026-07-28) tasks-extension projection types — ADDITIVE ONLY.
//
// Nothing above this line changes. `Task`, `CreateTaskResult`, `GetTaskResult`
// and `CancelTaskResult` are the shapes every tasks server already emits on the
// 2025-11-25 wire, and a serde-level rename or re-nesting on any of them would
// move those bytes for ALL of them — the D-02 lock `tests/v1_tasks_golden.rs`
// pins. The v2 shapes therefore live in SEPARATE types, projected above the
// `serde_json::Value` seam in `server::task_dispatch`, and the two eras never
// share a serde attribute.
//
// The v2 union is also not expressible as one flat struct: the extension models
// the detailed task as five status-discriminated variants with PER-VARIANT
// required fields (`result` on `completed`, `error` on `failed`,
// `inputRequests` on `input_required`), which `Option` + `skip_serializing_if`
// can only approximate.
// ===========================================================================

/// The commit of the vendored tasks-extension artifact every v2 wire name in
/// this module is read from.
///
/// Recorded as a constant rather than only in prose so a re-vendoring at the
/// D-18 schema gate has a compile-visible thing to update. The digests live in
/// `schema/vendored/ext-tasks/PROVENANCE.md`.
pub const EXT_TASKS_SCHEMA_COMMIT: &str = "2c1425d9a288b9b1f489430fe1e00bb392b47e48";

/// The flat v2 `Task` payload of the MCP tasks extension (2026-07-28).
///
/// # Provenance
///
/// Every field name here was read from the vendored artifact
/// `schema/vendored/ext-tasks/schema.ts` (`interface Task`, lines 46-92) at
/// [`EXT_TASKS_SCHEMA_COMMIT`], never from research prose. The governing
/// wire-value inventory is
/// `.planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md`
/// rows 4-10 — that document is the authority if it and this rustdoc ever
/// disagree.
///
/// # Required fields: FIVE, not four
///
/// `schema.json` `$defs.Task.required` is
/// `["taskId", "status", "createdAt", "lastUpdatedAt", "ttlMs"]`. `ttlMs` is
/// **required and nullable**; "optional because it can be null" is the wrong
/// reading and produces a schema-invalid response. Only `pollIntervalMs` and
/// `statusMessage` are genuinely optional.
///
/// # Relationship to the v1 [`Task`]
///
/// This is a PROJECTION of it, built by [`TaskV2::from_v1`], not a replacement:
/// `ttl` becomes `ttlMs` and `poll_interval` becomes `pollIntervalMs`. Those two
/// are RENAMES on the wire, not merely re-nestings, which is why they carry
/// their own per-field provenance notes below.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskV2 {
    /// The task identifier. **Required.** (`schema.ts:50`)
    pub task_id: String,
    /// Current task status. **Required.** (`schema.ts:55`)
    ///
    /// Serialized by the SAME [`TaskStatus`] the v1 wire uses — the two eras'
    /// five status strings are name-identical, so there is no conversion table
    /// to drift, only a locking tripwire in `tests/v2_tasks_shapes.rs`.
    pub status: TaskStatus,
    /// Optional human-readable message describing the current task state.
    /// (`schema.ts:57-66`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// ISO 8601 creation timestamp. **Required.** (`schema.ts:71`)
    pub created_at: String,
    /// ISO 8601 last-updated timestamp. **Required.** (`schema.ts:76`)
    pub last_updated_at: String,
    /// Time-to-live from creation in integer milliseconds, `null` for unlimited.
    ///
    /// **RENAMED from the v1 `ttl`** — inventory row 8. It is **required AND
    /// nullable** (`schema.ts:79-84`, `$defs.Task.required[4]`), so it is
    /// deliberately modelled WITHOUT `skip_serializing_if`: `None` must
    /// serialize as `"ttlMs":null` (present), never be omitted. This is the same
    /// treatment [`Task::ttl`] already documents, for the same reason.
    ///
    /// **Not a cache hint (D-10).** This `ttlMs` is a task LIFETIME — how long
    /// the server retains this task record. It shares its spec name with the
    /// `ttlMs` of [`crate::types::caching`], which is a cache-FRESHNESS hint on
    /// the six `CacheableResult` list/read responses. The two are unrelated and
    /// the modules deliberately never import each other; copying a long task
    /// lifetime into a cache hint would tell clients that stale data is fresh.
    pub ttl_ms: Option<u64>,
    /// Suggested polling interval in integer milliseconds.
    ///
    /// **RENAMED from the v1 `pollInterval`** — inventory row 9. Genuinely
    /// OPTIONAL: it is absent from every per-variant `required` array
    /// (`schema.ts:86-91`), so it carries `skip_serializing_if` and a `None`
    /// omits the key entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_ms: Option<u64>,
    /// Full operator-facing diagnostic detail (step ids, URLs, internal error
    /// text) for this task.
    ///
    /// **PMCP EXTENSION — not an MCP-spec field (D-17).** Carried through from
    /// [`Task::diagnostic_detail`] verbatim in argument: the extension's `Task`
    /// has no `deny_unknown_fields` equivalent in practice, so a consumer that
    /// does not know this key simply ignores the extra `diagnosticDetail` on the
    /// wire — additive and non-breaking. Producers MUST redact secrets/tokens
    /// before setting it. When absent (`None`) it is skip-serialized, so a
    /// projection that never sets it is byte-identical to one from before this
    /// field existed.
    ///
    /// **Future migration note:** if/when the extension grows a `_meta` slot on
    /// `Task` (mirroring [`RELATED_TASK_META_KEY`]), this field is a candidate to
    /// migrate under that slot instead of a top-level key, to keep the projected
    /// `Task` spec-pure. Until then this is the pragmatic wire slot — and note
    /// that the generated `schema.json` marks the `Task` subschema
    /// `additionalProperties: false`, which is a `ts-to-zod` artifact of the
    /// `allOf` composition (it would also reject the `_meta` the same result is
    /// required to carry), not a signal to drop this key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_detail: Option<String>,
}

impl TaskV2 {
    /// Project a v1 [`Task`] onto the v2 wire shape.
    ///
    /// This is the ONLY place the two renames happen (`ttl` -> `ttlMs`,
    /// `poll_interval` -> `pollIntervalMs`), so a store that grows another field
    /// has exactly one function to teach.
    pub fn from_v1(task: &Task) -> Self {
        Self {
            task_id: task.task_id.clone(),
            status: task.status,
            status_message: task.status_message.clone(),
            created_at: task.created_at.clone(),
            last_updated_at: task.last_updated_at.clone(),
            ttl_ms: task.ttl,
            poll_interval_ms: task.poll_interval,
            diagnostic_detail: task.diagnostic_detail.clone(),
        }
    }

    /// Project this v2 wire shape back onto the v1 [`Task`] — the exact inverse
    /// of [`from_v1`](Self::from_v1) (Phase 114, plan 19).
    ///
    /// It lives HERE, immediately beside its inverse, for the reason
    /// [`from_v1`](Self::from_v1) states: the two renames (`ttl` <-> `ttlMs`,
    /// `pollInterval` <-> `pollIntervalMs`) must have exactly ONE definition. The
    /// v2 CLIENT needs the backwards direction so [`Client::tasks_get`](crate::Client::tasks_get)
    /// can keep returning a `Task` — a public signature that cannot change — from
    /// a flat v2 payload, and doing that remap inside the client would have been
    /// the second copy of this table.
    ///
    /// # Round-trip
    ///
    /// `TaskV2::from_v1(&t).to_v1()` is field-for-field `t`: every field of
    /// [`Task`] has a counterpart here, so nothing is dropped in either
    /// direction. A unit test pins that.
    pub fn to_v1(&self) -> Task {
        Task {
            task_id: self.task_id.clone(),
            status: self.status,
            ttl: self.ttl_ms,
            created_at: self.created_at.clone(),
            last_updated_at: self.last_updated_at.clone(),
            poll_interval: self.poll_interval_ms,
            status_message: self.status_message.clone(),
            diagnostic_detail: self.diagnostic_detail.clone(),
        }
    }

    /// Create a minimal projection with the five required fields set.
    ///
    /// Timestamps and `ttlMs` are taken explicitly because all three are
    /// REQUIRED on the wire — there is no honest default for them.
    pub fn new(
        task_id: impl Into<String>,
        status: TaskStatus,
        created_at: impl Into<String>,
        last_updated_at: impl Into<String>,
        ttl_ms: Option<u64>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            status,
            status_message: None,
            created_at: created_at.into(),
            last_updated_at: last_updated_at.into(),
            ttl_ms,
            poll_interval_ms: None,
            diagnostic_detail: None,
        }
    }
}

/// The status-discriminated detail of a v2 `DetailedTask`.
///
/// One variant per [`TaskStatus`], each carrying exactly the field its schema
/// variant marks required:
///
/// | Variant | Schema `$defs` | Extra required field |
/// |---|---|---|
/// | [`Working`](Self::Working) | `WorkingTask` | *(none)* |
/// | [`InputRequired`](Self::InputRequired) | `InputRequiredTask` | `inputRequests` |
/// | [`Completed`](Self::Completed) | `CompletedTask` | `result` |
/// | [`Failed`](Self::Failed) | `FailedTask` | `error` |
/// | [`Cancelled`](Self::Cancelled) | `CancelledTask` | *(none)* |
///
/// Expressing the union as an enum is what makes the per-variant requirement
/// STRUCTURAL: a `completed` projection cannot be built without a `result`, a
/// `failed` one without an `error`, an `input_required` one without
/// `inputRequests` — there is no constructor that takes fewer fields.
///
/// # Deliberately NOT `#[non_exhaustive]`
///
/// The union is closed by construction: it has exactly one variant per
/// [`TaskStatus`], and `TaskStatus` is itself deliberately exhaustive (D-15). A
/// sixth variant here could only follow a sixth status, which is a wire-breaking
/// change on both types at once — so a wildcard arm downstream would hide the
/// drift rather than absorb it. `tests/v2_tasks_shapes.rs` locks the five status
/// strings against the vendored schema by SET EQUALITY for the same reason.
///
/// # No `PartialEq`
///
/// [`InputRequest`](crate::types::mrtr::InputRequest) does not implement it (its
/// payloads are the elicitation/sampling param structs, which do not either), so
/// deriving it here would mean widening two other modules' public API for a
/// convenience this type does not need. Compare projections by their wire object
/// instead — which is the comparison that actually matters for a wire type.
#[derive(Debug, Clone)]
pub enum TaskDetailV2 {
    /// `WorkingTask` — the task is running. No extra required field.
    Working,
    /// `InputRequiredTask` — the task is paused awaiting client input.
    InputRequired {
        /// The server-to-client requests that must be fulfilled, keyed by the
        /// server-assigned key. A **top-level** key of the `tasks/get` result on
        /// v2 (inventory row 23), not a nested one.
        input_requests: crate::types::mrtr::InputRequests,
    },
    /// `CompletedTask` — the task finished successfully.
    Completed {
        /// The final result, whose structure matches the result type of the
        /// originating request (for a `tools/call` task, a `CallToolResult`).
        /// Modelled as a `Map` rather than a `Value` because the schema types it
        /// `{ [key: string]: unknown }` — an OBJECT.
        result: serde_json::Map<String, serde_json::Value>,
    },
    /// `FailedTask` — the task failed with a JSON-RPC error.
    ///
    /// Reserved for JSON-RPC protocol errors. A tool that ran to completion and
    /// returned `isError: true` is `completed`, with the error detail inside
    /// `result` — the two look identical from a "the tool failed" mindset and are
    /// opposite on the wire.
    Failed {
        /// The JSON-RPC error object (`code`/`message`/`data`) that ended the
        /// task. An OBJECT, per the schema.
        error: serde_json::Map<String, serde_json::Value>,
    },
    /// `CancelledTask` — the task was cancelled. No extra required field.
    Cancelled,
}

impl TaskDetailV2 {
    /// The [`TaskStatus`] this variant IS.
    ///
    /// The mapping is total and has no wildcard arm, so a status added to either
    /// type without the other fails to compile.
    pub fn status(&self) -> TaskStatus {
        match self {
            Self::Working => TaskStatus::Working,
            Self::InputRequired { .. } => TaskStatus::InputRequired,
            Self::Completed { .. } => TaskStatus::Completed,
            Self::Failed { .. } => TaskStatus::Failed,
            Self::Cancelled => TaskStatus::Cancelled,
        }
    }

    /// The wire key this variant contributes to the flattened result, if any.
    pub fn wire_key(&self) -> Option<&'static str> {
        match self {
            Self::Working | Self::Cancelled => None,
            Self::InputRequired { .. } => Some(DETAIL_KEY_INPUT_REQUESTS),
            Self::Completed { .. } => Some(DETAIL_KEY_RESULT),
            Self::Failed { .. } => Some(DETAIL_KEY_ERROR),
        }
    }
}

/// The `inputRequests` wire key of `InputRequiredTask`.
///
/// Spelled once here and read by name everywhere else. It is deliberately the
/// SAME string the reserved-result-field registry owns
/// (`crate::types::mrtr::INPUT_REQUESTS_KEY`), and the equality is asserted by a
/// unit test rather than left to a reader to notice.
pub const DETAIL_KEY_INPUT_REQUESTS: &str = "inputRequests";
/// The `result` wire key of `CompletedTask`.
pub const DETAIL_KEY_RESULT: &str = "result";
/// The `error` wire key of `FailedTask`.
pub const DETAIL_KEY_ERROR: &str = "error";

/// The v2 `DetailedTask` — a flat [`TaskV2`] plus its status-specific detail.
///
/// This is the body of a v2 `tasks/get` result (`GetTaskResult = Result &
/// DetailedTask`, `schema.ts:252-259`), and `$defs.GetTaskResult` in the vendored
/// `schema.json` is a **flat `allOf`**, NOT a `{"task": …}` wrapper — verified
/// on disk rather than quoted. v1's `GetTaskResult` DOES wrap under `task`; the
/// two eras genuinely differ, and that difference is what this type exists to
/// express.
///
/// # Why the fields are private
///
/// [`TaskV2::status`] and the detail variant both encode the status, and a
/// response whose `status` disagrees with its inlined detail is exactly the
/// schema-invalid shape this type exists to prevent. [`DetailedTaskV2::new`] is
/// the only constructor and it makes the DETAIL authoritative, overwriting the
/// base's status. Private fields are what make that impossible to bypass.
///
/// No `PartialEq` — see [`TaskDetailV2`]'s note; compare
/// [`to_wire_object`](Self::to_wire_object) outputs instead.
#[derive(Debug, Clone)]
pub struct DetailedTaskV2 {
    base: TaskV2,
    detail: TaskDetailV2,
}

impl DetailedTaskV2 {
    /// Pair a flat task payload with its status detail.
    ///
    /// The DETAIL is authoritative: `base.status` is overwritten with
    /// [`TaskDetailV2::status`], so the emitted `status` and the emitted
    /// `result`/`error`/`inputRequests` can never disagree.
    pub fn new(base: TaskV2, detail: TaskDetailV2) -> Self {
        let mut base = base;
        base.status = detail.status();
        Self { base, detail }
    }

    /// The flat task payload.
    pub fn task(&self) -> &TaskV2 {
        &self.base
    }

    /// The status-specific detail.
    pub fn detail(&self) -> &TaskDetailV2 {
        &self.detail
    }

    /// Serialize to the FLAT wire object: the five required `Task` fields, the
    /// optional ones actually set, and the variant's own required key last.
    ///
    /// Built by hand rather than by `#[serde(flatten)]` + `#[serde(untagged)]`
    /// so the key set is decided by code a reader can follow, and so the
    /// `ttlMs: null` (present) / `pollIntervalMs` (omitted) asymmetry survives
    /// the flatten serializer.
    ///
    /// # Panics
    ///
    /// Never in practice: [`TaskV2`] is a plain struct of `String`/`u64`/`Option`
    /// fields, so its `Serialize` impl cannot fail and cannot produce a
    /// non-object. The fallback returns an empty map rather than unwrapping.
    pub fn to_wire_object(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut object = match serde_json::to_value(&self.base) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        };
        match &self.detail {
            TaskDetailV2::Working | TaskDetailV2::Cancelled => {},
            TaskDetailV2::InputRequired { input_requests } => {
                object.insert(
                    DETAIL_KEY_INPUT_REQUESTS.to_string(),
                    serde_json::to_value(input_requests)
                        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new())),
                );
            },
            TaskDetailV2::Completed { result } => {
                object.insert(
                    DETAIL_KEY_RESULT.to_string(),
                    serde_json::Value::Object(result.clone()),
                );
            },
            TaskDetailV2::Failed { error } => {
                object.insert(
                    DETAIL_KEY_ERROR.to_string(),
                    serde_json::Value::Object(error.clone()),
                );
            },
        }
        object
    }

    /// Decode a flat wire object back into a `DetailedTaskV2`, STATUS-DIRECTED.
    ///
    /// The status is read first and it decides which key is required — the same
    /// discipline [`InputResponse::decode_for`](crate::types::mrtr::InputResponse::decode_for)
    /// applies to `inputResponses`, and the reason is the same: an untagged
    /// best-effort decode would silently accept a `completed` task with no
    /// `result` by falling back to a variant that fits.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the base `Task` fields are missing
    /// or mistyped, or when the status's required detail key is absent or is not
    /// an object.
    pub fn from_wire_value(value: &serde_json::Value) -> Result<Self, String> {
        let base: TaskV2 = serde_json::from_value(value.clone())
            .map_err(|e| format!("not a v2 Task payload: {e}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "a DetailedTask must be a JSON object".to_string())?;
        let required_object = |key: &str| -> Result<serde_json::Map<String, _>, String> {
            object
                .get(key)
                .and_then(serde_json::Value::as_object)
                .cloned()
                .ok_or_else(|| format!("a {} task requires an object `{key}`", base.status))
        };
        let detail = match base.status {
            TaskStatus::Working => TaskDetailV2::Working,
            TaskStatus::Cancelled => TaskDetailV2::Cancelled,
            TaskStatus::Completed => TaskDetailV2::Completed {
                result: required_object(DETAIL_KEY_RESULT)?,
            },
            TaskStatus::Failed => TaskDetailV2::Failed {
                error: required_object(DETAIL_KEY_ERROR)?,
            },
            TaskStatus::InputRequired => TaskDetailV2::InputRequired {
                input_requests: object
                    .get(DETAIL_KEY_INPUT_REQUESTS)
                    .ok_or_else(|| {
                        format!("an input_required task requires `{DETAIL_KEY_INPUT_REQUESTS}`")
                    })
                    .and_then(|v| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| format!("malformed `{DETAIL_KEY_INPUT_REQUESTS}`: {e}"))
                    })?,
            },
        };
        Ok(Self::new(base, detail))
    }
}

impl Serialize for DetailedTaskV2 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_wire_object().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DetailedTaskV2 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Self::from_wire_value(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_status_serialization() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Working).unwrap(),
            "working"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::InputRequired).unwrap(),
            "input_required"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::Completed).unwrap(),
            "completed"
        );
        assert_eq!(serde_json::to_value(TaskStatus::Failed).unwrap(), "failed");
        assert_eq!(
            serde_json::to_value(TaskStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    #[test]
    fn task_roundtrip() {
        let task = Task::new("t-123", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
            .with_ttl(60000)
            .with_poll_interval(5000)
            .with_status_message("Processing...");
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["taskId"], "t-123");
        assert_eq!(json["status"], "working");
        assert_eq!(json["ttl"], 60000);
        assert_eq!(json["createdAt"], "2025-11-25T00:00:00Z");
        assert_eq!(json["pollInterval"], 5000);

        let roundtrip: Task = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task_id, "t-123");
        assert_eq!(roundtrip.status, TaskStatus::Working);
    }

    #[test]
    fn create_task_result_roundtrip() {
        let result = CreateTaskResult::new(
            Task::new("t-456", TaskStatus::Completed)
                .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:05:00Z"),
        );
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["task"]["taskId"], "t-456");
        assert_eq!(json["task"]["status"], "completed");

        let roundtrip: CreateTaskResult = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task.status, TaskStatus::Completed);
    }

    #[test]
    fn task_ts_format_interop() {
        // Test deserialization from TypeScript-format JSON
        let ts_json = json!({
            "taskId": "task-abc",
            "status": "input_required",
            "createdAt": "2025-11-25T12:00:00.000Z",
            "lastUpdatedAt": "2025-11-25T12:01:00.000Z",
            "pollInterval": 3000,
            "statusMessage": "Waiting for user input"
        });
        let task: Task = serde_json::from_value(ts_json).unwrap();
        assert_eq!(task.task_id, "task-abc");
        assert_eq!(task.status, TaskStatus::InputRequired);
        assert_eq!(task.poll_interval, Some(3000));
    }

    #[test]
    fn task_metadata_serde_round_trip() {
        let meta = TaskMetadata {
            task_id: "t1".to_string(),
            poll_interval: Some(5000),
            max_poll_duration_secs: None,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["taskId"], "t1");
        assert_eq!(json["pollInterval"], 5000);
        // maxPollDurationSecs omitted (skip_serializing_if = None)
        assert!(
            json.get("maxPollDurationSecs").is_none(),
            "maxPollDurationSecs should be omitted when None"
        );

        let roundtrip: TaskMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task_id, "t1");
        assert_eq!(roundtrip.poll_interval, Some(5000));
        assert_eq!(roundtrip.max_poll_duration_secs, None);
    }

    #[test]
    fn task_metadata_minimal_shape_deserializes() {
        // The minimal native emit shape carries only taskId.
        let meta: TaskMetadata = serde_json::from_value(json!({ "taskId": "t1" })).unwrap();
        assert_eq!(meta.task_id, "t1");
        assert_eq!(meta.poll_interval, None);
        assert_eq!(meta.max_poll_duration_secs, None);
    }

    #[test]
    fn related_task_meta_key_value() {
        assert_eq!(
            RELATED_TASK_META_KEY,
            "io.modelcontextprotocol/related-task"
        );
    }

    #[test]
    fn task_status_is_terminal() {
        assert!(!TaskStatus::Working.is_terminal());
        assert!(!TaskStatus::InputRequired.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
    }

    #[test]
    fn task_status_can_transition_to() {
        // Working can transition to all except itself
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::InputRequired));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Cancelled));

        // InputRequired can transition to all except itself
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Working));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Cancelled));
    }

    #[test]
    fn task_status_self_transition_rejected() {
        assert!(!TaskStatus::Working.can_transition_to(&TaskStatus::Working));
        assert!(!TaskStatus::InputRequired.can_transition_to(&TaskStatus::InputRequired));
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Completed));
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Failed));
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Cancelled));
    }

    #[test]
    fn task_status_terminal_rejects_all() {
        for terminal in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            for target in [
                TaskStatus::Working,
                TaskStatus::InputRequired,
                TaskStatus::Completed,
                TaskStatus::Failed,
                TaskStatus::Cancelled,
            ] {
                assert!(
                    !terminal.can_transition_to(&target),
                    "{terminal:?} should not transition to {target:?}"
                );
            }
        }
    }

    #[test]
    fn task_ttl_null_serialization() {
        let task = Task::new("test-null-ttl", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z");
        let json = serde_json::to_value(&task).unwrap();
        // ttl MUST be present as null, not omitted (MCP spec: number | null)
        assert!(json.get("ttl").is_some(), "ttl must be present");
        assert!(json["ttl"].is_null(), "ttl must be null when None");
        // pollInterval SHOULD be omitted when None
        assert!(
            json.get("pollInterval").is_none(),
            "pollInterval should be omitted when None"
        );
    }

    #[test]
    fn task_ttl_present_serialization() {
        let task = Task::new("test-present-ttl", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
            .with_ttl(60000);
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["ttl"], 60000);
    }

    #[test]
    fn task_diagnostic_detail_absent_when_none() {
        // D-17 PMCP extension: diagnostic_detail defaults to None and MUST be
        // skip-serialized — byte-identical to pre-field JSON for callers that
        // never set it.
        let task = Task::new("t-diag-none", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z");
        assert_eq!(task.diagnostic_detail, None);
        let json = serde_json::to_value(&task).unwrap();
        assert!(
            json.get("diagnosticDetail").is_none(),
            "diagnosticDetail must be omitted from JSON when None"
        );
    }

    #[test]
    fn task_diagnostic_detail_round_trip() {
        // D-17 PMCP extension serde round-trip: Some(...) serializes under the
        // camelCase `diagnosticDetail` key and deserializes back unchanged.
        let task = Task::new("t-diag-some", TaskStatus::Failed)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
            .with_status_message("The AI service was temporarily unavailable")
            .with_diagnostic_detail(
                "step=call_tool op=propose_schema url=https://api.example/v1/x error=timeout",
            );
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(
            json["diagnosticDetail"],
            "step=call_tool op=propose_schema url=https://api.example/v1/x error=timeout"
        );
        // Both voices ride independently — statusMessage stays the friendly one.
        assert_eq!(
            json["statusMessage"],
            "The AI service was temporarily unavailable"
        );

        let roundtrip: Task = serde_json::from_value(json).unwrap();
        assert_eq!(
            roundtrip.diagnostic_detail.as_deref(),
            Some("step=call_tool op=propose_schema url=https://api.example/v1/x error=timeout")
        );
        assert_eq!(
            roundtrip.status_message.as_deref(),
            Some("The AI service was temporarily unavailable")
        );
    }

    #[test]
    fn task_diagnostic_detail_consumer_tolerance() {
        // Review concern #3 / T-162-05-COMPAT: a Task JSON carrying the NEW
        // diagnosticDetail key (plus a hypothetical extra unknown key a
        // strict/older consumer wouldn't recognize) deserializes cleanly into
        // `Task` — proving Task does NOT use deny_unknown_fields and existing
        // consumers tolerate additive wire fields.
        let wire_json = json!({
            "taskId": "task-tolerant",
            "status": "failed",
            "ttl": null,
            "createdAt": "2025-11-25T12:00:00.000Z",
            "lastUpdatedAt": "2025-11-25T12:01:00.000Z",
            "statusMessage": "The AI service was temporarily unavailable",
            "diagnosticDetail": "step=call_tool op=propose_schema error=timeout",
            "someFutureUnknownField": { "nested": "value" }
        });
        let task: Task = serde_json::from_value(wire_json)
            .expect("Task must tolerate diagnosticDetail + an unrelated unknown field");
        assert_eq!(task.task_id, "task-tolerant");
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.diagnostic_detail.as_deref(),
            Some("step=call_tool op=propose_schema error=timeout")
        );
        assert_eq!(
            task.status_message.as_deref(),
            Some("The AI service was temporarily unavailable")
        );
    }

    #[test]
    fn task_status_display() {
        assert_eq!(TaskStatus::Working.to_string(), "working");
        assert_eq!(TaskStatus::InputRequired.to_string(), "input_required");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    /// All five `TaskStatus` values, for exhaustive table tests.
    const ALL_STATUSES: [TaskStatus; 5] = [
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];

    /// The expected `poll_decision()` classification for a status, given the
    /// task's `poll_interval` (only consulted for the `Working` case).
    fn expected_decision(status: TaskStatus, poll_interval: Option<u64>) -> TaskPollDecision {
        match status {
            TaskStatus::Working => TaskPollDecision::InProgress {
                poll_hint: poll_interval,
            },
            TaskStatus::InputRequired => TaskPollDecision::InputRequired,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                TaskPollDecision::Terminal { status }
            },
        }
    }

    #[test]
    fn poll_decision_maps_every_status() {
        // Working -> InProgress carrying the poll_interval verbatim.
        let working = Task::new("t-w", TaskStatus::Working).with_poll_interval(2500);
        assert_eq!(
            working.poll_decision(),
            TaskPollDecision::InProgress {
                poll_hint: Some(2500)
            }
        );

        // Working with no poll_interval -> InProgress { poll_hint: None }.
        let working_none = Task::new("t-wn", TaskStatus::Working);
        assert_eq!(
            working_none.poll_decision(),
            TaskPollDecision::InProgress { poll_hint: None }
        );

        // InputRequired -> unit variant.
        let waiting = Task::new("t-i", TaskStatus::InputRequired);
        assert_eq!(waiting.poll_decision(), TaskPollDecision::InputRequired);

        // Each terminal status -> Terminal carrying that status.
        for status in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let task = Task::new("t-term", status);
            assert_eq!(
                task.poll_decision(),
                TaskPollDecision::Terminal { status },
                "{status:?} must classify as Terminal"
            );
        }
    }

    #[test]
    fn poll_decision_covers_all_statuses_exhaustively() {
        // Table drive across EVERY TaskStatus value (guards D-15 exhaustiveness).
        for status in ALL_STATUSES {
            let task = Task::new("t-x", status).with_poll_interval(1234);
            assert_eq!(
                task.poll_decision(),
                expected_decision(status, Some(1234)),
                "poll_decision() drifted for {status:?}"
            );
        }
    }

    #[test]
    fn resolve_poll_interval_precedence() {
        // Caller override wins over the server hint.
        assert_eq!(resolve_poll_interval(Some(200), Some(999)), 200);
        // Server hint used when there is no caller override.
        assert_eq!(resolve_poll_interval(None, Some(300)), 300);
        // Falls back to DEFAULT_POLL_MS when neither is specified.
        assert_eq!(resolve_poll_interval(None, None), DEFAULT_POLL_MS);
        assert_eq!(resolve_poll_interval(None, None), 1000);
    }

    #[test]
    fn resolve_poll_interval_floors_zero() {
        // A zero override cannot hot-spin — floored to MIN_POLL_MS.
        assert_eq!(resolve_poll_interval(Some(0), None), MIN_POLL_MS);
        assert_eq!(resolve_poll_interval(Some(0), None), 50);
        // The floor also applies to a low server hint.
        assert_eq!(resolve_poll_interval(None, Some(10)), 50);
        // And to a zero hint.
        assert_eq!(resolve_poll_interval(None, Some(0)), 50);
    }

    proptest::proptest! {
        /// For every TaskStatus and any poll_interval, poll_decision() returns
        /// exactly the mapped variant — the classifier never drifts.
        #[test]
        fn poll_decision_matches_expected_map(
            status_idx in 0usize..ALL_STATUSES.len(),
            poll_interval in proptest::option::of(proptest::prelude::any::<u64>()),
        ) {
            let status = ALL_STATUSES[status_idx];
            let mut task = Task::new("t-prop", status);
            task.poll_interval = poll_interval;
            proptest::prop_assert_eq!(
                task.poll_decision(),
                expected_decision(status, poll_interval)
            );
        }

        /// The 50 ms floor holds for ALL caller/hint inputs, not just the
        /// tabled cases (T-105-01 invariant).
        #[test]
        fn resolve_poll_interval_never_below_floor(
            caller in proptest::option::of(proptest::prelude::any::<u64>()),
            hint in proptest::option::of(proptest::prelude::any::<u64>()),
        ) {
            proptest::prop_assert!(resolve_poll_interval(caller, hint) >= MIN_POLL_MS);
        }
    }
}

/// Unit locks for the v2 projection types (114-11, TASK-04).
///
/// The `required` key sets are read from the VENDORED schema at compile time
/// rather than restated, so a re-vendoring at the D-18 gate moves these
/// assertions automatically instead of leaving them asserting yesterday's
/// contract.
#[cfg(test)]
mod v2_projection_tests {
    use super::*;
    use serde_json::{json, Value};

    /// The vendored tasks-extension JSON Schema, embedded at compile time.
    const EXT_TASKS_SCHEMA_JSON: &str = include_str!("../../schema/vendored/ext-tasks/schema.json");

    /// The `required` array of a `$defs` entry, as a sorted `Vec<String>`.
    fn schema_required(def: &str) -> Vec<String> {
        let schema: Value =
            serde_json::from_str(EXT_TASKS_SCHEMA_JSON).expect("vendored schema parses");
        let mut required: Vec<String> = schema["$defs"][def]["required"]
            .as_array()
            .unwrap_or_else(|| panic!("$defs.{def}.required is an array"))
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("a required entry is a string")
                    .to_string()
            })
            .collect();
        required.sort();
        required
    }

    fn sorted_keys(object: &serde_json::Map<String, Value>) -> Vec<String> {
        let mut keys: Vec<String> = object.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// A minimal projection with only the five required fields populated.
    fn minimal(status: TaskStatus) -> TaskV2 {
        TaskV2::new(
            "t-1",
            status,
            "2026-07-28T00:00:00Z",
            "2026-07-28T00:00:01Z",
            Some(60_000),
        )
    }

    fn requests() -> crate::types::mrtr::InputRequests {
        let mut map = crate::types::mrtr::InputRequests::new();
        map.insert(
            "roots".to_string(),
            crate::types::mrtr::InputRequest::ListRoots,
        );
        map
    }

    #[test]
    fn v2_projection_uses_the_renamed_ttl_ms_and_poll_interval_ms_keys() {
        let mut task = minimal(TaskStatus::Working);
        task.poll_interval_ms = Some(2500);
        let raw = serde_json::to_string(&task).expect("serializes");
        assert!(
            raw.contains("\"ttlMs\":60000"),
            "the v2 projection must spell `ttlMs`, got {raw}"
        );
        assert!(
            raw.contains("\"pollIntervalMs\":2500"),
            "the v2 projection must spell `pollIntervalMs`, got {raw}"
        );
        // The v1 spellings must NOT appear: these are RENAMES, not additions.
        assert!(!raw.contains("\"ttl\":"), "v1 `ttl` leaked into v2: {raw}");
        assert!(
            !raw.contains("\"pollInterval\":"),
            "v1 `pollInterval` leaked into v2: {raw}"
        );
    }

    #[test]
    fn v2_projection_serializes_a_none_ttl_as_an_explicit_null() {
        let task = TaskV2::new(
            "t-1",
            TaskStatus::Working,
            "2026-07-28T00:00:00Z",
            "2026-07-28T00:00:01Z",
            None,
        );
        let raw = serde_json::to_string(&task).expect("serializes");
        // `ttlMs` is REQUIRED and NULLABLE: present as null, never omitted.
        assert!(
            raw.contains("\"ttlMs\":null"),
            "a None ttlMs must serialize as an explicit null, got {raw}"
        );
    }

    #[test]
    fn v2_projection_omits_a_none_poll_interval_ms_entirely() {
        let task = minimal(TaskStatus::Working);
        assert_eq!(task.poll_interval_ms, None);
        let raw = serde_json::to_string(&task).expect("serializes");
        assert!(
            !raw.contains("pollIntervalMs"),
            "a None pollIntervalMs is OPTIONAL and must be omitted, got {raw}"
        );
    }

    /// `from_v1` and `to_v1` are exact inverses (Phase 114, plan 19).
    ///
    /// The v2 CLIENT decodes `tasks/get` and the flat create result through
    /// `to_v1`, so a field this pair drops would silently vanish from a v2
    /// caller's `Task`. Every field is compared by name — a `Task` that grew a
    /// field without teaching BOTH directions fails here rather than in a user's
    /// poll loop.
    #[test]
    fn v2_projection_round_trips_a_v1_task_field_for_field() {
        let original = Task::new("task-round-trip", TaskStatus::InputRequired)
            .with_timestamps("2026-07-28T00:00:00Z", "2026-07-28T00:00:09Z")
            .with_ttl(60_000)
            .with_poll_interval(250)
            .with_status_message("waiting on you")
            .with_diagnostic_detail("step 3 of 7");

        let round_tripped = TaskV2::from_v1(&original).to_v1();

        assert_eq!(round_tripped.task_id, original.task_id);
        assert_eq!(round_tripped.status, original.status);
        assert_eq!(round_tripped.ttl, original.ttl);
        assert_eq!(round_tripped.created_at, original.created_at);
        assert_eq!(round_tripped.last_updated_at, original.last_updated_at);
        assert_eq!(round_tripped.poll_interval, original.poll_interval);
        assert_eq!(round_tripped.status_message, original.status_message);
        assert_eq!(round_tripped.diagnostic_detail, original.diagnostic_detail);
        // Serialized equality catches a field the per-field list above forgot.
        assert_eq!(
            serde_json::to_value(&round_tripped).expect("serializes"),
            serde_json::to_value(&original).expect("serializes"),
        );
    }

    /// `to_v1` performs the two RENAMES, not a re-spelling of the v2 keys.
    #[test]
    fn to_v1_maps_ttl_ms_and_poll_interval_ms_onto_the_v1_names() {
        let mut v2 = TaskV2::new(
            "task-renames",
            TaskStatus::Working,
            "2026-07-28T00:00:00Z",
            "2026-07-28T00:00:01Z",
            Some(30_000),
        );
        v2.poll_interval_ms = Some(750);

        let v1 = v2.to_v1();

        assert_eq!(v1.ttl, Some(30_000), "ttlMs must land on ttl");
        assert_eq!(
            v1.poll_interval,
            Some(750),
            "pollIntervalMs must land on pollInterval"
        );
        let raw = serde_json::to_string(&v1).expect("serializes");
        assert!(
            !raw.contains("ttlMs") && !raw.contains("pollIntervalMs"),
            "a v2 key spelling leaked onto the v1 Task: {raw}"
        );
    }

    #[test]
    fn v2_projection_working_key_set_equals_the_schema_required_set() {
        let detailed = DetailedTaskV2::new(minimal(TaskStatus::Working), TaskDetailV2::Working);
        let object = detailed.to_wire_object();
        assert_eq!(
            sorted_keys(&object),
            schema_required("WorkingTask"),
            "a minimal working projection must emit EXACTLY the schema's \
             WorkingTask.required set — no optional key is set on this fixture"
        );
    }

    #[test]
    fn v2_projection_working_key_set_grows_only_by_the_optional_keys_actually_set() {
        let mut base = minimal(TaskStatus::Working);
        base.poll_interval_ms = Some(1000);
        base.status_message = Some("still going".to_string());
        let object = DetailedTaskV2::new(base, TaskDetailV2::Working).to_wire_object();
        let mut expected = schema_required("WorkingTask");
        expected.push("pollIntervalMs".to_string());
        expected.push("statusMessage".to_string());
        expected.sort();
        assert_eq!(sorted_keys(&object), expected);
    }

    /// Each variant emits the schema's required set for ITS `$defs` entry.
    ///
    /// This is the per-variant requirement expressed as a wire assertion; the
    /// TYPE-level half is that none of these variants can be constructed without
    /// its payload — `TaskDetailV2::Completed {}` does not compile, which is why
    /// there is no "completed projection with no result" test to write.
    #[test]
    fn v2_projection_each_variant_emits_its_schema_required_set() {
        let cases: Vec<(&str, TaskDetailV2)> = vec![
            ("WorkingTask", TaskDetailV2::Working),
            ("CancelledTask", TaskDetailV2::Cancelled),
            (
                "InputRequiredTask",
                TaskDetailV2::InputRequired {
                    input_requests: requests(),
                },
            ),
            (
                "CompletedTask",
                TaskDetailV2::Completed {
                    result: json!({ "content": [] })
                        .as_object()
                        .expect("object")
                        .clone(),
                },
            ),
            (
                "FailedTask",
                TaskDetailV2::Failed {
                    error: json!({ "code": -32603, "message": "boom" })
                        .as_object()
                        .expect("object")
                        .clone(),
                },
            ),
        ];
        for (def, detail) in cases {
            // The base's status is deliberately WRONG here; `new` must fix it.
            let object = DetailedTaskV2::new(minimal(TaskStatus::Working), detail).to_wire_object();
            assert_eq!(
                sorted_keys(&object),
                schema_required(def),
                "{def} must emit exactly its schema-required key set"
            );
        }
    }

    #[test]
    fn v2_projection_detail_overrides_a_disagreeing_base_status() {
        let detailed = DetailedTaskV2::new(
            minimal(TaskStatus::Working),
            TaskDetailV2::Failed {
                error: json!({ "code": -32603 })
                    .as_object()
                    .expect("object")
                    .clone(),
            },
        );
        assert_eq!(detailed.task().status, TaskStatus::Failed);
        assert_eq!(detailed.to_wire_object()["status"], json!("failed"));
    }

    #[test]
    fn v2_projection_status_and_detail_agree_for_every_variant() {
        for detail in [
            TaskDetailV2::Working,
            TaskDetailV2::Cancelled,
            TaskDetailV2::InputRequired {
                input_requests: requests(),
            },
            TaskDetailV2::Completed {
                result: serde_json::Map::new(),
            },
            TaskDetailV2::Failed {
                error: serde_json::Map::new(),
            },
        ] {
            let expected = detail.status();
            let object = DetailedTaskV2::new(minimal(TaskStatus::Working), detail).to_wire_object();
            assert_eq!(
                object["status"],
                serde_json::to_value(expected).expect("status serializes")
            );
        }
    }

    #[test]
    fn v2_projection_round_trips_through_from_wire_value() {
        let original = DetailedTaskV2::new(
            minimal(TaskStatus::Working),
            TaskDetailV2::InputRequired {
                input_requests: requests(),
            },
        );
        let value = serde_json::to_value(&original).expect("serializes");
        let decoded = DetailedTaskV2::from_wire_value(&value).expect("decodes");
        // Compared on the WIRE OBJECT, which is the identity that matters for a
        // wire type (and the only one available — see the `PartialEq` note).
        assert_eq!(decoded.to_wire_object(), original.to_wire_object());
        assert_eq!(
            serde_json::to_string(&decoded).expect("re-serializes"),
            serde_json::to_string(&original).expect("serializes"),
            "a round trip must be byte-identical, not merely structurally equal"
        );
    }

    /// The status-directed decode REFUSES a variant missing its required key —
    /// the failure an untagged best-effort decode would swallow.
    #[test]
    fn v2_projection_decode_rejects_a_variant_missing_its_required_key() {
        for (status, key) in [
            ("completed", "result"),
            ("failed", "error"),
            ("input_required", "inputRequests"),
        ] {
            let value = json!({
                "taskId": "t-1",
                "status": status,
                "createdAt": "2026-07-28T00:00:00Z",
                "lastUpdatedAt": "2026-07-28T00:00:01Z",
                "ttlMs": null,
            });
            let err = DetailedTaskV2::from_wire_value(&value)
                .err()
                .unwrap_or_else(|| {
                    panic!("a {status} task carrying no {key} must be refused, not decoded")
                });
            assert!(
                err.contains(key),
                "the refusal must name the missing key `{key}`, got {err}"
            );
        }
    }

    /// The detail key the projection writes and the key the reserved-result-field
    /// registry grants the tasks dispatch are ONE string.
    ///
    /// If they ever diverged, the projection would emit a key the egress strips
    /// — the exact row-23 failure mode, re-introduced by a typo.
    #[test]
    fn v2_projection_input_requests_key_matches_the_reserved_registry_key() {
        assert_eq!(
            DETAIL_KEY_INPUT_REQUESTS,
            crate::types::mrtr::INPUT_REQUESTS_KEY
        );
    }

    #[test]
    fn v2_projection_from_v1_renames_ttl_and_poll_interval_without_touching_v1() {
        let v1 = Task::new("t-1", TaskStatus::Working)
            .with_timestamps("2026-07-28T00:00:00Z", "2026-07-28T00:00:01Z")
            .with_ttl(60_000)
            .with_poll_interval(2500)
            .with_status_message("running");
        let projected = TaskV2::from_v1(&v1);
        assert_eq!(projected.ttl_ms, Some(60_000));
        assert_eq!(projected.poll_interval_ms, Some(2500));
        assert_eq!(projected.task_id, "t-1");
        assert_eq!(projected.status_message.as_deref(), Some("running"));

        // The v1 type's OWN bytes are untouched by the projection existing.
        let v1_raw = serde_json::to_string(&v1).expect("v1 serializes");
        assert!(v1_raw.contains("\"ttl\":60000"), "{v1_raw}");
        assert!(v1_raw.contains("\"pollInterval\":2500"), "{v1_raw}");
        assert!(!v1_raw.contains("ttlMs"), "{v1_raw}");
        assert!(!v1_raw.contains("pollIntervalMs"), "{v1_raw}");
    }

    #[test]
    fn v2_projection_carries_the_pmcp_diagnostic_detail_extension() {
        let v1 = Task::new("t-1", TaskStatus::Working)
            .with_timestamps("2026-07-28T00:00:00Z", "2026-07-28T00:00:01Z")
            .with_diagnostic_detail("step-3 timed out");
        let raw = serde_json::to_string(&TaskV2::from_v1(&v1)).expect("serializes");
        assert!(
            raw.contains("\"diagnosticDetail\":\"step-3 timed out\""),
            "{raw}"
        );
    }
}
