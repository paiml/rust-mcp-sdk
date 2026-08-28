//! `pack_server`/`pack_agent`/`pack_team`/`pack_workflow` — serialize each
//! package type into a local [`OciLayout`] (RESEARCH System Architecture
//! Diagram, pack() steps 1-5):
//!
//! 1. Serialize each logical layer to bytes via [`canonicalize`] (deterministic).
//! 2. `layout.write_blob()` each → a content-addressed `Descriptor` with the
//!    correct vendor `MediaType`.
//! 3. Build an `oci_spec::image::ImageManifest` (empty config + layer
//!    descriptors + `artifactType`).
//! 4. Canonicalize the manifest via `olpc-cjson`.
//! 5. `sha256(canonical manifest bytes)` — stored as the manifest blob's own
//!    content-addressed digest AND returned as the function's
//!    [`ManifestDigest`] (identity key). One hash, one source of truth —
//!    no separate re-derivation of "the" manifest digest.
//!
//! The `ServerPackage` binary is a SEPARATE [`BinaryMode`] parameter to
//! `pack_server`, never a field read off the struct (cross-AI review: binary
//! payloads are OCI layers, not typed-struct fields) — it becomes either an
//! `MT_SERVER_BOOTSTRAP` layer (embedded bytes) or an `MT_SERVER_BINARY_REF`
//! layer (a digest the target environment resolves), never both.

use crate::digest::{canonicalize, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::oci::config_validation::{
    parse_declared_config_slots_in, parse_document, validate_config_slot_agreement,
    validate_config_slot_placeholders_in, validate_no_undeclared_env_refs_in,
};
use crate::oci::layout::OciLayout;
use crate::oci::media_types::{
    empty_config_descriptor, vendor_media_type, ANNOTATION_ATTESTATION_ISSUER,
    ANNOTATION_ATTESTATION_PAYLOAD_TYPE, ANNOTATION_ATTESTATION_SUBJECT, ARTIFACT_TYPE_SERVER,
    EMPTY_CONFIG_BLOB, MT_ATTESTATION, MT_EMPTY_CONFIG, MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP,
    MT_SERVER_CEDAR_POLICY_SET, MT_SERVER_CONFIG, MT_SERVER_CONFIG_SLOTS,
    MT_SERVER_DEPLOY_DESCRIPTOR, MT_SERVER_ENVELOPE, MT_SERVER_OPENAPI_SPEC,
    MT_SERVER_TOOL_METADATA,
};
use crate::oci::SingleLayerPackage;
use crate::package::{AgentPackage, BinaryRef, ServerPackage, TeamPackage, WorkflowManifest};
use oci_spec::image::{
    Descriptor, ImageManifest, ImageManifestBuilder, MediaType, ANNOTATION_TITLE, SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `ServerPackage` fields NOT covered by the deploy/cedar/tools/config-slots
/// layers — `name`, `version` and the top-level `digest` — packed as their own
/// small JSON layer ([`MT_SERVER_ENVELOPE`]) so every field round-trips
/// losslessly by plain serialize/deserialize. `pub(super)` so `oci::unpack`
/// (a sibling module) can reconstruct a `ServerPackage` from it.
///
/// The binary reference is deliberately NOT a field here (D-08): it lives in
/// its own [`MT_SERVER_BINARY_REF`] layer, so "which binary" is one fact in
/// one place rather than a struct field that could disagree with a layer.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ServerEnvelope {
    pub(super) name: String,
    pub(super) version: semver::Version,
    pub(super) digest: Option<ManifestDigest>,
}

/// How a packed server names its binary — exactly one of two mutually
/// exclusive shapes (D-05, D-06). A package always carries one binary layer;
/// which of the two it is decides whether the package is self-contained or
/// resolved by the target environment.
///
/// [`BinaryMode::Referenced`]'s `digest` is taken VERBATIM from the caller and
/// is NON-optional: `pmcp-package` never contacts a registry to derive or
/// confirm it (milestone Decision 2 forbids a registry client), and a
/// referenced binary with no digest would be an instruction to run an
/// unpinned binary. The canonical producer in this repo is
/// `cargo-pmcp`'s `deployment::targets::aws_lambda::artifact`, which builds it
/// via [`ManifestDigest::from_bytes`] against the release `.sha256` sidecar.
///
/// Note that a bare-hex sidecar (`<64 hex chars>` with no prefix) is NOT a
/// valid digest string — [`ManifestDigest::parse`] requires the `sha256:`
/// prefix, so prepend it before parsing.
///
/// # Examples
///
/// Both arms, side by side — a self-contained package that carries its binary,
/// and a Shape A pure-config package that only names one:
///
/// ```
/// use pmcp_package::digest::ManifestDigest;
/// use pmcp_package::oci::BinaryMode;
///
/// // Self-contained: the compiled bootstrap's bytes travel inside the package.
/// let bootstrap: Vec<u8> = b"\x7fELF...compiled-bootstrap-bytes".to_vec();
/// let embedded = BinaryMode::Embedded(&bootstrap);
/// assert!(matches!(embedded, BinaryMode::Embedded(bytes) if bytes == bootstrap));
///
/// // Pure config: the package carries no bytes at all, only a pinned digest
/// // the target environment resolves for itself.
/// let referenced = BinaryMode::Referenced {
///     digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
///     media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
/// };
/// match referenced {
///     BinaryMode::Referenced { digest, media_type } => {
///         assert!(digest.as_str().starts_with("sha256:"));
///         assert_eq!(media_type, "application/x-lambda-bootstrap; arch=arm64");
///     },
///     BinaryMode::Embedded(_) => unreachable!("constructed as Referenced"),
/// }
/// ```
#[derive(Debug, Clone)]
pub enum BinaryMode<'a> {
    /// The binary's bytes are embedded in the package as an
    /// [`MT_SERVER_BOOTSTRAP`] layer. The package is self-contained.
    Embedded(&'a [u8]),
    /// The binary is NOT embedded: the package records only its digest and
    /// media type in an [`MT_SERVER_BINARY_REF`] layer, and the target
    /// environment resolves the bytes for itself.
    Referenced {
        /// Content digest of the binary the target environment must run.
        digest: ManifestDigest,
        /// Descriptive media-type hint for that binary (e.g.
        /// `application/x-lambda-bootstrap; arch=arm64`).
        media_type: String,
    },
}

/// The author's server config file, carried into the package VERBATIM.
///
/// `bytes` are written to the [`MT_SERVER_CONFIG`] layer byte-for-byte and are
/// never re-derived from a parsed struct — packing must not rewrite,
/// templatize, normalize or reformat what the author wrote. `file_name` is
/// recorded in the layer descriptor's `org.opencontainers.image.title`
/// annotation so unpack can restore it under its original name.
///
/// Distinct from [`OpenApiSpecFile`] on purpose: two types rather than one
/// shared "named blob" means a caller cannot transpose the config and the spec.
#[derive(Debug, Clone, Copy)]
pub struct ConfigFile<'a> {
    /// The config file's original name, e.g. `london-tube.toml`.
    pub file_name: &'a str,
    /// The config file's exact bytes.
    pub bytes: &'a [u8],
}

