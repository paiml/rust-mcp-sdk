//! Local OCI Image Layout pack/unpack (the titular scope): serialize
//! each of the four package types into an OCI Image Layout on disk with
//! custom `application/vnd.pmcp.*` media types per layer, content-address
//! each blob by sha256, and verify every blob's digest before deserializing
//!. Pure local disk I/O — no network, no `oci-client` — so
//! the registry push/pull can consume these exact `oci_spec::image`
//! types with zero translation.
//!
//! - [`config_validation`] — pack-time validation of a config server's
//!   `[[config_slots]]` declaration block against the package's slot list.
//! - [`media_types`] — vendor media-type constants per layer + the standard
//!   OCI empty-config blob constants.
//! - [`layout`] — [`OciLayout`], the local Image Layout directory
//!   reader/writer (`oci-layout` + `index.json` + `blobs/sha256/<hex>`).
//! - [`pack`] — `pack_server`/`pack_agent`/`pack_team`/`pack_workflow`.
//! - [`unpack`] — `unpack_server`/`unpack_agent`/`unpack_team`/`unpack_workflow`.
//!
//! # Artifact tar framing
//!
//! A package has two on-disk forms: the layout DIRECTORY described above, which
//! is the identity-bearing working form, and an uncompressed `.tar` of that
//! directory, which is the movable carriage envelope. This section is the
//! NORMATIVE rule for that tar.
//!
//! It is addressed to TWO implementers — this SDK and the pmcp.run platform —
//! and it is not a description of what any one of them happens to do today. The
//! platform PRODUCES the tar that `cargo pmcp package pull` consumes, and the
//! SDK produces the tar `cargo pmcp package save` emits, so this is the one
//! artifact shape the two sides must agree on byte-for-byte. Where the
//! platform's own handoff document (`docs/design/package-portability-pmcp-run-handoff.md`
//! §5.1, *"a tar of `index.json` + `blobs/`"*) is silent, this section states an
//! answer and labels it as an SDK assumption awaiting confirmation rather than
//! leaving the next implementer to guess.
//!
//! ## Entry inventory
//!
//! Exactly three entry shapes are legal, all at the archive ROOT:
//!
//! 1. the layout marker file, `oci-layout`;
//! 2. `index.json`;
//! 3. `blobs/sha256/<hex>`, where `<hex>` is exactly 64 lowercase hexadecimal
//!    characters.
//!
//! Nothing else. An unrecognized path shape is a REFUSAL, never an entry to
//! skip silently. Reason: a reader that skips what it does not recognize has an
//! output that is not a function of its input, and gives a producer a channel
//! for bytes no reader accounts for.
//!
//! ## No wrapper directory
//!
//! `index.json` is at the archive root. A leading component — a package name, a
//! `./` prefix, an `oci/` folder — is a refusal. Reason: the reader must be able
//! to identify an entry from its path alone, without first guessing which prefix
//! a producer chose to wrap the layout in.
//!
//! ## No absolute paths, no parent-directory components
//!
//! An entry path that is absolute, or that carries a `..` component, is a
//! refusal. This is the zip-slip defence, stated at the CONTRACT level rather
//! than left to one implementation: untarring attacker-controlled bytes is a
//! path-traversal surface, and an artifact arrives from a network the handoff
//! itself says is never trusted.
//!
//! The SDK's own reader goes further, and any implementer is encouraged to do
//! the same: it never writes an archive-supplied path at all. Entries are parsed
//! into memory, gated, and then written through [`OciLayout::write_blob`], whose
//! destination is derived from a digest THIS code computed over bytes it is
//! holding. An archive path is a lookup key during validation and never a
//! filesystem destination, which makes traversal unrepresentable rather than
//! filtered — and a filter is only ever the list of traversals someone thought
//! of. That stronger construction is a recommendation; the path prohibition
//! above stays NORMATIVE for implementers who do join archive paths onto disk.
//!
//! ## Regular files only
//!
//! Every entry's type flag must be regular file. Symlinks, hardlinks, directory
//! entries, fifos and device entries are all refusals. Reason: a symlink or
//! hardlink is an escape primitive — a request to create a named object pointing
//! somewhere the archive does not contain — and a directory entry is
//! unnecessary, because the three legal paths above already imply the only
//! structure a layout has.
//!
//! ## No duplicate paths
//!
//! A path appearing more than once is a refusal, never last-wins. Reason: two
//! entries claiming one path is an authoring bug or an attack, and last-wins
//! merging lets a hostile writer append a benign-looking entry that shadows a
//! real one after the real one has already been inspected.
//!
//! ## Uncompressed
//!
//! The artifact is a PLAIN tar. It is not gzipped, zstd-compressed or otherwise
//! wrapped. Reason: §5.1 names no compression, and adding one buys nothing while
//! creating a decompression-bomb surface over untrusted bytes.
//!
//! **This is an SDK ASSUMPTION awaiting platform confirmation, not a settled
//! fact about the platform.** It is written as an explicit open question in the
//! vendored SDL at `contracts/pmcp-run/portability-v1.graphql` (on
//! `getPackageArtifact`'s `downloadUrl` field), which records it as the single
//! highest-value thing on that page to confirm: if the platform gzips, `pull`
//! refuses every real artifact AND `save` produces a shape the platform cannot
//! read.
//!
//! ## Reproducible headers
//!
//! Entry headers are NORMALIZED and entry ORDER is fixed.
//!
//! - Normalized headers: mtime `0`, uid `0`, gid `0`, empty user name, empty
//!   group name, a fixed regular-file mode (`0o644`), and a `ustar` header so no
//!   PAX (`x`/`g`) or GNU long-name (`L`/`K`) extension record is emitted.
//! - Fixed order: the layout marker first, then `index.json`, then every blob
//!   sorted lexicographically by its hex digest. Directory-iteration order is
//!   not defined by any filesystem, so a writer must sort rather than emit what
//!   `read_dir` handed it.
//!
//! Consequence, which is the reason the rule exists: packing the same layout
//! twice yields BYTE-IDENTICAL archives. The artifact can therefore be compared,
//! cached and digested by transport machinery without spurious differences, and
//! a fixture can pin a writer's output exactly.
//!
//! ## The layout marker
//!
//! §5.1 is silent about `oci-layout`, so the rule states it. MEASURED fact that
//! makes the answer safe: NOTHING in this crate ever READS the marker file — a
//! grep of `crates/pmcp-package/src/` finds only the write in
//! [`layout::OciLayout::create`], doc comments, and one `#[cfg(test)]`
//! read-back.
//!
//! Therefore:
//!
//! - EMIT it on write. A plain `tar -xf` of the artifact then yields a valid OCI
//!   image layout for a human debugging by hand.
//! - ACCEPT it on read, IGNORE its content, and always REGENERATE it through
//!   [`layout::OciLayout::create`].
//! - Do NOT REQUIRE it on read. An implementation that omits the marker still
//!   interoperates, which is what keeps this rule from breaking a producer that
//!   read §5.1 literally.
//!
//! **Also an SDK assumption awaiting platform confirmation:** whether the
//! platform's own reader TOLERATES the extra marker entry alongside `index.json`
//! and `blobs/`. If its untar is strict about its entry inventory, an extra
//! entry is a refusal. That question is likewise written into
//! `contracts/pmcp-run/portability-v1.graphql`.
//!
//! ## Fixture provenance — the rule that makes this enforceable
//!
//! The worked examples of every rule above live at
//! `crates/pmcp-package/tests/golden_fixtures/artifact_tar_v1/`: one conformant
//! tar (shipped as a pair with `conformant.layout/`, the unpacked directory it
//! was authored from) and one hostile sibling per violation.
//!
//! **Those fixtures are CHECKED-IN BYTES and are NEVER REGENERATED FROM THE
//! WRITER UNDER TEST.** A fixture produced by the code it tests is a tautology:
//! it agrees with that code by construction and therefore cannot detect the
//! drift it exists to detect. This is the provenance property the platform and
//! the SDK adopted jointly (handoff §3.2) in place of the earlier question of
//! where the corpus should live.
//!
//! ## The boundary this section deliberately does not cross
//!
//! This crate holds the RULE. The CODEC that implements it —
//! `cargo-pmcp/src/commands/package/artifact.rs`, the only module in the
//! repository that names `tar::` — lives in `cargo-pmcp`.
//!
//! That split is deliberate and must not be "tidied up" by moving the reader in.
//! It exists so that no archive dependency enters this crate's resolved
//! dependency graph, which is machine-enforced by the crate-local
//! `[bans].allow` allowlist in `crates/pmcp-package/deny.toml` (Phase 122). The
//! friction of that gate is the feature: adding `tar` here would be a
//! supply-chain decision disguised as a refactor.

