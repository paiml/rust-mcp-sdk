# Phase 120: Config-Server Packaging - Context

**Gathered:** 2026-08-22
**Status:** Ready for planning

<domain>
## Phase Boundary

A server whose entire identity is a `config.toml` plus an OpenAPI spec gets a complete
AI-Package identity. Three things land:

1. **Vendor media-type layers** for the server's own `config.toml` and its OpenAPI spec, so a
   Shape A pure-config server (`pmcp-openapi-server`) is representable at all. Today
   `pack_server` (`crates/pmcp-package/src/oci/pack.rs:51`) takes `bootstrap: &[u8]` as a
   required positional parameter, so a config-only server is literally unrepresentable.
2. **A dual-mode binary** — embedded (bootstrap bytes) or referenced (`BinaryRef { digest,
   media_type }` resolved in the target environment).
3. **A machine-checkable baked-versus-slot split** — the spec is baked (it defines the tool
   surface); endpoint, credentials and auth mode are slots.

Requirements: PKG-01, PKG-02, PKG-03. Proving fixture:
`crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` + `london-tube-api.yaml`.

**Not this phase:** the round-trip E2E and tool-list parity (Phase 121, PKG-04); attestation
carriage (Phase 122); CLI verbs (Phase 123); the publish ledger and version bump mechanics
(Phase 124). Signing/crypto and an OCI registry client are milestone-level non-goals.

</domain>

<decisions>
## Implementation Decisions

### Slot derivation and vocabulary

- **D-01:** Slots are **declared explicitly** by the config author in a `[[config_slots]]`
  block in the server's `config.toml`; `pack` reads them. Nothing is auto-derived from
  `${VAR}` placeholders and nothing is structurally inferred from known Shape A keys. Rejected:
  placeholder auto-derivation (`base_url` is a bare literal in the fixture and auth mode is a
  structural key, so it cannot cover PKG-03's three), structural mapping, and structural +
  override. — **Reversibility:** costly — the block becomes part of the Shape A config schema
  that every packable server config must carry; adding auto-derivation later is additive, but
  removing the block after configs adopt it is a config-format migration.

- **D-02:** `SlotType` gains **two new typed variants**, `Endpoint { name, tested_value }` and
  `AuthMode { name, tested_value }`, alongside the existing six. Both carry `tested_value`, so
  `classify` (`crates/pmcp-package/src/slot/classification.rs:24`) places them in the
  **behavior-relevant** family automatically — the classification is derived from
  `SlotType::tested_value()`, not hand-maintained. Credentials remain `Secret { name }`
  (identity-bearing). Rejected: one generic `Config { name, tested_value }` variant (loses the
  type-level distinction), and reusing existing variants (nothing fits; calling an endpoint an
  `LlmProvider` would read as a bug). — **Reversibility:** one-way — `SlotType` is a serialized
  wire type; once packages carry `endpoint`/`auth_mode` discriminants, removing or renaming a
  variant breaks every package already written. Note `SlotType` is deliberately NOT
  `deny_unknown_fields`, so older readers tolerate the addition.

- **D-03:** Enumerating "the slots the target environment must fill" gets a **separate
  required-slots function** (e.g. `required_slots` / `unfilled_slots`) that walks a package's
  `config_slots` and returns identity-bearing and behavior-relevant slots alike.
  `detect_deviation` (`crates/pmcp-package/src/slot/deviation.rs:28`) keeps its current, correct
  meaning — "this behavior-relevant value differs from what was tested" — and keeps returning
  `None` for identity-bearing slots. Rejected: widening `detect_deviation` (breaks a documented
  invariant its own tests assert), and deferring to Phase 121. — **Reversibility:** reversible.

- **D-04:** The packed `config.toml` layer holds the file **byte-for-byte as authored**, and
  `pack` **validates that every key named by a `[[config_slots]]` entry already holds a `${VAR}`
  placeholder** — a resolved literal in a slot-declared key is a pack-time error. Consequences:
  the manifest digest is environment-independent (two environments differing only in endpoint
  produce the same package), no resolved secret can travel in a layer (upholding the
  "secrets never travel" non-goal in `crates/pmcp-package/src/slot/types.rs`), and `unpack`
  restores the file byte-identically as PKG-01 criterion 1 requires. Rejected: templatizing at
  pack time (breaks byte-identical restoration; silent mutation of the author's file), and
  packing verbatim with literals as tested values (environment-dependent digests; a resolved
  secret could be packed). — **Reversibility:** costly — relaxing the validation later is
  additive, but tightening it after configs exist forces every config to be edited.

### Binary dual-mode

- **D-05:** **Two modes, not three.** A config-only server *is* the **referenced** mode: the
  package carries `BinaryRef { digest, media_type }` naming the shared, already-deployed runtime
  binary, and no bootstrap layer. "No bespoke binary" and "the binary lives in the target
  environment" are the same fact. Rejected: a third `none` mode (leaves the target env with no
  statement about which runtime to run). — **Reversibility:** costly — adding a third mode later
  is an enum arm on a wire type.

- **D-06:** The modes are expressed as an **enum parameter and an enum return**:
  `pack_server(package, binary: BinaryMode<'_>, layout)` with
  `BinaryMode::Embedded(&[u8]) | BinaryMode::Referenced { digest, media_type }`, and
  `unpack_server` returns `(ServerPackage, UnpackedBinary)` with the same two-arm shape. There is
  no `Vec<u8>` to reach for on a referenced package, so PKG-02's "a caller cannot mistake a
  referenced package for one that has bytes" is enforced by the type system rather than by a
  runtime check. Rejected: a parallel `pack_config_server` function (two paths to keep in sync;
  the mistake moves rather than disappearing), and `Option<&[u8]>`/`Option<Vec<u8>>` (`None`
  conflates "referenced, resolve this digest" with "nothing here" and hands the caller no
  digest). — **Reversibility:** one-way — breaking change to both public signatures; every caller
  updates. Paired with D-09 (0.2.0).

- **D-07:** Unpacking a referenced package **never looks for the blob locally** — it always
  returns `Referenced { digest, media_type }`. `unpack` stays a local, offline operation and
  resolution is the target environment's job. Rejected: look-then-fall-back (the same package
  would unpack to different shapes depending on ambient state, which would make Phase 121's
  regression net pass in env A and fail in env B for reasons unrelated to the package). —
  **Reversibility:** reversible.

