//! Core types for Code Mode.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Risk level assessed for a query or workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Read-only, small result set, no sensitive data
    Low,
    /// Read-only with sensitive data, or small mutations
    Medium,
    /// Large mutations, cross-table operations
    High,
    /// Schema changes, bulk deletes, admin operations
    Critical,
}

impl RiskLevel {
    /// Returns true if this risk level requires human approval.
    pub fn requires_approval(&self, auto_approve_levels: &[RiskLevel]) -> bool {
        !auto_approve_levels.contains(self)
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Type of code being validated/executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeType {
    /// GraphQL query (read-only)
    GraphQLQuery,
    /// GraphQL mutation (write)
    GraphQLMutation,
    /// SQL SELECT query
    SqlQuery,
    /// SQL INSERT/UPDATE/DELETE
    SqlMutation,
    /// REST GET request
    RestGet,
    /// REST POST/PUT/DELETE request
    RestMutation,
    /// Multi-tool workflow
    Workflow,
}

impl CodeType {
    /// Returns true if this code type is read-only.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            CodeType::GraphQLQuery | CodeType::SqlQuery | CodeType::RestGet
        )
    }
}

/// Unified action model that maps to business permissions.
/// Works consistently across GraphQL, OpenAPI, and SQL servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnifiedAction {
    /// Retrieve data without modification (Query, GET, SELECT)
    Read,
    /// Create or modify data (Mutation create/update, POST/PUT/PATCH, INSERT/UPDATE)
    Write,
    /// Remove data (Mutation delete, DELETE, DELETE/TRUNCATE)
    Delete,
    /// Schema changes, permissions, admin operations (DDL: CREATE/ALTER/DROP)
    Admin,
}

impl UnifiedAction {
    /// Infer action from GraphQL operation type.
    pub fn from_graphql(operation: &str, mutation_name: Option<&str>) -> Self {
        match operation.to_lowercase().as_str() {
            "query" => Self::Read,
            "mutation" => {
                if let Some(name) = mutation_name {
                    let lower = name.to_lowercase();
                    if lower.starts_with("delete")
                        || lower.starts_with("remove")
                        || lower.starts_with("purge")
                    {
                        return Self::Delete;
                    }
                }
                Self::Write
            },
            _ => Self::Read,
        }
    }

    /// Infer action from HTTP method.
    pub fn from_http_method(method: &str) -> Self {
        match method.to_uppercase().as_str() {
            "GET" | "HEAD" | "OPTIONS" => Self::Read,
            "POST" | "PUT" | "PATCH" => Self::Write,
            "DELETE" => Self::Delete,
            _ => Self::Read,
        }
    }

    /// Infer action from SQL statement type.
    pub fn from_sql(statement_type: &str) -> Self {
        match statement_type.to_uppercase().as_str() {
            "SELECT" => Self::Read,
            "INSERT" | "UPDATE" | "MERGE" => Self::Write,
            "DELETE" | "TRUNCATE" => Self::Delete,
            "CREATE" | "ALTER" | "DROP" | "GRANT" | "REVOKE" => Self::Admin,
            _ => Self::Read,
        }
    }

    /// Resolve action with optional tag override.
    pub fn resolve(
        inferred: Self,
        action_tags: &HashMap<String, String>,
        operation_name: &str,
    ) -> Self {
        if let Some(tag) = action_tags.get(operation_name) {
            match tag.to_lowercase().as_str() {
                "read" => Self::Read,
                "write" => Self::Write,
                "delete" => Self::Delete,
                "admin" => Self::Admin,
                _ => inferred,
            }
        } else {
            inferred
        }
    }
}

impl std::fmt::Display for UnifiedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
            Self::Delete => write!(f, "Delete"),
            Self::Admin => write!(f, "Admin"),
        }
    }
}

