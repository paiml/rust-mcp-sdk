// Net-new code for Phase 83 TKIT-07.
// Lands the `[[tools]]`-config-driven synthesizer that turns config rows into
// `ToolInfo` + `Arc<dyn ToolHandler>` pairs.

//! `[[tools]]` → `ToolInfo` + `Arc<dyn ToolHandler>` synthesizer.
//!
//! Net-new code for Phase 83 TKIT-07. Turns curated `[[tools]]` config entries
//! into complete pmcp [`ToolInfo`] + [`Arc<dyn ToolHandler>`] pairs with zero
//! per-tool Rust handlers.
//!
//! # Invariants enforced
//!
//! - **JSON Schema object envelope.** Every synthesized [`ToolInfo`] carries an
//!   `input_schema` with `"type": "object"`, an explicit `properties` map, a
//!   `required` array, and `"additionalProperties": false`. Unknown argument
//!   keys are rejected by pmcp's request-validation path at `tools/call` time —
//!   defence-in-depth against arg-injection (threat T-83-05-02).
//! - **`handler.metadata()` returns `Some(ToolInfo)`.** Phase 82's `tool_arc`
//!   consumes `handler.metadata()` at registration; returning `None` would
//!   silently degrade the schema enforcement to "anything goes" (RESEARCH
//!   §Risks #2 — threat T-83-05-01).
//! - **Constructors, never struct-literals.** Both [`ToolInfo`] and
//!   [`ToolAnnotations`] are `#[non_exhaustive]` (PATTERNS §Pattern C). The
//!   synthesizer uses [`ToolInfo::with_annotations`] / [`ToolInfo::new`] and
//!   the [`ToolAnnotations::new()`]-then-`.with_*` fluent builder.
//! - **Cognitive complexity ≤25 per function.** Decomposed into
//!   [`build_input_schema`], [`build_param_property`], and [`build_annotations`]
//!   per Phase 75 D-03 + PATTERNS §Pattern G. No `#[allow]` annotations.

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::ToolHandler;
use pmcp::types::{ToolAnnotations, ToolInfo};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Map, Value};

use crate::config::{AnnotationsDecl, ParamDecl, ServerConfig, ToolDecl, ValidationSection};
use crate::error::Result;
use crate::sql::SqlConnector;

#[cfg(feature = "http")]
use crate::error::ToolkitError;
#[cfg(feature = "http")]
use crate::http::{HttpConnector, Operation, Parameter, ParameterLocation};

#[cfg(feature = "openapi-code-mode")]
use crate::code_mode::HttpCodeExecutor;
#[cfg(feature = "openapi-code-mode")]
use pmcp_code_mode::ExecutionConfig;

/// Type alias for one synthesized tool tuple: `(name, ToolInfo, Arc<dyn ToolHandler>)`.
///
/// Exists so [`synthesize_from_config`]'s return type does not trip
/// `clippy::type_complexity` while preserving the exact `(name, ToolInfo, Arc)`
/// shape consumers register with `pmcp::ServerBuilder::tool_arc` (PATTERNS §9).
pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>);

/// Synthesize one `ToolInfo` + handler per `[[tools]]` config entry.
///
/// Each returned tuple is `(name, ToolInfo, Arc<dyn ToolHandler>)` and is
/// ready to feed into `pmcp::ServerBuilder::tool_arc(name, handler)`. The
/// `ToolInfo` carries the full input schema (synthesized from
/// `[[tools.parameters]]`) and `ToolAnnotations` (from `[tools.annotations]`)
/// so the builder's metadata cache will never fall back to the empty schema.
///
/// # Errors
///
/// Returns [`crate::ToolkitError::Synth`] if a tool declaration is internally
/// inconsistent. The Plan 05 GREEN body never produces this error path —
/// synthesis is total over the parsed [`ServerConfig`] surface — but the
/// `Result` return is kept for forward compatibility with Plan 06 (code-mode
/// wiring) and Phase 84 (SQL backend resolution).
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::tools::synthesize_from_config;
///
/// let cfg = ServerConfig::default();
/// let synthesized = synthesize_from_config(&cfg).unwrap();
/// assert_eq!(synthesized.len(), 0);
/// ```
pub fn synthesize_from_config(config: &ServerConfig) -> Result<Vec<SynthesizedTool>> {
    synthesize_inner(config, None)
}

/// Synthesize tools that execute against a wired [`SqlConnector`] (Phase 84
/// CONN-01 / D-06). ADDITIVE variant alongside [`synthesize_from_config`] — the
/// existing API is unchanged and all P83 callers compile without modification.
///
/// Each synthesized [`SynthesizedToolHandler`] holds the shared `connector`, so
/// its `handle()` body calls [`SqlConnector::execute`] with the tool's declared
/// `sql` + the named parameters extracted from the validated args. When a tool
/// declares `ui_resource_uri`, the synthesized [`ToolInfo`] also carries widget
/// metadata so pmcp core's `with_widget_enrichment` populates `structuredContent`
/// (D-06) — that flip lives in the shared [`synthesize_inner`] helper and so
/// fires for both entry points.
///
/// # Errors
///
/// Returns [`crate::ToolkitError::Synth`] if a tool declaration is internally
/// inconsistent. Synthesis is total over the parsed [`ServerConfig`] surface —
/// the connector is threaded into each handler for runtime use, not consulted at
/// synthesis time.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
/// use pmcp_server_toolkit::tools::synthesize_from_config_with_connector;
///
/// fn build(connector: Arc<dyn SqlConnector>) {
///     let cfg = ServerConfig::default();
///     let tools = synthesize_from_config_with_connector(&cfg, connector).unwrap();
///     assert_eq!(tools.len(), 0);
/// }
/// ```
pub fn synthesize_from_config_with_connector(
    config: &ServerConfig,
    connector: Arc<dyn SqlConnector>,
) -> Result<Vec<SynthesizedTool>> {
    synthesize_inner(config, Some(connector))
}

/// Shared synthesizer body for both [`synthesize_from_config`] (no connector)
/// and [`synthesize_from_config_with_connector`] (connector wired).
///
/// Keeps the two public entry points one-liners so the widget_meta flip (D-06)
/// and the handler construction logic are not duplicated. Decomposed per
/// PATTERNS §Pattern G — the per-tool body delegates to [`build_input_schema`],
/// [`build_annotations`], and [`apply_widget_meta`] to stay under cog 25.
fn synthesize_inner(
    config: &ServerConfig,
    connector: Option<Arc<dyn SqlConnector>>,
) -> Result<Vec<SynthesizedTool>> {
    let validation = &config.server.validation;
    let mut out = Vec::with_capacity(config.tools.len());
    for decl in &config.tools {
        let info = build_tool_info(decl, validation);
        let handler: Arc<dyn ToolHandler> = Arc::new(SynthesizedToolHandler {
            info: info.clone(),
            decl: decl.clone(),
            connector: connector.clone(),
        });
        // Push site 1 of 3 (SQL handler) — D1 enforcement.
        let handler = enforce_input_schema(handler, &info, decl, validation);
        out.push((decl.name.clone(), info, handler));
    }
    Ok(out)
}

/// Enforce a synthesized tool's declared `inputSchema` before its inner handler
/// runs (Phase 128, D1 / T-128-01).
///
/// # Enforcing function
///
/// `pmcp::server::schema_validation::validate_input`, called from BOTH
/// [`ValidatingToolHandler::handle`] and [`ValidatingToolHandler::handle_output`].
/// Backed by `tests/input_validation_acceptance.rs` acceptance rows 8–11, which
/// fail if either call is removed.
///
/// Wrapping happens at every handler push site in this module, so all four public
/// entry points inherit the enforcement with none forgotten.
///
/// With the `input-validation` feature OFF this is the identity function, and the
/// opt-out is logged once per synthesized tool at synthesis time: a validation rule
/// that is off must never read as on.
///
/// # `[server.validation] enforce_input_schema = false`
///
/// That flag is an operator opt-out from the SCHEMA CHECK, not from the decorator.
/// [`ValidatingToolHandler`] carries an `enforce_schema: bool` and skips only the
/// `validate_input` call, so a future explicitly-registered argument validator
/// living in the same decorator keeps running. Turning off one enforcement must
/// never silently turn off another — and losing an explicit custom validator
/// because schema enforcement was disabled is the worst form of that class, since
/// the operator turned off A and lost B without being told.
///
/// The decorator is constructed when schema enforcement is ON **or** a validator is
/// registered for this tool. No validator registry exists yet (it lands with E2),
/// so `has_registered_validator` is `false` today and the decorator is skipped
/// entirely when `enforce_input_schema = false` — which preserves the "no added
/// allocation for a server that uses neither" property. When the registry arrives,
/// only that one input changes; the separation it depends on is already in place
/// and pinned by a test.
#[allow(unused_variables)]
fn enforce_input_schema(
    handler: Arc<dyn ToolHandler>,
    info: &ToolInfo,
    decl: &ToolDecl,
    validation: &ValidationSection,
) -> Arc<dyn ToolHandler> {
    #[cfg(feature = "input-validation")]
    {
        // E2 (plan 09) replaces this with a registry lookup on `decl.name`.
        let has_registered_validator = false;
        if !validation.enforce_input_schema && !has_registered_validator {
            tracing::warn!(
                tool = %decl.name,
                "[server.validation] enforce_input_schema = false: this tool's arguments are \
                 NOT checked against its declared inputSchema before the backend call"
            );
            return handler;
        }
        ValidatingToolHandler::wrap(handler, info, decl, validation.enforce_input_schema)
    }
    #[cfg(not(feature = "input-validation"))]
    {
        tracing::warn!(
            tool = %decl.name,
            "the `input-validation` feature is OFF: this tool's arguments are NOT checked \
             against its declared inputSchema before the backend call"
        );
        handler
    }
}

