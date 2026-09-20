---
phase: 94-cli-subcommands-pmcp-toml
plan: 05
subsystem: testing
tags: [cargo-pmcp, workbook-compiler, purity-gate, integration-test, cargo-example, governance-gate]

# Dependency graph
requires:
  - phase: 94-00
    provides: "library seams — the gated lane (prepare_candidate → gate → promote) and the hash-covered ungated marker channel (write_gate_marker / read_gate_marker)"
  - phase: 94-01
    provides: "pmcp.toml [[workbook]] config parser (PmcpToml / WorkbookEntry)"
  - phase: 94-02
    provides: "workbook lint handler (dialect linter standalone, --format json, D-10 exit codes)"
  - phase: 94-03
    provides: "workbook compile handler (seed + gated-update lanes, --approver, compile-all)"
  - phase: 94-04
    provides: "workbook emit handler (ungated seed lane, UNGATED banner, hash-covered gated:false marker)"
  - phase: 93
    provides: "the sole reusable workbook fixture tax-calc.xlsx + provenance-override"
provides:
  - "End-to-end CLI integration coverage driving the REAL cargo-pmcp binary over workbook lint/compile/emit + compile-all (WBCL-01..04, D-05/06/08/09/10)"
  - "Purity-boundary confirmation: cargo-pmcp links the compiler (umya enters its offline tree) but every SERVED reader-free tree stays umya/quick-xml-free — fast default-run cargo-tree assertions + an #[ignore]-d make-purity-check guard"
  - "A runnable cargo-run example demonstrating the workbook CLI surface over the reused fixture (CLAUDE.md ALWAYS: cargo run --example)"
affects: [94-06, phase-95, phase-96, workbook-cli, release]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Example-via-library-verbs: a cargo example links the cargo_pmcp LIBRARY (handlers are bin-only / commands::* excluded from lib.rs, and examples get no dev-deps), so it demonstrates the subcommand flow by calling the SAME public pmcp_workbook_compiler verbs each handler shells over — hermetic, fast, non-flaky"
    - "Honest-boundary narration: a provenance refusal narrated is a SUCCESSFUL demonstration (exit 0), never a faked write"
    - "Quarantine slow/recursive make-shell behind #[ignore]; assert the same invariant fast via cargo tree in default-run tests (concern E)"

key-files:
  created:
    - "cargo-pmcp/examples/workbook_cli_demo.rs"
  modified:
    - "cargo-pmcp/tests/workbook_cli_integration.rs (Tasks 1+2, prior commits)"

key-decisions:
  - "Example calls compiler library verbs, NOT the bin-only handlers (handlers absent from cargo-pmcp's lib; examples have no dev-deps) — documented deviation"
  - "The seed-lane compile/emit E2E asserts the fixture's ACTUAL production outcome — a clean typed version-provenance REFUSAL (tax-calc.xlsx declares no `version` named range), not a written bundle"
  - "The emit (false, true) hash-covered marker contract is asserted directly against the 94-00 write_gate_marker/read_gate_marker channel (fixture-independent, the exact channel the handler stamps)"

patterns-established:
  - "Example-via-library-verbs for bin-only CLI handlers"
  - "Fast cargo-tree purity assertions + #[ignore]-d make-purity-check guard"

requirements-completed: [WBCL-01, WBCL-02, WBCL-03, WBCL-04]

# Metrics
duration: ~20min
completed: 2026-06-13
---

# Phase 94 Plan 05: CLI Integration, Purity Guard & Runnable Example Summary

**End-to-end `cargo pmcp workbook` integration tests over the real tax-calc.xlsx fixture (asserting the honest version-provenance refusal + the hash-covered ungated marker), a fast-plus-#[ignore]d purity boundary guard proving umya stays out of every served tree, and a runnable BA-lifecycle example that demonstrates lint/compile/emit via the compiler library verbs.**

## Performance

- **Duration:** ~20 min (this resumed session; Tasks 1+2 committed in a prior session)
- **Tasks:** 3 (Tasks 1 & 2 pre-committed; Task 3 implemented this session)
- **Files modified:** 2 (1 created this session, 1 from prior tasks)

