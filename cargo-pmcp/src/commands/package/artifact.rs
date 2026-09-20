//! The `.tar` <-> OCI-image-layout codec behind `package save` / `package load`
//! — the ONLY module in this repository that names `tar::`.
//!
//! D-11 gives a package TWO on-disk representations: the OCI image layout
//! DIRECTORY, which is the identity-bearing working form every shipped verb
//! already operates on, and an uncompressed `.tar` of that directory, which is a
//! pure carriage envelope. The tar contributes NOTHING to package identity —
//! identity is the manifest digest `pack_server` computes over the layout's
//! blobs — and `load` discards the tar the moment its contents are verified.
//!
//! # Why the whole-archive extraction path is never used
//!
//! `tar::Archive::unpack` (and any `dest.join(entry.path()?)` written by hand)
//! takes a destination filename FROM THE ARCHIVE. Every defence against that is
//! a filter, and filters are a list of the traversals someone thought of.
//!
//! This module does not filter, because it never has a path to filter. Entries
//! are parsed into memory, gated, and then written through
//! [`OciLayout::write_blob`], which derives its destination from
//! `ManifestDigest::from_bytes(bytes)` — a digest THIS code computed over bytes
//! it is holding (`crates/pmcp-package/src/oci/layout.rs:96-101`). An archive
//! path is a lookup key during validation and is never joined onto the
//! filesystem, so traversal is unrepresentable rather than filtered.
//!
//! That is also why `cap-std` is deliberately absent. The TOCTOU class it
//! defends — a path resolving to a different object between check and open — is
//! unreachable when no archive-supplied path is ever opened. The
//! resource-exhaustion half of the untrusted-bytes problem is answered
//! separately, by the byte caps this module installs on the read path.
//!
//! # This module is a dependency-light LEAF, on purpose
//!
//! It names `tar`, `anyhow`, `serde_json`, `oci_spec`, `tempfile`,
//! `pmcp_package` and `std` — no `clap`, no `GlobalFlags`, no
//! `crate::commands::*`. `cargo-pmcp/src/lib.rs` mounts it a second time as
//! `cargo_pmcp::package_artifact` with `#[path]`, exactly as it does
//! `commands/package/kind.rs`, so the property tests run under `cargo test
//! --lib` and a fuzz target can reach the untrusted-bytes boundary. That mount
//! is what forbids `super::`/`crate::commands::` here: in the lib target this
//! file's parent is the crate root, which declares no `commands` module.
//!
//! The consequence is visible in [`install_layout`]'s signature. The semantic
//! gate it runs — `detect_kind` plus the matching `unpack_*` — lives in the
//! bin-only command tree, so it arrives as a CLOSURE rather than as a direct
//! call. The ordering the closure is called in is the whole point and is fixed
//! here, not at the call site: staging is written, the closure validates the
//! STAGING layout, and only a successful closure earns the rename into place.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use oci_spec::image::{Descriptor, ImageIndex, ImageManifest, MediaType};
use pmcp_package::oci::OciLayout;
use pmcp_package::{canonicalize, ManifestDigest};

/// The layout marker file's archive path (`crates/pmcp-package/src/oci/layout.rs:48`).
const LAYOUT_MARKER_PATH: &str = "oci-layout";

/// The image index's archive path — the required entry point of an OCI layout.
const INDEX_PATH: &str = "index.json";

/// The one directory prefix a content-addressed blob may appear under.
const BLOB_PREFIX: &str = "blobs/sha256/";

// ---------------------------------------------------------------------
// The validated artifact model
// ---------------------------------------------------------------------

/// One blob the archive carried, paired with the descriptor facts that
/// authorize writing it.
///
/// `media_type` is NOT decoration: [`OciLayout::write_blob`] takes a
/// `MediaType` for every blob it writes, including the manifest blob and the
/// config blob (`crates/pmcp-package/src/oci/layout.rs:108`). A digest -> bytes
/// map carries none, so it cannot reconstruct a layout without inventing one.
/// Every value here came from the descriptor that referenced the blob.
#[derive(Debug, Clone)]
pub struct VerifiedBlob {
    /// The blob's exact bytes as the archive carried them.
    pub bytes: Vec<u8>,
    /// The media type of the FIRST descriptor that referenced this blob.
    ///
    /// "First" rather than "the" because an OCI layout is content-addressed and
    /// therefore de-duplicating: two layers whose payloads are byte-identical
    /// (two empty `[]` sections, say) share one blob file while carrying
    /// different vendor media types in the manifest. That is a legitimate
    /// package, so a media-type disagreement is recorded rather than refused.
    ///
    /// It is also why the value is safe: `write_blob` derives the destination
    /// filename from the BYTES, and the manifest itself is written back
    /// verbatim as its own blob, so the media type chosen here cannot change
    /// what lands on disk or what a reader sees.
    pub media_type: MediaType,
    /// The blob's length in bytes, cross-checked against every descriptor that
    /// declared a size for it.
    pub size: u64,
}

/// An artifact that has passed EVERY read-side gate, modelled as a validated
/// descriptor graph rather than as a bag of bytes.
///
/// Only [`read_verified_with_limits`] constructs one, and it touches the
/// filesystem zero times. [`write_layout`] therefore performs no validation of
/// its own: holding this type IS the proof. That split is the read-side mirror
/// of `pack_server`'s "a rejected pack adds neither a blob nor an index entry"
/// (`crates/pmcp-package/src/oci/pack.rs:913-937`).
#[derive(Debug, Clone)]
pub struct VerifiedArtifact {
    /// `index.json`, parsed.
    pub index: ImageIndex,
    /// `index.json`'s exact bytes as read, kept for byte-fidelity checks.
    ///
    /// [`write_layout`] deliberately does NOT write these back: it regenerates
    /// the destination's `index.json` through [`write_canonical_index`], so a
    /// loaded layout's index is canonical whatever the producer emitted.
    /// Keeping the originals lets a caller (and this module's tests) ask
    /// whether a producer's index was ALREADY canonical — a question about the
    /// PRODUCER, which must not be answered by silently preserving its bytes.
    pub index_bytes: Vec<u8>,
    /// THE one descriptor `index.manifests()` declares.
    pub manifest_descriptor: Descriptor,
    /// The image manifest parsed from the blob `manifest_descriptor` addresses.
    pub manifest: ImageManifest,
    /// Every blob the archive carried, keyed by lowercase hex digest.
    pub blobs: BTreeMap<String, VerifiedBlob>,
    /// The manifest digest, derived LOCALLY over the manifest blob's bytes —
    /// never read out of the archive and called derived.
    pub manifest_digest: ManifestDigest,
}

/// A layout that has been staged, semantically validated and renamed into its
/// final destination, together with whatever the validation produced.
///
/// `unpacked` is the value the caller's semantic gate returned while it ran
/// against the STAGING layout. Handing it back is what stops `load` from
/// unpacking a second time — and, more importantly, stops a second unpack call
/// site existing at all, since that is where a later change would reintroduce
/// the install-then-validate ordering this design removes.
#[derive(Debug)]
pub struct InstalledLayout<T> {
    /// The installed layout, opened at its final destination.
    pub layout: OciLayout,
    /// What the semantic gate produced against the staging layout.
    pub unpacked: T,
}

