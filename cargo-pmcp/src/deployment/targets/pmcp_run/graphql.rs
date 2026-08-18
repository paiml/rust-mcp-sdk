use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::auth::{
    discover_graphql_url, load_cached_config, refresh_graphql_url, DEFAULT_GRAPHQL_URL,
};

/// The explicit endpoint override, when it is set to something usable.
///
/// An EMPTY (or whitespace-only) `PMCP_RUN_GRAPHQL_URL` is treated as ABSENT,
/// which is what `auth::refresh_graphql_url` already does through its own
/// `nonempty_env`. The two must agree: a bare `std::env::var(..).ok()` here
/// would post to the empty string while the refresh path decided no override
/// existed and re-ran discovery, so the same variable would mean two different
/// things on the two halves of one retry.
fn graphql_url_override() -> Option<String> {
    let value = std::env::var("PMCP_RUN_GRAPHQL_URL").ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Async endpoint resolution: env var > discovery cache > **discovery** > default.
///
/// The sync predecessor (`get_graphql_url`) could not perform discovery, so it
/// degraded to `DEFAULT_GRAPHQL_URL` whenever the cache was cold — and that
/// default resolves to nothing, turning a recoverable cache miss into an opaque
/// "Failed to send GraphQL request". Every caller in this file is already async,
/// so the sync ladder was deleted rather than left as a second, weaker copy of
/// this one.
async fn resolve_graphql_url() -> String {
    if let Some(url) = graphql_url_override() {
        return url;
    }
    if let Some(url) = load_cached_config().and_then(|c| c.graphql_url) {
        return url;
    }
    if let Some(url) = discover_graphql_url().await {
        return url;
    }
    DEFAULT_GRAPHQL_URL.to_string()
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GraphQLError {
    message: String,
}

/// Response from getUploadUrls mutation
#[derive(Debug, Deserialize)]
pub struct UploadUrls {
    #[serde(rename = "templateUploadUrl")]
    pub template_upload_url: String,
    #[serde(rename = "templateS3Key")]
    pub template_s3_key: String,
    #[serde(rename = "bootstrapUploadUrl")]
    pub bootstrap_upload_url: String,
    #[serde(rename = "bootstrapS3Key")]
    pub bootstrap_s3_key: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i32, // 900 seconds (15 minutes)
}

/// Response from createDeploymentFromS3 mutation
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeploymentInfo {
    #[serde(rename = "deploymentId")]
    pub deployment_id: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Deployment status from getDeployment query
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeploymentStatus {
    pub id: String,
    pub status: String,
    pub url: Option<String>,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
}

/// Get presigned S3 upload URLs
pub async fn get_upload_urls(
    access_token: &str,
    project_name: &str,
    template_size: usize,
    bootstrap_size: usize,
) -> Result<UploadUrls> {
    let query = r#"
        mutation GetUploadUrls(
            $projectName: String!,
            $templateSize: Int!,
            $bootstrapSize: Int!
        ) {
            getUploadUrls(
                projectName: $projectName,
                templateSize: $templateSize,
                bootstrapSize: $bootstrapSize
            ) {
                templateUploadUrl
                templateS3Key
                bootstrapUploadUrl
                bootstrapS3Key
                expiresIn
            }
        }
    "#;

    let variables = serde_json::json!({
        "projectName": project_name,
        "templateSize": template_size as i64,
        "bootstrapSize": bootstrap_size as i64
    });

    #[derive(Debug, Deserialize)]
    struct GetUploadUrlsResponse {
        #[serde(rename = "getUploadUrls")]
        get_upload_urls: UploadUrls,
    }

    let response: GetUploadUrlsResponse = execute_graphql(access_token, query, variables).await?;

    Ok(response.get_upload_urls)
}

/// Upload file directly to S3 using presigned URL.
///
/// `label` is a human-readable name for the upload (e.g., "template", "bootstrap")
/// used in progress and error messages instead of exposing the presigned URL.
pub async fn upload_to_s3(
    url: &str,
    content: Vec<u8>,
    content_type: &str,
    label: &str,
) -> Result<()> {
    let content_len = content.len();
    let max_attempts: u32 = 5;

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300)) // 5 min for large binaries
        .build()
        .context("Failed to create HTTP client")?;

    for attempt in 1..=max_attempts {
        let response = client
            .put(url)
            .header("Content-Type", content_type)
            .header("Content-Length", content_len)
            .body(content.clone())
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            },
            Ok(resp) => {
                let status = resp.status();
                let error_body = resp.text().await.unwrap_or_default();
                // Extract meaningful S3 error (e.g., AccessDenied, RequestTimeout)
                let s3_error = extract_s3_error(&error_body).unwrap_or(error_body.clone());

                if attempt < max_attempts {
                    let backoff = Duration::from_secs(2u64.pow(attempt));
                    eprintln!(
                        "   Retry {}/{}: {} upload got HTTP {} ({}), retrying in {}s...",
                        attempt,
                        max_attempts,
                        label,
                        status.as_u16(),
                        s3_error,
                        backoff.as_secs()
                    );
                    tokio::time::sleep(backoff).await;
                } else {
                    bail!(
                        "{} upload failed after {} attempts: HTTP {} — {}",
                        label,
                        max_attempts,
                        status.as_u16(),
                        s3_error
                    );
                }
            },
            Err(e) => {
                let cause = describe_reqwest_error(&e);

                if attempt < max_attempts {
                    let backoff = Duration::from_secs(2u64.pow(attempt));
                    eprintln!(
                        "   Retry {}/{}: {} upload failed ({}), retrying in {}s...",
                        attempt,
                        max_attempts,
                        label,
                        cause,
                        backoff.as_secs()
                    );
                    tokio::time::sleep(backoff).await;
                } else {
                    bail!(
                        "{} upload failed after {} attempts: {}",
                        label,
                        max_attempts,
                        cause
                    );
                }
            },
        }
    }

    Ok(())
}

/// Extract a human-readable error code/message from S3 XML error responses.
fn extract_s3_error(body: &str) -> Option<String> {
    // S3 returns XML like: <Error><Code>RequestTimeout</Code><Message>...</Message></Error>
    if let Some(start) = body.find("<Code>") {
        let after = &body[start + 6..];
        if let Some(end) = after.find("</Code>") {
            return Some(after[..end].to_string());
        }
    }
    if body.trim().is_empty() {
        return None;
    }
    // Return first 200 chars if not XML
    Some(body.chars().take(200).collect())
}

