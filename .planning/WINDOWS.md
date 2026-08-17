---
schema_version: 1
open_count: 8
waived_count: 0
fixed_count: 0
total_count: 8
last_updated: 2026-08-17T12:28:31.523Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 118.2 | deviation | src/shared/streamable_http.rs |  | 118.2-03 rewrote two collected_body_cap unit tests that measured a whole-body cap the POST SSE path no longer has (parser bound + receive() refusal is the new contract) | open |  | 2026-08-17T08:07:00.553Z |  |
| 2 | 118.2 | deviation | src/shared/streamable_http.rs |  | decode_sse_chunks_for_fuzz carries an annotated #[allow(clippy::type_complexity)] on its four-tuple return; a pub type alias was rejected to keep this plan's public-API addition at exactly one item | open |  | 2026-08-17T09:51:32.637Z |  |
| 3 | 118.2 | deviation | src/server/streamable_http_server.rs |  | A malformed v1 logging/setLevel is rejected 400/-32601 by the pre-existing typed parse rather than answered {} as plan 118.2-07 predicted; the v2 _meta half of the claim holds | open |  | 2026-08-17T10:29:37.425Z |  |
| 4 | 118.2 | deviation | src/types/notifications.rs | 161 | LogMessageParams diverges from the 2026-07-28 schema: pmcp emits a required message and no data, the schema requires data and defines no message. 118.2-08 VERDICT: declared not fixed (breaking public-type change; the pinned suite validates no emitted notifications/message params). Mechanized by the_vendored_schema_requires_data_where_pmcp_emits_message. | open |  | 2026-08-17T11:17:30.173Z |  |
| 5 | 118.2 | unmet-truth | src/server/streamable_http_server.rs | 1677 | SEP-2575: on v2 a request with no _meta logLevel still gets notifications/message, because resolve_request_log_level returns None and DEFAULT_LOG_LEVEL (info) applies. Measured 118.2-09 RED mutation 2. Fixture guards; SDK does not. | open |  | 2026-08-17T11:41:50.564Z |  |
| 6 | 118.2 | unmet-truth | crates/pmcp-team-servers/tests/era_matrix.rs | 776 | deprecated_capabilities_complete_under_both_eras still asserts no-live-stream, not completed: phase 118.2 fixed DELIVERY of the server-to-client request but a pmcp client cannot ANSWER one issued during its own call (parked inside transport.send while the server holds the tools/call POST). Detail moved from 'Dispatch oneshot channel closed' (0.18s) to 'Server request dispatch-1 timed out' (~30s). | open |  | 2026-08-17T12:11:22.433Z |  |
| 7 | 118.2 | deviation | .planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/deferred-items.md |  | 118.2-10 deferred the Client-level notification observation API: Client::notification_tx is None at src/client/mod.rs:406/:453/:496, ClientBuilder has no setter, the forwarding branch at :3764 is dead. A pmcp::Client consumer still cannot see notifications/message without dropping to Transport::receive(). | open |  | 2026-08-17T12:11:22.529Z |  |
| 8 | 118.2 | unmet-truth | src/types/notifications.rs | 168 | LogMessageParams emits required 'message' and omits optional 'data'; the spec requires 'data' and has no 'message'. Measured 118.2-11: official suite 2025-11-25:tools-call-with-logging went 1/1 -> 0/2 (WireSchemaValid rejects all 3 frames; the reference zod client drops them so logCount=0). GAP_ATTRIBUTABLE_FAILURES 1 -> 2. Refutes 118.2-08's 'no scenario validates emitted params'. | open |  | 2026-08-17T12:28:31.523Z |  |

````json
[
  {
    "id": 1,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "118.2-03 rewrote two collected_body_cap unit tests that measured a whole-body cap the POST SSE path no longer has (parser bound + receive() refusal is the new contract)",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T08:07:00.553Z",
    "resolved_at": null
  },
  {
    "id": 2,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "decode_sse_chunks_for_fuzz carries an annotated #[allow(clippy::type_complexity)] on its four-tuple return; a pub type alias was rejected to keep this plan's public-API addition at exactly one item",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T09:51:32.637Z",
    "resolved_at": null
  },
  {
    "id": 3,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/server/streamable_http_server.rs",
    "line": null,
    "description": "A malformed v1 logging/setLevel is rejected 400/-32601 by the pre-existing typed parse rather than answered {} as plan 118.2-07 predicted; the v2 _meta half of the claim holds",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T10:29:37.425Z",
    "resolved_at": null
  },
  {
    "id": 4,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/types/notifications.rs",
    "line": 161,
    "description": "LogMessageParams diverges from the 2026-07-28 schema: pmcp emits a required message and no data, the schema requires data and defines no message. 118.2-08 VERDICT: declared not fixed (breaking public-type change; the pinned suite validates no emitted notifications/message params). Mechanized by the_vendored_schema_requires_data_where_pmcp_emits_message.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T11:17:30.173Z",
    "resolved_at": null
  },
  {
    "id": 5,
    "kind": "unmet-truth",
    "phase": "118.2",
    "file": "src/server/streamable_http_server.rs",
    "line": 1677,
    "description": "SEP-2575: on v2 a request with no _meta logLevel still gets notifications/message, because resolve_request_log_level returns None and DEFAULT_LOG_LEVEL (info) applies. Measured 118.2-09 RED mutation 2. Fixture guards; SDK does not.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T11:41:50.564Z",
    "resolved_at": null
  },
  {
    "id": 6,
    "kind": "unmet-truth",
    "phase": "118.2",
    "file": "crates/pmcp-team-servers/tests/era_matrix.rs",
    "line": 776,
    "description": "deprecated_capabilities_complete_under_both_eras still asserts no-live-stream, not completed: phase 118.2 fixed DELIVERY of the server-to-client request but a pmcp client cannot ANSWER one issued during its own call (parked inside transport.send while the server holds the tools/call POST). Detail moved from 'Dispatch oneshot channel closed' (0.18s) to 'Server request dispatch-1 timed out' (~30s).",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T12:11:22.433Z",
    "resolved_at": null
  },
  {
    "id": 7,
    "kind": "deviation",
    "phase": "118.2",
    "file": ".planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/deferred-items.md",
    "line": null,
    "description": "118.2-10 deferred the Client-level notification observation API: Client::notification_tx is None at src/client/mod.rs:406/:453/:496, ClientBuilder has no setter, the forwarding branch at :3764 is dead. A pmcp::Client consumer still cannot see notifications/message without dropping to Transport::receive().",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T12:11:22.529Z",
    "resolved_at": null
  },
  {
    "id": 8,
    "kind": "unmet-truth",
    "phase": "118.2",
    "file": "src/types/notifications.rs",
    "line": 168,
    "description": "LogMessageParams emits required 'message' and omits optional 'data'; the spec requires 'data' and has no 'message'. Measured 118.2-11: official suite 2025-11-25:tools-call-with-logging went 1/1 -> 0/2 (WireSchemaValid rejects all 3 frames; the reference zod client drops them so logCount=0). GAP_ATTRIBUTABLE_FAILURES 1 -> 2. Refutes 118.2-08's 'no scenario validates emitted params'.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T12:28:31.523Z",
    "resolved_at": null
  }
]
````
