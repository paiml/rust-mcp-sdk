//! `#[mcp_server]` attribute macro expansion for impl blocks.
//!
//! Processes an impl block annotated with `#[mcp_server]`, collects all methods
//! marked with `#[mcp_tool(...)]` and `#[mcp_prompt(...)]`, and generates:
//!
//! 1. Per-tool handler structs implementing `ToolHandler` (using `Arc<ServerType>`)
//! 2. Per-prompt handler structs implementing `PromptHandler` (using `Arc<ServerType>`)
//! 3. An `impl McpServer for ServerType` with `register()` for bulk registration
//! 4. The original impl block preserved with `#[mcp_tool]`/`#[mcp_prompt]` attributes stripped
//!
//! # Example
//!
//! ```rust,ignore
//! #[mcp_server]
//! impl MyServer {
//!     #[mcp_tool(description = "Query database")]
//!     async fn query(&self, args: QueryArgs) -> Result<QueryResult> {
//!         self.db.execute(&args.sql).await
//!     }
//!
//!     #[mcp_prompt(description = "Generate a query")]
//!     async fn query_prompt(&self, args: PromptArgs) -> Result<GetPromptResult> {
//!         Ok(GetPromptResult::new(vec![...], None))
//!     }
//! }
//!
//! // Register all tools and prompts at once:
//! let builder = ServerBuilder::new()
//!     .mcp_server(my_server);
//! ```

use crate::mcp_common::{self, ParamRole};
use crate::mcp_prompt::McpPromptArgs;
use crate::mcp_resource::McpResourceArgs;
use crate::mcp_tool::{McpToolAnnotations, McpToolArgs};
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{FnArg, ImplItem, ImplItemFn, ItemImpl, ReturnType, Type};

use mcp_common::ParamSlot;

/// Information collected from a single `#[mcp_tool]`-annotated method.
struct ToolMethodInfo {
    /// The method identifier (e.g., `query`).
    method_name: syn::Ident,
    /// The resolved tool name (from `name = "..."` or method name).
    tool_name: String,
    /// Tool description (mandatory).
    description: String,
    /// Whether the method is async.
    is_async: bool,
    /// The args type if the method takes a typed input parameter.
    args_type: Option<Type>,
    /// Whether the method takes `RequestHandlerExtra`.
    has_extra: bool,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
    /// The return type of the method (full signature type).
    return_type: Option<Type>,
    /// MCP standard annotations.
    annotations: Option<McpToolAnnotations>,
    /// UI widget resource URI.
    ui: Option<syn::Expr>,
}

/// Information collected from a single `#[mcp_prompt]`-annotated method.
struct PromptMethodInfo {
    /// The method identifier (e.g., `query_builder`).
    method_name: syn::Ident,
    /// The resolved prompt name (from `name = "..."` or method name).
    prompt_name: String,
    /// Prompt description (mandatory).
    description: String,
    /// Whether the method is async.
    is_async: bool,
    /// The args type if the method takes a typed input parameter.
    args_type: Option<Type>,
    /// Whether the method takes `RequestHandlerExtra`.
    has_extra: bool,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
}

/// Information collected from a single `#[mcp_resource]`-annotated method.
struct ResourceMethodInfo {
    /// The method identifier.
    method_name: syn::Ident,
    /// The resolved resource name.
    resource_name: String,
    /// Resource description.
    description: String,
    /// URI or URI template.
    uri: String,
    /// MIME type.
    mime_type: String,
    /// Whether the method is async.
    is_async: bool,
    /// URI template variable parameter names (in order).
    uri_param_names: Vec<String>,
    /// Parameter order for correct call-site generation (skips &self).
    param_order: Vec<ParamSlot>,
}

/// Generic-parameter splits used by handler-struct codegen. Avoids
/// re-deriving the same lifetime/type parameter triplets in each per-method
/// helper.
struct HandlerGenerics<'a> {
    impl_gen_params: syn::ImplGenerics<'a>,
    ty_gen_params: syn::TypeGenerics<'a>,
    where_clause: Option<&'a syn::WhereClause>,
    handler_impl_params: syn::ImplGenerics<'a>,
    handler_where: Option<&'a syn::WhereClause>,
}

/// The `extra` / `_extra` parameter name used in generated `handle()`
/// signatures, depending on whether the user method takes `RequestHandlerExtra`.
fn extra_param_ident(has_extra: bool) -> syn::Ident {
    if has_extra {
        format_ident!("extra")
    } else {
        format_ident!("_extra")
    }
}

/// Build call-site argument tokens (`typed_args`, `extra`) in user-declared
/// parameter order for tool/prompt methods. Panics on State<T> (collected
/// against `#[mcp_server]` at parse time).
fn build_call_args_for_tool_or_prompt(param_order: &[ParamSlot]) -> Vec<TokenStream> {
    param_order
        .iter()
        .map(|slot| match slot {
            ParamSlot::Args => quote! { typed_args },
            ParamSlot::Extra => quote! { extra },
            ParamSlot::State => unreachable!("#[mcp_server] uses &self, not State<T>"),
        })
        .collect()
}

