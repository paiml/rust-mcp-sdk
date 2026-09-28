// Net-new code for Phase 90 OAPI-04 / OAPI-02a (D-03 — spec OPTIONAL at runtime).
// BODY lifted from the pmcp-run OpenAPI reference
// (`mcp-openapi-server-core::schema::parser`): the openapiv3 parse + serde_yaml
// fallback + the per-location parameter extraction. SHAPE adapted to the
// toolkit-owned `Operation` model (the authoritative request type, re-exported
// from `http::mod`).

//! OpenAPI schema parser — the AUTHORITATIVE home of [`Operation`] (OAPI-04).
//!
//! Parses an OpenAPI 3.0/3.1 document (JSON **or** YAML) into an indexed
//! [`OpenApiSchema`] whose [`Operation`] values the single-call synthesizer
//! (Plan 03) and the code-mode executor (Plan 04/05) consume. The parser is the
//! producer of [`Operation`], so the canonical struct lives HERE and is
//! re-exported from [`crate::http`] (mod.rs) — the type path
//! `crate::http::Operation` stays stable across every plan (Codex MEDIUM: one
//! home from day one).
//!
//! # Runtime-optional (D-03)
//!
//! A spec is OPTIONAL at runtime. [`OpenApiSchema::parse`] is never called unless
//! the operator supplies a `--spec` document; the binary threads the result as an
//! `Option<OpenApiSchema>`, and a curated-only server (single-call `[[tools]]`
//! with explicit `path`/`method`) boots with `None`. Contrast the SQL `--schema`
//! input which is effectively required. The spec, when present, surfaces two
//! ways: (a) verbatim spec text for the code-mode `api_schema` resource, and
//! (b) parsed [`Operation`] values for richer tool synthesis.

// Why: HTTP method names ("GET", "POST") and product nouns ("OpenAPI") are
// proper nouns / acronyms clippy::doc_markdown otherwise flags for back-ticks.
#![allow(clippy::doc_markdown)]

use std::collections::HashMap;
use std::path::Path;

use openapiv3::{OpenAPI, ReferenceOr};
use serde::{Deserialize, Serialize};

use super::HttpConnectorError;

// Phase 128 D4(b). The RETURN TYPE of `Parameter::placeholder_rules` comes from a
// feature-gated core module (`pmcp/schema-validation`, forwarded by the toolkit's
// `input-validation`), and bare `http` does NOT enable it — so the import and the
// method that names the type are both gated. Gating only the CALL SITE in
// `http/client.rs` would leave `--no-default-features --features http` broken.
#[cfg(feature = "input-validation")]
use pmcp::server::schema_validation::PlaceholderRules;

/// An extracted REST operation backed by an OpenAPI definition.
///
/// The AUTHORITATIVE request model the [`crate::http::HttpConnector::execute`]
/// signature names (re-exported from [`crate::http`]). Plan 01 defined a minimal
/// shape; Plan 03 makes this the canonical home and populates these values from
/// an `openapiv3` parse. The shape mirrors the pmcp-run reference
/// `mcp-openapi-server-core::schema::Operation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// HTTP method (`GET`, `POST`, ...).
    pub method: String,

    /// Path template, e.g. `"/users/{id}"`.
    pub path: String,

    /// Input parameters (path / query / header).
    #[serde(default)]
    pub parameters: Vec<Parameter>,

    /// Whether this operation expects a request body.
    #[serde(default)]
    pub has_request_body: bool,

    /// Per-tool base-URL override (D-06 / Codex MEDIUM). When `Some`, this
    /// operation targets the given host instead of the configured `[backend]`
    /// `base_url`. Carried so the synthesizer NEVER silently drops a per-tool
    /// `base_url`; `None` means inherit the connector's configured base.
    #[serde(default)]
    pub base_url: Option<String>,
}

