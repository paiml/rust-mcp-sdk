// Originated from pmcp-run/built-in/shared/mcp-server-common/src/config.rs
// (https://github.com/guyernest/pmcp-run). Lifted into rust-mcp-sdk for Phase 83.

//! `ServerConfig` + sub-sections. Strict `#[serde(deny_unknown_fields)]` per D-13.
//!
//! # Strict-parse discipline (D-13)
//!
//! Every struct in this module carries `#[serde(deny_unknown_fields)]`. A typo
//! in any key (e.g. `auto_aprove_levels` for `auto_approve_levels`) is a
//! **parse error**, not a silent default. This is the defence-in-depth path
//! against the Tampering threat documented in `83-04-PLAN.md` T-83-04-02 —
//! mis-spelled keys MUST NOT degrade security policy.
//!
//! # REF-01 superset invariant
//!
//! `ServerConfig` is a strict **superset** of every key emitted by the three
//! reference config.tomls (`tests/fixtures/{open-images,imdb,msr-vtt}-config.toml`,
//! lifted in Plan 01 Task 4). When a fixture grows a new key, the toolkit grows
//! a new field — typed if known, `toml::Value` if heterogeneous. The invariant
//! is enforced empirically by the [`tests/reference_configs.rs`] integration
//! test (REF-01 superset, D-13, ROADMAP SC-2).
//!
//! **Anti-pattern (RESEARCH §Pitfall 1, PATTERNS §8):** Do NOT loosen
//! `deny_unknown_fields` to make a fixture parse. Always ADD the missing field.
//!
//! # Three entry points
//!
//! | Method | Returns | Use case |
//! |--------|---------|----------|
//! | [`ServerConfig::from_toml`] | `Result<Self, ToolkitError::Parse>` | Programmatic partial-config merge; no semantic checks |
//! | [`ServerConfig::validate`] | `Result<(), ConfigValidationError>` | Post-parse semantic check (run after a merge) |
//! | [`ServerConfig::from_toml_strict_validated`] | `Result<Self, ToolkitError>` | Production entry: parse + validate in one call |
//!
//! Per Phase 83 review R8, `validate()` exists because the `Default` impls on
//! `ServerSection` etc. would otherwise let `[server]` typos land empty
//! `name`/`version` strings without an error. The strict-validated convenience
//! is what production callers should reach for.
//!
//! REF-01 superset enumeration (from `tests/fixtures/{open-images,imdb,msr-vtt,reference}-config.toml`;
//! the SQLite Chinook `reference-config.toml` was lifted in Plan 85-01):
//!
//! ```text
//! [server]            : id, name, description, type, version, is_reference
//! [metadata]          : display_name, short_description, description, tags, author, visibility
//! [database]          : type, database, output_location, workgroup, query_timeout_ms,
//!                       url, file_path, [[database.tables]], [database.pool]
//! [[database.tables]] : name, description
//! [database.pool]     : max_connections, connection_timeout_seconds
//! [code_mode]         : enabled, server_id, allow_writes, allow_deletes, allow_ddl,
//!                       require_limit, max_limit, blocked_tables, sensitive_columns,
//!                       auto_approve_levels, token_ttl_seconds, token_secret,
//!                       [code_mode.limits]
//! [code_mode.limits]  : max_tables_per_query, max_join_depth, max_subquery_depth
//! [shared_policy_store] : creates_shared_store, export_to_ssm, ssm_path, templates
//! [[tools]]           : name, description, sql, ui_resource_uri,
//!                       [[tools.parameters]], [tools.annotations]
//! [[tools.parameters]] : name, type, description, required, default, max_length,
//!                       minimum, maximum, enum
//! [tools.annotations] : read_only_hint, destructive_hint, idempotent_hint,
//!                       open_world_hint, cost_hint
//! [[prompts]]         : name, description, include_resources, arguments
//! [[resources]]       : uri, name, description, mime_type, content
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{ConfigValidationError, ConfigWarning, Result, ToolkitError};

// -----------------------------------------------------------------------------
// Top-level
// -----------------------------------------------------------------------------

/// Top-level `pmcp-server-toolkit` configuration parsed from a `config.toml`.
///
/// One struct parses the entire file in one shot (per D-13). All sub-sections
/// carry `#[serde(deny_unknown_fields)]` — a typo anywhere in the file is a
/// hard parse error.
///
/// # Entry points
///
/// Use [`ServerConfig::from_toml_strict_validated`] for production callers.
/// [`ServerConfig::from_toml`] is the no-validation variant for programmatic
/// merges; [`ServerConfig::validate`] runs the semantic checks separately.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::config::ServerConfig;
///
/// let toml = r#"
///     [server]
///     name = "demo"
///     version = "0.1.0"
/// "#;
/// let cfg = ServerConfig::from_toml_strict_validated(toml)
///     .expect("valid minimum config");
/// assert_eq!(cfg.server.name, "demo");
/// assert_eq!(cfg.server.version, "0.1.0");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// `[server]` — identity and version metadata.
    #[serde(default)]
    pub server: ServerSection,

    /// `[metadata]` — admin-facing display defaults.
    #[serde(default)]
    pub metadata: MetadataSection,

    /// `[database]` — backend connection + tables.
    #[serde(default)]
    pub database: DatabaseSection,

    /// `[backend]` (optional, `http` feature) — OpenAPI/REST HTTP backend
    /// declaration (`base_url` + `[backend.auth]` + `[backend.http]`).
    ///
    /// Additive per the REF-01 superset invariant (D-06): a pure-SQL config
    /// omits `[backend]` and this field parses to `None`. The whole section is
    /// gated behind the `http` feature — a no-http build has no OpenAPI backend,
    /// so exposing an unusable stub type would be misleading. See
    /// [`BackendSection`].
    #[cfg(feature = "http")]
    #[serde(default)]
    pub backend: Option<BackendSection>,

    /// `[code_mode]` (optional) — code-mode policy and limits.
    #[serde(default)]
    pub code_mode: Option<CodeModeSection>,

    /// `[[tools]]` — declarative tool surface (TOML-defined handlers).
    #[serde(default)]
    pub tools: Vec<ToolDecl>,

    /// `[[config_slots]]` — declared config slots the TARGET environment must
    /// fill (PKG-03). Additive per the REF-01 superset invariant: a config
    /// omitting the block parses to an empty vec.
    ///
    /// Deliberately NOT gated on the `http` feature — a SQL or workbook Shape A
    /// server declares slots too, and gating it would make the field vanish in
    /// the toolkit's own default build.
    #[serde(default)]
    pub config_slots: Vec<ConfigSlotDecl>,

    /// `[[prompts]]` — declarative prompt surface.
    #[serde(default)]
    pub prompts: Vec<PromptDecl>,

    /// `[[resources]]` — declarative resource surface.
    #[serde(default)]
    pub resources: Vec<ResourceDecl>,

    /// `[shared_policy_store]` (optional) — AVP/Cedar shared-policy-store
    /// declaration emitted by the reference SQL server (`is_reference = true`),
    /// which provisions the policy store all sibling SQL servers attach to.
    /// Additive per the REF-01 superset invariant (Plan 85-01); parsed
    /// verbatim — the toolkit does not provision SSM at parse time.
    #[serde(default)]
    pub shared_policy_store: Option<SharedPolicyStoreSection>,
}

impl ServerConfig {
    /// Parse `ServerConfig` from a TOML config string.
    ///
    /// Performs **strict parsing** (`#[serde(deny_unknown_fields)]` on every
    /// section, per D-13). Does **not** run semantic validation — callers
    /// wanting required-field guarantees should use
    /// [`Self::from_toml_strict_validated`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::Parse`] on syntax error or unknown field. A
    /// mis-spelled key (e.g. `auto_aprove_levels` for `auto_approve_levels`)
    /// produces a parse error here, not a silent default.
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ServerConfig;
    ///
    /// let toml = r#"
    ///     [server]
    ///     id = "demo"
    ///     name = "Demo"
    ///     version = "0.1.0"
    /// "#;
    /// let cfg = ServerConfig::from_toml(toml).expect("parse");
    /// assert_eq!(cfg.server.name, "Demo");
    /// ```
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).map_err(ToolkitError::Parse)
    }

    /// Parse + validate. Per Phase 83 review R8 — guards against the
    /// missing-required-value trap that the `Default` impls on sub-sections
    /// would otherwise hide behind silent empty strings (e.g. a typo'd
    /// `[serever]` header makes `server.name` default to `""`).
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::Parse`] on TOML syntax / unknown-field errors,
    /// or [`ToolkitError::Validation`] (wrapping
    /// [`ConfigValidationError`]) on missing required values
    /// (empty `server.name`, empty `server.version`, empty tool name, empty
    /// table name).
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ServerConfig;
    /// let toml = r#"
    ///     [server]
    ///     name = "demo"
    ///     version = "0.1.0"
    /// "#;
    /// let cfg = ServerConfig::from_toml_strict_validated(toml).expect("valid");
    /// # let _ = cfg;
    /// ```
    pub fn from_toml_strict_validated(toml_str: &str) -> Result<Self> {
        let cfg = Self::from_toml(toml_str)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate required-field semantics that `#[serde(default)]` would
    /// otherwise mask. Per Phase 83 review R8.
    ///
    /// Rules checked, in order:
    /// 1. `server.name` is non-empty (trimmed).
    /// 2. `server.version` is non-empty (trimmed).
    /// 3. Every `[[tools]]` entry has a non-empty `name`.
    /// 4. No `[[tools]]` entry mixes tool kinds (`sql` / `path`+`method` /
    ///    `script`) — D-01 / T-90-02-04.
    /// 5. Every `[[database.tables]]` entry has a non-empty `name`.
    /// 6. Every `[[config_slots]]` entry has a non-empty `key` AND `name`
    ///    (PKG-03). The entry's `kind` needs no rule here — it is the closed
    ///    [`ConfigSlotKind`] enum, so serde rejects an unknown discriminator at
    ///    parse time, before `validate()` is called.
    /// 7. When a `[backend]` block is present (`http` feature), its `base_url`
    ///    is non-empty (trimmed) — GAP 3 / WR-02. Absent on no-http builds.
    /// 8. Every `[[tools.parameters]]` declaration is well-formed (Phase 128
    ///    D2 / SC-2): no empty `pattern`, no `minimum`/`maximum` outside the
    ///    exactly-representable `f64` integer range, and the tool's synthesized
    ///    `inputSchema` COMPILES as a Draft 2020-12 schema. The compile check
    ///    requires the `input-validation` feature; on a build without it the check
    ///    is skipped and a `tracing::warn!` says so once, because an enforcement
    ///    that is off must never read as on.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigValidationError`] variant identifying the
    /// first rule violated. Iteration order matches struct field order.
    pub fn validate(&self) -> std::result::Result<(), ConfigValidationError> {
        warn_if_pattern_checking_unavailable(&self.tools);
        if self.server.name.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerName);
        }
        if self.server.version.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerVersion);
        }
        for (i, tool) in self.tools.iter().enumerate() {
            if tool.name.trim().is_empty() {
                return Err(ConfigValidationError::EmptyToolName(i));
            }
            // D-01 / T-90-02-04: a tool is EITHER sql, single-call (path/method),
            // OR script — never a mixture. Reject ambiguity instead of letting a
            // silent "script wins" precedence hide a config mistake.
            if tool.declared_kind_count() > 1 {
                return Err(ConfigValidationError::AmbiguousToolKind(i));
            }
            // Phase 128 D2 / SC-2. Deliberately placed AFTER the name and
            // kind arms so no pre-existing test's expected variant changes.
            validate_tool_parameters(tool, &self.server.validation)?;
        }
        for (i, table) in self.database.tables.iter().enumerate() {
            if table.name.trim().is_empty() {
                return Err(ConfigValidationError::EmptyTableName(i));
            }
        }
        // PKG-03 (Phase 120 Plan 04): a declared slot must actually name a
        // config path AND a variable. An empty `key`/`name` claims coverage the
        // declaration cannot deliver. Deliberately NOT a completeness
        // heuristic — a "this literal looks secret, so a slot is missing" check
        // would flag the london-tube fixture's guarded dev `token_secret`, and
        // a check that cries wolf is worse than none.
        for (i, slot) in self.config_slots.iter().enumerate() {
            if slot.key.trim().is_empty() || slot.name.trim().is_empty() {
                return Err(ConfigValidationError::EmptyConfigSlotField(i));
            }
            // Identity-bearing slots structurally carry no value (the whole
            // "secrets never travel" premise) — a `tested_value` on a `secret`
            // declaration is the one field where a REAL credential could sit in
            // a config that is served but never packed, so the doc-comment rule
            // is enforced here rather than trusted.
            if slot.kind == ConfigSlotKind::Secret && slot.tested_value.is_some() {
                return Err(ConfigValidationError::SecretSlotCarriesTestedValue(i));
            }
        }
        // Phase 90 gap-closure (GAP 3 / WR-02): when a `[backend]` block is
        // declared, its `base_url` must be non-empty. Catch a typo'd / omitted
        // URL here (the field is `#[serde(default)]` -> `""`) rather than
        // letting it surface late as an opaque DispatchError at request time.
        // Gated on `http` because the `backend` field itself is http-only; the
        // block simply vanishes in a no-http build (SQL configs unaffected).
        #[cfg(feature = "http")]
        if let Some(backend) = &self.backend {
            if backend.base_url.trim().is_empty() {
                return Err(ConfigValidationError::EmptyBackendBaseUrl);
            }
            // Phase 120 follow-up: a reference-shaped base_url must name
            // exactly ONE variable. The grammar maps every malformed brace
            // form — the empty `${}` and multi-placeholder compositions like
            // `${SCHEME}://${HOST}` — to the empty name; catching that here
            // turns a boot-time `UnresolvedBaseUrlRef` with an empty variable
            // name into a load-time error naming the actual mistake.
            if crate::env_ref::parse_env_ref(&backend.base_url) == Some("") {
                return Err(ConfigValidationError::MalformedBackendBaseUrlRef);
            }
            // The same rule for `[backend.auth]` credentials. It is NOT the
            // same consequence: an unresolvable base_url breaks every request
            // loudly, while an unresolvable CREDENTIAL was silently omitted
            // (`expand_api_key_map` drops the entry; the scalar variants
            // collapse to `NoAuth`), so the server booted and sent every
            // backend request unauthenticated. Catching it here is what makes
            // that failure visible at all.
            if let Some(field) = backend.auth.malformed_env_ref_field() {
                return Err(ConfigValidationError::MalformedBackendAuthRef(field));
            }
        }
        Ok(())
    }

    /// Non-fatal configuration findings (Phase 128, D-07).
    ///
    /// Returns, in `[[tools]]` then `[[tools.parameters]]` DECLARATION order:
    ///
    /// 1. one `uncapped-string` finding per BODY-position string parameter with no
    ///    `max_length` — the residual D-05 accepts, surfaced rather than refused;
    /// 2. one `declared-max-length-above-placeholder-cap` finding per path- or
    ///    query-position parameter whose declared `max_length` EXCEEDS
    ///    `pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH`, because such a
    ///    parameter publishes a limit in `inputSchema` that the always-on
    ///    placeholder floor will not honour — so a refusal would name a rule the
    ///    client was never told about;
    ///
    /// and then one finding per ACTIVE `[server.validation]` opt-out.
    ///
    /// Never returns an error and never refuses anything. A config with zero tools
    /// and no active opt-out returns an empty `Vec`. Consumed by
    /// `cargo pmcp validate config` and by the once-at-startup log.
    #[must_use]
    pub fn lint(&self) -> Vec<ConfigWarning> {
        let mut out = Vec::new();
        for tool in &self.tools {
            lint_tool(tool, &self.server.validation, &mut out);
        }
        lint_opt_outs(&self.server.validation, &mut out);
        out
    }

    /// The findings that need the operator's OpenAPI document to be computable
    /// (Phase 128, D4(b) / T-128-36a).
    ///
    /// Separate from [`Self::lint`] rather than folded into it, because `lint`
    /// takes only `&self` and a config is meaningful with no spec at all — a
    /// spec-less deployment is supported and must produce no findings from its own
    /// absence.
    ///
    /// Returns, in `[[tools]]` declaration order, one
    /// [`CONFIGURED_TEMPLATE_NOT_IN_SPEC`] finding per single-call tool whose
    /// `(method, path)` matches no operation the spec declares. Such a tool reaches
    /// its endpoint and keeps the unconditional character floor and the always-on
    /// length cap, but the spec's declared `pattern`/`maxLength` narrowing for its
    /// placeholders is silently not applied — the `/users/{alias}` versus
    /// `/users/{id}` drift. That is an author error an operator can fix before
    /// deploy, which is the whole reason this runs at config time.
    ///
    /// An author-written query string on the configured `path` is stripped before
    /// the lookup, because an OpenAPI path template never carries one — so
    /// `/content/{version}/CUI?string=x` is matched as `/content/{version}/CUI`
    /// rather than reported as drift.
    ///
    /// # The bound on this guard, stated
    ///
    /// It covers a template written in the CONFIG. A template a Code Mode script
    /// COMPOSES at runtime is not visible here and cannot be, which is why the
    /// runtime miss is additionally reported once per `(method, template)` pair by
    /// `crate::code_mode`'s `log_spec_lookup_miss`. Neither signal is a refusal:
    /// this returns findings, and never an error.
    ///
    /// Tools with no `path`/`method` pair — SQL tools, script tools — are skipped:
    /// they address no single spec operation.
    #[cfg(feature = "http")]
    #[must_use]
    pub fn lint_against_spec(&self, spec: &crate::http::OpenApiSchema) -> Vec<ConfigWarning> {
        let mut out = Vec::new();
        for tool in &self.tools {
            let (Some(path), Some(method)) = (tool.path.as_deref(), tool.method.as_deref()) else {
                continue;
            };
            // An OpenAPI path template never carries a query string; the curated
            // surface permits one (plan 06's `?` narrowing), so strip it first.
            let template = path.split_once('?').map_or(path, |(p, _)| p);
            if spec.operation_for(template, method).is_some() {
                continue;
            }
            out.push(ConfigWarning {
                tool: tool.name.clone(),
                param: String::new(),
                rule: CONFIGURED_TEMPLATE_NOT_IN_SPEC,
                detail: format!(
                    "declares `method = \"{method}\"` and `path = \"{path}\"`, which matches no \
                     operation in the supplied OpenAPI document. The tool still works and its \
                     path placeholders still face the unconditional character floor and the \
                     always-on length cap, but the spec's declared pattern/maxLength narrowing \
                     is NOT applied to them — a placeholder named differently from the spec's \
                     own (`{{alias}}` against a declared `{{id}}`) reaches the same endpoint \
                     with its declaration silently dropped. Spell the path and method exactly \
                     as the spec declares them, or remove the spec if this endpoint is \
                     deliberately undocumented."
                ),
            });
        }
        out
    }

    /// A structured account of what THIS config actually enforces, for the
    /// once-at-startup log (Phase 128 D-07 / `<specifics>`).
    ///
    /// The startup log is the regression-tracing mechanism, not optional polish: it
    /// is how an operator discovers, from a deploy log alone, that a server is
    /// running with schema enforcement off or with the cap disabled.
    #[must_use]
    pub fn validation_report(&self) -> ValidationReport {
        let validation = &self.server.validation;
        let mut opt_outs = Vec::new();
        lint_opt_outs(validation, &mut opt_outs);
        ValidationReport {
            enforce_input_schema: validation.enforce_input_schema,
            default_max_length: validation.default_max_length,
            additional_properties: validation.additional_properties,
            strict: validation.strict,
            tools: self
                .tools
                .iter()
                .map(|t| tool_validation_report(t, validation))
                .collect(),
            opt_outs: opt_outs.iter().map(ToString::to_string).collect(),
        }
    }
}

