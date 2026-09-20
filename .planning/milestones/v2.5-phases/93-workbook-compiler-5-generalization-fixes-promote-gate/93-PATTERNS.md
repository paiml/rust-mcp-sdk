# Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate - Pattern Map

**Mapped:** 2026-06-11
**Files analyzed:** 1 new crate skeleton + 13 module groups + 2 build-gate files (~16 mapped surfaces)
**Analogs found:** 16 / 16 (every compiler file has BOTH an in-repo structural analog AND a lighthouse lift source)

> **Two-source model for this phase.** Every new compiler file has TWO references:
> 1. **Lift source** (lighthouse, private absolute path) — the code to port + scrub.
> 2. **In-repo structural analog** — the convention to mirror (Cargo.toml shape, crate-deny, re-export style, the runtime types to reuse instead of re-declaring).
>
> The planner must instruct executors to **copy the lighthouse code, then conform it to the in-repo analog** (bump `thiserror` 1→2, re-export runtime types instead of re-declaring, add the `annotations` field, scrub all customer identifiers per Phase 92 D-13).

## File Classification

| New File | Role | Data Flow | In-Repo Analog (convention) | Lighthouse Lift Source (code) | Match |
|----------|------|-----------|------------------------------|-------------------------------|-------|
| `crates/pmcp-workbook-compiler/Cargo.toml` | config | — | `crates/pmcp-workbook-dialect/Cargo.toml` | (new) `workbook-compiler/Cargo.toml` deps | exact |
| `…/src/lib.rs` | config/re-export | — | `crates/pmcp-workbook-runtime/src/lib.rs` (re-export surface) | `workbook-compiler/src/lib.rs:1-238` | exact |
| `…/src/ingest/` | service | file-I/O (umya read) | — (umya-owning; no in-repo analog) | `ingest/{mod,cell_map}.rs` | role-match |
| `…/src/dialect/` | service | transform (lint) | `pmcp-workbook-dialect/src/lib.rs` (WHITELIST contract) | `dialect/{linter,finding,rules,mod}.rs` | exact |
| `…/src/manifest/` | service | transform (synth) | `runtime/src/manifest_model.rs` (`Manifest`/`CellRole`/`Dtype`) | `manifest/{synth,ratify,projections,model,mod}.rs` | role-match |
| `…/src/formula/` | service | transform (parse) | `runtime/src/formula.rs` (`Expr`/`BinOp`/`UnOp` — reuse) | `formula/{token,parser,rebase,mod}.rs` | role-match |
| `…/src/dag/` | service | transform (toposort) | `runtime/src/dag.rs` + `resolve.rs` (`Dag`/`toposort` — reuse) | `dag/{graph,resolve,topo,mod}.rs` | role-match |
| `…/src/sheet_ir/` | service | transform (eval bridge) | `runtime/src/sheet_ir/` (executor + `rounding` — reuse) | `sheet_ir/{eval_bridge,executor,mod}.rs` (NOT `loop_exec`) | role-match |
| `…/src/reconcile/` | service | transform (grade vs oracle) | `runtime/src/sheet_ir/rounding.rs` (anchor) | `reconcile/{classifier,drift,mod}.rs` | role-match |
| `…/src/provenance/` | service | file-I/O (quarantined raw read) | — (quick-xml/zip-owning; no analog) | `provenance/{gate,raw_parts,region_hash,mod}.rs` | role-match |
| `…/src/artifact/` | service | file-I/O (bundle emit) | `runtime/src/artifact_model.rs` (`build_bundle_lock`/`fold_evidence_hash` — reuse) + `toolkit/src/workbook/` (consumer contract) + `tests/fixtures/tax-calc@1.1.0/` (golden) | `artifact/{mod,bundle_lock,cell_map,evidence,executable,layout}.rs` | exact |
| `…/src/change_class/` | service | transform (diff/classify) | `runtime/src/changelog.rs` (`ChangeClass` enum — reuse) | `change_class/{mod,ir_identity,schema_diff}.rs` | exact |
| `…/src/gate/` | service | event-driven (promote gate) | — (build-time governance; no analog) | `gate/{mod,corpus,accept,governed_artifact}.rs` | role-match |
| `…/src/stage1.rs` | service | batch (composed pass) | — | `stage1.rs` | role-match |
| `Makefile` (purity-check ext) | config | — | `Makefile:493-516` (`PURITY_CRATES` extension procedure) | — (in-repo only) | exact |
| `…/tests/` neutral `tax-calc.xlsx` fixture | test | file-I/O | `tests/fixtures/tax-calc@1.1.0/` (golden target) | — (new authoring) | partial |

