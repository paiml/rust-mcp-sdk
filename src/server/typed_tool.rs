//! Type-safe tool implementations with automatic schema generation.
//!
//! This module provides the native (non-WASM) typed tool implementations with full
//! input and output typing support. For WASM environments, see `wasm_typed_tool.rs`
//! which provides input typing only due to async constraints.

use crate::types::{CallToolResult, ToolAnnotations, ToolExecution, ToolInfo};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::cancellation::RequestHandlerExtra;
use super::{ToolHandler, ToolOutput};

#[cfg(feature = "schema-generation")]
use schemars::JsonSchema;

/// A stored, type-erased `garde` entry point for `T`.
///
/// Exists as an alias so the field it types stays readable and so the
/// `clippy::type_complexity` shape is named once.
#[cfg(feature = "validation")]
type GardeValidator<T> = Box<dyn Fn(&T) -> std::result::Result<(), garde::Report> + Send + Sync>;

/// A typed tool implementation with automatic schema generation and validation.
pub struct TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,
    execution: Option<ToolExecution>,
    handler: F,
    /// The `garde` validator, populated ONLY by [`TypedTool::new_validated`] and
    /// [`TypedTool::new_validated_with_schema`]. Every other constructor leaves it
    /// `None`, which is what keeps this addition behaviour-preserving for every
    /// existing `TypedTool<T>` — including one whose `T` happens to implement
    /// `garde::Validate`.
    #[cfg(feature = "validation")]
    validator: Option<GardeValidator<T>>,
    _phantom: PhantomData<T>,
}

impl<T, F> fmt::Debug for TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("annotations", &self.annotations)
            .finish()
    }
}

