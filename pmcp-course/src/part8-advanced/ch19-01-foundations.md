# Foundation Servers

Foundation servers provide core capabilities that domain servers build upon. They embody the DRY principle—write common functionality once, use it everywhere.

## What Makes a Good Foundation?

Foundation servers should be:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Foundation Server Characteristics                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ✓ STABLE: APIs rarely change (breaking changes affect all domains)     │
│  ✓ GENERIC: No business-specific logic                                  │
│  ✓ COMPOSABLE: Easy to combine with other foundations                   │
│  ✓ WELL-TESTED: Heavily tested since bugs affect everyone               │
│  ✓ DOCUMENTED: Clear contracts for domain developers                    │
│                                                                         │
│  Good Foundation Candidates:       Bad Foundation Candidates:           │
│  ═══════════════════════════       ═══════════════════════════          │
│  • Authentication/Authorization    • Business rules                     │
│  • Database connectivity           • Domain calculations                │
│  • File system access              • Department-specific logic          │
│  • HTTP client operations          • UI/presentation code               │
│  • Caching infrastructure          • Company-specific policies          │
│  • Logging and metrics                                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Common Foundation Patterns

### 1. Authentication Foundation

Most enterprise servers need authentication. Build it once:

```rust
use pmcp::{Result, Server};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// User information returned by authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub department: String,
}

/// Authentication foundation providing user validation and info retrieval
pub struct AuthFoundation {
    // In production: connection to identity provider (Okta, Auth0, etc.)
    user_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, AuthenticatedUser>>>,
}

impl AuthFoundation {
    pub fn new() -> Self {
        Self {
            user_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Validate a token and return user information
    pub async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser> {
        // In production: validate JWT, check with IdP, etc.
        // This is the SINGLE place where token validation logic lives

        if token.starts_with("valid_") {
            let user_id = token.strip_prefix("valid_").unwrap_or("unknown");
            Ok(AuthenticatedUser {
                id: user_id.to_string(),
                email: format!("{}@company.com", user_id),
                roles: vec!["employee".to_string()],
                department: "engineering".to_string(),
            })
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "Invalid authentication token",
            ))
        }
    }

    /// Check if user has required role
    pub fn has_role(&self, user: &AuthenticatedUser, required_role: &str) -> bool {
        user.roles.iter().any(|r| r == required_role || r == "admin")
    }

    /// Create middleware that validates tokens on every request
    pub fn create_middleware(&self) -> AuthMiddleware {
        AuthMiddleware {
            foundation: self.clone(),
        }
    }
}

impl Clone for AuthFoundation {
    fn clone(&self) -> Self {
        Self {
            user_cache: self.user_cache.clone(),
        }
    }
}

/// Middleware that validates auth tokens on requests
pub struct AuthMiddleware {
    foundation: AuthFoundation,
}
```

### 2. Database Foundation

Centralize database access patterns:

```rust
use async_trait::async_trait;
use pmcp::server::dynamic_resources::{DynamicResourceProvider, RequestContext, UriParams};
use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};

/// Database foundation providing query capabilities
pub struct DatabaseFoundation {
    connection_string: String,
    // In production: connection pool (sqlx, diesel, etc.)
}

impl DatabaseFoundation {
    pub fn new(connection_string: impl Into<String>) -> Self {
        Self {
            connection_string: connection_string.into(),
        }
    }

    /// Execute a read-only query
    pub async fn query(&self, sql: &str, params: &[&str]) -> Result<Vec<serde_json::Value>> {
        // Single place for:
        // - Query validation
        // - SQL injection prevention
        // - Connection pooling
        // - Query logging
        // - Performance metrics

        tracing::info!(sql = %sql, "Executing query");

        // Simulated response
        Ok(vec![serde_json::json!({
            "id": 1,
            "result": "sample data"
        })])
    }

    /// Create a dynamic resource provider for database tables
    pub fn create_table_provider(&self, allowed_tables: Vec<String>) -> TableResourceProvider {
        TableResourceProvider {
            foundation: self.clone(),
            allowed_tables,
        }
    }
}

impl Clone for DatabaseFoundation {
    fn clone(&self) -> Self {
        Self {
            connection_string: self.connection_string.clone(),
        }
    }
}

/// Dynamic resource provider for database tables
///
/// Provides resources like:
/// - tables://{table}/schema - Table schema
/// - tables://{table}/sample - Sample rows
/// - tables://{table}/count - Row count
pub struct TableResourceProvider {
    foundation: DatabaseFoundation,
    allowed_tables: Vec<String>,
}

#[async_trait]
impl DynamicResourceProvider for TableResourceProvider {
    fn templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                uri_template: "tables://{table}/schema".to_string(),
                name: "Table Schema".to_string(),
                Some("Schema definition for a database table".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "tables://{table}/sample".to_string(),
                name: "Sample Rows".to_string(),
                Some("Sample rows from the table (first 10)".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "tables://{table}/count".to_string(),
                name: "Row Count".to_string(),
                Some("Number of rows in the table".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    async fn fetch(
        &self,
        uri: &str,
        params: UriParams,
        _context: RequestContext,
    ) -> Result<ReadResourceResult> {
        let table = params.get("table").ok_or_else(|| {
            pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Missing table name")
        })?;

        // Validate table is in allowed list (security!)
        if !self.allowed_tables.contains(&table.to_string()) {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("Table '{}' not accessible", table),
            ));
        }

        let content = if uri.contains("/schema") {
            let schema = self.foundation
                .query(
                    "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = $1",
                    &[table],
                )
                .await?;
            Content::Text {
                text: serde_json::to_string_pretty(&schema)?,
            }
        } else if uri.contains("/sample") {
            let sample = self.foundation
                .query(&format!("SELECT * FROM {} LIMIT 10", table), &[])
                .await?;
            Content::Text {
                text: serde_json::to_string_pretty(&sample)?,
            }
        } else if uri.contains("/count") {
            let count = self.foundation
                .query(&format!("SELECT COUNT(*) as count FROM {}", table), &[])
                .await?;
            Content::Text {
                text: serde_json::to_string_pretty(&count)?,
            }
        } else {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "Unknown resource type",
            ));
        };

        Ok(ReadResourceResult::new(vec![content]))
    }

    fn priority(&self) -> i32 {
        50
    }
}
```

### 3. File System Foundation

Secure, audited file access:

```rust
use std::path::{Path, PathBuf};

/// File system foundation with security controls
pub struct FileSystemFoundation {
    base_path: PathBuf,
    allowed_extensions: Vec<String>,
    max_file_size: usize,
}

impl FileSystemFoundation {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            allowed_extensions: vec![
                "txt".to_string(),
                "json".to_string(),
                "csv".to_string(),
                "md".to_string(),
            ],
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }

    /// Safely resolve a path, preventing directory traversal attacks
    fn safe_path(&self, relative_path: &str) -> Result<PathBuf> {
        let path = self.base_path.join(relative_path);
        let canonical = path.canonicalize().map_err(|_| {
            pmcp::Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "Path not found")
        })?;

        // Prevent directory traversal (../../../etc/passwd)
        if !canonical.starts_with(&self.base_path) {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "Path traversal not allowed",
            ));
        }

        // Check extension
        if let Some(ext) = canonical.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if !self.allowed_extensions.contains(&ext_str) {
                return Err(pmcp::Error::protocol(
                    pmcp::ErrorCode::INVALID_PARAMS,
                    format!("File extension '{}' not allowed", ext_str),
                ));
            }
        }

        Ok(canonical)
    }

    /// Read a file with security checks
    pub async fn read_file(&self, relative_path: &str) -> Result<String> {
        let path = self.safe_path(relative_path)?;

        // Check file size
        let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
            pmcp::Error::protocol(pmcp::ErrorCode::INTERNAL_ERROR, e.to_string())
        })?;

        if metadata.len() > self.max_file_size as u64 {
            return Err(pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("File exceeds maximum size of {} bytes", self.max_file_size),
            ));
        }

        // Audit log
        tracing::info!(path = %path.display(), "File read access");

        tokio::fs::read_to_string(&path).await.map_err(|e| {
            pmcp::Error::protocol(pmcp::ErrorCode::INTERNAL_ERROR, e.to_string())
        })
    }

    /// List files in a directory
    pub async fn list_files(&self, relative_path: &str) -> Result<Vec<String>> {
        let path = self.safe_path(relative_path)?;

        let mut entries = tokio::fs::read_dir(&path).await.map_err(|e| {
            pmcp::Error::protocol(pmcp::ErrorCode::INTERNAL_ERROR, e.to_string())
        })?;

        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            pmcp::Error::protocol(pmcp::ErrorCode::INTERNAL_ERROR, e.to_string())
        })? {
            files.push(entry.file_name().to_string_lossy().to_string());
        }

        Ok(files)
    }
}
```

## Composing Foundations

Domain servers compose multiple foundations:

```rust
use pmcp::Server;
use std::sync::Arc;

/// A domain server that composes multiple foundations
pub async fn create_finance_server(
    auth: Arc<AuthFoundation>,
    db: Arc<DatabaseFoundation>,
    fs: Arc<FileSystemFoundation>,
) -> Result<Server> {
    // Create typed tools that use foundations
    let auth_clone = auth.clone();
    let db_clone = db.clone();

    Server::builder()
        .name("finance-server")
        .version("1.0.0")
        // Tool using auth + database foundations
        .tool_typed("get_expense_report", move |input: ExpenseReportInput, extra| {
            let auth = auth_clone.clone();
            let db = db_clone.clone();
            Box::pin(async move {
                // Use auth foundation
                let user = auth.validate_token(&input.token).await?;

                // Check permissions
                if !auth.has_role(&user, "finance_viewer") {
                    return Err(pmcp::Error::protocol(
                        pmcp::ErrorCode::INVALID_PARAMS,
                        "Insufficient permissions",
                    ));
                }

                // Use database foundation
                let expenses = db.query(
                    "SELECT * FROM expenses WHERE user_id = $1 AND month = $2",
                    &[&user.id, &input.month],
                ).await?;

                Ok(serde_json::json!({
                    "user": user.email,
                    "month": input.month,
                    "expenses": expenses
                }))
            })
        })
        // Add file resources using filesystem foundation
        .resources(
            pmcp::server::simple_resources::ResourceCollection::new()
                .add_dynamic_provider(Arc::new(
                    fs.create_resource_provider("reports://")
                ))
        )
        .build()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ExpenseReportInput {
    token: String,
    month: String,
}
```

## Foundation Versioning

When foundations evolve, version them carefully:

```rust
/// Foundation trait with version information
pub trait Foundation: Send + Sync {
    /// Foundation version for compatibility checking
    fn version(&self) -> &str;

    /// Minimum compatible version
    fn min_compatible_version(&self) -> &str;
}

impl Foundation for AuthFoundation {
    fn version(&self) -> &str {
        "2.0.0"
    }

    fn min_compatible_version(&self) -> &str {
        "1.5.0"  // Backwards compatible with 1.5+
    }
}

/// Check foundation compatibility before composing
fn check_compatibility(foundation: &dyn Foundation, required_version: &str) -> Result<()> {
    let version = semver::Version::parse(foundation.version())?;
    let required = semver::Version::parse(required_version)?;

    if version < required {
        return Err(pmcp::Error::protocol(
            pmcp::ErrorCode::INTERNAL_ERROR,
            format!(
                "Foundation version {} is below required version {}",
                version, required
            ),
        ));
    }

    Ok(())
}
```

## Testing Foundations

Foundations need thorough testing since bugs affect all consumers:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn auth_foundation_validates_tokens() {
        let auth = AuthFoundation::new();

        // Valid token
        let user = auth.validate_token("valid_user123").await.unwrap();
        assert_eq!(user.id, "user123");

        // Invalid token
        let result = auth.validate_token("invalid_token").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_foundation_checks_roles() {
        let auth = AuthFoundation::new();
        let user = auth.validate_token("valid_admin").await.unwrap();

        assert!(auth.has_role(&user, "employee"));
        assert!(!auth.has_role(&user, "super_admin"));
    }

    #[tokio::test]
    async fn filesystem_prevents_traversal() {
        let fs = FileSystemFoundation::new("/data");

        // Attempting path traversal should fail
        let result = fs.read_file("../../../etc/passwd").await;
        assert!(result.is_err());

        // Valid path should work
        let result = fs.read_file("reports/q1.txt").await;
        // Depends on actual file existence
    }

    #[tokio::test]
    async fn database_validates_tables() {
        let db = DatabaseFoundation::new("postgres://localhost/test");
        let provider = db.create_table_provider(vec!["users".to_string(), "orders".to_string()]);

        // Allowed table
        let templates = provider.templates();
        assert_eq!(templates.len(), 3);

        // Verify URI template format
        assert!(templates[0].uri_template.contains("{table}"));
    }
}
```

## Summary

| Foundation Type | Provides | Used By |
|-----------------|----------|---------|
| **Authentication** | Token validation, user info, roles | All domain servers |
| **Database** | Connection pooling, query execution, resource providers | Servers needing data access |
| **File System** | Safe file access, directory listing | Servers handling documents |
| **HTTP Client** | External API calls, retry logic | Integration servers |
| **Cache** | In-memory and distributed caching | Performance-critical servers |

Building good foundations takes time upfront but pays dividends as your MCP server ecosystem grows. Every domain server benefits from the shared, well-tested, consistently-behaved foundation layer.

---

*Continue to [Domain Servers](./ch19-02-domains.md) →*