/// Which of the three slots an archive entry occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySlot {
    /// The `oci-layout` marker file. Accepted, its content ignored, and always
    /// regenerated by `OciLayout::create` on write (RESEARCH Pitfall 7).
    LayoutMarker,
    /// `index.json`.
    Index,
    /// `blobs/sha256/<hex>`, carrying the lowercase hex digest.
    Blob(String),
}

// ---------------------------------------------------------------------
// Reading: untrusted bytes in, a validated model out, nothing written
// ---------------------------------------------------------------------

/// The two byte budgets [`read_verified_with_limits`] enforces while parsing an
/// artifact into memory.
///
/// D-06 accepts holding the artifact in memory as a cost but names no bound,
/// and an unbounded hold over untrusted bytes is an OOM waiting for a
/// mis-sized, hostile or accidentally-huge input. These are that bound.
///
/// # Why this is injectable
///
/// Not for convenience — for FALSIFIABILITY. A cap that is never observed to
/// refuse anything is indistinguishable from a cap that does not work, and
/// proving a multi-hundred-megabyte cap with real bytes is not a test anyone
/// will run. With the limits as a parameter, "the cap is what refused this" is
/// a two-line deterministic experiment: feed one input under a tiny cap and
/// assert the specific refusal, feed the SAME input under a large cap and
/// assert acceptance. The pair is what turns "an error happened" into "the cap
/// caused it".
///
/// A fuzz campaign cannot do this. Arbitrary bytes are overwhelmingly unlikely
/// to produce a well-framed multi-megabyte entry, so a short campaign would
/// never approach the cap at all. Fuzzing keeps the job it is actually good at
/// — panic and hang resistance at the untrusted-bytes boundary.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct ArtifactLimits {
    /// Maximum bytes admitted from any ONE archive entry.
    ///
    /// The largest single blob a package can plausibly carry is an EMBEDDED
    /// server binary (an `MT_SERVER_BOOTSTRAP` layer). A Lambda bootstrap built
    /// from this workspace is tens of megabytes, so 512 MiB is roughly an order
    /// of magnitude of headroom over the realistic worst case while still
    /// refusing anything that could only be an attack or an accident.
    pub per_entry: u64,
    /// Maximum cumulative bytes admitted from the whole archive.
    ///
    /// A package is one binary plus a handful of small JSON/TOML/YAML layers,
    /// so the total is dominated by that single largest blob. 1 GiB leaves room
    /// for one over-large binary plus every other layer without ever admitting
    /// an archive whose sheer size is the payload.
    pub total: u64,
}

impl ArtifactLimits {
    /// The budgets every production caller uses. See the per-field rustdoc for
    /// why each number is that number.
    pub const DEFAULT: Self = Self {
        per_entry: 512 * 1024 * 1024,
        total: 1024 * 1024 * 1024,
    };
}

/// Everything the entry loop collected, before any graph reasoning.
struct RawArchive {
    index_bytes: Option<Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
    entry_count: usize,
}

/// Classify one archive entry from its HEADER, before a byte of its content is
/// read, returning a typed slot or a refusal that names the offending path.
///
/// Pure by construction: it consults the filesystem for nothing. In particular
/// the path checks run over the raw [`Component`]s rather than over a
/// canonicalized string, because canonicalizing would consult the filesystem
/// and this function must not.
///
/// `seen` accumulates the paths already admitted from this archive, so a
/// repeated path is refused rather than resolved. Two entries claiming one path
/// is an authoring bug or an attack, and last-wins merging would let a hostile
/// writer append a benign-looking entry to shadow a real one — so the refusal
/// says exactly that.
fn classify_entry(
    path: &Path,
    entry_type: tar::EntryType,
    seen: &mut BTreeSet<String>,
) -> Result<EntrySlot> {
    let text = path
        .to_str()
        .ok_or_else(|| anyhow!("refusing an archive entry whose path is not valid UTF-8"))?;

    // Entry TYPE first: a symlink or hardlink entry is a request to create a
    // named object pointing somewhere else, and an artifact carries only
    // content. `EntryType::new` already folds the legacy AREGTYPE (`b'\0'`)
    // into `Regular`, so this rejects no real archive.
    if entry_type != tar::EntryType::Regular {
        bail!(
            "refusing archive entry '{text}': only regular files are admitted, and this entry is \
             {entry_type:?}. A symlink, hardlink, directory or device entry has no meaning in a \
             content-addressed artifact."
        );
    }

    if path.is_absolute() {
        bail!(
            "refusing archive entry '{text}': the path is absolute, and an artifact's entries are \
             all relative to the archive root"
        );
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!(
                "refusing archive entry '{text}': the path contains a parent-directory ('..') \
                 component"
            );
        }
        if !matches!(component, Component::Normal(_)) {
            bail!(
                "refusing archive entry '{text}': only plain relative path components are \
                 admitted, and this path carries {component:?}"
            );
        }
    }

    let slot = if text == LAYOUT_MARKER_PATH {
        EntrySlot::LayoutMarker
    } else if text == INDEX_PATH {
        EntrySlot::Index
    } else if let Some(hex) = text.strip_prefix(BLOB_PREFIX).filter(|h| is_sha256_hex(h)) {
        EntrySlot::Blob(hex.to_string())
    } else {
        bail!(
            "refusing archive entry '{text}': an artifact carries exactly '{LAYOUT_MARKER_PATH}', \
             '{INDEX_PATH}' and '{BLOB_PREFIX}<64 lowercase hex>' at the archive ROOT — a wrapper \
             directory or any other path shape is not part of an OCI image layout"
        );
    };

    if !seen.insert(text.to_string()) {
        bail!(
            "refusing duplicate archive entry '{text}': this path appears more than once. A \
             repeated entry is refused rather than merged last-wins, because last-wins would let \
             a writer append an entry that shadows a real one."
        );
    }
    Ok(slot)
}

