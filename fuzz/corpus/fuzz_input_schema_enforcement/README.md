# Seed corpus — `fuzz_input_schema_enforcement`

Phase 128-10, SC-7 / T-128-47. Hand-written seeds for the `tools/call`
INPUT-schema enforcement path and its value-free refusal renderer.

## Why seeds are committed at all

The target's input is TWO JSON documents, length-prefixed. Split raw bytes at an
arbitrary point and both halves fail to parse on nearly every iteration, so an
unseeded campaign degenerates into a JSON-parser fuzz that never reaches
validation. `fuzz_schema_draft_pin`'s corpus README records the same argument for
the same layout; this target reuses it deliberately so the crate has one
convention.

There is a second, target-specific reason. The schema reaching `validate_input`
is a **bounded projection** of the schema document (see the target's header for
the process-global-cache argument that forces it), and the projection is chosen
by the keywords the document itself declares. So a seed carrying a real
`maxLength` schema selects the `maxLength` arm of the refusal renderer, a seed
carrying `patternProperties` selects the redaction case, and so on. Without the
seeds the projection would be driven almost entirely by selector-byte fallbacks
and most arms would go unvisited.

## Layout

```text
byte 0                  : selector — sentinel placement + knob fallbacks
bytes 1..5              : u32 little-endian schema_len
bytes 5..5+schema_len   : schema document bytes
bytes 5+schema_len..    : instance document bytes (the remainder)
```

Both documents in every seed are valid JSON — asserted at generation time. A seed
whose halves do not parse teaches the fuzzer nothing it could not find alone.

## The seeds and what each one reaches

| Seed | Reaches |
|---|---|
| `01_additional_properties_false_undeclared_key` | The closed envelope, with a PHI-shaped UNDECLARED key. The `additionalProperties` refusal must carry a COUNT and the allowed names, never the offending key. |
| `02_max_length_violation` | The `maxLength` arm — the case where a DECLARED name IS legitimately echoed, which is why the target's oracle is provenance-based rather than absence-based. |
| `03_pattern_violation` | The `pattern` arm; the refusal names the DECLARED pattern. |
| `04_required_absent` | The `required` arm on an empty instance — the MCP "missing arguments" semantics, refused by `required` rather than by `type` (whose message would echo `null`). |
| `05_pattern_properties_phi_key` | `safe_pointer`'s redaction: a key matched only by `patternProperties` must reach the refusal as `<redacted>`, never verbatim. |
| `06_additional_properties_subschema_open` | The OPEN envelope (`additionalProperties` as a subschema) — the other shape whose keys are caller-chosen and must be redacted. |
| `07_enum_violation` | The `enum` arm; the refusal lists DECLARED options only. |
| `08_array_items_max_length` | Nested `items` — the case where a declared ARRAY INDEX legitimately survives the pointer projection (`/alpha/1`), so the redaction cannot have become a blanket suppression. |
| `09_uncompilable_declared_pattern` | A declared `pattern` that does not compile. `validate_input` must refuse EVERY call with the detail-free `keyword: "schema"` violation, while `check_input_schema_compiles` — whose audience is the config author — may name the offending declaration. |
| `10_format_uuid` | The `format` arm, which asserts on inputs (Phase 128 Q1) where it is annotative on outputs. |

## Replaying

```bash
cd fuzz && cargo +nightly fuzz run fuzz_input_schema_enforcement \
  corpus/fuzz_input_schema_enforcement -- -runs=0
```

`-runs=0` replays the corpus without mutating, which is the cheapest way to check
the seeds still parse and the invariants still hold after a renderer edit.
Afterwards `fuzz/artifacts/fuzz_input_schema_enforcement/` must be empty.

`+nightly` is required — `cargo fuzz` passes `-Zsanitizer=address`, which stable
rustc refuses. Do not cite `make test-fuzz` as evidence for this target; its
blanket `|| echo` reports success having fuzzed nothing. `make test-fuzz-strict`
is the leg that propagates a crash.
