# Phase 120: Config-Server Packaging - Research

**Researched:** 2026-08-22
**Domain:** OCI artifact layer modelling (`oci-spec` 0.10), Rust wire-format evolution, config-slot typing
**Confidence:** HIGH for the in-repo mechanics (every claim below was read from source this session); MEDIUM for the three "Open for planning" resolutions, which need a user decision.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Slot derivation and vocabulary**

- **D-01:** Slots are **declared explicitly** by the config author in a `[[config_slots]]` block in the server's `config.toml`; `pack` reads them. Nothing is auto-derived from `${VAR}` placeholders and nothing is structurally inferred from known Shape A keys. Rejected: placeholder auto-derivation (`base_url` is a bare literal in the fixture and auth mode is a structural key, so it cannot cover PKG-03's three), structural mapping, and structural + override. — **Reversibility:** costly — the block becomes part of the Shape A config schema that every packable server config must carry; adding auto-derivation later is additive, but removing the block after configs adopt it is a config-format migration.

- **D-02:** `SlotType` gains **two new typed variants**, `Endpoint { name, tested_value }` and `AuthMode { name, tested_value }`, alongside the existing six. Both carry `tested_value`, so `classify` (`crates/pmcp-package/src/slot/classification.rs:24`) places them in the **behavior-relevant** family automatically — the classification is derived from `SlotType::tested_value()`, not hand-maintained. Credentials remain `Secret { name }` (identity-bearing). Rejected: one generic `Config { name, tested_value }` variant (loses the type-level distinction), and reusing existing variants (nothing fits; calling an endpoint an `LlmProvider` would read as a bug). — **Reversibility:** one-way — `SlotType` is a serialized wire type; once packages carry `endpoint`/`auth_mode` discriminants, removing or renaming a variant breaks every package already written. Note `SlotType` is deliberately NOT `deny_unknown_fields`, so older readers tolerate the addition.

- **D-03:** Enumerating "the slots the target environment must fill" gets a **separate required-slots function** (e.g. `required_slots` / `unfilled_slots`) that walks a package's `config_slots` and returns identity-bearing and behavior-relevant slots alike. `detect_deviation` (`crates/pmcp-package/src/slot/deviation.rs:28`) keeps its current, correct meaning — "this behavior-relevant value differs from what was tested" — and keeps returning `None` for identity-bearing slots. Rejected: widening `detect_deviation` (breaks a documented invariant its own tests assert), and deferring to Phase 121. — **Reversibility:** reversible.

- **D-04:** The packed `config.toml` layer holds the file **byte-for-byte as authored**, and `pack` **validates that every key named by a `[[config_slots]]` entry already holds a `${VAR}` placeholder** — a resolved literal in a slot-declared key is a pack-time error. Consequences: the manifest digest is environment-independent (two environments differing only in endpoint produce the same package), no resolved secret can travel in a layer (upholding the "secrets never travel" non-goal in `crates/pmcp-package/src/slot/types.rs`), and `unpack` restores the file byte-identically as PKG-01 criterion 1 requires. Rejected: templatizing at pack time (breaks byte-identical restoration; silent mutation of the author's file), and packing verbatim with literals as tested values (environment-dependent digests; a resolved secret could be packed). — **Reversibility:** costly — relaxing the validation later is additive, but tightening it after configs exist forces every config to be edited.

**Binary dual-mode**

- **D-05:** **Two modes, not three.** A config-only server *is* the **referenced** mode: the package carries `BinaryRef { digest, media_type }` naming the shared, already-deployed runtime binary, and no bootstrap layer. "No bespoke binary" and "the binary lives in the target environment" are the same fact. Rejected: a third `none` mode (leaves the target env with no statement about which runtime to run). — **Reversibility:** costly — adding a third mode later is an enum arm on a wire type.

- **D-06:** The modes are expressed as an **enum parameter and an enum return**: `pack_server(package, binary: BinaryMode<'_>, layout)` with `BinaryMode::Embedded(&[u8]) | BinaryMode::Referenced { digest, media_type }`, and `unpack_server` returns `(ServerPackage, UnpackedBinary)` with the same two-arm shape. There is no `Vec<u8>` to reach for on a referenced package, so PKG-02's "a caller cannot mistake a referenced package for one that has bytes" is enforced by the type system rather than by a runtime check. Rejected: a parallel `pack_config_server` function (two paths to keep in sync; the mistake moves rather than disappearing), and `Option<&[u8]>`/`Option<Vec<u8>>` (`None` conflates "referenced, resolve this digest" with "nothing here" and hands the caller no digest). — **Reversibility:** one-way — breaking change to both public signatures; every caller updates. Paired with D-09 (0.2.0).

- **D-07:** Unpacking a referenced package **never looks for the blob locally** — it always returns `Referenced { digest, media_type }`. `unpack` stays a local, offline operation and resolution is the target environment's job. Rejected: look-then-fall-back (the same package would unpack to different shapes depending on ambient state, which would make Phase 121's regression net pass in env A and fail in env B for reasons unrelated to the package). — **Reversibility:** reversible.

- **D-08:** **`ServerPackage.binary_ref` is dropped** (`crates/pmcp-package/src/package/server.rs:379`). `BinaryMode` is the single source of the binary's identity, reconstructed at unpack from the manifest. This follows the crate's own documented rule that binary payloads are OCI layers, not typed-struct fields. Rejected: keeping the field with a pack-time agreement check, and keeping it silently overwritten by `BinaryMode` — both hold the same fact in two places. — **Reversibility:** one-way — changes `ServerPackage`'s serialized shape, so the `MT_SERVER_ENVELOPE` layer's bytes and `EXPECTED_SERVER_DIGEST` move. This is what forces D-09.

**Format compatibility and the wire freeze**

- **D-09:** `pmcp-package` goes **0.1.1 → 0.2.0 as a declared break**. `EXPECTED_SERVER_DIGEST` in `crates/pmcp-package/tests/digest_stability.rs` is re-pinned and the wire-freeze comment is updated to name the 0.2.x line — the file already states that the day these digests must change is the day the format goes 0.2.0. **`ARTIFACT_TYPE_SERVER` stays `.v1`** (no second version axis). Rejected: staying 0.1.x (would require reversing D-08), and bumping the `artifactType` alongside. Phase 124 owns moving `cargo-pmcp`'s caret pin and the `cargo-pmcp/tests/pmcp_package_pin.rs` tripwire in the same change. — **Reversibility:** one-way — a published version number and a re-pinned wire freeze.

- **D-10:** The 0.2.0 `unpack` path **does not read 0.1.x packages**. It detects the 0.1.x layer shape and **refuses with a clear error** naming the format change and the version that wrote it, rather than mis-deserializing. `pmcp-package` is an experimental 0.x crate with no published consumers, so nothing in the wild is stranded. Rejected: reading both shapes (two deserialization paths and a synthesized `binary_ref` carried indefinitely), and refusing plus a `.v2` artifactType. — **Reversibility:** reversible — adding a compat reader later is additive.

- **D-11:** `unpack` locates layers by **vendor media type, not by index**. Today `unpack_server` (`crates/pmcp-package/src/oci/unpack.rs:79-90`) reads positionally (`.first()`, `.get(1)` … `.get(5)`), which cannot survive an optional bootstrap layer plus two new layers. Media-type keying makes layer order non-load-bearing, lets a missing required layer name itself, and gives D-10's refusal a precise signal (a bootstrap layer sitting alongside a config layer, or an envelope carrying `binary_ref`). Rejected: a fixed extended positional order ("optional layer at a fixed index" is a contradiction — every later layer shifts). — **Reversibility:** reversible.

- **D-12:** The config-only golden fixture pins the **packed OCI manifest digest** returned by `finalize_pack` — i.e. the test packs a layout rather than only deserializing a struct. This is the only value that actually moves when the layer set, layer order, or a media-type string changes, which is what PKG-01/PKG-02 criterion 4 asks for. The four existing goldens under `crates/pmcp-package/tests/golden_fixtures/` pin `manifest_digest(value)` over a **struct**, which is blind to all three. Rejected: adding a struct-level fixture in the established pattern (would not meet the criterion), and doing both. — **Reversibility:** reversible.

**Layer generality**

- **D-13:** **Generic config layer, typed spec layer.** One shared `application/vnd.pmcp.mcp-server.config.v1+toml` for the `config.toml` — identical across all three Shape A siblings — and a per-kind spec layer whose media type names the kind, i.e. `application/vnd.pmcp.mcp-server.openapi-spec.v1` now, siblings later. The half that genuinely generalizes is shared; the half that does not stays honest. Rejected: naming both layers OpenAPI-specific (the config layer is provably identical across siblings, so the second server forces a rename or a duplicate constant), and a fully generic `schema.v1` pair (`pmcp-workbook-server` takes a `--bundle-dir` **directory**, so "generic" would have to mean an archive — speculative from one example). — **Reversibility:** costly — media-type strings are wire identifiers; renaming one after packages exist is a format change.

- **D-14:** The **spec layer is optional**, mirroring the runtime. `pmcp-openapi-server`'s `--spec` is `Option<PathBuf>` (D-03 of that crate: a curated-only server boots without it), and `crates/pmcp-openapi-server/tests/parity_replay.rs` currently runs with `spec: None`. A config-only package may therefore carry a spec layer or not. The golden fixture (D-12) must pin the **with-spec** case explicitly so the without-spec path cannot quietly become the default. Rejected: requiring the spec (would make curated-only servers — which the binary explicitly supports — unpackable), and optional-plus-an-absence-marker. — **Reversibility:** reversible.

- **D-15:** Original **filenames are carried in OCI descriptor annotations** (`org.opencontainers.image.title` is the standard field for exactly this), so `unpack` restores `london-tube.toml` and `london-tube-api.yaml` under their own names. Standards-native, and a registry client reads it with zero translation — which is the stated reason `oci-spec` was kept when `oci-client` was excluded. Note the annotations become part of the canonical manifest, so a filename change moves the digest. Rejected: canonical fixed names (renames the author's files; the spec's extension must be inferred), and bytes-only (pushes naming onto every caller). — **Reversibility:** costly — annotations are inside the digested manifest.

