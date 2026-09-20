# Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation - Pattern Map

**Mapped:** 2026-06-14
**Files analyzed:** 13 new/modified
**Analogs found:** 13 / 13 (every file maps onto a proven in-repo analog — this phase is composition, not new infrastructure)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `cargo-pmcp/src/commands/new.rs` (MODIFY: add `workbook-server` arm) | command/dispatch | request-response | same file — `execute_sql_server` arm (`new.rs:70-80`, `146-156`) | exact (in-file sibling) |
| `cargo-pmcp/src/templates/workbook_server.rs` (NEW) | template/code-gen | file-I/O (raw `fs::write`) | `cargo-pmcp/src/templates/sql_server.rs` | exact (clone) |
| `cargo-pmcp/src/templates/mod.rs` (MODIFY: register module) | config/module-index | — | same file (`pub mod sql_server;` line) | exact |
| `crates/pmcp-workbook-compiler/src/dialect_version.rs` (NEW) | service/accessor | transform (read-only over `WorkbookMap`) | `crates/pmcp-workbook-compiler/src/version.rs` | exact-shape, INVERTED absence policy |
| `crates/pmcp-workbook-compiler/src/lib.rs` (MODIFY: wire reader into `compile_workbook_inner`) | service/orchestrator | transform pipeline | same file — `promote_named_outputs` call at step (3a), `lib.rs:264-269` | exact (in-file sibling) |
| `crates/pmcp-workbook-dialect/src/lib.rs` (MODIFY: add `SUPPORTED_/BASELINE_DIALECT_VERSION` consts + drift guard) | config/contract + test | — | same file — `WHITELIST` const + `dialect_spec` drift-binding test (`lib.rs:35-38`, `237-342`) | exact (in-file parallel) |
| `docs/workbook-dialect-spec.md` (MODIFY: add version-policy §) | doc/contract | — | same file — the WHITELIST table the `dialect_spec` test binds to | exact (in-file parallel) |
| `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx` (NEW) + `loan-calc.provenance-override.json` | fixture (binary) + marker | file-I/O | `tests/fixtures/tax-calc.xlsx` + `tax-calc.provenance-override.json` | exact (recipe clone) |
| `crates/pmcp-workbook-compiler/src/reemit_loan.rs` (NEW, `#[cfg(test)]`) | test/integration proof | batch (compile→serve) | `crates/pmcp-workbook-compiler/src/reemit_golden.rs` | exact (clone, ITS own oracle) |
| `crates/pmcp-workbook-compiler/tests/fixtures/quirks/*.xlsx` (~7-9 NEW) + override markers | fixture (binary) + markers | file-I/O | `tax-calc.xlsx` recipe + per-fixture override | exact (recipe clone) |
| `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs` (NEW, `#[cfg(test)]`) | test/integration | batch (reconcile fixtures) | `reemit_golden.rs` compile path + `reconcile::within_tol` (`reconcile/mod.rs:43-53`) | exact (compose two analogs) |
| `crates/pmcp-workbook-runtime/src/scalar_eval.rs` (MODIFY: add quirk unit tests) | test/unit | — | same file — existing `mod tests` (`scalar_eval.rs:262-332`) | exact (in-file extend) |
| `cargo-pmcp` fuzz target (NEW) for dialect-version-string parse + property tests | test/fuzz+property | — | `version.rs` round-trip property test (`version.rs:241-257`) | role-match (property); fuzz is new surface |

## Pattern Assignments

---

### `cargo-pmcp/src/commands/new.rs` (command/dispatch — MODIFY) — WBCL-05

**Analog:** same file, the `sql-server` arm.

**Dispatch arm to add** (mirror `new.rs:70-80`):
```rust
match kind.as_deref() {
    Some("sql-server") => return execute_sql_server(&workspace_dir, &name, global_flags),
    Some("openapi-server") => return execute_openapi_server(&workspace_dir, &name, global_flags),
    // ADD:
    Some("workbook-server") => return execute_workbook_server(&workspace_dir, &name, global_flags),
    Some(k) => anyhow::bail!(
        "unknown --kind '{}'; supported: sql-server, openapi-server, workbook-server", k),
    None => {},
}
```
Note: the `bail!` supported-list string MUST be updated to include `workbook-server`.

