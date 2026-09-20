//! `TeamPackage` — the captured `AgentTeam` roster + human-role declarations
//!.
//!
//! Field mapping is drawn from `amplify/data/resource.ts:1591-1785`'s
//! `AgentTeam`/`TeamMember`/`TeamHumanMember` models. The one hard structural
//! rule (permanent non-goal §12): a captured `TeamPackage` NEVER carries
//! a human identity. The source `TeamHumanMember` row DOES carry a real
//! `userId`/`channelId`/`channelAddress`/`pendingApproverEmail`, but capture
//! strips those down to a role declaration — [`HumanRole`] structurally
//! cannot represent an identity field; there is no field to accidentally
//! populate with one.

use crate::error::{PackageError, Result};
use crate::reference::{ComponentRef, PinnedRef};
use crate::slot::{ConfigSlot, SlotType};
use serde::{Deserialize, Serialize};

/// A team member's role within the roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    EntryPoint,
    Member,
}

/// One agent-roster entry: which agent (by capture-time range — see
/// `WorkflowManifest` for the pinned form), and its role in the team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent: ComponentRef,
    pub role: TeamRole,
}

/// A human role a team needs filled — role declaration ONLY. This
/// struct's field list is closed and MUST NEVER gain a `user_id`/
/// `channel_id`/`email`/`channel_address` (or any other identity) field: the
/// source `TeamHumanMember` row's identity fields are resolved at BIND time
/// (when a real human is assigned this role in a live team), never at
/// capture/package time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanRole {
    /// The role label (from `TeamHumanMember.roleLabel`).
    pub role: String,
    /// A human-readable description (from `TeamHumanMember.toolDescription`).
    pub description: String,
    /// What this role is expected to cover.
    pub responsibilities: Vec<String>,
    /// Display-only hints about suitable channel kinds for this role.
    pub channel_hints: Vec<String>,
}

impl HumanRole {
    /// Map this role declaration into its `ConfigSlot` representation
    /// (`SlotType::HumanRole`) — "human seats become human-role config
    /// slots".
    pub fn to_config_slot(&self) -> ConfigSlot {
        ConfigSlot::new(SlotType::HumanRole {
            role: self.role.clone(),
            description: self.description.clone(),
            responsibilities: self.responsibilities.clone(),
            channel_hints: self.channel_hints.clone(),
        })
    }
}

/// Team-wide default limits (from `AgentTeam.limits`'s JSON document).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamLimits {
    pub max_team_depth: i64,
    pub max_team_total_tokens: i64,
    pub max_team_wall_clock_seconds: i64,
    pub poll_interval_ms: i64,
}

/// The captured `team` AI-Package payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamPackage {
    pub name: String,
    pub version: semver::Version,
    /// The team's entry-point agent (from `AgentTeam.entryPointAgentId`).
    pub entry_point: ComponentRef,
    pub members: Vec<TeamMember>,
    ///: role declarations only — see [`HumanRole`] doc.
    pub human_roles: Vec<HumanRole>,
    pub limits: TeamLimits,
    /// Built-in servers available to every member (from
    /// `AgentTeam.builtInServerIds`).
    pub built_in_servers: Vec<ComponentRef>,
    /// Finalizer agents (from `AgentTeam.finalizerAgentIds`).
    pub finalizer_agents: Vec<ComponentRef>,
    /// Budget-override DEFAULTS captured at test time (same
    /// deviation-on-override semantics as `AgentPackage::budget_defaults`).
    pub budget_defaults: Vec<ConfigSlot>,
    /// All declared config slots for the team as a whole, INCLUDING each
    /// `human_roles` entry's `to_config_slot()` mapping.
    pub config_slots: Vec<ConfigSlot>,
}

impl TeamPackage {
    /// Every `ComponentRef` this team holds, in a FIXED traversal order:
    /// `entry_point`, then each `members[].agent` in roster order, then
    /// `built_in_servers`, then `finalizer_agents`.
    ///
    /// The order is fixed (rather than sorted) deliberately: a stable traversal
    /// is cheaper than a sort, and the caller of [`Self::pinned_components`]
    /// needs identification, not ordering.
    fn component_refs(&self) -> impl Iterator<Item = &ComponentRef> {
        std::iter::once(&self.entry_point)
            .chain(self.members.iter().map(|member| &member.agent))
            .chain(self.built_in_servers.iter())
            .chain(self.finalizer_agents.iter())
    }

