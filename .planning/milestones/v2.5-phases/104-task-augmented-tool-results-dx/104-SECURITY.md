---
phase: 104
slug: task-augmented-tool-results-dx
status: secured
threats_open: 0
threats_closed: 22
asvs_level: 1
created: 2026-07-04
---

# SECURITY.md — Phase 104: Task-Augmented Tool-Result DX

**Audit date:** 2026-07-04
**Auditor:** gsd-security-auditor
**Register:** Phase 104 STRIDE (22 threats, `register_authored_at_plan_time: true`)
**Config:** asvs_level=default, block_on=default
**Result:** SECURED — 22/22 CLOSED, 0 OPEN, 0 unregistered flags

Every declared mitigation was verified by locating the actual code / documentation
artifact, not by accepting intent. Both native dispatchers (`Server` in
`src/server/mod.rs` and `ServerCore` in `src/server/core.rs`) were checked
independently for each dispatch-level mitigation (no single-entry-point acceptance).

---

## Threat Verification

### Plan 104-01 — Client DX

| Threat | Category | Disposition | Evidence |
|--------|----------|-------------|----------|
| T-104-01-01 | Information Disclosure | mitigate | `Client::wait_for_task` composes `tasks_get`/`tasks_result` (`src/client/mod.rs:696,735`); owner derived from auth/router NEVER client params — IDOR-safe (`src/server/task_dispatch.rs:352-354`), `tasks/result` scoped by `owner_id` (`:538-545`). |
| T-104-01-02 | DoS | mitigate | `web_time::Instant` (`src/client/mod.rs:694`), 50 ms interval floor `MIN_POLL_MS` (`:716`), `max_poll_duration` budget clamp with `saturating_mul/sub` + sleep clamped to remaining budget (`:723-730`, WR-01), `InputRequired` early-return (`:705-710`, CR-01). |
| T-104-01-03 | Tampering | mitigate | `related_task()` accessor uses `serde_json::from_value(..).ok()` → `None` on malformed, never panics (`src/types/tools.rs:665`). |

### Plan 104-02 — ToolOutput verbatim pass-through

| Threat | Category | Disposition | Evidence |
|--------|----------|-------------|----------|
| T-104-02-01 | Spoofing/Tampering | mitigate | `Verbatim` arm early-returns BEFORE the create-path gate in both dispatchers (`src/server/mod.rs:1536-1538` returns before `:1557-1601`; `src/server/core.rs:565-566` returns before `:628-666`); store id is minted server-side, tool `taskId` never trusted. |
| T-104-02-02 | Tampering | mitigate | REQUEST middleware runs at `src/server/mod.rs:1472-1476` and auth context bound at `:1448`, BOTH before `handle_output` at `:1482`; core parity `:538` before `:556`. |
| T-104-02-03 | Information Disclosure | mitigate (USER-APPROVED, LOCKED — D-04a) | Loud `⚠️ BYPASS WARNING` rustdoc on `ToolOutput::Result` (`src/server/mod.rs:252-301`); regression battery in `tests/tool_output_passthrough.rs` (response-bypass + request-still-runs + error-path on both dispatchers). |
| T-104-02-04 | Tampering | mitigate | Single shared `resolve_tool_output`/`DispatchOutput` decision (`src/server/task_dispatch.rs:185-190`) consumed identically by both dispatchers (`mod.rs:1483`, `core.rs:557`); Server-vs-ServerCore parity test in `tests/tool_output_passthrough.rs`. |
| T-104-02-05 | Tampering | mitigate | Workflow-internal tool steps stay on `handler.handle()` (`src/server/core.rs:746,928`), never `handle_output`; grep of `src/server/workflow/` shows zero `handle_output` references. |

### Plan 104-03 — Double-wrap tripwire

| Threat | Category | Disposition | Evidence |
|--------|----------|-------------|----------|
| T-104-03-01 | DoS | mitigate | `debug_assert!(false, ..)` (NOT `assert!`) so release compiles the hard-fail out and only `tracing::warn!` fires (`src/server/task_dispatch.rs:313-329`); release-mode test run in `tests/double_wrap_tripwire.rs`. |
| T-104-03-02 | DoS | mitigate | Cheap `_meta[related-task]` single-lookup short-circuits first (`src/server/task_dispatch.rs:250-256`); content scan is O(len) with `.all()` first-failure stop, no full parse; envelope-only key set `RESULT_ENVELOPE_KEYS` (`:246,262-263`, WR-02 precision fix). |
| T-104-03-03 | Tampering | mitigate | Single `suppress_double_wrap` `HashSet` threaded into `ServerCore` via `with_suppress_double_wrap` (`src/server/core.rs:418`); both dispatchers call the same `double_wrap_tripwire` (`mod.rs:1609`, `core.rs:675`); suppression-parity test in `tests/double_wrap_tripwire.rs`. |
| T-104-03-SC | Tampering | mitigate | Cargo.toml diff over the phase range adds only the `s47` `[[example]]` block — zero dependency lines; `proptest` already a dev-dependency. |

