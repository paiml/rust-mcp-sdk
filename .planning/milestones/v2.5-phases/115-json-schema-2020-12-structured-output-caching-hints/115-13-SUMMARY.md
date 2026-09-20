---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 13
subsystem: testing
tags: [schm-01, json-schema, draft-2020-12, property-testing, fuzzing, gap-closure, generator-coverage, quality-gate, embedded-schema-resource]

# Dependency graph
requires:
  - phase: 115-12
    provides: "the recursive `normalize_schema_dialect` this plan generatively covers, and the fixed-example fences it generalizes"
  - phase: 115-09
    provides: "`fuzz_support::normalize_bytes` / `validate_bytes`, `arb_schema_document()`, the fuzz target and its 11-seed corpus — the three artifacts widened here"
  - phase: 115-03
    provides: "`compile_for_era`, the era-branched compile path the property and the fuzzer drive"
provides:
  - "`arb_schema_document()` generates `$id`-bearing EMBEDDED SCHEMA RESOURCES with an independently-drawn dialect — the shape it structurally excluded — measured at 100 of 256 cases"
  - "`property_schema_normalization_is_idempotent_and_surgical` gains a RECURSIVE surgical-scope strip, a total dialect-purity assertion and a pointer-addressed nested-key arm"
  - "fuzz invariant 5 `assert_no_legacy_dialect_survives` — TOTAL, no skip condition, walk implemented independently of the crate's own detector"
  - "`$defs` / `$id` / sole-key `$ref` in `DIALECT_NEUTRAL_KEYWORDS`, so invariant 3 reaches embedded-resource shapes for the first time"
  - "Seeds `12_embedded_legacy_resource` and `13_embedded_resource_no_dialect` — 13 committed seeds"
  - "The whole-phase `make quality-gate` 115-12 deferred, plus the PR-blocking `pmat --checks complexity` the gate does not cover"
  - "SCHM-01 re-booked `[x]` on post-fix measured evidence, as option (a) of `115-VERIFICATION.md` § Human Verification Required"
  - "`D-115-AB` — `make quality-gate` cannot see the `fuzz/` crate at all"
