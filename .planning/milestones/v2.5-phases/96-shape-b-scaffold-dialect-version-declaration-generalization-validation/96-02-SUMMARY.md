---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
plan: 02
subsystem: cli
tags: [cargo-pmcp, scaffold, workbook, include_dir, embedded-bundle, shape-b, purity-gate]

# Dependency graph
requires:
  - phase: 92-bundlesource-served-tool-toolkit
    provides: EmbeddedSource + WorkbookBuilderExt::try_with_workbook_bundle (workbook-embedded feature)
  - phase: 95-shape-a-binary-pmcp-workbook-server
    provides: the default-features=false workbook-embedded+http purity posture (T-95-06) + workbook_server_http.rs canonical wiring
  - phase: 94-cli-subcommands-pmcp-toml
    provides: the pmcp.toml [[workbook]] project-config shape
provides:
  - "`cargo pmcp new --kind workbook-server <name>` Shape B scaffold (WBCL-05)"
  - "A purity-safe runnable workbook-server crate (Cargo.toml + EmbeddedSource main.rs + pmcp.toml + source .xlsx + pre-compiled bundle)"
  - "Publish-safe embedded scaffold assets (under the cargo-pmcp package root, via include_dir!/include_bytes!)"
  - "A narrow public lib seam (cargo_pmcp::templates_workbook_server) for the scaffold generator"
affects: [shape-b-docs, pmcp-book, pmcp-course, future workbook scaffold variants]

# Tech tracking
tech-stack:
  added: [include_dir 0.7.4 (cargo-pmcp)]
  patterns:
    - "Embedded-binary-asset template: include_dir!/include_bytes! over an in-package asset dir so scaffold bytes survive cargo publish"
    - "Narrow #[path] lib seam to expose ONE bin-only template module to the lib target (example + integration test)"
    - "LF/CRLF-insensitive emitted-source drift-lock golden test with match-arm normalization"
    - "Hardcoded-dep-version drift guard (emitted pin vs workspace-root Cargo.toml)"

key-files:
  created:
    - cargo-pmcp/src/templates/workbook_server.rs
    - cargo-pmcp/src/templates/workbook_bundle/ (embedded tax-calc@1.1.0 bundle + tax-calc.xlsx)
    - cargo-pmcp/tests/workbook_scaffold.rs
    - cargo-pmcp/examples/workbook_server_scaffold.rs
  modified:
    - cargo-pmcp/src/commands/new.rs
    - cargo-pmcp/src/templates/mod.rs
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/Cargo.toml

key-decisions:
  - "Embed assets under the cargo-pmcp package root (not copy from crates/* at generate-time) so they survive cargo publish"
  - "Mount a SEPARATE narrow lib seam (templates_workbook_server via #[path]) rather than promote the whole templates tree to the lib target"
  - "Reuse the tax-calc@1.1.0 golden (D-07) — no new .xlsx authored in this plan"
  - "Pin pmcp via a PMCP_VERSION const guarded by a workspace-root version-drift test"

patterns-established:
  - "Embedded-binary-asset scaffold template (first in cargo-pmcp; sql_server had text-only)"
  - "Drift-lock golden test that normalizes a match-expression harness branch down to the single load-bearing call"

requirements-completed: [WBCL-05]

# Metrics
duration: 38min
completed: 2026-06-15
---

# Phase 96 Plan 02: Shape B `--kind workbook-server` Scaffold Summary

**`cargo pmcp new --kind workbook-server` emits a runnable, purity-safe (default-features=false, workbook-embedded+http) governed-Excel workbook MCP server crate whose pre-compiled `tax-calc@1.1.0` bundle + source `.xlsx` are embedded under the cargo-pmcp package root via `include_dir!`/`include_bytes!` so they survive `cargo publish`.**

## Performance

- **Duration:** ~38 min
- **Started:** 2026-06-15 (this session)
- **Completed:** 2026-06-15
- **Tasks:** 3
- **Files modified/created:** 15 (incl. the 8-file embedded bundle tree)

## Accomplishments

