# Phase 120: Config-Server Packaging - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-22
**Phase:** 120-Config-Server Packaging
**Areas discussed:** Slot derivation & vocabulary, Binary dual-mode API shape, Format compat & wire freeze, Layer generality

---

## Slot derivation & vocabulary

### Q1 — How should endpoint, credentials, and auth mode become ConfigSlots at pack time?

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit `[[config_slots]]` block | Config author declares slots in a dedicated TOML section; `pack` reads them. Deterministic, nothing guessed. Cost: every Shape A config must gain the block; a forgotten slot bakes a value silently. | ✓ |
| Auto-derive from `${VAR}` placeholders | Scan for `${VAR}`, one slot per placeholder. No new syntax. Cost: `base_url` is a bare literal so endpoint never surfaces; auth mode is a structural key that cannot be a placeholder. | |
| Structural mapping of known keys | `pack` knows the Shape A schema: `[backend].base_url` → endpoint, `[backend.auth].*` → credential, `[backend.auth].type` → auth mode. Cost: hard-coded per server kind. | |
| Structural + explicit override | Structural defaults, optional block adds/overrides. Cost: two code paths and a precedence rule. | |

**User's choice:** Explicit `[[config_slots]]` block
**Notes:** The config author owns the baked-vs-slot split; `pack` does not infer.

### Q2 — How should endpoint and auth mode be typed in SlotType?

| Option | Description | Selected |
|--------|-------------|----------|
| New typed variants | `Endpoint { name, tested_value }` and `AuthMode { name, tested_value }` alongside the existing six; `classify()` puts both in the behavior-relevant family automatically. Cost: `SlotType`'s serialized surface grows. | ✓ |
| One generic `Config` variant | Single `Config { name, tested_value }`; the name carries the meaning. Covers every future non-secret slot. Cost: the type system stops distinguishing kinds. | |
| Reuse existing variants only | Map onto what exists, add nothing. Cost: nothing fits — calling an endpoint an `LlmProvider` is a lie that reads as a bug. | |

**User's choice:** New typed variants
**Notes:** Both carry `tested_value`, so the behavior-relevant classification is derived rather than hand-maintained. Credentials stay `Secret { name }` (identity-bearing).

### Q3 — How should "the slots the target environment must fill" be enumerated?

| Option | Description | Selected |
|--------|-------------|----------|
| Separate required-slots function | New function walks `config_slots` and returns identity-bearing and behavior-relevant alike; `detect_deviation` keeps its current meaning. Two functions, two questions, neither overloaded. | ✓ |
| Widen `detect_deviation` | One call answers both questions and Phase 121's wording holds literally. Cost: breaks the documented invariant that identity-bearing slots are never flagged, which existing tests assert. | |
| Leave it to Phase 121 | Phase 120 lands vocabulary only. Cost: 121 is the regression net every later phase leans on; PKG-03 asks for machine-checkable now. | |

**User's choice:** Separate required-slots function
**Notes:** Raised during discussion — Phase 121's success criterion 2 says `detect_deviation` "names exactly the slots B must fill", which is false against the code. That criterion must be restated against the new function when 121 is planned. Carried into CONTEXT.md `<specifics>`.

### Q4 — What does the packed config.toml layer contain for slot-declared keys?

| Option | Description | Selected |
|--------|-------------|----------|
| Placeholders required, verbatim pack | Pack byte-for-byte, but validate every slot-declared key already holds a `${VAR}` placeholder; a resolved literal is a pack-time error. Environment-independent digest, no secret travels, byte-identical restoration holds. Cost: `london-tube.toml`'s `base_url` literal must become a placeholder. | ✓ |
| Templatize at pack time | `pack` rewrites slot-declared keys into placeholders. No config edits needed. Cost: unpack no longer restores byte-identically (criterion 1), and silent mutation of the author's file. | |
| Pack verbatim, values are tested values | Config goes in as authored; literals recorded as tested values. Simplest. Cost: environment-dependent digests, and a resolved secret would be packed into a layer. | |

**User's choice:** Placeholders required, verbatim pack
**Notes:** Upholds the "secrets never travel" permanent non-goal documented in `slot/types.rs`.

---

## Binary dual-mode API shape

### Q1 — Is a config-only server a third binary mode, or is it the referenced mode?

| Option | Description | Selected |
|--------|-------------|----------|
| Two modes — config-only IS referenced | Package carries `BinaryRef { digest, media_type }` naming the shared already-deployed runtime; no bootstrap layer. Matches the roadmap's dual-mode wording. Cost: packing requires knowing the runtime's digest. | ✓ |
| Three modes — embedded / referenced / none | Config-only carries no binary identity at all. Cost: a third state to test, and the target env is told nothing about which runtime to run. | |

