---
phase: 73-typed-client-helpers-list-all-pagination-parity-client-01
plan: 01
subsystem: client
tags: [rust, client, ergonomics, serde, typed-helpers, pagination-config, parity-client-01]

# Dependency graph
requires:
  - phase: 69-rmcp-parity-research-gap-analysis
    provides: PARITY-CLIENT-01 scope (typed call helpers + list_all + ClientOptions)
provides:
  - pmcp::ClientOptions public config struct (non_exhaustive)
  - Client::with_client_options(transport, options) constructor
  - ClientOptions::with_max_iterations builder-setter
  - Client::call_tool_typed / _with_task / _and_poll (serialize+delegate)
  - Client::get_prompt_typed (with D-06 leaf coercion)
  - CaptureTransport property-test helper (tests/property_tests.rs)
affects: [73-02-list-all-helpers, 73-03-release-coordination]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Thin serialize-and-delegate typed wrapper over Value-based sibling"
    - "#[non_exhaustive] config struct with builder-setter for downstream ergonomics"
    - "rust,no_run doctests on every new public method (Phase 66 convention)"
    - "MockTransport-driven unit tests + proptest wire-capture for delegation equivalence"

key-files:
  created:
    - src/client/options.rs
    - .planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/deferred-items.md
  modified:
    - src/client/mod.rs (new field + 1 constructor + 4 typed helpers + 6 tests)
    - src/lib.rs (ClientOptions re-export)
    - tests/property_tests.rs (new phase73_typed_helpers module)

key-decisions:
  - "Chose Client::with_client_options (not with_options) to avoid collision with pre-existing with_options(transport, Implementation, ProtocolOptions)"
  - "Added ClientOptions::with_max_iterations builder-setter because #[non_exhaustive] forbids struct-literal construction from downstream crates (E0639), which blocked doctests"
  - "Typed task-aware wrappers match live sibling signatures verbatim: two-arg call_tool_typed_with_task (no TaskMetadata), three-arg call_tool_typed_and_poll (no Duration poll_interval)"
  - "Property test captures wire messages via a CaptureTransport pair; falls back to typed-vs-untyped sent-vec equivalence when internal wire-format traversal fails"

patterns-established:
  - "#[non_exhaustive] public config structs MUST ship with a chainable builder-setter to support external doctests"
  - "Typed helpers live in a clearly-marked // === Phase 73: Typed helpers (PARITY-CLIENT-01) === block for future greppability"

requirements-completed: [PARITY-CLIENT-01]

# Metrics
duration: ~55min
completed: 2026-04-22
---

# Phase 73 Plan 01: Typed client helpers + ClientOptions scaffold Summary

**ClientOptions config struct + Client::with_client_options constructor + four typed call/prompt helpers (call_tool_typed, _with_task, _and_poll, get_prompt_typed) shipped additively on Client<T>, with 6 doctests, 6 unit tests, and 1 proptest wire-equivalence check.**

## Performance

- **Duration:** ~55 min
- **Started:** 2026-04-22T06:00:00Z (approximate — wall-clock within wave 1 execution window)
- **Completed:** 2026-04-22T06:55:00Z
- **Tasks:** 3 (all committed)
- **Files modified:** 4 (plus 1 phase artefact: deferred-items.md)

## Accomplishments

- `pmcp::ClientOptions` public type (`#[non_exhaustive]`, `Debug + Clone + Default`, `max_iterations: usize = 100`). Rustdoc documents memory-amplification risk and `max_iterations = 0` degenerate case.
- New `Client::with_client_options(transport, options)` constructor threads a custom `ClientOptions` without colliding with the pre-existing `Client::with_options(transport, Implementation, ProtocolOptions)`. Rustdoc carries the D-09 `ClientBuilder`-parity note.
- Four typed helpers on `impl<T: Transport> Client<T>`:
  - `call_tool_typed(name, &args) -> Result<CallToolResult>`
  - `call_tool_typed_with_task(name, &args) -> Result<ToolCallResponse>` (two-arg, matches live sibling at `src/client/mod.rs:463` — NO `TaskMetadata`)
  - `call_tool_typed_and_poll(name, &args, max_polls: usize) -> Result<CallToolResult>` (three-arg, matches live sibling at `src/client/mod.rs:620` — NO `TaskMetadata`, NO `Duration`)
  - `get_prompt_typed(name, &args) -> Result<GetPromptResult>` with D-06 leaf coercion (Null skipped, String pass-through, Number/Bool via Display, Array/Object via `serde_json::to_string`)
