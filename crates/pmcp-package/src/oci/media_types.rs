//! Vendor `application/vnd.pmcp.*` media-type constants (RESEARCH Pattern 3)
//! for every OCI layer this crate packs, plus the standard OCI empty-config
//! blob constants (OCI 1.1's non-null-config-descriptor requirement).
//!
//! # Layer inventory
//!
//! - `ServerPackage` decomposes into content-addressed layers. Exactly ONE
//!   binary layer is always present, and it is one of two mutually exclusive
//!   shapes: the embedded bootstrap binary ([`MT_SERVER_BOOTSTRAP`] — supplied
//!   as raw bytes, never a struct field) OR a binary REFERENCE
//!   ([`MT_SERVER_BINARY_REF`] — a small JSON layer naming the digest and
//!   media type of a binary the target environment resolves for itself; a
//!   pure-config Shape A server carries this one and no bytes at all).
//!   Alongside it are the four typed sections — the deploy descriptor
//!   ([`MT_SERVER_DEPLOY_DESCRIPTOR`]), the cedar policy set
//!   ([`MT_SERVER_CEDAR_POLICY_SET`]), tool metadata
//!   ([`MT_SERVER_TOOL_METADATA`]), and the declared config slots
//!   ([`MT_SERVER_CONFIG_SLOTS`]) — plus the `name`/`version`/top-level
//!   `digest` "envelope" layer ([`MT_SERVER_ENVELOPE`]) so every remaining
//!   field round-trips losslessly by plain serialize/deserialize.
//! - Three OPTIONAL vendor-content layers may follow: the author's verbatim
//!   server config file ([`MT_SERVER_CONFIG`]), for an OpenAPI-backed server
//!   its spec file ([`MT_SERVER_OPENAPI_SPEC`]), and a platform-issued
//!   attestation ([`MT_ATTESTATION`]). All three carry raw bytes (never
//!   re-derived from a parsed struct); the two file layers record the
//!   original file name in their descriptor's
//!   `org.opencontainers.image.title` annotation, and the attestation records
//!   its subject/issuer/payload-type in its own descriptor's annotations. Any
//!   of them may be absent, and absence is exactly the layer NOT being in the
//!   manifest — there is no absence marker (D-14). An absent
//!   [`MT_SERVER_OPENAPI_SPEC`] layer is the author's declaration of a
//!   curated-only server, mirroring `pmcp-openapi-server`'s
//!   `--spec: Option<PathBuf>`, never a silent drop of a spec that was
//!   supplied; an absent [`MT_ATTESTATION`] layer means the package is simply
//!   unattested.
//!
//!   An attested package carries TWO digests, and that is by design (D-01):
//!   the attestation names the UNATTESTED manifest digest as its subject,
//!   while the package that carries the attestation necessarily hashes to
//!   something else — the attestation layer and its annotations are inside
//!   the manifest the digest covers. An attested package's own digest can
//!   therefore never equal the subject it names.
//! - Layers are located at unpack time by MEDIA TYPE, never by position — the
//!   optional layers make any positional contract false.
//! - `AgentPackage` and `WorkflowManifest` each pack as STRICTLY ONE JSON layer
//!   ([`MT_AGENT_CONFIG`]/[`MT_WORKFLOW_MANIFEST`]) — the whole struct
//!   serialized once; no decomposition needed since (unlike `ServerPackage`)
//!   they don't carry a large binary blob that must live in its own layer.
//!   Neither may carry an attestation (D-08), so for both of them the single
//!   layer is the entire inventory and an extra layer is a malformed layout.
//! - `TeamPackage` packs as one [`MT_TEAM_CONFIG`] JSON layer — same shape,
//!   same reason — PLUS an OPTIONAL [`MT_ATTESTATION`] layer, because a team
//!   is one of the two kinds attestation carriage covers (D-08). A team layout
//!   therefore holds one or two layers and nothing else. See the cross-kind
//!   bullet below for the rule about what an attestation layer may and may not
//!   be read to imply.
//! - [`MT_ATTESTATION`] is this file's ONLY layer media type not owned by a
//!   single package kind. It may appear on a server package (`pack_server`)
//!   or on a team package (`pack_team`, per D-08), and on neither an agent
//!   nor a workflow — an agent that needs an attestation is wrapped as a team
//!   of one. Package kind is read from the manifest's `artifactType` plus the
//!   typed package layer, NEVER from this layer's presence or spelling. See
//!   the constant's own rustdoc for the four reasons the spelling is
//!   kind-neutral.
//!
//! Every layer's `Descriptor` uses [`MediaType::Other`] via [`vendor_media_type`]
//! (RESEARCH Pattern 3) — never a hand-rolled parallel media-type enum.