- Shape B `workbook-server` template module (`workbook_server.rs`) mirroring the `sql_server.rs` orchestrator pattern but introducing the first BINARY-asset handling in cargo-pmcp's templates: emits `Cargo.toml`, `src/main.rs` (EmbeddedSource wiring), `pmcp.toml`, `workbook/tax-calc.xlsx`, and the full `bundle/tax-calc@1.1.0/*` tree.
- Purity-safe emitted `Cargo.toml`: `default-features = false, features = ["workbook-embedded", "http"]`, never `code-mode` (T-95-06). Unit-asserted and verified end-to-end by `make purity-check` (toolkit `workbook-embedded` combo is reader-/code-mode-free).
- Publish-safe assets: the golden bundle + source `.xlsx` are committed UNDER the cargo-pmcp package root (`src/templates/workbook_bundle/`) and embedded via `include_dir!`/`include_bytes!`. A `cargo package --list` smoke proves they ship.
- `--kind workbook-server` dispatch arm + `execute_workbook_server` (validate_crate_name FIRST — T-96-04 path-traversal guard reused, not reimplemented) + next-steps printer.
- Narrow public lib seam (`cargo_pmcp::templates_workbook_server`) mounted via `#[path]` so the runnable example and the integration test reach `generate()` from the lib target (the `templates::*` tree is bin-only).
- Golden tests: emitted-main drift-lock to `workbook_server_http.rs` (LF/CRLF-safe, harness `--bundle-dir`/`LocalDirSource` branch normalized away), embedded-bundle byte-equality to the committed golden, embedded-xlsx byte-equality, full-file-tree emit, and a pmcp version-drift guard.
- ALWAYS requirements satisfied: runnable example (`workbook_server_scaffold`) driving the lib seam, plus the drift-lock / bundle-bytes / scaffold-build / packaging smoke tests.

## Task Commits

1. **Task 1: template module + EMBEDDED bundle/xlsx/pmcp.toml payload + lib seam** — `91933535` (feat)
2. **Task 2: dispatch arm + mod registration (golden tests carried from Task 1)** — `736a1266` (feat)
3. **Task 3: integration test + packaging smoke + runnable example** — `cf670b6b` (test)

**Plan metadata:** (this commit) — docs: complete plan

## Files Created/Modified

- `cargo-pmcp/src/templates/workbook_server.rs` — Shape B emitter + embedded assets + 7 golden/unit tests.
- `cargo-pmcp/src/templates/workbook_bundle/tax-calc@1.1.0/*` + `tax-calc.xlsx` — embedded, publish-safe scaffold assets (copied from the committed golden + compiler fixture).
- `cargo-pmcp/src/commands/new.rs` — `workbook-server` dispatch arm + `execute_workbook_server` + next-steps printer.
- `cargo-pmcp/src/templates/mod.rs` — `pub mod workbook_server;`.
- `cargo-pmcp/src/lib.rs` — `templates_workbook_server` narrow public seam (`#[path]`).
- `cargo-pmcp/Cargo.toml` — `include_dir = "0.7.4"` dependency.
- `cargo-pmcp/tests/workbook_scaffold.rs` — CLI integration test + path-traversal rejection + `#[ignore]` scaffold-build & packaging smokes.
- `cargo-pmcp/examples/workbook_server_scaffold.rs` — runnable example over the lib seam (ALWAYS EXAMPLE).

## Decisions Made

- **Embed, don't copy:** assets are baked into the published cargo-pmcp crate (under the package root) rather than copied from `crates/*` at generate-time, which would break a standalone publish. Verified by the packaging smoke.
- **Separate lib seam, not whole-tree promotion:** mounting only `templates_workbook_server` via `#[path]` keeps the lib surface minimal (the rest of `templates::*` references bin-only `crate::commands::*`).
- **Version-drift guard:** the hardcoded `PMCP_VERSION` is asserted equal to the workspace-root `pmcp` version so the pin cannot silently drift.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `include_dir` dependency to cargo-pmcp**
- **Found during:** Task 1
- **Issue:** cargo-pmcp had no `include_dir` dependency; the embedded-asset emitter needs `include_dir!`. (Not a package-install legitimacy concern — `include_dir` is already a vetted, in-tree dependency used by `pmcp-server-toolkit` at the same 0.7.4 pin.)
- **Fix:** Added `include_dir = "0.7.4"` to `cargo-pmcp/Cargo.toml`.
- **Files modified:** cargo-pmcp/Cargo.toml
- **Verification:** `cargo build -p cargo-pmcp --lib` and full build pass.
- **Committed in:** `91933535` (Task 1 commit)

**2. [Rule 3 - Blocking] Reworked the drift-lock normalizer for the match-expression harness branch**
- **Found during:** Task 1 (first test run)
- **Issue:** The canonical example imports `{EmbeddedSource, LocalDirSource, WorkbookBuilderExt}` and selects the source via a `match bundle_dir { Some(dir) => LocalDirSource..., None => EmbeddedSource... }`. The scaffold has no `--bundle-dir` branch (it always serves its embedded bundle), so a naive line filter could not align the two wirings.
- **Fix:** The `wiring_lines()` normalizer now (a) normalizes the toolkit import (drops the extra `LocalDirSource` token), (b) drops the `--bundle-dir`/`LocalDirSource`/`match` scaffolding lines, and (c) collapses BOTH the example's `None => builder.try_...` match arm and the scaffold's `let builder = builder.try_...` statement to a common `try:builder.try_...` token. Also LF/CRLF-insensitive (Gemini LOW).
- **Files modified:** cargo-pmcp/src/templates/workbook_server.rs
- **Verification:** `emitted_main_matches_example_modulo_setup` passes.
- **Committed in:** `91933535` (Task 1 commit)

