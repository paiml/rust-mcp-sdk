---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 17
subsystem: testing
tags: [schm-01, json-schema, draft-2020-12, gap-closure, property-testing, proptest, mirror-drift, subschema-map-keywords, dependencies, negative-controls, fuzzing-seam]

# Dependency graph
requires:
  - phase: 115-16
    provides: "the six-entry shipped `SUBSCHEMA_MAP_KEYWORDS` and its publication through the `fuzzing` seam as `pub const` re-exports — the thing this plan GATES against instead of hand-maintaining"
  - phase: 115-15
    provides: "`arb_definition_name` / `arb_container` / the rename-invariance property this plan widens and finally makes able to fire"
  - phase: 115-13
    provides: "the embedded-schema-resource generator whose pointer assertion turns out to be this module's SECOND derived fence"
provides:
  - "A SIX-entry `SUBSCHEMA_MAP_KEYWORDS` mirror in `tests/property_tests.rs`, byte-identical to the shipped list including its trailing comment and its ORDER"
  - "`keyword_lists_mirror_the_shipped_ones` — a COMPILED equality gate over both lists against the 115-16 seam re-exports, with both drift directions observed as controls"
  - "`CONTAINER_DRAW` + a six-way `arb_container()` — the generated space reaches all six spec-defined containers for the first time, so `dependencies`, `patternProperties` and `dependentSchemas` are drawable at all"
  - "A SUPERSET guard (every SHIPPED keyword must be drawable) fencing the fourth literal without disabling the negative controls"
  - "The three negative controls SCHM-01's closure needed: mirror-drift, crate-drift, and the both-blind run in which rename invariance fires alone among the restatements"
  - "`D-115-AI` — five plan/criterion defects, two of which changed what shipped: the plan's own `arb_container()` instruction would have re-shipped CR-01, and this module has TWO derived fences rather than the one its docs claim"
affects: [115-18, 115-19]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A fence's REACHABILITY must not be derived from the same artifact as the rule it checks, even when that artifact is GATED. A gate makes a copy correct; it does not make the fence able to fire. Measured: sourcing `arb_container()` from the gated mirror made all three negative controls report `21 passed`"
    - "A guard that fences a deliberate duplicate must be a SUPERSET check, not an equality check, when a negative control deliberately shortens one side — and must be asserted LAST, so it cannot mask the assertion it accompanies (D-115-AF, applied twice in one function)"
    - "Assertion ORDER inside a `proptest!` body is evidence: a failure reported at assertion N is positive proof that assertions 1..N-1 passed for that case. That is how the both-blind criterion was discharged when an all-pass turned out to be impossible"
    - "A grep-shaped criterion over a file also constrains what that file may say ABOUT the criterion — quoting the criterion inside the amendment re-matched it"
    - "Restore a temporarily-mutated shared file from a `/bin/cp` snapshot verified with `shasum -a 256 -c`, never `git checkout --`, `git stash` or `git clean`"

key-files:
  created:
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-17-SUMMARY.md
  modified:
    - tests/property_tests.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "`arb_container()` draws from `CONTAINER_DRAW`, its OWN six-element literal — NOT from `SUBSCHEMA_MAP_KEYWORDS`, which is what the plan instructed. The instruction was implemented literally first and MEASURED to make every negative control go green, because shortening the mirror shrinks the generated space in the same edit. This is 115-16's own-literal pattern applied one layer up"
  - "The fourth literal's drift risk is fenced by a SUPERSET guard (every SHIPPED keyword must be drawable), asserted LAST inside `keyword_lists_mirror_the_shipped_ones`. Superset rather than equality so the both-blind control — in which the shipped list is deliberately SHORTER — is not disturbed"
  - "The both-blind criterion 'the surgical test must PASS' is unsatisfiable and was discharged by its INTENT instead: the failure is reported at the embedded-resource POINTER assertion, which proves surgical scope and dialect purity passed earlier in the same body, and the mirror gate PASSING is the independent proof that no mirror was missed"
  - "Task 1's two container-dependent controls were run in the Task 2 tree, where they are reachable at all; the tree each was measured in is stated rather than glossed"
  - "The WR-06 correction PARAPHRASES the falsified sentence rather than quoting it, because the plan's grep criterion requires the literal gone. The verbatim text lives in `115-REVIEW.md` WR-06 and in this SUMMARY"
  - "`src/server/output_validation.rs` is a 0-byte diff. `.planning/REQUIREMENTS.md` UNTOUCHED and `requirements mark-complete` NOT run — SCHM-01's re-booking follows 115-19's whole-closure gate (`D-115-G` / `D-115-AG`)"
  - "No fuzz target built or run: `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` still carries the five-entry list until 115-18 lands, so a `dependencies`-shaped input crashes it on CORRECT behaviour"

