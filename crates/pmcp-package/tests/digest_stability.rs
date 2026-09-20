//! Integration test: digest-stability guarantees over the checked-in
//! golden fixtures.
//!
//! - `manifest_digest()` computed repeatedly (≥100 times) over the same
//!   fixture value is always identical.
//! - Re-ordering a source collection field (a `BTreeMap`'s insertion order, or
//!   the order `WorkflowManifest::new` receives its `components`/
//!   `aggregated_slots` arguments) does not change the resulting canonical
//!   digest — olpc-cjson key-sorting for maps, and `WorkflowManifest::new`'s
//!   own deterministic sort for its `Vec` fields.
//! - `AgentPackage` (which now carries no bare-float field) routes through the
//!   SAME `manifest_digest()`/`canonicalize()` path as the other three package
//!   types — proven directly by the agent stability test below.

use pmcp_package::digest::{canonicalize, manifest_digest, verify};
use pmcp_package::error::PackageError;
use pmcp_package::package::{AgentPackage, ServerPackage, TeamPackage, WorkflowManifest};
use pmcp_package::reference::{ComponentRef, ComponentType};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;

mod common;
use common::{london_tube_spec_bytes, pack_london_tube, read_manifest_bytes};

/// Read a checked-in golden fixture's raw bytes (delegates to the shared
/// `common::fixture_bytes` so the path/panic logic lives once per crate).
fn read_fixture(name: &str) -> Vec<u8> {
    common::fixture_bytes(name)
}

// --- Pinned wire-freeze digests (PRIMARY gate) -----------------------------
//
// Each constant is the `manifest_digest(value).as_str()` (`sha256:<64-hex>`)
// of the corresponding golden fixture, computed once at authoring time and
// checked in. A change to ANY serialized field of a package kind — a removed
// field (dropped on deserialize) or a defaulted-in new field — alters the
// canonical bytes and therefore this digest, so the matching assertion below
// FAILS CI. This is a real wire freeze for the 0.2.x line, NOT just
// determinism: the day these must change is the day the format breaks again.
// Bump the version intentionally — do NOT silently repin.
//
// `EXPECTED_SERVER_DIGEST` moved once already, at the 0.1.x -> 0.2.0 break:
// D-08 removed `ServerPackage.binary_ref` (which binary a package names is a
// LAYER, not a struct field) and D-09 took that break deliberately rather than
// carrying a dead field forward. That is the ONLY sanctioned reason this
// constant has changed. The other three pinned constants below were untouched
// by that break — their shapes did not change, so if any of them ever moves,
// that is a real defect and not a repin.
//
// The `<kind>.canonical.json` snapshots asserted byte-equal via `canonicalize`
// are the belt-and-suspenders second gate (catches a silent field add/remove
// even in the theoretical case a digest were to collide).

const EXPECTED_SERVER_DIGEST: &str =
    "sha256:1d8a792e6f7dc7c4e965fdd65e246e7bca416a5adf8fdd9f1d2e7693273a9c77";
const EXPECTED_WORKFLOW_DIGEST: &str =
    "sha256:ef8a7a08efd28f95128db481d5b8ba809516ef1097cbd8ded847ccf9de5aa7af";
const EXPECTED_AGENT_DIGEST: &str =
    "sha256:9e502b0b2e422dbf04a1d7d3d677e396a329df4dac0368afa26a29eb741e8d5a";
const EXPECTED_TEAM_DIGEST: &str =
    "sha256:79cb29da5025528681674866d006f3bbc8ac63991fb0557cb97b5755fa34bd73";

// Checked-in canonical-JSON snapshots (secondary gate).
const SERVER_CANONICAL: &[u8] = include_bytes!("golden_fixtures/canonical/server.canonical.json");
const WORKFLOW_CANONICAL: &[u8] =
    include_bytes!("golden_fixtures/canonical/workflow.canonical.json");
const AGENT_CANONICAL: &[u8] = include_bytes!("golden_fixtures/canonical/agent.canonical.json");
const TEAM_CANONICAL: &[u8] = include_bytes!("golden_fixtures/canonical/team.canonical.json");