/// An OpenAPI-backed server's spec file, carried into the package VERBATIM.
///
/// The spec sibling of [`ConfigFile`], with the same verbatim-bytes and
/// original-file-name contract, written to the [`MT_SERVER_OPENAPI_SPEC`]
/// layer.
///
/// [`MT_SERVER_OPENAPI_SPEC`]: crate::oci::media_types::MT_SERVER_OPENAPI_SPEC
#[derive(Debug, Clone, Copy)]
pub struct OpenApiSpecFile<'a> {
    /// The spec file's original name, e.g. `openapi.yaml`.
    pub file_name: &'a str,
    /// The spec file's exact bytes.
    pub bytes: &'a [u8],
}

/// A platform-issued attestation ABOUT this package, carried into it VERBATIM.
///
/// `bytes` are written to the [`MT_ATTESTATION`] layer byte-for-byte and are
/// NEVER deserialized, parsed, sniffed or re-derived by this crate — the
/// attestation format is the issuing platform's, not this crate's. The three
/// metadata fields are recorded as LAYER-descriptor annotations
/// ([`ANNOTATION_ATTESTATION_SUBJECT`], [`ANNOTATION_ATTESTATION_ISSUER`],
/// [`ANNOTATION_ATTESTATION_PAYLOAD_TYPE`]) so that reading metadata never
/// requires reading the payload.
///
/// `subject` is the `sha256:<hex>` manifest digest of the UNATTESTED package
/// this attestation is about — not the digest of the package that carries it.
/// See [`pack_server`]'s "two digests" section.
///
/// Distinct from [`ConfigFile`] and [`OpenApiSpecFile`] on purpose, for the
/// same reason those two are distinct from each other: three types rather than
/// one shared "named blob" type means a caller cannot transpose them.
///
/// [`MT_ATTESTATION`]: crate::oci::media_types::MT_ATTESTATION
/// [`ANNOTATION_ATTESTATION_SUBJECT`]: crate::oci::media_types::ANNOTATION_ATTESTATION_SUBJECT
/// [`ANNOTATION_ATTESTATION_ISSUER`]: crate::oci::media_types::ANNOTATION_ATTESTATION_ISSUER
/// [`ANNOTATION_ATTESTATION_PAYLOAD_TYPE`]: crate::oci::media_types::ANNOTATION_ATTESTATION_PAYLOAD_TYPE
#[derive(Debug, Clone, Copy)]
pub struct AttestationFile<'a> {
    /// The attestation payload's exact bytes, carried opaquely.
    pub bytes: &'a [u8],
    /// `sha256:<hex>` manifest digest of the UNATTESTED package this
    /// attestation is about.
    pub subject: &'a str,
    /// Who issued the attestation.
    pub issuer: &'a str,
    /// The payload's own media type (e.g. a report schema, or a signed
    /// envelope).
    pub payload_type: &'a str,
}

/// One layer's finished BYTES, plus everything needed either to write it or
/// merely to describe it — an in-memory layer plan, private to this module.
///
/// It exists so byte production can happen strictly BEFORE the first
/// `write_blob`. Without it, every descriptor `pack_server` assembles would
/// come out of a writing call, and the pack-time attestation-subject gate
/// (which needs the would-be unattested manifest digest) could not run until
/// the layers it is meant to gate were already on disk.
///
/// Deliberately NOT a public type and NOT a staging directory: a heavier
/// two-phase `OciLayout` API was considered during cross-AI review and
/// rejected. This is a plain in-memory vector element and nothing more.
struct PlannedLayer {
    /// The vendor (or standard) media type this layer's descriptor carries.
    media_type: MediaType,
    /// The layer's exact bytes, already canonicalized where canonicalization
    /// applies and left verbatim where it does not.
    bytes: Vec<u8>,
    /// Annotations to attach to the resulting LAYER descriptor, if any.
    annotations: Option<HashMap<String, String>>,
}

impl PlannedLayer {
    /// The descriptor this layer WOULD have, computed without touching the
    /// filesystem.
    ///
    /// Equal by construction to what [`write_planned_layer`] returns for the
    /// same plan: both route through the shared [`annotate`] helper over
    /// [`OciLayout::describe_blob`]/[`OciLayout::write_blob`], whose own
    /// equality is pinned by a test in `layout.rs`.
    fn describe(&self) -> Descriptor {
        annotate(
            OciLayout::describe_blob(self.media_type.clone(), &self.bytes),
            self.annotations.as_ref(),
        )
    }
}

/// Attach `annotations` to `descriptor`, if there are any.
///
/// Shared by the describing and the writing path so the two cannot drift into
/// annotating differently — which would make the two manifests differ by more
/// than their layer vectors and silently break the pack-time subject gate.
///
/// Unlike the index-descriptor annotations set in [`finalize_pack`] — which are
/// applied AFTER `write_manifest` has already computed the manifest digest and
/// therefore do NOT feed it — a LAYER descriptor's annotations live inside the
/// manifest that `canonicalize` then hashes, so these annotations DO feed the
/// manifest digest. Renaming the config file, or swapping an attestation's
/// claimed subject, changes the package's identity.
fn annotate(
    mut descriptor: Descriptor,
    annotations: Option<&HashMap<String, String>>,
) -> Descriptor {
    if let Some(annotations) = annotations {
        descriptor.set_annotations(Some(annotations.clone()));
    }
    descriptor
}

/// Write one planned layer's bytes to `layout` and return its descriptor.
///
/// The ONE writing path for a server package's layers, called only after every
/// gate in [`validate_pack_preconditions`] has returned `Ok`.
fn write_planned_layer(layout: &OciLayout, planned: &PlannedLayer) -> Result<Descriptor> {
    let descriptor = layout.write_blob(planned.media_type.clone(), &planned.bytes)?;
    Ok(annotate(descriptor, planned.annotations.as_ref()))
}

/// Plan a STRUCT layer: the typed value serialized through the crate's one
/// canonicalizer.
fn plan_struct_layer<T: Serialize>(media_type: &str, value: &T) -> Result<PlannedLayer> {
    Ok(PlannedLayer {
        media_type: vendor_media_type(media_type),
        bytes: canonicalize(value)?,
        annotations: None,
    })
}

/// Plan a VERBATIM-bytes layer carrying `annotations` on its descriptor.
///
/// Raw bytes, never `canonicalize` — the crate rule is to digest what was
/// stored, never re-derive it from a parsed struct.
fn plan_annotated_layer(
    media_type: &str,
    annotations: HashMap<String, String>,
    bytes: &[u8],
) -> PlannedLayer {
    PlannedLayer {
        media_type: vendor_media_type(media_type),
        bytes: bytes.to_vec(),
        annotations: Some(annotations),
    }
}

/// Plan the layer for a named vendor-content file, recording the author's
/// original file name in the descriptor's standard
/// `org.opencontainers.image.title` annotation.
///
/// One mechanism, not two: a thin caller of [`plan_annotated_layer`] supplying
/// the single-entry title map, where the attestation supplies a three-entry
/// map.
fn plan_named_file_layer(media_type: &str, file_name: &str, bytes: &[u8]) -> PlannedLayer {
    plan_annotated_layer(
        media_type,
        HashMap::from([(ANNOTATION_TITLE.to_string(), file_name.to_string())]),
        bytes,
    )
}