patterns-established:
  - "Run a negative control with `--no-fail-fast`. nextest's default cancels the run at the first failure, so a control specified as 'expect exactly 2 failures' silently reports 1 and the second is never attempted"
  - "When a control comes back GREEN, that is a result about the INSTRUMENT, not a pass. Both green controls in this plan were defects in the harness that the plan text had asked for"

# Metrics
duration: ~70m
completed: 2026-08-02
tasks_completed: 2
files_modified: 2
---

# Phase 115 Plan 17: The Property Mirror, Gated and Finally Able to Fire Summary

Round 4 on SCHM-01, first of two mirror plans. `tests/property_tests.rs` carried a five-entry
restatement of a rule `src/` had widened to six, with nothing checking the restatement, and an
`arb_container()` that drew three of six — so the one fence the phase advertises as immune to a rule
defect **could not reach the position the defect lived at**. Both are closed: the mirror is
byte-identical to the shipped list and a COMPILED test fails when it is not, the generated space
reaches all six spec-defined containers with a measured floor, and the rename-invariance property was
OBSERVED to fail on a `dependencies` entry in the configuration where every restatement in the module
passes.

**The plan's own instruction for how to widen the draw would have re-shipped CR-01.** That is the
finding of this plan and it was caught only because the negative controls were run rather than
assumed.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Six-entry mirror, the compiled mirror-equality gate, corrected mirror rustdocs | `0268fa34` | `tests/property_tests.rs` |
| 2 | Six-way container draw from an own literal, coverage proof, three negative controls | `9fc3534c` | `tests/property_tests.rs` |
| — | Deviations booked as `D-115-AI` | *(this commit)* | `deferred-items.md` |

## Measured results against every criterion

| Command | Expected | Observed |
|---|---|---|
| `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)' --test-threads=1` | 21 passed | **21 tests run: 21 passed** ✓ |
| the same selector under `--features full` | 18, unchanged | **18 tests run: 18 passed** ✓ |
| `cargo fmt --all -- --check` | exit 0 | **exit 0** ✓ |
| `make lint` (pedantic + nursery, run before EACH commit) | exit 0 | **exit 0**, twice ✓ |
| `cargo clippy --features "full fuzzing" --test property_tests` | no warnings | **no warnings** ✓ |
| `grep -c '"dependencies"' tests/property_tests.rs` | ≥ 1 | **1** ✓ |
| `"dependencies",` on the line after `"dependentSchemas",` | yes | line 1055 → 1056 ✓ |
| `grep -n 'remove it from only one side'` | nothing | **nothing** ✓ |
| `grep -n 'Just("$defs"), Just("definitions"), Just("properties")'` | nothing | **nothing** ✓ |
| `grep -n 'println!'` inside the module | nothing | **nothing** ✓ |
| `git status --short src/` | empty | **empty** ✓ |
| `git diff --exit-code src/server/output_validation.rs` | 0 | **0** ✓ |
| `shasum -a 256 -c` on the pre-control snapshot | `OK` | **OK** (`a97f5cb2…3192c`) ✓ |
| ledger whole-ID duplicate check | nothing | **nothing**; `D-115-AI` present once ✓ |

