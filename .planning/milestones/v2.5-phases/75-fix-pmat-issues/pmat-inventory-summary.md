---
inventory_snapshot: pmat-inventory-2026-04-22.json
pmat_version: pmat 3.15.0
date: 2026-04-23
commit_sha: 52af6cdd7fe3f195bd6f48d92bcc61b0a7783e0b
---

# PMAT Inventory Snapshot — Authoritative Counts

## Source of truth

Every Wave-N success criterion derives from
`pmat-inventory-2026-04-22.json` via `jq`, NOT from prose counts in
CONTEXT.md or RESEARCH.md.

The snapshot was generated with:

```bash
pmat analyze complexity --format json --max-cognitive 25 --top-files 0
```

Then normalized (see `pmat-inventory-2026-04-22.json` keys
`pmat_version`, `generated_at`, `source_command`, `summary`,
`violations`, `files`) so `jq '.violations | length'` works at the
top level.

## Top-line counts

- `pmat analyze complexity --format json --max-cognitive 25 --top-files 0`
  total violations (cognitive + cyclomatic combined): **166**
  - cognitive-complexity rule (max 25): **91**
  - cyclomatic-complexity rule (max 30): **75**
- `pmat quality-gate --fail-on-violation --checks complexity --format json`
  total violations: **94**
- **Badge-relevant count (use this for wave deltas):** **94** (the gate count)

The two commands give different numbers because:
- `pmat analyze complexity` walks every file under the project root (including `examples/`, `fuzz/`) and reports BOTH cyclomatic and cognitive thresholds.
- `pmat quality-gate --checks complexity` applies a different in-built filter (it reports 0 examples/ violations even though `analyze` reports 25). The filter mechanism is opaque from the help text — this is one of the asymmetries CONTEXT.md flagged. Empirically the gate is a SUBSET of `analyze` results.

For wave delta tracking, the **gate count (94)** is what matters because the badge / CI gate exit code is the binding goal per D-01.

## Per-top-level-directory breakdown (from `pmat analyze` superset)

| Top-level dir | Cognitive (max 25) | Cyclomatic (max 30) | Combined |
|---------------|-------------------:|--------------------:|---------:|
| cargo-pmcp/   | 42                 | 35                  | 77       |
| examples/     | 14                 | 11                  | 25       |
| src/          | 13                 | 10                  | 23       |
| crates/       | 11                 | 10                  | 21       |
| pmcp-macros/  | 7                  | 5                   | 12       |
| fuzz/         | 4                  | 4                   | 8        |
| **total**     | **91**             | **75**              | **166**  |

## Per-top-level-directory breakdown (from `pmat quality-gate --checks complexity` — gate-relevant)

| Top-level dir | Gate count (cognitive + cyclomatic combined, gate-filtered) |
|---------------|------------------------------------------------------------:|
| cargo-pmcp/   | 51                                                          |
| src/          | 14                                                          |
| crates/       | 14                                                          |
| pmcp-macros/  | 7                                                           |
| fuzz/         | 5                                                           |
| packages/     | 3 (TypeScript files)                                        |
| examples/     | 0                                                           |
| **total**     | **94**                                                      |

## Reconciliation note

Earlier prose counts in 75-CONTEXT.md (94 complexity / 73 in-scope src
/ 21 examples / 3 fuzz) and in 75-RESEARCH.md were gathered against
**pre-Phase-76 HEAD** (2026-04-22). Phase 76 (cargo-pmcp IAM
declarations) landed on main between then and Wave 0 execution
(2026-04-23). The current authoritative numbers are above.

**Material divergences from CONTEXT.md figures (pre-76 vs post-76):**

