//! Integration test: golden-fixture-based + programmatically-built
//! round-trip tests for all four AI-Package types (losslessness), plus
//! the canonical-byte-identity assertion against the two checked-in
//! golden fixtures (`tests/golden_fixtures/*.json`).
//!
//! `server_team_fs_v1.json` and `workflow_claims_triage_v1.json` are stored
//! in CANONICAL byte form (the exact bytes `canonicalize()` emits — compact,
//! olpc-cjson key-sorted JSON), authored by a one-off generator that wrote
//! `canonicalize(&value)`'s output directly to disk (see the plan's
//! recommended authoring procedure). The byte-identical assertion below
//! therefore compares the fixture bytes against `canonicalize(&parsed)` and
//! cannot fight the canonicalizer.
//!
//! `AgentPackage`/`TeamPackage` are NOT fixture-backed (no checked-in golden
//! file for either) — they are built programmatically here and only
//! round-tripped through pack/unpack, per this plan's scope (only two golden
//! fixtures are required: one server, one workflow).

use pmcp_package::digest::{canonicalize, ManifestDigest};
use pmcp_package::oci::media_types::{
    ANNOTATION_ATTESTATION_ISSUER, ANNOTATION_ATTESTATION_PAYLOAD_TYPE,
    ANNOTATION_ATTESTATION_SUBJECT, ARTIFACT_TYPE_SERVER, ARTIFACT_TYPE_TEAM, MT_ATTESTATION,
    MT_TEAM_CONFIG,
};
use pmcp_package::oci::{
    pack_agent, pack_server, pack_team, pack_workflow, unpack_agent, unpack_server, unpack_team,
    unpack_workflow, AttestationFile, BinaryMode, OciLayout, UnpackedAttestation, UnpackedBinary,
};
use pmcp_package::package::{
    AgentPackage, HumanRole, ServerPackage, TeamLimits, TeamMember, TeamPackage, TeamRole,
    WorkflowManifest,
};
use pmcp_package::reference::{ComponentRef, ComponentType, PinnedRef};
use pmcp_package::slot::{ConfigSlot, SlotType};
use pmcp_package::PackageError;
mod common;

/// Read a checked-in golden fixture's raw bytes (delegates to the shared
/// `common::fixture_bytes` so the path/panic logic lives once per crate).
fn read_fixture(name: &str) -> Vec<u8> {
    common::fixture_bytes(name)
}

// ---------------------------------------------------------------------
// Fixture-backed types: ServerPackage, WorkflowManifest
// ---------------------------------------------------------------------

#[test]
fn server_package_fixture_round_trips_and_matches_canonical_bytes() {
    let fixture_bytes = read_fixture("server_team_fs_v1.json");
    let parsed: ServerPackage =
        serde_json::from_slice(&fixture_bytes).expect("fixture must parse as ServerPackage");

    // canonicality: the checked-in fixture bytes ARE canonicalize(&parsed)'s
    // output, byte-for-byte — no re-pretty-printing, no key reordering.
    let recanonicalized = canonicalize(&parsed).expect("ServerPackage must canonicalize");
    assert_eq!(
        recanonicalized, fixture_bytes,
        "server_team_fs_v1.json must be stored in canonical byte form"
    );

    let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_server(
        &parsed,
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        None,
        &layout,
    )
    .unwrap();
    let unpacked = unpack_server(&layout).unwrap();

    assert_eq!(
        unpacked.package, parsed,
        "ServerPackage must round-trip pack/unpack losslessly"
    );
    assert_eq!(
        unpacked.binary,
        UnpackedBinary::Embedded(bootstrap),
        "bootstrap bytes must round-trip pack/unpack losslessly"
    );
}

#[test]
fn workflow_manifest_fixture_round_trips_and_matches_canonical_bytes() {
    let fixture_bytes = read_fixture("workflow_claims_triage_v1.json");
    let parsed: WorkflowManifest =
        serde_json::from_slice(&fixture_bytes).expect("fixture must parse as WorkflowManifest");

    let recanonicalized = canonicalize(&parsed).expect("WorkflowManifest must canonicalize");
    assert_eq!(
        recanonicalized, fixture_bytes,
        "workflow_claims_triage_v1.json must be stored in canonical byte form"
    );

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_workflow(&parsed, &layout).unwrap();
    let unpacked = unpack_workflow(&layout).unwrap();

    assert_eq!(
        unpacked, parsed,
        "WorkflowManifest must round-trip pack/unpack losslessly"
    );
    assert!(
        unpacked.validate_all_pinned().is_ok(),
        "the fixture manifest must be fully pinned"
    );
}

