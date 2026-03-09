//! WASM-compatible typed tools with automatic schema generation
//!
//! Provides type-safe tool creation for WASM environments (Browser, Cloudflare Workers, WASI).
//! Mirrors the native typed tool API but works within WASM constraints.
//!
//! **Note**: Currently supports input typing only. Output typing (`TypedToolWithOutput`)
//! is available in the native API (`typed_tool.rs`) but not yet implemented for WASM
//! due to async constraints and cancellation token compatibility.

use crate::server::wasm_server::{WasmMcpServerBuilder, WasmTool};
use crate::types::ToolInfo;
use crate::{Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::marker::PhantomData;

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;

/// A typed tool for WASM environments with automatic schema generation.
pub struct WasmTypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T) -> Result<Value> + Send + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    ui_resource_uri: Option<String>,
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> WasmTypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T) -> Result<Value> + Send + Sync,
{
    /// Create a new typed tool with automatic schema generation.
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema,
    {
        let schema = generate_schema::<T>();
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            ui_resource_uri: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create a new typed tool with a manually provided schema.
    pub fn new_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            ui_resource_uri: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add schema from a type (for use when created with new_with_schema).
    #[cfg(feature = "schema-generation")]
    pub fn with_schema_from<S: JsonSchema>(mut self) -> Self {
        self.input_schema = generate_schema::<S>();
        self
    }

    /// Associate this tool with a UI resource (MCP Apps Extension).
    ///
    /// Sets `_meta.ui.resourceUri` and `openai/outputTemplate` in the tool's
    /// `ToolInfo` metadata for MCP and ChatGPT host compatibility.
    pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(ui_resource_uri.into());
        self
    }
}

impl<T, F> WasmTool for WasmTypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T) -> Result<Value> + Send + Sync,
{
    fn execute(&self, args: Value) -> Result<Value> {
        // Deserialize and validate the arguments
        let typed_args: T = serde_json::from_value(args).map_err(|e| {
            Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args)
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: None,
            _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
            execution: None,
        }
    }
}

/// Generate a JSON schema for a type using schemars.
#[cfg(feature = "schema-generation")]
fn generate_schema<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);
    let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    });

    // Use WASM-safe normalization
    normalize_schema_wasm_safe(json_schema)
}

/// WASM-safe schema normalization (no filesystem operations).
#[cfg(feature = "schema-generation")]
fn normalize_schema_wasm_safe(schema: Value) -> Value {
    // Reuse the normalization logic but ensure it's WASM-compatible
    #[cfg(feature = "schema-generation")]
    {
        crate::server::schema_utils::normalize_schema(schema)
    }
    #[cfg(not(feature = "schema-generation"))]
    {
        schema
    }
}

/// Simple typed tool helper for common cases.
pub struct SimpleWasmTool<T>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    handler: Box<dyn Fn(T) -> Result<T> + Send + Sync>,
}

