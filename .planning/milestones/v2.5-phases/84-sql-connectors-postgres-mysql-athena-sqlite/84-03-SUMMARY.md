---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 03
subsystem: api
tags: [sql, toolkit, config, code-mode, structured-content, widget-meta, synthesizer]

# Dependency graph
requires:
  - phase: 84-00
    provides: "3 backend crate scaffolds + translate stub + database-url fuzz seed"
  - phase: 84-01
    provides: "SqlConnector.execute + ConnectorError execute-time variants"
  - phase: 83
    provides: "synthesize_from_config, SynthesizedToolHandler, ServerConfig, assemble_code_mode_prompt"
provides:
  - "build_code_mode_prompt alias (CONN-04 literal naming) next to assemble_code_mode_prompt"
  - "DatabaseSection.url: Option<String> additive config field (D-08 URL constructor input)"
  - "synthesize_from_config_with_connector — connector-threaded synthesizer variant (D-06)"
  - "widget_meta flip on synthesized ToolInfo when ui_resource_uri present (D-06 structuredContent gate)"
  - "SynthesizedToolHandler.handle executes SQL via connector.execute when wired"
  - "ServerBuilderExt::{tools_from_config_with_connector, try_tools_from_config_with_connector} Shape A wiring"
affects: [84-04, 84-05, 84-06, 84-07, 85, 86]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Additive variant-not-overload: synthesize_inner(cfg, Option<connector>) shared by both public entry points so the 11 P83 callers compile unchanged"
    - "Feature-independent widget metadata via ToolInfo::with_meta_entry (not mcp-apps-gated with_widget_meta)"

key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/code_mode.rs — build_code_mode_prompt alias"
    - "crates/pmcp-server-toolkit/src/config.rs — DatabaseSection.url + parse test"
    - "crates/pmcp-server-toolkit/src/tools.rs — synthesize_inner / _with_connector / apply_widget_meta / extract_named_params / handler execute body + 2 widget_meta tests"
    - "crates/pmcp-server-toolkit/src/lib.rs — additive re-export of synthesize_from_config_with_connector"
    - "crates/pmcp-server-toolkit/src/builder_ext.rs — tools_from_config_with_connector Shape A wiring"

key-decisions:
  - "REVIEWS M1 resolution: used ToolInfo::with_meta_entry(\"ui\", {resourceUri}) NOT with_widget_meta — the latter is gated on pmcp's mcp-apps feature which the toolkit does not enable; with_meta_entry is feature-independent, chainable (preserves annotations), and produces the ui.resourceUri shape widget_meta() recognises"
  - "Connector threaded as Option<Arc<dyn SqlConnector>> on a single handler struct (not two structs); no-connector path returns explicit Err on invocation (T-84-03-05)"
  - "builder_ext gained NEW with_connector methods rather than mutating the external pmcp::ServerBuilder; existing tools_from_config kept for connector-less callers"

patterns-established:
  - "Additive synthesizer variant via shared synthesize_inner helper — existing public API and callers untouched (REF-01 superset / superset invariant)"
  - "Feature-independent widget metadata emission via with_meta_entry for D-06 structuredContent"

requirements-completed: [CONN-04]

# Metrics
duration: 6min
completed: 2026-05-26
---

# Phase 84 Plan 03: Toolkit-Core Surgical Edits (CONN-04 + D-06 + D-08 URL) Summary

**`build_code_mode_prompt` alias, `DatabaseSection.url` config field, and an additive `synthesize_from_config_with_connector` variant that flips `widget_meta` (via the feature-independent `with_meta_entry` API) so synthesized tools emit `structuredContent` while all 11 P83 callers compile unchanged.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-26T21:13:15Z
- **Completed:** 2026-05-26T21:19:28Z
- **Tasks:** 2 (Task 2 was TDD: RED → GREEN)
- **Files modified:** 5

## Accomplishments

