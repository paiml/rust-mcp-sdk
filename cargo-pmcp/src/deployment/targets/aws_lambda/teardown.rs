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
//!
//! # After the stack is gone (0.28.1)
//!
//! Two things outlived a destroyed stack (forecast-coach spike 021):
//!
//! - **The deploy artifacts (#406).** The native engine uploads each zip to
//!   `pmcp-deploy-{account}-{region}/{server}/bootstrap-<digest>.zip`, a
//!   bucket shared by every server in the account and region. Destroy now
//!   deletes exactly those keys ([`is_owned_artifact_key`]: a direct child of
//!   the `{server}/` prefix, `bootstrap-<lowercase hex>.zip`), never another
//!   prefix and never the bucket.
//! - **A function's log group (#407).** Both engines' stacks own
//!   `/aws/lambda/<function>` and delete it with the stack, but
//!   `CloudFormation` deletes it BEFORE the function (its name refers to the
//!   function), and Lambda re-creates it from the function's last log
//!   delivery in between. Destroy reads what the stack holds just before
//!   deleting it ([`StackInventory`]), and afterwards deletes the
//!   `/aws/lambda/<function>` group of every function the stack owned, except
//!   one the template retains (`DeletionPolicy: Retain`).
//!
//! Both run only after the deletion succeeded, and neither can fail the
//! destroy: what cannot be done is a warning carrying the exact command that
//! does it.

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
    /// What `stack_name` holds (see [`StackInventory`]).
    async fn inventory(&self, stack_name: &str) -> Result<StackInventory>;
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

    async fn inventory(&self, stack_name: &str) -> Result<StackInventory> {
        let template = self
            .client
            .get_template()
            .stack_name(stack_name)
            .send()
            .await
            .context("CloudFormation GetTemplate failed")?;
        let resources = self
            .client
            .describe_stack_resources()
            .stack_name(stack_name)
            .send()
            .await
            .context("CloudFormation DescribeStackResources failed")?;
        let resources: Vec<StackResourceInfo> = resources
            .stack_resources()
            .iter()
            .map(|r| StackResourceInfo {
                logical_id: r.logical_resource_id().unwrap_or_default().to_string(),
                resource_type: r.resource_type().unwrap_or_default().to_string(),
                physical_id: r.physical_resource_id().map(str::to_string),
            })
            .collect();
        inventory_from(template.template_body().unwrap_or_default(), &resources)
    }
}

/// What a destroy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DestroyOutcome {
    /// The stack existed and is now deleted. `inventory` is what the stack
    /// held, read just before the deletion (`None` when it could not be
    /// read); the log-group cleanup works from it (#407).
    Deleted { inventory: Option<StackInventory> },
    /// No such stack existed, and no stack deploy/outputs.json records does
    /// either.
    NothingToDelete,
}

/// The parts of a stack the after-destroy cleanup needs (#407): the Lambda
/// functions it owned, and the log groups its template RETAINS on deletion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StackInventory {
    /// Physical names of the stack's `AWS::Lambda::Function` resources.
    pub(super) function_names: Vec<String>,
    /// Names of the stack's `AWS::Logs::LogGroup` resources whose
    /// `DeletionPolicy` keeps them (`Retain`, `RetainExceptOnCreate`).
    pub(super) retained_log_groups: Vec<String>,
}

/// One resource of a stack, as `DescribeStackResources` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StackResourceInfo {
    pub(super) logical_id: String,
    pub(super) resource_type: String,
    pub(super) physical_id: Option<String>,
}

