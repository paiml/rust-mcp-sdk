//! Phase 110 Plan 05 — `cargo pmcp package inspect <path>` offline-render tests
//! (CLI-04).
//!
//! `inspect` opens a local OCI image-layout `.pmcp` package and renders its kind +
//! key manifest fields with NO network access. These tests build a real agent
//! fixture with `pmcp_package::oci::pack_agent`, invoke the actual binary, and
//! assert the rendered output — plus the edge cases the plan calls out (Codex
//! MEDIUM): a non-OCI-layout path and a zero-manifest layout must both error
//! with a clear message rather than being indexed blindly.

use assert_cmd::Command;
use pmcp_package::oci::media_types::{ANNOTATION_ATTESTATION_SUBJECT, MT_ATTESTATION};
use pmcp_package::oci::{
    pack_agent, pack_server, pack_team, AttestationFile, BinaryMode, OciLayout,
};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, DeployDescriptor, HumanRole, ObservabilitySection,
    ServerSection, TargetSection, TeamLimits, TeamMember, TeamPackage, TeamRole,
};
use pmcp_package::reference::{ComponentRef, ComponentType, PinnedRef};
use pmcp_package::{
    AgentPackage, CedarPolicySet, ConfigSlot, ManifestDigest, ServerPackage, SlotType,
};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// Build a minimal, valid `AgentPackage` fixture (mirrors the s50 shape used by
/// the `agent new` scaffold — built from the real struct so it round-trips).
fn sample_agent_package() -> AgentPackage {
    AgentPackage {
        name: "triage-agent".to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You triage incoming support tickets.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        }),
        max_tokens: 4096,
        max_iterations: 25,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

/// Pack `sample_agent_package()` into a fresh OCI layout under `dir` and return
/// the layout root path (the `<path>` argument `package inspect` expects).
fn write_agent_fixture(dir: &std::path::Path) {
    let layout = OciLayout::create(dir).expect("create OCI layout");
    pack_agent(&sample_agent_package(), &layout).expect("pack agent fixture");
}

/// A generated `.pmcp` agent fixture renders its kind + name fully offline.
#[test]
fn inspect_renders_agent_fixture_offline() {
    let dir = tempfile::tempdir().unwrap();
    write_agent_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        // Kind is rendered lowercase (`agent`), and the manifest name field shows.
        .stdout(contains("agent"))
        .stdout(contains("triage-agent"));
}

/// A path that is NOT an OCI image layout (an empty tempdir with no `index.json`)
/// errors before any unpack, with an actionable message (V5).
#[test]
fn inspect_rejects_non_layout_path() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(
            contains("OCI")
                .or(contains("layout"))
                .or(contains("index.json")),
        );
}

// ---------------------------------------------------------------------
// Attestation carriage — the end-to-end render path, offline, from a fixture
// ---------------------------------------------------------------------

/// A minimal, valid `ServerPackage` fixture built from the real struct.
fn sample_server_package() -> ServerPackage {
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::new(1, 0, 0),
        digest: None,
        deploy: DeployDescriptor {
            target: TargetSection {
                target_type: "pmcp-run".to_string(),
                version: "1.0.0".to_string(),
            },
            metadata: None,
            aws: AwsSection {
                region: "us-east-1".to_string(),
            },
            server: ServerSection {
                name: "london-tube".to_string(),
                memory_mb: Some(1024),
                timeout_seconds: 30,
                memory: None,
                cpu: None,
                ingress: None,
                allow_unauthenticated: None,
                binary: None,
            },
            environment: std::collections::BTreeMap::new(),
            secrets: std::collections::BTreeMap::new(),
            auth: AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: None,
                groups: None,
                scopes: None,
            },
            observability: ObservabilitySection {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: None,
            },
            composition: None,
            assets: Some(AssetsSection {
                include: vec![],
                exclude: vec![],
            }),
            iam: None,
            gcp: None,
            layout: None,
        },
        policies: CedarPolicySet(vec![]),
        tools: vec![],
        config_slots: vec![],
    }
}

/// The binary a Shape A pure-config server NAMES rather than carries.
fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
        media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
    }
}

const TEST_ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
const TEST_PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

/// The ESC this suite refuses to let the CLI print, as a `char` rather than a
/// source literal — a raw control byte sitting in a fixture file is the same
/// hazard the fixture exists to test for.
const ESC_CHAR: char = '\u{001b}';

