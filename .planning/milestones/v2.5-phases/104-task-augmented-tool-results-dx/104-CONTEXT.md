# Phase 104: Task-Augmented Tool Results DX (SEP-1686 junction) - Context

**Gathered:** 2026-07-04
**Amended:** 2026-07-04 (D-04a added after cross-AI review — user-approved bypass hardening)
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the junction between the tool contract and the tasks layer so a tool can
return a full `CallToolResult` — `_meta` included — through the normal `Server`
dispatch front door, instead of dispatch stringifying it into `content[0].text`
(the unconditional text-wrap at `src/server/mod.rs:1493`). Four deliverables,
sourced from the pmcp.run team's incident report (5 variants of one silent
wire-shape bug class, incl. a 2-week silent production outage):

1. **TOUT-01** — typed tool output: an explicit, additive way for any
   `ToolHandler` to return a full `CallToolResult` that reaches the wire
   un-re-wrapped.
2. **TOUT-02** — double-wrap tripwire: dispatch warns (and debug-fails) when
   about to text-wrap a `Value` that structurally looks like an already-built
   `CallToolResult`.
3. **TOUT-03** — client-side task detection owned by the SDK:
   `related_task()` accessor + a `wait_for_task` polling convenience.
4. **TOUT-04** — migration guide from the three hand-rolled pre-2.11 `_meta`
   task patterns to native `with_task_store()` machinery.

Everything is additive: Phase 101 froze the `tasks/*` wire contract; no
breaking change to `ToolHandler`, `Server`/`ServerBuilder` public API, or wire
shapes.

</domain>

<decisions>
## Implementation Decisions

### Typed-output API shape (TOUT-01)
- **D-01:** Core mechanism is a `ToolOutput` enum —
  `ToolOutput::Payload(Value) | ToolOutput::Result(CallToolResult)` — exposed
  via a NEW default-implemented trait method (e.g.
  `ToolHandler::handle_output()`) that delegates to `handle()`. Fully
  additive; existing handlers untouched; works for hand-written `ToolHandler`
  impls (the pmcp.run case), not just builder closures.
- **D-02:** Implicit "returned Value parses as `CallToolResult` → pass
  through" sniffing is **REJECTED**. `CallToolResult`'s serde is fully
  defaulted (`#[serde(default)]` on `content`/`is_error`, unknown fields
  ignored), so ANY JSON object parses — implicit detection would silently
  swallow arbitrary payloads, recreating the bug class.
- **D-03:** Three sugar layers ALL ship alongside the enum:
  1. `CallToolResult::with_related_task(TaskMetadata)` builder method keyed by
     `RELATED_TASK_META_KEY` (server-emit twin of the client accessor);
  2. typed closure registration
     `ServerBuilder::tool_with_result(name, |args: T, extra| -> Result<CallToolResult>)`
     mirroring the existing `tool_typed()` precedent;
  3. `RequestHandlerExtra::set_result_meta()` — merges `_meta` onto the
     dispatch-built result (lowest-friction retrofit for existing handlers).
- **D-04:** Precedence: the Phase 102 create-path gate
  (`maybe_build_task_created`) keeps running FIRST — native task machinery and
  D-STORE-MINTS-ID remain un-bypassable (a `Result` output isn't task-shaped,
  so the gate naturally passes). Then `ToolOutput::Result(...)` goes to the
  wire verbatim: NO text-wrap, NO widget enrichment. The handler owns the full
  envelope.
- **D-04a (USER-APPROVED during cross-AI review, 2026-07-04 — LOCKED):**
  Codex flagged (HIGH) that `ToolOutput::Result` bypassing RESPONSE middleware
  (redaction/sanitization/audit) is a security-relevant escape path that D-04
  only implied. The user's decision: **"Keep the bypass, harden it."** The
  `ToolOutput::Result` response-middleware bypass is now an EXPLICITLY
  user-approved, locked decision — the handler owns the full envelope at the
  same trust level as today's raw `Value` return. There is deliberately NO
  result-aware response-middleware hook. The bypass ships WITH these four
  mandatory hardening mitigations:
  1. **Loud rustdoc** on the `ToolOutput::Result` variant AND on
     `tool_with_result` (not only the migration guide) stating that the value
     goes to the wire verbatim and bypasses response middleware — the tool is
     responsible for its OWN redaction/sanitization.
  2. A **request-middleware-still-runs test** proving REQUEST middleware
     (`process_request`) still fires before a `ToolOutput::Result` tool
     executes (only RESPONSE middleware is bypassed).
  3. A **handler-error-path regression test** proving handler errors returned
     from `handle_output()` still route through the existing error handling
     (`handle_tool_error` / the response-middleware error path), i.e. the
     bypass applies ONLY to the successful `Ok(ToolOutput::Result(_))` arm.
  4. A **migration-guide + rustdoc callout** that a
     `tool_with_result`/`ToolOutput::Result` tool is responsible for its own
     redaction (surfaced where authors read it, not buried).
  This amendment survives future replans: the bypass is settled, not open.
