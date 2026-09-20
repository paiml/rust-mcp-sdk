//! CFN deploy engine (Task 9): applies an already-rendered CloudFormation
//! template directly against real AWS APIs — `aws-sdk-cloudformation` +
//! `aws-sdk-s3` + `aws-sdk-sts` — with NO `npx`, NO CDK, and NO Node.js
//! anywhere in the path. This is what lets the `aws-lambda` deploy target's
//! renderer path (Task 9's caller, `aws_lambda::deploy::deploy_aws_lambda`)
//! deploy an unmodified-scaffold project without any JS tooling installed.
//!
//! # Flow
//!
//! 1. Ensure the deploy-artifact S3 bucket exists ([`ensure_bucket`]) —
//!    convention `pmcp-deploy-{account_id}-{region}`, created private if
//!    missing (idempotent: an "already exists, owned by you" race is
//!    tolerated as success, never an error).
//! 2. Upload the bootstrap zip's already-read bytes to
//!    `{server}/bootstrap-{sha256-prefix}.zip` ([`upload_artifact`]) — the
//!    caller's single [`artifact_s3_key`] call (BEFORE this module ever
//!    runs) derives both the key baked into the already-rendered template's
//!    `Code.S3Key` AND the key `EngineParams::s3_key` threads through here,
//!    so there is no second read-and-rehash of the zip and no derivation to
//!    keep in sync.
//! 3. Classify create-vs-update from `describe_stacks` and apply the
//!    template ([`apply_stack`]) — "No updates are to be performed" is
//!    success, not an error.
//! 4. Poll `describe_stacks` to a terminal status ([`poll_to_terminal`]);
//!    CREATE_COMPLETE/UPDATE_COMPLETE succeed, ROLLBACK*/*_FAILED fetch the
//!    last ~10 `describe_stack_events` failures and bail with them.
//! 5. Read the stack's `Outputs` into a [`DeploymentOutputs`] using the
//!    output names the renderer emits (`ApiUrl`/`DashboardUrl`/`LambdaArn`/
//!    `McpRoleArn`, or the Cognito+DCR shape's own set — see
//!    `pmcp_cfn_renderer::resources::outputs`), AND write `deploy/outputs.json`
//!    in the exact shape [`crate::deployment::outputs::load_cdk_outputs`]
//!    parses, for `cargo pmcp outputs`/`status` compat.
//!
//! # Testability (T8 `Downloader`-trait precedent)
//!
//! The CloudFormation describe/poll decision logic never touches
//! `aws_sdk_cloudformation` types directly — it is written against the
//! small [`StackDescriber`] trait (real implementation: [`AwsStackDescriber`],
//! a thin pass-through adapter; tests: a scripted stub returning a canned
//! status SEQUENCE, mirroring T8's `StubDownloader`). Likewise bucket
//! idempotency is written against [`BucketEnsurer`]. Everything else
//! interesting (status classification, failure-event formatting,
//! `outputs.json` shape, bucket-name/artifact-key derivation) is a plain
//! function of plain data, unit-tested directly with no trait needed at
//! all. Live AWS calls (the two trait implementations, plus `create_stack`/
//! `update_stack`/`put_object`) are exercised only by the Task 10 real-deploy
//! gate — never in this crate's default test harness (repo rule: no
//! live-cloud calls in CI).

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::deployment::r#trait::DeploymentOutputs;

/// Poll interval for `describe_stacks` while waiting for a terminal status.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// How many trailing hex characters of the artifact's SHA-256 digest to use
/// in its S3 key (`{server}/bootstrap-{prefix}.zip`) — 12 hex chars = 48
/// bits, ample collision resistance for a per-server cache key while
/// keeping keys short and readable in the S3 console.
const ARTIFACT_KEY_DIGEST_PREFIX_LEN: usize = 12;

/// How many trailing failure events to report when a deploy fails (brief:
/// "last ~10 failure events").
const MAX_FAILURE_EVENTS: usize = 10;

/// Public engine parameters (Task 9 interface).
///
/// `artifact_bytes`/`s3_key` are threaded through from the caller's OWN
/// single [`artifact_s3_key`] call (simplify-wave fix — this module used to
/// re-read `artifact_zip` from disk and re-derive its digest/key a second
/// time here, duplicating work the caller already did to build
/// `ArtifactRef::s3_key`/`digest` for the template). `s3_key` MUST be the
/// exact same key the caller baked into `template_json`'s `Code.S3Key` —
/// since both now come from that ONE call, there is no drift risk to guard
/// against. `project_root` is where `deploy/outputs.json` is written (step
/// 5) — there is no other way to recover it from the other fields.
#[derive(Debug, Clone)]
pub struct EngineParams {
    pub stack_name: String,
    pub region: String,
    /// The artifact zip's already-read bytes, uploaded verbatim to `s3_key`.
    pub artifact_bytes: Vec<u8>,
    /// The exact S3 key to upload `artifact_bytes` under — see the struct
    /// doc comment for why this must match `template_json`'s `Code.S3Key`.
    pub s3_key: String,
    pub bucket: String,
    /// Project root, for writing `deploy/outputs.json` (step 5).
    pub project_root: PathBuf,
}