impl Operation {
    /// Path parameters (the `{...}` segments of [`Operation::path`]).
    #[must_use]
    pub fn path_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Path)
            .collect()
    }

    /// Query parameters.
    #[must_use]
    pub fn query_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Query)
            .collect()
    }

    /// Header parameters.
    #[must_use]
    pub fn header_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Header)
            .collect()
    }

    /// Body parameters — the declared parameters that travel as fields of the
    /// JSON request body (Phase 128 CR-02).
    ///
    /// Non-empty only when [`Self::has_request_body`] is set, because
    /// `crate::tools::build_operation` derives both from the one
    /// `crate::config::method_carries_request_body` predicate.
    #[must_use]
    pub fn body_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Body)
            .collect()
    }
}

/// A single OpenAPI operation parameter.
///
/// # The three rule fields (Phase 128, D4(b))
///
/// [`Self::pattern`], [`Self::max_length`] and [`Self::allow_slash`] carry the
/// DECLARED narrowing for this parameter to the substitution point in
/// [`crate::http::HttpClient`], where
/// `pmcp::server::schema_validation::validate_path_placeholder` reads them through
/// `Parameter::placeholder_rules`. They are deliberately NOT feature-gated — they are
/// plain scalars, they are `#[serde(default)]`, and gating them would make this
/// struct's serialized shape feature-dependent, which is a worse break than gating
/// the accessor that names a feature-gated type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name (matches the `{name}` placeholder for path params).
    pub name: String,

    /// Where the parameter is carried in the request.
    pub location: ParameterLocation,

    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,

    /// Declared regular expression the value must match (Phase 128, D4(b)).
    ///
    /// NARROWS the unconditional character floor; it never replaces it (D-10).
    /// Populated from `[[tools.parameters]] pattern` for a curated tool and from a
    /// spec parameter's own string schema for a spec-driven one.
    #[serde(default)]
    pub pattern: Option<String>,

    /// Declared maximum length in Unicode code points (Phase 128, D4(b)).
    ///
    /// NARROWS the always-on `pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH`
    /// cap; a declared value above that constant cannot widen it (D-08).
    #[serde(default)]
    pub max_length: Option<u64>,

    /// Permit `/` inside this parameter's substituted value (Phase 128, D-11).
    ///
    /// The ONLY legitimate source is the server's own
    /// `[[tools.parameters]] allow_slash`. A parsed OpenAPI document must never set
    /// it — see `Parameter::placeholder_rules` and
    /// `crate::config::ParamDecl::allow_slash` for why.
    #[serde(default)]
    pub allow_slash: bool,
}

impl Parameter {
    /// Construct a parameter (test/parser convenience).
    ///
    /// The Phase 128 rule fields default to "no declared narrowing", so every
    /// pre-Phase-128 call site keeps compiling unchanged and gets the
    /// unconditional floor plus the always-on cap. Attach declared rules with
    /// [`Self::with_rules`].
    #[must_use]
    pub fn new(name: impl Into<String>, location: ParameterLocation, required: bool) -> Self {
        Self {
            name: name.into(),
            location,
            required,
            pattern: None,
            max_length: None,
            allow_slash: false,
        }
    }

    /// Attach the declared placeholder rules (Phase 128, D4(b)).
    ///
    /// A consuming builder rather than three setters, so the three values that
    /// travel together are set together.
    #[must_use]
    pub fn with_rules(
        mut self,
        pattern: Option<String>,
        max_length: Option<u64>,
        allow_slash: bool,
    ) -> Self {
        self.pattern = pattern;
        self.max_length = max_length;
        self.allow_slash = allow_slash;
        self
    }

