---
phase: 104-task-augmented-tool-results-dx
plan: 05
subsystem: docs
tags: [tasks, sep-1686, migration-guide, example, http-acceptance-gate, tool-output, related-task]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-01 CallToolResult::with_related_task/related_task + TaskMetadata + wait_for_task"
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-02 ToolOutput enum + ToolHandler::handle_output + verbatim Result dispatch (D-04a bypass)"
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-03 double_wrap_tripwire + suppress_double_wrap_check"
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-04 ServerBuilder::tool_with_result + RequestHandlerExtra::set_result_meta"
  - phase: 102-http-task-dispatch
    provides: "HTTP task-dispatch seam + live HTTP loopback harness conventions"
provides:
  - "examples/s47_task_augmented_result.rs — runnable BEFORE/AFTER migration (BEFORE suppressed, AFTER store-minted) over a live HTTP loopback"
  - "tests/tool_output_result_http.rs — D-14 live-HTTP _meta-at-top-level acceptance gate consuming REAL dispatch output"
  - "docs/design/sep-1686-task-augmented-results.md — junction rationale + migration + wire-compat proof + D-04a bypass callout"
  - "pmcp-book chapter task-augmented-results.md (registered under Ch 12.7) + README pointer"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Runnable BEFORE/AFTER example doubles as the migration guide AND a regression harness (D-13)"
    - "Wire-shape acceptance gate reads raw JSON via the transport's TransportMessage::Response payload (untyped Value) — faithful, never a hand-authored fixture (D-14)"
    - "Manual ToolHandler with a handle_output override exercises ToolOutput::Result without the schema-generation feature gate (keeps the test cfg to streamable-http + non-wasm exactly)"

key-files:
  created:
    - examples/s47_task_augmented_result.rs
    - tests/tool_output_result_http.rs
    - docs/design/sep-1686-task-augmented-results.md
    - pmcp-book/src/task-augmented-results.md
  modified:
    - Cargo.toml
    - pmcp-book/src/SUMMARY.md
    - README.md

key-decisions:
  - "s47 serves BOTH tools over a single StreamableHttpServer loopback (not in-process) — BEFORE demonstrates the double-wrap bug (related_task() lost, envelope stringified into content[0].text), AFTER carries the store-minted related-task at result top level"
  - "The D-14 gate registers a manual ToolHandler (handle_output -> ToolOutput::Result) rather than tool_with_result, so the file's cfg stays exactly `all(feature = \"streamable-http\", not(wasm32))` without coupling to the schema-generation feature that gates tool_with_result"
  - "s47 AFTER mints the related-task id via TaskStore::create BEFORE building the server and asserts the wire related_task().task_id equals that store-minted id (D-13: real store-minted, not hand-written)"

patterns-established:
  - "Read raw wire result JSON off StreamableHttpTransport::receive() -> TransportMessage::Response -> ResponsePayload::Result(Value) to assert on the un-deserialized shape"

requirements-completed: [TOUT-04]

# Metrics
duration: ~40min
completed: 2026-07-04
---

# Phase 104 Plan 05: SEP-1686 Migration Guide + Acceptance Gate Summary

**The SEP-1686 junction is now documented end-to-end and CI-locked: a runnable BEFORE/AFTER example (`s47`) shows the hand-rolled `_meta` anti-pattern (suppressed so it runs) versus native `task_store()` + `tool_with_result` with a store-minted related task; a live-HTTP gate asserts `ToolOutput::Result` carries `_meta` at the result top level (not stringified into `content[0].text`) over a real transport; and the migration guide (design doc + pmcp-book chapter + README pointer) records the junction rationale, the wire-compat proof, and the D-04a response-middleware bypass semantics.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-04
- **Tasks:** 3
- **Files:** 4 created, 3 modified

## Accomplishments

