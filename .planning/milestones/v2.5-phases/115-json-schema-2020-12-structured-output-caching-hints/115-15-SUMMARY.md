---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 15
subsystem: testing
tags: [schm-01, json-schema, draft-2020-12, gap-closure, metamorphic-testing, rename-invariance, property-testing, fuzzing, derived-vs-restated-fences]

# Dependency graph
requires:
  - phase: 115-14
    provides: "the shipped position-aware traversal rule (`SUBSCHEMA_MAP_KEYWORDS`, `first_legacy_dialect_in_member` / `pin_dialect_in_member`) that both restated copies must now agree with, and the VACUOUS-POSTCONDITION measurement this plan's WR-02 work builds on"
  - phase: 115-13
    provides: "the widened generators whose restated copies of the traversal rule this plan corrects, and the hard-coded `\"Inner\"` (WR-06) it parameterizes"
  - phase: 115-03
    provides: "the `fuzz_support` seam (`normalize_bytes`) both generators drive"
provides:
  - "`property_normalization_does_not_depend_on_a_subschema_map_key_name` — a metamorphic fence DERIVED from a JSON Schema 2020-12 vocabulary fact rather than restated from pmcp's keyword lists, observed to FAIL against a position-blind normalizer"
  - "fuzz invariant 6 `assert_normalization_is_invariant_under_rename` — the same relation in the fuzz target, and the ONLY fence in this repo proven to fire when BOTH restated copies of the rule are also wrong"
  - "`arb_definition_name` / `arb_container` — a property generator that can DRAW the four colliding names and three containers (WR-06 discharged); 58 of 256 cases drew one with an embedded non-2020-12 dialect"
  - "Both restated copies of the traversal rule brought onto the shipped position rule, closing the FALSE-POSITIVE window 115-14-SUMMARY named"
  - "Seed `14_defs_named_default` — the 115-VERIFICATION.md reproduction document, committed (14 tracked seeds)"
  - "Invariant 5's two false module-doc claims (\"TOTAL — no skip condition\", \"INDEPENDENT\") corrected in place, amend-not-delete"
  - "SCHM-01 re-booked `[x]` on evidence covering the colliding-name case, written AFTER the whole-phase gate ran"
  - "`D-115-AF` (a fence specified to probe only the FIRST entry was blind to this phase's own seed) and `D-115-AG` (this round's process outcome + the standing marker rule)"