    /// This parameter's declared rules as a
    /// `pmcp::server::schema_validation::PlaceholderRules` (Phase 128, D4(b)).
    ///
    /// # D-10 — the returned rules NARROW, they never replace
    ///
    /// `validate_path_placeholder` applies its unconditional character floor and
    /// its always-on length cap FIRST and consults these rules afterwards, so a
    /// permissive declared pattern such as `^.*$` cannot switch the injection
    /// check off. A declared `max_length` above `PLACEHOLDER_MAX_LENGTH` likewise
    /// cannot widen the constant.
    ///
    /// # D-11 — `allow_slash` is config-only
    ///
    /// The returned `allow_slash` comes from the server's own
    /// `[[tools.parameters]]` declaration and from nowhere else. An OpenAPI
    /// document's reserved-expansion keyword must NEVER be wired to it: a spec is
    /// third-party content baked into a package, that keyword describes
    /// percent-encoding latitude in the spec author's serialization rules rather
    /// than permission to restructure the request target, and it is widely
    /// copy-pasted without intent. The parser in this module therefore leaves
    /// `allow_slash` false unconditionally.
    ///
    /// # The curated template parser recognizes WHOLE-SEGMENT placeholders only
    ///
    /// On the curated single-call surface a placeholder is a whole `/`-delimited
    /// segment: `crate::config::path_placeholder_names` matches `{name}` spanning
    /// an entire segment and nothing else. So `/search/{a}{b}` is NOT two
    /// placeholders (it parses as the single name `a}{b`) and `/prefix-{id}` is not
    /// recognized as carrying a placeholder at all. Attaching rules to a
    /// `Parameter` does not change that parse. Both shapes are refused at CONFIG
    /// time by `crate::config::ServerConfig::validate`, so neither can reach
    /// runtime from a `[[tools]]` declaration — but a spec-derived operation is not
    /// config-validated, which is why the composed-path check at the tail of
    /// substitution is what closes a residual `{`/`}` there.
    #[cfg(feature = "input-validation")]
    #[must_use]
    pub fn placeholder_rules(&self) -> PlaceholderRules<'_> {
        PlaceholderRules::default()
            .with_pattern(self.pattern.as_deref())
            // A declared length that does not fit a `usize` contributes NO
            // narrowing, which is the safe direction: the always-on module cap
            // still applies.
            .with_max_length(self.max_length.and_then(|m| usize::try_from(m).ok()))
            .allowing_slash(self.allow_slash)
    }
}

/// Where an [`Operation`] parameter is carried in the outgoing request.
///
/// Every variant has exactly one consumer in
/// [`crate::http::HttpClient::execute`], which is what makes this enum a routing
/// decision rather than a label: [`Self::Path`] is read by `substitute_path`,
/// [`Self::Query`] by `build_query`, [`Self::Header`] by `build_headers` and
/// [`Self::Body`] by `build_body`. A parameter carrying a location whose consumer
/// does not run is a parameter that is silently dropped, which is the defect
/// Phase 128 CR-02 recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    /// Substituted into the path template (`/users/{id}`).
    Path,
    /// Appended to the query string.
    Query,
    /// Sent as a request header.
    Header,
    /// Carried as a field of the JSON request body (Phase 128 CR-02).
    ///
    /// Only an operation whose method carries a request body
    /// (`POST` / `PUT` / `PATCH` — see
    /// `crate::config::method_carries_request_body`) may place a parameter here;
    /// `crate::tools::build_operation` reads that one predicate for BOTH this
    /// assignment and [`Operation::has_request_body`], so a `Body`-located
    /// parameter on a body-less request is not constructible.
    ///
    /// No OpenAPI `in:` value maps here — the spec models a request body as a
    /// separate `requestBody` object, not as a parameter — so this variant is
    /// reached only from a curated `[[tools]]` declaration.
    Body,
}

/// A parsed OpenAPI document with its [`Operation`] values indexed by
/// `(path, METHOD)` (OAPI-04 / D-03).
///
/// Runtime-OPTIONAL: the binary holds an `Option<OpenApiSchema>` and only parses
/// when the operator supplies a spec. Retains the raw spec text so the code-mode
/// `api_schema` resource (D-03 surface (a)) can serve it verbatim.
#[derive(Debug, Clone)]
pub struct OpenApiSchema {
    /// Raw spec text, retained verbatim for the code-mode `api_schema` resource.
    spec_text: String,

    /// Extracted operations in document order.
    operations: Vec<Operation>,

    /// `(path, METHOD)` → index into [`Self::operations`].
    by_path: HashMap<(String, String), usize>,
}

