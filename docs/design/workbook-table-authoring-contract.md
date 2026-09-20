# Workbook → MCP: the Table-Based Authoring Contract

**Status:** Design contract — converged, ready to seat as a phase
**Date:** 2026-06-20
**Supersedes:** the per-cell `in_*`/`out_*` named-range model (incl. the just-merged F1 + F3-for-inputs from debug session `workbook-tool-surface-gaps`). **Keeps:** F2 (override discoverability).
**Origin:** pmcp.run dev-team feedback (`SDK-ISSUE-workbook-tool-surface.md`) → end-to-end redesign of the BA authoring experience and the emitted MCP tool surface.
**Pre-1.0 freedom:** no production customers on the legacy interface → we break it cleanly rather than migrate it.

---

## 1. North star

**The Excel workbook IS the MCP tool contract.** A business analyst already has the
process in a spreadsheet; authoring should be *visible, standard Excel* with no hidden
machinery, and the compiler should derive a well-named, well-described, well-typed MCP
tool surface that an LLM client can select and call correctly on the first try.

Two ends of one pipeline:
- **BA authoring** — name two kinds of region (inputs, outputs) as **Excel Tables** with
  standard columns; fill realistic example values; pick governance from a dropdown.
- **LLM consumption** — each output table becomes a **named, described MCP tool** with a
  precise input schema (only the fields it uses) and an output schema.

---

## 2. Verified capability (umya 3.0.0 reads Excel Tables)