/// Plan the attestation layer: the payload VERBATIM, with the subject, issuer
/// and payload media type on the LAYER descriptor's annotations (and therefore
/// inside the manifest digest).
fn plan_attestation_layer(attestation: AttestationFile<'_>) -> PlannedLayer {
    plan_annotated_layer(
        MT_ATTESTATION,
        HashMap::from([
            (
                ANNOTATION_ATTESTATION_SUBJECT.to_string(),
                attestation.subject.to_string(),
            ),
            (
                ANNOTATION_ATTESTATION_ISSUER.to_string(),
                attestation.issuer.to_string(),
            ),
            (
                ANNOTATION_ATTESTATION_PAYLOAD_TYPE.to_string(),
                attestation.payload_type.to_string(),
            ),
        ]),
        attestation.bytes,
    )
}

/// Plan the one binary layer this package carries, per [`BinaryMode`] — the
/// bytes-producing sibling of the writing path.
///
/// Embedded bytes travel verbatim. A reference is a STRUCT layer: the existing
/// [`BinaryRef`] wire type serialized via `canonicalize`, keeping the wire
/// payload's `Option<ManifestDigest>` tolerance while the API type
/// [`BinaryMode::Referenced`] stays non-optional.
fn plan_binary_layer(binary: &BinaryMode<'_>) -> Result<PlannedLayer> {
    match binary {
        BinaryMode::Embedded(bootstrap) => Ok(PlannedLayer {
            media_type: vendor_media_type(MT_SERVER_BOOTSTRAP),
            bytes: (*bootstrap).to_vec(),
            annotations: None,
        }),
        BinaryMode::Referenced { digest, media_type } => {
            let binary_ref = BinaryRef {
                digest: Some(digest.clone()),
                media_type: media_type.clone(),
            };
            plan_struct_layer(MT_SERVER_BINARY_REF, &binary_ref)
        },
    }
}

/// Every layer a `pack_server` call will write, produced as BYTES before
/// anything reaches the filesystem.
///
/// The attestation is held SEPARATELY rather than appended to `unattested`, so
/// the pack-time subject gate reads the unattested layer vector directly
/// instead of filtering a mixed one by media type. The two manifests the gate
/// compares can then differ only in their layer vectors — which is exactly the
/// difference the subject digest is about.
struct ServerLayerPlan {
    /// Every layer EXCEPT the attestation, in deterministic push order.
    unattested: Vec<PlannedLayer>,
    /// The attestation layer, when the caller supplied one.
    attestation: Option<PlannedLayer>,
}

/// Produce every layer's bytes for a server pack. Pure: no filesystem access,
/// no `OciLayout` parameter, nothing observable outside the returned plan.
fn plan_server_layers(
    package: &ServerPackage,
    binary: &BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    attestation: Option<AttestationFile<'_>>,
) -> Result<ServerLayerPlan> {
    let envelope = ServerEnvelope {
        name: package.name.clone(),
        version: package.version.clone(),
        digest: package.digest.clone(),
    };

    // Push order is deterministic so the manifest bytes (and therefore the
    // manifest digest) are reproducible. It is NOT a read-order contract:
    // the optional layers make position meaningless, and `unpack_server`
    // locates every layer by media type.
    let mut unattested = vec![
        plan_binary_layer(binary)?,
        plan_struct_layer(MT_SERVER_ENVELOPE, &envelope)?,
        plan_struct_layer(MT_SERVER_DEPLOY_DESCRIPTOR, &package.deploy)?,
        plan_struct_layer(MT_SERVER_CEDAR_POLICY_SET, &package.policies)?,
        plan_struct_layer(MT_SERVER_TOOL_METADATA, &package.tools)?,
        plan_struct_layer(MT_SERVER_CONFIG_SLOTS, &package.config_slots)?,
    ];

    if let Some(config) = config {
        unattested.push(plan_named_file_layer(
            MT_SERVER_CONFIG,
            config.file_name,
            config.bytes,
        ));
    }

    // The spec is planned after the config so the push order stays
    // deterministic. Its bytes stay RAW — never through `canonicalize` and
    // never parsed: one media type carries both JSON and YAML documents, and
    // the format is evident from the file-name annotation (D-16).
    if let Some(spec) = spec {
        unattested.push(plan_named_file_layer(
            MT_SERVER_OPENAPI_SPEC,
            spec.file_name,
            spec.bytes,
        ));
    }

    Ok(ServerLayerPlan {
        unattested,
        attestation: attestation.map(plan_attestation_layer),
    })
}

/// Every gate [`pack_server`] runs BEFORE its first `write_blob`, so a rejected
/// pack adds neither a blob nor an index entry — which is what makes "a
/// resolved secret never travels in a layer" a property of the filesystem and
/// not merely of the return value.
///
/// That invariant holds LITERALLY, for every gate including the
/// attestation-subject one. `pack_server` produces every layer's bytes first
/// ([`plan_server_layers`], which touches nothing on disk), runs this function
/// over the resulting plan, and only then walks the plan writing blobs. A
/// refused pack therefore leaves the destination layout byte-for-byte as it
/// found it, which
/// `negative.rs::a_refused_pack_leaves_the_destination_layout_byte_for_byte_unchanged`
/// asserts over the FULL recursive file list rather than over the attestation
/// blob alone.
///
/// Extracted from `pack_server` rather than inlined so that function stays
/// under the repo's cognitive-complexity ceiling (CLAUDE.md: "Cognitive
/// complexity ≤25 per function", enforced in CI by
/// `pmat quality-gate --checks complexity`); inlined, the arms below push
/// `pack_server` over it. Each gate is its own named free function for the
/// same reason — growing THIS function inline is how that PR-blocking gate
/// fires next.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] when the config document's
/// `[[config_slots]]` block disagrees with `package.config_slots`, when a
/// slot-declared value key holds a resolved literal rather than an environment
/// reference, when the config defers a slot-addressable key to the environment
/// that NO slot declares, or when a package that ships NO config file
/// nonetheless declares a slot naming a `config_key`. Returns
/// [`PackageError::Serialize`] when the
/// config document is not parseable TOML. Returns
/// [`PackageError::MalformedDigest`] or
/// [`PackageError::AttestationSubjectMismatch`] per
/// [`reject_an_attestation_subject_naming_another_package`].
///
/// [`PackageError::MalformedDigest`]: crate::error::PackageError::MalformedDigest
/// [`PackageError::AttestationSubjectMismatch`]: crate::error::PackageError::AttestationSubjectMismatch
fn validate_pack_preconditions(
    package: &ServerPackage,
    config: Option<ConfigFile<'_>>,
    attestation: Option<AttestationFile<'_>>,
    unattested_layers: &[PlannedLayer],
) -> Result<()> {
    reject_attestation_annotations_that_break_canonical_json(attestation)?;
    reject_an_attestation_subject_naming_another_package(
        attestation,
        unattested_layers,
        ARTIFACT_TYPE_SERVER,
    )?;
    // The document gates run only when a config file is present: an embedded,
    // pre-built package has no config document to read declarations out of,
    // and every pre-0.2 server package is exactly that shape.
    let Some(config) = config else {
        // No config file: a slot may still be declared (the keyless embedded
        // shape every pre-0.2 package has), but a slot naming a `config_key`
        // points into a config document this package does not ship — a claim
        // nothing could validate at pack time and nothing could fill at
        // deploy time. Refusing it here is the mirror of the "a value slot
        // must name the config key it fills" rule on the with-config path.
        return reject_config_keys_without_a_config(package);
    };
    // One parse feeds all three gates — the `_in` variants take the document.
    let document = parse_document(config.bytes)?;
    let declared = parse_declared_config_slots_in(&document)?;
    validate_config_slot_agreement(&declared, &package.config_slots)?;
    validate_config_slot_placeholders_in(&document, &package.config_slots)?;
    // The CONFIG -> SLOT direction. The two gates above both start from the
    // declared slot list, so a config declaring NO slots satisfies both
    // trivially; this one starts from the document and is what makes the pair
    // of questions symmetric.
    validate_no_undeclared_env_refs_in(&document, &package.config_slots)
}