/// Exactly 64 lowercase ASCII hex characters — the shape a sha256 blob file
/// name has, and the shape `ManifestDigest` guarantees.
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Parse every entry of `tar_bytes` into memory, within `limits`. Touches the
/// filesystem zero times.
fn collect_entries(tar_bytes: &[u8], limits: &ArtifactLimits) -> Result<RawArchive> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = archive
        .entries()
        .context("the artifact is not a readable tar archive")?;

    let mut index_bytes: Option<Vec<u8>> = None;
    let mut blobs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut entry_count = 0usize;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut running_total = 0u64;

    for entry in entries {
        let mut entry = entry.context("read a tar entry from the artifact")?;
        let entry_type = entry.header().entry_type();
        let declared_size = entry.header().size().unwrap_or(0);
        let path = entry
            .path()
            .context("read a tar entry's path from the artifact")?
            .into_owned();
        let slot = classify_entry(&path, entry_type, &mut seen)?;
        entry_count += 1;

        // The header's declared size is ATTACKER-CONTROLLED input. It is used
        // here only as an early refusal — never as the authority for how much
        // is read. The bounded read below is what actually holds the line, per
        // the ordering rule Phase 113 recorded: collecting an over-cap body and
        // then measuring it performs exactly the allocation the cap exists to
        // prevent.
        if declared_size > limits.per_entry {
            bail!(
                "refusing archive entry '{}': its header declares {declared_size} bytes, over the \
                 per-entry cap of {} bytes",
                path.display(),
                limits.per_entry
            );
        }

        let mut buf = Vec::new();
        entry
            .by_ref()
            .take(limits.per_entry.saturating_add(1))
            .read_to_end(&mut buf)
            .with_context(|| format!("read the content of archive entry '{}'", path.display()))?;
        let actual = buf.len() as u64;
        if actual > limits.per_entry {
            bail!(
                "refusing archive entry '{}': its content exceeds the per-entry cap of {} bytes",
                path.display(),
                limits.per_entry
            );
        }
        running_total = running_total.saturating_add(actual);
        if running_total > limits.total {
            bail!(
                "refusing this artifact at archive entry '{}': its cumulative entry size exceeds \
                 the total cap of {} bytes",
                path.display(),
                limits.total
            );
        }

        match slot {
            // Accepted, content ignored: nothing in `pmcp-package` ever reads
            // this file, and `OciLayout::create` regenerates it verbatim.
            EntrySlot::LayoutMarker => {},
            EntrySlot::Index => index_bytes = Some(buf),
            EntrySlot::Blob(hex) => {
                blobs.insert(hex, buf);
            },
        }
    }

    Ok(RawArchive {
        index_bytes,
        blobs,
        entry_count,
    })
}

/// Re-derive every blob's sha256 in memory and compare it to the hex in its own
/// archive path. A substituted blob fails HERE, before any write.
fn verify_blob_integrity(blobs: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for (hex, bytes) in blobs {
        let derived = ManifestDigest::from_bytes(bytes);
        let expected = format!("sha256:{hex}");
        if derived.as_str() != expected {
            bail!(
                "blob content does not match its own name: '{BLOB_PREFIX}{hex}' hashes to {}",
                derived.as_str()
            );
        }
    }
    Ok(())
}

/// Resolve `descriptor` against the archive's blobs, cross-checking its
/// declared size and recording its media type. `role` names the descriptor in
/// any refusal so two failures never share a message.
fn resolve_descriptor<'a>(
    descriptor: &Descriptor,
    raw: &'a BTreeMap<String, Vec<u8>>,
    resolved: &mut BTreeMap<String, VerifiedBlob>,
    role: &str,
) -> Result<&'a [u8]> {
    let digest = ManifestDigest::try_from(descriptor.digest())
        .with_context(|| format!("the {role} descriptor carries an unusable digest"))?;
    let hex = digest
        .as_str()
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("the {role} descriptor is not a sha256 digest: {digest}"))?
        .to_string();

    let bytes = raw.get(&hex).ok_or_else(|| {
        anyhow!(
            "dangling descriptor: the {role} descriptor names blob sha256:{hex}, which this \
             artifact does not carry"
        )
    })?;

    let actual = bytes.len() as u64;
    if descriptor.size() != actual {
        bail!(
            "descriptor size disagreement: the {role} descriptor for sha256:{hex} declares {} \
             bytes but the blob is {actual} bytes",
            descriptor.size()
        );
    }

    resolved.entry(hex).or_insert_with(|| VerifiedBlob {
        bytes: bytes.clone(),
        media_type: descriptor.media_type().clone(),
        size: actual,
    });

    Ok(bytes)
}

/// Walk the descriptor graph from `index.json` and close it in BOTH directions.
fn resolve_graph(raw: RawArchive) -> Result<VerifiedArtifact> {
    let index_bytes = raw.index_bytes.ok_or_else(|| {
        anyhow!(
            "artifact carries no {INDEX_PATH}: an OCI image layout without its index is not a \
             package, and this reader will not synthesize an empty one"
        )
    })?;

    if raw.blobs.is_empty() {
        bail!(
            "artifact carries {INDEX_PATH} but no blobs under '{BLOB_PREFIX}' — there is nothing \
             for the index to describe"
        );
    }

    let index: ImageIndex = serde_json::from_slice(&index_bytes)
        .context("artifact's index.json does not deserialize as an OCI ImageIndex")?;

    let manifests = index.manifests();
    if manifests.len() != 1 {
        bail!(
            "expected exactly one manifest in {INDEX_PATH}, found {} — this reader mirrors \
             `read_the_one_manifest`'s rule at the framing boundary so the refusal happens \
             before any write rather than after one",
            manifests.len()
        );
    }
    let manifest_descriptor = manifests[0].clone();

    let mut resolved: BTreeMap<String, VerifiedBlob> = BTreeMap::new();
    let manifest_bytes =
        resolve_descriptor(&manifest_descriptor, &raw.blobs, &mut resolved, "manifest")?;
    let manifest: ImageManifest = serde_json::from_slice(manifest_bytes)
        .context("the artifact's manifest blob does not deserialize as an OCI ImageManifest")?;

    resolve_descriptor(manifest.config(), &raw.blobs, &mut resolved, "config")?;
    for (position, layer) in manifest.layers().iter().enumerate() {
        resolve_descriptor(
            layer,
            &raw.blobs,
            &mut resolved,
            &format!("layer[{position}]"),
        )?;
    }

    // Close the graph the other way: bytes nothing references are bytes a
    // producer smuggled in, and a reader that silently drops them is a reader
    // whose output is not a function of its input.
    for hex in raw.blobs.keys() {
        if !resolved.contains_key(hex) {
            bail!(
                "orphan blob: '{BLOB_PREFIX}{hex}' is present in the artifact but no descriptor \
                 reachable from {INDEX_PATH} references it"
            );
        }
    }

    let manifest_digest = ManifestDigest::from_bytes(manifest_bytes);

    Ok(VerifiedArtifact {
        index,
        index_bytes,
        manifest_descriptor,
        manifest,
        blobs: resolved,
        manifest_digest,
    })
}

/// How much of an artifact file may be reserved UP FRONT from its declared
/// length, before a byte has been read.
///
/// Sized to cover the realistic worst case — an embedded server binary is tens
/// of megabytes, per [`ArtifactLimits::per_entry`]'s own reasoning — in ONE
/// allocation, while still being an order of magnitude below
/// [`ArtifactLimits::total`]. The clamp is what keeps a declared length from
/// choosing an allocation as large as the cap: the length is only a hint, and
/// the bounded read is the control.
const FILE_RESERVE_CEILING: u64 = 64 * 1024 * 1024;

