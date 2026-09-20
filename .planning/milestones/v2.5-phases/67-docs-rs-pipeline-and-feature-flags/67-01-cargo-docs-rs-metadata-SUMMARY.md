---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 01
subsystem: infra
tags:
  - rust
  - cargo
  - docs-rs
  - feature-flags
  - arm64

# Dependency graph
requires:
  - phase: 66-macros-documentation-rewrite
    provides: include_str! pattern reference for crate-level docs (pmcp-macros precedent)
provides:
  - Cargo.toml [package.metadata.docs.rs] with explicit 15-feature list (D-16)
  - docs.rs build targets expanded to x86_64-unknown-linux-gnu + aarch64-unknown-linux-gnu (D-18)
  - Single-source-of-truth feature list that Plan 03 (CRATE-README.md table) and Plan 05 (Makefile doc-check) must mirror
affects:
  - 67-02-manual-doc-cfg-cleanup
  - 67-03-crate-readme-and-include-str
  - 67-05-makefile-and-ci-gate
  - 67-06-final-integration-verification

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Explicit feature-list metadata instead of all-features = true on docs.rs
    - ARM64 (aarch64-unknown-linux-gnu) as first-class docs.rs target alongside x86_64

key-files:
  created: []
  modified:
    - Cargo.toml

key-decisions:
  - "D-16: explicit 15-feature list replaces all-features = true on docs.rs"
  - "D-17: exclude unstable, test-helpers, wasm*, full, and *_example gates from docs.rs rendering"
  - "D-18: two docs.rs targets (x86_64-unknown-linux-gnu + aarch64-unknown-linux-gnu) for ARM64 positioning"
  - "D-19: no default-target override — docs.rs defaults to first entry (x86_64)"
  - "D-28: no pmcp version bump — stays at 2.3.0"
  - "logging intentionally omitted from features list (implicit via default = [logging])"

patterns-established:
  - "Pattern 1: Cargo.toml [package.metadata.docs.rs] is the single source of truth for the feature set docs.rs renders; Plan 03 CRATE-README.md table and Plan 05 Makefile doc-check must mirror this list exactly (with the single permitted diff being CRATE-README.md adding a logging row per D-13)."
  - "Pattern 2: docs.rs target list includes aarch64-unknown-linux-gnu to surface ARM64 as a first-class PMCP deployment target (AWS Graviton / Ampere cost positioning)."

requirements-completed:
  - DRSD-02

# Metrics
duration: ~1min
completed: 2026-04-12
---

# Phase 67 Plan 01: Cargo docs.rs Metadata Summary

**Replaced `all-features = true` with an explicit 15-feature list and added `aarch64-unknown-linux-gnu` as a second docs.rs target, making docs.rs render only the user-facing feature surface on both x86_64 and ARM64.**

## Performance

- **Duration:** ~1 min (single-task metadata edit)
- **Started:** 2026-04-12T00:52:51Z
- **Completed:** 2026-04-12T00:52:59Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Replaced the 3-line `[package.metadata.docs.rs]` block (`all-features = true` + `rustdoc-args`) with the 22-line explicit D-16 configuration.
- Established an explicit 15-feature list (composition, http, http-client, jwt-auth, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket) that will be mirrored by Plan 03 (`CRATE-README.md` feature table) and Plan 05 (`make doc-check`).
- Added `aarch64-unknown-linux-gnu` alongside `x86_64-unknown-linux-gnu` so docs.rs renders ARM64 as a first-class target (D-18, ARM64 pragmatic positioning).
- Preserved `rustdoc-args = ["--cfg", "docsrs"]` unchanged so the existing `#![cfg_attr(docsrs, feature(doc_cfg))]` machinery keeps working.
- Omitted `unstable`, `test-helpers`, `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http`, `authentication_example`, `cancellation_example`, `progress_example`, `full`, and `logging` per D-17 (internal / WASM-conflict / example gates / redundant meta / implicit-via-default respectively).
- Kept pmcp at version `2.3.0` (D-28, no version bump for this infrastructure-only phase).
- No `default-target` override added (D-19, docs.rs picks the first entry of `targets`).

## Task Commits

Each task was committed atomically (using `--no-verify` per parallel-executor protocol; orchestrator validates hooks after the wave completes):

1. **Task 1: Rewrite [package.metadata.docs.rs] block in Cargo.toml** — `cf09052e` (feat)

## Files Created/Modified