/// Decorator that refuses a `tools/call` whose arguments violate the tool's
/// declared `inputSchema`, WITHOUT invoking the inner handler.
///
/// Crate-private by design: the validator and the value-free refusal renderer both
/// live in core `pmcp` (D-01), so this type holds no schema logic of its own.
#[cfg(feature = "input-validation")]
struct ValidatingToolHandler {
    inner: Arc<dyn ToolHandler>,
    /// The synthesized `ToolInfo`'s `input_schema`, verbatim.
    input_schema: Value,
    /// `input_schema.to_string()`, computed ONCE at synthesis so the hot
    /// `tools/call` path never re-serializes the whole schema to hit the core
    /// validator cache.
    schema_key: String,
    /// Declared parameter names in declaration order — the ONLY names a refusal
    /// message may echo (SC-7).
    declared: Vec<String>,
    /// Whether the SCHEMA CHECK runs, from `[server.validation]
    /// enforce_input_schema`.
    ///
    /// A SEPARATE switch from the decorator's existence on purpose: when the E2
    /// argument-validator registry lands in the same decorator, an operator who
    /// turns schema enforcement off must not silently lose an
    /// explicitly-registered validator too.
    enforce_schema: bool,
}

#[cfg(feature = "input-validation")]
impl ValidatingToolHandler {
    /// Wrap `handler`, taking the schema from the already-built `info` and the
    /// declared parameter names from `decl`.
    fn wrap(
        inner: Arc<dyn ToolHandler>,
        info: &ToolInfo,
        decl: &ToolDecl,
        enforce_schema: bool,
    ) -> Arc<dyn ToolHandler> {
        let input_schema = info.input_schema.clone();
        let schema_key = input_schema.to_string();
        Arc::new(Self {
            inner,
            input_schema,
            schema_key,
            declared: decl.parameters.iter().map(|p| p.name.clone()).collect(),
            enforce_schema,
        })
    }

    /// Validate `args`, mapping any violation to a value-free
    /// `pmcp::Error::Validation`. Kept a separate helper so both trait entry
    /// points stay one-liners and well under the cog-25 gate.
    ///
    /// Returns `Ok(())` without consulting the schema when `enforce_schema` is
    /// `false` — skipping the CHECK, not the decorator, so anything else this
    /// decorator does still happens.
    fn check(&self, args: &Value) -> pmcp::Result<()> {
        use pmcp::server::schema_validation::{render_refusal, validate_input};

        if !self.enforce_schema {
            return Ok(());
        }
        validate_input(&self.input_schema, Some(args), Some(&self.schema_key)).map_err(
            |violations| {
                let declared: Vec<&str> = self.declared.iter().map(String::as_str).collect();
                pmcp::Error::Validation(render_refusal(&violations, &declared))
            },
        )
    }
}

#[cfg(feature = "input-validation")]
#[async_trait]
impl ToolHandler for ValidatingToolHandler {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        self.check(&args)?;
        self.inner.handle(args, extra).await
    }

    fn metadata(&self) -> Option<ToolInfo> {
        self.inner.metadata()
    }

    /// Validates, then DELEGATES to the inner handler's own `handle_output`.
    ///
    /// Implementing only `handle` would silently replace an inner handler's
    /// `handle_output` override with the trait default
    /// (`handle(..).map(ToolOutput::Payload)`), stripping that handler's ability to
    /// own its `CallToolResult` envelope. A decorator must not narrow the contract
    /// of what it wraps — and both entry points must validate, or an inner handler
    /// that overrides `handle_output` becomes an unvalidated path.
    async fn handle_output(
        &self,
        args: Value,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<pmcp::server::ToolOutput> {
        self.check(&args)?;
        self.inner.handle_output(args, extra).await
    }
}

/// Flip widget metadata onto `info` when the declaration carries a
/// `ui_resource_uri` (D-06 / REVIEWS M1).
///
/// Uses the feature-independent [`ToolInfo::with_meta_entry`] surface to insert
/// `_meta.ui.resourceUri`. This is the verified-correct API: `with_widget_meta`
/// is gated on pmcp's `mcp-apps` feature (which the toolkit does not enable),
/// whereas `with_meta_entry` is always available and produces the `ui.resourceUri`
/// shape that `ToolInfo::widget_meta()` recognises — so pmcp core's
/// `with_widget_enrichment` populates `structuredContent`. Annotations on `info`
/// are preserved (chained, not reconstructed).
fn apply_widget_meta(info: ToolInfo, decl: &ToolDecl) -> ToolInfo {
    match decl.ui_resource_uri.as_deref() {
        Some(uri) => info.with_meta_entry("ui", json!({ "resourceUri": uri })),
        None => info,
    }
}

/// Build the [`ToolInfo`] for a synthesized tool from its declaration.
///
/// The schema + annotations + widget-meta sequence is identical for every tool
/// kind (single-call HTTP, SQL, and script), so it lives here once — keeping the
/// `#[non_exhaustive]` [`ToolInfo`] constructor discipline (the `with_annotations`
/// vs `new` arms) in a single place rather than copy-pasted per synthesizer.
fn build_tool_info(decl: &ToolDecl, validation: &ValidationSection) -> ToolInfo {
    let schema = build_input_schema(decl, validation);
    let annotations = build_annotations(decl.annotations.as_ref());
    let base = match annotations {
        Some(ann) => {
            ToolInfo::with_annotations(decl.name.clone(), decl.description.clone(), schema, ann)
        },
        None => ToolInfo::new(decl.name.clone(), decl.description.clone(), schema),
    };
    apply_widget_meta(base, decl)
}

/// Build the JSON Schema `properties` + `required` envelope from a
/// `[[tools.parameters]]` list.
///
/// Decomposed from [`synthesize_from_config`] to keep cognitive complexity ≤25
/// (Phase 75 D-03 + PATTERNS §Pattern G).
///
/// `pub(crate)` since Phase 128 SC-2: [`crate::config::ServerConfig::validate`]
/// builds each tool's schema through THIS function and compiles it at config time,
/// so the schema the compile gate checks is byte-identical to the one the runtime
/// validator enforces. A second construction path there would let the gate pass a
/// schema the server never serves.
///
/// Takes the whole [`ToolDecl`] rather than just its parameters because the D3
/// default cap is POSITION-scoped, and position is a property of the tool's `path`
/// and `method` — see [`crate::config::ToolDecl::param_position`].
///
/// `properties` and `required` are emitted in `[[tools.parameters]]` declaration
/// order, so two synthesis runs over one config produce byte-identical output.
pub(crate) fn build_input_schema(decl: &ToolDecl, validation: &ValidationSection) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in &decl.parameters {
        let mut prop = build_param_property(p);
        apply_position_cap(&mut prop, p, decl.param_position(&p.name), validation);
        props.insert(p.name.clone(), prop);
        if p.required {
            required.push(Value::String(p.name.clone()));
        }
    }
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": validation.additional_properties,
    })
}

/// Apply the D3 position-scoped default length cap to ONE already-built property
/// object (Phase 128).
///
/// Emits `maxLength = validation.default_max_length` only when ALL of:
/// the parameter's effective type is `string`; it declares no `max_length` of its
/// own; `default_max_length` is non-zero; and `position` is `Path` or `Query`.
/// BODY position emits nothing — capping a `POST` payload's free-text field is
/// exactly the breakage D-05 exists to avoid, and `ServerConfig::lint` surfaces
/// those instead.
///
/// A DECLARED `max_length` is never overridden and never merged with the default in
/// either direction: the author's number wins outright, and the always-on
/// `PLACEHOLDER_MAX_LENGTH` floor is what bounds a URL segment regardless.
///
/// The four conditions live in `crate::config::default_cap_applies` rather than
/// here, so `lint()` and this function cannot disagree about which parameters are
/// covered — a disagreement would make `lint()` report a parameter as uncapped
/// while the schema caps it, or the reverse.
///
/// Its own free function (not inlined into [`build_input_schema`]) to keep that
/// function's cognitive complexity well under the 25 gate.
fn apply_position_cap(
    prop: &mut Value,
    p: &ParamDecl,
    position: crate::config::ParamPosition,
    validation: &ValidationSection,
) {
    if crate::config::default_cap_applies(p, position, validation) {
        prop["maxLength"] = json!(validation.default_max_length);
    }
}

/// Build a single JSON Schema property object from a [`ParamDecl`].
///
/// Per-parameter constraints (`minimum`, `maximum`, `maxLength`, `minLength`,
/// `pattern`, `format`, `maxItems`, `items`, `default`, `enum`) are folded in only
/// when present — an undeclared keyword is ABSENT from the emitted schema, never
/// present-and-empty, because an empty `pattern` matches everything and an empty
/// `items` object constrains nothing while both read like rules.
///
/// The param's `param_type` defaults to `"string"` when omitted in TOML to match
/// JSON Schema's permissive default.
fn build_param_property(p: &ParamDecl) -> Value {
    let ty = p.param_type.as_deref().unwrap_or("string");
    let mut prop = json!({ "type": ty });
    if let Some(desc) = &p.description {
        prop["description"] = Value::String(desc.clone());
    }
    if let Some(min) = p.minimum {
        prop["minimum"] = json!(min);
    }
    if let Some(max) = p.maximum {
        prop["maximum"] = json!(max);
    }
    if let Some(max_len) = p.max_length {
        prop["maxLength"] = json!(max_len);
    }
    if let Some(min_len) = p.min_length {
        prop["minLength"] = json!(min_len);
    }
    if let Some(pattern) = &p.pattern {
        prop["pattern"] = Value::String(pattern.clone());
    }
    if let Some(format) = &p.format {
        prop["format"] = Value::String(format.clone());
    }
    if let Some(max_items) = p.max_items {
        prop["maxItems"] = json!(max_items);
    }
    if let Some(items) = &p.items {
        prop["items"] = build_items_property(items);
    }
    if let Some(default) = &p.default {
        // toml::Value serializes losslessly into serde_json::Value via serde.
        if let Ok(v) = serde_json::to_value(default) {
            prop["default"] = v;
        }
    }
    if let Some(enum_vals) = &p.enum_values {
        if let Ok(v) = serde_json::to_value(enum_vals) {
            prop["enum"] = v;
        }
    }
    prop
}