impl<T, F> TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
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
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            #[cfg(feature = "validation")]
            validator: None,
            _phantom: PhantomData,
        }
    }

    /// Create a new typed tool with a manually provided schema.
    pub fn new_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            #[cfg(feature = "validation")]
            validator: None,
            _phantom: PhantomData,
        }
    }

    /// Create a new typed tool that runs `T`'s `garde` field rules after
    /// deserialization (Phase 128, E3).
    ///
    /// # Why the bound is on the constructor
    ///
    /// `T: garde::Validate<Context = ()>` is declared HERE and nowhere else — not
    /// on the `struct`, not on the inherent `impl`, and not on the
    /// [`ToolHandler`] impl. That is deliberate and load-bearing: adding it to the
    /// type would be a breaking change for every existing `TypedTool<T>` whose `T`
    /// does not implement `garde::Validate`, and stable Rust has no specialization
    /// that would make the bound conditional. Confining it to the constructor
    /// makes field validation opt-in per tool at zero cost to every other tool.
    ///
    /// # Scope boundary — a non-unit `Context`
    ///
    /// Only `Context = ()` is supported here, because `garde`'s argument-free
    /// `Validate::validate` requires `Self::Context: Default`. A rule that needs
    /// request context or server-side state is out of scope for this entry point;
    /// validate it inside the handler body instead.
    ///
    /// # Refusal shape
    ///
    /// A violation short-circuits the handler with [`crate::Error::Validation`]
    /// rendered value-free — see `render_garde_refusal` for the two residual leak
    /// surfaces (`#[garde(custom(..))]` messages and `#[garde(dive)]` map keys).
    /// A DESERIALIZATION failure on a tool built through this constructor is also
    /// redacted, which is the one behavioural difference from the plain
    /// constructors; see `deserialize_args`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(all(feature = "validation", feature = "schema-generation"))] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    /// use garde::Validate;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema, Validate)]
    /// struct CreateArgs {
    ///     #[garde(length(min = 1, max = 64))]
    ///     title: String,
    /// }
    ///
    /// let tool = TypedTool::new_validated("create", |args: CreateArgs, _extra| {
    ///     Box::pin(async move { Ok(serde_json::json!({"title": args.title})) })
    /// });
    /// # }
    /// ```
    #[cfg(all(feature = "validation", feature = "schema-generation"))]
    pub fn new_validated(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema + garde::Validate<Context = ()>,
    {
        Self::new(name, handler).with_garde_validator()
    }

    /// [`TypedTool::new_validated`] with a manually provided schema.
    ///
    /// The `validation`-gated sibling of [`TypedTool::new_with_schema`], for builds
    /// that do not enable `schema-generation`. The bound story is identical: see
    /// [`TypedTool::new_validated`].
    #[cfg(feature = "validation")]
    pub fn new_validated_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self
    where
        T: garde::Validate<Context = ()>,
    {
        Self::new_with_schema(name, schema, handler).with_garde_validator()
    }

    /// Store the `garde` entry point. Private: the only way to reach it is a
    /// `new_validated*` constructor, so the bound cannot leak onto the type.
    #[cfg(feature = "validation")]
    fn with_garde_validator(mut self) -> Self
    where
        T: garde::Validate<Context = ()>,
    {
        self.validator = Some(Box::new(garde::Validate::validate));
        self
    }

    /// Deserialize `args` into `T`.
    ///
    /// # The message shape is asymmetric BY DESIGN (T-128-17a)
    ///
    /// `serde_json::Error`'s `Display` quotes the offending input for several
    /// failure kinds (an invalid string, an unknown enum variant, a type
    /// mismatch), and this step runs BEFORE `garde`. A tool built through
    /// [`TypedTool::new_validated`] advertises a value-free refusal, so on that
    /// path the error is redacted down to the tool name plus the failure's
    /// CLASSIFICATION. A tool built through any other constructor keeps today's
    /// message byte-identical, so no existing user's error text changes — the
    /// narrowing is what keeps E3 additive.
    ///
    #[cfg(feature = "validation")]
    fn deserialize_args(&self, args: Value) -> Result<T> {
        serde_json::from_value(args).map_err(|e| {
            if self.validator.is_some() {
                redacted_deserialize_error(&self.name, &e)
            } else {
                legacy_deserialize_error(&self.name, &e)
            }
        })
    }

    /// Deserialize `args` into `T`. Without the `validation` feature no validated
    /// constructor exists, so today's message is the only one.
    #[cfg(not(feature = "validation"))]
    fn deserialize_args(&self, args: Value) -> Result<T> {
        serde_json::from_value(args).map_err(|e| legacy_deserialize_error(&self.name, &e))
    }

    /// Run the stored `garde` validator, if this tool has one.
    ///
    /// A tool built through a plain constructor stores none, and this is a no-op.
    #[cfg(feature = "validation")]
    fn run_garde(&self, typed_args: &T) -> Result<()> {
        match self.validator.as_deref() {
            Some(validate) => {
                validate(typed_args).map_err(|report| render_garde_refusal(&self.name, &report))
            },
            None => Ok(()),
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// Annotations provide behavioral hints to AI clients about how this tool
    /// should be used (read-only, destructive, idempotent, etc.).
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use pmcp::types::ToolAnnotations;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct DeleteArgs {
    ///     id: String,
    /// }
    ///
    /// let tool = TypedTool::new("delete_record", |args: DeleteArgs, _extra| {
    ///     Box::pin(async move {
    ///         Ok(serde_json::json!({"deleted": true}))
    ///     })
    /// })
    /// .with_description("Permanently delete a record")
    /// .with_annotations(
    ///     ToolAnnotations::new()
    ///         .with_read_only(false)
    ///         .with_destructive(true)
    ///         .with_idempotent(true)
    /// );
    /// # }
    /// ```
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_read_only(true))`
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    ///
    /// Sets `readOnlyHint: false` and `destructiveHint: true`.
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_idempotent(true))`
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    ///
    /// Equivalent to `.with_annotations(ToolAnnotations::new().with_open_world(true))`
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }

    /// Associate this tool with a UI resource (MCP Apps Extension).
    ///
    /// This sets the nested `_meta.ui.resourceUri` field and the `openai/outputTemplate`
    /// alias in the tool's `_meta`, allowing both MCP and `ChatGPT` hosts to display
    /// an interactive UI when this tool is invoked.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct AnalyzeArgs {
    ///     query: String,
    /// }
    ///
    /// let tool = TypedTool::new("analyze_sales", |args: AnalyzeArgs, _extra| {
    ///     Box::pin(async move {
    ///         Ok(serde_json::json!({"result": "data"}))
    ///     })
    /// })
    /// .with_description("Analyze sales data")
    /// .with_ui("ui://charts/sales");  // Associate with UI resource
    /// # }
    /// ```
    pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(ui_resource_uri.into());
        self
    }

    /// Declare execution metadata for this tool (MCP 2025-11-25).
    ///
    /// Use this to advertise task support so clients know whether to send
    /// a `task` field in `tools/call` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedTool;
    /// use pmcp::types::{ToolExecution, TaskSupport};
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct AnalyzeArgs { region: String }
    ///
    /// let tool = TypedTool::new("analyze", |args: AnalyzeArgs, _extra| {
    ///     Box::pin(async move {
    ///         Ok(serde_json::json!({"taskId": "t-1", "status": "working"}))
    ///     })
    /// })
    /// .with_description("Long-running analysis")
    /// .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));
    /// # }
    /// ```
    pub fn with_execution(mut self, execution: ToolExecution) -> Self {
        self.execution = Some(execution);
        self
    }
}

