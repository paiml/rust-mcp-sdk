---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 12
subsystem: server
tags: [schm-01, json-schema, draft-2020-12, output-validation, gap-closure, blocker, contract-correction, embedded-schema-resource]

# Dependency graph
requires:
  - phase: 115-03
    provides: "`normalize_schema_dialect`, `compile_2020_12`, `compile_for_era` and the `cached_validator` era key — the compile path this plan repairs"
  - phase: 115-09
    provides: "`fuzz_support::validate_bytes` / `normalize_bytes`, the seam through which the defect was measured before and after"
  - phase: 115-11
    provides: "`contracts/binding.yaml` and `tests/phase115_contract_bindings.rs`, the resolver the two new helper bindings must satisfy"
  - phase: 115-10
    provides: "`deferred-items.md` in one-heading-per-item form, and `compile_for_era`'s binding — the precedent for binding a private helper that IS the mechanism"
provides:
  - "`normalize_schema_dialect` rewrites EVERY string-valued `$schema` at any depth, behind an unchanged signature — the `115-VERIFICATION.md` BLOCKER is closed in code"
  - "`first_legacy_dialect` (detector) and `pin_dialect_in_place` (rewriter), one stated traversal rule implemented twice, with the postcondition that fences their agreement"
  - "`v2_pin_still_enforces_an_embedded_legacy_resource` — a gate-visible fence OBSERVED to fail against the pre-fix normalizer"
  - "`normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone` — the guard against the corruption the CR-01 fix sketch would have introduced"
  - "`normalization_cases()` case (e): the `$id`-bearing embedded schema resource, `expected_owned == true`"
  - "`output_schema_draft_pin` formula + invariants 1 and 5 corrected, plus a NEW checkable postcondition invariant"
  - "Two new `implemented` bindings, and a `115-12 SCOPE CORRECTION:` on `normalize_schema_dialect`'s note"
  - "`115-RESEARCH.md`'s 'Root-level only' bullet amended at the source, so the generalization cannot ship a third time"
