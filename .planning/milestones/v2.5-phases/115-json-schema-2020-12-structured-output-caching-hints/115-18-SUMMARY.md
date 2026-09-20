---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 18
subsystem: testing
tags: [schm-01, json-schema, draft-2020-12, gap-closure, fuzzing, libfuzzer, mirror-drift, subschema-map-keywords, dependencies, negative-controls, corpus-seed, false-positive]

# Dependency graph
requires:
  - phase: 115-16
    provides: "the six-entry shipped `SUBSCHEMA_MAP_KEYWORDS` this file's mirror is brought onto, and the `src`-side own-container-literal fence that covers this target's measured blind spot"
  - phase: 115-15
    provides: "invariant 6 widened to every entry of every root-level subschema map — the reach this plan's Control E depends on"
  - phase: 115-13
    provides: "invariant 5 (post-normalization dialect purity) and the seed-corpus conventions"
provides:
  - "A SIX-entry `SUBSCHEMA_MAP_KEYWORDS` in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, ordered identically to `src/` and `tests/`, closing a LIVE false-positive window that crashed the fuzzer on correct behaviour"
  - "Seed `15_dependencies_named_default` — CR-01's reproduction document as a permanent committed input, bringing the tracked corpus to 15"
  - "Three negative controls in three deliberately different configurations: invariant 5 isolated, invariant 6 isolated, and the both-blind configuration in which this target detects NOTHING"
  - "The target's detection LIMIT measured at exit 0 and written in three places, with the covering mechanism (`src`'s own-literal fence) MEASURED failing in the same tree rather than merely named"
  - "`D-115-AJ` — five items, including the checked-not-inherited verdict on the `<which_fence_catches_what_here>` taxonomy that `D-115-AI(5)` falsified for the sibling module"
affects: [115-19]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A mirror going stale is not a cosmetic defect: a position-blind SCAN against a position-aware shipped walk is a FALSE-POSITIVE GENERATOR that crashes the fuzzer on correct code. Measured before and after, on the same input"
    - "When a control fires, name the FRAME, not just the exit code. All four firings here are attributed to a specific function and line"
    - "To prove a WEAKER fence reaches a shape, silence the STRONGER one and re-run — otherwise you have measured only that the stronger one works (D-115-AF)"
    - "The mechanism you name as covering a blind spot must be MEASURED failing in the blind configuration, or the attribution is just a claim. `src`'s fence was run and observed to fail at `output_validation.rs:1429`"
    - "Restore a temporarily-mutated shared file from a `/bin/cp` snapshot verified with `shasum -a 256 -c`, then REBUILD, so no control's binary reaches the evidence run"

key-files:
  created:
    - fuzz/corpus/fuzz_schema_draft_pin/15_dependencies_named_default
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-18-SUMMARY.md
  modified:
    - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
    - fuzz/corpus/fuzz_schema_draft_pin/README.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "The mirror stays an INDEPENDENT literal and is NOT imported from 115-16's `fuzzing` seam re-export. Deriving it would make the scan skip exactly what the walk skips, so the target would exit 0 on the very document that reproduces the defect — blind to every keyword-list omission by construction. Control D is the measurement of the capability that independence buys"
  - "The rustdoc's measured control numbers were written in TASK 2, not Task 1, because they do not exist until Task 2 has run. Writing them earlier would be the booking-ahead-of-measurement defect (D-115-G / D-115-AG)"
  - "The README's historical WR-07 measurement (3382 vs a tracked count of 14) was NOT rewritten to 15. The live count is stated separately and explicitly; falsifying a measurement to satisfy a grep criterion is the defect class this round exists to eliminate"
  - "`src/server/output_validation.rs` is a 0-byte diff, restored and verified twice. `.planning/REQUIREMENTS.md` UNTOUCHED and `requirements mark-complete` NOT run — SCHM-01's re-booking follows 115-19's whole-closure gate"
  - "`make quality-gate` was not run — it belongs to 115-19's whole-closure gate, per the 115-14/15/16/17 precedent. `make lint` DID run before each commit (exit 0, twice), because there is no pre-commit hook in this checkout"