The 21/18 pair is the proof the `fuzzing`-gated module actually RAN rather than being silently gated
out. Selected with `binary(property_tests)`, never `test(/…/)` (`D-115-Y`); every binary spelled
absolutely (`D-115-AA`).

## THE THREE NEGATIVE CONTROLS — verbatim, with the failing test named in each

All three were run against the **final** tree (both tasks landed). Task 1's two are
container-dependent and are structurally unreachable in the Task 1 tree; see `D-115-AI(2)` and the
Deviations section.

Every control run used `--no-fail-fast`. **This is not incidental**: nextest's default cancels the
run at the first failure, and the first control was initially reported as "1 failure" purely because
the remaining eleven tests were never attempted.

### Control A — mirror stale, `src/` correct → **2 failures**, as specified

`"dependencies"` removed from `tests/property_tests.rs`'s local `SUBSCHEMA_MAP_KEYWORDS` only.

**Failure 1 — `keyword_lists_mirror_the_shipped_ones`** (the gate doing its job):

```
assertion `left == right` failed: this module's SUBSCHEMA_MAP_KEYWORDS mirror has DRIFTED
from the shipped list. Compared as ORDERED slices, deliberately: 115-19's source-text drift
gate compares the three copies the same way, and `dependencies` is ordered LAST in `src/`
for that reason. If the CRATE gained an entry and this copy did not, the restated
collect_dialect_declarations below now holds the OLD rule against the NEW behaviour and
reports a name-bound $schema STRING as a surviving legacy declaration — a FALSE POSITIVE
against a correct normalizer, which is what
property_schema_normalization_is_idempotent_and_surgical will fail on next. …
  left: ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas"]
 right: ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas",
         "dependencies"]
```

**Failure 2 — `property_schema_normalization_is_idempotent_and_surgical`, on the SURGICAL-SCOPE
assertion**, for a drawn `dependencies` container:

```
normalization touched a key other than a string-valued $schema:
{"const":null,"dependencies":{"const":{"$id":"https://example.test/inner",
 "$schema":"aa://aa/aa","type":"integer"}},"properties":{"n":{"$ref":"#/dependencies/const"}}}
became
{"const":null,"dependencies":{"const":{"$id":"https://example.test/inner",
 "$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}}, …}
```

This second failure is WR-01's *"crate gains an entry, mirrors do not"* mode made loud: `src/`
correctly rewrites inside `dependencies`, the stale mirror's blind strip skips that subtree because
the entry is named `const` (a `DATA_ONLY_KEYWORDS` collision), and the two stripped clones therefore
differ. `property_normalization_does_not_depend_on_a_subschema_map_key_name` **PASSED** — correctly,
`src/` is not defective in this configuration.

### Control B — `src/` stale, mirror correct → dialect purity REACHES the position

`"dependencies"` removed from `src/server/output_validation.rs` only. **3 failures.**

The one the plan specified — `property_schema_normalization_is_idempotent_and_surgical`, on the
**DIALECT-PURITY** assertion, with a `/dependencies/…` path in the reported document:

```
a LEGACY $schema survived normalization: ["aa://aa/aa"] in
{"const":null,"dependencies":{"const":{"$id":"https://example.test/inner",
 "$schema":"aa://aa/aa","type":"integer"}},"properties":{"n":{"$ref":"#/dependencies/const"}}}.
A declaration that survives on an $id-bearing embedded schema resource resolves an EMPTY
vocabulary set there and produces a sub-validator that accepts everything …
```

That proves the corrected mirror's scan genuinely reaches a position the crate's walk skipped — the
capability that would be **destroyed** by deriving this copy from the seam instead of gating it.

The other two are expected and worth naming: `keyword_lists_mirror_the_shipped_ones` fires (the gate
is symmetric — it catches drift in *either* direction, and here the mirror is the one ahead of the
crate), and `property_normalization_does_not_depend_on_a_subschema_map_key_name` fires too, because
`src/` really is defective in this configuration.