affects: [115-13, 116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A detector/rewriter pair over the same tree states its traversal rule ONCE in rustdoc and is fenced by a postcondition asserting they agree — a disagreement is otherwise invisible (it yields a `Cow::Owned` that still carries what it claims to have removed)"
    - "A `Cow::Borrowed` zero-allocation fast path decided by a whole-document predicate rather than a single-key lookup"
    - "Distinguish schema KEYWORDS from instance DATA by value type (`Value::String`) plus a `DATA_ONLY_KEYWORDS` skip list, so a normalization walk cannot corrupt a `const`/`enum`/`default`/`examples` payload"
    - "A negative control is run, observed and RECORDED with its panic message — an unfired fence is not evidence"

key-files:
  created:
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-12-SUMMARY.md
  modified:
    - src/server/output_validation.rs
    - contracts/mcp-protocol-sdk-v1.yaml
    - contracts/binding.yaml
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-RESEARCH.md
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md
    - .planning/STATE.md
    - .planning/ROADMAP.md

key-decisions:
  - "Rewrite EVERY dialect declaration, deliberately a SUPERSET of what `jsonschema` honours — an `$id`-less nested `$schema` is inert and is rewritten anyway, because that is what makes the postcondition statable without a per-node `$id` analysis"
  - "A `$schema` is a declaration ONLY when its value is a `Value::String`; the CR-01 fix sketch's `map.contains_key(\"$schema\")` would have replaced a `properties` subschema with a string and made the document uncompilable"
  - "Never descend into `const`/`enum`/`default`/`examples` — a `$schema` there is instance data, and rewriting it changes which instances conform"
  - "Source `compile_2020_12`'s warned `declared` value from the DETECTOR, not from the root key: reading `schema[\"$schema\"]` reports `<unknown>` for exactly the embedded-resource case the warning exists to explain"
  - "The `expect(\"a document with a root $schema key is a JSON object\")` is deleted, not relocated — its REASON survives as the checkable postcondition `first_legacy_dialect(&owned) == None`"
  - "All three new fences live in `mod tests` (feature `validation`), NOT in the `fuzzing`-gated module — `fuzzing` is in neither `default` nor `full`, which is precisely why the defect shipped past a green gate"
  - "v1 is NOT touched (D-01). Row 1's v1 column stays `Conforms` and is asserted to stay there; only the v2 column moved"
  - "SCHM-01 is deliberately NOT re-booked here — that is 115-13 Task 3, after the whole-phase gate has run (ledger entry `D-115-G`)"

patterns-established:
  - "Correct a false claim at its SOURCE document (`115-RESEARCH.md`), not only where it was copied to — the same generalization had already propagated into the module rustdoc and the provable contract"
  - "State a safety property in a form a test can execute. 'The pin wins unconditionally' was satisfied by a root-only normalizer; 'no `$schema` string anywhere is anything but the 2020-12 URI' is not"
  - "A grep-shaped acceptance criterion counting a phrase must account for YAML line folding — `embedded schema resource` split across a fold matched zero times until the block was rewrapped"

# Metrics
duration: ~1h05m
completed: 2026-08-01
tasks_completed: 3
files_modified: 7
---

# Phase 115 Plan 12: SCHM-01 Gap Closure — Recursive `$schema` Normalization Summary

Closed the single BLOCKER in `115-VERIFICATION.md`: `normalize_schema_dialect` now rewrites every
string-valued `$schema` at any depth — not just the document root — so a legacy dialect declaration
on an embedded schema resource can no longer resolve an empty vocabulary set and produce the
accept-everything sub-validator the v2 pin exists to prevent.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Recursive `$schema` normalization + rustdoc correction | `fdf236c8` | `src/server/output_validation.rs` |
| 2 | The three fences three test layers could not reach | `a9af3a5d` | `src/server/output_validation.rs` |
| 3 | Contract, bindings and research finding corrected | `60cda794` | `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml`, `115-RESEARCH.md` |

### Task 1 — the normalizer

`normalize_schema_dialect` keeps its recorded signature
(`fn normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value>`, byte-identical to
`contracts/binding.yaml`) and its `Cow::Borrowed` zero-allocation path. Its body is now a
detector/rewriter pair:

- `fn first_legacy_dialect(node: &Value) -> Option<&str>` — root-first walk, returns the first
  `$schema` string that is not `DRAFT_2020_12`, or `None`.
- `fn pin_dialect_in_place(node: &mut Value)` — overwrites every such key, in place, on the clone.

Both implement ONE traversal rule, stated once in rustdoc: a `$schema` key is a declaration only
when its value is a `Value::String`; recurse into every member value except the
`DATA_ONLY_KEYWORDS` (`const`, `enum`, `default`, `examples`); recurse into every array element.

The `.expect("a document with a root $schema key is a JSON object")` is gone — a non-object root
falls out of the walk naturally. Its REASON is preserved as a checkable postcondition:
`first_legacy_dialect(&owned) == None` after an `Owned` return, which is what guarantees an `Owned`
really was rewritten rather than silently handed back.

`compile_2020_12` keeps the `matches!(normalized, Cow::Owned(_))` warn trigger but sources
`declared` from `first_legacy_dialect(schema)` instead of the root key, and its message now says
the declaration was found "at the document root or on an embedded schema resource".

### Task 2 — the fences

All three live in `mod tests` (`#[cfg(all(test, feature = "validation"))]`), **not** in
`fuzz_support_tests`. That placement is the whole point: `fuzzing` is in neither `default` nor
`full`, so a fence written there does not run under `make quality-gate` — and all three of this
defect's would-be fences either sat behind that feature or structurally excluded the shape.

### Task 3 — the documents

`output_schema_draft_pin`'s `formula:` block, invariant 1 (now naming the embedded schema resource
explicitly) and invariant 5 (now string-valued keys at every depth, never instance data) restated
against the shipped rule, plus a NEW invariant carrying the postcondition. `binding.yaml` gains a
`115-12 SCOPE CORRECTION:` note (signature untouched) and two `implemented` bindings.
`115-RESEARCH.md`'s "Root-level only" bullet is amended in place, naming the `$id`-less shape it
measured and the `$id`-bearing shape it did not.