// ---------------------------------------------------------------------
// Programmatically-built types: AgentPackage, TeamPackage
// ---------------------------------------------------------------------

fn sample_agent_package() -> AgentPackage {
    AgentPackage {
        name: "claims-triage-agent".to_string(),
        version: semver::Version::parse("1.2.0").unwrap(),
        instructions: "You triage incoming insurance claims and route them to specialists."
            .to_string(),
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
    }
}

#[test]
fn agent_package_round_trips_losslessly() {
    let package = sample_agent_package();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_agent(&package, &layout).unwrap();
    let unpacked = unpack_agent(&layout).unwrap();

    assert_eq!(
        unpacked, package,
        "AgentPackage must round-trip pack/unpack losslessly"
    );
}

fn sample_team_package() -> TeamPackage {
    let entry_point = ComponentRef::Range {
        name: "claims-triage-agent".to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Agent,
    };
    let human_role = HumanRole {
        role: "approver".to_string(),
        description: "Approves high-value claims".to_string(),
        responsibilities: vec!["review".to_string(), "approve".to_string()],
        channel_hints: vec!["email".to_string()],
    };
    TeamPackage {
        name: "claims-triage-team".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        entry_point: entry_point.clone(),
        members: vec![TeamMember {
            agent: entry_point,
            role: TeamRole::EntryPoint,
        }],
        human_roles: vec![human_role.clone()],
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 200_000,
            max_team_wall_clock_seconds: 600,
            poll_interval_ms: 2000,
        },
        built_in_servers: vec![ComponentRef::Range {
            name: "team-fs".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Server,
        }],
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![human_role.to_config_slot()],
    }
}

#[test]
fn team_package_round_trips_losslessly() {
    let package = sample_team_package();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_team(&package, None, &layout).unwrap();
    let unpacked = unpack_team(&layout).unwrap();

    assert_eq!(
        unpacked.package, package,
        "TeamPackage must round-trip pack/unpack losslessly"
    );
    assert_eq!(
        unpacked.attestation, None,
        "an unattested team must round-trip with no attestation — absence is the layer's \
         absence, never a decoding default (D-14)"
    );
}

/// The unattested team layout must be structurally what it always was: adding
/// the OPTIONAL attestation layer to the team path must not have added a layer
/// (or an absence marker) to packages that carry no attestation.
#[test]
fn an_unattested_team_manifest_carries_exactly_one_layer() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_team(&sample_team_package(), None, &layout).unwrap();

    assert_eq!(
        layer_media_types(&layout),
        vec![MT_TEAM_CONFIG.to_string()],
        "an unattested team packs the team-config layer and nothing else"
    );
}

// ---------------------------------------------------------------------
// Attestation carriage on the TEAM path — SC2's second half
//
// The team carrier reuses the server path's mechanism verbatim: the same
// kind-neutral `MT_ATTESTATION` constant, the same annotation vocabulary, the
// same subject verdict re-derived rather than read. These tests assert that
// from the outside, so a future kind dispatch would break them.
// ---------------------------------------------------------------------

/// The same team with every reference PINNED.
///
/// Attested team fixtures must be fully pinned: an attestation over a team
/// holding a `ComponentRef::Range` is refused at pack time (D-09), so a
/// range-bearing fixture would fail at the pack step and mask what these tests
/// are actually asserting.
fn fully_pinned_team_package() -> TeamPackage {
    let mut package = sample_team_package();
    package.entry_point = pinned("claims-triage-agent", ComponentType::Agent);
    package.members = vec![TeamMember {
        agent: pinned("claims-triage-agent", ComponentType::Agent),
        role: TeamRole::EntryPoint,
    }];
    package.built_in_servers = vec![pinned("team-fs", ComponentType::Server)];
    package.finalizer_agents = vec![pinned("formatter-agent", ComponentType::Agent)];
    package
}

fn pinned(name: &str, component_type: ComponentType) -> ComponentRef {
    ComponentRef::Pinned(PinnedRef {
        name: name.to_string(),
        component_type,
        version: semver::Version::parse("1.0.0").unwrap(),
        digest: ManifestDigest::from_bytes(name.as_bytes()),
        resolved_from: None,
    })
}

