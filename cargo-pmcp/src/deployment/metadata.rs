//! MCP Deployment Metadata Extraction and Injection
//!
//! This module extracts metadata from MCP server configurations and injects it
//! into deployment artifacts (CDK context, CloudFormation metadata).
//!
//! # Architecture
//!
//! cargo-pmcp's responsibility is metadata extraction and injection.
//! Platforms (like pmcp.run) handle actual resource provisioning based on this metadata.
//!
//! # Supported Formats
//!
//! 1. **Built-in servers** (pmcp-run style):
//!    - `builtin-manifest.toml` → server type, config path
//!    - Instance config (e.g., `instances/server.toml`) → secrets, tools
//!
//! 2. **Template-generated servers**:
//!    - `.pmcp/template-info.toml` → template provenance
//!    - Server config → secrets, parameters
//!
//! 3. **Custom servers**:
//!    - `pmcp.toml` or `.pmcp/config.toml` → manual declarations
//!
//! # Metadata Schema
//!
//! Uses `mcp:` namespace for standardized metadata:
//! - `mcp:version` - Metadata schema version
//! - `mcp:serverType` - Server type (graphql-api, openapi-api, custom)
//! - `mcp:serverId` - Unique server identifier
//! - `mcp:resources` - Required secrets, parameters, permissions
//! - `mcp:capabilities` - Tools, resources, prompts the server provides

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// MCP metadata version
pub const MCP_METADATA_VERSION: &str = "1.0";

// ============================================================================
// Core Metadata Types
// ============================================================================

/// Complete MCP server metadata for deployment.
///
/// This structure is extracted from server configuration files and injected
/// into deployment artifacts for platforms to consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMetadata {
    /// Metadata schema version (always "1.0" for now)
    pub version: String,

    /// Server type (e.g., "graphql-api", "openapi-api", "custom")
    pub server_type: String,

    /// Unique server identifier
    pub server_id: String,

    /// Template that generated this server (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Version of the template used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_version: Option<String>,

    /// Required resources (secrets, parameters, permissions)
    pub resources: ResourceRequirements,

    /// Server capabilities (tools, resources, prompts)
    pub capabilities: ServerCapabilities,

    /// Available operations for Code Mode policy enforcement.
    /// Extracted from config.toml or set directly in CloudFormation Metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_operations: Option<AvailableOperations>,

    /// Whether the server's snapshot is baked into the deployment artifact.
    ///
    /// Drives the `mcp:snapshotBaked` template/synth literal (DSTK-03). Defaults
    /// to `false` and is skipped during serialization when `false` so servers
    /// that do not opt in produce byte-identical JSON / CloudFormation metadata
    /// (the Phase 98 backward-compat contract).
    #[serde(default, skip_serializing_if = "is_false")]
    pub snapshot_baked: bool,

    /// Whether this deploy's `deploy/lib/stack.ts` is a hand-modified,
    /// no-longer-scaffold file (Task 7, CFN-renderer extraction routing).
    ///
    /// Set by `targets::pmcp_run::deploy::synth_template` alongside
    /// `server_type`/`snapshot_baked` when the on-disk stack.ts no longer
    /// matches what `cargo pmcp` would itself regenerate — the same signal
    /// that routes synth to the legacy `cdk synth` subprocess instead of the
    /// pure `pmcp-cfn-renderer` crate. Drives the `mcp:customStack`
    /// template/synth literal, conditionally emitted like `snapshotBaked` so
    /// the untainted (default) path stays byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub custom_stack: bool,
}

/// Serde predicate: `true` when the bool is `false`.
///
/// Used by `McpMetadata::snapshot_baked`'s `skip_serializing_if` so the default
/// (`false`) field is elided, keeping no-opt-in metadata byte-identical (DSTK-03
/// backward-compat).
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

/// Resource requirements for the server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Required secrets (sensitive values)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretRequirement>,

    /// Required parameters (non-sensitive configuration)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParameterRequirement>,

    /// Required permissions (network access, AWS services, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PermissionRequirement>,
}

/// A required secret for the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRequirement {
    /// Secret name (e.g., "STATE_POLICIES_API_KEY")
    pub name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the server requires this secret to function
    pub required: bool,

    /// Environment variable name the server code expects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,

    /// URL where developers can obtain this secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obtain_url: Option<String>,
}

