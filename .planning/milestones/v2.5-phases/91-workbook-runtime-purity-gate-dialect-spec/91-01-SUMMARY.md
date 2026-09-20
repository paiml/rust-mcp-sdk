---
phase: 91-workbook-runtime-purity-gate-dialect-spec
plan: 01
subsystem: infra
tags: [workbook, excel, xlsx, ir, executor, rust_xlsxwriter, serde, schemars, scalar-eval]

# Dependency graph
requires:
  - phase: lighthouse (towelrads quote-pricing)
    provides: crates/workbook-runtime lift-source (owned IR/model types, topo executor, scalar_eval, writer-only renderer, finding model)
provides:
  - "pmcp-workbook-runtime — reader-free leaf crate (slot 2a) with owned IR/model types, deterministic Kahn topo executor + per-cell EvalTrace, pure-Rust scalar_eval, writer-only .xlsx renderer, LintFinding model"
  - "LintFinding/Severity/LintReport now round-trip through JSON (D-08 Deserialize)"
  - "A cargo-tree-provable reader-free boundary: zip enters ONLY via rust_xlsxwriter (writer-only); no umya/quick-xml/swc/pmcp-code-mode in the tree"
affects: [Plan 91-02 (dialect crate depends on runtime finding types), Plan 91-03 (purity gate defends this boundary), Phase 92 (BundleSource), Phase 93 (compiler), Phase 95 (Shape A binary)]

# Tech tracking
tech-stack:
  added: [rust_xlsxwriter 0.95 (writer-only, default-features=false)]
  patterns:
    - "SDK literal-version manifest convention (NOT version.workspace / workspace deps)"
    - "Crate-level panic-freedom: #![deny(clippy::unwrap_used, expect_used, panic)] with cfg(test) allow"
    - "Verbatim lift + targeted delta (rename in doc-comments only; D-08 Deserialize add)"

key-files:
  created:
    - crates/pmcp-workbook-runtime/Cargo.toml
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-workbook-runtime/src/finding.rs
    - crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
    - crates/pmcp-workbook-runtime/src/render/mod.rs
  modified:
    - Cargo.toml

key-decisions:
  - "rust_xlsxwriter provenance (T-91-SC) approved by human: sole owner jmcnamara, MIT/Apache, v0.95.0 not yanked"
  - "No pmcp dependency added to runtime (D-09 permits but runtime is functionally pmcp-free)"
  - "thiserror bumped lighthouse 1 -> SDK pin 2 (source-compatible for the simple #[error] form)"
  - "Verbatim lift carried ZERO SATD and no over-cog-25 function — Task 4 audit passed with no remediation, no // Why: annotations needed"

patterns-established:
  - "Two-sided lift: copy lighthouse src tree verbatim via filesystem, then apply only the documented deltas (crate-name doc rename + D-08 Deserialize)"
  - "Writer-only dependency discipline: cargo tree -i zip proves zip enters only via the writer, no reader leak"

requirements-completed: [WBRT-01, WBRT-02, WBRT-03]

# Metrics
duration: 14min
completed: 2026-06-10
---

# Phase 91 Plan 01: Workbook Runtime (reader-free leaf) Summary

**Reader-free pmcp-workbook-runtime leaf crate lifted verbatim from lighthouse — owned IR/model types, deterministic Kahn topo executor with per-cell traces, pure-Rust scalar_eval, writer-only rust_xlsxwriter .xlsx renderer, and a now-round-trippable LintFinding model — registered as workspace slot 2a with a cargo-tree-provable writer-only dependency boundary.**

## Performance

- **Duration:** ~14 min
- **Started:** 2026-06-10 (foreground resume after T-91-SC checkpoint approval)
- **Completed:** 2026-06-10
- **Tasks:** 3 executed (Task 1 checkpoint pre-resolved as approved; Tasks 2-4)
- **Files created/modified:** 21 (20 src files + new manifest) + root Cargo.toml

## Accomplishments
- Created the `pmcp-workbook-runtime` crate manifest in SDK literal-version convention (literal `0.1.0`, `thiserror = "2"`, writer-only `rust_xlsxwriter = { version = "0.95", default-features = false }`, no `pmcp` dep) and registered it as a workspace member.
- Lifted all 20 source files verbatim from the lighthouse runtime (IR/model types, `dag.rs` owned Kahn toposort, `sheet_ir/` executor + semantics + rounding + eval layers, `render/` writer-only renderer, `artifact_model.rs` sha2/hex hashing, `scalar_eval.rs`, `changelog.rs`).
- Applied the single code delta D-08: added `Deserialize` to `Severity`/`LintFinding`/`LintReport` and added the `lint_report_round_trips_through_json` test (LintReport JSON round-trip).
- 128 lib tests pass under `--test-threads=1`; `cargo clippy -p pmcp-workbook-runtime -- -D warnings` is clean (panic-freedom deny lints hold).
- Confirmed the writer-only boundary: `cargo tree -p pmcp-workbook-runtime -i zip` shows `zip` reachable ONLY through `rust_xlsxwriter`; `thiserror` resolves to a single major v2.0.18.
- Toyota-Way audit (Task 4) passed with zero remediation: zero SATD across the lifted tree (`make check-todos` green) and zero `clippy::cognitive_complexity` warnings (no function over 25, none near the cog-50 cap, no `// Why:` annotations required).