    /// Borrow every one of this team's component references as a
    /// [`PinnedRef`], failing with `PackageError::InvalidReference` if ANY of
    /// them is still a `Range`.
    ///
    /// This is `WorkflowManifest::pinned_components`'s guard generalized to a
    /// team's FOUR reference surfaces — `entry_point`, each `members[].agent`,
    /// `built_in_servers` and `finalizer_agents` — traversed in that fixed
    /// order, so the returned order is deterministic for a given input. The
    /// error reuses `InvalidReference` and names both the offending component
    /// and its `component_type`, because a team can legitimately hold a server
    /// and an agent sharing one name and the name alone would not identify
    /// which failed.
    ///
    /// # The guard is ONE LEVEL DEEP (D-09) — state it, do not discover it
    ///
    /// It checks THIS team's own four surfaces and nothing beyond them. A
    /// `TeamMember.agent` pins an agent by digest, and that digest covers the
    /// agent package's own contents INCLUDING its
    /// `connectors: Vec<ComponentRef>`, which may themselves be ranges. The
    /// team package holds only a digest, and this crate is forbidden a registry
    /// client, so nothing here can resolve a referenced package offline to look
    /// inside it.
    ///
    /// Closing that transitively is platform ADMISSION POLICY — requiring every
    /// pinned component to itself be attested — not SDK work. A team that
    /// passes this guard is resolved at its own level; it is NOT transitively
    /// resolved, and must not be read as such. The same sentence appears in the
    /// error a caller sees, so the limit is visible without reading this doc.
    ///
    /// # Errors
    ///
    /// `PackageError::InvalidReference` naming the first unresolved component
    /// encountered in traversal order, its `component_type`, and the depth
    /// limit above.
    pub fn pinned_components(&self) -> Result<Vec<&PinnedRef>> {
        self.component_refs()
            .map(|component| {
                component
                    .as_pinned()
                    .ok_or_else(|| PackageError::InvalidReference {
                        reason: format!(
                            "component '{}' ({:?}) is a Range, not a Pin — an attested \
                             TeamPackage may only contain exact pins. This guard is one \
                             level deep: it covers this team's own entry_point, members' \
                             agents, built_in_servers and finalizer_agents, and cannot see \
                             inside a pinned component's own references.",
                            component.name(),
                            component.component_type(),
                        ),
                    })
            })
            .collect()
    }