/// The no-config half of [`validate_pack_preconditions`]: refuse a package that
/// ships no config document while declaring a slot that names a `config_key`.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the offending key.
fn reject_config_keys_without_a_config(package: &ServerPackage) -> Result<()> {
    for slot in &package.config_slots {
        if let Some(config_key) = slot.config_key.as_deref() {
            return Err(PackageError::ConfigSlotViolation {
                key: config_key.to_string(),
                reason: "the package slot names a config key, but the package ships no \
                         config file for that key to address — pack with the config \
                         document, or drop the slot's config_key"
                    .to_string(),
            });
        }
    }
    Ok(())
}

/// Gate A: refuse an attestation attached to a team whose references are not
/// all resolved (D-09).
///
/// # Why — "attestation implies resolved", the `cargo build --locked` analogue
///
/// An attestation's subject is a DIGEST, and a digest is a claim about exactly
/// which code was examined. If that digest covers a package holding a
/// `ComponentRef::Range`, two environments with the same package digest run
/// DIFFERENT code — dev resolves `london-tube@^1.2` to 1.3.0 while prod
/// resolves the same range to the 1.2.0 it already has. The attestation would
/// then attest nothing about what actually runs, while this crate ships a
/// verification path implying otherwise. A reader meeting this refusal deserves
/// that reason at the refusal, which is why it is written here.
///
/// # Scoped to the CLAIM, not to the format
///
/// The gate is VACUOUS when no attestation is supplied, and that is the whole
/// design: D-09 is *attestation implies resolved*, NOT *teams must always be
/// pinned*. A team holding ranges is a legal package — it is what capture
/// produces before resolution — and it must keep packing unattested. Guarding
/// unconditionally would break every existing unattested team pack.
///
/// # The ONE-LEVEL DEPTH LIMIT, restated here and not only on the helper
///
/// `TeamMember.agent` pins an agent BY DIGEST. That digest covers the agent
/// package's own contents, including its `connectors: Vec<ComponentRef>`, which
/// may themselves be ranges. The team package holds only the digest, and
/// milestone Decision 2 forbids this crate a registry client, so nothing here
/// can resolve a referenced package offline to look inside it.
///
/// A team that passes this gate is resolved AT ITS OWN LEVEL. It is NOT
/// transitively resolved and must never be read as such. Closing that
/// transitively is platform ADMISSION POLICY — requiring every pinned component
/// to itself be attested — not SDK work. The limit is pinned as visible
/// behaviour by
/// `negative.rs::attestation_resolved::an_attested_team_whose_pinned_agent_itself_holds_a_range_still_packs`,
/// and it is also inside the error text a caller sees, so it cannot be missed
/// by someone who never opens these docs.
///
/// # Why there is no server-path equivalent — a fact about the type
///
/// This gate is VACUOUS on the server path, so `pack_server` deliberately does
/// not call it. `ServerPackage`'s fields are `name`, `version`, `digest`,
/// `deploy`, `policies`, `tools` and `config_slots` — not one of them is a
/// `ComponentRef`, so there is no unresolved reference a server package could
/// hold. Its absence there is a fact about the type rather than an omission;
/// do NOT "fix" the apparent asymmetry by adding a no-op call on the server
/// path.
///
/// # Errors
///
/// Propagates [`TeamPackage::validate_all_pinned`]'s
/// [`PackageError::InvalidReference`] unchanged, which already names the
/// offending component, its `component_type` and the depth limit. No new error
/// variant exists for this case, deliberately: an attestation over an
/// unresolved package IS an invalid reference.
///
/// [`PackageError::InvalidReference`]: crate::error::PackageError::InvalidReference
fn reject_an_attestation_over_an_unresolved_team(
    package: &TeamPackage,
    attestation: Option<AttestationFile<'_>>,
) -> Result<()> {
    if attestation.is_none() {
        return Ok(());
    }
    package.validate_all_pinned()
}

/// The first byte offset at which `value` carries a C0 control character, with
/// the character itself.
///
/// C0 (U+0000–U+001F) is exactly the range RFC 8259 forbids unescaped inside a
/// JSON string. DEL (U+007F) and every other non-ASCII code point are legal
/// unescaped and are deliberately NOT rejected — over-refusing an issuer that
/// happens to be non-ASCII would be a bug of its own.
fn first_control_character(value: &str) -> Option<(usize, char)> {
    value
        .char_indices()
        .find(|(_, character)| (*character as u32) < 0x20)
}

/// Refuse an attestation whose issuer or payload-type value cannot survive this
/// crate's canonical JSON form.
///
/// # Why this gate exists
///
/// `canonicalize` is OLPC/TUF Canonical JSON, which escapes ONLY `"` and `\`
/// and writes every other character — including C0 control characters —
/// LITERALLY. That is correct for its own spec and wrong for RFC 8259, so a
/// control character in ANY string reaching the manifest produces manifest bytes
/// that no JSON parser will read back. The package packs cleanly, its digest
/// verifies (the digest is over the bytes as stored), and then `unpack_server`
/// — and every other OCI tool — fails on the manifest.
///
/// Found by `tests/attestation_opacity.rs`'s adversarial-annotation property,
/// which generated `issuer = "\0"` and observed a package that packed and could
/// not be unpacked. Refusing at pack time is the fix that keeps an unreadable
/// artifact from being produced at all.
///
/// # Why only issuer and payload-type
///
/// `subject` is validated more strictly one gate later:
/// [`reject_an_attestation_subject_naming_another_package`] parses it as
/// `sha256:<64 hex>`, a form that admits no control character. Duplicating the
/// weaker check here would only change which of two refusals a caller sees.
///
/// # Known wider blind spot, stated rather than left to be found
///
/// The same canonical-JSON hazard applies to every OTHER string this crate
/// canonicalizes: a `ConfigFile`/`OpenApiSpecFile` `file_name` (which becomes
/// the standard `org.opencontainers.image.title` annotation) and every `String`
/// inside `ServerPackage` itself. Those values come from the packaging author's
/// own filesystem and source, not from an untrusted platform-issued
/// attestation, so they are a different trust class and are deliberately out of
/// this gate's scope rather than silently assumed safe.
///
/// # Errors
///
/// Returns [`PackageError::AttestationAnnotationInvalid`] naming the annotation
/// key and the offending code point. It never reproduces the value.
///
/// [`PackageError::AttestationAnnotationInvalid`]: crate::error::PackageError::AttestationAnnotationInvalid
fn reject_attestation_annotations_that_break_canonical_json(
    attestation: Option<AttestationFile<'_>>,
) -> Result<()> {
    let Some(attestation) = attestation else {
        return Ok(());
    };
    for (key, value) in [
        (ANNOTATION_ATTESTATION_ISSUER, attestation.issuer),
        (
            ANNOTATION_ATTESTATION_PAYLOAD_TYPE,
            attestation.payload_type,
        ),
    ] {
        if let Some((offset, character)) = first_control_character(value) {
            return Err(PackageError::AttestationAnnotationInvalid {
                annotation: key.to_string(),
                reason: format!(
                    "the value carries the control character U+{:04X} at byte offset {offset}; \
                     canonical JSON writes control characters literally, so the manifest would \
                     not be parseable JSON",
                    character as u32
                ),
            });
        }
    }
    Ok(())
}