fn server_fixture() -> ServerPackage {
    serde_json::from_slice(&read_fixture("server_team_fs_v1.json")).unwrap()
}
fn workflow_fixture() -> WorkflowManifest {
    serde_json::from_slice(&read_fixture("workflow_claims_triage_v1.json")).unwrap()
}
fn agent_fixture() -> AgentPackage {
    serde_json::from_slice(&read_fixture("agent_pto_researcher_v1.json")).unwrap()
}
fn team_fixture() -> TeamPackage {
    serde_json::from_slice(&read_fixture("team_small_review_v1.json")).unwrap()
}

// --- PRIMARY gate: pinned canonical digest per kind ------------------------

#[test]
fn server_fixture_digest_matches_pinned_wire_freeze_constant() {
    assert_eq!(
        manifest_digest(&server_fixture()).unwrap().as_str(),
        EXPECTED_SERVER_DIGEST,
        "ServerPackage serialized shape changed — this is a wire-freeze break (bump the version \
         intentionally, do not silently repin)"
    );
}

#[test]
fn workflow_fixture_digest_matches_pinned_wire_freeze_constant() {
    assert_eq!(
        manifest_digest(&workflow_fixture()).unwrap().as_str(),
        EXPECTED_WORKFLOW_DIGEST,
        "WorkflowManifest serialized shape changed — wire-freeze break (bump the version intentionally)"
    );
}

#[test]
fn agent_fixture_digest_matches_pinned_wire_freeze_constant() {
    assert_eq!(
        manifest_digest(&agent_fixture()).unwrap().as_str(),
        EXPECTED_AGENT_DIGEST,
        "AgentPackage serialized shape changed — wire-freeze break (bump the version intentionally)"
    );
}

#[test]
fn team_fixture_digest_matches_pinned_wire_freeze_constant() {
    assert_eq!(
        manifest_digest(&team_fixture()).unwrap().as_str(),
        EXPECTED_TEAM_DIGEST,
        "TeamPackage serialized shape changed — wire-freeze break (bump the version intentionally)"
    );
}

// --- SECONDARY gate: canonical-bytes snapshot per kind ---------------------

#[test]
fn server_canonical_bytes_match_checked_in_snapshot() {
    assert_eq!(
        canonicalize(&server_fixture()).unwrap(),
        SERVER_CANONICAL,
        "ServerPackage canonical bytes diverged from the checked-in snapshot"
    );
}

#[test]
fn workflow_canonical_bytes_match_checked_in_snapshot() {
    assert_eq!(
        canonicalize(&workflow_fixture()).unwrap(),
        WORKFLOW_CANONICAL,
        "WorkflowManifest canonical bytes diverged from the checked-in snapshot"
    );
}

#[test]
fn agent_canonical_bytes_match_checked_in_snapshot() {
    assert_eq!(
        canonicalize(&agent_fixture()).unwrap(),
        AGENT_CANONICAL,
        "AgentPackage canonical bytes diverged from the checked-in snapshot"
    );
}

#[test]
fn team_canonical_bytes_match_checked_in_snapshot() {
    assert_eq!(
        canonicalize(&team_fixture()).unwrap(),
        TEAM_CANONICAL,
        "TeamPackage canonical bytes diverged from the checked-in snapshot"
    );
}

// --- D-10: `resolved_from` participates in package identity ----------------

