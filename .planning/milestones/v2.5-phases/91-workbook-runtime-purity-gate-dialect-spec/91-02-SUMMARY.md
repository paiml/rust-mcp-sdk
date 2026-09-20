---
phase: 91-workbook-runtime-purity-gate-dialect-spec
plan: 02
subsystem: dialect
tags: [workbook, excel, dialect, whitelist, governance, binding-test, contract]

# Dependency graph
requires:
  - phase: 91-01
    provides: pmcp-workbook-runtime::finding::{LintFinding, LintReport, Severity} (re-exported per D-03)
  - phase: lighthouse (towelrads quote-pricing)
    provides: dialect/rules.rs (WHITELIST + DialectRules + CandidateRole) + dialect/mod.rs binding test + docs/workbook-dialect-spec.md
provides:
  - "pmcp-workbook-dialect — reader-free leaf crate (slot 2b) holding the dialect contract: the flat 13-fn WHITELIST const (D-05), DialectRules + CandidateRole + colour ontology, the published docs/workbook-dialect-spec.md, and the doc↔const binding test (WBDL-01)"
  - "docs/workbook-dialect-spec.md — SDK-owned, versioned, BA/auditor-facing dialect contract with a flat 13-row whitelist table"
  - "A passing doc↔const binding test (doc_whitelist_table_matches_const) that fails the build if the spec doc table and WHITELIST const diverge"
  - "Re-exported finding types (LintFinding/LintReport/Severity) sourced from the runtime (D-03)"