#[async_trait]
impl<T, F> ToolHandler for TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Deserialize the arguments, and validate them ONLY when this tool was built
        // through `TypedTool::new_validated` / `new_validated_with_schema` — every
        // other constructor stores no validator and this step is deserialization
        // alone (T-128-18: the comment this replaced claimed validation that never
        // happened).
        let typed_args: T = self.deserialize_args(args)?;
        #[cfg(feature = "validation")]
        self.run_garde(&typed_args)?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args, extra).await
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo {
            name: self.name.clone(),
            title: None,
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: self.annotations.clone(),
            icons: None,
            _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
            execution: self.execution.clone(),
        })
    }
}

/// Today's deserialization refusal, preserved byte-for-byte for every tool built
/// through a NON-validated constructor.
///
/// `serde_json::Error`'s `Display` can quote the caller's input. That is a known
/// leak (T-128-17a) which is fixed only on the validated path, so that no existing
/// consumer's error text changes.
fn legacy_deserialize_error(tool: &str, e: &serde_json::Error) -> Error {
    Error::Validation(format!("Invalid arguments for tool '{}': {}", tool, e))
}

/// The REDACTED deserialization refusal a VALIDATED tool returns (T-128-17a).
///
/// `serde_json::Error`'s `Display` quotes the caller's input for several failure
/// kinds — an invalid string, an unknown enum variant, a type mismatch — and this
/// route sits in FRONT of `garde`, so a tool advertising a value-free refusal would
/// otherwise leak before any field rule ran. This renders the failure's
/// CLASSIFICATION instead, plus the position when `serde_json` reports one (it does
/// for `from_str`; `from_value` has no position and reports line 0).
#[cfg(feature = "validation")]
fn redacted_deserialize_error(tool: &str, e: &serde_json::Error) -> Error {
    let classification = match e.classify() {
        serde_json::error::Category::Data => {
            "the arguments do not match the declared argument type"
        },
        serde_json::error::Category::Syntax => "the arguments are not well-formed JSON",
        serde_json::error::Category::Eof => "the arguments ended unexpectedly",
        serde_json::error::Category::Io => "the arguments could not be read",
    };
    let position = if e.line() == 0 {
        String::new()
    } else {
        format!(" at line {}, column {}", e.line(), e.column())
    };
    Error::Validation(format!(
        "Invalid arguments for tool '{tool}': {classification}{position}"
    ))
}

/// Map a `garde::Report` onto a value-free [`Error::Validation`].
///
/// # Why the text is rebuilt rather than `Display`-formatted
///
/// `garde::Report`'s own `Display` is value-free by construction — a rule's message
/// names the DECLARED bound ("length is greater than 10"), never the rejected value
/// (measured, Phase 128 RESEARCH Finding 4b). The `Path` half is NOT: garde 0.23's
/// map validator EXTENDS the path with the map's KEYS
/// (`garde-0.23.0/src/validate.rs:293`), and `T` here is generic and unrestricted,
/// so a `#[garde(dive)]` field of type `HashMap<String, _>` contributes
/// caller-chosen segments that may themselves be PHI — the same class as an
/// `additionalProperties` key on the D1 path (T-128-17b). Every segment is
/// therefore PROJECTED through [`project_garde_segment`], reusing the D1 path's
/// redaction token so the two refusals read alike.
///
/// # Residual leak surfaces, stated rather than implied
///
/// 1. **`#[garde(custom(..))]` supplies its own message**, copied here verbatim. A
///    custom validator must not put the rejected value in it; `pmcp` cannot inspect
///    a closure's text.
/// 2. **A caller-chosen map key that is ITSELF a bare identifier survives.** The
///    projection cannot enumerate `T`'s declared field names at runtime, so its
///    test is "does this segment have the SHAPE of a declared identifier". A map key
///    like `ssn` is indistinguishable from a field named `ssn` and is emitted. Keys
///    carrying anything else — a space, a dot, a hyphen, a digit first — are
///    redacted. A tool whose map keys are themselves sensitive should validate
///    inside the handler body instead.
#[cfg(feature = "validation")]
fn render_garde_refusal(tool: &str, report: &garde::Report) -> Error {
    let detail = report
        .iter()
        .map(|(path, error)| {
            let pointer = project_garde_path(path);
            if pointer.is_empty() {
                error.message().to_string()
            } else {
                format!("{pointer}: {}", error.message())
            }
        })
        .collect::<Vec<String>>()
        .join("; ");
    let detail = if detail.is_empty() {
        "the arguments do not satisfy the declared field rules".to_string()
    } else {
        detail
    };
    Error::Validation(format!("Invalid arguments for tool '{tool}': {detail}"))
}