impl OpenApiSchema {
    /// Parse an OpenAPI spec from JSON, falling back to YAML.
    ///
    /// Tries `serde_json` first (the common machine-emitted shape), then
    /// `serde_yaml`. The retained spec text is `text` verbatim so the
    /// `api_schema` resource serves exactly what the operator supplied.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when the text is neither valid
    /// OpenAPI JSON nor YAML. The error message carries a static reason only —
    /// it does NOT echo the (admin-authored) spec body (T-90-03-03 discipline).
    pub fn parse(text: &str) -> Result<Self, HttpConnectorError> {
        let spec: OpenAPI = serde_json::from_str(text)
            .or_else(|_| serde_yaml::from_str(text))
            .map_err(|_| {
                HttpConnectorError::Backend("OpenAPI spec is not valid JSON or YAML".to_string())
            })?;
        Self::from_spec(spec, text.to_string())
    }

    /// Read and parse an OpenAPI spec from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when the file cannot be read or
    /// the contents do not parse. The error message carries a static reason and
    /// never echoes the file path or spec body (T-90-03-03 discipline).
    pub fn parse_path(path: &Path) -> Result<Self, HttpConnectorError> {
        let text = std::fs::read_to_string(path).map_err(|_| {
            HttpConnectorError::Backend("could not read OpenAPI spec file".to_string())
        })?;
        Self::parse(&text)
    }

    /// Build the indexed schema from an already-parsed `openapiv3` document.
    fn from_spec(spec: OpenAPI, spec_text: String) -> Result<Self, HttpConnectorError> {
        let mut operations = Vec::new();
        let mut by_path = HashMap::new();

        for (path, path_item) in &spec.paths.paths {
            let item = match path_item {
                ReferenceOr::Item(item) => item,
                // $ref path items are skipped (reference resolution not required
                // for the single-call surface — admin-authored specs inline).
                ReferenceOr::Reference { .. } => continue,
            };

            let path_level: Vec<Parameter> = item
                .parameters
                .iter()
                .filter_map(convert_parameter)
                .collect();

            let methods = [
                ("GET", &item.get),
                ("POST", &item.post),
                ("PUT", &item.put),
                ("PATCH", &item.patch),
                ("DELETE", &item.delete),
                ("HEAD", &item.head),
                ("OPTIONS", &item.options),
            ];

            for (method, op_opt) in methods {
                if let Some(op) = op_opt {
                    let operation = extract_operation(path, method, op, &path_level);
                    let idx = operations.len();
                    by_path.insert((path.clone(), method.to_string()), idx);
                    operations.push(operation);
                }
            }
        }

        Ok(Self {
            spec_text,
            operations,
            by_path,
        })
    }

    /// All extracted operations, in document order.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Look up an operation by path template and HTTP method (case-insensitive
    /// on the method).
    #[must_use]
    pub fn operation_for(&self, path: &str, method: &str) -> Option<&Operation> {
        self.by_path
            .get(&(path.to_string(), method.to_uppercase()))
            .and_then(|&idx| self.operations.get(idx))
    }

    /// The raw spec text, for the code-mode `api_schema` resource (D-03 (a)).
    #[must_use]
    pub fn spec_text(&self) -> &str {
        &self.spec_text
    }
}

/// Merge path-level and operation-level parameters (operation-level wins on a
/// name collision) into the toolkit [`Operation`] model.
fn extract_operation(
    path: &str,
    method: &str,
    op: &openapiv3::Operation,
    path_level: &[Parameter],
) -> Operation {
    let mut parameters: Vec<Parameter> = path_level.to_vec();
    for param_ref in &op.parameters {
        if let Some(p) = convert_parameter(param_ref) {
            if let Some(idx) = parameters.iter().position(|x| x.name == p.name) {
                parameters[idx] = p;
            } else {
                parameters.push(p);
            }
        }
    }

    Operation {
        method: method.to_string(),
        path: path.to_string(),
        parameters,
        has_request_body: op.request_body.is_some(),
        base_url: None,
    }
}

