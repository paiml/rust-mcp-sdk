//! # pmcp-package
//!
//! The AI-Package format for portable MCP packages: typed manifest schemas,
//! config-slot aggregation, local OCI pack/unpack, and canonical-digest
//! computation.
//!
//! ## Dual-consumer contract
//!
//! This crate is consumed by **two** independent call sites:
//! - `cargo-pmcp` (the CLI) — packs local server/agent/team/workflow artifacts
//!   for publish.
//! - The `pmcp.run` platform — unpacks and validates packages at
//!   import/pre-flight time.
//!
//! Both call sites MUST resolve identical validation/digest behavior from the
//! same crate code — that is the entire point of this crate existing as a
//! shared, standalone, publishable library rather than being duplicated in
//! each consumer.
//!
//! ## Scope fence
//!
//! This crate is **format only**:
//! - Typed manifest schemas for the four package kinds (mcp-server, agent,
//!   team, workflow).
//! - Config-slot type system: classification, aggregation, deviation
//!   detection.
//! - Local OCI artifact pack/unpack (construct manifests + content-addressed
//!   blobs on disk — no registry calls).
//! - Canonical-digest computation for approval-record keying.
//! - A DELIBERATELY NARROW read of a server's own config document
//!   (`oci::config_validation`): it reads the `[[config_slots]]` declaration
//!   table and resolves the dotted TOML path each declared slot names, so
//!   `pack_server` can refuse a package whose slot list disagrees with the
//!   config it ships, or whose slot-declared value key holds a resolved
//!   literal. That is the whole extent of it — the crate models no config
//!   schema, holds no opinion about the toolkit's config shape beyond those
//!   two things, and never re-serializes the document (config layer bytes stay
//!   verbatim).
//!
//! It explicitly does **NOT** contain:
//! - Agent runtime semantics (no execution, no LLM calls, no tool dispatch).
//! - Network or AWS SDK dependencies (no `reqwest`, no `tokio`, no
//!   `oci-client`, no `aws-sdk-*` crate). Registry push/pull is a caller-side
//!   concern at the *caller's* call site, not this crate's.
//! - Secret **values** — config slots may declare that a secret is required
//!   by *name*, but the crate's types are structurally incapable of holding
//!   a resolved secret value (see `slot` module docs).

pub mod digest;
pub mod error;
pub mod oci;
pub mod package;
pub mod reference;
pub mod slot;
pub mod validation;

// ---------------------------------------------------------------------
// Crate-root re-exports (dual-consumer ergonomics)
// ---------------------------------------------------------------------
//
// `cargo-pmcp` and the `pmcp.run` platform both consume this crate's primary
// types and functions without needing deep module paths (`pmcp_package::ServerPackage`
// rather than `pmcp_package::package::ServerPackage`). Deep paths still work —
// this block is additive, not a replacement for the module tree.

pub use digest::{canonicalize, manifest_digest, verify, ManifestDigest};
pub use error::{PackageError, Result};
pub use oci::{
    pack_agent, pack_server, pack_team, pack_workflow, parse_declared_config_slots, unpack_agent,
    unpack_server, unpack_team, unpack_workflow, validate_config_slot_agreement,
    validate_config_slot_placeholders, validate_no_undeclared_env_refs, AttestationFile,
    BinaryMode, ConfigFile, DeclaredConfigSlot, OciLayout, OpenApiSpecFile, RestoredFile,
    SubjectVerdict, UnpackedAttestation, UnpackedBinary, UnpackedServer, UnpackedTeam,
};
pub use package::{
    AgentPackage, CedarPolicySet, DeployDescriptor, ServerPackage, TeamPackage, WorkflowManifest,
};
pub use reference::{ComponentRef, PinnedRef};
pub use slot::{
    aggregate, classify, detect_deviation, required_slots, ConfigSlot, Deviation, RequiredSlot,
    SlotClass, SlotType,
};
pub use validation::validate as validate_deploy_descriptor;
