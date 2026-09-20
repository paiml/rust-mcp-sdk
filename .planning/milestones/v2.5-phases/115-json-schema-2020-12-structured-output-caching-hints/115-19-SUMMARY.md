---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 19
subsystem: testing
tags: [schm-01, json-schema, draft-2020-12, gap-closure, drift-gate, source-text-gate, contracts, booking, negative-controls, meta-schema-derivation]

# Dependency graph
requires:
  - phase: 115-16
    provides: "the six-entry shipped `SUBSCHEMA_MAP_KEYWORDS`, the meta-schema DERIVATION this gate's expectation is anchored to, and the structural container fence"
  - phase: 115-17
    provides: "the corrected `tests/property_tests.rs` mirror, and `D-115-AI(4)` — the reachability lesson that shapes this gate's expectation source"
  - phase: 115-18
    provides: "the corrected `fuzz/` mirror, so all three copies were consistent before the gate that holds them consistent landed; and Control F, the blind spot this gate covers"
provides:
  - "`tests/keyword_list_mirrors.rs` — a FEATURELESS source-text drift gate over all three literal copies of both keyword lists, comparing them as ORDERED sequences AND against the meta-schema-derived expectation; confirmed running INSIDE `make quality-gate`"
  - "A rescoped `output_schema_draft_pin` equation: head, `walk:` clause, name-position invariant and POSTCONDITION now state ONE scope over SIX keywords, with the residual named"
  - "Three `contracts/binding.yaml` note HEADS stating the corrected scope in their opening sentence, plus five `115-16 COMPLETENESS CORRECTION` paragraphs"
  - "SCHM-01 booked `[x]` on this round's measured evidence, written AFTER both gates exited 0, with all three prior records amended"
  - "`D-115-AK` — a complete triage of all TEN round-3 review findings, plus the residual this closure cannot close"
  - "`D-115-AL` — the gate results, a red gate run discarded with justification, the standing rules, and two plan-text predictions measured wrong"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A drift gate over N copies must compare them to something NONE of them is. Copy-to-copy comparison catches the lag/lead mode; only a DERIVATION-anchored expectation catches the lockstep removal, which is the mode that deletes coverage with zero test failures"
    - "A source-text gate reaches code no compiler in the workspace touches. `tests/keyword_list_mirrors.rs` imports nothing from the crate, so it can assert over `fuzz/` — which the workspace `exclude` array hides from every other gate (D-115-AB)"
    - "An extractor needs two anti-vacuity guards or it is a decoration: the definition must be found EXACTLY ONCE per file, and the extraction must be non-empty. Three empty lists are trivially equal"
    - "A red gate run must be DISCARDED with a justification recorded, never dropped. The identical binary was re-run standalone and passed before the transient failure was set aside, and the failure is booked"
    - "A grep-shaped criterion over a file constrains what that file may SAY about the criterion — third and fourth instances this round, in a new test's rustdoc and in a contract's historical quotation"

key-files:
  created:
    - tests/keyword_list_mirrors.rs
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-19-SUMMARY.md
  modified:
    - contracts/mcp-protocol-sdk-v1.yaml
    - contracts/binding.yaml
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "The gate's EXPECTATION is a literal in the test file with the derivation in its rustdoc, sourced from NONE of the three copies. Deriving it from any of them would reproduce D-115-AI(4) exactly — a fence whose reachability comes from the artifact it checks"
  - "The gate declares no import of the SDK crate. That is what lets it read the `fuzz/` copy, and it also removes the temptation to derive the expectation from the `fuzz_support` seam"
  - "The historical retracted wording in the contract's `115-14 SCOPE CORRECTION` was PARAPHRASED, not deleted and not requoted — the whole-file grep criterion and amend-don't-delete are otherwise mutually unsatisfiable. Same resolution 115-17 and 115-18 each reached; the verbatim text lives in `115-REVIEW.md` WR-04"
  - "`WR-04` was added to `D-115-AK`'s DISCHARGED table although the plan's enumeration omitted it. It is Gap 2, discharged by this plan's own Task 2; leaving it out would have made the 'all ten findings' criterion pass over a nine-finding table"
  - "The first `make quality-gate` run exited 2 and was NOT normalized. The identical test binary was re-run standalone (2 passed, no rebuild), free space checked both sides, the whole gate re-run to exit 0, and the transient booked as `D-115-AL(2)`"
  - "`.planning/ROADMAP.md`'s Phase 115 marker left `[~]` and `115-VERIFICATION.md` untouched — this plan produces the evidence, `/gsd:verify-phase 115` scores it"

