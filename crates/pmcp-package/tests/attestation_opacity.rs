//! Attestation OPACITY as a generated property, plus the untrusted-annotation
//! robustness sweep.
//!
//! `tests/roundtrip.rs` demonstrates opacity ONCE, with a single hand-picked
//! payload that is neither valid JSON nor valid UTF-8. That is evidence, not a
//! property. This binary asserts the same claim over generated input: for an
//! arbitrary byte vector — including invalid UTF-8, embedded NUL bytes,
//! JSON-shaped-but-not-JSON bytes and the empty vector — what `pack_server`
//! stores is exactly what `unpack_server` returns.
//!
//! The other untrusted surface on this boundary is the three annotation
//! values. They arrive from a layout that may have been authored by anyone, so
//! they are generated adversarially here (path separators, `..` segments,
//! control characters, non-ASCII) and asserted to come back as inert DATA.
//!
//! # Why proptest and not `cargo fuzz`
//!
//! Recorded so the absence is a visible decision rather than an oversight:
//! `make test-fuzz` pipes every fuzz target through `timeout 30s ... || echo`,
//! so a crashing target exits that gate green; `cargo fuzz` requires nightly
//! while CI is `dtolnay/rust-toolchain@stable`; and `fuzz/` is the ROOT crate's
//! fuzz workspace, which cannot reach this workspace-EXCLUDED crate without
//! manifest wiring nothing else in the repo has. This suite covers the same
//! input space and — through `make pmcp-package-gate`'s nonzero-test-count
//! assertion — actually runs on stable, where a failure is a red gate.

use pmcp_package::digest::ManifestDigest;
use pmcp_package::oci::{
    pack_server, unpack_server, AttestationFile, OciLayout, UnpackedAttestation,
};
use pmcp_package::package::{CedarPolicySet, ServerPackage};
use pmcp_package::PackageError;
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

mod common;

/// Upper bound on a generated attestation payload.
///
/// 4 KiB, chosen rather than left implicit: every generated case performs TWO
/// full packs (one unattested, to learn the subject the pack-time gate demands,
/// and one attested) plus an unpack, and each of those canonicalizes and
/// SHA-256s the whole manifest. A megabyte-scale bound would buy no additional
/// coverage of the opacity path — nothing on it is length-sensitive — while
/// pushing the suite past the phase's 120-second feedback budget at
/// `PROPTEST_CASES=1000`.
const MAX_PAYLOAD_BYTES: usize = 4096;

/// A minimal but REAL `ServerPackage`, sharing `tests/common/mod.rs`'s deploy
/// descriptor and referenced binary so this suite packs the same shape the rest
/// of the crate's integration tests do.
fn attestation_property_package() -> ServerPackage {
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        digest: None,
        deploy: common::minimal_deploy_descriptor(),
        policies: CedarPolicySet(vec![]),
        tools: vec![],
        config_slots: vec![],
    }
}

/// Pack the property package into a fresh layout at `dir`, WITHOUT asserting
/// success — the annotation property needs to observe a refusal as an outcome
/// rather than as a panic.
fn try_pack_at(
    dir: &Path,
    attestation: Option<AttestationFile<'_>>,
) -> (OciLayout, Result<ManifestDigest, PackageError>) {
    let layout = OciLayout::create(dir).expect("the layout directory must be creatable");
    let result = pack_server(
        &attestation_property_package(),
        common::referenced_binary(),
        None,
        None,
        attestation,
        &layout,
    );
    (layout, result)
}

/// Pack the property package into a fresh layout at `dir`, with or without an
/// attestation, returning the layout and the manifest digest.
fn pack_at(dir: &Path, attestation: Option<AttestationFile<'_>>) -> (OciLayout, ManifestDigest) {
    let (layout, result) = try_pack_at(dir, attestation);
    (layout, result.expect("the property package must pack"))
}