/// What [`ServerConfig::validation_report`] returns: the enforcement actually in
/// effect, per server and per tool.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// Effective [`ValidationSection::enforce_input_schema`].
    pub enforce_input_schema: bool,
    /// Effective [`ValidationSection::default_max_length`].
    pub default_max_length: u64,
    /// Effective [`ValidationSection::additional_properties`].
    pub additional_properties: bool,
    /// Effective [`ValidationSection::strict`].
    pub strict: bool,
    /// One entry per `[[tools]]`, in declaration order.
    pub tools: Vec<ToolValidationReport>,
    /// Rendered active opt-outs — EMPTY when the server enforces everything it can.
    pub opt_outs: Vec<String>,
}

/// One tool's row in a [`ValidationReport`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolValidationReport {
    /// The `[[tools]]` `name`.
    pub tool: String,
    /// Rendered per-parameter rules in declaration order, e.g.
    /// `"region: path, pattern, maxLength=256 (default)"`. Reads as a log line.
    pub rules: Vec<String>,
}

// -----------------------------------------------------------------------------
// Phase 128 D3 / D-07 — `lint()` internals
// -----------------------------------------------------------------------------

/// Whether a parameter's EFFECTIVE JSON Schema type is `string`.
///
/// `param_type` defaults to `"string"` when omitted, matching
/// `tools.rs::build_param_property`, so an omitted `type` is a string here too. A
/// divergence would make the cap apply to a different set of parameters than the
/// one the schema builder emits it for.
pub(crate) fn is_string_param(p: &ParamDecl) -> bool {
    p.param_type.as_deref().unwrap_or("string") == "string"
}

/// Whether the D3 default cap applies to a parameter at `position`.
///
/// All four conditions, in one place so `lint()` and
/// `tools.rs::apply_position_cap` cannot disagree about which parameters are
/// covered: the effective type is `string`, no `max_length` is declared, the
/// configured default is non-zero, and the position is `Path` or `Query`.
pub(crate) fn default_cap_applies(
    p: &ParamDecl,
    position: ParamPosition,
    validation: &ValidationSection,
) -> bool {
    is_string_param(p)
        && p.max_length.is_none()
        && validation.default_max_length != 0
        && matches!(position, ParamPosition::Path | ParamPosition::Query)
}

/// Per-tool `lint()` findings, appended in `[[tools.parameters]]` declaration
/// order.
fn lint_tool(tool: &ToolDecl, validation: &ValidationSection, out: &mut Vec<ConfigWarning>) {
    for p in &tool.parameters {
        let position = tool.param_position(&p.name);
        if is_string_param(p)
            && p.max_length.is_none()
            && !default_cap_applies(p, position, validation)
        {
            out.push(ConfigWarning {
                tool: tool.name.clone(),
                param: p.name.clone(),
                rule: UNCAPPED_STRING,
                detail: format!(
                    "declares no max_length and is in {position:?} position, where the \
                     [server.validation] default_max_length cap deliberately does not apply \
                     (free text must keep working) — declare an explicit max_length, or set \
                     [server.validation] strict = true to make this an error"
                ),
            });
        }
        if let Some(declared) = p.max_length {
            lint_declared_cap_above_placeholder_floor(tool, p, position, declared, out);
        }
    }
}

/// SC-7 shape mismatch: a path- or query-position parameter declaring a
/// `max_length` ABOVE the always-on placeholder floor publishes a limit in
/// `inputSchema` that `validate_path_placeholder` will not honour, so the call is
/// refused against a rule the client was never told about.
///
/// The floor is read from
/// `pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH` — the ONE copy of
/// that number (D-08) — and is therefore only available under `input-validation`.
/// See the `cfg(not(...))` sibling for why its absence makes this finding vacuous
/// rather than merely unavailable.
#[cfg(feature = "input-validation")]
fn lint_declared_cap_above_placeholder_floor(
    tool: &ToolDecl,
    p: &ParamDecl,
    position: ParamPosition,
    declared: u64,
    out: &mut Vec<ConfigWarning>,
) {
    if !matches!(position, ParamPosition::Path | ParamPosition::Query) {
        return;
    }
    let floor = pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH as u64;
    if declared <= floor {
        return;
    }
    out.push(ConfigWarning {
        tool: tool.name.clone(),
        param: p.name.clone(),
        rule: DECLARED_MAX_LENGTH_ABOVE_PLACEHOLDER_CAP,
        detail: format!(
            "declares max_length = {declared} in {position:?} position, but the always-on \
             path-placeholder floor refuses at {floor} code points regardless — so the \
             effective limit is {floor}, and a refusal would name a limit the published \
             inputSchema never advertised. Lower the declared max_length to {floor} or below."
        ),
    });
}

/// The `input-validation`-off half of the placeholder-floor lint.
//
// Why a no-op rather than a duplicated `256`: the floor this finding warns about
// is `pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH`, which is a module
// constant precisely so exactly one copy of the number exists (D-08). On a build
// without `input-validation` that floor is not compiled and not enforced, so there
// is no shape MISMATCH to report — the declared `max_length` is the only limit
// there is, and it is honoured. Hard-coding a second `256` here to keep the
// finding alive would report a rule that this build does not apply.
//
// The genuinely-missing enforcement on such a build is reported once per
// `validate()` by `warn_if_pattern_checking_unavailable`, not here.
#[cfg(not(feature = "input-validation"))]
fn lint_declared_cap_above_placeholder_floor(
    _tool: &ToolDecl,
    _p: &ParamDecl,
    _position: ParamPosition,
    _declared: u64,
    _out: &mut Vec<ConfigWarning>,
) {
}

/// One finding per ACTIVE `[server.validation]` opt-out, so a switched-off
/// enforcement can never read as switched on.
fn lint_opt_outs(validation: &ValidationSection, out: &mut Vec<ConfigWarning>) {
    if !validation.enforce_input_schema {
        out.push(server_warning(
            OPT_OUT_ENFORCE_INPUT_SCHEMA,
            "[server.validation] enforce_input_schema = false: declared inputSchema values \
             are NOT checked at tools/call time. Explicitly-registered argument validators \
             still run — this flag does not disable them."
                .to_string(),
        ));
    }
    if validation.default_max_length == 0 {
        out.push(server_warning(
            OPT_OUT_DEFAULT_MAX_LENGTH_ZERO,
            "[server.validation] default_max_length = 0: no default maxLength is emitted in \
             ANY position, so a path or query string parameter that declares no max_length \
             is unbounded in the published schema. The always-on path-placeholder floor is \
             unaffected."
                .to_string(),
        ));
    }
    if validation.additional_properties {
        out.push(server_warning(
            OPT_OUT_ADDITIONAL_PROPERTIES,
            "[server.validation] additional_properties = true: UNDECLARED arguments are \
             accepted, re-opening the unknown-argument class for every tool on this server."
                .to_string(),
        ));
    }
}

/// A server-level [`ConfigWarning`] — empty `tool` / `param`, per that struct's
/// documented convention.
fn server_warning(rule: &'static str, detail: String) -> ConfigWarning {
    ConfigWarning {
        tool: String::new(),
        param: String::new(),
        rule,
        detail,
    }
}

/// One tool's [`ToolValidationReport`] row.
fn tool_validation_report(tool: &ToolDecl, validation: &ValidationSection) -> ToolValidationReport {
    ToolValidationReport {
        tool: tool.name.clone(),
        rules: tool
            .parameters
            .iter()
            .map(|p| render_param_rules(tool, p, validation))
            .collect(),
    }
}

/// Render ONE parameter's effective rules as a log-readable line.
fn render_param_rules(tool: &ToolDecl, p: &ParamDecl, validation: &ValidationSection) -> String {
    let position = tool.param_position(&p.name);
    let mut parts = vec![format!("{position:?}")];
    if p.required {
        parts.push("required".to_string());
    }
    if p.pattern.is_some() {
        parts.push("pattern".to_string());
    }
    if let Some(format) = &p.format {
        parts.push(format!("format={format}"));
    }
    if let Some(min) = p.min_length {
        parts.push(format!("minLength={min}"));
    }
    if let Some(max) = p.max_length {
        parts.push(format!("maxLength={max} (declared)"));
    } else if default_cap_applies(p, position, validation) {
        parts.push(format!(
            "maxLength={} (default)",
            validation.default_max_length
        ));
    }
    format!("{}: {}", p.name, parts.join(", "))
}

/// Machine-readable [`ConfigWarning::rule`] identifier: an uncapped body string.
pub const UNCAPPED_STRING: &str = "uncapped-string";
/// Machine-readable [`ConfigWarning::rule`] identifier: a declared `max_length`
/// above the always-on path-placeholder floor.
pub const DECLARED_MAX_LENGTH_ABOVE_PLACEHOLDER_CAP: &str =
    "declared-max-length-above-placeholder-cap";
/// Machine-readable [`ConfigWarning::rule`] identifier: a configured single-call
/// `(method, path)` that matches no operation in the supplied OpenAPI document, so
/// the spec's declared placeholder narrowing is not applied to that tool
/// (Phase 128, D4(b) / T-128-36a). Emitted by
/// [`ServerConfig::lint_against_spec`](crate::config::ServerConfig::lint_against_spec).
pub const CONFIGURED_TEMPLATE_NOT_IN_SPEC: &str = "configured-template-not-in-spec";
/// Machine-readable [`ConfigWarning::rule`] identifier: schema enforcement off.
pub const OPT_OUT_ENFORCE_INPUT_SCHEMA: &str = "opt-out-enforce-input-schema";
/// Machine-readable [`ConfigWarning::rule`] identifier: default cap disabled.
pub const OPT_OUT_DEFAULT_MAX_LENGTH_ZERO: &str = "opt-out-default-max-length-zero";
/// Machine-readable [`ConfigWarning::rule`] identifier: unknown arguments accepted.
pub const OPT_OUT_ADDITIONAL_PROPERTIES: &str = "opt-out-additional-properties";

// -----------------------------------------------------------------------------
// Phase 128 D2 / SC-2 — per-parameter declaration checks
// -----------------------------------------------------------------------------

/// The largest integer magnitude an `f64` represents exactly (2^53).
///
/// [`ParamDecl::minimum`] / [`ParamDecl::maximum`] are `f64`, so a declared bound
/// above this cannot round-trip. See [`ConfigValidationError::NonFiniteParamBound`]
/// for exactly what a magnitude check on the already-parsed value can and cannot
/// establish.
const MAX_EXACT_INTEGER_BOUND: f64 = 9_007_199_254_740_992.0;

/// Run the Phase 128 D2 / SC-2 declaration checks for ONE `[[tools]]` entry.
///
/// Split out of [`ServerConfig::validate`] so that function stays well under the
/// cog-25 gate as rules accumulate.
///
/// # Errors
///
/// [`ConfigValidationError::EmptyParamPattern`],
/// [`ConfigValidationError::NonFiniteParamBound`], or
/// [`ConfigValidationError::UncompilableParamSchema`] — first rule violated, in
/// `[[tools.parameters]]` declaration order.
fn validate_tool_parameters(
    tool: &ToolDecl,
    validation: &ValidationSection,
) -> std::result::Result<(), ConfigValidationError> {
    for p in &tool.parameters {
        check_param_patterns_non_empty(tool, p)?;
        check_param_bounds_representable(tool, p)?;
        if validation.strict {
            check_param_capped_under_strict(tool, p, validation)?;
        }
    }
    check_path_template_segments(tool)?;
    check_tool_input_schema_compiles(tool, validation)
}

/// Refuse a single-call `path` template segment the curated substitution parser
/// cannot recognize (Phase 128, D4(b)).
///
/// # Why this is a hard error rather than a `lint()` finding
///
/// A malformed segment has no working interpretation. `/search/{a}{b}` yields the
/// parameter name `a}{b`, which no `[[tools.parameters]]` entry can match, and
/// `/prefix-{id}` is not recognized as carrying a placeholder at all — so in both
/// cases literal braces are what would travel toward the backend. That request is
/// already refused at call time by the composed-path check in
/// `crate::http::HttpClient`, so the choice here is only between failing loudly at
/// startup and failing obscurely on every call. Nothing that worked before loses a
/// working behaviour; a silently-broken tool gains a message naming the segment.
///
/// Contrast [`ConfigValidationError::UncappedStringParam`], which is `strict`-only
/// precisely because an uncapped free-text field DOES have a working
/// interpretation (D-05).
///
/// # Errors
///
/// [`ConfigValidationError::MalformedPathTemplateSegment`], naming the first
/// offending segment in left-to-right order.
fn check_path_template_segments(tool: &ToolDecl) -> std::result::Result<(), ConfigValidationError> {
    let Some(path) = tool.path.as_deref() else {
        // No `path` — a SQL or script tool carries no template.
        return Ok(());
    };
    for segment in path.split('/') {
        if is_supported_path_segment(segment) {
            continue;
        }
        return Err(ConfigValidationError::MalformedPathTemplateSegment {
            tool: tool.name.clone(),
            segment: segment.to_string(),
        });
    }
    Ok(())
}