- **Task 1 — s47 BEFORE/AFTER example.** One self-contained file, served over a real `StreamableHttpServer` loopback (ephemeral `127.0.0.1:0`, `JoinHandle::abort()` shutdown), demonstrating the migration:
  - BEFORE: a `TypedTool` that returns a `CallToolResult`-shaped `Value` on the Payload path, registered with `suppress_double_wrap_check("before_hand_rolled")` (with an inline comment on WHY — to show the anti-pattern without the Plan 03 `debug_assert` aborting; real tools migrate, not suppress). The demo hard-asserts `related_task()` is `None` and that the hand-rolled `_meta` got stringified into `content[0].text` — the double-wrap bug, live.
  - AFTER: the SAME tool via `tool_with_result::<StartArgs>` returning `CallToolResult::new(..).with_related_task(TaskMetadata::new(store_minted_id))`. The id is minted by `TaskStore::create` (native store machinery), NOT hand-written; the demo asserts the wire `related_task().task_id` equals that store-minted id and that `content[0].text` is the real text (not a stringified envelope).
  - Added the `[[example]]` block to `Cargo.toml` with `required-features = ["full"]`.
- **Task 2 — D-14 live-HTTP acceptance gate** (`tests/tool_output_result_http.rs`). Real `StreamableHttpServer` + `StreamableHttpTransport` round-trip; a tool returns `ToolOutput::Result(CallToolResult::with_related_task("t-http"))`. The test reads the RAW JSON-RPC result Value off the transport (never deserialized into `CallToolResult`, never a hand-authored fixture) and asserts `result._meta[related-task].taskId == "t-http"` at TOP LEVEL AND that `content[0].text` (if present) is not a stringified `_meta` envelope. Ephemeral port, readiness via bound listener, `abort()` shutdown.
- **Task 3 — migration guide.** `docs/design/sep-1686-task-augmented-results.md` records the junction rationale (the `mod.rs` text-wrap bug class, 5 incident variants incl. the agent-lake double-wrap), the three hand-rolled → native migration patterns (A: `Value`-shaped result → `tool_with_result`; B: hand-written handler → `handle_output`/`ToolOutput::Result`; C: existing handler → `set_result_meta`), the wire-compat confirmation (cites `core_tests.rs:855-896` native `_meta[related-task]` emission ⇒ pmcp.run `detect_task_response` works unchanged; Required-without-store build error; client WARNs on deserialize failure at `client/mod.rs:629-640`), and the D-04a response-middleware bypass callout. A pmcp-book chapter (`task-augmented-results.md`, registered under Chapter 12.7 in `SUMMARY.md`) carries the same story with the bypass note boxed; the README MCP Tasks bullet points to it.

## Task Commits

Each task was committed atomically:

1. **Task 1: s47 BEFORE/AFTER example + Cargo.toml block** — `77d2a419` (feat)
2. **Task 2: D-14 live-HTTP _meta-at-top-level acceptance gate** — `861c70a0` (test)
3. **Task 3: SEP-1686 migration guide (design doc + book chapter + README)** — `c6f3da2e` (docs)

## Files Created/Modified

- `examples/s47_task_augmented_result.rs` (created) — runnable BEFORE/AFTER migration over a live HTTP loopback; hard assertions.
- `tests/tool_output_result_http.rs` (created) — D-14 live-HTTP wire-shape gate on raw dispatch output.
- `docs/design/sep-1686-task-augmented-results.md` (created) — junction rationale + migration + wire-compat + D-04a bypass callout.
- `pmcp-book/src/task-augmented-results.md` (created) — user-facing chapter with the boxed bypass note.
- `Cargo.toml` (modified) — added the `s47_task_augmented_result` `[[example]]` block.
- `pmcp-book/src/SUMMARY.md` (modified) — registered the new chapter under Ch 12.7.
- `README.md` (modified) — MCP Tasks bullet now links the migration chapter.

## Decisions Made