patterns-established:
  - "A negative control that must be observed BEFORE a fix is a scheduling constraint on the task, not a nice-to-have: the stale state is destroyed by the edit that closes it"
  - "`git check-ignore -v` on a new corpus seed proves the `.gitignore` re-include matched, BEFORE relying on `git status` to have shown it"

# Metrics
duration: ~75m
completed: 2026-08-02
tasks_completed: 2
files_modified: 3
---

# Phase 115 Plan 18: The Fuzz Mirror, Widened Under a Live Defect Summary

Round 4 on SCHM-01, second of two mirror plans. `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`
restated a traversal rule `src/` had widened to six keywords in 115-16 and was still at five. That was
not latent debt — it was **an active false-positive generator against correct code**, and it was
observed firing before it was closed. The mirror is now six, CR-01's reproduction document is a
committed seed, and the two invariants that fence this position were each OBSERVED to trip on it in a
configuration that isolates them from each other.

**The most useful result is the one that says this target is blind.** With both keyword lists sharing
an omission, every invariant in the file passes and the seed that reproduces CR-01 is waved straight
through at exit 0. That is now measured, written in three places, and attributed to the mechanism that
actually covers it — which was itself run and observed to fail in the same tree, rather than merely
named.

## What shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Six-entry mirror, the two retracted TOTAL claims, the two-list distinction, invariant 6's reach limit | `768460f9` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` |
| 2 | Seed `15_dependencies_named_default`, three controls in three configurations, replay + campaign | `5e9c1474` | seed, corpus `README.md`, fuzz target |
| — | Deviations booked as `D-115-AJ` | *(docs commit)* | `deferred-items.md` |

## Task 1's false-positive control — the window that was OPEN when this plan started

115-16 landed the sixth keyword in `src/`; until Task 1 landed, this copy was stale. That drift state
has a directly observable symptom, and it was observed on a scratch input built with the README's
`python3` recipe, selector byte 1, written **outside** the corpus (no `[0-9][0-9]_` prefix, and outside
the repo entirely, so `D-115-Z` is satisfied trivially):

```
schema  : {"dependencies":{"$schema":"http://json-schema.org/draft-07/schema#"}}
instance: {}
```

A NAME bound to a NON-schema — which the shipped six-keyword walk correctly leaves alone.

**Run 1, mirror at five — non-zero exit, INVARIANT 5, verbatim:**

```
thread '<unnamed>' panicked at fuzz_targets/fuzz_schema_draft_pin.rs:518:5:
A LEGACY $schema SURVIVED NORMALIZATION: ["http://json-schema.org/draft-07/schema#"]. …
Input was: {"dependencies":{"$schema":"http://json-schema.org/draft-07/schema#"}},
normalized to: {"dependencies":{"$schema":"http://json-schema.org/draft-07/schema#"}}
```

`#14 assert_no_legacy_dialect_survives fuzz_schema_draft_pin.rs:518`, call site `:727`,
`Error: Fuzz target exited with exit status: 77`.

**`normalized to:` is byte-identical to `Input was:`** — the shipped normalizer touched nothing and
the position-blind scan invented the finding. This is a false positive against CORRECT behaviour.

**Run 2, after the widening — exit 0.** Same input, rebuilt target, `Executed … in 9 ms`.

**Which invariant fired is part of the criterion** (`D-115-AF`), and it was invariant 5, not invariant
2. Invariant 2 runs FIRST in the `fuzz_target!` body and PASSED. That is `115-REVIEW.md` **WR-06**
confirmed by measurement: the strip is applied to BOTH sides of the surgical-scope comparison, so on
an input the shipped walk leaves unchanged, any deterministic strip keeps the two clones equal and the
assertion cannot fire. Only the SCAN half can false-positive. The mirror rustdoc previously
over-generalised this to cover the stripper; that claim is now corrected in this file too (115-17 fixed
the `tests/` copy).

**The scratch input was deleted** (`/bin/rm`, absence confirmed) and the artifacts directory was empty
before and after.

## The seed

`fuzz/corpus/fuzz_schema_draft_pin/15_dependencies_named_default` — **230 bytes**, selector **1** (JSON
family), `schema_len` **203**. Decoded by re-splitting at `HEADER_LEN + u32::from_le_bytes(...)`:

