---
phase: 94-cli-subcommands-pmcp-toml
plan: 04
subsystem: cli
tags: [cargo-pmcp, workbook, emit, ungated, evidence-marker, tamper-evident, colored, clap]

# Dependency graph
requires:
  - phase: 94-00
    provides: "write_gate_marker/read_gate_marker hash-covered evidence channel + read_workbook_version re-exports"
  - phase: 94-01
    provides: "PmcpToml::load/resolve/all_entries + WorkbookEntry"
  - phase: 94-02
    provides: "EXIT_* constants + format_lint_report/print_lint_report/lint_exit_code"
  - phase: 94-03
    provides: "compile.rs sibling — target-resolution + lint-phase reuse shape"
provides:
  - "cargo pmcp workbook emit <wb.xlsx> — ungated dev/reference bundle regeneration (WBCL-03)"
  - "EmitArgs clap struct + execute handler (compile minus the gate)"
  - "hash-covered gated:false evidence marker stamped via the 94-00 write_gate_marker channel"
  - "deterministic UNGATED stderr banner (even under --quiet; json-clean stdout)"
  - "CR-02 @<version> non-overwrite protection against clobbering a promoted baseline"
affects: [94-cli-subcommands-pmcp-toml, workbook-compiler-cli, deploy-pipeline-trust-boundary]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ungated emit = seed-lane compile_workbook + post-write write_gate_marker stamp"
    - "safety banner printed to stderr unconditionally (bypasses --quiet); data/json stays on stdout"

key-files:
  created: []
  modified:
    - "cargo-pmcp/src/commands/workbook/emit.rs (replaced the Plan-02 neutral stub)"

key-decisions:
  - "Lint errors BLOCK emit (promoted from discretion to a decision): a broken sheet must never silently emit a bundle"
  - "Approver recorded as the fixed placeholder \"ungated\" in the seed-lane manifest sign-off (no --approver flag on emit)"
  - "No cli-side loose-marker fallback — only the hash-covered library channel (write_gate_marker)"

patterns-established:
  - "emit-all worst-status-wins reduces over only EXIT_ERROR > EXIT_OK (no gate-block tier on the ungated path)"
  - "the UNGATED banner is a const substring asserted by a pure test, so determinism is testable without stdout capture"

requirements-completed: [WBCL-03, WBCL-04]

# Metrics
duration: ~35min
completed: 2026-06-13
---

# Phase 94 Plan 04: Ungated Workbook Emit Handler Summary

**`cargo pmcp workbook emit` regenerates a workbook bundle WITHOUT the golden-corpus gate for dev/reference, stamping a hash-covered `gated: false` evidence marker via the 94-00 `write_gate_marker` channel and printing a loud deterministic `UNGATED` stderr banner so an unvetted bundle can never masquerade as a promoted one downstream.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-06-13
- **Tasks:** 1 (tdd)
- **Files modified:** 1