- **D-05:** The change lands in the SHARED task-dispatch seam (Phase 102
  anti-drift rule): `Server` and `ServerCore` dispatch must honor `ToolOutput`
  identically — no divergent second copy of the pass-through logic.

### Tripwire semantics (TOUT-02)
- **D-06:** Loudness: `tracing::warn!` in ALL builds PLUS `debug_assert!`
  hard-fail in debug builds. Release builds never panic. Rationale: "one local
  run would have caught a two-week outage" — a debug-build failure makes any
  local run or CI test catch it; the WARN still surfaces it in production
  logs.
- **D-07:** Heuristic (high-precision structural markers, NOT full
  deserialize): fire when the `Value` is an object with (a) a `content` array
  whose elements ALL deserialize as valid `Content` items, OR (b) `_meta`
  containing `RELATED_TASK_META_KEY`. Catches both the June hand-rolled class
  and the agent-lake double-wrap variant with near-zero false positives.
- **D-08:** Escape hatch: per-tool registration-time opt-out flag (e.g.
  `.suppress_double_wrap_check()`) for tools whose legitimate payload trips
  the heuristic. Explicit and reviewable; no env-var global kill switch.
  Suppression should be RARE and reviewed (rustdoc must say so), and the
  suppression set MUST survive the builder→`ServerCore` conversion so both
  dispatchers agree.

### Client accessor scope (TOUT-03)
- **D-09:** Ship BOTH the typed accessor
  `CallToolResult::related_task() -> Option<TaskMetadata>` (SEP-1686 detection
  of `_meta["io.modelcontextprotocol/related-task"]`) AND a
  `wait_for_task(task_id, opts)` client convenience that drives `tasks/get`
  polling until a terminal status, then fetches `tasks/result`. Honors
  `pollInterval`/`maxPollDurationSecs` from `TaskMetadata` with caller
  overrides. `wait_for_task` MUST compose directly with `TaskMetadata` (a
  `From<TaskMetadata> for WaitForTaskOptions` / `WaitForTaskOptions::from_metadata`
  or a `wait_for_related_task(meta, opts)` convenience) so a caller who has a
  `related_task()` result never hand-copies poll fields (cross-AI review,
  Plan 01 MEDIUM).
- **D-10:** `wait_for_task` must be wasm32-compatible: platform-abstracted
  delay via `crate::runtime::sleep` (tokio native / wasm-bindgen-futures on
  wasm — no new dep) AND a wasm-safe ELAPSED-TIME source for the timeout,
  `web_time::Instant` (precedent: `src/shared/middleware.rs:25`, Cargo.toml:97).
  The Phase 103 web-channel browser client is a direct consumer; its
  hand-rolled JS poll loop can shrink.
- **D-11:** A `Stream`-based poll API is rejected for now (bigger surface,
  more design risk) — can be added additively later.

### Migration guide shape (TOUT-04)
- **D-12:** Canonical guide = pmcp-book chapter (durable, user-facing) + a
  `docs/design/` companion recording the SEP-1686 junction rationale and the
  D-08/D-09 wire-compat confirmation (native `CreateTaskResult` carries
  `_meta[related-task]` with the store-minted id — `_meta`-sniffing clients
  detect it unchanged). README gets a short pointer. Course chapter deferred
  until the API has met users. The guide MUST also call out the D-04a
  response-middleware bypass semantics (a `ToolOutput::Result` tool owns its
  own redaction).
