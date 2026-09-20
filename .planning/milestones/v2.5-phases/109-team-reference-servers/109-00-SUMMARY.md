---
phase: 109-team-reference-servers
plan: 00
subsystem: api
tags: [pmcp-core, request-meta, serde-flatten, tools-call, client-api, mcp-tasks]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results-dx
    provides: "RequestHandlerExtra result_meta slot + ToolOutput dispatch; RELATED_TASK_META_KEY constant"
provides:
  - "RequestMeta extended with a #[serde(flatten)] namespaced `other` map + with_meta/get_meta builders (additive)"
  - "RequestHandlerExtra (native + wasm mirror) carries request_meta + with_request_meta builder"
  - "ServerCore AND high-level Server tool-call paths propagate req._meta into extra.request_meta"
  - "Client::call_tool_with_meta (non-task) and Client::call_tool_with_task_and_meta (task+_meta) forwarding APIs"
affects: [109-05-team-mcp-guard-meta-hop, 109-07-conformance-runner]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "serde(flatten) over a default-empty serde_json::Map for an additive, byte-identical-when-empty extensible field"
    - "Raw-JSON _meta on RequestHandlerExtra so handlers read arbitrary namespaced keys without a typed RequestMeta dependency"

key-files:
  created:
    - "tests/request_meta_roundtrip.rs"
  modified:
    - "src/types/protocol/mod.rs"
    - "src/server/cancellation.rs"
    - "src/shared/cancellation.rs"
    - "src/server/core.rs"
    - "src/server/mod.rs"
    - "src/client/mod.rs"

key-decisions:
  - "Route (A) from locked D-14: guard state travels as namespaced _meta, NOT smuggled inside tool arguments"
  - "request_meta carried as raw serde_json::Value (not typed RequestMeta) so handlers parse defensively at their own trust level (T-109-00-01)"
  - "Also wired the high-level Server dispatch path (server/mod.rs), not only ServerCore/core.rs, so real handlers (109-05 team-mcp) observe _meta — mirrors the existing task_request wiring pattern"

patterns-established:
  - "Additive extensible-map field: #[serde(flatten)] pub other: serde_json::Map<String, Value> emits nothing when empty"
  - "Twin propagation: any per-request field must be populated in BOTH core.rs and server/mod.rs dispatch sites (+ wasm mirror parity)"

requirements-completed: [TEAM-05, TEAM-06]

# Metrics
duration: 25min
completed: 2026-07-18
---

# Phase 109 Plan 00: pmcp-core `_meta` Enablement Summary

**Additive, non-breaking pmcp-core change: `RequestMeta` gains a `#[serde(flatten)]` namespaced `other` map, the request `_meta` is propagated into `RequestHandlerExtra.request_meta` on both dispatch paths (native + wasm mirror), and two `_meta`-forwarding client APIs (`call_tool_with_meta`, `call_tool_with_task_and_meta`) make locked decision D-14's route real.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-18T22:08:04Z
- **Tasks:** 2
- **Files modified:** 6 (+1 test file created)

## Accomplishments
- `RequestMeta` now preserves arbitrary namespaced `_meta` keys on serialize→deserialize round-trip via a `#[serde(flatten)]` `other: serde_json::Map` catch-all — with unit-tested proof that empty-`other` serialization is byte-identical to the pre-change form (no key emitted) and that `progressToken`/`_task_id` never leak into `other`.
- Tool handlers can now read the request's namespaced `_meta` via `RequestHandlerExtra.request_meta` (raw JSON), added additively to both the native struct (`src/server/cancellation.rs`) and the wasm mirror (`src/shared/cancellation.rs`) with `with_request_meta` builders.
- Both dispatch paths populate it: `ServerCore::handle_call_tool` (core.rs) AND the high-level `Server` tool-call dispatch (server/mod.rs).
- Two additive client methods forward custom `_meta`: `call_tool_with_meta` (non-task → `CallToolResult`) and `call_tool_with_task_and_meta` (task + `_meta` → `ToolCallResponse`), each with rustdoc examples.
- End-to-end integration test proves a custom `_meta` key round-trips through a real `Server` handler observing `extra.request_meta`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend RequestMeta + propagate request _meta into RequestHandlerExtra** - `73268a87` (feat)
2. **Task 2: Client _meta-forwarding APIs + high-level Server _meta propagation** - `80f5248e` (feat)

_Note: This repo's pre-commit quality gate blocks any non-compiling commit, so RED and GREEN TDD phases were combined into a single passing `feat` commit per task (see TDD Gate Compliance below)._