/// Convert an `openapiv3` parameter into the toolkit [`Parameter`] model.
///
/// Cookie parameters and unresolved `$ref` parameters are dropped (the
/// single-call surface carries path / query / header only); path parameters are
/// always required.
fn convert_parameter(param_ref: &ReferenceOr<openapiv3::Parameter>) -> Option<Parameter> {
    let param = match param_ref {
        ReferenceOr::Item(p) => p,
        ReferenceOr::Reference { .. } => return None,
    };
    let (location, parameter_data, required) = match param {
        openapiv3::Parameter::Query { parameter_data, .. } => (
            ParameterLocation::Query,
            parameter_data,
            parameter_data.required,
        ),
        // A path parameter is always required, per the OpenAPI spec itself.
        openapiv3::Parameter::Path { parameter_data, .. } => {
            (ParameterLocation::Path, parameter_data, true)
        },
        openapiv3::Parameter::Header { parameter_data, .. } => (
            ParameterLocation::Header,
            parameter_data,
            parameter_data.required,
        ),
        openapiv3::Parameter::Cookie { .. } => return None,
    };
    let (pattern, max_length) = declared_string_rules(parameter_data);
    Some(
        Parameter::new(parameter_data.name.clone(), location, required)
            // Phase 128 D-11: the third argument is `false` UNCONDITIONALLY, and it
            // must stay that way. `allow_slash` lifts the path-separator refusal, so
            // the only legitimate source for it is the server operator's own
            // `[[tools.parameters]]` declaration. NO keyword read from a parsed
            // document may be wired here — including the one that grants latitude
            // over reserved characters in a serialization, which describes
            // percent-encoding rather than permission to restructure the request
            // target and is widely copy-pasted without intent. A spec is
            // third-party content baked into a package; it may NARROW the floor
            // (the two values above) and may never widen it. See
            // `crate::config::ParamDecl::allow_slash`, which names the keyword and
            // states the rule in full.
            .with_rules(pattern, max_length, false),
    )
}

/// The `pattern` and `max_length` a spec parameter's OWN schema declares
/// (Phase 128, D4(b)).
///
/// # Which schema shapes narrow, and which deliberately do not
///
/// ONLY a direct `ReferenceOr::Item` whose `SchemaKind` is
/// `Type(Type::String(..))` contributes, and it contributes exactly that string
/// type's own `pattern` and `max_length`. Every other shape yields `(None, None)`:
/// an unresolved `$ref`, a `oneOf` / `allOf` / `anyOf` composition, a non-string
/// type, and the `content` form (which is how a parameter with no direct `schema`
/// is represented).
///
/// That is deliberately conservative and it is conservative in the SAFE direction
/// per D-10: a shape this function does not understand contributes NO narrowing,
/// which leaves the unconditional character floor and the always-on length cap
/// fully intact. Guessing at a `$ref` target — this parser resolves no references —
/// could narrow from the wrong schema, which is strictly worse than narrowing from
/// nothing.
fn declared_string_rules(
    parameter_data: &openapiv3::ParameterData,
) -> (Option<String>, Option<u64>) {
    let openapiv3::ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) =
        &parameter_data.format
    else {
        return (None, None);
    };
    let openapiv3::SchemaKind::Type(openapiv3::Type::String(string_type)) = &schema.schema_kind
    else {
        return (None, None);
    };
    (
        string_type.pattern.clone(),
        // A declared length that does not fit a `u64` contributes nothing, which is
        // the safe direction: the always-on module cap still applies.
        string_type.max_length.and_then(|m| u64::try_from(m).ok()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"
    {
        "openapi": "3.0.0",
        "info": { "title": "Test API", "version": "1.0.0" },
        "paths": {
            "/users/{id}": {
                "get": {
                    "operationId": "getUser",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true,
                          "schema": { "type": "string" } },
                        { "name": "verbose", "in": "query", "required": false,
                          "schema": { "type": "boolean" } }
                    ],
                    "responses": { "200": { "description": "OK" } }
                }
            }
        }
    }
    "#;

    const SAMPLE_YAML: &str = r#"
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /users/{id}:
    get:
      operationId: getUser
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: verbose
          in: query
          required: false
          schema:
            type: boolean
      responses:
        '200':
          description: OK