/// Deploy an already-rendered CloudFormation `template_json` per the flow
/// documented on the module. Returns the stack's outputs, and ALSO writes
/// `deploy/outputs.json` under `params.project_root` as a side effect (for
/// `status`/other consumers that read it directly).
pub async fn deploy_stack(template_json: &str, params: EngineParams) -> Result<DeploymentOutputs> {
    let aws_cfg = load_aws_config(&params.region).await;
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);
    let cfn = aws_sdk_cloudformation::Client::new(&aws_cfg);

    ensure_bucket(
        &AwsBucketEnsurer { client: &s3 },
        &params.bucket,
        &params.region,
    )
    .await?;

    upload_artifact(&s3, &params.bucket, &params.s3_key, params.artifact_bytes).await?;

    let describer = AwsStackDescriber { client: &cfn };
    apply_stack(&cfn, &params.stack_name, template_json, &describer).await?;

    let raw_outputs = poll_to_terminal(&describer, &params.stack_name, POLL_INTERVAL).await?;

    write_outputs_json(&params.project_root, &params.stack_name, &raw_outputs)?;
    Ok(outputs_from_raw(
        &raw_outputs,
        &params.region,
        &params.stack_name,
    ))
}

/// Resolve the calling AWS identity's account id via STS — used by the
/// caller (`aws_lambda::deploy`) both to derive [`bucket_name`] and to
/// populate `pmcp_cfn_renderer::RenderParams::account_id` BEFORE rendering
/// (the account must be known at render time; it is baked as a literal into
/// several ARNs the renderer emits — see `resources::http_api`'s doc
/// comment).
pub(crate) async fn resolve_account_id(region: &str) -> Result<String> {
    let aws_cfg = load_aws_config(region).await;
    let sts = aws_sdk_sts::Client::new(&aws_cfg);
    let identity = sts
        .get_caller_identity()
        .send()
        .await
        .context("STS GetCallerIdentity failed — check AWS credentials/region")?;
    identity
        .account()
        .map(str::to_string)
        .context("STS GetCallerIdentity did not return an account id")
}

/// Load the AWS SDK config for `region` (standard credential/region chain
/// via `aws-config`).
async fn load_aws_config(region: &str) -> aws_config::SdkConfig {
    aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_string()))
        .load()
        .await
}

// ===========================================================================
// Bucket naming + artifact key derivation (pure, unit-tested directly)
// ===========================================================================

/// The deploy-artifact bucket-naming convention this module DEFINES (no
/// config field for it exists — see the Task 9 brief's Interfaces section):
/// `pmcp-deploy-{account_id}-{region}`.
#[must_use]
pub(crate) fn bucket_name(account_id: &str, region: &str) -> String {
    format!("pmcp-deploy-{account_id}-{region}")
}

/// Derive `(digest_hex, s3_key)` for an artifact zip's bytes:
/// `{server_name}/bootstrap-{sha256-prefix}.zip`. Called exactly ONCE by
/// the caller (`aws_lambda::deploy::try_render_and_deploy`), which threads
/// both the digest (as `ArtifactRef::digest`) and the key (as
/// `ArtifactRef::s3_key`/`EngineParams::s3_key`) onward — see
/// [`EngineParams`]'s doc comment. Hashes via
/// [`pmcp_package::digest::ManifestDigest::from_bytes`] (this crate's
/// shared content-digest primitive) rather than hand-rolling SHA-256; its
/// `sha256:<hex>` output is stripped back to bare hex here to preserve this
/// function's existing `(hex, key)` contract.
#[must_use]
pub(crate) fn artifact_s3_key(server_name: &str, zip_bytes: &[u8]) -> (String, String) {
    let digest = pmcp_package::digest::ManifestDigest::from_bytes(zip_bytes);
    let hex = digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("ManifestDigest::from_bytes always yields a sha256:-prefixed digest")
        .to_string();
    let prefix = &hex[..ARTIFACT_KEY_DIGEST_PREFIX_LEN.min(hex.len())];
    let key = format!("{server_name}/bootstrap-{prefix}.zip");
    (hex, key)
}

// ===========================================================================
// Bucket ensure (thin trait seam over aws-sdk-s3 — T8 Downloader precedent)
// ===========================================================================

