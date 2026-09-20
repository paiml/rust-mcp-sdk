//! Integration test: the config-only (Shape A) server package, end to end.
//!
//! A pure-config server has no binary of its own — its entire identity is its
//! config file plus a REFERENCE to a runtime binary the target environment
//! resolves. These tests prove that whole path exists and closes: pack with
//! [`BinaryMode::Referenced`] and a [`ConfigFile`], unpack, and get the
//! author's exact bytes back under the author's original file name, with no
//! bootstrap layer anywhere in the manifest.
//!
//! Every layout is a `tempfile::tempdir()` — never a hand-rolled temp dir.
//! The `ServerPackage` fixture is built inline here rather than reaching into
//! the crate's `#[cfg(test)]` fixtures, so this file exercises the same public
//! API an external consumer has.

use proptest::prelude::*;

use pmcp_package::digest::ManifestDigest;
use pmcp_package::error::{PackageError, Result};
use pmcp_package::oci::media_types::{
    MT_SERVER_BOOTSTRAP, MT_SERVER_CONFIG, MT_SERVER_OPENAPI_SPEC,
};
use pmcp_package::oci::parse_declared_config_slots;
use pmcp_package::oci::{
    pack_server, unpack_server, BinaryMode, ConfigFile, OciLayout, OpenApiSpecFile, UnpackedBinary,
};
use pmcp_package::package::ServerPackage;
use pmcp_package::slot::{aggregate, classify, required_slots, ConfigSlot, SlotClass, SlotType};

mod common;
use common::{
    london_tube_package, openapi_server_crate_dir, pack_london_tube, parse_env_ref_grammar_table,
    referenced_binary, referenced_binary_digest, vendored_fixture, ENV_REF_GRAMMAR_TABLE,
    LONDON_TUBE_CONFIG_NAME, LONDON_TUBE_SPEC_NAME, REFERENCED_MEDIA_TYPE,
};

/// The author's `config.toml`, verbatim — the bytes a Shape A server's whole
/// identity rests on. Deliberately holds comments, blank lines and
/// non-alphabetical key order: if pack ever normalized or re-derived the
/// config from a parsed struct, this content would not survive byte-for-byte.
const CONFIG_TOML: &[u8] = br#"# london-tube MCP server
name    = "london-tube"
version = "1.0.0"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
kind = "openapi"
# a trailing comment, and irregular   spacing
base_url = "https://api.tfl.gov.uk"
# A slot-declared credential key: it holds an environment REFERENCE, never a
# resolved literal, which is exactly what `pack_server` now enforces.
api_key = "${TFL_API_KEY}"
"#;

const CONFIG_FILE_NAME: &str = "london-tube.toml";

/// A representative pure-config `ServerPackage`. Note it carries no binary
/// information at all — that is a layer, not a field (D-08).
///
/// Delegates to `common::london_tube_package`, which DERIVES `config_slots`
/// from CONFIG_TOML's own `[[config_slots]]` block — so this package agrees
/// with the config by construction rather than by a hand-written copy that
/// could drift from it (the failure mode `common/mod.rs` documents).
fn config_server_package() -> ServerPackage {
    london_tube_package(CONFIG_TOML)
}

fn config_file() -> ConfigFile<'static> {
    ConfigFile {
        file_name: CONFIG_FILE_NAME,
        bytes: CONFIG_TOML,
    }
}

// ---------------------------------------------------------------------
// PKG-01: a config-only package round-trips its config verbatim
// ---------------------------------------------------------------------

#[test]
fn config_only_package_restores_config_bytes_verbatim_under_its_original_name() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let config = unpacked
        .config
        .expect("a package packed WITH a config must unpack WITH a config");

    assert_eq!(
        config.bytes, CONFIG_TOML,
        "config bytes must be byte-identical to what the author supplied — pack must never \
         rewrite, templatize, normalize or reformat them"
    );
    assert_eq!(
        config.file_name, CONFIG_FILE_NAME,
        "the author's original file name must survive the round trip"
    );
    assert_eq!(
        unpacked.spec, None,
        "a package packed without a spec must unpack without one"
    );
}

#[test]
fn config_only_package_manifest_carries_no_bootstrap_layer() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout,
    )
    .unwrap();

    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    let media_types: Vec<String> = manifest
        .layers()
        .iter()
        .map(|l| l.media_type().to_string())
        .collect();

    assert!(
        !media_types.iter().any(|m| m == MT_SERVER_BOOTSTRAP),
        "a config-only package embeds no binary, so no bootstrap layer may exist: {media_types:?}"
    );
    assert!(
        media_types.iter().any(|m| m == MT_SERVER_CONFIG),
        "the config layer must be present: {media_types:?}"
    );
}

// ---------------------------------------------------------------------
// PKG-02: the referenced-binary arm carries a digest and no bytes
// ---------------------------------------------------------------------

#[test]
fn config_only_package_unpacks_to_referenced_binary_with_the_callers_digest() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();

    match unpacked.binary {
        UnpackedBinary::Referenced { digest, media_type } => {
            assert_eq!(
                digest,
                referenced_binary_digest(),
                "the referenced digest must be passed through verbatim"
            );
            assert_eq!(media_type, REFERENCED_MEDIA_TYPE);
        },
        UnpackedBinary::Embedded(_) => {
            panic!(
                "a package packed as Referenced must never unpack as Embedded — unpack is a \
                    local, offline operation and must not resolve a local binary (D-07)"
            )
        },
    }
}

// ---------------------------------------------------------------------
// The embedded path is unchanged in behaviour
// ---------------------------------------------------------------------