affects: [Phase 93 (linter execution + WorkbookMap layer on this contract), Plan 91-03 (purity gate negatively asserts reader/JS absence in the dialect tree)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SDK literal-version manifest convention + version+path workspace-internal dep (pmcp-sql-server:33 shape)"
    - "Doc↔const binding test: parse the published markdown whitelist table, assert BTreeSet set-equality with the enforced const, with a !doc_set.is_empty() drift guard (Pitfall 4)"
    - "Reader-free leaf: depends ONLY on the runtime; no serde, no third-party — cargo-tree-provable"

key-files:
  created:
    - crates/pmcp-workbook-dialect/Cargo.toml
    - crates/pmcp-workbook-dialect/src/lib.rs
    - docs/workbook-dialect-spec.md
  modified:
    - Cargo.toml

key-decisions:
  - "D-05 flatten applied: the WHITELIST is 13 flat first-class names; the spec table relabels all 13 rows to a single 'whitelist' category and the binding-test predicate keys on category == \"whitelist\" (no 8-core/5-D-09-widened tiering)"
  - "No serde dep on the dialect crate — DialectRules/CandidateRole/WHITELIST carry no serde derives and need none this phase"
  - "Linter execution + WorkbookMap deliberately NOT lifted (D-02 — Phase 93); the lighthouse dialect/mod.rs linter/lint re-exports were excluded and an acceptance grep asserts their absence"
  - "Added #[must_use] to the pure getters (label/whitelist/sheet_layer_prefixes/candidate_role) — SDK clippy pedantic surfaces it where the lighthouse (looser gate) did not (Rule 1 polish, see Deviations)"

patterns-established:
  - "Two-source merge lift: rules.rs (the contract) + dialect/mod.rs (the binding-test mechanism) collapse into a single-file dialect crate lib.rs"
  - "Binding-test category-predicate co-evolution: flattening the published table to one category value and the parser predicate to match it, in lockstep, keeps WBDL-01 enforced"

requirements-completed: [WBDL-01]

# Metrics
duration: ~9min
completed: 2026-06-10
---

# Phase 91 Plan 02: Workbook Dialect Contract (reader-free leaf) Summary

**Reader-free pmcp-workbook-dialect leaf crate (slot 2b) holding the SDK-owned dialect governance contract — the flat 13-function WHITELIST const (D-05), DialectRules + CandidateRole + colour ontology lifted from the lighthouse compiler, the published docs/workbook-dialect-spec.md, and a doc↔const binding test that fails the build on drift (WBDL-01) — depending only on the runtime whose finding types it re-exports (D-03).**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-06-10
- **Completed:** 2026-06-10
- **Tasks:** 3 executed (all auto; Task 3 TDD-lift)
- **Files created/modified:** 4 (3 created + root Cargo.toml)

## Accomplishments
- Created `crates/pmcp-workbook-dialect/Cargo.toml` in SDK literal-version convention (literal `0.1.0`, `version + path` dep on `pmcp-workbook-runtime`, no serde) and registered it as workspace slot 2b.
- Ported the lighthouse `docs/workbook-dialect-spec.md` to repo-root `docs/` and applied the D-05 flatten: all 13 whitelist rows relabelled to a single `whitelist` category (dropping the `core` / `**D-09 widened**` tiering), the total line reduced to "Total: **13 names**", and the binding-test note repointed at `pmcp_workbook_dialect`.
- Built `crates/pmcp-workbook-dialect/src/lib.rs` by merging two lighthouse sources: `rules.rs` (the `WHITELIST` const flattened to 13 flat names, `DialectRules`, `CandidateRole`, colour-ontology ARGB constants) and `dialect/mod.rs`'s `#[cfg(test)] mod dialect_spec` binding test, adapted for D-05 (predicate `category == "whitelist"`, kept the `!doc_set.is_empty()` Pitfall-4 guard).
- Re-exported `pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity}` (D-03); deliberately did NOT re-export `linter`/`lint` (Phase 93, D-02) — an acceptance grep confirms their absence.
- 4 lib tests pass under `--test-threads=1` (including the WBDL-01 binding test); `cargo clippy -p pmcp-workbook-dialect --all-targets -- -D warnings` is clean (panic-freedom deny lints hold).
- Confirmed the reader-free boundary: `cargo tree -p pmcp-workbook-dialect` shows only `pmcp-workbook-runtime` + its writer-only transitives — no `umya`/`calamine`/`quick-xml`/`swc`/`pmcp-code-mode`.

## Task Commits

1. **Task 1: Create the dialect crate manifest + register in workspace** — `cf61a8ba` (feat)
2. **Task 2: Port the dialect spec doc + flatten the whitelist table (D-05)** — `e7f0880e` (docs)
3. **Task 3: Lift rules.rs + the binding test; flatten for D-05; re-export findings (D-03)** — `b579a476` (feat, TDD-lift — lifted tests + the binding test pass on first run)

**Plan metadata:** (final commit — docs: complete plan)

## Files Created/Modified
- `crates/pmcp-workbook-dialect/Cargo.toml` — SDK-convention manifest: literal version 0.1.0, `version + path` dep on the runtime, no serde, no `version.workspace`.
- `crates/pmcp-workbook-dialect/src/lib.rs` — flat-13 `WHITELIST` + `DialectRules` + `CandidateRole` + colour ontology + re-exported finding types + the `dialect_spec` binding test (+ the flattened `whitelist_is_exactly_the_thirteen_names` test and the palette/default-construction unit tests).
- `docs/workbook-dialect-spec.md` — published BA/auditor-facing contract with the flat 13-row whitelist table.
- `Cargo.toml` (root) — appended `crates/pmcp-workbook-dialect` to the `[workspace] members` array (immediately after the runtime crate).

## Decisions Made
- **D-05 flatten applied consistently across doc + const + test:** the published table, the `WHITELIST` const, and the binding-test predicate were flattened in lockstep — the table category column carries a single `whitelist` value, the const drops the two-tier comment framing, and the parser keys on `category == "whitelist"`. This keeps WBDL-01 enforced without a tiering concept.
- **No serde dependency** on the dialect crate — the lifted types carry no serde derives and need none this phase (matching the plan's explicit instruction and the lighthouse source).
- **Linter/WorkbookMap excluded (D-02):** only `rules.rs` + the binding test were lifted; the lighthouse `dialect/mod.rs` `linter`/`lint` re-exports are out of scope (Phase 93). An acceptance grep (`! grep 'pub use.*lint'`) negatively asserts this.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Lint] Added `#[must_use]` to the pure getters**
- **Found during:** Task 3
- **Issue:** The SDK clippy gate (pedantic) flags `clippy::must_use_candidate` on the pure accessors (`CandidateRole::label`, `DialectRules::whitelist`/`sheet_layer_prefixes`/`candidate_role`). The lighthouse source has a looser gate and omits the attribute, so a verbatim lift would have tripped `cargo clippy -- -D warnings`.
- **Fix:** Added `#[must_use]` to those four pure getters. No behaviour change — purely a lint-conformance polish to match the SDK Toyota-Way ceiling.
- **Files modified:** `crates/pmcp-workbook-dialect/src/lib.rs`
- **Commit:** `b579a476`

Otherwise the plan executed as written: the flattened whitelist + binding test passed on the first build, the re-export surface matches D-03, and the reader-free boundary holds.

## Issues Encountered
None. The two-source merge compiled and tested green on the first run; the binding test confirmed the doc↔const set-equality immediately.

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- Slot 2b (`pmcp-workbook-dialect`) is complete: compiles, 4 tests green (incl. WBDL-01 binding test), clippy-clean, reader-free, registered as a workspace member.
- Plan 91-03 (purity gate) now has a second crate tree to negatively assert against: `cargo tree -p pmcp-workbook-dialect` already shows no reader/JS dep, and the runtime's writer-only `rust_xlsxwriter` is the only zip path.
- Phase 93 (linter) can layer its execution + `WorkbookMap` on top of this contract, consuming `DialectRules`/`WHITELIST` and the re-exported finding types — the contract↔doc binding is now build-enforced so the auditor-facing spec can never silently drift from the enforced rule.

## Self-Check: PASSED

- Created files verified on disk: crates/pmcp-workbook-dialect/Cargo.toml, crates/pmcp-workbook-dialect/src/lib.rs, docs/workbook-dialect-spec.md — all FOUND.
- Commits verified in git log: cf61a8ba (Task 1), e7f0880e (Task 2), b579a476 (Task 3) — all FOUND.

---
*Phase: 91-workbook-runtime-purity-gate-dialect-spec*
*Completed: 2026-06-10*
