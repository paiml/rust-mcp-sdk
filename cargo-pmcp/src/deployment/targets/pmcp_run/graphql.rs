use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Production default for GraphQL API
const DEFAULT_GRAPHQL_URL: &str = "https://api.pmcp.run/graphql";

// GraphQL URL - reads from environment variable or uses default
fn get_graphql_url() -> String {
    std::env::var("PMCP_RUN_GRAPHQL_URL").unwrap_or_else(|_| DEFAULT_GRAPHQL_URL.to_string())
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
async fn execute_graphql<T>(
    access_token: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::new();
    let graphql_url = get_graphql_url();

    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(&graphql_url)
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
            $format: UploadTestScenarioFormat
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
            $format: DownloadTestScenarioFormat
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