"#;

    fn assert_get_user(schema: &OpenApiSchema) {
        let op = schema
            .operation_for("/users/{id}", "GET")
            .expect("getUser operation present");
        assert_eq!(op.method, "GET");
        assert_eq!(op.path, "/users/{id}");
        let path_params: Vec<&str> = op
            .path_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(path_params, vec!["id"]);
        let query_params: Vec<&str> = op
            .query_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(query_params, vec!["verbose"]);
    }

    #[test]
    fn schema_parse_json_extracts_operation_and_path_params() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        assert_get_user(&schema);
        assert_eq!(schema.operations().len(), 1);
    }

    #[test]
    fn schema_parse_yaml_matches_json() {
        let schema = OpenApiSchema::parse(SAMPLE_YAML).expect("parse YAML");
        assert_get_user(&schema);
    }

    #[test]
    fn schema_parse_retains_spec_text_for_resource() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        // D-03 surface (a): the raw text is served verbatim by api_schema.
        assert_eq!(schema.spec_text(), SAMPLE_JSON);
    }

    #[test]
    fn schema_parse_method_case_insensitive_lookup() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        assert!(schema.operation_for("/users/{id}", "get").is_some());
        assert!(schema.operation_for("/users/{id}", "GET").is_some());
        assert!(schema.operation_for("/users/{id}", "POST").is_none());
    }

    #[test]
    fn schema_parse_malformed_returns_typed_error_no_panic() {
        let err = OpenApiSchema::parse("this is neither json nor yaml: [unclosed").unwrap_err();
        // Typed error, no panic.
        assert!(matches!(err, HttpConnectorError::Backend(_)));
    }

    // -- Phase 128 D4(b): the declared placeholder rules on `Parameter` ---------

    /// A spec document exercising every `openapiv3` parameter-schema SHAPE the
    /// narrowing rule has to decide about: a direct string type (narrows), an
    /// unresolved `$ref` (floor-only), a non-string type (floor-only), a composed
    /// `oneOf` (floor-only), and the `content` form, which is the representable
    /// stand-in for "no direct schema" — `ParameterData::format` is a required
    /// flattened field, so a parameter carrying neither `schema` nor `content` does
    /// not deserialize at all and cannot be exercised here.
    // `r##"…"##`, not `r#"…"#`: the document contains the `$ref` value
    // `"#/components/…`, whose `"#` would otherwise close the raw string.
    const SHAPES_JSON: &str = r##"
    {
        "openapi": "3.0.0",
        "info": { "title": "Shapes", "version": "1.0.0" },
        "paths": {
            "/content/{version}/CUI/{cui}": {
                "get": {
                    "operationId": "getContent",
                    "parameters": [
                        { "name": "version", "in": "path", "required": true,
                          "schema": { "type": "string", "pattern": "^[0-9]{4}[A-Z]{2}$",
                                      "maxLength": 64 } },
                        { "name": "cui", "in": "path", "required": true,
                          "schema": { "$ref": "#/components/schemas/Cui" } },
                        { "name": "verbose", "in": "query", "required": false,
                          "schema": { "type": "boolean" } },
                        { "name": "composed", "in": "query", "required": false,
                          "schema": { "oneOf": [ { "type": "string" },
                                                 { "type": "integer" } ] } },
                        { "name": "ctyped", "in": "query", "required": false,
                          "content": { "application/json": { "schema": { "type": "string",
                                                                         "maxLength": 8 } } } }
                    ],
                    "responses": { "200": { "description": "OK" } }
                }
            }
        },
        "components": {
            "schemas": { "Cui": { "type": "string", "maxLength": 12 } }
        }
    }
    "##;

    fn shapes_param(name: &str) -> Parameter {
        OpenApiSchema::parse(SHAPES_JSON)
            .expect("parse shapes spec")
            .operation_for("/content/{version}/CUI/{cui}", "GET")
            .expect("getContent present")
            .parameters
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("parameter {name} present"))
    }

    /// `Parameter::new` keeps its three-argument signature and defaults every rule
    /// field to "no declared narrowing".
    #[test]
    fn schema_parameter_new_defaults_the_rule_fields() {
        let p = Parameter::new("id", ParameterLocation::Path, true);
        assert_eq!(p.name, "id");
        assert!(p.required);
        assert_eq!(p.pattern, None);
        assert_eq!(p.max_length, None);
        assert!(!p.allow_slash);
    }

    /// `with_rules` carries all three values, and `placeholder_rules` round-trips
    /// them into the core type the substitution point reads.
    #[cfg(feature = "input-validation")]
    #[test]
    fn schema_parameter_with_rules_round_trips_into_placeholder_rules() {
        let p = Parameter::new("region", ParameterLocation::Path, true).with_rules(
            Some("^C[0-9]+$".to_string()),
            Some(64),
            true,
        );
        let rules = p.placeholder_rules();
        assert_eq!(rules.declared_pattern, Some("^C[0-9]+$"));
        assert_eq!(rules.declared_max_length, Some(64));
        assert!(rules.allow_slash);
    }

    /// A direct `ReferenceOr::Item` whose `SchemaKind` is a string type contributes
    /// BOTH `pattern` and `max_length` — the only shape that narrows.
    #[test]
    fn schema_parser_narrows_from_a_direct_string_schema() {
        let version = shapes_param("version");
        assert_eq!(version.pattern.as_deref(), Some("^[0-9]{4}[A-Z]{2}$"));
        assert_eq!(version.max_length, Some(64));
    }

    /// An unresolved `$ref` parameter schema yields FLOOR-ONLY rules. Deliberately
    /// conservative: guessing at the `$ref` target could narrow from the wrong
    /// schema, whereas contributing nothing leaves the unconditional floor and the
    /// always-on cap intact.
    #[test]
    fn schema_parser_yields_floor_only_rules_for_a_ref_schema() {
        let cui = shapes_param("cui");
        assert_eq!(cui.pattern, None);
        assert_eq!(
            cui.max_length, None,
            "the `$ref` target's maxLength (12) must NOT be read"
        );
    }

    /// A non-string schema type yields floor-only rules.
    #[test]
    fn schema_parser_yields_floor_only_rules_for_a_non_string_schema() {
        let verbose = shapes_param("verbose");
        assert_eq!(verbose.pattern, None);
        assert_eq!(verbose.max_length, None);
    }

    /// A composed (`oneOf`) schema yields floor-only rules.
    #[test]
    fn schema_parser_yields_floor_only_rules_for_a_composed_schema() {
        let composed = shapes_param("composed");
        assert_eq!(composed.pattern, None);
        assert_eq!(composed.max_length, None);
    }

    /// The `content` form — the representable stand-in for a parameter with no
    /// direct schema — yields floor-only rules.
    #[test]
    fn schema_parser_yields_floor_only_rules_for_a_content_form_parameter() {
        let ctyped = shapes_param("ctyped");
        assert_eq!(ctyped.pattern, None);
        assert_eq!(
            ctyped.max_length, None,
            "the media-type schema's maxLength (8) must NOT be read"
        );
    }

    /// D-11: `allow_slash` is false for EVERY spec-derived parameter, whatever the
    /// document says. The end-to-end row that drives a document declaring the
    /// reserved-expansion keyword lives in
    /// `tests/curated_path_injection.rs`, because this file carries a
    /// verification gate forbidding that keyword's name outside a comment.
    #[test]
    fn schema_parser_leaves_allow_slash_false_for_every_spec_parameter() {
        let op = OpenApiSchema::parse(SHAPES_JSON)
            .expect("parse shapes spec")
            .operation_for("/content/{version}/CUI/{cui}", "GET")
            .expect("getContent present")
            .clone();
        assert_eq!(op.parameters.len(), 5);
        for p in &op.parameters {
            assert!(
                !p.allow_slash,
                "a spec must never widen the path floor: {}",
                p.name
            );
        }
    }

    /// T-90-03-03: the parser error MUST NOT echo the spec body (redaction
    /// discipline kept consistent with the connector, though specs carry no
    /// creds).
    #[test]
    fn test_schema_parse_error_display_no_secret() {
        let secret_marker = "SUPER_SECRET_TOKEN_abc123";
        let bad_spec = format!("not-a-spec {secret_marker} [");
        let err = OpenApiSchema::parse(&bad_spec).unwrap_err();
        let rendered = format!("{err}");
        assert!(
            !rendered.contains(secret_marker),
            "parser error must not echo the spec body; got {rendered:?}"
        );
    }
}
