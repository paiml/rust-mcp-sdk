# Phase 96: Shape B Scaffold + Dialect-Version Declaration + Generalization Validation - Context

**Gathered:** 2026-06-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver the three remaining v2.3 pieces that close the milestone:

1. **WBCL-05 — Shape B scaffold:** `cargo pmcp new --kind workbook-server` scaffolds a
   runnable, editable crate (Cargo.toml + `main.rs` over `EmbeddedSource` + sample
   `pmcp.toml` + sample workbook source + pre-compiled embedded bundle) — a thin shell
   over the Phase-92 served-tool toolkit module, mirroring the existing
   `--kind sql-server` template-module pattern.
2. **WBDL-02 — Dialect-version declaration:** a workbook declares the dialect version
   it targets, and the compiler validates that declaration, enabling forward-compatible
   dialect evolution.
3. **WBEX-01 / WBEX-02 — Generalization gates:** a second, non-lighthouse example
   workbook compiles and serves end-to-end through the SDK path (its `get_manifest` /
   `tools/list` schema reflecting ITS OWN inputs with zero shared Rust and no privileged
   single output), plus an Excel-quirk fixture corpus that verifies reconcile
   determinism beyond the single golden case.

**Requirements:** WBCL-05, WBDL-02, WBEX-01, WBEX-02.

**Explicitly NOT in this phase:**
- Re-opening the compiler pipeline, runtime, or purity gate (Phases 91–93, stable).
- The Shape A `pmcp-workbook-server` binary (Phase 95, complete).
- Deferred-by-design v2.3 items: row-block iteration / arbitrary-N loops, capability
  cells (Rust/remote/MCP escape hatches), named-range-backed validation lists, S3/registry
  bundle store. The second workbook MUST stay within the constrained fixed-cell DAG.
- Does not touch `pmcp-code-mode`.
</domain>

<decisions>
## Implementation Decisions

### A. Second workbook — the WBEX-01 generalization gate
- **D-01 (domain = loan/mortgage payment calculator):** The second, non-lighthouse
  example workbook models a loan/mortgage calculation — deliberately different inputs and
  outputs from `tax-calc` so the served `get_manifest` / `tools/list` schema is visibly its
  own. It must be a **fixed-cell formula DAG** (no arbitrary-N row iteration — deferred in v2.3).
- **D-02 (maximal divergence via a rate-table lookup model — whitelist-legal):** The 13-function
  dialect whitelist has **no PMT, no POWER, no exponentiation** (`crates/pmcp-workbook-dialect/src/lib.rs:35`
  — `IF, VLOOKUP, INDEX, MATCH, SUMIF, SUM, ROUNDUP, CEILING, IFERROR, ISNUMBER, SEARCH, ROUND, TEXT`),
  so arbitrary-term `(1+r)^n` amortization is NOT expressible (it needs the deferred iteration).
  Divergence is therefore achieved with the **lookup families that `tax-calc` barely uses**: a
  rate-tier table driven by **VLOOKUP / INDEX-MATCH** (e.g. credit-score or LTV band → rate),
  **IFERROR** guards, **nested IF** tiering, and **ROUND / CEILING** to currency (which also
  overlaps the WBEX-02 quirk corpus). This is genuinely divergent formula coverage AND fully
  whitelist-legal AND quirk-rich.

### B. Dialect-version declaration (WBDL-02)
- **D-03 (declared in a reserved named cell/range inside the workbook):** The version lives in a
  reserved named-range (working name `pmcp_dialect_version`) or a config-sheet cell **inside the
  .xlsx** — self-describing, travels with the workbook, honors "the workbook is the specification."
  The compiler reads it during ingest. (Chosen over a `pmcp.toml` field; one `pmcp.toml` can map to
  multiple workbooks, and the declaration should be authoritative where the formulas live.)
