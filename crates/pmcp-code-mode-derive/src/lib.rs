// Allow needless_continue from darling's generated derive code
#![allow(clippy::needless_continue)]

//! Derive macro for Code Mode validation and execution in MCP servers.
//!
//! Provides `#[derive(CodeMode)]` which generates a `register_code_mode_tools`
//! method that registers `validate_code` and `execute_code` tools on a
//! [`pmcp::ServerBuilder`].
//!
//! # Field Name Convention (v0.1.0)
//!
//! The macro identifies required fields by **fixed well-known names**. This is
//! the v0.1.0 contract -- the field names are the API:
//!
//! | Field Name | Required Type | Purpose |
//! |------------|---------------|---------|
//! | `code_mode_config` | `CodeModeConfig` | Validation pipeline configuration |
//! | `token_secret` | `TokenSecret` | HMAC signing secret |
//! | `policy_evaluator` | `Arc<dyn PolicyEvaluator>` or `Arc<P>` | Policy evaluation |
//! | `code_executor` | `Arc<dyn CodeExecutor>` or `Arc<E>` | Code execution |
//!
//! If any required field is missing, the macro emits a **single** compile error
//! listing all absent fields.
//!
//! # Struct-Level Attributes (v0.2.0)
//!
//! | Attribute | Type | Default | Purpose |
//! |-----------|------|---------|---------|
//! | `context_from` | `String` | (none) | Method name returning `ValidationContext` |
//! | `language` | `String` | `"graphql"` | Selects validation path and tool metadata |
//!
//! Supported `language` values:
//!
//! | Value | Validation Method | Feature Required |
//! |-------|-------------------|------------------|
//! | `"graphql"` (default) | `validate_graphql_query_async` | *(none)* |
//! | `"javascript"` / `"js"` | `validate_javascript_code` | `openapi-code-mode` |
//! | `"sql"` | `validate_sql_query_async` | `sql-code-mode` |
//! | `"mcp"` | `validate_mcp_composition` | `mcp-code-mode` |
//!
//! When `context_from` is specified, `register_code_mode_tools` requires
//! `self: &Arc<Self>` and the generated handler calls `self.parent.{method}(&extra)`
//! to obtain real `ValidationContext` bound to the current user/session.
//!
//! When `context_from` is omitted, `register_code_mode_tools` uses `&self` (no Arc
//! required) with placeholder context values and a `#[deprecated]` warning guiding
//! users toward the production path.
//!
//! # Generated Code
//!
//! The macro generates:
//!
//! 1. A `register_code_mode_tools` method on the struct that takes a
//!    `ServerBuilder` **by value** and returns it (by-value fluent pattern).
//! 2. Two internal handler structs (`ValidateCodeHandler` and
//!    `ExecuteCodeHandler`) that implement `pmcp::ToolHandler`.
//! 3. A `Send + Sync` compile-time assertion (per D-08).
//!
//! # Examples
//!
//! **Production (with `context_from`):**
//! ```rust,ignore
//! #[derive(CodeMode)]
//! #[code_mode(context_from = "get_context", language = "graphql")]
//! struct MyServer {
//!     code_mode_config: CodeModeConfig,
//!     token_secret: TokenSecret,
//!     policy_evaluator: Arc<NoopPolicyEvaluator>,
//!     code_executor: Arc<MyExecutor>,
//! }
//!
//! impl MyServer {
//!     fn get_context(&self, extra: &RequestHandlerExtra) -> ValidationContext {
//!         ValidationContext::new("user-1", "session-1", "schema-v1", "perms-v1")
//!     }
//! }
//!
//! // Generated: MyServer::register_code_mode_tools(self: &Arc<Self>, builder) -> ServerBuilder
//! ```
//!
//! **Testing (without `context_from`, deprecated placeholder path):**
//! ```rust,ignore
//! #[derive(CodeMode)]
//! struct MyServer {
//!     code_mode_config: CodeModeConfig,
//!     token_secret: TokenSecret,
//!     policy_evaluator: Arc<NoopPolicyEvaluator>,
//!     code_executor: Arc<MyExecutor>,
//! }
//!
//! // Generated: #[deprecated] MyServer::register_code_mode_tools(&self, builder) -> ServerBuilder
//! ```

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Required field names for the Code Mode derive macro.
const REQUIRED_FIELDS: &[&str] = &[
    "code_mode_config",
    "token_secret",
    "policy_evaluator",
    "code_executor",
];

