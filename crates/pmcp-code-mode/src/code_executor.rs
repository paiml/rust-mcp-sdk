//! High-level code execution trait for MCP servers.
//!
//! This module provides the [`CodeExecutor`] trait, which is the primary public API
//! for implementing code execution in MCP servers. It replaces the internal
//! `HttpExecutor`, `SdkExecutor`, and `McpExecutor` traits for external server
//! developers (those traits remain available for advanced use behind the
//! `js-runtime` feature flag).

use crate::types::ExecutionError;

/// High-level trait for executing validated code.
///
/// Implementations handle the execution of code that has already passed
/// validation and token verification. This is the primary public API
/// for code execution -- it replaces the internal `HttpExecutor`,
/// `SdkExecutor`, and `McpExecutor` traits for external server developers.
///
/// # Execution Patterns
///
/// The four supported patterns (all implemented via this single trait):
/// - **Pattern A (SQL):** Direct SQL execution, no JS runtime
/// - **Pattern B (JS+HTTP):** JavaScript plan compiled and executed via HTTP calls
/// - **Pattern C (JS+SDK):** JavaScript plan executed via AWS SDK calls
/// - **Pattern D (JS+MCP):** JavaScript plan executed via MCP tool calls
///
/// # API Stability Note
///
/// **\[Addresses divergent review concern: CodeExecutor trait surface area\]**
/// This v0.1.0 API uses a simple `(code, variables)` signature per D-04.
/// A future v0.2.0 may add an `ExecutionContext` parameter carrying timeout,
/// cancellation token, and request metadata. The `(code, variables)` signature
/// will be preserved as a default-method wrapper for backward compatibility.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{CodeExecutor, ExecutionError};
/// use serde_json::Value;
///
/// struct MyExecutor { /* database pool, http client, etc. */ }
///
/// #[pmcp_code_mode::async_trait]
/// impl CodeExecutor for MyExecutor {
///     async fn execute(
///         &self,
///         code: &str,
///         variables: Option<&Value>,
///     ) -> Result<Value, ExecutionError> {
///         // Execute validated code against your backend
///         todo!()
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait CodeExecutor: Send + Sync {
    /// Execute validated code and return the result.
    ///
    /// `code` has already passed validation and token verification.
    /// `variables` are optional user-provided parameters (e.g., GraphQL variables).
    ///
    /// Implementations should NOT re-verify the token -- that is handled
    /// by the Code Mode framework before calling this method.
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError>;
}

// ---------------------------------------------------------------------------
// Standard adapters: bridge low-level executor traits to CodeExecutor
// ---------------------------------------------------------------------------
//
// These adapters solve the &mut self vs &self mismatch: PlanExecutor::execute
// requires &mut self, but CodeExecutor::execute takes &self. Each adapter
// creates a fresh PlanCompiler + PlanExecutor per call (cheap — both are small
// structs, and the caller's HttpExecutor/SdkExecutor holds Arc'd state).

/// Compile JavaScript code and execute the plan, returning the result value.
///
/// Shared implementation for all three adapters. The `setup` closure configures
/// the `PlanExecutor` with the appropriate backend (HTTP, SDK, or MCP) before
/// execution begins.
#[cfg(feature = "js-runtime")]
async fn compile_and_execute<H: crate::executor::HttpExecutor + 'static>(
    config: &crate::executor::ExecutionConfig,
    http: H,
    code: &str,
    variables: Option<&serde_json::Value>,
    setup: impl FnOnce(&mut crate::executor::PlanExecutor<H>),
    adapter: &str,
) -> Result<serde_json::Value, ExecutionError> {
    let mut compiler = crate::executor::PlanCompiler::with_config(config);
    let plan = compiler
        .compile_code(code)
        .map_err(|e| ExecutionError::RuntimeError {
            message: format!("Compilation failed: {e}"),
        })?;
    let mut executor = crate::executor::PlanExecutor::new(http, config.clone());
    if let Some(vars) = variables {
        executor.set_variable("args", vars.clone());
    }
    setup(&mut executor);
    let result = executor.execute(&plan).await?;
    tracing::debug!(
        adapter,
        api_calls = result.api_calls.len(),
        execution_time_ms = result.execution_time_ms,
        "plan executed"
    );
    Ok(result.value)
}

