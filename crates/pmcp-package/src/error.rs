//! Crate-wide error surface for `pmcp-package`.
//!
//! One flat, structured `PackageError` enum covers every failure mode the
//! crate's modules produce: digest/verify, allowlist enforcement
//!, reference parsing, slot-conflict detection, OCI layout I/O,
//! and (de)serialization. Variants carry concrete structured fields (not bare
//! `String`-wrapped messages), following the `PathError`
//! (`built-in/agents-api/crates/mcp-builtin-server-core/src/path_validator.rs`)
//! and `PolicyManagementError`
//! (`built-in/shared/mcp-server-common/src/code_mode/policy_management.rs`)
//! precedents.
//!
//! There is deliberately NO `SlotDeviation` variant: behavior-relevant
//! deviation detection returns `Option<Deviation>` — a value, not
//! an error — so an error variant for it would be dead code. `SlotConflict`
//! is the one slot-related error variant; it is returned by
//! `slot::aggregate()` when the same behavior-relevant slot (kind+name)
//! carries different tested values across components (a silent
//! discard would mask a behavioral change).
//!
//! `serde_json::Error` has a single type covering both serialize and
//! deserialize failures, so there is exactly one `#[from]` variant
//! (`Serialize`) for it — a second `#[from] serde_json::Error` variant would
//! be a duplicate `impl From` and fail to compile.
//!
//! No `reqwest::Error` variant exists — this crate makes no HTTP calls.

/// Result type alias for this crate.
pub type Result<T> = std::result::Result<T, PackageError>;