use oci_spec::image::{Descriptor, MediaType};
use std::str::FromStr;

// ---------------------------------------------------------------------
// ServerPackage layers
// ---------------------------------------------------------------------

/// The compiled bootstrap Lambda binary — supplied to `pack_server` as raw
/// `&[u8]`, never inlined on `ServerPackage` (cross-AI review: binary
/// payloads are OCI layers, not typed-struct fields).
pub const MT_SERVER_BOOTSTRAP: &str = "application/vnd.pmcp.mcp-server.bootstrap.v1+binary";
/// The `name`/`version`/top-level `digest`/`binary_ref` fields not covered by
/// any of the other typed `ServerPackage` sections.
pub const MT_SERVER_ENVELOPE: &str = "application/vnd.pmcp.mcp-server.envelope.v1+json";
/// The typed equivalent of a `.pmcp/deploy.toml` (`ServerPackage.deploy`).
pub const MT_SERVER_DEPLOY_DESCRIPTOR: &str =
    "application/vnd.pmcp.mcp-server.deploy-descriptor.v1+json";
/// The server's cedar-policy set (`ServerPackage.policies`).
pub const MT_SERVER_CEDAR_POLICY_SET: &str =
    "application/vnd.pmcp.mcp-server.cedar-policy-set.v1+json";
/// The server's tool/connector metadata (`ServerPackage.tools`).
pub const MT_SERVER_TOOL_METADATA: &str = "application/vnd.pmcp.mcp-server.tool-metadata.v1+json";
/// The server's declared config slots (`ServerPackage.config_slots`).
pub const MT_SERVER_CONFIG_SLOTS: &str = "application/vnd.pmcp.mcp-server.config-slots.v1+json";
/// The author's server config file (`config.toml`), carried VERBATIM as raw
/// bytes. Generic across all three Shape A pure-config siblings (SQL,
/// workbook, OpenAPI) — the config's *dialect* is the server binary's
/// concern, not the package format's, so there is one media type rather than
/// one per kind. The original file name travels in the layer descriptor's
/// `org.opencontainers.image.title` annotation.
pub const MT_SERVER_CONFIG: &str = "application/vnd.pmcp.mcp-server.config.v1+toml";
/// An OpenAPI-backed server's spec file, carried VERBATIM as raw bytes.
/// Unlike [`MT_SERVER_CONFIG`] this one IS per-kind: only an OpenAPI server
/// has a spec, and its bytes are the spec document exactly as the author
/// wrote it (JSON or YAML — the extension travels in the descriptor's
/// `org.opencontainers.image.title` annotation).
pub const MT_SERVER_OPENAPI_SPEC: &str = "application/vnd.pmcp.mcp-server.openapi-spec.v1";
/// A REFERENCE to a server binary the package does not embed: the digest and
/// media type of a blob the *target environment* resolves for itself. The
/// mutually-exclusive counterpart of [`MT_SERVER_BOOTSTRAP`] — a package
/// carries exactly one of the two, never both and never neither. The payload
/// is a canonical-JSON `BinaryRef`.
pub const MT_SERVER_BINARY_REF: &str = "application/vnd.pmcp.mcp-server.binary-ref.v1+json";

// ---------------------------------------------------------------------
// AgentPackage / TeamPackage / WorkflowManifest — one layer each
// ---------------------------------------------------------------------

/// The whole `AgentPackage` struct, serialized as a single JSON layer.
pub const MT_AGENT_CONFIG: &str = "application/vnd.pmcp.agent.config.v1+json";
/// The whole `TeamPackage` struct, serialized as a single JSON layer.
pub const MT_TEAM_CONFIG: &str = "application/vnd.pmcp.team.config.v1+json";
/// The whole `WorkflowManifest` struct, serialized as a single JSON layer.
pub const MT_WORKFLOW_MANIFEST: &str = "application/vnd.pmcp.workflow.manifest.v1+json";

// ---------------------------------------------------------------------
// Cross-kind layers — the media types NOT owned by exactly one package kind
// ---------------------------------------------------------------------

