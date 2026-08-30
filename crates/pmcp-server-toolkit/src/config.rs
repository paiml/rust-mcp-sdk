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

use crate::error::{ConfigValidationError, Result, ToolkitError};

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
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigValidationError`] variant identifying the
    /// first rule violated. Iteration order matches struct field order.
    pub fn validate(&self) -> std::result::Result<(), ConfigValidationError> {
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
}

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
}

/// Single `[[tools.parameters]]` entry.
///
/// The `default` and `enum` fields use [`toml::Value`] because they are
/// heterogeneous in the reference configs (a `default` may be an integer,
/// a string, or a boolean depending on the parameter type).
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
    #[serde(default)]
    pub minimum: Option<f64>,
    /// Inclusive maximum (integer / number parameters only).
    #[serde(default)]
    pub maximum: Option<f64>,
    /// Closed set of allowed values (any TOML scalar).
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<toml::Value>>,
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
