//! Code Mode Handler trait for unified soft-disable and tool management.
//!
//! This module provides the `CodeModeHandler` trait that all Code Mode implementations
//! should implement. It provides:
//!
//! - **Policy check**: Requires a policy evaluator to be configured
//! - **Pre-handle hook**: Extensible hook for soft-disable and other checks
//! - **Standard tool definitions**: Consistent `validate_code` and `execute_code` tools
//! - **Response formatting**: Consistent JSON responses across server types

use pmcp::types::{ToolAnnotations, ToolInfo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::types::{
    PolicyViolation, RiskLevel, UnifiedAction, ValidationMetadata, ValidationResult,
};

/// Response from `validate_code_impl` containing validation results plus
/// handler-specific metadata.
///
/// Wraps [`ValidationResult`] from the validation pipeline and adds fields
/// for handler-level concerns (auto-approval, unified action, code hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponse {
    /// Core validation result from the pipeline.
    #[serde(flatten)]
    pub result: ValidationResult,

    /// Whether this was auto-approved based on risk level.
    pub auto_approved: bool,

    /// Unified action (Read, Write, Delete, Admin).
    pub action: Option<UnifiedAction>,

    /// SHA-256 hash of the canonicalized code that was validated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validated_code_hash: Option<String>,
}

impl ValidationResponse {
    /// Create a successful validation response.
    pub fn success(
        explanation: String,
        risk_level: RiskLevel,
        approval_token: String,
        metadata: ValidationMetadata,
    ) -> Self {
        Self {
            result: ValidationResult::success(explanation, risk_level, approval_token, metadata),
            auto_approved: false,
            action: None,
            validated_code_hash: None,
        }
    }

    /// Create a failed validation response.
    pub fn failure(violations: Vec<PolicyViolation>, metadata: ValidationMetadata) -> Self {
        Self {
            result: ValidationResult::failure(violations, metadata),
            auto_approved: false,
            action: None,
            validated_code_hash: None,
        }
    }

    /// Create from an existing `ValidationResult`.
    pub fn from_result(result: ValidationResult) -> Self {
        Self {
            result,
            auto_approved: false,
            action: None,
            validated_code_hash: None,
        }
    }

    /// Set the validated code hash (SHA-256 of canonicalized code).
    pub fn with_code_hash(mut self, hash: String) -> Self {
        self.validated_code_hash = Some(hash);
        self
    }

    /// Set the action for this response.
    pub fn with_action(mut self, action: UnifiedAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Set auto_approved flag.
    pub fn with_auto_approved(mut self, auto_approved: bool) -> Self {
        self.auto_approved = auto_approved;
        self
    }

    /// Add warnings to the response.
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.result.warnings = warnings;
        self
    }

    /// Convert to JSON response format.
    ///
    /// Returns a tuple of (json_value, is_error).
    pub fn to_json_response(&self) -> (Value, bool) {
        let response = json!({
            "valid": self.result.is_valid,
            "explanation": self.result.explanation,
            "risk_level": format!("{}", self.result.risk_level),
            "approval_token": self.result.approval_token,
            "action": self.action.as_ref().map(|a| a.to_string()),
            "auto_approved": self.auto_approved,
            "warnings": self.result.warnings,
            "violations": self.result.violations.iter().map(|v| json!({
                "policy": v.policy_name,
                "rule": v.rule,
                "message": v.message,
                "suggestion": v.suggestion
            })).collect::<Vec<_>>(),
            "validated_code_hash": self.validated_code_hash,
            "metadata": {
                "is_read_only": self.result.metadata.is_read_only,
                "accessed_types": self.result.metadata.accessed_types,
                "accessed_fields": self.result.metadata.accessed_fields,
                "validation_time_ms": self.result.metadata.validation_time_ms
            }
        });

        (response, !self.result.is_valid)
    }
}

/// Code Mode handler trait with policy check and standard tool handling.
#[async_trait::async_trait]
pub trait CodeModeHandler: Send + Sync {
    /// Get the server name/ID for identification.
    fn server_name(&self) -> &str;

    /// Check if Code Mode is enabled in the configuration.
    fn is_enabled(&self) -> bool;

    /// Get the code format for this server (e.g., "graphql", "javascript", "sql").
    fn code_format(&self) -> &str;

    /// Validate code and return a validation response.
    async fn validate_code_impl(
        &self,
        code: &str,
        variables: Option<&Value>,
        dry_run: bool,
        user_id: &str,
        session_id: &str,
    ) -> Result<ValidationResponse, String>;

    /// Execute validated code and return the result.
    async fn execute_code_impl(
        &self,
        code: &str,
        approval_token: &str,
        variables: Option<&Value>,
    ) -> Result<Value, String>;