patterns-established:
  - "Run the negative control for the CONTROL ITSELF. The amend-not-delete `grep -c` guard was exercised (2, then 1) rather than trusted; the plan required it and it was worth the ninety seconds"
  - "When a plan predicts an instrument's behaviour under corruption, measure BOTH the predicted corruption and a neighbouring one. The line-wise contract reader turned out to have partial, position-dependent sensitivity to YAML damage — worse than none for a reader who assumes it has either"

# Metrics
duration: ~150m
completed: 2026-08-02
tasks_completed: 3
files_modified: 6
---

# Phase 115 Plan 19: The Drift Gate, the Contract's One Scope, and the Fourth Booking Summary

Round 4 on SCHM-01, last of four. `115-16`/`115-17`/`115-18` closed Gap 1's code. What none of them
closed is **why this defect class keeps returning**: `115-REVIEW.md` WR-01 found three literal copies
of two keyword lists, each file's rustdoc calling the mirror REQUIRED, with **no gate that they
agree** — and both failure modes silent. A fourth hand-maintained copy is not a fix; a gate is, and
the gate has to compare the copies to something none of them is.

That gate now exists, is featureless, runs inside `make quality-gate`, and all three of its failure
shapes were observed with the file at fault named in each. Gap 2's contract rescoping landed with it.
Then — and only after `make quality-gate` and the PR-blocking `pmat quality-gate` both exited 0 —
SCHM-01 was booked.

## What shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Source-text drift gate over all three copies plus the derivation-anchored expectation | `4059e1e1` | `tests/keyword_list_mirrors.rs` |
| 2 | Gap 2: one stated scope in the equation, walk clause, invariants and three binding note heads | `73e5e043` | `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml` |
| 3 | The whole-closure gate, then the SCHM-01 booking, the ROADMAP and two ledger entries | `781e6b04` | `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `deferred-items.md` |

## Task 1 — THE THREE DRIFT-GATE CONTROLS, verbatim, each naming the file at fault

All three were run against the final Task 1 tree, restored between each from a `/bin/cp` snapshot
verified with `shasum -a 256 -c` (never `git checkout --`, `git stash` or `git clean`). The
pre-control `src/server/output_validation.rs` hash was `a97f5cb2335d9b19…3192c` — byte-identical to
what `115-16` shipped and what `115-17`/`115-18` handed over.

### Control A — ONE copy drifts (run twice, once per mirror)

`"dependencies"` removed from `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` only:

```
panicked at tests/keyword_list_mirrors.rs:277:9:
assertion `left == right` failed: `SUBSCHEMA_MAP_KEYWORDS` has DRIFTED between
fuzz/fuzz_targets/fuzz_schema_draft_pin.rs and src/server/output_validation.rs.
fuzz/fuzz_targets/fuzz_schema_draft_pin.rs: ["properties", "patternProperties", "$defs",
  "definitions", "dependentSchemas"]
src/server/output_validation.rs: ["properties", "patternProperties", "$defs", "definitions",
  "dependentSchemas", "dependencies"]
CONSEQUENCE, which is why this is a hard failure and not a lint: a copy that LAGS the crate turns
the property and fuzz scans into FALSE-POSITIVE generators against CORRECT behaviour — 115-18
measured the fuzz target exiting 77 on a document the shipped walk rightly left untouched. A copy
that LEADS the crate means the shipped walk is skipping a real schema position.
```

Restored, re-run green (**2 passed**). Repeated with the entry removed from
`tests/property_tests.rs` only — the identical message with **`tests/property_tests.rs`** in place of
the fuzz path, at the same `:277`. *A gate that reports "they differ" without saying which copy is
wrong costs the next maintainer the same hour twice*, so both were run rather than one being inferred
from the other.

### Control B — LOCKSTEP removal from all three

`"dependencies"` removed from `src/`, `tests/` and `fuzz/` together. **Assertion 1 passed** — the
three agreed — and the failure came from assertion 2, at a different line:

```
panicked at tests/keyword_list_mirrors.rs:300:9:
assertion `left == right` failed: `SUBSCHEMA_MAP_KEYWORDS` in src/server/output_validation.rs
disagrees with the DERIVATION-anchored expectation.
found:    ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas"]
expected: ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas",
           "dependencies"]