/// Project a `garde::Path` into a value-free RFC 6901-shaped pointer.
///
/// Walks the path's COMPONENTS rather than parsing its `Display`, which is
/// deliberate: `Display` joins components with `.` and `[`/`]`, so a caller-chosen
/// key containing one of those characters is indistinguishable from a nesting
/// separator once rendered, and a lossy split could emit half of a sensitive key
/// verbatim (`ssn.value` -> `ssn` + `value`, two identifier-shaped halves).
/// `Path::__iter` is `#[doc(hidden)]` in garde 0.23 and `garde_derive` itself calls
/// it the same way (`garde_derive-0.23.0/src/lib.rs:126`); a future garde release
/// that removes it is a COMPILE error here rather than a silent behaviour change.
#[cfg(feature = "validation")]
fn project_garde_path(path: &garde::Path) -> String {
    let mut out = String::new();
    for (_, component) in path.__iter().rev() {
        out.push('/');
        out.push_str(project_garde_segment(component.as_str()));
    }
    out
}

/// One segment of [`project_garde_path`]'s walk.
///
/// Emitted VERBATIM only when it cannot carry caller-chosen text — a bare
/// identifier (the shape of a declared struct field name) or a base-10 index (a
/// sequence position). Everything else becomes the D1 path's `REDACTED_SEGMENT`.
#[cfg(feature = "validation")]
fn project_garde_segment(segment: &str) -> &str {
    if is_identifier_shaped(segment) || is_base10_index(segment) {
        segment
    } else {
        crate::server::schema_validation::REDACTED_SEGMENT
    }
}

/// `true` when `segment` has the shape of a Rust identifier, which is what a
/// declared struct field name always is.
#[cfg(feature = "validation")]
fn is_identifier_shaped(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_alphabetic() || first == b'_' => {},
        _ => return false,
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// `true` when `segment` is a base-10 sequence index. Carries no caller text.
#[cfg(feature = "validation")]
fn is_base10_index(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
}

/// A synchronous typed tool implementation with automatic schema generation.
pub struct TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,
    execution: Option<ToolExecution>,
    handler: F,
    /// The `garde` validator, populated ONLY by [`TypedSyncTool::new_validated`]
    /// and [`TypedSyncTool::new_validated_with_schema`]. See
    /// [`TypedTool`]'s field of the same name for why it is optional.
    #[cfg(feature = "validation")]
    validator: Option<GardeValidator<T>>,
    _phantom: PhantomData<T>,
}

impl<T, F> fmt::Debug for TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedSyncTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("annotations", &self.annotations)
            .finish()
    }
}