/// Produce a concise description of a reqwest error without leaking the full URL.
fn describe_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "connection timed out".to_string()
    } else if e.is_connect() {
        "failed to connect to S3".to_string()
    } else if e.is_body() {
        "error sending request body".to_string()
    } else {
        // Strip the URL from the Display output to avoid leaking presigned credentials.
        // reqwest errors format as "error sending request for url (https://...): cause"
        let msg = e.to_string();
        if msg.contains("error sending request for url") {
            if let Some(end) = msg.find("): ") {
                // Keep just the cause after "): "
                return format!("network error: {}", &msg[end + 3..]);
            }
        }
        format!(
            "network error: {}",
            msg.chars().take(200).collect::<String>()
        )
    }
}

/// Deployment settings including composition and versioning
#[derive(Debug, Clone, Default)]
pub struct CompositionSettings {
    pub tier: String,
    pub allow_composition: bool,
    pub internal_only: bool,
    pub description: Option<String>,
    /// Server version from manifest (e.g., "1.2.3" from Cargo.toml)
    pub server_version: Option<String>,
}

/// Create deployment from S3 files
#[allow(dead_code)]
pub async fn create_deployment_from_s3(
    access_token: &str,
    urls: &UploadUrls,
    project_name: &str,
) -> Result<DeploymentInfo> {
    create_deployment_from_s3_with_composition(
        access_token,
        urls,
        project_name,
        CompositionSettings::default(),
    )
    .await
}

/// Create deployment from S3 files with composition settings
pub async fn create_deployment_from_s3_with_composition(
    access_token: &str,
    urls: &UploadUrls,
    project_name: &str,
    composition: CompositionSettings,
) -> Result<DeploymentInfo> {
    let query = r#"
        mutation CreateDeploymentFromS3(
            $templateS3Key: String!,
            $bootstrapS3Key: String!,
            $projectName: String!,
            $runtime: String,
            $memorySize: Int,
            $timeout: Int,
            $serverVersion: String,
            $tier: String,
            $allowComposition: Boolean,
            $internalOnly: Boolean,
            $compositionDescription: String
        ) {
            createDeploymentFromS3(
                templateS3Key: $templateS3Key,
                bootstrapS3Key: $bootstrapS3Key,
                projectName: $projectName,
                runtime: $runtime,
                memorySize: $memorySize,
                timeout: $timeout,
                serverVersion: $serverVersion,
                tier: $tier,
                allowComposition: $allowComposition,
                internalOnly: $internalOnly,
                compositionDescription: $compositionDescription
            ) {
                deploymentId
                status
                projectName
                createdAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "templateS3Key": urls.template_s3_key,
        "bootstrapS3Key": urls.bootstrap_s3_key,
        "projectName": project_name,
        "runtime": "provided.al2023",
        "memorySize": 512,
        "timeout": 30,
        "serverVersion": composition.server_version,
        "tier": composition.tier,
        "allowComposition": composition.allow_composition,
        "internalOnly": composition.internal_only,
        "compositionDescription": composition.description
    });

    #[derive(Debug, Deserialize)]
    struct CreateDeploymentResponse {
        #[serde(rename = "createDeploymentFromS3")]
        create_deployment_from_s3: Option<DeploymentInfo>,
    }

    let response: CreateDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .create_deployment_from_s3
        .context("Deployment creation returned null - check pmcp.run service logs")
}

/// Get deployment status
pub async fn get_deployment(access_token: &str, deployment_id: &str) -> Result<DeploymentStatus> {
    let query = r#"
        query GetDeployment($id: ID!) {
            getDeployment(id: $id) {
                id
                status
                url
                projectName
                errorMessage
                createdAt
                completedAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": deployment_id
    });

    #[derive(Debug, Deserialize)]
    struct GetDeploymentResponse {
        #[serde(rename = "getDeployment")]
        get_deployment: Option<DeploymentStatus>,
    }

    let response: GetDeploymentResponse = execute_graphql(access_token, query, variables).await?;

    response.get_deployment.context("Deployment not found")
}

/// Execute GraphQL query
/// Does this error look like the endpoint does not know our schema?
///
/// AppSync rejects an operation the schema lacks at VALIDATION time, before any
/// resolver runs, with `FieldUndefined` / `UnknownType` / `Validation error`. The
/// usual cause is a client bug, but it is also exactly what a STALE discovery cache
/// produces: pmcp.run federates three source APIs into one merged API, so a cached
/// URL pointing at a single source API sees only part of the schema and reports the
/// rest as undefined. Retrying elsewhere is only worth it for this error class.
fn looks_like_unknown_schema(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains("FieldUndefined")
        || msg.contains("UnknownType")
        || msg.contains("Validation error")
}

/// Execute a GraphQL request, re-running discovery once if the endpoint reports our
/// operation as undefined.
///
/// The discovery cache is keyed by `api_url`, so it does not notice when the server
/// changes which endpoint it advertises under a stable `api_url` — the entry then
/// stays wrong for up to an hour. Rather than make users wait out the TTL (or know to
/// delete a cache file), treat a schema-validation failure as evidence the cached
/// endpoint is stale, refresh, and retry ONCE — and only when the refreshed URL is
/// actually different, so a genuine client-side schema bug fails at its original
/// speed instead of paying an extra round-trip to fail identically.
async fn execute_graphql<T>(
    access_token: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    // NOT get_graphql_url(): that is sync, so with an empty cache it can only fall
    // through to DEFAULT_GRAPHQL_URL — a host that does not resolve. Deleting the
    // cache (or a first run on a fresh machine) would then fail with a transport
    // error naming no endpoint. Here we are already async, so an absent cache is
    // resolved by running discovery, which is what the sync path cannot do.
    let url = resolve_graphql_url().await;
    match execute_graphql_at(&url, access_token, query, variables.clone()).await {
        Err(e) if looks_like_unknown_schema(&e) => {
            let Some(fresh) = refresh_graphql_url().await else {
                return Err(e);
            };
            if fresh == url {
                return Err(e);
            }
            eprintln!(
                "note: endpoint did not recognize the operation; discovery now advertises \n      {fresh}\n      (cached endpoint was stale) — retrying once."
            );
            execute_graphql_at(&fresh, access_token, query, variables).await
        },
        other => other,
    }
}

