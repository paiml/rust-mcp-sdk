//! Secret provider trait and related types.

use async_trait::async_trait;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use super::error::SecretResult;
use super::value::{SecretEntry, SecretMetadata, SecretValue};

/// Which Lambda the secret is intended for.
///
/// pmcp.run servers that opt into Stripe billing have two Lambdas: the MCP
/// server Lambda (handles MCP protocol traffic) and a subscription Lambda
/// (handles Stripe webhook events). Each Lambda has its own pool of
/// environment-variable secrets.
///
/// `Mcp` (default) targets the MCP server Lambda. `Billing` targets the
/// subscription Lambda; only the `pmcp-run` target supports it — other
/// providers reject `Billing` because they have no subscription-Lambda concept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum Audience {
    /// MCP server Lambda (default).
    #[default]
    Mcp,
    /// Subscription Lambda for Stripe billing (pmcp-run only).
    Billing,
}

impl Audience {
    /// GraphQL enum variant string for the audience argument. Note: the platform
    /// generates a DISTINCT enum type per operation (`ListServerSecretsAudience`,
    /// `GetServerSecretAudience`, `SetServerSecretAudience`, `DeleteServerSecretAudience`)
    /// rather than a single shared `ServerSecretAudience` type — see the per-query
    /// `$audience` variable declarations in `providers/pmcp_run.rs`.
    pub fn as_graphql(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Billing => "billing",
        }
    }
}

impl std::fmt::Display for Audience {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_graphql())
    }
}

/// Capabilities supported by a secret provider.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProviderCapabilities {
    /// Whether the provider supports versioning
    pub versioning: bool,
    /// Whether the provider supports tags
    pub tags: bool,
    /// Whether the provider supports descriptions
    pub descriptions: bool,
    /// Whether the provider supports binary values
    pub binary_values: bool,
    /// Maximum value size in bytes
    pub max_value_size: usize,
    /// Whether the provider supports hierarchical names (e.g., "api/openai/key")
    pub hierarchical_names: bool,
}

/// Health status of a provider.
#[derive(Debug, Clone)]
pub struct ProviderHealth {
    /// Whether the provider is available and authenticated
    pub available: bool,
    /// Authentication method used
    pub auth_method: Option<String>,
    /// Additional status information
    pub message: Option<String>,
    /// User or account identifier (if available)
    pub user: Option<String>,
}

impl ProviderHealth {
    /// Create a healthy status.
    pub fn healthy(auth_method: impl Into<String>) -> Self {
        Self {
            available: true,
            auth_method: Some(auth_method.into()),
            message: None,
            user: None,
        }
    }

    /// Create a healthy status with user info.
    pub fn healthy_with_user(auth_method: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            available: true,
            auth_method: Some(auth_method.into()),
            message: None,
            user: Some(user.into()),
        }
    }

    /// Create an unavailable status.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            auth_method: None,
            message: Some(message.into()),
            user: None,
        }
    }
}

/// Options for listing secrets.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Filter by name pattern (glob syntax)
    pub filter: Option<String>,
    /// Filter by server ID
    pub server_id: Option<String>,
    /// Include metadata in results
    pub include_metadata: bool,
}

/// Result of a list operation.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ListResult {
    /// List of secrets (values hidden)
    pub secrets: Vec<SecretEntry>,
    /// Total count (if different from secrets.len())
    pub total_count: Option<usize>,
}

/// Options for setting a secret.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SetOptions {
    /// Description for the secret
    pub description: Option<String>,
    /// Tags to attach to the secret
    pub tags: std::collections::HashMap<String, String>,
    /// Fail if secret already exists
    pub no_overwrite: bool,
    /// Server ID for namespacing
    pub server_id: Option<String>,
}

/// Trait for secret storage providers.
///
/// Implementations handle the actual storage and retrieval of secrets
/// from various backends (local files, pmcp.run, AWS Secrets Manager, etc.).
#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "local", "pmcp", "aws").
    fn id(&self) -> &str;

    /// Human-readable name for this provider.
    fn name(&self) -> &str;

    /// Get the capabilities of this provider.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Validate a secret name for this provider.
    ///
    /// Different providers have different naming restrictions.
    fn validate_name(&self, name: &str) -> SecretResult<()>;

    /// List secrets (values are never returned, only names/metadata).
    async fn list(&self, audience: Audience, options: ListOptions) -> SecretResult<ListResult>;

    /// Get a secret value by name.
    ///
    /// The name should include the server prefix (e.g., "chess/ANTHROPIC_API_KEY").
    async fn get(&self, audience: Audience, name: &str) -> SecretResult<SecretValue>;

    /// Set a secret value.
    ///
    /// Returns metadata about the created/updated secret. The metadata may
    /// include a non-fatal `warning` (e.g., billing audience targeted but no
    /// subscription Lambda registered yet) — the caller should display it
    /// without flipping the exit code.
    async fn set(
        &self,
        audience: Audience,
        name: &str,
        value: SecretValue,
        options: SetOptions,
    ) -> SecretResult<SecretMetadata>;

    /// Delete a secret.
    async fn delete(&self, audience: Audience, name: &str, force: bool) -> SecretResult<()>;

    /// Check the health/availability of this provider.
    async fn health_check(&self) -> SecretResult<ProviderHealth>;
}