/// Adapter bridging [`HttpExecutor`] to [`CodeExecutor`] for JavaScript/OpenAPI
/// servers (Pattern B: JS+HTTP).
///
/// Compiles JavaScript code into an execution plan, then runs it against an
/// HTTP backend. The executor holds its own [`ExecutionConfig`] for limits
/// (`max_api_calls`, `timeout_seconds`, `max_loop_iterations`).
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{JsCodeExecutor, ExecutionConfig};
///
/// let http = CostExplorerHttpExecutor::new(clients.clone());
/// let config = ExecutionConfig::default();
/// let executor = Arc::new(JsCodeExecutor::new(http, config));
/// // Pass executor to #[derive(CodeMode)] struct as code_executor field
/// ```
#[cfg(feature = "js-runtime")]
pub struct JsCodeExecutor<H> {
    http: H,
    config: crate::executor::ExecutionConfig,
}

#[cfg(feature = "js-runtime")]
impl<H: crate::executor::HttpExecutor + Clone> JsCodeExecutor<H> {
    /// Create a new JS code executor with the given HTTP backend and config.
    pub fn new(http: H, config: crate::executor::ExecutionConfig) -> Self {
        Self { http, config }
    }
}

#[cfg(feature = "js-runtime")]
#[async_trait::async_trait]
impl<H: crate::executor::HttpExecutor + Clone + 'static> CodeExecutor for JsCodeExecutor<H> {
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        compile_and_execute(
            &self.config,
            self.http.clone(),
            code,
            variables,
            |_| {},
            "js",
        )
        .await
    }
}

/// Adapter bridging [`SdkExecutor`] to [`CodeExecutor`] for SDK-backed servers
/// (Pattern C: JS+SDK).
///
/// Uses a no-op HTTP executor stub since SDK plans route through
/// `PlanExecutor::set_sdk_executor` instead of HTTP calls.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{SdkCodeExecutor, ExecutionConfig};
///
/// let sdk = MyCostExplorerSdk::new(credentials);
/// let config = ExecutionConfig::default();
/// let executor = Arc::new(SdkCodeExecutor::new(sdk, config));
/// ```
#[cfg(feature = "js-runtime")]
pub struct SdkCodeExecutor<S> {
    sdk: S,
    config: crate::executor::ExecutionConfig,
}

#[cfg(feature = "js-runtime")]
impl<S: crate::executor::SdkExecutor + Clone + 'static> SdkCodeExecutor<S> {
    /// Create a new SDK code executor with the given SDK backend and config.
    pub fn new(sdk: S, config: crate::executor::ExecutionConfig) -> Self {
        Self { sdk, config }
    }
}

#[cfg(feature = "js-runtime")]
#[async_trait::async_trait]
impl<S: crate::executor::SdkExecutor + Clone + 'static> CodeExecutor for SdkCodeExecutor<S> {
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        let sdk = self.sdk.clone();
        compile_and_execute(
            &self.config,
            NoopHttpExecutor,
            code,
            variables,
            move |ex| {
                ex.set_sdk_executor(sdk);
            },
            "sdk",
        )
        .await
    }
}

/// Adapter bridging [`McpExecutor`] to [`CodeExecutor`] for MCP composition
/// servers (Pattern D: JS+MCP).
///
/// Uses a no-op HTTP executor stub since MCP plans route through
/// `PlanExecutor::set_mcp_executor` instead of HTTP calls.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{McpCodeExecutor, ExecutionConfig};
///
/// let mcp = MyMcpRouter::new(foundation_servers);
/// let config = ExecutionConfig::default();
/// let executor = Arc::new(McpCodeExecutor::new(mcp, config));
/// ```
///
/// Note: `mcp-code-mode` feature implies `js-runtime` in Cargo.toml,
/// which provides `PlanCompiler`, `PlanExecutor`, and `NoopHttpExecutor`.
#[cfg(feature = "mcp-code-mode")]
pub struct McpCodeExecutor<M> {
    mcp: M,
    config: crate::executor::ExecutionConfig,
}