/// Generate the args-deserialization snippet for tool methods (or empty
/// tokens when the method takes no typed-args parameter).
fn generate_tool_args_deser(method_info: &ToolMethodInfo) -> TokenStream {
    let Some(ref at) = method_info.args_type else {
        return quote! {};
    };
    let tool_name_err = &method_info.tool_name;
    quote! {
        let typed_args: #at = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::tool_rejected(
                format!("Invalid arguments for tool '{}': {}", #tool_name_err, e),
                None,
            ))?;
    }
}

/// Generate the input-schema construction code for a tool method.
fn generate_tool_input_schema_code(method_info: &ToolMethodInfo) -> TokenStream {
    if let Some(ref at) = method_info.args_type {
        mcp_common::generate_input_schema_code(at)
    } else {
        mcp_common::generate_empty_schema_code()
    }
}

/// Generate one tool's handler struct + `ToolHandler` impl as a `TokenStream`.
fn generate_tool_handler_struct(
    method_info: &ToolMethodInfo,
    server_type: &syn::Type,
    generics: &HandlerGenerics<'_>,
) -> (syn::Ident, TokenStream) {
    let handler_name = format_ident!(
        "{}ToolHandler",
        method_info.method_name.to_string().to_upper_camel_case()
    );
    let method_ident = &method_info.method_name;
    let tool_name = &method_info.tool_name;
    let description = &method_info.description;

    let args_deser = generate_tool_args_deser(method_info);
    let call_args = build_call_args_for_tool_or_prompt(&method_info.param_order);
    let fn_call = if method_info.is_async {
        quote! { let result = self.server.#method_ident(#(#call_args),*).await?; }
    } else {
        quote! { let result = self.server.#method_ident(#(#call_args),*)?; }
    };
    let extra_param_name = extra_param_ident(method_info.has_extra);
    let input_schema_code = generate_tool_input_schema_code(method_info);
    let output_schema_code = generate_method_output_schema(method_info);
    let tool_info_code = crate::mcp_tool::generate_tool_info_code(
        tool_name,
        description,
        method_info.annotations.as_ref(),
        method_info.ui.as_ref(),
    );

    let handler_impl_params = &generics.handler_impl_params;
    let ty_gen_params = &generics.ty_gen_params;
    let handler_where = &generics.handler_where;

    let handler_struct = quote! {
        struct #handler_name #handler_impl_params #handler_where {
            server: std::sync::Arc<#server_type>,
        }

        #[pmcp::async_trait]
        impl #handler_impl_params pmcp::ToolHandler for #handler_name #ty_gen_params #handler_where {
            async fn handle(
                &self,
                args: serde_json::Value,
                #extra_param_name: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<serde_json::Value> {
                #args_deser
                #fn_call
                serde_json::to_value(result)
                    .map_err(|e| pmcp::Error::internal(
                        format!("Failed to serialize result: {}", e)
                    ))
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
    };
    (handler_name, handler_struct)
}

/// Generate the args-deserialization snippet for prompt methods.
fn generate_prompt_args_deser(prompt_info: &PromptMethodInfo) -> TokenStream {
    let Some(ref at) = prompt_info.args_type else {
        return quote! {};
    };
    let prompt_name_err = &prompt_info.prompt_name;
    quote! {
        let typed_args: #at = pmcp::server::typed_prompt::deserialize_prompt_args(args, #prompt_name_err)?;
    }
}

/// Generate the `metadata()` body for a prompt handler, branching on
/// whether the prompt has typed args.
fn generate_prompt_metadata_body(prompt_info: &PromptMethodInfo) -> TokenStream {
    let prompt_name = &prompt_info.prompt_name;
    let description = &prompt_info.description;
    let Some(ref at) = prompt_info.args_type else {
        return quote! {
            fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
                Some(pmcp::types::PromptInfo::new(#prompt_name)
                    .with_description(#description))
            }
        };
    };
    quote! {
        fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
            let mut info = pmcp::types::PromptInfo::new(#prompt_name)
                .with_description(#description);

            let schema = schemars::schema_for!(#at);
            let json_schema = serde_json::to_value(&schema).unwrap_or_default();
            let arguments = pmcp::server::typed_prompt::extract_prompt_arguments_from_schema(&json_schema);
            if !arguments.is_empty() {
                info = info.with_arguments(arguments);
            }
            Some(info)
        }
    }
}

/// Generate one prompt's handler struct + `PromptHandler` impl.
fn generate_prompt_handler_struct(
    prompt_info: &PromptMethodInfo,
    server_type: &syn::Type,
    generics: &HandlerGenerics<'_>,
) -> (syn::Ident, TokenStream) {
    let handler_name = format_ident!(
        "{}PromptHandler",
        prompt_info.method_name.to_string().to_upper_camel_case()
    );
    let method_ident = &prompt_info.method_name;

    let args_deser = generate_prompt_args_deser(prompt_info);
    let call_args = build_call_args_for_tool_or_prompt(&prompt_info.param_order);
    let fn_call = if prompt_info.is_async {
        quote! { self.server.#method_ident(#(#call_args),*).await }
    } else {
        quote! { self.server.#method_ident(#(#call_args),*) }
    };
    let extra_param_name = extra_param_ident(prompt_info.has_extra);
    let metadata_body = generate_prompt_metadata_body(prompt_info);

    let handler_impl_params = &generics.handler_impl_params;
    let ty_gen_params = &generics.ty_gen_params;
    let handler_where = &generics.handler_where;

    let handler_struct = quote! {
        struct #handler_name #handler_impl_params #handler_where {
            server: std::sync::Arc<#server_type>,
        }

        #[pmcp::async_trait]
        impl #handler_impl_params pmcp::PromptHandler for #handler_name #ty_gen_params #handler_where {
            async fn handle(
                &self,
                args: std::collections::HashMap<String, String>,
                #extra_param_name: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<pmcp::types::GetPromptResult> {
                #args_deser
                #fn_call
            }

            #metadata_body
        }
    };
    (handler_name, handler_struct)
}

/// Generate URI-parameter `let` bindings extracted from the URI template.
fn generate_uri_param_extractions(res_info: &ResourceMethodInfo) -> Vec<TokenStream> {
    let uri = &res_info.uri;
    res_info
        .uri_param_names
        .iter()
        .map(|name| {
            let ident = format_ident!("{}", name);
            quote! {
                let #ident = params.get(#name)
                    .ok_or_else(|| pmcp::Error::validation(format!(
                        "Missing URI parameter '{}' for resource '{}'", #name, #uri
                    )))?
                    .clone();
            }
        })
        .collect()
}

/// Build call-site argument tokens for resource methods (URI vars + extra).
fn build_call_args_for_resource(res_info: &ResourceMethodInfo) -> Vec<TokenStream> {
    let mut uri_var_idx = 0;
    res_info
        .param_order
        .iter()
        .map(|slot| match slot {
            ParamSlot::Args => {
                let ident = format_ident!("{}", res_info.uri_param_names[uri_var_idx]);
                uri_var_idx += 1;
                quote! { #ident }
            },
            ParamSlot::Extra => quote! { _context.extra.clone() },
            ParamSlot::State => unreachable!("#[mcp_server] uses &self, not State<T>"),
        })
        .collect()
}

/// Generate one resource's handler struct + `DynamicResourceProvider` impl.
fn generate_resource_handler_struct(
    res_info: &ResourceMethodInfo,
    server_type: &syn::Type,
    generics: &HandlerGenerics<'_>,
) -> (syn::Ident, TokenStream) {
    let handler_name = format_ident!(
        "{}ResourceHandler",
        res_info.method_name.to_string().to_upper_camel_case()
    );
    let method_ident = &res_info.method_name;
    let uri = &res_info.uri;
    let resource_name = &res_info.resource_name;
    let description = &res_info.description;
    let mime_type = &res_info.mime_type;

    let uri_param_extraction = generate_uri_param_extractions(res_info);
    let call_args = build_call_args_for_resource(res_info);
    let fn_call = if res_info.is_async {
        quote! { let content_str: String = self.server.#method_ident(#(#call_args),*).await?; }
    } else {
        quote! { let content_str: String = self.server.#method_ident(#(#call_args),*)?; }
    };

    let handler_impl_params = &generics.handler_impl_params;
    let ty_gen_params = &generics.ty_gen_params;
    let handler_where = &generics.handler_where;

    let handler_struct = quote! {
        struct #handler_name #handler_impl_params #handler_where {
            server: std::sync::Arc<#server_type>,
        }

        #[pmcp::async_trait]
        impl #handler_impl_params pmcp::server::dynamic_resources::DynamicResourceProvider
            for #handler_name #ty_gen_params #handler_where
        {
            fn templates(&self) -> Vec<pmcp::types::ResourceTemplate> {
                let mut tmpl = pmcp::types::ResourceTemplate::new(#uri, #resource_name)
                    .with_description(#description);
                tmpl.mime_type = Some(#mime_type.to_string());
                vec![tmpl]
            }

            async fn fetch(
                &self,
                _uri: &str,
                params: pmcp::server::dynamic_resources::UriParams,
                _context: pmcp::server::dynamic_resources::RequestContext,
            ) -> pmcp::Result<pmcp::types::ReadResourceResult> {
                #(#uri_param_extraction)*
                #fn_call
                Ok(pmcp::types::ReadResourceResult::new(
                    vec![pmcp::types::Content::text(content_str)]
                ))
            }
        }
    };
    (handler_name, handler_struct)
}

/// Build the `pmcp::ResourceCollection::new()...add_dynamic_provider...`
/// chain emitted into the `register()` body when resources exist.
fn build_resource_registration(resource_provider_names: &[syn::Ident]) -> TokenStream {
    if resource_provider_names.is_empty() {
        return quote! {};
    }
    let provider_adds: Vec<TokenStream> = resource_provider_names
        .iter()
        .map(|name| {
            quote! {
                .add_dynamic_provider(std::sync::Arc::new(#name { server: shared.clone() }))
            }
        })
        .collect();
    quote! {
        let resource_collection = pmcp::ResourceCollection::new()
            #(#provider_adds)*;
        builder = builder.resources(resource_collection);
    }
}

/// Validate that at least one mcp-annotated method exists.
fn require_at_least_one_method(
    input: &ItemImpl,
    tools: &[ToolMethodInfo],
    prompts: &[PromptMethodInfo],
    resources: &[ResourceMethodInfo],
) -> syn::Result<()> {
    if !tools.is_empty() || !prompts.is_empty() || !resources.is_empty() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        input,
        "No methods marked with #[mcp_tool], #[mcp_prompt], or #[mcp_resource] found in impl block",
    ))
}

/// Expand `#[mcp_server]` attribute macro on an impl block.
///
/// The `args` token stream is currently unused (reserved for future options).
/// The `input` is the parsed `ItemImpl` block containing `#[mcp_tool]` methods.
///
/// Refactored in 75-01 Task 1b-A (P1): per-section codegen extracted into
/// [`generate_tool_handler_struct`], [`generate_prompt_handler_struct`],
/// [`generate_resource_handler_struct`], [`build_resource_registration`],
/// [`require_at_least_one_method`], and a `HandlerGenerics` carrier. The
/// orchestrator now reads as a sequential pipeline of named codegen calls.
pub fn expand_mcp_server(_args: TokenStream, mut input: ItemImpl) -> syn::Result<TokenStream> {
    let tool_methods = collect_tool_methods(&input)?;
    let prompt_methods = collect_prompt_methods(&input)?;
    let resource_methods = collect_resource_methods(&input)?;
    require_at_least_one_method(&input, &tool_methods, &prompt_methods, &resource_methods)?;

    // Extract the server type (clone to avoid borrow conflict with later
    // mutable strip operation).
    let server_type = input.self_ty.clone();

    // Extract generics (D-25 generic-server support).
    let impl_generics = input.generics.clone();
    let handler_generics = mcp_common::add_async_trait_bounds(impl_generics.clone());
    let (impl_gen_params, ty_gen_params, where_clause) = impl_generics.split_for_impl();
    let (handler_impl_params, _handler_ty_params, handler_where) =
        handler_generics.split_for_impl();
    let generics = HandlerGenerics {
        impl_gen_params: impl_gen_params.clone(),
        ty_gen_params,
        where_clause,
        handler_impl_params,
        handler_where,
    };

    let mut handler_structs: Vec<TokenStream> = Vec::new();
    let mut register_lines: Vec<TokenStream> = Vec::new();

    for method_info in &tool_methods {
        let (handler_name, handler_struct) =
            generate_tool_handler_struct(method_info, &server_type, &generics);
        handler_structs.push(handler_struct);
        let tool_name = &method_info.tool_name;
        register_lines.push(quote! {
            builder = builder.tool(#tool_name, #handler_name { server: shared.clone() });
        });
    }

    for prompt_info in &prompt_methods {
        let (handler_name, handler_struct) =
            generate_prompt_handler_struct(prompt_info, &server_type, &generics);
        handler_structs.push(handler_struct);
        let prompt_name = &prompt_info.prompt_name;
        register_lines.push(quote! {
            builder = builder.prompt(#prompt_name, #handler_name { server: shared.clone() });
        });
    }

    let mut resource_provider_names = Vec::new();
    for res_info in &resource_methods {
        let (handler_name, handler_struct) =
            generate_resource_handler_struct(res_info, &server_type, &generics);
        handler_structs.push(handler_struct);
        resource_provider_names.push(handler_name);
    }

    strip_mcp_attrs(&mut input);

    let resource_registration = build_resource_registration(&resource_provider_names);
    let impl_gen_params = &generics.impl_gen_params;
    let where_clause = &generics.where_clause;
    let mcp_server_impl = quote! {
        impl #impl_gen_params pmcp::McpServer for #server_type #where_clause {
            fn register(self, mut builder: pmcp::ServerBuilder) -> pmcp::ServerBuilder {
                let shared = std::sync::Arc::new(self);
                #(#register_lines)*
                #resource_registration
                builder
            }
        }
    };

    let expanded = quote! {
        #input

        #(#handler_structs)*

        #mcp_server_impl
    };

    Ok(expanded)
}

/// Locate a method-level attribute by ident name (`mcp_tool`, `mcp_prompt`,
/// `mcp_resource`). Returns the attribute reference if present.
fn find_mcp_attr<'a>(method: &'a ImplItemFn, name: &str) -> Option<&'a syn::Attribute> {
    method.attrs.iter().find(|a| a.path().is_ident(name))
}

/// Validate that a method takes `&self` as its first parameter.
fn require_self_receiver(method: &ImplItemFn, has_self: bool) -> syn::Result<()> {
    if has_self {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        &method.sig.ident,
        "#[mcp_server] methods must take &self as the first parameter",
    ))
}

/// Push a single argument-typed parameter slot, rejecting duplicates.
fn push_arg_slot(
    param: &FnArg,
    ty: Type,
    args_type: &mut Option<Type>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if args_type.is_some() {
        return Err(syn::Error::new_spanned(
            param,
            "#[mcp_server] methods can have at most one args parameter",
        ));
    }
    *args_type = Some(ty);
    param_order.push(ParamSlot::Args);
    Ok(())
}

/// Push the `RequestHandlerExtra` slot, rejecting duplicates.
fn push_extra_slot(
    param: &FnArg,
    has_extra: &mut bool,
    param_order: &mut Vec<ParamSlot>,
    err_msg: &'static str,
) -> syn::Result<()> {
    if *has_extra {
        return Err(syn::Error::new_spanned(param, err_msg));
    }
    *has_extra = true;
    param_order.push(ParamSlot::Extra);
    Ok(())
}

/// Reject `State<T>` parameters in `#[mcp_server]` impls.
fn reject_state_param(param: &FnArg) -> syn::Error {
    syn::Error::new_spanned(
        param,
        "#[mcp_server] methods use &self for state access, not State<T>",
    )
}

/// Classify the parameters of a tool/prompt method into `(args_type,
/// has_extra, has_self, param_order)` slots.
fn classify_params_for_tool_or_prompt(
    method: &ImplItemFn,
    extra_err_msg: &'static str,
) -> syn::Result<(Option<Type>, bool, bool, Vec<ParamSlot>)> {
    let mut args_type: Option<Type> = None;
    let mut has_extra = false;
    let mut has_self = false;
    let mut param_order: Vec<ParamSlot> = Vec::new();

    for param in &method.sig.inputs {
        let role = mcp_common::classify_param(param)?;
        match role {
            ParamRole::SelfRef => has_self = true,
            ParamRole::Args(ty) => push_arg_slot(param, ty, &mut args_type, &mut param_order)?,
            ParamRole::Extra => {
                push_extra_slot(param, &mut has_extra, &mut param_order, extra_err_msg)?;
            },
            ParamRole::State { .. } => return Err(reject_state_param(param)),
        }
    }
    Ok((args_type, has_extra, has_self, param_order))
}

/// Build a `ToolMethodInfo` from the classified parameters + parsed attrs.
fn build_tool_method_info(
    method: &ImplItemFn,
    macro_args: McpToolArgs,
    args_type: Option<Type>,
    has_extra: bool,
    param_order: Vec<ParamSlot>,
) -> ToolMethodInfo {
    let tool_name = macro_args
        .name
        .unwrap_or_else(|| method.sig.ident.to_string());
    let return_type = match &method.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some(ty.as_ref().clone()),
    };
    ToolMethodInfo {
        method_name: method.sig.ident.clone(),
        tool_name,
        description: macro_args.description,
        is_async: method.sig.asyncness.is_some(),
        args_type,
        has_extra,
        param_order,
        return_type,
        annotations: macro_args.annotations,
        ui: macro_args.ui,
    }
}

/// Collect information from all `#[mcp_tool(...)]`-annotated methods in the impl block.
///
/// Refactored in 75-01 Task 1b-A (P1): extracted [`find_mcp_attr`],
/// [`classify_params_for_tool_or_prompt`], [`require_self_receiver`], and
/// [`build_tool_method_info`] so the loop body is a thin pipeline of
/// resolved values.
fn collect_tool_methods(impl_block: &ItemImpl) -> syn::Result<Vec<ToolMethodInfo>> {
    let mut methods = Vec::new();
    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let Some(attr) = find_mcp_attr(method, "mcp_tool") else {
            continue;
        };
        let macro_args = parse_mcp_tool_attr(attr, method)?;
        let (args_type, has_extra, has_self, param_order) = classify_params_for_tool_or_prompt(
            method,
            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
        )?;
        require_self_receiver(method, has_self)?;
        methods.push(build_tool_method_info(
            method,
            macro_args,
            args_type,
            has_extra,
            param_order,
        ));
    }
    Ok(methods)
}

/// Parse `#[mcp_tool(...)]` on an impl-block method into `McpToolArgs`.
///
/// Delegates to `mcp_common::resolve_tool_args` so the rustdoc-fallback
/// and missing-description-error logic matches the standalone parse site.
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        // `#[mcp_tool]` with no parens — fall through to the resolver with
        // an empty token stream. The resolver will consult rustdoc.
        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
        // `#[mcp_tool = "..."]` is an orthogonal syntax error — keep the
        // pre-existing early-return.
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_tool requires parenthesized arguments: #[mcp_tool(description = \"...\")]",
            ));
        },
    };

    let nested_metas =
        crate::mcp_common::resolve_tool_args(tokens, &method.attrs, &method.sig.ident)?;

    McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Generate output schema code from a method's return type.
///
/// Returns `None` (as tokens) for `Result<Value>` or no return type.
/// Returns `Some(schema)` (as tokens) for `Result<TypedStruct>`.
fn generate_method_output_schema(method_info: &ToolMethodInfo) -> TokenStream {
    let Some(ref return_type) = method_info.return_type else {
        return quote! { None };
    };

    if let Some(ok_type) = mcp_common::extract_result_ok_type(return_type) {
        if mcp_common::is_value_type(&ok_type) {
            quote! { None }
        } else {
            mcp_common::generate_output_schema_code(&ok_type)
        }
    } else {
        quote! { None }
    }
}

/// Build a `PromptMethodInfo` from the classified parameters + parsed attrs.
fn build_prompt_method_info(
    method: &ImplItemFn,
    macro_args: McpPromptArgs,
    args_type: Option<Type>,
    has_extra: bool,
    param_order: Vec<ParamSlot>,
) -> PromptMethodInfo {
    let prompt_name = macro_args
        .name
        .unwrap_or_else(|| method.sig.ident.to_string());
    PromptMethodInfo {
        method_name: method.sig.ident.clone(),
        prompt_name,
        description: macro_args.description,
        is_async: method.sig.asyncness.is_some(),
        args_type,
        has_extra,
        param_order,
    }
}

/// Collect information from all `#[mcp_prompt(...)]`-annotated methods in the impl block.
///
/// Refactored in 75-01 Task 1b-A (P1): same shape as
/// [`collect_tool_methods`]; shares [`find_mcp_attr`],
/// [`classify_params_for_tool_or_prompt`], [`require_self_receiver`].
fn collect_prompt_methods(impl_block: &ItemImpl) -> syn::Result<Vec<PromptMethodInfo>> {
    let mut methods = Vec::new();
    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let Some(attr) = find_mcp_attr(method, "mcp_prompt") else {
            continue;
        };
        let macro_args = parse_mcp_prompt_attr(attr, method)?;
        let (args_type, has_extra, has_self, param_order) = classify_params_for_tool_or_prompt(
            method,
            "#[mcp_server] methods can have at most one RequestHandlerExtra parameter",
        )?;
        require_self_receiver(method, has_self)?;
        methods.push(build_prompt_method_info(
            method,
            macro_args,
            args_type,
            has_extra,
            param_order,
        ));
    }
    Ok(methods)
}

/// Parse `#[mcp_prompt(...)]` attribute into `McpPromptArgs`.
fn parse_mcp_prompt_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpPromptArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_prompt requires at least `description = \"...\"` attribute",
            ));
        },
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_prompt requires parenthesized arguments: #[mcp_prompt(description = \"...\")]",
            ));
        },
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpPromptArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Match a resource-arg parameter against the URI template variables.
/// On a name match, push it as a URI param; otherwise return a typed error.
fn classify_resource_string_arg(
    param: &FnArg,
    template_vars: &[String],
    uri_param_names: &mut Vec<String>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    if let FnArg::Typed(pat_type) = param {
        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
            let name = pat_ident.ident.to_string();
            if template_vars.contains(&name) {
                uri_param_names.push(name);
                param_order.push(ParamSlot::Args);
                return Ok(());
            }
        }
    }
    Err(syn::Error::new_spanned(
        param,
        format!(
            "Parameter name must match a URI template variable. Available: {:?}",
            template_vars
        ),
    ))
}

