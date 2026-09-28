# Seed corpus — `fuzz_placeholder_pattern_redos`

Phase 128-10, T-128-48. Hand-written seeds for the LOOK at RESEARCH assumption
**A2**: a config-supplied `pattern` as a `ReDoS` vector on
`pmcp::server::schema_validation::validate_path_placeholder`.

## Why seeds are committed at all

Two reasons, and the second is the load-bearing one.

1. The layout is two length-prefixed documents, so an unseeded campaign splits raw
   bytes at arbitrary points — the same degeneracy `fuzz_schema_draft_pin`'s
   README records.
2. **The four classic catastrophic-backtracking shapes are essentially
   undiscoverable from random bytes.** `^(a+)+$`, `^(a|aa)+$`, `^([0-9]+)*$` and
   `^((a)*)*b$` are each a precise arrangement of nesting and alternation, and
   they are exactly what A2 is about. A look for A2 that cannot reach a nested
   quantifier is not a look — it is a no-panic check wearing the name of one.

## Layout

```text
byte 0                   : selector — bit 0 allow_slash, bit 1 declare a max_length
bytes 1..5               : u32 little-endian pattern_len
bytes 5..5+pattern_len   : the declared `pattern` bytes
bytes 5+pattern_len..    : the caller-supplied value bytes (the remainder)
```

## The seeds and what each one reaches

| Seed | Reaches |
|---|---|
| `01_nested_quantifier_classic` | `^(a+)+$` against 28 `a`s and a non-matching tail — the textbook catastrophic case, at the same `n <= 28` RESEARCH Finding 1j attempted falsification at. |
| `02_alternation_overlap` | `^(a|aa)+$`, the overlapping-alternation shape. |
| `03_digit_group_star` | `^([0-9]+)*$`, a character-class variant that a pattern-shape heuristic keyed on literals would miss. |
| `04_double_star_group` | `^((a)*)*b$`, doubly-nested star with a required trailing literal. |
| `05_benign_version` | **The ACCEPT control.** `^v[0-9]+$` against `v12` matches, so the declared-pattern step returns `Ok`. Without it the corpus would consist entirely of refusals and could not catch an over-refusing narrowing. |
| `06_permissive_pattern_floor` | **The D-10 ordering control.** A permissive `^.*$` with a traversal value: the unconditional character floor runs BEFORE the declared pattern, so the value stays refused. This is the invariant-3 case stated as a seed. |
| `07_slash_allowed_declared_max` | `allow_slash` on plus a declared `max_length`, the config-narrowed shape — the only route by which `/` is legitimately permitted. |
| `08_uncompilable_pattern` | A declared `pattern` that does not compile. The refusal carries `rule == "pattern"` and no compile detail. |

## A timeout is a FINDING TO FILE

RESEARCH Finding 1j attempted falsification at `n <= 28` on both the linear and
the backtracking engine and did not reproduce. That is evidence, not proof, which
is why T-128-48's disposition is **accept, instrumented** rather than mitigated.

If a run here produces a timeout or a crash artifact: file it with the reproducing
artifact from `fuzz/artifacts/fuzz_placeholder_pattern_redos/`, record the
`jsonschema` version and which engine was in play, and let the fix be chosen with
the measurement in hand. **Do not hand-roll a regex-complexity guard on a hunch** —
a narrowing that authors rely on would start refusing legitimate declarations, and
that is a worse outcome than the residual.

## Replaying

```bash
cd fuzz && cargo +nightly fuzz run fuzz_placeholder_pattern_redos \
  corpus/fuzz_placeholder_pattern_redos -- -runs=0 -timeout=5
```

Keep `-timeout` set: the whole point of the target is that the timeout is the
signal. Do NOT run it unbounded — the declared-pattern step memoizes each distinct
pattern in a process-global map, so an unbounded campaign can be killed by memory
rather than by a finding, and the two are indistinguishable from the exit code
(see the target's header).