#[test]
fn an_embedded_package_still_round_trips_its_bootstrap_bytes() {
    let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    // KEYLESS slots: an embedded, config-less package is the pre-0.2 shape,
    // whose slots never name a config path. (A keyed slot without a config
    // file is refused — see the dedicated test below.)
    let mut package = config_server_package();
    package.config_slots = package
        .config_slots
        .into_iter()
        .map(|slot| ConfigSlot::new(slot.slot))
        .collect();

    pack_server(
        &package,
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();

    assert_eq!(unpacked.binary, UnpackedBinary::Embedded(bootstrap));
    assert_eq!(
        unpacked.config, None,
        "a package packed without a config must unpack without one"
    );
}

/// The inverse of "a value slot must name the config key it fills": a slot
/// naming a `config_key` while the package ships NO config file points into a
/// document that does not exist — a coverage claim nothing can validate at
/// pack time and nothing can fill at deploy time, so `pack_server` refuses it
/// before writing a single blob.
#[test]
fn a_keyed_slot_without_a_config_file_is_refused_naming_the_dangling_key() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    // config_server_package()'s slots each carry a config_key derived from
    // CONFIG_TOML's declaration block — but no ConfigFile travels with them.
    let err = pack_server(
        &config_server_package(),
        referenced_binary(),
        None,
        None,
        None,
        &layout,
    )
    .expect_err("a config_key with no config document to address must not pack");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("ships no"), "was: {reason}");
    assert!(
        layout.read_index().unwrap().manifests().is_empty(),
        "the refusal must land before anything is written"
    );
}

/// The CONFIG -> SLOT direction, at the `pack_server` boundary an author
/// actually meets.
///
/// Both older document gates start from the DECLARED slot list, so a config
/// that defers a value to the environment without declaring a slot for it
/// satisfied them trivially — and packed at exit 0 into a package that then
/// told its target environment "no config slots — nothing to fill" about a
/// server that could not start without one.
///
/// The config below is the reported shape reduced: one declared slot that is
/// correct in every way the old gates check, plus a SECOND reference nothing
/// declares. A gate that only ran one way passes this document.
#[test]
fn a_config_reference_no_slot_declares_is_refused_before_anything_is_written() {
    const UNDER_DECLARED: &[u8] = br#"
name    = "london-tube"
version = "1.0.0"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
kind = "openapi"
api_key = "${TFL_API_KEY}"
# Deferred to the environment, and declared by nothing.
base_url = "${TFL_BASE_URL}"
"#;

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let before = blob_file_names(dir.path());

    let err = pack_server(
        &london_tube_package(UNDER_DECLARED),
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: UNDER_DECLARED,
        }),
        None,
        None,
        &layout,
    )
    .expect_err("a config deferring a value nothing declares must not pack");

    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("TFL_BASE_URL"), "was: {reason}");
    assert!(
        reason.contains("[[config_slots]]"),
        "the message must say where the declaration goes: {reason}"
    );
    assert!(
        !reason.contains("backend.api_key"),
        "the correctly declared key must not be reported: {reason}"
    );
    // "Before anything is written" is asserted against the BLOBS, not only the
    // index. An empty index is the weaker half: move the precondition check
    // after the layer-write loop and the config layer lands in
    // `blobs/sha256/` while the index stays empty, so an index-only assertion
    // stays green with its own name false (measured: 7 leaked blobs). Leaked
    // config bytes are exactly what this validation exists to prevent, which
    // is why `a_rejected_pack_adds_neither_a_blob_nor_an_index_entry` asserts
    // both — the reference-bearing path needs the pair just as much.
    assert_eq!(
        blob_file_names(dir.path()),
        before,
        "the refusal must land before a single blob is written"
    );
    assert!(
        layout.read_index().unwrap().manifests().is_empty(),
        "the refusal must land before anything is recorded in the index"
    );
}

/// The same config packs once the missing slot is declared — the gate demands
/// a DECLARATION, never a particular value, and never a second copy of the
/// config.
#[test]
fn declaring_the_missing_slot_is_what_makes_the_same_config_pack() {
    const FULLY_DECLARED: &[u8] = br#"
name    = "london-tube"
version = "1.0.0"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[backend]
kind = "openapi"
api_key = "${TFL_API_KEY}"
base_url = "${TFL_BASE_URL}"
"#;

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &london_tube_package(FULLY_DECLARED),
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: FULLY_DECLARED,
        }),
        None,
        None,
        &layout,
    )
    .expect("every deferred value now has a slot");

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.package.config_slots.len(),
        2,
        "the target environment must be told about BOTH values it has to supply"
    );

    // THE NEGATIVE CONTROL, without which this test proves nothing about the
    // gate. `london_tube_package` DERIVES its `config_slots` from the config's
    // own `[[config_slots]]` block, so the assertion above only re-measures
    // that derivation plus a `Vec` round trip — it is green whether or not
    // `validate_no_undeclared_env_refs_in` runs at all. Removing exactly one
    // declaration from the SAME bytes must flip the pack to a refusal naming
    // exactly that key.
    const MISSING_THE_ENDPOINT_SLOT: &[u8] = br#"
name    = "london-tube"
version = "1.0.0"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
kind = "openapi"
api_key = "${TFL_API_KEY}"
base_url = "${TFL_BASE_URL}"
"#;
    let control_dir = tempfile::tempdir().unwrap();
    let control_layout = OciLayout::create(control_dir.path()).unwrap();
    let err = pack_server(
        &london_tube_package(MISSING_THE_ENDPOINT_SLOT),
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: MISSING_THE_ENDPOINT_SLOT,
        }),
        None,
        None,
        &control_layout,
    )
    .expect_err("dropping the endpoint declaration must un-do what made it pack");
    let (key, _) = config_slot_violation(err);
    assert_eq!(
        key, "backend.base_url",
        "the declaration is what made the difference, so removing it must name that key"
    );
}