| Metric                              | CONTEXT.md (pre-76) | This snapshot (post-76) | Δ      |
|-------------------------------------|--------------------:|------------------------:|-------:|
| complexity (gate)                   | 94                  | 94                      | 0      |
| duplicate (gate, bare run)          | 439                 | 1545                    | +1106  |
| satd (gate, bare run)               | 33                  | 33                      | 0      |
| entropy (gate, bare run)            | 4                   | 13                      | +9     |
| sections (gate, bare run)           | 2                   | 2                       | 0      |
| in-scope src/ count                 | 73                  | 86                      | +13    |
| examples/ count (gate)              | 21                  | 0                       | -21    |
| fuzz/ count (gate)                  | 3                   | 5                       | +2     |

**Material findings:**

1. **Complexity gate count (94) is unchanged** — Phase 76's net effect on the gate-relevant complexity count was zero (some functions added in cargo-pmcp/deploy IAM paths, but they may have been balanced by cleanup elsewhere; or PMAT's gate-internal filter masks them).
2. **Examples/ count dropped from 21 → 0** under the gate — most likely Phase 76 introduced a PMAT config or `.pmatignore`-style filter, OR earlier cleanup was already in flight. The `analyze complexity` superset still shows 25 examples violations, so they exist; the gate just doesn't count them. Wave 5 should preserve this filter behavior.
3. **Duplicate count tripled (439 → 1545)** — a Phase 76 side-effect. Duplicates are not gating per D-01, so this is informational. May want a follow-on housekeeping pass post-Phase-75.
4. **Entropy count tripled (4 → 13)** — also Phase 76 side-effect; not gating.
5. **In-scope src/ count grew (73 → 86)** — explainable: Phase 76 added cargo-pmcp/src/commands/deploy IAM-validator paths and the iam.rs validator module; these contain new branchy validation logic.

The above 94/91/75 numbers are the operative baseline for Waves 1-5; the CONTEXT.md prose numbers should be considered superseded.

## Phase-76-dependency-inversion flag

Phase 76 was authored to depend on Phase 75 (Phase 75 fixes existing
PMAT debt; Phase 76 adds new functionality on top). In practice Phase
76 shipped first. The inversion is benign for **complexity** counts
(unchanged at 94), but introduced significant **duplicate** and
**entropy** count growth (1106 new duplicate violations, 9 new entropy
violations). Neither is gating per D-01 so Phase 75 closure is not
blocked, but downstream housekeeping work (out of scope here) will
inherit the larger duplicate count.

## How later waves use this

Each Wave's `<verification>` block now includes:

```bash
BASELINE=$(jq '.violations | length' .planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json)
CURRENT=$(pmat analyze complexity --format json --max-cognitive 25 --top-files 0 | jq '.violations | length')
echo "wave delta (analyze, all-rule): $((BASELINE - CURRENT))"
```

For the badge-relevant gate count:

```bash
pmat quality-gate --fail-on-violation --checks complexity --format json 2>/dev/null \
  | sed -n '/^{/,/^}/p' \
  | jq '.results.complexity_violations'
```

Expected delta per wave (informational, not gating; updated for D-10-B "no #[allow] shortcut"):

| Wave  | Description                                            | Gate count drop estimate |
|-------|--------------------------------------------------------|--------------------------:|
| 1a    | streamable_http_server hotspots (6 fns, all refactor) | -6                        |
| 1b    | pmcp-macros hotspots (7 fns, all refactor)            | -7                        |
| 2     | cargo-pmcp/pentest + deploy + scattered cmds (51 fns) | -51                       |
| 3     | pmcp-code-mode hotspots (5 fns, all refactor)         | -5                        |
| 4     | .pmatignore for fuzz/, packages/, examples/ defensive | -8 (5 fuzz + 3 packages, examples already 0) |
| 4-src | other in-tree src/, crates/ residue (≈17 remaining)   | -17                       |
| **net** | reduce 94 → 0                                         | **-94**                  |

Wave 1a + 1b + 2 + 3 alone clear 69 of 94. Remaining 25 must be picked up by Wave 4 + 4-src (other in-scope hotspots not in the named directories). The in-scope superset (86) minus named-hotspot subtotal (51 cargo-pmcp + 7 pmcp-macros + 6 streamable_http + 5 pmcp-code-mode = 69) = 17 residual gates that Wave 4-src must address.
