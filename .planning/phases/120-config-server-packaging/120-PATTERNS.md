# Phase 120: Config-Server Packaging - Pattern Map

**Mapped:** 2026-08-22
**Files analyzed:** 15 (11 wave-1 `pmcp-package`, 4 wave-2 toolkit/openapi-server)
**Analogs found:** 15 / 15 (every file already exists or has an in-crate sibling)

Scope note: `crates/pmcp-package/` is workspace-**excluded** — all analogs for wave 1 come from
inside that crate. Wave 2 analogs come from `crates/pmcp-server-toolkit/` and
`crates/pmcp-openapi-server/`. No new module tree is needed; only `src/slot/required.rs` and the
D-12 fixture directory are new files.

## File Classification

### Wave 1 — `crates/pmcp-package` (D-01..D-16)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/oci/media_types.rs` (M) | config/constants | — | itself: `MT_SERVER_CONFIG_SLOTS` block, lines 40-53 | exact (same file) |
| `src/oci/pack.rs` (M) | service (serializer) | file-I/O + transform | itself: `pack_server`, lines 51-100 | exact (same file) |
| `src/oci/unpack.rs` (M) | service (deserializer) | file-I/O + transform | itself: `unpack_server`, lines 74-120 | exact (same file) |
| `src/package/server.rs` (M) | model (wire type) | transform | itself: `BinaryRef`/`ServerPackage`, lines 352-384 | exact (same file) |
| `src/slot/types.rs` (M) | model (wire enum) | transform | itself: `LlmProvider`/`BudgetOverride` variants, lines 57-73 | exact (same file) |
| `src/slot/required.rs` (**NEW**) | utility (pure fn) | transform | `src/slot/deviation.rs` (whole file, 127 lines) | exact — same shape: pure fn over `SlotType` + `classify` + inline `#[cfg(test)] mod tests` |
| `src/slot/mod.rs` (M) | module barrel | — | itself, lines 10-18 (`pub mod` + `pub use` pairs) | exact |
| `tests/digest_stability.rs` (M) | test (integration) | transform | itself, lines 31-100 (pinned-constant gate) | exact |
| `tests/golden_fixtures/server_team_fs_v1.json` (regen) | fixture (canonical bytes) | — | itself + `roundtrip.rs:5-12` authoring note | exact |
| `tests/golden_fixtures/canonical/server.canonical.json` (regen) | fixture | — | same | exact |
| `tests/golden_fixtures/config_server_london_tube_v1/` (**NEW**) | fixture inputs | file-I/O | `tests/roundtrip.rs::read_fixture` (lines 32-38) | role-match |
| `tests/roundtrip.rs` (M — callers of `pack_server`) | test (integration) | file-I/O | itself, lines 45-62 | exact |
| `tests/negative.rs` (M — tamper/refusal cases incl. D-10) | test (negative) | transform | itself | exact |
| `Cargo.toml` (M — 0.1.1 → 0.2.0) | config | — | itself | exact |

