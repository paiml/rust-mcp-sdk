# Phase 91: Workbook Runtime + Purity Gate + Dialect Spec - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Port the reader-free `pmcp-workbook-runtime` leaf crate (owned IR/model types,
deterministic topo executor, pure-Rust `scalar_eval`, writer-only `.xlsx`
renderer), stand up a mechanically-provable purity gate **before any `umya` code
exists**, and establish the SDK-owned versioned **dialect contract** (function
whitelist + refuse-set + finding model + published spec, bound to code by a
test). This is the smallest cut that proves the reader/served-binary boundary
(RFC §7), and every downstream consumer (toolkit module, compiler, served
binary) depends on its output.

**Requirements:** WBRT-01, WBRT-02, WBRT-03, WBRT-04, WBDL-01 — **and** WBDL-03
is **re-mapped out of this phase** (see Decision D-02 below).

**Explicitly NOT in this phase:** any `umya`/`quick-xml` reader code (Phase 93),
the `WorkbookMap` ingest model and linter *execution* (Phase 93), the §5
generalization fixes / CR-01/CR-02/WR-01 (Phase 93), the served-tool toolkit
module (Phase 92). Does not touch `pmcp-code-mode`.
</domain>

<decisions>
## Implementation Decisions

### Dialect + Linter Boundary (Area 1)
- **D-01:** Phase 91 ships a **new reader-free leaf crate `pmcp-workbook-dialect`**
  (not folded into the runtime) holding the *dialect contract*: the `WHITELIST`
  const + `DialectRules` (refuse-set rule ids, sheet-layer prefixes, colour-ontology
  constants) + the published versioned dialect spec doc + the doc↔const binding
  test. This satisfies **WBDL-01**. Rationale (user choice): keep
  governance/lint-contract a distinct, independently-versionable concern from
  the IR/compute/render runtime.
- **D-02:** **WBDL-03 ("lint a workbook against the dialect") migrates from
  Phase 91 → Phase 93.** The `WorkbookMap` owned model (lighthouse
  `workbook-compiler/src/ingest/cell_map.rs`, 246 LOC, umya-free) and the linter
  *execution* (lighthouse `workbook-compiler/src/dialect/linter.rs`, 596 LOC,
  zero umya) stay in / move to the **`pmcp-workbook-compiler`** crate (Phase 93),
  where umya ingest can feed a *real* `.xlsx` through `lint(&WorkbookMap) ->
  LintReport`. Phase 91 delivers only the *contract* the linter enforces, not a
  running linter. **Roadmap/REQUIREMENTS must be updated to reflect WBDL-03 →
  Phase 93.**
- **D-03 (derived, planner-actionable):** The **finding model
  (`LintFinding`/`Severity`/`LintReport`) stays in `pmcp-workbook-runtime`** — the
  `run()` executor returns a `LintFinding` on a dependency cycle, exactly as in
  the lighthouse (`workbook-runtime/src/finding.rs`). `pmcp-workbook-dialect`
  **depends on `pmcp-workbook-runtime`** and re-exports the finding types. So the
  Area-3 "lift finding.rs + Deserialize" work lands in the **runtime**, and the
  dialect crate layers WHITELIST/rules/spec on top.
- **D-04 (derived):** Publish/dependency order extends the CLAUDE.md list:
  **slot 2a `pmcp-workbook-runtime`** (leaf) → **slot 2b `pmcp-workbook-dialect`**
  (depends on runtime). Both are reader-free and both are asserted by the purity
  gate (D-09).

### Dialect v1 Whitelist Policy (Area 2)
- **D-05:** Dialect **v1 = the 13 functions as a flat, first-class set**
  (`IF`, `VLOOKUP`, `INDEX`, `MATCH`, `SUMIF`, `SUM`, `ROUNDUP`, `CEILING`,
  `IFERROR`, `ISNUMBER`, `SEARCH`, `ROUND`, `TEXT`). **Drop the lighthouse's
  "8 core + 5 D-09 widened" two-tier framing** — it was a lighthouse historical
  artifact, not an SDK contract distinction.
- **D-06:** Arithmetic **operators `+ - * / ^` are part of the dialect but
  checked separately** — they are not function tokens and do not appear in the
  whitelist (preserve the lighthouse separation).
- **D-07:** The set is **deny-by-default**; future additions/removals go through
  **dialect versioning** (WBDL-02, Phase 96), never silent widening. Removing any
  of the 13 would break the reference workbook's clean lint in Ph93/96 — do not
  curate down.