/// Outcome of a bucket-creation attempt, abstracted away from
/// `aws_sdk_s3::operation::create_bucket::CreateBucketError` so
/// [`ensure_bucket`]'s race-tolerance logic is testable without the SDK.
enum EnsureBucketOutcome {
    Created,
    /// The "already exists, owned by you" race (brief: "handle
    /// already-exists-owned race") — tolerated as success.
    AlreadyOwnedByYou,
}

/// S3 bucket existence-check + creation, abstracted for testability.
#[async_trait]
trait BucketEnsurer: Send + Sync {
    async fn exists(&self, bucket: &str) -> Result<bool>;
    async fn create(&self, bucket: &str, region: &str) -> Result<EnsureBucketOutcome>;
}

/// Ensure `bucket` exists (creating it, private, if missing) — the WRAPPER
/// logic under test: check-then-create, tolerating the already-owned race.
/// S3 buckets are private by default (Block Public Access has been the
/// account-level default since April 2023, and no bucket policy is ever
/// attached here), so no separate ACL/public-access-block call is made.
async fn ensure_bucket(ensurer: &dyn BucketEnsurer, bucket: &str, region: &str) -> Result<()> {
    if ensurer.exists(bucket).await? {
        return Ok(());
    }
    match ensurer.create(bucket, region).await? {
        EnsureBucketOutcome::Created | EnsureBucketOutcome::AlreadyOwnedByYou => Ok(()),
    }
}

struct AwsBucketEnsurer<'a> {
    client: &'a aws_sdk_s3::Client,
}

#[async_trait]
impl BucketEnsurer for AwsBucketEnsurer<'_> {
    async fn exists(&self, bucket: &str) -> Result<bool> {
        match self.client.head_bucket().bucket(bucket).send().await {
            Ok(_) => Ok(true),
            Err(err) => {
                if matches!(
                    err.as_service_error(),
                    Some(aws_sdk_s3::operation::head_bucket::HeadBucketError::NotFound(_))
                ) {
                    Ok(false)
                } else {
                    Err(err).context("S3 HeadBucket failed")
                }
            },
        }
    }

    async fn create(&self, bucket: &str, region: &str) -> Result<EnsureBucketOutcome> {
        let mut req = self.client.create_bucket().bucket(bucket);
        // us-east-1 is S3's default region: passing a LocationConstraint for
        // it is REJECTED by the API (`InvalidLocationConstraint`) — every
        // other region requires one.
        if region != "us-east-1" {
            let configuration = aws_sdk_s3::types::CreateBucketConfiguration::builder()
                .location_constraint(aws_sdk_s3::types::BucketLocationConstraint::from(region))
                .build();
            req = req.create_bucket_configuration(configuration);
        }
        match req.send().await {
            Ok(_) => Ok(EnsureBucketOutcome::Created),
            Err(err) => {
                if matches!(
                    err.as_service_error(),
                    Some(aws_sdk_s3::operation::create_bucket::CreateBucketError::BucketAlreadyOwnedByYou(_))
                ) {
                    Ok(EnsureBucketOutcome::AlreadyOwnedByYou)
                } else {
                    Err(err).context("S3 CreateBucket failed")
                }
            },
        }
    }
}

/// Upload the artifact zip's already-read `bytes` to `bucket`/`key`.
async fn upload_artifact(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    bytes: Vec<u8>,
) -> Result<()> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(aws_sdk_s3::primitives::ByteStream::from(bytes))
        .send()
        .await
        .with_context(|| format!("S3 PutObject failed for s3://{bucket}/{key}"))?;
    Ok(())
}

// ===========================================================================
// Stack describe/classify/poll (thin trait seam — T8 Downloader precedent)
// ===========================================================================

/// Result of describing a stack by name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StackLookup {
    /// No stack with this name exists yet (CREATE path).
    NotFound,
    /// A stack exists; carries its current status and (if any) outputs.
    Found {
        status: String,
        outputs: Vec<(String, String)>,
    },
}

/// One CloudFormation stack event, reduced to the three fields
/// [`format_failure_events`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StackEvent {
    logical_id: String,
    status: String,
    reason: Option<String>,
}

/// Describe/poll operations, abstracted away from `aws_sdk_cloudformation`
/// types so the classification/poll-loop logic is unit-testable via a
/// scripted stub (T8 `Downloader`-trait precedent).
#[async_trait]
trait StackDescriber: Send + Sync {
    async fn describe(&self, stack_name: &str) -> Result<StackLookup>;
    /// The most recent (up to [`MAX_FAILURE_EVENTS`]) failure-classified
    /// events for `stack_name`, newest first.
    async fn recent_failure_events(&self, stack_name: &str) -> Result<Vec<StackEvent>>;
}

/// `Create` when no stack exists yet; `Update` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackAction {
    Create,
    Update,
}

fn classify_action(lookup: &StackLookup) -> StackAction {
    match lookup {
        StackLookup::NotFound => StackAction::Create,
        StackLookup::Found { .. } => StackAction::Update,
    }
}

