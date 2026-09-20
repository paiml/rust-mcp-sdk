---
phase: 115-json-schema-2020-12-structured-output-caching-hints
reviewed: 2026-08-02T18:35:02Z
depth: standard
scope: gap-closure round 4 (plans 115-16..115-19) — NOT full-phase coverage
diff_base: f9fad51c
head: a7c31fbf
supersedes_note: >-
  This file replaces the gap-closure round-3 review (plans 115-14 and 115-15) at
  the same path. That pass is preserved in git history at 1b286b22; its findings
  (CR-01, WR-01..WR-07, IN-01..IN-03) are NOT re-litigated here. Where a round-3
  finding is only PARTIALLY closed by round 4, the residue is booked below as a
  NEW finding with the round-3 ID named for traceability (see IN-02, which is
  round-3 IN-03's residue, and WR-03, which is round-3 WR-02's shape one position
  class over). Round 3 in turn replaced round 2 (695a7123 / c478e75a lineage).
files_reviewed: 8
files_reviewed_list:
  - src/server/output_validation.rs
  - tests/keyword_list_mirrors.rs
  - tests/property_tests.rs
  - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
  - fuzz/corpus/fuzz_schema_draft_pin/15_dependencies_named_default
  - fuzz/corpus/fuzz_schema_draft_pin/README.md
  - contracts/mcp-protocol-sdk-v1.yaml
  - contracts/binding.yaml
findings:
  critical: 1
  warning: 6
  info: 4
  total: 11
status: issues_found
---

# Phase 115: Code Review Report — gap-closure round 4

**Reviewed:** 2026-08-02T18:35:02Z
**Depth:** standard
**Diff base:** `f9fad51c..a7c31fbf`
**Files Reviewed:** 8
**Status:** issues_found

## Scope

Plans 115-16 through 115-19 only. This is NOT a full-phase review. Round-3
findings are not re-litigated except where round 4 closed them only partially.

The round's stated purpose was to close round-3 `CR-01` (`SUBSCHEMA_MAP_KEYWORDS`
omitted `dependencies`) by DERIVATION rather than by patching, and to make the
defending instruments observable. Both halves were judged.

## Summary

**The six-keyword derivation is CORRECT and COMPLETE, and I re-ran it offline
rather than taking it on trust.** Over the nineteen meta-schema documents
`jsonschema` 0.49.2 ships in `metaschemas/`, the keywords whose `.properties`
entry is an object binding `additionalProperties` to a self-reference are exactly
`{properties, patternProperties, $defs, definitions, dependentSchemas,
dependencies}`, with `$vocabulary` (boolean values) and `dependentRequired`
(string-array values) rejected by that same criterion. There is no seventh
omission at the map-position class. The `dependencies` addition is behaviourally
sound: I traced every shape it changes and the only behaviour change is on the
malformed `{"dependencies": {"$schema": "<string>"}}` document, which the module
rustdoc already names.

**`tests/keyword_list_mirrors.rs`'s two-assertion design is sound in the specific
way the scope note asked about.** `EXPECTED_SUBSCHEMA_MAP_KEYWORDS` is a literal
in the gate file, sourced from none of the three copies; the file imports nothing
from the crate; the extractor is fail-CLOSED (exactly-one-definition guard,
non-empty guard, comment-stripping that survives the trailing `dependencies`
comment). All 25 `output_validation` unit tests, both drift-gate tests, all 21
property tests and a `cargo check` of the excluded `fuzz/` crate pass on this
tree.

**But three things are wrong, and the first ships.**

1. `tests/keyword_list_mirrors.rs` is PACKAGED into the published crate while the
   `fuzz/` tree it reads at runtime is EXCLUDED from it — so `cargo test` on the
   published `pmcp` tarball panics. The repo has already hit this exact failure
   twice and excluded two other tests for it, with comments in `Cargo.toml`
   stating the rule verbatim. Measured with `cargo package --list`. (CR-01)
2. The round's PRIMARY fence — the one the contract, three rustdocs and the plan
   all name as the instrument for a list omission — carries an anti-vacuity
   assertion that is a **tautology** and can never fail, and draws its NAME axis
   from a crate-derived list while its rustdoc's independence claim covers only
   the CONTAINER axis. (WR-01, WR-02)
3. The round derived ONE position class (maps) and then wrote its scope
   statements as if that class plus ordinary member descent were the whole rule.
   **Array descent — `allOf` / `anyOf` / `oneOf` / `prefixItems`, the commonest
   carrier of an embedded schema resource — is implemented in both walkers,
   absent from the contract's `SCHEMA POSITION` definition, and exercised by no
   test, no property draw and no corpus seed.** Deleting both `Value::Array` arms
   passes the entire suite. That is round-3 `WR-02`'s shape one position class
   over, and the derivation criterion the round adopted structurally cannot find
   it. (WR-03)