## Pattern Assignments

### `crates/pmcp-workbook-compiler/Cargo.toml` (config)

**In-repo analog:** `crates/pmcp-workbook-dialect/Cargo.toml` (closest because it is a path-dep'ing sibling workbook crate with the lean `exclude` + `docs.rs` + `[lib]` shape).

**Mirror this package/lib block** (`pmcp-workbook-dialect/Cargo.toml:1-19`):
```toml
[package]
name = "pmcp-workbook-compiler"      # was workbook-compiler
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Offline Excel→MCP compiler: umya-isolated ingest→lint→synth→DAG→reconcile→emit→promote-gate"
exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
name = "pmcp_workbook_compiler"       # underscore crate name (matches dialect/runtime convention)
path = "src/lib.rs"
```

**Dep conventions to copy from in-repo siblings:**
- Path+version dep form (`pmcp-workbook-dialect/Cargo.toml:25`): `pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }` — add a second identical line for `pmcp-workbook-dialect`.
- `thiserror = "2"`, `serde`/`serde_json = "1"`, `schemars = "1.0"`, `sha2 = "0.11"`, `hex = "0.4"`, `chrono = "0.4"` — match the runtime pins (`pmcp-workbook-runtime/Cargo.toml:22-27`). **Lighthouse uses `thiserror = "1"`; bump to `2` on lift.**
- **NEW reader deps (not in any sibling — gate first install per RESEARCH Package Audit):** `umya-spreadsheet = "3.0"`, `quick-xml`/`zip` pinned via `cargo tree -p umya-spreadsheet -i quick-xml` / `-i zip`.
- **DROP `pmcp-code-mode`/SWC** from the lighthouse dep list (runtime has pure-Rust `scalar_eval`; verify reconcile parity per O-1).

---

### `…/src/lib.rs` (config / re-export surface)

**In-repo analog:** `crates/pmcp-workbook-runtime/src/lib.rs` (the re-export style + the crate-deny header). **Lift source:** `workbook-compiler/src/lib.rs:1-238`.

**Crate-deny header — copy verbatim** (identical in both `runtime/src/lib.rs:18-19` and lighthouse `lib.rs:25-28`):
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```

**Re-export-don't-re-declare pattern.** The lighthouse already re-exports the relocated runtime types (`lib.rs:164-173`). In the SDK, rename `workbook_runtime::` → `pmcp_workbook_runtime::`:
```rust
// Lighthouse lib.rs:164 — rename crate path on lift:
pub use pmcp_workbook_runtime::{self, toposort, BinOp, CellValue, Dag, ExcelError, Expr, UnOp};
pub use pmcp_workbook_runtime::changelog;
pub use pmcp_workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};
```

**Name-collision precedent to preserve** (lighthouse `lib.rs:146-152`, runtime `lib.rs:105-109`): `changelog::Severity` vs `finding::Severity` — keep `changelog::Severity` reachable only via module path; the bare `Severity` stays the lint-finding tier. The runtime already enforces this exact rule — match it.

**DELETE on lift (the one surviving §5 gap — WBCO-02):** `build_reference_manifest` (`lib.rs:769-858`) and its re-export, plus `REFERENCE_WORKBOOK_PATH`/`WORKFLOW_NAME = "ufh-quote"` consts. Replace with a generic `compile_workbook(workbook_path, out_root, …)` driver (see RESEARCH "Code Examples"). Keep `build_reference_manifest` ONLY as a `#[cfg(test)]` anti-drift fixture if needed, scrubbed.

