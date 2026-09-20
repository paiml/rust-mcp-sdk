# Phase 122: Attestation Carriage - Pattern Map

**Mapped:** 2026-08-25
**Files analyzed:** 13 new/modified
**Analogs found:** 13 / 13 (11 exact, 2 role-match)

Every file in this phase modifies or copies an existing, shipped instance of the
pattern it needs. There is no "no analog" row — that is the phase's central
claim, and it held under search.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-package/src/oci/media_types.rs` (modify) | config/constants | — | same file, `MT_SERVER_OPENAPI_SPEC` at `:79` | exact (in-file) |
| `crates/pmcp-package/src/oci/pack.rs` — `AttestationFile<'a>` (new type) | model | file-I/O | `ConfigFile<'_>` / `OpenApiSpecFile<'_>` (same file) | exact |
| `crates/pmcp-package/src/oci/pack.rs` — `pack_server` 6th param | service | file-I/O (write) | `pack_server`'s existing `spec: Option<...>` arm, `pack.rs:427-436` | exact (in-file) |
| `crates/pmcp-package/src/oci/pack.rs` — `write_annotated_layer` | utility | file-I/O | `write_named_file_layer`, `pack.rs:163-177` | exact (generalize, don't copy) |
| `crates/pmcp-package/src/oci/pack.rs` — Gate A / Gate B free fns | middleware (guard) | transform | `reject_config_keys_without_a_config`, `pack.rs:251` | exact |
| `crates/pmcp-package/src/oci/pack.rs` — `pack_single_layer` attestation thread | service | file-I/O (write) | `pack_single_layer`, `pack.rs:471-485` | exact (in-file) |
| `crates/pmcp-package/src/oci/unpack.rs` — `UnpackedAttestation` + `UnpackedServer` field | model | file-I/O (read) | `RestoredFile` + `UnpackedServer.spec`, `unpack.rs:104-131` | exact |
| `crates/pmcp-package/src/oci/unpack.rs` — `read_attestation_layer` | utility | file-I/O (read) | `read_named_file_layer`, `unpack.rs:~200-227` | exact |
| `crates/pmcp-package/src/oci/unpack.rs` — `UnpackedTeam` (Pitfall 2) | model | file-I/O (read) | `UnpackedServer` (server side is already bespoke at `unpack.rs:331`) | role-match |
| `crates/pmcp-package/src/reference.rs` — `PinnedRef.resolved_from` | model | transform (serde) | `ConfigSlot.config_key`, `slot/types.rs:208-221` | exact |
| `crates/pmcp-package/src/package/team.rs` — `pinned_components`/`validate_all_pinned` | model (guard) | transform | `WorkflowManifest::pinned_components`, `workflow.rs:88-110` | exact |
| `crates/pmcp-package/deny.toml` (new) | config | — | `crates/pmcp-workbook-runtime/deny.toml` | exact |
| `Makefile` — `no-crypto-check` + cargo-pmcp gate-reach target | config/build | batch | `Makefile:1078-1101` (Layer 2) and `Makefile:492-525` (`test-openapi-server`) | exact |
| `contracts/pmcp-run/attestation-v1.graphql` (new) | config/contract | request-response (proposed) | `contracts/pmcp-run/capture-v1.graphql` | exact-but-inverted header |
| `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` — `VERIFY_ATTESTATION_QUERY` | config/constants | request-response | `SUBMIT_PACKAGE_CAPTURE_QUERY` (same file, `:26`) | exact (in-file) |
| `cargo-pmcp/tests/package_attestation_contract.rs` (new) | test | transform | `cargo-pmcp/tests/package_capture_contract.rs` | exact |
| `cargo-pmcp/src/commands/package/inspect.rs` — render + exit 1 | controller | request-response (CLI) | same file, `render_kind`/`render_server` at `:104-175` | exact (in-file) |
| SC5 live-leg test (file TBD) | test | request-response (network, gated) | `crates/pmcp-openapi-server/tests/parity_replay.rs:325-336` | exact |

## Pattern Assignments

### `crates/pmcp-package/src/oci/media_types.rs` (config/constants)

**Analog:** its own neighbours at `media_types.rs:73-85`.

```rust
// crates/pmcp-package/src/oci/media_types.rs:73,79,85
pub const MT_SERVER_CONFIG: &str = "application/vnd.pmcp.mcp-server.config.v1+toml";
pub const MT_SERVER_OPENAPI_SPEC: &str = "application/vnd.pmcp.mcp-server.openapi-spec.v1";
pub const MT_SERVER_BINARY_REF: &str = "application/vnd.pmcp.mcp-server.binary-ref.v1+json";
```

**Module-doc pattern to extend (the D-14 absence rule lives here — the new layer's
paragraph goes beside it, plus D-01's two-digest note per Pitfall 4):**

```rust
// crates/pmcp-package/src/oci/media_types.rs:21-31
//! - Two OPTIONAL vendor-content layers may follow: ... Both carry raw
//!   author bytes (never re-derived from a parsed struct) and record the
//!   original file name in their descriptor's
//!   `org.opencontainers.image.title` annotation. Either may be absent, and
//!   absence is exactly the layer NOT being in the manifest — there is no
//!   absence marker (D-14).
```

**Copy:** the suffix-free spelling of `MT_SERVER_OPENAPI_SPEC` (D-05), and the
"three optional layers" edit to the module docs. **Do not copy** `MT_SERVER_CONFIG`'s
`+toml`. Annotation key consts have **no in-repo precedent** — the only annotation key
used today is `oci_spec`'s `ANNOTATION_TITLE` — so RESEARCH's reverse-DNS proposal
(`run.pmcp.attestation.*`) is a new choice to record, not a pattern to follow.

---

### `crates/pmcp-package/src/oci/pack.rs` — optional layer write (service, file-I/O)

**Analog:** the spec arm of `pack_server`, same file.

**Signature + optional-arm pattern** (`pack.rs:373-436`):
```rust
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    validate_pack_preconditions(package, config)?;
    ...
    // Push order is deterministic so the manifest bytes (and therefore the
    // manifest digest) are reproducible. It is NOT a read-order contract:
    // the optional layers make position meaningless, and `unpack_server`
    // locates every layer by media type.
    let mut layers = vec![ /* 6 required layers */ ];

    if let Some(config) = config {
        layers.push(write_named_file_layer(layout, MT_SERVER_CONFIG, config.file_name, config.bytes)?);
    }
    // The spec is written last so the push order stays deterministic. Its
    // bytes go to the blob store RAW — never through `canonicalize` and never
    // parsed ...
    if let Some(spec) = spec {
        layers.push(write_named_file_layer(layout, MT_SERVER_OPENAPI_SPEC, spec.file_name, spec.bytes)?);
    }

    finalize_pack(layout, layers, ARTIFACT_TYPE_SERVER, &package.name, &package.version)
}
```

**Copy:** the `Option<XFile<'_>>` param placed before `layout`; the `if let Some(..)
{ layers.push(..) }` arm appended **after** the spec arm so push order stays
deterministic; the raw-bytes rule (`write_blob`, never `canonicalize`).

**Annotation-writing helper to GENERALIZE, not copy** (`pack.rs:163-177`) — it
hard-codes exactly one key and the attestation needs three:

```rust
fn write_named_file_layer(
    layout: &OciLayout, media_type: &str, file_name: &str, bytes: &[u8],
) -> Result<Descriptor> {
    // Raw author bytes, never `canonicalize` — the crate rule is to digest
    // what was stored, never re-derive it from a parsed struct.
    let mut descriptor = layout.write_blob(vendor_media_type(media_type), bytes)?;
    descriptor.set_annotations(Some(HashMap::from([(
        ANNOTATION_TITLE.to_string(), file_name.to_string(),
    )])));
    Ok(descriptor)
}
```

**Tamper-evidence rule this helper's rustdoc states — the reason annotations must be
LAYER-descriptor, never index-descriptor** (`pack.rs:158-162`):

```rust
/// Unlike the index-descriptor annotations set in [`finalize_pack`] — which are
/// applied AFTER `write_manifest` has already computed the manifest digest and
/// therefore do NOT feed it — a LAYER descriptor's annotations live inside the
/// manifest that `canonicalize` then hashes, so this annotation DOES feed the
/// manifest digest.
```

---

### `crates/pmcp-package/src/oci/pack.rs` — Gates A and B (middleware/guard, transform)

**Analog:** `reject_config_keys_without_a_config` at `pack.rs:251`, extracted as a
named free function out of `validate_pack_preconditions` for exactly this reason.

**Contract the extraction preserves** (`pack.rs:203-212`):
```rust
/// Every gate [`pack_server`] runs BEFORE its first `write_blob`, so a rejected
/// pack adds neither a blob nor an index entry ...
///
/// Extracted from `pack_server` rather than inlined so that function stays
/// under the repo's cognitive-complexity ceiling (CLAUDE.md: "Cognitive
/// complexity ≤25 per function", enforced in CI by
/// `pmat quality-gate --checks complexity`)
```

**Copy:** one named free function per gate, called from
`validate_pack_preconditions`; both run before the first `write_blob` (Gate B
therefore needs a *dry* canonicalization — `finalize_pack` canonicalizes once at
`pack.rs:516`).

**Error-type reuse for Gate A** — the existing range-vs-pin error text, verbatim from
`workflow.rs:88-99`, is what D-09 says to reuse:

```rust
    /// Borrow every component as a [`PinnedRef`], failing with
    /// `PackageError::InvalidReference` if ANY component in the graph is
    /// still a `Range` (a workflow manifest carries only pins).
    pub fn pinned_components(&self) -> Result<Vec<&PinnedRef>> {
        self.components.iter().map(|component| {
            component.as_pinned().ok_or_else(|| PackageError::InvalidReference {
                reason: format!(
                    "component '{}' is a Range, not a Pin — a WorkflowManifest may only contain exact pins",
                    component.name()
                ),
            })
        }).collect()
    }

    /// `Ok(())` iff every component is pinned; `Err(InvalidReference)`
    /// otherwise. A thin boolean-style guard over [`Self::pinned_components`].
    pub fn validate_all_pinned(&self) -> Result<()> {
        self.pinned_components().map(|_| ())
    }
```

**Error-hygiene rule to carry** (`error.rs:80-83`, on `ConfigSlotViolation`): *"Neither
ever carries the key's VALUE — a config slot may name a secret, and an error message is
the wrong place for one."* Attestation errors name the digest and the component, never
the attestation bytes.

---

### `crates/pmcp-package/src/package/team.rs` — pinned guard (model/guard, transform)

**Analog:** `workflow.rs:88-110` (quoted above) — the only existing instance.

**Difference the plan must handle:** `WorkflowManifest` has one flat
`components: Vec<ComponentRef>`; `TeamPackage` has **four** `ComponentRef` surfaces
(`team.rs:76-95`):

```rust
pub struct TeamPackage {
    pub name: String,
    pub version: semver::Version,
    /// The team's entry-point agent (from `AgentTeam.entryPointAgentId`).
    pub entry_point: ComponentRef,
    pub members: Vec<TeamMember>,          // each `.agent: ComponentRef`
    ...
    pub built_in_servers: Vec<ComponentRef>,
    pub finalizer_agents: Vec<ComponentRef>,
    ...
}
```

So the team version needs an iterator that chains all four before applying the identical
`as_pinned().ok_or_else(...)` mapping. **Copy the error shape and wording** (adapted to
name `component_type` per D-09, and to state the one-level depth limit); do not invent a
second guard idiom.

---

### `crates/pmcp-package/src/reference.rs` — `PinnedRef.resolved_from` (model, serde)

**Analog:** `crates/pmcp-package/src/slot/types.rs:208-221` — Phase 120 solved this exact
additive-optional-field problem and wrote the reasoning down:

```rust
/// # Compatibility — both halves, because only stating one under-scopes the next change
///
/// - **Serde/wire: ADDITIVE.** `#[serde(default)]` means slot JSON written before this
///   field existed still deserializes (yielding `None`), and `skip_serializing_if` means
///   nothing new is emitted for a `None`. No checked-in fixture byte and no pinned digest
///   moves. `skip_serializing_if` is load-bearing here, not cosmetic.
/// - **Rust source: BREAKING.** ...
#[serde(default, skip_serializing_if = "Option::is_none")]
pub config_key: Option<String>,
```

**Target struct today** (`reference.rs:47-54`):
```rust
/// An exact, digest-verified component pin. `version` and `digest` are both
/// mandatory (non-`Option`) — a pin always carries both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedRef {
    pub name: String,
    pub component_type: ComponentType,
    pub version: semver::Version,
    pub digest: ManifestDigest,
}
```

**Copy:** the two-halves compatibility rustdoc verbatim in structure, and the
`#[serde(default, skip_serializing_if = "Option::is_none")]` attribute pair. The type is
`Option<semver::VersionReq>`, matching `ComponentRef::Range.range` (`reference.rs:63`).

**Module-doc claim that must be updated, not silently invalidated** (`reference.rs:8-13`):
*"a [`PinnedRef`] whose `version` and `digest` fields are both non-`Option`. A pin can
never exist without a digest — this is a STRUCTURAL guarantee."* The new field is the
first `Option` on this struct; say why it is legitimately optional.

**Round-trip test pattern to copy** (`reference.rs:~131-175`): each test builds the
literal, `serde_json::to_value`, asserts specific keys, then round-trips back. Add a
`resolved_from`-present and a `resolved_from`-absent case in that shape.

---

### `crates/pmcp-package/src/oci/unpack.rs` — read side (model + utility, file-I/O)

**Analog:** `RestoredFile` + `read_named_file_layer` + `UnpackedServer`, same file.

**Result-struct shape** (`unpack.rs:104-131`):
```rust
/// A verbatim vendor-content file restored from a layer ...
/// `file_name` comes from the layer descriptor's
/// `org.opencontainers.image.title` annotation, which is ATTACKER-CONTROLLED
/// input from an untrusted layout. It is returned as DATA only ...
pub struct RestoredFile { pub file_name: String, pub bytes: Vec<u8> }

pub struct UnpackedServer {
    pub package: ServerPackage,
    pub binary: UnpackedBinary,
    /// The author's config file, if the package carried one.
    pub config: Option<RestoredFile>,
    /// The OpenAPI spec file, if the package carried one.
    pub spec: Option<RestoredFile>,
}
```

**Copy for `UnpackedAttestation`:** the "ATTACKER-CONTROLLED input ... returned as DATA
only" framing — it is precisely D-03's stance (subject mismatch is data, not an error),
and this crate already words it once.

**Annotation-read helper to mirror** (`unpack.rs:~200-227`) — note it treats a
present-layer-with-missing-annotation as a `Layout` error, which is the right precedent
for a missing `subject` annotation:

```rust
fn read_named_file_layer(
    layout: &OciLayout, descriptor: Option<&&Descriptor>, layer_name: &str,
) -> Result<Option<RestoredFile>> {
    let Some(descriptor) = descriptor else { return Ok(None) };
    let bytes = read_verified_blob(layout, descriptor)?;
    let file_name = descriptor.annotations().as_ref()
        .and_then(|a| a.get(ANNOTATION_TITLE))
        .ok_or_else(|| PackageError::Layout {
            reason: format!("the '{layer_name}' layer has no '{ANNOTATION_TITLE}' annotation"),
        })?.clone();
    Ok(Some(RestoredFile { file_name, bytes }))
}
```

**Call-site pattern** (`unpack.rs:380-390`) — the new read goes alongside these two, and
the struct literal gains one field:
```rust
    let config = read_named_file_layer(layout, by_media_type.get(MT_SERVER_CONFIG), "config")?;
    let spec = read_named_file_layer(layout, by_media_type.get(MT_SERVER_OPENAPI_SPEC), "openapi-spec")?;

    Ok(UnpackedServer { package, binary, config, spec })
```

**No change needed to layer location** (`unpack.rs:133-152`) — duplicate media types are
already rejected, so a crafted second attestation layer cannot shadow the real one:
```rust
/// A duplicate media type is rejected with [`PackageError::Layout`] naming the
/// duplicated type — never last-wins. Silently keeping one of two layers with
/// the same media type would let a crafted layout shadow the real config,
/// deploy descriptor or binary reference with an attacker's.
```

**`UnpackedTeam` (role-match only):** there is no team-side wrapper today
(`unpack_team -> Result<TeamPackage>`). Model it on `UnpackedServer`'s
`{ package, <optional carriage> }` shape; `unpack_server` at `unpack.rs:331` is already
the bespoke non-generic path, so it is the structural precedent for peeling teams out of
`unpack_single_layer`.

---

### `crates/pmcp-package/deny.toml` (config)

**Analog:** `crates/pmcp-workbook-runtime/deny.toml`, copied structurally in full.

```toml
# Crate-local cargo-deny config — Layer 2 of the Phase 91 purity gate (WBRT-04, D-09).
#
# Scope: this file is invoked ONLY via
#   cargo deny --manifest-path crates/pmcp-workbook-runtime/Cargo.toml \
#             --config crates/pmcp-workbook-runtime/deny.toml check bans
# ... this crate-local file is bans-only.

[advisories]
version = 2
ignore = []

[licenses]
version = 2
allow = []
confidence-threshold = 0.0

[sources]
unknown-registry = "allow"
unknown-git = "allow"

[bans]
multiple-versions = "allow"
wildcards = "allow"
deny = [ { name = "umya-spreadsheet" }, ... ]
```

**Copy:** the four-stanza skeleton, the permissive advisories/licenses/sources form, the
`multiple-versions`/`wildcards` allowances, and the header comment explaining the exact
invocation and why the workspace-global config is untouched.

**Invert one thing:** `deny = [...]` becomes `allow = [...]` (D-13), and the header
comment must carry the hashing-is-not-signing reasoning — the analog's header already
demonstrates the *style* (it justifies why `pmcp` and `zip` are deliberately NOT banned).

**Key-spelling constraint:** the Makefile parity grep is `{ name = `-keyed
(`Makefile:1091`). Use `{ name = "x" }` in the allowlist or spell the sibling group's
grep for `{ crate = ` — both are accepted by cargo-deny 0.18.3.

---

### `Makefile` — `no-crypto-check` (config/build, batch)

**Analog A — Layer 2 loop with the WR-02 fail-closed guard** (`Makefile:1078-1101`):
```make
	@# WR-02 fail-closed guard: cargo-deny 0.18.3 does NOT fail on a missing
	@# --config path — it WARNs and falls back to the default (empty-ban) config,
	@# reporting "bans ok" vacuously. A deleted/renamed crate-local deny.toml
	@# must FAIL the gate, not silently disable Layer 2. ...
	@set -euo pipefail; \
	ref=""; refcrate=""; \
	for crate in $(PURITY_CRATES); do \
	  test -f crates/$$crate/deny.toml || { echo "purity-check FAILED: crates/$$crate/deny.toml missing — Layer 2 would be vacuous; failing closed"; exit 1; }; \
	  bans=$$(grep -E '\{ name = ' crates/$$crate/deny.toml | sort); \
	  if [ -z "$$refcrate" ]; then ref="$$bans"; refcrate=$$crate; \
	  elif [ "$$bans" != "$$ref" ]; then \
	    echo "purity-check FAILED: ... must stay in lockstep"; exit 1; \
	  fi; \
	done; \
	for crate in $(PURITY_CRATES); do \
	  cargo deny --manifest-path crates/$$crate/Cargo.toml check --config deny.toml bans; \
	done
```

**Copy:** `set -euo pipefail`, the `test -f .../deny.toml` fail-closed guard with its
comment, and the exact `--manifest-path … check --config deny.toml bans` argument order
(cargo-deny 0.18.3 accepts `--config` only after `check`).

**Adapt:** the sibling list is one crate, so the parity loop is degenerate — say so
rather than shipping a loop that compares a list to itself. Add the allowlist-specific
guard the analog has no equivalent of: `grep -q '^allow = \['`, because cargo-deny
reports `bans ok` for an **empty** allow list exactly as vacuously as for a missing file.

**Analog B — gate-reach with `REQUIRED_TEST_BINARIES`** (`Makefile:492-525`), the exact
target to mirror for cargo-pmcp's `tests/` (Pitfall 1):
```make
	@out=$$(... $(CARGO) test -p pmcp-openapi-server -- --test-threads=1 2>&1); \
	status=$$?; echo "$$out"; if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then echo "... 0 tests — the gate is not reaching this crate"; exit 1; fi; \
	REQUIRED_TEST_BINARIES="parity_replay pmcp_package_pin roundtrip_e2e"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v want="tests/$$b.rs" -f scripts/named-test-binary-count.awk); \
		case "$$n" in \
		-1) echo "✗ required test binary '$$b' never RAN ..."; exit 1;; \
		-2) echo "✗ ... printed a target line but NO 'test result:' line followed it ..."; exit 1;; \
		0)  echo "✗ ... RAN but passed ZERO tests ..."; exit 1;; \
		''|*[!0-9]*) echo "✗ ... the count extractor produced no usable reading ('$$n') ..."; exit 1;; \
		*)  echo "  ✓ $$b passed $$n tests";; \
		esac; \
	done
```
**Copy this whole `case` block verbatim** including all four failure messages — each
verdict exists for a measured failure class. Substitute the crate/binary list
(`package_attestation_contract package_capture_contract package_inspect pmcp_package_pin`).
Both new targets chain into `quality-gate` the way `purity-check` and `pmcp-package-gate`
already do (`Makefile:1134`).

---

### `contracts/pmcp-run/attestation-v1.graphql` (config/contract)

**Analog:** `contracts/pmcp-run/capture-v1.graphql:1-9` — copy the header *shape*,
**invert its content**:
```graphql
# capture-v1.graphql — pmcp.run package-capture contract (SDL subset)
# Source:   pmcp.run dev amplifyData AppSync API, apiId q3gd4hrbabeytc2o2zazld6igy (us-east-1)
# Exported: 2026-07-20 via `aws appsync get-introspection-schema --format SDL`, reduced to
#           the two capture operations + their return types by the pmcp.run platform team.
# Version:  v1
#
# OWNERSHIP: the platform owns this file's CONTENTS. When the capture schema changes,
# the platform re-exports and PRs an update here; the SDK's blocking contract test then
# forces cargo-pmcp's queries/structs to follow in the same PR.
```
**Copy:** the `# <file> — <purpose> (SDL subset)` first line, the `Version:` line, an
explicit ownership paragraph, and the `String`-not-enum discipline.
**Do NOT copy:** the `Source:`/`Exported:` provenance lines — D-07's whole point is that
this file has none. Replace with `STATUS: SDK-PROPOSED. NOT PLATFORM-EXPORTED. AWAITING
RATIFICATION.` and a pointer to the ask in
`docs/platform-requests/package-portability-alignment.md`.

---

### `cargo-pmcp/.../graphql_contract.rs` — `VERIFY_ATTESTATION_QUERY` (constants)

**Analog:** same file, `:22-45`.
```rust
/// The exact `submitPackageCapture` operation the CLI sends. Shared with the
/// offline contract test (`tests/package_capture_contract.rs`) so the test
/// validates the real runtime query against the vendored SDL.
pub const SUBMIT_PACKAGE_CAPTURE_QUERY: &str = r#"
        mutation SubmitPackageCapture(
            $rootComponentType: String!, ...
        ) { submitPackageCapture(...) { captureId status createdAt } }
    "#;
```

**Why the constant lives in this narrow leaf** (`graphql_contract.rs:9-18` — the
architectural reason, worth restating in the new const's docs):
```rust
//! This file exists ONLY so the offline blocking contract test
//! (`tests/package_capture_contract.rs`) can validate the real runtime
//! queries against the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`)
//! without pulling `pmcp_run`'s auth/deploy/reqwest tree into the `cargo-pmcp`
//! lib target.
```
Mounted at `cargo-pmcp/src/lib.rs:174-177`:
```rust
#[doc(hidden)]
#[path = "deployment/targets/pmcp_run/graphql_contract.rs"]
pub mod pmcp_run_graphql;
```
**Copy:** the raw-string const with an indented operation body, and the doc sentence
naming which test consumes it.

---

### `cargo-pmcp/tests/package_attestation_contract.rs` (test)

**Analog:** `cargo-pmcp/tests/package_capture_contract.rs` — mirror all three tests.

**Module docs + schema loader** (`:1-29`):
```rust
//! Offline blocking contract test (170-08 Task 3).
//! ...
//! This is a pure offline/static check: no network access, no pmcp.run
//! credentials. It runs in the normal `cargo test` workspace gate, so any
//! drift between the CLI's queries and the platform contract fails the SDK
//! build immediately rather than surfacing at runtime against a live server.

use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

const SDL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../contracts/pmcp-run/capture-v1.graphql");

// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it ...
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read capture-v1.graphql");
    Schema::parse_and_validate(sdl, "capture-v1.graphql").expect("vendored SDL is itself valid")
}
```
**Correction to make when copying the docs:** the analog claims *"It runs in the normal
`cargo test` workspace gate"* — RESEARCH measured that this is **false** today
(`named-test-binary-count.awk` reports `-1`). The new file's docs may only make that
claim once the Analog-B Makefile target exists.

**Test 1 — op validation** (`:31-48`): loop `[(name, CONST)]`, `parse_and_validate`,
`unwrap_or_else(|e| panic!("`{name}` op does not match ...: {e}"))`.

**Test 2 — shape pin** (`:50-65`): assert `sdl.contains("status: String")` and
`!sdl.contains("enum ...")`.

**Test 3 — selection-set drift** (`:67-105`), including its honest limitation note,
which D-07 says to match in candour:
```rust
/// NOTE: this greps the hardcoded field-name lists below against the query
/// STRINGS, not the actual `CaptureInfo`/`CaptureStatus` Rust struct
/// definitions — it does NOT parse those structs. ... So this test is a drift
/// sanity check on the selection set, not a full struct-vs-schema proof
```
**Add** the D-07-specific limitation the analog cannot have: this file validates
SDK-written queries against an SDK-written schema, so it pins SDK-internal agreement
only and cannot detect drift from a platform that has not spoken.

---

### `cargo-pmcp/src/commands/package/inspect.rs` (controller, CLI)

**Analog:** the same file's existing dispatch and renderers.

**Current dispatch — the two lines Pitfall 7 flags** (`:104-131`):
```rust
/// Unpack the resolved kind (digest-verified) and render it when output is
/// enabled. Unpacking runs even in quiet mode so tamper/digest failures still
/// surface — only the decorative rendering is gated.
fn render_kind(layout: &OciLayout, kind: PackageKind, output: bool) -> Result<()> {
    match kind {
        PackageKind::Team => {
            let pkg = unpack_team(layout).context("unpack team package")?;
            if output { render_team(&pkg); }
        },
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            if output { render_server(&unpacked.package); }   // <- attestation discarded here
        },
        ...
    }
    Ok(())
}
```
**Change:** pass `&unpacked` (not `&unpacked.package`) to `render_server`; return an
`Err` on subject mismatch **outside** the `if output` block, so the D-06 exit-1 holds in
quiet mode too — matching the docstring's existing rule.

**Rendering pattern to extend** (`:141-175`):
```rust
fn field(label: &str, value: impl std::fmt::Display) {
    println!("  {:<14} {}", format!("{label}:").bright_black(), value);
}
fn header(kind: PackageKind) {
    println!("\n{}", "Package".bright_cyan().bold());
    field("Kind", kind.label().bright_green().bold());
}
fn render_server(pkg: &ServerPackage) {
    header(PackageKind::Server);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Config slots", pkg.config_slots.len());
}
```
**Copy:** `header(..)` then a run of `field("Label", value)` calls, using `colored`'s
`.bright_*()` for emphasis. The three D-06 states become three `field` groups
(issuer / claimed subject / actual unattested digest side by side on mismatch).

**Module-doc rule D-03 deliberately departs from — the departure must be written HERE**
(`inspect.rs:1-12`):
```rust
//! ... Digest verification lives inside `unpack_*`; failures
//! surface verbatim (V6), never bypassed.
```

**Layout invariant that ruled out an OCI referrers sidecar** (`:52-59`) — do not
"improve" it:
```rust
    // Reject a zero/multiple-manifest index — do NOT index blindly (Codex MEDIUM).
    let manifests = index.manifests();
    if manifests.len() != 1 { bail!("expected exactly one manifest in {}, found {} ...", ...); }
