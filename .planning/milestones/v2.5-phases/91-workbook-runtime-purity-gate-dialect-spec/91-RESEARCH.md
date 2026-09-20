# Phase 91: Workbook Runtime + Purity Gate + Dialect Spec - Research

**Researched:** 2026-06-09
**Domain:** Rust workspace crate extraction (reader-free leaf), mechanically-provable dependency-boundary gate, versioned dialect contract
**Confidence:** HIGH (every claim grounded in direct reads of the lighthouse source + the SDK integration targets; crate versions verified against crates.io sparse index 2026-06-09; the purity gate was executed against the live lighthouse tree)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Phase 91 ships a **new reader-free leaf crate `pmcp-workbook-dialect`** (not folded into the runtime) holding the *dialect contract*: the `WHITELIST` const + `DialectRules` (refuse-set rule ids, sheet-layer prefixes, colour-ontology constants) + the published versioned dialect spec doc + the doc↔const binding test. Satisfies **WBDL-01**. Rationale: keep governance/lint-contract a distinct, independently-versionable concern from the IR/compute/render runtime.
- **D-02:** **WBDL-03 ("lint a workbook against the dialect") migrates from Phase 91 → Phase 93.** The `WorkbookMap` owned model and the linter *execution* stay in / move to **`pmcp-workbook-compiler`** (Phase 93). Phase 91 delivers only the *contract* the linter enforces, not a running linter. **Roadmap/REQUIREMENTS must be updated to reflect WBDL-03 → Phase 93.**
- **D-03:** The **finding model (`LintFinding`/`Severity`/`LintReport`) stays in `pmcp-workbook-runtime`** — the `run()` executor returns a `LintFinding` on a dependency cycle. `pmcp-workbook-dialect` **depends on `pmcp-workbook-runtime`** and re-exports the finding types. The "lift finding.rs + Deserialize" work lands in the **runtime**.
- **D-04:** Publish/dependency order extends the CLAUDE.md list: **slot 2a `pmcp-workbook-runtime`** (leaf) → **slot 2b `pmcp-workbook-dialect`** (depends on runtime). Both reader-free, both asserted by the purity gate (D-09).
- **D-05:** Dialect **v1 = the 13 functions as a flat, first-class set** (`IF`, `VLOOKUP`, `INDEX`, `MATCH`, `SUMIF`, `SUM`, `ROUNDUP`, `CEILING`, `IFERROR`, `ISNUMBER`, `SEARCH`, `ROUND`, `TEXT`). **Drop the lighthouse's "8 core + 5 D-09 widened" two-tier framing.**
- **D-06:** Arithmetic **operators `+ - * / ^` are part of the dialect but checked separately** — not function tokens, do not appear in the whitelist.
- **D-07:** The set is **deny-by-default**; future additions/removals go through **dialect versioning** (WBDL-02, Phase 96), never silent widening.
- **D-08:** **Lift `finding.rs` and add `Deserialize`** so the lint report is a **round-trippable public contract**. Keep `rule` as a `String`. Retain the existing shape (`severity` Error/Warning/Info with only `Error` gating, stable slash-namespaced `rule`, `sheet` + optional `cell`, human `message`, BA-actionable `repair`, `schemars::JsonSchema` derive, collect-all `LintReport`, `has_errors()` gate). Lands in the runtime per D-03.
- **D-09:** **Three-layer purity gate**, run **per feature-combination** in both `just purity-check` and CI:
  1. **`cargo tree` per-crate assertions**: NEGATIVE — `umya`/`quick-xml`/`swc_*`/`pmcp-code-mode` ∉ runtime **and** dialect trees. POSITIVE — `rust_xlsxwriter` **IS present** in the runtime tree. `zip` is **permitted**.
  2. **`cargo-deny [bans]`** declarative backstop banning the reader/JS crates.
  3. The **structural crate split itself** (umya is never a `[dependencies]` entry of runtime or dialect).
  Runtime **may** depend on `pmcp` (unlike lighthouse which banned its core).
- **D-10:** Panic-freedom is **not** the purity gate's job — it stays enforced by the crate-level `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`.

### Claude's Discretion

- Exact `cargo tree` invocation form, the precise `cargo-deny [bans]` stanza, and the CI feature-combination matrix shape are planner/researcher details (answered below, §Purity Gate Mechanics).
- The `zip` version pin for `rust_xlsxwriter` is a planner detail; the compiler-side `quick-xml`/`zip` transitive-pin re-derivation is a **Phase 93** concern.
- The doc↔const binding-test mechanism mirrors the lighthouse `dialect::dialect_spec::doc_whitelist_table_matches_const` — adapt as-is.

### Deferred Ideas (OUT OF SCOPE)

- **WBDL-03 linter execution + `WorkbookMap`** → Phase 93 (D-02).
- **WBDL-02 (workbook declares target dialect version)** → Phase 96.
- **`quick-xml`/`zip` compiler-side transitive-pin re-derivation** → Phase 93.
- **Typed `RuleId` enum + rule-id↔doc binding test** — deferred in favour of extensible `rule: String` (D-08).
- **JS-oracle (`pmcp-code-mode`/SWC) reconcile parity** — Phase-93 open question; not Phase 91.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WBRT-01 | Reader-free crate owning shared model types (Manifest, CellMap, BundleLock, VersionChangelog, IR `Cell`/`Expr`), serde/schemars-clean | All types confirmed present in lighthouse `workbook-runtime/src/{manifest_model,artifact_model,changelog,sheet_ir,formula}.rs`; already serde/schemars-clean and `Deserialize`-deriving (except finding.rs — D-08). Direct lift. |
| WBRT-02 | Deterministic evaluator producing typed outputs + per-cell derivation traces | `sheet_ir/executor.rs::run` (topo-ordered) returns `RunResult` with `EvalTrace`; `scalar_eval.rs` pure-Rust leaf eval; `sheet_ir/semantics.rs` covers all 13 whitelisted fns. Direct lift. |
| WBRT-03 | Writer-only `.xlsx` renderer (`rust_xlsxwriter`) keeping the served binary reader-free | `render/mod.rs::render_xlsx` + `render/layout.rs::LayoutDescriptor`; `rust_xlsxwriter 0.95 default-features=false`; **verified reader-free via live `cargo tree`** (§Purity Gate). Direct lift. |
| WBRT-04 | CI/`just` purity gate fails build if reader appears in runtime/served tree (cargo-tree + cargo-deny backstop) | Lighthouse `justfile` `purity-check` recipe lifted + adapted (`pmcp` token removed from ban list per D-09); SDK CI `gate` job aggregation identified for merge-blocking wiring. |
| WBDL-01 | SDK owns a versioned dialect spec doc (whitelist + refuse-set) bound to `WHITELIST` const by a drift test | `docs/workbook-dialect-spec.md` (155 LOC) + `dialect/rules.rs` `WHITELIST` + `dialect/mod.rs::dialect_spec::doc_whitelist_table_matches_const` binding test. Relocates into `pmcp-workbook-dialect`. |
| ~~WBDL-03~~ | **RE-MAPPED to Phase 93 per D-02** — NOT in Phase 91 | Phase 91 ships the *contract*; the running linter + `WorkbookMap` stay compiler-side. REQUIREMENTS.md traceability table (line 103) currently lists WBDL-03 → Phase 91 and MUST be updated. |
</phase_requirements>

