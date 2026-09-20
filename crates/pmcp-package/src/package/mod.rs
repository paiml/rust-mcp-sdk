//! The four AI-Package schema types: [`ServerPackage`] (Task 1),
//! `AgentPackage`/`TeamPackage` (Task 2), and `WorkflowManifest` (Task 3).
//!
//! Declared progressively across this plan's tasks so the crate compiles
//! green at every task's commit boundary (mirrors the 168-01/168-03
//! stub-then-implement precedent) — `server` is registered here first;
//! `agent`/`team` are added by Task 2; `workflow` is added by Task 3.

pub mod agent;
pub mod server;
pub mod team;
pub mod workflow;

pub use agent::AgentPackage;
pub use server::{
    AssetsSection, AuthDcrSection, AuthScopesSection, AuthSection, AwsSection, BinaryRef,
    CedarPolicy, CedarPolicySet, CognitoSection, CompositionSection, DeployDescriptor, GcpSection,
    IamSection, IamStatement, LayoutSection, MetadataSection, ObservabilityAlarmsSection,
    ObservabilitySection, ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
pub use team::{HumanRole, TeamLimits, TeamMember, TeamPackage, TeamRole};
pub use workflow::{Provenance, WorkflowManifest, REGISTRY_NAMESPACE_PATTERN};