/// Build the [`StackInventory`] from the stack's template (`GetTemplate`) and
/// its resources (`DescribeStackResources`). A template that is not JSON is
/// refused: which log groups it retains would be a guess. (Both the native
/// engine and `cdk synth` emit JSON.)
pub(super) fn inventory_from(
    template_body: &str,
    resources: &[StackResourceInfo],
) -> Result<StackInventory> {
    let template: serde_json::Value = serde_json::from_str(template_body)
        .context("the stack's template is not JSON, so its log-group retention is unknown")?;
    let retained_ids: Vec<&str> = template["Resources"]
        .as_object()
        .map(|declared| {
            declared
                .iter()
                .filter(|(_, r)| {
                    r["Type"] == "AWS::Logs::LogGroup"
                        && matches!(
                            r["DeletionPolicy"].as_str(),
                            Some("Retain" | "RetainExceptOnCreate")
                        )
                })
                .map(|(id, _)| id.as_str())
                .collect()
        })
        .unwrap_or_default();
    let physical_of = |kind: &str, keep: &dyn Fn(&StackResourceInfo) -> bool| -> Vec<String> {
        resources
            .iter()
            .filter(|r| r.resource_type == kind && keep(r))
            .filter_map(|r| r.physical_id.clone())
            .collect()
    };
    Ok(StackInventory {
        function_names: physical_of("AWS::Lambda::Function", &|_| true),
        retained_log_groups: physical_of("AWS::Logs::LogGroup", &|r| {
            retained_ids.contains(&r.logical_id.as_str())
        }),
    })
}

// ===========================================================================
// After-destroy cleanup (#406 artifacts, #407 log groups)
// ===========================================================================

/// The deploy-artifact bucket (S3), as the cleanup uses it.
#[async_trait]
pub(super) trait ArtifactStore: Send + Sync {
    /// Every key under `prefix` in `bucket`, all pages; empty when the bucket
    /// does not exist.
    async fn list_keys(&self, bucket: &str, prefix: &str) -> Result<Vec<String>>;
    /// Delete one object.
    async fn delete_key(&self, bucket: &str, key: &str) -> Result<()>;
}

/// `CloudWatch` Logs, as the cleanup uses it.
#[async_trait]
pub(super) trait LogGroupStore: Send + Sync {
    /// Delete the log group `name`: `Ok(true)` when it existed and is now
    /// deleted, `Ok(false)` when it did not exist.
    async fn delete_log_group(&self, name: &str) -> Result<bool>;
}

/// What the after-destroy cleanup did, for the caller to print.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CleanupReport {
    /// Artifact keys deleted from the deploy bucket.
    pub(super) removed_artifacts: Vec<String>,
    /// Log groups deleted.
    pub(super) removed_log_groups: Vec<String>,
    /// Log groups left because the stack's template retains them.
    pub(super) retained_log_groups: Vec<String>,
    /// What could not be done, each with the command that does it.
    pub(super) warnings: Vec<String>,
}

