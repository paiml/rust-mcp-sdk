---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 16
subsystem: server
tags: [schm-01, json-schema, draft-2020-12, gap-closure, subschema-map-keywords, dependencies, structural-fence, meta-schema-derivation, fuzzing-seam]

# Dependency graph
requires:
  - phase: 115-14
    provides: "`SUBSCHEMA_MAP_KEYWORDS` and the position-aware member dispatch this plan widens from five entries to six"
  - phase: 115-15
    provides: "the rename-invariance fences whose reliance on `SUBSCHEMA_MAP_KEYWORDS` is exactly why this plan's fence carries its own container literal"
  - phase: 115-03
    provides: "`compile_for_era` and the `fuzz_support` seam, and the `D-115-03-C` measurement that `jsonschema` 0.49.2 still honours `dependencies`"
provides:
  - "A SIX-entry `SUBSCHEMA_MAP_KEYWORDS`, established by a recorded DERIVATION over the meta-schema documents `jsonschema` 0.49.2 ships offline rather than by patching the one case a reviewer found"
  - "`v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map` — a STRUCTURAL fence over 6 containers x 4 colliding names, carrying its OWN container literal, collecting rather than aborting, OBSERVED to fail on exactly the four `dependencies` pairs"
  - "`embedded_legacy_resource_in_container(container, name)` — one fixture, two axes; `properties_embedded_legacy_resource_named` now delegates to it"
  - "`normalization_cases()` (h) `dependencies.default`, (i) `patternProperties.default`, (j) `dependentSchemas.default`, (k) `definitions.default` — WR-02's ask"
  - "`keyword_lists_are_disjoint` — WR-05's silent precondition of the two differently-shaped member dispatches, fenced with an observed failure"
  - "`fuzz_support::{DATA_ONLY_KEYWORDS, SUBSCHEMA_MAP_KEYWORDS}` — both lists published through the `fuzzing` seam so 115-17/115-19 can GATE the restated mirrors instead of hand-maintaining them a fourth time"
  - "Rustdoc stating the scope the walk ACTUALLY has, including the residual it does not cover (`components.default -> rewritten=false`)"
  - "`D-115-AH` — three plan-text/criterion defects, one of which (no pre-commit hook exists) invalidates a process rationale this phase has repeated five times"
affects: [115-17, 115-18, 115-19]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "An allow-list of SPEC-DEFINED keywords must be DERIVED from the spec documents the pinned library ships, not assembled from memory. Three rounds of this phase patched the one case a reviewer found; the fourth enumerated, and the enumeration is re-runnable in one jq command recorded in the rustdoc"
    - "A fence parameterised by the list whose incompleteness IS the defect cannot fire on that defect. The new fence carries its own six-element container literal and asserts list membership SEPARATELY, as a guard"
    - "Collect violations into a Vec and assert emptiness at the END. A first-failure abort reports one cell and hides the shape of the break; the collected form printed the complete 4-of-24 broken set, which is the evidence the task existed to produce"
    - "When the defect produces NO behavioural difference on the pinned library, the fence must be STRUCTURAL. Both `dependencies.Inner` and `dependencies.default` measure `(Violates, Violates)` on jsonschema 0.49.2, so a verdict assertion would have PASSED against the defective code"
    - "A negative control that PASSES can be the finding. Drifting the seam re-export to a stale literal left the suite green at 25 — proving nothing in `src/` catches seam drift, which is the justification for the next two plans' gates"
    - "Restore a temporarily-mutated file from a `/bin/cp` snapshot verified with `shasum -a 256 -c`, never with `git checkout --` or `git stash`"