### Control C — BOTH copies blind → **rename invariance, at `dependencies`**

`"dependencies"` removed from `src/` AND from the mirror, restoring the pre-115-16 world in which
both copies share the omission. `CONTAINER_DRAW` is untouched, so the generated space still reaches
the position — **that is the whole point, and it is what the plan's original `arb_container()`
instruction would have prevented.**

**`keyword_lists_mirror_the_shipped_ones` PASSED.** This is the positive, independent proof that the
control is genuinely both-blind and no mirror was missed — the check `D-115-AF` asks for, discharged
by an instrument rather than by inspection.

**`property_normalization_does_not_depend_on_a_subschema_map_key_name` FAILED**, the decisive
observation:

```
RENAME INVARIANCE VIOLATED at /dependencies/const vs /dependencies/__rename_probe__.
The keys of properties / patternProperties / $defs / definitions / dependentSchemas /
dependencies are AUTHOR-CHOSEN NAMES with no keyword semantics under the JSON Schema
2020-12 core and applicator vocabularies, so normalizing an entry CANNOT depend on the name
it is filed under. … Normalized under the drawn name:
{"const":null,"dependencies":{"const":{"$id":"https://example.test/inner",
 "$schema":"aa://aa/aa","type":"integer"}}, …}. Normalized under the probe:
{"const":null,"dependencies":{"__rename_probe__":{"$id":"https://example.test/inner",
 "$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}}, …}.
```

**Shrunk counterexample: container `dependencies`, name `const`** — one of the four colliding
literals, exactly as the criterion requires. Not `$defs`, not `properties`; those already worked and
would have proved nothing about this round's defect. No re-seeding or strategy narrowing was needed:
the widened draw reaches the shape on the first run, which the coverage proof below quantifies.

**And the surgical-scope and dialect-purity assertions PASSED in this configuration** — confirmed
explicitly, because it is the criterion's substance. `property_schema_normalization_is_idempotent_and_surgical`
did fail, but at the **embedded-resource POINTER** assertion, four assertions later in the same body:

```
an embedded schema resource's dialect declaration must be rewritten to the 2020-12 URI
at /dependencies/const/$schema: {"const":null,"dependencies":{"const":{…}}, …}
```

Assertions in a `proptest!` body run top to bottom, so a failure reported there is **positive
evidence** that idempotence, surgical scope, the root check and dialect purity all passed for that
case. Verified against the source line map: surgical-scope `prop_assert_eq!` at `:1632`,
dialect-purity `prop_assert!` at `:1673`, embedded-pointer `prop_assert_eq!` at `:1701`, rename at
`:1795`. The failure was reported at the third of those. See `D-115-AI(5)` — the plan's criterion
asked for the whole test to pass, which is impossible, because this module has **two** fences of the
derived kind and its own documentation names one.

## THE COVERAGE PROOF — measured on the SHIPPED generator

Temporary `println!` in `arb_embedded_schema_document()`'s `prop_map` (the strategy that ALWAYS
embeds, so the rate is not halved by an `embed` bool), read back through `--no-capture`:

| Quantity | Floor | Observed |
|---|---|---|
| `dependencies` + a colliding name + a non-2020-12 embedded dialect | **≥ 8** | **21** |
| distinct containers drawn | **6** | **6** |
| distinct (container, name) combinations | recorded | **70** |
| total draws over the configured 256 cases | — | 260 |

Per-container draw counts: `$defs` 52, `dependencies` 47, `properties` 45, `definitions` 39,
`dependentSchemas` 39, `patternProperties` 38. The `dependencies` × colliding-name × legacy-dialect
breakdown from the first (pre-fix-4) measurement was `const` 5, `default` 4, `enum` 3, `examples` 2.

21 of 260 sits almost exactly on the plan's predicted *"roughly 20 of 256"* — one sixth of 115-15's
58, once the container draw widened from three to six.