/// Single GraphQL round-trip against an explicit endpoint.
async fn execute_graphql_at<T>(
    graphql_url: &str,
    access_token: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    // Process-wide client: reuses connections across calls (the capture poll
    // loop hits this every 2s — a per-call client would redo TCP+TLS each time).
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(reqwest::Client::new);

    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(graphql_url)
        .header("Authorization", access_token)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send GraphQL request")?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        bail!("GraphQL request failed: {}", error_text);
    }

    // Get raw text first for debugging
    let response_text = response.text().await.context("Failed to read response")?;

    // Try to parse as generic JSON first to check for errors
    let raw_json: serde_json::Value =
        serde_json::from_str(&response_text).context("Failed to parse response as JSON")?;

    // Check for GraphQL errors in raw response
    if let Some(errors) = raw_json.get("errors") {
        if let Some(errors_array) = errors.as_array() {
            let error_messages: Vec<String> = errors_array
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(|s| s.to_string())
                .collect();
            if !error_messages.is_empty() {
                bail!("GraphQL errors: {}", error_messages.join(", "));
            }
        }
    }

    // Now parse as the expected type
    let graphql_response: GraphQLResponse<T> = serde_json::from_str(&response_text)
        .with_context(|| format!("Failed to parse GraphQL response: {}", response_text))?;

    graphql_response
        .data
        .with_context(|| format!("No data in GraphQL response: {}", response_text))
}

/// Response from destroyDeployment mutation
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DestroyDeploymentResult {
    pub id: String,
    #[serde(rename = "stackName")]
    pub stack_name: Option<String>,
    pub status: String,
    pub message: Option<String>,
    #[serde(rename = "executionArn")]
    pub execution_arn: Option<String>,
}

/// Destroy deployment by ID (complete cleanup including CloudFormation stack)
///
/// This performs a complete cleanup:
/// - Deletes CloudFormation stack
/// - Removes OAuth configuration and Cognito User Pool
/// - Deletes McpServer registry entry
/// - Deletes Deployment DynamoDB record
///
/// Returns the operation result which may be async (initiated) or sync (completed/failed).
pub async fn destroy_deployment(
    access_token: &str,
    deployment_id: &str,
) -> Result<DestroyDeploymentResult> {
    let query = r#"
        mutation DestroyDeployment($id: ID!) {
            destroyDeployment(id: $id) {
                id
                stackName
                status
                message
                executionArn
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": deployment_id
    });

    #[derive(Debug, Deserialize)]
    struct DestroyDeploymentResponse {
        #[serde(rename = "destroyDeployment")]
        destroy_deployment: Option<DestroyDeploymentResult>,
    }

    let response: DestroyDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .destroy_deployment
        .context("Failed to destroy deployment: no response returned")
}