## Accomplishments

- **Task 3 (this session): runnable example** `cargo-pmcp/examples/workbook_cli_demo.rs` walks the BA lifecycle (author → first build → fix → build → emit) over the reused Phase-93 fixture, resolved via `CARGO_MANIFEST_DIR`. It exercises LINT (clean, via ingest + dialect linter), COMPILE seed lane (narrating the honest version-provenance refusal — the correctness boundary working), and EMIT (UNGATED banner + the hash-covered `gated: false` marker via `write_gate_marker`/`read_gate_marker` round-tripping to `(false, true)`). Exits 0; each step labeled with the `cargo pmcp workbook <verb>` it maps to.
- **Task 1 (prior commit `6275ea65`): purity boundary confirmed** — fast default-run `cargo tree` assertions prove `pmcp-workbook-runtime` and `pmcp-workbook-dialect` (the served `PURITY_CRATES`) contain NO `umya`/`quick-xml`, the cargo-pmcp → compiler edge is non-vacuous, and umya IS present in cargo-pmcp's offline tree; a `#[ignore]`-d `make purity-check` guard documents the slow recursive shell (concern E). No Makefile edit was required.
- **Task 2 (prior commit `dbf95f4f`): end-to-end CLI integration** — drives the real built `cargo-pmcp` binary (via `assert_cmd::Command::cargo_bin`) over lint (clean + `--format json` parseable), the seed-lane compile asserting the fixture's actual refusal (no bundle minted), compile-without-`--approver` clap rejection (D-06), the emit refusal + the hash-covered tamper-evident marker contract (D-08/T-94-04-UNGATED), and compile-all over a two-entry `pmcp.toml` with continue-on-error (WBCL-04/D-05).

## Task Commits