/// A required parameter (non-sensitive configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRequirement {
    /// Parameter name
    pub name: String,

    /// Parameter value (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Environment variable name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A required permission for the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequirement {
    /// Permission identifier
    pub id: String,

    /// Permission type (outbound_https, s3_read, dynamodb, etc.)
    #[serde(rename = "type")]
    pub permission_type: String,

    /// Specific targets (URLs, ARN patterns, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Server capabilities advertised via MCP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// List of tool names this server provides
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,

    /// Whether server provides MCP resources
    #[serde(default)]
    pub resources: bool,

    /// List of prompt names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<String>,

    /// Whether server supports composition (server-to-server calls)
    #[serde(default)]
    pub composition: bool,
}

/// Available operations for Code Mode policy enforcement.
///
/// Operations are categorized by access level (read/write/delete/admin) based
/// on the server type. The pmcp.run admin UI uses this to populate the Code Mode
/// settings page, allowing administrators to adjust policies per operation.
///
/// This is extracted from `config.toml` in the deploy ZIP, or set directly
/// in CloudFormation Metadata as `mcp:availableOperations`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableOperations {
    /// Read operations (GET, query, SELECT)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<OperationEntry>,

    /// Write operations (POST/PUT/PATCH, mutation, INSERT/UPDATE)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<OperationEntry>,

    /// Delete operations (DELETE, destructive mutations, DELETE SQL)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deletes: Vec<OperationEntry>,

    /// Admin operations (schema changes, user management, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admin: Vec<OperationEntry>,

    /// ISO 8601 timestamp of when operations were last extracted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

/// A single operation available in Code Mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    /// Operation identifier (tool name, query name, endpoint path, table name)
    pub id: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Server-type-specific metadata
    #[serde(default, skip_serializing_if = "OperationMetadata::is_empty")]
    pub metadata: OperationMetadata,
}

/// Server-type-specific metadata for an operation.
///
/// Different server types populate different fields:
/// - **OpenAPI:** `path`, `method`
/// - **GraphQL:** `operation_type`, `destructive_hint`
/// - **SQL:** `table`, `access_type`
/// - **MCP-API:** `read_only_hint`, `destructive_hint`, `operation_category`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationMetadata {
    // OpenAPI fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    // GraphQL fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_type: Option<String>,

    // SQL fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_type: Option<String>,

    // MCP-API / shared fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_category: Option<String>,
}