/// Pack a fully pinned team twice — once unattested to learn the subject, once
/// attested with it. Returns `(the attested layout, the unattested digest the
/// attestation names, the attested digest)`.
fn pack_the_same_team_with_and_without_an_attestation(
    dir: &std::path::Path,
) -> (OciLayout, ManifestDigest, ManifestDigest) {
    let package = fully_pinned_team_package();

    let scratch = tempfile::tempdir().unwrap();
    let scratch_layout = OciLayout::create(scratch.path()).unwrap();
    let unattested = pack_team(&package, None, &scratch_layout).unwrap();

    let layout = OciLayout::create(dir).unwrap();
    let attested = pack_team(
        &package,
        Some(attestation_claiming(unattested.as_str())),
        &layout,
    )
    .unwrap();
    (layout, unattested, attested)
}

/// SC2's team half, end to end: an attested team round-trips its typed package
/// AND its attestation, with the payload bytes byte-identical and the three
/// annotation values carried verbatim.
#[test]
fn an_attested_team_round_trips_its_package_and_its_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, unattested, attested) =
        pack_the_same_team_with_and_without_an_attestation(dir.path());

    assert_ne!(
        attested.as_str(),
        unattested.as_str(),
        "the attestation layer is inside the bytes the digest covers, so an attested team must \
         hash to something other than the subject it names (D-01, on the team path too)"
    );

    let unpacked = unpack_team(&layout).unwrap();
    assert_eq!(unpacked.package, fully_pinned_team_package());

    let attestation = unpacked
        .attestation
        .expect("the team carried an attestation");
    assert_eq!(
        attestation.bytes, OPAQUE_ATTESTATION_BYTES,
        "the payload travels VERBATIM — bytes that are neither JSON nor UTF-8 prove nothing \
         parsed them"
    );
    assert_eq!(attestation.issuer, ATTESTATION_ISSUER);
    assert_eq!(attestation.payload_type, ATTESTATION_PAYLOAD_TYPE);
    assert_eq!(attestation.subject.claimed, unattested.as_str());
    assert!(
        attestation.subject.matches(),
        "the subject names this very team, so the verdict must be a match"
    );
}

/// The verdict is RE-DERIVED on the team path too, never read from the stored
/// claim: altering only the subject annotation yields a successful unpack whose
/// verdict is a MISMATCH, with the claim and the reality both readable.
#[test]
fn an_altered_team_subject_annotation_unpacks_successfully_and_reports_a_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, unattested, _) = pack_the_same_team_with_and_without_an_attestation(dir.path());

    let other = a_subject_naming_another_package();
    alter_the_claimed_subject(&layout, other.as_str());

    let attestation = unpack_team(&layout)
        .expect("every blob still verifies — a false CLAIM is not a corrupt package")
        .attestation
        .expect("the team still carries an attestation");

    assert_eq!(attestation.subject.claimed, other.as_str());
    assert_eq!(
        attestation.subject.unattested_digest, unattested,
        "the re-derived digest must be computed from the layout, not read from the annotation"
    );
    assert!(
        !attestation.subject.matches(),
        "a subject naming another package must report a mismatch"
    );
}

/// The kind-neutral media type is reused with NO team-specific spelling: an
/// attested TEAM layout declares the team `artifactType` while its attestation
/// layer declares the same constant a server's does.
#[test]
fn an_attested_team_declares_the_team_artifact_type_and_the_kind_neutral_attestation_type() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _, _) = pack_the_same_team_with_and_without_an_attestation(dir.path());

    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    assert_eq!(
        manifest.artifact_type().as_ref().map(ToString::to_string),
        Some(ARTIFACT_TYPE_TEAM.to_string()),
        "package kind is a function of artifactType, never of the attestation layer"
    );
    assert_eq!(
        layer_media_types(&layout),
        vec![MT_TEAM_CONFIG.to_string(), MT_ATTESTATION.to_string()],
        "an attested team carries its config layer plus the SAME kind-neutral attestation layer \
         a server carries — one constant, no kind dispatch"
    );
}