pub mod config_validation;
pub mod layout;
pub mod media_types;
pub mod pack;
pub mod unpack;

pub use config_validation::{
    parse_declared_config_slots, validate_config_slot_agreement, validate_config_slot_placeholders,
    validate_no_undeclared_env_refs, DeclaredConfigSlot,
};
pub use layout::OciLayout;
pub use pack::{
    pack_agent, pack_server, pack_team, pack_workflow, AttestationFile, BinaryMode, ConfigFile,
    OpenApiSpecFile,
};
pub use unpack::{
    unpack_agent, unpack_server, unpack_team, unpack_workflow, RestoredFile, SubjectVerdict,
    UnpackedAttestation, UnpackedBinary, UnpackedServer, UnpackedTeam,
};

use crate::oci::media_types::{
    ARTIFACT_TYPE_AGENT, ARTIFACT_TYPE_TEAM, ARTIFACT_TYPE_WORKFLOW, MT_AGENT_CONFIG,
    MT_TEAM_CONFIG, MT_WORKFLOW_MANIFEST,
};
use crate::package::{AgentPackage, TeamPackage, WorkflowManifest};

/// The three single-layer package kinds (agent/team/workflow) share ONE
/// pack/unpack path: serialize to a single canonical-JSON config layer under a
/// vendor media type, wrapped in a manifest carrying the kind's `artifactType`.
/// This trait is the single source of truth binding each kind to its
/// media-type / artifact-type / layer-name constants, so
/// [`pack::pack_single_layer`] and [`unpack::unpack_single_layer`] are fully
/// generic over it (no per-kind copy-paste). `ServerPackage` is deliberately
/// NOT a member — it is multi-layer (bootstrap + envelope + 4 typed sections)
/// and keeps its own bespoke pack/unpack path.
pub(crate) trait SingleLayerPackage: serde::Serialize + serde::de::DeserializeOwned {
    /// Vendor media type for this kind's single config layer.
    const LAYER_MEDIA_TYPE: &'static str;
    /// OCI `artifactType` recorded on the manifest.
    const ARTIFACT_TYPE: &'static str;
    /// Human-readable layer name used in "missing layer" errors.
    const LAYER_NAME: &'static str;
    /// The package's declared name (used for `index.json` annotations).
    fn name(&self) -> &str;
    /// The package's declared version (used for `index.json` annotations).
    fn version(&self) -> &semver::Version;
}

impl SingleLayerPackage for AgentPackage {
    const LAYER_MEDIA_TYPE: &'static str = MT_AGENT_CONFIG;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_AGENT;
    const LAYER_NAME: &'static str = "agent-config";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}

impl SingleLayerPackage for TeamPackage {
    const LAYER_MEDIA_TYPE: &'static str = MT_TEAM_CONFIG;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_TEAM;
    const LAYER_NAME: &'static str = "team-config";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}

impl SingleLayerPackage for WorkflowManifest {
    const LAYER_MEDIA_TYPE: &'static str = MT_WORKFLOW_MANIFEST;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_WORKFLOW;
    const LAYER_NAME: &'static str = "workflow-manifest";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}