affects: [116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A fence that RESTATES the implementation's rule is an AGREEMENT check between two copies of one rule, satisfied VACUOUSLY when the rule is wrong. Evidence ABOUT a rule must be DERIVED from something outside the implementation — here a JSON Schema 2020-12 vocabulary fact"
    - "Rename invariance as the metamorphic relation for name-vs-keyword confusion: the keys of `properties`/`patternProperties`/`$defs`/`definitions`/`dependentSchemas` are semantically inert author-chosen names, so normalizing an entry cannot depend on the name it is filed under. Consults no keyword list at all, and fires on FUTURE rule defects too"
    - "When a negative control fires, check WHICH fence fired. A stronger fence firing first MASKS a weaker one that never ran — run the control in the configuration that silences the fences you are not trying to measure (`D-115-AF`)"
    - "A widened generator must be PROVEN to emit the new shape. Instrument, count, record, remove — 58 of 256 here, 100 of 256 in 115-13. A widened space that never draws the new case is the same failure this phase shipped once already"
    - "Compare SUBTREES, not whole documents, when the metamorphic transform legitimately changes something else (here the `$ref` strings, which normalization never resolves)"

key-files:
  created:
    - fuzz/corpus/fuzz_schema_draft_pin/14_defs_named_default
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-15-SUMMARY.md
  modified:
    - tests/property_tests.rs
    - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
    - fuzz/corpus/fuzz_schema_draft_pin/README.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "Invariant 6's entry selection was WIDENED past the plan's literal spec after MEASURING that the specified form did not fire on this phase's own reproduction seed. `D-115-AF` — the finding, the measurement and the ~3% cost are recorded rather than absorbed"
  - "`src/` was NOT touched (0-byte diff). This plan is the structural half; 115-14 owns the fix. `git diff fc674e40..HEAD -- src/` shows 0 new public API items across the whole closure"
  - "`gsd-sdk query requirements.mark-complete SCHM-01` was deliberately NOT run: the checkbox was already `[x]` and the traceability row was hand-written with the two-round closure detail the tool would overwrite with a generic string"
  - "`prop_assert_ne!` rather than `prop_assume!` for the probe-name collision guard — the collision is structurally unreachable (16-char probe vs a 7-char regex ceiling), so asserting it discards nothing and keeps the effective sample size at 256"
  - "The property's rename check compares SUBTREES, not whole documents: the two documents' `$ref` strings legitimately differ and normalization never resolves refs"
  - "`is_neutral_subschema` in the fuzz target was deliberately left ALONE — it was already position-aware — with a DO-NOT-\"FIX\" comment, because that is why 115-VERIFICATION.md measured invariant 3 as correctly SKIPPING the defective document"

patterns-established:
  - "The decisive negative control for a derived fence is the BOTH-BLIND configuration: make the implementation AND every restated copy wrong at once, so the restating fences pass vacuously and only the derived one can fire"
  - "A booking task placed last in the plan, with its marker gated on measured exit codes and named counts, is the mechanism that stops `D-115-G` recurring — this requirement has now paid for that lesson twice"

# Metrics
duration: ~75m
completed: 2026-08-02
tasks_completed: 3
files_modified: 7
---

# Phase 115 Plan 15: Rename Invariance — the Fence a Rule Defect Cannot Satisfy

Closed the STRUCTURAL half of `115-VERIFICATION.md`'s gap. `115-14` fixed the traversal; this plan
fixed the reason nothing caught it. All three defensive layers `115-12`/`115-13` built RESTATE the
same `DATA_ONLY_KEYWORDS`-per-key rule as the code under test, so a defect in that RULE was invisible
to every one of them. Both restated copies are now on the shipped position rule, the property
generator can DRAW the colliding names, and both generators gained a metamorphic fence — **rename
invariance** — whose invariant is DERIVED from a JSON Schema 2020-12 vocabulary fact rather than
restated from pmcp's source. SCHM-01 was re-booked last, after the gate had actually run.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Draw the colliding name, position-aware walkers, rename invariance | `43246c19` | `tests/property_tests.rs` |
| 2 | Position-aware fuzz walkers, invariant 6, seed `14_defs_named_default` | `fb97b23d` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `fuzz/corpus/fuzz_schema_draft_pin/{README.md,14_defs_named_default}` |
| 3 | Run the CI-equivalent gate, THEN correct SCHM-01's booking | `d666fffa` | `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `deferred-items.md` |

`src/` was **not touched** — a 0-byte diff. The two temporary reverts below were restored from
`shasum -a 256 -c`-verified snapshots and are in no commit.

## THE COVERAGE PROOF — a widened generator that never emits the new shape is not a widened generator

`arb_definition_name()` draws `Just("Inner")` (the control), the four colliding literals
`const`/`enum`/`default`/`examples`, and a `[a-zA-Z_][a-zA-Z0-9_]{0,6}` regex.
`arb_container()` draws `$defs` / `definitions` / `properties`.

Instrumented with a temporary `println!` behind `--nocapture`, counting cases that drew a colliding
name **together with** an embedded non-2020-12 dialect:

```
58 of 256      (floor: 20; strategy's expected rate ≈ 0.667 × 0.5 × 5/7 ≈ 61)
```

A second run broke it down and hit **all 12** container × colliding-name combinations:

| | `$defs` | `definitions` | `properties` |
|---|---|---|---|
| `const` | 3 | 7 | 1 |
| `enum` | 4 | 4 | 3 |
| `default` | 8 | 6 | 7 |
| `examples` | 2 | 6 | 6 |

(57 in that run; proptest reseeds per run.) **The instrumentation is removed** —
`grep -n 'TEMP_COVERAGE_PROOF\|TEMP-COVERAGE-PROOF' tests/property_tests.rs` returns nothing.

**SEP-2106 by inspection:** `grep -n 'https://example.test'` still shows the `$id` value (the
reserved, non-resolvable host), and the file's ONLY `$ref` construction is
`format!("#/{container}/{name}")` — a LOCAL JSON pointer. No generated `$ref` carries an
`http`/`https`/`file` scheme at any draw. Drawn names cannot contain `/` or `~`, so no RFC 6901
escaping is needed.

## THE NEGATIVE CONTROLS — three observed, and the third is the one that matters

An unfired fence is not evidence. That is the standard `115-VERIFICATION.md` applied when it refused
to inherit the SUMMARYs' conclusions.

### 1. The property test (Task 1)

With the position-BLIND member filter restored in `src/server/output_validation.rs`:

```
Summary [0.479s] 10/20 tests run: 9 passed, 1 failed, 0 skipped
FAIL  schema_dialect_normalization_properties::property_normalization_does_not_depend_on_a_subschema_map_key_name
```

> RENAME INVARIANCE VIOLATED at `/$defs/const` vs `/$defs/__rename_probe__`. The keys of properties /
> patternProperties / $defs / definitions / dependentSchemas are AUTHOR-CHOSEN NAMES with no keyword
> semantics under the JSON Schema 2020-12 core and applicator vocabularies, so normalizing an entry
> CANNOT depend on the name it is filed under. A difference here means the traversal is treating a
> NAME as a KEYWORD — the 115-VERIFICATION.md defect class, measured as `$defs.default -> verdicts=(Conforms,
> Conforms), rewritten=false` against the control `$defs.Inner -> (Conforms, Violates), rewritten=true`.
> This invariant is DERIVED from the spec, not restated from the crate's keyword lists, which is why
> it fires where the purity assertion above passes vacuously.

Shrunk counterexample: `container: "$defs"`, `original_name: "const"` — one of the four colliding
literals, exactly as required. `left` carried `"$schema": "…draft-04…"`, `right` carried the 2020-12
URI.

### 2. Seed 14 under the fuzz target, `src/` blind only (Task 2)

`cargo +nightly fuzz run fuzz_schema_draft_pin corpus/fuzz_schema_draft_pin/14_defs_named_default`
→ **exit 1**, naming **invariant 5**:

> A LEGACY $schema SURVIVED NORMALIZATION: ["http://json-schema.org/draft-07/schema#"] … Input was:
> `{"type":"object","properties":{"n":{"$ref":"#/$defs/default"}},"$defs":{"default":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}`,
> normalized to: *(byte-identical)*

Invariant 5 can see it **now** only because Task 2(a) made its scan position-aware. Before this plan,
scan and rewriter were blind together and agreed.

### 3. BOTH copies of the rule blind — the decisive measurement

This is the configuration that reproduces the pre-`115-14` world: `src/` blind **and**
`collect_dialect_declarations_in_member` blind, so invariants 2 and 5 both pass vacuously exactly as
they did then. Seed 14 → **exit 1**, naming **invariant 6**:

> RENAME INVARIANCE VIOLATED. … This invariant is DERIVED from the spec rather than restated from the
> crate's keyword lists, which is precisely what invariants 2 and 5 are not — a defect in the rule
> they share satisfies both of them.

That is the direct, measured proof of this plan's thesis. It is also how `D-115-AF` was found: under
the plan's literal first-entry bounding, **this same run exited 0**.

The third control from this round is cited, not re-run: `115-14-SUMMARY.md` records **16 passed / 2
failed** against the position-blind body, with the `BYPASS ($defs.const)` and borrow/own messages.

All reverts restored; `git status --short src/` empty; `shasum -a 256 -c` **OK** on both snapshots.

## The rename relation, and why it is not just a fourth restatement

`115-REVIEW.md` WR-02 does not ask for the same walk written a fourth time in a different type. It
asks for an invariant a RULE defect cannot satisfy.

A keyword-list-free TOTAL scan was rejected: it fires on legitimate DATA (a `$schema` inside a real
`const` payload must survive), so it needs a precondition excluding documents containing a data-only
keyword — expressible only BY NAME, and the defect's shape is a `$defs` entry NAMED `default`. The
precondition would skip exactly the case it must catch.

Rename invariance is a metamorphic relation instead. From the 2020-12 core and applicator
vocabularies: the keys of `properties`, `patternProperties`, `$defs`, `definitions` and
`dependentSchemas` are AUTHOR-CHOSEN NAMES with no keyword semantics. Therefore **normalizing an
entry must not depend on the name it is filed under**, and two documents differing only in that name
must normalize to equal subtrees. It consults no `DATA_ONLY_KEYWORDS` list at all. The only list it
needs is the five container keywords, declared from the spec — small, closed and citable.

Subtree equality, not whole-document equality: the `$ref` strings legitimately differ
(`#/<container>/<name>` vs `#/<container>/__rename_probe__`) and normalization never resolves refs.

## Both restated copies are now on the shipped rule

`115-14-SUMMARY.md` named the window this closes: with the fix landed and these copies blind, an
input shaped `{"properties": {"$schema": "http://json-schema.org/draft-07/schema#"}}` — a `properties`
entry NAMED `$schema` bound to a non-schema — is correctly left alone by the shipped walk while a
blind strip removes it from one side of the comparison and a blind scan reports it as surviving.
Both are FALSE POSITIVES against correct code, and the fuzz one crashes the fuzzer.

Each copy gained the same three-way member dispatch as `pin_dialect_in_member`, split into
`*_in_member` helpers so the mirror is visible:

1. key in `SUBSCHEMA_MAP_KEYWORDS` **and** value is an object → recurse into every VALUE; the map's
   own keys are never keyword-filtered;
2. same key, non-object (malformed) value → ordinary walk, so no coverage is lost;
3. otherwise → the `DATA_ONLY_KEYWORDS` skip, unchanged.

`is_neutral_subschema` was deliberately **left alone** — it already descended into `$defs`/`properties`
values by name, which is why invariant 3 correctly SKIPPED the defective document — and carries a
`DO NOT "FIX" THIS` comment so a later reader does not "repair" what is already right.

## Invariant 5's two false claims, corrected in place

115-13 documented invariant 5 as *"TOTAL — no skip condition, no neutrality reasoning — so it holds
for every input that parses as JSON"* and its scan as *"implemented INDEPENDENTLY"*. Both are false
as written, and both are now quoted beside their correction (amend, never delete):

- the scan **does** have a skip condition — it must not descend into a data-only payload, or a
  `$schema` that is instance DATA gets reported. The invariant is total over **SCHEMA POSITIONS**;
- independence in IMPLEMENTATION is not independence in RULE. It catches a detector/rewriter
  DISAGREEMENT and cannot catch a defect in the rule they share — measured, since all three copies
  agreed there was nothing at `$defs.default`.

Invariant 6 is added to the numbered list with its derivation stated.

`grep -n 'TOTAL — no skip condition'` returns **nothing** — rustfmt wraps the quoted claim across two
lines — and the quotation lives inside its own correcting sentence, which is what the criterion
allows.

## The corpus

Seed `14_defs_named_default` written with the README's `python3` heredoc, selector byte 1, 216 bytes:
root `type: "object"`, `properties.n` = `{"$ref": "#/$defs/default"}`, `$defs.default` = `$id` +
draft-07 `$schema` + `type: integer`, no root `$schema`, instance `{"n": "NOT-AN-INTEGER"}`. The
`115-VERIFICATION.md` reproduction document verbatim.

The README's acceptance-check sentence (WR-07) is corrected: `ls | grep -c '^[0-9]'` counts
libFuzzer's hex-named runtime artifacts, which land in the same gitignored directory and often begin
with a digit — it returns thousands where the answer is 14. Replaced with
`git ls-files fuzz/corpus/fuzz_schema_draft_pin/ | grep -c '/[0-9][0-9]_'`, which returns **14**.

| Run | Result |
|---|---|
| `cargo +nightly fuzz build fuzz_schema_draft_pin` | exit **0** |
| `-runs=0 corpus/fuzz_schema_draft_pin` | exit **0**, **15 996** runs, artifacts dir EMPTY |
| `-max_total_time=300` | exit **0**, **3 697 874** runs, artifacts dir EMPTY |

The replay's run count exceeds the seed count because cargo-fuzz ALSO passes its default corpus
directory, so the same 7 994 directory entries load twice (7 994 = 14 tracked seeds plus gitignored
libFuzzer units from earlier campaigns; documented by `115-13`).

`git status --porcelain fuzz/corpus/fuzz_schema_draft_pin/` showed **exactly** the one new seed plus
the modified README — no runtime-discovered unit leaked past `.gitignore` (T-115-POS-10).

## Gate results (Task 3, run BEFORE any booking was touched)

Free space checked first (`D-115-0`): **52 Gi** available on `/System/Volumes/Data`.

| Check | Result |
|---|---|
| `/usr/bin/make quality-gate` | **exit 0** — **5054 passed / 0 failed / 81 ignored** across **309** `test result:` lines; **0** `test result: FAILED` lines |
| `pmat quality-gate --fail-on-violation --checks complexity` | **exit 0**, **0 violations** |
| Seven SCHM-02/03 binaries, combined | **78 passed / 0 skipped** |
| `binary(property_tests)` `--features "full fuzzing"` | **20 passed** |
| `binary(property_tests)` `--features full` | **18 passed** |
| `cargo fmt --all -- --check` (root) | exit **0** |
| `cargo fmt --all -- --check` (from INSIDE `fuzz/`, `D-115-AB`) | exit **0** |

Per `D-115-T` no redirected `make` transcript is pasted — that is unfaithful in this environment; the
exit code and the parsed totals are the record.

**Per-binary counts, each matching `115-VERIFICATION.md` exactly — no deviation to report:**

| Binary | Observed | Expected |
|---|---|---|
| `structured_tool_output` | 20 | 20 |
| `v2_caching_hints` | 19 | 19 |
| `v1_lists_golden` | 7 | 7 |
| `v2_schema_tripwires` | 13 | 13 |
| `v2_core_schema_facts` | 8 | 8 |
| `vendored_schema_provenance` | 6 | 6 |
| `phase115_contract_bindings` | 5 | 5 |
| **Total** | **78** | **78** |

The 20-vs-18 pair on `property_tests` is the proof the `fuzzing`-gated module actually RAN rather
than being silently gated out (`D-115-Y`: a `test(/…/)` selector would have selected zero and exited
0 — `binary(...)` was used throughout).

Closure diff (`fc674e40..HEAD`, i.e. `115-14` + `115-15`): **no `Cargo.toml`, no `Cargo.lock`**
(T-115-SC closed by measurement, no supply-chain review triggered), and
`git diff -- src/ | grep -c '^+.*\bpub fn\|^+.*\bpub struct\|^+.*\bpub enum'` returns **0** — the
milestone's additive 2.x-minor posture holds without a `cargo public-api` run.

## The booking

**SCHM-01 reads `[x]`.** Every command above exited 0 and every count matched, which is the marker
rule this task exists to enforce.

`.planning/REQUIREMENTS.md` gained a new block ABOVE `115-13`'s, amending and not deleting it. It
states: that `115-13`'s `[x]` was premature **for the second time on this requirement** (`D-115-G`
recurring on the very requirement it was filed about — accurate for the cases it measured, but
generalized past them); the residual position-blind defect and the shipped fix; the measurement table
including `$defs.default` before/after and the structural `properties`-position row; the fences by
name with counts and gate visibility; the three observed negative controls; the structural finding and
the derived repair; and that this closure is option **(a)** of `115-VERIFICATION.md` § *Human
Verification Required*, so the owner's `115-10` sign-off is expressly not read as covering CR-01.