/// The extra-layer defence still holds where it still applies: admitting a
/// SECOND layer on the team path must not have loosened the strict
/// exactly-one-layer rule for agents.
#[test]
fn a_crafted_agent_layout_with_an_extra_layer_is_still_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_agent(&sample_agent_package(), &layout).unwrap();

    // Graft the attestation layer from an attested TEAM onto the agent's
    // manifest — the exact shape a crafted layout would use to smuggle a second
    // layer past a rule that merely counted "at least one".
    let team_dir = tempfile::tempdir().unwrap();
    let (team_layout, _, _) = pack_the_same_team_with_and_without_an_attestation(team_dir.path());
    let team_index = team_layout.read_index().unwrap();
    let team_manifest = team_layout
        .read_manifest(&team_index.manifests()[0])
        .unwrap();
    let extra = team_manifest
        .layers()
        .iter()
        .find(|l| l.media_type().to_string() == MT_ATTESTATION)
        .expect("the attested team carries an attestation layer")
        .clone();

    rewrite_manifest(&layout, |manifest| {
        let mut layers = manifest.layers().clone();
        layers.push(extra.clone());
        manifest.set_layers(layers);
    });

    let err = unpack_agent(&layout)
        .expect_err("an agent package must carry EXACTLY one layer, no matter what the extra is");
    assert!(matches!(err, PackageError::Layout { .. }), "got {err:?}");
}

// ---------------------------------------------------------------------
// Attestation carriage — the properties the tracer only demonstrated once
// ---------------------------------------------------------------------

const ATTESTATION_ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
const ATTESTATION_PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

/// An attestation payload that is neither valid JSON nor valid UTF-8.
///
/// Both properties are load-bearing: if anything on the carriage path parsed
/// the payload as JSON, or assumed it was a string, packing or unpacking would
/// fail. `\xff` and `\xfe` are never valid UTF-8 lead bytes, and `\x80` is a
/// continuation byte with no lead.
const OPAQUE_ATTESTATION_BYTES: &[u8] = b"\x00\x01 not json \xff\xfe\x80 \x00";

fn attestation_fixture_package() -> ServerPackage {
    serde_json::from_slice(&read_fixture("server_team_fs_v1.json"))
        .expect("fixture must parse as ServerPackage")
}

fn attestation_fixture_bootstrap() -> Vec<u8> {
    b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec()
}

/// Pack the fixture server into a fresh layout at `dir`, with or without an
/// attestation. Returns the layout and the manifest digest `pack_server`
/// produced.
fn pack_fixture_server(
    dir: &std::path::Path,
    attestation: Option<AttestationFile<'_>>,
) -> (OciLayout, ManifestDigest) {
    let layout = OciLayout::create(dir).unwrap();
    let digest = pack_server(
        &attestation_fixture_package(),
        BinaryMode::Embedded(&attestation_fixture_bootstrap()),
        None,
        None,
        attestation,
        &layout,
    )
    .unwrap();
    (layout, digest)
}

/// Build the `AttestationFile` used throughout these tests, claiming `subject`.
fn attestation_claiming(subject: &str) -> AttestationFile<'_> {
    AttestationFile {
        bytes: OPAQUE_ATTESTATION_BYTES,
        subject,
        issuer: ATTESTATION_ISSUER,
        payload_type: ATTESTATION_PAYLOAD_TYPE,
    }
}

/// Pack the fixture twice: once unattested, once carrying an attestation whose
/// subject is the unattested digest. Returns `(unattested_digest,
/// attested_digest, attested_layout)`.
///
/// The two `TempDir`s are returned so the caller keeps them alive — dropping a
/// `TempDir` deletes the layout out from under the test.
fn pack_the_same_package_with_and_without_an_attestation(
) -> (ManifestDigest, ManifestDigest, OciLayout, tempfile::TempDir) {
    let unattested_dir = tempfile::tempdir().unwrap();
    let (_unattested_layout, unattested_digest) = pack_fixture_server(unattested_dir.path(), None);

    let attested_dir = tempfile::tempdir().unwrap();
    let (attested_layout, attested_digest) = pack_fixture_server(
        attested_dir.path(),
        Some(attestation_claiming(unattested_digest.as_str())),
    );
    (
        unattested_digest,
        attested_digest,
        attested_layout,
        attested_dir,
    )
}

/// Pack the fixture server carrying an attestation, into a fresh layout at
/// `dir`. Returns the layout and the UNATTESTED digest the attestation names.
///
/// The subject cannot be an arbitrary placeholder: `pack_server`'s Gate B
/// refuses any subject that does not name this very package, so the unattested
/// digest has to be computed first by packing without the attestation.
fn pack_attested_fixture_server(dir: &std::path::Path) -> (OciLayout, ManifestDigest) {
    let scratch = tempfile::tempdir().unwrap();
    let (_scratch_layout, unattested_digest) = pack_fixture_server(scratch.path(), None);
    let (layout, _attested_digest) =
        pack_fixture_server(dir, Some(attestation_claiming(unattested_digest.as_str())));
    (layout, unattested_digest)
}