/// The pack-time half of D-02's two-ended subject check: refuse an attestation
/// whose `subject` does not name the package it is being packed into.
///
/// The comparison is against the would-be UNATTESTED manifest digest — the
/// digest this same package would have had if packed with no attestation at
/// all — because that is what an attestation's subject means (D-01's two-digest
/// consequence). Comparing against the ATTESTED digest instead could never
/// match, and "fixing" it that way is one of the two regressions
/// `roundtrip.rs`'s two-digest test exists to block.
///
/// Runs BEFORE the first `write_blob`, so an attestation attached to the wrong
/// package is unrepresentable in a produced layout rather than merely reported
/// after one is on disk.
///
/// # The accepted cost, stated rather than discovered
///
/// On an ATTESTED pack the manifest is canonicalized TWICE: once dry here, and
/// once for real in [`finalize_pack`]. D-02 accepts that. An unattested pack is
/// unaffected — the `else` below returns immediately and the manifest is
/// canonicalized exactly once, as it always was.
///
/// # Errors
///
/// Returns [`PackageError::MalformedDigest`] when `subject` is not a
/// well-formed `sha256:<64 hex>` string, and
/// [`PackageError::AttestationSubjectMismatch`] when it is well-formed but
/// names a different package. Neither error carries any attestation payload
/// material — see that variant's rustdoc for the rule.
///
/// [`PackageError::MalformedDigest`]: crate::error::PackageError::MalformedDigest
/// [`PackageError::AttestationSubjectMismatch`]: crate::error::PackageError::AttestationSubjectMismatch
fn reject_an_attestation_subject_naming_another_package(
    attestation: Option<AttestationFile<'_>>,
    unattested_layers: &[PlannedLayer],
    artifact_type: &str,
) -> Result<()> {
    let Some(attestation) = attestation else {
        return Ok(());
    };
    // Parsed, not string-compared: a subject that is not a well-formed digest
    // is a malformed claim, and reporting it as a MISMATCH would tell an
    // operator to go looking for the wrong package.
    let supplied = ManifestDigest::parse(attestation.subject)?;
    let computed = would_be_unattested_manifest_digest(unattested_layers, artifact_type)?;
    if supplied == computed {
        return Ok(());
    }
    Err(PackageError::AttestationSubjectMismatch {
        supplied: supplied.as_str().to_string(),
        computed: computed.as_str().to_string(),
    })
}

/// The manifest digest this package WOULD have if packed with no attestation:
/// describe every non-attestation layer, assemble the manifest through the
/// SAME helper [`finalize_pack`] uses, canonicalize with the crate's ONE
/// canonicalizer, and hash.
///
/// Every step is the dry twin of a step `pack_server` performs for real, which
/// is why the result is comparable at all:
/// [`OciLayout::describe_blob`] mirrors `write_blob`,
/// [`assemble_manifest`] IS the builder chain `finalize_pack` runs, and
/// [`crate::oci::media_types::empty_config_descriptor`] returns exactly the
/// descriptor `finalize_pack`'s empty-config `write_blob` produces (both hash
/// the same two fixed bytes). A bespoke hash or a hand-rolled serialization
/// here would not equal the digest `finalize_pack` later stores.
///
/// `artifact_type` is a PARAMETER rather than a constant because two package
/// kinds now carry attestations (D-08): a server pack passes
/// [`ARTIFACT_TYPE_SERVER`] and a team pack passes `P::ARTIFACT_TYPE`
/// (`ARTIFACT_TYPE_TEAM`). It must be the SAME value the matching
/// [`finalize_pack`] call will use, or the dry manifest differs from the real
/// one in a field that is inside the hash and the comparison silently stops
/// meaning anything.
fn would_be_unattested_manifest_digest(
    unattested_layers: &[PlannedLayer],
    artifact_type: &str,
) -> Result<ManifestDigest> {
    let layers: Vec<Descriptor> = unattested_layers
        .iter()
        .map(PlannedLayer::describe)
        .collect();
    let manifest = assemble_manifest(empty_config_descriptor(), layers, artifact_type)?;
    Ok(ManifestDigest::from_bytes(&canonicalize(&manifest)?))
}

