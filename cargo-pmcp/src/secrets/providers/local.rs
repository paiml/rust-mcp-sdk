//! Local filesystem secret provider for development.
//!
//! Stores secrets in `.pmcp/secrets/{server-id}/` with file permissions set to 0600.
//! Automatically manages `.gitignore` to prevent accidental commits.

use async_trait::async_trait;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::secrets::error::{SecretError, SecretResult};
use crate::secrets::provider::{
    parse_secret_name, ListOptions, ListResult, ProviderCapabilities, ProviderHealth,
    SecretProvider, SetOptions,
};
use crate::secrets::value::{SecretEntry, SecretMetadata, SecretValue};

/// Local filesystem secret provider.
///
/// Stores secrets as individual files in a directory structure:
/// ```text
/// .pmcp/secrets/
/// ├── .gitignore          # Contains "*" to ignore all
/// ├── server-1/
/// │   └── API_KEY
/// └── server-2/
///     └── DATABASE_URL
/// ```
pub struct LocalSecretProvider {
    secrets_dir: PathBuf,
}

impl LocalSecretProvider {
    /// Create a new local secret provider.
    pub fn new(secrets_dir: PathBuf) -> Self {
        Self { secrets_dir }
    }

    /// Ensure the secrets directory exists with proper permissions.
    fn ensure_dir(&self) -> SecretResult<()> {
        if !self.secrets_dir.exists() {
            fs::create_dir_all(&self.secrets_dir)?;

            // Set directory permissions to 0700
            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&self.secrets_dir)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&self.secrets_dir, perms)?;
            }

            // Create .gitignore
            self.ensure_gitignore()?;
        }
        Ok(())
    }

    /// Ensure .gitignore exists in secrets directory.
    fn ensure_gitignore(&self) -> SecretResult<()> {
        let gitignore_path = self.secrets_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "*\n")?;
        }
        Ok(())
    }

    /// Get the path to a secret file.
    fn secret_path(&self, server_id: &str, secret_name: &str) -> PathBuf {
        self.secrets_dir.join(server_id).join(secret_name)
    }

    /// Ensure server directory exists.
    fn ensure_server_dir(&self, server_id: &str) -> SecretResult<PathBuf> {
        let server_dir = self.secrets_dir.join(server_id);
        if !server_dir.exists() {
            fs::create_dir_all(&server_dir)?;

            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&server_dir)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&server_dir, perms)?;
            }
        }
        Ok(server_dir)
    }
}

#[async_trait]
impl SecretProvider for LocalSecretProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn name(&self) -> &str {
        "Local Filesystem"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            versioning: false,
            tags: false,
            descriptions: false,
            binary_values: true,
            max_value_size: 1024 * 1024, // 1MB
            hierarchical_names: true,
        }
    }

    fn validate_name(&self, name: &str) -> SecretResult<()> {
        // Parse to validate format
        let (server_id, secret_name) = parse_secret_name(name)?;

        // Check for invalid characters in filenames
        let invalid_chars = ['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'];
        for ch in invalid_chars {
            if server_id.contains(ch) {
                return Err(SecretError::InvalidName {
                    name: name.to_string(),
                    reason: format!("Server ID contains invalid character: '{}'", ch),
                });
            }
            // Secret name can contain '/' for nested paths
            if ch != '/' && secret_name.contains(ch) {
                return Err(SecretError::InvalidName {
                    name: name.to_string(),
                    reason: format!("Secret name contains invalid character: '{}'", ch),
                });
            }
        }

        Ok(())
    }

    async fn list(&self, options: ListOptions) -> SecretResult<ListResult> {
        self.ensure_dir()?;

        let mut secrets = Vec::new();

        // List all server directories
        if !self.secrets_dir.exists() {
            return Ok(ListResult::default());
        }

        for entry in fs::read_dir(&self.secrets_dir)? {
            let entry = entry?;
            let server_dir = entry.path();

            // Skip non-directories and .gitignore
            if !server_dir.is_dir() {
                continue;
            }

            let server_id = entry.file_name().to_string_lossy().to_string();

            // Filter by server_id if specified
            if let Some(ref filter_server) = options.server_id {
                if &server_id != filter_server {
                    continue;
                }
            }

            // List secrets in this server directory
            for secret_entry in fs::read_dir(&server_dir)? {
                let secret_entry = secret_entry?;
                let secret_path = secret_entry.path();

                if !secret_path.is_file() {
                    continue;
                }

                let secret_name = secret_entry.file_name().to_string_lossy().to_string();
                let full_name = format!("{}/{}", server_id, secret_name);

                // Apply name filter
                if let Some(ref pattern) = options.filter {
                    if !glob_match(pattern, &full_name) {
                        continue;
                    }
                }

                let metadata = if options.include_metadata {
                    let file_meta = fs::metadata(&secret_path)?;
                    SecretMetadata {
                        name: secret_name.clone(),
                        version: None,
                        created_at: file_meta.created().ok().map(|t| format!("{:?}", t)),
                        modified_at: file_meta.modified().ok().map(|t| format!("{:?}", t)),
                        description: None,
                        tags: Default::default(),
                    }
                } else {
                    SecretMetadata::new(&secret_name)
                };

                secrets.push(SecretEntry {
                    name: full_name,
                    metadata,
                });
            }
        }

        // Sort by name for consistent output
        secrets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(ListResult {
            secrets,
            total_count: None,
        })
    }

    async fn get(&self, name: &str) -> SecretResult<SecretValue> {
        self.validate_name(name)?;
        let (server_id, secret_name) = parse_secret_name(name)?;

        let path = self.secret_path(&server_id, &secret_name);

        if !path.exists() {
            return Err(SecretError::NotFound {
                name: name.to_string(),
            });
        }

        let value = fs::read_to_string(&path)?;
        Ok(SecretValue::new(value))
    }

    async fn set(
        &self,
        name: &str,
        value: SecretValue,
        options: SetOptions,
    ) -> SecretResult<SecretMetadata> {
        self.validate_name(name)?;
        self.ensure_dir()?;

        let (server_id, secret_name) = parse_secret_name(name)?;
        self.ensure_server_dir(&server_id)?;

        let path = self.secret_path(&server_id, &secret_name);

        // Check for existing secret if no_overwrite is set
        if options.no_overwrite && path.exists() {
            return Err(SecretError::AlreadyExists {
                name: name.to_string(),
            });
        }

        // Create parent directories if needed (for nested secret names)
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the secret value
        fs::write(&path, value.expose())?;

        // Set file permissions to 0600
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(SecretMetadata {
            name: secret_name,
            version: Some(1),
            created_at: None,
            modified_at: Some(chrono::Utc::now().to_rfc3339()),
            description: options.description,
            tags: options.tags,
        })
    }

    async fn delete(&self, name: &str, _force: bool) -> SecretResult<()> {
        self.validate_name(name)?;
        let (server_id, secret_name) = parse_secret_name(name)?;

        let path = self.secret_path(&server_id, &secret_name);

        if !path.exists() {
            return Err(SecretError::NotFound {
                name: name.to_string(),
            });
        }

        fs::remove_file(&path)?;

        // Clean up empty server directory
        let server_dir = self.secrets_dir.join(&server_id);
        if server_dir.exists() && fs::read_dir(&server_dir)?.next().is_none() {
            fs::remove_dir(&server_dir)?;
        }

        Ok(())
    }

    async fn health_check(&self) -> SecretResult<ProviderHealth> {
        // Local provider is always available
        // Check if we can write to the secrets directory
        match self.ensure_dir() {
            Ok(_) => Ok(ProviderHealth::healthy("filesystem")),
            Err(e) => Ok(ProviderHealth::unavailable(format!(
                "Cannot access secrets directory: {}",
                e
            ))),
        }
    }
}