/// Everything about an attestation that is a fact of CARRIAGE — what bytes
/// travelled, what they CLAIM, who issued them and in what format.
///
/// Deliberately excludes `subject.unattested_digest`, which is not a fact about
/// the attestation at all: it is re-derived from the MANIFEST, so it changes
/// whenever the manifest does. The two position/kind-independence tests below
/// mutate the manifest on purpose, so comparing the whole `UnpackedAttestation`
/// there would assert that a mutated manifest hashes to the unmutated manifest's
/// digest — which is false, and whose falseness is exactly the tamper-evidence
/// the subject verdict provides. Those tests compare carriage; the verdict is
/// asserted separately, where a mutation is not in flight.
fn carried_facts(attestation: &UnpackedAttestation) -> (Vec<u8>, String, String, String) {
    (
        attestation.bytes.clone(),
        attestation.subject.claimed.clone(),
        attestation.issuer.clone(),
        attestation.payload_type.clone(),
    )
}

/// Read the single layer descriptor whose media type is `media_type`, or
/// `None` if the manifest carries no such layer.
fn layer_annotation(layout: &OciLayout, media_type: &str, key: &str) -> Option<String> {
    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    manifest
        .layers()
        .iter()
        .find(|l| l.media_type().to_string() == media_type)
        .and_then(|l| l.annotations().as_ref().and_then(|a| a.get(key)).cloned())
}

fn layer_media_types(layout: &OciLayout) -> Vec<String> {
    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    manifest
        .layers()
        .iter()
        .map(|l| l.media_type().to_string())
        .collect()
}

/// Rewrite `layout`'s single manifest after applying `mutate` to it.
///
/// An in-place edit is not implementable: `write_manifest` derives both the
/// blob path and the descriptor digest from the bytes, while `unpack_server`
/// digest-verifies the index descriptor BEFORE parsing. So the index
/// descriptor's hand-set annotations are carried across by hand
/// (`finalize_pack` applies them AFTER the digest is computed, so they cannot
/// be recomputed), the mutated manifest is serialized with the SAME
/// canonicalizer `finalize_pack` uses, written as a NEW content-addressed
/// blob, and the index's single descriptor is REPLACED — never appended, which
/// would trip the "expected exactly one manifest" guard and look like a code
/// bug rather than a broken test.
///
/// Mirrors `tests/config_server.rs`'s `rewrite_manifest_layers` and
/// `unpack.rs`'s duplicate-layer test, generalized over the mutation.
fn rewrite_manifest(layout: &OciLayout, mutate: impl FnOnce(&mut oci_spec::image::ImageManifest)) {
    let mut index = layout.read_index().unwrap();
    let old_descriptor = index.manifests()[0].clone();
    let annotations = old_descriptor.annotations().clone();

    let mut manifest = layout.read_manifest(&old_descriptor).unwrap();
    mutate(&mut manifest);

    let manifest_bytes = canonicalize(&manifest).unwrap();
    let mut new_descriptor = layout.write_manifest(&manifest_bytes).unwrap();
    new_descriptor.set_annotations(annotations);
    index.set_manifests(vec![new_descriptor]);
    layout.write_index(&index).unwrap();
}

/// D-01's accepted consequence, pinned so a later reader who finds two digests
/// surprising meets the reasoning at the assertion rather than hunting for it.
///
/// An attestation names the digest of the UNATTESTED package as its subject.
/// The attestation layer, and its annotations, live INSIDE the manifest whose
/// canonical bytes are hashed — so attaching one necessarily changes the
/// package's own digest. An attested package's digest can therefore NEVER
/// equal the subject it names.
///
/// Both "fixes" for that surprise are regressions, and neither may be applied
/// later: comparing the subject against the ATTESTED digest would make the
/// check pass on a mis-attached attestation, and excluding the attestation
/// layer from the canonical digest would weaken `digest::verify` into
/// verifying everything except the one layer an attacker would most want to
/// swap.
#[test]
fn packing_with_and_without_an_attestation_yields_two_distinct_digests() {
    let (unattested_digest, attested_digest, _layout, _dir) =
        pack_the_same_package_with_and_without_an_attestation();

    assert_ne!(
        unattested_digest, attested_digest,
        "attaching an attestation must change the manifest digest — if it did not, the \
         attestation layer would be outside the bytes the digest covers, and a swapped \
         attestation would be invisible to `digest::verify`"
    );
}