// ---------------------------------------------------------------------
// Determinism: pack is environment-independent
// ---------------------------------------------------------------------

#[test]
fn packing_identical_config_only_inputs_into_two_layouts_yields_one_digest() {
    let package = config_server_package();

    let dir_a = tempfile::tempdir().unwrap();
    let layout_a = OciLayout::create(dir_a.path()).unwrap();
    let digest_a = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout_a,
    )
    .unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let layout_b = OciLayout::create(dir_b.path()).unwrap();
    let digest_b = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout_b,
    )
    .unwrap();

    assert_eq!(
        digest_a, digest_b,
        "pack must be deterministic and environment-independent: a repeated or concurrent pack \
         of identical inputs cannot produce a different package"
    );
}

// ---------------------------------------------------------------------
// A referenced binary must never be unpinned
// ---------------------------------------------------------------------

#[test]
fn a_binary_ref_layer_with_no_digest_is_rejected_at_unpack() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout,
    )
    .unwrap();

    // Rewrite the binary-ref layer with a wire payload whose `digest` decodes
    // to `None` — the one shape the tolerant wire type admits and the API type
    // cannot express. The target environment must never be handed an
    // instruction to run an unpinned binary.
    let index = layout.read_index().unwrap();
    let mut manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    let unpinned = serde_json::to_vec(&serde_json::json!({
        "media_type": REFERENCED_MEDIA_TYPE,
    }))
    .unwrap();
    let new_layer = layout
        .write_blob(
            oci_spec::image::MediaType::from(pmcp_package::oci::media_types::MT_SERVER_BINARY_REF),
            &unpinned,
        )
        .unwrap();
    let layers: Vec<_> = manifest
        .layers()
        .iter()
        .map(|l| {
            if l.media_type().to_string() == pmcp_package::oci::media_types::MT_SERVER_BINARY_REF {
                new_layer.clone()
            } else {
                l.clone()
            }
        })
        .collect();
    manifest.set_layers(layers);
    let manifest_bytes = pmcp_package::digest::canonicalize(&manifest).unwrap();
    let manifest_descriptor = layout.write_manifest(&manifest_bytes).unwrap();
    let new_index = oci_spec::image::ImageIndexBuilder::default()
        .schema_version(oci_spec::image::SCHEMA_VERSION)
        .manifests(vec![manifest_descriptor])
        .build()
        .unwrap();
    layout.write_index(&new_index).unwrap();

    let err = unpack_server(&layout).unwrap_err();
    assert!(
        matches!(err, pmcp_package::PackageError::Layout { .. }),
        "an unpinned binary reference must be a Layout error, got {err:?}"
    );
}

// ---------------------------------------------------------------------
// PKG-01: the OPTIONAL OpenAPI spec layer (D-14, D-15, D-16)
// ---------------------------------------------------------------------

/// The author's OpenAPI document, verbatim. Deliberately YAML with comments
/// and irregular spacing: if pack ever parsed and re-emitted the spec, this
/// content would not survive byte-for-byte.
const SPEC_YAML: &[u8] = br#"# london-tube OpenAPI contract
openapi: 3.1.0
info:
  title:   London Tube API
  version: "1.0.0"
paths: {}
"#;

const SPEC_FILE_NAME: &str = "london-tube-api.yaml";

fn spec_file() -> OpenApiSpecFile<'static> {
    OpenApiSpecFile {
        file_name: SPEC_FILE_NAME,
        bytes: SPEC_YAML,
    }
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

#[test]
fn a_packed_spec_restores_its_bytes_verbatim_under_its_original_name() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let spec = unpacked
        .spec
        .expect("a package packed WITH a spec must unpack WITH a spec");

    assert_eq!(
        spec.bytes, SPEC_YAML,
        "spec bytes must be byte-identical to what the author supplied — pack must never parse, \
         reformat or re-emit them"
    );
    assert_eq!(
        spec.file_name, SPEC_FILE_NAME,
        "the author's original spec file name must survive the round trip"
    );
}

#[test]
fn a_package_packed_without_a_spec_carries_no_spec_layer_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        None,
        &layout,
    )
    .unwrap();

    let media_types = layer_media_types(&layout);
    assert!(
        !media_types.iter().any(|m| m == MT_SERVER_OPENAPI_SPEC),
        "absence of a spec is the absence of the layer — never an absence marker: {media_types:?}"
    );

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.spec, None,
        "a curated-only server (pmcp-openapi-server's `--spec: Option<PathBuf>`) must pack and \
         unpack cleanly with no spec"
    );
}

#[test]
fn a_json_spec_round_trips_under_exactly_the_same_media_type_as_a_yaml_one() {
    let json_spec = OpenApiSpecFile {
        file_name: "api.json",
        bytes: b"{}",
    };

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(json_spec),
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let spec = unpacked.spec.expect("the JSON spec layer must be present");
    assert_eq!(spec.bytes, b"{}", "JSON spec bytes must survive verbatim");
    assert_eq!(spec.file_name, "api.json");

    let json_media_types = layer_media_types(&layout);
    assert!(
        json_media_types.iter().any(|m| m == MT_SERVER_OPENAPI_SPEC),
        "a JSON spec uses the SAME media type as a YAML one — the format is evident from the \
         file-name annotation, not from a second media type: {json_media_types:?}"
    );

    let yaml_dir = tempfile::tempdir().unwrap();
    let yaml_layout = OciLayout::create(yaml_dir.path()).unwrap();
    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        None,
        &yaml_layout,
    )
    .unwrap();
    let yaml_media_types = layer_media_types(&yaml_layout);

    assert_eq!(
        json_media_types
            .iter()
            .filter(|m| *m == MT_SERVER_OPENAPI_SPEC)
            .count(),
        yaml_media_types
            .iter()
            .filter(|m| *m == MT_SERVER_OPENAPI_SPEC)
            .count(),
        "one spec media type covers both formats"
    );
}

