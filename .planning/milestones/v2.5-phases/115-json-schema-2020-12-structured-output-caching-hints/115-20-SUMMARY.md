# 115-20-SUMMARY — the ARRAY schema position is fenced (`115-REVIEW.md` WR-03)

**Executed:** 2026-08-08
**Plan:** `115-20-PLAN.md`
**Requirement:** SCHM-01
**Disposition change this plan executes:** the owner re-dispositioned WR-03 from
*defer-and-book* (`D-115-AM (2)`, 2026-08-02) to **FIX** on 2026-08-08.

---

## What changed

**No behavioural change.** Every pre-existing case keeps its verdict and its borrow/own
decision; array descent was already correct and unconditional. What landed is the four layers of
fence and prose that were absent around it.

| Layer | File | Change |
|---|---|---|
| Fixture | `src/server/output_validation.rs` | `embedded_legacy_resource_in_array(container, index)` — index fillers make a non-zero index addressable |
| Case row | same | `normalization_cases()` row (l): `allOf` at index **1**, so a first-element-only walk fails |
| Grid fence | same | `v2_pin_rewrites_an_embedded_resource_at_every_spec_defined_array_position` — 4 keywords × indices [0, 2] |
| Property draw | `tests/property_tests.rs` | `ARRAY_CONTAINER_DRAW`, an array branch in `embed_resource`, `arb_any_position_container()` feeding `arb_schema_document()` |
| Corpus | `fuzz/corpus/fuzz_schema_draft_pin/16_all_of_embedded_legacy` | new seed + README row, tracked count 15 → **16** |
| Contract | `contracts/mcp-protocol-sdk-v1.yaml` | `walk:` array clause, POSTCONDITION `SCHEMA POSITION` extension, embedded-resource invariant |
| Contract | `contracts/binding.yaml` | the same correction mirrored into all three note heads |

---

## T5 — the negative control, RUN and RECORDED

Both `Value::Array` arms deleted (`first_legacy_dialect`, `pin_dialect_in_place`), rebuilt,
suites re-run. **All three layers fired.** Verbatim:

**Unit — 2 of 21 failed** (before this plan the same deletion passed 25/25):

```
---- v2_pin_rewrites_an_embedded_resource_at_every_spec_defined_array_position stdout ----
panicked at src/server/output_validation.rs:1565:9:
an $id-bearing embedded schema resource carrying a legacy $schema was NOT rewritten
in 8 of 8 ARRAY positions:
[ "allOf[0]: rewritten=false, /allOf/0/$schema=Some(\"http://json-schema.org/draft-07/schema#\")",
  "allOf[2]: …", "anyOf[0]: …", "anyOf[2]: …", "oneOf[0]: …", "oneOf[2]: …",
  "prefixItems[0]: …", "prefixItems[2]: …" ]

---- normalize_schema_dialect_changes_only_dollar_schema_keys stdout ----
panicked at src/server/output_validation.rs:1976:13:
borrow/own decision is wrong for {"type":"object","allOf":[{},{"$id":"https://example.test/inner",
"$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}]}
  left: false   right: true
```

`8 of 8` is the anti-vacuity count doing its job: the fence swept the whole grid and the whole
grid was broken.

**Property — 1 failed:** `property_schema_normalization_is_idempotent_and_surgical`,
`tests/property_tests.rs:1669`. Summary `17/21 tests run: 16 passed, 1 failed`.

**Fuzz — seed 16 alone, exit 77:**

```
$ target/aarch64-apple-darwin/release/fuzz_schema_draft_pin -runs=0 <dir containing only seed 16>
EXIT=77
panicked at fuzz_targets/fuzz_schema_draft_pin.rs:658:
A LEGACY $schema SURVIVED NORMALIZATION: ["http://json-schema.org/draft-07/schema#"]
Input was: {"type":"object","properties":{"n":{"allOf":[{"$id":"https://example.test/inner",
"$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}]}}}
```

The `Input was:` line is seed 16 **verbatim** — invariant 5, isolated to this one seed.

### The isolation caveat, recorded honestly

