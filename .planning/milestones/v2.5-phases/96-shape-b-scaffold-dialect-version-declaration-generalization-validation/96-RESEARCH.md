<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**A. Second workbook — WBEX-01 generalization gate**
- **D-01:** The second example workbook models a **loan/mortgage payment calculator** — deliberately different inputs/outputs from `tax-calc` so the served `get_manifest`/`tools/list` schema is visibly its own. It MUST be a **fixed-cell formula DAG** (no arbitrary-N row iteration — deferred in v2.3).
- **D-02:** Divergence achieved with **whitelist-legal lookup families** the 13-fn whitelist provides — a rate-tier table driven by **VLOOKUP / INDEX-MATCH** (e.g. credit-score or LTV band → rate), **IFERROR** guards, **nested IF** tiering, and **ROUND / CEILING** to currency. NO PMT, NO POWER, NO exponentiation (`(1+r)^n` amortization is NOT expressible — it needs the deferred iteration). Genuinely divergent formula coverage, fully whitelist-legal, quirk-rich.

**B. Dialect-version declaration (WBDL-02)**
- **D-03:** Version lives in a **reserved named-range (working name `pmcp_dialect_version`) or a config-sheet cell INSIDE the .xlsx** — self-describing, travels with the workbook. The compiler reads it during ingest. Chosen over a `pmcp.toml` field.
- **D-04:** **Semver-compatible, fail-closed.** Same MAJOR = compatible (compiler accepts declared minor ≤ its supported minor). Different major OR declared version newer than supported → **hard, typed compile error** (fail-closed).
- **D-05:** **Absent declaration → baseline version, no error.** A workbook with no version cell (the existing `tax-calc` golden + every Phase 91/92/93/95 fixture) is treated as targeting the **baseline/oldest-supported dialect (e.g. 1.0)** and compiles normally. Compiler MAY emit a **non-fatal advisory** to add an explicit cell. Existing fixtures keep working with **zero edits** — no churn this phase.

