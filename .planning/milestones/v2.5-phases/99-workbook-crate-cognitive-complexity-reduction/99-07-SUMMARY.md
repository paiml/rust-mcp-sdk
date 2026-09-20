---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 07
subsystem: pmcp-workbook-compiler / provenance
tags: [refactor, pmat, cognitive-complexity, provenance, behavior-preserving]
requires:
  - "pmcp-workbook-compiler provenance subsystem (raw_parts.rs, gate.rs)"
provides:
  - "parse_calc_pr, parse_app_props, gate_inner all below the PMAT cog-25 gate (<=23)"
affects:
  - "PR #279 PMAT complexity gate (3 of 21 workbook violations cleared)"
tech-stack:
  added: []
  patterns:
    - "per-event handler extraction from quick-xml read-event loops"
    - "borrowed-struct (FreshnessSignals) parameter object for a findings collector"
    - "early-return guard helper (push_identity_finding) + accept-path extraction"
key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/provenance/raw_parts.rs
    - crates/pmcp-workbook-compiler/src/provenance/gate.rs
decisions:
  - "Kept #[allow(clippy::too_many_lines)] on gate_inner: still 153 lines (doc + struct-literal heavy), so the allow remains load-bearing for clippy's separate too_many_lines lint — unrelated to the PMAT cognitive gate this plan targets."
  - "PMAT oracle JSON shape under pmat 3.15.0 is {files, summary{violations}}, NOT a top-level .violations[] — the plan's jq filter was adapted; per-function cog read from a --path-scoped --top-files 200 run (the summary.violations array is top-N truncated and unreliable for verification)."
metrics:
  duration: ~25m
  completed: 2026-06-16
  tasks: 2
  files: 2
---

# Phase 99 Plan 07: Provenance Cognitive-Complexity Reduction Summary

Behavior-preserving decomposition of the three flagged `pmcp-workbook-compiler`
provenance functions (`parse_calc_pr` cog 44, `parse_app_props` cog 39,
`gate_inner` cog 29) below the PMAT cog-25 gate by extracting per-event XML
handlers and the gate's collect-all findings logic into named private helpers —
no change to parsed `RawCalcPr`/`RawAppProps` values or gate accept/refuse
decisions.

## What Was Done

### Task 1 — `raw_parts.rs` parsers (commit `bbfe1543`)
- `parse_calc_pr` (cog **44 → 10**): extracted the per-event branch logic of the
  `quick-xml` read-event loop into `step_calc_pr_event` (cog 22) returning a
  `CalcPrFlow { Continue, Done }` control enum; pulled the workbook-child guard
  into `is_workbook_child_calc_pr` (cog 1). The loop body is now a thin
  read-event → step → break orchestrator with the XML error mapped via
  `map_err` instead of an inline `match` arm.
- `parse_app_props` (cog **39 → 10**): extracted `step_app_props_event` (cog 8)
  and `accumulate_app_text` (cog 2); replaced the stringly-typed
  `Option<&'static str>` current-field tracker with an `AppField` enum.
- `apply_calc_pr_attrs` (cog 13) and `read_named_part` (cog 10) were already
  factored and untouched.

### Task 2 — `gate.rs::gate_inner` (commit `51df4440`)
- `gate_inner` (cog **29 → 20**): extracted the 6 collect-all `oracle/*` finding
  blocks into `collect_freshness_findings` (cog 6), driven by a borrowed
  `FreshnessSignals<'a>` parameter object (no value copied); pulled the WBCO-07
  identity refusal into `push_identity_finding` (cog 4) using an early return for
  the `ExcelTrusted` no-op; extracted the accept-path corpus build into
  `build_oracle_corpus` (cog 0). The orchestrator is now a linear
  read → classify → collect → soften → decide pipeline.

## Verification

