---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 02
subsystem: compiler
tags: [umya, quick-xml, zip, provenance, oracle, fuzz, excel, wbco-07, wbco-01]

# Dependency graph
requires:
  - phase: 93-01
    provides: "pmcp-workbook-compiler crate scaffold + ingest/provenance compilable stubs + curated runtime re-export surface"
  - phase: 91
    provides: "pmcp-workbook-runtime owned model/finding/hash types (LintFinding, Manifest, Role, CellRole, update_field, RangeRef, cell_key)"
provides:
  - "ingest/ — umya read into an owned WorkbookMap/CellRecord model (no umya type leaks); cached <v> values captured as the trusted oracle (WBCO-01)"
  - "MAX_CELL_COUNT ingest DoS guard → oracle/too-many-cells Error finding, fail-closed"
  - "provenance/ — quarantined pub(crate) quick-xml/zip raw-parts reader with ZIP/XML hard limits"
  - "ProvenanceClass model {ExcelTrusted,NonExcel,UmyaFabricated,UnknownStale} REFUSING umya-fabricated identity (WBCO-07)"
  - "TrustedFixtureMarker test-only provenance override that cannot weaken production refusal"
  - "compute_region_hashes — four region hashes reusing the runtime's single update_field canonicalization"
  - "fuzz_provenance_reader cargo-fuzz target over the untrusted .xlsx raw reader"
affects: [93-03-linter, 93-04-manifest-synth, 93-05-formula-dag, 93-06-reconcile, 93-07-artifact-gate]

# Tech tracking
tech-stack:
  added: [quick-xml 0.37 (direct, umya-resolved), zip 8.6 (direct, umya-resolved), libfuzzer-sys 0.4 (fuzz crate)]
  patterns:
    - "Reader quarantine: umya confined to ingest/, quick-xml/zip confined to provenance/ as pub(crate); no foreign type crosses a public signature"
    - "ProvenanceClass enum is the single authoritative provenance verdict; the refuse decision derives from it, never an ad-hoc check"
    - "Positive-marker accept policy: ExcelTrusted requires anchored name AND a positive Excel AppVersion marker (false-positive-safe)"
    - "Test-only override entry (#[cfg(test)] gate_with_fixture_override) that production gate() never reaches"
    - "#[cfg(fuzzing)] pub hook keeps the raw reader pub(crate)-quarantined on every non-fuzz build"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/ingest/cell_map.rs
    - crates/pmcp-workbook-compiler/src/provenance/raw_parts.rs
    - crates/pmcp-workbook-compiler/src/provenance/gate.rs
    - crates/pmcp-workbook-compiler/src/provenance/region_hash.rs
    - crates/pmcp-workbook-compiler/fuzz/Cargo.toml
    - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/fuzz_provenance_reader.rs
  modified:
    - crates/pmcp-workbook-compiler/src/ingest/mod.rs
    - crates/pmcp-workbook-compiler/src/provenance/mod.rs
    - crates/pmcp-workbook-compiler/Cargo.toml

key-decisions:
  - "umya-fabrication signal (O-3): UmyaFabricated = anchored 'Microsoft Excel' name AND no positive Excel marker (absent AppVersion); the sentinel calcId=122211 ALONE never refuses — a present AppVersion always admits (false-positive policy)"
  - "ExcelTrusted requires anchored name AND a positive AppVersion build marker; a real Excel save always carries one, umya carries none"
  - "MAX_CELL_COUNT = 5_000_000 ingest cap; over-limit → oracle/too-many-cells Error finding (Rule 2 — required by plan, absent in lighthouse)"
  - "Added DecompressBomb + XmlTooDeep ProvenanceError variants for the cumulative-decompression and XML-depth hard limits (net-new vs lighthouse)"
  - "OracleProvenance now carries the ProvenanceClass verdict (class field) so the evidence bundle records the authoritative classification"
  - "ProvenanceError marked #[non_exhaustive] so future limit variants are additive"
  - "Replaced all lighthouse-fixture-dependent tests with umya-authored / in-memory-zip fixtures (no committed customer .xlsx)"

patterns-established:
  - "Pattern: provenance verdict as a serialized enum that the refuse path keys on"
  - "Pattern: hard-limit guards return typed oracle/* Error findings, never panic (crate #![deny(panic)] on value paths)"

requirements-completed: [WBCO-01, WBCO-07]

# Metrics
duration: ~40min
completed: 2026-06-11
---

# Phase 93 Plan 02: Ingest + Provenance (umya-isolated reader, ProvenanceClass, WBCO-07 refusal) Summary

**Lifted the two reader-bearing module groups — `ingest/` (umya → owned WorkbookMap with cached-value oracle, WBCO-01) and `provenance/` (quarantined quick-xml/zip raw reader + anchored Excel-identity gate, WBCO-07) — and implemented the net-new `ProvenanceClass` model that REFUSES umya-fabricated provenance under a documented false-positive policy, with a test-only fixture override that cannot weaken production refusal, ZIP/XML/cell hard limits, and a cargo-fuzz target over the untrusted-bytes reader.**