**C. Shape B scaffold (WBCL-05)**
- **D-06:** `cargo pmcp new --kind workbook-server` ships Cargo.toml + `main.rs` (using `EmbeddedSource` / the `workbook-embedded` feature — inverse of Shape A's LocalDirSource) + sample `pmcp.toml` + the **source .xlsx** + a **pre-compiled embedded bundle**. `cargo run` works immediately; the dev can edit the workbook → `cargo pmcp workbook compile` → rerun. Mirrors the `--kind sql-server` dispatch + template-module pattern.
- **D-07:** Sample workbook = **reuse the existing `tax-calc` golden** (its bundle already exists). The **only newly-authored .xlsx this phase is the loan workbook** (D-01).

**D. Excel-quirk corpus (WBEX-02)**
- **D-08:** Encode the corpus as **scalar_eval unit tests** (fast, pinpoint coverage) **AND mini reconcile fixtures** (each quirk a tiny workbook with cached cell values as the oracle, run through the **real penny-reconcile path**). The reconcile fixtures literally satisfy "verifies reconcile determinism."
- **D-09:** Cover the **four roadmap-named quirks** (1900 leap-year, empty-cell coercion, error propagation, half-rounding boundaries) PLUS a curated set (text→number coercion, explicit `#DIV/0!` propagation, a float boundary such as 0.1+0.2, negative-value rounding sign). Capped **~7–9 total**.

### Claude's Discretion
- Exact reserved named-range identifier for D-03, the baseline version string/number for D-05, the scaffolded crate's package name and precise file layout, the loan workbook's exact rate-tier table contents and input/output cell names, the precise WBEX-02 fixture file locations — all left to research/planning provided they honor the locked decisions above.

### Deferred Ideas (OUT OF SCOPE)
- None added this phase. Deferred-by-design v2.3 items remain deferred: **row-block iteration / arbitrary-N loops**, **capability cells** (Rust/remote/MCP escape hatches), **named-range-backed validation lists**, **S3/registry bundle store**. The second workbook MUST stay within the constrained fixed-cell DAG. Does NOT touch `pmcp-code-mode`. Does NOT re-open the compiler pipeline, runtime, or purity gate (Phases 91–93, stable).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WBCL-05 | `cargo pmcp new --kind workbook-server` scaffolds a thin binary over `BundleSource`/`EmbeddedSource` + the served-tool toolkit module (Shape B). | §Scaffold pattern (`new.rs` dispatch + `sql_server.rs` template module); §EmbeddedSource wiring (the `workbook_server_http` example IS the canonical `main.rs`); §toolkit feature `workbook-embedded`. Sample bundle = committed `tax-calc@1.1.0` (D-07). |
| WBDL-02 | A workbook declares the dialect version it targets; the compiler validates it (fail-closed, forward-compatible). | §Version declaration. **Critical:** a `read_workbook_version` accessor already exists (Phase 94) but reads the BUNDLE version (`version`/`wb_version` named-range, fail-on-absent) — WBDL-02 needs a SEPARATE `pmcp_dialect_version` declaration with semver-compat policy and absent→baseline. §Ingest integration point + §typed-error pattern. |
| WBEX-01 | A second, non-lighthouse workbook compiles and serves end-to-end; its `get_manifest`/`tools/list` reflects its OWN inputs (no per-workbook Rust, no privileged output). | §Loan workbook authoring; §end-to-end serve gate (mirror `reemit_golden.rs` in-crate proof + a toolkit serve test). Runtime semantics already dispatch VLOOKUP/INDEX/MATCH/ROUND/CEILING/IFERROR. |
| WBEX-02 | Excel-quirk fixture corpus verifies reconcile determinism beyond the single golden case. | §Quirk corpus (two-layer: `scalar_eval` unit tests + mini reconcile fixtures through `reconcile::reconcile`/`within_tol`). §Landmine: NO 1900-leap-year/date-serial code exists today — scope the date quirk as serial-arithmetic, not DATE(). |
</phase_requirements>

# Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation - Research

**Researched:** 2026-06-14
**Domain:** Rust workspace tooling (cargo subcommand scaffolding), Excel-workbook compiler ingest, deterministic spreadsheet evaluation, fixture-based reconcile testing
**Confidence:** HIGH (all findings verified against in-repo source; no external dependency research needed — this is a pure in-repo extension of stable Phases 91–95)

## Summary

Phase 96 closes the v2.3 milestone with three independent deliverables over a **frozen, stable** runtime + compiler + toolkit stack. Two of the three are near-mechanical mirrors of existing, proven patterns: the **Shape B scaffold (WBCL-05)** clones the `cargo pmcp new --kind sql-server` template-module pattern, and its emitted `main.rs` is effectively the already-shipped `workbook_server_http` toolkit example (EmbeddedSource over an `include_dir!`'d bundle). The **second-workbook gate (WBEX-01)** reuses the existing `compile_workbook` driver and the `WorkbookBuilderExt::try_with_workbook_bundle` serve path — the runtime's `semantics::apply` already dispatches every dialect function the loan workbook needs (VLOOKUP/INDEX/MATCH/ROUND/CEILING/IFERROR/nested IF).

The genuinely novel work is **WBDL-02 (dialect-version declaration)** and the **two authoring landmines** that gate WBEX-01 and WBEX-02. WBDL-02 must NOT be confused with the existing `read_workbook_version` accessor (Phase 94): that reads the *bundle* version from a `version`/`wb_version` named range and **fails on absent**, whereas WBDL-02 needs a *separate* `pmcp_dialect_version` declaration with a semver-compatibility check (D-04) and **absent → baseline, no error** (D-05). They share the defined-name-reading machinery but have opposite absence semantics.

The two landmines: (1) **authoring a new .xlsx is non-trivial** — the repo has NO in-repo .xlsx generator; `tax-calc.xlsx` is a committed binary authored externally by `rust_xlsxwriter` carrying a *genuine* `<Application>Microsoft Excel</Application>` identity, and the production freshness/provenance gate REFUSES non-Excel-provenanced workbooks. The loan workbook (D-01) and the quirk fixtures (D-09) must either carry genuine Excel provenance or ride the `#[cfg(test)]`-only `compile_workbook_with_fixture_override` / `FreshnessPolicy::TrustedFixture` path. (2) **There is no 1900-leap-year / date-serial handling in the runtime today** — Excel dates are bare `f64` serials and DATE() is not whitelisted, so the "1900 leap-year" quirk (D-09) must be scoped as serial-number arithmetic semantics, not a date function.

**Primary recommendation:** Plan three loosely-coupled work streams. (1) WBCL-05: clone `sql_server.rs` → `workbook_server.rs` template module; emitted `main.rs` = the `workbook_server_http` example wiring; carry the committed `tax-calc@1.1.0` bundle bytes + the committed `tax-calc.xlsx` + a sample `pmcp.toml` into the scaffold. (2) WBDL-02: add a `pmcp_dialect_version` reader to the compiler ingest (mirroring `version.rs` but with absent→baseline + a new `semver`-style compatibility check + typed `CompileError`), extend `docs/workbook-dialect-spec.md` with a version-policy section, and add a doc↔const binding-style drift guard for the supported-version constant. (3) WBEX-01/02: author the loan `.xlsx` + the ~7–9 quirk `.xlsx` fixtures (use the `rust_xlsxwriter`-with-Excel-identity recipe + the trusted-fixture test override), wire the loan workbook through an in-crate compile-and-serve proof (mirror `reemit_golden.rs`), and encode each quirk as both a `scalar_eval` unit test and a mini reconcile fixture through `reconcile::reconcile`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `--kind workbook-server` dispatch + crate emission | CLI (`cargo-pmcp`) | — | Mirrors `execute_sql_server` in `new.rs`; pure filesystem-write, no runtime. |
| Scaffolded `main.rs` (EmbeddedSource wiring) | Generated binary (downstream consumer) | Toolkit (`pmcp-server-toolkit[workbook-embedded]`) | The emitted code is a thin shell over `WorkbookBuilderExt`; the toolkit owns all served logic. |
| `pmcp_dialect_version` read + semver-compat validation | Compiler (`pmcp-workbook-compiler` ingest/stage1) | Dialect (`pmcp-workbook-dialect` — owns supported-version const + spec) | Compiler is the only crate that reads `.xlsx` (umya isolation); the dialect crate owns the version contract surface (parallel to WHITELIST). |
| Loan workbook formula evaluation | Runtime (`pmcp-workbook-runtime` `sheet_ir::semantics`) | — | `apply()` already dispatches all 13 whitelisted fns; no new runtime code. |
| Loan workbook compile→serve end-to-end | Compiler (compile) + Toolkit (serve) | Runtime (executor + manifest projection) | Generalization gate exercises the full existing pipeline with zero per-workbook Rust. |
| Quirk reconcile fixtures | Compiler (`reconcile::reconcile` / `within_tol`) | Runtime (`scalar_eval` for the unit-test layer) | Reconcile is compiler-owned (grades IR against cached oracle); the scalar layer is runtime-owned. |
| `.xlsx` fixture authoring | Test/dev harness (`rust_xlsxwriter` dev-dep) | Compiler (`FreshnessPolicy::TrustedFixture` test override) | Authoring is a build/test concern; production never authors `.xlsx`. |

## Standard Stack

This phase adds **no new external dependencies** to the production tree. All work is in-repo extension. The relevant existing stack:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `umya-spreadsheet` | 3.0 | `.xlsx` reader (compiler-only) | Already the compiler's reader; purity-gated OUT of the served tree. [VERIFIED: compiler Cargo.toml line 69] |
| `rust_xlsxwriter` | 0.95 (`default-features = false`) | `.xlsx` writer — runtime renderer AND the test-only fixture author | Pure writer, pulls only `zip`; the only sanctioned way to author a fixture `.xlsx` in-repo. [VERIFIED: compiler Cargo.toml line 91, runtime render/mod.rs] |
| `clap` | 4 (`derive`, `env`) | CLI arg parsing for scaffolded crates | Existing scaffold + binary convention. [VERIFIED: sql_server.rs, workbook-server Cargo.toml] |
| `include_dir` | 0.7.4 | Bakes the embedded bundle into the scaffolded binary | The `workbook-embedded` feature's mechanism; already used by `workbook_server_http` example. [VERIFIED: toolkit Cargo.toml `workbook-embedded` feature] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tempfile` | 3 | Scratch dirs for compile-and-serve / reconcile fixture tests | All in-crate compile proofs use `TempDir`. [VERIFIED: reemit_golden.rs] |
| `proptest` | 1 | Property tests (ALWAYS requirement for the version-compat policy) | The semver-compat decision and version round-trip are natural property targets. [VERIFIED: version.rs round-trip test, project CLAUDE.md ALWAYS] |
| `serde_json` | 1 | Manifest/IR JSON equality assertions in the serve gate | Mirrors `reemit_golden.rs` structural-equivalence checks. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| A new `semver` crate for D-04 compat | Hand-rolled major/minor parse over `&str` | The dialect version is a constrained `MAJOR.MINOR[.PATCH]` and the policy is "same major, declared minor ≤ supported." A 2-field integer parse is ~15 lines and adds no dependency to a purity-gated crate. **Recommend hand-roll** (the existing `version.rs` already parses semver-shaped strings without a crate); confirm with the planner. [ASSUMED — needs confirmation] |
| Authoring fixtures with `umya` | `rust_xlsxwriter` | A umya-authored fixture is REFUSED by the provenance gate (it fabricates Excel identity). MUST use `rust_xlsxwriter` + the test override. [VERIFIED: provenance-override.json, reemit_golden.rs header] |

**Installation:** No `cargo add` needed for production. `rust_xlsxwriter` (0.95) and `tempfile`/`proptest` are already dev-dependencies of the compiler crate. If the loan/quirk fixtures are authored in a NEW location (e.g. a `gen_fixtures` test or example), that crate's `[dev-dependencies]` must include `rust_xlsxwriter = { version = "0.95", default-features = false }`.

## Package Legitimacy Audit

> No new external packages are introduced by this phase. All packages below are already in the workspace tree and were vetted in prior phases (91–95).

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `umya-spreadsheet` | crates.io | mature (3.x) | established | github.com/MathNya/umya-spreadsheet | not re-run (existing dep) | Approved (compiler-only, purity-gated) |
| `rust_xlsxwriter` | crates.io | mature (0.95) | established | github.com/jmcnamara/rust_xlsxwriter | not re-run (existing dep) | Approved (writer-only) |
| `include_dir` | crates.io | mature (0.7.4) | established | github.com/Michael-F-Bryan/include_dir | not re-run (existing dep) | Approved (embedded feature) |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*slopcheck was not re-run because this phase introduces zero new external packages — every package is a pre-existing, prior-phase-vetted workspace dependency. If the planner elects to add a `semver` crate for D-04 (NOT recommended — see Alternatives), run the Package Legitimacy Gate on it before install.*

## Architecture Patterns

### System Architecture Diagram

```
WBCL-05 (scaffold)                    WBDL-02 (version decl)              WBEX-01/02 (generalization)
─────────────────                     ──────────────────                  ──────────────────────────

cargo pmcp new                        author loan.xlsx /                  author loan.xlsx + quirk*.xlsx
  --kind workbook-server                quirk.xlsx                          (rust_xlsxwriter + Excel identity)
       │                                  │  (declares                            │
       ▼                                  │   pmcp_dialect_version cell)           ▼
new.rs::execute                           ▼                              ┌─ compile_workbook[_with_fixture_override]
  match kind {                    compile_workbook                       │     │
   "workbook-server" =>             │                                    │     ▼ (1) umya ingest → WorkbookMap
     execute_workbook_server }      ▼ ingest::ingest → WorkbookMap       │     ▼ (2) stage1: lint + synth + freshness
       │                            │                                    │     ▼ (3) parse + Kahn DAG
       ▼                      [NEW] read_dialect_version(map)            │     ▼ (4) executor (sheet_ir::semantics::apply)
templates/workbook_server.rs        │   - scan defined_names for         │     ▼ (5) reconcile::reconcile vs cached oracle  ◄── WBEX-02
  generate():                       │     `pmcp_dialect_version`          │     ▼ (6) seven-member bundle emit
   Cargo.toml (workbook-embedded)   │   - absent → BASELINE (D-05)       │
   src/main.rs (EmbeddedSource)     │   - present → semver compat        ▼
   pmcp.toml (sample)               │     check vs SUPPORTED const       WorkbookBuilderExt::try_with_workbook_bundle
   workbook/tax-calc.xlsx (D-07)    │       same major & ≤ supported     │   (boot integrity gate: BUNDLE.lock recompute)
   bundle/tax-calc@1.1.0/* (D-07)   │       → accept                     ▼
       │                            │     else → typed CompileError      Server (5 tools) → tools/list / get_manifest  ◄── WBEX-01
       ▼                            │       (fail-closed, D-04)              reflects loan's OWN inputs
  cargo run → EmbeddedSource        ▼
  serves 5 tools immediately    [NEW] supported-version const in
                                  pmcp-workbook-dialect + spec-doc
                                  drift guard (parallel to WHITELIST)
```

### Recommended Project Structure
```
cargo-pmcp/src/
├── commands/new.rs              # add "workbook-server" arm to the match (line ~70)
└── templates/
    ├── mod.rs                   # add `pub mod workbook_server;`
    └── workbook_server.rs       # NEW — clone of sql_server.rs

crates/pmcp-workbook-compiler/src/
├── dialect_version.rs           # NEW — read_dialect_version + semver compat (mirror version.rs)
│                                #   OR add to existing version.rs / ingest path
└── lib.rs                       # wire read_dialect_version into compile_workbook_inner before/at stage1

crates/pmcp-workbook-dialect/src/
└── lib.rs                       # add SUPPORTED_DIALECT_VERSION const + a spec-doc drift guard

docs/
└── workbook-dialect-spec.md     # add §"Dialect version declaration & compatibility policy"

crates/pmcp-workbook-compiler/tests/fixtures/   # OR a new committed-fixtures location
├── loan-calc.xlsx               # NEW (D-01) — rust_xlsxwriter-authored, Excel identity
├── loan-calc.provenance-override.json  # NEW — trusted-fixture marker (mirror tax-calc's)
└── quirks/                      # NEW (D-09) — ~7–9 tiny .xlsx + override markers
    ├── leap1900.xlsx
    ├── empty-coercion.xlsx
    └── ...
crates/pmcp-workbook-compiler/src/
├── reemit_loan.rs               # NEW #[cfg(test)] — loan compile-and-serve proof (mirror reemit_golden.rs)
└── quirks_reconcile.rs          # NEW #[cfg(test)] — mini reconcile fixtures through reconcile::reconcile

crates/pmcp-workbook-runtime/src/scalar_eval.rs   # add quirk unit tests (D-08 layer 1)
```

### Pattern 1: Single-crate `--kind` scaffold (clone of sql-server)
**What:** Add an arm to `new.rs::execute`'s `match kind.as_deref()` and a template module that emits one runnable crate via raw `fs::write` (NO template engine — `format!` with `{{`/`}}` escaping).
**When to use:** WBCL-05.
**Example:**
```rust
// Source: cargo-pmcp/src/commands/new.rs:70-80 (VERIFIED in-repo)
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
The `execute_workbook_server` fn calls `validate_crate_name(name)?` FIRST (path-traversal guard, line 116), then `fs::create_dir_all(...src)`, then `templates::workbook_server::generate(dir, name)?`.

### Pattern 2: Emitted `main.rs` == the existing example (drift-locked)
**What:** The scaffold's emitted `main.rs` should be the EmbeddedSource wiring from `crates/pmcp-server-toolkit/examples/workbook_server_http.rs`, with a golden test asserting it matches modulo harness lines (exactly the `sql_server.rs` `emitted_main_matches_example_modulo_setup` pattern).
**When to use:** WBCL-05.
**Example:**
```rust
// Source: crates/pmcp-server-toolkit/examples/workbook_server_http.rs:65-78 (VERIFIED)
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundle");
// ...
let builder = Server::builder().name("...").version("...");
let builder = builder.try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))?;
let server = builder.build()?;
let (addr, handle) = serve(server).await?;
println!("PMCP_WORKBOOK_SERVER_ADDR=http://{addr}");
```
NOTE the import is from the TOOLKIT (`pmcp_server_toolkit::workbook::{EmbeddedSource, WorkbookBuilderExt}`), never naming `pmcp-workbook-runtime` (D-11 carry-forward from Phase 92/95). The scaffold's `Cargo.toml` pins `pmcp-server-toolkit = { version = "...", default-features = false, features = ["workbook-embedded", "http"] }` (mirror `pmcp-workbook-server/Cargo.toml`'s `default-features = false` posture — `code-mode` defaults leak `pmcp-code-mode` into the served tree and trip the purity gate).

### Pattern 3: Dialect-version reader (mirror `version.rs`, INVERT absence policy)
**What:** A read-only accessor over the ingested `WorkbookMap` that scans `defined_names` for `pmcp_dialect_version` (D-03 working name), reads the target single cell's cached value, and applies the D-04 compatibility check — but with **absent → baseline (D-05)** rather than `version.rs`'s **absent → error**.
**When to use:** WBDL-02.
**Example (adapt from the EXISTING `version.rs` `declared_version_cell`):**
```rust
// Source: crates/pmcp-workbook-compiler/src/version.rs:62-95 (VERIFIED — reuse the scan,
// change the name constant and the None-handling)
fn declared_dialect_version(map: &WorkbookMap) -> Option<String> {
    for dn in &map.defined_names {
        if !dn.name.eq_ignore_ascii_case("pmcp_dialect_version") { continue; }
        if dn.target.start != dn.target.end { continue; }      // single-cell only
        if let Some(v) = cell_value_for_key(map, &dn.target.sheet, &dn.target.start) {
            let t = v.trim();
            if !t.is_empty() { return Some(t.to_string()); }
        }
    }
    None
}
// D-05: None → use BASELINE_DIALECT_VERSION (no error); MAY push a Severity::Info advisory.
// D-04: Some(v) → parse major.minor, compare to SUPPORTED_DIALECT_VERSION:
//   same major && declared_minor <= supported_minor  → accept
//   else (different major | newer)                   → Err(CompileError::Lint(...))  fail-closed
```
**Integration point:** `compile_workbook_inner` (lib.rs:240) reads bytes (step 1) and ingests (step 2) producing `map`, then runs `stage1` (step 3). The dialect-version check is naturally a **stage-1 collect-all finding** — emit it alongside lint/freshness so a version mismatch is reported in the same collect-all refuse pass (D-01 collect-all ethos). Alternatively a standalone gate immediately after ingest. Confirm placement with the planner; the collect-all stage-1 path is more consistent with the milestone's "fix one re-save" ethos.

### Pattern 4: Authoring a fixture .xlsx that passes the freshness gate
**What:** The production freshness/provenance gate (Phase 93 WBCO-07) REFUSES any workbook whose cache lacks a genuine Excel identity. There are TWO sanctioned ways to get a fixture through:
1. **Genuine-Excel-identity author (what `tax-calc.xlsx` does):** author with `rust_xlsxwriter` setting `DocProperties` so the cache carries `<Application>Microsoft Excel</Application>` + an `<AppVersion>` + a non-sentinel calcId → classifies `ProvenanceClass::ExcelTrusted`. Its only residual problem is `fullCalcOnLoad=1` (staleness).
2. **Trusted-fixture test override:** compile via the `#[cfg(test)]`-only `compile_workbook_with_fixture_override` (`FreshnessPolicy::TrustedFixture`), which DEMOTES the `fullCalcOnLoad` staleness to a Warning. It CANNOT soften the fabricated-/non-Excel-identity refusal — that still requires the genuine identity from (1).
**When to use:** WBEX-01 (loan workbook) + WBEX-02 (quirk fixtures).
**Example:**
```rust
// Source: crates/pmcp-workbook-compiler/src/reemit_golden.rs (VERIFIED — the proven recipe)
compile_workbook_with_fixture_override(&xlsx, &out_root, "loan-calc", "1.0.0", "proof-approver")
    .expect("compile the loan fixture via the trusted-fixture override");
```
Each committed fixture pairs with a `*.provenance-override.json` marker (mirror `tax-calc.provenance-override.json`).