**Re-export surface to KEEP** (lighthouse `lib.rs:120-230`): the `dialect::{lint,…}`, `ingest::{ingest,WorkbookMap,…}`, `manifest::{synthesize,ratify,…}`, `gate::corpus::{candidate_fingerprint,ApprovalRecord,…}`, `change_class::{effective_policy,ir_subdag_hash,…}` lists — verbatim, scrubbed of customer names.

**DO NOT re-export** (deferred, RESEARCH Anti-Patterns): `run_with_loop`, `RoomAggregator` (lighthouse `lib.rs:215`), the `reconcile::build_coil_*`/`build_supply_total_*` customer IR builders (`lib.rs:227-230`).

---

### `…/src/manifest/` (service, transform — WBCO-02/06)

**In-repo analog:** `crates/pmcp-workbook-runtime/src/manifest_model.rs` — **the `Manifest` the compiler EMITS already lives here. Re-export it; do NOT re-declare a local `Manifest`.** **Lift source:** `manifest/{synth,ratify}.rs` (the colour/Guide/header heuristics).

**CRITICAL reconciliation site — the `annotations` delta (RESEARCH Pitfall 2).** Every hand-built `Manifest { … }` literal lifted from the lighthouse MUST add the in-repo field. The in-repo struct (`manifest_model.rs:290-327`) has fields the lighthouse `Manifest` lacks:
```rust
// pmcp-workbook-runtime/src/manifest_model.rs:290-327 — the TARGET struct:
pub struct Manifest {
    pub schema_version: u32,
    pub workflow: String,
    pub workbook_hash: Option<String>,
    pub ratified: bool,
    pub ratified_by: Option<String>,        // D-04 sign-off — carry forward
    pub ratified_at: Option<String>,        // D-04 sign-off — carry forward
    pub cells: Vec<CellRole>,
    pub loop_block: Option<LoopDecl>,
    #[serde(default)] pub governed_data: Vec<GovernedDatum>,
    #[serde(default)] pub changelog: Vec<ChangelogEntry>,
    #[serde(default)] pub capability_calls: Vec<CapabilityDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<AnnotationDecl>,   // ← NOT in lighthouse; synth must populate or `vec![]`
}
```
Executor instruction: at every `Manifest { … }` literal site, supply `annotations: vec![]` (or synthesize from a Guide-annotation convention) and confirm `ratified`/`ratified_by`/`ratified_at` are present. The byte-identical re-emit test fails loudly if `annotations` is wrong.

**Also reconcile** (runtime `manifest_model.rs:40,74,143`): `Role` (`Input`/`Constant`/`Output`/`Formula`), `Dtype` (`Number`/`Text`/`Bool`), `InputTier` — re-export these from runtime, do not redefine.

---

### `…/src/dialect/` (service, transform — WBDL-03)

**In-repo analog:** `crates/pmcp-workbook-dialect/src/lib.rs` — **the `WHITELIST` const + `DialectRules` the linter runs against ALREADY exist here (Phase 91).** The running linter executes against this contract. **Lift source:** `dialect/{linter,finding,rules}.rs`.