key-files:
  created:
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-16-SUMMARY.md
  modified:
    - src/server/output_validation.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "The sixth entry was established by ENUMERATION over the five pinned meta-schema documents, with the selection criterion and BOTH rejected rows ($vocabulary, dependentRequired) recorded in the shipped rustdoc as a re-runnable jq command. The union is exactly the predicted six — no deviation from the plan's expectation"
  - "The fence is STRUCTURAL (Cow borrow/own + the rewritten pointer) and deliberately NOT behavioural: the defect produces no v2 verdict flip on jsonschema 0.49.2, measured three times independently, so a verdict assertion would be a fence that cannot fire — the exact failure mode this phase shipped three times"
  - "The fence iterates its OWN six-element container literal, never SUBSCHEMA_MAP_KEYWORDS. The list-membership check is a SEPARATE guard asserted AFTER the collected-violation assertion, so the control fires through the sweep rather than through the guard (D-115-AF: check WHICH fence fired)"
  - "`dependencies` is ordered LAST, after `dependentSchemas`, because 115-17/115-18's mirrors and 115-19's drift gate compare the three copies as ORDERED slices"
  - "The plan's inline-comment requirement and its ordered-last grep criterion are mutually unsatisfiable with a comment BLOCK; resolved with a trailing same-line comment plus the full rationale in the const's rustdoc. Booked as D-115-AH(1), a D-115-1 instance"
  - "WR-05's other half — rewriting the two member dispatches into a common shape — was deliberately NOT done: it touches both walkers' bodies and is 115-19's explicitly-not-owned ledger entry"
  - "No fuzz target was run. The two restated mirrors still carry the five-entry list, so a `dependencies`-shaped input crashes the fuzzer on CORRECT behaviour until 115-17/115-18 land"
  - "`.planning/REQUIREMENTS.md` deliberately UNTOUCHED and `requirements mark-complete` NOT run: SCHM-01's re-booking follows the whole-closure gate in 115-19, not this plan. Booking ahead of measurement is ledger D-115-G / D-115-AG, and this requirement has now carried that defect twice"

patterns-established:
  - "The negative control runs INSIDE the task — but the STATED reason (a pre-commit hook) does not exist in this checkout. Measured: `.git/hooks/` holds only `*.sample`. The practice is still right; its justification is honour-system, not mechanism (D-115-AH(3))"
  - "A jq filter over meta-schema `.properties` must guard `(.value|type)==\"object\"` FIRST — `draft7.json` binds `default` and `const` to booleans, and an unguarded `.value.type` exits 5 with the error on stderr and nothing on stdout, which is the criterion's own pass condition"

# Metrics
duration: ~55m
completed: 2026-08-02
tasks_completed: 2
files_modified: 2
---

# Phase 115 Plan 16: The `dependencies` Omission, Closed by Derivation Summary

Round 4 on SCHM-01. `SUBSCHEMA_MAP_KEYWORDS` was a five-entry allow-list that omitted
`dependencies` — the keyword this same module records at `D-115-03-C` as still honoured by
`jsonschema` 0.49.2 under the 2020-12 pin. The entry is now present, established by a **derivation
over the pinned meta-schema documents** rather than by patching the reviewed case, and fenced by a
**structural** instrument that was observed to fail on exactly the four `dependencies` pairs before
the entry landed.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Enumerate the spec-defined subschema-map keywords, land the structural fence RED, then add `dependencies` | `c350cb53` | `src/server/output_validation.rs` |
| 2 | Publish both lists through the `fuzzing` seam, fence disjointness, correct the falsified scope claims | `87d241b2` | `src/server/output_validation.rs` |
| — | Deviations booked | *(this commit)* | `deferred-items.md` |

## (a) THE DERIVATION — one row per (meta-schema file, keyword)

Run over `$HOME/.cargo/registry/src/index.crates.io-*/jsonschema-0.49.2/metaschemas/`, listing every
entry of each meta-schema's own `.properties` map that is OBJECT-typed and carries an
`additionalProperties`. **KEEP** when `additionalProperties` references the meta-schema itself;
**REJECT** otherwise.

