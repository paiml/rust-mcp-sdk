//! `AgentPackage` — the captured `AgentConfig` document.
//!
//! Field mapping is drawn from `amplify/data/resource.ts:1150-1227`'s
//! `AgentConfig` model: `instructions` (system prompt), `modelId` (captured
//! as the `llm` config slot's tested value,), `maxTokens`/
//! `maxIterations`, `mcpServerIds` (connector refs, ranged — e.g.
//! `london-tube@^1.2`), `toolSelection`/`inputSchema`/`outputSchema` (kept as
//! open `serde_json::Value` — these are already-typed JSON documents in the
//! source model, not something this crate should re-model), `importance`,
//! `finalizerRole`.
//!
//! `budget_defaults` are DEFAULTS captured at test time — overriding a
//! budget-override slot's tested value at import time is a real behavioral
//! change and surfaces as an deviation (`slot::detect_deviation`), not a
//! silent override.
//!
//! # No bare floats — always `canonicalize()`-able
//!
//! `AgentPackage` deliberately carries NO `f32`/`f64` field: all numeric
//! config is either an integer (`max_tokens`/`max_iterations`) or, for any
//! future fractional value (e.g. a budget limit), string-encoded. This is a
//! crate-wide policy — `olpc-cjson`'s `CanonicalFormatter` unconditionally
//! rejects floating-point numbers ("floating point numbers are not allowed in
//! canonical JSON"), so a bare float anywhere in a package schema would make
//! that value un-`canonicalize()`-able and break its digest. Removing the
//! former `temperature: f64` field lets `AgentPackage` route through
//! `canonicalize()` uniformly with the other three package types (see
//! `oci::pack::pack_agent`).
//!
//! The open `serde_json::Value` fields (`tool_selection`/`input_schema`/
//! `output_schema`) are the one remaining place a float could sneak in from
//! an upstream document. If one does, `canonicalize()` returns an error at
//! pack time — a deliberate, loud failure, not a silent divergence.

use crate::reference::ComponentRef;
use crate::slot::ConfigSlot;
use serde::{Deserialize, Serialize};

/// The captured `agent` AI-Package payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentPackage {
    pub name: String,
    pub version: semver::Version,
    /// The system prompt (`AgentConfig.instructions`).
    pub instructions: String,
    /// The llm-provider config slot — `slot` is `SlotType::LlmProvider {
    /// name, tested_value }`, where `tested_value` is the `modelId` that was
    /// exercised when this package was tested.
    pub llm: ConfigSlot,
    pub max_tokens: i64,
    pub max_iterations: i64,
    /// Connector dependencies as capture-time ranges (from
    /// `AgentConfig.mcpServerIds`, e.g. `london-tube@^1.2`) — NOT pins; a
    /// `WorkflowManifest` (Task 3) is what pins these to exact digests.
    pub connectors: Vec<ComponentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_selection: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finalizer_role: Option<String>,
    /// Budget-override DEFAULTS (`SlotType::BudgetOverride` entries) captured
    /// at test time. Overriding one of these at import is a real behavioral
    /// change — see module docs.
    #[serde(default)]
    pub budget_defaults: Vec<ConfigSlot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::ComponentType;
    use crate::slot::SlotType;

    fn sample_agent_package() -> AgentPackage {
        AgentPackage {
            name: "triage-agent".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You triage incoming support tickets.".to_string(),
            llm: ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            }),
            max_tokens: 4096,
            max_iterations: 25,
            connectors: vec![ComponentRef::Range {
                name: "london-tube".to_string(),
                range: semver::VersionReq::parse("^1.2").unwrap(),
                component_type: ComponentType::Server,
            }],
            tool_selection: Some(serde_json::json!({ "london-tube": ["get_status"] })),
            input_schema: None,
            output_schema: Some(serde_json::json!({ "type": "object" })),
            importance: Some("HIGH".to_string()),
            finalizer_role: Some("formatter".to_string()),
            budget_defaults: vec![ConfigSlot::new(SlotType::BudgetOverride {
                name: "monthly-cap".to_string(),
                tested_value: "1000".to_string(),
            })],
        }
    }

    #[test]
    fn agent_package_round_trips_with_ranged_connector_ref() {
        let pkg = sample_agent_package();
        let json = serde_json::to_string(&pkg).unwrap();
        let back: AgentPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkg);
        assert!(!back.connectors[0].is_pinned());
    }

    #[test]
    fn agent_package_llm_slot_carries_tested_value() {
        let pkg = sample_agent_package();
        assert_eq!(pkg.llm.slot.tested_value(), Some("anthropic"));
    }

    #[test]
    fn agent_package_connectors_field_is_component_ref_vec() {
        let pkg = sample_agent_package();
        let names: Vec<&str> = pkg.connectors.iter().map(|c| c.name()).collect();
        assert_eq!(names, vec!["london-tube"]);
    }
}
