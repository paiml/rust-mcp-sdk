---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 02
subsystem: toolkit
tags: [workbook, served-tool, feature-gating, golden-fixture, byte-stability, tamper, integrity, determinism]

# Dependency graph
requires:
  - phase: 92-bundlesource-served-tool-toolkit-module
    plan: 01
    provides: "BundleSource + LocalDirSource + EmbeddedSource, the fail-closed BundleLoader::load (WorkbookBundle/BundleLoadError), the artifact/manifest/changelog/layout Serialize model, build_bundle_lock/update_field/sha256_hex"
provides:
  - "pmcp-server-toolkit workbook/workbook-embedded feature pair + gated #[cfg(feature=\"workbook\")] pub mod workbook skeleton — wave-3/4 plans now compile under --features workbook before handlers land"
  - "The synthetic tax-calc@1.1.0 golden bundle (7 committed artifacts) — the byte-stable test oracle every WBSV test runs against, with a real 1.0.0->1.1.0 changelog"
  - "An in-repo deterministic generator (generate_tax_calc_bundle) producing the golden via the runtime's own Serialize types + build_bundle_lock"
  - "A byte-stability CI check (regenerate-then-diff) + a golden-passes-boot-integrity check + copy-then-corrupt tamper helpers provoking the four fail-closed BundleLoadError variants"
affects: [92-03-provstamp, 92-04-served-handlers, 92-05-builder-ext, workbook-server-scaffold, phase-93-compiler]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Feature PAIR (workbook / workbook-embedded) keeps include_dir out of the LocalDirSource-only build"
    - "Gated module skeleton with submodule decls COMMENTED until the creating plan uncomments each (never pub mod before the file exists)"
    - "Golden built via the runtime's OWN build_bundle_lock so the committed lock == the loader's recompute byte-for-byte"
    - "Determinism via BTreeMap (sorted keys) + a single to_string_pretty config — regeneration is byte-reproducible (CI-enforced)"
    - "Copy-then-corrupt tamper-at-test-time (one committed golden, corruption in a tempdir copy) — no committed corrupt fixtures"

key-files:
  created:
    - crates/pmcp-server-toolkit/src/workbook/mod.rs
    - crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
    - crates/pmcp-server-toolkit/tests/support/tamper.rs
    - crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/manifest.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/executable.ir.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/cell_map.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/layout.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/changelog.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/parser_equivalence.json
  modified:
    - crates/pmcp-server-toolkit/Cargo.toml
    - crates/pmcp-server-toolkit/src/lib.rs
    - crates/pmcp-server-toolkit/tests/support/mod.rs

key-decisions:
  - "base64 stays a TOOLKIT optional dep (Codex MEDIUM #6) — the workbook feature pulls it, the runtime stays base64-free"
  - "The workbook/mod.rs skeleton exposes NOTHING — submodule decls (error/schema/input/handler) stay COMMENTED until Plan 03 creates each file (Codex HIGH #2)"
  - "Added sha2 + hex as dev-deps (already workspace deps the runtime uses — no new package to vet) so the generator can fold the evidence hash via the runtime's own update_field"
  - "flip_byte mutates an ASCII byte to a different ASCII byte (not a high-bit flip) so the integrity gate fires — a non-UTF8 flip would hit the loader's str::from_utf8 Parse check first"
  - "manifest.workflow == bundle_id == 'tax-calc' and changelog.to_version == version == '1.1.0' and layout.source_workbook_hash == lock.workbook_hash, so the golden passes the loader's stamp-binding gate"

patterns-established:
  - "Workbook served-tool home is a feature-gated toolkit module landed EARLY (skeleton-first) so downstream waves compile against it"
  - "A synthetic, customer-free golden bundle is the frozen contract Phase 93's compiler will re-emit and diff against"

requirements-completed: [WBSV-08, WBSV-09]

# Metrics
duration: ~50min
completed: 2026-06-10
---