/// Parsed attributes from `#[derive(CodeMode)]`.
///
/// Struct-level attributes (v0.2.0):
/// - `context_from`: Optional method name for extracting `ValidationContext`.
///   When specified, the generated `register_code_mode_tools` requires `self: &Arc<Self>`.
/// - `language`: Code language for tool metadata. Defaults to `"graphql"`.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(code_mode))]
struct CodeModeOpts {
    ident: syn::Ident,
    data: darling::ast::Data<(), CodeModeField>,
    /// Optional method name for extracting `ValidationContext` from the struct.
    /// When specified, the generated registration method requires `self: &Arc<Self>`.
    #[darling(default)]
    context_from: Option<String>,
    /// Code language — selects both the validation method and tool metadata.
    /// Supported: `"graphql"` (default), `"javascript"`/`"js"`, `"sql"`, `"mcp"`.
    /// Non-default languages require their respective feature flag on `pmcp-code-mode`.
    #[darling(default)]
    language: Option<String>,
}

/// A single field parsed from the struct.
#[derive(Debug, Clone, darling::FromField)]
#[darling(attributes(code_mode))]
struct CodeModeField {
    ident: Option<syn::Ident>,
}

/// Derive macro that generates `register_code_mode_tools` for Code Mode servers.
///
/// Requires a named struct with four well-known fields:
/// `code_mode_config`, `token_secret`, `policy_evaluator`, `code_executor`.
///
/// See [crate-level documentation](crate) for the full field name convention.
#[proc_macro_derive(CodeMode, attributes(code_mode))]
pub fn code_mode_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_code_mode(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Core expansion logic for `#[derive(CodeMode)]`.
fn expand_code_mode(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let opts = CodeModeOpts::from_derive_input(input)
        .map_err(|e| syn::Error::new_spanned(input, e.to_string()))?;

    let struct_name = &opts.ident;

    // Extract named fields
    let fields = match &opts.data {
        darling::ast::Data::Struct(ref fields) => &fields.fields,
        darling::ast::Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(CodeMode)] can only be applied to structs with named fields",
            ));
        },
    };

    let field_names: Vec<String> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    // Check for missing required fields
    let missing: Vec<&str> = REQUIRED_FIELDS
        .iter()
        .filter(|&&name| !field_names.contains(&name.to_string()))
        .copied()
        .collect();

    if !missing.is_empty() {
        let all_required = REQUIRED_FIELDS.join(", ");
        let missing_msgs: Vec<String> = missing
            .iter()
            .map(|&name| {
                let type_hint = match name {
                    "code_mode_config" => "CodeModeConfig",
                    "token_secret" => "TokenSecret",
                    "policy_evaluator" => "Arc<dyn PolicyEvaluator>",
                    "code_executor" => "Arc<dyn CodeExecutor>",
                    _ => "unknown",
                };
                format!(
                    "#[derive(CodeMode)] requires field `{name}` (type: {type_hint}).\n\
                     Required fields: {all_required}"
                )
            })
            .collect();
        let msg = missing_msgs.join("\n\n");
        return Err(syn::Error::new_spanned(&input.ident, msg));
    }

    // Generate the handler module name to avoid collision (snake_case to suppress warnings)
    let mod_name = syn::Ident::new(
        &format!(
            "__code_mode_impl_{}",
            struct_name.to_string().to_lowercase()
        ),
        struct_name.span(),
    );

    // Extract and validate language (defaults to "graphql").
    // Known values mirror pmcp_code_mode::CodeLanguage — keep in sync.
    let language = opts.language.as_deref().unwrap_or("graphql");
    let language_lit = syn::LitStr::new(language, struct_name.span());

    let validation_call = gen_validation_call(language, &input.ident)?;

    // Branch code generation based on context_from presence
    let expanded = if let Some(ref method_name) = opts.context_from {
        // Validate that context_from is a valid Rust identifier
        if method_name.is_empty() || syn::parse_str::<syn::Ident>(method_name).is_err() {
            return Err(syn::Error::new_spanned(
                &input.ident,
                format!("`context_from = \"{method_name}\"` is not a valid Rust identifier"),
            ));
        }
        let method_ident = syn::Ident::new(method_name, struct_name.span());
        expand_with_context_from(
            struct_name,
            &mod_name,
            &language_lit,
            &method_ident,
            &validation_call,
        )
    } else {
        // --- default path: placeholder context with deprecation warning ---
        expand_without_context_from(struct_name, &mod_name, &language_lit, &validation_call)
    };

    Ok(expanded)
}