/// The other half of the two-digest fact: the subject annotation read back off
/// the packed manifest equals the digest the UNATTESTED pack returned, and NOT
/// the digest of the package carrying it.
#[test]
fn the_attestation_subject_annotation_names_the_unattested_digest() {
    let (unattested_digest, attested_digest, layout, _dir) =
        pack_the_same_package_with_and_without_an_attestation();

    let subject = layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_SUBJECT)
        .expect("an attested package's attestation layer must carry a subject annotation");

    assert_eq!(
        subject,
        unattested_digest.as_str(),
        "the subject must name the UNATTESTED package this attestation is about"
    );
    assert_ne!(
        subject,
        attested_digest.as_str(),
        "the subject can never equal the carrying package's own digest (D-01)"
    );
}

/// D-14 for the attestation layer: absence is the layer's absence, observed
/// through the public read path.
#[test]
fn an_unattested_package_round_trips_with_no_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), None);

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.attestation, None,
        "`attestation: None` in must be `attestation: None` out — never a decoding default"
    );
}

/// D-14 again, this time observed on disk: there is no sentinel layer, no
/// empty attestation layer and no absence marker in the manifest.
#[test]
fn an_unattested_manifest_carries_no_attestation_layer_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), None);

    let media_types = layer_media_types(&layout);
    assert!(
        !media_types.iter().any(|mt| mt == MT_ATTESTATION),
        "absence of an attestation is the absence of the layer — never a marker; layers were: \
         {media_types:?}"
    );
}

/// Opacity: `pmcp-package` never deserializes, parses or sniffs the payload,
/// so bytes that no parser would accept survive the round trip untouched.
#[test]
fn attestation_bytes_that_are_neither_json_nor_utf8_round_trip_byte_identically() {
    // Establish that the payload really is un-parseable, so a passing round
    // trip is evidence about the carriage path rather than about the fixture.
    //
    // The UTF-8 check goes through an owned `Vec` deliberately: called on the
    // constant directly, rustc's `invalid_from_utf8` lint recognises the
    // literal and warns (which `-D warnings` turns into a build failure). The
    // indirection keeps the assertion runtime-evaluated without weakening it.
    let payload: Vec<u8> = OPAQUE_ATTESTATION_BYTES.to_vec();
    assert!(
        std::str::from_utf8(&payload).is_err(),
        "the opacity fixture must contain invalid UTF-8, or it proves nothing about parsing"
    );
    assert!(
        serde_json::from_slice::<serde_json::Value>(OPAQUE_ATTESTATION_BYTES).is_err(),
        "the opacity fixture must not be valid JSON"
    );

    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_attested_fixture_server(dir.path());

    let attestation = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");
    assert_eq!(
        attestation.bytes,
        OPAQUE_ATTESTATION_BYTES.to_vec(),
        "the payload must come back byte-identical, in full"
    );
}

/// Position independence — asserting the property `pack.rs`'s own comment
/// claims, rather than restating it in prose: the deterministic push order
/// that writes the attestation LAST is not a read-order contract. Layers are
/// located by media type, so re-ordering the manifest cannot change what is
/// read.
#[test]
fn re_ordering_the_manifest_layers_does_not_change_the_attestation_read() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_attested_fixture_server(dir.path());

    let media_types = layer_media_types(&layout);
    assert_eq!(
        media_types.last().map(String::as_str),
        Some(MT_ATTESTATION),
        "pack writes the attestation layer last — the fact this test then breaks"
    );

    let baseline = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");
    rewrite_manifest(&layout, |manifest| {
        let mut layers = manifest.layers().clone();
        layers.reverse();
        manifest.set_layers(layers);
    });

    let media_types = layer_media_types(&layout);
    assert_eq!(
        media_types.first().map(String::as_str),
        Some(MT_ATTESTATION),
        "the rewrite must actually have moved the attestation layer, or the test is a no-op"
    );

    let after = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the attestation must still be located after the layers were re-ordered");
    assert_eq!(
        carried_facts(&after),
        carried_facts(&baseline),
        "the attestation is located by media type, so its position carries no meaning"
    );

    // The re-derived digest is NOT compared above, and this is why: layer order
    // is inside the bytes the manifest digest covers, so a re-ordered manifest
    // is a genuinely different package. The verdict noticing that is the
    // tamper-evidence working, not a position dependency in the read path —
    // which the carriage comparison above has just shown is absent.
    assert_ne!(
        after.subject.unattested_digest, baseline.subject.unattested_digest,
        "re-ordering the layers changes the manifest, so it must change the digest re-derived \
         from it"
    );
}