/// Process a single resource method parameter: classify role, push slot.
fn process_resource_param(
    param: &FnArg,
    template_vars: &[String],
    has_self: &mut bool,
    has_extra: &mut bool,
    uri_param_names: &mut Vec<String>,
    param_order: &mut Vec<ParamSlot>,
) -> syn::Result<()> {
    let role = mcp_common::classify_param(param)?;
    match role {
        ParamRole::SelfRef => {
            *has_self = true;
            Ok(())
        },
        ParamRole::Args(ty) => {
            if !mcp_common::type_name_matches(&ty, "String") {
                return Err(syn::Error::new_spanned(
                    param,
                    "Resource function parameters (URI template variables) must be String type",
                ));
            }
            classify_resource_string_arg(param, template_vars, uri_param_names, param_order)
        },
        ParamRole::Extra => push_extra_slot(
            param,
            has_extra,
            param_order,
            "#[mcp_resource] methods can have at most one RequestHandlerExtra parameter",
        ),
        ParamRole::State { .. } => Err(reject_state_param(param)),
    }
}

/// Classify all parameters of a `#[mcp_resource]` method, returning the
/// `(uri_param_names, has_extra, has_self, param_order)` tuple.
fn classify_params_for_resource(
    method: &ImplItemFn,
    template_vars: &[String],
) -> syn::Result<(Vec<String>, bool, bool, Vec<ParamSlot>)> {
    let mut has_self = false;
    let mut has_extra = false;
    let mut uri_param_names: Vec<String> = Vec::new();
    let mut param_order: Vec<ParamSlot> = Vec::new();
    for param in &method.sig.inputs {
        process_resource_param(
            param,
            template_vars,
            &mut has_self,
            &mut has_extra,
            &mut uri_param_names,
            &mut param_order,
        )?;
    }
    Ok((uri_param_names, has_extra, has_self, param_order))
}