- Every new public item carries a `rust,no_run` doctest; 344 doctests pass in total (was 338 pre-task).
- Six new in-file unit tests; one new `tests/property_tests.rs` proptest (`prop_call_tool_typed_sends_expected_value`).
- Zero regressions: all 49 `client::` unit tests pass; property test suite green; build clean with no new warnings.

## Task Commits

Each task was committed atomically with `--no-verify` (worktree-mode parallel-executor rule; orchestrator runs hooks after merge):

1. **Task 1: Create ClientOptions module + re-export** — `1a8798f3` (feat)
2. **Task 2: Thread ClientOptions through Client + with_client_options constructor** — `1d9aa4ec` (feat)
3. **Task 3: Four typed client helpers + tests + property test** — `984b569f` (feat)
4. **Task 3 fmt cleanup** — `e90cb335` (style)

## Files Created/Modified

### Created
- `src/client/options.rs` — ClientOptions struct, Default impl, `with_max_iterations` setter, 3 unit tests
- `.planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/deferred-items.md` — pre-existing clippy warnings in out-of-scope files (documented, not fixed)

### Modified
- `src/client/mod.rs` — added `options: ClientOptions` field, `with_client_options` constructor, 4 typed helpers, 3 constructor-wiring tests, 3 typed-helper unit tests, Clone-impl field addition. Also patched `with_info` and `with_options` to initialise `options` to default.
- `src/lib.rs` — extended `pub use client::{Client, ClientBuilder, ClientOptions, ToolCallResponse};` line 109.
- `tests/property_tests.rs` — added `phase73_typed_helpers` module with `CaptureTransport` helper + `prop_call_tool_typed_sends_expected_value` proptest.

## New Public Method Signatures (verbatim from shipped source)

```rust
// src/client/mod.rs
pub fn with_client_options(transport: T, options: ClientOptions) -> Self { /* ... */ }

pub async fn call_tool_typed<A: serde::Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
) -> Result<CallToolResult>

pub async fn call_tool_typed_with_task<A: serde::Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
) -> Result<ToolCallResponse>   // two-arg, NO TaskMetadata

pub async fn call_tool_typed_and_poll<A: serde::Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
    max_polls: usize,
) -> Result<CallToolResult>      // three-arg, NO TaskMetadata, NO Duration

pub async fn get_prompt_typed<A: serde::Serialize + ?Sized>(
    &self,
    name: impl Into<String>,
    args: &A,
) -> Result<GetPromptResult>
```

```rust
// src/client/options.rs
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub max_iterations: usize,
}
impl ClientOptions {
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self { /* ... */ }
}
impl Default for ClientOptions { /* max_iterations: 100 */ }
```

**Explicit confirmation:** `call_tool_typed_with_task` is a two-arg wrapper (no `TaskMetadata`) and `call_tool_typed_and_poll` is a three-arg wrapper (no `TaskMetadata`, no `Duration poll_interval`). These match the verified live sibling signatures at `src/client/mod.rs:463` and `src/client/mod.rs:620` respectively.

## New Tests

**Unit (src/client/options.rs):**
- `default_max_iterations_is_100` — PASS
- `field_update_idiom_compiles` — PASS (in-crate usage OK despite non_exhaustive)
- `clone_is_independent` — PASS

**Unit (src/client/mod.rs):**
- `test_client_new_uses_default_options` — PASS (Client::new → Client::with_info → default options = 100)
- `test_client_with_client_options_threads_value` — PASS (max_iterations: 7 threaded through)
- `test_client_with_options_preserves_default_client_options` — PASS (pre-existing `with_options` gains default options field)
- `test_call_tool_typed_serialize_error_maps_to_validation` — PASS (serde error surfaces as Error::Validation with method-name prefix)
- `test_get_prompt_typed_non_object_rejected` — PASS (Vec → Array → rejected)
- `test_get_prompt_typed_string_values_not_quoted` — PASS (D-06 coercion rules)