```
schema  : {"type":"object","properties":{"n":{"$ref":"#/dependencies/default"}},
           "dependencies":{"default":{"$id":"https://example.test/inner",
           "$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}
instance: {"n":"NOT-AN-INTEGER"}
```

This is seed `14_defs_named_default` with `$defs` replaced by `dependencies` — i.e. `115-REVIEW.md`
CR-01's measured row verbatim. No root `$schema`.

**SEP-2106, confirmed:** the `$id` host is the reserved, non-resolvable `example.test`, and the only
`$ref` is a LOCAL JSON pointer `#/dependencies/default`. No outbound fetch is constructible from this
seed, and the structural fence is unchanged anyway (`jsonschema` is `default-features = false`
everywhere, asserted by `binary(v2_schema_tripwires)`).

`git check-ignore -v` confirmed the seed matches `fuzz/.gitignore:22`'s re-include
`!corpus/fuzz_schema_draft_pin/[0-9][0-9]_*` BEFORE relying on `git status` to show it.

## THE THREE CONTROLS — three configurations, three questions, verbatim

Each was run on the SINGLE file `corpus/fuzz_schema_draft_pin/15_dependencies_named_default` so
attribution is unambiguous, with a rebuild between configurations.

### Control D — does the corrected mirror detect a CRATE-list omission? → INVARIANT 5

`"dependencies"` removed from `src/server/output_validation.rs` only; this file keeps its six.

```
thread '<unnamed>' panicked at fuzz_targets/fuzz_schema_draft_pin.rs:642:5:
A LEGACY $schema SURVIVED NORMALIZATION: ["http://json-schema.org/draft-07/schema#"]. …
Input was: {"type":"object","properties":{"n":{"$ref":"#/dependencies/default"}},
  "dependencies":{"default":{"$id":"https://example.test/inner",
  "$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}},
normalized to: {"type":"object","properties":{"n":{"$ref":"#/dependencies/default"}},
  "dependencies":{"default":{"$id":"https://example.test/inner",
  "$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}
```

`#14 assert_no_legacy_dialect_survives fuzz_schema_draft_pin.rs:642`;
`Error: Fuzz target exited with exit status: 77`; artifacts dir empty.

`normalized to:` identical to `Input was:` — the five-entry crate walk fell to the ordinary arm at
`dependencies`, then hit the entry named `default`, matched `DATA_ONLY_KEYWORDS` in NAME position and
skipped the embedded resource. The corrected mirror's scan reached the position the walk skipped.
**That is the detection capability deriving this copy from the seam would destroy.**

Invariant 2 passed, and invariant 3 was not reached — as predicted; if either had fired the
configuration would have been wrong.

### Control E — does the DERIVED fence REACH the shape, or is it merely masked? → INVARIANT 6

Same `src` revert, **plus** `assert_no_legacy_dialect_survives(schema_bytes);` commented out in the
`fuzz_target!` body so the stronger fence cannot fire first.

```
thread '<unnamed>' panicked at fuzz_targets/fuzz_schema_draft_pin.rs:796:5:
assertion `left == right` failed: RENAME INVARIANCE VIOLATED. … container: dependencies,
name: default, normalized under the original name:
  {"dependencies":{"default":{"$id":"https://example.test/inner",
   "$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}},
under the probe:
  {"dependencies":{"__rename_probe__":{"$id":"https://example.test/inner",
   "$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}}}
```

`#16 assert_entry_normalizes_the_same_under_any_name fuzz_schema_draft_pin.rs:796`,
`#17 assert_normalization_is_invariant_under_rename fuzz_schema_draft_pin.rs:754`;
exit status 77.

The two probe documents are the whole finding: filed under `default` the declaration stays draft-07;
filed under `__rename_probe__` it becomes 2020-12. Normalization is name-dependent — the traversal is
treating an AUTHOR-CHOSEN NAME as a KEYWORD.

**This control is why the plan required it.** Control D alone proves only that invariant 5 works and
says nothing about whether invariant 6 ever ran, which is precisely the `D-115-AF` failure. An exit 0
here would have meant the derived fence does not reach the shape — a finding to fix, not a control to
skip. It fired.

**No `D-115-AI(4)` trap here, checked deliberately.** Controls D and E mutate `src/`, NOT this file's
list, so the container selection that gives invariant 6 its reach is untouched while the rule under
test is broken. That is the structural difference from 115-17's re-shipped-CR-01 instruction, where
the container DRAW was sourced from the very mirror being shortened.