### Wave 2 — toolkit + proving case (D-18)

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/pmcp-server-toolkit/src/config.rs` (M — `config_slots` field + `ConfigSlotDecl`) | model/config | transform | same file: `pub backend: Option<BackendSection>` (lines 115-125) and `pub struct ToolDecl` (line 579) | exact |
| `crates/pmcp-server-toolkit/src/config.rs` or a backend seam (M — `base_url` `${VAR}` expansion) | service (resolver) | transform | `crates/pmcp-server-toolkit/src/http/auth.rs::parse_env_ref` + `resolve_secret_ref` (lines ~490-545) | exact |
| `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` (M) + `examples/london-tube.toml` (M) | fixture/config | — | each other (two copies — edit BOTH) | exact |
| `crates/pmcp-openapi-server/tests/parity_replay.rs` (M — drop string surgery) | test (integration) | request-response | same file: `std::env::set_var("TFL_APP_KEY", …)` at line ~239 | exact |

## Pattern Assignments

### `src/oci/media_types.rs` (config/constants) — D-13, D-15, D-16

**Analog:** itself, the `ServerPackage layers` block.

**Constant + rustdoc pattern** (lines 50-53) — copy this exact shape for
`MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`, `MT_SERVER_BINARY_REF`:

```rust
/// The server's tool/connector metadata (`ServerPackage.tools`).
pub const MT_SERVER_TOOL_METADATA: &str = "application/vnd.pmcp.mcp-server.tool-metadata.v1+json";
/// The server's declared config slots (`ServerPackage.config_slots`).
pub const MT_SERVER_CONFIG_SLOTS: &str = "application/vnd.pmcp.mcp-server.config-slots.v1+json";
```

**Drift-guard test pattern** (lines 129-136) — the in-crate way to pin a constant against a
computed value; reuse for any new digest constant:

```rust
/// Drift guard (Task 1 `<action>`): the checked-in `EMPTY_CONFIG_DIGEST`
/// constant must always equal the actual sha256 of `EMPTY_CONFIG_BLOB` —
/// this test fails loudly if the two are ever edited out of sync.
#[test]
fn empty_config_digest_matches_hash_of_empty_json_blob() {
    let computed = ManifestDigest::from_bytes(EMPTY_CONFIG_BLOB);
    assert_eq!(computed.as_str(), EMPTY_CONFIG_DIGEST);
}
```

**Module-doc obligation:** the layer inventory at lines 5-25 names `binary_ref` explicitly
(lines 13-20). D-08 makes that prose false — update it in the same edit.

---

### `src/oci/pack.rs` (service, transform) — D-06, D-08, D-13..D-16

**Analog:** `pack_server` itself, lines 51-100.

**Layer-write pattern to copy verbatim for the two new layers** (lines 63-83):

```rust
    let bootstrap_descriptor =
        layout.write_blob(vendor_media_type(MT_SERVER_BOOTSTRAP), bootstrap)?;
    let envelope_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_ENVELOPE),
        &canonicalize(&envelope)?,
    )?;
```

Note the split: struct layers go through `canonicalize(&…)`; byte payloads (bootstrap — and now
config + spec) are written **raw**. The config/spec layers follow the *bootstrap* arm, never the
`canonicalize` arm (RESEARCH anti-pattern: never re-derive a digest from a parsed struct).

**Annotation pattern for D-15** — the crate already does this on the *index* descriptor
(`pack.rs:170-175`); apply the same call to the new **layer** descriptors:

```rust
    let annotations = HashMap::from([
        ("name".to_string(), name.to_string()),
        ("version".to_string(), version.to_string()),
    ]);
    manifest_descriptor.set_annotations(Some(annotations));
```

For layers use `oci_spec::image::ANNOTATION_TITLE` as the key (do not hand-roll the string).
Unlike the index case, layer annotations **do** feed the manifest digest.

**Envelope struct to edit for D-08** (lines 34-46) — drop the `binary_ref` field and its
doc-comment mention:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ServerEnvelope {
    pub(super) name: String,
    pub(super) version: semver::Version,
    pub(super) digest: Option<ManifestDigest>,
    pub(super) binary_ref: BinaryRef,   // ← removed by D-08
}
```

**`finalize_pack` is unchanged** (lines 148-186) — it already takes `layers: Vec<Descriptor>`
generically. D-12's golden pins *its return value*.

**Optional-layer construction:** build the `layers` vec with `push` rather than the current
`vec![…]` literal (lines 85-92), so the spec layer (D-14) and the exactly-one-of binary arm
(D-05/D-06) can be conditional. Keep the push order deterministic — it feeds the digest even
though D-11 makes reads order-independent.

---

### `src/oci/unpack.rs` (service, transform) — D-06, D-07, D-10, D-11

**Analog:** `unpack_server` itself, lines 74-120, plus the helper trio above it.

**The verified-read chokepoint every new layer must go through** (lines 33-38):

```rust
fn read_verified_blob(layout: &OciLayout, descriptor: &Descriptor) -> Result<Vec<u8>> {
    let expected = ManifestDigest::try_from(descriptor.digest())?;
    let bytes = layout.read_blob(descriptor)?;
    verify(&expected, &bytes)?;
    Ok(bytes)
}
```