1. **Task 1: Purity gate confirmation (explicit step + #[ignore] guard)** — `6275ea65` (test) — *prior session*
2. **Task 2: End-to-end CLI integration (lint/compile/emit + compile-all)** — `dbf95f4f` (test, TDD) — *prior session*
3. **Task 3: Runnable workbook CLI demo example** — `42b962c2` (feat) — *this session*

## Files Created/Modified

- `cargo-pmcp/examples/workbook_cli_demo.rs` (created) — runnable BA-lifecycle demo of lint/compile/emit over the reused fixture, via the compiler library verbs.
- `cargo-pmcp/tests/workbook_cli_integration.rs` (Tasks 1+2, prior commits) — end-to-end CLI integration + purity guard.

## Decisions Made

- **Example calls the compiler library verbs, not the bin-only handlers.** See the deviation below — this is the load-bearing approach for Task 3 and is the documented variance from the plan's "MAY call the handler `execute` functions directly" suggestion.
- **Asserted the fixture's ACTUAL outcome.** `tax-calc.xlsx` declares no `version` named range, so the seed-lane `read_workbook_version` refuses it cleanly (exit 1, no bundle written). The integration test and the example both narrate this refusal as the boundary working (D-02/D-11), rather than fabricating an accepted-fixture path.

## Deviations from Plan

### Deviations

**1. [Rule 3 - Blocking constraint] Example demonstrates the flow via the public COMPILER library verbs, NOT the bin-only CLI handlers**
- **Found during:** Task 3 (runnable example)
- **Issue:** The plan's `<action>` suggested the example "MAY call the handler `execute` functions directly (constructing the `Args` structs) OR shell the built binary — direct handler calls keep it hermetic and are preferred." Neither is actually reachable from an example:
  - `cargo-pmcp/src/lib.rs` deliberately excludes `commands::*`, so the handlers (`commands::workbook::{compile,lint,emit}::execute`) and `PmcpToml` are **bin-only** — an example links the `cargo_pmcp` LIBRARY and cannot see them.
  - Examples do not get dev-dependencies, so `assert_cmd`/`tempfile` (the Task-2 spawn path) are unavailable to an example.
  - Shelling `cargo run -p cargo-pmcp -- workbook ...` from inside `cargo run --example` risks a build-lock deadlock and the bin may be unbuilt at example-run time.
- **Fix:** The example calls the SAME public `pmcp_workbook_compiler` verbs each handler shells over — `ingest::ingest` + `WorkbookCellSource` + `dialect::linter::lint` (mirrors `lint`), `read_workbook_version` (the exact provenance step `compile` runs before `compile_workbook`), and `write_gate_marker`/`read_gate_marker` (the exact ungated-marker channel `emit` stamps) — labeling each step with the `cargo pmcp workbook <verb>` it maps to. This is hermetic, fast, non-flaky, and honest.
- **Files modified:** `cargo-pmcp/examples/workbook_cli_demo.rs`
- **Verification:** `cargo run -p cargo-pmcp --example workbook_cli_demo` exits 0; `cargo clippy -p cargo-pmcp --examples` zero warnings; `cargo fmt --all -- --check` clean; zero SATD tokens.
- **Committed in:** `42b962c2` (Task 3 commit)

---

**Total deviations:** 1 (Rule 3 — a structural constraint of the cargo-pmcp lib/bin split that the plan's preferred approach could not satisfy).
**Impact on plan:** The deviation is purely the *mechanism* of the example; the demonstrated surface (lint/compile/emit over the reused fixture, narrating the honest outcome) is exactly what the plan and acceptance criteria require. No scope change.

## Known Residual Risk

- **Gate-BLOCK E2E is not constructible from the available fixtures.** A real over-tolerance second version of a workbook (the input that drives the governance gate to BLOCK and emit the copy-pasteable `--accept` command) cannot be built from the single Phase-93 `tax-calc.xlsx` fixture — there is no two-version / gate-block fixture, and hand-authoring a genuine Excel workbook is out of scope (it would need a real Excel save). The gate-before-write block path, the tamper-evident marker, and the `GateBlock::render` accept-command surface are **unit-covered in Plans 94-00/94-03/94-04**. A future two-version fixture (a genuine workbook saved at v1.0.0 then v1.1.0 with an over-tolerance output delta) would close this E2E gap. This is recorded here (the durable, non-SATD location) rather than as a TODO in any test file — the integration test contains zero TODO/STUB/FIXME tokens.
- **Accepted-fixture compile/emit happy path is not E2E-exercised.** Because `tax-calc.xlsx` declares no `version` named range, the production seed lane refuses it before any bundle is written, so the seven-member-bundle WRITE and the on-disk `read_gate_marker(bundle_dir) == (false, true)` over a real emitted bundle are not driven end-to-end from the CLI. The bundle-write behavior is unit-covered in Plans 94-03/94-04, and the marker `(false, true)` contract is asserted directly against the 94-00 library channel in the integration test. A genuine versioned fixture would lift these to full E2E.

## Issues Encountered

- `cargo fmt` reflowed the example's long `use` list and `println!` args after the first write; re-ran `cargo fmt --all` and confirmed `--check` clean. No logic change.
- Per `<environment_note>`: `make quality-gate`'s `cargo fuzz` ASAN/sancov step fails on this host (pre-existing nightly-sanitizer toolchain limitation, non-fatal). Task 3 was validated via the targeted `cargo run --example`, `cargo clippy -p cargo-pmcp --examples`, and `cargo fmt --all -- --check` (all pass); Tasks 1/2 already gated in their prior session.

## Next Phase Readiness

- Phase 94 closes its CLAUDE.md ALWAYS requirements: end-to-end integration coverage (Task 2), the purity-boundary confirmation after the new dependency edge (Task 1), and a runnable example (Task 3).
- The accepted-fixture happy-path E2E and the gate-block E2E both await a genuine versioned (and a two-version) workbook fixture — a candidate work item for a future phase that can author real Excel inputs.

## Self-Check: PASSED

- `cargo-pmcp/examples/workbook_cli_demo.rs` — FOUND
- `.planning/phases/94-cli-subcommands-pmcp-toml/94-05-SUMMARY.md` — FOUND
- Commit `6275ea65` (Task 1) — FOUND
- Commit `dbf95f4f` (Task 2) — FOUND
- Commit `42b962c2` (Task 3) — FOUND

---
*Phase: 94-cli-subcommands-pmcp-toml*
*Completed: 2026-06-13*