/// `value` in Unicode Normalization Form C.
///
/// Annotation values do NOT round-trip byte-verbatim, and no change to this
/// crate can make them: the manifest is written as Canonical JSON, and the
/// Canonical JSON specification requires strings to be NFC-normalized.
/// `olpc-cjson`'s `write_string_fragment` applies `str::nfc` to every fragment
/// it writes, so what reaches the blob is already normalized before the digest
/// is taken over it. `unpack_server` reads back exactly what was written.
///
/// Concretely, and this is the case proptest found (seed
/// `tests/attestation_opacity.proptest-regressions`): U+F900 CJK COMPATIBILITY
/// IDEOGRAPH-F900 has a *singleton* canonical decomposition to U+8C48, so NFC
/// maps one to the other. The two render identically, which is why the failure
/// reads as `left: "<CJK>", right: "<CJK>"` with no visible difference.
///
/// # Why the property is normalized rather than the input
///
/// Restricting the generator to NFC input would make the test agree with the
/// implementation by construction and stop covering the transformation at all.
/// Making `pack_server` REFUSE non-NFC annotations was the other candidate and
/// is worse: it is exactly the "quietly widen into refusing legitimate
/// non-ASCII issuers" failure this property's own doc names below, since an
/// issuer legitimately written in NFD would start being rejected.
///
/// NFC normalization does not weaken what this property exists to prove. The
/// claim is that annotation values are inert DATA — they reach no filesystem
/// API and are never interpreted as paths. A normalization applied uniformly
/// by the serializer, before any byte is written, touches neither half.
fn nfc(value: &str) -> String {
    value.nfc().collect()
}

/// The first C0 control character in `value`, mirroring the range
/// `pack_server`'s annotation gate refuses.
///
/// Deliberately a SEPARATE implementation from the crate's private
/// `first_control_character` rather than a re-export: a test that called the
/// production predicate would agree with it by construction, including when
/// both are wrong.
fn carries_a_control_character(value: &str) -> bool {
    value.chars().any(|character| (character as u32) < 0x20)
}

/// The digest this package WOULD have with no attestation attached.
///
/// Every attested case needs it: plan 122-03's pack-time gate refuses a subject
/// that does not name this very package, so a placeholder subject cannot
/// exercise the success path at all. Computing it by packing unattested into a
/// scratch layout is the honest way to obtain it, and it re-asserts that gate on
/// every generated case for free.
fn unattested_digest_of_the_property_package() -> (tempfile::TempDir, ManifestDigest) {
    let scratch = tempfile::tempdir().expect("a scratch temp dir must be creatable");
    let (_layout, digest) = pack_at(scratch.path(), None);
    (scratch, digest)
}

/// Pack `payload` as an attestation into a layout under `layout_dir`, unpack it,
/// and hand back what came out along with both digests.
fn round_trip_attested(
    layout_dir: &Path,
    payload: &[u8],
    issuer: &str,
    payload_type: &str,
) -> (UnpackedAttestation, ManifestDigest, ManifestDigest) {
    let (_scratch, unattested_digest) = unattested_digest_of_the_property_package();
    let (layout, attested_digest) = pack_at(
        layout_dir,
        Some(AttestationFile {
            bytes: payload,
            subject: unattested_digest.as_str(),
            issuer,
            payload_type,
        }),
    );
    let unpacked = unpack_server(&layout).expect("an attested layout must unpack");
    let attestation = unpacked
        .attestation
        .expect("a package packed WITH an attestation must unpack with one");
    (attestation, unattested_digest, attested_digest)
}

// ---------------------------------------------------------------------
// The sandbox-parent snapshot
// ---------------------------------------------------------------------

/// Every entry beneath `root`, as a sorted `(path relative to root, byte
/// length)` list. Directories are recorded with length 0 so an empty directory
/// appearing or vanishing is visible too.
///
/// Hand-rolled `std::fs::read_dir` recursion on purpose: `walkdir` would be a
/// NEW dependency in a crate whose `make no-crypto-check` gate allowlists its
/// entire resolved graph, and fifteen lines of recursion is cheaper than that
/// conversation.
fn snapshot_tree(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut entries = Vec::new();
    collect_tree(root, root, &mut entries);
    entries.sort();
    entries
}

fn collect_tree(root: &Path, dir: &Path, entries: &mut Vec<(PathBuf, u64)>) {
    let read = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("failed to read {dir:?}: {e}"));
    for entry in read {
        let entry = entry.expect("a directory entry must be readable");
        let path = entry.path();
        let metadata = entry.metadata().expect("entry metadata must be readable");
        let relative = path
            .strip_prefix(root)
            .expect("every walked path is under the root")
            .to_path_buf();
        if metadata.is_dir() {
            entries.push((relative, 0));
            collect_tree(root, &path, entries);
        } else {
            entries.push((relative, metadata.len()));
        }
    }
}

// ---------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------