- **D-16:** **One spec media type, bytes verbatim.** A single `mcp-server.openapi-spec.v1` layer carries whatever the author supplied, byte-for-byte; the format (YAML or JSON) is evident from the filename annotation established in D-15. Rejected: two media types (`+yaml` / `+json`, adding a pack-time detection rule and a two-way lookup), and normalizing to JSON (would break byte-identical restoration — criterion 1 — and would make `pmcp-package` parse a payload it has no reason to understand). — **Reversibility:** reversible.

### Claude's Discretion

No area was delegated with "you decide". Three sub-questions were surfaced and consciously left for research/planning to resolve — they are recorded under **Open for planning** below, not as locked decisions.

### Open for planning

These were raised during discussion, are in scope for Phase 120, and the user chose not to pre-decide them. The planner should resolve them, or the researcher should surface options:

1. **Where the referenced runtime digest comes from at pack time.** D-05 requires a referenced package to name a digest, but `BinaryRef.digest` is `Option<ManifestDigest>` today (`None` before packing). `BinaryMode::Referenced` needs it non-optional. Whether the caller supplies it verbatim, or it is derived from something in the deploy descriptor, is unresolved. Note milestone Decision 2 forbids an OCI registry client, so `pmcp-package` can only treat the digest as opaque.
2. **Whether the `[[config_slots]]` block needs a completeness check.** D-01 makes the author responsible for declaring slots; a forgotten slot bakes a value silently. D-04's placeholder validation catches the inverse (a declared slot holding a literal) but not an undeclared environment-specific literal.
3. **How `london-tube.toml`'s `base_url = "https://api.tfl.gov.uk"` literal becomes a `${VAR}` placeholder** without breaking `parity_replay.rs`, which is an existing green offline test.

> All three are answered with evidence in **## Open Questions** below. A **fourth** question, not in CONTEXT.md, was surfaced by research (one crate or two) and is recorded there too.

### Deferred Ideas (OUT OF SCOPE)

- **Generic schema/bundle layer covering all three Shape A siblings** — rejected in D-13 as speculative from one example; `pmcp-workbook-server` takes a directory, so a generic layer would have to mean an archive. Revisit when a second config-only server is actually packed.
- **`.v2` `artifactType` so the manifest self-declares its format version** — considered and declined in D-09 to avoid a second version axis. Worth revisiting if a third format break lands.
- **Packed-manifest golden pins for the other three package kinds** (agent/team/workflow) — D-12 adds one only for the config-only server kind. The same blindness (struct digests cannot see layer set/order/media types) applies to all four.
- **A compat reader for 0.1.x packages** — declined in D-10; additive if it is ever needed.

**Also out of scope (milestone-level, from REQUIREMENTS.md):** signing keys or PKI in the SDK; an ECR/OCI registry client (`oci-client`); changing `LATEST_PROTOCOL_VERSION`; refactoring the manifest schema for elegance; **byte-for-byte round-tripping of the package** (note: byte-identical restoration of the *config and spec files* IS required by criterion 1 — the non-goal is byte identity of the *package artifact* across manifest revisions); SEP-2243. Phase 121 (PKG-04 round-trip E2E), Phase 122 (attestation), Phase 123 (CLI verbs), Phase 124 (publish ledger / version bump mechanics) are separate phases.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **PKG-01** | A server with **no bespoke binary** can be packed. Vendor media types carry the server's own `config.toml` and its OpenAPI spec as layers, so a Shape A config-only server (`pmcp-openapi-server`) has a complete package identity. Today `pack_server` requires `bootstrap: &[u8]` and neither file has a layer type. | Pattern 1 (media-type-keyed lookup, D-11) removes the positional constraint that makes an optional bootstrap layer impossible; Pattern 2 (D-15 `ANNOTATION_TITLE`, verified in `oci-spec` 0.10 source) restores original filenames; `vendor_media_type` needs only two new constants (no new machinery). **Blocker surfaced:** Pitfall 1 — `pack` cannot read `[[config_slots]]` from TOML without promoting a dev-dep or moving the seam; Pitfall 2 — the fixture will not parse at runtime until `pmcp-server-toolkit::ServerConfig` grows the field. |
| **PKG-02** | The binary is **dual-mode** — embedded (bootstrap bytes) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment). Both modes are required; `BinaryRef` already has the right shape but nothing resolves it. | Pattern 3 gives the `BinaryMode`/`UnpackedBinary` enum pair with a non-optional digest, keeping `BinaryRef` as the *wire* payload of a new `MT_SERVER_BINARY_REF` layer (rationale + rejected alternative in Alternatives Considered / A2). Open Question 1 resolves where the digest comes from: caller-supplied verbatim, already produced by `cargo-pmcp/src/deployment/targets/aws_lambda/artifact.rs:330-337`. Only **one** downstream call site exists (`cargo-pmcp/src/commands/package/inspect.rs:119`). |
| **PKG-03** | What is **baked** versus what is a **slot** is decided and documented. Working split: the OpenAPI spec is baked; endpoint, credentials and auth mode are slots filled at unpack. | Pattern 4 shows the two-arm edit that makes D-02's variants classify correctly for free — **and the `_ => None` wildcard at `slot/types.rs:97` that will silently defeat it**. Criterion 3's digest assertion is mechanical (Code Examples). **Blockers surfaced:** Pitfall 3 (no `${VAR}` expansion for `base_url`; `parity_replay.rs:220-223` asserts the literal) and Pitfall 4 (`AuthConfig` is `#[serde(tag = "type")]`, so `type = "${AUTH_MODE}"` cannot deserialize — **D-04 is unsatisfiable for this key as written; needs a user decision**). |

</phase_requirements>

<project_constraints>
## Project Constraints (from CLAUDE.md)

Directives the planner must honour. These carry the same authority as CONTEXT.md's locked decisions.

| Directive | Applies here as |
|-----------|-----------------|
| **Zero tolerance for defects; `make quality-gate` before any commit/push** | The gate reaches `pmcp-package` via `make pmcp-package-gate` (`Makefile:871-877`, chained at `:896`). Plan verify blocks must use it, not a bare `cargo test`. |
| **Cognitive complexity ≤ 25 per function (PMAT, CI-blocking, pinned 3.15.0)** | `unpack_server` is already a long linear function; D-11's media-type index + D-10's shape detection + the two-arm binary mode will push it. **Extract `index_layers`, `detect_legacy_shape`, and `read_binary_mode` as separate functions from the start** rather than refactoring under a failing gate. `pmat` 3.15.0 is installed locally. |
| **Zero SATD comments** | No `TODO`/`FIXME`/`XXX` in the new code, including in the regenerated fixtures' surrounding comments. |
| **80%+ test coverage; comprehensive rustdoc with working examples** | Every new public item (`BinaryMode`, `UnpackedBinary`, `required_slots`, the three media-type constants) needs rustdoc. Doctests are the phase's cheapest satisfaction of the "EXAMPLE demonstration" requirement — `pmcp-package` has no `examples/` directory. |
| **ALWAYS requirements for new features: FUZZ + PROPERTY + UNIT + EXAMPLE** | Property test recommended: media-type-keyed lookup invariant under layer permutation (the D-11 analogue of the existing `aggregate_ordering_is_stable_under_permutation` proptest at `slot/aggregate.rs:116-139`). Fuzz: `cargo-pmcp/src/commands/package/kind.rs` is already the fuzzed manifest-parse boundary (`kind.rs:1-12`) — extending `detect_kind` (Pitfall 8) keeps that target meaningful. |
| **Contract-first: update the contract YAML in `../provable-contracts/contracts/<crate>/`, run `pmat comply check`** | **Currently vacuous — measured.** `/Users/guy/Development/mcp/sdk/provable-contracts/` exists but contains only `README.md` and `.pmat/`; there is **no `contracts/` directory at all**, so no `pmcp-package` contract exists to update. Do not write a task that edits a non-existent file. If the milestone wants this directive honoured, authoring the first contract is its own scoping decision. |
| **Release & Publish Workflow, slot 13** | `pmcp-package` is workspace-**excluded**; it publishes via `cargo publish --manifest-path crates/pmcp-package/Cargo.toml`, **not** `-p pmcp-package` (`release.yml:438-445`). D-09's 0.1.1 → 0.2.0 bump is a Cargo.toml edit here; moving `cargo-pmcp`'s caret pin and `cargo-pmcp/tests/pmcp_package_pin.rs` is **Phase 124's** job, not this one. |
| **Pre-commit hook blocks commits on fmt/clippy/build/doctest** | `make pmcp-package-gate` runs clippy with `-D warnings` and `--all-targets`, which is stricter than the root gate's allow-list. Budget for it. |

</project_constraints>

## Summary

This phase is almost entirely a **surgical refactor of one workspace-excluded crate** (`crates/pmcp-package`, v0.1.1, 3616 LOC across `src/`, 130 tests green in 0.1s). There is essentially no new technology to learn: the OCI layer machinery, digest-verify-before-deserialize discipline, canonical-JSON determinism, and slot classification already exist and already work. The phase's difficulty is not "how do I build this" — it is **"what does the change break, and how far outside `pmcp-package` does the blast radius actually reach."** Research found the blast radius is **larger than CONTEXT.md's canonical-refs list implies**, in three specific and independently-verified ways.

**Finding 1 — the public API break is nearly free.** `pack_server` has **zero callers outside the crate**; `unpack_server` has exactly **one** (`cargo-pmcp/src/commands/package/inspect.rs:119`). D-06's signature change costs one line of downstream edit. This is the cheap part and it validates D-10's "no published consumers, break now."

**Finding 2 — D-01 and D-04 collide with `pmcp-server-toolkit`, a crate CONTEXT.md does not list as in-scope.** Three independent, verified collisions: (a) `ServerConfig` carries `#[serde(deny_unknown_fields)]`, so a `[[config_slots]]` block in `london-tube.toml` is a **hard parse error** and the server will not boot; (b) `BackendSection.base_url` is a plain `String` used verbatim — the toolkit has `${VAR}` expansion for auth credentials and `code_mode.token_secret` but **not** for `base_url`, so `base_url = "${TFL_BASE_URL}"` parses, validates, and then sends HTTP requests to a literal `${...}` URL; (c) `AuthConfig` is `#[serde(tag = "type")]`, so `type = "${AUTH_MODE}"` **cannot deserialize at all** — D-04's blanket placeholder rule is literally unsatisfiable for the auth-mode key. On top of that, `parity_replay.rs:220-223` **asserts** the exact literal `base_url = "https://api.tfl.gov.uk"` is present in the fixture before string-replacing it. All three of the "Open for planning" items trace back to this one root.

