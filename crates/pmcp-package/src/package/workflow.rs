//! `WorkflowManifest` — the pinned, lockfile-style `workflow` AI-Package
//! payload.
//!
//! A workflow manifest is the fully-resolved, deployable form of a team-plus-
//! its-dependency-graph: every component MUST be an exact
//! `ComponentRef::Pinned` (name + version + digest) — a `Range` anywhere in
//! the graph is a validation error, not merely discouraged ("converge
//! on exact digests"). It also carries the pre-aggregated `ConfigSlot`s for
//! the whole graph and capture [`Provenance`].
//!
//! Deliberately NOT `#[serde(deny_unknown_fields)]` on the outer envelope
//! (RESEARCH Pitfall 4): this type is expected to evolve additively across
//! Phases 168-173, and a closed set here would turn every future field
//! addition into a hard breaking change for an older-crate-version reader.

use crate::digest::{manifest_digest, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::reference::{ComponentRef, PinnedRef};
use crate::slot::ConfigSlot;
use serde::{Deserialize, Serialize};

/// Where/when/by-whom a `WorkflowManifest` was captured. Also NOT
/// `deny_unknown_fields` (RESEARCH Pitfall 4 — provenance metadata is
/// allowed to grow).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_environment: String,
    pub capturer: String,
    pub timestamp: String,
    pub root_team_id: String,
}

/// Registry namespacing convention (design spec §7):
/// `pmcp-packages/{type}/{name}` (e.g. `pmcp-packages/workflow/support-triage`).
/// Format-level documentation only — actual ECR repo provisioning is Phase
/// 169's concern.
pub const REGISTRY_NAMESPACE_PATTERN: &str = "pmcp-packages/{type}/{name}";

/// The pinned, lockfile-style `workflow` AI-Package payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowManifest {
    pub name: String,
    pub version: semver::Version,
    pub components: Vec<ComponentRef>,
    /// Pre-aggregated slots with tested values for behavior-relevant slots.
    /// The `slot::aggregate()` call site that PRODUCES this vector is Phase
    /// 170 capture's responsibility — this phase only defines the field that
    /// receives the result.
    pub aggregated_slots: Vec<ConfigSlot>,
    pub provenance: Provenance,
}

impl WorkflowManifest {
    /// Construct a `WorkflowManifest`, sorting `components` by
    /// `(component_type, name)` — canonical type-then-name ordering (D-D) —
    /// and `aggregated_slots` by their `(kind, name)` key, for stable diffs
    /// (CONTEXT "one entry per component" — a workflow manifest should diff
    /// cleanly across captures of the same logical graph).
    ///
    /// Sorting by `component_type` FIRST (server < agent < team, per
    /// [`crate::reference::ComponentType`]'s `Ord`) before `name` is what
    /// makes a same-named server+agent pair (or any other cross-type name
    /// collision) deterministically orderable and unambiguously
    /// identifiable — sorting by name alone cannot distinguish them.
    pub fn new(
        name: String,
        version: semver::Version,
        mut components: Vec<ComponentRef>,
        mut aggregated_slots: Vec<ConfigSlot>,
        provenance: Provenance,
    ) -> Self {
        components.sort_by(|a, b| {
            a.component_type()
                .cmp(&b.component_type())
                .then_with(|| a.name().cmp(b.name()))
        });
        aggregated_slots.sort_by(|a, b| a.slot.key().cmp(&b.slot.key()));
        Self {
            name,
            version,
            components,
            aggregated_slots,
            provenance,
        }
    }

    /// Borrow every component as a [`PinnedRef`], failing with
    /// `PackageError::InvalidReference` if ANY component in the graph is
    /// still a `Range` (a workflow manifest carries only pins).
    pub fn pinned_components(&self) -> Result<Vec<&PinnedRef>> {
        self.components
.iter()
.map(|component| {
                component.as_pinned().ok_or_else(|| PackageError::InvalidReference {
                    reason: format!(
                        "component '{}' is a Range, not a Pin — a WorkflowManifest may only contain exact pins",
                        component.name()
                   ),
                })
            })
.collect()
    }

    /// `Ok(())` iff every component is pinned; `Err(InvalidReference)`
    /// otherwise. A thin boolean-style guard over [`Self::pinned_components`].
    pub fn validate_all_pinned(&self) -> Result<()> {
        self.pinned_components().map(|_| ())
    }