impl<T, F> TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    /// Create a new synchronous typed tool with automatic schema generation.
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
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            #[cfg(feature = "validation")]
            validator: None,
            _phantom: PhantomData,
        }
    }

    /// Create a new synchronous typed tool with a manually provided schema.
    pub fn new_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: schema,
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            #[cfg(feature = "validation")]
            validator: None,
            _phantom: PhantomData,
        }
    }

    /// Create a new synchronous typed tool that runs `T`'s `garde` field rules
    /// after deserialization (Phase 128, E3).
    ///
    /// The sync twin of [`TypedTool::new_validated`] — see it for why the
    /// `garde::Validate` bound sits on the constructor rather than on the type,
    /// for the `Context = ()` scope boundary, and for the refusal shape.
    #[cfg(all(feature = "validation", feature = "schema-generation"))]
    pub fn new_validated(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema + garde::Validate<Context = ()>,
    {
        Self::new(name, handler).with_garde_validator()
    }

    /// [`TypedSyncTool::new_validated`] with a manually provided schema.
    ///
    /// See [`TypedTool::new_validated_with_schema`].
    #[cfg(feature = "validation")]
    pub fn new_validated_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self
    where
        T: garde::Validate<Context = ()>,
    {
        Self::new_with_schema(name, schema, handler).with_garde_validator()
    }

    /// Store the `garde` entry point. Private: the only way to reach it is a
    /// `new_validated*` constructor, so the bound cannot leak onto the type.
    #[cfg(feature = "validation")]
    fn with_garde_validator(mut self) -> Self
    where
        T: garde::Validate<Context = ()>,
    {
        self.validator = Some(Box::new(garde::Validate::validate));
        self
    }

    /// Deserialize `args` into `T`. See [`TypedTool`]'s method of the same name for
    /// why the validated path's message is redacted and the unvalidated path's is
    /// byte-identical to today (T-128-17a).
    ///
    #[cfg(feature = "validation")]
    fn deserialize_args(&self, args: Value) -> Result<T> {
        serde_json::from_value(args).map_err(|e| {
            if self.validator.is_some() {
                redacted_deserialize_error(&self.name, &e)
            } else {
                legacy_deserialize_error(&self.name, &e)
            }
        })
    }

    /// Deserialize `args` into `T`. Without the `validation` feature no validated
    /// constructor exists, so today's message is the only one.
    #[cfg(not(feature = "validation"))]
    fn deserialize_args(&self, args: Value) -> Result<T> {
        serde_json::from_value(args).map_err(|e| legacy_deserialize_error(&self.name, &e))
    }

    /// Run the stored `garde` validator, if this tool has one.
    ///
    /// A tool built through a plain constructor stores none, and this is a no-op.
    #[cfg(feature = "validation")]
    fn run_garde(&self, typed_args: &T) -> Result<()> {
        match self.validator.as_deref() {
            Some(validate) => {
                validate(typed_args).map_err(|report| render_garde_refusal(&self.name, &report))
            },
            None => Ok(()),
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// See [`TypedTool::with_annotations`] for detailed documentation.
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }

    /// Associate this tool with a UI resource (MCP Apps Extension).
    ///
    /// This sets the nested `_meta.ui.resourceUri` field and the `openai/outputTemplate`
    /// alias in the tool's `_meta`, allowing both MCP and `ChatGPT` hosts to display
    /// an interactive UI when this tool is invoked.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::typed_tool::TypedSyncTool;
    /// use serde_json::json;
    ///
    /// let tool = TypedSyncTool::new_with_schema(
    ///     "render_chart",
    ///     json!({"type": "object"}),
    ///     |_args: serde_json::Value, _extra| Ok(json!({"data": "chart"})),
    /// )
    /// .with_ui("ui://charts/sales");
    /// ```
    pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(ui_resource_uri.into());
        self
    }

    /// Declare execution metadata for this tool (MCP 2025-11-25).
    ///
    /// See [`TypedTool::with_execution`] for detailed documentation.
    pub fn with_execution(mut self, execution: ToolExecution) -> Self {
        self.execution = Some(execution);
        self
    }
}

#[async_trait]
impl<T, F> ToolHandler for TypedSyncTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Result<Value> + Send + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Deserialize the arguments, and validate them ONLY when this tool was built
        // through `TypedSyncTool::new_validated` / `new_validated_with_schema` —
        // every other constructor stores no validator and this step is
        // deserialization alone (T-128-18).
        let typed_args: T = self.deserialize_args(args)?;
        #[cfg(feature = "validation")]
        self.run_garde(&typed_args)?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args, extra)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo {
            name: self.name.clone(),
            title: None,
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: self.annotations.clone(),
            icons: None,
            _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
            execution: self.execution.clone(),
        })
    }
}

/// Generate a JSON schema for a type using schemars.
#[cfg(feature = "schema-generation")]
fn generate_schema<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);

    // Convert the schema to JSON value
    let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    });

    // Normalize the schema by inlining $ref references
    crate::server::schema_utils::normalize_schema(json_schema)
}

/// Extension trait to add type-safe schema generation to `SimpleTool`.
pub trait SimpleToolExt {
    /// Create a `SimpleTool` with schema generated from a type.
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self;
}

use super::simple_tool::SimpleTool;

impl<F> SimpleToolExt for SimpleTool<F>
where
    F: Fn(Value, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self {
        let schema = generate_schema::<T>();
        self.with_schema(schema)
    }
}

/// Extension trait to add type-safe schema generation to `SyncTool`.
pub trait SyncToolExt {
    /// Create a `SyncTool` with schema generated from a type.
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self;
}

use super::simple_tool::SyncTool;

impl<F> SyncToolExt for SyncTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    #[cfg(feature = "schema-generation")]
    fn with_schema_from<T: JsonSchema>(self) -> Self {
        let schema = generate_schema::<T>();
        self.with_schema(schema)
    }
}