| Meta-schema file | Keyword | `additionalProperties` | Verdict |
|---|---|---|---|
| `draft4.json` | `definitions` | `{"$ref":"#"}` | KEEP |
| `draft4.json` | `properties` | `{"$ref":"#"}` | KEEP |
| `draft4.json` | `patternProperties` | `{"$ref":"#"}` | KEEP |
| `draft4.json` | `dependencies` | `{"anyOf":[{"$ref":"#"},{"$ref":"#/definitions/stringArray"}]}` | KEEP |
| `draft6.json` | `definitions` | `{"$ref":"#"}` | KEEP |
| `draft6.json` | `properties` | `{"$ref":"#"}` | KEEP |
| `draft6.json` | `patternProperties` | `{"$ref":"#"}` | KEEP |
| `draft6.json` | `dependencies` | `{"anyOf":[{"$ref":"#"},{"$ref":"#/definitions/stringArray"}]}` | KEEP |
| `draft7.json` | `definitions` | `{"$ref":"#"}` | KEEP |
| `draft7.json` | `properties` | `{"$ref":"#"}` | KEEP |
| `draft7.json` | `patternProperties` | `{"$ref":"#"}` | KEEP |
| `draft7.json` | `dependencies` | `{"anyOf":[{"$ref":"#"},{"$ref":"#/definitions/stringArray"}]}` | KEEP |
| `draft2019-09/schema.json` | `definitions` | `{"$recursiveRef":"#"}` | KEEP |
| `draft2019-09/schema.json` | `dependencies` | `{"anyOf":[{"$recursiveRef":"#"},{"$ref":"meta/validation#/$defs/stringArray"}]}` | KEEP |
| `draft2019-09/meta/applicator.json` | `properties` | `{"$recursiveRef":"#"}` | KEEP |
| `draft2019-09/meta/applicator.json` | `patternProperties` | `{"$recursiveRef":"#"}` | KEEP |
| `draft2019-09/meta/applicator.json` | `dependentSchemas` | `{"$recursiveRef":"#"}` | KEEP |
| `draft2019-09/meta/core.json` | `$defs` | `{"$recursiveRef":"#"}` | KEEP |
| `draft2019-09/meta/core.json` | **`$vocabulary`** | `{"type":"boolean"}` | **REJECT** |
| `draft2019-09/meta/validation.json` | **`dependentRequired`** | `{"$ref":"#/$defs/stringArray"}` | **REJECT** |
| `draft2020-12/schema.json` | `definitions` | `{"$dynamicRef":"#meta"}` | KEEP |
| `draft2020-12/schema.json` | `dependencies` | `{"anyOf":[{"$dynamicRef":"#meta"},{"$ref":"meta/validation#/$defs/stringArray"}]}` | KEEP |
| `draft2020-12/meta/applicator.json` | `properties` | `{"$dynamicRef":"#meta"}` | KEEP |
| `draft2020-12/meta/applicator.json` | `patternProperties` | `{"$dynamicRef":"#meta"}` | KEEP |
| `draft2020-12/meta/applicator.json` | `dependentSchemas` | `{"$dynamicRef":"#meta"}` | KEEP |
| `draft2020-12/meta/core.json` | `$defs` | `{"$dynamicRef":"#meta"}` | KEEP |
| `draft2020-12/meta/core.json` | **`$vocabulary`** | `{"type":"boolean"}` | **REJECT** |
| `draft2020-12/meta/validation.json` | **`dependentRequired`** | `{"$ref":"#/$defs/stringArray"}` | **REJECT** |

### The two REJECTED rows and why

- **`$vocabulary`** — `additionalProperties` is `{"type": "boolean"}`. Its values are vocabulary
  ENABLEMENT FLAGS, not subschemas. Descending into it would find no schema position.
- **`dependentRequired`** — `additionalProperties` is a `stringArray` `$ref`. Its values are LISTS
  OF PROPERTY NAMES, not subschemas. (This is the half of draft-07 `dependencies` that 2020-12 split
  off; `dependentSchemas` is the other half, and only that half is a subschema map.)

### The union, as a set

```
{ properties, patternProperties, definitions, dependencies, dependentSchemas, $defs }
```

