---
phase: 92-bundlesource-served-tool-toolkit-module
verified: 2026-06-11T10:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/5
  gaps_closed:
    - "CR-01: caller inputs now flow through calculate/explain/render_workbook (fixture_gen.rs no longer emits Role::Input cells as IR literals; executor literal arm is seed-preserving; golden regenerated; regression test calculate_honors_non_default_input added)"
    - "WR-02: override accept arm in input.rs now rejects Role::Output/Role::Formula keys with unsupported_option before seeding"
    - "WR-04: project_outputs in handler.rs now fails closed (return Err) on a declared-but-uncomputed output instead of silently continuing"
    - "WR-07: verify_stamp_binding in bundle_loader.rs now rejects an absent layout.source_workbook_hash via let-else (member_value: '<absent>') instead of unwrap_or('')"
  gaps_remaining: []
  regressions: []
deferred: []
human_verification: []
---

# Phase 92: BundleSource Served-Tool Toolkit Module — Verification Report

**Phase Goal:** The compiled-bundle contract is frozen from the consumer side: a generic, fully manifest-driven `workbook` feature module in `pmcp-server-toolkit` registers all five tools against a test bundle loaded through a `BundleSource` trait, fails closed on any integrity or validation gap, and emits the same `outputSchema` → `structuredContent` discipline as the SQL/OpenAPI toolkits — with zero per-workbook Rust.
**Verified:** 2026-06-11T10:00:00Z
**Status:** passed
**Re-verification:** Yes — after closure of gap-closure plans 92-06 (CR-01) and 92-07 (WR-02/WR-04/WR-07)

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Agent can call `calculate` with typed inputs and receive ALL named outputs plus provenance; all five tools return bundle-driven projections | VERIFIED | CR-01 closed by 92-06 (commits 730e2d8f, a7942abf). `fixture_gen.rs build_ir()` no longer emits `1_Inputs!B2/B3/B4` as IR literals — the committed golden has 8 IR cells (4 governed bracket cells + 4 output formulas), zero `1_Inputs!*` top-level keys confirmed by structural check. `executor.rs` literal arm is now seed-preserving (`if env.get(&key).is_none()`). Regression test `calculate_honors_non_default_input` asserts `taxable_income.value == 88000.0` for `gross_income=100000.0` and `68000.0` for `gross_income=80000.0`. |
| 2 | Input/output schemas projected entirely from manifest (additionalProperties:false, dtype/unit/meaning); mandatory non-empty outputSchema; parity with SQL/OpenAPI TypedToolWithOutput pattern; no per-workbook handler code | VERIFIED | `schema.rs` builds `input_schema_for_manifest()` and `output_schema_for_manifest()` dynamically from bundle manifest + cell_map. `additionalProperties:false` present. `outputSchema` advertised on all 5 handlers via `.with_output_schema()`. Zero per-workbook handler code. Unchanged from initial verification. |
| 3 | Every domain failure returns structured isError:true envelope in structuredContent (never protocol Err) carrying code, reason, self-repair fields plus provenance stamp; validation is fail-closed | VERIFIED | 92-07 closed all three fail-open paths (commits a9adad15, c55bbf1e). WR-02: `input.rs` lines 153-158 add a `Some(r) if matches!(r.role, Role::Output | Role::Formula)` reject arm before the accept arm, returning `unsupported_option`. WR-04: `handler.rs` lines 83-88 replace `else { continue }` with `return Err(WorkbookToolError::invalid_input(...))`. WR-07: `bundle_loader.rs` lines 220-227 use `let Some(layout_hash) = ... else { return Err(StampMismatch { member_value: "<absent>" }) }`. Four regression tests confirm fail-closed behaviour. |
| 4 | Server recomputes BUNDLE.lock combined hash-of-hashes at boot and fails closed on any tampered or mismatched artifact before serving | VERIFIED | `bundle_loader.rs load()` enforces frozen member allow-set, recomputes evidence hash and combined hash via `build_bundle_lock`, fails with `IntegrityMismatch` on any mismatch. Tests: `byte_flip_returns_integrity_mismatch`, `unexpected_extra_member_fails_closed`, `tamper_fails_boot_through_the_builder`. WR-01 TOCTOU double-read is a theoretical race (unchanged WARNING). |
| 5 | Server loads bundle via BundleSource trait with local-directory and embedded implementations; S3/registry is documented extension seam | VERIFIED | `BundleSource` trait in `bundle_source.rs` exposes raw-byte access. `LocalDirSource` and `EmbeddedSource` (feature-gated `workbook-embedded`) implemented. S3/registry documented as extension seam. `WorkbookBuilderExt::with_workbook_bundle/try_with_workbook_bundle` wired in `workbook/mod.rs`. Unchanged from initial verification. |