### Control F — the LIMIT, measured → EXIT 0, nothing fired

`"dependencies"` removed from BOTH `src/` and this file (the pre-115-16 world), invariant 5's call
restored.

```
Running: corpus/fuzz_schema_draft_pin/15_dependencies_named_default
Executed corpus/fuzz_schema_draft_pin/15_dependencies_named_default in 9 ms
```

**Exit 0.** 1 input, no invariant reached. Every fence goes quiet in a way worth spelling out: the
strip is symmetric so invariant 2 passes; the scan skips exactly what the walk skipped so invariant 5
passes **vacuously**; invariant 6 never selects the container because its selection reads this file's
list; and invariant 3 skips the document because `dependencies` is not dialect-neutral.

**This is the target's detection limit for a SHARED list omission, and a green fuzz run is therefore
NOT evidence that a keyword-list omission is absent.** It is recorded in three places a reader might
arrive from: `SUBSCHEMA_MAP_KEYWORDS`' rustdoc, invariant 6's rustdoc, and the seed's README row.

**The covering mechanism was MEASURED, not merely named.** In the same both-blind tree, before
restoring, `src`'s own-container-literal fence was run:

```
cargo test --lib --features validation \
  v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map
→ test result: FAILED. 0 passed; 1 failed
  panicked at src/server/output_validation.rs:1429
```

So while this entire fuzz target stayed green, the `src`-side fence that carries its OWN container
literal failed. That is the attribution discharged by an instrument instead of by assertion. The
second mechanism, 115-19's source-text drift gate, is what would catch the two lists being shortened
together in the first place.

Its panic text also re-states why the fence is structural rather than behavioural — on `jsonschema`
0.49.2 both `dependencies.Inner` and `dependencies.default` report `(Violates, Violates)`, so a verdict
assertion would pass against the defective code. That matches this round's standing constraint exactly.

## Restore — blocking, and verified twice

`src/server/output_validation.rs` is shared with `115-17` in this same wave. Both files were restored
from `/bin/cp` snapshots (never `git checkout --`, `git stash` or `git clean`) and verified:

| Check | Expected | Observed |
|---|---|---|
| `shasum -a 256 -c` on the pre-control snapshot | `OK` both files | `src/server/output_validation.rs: OK`, `fuzz/…/fuzz_schema_draft_pin.rs: OK` ✓ |
| `git diff --exit-code src/server/output_validation.rs` | 0 | **0** ✓ |
| `git status --short src/` | empty | **empty** ✓ |
| `shasum -a 256 src/server/output_validation.rs` | 115-16's shipped hash | `a97f5cb2335d9b195ea75eb787084a25c219aeaefb103bd1b96f7e481763192c` ✓ |

That hash is the value `115-17-SUMMARY.md` recorded (`a97f5cb2…3192c`), so the file 115-19 inherits is
byte-for-byte what 115-16 shipped.

**Rebuilt AFTER restoring**, before any evidence run — so no control's binary described the mutated
crate in the campaign numbers below.

## The real evidence, against the restored tree

| Run | Expected | Observed |
|---|---|---|
| `cargo +nightly fuzz build fuzz_schema_draft_pin` | exit 0 | **exit 0** ✓ |
| `-runs=0` replay over the committed corpus | exit 0, artifacts empty | **exit 0, 20 098 runs**, artifacts **EMPTY** ✓ |
| `-max_total_time=300` campaign | exit 0, artifacts empty | **exit 0, 3 614 479 runs in 301 s**, artifacts **EMPTY** ✓ |
| `cargo fmt --all -- --check` from INSIDE `fuzz/` | exit 0 | **exit 0** ✓ |
| `make lint` (before EACH commit) | exit 0 | **exit 0**, twice ✓ |
| `cargo +nightly clippy --bin fuzz_schema_draft_pin -- -D warnings` | clean | **exit 0** ✓ |

**The replay count exceeds the 15 seeds** because cargo-fuzz also loads its default corpus directory
(documented by 115-13): `10045 files found in corpus/fuzz_schema_draft_pin` plus the same directory
again as the default, `seed corpus: files: 20090`, `Done 20098 runs`.