/// One fragment of an adversarial annotation value.
///
/// The literals are deliberate and named so a reader can confirm the
/// adversarial classes are present rather than take the strategy's word for it:
/// POSIX and Windows path separators, parent-directory segments, an absolute
/// path, a home-relative path, a shell variable, C0 control characters, DEL,
/// an embedded NUL, and non-ASCII text (Latin-1, CJK, emoji). `any::<char>()`
/// and the filler regex widen the space beyond the hand-picked set.
fn adversarial_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("../".to_string()),
        Just("..\\".to_string()),
        Just("..".to_string()),
        Just("/".to_string()),
        Just("\\".to_string()),
        Just("/etc/passwd".to_string()),
        Just("~/.ssh/authorized_keys".to_string()),
        Just("$HOME".to_string()),
        Just("${PWD}".to_string()),
        Just("\u{0}".to_string()),
        Just("\u{1}\u{7}\u{1b}".to_string()),
        Just("\u{7f}".to_string()),
        Just("café-北京-\u{1f600}".to_string()),
        Just("blobs".to_string()),
        Just("oci-layout".to_string()),
        "[a-zA-Z0-9._-]{1,8}",
        any::<char>().prop_map(|c| c.to_string()),
    ]
}

/// An adversarial annotation value: one to six fragments concatenated, so
/// `../` chains and mixed control/non-ASCII sequences both occur.
fn adversarial_annotation() -> impl Strategy<Value = String> {
    prop::collection::vec(adversarial_fragment(), 1..7).prop_map(|parts| parts.concat())
}

/// An arbitrary attestation payload, bounded by [`MAX_PAYLOAD_BYTES`].
///
/// `any::<u8>()` over an unrestricted length range covers invalid UTF-8,
/// embedded NULs, JSON-shaped-but-not-JSON bytes and — because the range starts
/// at 0 — the empty payload.
fn arbitrary_payload() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=MAX_PAYLOAD_BYTES)
}

// ---------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------

