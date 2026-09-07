# Workbook Dialect Spec (v0.5.0, Phase 91)

> **The published, constrained workbook dialect contract (DIA-01).** This is the
> BA/auditor-facing "moat" document: a governed Excel workbook conforms to *this*
> dialect or it does not compile. It is published as human-readable prose **and**
> bound by an automated test to the machine `WHITELIST` const in
> `crates/pmcp-workbook-dialect/src/lib.rs` so the published contract and the
> enforced rule can never drift.
>
> Derives from the lighthouse project's Excel-as-Configuration architecture
> brief — §5 (the dialect: layered sheets, function set, acyclic DAG) and §7
> (cell taxonomy). The brief is an **external lighthouse document, not vendored
> in this repository**; the §5/§7 pointers below refer to it for design
> rationale, but this spec is self-contained for SDK readers.

## 1. What this dialect is

A constrained subset of Excel/Open XML that a governed quoting workbook must
stay within so it can be **compiled** — offline, deny-by-default — into a
deterministic, evidence-bearing MCP tool surface. The dialect is a *moat*: a
workbook that conforms is safe to compile; a workbook outside it is refused with
a precise, located, BA-actionable lint finding (sheet + cell + rule + repair),
never silently accepted and never auto-"fixed".

The compiler reads the workbook into an **owned, `umya`-free model** (the
`WorkbookMap`) and lints over that owned model. Colour is read only as a
**lintable UX projection** — never as canonical truth (architecture brief §7).

## 2. Layered sheets (architecture brief §5)

A conforming workbook is organised into **numbered layers** by sheet-name prefix
so the compiler can reason about sheet roles structurally rather than by guessing:

| Prefix | Layer | Purpose |
|--------|-------|---------|
| `0_` | Guide | The legend: the colour ontology + human documentation |
| `1_` | Inputs | Per-quote, BA-overridable inputs |
| `2_` | Constants / governed | BA-set governed constants (prices, margins, rules) |
| `3_` | Calculation / output | Derived formulas and the quote output |

The numbered-layer convention is the ordered set the linter checks sheet names
against (`DialectRules::sheet_layer_prefixes()`).

## 3. Whitelisted function set (DIA-05)

A conforming formula may only call functions in the **whitelist**. The whitelist
is **deny-by-default**: any function token not in the set is a located
`whitelist/unsupported-fn` lint **error** (never silently accepted, never
auto-widened — see §6).

The set is a flat list of **17 first-class functions** that the lighthouse
workbook (`UFH_Quote_Process_Model_Plot3.xlsx` — the lighthouse project's
reference workbook, not vendored in this repository) already authors, so it
lints clean **as-authored**. Every function below is first-class — there is no
core/widened tiering.

| Function | Category | Notes |
|----------|----------|-------|
| `IF` | whitelist | conditional |
| `VLOOKUP` | whitelist | table lookup |
| `INDEX` | whitelist | positional lookup |
| `MATCH` | whitelist | positional search |
| `SUMIF` | whitelist | conditional sum |
| `SUM` | whitelist | aggregate |
| `ROUNDUP` | whitelist | round toward +inf |
| `CEILING` | whitelist | round up to multiple |
| `IFERROR` | whitelist | error-guard the lighthouse authors |
| `ISNUMBER` | whitelist | type test the lighthouse authors |
| `SEARCH` | whitelist | substring search the lighthouse authors |
| `ROUND` | whitelist | half-up rounding the lighthouse authors |
| `TEXT` | whitelist | number formatting the lighthouse authors |
| `ROUNDDOWN` | whitelist | round toward zero (dialect 1.1) |
| `MAX` | whitelist | aggregate maximum (dialect 1.1) |
| `MIN` | whitelist | aggregate minimum (dialect 1.1) |
| `XLOOKUP` | whitelist | exact lookup, narrow 3/4-arg form only — see §3.2 (dialect 1.1) |

Total: **17 names**. This table is the *published* contract; the `WHITELIST`
const is the *enforced* contract. An automated binding test
(`pmcp_workbook_dialect::dialect_spec::doc_whitelist_table_matches_const`) parses
the function names out of this very table and asserts set-equality with
`WHITELIST`, so if either drifts the build fails.

