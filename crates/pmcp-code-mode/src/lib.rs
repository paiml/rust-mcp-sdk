// Originated from pmcp-run/built-in/shared/pmcp-code-mode (https://github.com/guyernest/pmcp-run)
// Moved into rust-mcp-sdk workspace as a first-class SDK crate for Phase 67.1
//
// Clippy pedantic/nursery allows for code imported from pmcp-run.
// These will be cleaned up incrementally in future phases.
#![allow(clippy::use_self)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_raw_string_hashes)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::unused_self)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::self_only_used_in_recursion)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::manual_is_variant_and)]
#![allow(clippy::unnecessary_literal_bound)]

//! Code Mode - LLM-generated query validation and execution.
//!
//! This crate provides the infrastructure for "Code Mode", which allows MCP clients
//! to generate and execute structured queries (GraphQL, SQL, REST) with a validation
//! pipeline that ensures security and provides human-readable explanations.
//!
//! ## Architecture
//!
//! ```text
//! describe_schema() → LLM generates code → validate_code() → user approval → execute_code()
//! ```
//!
//! ## Key Components
//!
//! - **Validation Pipeline**: Parse → Policy Check → Security Analysis → Explanation → Token
//! - **Approval Tokens**: HMAC-signed tokens binding code hash to validation result
//! - **Explanations**: Template-based business-language descriptions of queries
//! - **Policy Evaluation**: Pluggable trait for Cedar/AVP/custom policy engines
//!
//! ## Example Usage
//!
//! ```ignore
//! use pmcp_code_mode::{
//!     CodeModeConfig, ValidationPipeline, ValidationContext
//! };
//!
//! // Create a validation pipeline
//! let config = CodeModeConfig::enabled();
//! let pipeline = ValidationPipeline::new(config, b"secret-key".to_vec());
//!
//! // Validate a query
//! let context = ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash");
//! let result = pipeline.validate_graphql_query("query { users { id name } }", &context)?;
//! ```

// High-level CodeExecutor trait (always available, no feature gate)
mod code_executor;

pub mod config;
mod explanation;
mod graphql;
pub mod handler;
mod token;
mod types;
pub mod validation;

// Code Mode instruction and policy templates
pub mod templates;

// Schema Exposure Architecture - Three-Layer Schema Model
pub mod schema_exposure;

// Policy evaluation framework
pub mod policy;

// Cedar policy annotation parsing (no AWS dependency)
pub mod policy_annotations;

// Cedar schema and policy validation (test only)
#[cfg(test)]
pub mod cedar_validation;

// JavaScript validation for OpenAPI Code Mode (requires SWC parser)
#[cfg(feature = "openapi-code-mode")]
mod javascript;

// SQL validation for SQL Code Mode (requires sqlparser)
#[cfg(feature = "sql-code-mode")]
pub mod sql;

// AWS Verified Permissions policy evaluator
#[cfg(feature = "avp")]
pub mod avp;

// JavaScript execution runtime (AST-based execution in pure Rust)
#[cfg(feature = "js-runtime")]
pub mod executor;

// Shared expression evaluation logic (used by both sync and async executors).
//
// `pub` (rather than the original `mod`) so that `tests/eval_semantic_regression.rs`
// can pin the JsonValue output of `evaluate_with_scope` and
// `evaluate_array_method_with_scope` against representative ValueExpr programs
// (Phase 75 Wave 0 Task 2 — regression contract for Wave 3's mandatory cog 123/117 → ≤25 refactor).
//
// Why public: the eval functions need to be directly callable from a separate
// `tests/` integration target so the snapshot can detect semantic drift. Making
// the module `pub` is the smallest change that exposes the symbols; alternative
// (per-symbol re-export at the crate root) would clutter the public surface
// with internal helpers like `is_truthy` / `to_number` / `evaluate_binary_op`.
#[cfg(feature = "js-runtime")]
pub mod eval;

// Re-export async_trait to avoid version conflicts in derive macro output (D-07)
pub use async_trait::async_trait;

/// The D4 path-placeholder floor, re-exported from core `pmcp`.
///
/// There is exactly **ONE** implementation of these rules and it lives in
/// `pmcp::server::schema_validation`. This is a `pub use`, never a second copy
/// (Phase 128, Q2). Two reasons the home is core rather than here:
///
/// 1. The toolkit's curated `http` build has no `pmcp-code-mode` edge and must
///    not gain one (SC-1), so the shared rule cannot live in this crate.
/// 2. This repo has a documented three-way-drift incident from a security rule
///    that existed in more than one copy, so a second denylist is a prohibited
///    shape rather than a style preference.
///
/// The re-export exists because D-09 obliges the SDK to publish the helper under
/// the name a third-party `HttpExecutor` implementor would look for. An
/// implementor whose template syntax is not OpenAPI's `{key}` can call
/// `pmcp_code_mode::validate_path_placeholder` on each value it substitutes and
/// `pmcp_code_mode::validate_resolved_path` on the composed result — or
/// `pmcp_code_mode::validate_resolved_target`, which is that rule widened by the
/// single author-written `?` separator, and is what `ResolvedPath::from_checked`
/// itself calls. Either reaches the
/// same rule the SDK itself applies before calling
/// `HttpExecutor::execute_request`. (Plain backticks, not an intra-doc link:
/// `executor` is gated on `js-runtime` and the link would not resolve in a
/// default-feature doc build.)
pub use pmcp::server::schema_validation::{
    validate_path_placeholder, validate_resolved_path, validate_resolved_target,
    PlaceholderRefusal, PlaceholderRules, PLACEHOLDER_MAX_LENGTH,
};

