---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 10
subsystem: pmcp-server-toolkit / workbook input validation
tags: [refactor, complexity, pmat-gate, behavior-preserving, CPLX-03]
requires:
  - "PR #279 PMAT complexity gate (CI-only)"
provides:
  - "validate_input cog 33 -> <=5 (gate <=23) — server-toolkit crate clears PMAT"
affects:
  - "crates/pmcp-server-toolkit/src/workbook/input.rs"
tech-stack:
  added: []
  patterns:
    - "Multi-phase function split (parse -> validate -> build) into per-phase helpers with a thin orchestrator"
    - "Match-ladder isolated into a classify_* helper returning accept-or-reject Result"
key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/workbook/input.rs"
decisions:
  - "Single commit for both plan tasks: decomposition and clippy/test verification are one cohesive change to one file."
  - "Pre-existing, unrelated clippy warnings (pmcp-code-mode/avp.rs, http/auth.rs) logged to deferred-items.md, NOT fixed (scope boundary)."
metrics:
  duration: "~12m"
  completed: "2026-06-16"
  tasks: 2
  files: 1
requirements: [CPLX-03]
---

# Phase 99 Plan 10: Workbook validate_input Complexity Reduction Summary

Decomposed `pmcp-server-toolkit/src/workbook/input.rs::validate_input` (cog 33, the sole PMAT-flagged function in the server-toolkit crate) into a thin orchestrator over four named per-phase helpers, dropping its cognitive complexity from 33 to <=5 — well under the gate's recommended-23 tier — with zero behavior change.

## What Was Done

`validate_input` ran three sequential phases inline (tier-default seeding, supplied-input mapping, override tier-checking). The override phase's 4-arm `match` ladder nested inside the loop was the dominant complexity driver. Extracted:

- `seed_tier_defaults(manifest) -> BTreeMap` — phase 1: seed each `Role::Input` cell with its tier default.
- `seed_supplied_inputs(inputs, manifest, cell_map, &mut seeds) -> Result<()>` — phase 2: map each input key to its `seed_coord`, fail-closed on unknown key / roleless seed (WR-05), dtype-check and overlay.
- `seed_accepted_overrides(overrides, manifest, &mut seeds) -> Result<Vec<String>>` — phase 3: iterate overrides, classify, dtype-check, seed, collect accepted keys.
- `classify_override(manifest, key) -> Result<&CellRole>` — the isolated accept-or-reject match ladder (strict-constant -> V4 reject; computed -> WR-02 reject; unknown -> reject; variable-tier -> accept).

`validate_input` is now: parse DTO -> `seed_tier_defaults` -> `seed_supplied_inputs` -> `seed_accepted_overrides` -> build canonical DTO -> return.

## Behavior Preservation

Every error variant, error message, and fail-closed gate is identical to the original:
- `invalid_input` on arg-parse / roleless-seed,
- `invalid_input_field` on unknown input key,
- `strict_constant_override` (V4),
- `unsupported_option` on computed-cell / unknown override (WR-02),
- dtype + closed-enum gates unchanged (still via `check_value_dtype`).

The override classification order (strict-constant before computed before accept before unknown) is preserved exactly via guard-ordered match arms. `.clone()` on keys became `.to_string()` only where the helper takes `&str` (semantically identical owned String).

## Verification

| Check | Result |
| ----- | ------ |
| PMAT oracle: `input.rs` files flagged at `--max-cognitive 25` | **0** (was 1) |
| validate_input cognitive (via `.files[].functions[].metrics.cognitive`) | **33 -> <=5** |
| `cargo test -p pmcp-server-toolkit` | **121+ passed, 0 failed** (lib unit + doctests + integration) |
| clippy on `workbook/input.rs` | **0 warnings** (new helpers are private, no doc requirement) |

PMAT JSON note: this PMAT build (3.x) exposes results under `.files[].functions[].metrics.cognitive`, not the `.violations[]` path the plan's verify snippet assumed (that path yields `null` -> jq error -> empty output, which would falsely pass `test -z`). The oracle used here (`.files[] | select(path) | functions[] | metrics.cognitive`) is the reliable form and confirms the gate clears.

## Deviations from Plan

### Out-of-scope discoveries (logged, NOT fixed)

Pre-existing clippy `-D warnings` failures in files NOT touched by this plan (scope boundary — predate base commit a4d60cb6):
- `crates/pmcp-code-mode/src/avp.rs:59` — `clippy::derivable_impls`.
- `crates/pmcp-server-toolkit/src/http/auth.rs:538` — `clippy::redundant_guards`.
- test-only `unused import: pmcp_code_mode::CodeExecutor`.

These make a literal `cargo clippy -p pmcp-server-toolkit --all-features -- -D warnings` (plan Task 2 verify) non-zero, but they are unrelated pre-existing debt. Per project MEMORY, the real CI merge gate lints only root `pmcp` with a generous allow-list and these toolkit/code-mode crates are not `-D warnings`-gated, so they do not block CI. Logged to `deferred-items.md`. My changed file is clippy-clean.

No `#[allow(clippy::cognitive_complexity)]` added; no `.pmatignore` edit (both would be no-ops for PMAT and are out of scope).

## Self-Check: PASSED

- FOUND: crates/pmcp-server-toolkit/src/workbook/input.rs (modified, committed cb5d6e03)
- FOUND: commit cb5d6e03 in git log
- VERIFIED: input.rs flagged-file count at PMAT gate threshold 25 = 0