### Plan 104-04 — tool_with_result + set_result_meta

| Threat | Category | Disposition | Evidence |
|--------|----------|-------------|----------|
| T-104-04-01 | Tampering | mitigate | `result_meta_handle()` (`Arc::clone`) taken BEFORE `extra` moves into `handle_output` and drained after in both dispatchers (`src/server/mod.rs:1456,1629`; `src/server/core.rs:520,698`); slot survives Clone like `peer` (`src/server/cancellation.rs:242,417`). |
| T-104-04-02 | Information Disclosure | accept | See Accepted Risks Log AR-1. |
| T-104-04-03 | DoS | mitigate | `std::sync::Mutex` (not tokio); guard acquired, mutated, dropped synchronously — never held across `await` (`src/server/cancellation.rs:398-407,539-545`); poisoning recovered via `PoisonError::into_inner` (`:402,543`). |
| T-104-04-04 | Tampering | mitigate | `merge_result_meta` inserts per-key onto `get_or_insert_with` map — handler-key-wins, never whole-map replace (`src/server/cancellation.rs:554-565`); `set_result_meta` merges likewise (`:398-407`); collision test in `tests/tool_with_result.rs`. |
| T-104-04-SC | Tampering | mitigate | No new dependencies (Cargo.toml diff = example block only). |

### Plan 104-05 — Migration guide + acceptance gate

| Threat | Category | Disposition | Evidence |
|--------|----------|-------------|----------|
| T-104-05-01 | Tampering | mitigate | D-14 live-HTTP acceptance gate reads RAW JSON-RPC result off the transport and asserts `_meta[related-task]` at top level (`tests/tool_output_result_http.rs`); registered as a CI test file. |
| T-104-05-02 | Information Disclosure | accept | See Accepted Risks Log AR-2. |
| T-104-05-03 | DoS | mitigate | Ephemeral `127.0.0.1:0` bind + readiness via bound listener + `JoinHandle::abort()` shutdown (`tests/tool_output_result_http.rs:23-26,100-105,149`). |
| T-104-05-04 | Information Disclosure | mitigate | D-04a bypass callout present in `docs/design/sep-1686-task-augmented-results.md`, `pmcp-book/src/task-augmented-results.md`, and loud rustdoc on `ToolOutput::Result`/`tool_with_result` (`src/server/mod.rs:252-301,2841-2922`). |
| T-104-05-SC | Tampering | mitigate | Cargo.toml diff confirms zero new dependencies; `web-time` was already in-tree (`Cargo.toml:97`). |

---

## Accepted Risks Log

**AR-1 — T-104-04-02 (Information Disclosure): handler injects cross-owner data via `set_result_meta`.**
Accepted. `set_result_meta` writes handler-authored content into the result `_meta`
at the SAME trust level as the value a handler already returns from `handle()`. The
handler is trusted server code; nothing new is exposed that a handler could not
already place in its returned `Value`. Documented at the SDK-API level in
`docs/design/sep-1686-task-augmented-results.md:183` ("...at the same trust level
as..."). No additional control required.

**AR-2 — T-104-05-02 (Information Disclosure): migration guide leaks internal incident detail.**
Accepted. The guide references the upstream bug class ("five variants of this one
bug", `docs/design/sep-1686-task-augmented-results.md:34`) reframed at the SDK-API
level — no credentials, tokens, hostnames, customer data, or exploit specifics. The
narrative exists to justify the tripwire/verbatim design. No secrets present.

---

## Unregistered Flags

None. SUMMARY.md `## Threat Flags` sections for plans 01 and 05 explicitly declare
"None"; plans 02/03/04 introduced no new network endpoints, auth paths, or trust
boundaries (03 maps its surface to T-104-03-* in a Threat Model Coverage section).
No new attack surface appeared during implementation without a mapped threat ID.

---

## Post-plan review linkage (verified, not merely cited)

- CR-01 (`wait_for_task` hang on `input_required`) — fix present at `src/client/mod.rs:705-710`.
- WR-01 (timeout overshoot) — budget clamp present at `src/client/mod.rs:723-730`.
- WR-02 (tripwire false-positive on chat payloads) — envelope-only key gate present at `src/server/task_dispatch.rs:246,262-263`.

All three strengthen T-104-01-02 / T-104-03-02 and were confirmed live in the code.