- **D-08:** **`ServerPackage.binary_ref` is dropped** (`crates/pmcp-package/src/package/server.rs:379`).
  `BinaryMode` is the single source of the binary's identity, reconstructed at unpack from the
  manifest. This follows the crate's own documented rule that binary payloads are OCI layers, not
  typed-struct fields. Rejected: keeping the field with a pack-time agreement check, and keeping
  it silently overwritten by `BinaryMode` — both hold the same fact in two places. —
  **Reversibility:** one-way — changes `ServerPackage`'s serialized shape, so the
  `MT_SERVER_ENVELOPE` layer's bytes and `EXPECTED_SERVER_DIGEST` move. This is what forces D-09.

### Format compatibility and the wire freeze

- **D-09:** `pmcp-package` goes **0.1.1 → 0.2.0 as a declared break**. `EXPECTED_SERVER_DIGEST` in
  `crates/pmcp-package/tests/digest_stability.rs` is re-pinned and the wire-freeze comment is
  updated to name the 0.2.x line — the file already states that the day these digests must change
  is the day the format goes 0.2.0. **`ARTIFACT_TYPE_SERVER` stays `.v1`** (no second version
  axis). Rejected: staying 0.1.x (would require reversing D-08), and bumping the `artifactType`
  alongside. Phase 124 owns moving `cargo-pmcp`'s caret pin and the
  `cargo-pmcp/tests/pmcp_package_pin.rs` tripwire in the same change. — **Reversibility:** one-way
  — a published version number and a re-pinned wire freeze.

- **D-10:** The 0.2.0 `unpack` path **does not read 0.1.x packages**. It detects the 0.1.x layer
  shape and **refuses with a clear error** naming the format change and the version that wrote it,
  rather than mis-deserializing. `pmcp-package` is an experimental 0.x crate with no published
  consumers, so nothing in the wild is stranded. Rejected: reading both shapes (two
  deserialization paths and a synthesized `binary_ref` carried indefinitely), and refusing plus a
  `.v2` artifactType. — **Reversibility:** reversible — adding a compat reader later is additive.