/// Kind independence — the cross-kind guard the kind-neutral media type
/// requires, and the reason plan 122-07 can reuse `MT_ATTESTATION` unchanged
/// for team packages.
///
/// Package kind and attestation location are INDEPENDENT axes: kind is read
/// from the manifest's `artifactType` plus the typed package layer, while the
/// attestation is located purely by its own media type. Altering ONLY the
/// `artifactType` therefore changes nothing about how the attestation is found
/// or read. If this ever fails, the shared constant has become a kind signal
/// by accident and the team carrier cannot reuse it.
#[test]
fn changing_only_the_artifact_type_does_not_change_how_the_attestation_is_located() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_attested_fixture_server(dir.path());

    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    assert_eq!(
        manifest.artifact_type().as_ref().map(ToString::to_string),
        Some(ARTIFACT_TYPE_SERVER.to_string()),
        "an attested SERVER layout declares the server artifactType ..."
    );
    assert!(
        layer_media_types(&layout)
            .iter()
            .any(|mt| mt == MT_ATTESTATION),
        "... while its attestation layer declares the KIND-NEUTRAL media type"
    );

    let baseline: UnpackedAttestation = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");

    rewrite_manifest(&layout, |manifest| {
        manifest.set_artifact_type(Some(oci_spec::image::MediaType::Other(
            ARTIFACT_TYPE_TEAM.to_string(),
        )));
    });

    let after = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the attestation must still be located after the artifactType changed");
    assert_eq!(
        carried_facts(&after),
        carried_facts(&baseline),
        "kind detection and attestation location are independent axes — nothing may infer a \
         package's kind from the attestation layer, and nothing may use the kind to find it"
    );
    assert_eq!(after.issuer, ATTESTATION_ISSUER);
    assert_eq!(after.payload_type, ATTESTATION_PAYLOAD_TYPE);
}

/// Both non-subject annotations survive the round trip, so a later reader is
/// not left to assume the subject is the only one that is actually carried.
#[test]
fn the_issuer_and_payload_type_annotations_round_trip_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_attested_fixture_server(dir.path());

    assert_eq!(
        layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_ISSUER).as_deref(),
        Some(ATTESTATION_ISSUER)
    );
    assert_eq!(
        layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_PAYLOAD_TYPE).as_deref(),
        Some(ATTESTATION_PAYLOAD_TYPE)
    );
}

// ---------------------------------------------------------------------
// The unpack-side subject verdict (D-02's second end, D-03's soft verdict)
//
// `pack_server` refuses a mismatched subject, but `unpack_server` must NOT
// assume that gate ran: a `.pmcp` layout arrives from the platform, not from
// this repo. It re-derives the unattested digest independently and reports the
// comparison as DATA.
//
// The distinction these tests jointly pin, deliberately NOT harmonized:
// integrity failure means the BYTES are corrupt (fail closed, `DigestMismatch`);
// subject mismatch means the bytes are fine and the CLAIM is wrong (`Ok`, with
// a verdict).
// ---------------------------------------------------------------------

/// Overwrite the attestation layer's subject annotation with `subject`,
/// rewriting the manifest so the layout stays internally consistent.
///
/// This is the tamper an attacker performs: the payload bytes stay valid (so
/// `digest::verify` is satisfied) while the CLAIM is swapped for one naming a
/// package that was never attested.
fn alter_the_claimed_subject(layout: &OciLayout, subject: &str) {
    rewrite_manifest(layout, |manifest| {
        let mut layers = manifest.layers().clone();
        for layer in &mut layers {
            if layer.media_type().to_string() == MT_ATTESTATION {
                let mut annotations = layer.annotations().clone().unwrap_or_default();
                annotations.insert(
                    ANNOTATION_ATTESTATION_SUBJECT.to_string(),
                    subject.to_string(),
                );
                layer.set_annotations(Some(annotations));
            }
        }
        manifest.set_layers(layers);
    });
}

/// A subject that names some other package — well-formed, so the only thing
/// wrong with it is that it is false.
fn a_subject_naming_another_package() -> ManifestDigest {
    ManifestDigest::from_bytes(b"an entirely different package")
}