/// Whether `key` is an artifact cargo-pmcp's deploy uploaded for
/// `server_name`: exactly `{server}/bootstrap-<lowercase hex>.zip`, a direct
/// child of the `{server}/` prefix (the shape `engine::artifact_s3_key`
/// writes). The trailing `/` is what keeps server `a` from matching `ab/` or
/// `a-b/`; anything else under the prefix was not written by cargo-pmcp and is
/// never touched.
pub(super) fn is_owned_artifact_key(server_name: &str, key: &str) -> bool {
    key.strip_prefix(&artifact_prefix(server_name))
        .and_then(|rest| rest.strip_prefix("bootstrap-"))
        .and_then(|rest| rest.strip_suffix(".zip"))
        .is_some_and(|hex| {
            !hex.is_empty() && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// The command that removes `server_name`'s deploy artifacts by hand. With
/// no bucket (the account is unknown) the command looks the account up.
fn artifact_cleanup_command(bucket: Option<&str>, server_name: &str, region: &str) -> String {
    let bucket = bucket.map_or_else(
        || {
            format!(
                "pmcp-deploy-$(aws sts get-caller-identity --query Account --output text)-{region}"
            )
        },
        str::to_string,
    );
    format!(
        "aws s3 rm s3://{bucket}/{} --recursive --exclude \"*\" --include \"bootstrap-*.zip\" \
         --region {region}",
        artifact_prefix(server_name)
    )
}

/// Delete `server_name`'s deploy artifacts from `bucket` (#406; `None` when
/// the account, and so the bucket name, is unknown). Only keys
/// [`is_owned_artifact_key`] accepts are deleted; the bucket is shared by
/// every server in the account and region and is never deleted.
pub(super) async fn remove_artifacts(
    store: &dyn ArtifactStore,
    bucket: Option<&str>,
    server_name: &str,
    region: &str,
    report: &mut CleanupReport,
) {
    let command = artifact_cleanup_command(bucket, server_name, region);
    let Some(bucket) = bucket else {
        report.warnings.push(format!(
            "the deploy artifacts for `{server_name}` were not removed (the deploy bucket is \
             unknown). Remove them with:\n    {command}"
        ));
        return;
    };
    let keys = match store.list_keys(bucket, &artifact_prefix(server_name)).await {
        Ok(keys) => keys,
        Err(err) => {
            report.warnings.push(format!(
                "could not list the deploy artifacts under s3://{bucket}/{} ({err:#}). Remove \
                 them with:\n    {command}",
                artifact_prefix(server_name)
            ));
            return;
        },
    };
    for key in keys
        .iter()
        .filter(|key| is_owned_artifact_key(server_name, key))
    {
        match store.delete_key(bucket, key).await {
            Ok(()) => report.removed_artifacts.push(key.clone()),
            Err(err) => report.warnings.push(format!(
                "could not delete s3://{bucket}/{key} ({err:#}). Remove it with:\n    {command}"
            )),
        }
    }
}

/// The command that deletes log group `name` by hand.
fn log_group_cleanup_command(name: &str, region: &str) -> String {
    format!("aws logs delete-log-group --log-group-name {name} --region {region}")
}

/// Delete the `/aws/lambda/<function>` log groups of the functions the
/// deleted stack owned (#407), except those its template retains. Without an
/// inventory nothing is deleted (which groups are the stack's, and whether
/// one is retained, is unknown); the command for the `[server] name`
/// function's group is printed instead.
pub(super) async fn remove_log_groups(
    store: &dyn LogGroupStore,
    inventory: Option<&StackInventory>,
    server_name: &str,
    region: &str,
    report: &mut CleanupReport,
) {
    let Some(inventory) = inventory else {
        let group = format!("/aws/lambda/{server_name}");
        report.warnings.push(format!(
            "could not read what the stack held, so its functions' log groups were not checked. \
             If {group} still exists, delete it with:\n    {}",
            log_group_cleanup_command(&group, region)
        ));
        return;
    };
    let mut groups: Vec<String> = inventory
        .function_names
        .iter()
        .map(|function| format!("/aws/lambda/{function}"))
        .collect();
    groups.sort();
    groups.dedup();
    for group in groups {
        if inventory.retained_log_groups.contains(&group) {
            report.retained_log_groups.push(group);
            continue;
        }
        match store.delete_log_group(&group).await {
            Ok(true) => report.removed_log_groups.push(group),
            Ok(false) => {},
            Err(err) => report.warnings.push(format!(
                "could not delete log group {group} ({err:#}). Delete it with:\n    {}",
                log_group_cleanup_command(&group, region)
            )),
        }
    }
}

/// [`ArtifactStore`] against S3.
struct AwsArtifactStore<'a> {
    client: &'a aws_sdk_s3::Client,
}

#[async_trait]
impl ArtifactStore for AwsArtifactStore<'_> {
    async fn list_keys(&self, bucket: &str, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut pages = self
            .client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .into_paginator()
            .send();
        while let Some(page) = pages.next().await {
            match page {
                Ok(page) => keys.extend(
                    page.contents()
                        .iter()
                        .filter_map(|object| object.key().map(str::to_string)),
                ),
                Err(err)
                    if err
                        .as_service_error()
                        .is_some_and(aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Error::is_no_such_bucket) =>
                {
                    return Ok(Vec::new());
                },
                Err(err) => return Err(err).context("S3 ListObjectsV2 failed"),
            }
        }
        Ok(keys)
    }

    async fn delete_key(&self, bucket: &str, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .context("S3 DeleteObject failed")?;
        Ok(())
    }
}

/// [`LogGroupStore`] against `CloudWatch` Logs.
struct AwsLogGroupStore<'a> {
    client: &'a aws_sdk_cloudwatchlogs::Client,
}