affects: [116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A generator that was widened must PROVE it emits the new shape — a counter/println run behind `--nocapture`, removed after. A widened generator that never emits the shape is the same failure the phase already shipped once"
    - "When a fix makes one era legitimately STRICTER than the other, do not relax an EQUALITY invariant to admit the documents — add a second, structural invariant over the post-transform artifact that needs no cross-era reasoning"
    - "Split a 'relax the predicate' finding by which half is safe: reference keywords (`$defs`/`$id`/`$ref`) carry no dialect switch and can be admitted; the dialect declaration itself cannot"
    - "A detector reimplemented INDEPENDENTLY in the fuzz target is what catches a detector/rewriter disagreement the crate's own postcondition cannot see"
    - "`grep -c 'REOPENED'` unchanged is a check that a record was AMENDED, not deleted — so a closure block must not repeat the word it counts"

key-files:
  created:
    - fuzz/corpus/fuzz_schema_draft_pin/12_embedded_legacy_resource
    - fuzz/corpus/fuzz_schema_draft_pin/13_embedded_resource_no_dialect
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-13-SUMMARY.md
  modified:
    - tests/property_tests.rs
    - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
    - fuzz/corpus/fuzz_schema_draft_pin/README.md
    - src/server/output_validation.rs
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "Took `115-VERIFICATION.md` missing-item-4's SECOND option (add an invariant) over its first (relax `is_dialect_neutral`), because after the recursive pin a nested legacy declaration makes v2 STRICTER than v1 — v1's auto-detect still honours the per-resource switch — and invariant 3 is an EQUALITY, so admitting those documents would fail the fuzzer on CORRECT behaviour"
  - "The `$ref`/`$defs`/`$id` half of the same finding IS safe to widen and was: those keywords carry no dialect switch. `$ref` under one guard — neutral only as the SOLE key of its object, because siblings are ignored in draft-07 and honoured under 2020-12"
  - "Both new fences carry a MANDATORY negative control, observed and recorded with its message. An unfired fence is not evidence — the standard `115-VERIFICATION.md` applied when it refused to inherit the SUMMARYs' conclusions"
  - "The property's surgical-scope strip had to become recursive BEFORE the purity assertion could be added: a root-only strip reads a legitimate nested rewrite as collateral damage and fails on correct behaviour"
  - "Invariant 5 reimplements the dialect walk independently rather than reaching for the crate's `first_legacy_dialect` — the crate's own postcondition already uses that detector, so only a separate walk catches a detector/rewriter disagreement"
  - "The `clippy::similar_names` gate failure was fixed by RENAMING, not by an `#[allow]` — a `#[allow]` is the last resort, not the first"
  - "The Phase 115 ROADMAP marker deliberately stays `[~]`: scoring the closure is `/gsd:verify-phase 115`'s job, and this plan's output is the evidence it scores"

patterns-established:
  - "A plan that DEFERS `make quality-gate` to a later plan must have that later plan actually run it — 115-12 deferred it and the first run of it here exited 2 on a lint 115-12 introduced"
  - "`cargo clippy --all-targets --features full -- -D warnings` is strictly WEAKER than `make lint` (which adds pedantic + nursery with the repo allow-list). CLAUDE.md says so; this is the measured instance"
  - "Workspace `exclude` is a gate blind spot: a whole crate can drift for three green gates. Assert nothing about `fuzz/` from a green `make quality-gate`"

# Metrics
duration: ~50m
completed: 2026-08-01
tasks_completed: 3
files_modified: 10
---

# Phase 115 Plan 13: SCHM-01 Gap Closure — Generator Widening + Whole-Phase Gate Summary

Closed `115-VERIFICATION.md` `missing:` item 4: the property test and the fuzz target can now both
REACH the `$id`-bearing embedded-schema-resource shape that `115-12` fixed, and both were OBSERVED
to fail against a deliberately reverted root-only normalizer. The whole-phase gate ran green over
the fixed tree and SCHM-01 is re-booked `[x]` on post-fix measured evidence.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Embedded resources in the property generator + dialect purity | `c913aeb1` | `tests/property_tests.rs` |
| 2 | Fuzz invariant 5, widened allowlist, two seeds | `d74ef8b7` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, corpus README + 2 seeds, `deferred-items.md` |
| — | *(deviation)* `clippy::similar_names` regression from 115-12 | `cab8937a` | `src/server/output_validation.rs` |
| 3 | CI-equivalent gate + SCHM-01 re-booking | `1621b3b0` | `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md` |

### Task 1 — the property generator

`arb_schema_document()` previously called `object.remove("$schema")` and then injected a ROOT
declaration only, which is precisely what made the generated space unable to contain the defect.
It now draws a nested dialect INDEPENDENTLY from the same seven-way spread (extracted as
`arb_dialect()`), plus a boolean deciding whether to embed. When embedding, the document gains
`$defs.Inner` carrying `$id: "https://example.test/inner"`, `type: "integer"` and the drawn
declaration, plus `properties.n = {"$ref": "#/$defs/Inner"}` on the root.

**SEP-2106, asserted by inspection and recorded here as the plan requires:** `example.test` is a
reserved, non-resolvable host; an `$id` establishes a base URI with no fetch; and the only `$ref`
this module generates is the LOCAL pointer `#/$defs/Inner`. `grep -n '\$ref' tests/property_tests.rs`
returns two lines — one rustdoc sentence and the `json!({ "$ref": "#/$defs/Inner" })` literal.
Neither carries an `http`/`https`/`file` scheme.

The property gained three things:

- the surgical-scope strip became RECURSIVE (`strip_dialect_declarations`), mirroring the shipped
  traversal rule (string-valued `$schema` only; never descend into `const`/`enum`/`default`/
  `examples`). Without this a legitimate nested rewrite reads as collateral damage;
- a total **dialect-purity** assertion over `once` (`collect_dialect_declarations`), naming the
  `(Violates, Conforms)` row in its failure message;
- a nested-key arm addressed through `Value::pointer("/$defs/Inner/$schema")`, so the failure
  message shows the path.

The root-key match arm is unchanged — it is still the arm that catches a normalizer that DELETES
rather than rewrites. The module's `#[cfg(all(test, feature = "fuzzing", feature = "validation"))]`
gate and its rustdoc rationale for reaching a private function through the `fuzzing` seam are
untouched; only the sentence describing the normalizer's scope was corrected.

### Task 2 — the fuzz target

**(a)** invariant 2's strip made recursive, message corrected from "a key other than the ROOT
$schema".

**(b)** invariant 5, `assert_no_legacy_dialect_survives`: TOTAL, no skip condition, holding for
every input that parses as JSON including the documents invariant 3 excludes. Its walk is written
INDEPENDENTLY of `first_legacy_dialect` — the crate's unit-test postcondition uses that detector, so
only a separate walk catches a detector/rewriter disagreement. `normalize_bytes` is called exactly
once at each invariant helper's entry and never in the `fuzz_target!` body.

**(c)** `$defs`, `$id` and `$ref` added to `DIALECT_NEUTRAL_KEYWORDS`; `$ref` guarded structurally
(neutral only as the SOLE key of its object); `$defs` values recursed into like `properties` values,
its keys never allowlist-checked. **The nested-`$schema` exclusion was NOT relaxed**, with the reason
written into the module docs.

**(d)** module docs: invariant 5 added to the numbered list, invariant 2's "ONLY at the root
`$schema` key" wording corrected, and the allowlist paragraph extended with all three new keywords
and their reasons.

**(e)** two seeds via the README's `python3` recipe, selector byte 1, both with two-digit prefixes so
`fuzz/.gitignore`'s `[0-9][0-9]_*` re-include actually commits them.

## Verification Results

Every command re-run through absolute binaries (`$HOME/.cargo/bin/cargo`, `/usr/bin/make`,
`/usr/bin/grep`) per `D-115-AA` — a bare `cargo`/`grep` in this environment is fail-OPEN.

| # | Command | Expected | Observed |
|---|---|---|---|
| 1 | `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)' --test-threads=1` | > the `--features full` count | **19 passed, 0 skipped** |
| 1b | the same with `--features full` (module not compiled) | baseline | **18 passed, 0 skipped** |
| 2 | `cd fuzz && cargo +nightly fuzz build fuzz_schema_draft_pin` | exit 0 | **exit 0** |
| 3 | `cd fuzz && cargo +nightly fuzz run … -- -runs=0 corpus/fuzz_schema_draft_pin` | exit 0, artifacts empty | **exit 0**, artifacts dir **0 entries** |
| 4 | `cd fuzz && cargo +nightly fuzz run … -- -max_total_time=300` | exit 0, artifacts empty | **exit 0, 3 951 202 runs in 301 s**, artifacts dir **0 entries** |
| 5 | `/usr/bin/make quality-gate` | exit 0 | **exit 0** (second run — see Deviations) |
| 6 | `pmat quality-gate --fail-on-violation --checks complexity` | exit 0 | **PASSED, Total violations: 0** (pmat 3.15.0) |
| 7 | the seven-binary nextest run | 78 passed | **78 tests run: 78 passed, 0 skipped** |

The `--features full` / `--features "full fuzzing"` pair is the proof the widened module actually
RAN rather than being silently gated out: 18 → 19.

### `make quality-gate` totals

Exit code **0**. From the transcript: **309 `test result:` lines**, **5052 passed / 0 failed / 81
ignored**, and **0** non-`ok.` result lines. Per `D-115-T` the transcript itself is not pasted — a
redirected `make` capture is unfaithful in this environment; the exit code is the evidence and the
counts are reported as read, not as proof.

For reference, `115-10` measured 5045 passed across the same 309 lines at phase close; the +7 is the
gap closure's own new unit tests and cases.

### The seven binaries, individually

| Binary | `115-VERIFICATION.md` | Observed |
|---|---|---|
| `structured_tool_output` | 20 | **20** |
| `v2_caching_hints` | 19 | **19** |
| `v1_lists_golden` | 7 | **7** |
| `v2_schema_tripwires` | 13 | **13** |
| `v2_core_schema_facts` | 8 | **8** |
| `vendored_schema_provenance` | 6 | **6** |
| `phase115_contract_bindings` | 5 | **5** |
| **Total** | **78** | **78** |

No deviation to report — every count matches.

### Grep-shaped criteria

- `grep -c '\$defs' tests/property_tests.rs` → **7** (≥ 2 required)
- `grep -c 'example.test' tests/property_tests.rs` → **2** (≥ 1 required); `grep -n
  'https://example.test'` shows the `$id` value at `tests/property_tests.rs:912`
- `ls fuzz/corpus/fuzz_schema_draft_pin/ | grep -c '^[0-9][0-9]_'` → **13**
- `git status --porcelain fuzz/corpus/fuzz_schema_draft_pin/` → exactly the two new seeds plus the
  modified README. No libFuzzer-discovered unit leaked past `.gitignore`, despite the directory
  holding 2600+ runtime units after the campaign
- `grep -c '^| \`1[23]_' …/README.md` → **2**
- `grep -c 'assert_no_legacy_dialect_survives' fuzz/…/fuzz_schema_draft_pin.rs` → **2**
- `grep -n 'normalize_bytes'` → the seam appears once at each invariant-helper entry (`:351`,
  `:392`) and nowhere in the `fuzz_target!` body
- `git diff --name-only <closure-base>..HEAD | grep -E 'Cargo\.(toml|lock)$'` → **no hits** (no
  supply-chain review triggered)
- `git diff <closure-base> -- src/ | grep -c '^+.*\bpub fn\|^+.*\bpub struct\|^+.*\bpub enum'` →
  **0** (additive 2.x-minor posture preserved without a `cargo public-api` run)
- `.planning/REQUIREMENTS.md` SCHM-01 reads `[x]` and its block contains `(Violates, Violates)`
- `grep -c 'REOPENED' .planning/REQUIREMENTS.md` → **1**, unchanged
- `grep -n '115-12-PLAN.md\|115-13-PLAN.md' .planning/ROADMAP.md` → both `[x]`

## The Coverage Proof (Task 1)

A widened generator that never emits the new shape is the same failure the phase already shipped
once, so this was measured rather than assumed. A temporary `println!` was added to the property
body behind `--nocapture`, the module was run, and the instrumentation removed:

```
COVERAGE-PROOF embedded non-2020-12 dialect: http://json-schema.org/draft-04/schema#
COVERAGE-PROOF embedded non-2020-12 dialect: http://json-schema.org/draft-06/schema#
COVERAGE-PROOF embedded non-2020-12 dialect: http://json-schema.org/draft-07/schema#
COVERAGE-PROOF embedded non-2020-12 dialect: https://json-schema.org/draft/2019-09/schema
COVERAGE-PROOF embedded non-2020-12 dialect: arzc://qje.e../jhxdou
… (invented URIs)
```

**100 of the 256 default proptest cases** carried `/$defs/Inner/$schema` with a non-2020-12 URI,
spanning all four legacy drafts and the invented-URI branch. The instrumentation is not in the
committed file (`grep 'COVERAGE-PROOF'` → no hits).

## The Two Negative Controls (Observed, Not Assumed)

Both were run against a deliberately reverted `pin_dialect_in_place` (its recursion removed, leaving
a root-only rewriter). `src/server/output_validation.rs` was snapshotted before and restored after;
`git status --short src/` read clean afterwards both times.

**Task 1 — the property test FAILED** with the dialect-purity message:

> Test failed: a LEGACY $schema survived normalization:
> `["http://json-schema.org/draft-04/schema#"]` in
> `{"const":null,"$defs":{"Inner":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-04/schema#","type":"integer"}},"properties":{"n":{"$ref":"#/$defs/Inner"}}}`.
> … the vacuous-validator bypass 115-VERIFICATION.md reproduced as the row `root-draft07 + embedded
> (v1,v2) = (Violates, Conforms)`, v2 measurably WEAKER than v1.

proptest shrank it to exactly the embedded-resource minimal case. `successes: 0`.

**Task 2 — seed `12_embedded_legacy_resource` tripped invariant 5, exit 77.** Run by invoking the
built fuzz binary directly on the single seed file, so the attribution is unambiguous:

```
Running: fuzz/corpus/fuzz_schema_draft_pin/12_embedded_legacy_resource
A LEGACY $schema SURVIVED NORMALIZATION: ["http://json-schema.org/draft-07/schema#"] …
Input was:       {"$schema":"…draft-07…","type":"object","properties":{"n":{"$ref":"#/$defs/Inner"}},"$defs":{"Inner":{"$id":"https://example.test/inner","$schema":"…draft-07…","type":"integer"}}}
normalized to:   {"$schema":"https://json-schema.org/draft/2020-12/schema", … "$defs":{"Inner":{… "$schema":"…draft-07…" …}}}
SUMMARY: libFuzzer: deadly signal
```

The `-runs=0` corpus replay under the same reverted build also exited 1 (on a mutated unit reaching
the same invariant). After restore, rebuild and replay: **exit 0, artifacts dir empty.** The crash
artifact written by the negative control was deleted; `ls fuzz/artifacts/fuzz_schema_draft_pin/` is
empty.

## Deviations from Plan

### 1. [Rule 1 — Bug] `make quality-gate` FAILED on its first run, on a lint 115-12 introduced

- **Found during:** Task 3(b), the first `/usr/bin/make quality-gate` invocation.
- **Issue:** exit **2** at `make lint`:

  ```
  error: binding's name is too similar to existing binding
     --> src/server/output_validation.rs:826:13
  826 |         let row3 = root_and_embedded_legacy_schema();
  note: existing binding defined here --> :792:13   (`let rows = [`)
  = note: `-D clippy::similar-names` implied by `-D warnings`
  ```

  `row3` beside `rows`, inside `v2_pin_still_enforces_an_embedded_legacy_resource` — the very fence
  115-12 added. 115-12's SUMMARY records `cargo clippy --all-targets --features full -- -D warnings`
  as exit 0, and that is TRUE and NOT a contradiction: `similar_names` is a **pedantic** lint, and
  only `make lint` enables the pedantic + nursery groups with the repo's allow-list. This is the
  measured instance of CLAUDE.md § *Why `make quality-gate` (not individual cargo commands)*, and
  the concrete reason a plan that defers the gate must have a later plan actually run it.
- **Fix:** renamed to `regression_direction`, which is what the binding is. **No `#[allow]`** — the
  plan reserves that for irreducible complexity, and this is a name.
- **Files modified:** `src/server/output_validation.rs`
- **Commit:** `cab8937a`
- **Re-run:** `make quality-gate` exit **0**.

### 2. [Rule 2 — Missing critical] `make quality-gate` cannot SEE the `fuzz/` crate — ledger `D-115-AB`

- **Found during:** Task 2's formatting check.
- **Issue:** `Cargo.toml:665` lists `fuzz` in the workspace `exclude` array, so `cargo fmt --all`,
  `cargo clippy --all-targets`, `cargo test` — and therefore `make quality-gate` and CI — format,
  lint, build and run **nothing** under `fuzz/`. Measured: at commit `c913aeb1` the fuzz target
  carried a **pre-existing rustfmt violation** (introduced by 115-09, survived 115-09's, 115-10's
  and 115-12's green gates) while root `cargo fmt --all -- --check` exited **0**. It was found only
  because this plan ran `cargo fmt` from *inside* `fuzz/`.
- **Fix:** the formatting violation was fixed in passing (`cd fuzz && cargo fmt --all`, one file
  changed, target rebuilt and replayed clean afterwards). The **CI gap is NOT fixed** and is filed
  **unowned** as `D-115-AB` — adding `fuzz` to the workspace members would pull `libfuzzer-sys` and
  a nightly-only sanitizer flag into every build; the right shape is a separate CI job, which is a
  workflow change rather than a phase-115 code change.
- **Why this matters beyond formatting:** it is the same class of blindness this closure exists to
  repair, one level up. `115-12` recorded that `fuzzing` is in neither `default` nor `full`, so a
  fence written behind that feature does not run under the gate. `D-115-AB` is strictly larger: the
  entire crate that HOSTS those fences is outside the gate's field of view. A fuzz target that
  stopped compiling would be reported by nothing.
- **Files modified:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `deferred-items.md`
- **Commit:** `d74ef8b7`

### 3. [Rule 2 — Missing critical] The corpus README documented the fail-open `cargo fuzz run` form

- **Found during:** Task 2(e), while adding the two seed rows.
- **Issue:** the README's replay recipe read `cargo fuzz run …` without `+nightly`, which per
  `D-115-U` fails to BUILD on a stable default toolchain. A reader following the README verbatim
  would get a non-zero exit and no fuzzing.
- **Fix:** the replay command now reads `cargo +nightly fuzz run`, with a short paragraph naming
  `D-115-U` and warning off `make test-fuzz`. The "Adding a case" example's filename was also bumped
  from `12_my_case` to `14_my_case`, since 12 and 13 are now taken.
- **Commit:** `d74ef8b7`

### 4. [Documented, not a defect] The `-runs=0` replay covers more than the 13 seeds

`cargo fuzz run <target> -- -runs=0 <dir>` appends cargo-fuzz's own default corpus directory to the
positional arguments, so the replay loaded the committed seeds **and** the ~1580 units libFuzzer
discovered in earlier sessions (3198 runs on the first replay, 10 674 after the campaign). That is
strictly more evidence, not less, and it exited 0. It is recorded so the run count in this SUMMARY
is not read as "13 seeds took 3198 runs".

### Not deviations, stated so they are not mistaken for one

- **The nested-`$schema` exclusion in `is_neutral_subschema` was left in place.** In-plan and
  deliberate; the reason is now written into the module docs.
- **`115-13` did not touch `115-VERIFICATION.md`** and did **not** flip the Phase 115 ROADMAP marker
  to complete. Both are in-plan: re-verification is `/gsd:verify-phase 115`'s job.
- **`deferred-items.md` and `.planning/STATE.md` are edited although `files_modified` names neither.**
  The plan's `<output>` block mandates the ledger append with a two-character ID, and the executor
  workflow mandates the STATE.md update.
- **The ledger's whole-ID duplicate check** (`grep -o '^## D-115-[A-Z0-9]\{1,2\}' | sort | uniq -d`),
  corrected by 115-12, was used and returns nothing across **38** headings.

## Known Stubs

None. Every added path is exercised: `arb_dialect()` on every generated document,
`strip_dialect_declarations` / `collect_dialect_declarations` on every property case and every fuzz
iteration, `assert_no_legacy_dialect_survives` on every input that parses as JSON, and the widened
allowlist on every neutrality decision. Both new fences were observed to FIRE against the defect and
to PASS against the fix.

## Threat Flags

None. The plan's `<threat_model>` dispositions were all satisfied:

- **T-115-CR01-06** (SSRF via `$id`/`$ref`) — `mitigate`: every generated `$id` is the
  non-resolvable `example.test` host and every generated `$ref` is a local `#/$defs/…` pointer;
  seed 12/13's `$id` is the same host. Structurally fenced regardless — `binary(v2_schema_tripwires)`
  13/13 re-run in Task 3 asserts no retriever is compiled in, across cargo's declared AND resolved
  graphs.
- **T-115-CR01-07** (DoS via pathological generation) — `accept`: the 300 s campaign closed at
  3 951 202 runs with an empty artifacts dir.
- **T-115-CR01-08** (corpus tampering) — `mitigate`: `git status --porcelain` over the corpus
  directory shows exactly the two new seeds plus the README, with 2600+ runtime units present and
  correctly ignored.
- **T-115-CR01-09** (a requirement booked on absent evidence) — `mitigate`: the `[~]` → `[x]` flip
  ran only after every command in Task 3(b) exited 0 and all seven counts matched; the downgrade
  block was amended, not deleted.
- **T-115-CR01-10** (a vacuous fence) — `mitigate`: both negative controls observed, plus the
  generator coverage proof.
- **T-115-SC** — `accept`: no package-manager install, no manifest edit; asserted by the
  `Cargo.toml`/`Cargo.lock` criterion above.

No new network endpoint, auth path, file access pattern or schema change at a trust boundary.

## What This Leaves Open

- **`/gsd:verify-phase 115`** — the phase marker stays `[~]` until re-verification scores this
  closure. This SUMMARY is evidence, not a verdict.
- **`D-115-AB`** is **unowned**: nothing in CI reaches the `fuzz/` crate.
- **`D-114-S`** (nothing watches `modelcontextprotocol/ext-tasks`) and **`D-113-U`** remain open and
  are untouched by this plan. Phase 114's **D-18 hold stays ENGAGED** — a green Phase 115 closure is
  exactly when an unrelated hold gets released by accident.

## Self-Check: PASSED

- Files claimed created: `12_embedded_legacy_resource`, `13_embedded_resource_no_dialect` — both
  present on disk and tracked in git.
- Files claimed modified: `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`,
  `fuzz/corpus/fuzz_schema_draft_pin/README.md`, `src/server/output_validation.rs`,
  `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md`, `deferred-items.md` —
  all present, all with the claimed content verified by grep.
- Commits claimed: `c913aeb1`, `d74ef8b7`, `cab8937a`, `1621b3b0` — all resolve in `git log`.
- Symbols claimed: `arb_dialect`, `strip_dialect_declarations`, `collect_dialect_declarations`,
  `assert_no_legacy_dialect_survives`, `regression_direction` — all found; `row3` and
  `COVERAGE-PROOF` both absent.