**`execute_workbook_server` body** (mirror `execute_sql_server`, `new.rs:146-156`) — `validate_crate_name` FIRST (path-traversal guard), then create dirs, then delegate:
```rust
fn execute_workbook_server(
    workspace_dir: &Path,
    name: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    validate_crate_name(name)?;                       // BEFORE any fs::write (T-86-03-02)
    fs::create_dir_all(workspace_dir.join("src")).context("...")?;
    templates::workbook_server::generate(workspace_dir, name)?;
    // ... not_quiet success println
    Ok(())
}
```

**V5 input-validation guard (REUSE, do not reimplement)** — `validate_crate_name` at `new.rs:116-140` already rejects `/`, `\`, `..`, leading digit, and non-`[A-Za-z0-9_-]`. Call it; do not duplicate.

**CLI `--kind` arg is already generic** — `main.rs:101-104` defines `kind: Option<String>` and `main.rs:555-556` forwards it to `new::execute`; no clap change needed, only the doc comment example MAY be updated.

---

### `cargo-pmcp/src/templates/workbook_server.rs` (template/code-gen — NEW) — WBCL-05

**Analog:** `cargo-pmcp/src/templates/sql_server.rs` (full file is the template; clone its structure).

**Module shape to mirror** (`sql_server.rs:33-44`): one `pub fn generate(dir, name)` orchestrator calling one private `generate_<file>` per emitted file, each a single raw `fs::write(...).context(...)`. **NO template engine** — raw string literals via `format!` with `{{`/`}}` escaping.

**Emitted files for Shape B (D-06):** `Cargo.toml`, `src/main.rs`, `pmcp.toml` (sample), `workbook/tax-calc.xlsx` (D-07 source), and the embedded `bundle/tax-calc@1.1.0/*` (D-07 pre-compiled). NOTE this DIVERGES from sql_server's `schema.sql`/`deploy.toml` set — emit the workbook payload set instead.

**Cargo.toml — CRITICAL purity posture** (mirror Shape A `crates/pmcp-workbook-server/Cargo.toml`, NOT sql_server's `code-mode` posture):
```toml
[dependencies]
pmcp = { version = "2.9.0", features = ["streamable-http"] }
# default-features = false is MANDATORY (T-95-06 purity gate): toolkit DEFAULT
# pulls `code-mode` → pmcp-code-mode (SWC/JS) into the served tree → trips
# `make purity-check`. workbook-embedded (NOT bare workbook) for EmbeddedSource.
pmcp-server-toolkit = { version = "0.1.0", default-features = false, features = ["workbook-embedded", "http"] }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```
(Anti-pattern from sql_server.rs:58 — do NOT copy `features = ["code-mode", "sqlite", "http"]`; that is the purity-gate trip, Pitfall 4.)

**Emitted `src/main.rs` — drift-locked to the example** (mirror `sql_server.rs:75-127` `emitted_main_rs()`): the body is the EmbeddedSource wiring from `crates/pmcp-server-toolkit/examples/workbook_server_http.rs:65-78`:
```rust
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundle/tax-calc@1.1.0");
// ...
let builder = Server::builder().name("workbook-tax-calc").version("1.1.0");
let builder = builder.try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))?;
let server = builder.build()?;
let (addr, handle) = serve(server).await?;
println!("PMCP_WORKBOOK_SERVER_ADDR=http://{addr}");
```
Import SOLELY from the toolkit (D-11): `use pmcp_server_toolkit::workbook::{EmbeddedSource, WorkbookBuilderExt};` — NEVER name `pmcp-workbook-runtime`. Note the example's `include_dir!` path must be rewritten from `tests/fixtures/tax-calc@1.1.0` (example's path) to the scaffold-local `bundle/tax-calc@1.1.0` (the carried copy).

**Drift-lock golden test (clone `sql_server.rs:285-339`):** a `#[cfg(test)] mod tests` with `EXAMPLE_SRC = include_str!("../../../crates/pmcp-server-toolkit/examples/workbook_server_http.rs")`, a `wiring_lines()` normalizer that strips comments + harness-only seams (the example's `--bundle-dir`/`LocalDirSource` branch is the harness seam to filter), and `emitted_main_matches_example_modulo_setup` asserting equality. Mirror the exact filter/compare structure.

**Bundle-bytes-match golden test (Pitfall 5):** add a test asserting the carried `bundle/tax-calc@1.1.0` bytes match the committed `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0` source (no drift / no boot-integrity break).

---

### `cargo-pmcp/src/templates/mod.rs` (module-index — MODIFY) — WBCL-05

**Analog:** same file. Add `pub mod workbook_server;` alongside `pub mod sql_server;` (kept alphabetical with the existing list).

---

### `crates/pmcp-workbook-compiler/src/dialect_version.rs` (service/accessor — NEW) — WBDL-02

**Analog:** `crates/pmcp-workbook-compiler/src/version.rs` (clone the defined-name scan; **INVERT the absence policy**).

**CRITICAL — do NOT edit `version.rs`.** `version.rs::read_workbook_version` reads `version`/`wb_version` (the BUNDLE version, Phase 94 CLI) and **errors on absent** (`version.rs:45-56`). WBDL-02 is a SEPARATE declaration (`pmcp_dialect_version`) with **absent → baseline, no error** (D-05). Add a sibling; editing `version.rs`'s absence semantics breaks Phase 94 (Pitfall 2 / Anti-Pattern).

**Reuse the single-cell defined-name scan** (copy `version.rs:62-95`, change the name const + None-handling):
```rust
const DIALECT_VERSION_NAME: &str = "pmcp_dialect_version";  // D-03 working name

fn declared_dialect_version(map: &WorkbookMap) -> Option<String> {
    for dn in &map.defined_names {
        if !dn.name.eq_ignore_ascii_case(DIALECT_VERSION_NAME) { continue; }
        if dn.target.start != dn.target.end { continue; }     // single-cell only
        if let Some(v) = cell_value_for_key(map, &dn.target.sheet, &dn.target.start) {
            let t = v.trim();
            if !t.is_empty() { return Some(t.to_string()); }
        }
    }
    None  // D-05: None → BASELINE (NOT an error, unlike version.rs)
}

// REUSE verbatim from version.rs:91-95 (the owned-map cell read):
fn cell_value_for_key<'a>(map: &'a WorkbookMap, sheet: &str, addr: &str) -> Option<&'a str> {
    let sheet_rec = map.sheets.iter().find(|s| s.name == sheet)?;
    let cell = sheet_rec.cells.iter().find(|c| c.addr == addr)?;
    cell.value.as_deref()
}
```

**D-04 semver-compat decision (the only genuinely new ~30 lines):** parse `MAJOR.MINOR` (hand-roll; no `semver` crate — A1, matches `version.rs`'s crate-free parse), compare to `SUPPORTED_DIALECT_VERSION` (owned by the dialect crate):
```rust
// same major && declared_minor <= supported_minor  → accept
// different major | declared > supported           → Err(CompileError::Lint(...))  fail-closed (D-04)
// None                                              → BASELINE_DIALECT_VERSION, MAY push Severity::Info advisory (D-05)
```
Use the typed-error pattern from `version.rs:49` (`CompileError::Lint(format!(...))`).

**Test module — clone `version.rs:97-258` synthetic-map helpers** (`map_declaring`, `cell`, `single_cell_range`, `defined_name`) verbatim; they let tests drive the reader without a real `.xlsx`. Add cases: same-major-≤-supported accepts; different-major / newer → typed Err; absent → baseline (no error); plus the `version.rs:241-257` round-trip PROPERTY adapted to the compat decision (ALWAYS property requirement).

---

### `crates/pmcp-workbook-compiler/src/lib.rs` (orchestrator — MODIFY) — WBDL-02

**Analog:** same file — the `promote_named_outputs(&mut manifest, &map)` call at step (3a) (`lib.rs:264-269`) is the model for inserting a new read over the ingested `map` into the pipeline.

**Integration seam:** `compile_workbook_inner` (`lib.rs:243-330`) does: (1) read bytes, (2) `ingest::ingest` → `map`, (3) `stage1::run_stage1`. The dialect-version check goes **after step (2) ingest produces `map`** — naturally either as a stage-1 collect-all finding (A2, more consistent with the collect-all "fix one re-save" ethos) or a standalone gate immediately after ingest. Mirror how `promote_named_outputs` consumes `&map` post-ingest. Re-export the new reader at the `pub use version::read_workbook_version;` line (`lib.rs:189`).

---

### `crates/pmcp-workbook-dialect/src/lib.rs` (contract const + drift guard — MODIFY) — WBDL-02

**Analog:** same file — `WHITELIST` const (`lib.rs:35-38`) + the `dialect_spec` doc-binding test (`lib.rs:237-342`) are the exact parallel for a version const + its spec drift-guard.

**Add the consts (parallel to WHITELIST, A4):**
```rust
pub const BASELINE_DIALECT_VERSION: &str = "1.0";   // D-05 absent→this (exact string = discretion)
pub const SUPPORTED_DIALECT_VERSION: &str = "1.0";  // the compiler's max-supported
```
Ownership in the dialect crate matches the WBDL-01 precedent (the contract surface lives here, parallel to WHITELIST).

**Drift-guard test (clone the `dialect_spec` module, `lib.rs:246-342`):** the spec doc declares the supported/baseline version in a documented section; a test parses it out of `docs/workbook-dialect-spec.md` and asserts set/string-equality with the const — READS the const, does not redefine it. Reuse verbatim: the `SPEC_PATH = "../../docs/workbook-dialect-spec.md"` resolution, the published-package SKIP-if-absent / in-repo-FAIL-if-absent `.git`-two-levels-up guard (`lib.rs:294-321`), and the non-empty-parse Pitfall guard (`lib.rs:328-332`). Also mirror the `whitelist_canary_count_unique_uppercase` canary (`lib.rs:163-182`) style for a version-format canary if useful.

---

### `docs/workbook-dialect-spec.md` (doc/contract — MODIFY) — WBDL-02

**Analog:** same file — the existing whitelist table that the `dialect_spec` test binds to.

**Add a "Dialect version declaration & compatibility policy" section** documenting: the `pmcp_dialect_version` named-range convention (D-03), the semver-compat rule (same major, declared minor ≤ supported → accept; else fail-closed, D-04), absent → baseline (D-05), and the `SUPPORTED_/BASELINE_DIALECT_VERSION` values in a parse-stable form the new drift-guard test reads (mirror the whitelist table's `| \`token\` | category | ... |` machine-parseable shape).

---

### `tests/fixtures/loan-calc.xlsx` + `loan-calc.provenance-override.json` (fixture — NEW) — WBEX-01

**Analog:** `crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx` + `tax-calc.provenance-override.json`.

**LANDMINE — there is NO in-repo .xlsx generator.** `tax-calc.xlsx` is a committed binary; no `set_application`/`DocProperties` author exists in the tree (verified). The plan MUST include an explicit fixture-authoring task. Two sanctioned recipes (RESEARCH Pattern 4):
1. **Genuine Excel identity:** author via `rust_xlsxwriter` (already a compiler dev-dep, `Cargo.toml:91`) setting `DocProperties` so app.xml carries `<Application>Microsoft Excel</Application>` + `<AppVersion>` + non-sentinel calcId → `ProvenanceClass::ExcelTrusted`. Write formulas WITH cached `<v>` via `Formula::set_result` (the reconcile oracle — see compiler `Cargo.toml:86-90` note). Residual `fullCalcOnLoad=1` staleness is demoted by recipe 2.
2. **Trusted-fixture test override:** compile via `compile_workbook_with_fixture_override` (`#[cfg(test)]`-only); demotes `fullCalcOnLoad` to Warning. CANNOT soften non-Excel-identity refusal — still needs recipe 1's identity.

**Provenance-override marker (clone `tax-calc.provenance-override.json` verbatim, retarget `fixture`):**
```json
{
  "kind": "trusted-fixture",
  "fixture": "loan-calc.xlsx",
  "reason": "authored by rust_xlsxwriter ... honoured ONLY on the test/dev path ...",
  "authored_by": "rust_xlsxwriter",
  "scope": "test-path-only"
}
```

**Loan workbook content (D-01/D-02):** fixed-cell DAG; divergence via VLOOKUP/INDEX-MATCH rate-tier table + IFERROR + nested IF + ROUND/CEILING. NO PMT/POWER/`^`. MULTIPLE named outputs (e.g. `monthly_payment`, `total_interest`, `applied_rate`) — NO privileged single output (Anti-Pattern). Use the `out_*` named-range convention `promote_named_outputs` expects (see `lib.rs:264-269` doc) and blue-font-input / colour-role palette from `pmcp-workbook-dialect/src/lib.rs:43-45` (`FF0000FF` input, `FFE2EFDA` constant). Add an explicit `pmcp_dialect_version` cell here to exercise WBDL-02 present-path.

---

### `crates/pmcp-workbook-compiler/src/reemit_loan.rs` (test/integration proof — NEW, `#[cfg(test)]`) — WBEX-01

**Analog:** `crates/pmcp-workbook-compiler/src/reemit_golden.rs` (full file — clone structure, swap the oracle to the loan workbook's OWN values).

**Why in `src/` under `#[cfg(test)]` (CR-01, reemit_golden.rs:1-14):** the fixture override is `#[cfg(test)]`-only; an external `tests/` crate can't see it. The proof MUST live INSIDE the crate.

**Compile helper (mirror `reemit_golden.rs:54-72`):**
```rust
fn compile_fixture() -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_fixture(scratch.path());   // copies committed loan-calc.xlsx
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(&xlsx, &out_root, "loan-calc", "1.0.0", "proof-approver")
        .expect("compile the loan fixture via the trusted-fixture override");
    (scratch, out_root.join("loan-calc@1.0.0"))
}
```

**Serve gate — assert ITS OWN manifest (the generalization proof, WBEX-01).** Mirror `reemit_golden.rs:178-192` `structural_eq_check4_loads_via_toolkit` but assert the LOAN's own inputs/outputs (compiler already dev-deps `pmcp-server-toolkit` with `workbook` feature, `Cargo.toml:97`):
```rust
use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource};
let loaded = load_bundle(&LocalDirSource::new(&bundle)).expect("loads via toolkit");
assert_eq!(loaded.stamp.bundle_id, "loan-calc");
// assert loaded.cell_map.inputs / .outputs reflect the LOAN's own cells
// (NOT tax-calc's 3 inputs / 4 outputs) — the manifest behind the 5 generic
// tool names differs; THAT difference is the generalization proof.
```
Also clone the seven-member-present check (`reemit_golden.rs:124-142`), BUNDLE.lock recompute via `build_bundle_lock` (`reemit_golden.rs:144-175`), and the production-refusal counter-test (`reemit_golden.rs:239-252`) asserting bare `compile_workbook` (Enforce) refuses the same bytes. **Note:** structural equality vs a golden does NOT apply here (no loan golden exists) — assert on intrinsic structure (member presence, lock recompute, named-output count/dtype) and the serve manifest, not equality to a second file.

For a live `tools/list`/`get_manifest` parity assertion, the optional `mcp-tester` harness exists (`crates/pmcp-workbook-server/Cargo.toml` parity precedent) — but the direct `load_bundle` cell_map assertion above already satisfies the gate without a server boot.

---

### `tests/fixtures/quirks/*.xlsx` (~7-9 fixtures — NEW) — WBEX-02

**Analog:** the `tax-calc.xlsx` authoring recipe + per-fixture `*.provenance-override.json` (same as loan above).

**D-09 corpus (~7-9):** 4 named — 1900-leap-year (scope as **serial-number arithmetic over `f64`**, NOT DATE() — Pitfall 3 / Open Question 1; spike a fixture, fall back to `scalar_eval`-only or a documented limitation if not expressible without date code), empty-cell coercion, error propagation, half-rounding boundaries — PLUS curated: text→number coercion, explicit `#DIV/0!` propagation, float boundary (0.1+0.2), negative-value rounding sign. Each a tiny `.xlsx` with cached cell values as the oracle. Each needs an Excel-identity author + override marker (same landmine as loan).

---

### `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs` (test/integration — NEW, `#[cfg(test)]`) — WBEX-02

**Analog:** `reemit_golden.rs` compile path (the trusted-fixture override) + `reconcile::within_tol`/`reconcile` (`reconcile/mod.rs:35-53`).

**Mechanism:** for each quirk fixture, compile via `compile_workbook_with_fixture_override`, then verify the executor's computed value reconciles to the cached oracle within `TOL = 0.01` through the REAL penny path. Reuse the `ComparisonMap` builder (`reconcile/mod.rs:70-90` `.with(cell_key, target)` / `.with_value(...)`) and `within_tol`:
```rust
use crate::reconcile::{within_tol, TOL};   // TOL = 0.01 (reconcile/mod.rs:35)
// within_tol: NUMBERS via ±0.01 penny; non-numeric STRUCTURALLY (reconcile/mod.rs:43-53)
```
**Anti-pattern:** exact-float `==` on money is FORBIDDEN repo-wide — always go through `within_tol`.

---

### `crates/pmcp-workbook-runtime/src/scalar_eval.rs` (test/unit — MODIFY) — WBEX-02

**Analog:** same file — existing `mod tests` (`scalar_eval.rs:262-332`); extend it.

**Existing quirk-adjacent tests to mirror:** `div_by_zero_matches_kernel_nan_clamped_to_zero` (`:285-300`), `error_leaf_short_circuits_above_arithmetic` (`:315-332`). Add a `num(n)` / `CellEnv::new().with_value(...)` driven `#[test]` per quirk (D-08 layer 1).

**For the half-rounding quirk, assert against the SOURCE OF TRUTH** (`crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs:30-41`), NOT a naive round:
```rust
use crate::sheet_ir::rounding::excel_round;
assert_eq!(excel_round(1594.925, 2), 1594.93);   // NOT 1594.92 (binary-f64 half boundary)
```
This same `excel_round`/`excel_roundup`/`excel_ceiling` helper set is what the loan workbook (D-02) and quirk corpus (D-09) both lean on — the two gates reinforce each other.

---

### Fuzz target + property tests (NEW) — ALWAYS requirement (CLAUDE.md)

**Property analog:** `version.rs:241-257` `declared_version_round_trips_over_a_semver_grid` (exhaustive grid, no new dep) — adapt for the D-04 semver-compat decision. **Fuzz:** new robustness surface = the `dialect_version` `MAJOR.MINOR` string parse; add a fuzz target feeding arbitrary strings (must never panic; malformed → typed error). **Example:** a runnable demonstration of `cargo pmcp new --kind workbook-server` output (or the scaffolded crate running and serving 5 tools).

---

## Shared Patterns

### Path-traversal validation (V5)
**Source:** `cargo-pmcp/src/commands/new.rs:116-140` (`validate_crate_name`)
**Apply to:** the new `execute_workbook_server` arm — call it FIRST, before any `fs::write`. Do NOT reimplement.

### Single-cell defined-name read over the owned `WorkbookMap`
**Source:** `crates/pmcp-workbook-compiler/src/version.rs:62-95` (`declared_version_cell` + `cell_value_for_key`)
**Apply to:** the new `dialect_version.rs` reader — copy the scan, change the name const, INVERT the None-handling (absent → baseline, not error).

### Typed fail-closed compile error
**Source:** `crates/pmcp-workbook-compiler/src/version.rs:48-55` / `CompileError::Lint(format!(...))`
**Apply to:** the D-04 version-mismatch path (different major / newer → hard typed error, non-zero exit).

### Trusted-fixture authoring + override (the .xlsx landmine)
**Source:** `crates/pmcp-workbook-compiler/src/reemit_golden.rs:48-72` (`compile_workbook_with_fixture_override` recipe) + `tests/fixtures/tax-calc.provenance-override.json` (marker) + `crates/pmcp-workbook-compiler/Cargo.toml:81-91` (`rust_xlsxwriter` dev-dep + the `Formula::set_result` cached-oracle note)
**Apply to:** EVERY new fixture (loan + ~7-9 quirks). Author with `rust_xlsxwriter` carrying genuine Excel identity; pair each with a `*.provenance-override.json`; compile via the `#[cfg(test)]` override. NEVER author with umya (refused). NEVER edit existing fixtures (D-05 zero-churn).

### Penny reconcile (±0.01), never exact-float on money
**Source:** `crates/pmcp-workbook-compiler/src/reconcile/mod.rs:35-53` (`TOL`, `within_tol`)
**Apply to:** all quirk reconcile fixtures and any money assertion.

### Excel rounding source-of-truth
**Source:** `crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs:30-60` (`excel_round`/`excel_roundup`/`excel_ceiling`)
**Apply to:** the half-rounding quirk assertions AND the loan workbook's currency rounding expectations.

### Drift-binding test (doc ↔ const)
**Source:** `crates/pmcp-workbook-dialect/src/lib.rs:237-342` (`dialect_spec` module) + `lib.rs:163-182` (canary)
**Apply to:** the new `SUPPORTED_/BASELINE_DIALECT_VERSION` const ↔ `docs/workbook-dialect-spec.md` version section. Reuse the published-package SKIP / in-repo FAIL `.git` guard verbatim.

### Emitted-source drift-lock golden test
**Source:** `cargo-pmcp/src/templates/sql_server.rs:285-339` (`wiring_lines` + `emitted_main_matches_example_modulo_setup`)
**Apply to:** the scaffolded `main.rs` ↔ `workbook_server_http.rs` example.

### Purity-safe served-tree dependency posture
**Source:** `crates/pmcp-workbook-server/Cargo.toml` (`default-features = false, features = ["workbook"...]` with the T-95-06 rationale comment)
**Apply to:** the scaffolded `Cargo.toml` — `default-features = false, features = ["workbook-embedded", "http"]`. Never leave `code-mode` defaults on.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | Every Phase 96 file maps onto a proven in-repo analog. The only genuinely new LOGIC is the ~30-line D-04 semver-compat decision (no analog for the decision itself, but `version.rs`'s crate-free semver parse + the typed-error pattern cover its shape) and the fuzz target for the version-string parse (new robustness surface, but the `version.rs` round-trip property covers the property layer). |

## Metadata

**Analog search scope:** `cargo-pmcp/src/{commands,templates}/`, `crates/pmcp-workbook-compiler/src/` + `tests/fixtures/`, `crates/pmcp-workbook-dialect/src/`, `crates/pmcp-workbook-runtime/src/{scalar_eval.rs,sheet_ir/}`, `crates/pmcp-server-toolkit/{examples,Cargo.toml}`, `crates/pmcp-workbook-server/Cargo.toml`, `docs/`
**Files scanned (read in full or targeted):** new.rs, version.rs, sql_server.rs, pmcp-workbook-dialect/lib.rs, reemit_golden.rs, workbook_server_http.rs, reconcile/mod.rs, scalar_eval.rs (+tests), rounding.rs, compiler lib.rs (driver), templates/mod.rs, both Cargo.tomls, provenance marker + gate
**Pattern extraction date:** 2026-06-14