The trap the scope note asked about — "a fence's reachability must not derive
from the same artifact as the rule it checks" — **survives, on the NAME axis of
the `src/` fence** (WR-01) and in the `make quality-gate`-invisibility of the one
guard that pins `CONTAINER_DRAW` (WR-05). It does NOT survive in
`tests/property_tests.rs`, where 115-17 handled it correctly with an own literal
plus a superset guard; `src/` simply did not copy that pattern.

No assertion in this round depends on a v2 verdict flip that cannot happen: I
checked every new assertion, and all of them are structural (`Cow` borrow/own
plus a rewritten pointer) exactly as the scope note requires.

---

## Critical Issues

### CR-01: `tests/keyword_list_mirrors.rs` ships in the published crate but reads `fuzz/`, which does not — `cargo test` on the published tarball panics

**File:** `tests/keyword_list_mirrors.rs:79`, `:149-167`; `Cargo.toml:15-53`

**Issue:**

The new drift gate reads three repository files at RUNTIME and hard-panics when
any is missing:

```rust
const FUZZ_FILE: &str = "fuzz/fuzz_targets/fuzz_schema_draft_pin.rs";

fn read(relative: &str) -> String {
    let full = repo_root().join(relative);          // repo_root() = env!("CARGO_MANIFEST_DIR")
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("cannot read {relative}: {e}\n…"))
}
```

`Cargo.toml`'s package `exclude` array contains `"fuzz/"` (line 20). It does NOT
contain `tests/keyword_list_mirrors.rs`. Measured on this tree:

```
$ cargo package --list --allow-dirty | grep -E '^(tests/(keyword_list_mirrors|property_tests)\.rs|fuzz/)'
tests/keyword_list_mirrors.rs
tests/property_tests.rs
```

`fuzz/` contributes zero entries. So the published `pmcp` crate ships an
integration test that reads a path guaranteed to be absent, and both of its tests
panic:

```
cannot read fuzz/fuzz_targets/fuzz_schema_draft_pin.rs: No such file or directory (os error 2)
```

This breaks `cargo test` for every downstream consumer who builds from the
crates.io tarball — distro packagers (Debian/Fedora rust packaging run the test
suite from the tarball) and `cargo vendor` flows most directly. Nothing catches
it locally or in CI: `cargo package`'s verify step BUILDS the package, it does
not RUN its tests, and every in-repo gate runs from a tree that has `fuzz/`.

**This repository already knows the rule and has applied it twice.** The very
same `exclude` array carries two entries whose comments state the defect verbatim:

```toml
    "contracts/",
    # Reads contracts/team-servers-v1.yaml at runtime; contracts/ is excluded
    # above (crates.io size limit), so shipping this test would break a
    # downstream `cargo test` on the published crate — keep it out too.
    "tests/team_contracts_conformance.rs",
    # Same reason: reads contracts/binding.yaml and
    # contracts/mcp-protocol-sdk-v1.yaml at runtime and panics when they are
    # absent, which is exactly what a published-crate `cargo test` would hit.
    "tests/phase115_contract_bindings.rs",
```

The new file's own rustdoc (`:143-147`) says it is written in the "same shape as
`tests/phase115_contract_bindings.rs`, so there is one convention in this
repository for 'an integration test that reads repository source files', not
two." It copied the shape and missed the one packaging step the convention
requires.

**Fix** — add the exclusion, with the same comment form so the next reader sees
the rule rather than a bare path:

```toml
    # Reads src/server/output_validation.rs, tests/property_tests.rs AND
    # fuzz/fuzz_targets/fuzz_schema_draft_pin.rs at runtime and panics when any
    # is absent; `fuzz/` is excluded above, so shipping this test would break a
    # downstream `cargo test` on the published crate — keep it out too.
    "tests/keyword_list_mirrors.rs",
```

Do NOT "fix" this by making `read()` tolerant of a missing file: a gate that
silently skips the one copy no other instrument can see is the fail-open shape
`D-115-AE` records, and the file's own `extract_list` guards already reject that
pattern for the empty-list case. Excluding the file from the package is the
correct fix — the gate is a repository-internal instrument, not crate content.

A follow-up worth doing in the same edit: add a one-line acceptance criterion
that `cargo package --list | grep -c 'tests/keyword_list_mirrors.rs'` returns 0,
so the third occurrence of this class is caught by a criterion rather than by a
fourth review.

