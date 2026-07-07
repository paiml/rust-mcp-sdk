//! pmcp.run secret provider using GraphQL API.
//!
//! Enterprise-grade secret management with organization-level shared secrets.
//! Secrets are namespaced by server ID and stored securely in AWS Secrets Manager
//! via the pmcp.run platform.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::secrets::error::{SecretError, SecretResult};
use crate::secrets::provider::{
    parse_secret_name, Audience, ListOptions, ListResult, ProviderCapabilities, ProviderHealth,
    SecretProvider, SetOptions,
};
use crate::secrets::value::{SecretEntry, SecretMetadata, SecretValue};

use crate::deployment::targets::pmcp_run::auth::{load_cached_config, DEFAULT_GRAPHQL_URL};

/// pmcp.run secret provider.
///
/// Uses GraphQL API to manage secrets at the server level.
/// Secrets are namespaced by server ID: `{server-id}/{SECRET_NAME}`.
/// The organization is derived server-side from the server record.
pub struct PmcpRunSecretProvider {
    api_url: String,
}

impl PmcpRunSecretProvider {
    /// Create a new pmcp.run secret provider.
    ///
    /// Note: Organization ID is not required - the backend derives it from the server ID.
    pub fn new(api_url: Option<String>) -> Self {
        Self {
            api_url: api_url.unwrap_or_else(|| {
                // Priority: env var > discovery cache > default
                std::env::var("PMCP_RUN_GRAPHQL_URL")
                    .ok()
                    .or_else(|| load_cached_config().and_then(|c| c.graphql_url))
                    .unwrap_or_else(|| DEFAULT_GRAPHQL_URL.to_string())
            }),
        }
    }

    /// Get the access token for authentication.
    async fn get_access_token(&self) -> SecretResult<String> {
        // Try to get from environment first
        if let Ok(token) = std::env::var("PMCP_ACCESS_TOKEN") {
            return Ok(token);
        }

        // Try to load from stored credentials
        use crate::deployment::targets::pmcp_run::auth;
        let credentials =
            auth::get_credentials()
                .await
                .map_err(|e| SecretError::AuthenticationFailed {
                    provider: "pmcp".to_string(),
                    message: e.to_string(),
                })?;

        Ok(credentials.access_token)
    }

    /// Execute a GraphQL query.
    async fn execute_graphql<T>(&self, query: &str, variables: serde_json::Value) -> SecretResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let access_token = self.get_access_token().await?;
        let client = reqwest::Client::new();

        #[derive(Serialize)]
        struct GraphQLRequest {
            query: String,
            variables: serde_json::Value,
        }

        #[derive(Deserialize)]
        struct GraphQLResponse<D> {
            data: Option<D>,
            errors: Option<Vec<GraphQLError>>,
        }

        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }

        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        // Verbose logging for debugging
        if std::env::var("PMCP_VERBOSE").is_ok() {
            eprintln!("[VERBOSE] GraphQL request to: {}", self.api_url);
            eprintln!(
                "[VERBOSE] GraphQL variables: {}",
                serde_json::to_string(&request.variables).unwrap_or_default()
            );
        }

        let response = client
            .post(&self.api_url)
            .header("Authorization", &access_token)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| SecretError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: format!("GraphQL request failed: {}", error_text),
            });
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| SecretError::NetworkError(e.to_string()))?;

        // Verbose logging for debugging
        if std::env::var("PMCP_VERBOSE").is_ok() {
            eprintln!("[VERBOSE] GraphQL response: {}", response_text);
        }

        let graphql_response: GraphQLResponse<T> =
            serde_json::from_str(&response_text).map_err(|e| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: format!("Failed to parse response: {}. Raw: {}", e, response_text),
            })?;

        if let Some(errors) = graphql_response.errors {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: messages.join(", "),
            });
        }

        graphql_response
            .data
            .ok_or_else(|| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: "No data in GraphQL response".to_string(),
            })
    }
}

#[async_trait]
impl SecretProvider for PmcpRunSecretProvider {
    fn id(&self) -> &str {
        "pmcp"
    }