/// Build the OBJECT-form JSON Schema `items` value from an [`ItemsDecl`]
/// (Phase 128, D2).
///
/// A separate free function for two reasons. First, cognitive complexity:
/// [`build_param_property`] gained five arms in this phase and RESEARCH assumption
/// A4 (that they stay under cog 25) was explicitly unmeasured, so the construction
/// lives outside it. Second, and more importantly, there is exactly ONE place that
/// can decide the SHAPE of `items`, and it returns a [`Value::Object`]
/// unconditionally. The array form of `items` is a draft-07 tuple construct that
/// does NOT compile under the Draft 2020-12 pin, and a schema that does not compile
/// takes the whole tool's validator down — so "never an array here" is a safety
/// property, not a style preference, and it is enforced by this function having no
/// code path that builds a JSON array.
fn build_items_property(items: &crate::config::ItemsDecl) -> Value {
    let ty = items.item_type.as_deref().unwrap_or("string");
    let mut prop = json!({ "type": ty });
    if let Some(max_len) = items.max_length {
        prop["maxLength"] = json!(max_len);
    }
    if let Some(pattern) = &items.pattern {
        prop["pattern"] = Value::String(pattern.clone());
    }
    prop
}

/// Build [`ToolAnnotations`] from an optional `[tools.annotations]` block.
///
/// Per PATTERNS §Pattern C, the constructor + fluent builder is used (never a
/// struct literal — [`ToolAnnotations`] is `#[non_exhaustive]`). The `cost_hint`
/// field has no `ToolAnnotations` accessor and is therefore not propagated at
/// this layer; it lives on the toolkit's [`AnnotationsDecl`] and is consumed
/// by future plans that surface cost into rate-limiting policy.
fn build_annotations(decl: Option<&AnnotationsDecl>) -> Option<ToolAnnotations> {
    let d = decl?;
    let a = ToolAnnotations::new()
        .with_read_only(d.read_only_hint)
        .with_destructive(d.destructive_hint)
        .with_idempotent(d.idempotent_hint)
        .with_open_world(d.open_world_hint);
    Some(a)
}

// -----------------------------------------------------------------------------
// SynthesizedToolHandler — crate-private
// -----------------------------------------------------------------------------

/// Crate-private handler wrapping a synthesized [`ToolInfo`].
///
/// [`ToolHandler::metadata`] MUST return `Some(self.info.clone())` — Phase 82's
/// `tool_arc` consumes `handler.metadata()` at registration time; returning
/// `None` would cause the builder to fall back to an empty schema (RESEARCH
/// §Risks #2 — threat T-83-05-01). The unit + property tests in
/// [`crate::tools`] and `tests/tool_synthesis_props.rs` lock this in.
///
/// `handle()` reads the declared `sql`, extracts named parameters from the
/// validated args, and calls [`SqlConnector::execute`] when a `connector` is
/// wired (handlers built via [`synthesize_from_config_with_connector`]).
/// Handlers built via the no-connector [`synthesize_from_config`] carry
/// `connector = None` and return an explicit `Err` on invocation — preserving
/// P83 behaviour where the no-connector path was test-only (T-84-03-05). The
/// `decl` is held so the handler can read `sql` / `ui_resource_uri` /
/// `parameters` without re-walking the config.
struct SynthesizedToolHandler {
    info: ToolInfo,
    decl: ToolDecl,
    /// `Some` only for handlers built via [`synthesize_from_config_with_connector`].
    connector: Option<Arc<dyn SqlConnector>>,
}

/// Extract the named `(name, value)` parameter pairs the connector binds from,
/// filtering the caller's validated `args` against the declared parameter list
/// (T-84-03-01: only declared parameter names reach `execute()`; extra keys are
/// silently dropped — JSON-schema validation rejects them upstream).
///
/// When the caller omits an optional parameter that declares a `default`, the
/// default is applied so the bound SQL sees a concrete value. Without this an
/// omitted `:limit` / `:offset` would bind as unbound `NULL` and SQLite rejects
/// `LIMIT NULL` with a "datatype mismatch" — so the declared default is the
/// difference between a working and a broken tool call (the reference
/// `search_tracks` / `list_artists` calls rely on it).
///
/// An EXPLICIT JSON `null` for a declared-default parameter is treated the SAME
/// as an omitted parameter — the declared default is applied (85-10 WR-02
/// secondary fix). Without the `is_null` filter a caller sending
/// `{"limit": null}` would bind `LIMIT NULL` and SQLite would reject the query
/// with "datatype mismatch", even though the tool declares `default = 20`.
fn extract_named_params(decl: &ToolDecl, args: &Value) -> Vec<(String, Value)> {
    decl.parameters
        .iter()
        .filter_map(|p| {
            args.get(&p.name)
                // Explicit JSON `null` falls through to the declared default,
                // exactly like an omitted key (no `LIMIT NULL` bind).
                .filter(|v| !v.is_null())
                .cloned()
                .or_else(|| {
                    p.default
                        .as_ref()
                        .and_then(|d| serde_json::to_value(d).ok())
                })
                .map(|v| (p.name.clone(), v))
        })
        .collect()
}

#[async_trait]
impl ToolHandler for SynthesizedToolHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let sql = self.decl.sql.as_deref().ok_or_else(|| {
            pmcp::Error::Internal(format!("tool '{}' has no `sql` declared", self.info.name))
        })?;
        let connector = self.connector.as_ref().ok_or_else(|| {
            pmcp::Error::Internal(format!(
                "tool '{}' requires connector wiring — build via synthesize_from_config_with_connector",
                self.info.name
            ))
        })?;
        let named_params = extract_named_params(&self.decl, &args);
        // T-84-03-02: format!("{e}") uses ConnectorError::Display, which Plan 01
        // Task 2 guarantees does not echo credentials.
        let rows = connector
            .execute(sql, &named_params)
            .await
            .map_err(|e| pmcp::Error::Internal(format!("connector error: {e}")))?;
        Ok(Value::Array(rows))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(self.info.clone())
    }
}

// -----------------------------------------------------------------------------
// Single-call HTTP synthesizer (Phase 90 OAPI-02a) — feature `http`
// -----------------------------------------------------------------------------

/// Synthesize one `ToolInfo` + handler per single-call `[[tools]]` entry,
/// executing against a wired [`HttpConnector`] (Phase 90 OAPI-02a / D-01).
///
/// Mirrors [`synthesize_from_config_with_connector`] (the SQL analog) in shape.
/// For each `[[tools]]` where [`ToolDecl::is_script_tool`] is `false` and a
/// `path` + `method` pair is present, an [`Operation`] is built from the path
/// template (the `{...}` segments become path parameters), the declared
/// `[[tools.parameters]]` (non-path params become query params; `POST`/`PUT`/
/// `PATCH` carry a request body), and the per-tool `base_url` override — which is
/// reflected onto the [`Operation`] so the connector targets the per-tool host
/// (never silently dropped, Codex MEDIUM). The synthesized [`ToolInfo`] uses the
/// EXISTING [`build_input_schema`] (object envelope + `additionalProperties:false`)
/// and [`build_annotations`] helpers; the handler calls
/// [`HttpConnector::execute`] and returns the JSON.
///
/// # Script-tool seam (Plan 05)
///
/// A `script` tool encountered here returns a typed [`ToolkitError::Synth`] — it
/// is an EXPLICIT, clearly-marked seam, NOT a silent skip and NOT a `todo!()`.
/// Plan 05 widens this function's signature (adding the shared `http_exec` +
/// `exec_config`) and fills the `is_script_tool()` arm with a `ScriptToolHandler`
/// branch; that change is a localized, anticipated edit because the seam is
/// surfaced here.
///
/// # Errors
///
/// Returns [`ToolkitError::Synth`] when a `[[tools]]` entry is a `script` tool
/// (Plan 05 seam) or is neither a valid single-call (missing `path` OR `method`)
/// nor a script tool (T-90-03-04 negative validation — an ill-formed tool is
/// rejected, never silently registered).
#[cfg(feature = "http")]
pub fn synthesize_from_config_with_http_connector(
    config: &ServerConfig,
    connector: Arc<dyn HttpConnector>,
) -> Result<Vec<SynthesizedTool>> {
    // No script-tool builder is supplied on this (single-call-only) entry point,
    // so the `is_script_tool()` arm of [`synthesize_http_inner`] returns the
    // typed Plan 05 seam error. The OpenAPI Code Mode build calls
    // [`synthesize_from_config_with_http_connector_and_scripts`], which supplies
    // a [`ScriptToolHandler`] builder so a `script` tool synthesizes a real
    // handler over the shared engine (OAPI-02b / D-01 / D-02).
    synthesize_http_inner(config, connector, |decl| {
        Err(ToolkitError::Synth(format!(
            "tool '{}' is a script tool — script tools require the `openapi-code-mode` \
             feature (use synthesize_from_config_with_http_connector_and_scripts)",
            decl.name
        )))
    })
}