/// Result of validating code through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the code is valid and can be executed
    pub is_valid: bool,

    /// Human-readable explanation of what the code does
    pub explanation: String,

    /// Assessed risk level
    pub risk_level: RiskLevel,

    /// Signed approval token (if valid)
    pub approval_token: Option<String>,

    /// Detailed metadata about the validation
    pub metadata: ValidationMetadata,

    /// Any policy violations found
    pub violations: Vec<PolicyViolation>,

    /// Warnings (non-blocking)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a successful validation result.
    pub fn success(
        explanation: String,
        risk_level: RiskLevel,
        approval_token: String,
        metadata: ValidationMetadata,
    ) -> Self {
        Self {
            is_valid: true,
            explanation,
            risk_level,
            approval_token: Some(approval_token),
            metadata,
            violations: vec![],
            warnings: vec![],
        }
    }

    /// Create a failed validation result.
    pub fn failure(violations: Vec<PolicyViolation>, metadata: ValidationMetadata) -> Self {
        Self {
            is_valid: false,
            explanation: String::new(),
            risk_level: RiskLevel::Critical,
            approval_token: None,
            metadata,
            violations,
            warnings: vec![],
        }
    }
}

/// Detailed metadata about a validation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationMetadata {
    /// Whether the code is read-only
    pub is_read_only: bool,

    /// Estimated number of rows that will be returned/affected
    pub estimated_rows: Option<u64>,

    /// Tables/types accessed by the code
    pub accessed_types: Vec<String>,

    /// Fields accessed by the code
    pub accessed_fields: Vec<String>,

    /// Whether the query has aggregations
    pub has_aggregation: bool,

    /// Code type detected
    pub code_type: Option<CodeType>,

    /// Unified action determined for this operation
    pub action: Option<UnifiedAction>,

    /// Time taken to validate (milliseconds)
    pub validation_time_ms: u64,
}

/// Security analysis of code.
#[derive(Debug, Clone, Default)]
pub struct SecurityAnalysis {
    /// Whether the code is read-only
    pub is_read_only: bool,

    /// Tables/types accessed
    pub tables_accessed: HashSet<String>,

    /// Fields accessed
    pub fields_accessed: HashSet<String>,

    /// Whether the query has aggregations
    pub has_aggregation: bool,

    /// Whether the query has subqueries/nested operations
    pub has_subqueries: bool,

    /// Estimated complexity
    pub estimated_complexity: Complexity,

    /// Potential security issues found
    pub potential_issues: Vec<SecurityIssue>,

    /// Estimated number of rows
    pub estimated_rows: Option<u64>,
}

impl SecurityAnalysis {
    /// Assess the risk level based on the security analysis.
    pub fn assess_risk(&self) -> RiskLevel {
        // Critical: Has critical security issues
        if self.potential_issues.iter().any(|i| i.is_critical()) {
            return RiskLevel::Critical;
        }

        // High: Mutations with high complexity or affecting many rows
        if !self.is_read_only {
            if let Some(rows) = self.estimated_rows {
                if rows > 100 {
                    return RiskLevel::High;
                }
            }
            if matches!(self.estimated_complexity, Complexity::High) {
                return RiskLevel::High;
            }
            return RiskLevel::Medium;
        }

        // Medium: Read-only but has sensitive issues or high complexity
        if self.potential_issues.iter().any(|i| i.is_sensitive()) {
            return RiskLevel::Medium;
        }
        if matches!(self.estimated_complexity, Complexity::High) {
            return RiskLevel::Medium;
        }

        // Low: Simple read-only queries
        RiskLevel::Low
    }
}

/// Estimated complexity of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Complexity {
    #[default]
    Low,
    Medium,
    High,
}

/// Potential security issues found during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    /// Issue type
    pub issue_type: SecurityIssueType,
    /// Human-readable message
    pub message: String,
    /// Location in code (if applicable)
    pub location: Option<CodeLocation>,
}

impl SecurityIssue {
    pub fn new(issue_type: SecurityIssueType, message: impl Into<String>) -> Self {
        Self {
            issue_type,
            message: message.into(),
            location: None,
        }
    }