/// Response from getDeploymentOperationStatus query
#[derive(Debug, Clone, Deserialize)]
pub struct OperationStatusResult {
    pub id: String,
    pub status: String,
    pub message: Option<String>,
    #[serde(rename = "executionArn")]
    pub execution_arn: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// Get the status of an async deployment operation
///
/// Use this to poll for completion of long-running operations like destroy.
pub async fn get_deployment_operation_status(
    access_token: &str,
    operation_id: &str,
) -> Result<OperationStatusResult> {
    let query = r#"
        query GetDeploymentOperationStatus($id: ID!) {
            getDeploymentOperationStatus(id: $id) {
                id
                status
                message
                executionArn
                updatedAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": operation_id
    });

    #[derive(Debug, Deserialize)]
    struct GetOperationStatusResponse {
        #[serde(rename = "getDeploymentOperationStatus")]
        get_deployment_operation_status: Option<OperationStatusResult>,
    }

    let response: GetOperationStatusResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .get_deployment_operation_status
        .context("Operation not found")
}

/// Find deployment ID by project name
pub async fn find_deployment_id_by_name(access_token: &str, project_name: &str) -> Result<String> {
    let query = r#"
        query ListDeployments {
            listDeployments {
                items {
                    id
                    projectName
                }
            }
        }
    "#;

    let variables = serde_json::json!({});

    #[derive(Debug, Deserialize)]
    struct ListDeploymentsResponse {
        #[serde(rename = "listDeployments")]
        list_deployments: DeploymentList,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentList {
        items: Vec<DeploymentItem>,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentItem {
        id: String,
        #[serde(rename = "projectName")]
        project_name: String,
    }

    let response: ListDeploymentsResponse = execute_graphql(access_token, query, variables).await?;

    // Find deployment by project name
    response
        .list_deployments
        .items
        .iter()
        .find(|d| d.project_name == project_name)
        .map(|d| d.id.clone())
        .context(format!("No deployment found for project: {}", project_name))
}

/// Get deployment outputs (for outputs command)
pub async fn get_deployment_outputs(
    access_token: &str,
    project_name: &str,
) -> Result<crate::deployment::r#trait::DeploymentOutputs> {
    // Reuse get_deployment but find by project name
    let query = r#"
        query ListDeployments {
            listDeployments {
                items {
                    id
                    projectName
                    status
                    url
                }
            }
        }
    "#;

    let variables = serde_json::json!({});

    #[derive(Debug, Deserialize)]
    struct ListDeploymentsResponse {
        #[serde(rename = "listDeployments")]
        list_deployments: DeploymentList,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentList {
        items: Vec<DeploymentItem>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct DeploymentItem {
        id: String,
        #[serde(rename = "projectName")]
        project_name: String,
        status: String,
        url: Option<String>,
    }

    let response: ListDeploymentsResponse = execute_graphql(access_token, query, variables).await?;

    // Find deployment by project name
    let deployment = response
        .list_deployments
        .items
        .iter()
        .find(|d| d.project_name == project_name)
        .context(format!("No deployment found for project: {}", project_name))?;

    Ok(crate::deployment::r#trait::DeploymentOutputs {
        url: deployment.url.clone(),
        additional_urls: vec![],
        regions: vec![],
        stack_name: None,
        version: None,
        custom: std::collections::HashMap::new(),
    })
}

// ========== Landing Page Deployment GraphQL Functions ==========

/// Response from getLandingUploadUrl mutation
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LandingUploadUrl {
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
    #[serde(rename = "s3Key")]
    pub s3_key: String,
    #[serde(rename = "s3Bucket")]
    pub s3_bucket: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i32,
}

/// Response from deployLandingPage mutation
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LandingInfo {
    #[serde(rename = "landingId")]
    pub landing_id: String,
    #[serde(rename = "amplifyAppId")]
    pub amplify_app_id: String,
    #[serde(rename = "amplifyDomainUrl")]
    pub amplify_domain_url: String,
    #[serde(rename = "landingUrl")]
    pub landing_url: String, // Clean URL: https://{serverName}.{region}.true-mcp.com/landing
    pub status: String,
    #[serde(rename = "buildJobId")]
    pub build_job_id: String,
}

/// Landing page status from getLandingStatus mutation
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LandingStatus {
    pub id: String,
    #[serde(rename = "serverId")]
    pub server_id: String,
    pub status: String,
    #[serde(rename = "amplifyDomainUrl")]
    pub amplify_domain_url: Option<String>,
    #[serde(rename = "customDomain")]
    pub custom_domain: Option<String>,
    #[serde(rename = "lastDeployedAt")]
    pub last_deployed_at: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

/// Get presigned S3 upload URL for landing page zip
pub async fn get_landing_upload_url(
    access_token: &str,
    server_id: &str,
    zip_size: usize,
) -> Result<LandingUploadUrl> {
    let query = r#"
        mutation GetLandingUploadUrl(
            $serverId: String!,
            $fileSize: Int!
        ) {
            getLandingUploadUrl(
                serverId: $serverId,
                fileSize: $fileSize
            ) {
                uploadUrl
                s3Key
                s3Bucket
                expiresIn
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "fileSize": zip_size as i64
    });

    #[derive(Debug, Deserialize)]
    struct GetLandingUploadUrlResponse {
        #[serde(rename = "getLandingUploadUrl")]
        get_landing_upload_url: LandingUploadUrl,
    }

    let response: GetLandingUploadUrlResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.get_landing_upload_url)
}

/// Deploy landing page from S3 zip file
pub async fn deploy_landing_page(
    access_token: &str,
    s3_key: &str,
    server_id: &str,
    server_name: &str,
    config_json: &str,
) -> Result<LandingInfo> {
    let query = r#"
        mutation DeployLandingPage(
            $serverId: String!,
            $serverName: String!,
            $sourceS3Key: String!,
            $config: AWSJSON!
        ) {
            deployLandingPage(
                serverId: $serverId,
                serverName: $serverName,
                sourceS3Key: $sourceS3Key,
                config: $config
            ) {
                landingId
                amplifyAppId
                amplifyDomainUrl
                landingUrl
                status
                buildJobId
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "serverName": server_name,
        "sourceS3Key": s3_key,
        "config": config_json
    });

    #[derive(Debug, Deserialize)]
    struct DeployLandingResponse {
        #[serde(rename = "deployLandingPage")]
        deploy_landing_page: LandingInfo,
    }

    let response: DeployLandingResponse = execute_graphql(access_token, query, variables).await?;

    Ok(response.deploy_landing_page)
}

/// Get landing page status
/// NOTE: This is a MUTATION, not a Query! It checks Amplify job status and updates DB.
pub async fn get_landing_status(access_token: &str, landing_id: &str) -> Result<LandingStatus> {
    let query = r#"
        mutation GetLandingStatus($landingId: String!) {
            getLandingStatus(landingId: $landingId) {
                id
                serverId
                status
                amplifyDomainUrl
                customDomain
                lastDeployedAt
                errorMessage
            }
        }
    "#;

    let variables = serde_json::json!({
        "landingId": landing_id
    });

    #[derive(Debug, Deserialize)]
    struct GetLandingStatusResponse {
        #[serde(rename = "getLandingStatus")]
        get_landing_status: Option<LandingStatus>,
    }

    let response: GetLandingStatusResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .get_landing_status
        .context("Landing page not found")
}

// ========== OAuth Configuration GraphQL Functions ==========

/// OAuth configuration response from configureServerOAuth mutation
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OAuthConfig {
    #[serde(rename = "serverId")]
    pub server_id: String,
    #[serde(rename = "oauthEnabled")]
    pub oauth_enabled: bool,
    #[serde(rename = "userPoolId")]
    pub user_pool_id: Option<String>,
    #[serde(rename = "userPoolRegion")]
    pub user_pool_region: Option<String>,
    #[serde(rename = "discoveryUrl")]
    pub discovery_url: Option<String>,
    #[serde(rename = "registrationEndpoint")]
    pub registration_endpoint: Option<String>,
    #[serde(rename = "authorizationEndpoint")]
    pub authorization_endpoint: Option<String>,
    #[serde(rename = "tokenEndpoint")]
    pub token_endpoint: Option<String>,
}

/// OAuth endpoints response from fetchServerOAuthEndpoints query
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OAuthEndpoints {
    #[serde(rename = "serverId")]
    pub server_id: String,
    #[serde(rename = "oauthEnabled")]
    pub oauth_enabled: bool,
    pub provider: Option<String>,
    #[serde(rename = "userPoolId")]
    pub user_pool_id: Option<String>,
    #[serde(rename = "userPoolRegion")]
    pub user_pool_region: Option<String>,
    pub scopes: Option<Vec<String>>,
    #[serde(rename = "dcrEnabled")]
    pub dcr_enabled: Option<bool>,
    #[serde(rename = "discoveryUrl")]
    pub discovery_url: Option<String>,
    #[serde(rename = "registrationEndpoint")]
    pub registration_endpoint: Option<String>,
    #[serde(rename = "authorizationEndpoint")]
    pub authorization_endpoint: Option<String>,
    #[serde(rename = "tokenEndpoint")]
    pub token_endpoint: Option<String>,
}

/// Configure OAuth for an MCP server
///
/// This creates a Cognito User Pool if one doesn't exist and configures
/// the API Gateway routes with the shared authorizer Lambda.
pub async fn configure_server_oauth(
    access_token: &str,
    server_id: &str,
    enabled: bool,
    scopes: Option<Vec<String>>,
    dcr_enabled: Option<bool>,
    public_client_patterns: Option<Vec<String>>,
    shared_pool_name: Option<String>,
) -> Result<OAuthConfig> {
    let query = r#"
        mutation ConfigureServerOAuth(
            $serverId: String!
            $enabled: Boolean!
            $scopes: [String]
            $dcrEnabled: Boolean
            $publicClientPatterns: [String]
            $sharedPoolName: String
        ) {
            configureServerOAuth(
                serverId: $serverId
                enabled: $enabled
                scopes: $scopes
                dcrEnabled: $dcrEnabled
                publicClientPatterns: $publicClientPatterns
                sharedPoolName: $sharedPoolName
            ) {
                serverId
                oauthEnabled
                userPoolId
                userPoolRegion
                discoveryUrl
                registrationEndpoint
                authorizationEndpoint
                tokenEndpoint
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "enabled": enabled,
        "scopes": scopes,
        "dcrEnabled": dcr_enabled,
        "publicClientPatterns": public_client_patterns,
        "sharedPoolName": shared_pool_name
    });

    #[derive(Debug, Deserialize)]
    struct ConfigureServerOAuthResponse {
        #[serde(rename = "configureServerOAuth")]
        configure_server_oauth: OAuthConfig,
    }

    let response: ConfigureServerOAuthResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.configure_server_oauth)
}

/// Disable OAuth for an MCP server
pub async fn disable_server_oauth(access_token: &str, server_id: &str) -> Result<()> {
    let query = r#"
        mutation DisableServerOAuth($serverId: String!) {
            disableServerOAuth(serverId: $serverId) {
                serverId
                oauthEnabled
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id
    });

    #[derive(Debug, Deserialize)]
    struct DisableServerOAuthResponse {
        #[serde(rename = "disableServerOAuth")]
        disable_server_oauth: DisableResult,
    }

    #[derive(Debug, Deserialize)]
    struct DisableResult {
        #[serde(rename = "serverId")]
        _server_id: String,
        #[serde(rename = "oauthEnabled")]
        oauth_enabled: bool,
    }

    let response: DisableServerOAuthResponse =
        execute_graphql(access_token, query, variables).await?;

    if response.disable_server_oauth.oauth_enabled {
        bail!("Failed to disable OAuth - server still reports OAuth enabled");
    }

    Ok(())
}

/// Fetch OAuth endpoints for an MCP server
pub async fn fetch_server_oauth_endpoints(
    access_token: &str,
    server_id: &str,
) -> Result<OAuthEndpoints> {
    let query = r#"
        query FetchServerOAuthEndpoints($serverId: String!) {
            fetchServerOAuthEndpoints(serverId: $serverId) {
                serverId
                oauthEnabled
                provider
                userPoolId
                userPoolRegion
                scopes
                dcrEnabled
                discoveryUrl
                registrationEndpoint
                authorizationEndpoint
                tokenEndpoint
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id
    });

    #[derive(Debug, Deserialize)]
    struct FetchServerOAuthEndpointsResponse {
        #[serde(rename = "fetchServerOAuthEndpoints")]
        fetch_server_oauth_endpoints: Option<OAuthEndpoints>,
    }

    let response: FetchServerOAuthEndpointsResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .fetch_server_oauth_endpoints
        .context("OAuth not configured for this server")
}

// ========== Test Scenario Management GraphQL Functions ==========

/// Response from uploadTestScenario mutation
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UploadScenarioResult {
    #[serde(rename = "scenarioId")]
    pub scenario_id: String,
    pub version: i32,
}

/// Response from downloadTestScenario query
#[derive(Debug, Deserialize)]
pub struct DownloadScenarioResult {
    pub name: String,
    pub content: String,
    pub version: i32,
}

/// Scenario info from queryTestScenariosForServer query
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ScenarioInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub enabled: bool,
    pub version: i32,
    #[serde(rename = "lastExecutedAt")]
    pub last_executed_at: Option<String>,
    #[serde(rename = "lastExecutionStatus")]
    pub last_execution_status: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// Response from queryTestScenariosForServer query
#[derive(Debug, Deserialize)]
pub struct ListScenariosResult {
    pub scenarios: Vec<ScenarioInfo>,
}

/// Upload a test scenario to pmcp.run
pub async fn upload_test_scenario(
    access_token: &str,
    server_id: &str,
    name: &str,
    description: Option<&str>,
    content: &str,
    format: &str,
) -> Result<UploadScenarioResult> {
    let query = r#"
        mutation UploadTestScenario(
            $serverId: String!
            $name: String!
            $description: String
            $content: String!
            $format: String
        ) {
            uploadTestScenario(
                serverId: $serverId
                name: $name
                description: $description
                content: $content
                format: $format
            ) {
                scenarioId
                version
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "name": name,
        "description": description,
        "content": content,
        "format": format.to_lowercase()
    });

    #[derive(Debug, Deserialize)]
    struct UploadTestScenarioResponse {
        #[serde(rename = "uploadTestScenario")]
        upload_test_scenario: UploadScenarioResult,
    }

    let response: UploadTestScenarioResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.upload_test_scenario)
}

/// Download a test scenario from pmcp.run
pub async fn download_test_scenario(
    access_token: &str,
    scenario_id: &str,
    format: &str,
) -> Result<DownloadScenarioResult> {
    let query = r#"
        query DownloadTestScenario(
            $scenarioId: String!
            $format: String
        ) {
            downloadTestScenario(
                scenarioId: $scenarioId
                format: $format
            ) {
                name
                content
                version
            }
        }
    "#;

    let variables = serde_json::json!({
        "scenarioId": scenario_id,
        "format": format.to_lowercase()
    });

    #[derive(Debug, Deserialize)]
    struct DownloadTestScenarioResponse {
        #[serde(rename = "downloadTestScenario")]
        download_test_scenario: DownloadScenarioResult,
    }

    let response: DownloadTestScenarioResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.download_test_scenario)
}

