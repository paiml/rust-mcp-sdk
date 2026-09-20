# Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate — Research

**Researched:** 2026-06-11
**Domain:** Offline Excel→MCP **compiler library** extraction (`pmcp-workbook-compiler`) — umya-isolated ingest→lint→synth→parse→DAG→penny-reconcile→emit→promote-gate pipeline, with five RFC §5 generalization fixes applied at extraction time, plus a change-class + auto-derived golden-corpus promote gate with a fingerprint-bound `ApprovalRecord`.
**Confidence:** HIGH (direct read of the lighthouse compiler source, the in-repo runtime/dialect/served crates, the committed tax-calc golden, the existing purity gate, and live crates.io version checks)

## Summary

Phase 93 ports the lighthouse `workbook-compiler` crate (~13K LOC across 12 module groups) into a new in-repo `pmcp-workbook-compiler` crate, confining `umya-spreadsheet` (the Excel reader) + the quarantined `quick-xml`/`zip` provenance reader to this one crate. The compiler **re-exports and emits** the already-lifted `pmcp-workbook-runtime` shared types (IR, manifest model, bundle lock + hashing, changelog, sheet_ir incl. `rounding.rs`, finding model) so the served binary deserializes without the reader. It applies five §5 fixes — manifest-driven emit, CR-01 symmetric change-class, CR-02 versioned non-overwriting writes, WR-01 enum-tier skip, umya fabricated-provenance refusal — and ships the change-class router + auto-derived golden-corpus promote gate with a copy-pasteable `--accept` block-loop output and a content-hash-fingerprinted `ApprovalRecord`.

**The single most important finding:** **all five §5 "fixes" are ALREADY IMPLEMENTED in the current lighthouse** — the lighthouse evolved past the RFC §5 snapshot (the RFC is dated 2026-06-10; the §5 list describes the *pre-fix* state). The CR-01 symmetric classifier (`change_class/mod.rs` lines 165-255, with assumption-first hard rule, demotion arms, and a full symmetric test suite), the CR-02 versioned promote (`commands/compile_workbook.rs:603-611` sets `candidate.version = next_version` before a `GatedUpdate` write to a NEW `{name}@{next_version}/` dir), the WR-01 enum-tier skip (`artifact/mod.rs:121-130` `ratify_tiers` skips `allowed_values.is_some()` inputs, with a passing `ratify_skips_frozen_enum_inputs` test), the fingerprint-bound `ApprovalRecord` (`gate/corpus.rs:96-160` binds prev+candidate content hashes), and the anchored umya-provenance identity check (`provenance/gate.rs:255-275` `.starts_with("Microsoft Excel")`) are all present. **This re-frames Phase 93 from "port-then-fix" to "lift-clean-scrub-generalize-and-verify-the-fixes-survive."** The planner must NOT plan these as net-new bug fixes; it must plan them as *invariants to verify at extraction* (each carries a symmetric/property/integration test that must come over with the code and stay green).

**Two genuine net-new pieces remain** (these are the real Phase-93 design work, not lifts): (1) **delete `build_reference_manifest` from every non-test path** — the lighthouse `lib.rs:769-858` still hand-constructs the ufh-quote manifest (the one true §5 generalization gap that survives in the current lighthouse); replace it with a generic `compile_workbook(workbook_path, …)` driver that routes `synthesize → ratify → emit_bundle`. (2) **The D-09 auto-derived regression corpus** — the lighthouse corpus is BA-curated (`cases.json` authored by hand); Phase 93's D-09 requires *auto-generating* the input grid from manifest defaults + enum domains so the BA authors no cases. The lighthouse `ApprovalRecord`/fingerprint/`--accept` machinery is reusable verbatim; only the *case generation* is new.