## Summary

Phase 91 is a **near-verbatim two-crate lift** from the proven lighthouse (`towelrads/mcp-servers/quote-pricing`), not new design. The reader-free `workbook-runtime` leaf already exists, already compiles under `#![deny(clippy::unwrap_used, expect_used, panic)]`, and — verified empirically this session — its dependency tree is **already reader-free** (`umya`/`quick-xml`/`swc_*`/`pmcp-code-mode`/`calamine` all absent) while `rust_xlsxwriter` IS present. The purity gate the SDK needs is the same `cargo tree` recipe the lighthouse ran, with exactly **one adaptation**: the lighthouse banned `pmcp` from its core tree; the SDK runtime is allowed to depend on `pmcp` (D-09), so `pmcp` is removed from the ban token set.

The single deliberate structural delta from the lighthouse is splitting the dialect *contract* (the `WHITELIST` const, `DialectRules`, the published spec doc, and the doc↔const binding test) out of the compiler's `dialect/` module into a new standalone `pmcp-workbook-dialect` leaf crate that depends on the runtime (D-01). The lighthouse keeps all of this **inside** `workbook-compiler/src/dialect/` as a module; the SDK promotes the *contract half* (`rules.rs`, the spec doc, the binding test) to its own crate while the *execution half* (`linter.rs`, `WorkbookMap`) stays behind for Phase 93 (D-02). The finding model moves to the runtime and gains `Deserialize` (D-08) — the only code-level change to lifted source.

**Primary recommendation:** Lift `workbook-runtime/src/` verbatim into `crates/pmcp-workbook-runtime/` (rename crate `workbook-runtime`→`pmcp-workbook-runtime`, lib name `workbook_runtime`→`pmcp_workbook_runtime`, bump `thiserror` 1→2 to match SDK convention, add `Deserialize` to the three `finding.rs` types). Create `crates/pmcp-workbook-dialect/` holding only `rules.rs` + the relocated `docs/workbook-dialect-spec.md` + the binding test, depending on and re-exporting the runtime's finding types. Land the adapted `purity-check` recipe (cargo-tree + cargo-deny + crate split) wired into a new merge-blocking CI job from day one, before any `umya` code exists in the workspace.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Shared IR/model types (Manifest, CellMap, BundleLock, Expr, Cell) | `pmcp-workbook-runtime` (reader-free leaf) | — | The served binary deserializes these; they MUST live in the reader-free crate so the compiler can re-export them upward without dragging the reader down (WBRT-01) [VERIFIED: lighthouse `workbook-runtime/src/lib.rs` re-export surface] |
| Deterministic IR evaluation + per-cell traces | `pmcp-workbook-runtime` (`sheet_ir/executor.rs`) | — | Pure-Rust topo executor; no I/O, no reader (WBRT-02) [VERIFIED: lighthouse source] |
| Writer-only `.xlsx` render | `pmcp-workbook-runtime` (`render/`) | `rust_xlsxwriter` (writer crate) | A writer is not a reader; the renderer's only transitive archive dep is `zip` via the writer (WBRT-03) [VERIFIED: live `cargo tree`] |
| Dialect contract (WHITELIST, refuse-set ids, colour ontology, spec doc, binding test) | `pmcp-workbook-dialect` (reader-free leaf) | depends on runtime for finding types | Governance/lint contract is independently versionable; kept distinct from compute (D-01, WBDL-01) [CITED: CONTEXT.md D-01] |
| Lint *finding* model (LintFinding/Severity/LintReport) | `pmcp-workbook-runtime` (`finding.rs`) | re-exported by dialect crate | `run()` returns a finding on a cycle, so it must sit on the umya-free runtime side (D-03) [VERIFIED: lighthouse `finding.rs` module doc] |
| Lint *execution* + `WorkbookMap` ingest | **Phase 93 `pmcp-workbook-compiler`** (NOT this phase) | — | Needs a real `.xlsx` via umya; execution belongs where the reader lives (D-02) [CITED: CONTEXT.md D-02] |
| Reader/served-binary boundary proof | CI + `just` purity gate | `cargo-deny` backstop | Mechanically-provable link boundary, established before any umya code (WBRT-04, Pitfall 1) [VERIFIED: lighthouse justfile + live tree] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` (+ `derive`) | `1` | IR/manifest/finding (de)serialization | SDK workspace baseline; lighthouse runtime uses it [VERIFIED: crates.io + SDK root Cargo.toml] |
| `serde_json` | `1` | Bundle-artifact JSON I/O | SDK baseline [VERIFIED] |
| `schemars` | `1.0` | `outputSchema`/manifest JSON-Schema projection; finding-type `JsonSchema` derive | SDK pins `schemars = "1.0"` behind `schema-generation`; lighthouse runtime resolves to `1.2.1` [VERIFIED: lighthouse tree shows `schemars v1.2.1`] |
| `thiserror` | **`2`** (lighthouse runtime carries `1.0.69`) | Runtime + render error enums | **SDK toolkit crates standardized on `thiserror = "2"`; bump from lighthouse `1` to match SDK convention** [VERIFIED: SDK `pmcp-server-toolkit/Cargo.toml` line `thiserror = "2"`; lighthouse tree shows `thiserror v1.0.69`] |
| `sha2` | `0.11` | Bundle-artifact hashing (`build_bundle_lock`, `sha256_hex`) | Matches `pmcp-code-mode` pin [VERIFIED: lighthouse `workbook-runtime/Cargo.toml`] |
| `hex` | `0.4` | Hash hex encoding | Matches `pmcp-code-mode` pin [VERIFIED] |
| `rust_xlsxwriter` | `0.95` (newest stable `0.95.0`, 2026-05-09), `default-features = false` | WRITER-ONLY `.xlsx` emitter for the render path (WBRT-03) | The single deliberate purity relaxation. Author `jmcnamara`, MIT/Apache [VERIFIED: crates.io sparse index newest non-yanked = `0.95.0` (119 versions); lighthouse Cargo.toml] |

**`pmcp` dependency — planner decision (FLAG):** CONTEXT.md D-09 *permits* the runtime to depend on `pmcp`, but the lighthouse runtime **does not** depend on it (verified: zero `pmcp` references in runtime source except doc-comments naming the SWC kernel it *replaced*; `cargo tree -p workbook-runtime` shows no `pmcp`). Phase 91's runtime port has **no functional need** for `pmcp` — the IR/finding/render types are all self-contained. **Recommendation: do NOT add a `pmcp` dependency in Phase 91.** D-09 only loosens the *ban* (so a future need wouldn't break the gate); it does not mandate the dependency. Adding `pmcp` now would only enlarge the runtime's tree for no purpose. The purity gate's ban list omits `pmcp` regardless (so the door stays open for Phase 92 if needed). `[ASSUMED]` — confirm with planner that "may depend" ≠ "must depend."

### Supporting (pmcp-workbook-dialect crate)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `pmcp-workbook-runtime` | path (`../pmcp-workbook-runtime`) | Re-export the finding types (`LintFinding`/`Severity`/`LintReport`) so the dialect crate layers WHITELIST/rules on top (D-03) | Always — the dialect crate's only non-trivial dependency |
| `serde` (+ `derive`) | `1` | If `DialectRules`/`CandidateRole` need serde (lighthouse `rules.rs` does NOT derive serde currently — they are plain enums/structs) | Only if the spec/rules need round-tripping; lighthouse `rules.rs` has no serde derives |

**Note:** The lighthouse `dialect/rules.rs` types (`DialectRules`, `CandidateRole`, `WHITELIST`) carry **no serde derives** and **no third-party deps** — they are pure `const` + plain enums/structs over `std`. The dialect crate is therefore extremely lean: it needs only `pmcp-workbook-runtime` (for re-exported findings) and possibly `serde` only if a future requirement serializes `DialectRules`. [VERIFIED: lighthouse `rules.rs`]

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled IR/DAG/scalar_eval (lift verbatim) | `petgraph`, `formualizer`, `xlformula_engine` | No off-the-shelf formula engine enforces the dialect whitelist at parse time — adopting one loses the core safety property and adds deps. The owned `Dag` + Kahn toposort is ~214 LOC. DO NOT introduce a formula/graph crate. [VERIFIED: STACK.md §3; lighthouse `dag.rs` 214 LOC] |
| `rust_xlsxwriter` (writer-only) | `umya` for output too | Reusing umya for render drags the READER into the served tree — violates the purity rule. A writer-only crate is the whole point. [CITED: STACK.md] |
| `thiserror = "2"` (match SDK) | `thiserror = "1"` (lighthouse) | SDK standardized on `2`; aligning avoids two majors in-tree. [VERIFIED: SDK toolkit Cargo.toml] |

**Installation (anticipated Cargo.toml shape):**

```toml
# crates/pmcp-workbook-runtime/Cargo.toml — reader-free served-binary leaf
[package]
name = "pmcp-workbook-runtime"
version = "0.1.0"          # SDK convention: literal 0.1.0, NOT version.workspace
edition = "2021"
license = "MIT OR Apache-2.0"