/// List test scenarios for an MCP server on pmcp.run
pub async fn list_test_scenarios(
    access_token: &str,
    server_id: &str,
) -> Result<ListScenariosResult> {
    let query = r#"
        query QueryTestScenariosForServer($serverId: String!) {
            queryTestScenariosForServer(serverId: $serverId) {
                scenarios
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id
    });

    #[derive(Debug, Deserialize)]
    struct QueryTestScenariosResponse {
        #[serde(rename = "queryTestScenariosForServer")]
        query_test_scenarios: ListScenariosRaw,
    }

    #[derive(Debug, Deserialize)]
    struct ListScenariosRaw {
        scenarios: serde_json::Value,
    }

    let response: QueryTestScenariosResponse =
        execute_graphql(access_token, query, variables).await?;

    // Parse the JSON scenarios array
    let scenarios: Vec<ScenarioInfo> =
        serde_json::from_value(response.query_test_scenarios.scenarios)
            .context("Failed to parse scenarios list")?;

    Ok(ListScenariosResult { scenarios })
}

// ========== Loadtest Scenario Upload GraphQL Functions ==========

/// Upload a loadtest scenario to pmcp.run
pub async fn upload_loadtest_scenario(
    access_token: &str,
    server_id: &str,
    name: &str,
    description: Option<&str>,
    content: &str,
) -> Result<UploadScenarioResult> {
    let query = r#"
        mutation UploadLoadTestScenario(
            $serverId: String!
            $name: String!
            $description: String
            $content: String!
        ) {
            uploadLoadTestScenario(
                serverId: $serverId
                name: $name
                description: $description
                content: $content
            ) {
                scenarioId
                version
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "name": name,
        "description": description,
        "content": content,
    });

    #[derive(Debug, Deserialize)]
    struct UploadLoadTestScenarioResponse {
        #[serde(rename = "uploadLoadTestScenario")]
        upload_loadtest_scenario: UploadScenarioResult,
    }

    let response: UploadLoadTestScenarioResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.upload_loadtest_scenario)
}