/// A platform-issued attestation ABOUT a package, carried as an OPAQUE layer.
///
/// The bytes are written and read back VERBATIM. This crate never
/// deserializes, parses, sniffs or otherwise interprets them: the attestation
/// format is owned by the issuing platform, not by this crate. Everything this
/// crate reads about an attestation comes from the layer DESCRIPTOR's
/// annotations — [`ANNOTATION_ATTESTATION_SUBJECT`],
/// [`ANNOTATION_ATTESTATION_ISSUER`] and
/// [`ANNOTATION_ATTESTATION_PAYLOAD_TYPE`] — which is what makes "never
/// interprets the payload" literally true while still leaving something to
/// compare.
///
/// The string carries NO format suffix, following
/// [`MT_SERVER_OPENAPI_SPEC`] rather than [`MT_SERVER_CONFIG`]'s `+toml`
/// (D-05): the attestation format is platform-owned and expected to churn, and
/// pinning a format into the layer-index key would turn a format change into a
/// media-type change — a wire break for every already-published CLI.
///
/// # Why the spelling is KIND-NEUTRAL
///
/// 1. **Two carrier kinds exist.** Attestation carriage lands on the server
///    packer AND the team packer (D-08). A media type namespaced to one
///    package kind would make a team package carry a layer whose type names a
///    different kind — a claim that is simply false on disk. The kind
///    qualifier was already wrong at the moment it was written.
/// 2. **Kind-specificity elsewhere in this file tracks a kind-specific
///    PAYLOAD.** [`MT_TEAM_CONFIG`]'s bytes ARE a `TeamPackage`;
///    [`MT_SERVER_ENVELOPE`]'s bytes ARE server envelope fields. An
///    attestation payload is platform-owned opaque bytes whose shape does not
///    vary with the carrying kind, so a kind qualifier here would be false
///    precision rather than the existing convention.
/// 3. **D-05's own reasoning generalizes.** A format suffix was omitted so a
///    format change would not become a media-type change, and a media-type
///    change is a wire break for every already-published CLI. A kind qualifier
///    has the identical failure mode one axis over: adding the second carrier
///    kind would force either a second constant or a lie. One neutral spelling
///    avoids both.
/// 4. **One media type is one mechanism** (D-08: "one mechanism, not two").
///    The duplicate-media-type rejection on the read side, the attestation
///    read helper and the annotated-layer write call site are shared verbatim
///    by the server and team paths with no kind dispatch. Two constants would
///    require a kind-keyed lookup at every read site.
///
/// # This is NOT a package-kind signal
///
/// Package kind is determined by the manifest's `artifactType`
/// ([`ARTIFACT_TYPE_SERVER`] / [`ARTIFACT_TYPE_TEAM`] / [`ARTIFACT_TYPE_AGENT`]
/// / [`ARTIFACT_TYPE_WORKFLOW`]) together with which typed package layer is
/// present. Nothing may infer a package's kind from the presence, absence or
/// spelling of this layer.
///
/// # Rejected alternative
///
/// A PER-KIND PAIR — one attestation media type per carrying package kind,
/// sharing a single annotation vocabulary — was considered and rejected. It
/// buys nothing a reader cannot already get from `artifactType`, while costing
/// a kind-keyed lookup on every read path. (The rejected spellings are
/// deliberately not written out here: two acceptance criteria in this phase
/// negative-grep the source tree for them, and naming them in a comment would
/// make those guards self-invalidating.)
pub const MT_ATTESTATION: &str = "application/vnd.pmcp.attestation.v1";

/// Layer-descriptor annotation key naming the package an attestation is ABOUT.
///
/// The value is the `sha256:<hex>` manifest digest of the **UNATTESTED**
/// package — explicitly NOT the digest of the package that carries this layer.
/// Those two digests necessarily differ: the attestation layer lives inside
/// the manifest whose canonical bytes the carrying package's digest covers
/// (D-01). An attested package's own digest can therefore never equal the
/// subject it names, and that is by design.
///
/// # Why reverse-DNS
///
/// The OCI image-spec annotations document says custom annotation keys SHOULD
/// use reverse domain notation, and reserves the `org.opencontainers`
/// namespace for the spec itself. `vnd.pmcp` is a MEDIA-TYPE prefix, not a
/// domain, so it is the wrong shape for an annotation key. There is no
/// in-repo precedent to follow: this crate's only annotation key today is
/// `oci_spec`'s re-exported `org.opencontainers.image.title`.
pub const ANNOTATION_ATTESTATION_SUBJECT: &str = "run.pmcp.attestation.subject";
/// Layer-descriptor annotation key naming the issuer of an attestation.
///
/// Reverse-DNS for the same reason as [`ANNOTATION_ATTESTATION_SUBJECT`]. The
/// value is issuer-supplied and arrives from outside this repo; this crate
/// carries it and never validates it.
pub const ANNOTATION_ATTESTATION_ISSUER: &str = "run.pmcp.attestation.issuer";
/// Layer-descriptor annotation key naming the attestation payload's OWN media
/// type (e.g. a report-schema JSON document, or a signed envelope).
///
/// Reverse-DNS for the same reason as [`ANNOTATION_ATTESTATION_SUBJECT`].
/// Recording the payload's format HERE rather than in [`MT_ATTESTATION`] is
/// what lets the layer media type stay suffix-free while the format churns.
pub const ANNOTATION_ATTESTATION_PAYLOAD_TYPE: &str = "run.pmcp.attestation.payload-type";