[lib]
name = "pmcp_workbook_runtime"   # renamed from lighthouse `workbook_runtime`

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1.0"
thiserror = "2"           # bumped from lighthouse 1.0.69 to match SDK convention
sha2 = "0.11"
hex = "0.4"
rust_xlsxwriter = { version = "0.95", default-features = false }  # WRITER-ONLY

# crates/pmcp-workbook-dialect/Cargo.toml — reader-free dialect-contract leaf
[package]
name = "pmcp-workbook-dialect"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[dependencies]
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }
# serde only if DialectRules ever needs serialization — lighthouse rules.rs has none
```

**Version verification (2026-06-09, crates.io sparse index, absolute-path curl to bypass shell hooks):**
- `rust_xlsxwriter` newest non-yanked = **0.95.0** (119 versions total) — matches pin.
- `umya-spreadsheet` newest non-yanked = **3.0.0** — confirms STACK.md (NOT a Phase 91 dep; compiler-only, Phase 93).
- `quick-xml` newest = **0.40.1** — NOT a Phase 91 dep (compiler-only).
- **`zip` actual transitive of `rust_xlsxwriter 0.95` = `zip v7.2.0`** (live `cargo tree`), NOT zip 8. STACK.md's "zip 8" refers to *umya's* transitive zip (compiler-side, Phase 93). The runtime's only zip is the writer's `zip 7.2.0`. [VERIFIED: `cargo tree -p workbook-runtime --depth 1`]

## Package Legitimacy Audit

> The only new third-party crate Phase 91 introduces is `rust_xlsxwriter` (all others — serde, serde_json, schemars, thiserror, sha2, hex — are already in the SDK workspace at matching versions). slopcheck could not be installed in this sandboxed session (pip unavailable / network-restricted); registry verification was performed via the crates.io sparse index directly.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `rust_xlsxwriter` | crates.io | mature (119 published versions; 0.95.0 latest 2026-05-09) | high (widely used Excel writer) | github.com/jmcnamara/rust_xlsxwriter (author `jmcnamara`, the libxlsxwriter/XlsxWriter author) | not run (unavailable) | **Approved with verify-checkpoint** — lighthouse already gated it through a blocking human-verify (crates.io legitimacy, `cargo tree -i` writer-only confirmation, `cargo audit` clean) per its Cargo.toml comment; SDK should re-run `cargo audit` + `cargo tree -i zip` to re-confirm writer-only entry. |
| serde, serde_json, schemars, thiserror, sha2, hex | crates.io | already vendored in SDK workspace | — | — | n/a (pre-existing) | Approved (no new install) |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none — but because slopcheck was unavailable, the planner SHOULD add a single `checkpoint:human-verify` before the `rust_xlsxwriter` install confirming author `jmcnamara` + repo `github.com/jmcnamara/rust_xlsxwriter` + `cargo audit` clean (mirroring the lighthouse's own T-12-SC gate). [ASSUMED — slopcheck unavailable; registry-existence confirmed but provenance gate recommended]

## Architecture Patterns

### System Architecture Diagram

```
                    ┌──────────────────────────────────────────────┐
   bundle .json     │  pmcp-workbook-runtime  (reader-free LEAF)    │
   (manifest.json,  │                                              │
   executable.ir,   │  ┌────────────┐   serde/Deserialize           │
   cell_map.json,   │  │ model types│◄── manifest_model / artifact   │
   layout.json,     │  │ (WBRT-01)  │    _model / changelog / formula│
   BUNDLE.lock)     │  └─────┬──────┘                               │
        │           │        │ feeds                                 │
        ▼           │        ▼                                       │
   [deserialize]───►│  ┌────────────┐   topo-order Kahn              │
                    │  │ run executor│  sheet_ir/executor.rs (WBRT-02)│
   seed inputs ────►│  │  + traces  │──► RunResult{values, EvalTrace} │
                    │  └─────┬──────┘     │ on cycle → LintFinding     │
                    │        │            ▼ (finding.rs, D-03)        │
                    │        ▼        ┌─────────────┐                 │
                    │  ┌────────────┐ │ render_xlsx │ rust_xlsxwriter │
                    │  │scalar_eval │ │  (WBRT-03)  │──► .xlsx bytes   │
                    │  │ (pure Rust)│ └─────────────┘   (writer-only,  │
                    │  └────────────┘                    zip 7.2.0)    │
                    └──────────────┬───────────────────────────────────┘
                                   │ depends + re-exports findings
                                   ▼
                    ┌──────────────────────────────────────────────┐
                    │  pmcp-workbook-dialect  (reader-free LEAF)    │
                    │  WHITELIST const (13 fns, D-05) + DialectRules│
                    │  + docs/workbook-dialect-spec.md (WBDL-01)    │
                    │  + doc↔const binding test                    │
                    └──────────────────────────────────────────────┘

   ┌──────────── PURITY GATE (WBRT-04) — runs per feature-combo ─────────────┐
   │ cargo tree -p pmcp-workbook-runtime  → MUST NOT contain umya|quick-xml| │
   │   swc_|pmcp-code-mode|calamine ; MUST contain rust_xlsxwriter ; zip OK  │
   │ cargo tree -p pmcp-workbook-dialect  → same negative assertion          │
   │ cargo deny check bans                → declarative backstop             │
   │ structural split                     → umya never a [dependencies] entry│
   └─────────────────────────────────────────────────────────────────────────┘