- **D-13:** One new numbered runnable example (next free slot, likely `s47`)
  showing BEFORE (hand-rolled `_meta` task tool) and AFTER (same tool on
  native `with_task_store()` + `ToolOutput`) — the diff IS the migration
  guide; it doubles as the ALWAYS-required example and as a regression harness
  proving `_meta`-sniffing clients detect both shapes. The BEFORE tool (which
  intentionally emits a `CallToolResult`-shaped `Value` on the Payload path)
  MUST be registered with `suppress_double_wrap_check()` so the demo does not
  trip the Plan 03 debug-assert; the AFTER tool MUST use a REAL store-minted
  task via `with_task_store()`, not a hand-minted task id, so the example
  teaches the native pattern (cross-AI review, Plan 05 MEDIUM).
- **D-14:** Acceptance gate for wire-shape correctness: extend the Phase 102
  live HTTP loopback harness (real `StreamableHttpServer` +
  `StreamableHttpTransport`) with a `ToolOutput::Result` tool, asserting the
  RAW wire JSON carries `_meta` at result top-level — a "_meta-sniffing
  client" regression test that consumes the REAL dispatch output (the note's
  ask #4: never hand-authored fixtures). In-repo, CI-enforced. A coordinated
  pmcp.run-side UAT is NOT a phase-closure gate (avoid coupling to an external
  team), but the guide invites it.

### Claude's Discretion
- Exact names (`ToolOutput`, `handle_output`, `tool_with_result`,
  `wait_for_task`, `suppress_double_wrap_check`) and module placement within
  `src/server/` / `src/client/` — provided the shared-seam rule (D-05) holds.
- Internal signature of the pass-through in the shared dispatch unit.
- WARN message contents (should include tool name and which marker fired).
- Wasm timer mechanism selection for D-10 (reuse `crate::runtime::sleep` +
  `web_time::Instant`; no new crate).
- Whether `set_result_meta` merges or overwrites on key collision — RESOLVED
  by cross-AI review to MERGE with precise precedence (D-03.3 / Plan 04):
  handler-set keys overwrite same-name keys, unrelated existing `_meta` keys
  (widget/native emission) are preserved.
- Test file organization (mirror `tests/tool_as_task_lifecycle.rs` / Phase 102
  HTTP harness conventions).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### The source issue (defines the bug class and the asks)
- `/Users/guy/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-tool-as-task-dx.md`
  — pmcp.run team incident report (2026-06-21 + 2026-07-04 addendum): 5
  incident variants, ranked asks 1–9, the agent-lake wire probe evidence, and
  the "live round-trip, not code reading" process lesson. THE primary spec for
  this phase. (Outside this repo — sibling checkout `../../pmcp-run/`.)

### The code under change (verified 2026-07-04 against pmcp 2.11.0)
- `src/server/mod.rs:1493` — the unconditional
  `result.to_string()` → `Content::text` wrap in `handle_call_tool`; the exact
  site TOUT-01/TOUT-02 change. Note the create-path gate call above it
  (`maybe_build_task_created`, mod.rs:1463-1490) which MUST keep precedence
  (D-04).
- `src/types/tools.rs:526` — `CallToolResult` (has `_meta` at :556; serde
  fully defaulted — the reason D-02 rejects implicit sniffing).
- `src/types/tasks.rs` — `RELATED_TASK_META_KEY` (:9), `TaskMetadata` (:193):
  the types D-03/D-07/D-09 build on.
- `src/client/mod.rs:496-601` — `call_tool_with_task` / `tasks_get` /
  `tasks_result`: the client surface `wait_for_task` composes.
- `src/server/core_tests.rs:855-896` — proof the native create-path emits
  `_meta[related-task]` with the store-minted id (D-08/D-09 of Phase 101/102)
  — the wire-compat fact the migration guide (D-12) documents.

### Prior-phase contracts that constrain this phase
- `.planning/phases/102-http-task-dispatch/102-RESEARCH.md` — the shared
  task-dispatch seam map (§Shared-Seam Map, §Pattern 2 create-path), the
  two-dispatchers-drift pitfall (Pitfall 1), and HTASK-03's live HTTP loopback
  harness (`tests/workflow_prompt_e2e_test.rs:54-97` pattern) that D-14
  extends.
