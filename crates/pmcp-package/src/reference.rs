//! Typed component references.
//!
//! A [`ComponentRef`] encodes the range-vs-pin distinction that makes the
//! "converge on exact digests" property computable:
//!
//! - [`ComponentRef::Range`] — a capture-time semver range (e.g.
//!   `london-tube@^1.2`), used while authoring/discovering components.
//! - [`ComponentRef::Pinned`] — a finalized exact reference, wrapping a
//!   [`PinnedRef`] whose `version` and `digest` fields are both non-`Option`.
//!   A pin can never exist without a digest — this is a STRUCTURAL
//!   guarantee (no field is missing at the type level), not a runtime check
//!   that could be forgotten at some call site.
//!
//!   [`PinnedRef::resolved_from`] (D-10) is the struct's FIRST `Option`, and
//!   it does NOT weaken that guarantee — `version` and `digest` remain
//!   mandatory, so the "a pin can never exist without a digest" claim above
//!   still holds exactly as stated. `resolved_from` is legitimately optional
//!   for a different reason: it records the semver range a pin was resolved
//!   FROM, and a direct pin genuinely had no declared range. A non-`Option`
//!   field would force every direct pin to invent one, which would turn an
//!   honestly-absent fact into a fabricated one.
//!
//! `PinnedRef` is a dedicated struct (not an inline enum-variant struct) so
//! downstream helpers (e.g. `pinned_components() -> Result<Vec<&PinnedRef>>`)
//! can name the pin body directly.
//!
//! ## Component identity (D-D)
//!
//! [`ComponentType`] is a SEPARATE axis from the `kind` discriminator above:
//! `kind` says whether a reference is a range or a pin (a wire-shape
//! concern); `component_type` says WHAT the referenced thing is — a server,
//! an agent, or a team. A workflow graph can legitimately contain a server
//! and an agent that share the same `name` (e.g. an agent named "x" that
//! calls a server also named "x") — without `component_type`, those two
//! pins would be indistinguishable by name alone. Both `ComponentRef`
//! variants (`Range` and `Pinned`) carry a required `component_type` field
//! so identity is unambiguous regardless of range-vs-pin state.

use crate::digest::ManifestDigest;
use serde::{Deserialize, Serialize};

/// What kind of component a [`ComponentRef`] identifies. Ordering
/// (`server < agent < team`, derived from declaration order via `Ord`) is
/// the canonical type-then-name sort key `WorkflowManifest::new` uses (D-D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    Server,
    Agent,
    Team,
}

/// An exact, digest-verified component pin. `version` and `digest` are both
/// mandatory (non-`Option`) — a pin always carries both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedRef {
    pub name: String,
    pub component_type: ComponentType,
    pub version: semver::Version,
    pub digest: ManifestDigest,
    /// The semver range this pin was RESOLVED FROM, when it was resolved from
    /// one. Same type as [`ComponentRef::Range`]'s `range` field, because a
    /// resolution records the same kind of thing the range declared.
    ///
    /// # Why a pin keeps the range it resolved (D-10)
    ///
    /// Cargo's model, adopted deliberately because that logic is proven and
    /// will pass the same security reviews. Cargo keeps TWO artifacts:
    /// `Cargo.toml` holds the range (author intent — what enables reuse and
    /// upgrade), `Cargo.lock` holds the resolution (the exact version plus its
    /// checksum). Pinning never destroys the range.
    ///
    /// Before this field, [`ComponentRef`] was strictly either/or, so pinning
    /// DISCARDED the declared range — losing the one fact the dev-to-prod case
    /// turns on. A target environment could not distinguish *"dev declared
    /// `^1.2` and resolved 1.3.0"* from *"dev declared `=1.2.0`"*. In the first
    /// case prod's already-deployed 1.2.0 still satisfies `^1.2`, so it is
    /// silently kept — the wrong reference, with nothing in the package able to
    /// say so.
    ///
    /// # What `None` means — decided, not left implicit (D-10)
    ///
    /// `None` means "no declared range is recorded on this pin". It is NOT
    /// distinguishable from "this package was packed before this field
    /// existed": the additive serde attributes below make an old pin
    /// deserialize to exactly the value a direct pin produces. The crate
    /// ACCEPTS that ambiguity rather than carrying a schema-version
    /// discriminator — `pmcp-package` is 0.x, and the standing position for the
    /// package tree is to break freely rather than ship compatibility shims.
    ///
    /// Two obligations follow, and both are load-bearing:
    ///
    /// - **Consumer side.** Anything building skew reporting on this field —
    ///   Phase 123's dev-to-prod import check is the named one — MUST treat
    ///   `None` as "cannot report" and NEVER as "no skew". Reading an absent
    ///   fact as a positive claim is precisely the failure this field exists to
    ///   prevent.
    /// - **Producer side.** A producer that resolved a range MUST record it. A
    ///   `None` written by a range-resolving producer is indistinguishable from
    ///   an old package, and destroys the signal for every consumer downstream.
    ///
    /// # Compatibility — both halves, because only stating one under-scopes the next change
    ///
    /// - **Serde/wire: ADDITIVE.** `#[serde(default)]` means pin JSON written
    ///   before this field existed still deserializes (yielding `None`), and
    ///   `skip_serializing_if` means nothing new is emitted for a `None`. No
    ///   checked-in fixture byte and no pinned digest constant moves — measured,
    ///   not assumed: all four `tests/golden_fixtures/canonical/*.json` files
    ///   and all five pinned constants in `tests/digest_stability.rs` are
    ///   byte-identical across this addition, and their tests pass unedited.
    ///   `skip_serializing_if` is load-bearing here, not cosmetic: without it
    ///   every pin would emit `"resolved_from": null`, and both
    ///   `workflow.canonical.json` and `EXPECTED_WORKFLOW_DIGEST` would move.
    ///
    ///   A `Some(range)` DOES change the canonical bytes and therefore the
    ///   manifest digest — see
    ///   `recording_the_range_a_pin_resolved_changes_the_manifest_digest` in
    ///   `tests/digest_stability.rs`. The field participates in package
    ///   identity; it is not cosmetic metadata that could be stripped or forged
    ///   without changing what the package IS.
    /// - **Rust source: BREAKING.** `PinnedRef` had four public fields before
    ///   this, all non-`Option`, all set by struct literal — a fifth field
    ///   breaks every literal in the language, everywhere. The MEASURED
    ///   inventory is EIGHT construction sites (`grep -rn 'PinnedRef {'
    ///   --include="*.rs" crates cargo-pmcp` returns nine hits; the ninth is
    ///   this struct's own `pub struct PinnedRef {` definition, which is not a
    ///   construction site):
    ///
    ///   - `crates/pmcp-package/src/reference.rs` — 4 (all inside
    ///     `#[cfg(test)] mod tests`)
    ///   - `crates/pmcp-package/src/oci/unpack.rs` — 2 (both in a
    ///     `#[cfg(test)]` fixture helper)
    ///   - `crates/pmcp-package/src/package/workflow.rs` — 1 (`#[cfg(test)]`
    ///     helper)
    ///   - `crates/pmcp-team-servers/src/team/identity.rs` — 1 (`#[cfg(test)]`
    ///     helper)
    ///
    ///   A reader who takes "additive" at face value will under-scope the next
    ///   field addition exactly as this one would have been.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_from: Option<semver::VersionReq>,
}

