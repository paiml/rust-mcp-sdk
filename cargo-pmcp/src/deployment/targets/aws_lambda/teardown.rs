//! aws-lambda `destroy`, through `CloudFormation` itself (debug session
//! `cargo-pmcp-deploy-targets`: "destroy must remove what deploy created").
//!
//! An aws-lambda deployment is the `CloudFormation` stack `{[server] name}-stack`,
//! created by the native engine or by `npx cdk deploy`. `destroy` used to run
//! `npx cdk destroy`, which reaches only a stack the CDK app in
//! deploy/bin/app.ts declares. A stack the native engine created after a
//! rename was therefore refused by the stack guard, every destroy needed
//! Node.js and a synthesizable CDK app, and it ran in the shell's region
//! instead of `[aws] region` (A7). It now deletes the stack with
//! `DeleteStack`, in `[aws] region`, and waits for the deletion to finish.
//!
//! `DeleteStack` of a stack that does not exist succeeds silently, as
//! `cdk destroy` of an undeclared stack did, so destroy describes first. When
//! the configured stack does not exist, it checks the stacks
//! `deploy/outputs.json` records (written by the last deploy): if one of them
//! still exists, the deployment lives under another name and destroy refuses
//! instead of reporting success, so `--clean` cannot delete the local files
//! of a running deployment.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;

use super::engine::{
    format_failure_events, load_aws_config, AwsStackDescriber, StackDescriber, StackLookup,
    POLL_INTERVAL,
};
use crate::deployment::cdk_stack_guard::expected_stack_name;
use crate::deployment::DeployConfig;

/// Stack deletion, on top of [`StackDescriber`].
#[async_trait]
pub(super) trait StackDeleter: StackDescriber {
    /// Start deleting `stack_name`.
    async fn delete(&self, stack_name: &str) -> Result<()>;
}

#[async_trait]
impl StackDeleter for AwsStackDescriber<'_> {
    async fn delete(&self, stack_name: &str) -> Result<()> {
        self.client
            .delete_stack()
            .stack_name(stack_name)
            .send()
            .await
            .context("CloudFormation DeleteStack failed")?;
        Ok(())
    }
}

/// What a destroy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DestroyOutcome {
    /// The stack existed and is now deleted.
    Deleted,
    /// No such stack existed, and no stack deploy/outputs.json records does
    /// either.
    NothingToDelete,
}

/// Delete `{[server] name}-stack` in `[aws] region`, against real AWS.
pub(super) async fn destroy_stack(config: &DeployConfig) -> Result<DestroyOutcome> {
    let aws_cfg = load_aws_config(&config.aws().region).await;
    let client = aws_sdk_cloudformation::Client::new(&aws_cfg);
    destroy_with(
        &AwsStackDescriber { client: &client },
        config,
        POLL_INTERVAL,
    )
    .await
}

/// The decision logic of [`destroy_stack`], against any [`StackDeleter`].
pub(super) async fn destroy_with(
    ops: &dyn StackDeleter,
    config: &DeployConfig,
    interval: Duration,
) -> Result<DestroyOutcome> {
    let expected = expected_stack_name(&config.server.name);
    let region = &config.aws().region;
    if ops.describe(&expected).await? == StackLookup::NotFound {
        ensure_no_recorded_stack_survives(ops, config, &expected).await?;
        return Ok(DestroyOutcome::NothingToDelete);
    }
    println!("   Deleting CloudFormation stack {expected} ({region})...");
    ops.delete(&expected).await?;
    wait_until_deleted(ops, &expected, interval).await?;
    Ok(DestroyOutcome::Deleted)
}

/// Poll until `stack` is gone; a `DELETE_FAILED` stack fails with its recent
/// failure events.
async fn wait_until_deleted(
    ops: &dyn StackDescriber,
    stack: &str,
    interval: Duration,
) -> Result<()> {
    loop {
        let StackLookup::Found { status, .. } = ops.describe(stack).await? else {
            return Ok(());
        };
        match status.as_str() {
            "DELETE_COMPLETE" => return Ok(()),
            "DELETE_FAILED" => {
                let events = ops.recent_failure_events(stack).await?;
                bail!("{}", format_failure_events(stack, &status, &events));
            },
            _ => tokio::time::sleep(interval).await,
        }
    }
}