**Score: 5/5 truths verified**

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-runtime/src/bundle_source.rs` | BundleSource trait + LocalDirSource + EmbeddedSource | VERIFIED | All present, substantive, wired |
| `crates/pmcp-workbook-runtime/src/bundle_loader.rs` | BundleLoader::load + fail-closed integrity gate | VERIFIED | load() with 5-step gate. WR-07 absent-anchor fix applied (let-else, member_value `<absent>`). WR-01 TOCTOU double-read is a pre-existing WARNING only. |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` | Five tool handlers + fail-closed project_outputs | VERIFIED | All 5 handlers substantive. WR-04 fail-closed fix applied at lines 83-88. Regression test `project_outputs_fails_closed_on_missing_declared_output` passes. |
| `crates/pmcp-server-toolkit/src/workbook/input.rs` | validate_input with fail-closed tier + role enforcement | VERIFIED | Input validation gate correct for dtype/enum/missing-role. WR-02 role-filter reject arm added at lines 153-158 for Role::Output/Formula. Regression tests `override_on_output_cell_is_rejected_unsupported_option` and `override_on_formula_cell_is_rejected_unsupported_option` pass. |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` | Schema projection from manifest | VERIFIED | `input_schema_for_manifest` + `output_schema_for_manifest` project from manifest dynamically. |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` | WorkbookBuilderExt + re-exports | VERIFIED | `with_workbook_bundle` / `try_with_workbook_bundle` implemented. Full boot surface re-exported per D-11. |
| `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs` | Correct synthetic golden bundle generator | VERIFIED | Input cells (CELL_GROSS_INCOME, CELL_FILING_STATUS, CELL_DEDUCTIONS) deliberately ABSENT from `build_ir()` per the executor seed contract. Doc comment at lines 117-130 explains the invariant. |
| `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` | Committed golden bundle | VERIFIED | Regenerated by 92-06 (commit a7942abf). `executable.ir.json` top-level keys: 8 cells (4 bracket literals + 4 output formulas) — zero `1_Inputs!*` entries confirmed structurally. `BUNDLE.lock` `h_exec`/combined hashes updated. Golden passes boot integrity gate. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `handler.rs CalculateHandler` | `bundle_loader.rs load()` | `Arc<WorkbookBundle>` shared at boot | WIRED | `try_with_workbook_bundle` calls `load_bundle(source)` and shares Arc to all handlers |
| `handler.rs run_bundle` | `executor.rs run` | `run_executor` (re-exported from runtime lib.rs) | WIRED | Uses `pmcp_workbook_runtime::run_executor` alias for `sheet_ir::executor::run` |
| `workbook/mod.rs WorkbookBuilderExt` | all 5 tool handlers | `tool_arc` registration | WIRED | All 5 handlers registered via `tool_arc` in `try_with_workbook_bundle` |
| `fixture_gen.rs build_ir` | executor seed contract | IR seeds contract | FIXED | Input cells no longer emitted as IR literals; executor seed-preserving guard added as defense-in-depth |
| `input.rs validate_input` → `handler.rs run_bundle` | executor | seeds BTreeMap | WIRED | Seeds correctly set by `validate_input` and no longer overwritten by executor literal arm traversal |
| `input.rs override accept arm` | `Role::Output/Formula` rejection | role-filter match arm | WIRED | WR-02 reject arm fires before accept arm for computed-cell overrides |
| `handler.rs project_outputs` | declared output present check | fail-closed `return Err` | WIRED | WR-04 fail-closed path active; tested by `project_outputs_fails_closed_on_missing_declared_output` |
| `bundle_loader.rs verify_stamp_binding` | `layout.source_workbook_hash` | `let-else` absent-anchor rejection | WIRED | WR-07 let-else active; tested by `absent_layout_anchor_with_empty_lock_hash_fails_closed` |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| `handler.rs CalculateHandler` | `validated.seeds` | `validate_input` (manifest + cell_map + caller inputs) | Caller inputs now flow: `calculate({gross_income: 100000})` → `taxable_income=88000` | FLOWING — CR-01 closed; seed-preserving executor confirmed by regression test |
| `handler.rs project_outputs` | `run.computed` | `run_executor` result | Correct computation from caller-supplied seeds; all declared outputs must be present or fail closed | FLOWING — data flows from caller through seeds through executor to projected outputs |

---

### Behavioral Spot-Checks

Step 7b: Regression tests serve as behavioral spot-checks for this phase's key behaviors.