/// A reference to a component: either a capture-time semver range or a
/// finalized exact pin. The `kind` discriminator (`"range"` | `"pinned"`) is
/// the wire-format contract other consumers key off of. `component_type` is
/// a separate, required axis on BOTH variants (D-D) — see module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComponentRef {
    Range {
        name: String,
        range: semver::VersionReq,
        component_type: ComponentType,
    },
    Pinned(PinnedRef),
}

impl ComponentRef {
    /// The component name, regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            ComponentRef::Range { name, .. } => name,
            ComponentRef::Pinned(pinned) => &pinned.name,
        }
    }

    /// The component type, regardless of variant (D-D identity axis).
    pub fn component_type(&self) -> ComponentType {
        match self {
            ComponentRef::Range { component_type, .. } => *component_type,
            ComponentRef::Pinned(pinned) => pinned.component_type,
        }
    }

    /// `true` if this reference is a finalized exact pin.
    pub fn is_pinned(&self) -> bool {
        matches!(self, ComponentRef::Pinned(_))
    }

    /// Borrow the pin body, if this reference is pinned.
    pub fn as_pinned(&self) -> Option<&PinnedRef> {
        match self {
            ComponentRef::Pinned(pinned) => Some(pinned),
            ComponentRef::Range { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_digest() -> ManifestDigest {
        ManifestDigest::from_bytes(b"fixture-bytes")
    }

    #[test]
    fn range_round_trips_with_kind_discriminator() {
        let r = ComponentRef::Range {
            name: "london-tube".to_string(),
            range: semver::VersionReq::parse("^1.2").unwrap(),
            component_type: ComponentType::Server,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("range"));
        assert_eq!(
            json.get("name").and_then(|v| v.as_str()),
            Some("london-tube")
        );
        assert_eq!(
            json.get("component_type").and_then(|v| v.as_str()),
            Some("server")
        );
        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn pinned_round_trips_flat_with_kind_discriminator() {
        let pinned = PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.2.3").unwrap(),
            digest: sample_digest(),
            resolved_from: None,
        };
        let r = ComponentRef::Pinned(pinned.clone());
        let json = serde_json::to_value(&r).unwrap();

        // FLAT shape: {"kind":"pinned","name":...,"version":...,"digest":...}
        // — NOT nested under a "Pinned" key.
        let obj = json.as_object().expect("must serialize as a flat object");
        let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["component_type", "digest", "kind", "name", "version"]
        );
        assert_eq!(obj.get("kind").and_then(|v| v.as_str()), Some("pinned"));
        assert_eq!(
            obj.get("name").and_then(|v| v.as_str()),
            Some("london-tube")
        );
        assert_eq!(obj.get("version").and_then(|v| v.as_str()), Some("1.2.3"));
        assert_eq!(
            obj.get("component_type").and_then(|v| v.as_str()),
            Some("server")
        );
        assert!(obj
            .get("digest")
            .and_then(|v| v.as_str())
            .unwrap()
            .starts_with("sha256:"));

        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn pinned_ref_always_carries_version_and_digest() {
        // Structural test: PinnedRef's fields are non-Option — this compiles
        // only because both fields are mandatory at construction.
        let pinned = PinnedRef {
            name: "x".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("0.1.0").unwrap(),
            digest: sample_digest(),
            resolved_from: None,
        };
        assert_eq!(pinned.version.to_string(), "0.1.0");
        assert!(pinned.digest.as_str().starts_with("sha256:"));
    }

    #[test]
    fn name_accessor_works_for_both_variants() {
        let range = ComponentRef::Range {
            name: "a".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Server,
        };
        assert_eq!(range.name(), "a");
        assert!(!range.is_pinned());
        assert!(range.as_pinned().is_none());

        let pinned = ComponentRef::Pinned(PinnedRef {
            name: "b".to_string(),
            component_type: ComponentType::Agent,
            version: semver::Version::parse("2.0.0").unwrap(),
            digest: sample_digest(),
            resolved_from: None,
        });
        assert_eq!(pinned.name(), "b");
        assert!(pinned.is_pinned());
        assert!(pinned.as_pinned().is_some());
    }

    #[test]
    fn component_type_orders_server_before_agent_before_team() {
        assert!(ComponentType::Server < ComponentType::Agent);
        assert!(ComponentType::Agent < ComponentType::Team);
        assert!(ComponentType::Server < ComponentType::Team);
    }

    /// A pin that records no declared range must emit NO key for it, so every
    /// package written before the field existed stays byte-identical.
    #[test]
    fn a_pin_with_no_resolved_range_emits_exactly_the_original_five_keys() {
        let r = ComponentRef::Pinned(PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.2.3").unwrap(),
            digest: sample_digest(),
            resolved_from: None,
        });
        let json = serde_json::to_value(&r).unwrap();
        let obj = json.as_object().expect("must serialize as a flat object");

        let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["component_type", "digest", "kind", "name", "version"],
            "a `None` resolved_from must emit no key at all — `skip_serializing_if` is what \
             keeps every checked-in fixture and every pinned digest constant unmoved"
        );
        assert!(
            !obj.contains_key("resolved_from"),
            "not even a null: a null would move the canonical bytes"
        );

        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }

    /// The Cargo half (D-10): a pin resolved FROM a range carries that range,
    /// so a target environment can tell `^1.2 -> 1.3.0` from `=1.2.0`.
    #[test]
    fn a_pin_carrying_the_range_it_resolved_emits_it_and_round_trips() {
        let declared = semver::VersionReq::parse("^1.2").unwrap();
        let r = ComponentRef::Pinned(PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.3.0").unwrap(),
            digest: sample_digest(),
            resolved_from: Some(declared.clone()),
        });
        let json = serde_json::to_value(&r).unwrap();
        let obj = json.as_object().expect("must serialize as a flat object");

        let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "component_type",
                "digest",
                "kind",
                "name",
                "resolved_from",
                "version"
            ]
        );
        assert_eq!(
            obj.get("resolved_from").and_then(|v| v.as_str()),
            Some("^1.2"),
            "the declared range is recorded verbatim, not collapsed into the resolution"
        );
        // Both facts survive together: what was asked for AND what was chosen.
        assert_eq!(obj.get("version").and_then(|v| v.as_str()), Some("1.3.0"));

        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.as_pinned().unwrap().resolved_from, Some(declared));
    }

    /// Backward compatibility, asserted against a hand-written JSON literal
    /// rather than a re-serialized value — the literal IS what a package
    /// written before this field existed contains on disk.
    #[test]
    fn pin_json_written_before_resolved_from_existed_deserializes_to_none() {
        let legacy = serde_json::json!({
            "kind": "pinned",
            "name": "london-tube",
            "component_type": "server",
            "version": "1.2.3",
            "digest": sample_digest().as_str(),
        });

        let back: ComponentRef =
            serde_json::from_value(legacy).expect("five-key pin JSON must still deserialize");
        let pin = back.as_pinned().expect("must deserialize as a pin");
        assert_eq!(
            pin.resolved_from, None,
            "an absent key must degrade to a defined value, never a decode failure"
        );
        assert_eq!(pin.version.to_string(), "1.2.3");
    }

    #[test]
    fn component_type_accessor_works_for_both_variants() {
        let range = ComponentRef::Range {
            name: "x".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Team,
        };
        assert_eq!(range.component_type(), ComponentType::Team);

        let pinned = ComponentRef::Pinned(PinnedRef {
            name: "x".to_string(),
            component_type: ComponentType::Agent,
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: sample_digest(),
            resolved_from: None,
        });
        assert_eq!(pinned.component_type(), ComponentType::Agent);
    }
}