The arithmetic **operators** `+ - * / ^` are part of the dialect but are checked
separately — they are not function tokens and do not appear in the whitelist.

### 3.1 Exact-match lookup contract (dialect 1.1)

`VLOOKUP`, `MATCH` and `XLOOKUP` perform **EXACT matching only**. The dialect
does not support approximate (sorted-range) lookup in any form, and it will not
guess one for you:

| You wrote | Result | Repair |
|-----------|--------|--------|
| `VLOOKUP(key, table, col, FALSE)` | accepted | — |
| `VLOOKUP(key, table, col, 0)` | accepted | — |
| `VLOOKUP(key, table, col)` | **refused** | write `VLOOKUP(key, table, col, FALSE)` |
| `VLOOKUP(key, table, col, TRUE)` / `…, 1)` | **refused** | write `VLOOKUP(key, table, col, FALSE)` |
| `MATCH(key, range, 0)` | accepted | — |
| `MATCH(key, range)` | **refused** | write `MATCH(key, range, 0)` |
| `MATCH(key, range, 1)` / `…, -1)` | **refused** | write `MATCH(key, range, 0)` |

The fourth `VLOOKUP` argument and the third `MATCH` argument are **REQUIRED**,
even though Excel itself lets you omit them.

**Why the omitted form is refused rather than assumed exact.** Excel's DEFAULT
for both is **APPROXIMATE**. An approximate lookup over an unsorted governed
table does not fail — it returns *a wrong number, with no signal*, silently
propagating through every downstream formula. A refusal costs the author one
edit; a silently wrong answer costs whatever the number was used for. This
dialect refuses rather than guesses.

The refusal is raised at parse time as a **located** error naming the cell
(`parse 1_Inputs!B7: call to \`MATCH\` has an unsupported shape: …`), and the
evaluator carries an independent `#VALUE!` backstop for the same conditions, so
neither layer can be bypassed.

### 3.2 `XLOOKUP` — the constrained form (dialect 1.1)

The one accepted signature is:

```text
XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found])
```

- **Exact match only.** The optional Excel arguments `match_mode` and
  `search_mode` are **NOT supported** — a 5th or 6th argument is refused at parse
  time. The evaluator never guesses a mode.
- **`return_array` must be single-column.** A multi-column `return_array` is
  refused.
- **`lookup_array` and `return_array` must be conformable** — equal member
  counts, so the positional pairing is meaningful.
- **`if_not_found` is optional.** On a miss it is returned; when it is omitted a
  miss yields Excel's `#N/A`.

**No correct spill behaviour is being withheld.** A spilling `XLOOKUP` is
already refused independently by the §5 `formula/array` rule, and the compiled IR
carries a scalar value per cell, so a spilling form is unrepresentable rather
than merely unimplemented.

## 4. Typed inputs and the two-layer metadata model

- **Typed inputs.** Inputs are typed (number / text / bool) so the compiled tool
  surface can carry a `outputSchema` with units and meanings (architecture brief §7).
- **Two-layer metadata (DIA-03 / DIA-04).** One logical manifest with two
  physical projections that must agree on overlap: **named ranges (+ structured
  tables)** are the BA-facing canonical surface, and a hidden **`_Manifest`
  sheet** is the technical enrichment layer (units, meanings, loop metadata, role
  classifications). Colour is linted *against* the manifest; it is never the
  source of truth. (Synthesis + round-trip of this model land with the Phase 93
  compiler.)

## 5. The refuse-set (DIA-02, D-07 / D-08)

A conforming workbook contains **none** of the following. Each is a precise,
located lint **error** carrying a BA-actionable repair, collected all at once
(the linter never stops at the first finding):