#[test]
fn renaming_only_the_spec_file_changes_the_manifest_digest() {
    let package = config_server_package();

    let dir_a = tempfile::tempdir().unwrap();
    let layout_a = OciLayout::create(dir_a.path()).unwrap();
    let digest_a = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        Some(OpenApiSpecFile {
            file_name: "london-tube-api.yaml",
            bytes: SPEC_YAML,
        }),
        None,
        &layout_a,
    )
    .unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let layout_b = OciLayout::create(dir_b.path()).unwrap();
    let digest_b = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        Some(OpenApiSpecFile {
            file_name: "renamed-api.yaml",
            bytes: SPEC_YAML,
        }),
        None,
        &layout_b,
    )
    .unwrap();

    assert_ne!(
        digest_a, digest_b,
        "the file-name annotation lives on a LAYER descriptor, which is inside the manifest that \
         gets hashed (D-15) — renaming the spec changes the package's identity"
    );
}

// ---------------------------------------------------------------------
// D-11: layer ORDER is not load-bearing — every read is keyed by media type
// ---------------------------------------------------------------------

/// Number of layers a fully-populated config-only package carries:
/// binary-ref, envelope, deploy-descriptor, cedar-policy-set, tool-metadata,
/// config-slots, config, spec.
const FULL_LAYER_COUNT: usize = 8;

/// Pack a fully-populated config-only package (both optional layers present,
/// so the permutation exercises every media type) into a fresh layout.
fn packed_full_package(dir: &std::path::Path) -> (OciLayout, ManifestDigest) {
    let layout = OciLayout::create(dir).unwrap();
    let digest = pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        None,
        &layout,
    )
    .unwrap();
    (layout, digest)
}

/// Rewrite `layout`'s single manifest so its `layers` array is `order`, and
/// return the new manifest digest.
///
/// "Rewrite the manifest with the permuted order" is NOT implementable as an
/// in-place edit, and getting that wrong produces a test that measures
/// nothing. `OciLayout::write_blob` derives BOTH the blob path and the
/// descriptor digest from the bytes, while `unpack_server`'s
/// `read_the_one_manifest` digest-verifies the index descriptor's declared
/// digest BEFORE it parses. So:
///
/// - overwrite the original blob path in place -> unpack dies at `verify`,
///   never reaching the media-type lookup under test;
/// - write permuted bytes without touching the index -> the index still
///   points at the ORIGINAL manifest, so unpack reads the unpermuted order
///   and the test passes vacuously.
///
/// The only correct shape is the five steps below: carry the index
/// descriptor's annotations across by hand (`finalize_pack` sets them AFTER
/// the digest is computed, so they cannot be recomputed), permute, serialize
/// with the SAME canonicalizer `finalize_pack` uses, write a NEW
/// content-addressed manifest blob, and REPLACE — never append — the index's
/// single descriptor.
fn rewrite_manifest_layers(layout: &OciLayout, order: &[usize]) -> Result<ManifestDigest> {
    // 1. The existing index descriptor, and its hand-set annotations.
    let mut index = layout.read_index()?;
    let old_descriptor =
        index
            .manifests()
            .first()
            .cloned()
            .ok_or_else(|| PackageError::Layout {
                reason: "index.json carries no manifest to rewrite".to_string(),
            })?;
    let annotations = old_descriptor.annotations().clone();

    // 2. Permute the layer vector.
    let mut manifest = layout.read_manifest(&old_descriptor)?;
    let layers = manifest.layers().clone();
    let permuted: Vec<_> = order
        .iter()
        .map(|&i| {
            layers.get(i).cloned().ok_or_else(|| PackageError::Layout {
                reason: format!("permutation index {i} is out of range"),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    manifest.set_layers(permuted);

    // 3. The SAME canonical form finalize_pack uses, so layer order is the
    //    only difference from the original bytes.
    let manifest_bytes = pmcp_package::canonicalize(&manifest)?;

    // 4. A NEW content-addressed blob, re-annotated by hand.
    let mut new_descriptor = layout.write_manifest(&manifest_bytes)?;
    new_descriptor.set_annotations(annotations);
    let digest = ManifestDigest::try_from(new_descriptor.digest())?;

    // 5. REPLACE the single index entry — a push would make
    //    read_the_one_manifest fail with "expected exactly one manifest",
    //    which looks like a code bug rather than a broken test.
    index.set_manifests(vec![new_descriptor]);
    layout.write_index(&index)?;

    Ok(digest)
}

#[test]
fn the_permutation_helper_actually_rewrites_the_content_addressed_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, baseline_digest) = packed_full_package(dir.path());

    // The identity permutation reproduces the original bytes exactly — proof
    // the helper's canonical form matches finalize_pack's.
    let identity: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    let identity_digest = rewrite_manifest_layers(&layout, &identity).unwrap();
    assert_eq!(
        identity_digest, baseline_digest,
        "the identity permutation must round-trip to the SAME digest — a different one would mean \
         the helper serializes differently from finalize_pack, so the property test would be \
         measuring the helper rather than unpack_server"
    );

    // A real permutation must produce DIFFERENT manifest bytes; if it did
    // not, the property test below would be a no-op.
    let mut reversed: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    reversed.reverse();
    let reversed_digest = rewrite_manifest_layers(&layout, &reversed).unwrap();
    assert_ne!(
        reversed_digest, baseline_digest,
        "a non-identity permutation must yield a different manifest blob — otherwise the helper \
         is a no-op and proves nothing"
    );
}

#[test]
fn a_reversed_layer_order_still_unpacks_to_the_same_server() {
    let baseline_dir = tempfile::tempdir().unwrap();
    let (baseline_layout, _) = packed_full_package(baseline_dir.path());
    let baseline = unpack_server(&baseline_layout).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = packed_full_package(dir.path());
    let mut reversed: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    reversed.reverse();
    rewrite_manifest_layers(&layout, &reversed).unwrap();

    assert_eq!(
        unpack_server(&layout).unwrap(),
        baseline,
        "unpack_server resolves every layer by media type, so reversing the manifest's layer \
         array cannot change what it yields"
    );
}

proptest! {
    /// PROPERTY (D-11, CLAUDE.md ALWAYS PROPERTY): for an ARBITRARY
    /// permutation of the manifest's `layers` array, `unpack_server` yields an
    /// EQUAL `UnpackedServer`. Layer position carries no meaning — every read
    /// is keyed by media type — so a layout that only shuffles its layers is
    /// the same package.
    ///
    /// The permutation goes through `rewrite_manifest_layers`, which performs
    /// the full content-addressed rewrite; without it the manifest's
    /// digest-verify-before-parse chain would either reject the layout or
    /// leave the original manifest in place and prove nothing.
    #[test]
    fn any_layer_permutation_unpacks_to_an_equal_server(
        seed in proptest::collection::vec(0u32..1000, FULL_LAYER_COUNT)
    ) {
        // The baseline is seed-independent, so compute it once per test binary
        // instead of re-packing and re-unpacking an identical layout on every
        // proptest case. The TempDir is dropped INSIDE the closure — an
        // `UnpackedServer` owns every byte it yields, so nothing reads the
        // directory afterwards, and a TempDir parked in a static is never
        // dropped (statics don't run destructors), which would leak one packed
        // layout into $TMPDIR per test-binary run.
        static BASELINE: std::sync::LazyLock<pmcp_package::UnpackedServer> =
            std::sync::LazyLock::new(|| {
                let dir = tempfile::tempdir().unwrap();
                let (layout, _) = packed_full_package(dir.path());
                unpack_server(&layout).unwrap()
            });
        let baseline = &*BASELINE;

        let mut order: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
        order.sort_by_key(|&i| seed[i]);

        let dir = tempfile::tempdir().unwrap();
        let (layout, _) = packed_full_package(dir.path());
        rewrite_manifest_layers(&layout, &order).unwrap();

        prop_assert_eq!(&unpack_server(&layout).unwrap(), baseline);
    }
}

// =====================================================================
// Plan 120-05 Task 1 — the config's `[[config_slots]]` block is the
// SOURCE OF TRUTH that `pack_server` reads and enforces (D-01).
//
// These exercise the agreement gate through the REAL public API path
// (`pack_server`), not through a unit call — that path is what D-01's
// "pack reads them" claims, and before this it was true of no code.
// =====================================================================

/// Pack `package` with `config_bytes` into a fresh layout, returning the result.
fn pack_with_config(
    package: &ServerPackage,
    config_bytes: &'static [u8],
    dir: &std::path::Path,
) -> Result<ManifestDigest> {
    let layout = OciLayout::create(dir).unwrap();
    pack_server(
        package,
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: config_bytes,
        }),
        None,
        None,
        &layout,
    )
}

