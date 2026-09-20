---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
plan: 01
subsystem: compiler
tags: [workbook, dialect, semver, version-declaration, fail-closed, fuzz, drift-guard, rust]

# Dependency graph
requires:
  - phase: 91-workbook-runtime-purity-gate-dialect-spec
    provides: pmcp-workbook-dialect crate (WHITELIST + dialect_spec doc↔const drift-guard pattern), docs/workbook-dialect-spec.md
  - phase: 94 (workbook CLI)
    provides: version.rs read_workbook_version (the bundle-version reader this plan mirrors but must NOT edit)
  - phase: 93 (compiler)
    provides: compile_workbook_inner ingest→stage1 pipeline + CompileError::Lint typed error + WorkbookMap
provides:
  - "pmcp_dialect_version reader (src/dialect_version.rs): a SIBLING of version.rs with absent→baseline (D-05) semantics"
  - "Hand-rolled MAJOR.MINOR[.PATCH] parser (public parse_dialect_version) + fail-closed semver-compat decision (D-04)"
  - "SUPPORTED_DIALECT_VERSION + BASELINE_DIALECT_VERSION consts in pmcp-workbook-dialect, bound to the spec doc by a drift guard"
  - "Wired dialect-version validation into compile_workbook_inner at step (2a)"
  - "Registered fuzz target dialect_version_parse + a compat-decision property grid + an ALWAYS example"
affects: [96-04 (CLI), 96-05 (Shape A binary), future dialect-version evolution, workbook authoring]

# Tech tracking
tech-stack:
  added: []  # No new external dependencies (hand-rolled parse, A1)
  patterns:
    - "Sibling reader with inverted absence policy (absent→baseline vs version.rs absent→error)"
    - "doc↔const drift guard cloned for a second contract surface (version values, parallel to WHITELIST)"
    - "Non-masking fuzz verify (grep gate, no || echo fallthrough)"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/dialect_version.rs
    - crates/pmcp-workbook-compiler/examples/dialect_version_demo.rs
    - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dialect_version_parse.rs
  modified:
    - crates/pmcp-workbook-dialect/src/lib.rs
    - docs/workbook-dialect-spec.md
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/fuzz/Cargo.toml

key-decisions:
  - "BASELINE_DIALECT_VERSION = SUPPORTED_DIALECT_VERSION = \"1.0\" (D-05 discretion; baseline == current supported this phase)"
  - "Reserved named range = pmcp_dialect_version (D-03); single-cell defined-name scan, case-insensitive"
  - "PATCH parsed but IGNORED for compatibility — compat decided on MAJOR.MINOR only"
  - "Dialect-version check placed as a standalone gate immediately after ingest (step 2a), the sanctioned A2 alternative to a stage-1 finding; surfaces the same typed CompileError::Lint before stage-1"
  - "Hand-rolled base-10 parser (no semver crate, A1) — matches version.rs's crate-free posture and keeps the purity-gated crate dependency-free"

patterns-established:
  - "Pattern: sibling defined-name reader that inverts the absence policy without touching the original (version.rs stayed byte-unchanged)"
  - "Pattern: a second doc↔const drift-guard module (dialect_version_spec) reusing the .git-guarded SKIP/FAIL + non-empty-parse guards verbatim"

requirements-completed: [WBDL-02]

# Metrics
duration: 8min
completed: 2026-06-15
---

# Phase 96 Plan 01: Dialect-Version Declaration (WBDL-02) Summary

**A `pmcp_dialect_version` named-range reader with a hand-rolled MAJOR.MINOR[.PATCH] parser, fail-closed semver-compat (different major / newer minor → typed CompileError), absent→baseline (D-05 zero-churn), bound to the spec doc by a drift guard and wired into the compile pipeline — plus a registered fuzz target and a runnable example.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-06-15T08:00:12Z
- **Completed:** 2026-06-15T08:08Z
- **Tasks:** 3
- **Files modified:** 7 (3 created, 4 modified)

## Accomplishments
- Added `BASELINE_DIALECT_VERSION` / `SUPPORTED_DIALECT_VERSION` consts to `pmcp-workbook-dialect` (both `"1.0"`), a §7 version-declaration policy section to `docs/workbook-dialect-spec.md`, and a `dialect_version_spec` drift-guard test binding the two — flipping a const without editing the doc fails the build (verified).
- Created `src/dialect_version.rs`: a sibling of `version.rs` that reads `pmcp_dialect_version`, with a public hand-rolled `MAJOR.MINOR[.PATCH]` parser, a `DialectVersion` value, `validate_declared` (D-04 fail-closed semver-compat), and `resolve_dialect_version` (D-05 absent→baseline). `version.rs` left byte-unchanged.
- Wired `resolve_dialect_version` into `compile_workbook_inner` at step (2a); a compatible/absent version doesn't change behavior (all 291 compiler tests + reemit_golden/tax-calc still pass = D-05 zero-churn witness).
- ALWAYS coverage: full grammar + compat unit matrix, a 5-case wired-path integration module, a compat-decision property grid, a runnable example (`dialect_version_demo`, exits 0), and a registered fuzz target (`dialect_version_parse`) that ran 200k nightly runs clean.

## Task Commits

Each task was committed atomically:

1. **Task 1: version consts + spec policy + drift guard (dialect crate)** — `14047806` (feat)
2. **Task 2: dialect_version.rs reader + semver-compat decision + ALWAYS example** — `45e4fa4f` (feat)
3. **Task 3: wire reader into compile_workbook_inner + integration test + fuzz target** — `4702a329` (feat)