/// The four pinned constants above prove a `None` `resolved_from` moves
/// NOTHING — they are computed over fixtures carrying no `resolved_from` key
/// and they still match unmodified. This test proves the COMPLEMENT: recording
/// the range a pin resolved changes the canonical bytes and therefore the
/// manifest digest.
///
/// Both halves are needed. Without this one, `skip_serializing_if` would be
/// indistinguishable from a field the digest path ignores entirely — and a
/// "compatible" field that never reaches the digest is a field an attacker can
/// strip or forge for free, which would make D-10's dev-to-prod signal
/// unattestable.
///
/// `workflow.canonical.json` is the fixture used because it is the only
/// canonical fixture holding `"kind":"pinned"` refs (three of them); `agent`
/// and `team` hold ranges and `server` holds neither.
#[test]
fn recording_the_range_a_pin_resolved_changes_the_manifest_digest() {
    let none_variant = workflow_fixture();
    assert!(
        none_variant.components.iter().any(ComponentRef::is_pinned),
        "the workflow fixture must carry at least one pin or this test asserts nothing"
    );

    let none_digest = manifest_digest(&none_variant).unwrap();
    assert_eq!(
        none_digest.as_str(),
        EXPECTED_WORKFLOW_DIGEST,
        "a `None` resolved_from must emit no key, so the pinned wire-freeze constant must \
         still match — this is what makes the assert_ne below attributable to the field \
         rather than to some unrelated drift"
    );

    let mut some_variant = none_variant.clone();
    let pin = some_variant
        .components
        .iter_mut()
        .find_map(|component| match component {
            ComponentRef::Pinned(pin) => Some(pin),
            ComponentRef::Range { .. } => None,
        })
        .expect("asserted present above");
    pin.resolved_from = Some(semver::VersionReq::parse("^1.2").unwrap());

    let some_digest = manifest_digest(&some_variant).unwrap();
    assert_ne!(
        none_digest, some_digest,
        "recording a resolved range MUST change package identity — a pin that declares it \
         resolved `^1.2` is a different fact from a pin that declares nothing, and the \
         digest is where that difference has to show up"
    );
}

// --- Round-trip parse checks for the two new kinds -------------------------

#[test]
fn agent_fixture_deserializes_as_agent_package() {
    let agent = agent_fixture();
    assert_eq!(agent.name, "pto-researcher");
    assert_eq!(agent.connectors.len(), 1);
}

#[test]
fn team_fixture_deserializes_as_team_package() {
    let team = team_fixture();
    assert_eq!(team.name, "small-review");
    assert_eq!(team.members.len(), 2);
    assert_eq!(team.human_roles.len(), 1);
}

// --- Determinism: ≥100 recomputations for the two new fixture-backed kinds --
// (server/workflow retain their existing ≥100 tests below; the in-code agent
//  ≥100 test also remains — this adds fixture-backed agent + team coverage.)

#[test]
fn agent_fixture_digest_is_stable_across_100_computations() {
    let agent = agent_fixture();
    let first = manifest_digest(&agent).unwrap();
    for _ in 0..100 {
        assert_eq!(manifest_digest(&agent).unwrap(), first);
    }
}

#[test]
fn team_fixture_digest_is_stable_across_100_computations() {
    let team = team_fixture();
    let first = manifest_digest(&team).unwrap();
    for _ in 0..100 {
        assert_eq!(manifest_digest(&team).unwrap(), first);
    }
}

#[test]
fn server_fixture_digest_is_stable_across_100_computations() {
    let bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&bytes).unwrap();

    let first = manifest_digest(&package).unwrap();
    for _ in 0..100 {
        let next = manifest_digest(&package).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
        );
    }
}

#[test]
fn workflow_fixture_digest_is_stable_across_100_computations() {
    let bytes = read_fixture("workflow_claims_triage_v1.json");
    let manifest: WorkflowManifest = serde_json::from_slice(&bytes).unwrap();

    let first = manifest_digest(&manifest).unwrap();
    for _ in 0..100 {
        let next = manifest_digest(&manifest).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
        );
    }
}