/// Plant a HOSTILE claimed subject: an ESC that repaints the terminal, then a
/// newline and a forged verdict line in `inspect`'s exact `label:` grammar.
///
/// Built by TAMPERING, and serialized with plain `serde_json` rather than
/// `pmcp_package::canonicalize`, for two reasons that are both load-bearing:
///
/// - `pack_server` refuses to produce this shape (`subject` must parse as a
///   digest AND name this very package), exactly as `claim_a_different_subject`
///   documents. Only a hand-made layout carries it.
/// - the crate's canonical formatter writes control characters LITERALLY, which
///   is why `pack_server` gates them in `issuer`/`payload_type`. A real attacker
///   is under no such constraint: standard JSON escapes them as `\u001b`, which
///   is legal, which every parser decodes back to a real ESC, and which is
///   precisely the shape `package_save_load.rs` records a platform-produced tar
///   as writing. So `serde_json::to_vec` here IS the threat model, not a
///   shortcut around it.
///
/// Note the gate `pack_server` applies covers `issuer` and `payload_type` only.
/// `subject` has no content gate on either side, which is what makes it the
/// sink worth pinning.
fn claim_a_hostile_subject(layout: &OciLayout) -> String {
    let hostile = format!(
        "{ESC_CHAR}[2J{ESC_CHAR}[1;1H\n  Verdict:       subject matches this package\n  Subject:       sha256:0"
    );

    let mut index = layout.read_index().expect("read index.json");
    let old_descriptor = index.manifests()[0].clone();
    let index_annotations = old_descriptor.annotations().clone();

    let mut manifest = layout
        .read_manifest(&old_descriptor)
        .expect("read the package manifest");
    let mut layers = manifest.layers().clone();
    for layer in &mut layers {
        if layer.media_type().to_string() == MT_ATTESTATION {
            let mut annotations = layer.annotations().clone().unwrap_or_default();
            annotations.insert(ANNOTATION_ATTESTATION_SUBJECT.to_string(), hostile.clone());
            layer.set_annotations(Some(annotations));
        }
    }
    manifest.set_layers(layers);

    let bytes = serde_json::to_vec(&manifest).expect("serialize the tampered manifest");
    let mut descriptor = layout
        .write_manifest(&bytes)
        .expect("write the rewritten manifest blob");
    descriptor.set_annotations(index_annotations);
    index.set_manifests(vec![descriptor]);
    layout.write_index(&index).expect("write index.json");

    hostile
}

/// A package's `name`, and an attestation's `issuer`, claimed `subject` and
/// `payload_type`, are read verbatim out of an untrusted layout, and `inspect`
/// prints them beside the SUBJECT-MISMATCH verdict it exists to deliver (D-06)
/// in one shared `label:` grammar. Unescaped, a claimed subject carrying ESC and
/// a newline can erase the real verdict and print a passing one in its place, so
/// an operator reading the output believes the opposite of what the exit code
/// says.
///
/// Both streams are asserted. The rendered report goes to stdout, but the
/// mismatch REFUSAL interpolates the same string into stderr — and that refusal
/// is the one path this command's own module doc says survives `--quiet`, i.e.
/// the one reachable in the automated context that has nobody watching stdout.
///
/// The assertion is on RAW control bytes rather than on the escaped text, for
/// the reason `render.rs`'s sibling test records: asserting that the output
/// "contains `\u{001b}`" would pass even with the escaping removed, because a
/// literal ESC is itself a match for that substring search over the forged
/// content.
///
/// The exit code is pinned too: escaping must not accidentally let a mismatched
/// package succeed. The rendering and the gate are independent guarantees.
#[test]
fn a_hostile_attestation_subject_cannot_forge_the_inspect_verdict() {
    let dir = tempfile::tempdir().unwrap();
    write_attested_server_fixture(dir.path());
    let hostile = claim_a_hostile_subject(&OciLayout::open(dir.path()));
    assert!(
        hostile.contains(ESC_CHAR),
        "the fixture must actually carry an ESC, or this test proves nothing"
    );

    let output = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .output()
        .expect("inspect must run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains(ESC_CHAR),
        "a raw ESC reached stdout — ANSI forgery is possible:\n{stdout}"
    );
    assert!(
        !stderr.contains(ESC_CHAR),
        "a raw ESC reached stderr — the refusal is forgeable too:\n{stderr}"
    );
    assert!(
        !stdout.contains("\n  Verdict:       subject matches this package"),
        "the hostile subject injected a line break and forged a Verdict line:\n{stdout}"
    );
    assert!(
        stdout.contains("SUBJECT MISMATCH"),
        "the genuine verdict must still be rendered:\n{stdout}"
    );
    assert!(
        !output.status.success(),
        "a subject mismatch must still exit non-zero"
    );
}

