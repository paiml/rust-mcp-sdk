//! Local OCI Image Layout reader/writer — `oci-layout` marker file,
//! `index.json` (`ImageIndex`), and content-addressed blobs under
//! `blobs/sha256/<hex>` (opencontainers/image-spec `image-layout.md`).
//!
//! Pure local-disk I/O — no network calls. the `oci-client`
//! push/pull consumes the exact `oci_spec::image` types this module
//! reads/writes, with zero translation layer.
//!
//! # `oci_spec::image::OciLayout` (RESEARCH Assumption A2, resolved)
//!
//! `oci_spec::image` DOES expose a typed `OciLayout` struct
//! (`image_layout_version: String`, with `Builder`/`from_file`/`to_file`
//! helpers) — see `oci_spec::image::OciLayoutBuilder`. This module's own
//! [`OciLayout`] type (a different concept: the on-disk DIRECTORY, not the
//! one-field marker file) writes the marker file's JSON by hand
//! (`{"imageLayoutVersion":"1.0.0"}`) rather than depending on that type
//! directly, to keep this module's public surface centered on directory
//! operations (`write_blob`/`read_blob`/`write_index`/...) rather than
//! re-exporting an oci_spec type under a colliding name.

use crate::digest::ManifestDigest;
use crate::error::{PackageError, Result};
use oci_spec::image::{
    Descriptor, ImageIndex, ImageIndexBuilder, ImageManifest, MediaType, SCHEMA_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The `oci-layout` marker file's exact required content (image-layout.md).
const OCI_LAYOUT_FILE_CONTENTS: &str = r#"{"imageLayoutVersion":"1.0.0"}"#;

/// A local OCI Image Layout directory: `oci-layout` + `index.json` +
/// content-addressed `blobs/sha256/<hex>`.
#[derive(Debug, Clone)]
pub struct OciLayout {
    root: PathBuf,
}

impl OciLayout {
    /// Create a fresh, empty OCI Image Layout at `root`: writes the
    /// `oci-layout` marker file, an initially-empty `index.json`, and the
    /// `blobs/sha256/` directory. Fails if `root` cannot be created.
    pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(blobs_sha256_dir(&root))?;
        let layout = Self { root };
        fs::write(layout.root.join("oci-layout"), OCI_LAYOUT_FILE_CONTENTS)?;
        let empty_index = ImageIndexBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .manifests(Vec::<Descriptor>::new())
            .build()
            .map_err(|e| PackageError::Layout {
                reason: format!("failed to build empty ImageIndex: {e}"),
            })?;
        layout.write_index(&empty_index)?;
        Ok(layout)
    }

    /// Open a reference to an existing OCI Image Layout directory at `root`.
    /// Does not validate contents until `read_index`/`read_manifest`/
    /// `read_blob` is called.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// This layout's root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Describe the blob `bytes` WOULD have if it were written under
    /// `media_type` — without writing anything.
    ///
    /// Side-effect free by construction: it touches no directory (hence no
    /// `&self`), performs no filesystem call, and is infallible. Nothing on
    /// this path can fail — `blob_path`'s `Result` comes only from its
    /// `sha256:` prefix check, and a descriptor needs no path.
    ///
    /// # The contract that makes this load-bearing
    ///
    /// `Self::describe_blob(mt, b)` and `layout.write_blob(mt, b)` return
    /// EQUAL descriptors — same digest, same size, same media type — for equal
    /// inputs. The only difference between them is that one writes. That
    /// equality is not a convenience: `pack_server`'s attestation-subject gate
    /// computes the would-be unattested manifest digest from descriptions
    /// alone, BEFORE the first blob is written, and then `pack_server` writes
    /// those very layers. If the two paths ever disagreed, the gate would be
    /// comparing against a digest no produced package could ever have.
    ///
    /// [`Self::write_blob`] is therefore expressed IN TERMS of this function
    /// rather than constructing a descriptor of its own, and
    /// `describe_blob_returns_the_same_descriptor_write_blob_does` pins the
    /// equality. Do not "optimize" either path independently.
    pub fn describe_blob(media_type: MediaType, bytes: &[u8]) -> Descriptor {
        let digest = ManifestDigest::from_bytes(bytes);
        Descriptor::new(media_type, bytes.len() as u64, oci_digest(&digest))
    }

    /// Content-address `bytes` under `blobs/sha256/<hex>` and return the
    /// resulting `Descriptor` (the caller supplies `media_type` — the
    /// vendor `application/vnd.pmcp.*` type for this logical layer, or a
    /// standard OCI type for the manifest/empty-config blobs).
    ///
    /// The returned descriptor comes from [`Self::describe_blob`], so the
    /// written blob and the described blob can never disagree on digest, size
    /// or media type.
    pub fn write_blob(&self, media_type: MediaType, bytes: &[u8]) -> Result<Descriptor> {
        let descriptor = Self::describe_blob(media_type, bytes);
        // Round-trips the descriptor's own digest rather than re-hashing the
        // bytes, so the path written to is derived from the SAME digest the
        // caller is handed back.
        let digest = ManifestDigest::try_from(descriptor.digest())?;
        let path = self.blob_path(&digest)?;
        fs::write(&path, bytes)?;
        Ok(descriptor)
    }

    /// Read the blob addressed by `descriptor`'s digest. This looks the
    /// blob up by its content-addressed path only — it does NOT re-verify
    /// the bytes against the descriptor's declared digest (that tamper
    /// check is [`crate::digest::verify()`], called explicitly by
    /// `oci::unpack` before any deserialize — see that module's docs for
    /// why the check belongs there, not here).
    pub fn read_blob(&self, descriptor: &Descriptor) -> Result<Vec<u8>> {
        let digest = ManifestDigest::try_from(descriptor.digest())?;
        let path = self.blob_path(&digest)?;
        let bytes = fs::read(&path).map_err(|e| PackageError::Layout {
            reason: format!("failed to read blob {}: {e}", path.display()),
        })?;
        Ok(bytes)
    }

    /// Overwrite `index.json` with `index`.
    pub fn write_index(&self, index: &ImageIndex) -> Result<()> {
        let bytes = serde_json::to_vec(index)?;
        fs::write(self.root.join("index.json"), bytes)?;
        Ok(())
    }

    /// Read `index.json`.
    pub fn read_index(&self) -> Result<ImageIndex> {
        let bytes = fs::read(self.root.join("index.json")).map_err(|e| PackageError::Layout {
            reason: format!("failed to read index.json: {e}"),
        })?;
        let index = serde_json::from_slice(&bytes)?;
        Ok(index)
    }

    /// Write `manifest` as a content-addressed blob (an OCI manifest is
    /// itself just a JSON blob, referenced by digest from `index.json`
    /// like any other layer). Returns the manifest blob's own `Descriptor`.
    pub fn write_manifest(&self, manifest_bytes: &[u8]) -> Result<Descriptor> {
        self.write_blob(MediaType::ImageManifest, manifest_bytes)
    }

    /// Read the manifest blob referenced by `descriptor` and parse it. Like
    /// [`Self::read_blob`], this does not itself verify the digest — callers
    /// that need tamper detection should verify the raw bytes (via
    /// [`Self::read_blob`] + [`crate::digest::verify()`]) before parsing.
    pub fn read_manifest(&self, descriptor: &Descriptor) -> Result<ImageManifest> {
        let bytes = self.read_blob(descriptor)?;
        let manifest = serde_json::from_slice(&bytes)?;
        Ok(manifest)
    }

    /// Derive the `blobs/sha256/<hex>` path for `digest`.
    ///
    /// Path-traversal guard: the hex segment comes ONLY from an
    /// already-validated [`ManifestDigest`] (exactly 64 lowercase ASCII-hex
    /// characters — `ManifestDigest::parse`/`from_bytes`/`TryFrom<&Digest>`
    /// are its only constructors, and none of them can produce a `/`, `.`,
    /// or `..` in the hex portion), so the joined path can never escape
    /// `root`. The `starts_with` check below is defense-in-depth, asserting
    /// that invariant explicitly rather than relying on it silently.
    fn blob_path(&self, digest: &ManifestDigest) -> Result<PathBuf> {
        let hex = digest
            .as_str()
            .strip_prefix("sha256:")
            .ok_or_else(|| PackageError::Layout {
                reason: format!("expected a sha256 digest, got: {digest}"),
            })?;
        let dir = blobs_sha256_dir(&self.root);
        let path = dir.join(hex);
        if !path.starts_with(&dir) {
            return Err(PackageError::Layout {
                reason: format!("blob path {path:?} escaped layout root {dir:?}"),
            });
        }
        Ok(path)
    }
}