/// Verify all URI template variables are covered by function parameters.
fn require_template_var_coverage(
    method: &ImplItemFn,
    template_vars: &[String],
    uri_param_names: &[String],
) -> syn::Result<()> {
    let uncovered: Vec<&str> = template_vars
        .iter()
        .filter(|v| !uri_param_names.contains(v))
        .map(String::as_str)
        .collect();
    if uncovered.is_empty() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        &method.sig.ident,
        format!(
            "URI template variables not covered by function parameters: {:?}",
            uncovered
        ),
    ))
}

/// Build a `ResourceMethodInfo` from the parsed attribute + classified params.
fn build_resource_method_info(
    method: &ImplItemFn,
    macro_args: McpResourceArgs,
    uri: String,
    uri_param_names: Vec<String>,
    param_order: Vec<ParamSlot>,
) -> ResourceMethodInfo {
    let resource_name = macro_args
        .name
        .unwrap_or_else(|| method.sig.ident.to_string());
    let mime_type = macro_args
        .mime_type
        .unwrap_or_else(|| "text/plain".to_string());
    ResourceMethodInfo {
        method_name: method.sig.ident.clone(),
        resource_name,
        description: macro_args.description,
        uri,
        mime_type,
        is_async: method.sig.asyncness.is_some(),
        uri_param_names,
        param_order,
    }
}