**Exactly SIX — no deviation from the plan's expectation.** `dependencies` is present in draft-04,
draft-06, draft-07, 2019-09 and (deprecated) 2020-12, and was absent from `SUBSCHEMA_MAP_KEYWORDS`.
`$defs`, `patternProperties` and `dependentSchemas` do not appear in the draft-04/06/07 rows because
those keywords do not exist in those drafts; `properties`/`patternProperties`/`dependentSchemas` sit
in the 2019-09 and 2020-12 **applicator** vocabulary files rather than the root `schema.json`, which
is why the sweep has to include `meta/*.json` and not just the two roots.

### The jq trap, confirmed rather than assumed

The plan predicted that an unguarded `.value.type` would exit 5. Measured: `draft7.json` binds
`default` and `const` to the boolean `true` inside `.properties`, and

```
jq -r '.properties | to_entries[] | select(.value.type=="object")' draft7.json
  exit=5
  jq: error (at draft7.json:170): Cannot index boolean with string "type"   [on STDERR]
```

— **nothing on stdout, which is that criterion's own pass condition.** This is the `D-115-AE` shape
exactly. The guarded form `select((.value|type)=="object")` is what the shipped rustdoc records.

### The scope boundary this closure does NOT cross

An author-invented container (`components`, any vendor extension) appears in NO meta-schema and is
therefore not in the derived set. `{"components": {"default": {…}}}` stays name-dependent —
`115-VERIFICATION.md` measured `rewritten=false` — and that residual is now **named in the shipped
rustdoc** where the false SUPERSET claim used to stand, rather than hidden. It is 115-19's ledger
re-booking, not this task's code.

## THE NEGATIVE CONTROL — recorded verbatim, run BEFORE the fix by construction

With the fence and the four `normalization_cases()` rows landed and `SUBSCHEMA_MAP_KEYWORDS` still
at **five** entries:

```
test result: FAILED. 17 passed; 2 failed; 0 ignored; 0 measured; 1793 filtered out
failures:
    server::output_validation::tests::normalize_schema_dialect_changes_only_dollar_schema_keys
    server::output_validation::tests::v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map
```

**Exactly 2 failed**, as predicted, no more and no fewer.

### Failure 1 — the container fence, with its COLLECTED violation vec

```
an $id-bearing embedded schema resource carrying a legacy $schema was NOT rewritten
in 4 of 24 (container, name) positions:
[
    "dependencies/const: rewritten=false, /dependencies/const/$schema=Some(\"http://json-schema.org/draft-07/schema#\")",
    "dependencies/enum: rewritten=false, /dependencies/enum/$schema=Some(\"http://json-schema.org/draft-07/schema#\")",
    "dependencies/default: rewritten=false, /dependencies/default/$schema=Some(\"http://json-schema.org/draft-07/schema#\")",
    "dependencies/examples: rewritten=false, /dependencies/examples/$schema=Some(\"http://json-schema.org/draft-07/schema#\")",
]
```

**The vec contains EXACTLY the four `dependencies` pairs and NO pair from the other five
containers** — `properties`, `patternProperties`, `$defs`, `definitions` and `dependentSchemas` all
swept clean, which is what makes this a statement about the OMISSION and not about the fixture.

**Which fence fired, and on which shape (`D-115-AF`, an acceptance criterion in its own right):** the
control fired through the **collected violation vec**, not through the
`SUBSCHEMA_MAP_KEYWORDS.contains(&"dependencies")` guard. The guard is asserted AFTER the vec
assertion precisely so this is observable — had the guard fired first, the fence would not have been
reaching the shape and would have needed widening. It did not.

### Failure 2 — the new `(h)` row

```
thread '…normalize_schema_dialect_changes_only_dollar_schema_keys' panicked at
src/server/output_validation.rs:1633:13:
assertion `left == right` failed: borrow/own decision is wrong for
{"type":"object","dependencies":{"default":{"$id":"https://example.test/inner",
 "$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}
 — the no-op cases must allocate nothing
  left: false
 right: true
```

`left: false` is the measured `rewritten=false`: the normalizer returned `Cow::Borrowed`, so
`compile_2020_12`'s `tracing::warn!` did not fire either and the author got **no signal at all**.