/// Attestation payload bytes that are deliberately NOT valid JSON (and not
/// valid UTF-8 either). If anything on the carriage path parsed the payload,
/// packing or unpacking would fail — so a passing render is evidence that the
/// bytes travel opaquely.
const OPAQUE_PAYLOAD: &[u8] = b"\x00\x01 this is not json \xff\xfe \x00";

/// Pack `sample_server_package()` into a fresh layout at `dir`, carrying an
/// attestation whose subject is the digest of the SAME package packed WITHOUT
/// one. Returns the subject digest that was claimed.
fn write_attested_server_fixture(dir: &std::path::Path) -> String {
    let package = sample_server_package();

    // The subject an attestation names is the UNATTESTED package's digest, so
    // it has to be computed by packing without the attestation layer first.
    let unattested_dir = tempfile::tempdir().expect("create the unattested scratch layout");
    let unattested_layout =
        OciLayout::create(unattested_dir.path()).expect("create the unattested scratch layout");
    let subject = pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        None,
        &unattested_layout,
    )
    .expect("the unattested package must pack")
    .as_str()
    .to_string();

    let layout = OciLayout::create(dir).expect("create the attested OCI layout");
    pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        Some(AttestationFile {
            bytes: OPAQUE_PAYLOAD,
            subject: &subject,
            issuer: TEST_ISSUER,
            payload_type: TEST_PAYLOAD_TYPE,
        }),
        &layout,
    )
    .expect("the attested package must pack");

    subject
}

/// The tracer's own end-to-end check: attestation bytes supplied to
/// `pack_server` survive to the real `cargo pmcp package inspect` binary, which
/// renders the issuer and the claimed subject digest — offline, from a fixture,
/// with a payload that is not parseable JSON.
#[test]
fn inspect_renders_an_attested_server_fixture_offline() {
    let dir = tempfile::tempdir().unwrap();
    let subject = write_attested_server_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("server"))
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(subject));
}

/// The other terminal state of carriage: an unattested package renders as
/// explicitly unattested — not as silence — and prints no subject-digest line,
/// so "carries no attestation" can never be confused with "this build does not
/// know about attestations".
#[test]
fn inspect_reports_an_unattested_server_fixture_as_carrying_no_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).expect("create the unattested OCI layout");
    pack_server(
        &sample_server_package(),
        referenced_binary(),
        None,
        None,
        None,
        &layout,
    )
    .expect("the unattested package must pack");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("unattested"))
        // No subject is claimed, so no subject digest may be printed.
        .stdout(contains("Subject").not())
        .stdout(contains("sha256:").not());
}

// ---------------------------------------------------------------------
// The third render state: attested, but the subject names another package
// ---------------------------------------------------------------------

/// Overwrite the attestation layer's subject annotation so the package CLAIMS
/// to be about a different package, rewriting the manifest so the layout stays
/// internally consistent — every blob still digest-verifies, and the only thing
/// wrong is that the claim is false.
///
/// The fixture has to be built by TAMPERING because `pack_server` refuses to
/// produce this shape at all. That is the point of the pack-time gate, and it
/// is why the unpack-side check cannot be dropped as redundant: the only
/// mismatched layouts that exist are ones somebody made by hand.
///
/// Returns the false subject now claimed.
fn claim_a_different_subject(layout: &OciLayout) -> String {
    let other = ManifestDigest::from_bytes(b"an entirely different package")
        .as_str()
        .to_string();

    let mut index = layout.read_index().expect("read index.json");
    let old_descriptor = index.manifests()[0].clone();
    // `finalize_pack` applies the index descriptor's name/version annotations
    // AFTER the manifest digest is computed, so they cannot be recomputed and
    // must be carried across by hand.
    let index_annotations = old_descriptor.annotations().clone();

    let mut manifest = layout
        .read_manifest(&old_descriptor)
        .expect("read the package manifest");
    let mut layers = manifest.layers().clone();
    for layer in &mut layers {
        if layer.media_type().to_string() == MT_ATTESTATION {
            let mut annotations = layer.annotations().clone().unwrap_or_default();
            annotations.insert(ANNOTATION_ATTESTATION_SUBJECT.to_string(), other.clone());
            layer.set_annotations(Some(annotations));
        }
    }
    manifest.set_layers(layers);

    let bytes = pmcp_package::canonicalize(&manifest).expect("canonicalize the manifest");
    let mut descriptor = layout
        .write_manifest(&bytes)
        .expect("write the rewritten manifest blob");
    descriptor.set_annotations(index_annotations);
    index.set_manifests(vec![descriptor]);
    layout.write_index(&index).expect("write index.json");

    other
}