**The instrumentation is removed**: `grep -c 'TEMP_COVERAGE_PROOF' tests/property_tests.rs` → **0**,
and `println!` appears nowhere in the `schema_dialect_normalization_properties` module. The file was
restored from a `shasum -a 256 -c`-verified snapshot rather than by hand-editing the instrumentation
back out.

## SEP-2106, by inspection and recorded

- `grep -n 'https://example.test'` still shows the `$id` value — the reserved, NON-RESOLVABLE host,
   **1** occurrence, unchanged by this plan.
- The module's ONLY `$ref` construction is `embed_resource`'s
  `serde_json::json!({ "$ref": format!("#/{container}/{name}") })` — a LOCAL JSON pointer. Confirmed
  by reading `embed_resource` and by grep: **no** generated `$ref` carries an `http`, `https` or
  `file` scheme, at any draw.
- Widening the container draw changes only WHICH of six string keys the resource is filed under. It
  introduces no new URI construction site, and none of the six containers or the names
  `arb_definition_name` can draw contains `/` or `~`, so the pointers need no RFC 6901 escaping.

## Deviations from Plan

All five are booked as **`D-115-AI`** in `deferred-items.md`, continuing after `D-115-AH` per
`115-16-SUMMARY.md`'s instruction. **`115-19` must continue at `D-115-AJ`.**

### 1. [Rule 1 — the plan instruction would have re-shipped CR-01] `arb_container()` must NOT be sourced from `SUBSCHEMA_MAP_KEYWORDS`

- **Found during:** Task 2, running Control A — which came back GREEN.
- **Issue:** Task 2(a) instructs *"Build it from that constant rather than from a fourth literal — the
  constant is now gated … so a fourth hand-written copy would add drift surface for no gain."* The
  drift reasoning is correct; the reachability consequence is fatal. With the draw sourced from the
  mirror, removing an entry from the mirror removes that container from the **generated space in the
  same edit**:

  | Control | draw from the mirror | draw from an OWN literal |
  |---|---|---|
  | mirror stale, `src/` correct | **21 passed** (only the gate fires) | **2 failed** (gate + surgical scope) |
  | BOTH blind | **21 passed** — every fence green | **2 failed** (rename + embedded pointer) |

  That is `115-REVIEW.md` CR-01 verbatim — *"gated by a crate-derived list one line earlier"* — being
  recreated by the plan written to close it, in round 4 on the same requirement.
- **Fix:** `CONTAINER_DRAW`, a six-element literal owned by the module, exactly the shape 115-16 chose
  for `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map` on the `src/` side.
  The drift risk is fenced SEPARATELY by a **superset** guard in
  `keyword_lists_mirror_the_shipped_ones` — every keyword `src/` ships must be drawable — asserted
  LAST so it can mask neither the mirror comparison nor the both-blind control. Superset rather than
  equality precisely so the both-blind configuration (shipped list at five, `CONTAINER_DRAW` at six)
  leaves the generated space intact.
- **Files:** `tests/property_tests.rs`. **Commit:** `9fc3534c`. **Booked:** `D-115-AI(4)`.
- **The standing lesson, and it is the sharpest form this phase has produced:** *a fence's
  REACHABILITY must not be derived from the same artifact as the rule it checks, even when that
  artifact is gated.* A gate makes a copy CORRECT; it does not make the fence able to FIRE.

### 2. [Rule 2 — falsified documentation] This module has TWO derived fences; its docs claim one

- **Found during:** Task 2, Control C.
- **Issue:** The module rustdoc, the rename property's rustdoc and the plan's
  `<which_fence_catches_what_here>` block all state that rename invariance is *the one* fence here a
  rule defect cannot satisfy. The both-blind control falsified that: the **embedded-resource pointer
  assertion** addresses the pointer the generator drew, consults no keyword list, and fired at
  `/dependencies/const/$schema` while every restatement passed.