/// Process a single `#[mcp_resource]` method's `ItemImpl::Fn` and produce
/// either a `ResourceMethodInfo` or an error.
fn process_resource_method(method: &ImplItemFn) -> syn::Result<Option<ResourceMethodInfo>> {
    let Some(attr) = find_mcp_attr(method, "mcp_resource") else {
        return Ok(None);
    };
    let macro_args = parse_mcp_resource_attr(attr, method)?;
    let uri = macro_args.uri.clone();
    let template_vars = crate::mcp_resource::extract_template_vars(&uri)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e))?;
    let (uri_param_names, _has_extra, has_self, param_order) =
        classify_params_for_resource(method, &template_vars)?;
    require_self_receiver(method, has_self)?;
    require_template_var_coverage(method, &template_vars, &uri_param_names)?;
    Ok(Some(build_resource_method_info(
        method,
        macro_args,
        uri,
        uri_param_names,
        param_order,
    )))
}

/// Collect information from all `#[mcp_resource(...)]`-annotated methods in the impl block.
///
/// Refactored in 75-01 Task 1b-A (P1, escapee candidate per addendum
/// Rule 3): the original cog-80 implementation was decomposed into
/// [`process_resource_method`], [`classify_params_for_resource`],
/// [`process_resource_param`], [`classify_resource_string_arg`],
/// [`require_template_var_coverage`], and [`build_resource_method_info`]
/// so the outer loop is a thin filter-map.
fn collect_resource_methods(impl_block: &ItemImpl) -> syn::Result<Vec<ResourceMethodInfo>> {
    let mut methods = Vec::new();
    for item in &impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        if let Some(info) = process_resource_method(method)? {
            methods.push(info);
        }
    }
    Ok(methods)
}