**The error-helper pattern to reuse for missing/duplicate layers** (lines 66-70):

```rust
fn missing_layer(name: &str) -> PackageError {
    PackageError::Layout {
        reason: format!("manifest is missing the '{name}' layer"),
    }
}
```

**The code D-11 replaces** (lines 78-90) — positional, and the module doc at lines 13-17
("Layer order mirrors `super::pack`'s push order exactly") must be rewritten with it:

```rust
    let layers = manifest.layers();
    let bootstrap_descriptor = layers.first().ok_or_else(|| missing_layer("bootstrap"))?;
    let envelope_descriptor = layers.get(1).ok_or_else(|| missing_layer("envelope"))?;
    ...
    let config_slots_descriptor = layers.get(5).ok_or_else(|| missing_layer("config-slots"))?;
```

**Complexity guard (CI-blocking, cog ≤ 25):** `unpack_server` is already long and linear.
Extract `index_layers`, `detect_legacy_shape` (D-10), and `read_binary_mode` (D-06) as separate
free functions **from the start** — mirror the existing helper style (`read_the_one_manifest`,
`verify_config_blob`, `read_verified_blob`), each with a `///` doc comment stating what it
verifies.

**Single-layer counterpart for style reference** — `unpack_single_layer` (lines 122-135) shows
the crate's preferred "one path, no per-kind copy-paste" idiom.

---

### `src/slot/types.rs` (model, transform) — D-02

**Analog:** the `LlmProvider` / `BudgetOverride` variants, lines 57-73. Copy their shape exactly
for `Endpoint` and `AuthMode`:

```rust
    /// A named LLM provider slot, carrying the `tested_value` (e.g. `"anthropic"`) that was
    /// exercised when the package was tested. Behavior-relevant — a proposed binding
    /// that differs from `tested_value` is a real behavioral change, not an identity swap.
    LlmProvider {
        /// The slot's declared name.
        name: String,
        /// The provider value exercised when the package was tested.
        tested_value: String,
    },
```

**The two `match` arms to extend** (lines 80-99). ⚠ The `_ => None` wildcard at line 97 will
silently misclassify the new variants as identity-bearing — replace it with explicit arms so a
future variant is a compile error:

```rust
    pub fn key(&self) -> (&'static str, &str) {
        match self {
            SlotType::Secret { name } => ("secret", name.as_str()),
            ...
            SlotType::BudgetOverride { name, .. } => ("budget_override", name.as_str()),
        }
    }

    pub fn tested_value(&self) -> Option<&str> {
        match self {
            SlotType::LlmProvider { tested_value, .. }
            | SlotType::BudgetOverride { tested_value, .. } => Some(tested_value.as_str()),
            _ => None,        // ← replace with explicit arms
        }
    }
```

**Per-variant round-trip test pattern to copy** (lines 178-189) — one test per new variant:

```rust
    #[test]
    fn budget_override_round_trips_with_tested_value() {
        let slot = SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "1000".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "budget_override");
        assert_eq!(json["tested_value"], "1000");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }
```

---

### `src/slot/required.rs` (**NEW** — utility, transform) — D-03

**Analog:** `src/slot/deviation.rs` (entire 127-line file). Same shape: module doc stating the
invariant, one pure function gated through `classify`, an inline `#[cfg(test)] mod tests`.

**Imports + module doc** (deviation.rs lines 1-6):

```rust
//! Behavior-relevant deviation detection: flags a behavior-relevant slot whose
//! proposed value differs from the value that was exercised when the package was tested,
//! and never flags an identity-bearing slot.

use crate::slot::classification::{classify, SlotClass};
use crate::slot::types::SlotType;
```

**Function shape** (deviation.rs lines 17-47) — note the doc comment names both what it returns
and what it deliberately does *not* do. `required_slots` must carry the mirror statement: it
returns **both** families, unlike `detect_deviation`.

```rust
pub fn detect_deviation(tested: &SlotType, proposed: &SlotType) -> Option<Deviation> {
    if classify(tested) != SlotClass::BehaviorRelevant
        || classify(proposed) != SlotClass::BehaviorRelevant
    {
        return None;
    }
    ...
}
```