---

## Warnings

### WR-01: The anti-vacuity assertion in the round's primary fence is a tautology, and the fence's NAME axis is drawn from a crate-derived list

**File:** `src/server/output_validation.rs:1396-1425` (assertion at `:1418-1425`,
name draw at `:1400`)

**Issue:**

```rust
let mut examined = 0usize;
for container in containers {
    for &name in DATA_ONLY_KEYWORDS {
        examined += 1;
        …
    }
}

// Anti-vacuity: the grid was actually swept.
assert_eq!(
    examined,
    containers.len() * DATA_ONLY_KEYWORDS.len(),
    …
);
```

`examined` is incremented exactly `containers.len() * DATA_ONLY_KEYWORDS.len()`
times by construction, and the assertion recomputes that same product from the
same two slices. **It cannot fail for any value of either list.** With
`DATA_ONLY_KEYWORDS` empty the loop body never runs, `examined == 0`,
`0 == 6 * 0` passes, `violations` is empty, the membership guard passes, and the
fence reports green having examined ZERO (container, name) pairs — while its
comment says "the grid was actually swept."

This is the exact pattern the round's own new file names and defends against one
directory over (`tests/keyword_list_mirrors.rs:212-219`: *"a criterion whose
failure mode is indistinguishable from its success condition verifies
nothing"*, citing `D-115-AE` and `D-115-AA`). The lesson was applied to
`extract_list`'s non-empty guard and not to the fence that the contract
(`mcp-protocol-sdk-v1.yaml:382-388`), the module rustdoc (`:527-530`), the
property module (`tests/property_tests.rs:1083-1088`) and the fuzz target
(`:371-379`) all name as the PRIMARY instrument for a keyword-list omission.

Second, subtler half: the fence's rustdoc (`:1349-1358`) makes an independence
claim that covers only ONE of the two axes — *"The container list below is
deliberately NOT `SUBSCHEMA_MAP_KEYWORDS` … a fence parameterised by the list
whose incompleteness is the defect cannot fire on that defect."* The NAME axis
is `for &name in DATA_ONLY_KEYWORDS`, i.e. parameterised by the other half of the
very rule under test, and the rustdoc is silent about it. The coupling happens to
be principled today (the colliding-name set IS the data-only set), but the claim
as written is broader than the code, which is the failure mode this phase has
shipped three times.

**Fix** — make the guard falsifiable and state the axis honestly:

```rust
// Anti-vacuity: the grid was actually swept. The expected count is a LITERAL,
// not a product of the two slices being iterated — recomputing the loop bounds
// from the loop bounds cannot fail, and `DATA_ONLY_KEYWORDS` emptied would then
// report a zero-pair sweep as a pass (115-REVIEW.md round-4 WR-01).
assert_eq!(
    examined, 24,
    "expected 6 spec-defined containers x 4 data-only names = 24 probes, swept {examined}. \
     If the NAME axis (DATA_ONLY_KEYWORDS) changed, update this literal deliberately — the \
     names swept here are the colliding-name set, which is what makes this fence a fence."
);
```

### WR-02: The same fence's own `containers` literal has no completeness guard, so it can silently lag the shipped list

**File:** `src/server/output_validation.rs:1386-1394`, `:1444-1453`

**Issue:** The fence carries its own six-element `containers` literal (correct,
and the reason it can fire on an omission FROM `SUBSCHEMA_MAP_KEYWORDS`). The
price of an own literal is drift, and the only thing paid against it is a
single-keyword membership assertion:

```rust
assert!(SUBSCHEMA_MAP_KEYWORDS.contains(&"dependencies"), …);
```

That pins one keyword. It says nothing about the other five, and nothing at all
about a seventh. If a future round adds a keyword to `SUBSCHEMA_MAP_KEYWORDS`
(and to the three copies the drift gate pins, and to `EXPECTED_*`, and to
`CONTAINER_DRAW`, all of which have gates), this fence's literal stays at six,
never probes the new position, and reports green — the one instrument billed as
immune to the list's incompleteness silently stops covering the newest entry.

`tests/property_tests.rs` faced the identical tension for `CONTAINER_DRAW` and
solved it correctly (`:1126-1150`): an own literal for the DRAW, plus a SUPERSET
guard asserted separately and LAST, deliberately not an equality so the
both-blind negative control still works. `src/` did not copy that half.

**Fix** — replace the single-keyword guard with the superset form, keeping it
separate from the sweep exactly as the property module does:

```rust
// The completeness guard, asserted SEPARATELY from the sweep above so the sweep
// stays independent of the list it is checking. SUPERSET, not equality: in the
// both-blind negative control the shipped list is deliberately SHORTER, and an
// equality would fail there and mask the result that control exists to produce.
let unprobed: Vec<&&str> = SUBSCHEMA_MAP_KEYWORDS
    .iter()
    .filter(|shipped| !containers.contains(shipped))
    .collect();
assert!(
    unprobed.is_empty(),
    "{unprobed:?} are SHIPPED subschema-map keywords this fence never probes, so an embedded \
     resource filed there is covered by nothing here. Add them to `containers` — and do NOT \
     source `containers` from SUBSCHEMA_MAP_KEYWORDS, which would make the sweep unable to fire \
     on an omission from that list (115-REVIEW.md CR-01, D-115-AI(4))."
);
```

### WR-03: Array descent is a schema-position class the contract does not define and nothing exercises — deleting both `Value::Array` arms passes the whole suite

**File:** `src/server/output_validation.rs:265`, `:325`, `:1736-1808`;
`contracts/mcp-protocol-sdk-v1.yaml:254-261`, `:324-329`;
`contracts/binding.yaml:509-516`, `:560-565`, `:599-604`

**Issue:** Both walkers descend into arrays:

```rust
Value::Array(items) => items.iter().find_map(first_legacy_dialect),      // detector, :265
Value::Array(items) => items.iter_mut().for_each(pin_dialect_in_place),  // rewriter, :325
```

That branch is what reaches an `$id`-bearing embedded schema resource carried by
`allOf` / `anyOf` / `oneOf` / `prefixItems` — the commonest way a real schema
carries subschemas, and a strictly larger position class than
`patternProperties` and `dependentSchemas`, whose non-exercise was round-3
`WR-02`. Two problems, both measured:

**(a) The contract does not define it as a schema position.** 115-19 rewrote the
equation head, the `walk:` clause and three `binding.yaml` note heads and
defined `SCHEMA POSITION` as:

> the root, plus every node reached by descending into every member value EXCEPT
> a `const` / `enum` / `default` / `examples` payload, plus every VALUE of a
> `properties` / … / `dependencies` map

An array has no members. Descending into the member value `allOf` reaches the
ARRAY node and the definition terminates there — so under the contract as
written, the elements of an `allOf` array are NOT schema positions, and the
`POSTCONDITION` invariant (`:324-329`) says nothing about a legacy `$schema`
surviving inside one. An implementation that dropped array descent would satisfy
the contract while re-opening the vacuous-validator bypass for every
`allOf`-borne embedded resource. This is round-3 `WR-04`'s shape — a scope
statement that does not match the code — reproduced in the very sentences 115-19
rewrote to close `WR-04`.

**(b) No instrument exercises it.** Measured by exhaustive absence across every
file in scope plus the tracked seed corpus:

```
$ grep -rn 'allOf\|anyOf\|oneOf\|prefixItems' src/server/output_validation.rs \
      tests/property_tests.rs fuzz/fuzz_targets/fuzz_schema_draft_pin.rs \
      tests/keyword_list_mirrors.rs
# only prose matches (the `anyOf` branch of the derivation criterion, and
# seed 07's array-form `items` COMPILE-error row) — no document, no fixture
$ for f in $(git ls-files fuzz/corpus/fuzz_schema_draft_pin/ | grep '/[0-9][0-9]_'); do
      grep -lE 'allOf|anyOf|oneOf|prefixItems' "$f"; done
# (no output — 15 tracked seeds, zero hits)
```

`normalization_cases()` has eleven rows (a)..(k) and not one puts a `$schema`
inside an array in schema position. The property generator injects `$schema` only
at the root and at `container/name`, and `arb_json()` structurally cannot emit a
`$schema` key at all — so the generated space cannot contain the shape either.
The only array in any fixture is `dollar_schema_inside_examples`
(`:1952-1955`), which asserts the walk does NOT reach into an array under a
data-only keyword — the negative case, never the positive one.

Consequence: deleting both `Value::Array` arms leaves the entire suite green.
That is exactly the `patternProperties` / `dependentSchemas` situation round-3
`WR-02` reported, one position class over, and the round's derivation criterion
(objects whose `additionalProperties` self-references) structurally cannot find
it, because array-of-subschema keywords bind `items` / `$ref`-to-self, not
`additionalProperties`.

**Fix** — two edits, both small:

1. Add a positive row to `normalization_cases()` so the branch is fenced:

```rust
// (l) ARRAY position. `allOf` / `anyOf` / `oneOf` / `prefixItems` carry
// subschemas in an ARRAY, and both walkers reach them through their
// `Value::Array` arm (:265, :325). That arm was exercised by no test, no
// property draw and no corpus seed until this row — deleting it passed the
// whole suite (115-REVIEW.md round-4 WR-03).
(
    json!({
        "type": "object",
        "allOf": [{
            "$id": "https://example.test/inner",
            "$schema": DRAFT_07,
            "type": "integer"
        }]
    }),
    true,
),
```

2. Extend the contract's `walk:` clause with the rule the module rustdoc already
   states as rule 4, and mirror it into the `SCHEMA POSITION` sentence of the
   `POSTCONDITION` invariant and the three `binding.yaml` note heads:

```
              at an ARRAY node, every ELEMENT is a schema position and is
              descended into (allOf / anyOf / oneOf / prefixItems carry
              subschemas in arrays); the data-only exclusion still applies at the
              MEMBER that introduced the array, so an `enum` or `examples` array
              is never entered
```

### WR-04: The derivation every copy is anchored to is under-specified in all five shipped statements — followed literally it yields FOUR keywords, not six

**File:** `tests/keyword_list_mirrors.rs:86-118` and `:300-314`;
`src/server/output_validation.rs:190-227`;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:309-323`;
`tests/property_tests.rs:1022-1041`;
`contracts/mcp-protocol-sdk-v1.yaml:375-381`

**Issue:** All five shipped copies state the derivation the same way:

> the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09 / 2020-12
> meta-schema documents `jsonschema` 0.49.2 ships offline, of the keywords each
> meta-schema's own `.properties` map binds to an OBJECT-typed schema carrying an
> `additionalProperties` that REFERENCES THE META-SCHEMA ITSELF

with a `jq` one-liner over `<metaschema.json>`. Run exactly that over the five
named documents (measured against
`~/.cargo/registry/src/*/jsonschema-0.49.2/metaschemas/`):

| document | keywords produced |
|---|---|
| `draft4.json` | `definitions`, `properties`, `patternProperties`, `dependencies` |
| `draft6.json` | same four |
| `draft7.json` | same four |
| `draft2019-09/schema.json` | `definitions`, `dependencies` |
| `draft2020-12/schema.json` | `definitions`, `dependencies` |
| **UNION** | **FOUR: `definitions`, `properties`, `patternProperties`, `dependencies`** |

`$defs` and `dependentSchemas` do not appear, and NEITHER of the two keywords the
rustdocs name as "REJECTED by the self-reference criterion" appears either —
because `$defs` and `$vocabulary` live in `draft20{19-09,20-12}/meta/core.json`,
`properties` / `patternProperties` / `dependentSchemas` in
`…/meta/applicator.json`, and `dependentRequired` in `…/meta/validation.json`.
The 2019-09 and 2020-12 top-level `schema.json` files are `allOf` shells that
declare almost no applicator keywords at all. The sweep must include nine
`meta/*.json` vocabulary documents, and no shipped copy says so.

The full, correct per-file table IS recorded — but only in
`115-16-SUMMARY.md § THE DERIVATION`, a planning artifact, which does say *"which
is why the sweep has to include `meta/*.json` and not just the two roots."*

Why this matters rather than being pedantry: `tests/keyword_list_mirrors.rs`'s
ASSERTION 2 is the only instrument that can see a lockstep removal, and its
failure message (`:310-313`) routes the maintainer straight into the
under-specified copy:

> WHAT TO DO: re-run the meta-schema derivation documented on
> `EXPECTED_SUBSCHEMA_MAP_KEYWORDS` **in this file** and change the expectation
> only if the derivation itself produces a different union.

A maintainer who follows that instruction gets four keywords, concludes the
derivation "produces a different union", and deletes `$defs` and
`dependentSchemas` from the expectation and then from all three copies — which
is precisely the WR-01 lockstep-removal mode the gate exists to prevent, executed
under the gate's own instructions.

**Fix** — one sentence in each of the five copies, and the `jq` invocation
corrected in the two that carry it:

```text
/// The sweep is over NINETEEN documents, not five: for 2019-09 and 2020-12 the
/// applicator, core and validation VOCABULARY documents under `meta/` are where
/// the keywords live — `$defs` and `$vocabulary` in `meta/core.json`,
/// `properties` / `patternProperties` / `dependentSchemas` in
/// `meta/applicator.json`, `dependentRequired` in `meta/validation.json`. The
/// two root `schema.json` files are `allOf` shells and yield only `definitions`
/// and `dependencies`. Running the command below over the five DRAFT roots alone
/// produces FOUR keywords and neither rejected keyword — do not stop there.
///
/// ```text
/// for f in draft4.json draft6.json draft7.json \
///          draft2019-09/schema.json draft2019-09/meta/*.json \
///          draft2020-12/schema.json draft2020-12/meta/*.json; do
///   jq -r '(.properties // {}) | to_entries[]
///          | select((.value|type)=="object")
///          | select(.value|has("additionalProperties"))
///          | "\(.key)\t\(.value.additionalProperties|tojson)"' "$f"
/// done | sort -u
/// ```
```

(The guarded `select((.value|type)=="object")` note is accurate and should stay —
I reproduced the unguarded form exiting 5 on `draft7.json`, which binds
`.properties.default` and `.properties.const` to `true`.)

### WR-05: The only guard pinning `CONTAINER_DRAW` to the shipped list does not run under `make quality-gate`, and the gate that does run cannot see it

**File:** `tests/property_tests.rs:953`, `:1126-1150`, `:1246-1253`;
`tests/keyword_list_mirrors.rs:82`; `Makefile:296-299`, `:361`

**Issue:** 115-17 introduced a FOURTH literal, `CONTAINER_DRAW`, for a measured
and correct reason (`D-115-AI(4)`: sourcing the draw from the mirror makes every
negative control go green). The drift risk that creates is paid for by exactly
one assertion — the `undrawable` superset guard inside
`keyword_lists_mirror_the_shipped_ones`.

That test lives in a module gated
`#[cfg(all(test, feature = "fuzzing", feature = "validation"))]`, and `fuzzing`
is in neither `default` nor `full`. Trace the mandated local gate:

- `make quality-gate` → `test-all` → `test-unit` (`--lib --features full`),
  `test-doc`, `test-property` (`--features "full" -- --ignored property_` — note
  BOTH the missing `fuzzing` and the `--ignored` filter), `test-examples`,
  `test-integration` (`cargo test --test '*' --features "full"`).
- None of those enables `fuzzing`, so the whole
  `schema_dialect_normalization_properties` module — the mirror equality test,
  the `undrawable` guard, and both properties — is not compiled and does not run.

It runs only in CI (`.github/workflows/ci.yml:93`, `cargo test --all-features`).
Meanwhile the gate that DOES run locally, `tests/keyword_list_mirrors.rs`, reads
`tests/property_tests.rs` as text but extracts only `SUBSCHEMA_MAP_KEYWORDS` and
`DATA_ONLY_KEYWORDS` — `CONTAINER_DRAW` (`:1246`) and the `src/` fence's
`containers` literal (`src/server/output_validation.rs:1387`) are outside its
`COPIES` list entirely.

`CLAUDE.md` states `make quality-gate` is mandatory before every commit and PR.
So of the six literal copies of this list in the tree, the mandated local gate
covers three; CI covers a fourth; and two — the two that exist specifically to
be independent — are covered locally by nothing.

**Fix** — the drift gate is featureless and already reads both files as text.
Extend it rather than adding a sixth mechanism:

```rust
/// The two INDEPENDENT literals — `CONTAINER_DRAW` (tests/property_tests.rs) and
/// the `src/` fence's own container list. Each exists to be independent of the
/// shipped constant, so neither may be an equality; each must be a SUPERSET of
/// the derivation-anchored expectation, or the fence that reads it silently
/// stops probing a shipped position.
#[test]
fn independent_container_literals_cover_every_derived_keyword() {
    for (path, name) in [
        (PROPERTY_FILE, "CONTAINER_DRAW"),
        (SRC_FILE, "SPEC_DEFINED_SUBSCHEMA_MAP_CONTAINERS"),
    ] {
        let drawn = extract_list(&read(path), path, name);
        let missing: Vec<&&str> = EXPECTED_SUBSCHEMA_MAP_KEYWORDS
            .iter()
            .filter(|k| !drawn.iter().any(|d| d == *k))
            .collect();
        assert!(missing.is_empty(), "{name} in {path} cannot reach {missing:?} …");
    }
}
```

This requires promoting the `src/` fence's `let containers = [...]` to a
module-level `const SPEC_DEFINED_SUBSCHEMA_MAP_CONTAINERS: &[&str] = &[…];` so
the text extractor's marker can find it — a one-line change that also makes
WR-02's superset guard read naturally.

### WR-06: `strip_every_dollar_schema` is the one restatement left position-BLIND, inside the gate-visible surgical-scope fence

**File:** `src/server/output_validation.rs:1816-1827`, used at `:1866-1876`

**Issue:** 115-17 and 115-18 made the property-test and fuzz-target strippers
position-aware, and both rustdocs now argue at length that the position-aware
form buys SENSITIVITY: *"were the shipped walk ever to over-reach into NAME
position, the position-aware strip leaves the differing `$schema` in place on
both sides and the surgical-scope assertion FIRES, whereas a blind strip would
delete the difference from both sides and mask it"*
(`tests/property_tests.rs:1013-1020`; same argument at
`fuzz_schema_draft_pin.rs:293-300`).

`src/`'s own copy was not updated and remains fully blind:

```rust
fn strip_every_dollar_schema(node: &mut Value) {
    match node {
        Value::Object(map) => {
            map.remove("$schema");                       // unconditional: value kind ignored
            for value in map.values_mut() { strip_every_dollar_schema(value); }  // no DATA_ONLY skip
        },
        Value::Array(items) => items.iter_mut().for_each(strip_every_dollar_schema),
        _ => {},
    }
}
```

It is applied to both sides of the `expected_owned == true` branch of
`normalize_schema_dialect_changes_only_dollar_schema_keys` — the gate-visible
surgical-scope fence. By the same argument the other two files now make, it
MASKS, for every Owned case, a normalizer that over-reached into a nested
`const` / `enum` / `default` / `examples` payload or that corrupted a nested
`properties` entry named `$schema` (it deletes the subschema outright, since it
does not check the value kind). The `mixed` document in
`normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone`
(`:1979-2007`) covers both positions, but only at the ROOT.

The residual risk is bounded — an edit that deleted `DATA_ONLY_KEYWORDS` wholesale
still trips the Borrowed-case assertions — but the inconsistency is exactly the
kind this round exists to remove: three restatements of one rule, two corrected
with a stated rationale and the third left behind.

**Fix** — mirror `pin_dialect_in_member`, as the other two strippers now do:

```rust
fn strip_every_dollar_schema(node: &mut Value) {
    match node {
        Value::Object(map) => {
            if map.get("$schema").is_some_and(Value::is_string) {
                map.remove("$schema");
            }
            for (key, value) in map.iter_mut() {
                strip_every_dollar_schema_in_member(key, value);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(strip_every_dollar_schema),
        _ => {},
    }
}

fn strip_every_dollar_schema_in_member(member_key: &str, member_value: &mut Value) {
    if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
        match member_value {
            Value::Object(named) => named.values_mut().for_each(strip_every_dollar_schema),
            malformed => strip_every_dollar_schema(malformed),
        }
    } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
        strip_every_dollar_schema(member_value);
    }
}
```

---

## Info

### IN-01: Assertion 1 of the drift gate is logically subsumed by assertion 2

**File:** `tests/keyword_list_mirrors.rs:25-27`, `:270-315`

**Issue:** The module rustdoc presents the two assertions as covering different
modes — *"Mode 1 is caught by comparing the three copies to each other. Mode 2 is
caught only by comparing them to something none of them is"*. Assertion 2
compares EVERY copy to `expected` (`for (path, list) in &extracted`), so any
copy-to-copy disagreement necessarily makes at least one copy differ from
`expected` too. Assertion 1 therefore cannot fire on anything assertion 2 misses;
its real value is a more targeted failure message naming the two drifted files.

Not a defect — but the "two instruments" framing overstates the independence
count, and the scope note's question ("do the two assertions collapse into one?")
has the answer "yes for detection, no for diagnosis". Worth stating that way in
the rustdoc so the next reader does not budget two guarantees where there is one.

**Fix:** reword to "Assertion 1 exists for the failure MESSAGE — it names the two
files that disagree. Assertion 2 is the one that detects both modes."

### IN-02 (residue of round-3 IN-03): `disambiguate()` was not fixed, and 115-17 doubled its blast radius

**File:** `tests/property_tests.rs:1388-1396`, used at `:1429` and `:1472`

**Issue:** Round-3 `IN-03` reported that `disambiguate()` maps a drawn name `"n"`
to `"n_resource"` unconditionally, while the collision it guards exists only when
`container == "properties"` (the only case where `embed_resource` puts the
resource and the `$ref` holder in the same map, `:1359-1371`). It was not fixed.

115-17 then widened `arb_container()` from three containers to six, so the
narrowing now applies to FIVE containers where no collision is possible instead
of two. The sharpest case is the one this round added: `dependencies` keys are
INSTANCE PROPERTY NAMES, and `"n"` is precisely the instance property the
generated `$ref` holder uses — making `dependencies: {"n": …}` the most
realistic entry name in that container, and the one the generated space now
cannot produce.

**Fix:** the round-3 suggestion still applies verbatim —

```rust
fn disambiguate(container: &str, name: String) -> String {
    if container == "properties" && name == "n" {
        "n_resource".to_string()
    } else {
        name
    }
}
```

### IN-03: `cache_key = (Era, canonical_json_text(schema))` is not canonical — `preserve_order` is enabled

**File:** `src/server/output_validation.rs:614-650` (key built at `:639`);
`contracts/mcp-protocol-sdk-v1.yaml:266`

**Issue:** The cache key is `schema.to_string()`, and `Cargo.toml:58` enables
`serde_json`'s `preserve_order`, so `Value::Object` is insertion-ordered and
`to_string()` is order-DEPENDENT. Two schemas that compare `==` (serde_json's
`Map` equality is order-insensitive) but were built with different key order
produce two distinct cache entries. The contract calls this
`canonical_json_text` and the rustdoc calls it "canonical schema text"; neither
is accurate.

No correctness impact — the duplicate entries hold identical validators, and the
era half of the key is what the invariant actually depends on. But it does mean
the "bounded by the number of distinct DECLARED schemas" bound
(`:743-746`, and the fuzz seam's justification for using the uncached path) is
bounded by distinct schema TEXTS, which is a larger set. Worth naming so the
uncached-fuzz-path rationale is not read as stronger than it is.

**Fix:** say "insertion-ordered JSON text" in both places, or key on a canonical
form if the duplicate entries ever matter.

### IN-04: A cross-file line-number citation has already rotted

**File:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:376`

**Issue:** *"the fence FAILED at `src/server/output_validation.rs:1429` while
this whole file stayed green."* The assertion in question
(`assert!(violations.is_empty(), …)`) begins at `:1430` on this tree — the
citation was off by one before the round even landed. Hard-coded line numbers in
cross-file prose rot on the next edit and cannot be gated.

**Fix:** cite the test NAME and the assertion's message, both of which are
greppable and stable:
`v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`,
`"an $id-bearing embedded schema resource carrying a legacy $schema was NOT
rewritten"`.

---

## Verification performed for this review

Everything below was run on this tree at `a7c31fbf` unless noted. No source file
was modified.

| Check | Result |
|---|---|
| Re-derived `SUBSCHEMA_MAP_KEYWORDS` offline over all 19 `jsonschema-0.49.2/metaschemas/**.json` documents | Exactly the six shipped keywords; `$vocabulary` and `dependentRequired` rejected as documented. **No seventh omission.** |
| Same derivation over ONLY the five documents the shipped rustdocs name | FOUR keywords, neither rejected keyword — WR-04 |
| Unguarded `.value.type` `jq` form on `draft7.json` | exit 5, empty stdout — the rustdoc's guard note is accurate |
| `cargo test --test keyword_list_mirrors` | 2 passed |
| `cargo test --lib --features "full fuzzing" output_validation` | 25 passed (incl. `v2_pin_rewrites_…`, `keyword_lists_are_disjoint`) |
| `cargo test --features "full fuzzing" --test property_tests` | 21 passed (incl. both new properties and the mirror gate) |
| `cargo check --manifest-path fuzz/Cargo.toml --bin fuzz_schema_draft_pin` | clean — the workspace-excluded target still compiles |
| `cargo package --list --allow-dirty` | `tests/keyword_list_mirrors.rs` PRESENT, `fuzz/**` ABSENT — **CR-01** |
| Seed `15_dependencies_named_default` byte layout decoded | selector 1, `schema_len=203` == actual, instance `{"n":"NOT-AN-INTEGER"}`, schema parses; reaches invariants 5 and 6, correctly skipped by 3 |
| `git ls-files fuzz/corpus/fuzz_schema_draft_pin/ \| grep -c '/[0-9][0-9]_'` | 15 — matches the README's claim, and `fuzz/.gitignore`'s re-include pattern tracks the new seed |
| Detector/rewriter equivalence traced over all four member cases given disjoint lists | Agree in every case; `keyword_lists_are_disjoint` is the right precondition guard |
| Grep for `allOf`/`anyOf`/`oneOf`/`prefixItems` across the reviewed files and all 15 tracked seeds | Prose only; zero fixtures — **WR-03** |
| Feature-gate trace of `make quality-gate` → `test-all` vs `fuzzing` | `schema_dialect_normalization_properties` never runs locally; CI `--all-features` (`ci.yml:93`) does run it — **WR-05** |
| Round-3 WR-01/02/03/04/06/07 closure | Closed. WR-05 half-closed and correctly booked in `deferred-items.md:1314,1321`. IN-01 and IN-02 (round-3 numbering) untouched, not re-litigated. |

---

_Reviewed: 2026-08-02T18:35:02Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