- The D-14 gate uses a manual `ToolHandler` whose `handle_output` returns `ToolOutput::Result`, rather than `tool_with_result`. `tool_with_result` is `#[cfg(feature = "schema-generation")]`; using it would force the test's cfg to also require `schema-generation`. A manual handler keeps the file's cfg exactly `all(feature = "streamable-http", not(target_arch = "wasm32"))` (the plan's specified gate) while still exercising the same verbatim `ToolOutput::Result` wire path. (s47, run with `--features full`, uses `tool_with_result` directly for the closure-author ergonomics story.)
- s47 serves both tools over one real `StreamableHttpServer` loopback (rather than an in-process shim) so the BEFORE double-wrap and the AFTER top-level `_meta` are observed over a genuine transport — the same "live round-trip, not code reading" rule the phase inherits from 101/102. The example prints its steps but every claim is also a hard `Err`-returning assertion, so it doubles as a regression harness.
- s47 mints the AFTER related-task id via `TaskStore::create` before building the server and asserts equality on the wire, satisfying D-13's "store-minted, not hand-written" requirement without needing a full `tasks/*` create round-trip inside the example.

## Deviations from Plan

None — plan executed exactly as written. The one implementation choice worth noting (manual `ToolHandler` in the D-14 test instead of `tool_with_result`, to keep the plan-specified cfg gate) is a within-plan decision, not a scope or behavior deviation; both paths land the identical `ToolOutput::Result` verbatim wire shape the gate asserts.

## Threat Flags

None — no new network endpoints, auth paths, or trust boundaries. The only new HTTP surface is the acceptance-gate test's loopback server (ephemeral `127.0.0.1:0`, `abort()` shutdown — T-104-05-03 mitigated). T-104-05-01 (wire regression) and T-104-05-04 (authors unaware of the bypass) are mitigated by the D-14 gate and the guide/chapter/rustdoc bypass callout respectively.

## Verification Results

- `cargo run --example s47_task_augmented_result --features full` — exits 0; BEFORE `related_task() = None` + stringified `_meta` in content, AFTER `related_task().taskId` == the store-minted id, content not stringified (no debug-assert abort — suppression works).
- `cargo test --features full --test tool_output_result_http` — 1 passed (raw `result._meta[related-task]` at top level over real HTTP; content not stringified).
- `cargo fmt --all -- --check` — clean.
- `cargo clippy --features full --example s47_task_augmented_result -- -D warnings` and `--test tool_output_result_http -- -D warnings` — no warnings.
- Docs verify: `DOCS_OK` (design doc references `related-task`, contains the `bypass` callout; `SUMMARY.md` and `README.md` reference the SEP-1686 / task-augmented chapter).

## Next Phase Readiness

- TOUT-04 complete; all four Phase 104 deliverables (TOUT-01/02/03/04) now shipped. The migration path is durable (design doc + book chapter + README pointer), the `ToolOutput::Result` wire shape is CI-locked over a real transport, and pmcp.run has a runnable BEFORE/AFTER plus the wire-compat proof to delete their hand-rolled `_meta` intercepts.
- Follow-up (out of scope, tracked in CONTEXT): reply to the pmcp.run team mapping asks 6/7/8/9 → TOUT-01/02/03/04; optional pmcp.run-side UAT is invited, not a phase-closure gate.

## Self-Check: PASSED

- Created files exist: `examples/s47_task_augmented_result.rs`, `tests/tool_output_result_http.rs`, `docs/design/sep-1686-task-augmented-results.md`, `pmcp-book/src/task-augmented-results.md`.
- Task commits present: `77d2a419` (T1), `861c70a0` (T2), `c6f3da2e` (T3).
- Cargo.toml contains the `s47_task_augmented_result` example block; `SUMMARY.md` + `README.md` reference the new chapter.
- No unexpected file deletions in the plan's commit range.

---
*Phase: 104-task-augmented-tool-results-dx*
*Completed: 2026-07-04*
