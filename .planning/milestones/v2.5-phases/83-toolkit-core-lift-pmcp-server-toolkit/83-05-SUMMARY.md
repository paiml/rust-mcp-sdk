---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 05
subsystem: toolkit
tags:
  - toolkit
  - tools
  - synthesizer
  - tdd
  - property-test
  - jsonschema

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit
    provides: "ServerConfig parser + ToolDecl / ParamDecl / AnnotationsDecl (Plan 04); SecretValue + StaticPromptHandler + StaticResourceHandler (Plans 02-03) — re-exports the synthesizer joins"
  - phase: 82-builder-dx-prerequisites
    provides: "ServerBuilder::tool_arc + handler.metadata() consumption (the Phase 82 contract the synthesizer must satisfy)"
provides:
  - "pub fn synthesize_from_config(&ServerConfig) -> Result<Vec<SynthesizedTool>>"
  - "pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>)"
  - "SynthesizedToolHandler — crate-private handler whose metadata() always returns Some(ToolInfo) (RESEARCH §Risks #2 / threat T-83-05-01 mitigation)"
  - "JSON Schema synthesis from [[tools.parameters]] with type/description/minimum/maximum/maxLength/default/enum and `additionalProperties: false`"
  - "ToolAnnotations synthesis from [tools.annotations] via the fluent builder (PATTERNS §Pattern C — no struct literals on #[non_exhaustive] types)"
  - "Property test (tools_count_preserved + every_tool_has_object_schema + handler_metadata_always_some) — TEST-02"
  - "Reference-fixture synthesis tests (open-images/imdb/msr-vtt) — anchors Plan 08's tools_from_config smoke test"
  - "Crate-root re-export of synthesize_from_config — D-15 / review R3 headline DX promise"
affects:
  - 83-06 (code-mode handler dispatch will replace the placeholder Err in SynthesizedToolHandler::handle)
  - 83-08 (ServerBuilderExt::tools_from_config calls synthesize_from_config then loops tool_arc)
  - 84 (SQL connector wires per-backend execution into SynthesizedToolHandler::handle for tools with `sql`)
  - 85 (Shape A pmcp-sql-server binary reproduces the open-images config via this synthesizer end-to-end)
  - 87 (pmcp-config-helper authoring skills surface the [[tools]] schema users write)

# Tech tracking
tech-stack:
  added:
    - "serde_json::{json, Map, Value} — input-schema construction"
    - "async-trait — required by pmcp::server::ToolHandler"
  patterns:
    - "TDD plan-level discipline (RED test commit → GREEN feat commit → REFACTOR commit). PMAT complexity ≤25 enforced by inspection + advisory PMAT run."
    - "Type alias for return-type complexity: `pub type SynthesizedTool` keeps clippy::type_complexity quiet while preserving the (name, ToolInfo, Arc) tuple shape PATTERNS §9 mandates."
    - "Constructor-first synthesis (ToolInfo::with_annotations / ToolAnnotations::new()) over struct-literal — required by #[non_exhaustive] (PATTERNS §Pattern C)."
    - "Placeholder handler returns Err(pmcp::Error::Internal(...)) rather than a Value pretending success — Gemini-review note (MCP clients see tool-call error rather than silent ok)."
    - "Smoke-const re-export proof (crate-root `_ROOT_REEXPORT_SMOKE` extended from 10 to 11 entries) — D-15 / review R3 compile-time DX guarantee."

key-files:
  created:
    - "crates/pmcp-server-toolkit/tests/tool_synthesis_props.rs — TEST-02 property test (3 properties, 256 cases each by default)"
  modified:
    - "crates/pmcp-server-toolkit/src/tools.rs — full implementation (synthesize_from_config + build_input_schema + build_param_property + build_annotations + SynthesizedToolHandler + 6 unit tests + doctest)"
    - "crates/pmcp-server-toolkit/src/lib.rs — crate-root re-export `pub use crate::tools::synthesize_from_config;` + `_ROOT_REEXPORT_SMOKE` extended to assert fn-pointer resolution"
    - "crates/pmcp-server-toolkit/tests/reference_configs.rs — 3 new fixture-synthesis tests"