/// Generate the validation call token stream for the given language.
///
/// Each language maps to a specific `ValidationPipeline` method. The method may be
/// sync or async — the generated handler is always async, so sync methods work fine.
///
/// # Supported Languages
///
/// | Language | Method | Async | Feature |
/// |----------|--------|-------|---------|
/// | `graphql` | `validate_graphql_query_async` | yes | *(none)* |
/// | `javascript`/`js` | `validate_javascript_code` | no | `openapi-code-mode` |
/// | `sql` | `validate_sql_query_async` | yes | `sql-code-mode` |
/// | `mcp` | `validate_mcp_composition` | yes | `mcp-code-mode` |
///
/// To add a new language: add a match arm here and a variant to `CodeLanguage` in
/// `pmcp-code-mode/src/types.rs`.
fn gen_validation_call(
    language: &str,
    error_span: &syn::Ident,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let map_err = quote! {
        .map_err(|e| pmcp::Error::Internal(format!("Validation error: {}", e)))?
    };
    match language {
        "graphql" => Ok(quote! {
            self.pipeline.validate_graphql_query_async(code, &context).await #map_err
        }),
        "javascript" | "js" => Ok(quote! {
            self.pipeline.validate_javascript_code_async(code, &context).await #map_err
        }),
        "sql" => Ok(quote! {
            self.pipeline.validate_sql_query_async(code, &context).await #map_err
        }),
        "mcp" => Ok(quote! {
            self.pipeline.validate_mcp_composition(code, &context).await #map_err
        }),
        other => Err(syn::Error::new_spanned(
            error_span,
            format!(
                "`language = \"{other}\"` is not a supported language. \
                 Supported values: \"graphql\" (default), \"javascript\" (requires `openapi-code-mode`), \
                 \"sql\" (requires `sql-code-mode`), \"mcp\" (requires `mcp-code-mode`)"
            ),
        )),
    }
}