/// Synthesize single-call AND script `[[tools]]` against a wired
/// [`HttpConnector`] plus a shared [`HttpCodeExecutor`] + [`ExecutionConfig`]
/// (Phase 90 OAPI-02b / D-01 / D-02).
///
/// This is the OpenAPI Code Mode entry point (gated `openapi-code-mode`): it
/// adds the `http_exec` + `exec_config` the script-tool path needs, threading
/// the SAME `HttpCodeExecutor` instance that feeds Code Mode (D-02 — one engine,
/// two surfaces). Single-call tools synthesize exactly as in
/// [`synthesize_from_config_with_http_connector`]; a `script` tool synthesizes a
/// [`ScriptToolHandler`] that compiles + runs the embedded JS through the SAME
/// `PlanCompiler` + `PlanExecutor` + `HttpCodeExecutor` seam Code Mode uses,
/// with NO validate/token cycle (admin-authored, `ExecutionConfig`-bounded —
/// Pitfall 7).
///
/// The binary (Plan 06) supplies `http_exec` (built once over the resolved
/// backend `base_url` + auth provider) and `exec_config` (from the
/// `[code_mode.limits]` / defaults: `max_api_calls=50`, `max_loop_iterations=100`,
/// `timeout_seconds=30`).
///
/// # Errors
///
/// Returns [`ToolkitError::Synth`] when a `[[tools]]` entry is neither a valid
/// single-call (missing `path` OR `method`) nor a script tool (T-90-03-04
/// negative validation), or when a script tool fails to build its `ToolInfo`.
#[cfg(feature = "openapi-code-mode")]
pub fn synthesize_from_config_with_http_connector_and_scripts(
    config: &ServerConfig,
    connector: Arc<dyn HttpConnector>,
    http_exec: HttpCodeExecutor,
    exec_config: ExecutionConfig,
) -> Result<Vec<SynthesizedTool>> {
    let validation = &config.server.validation;
    synthesize_http_inner(config, connector, |decl| {
        let handler =
            ScriptToolHandler::new(decl, http_exec.clone(), exec_config.clone(), validation)?;
        let info = handler.tool_info.clone();
        let arc: Arc<dyn ToolHandler> = Arc::new(handler);
        Ok((info, arc))
    })
}

/// Shared synthesizer body for the single-call HTTP entry points.
///
/// `build_script_tool` is invoked for each `script` tool: the single-call-only
/// entry point passes a closure that returns the typed Plan 05 / `openapi-code-mode`
/// seam error, while the OpenAPI Code Mode entry point passes a closure that
/// constructs a [`ScriptToolHandler`]. Decomposed per PATTERNS §Pattern G to keep
/// the per-tool loop body under cog ≤25.
#[cfg(feature = "http")]
fn synthesize_http_inner(
    config: &ServerConfig,
    connector: Arc<dyn HttpConnector>,
    mut build_script_tool: impl FnMut(&ToolDecl) -> Result<(ToolInfo, Arc<dyn ToolHandler>)>,
) -> Result<Vec<SynthesizedTool>> {
    let validation = &config.server.validation;
    let mut out = Vec::with_capacity(config.tools.len());
    for decl in &config.tools {
        if decl.is_script_tool() {
            let (info, handler) = build_script_tool(decl)?;
            // Push site 2 of 3 (SCRIPT tool) — D1 enforcement. This site is
            // SEPARATE from the HTTP push below because of the `continue`; wrapping
            // only the HTTP push would leave every script tool unvalidated.
            let handler = enforce_input_schema(handler, &info, decl, validation);
            out.push((decl.name.clone(), info, handler));
            continue;
        }

        // Single-call requires BOTH path and method. A `[[tools]]` that is
        // neither a valid single-call nor a script tool is rejected (T-90-03-04).
        let (path, method) = match (decl.path.as_deref(), decl.method.as_deref()) {
            (Some(p), Some(m)) => (p, m),
            _ => {
                return Err(ToolkitError::Synth(format!(
                    "tool '{}' is not a valid single-call tool: both `path` and `method` are required",
                    decl.name
                )));
            },
        };

        let operation = build_operation(path, method, decl);
        let info = build_tool_info(decl, validation);
        let handler: Arc<dyn ToolHandler> = Arc::new(HttpToolHandler {
            info: info.clone(),
            operation,
            connector: connector.clone(),
        });
        // Push site 3 of 3 (single-call HTTP handler) — D1 enforcement.
        let handler = enforce_input_schema(handler, &info, decl, validation);
        out.push((decl.name.clone(), info, handler));
    }
    Ok(out)
}

/// Build the [`Operation`] for a single-call tool from its `path` template,
/// `method`, declared parameters, and per-tool `base_url`.
///
/// Path parameters are the `{...}` segments of the path template; every other
/// declared `[[tools.parameters]]` becomes a query parameter (the reference
/// `create_tool_from_config` mapping). `POST`/`PUT`/`PATCH` carry a request body
/// so non-path/query args are sent as JSON. The per-tool `base_url` is reflected
/// onto the [`Operation`] (Codex MEDIUM — never dropped).
///
/// # Relationship to `ToolDecl::param_position` (Phase 128)
///
/// The PATH split here and [`crate::config::ToolDecl::param_position`]'s `Path` arm
/// read the SAME [`crate::config::path_placeholder_names`] helper, so they cannot
/// drift — a drift would silently mis-scope the D3 cap and the D4 placeholder rules.
///
/// The QUERY assignment below deliberately DIVERGES from `param_position`: this
/// function marks every non-path declared parameter `ParameterLocation::Query`
/// regardless of method (its Phase 90 behaviour, unchanged), while `param_position`
/// classifies a `POST`/`PUT`/`PATCH` tool's non-path parameters as `Body`. The two
/// answer different questions — where a value TRAVELS versus where LENGTH is
/// dangerous — and capping a mutating tool's free-text payload field at 256 code
/// points is the breakage D-05 exists to avoid. Do not "reconcile" them.
#[cfg(feature = "http")]
fn build_operation(path: &str, method: &str, decl: &ToolDecl) -> Operation {
    let method_upper = method.to_uppercase();
    let path_param_names: Vec<&str> = crate::config::path_placeholder_names(path).collect();

    let mut parameters = Vec::with_capacity(decl.parameters.len());
    // Path params (template `{...}` segments) — always required.
    //
    // Phase 128 D4(b): this loop iterates the TEMPLATE, not `decl.parameters`, so
    // the declared narrowing is looked up BY NAME and falls back to "no declared
    // rules" when the template names a segment the config never declared. Such a
    // parameter still gets the unconditional character floor and the always-on cap
    // at substitution time — it simply gets no narrowing on top (D-10).
    for name in &path_param_names {
        let declared = decl.parameters.iter().find(|p| p.name == **name);
        parameters.push(
            Parameter::new((*name).to_string(), ParameterLocation::Path, true).with_rules(
                declared.and_then(|p| p.pattern.clone()),
                declared.and_then(|p| p.max_length),
                // D-11: the config is the ONE legitimate source of this permission.
                declared.is_some_and(|p| p.allow_slash),
            ),
        );
    }
    // Remaining declared params → query params.
    for p in &decl.parameters {
        if path_param_names.iter().any(|n| *n == p.name) {
            continue;
        }
        parameters.push(
            Parameter::new(p.name.clone(), ParameterLocation::Query, p.required).with_rules(
                p.pattern.clone(),
                p.max_length,
                p.allow_slash,
            ),
        );
    }

    let has_request_body = matches!(method_upper.as_str(), "POST" | "PUT" | "PATCH");

    Operation {
        method: method_upper,
        path: path.to_string(),
        parameters,
        has_request_body,
        base_url: decl.base_url.clone(),
    }
}

/// Crate-private handler for a single-call HTTP tool (Phase 90 OAPI-02a).
///
/// Holds the synthesized [`ToolInfo`], the built [`Operation`], and the shared
/// [`HttpConnector`]. [`ToolHandler::metadata`] returns `Some(self.info.clone())`
/// (the same RESEARCH §Risks #2 invariant the SQL handler upholds); `handle()`
/// calls [`HttpConnector::execute`] and returns the JSON response.
#[cfg(feature = "http")]
struct HttpToolHandler {
    info: ToolInfo,
    operation: Operation,
    connector: Arc<dyn HttpConnector>,
}

#[cfg(feature = "http")]
#[async_trait]
impl ToolHandler for HttpToolHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // T-90-03-01: arg injection is bounded by the object-envelope schema
        // (additionalProperties:false) enforced upstream; path substitution in the
        // connector touches only declared `{params}`.
        // The connector's Display is redaction-safe (T-90-01-01); no URL/credential
        // reaches the client error.
        self.connector
            .execute(&self.operation, &args)
            .await
            .map_err(|e| pmcp::Error::Internal(format!("connector error: {e}")))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(self.info.clone())
    }
}

// -----------------------------------------------------------------------------
// Script-tool handler (Phase 90 OAPI-02b / D-01 / D-02) — feature
// `openapi-code-mode`
// -----------------------------------------------------------------------------