#[cfg(feature = "mcp-code-mode")]
impl<M: crate::executor::McpExecutor + Clone + 'static> McpCodeExecutor<M> {
    /// Create a new MCP composition code executor.
    pub fn new(mcp: M, config: crate::executor::ExecutionConfig) -> Self {
        Self { mcp, config }
    }
}

#[cfg(feature = "mcp-code-mode")]
#[async_trait::async_trait]
impl<M: crate::executor::McpExecutor + Clone + 'static> CodeExecutor for McpCodeExecutor<M> {
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        let mcp = self.mcp.clone();
        compile_and_execute(
            &self.config,
            NoopHttpExecutor,
            code,
            variables,
            move |ex| {
                ex.set_mcp_executor(mcp);
            },
            "mcp",
        )
        .await
    }
}

/// No-op HTTP executor for SDK and MCP adapters that don't use HTTP calls.
/// Gated on `js-runtime`; `mcp-code-mode` implies `js-runtime` in Cargo.toml.
#[cfg(feature = "js-runtime")]
#[derive(Clone)]
struct NoopHttpExecutor;

#[cfg(feature = "js-runtime")]
#[async_trait::async_trait]
impl crate::executor::HttpExecutor for NoopHttpExecutor {
    async fn execute_request(
        &self,
        method: &str,
        path: crate::executor::ResolvedPath<'_>,
        _body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        Err(ExecutionError::RuntimeError {
            message: format!(
                "HTTP calls not supported in this executor mode (attempted {method} {path}). \
                 Use JsCodeExecutor for HTTP-based execution."
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct EchoExecutor;

    #[async_trait::async_trait]
    impl CodeExecutor for EchoExecutor {
        async fn execute(
            &self,
            code: &str,
            variables: Option<&serde_json::Value>,
        ) -> Result<serde_json::Value, ExecutionError> {
            Ok(json!({
                "code": code,
                "variables": variables,
            }))
        }
    }

    #[tokio::test]
    async fn code_executor_echo() {
        let executor = EchoExecutor;
        let result = executor.execute("SELECT 1", None).await.unwrap();
        assert_eq!(result["code"], "SELECT 1");
    }

    #[tokio::test]
    async fn code_executor_with_variables() {
        let executor = EchoExecutor;
        let vars = json!({"limit": 10});
        let result = executor
            .execute("query { users }", Some(&vars))
            .await
            .unwrap();
        assert_eq!(result["variables"]["limit"], 10);
    }

    #[tokio::test]
    async fn code_executor_returns_error() {
        struct FailingExecutor;

        #[async_trait::async_trait]
        impl CodeExecutor for FailingExecutor {
            async fn execute(
                &self,
                _code: &str,
                _variables: Option<&serde_json::Value>,
            ) -> Result<serde_json::Value, ExecutionError> {
                Err(ExecutionError::BackendError(
                    "database unavailable".to_string(),
                ))
            }
        }

        let executor = FailingExecutor;
        let result = executor.execute("SELECT 1", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("database unavailable"));
    }

    // Compile-time test: CodeExecutor requires Send + Sync
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _code_executor_is_send_sync() {
        _assert_send_sync::<EchoExecutor>();
    }

    /// T-128-21a — `NoopHttpExecutor`'s refusal names the METHOD and never the
    /// path. After Phase 128 D-09 the incoming `path` is the RESOLVED path rather
    /// than the template the script wrote, so this would otherwise be the one
    /// surviving path-echo site in the crate.
    #[cfg(feature = "js-runtime")]
    #[tokio::test]
    async fn noop_http_executor_error_names_the_method_and_not_the_path() {
        use crate::executor::{HttpExecutor, ResolvedPath};

        let path = ResolvedPath::from_checked("/secret/inventory/endpoint")
            .expect("a clean resolved path");
        let rendered = NoopHttpExecutor
            .execute_request("GET", path, None)
            .await
            .expect_err("the noop executor always refuses")
            .to_string();
        assert!(rendered.contains("GET"), "must name the method: {rendered}");
        assert!(
            !rendered.contains("/secret/inventory/endpoint"),
            "must NOT echo the resolved path: {rendered}"
        );
    }
}
