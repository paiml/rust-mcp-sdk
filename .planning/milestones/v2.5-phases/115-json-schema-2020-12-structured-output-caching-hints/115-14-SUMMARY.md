---
phase: 115-json-schema-2020-12-structured-output-caching-hints
plan: 14
subsystem: server
tags: [schm-01, json-schema, draft-2020-12, gap-closure, position-aware-traversal, embedded-schema-resource, contract, vacuous-postcondition]

# Dependency graph
requires:
  - phase: 115-12
    provides: "the recursive (but position-BLIND) `normalize_schema_dialect`, `DATA_ONLY_KEYWORDS`, and the two walkers this plan corrects"
  - phase: 115-13
    provides: "the widened property/fuzz generators whose restated copies of the traversal rule 115-15 must now update"
  - phase: 115-03
    provides: "`compile_for_era` and the `fuzz_support` seam through which the defect was measured"
provides:
  - "`SUBSCHEMA_MAP_KEYWORDS` — position-aware traversal in BOTH walkers: the values of a properties / patternProperties / $defs / definitions / dependentSchemas map are schema positions regardless of the author-chosen name they are filed under"
  - "`v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` — the gate-visible fence, OBSERVED to fail against the position-blind body, asserting NAME position both behaviourally and structurally and KEYWORD position as its twin"
  - "`normalization_cases()` (f) `$defs.default` and (g) `properties.examples`, flowing through the structural fence and the idempotence fence automatically"
  - "`first_legacy_dialect_in_member` / `pin_dialect_in_member` — the member dispatch split out under CI's cognitive-complexity gate, bound in `contracts/binding.yaml`"
  - "An `output_schema_draft_pin` postcondition a CORRECT implementation can satisfy: a total over SCHEMA POSITIONS, with the term defined inline (WR-01 discharged)"
  - "Rustdoc that states the shipped scope rather than a wider one, and that names the postcondition as a detector/rewriter AGREEMENT check satisfied VACUOUSLY by the defect"
  - "`D-115-AC` (WR-03, with the OPEN MEASUREMENT that decides its fix shape), `D-115-AD` (WR-04/WR-05/IN-01/IN-02/IN-03), `D-115-AE` (the pmat criterion is fail-open twice over)"