Rows `(i)` `patternProperties.default`, `(j)` `dependentSchemas.default` and `(k)`
`definitions.default` PASSED in the pre-fix run, as expected — those three have been in the list
since 115-14. They are WR-02's ask: exercised by no test, no property draw and no corpus seed until
now.

## Why the fence is STRUCTURAL and not behavioural

Rounds 1 and 2 were each caught by an actual accept-everything bypass — `(Conforms, Conforms)`
against a `(Conforms, Violates)` control. **This defect produces no such flip.** Both
`dependencies.Inner` and `dependencies.default` enforce `type` identically at `(Violates, Violates)`
on the pinned `jsonschema` 0.49.2, measured independently by the reviewer and the verifier.

A verdict assertion would therefore have **passed against the defective code** — a fence that cannot
fire, which is the exact failure mode this phase shipped three times. The observable is the
NORMALIZATION:

1. **the `Cow` borrow/own decision**, which is also precisely what `compile_2020_12` branches on to
   emit the only D-02 diagnostic an author gets, so asserting `Owned` covers the suppressed warning
   without a `tracing` subscriber; plus
2. **the rewritten pointer** `/{container}/{name}/$schema == DRAFT_2020_12`, which ties the rewrite
   to the position under test — `Owned` alone would be satisfied by a clone made for some other
   declaration.

The fence's container list is its **own six-element literal**, never `SUBSCHEMA_MAP_KEYWORDS`. Every
fence 115-15 added enumerates that constant, so an omission FROM it was invisible to all of them —
CR-01's sharpest sentence. Confirmed by reading the fence body: `SUBSCHEMA_MAP_KEYWORDS` appears in
that test **only** inside the `contains(&"dependencies")` guard (plus rustdoc and message prose); the
loop is `for container in containers`, over the local literal.

## Task 2's two negative controls — including the one that PASSED

### Control 1 — the disjointness fence (FIRED)

`"default"` temporarily appended to `SUBSCHEMA_MAP_KEYWORDS`:

```
test result: FAILED. 19 passed; 1 failed; …
---- server::output_validation::tests::keyword_lists_are_disjoint stdout ----
["default"] appear in BOTH SUBSCHEMA_MAP_KEYWORDS and DATA_ONLY_KEYWORDS. The two member
dispatches silently depend on these lists being disjoint: the DETECTOR
(first_legacy_dialect_in_member) is a `match` guarding on the VALUE kind and the key class
together, while the REWRITER (pin_dialect_in_member) is an `if` chain testing the KEY class
first. For such a key with a NON-OBJECT value the detector returns None while the rewriter
DESCENDS — a detector/rewriter divergence, which this module's own docs state is a defect,
yielding a Cow::Owned that still carries a legacy declaration while compile_2020_12 announces
that declaration as ignored. …
```

**Exactly ONE test failed, and it was the disjointness fence.** No other test in `mod tests` is
sensitive to list membership in that configuration — the plan asked for this to be recorded either
way, and nothing unpredicted surfaced.

### Control 2 — the seam re-export (PASSED, and that is the finding)

The `SUBSCHEMA_MAP_KEYWORDS` re-export temporarily replaced by a hand-written **stale five-entry
literal**:

```
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 1794 filtered out
```

**Exit 0. Nothing in `src/` catches seam drift today.** That is not a failure of this plan — it is
the measured justification for the two gates that follow:

- **115-17's compiled mirror test**, which will import `fuzz_support::SUBSCHEMA_MAP_KEYWORDS` and
  assert the property/fuzz copies equal it as ordered slices, so a drifted re-export becomes a
  compile-time-visible test failure rather than nothing; and
- **115-19's source-text drift gate**, which compares the three literal copies directly and is the
  only instrument that would catch a re-export that was itself hand-written to match a stale mirror.

Without one of those two, publishing the lists through the seam is necessary but not sufficient —
the seam makes the gate POSSIBLE, it is not itself the gate. Recording that distinction is the point
of running a control expected to pass.

### Restore discipline

Both controls were reverted and the file restored from a `/bin/cp` snapshot verified with
`shasum -a 256 -c` → **OK** (`a97f5cb2…3192c`). `git checkout --`, `git stash` and `git clean` were
not used.