/// All ways a `pmcp-package` operation can fail.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    /// JSON (de)serialization failure. Covers both directions — serde_json
    /// has a single `Error` type for both, so there is exactly one variant.
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    /// Filesystem I/O failure (OCI layout read/write, blob read/write).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A recomputed digest did not match the digest declared in a manifest
    /// or descriptor (tamper detection).
    #[error("digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    /// A digest string is not well-formed (e.g. not `sha256:<64-hex>`).
    #[error("malformed digest: {reason}")]
    MalformedDigest { reason: String },

    /// A deploy descriptor requested a CloudFormation resource type not on
    /// the allowlist.
    #[error("allowlist violation: {resource}")]
    AllowlistViolation { resource: String },

    /// A `ComponentRef` string (semver range or pinned+digest form) failed
    /// to parse.
    #[error("invalid reference: {reason}")]
    InvalidReference { reason: String },

    /// The same behavior-relevant slot (kind+name) carried different tested
    /// values across components during aggregation.
    #[error("slot conflict on '{slot}': tested={tested}, proposed={proposed}")]
    SlotConflict {
        slot: String,
        tested: String,
        proposed: String,
    },

    /// An OCI Image Layout on disk was malformed or incomplete (missing
    /// `oci-layout`, missing `index.json`, missing referenced blob, etc.).
    #[error("OCI layout error: {reason}")]
    Layout { reason: String },

    /// A server config declared or supplied a config slot that violates the
    /// package's slot contract (an undeclared key, a missing required key, a
    /// key whose shape does not match its declared slot type).
    ///
    /// `key` names the offending config KEY and `reason` describes the
    /// violation. Neither ever carries the key's VALUE — a config slot may
    /// name a secret, and an error message is the wrong place for one.
    #[error("config slot violation on '{key}': {reason}")]
    ConfigSlotViolation { key: String, reason: String },

    /// An attestation's subject digest does not name the package it was being
    /// packed into: the supplied subject differs from the would-be UNATTESTED
    /// manifest digest of the very package under construction.
    ///
    /// Returned by `pack_server` BEFORE its first blob write, which is what
    /// makes "an attestation attached to the wrong package" unrepresentable in
    /// a produced layout rather than merely reported afterwards.
    ///
    /// Distinct from [`PackageError::DigestMismatch`] on purpose, and the two
    /// must never be merged: a digest mismatch means the BYTES are corrupt,
    /// while a subject mismatch means the bytes are fine and the CLAIM is
    /// wrong.
    ///
    /// `supplied` and `computed` are both `sha256:<hex>` digest strings, and
    /// this variant carries nothing else. It never carries attestation payload
    /// bytes, an issuer, or any other attestation material — the same rule
    /// [`PackageError::ConfigSlotViolation`]'s rustdoc sets for this crate,
    /// applied to a payload that is opaque platform-owned data this crate has
    /// deliberately never parsed.
    ///
    /// # Version consequence
    ///
    /// `PackageError` is NOT `#[non_exhaustive]`, so adding this variant is a
    /// breaking change for every downstream `match` over it. Plan 122-08 owns
    /// the version number that names that break; this crate publishes nothing
    /// on its own account.
    #[error(
        "attestation subject mismatch: the attestation names {supplied}, but this package's \
         unattested manifest digest is {computed}"
    )]
    AttestationSubjectMismatch {
        /// The `sha256:<hex>` subject the caller supplied with the attestation.
        supplied: String,
        /// The `sha256:<hex>` unattested manifest digest actually computed for
        /// the package being packed.
        computed: String,
    },

    /// An attestation's annotation value cannot survive this crate's canonical
    /// JSON form, so packing it would produce a manifest no JSON parser can
    /// read back.
    ///
    /// Canonical JSON (OLPC/TUF, which `olpc-cjson` implements and this crate's
    /// `canonicalize` uses) escapes ONLY `"` and `\`. A C0 control character
    /// (U+0000–U+001F) in any string it emits is written LITERALLY, and RFC 8259
    /// forbids an unescaped control character inside a JSON string — so the
    /// manifest bytes would be invalid JSON. The package would pack cleanly and
    /// then fail to unpack, here and in every other OCI tool.
    ///
    /// Refused BEFORE the first blob write, for the same reason
    /// [`PackageError::AttestationSubjectMismatch`] is: an unreadable package is
    /// worse to produce and diagnose later than to refuse now. Attestation
    /// annotation values arrive from the issuing platform rather than from this
    /// repo, which is precisely why they are validated rather than trusted.
    ///
    /// `annotation` names the offending annotation KEY and `reason` describes
    /// the offending code point and where it sits. Neither ever carries the
    /// annotation's VALUE — the same rule
    /// [`PackageError::ConfigSlotViolation`] sets, applied to untrusted input
    /// that may be arbitrarily long or itself hostile to a terminal.
    ///
    /// # Version consequence
    ///
    /// `PackageError` is NOT `#[non_exhaustive]`, so adding this variant is a
    /// breaking change for every downstream `match` over it. Plan 122-08 owns
    /// the version number that names that break, together with the sibling
    /// break [`PackageError::AttestationSubjectMismatch`] introduced.
    #[error("attestation annotation '{annotation}' is not representable: {reason}")]
    AttestationAnnotationInvalid {
        /// The annotation KEY whose value was refused, e.g.
        /// `run.pmcp.attestation.issuer`.
        annotation: String,
        /// What is wrong with the value, naming the code point and its byte
        /// offset but never reproducing the value itself.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_mismatch_display_contains_structured_fields() {
        let err = PackageError::DigestMismatch {
            expected: "sha256:aaaa".to_string(),
            actual: "sha256:bbbb".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("sha256:aaaa"), "message was: {msg}");
        assert!(msg.contains("sha256:bbbb"), "message was: {msg}");
    }

    #[test]
    fn slot_conflict_display_contains_structured_fields() {
        let err = PackageError::SlotConflict {
            slot: "llm-provider".to_string(),
            tested: "anthropic".to_string(),
            proposed: "openai".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("llm-provider"), "message was: {msg}");
        assert!(msg.contains("anthropic"), "message was: {msg}");
        assert!(msg.contains("openai"), "message was: {msg}");
    }

    #[test]
    fn serialize_variant_wraps_serde_json_error_via_from() {
        let json_err = serde_json::from_str::<serde_json::Value>("{not valid json")
            .expect_err("must fail to parse");
        let err: PackageError = json_err.into();
        assert!(matches!(err, PackageError::Serialize(_)));
    }
}