### Pattern 5: Two-layer quirk encoding (D-08)
**What:** Each quirk is encoded BOTH as a fast `scalar_eval` unit test (runtime crate) AND as a mini reconcile fixture (compiler crate) run through the real `reconcile::reconcile` + `within_tol` penny path.
**When to use:** WBEX-02.
**Example (reconcile layer):**
```rust
// Source: crates/pmcp-workbook-compiler/src/reconcile/mod.rs (VERIFIED — within_tol is the
// ±0.01 penny oracle; a quirk fixture's cached value is the target)
pub const TOL: f64 = 0.01;
pub fn within_tol(computed: &CellValue, target: &CellValue) -> bool { /* ±0.01 numbers, structural else */ }
// A quirk fixture: a tiny .xlsx whose cached cell value (the oracle) encodes the
// expected Excel result; the executor recomputes it; reconcile must reconcile within TOL.
```

### Anti-Patterns to Avoid
- **Conflating the dialect version with the bundle version.** `version.rs::read_workbook_version` reads `version`/`wb_version` (the BUNDLE version, used by Phase 94 CLI compile) and **errors on absent**. WBDL-02 is a DIFFERENT declaration (`pmcp_dialect_version`) with **absent→baseline**. Do NOT reuse `read_workbook_version` for WBDL-02 or edit its absence semantics (it would break Phase 94). Add a sibling reader.
- **Authoring fixtures with umya or editing existing fixtures.** D-05 mandates ZERO edits to existing fixtures. New fixtures MUST be `rust_xlsxwriter`-authored with genuine Excel identity (umya-authored fixtures are refused).
- **A privileged single output in the loan manifest.** WBEX-01 success requires multiple named outputs with no headline (mirror tax-calc's 4 outputs). The loan workbook should emit e.g. `monthly_payment`, `total_interest`, `applied_rate` — not one "answer."
- **Re-implementing date/leap-year logic in the runtime.** No date-serial code exists; DATE() is not whitelisted. The 1900-leap quirk must be serial-arithmetic semantics over `f64`, not a new date function (which would be scope creep into a non-whitelisted area).
- **Leaving `code-mode` defaults on in the scaffold Cargo.toml.** Trips the purity gate (T-95-06). Use `default-features = false, features = ["workbook-embedded", "http"]`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reading a single-cell named-range value from `.xlsx` | A new umya scan | `version.rs`'s `defined_names` + `cell_value_for_key` scan (copy the 30-line pattern) | Already proven, single-cell-guarded, case-insensitive, trim-aware. [VERIFIED] |
| Emitting a scaffold's `main.rs` | New wiring | The `workbook_server_http` example body + a drift-lock golden test | The example is the canonical EmbeddedSource wiring; a golden test (sql_server.rs pattern) prevents drift. [VERIFIED] |
| Penny reconcile of a quirk's expected value | A custom tolerance check | `reconcile::reconcile` / `within_tol` (TOL = 0.01) | Exact-float `==` on money is FORBIDDEN repo-wide; the penny path is the sanctioned oracle. [VERIFIED] |
| Excel rounding semantics (half-away-from-zero, ROUNDUP/CEILING magnitude) | Naive `(x*10^d).round()/10^d` | `sheet_ir::rounding::{excel_round, excel_roundup, excel_ceiling}` | The naive form is wrong at the binary-f64 half boundary (1594.925→1594.92 not .93); the helpers apply the epsilon correction. **This is itself the half-rounding quirk (D-09) — assert against these helpers.** [VERIFIED: rounding.rs] |
| Compile-and-serve proof structure | New test scaffolding | `reemit_golden.rs`'s 5-dimension structural-equivalence + boot-load check | Proven in-crate `#[cfg(test)]` pattern that reaches the trusted-fixture override. [VERIFIED] |
| Getting a fixture past the freshness gate | Disabling the gate / a production override | `compile_workbook_with_fixture_override` (`#[cfg(test)]`-only) | The override is CR-01-locked to test paths; production refusal is preserved. [VERIFIED] |

**Key insight:** Nearly every primitive this phase needs already exists and is battle-tested in Phases 91–95. The phase is primarily *composition + authoring*, not new infrastructure. The only genuinely new logic is the ~30-line semver-compat decision (D-04) and the spec-doc/version-const drift guard.

## Runtime State Inventory

> Phase 96 is greenfield/additive (new scaffold, new compiler accessor, new fixtures). It is NOT a rename/refactor/migration. No stored data, live-service config, OS-registered state, secrets, or build-artifact rename surfaces are touched. **None — verified: no existing data keyed on any string this phase renames; the new `pmcp_dialect_version` named range is additive and absent from all existing fixtures (D-05 guarantees zero edits).**

## Common Pitfalls

### Pitfall 1: The .xlsx authoring gap (NO in-repo generator)
**What goes wrong:** A planner assumes there is a "compile this .xlsx" or "generate a fixture" tool and writes a task that just "authors the loan workbook," then execution stalls because the only existing `.xlsx` (`tax-calc.xlsx`) is a committed binary with no in-repo producer.
**Why it happens:** The golden BUNDLE is generated programmatically (`fixture_gen.rs` builds the seven members from runtime Serialize types), but the SOURCE `.xlsx` is NOT — it was authored externally with `rust_xlsxwriter` + Excel doc-properties and committed as binary bytes.
**How to avoid:** The plan MUST include an explicit fixture-authoring task that (a) writes a `rust_xlsxwriter` author (a `#[cfg(test)]` helper, an example, or a small generator) setting `DocProperties` for genuine Excel identity, (b) commits the resulting `.xlsx` + a `*.provenance-override.json` marker, and (c) compiles it via `compile_workbook_with_fixture_override`. Budget real effort here — this is the critical path for both WBEX-01 and WBEX-02.
**Warning signs:** A task that says "add loan.xlsx" with no authoring mechanism; a compile that returns `CompileError::Lint("oracle/non-excel-app")` or a `fullCalcOnLoad` refusal.

### Pitfall 2: Dialect version vs bundle version confusion
**What goes wrong:** Reusing or editing `read_workbook_version` for WBDL-02, breaking Phase 94's CLI compile (which depends on `version`/`wb_version` + fail-on-absent).
**Why it happens:** Both are "version in a named cell"; the existing accessor is the obvious-looking hook.
**How to avoid:** Add a SIBLING accessor for `pmcp_dialect_version` with absent→baseline. Keep `read_workbook_version` untouched. Document the distinction in the spec.
**Warning signs:** Phase 94 CLI tests failing; a workbook with no version cell suddenly erroring on compile (violates D-05).

### Pitfall 3: The 1900 leap-year quirk has no date primitives
**What goes wrong:** A task tries to test `DATE(1900,2,29)` or date arithmetic, but DATE() is not in the WHITELIST and the runtime has no date-serial code.
**Why it happens:** "1900 leap-year" reads like a date-function test; in Excel it's a serial-number quirk (Excel treats 1900 as a leap year, so serial 60 = the phantom Feb 29 1900, shifting all post-1900-02-28 serials by 1).
**How to avoid:** Scope the quirk as **serial-number arithmetic determinism** — a fixture whose cached values encode the Excel-serial offset, asserting the executor reproduces Excel's serial behavior over bare `f64` (no new date function). If the runtime genuinely cannot express the quirk with whitelisted ops, document it as a `scalar_eval`-only assertion or an Open Question, rather than inventing date code (scope creep).
**Warning signs:** A task proposing to add `DATE`/`DATEVALUE` to the whitelist (would break the doc↔const binding + violate the deferred-functions boundary).

### Pitfall 4: Purity gate trip from scaffold/serve-tree dependencies
**What goes wrong:** The scaffolded crate (or its required-features example) pulls `pmcp-code-mode` (via toolkit default `code-mode`) or `umya` into the served tree, failing `make purity-check`.
**Why it happens:** Toolkit `default` features include `code-mode`; `EmbeddedSource` needs `workbook-embedded` not bare `workbook`.
**How to avoid:** Scaffold Cargo.toml = `default-features = false, features = ["workbook-embedded", "http"]` (mirror `pmcp-workbook-server/Cargo.toml`'s explicit posture). Confirm the new served cone is covered by the existing purity job per feature-combination.
**Warning signs:** `cargo tree` showing `pmcp-code-mode`/`umya`/`quick-xml`/`swc` under the scaffolded binary.

### Pitfall 5: Embedded bundle bytes drifting from the on-disk golden
**What goes wrong:** The scaffold copies bundle bytes that differ from the committed `tax-calc@1.1.0`, so the embedded server and the LocalDir server disagree (or the boot integrity gate fails on a tampered-looking bundle).
**Why it happens:** Duplicating bundle bytes into the template invites drift.
**How to avoid:** Carry the EXACT committed `@1.1.0` bytes; the `workbook_server_http` example deliberately `include_dir!`s STRAIGHT at the committed golden (no duplication). A scaffold golden test should assert the carried bytes match the committed source. Confirm the scaffold's `include_dir!` path resolves to the carried `bundle/` dir at scaffold-build time.
**Warning signs:** Boot integrity gate (`BUNDLE.lock` recompute) failing on `cargo run` of the scaffolded crate.

## Code Examples

### Reading a single-cell defined-name value (the WBDL-02 hook)
```rust
// Source: crates/pmcp-workbook-compiler/src/version.rs:62-95 (VERIFIED in-repo)
fn cell_value_for_key<'a>(map: &'a WorkbookMap, sheet: &str, addr: &str) -> Option<&'a str> {
    let sheet_rec = map.sheets.iter().find(|s| s.name == sheet)?;
    let cell = sheet_rec.cells.iter().find(|c| c.addr == addr)?;
    cell.value.as_deref()
}
```

### Dialect-function dispatch already present (no new runtime code for the loan workbook)
```rust
// Source: crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs:29-42 (VERIFIED)
pub fn apply(name: &str, args: &[EvalValue]) -> CellValue {
    match name {
        "VLOOKUP" => f_vlookup(args),
        "INDEX"   => f_index(args),
        "MATCH"   => f_match(args),
        "ROUND"   => f_round(args),
        "CEILING" => f_ceiling(args),
        "IFERROR" => f_iferror(args),
        // ... IF, SUM, SUMIF, ROUNDUP, ISNUMBER, SEARCH, TEXT
    }
}
```

### Excel half-rounding boundary (the D-09 half-rounding quirk's source of truth)
```rust
// Source: crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs:30-41 (VERIFIED)
pub fn excel_round(x: f64, digits: i32) -> f64 {
    if !x.is_finite() { return x; }
    let factor = pow10(digits);
    let scaled = x * factor;
    let nudged = scaled + scaled.signum() * (scaled.abs() * ROUND_EPSILON); // undo binary-f64 half error
    nudged.round() / factor
}
// Quirk assertion: excel_round(1594.925, 2) == 1594.93 (NOT 1594.92).
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded `build_reference_manifest` (per-workbook Rust) | Fully manifest-driven synth from colour/Guide/headers | Phase 93 (§5 fix) | WBEX-01 is the gate that PROVES this generalized — a second workbook must serve its OWN schema with zero shared Rust. |
| Naive abs-delta reconcile tolerance | Operand-anchored penny reconcile (`within_tol`, TOL=0.01) | Phase 93 | WBEX-02 fixtures plug into this exact path. |
| Single golden bundle (`tax-calc`) as the only correctness witness | Quirk corpus + second workbook | Phase 96 (this) | Determinism is proven beyond one case. |

**Deprecated/outdated:**
- Nothing deprecated this phase. The deferred-by-design items (row iteration, capability cells, named-range validation lists, registry store) remain deferred — do not pull them in.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Hand-rolling the semver-compat check (no `semver` crate) is preferred for D-04 | Standard Stack / Alternatives | Low — if a crate is wanted, run slopcheck; the constrained `MAJOR.MINOR` parse is trivial either way. |
| A2 | The dialect-version check belongs in the stage-1 collect-all pass (vs a standalone post-ingest gate) | Pattern 3 / integration point | Low — both work; collect-all is more consistent with the milestone ethos. Planner decides. |
| A3 | The 1900-leap quirk is expressible as serial-arithmetic over `f64` with whitelisted ops | Pitfall 3 | Medium — if NOT expressible, the date quirk may need to be `scalar_eval`-only or documented as a known limitation (NOT new date code). Verify during planning by attempting a fixture. |
| A4 | `BASELINE_DIALECT_VERSION` = "1.0" and `SUPPORTED_DIALECT_VERSION` are new consts owned by `pmcp-workbook-dialect` (parallel to WHITELIST) with a spec-doc drift guard | Pattern 3 / structure | Low — exact string is Claude's discretion (D-05); ownership in the dialect crate matches the WBDL-01 precedent. |
| A5 | The scaffold carries the committed `tax-calc@1.1.0` bundle + `tax-calc.xlsx` (not a regenerated copy) and a sample `pmcp.toml` | Pattern 2 / Pitfall 5 | Low — D-07 mandates reuse; the example already include_dir!s the committed golden. |
| A6 | A NEW fixtures location/crate for loan + quirks needs `rust_xlsxwriter` added to ITS dev-deps if not the compiler crate | Standard Stack / Installation | Low — keeping fixtures + authors in the compiler crate (where `rust_xlsxwriter` is already a dev-dep) avoids this entirely. Recommend compiler crate. |

## Open Questions (RESOLVED)

1. **Is the 1900-leap-year quirk expressible with whitelisted ops over `f64` serials?**
   - What we know: No date-serial code exists; DATE() is not whitelisted; serials are bare `f64`.
   - What's unclear: Whether a fixture can meaningfully encode the leap-year serial offset using only IF/arithmetic/whitelisted fns.
   - Recommendation: During planning, spike a tiny fixture. If it can't be expressed without new date primitives, document the date quirk as a `scalar_eval`-level numeric assertion (or a known-limitation note) — do NOT add date functions (deferred-boundary violation).
   - **RESOLVED** — routed to the planning-time spike in Plan 96-03 Task 2 (records disposition in SPIKE-1900-leap.md); guarded against adding any DATE function (deferred-functions boundary + WBDL-01 doc↔const binding).

2. **Reserved identifier: `pmcp_dialect_version` (named range) vs a config-sheet cell?**
   - What we know: D-03 allows either; the named-range path reuses `version.rs`'s proven defined-name scan.
   - What's unclear: Whether a config-sheet cell (positional) is preferred for authoring ergonomics.
   - Recommendation: Use a **named range** (`pmcp_dialect_version`) — it reuses the existing scan, is position-independent, and matches the `version`/`out_*` named-range conventions already in the codebase.
   - **RESOLVED** — named-range `pmcp_dialect_version` (unanimous convention across all Phase 96 plans).

3. **Where do the loan + quirk fixtures live?**
   - What we know: `tax-calc.xlsx` lives in `pmcp-workbook-compiler/tests/fixtures/`; `rust_xlsxwriter` is already a dev-dep there.
   - Recommendation: Co-locate loan + quirk fixtures + authors + `#[cfg(test)]` proofs in `pmcp-workbook-compiler` (avoids adding `rust_xlsxwriter` elsewhere; reaches the `#[cfg(test)]`-only override). The loan compile-and-serve gate's serve assertion needs `pmcp-server-toolkit[workbook]` as a dev-dep of the compiler crate (or live as a toolkit integration test) — confirm dev-dep edge during planning.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust stable toolchain | All build/test | ✓ (assumed — project standard) | latest stable | — |
| `umya-spreadsheet` 3.0 | Compiler ingest (loan/quirk compile) | ✓ (existing dep) | 3.0 | — |
| `rust_xlsxwriter` 0.95 | Fixture authoring | ✓ (existing compiler dev-dep) | 0.95 | — |
| `include_dir` 0.7.4 | Scaffold embedded bundle | ✓ (existing, via `workbook-embedded`) | 0.7.4 | — |
| `mcp-tester` (dev) | Optional serve-parity harness for WBEX-01 | ✓ (workspace) | 0.7.0 | Assert tools/list via direct Server API instead |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none — this is a self-contained in-repo phase.

## Validation Architecture

> nyquist_validation is enabled (config.json `workflow.nyquist_validation: true`).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` (property) + `cargo fuzz` (where new robustness surface) |
| Config file | none (cargo test); fuzz targets under each crate's `fuzz/` |
| Quick run command | `cargo test -p pmcp-workbook-compiler` / `cargo test -p cargo-pmcp` (per-crate, < 30s) |
| Full suite command | `make quality-gate` (fmt --all + clippy pedantic+nursery + build + test + audit, matches CI) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WBCL-05 | `--kind workbook-server` emits the runnable crate files | unit | `cargo test -p cargo-pmcp workbook_server` | ❌ Wave 0 |
| WBCL-05 | emitted `main.rs` matches the example (drift-lock) | unit | `cargo test -p cargo-pmcp emitted_main` | ❌ Wave 0 (mirror sql_server.rs test) |
| WBCL-05 | scaffolded crate `cargo run` serves 5 tools (smoke) | integration | scaffold-then-build test, or example smoke over ephemeral port | ❌ Wave 0 |
| WBDL-02 | `pmcp_dialect_version` read + same-major-≤-supported accepts | unit + property | `cargo test -p pmcp-workbook-compiler dialect_version` | ❌ Wave 0 |
| WBDL-02 | different major / newer → typed CompileError (fail-closed) | unit | same | ❌ Wave 0 |
| WBDL-02 | absent declaration → baseline, no error (existing fixtures unchanged) | unit | `cargo test -p pmcp-workbook-compiler` (tax-calc still compiles) | partial (reemit_golden proves tax-calc compiles) |
| WBDL-02 | spec-doc ↔ supported-version-const drift guard | unit | `cargo test -p pmcp-workbook-dialect` | ❌ Wave 0 (mirror dialect_spec test) |
| WBEX-01 | loan workbook compiles via the driver (structural-equiv) | integration | `cargo test -p pmcp-workbook-compiler reemit_loan` | ❌ Wave 0 (mirror reemit_golden.rs) |
| WBEX-01 | loan server `tools/list`/`get_manifest` reflects its OWN inputs | integration | serve-and-assert test (toolkit or compiler dev-dep) | ❌ Wave 0 |
| WBEX-02 | each quirk: scalar_eval unit assertion | unit | `cargo test -p pmcp-workbook-runtime scalar_eval` | ❌ Wave 0 |
| WBEX-02 | each quirk: mini reconcile fixture through `reconcile::reconcile` | integration | `cargo test -p pmcp-workbook-compiler quirks` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** the relevant per-crate `cargo test -p <crate>` (quick).
- **Per wave merge:** `cargo test --workspace` for touched crates + `make purity-check` (served-tree reader/code-mode absence).
- **Phase gate:** `make quality-gate` green (fmt/clippy-pedantic/build/test/audit) AND `make purity-check` green before `/gsd:verify-work`.

### Wave 0 Gaps
- [ ] `cargo-pmcp/src/templates/workbook_server.rs` + its tests (emitted-files + drift-lock) — covers WBCL-05
- [ ] `cargo-pmcp/src/templates/mod.rs` — register `workbook_server`
- [ ] `pmcp-workbook-compiler/src/dialect_version.rs` (or version.rs extension) + tests — covers WBDL-02
- [ ] `pmcp-workbook-dialect/src/lib.rs` SUPPORTED/BASELINE consts + drift guard — covers WBDL-02
- [ ] `docs/workbook-dialect-spec.md` version-policy section — covers WBDL-02
- [ ] `pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx` + `*.provenance-override.json` — covers WBEX-01
- [ ] `pmcp-workbook-compiler/src/reemit_loan.rs` (#[cfg(test)] compile-and-serve proof) — covers WBEX-01
- [ ] `pmcp-workbook-compiler/tests/fixtures/quirks/*.xlsx` (~7–9) + override markers — covers WBEX-02
- [ ] `pmcp-workbook-compiler/src/quirks_reconcile.rs` (#[cfg(test)] mini reconcile fixtures) — covers WBEX-02
- [ ] `pmcp-workbook-runtime/src/scalar_eval.rs` quirk unit tests — covers WBEX-02
- [ ] ALWAYS requirements (CLAUDE.md): add a **fuzz** target for the new version-string parse (`dialect_version`) and **property** tests for the semver-compat decision; a runnable **example** demonstrating `cargo pmcp new --kind workbook-server` output (or the scaffolded crate running).

*Note: the existing `reemit_golden.rs` already proves `tax-calc` (absent dialect version) compiles — the D-05 zero-edit guarantee is partially witnessed there; add an explicit "absent → baseline, no error" assertion.*

## Security Domain

> `security_enforcement` is not set to `false` in config.json; treated as enabled. This phase is offline tooling (no network, no auth surface) — most ASVS categories are N/A, but the fail-closed + provenance posture is the relevant security story.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Offline compiler + scaffold; no auth surface. |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | `validate_crate_name` (path-traversal guard, new.rs:116) on the scaffold arm; fail-closed typed `CompileError` on a bad/incompatible `pmcp_dialect_version` (D-04); single-cell-target guard on the named-range read. |
| V6 Cryptography | no | Bundle integrity uses the existing `BUNDLE.lock` sha256 hash-of-hashes (Phase 92) — not modified here. |
| V12 Files & Resources | yes | Fixture `.xlsx` are committed binaries; the freshness/provenance gate (WBCO-07) refuses non-Excel provenance — fixtures ride the `#[cfg(test)]`-only `TrustedFixture` override, which CANNOT be armed from a production build (CR-01). |

### Known Threat Patterns for offline-Excel-compiler tooling

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via crate name (`cargo pmcp new --kind workbook-server ../evil`) | Tampering | `validate_crate_name` rejects `/`, `\`, `..`, leading digit, non-`[A-Za-z0-9_-]` BEFORE any fs::write. [VERIFIED new.rs:116] |
| Spoofed bundle/dialect version (a flag or pmcp.toml overrides the workbook's declaration) | Spoofing | D-03: version comes SOLELY from the workbook cell; no flag/toml input (mirrors the T-94-00-VERSION single-source-of-truth posture in version.rs). |
| Production bypass of the provenance gate via the test override | Elevation of Privilege | `compile_workbook_with_fixture_override` is `#[cfg(test)]`-only and unreachable from any default/`--all-features` build (CR-01); a regression test asserts the same bytes are refused under `Enforce`. [VERIFIED] |
| Untrusted workbook serving stale cached values as the oracle | Tampering | Freshness gate refuses `fullCalcOnLoad`/non-Excel provenance; reconcile penny-grades every named output. Not modified — but the quirk fixtures must not weaken it. |

## Sources

### Primary (HIGH confidence — all in-repo, read this session)
- `cargo-pmcp/src/commands/new.rs` — `--kind` dispatch + `validate_crate_name` (the scaffold arm to add)
- `cargo-pmcp/src/templates/sql_server.rs` — the template-module pattern + drift-lock golden test to clone
- `crates/pmcp-server-toolkit/examples/workbook_server_http.rs` — the canonical EmbeddedSource `main.rs` wiring
- `crates/pmcp-server-toolkit/Cargo.toml` — `workbook` vs `workbook-embedded` feature split + required-features
- `crates/pmcp-workbook-server/Cargo.toml` + `src/cli.rs` — Shape A posture (`default-features = false` purity fix; clap Args precedent)
- `crates/pmcp-workbook-compiler/src/version.rs` — the EXISTING bundle-version reader (the defined-name scan to mirror; the absence-policy to INVERT)
- `crates/pmcp-workbook-compiler/src/lib.rs` — `compile_workbook` / `compile_workbook_inner` / `prepare_candidate` signatures + the ingest→stage1 integration point
- `crates/pmcp-workbook-compiler/src/reemit_golden.rs` — the in-crate compile-and-serve proof + `compile_workbook_with_fixture_override` recipe
- `crates/pmcp-workbook-compiler/src/stage1.rs` — `FreshnessPolicy` (Enforce vs TrustedFixture), collect-all refuse pass
- `crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx` + `tax-calc.provenance-override.json` — the fixture + provenance-marker convention
- `crates/pmcp-workbook-runtime/src/sheet_ir/semantics.rs` — `apply()` dispatch of all 13 whitelisted fns
- `crates/pmcp-workbook-runtime/src/sheet_ir/rounding.rs` — Excel rounding source-of-truth (the half-rounding quirk)
- `crates/pmcp-workbook-runtime/src/scalar_eval.rs` — the scalar evaluator + existing quirk-adjacent tests (div-by-zero/coercion)
- `crates/pmcp-workbook-compiler/src/reconcile/mod.rs` — `reconcile` / `within_tol` (TOL=0.01) penny path
- `crates/pmcp-workbook-dialect/src/lib.rs` — WHITELIST + the doc↔const drift-guard pattern to clone for the version const
- `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs` — the programmatic golden generator (context: bundle is generated, source .xlsx is not)
- `crates/pmcp-workbook-runtime/src/bundle_source.rs` + `crates/pmcp-server-toolkit/src/workbook/mod.rs` — `EmbeddedSource`/`LocalDirSource`/`WorkbookBuilderExt` surface
- `.planning/REQUIREMENTS.md` (WBCL-05/WBDL-02/WBEX-01/WBEX-02 + WBDL-01/03 context), `.planning/ROADMAP.md` (Phase 96 + load-bearing invariants), `.planning/phases/95-.../95-CONTEXT.md` (carry-forward)

### Secondary (MEDIUM confidence)
- None — no external research required; all findings are direct source reads.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; every package verified in-repo.
- Architecture (scaffold + version + gates): HIGH — all three map onto existing, proven patterns read this session.
- Pitfalls: HIGH — the two landmines (.xlsx authoring gap, 1900-leap no-date-code) were verified by direct grep/read, not assumed.
- The one genuine uncertainty (Open Question 1: is the 1900-leap quirk expressible without date code) is flagged MEDIUM and routed to a planning-time spike.

**Research date:** 2026-06-14
**Valid until:** 2026-07-14 (stable in-repo domain; the Phases 91–95 stack is frozen). Re-verify only if the runtime/compiler/toolkit crates change before planning.

## RESEARCH COMPLETE