/// `AgentPackage` now carries no bare-float field, so it is
/// `canonicalize()`-able and routes through the exact same
/// `manifest_digest()` path as `ServerPackage`/`TeamPackage`/
/// `WorkflowManifest`. This test proves that path is stable for an
/// `AgentPackage` (it would have returned an `Err` from `manifest_digest()`
/// under the old `temperature: f64` schema, since olpc-cjson rejects floats).
#[test]
fn agent_package_digest_is_stable_across_100_computations_via_canonicalize() {
    let package = AgentPackage {
        name: "claims-triage-agent".to_string(),
        version: semver::Version::parse("1.2.0").unwrap(),
        instructions: "You triage incoming insurance claims.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        }),
        max_tokens: 8192,
        max_iterations: 15,
        connectors: vec![ComponentRef::Range {
            name: "claims-database".to_string(),
            range: semver::VersionReq::parse("^2").unwrap(),
            component_type: ComponentType::Server,
        }],
        tool_selection: Some(serde_json::json!({ "claims-database": ["lookup_claim"] })),
        input_schema: None,
        output_schema: Some(serde_json::json!({ "type": "object" })),
        importance: Some("HIGH".to_string()),
        finalizer_role: Some("formatter".to_string()),
        budget_defaults: vec![ConfigSlot::new(SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "500".to_string(),
        })],
    };

    let first = manifest_digest(&package)
        .expect("AgentPackage must be canonicalize()-able now that it carries no bare float");
    for _ in 0..100 {
        let next = manifest_digest(&package).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
        );
    }
}

#[test]
fn reordering_deploy_descriptor_environment_map_does_not_change_digest() {
    // Build the SAME logical DeployDescriptor via two different BTreeMap
    // insertion orders and confirm the digest matches (key-order
    // independence via olpc-cjson).
    let bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&bytes).unwrap();
    assert!(
        package.deploy.environment.len() >= 2,
        "fixture must declare at least 2 environment entries to make reordering meaningful"
    );

    let mut forward = BTreeMap::new();
    for (k, v) in package.deploy.environment.iter() {
        forward.insert(k.clone(), v.clone());
    }
    let mut backward = BTreeMap::new();
    for (k, v) in package.deploy.environment.iter().rev() {
        backward.insert(k.clone(), v.clone());
    }

    let mut package_forward = package.clone();
    package_forward.deploy.environment = forward;
    let mut package_backward = package.clone();
    package_backward.deploy.environment = backward;

    let digest_forward = manifest_digest(&package_forward).unwrap();
    let digest_backward = manifest_digest(&package_backward).unwrap();
    assert_eq!(
        digest_forward, digest_backward,
        "canonical digest must not depend on map insertion order"
    );
}

#[test]
fn reordering_workflow_components_and_slots_via_new_yields_same_digest() {
    // WorkflowManifest::new sorts `components`/`aggregated_slots`
    // deterministically regardless of the order they're supplied in —
    // confirm two differently-ordered constructions of the SAME logical
    // manifest produce the same digest.
    let bytes = read_fixture("workflow_claims_triage_v1.json");
    let manifest: WorkflowManifest = serde_json::from_slice(&bytes).unwrap();
    assert!(
        manifest.components.len() >= 2,
        "fixture must declare at least 2 components to make reordering meaningful"
    );

    let forward = WorkflowManifest::new(
        manifest.name.clone(),
        manifest.version.clone(),
        manifest.components.clone(),
        manifest.aggregated_slots.clone(),
        manifest.provenance.clone(),
    );

    let mut reversed_components = manifest.components.clone();
    reversed_components.reverse();
    let mut reversed_slots = manifest.aggregated_slots.clone();
    reversed_slots.reverse();
    let backward = WorkflowManifest::new(
        manifest.name.clone(),
        manifest.version.clone(),
        reversed_components,
        reversed_slots,
        manifest.provenance.clone(),
    );

    let digest_forward = manifest_digest(&forward).unwrap();
    let digest_backward = manifest_digest(&backward).unwrap();
    assert_eq!(
        digest_forward, digest_backward,
        "WorkflowManifest::new must sort deterministically regardless of input order"
    );
}

// =========================================================================
// Plan 120-05 Task 4 (D-12) — the PACKED-MANIFEST golden.
//
// The four constants above pin `manifest_digest(&struct)`, which is a
// function of the STRUCT's serialized fields and is therefore BLIND to the
// layer set, the layer order, the media-type strings and the layer
// annotations. This one packs a real layout and pins what `finalize_pack`
// RETURNS, which is the only value in the crate that moves when any of those
// four change. That difference is the whole point of adding it.
// =========================================================================