**Protected invariant — do not widen** (deviation.rs lines 69-78). This test is the reason D-03
mandates a new file:

```rust
    #[test]
    fn never_flags_identity_bearing_slots() {
        let tested = SlotType::Secret { name: "X".to_string() };
        let proposed = SlotType::Secret { name: "X".to_string() };
        assert_eq!(detect_deviation(&tested, &proposed), None);
    }
```

**Barrel registration** — `src/slot/mod.rs` lines 10-18, copy both halves:

```rust
pub mod deviation;
pub mod types;

pub use deviation::{detect_deviation, Deviation};
pub use types::{ConfigSlot, SlotType};
```

**Property-test pattern** for the new function (and for D-11's permutation invariant) —
`src/slot/aggregate.rs:116-139`:

```rust
    proptest! {
        #[test]
        fn aggregate_ordering_is_stable_under_permutation(seed in proptest::collection::vec(0u32..1000, 6)) {
            ...
            let baseline = aggregate(slots.iter()).unwrap();
            let shuffled = aggregate(permuted).unwrap();
            prop_assert_eq!(baseline, shuffled);
        }
    }
```

---

### `src/package/server.rs` (model) — D-08

**Analog:** itself, lines 352-384.

**The type that survives (wire payload of the new binary-ref layer), digest stays `Option`:**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::digest::ManifestDigest>,
    pub media_type: String,
}
```

**The field that goes** (line 379, inside `ServerPackage`) — `pub binary_ref: BinaryRef,`.
Its doc comment at lines 356-360 names `pack_server(package, bootstrap, layout)` and must be
retargeted to `BinaryMode`.

**Construction sites to update:** `sample_deploy_descriptor()` / the in-crate fixtures at
`src/package/server.rs:~385+`, `src/oci/unpack.rs:107-116` (the `ServerPackage { … }` literal),
`tests/roundtrip.rs`, `tests/digest_stability.rs`, and the two checked-in JSON fixtures.

---

### `tests/digest_stability.rs` (test) — D-09, D-12

**Analog:** itself, lines 31-100.

**The wire-freeze comment D-09 rewrites** (lines 31-41) — re-pin `EXPECTED_SERVER_DIGEST` and
restate the freeze against the 0.2.x line:

```rust
// FAILS CI. This is a real wire freeze for the 0.1.x line, NOT just
// determinism: the day these must change is the day the format goes 0.2.0.

const EXPECTED_SERVER_DIGEST: &str =
    "sha256:47de0265357cd4fe221c25d848fcc4414a037caf92b874995e03b75feef903a4";
```

**Assertion + failure-message pattern to copy for the D-12 packed golden** (lines 76-84):

```rust
#[test]
fn server_fixture_digest_matches_pinned_wire_freeze_constant() {
    assert_eq!(
        manifest_digest(&server_fixture()).unwrap().as_str(),
        EXPECTED_SERVER_DIGEST,
        "ServerPackage serialized shape changed — this is a wire-freeze break (bump 0.2.0 \
         intentionally, do not silently repin)"
    );
}
```

D-12 differs in one respect only: the left-hand side is `finalize_pack`'s return value via a
real `pack_*` call into a `tempfile::tempdir()` layout, not `manifest_digest(&struct)`.

**Fixture-reading helper to copy** (lines 23-28, identical in `roundtrip.rs:32-38`):

```rust
fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}
```

**Canonical-fixture regeneration rule** — `tests/roundtrip.rs:5-12` states why a stale
`binary_ref` cannot simply be ignored:

```rust
//! `server_team_fs_v1.json` and `workflow_claims_triage_v1.json` are stored
//! in CANONICAL byte form (the exact bytes `canonicalize()` emits — compact,
//! olpc-cjson key-sorted JSON) … The byte-identical assertion below
//! therefore compares the fixture bytes against `canonicalize(&parsed)` and
//! cannot fight the canonicalizer.
```

---

### `crates/pmcp-server-toolkit/src/config.rs` (model/config) — D-18 wave 2

**Analog:** the `[backend]` field, same file lines 115-125 — the crate's own precedent for an
additive optional section under `deny_unknown_fields`:

```rust
    /// `[backend]` (optional, `http` feature) — OpenAPI/REST HTTP backend
    /// declaration (`base_url` + `[backend.auth]` + `[backend.http]`).
    ///
    /// Additive per the REF-01 superset invariant (D-06): a pure-SQL config
    /// omits `[backend]` and this field parses to `None`.
    #[cfg(feature = "http")]
    #[serde(default)]
    pub backend: Option<BackendSection>,