_TDD note: Tasks 1 and 2 were authored test-and-impl together (single feat commit each) rather than split RED/GREEN commits; the behaviors were verified failing-then-passing locally (e.g. the drift-guard flip was confirmed to FAIL before commit). See TDD Gate Compliance below._

## Files Created/Modified
- `crates/pmcp-workbook-dialect/src/lib.rs` — added the two version consts + the `dialect_version_spec` drift-guard test module.
- `docs/workbook-dialect-spec.md` — added §7 (version declaration grammar, fail-closed compat rule, absent→baseline, parse-stable version table).
- `crates/pmcp-workbook-compiler/src/dialect_version.rs` — the new reader, parser, compat decision, resolve entry point, unit/property/wired-integration tests.
- `crates/pmcp-workbook-compiler/examples/dialect_version_demo.rs` — the ALWAYS example over absent/compatible/incompatible.
- `crates/pmcp-workbook-compiler/src/lib.rs` — `pub mod dialect_version;` (Task 2), the step-(2a) call site + the `resolve_dialect_version` re-export (Task 3).
- `crates/pmcp-workbook-compiler/fuzz/Cargo.toml` — registered the `dialect_version_parse` `[[bin]]`.
- `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dialect_version_parse.rs` — the new fuzz target over the public parser.

## Decisions Made
- **Integration-seam placement (A2 choice):** the plan offered either a stage-1 collect-all finding OR a standalone post-ingest gate, stating "both work." I chose the standalone gate at step (2a) because it surfaces the same typed `CompileError::Lint` in the same refuse path WITHOUT re-opening the stable `run_stage1` signature (the phase boundary forbids re-opening Phases 91-93). An incompatible dialect is refused before any synth/reconcile work; absent/compatible is a no-op.
- **Integration test shape:** `compile_workbook_inner` requires a real `.xlsx` (umya ingest), so the "wired path over synthetic maps" is expressed by driving `resolve_dialect_version` — the exact function the pipeline calls at step 2a — over all five cases. The full real-`.xlsx` absent→baseline end-to-end witness is `reemit_golden` (tax-calc declares no version cell and still compiles).
- Everything else followed the plan as specified.

## Deviations from Plan

None requiring a deviation rule. The two judgment calls above (integration-seam placement, integration-test shape) were both explicitly sanctioned by the plan/research (A2 "both work"; the integration-test-over-resolve approach is the only feasible synthetic-map path given the real-file pipeline). No bugs, no missing critical functionality, no blocking issues, no architectural changes.

## Issues Encountered
- The `wired_path_integration` module initially referenced `super::tests::map_declaring`, which is private to the sibling `tests` module. Resolved by giving the integration module its own local `map_declaring` helper (no visibility coupling). Caught before any commit.

## TDD Gate Compliance
This plan's tasks are marked `tdd="true"` but were committed as combined feat commits (test + impl together) rather than separate RED/GREEN commits. The RED behavior was verified manually before each commit:
- Task 1: the drift guard was confirmed to FAIL when a const was flipped to "9.9" without editing the doc, then restored (the canonical RED→GREEN witness for the drift guard).
- Task 2: the grammar/compat unit matrix and property grid all pass; the malformed/overflow/incompatible cases assert typed errors.
This is a process note, not a correctness gap — the behaviors are fully test-covered.

## Quality Gate Status
- `cargo fmt` — clean on both touched crates.
- `cargo clippy -p pmcp-workbook-dialect --all-targets` — No issues found.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` — No issues found (includes the example + tests).
- `cargo test -p pmcp-workbook-dialect` — 6 passed.
- `cargo test -p pmcp-workbook-compiler` — 291 passed (incl. reemit_golden/tax-calc D-05 witness).
- `cargo run -p pmcp-workbook-compiler --example dialect_version_demo` — exits 0, prints all three outcomes.
- Grep gates: fuzz `[[bin]]` registered AND target calls the public parser — both pass (non-masking).
- **Manual nightly fuzz gate:** `cargo +nightly fuzz run dialect_version_parse` ran 200,000 runs in ~5s with NO crash/panic (nightly + cargo-fuzz both available locally).
- `version.rs` byte-unchanged (`git diff --stat` empty); exactly one `pub mod dialect_version;` declaration.
- **Deferred:** the full `make quality-gate` (workspace fmt/clippy-pedantic+nursery/build/test/audit) was NOT run for this plan — only the per-crate fmt/clippy/test for the touched crates. Recommended to run the full gate at the phase wave merge before `/gsd:verify-work`.

## Next Phase Readiness
- WBDL-02 is satisfied: the compiler reads + validates a workbook's declared dialect version fail-closed (D-04), treats absent as baseline (D-05), and the version contract cannot drift from the spec doc.
- The public `parse_dialect_version` / `validate_declared` / `resolve_dialect_version` API is available for downstream plans (96-04 CLI, 96-05 Shape A) and the fixture-authoring plans (96-02/03), which may declare an explicit `pmcp_dialect_version` cell to exercise the present-path.
- No blockers.

## Self-Check: PASSED

- Created files exist: `src/dialect_version.rs`, `examples/dialect_version_demo.rs`, `fuzz/fuzz_targets/dialect_version_parse.rs` — all FOUND.
- Commits exist: `14047806`, `45e4fa4f`, `4702a329` — all FOUND in git log.
- Contains assertions: `pmcp_dialect_version` in dialect_version.rs and the spec doc; `parse_dialect_version` in the example; `SUPPORTED_DIALECT_VERSION` in the dialect crate — all present.

---
*Phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation*
*Completed: 2026-06-15*