## Performance

- **Duration:** ~40 min
- **Tasks:** 3
- **Files created:** 6 / **modified:** 3

## Accomplishments

- **WBCO-01 ingest:** umya read into an owned `WorkbookMap`/`SheetRecord`/`CellRecord` model with no umya type in the public API; cached `<v>` values captured as the trusted oracle; `MAX_CELL_COUNT` DoS guard emitting `oracle/too-many-cells` fail-closed.
- **WBCO-07 provenance refusal (net-new):** a `ProvenanceClass` enum (`ExcelTrusted`/`NonExcel`/`UmyaFabricated`/`UnknownStale`) is the single authoritative verdict; a umya-stamped workbook (`<Application>Microsoft Excel</Application>` + `calcId=122211`, no `<AppVersion>` build) classifies `UmyaFabricated` and is REFUSED with `oracle/non-excel-app` — the upgrade from the lighthouse's "record-only" behavior.
- **False-positive policy:** the ONLY accept path requires the anchored "Microsoft Excel" name AND a positive `<AppVersion>` build marker; the sentinel calcId alone never refuses (a present AppVersion always admits), so genuine Excel saves are never false-rejected. Documented in `gate.rs` module docs and here.
- **Trusted-fixture override:** `TrustedFixtureMarker` is honored ONLY via the `#[cfg(test)]` `gate_with_fixture_override` entry; production `gate()` always classifies from raw bytes. `override_does_not_weaken_production` asserts the SAME bytes are still refused on the production path.
- **DoS hard limits:** `MAX_ZIP_ENTRY_BYTES`, `MAX_TOTAL_DECOMPRESSED_BYTES`, `MAX_XML_DEPTH` (+ ingest `MAX_CELL_COUNT`) → typed `PartTooLarge`/`DecompressBomb`/`XmlTooDeep`/`TooManyCells`, fail-closed, no panic.
- **Fuzz target:** `fuzz_provenance_reader` drives the raw reader via a `#[cfg(fuzzing)]` hook; the raw reader stays `pub(crate)`-quarantined on non-fuzz builds.
- **47 unit tests** pass (12 ingest + 35 provenance), zero clippy warnings.

## Task Commits

1. **Task 1: Lift ingest/ — umya → owned WorkbookMap with cached-value oracle (WBCO-01)** — `ceb6d8ac` (feat)
2. **Task 2: Lift provenance/ + ProvenanceClass model, REFUSE umya-fabricated (WBCO-07), trusted-fixture override, ZIP/XML hard limits** — `5ac41a23` (feat; TDD red→green for the classifier)
3. **Task 3: Fuzz target over the provenance raw reader** — `785fe601` (test)

_Note: Task 2 is `tdd="true"`. See "TDD Gate Compliance" below._

## Files Created/Modified

- `crates/pmcp-workbook-compiler/src/ingest/cell_map.rs` — owned WorkbookMap/CellRecord/SheetRecord/RangeRef model (created)
- `crates/pmcp-workbook-compiler/src/ingest/mod.rs` — umya collect-all read + MAX_CELL_COUNT guard (filled)
- `crates/pmcp-workbook-compiler/src/provenance/mod.rs` — owned OracleProvenance/RegionHashes/OracleCorpus + ProvenanceError (incl. new DecompressBomb/XmlTooDeep) + quarantine test (filled)
- `crates/pmcp-workbook-compiler/src/provenance/raw_parts.rs` — quarantined pub(crate) quick-xml/zip reader + hard limits + #[cfg(fuzzing)] hook (created)
- `crates/pmcp-workbook-compiler/src/provenance/gate.rs` — ProvenanceClass classifier + WBCO-07 refusal + trusted-fixture override (created)
- `crates/pmcp-workbook-compiler/src/provenance/region_hash.rs` — four region hashes via the runtime's update_field (created)
- `crates/pmcp-workbook-compiler/fuzz/Cargo.toml` + `fuzz_targets/fuzz_provenance_reader.rs` — cargo-fuzz target (created)
- `crates/pmcp-workbook-compiler/Cargo.toml` — direct quick-xml/zip pins at umya's resolved versions + cfg(fuzzing) check-cfg lint (modified)

## Decisions Made

See `key-decisions` frontmatter. Headline: O-3 resolved — the umya-fabrication signal is "anchored Excel name AND no positive AppVersion marker" (sentinel calcId is corroborating, never sufficient alone), keeping the refusal false-positive-safe.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added DecompressBomb + XmlTooDeep ProvenanceError variants and the cumulative/XML-depth hard limits**
- **Found during:** Task 2 (provenance lift)
- **Issue:** The plan's `<interfaces>` mandates `MAX_TOTAL_DECOMPRESSED_BYTES` and `MAX_XML_DEPTH` as fail-closed guards with typed `DecompressBomb`/`XmlTooDeep` findings; the lighthouse `raw_parts.rs` only carried per-entry `PartTooLarge`. Added the constants, the two `ProvenanceError` variants, and an `XmlTooDeep` guard in both XML parsers (mapped to `oracle/unreadable-provenance`), plus a `deeply_nested_xml_fails_closed_to_xml_too_deep` test.
- **Files modified:** provenance/raw_parts.rs, provenance/mod.rs, provenance/gate.rs
- **Verification:** `deeply_nested_xml_fails_closed_to_xml_too_deep` + `zip_bomb_fails_closed` pass.
- **Committed in:** 5ac41a23 (Task 2)