| Rule id | What is refused | Why | Owned `WorkbookMap` field read |
|---------|-----------------|-----|--------------------------------|
| `structure/macro` | a macro-bearing workbook (`.xlsm` / VBA) | the compiler never executes anything; macros are unverifiable code | `wb.has_macros` |
| `structure/external-link` | a formula referencing another workbook (`[1]Sheet1!…`, `[Book.xlsx]…`) | pulls unseen values from outside the governed file | `wb.external_links` (+ raised during ingest) |
| `structure/hidden-sheet` | a `Hidden` **or** `VeryHidden` sheet | conceals real role/value from the BA and the compiler | `SheetRecord.state` |
| `structure/hidden-row` | a hidden row | conceals located content | `SheetRecord.hidden_rows` |
| `formula/array` | a legacy CSE array formula or a dynamic-array spill | non-scalar semantics the v1 compiler does not model | `CellRecord.formula_kind` (`Array` / `DynamicArray`) |
| `whitelist/unsupported-fn` | a function token outside the §3 whitelist | smuggles unsupported semantics | `CellRecord.formula` (token scan vs `WHITELIST`) |
| `manifest/range-out-of-bounds` | a defined name targeting a sheet outside the layered set | a named range pointing somewhere unexpected | `wb.defined_names` |
| `role/conditional-formatting` | conditional formatting covering a role cell (D-08) | CF colour is *evaluated*, not stored; the compiler refuses the role rather than evaluate CF | `SheetRecord.cf_ranges` |
| `role/merged-cell` | a merge covering a role cell | a role cell must be a single addressable cell | `SheetRecord.merges` |

**Colour ontology (lint-only).** The dialect reads blue font `FF0000FF` → input,
green fill `FFE2EFDA` → constant, yellow fill → assumption, default-font formula
cell → formula. These are *evidence labels* used to propose roles and to lint
colour against the manifest — never canonical (architecture brief §7).

## 6. Enforcement status in this repository (phases 91 / 93)

To avoid overclaiming, every dialect rule is marked as either **ENFORCED** (the
linter actually checks it over the owned `WorkbookMap`) or **DECLARED but
deferred** (part of the published dialect, but not checked until a later phase
builds the machinery). Phase numbers below are **this repository's** roadmap
phases (`.planning/ROADMAP.md`), not the lighthouse project's.

### Enforced today (Phase 91 — this phase)

- **Nothing in §5 is mechanically enforced yet.** Phase 91 publishes the
  contract: this document, the machine `WHITELIST` const, and the doc↔const
  binding test (so the published and enforced whitelist cannot drift before the
  linter even exists).
- The served **runtime** already fails a cyclic dependency DAG at run time (a
  located `dag/cycle` finding from `toposort`) — but that is a runtime guard
  over an already-compiled IR, not a compile-time workbook check.

### Enforced in Phase 93 (the compiler + linter phase)

- The full **refuse-set** of §5 (structural + role + formula-kind), over the
  owned `WorkbookMap` — the linter never reaches back into `umya`.
- The **whitelist token scan** (§3) — deny-by-default, located, no auto-widening.
- The **two-layer overlap** consistency check and **colour-vs-manifest** lint
  (DIA-03 / DIA-04) — land with manifest synthesis, also Phase 93.
- **Compile-time acyclic dependency DAG.** The architecture brief §5 requires
  the formula graph to be an acyclic DAG; the Phase 93 compiler's formula parser
  + DAG reconstruction checks it at compile time.

### Known linter limitation — whitelist string-literal false positive

The whitelist scan is a **token approximation**, not a lexer. A function name
that appears *inside a quoted string literal* — e.g. `TEXT(x,"0")` containing a
literal `"SUM("` inside the quotes, or a cell whose text content is `"OFFSET("` —
can be mistaken for a function call. The Phase 93 scanner mitigates the common
case by skipping over quoted-string regions, but a fully robust resolution (a
real lexer that tracks string vs formula context) is deferred beyond Phase 93.
This is an accepted approximation, documented here so it is not mistaken for a
defect.

The scanner also strips a leading `_xlfn.` future-function prefix before
comparison, so `_xlfn.CONCAT(` is compared as `CONCAT`.

## 7. Dialect version declaration & compatibility policy (WBDL-02)

A workbook MAY self-declare the dialect version it targets so the dialect can
evolve forward-compatibly without abandoning the milestone's fail-closed ethos.
The declaration travels *with* the workbook (it is the specification), so it
lives **inside the `.xlsx`**, not in `pmcp.toml` or a CLI flag — there is exactly
one source of truth for the dialect version, and a flag can never spoof it.

### 7.1 The `pmcp_dialect_version` named range (D-03)

The version is declared in a reserved **single-cell defined name** named
`pmcp_dialect_version` (case-insensitive), targeting one cell whose cached value
is the version string. This mirrors the `version` / `out_*` named-range
conventions already in the dialect. A multi-cell range named
`pmcp_dialect_version` is not a scalar version and is ignored.