/// Whether ONE `/`-delimited path-template segment is a shape
/// [`path_placeholder_names`] recognizes.
///
/// Exactly two shapes are supported: a segment containing no brace at all, and a
/// segment that is exactly one non-empty `{name}` spanning the whole segment with
/// no further brace inside the name. This predicate is the inverse of
/// [`path_placeholder_names`]'s filter, extended to also catch the
/// carries-a-brace-but-is-not-a-placeholder cases that filter silently drops.
fn is_supported_path_segment(segment: &str) -> bool {
    if !segment.contains('{') && !segment.contains('}') {
        return true;
    }
    // `{` and `}` are single-byte, so the inner slice is always a char boundary.
    segment.starts_with('{')
        && segment.ends_with('}')
        && segment.len() > 2
        && !segment[1..segment.len() - 1].contains('{')
        && !segment[1..segment.len() - 1].contains('}')
}

/// D-07 strict mode: promote an `uncapped-string` lint finding into a hard
/// `validate()` failure.
///
/// Gated on `[server.validation] strict` by the caller, because a running server
/// must NOT refuse to boot over an uncapped free-text body parameter (D-05). This
/// path exists for a CI config check, where refusing is exactly right.
///
/// # Errors
///
/// [`ConfigValidationError::UncappedStringParam`].
fn check_param_capped_under_strict(
    tool: &ToolDecl,
    p: &ParamDecl,
    validation: &ValidationSection,
) -> std::result::Result<(), ConfigValidationError> {
    let position = tool.param_position(&p.name);
    if is_string_param(p) && p.max_length.is_none() && !default_cap_applies(p, position, validation)
    {
        return Err(ConfigValidationError::UncappedStringParam {
            tool: tool.name.clone(),
            param: p.name.clone(),
        });
    }
    Ok(())
}

/// SC-2 edge (empty): refuse `pattern = ""` on the parameter OR on its
/// `[tools.parameters.items]` sub-table.
///
/// # Errors
///
/// [`ConfigValidationError::EmptyParamPattern`].
fn check_param_patterns_non_empty(
    tool: &ToolDecl,
    p: &ParamDecl,
) -> std::result::Result<(), ConfigValidationError> {
    let declared = [
        p.pattern.as_deref(),
        p.items.as_ref().and_then(|i| i.pattern.as_deref()),
    ];
    if declared.into_iter().flatten().any(str::is_empty) {
        return Err(ConfigValidationError::EmptyParamPattern {
            tool: tool.name.clone(),
            param: p.name.clone(),
        });
    }
    Ok(())
}

/// D3 edge (precision): refuse a `minimum`/`maximum` that an `f64` cannot carry
/// exactly. See [`ConfigValidationError::NonFiniteParamBound`] for what this does
/// and does not establish.
///
/// # Errors
///
/// [`ConfigValidationError::NonFiniteParamBound`].
fn check_param_bounds_representable(
    tool: &ToolDecl,
    p: &ParamDecl,
) -> std::result::Result<(), ConfigValidationError> {
    let unrepresentable = [p.minimum, p.maximum]
        .into_iter()
        .flatten()
        .any(|b| !b.is_finite() || b.abs() > MAX_EXACT_INTEGER_BOUND);
    if unrepresentable {
        return Err(ConfigValidationError::NonFiniteParamBound {
            tool: tool.name.clone(),
            param: p.name.clone(),
        });
    }
    Ok(())
}

/// SC-2: compile the tool's synthesized `inputSchema` at CONFIG time, so a
/// non-compiling `pattern` fails here — naming the parameter — rather than at call
/// time, where it would take the tool's entire validator down.
///
/// Reuses `crate::tools::build_input_schema`, the same constructor the runtime
/// serves from, so the gate cannot pass a schema the server never actually uses.
/// `jsonschema::meta::is_valid` is deliberately NOT the check: it returns `true`
/// for a schema whose nested `pattern` does not compile (measured — RESEARCH
/// Finding 1e), which is precisely the mistake this gate exists to catch.
///
/// # Errors
///
/// [`ConfigValidationError::UncompilableParamSchema`] carrying the compile error's
/// own schema path and detail. Quoting the detail is safe here and only here: this
/// runs at load time with no request in scope and its audience is the config author.
#[cfg(feature = "input-validation")]
fn check_tool_input_schema_compiles(
    tool: &ToolDecl,
    validation: &ValidationSection,
) -> std::result::Result<(), ConfigValidationError> {
    let schema = crate::tools::build_input_schema(tool, validation);
    pmcp::server::schema_validation::check_input_schema_compiles(&schema).map_err(|violation| {
        ConfigValidationError::UncompilableParamSchema {
            tool: tool.name.clone(),
            position: violation.pointer,
            detail: violation.expected,
        }
    })
}

/// The `input-validation`-off half of the SC-2 gate.
//
// Why this arm exists at all, rather than an ungated call: `ServerConfig::validate`
// compiles in EVERY toolkit feature set, while
// `pmcp::server::schema_validation::check_input_schema_compiles` exists only under
// `pmcp/schema-validation` (forwarded by the toolkit's `input-validation`). An
// ungated call breaks `cargo build -p pmcp-server-toolkit --no-default-features
// --features http`, which is a real supported configuration.
//
// Why it is not a SILENT skip: on this build a declared `pattern` is neither
// verified here nor enforced at call time, so an author who sees `validate()`
// return `Ok(())` would reasonably believe their rule was checked. That is exactly
// the "an enforcement that is off must never read as on" prohibition. The warning
// is emitted once per `validate()` by `warn_if_pattern_checking_unavailable`, not
// per tool, so a large config does not bury it.
#[cfg(not(feature = "input-validation"))]
fn check_tool_input_schema_compiles(
    _tool: &ToolDecl,
    _validation: &ValidationSection,
) -> std::result::Result<(), ConfigValidationError> {
    Ok(())
}

/// Emit the once-per-`validate()` warning when this build cannot check declared
/// `pattern` values. A no-op when the `input-validation` feature is on, and a
/// no-op when the config declares no `pattern` at all (there is nothing unchecked
/// to report).
#[cfg(not(feature = "input-validation"))]
fn warn_if_pattern_checking_unavailable(tools: &[ToolDecl]) {
    let declares_a_pattern = tools.iter().any(|t| {
        t.parameters
            .iter()
            .any(|p| p.pattern.is_some() || p.items.as_ref().is_some_and(|i| i.pattern.is_some()))
    });
    if declares_a_pattern {
        tracing::warn!(
            "this build lacks the `input-validation` feature: declared \
             [[tools.parameters]] `pattern` values were NOT checked for compilability at \
             config time, and will NOT be enforced at tools/call time either — enable \
             `input-validation` to get either"
        );
    }
}

/// See the `cfg(not(...))` sibling. Under `input-validation` the patterns ARE
/// checked, so there is nothing to warn about.
#[cfg(feature = "input-validation")]
#[allow(clippy::missing_const_for_fn)] // Why: mirrors the cfg(not(...)) sibling's signature, which cannot be const.
fn warn_if_pattern_checking_unavailable(_tools: &[ToolDecl]) {}

// -----------------------------------------------------------------------------
// [server]
// -----------------------------------------------------------------------------

/// `[server]` section — identity and version metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerSection {
    /// Stable server identifier (e.g. `"open-images"`). Optional in the TOML;
    /// callers that need it should fall back to deriving from `name`.
    #[serde(default)]
    pub id: Option<String>,
    /// Human-readable server name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Short server description.
    #[serde(default)]
    pub description: Option<String>,
    /// Server flavour (e.g. `"sql-api"`). Free-form for now; future plans may tighten.
    #[serde(default, rename = "type")]
    pub server_type: Option<String>,
    /// Semver version string (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub version: String,
    /// Whether this server is the **reference** server that provisions shared
    /// infrastructure (the `[shared_policy_store]` for all sibling SQL servers).
    /// Additive per the REF-01 superset invariant (Plan 85-01); the SQLite
    /// Chinook reference config sets `is_reference = true`.
    #[serde(default)]
    pub is_reference: bool,
    /// `[server.validation]` — input-enforcement policy (Phase 128, D3 / D-06).
    ///
    /// Absent in TOML yields [`ValidationSection::default`], i.e. enforcement ON
    /// with a 256-code-point default cap in path and query position.
    #[serde(default)]
    pub validation: ValidationSection,
}

/// `[server.validation]` — how strictly a config-declared tool's inputs are
/// enforced (Phase 128, D3 / D-06 / D-07).
///
/// Every field here is an OPT-OUT knob, and every active opt-out is reported by
/// [`ServerConfig::lint`] and by [`ServerConfig::validation_report`] so it appears
/// in the startup log. That is deliberate: a validation rule switched off by a
/// configuration value must never read as switched on.
///
/// # Forward incompatibility (D-15)
///
/// [`ServerSection`] and [`ServerConfig`] both carry
/// `#[serde(deny_unknown_fields)]`, so a config carrying a `[server.validation]`
/// section fails to PARSE on toolkit 0.1.3 rather than having the section ignored.
/// Named in the CHANGELOG, exactly as the [`ParamDecl`] D2 keys are.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::config::ValidationSection;
///
/// let v = ValidationSection::default();
/// assert!(v.enforce_input_schema);
/// assert_eq!(v.default_max_length, 256);
/// assert!(!v.additional_properties);
/// assert!(!v.strict);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationSection {
    /// Whether a declared `inputSchema` is CHECKED at `tools/call` time.
    ///
    /// Default `true`. Setting it `false` skips the schema check only — it does
    /// NOT disable an explicitly-registered argument validator. Turning off one
    /// enforcement must never silently turn off another, so the two live on
    /// separate switches and the decorator is still constructed whenever a
    /// validator is registered for the tool.
    #[serde(default = "default_enforce_input_schema")]
    pub enforce_input_schema: bool,
    /// Default `maxLength`, in Unicode code points, emitted for a PATH- or
    /// QUERY-position string parameter that declares no `max_length` of its own
    /// (D-06).
    ///
    /// Default `256`. A declared `max_length` is never overridden and never merged
    /// with this value. Body-position strings are deliberately NOT capped by it
    /// (D-05) — they are surfaced by [`ServerConfig::lint`] instead.
    ///
    /// `0` DISABLES the cap in every position and is reported as an active opt-out.
    /// It does not disable `pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH`,
    /// which is a module constant precisely so the length half of the path-traversal
    /// fix cannot be configured away (D-08).
    #[serde(default = "default_default_max_length")]
    pub default_max_length: u64,
    /// Whether to permit UNDECLARED arguments, by emitting
    /// `additionalProperties: true` instead of `false`.
    ///
    /// Default `false` (unknown arguments are refused). `true` re-opens the
    /// unknown-argument class for this server and is reported as an active opt-out.
    #[serde(default)]
    pub additional_properties: bool,
    /// Whether [`ServerConfig::lint`] findings are promoted into hard
    /// [`ServerConfig::validate`] failures.
    ///
    /// Default `false`: a running server never refuses to boot over an uncapped
    /// free-text body parameter (D-07). `true` turns each such parameter into
    /// [`ConfigValidationError::UncappedStringParam`], which is what a CI config
    /// check wants and what a production boot does not.
    #[serde(default)]
    pub strict: bool,
}

/// The shipped default cap, in Unicode code points (D-06).
const DEFAULT_MAX_LENGTH: u64 = 256;

/// serde default for [`ValidationSection::enforce_input_schema`]. Enforcement is
/// ON unless an operator explicitly opts out.
const fn default_enforce_input_schema() -> bool {
    true
}

/// serde default for [`ValidationSection::default_max_length`].
const fn default_default_max_length() -> u64 {
    DEFAULT_MAX_LENGTH
}

impl Default for ValidationSection {
    fn default() -> Self {
        Self {
            enforce_input_schema: default_enforce_input_schema(),
            default_max_length: default_default_max_length(),
            additional_properties: false,
            strict: false,
        }
    }
}

// -----------------------------------------------------------------------------
// [metadata]
// -----------------------------------------------------------------------------

/// `[metadata]` section — admin-facing display defaults (visible in the
/// pmcp.run UI before an operator customises them).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct MetadataSection {
    /// Long-form display name shown in the UI.
    #[serde(default)]
    pub display_name: Option<String>,
    /// One-line summary for list views.
    #[serde(default)]
    pub short_description: Option<String>,
    /// Multi-line description for detail pages.
    #[serde(default)]
    pub description: Option<String>,
    /// Tag list for filtering / discovery.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Server author (organisation or individual).
    #[serde(default)]
    pub author: Option<String>,
    /// Visibility flag (e.g. `"public"`, `"private"`).
    #[serde(default)]
    pub visibility: Option<String>,
}

// -----------------------------------------------------------------------------
// [database]
// -----------------------------------------------------------------------------

/// `[database]` section — backend identification and table catalogue.
///
/// Includes Athena-specific keys (`output_location`, `workgroup`) as optional
/// fields per the REF-01 superset invariant — non-Athena backends omit them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSection {
    /// Backend type (`"athena"`, `"postgres"`, `"mysql"`, `"sqlite"`, …).
    #[serde(default, rename = "type")]
    pub backend_type: Option<String>,
    /// Database / schema name.
    #[serde(default)]
    pub database: Option<String>,
    /// Athena S3 output location for query results.
    #[serde(default)]
    pub output_location: Option<String>,
    /// Athena workgroup name.
    #[serde(default)]
    pub workgroup: Option<String>,
    /// Per-query timeout in milliseconds.
    #[serde(default)]
    pub query_timeout_ms: Option<u64>,
    /// `[[database.tables]]` — declared table catalogue for schema enrichment.
    #[serde(default)]
    pub tables: Vec<DatabaseTableDecl>,
    /// Connection URL for Postgres / MySQL backends. Supports `env:VAR_NAME`
    /// indirection at the consumer-resolution layer (the toolkit parses the
    /// string as-is and leaves resolution to the per-backend connector or
    /// the secret-resolution machinery from P83 R6/R9). Optional/unused for
    /// Athena (uses `region` + `workgroup` + `output_location`) and SQLite
    /// (uses `database` for the file path or `:memory:` literal).
    #[serde(default)]
    pub url: Option<String>,
    /// Filesystem path to a SQLite database file (e.g.
    /// `"/var/task/assets/chinook.db"` for a Lambda-bundled asset). Additive per
    /// the REF-01 superset invariant (Plan 85-01). Distinct from `database`
    /// (which carries the `:memory:` literal or a schema name) and `url` (used
    /// by Postgres / MySQL). Stored verbatim; the SQLite connector resolves it.
    #[serde(default)]
    pub file_path: Option<String>,
    /// `[database.pool]` — connection-pool tuning (optional).
    #[serde(default)]
    pub pool: Option<DatabasePoolSection>,
}

/// Single `[[database.tables]]` entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseTableDecl {
    /// Table or view name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Human-readable table description for schema enrichment.
    #[serde(default)]
    pub description: Option<String>,
}

/// `[database.pool]` connection-pool tuning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabasePoolSection {
    /// Maximum concurrent connections.
    #[serde(default)]
    pub max_connections: Option<u32>,
    /// Connection-acquisition timeout, in seconds.
    #[serde(default)]
    pub connection_timeout_seconds: Option<u64>,
}

// -----------------------------------------------------------------------------
// [backend] (http feature)
// -----------------------------------------------------------------------------

/// Re-export of the outgoing-HTTP authentication config (owned by
/// [`crate::http::auth`], Plan 90-01). Callers may also reach it via the
/// `crate::http` module path; this re-export keeps `[backend.auth]` named
/// alongside the `ServerConfig` types it deserializes into.
#[cfg(feature = "http")]
pub use crate::http::auth::AuthConfig;

/// Re-export of the HTTP client tuning config (owned by [`crate::http::client`],
/// Plan 90-01) used by `[backend.http]`.
#[cfg(feature = "http")]
pub use crate::http::client::HttpConfig;

/// `[backend]` section — the OpenAPI/REST HTTP backend declaration (D-06).
///
/// This is the HTTP analog of [`DatabaseSection`]: it identifies the upstream
/// REST API the synthesized tools call. `base_url` is the API root; the optional
/// `[backend.auth]` sub-table selects an [`AuthConfig`] variant (`type = "..."`)
/// and `[backend.http]` carries [`HttpConfig`] tuning (timeout / retries / …).
///
/// Gated behind the `http` feature — the whole section (and the
/// [`ServerConfig::backend`] field) is absent in a no-http build so there is no
/// dead stub type. `AuthConfig` and `HttpConfig` are DEFINED in
/// [`crate::http`] (Plan 90-01) and re-exported here, not redefined (H3).
///
/// Strict-parse discipline (D-13) is preserved: `#[serde(deny_unknown_fields)]`
/// rejects a typo'd key under `[backend]` or `[backend.http]`.
///
/// Secrets posture (T-90-02-02): inline token fields under `[backend.auth]`
/// hold operator references (`${ENV}` / `env:VAR`) resolved upstream by the
/// Phase 83 secrets machinery — config parsing stores the string verbatim and
/// never the resolved value.
#[cfg(feature = "http")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct BackendSection {
    /// REST API root URL (e.g. `"https://api.tfl.gov.uk"`). Single-call tools
    /// concatenate their `path` onto this (an empty per-tool `base_url`
    /// inherits this value).
    #[serde(default)]
    pub base_url: String,
    /// `[backend.auth]` — outgoing authentication ([`AuthConfig`], six modes).
    /// Defaults to [`AuthConfig::None`] when the sub-table is omitted.
    #[serde(default)]
    pub auth: AuthConfig,
    /// `[backend.http]` — client tuning ([`HttpConfig`]: timeout / retries /
    /// backoff / user-agent / default headers). Defaults to [`HttpConfig`]'s
    /// defaults when the sub-table is omitted.
    #[serde(default)]
    pub http: HttpConfig,
}