## Measured results against every criterion

| Command | Expected | Observed |
|---|---|---|
| `cargo test --lib --features full output_validation::tests -- --test-threads=1` (pre-fix control) | 2 failed | **17 passed / 2 failed** ✓ |
| same, after Task 1 only | 19 passed | **19 passed / 0 failed** ✓ |
| same, after Task 2 | 20 passed | **20 passed / 0 failed** ✓ |
| `cargo test --lib --features "full fuzzing" output_validation -- --test-threads=1`, after Task 1 | 24 | **24 passed** ✓ |
| same, after Task 2 | 25 | **25 passed** ✓ |
| `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)' --test-threads=1` | 20, unchanged | **20 tests run: 20 passed** ✓ |
| `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | exit 0 | **exit 0** ✓ |
| `cargo fmt --all -- --check` | exit 0 | **exit 0** ✓ |
| `make lint` (pedantic + nursery, run after BOTH tasks) | exit 0 | **exit 0** ✓ |

The plan's note that Task 1 alone yields **19/24** and only Task 2 raises it to **20/25** held
exactly; 19 was not a shortfall.

### Grep-shaped criteria

| Criterion | Result |
|---|---|
| `grep -c '"dependencies"'` ≥ 1 | **4** ✓ |
| `grep -n 'dependentSchemas",$'` → `"dependencies",` on the FOLLOWING line | line 192 → line 193 `"dependencies", // draft-04..2019-09; …` ✓ |
| `SUBSCHEMA_MAP_KEYWORDS` in the new fence only inside the `contains` guard | confirmed by reading the fence body — the loop iterates the local `containers` literal ✓ |
| signature-move check | **0** ✓ — no function signature moved, so `contracts/binding.yaml`'s byte-for-byte records stay true |
| added `pub fn` / `pub struct` / `pub enum` in `src/` | **0** ✓ |
| added `pub const` in `src/` | **2** ✓ — hunk header `@@ -582,0 +671,37 @@ pub mod fuzz_support {`, so **both live inside `pub mod fuzz_support`**, which `fuzzing` keeps off `cargo public-api` |
| `grep -n 'SUPERSET'` returns the amended claim | 3 hits: the new `# What the walk is a SUPERSET of, and what it is NOT` heading, the sentence quoting the old false form, and the pre-existing in-test reference ✓ |
| file contains `components.default` | **2 hits** ✓ (the const rustdoc and the amended scope claim) |
| `grep -c 'dependentSchemas'` | **9** (pre-plan 4) |
| `grep -c 'dependencies'` | **29** — strictly greater than the pre-plan value, which is **2**, not the 3 the plan states (see `D-115-AH(2)`) |

Every place that enumerated five keywords now enumerates six — confirmed by reading each after the
edit: the module-header Era bullet, the `SUBSCHEMA_MAP_KEYWORDS` rustdoc, traversal-rule item 2, and
the `# Why the walk is position-aware` section. All four name `dependencies`.

## No fuzz target was run, and why