- **D-11:** `unpack` locates layers by **vendor media type, not by index**. Today
  `unpack_server` (`crates/pmcp-package/src/oci/unpack.rs:79-90`) reads positionally
  (`.first()`, `.get(1)` … `.get(5)`), which cannot survive an optional bootstrap layer plus two
  new layers. Media-type keying makes layer order non-load-bearing, lets a missing required layer
  name itself, and gives D-10's refusal a precise signal (a bootstrap layer sitting alongside a
  config layer, or an envelope carrying `binary_ref`). Rejected: a fixed extended positional order
  ("optional layer at a fixed index" is a contradiction — every later layer shifts). —
  **Reversibility:** reversible.

- **D-12:** The config-only golden fixture pins the **packed OCI manifest digest** returned by
  `finalize_pack` — i.e. the test packs a layout rather than only deserializing a struct. This is
  the only value that actually moves when the layer set, layer order, or a media-type string
  changes, which is what PKG-01/PKG-02 criterion 4 asks for. The four existing goldens under
  `crates/pmcp-package/tests/golden_fixtures/` pin `manifest_digest(value)` over a **struct**,
  which is blind to all three. Rejected: adding a struct-level fixture in the established pattern
  (would not meet the criterion), and doing both. — **Reversibility:** reversible.

### Layer generality

- **D-13:** **Generic config layer, typed spec layer.** One shared
  `application/vnd.pmcp.mcp-server.config.v1+toml` for the `config.toml` — identical across all
  three Shape A siblings — and a per-kind spec layer whose media type names the kind, i.e.
  `application/vnd.pmcp.mcp-server.openapi-spec.v1` now, siblings later. The half that genuinely
  generalizes is shared; the half that does not stays honest. Rejected: naming both layers
  OpenAPI-specific (the config layer is provably identical across siblings, so the second server
  forces a rename or a duplicate constant), and a fully generic `schema.v1` pair
  (`pmcp-workbook-server` takes a `--bundle-dir` **directory**, so "generic" would have to mean an
  archive — speculative from one example). — **Reversibility:** costly — media-type strings are
  wire identifiers; renaming one after packages exist is a format change.

- **D-14:** The **spec layer is optional**, mirroring the runtime. `pmcp-openapi-server`'s
  `--spec` is `Option<PathBuf>` (D-03 of that crate: a curated-only server boots without it), and
  `crates/pmcp-openapi-server/tests/parity_replay.rs` currently runs with `spec: None`. A
  config-only package may therefore carry a spec layer or not. The golden fixture (D-12) must pin
  the **with-spec** case explicitly so the without-spec path cannot quietly become the default.
  Rejected: requiring the spec (would make curated-only servers — which the binary explicitly
  supports — unpackable), and optional-plus-an-absence-marker. — **Reversibility:** reversible.