## Files Created/Modified
- `src/types/protocol/mod.rs` - Added `other` flatten map + `with_meta`/`get_meta` builders + 3 unit tests
- `src/server/cancellation.rs` - Added `request_meta: Option<Value>` field, `with_request_meta` builder, Debug field, constructor init
- `src/shared/cancellation.rs` - Wasm mirror parity: same field + builder + constructor init
- `src/server/core.rs` - ServerCore normal tool-call path populates `request_meta` from `req._meta`
- `src/server/mod.rs` - High-level Server dispatch mirrors the propagation
- `src/client/mod.rs` - `call_tool_with_meta` + `call_tool_with_task_and_meta`
- `tests/request_meta_roundtrip.rs` - End-to-end custom-`_meta` round-trip test (2 cases)

## Decisions Made
- Chose locked D-14 route (A): namespaced `_meta`, not tool-argument smuggling. Carrying guard state in `arguments` would violate the locked decision and pollute every member tool's input schema.
- `request_meta` is raw `serde_json::Value` (not typed `RequestMeta`) so core only carries the bytes and does not trust them — strict typed parsing is the downstream guard's job (109-05), per threat T-109-00-01.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Also wired `request_meta` into the high-level `Server` dispatch path**
- **Found during:** Task 2 (integration test)
- **Issue:** The plan scoped propagation to `src/server/core.rs` (ServerCore) only. The end-to-end test used the high-level `pmcp::Server` (the primary path 109-05 team-mcp handlers run under), which builds its OWN `RequestHandlerExtra` in `src/server/mod.rs` — so `extra.request_meta` was `None` there and the round-trip failed (observed `null`).
- **Fix:** Added the identical `.with_request_meta(req._meta → Value)` population in the `server/mod.rs` tool-call dispatch, exactly mirroring the pre-existing `with_task_request` twin-wiring pattern the code already documents.
- **Files modified:** src/server/mod.rs
- **Verification:** `tests/request_meta_roundtrip.rs` both cases pass; clippy clean.
- **Committed in:** `80f5248e` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Necessary for the feature to be usable by downstream 109-05/109-07 (which use the high-level Server). Additive and within the plan's intent ("ServerCore already wires this at core.rs" — the code comment itself signals the twin pattern). No scope creep.

## TDD Gate Compliance

Both tasks are `tdd="true"`. This repo's mandatory pre-commit quality gate (`make quality-gate`) blocks any commit whose build/tests do not pass, so a standalone RED commit referencing not-yet-existing API (compile failure) cannot be committed here. RED and GREEN were therefore authored together and landed as a single passing `feat` commit per task. Tests were still written to fail-first mentally against the target API and verified to exercise the new behavior:
- Task 1: 3 in-module unit tests (empty-`other` byte-identical serialization, custom-key round-trip via `other`, typed fields do not leak).
- Task 2: 2 integration tests over a real duplex `Server` (custom key observed server-side; empty meta behaves like a plain call).

## Issues Encountered
- Initial `cargo test -p pmcp` surfaced a PRE-EXISTING compile failure in `tests/tool_as_task_lifecycle.rs` (uses `pmcp::testing`, gated behind the `testing` feature). Unrelated to this plan — logged as out-of-scope, not fixed. Ran targeted `--lib` / `--test request_meta_roundtrip` instead.
- Clippy `unnecessary_join` on the test helper — fixed by collecting `&str` into `String` directly.

## Verification Performed
- `cargo test -p pmcp --lib request_meta` → 8 passed
- `cargo test -p pmcp --test request_meta_roundtrip` → 2 passed
- Project clippy (`--features full --lib --tests`, pedantic+nursery, exact `make lint` flags) → No issues found
- `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` → succeeds (wasm mirror parity)

## Threat Flags

None — the change introduces no new network endpoint, auth path, or schema at a trust boundary beyond the `_meta` carrier already registered in the plan's threat model (T-109-00-01/02/SC, all `mitigate` dispositions satisfied: raw-JSON carrier + defensive downstream parsing; flatten-empty byte-identical output; no new dependencies).

## Next Phase Readiness
- 109-05 (team-mcp) can send guard `_meta` + task augmentation in a single `call_tool_with_task_and_meta` and read it via `extra.request_meta`; it MUST use `RELATED_TASK_META_KEY` for related-task hops.
- 109-07 (conformance runner) can send per-fixture `_meta` via `call_tool_with_meta`.
- No blockers.

## Self-Check: PASSED

All 8 modified/created files present on disk; all 3 commits (`73268a87`, `80f5248e`, `274cb91a`) present in git history.

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