#[cfg(feature = "http")]
impl BackendSection {
    /// Resolve [`Self::base_url`], expanding a `${VAR}` / `env:VAR` reference
    /// from the process environment. Callers MUST use this rather than reading
    /// `base_url` directly — the raw field may hold an unresolved placeholder.
    ///
    /// A Shape A server's endpoint is frequently a slot the target environment
    /// fills, so the config records `base_url = "${TFL_BASE_URL}"` and the
    /// package digest stays environment-independent. Without expansion that
    /// literal `${...}` parses, VALIDATES (it is non-empty, so the emptiness
    /// rule passes) and is then sent as the request URL.
    ///
    /// Resolution rules — the grammar is [`crate::env_ref::parse_env_ref`], the
    /// single toolkit-wide chokepoint:
    /// - a plain literal (no `${...}` / `env:` prefix) is returned VERBATIM;
    /// - `${VAR}` / `env:VAR` reads `VAR` from the process environment;
    /// - a MALFORMED reference — the empty `${}`, or a multi-placeholder
    ///   composition like `${A}://${B}` (a brace reference names exactly ONE
    ///   variable) — is an error;
    /// - an UNSET variable, or one set to an empty / whitespace-only value, is
    ///   an error.
    ///
    /// # Deliberate divergence from credential resolution
    ///
    /// A credential resolves an unset reference to the empty string so an
    /// optional credential is OMITTED (see `crate::http::auth`). An endpoint
    /// does NOT get that treatment: an empty credential yields a degraded
    /// request, but an empty endpoint yields a broken one, and
    /// [`ServerConfig::validate`] only checks emptiness at parse time — an
    /// empty resolution would sail through and then break every request. This
    /// uses the error-on-unset semantics of `code_mode`'s `token_secret`
    /// resolution instead.
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::UnresolvedBaseUrlRef`] when the reference cannot
    /// be resolved. Per T-120-17 the error names the FIELD and the
    /// environment-variable NAME only — never a resolved URL or credential.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ServerConfig;
    ///
    /// let cfg = ServerConfig::from_toml_strict_validated(
    ///     "[server]\nname = \"demo\"\nversion = \"0.1.0\"\n\
    ///      [backend]\nbase_url = \"https://api.example.com\"\n",
    /// )
    /// .expect("valid config");
    /// let backend = cfg.backend.as_ref().expect("[backend] present");
    /// // A plain literal is used verbatim.
    /// assert_eq!(backend.resolved_base_url().unwrap(), "https://api.example.com");
    /// ```
    pub fn resolved_base_url(&self) -> std::result::Result<String, ToolkitError> {
        match crate::env_ref::parse_env_ref(&self.base_url) {
            // Plain literal — used verbatim (every existing [backend] config
            // and the four SQL reference configs land here, unchanged).
            None => Ok(self.base_url.clone()),
            // Malformed `${}` — a reference to an empty name. A credential
            // treats this as "omit"; an endpoint cannot be omitted.
            Some("") => Err(ToolkitError::UnresolvedBaseUrlRef { var: String::new() }),
            Some(name) => match std::env::var(name) {
                Ok(value) if !value.trim().is_empty() => Ok(value),
                // Unset, or set-but-empty/whitespace — the same error either
                // way. The VALUE is never carried into the error.
                _ => Err(ToolkitError::UnresolvedBaseUrlRef {
                    var: name.to_string(),
                }),
            },
        }
    }
}

// -----------------------------------------------------------------------------
// [code_mode]
// -----------------------------------------------------------------------------

/// `[code_mode]` section — code-mode policy + complexity limits.
///
/// The toolkit uses **unprefixed** field names (REF-01 invariant); the mapping
/// to `pmcp_code_mode::CodeModeConfig`'s prefixed names (`sql_allow_writes`,
/// etc.) is handled by Plan 06's executor wiring.
#[allow(clippy::struct_excessive_bools)]
// Why: REF-01 superset — these bools mirror the reference servers' [code_mode] block 1:1 (CONTEXT.md D-13). Grouping into a sub-struct would break REF-01.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeModeSection {
    /// Master enable flag for code-mode.
    #[serde(default)]
    pub enabled: bool,
    /// Server identifier used by AVP / Cedar policy resolution.
    #[serde(default)]
    pub server_id: Option<String>,
    /// Whether INSERT / UPDATE / MERGE statements are allowed.
    #[serde(default)]
    pub allow_writes: bool,
    /// Whether DELETE statements are allowed.
    #[serde(default)]
    pub allow_deletes: bool,
    /// Whether DDL (CREATE / ALTER / DROP) is allowed.
    #[serde(default)]
    pub allow_ddl: bool,
    /// Whether `SELECT` queries must declare a `LIMIT`.
    #[serde(default)]
    pub require_limit: bool,
    /// Maximum allowed `LIMIT` value.
    #[serde(default)]
    pub max_limit: Option<u64>,
    /// Table names blocked from any query (denylist).
    #[serde(default)]
    pub blocked_tables: Vec<String>,
    /// `table.column` strings stripped from query output.
    #[serde(default)]
    pub sensitive_columns: Vec<String>,
    /// Risk levels eligible for auto-approval (e.g. `["low"]`).
    #[serde(default)]
    pub auto_approve_levels: Vec<String>,
    /// Token TTL, in seconds, for HMAC-signed approval tokens.
    #[serde(default)]
    pub token_ttl_seconds: Option<u64>,
    /// Secret reference (e.g. `"${CODE_MODE_SECRET}"`) for HMAC signing — resolved
    /// at runtime by `SecretsProvider`. NEVER a raw secret value (review R6 +
    /// T-83-04-04 in the plan threat model).
    #[serde(default)]
    pub token_secret: Option<String>,
    /// Per Phase 83 review R9: inline `token_secret = "raw-string"` is REJECTED
    /// by default to prevent secrets from being committed to source-controlled
    /// configs. Set this flag to `true` ONLY in dev/test configs where the
    /// operator explicitly accepts the risk. NEVER set this in a committed
    /// production config — production must use the `env:VAR_NAME` syntax that
    /// resolves at runtime through `SecretsProvider`.
    #[serde(default)]
    pub allow_inline_token_secret_for_dev: bool,
    /// `[code_mode.limits]` — query-complexity caps.
    #[serde(default)]
    pub limits: Option<CodeModeLimits>,
}

/// `[code_mode.limits]` — query-complexity caps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeModeLimits {
    /// Maximum number of distinct tables referenced in a single query.
    #[serde(default)]
    pub max_tables_per_query: Option<u32>,
    /// Maximum JOIN nesting depth.
    #[serde(default)]
    pub max_join_depth: Option<u32>,
    /// Maximum subquery nesting depth.
    #[serde(default)]
    pub max_subquery_depth: Option<u32>,
}

// -----------------------------------------------------------------------------
// [shared_policy_store]
// -----------------------------------------------------------------------------

/// `[shared_policy_store]` section — AVP/Cedar shared-policy-store declaration.
///
/// Emitted only by the **reference** SQL server (`[server] is_reference = true`),
/// which provisions a single shared policy store + a set of Cedar templates that
/// all sibling SQL servers attach to (rather than each minting its own store).
///
/// Additive per the REF-01 superset invariant (Plan 85-01). The toolkit parses
/// this verbatim — SSM export and store provisioning are deployment-time
/// concerns handled outside config parsing (D-02 parse-only + lazy startup).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SharedPolicyStoreSection {
    /// Whether this server creates the shared policy store for all SQL servers.
    #[serde(default)]
    pub creates_shared_store: bool,
    /// Whether the created store's identifier is exported to SSM Parameter Store.
    #[serde(default)]
    pub export_to_ssm: bool,
    /// SSM Parameter Store path the store identifier is exported to (when
    /// `export_to_ssm = true`).
    #[serde(default)]
    pub ssm_path: Option<String>,
    /// Cedar policy-template names included in the shared store (e.g.
    /// `"PermitAllSelects"`, `"ForbidAllDeletes"`).
    #[serde(default)]
    pub templates: Vec<String>,
}

// -----------------------------------------------------------------------------
// [[config_slots]]
// -----------------------------------------------------------------------------

/// The kind of a declared `[[config_slots]]` entry — a CLOSED vocabulary.
///
/// Deliberately an enum rather than a free `String`. A free string lets a typo
/// (`kind = "endpont"`) parse cleanly, survive
/// [`ServerConfig::validate`], and fail only at package time when it maps to no
/// slot type — the failure surfacing two crates away from its cause. As a closed
/// enum, an unrecognized discriminator is a serde parse error naming the
/// accepted set, and a fourth kind becomes a deliberate addition here rather
/// than a silent pass-through.
///
/// # Why this type is toolkit-LOCAL
///
/// The three `snake_case` discriminators (`endpoint`, `secret`, `auth_mode`)
/// are deliberately the same strings the `pmcp-package` slot-type discriminator
/// uses for the corresponding variants, so a packaging tool can compare a
/// declaration against a package slot **without either crate depending on the
/// other**. The toolkit must NOT depend on `pmcp-package`: that crate is the
/// workspace-excluded leaf, and a toolkit dependency on it inverts the layering.
/// The agreement is enforced by the package side re-parsing the SAME config
/// bytes, not by a shared type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSlotKind {
    /// A network endpoint the target environment must supply (e.g. the backend
    /// API root). Behaviour-relevant: its `tested_value` records the endpoint
    /// the package was tested against.
    #[default]
    Endpoint,
    /// A named secret the target environment must supply (e.g. an API key).
    /// Identity-bearing: it structurally carries no `tested_value`.
    Secret,
    /// The backend authentication MODE. Structural rather than value-bearing:
    /// the auth-mode key is a serde tag, so no `${VAR}` placeholder form of it
    /// can deserialize — the baked literal IS the default and deviation
    /// surfaces through slot classification, not through a placeholder.
    AuthMode,
}

/// Single `[[config_slots]]` entry — a config value the TARGET environment must
/// fill for this server to run.
///
/// A Shape A server's whole identity is its config, so "what must the operator
/// supply?" has to be declarable IN that config rather than discovered by
/// grepping for `${...}`. This block is that declaration: it names the config
/// path, the kind of thing it is, and the value exercised when the server was
/// tested.
///
/// Additive per the REF-01 superset invariant — a config omitting the block
/// parses to an empty [`ServerConfig::config_slots`]. Strict-parse discipline
/// (D-13) applies: `#[serde(deny_unknown_fields)]` rejects a typo'd inner key.
///
/// # Example
///
/// ```toml
/// [[config_slots]]
/// key = "backend.base_url"
/// kind = "endpoint"
/// name = "TFL_BASE_URL"
/// tested_value = "https://api.tfl.gov.uk"
/// ```
/// Who fills a config slot's value.
///
/// Mirrors `pmcp-package`'s `SuppliedBy` **by TOML value name, never by shared
/// type** — the same arrangement as [`ConfigSlotKind`], and for the same
/// reason: the toolkit must NOT depend on `pmcp-package` (the
/// workspace-excluded leaf), and a dependency the other way inverts the
/// layering. The agreement is enforced by the package side re-parsing these
/// same config bytes, not by a shared definition.
///
/// Defaults to [`Environment`](Self::Environment), so every config written
/// before this field existed keeps its exact meaning: the operator supplies it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSlotSuppliedBy {
    /// The operator supplies it in the target environment. The default, and the
    /// only class a package enumerates as REQUIRED of an operator.
    #[default]
    Environment,
    /// The hosting platform injects it at deploy time.
    Platform,
    /// The execution environment injects it (e.g. `AWS_LAMBDA_FUNCTION_NAME`);
    /// neither the operator nor the platform supplies it.
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigSlotDecl {
    /// The dotted TOML path this slot fills, e.g. `backend.base_url`,
    /// `backend.auth.query_params.app_key`, `backend.auth.type`.
    #[serde(default)]
    pub key: String,
    /// The slot kind ([`ConfigSlotKind`] — a closed vocabulary). REQUIRED: an
    /// entry omitting `kind` is a parse error, because a defaulted kind would
    /// silently mis-classify the slot.
    pub kind: ConfigSlotKind,
    /// The slot's declared name — for a `secret`, the environment-variable
    /// name; for an `endpoint`, the variable the `${VAR}` placeholder reads.
    #[serde(default)]
    pub name: String,
    /// The value exercised when the server was tested. `None` for
    /// identity-bearing slots (a secret), which structurally carry no value —
    /// ENFORCED by [`ServerConfig::validate`], not just stated: a `secret`
    /// entry carrying a `tested_value` is refused, because that field is the
    /// one place a real credential could sit in a config that is served but
    /// never packed.
    #[serde(default)]
    pub tested_value: Option<String>,
    /// Who fills this slot — see [`ConfigSlotSuppliedBy`]. Defaults to
    /// `environment` (the operator supplies it), so a config written before this
    /// field existed is unchanged in meaning.
    ///
    /// This field is why the toolkit had to move in the same change as the
    /// packer: `deny_unknown_fields` above means a config carrying
    /// `supplied_by` would FAIL TO BOOT if only the package side learned it,
    /// and `pmcp-package` refuses to pack a config it knows the server cannot
    /// parse. Both sides accept it, or neither does.
    #[serde(default)]
    pub supplied_by: ConfigSlotSuppliedBy,
}

// -----------------------------------------------------------------------------
// [[tools]]
// -----------------------------------------------------------------------------

/// Single `[[tools]]` entry — a declaratively-defined tool surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ToolDecl {
    /// Tool name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Human-readable tool description.
    #[serde(default)]
    pub description: Option<String>,
    /// SQL template (uses `:param` placeholders bound by [`ParamDecl`]).
    #[serde(default)]
    pub sql: Option<String>,
    /// HTTP request path for a **single-call** OpenAPI/REST tool (D-01), e.g.
    /// `"/Line/Mode/tube/Status"`. Concatenated onto the backend `base_url`
    /// (or this tool's [`Self::base_url`] override). Additive per REF-01 — `None`
    /// for SQL / script tools.
    #[serde(default)]
    pub path: Option<String>,
    /// HTTP method for a single-call tool (`"GET"`, `"POST"`, …). Pairs with
    /// [`Self::path`] (D-01). Additive; `None` for SQL / script tools.
    #[serde(default)]
    pub method: Option<String>,
    /// Per-tool backend base-URL override. When absent a single-call tool
    /// inherits `[backend].base_url`. Additive; `None` for SQL / script tools.
    #[serde(default)]
    pub base_url: Option<String>,
    /// JavaScript body for a **script** tool (D-01) — a code-mode snippet that
    /// orchestrates multiple backend calls and binds `[[tools.parameters]]` to
    /// `args`. When set, this entry is a script tool ([`Self::is_script_tool`]).
    /// Additive; `None` for SQL / single-call tools.
    #[serde(default)]
    pub script: Option<String>,
    /// Optional UI-resource URI for `structuredContent` widgets.
    #[serde(default)]
    pub ui_resource_uri: Option<String>,
    /// `[[tools.parameters]]` — declared input parameters.
    #[serde(default)]
    pub parameters: Vec<ParamDecl>,
    /// `[tools.annotations]` — MCP `toolAnnotations`.
    #[serde(default)]
    pub annotations: Option<AnnotationsDecl>,
}

impl ToolDecl {
    /// Whether this `[[tools]]` entry is a **script** tool (D-01 detection rule).
    ///
    /// The detection rule is: `script.is_some()` ⇒ script tool; otherwise a
    /// `path` + `method` pair ⇒ single-call HTTP tool; otherwise (a `sql`
    /// field) ⇒ SQL tool. Plan 03/05 synthesizers branch on this method so the
    /// rule lives in exactly one place. Mutual-exclusivity is enforced at
    /// [`ServerConfig::validate`] (an entry mixing kinds is rejected, not
    /// silently resolved by precedence).
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ToolDecl;
    ///
    /// let script = ToolDecl { script: Some("await api.get('/x')".into()), ..Default::default() };
    /// assert!(script.is_script_tool());
    ///
    /// let single = ToolDecl {
    ///     path: Some("/Line/Mode/tube/Status".into()),
    ///     method: Some("GET".into()),
    ///     ..Default::default()
    /// };
    /// assert!(!single.is_script_tool());
    /// ```
    #[must_use]
    pub fn is_script_tool(&self) -> bool {
        self.script.is_some()
    }