/// Refuse when the configured stack does not exist but a stack the last
/// deploy recorded in deploy/outputs.json still does.
async fn ensure_no_recorded_stack_survives(
    ops: &dyn StackDeleter,
    config: &DeployConfig,
    expected: &str,
) -> Result<()> {
    for recorded in recorded_stack_names(&config.project_root) {
        if recorded == expected {
            continue;
        }
        if let StackLookup::Found { .. } = ops.describe(&recorded).await? {
            bail!(
                "{}",
                recorded_stack_survives_message(
                    &config.server.name,
                    &recorded,
                    &config.aws().region
                )
            );
        }
    }
    Ok(())
}

/// The stack names deploy/outputs.json records (its top-level keys); empty
/// when there is no readable file.
fn recorded_stack_names(project_root: &Path) -> Vec<String> {
    std::fs::read_to_string(project_root.join("deploy").join("outputs.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| {
            json.as_object()
                .map(|stacks| stacks.keys().cloned().collect())
        })
        .unwrap_or_default()
}

/// The refusal [`ensure_no_recorded_stack_survives`] prints.
fn recorded_stack_survives_message(server_name: &str, recorded: &str, region: &str) -> String {
    let expected = expected_stack_name(server_name);
    let keep = recorded
        .strip_suffix("-stack")
        .filter(|name| !name.is_empty())
        .map_or_else(
            || {
                "set `[server] name` in .pmcp/deploy.toml to the name it was deployed with"
                    .to_string()
            },
            |name| format!("set `[server] name = \"{name}\"` in .pmcp/deploy.toml"),
        );
    format!(
        "refusing to report a destroy: no CloudFormation stack named `{expected}` (from \
         `[server] name = \"{server_name}\"` in .pmcp/deploy.toml) exists in {region}, but \
         deploy/outputs.json records `{recorded}`, which does.\n\
         Nothing was deleted. To remove `{recorded}`, choose one:\n  \
         - Destroy it with cargo-pmcp: {keep}, then re-run `cargo pmcp deploy destroy`.\n  \
         - Delete it directly: aws cloudformation delete-stack --stack-name {recorded} --region \
         {region}"
    )
}

#[cfg(test)]
mod tests {
    use super::super::engine::{StackEvent, StackLookup};
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::path::Path;
    use std::sync::Mutex;

    /// Scripted `CloudFormation`: a queue of `describe` answers per stack (the
    /// last answer repeats; an unscripted stack does not exist), and a log of
    /// `delete` calls.
    struct ScriptedStacks {
        lookups: Mutex<HashMap<String, VecDeque<StackLookup>>>,
        deleted: Mutex<Vec<String>>,
        failure_events: Vec<StackEvent>,
    }

    impl ScriptedStacks {
        fn new(script: &[(&str, Vec<StackLookup>)]) -> Self {
            Self {
                lookups: Mutex::new(
                    script
                        .iter()
                        .map(|(name, answers)| {
                            ((*name).to_string(), answers.iter().cloned().collect())
                        })
                        .collect(),
                ),
                deleted: Mutex::new(Vec::new()),
                failure_events: vec![StackEvent {
                    logical_id: "McpFunction".to_string(),
                    status: "DELETE_FAILED".to_string(),
                    reason: Some("Export acme-McpRoleArn is in use".to_string()),
                }],
            }
        }

        fn deleted(&self) -> Vec<String> {
            self.deleted.lock().expect("lock").clone()
        }
    }

    #[async_trait]
    impl StackDescriber for ScriptedStacks {
        async fn describe(&self, stack_name: &str) -> Result<StackLookup> {
            let mut lookups = self.lookups.lock().expect("lock");
            let Some(queue) = lookups.get_mut(stack_name) else {
                return Ok(StackLookup::NotFound);
            };
            let answer = if queue.len() > 1 {
                queue.pop_front()
            } else {
                queue.front().cloned()
            };
            Ok(answer.unwrap_or(StackLookup::NotFound))
        }

        async fn recent_failure_events(&self, _stack_name: &str) -> Result<Vec<StackEvent>> {
            Ok(self.failure_events.clone())
        }
    }

    #[async_trait]
    impl StackDeleter for ScriptedStacks {
        async fn delete(&self, stack_name: &str) -> Result<()> {
            self.deleted
                .lock()
                .expect("lock")
                .push(stack_name.to_string());
            Ok(())
        }
    }