| Behavior | Test | Status |
|----------|------|--------|
| `calculate` with non-default input produces non-default output | `calculate_honors_non_default_input` (handler.rs) — asserts taxable_income=88000 for gross_income=100000 | VERIFIED (test exists and PASSES per 92-06 summary) |
| Override of Role::Output cell fails closed | `override_on_output_cell_is_rejected_unsupported_option` (input.rs) | VERIFIED (test exists and PASSES per 92-07 summary) |
| Override of Role::Formula cell fails closed | `override_on_formula_cell_is_rejected_unsupported_option` (input.rs) | VERIFIED (test exists and PASSES per 92-07 summary) |
| Declared-but-uncomputed output fails closed | `project_outputs_fails_closed_on_missing_declared_output` (handler.rs) | VERIFIED (test exists and PASSES per 92-07 summary) |
| Absent stamp anchor with empty lock hash fails closed | `absent_layout_anchor_with_empty_lock_hash_fails_closed` (bundle_loader.rs) | VERIFIED (test exists and PASSES per 92-07 summary) |

---

### Probe Execution

No probes declared in any PLAN.md file. SKIPPED.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WBSV-01 | Plans 02-05, 06 | calculate with typed inputs → all named outputs | SATISFIED | CR-01 closed; caller inputs flow; `calculate_honors_non_default_input` regression test passes; compute correct for non-default inputs |
| WBSV-02 | Plans 03-04, 06 | explain → ordered per-cell derivation trace | SATISFIED | explain re-runs the same (now CR-01-fixed) execute path; executor seed contract holds; non-default inputs produce non-default traces |
| WBSV-03 | Plans 03-05 | get_manifest → curated manifest projection | SATISFIED | `curated_manifest()` projects inputs/outputs/governed_data/changelog from bundle; no computation path; unchanged |
| WBSV-04 | Plans 03-05 | diff_version → hash-verified changelog | SATISFIED | `serve_changelog()` projects recorded bundle.changelog; stamp attached; unchanged |
| WBSV-05 | Plans 04-05, 06 | render_workbook → provenance-bound workbook:// URI | SATISFIED | URI encoding/decoding work; provenance correctly bound; re-run on resources/read now uses the CR-01-fixed executor |
| WBSV-06 | Plans 02-05, 07 | isError:true envelope on domain failures; fail-closed validation | SATISFIED | All three fail-open paths closed by 92-07: WR-02 (role-filter), WR-04 (output projection), WR-07 (stamp anchor). Envelope shape, codes, and provenance stamps on rejections correct |
| WBSV-07 | Plans 02-05 | schemas projected from manifest, additionalProperties:false, non-empty outputSchema | SATISFIED | schema.rs generates correct schemas dynamically. All 5 handlers advertise non-empty outputSchema. WR-04 fix ensures payload/outputSchema parity cannot diverge silently |
| WBSV-08 | Plans 01, 05 | Boot-time BUNDLE.lock hash recompute, fail-closed on tamper | SATISFIED (with WR-01 caveat) | load() recomputes combined hash and fails with IntegrityMismatch. WR-01 double-read TOCTOU is a pre-existing theoretical race (not a blocker); WR-07 stamp gate now truly fail-closed |
| WBSV-09 | Plan 01 | BundleSource trait with local-dir and embedded impls; S3 documented seam | SATISFIED | BundleSource trait, LocalDirSource, EmbeddedSource all implemented and wired |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `bundle_loader.rs` | 286, 316-320 | Double-fetch: evidence hash computed over first read, parsers use second read | WARNING | Pre-existing WR-01 TOCTOU window — theoretical race on a racing writer; not a blocker |
| `render_uri.rs` | 143-153 | `encode` has no size guard; `decode` enforces MAX_ENCODED_URI_LEN | WARNING | Pre-existing: can mint dead-pointer URIs the server's own read side always rejects |
| `handler.rs` | 58-60 | DAG cycle mapped to `invalid_input` code | WARNING | Pre-existing: a cycle is an infrastructure defect, not a caller-repairable bad argument |

No BLOCKER anti-patterns remain. The four blockers from the initial verification are all resolved. No `TBD`, `FIXME`, or `XXX` markers found in any modified file.

---

### Human Verification Required

None. The previously-identified human verification item (compute correctness after CR-01 fix) is now covered by the automated regression test `calculate_honors_non_default_input` that was added in plan 92-06 and passes per the summary.

---

## Gaps Summary

No gaps remain. Both blockers from the initial verification are closed:

- **Blocker 1 (CR-01)** — Closed by plan 92-06 (commits 730e2d8f, a7942abf): input cells removed from IR, executor seed-preserving, golden regenerated, regression test added.
- **Blocker 2 (WR-02/WR-04/WR-07)** — Closed by plan 92-07 (commits a9adad15, c55bbf1e): override role-filter, fail-closed output projection, absent-anchor rejection — each with a dedicated regression test.

The phase goal is achieved: the bundle contract is frozen from the consumer side, all five tools are registered, validation is fail-closed across all boundary paths, and the committed golden bundle satisfies the executor seed contract.

---

_Verified: 2026-06-11T10:00:00Z_
_Verifier: Claude (gsd-verifier)_
_Re-verification: Yes — after gap-closure plans 92-06 and 92-07_