/// Read an artifact tar off disk into memory, refusing an over-cap file WITHOUT
/// ever buffering more than the cap.
///
/// The local counterpart of `download_artifact_bytes`' pre-read refusal, and
/// the reason it lives HERE rather than in the `package load` command: the caps
/// belong to this module (see [`ArtifactLimits`]), and a command handler that
/// reached in to read `ArtifactLimits::DEFAULT.total` for itself would be a
/// second, unfalsifiable copy of the policy — the next disk reader would make a
/// third.
///
/// # Bounded by the READ, never by a prior `metadata()`
///
/// The obvious shape — stat the path, compare `len()` against the cap, then
/// `fs::read` — does not enforce anything, and the ways it fails are not
/// exotic:
///
/// - `metadata().len()` is **0** for a FIFO and for a character device, so
///   `--input /dev/zero` (or a named pipe planted in a shared drop directory)
///   sails past the comparison and the read that follows never terminates. That
///   is exactly the unbounded allocation the check exists to prevent, so a
///   stat-based gate closes the class it is easiest to trip and leaves the class
///   an attacker would actually pick.
/// - The stat and the read are two syscalls on a PATH, so a file that grows (or
///   is swapped) in between is read at its new size regardless of what the stat
///   said.
///
/// Taking `cap + 1` bytes from an opened handle and refusing when that extra
/// byte arrives has neither hole, needs no stat at all, and bounds the buffer at
/// the cap for EVERY input shape rather than only for a static regular file.
///
/// # What `cap` measures here
///
/// This bounds the TAR FILE's bytes, while [`ArtifactLimits::total`] bounds the
/// cumulative size of the entries [`read_verified`] admits. A tar is the larger
/// of the two (a 512-byte header per entry, per-entry padding, a 1024-byte
/// trailer), so this refusal is strictly conservative and its message says
/// "artifact file" rather than naming the entry budget. `read_verified` remains
/// the authority on what an artifact may CONTAIN; this only bounds what may be
/// buffered to ask it.
///
/// # Errors
///
/// Returns `Err` when the path cannot be opened, when reading it fails, or when
/// it yields more than [`ArtifactLimits::total`] bytes.
pub fn read_artifact_file(path: &Path) -> Result<Vec<u8>> {
    read_artifact_file_with_limits(path, &ArtifactLimits::DEFAULT)
}

/// [`read_artifact_file`] with an injectable budget, so the bound is
/// FALSIFIABLE by a deterministic test rather than by producing a 1 GiB file.
///
/// # Errors
///
/// See [`read_artifact_file`].
#[doc(hidden)]
pub fn read_artifact_file_with_limits(path: &Path, limits: &ArtifactLimits) -> Result<Vec<u8>> {
    let file =
        fs::File::open(path).with_context(|| format!("open the artifact {}", path.display()))?;

    let cap = limits.total;

    // Pre-size from the OPEN HANDLE's metadata, clamped to the reserve ceiling.
    //
    // `Take` does not forward a size hint, so `read_to_end` on it starts from a
    // 32-byte probe and grows by doubling — for a large artifact that is a
    // couple of dozen reallocations, each copying the whole buffer, with both
    // the old and new allocation live across the last one. This is a HINT only:
    // the `take` below is still the bound, so a lying or absent length costs a
    // wrong-sized first allocation and nothing else. Clamped for the same reason
    // its network sibling clamps `Content-Length` — an unclamped hint lets the
    // input choose an allocation as large as the cap.
    //
    // Reading the metadata off the handle rather than the path is what keeps
    // this free of the race the stat-then-read shape has: it describes the very
    // file that is about to be read.
    let hint = file
        .metadata()
        .map(|m| m.len().min(FILE_RESERVE_CEILING))
        .unwrap_or(0);
    let mut bytes = Vec::with_capacity(usize::try_from(hint).unwrap_or(0));

    // `cap + 1` is what makes "exceeded" observable: reading exactly `cap` is
    // indistinguishable from a file that happens to be that long.
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("read the artifact {}", path.display()))?;

    if bytes.len() as u64 > cap {
        bail!(
            "artifact refused: {} exceeds the {cap}-byte artifact-file cap. Nothing was parsed \
             and nothing was written.",
            path.display()
        );
    }

    Ok(bytes)
}

/// Read an artifact tar into a fully validated in-memory model. Writes NOTHING,
/// on success or on failure, to any path.
///
/// # Errors
///
/// Returns a distinct, named refusal for each gate: an unreadable archive, an
/// entry whose path is not part of an OCI image layout, an empty archive, an
/// artifact with no `index.json`, an artifact with an index but no blobs, a
/// blob whose bytes do not hash to its own file name, an index declaring other
/// than exactly one manifest, a descriptor naming a blob the artifact does not
/// carry, a descriptor whose declared size disagrees with the blob, and a blob
/// no descriptor reaches.
pub fn read_verified(tar_bytes: &[u8]) -> Result<VerifiedArtifact> {
    read_verified_with_limits(tar_bytes, &ArtifactLimits::DEFAULT)
}

/// [`read_verified`] with injectable byte budgets — the real implementation.
///
/// Every production caller goes through [`read_verified`]; the limits are not
/// exposed on the CLI. This entry point exists so the caps are FALSIFIABLE by a
/// deterministic test rather than by a fuzz campaign (see [`ArtifactLimits`]).
///
/// Neither this function nor anything it calls performs a single `std::fs`
/// operation, on any path, on success or on failure. That is enforced
/// structurally rather than by discipline: every write lives in
/// [`write_layout`], and `write_layout` is unreachable except through an owned
/// [`VerifiedArtifact`], which only this function constructs. It is the
/// read-side mirror of `pack_server`'s "a rejected pack adds neither a blob nor
/// an index entry".
///
/// # Errors
///
/// See [`read_verified`].
#[doc(hidden)]
pub fn read_verified_with_limits(
    tar_bytes: &[u8],
    limits: &ArtifactLimits,
) -> Result<VerifiedArtifact> {
    if tar_bytes.is_empty() {
        bail!("artifact is empty (zero bytes) — there is no archive here to read");
    }

    let raw = collect_entries(tar_bytes, limits)?;
    if raw.entry_count == 0 {
        bail!("artifact archive contains no entries at all");
    }
    verify_blob_integrity(&raw.blobs)?;
    resolve_graph(raw)
}

// ---------------------------------------------------------------------
// Writing: only ever reached through an owned VerifiedArtifact
// ---------------------------------------------------------------------

/// Materialize an already-verified artifact into `staging`.
///
/// Reached only after [`read_verified`] returned `Ok`, so it validates nothing
/// of its own — the owned [`VerifiedArtifact`] IS the proof.
///
/// The parameter is named `staging` rather than `dest` so the call contract is
/// visible at every call site: this function writes a layout somewhere it is
/// safe to abandon. [`install_layout`] is what decides a layout has earned its
/// destination.
///
/// Every destination filename comes from `OciLayout::write_blob`'s digest over
/// bytes held in memory here; no archive path string reaches the filesystem.
/// Every `MediaType` handed to `write_blob` came from the descriptor that
/// referenced that blob — this function fabricates no descriptor and defaults
/// no media type.
///
/// # Errors
///
/// Returns any I/O or layout error `OciLayout::create`/`write_blob`/
/// `write_index` produces.
pub fn write_layout(artifact: &VerifiedArtifact, staging: &Path) -> Result<OciLayout> {
    let layout = OciLayout::create(staging)
        .with_context(|| format!("create a staging OCI layout at {}", staging.display()))?;
    for (hex, blob) in &artifact.blobs {
        layout
            .write_blob(blob.media_type.clone(), &blob.bytes)
            .with_context(|| format!("write blob sha256:{hex} into the staging layout"))?;
    }
    write_canonical_index(&layout, &artifact.index)
        .context("write index.json into the staging layout")?;
    Ok(layout)
}