/// Pack `package` plus its binary (embedded or referenced) and its optional
/// verbatim config/spec files into `layout` as a local OCI artifact. Returns
/// the canonical manifest digest.
///
/// `config` and `spec` are both optional because an embedded, pre-built server
/// package has neither: they exist for the Shape A pure-config servers, whose
/// entire identity is their config (plus, for OpenAPI, their spec).
///
/// # The spec layer is OPTIONAL, and its absence is a declaration
///
/// Passing `spec: None` is the CALLER'S declaration that this server is
/// curated-only — it mirrors `pmcp-openapi-server`'s `--spec: Option<PathBuf>`,
/// where a server boots and serves its curated tools with no OpenAPI document
/// at all. It is never a default that quietly drops an author's spec: when
/// `spec` is `Some`, the bytes are always written to their own
/// [`MT_SERVER_OPENAPI_SPEC`] layer, VERBATIM and unparsed, under the author's
/// original file name. There is no absence marker (D-14) — an absent spec is
/// simply an absent layer.
///
/// [`MT_SERVER_OPENAPI_SPEC`]: crate::oci::media_types::MT_SERVER_OPENAPI_SPEC
///
/// # The attestation layer is OPTIONAL, and it carries TWO digests
///
/// Passing `attestation: Some(..)` writes the payload bytes VERBATIM to their
/// own [`MT_ATTESTATION`] layer — never canonicalized, never parsed — with the
/// subject, issuer and payload media type recorded as annotations on that
/// LAYER's descriptor. `None` writes no layer at all; absence is exactly the
/// layer's absence, with no marker (D-14).
///
/// The accepted consequence, stated here rather than discovered later: an
/// attested package's own manifest digest NECESSARILY DIFFERS from the
/// unattested digest the attestation names as its subject. The attestation
/// layer (and its annotations) sit inside the manifest whose canonical bytes
/// are hashed, so adding the attestation changes the hash. Two digests exist,
/// and that is deliberate.
///
/// The rejected alternative was to exclude the attestation layer from the
/// canonical digest. That would make the two digests equal, at the cost of
/// weakening [`crate::digest::verify()`] into verifying everything EXCEPT the
/// one layer an attacker would most want to swap.
///
/// [`MT_ATTESTATION`]: crate::oci::media_types::MT_ATTESTATION
///
/// # The subject is CHECKED, not trusted
///
/// The subject arrives from the issuing platform, not from this repo, so it is
/// verified rather than recorded: this function computes the would-be
/// unattested manifest digest and REFUSES to pack when the supplied subject
/// names anything else — before writing a single blob. See
/// `reject_an_attestation_subject_naming_another_package`, which also states
/// the accepted cost (an attested pack canonicalizes the manifest twice).
///
/// `unpack_server` re-derives the same digest INDEPENDENTLY rather than
/// trusting that this gate ran, because a layout can arrive from anywhere.
///
/// # Errors
///
/// Returns [`PackageError::Layout`] if the `ImageManifest` fails to build or
/// if any blob/index write fails. Returns [`PackageError::Serialize`] if a
/// layer fails to canonicalize.
///
/// When `config` is `Some`, returns [`PackageError::ConfigSlotViolation`]
/// BEFORE writing anything if the config's `[[config_slots]]` declaration block
/// disagrees with `package.config_slots` in either direction (D-01: the shipped
/// config is the source of truth), if a slot-declared value key holds a
/// resolved literal rather than a `${VAR}` / `env:VAR` reference (D-04), or if
/// the config holds a `${VAR}` / `env:VAR` reference at a slot-addressable key
/// that no slot declares — the CONFIG -> SLOT direction, without which a
/// package under-reports what a target environment must supply. See
/// [`crate::oci::config_validation`].
///
/// When `config` is `None`, returns [`PackageError::ConfigSlotViolation`] if
/// any package slot carries a `config_key`: the key names a path inside a
/// config document this package does not ship, a claim nothing can validate or
/// fill. Keyless slots (the pre-0.2 embedded shape) remain legal without a
/// config.
///
/// When `attestation` is `Some`, returns [`PackageError::MalformedDigest`]
/// BEFORE writing anything if its `subject` is not a well-formed
/// `sha256:<64 hex>` string, or
/// [`PackageError::AttestationSubjectMismatch`] if the subject is well-formed
/// but does not equal this package's would-be unattested manifest digest. Both
/// carry digest strings only, never attestation payload material.
///
/// When `attestation` is `Some`, also returns
/// [`PackageError::AttestationAnnotationInvalid`] BEFORE writing anything if
/// its `issuer` or `payload_type` carries a C0 control character. Canonical
/// JSON writes control characters literally, so packing one would produce a
/// manifest that is not parseable JSON — a package that packs and can never be
/// unpacked. See `reject_attestation_annotations_that_break_canonical_json`.
///
/// [`PackageError::Serialize`]: crate::error::PackageError::Serialize
/// [`PackageError::ConfigSlotViolation`]: crate::error::PackageError::ConfigSlotViolation
/// [`PackageError::MalformedDigest`]: crate::error::PackageError::MalformedDigest
/// [`PackageError::AttestationSubjectMismatch`]: crate::error::PackageError::AttestationSubjectMismatch
/// [`PackageError::AttestationAnnotationInvalid`]: crate::error::PackageError::AttestationAnnotationInvalid
///
/// # Examples
///
/// Pack a Shape A pure-config server — a referenced binary plus the author's
/// verbatim `config.toml` — and read the config back byte-for-byte:
///
/// ```
/// use pmcp_package::digest::ManifestDigest;
/// use pmcp_package::oci::{pack_server, unpack_server, BinaryMode, ConfigFile, OciLayout};
/// # use pmcp_package::package::*;
/// # use std::collections::BTreeMap;
/// # fn deploy() -> DeployDescriptor {
/// #     DeployDescriptor {
/// #         target: TargetSection { target_type: "pmcp-run".into(), version: "1.0.0".into() },
/// #         metadata: None,
/// #         aws: AwsSection { region: "us-east-1".into() },
/// #         server: ServerSection {
/// #             name: "london-tube".into(), memory_mb: Some(1024), timeout_seconds: 30,
/// #             memory: None, cpu: None, ingress: None, allow_unauthenticated: None, binary: None,
/// #         },
/// #         environment: BTreeMap::new(),
/// #         secrets: BTreeMap::new(),
/// #         auth: AuthSection {
/// #             enabled: false, provider: "none".into(), callback_urls: vec![],
/// #             cognito: None, dcr: None, groups: None, scopes: None,
/// #         },
/// #         observability: ObservabilitySection {
/// #             log_retention_days: 30, enable_xray: true, create_dashboard: true, alarms: None,
/// #         },
/// #         composition: None, assets: None, iam: None, gcp: None, layout: None,
/// #     }
/// # }
/// let package = ServerPackage {
///     name: "london-tube".to_string(),
///     version: semver::Version::parse("1.0.0").unwrap(),
///     digest: None,
///     deploy: deploy(),
///     policies: CedarPolicySet(vec![]),
///     tools: vec![],
///     config_slots: vec![],
/// };
///
/// let dir = tempfile::tempdir().unwrap();
/// let layout = OciLayout::create(dir.path()).unwrap();
///
/// let config_toml = b"name = \"london-tube\"\n";
/// pack_server(
///     &package,
///     BinaryMode::Referenced {
///         digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0"),
///         media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
///     },
///     Some(ConfigFile { file_name: "london-tube.toml", bytes: config_toml }),
///     // Curated-only: this server carries no OpenAPI spec.
///     None,
///     // Unattested: this server carries no attestation.
///     None,
///     &layout,
/// )
/// .unwrap();
///
/// let unpacked = unpack_server(&layout).unwrap();
/// let config = unpacked.config.unwrap();
/// assert_eq!(config.file_name, "london-tube.toml");
/// assert_eq!(config.bytes, config_toml);
/// assert_eq!(unpacked.spec, None);
/// assert_eq!(unpacked.attestation, None);
/// ```
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    attestation: Option<AttestationFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    // 1. BYTES. Nothing here touches the filesystem, which is what lets every
    //    gate below — including the attestation-subject gate, which needs the
    //    would-be unattested manifest digest — run before the first write.
    let plan = plan_server_layers(package, &binary, config, spec, attestation)?;

    // 2. GATES. Any refusal returns here, with the destination layout still
    //    byte-for-byte as it was found.
    validate_pack_preconditions(package, config, attestation, &plan.unattested)?;

    // 3. WRITES. The attestation goes LAST so the push order stays
    //    deterministic. As with every other layer, that order is explicitly
    //    NOT a read-order contract — `unpack_server` locates every layer by
    //    media type, so a re-ordered manifest reads identically.
    let mut layers = Vec::with_capacity(plan.unattested.len() + 1);
    for planned in plan.unattested.iter().chain(plan.attestation.iter()) {
        layers.push(write_planned_layer(layout, planned)?);
    }

    finalize_pack(
        layout,
        layers,
        ARTIFACT_TYPE_SERVER,
        &package.name,
        &package.version,
    )
}