    fn found(status: &str) -> StackLookup {
        StackLookup::Found {
            status: status.to_string(),
            outputs: vec![],
        }
    }

    fn config(root: &Path) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            "acme".to_string(),
            "eu-west-2".to_string(),
            root.to_path_buf(),
        );
        cfg.target.target_type = "aws-lambda".to_string();
        cfg
    }

    fn record_outputs_for(root: &Path, stack: &str) {
        std::fs::create_dir_all(root.join("deploy")).expect("mkdir deploy");
        std::fs::write(
            root.join("deploy/outputs.json"),
            format!("{{\"{stack}\": {{\"ApiUrl\": \"https://x.example.com\"}}}}"),
        )
        .expect("write outputs.json");
    }

    /// Run destroy against the scripted stacks. (The red run of these tests
    /// drove the CDK-based destroy this replaced, through a fake `npx` whose
    /// `cdk list` answered as app.ts would; the native destroy runs no `npx`.)
    async fn destroy(root: &Path, stacks: &ScriptedStacks) -> Result<DestroyOutcome> {
        destroy_with(stacks, &config(root), Duration::from_millis(1)).await
    }

    /// "destroy must remove what deploy created": after a rename, the native
    /// engine created `acme-stack` while app.ts still declares the old stack.
    /// Destroy deletes `acme-stack` directly through `CloudFormation`.
    #[tokio::test]
    async fn destroys_the_engine_created_stack_after_a_rename() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stacks = ScriptedStacks::new(&[(
            "acme-stack",
            vec![
                found("CREATE_COMPLETE"),
                found("DELETE_IN_PROGRESS"),
                StackLookup::NotFound,
            ],
        )]);

        let outcome = destroy(tmp.path(), &stacks)
            .await
            .expect("the engine-created stack is destroyed");

        assert_eq!(outcome, DestroyOutcome::Deleted);
        assert_eq!(stacks.deleted(), vec!["acme-stack".to_string()]);
    }

    /// The guard PR's protection, kept without Node.js: the configured stack
    /// does not exist, but deploy/outputs.json records another stack that
    /// does. Reporting success here would leave that stack running.
    #[tokio::test]
    async fn refuses_when_only_another_recorded_stack_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_outputs_for(tmp.path(), "old-server-stack");
        let stacks = ScriptedStacks::new(&[("old-server-stack", vec![found("UPDATE_COMPLETE")])]);

        let err = destroy(tmp.path(), &stacks).await.expect_err("must refuse");

        let msg = format!("{err:#}");
        assert!(msg.contains("acme-stack"), "{msg}");
        assert!(msg.contains("old-server-stack"), "{msg}");
        assert!(msg.contains("Nothing was deleted"), "{msg}");
        assert!(msg.contains("[server] name = \"old-server\""), "{msg}");
        assert!(
            msg.contains(
                "aws cloudformation delete-stack --stack-name old-server-stack --region eu-west-2"
            ),
            "{msg}"
        );
        assert_eq!(stacks.deleted(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn nothing_to_delete_when_no_stack_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stacks = ScriptedStacks::new(&[]);
        let outcome = destroy(tmp.path(), &stacks).await.expect("ok");
        assert_eq!(outcome, DestroyOutcome::NothingToDelete);
        assert_eq!(stacks.deleted(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn nothing_to_delete_when_the_recorded_stack_is_gone_too() {
        let tmp = tempfile::tempdir().expect("tempdir");
        record_outputs_for(tmp.path(), "old-server-stack");
        let stacks = ScriptedStacks::new(&[]);
        let outcome = destroy(tmp.path(), &stacks).await.expect("ok");
        assert_eq!(outcome, DestroyOutcome::NothingToDelete);
    }

    #[tokio::test]
    async fn a_failed_delete_reports_the_stack_events() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stacks = ScriptedStacks::new(&[(
            "acme-stack",
            vec![found("CREATE_COMPLETE"), found("DELETE_FAILED")],
        )]);

        let err = destroy(tmp.path(), &stacks)
            .await
            .expect_err("DELETE_FAILED must fail the destroy");

        let msg = format!("{err:#}");
        assert!(msg.contains("DELETE_FAILED"), "{msg}");
        assert!(msg.contains("Export acme-McpRoleArn is in use"), "{msg}");
    }
}