# Phase 92 Plan 02: Early toolkit workbook feature + synthetic tax-calc golden Summary

**The toolkit gains an opt-in workbook/workbook-embedded feature pair and a gated, empty `pub mod workbook` skeleton (so wave-3/4 plans compile under `--features workbook` before handlers land), plus a deterministic in-repo generator that emits the full-surface synthetic `tax-calc@1.1.0` golden bundle (3 inputs incl. a closed-enum, governed 2-bracket rate table, 4 named outputs with no headline, bracket-boundary annotations, a real 1.0.0→1.1.0 changelog) — committed as 7 byte-stable artifacts that regenerate identically (CI-enforced), pass their own fail-closed boot-integrity gate, and feed copy-then-corrupt tamper helpers provoking the four distinct `BundleLoadError` variants.**

## Performance
- **Duration:** ~50 min
- **Completed:** 2026-06-10
- **Tasks:** 4 (all `type="auto"`, no checkpoints)
- **Files:** 14 (11 created — incl. 7 golden artifacts, 3 modified)

## Accomplishments
- **Task 0 (Codex HIGH #1):** Added the optional `pmcp-workbook-runtime` dep + the D-06 feature pair: `workbook = ["dep:pmcp-workbook-runtime", "dep:base64"]` (LocalDirSource only, no include_dir) and `workbook-embedded = ["workbook", "pmcp-workbook-runtime/embedded"]`. `default = ["code-mode"]` unchanged (D-10). Added `#[cfg(feature="workbook")] pub mod workbook;` and a minimal skeleton whose submodule decls stay COMMENTED (Codex HIGH #2). Builds under `--features workbook`, `--features workbook-embedded`, and no-features.
- **Task 1:** `generate_tax_calc_bundle(out_dir)` builds the 7 members via the runtime's own Serialize types and `build_bundle_lock` (so the committed lock byte-reproduces the loader's recompute). Synthetic progressive-tax workbook @1.1.0: `gross_income`/`deductions` (numeric, USD, Variable) + `filing_status` (closed-enum: single/married_joint/head_of_household); a governed 2-bracket rate table; 4 named outputs (`taxable_income`/`tax_owed`/`effective_rate`/`marginal_rate`) with no headline (WBSV-01); 2 bracket-boundary `AnnotationDecl`s (D-18); a layout descriptor; and a real `1.0.0→1.1.0` `VersionChangelog`. Deterministic: `BTreeMap` IR + a single `to_string_pretty` config; the evidence fold byte-reproduces the loader (Pitfall 2, Codex MEDIUM #8). Uses `json_key` (not `plot3_json_key`); zero customer identifiers (S-4).
- **Task 2:** Committed the 7 golden artifacts as the D-03 contract. `golden_regeneration_is_byte_identical` regenerates into a tempdir and asserts each member is byte-for-byte equal (the CI mechanism enforcing D-03 / T-92-06); `golden_passes_boot_integrity` runs `LocalDirSource + load_bundle` and asserts Ok with 4 outputs / 3 inputs + annotations (WBSV-08 / T-92-05). A `#[ignore]` `regenerate_committed_golden` refreshes the golden on demand.
- **Task 3:** `tests/support/tamper.rs` copy-then-corrupt helpers (D-05 — no committed corrupt fixtures): `copy_golden_to_temp` + `flip_byte` / `delete_artifact` / `desync_lock_version` / `add_unexpected_member`. 4 smoke tests assert each provokes a DISTINCT fail-closed error: `IntegrityMismatch` (T-92-01), missing-member load error, `StampMismatch` on version (T-92-02), `UnexpectedMember` (T-92-22 / Codex MEDIUM #9).

## Task Commits
1. **Task 0: early workbook feature + gated module skeleton** — `58b1cff9` (feat).
2. **Task 1: synthetic tax-calc fixture generator** — `b2ef1603` (feat).
3. **Task 2: commit golden + byte-stability/boot tests** — `9df44d19` (feat).
4. **Task 3: tamper helpers for the negative paths** — `e1ac1fde` (feat).

## Files Created/Modified
- `crates/pmcp-server-toolkit/Cargo.toml` — workbook/workbook-embedded features + optional runtime dep; dev-deps `pmcp-workbook-runtime` (path, embedded off) + `tempfile` + `sha2`/`hex` (modified).
- `crates/pmcp-server-toolkit/src/lib.rs` — `#[cfg(feature="workbook")] pub mod workbook;` (modified).
- `crates/pmcp-server-toolkit/src/workbook/mod.rs` — empty gated skeleton, submodule decls commented (created).
- `crates/pmcp-server-toolkit/tests/support/mod.rs` — workbook-gated `pub mod fixture_gen;` + `pub mod tamper;` (modified).
- `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs` — `generate_tax_calc_bundle` + deterministic builders (created).
- `crates/pmcp-server-toolkit/tests/support/tamper.rs` — copy-then-corrupt helpers (created).
- `crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs` — regeneration / boot / 4 tamper tests (created).
- `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/{manifest,executable.ir,cell_map,layout}.json, BUNDLE.lock, evidence/{changelog,parser_equivalence}.json` — the 7 golden artifacts (created).

## Decisions Made
- **base64 toolkit-side (Codex MEDIUM #6):** the `workbook` feature pulls base64; the runtime stays base64-free.
- **Skeleton exposes nothing (Codex HIGH #2):** `error`/`schema`/`input`/`handler` decls stay commented until Plan 03 creates each file.
- **sha2/hex dev-deps (not a re-implementation):** the generator folds the evidence hash via the runtime's own `update_field`, which needs a `Sha256` instance + `hex::encode`; both are already workspace deps the runtime uses (no new package to vet).
- **flip_byte stays ASCII→ASCII:** mutating an ASCII alphanumeric byte to a different ASCII byte keeps the bytes valid UTF-8 so the integrity gate fires (a high-bit flip would hit the loader's `str::from_utf8` and surface as `Parse` first — see Deviation 1).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] flip_byte high-bit flip produced Parse instead of IntegrityMismatch**
- **Found during:** Task 3 (tamper smoke test)
- **Issue:** The first `flip_byte` implementation flipped the high bit of byte 0 (`{` → 0xFB), producing invalid UTF-8. The loader runs `std::str::from_utf8` on `manifest.json`/`executable.ir.json` BEFORE the integrity recompute, so the corrupted member surfaced as `BundleLoadError::Parse`, not `IntegrityMismatch` — the smoke test failed.
- **Fix:** Rewrote `flip_byte` to find the first ASCII alphanumeric byte and bump it to a DIFFERENT ASCII alphanumeric value (keeps the bytes valid UTF-8 + present, so the integrity gate is what fires).
- **Files modified:** crates/pmcp-server-toolkit/tests/support/tamper.rs
- **Verification:** `tamper_flip_byte_provokes_integrity_mismatch` passes; all 6 active byte-stability tests green.
- **Committed in:** e1ac1fde (Task 3 commit)

**2. [Rule 3 - Blocking] Added sha2 + hex dev-deps for the evidence-hash fold**
- **Found during:** Task 1 (generator evidence fold)
- **Issue:** The generator must fold the evidence-dir hash the EXACT way the loader does (path+length-prefixed via the runtime's `update_field`), which needs a `Sha256` instance + `hex::encode`. Neither was a direct dep of the toolkit. (NOT a package-install gate — both are existing workspace deps the runtime already uses; no new package to vet.)
- **Fix:** Added `sha2 = "0.11"` + `hex = "0.4"` to the toolkit `[dev-dependencies]` (pinned to the runtime's versions).
- **Files modified:** crates/pmcp-server-toolkit/Cargo.toml
- **Verification:** `golden_passes_boot_integrity` confirms the generator's evidence fold matches the loader's recompute (load returns Ok).
- **Committed in:** b2ef1603 (Task 1 commit)

**3. [Rule 1 - Doc] Reworded fixture_gen doc-comments to keep the S-4 / determinism grep gates literally green**
- **Found during:** Task 1 (acceptance grep checks)
- **Issue:** The module doc-comment explaining the scrub/determinism discipline literally listed the forbidden tokens (`ufh`/`quote`/`coil`/`plot3`) and the word `HashMap`, tripping the acceptance greps (`grep HashMap ... = 0`, `grep -Ei "ufh|quote|coil|plot.?3" tests/support/ = 0`) even though no artifact-body construction used them.
- **Fix:** Reworded the doc-comments to describe the discipline without the literal forbidden tokens / bare `HashMap`.
- **Files modified:** crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
- **Verification:** `grep HashMap fixture_gen.rs` = 0; `grep -Ei "ufh|quote|coil|plot.?3" tests/support/` = 0.
- **Committed in:** b2ef1603 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (1 bug, 1 blocking, 1 doc)
**Impact on plan:** No scope change. All three were necessary to satisfy the plan's own acceptance criteria (the integrity-mismatch tamper variant, the deterministic evidence fold, and the literal grep gates).

## Issues Encountered
- The pre-existing `crates/pmcp-server-toolkit/src/code_mode.rs:557` `unused import` warning surfaces whenever the default `code-mode` feature is active (it is present at HEAD, in a file this plan never touches). Logged to `deferred-items.md`; out of scope per the executor scope boundary. My own new code is clippy-clean (verified via `cargo clippy --no-default-features --features workbook --tests`, which excludes the default `code-mode` file).

## Threat Flags
None — all new surface (the golden bytes → loader, the generator's determinism, the tamper paths) is enumerated in the plan's `<threat_model>` (T-92-05/06/07/08) and mitigated: the golden lock is built by the runtime's own `build_bundle_lock` and accepted by `load_bundle` (T-92-05); regeneration is byte-stable via BTreeMap + a fixed serde config (T-92-06); zero customer identifiers in the fixture/generator/support (T-92-07); tamper-at-test-time with one committed golden (T-92-08).

## Known Stubs
- `crates/pmcp-server-toolkit/src/workbook/mod.rs` is an INTENTIONAL empty skeleton — its submodule declarations (`error`/`schema`/`input`/`handler`) stay commented until Plan 03 creates each file (Codex HIGH #2). This is the planned wave-2 deliverable (land the feature gate early so wave-3/4 compile under `--features workbook`); it is documented in the module doc-comment and is NOT a data/UI stub.

## Next Phase Readiness
- Wave-3/4 plans (03 ProvStamp, 04 served handlers, 05 builder-ext) can now `cargo test -p pmcp-server-toolkit --features workbook` and uncomment/add the `error`/`schema`/`input`/`handler` submodules as they land them.
- The `tax-calc@1.1.0` golden is the frozen contract: Phase 93's compiler re-emits the same workbook and diffs against these 7 committed artifacts; the byte-stability check guards against accidental drift.
- The tamper helpers give the wave-3/4 handler tests ready-made fail-closed negative paths (IntegrityMismatch / StampMismatch / missing-member / UnexpectedMember).

## Self-Check: PASSED
- FOUND: crates/pmcp-server-toolkit/src/workbook/mod.rs
- FOUND: crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
- FOUND: crates/pmcp-server-toolkit/tests/support/tamper.rs
- FOUND: crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
- FOUND: crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock (+ 6 other golden members)
- FOUND commit: 58b1cff9 (Task 0)
- FOUND commit: b2ef1603 (Task 1)
- FOUND commit: 9df44d19 (Task 2)
- FOUND commit: e1ac1fde (Task 3)

---
*Phase: 92-bundlesource-served-tool-toolkit-module*
*Completed: 2026-06-10*