/// Write `index.json` into `layout` with SORTED object keys.
///
/// # Why this exists, measured
///
/// `index.json` is the one file in an OCI layout that is not
/// content-addressed, and `oci_spec`'s `Descriptor::annotations` is an
/// `Option<HashMap<String, String>>`. Rust seeds `HashMap`'s hasher randomly
/// PER PROCESS, so `serde_json` emits its entries in a different order in
/// different runs. `finalize_pack` attaches exactly two annotations to the
/// manifest descriptor — `name` and `version`
/// (`crates/pmcp-package/src/oci/pack.rs:1181-1185`) — so the order flips
/// often.
///
/// Measured on the london-tube fixture across four `package save` processes:
/// the blob set was byte-identical every time, including the manifest digest
/// `sha256:afd2193b…`, and three runs emitted
/// `{"version":…,"name":…}` while the fourth emitted `{"name":…,"version":…}`.
/// Package IDENTITY was never in question; the ARTIFACT BYTES were.
///
/// Without this, `save` is not reproducible and two `load`s of one artifact
/// produce layouts that differ on disk. Both are properties this phase claims,
/// so both are enforced here rather than asserted in prose.
///
/// `canonicalize` (olpc-cjson) is the same primitive `finalize_pack` already
/// uses for the manifest BLOB; this applies the crate's own existing discipline
/// to the one file it had not been applied to.
///
/// # This is not the writer "fixing" a layout
///
/// [`write_tar`] still reads `index.json` off disk and emits it VERBATIM, and a
/// third-party artifact whose index is not canonical is still carried and still
/// loads. Normalization happens only where this code is the PRODUCER of the
/// layout — here, and in `save` over the layout it has just packed — which is
/// the same standing as `OciLayout::create` regenerating the `oci-layout`
/// marker.
///
/// The destination path is derived from the layout root and never from any
/// archive-supplied string.
///
/// # Errors
///
/// Returns an error if the index cannot be canonicalized or written.
pub fn write_canonical_index(layout: &OciLayout, index: &ImageIndex) -> Result<()> {
    let bytes = canonicalize(index).context("canonicalize index.json")?;
    let path = layout.root().join(INDEX_PATH);
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

/// Build a sibling path for `dest` by appending `suffix` to its file name.
///
/// Deliberately NOT `Path::with_extension`, which REPLACES an existing
/// extension: a destination named `london-tube.pmcp` would come back as
/// `london-tube.replaced-...`, colliding with any sibling of the same stem.
fn sibling_with_suffix(dest: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let name = dest
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name to install into", dest.display()))?;
    let mut renamed = name.to_os_string();
    renamed.push(suffix);
    Ok(parent.join(renamed))
}

/// Install a verified artifact at `dest`, transactionally.
///
/// The ordering is the guarantee, and it is fixed here rather than at the call
/// site:
///
/// 1. refuse when `dest` exists and `force` is false, BEFORE anything else;
/// 2. create a staging directory as a SIBLING of `dest`;
/// 3. write the layout into staging;
/// 4. run `validate` against the STAGING layout — any failure returns `Err`
///    with the staging guard dropping, and the destination never touched;
/// 5. only then rename into place.
///
/// # Why the staging directory is a sibling and not a temp directory
///
/// `std::fs::rename` fails `EXDEV` across filesystems, and a
/// `tempfile::TempDir` with default placement lands wherever `TMPDIR` points —
/// routinely a different filesystem from the destination. Creating staging in
/// the DESTINATION'S PARENT is what makes the final rename a same-filesystem
/// operation, and therefore what makes step 5 possible at all.
///
/// # Why `validate` is a parameter
///
/// The semantic gate — `detect_kind` plus the matching `unpack_*` — is
/// substantive validation (manifest structure, required media types, config
/// blob, legacy-shape detection, deserialization:
/// `crates/pmcp-package/src/oci/unpack.rs:639-656`) that can only run against a
/// layout that EXISTS. Running it against staging is what turns it from a
/// post-write check into a pre-write gate. It arrives as a closure because it
/// lives in the bin-only command tree and this module is a lib-mounted leaf.
///
/// # What is deliberately NOT in this gate
///
/// Attestation SUBJECT MISMATCH (D-15). A package whose bytes are sound but
/// whose claim is false is INSTALLED and then reported, exiting non-zero. Do
/// not "tidy up" by moving the subject check into the staging gate: that would
/// harmonize two verdicts Phase 122's D-03 keeps deliberately apart — integrity
/// failure means the bytes are corrupt, subject mismatch means the bytes are
/// fine and the claim is wrong.
///
/// # The residual window, named rather than claimed away
///
/// Between the two renames in step 5 there is a crash window. If the process
/// dies there, the PREVIOUS layout is intact under a `.replaced-<nanos>`
/// sibling of `dest`, and every error path on that step names that directory to
/// the operator. This is strictly smaller than writing the destination
/// directly, which can leave a marker, an empty index and a partial blob set
/// with no record that the destination had ever been complete.
///
/// # Concurrency
///
/// GUARANTEED: every blob's file name is a digest of its own content, so no
/// interleaving can produce a blob whose bytes disagree with its name. NOT
/// guaranteed: `index.json` is not content-addressed, which is why `load`
/// refuses an existing destination without `--force`. Two concurrent `--force`
/// runs into one destination are UNSUPPORTED.
///
/// # Errors
///
/// Returns an error when `dest` exists without `force`, when staging cannot be
/// created or written, when `validate` refuses, or when either rename fails.
pub fn install_layout<T>(
    artifact: &VerifiedArtifact,
    dest: &Path,
    force: bool,
    validate: impl FnOnce(&OciLayout) -> Result<T>,
) -> Result<InstalledLayout<T>> {
    if dest.exists() && !force {
        bail!(
            "{} already exists — refusing to replace it. Pass --force to replace it.",
            dest.display()
        );
    }

    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create the destination's parent {}", parent.display()))?;
    }

    let staging = tempfile::Builder::new()
        .prefix(".pmcp-load-staging-")
        .tempdir_in(parent)
        .with_context(|| {
            format!(
                "create a staging directory beside {} (staging MUST be a sibling so the final \
                 rename is same-filesystem)",
                dest.display()
            )
        })?;

    let staged_layout = write_layout(artifact, staging.path())?;
    // Any refusal here drops `staging`, which deletes it. The destination has
    // not been touched at any point up to this line.
    let unpacked = validate(&staged_layout)?;

    let staged_path = staging.keep();
    let installed = install_staged(&staged_path, dest);
    if installed.is_err() {
        let _ = fs::remove_dir_all(&staged_path);
    }
    installed?;

    Ok(InstalledLayout {
        layout: OciLayout::open(dest),
        unpacked,
    })
}