/// Crate-private handler for a **script** `[[tools]]` entry (OAPI-02b / D-01).
///
/// A script tool runs admin-authored embedded JS through the EXACT SAME
/// `pmcp_code_mode` engine that Code Mode uses (D-02 — one engine, two
/// surfaces): [`pmcp_code_mode::PlanCompiler`] compiles the `script` to an
/// execution plan, then [`pmcp_code_mode::PlanExecutor`] over the shared
/// [`HttpCodeExecutor`] walks it. The client's validated `args` are bound to the
/// `args` variable BEFORE the script runs — identical to the `JsCodeExecutor`
/// path's `set_variable("args", …)`, which is what makes the engine-parity proof
/// (Plan 05 Task 2) hold byte-for-byte.
///
/// # No token cycle (Pitfall 7 / T-90-05-01)
///
/// A script tool is admin-authored + trusted (like a `sql=` curated query), so it
/// skips the Code Mode validation + HMAC-token gate entirely. It is bounded ONLY by
/// the [`ExecutionConfig`] caps (`max_api_calls`, `max_loop_iterations`,
/// `timeout_seconds`) the [`PlanExecutor`](pmcp_code_mode::PlanExecutor) enforces,
/// and by the `PlanCompiler`-accepted JS subset (no `eval` / FFI).
///
/// # Feature gate (RESEARCH Pitfall 4)
///
/// Gated `openapi-code-mode` (the umbrella that forwards
/// `pmcp-code-mode/js-runtime`) — `PlanCompiler` / `PlanExecutor` are NOT in
/// scope under bare `code-mode`, so the light / curated-only build (`http
/// code-mode`) compiles without this type (single-call only).
#[cfg(feature = "openapi-code-mode")]
struct ScriptToolHandler {
    /// The admin-authored script, compiled ONCE at synthesis (the body is fixed
    /// content). Executed per `handle` over a fresh
    /// [`PlanExecutor`](pmcp_code_mode::PlanExecutor).
    plan: pmcp_code_mode::ExecutionPlan,
    /// The SAME executor instance that feeds Code Mode (D-02). Cloned per request
    /// to construct a fresh [`PlanExecutor`](pmcp_code_mode::PlanExecutor).
    http_exec: HttpCodeExecutor,
    /// The execution bounds (Pitfall 7 — the only limit on an admin script).
    exec_config: ExecutionConfig,
    /// The synthesized `ToolInfo` (object-envelope schema from
    /// `[[tools.parameters]]`, `additionalProperties:false`) — `args` are
    /// schema-validated against this BEFORE the script runs (T-90-05-03).
    tool_info: ToolInfo,
}