Grep-shaped criteria: `grep -c 'REOPENED'` still returns **1** (unchanged — the new block writes
around that word, which is the check proving the record was amended rather than removed);
`grep -c '115-14\|115-15'` returns **10**; `$defs.default` appears **5×** and `(Conforms, Violates)`
**4×**. The traceability row now names both rounds and the position-aware fix.

`.planning/ROADMAP.md`: a closure paragraph appended to the Phase 115 milestone tail with every
existing sentence intact, `115-15-PLAN.md` flipped to `[x]` (`115-14-PLAN.md` was already `[x]`), and
the plan count already read `15 plans (11 shipped + 2 gap-closure + 2 gap-closure round 2)` — the same
15, spelled with the rounds separated, so it was left as the more precise form. **The Phase 115 marker
stays `[~]`** and `115-VERIFICATION.md` was NOT edited: re-verification is `/gsd:verify-phase 115`'s
job and this plan set's output is the evidence it scores.

## Deviations from Plan

### 1. [Rule 1 — bug] Invariant 6's specified bounding was blind to this phase's own reproduction seed

- **Found during:** Task 2, negative control
- **Issue:** the plan specified invariant 6 probe "the FIRST member of the root object whose key is
  in `SUBSCHEMA_MAP_KEYWORDS` … [and] that map's FIRST entry". Implemented literally, then measured:
  in seed `14_defs_named_default` the first root-level subschema map is `properties` and its first
  entry is `n`, a plain `$ref` holder with no `$schema`. `$defs.default` was never probed. In the
  both-blind configuration the target on that seed exited **0** — a fence that cannot fire on the
  case it exists for, which is the exact failure mode this plan closes.