// ============================================================================
// Package capture service (170-08 D-A/D-B/D-D) — the `cargo pmcp package
// capture|show` remote thin client. Mirrors the create_deployment_from_s3 /
// get_deployment async-job idiom above for a second job type.
// ============================================================================

/// Response from `submitPackageCapture` mutation (170-08 D-A).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CaptureInfo {
    #[serde(rename = "captureId")]
    pub capture_id: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Status from `getPackageCaptureStatus` query — the STRUCTURED D-B error
/// contract (170-02's landed platform contract). The caller switches on
/// `error_code` and reads `divergent_components` directly; `message` is
/// display-only and MUST NEVER be parsed to detect `BUMP_REQUIRED` or any
/// other error condition.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CaptureStatus {
    pub id: String,
    pub status: String,
    pub message: Option<String>,
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "divergentComponents")]
    pub divergent_components: Option<Vec<String>>,
    #[serde(rename = "manifestDigest")]
    pub manifest_digest: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// Response from `getWorkflowPackageManifest` query — a published `WorkflowManifest`
/// looked up by `name`+`version` (org-scoped server-side by the caller's own
/// claim; the CLI never supplies an org id).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WorkflowPackageResp {
    pub name: String,
    pub version: String,
    #[serde(rename = "manifestJson")]
    pub manifest_json: String,
    #[serde(rename = "manifestDigest")]
    pub manifest_digest: String,
}

// The two runtime queries below live in `graphql_contract.rs` — a
// dependency-light leaf that's also mounted directly into the `cargo-pmcp`
// lib target (via `#[path]` in `lib.rs`) so the offline blocking contract
// test (`tests/package_capture_contract.rs`) can validate them against the
// vendored SDL without pulling this file's `reqwest`/auth/deploy tree into
// the lib target. Re-exported here so the rest of this file (and any other
// `pmcp_run` code) can keep referring to them unqualified.
pub(crate) use super::graphql_contract::{
    GET_PACKAGE_CAPTURE_STATUS_QUERY, SUBMIT_PACKAGE_CAPTURE_QUERY,
};

/// Submit an async package-capture job for a team's workflow dependency graph
/// (170-08 D-A). `root_type` is always `"team"` in v1; `root_id` is the
/// `AgentTeam` UUID. Never awaits the walk — returns the queued job id.
pub async fn submit_package_capture(
    access_token: &str,
    root_type: &str,
    root_id: &str,
    version: &str,
    bump: Option<&str>,
) -> Result<CaptureInfo> {
    let query = SUBMIT_PACKAGE_CAPTURE_QUERY;

    let variables = serde_json::json!({
        "rootComponentType": root_type,
        "rootComponentId": root_id,
        "version": version,
        "bump": bump,
    });

    #[derive(Debug, Deserialize)]
    struct SubmitPackageCaptureResponse {
        #[serde(rename = "submitPackageCapture")]
        submit_package_capture: Option<CaptureInfo>,
    }

    let response: SubmitPackageCaptureResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .submit_package_capture
        .context("submitPackageCapture returned null - check pmcp.run service logs")
}

/// Poll a package-capture job's status once (170-08 D-A/D-B). The caller is
/// responsible for looping/sleeping between calls — this fn does a single
/// consistent-read-backed fetch.
pub async fn get_package_capture_status(
    access_token: &str,
    capture_id: &str,
) -> Result<CaptureStatus> {
    let query = GET_PACKAGE_CAPTURE_STATUS_QUERY;

    let variables = serde_json::json!({
        "id": capture_id
    });

    #[derive(Debug, Deserialize)]
    struct GetPackageCaptureStatusResponse {
        #[serde(rename = "getPackageCaptureStatus")]
        get_package_capture_status: Option<CaptureStatus>,
    }

    let response: GetPackageCaptureStatusResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .get_package_capture_status
        .context("Capture job not found")
}

/// Fetch a published workflow manifest by `name`+`version` (170-08 D-D
/// `package show`) — a plain org-scoped DDB read server-side, NOT ECR.
pub async fn get_workflow_package(
    access_token: &str,
    name: &str,
    version: &str,
) -> Result<WorkflowPackageResp> {
    let query = r#"
        query GetWorkflowPackage($name: String!, $version: String!) {
            getWorkflowPackageManifest(name: $name, version: $version) {
                name
                version
                manifestJson
                manifestDigest
            }
        }
    "#;

    let variables = serde_json::json!({
        "name": name,
        "version": version,
    });

    #[derive(Debug, Deserialize)]
    struct GetWorkflowPackageResponse {
        #[serde(rename = "getWorkflowPackageManifest")]
        get_workflow_package: Option<WorkflowPackageResp>,
    }

    let response: GetWorkflowPackageResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .get_workflow_package
        .with_context(|| format!("WorkflowPackage not found: {name}@{version}"))
}

