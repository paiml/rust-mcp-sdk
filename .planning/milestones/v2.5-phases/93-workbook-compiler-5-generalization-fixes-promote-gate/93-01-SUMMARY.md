---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 01
subsystem: workbook-compiler
tags: [crate-skeleton, purity-gate, re-export-surface, reader-isolation]
requires:
  - pmcp-workbook-runtime (shared model/IR/hash/changelog/finding/rounding types)
  - pmcp-workbook-dialect (WHITELIST/DialectRules/CandidateRole contract)
provides:
  - pmcp-workbook-compiler crate skeleton (compiling, stub-bodied)
  - CompileError typed error surface (incl. NotImplemented sentinel)
  - compile_workbook generic driver signature
  - Makefile purity-check Phase 93 extension (reader-present positive + single-version guard)
affects:
  - Cargo.toml (workspace members)
  - Makefile (purity-check)
tech-stack:
  added:
    - umya-spreadsheet 3.0 (the ONE allowed Excel reader, compiler-confined)
  patterns:
    - re-export-don't-re-declare (keystone): all shared types re-exported from runtime/dialect
    - typed-NotImplemented stubs (no todo!()/unimplemented!() under #![deny(clippy::panic)])
    - purity-gate exception: compiler is the reader-bearing crate, NOT in PURITY_CRATES
key-files:
  created:
    - crates/pmcp-workbook-compiler/Cargo.toml
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/src/error.rs
    - crates/pmcp-workbook-compiler/src/{ingest,dialect,manifest,formula,dag,sheet_ir,reconcile,provenance,artifact,change_class,gate}/mod.rs
    - crates/pmcp-workbook-compiler/src/stage1.rs
  modified:
    - Cargo.toml
    - Makefile
decisions:
  - "quick-xml/zip kept transitive-only (no direct pins) — single quick-xml v0.37.5 resolved; zip is dual-version by design (writer zip7 + reader zip8)"
  - "provenance::gate is pub(crate) with #[allow(dead_code)] — raw reader never re-exported (purity boundary), no Wave 1 caller yet"
metrics:
  duration: ~12 min
  completed: 2026-06-11
  tasks: 3
  files: 17
---

# Phase 93 Plan 01: pmcp-workbook-compiler Crate Skeleton Summary

Stood up the `pmcp-workbook-compiler` crate skeleton — a compiling, stub-bodied
offline Excel→MCP compiler that re-exports the runtime/dialect shared types
(never re-declares them), confines the `umya-spreadsheet` reader to this one
crate, returns a typed `CompileError::NotImplemented` from every stub (no
panicking macros under the crate's `#![deny(clippy::panic)]` posture), and is
guarded by an extended `make purity-check` that positively asserts the reader
lives HERE and nowhere served.

## What Was Built

**Task 1 — reader-dep legitimacy gate (human-approved):** The three [ASSUMED]
reader packages (`umya-spreadsheet`, `quick-xml`, `zip`) were presented to the
human for crates.io legitimacy verification per the blocking-human
package-legitimacy protocol (T-93-01-SC). The orchestrator confirmed the human
EXPLICITLY APPROVED all three ("Approved — install all three") before any
install. Treated as satisfied; no re-prompt.

**Task 2 — crate manifest + workspace registration (commit `e872145d`):**
Created `crates/pmcp-workbook-compiler/Cargo.toml` mirroring the dialect crate's
package/lib/exclude/docs.rs shape; path+version deps on `pmcp-workbook-runtime`
and `pmcp-workbook-dialect`; pins matched to the runtime (`thiserror = "2"` —
NOT lighthouse's `1` — serde/serde_json `1`, schemars `1.0`, sha2 `0.11`, hex
`0.4`, chrono `0.4`). Added `umya-spreadsheet = "3.0"` as the SOLE direct reader
dep and registered the crate in the root workspace members. Pin derivation done
in order (add umya → resolve → inspect `-i umya-spreadsheet`/`-i quick-xml`/`-i
zip`). No SWC / code-mode dep.

**Task 3 — error.rs + lib.rs + module stubs + purity gate (commit `05273562`):**
- `error.rs`: `CompileError` (thiserror, `#[non_exhaustive]`) with the typed
  `NotImplemented(&'static str)` stub sentinel plus per-stage variants
  (`Io`/`ReadProvenance`/`Ingest`/`Lint`/`Reconcile`/`Emit`/`Gate`) for downstream
  plans, and a `From<std::io::Error>`.
- `lib.rs`: crate-deny header copied verbatim from the runtime; the full module
  tree declared; the keystone re-export surface (`Manifest`/`ChangeClass`/
  `WHITELIST`/`Expr`/`Dag`/`BinOp`/`UnOp`/`CellValue`/`ExcelError`/`toposort`/
  findings/`build_bundle_lock`/`fold_evidence_hash`/`sha256_hex`/`BundleLock`/
  rounding helpers + `DialectRules`/`CandidateRole`) all re-exported from
  `pmcp_workbook_runtime` / `pmcp_workbook_dialect`, never re-declared; the
  `changelog::Severity` (module-path-only) vs `finding::Severity` (bare)
  collision rule preserved.
- The generic `compile_workbook(workbook_path, out_root, approver) ->
  Result<BundleLock, CompileError>` driver stub returning
  `CompileError::NotImplemented("compile_workbook")` — with `build_reference_manifest`
  and the hardcoded reference-workbook-path / workflow-name consts deliberately
  ABSENT (the one surviving §5 gap, WBCO-02).
- 13 compilable module stub files (ingest, dialect, manifest, formula, dag,
  sheet_ir, reconcile, provenance, artifact, change_class, gate + stage1), each
  with a doc-comment and a typed `NotImplemented` stub fn. No `todo!()`/
  `unimplemented!()` anywhere.
- `Makefile` purity-check extended with the Phase 93 block: a POSITIVE
  `cargo tree -p pmcp-workbook-compiler -i umya-spreadsheet` assertion (the
  reader IS here, full package name), a single-version guard (exactly one
  quick-xml; zip bounded to the two legitimate sources), and the served-crate
  negatives re-confirmed via the existing PURITY_CRATES loop. The compiler is the
  EXCEPTION — deliberately NOT appended to PURITY_CRATES.

## Verification

- `cargo build -p pmcp-workbook-compiler` — exits 0, ZERO warnings.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` — no issues found
  (crate-deny header active).
- `cargo fmt -p pmcp-workbook-compiler -- --check` — clean.
- `make purity-check` — PASSED: Phase 91/92 layers unchanged + new Phase 93
  positive `umya-spreadsheet` assertion + single quick-xml + zip bounded to 2
  (writer zip7 + reader zip8) + cargo-deny bans clean.
- Acceptance greps: `NotImplemented` present in error.rs; NO
  `todo!()`/`unimplemented!()`; crate-deny header present; re-exports from both
  runtime AND dialect; NO re-declared `Manifest`/`ChangeClass`; NO
  `build_reference_manifest` on any path; ZERO customer identifiers.
- Reader pins: Cargo.lock holds exactly ONE `quick-xml` (v0.37.5).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Dead-code warning on `provenance::gate`**
- **Found during:** Task 3 (first build)
- **Issue:** `provenance::gate` is intentionally `pub(crate)` (the quick-xml/zip
  raw reader is never re-exported — purity boundary) and has no in-crate caller
  in Wave 1, producing a `dead_code` warning that would fail the zero-warning
  quality gate (CLAUDE.md Toyota Way).
- **Fix:** Added a `// Why:`-annotated `#[allow(dead_code)]` on the stub; Plan 02
  wires `compile_workbook` to call it.
- **Files modified:** crates/pmcp-workbook-compiler/src/provenance/mod.rs
- **Commit:** 05273562

**2. [Rule 3 - Blocking] Doc-comment token collisions with acceptance greps**
- **Found during:** Task 3 (acceptance verification)
- **Issue:** Doc-comments in error.rs and lib.rs literally contained the strings
  `todo!()`/`unimplemented!()` (explaining what the crate does NOT use) and
  `build_reference_manifest` (explaining the removed §5 gap), which tripped the
  forbidding acceptance greps even though no actual macro call or symbol exists.
- **Fix:** Rephrased the doc-comments to describe the concepts ("the panicking
  placeholder stub macros", "the lighthouse's hardcoded reference-manifest
  builder") without the exact forbidden tokens. Intent of both gates (no real
  panic stubs, no real reference-manifest symbol) is satisfied.
- **Files modified:** crates/pmcp-workbook-compiler/src/error.rs,
  crates/pmcp-workbook-compiler/src/lib.rs
- **Commit:** 05273562

## Design Notes

**zip is dual-version by design (not a fork).** The workspace legitimately holds
two zip majors: `zip v7.2.0` via the served runtime's writer-only
`rust_xlsxwriter`, and `zip v8.6.0` via the new `umya-spreadsheet` reader. These
are distinct, semver-incompatible sources (writer vs reader), so the single-version
guard is scoped to "no forked THIRD copy" (bounded to 2) rather than a
workspace-wide dedupe that is impossible. quick-xml IS single-version (v0.37.5).
No direct quick-xml/zip pins were added (Step 2c choice: rely on umya's transitive
copies); Plan 02's provenance reader may add exact-version direct pins if it needs
them as direct deps.

## Authentication Gates

None.

## Known Stubs

This entire crate is intentionally stub-bodied — it is the Wave 1 foundation.
Every module entry fn and `compile_workbook` return
`CompileError::NotImplemented`; downstream plans (02–07) fill the bodies. This is
by design per the plan objective ("A compiling (stub-bodied) compiler crate,
workspace-registered, purity-gated"), not unintentional debt. No data-flow stubs
reach a UI; the crate is build-time only and not yet wired into the CLI.

## Self-Check: PASSED

All created files verified present on disk; both task commits verified in
git log (e872145d, 05273562).