/// A typed tool with both input and output type safety
///
/// This variant provides type safety for both input arguments and return values.
/// The derived output schema is published as the tool's `outputSchema` in
/// `tools/list`, and the server dispatchers bridge it onto the wire: a tool
/// with a declared `outputSchema` has its result value emitted as
/// `structuredContent` (alongside the serialized text voice), per the MCP
/// spec's "SHOULD return structuredContent conforming to it".
pub struct TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    output_schema: Option<Value>,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,
    execution: Option<ToolExecution>,
    handler: F,
    _phantom: PhantomData<(TIn, TOut)>,
}

impl<TIn, TOut, F> fmt::Debug for TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedToolWithOutput")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("output_schema", &self.output_schema)
            .field("annotations", &self.annotations)
            .field("ui_resource_uri", &self.ui_resource_uri)
            .finish()
    }
}

impl<TIn, TOut, F> TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    /// Create a new typed tool with automatic input and output schema generation
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        TIn: JsonSchema,
        TOut: JsonSchema,
    {
        let input_schema = generate_schema::<TIn>();
        let output_schema = Some(generate_schema::<TOut>());

        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema,
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create with only input schema generation (output schema omitted)
    #[cfg(feature = "schema-generation")]
    pub fn new_input_only(name: impl Into<String>, handler: F) -> Self
    where
        TIn: JsonSchema,
    {
        let input_schema = generate_schema::<TIn>();

        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema: None,
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create with manually provided schemas
    pub fn new_with_schemas(
        name: impl Into<String>,
        input_schema: Value,
        output_schema: Option<Value>,
        handler: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema,
            output_schema,
            annotations: None,
            ui_resource_uri: None,
            execution: None,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set annotations for this tool.
    ///
    /// These annotations will be merged with the auto-generated output schema
    /// annotation. User-provided hints (readOnlyHint, destructiveHint, etc.)
    /// will be combined with the output schema annotation.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "schema-generation")] {
    /// use pmcp::server::typed_tool::TypedToolWithOutput;
    /// use pmcp::types::ToolAnnotations;
    /// use serde::{Deserialize, Serialize};
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct QueryArgs { sql: String }
    ///
    /// #[derive(Debug, Serialize, JsonSchema)]
    /// struct QueryResult { rows: Vec<String> }
    ///
    /// let tool = TypedToolWithOutput::new("query", |args: QueryArgs, _| {
    ///     Box::pin(async move {
    ///         Ok(QueryResult { rows: vec![] })
    ///     })
    /// })
    /// .with_description("Execute SQL query")
    /// .with_annotations(
    ///     ToolAnnotations::new()
    ///         .with_read_only(true)
    ///         .with_idempotent(true)
    /// );
    /// // Tool now has both readOnlyHint and auto-generated outputSchema
    /// # }
    /// ```
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Mark this tool as read-only (convenience method).
    pub fn read_only(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_read_only(true));
        self
    }

    /// Mark this tool as destructive (convenience method).
    pub fn destructive(mut self) -> Self {
        self.annotations = Some(
            self.annotations
                .unwrap_or_default()
                .with_read_only(false)
                .with_destructive(true),
        );
        self
    }

    /// Mark this tool as idempotent (convenience method).
    pub fn idempotent(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_idempotent(true));
        self
    }

    /// Mark this tool as interacting with external systems (convenience method).
    pub fn open_world(mut self) -> Self {
        self.annotations = Some(self.annotations.unwrap_or_default().with_open_world(true));
        self
    }

    /// Get the output schema (if any) for testing/documentation purposes
    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }

    /// Associate this tool with a UI resource (MCP Apps Extension).
    ///
    /// Sets `_meta.ui.resourceUri` and `openai/outputTemplate` in the tool's
    /// `ToolInfo` metadata for MCP and `ChatGPT` host compatibility.
    /// Deep-merged with any other `_meta` entries to prevent collision.
    pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
        self.ui_resource_uri = Some(ui_resource_uri.into());
        self
    }

    /// Declare execution metadata for this tool (MCP 2025-11-25).
    ///
    /// See [`TypedTool::with_execution`] for detailed documentation.
    pub fn with_execution(mut self, execution: ToolExecution) -> Self {
        self.execution = Some(execution);
        self
    }
}