proptest! {
    /// SC2's opacity claim, as a property rather than a fixture: whatever bytes
    /// go in come back out identical, for any byte vector.
    ///
    /// The comparison is on the FULL `Vec<u8>` — not a length, not a hash, not
    /// a prefix — because a canonicalizer or a re-encoder applied to the payload
    /// can preserve all three of those while changing the bytes.
    #[test]
    fn arbitrary_attestation_bytes_survive_a_pack_unpack_round_trip_byte_identically(
        payload in arbitrary_payload(),
    ) {
        let dir = tempfile::tempdir().expect("a temp dir must be creatable");
        let (attestation, _unattested, _attested) = round_trip_attested(
            dir.path(),
            &payload,
            "https://issuer.test.invalid/pmcp-run",
            "application/vnd.test.attestation-payload",
        );

        prop_assert_eq!(
            attestation.bytes,
            payload,
            "the attestation payload must come back byte-identical — anything else means \
             something on the carriage path interpreted, normalized or re-encoded it"
        );
    }

    /// D-01's two-digest consequence, over generated payloads: attaching an
    /// attestation ALWAYS changes the package's own manifest digest, including
    /// for the empty payload.
    ///
    /// If this ever held false, the attestation layer would be sitting outside
    /// the bytes `digest::verify` covers, and a swapped attestation would be
    /// invisible.
    #[test]
    fn an_attested_pack_always_has_a_different_digest_than_the_unattested_one(
        payload in arbitrary_payload(),
    ) {
        let dir = tempfile::tempdir().expect("a temp dir must be creatable");
        let (_attestation, unattested_digest, attested_digest) = round_trip_attested(
            dir.path(),
            &payload,
            "https://issuer.test.invalid/pmcp-run",
            "application/vnd.test.attestation-payload",
        );

        prop_assert_ne!(
            unattested_digest,
            attested_digest,
            "attaching an attestation must change the manifest digest"
        );
    }

    /// Adversarial annotation values are inert: an annotation value is either
    /// REFUSED at pack time with a named error, or it packs and comes back
    /// VERBATIM as data with nothing beneath the sandbox parent changed across
    /// the unpack. There is no third outcome — no panic, and in particular no
    /// package that packs successfully and then cannot be unpacked.
    ///
    /// # The refusal arm, and why it exists
    ///
    /// This property originally asserted only the round-trip arm. Its first run
    /// FAILED on the generated case `issuer = "\0"`: the package packed
    /// cleanly and `unpack_server` then returned a serde error, because
    /// canonical JSON writes C0 control characters literally and RFC 8259
    /// forbids them unescaped inside a JSON string. `pack_server` now refuses
    /// that input before its first write. The two arms are asserted to be
    /// EXACTLY complementary — a refusal implies a control character, and a
    /// success implies none — so the gate cannot quietly widen into refusing
    /// legitimate non-ASCII issuers, which would be a bug of its own.
    ///
    /// # Why the assertion is a sandbox-PARENT snapshot
    ///
    /// The layout is created in a SUBDIRECTORY of a temp parent, and the whole
    /// parent — not just the layout — is walked before and after. Enumerating
    /// only the layout directory can prove its own contents are expected but
    /// cannot see a path-traversal write landing OUTSIDE it, which is exactly
    /// what a `..`-bearing annotation would produce. The empty sibling space
    /// inside the parent is where such a write would land.
    ///
    /// Equality is the right assertion, not "no unexpected file": `unpack_server`
    /// is a pure reader and writes nothing at all, so any difference is a real
    /// finding.
    ///
    /// # Blind spot, stated rather than left to be discovered
    ///
    /// This detects a write anywhere beneath the sandbox parent. It does NOT
    /// detect a write to an absolute path elsewhere on the filesystem
    /// (`/tmp/pwned`, `~/.ssh/authorized_keys`). That half of the claim is
    /// carried by a reviewed data flow: the three annotation values are read by
    /// `required_annotation`, moved into `UnpackedAttestation`'s `subject`,
    /// `issuer` and `payload_type` fields, and never passed to any `std::fs` or
    /// `Path` API. The runtime snapshot and the reviewed data flow carry the
    /// claim jointly; neither carries it alone.
    #[test]
    fn adversarial_annotation_values_come_back_as_inert_data(
        issuer in adversarial_annotation(),
        payload_type in adversarial_annotation(),
    ) {
        let parent = tempfile::tempdir().expect("a sandbox parent must be creatable");
        let layout_dir = parent.path().join("layout");

        let (_scratch, unattested_digest) = unattested_digest_of_the_property_package();
        let (layout, packed) = try_pack_at(
            &layout_dir,
            Some(AttestationFile {
                bytes: b"opaque \x00\xff payload",
                subject: unattested_digest.as_str(),
                issuer: &issuer,
                payload_type: &payload_type,
            }),
        );

        let representable =
            !carries_a_control_character(&issuer) && !carries_a_control_character(&payload_type);

        match packed {
            Err(PackageError::AttestationAnnotationInvalid { .. }) => {
                prop_assert!(
                    !representable,
                    "pack_server refused an annotation value that carries no control \
                     character — the gate has widened beyond what canonical JSON cannot \
                     represent"
                );
                // Refused: there is nothing to unpack, and the round-trip arm
                // below does not apply.
                return Ok(());
            },
            Err(other) => {
                prop_assert!(
                    false,
                    "an adversarial annotation must be refused with \
                     AttestationAnnotationInvalid or accepted — got {:?}",
                    other
                );
                unreachable!()
            },
            Ok(_attested_digest) => prop_assert!(
                representable,
                "pack_server accepted an annotation carrying a control character; canonical \
                 JSON writes it literally, so this package cannot be unpacked"
            ),
        }

        let before = snapshot_tree(parent.path());
        let unpacked = unpack_server(&layout).expect("an attested layout must unpack");
        let after = snapshot_tree(parent.path());

        prop_assert_eq!(
            &before,
            &after,
            "unpack_server writes nothing, so the sandbox parent must be identical across \
             the call; a difference means an annotation value reached the filesystem"
        );

        let attestation = unpacked
            .attestation
            .expect("a package packed WITH an attestation must unpack with one");
        prop_assert_eq!(
            attestation.issuer,
            nfc(&issuer),
            "the issuer annotation must come back as data, NFC-normalized and \
             otherwise untouched"
        );
        prop_assert_eq!(
            attestation.payload_type,
            nfc(&payload_type),
            "the payload-type annotation must come back as data, NFC-normalized and \
             otherwise untouched"
        );
    }
}

// ---------------------------------------------------------------------
// The one case a generated payload cannot reach
// ---------------------------------------------------------------------