/// Reject `Audience::Billing` for providers without a subscription-Lambda concept.
///
/// Local filesystem and AWS Secrets Manager targets deploy a single Lambda;
/// they have no separate billing pool. Returning a clear error here is better
/// than silently storing a secret that nothing will read.
pub fn reject_billing_audience(provider: &str, audience: Audience) -> SecretResult<()> {
    if audience == Audience::Billing {
        Err(super::error::SecretError::ProviderError {
            provider: provider.to_string(),
            message: format!(
                "--audience billing is only supported when --target=pmcp-run; \
                 the {provider} target does not expose a subscription Lambda concept."
            ),
        })
    } else {
        Ok(())
    }
}

/// Parse a fully-qualified secret name into (server_id, secret_name).
///
/// Format: `server-id/SECRET_NAME`
///
/// # Examples
/// ```ignore
/// let (server, name) = parse_secret_name("chess/ANTHROPIC_API_KEY");
/// assert_eq!(server, "chess");
/// assert_eq!(name, "ANTHROPIC_API_KEY");
/// ```
pub fn parse_secret_name(full_name: &str) -> SecretResult<(String, String)> {
    match full_name.split_once('/') {
        Some((server_id, secret_name)) if !server_id.is_empty() && !secret_name.is_empty() => {
            Ok((server_id.to_string(), secret_name.to_string()))
        },
        _ => Err(super::error::SecretError::InvalidName {
            name: full_name.to_string(),
            reason: "Secret name must be in format 'server-id/SECRET_NAME'".to_string(),
        }),
    }
}

/// Create a fully-qualified secret name from server ID and secret name.
#[allow(dead_code)]
pub fn make_secret_name(server_id: &str, secret_name: &str) -> String {
    format!("{}/{}", server_id, secret_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_secret_name_valid() {
        let (server, name) = parse_secret_name("chess/ANTHROPIC_API_KEY").unwrap();
        assert_eq!(server, "chess");
        assert_eq!(name, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_parse_secret_name_nested() {
        let (server, name) = parse_secret_name("my-server/api/key").unwrap();
        assert_eq!(server, "my-server");
        assert_eq!(name, "api/key");
    }

    #[test]
    fn test_parse_secret_name_invalid_no_slash() {
        let result = parse_secret_name("ANTHROPIC_API_KEY");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_secret_name_invalid_empty_server() {
        let result = parse_secret_name("/ANTHROPIC_API_KEY");
        assert!(result.is_err());
    }

    #[test]
    fn test_make_secret_name() {
        let name = make_secret_name("chess", "ANTHROPIC_API_KEY");
        assert_eq!(name, "chess/ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_audience_default_is_mcp() {
        assert_eq!(Audience::default(), Audience::Mcp);
    }

    #[test]
    fn test_audience_graphql_strings() {
        assert_eq!(Audience::Mcp.as_graphql(), "mcp");
        assert_eq!(Audience::Billing.as_graphql(), "billing");
    }

    #[test]
    fn test_audience_display() {
        assert_eq!(Audience::Mcp.to_string(), "mcp");
        assert_eq!(Audience::Billing.to_string(), "billing");
    }

    #[test]
    fn test_reject_billing_audience_passes_mcp() {
        assert!(reject_billing_audience("local", Audience::Mcp).is_ok());
    }

    #[test]
    fn test_reject_billing_audience_rejects_billing() {
        let result = reject_billing_audience("local", Audience::Billing);
        match result {
            Err(super::super::error::SecretError::ProviderError { provider, message }) => {
                assert_eq!(provider, "local");
                assert!(message.contains("--audience billing"));
                assert!(message.contains("pmcp-run"));
            },
            other => panic!("expected ProviderError, got {other:?}"),
        }
    }
}