fn config_slot_violation(err: PackageError) -> (String, String) {
    match err {
        PackageError::ConfigSlotViolation { key, reason } => (key, reason),
        other => panic!("expected ConfigSlotViolation, got: {other}"),
    }
}

/// Test 3: agreement holds — the fixture package and its config describe the
/// same slot set, so the pack succeeds through the real API.
#[test]
fn pack_server_accepts_a_package_whose_slots_agree_with_its_shipped_config() {
    let dir = tempfile::tempdir().unwrap();
    pack_with_config(&config_server_package(), CONFIG_TOML, dir.path())
        .expect("agreeing declarations and package slots must pack");
}

/// Test 4: a declaration in the TOML with no matching package slot is refused,
/// naming the key. This is the "the config declares a slot the package forgot"
/// direction — an environment-specific value would otherwise be baked while the
/// package still looked slot-complete.
#[test]
fn pack_server_refuses_a_declaration_the_package_does_not_carry() {
    const EXTRA_DECLARATION: &[u8] = br#"name = "london-tube"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[backend]
api_key = "${TFL_API_KEY}"
base_url = "${TFL_BASE_URL}"
"#;
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&config_server_package(), EXTRA_DECLARATION, dir.path())
        .expect_err("a declared slot the package omits must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("absent from the package"), "was: {reason}");
}

/// Test 5: the opposite direction — a package inventing a slot its shipped
/// config never declares. The cross-AI review called this one out specifically.
#[test]
fn pack_server_refuses_a_package_slot_the_shipped_config_never_declares() {
    let mut package = config_server_package();
    package.config_slots.push(
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url"),
    );
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("an invented package slot must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("absent from the config"), "was: {reason}");
}

/// Test 6: same key on both sides, different KIND.
#[test]
fn pack_server_refuses_a_kind_disagreement_naming_both_kinds() {
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Endpoint {
        name: "TFL_API_KEY".to_string(),
        tested_value: "https://api.tfl.gov.uk".to_string(),
    })
    .with_config_key("backend.api_key")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("a kind disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("secret"), "was: {reason}");
    assert!(reason.contains("endpoint"), "was: {reason}");
}

/// Test 7: same key and kind, different `name`.
#[test]
fn pack_server_refuses_a_name_disagreement_without_echoing_either_name() {
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Secret {
        name: "SENTINEL_DISAGREEING_NAME".to_string(),
    })
    .with_config_key("backend.api_key")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("a name disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("`name`"), "was: {reason}");
    assert!(
        !reason.contains("SENTINEL_DISAGREEING_NAME"),
        "the error names the FIELD, never the values; was: {reason}"
    );
}

/// Test 7b: same key, kind and name, different `tested_value`.
#[test]
fn pack_server_refuses_a_tested_value_disagreement_without_echoing_either_value() {
    const ENDPOINT_ONLY: &[u8] = br#"[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[backend]
base_url = "${TFL_BASE_URL}"
"#;
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Endpoint {
        name: "TFL_BASE_URL".to_string(),
        tested_value: "https://sentinel.invalid/untested".to_string(),
    })
    .with_config_key("backend.base_url")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, ENDPOINT_ONLY, dir.path())
        .expect_err("a tested_value disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("`tested_value`"), "was: {reason}");
    assert!(
        !reason.contains("sentinel.invalid"),
        "the error names the FIELD, never the values; was: {reason}"
    );
}