key-decisions:
  - "Placeholder ToolHandler::handle returns Err(pmcp::Error::Internal) rather than a Value with `is_error: true`. The pmcp::server::ToolHandler trait at src/server/mod.rs:224 returns Result<Value>, not Result<CallToolResult> — so the plan's `CallToolResult::new(...).with_is_error(true)` body wouldn't compile. Err semantically expresses the same not-yet-wired condition while honoring the actual trait shape (Gemini review note preserved)."
  - "`pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>)` introduced to satisfy clippy::type_complexity while preserving the exact shape PATTERNS §9 mandates. The alias is `pub` and re-export-friendly so Plan 08's ServerBuilderExt can name it directly."
  - "Helper count = 3 (build_input_schema, build_param_property, build_annotations). Splitting `build_param_property` out of `build_input_schema` keeps both at cog ≤10 (measured: 3 + 10) — well under D-03's 25 ceiling."
  - "cost_hint (AnnotationsDecl) intentionally not propagated to ToolAnnotations because the pmcp ToolAnnotations builder has no `with_cost_*` method. The field is parsed and retained on AnnotationsDecl for future Plan 06 / Phase 84 rate-limit policy wiring."

patterns-established:
  - "TDD-plan commit triad: test() → feat() → refactor() with each commit referencing its TDD stage in the subject. Plan-level TDD gate validated by `git log --oneline` showing the three commits land in RED→GREEN→REFACTOR order."
  - "Per-tool dispatch placeholder = `Err(pmcp::Error::Internal(...))` until execution wiring lands. Reusable for Plan 06 and Phase 84 stub paths."
  - "Reference-fixture integration tests use `assert_fixture_synthesizes(toml, label)` helper to keep per-fixture tests one-line bodies — adds new fixtures (Plan 09 fuzz) without copy-paste."

requirements-completed:
  - TKIT-07
  - TEST-02

# Metrics
duration: 22min
completed: 2026-05-18
---

# Phase 83 Plan 05: `[[tools]]` → `ToolInfo` synthesizer (TKIT-07) Summary

**`synthesize_from_config` builds `(name, ToolInfo, Arc<dyn ToolHandler>)` tuples from `[[tools]]` config entries, with JSON-Schema-object input validation, fluent `ToolAnnotations` synthesis, and a `metadata()`-always-`Some` invariant locked in by both unit + property tests.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-05-18T21:05Z (approximate — STATE.md last_updated)
- **Completed:** 2026-05-18T21:27Z
- **Tasks:** 3 (RED / GREEN / REFACTOR)
- **Files modified:** 3 (`src/tools.rs`, `src/lib.rs`, `tests/reference_configs.rs`)
- **Files created:** 1 (`tests/tool_synthesis_props.rs`)

## Accomplishments