- `Cargo.toml` — `[package.metadata.docs.rs]` block at lines 507–526 rewritten from 3 lines to 22 lines (`features = [...]` + `targets = [...]` + `rustdoc-args = [...]`). Diff: +18 / -1 line. No other sections touched.

## Verification (Acceptance Criteria)

All acceptance criteria from the plan's `<acceptance_criteria>` block verified (except `cargo check`, which is deferred to Plan 06 per phase CONTEXT.md scope discipline and this plan's explicit no-build constraint):

| Check | Expected | Actual |
|-------|----------|--------|
| `grep -c 'all-features = true' Cargo.toml` | 0 | 0 (pass) |
| Each of 15 features appears exactly once as `^    "<name>",$` | 15 matches (1 each) | 15 matches at Cargo.toml:509–523 (pass) |
| `logging` in features block | 0 | 0 (pass; implicit via `default = ["logging"]`) |
| `full` in features block | 0 | 0 (pass) |
| `unstable` in features block | 0 | 0 (pass) |
| `test-helpers`/`wasm`/`websocket-wasm`/`wasm-tokio`/`wasi-http`/`*_example` | 0 each | 0 each (pass) |
| `targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]` | 1 match | 1 match at Cargo.toml:525 (pass) |
| `rustdoc-args = ["--cfg", "docsrs"]` | 1 match | 1 match at Cargo.toml:526 (pass) |
| `default-target` | 0 matches | 0 matches (pass, D-19) |
| `^version = "2.3.0"` | 1 match | 1 match at Cargo.toml:3 (pass, D-28) |
| `cargo check` with D-16 feature set | exits 0 | **deferred to Plan 06** (per phase no-build discipline — Plan 06 is the final integration verification gate and runs the full `make doc-check`) |

## Decisions Made

None beyond the locked D-16, D-17, D-18, D-19, D-28 decisions — plan executed verbatim. The logging row in Plan 03's CRATE-README.md table (D-13) is the one permitted diff between this Cargo.toml feature list and the CRATE-README.md feature table; that is Plan 03's responsibility and is not implemented here.

## Deviations from Plan

None — plan executed exactly as written. The Task 1 action wrote the D-16 block verbatim. No bugs found. No missing critical functionality discovered. No blocking issues. No architectural questions.

**Total deviations:** 0
**Impact on plan:** None.

## Issues Encountered

None.

## User Setup Required

None — this is a purely static build-metadata edit. No environment variables, no external services, no dashboard configuration. docs.rs will pick up the new metadata on the next pmcp re-render (automatic on next crate version publish, or manual via the docs.rs rebuild button — out of scope for this plan per D-28).

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. Only build metadata modified. Threat T-67-01-01 (`[package.metadata.docs.rs] features` information-disclosure via `all-features = true` surfacing internal `test-helpers`/`unstable` APIs) is now **mitigated** by the explicit feature list. Threat T-67-01-02 (`Cargo.toml` tampering) remains **accepted** per the plan — source control + pre-commit hooks handle it as designed.

## Next Plan Readiness

- **Plan 67-02** (manual doc_cfg cleanup) can proceed independently — it touches `src/lib.rs`, `src/types/mod.rs`, `src/server/mod.rs`, not Cargo.toml. No conflict.
- **Plan 67-03** (CRATE-README.md + include_str!) depends on this plan via the single-source-of-truth invariant: the feature table in `CRATE-README.md` must reference the same 15 features listed here plus `logging` (D-13).
- **Plan 67-05** (Makefile + CI gate) must use the same 15-feature list in the `make doc-check` recipe (D-23 explicit feature string).
- **Plan 67-06** (final integration verification) will enforce the single-source-of-truth invariant between this Cargo.toml block, CRATE-README.md, and the Makefile, and will run `cargo doc --no-deps --features <D-16 list>` on stable as the final gate.

## Self-Check: PASSED

- FOUND: `Cargo.toml` (modified — `[package.metadata.docs.rs]` block at lines 507–526)
- FOUND: `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-01-cargo-docs-rs-metadata-SUMMARY.md` (created)
- FOUND: commit `cf09052e` (`feat(67-01): rewrite [package.metadata.docs.rs] with explicit feature list (D-16)`)

All claimed files exist on disk and the task commit is present in `git log`.

---
*Phase: 67-docs-rs-pipeline-and-feature-flags*
*Plan: 01*
*Completed: 2026-04-12*