**Property (tests/property_tests.rs):**
- `prop_call_tool_typed_sends_expected_value` — PASS (wire-equivalence captured via CaptureTransport pair)

**Doctests:**
- 344 passed, 1 ignored, 78 filtered-out in the full-features run after Task 3. 6 new doctests (ClientOptions struct-level, with_max_iterations, with_client_options, call_tool_typed, call_tool_typed_with_task, call_tool_typed_and_poll, get_prompt_typed).

## Decisions Made

- **Constructor naming (D-09, plan §2a):** Chose `Client::with_client_options` rather than overloading `Client::with_options`. The pre-existing `with_options(transport, Implementation, ProtocolOptions)` cannot be renamed without breaking the public API — it is Mode 1 "additive-only". The rustdoc calls out that `ClientBuilder` does not yet gain a `.client_options()` setter (tracked for a future phase).
- **Fix for `#[non_exhaustive]` + doctest E0639 (deviation, see below):** Added `ClientOptions::with_max_iterations()` as a chainable setter so downstream doctests can configure the struct without hitting E0639 ("cannot create non-exhaustive struct using struct expression"). In-crate tests continue to use the field-update idiom (which compiles from inside the defining crate).
- **Property test shape (plan MEDIUM finding #3):** Implemented the recommended "CaptureTransport + wire-byte comparison" property. Used two `CaptureTransport` instances when direct wire traversal cannot surface `params.arguments` — the fallback compares the full serialized Request vec. Both paths assert the same delegation invariant.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `#[non_exhaustive]` doctests failed with E0639**

- **Found during:** Task 3 doctest run (`cargo test --doc -p pmcp --features full client::`)
- **Issue:** Plan's rustdoc examples on `ClientOptions` and `Client::with_client_options` used the field-update idiom `ClientOptions { max_iterations: 50, ..Default::default() }`. Doctests execute as **external** crates, where `#[non_exhaustive]` forbids struct-literal construction even with `..Default::default()` (compile error E0639).
- **Fix:**
  - Added a chainable setter `ClientOptions::with_max_iterations(self, usize) -> Self` (#[must_use]) in `src/client/options.rs`.
  - Updated the `ClientOptions` and `with_client_options` rustdoc examples to use `ClientOptions::default().with_max_iterations(50)` instead of the forbidden struct literal.
  - Left in-crate usage (inline unit tests, in-crate doctest that intentionally targets the in-crate path) on the field-update idiom since it compiles inside the defining crate.
- **Files modified:** `src/client/options.rs`, `src/client/mod.rs`
- **Verification:** `cargo test --doc -p pmcp --features full client::` → 344 pass, 0 fail (was 2 fail before fix).
- **Committed in:** `984b569f` (Task 3 commit — fix folded into same commit as the code it enables).

**2. [Rule 2 — Missing Critical] Builder setter for external ergonomics**

- **Found during:** Fix for deviation #1 above.
- **Issue:** Without a builder-style setter, downstream callers would be forced into the verbose mutable form (`let mut opts = ClientOptions::default(); opts.max_iterations = N;`). The plan's Task 1 rustdoc showed `..Default::default()` as the "nice" form; post-fix, `with_max_iterations` is the external-facing nice form.
- **Fix:** Added `ClientOptions::with_max_iterations` (already described in #1).
- **Files modified:** `src/client/options.rs`
- **Verification:** New `with_max_iterations` doctest passes.
- **Committed in:** `984b569f` (Task 3 commit).

**3. [Rule 3 — Blocking, Worktree Base Divergence] Worktree branch was missing the phase-73 planning artefacts**

- **Found during:** Session bootstrap (before Task 1).
- **Issue:** The parallel-executor worktree branch `worktree-agent-a0e69ceb` was cut from an older commit (`edc16b17`) that pre-dated the phase-73 planning docs on `main`. A direct `git merge main` hit conflicts in shared files (`STATE.md`, `Cargo.toml`, `ci.yml`, etc.); a `git rebase main` also stalled on the same conflicts.
- **Fix:** Used a narrow `git checkout main -- .planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/` to pull only the phase-73 planning directory into the worktree without disturbing shared infrastructure files. The phase-73 plan docs were then committed as part of Task 1's file set.
- **Files modified:** `.planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/**` (created, from main).
- **Verification:** `73-01-PLAN.md` readable; plan text intact.
- **Committed in:** `1a8798f3` (Task 1 commit, alongside source code changes).

### Pre-existing Out-of-Scope Issues (NOT fixed)

- **Pre-existing clippy warnings** in `src/error/recovery.rs`, `src/shared/middleware.rs`, `src/shared/sse_parser.rs`, `src/server/workflow/*`. Confirmed pre-existing by stashing Task-3 changes and re-running clippy — warnings still present at baseline. Triggered by newer clippy (1.95.0) lint tightening (`clippy::arithmetic_side_effects`, `clippy::collapsible_match`). Not caused by Phase 73. Documented in `.planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/deferred-items.md` for the orchestrator/verifier to address in a follow-up housekeeping phase.

### Partial-spec grep criterion resolved semantically, not literally

- **Plan Task 2 acceptance:** `grep -cE "options: ClientOptions::default\(\)" src/client/mod.rs` was expected to return `4`. Actual count: `2`. Semantic reason: `Client::new` delegates to `Client::with_info` (which has the default init), and `ClientBuilder::build` delegates to `Client::with_options` (which has the default init). The grep counts literal struct-literal occurrences, not behaviour. Every code path that yields a `Client<T>` still ends with `options` initialised to `ClientOptions::default()`. Behaviour is covered by the three Task 2 unit tests (all pass). Documented here to surface the discrepancy for the verifier.

---

**Total deviations:** 3 auto-fixed (1 Rule-1 bug, 1 Rule-2 missing-ergonomics, 1 Rule-3 blocking). Plus 1 out-of-scope issue set deferred and 1 grep-criterion reconciled semantically.
**Impact on plan:** No scope creep. Behaviour exactly matches plan's `success_criteria`:
- `pmcp::ClientOptions` is a public `#[non_exhaustive]` `Debug + Clone + Default` type with `max_iterations = 100` default ✅
- `Client::with_client_options` exists, doesn't collide with `Client::with_options`, carries D-09 builder-parity note ✅
- Every existing constructor + Clone impl initialise `options` (verified via tests, not grep count) ✅
- Four typed helpers match live sibling signatures (no `TaskMetadata`, no `Duration`) ✅
- `get_prompt_typed` rejects non-object with exact error string ✅
- Three new unit tests + one property test pass ✅
- No existing test regressed (49/49 client unit tests, 344/344 doctests) ✅

## Issues Encountered

- **Worktree baseline divergence** (resolved, see Deviation #3): the worktree was cut from an older commit and needed a surgical `git checkout main -- <path>` for the planning docs.
- **Pre-existing clippy warnings** (deferred, see out-of-scope section): surfaced during the Task 3 mid-plan clippy check. Not caused by Phase 73.

## User Setup Required

None — additive Rust SDK changes, no external service configuration.

## Next Phase Readiness

- **Plan 73-02 (list_all_* helpers)** can consume `self.options.max_iterations` directly; the `ClientOptions` field is wired through every `Client<T>` constructor.
- **Plan 73-03 (release coordination)** will need the typed helpers and `ClientOptions` re-export surfaced in CHANGELOG.md and the docs.rs landing page.

## Threat Flags

None. Phase 73-01 is purely client-side serialization scaffolding over existing Value-based APIs; no new network surface, auth path, or trust-boundary crossings. The Phase 73 `<threat_model>` block (T-73-P1-01, T-73-P1-02, T-73-P1-03) is fully addressed via the D-06 coercion pattern and `max_iterations = 0` documentation.

## Self-Check: PASSED

Verified presence of key artefacts:

- `src/client/options.rs` — FOUND
- `src/client/mod.rs` options field + 4 typed helpers + with_client_options — FOUND
- `src/lib.rs` ClientOptions re-export — FOUND
- `tests/property_tests.rs` prop_call_tool_typed_sends_expected_value — FOUND
- Commits `1a8798f3`, `1d9aa4ec`, `984b569f`, `e90cb335` — FOUND in `git log`

---
*Phase: 73-typed-client-helpers-list-all-pagination-parity-client-01*
*Plan: 01*
*Completed: 2026-04-22*