// ---------------------------------------------------------------------
// artifactType per package kind (OCI 1.1 top-level manifest field)
// ---------------------------------------------------------------------

/// `artifactType` for an `mcp-server` package's `ImageManifest`.
pub const ARTIFACT_TYPE_SERVER: &str = "application/vnd.pmcp.mcp-server.v1";
/// `artifactType` for an `agent` package's `ImageManifest`.
pub const ARTIFACT_TYPE_AGENT: &str = "application/vnd.pmcp.agent.v1";
/// `artifactType` for a `team` package's `ImageManifest`.
pub const ARTIFACT_TYPE_TEAM: &str = "application/vnd.pmcp.team.v1";
/// `artifactType` for a `workflow` package's `ImageManifest`.
pub const ARTIFACT_TYPE_WORKFLOW: &str = "application/vnd.pmcp.workflow.v1";

// ---------------------------------------------------------------------
// Standard OCI empty-config blob (OCI 1.1 non-null config descriptor)
// ---------------------------------------------------------------------

/// Standard OCI 1.1 empty-config media type. Registries/validators (ECR)
/// require a non-null config descriptor even for artifact manifests that
/// have no meaningful "config" of their own.
pub const MT_EMPTY_CONFIG: &str = "application/vnd.oci.empty.v1+json";
/// The empty-config blob's exact content — the most minimal valid JSON
/// object.
pub const EMPTY_CONFIG_BLOB: &[u8] = b"{}";
/// `sha256:<hex>` of [`EMPTY_CONFIG_BLOB`]. Guarded against drift by the
/// `empty_config_digest_matches_hash_of_empty_json_blob` test below — this
/// constant can never silently diverge from the actual hash of `{}`.
pub const EMPTY_CONFIG_DIGEST: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
/// Byte length of [`EMPTY_CONFIG_BLOB`].
pub const EMPTY_CONFIG_SIZE: u64 = 2;

/// Map a vendor media-type string constant into an `oci_spec::image::MediaType`.
/// `MediaType::from(&str)` already routes unrecognized strings to
/// `MediaType::Other` (RESEARCH Pattern 3) — this helper just documents the
/// call site's intent.
pub fn vendor_media_type(media_type: &str) -> MediaType {
    MediaType::from(media_type)
}

/// Build the standard, shared empty-config `Descriptor` — non-null config,
/// required by ECR/OCI 1.1-conformant registries and validators. Callers
/// that actually WRITE the config blob to a layout should prefer
/// `OciLayout::write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)`
/// (which persists the bytes AND returns an identical `Descriptor`, since
/// both hash the same fixed bytes); this function exists for callers that
/// only need the descriptor shape (e.g. tests, or a caller building a
/// manifest that references an empty-config blob written elsewhere).
pub fn empty_config_descriptor() -> Descriptor {
    let digest = oci_spec::image::Digest::from_str(EMPTY_CONFIG_DIGEST)
        .expect("EMPTY_CONFIG_DIGEST is a well-formed sha256 digest string");
    Descriptor::new(
        vendor_media_type(MT_EMPTY_CONFIG),
        EMPTY_CONFIG_SIZE,
        digest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ManifestDigest;

    /// Drift guard (Task 1 `<action>`): the checked-in `EMPTY_CONFIG_DIGEST`
    /// constant must always equal the actual sha256 of `EMPTY_CONFIG_BLOB` —
    /// this test fails loudly if the two are ever edited out of sync.
    #[test]
    fn empty_config_digest_matches_hash_of_empty_json_blob() {
        let computed = ManifestDigest::from_bytes(EMPTY_CONFIG_BLOB);
        assert_eq!(computed.as_str(), EMPTY_CONFIG_DIGEST);
    }

    #[test]
    fn vendor_media_type_wraps_custom_string_as_other() {
        let mt = vendor_media_type(MT_WORKFLOW_MANIFEST);
        assert_eq!(mt.to_string(), MT_WORKFLOW_MANIFEST);
        assert!(matches!(mt, MediaType::Other(_)));
    }

    #[test]
    fn empty_config_descriptor_uses_standard_media_type_size_and_digest() {
        let descriptor = empty_config_descriptor();
        assert_eq!(descriptor.media_type().to_string(), MT_EMPTY_CONFIG);
        assert_eq!(descriptor.size(), EMPTY_CONFIG_SIZE);
        assert_eq!(descriptor.digest().to_string(), EMPTY_CONFIG_DIGEST);
    }
}
