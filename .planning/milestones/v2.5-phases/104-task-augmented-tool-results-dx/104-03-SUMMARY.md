---
phase: 104-task-augmented-tool-results-dx
plan: 03
subsystem: api
tags: [tool-dispatch, tool-output, double-wrap, tripwire, tasks, pmcp-server]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results-dx
    plan: 02
    provides: ToolOutput enum + resolve_tool_output/DispatchOutput shared Payload-vs-Result branch
provides:
  - "task_dispatch::looks_like_call_tool_result — high-precision structural detector (RelatedTaskMeta / ContentArray markers, no full CallToolResult parse)"
  - "task_dispatch::double_wrap_tripwire — the SINGLE decision fn (WARN every build + debug_assert! in debug/CI, D-06 release never panics) called at both Payload wrap sites"
  - "ServerBuilder::suppress_double_wrap_check + ServerCoreBuilder::suppress_double_wrap_check (D-08 per-tool opt-out); the set threads into both Server and ServerCore so both dispatchers consult the SAME rule (no drift)"
  - "pmcp::__test_support seam re-exporting the crate-private marker/decision fns for the integration test"
affects: [104-04-tool_with_result-ergonomics, 104-05-migration-guide]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "High-precision structural marker check in cost order (cheap _meta key first, then per-element Content parse) instead of a full from_value parse (D-02)"
    - "Loud-but-safe tripwire: tracing::warn! in all builds + debug_assert! (never assert!) so debug/CI hard-fails and release only warns (D-06)"
    - "Helper-level decision fn returning Option<marker> so the debug-panic behavior is catch_unwind-testable in isolation, decoupled from end-to-end dispatch (Codex MEDIUM)"
    - "Per-tool opt-out set registered on the builder and threaded into the running server via a with_* setter (mirrors with_server_request_dispatcher)"

key-files:
  created:
    - tests/double_wrap_tripwire.rs
  modified:
    - src/server/task_dispatch.rs
    - src/server/mod.rs
    - src/server/core.rs
    - src/server/builder.rs
    - src/lib.rs

key-decisions:
  - "D-02: the detector checks two cheap structural markers, never a full from_value::<CallToolResult>; Content is internally #[serde(tag=type)] so a stray object never mis-parses as Content (near-zero false positives)"
  - "D-06: debug_assert! (not assert!) so release compiles the hard-fail out and never panics; a release test run proves it"
  - "D-08: suppress_double_wrap_check is a rare/reviewed per-tool opt-out; rustdoc steers authors to ToolOutput::Result instead"
  - "The crate-private looks_like_call_tool_result / double_wrap_tripwire are exposed to the integration test only via the existing hidden pmcp::__test_support seam (same precedent as ServerRequestDispatcher)"

patterns-established:
  - "One structural detector + one decision fn, called identically at both Payload wrap sites, so the WARN/panic/suppression rule can never drift between Server and ServerCore"

requirements-completed: [TOUT-02]

# Metrics
duration: ~50min
completed: 2026-07-04
---

# Phase 104 Plan 03: Double-wrap tripwire (TOUT-02) Summary

**When dispatch is about to text-wrap a `ToolOutput::Payload` `Value` that structurally resembles an already-built `CallToolResult`, it now WARNs in every build and `debug_assert!`-hard-fails in debug/CI — making the silent double-wrap bug (the agent-lake 2-week outage) loud at authoring/CI time with near-zero false positives, plus a rare/reviewed per-tool `suppress_double_wrap_check` opt-out that threads identically into both native dispatchers.**

## Performance

- **Duration:** ~50 min
- **Completed:** 2026-07-04
- **Tasks:** 2
- **Files modified:** 5 (+1 created)

