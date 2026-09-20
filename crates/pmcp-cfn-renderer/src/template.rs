//! [`CfnTemplate`] and its resource/output shapes, plus canonical-JSON
//! emission.
//!
//! Determinism is enforced structurally: every keyed collection here is a
//! `BTreeMap`, never a `HashMap`. `BTreeMap`'s own `Serialize` impl always
//! iterates (and therefore emits) its entries in sorted key order — this
//! holds regardless of whether `serde_json`'s `preserve_order` feature is
//! active anywhere else in the dependency graph, because that feature only
//! changes what backs `serde_json::Map`/`Value::Object`, not the order in
//! which a `BTreeMap` field feeds entries INTO that map during
//! serialization. `to_canonical_json` therefore never needs to sort
//! anything itself — it only needs to serialize through `BTreeMap` fields.

use serde::Serialize;
use std::collections::BTreeMap;

/// A single CloudFormation resource: `{"Type": ..., "Properties": ...,
/// "DependsOn": [...]}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CfnResource {
    /// The CFN resource type, e.g. `AWS::Lambda::Function`.
    #[serde(rename = "Type")]
    pub type_: String,
    /// Resource properties, already in CFN's PascalCase property-name shape.
    #[serde(rename = "Properties")]
    pub properties: serde_json::Value,
    /// Logical IDs this resource explicitly depends on. Omitted entirely
    /// when empty (an empty `DependsOn: []` is unusual in real CFN output).
    #[serde(rename = "DependsOn", skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

/// A single CloudFormation stack output: `{"Description": ..., "Value":
/// ...}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CfnOutput {
    /// Human-readable description of the output.
    #[serde(rename = "Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The output's value (an intrinsic-function `Value`, e.g. `{"Fn::GetAtt": [...]}`).
    #[serde(rename = "Value")]
    pub value: serde_json::Value,
    /// Cross-stack export, when this output is meant to be consumed by
    /// downstream stacks (e.g. `McpRoleArn`). Omitted entirely when absent.
    #[serde(rename = "Export", skip_serializing_if = "Option::is_none")]
    pub export: Option<CfnExport>,
}

/// A CloudFormation `Export` block: `{"Name": "..."}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CfnExport {
    /// The exported name, importable from other stacks via `Fn::ImportValue`.
    #[serde(rename = "Name")]
    pub name: String,
}

/// A rendered CloudFormation template.
///
/// `resources`/`outputs`/`metadata` are `BTreeMap`s so their key order is
/// always sorted — see the module doc comment for why this makes
/// [`CfnTemplate::to_canonical_json`] byte-deterministic without any manual
/// sorting step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CfnTemplate {
    /// The template's top-level `Description`.
    #[serde(rename = "Description")]
    pub description: String,
    /// Logical-ID -> resource. Always present (CFN requires a `Resources`
    /// section), even when empty (this task's stub output).
    #[serde(rename = "Resources")]
    pub resources: BTreeMap<String, CfnResource>,
    /// Logical-ID -> output. Omitted entirely when empty.
    #[serde(rename = "Outputs", skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, CfnOutput>,
    /// Free-form template `Metadata` (e.g. the `mcp:*` synth-context keys).
    /// Omitted entirely when empty.
    #[serde(rename = "Metadata", skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl CfnTemplate {
    /// Render this template as canonical, byte-deterministic JSON.
    ///
    /// "Canonical" here means: sorted keys (via the `BTreeMap` fields —
    /// see the module doc comment) and stable formatting
    /// (`serde_json::to_string_pretty`). Two calls of [`crate::render`] with
    /// identical inputs always produce an identical string.
    #[must_use]
    pub fn to_canonical_json(&self) -> String {
        let value = serde_json::to_value(self)
            .expect("CfnTemplate fields (String/BTreeMap/serde_json::Value) always serialize");
        serde_json::to_string_pretty(&value)
            .expect("a serde_json::Value produced by to_value always re-serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_template() -> CfnTemplate {
        let mut resources = BTreeMap::new();
        resources.insert(
            "McpFunction".to_string(),
            CfnResource {
                type_: "AWS::Lambda::Function".to_string(),
                properties: serde_json::json!({"MemorySize": 512}),
                depends_on: vec!["LogGroup".to_string()],
            },
        );
        let mut outputs = BTreeMap::new();
        outputs.insert(
            "FunctionArn".to_string(),
            CfnOutput {
                description: Some("The function's ARN".to_string()),
                value: serde_json::json!({"Fn::GetAtt": ["McpFunction", "Arn"]}),
                export: None,
            },
        );
        CfnTemplate {
            description: "PMCP MCP server: det-test".to_string(),
            resources,
            outputs,
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn resource_type_field_renders_as_type_key() {
        let template = sample_template();
        let json = template.to_canonical_json();
        assert!(json.contains("\"Type\": \"AWS::Lambda::Function\""));
        assert!(!json.contains("type_"));
    }

    #[test]
    fn empty_outputs_and_metadata_are_omitted() {
        let template = CfnTemplate {
            description: "empty".to_string(),
            resources: BTreeMap::new(),
            outputs: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let json = template.to_canonical_json();
        assert!(!json.contains("\"Outputs\""));
        assert!(!json.contains("\"Metadata\""));
        // Resources always present, even when empty.
        assert!(json.contains("\"Resources\": {}"));
    }

    #[test]
    fn depends_on_is_omitted_when_empty() {
        let mut resources = BTreeMap::new();
        resources.insert(
            "LogGroup".to_string(),
            CfnResource {
                type_: "AWS::Logs::LogGroup".to_string(),
                properties: serde_json::json!({}),
                depends_on: vec![],
            },
        );
        let template = CfnTemplate {
            description: "d".to_string(),
            resources,
            outputs: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        assert!(!template.to_canonical_json().contains("DependsOn"));
    }

    #[test]
    fn canonical_json_round_trips_byte_identically() {
        let template = sample_template();
        let json = template.to_canonical_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string_pretty(&value).unwrap(), json);
    }
}