The first fuzz control attempt used `cargo +nightly fuzz run … -runs=0 <isolated dir>`. It also
tripped invariant 5 — but on a DIFFERENT input (`{"$schema":…,"items":[{"$schema":"http://json-schameb]r"}]}`),
because **`cargo fuzz run` merges the target's own corpus directory into the run** regardless of
the path given, and 11,773 accumulated files came with it. That result is real but proves
nothing about seed 16. The isolated evidence above comes from invoking the built binary
directly, where 1 file is loaded and the tripping input is the seed itself.

**Two things follow, and the second is not flattering:**

1. Always invoke the binary directly when isolating a single seed; `cargo fuzz run` cannot do it.
2. The **accumulated** corpus already contained array-position documents that trip invariant 5 —
   so the fuzz target was not wholly blind to this defect *provided you replay a corpus grown by
   real fuzzing*. What was blind is the **committed 15-seed set**, which is the only thing a
   fresh clone or CI has. That is the gap seed 16 closes.

### Restoration

`shasum -a 256 src/server/output_validation.rs` → `1f335399fa05af7bb6ddd31590ebc857e3a2aece458ab6e60d3ffe1f4c6ab6dc`,
identical before and after the control, with zero `NEGATIVE CONTROL` markers left in the tree.
**This supersedes the `a97f5cb2…3192c` hash recorded by rounds 1–4** — that value described the
file before this plan added the fixture, the case row and the fence, and a future round citing
it against the current tree will find a mismatch that is expected, not a regression.

---

## Green-state evidence

| Check | Result |
|---|---|
| `cargo test --features full --lib output_validation` | **21 passed**, 0 failed (20 before; the new fence is the 21st) |
| `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'` | **21 tests run, 21 passed** — non-zero, per `project_nextest_selector_binary_not_test` |
| `cargo test --features full --test phase115_contract_bindings` | **5 passed** |
| Full corpus replay, `-runs=0` | 23,554 runs, `DONE`, no artifact written |
| Seed 16 alone, shipped code | exit **0** |
| Tracked seed count | `git ls-files … \| grep -c '/[0-9][0-9]_'` → **16** |
| Both contract YAMLs | `yaml.safe_load` parses clean |

**The unit count is 21, not the 25 rounds 3–4 recorded.** That is a FEATURE-SET difference, not a
regression: 25 is the count under `--all-features`, which enables the `fuzzing`-gated
`fuzz_support_tests` module. Under `--features full` the same tree measured 20 before this plan
and 21 after. Both numbers are correct for their flags; a future round comparing them must
compare like with like. (This is `115-REVIEW.md` WR-05's shape one layer out — a count that
means different things under different gates — and is why it is stated here rather than left for
a reader to reconcile.)

---

## Decisions taken (recorded in `115-20-PLAN.md`)

- **D-115-20-A — no shipped `SUBSCHEMA_ARRAY_KEYWORDS`.** Array descent consults no list, so a
  constant would be a fourth mirror to keep in lockstep for zero implementation benefit, and
  would import the list-incompleteness defect class (CR-01, WR-02) into a position structurally
  immune to it. The fences carry their own literals (`D-115-AI(4)`).
- **D-115-20-B — `EmbeddedPointer` reused unchanged**; `/allOf/0/$schema` is a valid RFC 6901
  pointer, so the index serves as the segment.
- **D-115-20-C — the rename-invariance draw stays map-only.** An array element has no name;
  widening it would generate byte-identical "renamed" pairs — vacuous passes.
- **D-115-20-D — hard-coded anti-vacuity count (`8`).** WR-01 showed a length-derived
  expectation cannot see a shortened literal. WR-01's own site is untouched and stays booked
  under `D-115-AM (3)`.

---

## What this does NOT close

`D-115-AM (3)` **WR-01** (the tautological assertion in the map grid fence), `(4)` **WR-05** (the
`fuzzing`-gated fences `make quality-gate` never runs), `(5)` **WR-04** (the derivation procedure
the rustdocs describe is not reproducible) and `(6)` remain residual and unowned. This plan
changed the disposition of **WR-03 only**.