/// Generate code when `context_from` is specified.
///
/// The registration method requires `self: &Arc<Self>` and the validate handler
/// calls `self.parent.{method_ident}(&extra)` for real `ValidationContext`.
fn expand_with_context_from(
    struct_name: &syn::Ident,
    mod_name: &syn::Ident,
    language_lit: &syn::LitStr,
    method_ident: &syn::Ident,
    validation_call: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        // Send + Sync compile-time assertion (D-08)
        const _: fn() = || {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<#struct_name>();
        };

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use super::*;
            use std::sync::Arc;
            // Import TokenGenerator trait to bring verify/verify_code into scope
            use pmcp_code_mode::TokenGenerator as _;

            /// Internal state for the `validate_code` tool handler.
            pub(super) struct ValidateCodeHandler {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) config: pmcp_code_mode::CodeModeConfig,
                pub(super) parent: Arc<#struct_name>,
            }

            #[pmcp_code_mode::async_trait]
            impl pmcp::ToolHandler for ValidateCodeHandler {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ValidateCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();
                    let dry_run = input.dry_run.unwrap_or(false);

                    // Real ValidationContext from user-defined method
                    let context = self.parent.#method_ident(&extra);

                    let result = #validation_call;

                    let mut response = pmcp_code_mode::ValidationResponse::from_result(result);
                    if response.result.is_valid {
                        if dry_run {
                            response.result.approval_token = None;
                        }
                        let risk = response.result.risk_level;
                        response = response.with_auto_approved(self.config.should_auto_approve(risk));
                    }

                    let (json, _is_error) = response.to_json_response();
                    Ok(json)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    Some(pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_validate_tool())
                }
            }

            /// Internal state for the `execute_code` tool handler.
            pub(super) struct ExecuteCodeHandler<E: pmcp_code_mode::CodeExecutor + 'static> {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) executor: Arc<E>,
            }

            #[pmcp_code_mode::async_trait]
            impl<E: pmcp_code_mode::CodeExecutor + 'static> pmcp::ToolHandler for ExecuteCodeHandler<E> {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ExecuteCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();

                    // Verify the approval token
                    let token_gen = self.pipeline.token_generator();
                    let token = pmcp_code_mode::ApprovalToken::decode(&input.approval_token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Invalid approval token: {}. Call validate_code to obtain a valid token.", e),
                            None,
                        ))?;

                    // Verify token signature and expiry
                    token_gen.verify(&token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Approval token is invalid or expired: {}. Call validate_code again to obtain a fresh token.", e),
                            None,
                        ))?;

                    // Verify code matches the token's code hash
                    token_gen.verify_code(code, &token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Code does not match the validated code: {}. execute_code must use the exact code string that was passed to validate_code.", e),
                            None,
                        ))?;

                    // Execute the validated code
                    let result = self.executor.execute(code, input.variables.as_ref()).await
                        .map_err(|e| match e {
                            // A refused request, or a script that does not compile,
                            // is the CALLER's to fix, so it is a tool-level rejection
                            // the model can act on. Only a genuine execution fault
                            // stays an internal error.
                            pmcp_code_mode::ExecutionError::RequestRefused { message }
                            | pmcp_code_mode::ExecutionError::InvalidScript { message } => {
                                pmcp::Error::tool_rejected(message, None)
                            }
                            other => pmcp::Error::Internal(
                                format!("Execution error: {}", other),
                            ),
                        })?;

                    Ok(result)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    Some(pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_execute_tool())
                }
            }
        }

        impl #struct_name {
            /// Register Code Mode tools (`validate_code` + `execute_code`) on the builder.
            ///
            /// Uses the `context_from` method to extract real `ValidationContext` from
            /// each request. Requires `self: &Arc<Self>` to share the server reference
            /// with the generated handler.
            ///
            /// # Errors
            ///
            /// Returns [`pmcp_code_mode::TokenError`] if the `token_secret` is too short
            /// for secure HMAC token generation.
            ///
            /// # Example
            ///
            /// ```rust,ignore
            /// let server = Arc::new(my_server);
            /// let builder = server.register_code_mode_tools(Server::builder())?;
            /// ```
            pub fn register_code_mode_tools(
                self: &std::sync::Arc<Self>,
                builder: pmcp::ServerBuilder,
            ) -> Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> {
                let pipeline = std::sync::Arc::new(
                    pmcp_code_mode::ValidationPipeline::from_token_secret_with_policy(
                        self.code_mode_config.clone(),
                        &self.token_secret,
                        std::sync::Arc::clone(&self.policy_evaluator) as std::sync::Arc<dyn pmcp_code_mode::PolicyEvaluator>,
                    )?
                );

                let validate_handler = #mod_name::ValidateCodeHandler {
                    pipeline: std::sync::Arc::clone(&pipeline),
                    config: self.code_mode_config.clone(),
                    parent: std::sync::Arc::clone(self),
                };

                let execute_handler = #mod_name::ExecuteCodeHandler {
                    pipeline,
                    executor: std::sync::Arc::clone(&self.code_executor),
                };

                Ok(builder
                    .tool("validate_code", validate_handler)
                    .tool("execute_code", execute_handler))
            }
        }
    }
}