/// A payload that IS a syntactically valid JSON object still round-trips
/// byte-identically — no reordering, no re-indentation, no normalization.
///
/// This case exists because the arbitrary-bytes property cannot catch a
/// canonicalizer on its own. An accidental
/// `canonicalize(&serde_json::from_slice::<Value>(bytes)?)` in the attestation
/// arm would simply FAIL on a non-JSON payload — a loud, obvious break. On a
/// VALID JSON object it would succeed while silently sorting the keys and
/// stripping the whitespace, and the package would still pack, unpack and
/// verify. The keys below are deliberately NOT in sorted order and the spacing
/// is deliberately irregular, so a canonicalizer changes these exact bytes.
#[test]
fn a_valid_json_payload_is_carried_verbatim_and_never_canonicalized() {
    const JSON_PAYLOAD: &[u8] =
        br#"{ "zebra": 1,  "alpha": {"b": 2, "a": [3,   4]},"middle":"x" }"#;

    // Guard the guard: if this ever stopped being parseable JSON the case would
    // silently degrade into a second arbitrary-bytes test.
    let parsed: serde_json::Value =
        serde_json::from_slice(JSON_PAYLOAD).expect("the case payload must be VALID JSON");
    assert_ne!(
        serde_json::to_vec(&parsed).unwrap(),
        JSON_PAYLOAD.to_vec(),
        "the payload's byte form must differ from any re-serialization of it, or a \
         canonicalizer would be invisible to this case"
    );

    let dir = tempfile::tempdir().expect("a temp dir must be creatable");
    let (attestation, _unattested, _attested) = round_trip_attested(
        dir.path(),
        JSON_PAYLOAD,
        "https://issuer.test.invalid/pmcp-run",
        "application/json",
    );

    assert_eq!(
        attestation.bytes,
        JSON_PAYLOAD.to_vec(),
        "a valid-JSON attestation payload must be carried VERBATIM — a key reordering or a \
         whitespace strip here means the payload was parsed and re-emitted"
    );
}

/// The ESCAPING case, pinned deterministically rather than left to sampling: an
/// issuer that IS a parent-directory traversal leaves the sandbox parent
/// untouched.
///
/// The property above generates traversal-shaped values, but proptest shrinks
/// to whatever fails FIRST — during this plan's escaping-write falsifiability
/// control it shrank to `issuer = "\\"`, a write that lands INSIDE the layout
/// and would also have been caught by the weaker layout-only enumeration this
/// snapshot replaced. This test names the escaping shape explicitly, so the
/// claim "the snapshot sees a write OUTSIDE the layout" is reproducible on
/// demand instead of depending on which case the shrinker happens to reach.
#[test]
fn a_traversal_shaped_issuer_writes_nothing_beside_the_layout() {
    let parent = tempfile::tempdir().expect("a sandbox parent must be creatable");
    let layout_dir = parent.path().join("layout");
    let (_scratch, unattested_digest) = unattested_digest_of_the_property_package();
    let (layout, _digest) = pack_at(
        &layout_dir,
        Some(AttestationFile {
            bytes: b"opaque payload",
            subject: unattested_digest.as_str(),
            // Resolves to `<parent>/escaped-by-the-issuer` — OUTSIDE the layout
            // root, INSIDE the sandbox parent, which is exactly the region a
            // layout-only enumeration cannot see.
            issuer: "../escaped-by-the-issuer",
            payload_type: "../escaped-by-the-payload-type",
        }),
    );

    let before = snapshot_tree(parent.path());
    let unpacked = unpack_server(&layout).expect("an attested layout must unpack");
    let after = snapshot_tree(parent.path());

    assert_eq!(
        before, after,
        "a `../`-shaped annotation must not produce a write beside the layout"
    );
    let attestation = unpacked
        .attestation
        .expect("the attestation must be present");
    assert_eq!(attestation.issuer, "../escaped-by-the-issuer");
    assert_eq!(attestation.payload_type, "../escaped-by-the-payload-type");
}

/// The sandbox-parent snapshot must be able to SEE a write, or the robustness
/// property above would be vacuous.
///
/// The property asserts two snapshots are equal. An assertion that can only
/// ever compare identical values proves nothing, so this test writes one file
/// into the sandbox parent between the two snapshots and asserts they differ.
#[test]
fn the_sandbox_parent_snapshot_detects_a_write_beneath_the_parent() {
    let parent = tempfile::tempdir().expect("a sandbox parent must be creatable");
    let layout_dir = parent.path().join("layout");
    let (_scratch, unattested_digest) = unattested_digest_of_the_property_package();
    pack_at(
        &layout_dir,
        Some(AttestationFile {
            bytes: b"opaque payload",
            subject: unattested_digest.as_str(),
            issuer: "https://issuer.test.invalid/pmcp-run",
            payload_type: "application/vnd.test.attestation-payload",
        }),
    );

    let before = snapshot_tree(parent.path());
    // The shape an escaping `../`-derived write would have: a file landing in
    // the parent, beside the layout rather than inside it.
    std::fs::write(parent.path().join("escaped"), b"x").unwrap();
    let after = snapshot_tree(parent.path());

    assert_ne!(
        before, after,
        "the snapshot must detect a file written beside the layout — otherwise the equality \
         assertion in the annotation property is vacuous"
    );
}