**Finding 3 — D-08 forces three artifact regenerations, not one.** Dropping `binary_ref` moves `EXPECTED_SERVER_DIGEST`, invalidates `golden_fixtures/canonical/server.canonical.json`, **and** invalidates `golden_fixtures/server_team_fs_v1.json` itself — because `roundtrip.rs` asserts the fixture's own bytes equal `canonicalize(&parsed)`, so the fixture is stored in canonical form and cannot simply keep an ignored field.

**Primary recommendation:** Plan this as **two crates, not one**. Wave A lands the `pmcp-package` layer/binary-mode/wire-freeze work (which is self-contained, well-understood, and satisfies criteria 1, 2, 4 outright). Wave B lands the `pmcp-server-toolkit` additive changes that make a slot-declaring, placeholder-holding `london-tube.toml` *actually bootable* — without which criterion 3 is provable only against a config the runtime rejects. Escalate the auth-mode placeholder impossibility to the user before planning: D-04 as written cannot be satisfied for `[backend.auth] type`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Vendor media-type constants for config + spec layers | Format library (`pmcp-package::oci::media_types`) | — | Wire identifiers; the crate is the single source of truth both `cargo-pmcp` and the pmcp.run platform resolve from (`src/lib.rs:7-18`) |
| Layer write + content addressing | Format library (`oci::layout::OciLayout`) | — | Already generic over arbitrary bytes; the config/spec layers are byte payloads exactly like bootstrap |
| Media-type-keyed layer lookup (D-11) | Format library (`oci::unpack`) | — | Replaces positional reads; must stay pure/offline |
| Binary dual-mode enum (D-06) | Format library (`oci::pack` / `oci::unpack` public API) | — | Type-level distinction is the enforcement mechanism |
| Resolving a referenced binary's blob | **Target environment / caller** | — | D-07 + milestone Decision 2 (no `oci-client`); `pmcp-package` treats the digest as opaque |
| Producing a referenced binary's digest | **Caller (`cargo-pmcp`)** | — | Already implemented: `cargo-pmcp/src/deployment/targets/aws_lambda/artifact.rs:330-337` derives `sha256_hex` via `pmcp_package::digest::ManifestDigest::from_bytes` and compares against the release `.sha256` sidecar |
| Parsing `[[config_slots]]` out of a `config.toml` | **Contested — see Pitfall 1** | — | `toml` is a **dev-dependency only** in `pmcp-package` (`Cargo.toml:43-50`); making `pack` read TOML promotes it to a runtime dep and breaches the crate's own scope fence (`src/lib.rs:31-38`) |
| Accepting `[[config_slots]]` in a server config | **`pmcp-server-toolkit::config`** | — | `ServerConfig` is `deny_unknown_fields`; the toolkit's own doctrine (`config.rs:23-24`) is "always ADD the missing field" |
| `${VAR}` expansion of `base_url` | **`pmcp-server-toolkit`** (does not exist today) | — | Expansion exists only in `http/auth.rs:510` (credentials) and `code_mode.rs:1173-1204` (token secret) |
| Slot classification / aggregation | Format library (`slot::classify` / `slot::aggregate`) | — | Already derived from `SlotType::tested_value()`; D-02's variants land in the right family for free |

## Standard Stack