### 7.2 Version grammar

The accepted version-string format is `MAJOR.MINOR` with an OPTIONAL `.PATCH`
suffix:

- `MAJOR.MINOR` is REQUIRED; `.PATCH` is tolerated but optional.
- Each component is **base-10 digits only**; each parses into a `u64`. A component
  that overflows `u64` is MALFORMED (a typed compile error, never a panic).
- Surrounding whitespace is trimmed before parse; embedded whitespace
  (e.g. `1 .0`) is MALFORMED.
- Leading zeros are accepted and parsed numerically (`01.0` == `1.0`); a single
  `0` component is legal.
- `PATCH` is **ignored for the compatibility decision** — compatibility is decided
  on `MAJOR.MINOR` only, so `1.0.999` is accepted when the supported version is
  `1.0` (a declared patch can never make a compatible `MAJOR.MINOR` incompatible).

### 7.3 Compatibility rule (D-04, fail-closed)

A declared version is **accepted** when it has the **same major** as the
compiler's supported version AND its **minor is less than or equal to** the
supported minor. Otherwise — a different major, OR a newer-than-supported minor —
the compile **fails closed** with a typed `CompileError`, never a silent accept.

### 7.4 Absent declaration → baseline (D-05)

A workbook with **no** `pmcp_dialect_version` cell is treated as targeting the
**baseline** dialect version and compiles normally (the compiler MAY emit a
non-fatal advisory recommending the author add an explicit cell). Every existing
fixture has no version cell and keeps working with zero edits.

### 7.5 Version values (bound to the consts)

These two values are the *published* version contract; the
`SUPPORTED_DIALECT_VERSION` / `BASELINE_DIALECT_VERSION` consts in
`crates/pmcp-workbook-dialect/src/lib.rs` are the *enforced* contract. An
automated binding test
(`pmcp_workbook_dialect::dialect_version_spec::doc_versions_match_consts`) parses
the values out of this very table and asserts string-equality with the consts, so
if either drifts the build fails.

| Version field | Value | Meaning |
|---------------|-------|---------|
| `supported` | `1.1` | the maximum `MAJOR.MINOR` the compiler accepts |
| `baseline` | `1.0` | the dialect an absent declaration targets (D-05) |

### 7.6 Recorded narrowing (dialect 1.1)

Dialect 1.1 both **WIDENS** and **NARROWS**, and the narrowing rode a MINOR
bump deliberately. The reasoning is recorded here, in the published contract,
rather than in a commit message.

- **The widening:** four new first-class functions — `ROUNDDOWN`, `MAX`, `MIN`,
  `XLOOKUP` (§3).
- **The narrowing:** the EXACT-match contract of §3.1 is now ENFORCED. A
  `VLOOKUP` without its `range_lookup` argument, and a `MATCH` without its
  `match_type` argument, are refused where they previously compiled.

**A MAJOR bump was considered and rejected**, for three reasons:

1. **Nothing correct is being removed.** EXACT-ONLY was ALWAYS the published
   contract — the approximate form was never supported, it was silently
   mis-evaluated as exact. Dialect 1.1 makes an already-published refusal real.
   That is a bug fix, not a capability withdrawal.
2. **The measured blast radius is zero.** Every `VLOOKUP`/`MATCH` call authored
   anywhere in this repository — fixtures, authored fixture source and unit
   tests alike — already writes the exact-match argument. No workbook and no
   fixture needed an edit.
3. **A major bump would fail closed on EVERY existing workbook.** Under §7.3 a
   `2.0` supported version rejects every workbook declaring `1.0` and (via
   §7.4's baseline) every workbook declaring nothing at all. Refusing the entire
   existing corpus is a catastrophic response to fixing a wrong answer.

Under §7.3, `supported = 1.1` keeps every `1.0`-declaring and every undeclared
workbook accepted.

---

*Bound to `crates/pmcp-workbook-dialect/src/lib.rs` `WHITELIST` by
`pmcp_workbook_dialect::dialect_spec::doc_whitelist_table_matches_const`.
Derives from the lighthouse project's Excel-as-Configuration architecture brief
§5 / §7 (external; not vendored in this repository).*
