//! Stable team-member identity derived from a member's [`pmcp_package::ComponentRef`].
//!
//! A [`pmcp_package::TeamMember`] has NO separate stable-id or display-name
//! field — its identity IS the `agent` `ComponentRef`. [`MemberId`] derives a
//! stable, hashable identity string from that reference so member lookup and
//! the self-call / ancestor-cycle guards (109-05) compare IDs, never names.
//!
//! Also defines [`MemberTaskForwarding`], the 109-05 member-task-forwarding
//! contract choice. Implemented atomically here (109-01) as a stable seam.

use std::fmt;

use pmcp_package::ComponentRef;

/// A stable team-member identity string, derived from the member's
/// `ComponentRef` as `"{name}@{version}"`.
///
/// Because a team can legitimately contain two same-named members pinned to
/// different versions, the version discriminator keeps them distinct. Identity
/// IS the `ComponentRef` — there is no separate id field on `TeamMember`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MemberId(String);

impl MemberId {
    /// Derive a stable identity from a member's component reference.
    ///
    /// Uses the exact pinned version for [`ComponentRef::Pinned`] and the
    /// capture-time range for [`ComponentRef::Range`], so same-name members at
    /// different versions/ranges are distinct identities.
    #[must_use]
    pub fn from_ref(r: &ComponentRef) -> Self {
        let version = match r {
            ComponentRef::Pinned(pinned) => pinned.version.to_string(),
            ComponentRef::Range { range, .. } => range.to_string(),
        };
        Self(format!("{}@{}", r.name(), version))
    }

    /// Reconstruct a `MemberId` from its already-serialized wire form.
    ///
    /// Guard state (caller id, ancestor chain) travels across a `tools/call` as
    /// namespaced `_meta` strings, each the `name@version` form produced by
    /// [`MemberId::as_str`] / [`fmt::Display`]. The 109-05 dispatch guards
    /// reconstruct a `MemberId` from those strings so identity comparison stays
    /// id-based (never display-name based). This is the inverse of
    /// [`MemberId::as_str`]: `MemberId::from_wire(id.as_str()) == *id`.
    #[must_use]
    pub fn from_wire(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The identity as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MemberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How team-mcp forwards a member `tools/call` and shapes the caller-visible
/// result (the 109-05 forwarding-contract choice).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MemberTaskForwarding {
    /// Poll the forwarded member task to a terminal state and synthesize a
    /// single synchronous `CallToolResult` carrying the related-task `_meta`
    /// (the default contract).
    #[default]
    Synthesize,
    /// Return the member task envelope to the caller verbatim without polling
    /// it to completion.
    ReturnEnvelope,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_package::reference::{ComponentType, PinnedRef};
    use pmcp_package::ManifestDigest;

    fn range_ref(name: &str, req: &str) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse(req).unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    fn pinned_ref(name: &str, version: &str) -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: name.to_string(),
            component_type: ComponentType::Agent,
            version: semver::Version::parse(version).unwrap(),
            digest: ManifestDigest::from_bytes(b"fixture"),
            resolved_from: None,
        })
    }

    #[test]
    fn same_ref_yields_equal_identity() {
        let a = MemberId::from_ref(&range_ref("triage", "^1"));
        let b = MemberId::from_ref(&range_ref("triage", "^1"));
        assert_eq!(a, b);
    }

    #[test]
    fn same_name_different_version_is_distinct() {
        let v1 = MemberId::from_ref(&pinned_ref("triage", "1.0.0"));
        let v2 = MemberId::from_ref(&pinned_ref("triage", "2.0.0"));
        assert_ne!(v1, v2);
        assert_eq!(v1.as_str(), "triage@1.0.0");
        assert_eq!(v2.as_str(), "triage@2.0.0");
    }

    #[test]
    fn display_matches_as_str() {
        let id = MemberId::from_ref(&pinned_ref("formatter", "0.3.1"));
        assert_eq!(id.to_string(), id.as_str());
        assert_eq!(id.to_string(), "formatter@0.3.1");
    }

    #[test]
    fn from_wire_is_the_inverse_of_as_str() {
        let id = MemberId::from_ref(&pinned_ref("triage", "1.0.0"));
        let round = MemberId::from_wire(id.as_str());
        assert_eq!(round, id);
        assert_eq!(round.as_str(), "triage@1.0.0");
    }

    #[test]
    fn forwarding_defaults_to_synthesize() {
        assert_eq!(
            MemberTaskForwarding::default(),
            MemberTaskForwarding::Synthesize
        );
    }
}
