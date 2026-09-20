# Phase 93: Workbook Compiler + §5 Generalization Fixes + Promote Gate - Context

**Gathered:** 2026-06-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Port the offline `pmcp-workbook-compiler` crate — the full pipeline
**ingest → dialect-lint → manifest synth → formula parse → DAG compile →
penny-reconcile → artifact emit → promote-time gate** — with `umya` (the Excel
reader) confined to this crate and never entering the served tree. Ship the
RFC §5 generalization fixes **at extraction time, not copied**: fully
manifest-driven emit (delete `build_reference_manifest`), CR-01 symmetric
change-class classification, CR-02 versioned non-overwriting bundle writes,
WR-01 enum-tiering correctness, and umya fabricated-provenance refusal. Ship the
change-class + golden-corpus **promote gate** with a BA `--accept` approval
flow. Reuse the runtime's already-lifted shared types (`dag.rs`, `formula.rs`,
`finding.rs`, `manifest_model.rs`, `changelog.rs`).

**Requirements:** WBDL-03, WBCO-01 … WBCO-07, WBGV-01 … WBGV-07.

**Guiding lens (user, explicit):** the **business-analyst lifecycle** — a BA who
knows Excel but not Rust authors a sheet from one of our examples, builds it for
the first time (sees errors + warnings, fixes the errors and some warnings,
builds), deploys + tests, then weeks later updates the versioned sheet, rebuilds,
sees the main diffs/errors/warnings, fixes, promotes a new version, tests, and
iterates. Every decision below optimizes for *easy, non-technical, tolerant* —
keep the BA in the spreadsheet they understand; resolve common issues with
warnings, not hard blocks.

**Explicitly NOT in this phase:** the `cargo pmcp` CLI command surface
(`compile-workbook`/`lint-workbook`/`emit-bundle`) and `pmcp.toml` (Phase 94);
the Shape A `pmcp-workbook-server` binary + deploy (Phase 95); the
`cargo pmcp new --kind workbook-server` scaffold + dialect-version declaration +
second-workbook generalization gate (Phase 96). This phase produces the compiler
**library** and the **structured findings/diff/gate outputs** those later phases
render and invoke. The `--accept` *flag* itself is Phase 94; the gate behavior,
`ApprovalRecord` shape, and golden-corpus mechanics are decided and built here.
Does not touch `pmcp-code-mode`.
</domain>

<decisions>
## Implementation Decisions