## The Three-Row Measurement, Re-Run on the Fixed Tree

Re-run through the same seam the review and the verifier used
(`pmcp::server::output_validation::fuzz_support::validate_bytes`, `jsonschema` 0.49.2, this working
tree) via a temporary example, deleted immediately after — `git status --short` confirms zero net
change to the tree. Schema: root object whose `properties.n` is `{"$ref": "#/$defs/Inner"}` and
whose `$defs.Inner` carries `$id: "https://example.test/inner"`, `$schema: draft-07` and
`type: integer`. Instance: `{"n": "NOT-AN-INTEGER"}`.

**Verbatim output:**

```
embedded-legacy-resource (v1,v2) = Some((Conforms, Violates))
control-no-nested-schema (v1,v2) = Some((Violates, Violates))
root-draft07 + embedded  (v1,v2) = Some((Violates, Violates))
```

Compared against `115-VERIFICATION.md`'s measurement of the same three documents:

| Case | Before 115-12 | After 115-12 | Verdict |
|---|---|---|---|
| embedded-legacy-resource | `(Conforms, Conforms)` | `(Conforms, Violates)` | v2 now enforces `type: integer`; **v1 unchanged**, as D-01 requires |
| control-no-nested-schema | `(Violates, Violates)` | `(Violates, Violates)` | unchanged — the control stays a control |
| root-draft07 + embedded | `(Violates, Conforms)` | `(Violates, Violates)` | **the BLOCKER row.** v2 is no longer weaker than v1 |

Row 3 is the plan's headline success criterion and it is met. Row 1's v1 column stayed `Conforms` —
v1's `jsonschema::validator_for` auto-detect still honours the embedded draft-07 declaration and
still drops `type` there. That is the D-01 freeze working as designed, not a residual defect, and
`v2_pin_still_enforces_an_embedded_legacy_resource` asserts it stays put so a future edit to the v1
arm is caught rather than absorbed.

## The Negative Control (Observed, Not Assumed)

The plan's evidence standard is that an unfired fence is not evidence. The file was snapshotted, the
pre-fix root-only body of `normalize_schema_dialect` was restored verbatim, the suite re-run, and
the file restored from the snapshot.

**Observed: `15 passed; 2 failed`.**

```
test server::output_validation::tests::normalize_schema_dialect_changes_only_dollar_schema_keys ... FAILED
test server::output_validation::tests::v2_pin_still_enforces_an_embedded_legacy_resource ... FAILED
```

Panic 1 — the behavioural fence, at `src/server/output_validation.rs:805`:

> BYPASS (embedded-legacy-resource): the v2 Draft 2020-12 pin accepted a STRING where the embedded
> schema resource declares `integer`. A `None` here means the legacy `$schema` on the `$id`-bearing
> `$defs.Inner` survived normalization, resolved an EMPTY vocabulary set there and produced a
> sub-validator that accepts everything — the vacuous-validator bypass the pin exists to close,
> moved one level down. `normalize_schema_dialect` must rewrite EVERY dialect declaration, not just
> the root one.

Panic 2 — the structural fence, at `src/server/output_validation.rs:1161`:

> assertion `left == right` failed: borrow/own decision is wrong for
> `{"type":"object","properties":{"a":{"$schema":"http://json-schema.org/draft-07/schema#","type":"string"}}}`
> — the no-op cases must allocate nothing / left: false / right: true

After restoring the fixed body: `17 passed; 0 failed`. The fences fire against the defect and pass
against the fix.

## Verification Results

Run in the plan's stated order. All commands invoked through `$HOME/.cargo/bin/cargo` — see
Deviations.