/// Test 8: with `config: None` no parsing and no agreement check happen at all.
/// The pre-existing embedded shape — a package carrying an undeclared `Secret`
/// slot and no config document — still packs exactly as it always did. This is
/// the regression that would otherwise break every earlier server-package test.
#[test]
fn an_embedded_package_with_an_undeclared_slot_still_packs_because_no_config_is_present() {
    let mut package = config_server_package();
    // No config_key at all, and nothing declares it — legal, because there is
    // no config document for a declaration to live in.
    package.config_slots = vec![ConfigSlot::new(SlotType::Secret {
        name: "TFL_API_KEY".to_string(),
    })];
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let bootstrap = b"fake-bootstrap-bytes".to_vec();
    pack_server(
        &package,
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        None,
        &layout,
    )
    .expect("a config-less package must skip config-slot validation entirely");
    assert_eq!(unpack_server(&layout).unwrap().package, package);
}

/// Test 9 (parse-side): an unknown `kind` in the shipped config is refused by
/// `pack_server` — `pmcp-package` re-validates the vocabulary rather than
/// trusting that the bytes came through the toolkit's `ServerConfig`.
#[test]
fn pack_server_refuses_a_config_declaring_an_unknown_slot_kind() {
    const BAD_KIND: &[u8] = br#"[[config_slots]]
key = "backend.api_key"
kind = "endpont"
name = "TFL_API_KEY"
"#;
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&config_server_package(), BAD_KIND, dir.path())
        .expect_err("an unknown kind must be refused at pack time");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("auth_mode"), "was: {reason}");
}

// =====================================================================
// Plan 120-05 Task 2 — D-04 placeholder validation, through the real
// `pack_server` path, and the "a rejected pack writes NOTHING" property.
// =====================================================================

/// The distinctive literal a failing fixture bakes in. Its ABSENCE from the
/// error message is what proves the validator never echoes a config value —
/// asserted, not inspected.
const SENTINEL_CREDENTIAL: &str = "sentinel-leaked-credential";

/// A config whose slot-declared credential key holds a RESOLVED literal — the
/// exact shape D-04 exists to refuse.
const CONFIG_WITH_BAKED_CREDENTIAL: &[u8] = br#"name = "london-tube"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
api_key = "sentinel-leaked-credential"
"#;

/// Every file currently in the layout's `blobs/sha256/` directory, sorted.
fn blob_file_names(root: &std::path::Path) -> Vec<String> {
    let blobs = root.join("blobs").join("sha256");
    let mut names: Vec<String> = std::fs::read_dir(&blobs)
        .unwrap_or_else(|e| panic!("blobs dir {blobs:?} must exist after create: {e}"))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Test 2/3: a slot-declared value key holding a resolved literal is refused by
/// `pack_server`, and the error names the key WITHOUT echoing the literal.
#[test]
fn pack_server_refuses_a_config_that_bakes_a_slot_declared_credential() {
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(
        &config_server_package(),
        CONFIG_WITH_BAKED_CREDENTIAL,
        dir.path(),
    )
    .expect_err("a resolved credential at a slot-declared key must not pack");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("resolved literal"), "was: {reason}");
    assert!(
        !reason.contains(SENTINEL_CREDENTIAL),
        "the message must never carry the value it rejected; was: {reason}"
    );
}

/// Test 9 (the precise form): a rejected pack leaves the layout in its
/// post-`create` state and NOTHING more.
///
/// Asserting only "the index holds no manifest" would miss leaked config bytes
/// sitting in `blobs/sha256/` — and leaked config bytes are the exact thing
/// this validation exists to prevent. `OciLayout::create` already writes
/// `oci-layout`, an empty `index.json` and the blobs directory, so "no layout
/// behind" was never literally true; the checkable claim is that the blob file
/// SET is unchanged.
#[test]
fn a_rejected_pack_adds_neither_a_blob_nor_an_index_entry() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let before = blob_file_names(dir.path());

    let err = pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: CONFIG_WITH_BAKED_CREDENTIAL,
        }),
        None,
        None,
        &layout,
    )
    .expect_err("the baked credential must abort the pack");
    assert!(matches!(err, PackageError::ConfigSlotViolation { .. }));

    assert_eq!(
        blob_file_names(dir.path()),
        before,
        "a rejected pack must not write a single blob — a leaked config layer here would be \
         the very disclosure the validation exists to prevent"
    );
    assert!(
        layout.read_index().unwrap().manifests().is_empty(),
        "a rejected pack must not record a manifest in the index"
    );
}