- **D-15:** Original **filenames are carried in OCI descriptor annotations**
  (`org.opencontainers.image.title` is the standard field for exactly this), so `unpack` restores
  `london-tube.toml` and `london-tube-api.yaml` under their own names. Standards-native, and a
  registry client reads it with zero translation — which is the stated reason `oci-spec` was kept
  when `oci-client` was excluded. Note the annotations become part of the canonical manifest, so a
  filename change moves the digest. Rejected: canonical fixed names (renames the author's files;
  the spec's extension must be inferred), and bytes-only (pushes naming onto every caller). —
  **Reversibility:** costly — annotations are inside the digested manifest.

- **D-16:** **One spec media type, bytes verbatim.** A single
  `mcp-server.openapi-spec.v1` layer carries whatever the author supplied, byte-for-byte; the
  format (YAML or JSON) is evident from the filename annotation established in D-15. Rejected:
  two media types (`+yaml` / `+json`, adding a pack-time detection rule and a two-way lookup), and
  normalizing to JSON (would break byte-identical restoration — criterion 1 — and would make
  `pmcp-package` parse a payload it has no reason to understand). — **Reversibility:** reversible.

### Claude's Discretion

No area was delegated with "you decide". Three sub-questions were surfaced and consciously left
for research/planning to resolve — they are recorded under **Open for planning** below, not as
locked decisions.

### Open for planning

These were raised during discussion, are in scope for Phase 120, and the user chose not to
pre-decide them. The planner should resolve them, or the researcher should surface options:

1. **Where the referenced runtime digest comes from at pack time.** D-05 requires a referenced
   package to name a digest, but `BinaryRef.digest` is `Option<ManifestDigest>` today (`None`
   before packing). `BinaryMode::Referenced` needs it non-optional. Whether the caller supplies it
   verbatim, or it is derived from something in the deploy descriptor, is unresolved. Note
   milestone Decision 2 forbids an OCI registry client, so `pmcp-package` can only treat the
   digest as opaque.
2. **Whether the `[[config_slots]]` block needs a completeness check.** D-01 makes the author
   responsible for declaring slots; a forgotten slot bakes a value silently. D-04's placeholder
   validation catches the inverse (a declared slot holding a literal) but not an undeclared
   environment-specific literal.
3. **How `london-tube.toml`'s `base_url = "https://api.tfl.gov.uk"` literal becomes a `${VAR}`
   placeholder** without breaking `parity_replay.rs`, which is an existing green offline test.

### Planning amendments (user decisions, 2026-08-22, post-research)

Research (120-RESEARCH.md) found two of the locked decisions unimplementable as written; the
user resolved both before planning:

- **D-17:** D-04's pack-time `${VAR}` placeholder validation is **scoped to value slots**
  (endpoint, credentials). The auth-mode key is **exempt as structural**: the toolkit's
  `AuthConfig` is `#[serde(tag = "type")]`, so `type = "${AUTH_MODE}"` is an unparseable
  unknown-variant error — no placeholder form of that key can exist. AuthMode remains a declared
  `ConfigSlot` (PKG-03's three slots are unchanged); its baked value is the default, and
  deviation surfaces through slot classification rather than a placeholder. Rejected: custom
  placeholder parsing before serde tag dispatch (invasive change to a published crate), and
  dropping the auth-mode slot (violates PKG-03 as written). — **Reversibility:** reversible
  (tightening validation later to more slot kinds is additive).
- **D-18:** Phase scope **expands to two waves**. Wave 1: `pmcp-package` packaging work.
  Wave 2: **additive `pmcp-server-toolkit` changes** so criterion 3 is demonstrable end-to-end
  against the real server — `ServerConfig` accepts a `[[config_slots]]` block (today
  `deny_unknown_fields` makes it a fatal parse error), `base_url` gains `${VAR}` expansion
  (today only auth credentials and `code_mode.token_secret` expand), and
  `parity_replay.rs` (which asserts the literal `base_url = "https://api.tfl.gov.uk"`) stays
  green. Rejected: pmcp-package-only scope with a synthetic fixture-copy demonstration. —
  **Reversibility:** costly — the `[[config_slots]]` schema becomes part of the toolkit's
  published config surface.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone scope and requirements
- `.planning/ROADMAP.md` § `## v2.6 AI-Package Portability (Phases 120-124)` and
  § `## Phase Details — Current Milestone` → Phase 120 — the goal, the four success criteria, and
  the two scoping decisions that shrink the SDK's share (no crypto, no `oci-client`).
- `.planning/REQUIREMENTS.md` § Package Portability (PKG) — PKG-01/02/03 text, the Out of Scope
  table (byte-for-byte round-tripping is an explicit non-goal), and the two measured-drift notes.
- `.planning/STATE.md` § `## v2.6 Phase Plan` — execution order (120 → 121 together, then 122 ∥
  123, then 124) and the parked-by-design statement.

### The crate being changed (`pmcp-package` 0.1.1)
- `crates/pmcp-package/src/oci/media_types.rs` — the full layer inventory and the
  `application/vnd.pmcp.*` naming convention the two new media types must be siblings of; also
  `ARTIFACT_TYPE_SERVER` and the empty-config blob constants.
- `crates/pmcp-package/src/oci/pack.rs:51` — `pack_server`'s current signature, the
  `ServerEnvelope` struct, and the six-layer push order.
- `crates/pmcp-package/src/oci/unpack.rs:74-110` — the **positional** layer reads D-11 replaces,
  and the `(ServerPackage, Vec<u8>)` return type D-06 replaces.
- `crates/pmcp-package/src/package/server.rs:352-384` — `BinaryRef` and `ServerPackage`, including
  the `binary_ref` field D-08 drops.
- `crates/pmcp-package/src/slot/types.rs` — the six existing `SlotType` variants, the
  identity-bearing vs behavior-relevant split, the "secrets never travel" non-goal, and the
  deliberate absence of `deny_unknown_fields`.
- `crates/pmcp-package/src/slot/classification.rs:24` — `classify` derives the family from
  `tested_value()`, which is why D-02's new variants land in the behavior-relevant family for free.
- `crates/pmcp-package/src/slot/aggregate.rs:23` — dedup/conflict rules the new variants inherit.
- `crates/pmcp-package/src/slot/deviation.rs:28` — `detect_deviation`'s pairwise contract and its
  identity-bearing short-circuit. **Read before touching it** — D-03 leaves it unchanged.
- `crates/pmcp-package/src/digest/verify.rs:28` — `verify` is an integrity check and stays one.
- `crates/pmcp-package/tests/digest_stability.rs:31-41` — the wire-freeze comment that D-09 acts
  on, and the four pinned struct-level digests.
- `crates/pmcp-package/tests/golden_fixtures/` — the existing four fixtures plus their
  `canonical/` snapshots; D-12 adds a different **kind** of golden here.

### The proving case (`pmcp-openapi-server`)
- `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` — the config to be packed. Note
  `[backend] base_url` is a bare literal, `[backend.auth] query_params = { app_key =
  "${TFL_APP_KEY}" }` is already a placeholder, and `[backend.auth] type = "api_key"` is a
  structural key — the three shapes D-01/D-04 must handle.
- `crates/pmcp-openapi-server/tests/fixtures/london-tube-api.yaml` — the spec whose one-byte
  change must move the manifest digest (criterion 3).
- `crates/pmcp-openapi-server/src/cli.rs:38-62` — `--config` required, `--spec` optional (that
  crate's D-03), which is what D-14 mirrors.
- `crates/pmcp-openapi-server/tests/parity_replay.rs` — the existing offline `wiremock` harness;
  runs with `spec: None` today (line ~255) and is the test D-04's placeholder change must not
  break. Phase 121 builds directly on it.

### Downstream coupling (do not change here, but do not break)
- `CLAUDE.md` § Release & Publish Workflow, slot 13 (`pmcp-package`, workspace-**excluded**, its
  own `[workspace]` table) and slot 9b (`pmcp-openapi-server`) — Phase 124 owns these.
- `cargo-pmcp/tests/pmcp_package_pin.rs` — the version tripwire that must move with D-09 in
  Phase 124.

### Sibling Shape A servers (evidence for D-13, not targets of this phase)
- `crates/pmcp-sql-server/src/cli.rs` — `--config` + a **required** `--schema`.
- `crates/pmcp-workbook-server/src/cli.rs` — `--bundle-dir` (a **directory**) + optional
  `--bundle-id`. This is why a fully generic schema layer was rejected.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `vendor_media_type()` in `oci/media_types.rs` — routes any `application/vnd.pmcp.*` string to
  `MediaType::Other`. The two new media types need no new machinery, just constants.
- `OciLayout::write_blob()` / `read_blob()` and `read_verified_blob()` — content-addressing and
  digest-before-deserialize already work for arbitrary bytes; the config and spec layers are plain
  byte payloads exactly like the bootstrap layer.
- `finalize_pack()` — canonical manifest construction and the single-hash rule (one hash, one
  source of truth) is unchanged by adding layers; D-12's golden pins its return value.
- `classify()` / `aggregate()` — both derive behaviour from `SlotType::tested_value()` and
  `SlotType::key()`, so D-02's two new variants need only those two impl arms extended, not the
  classification logic.
- `crates/pmcp-openapi-server/tests/parity_replay.rs` — an established offline `wiremock` +
  programmatic-`Args` harness; the model for any behavioural assertion here and the foundation of
  Phase 121.

### Established Patterns
- **Binary payloads are OCI layers, never typed-struct fields** — stated in `media_types.rs` and
  `pack.rs`. D-08 applies this rule to `binary_ref`.
- **Digest-verify before deserialize** — `read_verified_blob` verifies every blob, including the
  empty-config blob, before any parse. New layers must go through the same path.
- **Deterministic ordering everywhere** — `BTreeMap` not `HashMap` in `aggregate`, `olpc-cjson`
  canonicalization in `canonicalize`. Anything new that feeds the digest must be order-stable.
- **Forward-compatible slot decoding** — `SlotType` deliberately omits `deny_unknown_fields`
  (RESEARCH Pitfall 4) so an older reader ignores unknown fields. `DeployDescriptor` does the
  opposite (`deny_unknown_fields`) — do not assume one rule applies to both.
- **Drift-guard tests as constants** — `empty_config_digest_matches_hash_of_empty_json_blob` is
  the in-crate pattern for pinning a constant against its computed value; the
  `const + include_str! + assert` tripwire in `cargo-pmcp/tests/pmcp_package_pin.rs` is the
  cross-crate pattern.

### Integration Points
- `pack_server` / `unpack_server` are the two public functions whose signatures change (D-06). Find
  every caller before planning — `cargo-pmcp/src/commands/package/` currently ships five verbs
  (`inspect | capture | show | import | approve`) and `inspect` reads packages.
- `ServerPackage` is re-exported from `crates/pmcp-package/src/package/mod.rs`; dropping
  `binary_ref` (D-08) touches every construction site including the in-crate test fixture
  `sample_deploy_descriptor()` / the golden fixture JSON.
- `crates/pmcp-package` is **workspace-excluded** (its own `[workspace]` table). Root
  `cargo fmt/clippy/test` and `cargo publish -p pmcp-package` do **not** reach it — verify with
  `--manifest-path crates/pmcp-package/Cargo.toml`. `cargo metadata --no-deps` lists 28 packages
  and does not include it, which is also why the release-coverage gate is blind to it (Phase 124).

</code_context>

<specifics>
## Specific Ideas

- The proving fixture is fixed: `london-tube.toml` + `london-tube-api.yaml`, packed with **no
  bootstrap layer at all**.
- `detect_deviation`'s existing contract is treated as correct and is explicitly protected —
  the new enumeration capability goes in a new function rather than widening it (D-03). Phase
  121's success criterion 2 currently reads "`detect_deviation` names **exactly** the slots B
  must fill"; **that wording is wrong against the code and must be restated against the new
  required-slots function when Phase 121 is planned.** `detect_deviation` returns `None` for
  every identity-bearing slot by design, so it can never name a credential slot.
- The type system, not a runtime check, is what prevents mistaking a referenced package for one
  with bytes (D-06) — the same instinct as `SlotType`'s "a resolved secret is not representable".
- 0.1.x packages are refused loudly rather than read leniently (D-10) — the crate is
  experimental with no published consumers, so this is the cheap moment to break.

</specifics>

<deferred>
## Deferred Ideas

- **Generic schema/bundle layer covering all three Shape A siblings** — rejected in D-13 as
  speculative from one example; `pmcp-workbook-server` takes a directory, so a generic layer would
  have to mean an archive. Revisit when a second config-only server is actually packed.
- **`.v2` `artifactType` so the manifest self-declares its format version** — considered and
  declined in D-09 to avoid a second version axis. Worth revisiting if a third format break lands.
- **Packed-manifest golden pins for the other three package kinds** (agent/team/workflow) — D-12
  adds one only for the config-only server kind. The same blindness (struct digests cannot see
  layer set/order/media types) applies to all four.
- **A compat reader for 0.1.x packages** — declined in D-10; additive if it is ever needed.

</deferred>

---

*Phase: 120-Config-Server Packaging*
*Context gathered: 2026-08-22*