### A. First-build findings model (lint + compile errors/warnings)
- **D-01 (block semantics):** **Errors block bundle emit; warnings are advisory
  and the BA can build past them.** (Directly from the user's narrative: "fix the
  errors and *some* of the warnings, build the server.") Findings are
  **collect-all, located (cell-addressed), and carry repair guidance** — the
  whole lint pass reports at once, never fail-fast (WBDL-03; uses the runtime's
  already-lifted `finding.rs`).
- **D-02 (error/warning split — "integrity-only blocks"):**
  - **ERRORS (block emit):** named-output penny-reconcile failure (D-03);
    formula parse failure (WBCO-03); non-whitelist function / dialect violation
    (WBDL-03); umya fabricated-provenance (WBCO-07, refused with
    `oracle/non-excel-app` — locked behavior).
  - **WARNINGS (advisory, BA builds past):** manifest-inference ambiguity (D-06);
    enum source not an inline list — falls back to a dynamic input (D-07);
    helper-cell reconcile drift (D-03); style/Guide advisories.
- **D-03 (reconcile tolerance — block only when a published answer is wrong):**
  A penny-reconcile mismatch (compiled IR vs Excel's stored cached value, under
  operand-anchored rounding — WBCO-04) is a **hard error only on a named
  output**; a mismatch on an **intermediate/helper cell is a located warning**
  ("cell `Calc!C7` computed 41.2 but your sheet shows 41.18 — likely a rounding
  difference"). Rationale: a helper divergence that actually matters propagates
  to a named output and is caught there; one that doesn't is harmless. This only
  ever forgives small numeric/rounding drift — broken *logic* (unsupported
  function) is already a dialect error, not a tolerated warning. "Cached value" =
  the last-computed result Excel stores alongside each formula on save; the
  compiler uses it as the trusted oracle and grades its own compiled computation
  against it. The BA never sees this machinery.

### B. Manifest synthesis + BA ratification (WBCO-02)
- **D-04 (stay-in-Excel):** The candidate semantic manifest
  (inputs/outputs/dtypes/units/meanings/tiers) is synthesized **fully
  workbook-driven** from workbook conventions (cell **colour**, a **Guide**
  sheet, column **headers**) — `build_reference_manifest` is deleted from every
  non-test path (kills per-workbook Rust). `manifest.json` is an **emitted
  artifact the BA NEVER hand-edits**. To correct an inference the BA **adjusts
  the sheet** (recolour a cell, edit the Guide) and recompiles. **Ratification =
  a recorded sign-off** (approver + date) — carry forward the runtime
  `manifest_model` `ratified` / `ratified_by` / `ratified_at` fields.
- **D-05 (unclassifiable cells → safe default + selective warning):** A cell the
  compiler can't confidently classify **stays in the computation but is NOT
  exposed** as an input or output (treated as an internal helper). A warning is
  emitted **only when the cell looks like it should be exposed** (e.g. a bare
  hardcoded number a consumer might want to vary), naming the cell and how to
  mark it in the sheet. Obvious internal helpers do not generate warnings (avoid
  noise on large sheets).
- **D-06 (enum sources — WBCO-06 + WR-01, partly locked):** Only **inline DV
  lists (`formula1` quoted literals, ≤10 values)** become closed JSON-Schema
  enums. A dropdown sourced from a **cell range or named range falls back to a
  dynamic input with a warning** (precise reason code) — never a block; this is
  the common-Excel-pattern tolerance applied consistently with D-02. **WR-01
  (locked):** enum inputs **skip Variable-tier assignment** so the default path
  can never seed an out-of-enum empty string — verified against the **committed**
  manifest, not the in-memory builder.

### C. Change-class diff presentation on re-compile (WBGV-01/02/03)
- **D-07 (organize by action bucket):** The re-compile diff is grouped by
  **change class as the BA action**: **"Safe — auto-applied"** (HotReload),
  **"Needs your approval"** (BlockUntilAccept), **"New version required"**
  (NeverAutoPromote). Each entry carries a **plain-language "what changed → what
  it means for consumers"** line (non-technical).
- **D-08 (material changes only):** Show **named-output value changes**,
  **manifest changes** (input/output added or removed; type/unit/enum changed),
  and **structural IR changes** (a formula redefined, a new dependency). **Pure
  helper-cell numeric noise is summarized in one line** ("+3 internal cells
  recalculated"), not enumerated. Matches the user's "give me the *main* diffs."
- **(locked §5 classifier behaviors, not gray — captured for downstream):**
  CR-01 symmetric coverage of **demotion-direction** changes (Input→Constant,
  source/assumption flips) — each produces a non-empty class, never silent
  HotReload (WBGV-01); a **strictest-policy reducer** hard-blocks any assumption
  (yellow-cell) change even amid hot-reloadable deltas (WBGV-02); a **stable
  canonical IR sub-DAG identity hash** distinguishes numeric drift from semantic
  redefinition (WBGV-03).

### D. Promote gate + golden-corpus + `--accept` (WBGV-04/05/06)
- **D-09 (corpus = auto-derived regression baseline):** The golden-corpus gate
  **replays an auto-generated set of input cases (from the manifest's defaults +
  enum domains) through BOTH the prior accepted version and the candidate** and
  flags any named output that moved beyond tolerance. The BA authors **no test
  cases** — the "golden corpus" is the previous version's own behavior captured
  automatically.
- **D-10 (block loop — show deltas + exact approve command):** On a block (a
  named output moved beyond tolerance with no covering approval), the gate
  **stops the promote and prints: which outputs changed and by how much, the
  change class, and the exact copy-pasteable approve command**
  (`--accept --approver <X> --effective-date <D>`). Running it records a
  **fingerprint-bound `ApprovalRecord`** (content-hash of the candidate),
  re-baselines the corpus, and lets the version through (WBGV-04/05). One
  deliberate, auditable action — not an inline interactive prompt.
- **D-11 (versioning — declared in the workbook, CR-02):** The BA **bumps the
  version in the workbook** (a changelog/Guide entry); the compiler writes the
  new bundle to its **own `@<version>` directory and never overwrites the
  baseline** (CR-02 / WBGV-06); `BUNDLE.lock` version == declared version ==
  `changelog.to_version`. Consistent with D-04's stay-in-Excel principle.

### Derived decisions (captured, not separately discussed)
- **D-12 (first build has no gate):** On the **first** version there is no prior
  accepted baseline, so the promote gate is a no-op (it establishes the
  baseline). First-version correctness is enforced by **penny-reconcile against
  Excel's own cached values** (D-03) — v1 is gated against the sheet itself, not
  ungated.
- **D-13 (`evidence/` member):** The bundle's `evidence/` member records the
  reconciliation result + provenance trail (changelog, parser-equivalence) —
  carry forward the seven-member contract frozen in Phase 92 (D-17: `bundle_id`
  naming; the synthetic tax-calc golden is the producer/consumer agreement
  target — see below).

### Claude's Discretion
- Exact compiler module layout (mirror the lighthouse `workbook-compiler/src`
  tree: `ingest/ manifest/ formula/ dag/ sheet_ir/ reconcile/ change_class/
  gate/ provenance/ artifact/`), reusing the runtime's shared types rather than
  re-declaring them.
- The auto-generated input-case grid size/shape for D-09 (bounded; defaults +
  enum-domain combinations), reconcile-finding message wording, and the
  `ApprovalRecord` on-disk format/location.
- The operand-anchored rounding implementation detail (the runtime already has
  `sheet_ir/rounding.rs`) — must NOT be a naïve `delta.abs()` tolerance
  (grep-gated per the roadmap success criterion).
- Colour/Guide/header inference heuristics (lift the lighthouse `manifest/`
  synth), provenance-reader internals (`quick-xml`/`zip` quarantined raw-bytes
  reader detecting umya's fabricated `<Application>`/`calcId`).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

> ⚠ The lighthouse lives at
> `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/`
> (absolute path; not repo-relative). It is **private reference material** — lift
> generic engine code only and scrub all customer identifiers/business logic
> (carry forward Phase 92 D-13's scrub rule; no TowelRads names in SDK code,
> comments, fixtures, or docs).

### Phase contract
- `.planning/ROADMAP.md` — Phase 93 entry (goal, 5 success criteria, WBDL-03 +
  WBCO + WBGV mapping); Phases 94/95/96 entries (the downstream scope boundary)
- `.planning/REQUIREMENTS.md` — WBDL-03, WBCO-01…07, WBGV-01…07 verbatim
- `.planning/phases/92-bundlesource-served-tool-toolkit-module/92-CONTEXT.md` —
  the frozen bundle contract this compiler must emit (D-17 `bundle_id` naming,
  D-18 `AnnotationDecl`, seven-member layout, ProvStamp = bundle_id+version+
  combined hash); the synthetic **tax-calc@1.1.0 golden** is the
  producer/consumer agreement target (the compiler must re-emit the same workbook
  and diff byte-identical against this committed golden — Phase 92 D-03)
- `.planning/phases/91-workbook-runtime-purity-gate-dialect-spec/91-CONTEXT.md` —
  Phase 91 carry-forward: crate split, the three-layer purity gate (D-09,
  per-feature-combination), the dialect contract crate, WBDL-03 linter
  *execution* re-mapped to THIS phase (D-02), the `finding.rs` model (D-08)

### v2.3 research (in-repo)
- `.planning/research/SUMMARY.md` — phase-cut synthesis; build order; the three
  named promote-path bugs (CR-01/CR-02/WR-01) to fix not copy
- `.planning/research/ARCHITECTURE.md` — two non-overlapping dependency cones;
  offline cone = `cargo-pmcp → pmcp-workbook-compiler → pmcp-workbook-runtime`
  owns umya/quick-xml/zip
- `.planning/research/STACK.md` — `umya-spreadsheet 3.0` (compiler-only reader),
  `quick-xml 0.37`/`zip 8` transitive-pinned via `cargo tree -p umya-spreadsheet
  -i quick-xml`; hand-rolled formula/DAG lifted verbatim (no petgraph, no formula
  crate — the whitelist-at-parse-time is the security primitive)
- `.planning/research/PITFALLS.md` — purity-boundary erosion (gate on day one);
  naïve abs-delta reconcile tolerance trap

### Lighthouse compiler (lift source — scrub per Phase 92 D-13)
- `.../quote-pricing/crates/workbook-compiler/src/` — module tree to port:
  `ingest/ manifest/ formula/ dag/ sheet_ir/ reconcile/ change_class/ gate/
  provenance/ artifact/ dialect/ commands/`, `excel_error.rs`, `stage1.rs`
- `.../quote-pricing/docs/sdk-issue-excel-workbook-compiler-extraction.md` §5 —
  the generalization gaps (manifest-driven emit, CR-01/CR-02/WR-01, umya
  fabricated-provenance) — **the §5 fixes are mandated success criteria**
- `.../quote-pricing/docs/Excel-as-Configuration-Architecture-Brief.md` —
  two-surface model

### SDK code to reuse / mirror (in-repo)
- `crates/pmcp-workbook-runtime/src/` — shared types the compiler re-exports and
  emits: `dag.rs`, `formula.rs`, `finding.rs`, `manifest_model.rs`
  (`ratified`/`ratified_by`/`AnnotationDecl`), `changelog.rs`, `artifact_model.rs`
  (`BundleLock`, `build_bundle_lock`, `fold_evidence_hash`), `sheet_ir/`
  (`rounding.rs`, `semantics.rs`, executor)
- `crates/pmcp-workbook-dialect/src/lib.rs` — the WHITELIST the linter enforces
  (WBDL-03 execution lands here against this contract)
- `crates/pmcp-server-toolkit/src/workbook/` — the consumer side (Phase 92):
  what the emitted bundle must satisfy at boot (loader, input validation,
  projection) — the compiler's output contract
- `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` — the committed
  golden the compiler must reproduce
- `Makefile`/`justfile` `purity-check` + `.github/workflows` purity job — extend
  with the compiler cone (umya present in compiler, absent everywhere served)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-workbook-runtime`** (Phases 91–92): the compiler's entire *output*
  type system already exists in-repo — `manifest_model`, `artifact_model`
  (BundleLock + hashing), `changelog`, `dag`, `formula`, `finding`, `sheet_ir`
  (incl. `rounding.rs` for operand-anchored reconcile). The compiler re-exports
  and emits these so the served binary deserializes without the reader.
- **`pmcp-workbook-dialect`** (Phase 91): the WHITELIST + spec the WBDL-03 linter
  enforces — Phase 91 shipped the *contract*; this phase adds the running linter.
- **Phase 92 served layer + tax-calc golden**: the exact bundle shape + the
  producer/consumer agreement target the compiler must hit byte-identically.

### Established Patterns
- **Purity gate** (Phase 91 D-09): three-layer (`cargo tree` negative assertions
  per served-tree crate + `cargo-deny [bans]` + structural crate split), run
  per-feature-combination, merge-blocking. The compiler is the one crate where
  `umya`/`quick-xml`/`zip` are *allowed*; extend the gate to assert they're
  present here and absent everywhere served.
- **Crate-level `#![deny(clippy::unwrap_used, expect_used, panic)]`** on value
  paths (lighthouse convention, kept in Phases 91–92).
- **Collect-all located findings** with repair guidance (`finding.rs`) — the
  lint/compile UX (D-01) builds on this.

### Integration Points
- New crate `pmcp-workbook-compiler` → depends on `pmcp-workbook-runtime`
  (re-export source) + `pmcp-workbook-dialect` + umya/quick-xml/zip. Publish
  order (CLAUDE.md): runtime → dialect → … → compiler is the offline cone;
  the compiler is consumed by `cargo-pmcp` in Phase 94, never by the served tree.
- Producer/consumer proof: re-emit the Phase 92 tax-calc workbook through this
  compiler and diff byte-identical against the committed golden.
</code_context>

<specifics>
## Specific Ideas

- The **BA lifecycle narrative** (user, verbatim intent): author from an example
  → first build (errors + warnings) → fix errors + some warnings → build →
  deploy → test → weeks later update the versioned sheet → rebuild → see main
  diffs/errors/warnings → fix → promote new version → test → iterate. This is the
  acceptance lens for every Phase 93 output surface.
- **Tolerance principle (user, explicit):** "make it easy, not overly technical,
  for the BA to convert the Excel they know into an MCP server they don't
  understand; be tolerant of common issues we can resolve with a warning; don't
  put up technical hurdles." Applied in D-02/D-03/D-05/D-06.
- **Stay-in-Excel principle:** the BA expresses and corrects intent in the
  spreadsheet (colour/Guide/headers/changelog), never by hand-editing emitted
  JSON (D-04, D-11).

</specifics>

<deferred>
## Deferred Ideas

- **`cargo pmcp compile-workbook`/`lint-workbook`/`emit-bundle` CLI + `pmcp.toml`**
  — Phase 94. The compiler exposes library verbs + structured outputs; the CLI
  is a thin shell carrying the `--accept` flow. (Decisions D-09/D-10/D-11 fix the
  behavior the CLI surfaces.)
- **Shape A `pmcp-workbook-server` binary + deploy/test** — Phase 95.
- **`cargo pmcp new --kind workbook-server` scaffold, dialect-version
  declaration, second-workbook + Excel-quirk generalization gate** — Phase 96
  (the second-workbook test is the true generalization proof).
- **BA-curated input→expected test cases** as a gate basis — rejected for Phase
  93 (D-09 uses an auto-derived regression baseline to avoid asking a
  non-technical BA to author cases); revisit if auto-derivation proves
  insufficient.
- **Auto-bump versioning by change class** — rejected for Phase 93 (D-11 keeps
  versioning workbook-declared per stay-in-Excel); revisit if BAs find manual
  changelog bumps burdensome.
- **Interactive inline gate approval prompt** — rejected (D-10 uses an explicit,
  auditable, copy-pasteable `--accept` command instead).

</deferred>

---

*Phase: 93-workbook-compiler-5-generalization-fixes-promote-gate*
*Context gathered: 2026-06-11*