    /// Check if a policy evaluator is configured.
    ///
    /// Defaults to `false` (safe default). When `false`, `handle_tool` rejects
    /// all requests with a "policy evaluator required" error. Implementations
    /// that have configured a policy evaluator MUST override this to return `true`.
    fn is_policy_configured(&self) -> bool {
        false
    }

    /// Deprecated alias for `is_policy_configured()`.
    fn is_avp_configured(&self) -> bool {
        self.is_policy_configured()
    }

    /// Pre-handle hook for checks before tool execution.
    ///
    /// Override this to implement soft-disable checks (e.g., DynamoDB toggle).
    /// Return `Ok(Some((response, is_error)))` to short-circuit with a response.
    /// Return `Ok(None)` to proceed normally.
    async fn pre_handle_hook(&self) -> Result<Option<(Value, bool)>, String> {
        Ok(None)
    }

    // =========================================================================
    // Provided methods with default implementations
    // =========================================================================

    /// Check if this is a Code Mode tool.
    fn is_code_mode_tool(&self, name: &str) -> bool {
        name == "validate_code" || name == "execute_code"
    }

    /// Get the standard Code Mode tool definitions.
    fn get_tools(&self) -> Vec<ToolInfo> {
        if !self.is_enabled() {
            return vec![];
        }

        CodeModeToolBuilder::new(self.code_format()).build_tools()
    }

    /// Handle a Code Mode tool call with policy and pre-handle checks.
    async fn handle_tool(
        &self,
        name: &str,
        arguments: Value,
        user_id: &str,
        session_id: &str,
    ) -> Result<(Value, bool), String> {
        // Policy enforcement: require a policy evaluator to be configured
        if !self.is_policy_configured() {
            return Ok((
                json!({
                    "error": "Code Mode requires a policy evaluator to be configured. \
                              Configure AVP, local Cedar, or another policy backend.",
                    "valid": false
                }),
                true,
            ));
        }

        // Pre-handle hook (soft-disable, etc.)
        if let Some(response) = self.pre_handle_hook().await? {
            return Ok(response);
        }

        match name {
            "validate_code" => {
                self.handle_validate_code(arguments, user_id, session_id)
                    .await
            },
            "execute_code" => self.handle_execute_code(arguments).await,
            _ => Err(format!("Unknown Code Mode tool: {}", name)),
        }
    }

    /// Handle validate_code tool call.
    async fn handle_validate_code(
        &self,
        arguments: Value,
        user_id: &str,
        session_id: &str,
    ) -> Result<(Value, bool), String> {
        let mut input: ValidateCodeInput =
            serde_json::from_value(arguments).map_err(|e| format!("Invalid arguments: {}", e))?;

        input.code = input.code.trim().to_string();

        let response = self
            .validate_code_impl(
                &input.code,
                input.variables.as_ref(),
                input.dry_run.unwrap_or(false),
                user_id,
                session_id,
            )
            .await?;

        Ok(response.to_json_response())
    }

    /// Handle execute_code tool call.
    async fn handle_execute_code(&self, arguments: Value) -> Result<(Value, bool), String> {
        let mut input: ExecuteCodeInput =
            serde_json::from_value(arguments).map_err(|e| format!("Invalid arguments: {}", e))?;

        input.code = input.code.trim().to_string();

        let result = self
            .execute_code_impl(&input.code, &input.approval_token, input.variables.as_ref())
            .await?;

        Ok((result, false))
    }
}

/// Input for validate_code tool.
#[derive(Debug, Deserialize)]
pub struct ValidateCodeInput {
    pub code: String,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub dry_run: Option<bool>,
}

/// Input for execute_code tool.
#[derive(Debug, Deserialize)]
pub struct ExecuteCodeInput {
    pub code: String,
    pub approval_token: String,
    #[serde(default)]
    pub variables: Option<Value>,
}

/// Builder for standard Code Mode tool definitions.
pub struct CodeModeToolBuilder {
    code_format: String,
}

impl CodeModeToolBuilder {
    /// Create a new tool builder for the given code format.
    pub fn new(code_format: &str) -> Self {
        Self {
            code_format: code_format.to_string(),
        }
    }

    /// Build the standard Code Mode tools.
    pub fn build_tools(&self) -> Vec<ToolInfo> {
        vec![self.build_validate_tool(), self.build_execute_tool()]
    }