/// Every gate a [`pack_single_layer`] call runs BEFORE its first `write_blob`
/// — the single-layer sibling of [`validate_pack_preconditions`], carrying the
/// same "a rejected pack adds neither a blob nor an index entry" invariant.
///
/// It holds the two KIND-NEUTRAL attestation gates, called in the same order
/// and through the SAME two functions the server path uses: no second copy of
/// either check exists, and a fix applied to one is a fix applied to both.
///
/// # Errors
///
/// Returns [`PackageError::AttestationAnnotationInvalid`],
/// [`PackageError::MalformedDigest`] or
/// [`PackageError::AttestationSubjectMismatch`] per the two gates it calls.
///
/// [`PackageError::AttestationAnnotationInvalid`]: crate::error::PackageError::AttestationAnnotationInvalid
/// [`PackageError::MalformedDigest`]: crate::error::PackageError::MalformedDigest
/// [`PackageError::AttestationSubjectMismatch`]: crate::error::PackageError::AttestationSubjectMismatch
fn validate_single_layer_pack_preconditions(
    attestation: Option<AttestationFile<'_>>,
    unattested_layers: &[PlannedLayer],
    artifact_type: &str,
) -> Result<()> {
    reject_attestation_annotations_that_break_canonical_json(attestation)?;
    reject_an_attestation_subject_naming_another_package(
        attestation,
        unattested_layers,
        artifact_type,
    )
}

/// Pack any single-layer package (agent/team/workflow) into `layout`:
/// serialize it to one canonical-JSON config layer under its vendor media
/// type, optionally append a verbatim attestation layer, then wrap the result
/// in a manifest with the kind's `artifactType`. The per-kind constants come
/// from the [`SingleLayerPackage`] impl — one path, no per-kind copy-paste.
/// Returns the canonical manifest digest.
///
/// # Attestation carriage covers SERVER and TEAM packages only (D-08)
///
/// This helper takes `attestation` because all three single-layer kinds route
/// through it, but only [`pack_team`] EXPOSES the parameter. [`pack_agent`] and
/// [`pack_workflow`] pass `None` and their public signatures are unchanged.
///
/// The reason is stated here, at the shared helper, because this is exactly
/// where a later reader is tempted to "finish the job" by exposing it on all
/// three: **the unit that ships to production is a team, and an agent is a
/// team of one in its essence.** An agent that needs an attestation is wrapped
/// as a team of one and attested there; it does not get its own carriage path.
/// `pack_workflow` is out for the same reason — a workflow is a captured
/// resolution of a team, not an independently shipped unit.
///
/// The consequence to preserve: ONE mechanism, not two. The team path reuses
/// the server path's [`plan_attestation_layer`], [`write_planned_layer`],
/// [`reject_attestation_annotations_that_break_canonical_json`] and
/// [`reject_an_attestation_subject_naming_another_package`] verbatim, with no
/// kind dispatch anywhere and no team-specific attestation media type.
///
/// # Errors
///
/// Returns [`PackageError::Layout`] if the `ImageManifest` fails to build or if
/// any blob/index write fails, [`PackageError::Serialize`] if the package fails
/// to canonicalize, and any gate error from
/// [`validate_single_layer_pack_preconditions`].
///
/// [`PackageError::Serialize`]: crate::error::PackageError::Serialize
fn pack_single_layer<P: SingleLayerPackage>(
    package: &P,
    attestation: Option<AttestationFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    // 1. BYTES. Nothing here touches the filesystem — the same ordering
    //    `pack_server` uses, and what lets the subject gate below compute the
    //    would-be unattested manifest digest before the first write.
    let unattested = [plan_struct_layer(P::LAYER_MEDIA_TYPE, package)?];
    let attestation_layer = attestation.map(plan_attestation_layer);

    // 2. GATES. Any refusal returns here, with the destination layout still
    //    byte-for-byte as it was found.
    validate_single_layer_pack_preconditions(attestation, &unattested, P::ARTIFACT_TYPE)?;

    // 3. WRITES. The attestation goes LAST so the push order stays
    //    deterministic. As on the server path that order is explicitly NOT a
    //    read-order contract — every layer is located by media type.
    let mut layers = Vec::with_capacity(unattested.len() + 1);
    for planned in unattested.iter().chain(attestation_layer.iter()) {
        layers.push(write_planned_layer(layout, planned)?);
    }

    finalize_pack(
        layout,
        layers,
        P::ARTIFACT_TYPE,
        package.name(),
        package.version(),
    )
}

/// Pack an `AgentPackage` into `layout` as a single-layer local OCI artifact.
/// Returns the canonical manifest digest.
///
/// An agent package carries NO attestation, by decision rather than by
/// omission: see `pack_single_layer`'s "attestation carriage covers server
/// and team packages only" section for why, and what to do instead (wrap the
/// agent as a team of one).
pub fn pack_agent(package: &AgentPackage, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, None, layout)
}

/// Pack a `TeamPackage` into `layout` as a local OCI artifact — one config
/// layer, plus an OPTIONAL attestation layer. Returns the canonical manifest
/// digest.
///
/// # The attestation layer is OPTIONAL, and it carries TWO digests
///
/// Exactly as on [`pack_server`], and through the same code: `Some` writes the
/// payload bytes VERBATIM to their own [`MT_ATTESTATION`] layer — never
/// canonicalized, never parsed — with subject, issuer and payload media type as
/// annotations on that LAYER's descriptor. `None` writes no layer at all, and
/// absence is exactly the layer's absence with no marker (D-14).
///
/// An attested team's own manifest digest therefore NECESSARILY DIFFERS from
/// the unattested digest its attestation names as a subject (D-01). See
/// [`pack_server`]'s "two digests" section for the full reasoning and the
/// rejected alternative.
///
/// [`MT_ATTESTATION`]: crate::oci::media_types::MT_ATTESTATION
///
/// # An attestation implies a RESOLVED team (D-09)
///
/// Attaching an attestation to a team holding any `ComponentRef::Range` is
/// refused BEFORE the first write, naming the offending component and its
/// component type. Packing that same team WITHOUT an attestation still
/// succeeds — the guard is scoped to the CLAIM, not to the format.
///
/// The guard is ONE LEVEL DEEP: it covers this team's own four reference
/// surfaces and cannot see inside a pinned component's own references. See
/// `reject_an_attestation_over_an_unresolved_team` for why, and for what a
/// passing pack does and does not prove.
///
/// # Errors
///
/// Returns [`PackageError::InvalidReference`] when an attestation is supplied
/// over a team whose references are not all pinned,
/// [`PackageError::AttestationAnnotationInvalid`] when its `issuer` or
/// `payload_type` carries a C0 control character,
/// [`PackageError::MalformedDigest`] when its `subject` is not a well-formed
/// `sha256:<64 hex>` string, and [`PackageError::AttestationSubjectMismatch`]
/// when the subject is well-formed but names another package — all BEFORE
/// anything is written. Also returns [`PackageError::Layout`] or
/// [`PackageError::Serialize`] on a write or canonicalization failure.
///
/// [`PackageError::InvalidReference`]: crate::error::PackageError::InvalidReference
/// [`PackageError::AttestationAnnotationInvalid`]: crate::error::PackageError::AttestationAnnotationInvalid
/// [`PackageError::MalformedDigest`]: crate::error::PackageError::MalformedDigest
/// [`PackageError::AttestationSubjectMismatch`]: crate::error::PackageError::AttestationSubjectMismatch
/// [`PackageError::Serialize`]: crate::error::PackageError::Serialize
pub fn pack_team(
    package: &TeamPackage,
    attestation: Option<AttestationFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    // Gate A runs here rather than inside `validate_single_layer_pack_preconditions`
    // because it needs a `&TeamPackage` to inspect four `ComponentRef` surfaces,
    // and that helper is generic over every single-layer kind — hosting it there
    // would mean a kind-dispatch hook on `SingleLayerPackage` whose agent and
    // workflow impls could never run. The pre-write invariant is unaffected:
    // this function writes nothing of its own before delegating, so a Gate-A
    // refusal still leaves the destination layout byte-for-byte as it was found.
    //
    // It also runs FIRST because it is the cheapest of the three gates: it reads
    // the typed team's own reference surfaces and needs no dry manifest digest
    // at all, unlike the subject gate that follows.
    reject_an_attestation_over_an_unresolved_team(package, attestation)?;
    pack_single_layer(package, attestation, layout)
}