#[async_trait]
impl<TIn, TOut, F> ToolHandler for TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Parse the arguments to the input type
        let typed_args: TIn = serde_json::from_value(args)
            .map_err(|e| Error::Validation(format!("Invalid arguments: {}", e)))?;

        // Call the handler
        let result = (self.handler)(typed_args, extra).await?;

        // Convert the typed result to JSON
        serde_json::to_value(result)
            .map_err(|e| Error::Internal(format!("Failed to serialize result: {}", e)))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        // Start with user-provided annotations or empty
        let mut annotations = self.annotations.clone().unwrap_or_default();

        // Add output type name annotation if output schema is available and user hasn't set one
        if let Some(schema) = &self.output_schema {
            if annotations.output_type_name.is_none() {
                let type_name = schema
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Output")
                    .to_string();

                annotations = annotations.with_output_type_name(type_name);
            }
        }

        let has_annotations = !annotations.is_empty();

        Some(ToolInfo {
            name: self.name.clone(),
            title: None,
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            annotations: if has_annotations {
                Some(annotations)
            } else {
                None
            },
            icons: None,
            _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
            execution: self.execution.clone(),
        })
    }
}

/// A typed tool whose async closure returns a full [`CallToolResult`] the handler
/// owns end-to-end, emitted to the wire **VERBATIM**.
///
/// Registered by
/// [`ServerBuilder::tool_with_result`](crate::server::ServerBuilder::tool_with_result).
/// Unlike [`TypedToolWithOutput`] — which serializes a typed `TOut` into a value
/// the server then text-wraps / widget-enriches — this wrapper's
/// [`ToolHandler::handle_output`] override returns
/// [`ToolOutput::Result`], so the closure's
/// `CallToolResult` reaches the wire exactly as provided.
///
/// # ⚠️ Bypass warning
///
/// Because the result is emitted verbatim (see
/// [`ToolOutput::Result`]), it **bypasses
/// response middleware** (redaction / sanitization / audit) as well as
/// text-wrapping and widget enrichment. The closure owns its OWN redaction and
/// sanitization of both `content` and `_meta`, at the same trust level as
/// returning a raw `Value` today (D-04a).
pub struct TypedToolWithResult<TIn, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    handler: F,
    _phantom: PhantomData<TIn>,
}

impl<TIn, F> fmt::Debug for TypedToolWithResult<TIn, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedToolWithResult")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .finish()
    }
}

impl<TIn, F> TypedToolWithResult<TIn, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
{
    /// Create a new full-result tool with automatic input schema generation.
    #[cfg(feature = "schema-generation")]
    pub fn new(name: impl Into<String>, handler: F) -> Self
    where
        TIn: JsonSchema,
    {
        Self {
            name: name.into(),
            description: None,
            input_schema: generate_schema::<TIn>(),
            handler,
            _phantom: PhantomData,
        }
    }

    /// Create with a manually provided input schema.
    pub fn new_with_schema(name: impl Into<String>, input_schema: Value, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema,
            handler,
            _phantom: PhantomData,
        }
    }

    /// Set the description for this tool.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Deserialize the arguments and run the closure, producing the owned
    /// `CallToolResult`. Shared by `handle` (fallback) and `handle_output` (the
    /// real verbatim path).
    async fn run(&self, args: Value, extra: RequestHandlerExtra) -> Result<CallToolResult> {
        let typed_args: TIn = serde_json::from_value(args).map_err(|e| {
            Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;
        (self.handler)(typed_args, extra).await
    }
}

#[async_trait]
impl<TIn, F> ToolHandler for TypedToolWithResult<TIn, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Fallback for callers that only invoke `handle` (e.g. workflow-internal
        // tool steps): serialize the owned envelope to a Value. The real
        // dispatch path is `handle_output` below, which emits the envelope
        // VERBATIM as `ToolOutput::Result`.
        let result = self.run(args, extra).await?;
        serde_json::to_value(result)
            .map_err(|e| Error::Internal(format!("Failed to serialize CallToolResult: {}", e)))
    }

    async fn handle_output(&self, args: Value, extra: RequestHandlerExtra) -> Result<ToolOutput> {
        Ok(ToolOutput::Result(self.run(args, extra).await?))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo {
            name: self.name.clone(),
            title: None,
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: None,
            execution: None,
        })
    }
}