/// Whether a stack status is terminal, and if so, whether it succeeded.
/// `None` means still in progress — keep polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalOutcome {
    Success,
    Failure,
}

/// Classify a raw CloudFormation `StackStatus` string. Brief: "CREATE_COMPLETE/
/// UPDATE_COMPLETE ok; ROLLBACK*/FAILED → bail". Written as an explicit table
/// (not a `starts_with`/`ends_with` heuristic) because `UPDATE_ROLLBACK_COMPLETE`
/// ENDS in `_COMPLETE`, not `_FAILED`, yet represents a failed update that
/// successfully rolled back — a heuristic would misclassify it as success.
fn classify_terminal_status(status: &str) -> Option<TerminalOutcome> {
    match status {
        "CREATE_COMPLETE" | "UPDATE_COMPLETE" => Some(TerminalOutcome::Success),
        "CREATE_FAILED"
        | "ROLLBACK_COMPLETE"
        | "ROLLBACK_FAILED"
        | "UPDATE_ROLLBACK_COMPLETE"
        | "UPDATE_ROLLBACK_FAILED"
        | "DELETE_FAILED" => Some(TerminalOutcome::Failure),
        _ => None,
    }
}

/// `true` when a CloudFormation API error's displayed message indicates the
/// stack does not exist. CloudFormation models this as an untyped/generic
/// error (`DescribeStacksError` has no dedicated "not found" variant), so
/// this is a documented substring match on the message CloudFormation
/// itself uses (`"<name> does not exist"`), not a typed error match.
fn is_stack_not_found_error(msg: &str) -> bool {
    msg.contains("does not exist")
}

/// `true` when an `UpdateStack` error's displayed message is CloudFormation's
/// "nothing changed" response — brief: "treat `No updates are to be
/// performed` as success".
fn is_no_updates_error(msg: &str) -> bool {
    msg.contains("No updates are to be performed")
}

/// Create-or-update `stack_name` from `template_json`, classifying via
/// `describer.describe` first. An `UpdateStack` "no updates" response is
/// treated as success, not an error.
async fn apply_stack(
    cfn: &aws_sdk_cloudformation::Client,
    stack_name: &str,
    template_json: &str,
    describer: &dyn StackDescriber,
) -> Result<()> {
    let lookup = describer.describe(stack_name).await?;
    match classify_action(&lookup) {
        StackAction::Create => {
            cfn.create_stack()
                .stack_name(stack_name)
                .template_body(template_json)
                .capabilities(aws_sdk_cloudformation::types::Capability::CapabilityIam)
                .send()
                .await
                .context("CloudFormation CreateStack failed")?;
        },
        StackAction::Update => {
            let result = cfn
                .update_stack()
                .stack_name(stack_name)
                .template_body(template_json)
                .capabilities(aws_sdk_cloudformation::types::Capability::CapabilityIam)
                .send()
                .await;
            if let Err(err) = result {
                if !is_no_updates_error(&err.to_string()) {
                    return Err(err).context("CloudFormation UpdateStack failed");
                }
            }
        },
    }
    Ok(())
}

/// Poll `describer.describe` every `interval` until a terminal status,
/// returning the stack's raw `(OutputKey, OutputValue)` pairs on success.
/// On failure, fetches the recent failure events and bails with them
/// formatted via [`format_failure_events`].
async fn poll_to_terminal(
    describer: &dyn StackDescriber,
    stack_name: &str,
    interval: Duration,
) -> Result<Vec<(String, String)>> {
    loop {
        let lookup = describer.describe(stack_name).await?;
        let StackLookup::Found { status, outputs } = lookup else {
            bail!("stack '{stack_name}' disappeared while polling for completion");
        };
        match classify_terminal_status(&status) {
            Some(TerminalOutcome::Success) => return Ok(outputs),
            Some(TerminalOutcome::Failure) => {
                let events = describer.recent_failure_events(stack_name).await?;
                bail!(format_failure_events(stack_name, &status, &events));
            },
            None => tokio::time::sleep(interval).await,
        }
    }
}

/// Build the bail message for a failed stack: status + up to
/// [`MAX_FAILURE_EVENTS`] recent failure events.
fn format_failure_events(stack_name: &str, status: &str, events: &[StackEvent]) -> String {
    let mut msg = format!(
        "CloudFormation stack '{stack_name}' failed (status: {status}). Recent failure events:\n"
    );
    if events.is_empty() {
        msg.push_str("  (no failure events available)\n");
    }
    for event in events.iter().take(MAX_FAILURE_EVENTS) {
        msg.push_str(&format!(
            "  - {} [{}]: {}\n",
            event.logical_id,
            event.status,
            event.reason.as_deref().unwrap_or("no reason given")
        ));
    }
    msg
}