**User's choice:** Two modes — config-only IS referenced
**Notes:** "No bespoke binary" and "the binary lives in the target environment" are the same fact.

### Q2 — How should the two modes be expressed in the pack/unpack API?

| Option | Description | Selected |
|--------|-------------|----------|
| Enum param + enum return | `pack_server(package, BinaryMode, layout)`; `unpack_server → (ServerPackage, UnpackedBinary)`. There is no `Vec<u8>` to reach for on a referenced package. Cost: breaking change to both signatures. | ✓ |
| Separate `pack_config_server` fn | Nothing existing breaks. Cost: two pack paths and two unpack paths to keep in sync; the mistake moves rather than disappearing. | |
| `Option<&[u8]>` / `Option<Vec<u8>>` | Smallest diff. Cost: `None` conflates "referenced, resolve this digest" with "nothing here" — precisely the confusion criterion 2 rules out. | |

**User's choice:** Enum param + enum return
**Notes:** PKG-02's "a caller cannot mistake a referenced package for one that has bytes" becomes a type-system guarantee rather than a runtime check.

### Q3 — When unpacking a referenced package, should unpack look for the blob locally?

| Option | Description | Selected |
|--------|-------------|----------|
| Never look — always report the digest | Referenced mode always returns `Referenced { digest, media_type }`; resolution is the target environment's job. Modes stay crisp. Cost: a caller holding the blob still fetches it itself. | ✓ |
| Look, fall back to reporting | Convenient for same-machine round-trips. Cost: the same package unpacks to different shapes depending on ambient state — bad for the Phase 121 regression net. | |

**User's choice:** Never look — always report the digest

### Q4 — What happens to `ServerPackage.binary_ref`?

| Option | Description | Selected |
|--------|-------------|----------|
| Drop the field — BinaryMode is the source | One source of truth, matching the crate's own rule that binary payloads are layers, not struct fields. Cost: `ServerPackage`'s serialized shape changes, so the envelope layer and its golden digest move. | ✓ |
| Keep it, pack validates agreement | Nothing in the struct changes. Cost: two places hold the same fact, with a validation rule existing only to keep them honest. | |
| Keep it, BinaryMode fills it | No disagreement possible. Cost: a silently overwritten field is surprising to construct; the pre-pack value is meaningless. | |

**User's choice:** Drop the field — BinaryMode is the source
**Notes:** This is what forces the format-version decision in the next area.

---

## Format compat & wire freeze

### Q1 — How should the format version move for this change?

| Option | Description | Selected |
|--------|-------------|----------|
| 0.2.0 — declare the break | `pmcp-package` 0.1.1 → 0.2.0; re-pin `EXPECTED_SERVER_DIGEST`; update the wire-freeze comment to the 0.2.x line. Matches what `digest_stability.rs` already says should happen. | ✓ |
| Stay 0.1.x — keep it additive | Preserve every golden digest. Cost: requires reversing the `binary_ref` decision just made. | |
| 0.2.0 with a versioned artifactType | Bump `ARTIFACT_TYPE_SERVER` to `.v2` so the manifest declares its format. Cost: a second version axis to keep in step. | |

**User's choice:** 0.2.0 — declare the break
**Notes:** `ARTIFACT_TYPE_SERVER` stays `.v1`. Phase 124 owns moving `cargo-pmcp`'s pin and the `pmcp_package_pin.rs` tripwire in the same change.

### Q2 — Should the 0.2.0 unpack path still read packages written by 0.1.x?

| Option | Description | Selected |
|--------|-------------|----------|
| No — refuse with a clear error | Detect the 0.1.x layer shape and fail naming the format change and the writing version. Clean, one code path; the crate is experimental with no published consumers. | ✓ |
| Yes — read both shapes | Nothing already packed becomes unreadable. Cost: two deserialization paths and a synthesized `binary_ref`, carried indefinitely. | |
| No, and add the artifactType bump after all | Precise refusal via a declared version. Cost: revisits Q1 and adds the second version axis. | |

**User's choice:** No — refuse with a clear error

### Q3 — How should unpack locate layers now that the layer set varies?

| Option | Description | Selected |
|--------|-------------|----------|
| Media-type keyed lookup | Layer order stops being load-bearing; a missing required layer names itself; gives the 0.1.x refusal a precise signal. Cost: order is no longer self-documenting, so the golden must pin it deliberately. | ✓ |
| Fixed extended positional order | Smallest structural change. Cost: "optional layer at a fixed index" is a contradiction — every later layer shifts. | |

**User's choice:** Media-type keyed lookup
**Notes:** Today `unpack_server` reads `.first()`, `.get(1)` … `.get(5)`, which cannot survive an optional bootstrap layer plus two new ones.