The two RESTATED copies of the traversal rule (`tests/property_tests.rs`,
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`) still carry the **five-entry** list. With the sixth
entry landed and those copies stale, an input shaped
`{"dependencies": {"default": {"$schema": "…draft-07…", …}}}` makes the fuzz target's invariant-2
strip asymmetric and **crashes the fuzzer on CORRECT behaviour**. This is the identical window 115-14
opened and named; it closes in 115-17 and 115-18, and the shipped rustdoc now names those two plans
as its owners.

`tests/property_tests.rs` WAS run and is safe: its generator draws containers from a three-entry
`arb_container()` that cannot draw `dependencies`, so the stale mirror is unreachable from the
generated space. It reports **20 passed**, the round-3 baseline unchanged — proving this plan did not
break it. Selected with `binary(property_tests)`, never `test(/…/)` (`D-115-Y`).

## What this plan deliberately did NOT do

- **`contracts/mcp-protocol-sdk-v1.yaml` and `contracts/binding.yaml` were NOT touched.** They also
  carry Gap 2 (the `output_schema_draft_pin` `formula:` equation head), and splitting the same
  contract prose across two plans is how a half-corrected invariant survived 115-14 and became WR-04.
  115-19 owns those files outright. Verified as safe in the interim: `tests/phase115_contract_bindings.rs`
  does not deep-check invariant prose, and no plan between here and 115-19 ships a behaviour change
  the contract would misdescribe.
- **WR-05's other half** — rewriting the two member dispatches into a common shape — is untouched;
  it is 115-19's explicitly-not-owned ledger entry.
- **`.planning/REQUIREMENTS.md` is UNTOUCHED** and `requirements mark-complete` was NOT run.
  SCHM-01's re-booking follows the whole-closure gate in 115-19. Booking ahead of measurement is
  `D-115-G` / `D-115-AG`, and this requirement has carried that defect twice already.
- **`make quality-gate` and the `+nightly` fuzz campaign** were not run — they belong to the
  whole-closure gate, following the 115-14 precedent. `make lint` DID run, exit 0.

## Deviations from Plan

### 1. [Rule 3 — blocking criterion conflict] The inline comment and the ordered-last grep criterion are mutually unsatisfiable

- **Found during:** Task 1 step (c) verification
- **Issue:** Task 1(c) requires an inline comment on the new entry naming three things; the
  acceptance criterion requires `"dependencies",` on the line immediately FOLLOWING
  `"dependentSchemas",`. A comment block between them breaks the grep.
- **Fix:** a trailing SAME-LINE comment on the entry (which satisfies both literally), with the full
  `D-115-03-C` rationale moved into the const's rustdoc — where Task 2(d) expands it anyway, and
  where it does not have to be stripped before 115-19 can compare the three copies as ordered slices.
- **Booked:** `D-115-AH(1)`, a `D-115-1` instance.

### 2. [measurement] The plan's stated pre-plan `grep -c 'dependencies'` baseline is wrong

- **Issue:** the plan states the pre-plan value is **3**; the measured value is **2**, both in the
  working tree at plan start and via `git show HEAD~1:src/server/output_validation.rs`.
- **Impact:** none — the criterion is "strictly greater", and 29 > 3 > 2.
- **Booked:** `D-115-AH(2)`.

### 3. [Rule 2 — false process rationale] There is NO pre-commit hook in this checkout

- **Found during:** Task 1, before the first commit
- **Issue:** `115-14-SUMMARY.md`'s `patterns-established` (inherited by 115-15 and by this plan's
  framing) states *"the pre-commit hook forbids committing a red tree"*, and `CLAUDE.md` describes
  the hook as MANDATORY. Measured: `.git/hooks/` contains **only `*.sample` files** and
  `core.hooksPath` points at that same directory. **Nothing mechanically blocks a red commit or
  enforces `make quality-gate`.**
- **Fix:** ran `make lint` explicitly before each commit rather than relying on a hook, and booked
  the finding. The one-commit-per-fence practice is kept — a red commit is a bisect hazard — but its
  stated justification is honour-system, not mechanism.
- **Booked:** `D-115-AH(3)`, same shape as `D-115-U`/`V`/`W`/`AB` (a gate believed to be running that
  is not), applied to the gate `CLAUDE.md` names first.

### 4. [scope, deliberate] The base commit was not the one the session snapshot showed

`fc674e40` was stale by four commits; the actual parent is `f9fad51c` (`docs(115): fix plan-checker
blocker …`). Recorded because an early baseline measurement taken against `fc674e40` returned
`dependentSchemas` = 0, which momentarily looked like output corruption (`D-115-AA`) and was in fact
just an older tree. No action needed; the commits are correctly parented.

## Threat register outcomes

| Threat ID | Disposition | Outcome |
|---|---|---|
| T-115-DEP-01 | mitigate | **Closed.** `"dependencies"` in `SUBSCHEMA_MAP_KEYWORDS`, consumed by both walkers, fenced structurally over 6×4 with the fence observed to fail on exactly the four `dependencies` pairs first |
| T-115-DEP-02 | mitigate | **Closed.** The fence's primary observable IS the borrow/own decision, which is the condition `compile_2020_12`'s warning branches on |
| T-115-DEP-03 | mitigate | **Closed.** Own six-element literal; separate membership guard; violations COLLECTED; examined-pair count asserted at 24 = 6 × 4 |
| T-115-DEP-04 | mitigate | **Closed.** `keyword_lists_are_disjoint`, with its observed negative control |
| T-115-DEP-05 | mitigate | **Closed.** The new fixture's `$id` is the reserved non-resolvable `example.test` and carries NO `$ref`; the fence never compiles the document |
| T-115-DEP-06 | accept | Residual named in the shipped rustdoc (`components.default -> rewritten=false`) rather than hidden; 115-19 re-books it |
| T-115-DEP-07 | accept | Asserted by measurement: 0 added `pub fn`/`struct`/`enum`, exactly 2 added `pub const`, both inside `pub mod fuzz_support` (hunk header recorded above) |
| T-115-SC | accept | **No `Cargo.toml` / `Cargo.lock` in the diff** — dependency set unchanged, no package-manager install, no supply-chain review triggered |

## What 115-17 / 115-18 / 115-19 inherit

- **⚠ `D-115-AH` IS TAKEN — by this plan.** `.planning/ROADMAP.md`'s 115-19 entry instructs it to
  triage the round-3 review findings into *"`D-115-AH`/`D-115-AI`"*. This plan filed its own
  deviations as `D-115-AH` (three items) because deviations must be booked by the plan that MEASURED
  them. **115-19 must continue at `D-115-AI`/`D-115-AJ`.** Writing a second `D-115-AH` would break the
  ledger's whole-ID duplicate check, which is one of 115-19's own acceptance criteria — this is
  verbatim the situation `D-115-AG(2)` records for 115-15, one round later.
- **The two restated mirrors are STALE BY CONSTRUCTION** and the window is named in the shipped
  rustdoc. `tests/property_tests.rs` (115-17) and `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`
  (115-18) both carry the five-entry list; do not run the fuzzer to judge any tree until 115-18 lands.
- **The seam is published but UNGATED.** Control 2 above measured that a stale re-export leaves the
  suite green. 115-17's compiled mirror test and 115-19's source-text drift gate are what make the
  seam load-bearing; until then the `pub const`s are an affordance, not a guarantee.
- **`contracts/` is untouched, deliberately** — 115-19 owns both files so Gap 1's keyword widening and
  Gap 2's equation-head rescoping land as ONE edit. Verified safe in the interim:
  `tests/phase115_contract_bindings.rs` does not deep-check invariant prose.
- **SCHM-01 is NOT re-booked here.** `.planning/REQUIREMENTS.md` is a 0-byte diff and
  `requirements mark-complete` was not run. 115-19 books it AFTER `make quality-gate` and the
  PR-blocking `pmat quality-gate --fail-on-violation --checks complexity` both exit 0 — and per
  `D-115-AE`, `pmat analyze complexity --max-cognitive 25` does NOT reproduce that gate.
- **There is no pre-commit hook** (`D-115-AH(3)`). Every remaining plan in this closure must run
  `make lint` / `make quality-gate` explicitly; nothing will stop a red commit.

## Self-Check: PASSED

- `src/server/output_validation.rs` — FOUND; contains `"dependencies"` (4×), `SUBSCHEMA_MAP_KEYWORDS`
  with six entries, `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`,
  `keyword_lists_are_disjoint`, `embedded_legacy_resource_in_container`, `components.default` (2×)
- `.planning/phases/115-…/deferred-items.md` — FOUND; `D-115-AH` present exactly once; the whole-ID
  duplicate check `grep -o '^## D-115-[A-Z0-9]\{1,2\}' | sort | uniq -d` returns **nothing**
- Commit `c350cb53` — FOUND in `git log`
- Commit `87d241b2` — FOUND in `git log`
- `git diff --diff-filter=D` on both commits — **no deletions**