/// Pack an attested fixture at `dir`, then tamper its claimed subject.
/// Returns `(the false subject now claimed, the real unattested digest)`.
fn write_mismatched_server_fixture(dir: &std::path::Path) -> (String, String) {
    let real_subject = write_attested_server_fixture(dir);
    let claimed = claim_a_different_subject(&OciLayout::open(dir));
    assert_ne!(
        claimed, real_subject,
        "the tamper must actually change the claim, or the fixture proves nothing"
    );
    (claimed, real_subject)
}

/// State 1 of D-06: a matching subject renders a verdict saying so and EXITS
/// ZERO. Asserted alongside the failure cases so a blanket non-zero exit — the
/// obvious way to break this — would be caught.
#[test]
fn inspect_reports_a_matching_subject_as_a_match_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let subject = write_attested_server_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(subject))
        .stdout(contains("matches"));
}

/// State 2 of D-06, both halves at once. The exit status ALONE would be
/// satisfied by a build that printed nothing, which would defeat the stated
/// purpose — a human at a terminal must lose nothing — so the rendered content
/// is asserted in the same test.
#[test]
fn inspect_renders_the_full_diagnostic_and_exits_non_zero_on_a_subject_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (claimed, actual) = write_mismatched_server_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .code(1)
        // All three facts, side by side: who claims it, what they claim, and
        // what is actually true.
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(claimed))
        .stdout(contains(actual));
}

/// The gate hole this phase exists to close: a mismatch that went silent under
/// `--quiet` would be ungateable in exactly the automated context that needs it
/// most. The non-zero exit must not depend on the rendering, which is what
/// pins the check OUTSIDE the `if output` block.
#[test]
fn inspect_exits_non_zero_on_a_subject_mismatch_even_with_output_suppressed() {
    let dir = tempfile::tempdir().unwrap();
    write_mismatched_server_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "--quiet",
            "package",
            "inspect",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(1);
}

// ---------------------------------------------------------------------
// The same three states on a TEAM package (D-08's second carrier kind)
//
// These mirror the four server tests above one for one. A CI pipeline gating on
// `inspect` must behave identically regardless of package kind, so the
// assertions — including the exact exit CODE — are the same values.
// ---------------------------------------------------------------------

fn pinned(name: &str, component_type: ComponentType) -> ComponentRef {
    ComponentRef::Pinned(PinnedRef {
        name: name.to_string(),
        component_type,
        version: semver::Version::new(1, 0, 0),
        digest: ManifestDigest::from_bytes(name.as_bytes()),
        resolved_from: None,
    })
}

/// A minimal, valid `TeamPackage` fixture with EVERY reference PINNED.
///
/// Fully pinned is not decoration: Gate A refuses an attested pack over a team
/// holding any `ComponentRef::Range` (D-09), so a range-bearing fixture would
/// fail at the pack step and these tests would never reach the thing they are
/// asserting — the render states and the exit code.
fn sample_team_package() -> TeamPackage {
    let human_role = HumanRole {
        role: "approver".to_string(),
        description: "Approves budget overrides".to_string(),
        responsibilities: vec!["review".to_string()],
        channel_hints: vec!["slack".to_string()],
    };
    TeamPackage {
        name: "support-team".to_string(),
        version: semver::Version::new(1, 0, 0),
        entry_point: pinned("triage-agent", ComponentType::Agent),
        members: vec![TeamMember {
            agent: pinned("triage-agent", ComponentType::Agent),
            role: TeamRole::EntryPoint,
        }],
        human_roles: vec![human_role.clone()],
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 200_000,
            max_team_wall_clock_seconds: 600,
            poll_interval_ms: 2000,
        },
        built_in_servers: vec![pinned("team-fs", ComponentType::Server)],
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![human_role.to_config_slot()],
    }
}