- `docs/design/tasks-feature-design.md` — original tasks feature design.
- `CLAUDE.md` §ALWAYS Requirements + §Release & Publish Workflow — unit +
  property + fuzz + runnable example, `make quality-gate` before any commit.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `maybe_build_task_created` (shared task-dispatch unit, Phase 102): the
  create-path gate that keeps precedence over `ToolOutput::Result` (D-04).
- `TypedToolWithOutput` / `tool_typed()`: the registration-variant precedent
  `tool_with_result` (D-03.2) mirrors.
- `RELATED_TASK_META_KEY` + `TaskMetadata` (src/types/tasks.rs): already-typed
  SEP-1686 pieces; D-03.1/D-09 just add the builder/accessor around them.
- Phase 102 HTTP loopback harness + `s46_http_tool_as_task` example: the
  bases for D-14's sniffer test and D-13's s47 example.
- `crate::runtime::sleep` (src/runtime/mod.rs:38) + `web_time::Instant`
  (src/shared/middleware.rs:25): platform-abstracted delay + wasm-safe elapsed
  clock for D-10.
- Phase 103 web-channel client (`examples/web-channel-client/`): hand-rolled
  poll loop that becomes a `wait_for_task` consumer.

### Established Patterns
- Shared task-dispatch seam (Phase 102): ALL task-related dispatch logic lives
  in ONE unit called by both `Server` and `ServerCore` — the two
  `handle_call_tool` bodies drifted once before (102-RESEARCH Pitfall 1);
  TOUT-01/02 must not re-diverge them.
- Additive-only API evolution: default-impl trait methods, builder variants,
  `#[non_exhaustive]` types.
- `#[cfg(not(target_arch = "wasm32"))]` gating for server task machinery;
  client task calls ARE wasm-reachable (Phase 103 browser client).
- Error mapping precedent: `Error::ToolRejected` → `CallToolResult::rejected`
  (mod.rs:1438) — `ToolOutput` handling slots into the same match.

### Integration Points
- `src/server/mod.rs` `handle_call_tool` (~:1360-1501) and the `ServerCore`
  twin — both route through the shared seam.
- `src/server/traits.rs` — `ToolHandler` gains the default-impl
  `handle_output` method.
- `src/client/mod.rs` — `wait_for_task` next to `call_tool_with_task`.
- WASM server path (mod.rs:1421 direct `handler.handle`) — must also honor
  `handle_output` so browser-served tools aren't left behind.

</code_context>

<specifics>
## Specific Ideas

- The pmcp.run note's verdict frames the phase: "not wrong, but incomplete —
  and incomplete in a way that fails silently, which is the worst kind." The
  phase closes the SEP-1686 junction (task-augmented results through the front
  door), which Phase 101/102 deliberately did not cover.
- Wire-probe evidence to reproduce in the tripwire's regression test: the
  agent-lake shape — outer result with NO `_meta`, whole serialized
  `CallToolResult` as a JSON string inside `content[0].text`.
- Good-news fact for the guide: pmcp.run's durable-agent `detect_task_response`
  (reads `result._meta[related-task]`) works UNCHANGED against native
  `with_task_store()` servers because the native `CreateTaskResult` carries
  `_meta[related-task]` (core_tests.rs:881-892).
- Reply to the pmcp.run team after phase completion should map their asks 6/7/8/9
  → TOUT-01/02/03/04 and note ask-6-option-(b) was rejected with the serde
  rationale.

</specifics>

<deferred>
## Deferred Ideas

- `Stream`-based task-status polling API on the client (D-11) — additive
  later.
- pmcp-course chapter for the migration guide (D-12) — after the API
  stabilizes.
- Coordinated pmcp.run-side UAT as a formal gate (D-14) — invited, not
  required for phase closure.
- A result-aware `process_call_tool_result` response-middleware hook —
  explicitly REJECTED by D-04a (user chose "keep the bypass, harden it").
- Their June asks #1–#5 that are already shipped (server task lifecycle,
  capability coupling — Phase 101/102) or out of SDK scope (their durable
  client's WARN-on-deserialize-failure, ask #5, lives in THEIR client code —
  though pmcp's own client error paths should be checked during planning for
  the same swallow pattern).

</deferred>

---

*Phase: 104-task-augmented-tool-results-dx*
*Context gathered: 2026-07-04 · Amended 2026-07-04 (D-04a)*