    /// Number of distinct mutually-exclusive tool kinds declared on this entry.
    ///
    /// Used by [`ServerConfig::validate`] to reject an ambiguous `[[tools]]`
    /// entry (D-01 / T-90-02-04). A well-formed entry declares exactly one kind
    /// (count `1`); count `> 1` is ambiguous; count `0` is a kind-less stub
    /// (left to other validation rules).
    fn declared_kind_count(&self) -> usize {
        let is_sql = self.sql.is_some();
        let is_single_call = self.path.is_some() || self.method.is_some();
        let is_script = self.script.is_some();
        usize::from(is_sql) + usize::from(is_single_call) + usize::from(is_script)
    }

    /// Where `param_name` sits for the purposes of the D3 default length cap
    /// (Phase 128).
    ///
    /// # Derivation
    ///
    /// - The name appears as a `{name}` segment of [`Self::path`] -> [`ParamPosition::Path`].
    /// - Else [`Self::method`] is `GET`, `HEAD` or `DELETE` -> [`ParamPosition::Query`].
    /// - Else [`ParamPosition::Body`] — which covers `POST`/`PUT`/`PATCH` payload
    ///   fields, SQL named binds, and script-tool arguments alike.
    ///
    /// # Coupling with `tools.rs::build_operation` — and the DELIBERATE divergence
    ///
    /// The PATH arm must agree with `tools.rs::build_operation`'s `path_param_names`
    /// derivation exactly, because a divergence there would silently mis-scope the
    /// cap and, worse, mis-scope the D4 placeholder rules that share the same
    /// notion of "this value lands in the URL". That agreement is structural, not
    /// documentary: both read the same [`path_placeholder_names`] helper. A unit
    /// test additionally asserts agreement for a single-call tool.
    ///
    /// The QUERY/BODY split for NON-path parameters deliberately does NOT match
    /// `build_operation`, which marks every non-path declared parameter
    /// `ParameterLocation::Query` regardless of method. That is not a bug in either
    /// function and must not be "fixed": the two answer different questions.
    /// `build_operation` answers *where does this value travel* — and a `POST`
    /// tool's `Operation` has carried `Query` for its non-path parameters since
    /// Phase 90. `param_position` answers *where is length dangerous*. Applying a
    /// hard 256-code-point cap to a `POST` tool's `comment` or `body_text` field
    /// would break exactly the free-text parameters D-05 exists to protect, so a
    /// mutating method's non-path parameters are `Body` here.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_server_toolkit::config::{ParamPosition, ToolDecl};
    ///
    /// let search = ToolDecl {
    ///     path: Some("/lines/{line_id}/status".into()),
    ///     method: Some("GET".into()),
    ///     ..Default::default()
    /// };
    /// assert_eq!(search.param_position("line_id"), ParamPosition::Path);
    /// assert_eq!(search.param_position("detail"), ParamPosition::Query);
    ///
    /// let comment = ToolDecl {
    ///     path: Some("/issues/{id}/comments".into()),
    ///     method: Some("POST".into()),
    ///     ..Default::default()
    /// };
    /// assert_eq!(comment.param_position("id"), ParamPosition::Path);
    /// // Free text on a mutating method is NOT capped — D-05.
    /// assert_eq!(comment.param_position("body_text"), ParamPosition::Body);
    ///
    /// let sql = ToolDecl { sql: Some("SELECT 1".into()), ..Default::default() };
    /// assert_eq!(sql.param_position("anything"), ParamPosition::Body);
    /// ```
    #[must_use]
    pub fn param_position(&self, param_name: &str) -> ParamPosition {
        if let Some(path) = self.path.as_deref() {
            if path_placeholder_names(path).any(|n| n == param_name) {
                return ParamPosition::Path;
            }
        }
        match self.method.as_deref() {
            Some(m) if QUERY_BEARING_METHODS.contains(&m.to_uppercase().as_str()) => {
                ParamPosition::Query
            },
            _ => ParamPosition::Body,
        }
    }
}

/// HTTP methods whose non-path inputs genuinely travel in the query string, and
/// where an unbounded value is therefore dangerous rather than merely large.
const QUERY_BEARING_METHODS: [&str; 3] = ["GET", "HEAD", "DELETE"];

/// The `{name}` placeholder segments of a single-call tool's `path` template.
///
/// The ONE definition of that rule. `tools.rs::build_operation` reads it to build
/// its `Parameter` list and [`ToolDecl::param_position`] reads it to decide PATH
/// position, so the two cannot drift — which matters because a drift would
/// silently mis-scope the D3 cap and the D4 placeholder rules.
///
/// Matches a whole `/`-delimited segment only, and requires at least one character
/// between the braces (`{}` is not a placeholder).
pub(crate) fn path_placeholder_names(path: &str) -> impl Iterator<Item = &str> {
    path.split('/')
        .filter(|s| s.starts_with('{') && s.ends_with('}') && s.len() > 2)
        .map(|s| &s[1..s.len() - 1])
}

/// Where a declared parameter's value lands, for the purposes of the D3 default
/// length cap (Phase 128).
///
/// See [`ToolDecl::param_position`] for the derivation and for why the
/// query/body split deliberately differs from `tools.rs::build_operation`'s
/// `ParameterLocation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamPosition {
    /// A `{name}` segment of the tool's `path` template. Capped by default: a long
    /// value here has no legitimate use and is where the path-traversal class lives.
    Path,
    /// A non-path parameter of a `GET` / `HEAD` / `DELETE` tool, which travels in
    /// the query string.
    ///
    /// Capped at `[server.validation] default_max_length` code points by default.
    /// If a search tool starts refusing long queries after upgrading, this is why —
    /// declare an explicit `max_length` on that parameter to raise the limit.
    Query,
    /// Everything else: a `POST` / `PUT` / `PATCH` payload field, a SQL named bind,
    /// a script-tool argument.
    ///
    /// NOT capped by default (D-05 — free text must keep working). Surfaced by
    /// [`ServerConfig::lint`] and promotable to an error by
    /// `[server.validation] strict`.
    Body,
}

/// Single `[[tools.parameters]]` entry.
///
/// The `default` and `enum` fields use [`toml::Value`] because they are
/// heterogeneous in the reference configs (a `default` may be an integer,
/// a string, or a boolean depending on the parameter type).
///
/// # Forward incompatibility (Phase 128, D-15)
///
/// This struct carries `#[serde(deny_unknown_fields)]`, so a config declaring any
/// of the Phase 128 D2 keys — `pattern`, `min_length`, `format`, `max_items`,
/// `allow_slash`, or an `[tools.parameters.items]` table — fails to PARSE on
/// toolkit 0.1.3 rather than degrading to "the key was ignored". That is
/// deliberate (a silently-ignored validation rule is the class this phase closes)
/// but it means a config written for this release cannot be loaded by an older
/// toolkit. The same applies to the `[server.validation]` section. Named in the
/// CHANGELOG.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ParamDecl {
    /// Parameter name (the `:param` token used in the tool's `sql`).
    #[serde(default)]
    pub name: String,
    /// JSON-schema type (`"string"`, `"integer"`, `"number"`, `"boolean"`).
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,
    /// Human-readable parameter description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
    /// Optional default value (any TOML type).
    #[serde(default)]
    pub default: Option<toml::Value>,
    /// Maximum string length (string parameters only).
    #[serde(default)]
    pub max_length: Option<u64>,
    /// Inclusive minimum (integer / number parameters only).
    ///
    /// Stored as `f64`. See [`Self::maximum`] for the precision limit that applies
    /// to both bounds.
    #[serde(default)]
    pub minimum: Option<f64>,
    /// Inclusive maximum (integer / number parameters only).
    ///
    /// # Not a safe way to bound a 64-bit integer ID
    ///
    /// Both bounds are stored as `f64`, so an integer magnitude above 2^53
    /// (`9007199254740992`) cannot be represented exactly. `9007199254740993`
    /// written in TOML has ALREADY become `9007199254740992` by the time any code
    /// in this crate sees it, and no post-parse check can recover the fact that it
    /// was rounded. [`ServerConfig::validate`] therefore refuses a bound that is
    /// non-finite or whose magnitude EXCEEDS 2^53
    /// ([`ConfigValidationError::NonFiniteParamBound`]) — which catches the wildly
    /// out-of-range case, and deliberately does not claim to catch a value sitting
    /// one unit past the boundary.
    ///
    /// If you need to bound a `u64` identifier, express the rule as a
    /// [`Self::pattern`] over its string form instead. This limitation is a
    /// documented one, not an oversight: adding an `i64`-typed bound vocabulary is
    /// out of scope for D2.
    #[serde(default)]
    pub maximum: Option<f64>,
    /// Closed set of allowed values (any TOML scalar).
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<toml::Value>>,
    /// Regular expression the value must match, emitted as JSON Schema `pattern`
    /// (string parameters only).
    ///
    /// # It is UNANCHORED
    ///
    /// JSON Schema `pattern` is a SUBSTRING search, exactly as ECMA-262
    /// `RegExp.prototype.test` is. A rule written as a bare character class such as
    /// `[A-Z]{3}` matches `"../../etc/passwd-ABC"` and therefore buys no
    /// enforcement whatsoever. Anchor every rule you mean as a whole-value rule:
    /// `^[A-Z]{3}$`.
    ///
    /// # `\s` and `\S` do not mean one thing here
    ///
    /// Two regex engines are live inside one `jsonschema` 0.49.2 process, and which
    /// one evaluates your pattern depends on the pattern's own syntax:
    ///
    /// - A plain pattern takes the linear-time engine, whose `\s` is a PARTIAL
    ///   ECMA-262 set — measured as
    ///   `{U+0009, U+000A, U+000B, U+000C, U+000D, U+0020, U+00A0, U+2029, U+FEFF}`.
    ///   It does NOT include U+3000 IDEOGRAPHIC SPACE, U+0085 NEL, U+1680,
    ///   U+2000, U+2007, U+2028 or U+202F, and it DOES include the byte-order mark.
    /// - A pattern containing a lookaround or a backreference takes the
    ///   backtracking engine, where `\s` is exactly `\p{White_Space}` — so it DOES
    ///   match U+3000, and does NOT match U+FEFF.
    ///
    /// Adding a lookahead to a pattern therefore silently changes what `\s` means
    /// in it. For anything security-relevant, spell out an explicit character class
    /// (e.g. `[^\p{White_Space}]`) rather than using the shorthand.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Minimum string length in Unicode code points, emitted as JSON Schema
    /// `minLength` (string parameters only).
    ///
    /// Counted in code points — not bytes and not grapheme clusters — matching
    /// `maxLength`'s unit so a `min_length == max_length` pair names exactly one
    /// length.
    #[serde(default)]
    pub min_length: Option<u64>,
    /// JSON Schema `format` assertion, e.g. `"uuid"`, `"email"`, `"date-time"`.
    ///
    /// # It IS enforced on inputs in this SDK
    ///
    /// `format` is ANNOTATIVE by default in `jsonschema` 0.49 — a bare
    /// `draft202012` validator accepts `"!!!not-a-uuid!!!"` against
    /// `format = "uuid"`. Declared inputs do not take that path: core `pmcp`
    /// compiles a tool's `inputSchema` through a format-ASSERTING builder
    /// (Phase 128, Q1), so a declared `format` refuses a non-conforming value at
    /// `tools/call` time.
    ///
    /// Measured under this workspace's pinned `jsonschema` configuration
    /// (`0.49`, `default-features = false`), all NINETEEN standard Draft 2020-12
    /// format names assert: `date-time`, `date`, `time`, `duration`, `email`,
    /// `idn-email`, `hostname`, `idn-hostname`, `ipv4`, `ipv6`, `uri`,
    /// `uri-reference`, `iri`, `iri-reference`, `uuid`, `uri-template`,
    /// `json-pointer`, `relative-json-pointer`, `regex`. A format name OUTSIDE
    /// that list is accepted-and-ignored, per JSON Schema's own rule that an
    /// unknown format is an annotation — so a typo such as `"uid"` for `"uuid"`
    /// silently enforces nothing.
    ///
    /// `format` is NOT enforced on OUTPUTS: `structuredContent` validation is
    /// deliberately annotative there, and only warns.
    #[serde(default)]
    pub format: Option<String>,
    /// `[tools.parameters.items]` — the element schema for an array parameter,
    /// emitted as JSON Schema `items`.
    ///
    /// Emitted in OBJECT form only. Array-form `items` (the draft-07 tuple
    /// construct) does not compile under the Draft 2020-12 pin and would take the
    /// whole tool's validator down with it.
    #[serde(default)]
    pub items: Option<ItemsDecl>,
    /// Maximum number of array elements, emitted as JSON Schema `maxItems`
    /// (array parameters only).
    #[serde(default)]
    pub max_items: Option<u64>,
    /// Permit `/` inside this parameter's value when it is interpolated into a
    /// single-call tool's path template (Phase 128, D-11).
    ///
    /// Path-placeholder values are refused for path separators by default, because
    /// a `/` in a `{segment}` lets a caller reshape the request target. This
    /// per-parameter opt-in is the ONLY legitimate source of that permission — it
    /// exists for the genuine case of a parameter that names a multi-segment
    /// resource path.
    ///
    /// An OpenAPI spec's `allowReserved` must NEVER be wired to this field. That
    /// keyword describes URL percent-encoding latitude in the spec author's
    /// serialization rules; it is not a statement that the value may restructure
    /// the path, and treating it as one would turn a routine spec detail into a
    /// silent path-traversal opening.
    #[serde(default)]
    pub allow_slash: bool,
}

/// `[tools.parameters.items]` — the element schema of an array parameter
/// (Phase 128, D2).
///
/// Emitted into `inputSchema` as an OBJECT-form JSON Schema `items` value. The
/// array form of `items` is a draft-07 tuple construct that does not compile under
/// the Draft 2020-12 pin, so this struct has no way to express it by design.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ItemsDecl {
    /// Element type (`"string"`, `"integer"`, …). Defaults to `"string"` when
    /// omitted, matching [`ParamDecl::param_type`]'s convention.
    #[serde(default, rename = "type")]
    pub item_type: Option<String>,
    /// Maximum element length in Unicode code points (string elements only).
    #[serde(default)]
    pub max_length: Option<u64>,
    /// Regular expression each element must match. UNANCHORED — see
    /// [`ParamDecl::pattern`] for the anchoring and two-engine `\s` caveats, which
    /// apply identically here.
    #[serde(default)]
    pub pattern: Option<String>,
}

/// `[tools.annotations]` — MCP `toolAnnotations` hints.
#[allow(clippy::struct_excessive_bools)] // Why: REF-01 superset — mirrors the MCP `toolAnnotations` flag set 1:1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AnnotationsDecl {
    /// Whether the tool only reads (never mutates) state.
    #[serde(default)]
    pub read_only_hint: bool,
    /// Whether the tool may destroy data.
    #[serde(default)]
    pub destructive_hint: bool,
    /// Whether repeated calls with the same args produce the same result.
    #[serde(default)]
    pub idempotent_hint: bool,
    /// Whether the tool interacts with an open-world (external) service.
    #[serde(default)]
    pub open_world_hint: bool,
    /// Cost hint (`"low"`, `"medium"`, `"high"`).
    #[serde(default)]
    pub cost_hint: Option<String>,
}

// -----------------------------------------------------------------------------
// [[prompts]]
// -----------------------------------------------------------------------------

/// Single `[[prompts]]` entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PromptDecl {
    /// Prompt name (the identifier MCP clients call by).
    #[serde(default)]
    pub name: String,
    /// Human-readable prompt description.
    #[serde(default)]
    pub description: Option<String>,
    /// Resource URIs to include in the prompt's assembled body.
    #[serde(default)]
    pub include_resources: Vec<String>,
    /// Declared prompt arguments (MCP `PromptArgument`).
    #[serde(default)]
    pub arguments: Vec<PromptArgumentDecl>,
}

/// Single argument under `[[prompts.arguments]]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PromptArgumentDecl {
    /// Argument name.
    #[serde(default)]
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the argument is required.
    #[serde(default)]
    pub required: bool,
}