## Accomplishments
- Added `looks_like_call_tool_result` — a high-precision structural detector returning `Option<DoubleWrapMarker>`. It checks the cheap `_meta[related-task]` key first (short-circuits pathological large payloads, T-104-03-02), then a NON-EMPTY `content` array whose every element deserializes as the internally-tagged `Content` enum. Empty `content: []` and any non-`Content` element never fire. No `from_value::<CallToolResult>` (D-02).
- Added `double_wrap_tripwire(name, value, suppressed)` — the SINGLE decision fn: `None` when suppressed or benign; otherwise `tracing::warn!` (every build) + `debug_assert!(false, ..)` (debug/CI hard-fail, D-06 release compiled out) and returns `Some(marker)`. Returning the marker makes it unit-testable in isolation (release asserts the return value, debug asserts the panic via `catch_unwind`) with no dispatch spun up.
- Wired the tripwire at BOTH Payload wrap sites — `Server::handle_call_tool` (mod.rs, before `result.to_string()`) and `ServerCore::handle_call_tool` (core.rs, before the text-wrap tail) — each guarded by the per-tool suppression check, non-wasm-gated (the `task_dispatch` unit is non-wasm).
- Added `suppress_double_wrap_check(name)` to BOTH `ServerBuilder` and `ServerCoreBuilder` (D-08). The `HashSet<String>` threads into `Server` (via `build()`) and into `ServerCore` (via a new `ServerCore::with_suppress_double_wrap` setter called from `ServerCoreBuilder::build`), so both dispatchers consult the SAME suppression rule — asserted by an end-to-end parity test across `Server` and `ServerCore`.
- Rustdoc on both `suppress_double_wrap_check` methods states suppression should be RARE and REVIEWED, and steers authors to return `ToolOutput::Result` instead.

## Task Commits

Each task was committed atomically:

1. **Task 1: looks_like_call_tool_result marker fn + precision property test** - `0ba7cd88` (feat)
2. **Task 2: double_wrap tripwire decision fn + wiring + per-tool suppress opt-out** - `2d8c039f` (feat)

_Note: Rust's pre-commit build/format quality gate forbids committing a compile-failing RED test, so each TDD task shipped its implementation and tests together in one commit. RED/GREEN was still exercised locally (tests written and observed to drive the code)._

## Files Created/Modified
- `src/server/task_dispatch.rs` - Added `DoubleWrapMarker` enum, `looks_like_call_tool_result` (high-precision structural detector), and `double_wrap_tripwire` (the WARN + `debug_assert!` decision fn). `Content` added to imports.
- `src/server/mod.rs` - `ServerBuilder`/`Server` gained a `suppress_double_wrap: HashSet<String>` field + `ServerBuilder::suppress_double_wrap_check`; the tripwire is called at the Payload wrap site before `to_string()`, guarded by the suppression check.
- `src/server/core.rs` - `ServerCore` gained a `suppress_double_wrap` field + `with_suppress_double_wrap` setter; the tripwire is called at the Payload wrap tail before the text-wrap, guarded by the same suppression check.
- `src/server/builder.rs` - `ServerCoreBuilder` gained the field + `suppress_double_wrap_check`; `build()` now threads the set into `ServerCore` via `with_suppress_double_wrap`.
- `src/lib.rs` - Extended the hidden `__test_support` seam to re-export `looks_like_call_tool_result`, `double_wrap_tripwire`, and `DoubleWrapMarker`.
- `tests/double_wrap_tripwire.rs` - New gate: six `looks_like` behavior cases + a `proptest` precision fuzz; helper-level `double_wrap_tripwire` tests (suppressed / benign / release-return-value / debug-panic via `catch_unwind`); and end-to-end suppression parity across `Server` and `ServerCore`.

## Decisions Made
- The crate-private detector/decision fns are exposed to the integration test binary ONLY through the pre-existing hidden `pmcp::__test_support` module (the same sanctioned precedent used for the otherwise-`pub(crate)` `ServerRequestDispatcher`). This satisfies the plan's `--test double_wrap_tripwire looks_like` acceptance while keeping the helpers off the stable public API. Because `__test_support` re-exports with `pub use`, the two fns are declared `pub` (not `pub(crate)`) inside the `pub(crate) mod task_dispatch` — effective reachability is still crate-private + hidden, mirroring how `ServerRequestDispatcher` (a `pub` item in the same `pub(crate)` module) is re-exported. This is the only refinement to the plan's literal "`pub(crate)` free fn" wording; intent (internal-only, tested via the hidden seam) is preserved.
- End-to-end tests deliberately exercise the SUPPRESSED path (no panic → payload text-wrapped) via both dispatchers rather than the tripping/unsuppressed path, because in a debug build the tripping path `debug_assert!`-panics mid-dispatch (the intended behavior) which is brittle to assert end-to-end (Codex MEDIUM). The tripping WARN/panic behavior is covered deterministically at the helper level instead.