/// Generate code when `context_from` is NOT specified (backward-compatible path).
///
/// The registration method uses `&self` (no `Arc` needed) and the validate handler
/// uses placeholder `ValidationContext` values with a `#[deprecated]` warning guiding
/// users toward the `context_from` attribute for production use.
fn expand_without_context_from(
    struct_name: &syn::Ident,
    mod_name: &syn::Ident,
    language_lit: &syn::LitStr,
    validation_call: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        // Send + Sync compile-time assertion (D-08)
        const _: fn() = || {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<#struct_name>();
        };

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use super::*;
            use std::sync::Arc;
            // Import TokenGenerator trait to bring verify/verify_code into scope
            use pmcp_code_mode::TokenGenerator as _;

            /// Internal state for the `validate_code` tool handler.
            pub(super) struct ValidateCodeHandler {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) config: pmcp_code_mode::CodeModeConfig,
            }

            #[pmcp_code_mode::async_trait]
            impl pmcp::ToolHandler for ValidateCodeHandler {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ValidateCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();
                    let dry_run = input.dry_run.unwrap_or(false);

                    // WARNING: These are PLACEHOLDER values. The validation context
                    // uses static strings, so approval tokens are NOT bound to a
                    // specific user, session, or schema version. An attacker who
                    // obtains a valid token can replay it across different users and
                    // sessions until it expires.
                    //
                    // Use `#[code_mode(context_from = "method_name")]` for production.
                    let context = pmcp_code_mode::ValidationContext::new(
                        "anonymous",
                        "session",
                        "schema",
                        "perms",
                    );

                    let result = #validation_call;

                    let mut response = pmcp_code_mode::ValidationResponse::from_result(result);
                    if response.result.is_valid {
                        if dry_run {
                            response.result.approval_token = None;
                        }
                        let risk = response.result.risk_level;
                        response = response.with_auto_approved(self.config.should_auto_approve(risk));
                    }

                    let (json, _is_error) = response.to_json_response();
                    Ok(json)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    Some(pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_validate_tool())
                }
            }

            /// Internal state for the `execute_code` tool handler.
            pub(super) struct ExecuteCodeHandler<E: pmcp_code_mode::CodeExecutor + 'static> {
                pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
                pub(super) executor: Arc<E>,
            }

            #[pmcp_code_mode::async_trait]
            impl<E: pmcp_code_mode::CodeExecutor + 'static> pmcp::ToolHandler for ExecuteCodeHandler<E> {
                async fn handle(
                    &self,
                    args: serde_json::Value,
                    _extra: pmcp::RequestHandlerExtra,
                ) -> pmcp::Result<serde_json::Value> {
                    let input: pmcp_code_mode::ExecuteCodeInput =
                        serde_json::from_value(args).map_err(|e| {
                            pmcp::Error::Internal(format!("Invalid arguments: {}", e))
                        })?;

                    let code = input.code.trim();

                    // Verify the approval token
                    let token_gen = self.pipeline.token_generator();
                    let token = pmcp_code_mode::ApprovalToken::decode(&input.approval_token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Invalid approval token: {}. Call validate_code to obtain a valid token.", e),
                            None,
                        ))?;

                    // Verify token signature and expiry
                    token_gen.verify(&token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Approval token is invalid or expired: {}. Call validate_code again to obtain a fresh token.", e),
                            None,
                        ))?;

                    // Verify code matches the token's code hash
                    token_gen.verify_code(code, &token)
                        .map_err(|e| pmcp::Error::tool_rejected(
                            format!("Code does not match the validated code: {}. execute_code must use the exact code string that was passed to validate_code.", e),
                            None,
                        ))?;

                    // Execute the validated code
                    let result = self.executor.execute(code, input.variables.as_ref()).await
                        .map_err(|e| match e {
                            // A refused request, or a script that does not compile,
                            // is the CALLER's to fix, so it is a tool-level rejection
                            // the model can act on. Only a genuine execution fault
                            // stays an internal error.
                            pmcp_code_mode::ExecutionError::RequestRefused { message }
                            | pmcp_code_mode::ExecutionError::InvalidScript { message } => {
                                pmcp::Error::tool_rejected(message, None)
                            }
                            other => pmcp::Error::Internal(
                                format!("Execution error: {}", other),
                            ),
                        })?;

                    Ok(result)
                }

                fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                    Some(pmcp_code_mode::CodeModeToolBuilder::new(#language_lit).build_execute_tool())
                }
            }
        }

        impl #struct_name {
            /// Register Code Mode tools (`validate_code` + `execute_code`) on the builder.
            ///
            /// **Deprecated:** Uses placeholder `ValidationContext` values. Use
            /// `#[code_mode(context_from = "method_name")]` for production to bind
            /// approval tokens to real user identity and session.
            ///
            /// # Errors
            ///
            /// Returns [`pmcp_code_mode::TokenError`] if the `token_secret` is too short
            /// for secure HMAC token generation.
            ///
            /// # Example
            ///
            /// ```rust,ignore
            /// #[allow(deprecated)]
            /// let builder = server.register_code_mode_tools(Server::builder())?;
            /// ```
            #[deprecated(note = "Use #[code_mode(context_from = \"method_name\")] for production. This uses placeholder ValidationContext.")]
            pub fn register_code_mode_tools(
                &self,
                builder: pmcp::ServerBuilder,
            ) -> Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> {
                let pipeline = std::sync::Arc::new(
                    pmcp_code_mode::ValidationPipeline::from_token_secret_with_policy(
                        self.code_mode_config.clone(),
                        &self.token_secret,
                        std::sync::Arc::clone(&self.policy_evaluator) as std::sync::Arc<dyn pmcp_code_mode::PolicyEvaluator>,
                    )?
                );

                let validate_handler = #mod_name::ValidateCodeHandler {
                    pipeline: std::sync::Arc::clone(&pipeline),
                    config: self.code_mode_config.clone(),
                };

                let execute_handler = #mod_name::ExecuteCodeHandler {
                    pipeline,
                    executor: std::sync::Arc::clone(&self.code_executor),
                };

                Ok(builder
                    .tool("validate_code", validate_handler)
                    .tool("execute_code", execute_handler))
            }
        }
    }
}
