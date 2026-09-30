//! `#[mcp_tool]` attribute macro expansion.
//!
//! Generates a struct implementing `ToolHandler` from an annotated standalone
//! async or sync function, eliminating `Box::pin` boilerplate and providing
//! automatic schema generation, state injection, and annotation support.
//!
//! # Generated Code
//!
//! For each annotated function, the macro generates:
//! 1. The original function (preserved unchanged)
//! 2. A `{PascalCase(fn_name)}Tool` struct implementing `ToolHandler`
//! 3. A constructor function `fn fn_name() -> StructName` for ergonomic registration
//!
//! # Example
//!
//! ```rust,ignore
//! #[mcp_tool(description = "Add two numbers")]
//! async fn add(args: AddArgs) -> Result<AddResult> {
//!     Ok(AddResult { sum: args.a + args.b })
//! }
//!
//! // Register: server_builder.tool("add", add())
//! ```

use crate::mcp_common;
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{ItemFn, ReturnType, Type};

/// Parsed attributes for `#[mcp_tool(...)]`.
#[derive(Debug, FromMeta)]
pub struct McpToolArgs {
    /// Tool description (mandatory per D-05).
    pub(crate) description: String,
    /// Override tool name (defaults to function name per D-06).
    #[darling(default)]
    pub(crate) name: Option<String>,
    /// MCP standard annotations (per D-23).
    #[darling(default)]
    pub(crate) annotations: Option<McpToolAnnotations>,
    /// UI widget resource URI (per D-24).
    #[darling(default)]
    pub(crate) ui: Option<syn::Expr>,
}

/// MCP standard tool annotations parsed from macro attributes.
#[derive(Debug, Default, FromMeta)]
pub struct McpToolAnnotations {
    /// If true, the tool does not modify any state.
    #[darling(default)]
    pub(crate) read_only: Option<bool>,
    /// If true, the tool may perform destructive operations.
    #[darling(default)]
    pub(crate) destructive: Option<bool>,
    /// If true, calling the tool multiple times with same args has same effect.
    #[darling(default)]
    pub(crate) idempotent: Option<bool>,
    /// If true, the tool interacts with external systems.
    #[darling(default)]
    pub(crate) open_world: Option<bool>,
}

/// Bundled parameter classification for a `#[mcp_tool]` function.
struct ToolFnParams {
    args_type: Option<Type>,
    state_inner_ty: Option<Type>,
    param_order: Vec<mcp_common::ParamSlot>,
    has_extra: bool,
}

/// Push an args-typed parameter slot, rejecting duplicates.
fn push_tool_args_slot(
    param: &syn::FnArg,
    ty: Type,
    args_type: &mut Option<Type>,
    param_order: &mut Vec<mcp_common::ParamSlot>,
) -> syn::Result<()> {
    if args_type.is_some() {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_tool functions can have at most one args parameter",
        ));
    }
    *args_type = Some(ty);
    param_order.push(mcp_common::ParamSlot::Args);
    Ok(())
}

/// Push a `State<T>` slot, rejecting duplicates.
fn push_tool_state_slot(
    param: &syn::FnArg,
    inner_ty: Type,
    state_inner_ty: &mut Option<Type>,
    param_order: &mut Vec<mcp_common::ParamSlot>,
) -> syn::Result<()> {
    if state_inner_ty.is_some() {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_tool functions can have at most one State<T> parameter",
        ));
    }
    *state_inner_ty = Some(inner_ty);
    param_order.push(mcp_common::ParamSlot::State);
    Ok(())
}

/// Push a `RequestHandlerExtra` slot, rejecting duplicates.
fn push_tool_extra_slot(
    param: &syn::FnArg,
    has_extra: &mut bool,
    param_order: &mut Vec<mcp_common::ParamSlot>,
) -> syn::Result<()> {
    if *has_extra {
        return Err(syn::Error::new_spanned(
            param,
            "mcp_tool functions can have at most one RequestHandlerExtra parameter",
        ));
    }
    *has_extra = true;
    param_order.push(mcp_common::ParamSlot::Extra);
    Ok(())
}

/// Classify every parameter of an `#[mcp_tool]` function.
fn classify_tool_fn_params(input: &ItemFn) -> syn::Result<ToolFnParams> {
    let mut args_type: Option<Type> = None;
    let mut state_inner_ty: Option<Type> = None;
    let mut has_extra = false;
    let mut param_order: Vec<mcp_common::ParamSlot> = Vec::new();
    for param in &input.sig.inputs {
        let role = mcp_common::classify_param(param)?;
        match role {
            mcp_common::ParamRole::Args(ty) => {
                push_tool_args_slot(param, ty, &mut args_type, &mut param_order)?;
            },
            mcp_common::ParamRole::State { inner_ty, .. } => {
                push_tool_state_slot(param, inner_ty, &mut state_inner_ty, &mut param_order)?;
            },
            mcp_common::ParamRole::Extra => {
                push_tool_extra_slot(param, &mut has_extra, &mut param_order)?;
            },
            mcp_common::ParamRole::SelfRef => {
                return Err(syn::Error::new_spanned(
                    param,
                    "standalone #[mcp_tool] functions cannot have &self — use #[mcp_server] for impl block tools",
                ));
            },
        }
    }
    Ok(ToolFnParams {
        args_type,
        state_inner_ty,
        param_order,
        has_extra,
    })
}