**Campaign vs 115-15's 3 697 874: 3 614 479, a 2.3% decrease.** Inside the ~3% widening cost
`T-115-DEP-19` accepts, and no false positive from the widened strip or scan surfaced in 3.6M runs —
which is the check that Task 1's fix did not trade one false-positive window for another.

`+nightly` was used for every fuzz invocation (`-Zsanitizer=address`; stable rustc refuses).
**`make test-fuzz` is cited nowhere** — `D-115-U` records it fail-open twice over.

## Corpus hygiene

| Check | Expected | Observed |
|---|---|---|
| `git ls-files fuzz/corpus/fuzz_schema_draft_pin/ \| grep -c '/[0-9][0-9]_'` | 15 | **15** ✓ |
| `git status --porcelain fuzz/corpus/fuzz_schema_draft_pin/` | seed + README only | `?? …/15_dependencies_named_default`, ` M …/README.md` — nothing else ✓ |
| `grep -c '^\| \`15_' README.md` | 1 | **1** ✓ |
| files on disk in the corpus dir after the campaign | (many) | **11 772** — and git saw two ✓ |

11 772 units on disk against two paths visible to git is `.gitignore`'s narrow re-include
(`corpus/*` ignored, `README.md` and `[0-9][0-9]_*` re-included) doing its job — `D-115-Z` /
`T-115-DEP-17` closed by observation, and the count taken with `git ls-files`, never `ls`.

## Task 1's documentation changes, itemised

- **(a)** `"dependencies"` appended LAST, byte-identical to `src/` and `tests/` including the trailing
  `// draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME (D-115-03-C)` comment. Confirmed by
  reading the constant, not only by `grep -c` (which returns 2 — the second is the example document in
  the rustdoc). Rustdoc now carries the 115-16 derivation, the independence rationale with both
  directions of the trade, and the LIMIT.
- **(b)** The two-list distinction, in BOTH places a reader would try to harmonize them: the module
  docs' `Excluded, each with its reason` bullet and the `DO NOT "FIX" THIS TO MATCH THE OTHER WALKERS`
  comment. The framing is that they answer different questions — *do the eras agree about this
  keyword's MEANING* (no, so invariant 3 must skip) versus *are its VALUES schema positions the
  normalizer must reach* (yes, `D-115-03-C`).
- **(c)** WR-03's two surviving copies of the retracted "TOTAL — no skip condition" claim are gone from
  `assert_no_legacy_dialect_survives`'s rustdoc and the `fuzz_target!` call site. `grep -n 'TOTAL — no
  skip condition\|Total — no skip condition'` returns **nothing**. The retracted wording is
  deliberately NOT quoted in the amendment — it lives in `115-REVIEW.md` WR-03 — which is the
  `D-115-AI(1)` collision avoided by construction rather than rediscovered.
- **(d)** Invariant 6's REACH limit at its own rustdoc, with the two covering mechanisms named so the
  sentence is a pointer rather than a complaint.
- `is_dialect_neutral` / `is_neutral_subschema` / `DIALECT_NEUTRAL_KEYWORDS` **untouched** — the
  criterion grep over the diff returns **0**. They were position-aware before the crate's walkers were.

## Deviations from Plan

All five booked as **`D-115-AJ`** in `deferred-items.md`, continuing after `D-115-AI` per
`115-17-SUMMARY.md`'s instruction. **`115-19` must continue at `D-115-AK`.** The whole-ID duplicate
check `grep -o '^## D-115-[A-Z0-9]\{1,2\}' | sort | uniq -d` returns **nothing**.

### 1. [criterion vs. instrument] Control D's message reports URIs, not JSON pointers

The criterion asks for *"a `/dependencies/default` declaration in the reported list"*.
`collect_dialect_declarations` pushes `map.get("$schema")` VALUES and discards the path, so the list is
`["http://json-schema.org/draft-07/schema#"]`. The position is recoverable from the `Input was:` /
`normalized to:` documents the same message embeds, and the single-file run makes attribution
unambiguous regardless. **Not fixed:** threading pointers through the collector would require the paths
to be built by the same walk — more restatement of the rule, for a diagnostic improvement.

### 2. [scope] Task 2 edits a file its `<files>` line does not list