/// The auth-mode carve-out (D-17) holds through the real pack path: a
/// slot-declared auth-mode key holding a baked literal packs successfully,
/// because `AuthConfig` is internally tagged and no placeholder form of that
/// key can deserialize at all.
#[test]
fn pack_server_accepts_a_slot_declared_auth_mode_key_holding_a_literal() {
    const AUTH_MODE_CONFIG: &[u8] = br#"[[config_slots]]
key = "backend.auth.type"
kind = "auth_mode"
name = "backend-auth-mode"
tested_value = "api_key"

[backend.auth]
type = "api_key"
"#;
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::AuthMode {
        name: "backend-auth-mode".to_string(),
        tested_value: "api_key".to_string(),
    })
    .with_config_key("backend.auth.type")];
    let dir = tempfile::tempdir().unwrap();
    pack_with_config(&package, AUTH_MODE_CONFIG, dir.path())
        .expect("the structural auth-mode key is exempt from the placeholder rule (D-17)");
}

// =====================================================================
// Plan 120-05 Task 3 — the REAL london-tube fixture pair, vendored with a
// drift guard, packed for real, and asserted against PKG-03 criterion 3.
//
// Vendored rather than reached across with `include_str!` because
// `crates/pmcp-openapi-server/Cargo.toml` has `exclude = [... "tests/" ...]`:
// a published `pmcp-package` tarball cannot see another crate's excluded
// test directory. The copy is only trustworthy with the guard below.
// =====================================================================

/// The vendored copies must stay byte-identical to their sources, or the
/// digest this crate pins is a digest of a config the real server no longer
/// boots from (T-120-23).
///
/// Gated on the sibling CRATE DIRECTORY, not on the individual files: an
/// absent crate directory means a published tarball, where there is nothing to
/// compare against. A PRESENT crate directory with a MISSING source file means
/// the source moved — that must FAIL, not skip.
#[test]
fn the_vendored_london_tube_fixtures_have_not_drifted_from_their_sources() {
    let crate_dir = openapi_server_crate_dir();
    if !crate_dir.is_dir() {
        println!(
            "skipping drift guard: sibling crate {crate_dir:?} is absent (published-tarball build)"
        );
        return;
    }
    let source_dir = crate_dir.join("tests").join("fixtures");
    for name in [LONDON_TUBE_CONFIG_NAME, LONDON_TUBE_SPEC_NAME] {
        let source_path = source_dir.join(name);
        let source = std::fs::read(&source_path).unwrap_or_else(|e| {
            panic!(
                "the sibling crate exists but its fixture {source_path:?} is missing ({e}) — the \
                 source moved; update the vendored copy and this path rather than skipping"
            )
        });
        assert_eq!(
            vendored_fixture(name),
            source,
            "the vendored copy of {name} has DRIFTED from \
             crates/pmcp-openapi-server/tests/fixtures/{name}; re-copy it, because pmcp-package \
             must pack the same bytes the real server boots from"
        );
    }
}

/// The proving case: the real config the reference OpenAPI server boots from,
/// with its three declared slots and its `${TFL_BASE_URL}` endpoint
/// placeholder, packs as a config-only package with no bootstrap layer. Both
/// gates run against it — a fixture that regressed to a literal endpoint, or
/// whose declaration block drifted from what the package claims, fails here.
#[test]
fn the_real_london_tube_fixture_packs_as_a_config_only_package() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_london_tube(dir.path(), Some(&common::london_tube_spec_bytes()));

    let media_types = layer_media_types(&layout);
    assert!(
        !media_types.iter().any(|mt| mt == MT_SERVER_BOOTSTRAP),
        "a pure-config package carries no bootstrap layer; layers were: {media_types:?}"
    );
    assert!(
        media_types.iter().any(|mt| mt == MT_SERVER_CONFIG),
        "the config layer must be present; layers were: {media_types:?}"
    );
    assert!(
        media_types.iter().any(|mt| mt == MT_SERVER_OPENAPI_SPEC),
        "the spec layer must be present; layers were: {media_types:?}"
    );

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.config.unwrap().bytes,
        vendored_fixture(LONDON_TUBE_CONFIG_NAME),
        "the author's config bytes must survive the round trip verbatim"
    );
}