Confirmed against crate source (`umya-spreadsheet 3.0.0`, the compiler's existing reader):

- Load path is live: `reader/xlsx.rs:225` walks each worksheet's relationships to
  `xl/tables/tableN.xml` and populates tables (not writer-only).
- API: `Worksheet::get_tables() -> &[Table]`; `Table::get_name()` (ListObject name),
  `Table::get_area() -> (Coordinate, Coordinate)` (range), `Table::get_columns() -> &[TableColumn]`
  with `TableColumn::get_name()` (header names).
- **No new dependency** → the reader-free **purity boundary is untouched** (umya stays
  confined to the compiler; we do NOT add calamine/another reader).

Caveats for the spec (neither a blocker):
1. umya's table reader `.unwrap()`s on malformed table XML — keep the compiler's existing
   umya-isolation boundary so a bad workbook is a clean compile error, not a panic.
2. If the compiler ever re-emits tables, verify round-trip fidelity; today it predominantly
   reads, so risk is low.

---

## 3. The authoring model

### 3.1 Region types (what a BA learns)

| Region | Excel form | Purpose | Exposed? |
|--------|-----------|---------|----------|
| **Input table** | named Excel Table, columns `name \| value \| description [\| tier]` | caller-supplied fields | yes (as tool inputs) |
| **Output table(s)** | named Excel Table, columns `name \| value \| description` | a tool's results; **table name = tool name** | yes (one MCP tool per table) |
| **Reference / lookup** | any cells/ranges (rate cards, VLOOKUP tables) | DAG interior data | no |
| **Intermediate calc** | any sheets | business logic (DAG interior) | no |
| **`0_meta`** (optional) | workbook/server-level metadata only | server hints, version, governance defaults | n/a |

Only the **declaration tables** (inputs, outputs) are structured. Calc and lookup sheets stay
free-form. Intermediate sheets are fully supported — the compiler walks the cross-sheet
formula DAG from input-table value cells to output-table value cells.

### 3.2 Standard columns

**Input table** — `name | value | description | tier`
- `name` — the semantic key (the served `json_key`). Single source of truth; **no named range**.
- `value` — does quadruple duty: BA working example · **type witness** · **unit source** (number
  format) · **enum source** (data-validation dropdown) · the single **end-to-end test input** ·
  the **reconciliation seed**.
- `description` — per-field description, co-located (no `0_meta` duplication → no drift).
- `tier` — governance, via a data-validation dropdown `{variable, strict}`. `strict` =
  BA-governed constant, **not** caller-exposed (rejected as an input, allowed as an override
  only per existing rules). Default `variable`.

**Output table** — `name | value | description`
- `name` — served output key.
- `value` — the **authored expected result** = the reconciliation oracle the gate already checks.
- `description` — per-output description (feeds the tool's `outputSchema`).
- The **table's name** is the **tool name**; a **caption cell directly above the table** holds the
  **tool description** (co-located → no drift; `0_meta` does not list tools).

### 3.3 Harvest rules (compile-time, per row)

| Schema field | Source |
|--------------|--------|
| field key | `name` column cell |
| type (`number`/`string`/`boolean`) | `value` cell type |
| unit | `value` cell **number format** (currency → USD, `%` → rate, date → date) |
| enum domain | **data-validation list** on the `value` cell |
| description | `description` column cell |
| tier / strict | `tier` column cell (input tables) |
| example / test input | `value` cell |
| output expected (oracle) | output `value` cell |

The `tier` dropdown **dogfoods** the enum-from-dropdown mechanism — the template teaches the
pattern by using it.

---

## 4. Multi-tool model (output tables → MCP tools)

**Each named output table = one MCP tool.** A single named output value is the N=1 case (no
special path). Multiple tables in one workbook = multiple tools (intermediate steps / different
business paths), which is strictly better for LLM tool-selection than one generic `calculate`
with a `mode` enum.

### 4.1 Manifest model extension (the core engineering lift)

From `one input-set → one output-set` to:

```
Workbook → {
  inputs:  [ InputField{ key, type, unit?, enum?, description, tier } ],   // the shared pool
  tools:   [ Tool{
              name,                 // = output table name (→ MCP tool name)
              description,          // = caption above the output table
              input_keys: [..],     // DERIVED from the DAG (default) — see 4.2
              outputs: [ OutputField{ key, type, unit?, description } ],
              oracle:  { <output key>: <expected value> }   // gate reconciliation, per tool
            } ]
}
```

Each tool emits an MCP `inputSchema` (its derived inputs, fully typed) **and** an `outputSchema`
(its outputs) → `structuredContent`, matching the SDK's `TypedToolWithOutput` pattern.

### 4.2 Per-tool inputs — **LOCKED: DAG-derived by default, declared override available**

- **Default (recommended):** a tool advertises only the inputs that are **upstream of its output
  table's cells** in the formula DAG. The compiler already has the graph; reachability gives each
  tool a precise, minimal schema. (e.g. `calculate_tax` shows `income`+`filing`; `estimate_refund`
  adds `withheld`.) Better LLM ergonomics; smaller schemas.
- **Override:** a tool may explicitly declare its input set (a future `inputs:` caption/column on
  the output table) when the BA wants to widen/pin it. Out of scope for v1 unless a real case
  appears; the default covers the motivating examples.
- **Edge cases the compiler must handle:** an input reachable only through a constant path
  (exclude); an input that feeds *no* tool (lint: "feeds no tool"); shared intermediates feeding
  multiple tools (each tool lists the union of its own upstream leaves).

---

## 5. Tool & server naming

- Server name **prefixes** every tool → **no uniqueness engineering**. Focus is bundle *quality*
  for LLM selection: `server name + tool name + description + I/O schema` reading as a coherent
  "what I do / how to call me."
- Tool name = the output table's Excel name, mapped to MCP's `^[a-zA-Z0-9_-]{1,64}$` (sanitize
  casing/charset; reject empty).
- Tool description = caption cell above the table. Field descriptions = `description` columns.

---

## 6. Governance & provenance

- **Per-field governance moves into columns** (`tier` dropdown). Strict constants are not
  caller-exposed inputs. The shipped **template** carries the correct headers, number formats,
  and the `tier` dropdown pre-wired — so governance is "fill in the standard form," business-friendly.
- **Provenance is orthogonal and unchanged.** The `calcPr`/`app.xml` identity the provenance gate
  checks is workbook-level, read from raw parts via quick-xml (not umya/tables). The table redesign
  does not touch it.

---

## 7. The shipped template (one artifact, three jobs)

A single `template.xlsx` is simultaneously: the **BA starting point**, the **documentation/training
artifact** (the diagram made real), and the **honest reference fixture** (replacing the misleading
hand-authored `synthetic-fixture`, and the `tax-calc`/`leap1900` fixtures that masked the original bug).

Template contents:
- `0_meta` (optional): server name hint, version, governance defaults.
- `Inputs` Table: headers `name | value | description | tier`; `value` cells pre-formatted
  (currency/percent samples); `tier` column pre-wired with a `{variable, strict}` dropdown; an
  example field with a `{...}` dropdown to demonstrate enum harvest.
- one or more calc sheets + a sample VLOOKUP/rate table (reference region).
- Output Table(s) named with tool name(s), a caption row above each holding the description,
  columns `name | value | description`, value cells carrying realistic expected results.

> **Production note:** the template `.xlsx` must be authored so it passes the provenance gate
> (same discipline used for `tax-calc.xlsx` — preserve genuine Excel provenance identity, not a
> umya-fabricated one). Generating it is implementation step 1.

### Annotated reference (the diagram)

```
0_meta (optional)         server: tax-suite   version: 1
─────────────────────────────────────────────────────────────────────
Table "Inputs"            name      | value    | description            | tier ▼
                          income    | 100000   | annual gross (USD $)   | variable
                          filing    | single ▼ | filing status          | variable   ← enum from dropdown
                          withheld  | 15000    | tax withheld YTD (USD) | variable
                          rate      | 0.22     | statutory bracket rate | strict     ← not caller-exposed
                                                  └ value: type+unit+example+enum+test seed
        │ (formula DAG, cross-sheet)            ▲
        ▼                                        │ VLOOKUP(ref_brackets)
   ┌─ calc sheets + lookup/reference regions (DAG interior, not exposed) ─┐
        │                                        │
        ▼                                        ▼
"Calculate Tax"  ← caption: "Compute federal tax from income & filing"   [TOOL]
Table            name           | value | description
                 tax_owed       | 18241 | federal tax liability (USD)
                 effective_rate | 0.182 | effective tax rate (%)
                   inputs (DAG-derived): income, filing

"Estimate Refund" ← caption: "Estimate refund given withholding"         [TOOL]
Table            name    | value | description
                 refund  | -3241 | estimated refund (neg = owed)
                   inputs (DAG-derived): income, filing, withheld
```

Four region types to teach; two tools emitted; each tool's inputs derived automatically; the
whole contract + its end-to-end test case authored in visible Excel Tables.

---

## 8. BA ergonomics — fail *helpful*, preview before deploy

- **Fail-helpful linting** (compile errors name the exact cell/row): blank `name`, duplicate key,
  value-less row, an input that feeds no tool, an output table with no caption (missing tool
  description), a tool name that can't map to MCP charset.
- **Dry-run preview:** `cargo pmcp workbook explain <file>` renders "here is the tool surface an
  AI will see" (tool names, descriptions, per-tool input/output schemas) **before** deploy — the
  single best guard against the silent-broken-deploy class.
- The template + the preview together replace the invisible-named-range failure mode entirely.

---

## 9. Cleanup plan (relative to the merged F1/F2/F3)

| Merged piece | Fate under this contract |
|--------------|--------------------------|
| **F2** — advertise override keys in schema | **KEEP** — independent of the input model. |
| **F3 (outputs)** — strip `out_` prefix | **SUBSUMED** — output keys come from the output table `name` column; no prefix to strip. Remove the output-side strip once tables land. |
| **F3 (inputs)** — strip `in_` prefix | **RETIRE** — no `in_*` named ranges anymore. |
| **F1** — hard-error on missing `in_*` named range | **RETIRE / RESHAPE** — replaced by table-row linting (blank `name`, etc.). The *intent* (fail loud on an uncallable bundle) is preserved by the new linter + preview. |
| **`json_key_for_role` strip + `strip_governance_prefix`** | **RETIRE** once keying moves to table `name`; keep the function only if still used for legacy paths during transition (none expected pre-1.0). |
| **Fixture `in_*` injections** (tax-calc/leap1900) | **REPLACE** with the table-based template/reference fixture. |

Sequencing: do **not** fold this into PR #279 (deploy-stack-ts). Land #279 with **F2 only** of the
workbook changes (or hold the workbook pieces out of it), and seat this contract as its own phase so
the breaking surface changes ship once, coherently, before any release.

---

## 10. Decisions ledger

**Locked:**
- Excel Tables (ListObjects) as the declaration primitive (umya-verified).
- Standard columns `name | value | description [| tier]`; iterate rows at compile time.
- `value` = example + type + unit + enum + test seed + oracle.
- Output table → MCP tool; table name = tool name; caption = tool description.
- Per-tool inputs **DAG-derived** by default; explicit declaration as a future override.
- Co-locate all descriptions (no `0_meta` field/tool duplication); `0_meta` = workbook/server-level only, optional.
- Governance via columns + shipped template; provenance untouched/orthogonal.
- Server-name prefix → no tool-name uniqueness engineering.
- No legacy compat; retire the per-cell named-range model; keep F2.

**Open (resolve during phase plan, not blocking the spec):**
- Exact `0_meta` key set (and whether it's fully optional).
- Whether v1 ships the explicit per-tool input *override* or DAG-derived only.
- `cargo pmcp workbook explain` output format (text first; JSON for tooling later).

---

## 11. Suggested phasing

1. **Template + reference workbook** (provenance-valid `.xlsx`) — the anchor + fixture.
2. **Reader/ingest: harvest tables** — `get_tables()` → input/output field model (type/unit/enum/tier).
3. **Manifest model → multi-tool** + DAG-derived per-tool inputs.
4. **Emit named tools** — name/description/inputSchema/outputSchema (+ `structuredContent`).
5. **Gate + linting** — per-tool reconciliation; fail-helpful row lints; retire F1/F3-input.
6. **`cargo pmcp workbook explain`** dry-run preview.
7. **Docs/training** — pmcp-book + pmcp-course chapters seeded from this spec and the template.

Deliverables 1–6 are SDK; 7 is the BA-facing training story ("your Excel process becomes a
governed, AI-callable tool").