fn blobs_sha256_dir(root: &Path) -> PathBuf {
    root.join("blobs").join("sha256")
}

/// Convert a validated [`ManifestDigest`] into the `oci_spec::image::Digest`
/// type a `Descriptor` carries. Infallible: `ManifestDigest` already
/// guarantees the `sha256:<64-hex>` form `Digest::from_str` accepts.
fn oci_digest(digest: &ManifestDigest) -> oci_spec::image::Digest {
    oci_spec::image::Digest::from_str(digest.as_str())
        .expect("ManifestDigest always parses as a valid oci_spec::image::Digest")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oci::media_types::{EMPTY_CONFIG_BLOB, EMPTY_CONFIG_DIGEST, MT_EMPTY_CONFIG};

    #[test]
    fn create_writes_oci_layout_marker_file_and_empty_index() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();

        let oci_layout_contents = fs::read_to_string(dir.path().join("oci-layout")).unwrap();
        assert_eq!(oci_layout_contents, OCI_LAYOUT_FILE_CONTENTS);

        let index = layout.read_index().unwrap();
        assert_eq!(index.manifests().len(), 0);
        assert_eq!(index.schema_version(), SCHEMA_VERSION);

        assert!(dir.path().join("blobs").join("sha256").is_dir());
    }

    #[test]
    fn write_blob_then_read_blob_round_trips_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();

        let bytes = b"hello oci layout";
        let descriptor = layout
            .write_blob(
                MediaType::Other("application/vnd.pmcp.test.v1+json".to_string()),
                bytes,
            )
            .unwrap();

        let expected_digest = ManifestDigest::from_bytes(bytes);
        let hex = expected_digest.as_str().strip_prefix("sha256:").unwrap();
        let blob_path = dir.path().join("blobs").join("sha256").join(hex);
        assert!(
            blob_path.is_file(),
            "blob must be written under blobs/sha256/<hex>"
        );

        let read_back = layout.read_blob(&descriptor).unwrap();
        assert_eq!(read_back, bytes);
        assert_eq!(descriptor.digest().to_string(), expected_digest.as_str());
        assert_eq!(descriptor.size(), bytes.len() as u64);
    }

    /// The equality [`OciLayout::describe_blob`]'s rustdoc promises, pinned by
    /// a test rather than left to inspection: the pure description and the
    /// written one are the SAME descriptor, differing only in that one of them
    /// touched the disk.
    ///
    /// The pack-time attestation-subject gate computes a manifest digest over
    /// descriptions alone, then `pack_server` writes the very same layers. If
    /// these two paths ever disagreed on digest, size or media type, that gate
    /// would compare a digest no produced package could ever have.
    #[test]
    fn describe_blob_returns_the_same_descriptor_write_blob_does() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();

        let media_type = MediaType::Other("application/vnd.pmcp.test.v1+json".to_string());
        let bytes = b"the pure and the writing path must agree";

        let described = OciLayout::describe_blob(media_type.clone(), bytes);
        let written = layout.write_blob(media_type, bytes).unwrap();

        assert_eq!(
            described, written,
            "describe_blob and write_blob must return equal descriptors for equal inputs"
        );
    }

    #[test]
    fn write_manifest_then_read_manifest_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();

        let config = layout
            .write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)
            .unwrap();
        let layer = layout
            .write_blob(
                MediaType::Other("application/vnd.pmcp.test.v1+json".to_string()),
                b"{}",
            )
            .unwrap();
        let manifest = oci_spec::image::ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .config(config)
            .layers(vec![layer])
            .build()
            .unwrap();

        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let manifest_descriptor = layout.write_manifest(&manifest_bytes).unwrap();
        let read_back = layout.read_manifest(&manifest_descriptor).unwrap();
        assert_eq!(read_back, manifest);
    }

    #[test]
    fn empty_config_blob_digest_constant_matches_from_bytes_hash() {
        let digest = ManifestDigest::from_bytes(EMPTY_CONFIG_BLOB);
        assert_eq!(digest.as_str(), EMPTY_CONFIG_DIGEST);
    }
}
