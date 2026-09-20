//! Phase 123-07 (PKGX-02): fuzz target for the untrusted artifact-tar boundary
//! behind `cargo pmcp package load` / `package pull`.
//!
//! # The boundary
//!
//! Arbitrary, adversarial byte sequences go STRAIGHT into
//! `cargo_pmcp::package_artifact::read_verified` — the one function in this
//! repository that parses a `.tar` of unknown provenance. A tar handed to
//! `package load` arrives from wherever the user got it; nothing upstream of
//! this call has vouched for a single byte of it. The input is deliberately NOT
//! pre-shaped into a valid archive: the adversarial-bytes path is the point.
//!
//! This is the direct analogue of `fuzz_package_kind`, which campaigns the
//! sibling untrusted manifest-JSON boundary.
//!
//! # The three invariants — none of them tautological
//!
//! "It returned `Ok` or `Err`" is a property a completely broken reader passes,
//! so this target asserts three properties instead, on every accepted input:
//!
//! 1. **64-lowercase-hex keys.** Every key in `VerifiedArtifact::blobs` is
//!    exactly 64 characters of `[0-9a-f]`. A reader that admitted an
//!    uppercase, truncated or non-hex blob name would produce a key that
//!    cannot address a `blobs/sha256/<hex>` file.
//! 2. **Content hashes to its own key.** Re-deriving sha256 over each blob's
//!    bytes — here, with `sha2` DIRECTLY rather than through
//!    `pmcp_package::ManifestDigest`, so the check does not share an
//!    implementation with the code under test — reproduces that blob's key. A
//!    reader that stopped re-deriving digests, or that trusted the archive's
//!    own file name, violates this.
//! 3. **Descriptor-graph closure, in both directions.** Every descriptor the
//!    artifact carries (the index's one manifest descriptor, the manifest's
//!    config descriptor, and every layer descriptor) resolves to a blob that is
//!    present; and every present blob is reachable from one of those
//!    descriptors. A reader that admitted a dangling descriptor or an orphan
//!    blob violates this.
//!
//! All three hold over EVERY accepted input, which is what makes them suitable
//! for a campaign: any input the reader accepts is automatically a test case.
//!
//! # What this campaign does NOT establish
//!
//! It does not establish the byte caps (`ArtifactLimits::per_entry` /
//! `ArtifactLimits::total`), and no SUMMARY or comment may claim that it does.
//! Arbitrary bytes are overwhelmingly unlikely to produce a well-framed
//! multi-megabyte archive entry within a bounded campaign, so a campaign never
//! approaches those caps at all. Cap falsifiability is discharged
//! DETERMINISTICALLY, in Phase 123 plan 01, by the
//! `read_verified_with_limits` / `ArtifactLimits` test pairs in
//! `cargo-pmcp/tests/package_save_load.rs` and
//! `cargo-pmcp/src/commands/package/artifact.rs`'s own module tests: the same
//! input refused under a tiny injected cap and accepted under a large one, with
//! the refusal asserted by message. See `ArtifactLimits`' rustdoc, which states
//! the same division of labour from the other side.
//!
//! Fuzzing keeps the job it is actually good at: panic and hang resistance at
//! an untrusted-bytes boundary, plus the three invariants above.
//!
//! # Threat model
//!
//! T-123-61 (parser DoS — a panic or hang in the untrusted tar reader on
//! adversarial bytes). The campaign also exercises the paths that mitigate
//! T-123-01 (path traversal via archive-supplied entry paths), T-123-02
//! (symlink / hardlink entries), T-123-03 (allocation bomb — lying header
//! sizes and unbounded in-memory hold), T-123-04 (blob substitution inside the
//! tar) and T-123-05 (duplicate archive entries shadowing a real one), since
//! every one of those refusals lives inside `read_verified`.
//!
//! # Running it
//!
//! Full campaign:
//! ```sh
//! cargo +nightly fuzz run fuzz_package_artifact
//! ```
//! Quick smoke (bounded):
//! ```sh
//! cargo +nightly fuzz run fuzz_package_artifact -- -max_total_time=60
//! ```
//!
//! Seeding the corpus with `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/conformant.tar`
//! (and a copy with one blob byte flipped) is what makes invariant 2 reachable
//! by construction rather than by luck — an invariant only arbitrary bytes can
//! reach is an invariant nobody has watched fail.

#![no_main]

use std::collections::BTreeSet;

use libfuzzer_sys::fuzz_target;
use sha2::{Digest, Sha256};

use cargo_pmcp::package_artifact::{read_verified, VerifiedArtifact};

/// Re-derive a blob's sha256 as lowercase hex, INDEPENDENTLY of the digest
/// helper the code under test uses.
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The lowercase hex body of a descriptor's digest, if it is a sha256 digest.
fn descriptor_hex(digest: &str) -> Option<&str> {
    digest.strip_prefix("sha256:")
}

/// Assert the three invariants over an artifact the reader ACCEPTED.
fn assert_invariants(artifact: &VerifiedArtifact) {
    // 1. Every key is 64 lowercase hex characters.
    for hex in artifact.blobs.keys() {
        assert_eq!(
            hex.len(),
            64,
            "accepted blob key is not 64 characters: {hex:?}"
        );
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "accepted blob key is not lowercase hex: {hex:?}"
        );
    }

    // 2. Every blob's content hashes to its own key, and the recorded size is
    //    the real one.
    for (hex, blob) in &artifact.blobs {
        assert_eq!(
            sha256_hex(&blob.bytes),
            *hex,
            "accepted blob does not hash to its own key: {hex:?}"
        );
        assert_eq!(
            blob.size,
            blob.bytes.len() as u64,
            "accepted blob's recorded size disagrees with its bytes: {hex:?}"
        );
    }

    // 3. Descriptor-graph closure, both directions.
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let manifest_digest = artifact.manifest_descriptor.digest().to_string();
    if let Some(hex) = descriptor_hex(&manifest_digest) {
        referenced.insert(hex.to_string());
    }
    let config_digest = artifact.manifest.config().digest().to_string();
    if let Some(hex) = descriptor_hex(&config_digest) {
        referenced.insert(hex.to_string());
    }
    for layer in artifact.manifest.layers() {
        let layer_digest = layer.digest().to_string();
        if let Some(hex) = descriptor_hex(&layer_digest) {
            referenced.insert(hex.to_string());
        }
    }

    for hex in &referenced {
        assert!(
            artifact.blobs.contains_key(hex),
            "dangling descriptor survived verification: sha256:{hex} is referenced but absent"
        );
    }
    for hex in artifact.blobs.keys() {
        assert!(
            referenced.contains(hex),
            "orphan blob survived verification: sha256:{hex} is present but unreferenced"
        );
    }

    // The manifest digest the reader reports must itself be the manifest blob's
    // own address — it is derived locally, never read out of the archive.
    assert_eq!(
        artifact.manifest_digest.as_str(),
        manifest_digest,
        "reported manifest digest disagrees with the index's manifest descriptor"
    );
}

fuzz_target!(|data: &[u8]| {
    // The untrusted boundary: raw bytes, no pre-shaping, no utf8 filter. Must
    // neither panic nor hang, whatever it is handed.
    if let Ok(artifact) = read_verified(data) {
        assert_invariants(&artifact);
    }
});
