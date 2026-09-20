//! Constants for MCP Tasks protocol meta keys and method names.
//!
//! These constants ensure consistent use of protocol-defined strings
//! across the crate. Method name constants match the JSON-RPC `method`
//! field values defined by the MCP 2025-11-25 Tasks specification.
//!
//! # Era split (Phase 114)
//!
//! Every constant here is a **v1 (2025-11-25)** value. MCP 2026-07-28 moved
//! tasks out of the core spec into the `io.modelcontextprotocol/tasks`
//! extension, which declares exactly three request methods — `tasks/get`,
//! `tasks/update` and `tasks/cancel`. [`METHOD_TASKS_LIST`] and
//! [`METHOD_TASKS_RESULT`] are ABSENT from it and are answered `-32601` on a
//! v2-negotiated request; [`METHOD_TASKS_STATUS_NOTIFICATION`] and
//! [`MODEL_IMMEDIATE_RESPONSE_META_KEY`] have no v2 counterpart at all. Nothing
//! here is deleted: v1 continues to serve all of it unchanged.

// === Meta Key Constants ===

/// Meta key for related-task metadata on `tasks/result` responses.
///
/// Per the MCP spec, this key links a tool result back to its
/// originating task: `{ "io.modelcontextprotocol/related-task": { "taskId": "..." } }`.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Meta key for model-immediate-response on `CreateTaskResult._meta`.
///
/// Per the MCP 2025-11-25 spec, this key provides an immediate result for the
/// model to use while the task continues running asynchronously.
///
/// # v1-only — there is NO v2 counterpart
///
/// This is a SEP-1686 **v1** concept. The `io.modelcontextprotocol/tasks`
/// extension schema vendored at `schema/vendored/ext-tasks/` (plan 114-01)
/// declares no equivalent key on any of its four result shapes, and the v2
/// `CreateTaskResult` is a FLAT `Result & Task` carrying only the task fields
/// plus `resultType: "task"` — it has no slot for a provisional model answer.
/// Do not infer from this constant's existence that a v2 caller may send or
/// expect the key; it is neither emitted nor read on the 2026-07-28 wire.
pub const MODEL_IMMEDIATE_RESPONSE_META_KEY: &str =
    "io.modelcontextprotocol/model-immediate-response";

// === Method Name Constants ===

/// JSON-RPC method name for retrieving a task's current status.
pub const METHOD_TASKS_GET: &str = "tasks/get";

/// JSON-RPC method name for retrieving a task's final result (blocks until terminal).
///
/// # v1-only — RETIRED on the 2026-07-28 wire
///
/// This method exists on MCP **2025-11-25 only**. It is ABSENT from the
/// `io.modelcontextprotocol/tasks` extension schema, so a `tasks/result` on a
/// v2-negotiated request is answered `-32601 METHOD_NOT_FOUND` by
/// `pmcp::server::task_dispatch` (plan 114-08, inventory row 38). v2 replaced it
/// by inlining `result` / `error` on the terminal `tasks/get` task, so the
/// second round trip has nothing left to do.
///
/// The constant is KEPT because v1 still serves the method unchanged.
pub const METHOD_TASKS_RESULT: &str = "tasks/result";

/// JSON-RPC method name for listing tasks (paginated).
///
/// # v1-only — RETIRED on the 2026-07-28 wire
///
/// This method exists on MCP **2025-11-25 only**. It is ABSENT from the
/// `io.modelcontextprotocol/tasks` extension schema, so a `tasks/list` on a
/// v2-negotiated request is answered `-32601 METHOD_NOT_FOUND` by
/// `pmcp::server::task_dispatch` (plan 114-08, inventory row 37). The removal is
/// named by the extension as a SECURITY improvement: with no enumeration
/// primitive a server cannot inadvertently leak the existence of one caller's
/// tasks to another.
///
/// The constant is KEPT because v1 still serves the method unchanged.
pub const METHOD_TASKS_LIST: &str = "tasks/list";

/// JSON-RPC method name for cancelling a task.
pub const METHOD_TASKS_CANCEL: &str = "tasks/cancel";

/// JSON-RPC method name for task status change notifications.
///
/// # v1-only — no v2 counterpart under this name
///
/// The v2 extension's optional push surface is `notifications/tasks`, not
/// `notifications/tasks/status`, and it is a spec **MAY** that pmcp declines
/// this phase (inventory row 36) — a v2 client polls `tasks/get` instead, or
/// subscribes via `subscriptions/listen` with `taskIds`. This constant is
/// therefore never emitted on a v2-negotiated connection.
pub const METHOD_TASKS_STATUS_NOTIFICATION: &str = "notifications/tasks/status";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_key_values() {
        assert_eq!(
            RELATED_TASK_META_KEY,
            "io.modelcontextprotocol/related-task"
        );
        assert_eq!(
            MODEL_IMMEDIATE_RESPONSE_META_KEY,
            "io.modelcontextprotocol/model-immediate-response"
        );
    }

    #[test]
    fn method_name_values() {
        assert_eq!(METHOD_TASKS_GET, "tasks/get");
        assert_eq!(METHOD_TASKS_RESULT, "tasks/result");
        assert_eq!(METHOD_TASKS_LIST, "tasks/list");
        assert_eq!(METHOD_TASKS_CANCEL, "tasks/cancel");
        assert_eq!(
            METHOD_TASKS_STATUS_NOTIFICATION,
            "notifications/tasks/status"
        );
    }
}
