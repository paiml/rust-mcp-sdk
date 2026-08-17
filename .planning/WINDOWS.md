---
schema_version: 1
open_count: 12
waived_count: 0
fixed_count: 4
total_count: 16
last_updated: 2026-08-17T22:20:39.960Z
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
| 4 | 118.2 | deviation | src/types/notifications.rs | 161 | LogMessageParams diverges from the 2026-07-28 schema: pmcp emits a required message and no data, the schema requires data and defines no message. 118.2-08 VERDICT: declared not fixed (breaking public-type change; the pinned suite validates no emitted notifications/message params). Mechanized by the_vendored_schema_requires_data_where_pmcp_emits_message. | fixed |  | 2026-08-17T11:17:30.173Z | 2026-08-17T13:35:18.978Z |
| 5 | 118.2 | unmet-truth | src/server/streamable_http_server.rs | 1677 | SEP-2575: on v2 a request with no _meta logLevel still gets notifications/message, because resolve_request_log_level returns None and DEFAULT_LOG_LEVEL (info) applies. Measured 118.2-09 RED mutation 2. Fixture guards; SDK does not. | open |  | 2026-08-17T11:41:50.564Z |  |
| 6 | 118.2 | unmet-truth | crates/pmcp-team-servers/tests/era_matrix.rs | 776 | deprecated_capabilities_complete_under_both_eras still asserts no-live-stream, not completed: phase 118.2 fixed DELIVERY of the server-to-client request but a pmcp client cannot ANSWER one issued during its own call (parked inside transport.send while the server holds the tools/call POST). Detail moved from 'Dispatch oneshot channel closed' (0.18s) to 'Server request dispatch-1 timed out' (~30s). | open |  | 2026-08-17T12:11:22.433Z |  |
| 7 | 118.2 | deviation | .planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/deferred-items.md |  | 118.2-10 deferred the Client-level notification observation API: Client::notification_tx is None at src/client/mod.rs:406/:453/:496, ClientBuilder has no setter, the forwarding branch at :3764 is dead. A pmcp::Client consumer still cannot see notifications/message without dropping to Transport::receive(). | open |  | 2026-08-17T12:11:22.529Z |  |
| 8 | 118.2 | unmet-truth | src/types/notifications.rs | 168 | LogMessageParams emits required 'message' and omits optional 'data'; the spec requires 'data' and has no 'message'. Measured 118.2-11: official suite 2025-11-25:tools-call-with-logging went 1/1 -> 0/2 (WireSchemaValid rejects all 3 frames; the reference zod client drops them so logCount=0). GAP_ATTRIBUTABLE_FAILURES 1 -> 2. Refutes 118.2-08's 'no scenario validates emitted params'. | fixed |  | 2026-08-17T12:28:31.523Z | 2026-08-17T13:44:06.553Z |
| 9 | 118.2 | unmet-truth | src/client/mod.rs |  | MEASURED FLAKE: official suite 2025-11-25:tools-call-elicitation failed 1 of 9 fresh runs (118.2-11 re-measurement, held pin 0.2.0-alpha.11) with 'MCP error -32603: Dispatch oneshot channel closed' -- the same server-to-client request-lifecycle race as entry 6, here against the reference client. Already gate-fatal via BLOCKING_GREEN_SCENARIOS at pre-hardening settings, so the D-16 widening adds no new exposure. Nothing softened to accommodate it. | open |  | 2026-08-17T13:44:13.623Z |  |
| 10 | 118.2 | deviation | .planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/118.2-15-PLAN.md | 391 | pmat verify line uses .violations[]/.path, which does not exist in pmat 3.15.0 (correct: .summary.violations[]/.file with a ./ prefix); the command errors instead of measuring | open |  | 2026-08-17T20:14:57.119Z |  |
| 11 | 118.2 | deviation | .planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/118.2-17-PLAN.md | 341 | same broken pmat jq path as 118.2-15-PLAN.md:391 | open |  | 2026-08-17T20:14:57.208Z |  |
| 12 | 118.2 | deviation | src/shared/streamable_http.rs |  | BEHAVIOUR CHANGE, by design (118.2-15, CR-02 half 1): Transport::receive()'s terminal reason is now STICKY. OLD BEHAVIOUR: a terminal stream reason was pushed once onto the response mpsc, so it reached exactly ONE caller and every later caller blocked forever. NEW BEHAVIOUR: the reason is latched write-once on the transport and returned by EVERY subsequent receive() call, immediately. CONSUMER CONSEQUENCE: a consumer that loops on receive() and merely LOGS errors will now SPIN rather than hang. Mitigation already in the tree: Transport::receive's rustdoc carries the heading 'The terminal reason is STICKY - stop on it, do not loop', stating that the contract is to STOP on a terminal error. Sticky was chosen over one-shot deliberately - a one-shot reason restores the exact CR-02 hazard the latch exists to remove, where the error is consumed by whichever caller happened to be next and every caller after that gets an unexplained hang. T-118.2-15-04. Invisible to cargo semver-checks: the latch is a private field, so 223/223 checks pass and this disclosure is the only place a consumer learns of it. | open |  | 2026-08-17T22:19:42.169Z |  |
| 13 | 118.2 | deviation | src/client/mod.rs |  | BEHAVIOUR CHANGE (118.2-15, CR-02 half 2): Client::dispatch_request now compares response.id against the request_id it is awaiting and keeps looping on a mismatch; the mismatched frame is DISCARDED, because a Transport consumer holds no producer handle to re-queue it and an orphan holding pen would add its own unbounded-growth question. CONSUMER CONSEQUENCE: under concurrent calls on ONE Client, the request that frame belonged to now waits out its caller's own timeout instead of receiving a wrong answer - strictly better than one caller silently receiving another caller's tool result (a cross-request data leak inside one process), and still a real cost, accepted as T-118.2-15-03. A LENIENT SERVER that re-types the id - a JSON string where the client sent a number - now gets a wait too, because RequestId equality is TYPED and structural and JSON-RPC 2.0 requires the response id to be the same VALUE as the request's. FOLLOW-UP that removes the cost: per-id response ROUTING (active_requests carrying a response channel per id), recorded in this phase's deferred-items.md under '## DEFERRED (118.2-15): per-id response ROUTING, and the cost of discard-on-mismatch'. NOT the client lifecycle deadlock: entry 6 is a DIFFERENT defect (dispatch_request awaits transport.send before entering its receive loop while the server holds the POST open) and this edit deliberately did not touch that send-then-receive ordering - entry 6 stands unchanged and remains open. | open |  | 2026-08-17T22:19:53.731Z |  |
| 14 | 118.2 | unmet-truth | src/shared/streamable_http.rs |  | The gap-closure round (118.2-14..18) fixed CR-01, CR-02, WR-01 and WR-02 and DECLINES four review Warnings; disclosed here with their consequences so a reader scanning this ledger does not have to infer them from a plan summary. WR-03: start_sse's TOCTOU on abort_handle can leave a second, untracked GET session reader, so a client sees each server-initiated message TWICE - narrowed by 118.2-17, since the two readers can no longer poison each other's reconnect cursor (each owns a local) and both are now stoppable, so it is duplicate delivery only. WR-04: a reconnect GET gets no 401 refresh, so when a bearer token expires and the gateway drops the idle session stream, the reissued GET is answered 401, the reconnect loop treats it as terminal, and the session stream is lost PERMANENTLY - sampling/roots/elicitation dead with no recovery short of rebuilding the client - while POSTs on the same transport keep working. WR-05: an ordinary EOF on the session stream escalates to an application-visible Err, contradicting the receive() taxonomy row that promises the reader 'exits silently'; 118.2-15 rewrote how a terminal reason is DELIVERED while leaving that row exactly as it stood, so the divergence is untouched rather than half-fixed. WR-06: logging/setLevel reports success while silently discarding the level on stateless v1 deployments and inside a JSON-RPC batch, so a client that asked for 'error' keeps receiving 'info' chatter it explicitly declined; explicitly OUT OF SCOPE for this closure because fixing it reopens CONF-10 territory that plans 118.2-07, 118.2-08 and 118.2-13 argued to a booked conclusion on locked decisions D-10..D-13. FULL RECORD, with each finding's 118.2-REVIEW.md anchor, source span, consequence and reason (and IN-01..IN-06 besides): this phase's deferred-items.md, section '## GAP-CLOSURE ROUND (2026-08-17)'. The recorded source spans predate this closure's edits to the same file - locate by symbol, not by line. | open |  | 2026-08-17T22:20:07.236Z |  |
| 15 | 118.2 | deviation | src/shared/streamable_http.rs |  | CR-01 (Critical; raised by 118.2-REVIEW.md, independently confirmed against the merged source by 118.2-VERIFICATION.md): a peer-supplied SSE 'retry: 0' was bounded only from ABOVE and the reconnect budget was refunded by any single delivered frame (if delivered { attempt = 0; }), so one frame per body drove pmcp's OWN client into an unbounded zero-delay reconnect loop - a remote-triggerable client-side DoS that also fetched a fresh auth_provider access token per iteration. FIXED by 118.2-14 with a two-sided delay bound (MIN_SSE_RECONNECT_DELAY, 500 ms) plus an uptime-gated budget (budget_reset_earned / RECONNECT_BUDGET_RESET_UPTIME, 30 s); both bounds are separately load-bearing. Fence: reconnect_with_one_delivered_frame_and_zero_retry_stays_bounded in binary(client_sse_stream), 20 run / 20 passed (target/118.2-17-green.log). RED at 65 'GET / HTTP/1.1' lines in 7.9 s against a 3-GET budget (target/118.2-14-red.log) - a measurement that cannot be retaken. | fixed |  | 2026-08-17T22:20:25.030Z | 2026-08-17T22:20:39.888Z |
| 16 | 118.2 | deviation | src/shared/streamable_http.rs |  | CR-02 (Critical; raised by 118.2-REVIEW.md, independently confirmed against the merged source by 118.2-VERIFICATION.md): every terminal stream reason rode the SAME mpsc<Result<TransportMessage>> the responses ride, so a reason raised while the application was idle failed the next, unrelated request; and src/client/mod.rs's dispatch_request returned on the first Response frame it popped with NO comparison of response.id, so one out-of-band queue entry desynchronised the FIFO permanently and call n+1 silently received call n's result. FIXED by 118.2-15, both halves: a write-once terminal-reason latch consulted only after the queue is drained, plus response-id correlation that keeps looping on a mismatch. Fences: an_idle_terminal_error_does_not_fail_the_next_unrelated_call and a_response_whose_id_does_not_match_is_not_returned_as_this_calls_answer in binary(client_sse_stream), 20 run / 20 passed. RED at a tools/call that SUCCEEDED on the wire yet reported the stale session-stream reconnect error, and at call 2 receiving the marker 'call-answer-1' (target/118.2-15-red.log); the latch alone is a HALF fix, measured at 17 run / 16 passed / 1 failed (target/118.2-15-latch-only.log). The two consumer-observable COSTS of this fix are disclosed as separate OPEN entries in this ledger: the sticky receive() reason, and discard-on-mismatch. | fixed |  | 2026-08-17T22:20:25.106Z | 2026-08-17T22:20:39.960Z |

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
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-17T11:17:30.173Z",
    "resolved_at": "2026-08-17T13:35:18.978Z"
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
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-17T12:28:31.523Z",
    "resolved_at": "2026-08-17T13:44:06.553Z"
  },
  {
    "id": 9,
    "kind": "unmet-truth",
    "phase": "118.2",
    "file": "src/client/mod.rs",
    "line": null,
    "description": "MEASURED FLAKE: official suite 2025-11-25:tools-call-elicitation failed 1 of 9 fresh runs (118.2-11 re-measurement, held pin 0.2.0-alpha.11) with 'MCP error -32603: Dispatch oneshot channel closed' -- the same server-to-client request-lifecycle race as entry 6, here against the reference client. Already gate-fatal via BLOCKING_GREEN_SCENARIOS at pre-hardening settings, so the D-16 widening adds no new exposure. Nothing softened to accommodate it.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T13:44:13.623Z",
    "resolved_at": null
  },
  {
    "id": 10,
    "kind": "deviation",
    "phase": "118.2",
    "file": ".planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/118.2-15-PLAN.md",
    "line": 391,
    "description": "pmat verify line uses .violations[]/.path, which does not exist in pmat 3.15.0 (correct: .summary.violations[]/.file with a ./ prefix); the command errors instead of measuring",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T20:14:57.119Z",
    "resolved_at": null
  },
  {
    "id": 11,
    "kind": "deviation",
    "phase": "118.2",
    "file": ".planning/phases/118.2-the-v1-client-sse-transport-and-the-notifications-message-em/118.2-17-PLAN.md",
    "line": 341,
    "description": "same broken pmat jq path as 118.2-15-PLAN.md:391",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T20:14:57.208Z",
    "resolved_at": null
  },
  {
    "id": 12,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "BEHAVIOUR CHANGE, by design (118.2-15, CR-02 half 1): Transport::receive()'s terminal reason is now STICKY. OLD BEHAVIOUR: a terminal stream reason was pushed once onto the response mpsc, so it reached exactly ONE caller and every later caller blocked forever. NEW BEHAVIOUR: the reason is latched write-once on the transport and returned by EVERY subsequent receive() call, immediately. CONSUMER CONSEQUENCE: a consumer that loops on receive() and merely LOGS errors will now SPIN rather than hang. Mitigation already in the tree: Transport::receive's rustdoc carries the heading 'The terminal reason is STICKY - stop on it, do not loop', stating that the contract is to STOP on a terminal error. Sticky was chosen over one-shot deliberately - a one-shot reason restores the exact CR-02 hazard the latch exists to remove, where the error is consumed by whichever caller happened to be next and every caller after that gets an unexplained hang. T-118.2-15-04. Invisible to cargo semver-checks: the latch is a private field, so 223/223 checks pass and this disclosure is the only place a consumer learns of it.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T22:19:42.169Z",
    "resolved_at": null
  },
  {
    "id": 13,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/client/mod.rs",
    "line": null,
    "description": "BEHAVIOUR CHANGE (118.2-15, CR-02 half 2): Client::dispatch_request now compares response.id against the request_id it is awaiting and keeps looping on a mismatch; the mismatched frame is DISCARDED, because a Transport consumer holds no producer handle to re-queue it and an orphan holding pen would add its own unbounded-growth question. CONSUMER CONSEQUENCE: under concurrent calls on ONE Client, the request that frame belonged to now waits out its caller's own timeout instead of receiving a wrong answer - strictly better than one caller silently receiving another caller's tool result (a cross-request data leak inside one process), and still a real cost, accepted as T-118.2-15-03. A LENIENT SERVER that re-types the id - a JSON string where the client sent a number - now gets a wait too, because RequestId equality is TYPED and structural and JSON-RPC 2.0 requires the response id to be the same VALUE as the request's. FOLLOW-UP that removes the cost: per-id response ROUTING (active_requests carrying a response channel per id), recorded in this phase's deferred-items.md under '## DEFERRED (118.2-15): per-id response ROUTING, and the cost of discard-on-mismatch'. NOT the client lifecycle deadlock: entry 6 is a DIFFERENT defect (dispatch_request awaits transport.send before entering its receive loop while the server holds the POST open) and this edit deliberately did not touch that send-then-receive ordering - entry 6 stands unchanged and remains open.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T22:19:53.731Z",
    "resolved_at": null
  },
  {
    "id": 14,
    "kind": "unmet-truth",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "The gap-closure round (118.2-14..18) fixed CR-01, CR-02, WR-01 and WR-02 and DECLINES four review Warnings; disclosed here with their consequences so a reader scanning this ledger does not have to infer them from a plan summary. WR-03: start_sse's TOCTOU on abort_handle can leave a second, untracked GET session reader, so a client sees each server-initiated message TWICE - narrowed by 118.2-17, since the two readers can no longer poison each other's reconnect cursor (each owns a local) and both are now stoppable, so it is duplicate delivery only. WR-04: a reconnect GET gets no 401 refresh, so when a bearer token expires and the gateway drops the idle session stream, the reissued GET is answered 401, the reconnect loop treats it as terminal, and the session stream is lost PERMANENTLY - sampling/roots/elicitation dead with no recovery short of rebuilding the client - while POSTs on the same transport keep working. WR-05: an ordinary EOF on the session stream escalates to an application-visible Err, contradicting the receive() taxonomy row that promises the reader 'exits silently'; 118.2-15 rewrote how a terminal reason is DELIVERED while leaving that row exactly as it stood, so the divergence is untouched rather than half-fixed. WR-06: logging/setLevel reports success while silently discarding the level on stateless v1 deployments and inside a JSON-RPC batch, so a client that asked for 'error' keeps receiving 'info' chatter it explicitly declined; explicitly OUT OF SCOPE for this closure because fixing it reopens CONF-10 territory that plans 118.2-07, 118.2-08 and 118.2-13 argued to a booked conclusion on locked decisions D-10..D-13. FULL RECORD, with each finding's 118.2-REVIEW.md anchor, source span, consequence and reason (and IN-01..IN-06 besides): this phase's deferred-items.md, section '## GAP-CLOSURE ROUND (2026-08-17)'. The recorded source spans predate this closure's edits to the same file - locate by symbol, not by line.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T22:20:07.236Z",
    "resolved_at": null
  },
  {
    "id": 15,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "CR-01 (Critical; raised by 118.2-REVIEW.md, independently confirmed against the merged source by 118.2-VERIFICATION.md): a peer-supplied SSE 'retry: 0' was bounded only from ABOVE and the reconnect budget was refunded by any single delivered frame (if delivered { attempt = 0; }), so one frame per body drove pmcp's OWN client into an unbounded zero-delay reconnect loop - a remote-triggerable client-side DoS that also fetched a fresh auth_provider access token per iteration. FIXED by 118.2-14 with a two-sided delay bound (MIN_SSE_RECONNECT_DELAY, 500 ms) plus an uptime-gated budget (budget_reset_earned / RECONNECT_BUDGET_RESET_UPTIME, 30 s); both bounds are separately load-bearing. Fence: reconnect_with_one_delivered_frame_and_zero_retry_stays_bounded in binary(client_sse_stream), 20 run / 20 passed (target/118.2-17-green.log). RED at 65 'GET / HTTP/1.1' lines in 7.9 s against a 3-GET budget (target/118.2-14-red.log) - a measurement that cannot be retaken.",
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-17T22:20:25.030Z",
    "resolved_at": "2026-08-17T22:20:39.888Z"
  },
  {
    "id": 16,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "CR-02 (Critical; raised by 118.2-REVIEW.md, independently confirmed against the merged source by 118.2-VERIFICATION.md): every terminal stream reason rode the SAME mpsc<Result<TransportMessage>> the responses ride, so a reason raised while the application was idle failed the next, unrelated request; and src/client/mod.rs's dispatch_request returned on the first Response frame it popped with NO comparison of response.id, so one out-of-band queue entry desynchronised the FIFO permanently and call n+1 silently received call n's result. FIXED by 118.2-15, both halves: a write-once terminal-reason latch consulted only after the queue is drained, plus response-id correlation that keeps looping on a mismatch. Fences: an_idle_terminal_error_does_not_fail_the_next_unrelated_call and a_response_whose_id_does_not_match_is_not_returned_as_this_calls_answer in binary(client_sse_stream), 20 run / 20 passed. RED at a tools/call that SUCCEEDED on the wire yet reported the stale session-stream reconnect error, and at call 2 receiving the marker 'call-answer-1' (target/118.2-15-red.log); the latch alone is a HALF fix, measured at 17 run / 16 passed / 1 failed (target/118.2-15-latch-only.log). The two consumer-observable COSTS of this fix are disclosed as separate OPEN entries in this ledger: the sticky receive() reason, and discard-on-mismatch.",
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-17T22:20:25.106Z",
    "resolved_at": "2026-08-17T22:20:39.960Z"
  }
]
````