**Primary recommendation:** Lift the lighthouse compiler module tree verbatim into `pmcp-workbook-compiler`, drop `pmcp-code-mode`/SWC (the in-repo runtime already ships a pure-Rust `scalar_eval` + `sheet_ir` executor — verify no reconcile-parity gap, then drop SWC entirely; do NOT carry the JS oracle), scrub all 34 customer-identifier-bearing files per Phase 92 D-13, delete `build_reference_manifest` in favor of a generic driver, add the D-09 auto-corpus generator over the reusable gate, upgrade WBCO-07 from "record" to "REFUSE", reconcile the `Manifest.annotations` struct delta (in-repo has it, lighthouse does not), extend the existing `make purity-check` lists, and prove the loop by re-emitting a tax-calc bundle that diffs byte-identical against the committed `tax-calc@1.1.0` golden.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01 (block semantics):** Errors block bundle emit; warnings are advisory and the BA can build past them. Findings are collect-all, located (cell-addressed), and carry repair guidance — the whole lint pass reports at once, never fail-fast (WBDL-03; uses the runtime's already-lifted `finding.rs`).
- **D-02 (error/warning split — "integrity-only blocks"):**
  - **ERRORS (block emit):** named-output penny-reconcile failure (D-03); formula parse failure (WBCO-03); non-whitelist function / dialect violation (WBDL-03); umya fabricated-provenance (WBCO-07, refused with `oracle/non-excel-app` — locked behavior).
  - **WARNINGS (advisory, BA builds past):** manifest-inference ambiguity (D-06); enum source not an inline list — falls back to a dynamic input (D-07); helper-cell reconcile drift (D-03); style/Guide advisories.
- **D-03 (reconcile tolerance — block only when a published answer is wrong):** A penny-reconcile mismatch (compiled IR vs Excel's stored cached value, under operand-anchored rounding — WBCO-04) is a **hard error only on a named output**; a mismatch on an **intermediate/helper cell is a located warning**. Only ever forgives small numeric/rounding drift — broken logic (unsupported function) is a dialect error, not a tolerated warning. "Cached value" = the last-computed result Excel stores alongside each formula on save; the compiler uses it as the trusted oracle and grades its own compiled computation against it. The BA never sees this machinery.
- **D-04 (stay-in-Excel):** The candidate semantic manifest is synthesized fully workbook-driven from cell colour, a Guide sheet, column headers — `build_reference_manifest` is deleted from every non-test path. `manifest.json` is an emitted artifact the BA NEVER hand-edits. To correct an inference the BA adjusts the sheet (recolour a cell, edit the Guide) and recompiles. **Ratification = a recorded sign-off** (approver + date) — carry forward the runtime `manifest_model` `ratified` / `ratified_by` / `ratified_at` fields.
- **D-05 (unclassifiable cells → safe default + selective warning):** A cell the compiler can't confidently classify stays in the computation but is NOT exposed as an input or output (treated as an internal helper). A warning is emitted **only when the cell looks like it should be exposed** (e.g. a bare hardcoded number a consumer might want to vary), naming the cell and how to mark it in the sheet. Obvious internal helpers do not generate warnings.
- **D-06 (enum sources — WBCO-06 + WR-01, partly locked):** Only inline DV lists (`formula1` quoted literals, ≤10 values) become closed JSON-Schema enums. A dropdown sourced from a cell range or named range falls back to a dynamic input with a warning (precise reason code) — never a block. **WR-01 (locked):** enum inputs skip Variable-tier assignment so the default path can never seed an out-of-enum empty string — verified against the **committed** manifest, not the in-memory builder.
- **D-07 (organize by action bucket):** The re-compile diff is grouped by change class as the BA action: "Safe — auto-applied" (HotReload), "Needs your approval" (BlockUntilAccept), "New version required" (NeverAutoPromote). Each entry carries a plain-language "what changed → what it means for consumers" line.
- **D-08 (material changes only):** Show named-output value changes, manifest changes (input/output added/removed; type/unit/enum changed), and structural IR changes (a formula redefined, a new dependency). Pure helper-cell numeric noise is summarized in one line ("+3 internal cells recalculated"), not enumerated.
- **(locked §5 classifier behaviors):** CR-01 symmetric coverage of demotion-direction changes (Input→Constant, source/assumption flips) — each produces a non-empty class, never silent HotReload (WBGV-01); a strictest-policy reducer hard-blocks any assumption (yellow-cell) change even amid hot-reloadable deltas (WBGV-02); a stable canonical IR sub-DAG identity hash distinguishes numeric drift from semantic redefinition (WBGV-03).
- **D-09 (corpus = auto-derived regression baseline):** The golden-corpus gate replays an auto-generated set of input cases (from the manifest's defaults + enum domains) through BOTH the prior accepted version and the candidate and flags any named output that moved beyond tolerance. The BA authors NO test cases — the "golden corpus" is the previous version's own behavior captured automatically.
- **D-10 (block loop — show deltas + exact approve command):** On a block, the gate stops the promote and prints: which outputs changed and by how much, the change class, and the exact copy-pasteable approve command (`--accept --approver <X> --effective-date <D>`). Running it records a fingerprint-bound `ApprovalRecord` (content-hash of the candidate), re-baselines the corpus, and lets the version through (WBGV-04/05). One deliberate, auditable action — not an inline interactive prompt.
- **D-11 (versioning — declared in the workbook, CR-02):** The BA bumps the version in the workbook (a changelog/Guide entry); the compiler writes the new bundle to its own `@<version>` directory and never overwrites the baseline (CR-02 / WBGV-06); `BUNDLE.lock` version == declared version == `changelog.to_version`.
- **D-12 (first build has no gate):** On the first version there is no prior accepted baseline, so the promote gate is a no-op (it establishes the baseline). First-version correctness is enforced by penny-reconcile against Excel's own cached values (D-03).
- **D-13 (`evidence/` member):** The bundle's `evidence/` member records the reconciliation result + provenance trail (changelog, parser-equivalence) — carry forward the seven-member contract frozen in Phase 92.

### Claude's Discretion

- Exact compiler module layout (mirror the lighthouse `workbook-compiler/src` tree: `ingest/ manifest/ formula/ dag/ sheet_ir/ reconcile/ change_class/ gate/ provenance/ artifact/`), reusing the runtime's shared types rather than re-declaring them.
- The auto-generated input-case grid size/shape for D-09 (bounded; defaults + enum-domain combinations), reconcile-finding message wording, and the `ApprovalRecord` on-disk format/location.
- The operand-anchored rounding implementation detail (the runtime already has `sheet_ir/rounding.rs`) — must NOT be a naïve `delta.abs()` tolerance (grep-gated per the roadmap success criterion).
- Colour/Guide/header inference heuristics (lift the lighthouse `manifest/` synth), provenance-reader internals (`quick-xml`/`zip` quarantined raw-bytes reader detecting umya's fabricated `<Application>`/`calcId`).

### Deferred Ideas (OUT OF SCOPE)

- `cargo pmcp compile-workbook`/`lint-workbook`/`emit-bundle` CLI + `pmcp.toml` — **Phase 94**. The compiler exposes library verbs + structured outputs; the CLI is a thin shell carrying the `--accept` flow. The `--accept` *flag* is Phase 94; the gate *behavior*, `ApprovalRecord` shape, and golden-corpus mechanics are decided and built HERE.
- Shape A `pmcp-workbook-server` binary + deploy/test — **Phase 95**.
- `cargo pmcp new --kind workbook-server` scaffold, dialect-version declaration, second-workbook + Excel-quirk generalization gate — **Phase 96** (the second-workbook test is the true generalization proof).
- BA-curated input→expected test cases as a gate basis — rejected for Phase 93 (D-09 uses an auto-derived regression baseline).
- Auto-bump versioning by change class — rejected for Phase 93 (D-11 keeps versioning workbook-declared).
- Interactive inline gate approval prompt — rejected (D-10 uses an explicit, auditable, copy-pasteable `--accept` command instead).
- **Does NOT touch `pmcp-code-mode`.** (And: drop the lighthouse compiler's `pmcp-code-mode`/SWC dependency entirely — see Standard Stack.)
- **Row-block iteration / "for each room" loops** — the lighthouse `sheet_ir/loop_exec.rs` (574 LOC) and `RoomAggregator`/`run_with_loop` are the deferred arbitrary-N parser feature. Do NOT lift; out of scope (REQUIREMENTS.md "Out of Scope").
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WBDL-03 | Lint a workbook against the dialect (whitelist-only, deny-by-default), collect-all, located, BA-actionable findings with repair guidance | Lift `dialect/linter.rs` (596 LOC) + `dialect/finding.rs`; runs against `pmcp-workbook-dialect::WHITELIST` + `DialectRules`; re-uses runtime `finding.rs`. The linter EXECUTION (needs a real `.xlsx` via umya) was re-mapped from Phase 91 to Phase 93 (Phase 91 D-02). |
| WBCO-01 | Ingest `.xlsx` (umya, compiler-isolated); capture cached cell values as trusted oracle | Lift `ingest/mod.rs` (734 LOC) + `ingest/cell_map.rs` → `WorkbookMap`/`CellRecord`; oracle = `provenance/gate.rs` `OracleCorpus` over cached `<v>` values |
| WBCO-02 | Synthesize candidate semantic manifest from colour/Guide/headers with BA ratification — fully workbook-driven, no per-workbook Rust (kills `build_reference_manifest`) | Lift `manifest/synth.rs` (1006 LOC) + `ratify.rs`; **DELETE `build_reference_manifest` (lib.rs:769-858) — the one surviving §5 gap**; route synth→ratify→emit |
| WBCO-03 | Parse formulas + reconstruct dependency DAG with Excel-semantics (`sheet_ir`) | Lift `formula/{token,parser,rebase}.rs` (Pratt parser, whitelist-at-parse-time) + `dag/{graph,resolve,topo}.rs` (Kahn); `sheet_ir` executor already in runtime |
| WBCO-04 | Compile pure cells to IR; penny-reconcile vs oracle (operand-anchored rounding, not naïve abs-delta) | Lift `reconcile/{mod,classifier,drift}.rs`; classifier already forbids `delta.abs()` (grep-gated); uses runtime `sheet_ir::rounding::{excel_round,excel_roundup,excel_ceiling}` |
| WBCO-05 | Emit the seven-member bundle (manifest/IR/cell_map/layout/BUNDLE.lock/evidence/) — the compiler↔server contract | Lift `artifact/{mod,bundle_lock,cell_map,evidence,executable,layout}.rs`; emits runtime `build_bundle_lock`/`fold_evidence_hash`; target = Phase 92 seven-member contract |
| WBCO-06 | Closed JSON-Schema enums from inline DV literals (≤10); range/named-range rejected with precise reason codes | In `manifest/synth.rs` DV resolution; D-06 locked; range source → dynamic input + warning (WARNING, not block) |
| WBCO-07 | Distinct provenance class for umya-stamped (fabricated `<Application>`/`calcId`) workbooks; REFUSE with `oracle/non-excel-app` | Lift `provenance/{gate,raw_parts,region_hash}.rs`; anchored-identity check present; **UPGRADE from "record" to "REFUSE"** (see §5 Fix #5) |
| WBGV-01 | Auto-derive change class with symmetric demotion-direction coverage (Input→Constant, source flips) — fixes CR-01 | **Already implemented** `change_class/mod.rs:165-255`; verify the symmetric test suite comes over green |
| WBGV-02 | Strictest-policy reducer: assumption (yellow-cell) change hard-blocks even amid hot-reloadable deltas | **Already implemented** `change_class/mod.rs:351-359` `effective_policy` via derived `Ord` on `GatePolicy`; assumption → `NeverAutoPromote` |
| WBGV-03 | Distinguish numeric drift from semantic redefinition via a stable canonical IR sub-DAG identity hash | Lift `change_class/ir_identity.rs` (421 LOC) `ir_subdag_hash` + `schema_diff.rs` `diff_outputs` |
| WBGV-04 | Golden-corpus gate blocks over-tolerance named-output delta unless a fingerprint-matching `ApprovalRecord` covers the candidate | Lift `gate/{mod,corpus,governed_artifact}.rs`; **NEW: auto-derive the corpus (D-09)** instead of BA-curated cases |
| WBGV-05 | Record approval via `--accept --approver <X> --effective-date <D>`, re-baselining the corpus + writing a fingerprint-bound `ApprovalRecord` | Lift `gate/accept.rs` (265 LOC) `accept`; library verb here, CLI flag in Phase 94 |
| WBGV-06 | Promotion writes the new bundle to its own `@<next_version>` dir, never overwrites baseline — fixes CR-02 | **Already implemented** `commands/compile_workbook.rs:603-611`; verify the promote-twice integration test comes over green |
| WBGV-07 | Enum inputs skip Variable-tier assignment (no out-of-enum empty-string seed) — fixes WR-01 | **Already implemented** `artifact/mod.rs:121-130` `ratify_tiers`; verify the COMMITTED-manifest invariant test |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `.xlsx` ingest + cached-value oracle capture | **Offline compiler** (`pmcp-workbook-compiler`, umya-owning) | — | umya the reader MUST stay offline; the oracle is captured once at compile, never at serve |
| Dialect lint (whitelist-only) | Offline compiler | `pmcp-workbook-dialect` (contract) | The WHITELIST/`DialectRules` live in dialect; the running linter executes here against a real `WorkbookMap` |
| Manifest synthesis (colour/Guide/headers→roles) + ratification | Offline compiler | `pmcp-workbook-runtime` (`manifest_model` types) | Synthesis is build-time; the model types it emits live in the reader-free runtime so the served binary deserializes them |
| Formula parse + DAG compile + penny-reconcile | Offline compiler | `pmcp-workbook-runtime` (IR + `sheet_ir` executor + `rounding`) | Whitelist-at-parse-time is the security primitive (compiler); the executor + rounding helpers are shared types in runtime |
| Bundle emit (seven members + BUNDLE.lock) | Offline compiler | `pmcp-workbook-runtime` (`build_bundle_lock`, `fold_evidence_hash`) | The lock/hash helpers are shared so the served loader recomputes the same combined hash (boot integrity) |
| Provenance/freshness gate (umya-fabrication refusal) | Offline compiler (quarantined `quick-xml`/`zip` raw reader) | — | Reads ORIGINAL on-disk bytes; the raw reader is `pub(crate)`, never re-exported, never enters the served tree |
| Change-class router + strictest-policy reducer | Offline compiler | `pmcp-workbook-runtime` (`ChangeClass`, `VersionChangelog`) | The classifier is offline; the `ChangeClass` enum it produces is shared so the served `diff_version` tool reads the SAME definition |
| Promote gate (corpus replay + `ApprovalRecord` + `--accept`) | Offline compiler | — | Build-time governance; the corpus + approvals live in a reviewable on-disk dir, never in the served binary |
| **Served deserialization of the emitted bundle** | `pmcp-server-toolkit::workbook` (Phase 92, DONE) | `pmcp-workbook-runtime` | The compiler's OUTPUT contract — already built and frozen; Phase 93 must emit bytes this consumer accepts |

## Standard Stack

### Core (compiler crate `pmcp-workbook-compiler` — offline, reader-bearing)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp-workbook-runtime` | path (0.1.0) | Re-export source for IR/manifest/bundle-lock/changelog/sheet_ir/finding types; the compiler BUILDS them, runtime EXECUTES them | `[VERIFIED: in-repo Cargo.toml]` The reader-free leaf is already published in-repo; compiler depends one-directionally |
| `pmcp-workbook-dialect` | path (0.1.0) | `WHITELIST` + `DialectRules` the WBDL-03 linter enforces | `[VERIFIED: in-repo crates/pmcp-workbook-dialect/src/lib.rs]` Phase 91 shipped the contract; this phase adds the running linter |
| `umya-spreadsheet` | `3.0` (latest `3.0.0`) | The ONE Excel reader: cells, formulas, cached values, colours, named ranges, DV lists, custom sheets | `[ASSUMED]` (discovered from lighthouse Cargo.toml + STACK.md; crates.io confirms 3.0.0 but provenance is non-authoritative) The only mature pure-Rust full-surface reader; MUST stay confined here |
| `quick-xml` | `0.37` (pin to umya's transitive lock — NOT current `0.40.1`) | Quarantined provenance raw reader: parse `calcPr@calcId` + `<Application>` from `docProps/app.xml` | `[ASSUMED]` Pin must match umya's own `quick-xml`; re-derive via `cargo tree -p umya-spreadsheet -i quick-xml` at extraction |
| `zip` | `8` (latest stable `8.6.0`; **avoid `9.0.0-pre2`**) | Quarantined `.xlsx` ZIP-container part reader for the provenance probe | `[ASSUMED]` Match umya's transitive `zip`; `9.0.0` is pre-release only |
| `serde` / `serde_json` | `1` | Model (de)serialization, bundle artifact JSON I/O | `[VERIFIED: in-repo root Cargo.toml]` workspace-standard |
| `schemars` | `1.0` (features `preserve_order`, `chrono04`) | `outputSchema` / manifest JSON-Schema projection | `[VERIFIED: in-repo root Cargo.toml]` SDK already pins `schemars = "1.0"` |
| `sha2` | `0.11` | bundle content hashes, candidate fingerprint | `[VERIFIED: in-repo pmcp-code-mode Cargo.toml]` matches the SDK pin |
| `hex` | `0.4` | hash hex encoding | `[VERIFIED: in-repo]` matches the SDK pin |
| `thiserror` | `2` | compiler error enums | `[VERIFIED: in-repo toolkit crates]` SDK standardized on `2` (lighthouse uses `1` — bump on lift) |
| `chrono` | `0.4` (`clock`, `serde`, `std`) | effective-date / approval timestamps | `[VERIFIED: in-repo root Cargo.toml]` matches SDK root pin |

### Supporting (runtime crate — already in-repo, NOT re-added here)

The served-binary writer `rust_xlsxwriter 0.95` already lives in `pmcp-workbook-runtime`. The compiler does NOT add it — it only re-exports the runtime's render types.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled formula/DAG (lift verbatim) | `formualizer` / `xlformula_engine` / `petgraph` | **NO** — no off-the-shelf engine enforces the dialect whitelist at parse time; adopting one LOSES the core security property. `petgraph` would add weight for a ~200-LOC Kahn toposort |
| Pure-Rust `scalar_eval` + `sheet_ir` (in runtime) | `pmcp-code-mode` SWC JS oracle (lighthouse compiler links it) | **DROP SWC.** The in-repo runtime already replaced the JS kernel with a pure-Rust scalar evaluator + `sheet_ir` executor. **Open question O-1 (verify):** confirm no reconcile-parity gap before dropping; if a gap exists, gate behind a non-default `js-oracle` feature so the default build is SWC-free. Carrying SWC by default would also be a purity-gate risk and a heavy build. |
| `umya` for the provenance probe | quarantined `quick-xml`+`zip` raw read | **NO** — umya FABRICATES `<Application>Microsoft Excel</Application>`+`calcId=122211` on every read/write; trusting its metadata defeats the freshness gate. Read ORIGINAL bytes |

**Installation (anticipated `pmcp-workbook-compiler/Cargo.toml`):**
```bash
# In-repo path deps + the three reader crates (pins re-derived from umya's lock):
# pmcp-workbook-runtime = { path = "../pmcp-workbook-runtime" }
# pmcp-workbook-dialect = { path = "../pmcp-workbook-dialect" }
# umya-spreadsheet = "3.0"
# quick-xml = "0.37"   # re-derive: cargo tree -p umya-spreadsheet -i quick-xml
# zip = "8"            # re-derive: cargo tree -p umya-spreadsheet -i zip
```

**Version verification (run at extraction):**
```bash
cargo tree -p umya-spreadsheet -i quick-xml   # pin quick-xml to THIS
cargo tree -p umya-spreadsheet -i zip         # pin zip to THIS
cargo search umya-spreadsheet --limit 1       # confirm 3.0.0 current
```

## Package Legitimacy Audit

> slopcheck was **NOT available** at research time (`command -v slopcheck` → not found; `pip install slopcheck` not run in sandbox). Per the graceful-degradation rule, the three new third-party packages are tagged `[ASSUMED]` and the planner SHOULD gate each first-install behind a `checkpoint:human-verify` task. Crates.io existence was confirmed live via `cargo search`, but registry existence alone does not confer VERIFIED status.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `umya-spreadsheet` | crates.io | mature (3.x line) | high (established) | github.com/MathNya/umya-spreadsheet | unavailable | `[ASSUMED]` — gate first install; already proven in lighthouse |
| `quick-xml` | crates.io | mature, 0.37 line | very high | github.com/tafia/quick-xml | unavailable | `[ASSUMED]` — pin to umya's transitive lock; widely used |
| `zip` | crates.io | mature, 8.x stable | very high | github.com/zip-rs/zip2 | unavailable | `[ASSUMED]` — pin to umya transitive; avoid 9.0.0-pre |

**Packages removed due to slopcheck [SLOP] verdict:** none (slopcheck unavailable)
**Packages flagged as suspicious [SUS]:** none (slopcheck unavailable)

All three are already transitively present (or directly used) in the proven lighthouse and are standard Rust-ecosystem Excel/XML/ZIP crates. The risk is **version-pin drift** (forking a second `quick-xml`/`zip` copy), not slopsquatting — mitigated by re-deriving pins from umya's lock. Verify on crates.io (not npm — cross-ecosystem confusion does not apply to a Rust phase).

## Architecture Patterns

### System Architecture Diagram

```
   BA-authored .xlsx (original bytes on disk)
        │
        ▼
 ┌─────────────────────────────────────────────────────────────────────┐
 │  pmcp-workbook-compiler  (umya/quick-xml/zip confined HERE)          │
 │                                                                     │
 │  ingest::ingest(path) ──► WorkbookMap (cells, formulas, cached <v>, │
 │   (umya read)              colours, DV lists, named ranges)         │
 │        │                                                            │
 │        ├──► provenance::gate(ORIGINAL bytes, map, manifest)         │
 │        │      (quarantined quick-xml/zip raw read of calcPr +       │
 │        │       <Application>) ──► OracleCorpus  OR  REFUSE          │
 │        │       └─ umya-fabricated identity → oracle/non-excel-app   │
 │        │                                                            │
 │        ├──► dialect::lint(map, DialectRules) ──► LintReport         │
 │        │      (whitelist-only; collect-all located findings)       │
 │        │                                                            │
 │        ├──► manifest::synthesize(map) ──► candidate Manifest        │
 │        │      (colour/Guide/headers→roles; inline DV→enum ≤10;      │
 │        │       range-DV→dynamic input + WARNING) ──► ratify()       │
 │        │                                                            │
 │        ├──► formula::parse (Pratt, whitelist-at-parse) +            │
 │        │      dag::build_dag (Kahn toposort) ──► HashMap<Cell>      │
 │        │                                                            │
 │        ├──► reconcile::reconcile (run IR via runtime executor;      │
 │        │      grade vs OracleCorpus under operand-anchored          │
 │        │      rounding) ──► named-output mismatch = ERROR,          │
 │        │      helper mismatch = WARNING (D-03)                      │
 │        │                                                            │
 │   ┌────┴── GATE (before any write) ──────────────────────────────┐ │
 │   │  change_class::classify(prev, cur, prev_ir, cur_ir)          │ │
 │   │   ──► [(ChangeClass, region)]  (symmetric, demotion-aware)   │ │
 │   │  effective_policy(classes) ──► HotReload|Block|NeverPromote  │ │
 │   │  gate::gate(auto-derived corpus replay)                      │ │
 │   │   ──► over-TOL named-output delta? block + print deltas +    │ │
 │   │       change class + exact `--accept` command (D-10)         │ │
 │   │  accept(--approver --effective-date) ──► ApprovalRecord      │ │
 │   │   (content-hash fingerprint) re-baselines corpus            │ │
 │   └──────────────────────────────────────────────────────────────┘ │
 │        │  (gate passed OR first version no-op D-12)                 │
 │        ▼                                                            │
 │  artifact::emit_bundle (ratify_tiers: skip enum inputs WR-01)      │
 │   ──► {bundle_id}@{version}/  (NEW dir; never overwrite — CR-02)   │
 └────────┬───────────────────────────────────────────────────────────┘
          ▼
  Seven-member bundle  ◄── THE CONTRACT (frozen Phase 92)
  manifest.json / executable.ir.json / cell_map.json / layout.json /
  BUNDLE.lock / evidence/{changelog.json, parser_equivalence.json, …}
          │
          ▼  (deserialized by, NO reader)
  pmcp-server-toolkit::workbook  (Phase 92, DONE — the output consumer)
```

### Recommended Project Structure (mirror the lighthouse, Claude's discretion)
```
crates/pmcp-workbook-compiler/src/
├── lib.rs              # crate docs + re-export surface + generic compile_workbook driver
│                       #   (NO build_reference_manifest — deleted per §5 Fix #1)
├── ingest/             # umya → WorkbookMap (WBCO-01); cell_map.rs
├── dialect/            # the running linter (WBDL-03): linter.rs, finding.rs, rules.rs, mod.rs
├── manifest/           # synth.rs (WBCO-02/06), ratify.rs, projections.rs, model.rs, mod.rs
├── formula/            # token.rs, parser.rs (Pratt, whitelist-at-parse, WBCO-03), rebase.rs
├── dag/                # graph.rs, resolve.rs, topo.rs (Kahn, WBCO-03)
├── sheet_ir/           # COMPILER-SIDE pieces only (eval_bridge, executor wiring);
│                       #   the executor/rounding TYPES come from runtime (DO NOT re-declare)
│                       #   DO NOT lift loop_exec.rs / RoomAggregator (deferred row-block)
├── reconcile/          # classifier.rs (operand-anchored, delta.abs grep-gated), drift.rs, mod.rs (WBCO-04)
├── provenance/         # gate.rs (WBCO-07 REFUSE), raw_parts.rs (pub(crate) quick-xml/zip), region_hash.rs
├── artifact/           # emit_bundle, bundle_lock, cell_map, evidence, executable, layout (WBCO-05)
├── change_class/       # mod.rs (CR-01 symmetric), ir_identity.rs (WBGV-03), schema_diff.rs
├── gate/               # mod.rs, corpus.rs (+ NEW auto-derive D-09), accept.rs, governed_artifact.rs
└── stage1.rs           # composed collect-all analysis pass (lint+synth+freshness+drift in one ingest)
```

### Pattern 1: Re-export runtime shared types; never re-declare
**What:** The compiler `pub use`s `Expr`/`Dag`/`Manifest`/`CellRole`/`ChangeClass`/`VersionChangelog`/`Cell`/`sheet_ir::run`/`build_bundle_lock`/`rounding::*` FROM `pmcp_workbook_runtime`, so its call sites compile against the historical names while the served binary deserializes the SAME types from the reader-free crate.
**When to use:** Every shared model/IR/hash/changelog/finding type. The lighthouse `lib.rs:120-230` is the verbatim template (re-export list).
**Example (lighthouse `workbook-compiler/src/lib.rs:164`):**
```rust
// Source: lighthouse workbook-compiler/src/lib.rs
pub use workbook_runtime::{self, toposort, BinOp, CellValue, Dag, ExcelError, Expr, UnOp};
pub use workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};
// In the SDK these become `pmcp_workbook_runtime::{...}`.
```

### Pattern 2: Operand-anchored reconcile, never blanket tolerance
**What:** A `computed != cached` divergence is acceptable ONLY if the deciding cell is a rounding op (`ROUND`/`ROUNDUP`/`CEILING`), its operand sits within `BOUNDARY_EPSILON = 1e-6` of the rounding boundary, AND the divergence is ≤ one rounding step. A grep gate asserts `delta.abs()` never appears in `reconcile/classifier.rs`.
**When to use:** All penny-reconciliation (WBCO-04). Lift `reconcile/classifier.rs` verbatim; it already imports `workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup}`.
**Example (lighthouse `reconcile/classifier.rs:18-21`):**
```rust
// Source: lighthouse workbook-compiler/src/reconcile/classifier.rs
//   A rule that branches on `delta.abs() < X` is FORBIDDEN — a grep gate asserts
//   `delta.abs()` never appears in this file.
const BOUNDARY_EPSILON: f64 = 1e-6;
```

### Pattern 3: Gate-before-write (build-candidate → gate → write split)
**What:** `build_candidate_model` produces an in-memory `CandidateBundle` (everything EXCEPT the final write); the gate runs on it; `write_candidate_bundle(candidate, out_root, EmitLane)` commits ONLY on a clean gate. `EmitLane::{Seed, GatedUpdate}` enforces the changelog `from_version` shape (empty for seed, non-empty differing for promote) so a malformed lane writes ZERO bytes.
**When to use:** The whole compile pipeline. CR-02's fix (set `candidate.version = next_version` before a `GatedUpdate` write) lives here.
**Example (lighthouse `commands/compile_workbook.rs:603-611`):**
```rust
// Source: lighthouse workbook-compiler/src/commands/compile_workbook.rs
candidate.version = next_version;  // CR-02: write lands in a NEW {name}@{next_version}/ dir
crate::write_candidate_bundle(&candidate, out_root, crate::EmitLane::GatedUpdate)
```

### Pattern 4: CR-01 symmetric classification (assumption-first hard rule)
**What:** `classify_cell_roles` inspects BOTH prior and current roles. Assumption involvement on EITHER side → `Assumption` (NeverAutoPromote). A role flip AWAY from `Input`/`Output` → `InputSchema`/`OutputSchema`. The `Constant | Formula => {}` arm can no longer silently drop a demotion.
**When to use:** Lift `change_class/mod.rs:165-255` verbatim — it is ALREADY symmetric. Verify the demotion test suite (`assumption_demotion_to_plain_constant`, `input_demoted_to_constant`, `output_demoted_to_formula`, `enum_drop_with_role_flip`) comes over green.

### Anti-Patterns to Avoid
- **Copying `build_reference_manifest` onto the emit path:** the ONE surviving §5 gap. It hand-builds a manifest with literal cell addresses; a second workbook would be served the wrong schema. Delete from every non-test path; keep ONLY as an anti-drift TEST fixture.
- **Lifting `loop_exec.rs` / `RoomAggregator` / `run_with_loop`:** row-block iteration is deferred (arbitrary-N is the hardest parser problem). Out of scope.
- **Carrying `pmcp-code-mode`/SWC into the compiler by default:** the runtime already has pure-Rust eval; SWC is heavy and a purity risk. Drop (or non-default `js-oracle` feature only if a verified reconcile gap exists).
- **Trusting umya's `<Application>`/`calcId`:** umya fabricates `Microsoft Excel`/`122211`. Read original bytes; REFUSE umya-stamped identity.
- **Re-declaring runtime types locally** (a second `Manifest`/`ChangeClass`): the served `diff_version`/loader would read a DIFFERENT definition. Re-export only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bundle integrity hash-of-hashes | A new combined-hash fold | `pmcp_workbook_runtime::{build_bundle_lock, fold_evidence_hash, update_field, sha256_hex}` | The served loader recomputes with the SAME helpers; a divergent fold breaks boot integrity |
| Excel rounding (round-half-away-from-zero) | `f64::round` / custom rounding | `pmcp_workbook_runtime::sheet_ir::rounding::{excel_round, excel_roundup, excel_ceiling}` | Rust stdlib is half-to-even; Excel differs at .5 — the reconcile classifier anchors on these exact helpers |
| Change-class enum + routing | A local `ChangeClass` / policy table | `pmcp_workbook_runtime::ChangeClass` + lifted `change_class::{policy, effective_policy}` | The served `diff_version` tool shares the enum; the strictest-policy reducer is derived `Ord` on `GatePolicy` |
| Approval fingerprint | A version-label-keyed approval | Lifted `gate::corpus::candidate_fingerprint` (sha256 over prev-hash + candidate-hash + region deltas) | WR-04: version labels are forgeable across contents; content-hash binding closes the inherit-an-old-approval hole |
| Provenance identity check | umya metadata trust / `.contains` substring | Lifted `provenance::gate` anchored `.starts_with("Microsoft Excel")` over raw bytes | umya fabricates identity; `.contains` let "FauxMicrosoft Excelerator" pass (WR-03) |
| Formula whitelist enforcement | A post-parse name filter | Lifted `formula::parser` whitelist-AT-PARSE-time | An out-of-whitelist function must be a parse-time REJECTION, not a silent accept — the dialect security primitive |
| DAG toposort | `petgraph` | Lifted `dag::topo` (Kahn, ~200 LOC) | Keeps the runtime serde-clean + zero-dep |

**Key insight:** the runtime leaf (Phases 91–92) already owns the entire OUTPUT type system + hashing + rounding + executor. The compiler's job is to *produce* these types from a workbook, not to re-define any of them. Almost everything "hard" (hashing, rounding, IR execution, change-class enum) is a re-export, not new code.

## Runtime State Inventory

> This phase is a **rename/refactor/extraction** (lifting + scrubbing customer identifiers from a private repo into the SDK). The Runtime State Inventory applies to the SCRUB surface.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | None — the compiler is a pure offline transform (input `.xlsx` → output bundle dir). No database, datastore, or cached runtime state stores a renamed string. The committed `tax-calc@1.1.0` golden is the only persisted artifact and is already neutral (verified: zero customer names in `manifest.json`). | None — verified by inspecting the golden + the stateless pipeline |
| **Live service config** | None — no external service holds compiler state. The compiler is build-time tooling only. | None — verified (offline cone, no served component) |
| **OS-registered state** | None — no Task Scheduler / launchd / systemd / pm2 registration; the compiler runs on demand. | None |
| **Secrets/env vars** | None — the compiler reads no secrets; `REFERENCE_WORKBOOK_PATH` env-style consts in the lighthouse `lib.rs:863-872` point at a customer workbook and must be DELETED (replaced by the generic driver's path parameter). | Delete the hardcoded reference-workbook path consts; the generic driver takes a path argument |
| **Build artifacts / installed packages** | **Customer-identifier scrub surface: ~34 lighthouse source files** contain `ufh`/`towelrad`/`coil`/`heat_source`/`heat_pump`/`plot3`/`quote`/`radiator`/`underfloor` tokens (verified via grep). Every lifted file must be scrubbed per Phase 92 D-13: no customer names in SDK code, comments, fixtures, or docs. The `WORKFLOW_NAME = "ufh-quote"` const (`lib.rs:306`), `build_reference_manifest`'s literal cells (`7_Quote!C9`, `1_Inputs!C6`, `heat_source`), `renderer_equivalence_governed` (margin/coil constants), and `author_lighthouse_dv.rs` bin all carry customer business logic — scrub or omit. | Scrub all 34 files; delete customer-specific functions (`build_reference_manifest`, `emit_reference_bundle`, `renderer_equivalence_governed`, `author_lighthouse_dv`, the supply-total/coil IR builders); replace with the generic driver + a neutral fixture |

**The canonical question (after every file is lifted):** what customer string still survives in SDK code/comments/fixtures/docs? Answer: the scrub is a per-file task — a grep gate (`grep -ri 'ufh\|towelrad\|coil\|heat_source\|heat_pump\|underfloor\|radiator' crates/pmcp-workbook-compiler/`) must return ZERO matches in non-test, non-fixture paths and only neutral domain names (e.g. tax-calc) in fixtures. This is a verification step the planner must add.

## Common Pitfalls

### Pitfall 1: Planning the §5 fixes as net-new bug fixes
**What goes wrong:** The §5 RFC (dated 2026-06-10) and the milestone PITFALLS.md describe CR-01/CR-02/WR-01 as *unfixed debt to repair at extraction*. But the **current lighthouse already implements all five** — the classifier is symmetric, the promote is versioned, enum-tiering is skipped, the fingerprint is content-hash-bound, and the provenance identity is anchored.
**Why it happens:** The research snapshot pre-dates the lighthouse's own fix commits. Plan tasks written as "implement CR-01" would re-derive code that already exists.
**How to avoid:** Plan these as **invariants to verify survive the lift**, not new work. Each fix has a test/property/integration suite in the lighthouse (`change_class/mod.rs` demotion tests, the promote-twice test in `commands/compile_workbook.rs`, `ratify_skips_frozen_enum_inputs`). Lift the code AND its tests; the verification is "the test is green in `pmcp-workbook-compiler`."
**Warning signs:** A plan task that says "design the CR-01 symmetric classifier" rather than "lift `change_class/mod.rs` and confirm its 8 demotion tests pass."

### Pitfall 2: The `Manifest.annotations` struct delta (in-repo vs lighthouse)
**What goes wrong:** The in-repo `pmcp_workbook_runtime::Manifest` (Phase 92 D-18) has an `annotations: Vec<AnnotationDecl>` field (the tax-calc golden carries two bracket-boundary annotations). The lighthouse `Manifest` does NOT — it has no `annotations` field. The compiler synth must populate (or default-empty) `annotations`, and `build_candidate_model`'s hand-built `Manifest { … }` literals will not compile against the in-repo struct unless every field is supplied.
**Why it happens:** The runtime was lifted + extended in Phases 91-92; the compiler is being lifted from an older shape.
**How to avoid:** Reconcile the `Manifest` constructor sites against the in-repo struct (add `annotations: vec![]` or synthesize from a Guide-annotation convention). The byte-identical re-emit test (below) will fail loudly if `annotations` is wrong. Also reconcile `Dtype` (in-repo: `Number`/`Text`/`Bool`) and `Role` variants.
**Warning signs:** Compile errors on `Manifest { … }` literals; a re-emitted manifest whose JSON omits `annotations`.

### Pitfall 3: There is no `tax-calc.xlsx` — the golden was generated synthetically
**What goes wrong:** The producer/consumer proof ("re-emit tax-calc and diff byte-identical") assumes a source `.xlsx` to compile. There is **none** — `tax-calc@1.1.0` was built by `tests/support/fixture_gen.rs` directly from the runtime's Serialize types, NOT compiled from a workbook.
**Why it happens:** Phase 92 needed a golden to freeze the consumer contract before the compiler existed; synthesizing it from runtime types was the fastest path.
**How to avoid:** Phase 93 must AUTHOR a neutral `tax-calc.xlsx` (a real workbook: 3 inputs incl. an enum, a governed bracket table, multiple named outputs, real formulas with cached values) such that compiling it through the new pipeline reproduces the committed golden — OR adjust the golden generator and re-commit if exact byte-parity proves infeasible. **Open question O-2:** decide whether the proof is (a) author-xlsx-then-compile-to-byte-identical, or (b) compile-an-xlsx-then-assert-structural-equivalence-to-golden + regenerate the golden from the compiler. The CONTEXT calls the golden "the producer/consumer agreement target" — byte-identical re-emit is the strongest proof; flag the xlsx-authoring cost. **Authoring the fixture .xlsx must NOT use umya's write path** (fabricated provenance) — use real Excel or `rust_xlsxwriter`, or the provenance gate will refuse it.

### Pitfall 4: Purity-boundary erosion (the milestone's #1 trap)
**What goes wrong:** umya/quick-xml/swc leaks into a served-tree crate via a shared type, a feature-unification edge, or a "convenience" compiler helper in the toolkit.
**Why it happens:** The boundary is enforced by `make purity-check` lists, not the type system.
**How to avoid:** The compiler is the ONE crate where umya/quick-xml/zip are ALLOWED — it is NOT added to `PURITY_CRATES`. But add a NEW positive-and-negative assertion: `cargo tree -p pmcp-workbook-compiler -i umya` MUST be non-empty (the reader IS here), and re-run the existing per-served-crate negative assertions (runtime, dialect, toolkit[workbook], toolkit[workbook-embedded]) to confirm the compiler's existence did not leak the reader into them via the shared runtime dep. The existing Makefile (`PURITY_CRATES`/`PURITY_WRITER_CRATES` lists at lines 496-497) documents the extension procedure: "Adding a reader-free crate in a later phase (92-96): append it to PURITY_CRATES." The compiler is the EXCEPTION — do NOT append it; instead add a compiler-specific positive assertion + a `quick-xml`/`zip` transitive-pin guard.
**Warning signs:** `cargo tree -p pmcp-workbook-runtime` grows an `umya`/`quick-xml` line after the compiler lands; a second `quick-xml`/`zip` version in `Cargo.lock`.

### Pitfall 5: WBCO-07 is currently "record", not "REFUSE"
**What goes wrong:** The lighthouse `provenance/gate.rs:250-275` emits the `oracle/non-excel-app` finding for a non-Excel `<Application>` but the comment says "version recorded, NOT gated" for the app-version axis — and for a umya-AUTHORED workbook the identity *starts with* "Microsoft Excel" (umya fabricates exactly that), so the anchored `.starts_with` check PASSES it. The current gate refuses non-Excel-NAMED apps but does NOT refuse umya's fabricated-Excel identity.
**Why it happens:** The lighthouse's own fixtures are real-Excel-saved; the umya-fabrication case is a known caveat documented as a "planning-record note" (`lib.rs:521-523`), not yet a hard refusal.
**How to avoid:** Phase 93's WBCO-07 success criterion is explicit: a umya-stamped workbook is REFUSED with `oracle/non-excel-app` (or a distinct `oracle/umya-fabricated` class). This needs a NEW signal beyond `.starts_with("Microsoft Excel")` — e.g. detect umya's exact `calcId=122211` fingerprint, OR detect the absence of an Excel-specific app-version/build string, OR (cleanest) require a positive Excel-recalc marker umya does not write. **Open question O-3:** the exact umya-fabrication detection signal (calcId sentinel vs app-version absence vs a positive marker). Add the regression test: author a workbook with umya, assert the gate REFUSES it.
**Warning signs:** A umya-round-tripped fixture passing the freshness gate; `calcId == 122211` in any accepted `OracleProvenance`.

### Pitfall 6: D-09 auto-corpus — the genuinely new design
**What goes wrong:** The lighthouse corpus is BA-curated `cases.json`. D-09 forbids asking the BA to author cases; the corpus must be auto-derived from manifest defaults + enum domains. A naive "all enum combinations × all defaults" grid explodes combinatorially.
**Why it happens:** Auto-derivation is new; the lighthouse never needed it.
**How to avoid:** Bound the grid (Claude's discretion): each input at its default, plus one case per enum member (holding others at default), plus boundary cases for numeric inputs (e.g. default ± a step). The corpus replays BOTH the prior accepted version's IR and the candidate's IR over this grid; the prior version's outputs ARE the golden (captured automatically — "the previous version's own behavior"). On first version (D-12) the gate is a no-op that establishes the baseline. **Open question O-4:** the exact grid-size policy (how many enum/boundary cases; cap at N total). Reuse the lighthouse `ApprovalCase`/`expected_outputs` BTreeMap shape, `candidate_fingerprint`, and `accept` verbatim — only the case *generation* is new.

## Code Examples

### Manifest-driven generic compile driver (replaces `build_reference_manifest`)
```rust
// Source: NEW (Phase 93) — derived from lighthouse build_candidate_model
//   (workbook-compiler/src/lib.rs:443) with the hand-built manifest REMOVED.
// The shape: ingest → stage1 (lint+synth+freshness+drift) → parse+DAG →
// reconcile → emit, with the manifest coming SOLELY from synthesize→ratify.
pub fn compile_workbook(workbook_path: &Path, out_root: &Path /* … */)
    -> Result<BundleLock, CompileError> {
    let bytes = std::fs::read(workbook_path)?;            // ORIGINAL bytes (provenance)
    let (map, _findings) = ingest::ingest(workbook_path)?; // umya read
    let stage1 = stage1::run_stage1(&bytes, &map, /* no governed literals */, policy)?;
    let mut manifest = manifest::synthesize(&map);        // colour/Guide/headers → roles
    manifest::ratify(&mut manifest, &bytes, approver, &sidecar)?;
    let ir = dag::build_dag(&formula::parse_all(&map)?)?; // whitelist-at-parse
    reconcile::reconcile(&ir, &stage1.oracle, &manifest)?; // named-out=ERR, helper=WARN
    // … change_class gate (vs prior baseline) → emit on clean gate …
}
```

### Anchored provenance identity (lift verbatim; UPGRADE to refuse umya)
```rust
// Source: lighthouse workbook-compiler/src/provenance/gate.rs:255
let is_excel = app.application.as_deref()
    .is_some_and(|a| a.trim_start().starts_with("Microsoft Excel"));
// Phase 93 ADDS: detect umya's fabricated identity (e.g. calcId == 122211 sentinel)
// and REFUSE with oracle/non-excel-app even when the <Application> string matches.
```

## State of the Art

| Old Approach (RFC §5 snapshot) | Current Approach (lighthouse HEAD, verified) | Impact |
|--------------------------------|----------------------------------------------|--------|
| CR-01: demotions escape classification → auto-promote | `classify_cell_roles` is symmetric (assumption-first; flip-away arms) | Fix is DONE — lift + verify, don't re-implement |
| CR-02: promote overwrites `@1.0.0` baseline | `candidate.version = next_version` before `GatedUpdate` write | Fix is DONE — lift + verify the promote-twice test |
| WR-01: enum input seeded `Variable{default:""}` | `ratify_tiers` skips `allowed_values.is_some()` inputs | Fix is DONE — lift + verify committed-manifest invariant |
| WR-04: approval keyed on version label | `candidate_fingerprint` binds prev+candidate content hashes | Fix is DONE — lift verbatim |
| Provenance `.contains("Microsoft Excel")` | `.starts_with("Microsoft Excel")` anchored | WR-03 fixed; but umya-fabrication refusal still NET-NEW (Pitfall 5) |
| `build_reference_manifest` hand-built | **STILL PRESENT** in lighthouse `lib.rs:769` | The ONE real §5 gap — Phase 93 deletes it (genuinely new) |
| BA-curated `cases.json` corpus | **STILL BA-curated** in lighthouse | D-09 auto-derivation is genuinely new (Pitfall 6) |
| Compiler links `pmcp-code-mode`/SWC JS oracle | In-repo runtime has pure-Rust `scalar_eval`+`sheet_ir` | Drop SWC from the SDK compiler (verify no reconcile gap) |

**Deprecated/outdated for this lift:**
- `pmcp-code-mode`/SWC JS oracle: the runtime replaced it; do not carry it.
- `loop_exec.rs` / `RoomAggregator` / `run_with_loop`: deferred row-block feature; do not lift.
- The hardcoded `REFERENCE_WORKBOOK_PATH` consts + customer IR builders: delete (scrub).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `umya-spreadsheet 3.0` / `quick-xml 0.37` / `zip 8` are the correct pins (re-derive from umya's lock) | Standard Stack | A forked second `quick-xml`/`zip` copy in `Cargo.lock`; purity-pin drift. Mitigated by `cargo tree -i` re-derivation task |
| A2 | The in-repo runtime's `scalar_eval`+`sheet_ir` fully covers penny-reconcile parity without the SWC JS oracle | Alternatives Considered (O-1) | If a parity gap exists, reconcile fails on the golden; mitigation: non-default `js-oracle` feature. MUST verify against the lighthouse Phase-10 reconcile path before dropping SWC |
| A3 | All five §5 fixes in the current lighthouse are complete + correct (verified by reading the code + their tests, not by running them) | Summary / State of the Art | If a fix has a residual hole, the lifted test catches it; low risk because tests are present and the code matches the documented patches |
| A4 | The producer/consumer proof can be a re-emit that diffs byte-identical against `tax-calc@1.1.0` | Pitfall 3 (O-2) | No `tax-calc.xlsx` exists; authoring one to byte-parity may be costly. Fallback: structural-equivalence + regenerate golden from the compiler |
| A5 | The umya-fabrication refusal signal (calcId sentinel / app-version absence / positive marker) is detectable from raw bytes | Pitfall 5 (O-3) | If umya's fabrication is indistinguishable from real Excel, WBCO-07's hard-refuse cannot be met cleanly; needs the exact detection design |
| A6 | A bounded auto-derived corpus grid (defaults + per-enum-member + numeric boundary) satisfies D-09 | Pitfall 6 (O-4) | An under-covered grid misses a regression; an over-covered grid is slow. Grid-size policy is Claude's discretion but needs a decision |
| A7 | The lighthouse `Manifest` lacks `annotations`; the in-repo runtime `Manifest` has it (Phase 92 D-18) | Pitfall 2 | Constructor mismatch → compile errors (caught at build); synth must populate/default `annotations` |

## Open Questions (RESOLVED)

> All four open questions were resolved during planning (Phase 93 plan-phase).
> Resolution summary (each cites the plan/task where the decision lives):
> - **O-1 (SWC/JS oracle necessity):** RESOLVED — Plan 93-04 Task 2 verifies pure-Rust `scalar_eval` + `sheet_ir` reconcile parity; SWC/`pmcp-code-mode` dropped by default (gate behind a non-default `js-oracle` feature only if a parity gap appears).
> - **O-2 (producer/consumer proof shape):** RESOLVED — `checkpoint:decision` gate in Plan 93-07 Task 2, both options fully specified (author a neutral `tax-calc.xlsx` for byte-identical re-emit, or structural-equivalence + regenerate golden); either routes to full delivery in Task 3.
> - **O-3 (umya-fabrication detection signal):** RESOLVED — Plan 93-02 Task 2 detects the `calcId=122211` sentinel and/or absence of an Excel build/app-version string, with a umya-author → REFUSE regression test; chosen signal documented in the task action.
> - **O-4 (auto-corpus grid policy):** RESOLVED — Plan 93-06 Task 1 uses defaults + one case per enum member (others held at default) + numeric boundary cases, capped at a small documented N; policy documented in the plan SUMMARY.

1. **O-1 (SWC/JS oracle necessity):**
   - What we know: in-repo runtime ships pure-Rust `scalar_eval` + `sheet_ir` executor; lighthouse compiler links `pmcp-code-mode` with `js-runtime` as the offline reconcile oracle.
   - What's unclear: whether pure-Rust eval reconciles the golden to the penny without the JS oracle.
   - Recommendation: verify against the lighthouse Phase-10 reconcile path during planning; default to DROPPING SWC. If a gap appears, gate behind a non-default `js-oracle` feature (default build SWC-free). Do NOT carry SWC by default (heavy + purity risk).

2. **O-2 (producer/consumer proof shape):**
   - What we know: `tax-calc@1.1.0` is committed but was synthetically generated; no source `.xlsx` exists.
   - What's unclear: author-a-real-xlsx-to-byte-identical vs structural-equivalence + regenerate-golden-from-compiler.
   - Recommendation: prefer authoring a neutral `tax-calc.xlsx` (NOT via umya) and proving byte-identical re-emit (strongest). Flag the authoring cost; fallback to structural equivalence if byte-parity is intractable.

3. **O-3 (umya-fabrication detection signal):**
   - What we know: umya stamps `<Application>Microsoft Excel</Application>` + `calcId=122211`; the current anchored `.starts_with` PASSES umya identity.
   - What's unclear: the precise raw-byte signal that distinguishes umya-fabricated from real-Excel provenance for the WBCO-07 hard refuse.
   - Recommendation: detect the `calcId=122211` sentinel and/or the absence of an Excel build/app-version string; add the umya-author → REFUSE regression test.

4. **O-4 (auto-corpus grid policy):**
   - What we know: D-09 forbids BA-authored cases; the corpus auto-derives from manifest defaults + enum domains.
   - What's unclear: exact grid size/shape (how many enum/boundary cases; total cap).
   - Recommendation: defaults + one case per enum member + numeric boundary cases, capped at a small N; Claude's discretion, but decide and document the policy in the plan.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable toolchain | whole build | ✓ (assumed; CI uses dtolnay stable) | latest stable | — |
| `cargo tree` | purity-pin re-derivation + gate | ✓ (cargo builtin) | — | — |
| `cargo-deny` | purity gate Layer 2 (already wired) | ✓ (CI installs it) | 0.18.3 | — |
| `umya-spreadsheet 3.0.0` | WBCO-01 ingest | ✓ (crates.io confirmed live) | 3.0.0 | none — the only full-surface reader |
| `quick-xml` (umya transitive) | WBCO-07 provenance | ✓ (via umya) | 0.37.x (re-derive) | none |
| `zip` (umya transitive) | WBCO-07 provenance | ✓ (via umya) | 8.6.0 (re-derive) | avoid 9.0.0-pre |
| `make purity-check` target | purity gate | ✓ (exists, Makefile:500) | — | — |
| Real Excel or `rust_xlsxwriter` for fixture authoring | producer/consumer proof (O-2) | partial — `rust_xlsxwriter` in runtime; real Excel may be unavailable in CI | 0.95 | author fixture offline + commit the `.xlsx` |
| `slopcheck` | package legitimacy audit | ✗ | — | mark packages `[ASSUMED]`; gate first install behind checkpoint |

**Missing dependencies with no fallback:** none block execution (umya/quick-xml/zip are crates.io-available; slopcheck absence is handled by graceful degradation).
**Missing dependencies with fallback:** `slopcheck` (→ `[ASSUMED]` tagging + human-verify checkpoint); real-Excel for fixture authoring (→ commit a pre-authored `.xlsx`).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test`; `proptest`/`quickcheck` for property tests; `cargo fuzz` for fuzz targets (CLAUDE.md ALWAYS requirements); `insta` snapshots (lighthouse dev-dep) |
| Config file | none beyond `Cargo.toml` `[dev-dependencies]`; CI runs with `--test-threads=1` (CLAUDE.md) |
| Quick run command | `cargo test -p pmcp-workbook-compiler --lib` |
| Full suite command | `make quality-gate` (fmt+clippy pedantic/nursery+build+test+audit) then `make purity-check` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WBDL-03 | collect-all located dialect findings with repair | unit | `cargo test -p pmcp-workbook-compiler dialect::` | ❌ Wave 0 (lift from lighthouse `dialect/linter.rs` tests) |
| WBCO-01 | ingest captures cached values as oracle | unit | `cargo test -p pmcp-workbook-compiler ingest::` | ❌ Wave 0 |
| WBCO-02 | manifest fully synth-driven; no `build_reference_manifest` on emit path | unit + grep-gate | `cargo test -p pmcp-workbook-compiler manifest::` + `! grep -rn 'build_reference_manifest' src/ --include='*.rs' | grep -v test` | ❌ Wave 0 |
| WBCO-03 | whitelist-at-parse rejection; Kahn DAG | unit + property | `cargo test -p pmcp-workbook-compiler formula:: dag::` | ❌ Wave 0 |
| WBCO-04 | operand-anchored reconcile; no `delta.abs()` | unit + **grep-gate** | `cargo test -p pmcp-workbook-compiler reconcile::` + `! grep -n 'delta.abs()' src/reconcile/classifier.rs` | ❌ Wave 0 |
| WBCO-05 | seven-member bundle emit | integration | `cargo test -p pmcp-workbook-compiler artifact::` | ❌ Wave 0 |
| WBCO-06 | inline DV ≤10 → enum; range-DV → dynamic input + reason code | unit | `cargo test -p pmcp-workbook-compiler enum_` | ❌ Wave 0 |
| WBCO-07 | umya-authored workbook REFUSED `oracle/non-excel-app` | regression | `cargo test -p pmcp-workbook-compiler provenance::umya_fabricated_refused` | ❌ Wave 0 (NEW test) |
| WBGV-01 | symmetric demotion classification | unit (lift) | `cargo test -p pmcp-workbook-compiler change_class::` | ❌ Wave 0 (lift 8 demotion tests) |
| WBGV-02 | assumption hard-blocks amid hot-reload | unit (lift) | `cargo test -p pmcp-workbook-compiler any_assumption_forces` | ❌ Wave 0 |
| WBGV-03 | IR sub-DAG hash distinguishes drift vs redefine | unit (lift) | `cargo test -p pmcp-workbook-compiler ir_identity::` | ❌ Wave 0 |
| WBGV-04 | over-tolerance delta blocks without matching approval | integration | `cargo test -p pmcp-workbook-compiler gate::` | ❌ Wave 0 |
| WBGV-05 | `--accept` records fingerprint-bound `ApprovalRecord` | integration | `cargo test -p pmcp-workbook-compiler accept::` | ❌ Wave 0 |
| WBGV-06 | promote-twice → two dirs, baseline byte-identical | integration (lift) | `cargo test -p pmcp-workbook-compiler promote_twice` | ❌ Wave 0 |
| WBGV-07 | committed manifest: no out-of-enum seeded default | invariant (lift) | `cargo test -p pmcp-workbook-compiler ratify_skips_frozen_enum` | ❌ Wave 0 |
| **producer/consumer** | re-emit reproduces `tax-calc@1.1.0` byte-identical (or structural) | integration | `cargo test -p pmcp-workbook-compiler reemit_tax_calc_golden` | ❌ Wave 0 (NEW; depends on O-2) |

### ALWAYS Requirements (CLAUDE.md — every new feature)
- **FUZZ:** a `cargo fuzz` target over the formula parser (untrusted formula bytes) and over the provenance raw reader (untrusted `.xlsx` ZIP/XML bytes) — both are attacker-controlled-input surfaces.
- **PROPERTY:** `classify(A→B)` / `classify(B→A)` symmetry-cardinality invariant; "no seeded default is ever outside its cell's `allowed_values`"; "operand-anchored reconcile never accepts a divergence larger than one rounding step."
- **UNIT:** 80%+ coverage per module (lift the lighthouse tests).
- **EXAMPLE:** a `cargo run --example compile_a_workbook` demonstrating ingest→emit on the neutral fixture (library-level; the CLI is Phase 94).

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-workbook-compiler --lib`
- **Per wave merge:** `make quality-gate && make purity-check`
- **Phase gate:** full suite green + the byte-identical re-emit + the umya-fabrication-refused regression + the `delta.abs()`/`build_reference_manifest` grep gates, before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-workbook-compiler/Cargo.toml` + crate skeleton + `[lib]` (NEW crate)
- [ ] Lift + scrub all module tests from the lighthouse (34 files carry customer identifiers)
- [ ] NEW: `provenance::umya_fabricated_refused` regression test (WBCO-07 upgrade)
- [ ] NEW: `reemit_tax_calc_golden` producer/consumer test (depends on O-2 decision + a neutral fixture `.xlsx`)
- [ ] NEW: D-09 auto-corpus generator + its grid-coverage tests
- [ ] grep-gate tests: `delta.abs()` absent in `reconcile/classifier.rs`; `build_reference_manifest` absent on non-test paths; customer-identifier grep returns zero in non-fixture paths
- [ ] Extend `make purity-check`: positive `cargo tree -p pmcp-workbook-compiler -i umya` (reader IS here) + re-run served-crate negatives + `quick-xml`/`zip` single-version guard
- [ ] fuzz targets: formula parser, provenance raw reader

## Security Domain

> `security_enforcement` is ON. The compiler ingests **untrusted/attacker-controlled `.xlsx` bytes** and exposes BA-authored strings into the served schema — both are real attack surfaces.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | **yes** | Whitelist-at-parse-time formula gate (deny-by-default); inline-DV-only enums; fail-closed reconcile (named-output mismatch blocks emit); collect-all located findings |
| V1 Architecture / Trust boundaries | **yes** | The purity boundary (reader confined to the compiler); the compile-not-interpret invariant (served binary never parses Excel) |
| V6 Cryptography | yes (hashing) | `sha2`/`build_bundle_lock`/`candidate_fingerprint` — never hand-roll; content-hash-bound approvals (WR-04) |
| V8 Data Protection / provenance | **yes** | Trusted-oracle freshness gate; umya-fabricated-provenance refusal (WBCO-07); ratification sign-off recorded |
| V12 Files / Resources | **yes** | Untrusted `.xlsx` ZIP/XML parsing (`zip`/`quick-xml`) — `PartTooLarge` guard, fail-closed on malformed parts, no panic on attacker input |
| V2 Authentication / V3 Session / V4 Access Control | no | Offline build tool; no sessions/authn (the `--accept` approver identity is an audit field, not authn) |

### Known Threat Patterns for the workbook compiler

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed/oversize `.xlsx` (zip bomb, malformed XML) crashes or hangs the compiler | Denial of Service | `zip`/`quick-xml` quarantined reader with `PartTooLarge`/`UnreadableZip`/`UnreadableXml` → fail-closed `oracle/*` Error finding; no panic (`#![deny(panic)]`); fuzz the raw reader |
| Formula injection (a non-whitelisted/dangerous function in a cell) | Tampering / Elevation | Whitelist-AT-PARSE-time rejection — an out-of-whitelist function is a parse-time REJECTION, never a silent accept (the dialect security primitive) |
| Fabricated Excel provenance (umya-stamped identity admitted as a trusted oracle) | Spoofing | Read ORIGINAL bytes via quarantined raw reader; anchored identity; REFUSE umya's `calcId=122211`/fabricated identity with `oracle/non-excel-app` (WBCO-07) |
| Prompt-injection via BA-authored cell strings (`meaning`/`unit`/enum) reaching the agent's LLM through the served schema | Tampering (info-flow) | Length-cap + sanitize cell metadata before it enters the manifest (document that BA strings reach the agent); the served consumer (Phase 92) already fail-closes validation |
| Breaking schema change auto-promoted with no human approval | Repudiation / Elevation | CR-01 symmetric classification + strictest-policy reducer so a demotion/assumption change can NEVER auto-`HotReload` past the `--accept` gate (the classifier completeness IS a security property) |
| Approval inherited across unrelated bundle contents | Repudiation | `candidate_fingerprint` binds prev+candidate content hashes (WR-04) — an approval can never cover content it did not review |
| Baseline destruction on promote (audit-trail loss) | Repudiation | CR-02 versioned non-overwriting writes; prior `@version` baseline survives byte-identical |

## Sources

### Primary (HIGH confidence)
- Lighthouse `crates/workbook-compiler/src/{lib,change_class/mod,gate/corpus,reconcile/classifier,provenance/gate,artifact/mod,manifest/ratify,commands/compile_workbook}.rs` — direct read confirming the §5 fixes are already implemented [HIGH]
- In-repo `crates/pmcp-workbook-runtime/src/{lib,manifest_model,artifact_model}.rs` — the re-export source + `annotations`-field delta [VERIFIED]
- In-repo `crates/pmcp-workbook-dialect/src/lib.rs` — `WHITELIST` (13 fns) + `DialectRules` the WBDL-03 linter enforces [VERIFIED]
- In-repo `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/{manifest.json,BUNDLE.lock}` + `tests/support/fixture_gen.rs` — the producer/consumer target (synthetically generated; no source `.xlsx`) [VERIFIED]
- In-repo `Makefile:460-548` (`purity-check`) + `.github/workflows/ci.yml` — the existing three-layer gate + extension procedure [VERIFIED]
- `.planning/phases/93-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md` (Phase 93 entry) — decisions + requirement verbatim [VERIFIED]
- `.planning/research/{SUMMARY,ARCHITECTURE,STACK,PITFALLS}.md` — milestone-level synthesis [HIGH]

### Secondary (MEDIUM confidence)
- Lighthouse `docs/sdk-issue-excel-workbook-compiler-extraction.md` §5 — the generalization-gap list (dated 2026-06-10; PRE-fix snapshot — the lighthouse has since fixed CR-01/CR-02/WR-01) [MEDIUM — stale relative to current code]
- crates.io live `cargo search` (2026-06-11): umya-spreadsheet 3.0.0, quick-xml 0.40.1, zip 9.0.0-pre2, rust_xlsxwriter 0.95.0 [HIGH for currency, ASSUMED for package provenance]

### Tertiary (LOW confidence)
- `umya-spreadsheet`/`quick-xml`/`zip` package legitimacy — slopcheck unavailable; tagged `[ASSUMED]`, gate first install [LOW]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified live; pins documented; SWC-drop flagged for parity verification (O-1)
- Architecture / lift map: HIGH — direct read of both the lighthouse source and the in-repo runtime/served crates
- §5 fixes already done: HIGH — read the code AND its test suites (not run, but present and matching the documented patches)
- Net-new work (delete `build_reference_manifest`, D-09 auto-corpus, WBCO-07 refuse-upgrade, byte-identical re-emit): MEDIUM-HIGH — clearly scoped, but O-2/O-3/O-4 need design decisions in planning
- Pitfalls: HIGH — grounded in source + the committed golden + the existing gate

**Research date:** 2026-06-11
**Valid until:** 2026-07-11 (30 days; stable in-repo crates + a frozen consumer contract. Re-derive the umya/quick-xml/zip pins at extraction regardless.)

## RESEARCH COMPLETE