// High-level CodeExecutor trait (always available, no feature gate) (D-04)
pub use code_executor::CodeExecutor;

// Re-export public types
pub use config::{resolve_server_id_from_env, CodeModeConfig};

pub use explanation::{ExplanationGenerator, TemplateExplanationGenerator};

pub use graphql::{GraphQLOperationType, GraphQLQueryInfo, GraphQLValidator};

// JavaScript/OpenAPI Code Mode exports
#[cfg(feature = "openapi-code-mode")]
pub use javascript::{
    ApiCall, HttpMethod, JavaScriptCodeInfo, JavaScriptValidator, OutputDeclaration,
    SafetyViolation, SafetyViolationType,
};

// SQL Code Mode exports
#[cfg(feature = "sql-code-mode")]
pub use sql::{SqlStatementInfo, SqlStatementType, SqlValidator};

// JavaScript execution runtime exports
#[cfg(feature = "js-runtime")]
pub use executor::{
    filter_blocked_fields, find_blocked_fields_in_output, ApiCallLog, ArrayMethodCall,
    BinaryOperator, BuiltinFunction, CompileError, ExecutionConfig, ExecutionPlan, ExecutionResult,
    HttpExecutor, JsExecutor, MockExecutionMode, MockHttpExecutor, MockedCall, PathPart,
    PathTemplate, PlanCompiler, PlanExecutor, PlanMetadata, PlanStep, ResolvedPath, SdkExecutor,
    UnaryOperator, ValueExpr,
};

// Standard CodeExecutor adapters (bridge low-level traits to derive-macro-compatible API)
#[cfg(feature = "js-runtime")]
pub use code_executor::{JsCodeExecutor, SdkCodeExecutor};

// MCP Code Mode executor
#[cfg(feature = "mcp-code-mode")]
pub use executor::McpExecutor;

#[cfg(feature = "mcp-code-mode")]
pub use code_executor::McpCodeExecutor;

pub use token::{
    canonicalize_code, compute_context_hash, hash_code, ApprovalToken, HmacTokenGenerator,
    TokenGenerator, TokenSecret,
};

pub use types::{
    CodeLanguage, CodeLocation, CodeType, Complexity, ExecutionError, PolicyViolation, RiskLevel,
    SecurityAnalysis, SecurityIssue, SecurityIssueType, TokenError, UnifiedAction, ValidationError,
    ValidationMetadata, ValidationResult,
};

pub use validation::{ValidationContext, ValidationPipeline};

// Code Mode templates
pub use templates::TemplateContext;

// Code Mode handler trait and utilities
pub use handler::{
    format_error_response, format_execution_error, CodeModeHandler, CodeModeToolBuilder,
    ExecuteCodeInput, ValidateCodeInput, ValidationResponse,
};

// Policy types re-exports
pub use policy::{
    get_baseline_policies, get_code_mode_schema_json, AuthorizationDecision, NoopPolicyEvaluator,
    OperationEntity, PolicyEvaluationError, PolicyEvaluator, ServerConfigEntity,
};

#[cfg(feature = "openapi-code-mode")]
pub use policy::{
    get_openapi_baseline_policies, get_openapi_code_mode_schema_json, normalize_operation_format,
    normalize_path_to_pattern, OpenAPIServerEntity, ScriptEntity,
};

#[cfg(feature = "sql-code-mode")]
pub use policy::{
    get_sql_baseline_policies, get_sql_code_mode_schema_json, SqlServerEntity, StatementEntity,
};

// Cedar policy evaluator
#[cfg(feature = "cedar")]
pub use policy::cedar::CedarPolicyEvaluator;

// AVP (AWS Verified Permissions) policy evaluator
#[cfg(feature = "avp")]
pub use avp::{AvpClient, AvpConfig, AvpError, AvpPolicyEvaluator};

// Schema Exposure Architecture types
pub use schema_exposure::{
    CodeModeExposurePolicy, DerivationMetadata, DerivationStats, DerivedSchema, ExposureMode,
    FilterReason, FilteredOperation, GlobalBlocklist, McpExposurePolicy, MethodExposurePolicy,
    Operation, OperationCategory, OperationDetails, OperationParameter, OperationRiskLevel,
    SchemaDeriver, SchemaFormat, SchemaMetadata, SchemaSource, ToolExposurePolicy, ToolOverride,
};