```

For a repeated block use the `Vec` form (same file, lines 130-133):

```rust
    /// `[[tools]]` — declarative tool surface (TOML-defined handlers).
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
```

**Decl-struct analog for `ConfigSlotDecl`:** `pub struct ToolDecl` (line 579) — all fields
`#[serde(default)]`, one `///` line each. Keep `ConfigSlotDecl` **toolkit-local**; the toolkit
must not depend on `pmcp-package` (that would invert the layering). The
`ConfigSlotDecl → pmcp_package::ConfigSlot` mapping belongs in the packing caller.

**Do NOT loosen `deny_unknown_fields`** — the file says so at lines 23-24 ("Always ADD the
missing field"). Its guard test is `test_backend_unknown_field_rejected` (lines 1275-1293).

---

### `${VAR}` expansion for `base_url` (service, transform) — D-18 wave 2

**Analog:** `crates/pmcp-server-toolkit/src/http/auth.rs` — the existing single chokepoint.
Reuse these two functions rather than writing a second resolver (two resolvers with different
`${}` edge cases is a latent security bug).

**Parser** (auth.rs ~lines 501-509):

```rust
fn parse_env_ref(raw: &str) -> Option<&str> {
    if let Some(v) = raw.strip_prefix("env:") {
        Some(v)
    } else {
        // `${...}` → the inner name (possibly empty for the malformed `${}` form).
        raw.strip_prefix("${").and_then(|s| s.strip_suffix('}'))
    }
}
```

**Resolver** (auth.rs ~lines 536-545) — note the documented no-error, never-ship-the-literal
posture, and the empty-means-omitted rule:

```rust
fn resolve_secret_ref(raw: &str) -> String {
    match parse_env_ref(raw) {
        None => raw.to_string(),                       // plain literal, verbatim
        Some(name) if name.is_empty() => String::new(), // malformed `${}` → omitted
        Some(name) => std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default(),
    }
}
```

**Contrast case — the stricter variant** in `src/code_mode.rs:1163-1210`
(`expand_braced_var` + `resolve_secret_env_var`) **errors** on an unset var instead of
defaulting to empty. For `base_url`, an empty resolution would pass `validate()`'s
empty/whitespace check only at parse time and then break every request — prefer the
code_mode error-on-unset semantics for the endpoint, and say so in the rustdoc.

**Field being changed** (`config.rs:443-448`) — plain `String`, used verbatim today:

```rust
pub struct BackendSection {
    /// REST API root URL (e.g. `"https://api.tfl.gov.uk"`). …
    #[serde(default)]
    pub base_url: String,
```

---

### `crates/pmcp-openapi-server/tests/parity_replay.rs` (test) — D-18 wave 2

**Analog:** the same file's own env-var idiom, line ~239:

```rust
    // The fixture's `${TFL_APP_KEY}` resolves from the process env at auth
    // provider construction time — set it BEFORE spawning the server.
    std::env::set_var("TFL_APP_KEY", DUMMY_APP_KEY);
```

**The code to delete** (lines 216-225) — the literal-matching string surgery that D-04's
placeholder change breaks:

```rust
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

Replace with `std::env::set_var("TFL_BASE_URL", backend.uri())` before `run_serving`. The test
gets shorter and the brittle literal assertion disappears.

**Second copy to edit:** `crates/pmcp-openapi-server/examples/london-tube.toml:32` carries the
same `base_url` literal and is parsed by `parity_replay.rs:84`.

---

### `cargo-pmcp/src/commands/package/inspect.rs` (the one downstream caller) — D-06

**Analog:** itself, line 119 — the single line D-06's signature change costs:

```rust
        PackageKind::Server => {
            let (pkg, _bootstrap) = unpack_server(layout).context("unpack server package")?;
            if output {
                render_server(&pkg);
            }
        },
```

Also `cargo-pmcp/src/commands/package/kind.rs:49-57` — add the new server media types to the
`PackageKind::Server` arm so `detect_kind`'s `layers[0]` fallback still resolves a config-only
package (Pitfall 8).

## Shared Patterns

### Byte payloads never go through `canonicalize`
**Source:** `crates/pmcp-package/src/oci/pack.rs:63-64` (bootstrap arm) vs `:65-68` (struct arm);
rule stated at `src/digest/verify.rs:9-11`.
**Apply to:** the config layer, the spec layer.
```rust
    let bootstrap_descriptor =
        layout.write_blob(vendor_media_type(MT_SERVER_BOOTSTRAP), bootstrap)?;
```
Digest what was actually stored — never re-derive from a parsed `ServerConfig`.

### Digest-verify before deserialize
**Source:** `crates/pmcp-package/src/oci/unpack.rs:33-38` (`read_verified_blob`).
**Apply to:** every new layer read in `unpack_server`. This is what makes `tests/negative.rs`'s
tamper tests meaningful.

### Vendor media types via `vendor_media_type`, never a parallel enum
**Source:** `crates/pmcp-package/src/oci/media_types.rs:98-104`.
**Apply to:** all three new constants.
```rust
pub fn vendor_media_type(media_type: &str) -> MediaType {
    MediaType::from(media_type)
}
```

### Deterministic ordering in anything feeding a digest
**Source:** `src/slot/aggregate.rs` (`BTreeMap`), `src/digest/canonicalize` (olpc-cjson).
**Apply to:** the D-11 layer index (`BTreeMap<String, &Descriptor>`), the pack-time layer push
order. `Descriptor.annotations` is a `HashMap` — canonical bytes are safe because olpc-cjson
sorts keys, but any hand-built comparison or debug print is not.

### Pinned-constant drift guards
**Source:** `src/oci/media_types.rs:129-136` (in-crate);
`cargo-pmcp/tests/pmcp_package_pin.rs` (cross-crate).
**Apply to:** `EXPECTED_SERVER_DIGEST`, the D-12 packed-manifest constant, and any new
media-type-derived constant.

### `tempfile::tempdir()` for every pack/unpack test
**Source:** `crates/pmcp-package/tests/roundtrip.rs:60-61`; Cargo.toml dev-dep comment
"do NOT hand-roll temp dirs".
**Apply to:** every new pack/unpack test including the D-12 golden.

### Additive config fields, never loosened `deny_unknown_fields`
**Source:** `crates/pmcp-server-toolkit/src/config.rs:23-24` and the `[backend]` field
(lines 115-125).
**Apply to:** the wave-2 `config_slots` field.

## No Analog Found

None. Every file in scope either already exists or has a same-crate sibling with the identical
role and data flow. Two items are new-but-derivative:

| File | Role | Data Flow | Note |
|---|---|---|---|
| `src/slot/required.rs` | utility | transform | New file, but `deviation.rs` is a line-for-line structural template |
| `tests/golden_fixtures/config_server_london_tube_v1/` | fixture | file-I/O | New *kind* of golden (packed-manifest, not struct) — the assertion style still copies `digest_stability.rs:76-84`; the authoring procedure (placeholder constant → run once → paste `actual`) copies `roundtrip.rs:8-10` |

Two design decisions have **no** in-crate analog and must be authored fresh (RESEARCH supplies
the shapes): `BinaryMode`/`UnpackedBinary` (Pattern 3), and the D-10 legacy-shape refusal. The
nearest stylistic references are `SlotType`'s exhaustive-match discipline and `missing_layer`'s
named-error helper.

## Metadata

**Analog search scope:** `crates/pmcp-package/{src,tests}`, `crates/pmcp-server-toolkit/src`,
`crates/pmcp-openapi-server/tests`, `cargo-pmcp/src/commands/package/`
**Files read this session:** 14
**Pattern extraction date:** 2026-08-22