#[async_trait]
impl LogGroupStore for AwsLogGroupStore<'_> {
    async fn delete_log_group(&self, name: &str) -> Result<bool> {
        match self
            .client
            .delete_log_group()
            .log_group_name(name)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err)
                if err.as_service_error().is_some_and(
                    aws_sdk_cloudwatchlogs::operation::delete_log_group::DeleteLogGroupError::is_resource_not_found_exception,
                ) =>
            {
                Ok(false)
            },
            Err(err) => Err(err).context("CloudWatch Logs DeleteLogGroup failed"),
        }
    }
}

/// After the stack is deleted, against real AWS: remove the deploy artifacts
/// the native engine uploaded for this server (#406) and the
/// `/aws/lambda/<function>` log groups of the functions the stack owned
/// (#407), then print what was done. Never fails the destroy: what cannot be
/// done is a warning carrying the exact command that does it.
pub(super) async fn clean_up_after_destroy(
    config: &DeployConfig,
    inventory: Option<&StackInventory>,
) {
    let region = config.aws().region.clone();
    let server = config.server.name.clone();
    let aws_cfg = load_aws_config(&region).await;
    let mut report = CleanupReport::default();

    let bucket = match super::engine::resolve_account_id(&region).await {
        Ok(account) => Some(super::engine::bucket_name(&account, &region)),
        Err(err) => {
            report.warnings.push(format!(
                "could not resolve the AWS account to find the deploy bucket ({err:#})"
            ));
            None
        },
    };
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);
    remove_artifacts(
        &AwsArtifactStore { client: &s3 },
        bucket.as_deref(),
        &server,
        &region,
        &mut report,
    )
    .await;

    let logs = aws_sdk_cloudwatchlogs::Client::new(&aws_cfg);
    remove_log_groups(
        &AwsLogGroupStore { client: &logs },
        inventory,
        &server,
        &region,
        &mut report,
    )
    .await;

    print_cleanup_report(&report, bucket.as_deref(), &server);
}

/// Print `report`: what was removed (stdout), and every warning (stderr).
fn print_cleanup_report(report: &CleanupReport, bucket: Option<&str>, server: &str) {
    if !report.removed_artifacts.is_empty() {
        println!(
            "   Removed {} deploy artifact(s) under s3://{}/{}",
            report.removed_artifacts.len(),
            bucket.unwrap_or("?"),
            artifact_prefix(server)
        );
    }
    for group in &report.removed_log_groups {
        println!("   Removed log group {group}");
    }
    for group in &report.retained_log_groups {
        println!("   Kept log group {group} (the stack's DeletionPolicy retains it)");
    }
    for warning in &report.warnings {
        eprintln!("  {} {warning}", console::style("warning:").yellow());
    }
}