```

---

### SC5 live-leg test (test, gated network)

**Analog:** `crates/pmcp-openapi-server/tests/parity_replay.rs:325-336`:
```rust
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real TFL_APP_KEY"]
async fn parity_live_tfl() {
    // Double-gate: even when run with --ignored, bail unless explicitly enabled
    // AND a real key is present (never hit the live API by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("parity_live_tfl skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
```
**Copy:** `#[ignore = "<reason naming the env vars>"]`, the explicit `!= Some("1")` early
return, and the `eprintln!` that says how to enable. Keep the gate *double* (enable flag
+ credential/endpoint var).

**Placement constraint measured in RESEARCH:** a binary whose every test is `#[ignore]`d
reports `0 passed`, which `named-test-binary-count.awk` treats as a failure. Put this
test in a binary that also holds at least one non-ignored test.

## Shared Patterns

### Rustdoc states what a mechanism CANNOT see
**Source:** `cargo-pmcp/tests/package_capture_contract.rs:67-75`; `crates/pmcp-package/src/oci/pack.rs:158-162`; `crates/pmcp-package/src/slot/types.rs:208-221`.
**Apply to:** every artifact this phase adds — the D-09 one-level depth limit, the D-07
SDK-authored-schema limitation, the D-01 two-digest consequence, the D-10 `None`
ambiguity. The repo's convention is that a limitation is documented at its own site, not
discovered by the next reader.

### Fail-closed on unreadable evidence
**Source:** `Makefile:1085-1090` (missing `deny.toml`) and `Makefile:508-521` (the
`-1`/`-2`/`0`/non-numeric verdicts).
**Apply to:** both new Makefile targets. Never let a gate report success on output it
could not read.

### Raw author bytes, never re-derived
**Source:** `crates/pmcp-package/src/oci/pack.rs:170-172`, `:427-431`.
**Apply to:** the attestation write and read. `write_blob`, never `canonicalize`;
`serde_json::from_slice` on attestation bytes anywhere in `pmcp-package` fails PKGX-01.

### Errors name the identifier, never the payload
**Source:** `crates/pmcp-package/src/error.rs:80-83` (`ConfigSlotViolation`).
**Apply to:** Gate A and Gate B error text — name the component, its `component_type`,
and the digests; never the attestation bytes.

### Optional means absent, with no marker
**Source:** `crates/pmcp-package/src/oci/media_types.rs:21-31` (D-14).
**Apply to:** `MT_SERVER_ATTESTATION` and `UnpackedServer`'s new field.

## No Analog Found

None. Every file has at least a role-match analog in-repo.

Two analogs are **imperfect for the new case and must be adapted rather than copied** —
these are the phase's real engineering risk, per RESEARCH's key insight:

| File | Analog | Why it doesn't fit as-is |
|------|--------|--------------------------|
| `pack.rs` annotation helper | `write_named_file_layer:163-177` | Hard-codes ONE annotation key; the attestation needs three. Generalize to a `HashMap<String,String>` or add a sibling `write_annotated_layer` — do not copy-paste. |
| `unpack.rs` team result | `UnpackedServer:120-131` | `unpack_team` returns a bare `TeamPackage` with nowhere to hold the D-03 verdict. Requires a new `UnpackedTeam` (breaking) or a sibling fn (two mechanisms). Decide in the plan; RESEARCH Open Question 1 recommends breaking it. |

One analog carries a **factually stale claim** that must not be copied forward:
`package_capture_contract.rs:11-13` says it "runs in the normal `cargo test` workspace
gate". Measured: it runs in no gate at all.

## Metadata

**Analog search scope:** `crates/pmcp-package/src/{oci,package,slot}/`, `crates/pmcp-package/tests/`,
`cargo-pmcp/src/commands/package/`, `cargo-pmcp/src/deployment/targets/pmcp_run/`,
`cargo-pmcp/tests/`, `contracts/pmcp-run/`, `crates/pmcp-workbook-{runtime,dialect}/deny.toml`,
`crates/pmcp-openapi-server/tests/`, `Makefile`, `scripts/`.
**Files scanned:** 14 read (targeted ranges), plus RESEARCH.md's verified excerpts.
**Pattern extraction date:** 2026-08-25