### Finding Format (Area 3)
- **D-08:** **Lift `finding.rs` and add `Deserialize`** so the lint report is a
  **round-trippable public contract** (the Ph94 lint CLI emits it; other tooling
  and tests read it back). Keep `rule` as a `String` (extensible — a new rule
  doesn't break serde). Retain the existing shape: `severity`
  (Error/Warning/Info, **only `Error` gates conformance** — D-05 of the
  lighthouse), stable slash-namespaced `rule` id, `sheet` + optional `cell`
  location, human `message`, BA-actionable `repair`, `schemars::JsonSchema`
  derive, collect-all `LintReport`, and the `has_errors()` gate. (Lands in the
  runtime per D-03.)

### Purity Gate (Area 4)
- **D-09:** **Three-layer gate**, run **per feature-combination** (not just
  default features) in both `just purity-check` and CI:
  1. **`cargo tree` per-crate assertions** (lift + adapt the lighthouse
     `justfile` `purity-check` recipe): NEGATIVE — `umya` / `quick-xml` / `swc_*`
     / `pmcp-code-mode` ∉ the `pmcp-workbook-runtime` **and**
     `pmcp-workbook-dialect` dependency trees. POSITIVE — `rust_xlsxwriter` **IS
     present** in the runtime tree (the renderer is wired). `zip` is **permitted**
     (the writer's deflate container — a writer is not a reader).
  2. **`cargo-deny [bans]`** declarative backstop banning the reader/JS crates.
  3. The **structural crate split itself** (umya is never a `[dependencies]`
     entry of runtime or dialect).
- **D-10:** Panic-freedom is **not** the purity gate's job — it stays enforced by
  the existing crate-level `#![deny(clippy::unwrap_used, clippy::expect_used,
  clippy::panic)]` (already on the lighthouse runtime/compiler value paths). The
  gate focuses solely on the reader/writer dependency boundary.

### Claude's Discretion
- Exact `cargo tree` invocation form, the precise `cargo-deny [bans]` stanza, and
  the CI feature-combination matrix shape are planner/researcher details.
- The `zip` version pin for `rust_xlsxwriter` (the runtime's only transitive
  archive dep) is a planner detail; the `quick-xml`/`zip` *compiler-side*
  transitive-pin re-derivation (`cargo tree -p umya-spreadsheet -i quick-xml`)
  is a **Phase 93** concern, not Phase 91.
- The doc↔const binding-test mechanism (parse the spec doc's function table,
  assert set-equality with `WHITELIST`) mirrors the lighthouse
  `dialect::dialect_spec::doc_whitelist_table_matches_const` — adapt as-is.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

> ⚠ **Path correction:** the v2.3 research and ROADMAP cite these docs/source by
> their *lighthouse-relative* paths (e.g. `docs/sdk-issue-...`, `crates/workbook-runtime/`).
> Those paths do **not** resolve from this repo. The lighthouse lives at
> `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/`.
> Absolute paths are given below. (The lighthouse path casing is `AI-on-Cloud`,
> not `ai-on-cloud` as in PROJECT.md.)

### v2.3 Research (in-repo, HIGH confidence, 2026-06-09)
- `.planning/research/SUMMARY.md` — synthesis; Phase A = this phase (runtime + purity gate)
- `.planning/research/STACK.md` — `rust_xlsxwriter 0.95` (writer, runtime-only, `default-features=false`); the reader stack is compiler-only
- `.planning/research/ARCHITECTURE.md` — two dependency cones; runtime leaf; publish-order extension
- `.planning/research/PITFALLS.md` — Pitfall 1 (purity-boundary erosion) is THE phase-91 risk
- `.planning/research/FEATURES.md` — table-stakes runtime/dialect feature floor

### Lighthouse RFC + design briefs (extraction source of truth)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/docs/sdk-issue-excel-workbook-compiler-extraction.md` §5 (generalization gaps) / §6 (open questions) / §7 (runtime-first build order)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/docs/Excel-as-Configuration-Architecture-Brief.md` — two-surface model, cell taxonomy §7, dialect §5
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/docs/workbook-dialect-spec.md` — the dialect-spec doc to lift (13-fn whitelist table, refuse-set §5, enforced-vs-deferred §6) → becomes this repo's `docs/workbook-dialect-spec.md`

### Lighthouse runtime/dialect source (direct lift targets)
- `.../quote-pricing/crates/workbook-runtime/src/` — the `pmcp-workbook-runtime` lift target. Key modules: `lib.rs`, `formula.rs` (Expr AST), `finding.rs` (lift + add Deserialize — D-08), `dag.rs`, `resolve.rs`, `manifest_model.rs`, `artifact_model.rs` (CellMap/BundleLock + integrity hashing), `scalar_eval.rs`, `sheet_ir/`, `render/` (writer-only LayoutDescriptor), `changelog.rs`, `excel_error.rs`, `range_ref.rs`
- `.../quote-pricing/crates/workbook-compiler/src/dialect/rules.rs` — `WHITELIST` const + refuse-set → relocate into `pmcp-workbook-dialect`
- `.../quote-pricing/crates/workbook-compiler/src/dialect/linter.rs` — linter execution → **stays in Phase 93 compiler** (D-02), consumes the dialect crate's rules
- `.../quote-pricing/crates/workbook-compiler/src/ingest/cell_map.rs` — `WorkbookMap` owned model → **Phase 93** (D-02)
- `.../quote-pricing/justfile` (recipe `purity-check`, lines ~54-92) — the proven cargo-tree reader-vs-writer gate to lift + adapt (D-09)

### SDK conventions
- `CLAUDE.md` "Release & Publish Workflow" — publish order to extend with slots 2a/2b (D-04)
- `.planning/ROADMAP.md` Phase 91-96 details + v2.3 milestone framing
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`crates/pmcp-server-toolkit`** `#[cfg(feature="http")] pub mod http` — the
  feature-gate module precedent the Phase-92 `workbook` module will mirror (not
  this phase, but the runtime's API shape should anticipate it).
- **`crates/pmcp-sql-server`** lib `run`/`serve` + thin `main.rs` shim — the
  Shape-A pattern the runtime feeds into later (Phase 95).

### Established Patterns
- Workspace leaf crates live under `crates/`; root `Cargo.toml` `[workspace]
  members` must gain `crates/pmcp-workbook-runtime` and `crates/pmcp-workbook-dialect`.
- Lighthouse runtime already carries `#![deny(clippy::unwrap_used,
  expect_used, panic)]` with `cfg(test)` allow — keep verbatim (D-10).
- serde `rename_all`/`schemars::JsonSchema` discipline matches lighthouse models
  — no reshaping needed beyond D-08's `Deserialize` add.

### Integration Points
- `pmcp-workbook-runtime` depends on `pmcp` core only (served cone:
  served-binary → toolkit[workbook] → runtime → pmcp). It does **not** forbid
  `pmcp` (unlike the lighthouse `quote-pricing-core` which banned it) — adapt the
  lighthouse cargo-tree recipe accordingly.
- `pmcp-workbook-dialect` depends on `pmcp-workbook-runtime` (for the finding
  types it re-exports) + the dialect-doc binding test.
</code_context>

<specifics>
## Specific Ideas

- Lift the runtime + dialect-contract source **near-verbatim** from the
  lighthouse (research flags Phase A as "standard patterns, direct lift"); the
  only deliberate deltas are: (1) split a `pmcp-workbook-dialect` crate out of the
  compiler's `dialect/` (D-01), (2) flat 13-fn whitelist (D-05), (3) `Deserialize`
  on the finding types (D-08).
- The dialect spec doc is a *published BA/auditor-facing moat document* — port it
  to `docs/workbook-dialect-spec.md` and keep the doc↔`WHITELIST` binding test so
  the published contract and enforced rule can never drift.
</specifics>

<deferred>
## Deferred Ideas

- **WBDL-03 linter execution + `WorkbookMap`** → Phase 93 (D-02), not lost — it's
  a deliberate re-map, requires a ROADMAP/REQUIREMENTS update.
- **WBDL-02 (workbook declares target dialect version)** → Phase 96 (already
  roadmapped); v1 dialect set is frozen here (D-05).
- **`quick-xml`/`zip` compiler-side transitive-pin re-derivation** → Phase 93
  (the reader lives there).
- **Typed `RuleId` enum + rule-id↔doc binding test** — considered for the finding
  model; deferred in favour of extensible `rule: String` (D-08). Revisit if a
  stable rule taxonomy is needed.
- **JS-oracle (`pmcp-code-mode`/SWC) reconcile parity** — a Phase-93 open question
  (the runtime's `scalar_eval` replaces it on the served path); not a Phase-91
  concern.

</deferred>

---

*Phase: 91-workbook-runtime-purity-gate-dialect-spec*
*Context gathered: 2026-06-09*