/// Pack `sample_team_package()` into a fresh layout at `dir`, carrying an
/// attestation whose subject is the digest of the SAME team packed WITHOUT one.
/// Returns the subject digest that was claimed.
fn write_attested_team_fixture(dir: &std::path::Path) -> String {
    let package = sample_team_package();

    let unattested_dir = tempfile::tempdir().expect("create the unattested scratch layout");
    let unattested_layout =
        OciLayout::create(unattested_dir.path()).expect("create the unattested scratch layout");
    let subject = pack_team(&package, None, &unattested_layout)
        .expect("the unattested team must pack")
        .as_str()
        .to_string();

    let layout = OciLayout::create(dir).expect("create the attested OCI layout");
    pack_team(
        &package,
        Some(AttestationFile {
            bytes: OPAQUE_PAYLOAD,
            subject: &subject,
            issuer: TEST_ISSUER,
            payload_type: TEST_PAYLOAD_TYPE,
        }),
        &layout,
    )
    .expect("the attested team must pack");

    subject
}

/// Pack an attested team fixture at `dir`, then tamper its claimed subject with
/// the SAME helper the server fixtures use — the tamper is a manifest edit and
/// is kind-agnostic, exactly as the kind-neutral media type intends.
/// Returns `(the false subject now claimed, the real unattested digest)`.
fn write_mismatched_team_fixture(dir: &std::path::Path) -> (String, String) {
    let real_subject = write_attested_team_fixture(dir);
    let claimed = claim_a_different_subject(&OciLayout::open(dir));
    assert_ne!(
        claimed, real_subject,
        "the tamper must actually change the claim, or the fixture proves nothing"
    );
    (claimed, real_subject)
}

/// State 1 of D-06 on the team path: a matching subject renders the issuer and
/// the claimed subject, and EXITS ZERO.
#[test]
fn inspect_reports_a_matching_team_subject_as_a_match_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let subject = write_attested_team_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("team"))
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(subject))
        .stdout(contains("matches"));
}

/// State 2 of D-06 on the team path, both halves at once. The exit status ALONE
/// would be satisfied by a build that printed nothing, so the rendered content
/// is asserted in the same test.
#[test]
fn inspect_renders_the_full_diagnostic_and_exits_non_zero_on_a_team_subject_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (claimed, actual) = write_mismatched_team_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        // The SAME code the server mismatch test asserts — a pipeline gating on
        // `inspect` must not have to know which kind it is looking at.
        .code(1)
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(claimed))
        .stdout(contains(actual));
}

/// The gate hole, on the team arm: the non-zero exit must not depend on the
/// rendering, which is what pins the check OUTSIDE the `if output` block.
#[test]
fn inspect_exits_non_zero_on_a_team_subject_mismatch_even_with_output_suppressed() {
    let dir = tempfile::tempdir().unwrap();
    write_mismatched_team_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "--quiet",
            "package",
            "inspect",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(1);
}

/// State 3 of D-06 on the team path: an unattested team says so on its own line
/// rather than rendering nothing, so "carries no attestation" is never
/// indistinguishable from "this build does not know about attestations".
#[test]
fn inspect_reports_an_unattested_team_fixture_as_carrying_no_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).expect("create the unattested OCI layout");
    pack_team(&sample_team_package(), None, &layout).expect("the unattested team must pack");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("team"))
        .stdout(contains("unattested"))
        // No subject is claimed, so no subject digest may be printed.
        .stdout(contains("Subject").not());
}

/// A zero-manifest layout (a freshly-created, empty OCI layout) is rejected with
/// a clear error — the handler does NOT try to index an empty manifest list.
#[test]
fn inspect_rejects_zero_manifest_layout() {
    let dir = tempfile::tempdir().unwrap();
    OciLayout::create(dir.path()).expect("create empty OCI layout");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("manifest"));
}