```

The reader (`umya`) does not appear anywhere in this phase — that is the entire point. Phase 91 establishes the gate **before** any reader code exists (Phase 93).

### Recommended Project Structure

```
crates/
├── pmcp-workbook-runtime/        # NEW leaf (slot 2a)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # re-export surface (lift, rename crate/lib)
│       ├── excel_error.rs        # 66 LOC
│       ├── range_ref.rs          # 63 LOC
│       ├── formula.rs            # 186 LOC — Expr/BinOp/UnOp AST
│       ├── finding.rs            # 186 LOC — LIFT + ADD Deserialize (D-08)
│       ├── dag.rs                # 214 LOC — owned Dag + Kahn toposort
│       ├── resolve.rs            # 316 LOC — range/ref resolution primitives
│       ├── manifest_model.rs     # 618 LOC — Manifest/CellRole/Role/Dtype/InputTier
│       ├── artifact_model.rs     # 261 LOC — CellMap/BundleLock + integrity hashing
│       ├── scalar_eval.rs        # 330 LOC — pure-Rust leaf evaluator
│       ├── changelog.rs          # 243 LOC — VersionChangelog model
│       ├── sheet_ir/             # executor + semantics + rounding + value
│       │   ├── mod.rs            # Cell/CellExpr IR unit (already derives Deserialize)
│       │   ├── executor.rs       # 25.3K — the run() topo executor (WBRT-02)
│       │   ├── semantics.rs      # 25.8K — all 13 whitelisted fns
│       │   ├── eval_bridge.rs    # 11.3K — leaf arithmetic bridge
│       │   ├── eval_value.rs / value.rs / rounding.rs
│       └── render/               # writer-only render (WBRT-03)
│           ├── mod.rs            # 23.7K — render_xlsx (rust_xlsxwriter)
│           └── layout.rs         # 8.1K — LayoutDescriptor serde shape
├── pmcp-workbook-dialect/        # NEW leaf (slot 2b)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                # WHITELIST + DialectRules + CandidateRole (from rules.rs)
│                                 # + #[cfg(test)] doc_whitelist_table_matches_const
docs/
└── workbook-dialect-spec.md      # PORTED from lighthouse (155 LOC); binding test reads it
```

**Total runtime lift: ~2,584 LOC across the top-level `*.rs` files + sheet_ir + render** (verified `wc -l`). The dialect crate is ~200 LOC (`rules.rs` minus the colour-palette + whitelist) + the 155-LOC spec doc + the ~100-LOC binding test.

### Pattern 1: Reader-free leaf with provable link boundary
**What:** The runtime crate's `Cargo.toml` lists ONLY serde/schemars/sha2/hex/thiserror/rust_xlsxwriter. No reader, no JS runtime. The boundary is then a `cargo tree`-provable LINK boundary, not a convention.
**When to use:** Always for this phase — it is the WBRT-04 guarantee.
**Example (lifted verbatim from lighthouse `workbook-runtime/src/lib.rs`):**
```rust
// Source: lighthouse crates/workbook-runtime/src/lib.rs:16-19
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```

### Pattern 2: finding.rs round-trippable contract (the ONE code delta — D-08)
**What:** Add `Deserialize` to the three finding types so the lint report round-trips (Phase 94 lint CLI emits it; tooling/tests read it back). Keep `rule: String`.
**Example (the exact derive change):**
```rust
// Lighthouse finding.rs currently derives: Serialize, schemars::JsonSchema (NO Deserialize)
// Source: lighthouse crates/workbook-runtime/src/finding.rs:21,25,41,83
// CHANGE: add Deserialize to Severity, LintFinding, LintReport
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Error, Warning, Info }

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintFinding {
    pub severity: Severity,
    pub rule: String,          // kept String (extensible — D-08)
    pub sheet: String,
    pub cell: Option<String>,
    pub message: String,
    pub repair: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintReport { pub findings: Vec<LintFinding> }
```
**Add a round-trip test** alongside the existing serialize test: `let back: LintReport = serde_json::from_value(serde_json::to_value(&report)?)?; assert_eq!(...)`.

### Pattern 3: doc↔const binding test (WBDL-01 — adapt as-is)
**What:** A `#[cfg(test)]` test parses the function names out of the spec doc's whitelist table and asserts set-equality with `WHITELIST`. The lighthouse keeps it in `dialect/mod.rs` as a private `dialect_spec` module; in the SDK it moves into `pmcp-workbook-dialect/src/lib.rs`.
**Example (lifted from lighthouse `dialect/mod.rs:29-106`):**
```rust
// Source: lighthouse crates/workbook-compiler/src/dialect/mod.rs
const SPEC_PATH: &str = "../../docs/workbook-dialect-spec.md"; // relative to CARGO_MANIFEST_DIR
// parse_doc_whitelist: take FIRST backtick token on rows whose col-2 is "core" or
// contains "D-09 widened". Assert BTreeSet equality with WHITELIST.
```
**Two adaptations required for D-05 (drop the two-tier framing):**
1. The spec doc table currently uses `| core |` and `| **D-09 widened** |` in column 2 — the parser keys on these category strings. Under D-05 the 13 functions are a flat set. **The planner must decide:** either (a) keep a category column the parser can key on (simplest — relabel all 13 rows to a single category like `whitelist` and update the parser's `is_whitelist_row` predicate), or (b) change the parser to key on a table-header marker rather than per-row category. The lighthouse test also has a "belt-and-braces" loop asserting the 5 D-09 names — under D-05 that loop should assert all 13 names (or be dropped). `[ASSUMED]` — exact table format is a planner call; the binding-test *mechanism* lifts unchanged.

### Pattern 4: SDK crate version convention (NOT version.workspace)
**What:** The lighthouse crates use `version.workspace = true`. SDK workspace crates use a **literal `version = "0.1.0"`** (verified: `pmcp-sql-server` = `0.1.0`, `pmcp-server-toolkit` = `0.1.0`). The new crates must follow the SDK convention.
**Why:** The SDK does not define `[workspace.package] version`; its root `version = "2.9.0"` is the `pmcp` crate's own version, not a workspace-package inherited value. Using `version.workspace` would either fail or wrongly pin the new crates to 2.9.0.

### Anti-Patterns to Avoid

- **Adding `pmcp` to the runtime "because D-09 allows it":** D-09 *permits* it (removes the ban) but the runtime has zero functional need (verified pmcp-free). Adding it bloats the tree for nothing. Only add if a concrete Phase-92 need appears.
- **Folding the dialect contract back into the runtime:** D-01 deliberately splits it. Keep `WHITELIST`/`DialectRules`/spec in `pmcp-workbook-dialect`.
- **Lifting `linter.rs` or `WorkbookMap` (cell_map.rs) into Phase 91:** D-02 re-maps these to Phase 93. They need umya. Leave them.
- **Banning `zip` in the purity gate:** `zip 7.2.0` enters legitimately via `rust_xlsxwriter` (a writer). Ban only the READER stack (`umya`/`quick-xml`/`calamine`) + JS (`swc_`/`pmcp-code-mode`).
- **Using `version.workspace = true`:** breaks SDK convention (see Pattern 4).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| `.xlsx` writing | Custom OOXML/ZIP emitter | `rust_xlsxwriter 0.95` (writer-only) | Battle-tested, deterministic doc-properties pinning, MIT/Apache, reader-free [VERIFIED: lighthouse render/mod.rs] |
| Bundle integrity hashing | Custom hash-of-hashes | Lift `artifact_model.rs::{build_bundle_lock, sha256_hex, update_field}` verbatim | Already exists, shared by emitter + served integrity check [VERIFIED] |
| Dependency toposort | `petgraph` | Lift the owned `Dag` + Kahn `toposort` (`dag.rs`, 214 LOC) | Keeps the runtime serde-clean and zero-graph-dep; petgraph adds weight for no gain [CITED: STACK.md §3] |
| Excel formula semantics (rounding, error propagation) | `f64::round` / naive math | Lift `sheet_ir/{semantics,rounding}.rs` (`excel_round`/`excel_ceiling`/`excel_floor`) | Excel uses round-half-away-from-zero, not banker's rounding; stdlib differs [CITED: PITFALLS.md Pitfall 6] |
| Dialect whitelist enforcement | A general formula parser | The 13-name `WHITELIST` const checked at parse time | No off-the-shelf engine enforces a dialect whitelist — adopting one loses the security property [CITED: STACK.md §3] |

**Key insight:** Phase 91 builds almost nothing new — it RELOCATES proven, penny-reconciled lighthouse code. The only authored work is: rename two crate identifiers, add three `Deserialize` derives, split out the dialect crate, port one markdown doc, and adapt one `justfile` recipe + one CI job. The risk is in *what you accidentally drag along* (the reader), which the purity gate catches.

## Runtime State Inventory

> This is a code/crate-extraction phase, not a deployment rename. There is no stored data, live service, OS-registered state, or secret keyed on a renamed string. The only "rename" is crate/lib identifiers within the new SDK workspace (there is no prior `pmcp-workbook-*` published crate to migrate from).

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — verified: no datastore keys on `workbook-runtime`; the crate is brand-new to the SDK (not present in current `crates/`) | none |
| Live service config | None — verified: no deployed service references these crate names (first publish is Phase 91+) | none |
| OS-registered state | None — verified: no OS task/process registration involved in crate extraction | none |
| Secrets/env vars | None — verified: no secret/env var references the crate names | none |
| Build artifacts | The crate **lib name changes** `workbook_runtime` → `pmcp_workbook_runtime`. Any code `use`-ing the lighthouse name must use the new path. Since this is a fresh lift (no prior SDK consumers), there are no stale artifacts to clear. The `docs/workbook-dialect-spec.md` binding-test path (`CARGO_MANIFEST_DIR/../../docs/...`) must resolve from `crates/pmcp-workbook-dialect/` → repo-root `docs/` (two levels up — same depth as lighthouse `crates/workbook-compiler` → `../../docs`). | Verify the `SPEC_PATH` relative depth: `crates/pmcp-workbook-dialect/` → `../../docs/workbook-dialect-spec.md` is correct (two segments up). [VERIFIED: SDK `crates/` is one level below root, same as lighthouse] |

## Common Pitfalls

### Pitfall 1: Purity-boundary erosion — the Excel reader leaks into the served tree
**What goes wrong:** A shared crate pulls umya, or Cargo feature unification leaks the reader into the runtime/dialect tree.
**Why it happens:** The boundary is enforced by a bespoke recipe, not the type system. Extraction tends to drop the recipe ("CI later") or run it only on default features.
**How to avoid:** Land the purity gate (cargo-tree + cargo-deny + crate split) WITH the runtime crate, in a **merge-blocking CI job**, run **per feature-combination** (`--no-default-features` and `--all-features`/`--features full`). Include the POSITIVE `rust_xlsxwriter`-present assertion so a deleted renderer can't make the negative gate vacuously pass.
**Warning signs:** `Cargo.lock` churn touching `umya`/`quick-xml`/`calamine` for a runtime-only change; a new `[features]` entry gating a compiler helper; CI green but only default features.
**Phase-91 status:** This is THE phase-91 risk (PITFALLS.md Pitfall 1). The gate must exist before any umya code (Phase 93). [VERIFIED: lighthouse already runs this gate; live tree is reader-free]

### Pitfall 2: `version.workspace = true` mis-pin
**What goes wrong:** Copying the lighthouse `version.workspace = true` into the SDK crates pins them to `pmcp` 2.9.0 or fails (the SDK has no `[workspace.package] version`).
**How to avoid:** Use literal `version = "0.1.0"` per SDK convention (Pattern 4).
**Warning signs:** Cargo error "`workspace.package.version` was not defined" or new crates suddenly versioned 2.9.0. [VERIFIED: SDK `pmcp-sql-server`/`pmcp-server-toolkit` use literal `0.1.0`]

### Pitfall 3: `thiserror` major-version drift
**What goes wrong:** Lifting the lighthouse runtime verbatim keeps `thiserror = "1"`, putting two majors of `thiserror` in the SDK tree.
**How to avoid:** Bump to `thiserror = "2"`. The `#[derive(thiserror::Error)]` API is source-compatible for the simple `#[error("...")]` enums used in `render::RenderError` and the runtime errors. Verify clippy/build after the bump.
**Warning signs:** `cargo tree` shows both `thiserror v1.x` and `thiserror v2.x`; `multiple-versions` cargo-deny warning. [VERIFIED: lighthouse tree = `thiserror v1.0.69`; SDK convention = `2`]

### Pitfall 4: Binding-test table-format coupling under D-05
**What goes wrong:** The lighthouse binding test keys on `| core |` / `| D-09 widened |` category strings. D-05 drops the two-tier framing, so a naive doc edit (removing the category labels) silently makes the parser return zero names — which the test catches with its `!doc_set.is_empty()` guard, but only if that guard is preserved.
**How to avoid:** When flattening the doc table, keep a parseable category column (or change the parser's row predicate) AND keep the `!doc_set.is_empty()` guard. Update the "belt-and-braces" 5-name loop to either assert all 13 or be removed.
**Warning signs:** "parsed zero function names from the spec whitelist table" panic; the test passing against an empty intersection. [VERIFIED: lighthouse `dialect/mod.rs:44-67` parser predicate]

### Pitfall 5: `make quality-gate` vs `just quality-gate` divergence
**What goes wrong:** The SDK has BOTH a `Makefile` `quality-gate` (the CLAUDE.md-mandated one: fmt/lint-with-pedantic/build/test/audit/unused-deps) and a thinner `justfile` `quality-gate` (fmt/clippy/build/test). The purity-check must wire into the **Makefile** path (and CI), since CLAUDE.md mandates `make quality-gate` before commits/PRs. The lighthouse used `just purity-check`; the SDK's analogue is a new `make purity-check` target + a CI job.
**How to avoid:** Add `purity-check` as a Makefile target AND a CI job; per STACK.md §5 + CLAUDE.md D-07 precedent, keep heavy per-feature cargo-tree runs in CI (not blocking the local dev loop) but provide an on-demand `make purity-check`. [VERIFIED: SDK Makefile `quality-gate` has no purity step; justfile differs]

## Purity Gate Mechanics (answers CONTEXT.md discretion items 1-3)

### The exact `cargo tree` invocations (adapted from lighthouse `justfile:54-92`)

The lighthouse recipe ran `cargo tree -p workbook-runtime | grep -Ei 'umya|swc_|pmcp-code-mode|quick-xml'` (FAIL on match) plus a positive `grep -qi 'rust_xlsxwriter'`. **The ONE adaptation for the SDK (D-09): the lighthouse `quote-pricing-core` arm also banned `pmcp` (`grep -E 'tokio|pmcp|reqwest|hyper|axum'`) — drop `pmcp` from the SDK runtime/dialect ban list** because the SDK runtime is allowed to depend on `pmcp`.

```bash
# NEGATIVE assertions (FAIL if any reader/JS token appears) — per crate, per feature-combo
for crate in pmcp-workbook-runtime pmcp-workbook-dialect; do
  for feat in "" "--no-default-features" "--all-features"; do
    if cargo tree -p "$crate" $feat 2>/dev/null | grep -Ei 'umya|calamine|quick-xml|swc_|pmcp-code-mode'; then
      echo "purity-check FAILED: reader/JS dep in $crate ($feat)"; exit 1
    fi
  done
done

# POSITIVE assertion (FAIL if the writer is missing — guards against a vacuous negative pass)
if ! cargo tree -p pmcp-workbook-runtime 2>/dev/null | grep -qi 'rust_xlsxwriter'; then
  echo "purity-check FAILED: rust_xlsxwriter ABSENT from runtime tree"; exit 1
fi

# zip is PERMITTED — do NOT grep for it. It enters only via rust_xlsxwriter (zip 7.2.0).
```

Note: `pmcp` is intentionally ABSENT from the ban grep (D-09). The ban set is exactly `umya|calamine|quick-xml|swc_|pmcp-code-mode`.

### The CI feature-combination matrix (answers discretion item 2)

The runtime crate currently has **no `[features]`** (it is a plain leaf), so "per feature-combination" for Phase 91 means the three workspace build modes: default, `--no-default-features`, `--all-features`. Because the runtime has no optional deps, all three trees are identical *today* — but running all three future-proofs against a later `[features]` addition (e.g. a Phase-92 `pmcp` feature) silently leaking a reader via unification. Wire the matrix now so the gate is correct when features arrive. A GitHub Actions `strategy.matrix` over `["", "--no-default-features", "--all-features"]` × `["pmcp-workbook-runtime", "pmcp-workbook-dialect"]` is the recommended shape.

### The `cargo-deny [bans]` stanza (answers discretion item, with a CONSTRAINT)

**CONSTRAINT (verified):** the SDK `deny.toml` header reads *"Deployed from paiml/infra — do not edit manually. Spec: docs/specifications/unified-ci-pipeline.md"*. Adding a `[bans].deny` entry directly to `deny.toml` will be **overwritten by the next infra deploy**. The planner has three options, in order of preference:
1. **Rely on the `cargo tree` arm as the precise per-crate boundary** (the lighthouse's load-bearing mechanism) and treat `cargo-deny` bans as a *documented, deferred* backstop pending an upstream `paiml/infra` change. This is the honest recommendation — `cargo-deny`'s native ban scoping is *workspace-global*, not per-crate, so it cannot express "umya is banned from the runtime tree but allowed in the compiler tree" (which Phase 93 needs). The cargo-tree arm already expresses exactly that.
2. Open a `paiml/infra` PR to add the workbook ban stanza to the canonical `deny.toml` template.
3. Use a *separate* `deny.toml` (e.g. `crates/pmcp-workbook-runtime/deny.toml`) invoked with `cargo deny --manifest-path` scoped to the leaf — but cargo-deny still resolves the full workspace lock, so this does not cleanly scope either.

**The stanza shape (for whichever path):**
```toml
[bans]
deny = [
  { name = "umya-spreadsheet" },
  { name = "quick-xml" },        # NOTE: workspace-global — will conflict with Phase 93 compiler
  { name = "calamine" },
  { name = "swc_ecma_parser" },
]
```
**Critical caveat the planner MUST account for:** a workspace-global `quick-xml`/`umya` ban in `deny.toml` will **break Phase 93** (the compiler legitimately needs them). Therefore the cargo-deny layer of D-09 is best treated as a per-crate cargo-tree assertion (Layer 1), with cargo-deny Layer 2 limited to crates that can be banned workspace-wide without conflict, or deferred. **The cargo-tree per-crate arm (Layer 1) is the real boundary; the crate split (Layer 3) is the real enforcement; cargo-deny (Layer 2) is genuinely just a backstop and is partially blocked by both the infra-managed file and the workspace-global scoping.** Document this honestly rather than claiming a clean three-layer gate. [VERIFIED: SDK deny.toml header + cargo-deny scoping semantics]

### Wiring into the merge-blocking CI gate

The org-required status check is the `gate` job in `.github/workflows/ci.yml` (`needs: [test, quality-gate]`, line 281). To make purity-check merge-blocking, add a new `purity-check` job and append it to the `gate` job's `needs:` array (mirroring how CLAUDE.md describes the PMAT `quality-gate` job propagating to `gate`). [VERIFIED: ci.yml lines 281-296]

## Code Examples

### The render entry point (writer-only — WBRT-03)
```rust
// Source: lighthouse crates/workbook-runtime/src/render/mod.rs:38
use rust_xlsxwriter::{Color, DocProperties, ExcelDateTime, Format, Formula, Workbook};
// render_xlsx replays a LayoutDescriptor + injects the executor's computed values,
// producing DETERMINISTIC .xlsx bytes IN MEMORY (no filesystem — Lambda-safe).
// Determinism: pins doc properties to a FIXED creation datetime + empty author so
// two renders of the same (layout, run) are byte-identical.
fn writer_err(e: rust_xlsxwriter::XlsxError) -> RenderError {
    RenderError::Writer(e.to_string())
}
```

### The run executor + traces (WBRT-02)
```rust
// Source: lighthouse crates/workbook-runtime/src/sheet_ir/{mod,executor}.rs
pub use executor::{build_dag, run, EvalTrace, RunResult};
// run() walks Cells in Kahn topo order; on a dependency cycle it returns a
// Box<LintFinding> (hence finding.rs lives in the runtime — D-03).
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Lighthouse runtime depends on `pmcp-code-mode` SWC/JS kernel for eval | Pure-Rust `scalar_eval.rs` replaces it on the served path | Lighthouse Phase 11 Plan 05 | No JS runtime in the served tree; reader-free guarantee holds [VERIFIED: lighthouse lib.rs doc] |
| Dialect contract lives inside `workbook-compiler/src/dialect/` | Split into standalone `pmcp-workbook-dialect` leaf crate (D-01) | This phase | Independently versionable governance contract |
| finding types: `Serialize` + `JsonSchema` only | Add `Deserialize` (D-08) | This phase | Lint report becomes a round-trippable public contract |
| `thiserror = "1"` (lighthouse) | `thiserror = "2"` (SDK convention) | This phase | One major of thiserror in-tree |

**Deprecated/outdated:**
- The "8 core + 5 D-09 widened" two-tier whitelist framing — D-05 flattens to 13 first-class names. The lighthouse `rules.rs` comment block and the spec-doc category column reflect the old framing and must be flattened.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The runtime should NOT add a `pmcp` dependency in Phase 91 (D-09 permits but doesn't mandate; runtime is functionally pmcp-free) | Standard Stack | LOW — if planner wants pmcp for forward-compat, adding it is trivial and the gate already permits it; no rework |
| A2 | `thiserror` 1→2 bump is source-compatible for the simple `#[error("...")]` enums in the runtime | Standard Stack / Pitfall 3 | LOW — derive API for basic enums is stable across the major; verify at build |
| A3 | The flat-13 spec-doc table still needs a parseable category column for the binding test (exact format is a planner call) | Pattern 3 / Pitfall 4 | LOW — the binding-test mechanism is proven; only the doc table format is in question, caught by `!doc_set.is_empty()` |
| A4 | `rust_xlsxwriter` provenance is sound (author jmcnamara) but slopcheck was unavailable — planner should add a verify checkpoint | Package Legitimacy Audit | LOW — lighthouse already gated it; crates.io provenance confirmed; checkpoint is belt-and-braces |
| A5 | cargo-deny Layer 2 of D-09 is partially blocked (infra-managed deny.toml + workspace-global scoping conflicting with Phase 93) — recommend treating cargo-tree as the real boundary | Purity Gate Mechanics | MEDIUM — if the planner assumes a clean cargo-deny per-crate ban, they'll hit the infra-managed-file constraint and the Phase-93 conflict. The cargo-tree arm fully covers the requirement; document the cargo-deny limitation. |

## Open Questions

1. **cargo-deny `[bans]` ownership and Phase-93 conflict**
   - What we know: SDK `deny.toml` is infra-managed ("do not edit manually"); cargo-deny ban scoping is workspace-global; Phase 93 needs `umya`/`quick-xml` in the compiler.
   - What's unclear: whether to (a) defer the cargo-deny layer to a `paiml/infra` PR, (b) accept cargo-tree-only enforcement for Phase 91, or (c) find a per-crate cargo-deny invocation.
   - Recommendation: implement Layers 1 (cargo-tree per-crate) + 3 (crate split) fully in Phase 91 — they satisfy WBRT-04 completely. Track Layer 2 (cargo-deny backstop) as a deferred hardening with an explicit note that a workspace-global reader ban conflicts with Phase 93 and must be scoped or deferred. Do not claim a clean three-layer gate.

2. **WBDL-03 traceability update (mechanical, blocking)**
   - What we know: REQUIREMENTS.md line 103 maps `WBDL-03 → Phase 91`; D-02 re-maps it to Phase 93.
   - What's unclear: nothing — this is a required doc edit.
   - Recommendation: the planner's first task (or a checkpoint) updates REQUIREMENTS.md line 103 to `WBDL-03 | Phase 93` AND adds the `pmcp-workbook-dialect` crate to ROADMAP Phase 91 scope. Flag this so it isn't silently dropped.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust stable toolchain | All crate work | ✓ | stable (CI uses `dtolnay/rust-toolchain@stable`) | — |
| `cargo-deny` | Purity gate Layer 2 (backstop) | ✓ | 0.18.3 | cargo-tree Layer 1 covers the requirement if deny is unavailable [VERIFIED: `cargo-deny 0.18.3` installed] |
| `cargo tree` | Purity gate Layer 1 (primary boundary) | ✓ (built into cargo) | — | none needed |
| Lighthouse source tree | Lift source | ✓ | `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/` | — |
| `rust_xlsxwriter 0.95` | Render path (WBRT-03) | ✓ on crates.io | 0.95.0 | none (the only writer that keeps the binary reader-free) [VERIFIED: sparse index] |
| `slopcheck` | Package legitimacy audit | ✗ (pip/network restricted in this session) | — | crates.io sparse-index provenance check + planner verify-checkpoint |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `slopcheck` (fallback: manual crates.io provenance + a `checkpoint:human-verify` before the `rust_xlsxwriter` install).

## Validation Architecture

> `.planning/config.json` was not found to explicitly disable `nyquist_validation`; treating it as enabled.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[cfg(test)] mod tests` + `#[test]`); lighthouse runtime already ships unit tests in every module |
| Config file | none — cargo-native; CI runs `cargo test --lib --tests -- --test-threads=1` (per `--test-threads=1` race-prevention convention in CLAUDE.md) |
| Quick run command | `cargo test -p pmcp-workbook-runtime --lib` / `cargo test -p pmcp-workbook-dialect --lib` |
| Full suite command | `make quality-gate` (fmt + pedantic clippy + build + test + audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WBRT-01 | Model types serde round-trip (incl. new finding Deserialize) | unit | `cargo test -p pmcp-workbook-runtime finding -- --test-threads=1` | ⚠️ lift exists; ADD round-trip test for Deserialize (D-08) |
| WBRT-02 | `run()` evaluator determinism + traces; cycle → LintFinding | unit | `cargo test -p pmcp-workbook-runtime sheet_ir::executor` | ✅ lifted from lighthouse |
| WBRT-03 | `render_xlsx` produces byte-identical deterministic output | unit | `cargo test -p pmcp-workbook-runtime render` | ✅ lighthouse has a determinism test (two renders byte-equal) |
| WBRT-04 | Purity gate fails on reader presence; passes reader-free | CI/script | `make purity-check` (new) + per-feature CI matrix | ❌ Wave 0 — author the recipe + CI job |
| WBDL-01 | doc↔WHITELIST binding (drift fails build) | unit | `cargo test -p pmcp-workbook-dialect doc_whitelist_table_matches_const` | ⚠️ lift exists; adapt for flat-13 (D-05) |

### Sampling Rate
- **Per task commit:** `cargo test -p <crate> --lib -- --test-threads=1` (quick).
- **Per wave merge:** `make quality-gate` (full: fmt/pedantic-clippy/build/test/audit/unused-deps) + `make purity-check`.
- **Phase gate:** full suite green AND purity gate green (per-feature) before `/gsd:verify-work`. The purity gate is the load-bearing sampling point — measure reader-absence + writer-presence per feature-combination, determinism of `render_xlsx`, and doc↔const non-drift.

### Wave 0 Gaps
- [ ] `crates/pmcp-workbook-runtime/src/finding.rs` — ADD a `Deserialize` round-trip test (D-08 delta).
- [ ] `make purity-check` Makefile target + `.github/workflows/ci.yml` `purity-check` job appended to the `gate` job's `needs:` array (WBRT-04).
- [ ] Adapt `pmcp-workbook-dialect` binding test for the flat-13 table format (D-05).
- [ ] Verify `thiserror` 1→2 bump compiles clean under pedantic clippy.

*(All evaluator/render/model unit tests come over from the lighthouse intact — the framework is cargo-native and already present.)*

## Security Domain

> `security_enforcement` not explicitly disabled; included. Phase 91 is a reader-free library extraction with no auth/session/network surface — most ASVS categories are N/A. The security-load-bearing property is the *dependency boundary* itself.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth surface in a reader-free model/eval/render leaf |
| V3 Session Management | no | No sessions |
| V4 Access Control | no | No access-control surface this phase |
| V5 Input Validation | partial | The finding model + IR deserialization must round-trip safely; serde with `#[serde(rename_all)]` and typed enums (no untyped passthrough). Untrusted-`.xlsx` parsing is explicitly NOT in this phase (reader is Phase 93). |
| V6 Cryptography | yes (hashing only) | Bundle integrity via `sha2 0.11` (`build_bundle_lock`/`sha256_hex`) — never hand-roll; lift verbatim |
| V14 Config / Supply Chain | yes | The purity gate (WBRT-04) IS the supply-chain control: it mechanically proves the Excel reader never enters the served tree. `rust_xlsxwriter` install gated by provenance verify-checkpoint. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Excel reader (umya/quick-xml) silently entering the served binary via feature unification | Elevation of Privilege / Tampering (unverified parser on hot path) | Per-crate per-feature `cargo tree` purity gate + crate split (WBRT-04, D-09) [VERIFIED: live tree reader-free] |
| Supply-chain: a slopsquatted/compromised writer crate | Tampering | `rust_xlsxwriter` provenance pinned (author jmcnamara, github.com/jmcnamara/rust_xlsxwriter) + `cargo audit` + verify-checkpoint |
| `panic` on malformed IR/coordinate reaching the served value path | Denial of Service | Crate-level `#![deny(clippy::unwrap_used, expect_used, panic)]` (D-10); render errors surface as `RenderError`, never panic [VERIFIED: lighthouse render/mod.rs RenderError] |
| Determinism break in `render_xlsx` (non-reproducible bytes) | Tampering (audit/provenance integrity) | Pinned doc-properties (fixed creation datetime, empty author) + a byte-equality determinism test [VERIFIED: lighthouse render doc] |

## Sources

### Primary (HIGH confidence)
- Lighthouse `crates/workbook-runtime/src/` (lib.rs, finding.rs, sheet_ir/mod.rs, render/mod.rs, Cargo.toml) — direct reads; the lift target
- Lighthouse `crates/workbook-compiler/src/dialect/{rules.rs, mod.rs, finding.rs}` — WHITELIST const, binding test, finding re-export
- Lighthouse `docs/workbook-dialect-spec.md` (155 LOC) — the spec doc to port; whitelist-table format
- Lighthouse `justfile` purity-check recipe (lines 54-92) — the proven cargo-tree gate; **executed live** to confirm reader-free tree + `rust_xlsxwriter`/`zip 7.2.0` presence + no `pmcp`
- SDK `Cargo.toml` (workspace members, root version 2.9.0), `deny.toml` (infra-managed header), `Makefile` (quality-gate/lint), `justfile`, `.github/workflows/{ci.yml,quality-gate.yml}` — direct reads
- SDK `crates/pmcp-sql-server/Cargo.toml`, `crates/pmcp-server-toolkit/Cargo.toml` — version convention (`0.1.0`), thiserror=2, feature-gate precedent
- crates.io sparse index (verified 2026-06-09, absolute-path curl): rust_xlsxwriter 0.95.0, umya-spreadsheet 3.0.0, quick-xml 0.40.1
- `.planning/REQUIREMENTS.md` (traceability table line 103 — WBDL-03 mapping to correct), `.planning/research/{SUMMARY,STACK,ARCHITECTURE,PITFALLS}.md`

### Secondary (MEDIUM confidence)
- cargo-deny ban-scoping semantics (workspace-global, not per-crate) — basis for the Layer-2 limitation note

### Tertiary (LOW confidence)
- slopcheck verdict — UNAVAILABLE this session (pip/network restricted); compensated by crates.io provenance + verify-checkpoint recommendation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every crate read from lighthouse Cargo.toml + verified on crates.io; the `pmcp`-optional and `thiserror` bump are explicit, flagged decisions.
- Architecture: HIGH — direct lift of a proven, penny-reconciled reference impl; module list verified by `wc -l` + reads; reader-free tree verified by live `cargo tree`.
- Pitfalls: HIGH — grounded in lighthouse source + SDK constraints (infra-managed deny.toml, version convention) verified this session.
- Purity gate mechanics: HIGH on cargo-tree (executed live); MEDIUM on cargo-deny (scoping + infra-managed-file constraint surfaced, not fully resolved — flagged as an open question).

**Research date:** 2026-06-09
**Valid until:** 2026-07-09 (stable lighthouse source; re-verify `rust_xlsxwriter` version if planning slips past 30 days)