## Deviations from Plan

### Auto-fixed / adjusted (Rule 3 - make the plan's own acceptance compile)

**1. [Rule 3 - Visibility] `looks_like_call_tool_result` / `double_wrap_tripwire` declared `pub` (inside the `pub(crate) mod`) and re-exported via `pmcp::__test_support`**
- **Found during:** Task 1 (writing the integration test the plan's acceptance requires).
- **Issue:** The plan specifies a `pub(crate)` free fn AND a `tests/double_wrap_tripwire.rs` integration binary that calls it. An integration test is a separate crate and cannot reach `pub(crate)` items inside a `pub(crate) mod`.
- **Fix:** Followed the repo's established test-seam precedent — declared the two fns `pub` inside the crate-private `task_dispatch` module and re-exported them through the existing hidden `#[doc(hidden)] pub mod __test_support`. Effective reachability is unchanged (crate-private + hidden from docs). No new public API, no new dependency.
- **Files modified:** `src/server/task_dispatch.rs`, `src/lib.rs`.
- **Commits:** `0ba7cd88` (Task 1), `2d8c039f` (Task 2).

## Issues Encountered
- None. Both dispatchers already text-wrap at a clean, single site (from Plan 02), so the tripwire slotted in before each `to_string()` / `to_string_pretty()` without restructuring.

## Verification
- `cargo build --features full` — clean.
- `cargo test --features full --test double_wrap_tripwire` — 12/12 pass (debug: six `looks_like` cases + proptest + suppressed/benign decision + debug-panic via `catch_unwind` + Server/ServerCore suppression parity).
- `cargo test --release --features full --test double_wrap_tripwire` — 13/13 pass (release: the two release-only return-value tests run, the debug-panic test is compiled out; proves release NEVER panics — T-104-03-01).
- `cargo test --features full --test tool_output_passthrough` — 8/8 pass (Plan 02 not regressed).
- `cargo test --features full --lib task_dispatch` — 16/16 pass.
- `cargo clippy --features full --lib --test double_wrap_tripwire` — no warnings.
- `pmat analyze complexity --max-cognitive 25` — no violations in `task_dispatch.rs` / `core.rs` / `builder.rs` (the detector and decision fn are flat and short).

## Threat Model Coverage
- **T-104-03-01 (DoS: debug_assert reaching production):** mitigated — `debug_assert!` (not `assert!`); a release-mode test run confirms no panic.
- **T-104-03-02 (probe cost on large payloads):** mitigated — the `_meta` key lookup runs first (single map access); the content scan is O(len) per-element parse until the first non-`Content` element, no full-document deserialize.
- **T-104-03-03 (suppress-flag drift Server vs ServerCore):** mitigated — a single suppression `HashSet` is threaded into both dispatchers at construction; both call the SAME `double_wrap_tripwire`; the suppression-parity test asserts identical behavior via `Server` and `ServerCore`.
- **T-104-03-SC (package installs):** N/A — in-tree only, no new dependency (`proptest` already a dev-dependency; no tracing-capture crate added).

## Next Phase Readiness
- The tripwire is the safety net for Plan 04 (`tool_with_result` ergonomics): authors who forget to opt into a verbatim result now get a loud WARN/panic instead of a silent double-wrap.
- Plan 05's migration guide should reference `suppress_double_wrap_check` (rare/reviewed) and point authors at `ToolOutput::Result` as the preferred fix.

## Self-Check: PASSED

- Created file exists: `tests/double_wrap_tripwire.rs`.
- Task commits present: `0ba7cd88` (T1), `2d8c039f` (T2).
- No unexpected file deletions in the plan's commit range; working tree clean.

---
*Phase: 104-task-augmented-tool-results-dx*
*Completed: 2026-07-04*