- `build_code_mode_prompt(connector, config)` thin async alias next to `assemble_code_mode_prompt` — satisfies CONN-04 literal naming, no deprecation marker (P83 dual-naming precedent).
- `DatabaseSection.url: Option<String>` additive field with `env:VAR_NAME` indirection rustdoc; the Wave-0 `seed-database-url.toml` fuzz seed now parses as a valid config (was rejected by `deny_unknown_fields`).
- `synthesize_from_config_with_connector(cfg, Arc<dyn SqlConnector>)` added alongside the unchanged `synthesize_from_config`; both delegate to a shared `synthesize_inner(cfg, Option<connector>)`.
- `apply_widget_meta` flips `_meta.ui.resourceUri` when `ToolDecl.ui_resource_uri.is_some()` so pmcp core's `with_widget_enrichment` populates `structuredContent` (D-06) — fires for both entry points.
- `SynthesizedToolHandler` now holds `connector: Option<Arc<dyn SqlConnector>>`; `handle()` extracts named params (filtered to declared parameters, T-84-03-01) and calls `connector.execute()`, mapping `ConnectorError` through its sanitized `Display` (T-84-03-02), or returns an explicit error when no connector is wired (T-84-03-05).
- `ServerBuilderExt` gained `tools_from_config_with_connector` / `try_tools_from_config_with_connector` Shape A wiring; the existing connector-less methods and all 4 builder_ext tests are unchanged.

## Task Commits

1. **Task 1: build_code_mode_prompt alias + DatabaseSection.url field** — `da2fbc4b` (feat)
2. **Task 2 (TDD RED): failing widget_meta flip tests** — `bd984dec` (test)
3. **Task 2 (TDD GREEN): synthesize_from_config_with_connector + widget_meta flip + handler execute** — `c0b594b3` (feat)

_TDD gate sequence: `test(...)` (RED) → `feat(...)` (GREEN). No REFACTOR commit needed — helpers extracted up-front, all functions cog ≤25._

## TDD Gate Compliance

- RED gate: `bd984dec` (`test(84-03)`) — `widget_meta_flips_when_ui_resource_uri_present` failed before the flip existed; the negative case passed (fail-fast confirmed the positive test was testing the right thing).
- GREEN gate: `c0b594b3` (`feat(84-03)`) — flip implemented via `apply_widget_meta`; both tests pass.

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/code_mode.rs` — added `build_code_mode_prompt` async alias (delegates to `assemble_code_mode_prompt`) with `no_run` doctest.
- `crates/pmcp-server-toolkit/src/config.rs` — added `DatabaseSection.url: Option<String>` above `pool`; added `database_url_optional_field_parses` unit test.
- `crates/pmcp-server-toolkit/src/tools.rs` — `synthesize_inner` shared helper; `synthesize_from_config` delegates `None`; new `synthesize_from_config_with_connector` delegates `Some`; `apply_widget_meta` + `extract_named_params` helpers; `SynthesizedToolHandler` connector field + execute body; 2 in-plan widget_meta tests.
- `crates/pmcp-server-toolkit/src/lib.rs` — additive `pub use crate::tools::synthesize_from_config_with_connector;` (line 91 re-export + line 130 fn-type assertion unchanged).
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — `Arc` + `SqlConnector` imports; new `tools_from_config_with_connector` / `try_tools_from_config_with_connector` trait methods + impls.

## Decisions Made

- **REVIEWS M1 — WidgetMeta API surface:** The plan sketch (`WidgetMeta::new().domain(uri)` via `with_widget_meta`) does not compile in the toolkit: `with_widget_meta` is `#[cfg(feature = "mcp-apps")]` and the toolkit pulls `pmcp` with `default-features = false` (no `mcp-apps`); additionally `WidgetMeta::new()` has no `.domain()` (it exposes `.connect()`/`.resources()` domain setters, not a resource URI). Used `info.with_meta_entry("ui", json!({"resourceUri": uri}))` instead — feature-independent, chainable (preserves annotations), and produces exactly the `ui.resourceUri` shape that `ToolInfo::widget_meta()` recognises. Verified by the GREEN positive test and by clean `--no-default-features` + default-features builds.
- **Single handler struct with `Option<connector>`** rather than two handler types — keeps the no-connector P83 path (explicit runtime Err) and the connector path in one impl.
- **builder_ext additive methods** — `ServerBuilderExt` is implemented on the external `pmcp::ServerBuilder`, so the connector is threaded as a method parameter (new `*_with_connector` methods) rather than a stored field; existing methods stay intact.