    /// The annotations for `validate_code`, and ONLY for `validate_code`: a pure
    /// static analysis that touches no external state and can be retried freely.
    ///
    /// **Do not hand these to [`Self::build_execute_tool`].** They were shared by
    /// both tools briefly and that was wrong in the one direction that matters:
    /// `readOnlyHint`/`destructiveHint` are the hints a host reads to decide
    /// whether a call needs human confirmation, so declaring them on a tool that
    /// EXECUTES caller-supplied code tells every host it may auto-approve
    /// arbitrary execution. The claim was only ever true for apps whose declared
    /// op surface is all reads (`OperationEntry { category: "read" }`, and for
    /// GraphQL `allow_mutations = false`), and nothing enforces that — so the
    /// truthful answer for `execute_code` is to declare NOTHING and let the MCP
    /// defaults (`readOnlyHint = false`, `destructiveHint = true`) stand until
    /// the hints can be DERIVED per app.
    fn safe_read_annotations() -> ToolAnnotations {
        ToolAnnotations::new()
            .with_read_only(true)
            .with_destructive(false)
            .with_open_world(false)
            .with_idempotent(true)
    }

    pub fn build_validate_tool(&self) -> ToolInfo {
        ToolInfo::with_annotations(
            "validate_code",
            Some(
                "Validates code and returns a business-language explanation with an approval token. \
                 The code is analyzed for security, complexity, and data access patterns. \
                 You MUST call this before execute_code."
                    .to_string(),
            ),
            json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "The code to validate"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Optional variables for the query"
                    },
                    "format": {
                        "type": "string",
                        "enum": [&self.code_format],
                        "description": format!("Code format. Defaults to '{}' for this server.", self.code_format)
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, validate without generating approval token"
                    }
                },
                "required": ["code"]
            }),
            Self::safe_read_annotations(),
        )
    }

    /// Build the execute_code tool definition.
    ///
    /// Deliberately carries NO annotations. `readOnlyHint`/`destructiveHint` are
    /// the hints a host reads to decide whether a call needs human confirmation,
    /// and this tool runs caller-supplied code against the app's op surface — a
    /// hardcoded "read-only, non-destructive, idempotent" would be a false safety
    /// claim for any app that exposes a single mutating op, and nothing here
    /// enforces that it does not.
    ///
    /// The durable fix is to DERIVE the hints rather than hardcode them, and the
    /// pieces already converge: `OperationRegistry::lookup_category` (config.rs),
    /// the `ValidationPipeline` the generated handler already owns at its
    /// `metadata()` call site (pmcp-code-mode-derive), and the per-script fold
    /// precedent in javascript.rs (`is_read_only`); the full Cedar/AVP design
    /// layers on policy_annotations.rs. Until then the MCP defaults
    /// (`readOnlyHint = false`, `destructiveHint = true`) are the honest answer.
    pub fn build_execute_tool(&self) -> ToolInfo {
        ToolInfo::new(
            "execute_code",
            Some(
                "Executes validated code using an approval token. \
                 The token must be obtained from validate_code and the code must match exactly."
                    .into(),
            ),
            json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "The code to execute (must match validated code)"
                    },
                    "approval_token": {
                        "type": "string",
                        "description": "The approval token from validate_code"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Optional variables for the query"
                    }
                },
                "required": ["code", "approval_token"]
            }),
        )
    }
}

/// Format an error as a JSON response.
pub fn format_error_response(error: &str) -> (Value, bool) {
    (
        json!({
            "error": error,
            "valid": false
        }),
        true,
    )
}

/// Format an execution error as a JSON response.
pub fn format_execution_error(error: &str) -> (Value, bool) {
    (
        json!({
            "error": error
        }),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_response_to_json() {
        let response = ValidationResponse::success(
            "Test explanation".into(),
            RiskLevel::Low,
            "token123".into(),
            ValidationMetadata::default(),
        )
        .with_action(UnifiedAction::Read)
        .with_auto_approved(true);

        let (json, is_error) = response.to_json_response();

        assert!(!is_error);
        assert_eq!(json["valid"], true);
        assert_eq!(json["explanation"], "Test explanation");
        assert_eq!(json["risk_level"], "LOW");
        assert_eq!(json["approval_token"], "token123");
        assert_eq!(json["action"], "Read");
        assert_eq!(json["auto_approved"], true);
    }

    #[test]
    fn test_validation_response_failure() {
        let violations = vec![PolicyViolation::new("policy", "rule", "message")];
        let response = ValidationResponse::failure(violations, ValidationMetadata::default());

        let (json, is_error) = response.to_json_response();

        assert!(is_error);
        assert_eq!(json["valid"], false);
    }

    #[test]
    fn test_tool_builder() {
        let builder = CodeModeToolBuilder::new("graphql");
        let tools = builder.build_tools();

        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "validate_code");
        assert_eq!(tools[1].name, "execute_code");
    }
}