/// Parse `#[mcp_resource(...)]` attribute into `McpResourceArgs`.
fn parse_mcp_resource_attr(
    attr: &syn::Attribute,
    method: &ImplItemFn,
) -> syn::Result<McpResourceArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_resource requires `uri = \"...\"` and `description = \"...\"` attributes",
            ));
        },
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_resource requires parenthesized arguments: #[mcp_resource(uri = \"...\", description = \"...\")]",
            ));
        },
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpResourceArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}

/// Strip `#[mcp_tool]`, `#[mcp_prompt]`, and `#[mcp_resource]` attributes from all methods.
fn strip_mcp_attrs(input: &mut ItemImpl) {
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("mcp_tool")
                    && !attr.path().is_ident("mcp_prompt")
                    && !attr.path().is_ident("mcp_resource")
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_collect_tool_methods_finds_annotated() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query data")]
                async fn query(&self, args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }

                fn helper(&self) {}

                #[mcp_tool(description = "Clear cache")]
                async fn clear_cache(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method_name, "query");
        assert_eq!(methods[0].tool_name, "query");
        assert_eq!(methods[0].description, "Query data");
        assert!(methods[0].is_async);
        assert!(methods[0].args_type.is_some());
        assert!(!methods[0].has_extra);

        assert_eq!(methods[1].method_name, "clear_cache");
        assert_eq!(methods[1].tool_name, "clear_cache");
        assert!(!methods[1].has_extra);
        assert!(methods[1].args_type.is_none());
    }

    #[test]
    fn test_collect_tool_methods_empty_errors() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                fn helper(&self) {}
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert!(methods.is_empty());
    }

    #[test]
    fn test_collect_tool_methods_with_extra() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Export data")]
                async fn export(&self, args: ExportArgs, extra: RequestHandlerExtra) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 1);
        assert!(methods[0].has_extra);
        assert!(methods[0].args_type.is_some());
    }

    #[test]
    fn test_collect_tool_methods_state_rejected() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Bad tool")]
                async fn bad(&self, db: State<Database>) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let result = collect_tool_methods(&impl_block);
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("use &self for state access"),
                    "Expected State<T> rejection error, got: {}",
                    err_msg
                );
            },
            Ok(_) => panic!("Expected error for State<T> in #[mcp_server] method"),
        }
    }

    #[test]
    fn test_collect_tool_methods_custom_name() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query data", name = "custom_query")]
                async fn query(&self, args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods[0].tool_name, "custom_query");
    }

    #[test]
    fn test_strip_mcp_attrs() {
        let mut impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Query")]
                #[doc = "A query method"]
                async fn query(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }

                #[mcp_prompt(description = "Prompt")]
                async fn my_prompt(&self) -> Result<GetPromptResult> {
                    unimplemented!()
                }
            }
        };

        strip_mcp_attrs(&mut impl_block);

        // The #[mcp_tool] attr should be removed, but #[doc] preserved.
        if let ImplItem::Fn(method) = &impl_block.items[0] {
            assert!(
                !method.attrs.iter().any(|a| a.path().is_ident("mcp_tool")),
                "mcp_tool attribute should be stripped"
            );
            assert!(
                method.attrs.iter().any(|a| a.path().is_ident("doc")),
                "doc attribute should be preserved"
            );
        } else {
            panic!("Expected ImplItem::Fn");
        }

        // The #[mcp_prompt] attr should also be removed.
        if let ImplItem::Fn(method) = &impl_block.items[1] {
            assert!(
                !method.attrs.iter().any(|a| a.path().is_ident("mcp_prompt")),
                "mcp_prompt attribute should be stripped"
            );
        } else {
            panic!("Expected ImplItem::Fn");
        }
    }

    #[test]
    fn test_collect_tool_methods_no_self_errors() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "No self")]
                async fn no_self(args: QueryArgs) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let result = collect_tool_methods(&impl_block);
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("must take &self"),
                    "Expected &self requirement error, got: {}",
                    err_msg
                );
            },
            Ok(_) => panic!("Expected error for method without &self"),
        }
    }

    #[test]
    fn test_collect_tool_methods_sync() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_tool(description = "Get version")]
                fn version(&self) -> Result<Value> {
                    Ok(serde_json::json!({}))
                }
            }
        };

        let methods = collect_tool_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 1);
        assert!(!methods[0].is_async);
    }

    #[test]
    fn test_collect_prompt_methods_finds_annotated() {
        let impl_block: ItemImpl = parse_quote! {
            impl MyServer {
                #[mcp_prompt(description = "Generate query")]
                async fn query_builder(&self, args: QueryArgs) -> Result<GetPromptResult> {
                    unimplemented!()
                }

                fn helper(&self) {}

                #[mcp_prompt(description = "System status")]
                async fn status(&self) -> Result<GetPromptResult> {
                    unimplemented!()
                }
            }
        };

        let methods = collect_prompt_methods(&impl_block).unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method_name, "query_builder");
        assert_eq!(methods[0].prompt_name, "query_builder");
        assert_eq!(methods[0].description, "Generate query");
        assert!(methods[0].is_async);
        assert!(methods[0].args_type.is_some());
        assert!(!methods[0].has_extra);

        assert_eq!(methods[1].method_name, "status");
        assert_eq!(methods[1].prompt_name, "status");
        assert_eq!(methods[1].description, "System status");
        assert!(methods[1].args_type.is_none());
    }
}