CONSEQUENCE: if all three copies changed together, assertion 1 above PASSED and this is the only
instrument that fired. A lockstep removal deletes coverage with zero other test failures —
`patternProperties` and `dependentSchemas` sat unexercised from 115-14 until 115-16 exactly that
way (115-REVIEW.md WR-02), and `dependencies` was missing from every copy at once (CR-01).
```

**`:277` versus `:300` is the evidence, not a detail.** The two assertions are at different lines, so
the failure LOCATION is positive proof of which one fired — assertion 1 passed for this
configuration, exactly as WR-01's second mode predicts, and only the derivation-anchored expectation
caught it. This is the criterion that distinguishes this gate from a mirror check.

All three files restored; `shasum -a 256 -c` → **OK** on all three; `git status --short src/
tests/property_tests.rs fuzz/` **empty**.

### Control C — the extractor is not vacuous

`const SUBSCHEMA_MAP_KEYWORDS:` renamed to `SUBSCHEMA_MAP_KEYWORDS_OLD` in the fuzz copy:

```
panicked at tests/keyword_list_mirrors.rs:232:5:
assertion `left == right` failed: expected EXACTLY ONE definition of `SUBSCHEMA_MAP_KEYWORDS` in
fuzz/fuzz_targets/fuzz_schema_draft_pin.rs, found 0.
FAILURE MODE: this gate locates the list by its definition line. Zero matches means the constant
was renamed, moved or deleted and this gate is now asserting nothing about that file; two or more
means it would silently compare the first one it found.
```

A hard failure naming the file, not a silently-empty extraction that would have made all three
trivially equal — the fail-open shape `D-115-AE` and `D-115-AA` each record in another guise.

### The gate's other criteria

| Criterion | Expected | Observed |
|---|---|---|
| `cargo test --test keyword_list_mirrors -- --test-threads=1` (**NO features**) | 2 passed | **2 passed**, exit 0 ✓ |
| `cargo nextest run --features full -E 'binary(keyword_list_mirrors)'` | 2 passed | **2 tests run: 2 passed** ✓ |
| `grep -c 'use pmcp' tests/keyword_list_mirrors.rs` | 0 | **0** (after a fix — see Deviation 1) ✓ |
| `pmat quality-gate --fail-on-violation --checks complexity` with the file present | exit 0 | **exit 0, Total violations: 0** ✓ |
| `cargo fmt --all -- --check` | exit 0 | **exit 0** ✓ |
| `make lint` (pedantic + nursery) | exit 0 | **exit 0** (after a fix — see Deviation 2) ✓ |
| **the gate appears in the `make quality-gate` transcript** | present | **present**, running its 2 tests ✓ |

That last row is the property the whole design rests on: featureless means CI runs it.

## Task 2 — THE TWO CONTRACT CONTROLS, and the prediction one of them falsified

### Control A — the PyYAML check is live, and the contrast that justifies it

Line 249 (inside the rewritten `formula:` block scalar) de-indented from column 9 to **column 5**:

```
yaml.scanner.ScannerError: while scanning a simple key
  in "contracts/mcp-protocol-sdk-v1.yaml", line 249, column 5
could not find expected ':'
  in "contracts/mcp-protocol-sdk-v1.yaml", line 254, column 13
```

PyYAML **exit 1**, naming the line. In that same corrupted state,
`binary(phase115_contract_bindings)` reported **5 tests run: 5 passed** — it hand-parses line-wise, so
it is blind to the damage. **That is exactly the contrast the PyYAML check exists for**, and it is
why the check was added as a verify command rather than assumed covered by the bindings gate.

**But the plan's prediction is only half true, and the other half is more interesting.** De-indenting
the same line to **column 1** instead, the bindings gate ALSO fails — and instructively:

```
FAILURE MODE: a binding references an equation that contracts/mcp-protocol-sdk-v1.yaml does not
define. …
  `result_caching_hints`
  `structured_content_shape`
