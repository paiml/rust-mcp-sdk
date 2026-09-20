# Phase 91: Workbook Runtime + Purity Gate + Dialect Spec - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 91-workbook-runtime-purity-gate-dialect-spec
**Areas discussed:** Dialect+linter boundary, Dialect v1 whitelist policy, Linter finding format, Purity gate recipe & scope

---

## Dialect + Linter Boundary

### Where the umya-free dialect layer lives

| Option | Description | Selected |
|--------|-------------|----------|
| Inside pmcp-workbook-runtime | Fold WorkbookMap + rules + linter into the single runtime leaf | |
| Separate pmcp-workbook-dialect crate | Split the dialect concern into its own reader-free leaf; runtime keeps IR/eval/render | ✓ |
| You decide | Recommend based on SDK leaf-crate structure | |

**User's choice:** Separate `pmcp-workbook-dialect` crate.

### How WBDL-03 ("lint a workbook") is demonstrated reader-free in Phase 91

| Option | Description | Selected |
|--------|-------------|----------|
| Fixtures + public lint API | Ship `lint(&WorkbookMap)->LintReport`, prove over hand-built fixtures; real-.xlsx lint in Ph93/94 | |
| Defer lint entirely to Ph93 | Ph91 ships only WHITELIST + spec doc + binding test; linter + WorkbookMap + WBDL-03 land in Ph93 | ✓ |

**User's choice:** Defer lint entirely to Ph93.

### Where WorkbookMap lives (reconciliation follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| In the compiler (Ph93) | WorkbookMap + linter.rs stay in pmcp-workbook-compiler; dialect crate = pure contract | ✓ |
| In the dialect crate (Ph91) | WorkbookMap moves into pmcp-workbook-dialect now | |
| You decide | Recommend for simplest purity boundary | |

**User's choice:** In the compiler (Ph93).

**Notes:** The two top-level picks (separate dialect crate + defer lint to Ph93)
were reconciled into a **contract-crate design**: Phase 91's
`pmcp-workbook-dialect` = WHITELIST const + DialectRules + finding model + spec
doc + binding test (WBDL-01); the linter *execution* and `WorkbookMap` move to
the Phase 93 compiler, so **WBDL-03 migrates from Phase 91 → Phase 93** (requires
a ROADMAP/REQUIREMENTS update). Confirmed mechanically clean: WorkbookMap
(246 LOC), linter.rs (596 LOC), rules.rs (197 LOC) are all umya-free in the
lighthouse.

---

## Dialect v1 Whitelist Policy

| Option | Description | Selected |
|--------|-------------|----------|
| Ship 13 as flat v1 set | Adopt all 13 verbatim; drop the core/widened two-tier framing | ✓ |
| Keep 8-core/5-widened tiering | Preserve the lighthouse split + D-09 provenance in the spec doc | |
| Curate a different set | Re-decide the canonical set (note: removing any breaks the reference workbook) | |

**User's choice:** Ship 13 as a flat first-class v1 set.

**Notes:** Operators `+ - * / ^` stay checked separately (not whitelist tokens).
Future evolution via dialect versioning (WBDL-02, Ph96). Constraint surfaced: the
lighthouse reference workbook authors all 13 and is the Ph93/96 golden corpus —
curating down would break its clean lint.

---

## Linter Finding Format

| Option | Description | Selected |
|--------|-------------|----------|
| Lift + add Deserialize | Take finding.rs as-is + add Deserialize for a round-trippable public artifact; keep rule:String | ✓ |
| Lift verbatim | Port finding.rs exactly, Serialize-only | |
| Reshape to typed rule ids | Replace rule:String with a RuleId enum + rule-id↔doc binding test | |

**User's choice:** Lift + add Deserialize.

**Notes:** Finding model stays in the runtime (run() returns LintFinding on a
cycle); the dialect crate re-exports it. Retains severity (Error gates only),
slash-namespaced rule id, sheet+cell location, message, repair, JsonSchema,
collect-all LintReport, has_errors gate.

---

## Purity Gate Recipe & Scope

| Option | Description | Selected |
|--------|-------------|----------|
| cargo-tree + cargo-deny, all features | Lift cargo-tree recipe + add cargo-deny [bans] backstop, per feature-combination | ✓ |
| cargo-tree only, all features | cargo-tree per-crate assertions only, per feature-combination; skip cargo-deny | |
| cargo-tree + cargo-deny, default features | Both layers but default features only (risks feature-unification leak) | |

**User's choice:** cargo-tree + cargo-deny, all (feature-combinations).

**Notes:** Three-layer gate (cargo-tree per-crate + cargo-deny [bans] +
structural crate split), per feature-combination, in `just purity-check` + CI.
Negative bans umya/quick-xml/swc_/pmcp-code-mode in runtime + dialect trees;
positive asserts rust_xlsxwriter present, zip permitted. Panic-freedom stays with
the crate-level `#![deny(...)]`, not the gate.

## Claude's Discretion

- Exact `cargo tree` invocation, `cargo-deny [bans]` stanza, CI feature-matrix shape.
- `zip` pin for `rust_xlsxwriter`; doc↔const binding-test mechanism (mirror lighthouse).
- Publish/dep order (slots 2a runtime, 2b dialect) and finding-model placement (runtime) — recorded as derived decisions D-03/D-04.

## Deferred Ideas

- WBDL-03 linter execution + WorkbookMap → Phase 93 (deliberate re-map).
- WBDL-02 (workbook declares dialect version) → Phase 96.
- quick-xml/zip compiler-side transitive-pin re-derivation → Phase 93.
- Typed RuleId enum + rule-id↔doc binding → revisit if a stable rule taxonomy is needed.
- JS-oracle (pmcp-code-mode/SWC) reconcile parity → Phase 93 open question.