- **Fix:** amended both rustdocs to state the two-fence reality, with the observed message quoted, and
  added an inline note at the assertion itself. The plan's acceptance criterion (*"the surgical test
  MUST pass"*) is unsatisfiable; its INTENT was discharged exactly, and more strongly than an all-pass
  would have been.
- **Files:** `tests/property_tests.rs`. **Commit:** `9fc3534c`. **Booked:** `D-115-AI(5)`.

### 3. [Rule 3 — criterion collision] The WR-06 grep criterion and amend-don't-delete are mutually unsatisfiable

- **Issue:** Task 1(c) says *"Amend rather than delete, per the phase's convention"*; the criterion
  requires the falsified sentence's literal text to be ABSENT. An amendment that quotes it fails the
  grep. **Second-order instance:** the first correction quoted the criterion command itself, which of
  course still matched — a grep criterion over a file also constrains what that file may say ABOUT the
  criterion.
- **Fix:** paraphrase in the rustdoc, verbatim text left in `115-REVIEW.md` WR-06 and this SUMMARY,
  with the collision named inline so a reader does not "restore" the literal.
- **Booked:** `D-115-AI(1)`, a `D-115-1` instance — identical in shape to `D-115-AH(1)`.

### 4. [sequencing] Task 1's two negative controls are unreachable in Task 1's tree

- **Issue:** both require a `dependencies` container in the generated space, which arrives with Task
  2(a). In the Task 1 tree the mirror-drift control produced **1** failure (the gate alone, with the
  surgical test PASSING, verified via `--no-fail-fast`), and the crate-drift control could not fire.
- **Fix:** both were run to completion in the Task 2 tree and are recorded above with the tree stated.
  No coverage lost; the plan ordered the observations one task early.
- **Booked:** `D-115-AI(2)`.

### 5. [measurement] The false-positive mode did NOT silently ship between 115-16 and this plan

- **Issue:** Task 1's criterion calls the surgical-scope failure *"what silently shipped between
  115-16 and this plan"*. Measured: in that window `arb_container()` drew three of five, so no
  generated document reached the `dependencies` position and the stale mirror produced **no** false
  positive. The drift WINDOW was real; the firing was not reachable.
- **Impact:** none on the code — the mode is real in general and was observed by constructing the
  configuration that reaches it. Recorded because the claim would otherwise propagate.
- **Booked:** `D-115-AI(3)`.

## What this plan deliberately did NOT do

- **No fuzz target was built or run.** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` still carries the
  five-entry list until 115-18 lands, so a `dependencies`-shaped input crashes it on CORRECT
  behaviour. Running it here would have proved nothing about this file while risking an hour of
  misattribution.
- **`disambiguate()` is untouched** — `115-REVIEW.md` IN-03 is explicitly not this plan's, and is
  115-19's to book.
- **`src/server/output_validation.rs` is a 0-byte diff.** It appears in `files_modified` solely
  because Controls B and C mutate it; both were restored from a `shasum -a 256 -c`-verified snapshot.
  `115-18` inherits it clean.
- **`.planning/REQUIREMENTS.md` UNTOUCHED and `requirements mark-complete` NOT run.** SCHM-01's
  re-booking follows 115-19's whole-closure gate. Booking ahead of measurement is `D-115-G` /
  `D-115-AG`, and this requirement has carried that defect twice.
- **`make quality-gate` was not run** — it belongs to the whole-closure gate in 115-19, following the
  115-14/115-15/115-16 precedent. `make lint` DID run before each commit, exit 0, because
  **there is no pre-commit hook in this checkout** (`D-115-AH(3)`).

## Threat register outcomes

| Threat ID | Disposition | Outcome |
|---|---|---|
| T-115-DEP-08 | mitigate | **Closed, and it needed the deviation to close.** `arb_container()` draws all six with a measured floor (21 of 260, ≥ 8 required; 6 of 6 containers), and rename invariance was OBSERVED to fail on `/dependencies/const` in the both-blind configuration. Sourcing the draw from the gated mirror would have left the threat OPEN while every control read green |
| T-115-DEP-09 | mitigate | **Closed.** `keyword_lists_mirror_the_shipped_ones` compares ORDERED slices against the 115-16 seam re-exports; both drift directions observed (Controls A and B). The lockstep-removal mode is named in the message and attributed to the `src/`-side fence that carries its own literal |
| T-115-DEP-10 | mitigate | **Closed.** The false-positive risk is attributed to the SCAN only, in WR-06's terms, amend-not-delete (paraphrased — see `D-115-AI(1)`). The strip's real value — sensitivity against a normalizer over-reaching into NAME position — is stated as a reading of the two walks and explicitly labelled unmeasured |
| T-115-DEP-11 | accept | Confirmed cheap: the widening changes only which of six string keys the resource is filed under. Suite runtime 1.19s → 1.26s across the plan; sample size stays the configured 256 |
| T-115-DEP-12 | mitigate | **Closed.** Every `$id` is the reserved `example.test`; the module's only `$ref` construction is a local `#/{container}/{name}` pointer; asserted by inspection and by grep, recorded above |
| T-115-SC | accept | No `Cargo.toml` / `Cargo.lock` in the diff — no package-manager install, no manifest edit, no supply-chain review triggered |