`T-115-DEP-15`'s mitigation requires Control F's exit 0 to be *"repeated in the constant's rustdoc"*,
which lives in Task 1's file. The controls' numbers do not exist until Task 2 has run. Resolved by
writing the LIMIT qualitatively in Task 1 and appending the measured numbers in Task 2 — the
alternative, writing numbers in Task 1, is booking ahead of measurement (`D-115-G` / `D-115-AG`).

### 3. [criterion vs. history] The README's tracked-count criterion collides with a measurement

*"The tracked-seed count reads 15 in every place it appears"* would require rewriting WR-07's measured
pair (`ls | grep -c '^[0-9]'` returned **3382** against a tracked count of **14**) into a falsehood.
Resolved by stating the live count explicitly, rewriting the surrounding prose to refer to "the tracked
count above", and labelling the 3382/14 pair as historical and deliberately not restated. Same shape as
`D-115-AI(1)` / `D-115-AH(1)`.

### 4. [Rule 2, in scope] The WR-06 correction applied to this file's mirror rustdoc

115-17 corrected the `tests/` copy; the plan did not explicitly list the fuzz copy, but Task 1(a)
rewrites that rustdoc and leaving a claim the round's own review falsified would contradict the
purpose of the round. The corrected text attributes the false-positive risk to the SCAN only and cites
the Task 1 control that measured invariant 2 passing.

### 5. [SCOPE BOUNDARY — logged, not fixed] Pre-existing clippy failures in two other fuzz targets

`cd fuzz && cargo +nightly clippy --all-targets -- -D warnings` exits 101 on `fuzz_token_code_mode`
(4 errors) and `auth_flows` (8, e.g. `len_zero` at `:355`). Untouched by this plan and invisible to
every repository gate (`D-115-AB`). `fuzz_schema_draft_pin` itself is clean. **Unowned.**

## The inherited-findings check, answered explicitly

- **The `<which_fence_catches_what_here>` taxonomy** — `D-115-AI(5)` measured it WRONG for
  `tests/property_tests.rs` (two derived fences, not one). Re-checked against THIS file rather than
  restated: it **holds here**, for a structural reason. The only other candidate is invariant 3's
  `is_dialect_neutral`, independent of the crate's keyword lists — but it is not a second fence for
  this defect class twice over: a nested `$schema` makes a document non-neutral and every reproduction
  document carries one, and `dependencies` is additionally absent from `DIALECT_NEUTRAL_KEYWORDS`.
  Control F confirms it behaviourally: with both copies blind, nothing in this file fired. Recorded at
  invariant 6's rustdoc under *"Is it really the ONLY one? Checked, not assumed"*, with the `tests/`
  divergence named so the taxonomies are not re-unified.
- **`D-115-AI(4)`'s trap** — checked and absent. Controls D and E mutate `src/`, not this file's list,
  so invariant 6's reach is intact while the rule under test is broken. The one configuration where the
  reach IS destroyed is Control F, which is the measurement of the limit rather than an accident.
- **`D-115-AJ` is this plan's ID**, continuing after `AI`; no duplicate.
- **No pre-commit hook** — confirmed independently (`.git/hooks/` holds only `*.sample`;
  `core.hooksPath` points at that same directory). `make lint` was run explicitly before each commit.

## Threat register outcomes