impl<T> SimpleWasmTool<T>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    /// Create a simple tool that transforms input to output of the same type.
    #[cfg(feature = "schema-generation")]
    pub fn new<F>(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema,
        F: Fn(T) -> Result<T> + Send + Sync + 'static,
    {
        let schema = generate_schema::<T>();
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            handler: Box::new(handler),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl<T> WasmTool for SimpleWasmTool<T>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    fn execute(&self, args: Value) -> Result<Value> {
        let typed_args: T = serde_json::from_value(args).map_err(|e| {
            Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;

        let result = (self.handler)(typed_args)?;

        serde_json::to_value(result)
            .map_err(|e| Error::Internal(format!("Failed to serialize tool output: {}", e)))
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: None,
            _meta: None,
            execution: None,
        }
    }
}

/// Extension methods for WasmMcpServerBuilder to add typed tools.
impl WasmMcpServerBuilder {
    /// Add a typed tool with automatic schema generation.
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed<T, F>(self, name: impl Into<String> + Clone, handler: F) -> Self
    where
        T: DeserializeOwned + JsonSchema + Send + Sync + 'static,
        F: Fn(T) -> Result<Value> + Send + Sync + 'static,
    {
        let tool = WasmTypedTool::new(name.clone(), handler);
        self.tool(name, tool)
    }

    /// Add a typed tool with a custom schema.
    pub fn tool_typed_with_schema<T, F>(
        self,
        name: impl Into<String> + Clone,
        schema: Value,
        handler: F,
    ) -> Self
    where
        T: DeserializeOwned + Send + Sync + 'static,
        F: Fn(T) -> Result<Value> + Send + Sync + 'static,
    {
        let tool = WasmTypedTool::new_with_schema(name.clone(), schema, handler);
        self.tool(name, tool)
    }

    /// Add a simple typed tool (input and output are the same type).
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_simple<T, F>(self, name: impl Into<String> + Clone, handler: F) -> Self
    where
        T: DeserializeOwned + Serialize + JsonSchema + Send + Sync + 'static,
        F: Fn(T) -> Result<T> + Send + Sync + 'static,
    {
        let tool = SimpleWasmTool::new(name.clone(), handler);
        self.tool(name, tool)
    }
}

// Re-export for convenience (only available on non-WASM targets)
#[cfg(not(target_arch = "wasm32"))]
pub use crate::server::error_codes::{ValidationError, ValidationErrorCode};

/// WASM-safe validation helpers (no filesystem operations).
/// Note: This module requires error_codes which is not available on WASM targets.
#[cfg(not(target_arch = "wasm32"))]
pub mod validation {
    use crate::server::error_codes::{ValidationError, ValidationErrorCode};
    use crate::{Error, Result};

    /// Validate email format (WASM-safe).
    pub fn validate_email(field: &str, value: &str) -> Result<()> {
        if !value.contains('@') || !value.contains('.') || value.len() < 5 {
            return Err(
                ValidationError::new(ValidationErrorCode::InvalidFormat, field)
                    .expected("Valid email address (e.g., user@example.com)")
                    .to_error(),
            );
        }
        Ok(())
    }

    /// Validate URL format (WASM-safe).
    pub fn validate_url(field: &str, value: &str) -> Result<()> {
        if !value.starts_with("http://") && !value.starts_with("https://") {
            return Err(
                ValidationError::new(ValidationErrorCode::InvalidFormat, field)
                    .expected("Valid URL starting with http:// or https://")
                    .to_error(),
            );
        }
        Ok(())
    }

    /// Validate numeric range (WASM-safe).
    pub fn validate_range<T>(field: &str, value: T, min: T, max: T) -> Result<()>
    where
        T: PartialOrd + std::fmt::Display,
    {
        if value < min || value > max {
            return Err(ValidationError::new(ValidationErrorCode::OutOfRange, field)
                .expected(format!("Value between {} and {}", min, max))
                .to_error());
        }
        Ok(())
    }

    /// Validate string length (WASM-safe).
    pub fn validate_length(
        field: &str,
        value: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Result<()> {
        let len = value.len();

        if let Some(min_len) = min {
            if len < min_len {
                return Err(ValidationError::new(ValidationErrorCode::TooShort, field)
                    .expected(format!("Minimum length {}", min_len))
                    .to_error());
            }
        }

        if let Some(max_len) = max {
            if len > max_len {
                return Err(ValidationError::new(ValidationErrorCode::TooLong, field)
                    .expected(format!("Maximum length {}", max_len))
                    .to_error());
            }
        }

        Ok(())
    }

    /// WASM-safe path validation (string-based only, no filesystem).
    pub fn validate_path_string(field: &str, path: &str) -> Result<()> {
        // Check for null bytes
        if path.contains('\0') {
            return Err(
                ValidationError::new(ValidationErrorCode::SecurityViolation, field)
                    .message("Path contains null bytes")
                    .to_error(),
            );
        }

        // Check for path traversal patterns
        if path.contains("..") {
            return Err(
                ValidationError::new(ValidationErrorCode::SecurityViolation, field)
                    .message("Path traversal detected (.. not allowed)")
                    .to_error(),
            );
        }

        // Check for absolute paths that might escape sandbox
        #[cfg(not(windows))]
        if path.starts_with('/') && !path.starts_with("/tmp/") && !path.starts_with("/sandbox/") {
            return Err(
                ValidationError::new(ValidationErrorCode::SecurityViolation, field)
                    .message("Absolute paths must be in /tmp/ or /sandbox/")
                    .to_error(),
            );
        }

        // Check path depth (count separators)
        let depth = path.chars().filter(|&c| c == '/' || c == '\\').count();
        if depth > 10 {
            return Err(ValidationError::new(ValidationErrorCode::OutOfRange, field)
                .message(format!("Path depth {} exceeds maximum 10", depth))
                .to_error());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    #[cfg_attr(feature = "schema-generation", derive(JsonSchema))]
    struct TestArgs {
        message: String,
        count: Option<u32>,
    }

    #[test]
    fn test_wasm_typed_tool() {
        let tool = WasmTypedTool::new_with_schema(
            "test_tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                    "count": { "type": "integer" }
                }
            }),
            |args: TestArgs| {
                let count = args.count.unwrap_or(1);
                let result = vec![args.message; count as usize].join(", ");
                Ok(serde_json::json!({ "result": result }))
            },
        );

        let args = serde_json::json!({
            "message": "hello",
            "count": 3
        });

        let result = tool.execute(args).unwrap();
        assert_eq!(result["result"], "hello, hello, hello");
    }

    #[test]
    fn test_wasm_typed_tool_info_with_ui_has_openai_output_template() {
        let tool = WasmTypedTool::new_with_schema(
            "test_wasm_tool",
            serde_json::json!({"type": "object"}),
            |_args: serde_json::Value| Ok(serde_json::json!({})),
        )
        .with_ui("ui://widgets/chart.html");

        let info = tool.info();
        let meta = info._meta.as_ref().expect("_meta should be present");

        // Must have nested ui.resourceUri
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://widgets/chart.html");

        // Must have openai/outputTemplate
        assert_eq!(
            meta.get("openai/outputTemplate").unwrap(),
            &serde_json::Value::String("ui://widgets/chart.html".to_string())
        );
    }

    #[test]
    fn test_wasm_typed_tool_info_without_ui_has_no_meta() {
        let tool = WasmTypedTool::new_with_schema(
            "test_wasm_tool",
            serde_json::json!({"type": "object"}),
            |_args: serde_json::Value| Ok(serde_json::json!({})),
        );

        let info = tool.info();
        assert!(info._meta.is_none(), "_meta should be None without UI");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_wasm_validation() {
        use validation::*;

        // Email validation
        assert!(validate_email("email", "user@example.com").is_ok());
        assert!(validate_email("email", "invalid").is_err());

        // URL validation
        assert!(validate_url("url", "https://example.com").is_ok());
        assert!(validate_url("url", "not-a-url").is_err());

        // Range validation
        assert!(validate_range("age", 25, 18, 65).is_ok());
        assert!(validate_range("age", 10, 18, 65).is_err());

        // Path validation (WASM-safe)
        assert!(validate_path_string("path", "subdir/file.txt").is_ok());
        assert!(validate_path_string("path", "../etc/passwd").is_err());
        assert!(validate_path_string("path", "/etc/passwd").is_err());
    }
}