    pub fn with_location(mut self, location: CodeLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Returns true if this is a critical issue that should block execution.
    /// Note: DynamicTableName is NOT critical for REST APIs - it's a common pattern
    /// for discovery-then-use workflows (e.g., search for station ID, then use in path).
    pub fn is_critical(&self) -> bool {
        matches!(self.issue_type, SecurityIssueType::PotentialInjection)
    }

    /// Returns true if this issue involves sensitive data.
    pub fn is_sensitive(&self) -> bool {
        matches!(self.issue_type, SecurityIssueType::SensitiveFields)
    }
}

/// Types of security issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityIssueType {
    /// Query without LIMIT/pagination
    UnboundedQuery,
    /// Accessing PII or sensitive columns
    SensitiveFields,
    /// Joining across security boundaries
    CrossTypeJoin,
    /// Dynamic table/type name (potential injection)
    DynamicTableName,
    /// Potential injection vulnerability
    PotentialInjection,
    /// Deeply nested query
    DeepNesting,
    /// High complexity query
    HighComplexity,
}

/// Location in source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    pub line: u32,
    pub column: u32,
}

/// A policy violation found during validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    /// Name of the policy that was violated
    pub policy_name: String,
    /// Specific rule within the policy
    pub rule: String,
    /// Location in the code where the violation occurred
    pub location: Option<CodeLocation>,
    /// Human-readable message explaining the violation
    pub message: String,
    /// Suggestion for how to fix the violation
    pub suggestion: Option<String>,
}

impl PolicyViolation {
    pub fn new(
        policy_name: impl Into<String>,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            policy_name: policy_name.into(),
            rule: rule.into(),
            location: None,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_location(mut self, location: CodeLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for PolicyViolation {
    /// One-line rendering used wherever a single violation is surfaced to a
    /// model/operator (e.g. the `tools/call` rejection message): `rule:
    /// message` plus ` — suggestion` when a fix is available.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.suggestion {
            Some(suggestion) => write!(f, "{}: {} — {}", self.rule, self.message, suggestion),
            None => write!(f, "{}: {}", self.rule, self.message),
        }
    }
}

#[cfg(test)]
mod policy_violation_display_tests {
    use super::PolicyViolation;

    #[test]
    fn renders_rule_message_and_suggestion_when_present() {
        let v = PolicyViolation::new("code_mode", "missing_limit", "SELECT must declare a LIMIT")
            .with_suggestion("Add a LIMIT clause (e.g. `LIMIT 100`).");
        assert_eq!(
            v.to_string(),
            "missing_limit: SELECT must declare a LIMIT — Add a LIMIT clause (e.g. `LIMIT 100`)."
        );
    }