**3. [Rule 1 - Bug] Comment-stripping in the purity-safe Cargo.toml assertion**
- **Found during:** Task 1 (first test run)
- **Issue:** The emitted Cargo.toml's explanatory comment legitimately MENTIONS `code-mode` (the purity rationale), tripping the naive `!cargo.contains("code-mode")` negative assertion.
- **Fix:** The test (and the integration test) strip `#` comment lines before the negative check, so only real dependency/feature lines are evaluated.
- **Files modified:** cargo-pmcp/src/templates/workbook_server.rs, cargo-pmcp/tests/workbook_scaffold.rs
- **Verification:** `emitted_cargo_toml_is_purity_safe` passes.
- **Committed in:** `91933535` / `cf670b6b`

**4. [Rule 1 - Bug] Reworded two doc comments to avoid clippy `doc_list_item_without_indentation`**
- **Found during:** Tasks 2 & 3 (clippy)
- **Issue:** Doc-comment continuation lines beginning with `+` were parsed by clippy as markdown list items.
- **Fix:** Reworded the `execute_workbook_server` and `scaffold_crate_cargo_check_compiles` doc comments to prose.
- **Files modified:** cargo-pmcp/src/commands/new.rs, cargo-pmcp/tests/workbook_scaffold.rs
- **Verification:** clippy clean for the changed files (remaining 15 warnings are pre-existing, unrelated pentest/deployment unused-import warnings — out of scope).
- **Committed in:** `736a1266` / `cf670b6b`

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 bug). **Impact:** all necessary for correctness/compilation/quality-gate; no scope creep.

## Issues Encountered

- None beyond the auto-fixed items above. The scaffold-build smoke (`PMCP_RUN_SCAFFOLD_BUILD=1`) and the packaging smoke were both run locally and PASS (see Verification below).

## Verification

- `cargo test -p cargo-pmcp workbook_server` — 14 passed (7 golden/unit tests via both module mounts).
- `cargo test -p cargo-pmcp --test workbook_scaffold` — 2 passed, 2 ignored (default).
- `cargo run -p cargo-pmcp --example workbook_server_scaffold` — OK (prints the generated tree).
- **Scaffold-build smoke (run explicitly):** `PMCP_RUN_SCAFFOLD_BUILD=1 cargo test -p cargo-pmcp --test workbook_scaffold -- --ignored scaffold_crate_cargo_check_compiles --test-threads=1` — PASS; the emitted crate `cargo check`s end-to-end (`Checking scaffold_workbook_demo`).
- **Packaging smoke (run explicitly):** `cargo test -p cargo-pmcp --test workbook_scaffold -- --ignored embedded_assets_appear_in_cargo_package_list` — PASS; the embedded assets appear in `cargo package --list` (publish-safe).
- `make purity-check` — PASS (toolkit `workbook-embedded`+`http` combo reader-/code-mode-free; the posture the scaffold pins).
- `cargo fmt -p cargo-pmcp -- --check` clean; `cargo clippy` clean for all changed files.

### Deferred full-gate note

A repo-wide `make quality-gate` (workspace-wide pedantic+nursery clippy, audit, full test) was NOT run in this plan to keep the executor loop fast; the per-crate fmt/clippy/build/test gates for cargo-pmcp all pass. **Run `make quality-gate` before pushing/PR** (per CLAUDE.md). The two `#[ignore]`d smokes are intentionally not in the default `cargo test` set (slow: they compile the unpublished toolkit tree / run `cargo package`) but are documented and verified-passing above.

## Known Stubs

None — the scaffold emits a complete, compilable, servable crate (the scaffold-build smoke proves it compiles; the toolkit's existing tests prove the served bundle yields the five tools).

## Next Phase Readiness

- WBCL-05 complete. Shape B authoring on-ramp ships alongside the Shape A binary (Phase 95).
- The remaining Phase 96 plans (loan workbook WBEX-01, quirk corpus WBEX-02) are independent of this scaffold; Plan 01 (WBDL-02 dialect-version) is already complete.

## Self-Check: PASSED

- All key files verified present (workbook_server.rs, embedded bundle/.xlsx, integration test, example, SUMMARY).
- All three task commits verified in git history: `91933535`, `736a1266`, `cf670b6b`.

---
*Phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation*
*Completed: 2026-06-15*