/// The packed OCI manifest digest of the WITH-SPEC config-only london-tube
/// package.
///
/// # What this constant is a function of — and what it is NOT
///
/// It is `sha256(canonical manifest bytes)`, i.e. a function of
/// `{schemaVersion, mediaType, artifactType, the config descriptor, and per
/// layer: mediaType + size + digest + annotations}`.
///
/// It is **NOT** a function of the package's name or version: the index
/// descriptor's `name`/`version` annotations are applied AFTER `write_manifest`
/// has already computed the manifest digest. So "proving" this golden moves by
/// bumping the package version yields a FALSE GREEN — mutate a layer instead.
///
/// # Why the WITH-SPEC case (D-14)
///
/// The spec layer is optional. Pinning the without-spec shape would let the
/// spec-less path quietly become the default while this test stayed green, so
/// the pinned case carries the spec and a companion assertion below requires
/// the without-spec digest to DIFFER.
///
/// Authored by the procedure the four struct goldens used: write the assertion
/// with a placeholder, run once, copy `actual` out of the failure. Never
/// computed through a second code path — `pack.rs` keeps one hash with one
/// source of truth.
const EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST: &str =
    "sha256:5bd6bad19359c974d487e2754323c075a57c58559bac1619eb29d005882cf7e1";

#[test]
fn config_server_packed_manifest_digest_matches_pinned_constant() {
    let dir = tempfile::tempdir().unwrap();
    let (_, digest) = pack_london_tube(dir.path(), Some(&london_tube_spec_bytes()));
    assert_eq!(
        digest.as_str(),
        EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST,
        "the PACKED manifest digest of the with-spec config-only package changed. Unlike the \
         four struct-level goldens, this one sees the layer SET, the layer ORDER, the \
         media-type strings and the layer filename annotations — so one of those four moved, \
         and a previously published CLI can no longer read this package. Bump the format \
         version intentionally; do NOT silently repin."
    );
}

#[test]
fn the_with_spec_and_without_spec_packed_digests_differ() {
    let with_dir = tempfile::tempdir().unwrap();
    let (_, with_spec) = pack_london_tube(with_dir.path(), Some(&london_tube_spec_bytes()));
    let without_dir = tempfile::tempdir().unwrap();
    let (_, without_spec) = pack_london_tube(without_dir.path(), None);
    assert_ne!(
        with_spec, without_spec,
        "the pinned golden must not be satisfiable by a package that dropped its spec layer — \
         if these were equal, the spec-less shape could quietly become the default"
    );
}

/// PKG-03 criterion 3: the spec is BAKED into the package, so one byte of it is
/// a different package — and the stale digest is REJECTED against the new
/// manifest's bytes.
///
/// Both halves are required. `assert_ne!` alone shows the digest moved but not
/// that anything catches a stale one; the `verify` half is what makes tamper
/// detection a property rather than an observation.
#[test]
fn one_flipped_spec_byte_moves_the_packed_digest_and_the_stale_one_is_rejected() {
    let spec = london_tube_spec_bytes();
    let mut mutated = spec.clone();
    mutated[0] ^= 0x01;

    let dir_a = tempfile::tempdir().unwrap();
    let (_, digest_a) = pack_london_tube(dir_a.path(), Some(&spec));
    let dir_b = tempfile::tempdir().unwrap();
    let (layout_b, digest_b) = pack_london_tube(dir_b.path(), Some(&mutated));

    assert_ne!(
        digest_a, digest_b,
        "the OpenAPI spec is BAKED into the package: a one-byte change is a different package, \
         never a slot a target environment fills in"
    );

    let manifest_bytes_b = read_manifest_bytes(&layout_b);
    // Sanity: the NEW digest does verify against the NEW bytes, so the failure
    // below is about staleness and not about reading the wrong blob.
    verify(&digest_b, &manifest_bytes_b).expect("the fresh digest must verify against its bytes");

    let err = verify(&digest_a, &manifest_bytes_b)
        .expect_err("the stale digest must not verify against the new manifest");
    assert!(
        matches!(err, PackageError::DigestMismatch { .. }),
        "expected DigestMismatch, got: {err}"
    );
}