/// The S3 prefix deploy writes a server's artifacts under: `{server}/`.
fn artifact_prefix(server_name: &str) -> String {
    format!("{server_name}/")
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
    // What the stack holds, read while it still exists (#407). Never a
    // reason to stop the destroy.
    let inventory = match ops.inventory(&expected).await {
        Ok(inventory) => Some(inventory),
        Err(err) => {
            eprintln!(
                "  {} could not read the resources of {expected} ({err:#}); deleting it anyway.",
                console::style("warning:").yellow()
            );
            None
        },
    };
    println!("   Deleting CloudFormation stack {expected} ({region})...");
    ops.delete(&expected).await?;
    wait_until_deleted(ops, &expected, interval).await?;
    Ok(DestroyOutcome::Deleted { inventory })
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
        /// What `inventory` answers (`None` = an error).
        inventory: Option<StackInventory>,
        /// Every `delete`/`inventory` call, in order.
        calls: Mutex<Vec<String>>,
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
                inventory: Some(StackInventory::default()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn with_inventory(mut self, inventory: Option<StackInventory>) -> Self {
            self.inventory = inventory;
            self
        }

        fn deleted(&self) -> Vec<String> {
            self.deleted.lock().expect("lock").clone()
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("lock").clone()
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
            self.calls
                .lock()
                .expect("lock")
                .push(format!("delete {stack_name}"));
            self.deleted
                .lock()
                .expect("lock")
                .push(stack_name.to_string());
            Ok(())
        }

        async fn inventory(&self, stack_name: &str) -> Result<StackInventory> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("inventory {stack_name}"));
            self.inventory
                .clone()
                .ok_or_else(|| anyhow::anyhow!("AccessDenied: cloudformation:GetTemplate"))
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

        assert!(
            matches!(outcome, DestroyOutcome::Deleted { .. }),
            "{outcome:?}"
        );
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

    // ---------- #407: what the stack held, read before it is deleted ----------

    fn deleting_acme() -> ScriptedStacks {
        ScriptedStacks::new(&[(
            "acme-stack",
            vec![found("CREATE_COMPLETE"), StackLookup::NotFound],
        )])
    }

    fn acme_inventory() -> StackInventory {
        StackInventory {
            function_names: vec!["acme".to_string()],
            retained_log_groups: vec![],
        }
    }

    /// The inventory is read BEFORE `DeleteStack` (afterwards the stack's
    /// resources are gone) and carried in the outcome.
    #[tokio::test]
    async fn destroy_reads_the_inventory_before_deleting() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stacks = deleting_acme().with_inventory(Some(acme_inventory()));

        let outcome = destroy(tmp.path(), &stacks).await.expect("destroyed");

        assert_eq!(
            outcome,
            DestroyOutcome::Deleted {
                inventory: Some(acme_inventory())
            }
        );
        assert_eq!(
            stacks.calls(),
            vec![
                "inventory acme-stack".to_string(),
                "delete acme-stack".to_string()
            ]
        );
    }

    /// An inventory that cannot be read (no `GetTemplate` permission, say)
    /// never blocks the destroy itself.
    #[tokio::test]
    async fn an_unreadable_inventory_does_not_block_the_destroy() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let stacks = deleting_acme().with_inventory(None);

        let outcome = destroy(tmp.path(), &stacks).await.expect("destroyed");

        assert_eq!(outcome, DestroyOutcome::Deleted { inventory: None });
        assert_eq!(stacks.deleted(), vec!["acme-stack".to_string()]);
    }

    /// The scaffold's `stack.ts`, synthesized: a log group CDK deletes with
    /// the stack (`removalPolicy: DESTROY` -> `DeletionPolicy: Delete`), one a
    /// hand edit retains, and two functions.
    const CDK_TEMPLATE: &str = r#"{
      "Resources": {
        "McpFunction1A2B": { "Type": "AWS::Lambda::Function" },
        "LogGroupF5B4": { "Type": "AWS::Logs::LogGroup", "DeletionPolicy": "Delete",
                          "UpdateReplacePolicy": "Delete" },
        "OAuthProxyFunctionC3": { "Type": "AWS::Lambda::Function" },
        "OAuthProxyLogGroup9": { "Type": "AWS::Logs::LogGroup", "DeletionPolicy": "Retain" },
        "HttpApi": { "Type": "AWS::ApiGatewayV2::Api" }
      }
    }"#;

    fn resource(logical: &str, kind: &str, physical: &str) -> StackResourceInfo {
        StackResourceInfo {
            logical_id: logical.to_string(),
            resource_type: kind.to_string(),
            physical_id: Some(physical.to_string()),
        }
    }

    #[test]
    fn inventory_reads_the_functions_and_the_retained_log_groups() {
        let resources = vec![
            resource("McpFunction1A2B", "AWS::Lambda::Function", "acme"),
            resource("LogGroupF5B4", "AWS::Logs::LogGroup", "/aws/lambda/acme"),
            resource(
                "OAuthProxyFunctionC3",
                "AWS::Lambda::Function",
                "acme-oauth-proxy",
            ),
            resource(
                "OAuthProxyLogGroup9",
                "AWS::Logs::LogGroup",
                "/aws/lambda/acme-oauth-proxy",
            ),
            resource("HttpApi", "AWS::ApiGatewayV2::Api", "abc123"),
        ];
        assert_eq!(
            inventory_from(CDK_TEMPLATE, &resources).expect("inventory"),
            StackInventory {
                function_names: vec!["acme".to_string(), "acme-oauth-proxy".to_string()],
                retained_log_groups: vec!["/aws/lambda/acme-oauth-proxy".to_string()],
            }
        );
    }

    /// A template that is not JSON leaves the retention unknown: refuse to
    /// build an inventory rather than guess.
    #[test]
    fn inventory_refuses_a_template_it_cannot_read() {
        assert!(inventory_from("Resources:\n  X: {}\n", &[]).is_err());
    }

    // ---------- #406: deploy artifacts ----------

    const BUCKET: &str = "pmcp-deploy-123456789012-eu-west-2";

    /// An in-memory deploy bucket: keys, the prefixes listed, the keys
    /// deleted, and what to fail.
    #[derive(Default)]
    struct FakeBucket {
        keys: Mutex<std::collections::BTreeSet<String>>,
        listed: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        fail_list: bool,
        fail_delete: Option<String>,
    }

    impl FakeBucket {
        fn with(keys: &[&str]) -> Self {
            Self {
                keys: Mutex::new(keys.iter().map(|k| (*k).to_string()).collect()),
                ..Self::default()
            }
        }

        fn remaining(&self) -> Vec<String> {
            self.keys.lock().expect("lock").iter().cloned().collect()
        }
    }

    #[async_trait]
    impl ArtifactStore for FakeBucket {
        async fn list_keys(&self, bucket: &str, prefix: &str) -> Result<Vec<String>> {
            assert_eq!(bucket, BUCKET);
            self.listed.lock().expect("lock").push(prefix.to_string());
            if self.fail_list {
                anyhow::bail!("AccessDenied: s3:ListBucket");
            }
            Ok(self
                .keys
                .lock()
                .expect("lock")
                .iter()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }

        async fn delete_key(&self, bucket: &str, key: &str) -> Result<()> {
            assert_eq!(bucket, BUCKET);
            if self.fail_delete.as_deref() == Some(key) {
                anyhow::bail!("AccessDenied: s3:DeleteObject");
            }
            self.deleted.lock().expect("lock").push(key.to_string());
            self.keys.lock().expect("lock").remove(key);
            Ok(())
        }
    }

    /// A key the deploy engine writes for `server` (`engine::artifact_s3_key`).
    fn uploaded(server: &str, bytes: &[u8]) -> String {
        super::super::engine::artifact_s3_key(server, bytes).1
    }

    async fn remove(bucket: &FakeBucket, server: &str) -> CleanupReport {
        let mut report = CleanupReport::default();
        remove_artifacts(bucket, Some(BUCKET), server, "eu-west-2", &mut report).await;
        report
    }

    /// #406, the reported case (forecast-coach spike 021 step 5): after
    /// destroy, the zips deploy uploaded stayed in the shared bucket. Destroy
    /// removes exactly `{server}/bootstrap-<digest>.zip` — never another
    /// server's, never a key it did not write, never the bucket.
    #[tokio::test]
    async fn destroy_removes_only_this_servers_deploy_artifacts() {
        let a1 = uploaded("a", b"one");
        let a2 = uploaded("a", b"two");
        let others = [
            uploaded("ab", b"x"),
            uploaded("a-b", b"x"),
            uploaded("a.b", b"x"),
            uploaded("b", b"x"),
            "a/notes.txt".to_string(),
            "a/sub/bootstrap-0123456789ab.zip".to_string(),
            "a/bootstrap-.zip".to_string(),
            "a/bootstrap-NOTHEX.zip".to_string(),
            "a".to_string(),
        ];
        let mut all: Vec<&str> = vec![a1.as_str(), a2.as_str()];
        all.extend(others.iter().map(String::as_str));
        let bucket = FakeBucket::with(&all);

        let report = remove(&bucket, "a").await;

        let mut removed = report.removed_artifacts.clone();
        removed.sort();
        let mut want = vec![a1.clone(), a2.clone()];
        want.sort();
        assert_eq!(removed, want);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(*bucket.listed.lock().expect("lock"), vec!["a/".to_string()]);
        let mut remaining = bucket.remaining();
        remaining.sort();
        let mut expected: Vec<String> = others.to_vec();
        expected.sort();
        assert_eq!(remaining, expected);
    }

    /// Nothing uploaded (or no bucket at all): nothing to do, no warning.
    #[tokio::test]
    async fn nothing_uploaded_is_nothing_to_remove() {
        let bucket = FakeBucket::with(&[]);
        assert_eq!(remove(&bucket, "a").await, CleanupReport::default());
    }

    /// The exact command that removes what destroy could not.
    fn s3_cleanup_command(bucket: &str) -> String {
        format!(
            "aws s3 rm s3://{bucket}/acme/ --recursive --exclude \"*\" --include \
             \"bootstrap-*.zip\" --region eu-west-2"
        )
    }

    /// A listing that fails warns with the exact cleanup command and does
    /// not fail the destroy.
    #[tokio::test]
    async fn a_listing_failure_warns_with_the_exact_command() {
        let bucket = FakeBucket {
            fail_list: true,
            ..FakeBucket::with(&[])
        };
        let report = remove(&bucket, "acme").await;
        assert_eq!(report.removed_artifacts, Vec::<String>::new());
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains("AccessDenied"),
            "{:?}",
            report.warnings
        );
        assert!(
            report.warnings[0].contains(&s3_cleanup_command(BUCKET)),
            "{:?}",
            report.warnings
        );
    }

    /// A delete that fails warns (with the command) and the others are still
    /// removed.
    #[tokio::test]
    async fn a_delete_failure_warns_and_the_rest_are_removed() {
        let stuck = uploaded("acme", b"one");
        let fine = uploaded("acme", b"two");
        let bucket = FakeBucket {
            fail_delete: Some(stuck.clone()),
            ..FakeBucket::with(&[stuck.as_str(), fine.as_str()])
        };
        let report = remove(&bucket, "acme").await;
        assert_eq!(report.removed_artifacts, vec![fine]);
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(report.warnings[0].contains(&stuck), "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(&s3_cleanup_command(BUCKET)),
            "{:?}",
            report.warnings
        );
    }

    /// Without the account (STS failed), the bucket name is unknown: nothing
    /// is listed, and the command looks the account up itself.
    #[tokio::test]
    async fn an_unknown_account_prints_the_command_with_the_account_lookup() {
        let bucket = FakeBucket::with(&[]);
        let mut report = CleanupReport::default();
        remove_artifacts(&bucket, None, "acme", "eu-west-2", &mut report).await;
        assert!(bucket.listed.lock().expect("lock").is_empty());
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(&s3_cleanup_command(
                "pmcp-deploy-$(aws sts get-caller-identity --query Account --output text)-eu-west-2"
            )),
            "{:?}",
            report.warnings
        );
    }

    // ---------- #407: the functions' log groups ----------

    /// In-memory `CloudWatch` Logs: which groups exist, what was deleted.
    #[derive(Default)]
    struct FakeLogs {
        existing: Mutex<std::collections::BTreeSet<String>>,
        deleted: Mutex<Vec<String>>,
        fail: bool,
    }

    impl FakeLogs {
        fn with(groups: &[&str]) -> Self {
            Self {
                existing: Mutex::new(groups.iter().map(|g| (*g).to_string()).collect()),
                ..Self::default()
            }
        }
    }

    #[async_trait]
    impl LogGroupStore for FakeLogs {
        async fn delete_log_group(&self, name: &str) -> Result<bool> {
            if self.fail {
                anyhow::bail!("AccessDenied: logs:DeleteLogGroup");
            }
            self.deleted.lock().expect("lock").push(name.to_string());
            Ok(self.existing.lock().expect("lock").remove(name))
        }
    }

    async fn sweep(logs: &FakeLogs, inventory: Option<&StackInventory>) -> CleanupReport {
        let mut report = CleanupReport::default();
        remove_log_groups(logs, inventory, "acme", "eu-west-2", &mut report).await;
        report
    }

    /// #407, the reported case (spike 021 step 6): after the stack was
    /// deleted, `/aws/lambda/<function>` was still there (Lambda re-created
    /// it after `CloudFormation` deleted the stack's own log group, which goes
    /// before the function). Destroy deletes it.
    #[tokio::test]
    async fn destroy_removes_a_recreated_function_log_group() {
        let logs = FakeLogs::with(&["/aws/lambda/acme", "/aws/lambda/other"]);
        let report = sweep(&logs, Some(&acme_inventory())).await;
        assert_eq!(
            report.removed_log_groups,
            vec!["/aws/lambda/acme".to_string()]
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(
            *logs.existing.lock().expect("lock"),
            std::collections::BTreeSet::from(["/aws/lambda/other".to_string()])
        );
    }

    /// The normal case: the stack's own log group went with the stack and
    /// nothing re-created it. Nothing to report.
    #[tokio::test]
    async fn a_log_group_already_gone_is_fine() {
        let logs = FakeLogs::with(&[]);
        assert_eq!(
            sweep(&logs, Some(&acme_inventory())).await,
            CleanupReport::default()
        );
    }

    /// A log group the stack's template retains (`DeletionPolicy: Retain`)
    /// was kept on purpose: it is left, and said so.
    #[tokio::test]
    async fn a_retained_log_group_is_kept() {
        let logs = FakeLogs::with(&["/aws/lambda/acme"]);
        let inventory = StackInventory {
            function_names: vec!["acme".to_string()],
            retained_log_groups: vec!["/aws/lambda/acme".to_string()],
        };
        let report = sweep(&logs, Some(&inventory)).await;
        assert!(logs.deleted.lock().expect("lock").is_empty());
        assert_eq!(
            report.retained_log_groups,
            vec!["/aws/lambda/acme".to_string()]
        );
    }

    /// A delete that fails warns with the exact command.
    #[tokio::test]
    async fn a_log_group_delete_failure_warns_with_the_command() {
        let logs = FakeLogs {
            fail: true,
            ..FakeLogs::with(&["/aws/lambda/acme"])
        };
        let report = sweep(&logs, Some(&acme_inventory())).await;
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(
                "aws logs delete-log-group --log-group-name /aws/lambda/acme --region eu-west-2"
            ),
            "{:?}",
            report.warnings
        );
    }

    /// Without an inventory, which log groups are the stack's (and whether
    /// one is retained) is unknown: nothing is deleted, and the command for
    /// the `[server] name` function's group is printed instead.
    #[tokio::test]
    async fn without_an_inventory_nothing_is_deleted_and_the_command_is_printed() {
        let logs = FakeLogs::with(&["/aws/lambda/acme"]);
        let report = sweep(&logs, None).await;
        assert!(logs.deleted.lock().expect("lock").is_empty());
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(
                "aws logs delete-log-group --log-group-name /aws/lambda/acme --region eu-west-2"
            ),
            "{:?}",
            report.warnings
        );
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// A superset of the names `[server] name` allows (it takes letters,
        /// digits and `-`; `.` and `_` are added for a stricter boundary),
        /// short so the prefix boundary is exercised often.
        fn server_name() -> impl Strategy<Value = String> {
            "[a-z0-9][a-z0-9._-]{0,5}"
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            /// #406: a key is selected for `server` exactly when it is one
            /// the deploy engine writes for `server`; a key written for any
            /// OTHER server — including one whose name extends `server`
            /// (`a` vs `ab`, `a-b`, `a.b`) — never is.
            #[test]
            fn only_keys_deploy_writes_for_this_server_are_selected(
                server in server_name(),
                other in server_name(),
                suffix in "[a-z0-9._-]{1,3}",
                bytes in prop::collection::vec(any::<u8>(), 0..32),
            ) {
                prop_assert!(is_owned_artifact_key(&server, &uploaded(&server, &bytes)));
                let extended = format!("{server}{suffix}");
                prop_assert!(!is_owned_artifact_key(&server, &uploaded(&extended, &bytes)));
                prop_assume!(other != server);
                prop_assert!(!is_owned_artifact_key(&server, &uploaded(&other, &bytes)));
            }

            /// Nothing but a direct `bootstrap-<lowercase hex>.zip` child of
            /// `{server}/` is ever selected.
            #[test]
            fn arbitrary_keys_under_the_prefix_are_not_selected(
                server in server_name(),
                rest in "[ -~]{0,24}",
            ) {
                let key = format!("{server}/{rest}");
                let shape = rest
                    .strip_prefix("bootstrap-")
                    .and_then(|r| r.strip_suffix(".zip"))
                    .is_some_and(|hex| !hex.is_empty()
                        && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
                prop_assert_eq!(is_owned_artifact_key(&server, &key), shape);
            }
        }
    }
}
