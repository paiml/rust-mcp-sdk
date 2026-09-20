//! All three attestation carriage states, end to end, through the PRODUCTION
//! `pack_server` / `unpack_server` seam.
//!
//! Run it:
//!
//! ```text
//! cargo run --manifest-path crates/pmcp-package/Cargo.toml --example attestation_carriage
//! ```
//!
//! `make pmcp-package-gate` RUNS this example (it does not merely compile it),
//! so the `assert_eq!` below is a gate assertion, not decoration.
//!
//! # What it demonstrates
//!
//! 1. **Unattested** — a package with no attestation layer at all.
//! 2. **Attested, matching subject** — the offline verification path succeeding.
//! 3. **Attested, mismatched subject** — a claim that is simply false, reported
//!    as DATA on a successful unpack rather than as an error.
//!
//! Every layout it creates lives in a `tempfile::tempdir()` and is removed when
//! that handle drops, so running this twice leaves nothing in the working tree.
//! `tempfile` is a DEV-dependency of this crate, and Cargo makes dev-dependencies
//! available to examples — confirmed by building this target rather than assumed,
//! so no `std::env::temp_dir()` fallback was needed.
//!
//! # What this example does NOT do
//!
//! It never parses the attestation payload, holds no keys and checks no
//! signature. See the closing section it prints.

use pmcp_package::digest::{canonicalize, ManifestDigest};
use pmcp_package::oci::media_types::{ANNOTATION_ATTESTATION_SUBJECT, MT_ATTESTATION};
use pmcp_package::oci::{
    pack_server, unpack_server, AttestationFile, BinaryMode, ConfigFile, OciLayout,
};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, CedarPolicySet, DeployDescriptor, ObservabilitySection,
    ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
use std::collections::BTreeMap;
use std::error::Error;

/// The author's config, carried into the package verbatim.
const CONFIG_TOML: &[u8] = b"name = \"london-tube\"\nupstream = \"https://api.tfl.gov.uk\"\n";

/// A deliberately NON-JSON attestation payload.
///
/// Opacity is the point: this crate never parses the payload, so a payload it
/// COULD not parse is the honest demonstration. `\xff` and `\xfe` are never
/// valid UTF-8 lead bytes either.
const ATTESTATION_PAYLOAD: &[u8] = b"\x00\x01 pmcp.run attestation envelope \xff\xfe\x80 \x00";

const ISSUER: &str = "https://pmcp.run/attestations";
const PAYLOAD_TYPE: &str = "application/vnd.pmcp-run.build-provenance.v1+cbor";

fn london_tube_deploy() -> DeployDescriptor {
    DeployDescriptor {
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
        environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
        secrets: BTreeMap::new(),
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
            exclude: vec!["**/*.tmp".to_string()],
        }),
        iam: None,
        gcp: None,
        layout: None,
    }
}

/// A realistic Shape A pure-config server: it NAMES a runtime binary rather
/// than carrying one, and its entire identity is its config.
fn london_tube_package() -> Result<ServerPackage, Box<dyn Error>> {
    Ok(ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::parse("1.0.0")?,
        digest: None,
        deploy: london_tube_deploy(),
        policies: CedarPolicySet(vec![]),
        tools: vec![ToolMetadata {
            name: "get_status".to_string(),
            description: "Current status of every tube line".to_string(),
            annotations: Some(serde_json::json!({ "read_only_hint": true })),
        }],
        config_slots: vec![],
    })
}

fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
        media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
    }
}

/// Pack the london-tube package into a fresh layout at `dir`, with or without
/// an attestation. THE production entry point — this example never assembles a
/// manifest by hand (except for the deliberate scenario-3 alteration below).
fn pack(
    dir: &std::path::Path,
    attestation: Option<AttestationFile<'_>>,
) -> Result<(OciLayout, ManifestDigest), Box<dyn Error>> {
    let package = london_tube_package()?;
    let layout = OciLayout::create(dir)?;
    let digest = pack_server(
        &package,
        referenced_binary(),
        Some(ConfigFile {
            file_name: "london-tube.toml",
            bytes: CONFIG_TOML,
        }),
        None,
        attestation,
        &layout,
    )?;
    Ok((layout, digest))
}