```

It lost every equation defined AFTER the corruption point, while `output_schema_draft_pin` — defined
BEFORE it — still resolved. So the line-wise reader has **partial, position-dependent** sensitivity to
YAML damage, which is worse than none for a reader who assumes it has either. Booked as
`D-115-AL(5)`.

### Control B — the ghost-binding resolver is live over the edited file

`function: pin_dialect_in_place` temporarily changed to `pin_dialect_in_place_v2`:

```
FAILURE MODE: GHOST BINDING — a binding marked `status: implemented` names a symbol that does not
exist. The contract claims a behaviour is implemented by a function nobody wrote.

  contracts/binding.yaml:592 equation `output_schema_draft_pin` function `pin_dialect_in_place_v2`
  (module_path `pmcp::server::output_validation`) — no `fn`, `enum`, `struct`, `trait`, `const`,
  `static` or `type` named `pin_dialect_in_place_v2`, and no `pub use` re-export of it, anywhere
  under src/
```

Naming the symbol, as required. Restored, re-run green (**5 passed**). This proves the five-test gate
still covers `binding.yaml` after this task's edits, rather than passing because the edits moved
something it stopped reading.

### The three rewritten note heads, quoted

Required: none may carry "anywhere", "at any depth", or an unqualified "EVERY string-valued
`$schema`". A grep alone is insufficient — those words legitimately survive in the CORRECTION
paragraphs below each head — so each was read and is recorded here as a quotation.

**`normalize_schema_dialect`:**

> Pure and idempotent: `Cow::Borrowed` when no string-valued `$schema` in any SCHEMA POSITION of the
> document names a dialect other than `DRAFT_2020_12`, otherwise a clone in which every such
> `$schema` is overwritten with `DRAFT_2020_12`.

**`first_legacy_dialect`:**

> The DETECTOR half of the normalization: returns the first `$schema` string in a SCHEMA POSITION —
> the scope the equation's `walk:` clause defines, searched root-first — that is not already
> `DRAFT_2020_12`, or `None`.

**`pin_dialect_in_place`:**

> The REWRITER half: overwrites every string-valued `$schema` in a SCHEMA POSITION with
> `DRAFT_2020_12`, in place, on the clone `normalize_schema_dialect` already had to make — SCHEMA
> POSITION being the scope the equation's `walk:` clause defines.

Each then names the exclusion and the six subschema-map keywords, in the same sentence, before any
CORRECTION paragraph.

### Task 2's measured criteria

| Criterion | Expected | Observed |
|---|---|---|
| `python3 -c "import yaml; yaml.safe_load(…)"` over BOTH contract files | `yaml ok`, exit 0 | **`yaml ok`, exit 0** ✓ |
| `binary(phase115_contract_bindings)` | 5 passed | **5 tests run: 5 passed** ✓ |
| `grep -n 'anywhere in s'` | nothing | **0 hits** ✓ |
| `grep -n 'root or any depth'` | nothing | **0 hits** (was 2 — see Deviation 3) ✓ |
| `SCHEMA POSITION` in the `formula:` block | present | present at `:249`, the equation head ✓ |
| `grep -c 'dependencies'` in the contract | ≥ 3 | **9** (was **0**) ✓ |
| `grep -c '115-16 COMPLETENESS CORRECTION'` in `binding.yaml` | 5 | **5** ✓ |
| `git diff -- contracts/ \| grep -c '^[-+]\s*signature:\|function:\|status:\|module_path:'` | 0 | **0** — no binding identity moved ✓ |
| `pmat comply check` | recorded verbatim | **exit 1**, pre-existing project-level findings only — see below ✓ |

**`pmat comply check`** exits 1 on findings that name neither `output_schema_draft_pin` nor any Phase
115 equation (`grep -c 'output_schema_draft_pin'` over its output → **0**): `CB-1204` (contracts have
preconditions but no `build.rs`), `CB-1208` (L0 paper-only: 67 bindings but no `build.rs` or trait
enforcement), `CB-1308` (`team-servers-v1.yaml` at L1), `CB-1201` (PV Lint failed — the `pv` CLI is
not installed). This is exactly the situation `D-115-A`/`D-115-B` record and that `Makefile:797-808`
handles by design: `pmat comply check --path .` is informational here, and `make quality-gate`'s own
`comply` step printed *"pmat comply reported project-level advisories (informational; see CLAUDE.md
D-07); team-servers binding drift is enforced below"* and then resolved all four team-servers
bindings green. Recorded verbatim rather than treated as a pass or a blocker.

## Task 3 — THE WHOLE-CLOSURE GATE

`df -h /System/Volumes/Data` before starting: **47 Gi available** (95% capacity) — checked both
before and after, so `D-115-0`'s disk-exhaustion shape is ruled out by measurement rather than by
assumption.

### ⚠ The first `make quality-gate` run exited 2, and was NOT normalized

```
thread 'live_http_cross_owner_isolation' panicked at src/shared/streamable_http.rs:458:18:
Failed to load native root certificates: Custom { kind: NotFound, error: "no native root CA
certificates found (errors: [… kind: Os(Error { code: -36, message: \"I/O error.\" }) …])" }
```

Both tests in `tests/tool_as_task_lifecycle_http.rs` failed this way — a macOS keychain
trust-settings I/O error at a **pre-existing `.expect` in production code**, in a file no plan in this
closure touches. **Verified before discarding, not after:** the identical test binary
(`tool_as_task_lifecycle_http-cc836a23396b9623`, no rebuild) was run standalone and reported **2
passed**; free space was unchanged; the whole gate was then re-run end to end.

A red gate run that disappears from the record is how a phase talks itself into a green one. Booked
as **`D-115-AL(2)`**, with the `.expect` itself flagged **unowned** — it belongs to whoever owns the
transport, not to a schema phase.

### The gate, re-run

| Command | Expected | Observed |
|---|---|---|
| `/usr/bin/make quality-gate` (not redirected — `D-115-T`) | exit 0 | **exit 0** ✓ |
| its totals | recorded | **5060 passed / 0 failed / 81 ignored across 312 `test result:` lines** |
| `pmat quality-gate --fail-on-violation --checks complexity` | exit 0 | **exit 0, Total violations: 0** ✓ |
| the seven SCHM-02/SCHM-03 binaries | **78** | **78 tests run: 78 passed** ✓ |
| `output_validation::tests` | 20 | **20** ✓ |
| `output_validation` under `"full fuzzing"` | 25 | **25** ✓ |
| `binary(property_tests)` under `"full fuzzing"` | 21 | **21** ✓ |
| the same under `full` | 18 | **18** ✓ |
| `binary(keyword_list_mirrors)` | 2 | **2** ✓ |

**Every count matched the plan's list. Nothing was normalized.** The totals above come from the
background transcript and are reported alongside the exit code rather than in place of it, per
`D-115-T`.

The seven binaries, individually: `structured_tool_output` 20, `v2_caching_hints` 19,
`v1_lists_golden` 7, `v2_schema_tripwires` 13, `v2_core_schema_facts` 8,
`vendored_schema_provenance` 6, `phase115_contract_bindings` 5 = **78**, matching
`115-VERIFICATION.md`. SCHM-02 and SCHM-03 are **referenced, not rewritten** — neither record was
touched.

### The derivation, re-run on this tree

Not inherited. The one-command re-derivation now recorded in `.planning/REQUIREMENTS.md` was executed
against `jsonschema` 0.49.2's shipped meta-schemas and reproduced `115-16`'s table exactly: 28 rows,
the six subschema-map keywords plus the two rejects (`$vocabulary`, `dependentRequired`) that the
meta-schema-self-reference criterion excludes, and nothing else.

### Closure-wide diff checks

| Criterion | Expected | Observed |
|---|---|---|
| `git diff --stat c350cb53~1 HEAD` mentions `Cargo.toml`/`Cargo.lock` | 0 | **0** ✓ |
| new `pub fn`/`pub struct`/`pub enum` under `src/` across the closure | 0 | **0** ✓ |
| new `pub const` under `src/` | the 2 in `fuzz_support` | **exactly 2**, both `pub mod fuzz_support` re-exports ✓ |

The milestone's additive 2.x-minor posture holds without a `cargo public-api` run.

### THE AMEND-NOT-DELETE CONTROL — both counts

The `grep -c REOPENED` guard is the only check that the prior records were amended rather than
replaced, and a guard nobody has seen fail is not a guard. After writing the new SCHM-01 block:

| Step | `grep -c 'REOPENED' .planning/REQUIREMENTS.md` |
|---|---|
| baseline, new block written | **1** |
| a sentence containing that word temporarily appended to the new block | **2** |
| that sentence removed | **1** |

The guard moves. It measures what it claims.

### The booking

**SCHM-01 reads `[x]`**, and the new block contains the required literals `dependencies.default`
(2×), `rewritten=false` (4×) and `(Violates, Violates)` (5×). Had any command above exited non-zero
it would read `[~]` with the failing command named; none did at booking time, and the discarded run
of §*The first `make quality-gate`* is recorded in the block itself rather than omitted.

| Booking criterion | Observed |
|---|---|
| `grep -c 'REOPENED' .planning/REQUIREMENTS.md` | **1**, unchanged ✓ |
| `grep -c '115-16\|115-17\|115-18\|115-19' .planning/REQUIREMENTS.md` | **14** (≥ 2 required) ✓ |
| all four plan lines in `.planning/ROADMAP.md` marked `[x]` | **4/4** ✓ |
| `grep -n '19 plans' .planning/ROADMAP.md` | present ✓ |
| `grep -c '^- \[~\] \*\*Phase 115' .planning/ROADMAP.md` | **1** — still `[~]` ✓ |
| `git status --porcelain …/115-VERIFICATION.md` | **empty** — not edited ✓ |
| `grep -c '^## D-115-AK'` / `'^## D-115-AL'` | **1** / **1** ✓ |
| whole-ID duplicate check `grep -o '^## D-115-[A-Z0-9]\{1,2\}' \| sort \| uniq -d` | **nothing** ✓ |
| `D-115-AK` names all ten round-3 findings | CR-01, WR-01..WR-06, IN-01..IN-03 — **all present** ✓ |

The three prior SCHM-01 records are intact, all above the new block's predecessor, none deleted. The
traceability row was extended, not replaced.

## Deviations from Plan

All booked in `deferred-items.md` as **`D-115-AK`** (the review triage and the residual) and
**`D-115-AL`** (this round's process outcome). **The ledger continues at `AK`/`AL`, not the
`AH`/`AI` the plan text names** — `AH` was consumed by 115-16, `AI` by 115-17 and `AJ` by 115-18,
each booking its own measured deviations, which is correct. Writing a duplicate would have broken the
whole-ID check that is one of this plan's own acceptance criteria.

### 1. [Rule 3 — criterion collision] The new test's own rustdoc matched the grep criterion it must satisfy

- **Found during:** Task 1 verification. `grep -c 'use pmcp' tests/keyword_list_mirrors.rs` returned
  **1**, not 0 — the module rustdoc heading quoted the criterion's own pattern while explaining why
  the file must not import the crate.
- **Fix:** rephrased the heading to *"No import of the crate under test"* and added an inline note
  that the pattern is deliberately not quoted anywhere in the file. The INTENT — the gate compiles
  against nothing from the crate, which is what lets it read the `fuzz/` copy — is untouched.
- **Booked:** `D-115-AL(3)`, third instance of `D-115-1` / `D-115-AH(1)` / `D-115-AI(1)` this round.

### 2. [Rule 3 — blocking] `clippy::doc_markdown` on `OpenAPI` in the new file

- `make lint` (pedantic + nursery, run before each commit because **there is no pre-commit hook in
  this checkout** — `D-115-AH(3)`, confirmed independently) failed with
  `-D clippy::doc-markdown` on an unbackticked `OpenAPI` in the `DATA_ONLY_KEYWORDS` rustdoc.
  Backticked; `make lint` exit 0 thereafter. Recorded because the file would not have compiled under
  CI otherwise and no other gate would have said so before the PR.

### 3. [Rule 3 — criterion collision] The whole-file `root or any depth` grep vs amend-don't-delete

- **Issue:** the criterion greps the WHOLE contract file for that phrase. Two hits existed: the
  equation head (this task's target) and the `115-14 SCOPE CORRECTION`'s **historical quotation** of
  the retracted wording. Deleting the quotation would violate amend-don't-delete; keeping it fails
  the criterion.
- **Fix:** the round's established resolution — PARAPHRASE the retracted claim in the correction
  paragraph and point at `115-REVIEW.md` and git history for the verbatim text, with the collision
  named inline so a future reader does not "restore" the literal. Identical in shape to
  `D-115-AI(1)` (115-17, WR-06) and `D-115-AJ`'s WR-03 handling (115-18).
- **Booked:** `D-115-AL(3)`.

### 4. [measurement — plan prediction falsified] The line-wise bindings reader is NOT uniformly blind to YAML damage

- The plan states `binary(phase115_contract_bindings)` is *"expected to"* still pass over corrupted
  YAML. True for an in-block re-indent (measured: 5 passed while PyYAML exits 1). **False for a
  column-1 de-indent**, where it fails — and only for equations defined AFTER the corruption point.
- **Impact:** none on the shipped edits; the PyYAML check is still justified, and more precisely than
  the plan argued. **Booked:** `D-115-AL(5)`.

### 5. [Rule 2 — completeness] `WR-04` added to the triage table the plan's enumeration omitted

- The plan lists the findings to record as DISCHARGED as `WR-01`, `WR-02`, `WR-03`, `WR-06`, `CR-01`
  — omitting `WR-04`, which is Gap 2 and is discharged by this plan's own Task 2. Leaving it out
  would have made the *"names all ten"* criterion pass over a nine-finding table. Added, with the
  mechanism named.

### 6. [scope, pre-existing] The ROADMAP bookkeeping the plan asked for already existed

- The plan instructs changing `**Plans**: 15 plans` to 19 and ADDING a `Gap closure — round 3`
  heading. Both were already written when round 3 was planned, including the plan's requested note on
  the two numbering schemes. Only the `115-18` and `115-19` plan lines needed flipping from `[ ]` to
  `[x]`; both were flipped with shipped evidence appended. **Booked:** `D-115-AL(5)`.

### 7. [SCOPE BOUNDARY — logged, not fixed] The transient gate failure's underlying `.expect`

- `src/shared/streamable_http.rs:458` panics rather than erroring when the macOS trust store is
  momentarily unreadable. Pre-existing production behaviour, unrelated to this phase, **unowned**.
  Booked as `D-115-AL(2)`.

## Threat register outcomes

| Threat ID | Disposition | Outcome |
|---|---|---|
| T-115-DEP-20 | mitigate | **Closed.** `tests/keyword_list_mirrors.rs` is featureless (confirmed running inside the `make quality-gate` transcript), compares all three copies as ordered sequences AND against the meta-schema-derived expectation. Both WR-01 modes observed: Control A names the file at fault (twice, once per mirror), Control B fires the derivation assertion at `:300` while assertion 1 at `:277` passes |
| T-115-DEP-21 | mitigate | **Closed.** The extractor asserts the definition is found exactly ONCE per file and that the extraction is non-empty; Control C renamed a constant and got *"found 0"* rather than a vacuous pass |
| T-115-DEP-22 | mitigate | **Closed.** Equation head rescoped (both carriers of the retracted total gone from the file), walk clause / name-position invariant / POSTCONDITION at six keywords with the residual named, three binding note heads corrected in their opening sentence — each quoted above and read rather than grepped |
| T-115-DEP-23 | mitigate | **Closed.** Task 3 ran last by construction; the marker flip was gated on measured exit codes and named counts; the prior records are amended with the `grep -c` guard itself exercised (2 → 1); the new block names its predecessor's limitation and both measured LIMITS. Re-verification scores it independently |
| T-115-DEP-24 | mitigate | **Closed.** `D-115-AK` triages all ten round-3 findings — six DISCHARGED with the mechanism named (WR-04 added beyond the plan's list), four unowned with reasons |
| T-115-DEP-25 | accept | Residual recorded in three places rather than left implicit: the module rustdoc (115-16), the contract POSTCONDITION's completeness correction (this plan), and `D-115-AK`. `components.default` → `rewritten=false`; Control F's exit 0; the inverse walk declined by 115-14 with a stated reason |
| T-115-DEP-26 | mitigate | **Closed, and sharpened.** PyYAML `safe_load` over both files is a verify command; its live-ness was measured, as was the bindings gate's PARTIAL, position-dependent sensitivity — which is a stronger justification for the check than the plan's |
| T-115-SC | accept | **No `Cargo.toml` / `Cargo.lock` anywhere in the closure diff** (asserted over `c350cb53~1..HEAD`), no package-manager install, no manifest edit. No supply-chain checkpoint reachable |

## What this plan deliberately did NOT do

- **`115-VERIFICATION.md` is untouched and the Phase 115 ROADMAP marker stays `[~]`.** Scoring this
  closure is `/gsd:verify-phase 115`'s job; this plan set produces the evidence it scores.
- **SCHM-02 and SCHM-03 were not reopened or rewritten** — only re-measured (78/78) and referenced.
- **`src/`, `tests/property_tests.rs` and `fuzz/` are 0-byte diffs from this plan.** They appear only
  as control targets, each restored from a `shasum -a 256 -c`-verified snapshot.
- **WR-05's remaining half** (writing the two member dispatches in one shape) was not done — it
  touches both walkers' bodies, and reshaping production code inside a booking round is how a closure
  acquires an unfenced change. Booked unowned in `D-115-AK`.
- **`requirements mark-complete` was not run.** All three SCHM requirements were already `[x]` and
  their traceability rows already `Complete`; running it would have risked overwriting the amended
  SCHM-01 row that is this plan's actual output. The booking was done by hand, deliberately.

## What the verifier inherits

- **`D-115-AK` and `D-115-AL` are taken. Continue at `D-115-AM`.**
- **`tests/keyword_list_mirrors.rs` is load-bearing in a documented way.** It is one of the two named
  mechanisms covering `115-18`'s measured fuzz blind spot (Control F, exit 0 with both lists blind).
  If it is weakened, that residual becomes unowned.
- **The whole-closure gate is green over the tree as committed**, and one red run is recorded rather
  than erased.
- **The residual is real and is stated in three places**: the walk stays name-dependent under an
  author-invented container. A deny-list over an open keyword space cannot be completed.

## Self-Check: PASSED

- `tests/keyword_list_mirrors.rs` — FOUND; contains `SUBSCHEMA_MAP_KEYWORDS`,
  `EXPECTED_SUBSCHEMA_MAP_KEYWORDS`, both `#[test]` names; `grep -c 'use pmcp'` → **0**
- `contracts/mcp-protocol-sdk-v1.yaml` — FOUND; contains `SCHEMA POSITION` in the `formula:` block;
  `grep -c 'dependencies'` → **9**; `anywhere in s` and `root or any depth` → **0** each
- `contracts/binding.yaml` — FOUND; `grep -c '115-16 COMPLETENESS CORRECTION'` → **5**; both files
  parse under PyYAML (`yaml ok`)
- `.planning/REQUIREMENTS.md` — FOUND; SCHM-01 `[x]`; `grep -c 'REOPENED'` → **1**
- `.planning/ROADMAP.md` — FOUND; Phase 115 marker `[~]`; all four round-3 plan lines `[x]`
- `.planning/phases/115-…/deferred-items.md` — FOUND; `D-115-AK` and `D-115-AL` present exactly once
  each; whole-ID duplicate check returns **nothing**
- `.planning/phases/115-…/115-VERIFICATION.md` — `git status --porcelain` **empty**, not edited
- Commit `4059e1e1` — FOUND in `git log`
- Commit `73e5e043` — FOUND in `git log`
- Commit `781e6b04` — FOUND in `git log`
- `git diff --diff-filter=D --name-only 4059e1e1~1 HEAD` — **no deletions**