No new libraries. Everything this phase needs is already a resolved dependency of `pmcp-package`.

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `oci-spec` | 0.10.0 (resolved) | `Descriptor` / `ImageManifest` / `MediaType` / `Digest` types, and `ANNOTATION_TITLE` | Already the crate's OCI type source; chosen so a registry client consumes the manifests with zero translation (`Cargo.toml:23-25`) [VERIFIED: crates/pmcp-package/Cargo.toml:23-25 + `cargo metadata` resolved 0.10.0] |
| `olpc-cjson` | 0.1.4 (resolved) | Canonical-JSON formatter behind `digest::canonicalize` | Key-sorted, float-rejecting; the whole determinism story depends on it [VERIFIED: crates/pmcp-package/Cargo.toml:26-28] |
| `sha2` | 0.10.9 (resolved) | `ManifestDigest::from_bytes` | Pinned to 0.10 to match the repo-wide pin — "do NOT bump to 0.11 (breaking API change)" [VERIFIED: crates/pmcp-package/Cargo.toml:29-31] |
| `serde` / `serde_json` | 1 | Layer (de)serialization | Existing |
| `thiserror` | 2 | `PackageError` | Existing |
| `semver` | 1 (serde feature) | `ServerPackage.version` | Existing |
| `hex` | 0.4 | `sha256:<hex>` encoding | Existing |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tempfile` | 3.27.0 | OCI layout temp dirs in tests | Every pack/unpack test — "do NOT hand-roll temp dirs" [VERIFIED: crates/pmcp-package/Cargo.toml:45-46] |
| `proptest` | 1.11.0 | Property tests (already used for `aggregate` order-stability) | New slot variants + media-type-keyed lookup invariants |
| `toml` | 0.8.23 | **Dev-dependency ONLY today** | See Pitfall 1 — promoting it to a runtime dep is a scope-fence decision, not a mechanical one |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| A typed `MT_SERVER_BINARY_REF` JSON layer | An OCI layer descriptor pointing at an absent blob | OCI-native and zero extra bytes, but requires a **fabricated `size`** (`Descriptor::new(media_type, size, digest)` takes `u64`; the referenced binary's size is unknown at pack time) and breaks the crate's "every referenced blob is digest-verified" invariant (`oci/unpack.rs:33-38`). **Recommend the JSON layer.** |
| Promoting `toml` to a runtime dep of `pmcp-package` | Caller extracts `Vec<ConfigSlot>`; `pack_server` receives them typed | Keeps the scope fence intact, but moves D-04's placeholder validation out of the crate — see Pitfall 1 for the recommended split |
| `+yaml` structured-syntax suffix on the spec media type | Single suffix-free type (D-16, locked) | `+yaml` is a registered suffix (RFC 9512) and `+json` would need a second constant; D-16 locked one type, bytes verbatim, format inferred from the D-15 filename annotation. The existing `MT_SERVER_BOOTSTRAP` already uses the unregistered `+binary` suffix, so the crate is already suffix-loose — consistency wins. |

**Installation:** none. No `cargo add` in this phase (unless the `toml` promotion is chosen).

**Version verification:** performed against the local resolved graph rather than a registry guess:
```bash
cargo metadata --manifest-path crates/pmcp-package/Cargo.toml --format-version 1
# oci-spec 0.10.0 · olpc-cjson 0.1.4 · sha2 0.10.9 · semver 1.0.28 · proptest 1.11.0 · tempfile 3.27.0 · toml 0.8.23
```

## Package Legitimacy Audit

No new external packages are introduced. The three crates that could plausibly change role were checked anyway.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `toml` | crates.io | since 2014-11-11 | 15,205,670/wk | github.com/toml-rs/toml | OK | Approved (already a dev-dep; promotion is a design choice, not a supply-chain one) |
| `oci-spec` | crates.io | since 2019-05-28 | 315,107/wk | github.com/youki-dev/oci-spec-rs | OK | Approved (existing) |
| `olpc-cjson` | crates.io | since 2019-11-09 | 231,685/wk | github.com/awslabs/tough | OK | Approved (existing) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Runtime State Inventory

This is a **wire-format break** (D-09), so the "what still holds the old shape after every file is updated" question applies. Categories are answered explicitly.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **None** — `pmcp-package` writes only to a caller-supplied `OciLayout` directory. No database, no cache, no registry. Verified: `PackageError` has no network variant and the crate docs state "No `reqwest::Error` variant exists — this crate makes no HTTP calls" [VERIFIED: crates/pmcp-package/src/error.rs:26]. | none |
| Live service config | **None** in-repo. **One out-of-repo risk:** the crate's stated second consumer is "the `pmcp.run` platform — unpacks and validates packages at import/pre-flight time" [VERIFIED: crates/pmcp-package/src/lib.rs:11-13]. If pmcp.run already pins `pmcp-package = "0.1"`, D-09/D-10 strands it. CONTEXT.md D-10 asserts "no published consumers"; **the plan should record the evidence for that assertion**, because `pmcp-package` 0.1.1 *is* published to crates.io per the release workflow. | verify pmcp.run's pin before tagging 0.2.0 |
| OS-registered state | **None** — no scheduled tasks, services, or daemons reference the format. | none |
| Secrets/env vars | `TFL_APP_KEY` is set by `parity_replay.rs:239` (`std::env::set_var("TFL_APP_KEY", DUMMY_APP_KEY)`). If an endpoint slot introduces a new var (e.g. `TFL_BASE_URL`), the parity test must set it too or the server boots against a literal placeholder. | add the env var to the test harness |
| Build artifacts / checked-in generated files | **THREE artifacts are generated-and-checked-in and all move under D-08:** (1) `crates/pmcp-package/tests/golden_fixtures/server_team_fs_v1.json` — stored in *canonical byte form*, so it cannot merely keep an ignored `binary_ref` field; (2) `crates/pmcp-package/tests/golden_fixtures/canonical/server.canonical.json`; (3) the `EXPECTED_SERVER_DIGEST` constant. All three currently carry `binary_ref` — verbatim first bytes of both JSON files: `{"binary_ref":{"media_type":"application/x-lambda-bootstrap; arch=arm64"},"config_slots":[...` [VERIFIED: crates/pmcp-package/tests/golden_fixtures/server_team_fs_v1.json:1 and .../canonical/server.canonical.json:1]. | regenerate all three in the same commit |

**Canonical question answered:** after every `.rs` file is updated, what still holds the old shape? → the two checked-in JSON fixtures and one `const &str`, plus any out-of-repo consumer pinned at `0.1`.

## Architecture Patterns

### System Architecture Diagram

```
                     ┌──────────────────────────────────────────────┐
   author's files    │  london-tube.toml   london-tube-api.yaml     │
   (byte-verbatim)   │  (+ [[config_slots]] declarations, D-01)     │
                     └───────────────┬──────────────────────────────┘
                                     │
                    ┌────────────────▼─────────────────┐
                    │  CALLER (cargo-pmcp / test)      │
                    │  · parse TOML                    │
                    │  · extract Vec<ConfigSlot>       │
                    │  · D-04 placeholder validation   │  ← Pitfall 1: which side
                    │  · choose BinaryMode             │     of this line?
                    └────────────────┬─────────────────┘
                                     │ ServerPackage + raw bytes + BinaryMode
                    ┌────────────────▼─────────────────────────────────────┐
                    │  pack_server(package, binary: BinaryMode, layout)    │
                    │                                                      │
                    │  BinaryMode::Embedded(&[u8])   BinaryMode::Referenced│
                    │           │                            │             │
                    │           ▼                            ▼             │
                    │   MT_SERVER_BOOTSTRAP layer     MT_SERVER_BINARY_REF │
                    │   (raw bytes)                   layer ({digest,      │
                    │                                  media_type} JSON)   │
                    │           └──────────┬─────────────────┘             │
                    │                      │  EXACTLY ONE of the two       │
                    │   + MT_SERVER_ENVELOPE  (name/version/digest —       │
                    │                          NO binary_ref, D-08)        │
                    │   + MT_SERVER_DEPLOY_DESCRIPTOR                      │
                    │   + MT_SERVER_CEDAR_POLICY_SET                       │
                    │   + MT_SERVER_TOOL_METADATA                          │
                    │   + MT_SERVER_CONFIG_SLOTS                           │
                    │   + MT_SERVER_CONFIG        ◄── NEW (D-13)           │
                    │       annotations: {title: "london-tube.toml"}       │
                    │   + MT_SERVER_OPENAPI_SPEC  ◄── NEW, OPTIONAL (D-14) │
                    │       annotations: {title: "london-tube-api.yaml"}   │
                    └──────────────────────┬───────────────────────────────┘
                                           │
                    ┌──────────────────────▼───────────────────────────────┐
                    │  finalize_pack: empty-config blob → ImageManifest    │
                    │  → canonicalize(olpc-cjson) → sha256 → ManifestDigest│
                    │  (index.json annotations are set AFTER the digest    │
                    │   and therefore do NOT feed it)                      │
                    └──────────────────────┬───────────────────────────────┘
                                           │ ManifestDigest  ← D-12 golden pins THIS
                                           ▼
   ─────────────────────────────── layout on disk ────────────────────────────────
                                           │
                    ┌──────────────────────▼───────────────────────────────┐
                    │  unpack_server(layout)                               │
                    │    -> Result<(ServerPackage, UnpackedBinary)>        │
                    │                                                      │
                    │  1. read_the_one_manifest + verify config blob       │
                    │  2. 0.1.x DETECTION (D-10): envelope JSON carries a  │
                    │     `binary_ref` key?  → refuse, name the version    │
                    │  3. index layers BY MEDIA TYPE (D-11), not position  │
                    │  4. read_verified_blob each → deserialize            │
                    │  5. bootstrap layer present  → Embedded(Vec<u8>)     │
                    │     binary-ref layer present → Referenced{digest,mt} │
                    │     both / neither            → Layout error         │
                    └──────────────────────────────────────────────────────┘
```

### Recommended Project Structure

No new modules are required. Every change lands in files that already exist:

```
crates/pmcp-package/
├── src/oci/
│   ├── media_types.rs     # + MT_SERVER_CONFIG, MT_SERVER_OPENAPI_SPEC, MT_SERVER_BINARY_REF
│   ├── pack.rs            # BinaryMode param; ServerEnvelope loses binary_ref; +2..3 layers
│   └── unpack.rs          # media-type index; UnpackedBinary return; 0.1.x refusal
├── src/package/server.rs  # ServerPackage loses binary_ref (D-08); BinaryRef survives as a type
├── src/slot/types.rs      # + SlotType::Endpoint / SlotType::AuthMode (D-02); key() + tested_value() arms
├── src/slot/mod.rs        # + required_slots re-export (D-03)
├── src/slot/required.rs   # NEW — the D-03 required-slots function (a new file is cleaner than
│                          #        widening deviation.rs, which D-03 explicitly protects)
└── tests/
    ├── digest_stability.rs        # re-pin EXPECTED_SERVER_DIGEST; add the D-12 packed-manifest golden
    └── golden_fixtures/
        ├── server_team_fs_v1.json           # regenerate (canonical bytes)
        ├── canonical/server.canonical.json  # regenerate
        └── config_server_london_tube_v1/    # NEW — the D-12 config-only fixture inputs
```

### Pattern 1: Media-type-keyed layer lookup (D-11)

**What:** Replace the positional reads with a one-pass index from media-type string to `&Descriptor`.
**When to use:** Every `unpack_server` layer access, and the D-10 shape detection.

The code being replaced, verbatim:

```rust
// crates/pmcp-package/src/oci/unpack.rs:78-90 — CURRENT (positional)
    let layers = manifest.layers();
    let bootstrap_descriptor = layers.first().ok_or_else(|| missing_layer("bootstrap"))?;
    let envelope_descriptor = layers.get(1).ok_or_else(|| missing_layer("envelope"))?;
    let deploy_descriptor = layers
        .get(2)
        .ok_or_else(|| missing_layer("deploy-descriptor"))?;
    let cedar_descriptor = layers
        .get(3)
        .ok_or_else(|| missing_layer("cedar-policy-set"))?;
    let tools_descriptor = layers
        .get(4)
        .ok_or_else(|| missing_layer("tool-metadata"))?;
    let config_slots_descriptor = layers.get(5).ok_or_else(|| missing_layer("config-slots"))?;
```

Replacement shape (BTreeMap keeps the debug output deterministic; a duplicate media type must be an error, not a silent last-wins):

```rust
fn index_layers(manifest: &ImageManifest) -> Result<BTreeMap<String, &Descriptor>> {
    let mut by_type = BTreeMap::new();
    for descriptor in manifest.layers() {
        let key = descriptor.media_type().to_string();
        if by_type.insert(key.clone(), descriptor).is_some() {
            return Err(PackageError::Layout {
                reason: format!("manifest carries two '{key}' layers; layer media types must be unique"),
            });
        }
    }
    Ok(by_type)
}
```

**Why a duplicate check matters:** the existing `missing_layer` helper already gives a named error (`oci/unpack.rs:66-70`), so the "missing" half is covered. The *duplicate* half is new risk introduced by dropping positional guarantees.

### Pattern 2: Descriptor annotations for original filenames (D-15)

**What:** Attach `org.opencontainers.image.title` to the config and spec layer descriptors.
**When to use:** Both new layers. Note the constant already exists — do **not** hand-roll the string.

```rust
// oci-spec 0.10.0 exports the constant and the setter; no new dependency.
use oci_spec::image::ANNOTATION_TITLE;

let mut descriptor = layout.write_blob(vendor_media_type(MT_SERVER_CONFIG), config_bytes)?;
descriptor.set_annotations(Some(HashMap::from([
    (ANNOTATION_TITLE.to_string(), config_file_name.to_string()),
])));
```

Source of truth, verbatim:

```rust
// ~/.cargo/registry/src/index.crates.io-.../oci-spec-0.10.0/src/image/annotations.rs:43-45
/// AnnotationTitle is the annotation key for the human-readable title of the
/// image.
pub const ANNOTATION_TITLE: &str = "org.opencontainers.image.title";
```

```rust
// ~/.cargo/registry/src/index.crates.io-.../oci-spec-0.10.0/src/image/descriptor.rs:52-55
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    annotations: Option<HashMap<String, String>>,
```

Re-export confirmed: `pub use annotations::*;` [VERIFIED: oci-spec-0.10.0/src/image/mod.rs:17].

**Precedent:** the crate already does exactly this for the *index* descriptor — `manifest_descriptor.set_annotations(Some(annotations))` at `crates/pmcp-package/src/oci/pack.rs:180`. The new work is applying it to *layer* descriptors, which — unlike the index case — **does** feed the manifest digest (see Pitfall 5).

### Pattern 3: The binary-mode enum pair (D-06)

**What:** Symmetric enums on the parameter and the return, so `Referenced` has no `Vec<u8>` to reach for.

```rust
/// Pack-time input. Borrowed on the embedded arm — no copy of the binary.
pub enum BinaryMode<'a> {
    Embedded(&'a [u8]),
    Referenced { digest: ManifestDigest, media_type: String },
}

/// Unpack-time output. Owned on the embedded arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnpackedBinary {
    Embedded(Vec<u8>),
    Referenced { digest: ManifestDigest, media_type: String },
}
```

Note `digest: ManifestDigest` is **non-optional** here, unlike the existing `BinaryRef`:

```rust
// crates/pmcp-package/src/package/server.rs:360-365 — CURRENT
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::digest::ManifestDigest>,
    pub media_type: String,
}
```

`BinaryRef` should **survive as the serialized payload type** of the new binary-ref layer (with `digest` still `Option` for wire tolerance), while `BinaryMode`/`UnpackedBinary` are the non-optional *API* types. That gives D-06 its type-level guarantee without a second wire schema, and gives unpack a clean place to convert `None` → `PackageError::Layout { reason: "binary-ref layer carries no digest" }`.

### Pattern 4: Deriving the new slot family for free (D-02)

`classify` derives the family from `tested_value()` — verbatim:

```rust
// crates/pmcp-package/src/slot/classification.rs:24-30
pub fn classify(slot: &SlotType) -> SlotClass {
    if slot.tested_value().is_some() {
        SlotClass::BehaviorRelevant
    } else {
        SlotClass::IdentityBearing
    }
}
```

So adding `Endpoint { name, tested_value }` and `AuthMode { name, tested_value }` requires editing exactly two `match` arms:

```rust
// crates/pmcp-package/src/slot/types.rs:80-99 — the two impls to extend
    pub fn key(&self) -> (&'static str, &str) { /* add ("endpoint", name), ("auth_mode", name) */ }
    pub fn tested_value(&self) -> Option<&str> {
        match self {
            SlotType::LlmProvider { tested_value, .. }
            | SlotType::BudgetOverride { tested_value, .. } => Some(tested_value.as_str()),
            _ => None,
        }
    }
```

⚠ **The `_ => None` wildcard at `types.rs:97` will silently swallow the new variants.** If `Endpoint`/`AuthMode` are added without extending this arm, they classify as `IdentityBearing`, `detect_deviation` never fires for them, and `classify`'s own tests still pass. Recommend replacing the wildcard with explicit arms so a future variant is a compile error. This is the single highest-value one-line change in the phase.

Serialization is already forward-compatible — verbatim:

```rust
// crates/pmcp-package/src/slot/types.rs:19-24
/// Deliberately NOT `#[serde(deny_unknown_fields)]` — forward-compatible for future slot
/// kinds (RESEARCH Pitfall 4): an older reader silently ignores fields it doesn't know about
/// rather than hard-failing on a newer producer's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlotType {
```

Note the tolerance is for unknown **fields**, not unknown **variants** — an internally-tagged enum still hard-fails on an unrecognised `type` discriminator. CONTEXT.md D-02's note ("older readers tolerate the addition") is therefore only half-true: a 0.1.x reader hitting `{"type":"endpoint",...}` gets `unknown variant`. D-10 makes this moot for packages, but it is worth stating so nobody relies on it.

### Anti-Patterns to Avoid

- **Re-deriving a digest from a parsed struct.** The crate's own rule: "digest what you actually stored, never re-derive from the parsed struct" [VERIFIED: crates/pmcp-package/src/digest/verify.rs:9-11]. The config and spec layers must be digested over the **bytes read from the author's file**, never over a re-serialized `ServerConfig`.
- **Reading a new layer without `read_verified_blob`.** `oci/unpack.rs:33-38` is the only sanctioned path; it verifies before deserialize and is what makes the tamper tests meaningful.
- **Adding a third binary mode.** Rejected in D-05. A config-only server *is* the referenced mode.
- **Widening `detect_deviation`.** D-03 protects it, and its test `never_flags_identity_bearing_slots` (`slot/deviation.rs:69-78`) asserts the invariant that widening would break.
- **Loosening `deny_unknown_fields` on `ServerConfig` to make `[[config_slots]]` parse.** The toolkit states this explicitly: "Do NOT loosen `deny_unknown_fields` to make a fixture parse. Always ADD the missing field." [VERIFIED: crates/pmcp-server-toolkit/src/config.rs:23-24].
- **Treating `HashMap` iteration order as stable in anything that feeds a digest.** `Descriptor.annotations` is a `HashMap`; olpc-cjson sorts keys at serialization so the *canonical bytes* are safe, but any hand-built comparison or debug-print is not.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Original-filename annotation key | A `pmcp`-namespaced `filename` annotation | `oci_spec::image::ANNOTATION_TITLE` | Standard, registry-native, and already the ORAS convention for exactly this (`"org.opencontainers.image.title": "hello.txt"`) |
| SHA-256 of a blob | `sha2::Sha256` directly | `ManifestDigest::from_bytes` | Already the crate's single primitive; `cargo-pmcp` routes through it even for bare-hex comparisons (`aws_lambda/artifact.rs:330-337`) |
| Canonical JSON | `serde_json::to_vec` | `digest::canonicalize` | `serde_json` does not key-sort; the digest would be non-deterministic |
| Temp directories in tests | `std::env::temp_dir()` + manual cleanup | `tempfile::tempdir()` | Explicit dev-dep with the stated rule "do NOT hand-roll temp dirs" |
| Vendor `MediaType` wrapping | A parallel media-type enum | `vendor_media_type(&str)` | Routes any `application/vnd.pmcp.*` to `MediaType::Other`; documented as "RESEARCH Pattern 3 — never a hand-rolled parallel media-type enum" (`media_types.rs:27-28`) |
| A constant-vs-computed drift guard | An ad-hoc comment | The `empty_config_digest_matches_hash_of_empty_json_blob` pattern (`media_types.rs:132-136`) | In-crate precedent for pinning a `const` against its computed value |
| `${VAR}` expansion | A new resolver in `pmcp-package` | `pmcp-server-toolkit`'s existing `resolve_credential` (`http/auth.rs:510`) / `expand_braced_var` (`code_mode.rs:1173`) | Two resolvers with different edge-case behaviour on `${}` would be a latent security bug; the toolkit's already documents the malformed-brace case |

**Key insight:** `pmcp-package` is a *format* crate whose value is that two independent consumers resolve identical behaviour from it (`src/lib.rs:14-18`). Every hand-rolled primitive is a place those two consumers can diverge.

## Common Pitfalls

### Pitfall 1: `pack` cannot read TOML without breaching the crate's scope fence

**What goes wrong:** D-01 says slots are declared in the server's `config.toml` and "`pack` reads them", and D-04 says "`pack` **validates** that every key named by a `[[config_slots]]` entry already holds a `${VAR}` placeholder." Both require parsing TOML inside `pmcp-package`.

**Why it happens:** `toml` is a **dev-dependency only**, and the Cargo.toml says so in as many words — verbatim:

```toml
# crates/pmcp-package/Cargo.toml:43-50
[dev-dependencies]
proptest = "1.11"
# Explicit temp-dir dev-dep for the OCI layout tests — do NOT hand-roll temp dirs.
tempfile = "3"
# Dev-only: the crate's runtime format is JSON; TOML parsing is used ONLY by a
# fixture-coverage test that parses tracked .pmcp/deploy.toml files. Never a
# runtime dependency.
toml = "0.8"
```

The single in-crate `toml::from_str` call is inside a `#[cfg(test)]` fixture-coverage test [VERIFIED: crates/pmcp-package/src/package/server.rs:592]. The crate's scope fence also says it is "format only" and lists what it explicitly does **not** contain (`src/lib.rs:20-38`).

**How to avoid — three viable splits, recommend (B):**

- **(A) Promote `toml` to a runtime dependency.** Simplest code, but adds a parser to a format crate whose whole design premise is a minimal, dual-consumer-safe surface. Also means `pmcp-package` now has an opinion about the *toolkit's* config schema, coupling two crates that are deliberately independent.
- **(B) Split the seam: caller parses, crate validates structurally.** `pack_server` takes `config_bytes: &[u8]`, `config_file_name: &str`, and the already-extracted `Vec<ConfigSlot>` (which `ServerPackage.config_slots` *already is*). D-04's placeholder validation lives in a new, `toml`-gated helper — either behind an optional `toml` cargo feature in `pmcp-package`, or in `cargo-pmcp`/`pmcp-server-toolkit` where a TOML parser already lives. The layer bytes stay byte-verbatim either way, which is what criterion 1 actually asserts. **Recommended.**
- **(C) Optional `toml` feature on `pmcp-package`.** `[features] config-slots = ["dep:toml"]`. Keeps the default build lean and the validation in-crate. Costs a feature axis and a second CI build combination on a crate whose gate currently runs one configuration.

**Warning signs:** a plan task that says "pack reads `[[config_slots]]`" without naming which crate owns the TOML parse.

### Pitfall 2: `[[config_slots]]` in `london-tube.toml` will not parse — the server refuses to boot

**What goes wrong:** Criterion 1 packs `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml`. D-01 requires that file to carry a `[[config_slots]]` block. The runtime that must boot from that same file rejects it.

**Why it happens:** verbatim —

```rust
// crates/pmcp-server-toolkit/src/config.rs:100-102
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
```

An unknown top-level key is a hard `ToolkitError::Parse`, asserted by the toolkit's own test `test_backend_unknown_field_rejected` (`config.rs:1275-1293`).

**Blast radius (both currently-green tests fail at parse):**
- `crates/pmcp-openapi-server/tests/parity_replay.rs:107` — `ServerConfig::from_toml_strict_validated(&config_text).expect("vendored london-tube.toml parses + validates")`
- `crates/pmcp-openapi-server/tests/parity_replay.rs:84` — the same for `crates/pmcp-openapi-server/examples/london-tube.toml`, which is a **second copy** of the config that also has `base_url = "https://api.tfl.gov.uk"` (`examples/london-tube.toml:32`) and is easy to miss.

**How to avoid:** add a `#[serde(default)] pub config_slots: Vec<ConfigSlotDecl>` field to `ServerConfig`, per the toolkit's own doctrine ("Always ADD the missing field"). Keep `ConfigSlotDecl` a toolkit-local type — the toolkit must **not** take a dependency on `pmcp-package` (that would invert the layering; `pmcp-package` is the leaf). The mapping `ConfigSlotDecl → pmcp_package::ConfigSlot` belongs in the packing caller.

**Warning signs:** any plan that touches `london-tube.toml` without a paired `pmcp-server-toolkit` task.

### Pitfall 3: `base_url = "${TFL_BASE_URL}"` parses, validates, and then silently breaks at request time

**What goes wrong:** D-04 requires the endpoint key to hold a placeholder. The toolkit will accept it and then use it verbatim as a URL.

**Why it happens:** `base_url` is a plain `String` with no expansion — verbatim:

```rust
// crates/pmcp-server-toolkit/src/config.rs:443-448
pub struct BackendSection {
    /// REST API root URL (e.g. `"https://api.tfl.gov.uk"`). Single-call tools
    /// concatenate their `path` onto this (an empty per-tool `base_url`
    /// inherits this value).
    #[serde(default)]
    pub base_url: String,
```

`validate()` only rejects empty/whitespace (`config.rs:263`), so `"${TFL_BASE_URL}"` passes. `${VAR}` expansion exists in the toolkit **only** for auth credentials (`http/auth.rs:510` `resolve_credential`) and `code_mode.token_secret` (`code_mode.rs:1173-1204`) — grep for `${` across `crates/pmcp-server-toolkit/src` returns hits in exactly those two areas plus doc comments.

**Second, harder break:** the parity harness asserts on the literal. Verbatim:

```rust
// crates/pmcp-openapi-server/tests/parity_replay.rs:216-225
fn temp_config_pointing_at(backend_url: &str) -> String {
    const REFERENCE_BASE_URL: &str = r#"base_url = "https://api.tfl.gov.uk""#;
    let reference = std::fs::read_to_string(fixtures_dir().join("london-tube.toml"))
        .expect("read london-tube.toml");
    assert!(
        reference.contains(REFERENCE_BASE_URL),
        "london-tube.toml must contain the base_url line to override"
    );
    reference.replace(REFERENCE_BASE_URL, &format!("base_url = \"{backend_url}\""))
}
```

That `assert!` panics the moment the literal becomes a placeholder — this is the direct answer to CONTEXT.md's **Open for planning #3**.

**How to avoid:** two coordinated changes. (1) Add `${VAR}`/`env:VAR` expansion for `base_url` in the toolkit, reusing `resolve_credential`'s semantics rather than writing a second resolver. (2) Rewrite `temp_config_pointing_at` to set the env var instead of string-replacing — which is *simpler* than what it does today and removes the brittle literal assertion:

```rust
// after: no string surgery, no literal assertion
std::env::set_var("TFL_BASE_URL", backend.uri());
```

**Warning signs:** a green `london_tube_fixture` test alongside a failing `london_tube_parity_through_real_binary_path` — that combination means the config parsed but the endpoint never resolved.

### Pitfall 4: D-04's placeholder rule is unsatisfiable for the auth-mode key

**What goes wrong:** PKG-03 requires auth mode to be a slot. D-01 declares slots by key. D-04 requires every slot-declared key to hold `${VAR}`. `type = "${AUTH_MODE}"` cannot deserialize.

**Why it happens:** `AuthConfig` is an internally-tagged enum — verbatim:

```rust
// crates/pmcp-server-toolkit/src/http/auth.rs:58-60
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
```

The tag selects the variant *and* the variant's field set. `${AUTH_MODE}` is not one of `none | api_key | bearer | basic | oauth2_client_credentials | oauth_passthrough`, so serde fails with `unknown variant`. There is no placeholder form that both parses and defers.

**This is a genuine conflict between two locked decisions (D-01/D-04) and the codebase — it needs a user decision before planning.** Three resolutions, in increasing cost:

1. **Carve-out (recommended):** scope D-04's placeholder validation to slots whose `SlotType` is a *value* slot (`Secret`, `Endpoint`), and exempt `AuthMode` as a structural/discriminator slot. Cheapest, honest, and CONTEXT.md itself already calls `[backend.auth] type` "a structural key" (`120-CONTEXT.md` canonical-refs, `london-tube.toml` entry) — so the carve-out is arguably what was meant.
2. **Per-slot opt-out:** `[[config_slots]]` entries carry `templated = false`. More general, more schema.
3. **Restructure `AuthConfig`** to an adjacently-tagged or two-field shape so the mode is a plain string. Wire break in a second crate; out of proportion.

**Warning signs:** a plan task asserting "all three PKG-03 slots hold `${VAR}` placeholders" — the auth-mode one cannot.

### Pitfall 5: layer-descriptor annotations move the digest; index annotations do not

**What goes wrong:** Someone assumes annotations are "just metadata" and adds/renames one after the D-12 golden is pinned.

**Why it happens:** the two annotation sites behave oppositely. Verbatim:

```rust
// crates/pmcp-package/src/oci/pack.rs:173-182
    let manifest_bytes = canonicalize(&manifest)?;
    let mut manifest_descriptor = layout.write_manifest(&manifest_bytes)?;

    let annotations = HashMap::from([
        ("name".to_string(), name.to_string()),
        ("version".to_string(), version.to_string()),
    ]);
    manifest_descriptor.set_annotations(Some(annotations));

    let manifest_digest = ManifestDigest::try_from(manifest_descriptor.digest())?;
```

The index annotations are set on the descriptor **after** `write_manifest` already computed the digest from `manifest_bytes` — so `name`/`version` do **not** feed the manifest digest. The new *layer* annotations are inside `manifest` before `canonicalize`, so they **do**. CONTEXT.md D-15 flags this ("a filename change moves the digest"); the plan should encode it as a comment next to the golden constant.

**Consequence for D-12:** the pinned packed-manifest digest is a function of `{schemaVersion, mediaType, artifactType, config descriptor, and for each layer in order: mediaType + size + digest + annotations}`. It is **not** a function of the package's name or version. A plan that "proves" the golden moves by bumping the version will get a false green.

### Pitfall 6: three checked-in artifacts move together, and one of them is not obvious

**What goes wrong:** `EXPECTED_SERVER_DIGEST` gets re-pinned, `server.canonical.json` gets regenerated, and `server_team_fs_v1.json` is left alone because `ServerPackage` has no `deny_unknown_fields` so the stale `binary_ref` "just gets ignored."

**Why it happens:** the input fixture is itself stored in canonical byte form and asserted byte-identical. `roundtrip.rs:5-12` states it outright: the fixtures are "stored in CANONICAL byte form (the exact bytes `canonicalize()` emits...)" and "the byte-identical assertion below therefore compares the fixture bytes against `canonicalize(&parsed)`". Dropping a field from the struct changes `canonicalize(&parsed)` but not the file — the assertion fails.

**How to avoid:** treat the three as one atomic change. The wire-freeze comment D-09 acts on is verbatim:

```rust
// crates/pmcp-package/tests/digest_stability.rs:37-38
// FAILS CI. This is a real wire freeze for the 0.1.x line, NOT just
// determinism: the day these must change is the day the format goes 0.2.0.
```

### Pitfall 7: the `_ => None` wildcard silently mis-classifies the new slot variants

Covered under Pattern 4. Restated here because it is the failure that produces **passing tests and wrong behaviour** — `Endpoint`/`AuthMode` would classify as `IdentityBearing`, so `required_slots` might still list them (D-03 returns both families) while `detect_deviation` never fires, and Phase 121's parity story quietly loses its endpoint check.

### Pitfall 8: `detect_kind`'s fallback path degrades for config-only packages

**What goes wrong:** `cargo pmcp package inspect` mis-detects a config-only package as "unknown kind" in a fallback scenario.

**Why it happens:** verbatim —

```rust
// cargo-pmcp/src/commands/package/kind.rs:49-57
pub fn detect_kind(s: &str) -> Option<PackageKind> {
    match s {
        ARTIFACT_TYPE_AGENT | MT_AGENT_CONFIG => Some(PackageKind::Agent),
        ARTIFACT_TYPE_TEAM | MT_TEAM_CONFIG => Some(PackageKind::Team),
        ARTIFACT_TYPE_SERVER | MT_SERVER_ENVELOPE => Some(PackageKind::Server),
        ARTIFACT_TYPE_WORKFLOW | MT_WORKFLOW_MANIFEST => Some(PackageKind::Workflow),
        _ => None,
    }
}
```

`artifact_type_from_manifest_json` prefers the top-level `artifactType` (`kind.rs:66-68`), which `finalize_pack` always sets — so the **primary path is safe**. But the third-choice fallback reads `layers[0].mediaType` (`kind.rs:76-81`); with the bootstrap layer gone, `layers[0]` is no longer `MT_SERVER_BOOTSTRAP`-adjacent and the fallback returns `None`. Low severity, easy fix: add the new server media types to the `Server` arm. Worth a task so the phase does not leave a latent regression for Phase 123's CLI work.

## Code Examples

### Verifying criterion 3's "one byte of the spec moves the digest, and `verify` rejects the stale one"

```rust
// The two primitives, verbatim from source:
//   crates/pmcp-package/src/digest/verify.rs:28  →  pub fn verify(expected: &ManifestDigest, bytes: &[u8]) -> Result<()>
//   crates/pmcp-package/src/oci/pack.rs:149-155  →  finalize_pack(..) -> Result<ManifestDigest>

#[test]
fn one_byte_change_in_the_spec_moves_the_manifest_digest_and_verify_rejects_the_stale_one() {
    let spec = std::fs::read(fixture("london-tube-api.yaml")).unwrap();
    let mut mutated = spec.clone();
    mutated[0] ^= 0x01;

    let dir_a = tempfile::tempdir().unwrap();
    let layout_a = OciLayout::create(dir_a.path()).unwrap();
    let digest_a = pack_config_server(&package, &config_bytes, Some(&spec), &layout_a).unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let layout_b = OciLayout::create(dir_b.path()).unwrap();
    let digest_b = pack_config_server(&package, &config_bytes, Some(&mutated), &layout_b).unwrap();

    assert_ne!(digest_a, digest_b, "the spec is BAKED — a one-byte change is a different package");

    // `verify` is an integrity check over bytes, so "rejects the stale digest" means:
    // the OLD digest does not verify against the NEW manifest's canonical bytes.
    let manifest_bytes_b = std::fs::read(
        dir_b.path().join("blobs").join("sha256").join(digest_b.as_str().trim_start_matches("sha256:")),
    ).unwrap();
    let err = verify(&digest_a, &manifest_bytes_b).unwrap_err();
    assert!(matches!(err, PackageError::DigestMismatch { .. }));
}
```

### Asserting the type-level guarantee PKG-02 asks for

The strongest form is a **compile-fail** test, not a runtime assertion — mirroring the crate's own precedent (`slot/types.rs:191-201` documents a "compile-documented proof" for `Secret`). Since `pmcp-package` has no `trybuild` dev-dep, the pragmatic in-crate form is an exhaustive `match` with no wildcard:

```rust
#[test]
fn a_referenced_package_exposes_no_bytes() {
    let (package, unpacked) = unpack_server(&layout).unwrap();
    match unpacked {
        UnpackedBinary::Referenced { digest, media_type } => {
            assert!(!digest.as_str().is_empty());
            assert!(!media_type.is_empty());
            // There is no third field. Adding one would fail to compile here.
        },
        UnpackedBinary::Embedded(_) => panic!("a config-only package must unpack as Referenced"),
    }
    let _ = package;
}
```

### The D-12 packed-manifest golden (contrast with the existing struct-level goldens)

```rust
// EXISTING pattern — pins manifest_digest over a STRUCT. Blind to layer set/order/media types.
// crates/pmcp-package/tests/digest_stability.rs:76-83
manifest_digest(&server_fixture()).unwrap().as_str() == EXPECTED_SERVER_DIGEST

// D-12 pattern — pins the value finalize_pack RETURNS. Sees all three.
const EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST: &str = "sha256:<pinned at authoring time>";

#[test]
fn config_only_server_packed_manifest_digest_matches_pinned_wire_freeze_constant() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let digest = pack_config_server(&fixture_package(), &config_bytes(), Some(&spec_bytes()), &layout).unwrap();
    assert_eq!(
        digest.as_str(),
        EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST,
        "packed layer set / layer order / media-type strings changed — a previously published CLI \
         can no longer read this package. Bump intentionally, do not silently repin."
    );
}
```

**Authoring procedure** (the existing goldens used the same trick, per `roundtrip.rs:8-10`): write the test with a placeholder constant, run it once, copy the `actual` from the failure message. Do **not** compute the digest by a second code path — that would defeat the "one hash, one source of truth" rule at `pack.rs:11-14`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Config-descriptor `mediaType` used to signal artifact type | Top-level `artifactType` field, with a standard empty config descriptor | OCI image-spec 1.1 | The crate already does this (`ARTIFACT_TYPE_SERVER`, `MT_EMPTY_CONFIG`), so nothing to change — but it is why `detect_kind`'s primary path is safe (Pitfall 8) |
| Artifact manifests as a separate `application/vnd.oci.artifact.manifest.v1+json` type | Reuse the image manifest with `artifactType` | OCI 1.1 (artifact manifest was removed before final) | Confirms the crate's choice; do not reintroduce `oci_spec::image::artifact` |
| `binary_ref` as a typed field on `ServerPackage` | Binary identity expressed only through layers (D-08) | This phase | Aligns `binary_ref` with the crate's own stated rule that binary payloads are OCI layers, not struct fields (`media_types.rs:37-39`) |

**Deprecated/outdated:**
- Positional layer indexing (`oci/unpack.rs:79-90`) — replaced by D-11. The module doc that describes it ("Layer order mirrors [`super::pack`]'s push order exactly", `unpack.rs:13-17`) must be rewritten in the same change or it becomes a lie that a future reader will trust.
- `ServerEnvelope.binary_ref` (`oci/pack.rs:46`) — removed by D-08; its doc comment at `media_types.rs:13-20` also names `binary_ref` and must be updated.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `pmcp-package` 0.1.1 has **no published consumers**, so D-09/D-10 strand nobody. CONTEXT.md D-10 asserts this; the crate *is* published (release.yml:440-455) and `src/lib.rs:11-13` names the pmcp.run platform as a consumer. Not verified against pmcp.run's actual pin this session. | Runtime State Inventory | A 0.2.0 tag breaks a live platform importer. Cheap to check before tagging; expensive after. |
| A2 | A new `MT_SERVER_BINARY_REF` JSON layer is preferable to an absent-blob layer descriptor. Reasoned from `Descriptor::new`'s required `u64` size and the crate's verify-everything invariant, not from an external precedent. | Alternatives Considered / Pattern 3 | If the pmcp.run importer expects an OCI-native reference shape, the JSON layer needs translation there. |
| A3 | Adding `config_slots: Vec<ConfigSlotDecl>` to `pmcp-server-toolkit::ServerConfig` is additive and safe. Follows the toolkit's stated doctrine and the `[backend]` precedent (`config.rs:115-125`), but the REF-01 superset test (`tests/reference_configs.rs`) was not read this session. | Pitfall 2 | The superset test may need updating; low severity, but it is a second test file the plan should name. |
| A4 | The recommended carve-out for Pitfall 4 (exempt `AuthMode` from D-04's placeholder rule) matches user intent. CONTEXT.md calls the auth-mode key "structural", which supports it, but D-04's text is unqualified. | Pitfall 4 | Planning against the wrong reading produces an unimplementable task. **Confirm with the user.** |
| A5 | Phase 121's `parity_replay.rs` reuse is unaffected by rewriting `temp_config_pointing_at` to use an env var. Only lines 216-225 and 248 were read in detail; the scenario YAML was not. | Pitfall 3 | A scenario step may assert on the backend URL; would surface immediately as a test failure, not silently. |

## Open Questions (RESOLVED)

These are CONTEXT.md's three "Open for planning" items, now answered with evidence.
All four (including the extra scope question research surfaced) are RESOLVED — recommendations
were carried into the plans as PR-01 (plan 01), the no-completeness-check decision, the base_url
expansion tasks (plan 04), and the two-wave split (CONTEXT.md amendment D-18).

1. **Where the referenced runtime digest comes from at pack time.**
   - What we know: `pmcp-package` cannot derive it — milestone Decision 2 forbids an OCI registry client, and the deploy descriptor has **no** digest field (`ServerSection.binary: Option<String>` at `package/server.rs:107` is documented at `:88` as a "google-cloud-run-target-only extra", a binary *name*, not a digest). Meanwhile `cargo-pmcp` **already produces exactly this value**: `aws_lambda/artifact.rs` fetches prebuilt Shape A binaries (`pmcp-sql-server` / `pmcp-openapi-server` / `pmcp-workbook-server`) from GitHub release assets with a `.sha256` sidecar, and `sha256_hex` (`artifact.rs:330-337`) computes it via `pmcp_package::digest::ManifestDigest::from_bytes`.
   - What's unclear: nothing material.
   - RESOLVED: **Recommendation:** `BinaryMode::Referenced { digest: ManifestDigest, media_type: String }` takes the digest **verbatim from the caller, non-optional**. `pmcp-package` treats it as opaque. Document that the canonical producer is the release-asset checksum and that the bare-hex sidecar must be prefixed `sha256:` to become a `ManifestDigest`.

2. **Whether `[[config_slots]]` needs a completeness check.**
   - What we know: D-04 catches the inverse (declared slot holding a literal). Nothing catches an undeclared environment-specific literal. A structural heuristic is possible but the fixture shows why it is unreliable: `base_url` is a bare literal that *should* be a slot, `token_secret = "london-tube-parity-dev-secret-32bytes"` is a bare literal that is **deliberately** inline and guarded by `allow_inline_token_secret_for_dev = true` (`london-tube.toml:214-220`) — a naive "literal that looks secret ⇒ missing slot" check would flag it.
   - What's unclear: whether the milestone wants a heuristic at all.
   - RESOLVED: **Recommendation:** do **not** add a completeness check in Phase 120. Instead make the *absence* visible: have `required_slots` (D-03) render into `cargo pmcp package inspect` output in Phase 123, so an author sees "this package declares 2 slots" and notices the third is missing. A heuristic that cries wolf on the dev token_secret is worse than none.

3. **How `base_url` becomes a `${VAR}` placeholder without breaking `parity_replay.rs`.**
   - What we know: fully answered in Pitfall 3. Two blockers (no `base_url` expansion; a literal-matching `assert!` at `parity_replay.rs:220-223`), plus the second config copy at `examples/london-tube.toml:32`.
   - RESOLVED: **Recommendation:** add `${VAR}`/`env:VAR` expansion for `base_url` in `pmcp-server-toolkit` reusing `resolve_credential`'s semantics, then replace `temp_config_pointing_at`'s string surgery with `std::env::set_var("TFL_BASE_URL", backend.uri())`. Net: the test gets *shorter*.

**One additional open question research surfaced, not in CONTEXT.md:**

4. **Does the phase change one crate or two?** CONTEXT.md's canonical-refs scope the work to `pmcp-package` and list `pmcp-openapi-server` only as "the proving case". Pitfalls 2 and 3 show that criterion 3 cannot be honestly demonstrated without additive changes to `pmcp-server-toolkit`. RESOLVED: **Recommendation:** plan two waves (see Primary recommendation) and make the toolkit changes explicit tasks rather than incidental edits.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | everything | ✓ | rustc 1.98.0 / cargo 1.98.0 | — |
| `cargo fmt` / `cargo clippy` | `make pmcp-package-gate` | ✓ | bundled | — |
| `pmat` | CI quality gate (complexity) | ✓ | 3.15.0 (pinned version matches) | — |
| `just` | project scripts | ✓ | 1.46.0 | Makefile is the actual gate here |
| `cargo-nextest` | optional test runner | ✓ | present | plain `cargo test` — **use it**; see Validation Architecture |
| Network | none required | n/a | — | Every test in scope is offline (`wiremock` + local OCI layout) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` 1.11.0 (+ `tokio::test` on the openapi-server side) |
| Config file | none — `crates/pmcp-package/Cargo.toml` (its **own** `[workspace]` table at line 6) |
| Quick run command | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` — **130 tests, 0.1s** (measured this session) |
| Full suite command | `make pmcp-package-gate` (fmt + clippy `-D warnings` + test), plus `cargo test -p pmcp-openapi-server` and `cargo test -p pmcp-server-toolkit` for the Wave B changes |

The standalone gate, verbatim:

```makefile
# Makefile:871-877
.PHONY: pmcp-package-gate
pmcp-package-gate:
	@echo "$(BLUE)🔍 pmcp-package standalone gate (workspace-excluded crate)$(NC)"
	$(CARGO) fmt --manifest-path crates/pmcp-package/Cargo.toml --all -- --check
	$(CARGO) clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -D warnings
	$(CARGO) test --manifest-path crates/pmcp-package/Cargo.toml
	@echo "$(GREEN)✓ pmcp-package fmt/clippy/test OK$(NC)"
```

It is chained into `quality-gate` at `Makefile:896`. **The CONTEXT.md caveat that root commands do not reach `pmcp-package` is confirmed true (`Cargo.toml:6` empty `[workspace]`, root `Cargo.toml:831` `exclude = [... "crates/pmcp-package" ...]`) — but the Makefile already closes that hole.** Plan verify blocks should call `make pmcp-package-gate`, not a bare `cargo test`.

⚠ **Do not write `cargo nextest run -E 'test(/foo/)'` in a verify block.** Per project memory, the `test()` selector silently selects zero tests and exits 0. Use `binary(<name>)` or plain `cargo test <substring>`.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PKG-01 | Config-only package packs with **no** bootstrap layer; config + spec layers present under the new media types | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml config_only` | ❌ Wave 0 |
| PKG-01 | `unpack_server` restores `london-tube.toml` and `london-tube-api.yaml` **byte-identically** under their original filenames (D-15) | integration | same | ❌ Wave 0 |
| PKG-01 | Spec layer is optional — a package with no spec packs and unpacks (D-14) | unit | same | ❌ Wave 0 |
| PKG-02 | Embedded mode round-trips bootstrap bytes (existing behaviour preserved) | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml server_pack_then_unpack` | ✅ `src/oci/unpack.rs:372` (adapt to the new signature) |
| PKG-02 | Referenced mode round-trips with **no bootstrap blob in the layout** | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml referenced` | ❌ Wave 0 |
| PKG-02 | Unpacking a referenced package reports the digest instead of a missing-layer error (D-07) | unit | same | ❌ Wave 0 |
| PKG-02 | A referenced package exposes no bytes (exhaustive `match`, no wildcard) | unit | same | ❌ Wave 0 |
| PKG-02 | Both-layers-present and neither-layer-present are `PackageError::Layout`, not panics | unit | same | ❌ Wave 0 |
| PKG-02 | A 0.1.x-shaped package is **refused with a named error**, not mis-deserialized (D-10) | unit | same | ❌ Wave 0 |
| PKG-03 | `SlotType::Endpoint` / `AuthMode` classify as `BehaviorRelevant` via `tested_value()` | unit | `cargo test --manifest-path crates/pmcp-package/Cargo.toml classif` | ✅ `src/slot/classification.rs:32` (add 2 cases) |
| PKG-03 | `aggregate` returns the three slots in deterministic order with no spec-derived slot | unit | `cargo test --manifest-path crates/pmcp-package/Cargo.toml aggregate` | ✅ `src/slot/aggregate.rs:57` (add a case) |
| PKG-03 | `detect_deviation` is **unchanged** — still `None` for identity-bearing (D-03 regression guard) | unit | `cargo test --manifest-path crates/pmcp-package/Cargo.toml deviation` | ✅ `src/slot/deviation.rs:49` |
| PKG-03 | `required_slots` returns identity-bearing **and** behavior-relevant slots | unit | `cargo test --manifest-path crates/pmcp-package/Cargo.toml required_slots` | ❌ Wave 0 |
| PKG-03 | One byte of the spec moves the manifest digest; `verify` rejects the stale digest | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml spec_byte` | ❌ Wave 0 |
| PKG-03 | Pack **rejects** a slot-declared key holding a resolved literal (D-04) | unit | wherever the validation lands (see Pitfall 1) | ❌ Wave 0 |
| PKG-01/02 | D-12 golden: packed manifest digest pinned | integration | `cargo test --manifest-path crates/pmcp-package/Cargo.toml digest_stability` | ✅ file exists, constant is new |
| PKG-01/02 | Re-pinned `EXPECTED_SERVER_DIGEST` + regenerated `server.canonical.json` + regenerated `server_team_fs_v1.json` | integration | same + `cargo test --manifest-path crates/pmcp-package/Cargo.toml roundtrip` | ✅ all three exist, all three change |
| (support) | `london-tube.toml` with `[[config_slots]]` still parses + validates + boots + serves the same tool list | integration | `cargo test -p pmcp-openapi-server parity` | ✅ `tests/parity_replay.rs` (must not regress) |
| (support) | `cargo pmcp package inspect` still compiles against the new `unpack_server` signature | build | `cargo build -p cargo-pmcp` | ✅ one call site: `inspect.rs:119` |

Per CLAUDE.md's ALWAYS requirements, the phase also needs a **property test** (recommended: media-type-keyed lookup is invariant under layer permutation — the natural D-11 analogue of the existing `aggregate_ordering_is_stable_under_permutation` proptest at `slot/aggregate.rs:116-139`) and an **example**. There is no `examples/` directory in `pmcp-package` today; the cheapest honest satisfaction is a doctest on `pack_server`/`BinaryMode` showing both arms.

### Sampling Rate

- **Per task commit:** `cargo test --manifest-path crates/pmcp-package/Cargo.toml` (0.1s — effectively free, run it every time)
- **Per wave merge:** `make pmcp-package-gate` + `cargo test -p pmcp-openapi-server` + `cargo build -p cargo-pmcp`
- **Phase gate:** `make quality-gate` (covers all of the above plus fmt/clippy/doc-check/audit) before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/pmcp-package/tests/config_server.rs` — the config-only pack/unpack integration surface (PKG-01, PKG-02)
- [ ] `crates/pmcp-package/tests/fixtures/` — **does not exist**; the D-12 golden inputs need a home. Either add this directory or extend `tests/golden_fixtures/`. Note `pmcp-openapi-server` sets `exclude = [... "tests/" ...]` in its Cargo.toml (line 14), so `pmcp-package` **cannot** `include_str!` across into that crate's fixtures for a published build — the fixtures must be **copied into** `pmcp-package/tests/`, and a drift guard should assert the copy still matches the source.
- [ ] `crates/pmcp-package/src/slot/required.rs` — the D-03 function (new file)
- [ ] Wave B: `pmcp-server-toolkit` `config_slots` field + `base_url` expansion, with tests in `crates/pmcp-server-toolkit/src/config.rs`'s existing `mod tests`
- [ ] No framework install needed.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | The phase touches auth *declarations* (`AuthMode` slot), never an auth decision |
| V3 Session Management | no | No sessions |
| V4 Access Control | no | Cedar policies are carried as an opaque layer, unchanged |
| V5 Input Validation | **yes** | An OCI layout is untrusted input. Keep every new layer behind `read_verified_blob` (`oci/unpack.rs:33-38`); keep the `ManifestDigest`-only path-construction guard (`oci/layout.rs:135-140`) untouched; the new media-type index must not allow a duplicate key to silently win |
| V6 Cryptography | **yes, by exclusion** | `sha2` 0.10 only. **No crypto dependency may be added** — milestone Decision 1. `digest::verify` stays an integrity check, never a signature check |
| V14 Configuration | **yes** | D-04's "no resolved secret may travel in a layer" is the phase's headline security property |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| A resolved secret packed into the `config.toml` layer | Information Disclosure | D-04's pack-time placeholder validation, plus `SlotType`'s structural guarantee that a `Secret` variant has no value field (`slot/types.rs:25-31`) |
| Path traversal via a crafted blob digest | Tampering | Already mitigated — `blob_path` accepts only a validated `ManifestDigest` (64 lowercase hex, `oci/layout.rs:135-140`). Do not add a new path constructor. |
| Tampered layer bytes deserialized before verification | Tampering | `read_verified_blob` before every `serde_json::from_slice`; the new config/spec/binary-ref layers must use the same path |
| A duplicate layer media type shadowing the real layer | Tampering | New risk from D-11 — reject duplicates rather than last-wins (Pattern 1) |
| An environment-dependent digest leaking which environment packed it | Information Disclosure | D-04 makes the digest environment-independent by construction (placeholders, not resolved literals) |
| A 0.1.x package mis-deserialized into a 0.2.0 struct | Tampering / Integrity | D-10's explicit shape detection + named refusal. Note `ServerPackage` and `ServerEnvelope` lack `deny_unknown_fields`, so a stale `binary_ref` would otherwise be **silently dropped** — the refusal must check the raw JSON for the key, not rely on a deserialize error |
| A slopsquatted new dependency | Supply chain | None added; audit above is clean |

## Sources

### Primary (HIGH confidence — read from source this session)
- `crates/pmcp-package/` — `Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/oci/{mod,media_types,layout,pack,unpack}.rs`, `src/package/server.rs`, `src/slot/{mod,types,classification,aggregate,deviation}.rs`, `src/digest/verify.rs`, `tests/digest_stability.rs`, `tests/golden_fixtures/{server_team_fs_v1.json,canonical/server.canonical.json}`
- `crates/pmcp-server-toolkit/src/config.rs`, `src/http/auth.rs`
- `crates/pmcp-openapi-server/` — `Cargo.toml`, `src/cli.rs`, `tests/parity_replay.rs`, `tests/fixtures/london-tube.toml`, `tests/fixtures/london-tube-api.yaml`, `examples/london-tube.toml`
- `cargo-pmcp/src/commands/package/{kind,inspect}.rs`, `src/deployment/targets/aws_lambda/artifact.rs`, `crates/pmcp-cfn-renderer/src/params.rs`
- `Makefile:864-900`, root `Cargo.toml:828-831`, `.github/workflows/release.yml`
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/oci-spec-0.10.0/src/image/{annotations,descriptor,mod}.rs`
- Live measurement: `cargo test --manifest-path crates/pmcp-package/Cargo.toml` → 130 tests, all pass, 0.1s
- Live measurement: `gsd-tools query package-legitimacy check --ecosystem crates toml oci-spec olpc-cjson` → 3× OK

### Secondary (MEDIUM confidence)
- OCI image-spec `annotations.md` — `org.opencontainers.image.title` is "Human-readable title of the image (string)", listed among keys "intended for but not limited to image index, image manifest, and descriptor authors" [CITED: github.com/opencontainers/image-spec/blob/main/annotations.md]
- OCI image-spec `manifest.md` — layer-ordering requirements are scoped to `config.mediaType == application/vnd.oci.image.config.v1+json`; "Content other than OCI container images MAY be packaged using the image manifest"; `artifactType` "MUST be set when `config.mediaType` is set to the empty value". The spec gives **no** guidance on locating layers by media type [CITED: github.com/opencontainers/image-spec/blob/main/manifest.md]
- ORAS artifact concepts — uses `"org.opencontainers.image.title": "hello.txt"` on a layer descriptor to carry the original filename [CITED: oras.land/docs/concepts/artifact/]

### Tertiary (LOW confidence)
- None relied upon. The `classify-confidence` seam rates `webfetch` LOW; the two WebFetch findings above are tagged `[CITED]` rather than `[VERIFIED]` accordingly, and neither is load-bearing on its own — `ANNOTATION_TITLE`'s existence and value were independently confirmed by reading the vendored `oci-spec` source.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new packages; every version read from the resolved graph
- Architecture: HIGH — every pattern is a delta against source read this session, with verbatim quotes and line ranges
- Pitfalls: HIGH for 1-3 and 5-8 (each traced to a quoted line); HIGH for 4 (the serde tagging makes it a certainty, though the *resolution* is a user decision)
- Open questions: MEDIUM — all three are answered with evidence, but #4 (auth-mode carve-out) and the one/two-crate scope question need user confirmation before planning
- Security: HIGH — derived from the crate's own documented threat register and non-goals

**Research date:** 2026-08-22
**Valid until:** 2026-09-21 (30 days — the crate is stable and workspace-excluded; the main invalidator would be someone else editing `pmcp-server-toolkit::ServerConfig`)