    /// `Ok(())` iff every one of this team's four reference surfaces is
    /// pinned; `Err(InvalidReference)` otherwise. A thin boolean-style guard
    /// over [`Self::pinned_components`], matching `WorkflowManifest`'s pair.
    ///
    /// Carries the SAME one-level depth limit — see
    /// [`Self::pinned_components`]. `Ok(())` means this team's own references
    /// are resolved, NOT that its dependency graph is transitively resolved.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::pinned_components`]'s error unchanged.
    pub fn validate_all_pinned(&self) -> Result<()> {
        self.pinned_components().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ManifestDigest;
    use crate::reference::{ComponentType, PinnedRef};

    fn sample_human_role() -> HumanRole {
        HumanRole {
            role: "approver".to_string(),
            description: "Approves budget overrides".to_string(),
            responsibilities: vec!["review".to_string(), "approve".to_string()],
            channel_hints: vec!["slack".to_string()],
        }
    }

    fn sample_team_package() -> TeamPackage {
        let entry_point = ComponentRef::Range {
            name: "triage-agent".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        };
        let human_role = sample_human_role();
        TeamPackage {
            name: "support-team".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            entry_point: entry_point.clone(),
            members: vec![TeamMember {
                agent: entry_point,
                role: TeamRole::EntryPoint,
            }],
            human_roles: vec![human_role.clone()],
            limits: TeamLimits {
                max_team_depth: 3,
                max_team_total_tokens: 200_000,
                max_team_wall_clock_seconds: 600,
                poll_interval_ms: 2000,
            },
            built_in_servers: vec![ComponentRef::Range {
                name: "team-fs".to_string(),
                range: semver::VersionReq::parse("^1").unwrap(),
                component_type: ComponentType::Server,
            }],
            finalizer_agents: vec![],
            budget_defaults: vec![],
            config_slots: vec![human_role.to_config_slot()],
        }
    }

    fn sample_pinned(name: &str, component_type: ComponentType) -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: name.to_string(),
            component_type,
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: ManifestDigest::from_bytes(name.as_bytes()),
            resolved_from: None,
        })
    }

    fn sample_range(name: &str, component_type: ComponentType) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type,
        }
    }

    /// The shipped [`sample_team_package`] carries `Range` refs on
    /// `entry_point`, `members[0].agent` and `built_in_servers[0]`, and an
    /// EMPTY `finalizer_agents` — so it can prove a failure but never a pass,
    /// and it could not detect a traversal that skipped the finalizer surface.
    /// This variant pins all four surfaces and populates every one of them,
    /// with FIVE total references so an omitted surface changes the count.
    fn fully_pinned_team_package() -> TeamPackage {
        let mut team = sample_team_package();
        team.entry_point = sample_pinned("triage-agent", ComponentType::Agent);
        team.members = vec![
            TeamMember {
                agent: sample_pinned("triage-agent", ComponentType::Agent),
                role: TeamRole::EntryPoint,
            },
            TeamMember {
                agent: sample_pinned("reviewer-agent", ComponentType::Agent),
                role: TeamRole::Member,
            },
        ];
        team.built_in_servers = vec![sample_pinned("team-fs", ComponentType::Server)];
        team.finalizer_agents = vec![sample_pinned("formatter-agent", ComponentType::Agent)];
        team
    }

    /// The total number of `ComponentRef`s across all four surfaces of
    /// [`fully_pinned_team_package`], computed from the value rather than
    /// hardcoded so the helper and the assertion cannot drift apart.
    fn total_reference_count(team: &TeamPackage) -> usize {
        1 + team.members.len() + team.built_in_servers.len() + team.finalizer_agents.len()
    }

    #[test]
    fn pinned_components_returns_every_pin_from_all_four_surfaces() {
        let team = fully_pinned_team_package();
        let expected = total_reference_count(&team);
        assert!(
            expected > 3,
            "the fixture must carry more references than it has surfaces, or an omitted \
             surface would not change the count"
        );

        let pins = team
            .pinned_components()
            .expect("a fully pinned team must pass");
        assert_eq!(
            pins.len(),
            expected,
            "every reference across entry_point, members[].agent, built_in_servers and \
             finalizer_agents must be returned — a short vector means a surface was skipped"
        );

        // The chain order is fixed and documented, so it can be asserted.
        let names: Vec<&str> = pins.iter().map(|pin| pin.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "triage-agent",
                "triage-agent",
                "reviewer-agent",
                "team-fs",
                "formatter-agent"
            ]
        );
    }

    #[test]
    fn a_range_entry_point_fails_and_names_that_component() {
        let mut team = fully_pinned_team_package();
        team.entry_point = sample_range("unresolved-entry", ComponentType::Agent);

        let err = team
            .pinned_components()
            .expect_err("an unresolved entry_point must fail the guard");
        assert!(matches!(err, PackageError::InvalidReference { .. }));
        let rendered = err.to_string();
        assert!(
            rendered.contains("unresolved-entry"),
            "the error must name the offending component; got: {rendered}"
        );
        assert!(
            rendered.contains("Agent"),
            "the error must name the component_type, so a same-named server and agent are \
             distinguishable; got: {rendered}"
        );
    }

    /// The traversal must not stop at `entry_point`: this team's ONLY
    /// unresolved reference lives in `built_in_servers`.
    #[test]
    fn a_range_built_in_server_fails_and_names_that_component() {
        let mut team = fully_pinned_team_package();
        team.built_in_servers = vec![sample_range("unresolved-server", ComponentType::Server)];

        let err = team
            .pinned_components()
            .expect_err("an unresolved built_in_server must fail the guard");
        let rendered = err.to_string();
        assert!(
            rendered.contains("unresolved-server"),
            "a traversal that stopped at entry_point would never reach this; got: {rendered}"
        );
        assert!(rendered.contains("Server"), "got: {rendered}");
    }

    /// Same proof for the `members[].agent` surface, and on the SECOND member
    /// so a traversal reading only `members[0]` is caught too.
    #[test]
    fn a_range_member_agent_fails_and_names_that_component() {
        let mut team = fully_pinned_team_package();
        team.members[1].agent = sample_range("unresolved-member", ComponentType::Agent);

        let err = team
            .pinned_components()
            .expect_err("an unresolved member agent must fail the guard");
        let rendered = err.to_string();
        assert!(
            rendered.contains("unresolved-member"),
            "a traversal that stopped earlier would never reach this; got: {rendered}"
        );
        assert!(rendered.contains("Agent"), "got: {rendered}");
    }

    /// The fourth surface. This is also the test the falsifiability control
    /// targets: drop the `finalizer_agents` link from the chain and this test
    /// fails (the team wrongly passes).
    #[test]
    fn a_range_finalizer_agent_fails_and_names_that_component() {
        let mut team = fully_pinned_team_package();
        team.finalizer_agents = vec![sample_range("unresolved-finalizer", ComponentType::Agent)];

        let err = team
            .pinned_components()
            .expect_err("an unresolved finalizer agent must fail the guard");
        let rendered = err.to_string();
        assert!(
            rendered.contains("unresolved-finalizer"),
            "finalizer_agents is the last link in the chain and the easiest to omit; \
             got: {rendered}"
        );
    }

    #[test]
    fn validate_all_pinned_passes_a_pinned_team_and_fails_any_unpinned_one() {
        let pinned = fully_pinned_team_package();
        assert!(pinned.validate_all_pinned().is_ok());

        // The shipped sample carries Range refs on three of its four surfaces.
        let unpinned = sample_team_package();
        let err = unpinned
            .validate_all_pinned()
            .expect_err("a team holding any Range must fail");
        assert!(matches!(err, PackageError::InvalidReference { .. }));
    }

    /// D-09's depth limit must be visible to a CALLER, not only to a reader of
    /// the source — so it lives in the error text as well as the rustdoc.
    #[test]
    fn the_error_states_the_one_level_depth_limit() {
        let mut team = fully_pinned_team_package();
        team.entry_point = sample_range("unresolved-entry", ComponentType::Agent);

        let rendered = team.pinned_components().unwrap_err().to_string();
        assert!(
            rendered.contains("one level deep"),
            "a caller must not mistake a passing team for a transitively resolved one; \
             got: {rendered}"
        );
    }

    #[test]
    fn the_returned_pin_order_is_deterministic_across_runs() {
        let team = fully_pinned_team_package();
        let first: Vec<String> = team
            .pinned_components()
            .unwrap()
            .iter()
            .map(|pin| pin.name.clone())
            .collect();

        for _ in 0..50 {
            let next: Vec<String> = team
                .pinned_components()
                .unwrap()
                .iter()
                .map(|pin| pin.name.clone())
                .collect();
            assert_eq!(next, first, "traversal order must not vary between calls");
        }

        // An independently constructed equal value yields the same order too.
        let rebuilt = fully_pinned_team_package();
        let rebuilt_names: Vec<String> = rebuilt
            .pinned_components()
            .unwrap()
            .iter()
            .map(|pin| pin.name.clone())
            .collect();
        assert_eq!(rebuilt_names, first);
    }

    #[test]
    fn team_package_round_trips_losslessly() {
        let pkg = sample_team_package();
        let json = serde_json::to_string(&pkg).unwrap();
        let back: TeamPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkg);
    }

    #[test]
    fn team_role_round_trips_with_snake_case_discriminator() {
        let json = serde_json::to_value(TeamRole::EntryPoint).unwrap();
        assert_eq!(json, serde_json::json!("entry_point"));
        let back: TeamRole = serde_json::from_value(json).unwrap();
        assert_eq!(back, TeamRole::EntryPoint);
    }

    /// structural proof: `HumanRole` constructs with ONLY
    /// role/description/responsibilities/channel_hints — there is no
    /// user_id/channel_id/email/channel_address field to populate, and the
    /// serialized form carries none of those keys either.
    #[test]
    fn human_role_has_no_identity_field() {
        let role = sample_human_role();
        let json = serde_json::to_value(&role).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("user_id"));
        assert!(!obj.contains_key("userId"));
        assert!(!obj.contains_key("channel_id"));
        assert!(!obj.contains_key("channelId"));
        assert!(!obj.contains_key("email"));
        assert!(!obj.contains_key("channel_address"));
        assert!(!obj.contains_key("channelAddress"));
    }

    #[test]
    fn human_role_maps_to_human_role_config_slot() {
        let role = sample_human_role();
        let slot = role.to_config_slot();
        assert_eq!(slot.slot.key(), ("human_role", "approver"));
    }
}