/// Step 5 of [`install_layout`]: move `staged_path` onto `dest`, displacing an
/// existing destination through a named `.replaced-<nanos>` sibling first.
fn install_staged(staged_path: &Path, dest: &Path) -> Result<()> {
    let replaced = if dest.exists() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let holding = sibling_with_suffix(dest, &format!(".replaced-{nanos}"))?;
        fs::rename(dest, &holding).with_context(|| {
            format!(
                "move the existing layout at {} aside before installing",
                dest.display()
            )
        })?;
        Some(holding)
    } else {
        None
    };

    match fs::rename(staged_path, dest) {
        Ok(()) => {
            if let Some(holding) = replaced {
                fs::remove_dir_all(&holding).with_context(|| {
                    format!("remove the replaced layout at {}", holding.display())
                })?;
            }
            Ok(())
        },
        Err(install_error) => {
            let Some(holding) = replaced else {
                return Err(install_error)
                    .with_context(|| format!("install the layout at {}", dest.display()));
            };
            match fs::rename(&holding, dest) {
                Ok(()) => Err(anyhow!(install_error)).with_context(|| {
                    format!(
                        "install the layout at {} (the previous layout was restored)",
                        dest.display()
                    )
                }),
                Err(restore_error) => Err(anyhow!(
                    "failed to install the layout at {dest} ({install_error}) AND failed to \
                     restore the previous one ({restore_error}). The previous layout is intact \
                     at {holding} — move it back by hand.",
                    dest = dest.display(),
                    holding = holding.display()
                )),
            }
        },
    }
}

// ---------------------------------------------------------------------
// Writing the movable form
// ---------------------------------------------------------------------

/// Tar `layout` to `dest` as the movable form of the package (D-11).
///
/// Reads `oci-layout`, `index.json` and each `blobs/sha256/<hex>` off disk
/// VERBATIM and re-serializes nothing. A writer that re-serialized `index.json`
/// could silently "fix" a malformed layout, which would make byte-exact
/// writer-conformance untestable.
///
/// The output is REPRODUCIBLE: entry order is fixed (`oci-layout`, then
/// `index.json`, then every blob sorted lexicographically by hex) and every
/// header is normalized (mtime 0, uid/gid 0, empty user/group name, mode 0644,
/// regular-file type, ustar so no PAX/GNU extension record is emitted).
///
/// Emitting the `oci-layout` marker is deliberate (RESEARCH Pitfall 7): a plain
/// `tar -xf` then yields a valid layout for a human debugging by hand, while
/// readers accept it, ignore its content, and always regenerate it through
/// `OciLayout::create`.
///
/// # Errors
///
/// Returns an error if the layout cannot be read, if `blobs/sha256/` carries a
/// file whose name is not a sha256 hex digest, or if `dest` cannot be written.
pub fn write_tar(layout: &OciLayout, dest: &Path) -> Result<()> {
    let root = layout.root();
    let mut builder = tar::Builder::new(Vec::new());

    let marker = root.join(LAYOUT_MARKER_PATH);
    if marker.exists() {
        let bytes = fs::read(&marker).with_context(|| format!("read {}", marker.display()))?;
        append_normalized(&mut builder, LAYOUT_MARKER_PATH, &bytes)?;
    }

    let index_path = root.join(INDEX_PATH);
    let index_bytes =
        fs::read(&index_path).with_context(|| format!("read {}", index_path.display()))?;
    append_normalized(&mut builder, INDEX_PATH, &index_bytes)?;

    for hex in sorted_blob_hexes(root)? {
        let blob_path = root.join("blobs").join("sha256").join(&hex);
        let bytes =
            fs::read(&blob_path).with_context(|| format!("read {}", blob_path.display()))?;
        append_normalized(&mut builder, &format!("{BLOB_PREFIX}{hex}"), &bytes)?;
    }

    let bytes = builder
        .into_inner()
        .context("finish writing the artifact archive")?;
    fs::write(dest, bytes).with_context(|| format!("write the artifact to {}", dest.display()))
}

/// Every blob file name under `blobs/sha256/`, sorted — the deterministic entry
/// order [`write_tar`] depends on, since `read_dir` order is not defined.
fn sorted_blob_hexes(root: &Path) -> Result<Vec<String>> {
    let dir = root.join("blobs").join("sha256");
    let mut hexes = Vec::new();
    let entries =
        fs::read_dir(&dir).with_context(|| format!("read the blob directory {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("read an entry of {}", dir.display()))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            anyhow!(
                "the blob directory {} carries a file whose name is not valid UTF-8",
                dir.display()
            )
        })?;
        if !is_sha256_hex(name) {
            bail!(
                "refusing to tar {}: '{name}' is not a sha256 blob file name, so this is not a \
                 layout this writer can carry verbatim",
                dir.display()
            );
        }
        hexes.push(name.to_string());
    }
    hexes.sort();
    Ok(hexes)
}