    /// Convenience delegate to [`crate::digest::manifest_digest`] (the
    /// identity key) over this manifest's canonical bytes.
    pub fn manifest_digest(&self) -> Result<ManifestDigest> {
        manifest_digest(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::ComponentType;

    fn sample_pinned(name: &str, version: &str) -> ComponentRef {
        sample_pinned_typed(name, version, ComponentType::Server)
    }

    fn sample_pinned_typed(
        name: &str,
        version: &str,
        component_type: ComponentType,
    ) -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: name.to_string(),
            component_type,
            version: semver::Version::parse(version).unwrap(),
            digest: ManifestDigest::from_bytes(format!("{name}:{component_type:?}").as_bytes()),
            resolved_from: None,
        })
    }

    fn sample_range(name: &str, range: &str) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse(range).unwrap(),
            component_type: ComponentType::Server,
        }
    }

    fn sample_provenance() -> Provenance {
        Provenance {
            source_environment: "dev".to_string(),
            capturer: "cargo-pmcp".to_string(),
            timestamp: "2026-07-16T00:00:00Z".to_string(),
            root_team_id: "support-team".to_string(),
        }
    }

    fn sample_slot() -> ConfigSlot {
        ConfigSlot::new(crate::slot::SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        })
    }

    #[test]
    fn workflow_manifest_round_trips_with_two_pins_and_provenance() {
        let manifest = WorkflowManifest::new(
            "support-triage".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                sample_pinned("triage-agent", "1.2.0"),
                sample_pinned("london-tube", "2.0.1"),
            ],
            vec![sample_slot()],
            sample_provenance(),
        );
        let json = serde_json::to_string(&manifest).unwrap();
        let back: WorkflowManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, manifest);
    }

    #[test]
    fn new_sorts_components_by_type_then_name_for_stable_diffs() {
        let manifest = WorkflowManifest::new(
            "w".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                sample_pinned("zeta", "1.0.0"),
                sample_pinned("alpha", "1.0.0"),
            ],
            vec![],
            sample_provenance(),
        );
        let names: Vec<&str> = manifest.components.iter().map(|c| c.name()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    /// D-D collision test: an agent and a server sharing the SAME name "x"
    /// round-trip with unambiguous identity (distinct `component_type`s) and
    /// sort deterministically — server before agent, per `ComponentType`'s
    /// `Ord` (server < agent < team).
    #[test]
    fn same_named_agent_and_server_are_distinct_and_sort_type_then_name() {
        let manifest = WorkflowManifest::new(
            "w".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                sample_pinned_typed("x", "1.0.0", ComponentType::Agent),
                sample_pinned_typed("x", "2.0.0", ComponentType::Server),
            ],
            vec![],
            sample_provenance(),
        );

        // Both entries survive — same name is NOT deduplicated/collapsed.
        assert_eq!(manifest.components.len(), 2);

        // Deterministic order: server < agent, so the server-typed "x" sorts
        // first even though both share the same name.
        let types: Vec<ComponentType> = manifest
            .components
            .iter()
            .map(|c| c.component_type())
            .collect();
        assert_eq!(types, vec![ComponentType::Server, ComponentType::Agent]);

        // Identity is unambiguous: the two "x" entries differ in
        // component_type (and therefore digest), even though `name` alone
        // cannot distinguish them. (server "x" is version 2.0.0, agent "x"
        // is version 1.0.0 — order above already proved server sorts first.)
        assert_eq!(
            manifest.components[0]
                .as_pinned()
                .unwrap()
                .version
                .to_string(),
            "2.0.0"
        );
        assert_eq!(
            manifest.components[1]
                .as_pinned()
                .unwrap()
                .version
                .to_string(),
            "1.0.0"
        );
        assert_ne!(
            manifest.components[0].as_pinned().unwrap().digest,
            manifest.components[1].as_pinned().unwrap().digest,
            "same-named server and agent must carry distinct digests (unambiguous identity)"
        );

        // Round-trips losslessly through JSON with both entries intact.
        let json = serde_json::to_string(&manifest).unwrap();
        let back: WorkflowManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, manifest);
    }

    #[test]
    fn pinned_components_errors_on_any_range_component() {
        let manifest = WorkflowManifest::new(
            "w".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                sample_pinned("triage-agent", "1.2.0"),
                sample_range("london-tube", "^1.2"),
            ],
            vec![],
            sample_provenance(),
        );
        let err = manifest.pinned_components().unwrap_err();
        assert!(matches!(err, PackageError::InvalidReference { .. }));

        let err = manifest.validate_all_pinned().unwrap_err();
        assert!(matches!(err, PackageError::InvalidReference { .. }));
    }

    #[test]
    fn pinned_components_returns_all_pins_when_fully_pinned() {
        let manifest = WorkflowManifest::new(
            "w".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                sample_pinned("triage-agent", "1.2.0"),
                sample_pinned("london-tube", "2.0.1"),
            ],
            vec![],
            sample_provenance(),
        );
        let pins = manifest.pinned_components().unwrap();
        assert_eq!(pins.len(), 2);
        assert!(manifest.validate_all_pinned().is_ok());
    }

    #[test]
    fn manifest_digest_is_stable_across_repeated_calls() {
        let manifest = WorkflowManifest::new(
            "w".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![sample_pinned("triage-agent", "1.2.0")],
            vec![],
            sample_provenance(),
        );
        let first = manifest.manifest_digest().unwrap();
        let second = manifest.manifest_digest().unwrap();
        assert_eq!(first, second);
    }
}