impl OperationMetadata {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

// ============================================================================
// Built-in Manifest Types (pmcp-run format)
// These structs are used for TOML deserialization. Fields may not be directly
// read but are required for serde to properly deserialize the config files.
// ============================================================================

/// Built-in server manifest (servers/{name}/builtin-manifest.toml)
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BuiltinManifest {
    pub server: BuiltinServerSection,

    #[serde(default)]
    pub features: BuiltinFeaturesSection,

    #[serde(default)]
    #[allow(dead_code)]
    pub build: BuiltinBuildSection,

    #[serde(default)]
    pub resources: Option<BuiltinResourcesSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BuiltinServerSection {
    /// Server type (e.g., "graphql-api", "openapi-api")
    #[serde(rename = "type")]
    pub server_type: String,

    /// Path to the instance configuration file
    pub config: String,

    /// Optional template provenance
    #[serde(default)]
    pub template: Option<TemplateProvenance>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TemplateProvenance {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BuiltinFeaturesSection {
    /// Parse secrets from the config file
    #[serde(default)]
    pub secrets_from_config: bool,

    /// Enable composition permissions
    #[serde(default)]
    pub composition_enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BuiltinBuildSection {
    #[serde(default)]
    pub pre_build: Vec<String>,

    #[serde(default)]
    pub lambda_crate: Option<String>,
}

/// Optional explicit resource declarations in manifest
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BuiltinResourcesSection {
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,

    #[serde(default)]
    pub permissions: Vec<PermissionDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ParameterDefinition {
    pub name: String,
    pub env_var: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PermissionDefinition {
    pub id: String,
    #[serde(rename = "type")]
    pub permission_type: String,
    #[serde(default)]
    pub targets: Vec<String>,
    pub description: Option<String>,
}

// ============================================================================
// Instance Config Types (instances/*.toml)
// These structs are used for TOML deserialization. Fields may not be directly
// read but are required for serde to properly deserialize the config files.
// ============================================================================

/// Instance configuration file (e.g., instances/state-policies.toml)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct InstanceConfig {
    pub server: InstanceServerSection,

    #[serde(default)]
    pub secrets: SecretsSection,

    #[serde(default)]
    pub backend: Option<BackendSection>,

    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    #[serde(default)]
    pub observability: Option<ObservabilitySection>,

    /// Code Mode configuration with operation declarations for policy enforcement
    #[serde(default)]
    pub code_mode: Option<CodeModeSection>,

    /// Database tables (SQL servers)
    #[serde(default)]
    pub database: Option<DatabaseSection>,
}

/// Code Mode section in config.toml — declares available operations
/// and policy settings. The pmcp.run handler reads this to populate
/// the admin UI's Code Mode policy page.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CodeModeSection {
    /// Whether write operations are allowed
    #[serde(default)]
    pub allow_writes: bool,

    /// Whether delete operations are allowed
    #[serde(default)]
    pub allow_deletes: bool,

    /// Tables blocked from Code Mode access (SQL servers)
    #[serde(default)]
    pub blocked_tables: Vec<String>,

    /// Explicit operation declarations (preferred over [[tools]] when present)
    #[serde(default)]
    pub operations: Vec<CodeModeOperation>,
}

/// An operation declared in config.toml for Code Mode policy enforcement.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CodeModeOperation {
    /// Operation name/identifier
    pub name: String,

    /// Human-readable description
    pub description: Option<String>,

    /// HTTP path (OpenAPI servers)
    pub path: Option<String>,

    /// HTTP method (OpenAPI servers)
    pub method: Option<String>,

    /// GraphQL operation type: "query" or "mutation"
    pub operation_type: Option<String>,

    /// Whether this is a destructive operation
    #[serde(default)]
    pub destructive_hint: bool,

    /// Explicit category override: "read", "write", "delete", "admin"
    pub operation_category: Option<String>,
}

/// Database section in config.toml (SQL servers).
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct DatabaseSection {
    /// Available tables
    #[serde(default)]
    pub tables: Vec<DatabaseTable>,
}

/// A database table declaration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct DatabaseTable {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct InstanceServerSection {
    /// Server ID (preferred for deployment naming)
    pub id: Option<String>,
    /// Server name/display name
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SecretsSection {
    #[serde(default)]
    pub provider: Option<String>,

    #[serde(default)]
    pub definitions: Vec<SecretDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SecretDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    pub obtain_url: Option<String>,
    pub env_var: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BackendSection {
    pub endpoint: Option<String>,
    #[serde(default)]
    pub schema: Option<SchemaSection>,
    #[serde(default)]
    pub auth: Option<AuthSection>,
    #[serde(default)]
    pub http: Option<HttpSection>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SchemaSection {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct AuthSection {
    #[serde(rename = "type")]
    pub auth_type: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub query_params: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct HttpSection {
    pub timeout_seconds: Option<u32>,
    pub retries: Option<u32>,
    pub retry_backoff_ms: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    // Other fields omitted - we only need the name for capabilities
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ObservabilitySection {
    pub log_level: Option<String>,
    pub log_requests: Option<bool>,
    pub enable_metrics: Option<bool>,
    pub enable_tracing: Option<bool>,
}

// ============================================================================
// Metadata Extraction
// ============================================================================

impl McpMetadata {
    /// Extract metadata from a project directory.
    ///
    /// Checks for configuration files in this order:
    /// 1. `builtin-manifest.toml` (pmcp-run built-in servers)
    /// 2. `.pmcp/template-info.toml` (template-generated servers)
    /// 3. `pmcp.toml` (custom servers)
    ///
    /// Falls back to minimal metadata if no config is found.
    pub fn extract(project_root: &Path) -> Result<Self> {
        // Check for builtin-manifest.toml (pmcp-run format)
        let manifest_path = project_root.join("builtin-manifest.toml");
        if manifest_path.exists() {
            return Self::from_builtin_manifest(&manifest_path, project_root);
        }

        // Check for .pmcp/template-info.toml
        let template_info_path = project_root.join(".pmcp/template-info.toml");
        if template_info_path.exists() {
            return Self::from_template_info(&template_info_path, project_root);
        }

        // Check for pmcp.toml
        let pmcp_toml_path = project_root.join("pmcp.toml");
        if pmcp_toml_path.exists() {
            return Self::from_pmcp_toml(&pmcp_toml_path);
        }

        // Check for .pmcp/config.toml
        let pmcp_config_path = project_root.join(".pmcp/config.toml");
        if pmcp_config_path.exists() {
            return Self::from_pmcp_toml(&pmcp_config_path);
        }

        // Fall back to minimal metadata from Cargo.toml
        Self::default_from_cargo(project_root)
    }

    /// Extract metadata from a builtin-manifest.toml file.
    fn from_builtin_manifest(manifest_path: &Path, project_root: &Path) -> Result<Self> {
        let manifest_content = std::fs::read_to_string(manifest_path)
            .context("Failed to read builtin-manifest.toml")?;

        let manifest: BuiltinManifest =
            toml::from_str(&manifest_content).context("Failed to parse builtin-manifest.toml")?;

        // Resolve the config path relative to the manifest
        let manifest_dir = manifest_path.parent().unwrap_or(project_root);
        let config_path = manifest_dir.join(&manifest.server.config);

        // Parse the instance config
        let instance_config = if config_path.exists() {
            let config_content =
                std::fs::read_to_string(&config_path).context("Failed to read instance config")?;
            Some(
                toml::from_str::<InstanceConfig>(&config_content)
                    .context("Failed to parse instance config")?,
            )
        } else {
            None
        };

        // Extract server info
        // Prefer server.id over server.name for deployment naming (id should be kebab-case)
        let server_id = instance_config
            .as_ref()
            .map(|c| c.server.id.clone().unwrap_or_else(|| c.server.name.clone()))
            .unwrap_or_else(|| {
                project_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

        // Extract secrets
        let secrets = if manifest.features.secrets_from_config {
            instance_config
                .as_ref()
                .map(|c| {
                    c.secrets
                        .definitions
                        .iter()
                        .map(|s| SecretRequirement {
                            name: s.name.clone(),
                            description: s.description.clone(),
                            required: s.required,
                            env_var: s.env_var.clone(),
                            obtain_url: s.obtain_url.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![]
        };

        // Extract tools for capabilities
        let tools = instance_config
            .as_ref()
            .map(|c| c.tools.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();

        // Extract parameters and permissions from manifest if present
        let parameters = manifest
            .resources
            .as_ref()
            .map(|r| {
                r.parameters
                    .iter()
                    .map(|p| ParameterRequirement {
                        name: p.name.clone(),
                        value: p.value.clone(),
                        env_var: p.env_var.clone(),
                        description: p.description.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let permissions = manifest
            .resources
            .as_ref()
            .map(|r| {
                r.permissions
                    .iter()
                    .map(|p| PermissionRequirement {
                        id: p.id.clone(),
                        permission_type: p.permission_type.clone(),
                        targets: p.targets.clone(),
                        description: p.description.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Extract template provenance
        let (template_id, template_version) = manifest
            .server
            .template
            .as_ref()
            .map(|t| (Some(t.id.clone()), Some(t.version.clone())))
            .unwrap_or((None, None));

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: manifest.server.server_type,
            server_id,
            template_id,
            template_version,
            resources: ResourceRequirements {
                secrets,
                parameters,
                permissions,
            },
            capabilities: ServerCapabilities {
                tools,
                resources: false,
                prompts: vec![],
                composition: manifest.features.composition_enabled,
            },
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        })
    }

    /// Extract metadata from .pmcp/template-info.toml
    fn from_template_info(template_info_path: &Path, project_root: &Path) -> Result<Self> {
        // For now, treat template-info the same as builtin manifest
        // This can be extended later for different format
        let content = std::fs::read_to_string(template_info_path)
            .context("Failed to read template-info.toml")?;

        // Try to parse as builtin manifest format first
        if toml::from_str::<BuiltinManifest>(&content).is_ok() {
            return Self::from_builtin_manifest(template_info_path, project_root);
        }

        // Fall back to minimal metadata
        Self::default_from_cargo(project_root)
    }

    /// Extract metadata from pmcp.toml (custom servers)
    fn from_pmcp_toml(pmcp_toml_path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(pmcp_toml_path).context("Failed to read pmcp.toml")?;

        // Try to parse as instance config format
        if let Ok(config) = toml::from_str::<InstanceConfig>(&content) {
            let secrets = config
                .secrets
                .definitions
                .iter()
                .map(|s| SecretRequirement {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    required: s.required,
                    env_var: s.env_var.clone(),
                    obtain_url: s.obtain_url.clone(),
                })
                .collect();

            let tools = config.tools.iter().map(|t| t.name.clone()).collect();

            return Ok(Self {
                version: MCP_METADATA_VERSION.to_string(),
                server_type: "custom".to_string(),
                server_id: config.server.name,
                template_id: None,
                template_version: None,
                resources: ResourceRequirements {
                    secrets,
                    parameters: vec![],
                    permissions: vec![],
                },
                capabilities: ServerCapabilities {
                    tools,
                    resources: false,
                    prompts: vec![],
                    composition: false,
                },
                available_operations: None,
                snapshot_baked: false,
                custom_stack: false,
            });
        }

        // If parsing fails, return minimal metadata
        let server_id = pmcp_toml_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: "custom".to_string(),
            server_id,
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        })
    }

    /// Create minimal metadata from Cargo.toml
    fn default_from_cargo(project_root: &Path) -> Result<Self> {
        let cargo_toml_path = project_root.join("Cargo.toml");
        let server_id = if cargo_toml_path.exists() {
            let content = std::fs::read_to_string(&cargo_toml_path)?;
            // Simple extraction of package name
            content
                .lines()
                .find(|l| l.starts_with("name"))
                .and_then(|l| l.split('=').nth(1))
                .map(|n| n.trim().trim_matches('"').to_string())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            project_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: "custom".to_string(),
            server_id,
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        })
    }

    /// Apply operator `[metadata]` overrides from `.pmcp/deploy.toml`.
    ///
    /// When `config.server_type` is `Some`, it REPLACES the extracted/hardcoded
    /// `server_type` (DSTK-02: lets a custom/pmcp.toml server advertise e.g.
    /// `graph-rag` instead of the hardcoded `'custom'`). When
    /// `config.snapshot_baked` is `Some`, it sets `snapshot_baked` (DSTK-03,
    /// enabling the `mcp:snapshotBaked` literal). Absent fields leave the
    /// extracted/default values untouched, preserving backward-compat.
    pub fn apply_config_overrides(&mut self, config: &crate::deployment::config::MetadataConfig) {
        if let Some(server_type) = &config.server_type {
            self.server_type = server_type.clone();
        }
        if let Some(snapshot_baked) = config.snapshot_baked {
            self.snapshot_baked = snapshot_baked;
        }
    }

    /// Convert metadata to CDK context arguments.
    ///
    /// Returns a vector of `-c 'key=value'` arguments for CDK commands.
    /// All values are single-quoted to prevent shell interpretation of spaces/special chars.
    pub fn to_cdk_context(&self) -> Vec<String> {
        // Quote all values to handle spaces and special characters safely
        let mut context = vec![
            format!("-c 'mcp:version={}'", self.version),
            format!("-c 'mcp:serverType={}'", self.server_type),
            format!("-c 'mcp:serverId={}'", self.server_id),
        ];

        if let Some(ref template_id) = self.template_id {
            context.push(format!("-c 'mcp:templateId={}'", template_id));
        }

        if let Some(ref template_version) = self.template_version {
            context.push(format!("-c 'mcp:templateVersion={}'", template_version));
        }

        // DSTK-03: only emit mcp:snapshotBaked when opted in, so non-opting
        // servers' synth args are byte-identical (backward-compat).
        if self.snapshot_baked {
            context.push(format!("-c 'mcp:snapshotBaked={}'", self.snapshot_baked));
        }

        // Task 7 (CFN-renderer extraction): only emit mcp:customStack when
        // the routing taint was set, mirroring mcp:snapshotBaked's
        // conditional-emission/backward-compat pattern above.
        if self.custom_stack {
            context.push(format!("-c 'mcp:customStack={}'", self.custom_stack));
        }

        // Serialize resources as JSON with single quotes for shell safety
        if let Ok(resources_json) = serde_json::to_string(&self.resources) {
            context.push(format!("-c 'mcp:resources={}'", resources_json));
        }

        // Serialize capabilities as JSON with single quotes for shell safety
        if let Ok(capabilities_json) = serde_json::to_string(&self.capabilities) {
            context.push(format!("-c 'mcp:capabilities={}'", capabilities_json));
        }

        context
    }

    /// Convert metadata to CloudFormation-style metadata object.
    ///
    /// This format is used in CloudFormation template Metadata section — the
    /// `pmcp_run` renderer path's `build_render_params` (T7 review fix)
    /// consumes this directly via `cloudformation_metadata_from`, so this is
    /// no longer dead code (the `#[allow(dead_code)]` this carried before
    /// that wiring is removed).
    pub fn to_cloudformation_metadata(&self) -> serde_json::Value {
        let mut metadata = serde_json::json!({
            "mcp:version": self.version,
            "mcp:serverType": self.server_type,
            "mcp:serverId": self.server_id,
            "mcp:resources": self.resources,
            "mcp:capabilities": self.capabilities,
        });

        if let Some(ref template_id) = self.template_id {
            metadata["mcp:templateId"] = serde_json::json!(template_id);
        }

        if let Some(ref template_version) = self.template_version {
            metadata["mcp:templateVersion"] = serde_json::json!(template_version);
        }

        // DSTK-03: mirror the conditional to_cdk_context emission so non-opting
        // servers' CloudFormation metadata is byte-identical.
        if self.snapshot_baked {
            metadata["mcp:snapshotBaked"] = serde_json::json!(self.snapshot_baked);
        }

        if let Some(ref ops) = self.available_operations {
            metadata["mcp:availableOperations"] = serde_json::json!(ops);
        }

        metadata
    }

    /// Check if metadata has any secrets declared.
    #[allow(dead_code)]
    pub fn has_secrets(&self) -> bool {
        !self.resources.secrets.is_empty()
    }

    /// Get the list of required secret names.
    #[allow(dead_code)]
    pub fn required_secret_names(&self) -> Vec<&str> {
        self.resources
            .secrets
            .iter()
            .filter(|s| s.required)
            .map(|s| s.name.as_str())
            .collect()
    }
}

// ============================================================================
// Display Implementation
// ============================================================================

impl std::fmt::Display for McpMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "MCP Server Metadata v{}", self.version)?;
        writeln!(f, "  Server ID: {}", self.server_id)?;
        writeln!(f, "  Server Type: {}", self.server_type)?;

        if let Some(ref template_id) = self.template_id {
            writeln!(f, "  Template: {}", template_id)?;
        }

        if !self.resources.secrets.is_empty() {
            writeln!(f, "  Secrets: {}", self.resources.secrets.len())?;
            for secret in &self.resources.secrets {
                let required = if secret.required { "*" } else { "" };
                writeln!(f, "    - {}{}", secret.name, required)?;
            }
        }

        if !self.capabilities.tools.is_empty() {
            writeln!(f, "  Tools: {}", self.capabilities.tools.join(", "))?;
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_builtin_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("builtin-manifest.toml");
        let config_path = temp_dir.path().join("config.toml");

        // Write builtin manifest
        let manifest_content = r#"
[server]
type = "graphql-api"
config = "config.toml"

[features]
secrets_from_config = true
composition_enabled = false
"#;
        std::fs::write(&manifest_path, manifest_content).unwrap();

        // Write instance config
        let config_content = r#"
[server]
name = "test-server"
version = "1.0.0"
description = "A test server"

[secrets]
provider = "auto"

[[secrets.definitions]]
name = "API_KEY"
description = "API key for authentication"
required = true
obtain_url = "https://example.com/keys"

[[tools]]
name = "test-tool"
description = "A test tool"
"#;
        std::fs::write(&config_path, config_content).unwrap();

        let metadata = McpMetadata::extract(temp_dir.path()).unwrap();

        assert_eq!(metadata.version, "1.0");
        assert_eq!(metadata.server_type, "graphql-api");
        assert_eq!(metadata.server_id, "test-server");
        assert_eq!(metadata.resources.secrets.len(), 1);
        assert_eq!(metadata.resources.secrets[0].name, "API_KEY");
        assert!(metadata.resources.secrets[0].required);
        assert_eq!(metadata.capabilities.tools, vec!["test-tool"]);
        assert!(!metadata.capabilities.composition);
    }

    #[test]
    fn test_to_cdk_context() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "graphql-api".to_string(),
            server_id: "test-server".to_string(),
            template_id: Some("types/graphql".to_string()),
            template_version: Some("1.0.0".to_string()),
            resources: ResourceRequirements {
                secrets: vec![SecretRequirement {
                    name: "API_KEY".to_string(),
                    description: Some("Test key".to_string()),
                    required: true,
                    env_var: Some("API_KEY".to_string()),
                    obtain_url: None,
                }],
                parameters: vec![],
                permissions: vec![],
            },
            capabilities: ServerCapabilities {
                tools: vec!["test-tool".to_string()],
                resources: false,
                prompts: vec![],
                composition: false,
            },
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        };

        let context = metadata.to_cdk_context();

        assert!(context.iter().any(|c| c.contains("mcp:version=1.0")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:serverType=graphql-api")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:serverId=test-server")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:templateId=types/graphql")));
    }

    #[test]
    fn test_to_cloudformation_metadata() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "custom".to_string(),
            server_id: "my-server".to_string(),
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        };

        let cf_metadata = metadata.to_cloudformation_metadata();

        assert_eq!(cf_metadata["mcp:version"], "1.0");
        assert_eq!(cf_metadata["mcp:serverType"], "custom");
        assert_eq!(cf_metadata["mcp:serverId"], "my-server");
    }

    #[test]
    fn test_default_from_cargo() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        let cargo_content = r#"
[package]
name = "my-mcp-server"
version = "0.1.0"
"#;
        std::fs::write(&cargo_toml, cargo_content).unwrap();

        let metadata = McpMetadata::extract(temp_dir.path()).unwrap();

        assert_eq!(metadata.server_type, "custom");
        assert_eq!(metadata.server_id, "my-mcp-server");
    }

    /// Build a minimal `custom` metadata fixture for the DSTK-02/03 tests.
    fn custom_metadata() -> McpMetadata {
        McpMetadata {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: "custom".to_string(),
            server_id: "my-server".to_string(),
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        }
    }

    /// DSTK-02 + DSTK-03: `apply_config_overrides` replaces `server_type` and
    /// sets `snapshot_baked` from a `[metadata]` block; absent fields are left
    /// untouched.
    #[test]
    fn apply_config_overrides_replaces_server_type_and_sets_snapshot_baked() {
        use crate::deployment::config::MetadataConfig;

        let mut metadata = custom_metadata();
        metadata.apply_config_overrides(&MetadataConfig {
            server_type: Some("graph-rag".to_string()),
            snapshot_baked: Some(true),
        });

        assert_eq!(metadata.server_type, "graph-rag");
        assert!(metadata.snapshot_baked);

        // Absent fields leave the values untouched.
        let mut untouched = custom_metadata();
        untouched.apply_config_overrides(&MetadataConfig::default());
        assert_eq!(untouched.server_type, "custom");
        assert!(!untouched.snapshot_baked);
    }

    /// DSTK-03: `to_cdk_context` emits `mcp:snapshotBaked=true` when opted in and
    /// OMITS it when `false` (backward-compat byte-identity).
    #[test]
    fn to_cdk_context_snapshot_baked_is_conditional() {
        let mut metadata = custom_metadata();

        // Off by default → no snapshotBaked arg.
        let off = metadata.to_cdk_context();
        assert!(
            !off.iter().any(|c| c.contains("mcp:snapshotBaked")),
            "non-opting server must NOT emit mcp:snapshotBaked"
        );

        // Opted in → arg present.
        metadata.snapshot_baked = true;
        let on = metadata.to_cdk_context();
        assert!(
            on.iter().any(|c| c.contains("mcp:snapshotBaked=true")),
            "opted-in server must emit mcp:snapshotBaked=true"
        );
    }

    /// Task 7 (CFN-renderer extraction): `to_cdk_context` emits
    /// `mcp:customStack=true` when the routing taint is set and OMITS it
    /// otherwise — the same conditional-emission/backward-compat pattern as
    /// `mcp:snapshotBaked` above.
    #[test]
    fn to_cdk_context_custom_stack_is_conditional() {
        let mut metadata = custom_metadata();

        let off = metadata.to_cdk_context();
        assert!(
            !off.iter().any(|c| c.contains("mcp:customStack")),
            "an untainted deploy must NOT emit mcp:customStack"
        );

        metadata.custom_stack = true;
        let on = metadata.to_cdk_context();
        assert!(
            on.iter().any(|c| c.contains("mcp:customStack=true")),
            "a tainted (hand-modified stack.ts) deploy must emit mcp:customStack=true"
        );
    }

    /// DSTK-02: a `[metadata].server_type` override surfaces in `to_cdk_context`
    /// even for a `custom`/pmcp.toml server (previously forced to `'custom'`).
    #[test]
    fn config_server_type_override_surfaces_in_cdk_context() {
        use crate::deployment::config::MetadataConfig;

        let mut metadata = custom_metadata();
        metadata.apply_config_overrides(&MetadataConfig {
            server_type: Some("graph-rag".to_string()),
            snapshot_baked: Some(true),
        });

        let context = metadata.to_cdk_context();
        assert!(
            context
                .iter()
                .any(|c| c.contains("mcp:serverType=graph-rag")),
            "config server_type override must reach to_cdk_context"
        );
        assert!(
            context.iter().any(|c| c.contains("mcp:snapshotBaked=true")),
            "config snapshot_baked must reach to_cdk_context"
        );
    }

    /// DSTK-03 backward-compat: a `false` `snapshot_baked` is elided from the
    /// serialized JSON (no `snapshotBaked` key), keeping no-opt-in output
    /// byte-identical.
    #[test]
    fn snapshot_baked_false_is_elided_from_json() {
        let metadata = custom_metadata();
        let json = serde_json::to_string(&metadata).expect("serialize");
        assert!(
            !json.contains("snapshot_baked") && !json.contains("snapshotBaked"),
            "snapshot_baked=false must be elided from serialized JSON, got: {json}"
        );
    }

    // ========================================================================
    // DSTK-04 — config-survives-render property test (proptest)
    // ========================================================================
    //
    // The `to_cdk_context` synth seam is the real (bin-only `pub`) render
    // boundary the operator's `[metadata]` flows through. This is the ALWAYS
    // property coverage mandated by CLAUDE.md for the config-driven metadata
    // path: for ARBITRARY valid inputs, a config-declared `server_type` /
    // `snapshot_baked` survives `apply_config_overrides` → `to_cdk_context`
    // exactly — `mcp:serverType={server_type}` is always present, and
    // `mcp:snapshotBaked=true` is present iff `snapshot_baked` was opted in.
    mod dstk04_proptests {
        use super::custom_metadata;
        use crate::deployment::config::MetadataConfig;
        use proptest::prelude::*;

        /// Strategy for a sane, non-empty `server_type` string.
        ///
        /// Restricted to ASCII alnum + dash so the generated value cannot break
        /// the single-quoted `-c 'mcp:serverType=…'` shell arg that
        /// `to_cdk_context` emits (the same shell-safety convention T-98-06 the
        /// production path relies on). Length-bounded to keep the case space
        /// small and fast.
        fn server_type_strategy() -> impl Strategy<Value = String> {
            "[A-Za-z0-9][A-Za-z0-9-]{0,31}".prop_filter("non-empty", |s| !s.is_empty())
        }

        proptest! {
            /// For arbitrary `server_type`/`snapshot_baked`, the config value
            /// round-trips into the synth context exactly: serverType is always
            /// advertised; snapshotBaked is advertised iff opted in.
            #[test]
            fn config_metadata_survives_into_cdk_context(
                server_type in server_type_strategy(),
                snapshot_baked in any::<bool>(),
            ) {
                let mut metadata = custom_metadata();
                metadata.apply_config_overrides(&MetadataConfig {
                    server_type: Some(server_type.clone()),
                    snapshot_baked: Some(snapshot_baked),
                });

                let context = metadata.to_cdk_context();

                // server_type ALWAYS surfaces (DSTK-02).
                let expected_server_type = format!("mcp:serverType={server_type}");
                prop_assert!(
                    context.iter().any(|c| c.contains(&expected_server_type)),
                    "config server_type must surface in to_cdk_context: looked for {expected_server_type:?} in {context:?}"
                );

                // snapshotBaked surfaces IFF opted in (DSTK-03 — byte-identity
                // for non-opting servers).
                let has_snapshot = context.iter().any(|c| c.contains("mcp:snapshotBaked=true"));
                prop_assert_eq!(
                    has_snapshot,
                    snapshot_baked,
                    "mcp:snapshotBaked=true must be present IFF snapshot_baked is opted in (was {}); context: {:?}",
                    snapshot_baked,
                    context
                );

                // A non-opting render must NOT leak any snapshotBaked arg.
                if !snapshot_baked {
                    prop_assert!(
                        !context.iter().any(|c| c.contains("mcp:snapshotBaked")),
                        "non-opting server must emit NO mcp:snapshotBaked arg; context: {context:?}"
                    );
                }
            }

            /// Absent config fields leave the extracted/default metadata
            /// untouched for arbitrary inputs (backward-compat invariant).
            #[test]
            fn absent_config_fields_leave_metadata_untouched(
                _seed in any::<u8>(),
            ) {
                let mut metadata = custom_metadata();
                let before_server_type = metadata.server_type.clone();
                metadata.apply_config_overrides(&MetadataConfig::default());
                prop_assert_eq!(metadata.server_type, before_server_type);
                prop_assert!(!metadata.snapshot_baked);
            }
        }
    }
}