## What 115-18 and 115-19 inherit

- **⚠ `D-115-AI` IS TAKEN — by this plan, five items. `115-19` must continue at `D-115-AJ`.** Writing
  a duplicate breaks the whole-ID check that is one of 115-19's own acceptance criteria — verbatim the
  situation `D-115-AG(2)` and `D-115-AH` each record one round earlier.
- **⚠ `D-115-AI(4)` APPLIES DIRECTLY TO 115-18.** If the fuzz target's container selection is derived
  from its own restated `SUBSCHEMA_MAP_KEYWORDS`, its invariant-6 negative controls will go green for
  the same structural reason and prove nothing. The fuzz target must carry its own container literal,
  or select containers independently of the list under test.
- **⚠ The `<which_fence_catches_what_here>` taxonomy is copied into both remaining plans with the
  one-derived-fence framing.** `D-115-AI(5)` measured it to be wrong. Read it before trusting it.
- **`src/server/output_validation.rs` is clean** — `git diff --exit-code` 0, `shasum` OK against the
  pre-control snapshot `a97f5cb2…3192c`, `git status --short src/` empty. Nothing to unwind.
- **Run negative controls with `--no-fail-fast`.** A control specified as "expect N failures" is
  silently truncated to 1 otherwise.
- **The mirror gate is now load-bearing** — 115-16 measured that publishing the seam alone left the
  suite green at 25. This plan is the half that makes the seam a guarantee for `tests/`; 115-18 does
  it for `fuzz/`, and 115-19's source-text drift gate is the only instrument that would catch a
  re-export hand-written to match a stale mirror.

## Self-Check: PASSED

- `tests/property_tests.rs` — FOUND; contains `keyword_lists_mirror_the_shipped_ones` (1),
  `fuzz_support::SUBSCHEMA_MAP_KEYWORDS` (2), `arb_container` (1), `"dependencies"` (1, ordered last
  at line 1056 immediately after `"dependentSchemas",`), `CONTAINER_DRAW`; contains no `println!` and
  no `TEMP_COVERAGE_PROOF`
- `.planning/phases/115-…/deferred-items.md` — FOUND; `D-115-AI` present exactly once; the whole-ID
  duplicate check `grep -o '^## D-115-[A-Z0-9]\{1,2\}' | sort | uniq -d` returns **nothing**
- Commit `0268fa34` — FOUND in `git log`
- Commit `9fc3534c` — FOUND in `git log`
- `git diff --diff-filter=D` across both commits — **no deletions**
- `git diff --exit-code src/server/output_validation.rs` — **exit 0**; `shasum -a 256 -c` — **OK**