// -----------------------------------------------------------------------------
// [[resources]]
// -----------------------------------------------------------------------------

/// Single `[[resources]]` entry — a statically-shipped resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ResourceDecl {
    /// Resource URI (e.g. `"docs://open-images/schema"`).
    #[serde(default)]
    pub uri: String,
    /// Human-readable resource name.
    #[serde(default)]
    pub name: Option<String>,
    /// Resource description.
    #[serde(default)]
    pub description: Option<String>,
    /// MIME type (e.g. `"text/markdown"`).
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Inline resource content (or `"loaded from path.md"` placeholder string —
    /// the toolkit treats the value verbatim; resolution to filesystem reads
    /// is the caller's responsibility).
    #[serde(default)]
    pub content: Option<String>,
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const MINIMAL: &str = r#"
        [server]
        name = "demo"
        version = "0.1.0"
    "#;

    #[test]
    fn parse_minimal_config_succeeds() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("minimal must parse");
        assert_eq!(cfg.server.name, "demo");
        assert_eq!(cfg.server.version, "0.1.0");
        assert!(cfg.tools.is_empty());
        assert!(cfg.code_mode.is_none());
    }

    #[test]
    fn parse_unknown_field_fails() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            unknown_field = "x"
        "#;
        let err = ServerConfig::from_toml(toml).expect_err("unknown field must fail");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");
    }

    #[test]
    fn parse_typo_in_code_mode_key_fails() {
        // T-83-04-02: defence-in-depth against silent policy widening.
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            [code_mode]
            enabled = true
            auto_aprove_levels = ["low"]
        "#;
        let err = ServerConfig::from_toml(toml).expect_err("typo'd code_mode key must be rejected");
        assert!(matches!(err, ToolkitError::Parse(_)));
    }

    #[test]
    fn code_mode_section_optional() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        assert!(cfg.code_mode.is_none());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        cfg.validate().expect("minimal config must validate");
    }

    #[test]
    fn validate_rejects_empty_server_name() {
        let toml = r#"
            [server]
            name = ""
            version = "0.1.0"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyServerName) => {},
            other => panic!("expected EmptyServerName, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_server_version() {
        let toml = r#"
            [server]
            name = "demo"
            version = ""
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyServerVersion) => {},
            other => panic!("expected EmptyServerVersion, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_tool_name() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "ok"
            description = "first"

            [[tools]]
            name = ""
            description = "second-is-empty"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyToolName(1)) => {},
            other => panic!("expected EmptyToolName(1), got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_table_name() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[database.tables]]
            name = ""
            description = "missing-name"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyTableName(0)) => {},
            other => panic!("expected EmptyTableName(0), got {other:?}"),
        }
    }

    /// Phase 90 gap-closure (GAP 3 / WR-02): a `[backend]` block with an
    /// empty / missing `base_url` is rejected at validate() time with
    /// [`ConfigValidationError::EmptyBackendBaseUrl`] — not a late opaque
    /// `DispatchError::Connector("invalid base URL")` at request time.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_empty_backend_base_url() {
        // base_url key present but empty.
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = ""
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyBackendBaseUrl) => {},
            other => panic!("expected EmptyBackendBaseUrl, got {other:?}"),
        }
    }

    /// A `[backend]` block whose `base_url` key is omitted entirely (defaults
    /// to `""` via `#[serde(default)]`) is rejected the same way.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_omitted_backend_base_url() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyBackendBaseUrl) => {},
            other => panic!("expected EmptyBackendBaseUrl, got {other:?}"),
        }
    }

    /// A multi-placeholder composition (`${SCHEME}://${HOST}`) is a MALFORMED
    /// reference — the grammar resolves one whole-value `${VAR}`, it does not
    /// interpolate — so validate() refuses it at load time instead of letting
    /// every boot fail with an `UnresolvedBaseUrlRef` naming an empty variable.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_multi_placeholder_backend_base_url() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "${TFL_SCHEME}://${TFL_HOST}"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::MalformedBackendBaseUrlRef) => {},
            other => panic!("expected MalformedBackendBaseUrlRef, got {other:?}"),
        }
    }

    /// The empty `${}` form is the same class of defect and gets the same
    /// load-time refusal.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_empty_name_backend_base_url_ref() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "${}"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::MalformedBackendBaseUrlRef) => {},
            other => panic!("expected MalformedBackendBaseUrlRef, got {other:?}"),
        }
    }

    /// A well-formed single reference stays valid — the check refuses only
    /// malformed shapes, never the deferred-to-environment pattern itself.
    #[cfg(feature = "http")]
    #[test]
    fn validate_accepts_single_reference_backend_base_url() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "${TFL_BASE_URL}"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        cfg.validate()
            .expect("a single ${VAR} backend.base_url reference must validate");
    }

    /// The SAME malformed-reference rule applies to `[backend.auth]`
    /// credentials, and it applies at LOAD time. Without it the credential path
    /// resolved a malformed reference to the empty string and then OMITTED it:
    /// the server booted, every backend call went out unauthenticated, and
    /// nothing was logged. `${TFL-APP-KEY}` is the realistic shape — a dash is
    /// not a portably settable variable name, so the reference names nothing.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_malformed_backend_auth_credential_ref() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"

            [backend.auth]
            type = "bearer"
            token = "${TFL-APP-KEY}"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::MalformedBackendAuthRef(field)) => {
                assert_eq!(field, "token");
            },
            other => panic!("expected MalformedBackendAuthRef, got {other:?}"),
        }
    }

    /// The api_key map path gets the same refusal, and the error names the
    /// offending entry so the operator knows WHICH parameter to fix.
    #[cfg(feature = "http")]
    #[test]
    fn validate_rejects_malformed_backend_auth_api_key_entry() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"

            [backend.auth]
            type = "api_key"
            query_params = { app_key = "${TFL_SCHEME}://${TFL_HOST}" }
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::MalformedBackendAuthRef(field)) => {
                assert_eq!(field, "query_params.app_key");
            },
            other => panic!("expected MalformedBackendAuthRef, got {other:?}"),
        }
    }

    /// The refusal is scoped to MALFORMED shapes only: a well-formed reference
    /// and a plain literal both still validate, so the deferred-to-environment
    /// pattern and committed dev configs are untouched.
    #[cfg(feature = "http")]
    #[test]
    fn validate_accepts_wellformed_and_literal_backend_auth_credentials() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"

            [backend.auth]
            type = "basic"
            username = "svc-account"
            password = "${TFL_APP_KEY}"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        cfg.validate()
            .expect("a literal username and a single ${VAR} password must validate");
    }

    /// A `[backend]` block with a non-empty `base_url` validates OK.
    #[cfg(feature = "http")]
    #[test]
    fn validate_accepts_non_empty_backend_base_url() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        cfg.validate()
            .expect("config with a non-empty backend.base_url must validate");
    }

    /// A config with NO `[backend]` block (a pure-SQL config) is unaffected by
    /// the new check — `backend` is `None`, so the check never fires.
    #[cfg(feature = "http")]
    #[test]
    fn validate_accepts_absent_backend() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        assert!(cfg.backend.is_none());
        cfg.validate()
            .expect("a config without [backend] must validate (SQL configs unaffected)");
    }

    /// The error Display names the offending field and is actionable.
    #[cfg(feature = "http")]
    #[test]
    fn empty_backend_base_url_error_names_the_field() {
        let msg = ConfigValidationError::EmptyBackendBaseUrl.to_string();
        assert!(
            msg.contains("[backend].base_url"),
            "error must name the field, got: {msg}"
        );
    }

    #[test]
    fn database_url_optional_field_parses() {
        // Phase 84 CONN-04 / D-08: the additive `[database].url` field parses
        // under `#[serde(deny_unknown_fields)]` and carries the `env:VAR_NAME`
        // indirection string verbatim (resolution happens at the consumer layer).
        let toml = r#"
            [server]
            name = "x"
            version = "0.0.1"

            [database]
            url = "env:DATABASE_URL"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("config with [database].url must parse");
        assert_eq!(cfg.database.url, Some("env:DATABASE_URL".to_string()));
    }

    #[test]
    fn from_toml_strict_validated_rolls_both_errors() {
        // 1. Parse error path (unknown field).
        let bad_toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            nonsense = "x"
        "#;
        let err = ServerConfig::from_toml_strict_validated(bad_toml)
            .expect_err("unknown field must surface");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");

        // 2. Validation error path (empty required value).
        let invalid_toml = r#"
            [server]
            name = ""
            version = "0.1.0"
        "#;
        let err = ServerConfig::from_toml_strict_validated(invalid_toml)
            .expect_err("empty name must surface");
        assert!(
            matches!(
                err,
                ToolkitError::Validation(ConfigValidationError::EmptyServerName)
            ),
            "got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // ToolDecl two-kind detection — D-01 (shared, not http-gated)
    // -------------------------------------------------------------------------

    #[test]
    fn test_tooldecl_single_call_parses() {
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[tools]]
            name = "tube_status"
            path = "/Line/Mode/tube/Status"
            method = "GET"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("single-call tool must parse");
        let tool = &cfg.tools[0];
        assert_eq!(tool.path.as_deref(), Some("/Line/Mode/tube/Status"));
        assert_eq!(tool.method.as_deref(), Some("GET"));
        assert!(!tool.is_script_tool());
        cfg.validate()
            .expect("single-call tool is a valid single kind");
    }

    #[test]
    fn test_tooldecl_script_parses() {
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[tools]]
            name = "plan_journey"
            script = """
            const a = await api.get('/Journey/JourneyResults/' + args.from + '/to/' + args.to);
            return a;
            """

            [[tools.parameters]]
            name = "from"
            type = "string"
            required = true

            [[tools.parameters]]
            name = "to"
            type = "string"
            required = true
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("script tool must parse");
        let tool = &cfg.tools[0];
        assert!(tool.script.is_some());
        assert!(tool.is_script_tool());
        assert_eq!(tool.parameters.len(), 2);
        cfg.validate().expect("script tool is a valid single kind");
    }

    #[test]
    fn test_tooldecl_detection() {
        let script = ToolDecl {
            script: Some("return 1;".to_string()),
            ..Default::default()
        };
        assert!(script.is_script_tool());

        let single = ToolDecl {
            path: Some("/x".to_string()),
            method: Some("GET".to_string()),
            ..Default::default()
        };
        assert!(!single.is_script_tool());

        let sql = ToolDecl {
            sql: Some("SELECT 1".to_string()),
            ..Default::default()
        };
        assert!(!sql.is_script_tool());
    }

    #[test]
    fn test_tooldecl_ambiguous_rejected() {
        // script + path/method is ambiguous (Codex MEDIUM): rejected, not
        // resolved by a silent "script wins".
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[tools]]
            name = "confused"
            path = "/x"
            method = "GET"
            script = "return 1;"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse (ambiguity is a validate-time rule)");
        match cfg.validate() {
            Err(ConfigValidationError::AmbiguousToolKind(0)) => {},
            other => panic!("expected AmbiguousToolKind(0), got {other:?}"),
        }
    }

    #[test]
    fn test_tooldecl_ambiguous_sql_plus_script_rejected() {
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[tools]]
            name = "confused"
            sql = "SELECT 1"
            script = "return 1;"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::AmbiguousToolKind(0)) => {},
            other => panic!("expected AmbiguousToolKind(0), got {other:?}"),
        }
    }

    #[test]
    fn test_tooldecl_sql_still_parses() {
        // REF-01 superset regression: an existing sql= tool is unaffected by the
        // additive path/method/base_url/script fields.
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "list_tables"
            sql = "SELECT name FROM sqlite_master"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("sql tool must still parse");
        let tool = &cfg.tools[0];
        assert_eq!(tool.sql.as_deref(), Some("SELECT name FROM sqlite_master"));
        assert!(tool.path.is_none());
        assert!(tool.method.is_none());
        assert!(tool.base_url.is_none());
        assert!(tool.script.is_none());
        assert!(!tool.is_script_tool());
        cfg.validate().expect("sql tool validates as a single kind");
    }

    // -------------------------------------------------------------------------
    // [backend] / [backend.auth] / [backend.http] — D-06 (http feature)
    // -------------------------------------------------------------------------

    #[cfg(feature = "http")]
    #[test]
    fn test_backend_section_parses() {
        // A full [backend] + [backend.auth] (api_key) + [backend.http] block
        // round-trips into ServerConfig with backend.is_some().
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [backend]
            base_url = "https://api.tfl.gov.uk"

            [backend.auth]
            type = "api_key"

            [backend.auth.query_params]
            app_key = "${TFL_APP_KEY}"

            [backend.http]
            timeout_seconds = 10
            retries = 2
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("[backend] config must parse");
        let backend = cfg.backend.expect("backend must be Some");
        assert_eq!(backend.base_url, "https://api.tfl.gov.uk");
        assert_eq!(backend.http.timeout_seconds, 10);
        assert_eq!(backend.http.retries, 2);
        assert!(
            matches!(backend.auth, AuthConfig::ApiKey { .. }),
            "auth must be api_key, got {:?}",
            backend.auth
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_backend_auth_defaults_to_none() {
        // [backend] without a [backend.auth] sub-table defaults auth to None
        // and http to HttpConfig defaults (additive sub-tables).
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("backend w/o auth must parse");
        let backend = cfg.backend.expect("backend must be Some");
        assert!(matches!(backend.auth, AuthConfig::None));
        assert_eq!(backend.http, HttpConfig::default());
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_sql_config_unaffected() {
        // REF-01 superset / D-06 additive proof: a pure-SQL config with NO
        // [backend] still parses, and backend == None.
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [database]
            type = "sqlite"
            file_path = "/tmp/demo.db"

            [[tools]]
            name = "list_tables"
            sql = "SELECT name FROM sqlite_master"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("SQL config must still parse");
        assert!(
            cfg.backend.is_none(),
            "SQL config must have backend == None"
        );
        assert_eq!(cfg.tools.len(), 1);
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_backend_unknown_field_rejected() {
        // T-90-02-01: deny_unknown_fields preserved — an unknown key under
        // [backend.http] is a hard parse error, never a silent default.
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [backend]
            base_url = "https://api.example.com"

            [backend.http]
            foo = 1
        "#;
        let err =
            ServerConfig::from_toml(toml).expect_err("unknown [backend.http] key must be rejected");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // `[[config_slots]]` — PKG-03 slot declarations (Phase 120 Plan 04 Task 1)
    // -------------------------------------------------------------------------

    /// The three-slot declaration block the london-tube proving fixture carries.
    const CONFIG_SLOTS_TOML: &str = r#"
        [server]
        name = "london-tube"
        version = "1.1.0"

        [[config_slots]]
        key = "backend.base_url"
        kind = "endpoint"
        name = "TFL_BASE_URL"
        tested_value = "https://api.tfl.gov.uk"

        [[config_slots]]
        key = "backend.auth.query_params.app_key"
        kind = "secret"
        name = "TFL_APP_KEY"

        [[config_slots]]
        key = "backend.auth.type"
        kind = "auth_mode"
        name = "backend-auth-mode"
        tested_value = "api_key"
    "#;

    /// Test 1: a `[[config_slots]]` block parses through the STRICT + validated
    /// entry point and exposes all three entries with their fields intact.
    #[test]
    fn config_slots_block_parses_through_strict_entry_point() {
        let cfg = ServerConfig::from_toml_strict_validated(CONFIG_SLOTS_TOML)
            .expect("[[config_slots]] must parse through the strict entry point");
        assert_eq!(cfg.config_slots.len(), 3, "three declared slots");

        assert_eq!(cfg.config_slots[0].key, "backend.base_url");
        assert_eq!(cfg.config_slots[0].kind, ConfigSlotKind::Endpoint);
        assert_eq!(cfg.config_slots[0].name, "TFL_BASE_URL");
        assert_eq!(
            cfg.config_slots[0].tested_value.as_deref(),
            Some("https://api.tfl.gov.uk")
        );

        assert_eq!(cfg.config_slots[1].kind, ConfigSlotKind::Secret);
        assert_eq!(cfg.config_slots[1].name, "TFL_APP_KEY");
        assert_eq!(cfg.config_slots[2].kind, ConfigSlotKind::AuthMode);
    }

    /// A `[[config_slots]]` entry carrying `supplied_by` must BOOT.
    ///
    /// This is the load-bearing half of a two-crate change. `pmcp-package`
    /// refuses to pack a config whose fields this struct's
    /// `deny_unknown_fields` would reject, on the grounds that packing it would
    /// ship a server that cannot start. So if the packer learns `supplied_by`
    /// and this struct does not, every config using the field becomes
    /// unpackable; if this struct learns it and the packer does not, the packer
    /// rejects configs the server boots from happily. Both sides move together
    /// or neither does, and this test is the runtime half of that pin.
    #[test]
    fn a_config_slot_declaring_supplied_by_parses_through_the_strict_entry_point() {
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[config_slots]]
            key = "backend.base_url"
            kind = "endpoint"
            name = "TFL_BASE_URL"
            tested_value = "https://api.tfl.gov.uk"
            supplied_by = "platform"

            [[config_slots]]
            key = "backend.function_name"
            kind = "secret"
            name = "AWS_LAMBDA_FUNCTION_NAME"
            supplied_by = "runtime"
        "#;
        let cfg = ServerConfig::from_toml_strict_validated(toml)
            .expect("`supplied_by` must parse under deny_unknown_fields");
        assert_eq!(
            cfg.config_slots[0].supplied_by,
            ConfigSlotSuppliedBy::Platform
        );
        assert_eq!(
            cfg.config_slots[1].supplied_by,
            ConfigSlotSuppliedBy::Runtime
        );
    }

    /// Omitting it means `environment`, so every config written before the
    /// field existed keeps its meaning rather than failing to parse.
    #[test]
    fn a_config_slot_without_supplied_by_defaults_to_environment() {
        let cfg = ServerConfig::from_toml_strict_validated(CONFIG_SLOTS_TOML)
            .expect("the pre-existing fixture must still parse");
        for slot in &cfg.config_slots {
            assert_eq!(slot.supplied_by, ConfigSlotSuppliedBy::Environment);
        }
    }

    /// An unrecognized value is a parse ERROR, not a silent default — strict
    /// parse discipline (D-13). A defaulted typo here would tell an operator to
    /// supply a value the platform actually injects.
    #[test]
    fn an_unknown_supplied_by_value_is_a_parse_error() {
        let toml = r#"
            [server]
            name = "tube"
            version = "0.1.0"

            [[config_slots]]
            key = "backend.base_url"
            kind = "endpoint"
            name = "TFL_BASE_URL"
            tested_value = "x"
            supplied_by = "platfrom"
        "#;
        ServerConfig::from_toml_strict_validated(toml)
            .expect_err("a misspelled supplied_by must not silently default");
    }

    /// Test 2: the field is ADDITIVE — a config with no `[[config_slots]]` block
    /// parses unchanged and yields an empty vec (`#[serde(default)]`).
    #[test]
    fn config_without_config_slots_parses_with_empty_vec() {
        let cfg = ServerConfig::from_toml_strict_validated(MINIMAL)
            .expect("a config omitting [[config_slots]] still parses");
        assert!(
            cfg.config_slots.is_empty(),
            "absent block yields an empty vec, not a default entry"
        );
    }

    /// Test 3: `deny_unknown_fields` still bites at the TOP level — a typo'd
    /// `[[config_slotz]]` is a hard parse error, never a silently-ignored block.
    #[test]
    fn top_level_config_slots_typo_is_still_rejected() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[config_slotz]]
            key = "backend.base_url"
            kind = "endpoint"
            name = "TFL_BASE_URL"
        "#;
        let err = ServerConfig::from_toml(toml)
            .expect_err("a typo'd top-level array-of-tables must be rejected");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");
    }

    /// Test 4: the decl struct is itself `deny_unknown_fields` — a typo INSIDE
    /// the block (`nmae`) is rejected rather than silently dropped.
    #[test]
    fn config_slot_unknown_inner_key_is_rejected() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[config_slots]]
            key = "backend.base_url"
            kind = "endpoint"
            nmae = "TFL_BASE_URL"
        "#;
        let err = ServerConfig::from_toml(toml)
            .expect_err("an unknown key inside [[config_slots]] must be rejected");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");
    }

    /// Test 5: `tested_value` is OPTIONAL — an identity-bearing slot structurally
    /// carries no value, so omitting it parses to `None`.
    #[test]
    fn config_slot_tested_value_is_optional() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[config_slots]]
            key = "backend.auth.query_params.app_key"
            kind = "secret"
            name = "TFL_APP_KEY"
        "#;
        let cfg = ServerConfig::from_toml_strict_validated(toml)
            .expect("an entry without tested_value parses");
        assert_eq!(cfg.config_slots.len(), 1);
        assert!(
            cfg.config_slots[0].tested_value.is_none(),
            "omitted tested_value parses to None"
        );
    }

    /// Test 6 (Codex MEDIUM — the invalid-kind hole): `kind` is a CLOSED
    /// vocabulary. A typo such as `endpont` — or an empty string — is a PARSE
    /// error naming the accepted set, not a declaration that parses cleanly and
    /// then fails to map to any package slot type two crates away.
    #[test]
    fn config_slot_invalid_kind_is_rejected_naming_the_accepted_set() {
        for bad in ["endpont", ""] {
            let toml = format!(
                r#"
                [server]
                name = "demo"
                version = "0.1.0"

                [[config_slots]]
                key = "backend.base_url"
                kind = "{bad}"
                name = "TFL_BASE_URL"
                "#
            );
            let err = ServerConfig::from_toml(&toml)
                .expect_err("an unrecognized config-slot kind must be rejected at parse time");
            let rendered = err.to_string();
            for accepted in ["endpoint", "secret", "auth_mode"] {
                assert!(
                    rendered.contains(accepted),
                    "the error for kind = \"{bad}\" must name the accepted kind \
                     `{accepted}`: {rendered}"
                );
            }
        }
    }

    /// Test 7: all three valid kinds parse, and the parsed value is a CLOSED
    /// enum — comparable as `ConfigSlotKind`, not as a free string. A fourth
    /// kind is therefore a deliberate addition here, never a silent
    /// pass-through to the package side.
    #[test]
    fn config_slot_all_three_kinds_parse_as_a_closed_enum() {
        let cfg = ServerConfig::from_toml_strict_validated(CONFIG_SLOTS_TOML)
            .expect("all three kinds parse");
        let kinds: Vec<ConfigSlotKind> = cfg.config_slots.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ConfigSlotKind::Endpoint,
                ConfigSlotKind::Secret,
                ConfigSlotKind::AuthMode
            ],
            "kind is a closed enum, not a free string"
        );
    }

    /// `validate()` rejects an entry whose `key` or `name` is empty/whitespace,
    /// carrying the offending entry INDEX (the `EmptyTableName(i)` error shape).
    #[test]
    fn config_slot_empty_key_or_name_fails_validation() {
        for field in ["key", "name"] {
            let (key, name) = if field == "key" {
                ("   ", "TFL_BASE_URL")
            } else {
                ("backend.base_url", "  ")
            };
            let toml = format!(
                r#"
                [server]
                name = "demo"
                version = "0.1.0"

                [[config_slots]]
                key = "{key}"
                kind = "endpoint"
                name = "{name}"
                "#
            );
            let cfg = ServerConfig::from_toml(&toml).expect("parses; emptiness is semantic");
            let err = cfg
                .validate()
                .expect_err("an empty config-slot key/name must fail validation");
            assert!(
                matches!(err, ConfigValidationError::EmptyConfigSlotField(0)),
                "empty {field} must yield EmptyConfigSlotField(0), got: {err:?}"
            );
        }
    }

    /// `validate()` refuses a `secret` declaration carrying a `tested_value` —
    /// identity-bearing slots structurally record no value, and this field is
    /// the one place a REAL credential could sit in a config that is served
    /// but never packed (pack-time gates only run on packaging).
    #[test]
    fn config_slot_secret_with_tested_value_fails_validation_without_echoing_it() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[config_slots]]
            key = "backend.auth.query_params.app_key"
            kind = "secret"
            name = "TFL_APP_KEY"
            tested_value = "sentinel-real-credential"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parses; the rule is semantic");
        let err = cfg
            .validate()
            .expect_err("a secret slot carrying a tested_value must fail validation");
        assert!(
            matches!(err, ConfigValidationError::SecretSlotCarriesTestedValue(0)),
            "got: {err:?}"
        );
        assert!(
            !err.to_string().contains("sentinel-real-credential"),
            "the error must not echo the value: {err}"
        );
    }

    // -------------------------------------------------------------------------
    // Phase 128 D2 / SC-2 — the six new `ParamDecl` keys and the config-time
    // pattern-compile gate.
    // -------------------------------------------------------------------------

    /// D2: all six new keys parse from TOML, including the
    /// `[tools.parameters.items]` sub-table.
    #[test]
    fn param_decl_parses_all_d2_keys() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "batch_lookup"

            [[tools.parameters]]
            name = "codes"
            type = "array"
            required = true
            max_items = 25
            min_length = 2
            format = "uuid"
            pattern = "^[A-Z]{3}$"
            allow_slash = true

            [tools.parameters.items]
            type = "string"
            max_length = 8
            pattern = "^[a-z]+$"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        let p = &cfg.tools[0].parameters[0];
        assert_eq!(p.pattern.as_deref(), Some("^[A-Z]{3}$"));
        assert_eq!(p.min_length, Some(2));
        assert_eq!(p.format.as_deref(), Some("uuid"));
        assert_eq!(p.max_items, Some(25));
        assert!(p.allow_slash);
        let items = p.items.as_ref().expect("items sub-table");
        assert_eq!(items.item_type.as_deref(), Some("string"));
        assert_eq!(items.max_length, Some(8));
        assert_eq!(items.pattern.as_deref(), Some("^[a-z]+$"));
    }

    /// D2: the six new keys survive a `Serialize` -> `Deserialize` round trip, so
    /// a config re-emitted by the toolkit does not silently drop a declared rule.
    #[test]
    fn param_decl_d2_keys_round_trip_through_toml() {
        let original = ParamDecl {
            name: "codes".to_string(),
            param_type: Some("array".to_string()),
            required: true,
            pattern: Some("^[A-Z]{3}$".to_string()),
            min_length: Some(2),
            format: Some("uuid".to_string()),
            max_items: Some(25),
            allow_slash: true,
            items: Some(ItemsDecl {
                item_type: Some("string".to_string()),
                max_length: Some(8),
                pattern: Some("^[a-z]+$".to_string()),
            }),
            ..Default::default()
        };
        let cfg = ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools: vec![ToolDecl {
                name: "batch_lookup".to_string(),
                parameters: vec![original.clone()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = toml::to_string(&cfg).expect("serialize");
        let parsed = ServerConfig::from_toml(&text).expect("re-parse");
        assert_eq!(parsed.tools[0].parameters[0], original);
    }

    /// SC-2: a `pattern` that does not compile fails at CONFIG time, naming the
    /// offending parameter — not at call time, where it would take the whole
    /// tool's validator down.
    #[test]
    fn validate_rejects_uncompilable_param_pattern() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "lookup"

            [[tools.parameters]]
            name = "region"
            type = "string"
            pattern = "^[A-Z"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::UncompilableParamSchema {
                ref tool,
                ref position,
                ref detail,
            }) => {
                assert_eq!(tool, "lookup");
                assert!(
                    position.contains("region"),
                    "position must name the offending parameter, got {position:?}"
                );
                assert!(
                    !detail.is_empty(),
                    "the author-facing detail must be present"
                );
            },
            other => panic!("expected UncompilableParamSchema, got {other:?}"),
        }
    }

    /// SC-2 edge (adjacency): a pattern that COMPILES but matches nothing passes
    /// config validation. Config validation checks compilability, not
    /// satisfiability — `$^` will refuse every value at call time instead.
    #[test]
    fn validate_accepts_unsatisfiable_but_compilable_param_pattern() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "lookup"

            [[tools.parameters]]
            name = "region"
            type = "string"
            pattern = "$^"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        cfg.validate()
            .expect("an unsatisfiable pattern still compiles and must validate");
    }

    /// SC-2 edge (empty): an empty `pattern` matches everything, so it buys no
    /// enforcement while reading like a rule. Refused as a likely author error.
    #[test]
    fn validate_rejects_empty_param_pattern() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "lookup"

            [[tools.parameters]]
            name = "region"
            type = "string"
            pattern = ""
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyParamPattern {
                ref tool,
                ref param,
            }) => {
                assert_eq!(tool, "lookup");
                assert_eq!(param, "region");
            },
            other => panic!("expected EmptyParamPattern, got {other:?}"),
        }
    }

    /// D3 edge (precision): a bound whose magnitude exceeds 2^53 is refused,
    /// because `ParamDecl` stores bounds as `f64` and such a value cannot be
    /// represented exactly.
    #[test]
    fn validate_rejects_non_finite_param_bound() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "lookup"

            [[tools.parameters]]
            name = "count"
            type = "integer"
            maximum = 1e300
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::NonFiniteParamBound {
                ref tool,
                ref param,
            }) => {
                assert_eq!(tool, "lookup");
                assert_eq!(param, "count");
            },
            other => panic!("expected NonFiniteParamBound, got {other:?}"),
        }
    }

    /// D3 edge (precision), the honest half: a bound AT the 2^53 boundary is
    /// accepted. The check cannot see that a larger TOML integer was already
    /// rounded into this value, and the rustdoc says so rather than claiming a
    /// guarantee it cannot deliver.
    #[test]
    fn validate_accepts_param_bound_at_the_representable_boundary() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "lookup"

            [[tools.parameters]]
            name = "count"
            type = "integer"
            maximum = 9007199254740992
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        cfg.validate()
            .expect("a bound exactly at 2^53 is representable and must validate");
    }

    // -------------------------------------------------------------------------
    // Phase 128 D3 / D-07 — `[server.validation]`, `param_position`, `lint()`
    // -------------------------------------------------------------------------

    /// `[server.validation]` parses with all four keys.
    #[test]
    fn validation_section_parses_all_four_keys() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [server.validation]
            enforce_input_schema = false
            default_max_length = 64
            additional_properties = true
            strict = true
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        let v = &cfg.server.validation;
        assert!(!v.enforce_input_schema);
        assert_eq!(v.default_max_length, 64);
        assert!(v.additional_properties);
        assert!(v.strict);
    }

    /// Each absent key yields its documented default, and an absent SECTION yields
    /// the same — enforcement is on unless an operator opts out.
    #[test]
    fn validation_section_absent_keys_yield_documented_defaults() {
        let partial = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [server.validation]
            default_max_length = 10
        "#;
        let cfg = ServerConfig::from_toml(partial).expect("parse");
        let v = &cfg.server.validation;
        assert!(v.enforce_input_schema, "default is ON");
        assert_eq!(v.default_max_length, 10);
        assert!(!v.additional_properties, "default is a closed envelope");
        assert!(!v.strict, "default must not refuse to boot");

        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        assert_eq!(cfg.server.validation, ValidationSection::default());
        assert!(cfg.server.validation.enforce_input_schema);
        assert_eq!(cfg.server.validation.default_max_length, 256);
    }

    /// An unknown key under `[server.validation]` is refused at PARSE time — the
    /// section carries `deny_unknown_fields`, so a typo'd opt-out cannot be
    /// silently ignored.
    #[test]
    fn validation_section_rejects_an_unknown_key() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [server.validation]
            enforce_input_schemas = false
        "#;
        let err = ServerConfig::from_toml(toml)
            .expect_err("a typo'd opt-out key must not be silently ignored");
        assert!(matches!(err, ToolkitError::Parse(_)), "got {err:?}");
    }

    /// `ToolDecl::param_position` agrees with `tools.rs::build_operation`'s PATH
    /// split for every parameter of a single-call tool, and applies the
    /// method-aware query/body rule for the rest.
    #[test]
    fn param_position_agrees_with_build_operation_on_path_parameters() {
        let get_tool = ToolDecl {
            name: "line_status".to_string(),
            path: Some("/lines/{line_id}/status".to_string()),
            method: Some("GET".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "line_id".to_string(),
                    ..Default::default()
                },
                ParamDecl {
                    name: "detail".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        // The PATH set is the shared helper both functions read.
        let path_names: Vec<&str> =
            path_placeholder_names(get_tool.path.as_deref().expect("path")).collect();
        assert_eq!(path_names, vec!["line_id"]);
        for p in &get_tool.parameters {
            let expected = if path_names.contains(&p.name.as_str()) {
                ParamPosition::Path
            } else {
                ParamPosition::Query
            };
            assert_eq!(
                get_tool.param_position(&p.name),
                expected,
                "position for {} must agree with the path split",
                p.name
            );
        }

        // Deliberate divergence: a mutating method's non-path parameter is BODY
        // here while `build_operation` still marks it `ParameterLocation::Query`.
        let post_tool = ToolDecl {
            name: "add_comment".to_string(),
            path: Some("/issues/{id}/comments".to_string()),
            method: Some("POST".to_string()),
            ..Default::default()
        };
        assert_eq!(post_tool.param_position("id"), ParamPosition::Path);
        assert_eq!(post_tool.param_position("body_text"), ParamPosition::Body);
    }

    /// `{}` is not a placeholder, and a brace pair inside a larger segment is not
    /// one either — the helper matches whole `/`-delimited segments only.
    #[test]
    fn path_placeholder_names_matches_whole_segments_only() {
        let names: Vec<&str> = path_placeholder_names("/a/{}/b/{id}/c/pre{mid}post").collect();
        assert_eq!(names, vec!["id"]);
    }

    /// SC-3 edge (empty): a config with zero tools and no active opt-out lints
    /// clean.
    #[test]
    fn lint_returns_empty_vec_for_a_config_with_zero_tools() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        assert_eq!(cfg.lint(), Vec::new());
    }

    /// Build a config with one tool whose declarations are supplied by the caller.
    fn cfg_with_one_tool(tool: ToolDecl, validation: ValidationSection) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                validation,
                ..Default::default()
            },
            tools: vec![tool],
            ..Default::default()
        }
    }

    /// A SQL tool's string parameter with no `max_length` is BODY position and is
    /// surfaced by `lint()` — not refused (D-05 / D-07).
    #[test]
    fn lint_reports_one_uncapped_string_finding_per_body_parameter() {
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "search_tracks".to_string(),
                sql: Some("SELECT 1".to_string()),
                parameters: vec![ParamDecl {
                    name: "q".to_string(),
                    param_type: Some("string".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        let findings = cfg.lint();
        assert_eq!(findings.len(), 1, "got {findings:?}");
        assert_eq!(findings[0].rule, UNCAPPED_STRING);
        assert_eq!(findings[0].tool, "search_tracks");
        assert_eq!(findings[0].param, "q");
        // The same config must still BOOT — a non-strict finding never refuses.
        cfg.validate()
            .expect("a lint finding must not fail validate");
    }

    /// SC-3 ordering: findings follow `[[tools.parameters]]` declaration order, not
    /// alphabetical order.
    #[test]
    fn lint_returns_findings_in_declaration_order() {
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "note".to_string(),
                sql: Some("SELECT 1".to_string()),
                parameters: vec![
                    ParamDecl {
                        name: "zebra".to_string(),
                        param_type: Some("string".to_string()),
                        ..Default::default()
                    },
                    ParamDecl {
                        name: "alpha".to_string(),
                        param_type: Some("string".to_string()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        let findings = cfg.lint();
        assert_eq!(findings.len(), 2, "got {findings:?}");
        assert_eq!(findings[0].param, "zebra");
        assert_eq!(findings[1].param, "alpha");
    }

    /// D3 ordering (mixed): for a tool carrying BOTH an uncapped body string and an
    /// uncapped path string, only the body one is a finding — the path one is
    /// covered by the default cap — and the finding order follows declaration order.
    #[test]
    fn lint_skips_a_path_parameter_covered_by_the_default_cap() {
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "add_comment".to_string(),
                path: Some("/issues/{id}/comments".to_string()),
                method: Some("POST".to_string()),
                parameters: vec![
                    ParamDecl {
                        name: "body_text".to_string(),
                        param_type: Some("string".to_string()),
                        ..Default::default()
                    },
                    ParamDecl {
                        name: "id".to_string(),
                        param_type: Some("string".to_string()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        let findings = cfg.lint();
        assert_eq!(findings.len(), 1, "got {findings:?}");
        assert_eq!(findings[0].param, "body_text");
        assert_eq!(findings[0].rule, UNCAPPED_STRING);
    }

    /// SC-7 shape mismatch: a path-position parameter declaring a `max_length`
    /// ABOVE the always-on placeholder floor is surfaced, because it publishes a
    /// limit the floor will not honour.
    #[cfg(feature = "input-validation")]
    #[test]
    fn lint_reports_a_declared_max_length_above_the_placeholder_floor() {
        let floor = pmcp::server::schema_validation::PLACEHOLDER_MAX_LENGTH as u64;
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "line_status".to_string(),
                path: Some("/lines/{line_id}/status".to_string()),
                method: Some("GET".to_string()),
                parameters: vec![ParamDecl {
                    name: "line_id".to_string(),
                    param_type: Some("string".to_string()),
                    max_length: Some(floor + 1),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        let findings = cfg.lint();
        assert_eq!(findings.len(), 1, "got {findings:?}");
        assert_eq!(findings[0].rule, DECLARED_MAX_LENGTH_ABOVE_PLACEHOLDER_CAP);
        assert_eq!(findings[0].param, "line_id");
        assert!(
            findings[0].detail.contains(&floor.to_string()),
            "the finding must name the effective limit: {}",
            findings[0].detail
        );

        // Exactly AT the floor is fine — the adjacency case.
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "line_status".to_string(),
                path: Some("/lines/{line_id}/status".to_string()),
                method: Some("GET".to_string()),
                parameters: vec![ParamDecl {
                    name: "line_id".to_string(),
                    param_type: Some("string".to_string()),
                    max_length: Some(floor),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        assert_eq!(cfg.lint(), Vec::new());
    }

    /// Every ACTIVE opt-out is reported, so a switched-off enforcement can never
    /// read as switched on.
    #[test]
    fn lint_reports_every_active_opt_out() {
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "ping".to_string(),
                sql: Some("SELECT 1".to_string()),
                ..Default::default()
            },
            ValidationSection {
                enforce_input_schema: false,
                default_max_length: 0,
                additional_properties: true,
                strict: false,
            },
        );
        let rules: Vec<&str> = cfg.lint().iter().map(|w| w.rule).collect();
        assert_eq!(
            rules,
            vec![
                OPT_OUT_ENFORCE_INPUT_SCHEMA,
                OPT_OUT_DEFAULT_MAX_LENGTH_ZERO,
                OPT_OUT_ADDITIONAL_PROPERTIES,
            ]
        );
        // A server-level finding names no tool or parameter.
        for w in cfg.lint() {
            assert!(w.tool.is_empty(), "{w:?}");
            assert!(w.param.is_empty(), "{w:?}");
            assert!(!w.to_string().is_empty(), "Display must render");
        }
    }

    /// D-07 strict mode: the SAME config that returns `Ok(())` with one lint
    /// finding under the default becomes a hard `validate()` failure under
    /// `strict = true`.
    #[test]
    fn validate_rejects_uncapped_string_param_in_strict_mode() {
        let toml_body = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "search_tracks"
            sql = "SELECT 1"

            [[tools.parameters]]
            name = "q"
            type = "string"
        "#;
        let lenient = ServerConfig::from_toml(toml_body).expect("parse");
        lenient
            .validate()
            .expect("non-strict must never refuse to boot over an uncapped body string");
        assert_eq!(lenient.lint().len(), 1);

        let strict_toml = format!("{toml_body}\n[server.validation]\nstrict = true\n");
        let strict = ServerConfig::from_toml(&strict_toml).expect("parse");
        match strict.validate() {
            Err(ConfigValidationError::UncappedStringParam {
                ref tool,
                ref param,
            }) => {
                assert_eq!(tool, "search_tracks");
                assert_eq!(param, "q");
            },
            other => panic!("expected UncappedStringParam, got {other:?}"),
        }
    }

    /// `validation_report()` carries the effective policy and a per-tool rule list
    /// for the once-at-startup log.
    #[test]
    fn validation_report_carries_the_effective_policy_and_per_tool_rules() {
        let cfg = cfg_with_one_tool(
            ToolDecl {
                name: "line_status".to_string(),
                path: Some("/lines/{line_id}/status".to_string()),
                method: Some("GET".to_string()),
                parameters: vec![ParamDecl {
                    name: "line_id".to_string(),
                    param_type: Some("string".to_string()),
                    required: true,
                    pattern: Some("^[0-9a-z-]+$".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ValidationSection::default(),
        );
        let report = cfg.validation_report();
        assert!(report.enforce_input_schema);
        assert_eq!(report.default_max_length, 256);
        assert!(report.opt_outs.is_empty(), "nothing is opted out");
        assert_eq!(report.tools.len(), 1);
        assert_eq!(report.tools[0].tool, "line_status");
        let rule = &report.tools[0].rules[0];
        assert!(rule.contains("line_id"), "{rule}");
        assert!(rule.contains("Path"), "{rule}");
        assert!(rule.contains("pattern"), "{rule}");
        assert!(rule.contains("maxLength=256 (default)"), "{rule}");
    }

    // -- Phase 128 D4(b) step 3b: the curated template parser's limit, enforced at
    //    CONFIG time rather than failing obscurely at call time. ---------------

    /// A single-call `GET` tool on `path`, no parameters.
    fn single_call_on(path: &str) -> ServerConfig {
        cfg_with_one_tool(
            ToolDecl {
                name: "t".to_string(),
                description: Some("t".to_string()),
                path: Some(path.to_string()),
                method: Some("GET".to_string()),
                ..Default::default()
            },
            ValidationSection::default(),
        )
    }

    fn assert_malformed_segment(path: &str) {
        let err = single_call_on(path)
            .validate()
            .expect_err("an unsupported path-template segment must be refused at config time");
        match err {
            ConfigValidationError::MalformedPathTemplateSegment { tool, segment } => {
                assert_eq!(tool, "t");
                assert!(!segment.is_empty(), "the finding must name the segment");
            },
            other => panic!("expected MalformedPathTemplateSegment, got {other:?}"),
        }
    }

    /// `/search/{a}{b}` parses to the SINGLE name `a}{b`, which no `ParamDecl` can
    /// match — refused at config time instead of sending literal braces upstream.
    #[test]
    fn validate_rejects_a_path_template_segment_with_two_brace_pairs() {
        assert_malformed_segment("/search/{a}{b}");
    }

    /// `/prefix-{id}` is not recognized as carrying a placeholder at all.
    #[test]
    fn validate_rejects_a_path_template_segment_with_text_adjacent_to_a_brace_pair() {
        assert_malformed_segment("/prefix-{id}");
    }

    /// `{}` is a brace pair with no name — not a placeholder.
    #[test]
    fn validate_rejects_an_empty_path_template_placeholder() {
        assert_malformed_segment("/a/{}/b");
    }

    /// An unbalanced brace is the same author error seen from the other side.
    #[test]
    fn validate_rejects_an_unbalanced_path_template_brace() {
        assert_malformed_segment("/a/{id");
    }

    /// ACCEPT control: whole-segment placeholders are the supported shape. Without
    /// this row the four refusals above are satisfiable by refusing every template.
    #[test]
    fn validate_accepts_whole_segment_path_template_placeholders() {
        single_call_on("/content/{version}/CUI/{cui}")
            .validate()
            .expect("whole-segment placeholders are the supported shape");
    }

    /// ACCEPT control, and the CURATED half of the inherited `?` narrowing: an
    /// author-written query string in a `[[tools]]` `path` is configuration, not
    /// caller data, and must keep working.
    #[test]
    fn validate_accepts_a_path_template_carrying_an_author_written_query_string() {
        single_call_on("/content/{version}/CUI?string=x")
            .validate()
            .expect("an author-written query string in a curated path must be accepted");
    }

    /// A SQL tool has no `path`, so the template rule cannot reach it.
    #[test]
    fn validate_ignores_the_template_rule_for_a_tool_with_no_path() {
        cfg_with_one_tool(
            ToolDecl {
                name: "t".to_string(),
                sql: Some("SELECT 1".to_string()),
                ..Default::default()
            },
            ValidationSection::default(),
        )
        .validate()
        .expect("a SQL tool carries no path template");
    }

    proptest! {
        /// TEST-02: any valid `ServerConfig` round-trips through TOML.
        ///
        /// Builds a `ServerConfig` from an arbitrary (but valid) `(name, version)`
        /// pair, serializes it, parses it back, and asserts equality on the
        /// load-bearing scalars.
        #[test]
        fn server_config_minimal_round_trips(
            name in "[a-zA-Z0-9_-]{1,32}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+",
        ) {
            let cfg = ServerConfig {
                server: ServerSection {
                    name: name.clone(),
                    version: version.clone(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let s = toml::to_string(&cfg).unwrap();
            let parsed = ServerConfig::from_toml(&s).unwrap();
            prop_assert_eq!(parsed.server.name, name);
            prop_assert_eq!(parsed.server.version, version);
        }
    }
}

/// `ServerConfig::lint_against_spec` — the CONFIG-time half of the
/// template-spelling-drift guard (Phase 128, D4(b) / T-128-36a).
///
/// Four rows, and the shape matters: one row per DRIFT that must be reported, plus
/// three accept rows for the shapes that must NOT be, because a finding-producing
/// lint with no accept rows is indistinguishable from one that fires on everything.
#[cfg(all(test, feature = "http"))]
mod lint_against_spec_tests {
    use super::{ServerConfig, ToolDecl, CONFIGURED_TEMPLATE_NOT_IN_SPEC};
    use crate::http::OpenApiSchema;

    const SPEC: &str = r#"{
      "openapi": "3.0.0",
      "info": { "title": "t", "version": "1" },
      "paths": {
        "/content/{version}/CUI": {
          "get": {
            "operationId": "getCui",
            "parameters": [
              { "name": "version", "in": "path", "required": true,
                "schema": { "type": "string", "pattern": "^[a-z]+$" } }
            ],
            "responses": { "200": { "description": "ok" } }
          }
        }
      }
    }"#;

    fn spec() -> OpenApiSchema {
        OpenApiSchema::parse(SPEC).expect("the fixture spec parses")
    }

    fn cfg_with(tools: Vec<ToolDecl>) -> ServerConfig {
        ServerConfig {
            server: super::ServerSection {
                name: "t".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools,
            ..Default::default()
        }
    }

    fn http_tool(name: &str, method: &str, path: &str) -> ToolDecl {
        ToolDecl {
            name: name.to_string(),
            method: Some(method.to_string()),
            path: Some(path.to_string()),
            ..Default::default()
        }
    }

    /// The drift the guard exists for: a placeholder spelled differently from the
    /// spec's own reaches the same endpoint with the declaration dropped.
    #[test]
    fn lint_against_spec_reports_a_template_the_spec_does_not_declare() {
        let cfg = cfg_with(vec![http_tool("get_cui", "GET", "/content/{alias}/CUI")]);
        let findings = cfg.lint_against_spec(&spec());
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, CONFIGURED_TEMPLATE_NOT_IN_SPEC);
        assert_eq!(findings[0].tool, "get_cui");
        assert!(
            findings[0].detail.contains("floor"),
            "the finding must say what a miss RETAINS, not only what it loses: {}",
            findings[0].detail
        );
    }

    /// A method the spec does not declare on a path it does is the same class of
    /// drift, because operations are indexed by `(path, METHOD)`.
    #[test]
    fn lint_against_spec_reports_a_method_the_spec_does_not_declare() {
        let cfg = cfg_with(vec![http_tool(
            "del_cui",
            "DELETE",
            "/content/{version}/CUI",
        )]);
        let rules: Vec<&str> = cfg
            .lint_against_spec(&spec())
            .iter()
            .map(|w| w.rule)
            .collect();
        assert_eq!(rules, vec![CONFIGURED_TEMPLATE_NOT_IN_SPEC]);
    }

    /// ACCEPT — an exact match, in either method case, is no finding.
    #[test]
    fn lint_against_spec_accepts_an_exactly_declared_template() {
        let cfg = cfg_with(vec![
            http_tool("a", "GET", "/content/{version}/CUI"),
            http_tool("b", "get", "/content/{version}/CUI"),
        ]);
        assert_eq!(cfg.lint_against_spec(&spec()), Vec::new());
    }

    /// ACCEPT — an author-written query string on the configured `path` is legal
    /// curated authoring (plan 06's `?` narrowing) and an OpenAPI template never
    /// carries one, so it is stripped before the lookup rather than reported.
    #[test]
    fn lint_against_spec_accepts_an_author_written_query_string() {
        let cfg = cfg_with(vec![http_tool(
            "a",
            "GET",
            "/content/{version}/CUI?string=x",
        )]);
        assert_eq!(cfg.lint_against_spec(&spec()), Vec::new());
    }

    /// ACCEPT — a tool that addresses no single spec operation is skipped, not
    /// reported. A SQL tool has no `(method, path)` to look up.
    #[test]
    fn lint_against_spec_skips_a_tool_with_no_method_path_pair() {
        let cfg = cfg_with(vec![ToolDecl {
            name: "q".to_string(),
            sql: Some("SELECT 1".to_string()),
            ..Default::default()
        }]);
        assert_eq!(cfg.lint_against_spec(&spec()), Vec::new());
    }
}