- **Fix:** widened to EVERY entry of EVERY root-level subschema map, split into
  `assert_entry_normalizes_the_same_under_any_name`. Subtrees are disjoint and nested containers are
  still not descended into, so the total stays linear in the document — the same order as invariant
  5's scan. Measured cost: **3 814 764 → 3 697 874** runs over the 300 s campaign, ≈ 3 %. Re-measured
  both-blind, the widened invariant exits **1** on seed 14 with `RENAME INVARIANCE VIOLATED`.
- **Files modified:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`
- **Commit:** `fb97b23d`; booked as **`D-115-AF`** with the standing lesson (when a control fires,
  check WHICH fence fired — a stronger one firing first masks a weaker one that never ran).

### 2. [Rule 3 — blocking] The ledger ID the plan asked for was already taken

- **Found during:** Task 3(e)
- **Issue:** the plan says "Append `D-115-AE`" and "115-14 took `AC` and `AD`". `115-14` in fact took
  **`AC`, `AD` and `AE`** — `AE` is its pmat `--max-cognitive 25` fail-open entry, filed as a
  deviation after this plan was written. A second `D-115-AE` would have broken the ledger's whole-ID
  duplicate check, itself an acceptance criterion of this same task.
- **Fix:** continued at `D-115-AF` / `D-115-AG`. `grep -c '^## D-115-AE'` still returns **1**
  (115-14's), which is the correct end state — reached by not writing a duplicate. The whole-ID
  duplicate check returns nothing.
- **Commit:** `d666fffa`; explained inside `D-115-AG` itself.

### 3. [scope, deliberate] `requirements.mark-complete` was not run for SCHM-01

The checkbox was already `[x]` (hand-edited per the plan's exact wording) and the traceability row was
hand-written to name both closure rounds and the position-aware fix. Running the SDK verb would have
overwritten that with a generic string, losing the detail the plan explicitly required. Recorded here
rather than absorbed.

### 4. [advisory, not fixed] Two pedantic clippy warnings in the property module

`clippy::single_match_else` fires on the `match nested_declared { None => …, Some(_) => … }` arms —
one pre-existing at the root-key check, one on the embedded-resource check this plan moved. They are
**not gate-visible**: `make lint` runs `--features "full"`, and the whole module is
`#[cfg(all(test, feature = "fuzzing", feature = "validation"))]`, proven by the 20-vs-18 count. The
arms carry long explanatory comments that an `if` would not host as readably. Left as-is; out of scope
per the scope boundary.