fn heading(title: &str) {
    println!("\n────────────────────────────────────────────────────────────");
    println!("  {title}");
    println!("────────────────────────────────────────────────────────────");
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("pmcp-package — attestation carriage, all three states");

    // -----------------------------------------------------------------
    // 1. Unattested
    // -----------------------------------------------------------------
    heading("1. UNATTESTED — the package carries no attestation layer");

    let unattested_dir = tempfile::tempdir()?;
    let (unattested_layout, unattested_digest) = pack(unattested_dir.path(), None)?;
    let unattested = unpack_server(&unattested_layout)?;

    println!("manifest digest      : {}", unattested_digest.as_str());
    match &unattested.attestation {
        Some(_) => println!("attestation          : PRESENT (unexpected)"),
        None => {
            println!("attestation          : none — absence is the LAYER's absence, with no marker")
        },
    }
    println!(
        "\nKeep this digest. It is the SUBJECT the next scenario's attestation must name:\n  \
         an attestation is a claim about the UNATTESTED package."
    );

    // -----------------------------------------------------------------
    // 2. Attested, matching subject
    // -----------------------------------------------------------------
    heading("2. ATTESTED, MATCHING SUBJECT — the offline check succeeding");

    println!(
        "payload              : {} bytes that are deliberately NOT valid JSON,\n\
         \x20                      because this crate never parses them",
        ATTESTATION_PAYLOAD.len()
    );

    let attested_dir = tempfile::tempdir()?;
    let (attested_layout, attested_digest) = pack(
        attested_dir.path(),
        Some(AttestationFile {
            bytes: ATTESTATION_PAYLOAD,
            subject: unattested_digest.as_str(),
            issuer: ISSUER,
            payload_type: PAYLOAD_TYPE,
        }),
    )?;

    println!("unattested digest    : {}", unattested_digest.as_str());
    println!("attested digest      : {}", attested_digest.as_str());
    println!(
        "\nThose two values DIFFER, necessarily. The attestation names the unattested digest\n\
         as its subject, and the attestation layer lives inside the manifest that is hashed —\n\
         so attaching it changes the package's own digest. Two digests exist, deliberately:\n\
         excluding the attestation layer from the hash would make them equal, at the cost of\n\
         leaving the one layer an attacker would most want to swap outside what verify() covers."
    );

    let attested = unpack_server(&attested_layout)?;
    let attestation = attested
        .attestation
        .ok_or("scenario 2 packed an attestation, so unpack must return one")?;

    println!("\nissuer               : {}", attestation.issuer);
    println!("payload type         : {}", attestation.payload_type);
    println!("claimed subject      : {}", attestation.subject.claimed);
    println!(
        "re-derived unattested: {}",
        attestation.subject.unattested_digest.as_str()
    );
    println!(
        "verdict              : {}",
        if attestation.subject.matches() {
            "MATCH — this attestation names THIS package"
        } else {
            "MISMATCH (unexpected here)"
        }
    );

    // Asserted, not merely printed: an example that only prints can print a
    // wrong answer for as long as nobody reads it closely.
    assert_eq!(
        attestation.bytes, ATTESTATION_PAYLOAD,
        "the attestation payload must survive pack/unpack byte-identically"
    );
    assert!(
        attestation.subject.matches(),
        "scenario 2's subject names this very package, so the verdict must be a match"
    );
    println!("\nassert_eq! on the recovered payload bytes: PASSED (byte-identical)");

    // -----------------------------------------------------------------
    // 3. Attested, mismatched subject
    // -----------------------------------------------------------------
    heading("3. ATTESTED, MISMATCHED SUBJECT — the bytes are fine, the claim is not");

    let someone_elses_digest = ManifestDigest::from_bytes(b"a-completely-different-package");
    claim_a_different_subject(&attested_layout, someone_elses_digest.as_str())?;
    println!(
        "The packed layout's subject annotation was rewritten to name another package.\n\
         This is the only hand-assembled manifest edit in this example, and it stands in for\n\
         a layout that arrived from somewhere untrustworthy."
    );

    let tampered = unpack_server(&attested_layout)?;
    println!("\nunpack_server returned : Ok — this is D-03's whole point");
    let tampered_attestation = tampered
        .attestation
        .ok_or("the tampered layout still carries an attestation layer")?;

    println!("issuer                 : {}", tampered_attestation.issuer);
    println!(
        "claimed subject        : {}",
        tampered_attestation.subject.claimed
    );
    println!(
        "actual (re-derived)    : {}",
        tampered_attestation.subject.unattested_digest.as_str()
    );
    println!(
        "verdict                : {}",
        if tampered_attestation.subject.matches() {
            "MATCH (unexpected here)"
        } else {
            "MISMATCH — the attestation names a package this is not"
        }
    );
    assert!(
        !tampered_attestation.subject.matches(),
        "the rewritten subject names another package, so the verdict must be a mismatch"
    );
    assert_eq!(
        tampered_attestation.bytes, ATTESTATION_PAYLOAD,
        "the payload bytes are untouched — this is not an integrity failure"
    );

    println!(
        "\nEvery byte verified, and the claim written over them is false. That is why this is\n\
         DATA on a successful unpack and not an error: a digest mismatch means the bytes are\n\
         CORRUPT, a subject mismatch means the bytes are FINE and the claim is WRONG. The two\n\
         are deliberately different and must not be harmonized."
    );

    // -----------------------------------------------------------------
    // The boundary
    // -----------------------------------------------------------------
    heading("What the SDK did NOT do");
    println!(
        "It never parsed the attestation payload — the bytes above are the issuing platform's\n\
         format, not this crate's. It holds no keys and added no crypto dependency. It verified\n\
         NO signature and confirmed NO issuer identity.\n\n\
         Offline verification here is the subject-digest comparison and nothing more. Verifying\n\
         that {ISSUER} really issued this attestation is a REMOTE call\n\
         this crate deliberately does not make."
    );

    println!("\nAll three scenarios completed; every temporary layout is removed on exit.");
    Ok(())
}

/// Rewrite the packed layout's attestation-layer subject annotation to name
/// `subject`, then re-write the manifest and index.
///
/// The ONLY hand-assembled manifest work in this example, and it exists to
/// FORGE a wrong claim — there is no production API for that, by design.
fn claim_a_different_subject(layout: &OciLayout, subject: &str) -> Result<(), Box<dyn Error>> {
    let mut index = layout.read_index()?;
    let old_descriptor = index.manifests()[0].clone();
    let index_annotations = old_descriptor.annotations().clone();

    let mut manifest = layout.read_manifest(&old_descriptor)?;
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

    let manifest_bytes = canonicalize(&manifest)?;
    let mut new_descriptor = layout.write_manifest(&manifest_bytes)?;
    new_descriptor.set_annotations(index_annotations);
    index.set_manifests(vec![new_descriptor]);
    layout.write_index(&index)?;
    Ok(())
}