- `pub fn synthesize_from_config(&ServerConfig) -> Result<Vec<SynthesizedTool>>` — the headline TKIT-07 API surface — landed with the exact signature PATTERNS §9 specifies (modulo the cosmetic `SynthesizedTool` alias to keep clippy::type_complexity quiet).
- `SynthesizedToolHandler::metadata()` is guaranteed-`Some(ToolInfo)` by both `synthesized_handler_metadata_returns_some` (unit) and `handler_metadata_always_some` (property, 256 cases). The Phase 82 `tool_arc` empty-schema-fallback risk (RESEARCH §Risks #2 / threat T-83-05-01) is closed.
- Input schema includes `"additionalProperties": false`, mitigating threat T-83-05-02 (arg-injection at request validation time).
- All three reference fixtures (`open-images-config.toml`, `imdb-config.toml`, `msr-vtt-config.toml`) synthesize end-to-end and produce 3 tools each, exactly matching their `[[tools]]` entry counts.
- Crate-root re-export of `synthesize_from_config` lands per D-15 / review R3 — Shape C consumers now write `use pmcp_server_toolkit::{ServerConfig, synthesize_from_config};` as the entry-point one-liner.

## Task Commits

Each TDD stage was committed atomically:

1. **Task 1 (RED):** `f10754f7` — `test(83-05): RED — failing synthesize_from_config tests`
   - 6 unit tests in `tools.rs` (empty/non-empty, required/optional partitioning, max_length, annotations round-trip, metadata invariant)
   - 3 property tests in `tests/tool_synthesis_props.rs`
   - `synthesize_from_config` body is `todo!()` — tests compile but fail at runtime.

2. **Task 2 (GREEN):** `0c8b186d` — `feat(83-05): implement synthesize_from_config + helpers (GREEN)`
   - Full `synthesize_from_config` + 3 decomposed helpers + `SynthesizedToolHandler`.
   - `Err(pmcp::Error::Internal(...))` placeholder for the not-yet-wired handler body (Gemini review note: prefer `Err` over a `Value` pretending success).
   - 6 unit tests + 3 property tests + 1 doctest all GREEN.

3. **Task 3 (REFACTOR):** `85a9eb19` — `refactor(83-05): reference-fixture synthesis tests + crate-root re-export`
   - PMAT advisory complexity check confirms zero cog ≥25 violations.
   - 3 reference-fixture synthesis tests added.
   - Crate-root re-export wired; smoke const extended from 10 → 11 entries.
   - `make quality-gate` passes.

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/tools.rs` — full module (synthesizer + 3 helpers + SynthesizedToolHandler + 6 unit tests + doctest).
- `crates/pmcp-server-toolkit/tests/tool_synthesis_props.rs` *(new)* — TEST-02 property test (3 properties, 256 cases each).
- `crates/pmcp-server-toolkit/src/lib.rs` — added `pub use crate::tools::synthesize_from_config;` and extended `_ROOT_REEXPORT_SMOKE` to assert the fn-pointer resolves at crate root.
- `crates/pmcp-server-toolkit/tests/reference_configs.rs` — added `open_images_synthesizes`, `imdb_synthesizes`, `msr_vtt_synthesizes` (now 6 tests total — 3 parse + 3 synthesize).

## Cognitive Complexity (PMAT 3.15.0)

| Function | Cyclomatic | Cognitive | Ceiling | Status |
|----------|-----------:|----------:|--------:|--------|
| `synthesize_from_config` | 3 | 3 | 25 | OK |
| `build_input_schema` | 3 | 3 | 25 | OK |
| `build_param_property` | 9 | 10 | 25 | OK |
| `build_annotations` | 2 | 1 | 25 | OK |

Zero `#[allow(clippy::cognitive_complexity)]` annotations anywhere in `tools.rs`.

## Reference-Fixture Tool Counts

| Fixture | `[[tools]]` count | Synthesizes |
|---------|------------------:|:-----------:|
| `open-images-config.toml` | 3 | yes (`explore_category`, `search_relationships`, `browse_relationships`) |
| `imdb-config.toml` | 3 | yes |
| `msr-vtt-config.toml` | 3 | yes |

Recorded for Plan 08's `ServerBuilderExt::tools_from_config` smoke test, which can use the same fixtures to assert end-to-end tool-arc registration count parity.

## Decisions Made

1. **Placeholder body returns `Err`, not a stub `Value`.** The pmcp `ToolHandler::handle` signature returns `Result<Value>` (not `Result<CallToolResult>` as the plan's example suggested) — see `src/server/mod.rs:224`. An `Err(pmcp::Error::Internal(...))` payload semantically expresses "not yet wired" and surfaces as a tool-call error to MCP clients, satisfying the Gemini review note without forcing the plan's `CallToolResult::new(...).with_is_error(true)` body that would not compile against the real trait shape.

2. **`SynthesizedTool` type alias.** Clippy's `type_complexity` lint flagged the `Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>>` return type. Introducing `pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>)` keeps clippy happy under `-D warnings` while preserving the exact tuple shape PATTERNS §9 mandates.

3. **`cost_hint` deliberately not surfaced to `ToolAnnotations`.** The pmcp `ToolAnnotations` fluent builder has no `with_cost_*` method (the canonical MCP `toolAnnotations` block has `read_only_hint`/`destructive_hint`/`idempotent_hint`/`open_world_hint` — `cost_hint` is a pmcp-run extension). The field is parsed and retained on toolkit-side `AnnotationsDecl` so Plan 06 / Phase 84 can route it into rate-limit policy without re-parsing the config.

## Deviations from Plan

None significant — three small adjustments documented as Rule-3 (blocking-issue) fixes:

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Trait shape mismatch in plan example**
- **Found during:** Task 2 (GREEN implementation)
- **Issue:** Plan's example body returns `CallToolResult::new(vec![Content::text(...)]).with_is_error(true)` but the actual `pmcp::server::ToolHandler::handle` trait at `src/server/mod.rs:224` returns `Result<serde_json::Value>`, not `Result<CallToolResult>`. The plan's example wouldn't compile.
- **Fix:** Return `Err(pmcp::Error::Internal(format!(...)))` instead. Semantically equivalent (not-yet-wired surfaces as a tool error), honors the real trait shape, and preserves the Gemini review note ("client sees error rather than silent ok").
- **Files modified:** `crates/pmcp-server-toolkit/src/tools.rs`
- **Verification:** Builds; `synthesized_handler_metadata_returns_some` test passes against the real return type.
- **Committed in:** `0c8b186d` (Task 2 GREEN commit).

**2. [Rule 3 - Blocking] `clippy::type_complexity` under `-D warnings`**
- **Found during:** Task 2 (clippy run)
- **Issue:** `Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>>` triggers `clippy::type_complexity`. Adding `#[allow(...)]` would muddy the public surface.
- **Fix:** Introduced `pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>)` and rewrote the return as `Result<Vec<SynthesizedTool>>`. Public alias enables downstream callers (Plan 08) to name the type cleanly.
- **Files modified:** `crates/pmcp-server-toolkit/src/tools.rs`
- **Verification:** `cargo clippy -p pmcp-server-toolkit --all-targets -- -D warnings` clean.
- **Committed in:** `0c8b186d` (Task 2 GREEN commit).

**3. [Rule 3 - Blocking] Plan example used outdated field names**
- **Found during:** Task 1 (RED test scaffold)
- **Issue:** Plan example used `ParamDecl { ty, ... min, max }` field names; the real `ParamDecl` from Plan 04 uses `param_type`, `minimum`, `maximum` (renamed `type` → `param_type` to avoid the Rust keyword). `AnnotationsDecl` fields are plain `bool` (not `Option<bool>`) with a separate `cost_hint: Option<String>`. `ToolDecl.description` is `Option<String>` (not `String`).
- **Fix:** Tests use the real field names directly. No code changes to Plan 04 types — the synthesizer code adapts: `param_type.as_deref().unwrap_or("string")` for the missing-type default; `ToolAnnotations::new().with_read_only(d.read_only_hint)` (passes plain bool); `decl.description.clone()` is `Option<String>` already.
- **Files modified:** `crates/pmcp-server-toolkit/src/tools.rs`, `crates/pmcp-server-toolkit/tests/tool_synthesis_props.rs`
- **Verification:** Tests compile and pass.
- **Committed in:** `f10754f7` (Task 1 RED) + `0c8b186d` (Task 2 GREEN).

---

**Total deviations:** 3 auto-fixed (3× Rule 3 - Blocking)
**Impact on plan:** All three are mechanical reconciliations against the actual code (real `ToolHandler` shape, clippy strictness, and Plan 04 field names). No scope creep, no API surface change versus what PATTERNS §9 specified.

## TDD Gate Compliance

- **RED gate:** `test(83-05): RED — failing synthesize_from_config tests` at `f10754f7` (tests compile, fail at runtime via `todo!()`) — verified by absence of `test result: ok` for the unit suite at this commit.
- **GREEN gate:** `feat(83-05): implement synthesize_from_config + helpers (GREEN)` at `0c8b186d` — 6 unit tests + 3 property tests + 1 doctest pass.
- **REFACTOR gate:** `refactor(83-05): reference-fixture synthesis tests + crate-root re-export` at `85a9eb19` — PMAT advisory check passes; `make quality-gate` passes.

All three commits land in order on `gsd/phase-83-toolkit-core-lift`.

## Issues Encountered

None requiring escalation. The three "issues" above were standard Rule-3 reconciliations against the real code surface (trait shape, clippy strictness, Plan 04 field naming).

## User Setup Required

None — pure SDK code; no external services to configure.

## Next Phase Readiness

**Plan 06 (TKIT-09: Code Mode Wiring) unblocked.** The placeholder `SynthesizedToolHandler::handle` body is the obvious extension point for Plan 06 — replace the `Err(Internal)` arm with a `code_mode.execute(&self.decl.sql, args)` dispatch when `[code_mode]` is enabled.

**Plan 08 (TKIT-08: `ServerBuilderExt::tools_from_config`) unblocked.** Plan 08 can now write:

```rust
let synthesized = pmcp_server_toolkit::synthesize_from_config(&cfg)?;
for (name, _info, handler) in synthesized {
    builder = builder.tool_arc(name, handler);
}
```

**Phase 84 (SQL Connectors) unblocked at the API surface.** SQL connectors will swap the `Err(Internal)` placeholder for per-backend execution against `self.decl.sql`.

**Confidence:** High. The metadata-invariant property tests + reference-fixture integration tests provide strong regression protection as downstream plans wire execution paths.

## Self-Check: PASSED

Verified:
- `crates/pmcp-server-toolkit/src/tools.rs` exists, 365 lines after fmt.
- `crates/pmcp-server-toolkit/tests/tool_synthesis_props.rs` exists.
- `crates/pmcp-server-toolkit/src/lib.rs` contains `pub use crate::tools::synthesize_from_config;`.
- `crates/pmcp-server-toolkit/tests/reference_configs.rs` contains `open_images_synthesizes` + `imdb_synthesizes` + `msr_vtt_synthesizes`.
- Commits present in `git log --oneline`: `f10754f7`, `0c8b186d`, `85a9eb19`.
- `cargo test -p pmcp-server-toolkit`: 67 passed.
- `cargo test --doc -p pmcp-server-toolkit`: 11 passed.
- `make quality-gate`: passed (ALL TOYOTA WAY QUALITY CHECKS PASSED banner).

---
*Phase: 83-toolkit-core-lift-pmcp-server-toolkit*
*Plan: 05 — `[[tools]]` → `ToolInfo` synthesizer (TKIT-07)*
*Completed: 2026-05-18*