## Accomplishments
- Replaced the Plan-02 neutral `emit.rs` stub with `EmitArgs` + the full ungated emit handler (compile.rs minus the gate, D-08).
- Writes the seven-member bundle through the UNGATED seed lane (`compile_workbook`); the handler NEVER invokes the governance gate (`grep -c 'gate::gate' emit.rs` == 0).
- Stamps the HASH-COVERED `gated: false` marker (`evidence/gate.json` + `evidence/gate.sha256`) into the emitted bundle dir via the 94-00 `write_gate_marker` library channel — tamper-evident (`read_gate_marker` returns `(false, true)`; an edited marker flips `digest_ok` to false).
- Prints the loud `UNGATED — not regression-checked, do not deploy` banner DETERMINISTICALLY to stderr (always, even under `--quiet`); stdout stays clean for `--format json`.
- Requires NO `--approver` (dev/reference); CR-02 `@<version>` non-overwrite (reached via `compile_workbook`'s `atomic_promote_dir`) protects a promoted baseline from being clobbered.
- Reuses `PmcpToml` resolution + emit-all (continue-on-error, worst-status-wins) and the Plan-02 lint phase; lint errors block emit.

## Task Commits

Each task was committed atomically:

1. **Task 1: EmitArgs + ungated emit handler (banner + hash-covered marker)** - `dd25b89a` (feat)

_Note: the single TDD task landed the handler + its 11 unit tests in one feat commit (the failing-test RED → passing GREEN cycle converged within one file write since the marker/banner seams were already library-published in 94-00)._

## Files Created/Modified
- `cargo-pmcp/src/commands/workbook/emit.rs` - `EmitArgs` clap struct + `execute`/`emit_one`/`write_ungated_bundle`/`run_lint_phase`/`resolve_targets` + the deterministic `print_ungated_banner` + 11 unit tests.

## Decisions Made
- **Lint errors block emit** — promoted from planning discretion to a firm decision: `run_lint_phase` returns `EXIT_ERROR` on any `Severity::Error`, so a broken sheet never silently emits a bundle.
- **Approver placeholder `"ungated"`** — emit exposes no `--approver` flag (D-08); the seed lane still records an approver in the manifest sign-off, so the handler passes the fixed `"ungated"` literal to make the dev/reference provenance explicit.
- **No loose-marker fallback** — only the hash-covered library channel is used (the weaker cli-side loose-marker approach was dropped in planning).
- **`gate::gate` references scrubbed from comments** — the acceptance criterion is a literal `grep -c 'gate::gate' == 0`; the two doc-comment mentions were rephrased to "invokes the governance gate" so the grep is satisfied while the intent stays documented.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test target is `--bin cargo-pmcp`, not `--lib`**
- **Found during:** Task 1 (verification)
- **Issue:** The plan's `<verify>`/`<acceptance_criteria>` specify `cargo test -p cargo-pmcp --lib workbook::emit`, but `cargo-pmcp`'s `commands::*` is BIN-only — `--lib` resolves no tests (same gotcha verified in Waves 0-2 for compile/lint).
- **Fix:** Ran `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::emit` (11 passed). No code change required.
- **Files modified:** none (test invocation only).
- **Verification:** 11 emit tests pass via the `--bin` target.
- **Committed in:** n/a (verification-only deviation, documented here).

**2. [Rule 1 - Bug] Comment-only `gate::gate` tokens tripped the literal grep gate**
- **Found during:** Task 1 (acceptance-criteria check)
- **Issue:** Two doc-comments contained the literal substring `gate::gate`, which would make `grep -c 'gate::gate' emit.rs` return 2 even though no call exists.
- **Fix:** Rephrased both comments to "invokes the governance gate"; grep now returns 0.
- **Files modified:** `cargo-pmcp/src/commands/workbook/emit.rs`.
- **Verification:** `grep -c 'gate::gate' emit.rs` == 0; intent still documented.
- **Committed in:** `dd25b89a` (Task 1 commit).

---

**Total deviations:** 2 (1 blocking test-target correction, 1 bug-grade comment scrub)
**Impact on plan:** Both are mechanical corrections to satisfy stated acceptance criteria exactly. No scope creep; the handler behavior matches the plan verbatim.

## Issues Encountered
- The `cargo fmt --all -- --check` step in `make quality-gate` flagged a long `write_gate_marker(...)` line; resolved with `cargo fmt --all`. Gate re-ran green afterward (exit 0).
- `make quality-gate` also surfaces pre-existing `cargo fuzz` build errors ("the option `Z` is only accepted on the nightly compiler") — these come from the fuzz-target enumeration in unrelated gate steps, are environmental (nightly toolchain absent), non-fatal (`|| echo` in the recipe), and unrelated to this change. The gate's `make` exit code was 0.

## Acceptance Criteria Verification
- `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::emit` → 11 passed (marker `(false, true)` round-trip + tamper-detection, banner substring, emit-all/bundle-id resolution, no-approver construction, no-clobber baseline preservation).
- `grep -c 'gate::gate' emit.rs` == 0 (never invokes the gate — WBCL-03).
- `grep -c 'write_gate_marker' emit.rs` ≥ 1 (hash-covered library channel).
- `grep -c 'compile_workbook' emit.rs` ≥ 1 (uses the library seed driver).
- `cargo clippy -p cargo-pmcp` → 0 warnings on emit.rs.
- `make quality-gate` → exit 0 (fmt-check, lint "✓ No lint issues", build, test-all).

## Known Stubs
None — the handler is fully wired to the published library seams; no placeholder/empty data paths.

## Next Phase Readiness
- WBCL-03 (ungated dev/reference emit) + D-08 (deterministic banner + tamper-evident `gated: false` marker + CR-02 non-overwrite) are satisfied.
- The `workbook` command group (compile/lint/emit) is now fully implemented across Waves 0-3; the remaining phase work (main.rs wiring, Cargo.toml dep) lives in its own plan(s) per the PATTERNS map.

## Self-Check: PASSED
- FOUND: `cargo-pmcp/src/commands/workbook/emit.rs`
- FOUND: `.planning/phases/94-cli-subcommands-pmcp-toml/94-04-SUMMARY.md`
- FOUND: commit `dd25b89a` (feat(94-04): implement ungated workbook emit handler)

---
*Phase: 94-cli-subcommands-pmcp-toml*
*Completed: 2026-06-13*