/// Bundled state codegen for standalone `mcp_tool`.
struct ToolStateCodegen {
    struct_fields: TokenStream,
    with_state_method: TokenStream,
    constructor_default: TokenStream,
    state_resolution: TokenStream,
}

fn generate_tool_state_codegen(
    state_inner_ty: Option<&Type>,
    struct_name: &syn::Ident,
    tool_name: &str,
) -> ToolStateCodegen {
    let Some(inner) = state_inner_ty else {
        return ToolStateCodegen {
            struct_fields: quote! {},
            with_state_method: quote! {},
            constructor_default: quote! { #struct_name {} },
            state_resolution: quote! {},
        };
    };
    let inner_name = quote!(#inner).to_string();
    ToolStateCodegen {
        struct_fields: quote! { state: Option<std::sync::Arc<#inner>>, },
        with_state_method: quote! {
            /// Provide shared state for this tool.
            ///
            /// Call this at registration time to inject state:
            /// ```rust,ignore
            /// server_builder.tool("name", tool_fn().with_state(my_state))
            /// ```
            pub fn with_state(mut self, state: impl Into<std::sync::Arc<#inner>>) -> Self {
                self.state = Some(state.into());
                self
            }
        },
        constructor_default: quote! { #struct_name { state: None } },
        state_resolution: quote! {
            let state_val = pmcp::State(
                self.state.as_ref()
                    .ok_or_else(|| pmcp::Error::internal(format!(
                        "State<{}> not provided for tool '{}' -- call .with_state() during registration",
                        #inner_name, #tool_name
                    )))?
                    .clone()
            );
        },
    }
}

/// Generate the args deserialization snippet for a tool handler body.
fn generate_tool_args_deser(args_type: Option<&Type>, tool_name: &str) -> TokenStream {
    let Some(at) = args_type else {
        return quote! {};
    };
    quote! {
        let typed_args: #at = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::tool_rejected(
                format!("Invalid arguments for tool '{}': {}", #tool_name, e),
                None,
            ))?;
    }
}

/// Expand `#[mcp_tool]` attribute macro on a standalone function.
///
/// Refactored in 75-01 Task 1b-C (P1): parse/classify/codegen helpers
/// extracted ([`classify_tool_fn_params`], [`generate_tool_state_codegen`],
/// [`generate_tool_args_deser`]).
pub fn expand_mcp_tool(args: TokenStream, input: &ItemFn) -> syn::Result<TokenStream> {
    use mcp_common::ParamSlot;

    // Parse macro attributes via darling, routing through the shared resolver
    // so the rustdoc-fallback logic stays in one place (see mcp_common.rs).
    let nested_metas = crate::mcp_common::resolve_tool_args(args, &input.attrs, &input.sig.ident)?;

    let macro_args = McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;

    // Extract function info.
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let tool_name = macro_args.name.unwrap_or_else(|| fn_name_str.clone());
    let is_async = input.sig.asyncness.is_some();
    let struct_name = format_ident!("{}Tool", fn_name_str.to_upper_camel_case());
    let description = &macro_args.description;

    // Rename the original function to an internal name to avoid conflict
    // with the constructor function that uses the same name.
    let impl_fn_name = format_ident!("__{}_impl", fn_name_str);
    let mut impl_fn = input.clone();
    impl_fn.sig.ident = impl_fn_name.clone();

    let params = classify_tool_fn_params(input)?;
    let state_codegen =
        generate_tool_state_codegen(params.state_inner_ty.as_ref(), &struct_name, &tool_name);

    let struct_fields = state_codegen.struct_fields;
    let with_state_method = state_codegen.with_state_method;
    let constructor_default = state_codegen.constructor_default;
    let state_resolution = state_codegen.state_resolution;

    let args_deser = generate_tool_args_deser(params.args_type.as_ref(), &tool_name);

    // Generate extra parameter name in handle() signature.
    let extra_param_name: Ident = if params.has_extra {
        format_ident!("extra")
    } else {
        format_ident!("_extra")
    };

    // Generate function call arguments in correct parameter order.
    let call_args: Vec<TokenStream> = params
        .param_order
        .iter()
        .map(|slot| match slot {
            ParamSlot::Args => quote! { typed_args },
            ParamSlot::State => quote! { state_val },
            ParamSlot::Extra => quote! { #extra_param_name },
        })
        .collect();

    // Generate the function call (async vs sync).
    // Calls the renamed internal function, not the public constructor.
    let fn_call = if is_async {
        quote! { let result = #impl_fn_name(#(#call_args),*).await?; }
    } else {
        quote! { let result = #impl_fn_name(#(#call_args),*)?; }
    };

    // Generate result serialization.
    let result_serialize = quote! {
        serde_json::to_value(result)
            .map_err(|e| pmcp::Error::internal(format!("Failed to serialize result: {}", e)))
    };

    // Generate handle body (wrapped in async for sync functions too since ToolHandler is async).
    let handle_body = quote! {
        #args_deser
        #state_resolution
        #fn_call
        #result_serialize
    };

    // Generate input schema code for metadata().
    let input_schema_code = if let Some(ref at) = params.args_type {
        mcp_common::generate_input_schema_code(at)
    } else {
        mcp_common::generate_empty_schema_code()
    };

    // Generate output schema code for metadata().
    // Extract return type and check if it's Result<T> where T is not Value.
    let output_schema_code = extract_output_schema_code(input);

    // Generate ToolInfo construction (branching on annotations presence).
    let tool_info_code = generate_tool_info_code(
        &tool_name,
        description,
        macro_args.annotations.as_ref(),
        macro_args.ui.as_ref(),
    );

    // Assemble everything.
    let expanded = quote! {
        // Emit the original function body under an internal name to avoid
        // collision with the public constructor function.
        #impl_fn

        /// Auto-generated tool handler for the `#fn_name` MCP tool.
        #[derive(Clone)]
        pub struct #struct_name {
            #struct_fields
        }

        // Manual Debug impl to avoid requiring T: Debug on state.
        impl std::fmt::Debug for #struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!(#struct_name)).finish()
            }
        }

        #[pmcp::async_trait]
        impl pmcp::ToolHandler for #struct_name {
            async fn handle(
                &self,
                args: serde_json::Value,
                #extra_param_name: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<serde_json::Value> {
                #handle_body
            }

            fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                let input_schema = #input_schema_code;
                let output_schema: Option<serde_json::Value> = #output_schema_code;
                let mut info = #tool_info_code;
                if let Some(schema) = output_schema {
                    info = info.with_output_schema(schema);
                }
                Some(info)
            }
        }

        impl #struct_name {
            #with_state_method
        }

        /// Create a new instance of the [`#struct_name`] tool handler.
        ///
        /// Use this at registration time:
        /// ```rust,ignore
        /// server_builder.tool("#tool_name", #fn_name())
        /// ```
        pub fn #fn_name() -> #struct_name {
            #constructor_default
        }
    };

    Ok(expanded)
}