- **D-04 (semver-compatible, fail-closed):** Same MAJOR = compatible (compiler accepts a declared
  minor ≤ its own supported minor); a different major OR a declared version newer than the compiler
  supports → a **hard, typed compile error** (fail-closed). This delivers forward-compatible
  evolution without abandoning the milestone's fail-closed ethos.
- **D-05 (absent declaration → baseline version, no error):** A workbook with no version cell (the
  existing `tax-calc` golden + every Phase 91/92/93/95 fixture) is treated as targeting the
  **baseline/oldest-supported dialect (e.g. 1.0)** and compiles normally; the compiler MAY emit a
  non-fatal advisory to add an explicit cell. Existing fixtures keep working with **zero edits** —
  no churn this phase.

### C. Shape B scaffold (WBCL-05)
- **D-06 (full round-trip payload over `EmbeddedSource`):** `cargo pmcp new --kind workbook-server`
  ships Cargo.toml + `main.rs` (using `EmbeddedSource` / the `workbook-embedded` feature — carry-forward
  from Phase 95 D-02, the inverse of Shape A's LocalDirSource) + a sample `pmcp.toml` + the **source
  .xlsx** + a **pre-compiled embedded bundle**. `cargo run` works immediately; the dev can edit the
  workbook → `cargo pmcp workbook compile` → rerun (full authoring loop). Mirrors the existing
  `--kind sql-server` dispatch + template-module pattern (`cargo-pmcp/src/commands/new.rs`,
  `cargo-pmcp/src/templates/sql_server.rs`).
- **D-07 (sample workbook = reuse the `tax-calc` golden):** The scaffold samples the existing, proven,
  minimal `tax-calc` workbook (its bundle already exists), so the **only newly-authored .xlsx this phase
  is the loan workbook** (D-01). Keeps scope tight while still teaching the structure.

### D. Excel-quirk corpus (WBEX-02)
- **D-08 (both layers):** Encode the corpus as **scalar_eval unit tests** (fast, pinpoint coverage of
  the formula evaluator) **AND mini reconcile fixtures** (each quirk a tiny workbook with cached cell
  values as the oracle, run through the **real penny-reconcile path** — the same mechanism the golden
  gate uses). The reconcile fixtures are what literally satisfy "verifies reconcile determinism."
- **D-09 (four named + a curated few, ~7–9 total):** Cover the four roadmap-named quirks (1900
  leap-year, empty-cell coercion, error propagation, half-rounding boundaries) PLUS a small curated set
  of high-value additions (text→number coercion, explicit #DIV/0! propagation, a float boundary such as
  0.1+0.2, negative-value rounding sign). Capped ~7–9 total — meaningfully beyond the single golden
  without ballooning the phase.

### Claude's Discretion
- Exact reserved named-range identifier for D-03, the baseline version string/number for D-05, the
  scaffolded crate's package name and the precise file layout, the loan workbook's exact rate-tier table
  contents and input/output cell names, and the precise fixture file locations for the WBEX-02 corpus —
  all left to research/planning provided they honor the locked decisions above.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Dialect & version declaration (WBDL-02)
- `crates/pmcp-workbook-dialect/src/lib.rs` — the deny-by-default 13-function `WHITELIST` (line ~35) and
  `DialectRules`; the loan workbook (D-02) MUST stay within it; the version declaration extends this crate's surface.
- `docs/workbook-dialect-spec.md` — the human dialect spec the `WHITELIST` const is bound to (WBDL-01 doc↔const
  binding test); the dialect-version declaration + compatibility policy (D-03/D-04/D-05) extend this spec.

### Scaffold (WBCL-05)
- `cargo-pmcp/src/commands/new.rs` — the `--kind` dispatch (`execute_sql_server` / `execute_openapi_server`);
  add a `workbook-server` arm mirroring it. Note `validate_crate_name` (path-traversal guard).
- `cargo-pmcp/src/templates/sql_server.rs` — the template module to mirror for the scaffold payload shape.
- `crates/pmcp-server-toolkit/Cargo.toml` (lines ~146–147, 163–166) — the `workbook` vs `workbook-embedded`
  feature split + the `required-features = ["workbook-embedded", "http"]` example; the scaffold targets `workbook-embedded`.
- `crates/pmcp-workbook-runtime/src/bundle_source.rs` + `crates/pmcp-server-toolkit/src/workbook/mod.rs` —
  `EmbeddedSource` / `BundleSource` surface the scaffolded `main.rs` consumes.

### Generalization gates (WBEX-01/02) & carry-forward
- `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0` — the synthetic golden; reused as the scaffold
  sample (D-07) and the contrast case for the second-workbook gate (D-01). **Zero customer / TowelRads material** (hard constraint).
- `.planning/phases/95-shape-a-binary-pmcp-workbook-server/95-CONTEXT.md` — Shape A carry-forward decisions
  (esp. D-02: `EmbeddedSource` is the Shape B scaffold's concern; the five tool names; loopback/HTTP posture).
- `.planning/ROADMAP.md` (Phase 96 section) — goal + the four success criteria, including the WBEX-01 gate wording.
- `.planning/REQUIREMENTS.md` — WBCL-05, WBDL-02, WBEX-01, WBEX-02 traceability rows.
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cargo pmcp new --kind sql-server` / `--kind openapi-server`: the exact dispatch + template-module
  pattern the workbook-server scaffold copies (single runnable crate, not the multi-crate workspace path).
- Phase 92 `EmbeddedSource` + `workbook-embedded` feature: bakes a pre-compiled bundle into the binary —
  the Shape B mechanism (already shipped + tested), inverse of Shape A's LocalDirSource.
- `tax-calc@1.1.0` golden bundle: exists and is proven; reused as the scaffold sample (no new authoring).
- The penny-reconcile path (Phase 93 compiler) + the golden promote gate: the existing reconcile mechanism
  the WBEX-02 mini fixtures plug into.

### Established Patterns
- Deny-by-default dialect: any loan-workbook formula outside the 13-fn whitelist is rejected at lint — the
  divergence lever MUST come from whitelisted lookup/round families (D-02), not new functions.
- Fail-closed everywhere: the dialect-version mismatch policy (D-04) follows the same typed-error,
  non-zero-exit posture as the rest of the milestone.
- Doc↔const binding test (WBDL-01): the dialect spec and the `WHITELIST`/version surface must not drift —
  any version-declaration addition needs the same kind of guard test.

### Integration Points
- Compiler ingest (Phase 93) gains the named-range version read + semver compatibility check (WBDL-02).
- `cargo-pmcp` `new` command gains the `workbook-server` kind (WBCL-05).
- The compiler/runtime test suites gain the second-workbook end-to-end gate (WBEX-01) and the quirk corpus (WBEX-02).
</code_context>

<specifics>
## Specific Ideas

- The loan workbook should make the lookup families (VLOOKUP/INDEX-MATCH against a rate-tier table) the
  star, precisely because `tax-calc` leans on percentage-of-base arithmetic — a reviewer should be able to
  see the two manifests are structurally different at a glance.
- The WBEX-01 gate's assertion is on the SECOND workbook's OWN `get_manifest`/`tools/list` schema reflecting
  its own inputs — the five tool NAMES stay the same (they are generic toolkit tools); the manifest behind
  them differs. That difference IS the generalization proof.
- ROUND/CEILING currency rounding deliberately appears in BOTH the loan workbook (D-02) and the quirk corpus
  (D-09 half-rounding) so the two gates reinforce each other.
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. (Scope-creep guardrails held: row-block iteration, capability
cells, named-range validation lists, and registry bundle stores remain deferred-by-design v2.3 items and were
not pulled in.)
</deferred>

---

*Phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation*
*Context gathered: 2026-06-14*