/// Simple glob pattern matching.
fn glob_match(pattern: &str, text: &str) -> bool {
    // Simple implementation: just support * as wildcard
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return text.starts_with(prefix);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return text.ends_with(suffix);
    }

    pattern == text
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_provider_set_get() {
        let temp_dir = TempDir::new().unwrap();
        let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

        // Set a secret
        let result = provider
            .set(
                "test-server/API_KEY",
                SecretValue::new("secret-value".to_string()),
                SetOptions::default(),
            )
            .await;
        assert!(result.is_ok());

        // Get the secret
        let value = provider.get("test-server/API_KEY").await.unwrap();
        assert_eq!(value.expose(), "secret-value");
    }

    #[tokio::test]
    async fn test_local_provider_list() {
        let temp_dir = TempDir::new().unwrap();
        let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

        // Set some secrets
        provider
            .set(
                "server-a/KEY1",
                SecretValue::new("value1".to_string()),
                SetOptions::default(),
            )
            .await
            .unwrap();
        provider
            .set(
                "server-a/KEY2",
                SecretValue::new("value2".to_string()),
                SetOptions::default(),
            )
            .await
            .unwrap();
        provider
            .set(
                "server-b/KEY3",
                SecretValue::new("value3".to_string()),
                SetOptions::default(),
            )
            .await
            .unwrap();

        // List all secrets
        let result = provider.list(ListOptions::default()).await.unwrap();
        assert_eq!(result.secrets.len(), 3);

        // List for specific server
        let result = provider
            .list(ListOptions {
                server_id: Some("server-a".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(result.secrets.len(), 2);
    }

    #[tokio::test]
    async fn test_local_provider_delete() {
        let temp_dir = TempDir::new().unwrap();
        let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

        // Set a secret
        provider
            .set(
                "test-server/DELETE_ME",
                SecretValue::new("to-delete".to_string()),
                SetOptions::default(),
            )
            .await
            .unwrap();

        // Delete it
        provider
            .delete("test-server/DELETE_ME", false)
            .await
            .unwrap();

        // Verify it's gone
        let result = provider.get("test-server/DELETE_ME").await;
        assert!(matches!(result, Err(SecretError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_local_provider_no_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

        // Set a secret
        provider
            .set(
                "test-server/EXISTING",
                SecretValue::new("original".to_string()),
                SetOptions::default(),
            )
            .await
            .unwrap();

        // Try to set with no_overwrite
        let result = provider
            .set(
                "test-server/EXISTING",
                SecretValue::new("new-value".to_string()),
                SetOptions {
                    no_overwrite: true,
                    ..Default::default()
                },
            )
            .await;
        assert!(matches!(result, Err(SecretError::AlreadyExists { .. })));

        // Original value should remain
        let value = provider.get("test-server/EXISTING").await.unwrap();
        assert_eq!(value.expose(), "original");
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("server-*", "server-a"));
        assert!(glob_match("*KEY", "API_KEY"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("server-*", "other-a"));
    }
}