struct AwsStackDescriber<'a> {
    client: &'a aws_sdk_cloudformation::Client,
}

#[async_trait]
impl StackDescriber for AwsStackDescriber<'_> {
    async fn describe(&self, stack_name: &str) -> Result<StackLookup> {
        match self
            .client
            .describe_stacks()
            .stack_name(stack_name)
            .send()
            .await
        {
            Ok(output) => {
                let Some(stack) = output.stacks().first() else {
                    return Ok(StackLookup::NotFound);
                };
                let status = stack
                    .stack_status()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_default();
                let outputs = stack
                    .outputs()
                    .iter()
                    .filter_map(|o| {
                        Some((o.output_key()?.to_string(), o.output_value()?.to_string()))
                    })
                    .collect();
                Ok(StackLookup::Found { status, outputs })
            },
            Err(err) => {
                if is_stack_not_found_error(&err.to_string()) {
                    Ok(StackLookup::NotFound)
                } else {
                    Err(err).context("CloudFormation DescribeStacks failed")
                }
            },
        }
    }

    async fn recent_failure_events(&self, stack_name: &str) -> Result<Vec<StackEvent>> {
        let output = self
            .client
            .describe_stack_events()
            .stack_name(stack_name)
            .send()
            .await
            .context("CloudFormation DescribeStackEvents failed")?;
        let events = output
            .stack_events()
            .iter()
            .filter(|e| {
                e.resource_status()
                    .is_some_and(|s| s.as_str().contains("FAILED"))
            })
            .take(MAX_FAILURE_EVENTS)
            .map(|e| StackEvent {
                logical_id: e.logical_resource_id().unwrap_or("?").to_string(),
                status: e
                    .resource_status()
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_default(),
                reason: e.resource_status_reason().map(str::to_string),
            })
            .collect();
        Ok(events)
    }
}

// ===========================================================================
// Outputs: DeploymentOutputs mapping + deploy/outputs.json compat
// ===========================================================================

/// Map raw `(OutputKey, OutputValue)` pairs into a [`DeploymentOutputs`].
/// `url` comes from `ApiUrl` (the name the renderer emits — both the plain
/// `aws-lambda` shape and the Cognito+DCR shape use it); every raw pair is
/// ALSO stashed into `custom` under its original CFN key name.
fn outputs_from_raw(raw: &[(String, String)], region: &str, stack_name: &str) -> DeploymentOutputs {
    let url = raw
        .iter()
        .find(|(k, _)| k == "ApiUrl")
        .map(|(_, v)| v.clone());
    let custom = raw
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    DeploymentOutputs {
        url,
        additional_urls: vec![],
        regions: vec![region.to_string()],
        stack_name: Some(stack_name.to_string()),
        version: None,
        custom,
    }
}

