# Phase 104: Task-Augmented Tool Results DX (SEP-1686 junction) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-04
**Phase:** 104-task-augmented-tool-results-dx
**Areas discussed:** Typed-output API shape, Tripwire semantics, Client accessor scope, Migration guide shape

---

## Typed-output API shape

### Q1: Core mechanism for returning a full CallToolResult through dispatch

| Option | Description | Selected |
|--------|-------------|----------|
| ToolOutput enum | New default-impl trait method (`handle_output`) so ANY handler — incl. hand-written `ToolHandler` impls — can opt in; old `handle()` untouched | ✓ |
| TypedToolWithResult | Registration variant mirroring `TypedToolWithOutput`; zero trait changes but leaves hand-written handler impls out | |
| ctx meta-setter only | `extra.set_result_meta(...)` — covers the related-task case but not full-result control | |

**User's choice:** ToolOutput enum (recommended).

### Q2: Sugar layers alongside the enum (multiSelect)

| Option | Description | Selected |
|--------|-------------|----------|
| `with_related_task()` helper | `CallToolResult` builder method for the exact SEP-1686 `_meta` shape | ✓ |
| Typed closure registration | `ServerBuilder::tool_with_result(...)` mirroring `tool_typed()` | ✓ |
| `ctx.set_result_meta()` | `RequestHandlerExtra` setter merging `_meta` onto dispatch-built results | ✓ |

**User's choice:** All three.

### Q3: Precedence vs. native create-path gate and widget enrichment

| Option | Description | Selected |
|--------|-------------|----------|
| Native gate first | Phase 102 create-path gate keeps running first (D-STORE-MINTS-ID un-bypassable); then `Result(...)` verbatim — no wrap, no widget enrichment | ✓ |
| Result bypasses everything | Short-circuits before the gate; simplest model but makes native machinery skippable | |
| Result + enrichment | Skips text-wrap but widget enrichment still applies | |

**User's choice:** Native gate first (recommended).

---

## Tripwire semantics

### Q1: Loudness

| Option | Description | Selected |
|--------|-------------|----------|
| WARN + debug-fail | `tracing::warn!` all builds + `debug_assert!` in debug builds; release never panics | ✓ |
| WARN only | Never fail; zero risk but repeats the silent-failure pattern | |
| WARN + deny feature flag | Opt-in `strict-dispatch` feature hard-errors; opt-in protection tends to stay off | |

**User's choice:** WARN + debug-fail (recommended).

### Q2: Structural heuristic

| Option | Description | Selected |
|--------|-------------|----------|
| Markers: content OR meta | Object with a `content` array of valid `Content` items OR `_meta` containing `RELATED_TASK_META_KEY` | ✓ |
| related-task key only | Zero false positives but misses plain double-wraps | |
| Strict full deserialize | Broadest net; false-positive risk (fully-defaulted serde) | |

**User's choice:** Markers: content OR meta (recommended).

### Q3: False-positive escape hatch

| Option | Description | Selected |
|--------|-------------|----------|
| Per-tool opt-out | Registration-time flag (e.g. `.suppress_double_wrap_check()`); explicit and reviewable | ✓ |
| Env-var global kill switch | Process-wide, invisible in code review | |
| No escape hatch | Strictest; risks friction for data-shaped payloads | |

**User's choice:** Per-tool opt-out (recommended).

---

## Client accessor scope

### Q1: How much of the client-side task loop the SDK owns

| Option | Description | Selected |
|--------|-------------|----------|
| Accessor + poll helper | `related_task()` + `wait_for_task(task_id, opts)` driving tasks/get→tasks/result honoring pollInterval/maxPollDuration | ✓ |
| Accessor only | Minimal; integrators keep hand-rolling the loop | |
| Accessor + poll stream | `Stream<Item=Task>`; more composable but bigger surface | |

**User's choice:** Accessor + poll helper (recommended).

### Q2: WASM parity for the poll helper

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, wasm-compatible | Platform-abstracted delay; Phase 103 browser client becomes a direct consumer | ✓ |
| Native only for now | `#[cfg(not(wasm32))]`-gate it; wasm later | |

**User's choice:** Yes, wasm-compatible (recommended).

---

## Migration guide shape

### Q1: Where the guide lives

| Option | Description | Selected |
|--------|-------------|----------|
| Book chapter + design doc | pmcp-book chapter + docs/design companion (SEP-1686 rationale + D-08/D-09 wire-compat); README pointer; course deferred | ✓ |
| Full three shapes now | README + book + course in this phase | |
| docs/design only | Single consumer-team deliverable | |

**User's choice:** Book chapter + design doc (recommended).

### Q2: Runnable migration example

| Option | Description | Selected |
|--------|-------------|----------|
| Before/after example | New numbered slot with hand-rolled AND native versions of the same tool — the diff IS the guide; doubles as ALWAYS example + regression harness | ✓ |
| Native-only example | Just the new surface; migration stays prose-only | |
| No new example | Extend s46; muddles its teaching purpose | |

**User's choice:** Before/after example (recommended).

### Q3: Acceptance gate for wire-shape correctness

| Option | Description | Selected |
|--------|-------------|----------|
| HTTP round-trip + sniffer test | Extend Phase 102 live HTTP loopback: ToolOutput::Result tool over real StreamableHttpServer, assert raw wire JSON `_meta` at result top-level; consumes REAL dispatch output | ✓ |
| Also pmcp.run UAT | Plus coordinated external UAT before closure; couples to external team | |
| Unit + integration only | The fixture-shaped blind spot the incident documented | |

**User's choice:** HTTP round-trip + sniffer test (recommended). pmcp.run UAT invited but not a closure gate.

---

## Claude's Discretion

- Exact names (`ToolOutput`, `handle_output`, `tool_with_result`, `wait_for_task`, `suppress_double_wrap_check`) and module placement, provided the Phase 102 shared-seam rule holds.
- Internal pass-through signature in the shared dispatch unit.
- WARN message contents (tool name + which marker fired).
- Wasm timer mechanism for the poll helper.
- `set_result_meta` merge-vs-overwrite semantics on key collision (document the choice).
- Test file organization (mirror existing task-lifecycle / Phase 102 HTTP harness conventions).

## Deferred Ideas

- `Stream`-based task-status polling API (client) — additive later.
- pmcp-course chapter for the migration guide — after API stabilizes.
- Coordinated pmcp.run-side UAT as a formal gate — invited, not required.
- pmcp.run June asks #1–#5: already shipped in Phase 101/102 (task lifecycle, capability coupling) or live in their client code (WARN on deserialize failure) — though pmcp's own client error paths should be checked for the same swallow pattern during planning.