/// The concrete divergence the cross-AI review described, asserted against the
/// PROVING fixture rather than a synthetic one: drop one `[[config_slots]]`
/// entry from a copy of the real config, pack with the unmodified three-slot
/// package, and the removed key is named.
#[test]
fn dropping_one_declaration_from_the_real_fixture_is_refused_naming_that_key() {
    const REMOVED_KEY: &str = "backend.auth.query_params.app_key";
    let config_bytes = vendored_fixture(LONDON_TUBE_CONFIG_NAME);
    let package = london_tube_package(&config_bytes);
    assert_eq!(package.config_slots.len(), 3);

    let mutilated = remove_declaration_block(&config_bytes, REMOVED_KEY);
    assert_ne!(
        mutilated, config_bytes,
        "the removal must actually change the bytes"
    );
    assert_eq!(
        parse_declared_config_slots(&mutilated).unwrap().len(),
        2,
        "exactly one declaration must have been removed"
    );

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let err = pack_server(
        &package,
        referenced_binary(),
        Some(ConfigFile {
            file_name: LONDON_TUBE_CONFIG_NAME,
            bytes: &mutilated,
        }),
        None,
        None,
        &layout,
    )
    .expect_err("a package claiming a slot the shipped config no longer declares must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, REMOVED_KEY);
    assert!(reason.contains("absent from the config"), "was: {reason}");
}

/// Remove the `[[config_slots]]` block whose `key = "<config_key>"`, by line
/// range: back to the preceding `[[config_slots]]` header, forward to the next
/// line opening a new table.
fn remove_declaration_block(config_bytes: &[u8], config_key: &str) -> Vec<u8> {
    let text = std::str::from_utf8(config_bytes).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let key_line = format!("key = \"{config_key}\"");
    let at = lines
        .iter()
        .position(|line| line.trim() == key_line)
        .unwrap_or_else(|| panic!("the fixture must declare {config_key}"));
    let start = lines[..at]
        .iter()
        .rposition(|line| line.trim() == "[[config_slots]]")
        .expect("the key line must sit inside a [[config_slots]] block");
    let end = lines[at..]
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .map_or(lines.len(), |offset| at + offset);

    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    kept.extend_from_slice(&lines[..start]);
    kept.extend_from_slice(&lines[end..]);
    let mut out = kept.join("\n");
    out.push('\n');
    out.into_bytes()
}

/// PKG-03 criterion 3: the endpoint, credential and auth-mode slots surface as
/// `ConfigSlot`s with the right families, aggregate deterministically, and NO
/// slot among them derives from the OpenAPI spec — the spec is baked, never a
/// slot a target environment fills in.
#[test]
fn the_real_fixtures_three_slots_classify_aggregate_and_carry_no_spec_derived_slot() {
    let config_bytes = vendored_fixture(LONDON_TUBE_CONFIG_NAME);
    let package = london_tube_package(&config_bytes);
    assert_eq!(package.config_slots.len(), 3);

    // --- classify: two behaviour-relevant, one identity-bearing ----------
    for slot in &package.config_slots {
        let (kind, _) = slot.slot.key();
        let expected = match kind {
            "endpoint" | "auth_mode" => SlotClass::BehaviorRelevant,
            "secret" => SlotClass::IdentityBearing,
            other => panic!("unexpected kind {other}"),
        };
        assert_eq!(
            classify(&slot.slot),
            expected,
            "slot {kind} classified into the wrong family"
        );
    }

    // --- aggregate: exactly three, deterministically ordered -------------
    let aggregated = aggregate(&package.config_slots).unwrap();
    assert_eq!(aggregated.len(), 3);
    let ordered: Vec<&str> = aggregated.iter().map(|slot| slot.slot.key().0).collect();
    assert_eq!(
        ordered,
        vec!["auth_mode", "endpoint", "secret"],
        "aggregation order must be deterministic"
    );
    let reversed: Vec<ConfigSlot> = package.config_slots.iter().rev().cloned().collect();
    assert_eq!(
        aggregate(&reversed).unwrap(),
        aggregated,
        "aggregation must not depend on input order"
    );

    // --- no spec-derived slot --------------------------------------------
    let kinds: std::collections::BTreeSet<&str> =
        aggregated.iter().map(|slot| slot.slot.key().0).collect();
    assert_eq!(
        kinds,
        ["auth_mode", "endpoint", "secret"].into_iter().collect(),
        "the three slots are exactly endpoint/secret/auth_mode"
    );
    for slot in &aggregated {
        let (kind, name) = slot.slot.key();
        for token in ["spec", "openapi", "yaml", LONDON_TUBE_SPEC_NAME] {
            assert!(
                !kind.contains(token) && !name.contains(token),
                "no slot may derive from the OpenAPI spec — the spec is BAKED and can only move \
                 the package digest; offending slot: {kind}/{name}"
            );
        }
    }

    // --- required_slots returns BOTH families, which detect_deviation cannot
    let required = required_slots(&package.config_slots);
    assert_eq!(required.len(), 3);
    assert!(
        required
            .iter()
            .any(|r| r.class == SlotClass::IdentityBearing),
        "the credential must appear in the inventory a target environment must fill"
    );
    assert!(
        required.iter().all(|r| r.config_key.is_some()),
        "every config-server slot names the config path it fills"
    );
}

// =====================================================================
// Plan 120-05 Task 4 — the package half of the cross-crate env-reference
// grammar parity assertion. The toolkit half lives in
// crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs and reads
// the SAME table.
// =====================================================================

/// Every row of `tests/golden_fixtures/env_ref_grammar_v1.tsv` must hold.
///
/// `is_env_reference` is a private helper, so the grammar is asserted through
/// the public surface that depends on it: a slot-declared value key holding the
/// row's input packs iff the row says `accept`.
///
/// This is not a weaker assertion than calling the predicate directly — it is
/// the one that matters, because "does this config pack?" is the question the
/// grammar actually answers for a user.
#[test]
fn is_env_reference_agrees_with_the_shared_grammar_table_on_every_row() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_fixtures")
        .join(ENV_REF_GRAMMAR_TABLE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the shared grammar table {path:?} must exist: {e}"));
    let cases = parse_env_ref_grammar_table(&text);
    assert!(
        cases.len() >= 10,
        "the table must be non-trivial; it had {} rows",
        cases.len()
    );

    for case in &cases {
        // A TOML basic string cannot carry a raw `"` or `\`; no row needs one,
        // and asserting it here keeps a future row from silently mis-encoding.
        assert!(
            !case.input.contains('"') && !case.input.contains('\\'),
            "grammar-table inputs must be TOML-basic-string safe; row was {:?}",
            case.input
        );
        let config = format!(
            "[[config_slots]]\nkey = \"backend.api_key\"\nkind = \"secret\"\nname = \"K\"\n\n\
             [backend]\napi_key = \"{}\"\n",
            case.input
        );
        let package = {
            let mut package = config_server_package();
            package.config_slots = vec![ConfigSlot::new(SlotType::Secret {
                name: "K".to_string(),
            })
            .with_config_key("backend.api_key")];
            package
        };
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let result = pack_server(
            &package,
            referenced_binary(),
            Some(ConfigFile {
                file_name: CONFIG_FILE_NAME,
                bytes: config.as_bytes(),
            }),
            None,
            None,
            &layout,
        );
        assert_eq!(
            result.is_ok(),
            case.accepted,
            "grammar drift on row {:?}: the table says {}, pmcp-package says {}. This table is \
             the contract with pmcp-server-toolkit's parse_env_ref — change it only \
             deliberately, and move BOTH implementations.",
            case.input,
            if case.accepted { "accept" } else { "reject" },
            if result.is_ok() { "accept" } else { "reject" },
        );
    }
}