/// Extract output schema code from the function's return type.
///
/// Extract output schema code from a function's return type.
/// Delegates to `mcp_common::output_schema_tokens`.
fn extract_output_schema_code(input: &ItemFn) -> TokenStream {
    let return_type = match &input.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some(ty.as_ref()),
    };
    mcp_common::output_schema_tokens(return_type)
}

/// Generate `ToolInfo` construction code, branching on annotations presence.
///
/// Per the plan: `ToolInfo` has NO `set_annotations()` method, so we must branch:
/// - With annotations: `ToolInfo::with_annotations(...)`
/// - Without annotations: `ToolInfo::new(...)`
pub fn generate_tool_info_code(
    tool_name: &str,
    description: &str,
    annotations: Option<&McpToolAnnotations>,
    ui: Option<&syn::Expr>,
) -> TokenStream {
    let base_info = match annotations {
        Some(ann) => {
            // Build annotations chain.
            let mut chain_parts = Vec::new();
            chain_parts.push(quote! { pmcp::types::ToolAnnotations::new() });

            if let Some(read_only) = ann.read_only {
                chain_parts.push(quote! { .with_read_only(#read_only) });
            }
            if let Some(destructive) = ann.destructive {
                chain_parts.push(quote! { .with_destructive(#destructive) });
            }
            if let Some(idempotent) = ann.idempotent {
                chain_parts.push(quote! { .with_idempotent(#idempotent) });
            }
            if let Some(open_world) = ann.open_world {
                chain_parts.push(quote! { .with_open_world(#open_world) });
            }

            quote! {
                {
                    let annotations = #(#chain_parts)*;
                    pmcp::types::ToolInfo::with_annotations(
                        #tool_name,
                        Some(#description.to_string()),
                        input_schema,
                        annotations,
                    )
                }
            }
        },
        None => {
            quote! {
                pmcp::types::ToolInfo::new(
                    #tool_name,
                    Some(#description.to_string()),
                    input_schema,
                )
            }
        },
    };

    // If ui attribute is present, set _meta for widget attachment.
    if let Some(ui_expr) = ui {
        quote! {
            {
                let mut info = #base_info;
                info._meta = Some(pmcp::types::ui::ToolUIMetadata::build_meta_map(
                    &#ui_expr.to_string()
                ));
                info
            }
        }
    } else {
        base_info
    }
}