| Threat ID | Disposition | Outcome |
|---|---|---|
| T-115-DEP-13 | mitigate | **Closed, and the window was OPEN when the plan started.** The stale mirror was observed CRASHING on correct behaviour (exit 77, invariant 5) and exiting 0 after the widening. The 3 614 479-run campaign is the check that no false positive survives |
| T-115-DEP-14 | mitigate | **Closed.** Control E ran with invariant 5 silenced and invariant 6 FIRED at `container: dependencies, name: default` — its reach is measured directly, not inferred. The residual (shared omission) is Control F's exit 0, attributed to `src`'s own-literal fence, which was itself measured FAILING at `output_validation.rs:1429` in that tree |
| T-115-DEP-15 | mitigate | **Closed.** Control F's exit 0 is recorded in the constant's rustdoc, invariant 6's rustdoc and the seed's README row — the three places named — each stating that a green run is not evidence of an absent list omission |
| T-115-DEP-16 | mitigate | **Closed.** Both retracted "TOTAL — no skip condition" copies replaced with the corrected scope and cross-referenced to the module-doc correction; amend-not-delete, with the falsified literal left in `115-REVIEW.md` rather than requoted |
| T-115-DEP-17 | mitigate | **Closed by observation.** 11 772 libFuzzer units on disk, `git status --porcelain` over the corpus shows exactly the new seed plus the README, count taken with `git ls-files` = 15 |
| T-115-DEP-18 | mitigate | **Closed.** The seed's `$id` is the reserved non-resolvable `example.test`; its only `$ref` is the local pointer `#/dependencies/default`. `jsonschema` stays `default-features = false`, re-asserted by 115-19 |
| T-115-DEP-19 | accept | Confirmed cheap: campaign 3 614 479 vs 115-15's 3 697 874 = **−2.3%**, inside the ~3% already accepted. No regression to investigate |
| T-115-SC | accept | No `Cargo.toml` / `Cargo.lock` in the diff, no package-manager install, no manifest edit — no supply-chain checkpoint reachable |

## What this plan deliberately did NOT do

- **`.planning/REQUIREMENTS.md` UNTOUCHED and `requirements mark-complete` NOT run.** SCHM-01's
  re-booking follows 115-19's whole-closure gate; booking ahead of measurement is `D-115-G` /
  `D-115-AG`, and this requirement has carried that defect twice.
- **`make quality-gate` not run** — it belongs to 115-19, per the 115-14/15/16/17 precedent.
- **Seed 14's README row not corrected** — round-3 `IN-01` (it misattributes its provenance to seed 12
  rather than 13) is explicitly 115-19's to book, and was not silently fixed while editing the table.
- **`src/server/output_validation.rs` is a 0-byte diff.** It appears in `files_modified` solely because
  Controls D, E and F mutate it.

## What 115-19 inherits

- **⚠ `D-115-AJ` IS TAKEN — five items. Continue at `D-115-AK`.**
- **`src/server/output_validation.rs` is clean** — `git diff --exit-code` 0, `shasum` OK,
  `git status --short src/` empty, hash `a97f5cb2…3192c` identical to what 115-16 shipped and what
  115-17 handed over. Nothing to unwind.
- **All three `SUBSCHEMA_MAP_KEYWORDS` literals are now six entries in the same order**, with the
  trailing `// draft-04..2019-09…` comment on `src/` and `tests/` and on `fuzz/`. 115-19's source-text
  drift gate has a consistent tree to gate.
- **The fuzz target's measured blind spot is Control F**, and 115-19's drift gate is one of the two
  mechanisms this SUMMARY and three rustdocs name as covering it. That gate is now load-bearing in a
  documented way — if it is weakened, the residual becomes unowned.
- **Two clippy-dirty fuzz targets** (`fuzz_token_code_mode`, `auth_flows`) are unowned debt that no
  repository gate can see.

## Self-Check: PASSED

- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — FOUND; `SUBSCHEMA_MAP_KEYWORDS` at six with
  `"dependencies"` on the line immediately after `"dependentSchemas",`; import list is
  `normalize_bytes, validate_bytes, SchemaVerdict` and contains neither seam constant;
  `grep 'TOTAL — no skip condition\|Total — no skip condition'` returns nothing
- `fuzz/corpus/fuzz_schema_draft_pin/15_dependencies_named_default` — FOUND, 230 bytes, decodes to
  CR-01's reproduction document
- `fuzz/corpus/fuzz_schema_draft_pin/README.md` — FOUND; one `15_` row; tracked count stated as 15
- `.planning/phases/115-…/deferred-items.md` — FOUND; `D-115-AJ` present exactly once; whole-ID
  duplicate check returns nothing
- Commit `768460f9` — FOUND in `git log`
- Commit `5e9c1474` — FOUND in `git log`
- `git diff --diff-filter=D --name-only 768460f9~1 5e9c1474` — **no deletions**
- `git diff --exit-code src/server/output_validation.rs` — **exit 0**; `shasum -a 256 -c` — **OK**
- `git ls-files fuzz/corpus/fuzz_schema_draft_pin/ | grep -c '/[0-9][0-9]_'` — **15**
- `fuzz/artifacts/fuzz_schema_draft_pin/` — **empty**