- **PMAT gate (oracle):** `pmat analyze complexity --path crates/pmcp-workbook-compiler
  --max-cognitive 1 --top-files 200 --format json` — every provenance function now
  reports cognitive ≤ 23:
  - `parse_calc_pr` = 10, `parse_app_props` = 10, `step_calc_pr_event` = 22,
    `step_app_props_event` = 8, `accumulate_app_text` = 2,
    `is_workbook_child_calc_pr` = 1, `apply_calc_pr_attrs` = 13
  - `gate_inner` = 20, `collect_freshness_findings` = 6,
    `push_identity_finding` = 4, `build_oracle_corpus` = 0
  - A `--max-cognitive 25` workspace run lists **zero** provenance entries in
    both `summary.violations` and `files[].functions`.
- **Tests:** `cargo test -p pmcp-workbook-compiler` → 315 passed, 0 failed, 1
  ignored — green before and after both tasks (including the provenance gate +
  `backward_compat`/quirks/reemit goldens that guard against drift).
- **Clippy:** `cargo clippy -p pmcp-workbook-compiler --all-features -- -D warnings`
  → clean (no warnings, no errors).

## Behavior Preservation

Zero behavior change. The extracted helpers reproduce the exact same:
- `RawCalcPr`/`RawAppProps` field values and `ProvenanceError` variants for every
  input (decoy-`calcPr` shadowing guard, split-text accumulation, depth bound,
  unescape-failure mapping all preserved);
- gate finding set, order, severity, rule codes, and messages, and the
  accept/refuse decision (the stale-cache coherence backstop still fires only
  when no other Error was recorded; the trusted-fixture softening still applies
  only to `SOFTENABLE_FRESHNESS_RULES`).

The existing provenance gate tests and `backward_compat_*` goldens (the
milestone regression net) remain green, confirming no drift.

## Deviations from Plan

### Plan/oracle alignment (no code-behavior deviation)

**1. [Rule 3 - Blocking] PMAT JSON shape differs from the plan's jq filter**
- **Found during:** Task 1 baseline.
- **Issue:** The plan/verify blocks use `pmat analyze complexity ... | jq '.violations[]'`,
  but pmat **3.15.0** emits `{ files, summary{violations}, top_files_limit }` with no
  top-level `.violations` key, and the `summary.violations` array is **top-N
  truncated** (it omitted the provenance functions even when they violated).
- **Fix:** Used `.files[].functions[].metrics.cognitive` from a `--path`-scoped
  `--top-files 200` run as the authoritative per-function oracle, cross-checked
  against a `--max-cognitive 25` workspace run showing zero provenance entries.
  This is a measurement-method adaptation only — the cog targets and gate
  semantics are unchanged.
- **Files modified:** none (verification method only).

### gate_inner `#[allow(clippy::too_many_lines)]` retained
- The plan targeted cognitive complexity (the PMAT gate). `gate_inner` is now cog
  20 but still 153 source lines (heavy doc comments + the `OracleProvenance` and
  `FreshnessSignals` struct literals), which exceeds clippy's separate
  `too_many_lines` threshold (100). The pre-existing `#[allow(clippy::too_many_lines)]`
  is therefore still required to keep `clippy -D warnings` green and was left in
  place. This is orthogonal to the PMAT cognitive gate (`#[allow]` has no effect
  on PMAT per phase D-10-B).

No `#[allow(clippy::cognitive_complexity)]` was added and `.pmatignore` was not
touched (both forbidden by the phase context).

## Known Stubs

None. No placeholder values, TODOs, or unwired data paths were introduced; all
extracted helpers carry real logic exercised by the existing test net.

## Self-Check: PASSED

- `crates/pmcp-workbook-compiler/src/provenance/raw_parts.rs` — FOUND (modified, committed `bbfe1543`)
- `crates/pmcp-workbook-compiler/src/provenance/gate.rs` — FOUND (modified, committed `51df4440`)
- Commit `bbfe1543` — FOUND
- Commit `51df4440` — FOUND
