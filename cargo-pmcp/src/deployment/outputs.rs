use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::DeploymentOutputs;

/// CDK Stack outputs format (from deploy/outputs.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CdkStackOutputs {
    #[serde(rename = "ApiUrl")]
    pub api_url: String,

    #[serde(rename = "OAuthDiscoveryUrl", skip_serializing_if = "Option::is_none")]
    pub oauth_discovery_url: Option<String>,

    #[serde(rename = "ClientId", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    #[serde(rename = "DashboardUrl", skip_serializing_if = "Option::is_none")]
    pub dashboard_url: Option<String>,

    #[serde(rename = "UserPoolId", skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
}

/// Load the outputs of `stack_name` from `deploy/outputs.json` (written by
/// `cdk deploy --outputs-file` and by the native `CloudFormation` engine, both
/// keyed by stack name) and convert them to the standard format.
///
/// The stack is looked up BY NAME (debug session `cargo-pmcp-deploy-targets`,
/// finding #2). This used to take whichever stack came first and label it
/// `stack_name`, so after a `[server] name` rename the old stack's outputs
/// were shown under the new stack's name, and a CDK deploy that also deployed
/// a dependency stack could show that stack's outputs instead.
pub fn load_cdk_outputs(
    project_root: &Path,
    region: &str,
    stack_name: &str,
) -> Result<DeploymentOutputs> {
    let outputs_path = project_root.join("deploy/outputs.json");

    if !outputs_path.exists() {
        anyhow::bail!("No deployment found. Run: cargo pmcp deploy");
    }

    let outputs_str =
        std::fs::read_to_string(&outputs_path).context("Failed to read deploy/outputs.json")?;

    let outputs_json: serde_json::Value =
        serde_json::from_str(&outputs_str).context("Failed to parse deploy/outputs.json")?;

    let stacks = outputs_json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("deploy/outputs.json is not a JSON object"))?;
    let stack_outputs = stacks.get(stack_name).ok_or_else(|| {
        let recorded = if stacks.is_empty() {
            "no stacks".to_string()
        } else {
            stacks.keys().cloned().collect::<Vec<_>>().join(", ")
        };
        anyhow::anyhow!(
            "deploy/outputs.json has no outputs for `{stack_name}`, the stack cargo-pmcp deploys \
             for the current `[server] name` (it records: {recorded}). Run `cargo pmcp deploy` \
             to deploy `{stack_name}`, or set `[server] name` in .pmcp/deploy.toml back to the \
             deployment you mean."
        )
    })?;

    let cdk_outputs: CdkStackOutputs =
        serde_json::from_value(stack_outputs.clone()).context("Failed to parse stack outputs")?;

    // Convert to standard DeploymentOutputs
    let mut custom = std::collections::HashMap::new();

    if let Some(oauth_url) = &cdk_outputs.oauth_discovery_url {
        custom.insert(
            "oauth_discovery_url".to_string(),
            serde_json::json!(oauth_url),
        );
    }
    if let Some(client_id) = &cdk_outputs.client_id {
        custom.insert("client_id".to_string(), serde_json::json!(client_id));
    }
    if let Some(dashboard) = &cdk_outputs.dashboard_url {
        custom.insert("dashboard_url".to_string(), serde_json::json!(dashboard));
    }
    if let Some(pool_id) = &cdk_outputs.user_pool_id {
        custom.insert("user_pool_id".to_string(), serde_json::json!(pool_id));
    }

    Ok(DeploymentOutputs {
        url: Some(cdk_outputs.api_url),
        regions: vec![region.to_string()],
        stack_name: Some(stack_name.to_string()),
        version: None,
        additional_urls: vec![],
        custom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_outputs(root: &Path, body: &serde_json::Value) {
        std::fs::create_dir_all(root.join("deploy")).expect("mkdir deploy");
        std::fs::write(
            root.join("deploy/outputs.json"),
            serde_json::to_string_pretty(body).expect("serialize"),
        )
        .expect("write outputs.json");
    }

    /// Finding #2: outputs are read for the stack cargo-pmcp names, not for
    /// whichever stack happens to come first in deploy/outputs.json (a CDK
    /// deploy that also deployed a dependency stack writes several).
    #[test]
    fn reads_the_named_stack_not_the_first_one() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_outputs(
            tmp.path(),
            &serde_json::json!({
                "aaa-shared-stack": { "ApiUrl": "https://shared.example.com" },
                "acme-stack": { "ApiUrl": "https://acme.example.com" },
            }),
        );

        let outputs = load_cdk_outputs(tmp.path(), "us-east-1", "acme-stack").expect("load");

        assert_eq!(outputs.url.as_deref(), Some("https://acme.example.com"));
        assert_eq!(outputs.stack_name.as_deref(), Some("acme-stack"));
    }

    /// The reported mislabel: after a rename, outputs.json still holds the
    /// OLD stack's outputs, which were shown under the NEW stack's name.
    #[test]
    fn refuses_to_label_another_stack_s_outputs_with_the_expected_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_outputs(
            tmp.path(),
            &serde_json::json!({
                "forecast-coach-lambda-stack": { "ApiUrl": "https://old.example.com" },
            }),
        );

        let err = load_cdk_outputs(tmp.path(), "us-east-1", "forecast-coach-acme-stack")
            .expect_err("outputs of another stack must not be relabelled");
        let msg = format!("{err:#}");
        assert!(msg.contains("forecast-coach-acme-stack"), "{msg}");
        assert!(msg.contains("forecast-coach-lambda-stack"), "{msg}");
    }

    #[test]
    fn reads_the_single_matching_stack() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_outputs(
            tmp.path(),
            &serde_json::json!({ "acme-stack": { "ApiUrl": "https://acme.example.com", "ClientId": "c1" } }),
        );
        let outputs = load_cdk_outputs(tmp.path(), "eu-west-2", "acme-stack").expect("load");
        assert_eq!(outputs.url.as_deref(), Some("https://acme.example.com"));
        assert_eq!(outputs.regions, vec!["eu-west-2".to_string()]);
        assert_eq!(
            outputs.custom.get("client_id"),
            Some(&serde_json::json!("c1"))
        );
    }

    #[test]
    fn missing_outputs_json_says_nothing_is_deployed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = load_cdk_outputs(tmp.path(), "us-east-1", "acme-stack").expect_err("missing");
        assert!(err.to_string().contains("No deployment found"), "{err}");
    }
}