## Deviations from Plan

None of the Rule 1–4 deviations. One plan-text adaptation, anticipated and authorised by the plan itself:

### Plan-Authorised Adaptation (REVIEWS M1)

**1. WidgetMeta API surface differs from the plan's illustrative sketch**
- **Found during:** Task 2 (widget_meta flip implementation)
- **Issue:** The plan's `<action>` sketch showed `info.with_widget_meta(WidgetMeta::new().domain(uri))`, but `with_widget_meta` is gated behind pmcp's `mcp-apps` feature (the toolkit does not enable it) and `WidgetMeta::new()` has no `.domain()` method.
- **Resolution:** Used `info.with_meta_entry("ui", json!({"resourceUri": uri}))` — the plan's `<behavior>` REVIEWS M1 clause explicitly mandates "USE THE REAL API … pick the one that COMPILES + makes `info.widget_meta().is_some()` true." This is not a deviation; it is the documented contingency.
- **Files modified:** `crates/pmcp-server-toolkit/src/tools.rs`
- **Verification:** `widget_meta_flips_when_ui_resource_uri_present` (GREEN), `widget_meta_absent_when_ui_resource_uri_none`, default + `--no-default-features` builds.
- **Committed in:** `c0b594b3`

---

**Total deviations:** 0 auto-fixed; 1 plan-authorised API-surface adaptation (REVIEWS M1).
**Impact on plan:** No scope change. All success criteria met.

## Issues Encountered

- Local clippy (rust-1.95.0, newer than CI's pinned stable) surfaces two PRE-EXISTING lints in files this plan touched: `code_mode.rs:207-208` (`field_reassign_with_default`, Phase 83 `build_cm_config`) and `builder_ext.rs:284` (`needless_return`, the Phase 83 `return register_code_mode_tools(...)` line, shifted down by this plan's additions). Both are already logged in `deferred-items.md` and are out of scope (SCOPE BOUNDARY rule). This plan's new functions introduce ZERO new clippy warnings — the warning set is identical to the documented pre-existing list, only line numbers shifted.

## Known Stubs

None. The widget_meta flip is fully wired; `apply_widget_meta` emits real `ui.resourceUri` metadata. The connector-less `synthesize_from_config` path intentionally returns an explicit error on handler invocation (T-84-03-05) — this is documented behaviour, not a stub. The end-to-end D-06 integration test (handler rows → `CallToolResult.structured_content`) is OWNED BY PLAN 04 per REVIEWS H1, which ships `SqliteConnector` for a concrete connector.

## Verification Results

- `cargo build -p pmcp-server-toolkit --features sqlite --features code-mode` — green
- `cargo build -p pmcp-server-toolkit` (default) + `--no-default-features` — green (proves feature-independent widget_meta API)
- `cargo test -p pmcp-server-toolkit --features sqlite --features code-mode --lib` — 101 passed
- `--test reference_configs` — 6 passed (REF-01 superset intact, caller line 70 unmodified)
- `--test tool_synthesis_props` — 3 passed (3 prop tests unmodified)
- doctests `build_code_mode_prompt` (1) + `synthesize` (2) — passed
- `database_url_optional_field_parses` — passed; the `seed-database-url.toml` fuzz seed now parses as a valid config
- PMAT `analyze complexity --max-cognitive 25` on `tools.rs` — zero violations, no `#[allow]` added
- No file deletions across Task 1+2 commits; untracked fuzz corpus / render_ours.rs preserved

## Next Phase Readiness

- Plan 04 can `synthesize_from_config_with_connector(cfg, Arc::new(SqliteConnector::open_in_memory()?))` and ship the moved `tests/synthesizer_structured_content.rs` end-to-end D-06 test.
- Plans 05/06/07 per-backend connectors consume the same new variant via `try_tools_from_config_with_connector` Shape A wiring.
- `DatabaseSection.url` is ready for Postgres/MySQL connector constructors (D-08).

## Self-Check: PASSED

- FOUND: `84-03-SUMMARY.md`
- FOUND commit `da2fbc4b` (Task 1)
- FOUND commit `bd984dec` (Task 2 RED)
- FOUND commit `c0b594b3` (Task 2 GREEN)

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