#[cfg(test)]
#[allow(clippy::used_underscore_binding)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_typed_tool_metadata_with_ui_has_standard_key_only() {
        let tool = TypedTool::new_with_schema(
            "test_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Box::pin(async { Ok(json!({})) }),
        )
        .with_ui("ui://widgets/chart.html");

        let info = tool.metadata().unwrap();
        let meta = info._meta.as_ref().expect("_meta should be present");

        // Must have nested ui.resourceUri
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://widgets/chart.html");

        // Must NOT have openai/outputTemplate in standard-only mode
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );
    }

    #[test]
    fn test_typed_tool_metadata_without_ui_has_no_meta() {
        let tool = TypedTool::new_with_schema(
            "test_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Box::pin(async { Ok(json!({})) }),
        );

        let info = tool.metadata().unwrap();
        assert!(info._meta.is_none(), "_meta should be None without UI");
    }

    #[test]
    fn test_typed_sync_tool_metadata_with_ui_has_standard_key_only() {
        let tool = TypedSyncTool::new_with_schema(
            "test_sync_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Ok(json!({})),
        )
        .with_ui("ui://widgets/chart.html");

        let info = tool.metadata().unwrap();
        let meta = info._meta.as_ref().expect("_meta should be present");

        // Must have nested ui.resourceUri
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://widgets/chart.html");

        // Must NOT have openai/outputTemplate in standard-only mode
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );
    }

    #[test]
    fn test_typed_sync_tool_metadata_without_ui_has_no_meta() {
        let tool = TypedSyncTool::new_with_schema(
            "test_sync_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Ok(json!({})),
        );

        let info = tool.metadata().unwrap();
        assert!(info._meta.is_none(), "_meta should be None without UI");
    }

    #[test]
    fn test_typed_tool_with_output_with_ui_metadata() {
        let tool = TypedToolWithOutput::new_with_schemas(
            "ui_output_tool",
            json!({"type": "object"}),
            None,
            |_args: serde_json::Value, _extra: RequestHandlerExtra| {
                Box::pin(async { Ok(json!({"result": "ok"})) })
            },
        )
        .with_ui("ui://widgets/dashboard.html");

        let info = tool.metadata().unwrap();
        let meta = info._meta.as_ref().expect("_meta should be present");

        // Must have nested ui.resourceUri
        let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
        assert_eq!(ui_obj["resourceUri"], "ui://widgets/dashboard.html");

        // Must NOT have openai/outputTemplate in standard-only mode
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );
    }

    #[test]
    fn test_typed_tool_with_output_with_ui_and_output_schema_coexist() {
        let output_schema = json!({
            "type": "object",
            "title": "DashboardResult",
            "properties": {
                "data": { "type": "array" }
            }
        });

        let tool = TypedToolWithOutput::new_with_schemas(
            "ui_schema_tool",
            json!({"type": "object"}),
            Some(output_schema),
            |_args: serde_json::Value, _extra: RequestHandlerExtra| {
                Box::pin(async { Ok(json!({"data": []})) })
            },
        )
        .with_ui("ui://charts/bar.html");

        let info = tool.metadata().unwrap();

        // _meta must have UI metadata
        let meta = info
            ._meta
            .as_ref()
            .expect("_meta should be present with UI");
        assert!(meta.get("ui").is_some(), "ui key must be present");
        // No openai/outputTemplate in standard-only mode
        assert!(
            meta.get("openai/outputTemplate").is_none(),
            "must NOT have openai/outputTemplate in standard-only mode"
        );

        // output_schema must be top-level on ToolInfo
        assert!(
            info.output_schema.is_some(),
            "output_schema must be present on ToolInfo"
        );

        // annotations must have output_type_name
        let annotations = info
            .annotations
            .as_ref()
            .expect("annotations should be present");
        assert!(
            annotations.output_type_name.is_some(),
            "output_type_name annotation must be present"
        );
    }

    #[test]
    fn test_typed_tool_with_output_without_ui_has_no_meta() {
        let tool = TypedToolWithOutput::new_with_schemas(
            "no_ui_tool",
            json!({"type": "object"}),
            None,
            |_args: serde_json::Value, _extra: RequestHandlerExtra| {
                Box::pin(async { Ok(json!({})) })
            },
        );

        let info = tool.metadata().unwrap();
        assert!(info._meta.is_none(), "_meta should be None without UI");
    }

    #[test]
    fn test_typed_tool_with_execution_metadata() {
        use crate::types::{TaskSupport, ToolExecution};

        let tool = TypedTool::new_with_schema(
            "long_task",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Box::pin(async { Ok(json!({})) }),
        )
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let info = tool.metadata().unwrap();
        let exec = info
            .execution
            .as_ref()
            .expect("execution should be present");
        assert_eq!(exec.task_support, Some(TaskSupport::Required));
    }

    #[test]
    fn test_typed_tool_without_execution_returns_none() {
        let tool = TypedTool::new_with_schema(
            "simple_tool",
            json!({"type": "object"}),
            |_args: serde_json::Value, _extra| Box::pin(async { Ok(json!({})) }),
        );

        let info = tool.metadata().unwrap();
        assert!(info.execution.is_none());
    }
}