| # | Command | Expected | Observed |
|---|---|---|---|
| 1 | `cargo test --lib --features full output_validation::tests -- --test-threads=1` | 17 passed | **17 passed, 0 failed** |
| 2 | `cargo test --lib --features "full fuzzing" output_validation::fuzz_support_tests -- --test-threads=1` | 5 passed | **5 passed, 0 failed** |
| 3 | `cargo nextest run --features full -E 'binary(v2_schema_tripwires)' --test-threads=1` | 13 passed | **13 passed, 0 skipped** |
| 4 | `cargo nextest run --features full -E 'binary(phase115_contract_bindings)' --test-threads=1` | 5 passed | **5 passed, 0 skipped** |
| 5a | `cargo nextest run --features full -E 'binary(structured_tool_output)' --test-threads=1` | 20 passed | **20 passed, 0 skipped** |
| 5b | `cargo nextest run --features full -E 'binary(v1_lists_golden)' --test-threads=1` | 7 passed | **7 passed, 0 skipped** |
| 6 | `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | exit 0 | **exit 0** |
| 7a | `cargo fmt --all -- --check` | exit 0 | **exit 0** |
| 7b | `cargo clippy --all-targets --features full -- -D warnings` | exit 0 | **exit 0, no issues** |

`v1_lists_golden`'s byte-identical goldens passing is the direct evidence the v1 wire did not move.
`v2_schema_tripwires_validator_construction_sites_are_accounted_for` passing is the evidence the two
new functions introduced no validator construction site.

Grep-shaped acceptance criteria, all met:

- `grep -c 'fn first_legacy_dialect\|fn pin_dialect_in_place'` → **2**
- `normalize_schema_dialect`'s signature is byte-identical to `contracts/binding.yaml`'s record
- non-comment `validator_for|draft202012` occurrences → **2** (`compile_for_era`'s V1 arm,
  `compile_2020_12`'s `draft202012::new`)
- `Only the ROOT key is touched` / `does not trigger the bypass` → **no hits**
- `normalize_schema_dialect_changes_only_the_root_dollar_schema` → **no hits**;
  `..._changes_only_dollar_schema_keys` → present
- `--list` names both `v2_pin_still_enforces_an_embedded_legacy_resource` and
  `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone`; `17 tests, 0 benchmarks`
- `normalization_cases()` returns **5** entries, the fifth being the `$id`-bearing document with
  `true`
- `grep -c 'equation: output_schema_draft_pin' contracts/binding.yaml` → **8**
- `grep -c 'embedded schema resource' contracts/mcp-protocol-sdk-v1.yaml` → **2**
- zero `status: planned` bindings (the one `grep` hit is inside a comment)
- `115-RESEARCH.md`'s `Root-level only` bullet still present and now contains `$id`

`make quality-gate` was deliberately NOT run here — it is `115-13`'s Task 3, run once over the whole
closure.

## Deviations from Plan

### Auto-fixed / worked around

**1. [Rule 3 — Blocking] Every verification command had to be run through an absolute `cargo` path**

- **Found during:** Task 1 (grep criteria), then Task 2 (`--list` criterion)
- **Issue:** the environment rewrites bare `cargo` and `grep` through an `rtk` proxy that filters
  command output. `cargo test … -- --list` printed only the two `Finished`/`Running` lines with the
  17 test names silently dropped, exit 0; a `grep` for the new test names against that pipeline
  exited 1 and read as "the tests do not exist". `grep -v '^\s*//' … | grep -c …` likewise returned
  a proxy-formatted result rather than a count.
- **Fix:** re-ran the identical argv as `$HOME/.cargo/bin/cargo` and `/usr/bin/grep`. All criteria
  then evaluated correctly.
- **Files modified:** none — environment only.
- **Recorded as:** ledger entry **`D-115-AA`**, the first two-character ID (see below).

**2. [Rule 2 — Missing critical] The ledger's duplicate-ID check is wrong for two-character IDs**

- **Found during:** Task 2's ledger append
- **Issue:** `deferred-items.md`'s header mandates
  `grep -n 'D-115-' | awk -F'D-115-' '{print $2}' | cut -c1 | sort | uniq -d` returns nothing, and
  says explicitly that a two-character scheme breaks it and must extend it. Opening `AA` breaks it
  immediately — `cut -c1` collapses `AA` onto the existing `A`.
- **Fix:** replaced it with a whole-ID check,
  `grep -o '^## D-115-[A-Z0-9]\{1,2\}' deferred-items.md | sort | uniq -d`, which is correct for
  one- and two-character IDs alike and does not depend on the crosswalk rows dropping their `D-`
  prefix. Run against the file: **no duplicates**, 37 headings.
- **Files modified:** `deferred-items.md` (header + one new entry).

**3. [Rule 1 — Bug] `115-RESEARCH.md` and `deferred-items.md` edits beyond the declared `files_modified`**

- `files_modified` declares `115-RESEARCH.md` but not `deferred-items.md`. The plan's `<output>`
  block explicitly requires appending any deviation there with a two-character ID, so the edit is
  mandated by the plan even though the frontmatter list omits it. Recorded here rather than
  absorbed.

**4. [Rule 1 — Bug] `.planning/STATE.md`'s Current Position was clobbered before this plan ran**

- **Found during:** the post-Task-3 tree check
- **Issue:** the `execute-phase` orchestrator's own state write (uncommitted at plan start)
  overwrote the reopened-phase narrative — "REOPENED. 11/11 plans shipped, but NOT complete on the
  merits … SCHM-01 downgraded `[x]` → `[~]`" — with a generic `Plan: 1 of 13 / Status: Executing
  Phase 115`. Left as-is, STATE.md would have understated both the phase's position and the reason
  it was reopened.
- **Fix:** restored the reopen narrative, updated it to `Plan: 12 of 13`, and marked the gap
  **CLOSED IN CODE** with the re-run measurement and the three commit hashes. The pre-115-12 defect
  text is kept verbatim below it, for the record.

### Not deviations, stated so they are not mistaken for one

- **Row 1's v1 column stays `Conforms`.** D-01 freezes the v1 arm; the plan directed asserting it
  stays `None` and saying why. Done.
- **Case (d) flipped from `expected_owned == false` to `true`.** In-plan, and deliberate: an
  `$id`-less nested declaration is inert but is rewritten anyway (superset rule).
- **`cargo fmt --all` rewrote a tuple array in Task 2.** Formatting only; re-verified after.

## Known Stubs

None. Every code path added is reachable and exercised: `first_legacy_dialect` on every
normalization, `pin_dialect_in_place` on every `Owned` return, and both through all five
`normalization_cases()` entries plus the three behavioural documents.

## Threat Flags

None. The plan's `<threat_model>` disposition for `T-115-CR01-02` (rewriting a `$schema` that is
instance data) is `mitigate`; it is mitigated by the string-valued rule plus `DATA_ONLY_KEYWORDS`
and fenced by `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone`.
`T-115-CR01-03` is mitigated by the recursive walk plus the observed-to-fire behavioural fence.
`T-115-CR01-05` is mitigated by the non-resolvable `example.test` `$id` and the local
`#/$defs/Inner` pointer; `binary(v2_schema_tripwires)` (13/13) re-confirms no retriever is compiled
in. No new network endpoint, auth path, file access pattern or schema change at a trust boundary
was introduced — `files_modified` contains no manifest.

## What 115-13 Still Owes

`115-VERIFICATION.md` `missing:` item 4 is untouched here by design: `arb_schema_document()` in
`tests/property_tests.rs` injects a `$schema` at the root only, and
`is_dialect_neutral`/`is_neutral_subschema` in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` skip
every document containing an embedded resource. Until those are widened, the property test and the
fuzzer still cannot REACH the shape this plan fixed — the fixed-example fences are load-bearing on
their own for now. `115-13` also owns the SCHM-01 re-booking and the whole-phase
`make quality-gate`.

## Self-Check: PASSED

- Files claimed created/modified: all 5 source/contract/planning files verified present on disk.
- Commits claimed: `fdf236c8`, `a9af3a5d`, `60cda794` all resolve in `git log`.
- Symbols claimed: `fn first_legacy_dialect`, `v2_pin_still_enforces_an_embedded_legacy_resource`,
  `embedded schema resource` in the contract, `first_legacy_dialect` in `binding.yaml` — all found.