## Threat Flags

None. No new network endpoint, auth path, file-access pattern or trust-boundary schema change. Every
generated and seeded `$id` uses the reserved non-resolvable `example.test` host and every `$ref` is a
local `#/<container>/<name>` pointer (T-115-POS-13); `binary(v2_schema_tripwires)` re-ran 13/13,
asserting `jsonschema`'s `default-features = false` against cargo's declared AND resolved graphs.
T-115-SC closed by measurement — no manifest edit anywhere in the closure.

## Self-Check: PASSED

Files:
- `tests/property_tests.rs` — FOUND; `SUBSCHEMA_MAP_KEYWORDS` **6×** (≥3), `arb_definition_name|arb_container` **10×** (≥4), `/$defs/Inner/$schema` **absent**, `TEMP_COVERAGE_PROOF` **absent**
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — FOUND; `assert_normalization_is_invariant_under_rename` **3×** (≥2), `SUBSCHEMA_MAP_KEYWORDS` **7×** (≥3), `TOTAL — no skip condition` returns nothing
- `fuzz/corpus/fuzz_schema_draft_pin/14_defs_named_default` — FOUND, 216 bytes, tracked; `git ls-files … | grep -c '/[0-9][0-9]_'` = **14**
- `fuzz/corpus/fuzz_schema_draft_pin/README.md` — FOUND; ``grep -c '^| `14_' `` = **1**
- `.planning/REQUIREMENTS.md` — FOUND; SCHM-01 `[x]`, `REOPENED` **1×**, `115-14|115-15` **10×**
- `.planning/ROADMAP.md` — FOUND; `115-14-PLAN.md` and `115-15-PLAN.md` both `[x]`, `15 plans` present, Phase 115 marker still `[~]`
- `deferred-items.md` — FOUND; `D-115-AF` **1×**, `D-115-AG` **1×**, `D-115-AE` **1×**, whole-ID duplicate check returns nothing

Commits: `43246c19`, `fb97b23d`, `d666fffa` — all FOUND in `git log`.