### Q4 — What should the config-only golden fixture pin?

| Option | Description | Selected |
|--------|-------------|----------|
| Packed OCI manifest digest | The value `finalize_pack` returns — the only one that moves when layer set, layer order or a media-type string changes, which is what criterion 4 asks for. Cost: a new fixture kind; the test must pack, not just deserialize. | ✓ |
| Struct digest, like the existing four | Matches the established pattern. Cost: blind to layer set, order and media types — criterion 4 would not actually be met. | |
| Both | Each guards what it can see. Cost: two fixtures and two constants for one package kind. | |

**User's choice:** Packed OCI manifest digest
**Notes:** Surfaced during discussion — the four existing goldens pin `manifest_digest(value)` over a struct, which never sees layers.

---

## Layer generality

### Q1 — How general should the two new layer media types be?

| Option | Description | Selected |
|--------|-------------|----------|
| Generic config + typed spec | Shared `mcp-server.config.v1+toml` for the config; per-kind spec layer (`mcp-server.openapi-spec.v1` now). The half that generalizes is shared; the half that doesn't stays honest. Cost: each future sibling adds a constant. | ✓ |
| Both OpenAPI-specific | Matches the roadmap's literal wording, commits to nothing speculative. Cost: the config layer is provably identical across siblings, so the second server forces a rename or duplicate. | |
| Fully generic pair | One pair covers all three siblings. Cost: `pmcp-workbook-server` takes a directory, so "generic" would have to mean an archive — speculative from one example. | |

**User's choice:** Generic config + typed spec
**Notes:** Grounded in measurement — the three Shape A siblings take different second inputs: `--spec` (optional file), `--schema` (required file), `--bundle-dir` (a directory).

### Q2 — Is the spec layer optional in the package?

| Option | Description | Selected |
|--------|-------------|----------|
| Optional — mirror the runtime | Matches what the binary accepts; curated-only servers stay packable. Cost: "the spec is baked" becomes conditional, so the golden must pin the with-spec case explicitly. | ✓ |
| Required — spec always baked | PKG-03 holds unconditionally. Cost: curated-only servers become unpackable, and `parity_replay.rs` runs with `spec: None` today. | |
| Optional, but recorded | Distinguishes absence from omission. Cost: a field whose only job is that distinction. | |

**User's choice:** Optional — mirror the runtime

### Q3 — How should the packed files' filenames be preserved?

| Option | Description | Selected |
|--------|-------------|----------|
| OCI descriptor annotations | `org.opencontainers.image.title` is the standard field; a registry client reads it with zero translation — the stated reason `oci-spec` was kept. Cost: annotations are inside the digested manifest, so a filename change moves the digest. | ✓ |
| Canonical names | Digest is insensitive to what the author called the files. Cost: a round-trip renames the author's files; the spec's extension must be inferred. | |
| Bytes only — caller names them | Keeps unpack side-effect-light. Cost: "restores both files byte-identically" becomes the test's job; every caller re-invents naming. | |

**User's choice:** OCI descriptor annotations

### Q4 — Should the spec layer's media type encode YAML vs JSON?

| Option | Description | Selected |
|--------|-------------|----------|
| One media type, bytes verbatim | Single layer carries whatever was supplied; format is evident from the filename annotation. One constant, no branching. Cost: a parsing consumer must sniff or read the annotation. | ✓ |
| Two media types (`+yaml` / `+json`) | A consumer knows how to parse from the media type alone. Cost: two constants, a detection rule, and a two-way lookup. | |
| Normalize to JSON | Exactly one representation. Cost: breaks byte-identical restoration (criterion 1) and makes `pmcp-package` parse a payload it has no reason to understand. | |

**User's choice:** One media type, bytes verbatim

---

## Claude's Discretion

None — no area was delegated with "you decide". Three sub-questions were surfaced and
consciously left for research/planning rather than pre-decided; they are recorded in
CONTEXT.md under **Open for planning**:

1. Where the referenced runtime digest comes from at pack time (`BinaryRef.digest` is
   `Option` today; `BinaryMode::Referenced` needs it non-optional).
2. Whether the `[[config_slots]]` block needs a completeness check, since a forgotten slot
   bakes a value silently.
3. How `london-tube.toml`'s `base_url` literal becomes a `${VAR}` placeholder without
   breaking the existing green `parity_replay.rs`.

## Deferred Ideas

- A generic schema/bundle layer covering all three Shape A siblings — revisit when a second
  config-only server is actually packed.
- A `.v2` `artifactType` so the manifest self-declares its format version — revisit on a
  third format break.
- Packed-manifest golden pins for the agent/team/workflow package kinds — the same blindness
  applies to all four existing goldens.
- A compat reader for 0.1.x packages — additive if ever needed.