#[cfg(feature = "openapi-code-mode")]
impl ScriptToolHandler {
    /// Build a [`ScriptToolHandler`] from a script `[[tools]]` declaration,
    /// the shared [`HttpCodeExecutor`], and the [`ExecutionConfig`] bounds.
    ///
    /// The `tool_info` is built from `[[tools.parameters]]` via the SAME
    /// [`build_input_schema`] / [`build_annotations`] / [`apply_widget_meta`]
    /// helpers the single-call path uses, so a script tool's `args` are
    /// schema-validated identically (object envelope, `additionalProperties:false`
    /// unless `[server.validation] additional_properties` opts out).
    ///
    /// `validation` is threaded in so a script tool's schema is built under the SAME
    /// `[server.validation]` policy as every other tool kind — a script tool whose
    /// parameters escaped the D3 cap would be a hole in exactly the surface this
    /// phase closes. A script tool's parameters are BODY position (it declares no
    /// `path`/`method`), so in practice the cap does not apply to them; the point is
    /// that the decision is made by one rule rather than by which synthesizer ran.
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::Synth`] if the declaration carries no `script`
    /// (a defensive guard — callers route only `is_script_tool()` entries here),
    /// or if the script fails to compile (surfaced here at server build time,
    /// failing fast rather than on the first tool call).
    fn new(
        decl: &ToolDecl,
        http_exec: HttpCodeExecutor,
        exec_config: ExecutionConfig,
        validation: &ValidationSection,
    ) -> Result<Self> {
        let script = decl.script.clone().ok_or_else(|| {
            ToolkitError::Synth(format!(
                "tool '{}' has no `script` body — not a script tool",
                decl.name
            ))
        })?;
        // Compile the admin-authored JS ONCE at synthesis time — the script is
        // fixed content, so compiling per request would re-run a full SWC parse
        // on the hot path (the PlanCompiler-accepted subset, no eval / FFI, is
        // the static bound). A compile error surfaces here at server build.
        let plan = pmcp_code_mode::PlanCompiler::with_config(&exec_config)
            .compile_code(&script)
            .map_err(|e| {
                ToolkitError::Synth(format!(
                    "tool '{}' script failed to compile: {e}",
                    decl.name
                ))
            })?;
        let tool_info = build_tool_info(decl, validation);
        Ok(Self {
            plan,
            http_exec,
            exec_config,
            tool_info,
        })
    }
}

#[cfg(feature = "openapi-code-mode")]
#[pmcp_code_mode::async_trait]
impl ToolHandler for ScriptToolHandler {
    /// Run the pre-compiled admin-authored script over the shared engine, binding
    /// the validated `args` to the `args` variable (D-02 — identical to the
    /// `JsCodeExecutor` path's `set_variable("args", …)`).
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // (1) Execute the plan (compiled once in `new`) over a PER-REQUEST clone
        //     of the shared HttpCodeExecutor (D-02), threading the captured
        //     inbound MCP token (Plan 90-10 / OAPI-03 / OAPI-05) so an
        //     `oauth_passthrough` backend forwards it. Bounded by ExecutionConfig
        //     (Pitfall 7 — no token cycle, only these caps).
        let mut executor = pmcp_code_mode::PlanExecutor::new(
            crate::code_mode::request_executor_from_extra(&self.http_exec, &extra),
            self.exec_config.clone(),
        );
        // (2) Bind the schema-validated client args to `args` (T-90-05-03) —
        //     byte-identical to compile_and_execute's set_variable("args", …).
        executor.set_variable("args", args);

        let result = executor
            .execute(&self.plan)
            .await
            .map_err(|e| pmcp::Error::Internal(format!("script execution failed: {e}")))?;
        Ok(result.value)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(self.tool_info.clone())
    }
}

// -----------------------------------------------------------------------------
// Tests — Plan 05 Task 1 (RED) → GREEN in Task 2
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AnnotationsDecl, ItemsDecl, ParamDecl, ServerConfig, ServerSection, ToolDecl,
        ValidationSection,
    };
    use serde_json::Value;

    /// Construct a minimal `ServerConfig` that satisfies `validate()` (non-empty
    /// `name` + `version`) so the synthesizer path is the system under test —
    /// not the parser/validator from Plan 04.
    fn cfg_with_tools(tools: Vec<ToolDecl>) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools,
            ..Default::default()
        }
    }

    #[test]
    fn empty_tools_returns_empty_vec() {
        let cfg = cfg_with_tools(vec![]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn one_tool_no_params_yields_object_schema() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "ping".to_string(),
            description: Some("Ping the server".to_string()),
            parameters: vec![],
            annotations: None,
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(out.len(), 1);
        let (name, info, _handler) = &out[0];
        assert_eq!(name, "ping");
        assert_eq!(info.name, "ping");
        assert_eq!(info.description.as_deref(), Some("Ping the server"));
        let schema = &info.input_schema;
        assert_eq!(schema["type"], Value::String("object".to_string()));
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
    }

    #[test]
    fn required_and_optional_params_partitioned() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "search".to_string(),
            description: Some("Search".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "query".to_string(),
                    param_type: Some("string".to_string()),
                    description: Some("the search query".to_string()),
                    required: true,
                    ..Default::default()
                },
                ParamDecl {
                    name: "max_results".to_string(),
                    param_type: Some("integer".to_string()),
                    description: Some("maximum result count".to_string()),
                    required: false,
                    default: Some(toml::Value::Integer(100)),
                    minimum: Some(1.0),
                    maximum: Some(1000.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        let schema = &info.input_schema;
        assert_eq!(schema["required"], serde_json::json!(["query"]));
        let props = schema["properties"].as_object().expect("object");
        assert_eq!(props["query"]["type"], "string");
        assert_eq!(props["max_results"]["type"], "integer");
        assert_eq!(props["max_results"]["minimum"], serde_json::json!(1.0));
        assert_eq!(props["max_results"]["maximum"], serde_json::json!(1000.0));
        assert_eq!(props["max_results"]["default"], serde_json::json!(100));
    }

    #[test]
    fn param_max_length_propagates() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "echo".to_string(),
            description: Some("Echo".to_string()),
            parameters: vec![ParamDecl {
                name: "text".to_string(),
                param_type: Some("string".to_string()),
                description: Some("input text".to_string()),
                required: true,
                max_length: Some(256),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        assert_eq!(
            info.input_schema["properties"]["text"]["maxLength"],
            serde_json::json!(256)
        );
    }

    #[test]
    fn annotations_round_trip_via_fluent_builder() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "destroy_all".to_string(),
            description: Some("Destroy all data (test)".to_string()),
            parameters: vec![],
            annotations: Some(AnnotationsDecl {
                read_only_hint: false,
                destructive_hint: true,
                idempotent_hint: false,
                open_world_hint: false,
                cost_hint: Some("high".to_string()),
            }),
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        let ann = info.annotations.as_ref().expect("annotations");
        assert_eq!(ann.read_only_hint, Some(false));
        assert_eq!(ann.destructive_hint, Some(true));
        assert_eq!(ann.idempotent_hint, Some(false));
        assert_eq!(ann.open_world_hint, Some(false));
    }

    /// REVIEWS H1 (in-plan widget_meta flip — no SqliteConnector dependency).
    ///
    /// When a `[[tools]]` entry declares `ui_resource_uri`, the synthesized
    /// `ToolInfo` must carry widget metadata so pmcp core's
    /// `with_widget_enrichment` (gated on `info.widget_meta().is_some()`)
    /// populates `structuredContent` (D-06). The flip lives in the shared
    /// `synthesize_inner` helper, so it fires for BOTH entry points; this test
    /// exercises it via the no-connector `synthesize_from_config` path.
    #[test]
    fn widget_meta_flips_when_ui_resource_uri_present() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "widget_tool".to_string(),
            description: Some("renders a widget".to_string()),
            ui_resource_uri: Some("ui://test".to_string()),
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        assert!(
            info.widget_meta().is_some(),
            "ui_resource_uri set ⇒ widget_meta() must be Some so D-06 structuredContent fires"
        );
    }

    /// REVIEWS H1 negative case — a tool WITHOUT `ui_resource_uri` must NOT
    /// carry widget metadata (T-84-03-03: no accidental flip on non-widget
    /// tools).
    #[test]
    fn widget_meta_absent_when_ui_resource_uri_none() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "plain_tool".to_string(),
            description: Some("no widget".to_string()),
            ui_resource_uri: None,
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        assert!(
            info.widget_meta().is_none(),
            "ui_resource_uri absent ⇒ widget_meta() must be None (no accidental flip)"
        );
    }

    #[tokio::test]
    async fn synthesized_handler_metadata_returns_some() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "ping".to_string(),
            description: Some("ping".to_string()),
            parameters: vec![],
            annotations: None,
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, expected_info, handler) = &out[0];
        let actual = handler.metadata();
        assert!(
            actual.is_some(),
            "RESEARCH §Risks #2 invariant: SynthesizedToolHandler::metadata() MUST return Some(ToolInfo)"
        );
        assert_eq!(actual.unwrap().name, expected_info.name);
    }

    /// A `[[tools]]` declaration with one defaulted `limit` param (default=20),
    /// used to exercise [`extract_named_params`]'s default / explicit-null logic.
    fn decl_with_limit_default() -> ToolDecl {
        ToolDecl {
            name: "search".to_string(),
            description: Some("Search".to_string()),
            sql: Some("SELECT * FROM t LIMIT :limit".to_string()),
            parameters: vec![ParamDecl {
                name: "limit".to_string(),
                param_type: Some("integer".to_string()),
                description: Some("row limit".to_string()),
                required: false,
                default: Some(toml::Value::Integer(20)),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn extract_named_params_applies_default_when_absent() {
        // `{}` → declared default (20) is bound (the reference search/list calls
        // rely on this so an omitted :limit never binds NULL).
        let decl = decl_with_limit_default();
        let params = extract_named_params(&decl, &serde_json::json!({}));
        assert_eq!(params, vec![("limit".to_string(), serde_json::json!(20))]);
    }

    #[test]
    fn extract_named_params_explicit_null_applies_default() {
        // 85-10 WR-02 secondary fix: an EXPLICIT JSON null must NOT bind
        // `LIMIT NULL` — it falls through to the declared default exactly like
        // an omitted key.
        let decl = decl_with_limit_default();
        let params = extract_named_params(&decl, &serde_json::json!({ "limit": null }));
        assert_eq!(
            params,
            vec![("limit".to_string(), serde_json::json!(20))],
            "explicit null must apply the declared default, not bind LIMIT NULL"
        );
    }

    #[test]
    fn extract_named_params_explicit_value_overrides_default() {
        // A concrete value wins over the default.
        let decl = decl_with_limit_default();
        let params = extract_named_params(&decl, &serde_json::json!({ "limit": 5 }));
        assert_eq!(params, vec![("limit".to_string(), serde_json::json!(5))]);
    }

    // -------------------------------------------------------------------------
    // Phase 128 D2 — the six new `ParamDecl` keywords reach `inputSchema`
    // -------------------------------------------------------------------------

    /// Fetch the synthesized property object for `param` of the single tool in
    /// `tools`.
    fn prop_of(tools: Vec<ToolDecl>, param: &str) -> Value {
        let cfg = cfg_with_tools(tools);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_name, info, _handler) = &out[0];
        info.input_schema["properties"][param].clone()
    }

    /// D2 / SC-2: `pattern`, `minLength`, `format` and `maxItems` all reach the
    /// emitted `inputSchema`.
    #[test]
    fn input_schema_emits_d2_scalar_keywords() {
        let prop = prop_of(
            vec![ToolDecl {
                name: "lookup".to_string(),
                parameters: vec![ParamDecl {
                    name: "region".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    pattern: Some("^[A-Z]{3}$".to_string()),
                    min_length: Some(3),
                    format: Some("uuid".to_string()),
                    max_items: Some(5),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            "region",
        );
        assert_eq!(prop["pattern"], serde_json::json!("^[A-Z]{3}$"));
        assert_eq!(prop["minLength"], serde_json::json!(3));
        assert_eq!(prop["format"], serde_json::json!("uuid"));
        assert_eq!(prop["maxItems"], serde_json::json!(5));
    }

    /// D2 / RESEARCH Finding 1h: `items` is emitted in OBJECT form. The array
    /// (draft-07 tuple) form does not compile under the Draft 2020-12 pin and
    /// would take the whole tool's validator down.
    #[test]
    fn input_schema_emits_items_as_object_never_array() {
        let prop = prop_of(
            vec![ToolDecl {
                name: "batch".to_string(),
                parameters: vec![ParamDecl {
                    name: "codes".to_string(),
                    param_type: Some("array".to_string()),
                    required: true,
                    items: Some(ItemsDecl {
                        item_type: Some("string".to_string()),
                        max_length: Some(8),
                        pattern: Some("^[a-z]+$".to_string()),
                    }),
                    max_items: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            "codes",
        );
        assert!(
            prop["items"].is_object(),
            "items must be an OBJECT, got: {}",
            prop["items"]
        );
        assert!(
            !prop["items"].is_array(),
            "array-form items does not compile under the 2020-12 pin"
        );
        assert_eq!(
            prop["items"],
            serde_json::json!({
                "type": "string",
                "maxLength": 8,
                "pattern": "^[a-z]+$",
            })
        );
    }

    /// D2 edge (empty): a `ParamDecl` carrying NONE of the six new keywords emits
    /// exactly the five it emits today — no empty `pattern` string and no empty
    /// `items` object appears.
    #[test]
    fn input_schema_omits_d2_keywords_when_undeclared() {
        let prop = prop_of(
            vec![ToolDecl {
                name: "legacy".to_string(),
                parameters: vec![ParamDecl {
                    name: "count".to_string(),
                    param_type: Some("integer".to_string()),
                    description: Some("how many".to_string()),
                    required: false,
                    minimum: Some(1.0),
                    maximum: Some(10.0),
                    max_length: Some(4),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            "count",
        );
        let obj = prop.as_object().expect("property object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["description", "maxLength", "maximum", "minimum", "type"],
            "exactly the five pre-D2 keywords, and no more"
        );
        for absent in ["pattern", "minLength", "format", "items", "maxItems"] {
            assert!(
                obj.get(absent).is_none(),
                "undeclared keyword {absent} must not be emitted"
            );
        }
    }

    /// D2 edge (ordering): two synthesis runs over ONE config produce
    /// byte-identical `input_schema` values, so `properties` and `required` follow
    /// `[[tools.parameters]]` declaration order deterministically.
    #[test]
    fn input_schema_is_byte_identical_across_two_synthesis_runs() {
        let tools = vec![ToolDecl {
            name: "search".to_string(),
            parameters: vec![
                ParamDecl {
                    name: "zebra".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    pattern: Some("^z".to_string()),
                    ..Default::default()
                },
                ParamDecl {
                    name: "alpha".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    min_length: Some(1),
                    ..Default::default()
                },
                ParamDecl {
                    name: "middle".to_string(),
                    param_type: Some("integer".to_string()),
                    required: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];
        let cfg = cfg_with_tools(tools);
        let first = synthesize_from_config(&cfg).expect("synthesize")[0]
            .1
            .input_schema
            .to_string();
        let second = synthesize_from_config(&cfg).expect("synthesize")[0]
            .1
            .input_schema
            .to_string();
        assert_eq!(first, second, "synthesis must be byte-deterministic");
        // Declaration order, not sorted order.
        assert!(
            first.find("\"zebra\"").unwrap() < first.find("\"alpha\"").unwrap(),
            "properties must follow declaration order: {first}"
        );
    }

    /// D2 edge (adjacency): `min_length == max_length` accepts exactly that length
    /// and refuses one code point either side, through the SAME core validator the
    /// D1 decorator uses.
    #[cfg(feature = "input-validation")]
    #[test]
    fn min_length_equal_to_max_length_accepts_exactly_that_length() {
        use pmcp::server::schema_validation::validate_input;

        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "exact".to_string(),
            parameters: vec![ParamDecl {
                name: "code".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                min_length: Some(3),
                max_length: Some(3),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let schema = &out[0].1.input_schema;

        validate_input(schema, Some(&serde_json::json!({ "code": "abc" })), None)
            .expect("exactly three code points must be accepted");
        validate_input(schema, Some(&serde_json::json!({ "code": "ab" })), None)
            .expect_err("two code points must be refused");
        validate_input(schema, Some(&serde_json::json!({ "code": "abcd" })), None)
            .expect_err("four code points must be refused");
    }

    // -------------------------------------------------------------------------
    // Phase 128 D3 — position-scoped default cap and `[server.validation]`
    // -------------------------------------------------------------------------

    /// Synthesize `tools` under `validation` and return the property object for
    /// `param` of the FIRST tool.
    fn prop_under(tools: Vec<ToolDecl>, validation: ValidationSection, param: &str) -> Value {
        let mut cfg = cfg_with_tools(tools);
        cfg.server.validation = validation;
        let out = synthesize_from_config(&cfg).expect("synthesize");
        out[0].1.input_schema["properties"][param].clone()
    }

    /// A single-call `GET` tool declaring one path parameter and one query
    /// parameter, both uncapped strings.
    fn get_tool_with_path_and_query() -> Vec<ToolDecl> {
        vec![ToolDecl {
            name: "line_status".to_string(),
            path: Some("/lines/{line_id}/status".to_string()),
            method: Some("GET".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "line_id".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    ..Default::default()
                },
                ParamDecl {
                    name: "detail".to_string(),
                    param_type: Some("string".to_string()),
                    required: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]
    }

    /// D3: a PATH-position string with no `max_length` is capped at the configured
    /// default.
    #[test]
    fn default_cap_applies_to_path_position_string() {
        let prop = prop_under(
            get_tool_with_path_and_query(),
            ValidationSection::default(),
            "line_id",
        );
        assert_eq!(prop["maxLength"], serde_json::json!(256));
    }

    /// D3: a QUERY-position string (non-path parameter of a `GET` tool) with no
    /// `max_length` is capped at the configured default.
    #[test]
    fn default_cap_applies_to_query_position_string() {
        let prop = prop_under(
            get_tool_with_path_and_query(),
            ValidationSection::default(),
            "detail",
        );
        assert_eq!(prop["maxLength"], serde_json::json!(256));
    }

    /// D3 / D-05 / T-128-13d: a BODY-position string on a mutating single-call tool
    /// receives NO cap. Capping a `POST` payload's free-text field is exactly the
    /// breakage review note C objected to.
    #[test]
    fn default_cap_never_applies_to_body_position_string() {
        let tools = vec![ToolDecl {
            name: "add_comment".to_string(),
            path: Some("/issues/{id}/comments".to_string()),
            method: Some("POST".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "id".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    ..Default::default()
                },
                ParamDecl {
                    name: "body_text".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];
        let path_prop = prop_under(tools.clone(), ValidationSection::default(), "id");
        assert_eq!(
            path_prop["maxLength"],
            serde_json::json!(256),
            "the path parameter of a POST tool IS still capped"
        );
        let body_prop = prop_under(tools, ValidationSection::default(), "body_text");
        assert!(
            body_prop.get("maxLength").is_none(),
            "a POST payload field must receive no default cap, got: {body_prop}"
        );
    }

    /// D3 edge (adjacency): a DECLARED `max_length` is never overridden and never
    /// merged with the default, in either direction.
    #[test]
    fn declared_max_length_wins_over_the_default_cap() {
        let mut tools = get_tool_with_path_and_query();
        tools[0].parameters[0].max_length = Some(12);
        let prop = prop_under(tools, ValidationSection::default(), "line_id");
        assert_eq!(prop["maxLength"], serde_json::json!(12));
    }

    /// D3 edge (empty): `default_max_length = 0` emits no `maxLength` in ANY
    /// position and disables the cap entirely.
    #[test]
    fn default_max_length_zero_disables_the_cap_in_every_position() {
        let validation = ValidationSection {
            default_max_length: 0,
            ..Default::default()
        };
        for param in ["line_id", "detail"] {
            let prop = prop_under(get_tool_with_path_and_query(), validation.clone(), param);
            assert!(
                prop.get("maxLength").is_none(),
                "{param} must carry no maxLength when the default is 0, got: {prop}"
            );
        }
    }

    /// D3 edge (boundary + encoding): the cap is counted in Unicode CODE POINTS,
    /// not bytes — so exactly `default_max_length` multi-byte characters are
    /// accepted and one more is refused, through the SAME core validator.
    #[cfg(feature = "input-validation")]
    #[test]
    fn default_cap_boundary_is_counted_in_code_points_not_bytes() {
        use pmcp::server::schema_validation::validate_input;

        let validation = ValidationSection {
            default_max_length: 8,
            ..Default::default()
        };
        let mut cfg = cfg_with_tools(get_tool_with_path_and_query());
        cfg.server.validation = validation;
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let schema = &out[0].1.input_schema;

        // Each `é` is TWO bytes and ONE code point. Eight of them are 16 bytes.
        let at_limit: String = "é".repeat(8);
        let over_limit: String = "é".repeat(9);
        assert_eq!(
            at_limit.len(),
            16,
            "the fixture must actually be multi-byte"
        );
        validate_input(
            schema,
            Some(&serde_json::json!({ "line_id": at_limit })),
            None,
        )
        .expect("exactly 8 code points must be accepted even though they are 16 bytes");
        validate_input(
            schema,
            Some(&serde_json::json!({ "line_id": over_limit })),
            None,
        )
        .expect_err("9 code points must be refused");
    }

    /// `additional_properties = true` flips the emitted envelope, re-opening the
    /// unknown-argument class for this server.
    #[test]
    fn additional_properties_opt_out_flips_the_envelope() {
        let mut cfg = cfg_with_tools(get_tool_with_path_and_query());
        cfg.server.validation = ValidationSection {
            additional_properties: true,
            ..Default::default()
        };
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(
            out[0].1.input_schema["additionalProperties"],
            Value::Bool(true)
        );

        let mut cfg = cfg_with_tools(get_tool_with_path_and_query());
        cfg.server.validation = ValidationSection::default();
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(
            out[0].1.input_schema["additionalProperties"],
            Value::Bool(false),
            "the default must remain a closed envelope"
        );
    }

    /// T-128-13c: `enforce_input_schema = false` skips the SCHEMA CHECK, not the
    /// decorator. This test pins the separation the E2 argument-validator registry
    /// will depend on: with the flag off an undeclared argument is ACCEPTED (schema
    /// off), while the inner handler's own rule still REFUSES (the non-schema
    /// enforcement is untouched). Without this test the two collapse back together
    /// on the next refactor.
    #[cfg(feature = "input-validation")]
    #[tokio::test]
    async fn enforce_input_schema_false_skips_the_check_not_the_decorator() {
        /// Stands in for an explicitly-registered argument validator: a rule that
        /// lives INSIDE the decorated stack and is not the JSON Schema check.
        struct RefusingInner;

        #[async_trait]
        impl ToolHandler for RefusingInner {
            async fn handle(
                &self,
                _args: Value,
                _extra: RequestHandlerExtra,
            ) -> pmcp::Result<Value> {
                Err(pmcp::Error::Validation(
                    "the registered validator refused".to_string(),
                ))
            }
        }

        let decl = ToolDecl {
            name: "guarded".to_string(),
            parameters: vec![ParamDecl {
                name: "declared".to_string(),
                param_type: Some("string".to_string()),
                required: false,
                ..Default::default()
            }],
            ..Default::default()
        };
        let info = build_tool_info(&decl, &ValidationSection::default());
        let undeclared = serde_json::json!({ "not_declared_at_all": "x" });

        // (a) enforcement ON: the SCHEMA refuses before the inner handler runs, so
        //     the message is the schema refusal, not the inner one.
        let on = ValidatingToolHandler::wrap(Arc::new(RefusingInner), &info, &decl, true);
        let err = on
            .handle(undeclared.clone(), RequestHandlerExtra::default())
            .await
            .expect_err("an undeclared argument must be refused when enforcement is on");
        assert!(
            !err.to_string().contains("registered validator"),
            "the schema check must run FIRST when enforcement is on: {err}"
        );

        // (b) enforcement OFF: the schema check is skipped (the undeclared argument
        //     is accepted by it), and the inner rule STILL refuses. That is the
        //     separation — A off must not silently turn B off.
        let off = ValidatingToolHandler::wrap(Arc::new(RefusingInner), &info, &decl, false);
        let err = off
            .handle(undeclared, RequestHandlerExtra::default())
            .await
            .expect_err("the inner (non-schema) rule must still refuse");
        assert!(
            err.to_string().contains("registered validator"),
            "with the schema check off, the refusal must come from the inner rule: {err}"
        );
    }
}

// -----------------------------------------------------------------------------
// Tests — Phase 128 D4(b): `build_operation` carries the declared placeholder
// rules onto every `Parameter`.
// -----------------------------------------------------------------------------

/// Named `build_operation` deliberately: libtest matches the FULL test path, so a
/// test placed in the crate's existing `mod tests` would be
/// `tools::tests::build_operation_…` and this plan's `--lib tools::build_operation`
/// verify filter would select ZERO tests while exiting 0. As a SIBLING of `tests`
/// at the `tools` module level the filter resolves as written. Do not fold these
/// into `mod tests`.
#[cfg(all(test, feature = "http"))]
mod build_operation {
    use super::*;

    /// A single-call `GET` tool on `path` carrying `parameters`.
    fn decl(path: &str, parameters: Vec<ParamDecl>) -> ToolDecl {
        ToolDecl {
            name: "t".to_string(),
            description: Some("t".to_string()),
            path: Some(path.to_string()),
            method: Some("GET".to_string()),
            parameters,
            ..Default::default()
        }
    }

    fn param_named<'a>(op: &'a Operation, name: &str) -> &'a Parameter {
        op.parameters
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("parameter {name} present"))
    }

    /// A path parameter declaring a `pattern` produces a `Parameter` carrying it.
    #[test]
    fn build_operation_carries_a_declared_path_pattern() {
        let d = decl(
            "/content/{version}",
            vec![ParamDecl {
                name: "version".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                pattern: Some("^C[0-9]+$".to_string()),
                ..Default::default()
            }],
        );
        let op = super::build_operation("/content/{version}", "GET", &d);
        let p = param_named(&op, "version");
        assert_eq!(p.location, ParameterLocation::Path);
        assert_eq!(p.pattern.as_deref(), Some("^C[0-9]+$"));
    }

    /// A query parameter declaring `max_length = 64` produces a `Parameter`
    /// carrying `max_length: Some(64)`.
    #[test]
    fn build_operation_carries_a_declared_query_max_length() {
        let d = decl(
            "/search",
            vec![ParamDecl {
                name: "q".to_string(),
                param_type: Some("string".to_string()),
                max_length: Some(64),
                ..Default::default()
            }],
        );
        let op = super::build_operation("/search", "GET", &d);
        let p = param_named(&op, "q");
        assert_eq!(p.location, ParameterLocation::Query);
        assert_eq!(p.max_length, Some(64));
    }

    /// A template segment the config never declared carries NO declared rules and
    /// `allow_slash: false` — the path loop iterates the TEMPLATE, so it must look
    /// the `ParamDecl` up by name and fall back cleanly when there is none.
    #[test]
    fn build_operation_leaves_rules_absent_for_an_undeclared_template_segment() {
        let d = decl("/content/{version}", vec![]);
        let op = super::build_operation("/content/{version}", "GET", &d);
        let p = param_named(&op, "version");
        assert_eq!(p.pattern, None);
        assert_eq!(p.max_length, None);
        assert!(!p.allow_slash);
        assert!(p.required, "a template path parameter stays required");
    }

    /// D-11: `allow_slash` reaches the `Parameter` from the server's OWN config —
    /// the one legitimate source.
    #[test]
    fn build_operation_carries_allow_slash_from_the_config() {
        let d = decl(
            "/files/{subpath}",
            vec![ParamDecl {
                name: "subpath".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                allow_slash: true,
                ..Default::default()
            }],
        );
        let op = super::build_operation("/files/{subpath}", "GET", &d);
        assert!(param_named(&op, "subpath").allow_slash);
    }

    /// The three rule fields round-trip into the core rules type the substitution
    /// point reads.
    #[cfg(feature = "input-validation")]
    #[test]
    fn build_operation_rules_reach_placeholder_rules() {
        let d = decl(
            "/files/{subpath}",
            vec![ParamDecl {
                name: "subpath".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                pattern: Some("^[a-z/]+$".to_string()),
                max_length: Some(128),
                allow_slash: true,
                ..Default::default()
            }],
        );
        let op = super::build_operation("/files/{subpath}", "GET", &d);
        let rules = param_named(&op, "subpath").placeholder_rules();
        assert_eq!(rules.declared_pattern, Some("^[a-z/]+$"));
        assert_eq!(rules.declared_max_length, Some(128));
        assert!(rules.allow_slash);
    }
}

// -----------------------------------------------------------------------------
// Tests — Phase 90 OAPI-02a single-call HTTP synthesizer (feature `http`)
// -----------------------------------------------------------------------------

#[cfg(all(test, feature = "http"))]
mod synth_http_tests {
    use super::*;
    use crate::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
    use crate::http::{HttpConnector, HttpConnectorError, Operation};
    use pmcp::RequestHandlerExtra;
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    /// A mock [`HttpConnector`] that records the [`Operation`] it last received
    /// and returns a fixed JSON payload — so a synthesized handler can be
    /// invoked without any network.
    struct MockHttpConnector {
        last: Mutex<Option<Operation>>,
        payload: Value,
    }

    impl MockHttpConnector {
        fn new(payload: Value) -> Arc<Self> {
            Arc::new(Self {
                last: Mutex::new(None),
                payload,
            })
        }
    }

    #[async_trait]
    impl HttpConnector for MockHttpConnector {
        async fn execute(
            &self,
            operation: &Operation,
            _args: &Value,
        ) -> std::result::Result<Value, HttpConnectorError> {
            *self.last.lock().unwrap() = Some(operation.clone());
            Ok(self.payload.clone())
        }
        fn base_url(&self) -> &str {
            "https://mock.example.com"
        }
    }

    fn cfg_with_tools(tools: Vec<ToolDecl>) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools,
            ..Default::default()
        }
    }

    /// (1) A single-call `[[tools]]` with a `{id}` path param synthesizes a
    /// `ToolInfo` whose input schema marks `id` required (object envelope), and
    /// the handler (wired to a mock connector) returns the mocked JSON.
    #[tokio::test]
    async fn synth_http_single_call_path_param_required_and_handler_returns_json() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "line_status".to_string(),
            description: Some("Line status".to_string()),
            path: Some("/Line/{id}/Status".to_string()),
            method: Some("GET".to_string()),
            parameters: vec![ParamDecl {
                name: "id".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                ..Default::default()
            }],
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!({ "status": "Good Service" }));
        let out = synthesize_from_config_with_http_connector(&cfg, connector.clone())
            .expect("synthesize");
        assert_eq!(out.len(), 1);
        let (name, info, handler) = &out[0];
        assert_eq!(name, "line_status");
        let schema = &info.input_schema;
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["id"]));
        assert_eq!(schema["additionalProperties"], Value::Bool(false));

        let extra = RequestHandlerExtra::default();
        let result = handler
            .handle(json!({ "id": "victoria" }), extra)
            .await
            .expect("handle");
        assert_eq!(result, json!({ "status": "Good Service" }));

        // The operation carried the `{id}` path param as a Path parameter.
        let op = connector
            .last
            .lock()
            .unwrap()
            .clone()
            .expect("operation recorded");
        let path_params: Vec<&str> = op
            .path_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(path_params, vec!["id"]);
    }

    /// (2) A `POST` tool routes non-path args to the request body
    /// (`has_request_body` true) and the non-path param is NOT a path param.
    #[tokio::test]
    async fn synth_http_post_sets_request_body() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "create_item".to_string(),
            description: Some("Create".to_string()),
            path: Some("/items".to_string()),
            method: Some("post".to_string()),
            parameters: vec![ParamDecl {
                name: "title".to_string(),
                param_type: Some("string".to_string()),
                required: true,
                ..Default::default()
            }],
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!({ "ok": true }));
        let out = synthesize_from_config_with_http_connector(&cfg, connector.clone())
            .expect("synthesize");
        let (_, _, handler) = &out[0];
        let extra = RequestHandlerExtra::default();
        handler
            .handle(json!({ "title": "widget" }), extra)
            .await
            .expect("handle");
        let op = connector
            .last
            .lock()
            .unwrap()
            .clone()
            .expect("operation recorded");
        assert_eq!(op.method, "POST");
        assert!(op.has_request_body, "POST must carry a request body");
        assert!(op.path_parameters().is_empty());
    }

    /// (3) A tool with path + query params lands them in the right schema slots /
    /// `Operation` parameter locations.
    #[tokio::test]
    async fn synth_http_path_and_query_param_slots() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "search".to_string(),
            description: Some("Search".to_string()),
            path: Some("/repos/{owner}/issues".to_string()),
            method: Some("GET".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "owner".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    ..Default::default()
                },
                ParamDecl {
                    name: "state".to_string(),
                    param_type: Some("string".to_string()),
                    required: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!([]));
        let out = synthesize_from_config_with_http_connector(&cfg, connector.clone())
            .expect("synthesize");
        let (_, _, handler) = &out[0];
        let extra = RequestHandlerExtra::default();
        handler
            .handle(json!({ "owner": "rust-lang", "state": "open" }), extra)
            .await
            .expect("handle");
        let op = connector
            .last
            .lock()
            .unwrap()
            .clone()
            .expect("operation recorded");
        let path_params: Vec<&str> = op
            .path_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(path_params, vec!["owner"]);
        let query_params: Vec<&str> = op
            .query_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(query_params, vec!["state"]);
    }

    /// (4) A per-tool `base_url` is reflected in the synthesized `Operation`
    /// (Codex MEDIUM — not dropped).
    #[tokio::test]
    async fn synth_http_per_tool_base_url_reflected() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "other_host".to_string(),
            description: Some("Other host".to_string()),
            path: Some("/ping".to_string()),
            method: Some("GET".to_string()),
            base_url: Some("https://other.example.com/v2".to_string()),
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!({ "pong": true }));
        let out = synthesize_from_config_with_http_connector(&cfg, connector.clone())
            .expect("synthesize");
        let (_, _, handler) = &out[0];
        let extra = RequestHandlerExtra::default();
        handler.handle(json!({}), extra).await.expect("handle");
        let op = connector
            .last
            .lock()
            .unwrap()
            .clone()
            .expect("operation recorded");
        assert_eq!(
            op.base_url.as_deref(),
            Some("https://other.example.com/v2"),
            "per-tool base_url must be reflected on the Operation, not dropped"
        );
    }

    /// (5) NEGATIVE: a `[[tools]]` missing `method` (and without `script`) is
    /// rejected with a typed `ToolkitError` (T-90-03-04 — never silently
    /// registered).
    #[test]
    fn synth_http_missing_method_rejected() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "broken".to_string(),
            description: Some("missing method".to_string()),
            path: Some("/items".to_string()),
            method: None,
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!(null));
        let err = synthesize_from_config_with_http_connector(&cfg, connector)
            .err()
            .expect("ill-formed single-call tool must be rejected");
        assert!(matches!(err, ToolkitError::Synth(_)));
    }

    /// (5b) NEGATIVE: on the single-call-only entry point, a `script` tool is
    /// rejected with a typed `ToolkitError` pointing at the `openapi-code-mode`
    /// script path — NOT a silent skip, NOT a panic. (The OpenAPI Code Mode
    /// entry point `synthesize_from_config_with_http_connector_and_scripts`
    /// synthesizes a real `ScriptToolHandler` — proven in `script_tool` tests.)
    #[test]
    fn synth_http_script_tool_without_engine_is_rejected() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "scripted".to_string(),
            description: Some("script tool".to_string()),
            script: Some("await api.get('/x')".to_string()),
            ..Default::default()
        }]);
        let connector = MockHttpConnector::new(json!(null));
        let err = synthesize_from_config_with_http_connector(&cfg, connector)
            .err()
            .expect("script tool on the single-call-only entry point must be rejected");
        match err {
            ToolkitError::Synth(msg) => {
                assert!(
                    msg.contains("openapi-code-mode"),
                    "seam message must point at the openapi-code-mode script path: {msg}"
                );
            },
            other => panic!("expected Synth error, got {other:?}"),
        }
    }
}