/// Build the JSON body `deploy/outputs.json` wraps under `stack_name` — the
/// exact shape [`crate::deployment::outputs::load_cdk_outputs`] parses
/// (`CdkStackOutputs`, no `deny_unknown_fields`, so passing through every
/// raw CFN output key — not just the ones that struct names — is safe;
/// extras are silently ignored by that parser).
fn outputs_json_body(raw: &[(String, String)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in raw {
        map.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    serde_json::Value::Object(map)
}

/// Write `deploy/outputs.json` under `project_root`, in the
/// `{ "<stack_name>": { ...raw outputs... } }` shape `load_cdk_outputs`
/// reads (it indexes by the first-and-only top-level key, not by name).
fn write_outputs_json(
    project_root: &Path,
    stack_name: &str,
    raw: &[(String, String)],
) -> Result<()> {
    let deploy_dir = project_root.join("deploy");
    std::fs::create_dir_all(&deploy_dir)
        .with_context(|| format!("failed to create {}", deploy_dir.display()))?;
    let wrapped = serde_json::json!({ stack_name: outputs_json_body(raw) });
    let text =
        serde_json::to_string_pretty(&wrapped).context("failed to serialize outputs.json")?;
    let path = deploy_dir.join("outputs.json");
    std::fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    // -----------------------------------------------------------------
    // bucket_name / artifact_s3_key derivation
    // -----------------------------------------------------------------

    #[test]
    fn bucket_name_matches_the_documented_convention() {
        assert_eq!(
            bucket_name("123456789012", "us-east-1"),
            "pmcp-deploy-123456789012-us-east-1"
        );
    }

    #[test]
    fn artifact_s3_key_is_deterministic_for_identical_bytes() {
        let (digest_a, key_a) = artifact_s3_key("my-server", b"same bytes");
        let (digest_b, key_b) = artifact_s3_key("my-server", b"same bytes");
        assert_eq!(digest_a, digest_b);
        assert_eq!(key_a, key_b);
        assert!(key_a.starts_with("my-server/bootstrap-"));
        assert!(key_a.ends_with(".zip"));
    }

    #[test]
    fn artifact_s3_key_differs_for_different_bytes() {
        let (_, key_a) = artifact_s3_key("my-server", b"version one");
        let (_, key_b) = artifact_s3_key("my-server", b"version two");
        assert_ne!(
            key_a, key_b,
            "different artifact content must key differently"
        );
    }

    #[test]
    fn artifact_s3_key_uses_the_documented_digest_prefix_length() {
        let (digest, key) = artifact_s3_key("srv", b"content");
        let expected_prefix = &digest[..ARTIFACT_KEY_DIGEST_PREFIX_LEN];
        assert_eq!(key, format!("srv/bootstrap-{expected_prefix}.zip"));
    }

    // -----------------------------------------------------------------
    // classify_action / classify_terminal_status (pure decision helpers)
    // -----------------------------------------------------------------

    #[test]
    fn classify_action_not_found_is_create() {
        assert_eq!(classify_action(&StackLookup::NotFound), StackAction::Create);
    }

    #[test]
    fn classify_action_found_is_update_regardless_of_status() {
        let found = StackLookup::Found {
            status: "CREATE_COMPLETE".to_string(),
            outputs: vec![],
        };
        assert_eq!(classify_action(&found), StackAction::Update);
    }

    #[test]
    fn classify_terminal_status_success_cases() {
        assert_eq!(
            classify_terminal_status("CREATE_COMPLETE"),
            Some(TerminalOutcome::Success)
        );
        assert_eq!(
            classify_terminal_status("UPDATE_COMPLETE"),
            Some(TerminalOutcome::Success)
        );
    }

    #[test]
    fn classify_terminal_status_failure_cases() {
        for status in [
            "CREATE_FAILED",
            "ROLLBACK_COMPLETE",
            "ROLLBACK_FAILED",
            "UPDATE_ROLLBACK_COMPLETE",
            "UPDATE_ROLLBACK_FAILED",
            "DELETE_FAILED",
        ] {
            assert_eq!(
                classify_terminal_status(status),
                Some(TerminalOutcome::Failure),
                "{status} must classify as Failure"
            );
        }
    }

    #[test]
    fn classify_terminal_status_in_progress_is_none() {
        for status in [
            "CREATE_IN_PROGRESS",
            "UPDATE_IN_PROGRESS",
            "ROLLBACK_IN_PROGRESS",
            "UPDATE_ROLLBACK_IN_PROGRESS",
            "REVIEW_IN_PROGRESS",
            "UPDATE_COMPLETE_CLEANUP_IN_PROGRESS",
        ] {
            assert_eq!(
                classify_terminal_status(status),
                None,
                "{status} must keep polling (None)"
            );
        }
    }

    #[test]
    fn is_stack_not_found_error_matches_cfn_wording() {
        assert!(is_stack_not_found_error(
            "Stack with id my-stack does not exist"
        ));
        assert!(!is_stack_not_found_error("Access denied"));
    }

    #[test]
    fn is_no_updates_error_matches_cfn_wording() {
        assert!(is_no_updates_error("No updates are to be performed."));
        assert!(!is_no_updates_error("Some other update failure"));
    }

    // -----------------------------------------------------------------
    // format_failure_events
    // -----------------------------------------------------------------

    #[test]
    fn format_failure_events_includes_status_and_every_event() {
        let events = vec![
            StackEvent {
                logical_id: "McpFunction".to_string(),
                status: "CREATE_FAILED".to_string(),
                reason: Some("Resource creation cancelled".to_string()),
            },
            StackEvent {
                logical_id: "ExecutionRole".to_string(),
                status: "CREATE_FAILED".to_string(),
                reason: None,
            },
        ];
        let msg = format_failure_events("demo-stack", "ROLLBACK_COMPLETE", &events);
        assert!(msg.contains("demo-stack"));
        assert!(msg.contains("ROLLBACK_COMPLETE"));
        assert!(msg.contains("McpFunction"));
        assert!(msg.contains("Resource creation cancelled"));
        assert!(msg.contains("ExecutionRole"));
        assert!(msg.contains("no reason given"));
    }

    #[test]
    fn format_failure_events_caps_at_max_failure_events() {
        let events: Vec<StackEvent> = (0..20)
            .map(|i| StackEvent {
                logical_id: format!("Resource{i}"),
                status: "CREATE_FAILED".to_string(),
                reason: None,
            })
            .collect();
        let msg = format_failure_events("demo-stack", "ROLLBACK_COMPLETE", &events);
        let reported = (0..20)
            .filter(|i| msg.contains(&format!("Resource{i}")))
            .count();
        assert_eq!(reported, MAX_FAILURE_EVENTS);
    }

    // -----------------------------------------------------------------
    // outputs_from_raw / outputs.json round-trip against load_cdk_outputs
    // -----------------------------------------------------------------

    #[test]
    fn outputs_from_raw_maps_api_url_and_stashes_everything_into_custom() {
        let raw = vec![
            ("ApiUrl".to_string(), "https://x.example.com".to_string()),
            (
                "DashboardUrl".to_string(),
                "https://console.aws".to_string(),
            ),
            ("LambdaArn".to_string(), "arn:aws:lambda:...".to_string()),
        ];
        let outputs = outputs_from_raw(&raw, "us-east-1", "demo-stack");
        assert_eq!(outputs.url, Some("https://x.example.com".to_string()));
        assert_eq!(outputs.regions, vec!["us-east-1".to_string()]);
        assert_eq!(outputs.stack_name, Some("demo-stack".to_string()));
        assert_eq!(
            outputs.custom.get("LambdaArn"),
            Some(&serde_json::Value::String("arn:aws:lambda:...".to_string()))
        );
    }

    #[test]
    fn outputs_from_raw_url_is_none_when_no_api_url_output() {
        let raw = vec![("SomeOtherKey".to_string(), "value".to_string())];
        let outputs = outputs_from_raw(&raw, "us-east-1", "demo-stack");
        assert_eq!(outputs.url, None);
    }

    /// Round-trip against the REAL parser (`load_cdk_outputs`) — proves
    /// `write_outputs_json`'s shape is exactly what that function reads,
    /// not just that it "looks similar."
    #[test]
    fn write_outputs_json_round_trips_through_load_cdk_outputs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let raw = vec![
            ("ApiUrl".to_string(), "https://x.example.com".to_string()),
            (
                "DashboardUrl".to_string(),
                "https://console.aws".to_string(),
            ),
            ("LambdaArn".to_string(), "arn:aws:lambda:...".to_string()),
            ("McpRoleArn".to_string(), "arn:aws:iam:...".to_string()),
        ];
        write_outputs_json(tmp.path(), "demo-stack", &raw).expect("write succeeds");

        let outputs =
            crate::deployment::outputs::load_cdk_outputs(tmp.path(), "us-east-1", "demo-stack")
                .expect("load_cdk_outputs must parse what we just wrote");
        assert_eq!(outputs.url, Some("https://x.example.com".to_string()));
        assert_eq!(
            outputs.custom.get("dashboard_url"),
            Some(&serde_json::json!("https://console.aws"))
        );
    }

    /// The Cognito+DCR output shape (`OAuthDiscoveryUrl`/`UserPoolId`, no
    /// `ApiUrl`... wait, it DOES have ApiUrl too — see
    /// `render_cognito_outputs`) round-trips its OAuth-specific fields too.
    #[test]
    fn write_outputs_json_round_trips_cognito_shape_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let raw = vec![
            (
                "ApiUrl".to_string(),
                "https://oauth.example.com".to_string(),
            ),
            (
                "OAuthDiscoveryUrl".to_string(),
                "https://oauth.example.com/.well-known/openid-configuration".to_string(),
            ),
            ("UserPoolId".to_string(), "us-east-1_ABC123".to_string()),
        ];
        write_outputs_json(tmp.path(), "oauth-stack", &raw).expect("write succeeds");

        let outputs =
            crate::deployment::outputs::load_cdk_outputs(tmp.path(), "us-east-1", "oauth-stack")
                .expect("load_cdk_outputs must parse the cognito shape too");
        assert_eq!(
            outputs.custom.get("oauth_discovery_url"),
            Some(&serde_json::json!(
                "https://oauth.example.com/.well-known/openid-configuration"
            ))
        );
        assert_eq!(
            outputs.custom.get("user_pool_id"),
            Some(&serde_json::json!("us-east-1_ABC123"))
        );
    }

    // -----------------------------------------------------------------
    // ensure_bucket (BucketEnsurer trait seam — race tolerance)
    // -----------------------------------------------------------------

    struct StubBucketEnsurer {
        exists: bool,
        create_result: Mutex<Option<Result<EnsureBucketOutcome, ()>>>,
        create_calls: Mutex<u32>,
    }

    #[async_trait]
    impl BucketEnsurer for StubBucketEnsurer {
        async fn exists(&self, _bucket: &str) -> Result<bool> {
            Ok(self.exists)
        }
        async fn create(&self, _bucket: &str, _region: &str) -> Result<EnsureBucketOutcome> {
            *self.create_calls.lock().unwrap() += 1;
            match self.create_result.lock().unwrap().take() {
                Some(Ok(outcome)) => Ok(outcome),
                Some(Err(())) => bail!("simulated CreateBucket failure"),
                None => bail!("stub misconfigured: no create_result set"),
            }
        }
    }

    #[tokio::test]
    async fn ensure_bucket_skips_create_when_already_exists() {
        let stub = StubBucketEnsurer {
            exists: true,
            create_result: Mutex::new(None),
            create_calls: Mutex::new(0),
        };
        ensure_bucket(&stub, "my-bucket", "us-east-1")
            .await
            .expect("must succeed without calling create");
        assert_eq!(*stub.create_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn ensure_bucket_creates_when_missing() {
        let stub = StubBucketEnsurer {
            exists: false,
            create_result: Mutex::new(Some(Ok(EnsureBucketOutcome::Created))),
            create_calls: Mutex::new(0),
        };
        ensure_bucket(&stub, "my-bucket", "us-east-1")
            .await
            .expect("must succeed");
        assert_eq!(*stub.create_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn ensure_bucket_tolerates_already_owned_by_you_race() {
        let stub = StubBucketEnsurer {
            exists: false, // HeadBucket raced and missed a bucket that showed up by CreateBucket time
            create_result: Mutex::new(Some(Ok(EnsureBucketOutcome::AlreadyOwnedByYou))),
            create_calls: Mutex::new(0),
        };
        ensure_bucket(&stub, "my-bucket", "us-east-1")
            .await
            .expect("already-owned-by-you race must be tolerated as success");
    }

    #[tokio::test]
    async fn ensure_bucket_propagates_other_create_failures() {
        let stub = StubBucketEnsurer {
            exists: false,
            create_result: Mutex::new(Some(Err(()))),
            create_calls: Mutex::new(0),
        };
        let err = ensure_bucket(&stub, "my-bucket", "us-east-1")
            .await
            .expect_err("a non-race create failure must propagate");
        assert!(err.to_string().contains("simulated"));
    }

    // -----------------------------------------------------------------
    // poll_to_terminal (StackDescriber trait seam — scripted sequence,
    // T8 Downloader precedent)
    // -----------------------------------------------------------------

    struct ScriptedStackDescriber {
        lookups: Mutex<VecDeque<StackLookup>>,
        failure_events: Vec<StackEvent>,
    }

    #[async_trait]
    impl StackDescriber for ScriptedStackDescriber {
        async fn describe(&self, _stack_name: &str) -> Result<StackLookup> {
            self.lookups
                .lock()
                .unwrap()
                .pop_front()
                .context("scripted describer ran out of scripted lookups")
        }
        async fn recent_failure_events(&self, _stack_name: &str) -> Result<Vec<StackEvent>> {
            Ok(self.failure_events.clone())
        }
    }

    fn found(status: &str) -> StackLookup {
        StackLookup::Found {
            status: status.to_string(),
            outputs: vec![("ApiUrl".to_string(), "https://x.example.com".to_string())],
        }
    }

    #[tokio::test]
    async fn poll_to_terminal_returns_outputs_on_immediate_success() {
        let describer = ScriptedStackDescriber {
            lookups: Mutex::new(VecDeque::from([found("CREATE_COMPLETE")])),
            failure_events: vec![],
        };
        let outputs = poll_to_terminal(&describer, "demo-stack", Duration::from_millis(1))
            .await
            .expect("immediate CREATE_COMPLETE must succeed");
        assert_eq!(
            outputs,
            vec![("ApiUrl".to_string(), "https://x.example.com".to_string())]
        );
    }

    #[tokio::test]
    async fn poll_to_terminal_polls_through_in_progress_to_success() {
        let describer = ScriptedStackDescriber {
            lookups: Mutex::new(VecDeque::from([
                found("CREATE_IN_PROGRESS"),
                found("CREATE_IN_PROGRESS"),
                found("CREATE_COMPLETE"),
            ])),
            failure_events: vec![],
        };
        let outputs = poll_to_terminal(&describer, "demo-stack", Duration::from_millis(1))
            .await
            .expect("must poll through in-progress ticks to success");
        assert!(!outputs.is_empty());
    }

    #[tokio::test]
    async fn poll_to_terminal_bails_with_failure_events_on_rollback() {
        let describer = ScriptedStackDescriber {
            lookups: Mutex::new(VecDeque::from([
                found("CREATE_IN_PROGRESS"),
                found("ROLLBACK_COMPLETE"),
            ])),
            failure_events: vec![StackEvent {
                logical_id: "McpFunction".to_string(),
                status: "CREATE_FAILED".to_string(),
                reason: Some("Insufficient permissions".to_string()),
            }],
        };
        let err = poll_to_terminal(&describer, "demo-stack", Duration::from_millis(1))
            .await
            .expect_err("ROLLBACK_COMPLETE must bail");
        let msg = err.to_string();
        assert!(msg.contains("ROLLBACK_COMPLETE"));
        assert!(msg.contains("McpFunction"));
        assert!(msg.contains("Insufficient permissions"));
    }
}