    #[test]
    fn omits_dash_when_no_suggestion() {
        let v = PolicyViolation::new("code_mode", "blocked_table", "Table 'secrets' is blocked");
        assert_eq!(v.to_string(), "blocked_table: Table 'secrets' is blocked");
    }
}

/// Errors that can occur during validation.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        message: String,
        line: u32,
        column: u32,
    },

    #[error("Schema error for field '{field}': {message}")]
    SchemaError { message: String, field: String },

    #[error("Permission denied: {message} (requires: {required_permission})")]
    PermissionError {
        message: String,
        required_permission: String,
    },

    #[error("Security error: {message}")]
    SecurityError {
        message: String,
        issue: SecurityIssueType,
    },

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Errors that can occur during execution.
///
/// `#[non_exhaustive]` since 0.7: a new failure class is a new variant, and a
/// downstream `match` must carry a wildcard arm rather than break on each one.
/// Adding [`ExecutionError::RequestRefused`] is what made the enum non-exhaustive
/// in the first place; it could not be added to an exhaustive enum without
/// breaking every downstream `match`, so the two changes travel together.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExecutionError {
    #[error("Token has expired — request a new approval token via validate_code")]
    TokenExpired,

    #[error("Token signature is invalid: {0}")]
    TokenInvalid(String),

    #[error("Code hash mismatch — the code sent to execute_code does not match the code that was validated (expected {expected_hash}, got {actual_hash}). Ensure the code string is identical to what was sent to validate_code")]
    CodeMismatch {
        expected_hash: String,
        actual_hash: String,
    },

    #[error("Context has changed since validation (schema or permissions updated)")]
    ContextChanged,

    #[error("User mismatch: token was issued for a different user")]
    UserMismatch,

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("Execution timed out after {0} seconds")]
    Timeout(u32),

    #[error("Validation required before execution")]
    ValidationRequired,

    #[error("Runtime error: {message}")]
    RuntimeError { message: String },

    /// A request the script made was REFUSED before it was sent: by the SDK's
    /// path-placeholder floor, by a non-scalar path or query value, or by an
    /// embedder's outbound-request policy.
    ///
    /// Distinct from [`ExecutionError::RuntimeError`] because the two have
    /// different owners. A runtime fault is the server's; a refused request is the
    /// CALLER's to fix by changing what the script sends, so a tool handler should
    /// report it as a tool-level rejection the model can act on and never as an
    /// internal error. Until 0.7 both were `RuntimeError`, and a policy refusal
    /// reached the model as `Internal error: Execution error: Runtime error: ...`.
    ///
    /// `message` is value-free where it is raised (it names the rule and the
    /// declared expectation, never a rejected byte), and the plan executor keeps
    /// it value-free when it adds the method and result variable.
    #[error("Request refused: {message}")]
    RequestRefused { message: String },
}

/// Supported code languages for validation and execution.
///
/// Each variant selects a different validation path in the pipeline and
/// maps to the corresponding feature flag. The derive macro mirrors these
/// variants in `gen_validation_call()` (proc-macro crates cannot depend on
/// runtime crates, so the string matching is duplicated by necessity).
/// A sync test in the derive crate enforces both sides stay aligned.
///
/// # Adding a New Language
///
/// 1. Add a variant here with `from_attr` / `as_str` / `required_feature` arms
/// 2. Add the validation method to `ValidationPipeline` (feature-gated)
/// 3. Add the `quote!` branch in `pmcp-code-mode-derive/src/lib.rs` `gen_validation_call()`
/// 4. Update `CodeModeToolBuilder` for tool metadata
/// 5. Add the new string to the sync test in the derive crate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeLanguage {
    /// GraphQL queries and mutations. Default, no feature flag required.
    GraphQL,
    /// JavaScript/OpenAPI plans. Requires `openapi-code-mode` feature.
    JavaScript,
    /// Raw SQL statements. Requires `sql-code-mode` feature.
    Sql,
    /// MCP tool composition. Requires `mcp-code-mode` feature.
    Mcp,
}

impl CodeLanguage {
    /// Parse a language string from a derive macro attribute.
    ///
    /// Returns `None` for unrecognized values — the caller should emit
    /// a compile error listing supported values.
    pub fn from_attr(s: &str) -> Option<Self> {
        match s {
            "graphql" => Some(Self::GraphQL),
            "javascript" | "js" => Some(Self::JavaScript),
            "sql" => Some(Self::Sql),
            "mcp" => Some(Self::Mcp),
            _ => None,
        }
    }

    /// The string value used in tool metadata and serde serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GraphQL => "graphql",
            Self::JavaScript => "javascript",
            Self::Sql => "sql",
            Self::Mcp => "mcp",
        }
    }

    /// The feature flag required by this language, if any.
    pub fn required_feature(&self) -> Option<&'static str> {
        match self {
            Self::GraphQL => None,
            Self::JavaScript => Some("openapi-code-mode"),
            Self::Sql => Some("sql-code-mode"),
            Self::Mcp => Some("mcp-code-mode"),
        }
    }
}

impl std::fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors from token generator construction.
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    /// HMAC secret is too short for secure token generation.
    #[error("HMAC token secret must be at least {minimum} bytes, got {actual}")]
    SecretTooShort {
        /// Minimum required length in bytes.
        minimum: usize,
        /// Actual length provided.
        actual: usize,
    },
}