    fn name(&self) -> &str {
        "pmcp.run"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            versioning: true,
            tags: true,
            descriptions: true,
            binary_values: false,
            max_value_size: 64 * 1024, // 64KB
            hierarchical_names: true,
        }
    }

    fn validate_name(&self, name: &str) -> SecretResult<()> {
        // Parse to validate format
        let (server_id, secret_name) = parse_secret_name(name)?;

        // pmcp.run naming pattern: ^[a-zA-Z0-9][a-zA-Z0-9_\-/]*$
        let valid_chars = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/';

        if !server_id.starts_with(|c: char| c.is_ascii_alphanumeric()) {
            return Err(SecretError::InvalidName {
                name: name.to_string(),
                reason: "Server ID must start with an alphanumeric character".to_string(),
            });
        }

        if !server_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SecretError::InvalidName {
                name: name.to_string(),
                reason:
                    "Server ID can only contain alphanumeric characters, underscores, and hyphens"
                        .to_string(),
            });
        }

        if !secret_name.chars().all(valid_chars) {
            return Err(SecretError::InvalidName {
                name: name.to_string(),
                reason: "Secret name can only contain alphanumeric characters, underscores, hyphens, and slashes".to_string(),
            });
        }

        Ok(())
    }

    async fn list(&self, audience: Audience, options: ListOptions) -> SecretResult<ListResult> {
        // Server ID is required for listing secrets
        let server_id = options.server_id.ok_or_else(|| {
            SecretError::ConfigError(
                "Server ID is required to list secrets. Use --server <server-id>".to_string(),
            )
        })?;

        let query = r#"
            query ListServerSecrets($serverId: String!, $audience: ListServerSecretsAudience) {
                listServerSecrets(serverId: $serverId, audience: $audience) {
                    serverId
                    secrets
                }
            }
        "#;

        let variables = serde_json::json!({
            "serverId": server_id,
            "audience": audience.as_graphql(),
        });

        #[derive(Deserialize)]
        struct ListResponse {
            #[serde(rename = "listServerSecrets")]
            list_server_secrets: Option<SecretsData>,
        }

        #[derive(Deserialize)]
        struct SecretsData {
            #[serde(rename = "serverId")]
            server_id: String,
            // Backend returns secrets as a JSON string, not an array
            secrets: String,
        }

        #[derive(Deserialize)]
        struct SecretInfo {
            name: String,
            #[serde(rename = "hasValue")]
            #[allow(dead_code)]
            has_value: bool,
        }

        let response: ListResponse = self.execute_graphql(query, variables).await?;

        // Handle null response (auth error)
        let result = response
            .list_server_secrets
            .ok_or_else(|| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: "No response from server. Check authentication.".to_string(),
            })?;

        // Parse the secrets JSON string into an array
        let secret_infos: Vec<SecretInfo> =
            serde_json::from_str(&result.secrets).map_err(|e| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: format!("Failed to parse secrets list: {}", e),
            })?;

        let returned_server_id = result.server_id;
        let secrets = secret_infos
            .into_iter()
            .filter(|s| {
                // Apply filter if provided
                match &options.filter {
                    Some(filter) => s.name.contains(filter),
                    None => true,
                }
            })
            .map(|s| SecretEntry {
                name: format!("{}/{}", returned_server_id, s.name),
                metadata: SecretMetadata {
                    name: s.name,
                    version: None,
                    created_at: None,
                    modified_at: None,
                    description: None,
                    tags: Default::default(),
                    warning: None,
                },
            })
            .collect();

        Ok(ListResult {
            secrets,
            total_count: None,
        })
    }

    async fn get(&self, audience: Audience, name: &str) -> SecretResult<SecretValue> {
        self.validate_name(name)?;
        let (server_id, secret_name) = parse_secret_name(name)?;

        let query = r#"
            query GetServerSecret($serverId: String!, $secretName: String!, $audience: GetServerSecretAudience) {
                getServerSecret(serverId: $serverId, secretName: $secretName, audience: $audience) {
                    serverId
                    secretName
                    secretValue
                    exists
                }
            }
        "#;

        let variables = serde_json::json!({
            "serverId": server_id,
            "secretName": secret_name,
            "audience": audience.as_graphql(),
        });

        #[derive(Deserialize)]
        struct GetResponse {
            #[serde(rename = "getServerSecret")]
            get_server_secret: Option<SecretData>,
        }

        #[derive(Deserialize)]
        struct SecretData {
            #[serde(rename = "secretValue")]
            secret_value: Option<String>,
            exists: bool,
        }

        let response: GetResponse = self.execute_graphql(query, variables).await?;

        // Handle null response (auth error)
        let result = response
            .get_server_secret
            .ok_or_else(|| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: "No response from server. Check authentication.".to_string(),
            })?;

        if !result.exists {
            return Err(SecretError::NotFound {
                name: name.to_string(),
            });
        }

        match result.secret_value {
            Some(value) => Ok(SecretValue::new(value)),
            None => Err(SecretError::NotFound {
                name: name.to_string(),
            }),
        }
    }

    async fn set(
        &self,
        audience: Audience,
        name: &str,
        value: SecretValue,
        options: SetOptions,
    ) -> SecretResult<SecretMetadata> {
        self.validate_name(name)?;
        let (server_id, secret_name) = parse_secret_name(name)?;

        let query = r#"
            mutation SetServerSecret($serverId: String!, $secretName: String!, $secretValue: String!, $description: String, $audience: SetServerSecretAudience) {
                setServerSecret(serverId: $serverId, secretName: $secretName, secretValue: $secretValue, description: $description, audience: $audience) {
                    success
                    serverId
                    secretName
                    error
                    warning
                }
            }
        "#;

        let variables = serde_json::json!({
            "serverId": server_id,
            "secretName": secret_name,
            "secretValue": value.expose(),
            "description": options.description,
            "audience": audience.as_graphql(),
        });

        #[derive(Deserialize)]
        struct SetResponse {
            #[serde(rename = "setServerSecret")]
            set_server_secret: Option<SetResult>,
        }

        #[derive(Deserialize)]
        struct SetResult {
            success: bool,
            #[serde(rename = "serverId")]
            #[allow(dead_code)]
            server_id: Option<String>,
            #[serde(rename = "secretName")]
            secret_name: Option<String>,
            error: Option<String>,
            warning: Option<String>,
        }

        let response: SetResponse = self.execute_graphql(query, variables).await?;

        // Handle Case 3: null response (auth/GraphQL error already handled by execute_graphql)
        let result = response
            .set_server_secret
            .ok_or_else(|| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: "No response from server. Check authentication.".to_string(),
            })?;

        // Handle Case 2: Application error
        if !result.success {
            return Err(SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: result
                    .error
                    .unwrap_or_else(|| "Failed to set secret".to_string()),
            });
        }

        // Case 1: Success — `warning` carries the platform's non-fatal hint
        // (e.g. "billing audience targeted but no subscription Lambda registered").
        // Per CR Q2 the caller displays it on stderr without flipping the exit code.
        Ok(SecretMetadata {
            name: result
                .secret_name
                .unwrap_or_else(|| secret_name.to_string()),
            version: None,
            created_at: None,
            modified_at: None,
            description: options.description,
            tags: options.tags,
            warning: result.warning,
        })
    }

    async fn delete(&self, audience: Audience, name: &str, _force: bool) -> SecretResult<()> {
        self.validate_name(name)?;
        let (server_id, secret_name) = parse_secret_name(name)?;

        let query = r#"
            mutation DeleteServerSecret($serverId: String!, $secretName: String!, $audience: DeleteServerSecretAudience) {
                deleteServerSecret(serverId: $serverId, secretName: $secretName, audience: $audience) {
                    success
                    error
                }
            }
        "#;

        let variables = serde_json::json!({
            "serverId": server_id,
            "secretName": secret_name,
            "audience": audience.as_graphql(),
        });

        #[derive(Deserialize)]
        struct DeleteResponse {
            #[serde(rename = "deleteServerSecret")]
            delete_server_secret: Option<DeleteResult>,
        }

        #[derive(Deserialize)]
        struct DeleteResult {
            success: bool,
            error: Option<String>,
        }

        let response: DeleteResponse = self.execute_graphql(query, variables).await?;

        // Handle null response (auth error)
        let result = response
            .delete_server_secret
            .ok_or_else(|| SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: "No response from server. Check authentication.".to_string(),
            })?;

        if !result.success {
            return Err(SecretError::ProviderError {
                provider: "pmcp".to_string(),
                message: result
                    .error
                    .unwrap_or_else(|| format!("Failed to delete secret '{}'", name)),
            });
        }

        Ok(())
    }

    async fn health_check(&self) -> SecretResult<ProviderHealth> {
        // Try to get access token - that's all we need for pmcp.run
        // Organization is derived server-side from the server ID
        match self.get_access_token().await {
            Ok(_) => Ok(ProviderHealth::healthy("OAuth")),
            Err(e) => Ok(ProviderHealth::unavailable(format!(
                "Not authenticated: {}. Run: cargo pmcp login",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_valid() {
        let provider = PmcpRunSecretProvider::new(None);

        assert!(provider.validate_name("chess/ANTHROPIC_API_KEY").is_ok());
        assert!(provider.validate_name("my-server/api/openai/key").is_ok());
        assert!(provider.validate_name("server_1/DATABASE_URL").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        let provider = PmcpRunSecretProvider::new(None);

        // Missing slash
        assert!(provider.validate_name("just-a-name").is_err());

        // Invalid characters
        assert!(provider.validate_name("server/key with spaces").is_err());

        // Empty parts
        assert!(provider.validate_name("/SECRET").is_err());
    }
}
