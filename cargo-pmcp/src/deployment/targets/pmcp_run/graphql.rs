use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// GraphQL URL - reads from environment variable or uses default
fn get_graphql_url() -> String {
    std::env::var("PMCP_RUN_GRAPHQL_URL").unwrap_or_else(|_| {
        "https://noet4bfxcfdptmhw6tmirhtycm.appsync-api.us-west-2.amazonaws.com/graphql".to_string()
    })
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
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
pub struct DeploymentInfo {
    #[serde(rename = "deploymentId")]
    pub deployment_id: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Deployment status from getDeployment query
#[derive(Debug, Deserialize)]
pub struct DeploymentStatus {
    pub id: String,
    pub status: String,
    pub url: Option<String>,
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

/// Upload file directly to S3 using presigned URL
pub async fn upload_to_s3(url: &str, content: Vec<u8>, content_type: &str) -> Result<()> {
    let client = reqwest::Client::new();

    // Retry with exponential backoff for network failures
    for attempt in 1..=3 {
        let response = client
            .put(url)
            .header("Content-Type", content_type)
            .body(content.clone())
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            },
            Ok(resp) => {
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();

                if attempt < 3 {
                    eprintln!(
                        "⚠️  S3 upload failed (attempt {}/3): {} - {}. Retrying...",
                        attempt, status, error_text
                    );
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                } else {
                    bail!(
                        "S3 upload failed after 3 attempts: {} - {}",
                        status,
                        error_text
                    );
                }
            },
            Err(e) if attempt < 3 => {
                eprintln!(
                    "⚠️  S3 upload failed (attempt {}/3): {}. Retrying...",
                    attempt, e
                );
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            },
            Err(e) => {
                bail!("S3 upload failed after 3 attempts: {}", e);
            },
        }
    }

    Ok(())
}

/// Create deployment from S3 files
pub async fn create_deployment_from_s3(
    access_token: &str,
    urls: &UploadUrls,
    project_name: &str,
) -> Result<DeploymentInfo> {
    let query = r#"
        mutation CreateDeploymentFromS3(
            $templateS3Key: String!,
            $bootstrapS3Key: String!,
            $projectName: String!,
            $runtime: String,
            $memorySize: Int,
            $timeout: Int
        ) {
            createDeploymentFromS3(
                templateS3Key: $templateS3Key,
                bootstrapS3Key: $bootstrapS3Key,
                projectName: $projectName,
                runtime: $runtime,
                memorySize: $memorySize,
                timeout: $timeout
            ) {
                deploymentId
                status
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
        "timeout": 30
    });

    #[derive(Debug, Deserialize)]
    struct CreateDeploymentResponse {
        #[serde(rename = "createDeploymentFromS3")]
        create_deployment_from_s3: DeploymentInfo,
    }

    let response: CreateDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.create_deployment_from_s3)
}

/// Get deployment status
pub async fn get_deployment(access_token: &str, deployment_id: &str) -> Result<DeploymentStatus> {
    let query = r#"
        query GetDeployment($id: ID!) {
            getDeployment(id: $id) {
                id
                status
                url
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

    let graphql_response: GraphQLResponse<T> = response
        .json()
        .await
        .context("Failed to parse GraphQL response")?;

    if let Some(errors) = graphql_response.errors {
        let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        bail!("GraphQL errors: {}", error_messages.join(", "));
    }

    graphql_response.data.context("No data in GraphQL response")
}

/// Delete deployment
pub async fn delete_deployment(access_token: &str, project_name: &str) -> Result<()> {
    let query = r#"
        mutation DeleteDeployment($projectName: String!) {
            deleteDeployment(projectName: $projectName) {
                success
                message
            }
        }
    "#;

    let variables = serde_json::json!({
        "projectName": project_name
    });

    #[derive(Debug, Deserialize)]
    struct DeleteDeploymentResponse {
        #[serde(rename = "deleteDeployment")]
        delete_deployment: DeleteResult,
    }

    #[derive(Debug, Deserialize)]
    struct DeleteResult {
        success: bool,
        message: Option<String>,
    }

    let response: DeleteDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    if !response.delete_deployment.success {
        bail!(
            "Failed to delete deployment: {}",
            response.delete_deployment.message.unwrap_or_default()
        );
    }

    Ok(())
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
pub struct LandingInfo {
    #[serde(rename = "landingId")]
    pub landing_id: String,
    #[serde(rename = "amplifyAppId")]
    pub amplify_app_id: String,
    #[serde(rename = "amplifyDomainUrl")]
    pub amplify_domain_url: String,
    pub status: String,
    #[serde(rename = "buildJobId")]
    pub build_job_id: String,
}

/// Landing page status from getLandingStatus mutation
#[derive(Debug, Deserialize)]
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

    let response: GetLandingStatusResponse = execute_graphql(access_token, query, variables).await?;

    response.get_landing_status.context("Landing page not found")
}