**Consume the contract** (`pmcp-workbook-dialect/src/lib.rs:35,82,53`): `WHITELIST: &[&str]` (13 fns), `DialectRules`, `CandidateRole`. The lighthouse `dialect/rules.rs` re-declares these — **on lift, DELETE the local copies and re-export from `pmcp_workbook_dialect`** (the dialect crate's `rules.rs:248-336` spec-binding test asserts the WHITELIST table cannot drift; a second copy would defeat it).

**Reuse runtime findings** (`runtime/src/finding.rs`, re-exported at `runtime/lib.rs:77`): `LintFinding`/`LintReport`/`Severity` — the linter emits these collect-all located findings (D-01). The lighthouse `dialect/finding.rs` (502B) is now a runtime type; re-export, don't redeclare.

---

### `…/src/reconcile/` (service, transform — WBCO-04)

**In-repo analog:** `crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs` (the anchor helpers). **Lift source:** `reconcile/classifier.rs` (lift verbatim).

**Operand-anchored pattern — grep-gated, never `delta.abs()`** (lighthouse `reconcile/classifier.rs:14-24` invariant doc):
```rust
// RoundingBoundary fires ONLY when the deciding cell's Expr contains a
// ROUND/ROUNDUP/CEILING Expr::Call AND the operand sits within epsilon of the
// boundary. A rule branching on `delta.abs() < X` is FORBIDDEN — a grep gate
// asserts `delta.abs()` never appears in this file.
const BOUNDARY_EPSILON: f64 = 1e-6;
```
The classifier imports the runtime anchors — repoint `workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup}` → `pmcp_workbook_runtime::sheet_ir::rounding::{…}`. **D-03 split:** named-output mismatch = ERROR (blocks emit); helper-cell mismatch = WARNING. Add the grep-gate test: `! grep -n 'delta.abs()' src/reconcile/classifier.rs`.

---

### `…/src/change_class/` (service, transform — WBGV-01/02/03)

**In-repo analog:** `crates/pmcp-workbook-runtime/src/changelog.rs` (`ChangeClass`/`OutputDelta`/`VersionChangelog` enums — re-export, do NOT re-declare; the served `diff_version` tool reads the SAME enum). **Lift source:** `change_class/{mod,ir_identity,schema_diff}.rs`.

**CR-01 symmetric classifier — ALREADY DONE in lighthouse; lift + verify, do NOT re-implement** (`change_class/mod.rs:165-255`). Assumption-first hard rule + role-flip-away arms (excerpt `mod.rs:178-205`):
```rust
// Assumption FIRST: assumption involvement on EITHER side → Assumption (CR-01).
let prev_was_assumption = prev_role.is_some_and(is_assumption);
if is_assumption(cur) || prev_was_assumption { /* … push Assumption; continue */ }
// Role flips AWAY from Input/Output are schema changes too (CR-01):
if matches!(p.role, Role::Input) && !matches!(cur.role, Role::Input) {
    out.push((ChangeClass::InputSchema, key));
}
// Constant | Formula => {}  // no longer silently drops a demotion
```
**Verify the 8 demotion tests come over green** (`assumption_demotion_to_plain_constant`, `input_demoted_to_constant`, `output_demoted_to_formula`, `enum_drop_with_role_flip`). **WBGV-02:** `effective_policy` is derived `Ord` on `GatePolicy` (lighthouse `mod.rs:351-359`) — assumption → `NeverAutoPromote` wins. **WBGV-03:** lift `ir_identity.rs` `ir_subdag_hash`.

---

### `…/src/artifact/` (service, file-I/O — WBCO-05/07-tier WR-01)

**In-repo analogs (three):**
1. `runtime/src/artifact_model.rs` — `build_bundle_lock`/`fold_evidence_hash`/`sha256_hex` (re-export at `runtime/lib.rs:92-95`); **never hand-roll the combined hash** (the served loader recomputes with these).
2. `crates/pmcp-server-toolkit/src/workbook/` (the CONSUMER — `mod.rs`, `handler.rs`, `input.rs`, `schema.rs`) — the output contract the emitted bytes must satisfy at boot.
3. `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` — the seven-member golden to reproduce (`manifest.json`, `executable.ir.json`, `cell_map.json`, `layout.json`, `BUNDLE.lock`, `evidence/`).

**Lift source:** `artifact/{mod,bundle_lock,cell_map,evidence,executable,layout}.rs`.

**WR-01 enum-tier skip — ALREADY DONE; lift + verify** (`artifact/mod.rs:121-130`):
```rust
fn ratify_tiers(manifest: &Manifest) -> Result<Manifest, EmitError> {
    let mut ratified = manifest.clone();
    for role in &mut ratified.cells {
        if matches!(role.role, Role::Input) && role.tier.is_none()
            && role.allowed_values.is_none()        // ← WR-01: skip frozen-enum inputs
        { role.tier = Some(default_variable_tier(role)?); }
    }
    Ok(ratified)
}
```
Verify against the **committed** manifest (not the in-memory builder): `ratify_skips_frozen_enum_inputs` test (WBGV-07).

---

### `…/src/provenance/` (service, file-I/O — WBCO-07)

**In-repo analog:** none (quick-xml/zip-owning; the raw reader is `pub(crate)`, never re-exported, never enters the served tree — RESEARCH Pitfall 4). **Lift source:** `provenance/{gate,raw_parts,region_hash}.rs`.

**Anchored identity — lift verbatim, then UPGRADE to refuse umya** (`provenance/gate.rs:255-263`):
```rust
let is_excel = app.application.as_deref()
    .is_some_and(|a| a.trim_start().starts_with("Microsoft Excel"));   // WR-03 anchored, not .contains
```
**NET-NEW (WBCO-07, RESEARCH Pitfall 5 / O-3):** umya FABRICATES `<Application>Microsoft Excel</Application>` + `calcId=122211`, so `.starts_with` PASSES umya. Add a signal beyond it (detect `calcId == 122211` sentinel / absence of an Excel build string) and **REFUSE** with `oracle/non-excel-app`. Add the regression test: author a workbook with umya → assert the gate REFUSES it (`provenance::umya_fabricated_refused`).

---

### `…/src/gate/` (service, event-driven — WBGV-04/05/06)

**In-repo analog:** none (build-time governance, on-disk reviewable dir, never served). **Lift source:** `gate/{mod,corpus,accept,governed_artifact}.rs`.

**Reuse verbatim** (lighthouse `gate/corpus.rs:96-160`): `candidate_fingerprint` (sha256 over prev-hash + candidate-hash + region deltas — WR-04 content-binding), `ApprovalRecord`, `ApprovalCase`, `accept` (`gate/accept.rs`).

**CR-02 versioned non-overwriting promote — ALREADY DONE; lift + verify** (`commands/compile_workbook.rs:603-611`):
```rust
candidate.version = next_version;  // write lands in a NEW {name}@{next_version}/ dir
candidate.changelog = Some(changelog);
crate::write_candidate_bundle(&candidate, out_root, crate::EmitLane::GatedUpdate)?;
```
Verify the promote-twice integration test (two dirs, baseline byte-identical — WBGV-06).

**NET-NEW (D-09 / O-4 — the genuine new design):** the lighthouse corpus is BA-curated `cases.json`. Replace with an **auto-derived** grid from manifest defaults + enum domains (bounded: default + one case per enum member + numeric boundary, capped at small N). Reuse the `ApprovalCase`/`expected_outputs` BTreeMap shape + `candidate_fingerprint` + `accept` verbatim; only case *generation* is new. D-12: first version is a no-op that establishes the baseline.

---

### `Makefile` purity-check extension (config)

**In-repo analog (and modify-target):** `Makefile:493-516` — the `PURITY_CRATES`/`PURITY_WRITER_CRATES` lists + documented extension procedure.
```makefile
PURITY_CRATES := pmcp-workbook-runtime pmcp-workbook-dialect
PURITY_WRITER_CRATES := pmcp-workbook-runtime
```
**The compiler is the EXCEPTION — do NOT append it to `PURITY_CRATES`** (RESEARCH Pitfall 4). Instead: add a POSITIVE assertion `cargo tree -p pmcp-workbook-compiler -i umya` MUST be non-empty (reader IS here), re-run the existing per-served-crate negatives (runtime/dialect/toolkit) to confirm the compiler's dep did not leak umya into them via the shared runtime, and add a `quick-xml`/`zip` single-version guard (no forked second copy in `Cargo.lock`).

## Shared Patterns

### Crate-level panic-freedom gate
**Source:** `crates/pmcp-workbook-runtime/src/lib.rs:18-19` (identical to lighthouse `lib.rs:25-28`).
**Apply to:** every compiler module — the `lib.rs` header below covers all of them.
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```

### Re-export-don't-re-declare (the keystone pattern)
**Source:** `crates/pmcp-workbook-runtime/src/lib.rs:74-118` (the curated re-export surface).
**Apply to:** `manifest/`, `formula/`, `dag/`, `sheet_ir/`, `reconcile/`, `change_class/`, `artifact/`, `dialect/finding`.
Every shared model/IR/hash/changelog/finding/rounding type is `pub use pmcp_workbook_runtime::…` (or `pmcp_workbook_dialect::…`), NOT a fresh `struct`/`enum`. A second `Manifest`/`ChangeClass`/`WHITELIST` would make the served `diff_version`/loader read a DIFFERENT definition.

### Customer-identifier scrub (Phase 92 D-13)
**Source:** RESEARCH Runtime State Inventory (~34 lighthouse files).
**Apply to:** EVERY lifted file (code, comments, fixtures, docs).
Grep gate must return ZERO in non-test/non-fixture paths:
```bash
! grep -ri 'ufh\|towelrad\|coil\|heat_source\|heat_pump\|underfloor\|radiator\|plot3' \
    crates/pmcp-workbook-compiler/src/
```
Fixtures use only neutral domain names (e.g. `tax-calc`).

### Path+version internal dep form
**Source:** `crates/pmcp-workbook-dialect/Cargo.toml:25`.
**Apply to:** the compiler's `pmcp-workbook-runtime` + `pmcp-workbook-dialect` deps.
```toml
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }
```

## No In-Repo Analog (lift source only — use lighthouse + RESEARCH patterns)

| File | Role | Data Flow | Reason no in-repo structural analog |
|------|------|-----------|--------------------------------------|
| `…/src/ingest/` | service | file-I/O | umya is confined here; no in-repo crate reads `.xlsx` (purity boundary). Lift `ingest/{mod,cell_map}.rs`; expose owned `WorkbookMap`/`CellRecord` (no umya types leak). |
| `…/src/provenance/` | service | file-I/O | quick-xml/zip raw reader is `pub(crate)`-only; nothing in-repo parses ZIP/XML parts. Lift + WBCO-07 refuse-upgrade. |
| `…/src/gate/` | service | event-driven | Build-time governance dir; no served analog. Lift + D-09 auto-corpus (new). |
| `…/src/stage1.rs` | service | batch | Composed collect-all pass unique to the compiler. Lift `stage1.rs`. |
| neutral `tax-calc.xlsx` fixture | test | file-I/O | The golden was synthesized from runtime types (no source `.xlsx`); must AUTHOR a real workbook NOT via umya (O-2/Pitfall 3). |

## Metadata

**Analog search scope:** `crates/pmcp-workbook-runtime/`, `crates/pmcp-workbook-dialect/`, `crates/pmcp-server-toolkit/src/workbook/`, `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/`, `Makefile`; lift source `…/towelrads/…/crates/workbook-compiler/src/` (full tree listed).
**Files scanned:** runtime `lib.rs` + `manifest_model.rs`; dialect `lib.rs` + Cargo.toml; runtime + dialect Cargo.toml; lighthouse `lib.rs`, `change_class/mod.rs`, `reconcile/classifier.rs`, `artifact/mod.rs`, `commands/compile_workbook.rs`, `provenance/gate.rs` + full module tree listing.
**Pattern extraction date:** 2026-06-11