affects: [115-15, 116, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A deny-list of KEYWORDS must never be tested against keys in NAME position. `$defs`/`properties`/`patternProperties`/`definitions`/`dependentSchemas` map author-chosen names to subschemas; filtering those keys against a keyword list is a category error, and it was a reachable validation bypass"
    - "A postcondition asserted through the DETECTOR half of the thing it checks is an AGREEMENT check, not an independence check — it is satisfied VACUOUSLY by any defect in the shared rule. Measured here: the blind detector reported None for documents the blind rewriter never touched"
    - "When the library happens to behave correctly on a defective input today, fence the variant STRUCTURALLY. A behavioural assertion on the `properties`-position collision passes against the defective code — a fence that cannot fire"
    - "Fence the INVERSE at the same time: the cheapest way to make a NAME-position fence pass is to delete the data guard, so the KEYWORD-position twin lives in the same test and is read in one pass"
    - "`pmat analyze complexity --max-cognitive 25` is WEAKER than the `pmat quality-gate` it is meant to predict (recommended 23 vs maximum 25). Budget 23 and run the gate binary"

key-files:
  created:
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/115-14-SUMMARY.md
  modified:
    - src/server/output_validation.rs
    - contracts/mcp-protocol-sdk-v1.yaml
    - contracts/binding.yaml
    - .planning/phases/115-json-schema-2020-12-structured-output-caching-hints/deferred-items.md

key-decisions:
  - "Implemented `115-VERIFICATION.md` missing-item 1 EXACTLY and nothing wider: a SUBSCHEMA_MAP_KEYWORDS list descended into unconditionally. WR-04's inverse design (allow-list only vocabulary-defined subschema positions) was declined and BOOKED — it moves in the opposite safety direction, because the current walk is deliberately a SUPERSET of what jsonschema honours"
  - "The `properties`-position collision is fenced STRUCTURALLY, not behaviourally: jsonschema 0.49.2 still enforces `type` there against the DEFECTIVE code, so a behavioural assertion would have been a fence that can never fire — the exact failure mode this closure exists to repair"
  - "A malformed subschema map (a `$defs` whose value is not an object) falls THROUGH to the ordinary walk rather than stopping the descent. CR-01's fix sketch omits this and silently loses coverage relative to the position-blind walk"
  - "The member dispatch was extracted into two helpers only AFTER measuring: inline, it put `pin_dialect_in_place` at cognitive 24 against `pmat quality-gate`'s threshold of 23, with the base commit at 0 violations. Both halves were split so they stay visibly mirror-image; no `#[allow]` was used"
  - "The two RESTATED copies of the traversal rule (`tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`) were deliberately NOT touched — they are 115-15's, and the window in which they carry the old rule is named in the shipped rustdoc"
  - "`.planning/REQUIREMENTS.md` was deliberately NOT edited (0-byte diff) and `requirements mark-complete` was NOT run: SCHM-01's re-booking is 115-15's Task 3, after the whole-phase gate has actually run. Booking ahead of measurement is ledger `D-115-G`, the process defect this requirement has now carried twice. `.planning/ROADMAP.md` carries plan-progress bookkeeping ONLY — the 115-14 checkbox and the 12/13 -> 14/15 row — and the Phase 115 marker stays `[~]`"

patterns-established:
  - "The negative control runs INSIDE the task, not across a commit boundary: the pre-commit hook forbids committing a red tree, so 'add the fence → observe it fail → fix → observe it pass' is one commit whose message records both counts"
  - "A criterion whose failure mode prints nothing on stdout is indistinguishable from its pass condition — run the real gate binary as the tie-breaker"

# Metrics
duration: ~40m
completed: 2026-08-01
tasks_completed: 2
files_modified: 4
---

# Phase 115 Plan 14: Position-Aware `$schema` Traversal Summary

Closed `115-VERIFICATION.md`'s BLOCKER: the recursive normalizer `115-12` shipped tested
`DATA_ONLY_KEYWORDS` against EVERY object key, so an `$id`-bearing embedded schema resource filed
under a `$defs` or `properties` entry an author had NAMED `const`/`enum`/`default`/`examples` was
visited by neither walker, and its legacy `$schema` survived the v2 pin. The walk is now
position-aware, the fence was observed to fail before the fix, the data guard is proven
un-regressed from the KEYWORD side, and the rustdoc and contract now state the scope the code
actually has.

## What Shipped

| Task | Name | Commit | Key files |
|---|---|---|---|
| 1 | Fence the colliding-name bypass, observe it fail, make both walkers position-aware | `f8692f1d` | `src/server/output_validation.rs` |
| 2 | Correct the contract postcondition, book WR-03/WR-04 | `07bfdd52` | `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml`, `deferred-items.md` |
| — | *(deviation)* `D-115-AE` — the pmat complexity criterion is fail-open twice over | `2bf4d637` | `deferred-items.md` |

## THE NEGATIVE CONTROL — recorded verbatim, because an unfired fence is not evidence

This is the third round on SCHM-01. The fences were written FIRST, run against the still-defective
walkers, and only then was the fix applied — inside one task, because the pre-commit hook forbids
committing a red tree.

**Step 2 run**, `$HOME/.cargo/bin/cargo test --lib --features full output_validation::tests --
--test-threads=1`, against the position-blind body:

```
test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 1793 filtered out
failures:
    server::output_validation::tests::normalize_schema_dialect_changes_only_dollar_schema_keys
    server::output_validation::tests::v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword
```

Exactly the two predicted failures, no more and no fewer. Their assertion text:

**(1)** `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`, panicking at
`src/server/output_validation.rs:925`:

> BYPASS ($defs.const): the v2 Draft 2020-12 pin accepted a STRING where the embedded schema
> resource declares `integer`. Measured before 115-14: `$defs.default` -> verdicts=(Conforms,
> Conforms), rewritten=false, against the control `$defs.Inner` -> (Conforms, Violates),
> rewritten=true. A `$defs` key is an AUTHOR-CHOSEN NAME, never a keyword, so DATA_ONLY_KEYWORDS
> must NOT be applied to it — the values of a $defs / properties / patternProperties / definitions
> / dependentSchemas map are schema positions REGARDLESS of the name they are filed under. See
> SUBSCHEMA_MAP_KEYWORDS.

(`$defs.const` is simply the first of the four colliding names in iteration order; the assertion is
per-name and the control `Inner` passes.)

**(2)** `normalize_schema_dialect_changes_only_dollar_schema_keys`, panicking at
`src/server/output_validation.rs:1315`:

> assertion `left == right` failed: borrow/own decision is wrong for
> `{"type":"object","properties":{"n":{"$ref":"#/$defs/default"}},"$defs":{"default":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}`
> — the no-op cases must allocate nothing
> ```
>   left: false
>  right: true
> ```

`left: false` is the measured `rewritten=false`: the normalizer returned `Cow::Borrowed`, so no
`tracing::warn!` fired either and the author got NO signal.

### The postcondition passed VACUOUSLY — measured, not argued

The plan required this observation because it is the evidence `115-15`'s WR-02 work builds on. The
purity postcondition `first_legacy_dialect(&normalized) == None` inside
`normalize_schema_dialect_changes_only_dollar_schema_keys` is a **detector/rewriter AGREEMENT**
check: both halves implement the same rule, so a defect IN the rule satisfies it.

The Step 2 run aborts at the borrow/own assertion, which precedes the postcondition, so the
postcondition was measured directly against the same pre-fix body with a throwaway probe (added,
run, removed before Step 3 — it is in no commit):

```
owned=false postcondition_none=true doc={"type":"object","properties":{"n":{"$ref":"#/$defs/default"}},"$defs":{"default":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}
owned=false postcondition_none=true doc={"type":"object","properties":{"examples":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}
```

`owned=false` — nothing was rewritten — and `postcondition_none=true` — the detector reported no
surviving legacy declaration — **for the very documents that still carried one.** That is a
postcondition satisfied vacuously by the defect it was written to catch, and it is why an
independently-TYPED walk restating the same RULE (115-13's fuzz invariant 5) could not see this
either. The independent instrument is the rename-invariance fence `115-15` adds. This is now
stated in the shipped rustdoc and in the contract invariant, so the next reader cannot mistake
agreement for independence.

## The fix

`SUBSCHEMA_MAP_KEYWORDS = ["properties", "patternProperties", "$defs", "definitions",
"dependentSchemas"]`, consulted FIRST in the member dispatch of both walkers, which is now a
three-way decision:

1. key in `SUBSCHEMA_MAP_KEYWORDS` **and** value is a JSON object → recurse into every VALUE of
   that object directly; the map's own keys are never keyword-filtered;
2. key in `SUBSCHEMA_MAP_KEYWORDS` but value is NOT an object (malformed) → fall through to the
   ordinary walk, so no coverage is lost relative to the position-blind version. `115-REVIEW.md`
   CR-01's sketch omits this and silently stops descending;
3. otherwise → today's behaviour: skip a `DATA_ONLY_KEYWORDS` member, recurse otherwise.

Both function signatures are byte-identical to `contracts/binding.yaml` (`D-115-F`):
`fn first_legacy_dialect(node: &Value) -> Option<&str>` and
`fn pin_dialect_in_place(node: &mut Value)`.

### Post-fix verdicts

`$HOME/.cargo/bin/cargo test --lib --features full output_validation::tests -- --test-threads=1`
→ **18 passed / 0 failed** (17 existing + 1 new).

| Document | v2 verdict for `{"n": "NOT-AN-INTEGER"}` | v2 verdict for `{"n": 7}` |
|---|---|---|
| `$defs.const` | **enforced** (mismatch reported) | conforms |
| `$defs.enum` | **enforced** | conforms |
| `$defs.default` | **enforced** — was `(Conforms, Conforms)`, `rewritten=false` | conforms |
| `$defs.examples` | **enforced** | conforms |
| `$defs.Inner` (control) | enforced, as before | conforms |

`properties.{const,enum,default,examples}` each now normalize to `Cow::Owned` with
`/properties/<name>/$schema == DRAFT_2020_12` — asserted structurally, deliberately: `jsonschema`
0.49.2 still enforces `type` under a `properties` entry carrying a surviving legacy declaration, so
a behavioural assertion there would have passed against the defective code.

`{"type":"object","<const|enum|default|examples>":{"$schema":DRAFT_07,"note":"data"}}` still comes
back `Cow::Borrowed` and byte-identical — the fix is a POSITION distinction, not a deleted data
guard. `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone` gained a
keyword-position `default` payload and an `examples` ARRAY payload for the same reason; both pass
before and after, and exist so a future edit that drops the guard reports why it broke.

### The one behaviour change, on a malformed shape

`{"properties": {"$schema": "http://json-schema.org/draft-07/schema#"}}` — a `properties` entry
NAMED `$schema` whose value is a string. The old walk descended into the `properties` map as though
the map were itself a schema, read the string as a declaration and rewrote it. Under the position
rule it is a NAME bound to a non-schema and is left alone. That is correct, and it is exactly why
the two restated copies of this rule MUST be updated by `115-15`: until they are, such an input
makes their surviving-declaration scan report a FALSE positive. This is stated in the shipped
rustdoc, not only here.

## Complexity: the extraction WAS needed, and the gate that caught it is not the one the plan named

Measured with pmat 3.15.0, three ways, in this order:

| Tree | `analyze complexity --max-cognitive 25`, violations under `src/` | `pmat quality-gate --fail-on-violation --checks complexity` |
|---|---|---|
| base (`fc674e40`) | none | **exit 0**, `Total violations: 0` |
| 115-14 with the dispatch INLINE | **none** | **exit 1** — `./src/server/output_validation.rs:218 - pin_dialect_in_place: cognitive-complexity - Cognitive complexity of 24 exceeds recommended complexity of 23` |
| 115-14 after the extraction | none | **exit 0**, `Total violations: 0` |

The base measurement was taken by restoring `git show HEAD:src/server/output_validation.rs` over
the working file, running the gate, and restoring from a `/bin/cp` snapshot verified with
`shasum -a 256 -c` (**OK**) — `git stash` and `git checkout --` were not used. It proves the +1 is
this change's own cost and not an inherited one.

So the plan's contingency fired: the dispatch was extracted into
`first_legacy_dialect_in_member` and `pin_dialect_in_member` (mutual recursion with their parents),
BOTH halves so they stay visibly mirror-image, and both are bound in `contracts/binding.yaml` with
`status: implemented`. **No `#[allow]` was used.**

**The `analyze complexity --max-cognitive 25` column is the story**: it reported CLEAN for a
function the PR-blocking gate FAILED on, because `quality-gate` fires at pmat's *recommended*
threshold of 23 while `--max-cognitive 25` sets the *maximum*. Booked as `D-115-AE`. Budget 23.

## Gate results

| Check | Result |
|---|---|
| `cargo test --lib --features full output_validation::tests -- --test-threads=1` | **18 passed / 0 failed** |
| `cargo test --lib --features "full fuzzing" output_validation::fuzz_support_tests` | **5 passed / 0 failed** |
| `cargo nextest -E 'binary(phase115_contract_bindings)'` | **5 passed** |
| `cargo nextest -E 'binary(v2_schema_tripwires)'` | **13 passed** |
| `cargo nextest -E 'binary(structured_tool_output)'` | **20 passed** (SCHM-02 unmoved) |
| `cargo nextest -E 'binary(v1_lists_golden)'` | **7 passed** — the v1 wire did not move |
| `cargo nextest --features "full fuzzing" -E 'binary(property_tests)'` | **19 passed** |
| `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | **exit 0** |
| `/usr/bin/make lint` | **exit 0**, zero warning/error lines |
| `cargo fmt --all -- --check` | **exit 0** |
| `pmat quality-gate --fail-on-violation --checks complexity` | **exit 0**, 0 violations |

Grep-shaped criteria: `SUBSCHEMA_MAP_KEYWORDS` appears **5** times in
`src/server/output_validation.rs` (≥3 required); the code-only count of
`validator_for|draft202012` is **2**, unchanged, so `VALIDATOR_SITES` stays accounted for;
`normalization_cases()` returns **7** entries; `author-chosen` appears **4** times in
`contracts/mcp-protocol-sdk-v1.yaml` (≥2); `115-14 POSITION CORRECTION` appears **5** times in
`contracts/binding.yaml` (3 corrections + the 2 new helper bindings, the plan's stated
extraction case); anchored `status: planned` returns **0**; the ledger's whole-ID duplicate check
returns nothing and `unowned` went 32 → 35.

**`UNCONDITIONALLY` survives twice, both inside sentences that name BOTH exceptions** — the module
bullet and the `# Why the walk is position-aware` section, each quoting the old false claim in
order to say why it was false. The word is never used to describe the shipped behaviour.

`git diff --stat` carries no `Cargo.toml` and no `Cargo.lock` — the dependency set is unchanged, so
no supply-chain review is triggered (T-115-SC closed by measurement).

**`make quality-gate` and the `+nightly` fuzz campaign were deliberately NOT run** — they are
`115-15`'s Task 3, once over the whole closure. `make lint` (the pedantic + nursery step that
caught `115-12`'s regression a plan late) DID run here, exit 0.

## The contract

`output_schema_draft_pin` carried three false statements about shipped code. All three are now
corrected in place, amend-not-delete:

- the `formula:` `walk:` clause SPECIFIED the defect; it now states the name-position rule;
- invariant 1 now says an embedded resource is reached whatever the name of the entry it is filed
  under, and carries the measured `$defs.default` row;
- the POSTCONDITION invariant added by `115-12` was an UNSCOPED total, false in **two independent
  ways** and unsatisfiable by any CORRECT implementation: WR-01's (a `$schema` inside a data-only
  payload is DATA and must SURVIVE) and this closure's (a colliding name). It is now a total over
  SCHEMA POSITIONS, with the term defined inside the invariant, both historical measurements kept,
  and the fences named — including the note that a restatement of the same rule catches only a
  detector/rewriter disagreement.

`lean_theorem:`, `domain:`/`codomain:` and the other two equations were not touched; SCHM-02 and
SCHM-03 stay VERIFIED.

## Deviations from Plan

### 1. [Rule 3 — blocking] The plan's literal signature for `first_legacy_dialect_in_member` does not compile

- **Found during:** Task 1 Step 3, complexity contingency
- **Issue:** the plan specifies
  `fn first_legacy_dialect_in_member(member_key: &str, member_value: &Value) -> Option<&str>`.
  With two input references and one borrowed output, lifetime elision is ambiguous and rustc
  rejects it.
- **Fix:** the output lifetime is tied explicitly to `member_value`:
  `fn first_legacy_dialect_in_member<'a>(member_key: &str, member_value: &'a Value) -> Option<&'a str>`.
  rustfmt then wraps the declaration across four lines at 100 columns, so `contracts/binding.yaml`
  records it as a YAML **block scalar** reproducing the source byte-for-byte rather than
  normalizing it onto one line — `D-115-F` says a recorded signature that drifts from source is
  invisible to the gate, and silently one-lining it would be exactly that drift. Both the
  lifetime deviation and the block-scalar spelling are stated in that binding's `notes:`.
- **Commits:** `f8692f1d`, `07bfdd52`

### 2. [Rule 2 — missing critical instrumentation] The plan's pmat acceptance criterion is fail-open twice over

- **Found during:** Task 1 Step 3 verification
- **Issue:** the criterion
  `pmat analyze complexity --format json --max-cognitive 25 | jq '.summary.violations[] | select(.path | startswith("src/"))'`
  (a) names a field that does not exist — it is `file`, with values prefixed `./`, so jq exits **5**
  with an error on **stderr** and prints **nothing on stdout**, which is the criterion's own pass
  condition; and (b) uses a threshold WEAKER than the PR-blocking gate, which fires at pmat's
  recommended 23. Run as written, the criterion PASSED on a tree whose real gate FAILED.
- **Fix:** ran both the corrected jq form and `pmat quality-gate --fail-on-violation --checks
  complexity` as the tie-breaker; the gate failure is what forced the member-helper extraction.
  Booked as `D-115-AE`, extending ledger entry `K`, which had only the `.summary.` half.
- **Commit:** `2bf4d637`

### 3. [scope, deliberate] Two review findings declined rather than fixed

WR-04's inverse design (descend only into vocabulary-DEFINED subschema positions) was declined:
it is wider than this closure's scope and moves in the opposite safety direction, since the current
walk is deliberately a SUPERSET of what `jsonschema` honours. WR-03's fragment-suffixed-URI
false positive was declined because its correct FIX SHAPE depends on an unmeasured library
behaviour — if `jsonschema` 0.49.2 resolves `…/2020-12/schema#` to an EMPTY vocabulary set, simply
declassifying the spelling REINTRODUCES the bypass this plan closed. Both are booked with reasons
and with an OPEN MEASUREMENT as `D-115-AC` / `D-115-AD`, unowned.

## What 115-15 inherits

- **The restated copies still carry the OLD rule.** `tests/property_tests.rs` and
  `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` both keyword-filter every map key. With the fix
  landed, an input shaped `{"properties": {"$schema": "http://json-schema.org/draft-07/schema#"}}`
  makes the fuzz target's invariant-2 strip report a FALSE positive. **Do not run the fuzzer to
  judge this plan** — the window is known and named in the shipped rustdoc.
- **WR-02's instrument.** The vacuous-postcondition measurement above is the evidence: a
  differently-typed walk restating the same rule catches only a disagreement. The rename-invariance
  property (renaming a `$defs` key must not change the normalized document apart from that key) is
  the instrument that catches a rule defect.
- **SCHM-01's booking.** `.planning/REQUIREMENTS.md` is **UNTOUCHED — a 0-byte diff** — and
  `requirements mark-complete` was deliberately NOT run. Booking ahead of the whole-phase gate is
  ledger `D-115-G`, and this requirement has now carried that defect twice. 115-15's Task 3 owns
  the re-booking, AFTER `make quality-gate` has actually run over the closure.
  `.planning/ROADMAP.md` was touched for **plan-progress bookkeeping ONLY** (`roadmap
  update-plan-progress 115`): the `115-14-PLAN.md` checkbox and the progress-table row
  `12/13 → 14/15`, a two-line diff. **The Phase 115 marker stays `[~]`** and no requirement
  statement was edited — that is the distinction the plan's "do not edit ROADMAP" instruction is
  protecting, and it holds.

## Self-Check: PASSED

- `src/server/output_validation.rs` — FOUND, contains `SUBSCHEMA_MAP_KEYWORDS` (5×) and
  `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`
- `contracts/mcp-protocol-sdk-v1.yaml` — FOUND, contains `author-chosen` (4×), parses as YAML,
  7 invariants on `output_schema_draft_pin`
- `contracts/binding.yaml` — FOUND, contains `115-14 POSITION CORRECTION` (5×), parses as YAML,
  10 `output_schema_draft_pin` bindings, 0 `planned`
- `.planning/phases/115-.../deferred-items.md` — FOUND, `D-115-AC`/`D-115-AD`/`D-115-AE` present,
  whole-ID duplicate check returns nothing
- Commits `f8692f1d`, `07bfdd52`, `2bf4d637` — all FOUND in `git log`