**2. [Rule 2 - Missing Critical] Added MAX_CELL_COUNT ingest DoS guard**
- **Found during:** Task 1 (ingest lift)
- **Issue:** Plan acceptance requires a cell-count cap (T-93-02-DOS) absent in the lighthouse ingest. Added a bounded scan that stops at `MAX_CELL_COUNT` and emits a located `oracle/too-many-cells` Error finding.
- **Files modified:** ingest/mod.rs
- **Verification:** `over_cell_cap_yields_too_many_cells_finding` (drives the bounded scan against a small cap) passes.
- **Committed in:** ceb6d8ac (Task 1)

**3. [Rule 3 - Blocking] Added OracleProvenance.class field + #[non_exhaustive] on ProvenanceError**
- **Found during:** Task 2
- **Issue:** The net-new ProvenanceClass verdict needed a home on the evidence record; the new error variants warranted future-additive safety.
- **Fix:** Added the `class: ProvenanceClass` field to `OracleProvenance` and `#[non_exhaustive]` to `ProvenanceError`.
- **Verification:** quarantine serialize test + full suite green.
- **Committed in:** 5ac41a23 (Task 2)

---

**Total deviations:** 3 auto-fixed (2 missing-critical hard-limit guards, 1 blocking model addition). All were explicitly mandated by the plan's `<interfaces>`/`<threat_model>` (T-93-02-DOS / WBCO-07). No scope creep.

## TDD Gate Compliance

Task 2 is `tdd="true"`. The classifier (`ProvenanceClass`/`classify`) and the seven behavior tests were authored together in one feat commit (`5ac41a23`): the modules are interdependent compilable stubs being filled, so a separate test-only RED commit against a non-existent classifier would not compile. The behavior tests (`classify_excel_trusted_is_accepted`, `classify_umya_fabricated_refused`, `classify_non_excel_refused`, `classify_unknown_stale_refused`, `override_does_not_weaken_production`, `malformed_xlsx_fails_closed`, `zip_bomb_fails_closed`) drive the design and all pass GREEN. No standalone `test(...)` RED commit precedes the `feat(...)` GREEN commit for this task — flagged here per the gate-sequence rule.

## Issues Encountered

- **umya 3.0 Color API:** the colour-authoring test initially used `set_argb(&str)` / `get_color_mut()`, which umya 3.0 deprecated/changed (now `set_argb_str` / `color_mut`). Switched to the documented `Style::set_background_color(&str)` + `font_mut().color_mut().set_argb_str(&str)`. Resolved within Task 1.
- **Fuzz smoke run blocked by sandbox:** `cargo +nightly fuzz run` (executing the instrumented binary) is denied in this execution environment. Per the plan's environment-gated acceptance, the build gate (`cargo +nightly fuzz build fuzz_provenance_reader`, which PASSED) is the CI bar; the smoke run is recorded as skipped here. The invariant is additionally covered by the unit tests `malformed_xlsx_fails_closed`, `zip_bomb_fails_closed`, `deeply_nested_xml_fails_closed_to_xml_too_deep`, and `truncated_zip_is_unreadable_zip`.

## Known Stubs

None — both modules are fully wired. (The crate-level `compile_workbook` driver and the `dialect`/`manifest`/etc. modules remain Wave-1 stubs by design; they are filled by later plans 93-03..93-07 and are out of scope for this plan.)

## Threat Flags

None — no security surface beyond the plan's `<threat_model>` was introduced. The new `DecompressBomb`/`XmlTooDeep` guards strengthen the existing T-93-02-DOS mitigation.

## Next Phase Readiness

- `ingest::ingest` produces the owned `WorkbookMap` (oracle) the linter (93-03) and synthesis (93-04) consume.
- `provenance::gate` + `compute_region_hashes` are ready for the stage-1 composed pass and the reconcile/promote gate (93-06/93-07).
- The raw reader is quarantined `pub(crate)` (+ a `#[cfg(fuzzing)]` hook); the purity gate's positive `cargo tree -p pmcp-workbook-compiler -i umya` assertion and the served-crate negatives remain valid (quick-xml/zip are single-version, no fork).

## Self-Check: PASSED

- All created files verified present (cell_map.rs, raw_parts.rs, gate.rs, region_hash.rs, fuzz target, SUMMARY).
- All task commits verified in git log: `ceb6d8ac`, `5ac41a23`, `785fe601`.
- 47 unit tests pass; zero clippy warnings; fuzz target builds (`cargo +nightly fuzz build`).

---
*Phase: 93-workbook-compiler-5-generalization-fixes-promote-gate*
*Completed: 2026-06-11*