// ============================================================================
// AI-Package import control plane (Phase 171 D-02/D-04/D-05) — the
// `cargo pmcp package import|approve` remote thin client. Mirrors the
// capture/show idiom above for a second async job type (`ImportJob`) plus
// one-shot governance mutations. Reference-based ONLY: the caller supplies a
// workflow `name@version` reference (or an `organizationId`+`name`+`version`
// triple for the governance mutations) — NEVER a payload/OCI digest (Codex
// #1 / T-171-25b). The server resolves both digests server-side.
// ============================================================================

/// Response from `submitImport` mutation (D-04). `status` is always
/// `"queued"` on success.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ImportInfo {
    #[serde(rename = "importId")]
    pub import_id: String,
    pub status: String,
}

/// Status from `getImportStatus` query. `preflight_report_json` carries the
/// full Codex #7 disposition/deviation/allowlist/impact report once a
/// terminal status is reached (`completed_dry_run` / `blocked` /
/// `awaiting_bind`). `error_code` (NEVER `error_message`) is the only field
/// the CLI switches on for a `"failed"` terminal status (T-171-26).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ImportStatus {
    pub status: String,
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "preflightReportJson")]
    pub preflight_report_json: Option<String>,
}

/// Response from the `approvePackage` mutation (D-06/D-08).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ApprovalInfo {
    pub id: String,
    #[serde(rename = "approvedAt")]
    pub approved_at: String,
}

/// Response from the `revokeApprovedPackage` mutation (D-08 — append-only;
/// the server flips a `revoked` flag rather than deleting the row).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RevocationInfo {
    pub id: String,
    #[serde(rename = "revokedAt")]
    pub revoked_at: String,
}

/// Response from the `setPackageBinding` mutation (D-09/D-12). `status` is
/// always `"binding"` on success — the job resumes asynchronously; the
/// caller must poll `getImportStatus` to observe the real outcome.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BindingInfo {
    #[serde(rename = "importId")]
    pub import_id: String,
    pub status: String,
}

/// Submit an async dry-run pre-flight import job for a workflow REFERENCE
/// (D-02/D-04). ALWAYS sends `dryRun: true` — the server rejects any submit
/// where `dryRun` is not explicitly `true` (171-07); there is no client-side
/// way to request real execution this phase (deferred to Phase 172).
pub async fn submit_package_import(access_token: &str, reference: &str) -> Result<ImportInfo> {
    let query = r#"
        mutation SubmitImport($reference: String!, $dryRun: Boolean) {
            submitImport(reference: $reference, dryRun: $dryRun) {
                importId
                status
            }
        }
    "#;

    let variables = serde_json::json!({
        "reference": reference,
        "dryRun": true,
    });

    #[derive(Debug, Deserialize)]
    struct SubmitImportResponse {
        #[serde(rename = "submitImport")]
        submit_import: Option<ImportInfo>,
    }

    let response: SubmitImportResponse = execute_graphql(access_token, query, variables).await?;

    response
        .submit_import
        .context("submitImport returned null - check pmcp.run service logs")
}

/// Poll an import job's status once (D-04). The caller is responsible for
/// looping/sleeping between calls — this fn does a single
/// consistent-read-backed fetch.
pub async fn get_import_status(access_token: &str, import_id: &str) -> Result<ImportStatus> {
    let query = r#"
        query GetImportStatus($importId: String!) {
            getImportStatus(importId: $importId) {
                status
                errorCode
                errorMessage
                preflightReportJson
            }
        }
    "#;

    let variables = serde_json::json!({
        "importId": import_id,
    });

    #[derive(Debug, Deserialize)]
    struct GetImportStatusResponse {
        #[serde(rename = "getImportStatus")]
        get_import_status: Option<ImportStatus>,
    }

    let response: GetImportStatusResponse = execute_graphql(access_token, query, variables).await?;

    response.get_import_status.context("Import job not found")
}

/// Approve a workflow package by REFERENCE only (D-05/D-06/D-08) — the
/// server resolves BOTH digests from the source `WorkflowPackage` row; the
/// CLI never derives or sends a digest (Codex #1 / T-171-25b). `evidence` is
/// an optional freeform link/note (D-07).
pub async fn approve_package(
    access_token: &str,
    organization_id: &str,
    workflow_name: &str,
    workflow_version: &str,
    evidence: Option<&str>,
) -> Result<ApprovalInfo> {
    let query = r#"
        mutation ApprovePackage(
            $organizationId: String!,
            $workflowName: String!,
            $workflowVersion: String!,
            $evidence: String
        ) {
            approvePackage(
                organizationId: $organizationId,
                workflowName: $workflowName,
                workflowVersion: $workflowVersion,
                evidence: $evidence
            ) {
                id
                approvedAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "organizationId": organization_id,
        "workflowName": workflow_name,
        "workflowVersion": workflow_version,
        "evidence": evidence,
    });

    #[derive(Debug, Deserialize)]
    struct ApprovePackageResponse {
        #[serde(rename = "approvePackage")]
        approve_package: Option<ApprovalInfo>,
    }

    let response: ApprovePackageResponse = execute_graphql(access_token, query, variables).await?;

    response
        .approve_package
        .context("approvePackage returned null - check pmcp.run service logs")
}

/// Revoke a previously-approved workflow package by REFERENCE only (D-08).
/// Not yet wired to a CLI verb (Plan 09 exposes `import`/`approve`, not
/// `revoke`) — provided now so a future CLI verb or admin UI never needs a
/// second, divergent client implementation of this mutation.
#[allow(dead_code)]
pub async fn revoke_approved_package(
    access_token: &str,
    organization_id: &str,
    workflow_name: &str,
    workflow_version: &str,
) -> Result<RevocationInfo> {
    let query = r#"
        mutation RevokeApprovedPackage(
            $organizationId: String!,
            $workflowName: String!,
            $workflowVersion: String!
        ) {
            revokeApprovedPackage(
                organizationId: $organizationId,
                workflowName: $workflowName,
                workflowVersion: $workflowVersion
            ) {
                id
                revokedAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "organizationId": organization_id,
        "workflowName": workflow_name,
        "workflowVersion": workflow_version,
    });

    #[derive(Debug, Deserialize)]
    struct RevokeApprovedPackageResponse {
        #[serde(rename = "revokeApprovedPackage")]
        revoke_approved_package: Option<RevocationInfo>,
    }

    let response: RevokeApprovedPackageResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .revoke_approved_package
        .context("revokeApprovedPackage returned null - check pmcp.run service logs")
}