/// Append one entry under a fully normalized ustar header, so two runs over
/// identical inputs produce byte-identical archives.
fn append_normalized(builder: &mut tar::Builder<Vec<u8>>, path: &str, bytes: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_ustar();
    header
        .set_path(path)
        .with_context(|| format!("set the archive path '{path}'"))?;
    header.set_size(bytes.len() as u64);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header
        .set_username("")
        .context("normalize the archive entry's user name")?;
    header
        .set_groupname("")
        .context("normalize the archive entry's group name")?;
    header.set_cksum();
    builder
        .append(&header, bytes)
        .with_context(|| format!("append '{path}' to the artifact archive"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::image::{ImageIndexBuilder, ImageManifestBuilder, SCHEMA_VERSION};
    use pmcp_package::oci::media_types::{EMPTY_CONFIG_BLOB, MT_EMPTY_CONFIG};
    use proptest::prelude::*;

    /// Classify `path` as a regular file with a fresh duplicate-tracking set.
    fn classify(path: &str) -> Result<EntrySlot> {
        classify_entry(
            Path::new(path),
            tar::EntryType::Regular,
            &mut BTreeSet::new(),
        )
    }

    // -----------------------------------------------------------------
    // The framing gates
    // -----------------------------------------------------------------

    #[test]
    fn classify_entry_accepts_the_three_layout_slots() {
        assert_eq!(classify("oci-layout").unwrap(), EntrySlot::LayoutMarker);
        assert_eq!(classify("index.json").unwrap(), EntrySlot::Index);
        let hex = "a".repeat(64);
        assert_eq!(
            classify(&format!("blobs/sha256/{hex}")).unwrap(),
            EntrySlot::Blob(hex)
        );
    }

    #[test]
    fn classify_entry_refuses_a_wrapper_directory() {
        let err = classify("package/index.json").unwrap_err();
        assert!(
            err.to_string().contains("archive ROOT"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_a_parent_directory_component() {
        let err = classify("../index.json").unwrap_err();
        assert!(
            err.to_string().contains("parent-directory"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_an_absolute_path() {
        let err = classify("/index.json").unwrap_err();
        assert!(
            err.to_string().contains("absolute"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_every_non_regular_entry_type() {
        for entry_type in [
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Directory,
            tar::EntryType::Fifo,
            tar::EntryType::Char,
            tar::EntryType::Block,
        ] {
            let err = classify_entry(Path::new("index.json"), entry_type, &mut BTreeSet::new())
                .unwrap_err();
            assert!(
                err.to_string().contains("only regular files are admitted"),
                "{entry_type:?} must be refused by type: {err}"
            );
        }
    }

    #[test]
    fn classify_entry_refuses_a_repeated_path_rather_than_merging_it() {
        let mut seen = BTreeSet::new();
        classify_entry(Path::new("index.json"), tar::EntryType::Regular, &mut seen).unwrap();
        let err = classify_entry(Path::new("index.json"), tar::EntryType::Regular, &mut seen)
            .unwrap_err();
        assert!(
            err.to_string().contains("duplicate archive entry"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_uppercase_and_short_blob_hex() {
        for bad in [
            format!("blobs/sha256/{}", "A".repeat(64)),
            format!("blobs/sha256/{}", "a".repeat(63)),
        ] {
            assert!(classify(&bad).is_err(), "{bad} must be refused");
        }
    }

    #[test]
    fn read_verified_refuses_a_zero_byte_artifact() {
        let err = read_verified(&[]).unwrap_err();
        assert!(
            err.to_string().contains("zero bytes"),
            "unexpected message: {err}"
        );
    }

    // -----------------------------------------------------------------
    // The byte caps, proven by falsification PAIRS
    // -----------------------------------------------------------------

    /// The smallest artifact that passes every READ-side gate: an index naming
    /// one manifest, that manifest, and the standard empty-config blob it
    /// references. The descriptor graph closes in both directions.
    ///
    /// It is deliberately not a loadable SERVER — `read_verified` answers
    /// framing, integrity and graph closure, and knows nothing about package
    /// semantics. That separation is what lets this fixture stay three blobs
    /// long.
    fn minimal_valid_artifact() -> Vec<u8> {
        let layout_dir = tempfile::tempdir().expect("create a layout dir");
        let out_dir = tempfile::tempdir().expect("create an output dir");
        let layout = OciLayout::create(layout_dir.path()).expect("create the layout");

        let config = layout
            .write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)
            .expect("write the config blob");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type(MediaType::ImageManifest)
            .config(config)
            .layers(Vec::<Descriptor>::new())
            .build()
            .expect("build the manifest");
        let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize the manifest");
        let manifest_descriptor = layout
            .write_manifest(&manifest_bytes)
            .expect("write the manifest blob");
        let index = ImageIndexBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .manifests(vec![manifest_descriptor])
            .build()
            .expect("build the index");
        write_canonical_index(&layout, &index).expect("write index.json");

        let tar_path = out_dir.path().join("artifact.tar");
        write_tar(&layout, &tar_path).expect("tar the layout");
        std::fs::read(&tar_path).expect("read the artifact")
    }

    // -----------------------------------------------------------------
    // The pre-read file bound (`read_artifact_file`)
    // -----------------------------------------------------------------

    /// Budgets small enough to trip on a handful of bytes, so the file bound is
    /// falsifiable without producing a gibibyte.
    fn tiny_file_limits(total: u64) -> ArtifactLimits {
        ArtifactLimits {
            per_entry: ArtifactLimits::DEFAULT.per_entry,
            total,
        }
    }

    /// A file over the cap is refused, and the refusal names the cap and the
    /// path — a bound that refuses anonymously is one nobody can diagnose.
    #[test]
    fn a_file_over_the_cap_is_refused_naming_the_cap_and_the_path() {
        let dir = tempfile::tempdir().expect("scratch dir");
        let path = dir.path().join("oversized.tar");
        std::fs::write(&path, vec![0u8; 64]).expect("write the oversized file");

        let err = read_artifact_file_with_limits(&path, &tiny_file_limits(16)).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("16-byte artifact-file cap"),
            "the refusal must name the cap: {message}"
        );
        assert!(
            message.contains("oversized.tar"),
            "the refusal must name the file: {message}"
        );
    }

    /// The boundary is INCLUSIVE: a file of exactly `total` bytes is admitted.
    /// An off-by-one here would refuse a legitimate artifact sitting on the cap.
    #[test]
    fn a_file_of_exactly_the_cap_is_admitted_whole() {
        let dir = tempfile::tempdir().expect("scratch dir");
        let path = dir.path().join("at-cap.tar");
        std::fs::write(&path, vec![7u8; 16]).expect("write the at-cap file");

        let bytes =
            read_artifact_file_with_limits(&path, &tiny_file_limits(16)).expect("cap is inclusive");
        assert_eq!(bytes, vec![7u8; 16], "the whole file must come back");
    }

    /// One byte over is refused — the pair that makes the previous test mean
    /// something rather than merely pass.
    #[test]
    fn one_byte_over_the_cap_is_refused() {
        let dir = tempfile::tempdir().expect("scratch dir");
        let path = dir.path().join("one-over.tar");
        std::fs::write(&path, vec![7u8; 17]).expect("write the one-over file");

        assert!(
            read_artifact_file_with_limits(&path, &tiny_file_limits(16)).is_err(),
            "cap + 1 bytes must be refused"
        );
    }

    /// The bound is enforced by the READ, not by a prior `metadata()` stat.
    ///
    /// This is the test that distinguishes the two designs. `/dev/zero` reports
    /// `metadata().len() == 0`, so a stat-then-read gate compares 0 against the
    /// cap, passes, and then reads forever — the process dies with no refusal
    /// and no output. A FIFO behaves the same way. Because the budget is applied
    /// to the read itself, the endless input is refused in bounded time and
    /// bounded memory instead.
    ///
    /// The tiny cap is what keeps this a unit test: it bounds the buffer at 16
    /// bytes, so a regression does not hang CI at a gibibyte before failing.
    #[cfg(unix)]
    #[test]
    fn an_endless_input_is_refused_by_the_read_bound_not_by_a_stat() {
        let path = Path::new("/dev/zero");
        assert_eq!(
            fs::metadata(path).expect("stat /dev/zero").len(),
            0,
            "the premise: a stat-based gate would see 0 here and let this through"
        );

        let err = read_artifact_file_with_limits(path, &tiny_file_limits(16)).unwrap_err();
        assert!(
            err.to_string().contains("artifact-file cap"),
            "an endless input must trip the file cap: {err}"
        );
    }

    /// A missing path fails with a named refusal rather than a panic, and says
    /// which path it could not open.
    #[test]
    fn a_missing_artifact_path_is_a_named_refusal() {
        let dir = tempfile::tempdir().expect("scratch dir");
        let path = dir.path().join("absent.tar");

        let err = read_artifact_file(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("absent.tar"),
            "the refusal must name the path: {err:#}"
        );
    }

    proptest! {
        /// Any file at or under the cap round-trips byte-for-byte, and any file
        /// over it is refused. The bound never truncates silently — returning a
        /// clipped prefix as if it were the whole artifact would hand
        /// `read_verified` a tar that is not the one on disk.
        #[test]
        fn the_file_bound_either_returns_every_byte_or_refuses(
            content in proptest::collection::vec(any::<u8>(), 0..256usize),
            cap in 1u64..256,
        ) {
            let dir = tempfile::tempdir().expect("scratch dir");
            let path = dir.path().join("prop.tar");
            std::fs::write(&path, &content).expect("write the fixture");

            match read_artifact_file_with_limits(&path, &tiny_file_limits(cap)) {
                Ok(bytes) => {
                    prop_assert!(content.len() as u64 <= cap, "admitted an over-cap file");
                    prop_assert_eq!(bytes, content, "admitted bytes must be the file's bytes");
                },
                Err(_) => prop_assert!(content.len() as u64 > cap, "refused an at-cap file"),
            }
        }
    }

    /// Falsification pair 1a: under a tiny PER-ENTRY cap the artifact is
    /// refused, and the refusal names that cap and the entry it tripped on.
    #[test]
    fn a_tiny_per_entry_cap_refuses_an_artifact_naming_the_cap_and_the_entry() {
        let artifact = minimal_valid_artifact();
        let limits = ArtifactLimits {
            per_entry: 4,
            total: ArtifactLimits::DEFAULT.total,
        };
        let err = read_verified_with_limits(&artifact, &limits).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("per-entry cap of 4 bytes"),
            "the refusal must name the cap: {message}"
        );
        // Whichever entry it trips on first, the refusal must NAME it — a cap
        // that refuses anonymously is a cap nobody can diagnose. The layout
        // marker is emitted first, so under a 4-byte cap that is the entry the
        // reader reaches first.
        assert!(
            message.contains(LAYOUT_MARKER_PATH)
                || message.contains(INDEX_PATH)
                || message.contains(BLOB_PREFIX),
            "the refusal must name the entry: {message}"
        );
    }

    /// Falsification pair 1b: the SAME artifact under a large per-entry cap is
    /// accepted. Without this half, 1a only says "an error happened".
    #[test]
    fn a_large_per_entry_cap_accepts_the_same_artifact() {
        let artifact = minimal_valid_artifact();
        let limits = ArtifactLimits {
            per_entry: 1 << 20,
            total: ArtifactLimits::DEFAULT.total,
        };
        assert!(
            read_verified_with_limits(&artifact, &limits).is_ok(),
            "the artifact 1a refused must be accepted once the per-entry cap is raised"
        );
    }

    /// Falsification pair 2a: under a tiny TOTAL cap the artifact is refused,
    /// naming that cap.
    #[test]
    fn a_tiny_total_cap_refuses_an_artifact_naming_the_total_cap() {
        let artifact = minimal_valid_artifact();
        let limits = ArtifactLimits {
            per_entry: ArtifactLimits::DEFAULT.per_entry,
            total: 8,
        };
        let err = read_verified_with_limits(&artifact, &limits).unwrap_err();
        assert!(
            err.to_string().contains("total cap of 8 bytes"),
            "the refusal must name the total cap: {err}"
        );
    }

    /// Falsification pair 2b: the same artifact under a large total cap is
    /// accepted.
    #[test]
    fn a_large_total_cap_accepts_the_same_artifact() {
        let artifact = minimal_valid_artifact();
        let limits = ArtifactLimits {
            per_entry: ArtifactLimits::DEFAULT.per_entry,
            total: 1 << 20,
        };
        assert!(
            read_verified_with_limits(&artifact, &limits).is_ok(),
            "the artifact 2a refused must be accepted once the total cap is raised"
        );
    }

    /// A header that LIES about its size is refused without reading a body —
    /// the case the per-entry cap exists for, since the declared size is
    /// attacker-controlled and reading first would perform the very allocation
    /// the cap prevents.
    #[test]
    fn a_lying_oversized_header_is_refused_without_a_large_allocation() {
        let mut header = tar::Header::new_ustar();
        header.set_path("index.json").unwrap();
        header.set_size(u64::MAX / 2);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        // The BODY is tiny; only the header claims otherwise.
        builder.append(&header, &b"{}"[..]).unwrap();
        let bytes = builder.into_inner().unwrap();

        let err = read_verified(&bytes).unwrap_err();
        assert!(
            err.to_string().contains("per-entry cap"),
            "unexpected message: {err}"
        );
    }

    // -----------------------------------------------------------------
    // Where staging lives, captured rather than assumed
    // -----------------------------------------------------------------

    /// The semantic gate runs against a staging directory that is a SIBLING of
    /// the destination, never against the destination itself and never under
    /// `TMPDIR`.
    ///
    /// This is the "captured trace" form of the check rather than the
    /// different-filesystem form: mounting a second filesystem is not
    /// arrangeable in a unit test, and the property that actually matters —
    /// `staging.parent() == dest.parent()`, which is what makes the final
    /// rename same-filesystem and therefore not `EXDEV` — is directly
    /// observable from inside the validation closure, which is handed the
    /// staged layout. Asserting it here is exact; asserting it through
    /// `TMPDIR` would be circumstantial.
    #[test]
    fn install_layout_stages_in_the_destinations_parent_and_validates_there() {
        let artifact = read_verified(&minimal_valid_artifact()).expect("the fixture is valid");
        let home = tempfile::tempdir().expect("create a destination parent");
        let dest = home.path().join("layout");

        let observed = std::cell::RefCell::new(PathBuf::new());
        let installed = install_layout(&artifact, &dest, false, |layout| {
            observed.replace(layout.root().to_path_buf());
            // The destination must not exist while validation is running.
            assert!(
                !dest.exists(),
                "the destination must not be created before the semantic gate has passed"
            );
            Ok(())
        })
        .expect("install the artifact");

        let staging = observed.into_inner();
        assert_ne!(
            staging, dest,
            "validation must run against staging, not against the destination"
        );
        assert_eq!(
            staging.parent(),
            dest.parent(),
            "staging must be a SIBLING of the destination, or the final rename would cross \
             filesystems and fail EXDEV"
        );
        assert_eq!(
            installed.layout.root(),
            dest,
            "the installed layout is opened at the destination"
        );
        assert!(
            dest.is_dir(),
            "the destination exists after a successful install"
        );
        assert!(
            !staging.exists(),
            "the staging directory is consumed by the rename and must not linger"
        );
    }

    // -----------------------------------------------------------------
    // Properties
    // -----------------------------------------------------------------

    proptest! {
        /// PROPERTY: any path `classify_entry` ACCEPTS, joined onto a
        /// destination root, stays inside that root. This is the no-escape
        /// property stated over generated paths rather than asserted in prose.
        #[test]
        fn an_accepted_path_never_escapes_the_destination_root(candidate in ".*") {
            if classify(&candidate).is_ok() {
                let root = Path::new("/destination/root");
                let joined = root.join(&candidate);
                prop_assert!(
                    joined.starts_with(root),
                    "accepted path {candidate:?} escaped to {joined:?}"
                );
            }
        }

        /// PROPERTY (the complement, without which the one above is satisfied
        /// by a `classify_entry` that accepts nothing): a path carrying a
        /// parent-directory component or a leading separator is ALWAYS refused.
        #[test]
        fn a_traversing_path_is_always_refused(
            prefix in prop_oneof![Just("../"), Just("/"), Just("./../")],
            tail in "[a-z0-9./-]{0,40}",
        ) {
            let candidate = format!("{prefix}{tail}");
            prop_assert!(
                classify(&candidate).is_err(),
                "{candidate:?} must be refused"
            );
        }

        /// PROPERTY: the untrusted-bytes boundary is TOTAL — `read_verified`
        /// returns `Ok` or `Err` on arbitrary bytes and never unwinds. This is
        /// the same boundary a fuzz target campaigns; having the property here
        /// first is what makes that target's invariant meaningful.
        #[test]
        fn read_verified_never_panics_on_arbitrary_bytes(
            bytes in proptest::collection::vec(any::<u8>(), 0..4096)
        ) {
            let _ = read_verified(&bytes);
        }
    }
}