/// Pack a `WorkflowManifest` into `layout` as a single-layer local OCI
/// artifact. Returns the canonical manifest digest.
///
/// A workflow manifest carries NO attestation, by decision rather than by
/// omission: see `pack_single_layer`'s "attestation carriage covers server
/// and team packages only" section.
pub fn pack_workflow(package: &WorkflowManifest, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, None, layout)
}

/// Build the `ImageManifest` for a package: OCI schema version, the standard
/// `ImageManifest` media type, the kind's `artifactType`, the (non-null)
/// empty-config descriptor and the layer descriptors.
///
/// Pure — it writes nothing — and deliberately the ONE place the builder chain
/// lives. [`finalize_pack`] calls it with the descriptors it just WROTE, and
/// [`would_be_unattested_manifest_digest`] calls it with descriptors it merely
/// DESCRIBED. Sharing the helper is what guarantees the two manifests can
/// differ only in their layer vectors; a second copy of the chain would let
/// them drift in `artifactType`, media type or config and silently invalidate
/// the pack-time subject comparison.
///
/// # Errors
///
/// Returns [`PackageError::Layout`] if the builder rejects the combination.
fn assemble_manifest(
    config: Descriptor,
    layers: Vec<Descriptor>,
    artifact_type: &str,
) -> Result<ImageManifest> {
    ImageManifestBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .media_type(MediaType::ImageManifest)
        .artifact_type(MediaType::Other(artifact_type.to_string()))
        .config(config)
        .layers(layers)
        .build()
        .map_err(|e| PackageError::Layout {
            reason: format!("failed to build ImageManifest: {e}"),
        })
}

/// Shared steps 3-6: write the standard empty-config blob, build the
/// `ImageManifest` (config + `layers` + `artifact_type`), store it under its
/// OWN canonical-bytes digest (step 4-5 — this IS the returned
/// [`ManifestDigest`], not a separate re-derivation), record it in
/// `index.json` with `name`/`version` annotations, and return that digest.
fn finalize_pack(
    layout: &OciLayout,
    layers: Vec<Descriptor>,
    artifact_type: &str,
    name: &str,
    version: &semver::Version,
) -> Result<ManifestDigest> {
    let config_descriptor =
        layout.write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)?;

    let manifest = assemble_manifest(config_descriptor, layers, artifact_type)?;

    // Canonicalize (not plain serde_json) so the stored blob's own
    // content-addressed digest doubles as the identity digest —
    // one hash, one source of truth (RESEARCH steps 4-5).
    let manifest_bytes = canonicalize(&manifest)?;
    let mut manifest_descriptor = layout.write_manifest(&manifest_bytes)?;

    let annotations = HashMap::from([
        ("name".to_string(), name.to_string()),
        ("version".to_string(), version.to_string()),
    ]);
    manifest_descriptor.set_annotations(Some(annotations));

    let manifest_digest = ManifestDigest::try_from(manifest_descriptor.digest())?;

    let mut index = layout.read_index()?;
    let mut manifests = index.manifests().clone();
    manifests.push(manifest_descriptor);
    index.set_manifests(manifests);
    layout.write_index(&index)?;

    Ok(manifest_digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oci::media_types::{
        MT_EMPTY_CONFIG, MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_OPENAPI_SPEC,
    };
    use crate::oci::unpack::tests_support::sample_server_package;

    #[test]
    fn pack_server_writes_bootstrap_layer_with_vendor_media_type() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let manifest_descriptor = &index.manifests()[0];
        let manifest = layout.read_manifest(manifest_descriptor).unwrap();
        assert_eq!(
            manifest.layers()[0].media_type().to_string(),
            MT_SERVER_BOOTSTRAP
        );
        assert_eq!(manifest.layers()[0].size(), bootstrap.len() as u64);
    }

    #[test]
    fn pack_server_referenced_writes_binary_ref_layer_and_no_bootstrap_layer() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, _bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Referenced {
                digest: ManifestDigest::from_bytes(b"referenced-binary"),
                media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
            },
            None,
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
        assert!(media_types.iter().any(|m| m == MT_SERVER_BINARY_REF));
        assert!(!media_types.iter().any(|m| m == MT_SERVER_BOOTSTRAP));
    }

    #[test]
    fn pack_server_writes_the_supplied_spec_as_its_own_annotated_layer() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            Some(OpenApiSpecFile {
                file_name: "openapi.yaml",
                bytes: b"openapi: 3.1.0",
            }),
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        let spec_layer = manifest
            .layers()
            .iter()
            .find(|l| l.media_type().to_string() == MT_SERVER_OPENAPI_SPEC)
            .expect("a supplied spec must become its own layer");
        assert_eq!(spec_layer.size(), b"openapi: 3.1.0".len() as u64);
        assert_eq!(
            spec_layer
                .annotations()
                .as_ref()
                .and_then(|a| a.get(ANNOTATION_TITLE))
                .map(String::as_str),
            Some("openapi.yaml"),
            "the author's file name must ride in the descriptor's title annotation"
        );
    }

    #[test]
    fn pack_server_writes_no_spec_layer_when_the_caller_supplies_none() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        assert!(
            !manifest
                .layers()
                .iter()
                .any(|l| l.media_type().to_string() == MT_SERVER_OPENAPI_SPEC),
            "absence of a spec is the absence of the layer — never an absence marker (D-14)"
        );
    }

    #[test]
    fn pack_uses_standard_non_null_empty_config_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        assert_eq!(manifest.config().media_type().to_string(), MT_EMPTY_CONFIG);
        assert_eq!(manifest.config().size(), 2);
    }

    #[test]
    fn packing_the_same_package_twice_yields_an_identical_manifest_digest() {
        let (package, bootstrap) = sample_server_package();

        let dir_a = tempfile::tempdir().unwrap();
        let layout_a = OciLayout::create(dir_a.path()).unwrap();
        let digest_a = pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout_a,
        )
        .unwrap();

        let dir_b = tempfile::tempdir().unwrap();
        let layout_b = OciLayout::create(dir_b.path()).unwrap();
        let digest_b = pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout_b,
        )
        .unwrap();

        assert_eq!(
            digest_a, digest_b,
            ": packing identical input must yield an identical digest"
        );
    }

    #[test]
    fn index_json_records_manifest_with_name_and_version_annotations() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let annotations = index.manifests()[0].annotations().as_ref().unwrap();
        assert_eq!(annotations.get("name"), Some(&package.name));
        assert_eq!(
            annotations.get("version"),
            Some(&package.version.to_string())
        );
    }
}