/// The matching case: the verdict says so, and the digest it re-derived equals
/// the claim.
#[test]
fn an_attested_package_whose_subject_matches_reports_a_matching_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, unattested_digest) = pack_attested_fixture_server(dir.path());

    let attestation = unpack_server(&layout)
        .expect("an attested package must unpack")
        .attestation
        .expect("the package carried an attestation");

    assert!(
        attestation.subject.matches(),
        "a subject naming this very package must read as a match"
    );
    assert_eq!(attestation.subject.claimed, unattested_digest.as_str());
    assert_eq!(attestation.subject.unattested_digest, unattested_digest);
}

/// D-03, the whole decision in one test: a mis-attached or tampered
/// attestation unpacks SUCCESSFULLY and reports its mismatch as data. It must
/// NOT be an `Err` — the diagnostic case is exactly "show me the claim and the
/// reality side by side", and an error destroys the value that carries them.
#[test]
fn an_altered_subject_annotation_unpacks_successfully_and_reports_a_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, unattested_digest) = pack_attested_fixture_server(dir.path());
    let other = a_subject_naming_another_package();
    alter_the_claimed_subject(&layout, other.as_str());

    let unpacked =
        unpack_server(&layout).expect("a subject mismatch is DATA, never an unpack error (D-03)");
    let attestation = unpacked
        .attestation
        .expect("the tampered package still carries its attestation");

    assert!(
        !attestation.subject.matches(),
        "the altered subject names a different package, so the verdict must be a mismatch"
    );
    // The three facts, independently readable — this is the diagnostic.
    assert_eq!(attestation.subject.claimed, other.as_str());
    assert_eq!(attestation.subject.unattested_digest, unattested_digest);
    assert_ne!(
        attestation.subject.claimed,
        attestation.subject.unattested_digest.as_str()
    );
    assert_eq!(attestation.issuer, ATTESTATION_ISSUER);
}

/// The other side of the distinction, unchanged: corrupt BYTES still fail
/// closed. Softening `digest::verify` into a verdict alongside the subject
/// check would turn a tamper detector into a report, and this test is what
/// stops that.
#[test]
fn flipping_the_attestation_payload_bytes_still_fails_closed_with_a_digest_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_attested_fixture_server(dir.path());

    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    let hex = manifest
        .layers()
        .iter()
        .find(|l| l.media_type().to_string() == MT_ATTESTATION)
        .expect("the attested layout must carry an attestation layer")
        .digest()
        .digest()
        .to_string();
    let blob_path = dir.path().join("blobs").join("sha256").join(hex);
    let mut bytes = std::fs::read(&blob_path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(&blob_path, bytes).unwrap();

    let err = unpack_server(&layout)
        .expect_err("corrupt attestation BYTES are an integrity failure, not a soft verdict");
    assert!(
        matches!(err, PackageError::DigestMismatch { .. }),
        "expected DigestMismatch, got {err:?}"
    );
}

/// Independence (D-02: "neither end assumes the other ran"). The digest the
/// unpack side re-derives is the digest the PACK side returned for the
/// unattested package — computed from the layout on disk, with no stored claim
/// consulted.
#[test]
fn the_re_derived_unattested_digest_equals_the_digest_the_unattested_pack_returned() {
    let unattested_dir = tempfile::tempdir().unwrap();
    let (_unattested_layout, packed_unattested_digest) =
        pack_fixture_server(unattested_dir.path(), None);

    let attested_dir = tempfile::tempdir().unwrap();
    let (attested_layout, _) = pack_fixture_server(
        attested_dir.path(),
        Some(attestation_claiming(packed_unattested_digest.as_str())),
    );

    // Alter the claim so the re-derivation cannot be quietly reading it.
    alter_the_claimed_subject(
        &attested_layout,
        a_subject_naming_another_package().as_str(),
    );

    let attestation = unpack_server(&attested_layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");
    assert_eq!(
        attestation.subject.unattested_digest, packed_unattested_digest,
        "the unpack side must re-derive the digest from the manifest, not read the annotation"
    );
}

/// An unattested package produces no attestation and therefore NO verdict at
/// all — the comparison is not run, and no default verdict is invented.
#[test]
fn an_unattested_package_carries_no_verdict_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), None);

    assert!(
        unpack_server(&layout).unwrap().attestation.is_none(),
        "with no attestation there is no claim, so there is nothing to render a verdict about"
    );
}