/// Stage a binding (+optional per-deviation acknowledgments) onto an import
/// job and resume it (D-09/D-12). `status` is always `"binding"` on success —
/// bindings are NOT visible immediately; poll `getImportStatus` for the real
/// outcome (`completed_dry_run` / `blocked` / another `awaiting_bind`). Not
/// yet wired to a CLI verb this phase — provided for the same forward-looking
/// reason as `revoke_approved_package` above.
#[allow(dead_code)]
pub async fn set_package_binding(
    access_token: &str,
    import_id: &str,
    organization_id: &str,
    component_name: &str,
    slot_kind: &str,
    slot_name: &str,
    bound_value: &str,
    acknowledgments: Option<serde_json::Value>,
) -> Result<BindingInfo> {
    let query = r#"
        mutation SetPackageBinding(
            $importId: String!,
            $organizationId: String!,
            $componentName: String!,
            $slotKind: String!,
            $slotName: String!,
            $boundValue: String!,
            $acknowledgments: AWSJSON
        ) {
            setPackageBinding(
                importId: $importId,
                organizationId: $organizationId,
                componentName: $componentName,
                slotKind: $slotKind,
                slotName: $slotName,
                boundValue: $boundValue,
                acknowledgments: $acknowledgments
            ) {
                importId
                status
            }
        }
    "#;

    let variables = serde_json::json!({
        "importId": import_id,
        "organizationId": organization_id,
        "componentName": component_name,
        "slotKind": slot_kind,
        "slotName": slot_name,
        "boundValue": bound_value,
        "acknowledgments": acknowledgments,
    });

    #[derive(Debug, Deserialize)]
    struct SetPackageBindingResponse {
        #[serde(rename = "setPackageBinding")]
        set_package_binding: Option<BindingInfo>,
    }

    let response: SetPackageBindingResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .set_package_binding
        .context("setPackageBinding returned null - check pmcp.run service logs")
}

#[cfg(test)]
mod tests {
    use super::looks_like_unknown_schema;

    /// Every GraphQL variable declared by this file must use a type the pmcp.run
    /// schema actually defines. All of these operations take scalars only, so the
    /// check reduces to "is it a built-in GraphQL or AppSync scalar".
    ///
    /// Why this exists (2026-08-15): `uploadTestScenario` and `downloadTestScenario`
    /// each declared `$format: UploadTestScenarioFormat` / `DownloadTestScenarioFormat`
    /// — types that have never existed on ANY pmcp.run endpoint (the server takes a
    /// plain `format: String`, and both call sites were already sending a lowercased
    /// string). AppSync rejects the whole document at validation time with
    /// "Unknown type UploadTestScenarioFormat", so both commands failed against every
    /// server on the platform. The failure reads like a missing feature rather than a
    /// client bug, which is what made it expensive to diagnose.
    /// Only a schema-validation failure should trigger the discovery re-fetch and
    /// retry. Matching too broadly would double the latency of every unrelated
    /// failure; matching too narrowly leaves the stale-cache case unrecovered.
    #[test]
    fn unknown_schema_detection_matches_only_validation_failures() {
        let stale = [
            "GraphQL errors: Validation error of type FieldUndefined: Field \'uploadTestScenario\' in type \'Mutation\' is undefined",
            "GraphQL errors: Validation error of type UnknownType: Unknown type UploadTestScenarioFormat",
        ];
        for m in stale {
            assert!(
                looks_like_unknown_schema(&anyhow::anyhow!(m.to_string())),
                "should retry after refreshing discovery: {m}"
            );
        }

        let unrelated = [
            "Failed to send GraphQL request",
            "GraphQL errors: Not Authorized to access uploadTestScenario on type Mutation",
            "GraphQL request failed: 502 Bad Gateway",
        ];
        for m in unrelated {
            assert!(
                !looks_like_unknown_schema(&anyhow::anyhow!(m.to_string())),
                "must NOT pay an extra discovery round-trip for: {m}"
            );
        }
    }

    #[test]
    fn declared_variable_types_are_known_scalars() {
        const SOURCE: &str = include_str!("graphql.rs");
        // GraphQL built-ins plus the AppSync-specific scalars this API uses.
        const KNOWN: &[&str] = &[
            "String",
            "Boolean",
            "Int",
            "Float",
            "ID",
            "AWSJSON",
            "AWSDate",
            "AWSTime",
            "AWSDateTime",
            "AWSTimestamp",
            "AWSEmail",
            "AWSURL",
            "AWSPhone",
            "AWSIPAddress",
        ];

        let mut offenders: Vec<(usize, String)> = Vec::new();
        for (i, line) in SOURCE.lines().enumerate() {
            // Match a variable declaration inside a query string: `$name: Type` / `Type!`.
            let Some(colon) = line.find(": ") else {
                continue;
            };
            if !line.trim_start().starts_with('$') {
                continue;
            }
            // Normalize: drop a trailing comma (GraphQL allows comma-separated
            // variable lists) and the non-null / list punctuation, leaving the
            // bare named type — `[String!]!` and `String` both reduce to `String`.
            let ty: String = line[colon + 2..]
                .trim()
                .trim_end_matches(',')
                .chars()
                .filter(|c| !matches!(c, '!' | '[' | ']'))
                .collect();
            let ty = ty.trim();
            if ty.is_empty() || ty.contains(' ') {
                continue;
            }
            if !KNOWN.contains(&ty) {
                offenders.push((i + 1, ty.to_string()));
            }
        }

        assert!(
            offenders.is_empty(),
            "GraphQL variables declared with a type pmcp.run does not define \
             (AppSync rejects the whole document): {offenders:?}"
        );
    }
}