## Task Commits

1. **Task 1: Provenance gate for rust_xlsxwriter (T-91-SC)** — checkpoint, resolved "approved" by human before this run (no commit; pure provenance gate)
2. **Task 2: Create runtime crate manifest + register in workspace** - `9a0ba373` (feat)
3. **Task 3: Lift runtime source verbatim + D-08 Deserialize delta** - `1647c1cb` (feat, TDD lift — lifted tests + new round-trip test pass on first run)
4. **Task 4: Toyota-Way audit (zero SATD + cognitive-complexity ≤25)** - no commit (audit passed with no file changes; verbatim lift was already clean)

**Plan metadata:** (this commit — docs: complete plan)

## Files Created/Modified
- `crates/pmcp-workbook-runtime/Cargo.toml` - SDK-convention manifest: literal version 0.1.0, thiserror 2, writer-only rust_xlsxwriter 0.95, no pmcp dep; preserves the rust_xlsxwriter provenance comment block
- `crates/pmcp-workbook-runtime/src/lib.rs` - Crate root: panic-freedom deny lints + full module re-export surface (crate-name rename in doc-comments only)
- `crates/pmcp-workbook-runtime/src/finding.rs` - LintFinding/Severity/LintReport with D-08 Deserialize + round-trip test
- `crates/pmcp-workbook-runtime/src/sheet_ir/` - Kahn topo executor (build_dag/run/EvalTrace/RunResult), semantics (13 whitelisted fns), rounding (round-half-away-from-zero), eval layers
- `crates/pmcp-workbook-runtime/src/render/` - writer-only render_xlsx (fixed creation datetime + empty author for byte-determinism)
- `crates/pmcp-workbook-runtime/src/{dag,resolve,manifest_model,artifact_model,scalar_eval,changelog,formula,excel_error,range_ref}.rs` - owned model/utility types lifted verbatim
- `Cargo.toml` (root) - appended `crates/pmcp-workbook-runtime` to the `[workspace] members` array

## Decisions Made
- **T-91-SC provenance approved (human):** rust_xlsxwriter sole owner `jmcnamara` (author of libxlsxwriter / Python XlsxWriter), repo `github.com/jmcnamara/rust_xlsxwriter`, MIT OR Apache-2.0, v0.95.0 not yanked, 2.3M downloads. Dependency landed in Task 2.
- **No `pmcp` dependency** added (D-09 permits, but the runtime is functionally pmcp-free — adding it would bloat the tree).
- **thiserror 1 → 2** to match the SDK pin; the simple `#[error("...")]` form is source-compatible across the major (RenderError compiles clean).
- **Task 4 found nothing to fix:** the lighthouse code is penny-reconciled / production-proven, so the verbatim lift carried no SATD and no over-complexity — the SDK Toyota-Way ceiling was already met.

## Deviations from Plan

None - plan executed exactly as written. The verbatim lift required no auto-fixes; all lifted tests plus the new D-08 round-trip test passed on the first build, and the Task 4 audit passed with no remediation.

## Issues Encountered
None during this resumed run. (The prior background-agent attempt was blocked by harness permission auto-denials; running in the foreground with interactive permissions resolved that — no code-level issue.)

## Post-Dependency Automated Arms (required by continuation context)
- `cargo audit` (workspace root): no advisory implicates rust_xlsxwriter or its transitive zip. The single allowed warning (RUSTSEC-2026-0097, `unsound` on `rand 0.8.5`) enters via `swc_ecma_parser` (the pmcp-code-mode JS stack) — pre-existing, unrelated to this crate.
- `cargo tree -p pmcp-workbook-runtime -i zip`: `zip v7.2.0` → `rust_xlsxwriter v0.95.0` → `pmcp-workbook-runtime` (writer-only; no reader path). PASS.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Slot 2a (`pmcp-workbook-runtime`) is complete: compiles, 128 tests green, SATD-free, complexity-bounded, registered as a workspace member.
- Plan 91-02 (dialect crate) can now depend on `pmcp-workbook-runtime::finding::{LintFinding, LintReport, Severity}` (D-03).
- Plan 91-03 (purity gate) has a concrete boundary to defend: the writer-only `rust_xlsxwriter` presence + reader/JS absence are already cargo-tree-provable.

## Self-Check: PASSED

- Created files verified on disk: Cargo.toml, src/lib.rs, src/finding.rs, src/sheet_ir/executor.rs, src/render/mod.rs — all FOUND.
- Commits verified in git log: 9a0ba373 (Task 2), 1647c1cb (Task 3) — all FOUND.

---
*Phase: 91-workbook-runtime-purity-gate-dialect-spec*
*Completed: 2026-06-10*
