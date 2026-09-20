// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as a public SDK crate for Phase 83.

//! Toolkit error type and crate-level `Result` alias.
//!
//! [`ToolkitError`] is `#[non_exhaustive]`: downstream crates must match with a
//! catch-all arm so the toolkit can add variants without a breaking change.
//! Phase 83 Plan 04 extends this enum with a `Validation` variant wrapping a
//! [`ConfigValidationError`] (per review R8) which catches missing-required-value
//! bugs the `Default` impls on sub-sections would otherwise silently hide.

/// Crate-level result alias used by every public API in `pmcp-server-toolkit`.
pub type Result<T> = std::result::Result<T, ToolkitError>;

/// Errors surfaced by the `pmcp-server-toolkit` runtime.
///
/// The enum is `#[non_exhaustive]` — match callers must include a wildcard arm.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::ToolkitError;
/// use std::error::Error;
///
/// // ToolkitError is a real `std::error::Error`, with a usable `Display` impl.
/// let err: ToolkitError = ToolkitError::MissingField("database.dsn".into());
/// assert_eq!(err.to_string(), "missing required config field: database.dsn");
/// // Implements `std::error::Error`, so it composes with `?` and `Box<dyn Error>`.
/// let boxed: Box<dyn Error + Send + Sync> = Box::new(err);
/// assert!(boxed.source().is_none());
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolkitError {
    /// TOML parse failure while loading a `ServerConfig`.
    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),

    /// A required config field was absent during tool synthesis.
    #[error("missing required config field: {0}")]
    MissingField(String),

    /// `[[tools]]` synthesis failed (covers Phase 83 TKIT-07 failure modes).
    #[error("tool synthesis failed: {0}")]
    Synth(String),

    /// Code-mode wiring failed (covers Phase 83 TKIT-09 failure modes).
    #[error("code-mode wiring failed: {0}")]
    CodeMode(String),

    /// Filesystem failure while reading a config or fixture.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Secret resolution failed (env var missing, AWS API error, etc.).
    ///
    /// Carries the secret name and a descriptive cause string; the underlying
    /// raw value is NEVER carried in this variant — only the lookup-key
    /// metadata and the error context. This preserves the `SecretValue`
    /// negative-trait invariants at the error path (review R5 + T-83-02-02).
    #[error("secret '{name}' not resolvable: {cause}")]
    Secret {
        /// The secret name that could not be resolved.
        name: String,
        /// Human-readable cause (provider name + underlying error).
        cause: String,
    },

    /// Semantic validation of a parsed [`crate::config::ServerConfig`] failed.
    ///
    /// Wraps a [`ConfigValidationError`] surfaced by
    /// [`crate::config::ServerConfig::validate`] /
    /// [`crate::config::ServerConfig::from_toml_strict_validated`]. Per Phase 83
    /// review R8 this catches the empty-required-value trap that the
    /// `Default` impls on sub-sections would otherwise hide behind silent
    /// successes (e.g. `server.name = ""` if the `[server]` header is typo'd).
    #[error("config validation failed: {0}")]
    Validation(#[from] ConfigValidationError),

    /// `[backend].base_url` holds a `${VAR}` / `env:VAR` reference that could
    /// not be resolved: the named environment variable is unset, or is set to
    /// an empty / whitespace-only value.
    ///
    /// Filed HERE and NOT under [`ConfigValidationError`] deliberately (Phase
    /// 120 Plan 04, cross-AI review LOW). `ConfigValidationError` is the
    /// semantic validation surfaced by
    /// [`crate::config::ServerConfig::validate`] — i.e. PARSE time. This lookup
    /// happens at DISPATCH time, long after `validate()` returned `Ok` (the
    /// literal `"${TFL_BASE_URL}"` is non-empty, so the emptiness rule passes).
    /// Filing a runtime lookup failure under parse-time validation would make
    /// that enum's own documentation false and would let a caller matching
    /// `ToolkitError::Validation(..)` believe the config was malformed.
    ///
    /// # Security (T-120-17)
    ///
    /// The message names the FIELD and the environment-variable NAME only. It
    /// MUST NOT echo a resolved URL, the config's contents, or any credential
    /// substring.
    #[error(
        "[backend].base_url references environment variable '{var}', which is \
         unset or empty (set it to the REST API root URL)"
    )]
    UnresolvedBaseUrlRef {
        /// The environment-variable name the `base_url` reference points at.
        /// Never the resolved value.
        var: String,
    },

    /// A governed-Excel workbook bundle failed to load + integrity-verify at
    /// boot (Phase 92, WBSV-08 fail-closed). Wraps a
    /// [`pmcp_workbook_runtime::BundleLoadError`] — a source read failure, a
    /// malformed/truncated artifact, or an integrity-hash mismatch (a tampered
    /// or swapped bundle). Feature-gated on `workbook` so the no-`workbook`
    /// build never names the runtime type.
    #[cfg(feature = "workbook")]
    #[error("workbook bundle load failed: {0}")]
    Workbook(#[from] pmcp_workbook_runtime::BundleLoadError),
}

/// Semantic-validation errors surfaced by
/// [`crate::config::ServerConfig::validate`].
///
/// Per Phase 83 review R8 — the `Default` impls on `ServerConfig` and its
/// sub-sections deliberately allow `from_toml` to succeed even when required
/// fields are missing (so partial configs can be merged programmatically). The
/// [`crate::config::ServerConfig::validate`] entry-point catches these gaps at
/// parse time and surfaces them as a typed enum variant per rule.
///
/// The enum is `#[non_exhaustive]` — match callers must include a wildcard arm
/// so additional rules can be added without a breaking change.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::ConfigValidationError;
///
/// // Each variant has a precise `Display` describing the rule violated.
/// let err = ConfigValidationError::EmptyServerName;
/// assert_eq!(err.to_string(), "server.name must be non-empty");
/// let err = ConfigValidationError::EmptyToolName(3);
/// assert_eq!(err.to_string(), "[[tools]] entry at index 3 has empty name");
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    /// `[server] name` is missing or whitespace-only.
    #[error("server.name must be non-empty")]
    EmptyServerName,
    /// `[server] version` is missing or whitespace-only.
    #[error("server.version must be non-empty")]
    EmptyServerVersion,
    /// `[[tools]]` entry at `index` has an empty / whitespace-only `name`.
    #[error("[[tools]] entry at index {0} has empty name")]
    EmptyToolName(usize),
    /// `[[database.tables]]` entry at `index` has an empty / whitespace-only `name`.
    #[error("[[database.tables]] entry at index {0} has empty name")]
    EmptyTableName(usize),
    /// Per Phase 83 Plan 06 review R9: `[code_mode].token_secret` was given as
    /// an inline literal (e.g. `token_secret = "raw-string"`) instead of the
    /// `env:VAR_NAME` reference form, and the dev-only escape hatch
    /// `allow_inline_token_secret_for_dev` was not set. Inline literals in
    /// committed configs leak HMAC signing keys; the toolkit defaults to
    /// rejecting them.
    #[error(
        "[code_mode].token_secret is an inline literal; use 'env:VAR_NAME' \
         or set allow_inline_token_secret_for_dev=true (NEVER in production)"
    )]
    InlineSecretRejected,
    /// Per Phase 90 Plan 02 (D-01, T-90-02-04): a `[[tools]]` entry declares
    /// more than one mutually-exclusive tool kind. A tool is EITHER a SQL tool
    /// (`sql`), a single-call HTTP tool (`path`/`method`), OR a script tool
    /// (`script`) — never a mixture. The ambiguity is rejected rather than
    /// resolved by a silent precedence rule. The `usize` is the entry index.
    #[error(
        "[[tools]] entry at index {0} declares ambiguous tool kind: set exactly \
         one of `sql`, `path`/`method`, or `script` (not a mixture)"
    )]
    AmbiguousToolKind(usize),
    /// Per Phase 90 gap-closure (GAP 3 / WR-02): a `[backend]` block is present
    /// but its `base_url` is empty / whitespace-only (or the `base_url` key was
    /// omitted, defaulting to `""` via `#[serde(default)]`). Without this
    /// parse-time check a typo'd or missing `base_url` would validate cleanly
    /// and then surface late as an opaque `DispatchError::Connector("invalid
    /// base URL")` at the first backend request. Rejecting it here turns that
    /// late opaque failure into an actionable, field-naming error.
    #[error(
        "[backend].base_url must be non-empty (set the REST API root URL, \
         e.g. \"https://api.example.com\")"
    )]
    EmptyBackendBaseUrl,
    /// `[backend].base_url` is REFERENCE-shaped but does not name exactly one
    /// environment variable — the empty `${}` form, or a multi-placeholder
    /// composition like `"${SCHEME}://${HOST}"`. The grammar
    /// ([`crate::env_ref::parse_env_ref`]) resolves one whole-value `${VAR}` /
    /// `env:VAR` reference; it does not interpolate inside a larger string, so
    /// no environment could ever satisfy such a value. Without this check the
    /// config loads cleanly and every boot fails with an
    /// `UnresolvedBaseUrlRef` naming an empty variable.
    #[error(
        "[backend].base_url is a malformed environment reference; a reference must be \
         exactly one `${{VAR}}` or `env:VAR` naming a single variable — inline \
         compositions like \"${{SCHEME}}://${{HOST}}\" cannot be resolved by any \
         environment, so compose the full URL in ONE variable instead"
    )]
    MalformedBackendBaseUrlRef,
    /// A `[backend.auth]` credential field is REFERENCE-shaped but does not name
    /// exactly one environment variable — the empty `${}` form, a
    /// multi-placeholder composition like `"${SCHEME}://${HOST}"`, or a
    /// non-portable name like `"${TFL-APP-KEY}"` (a `${...}` name must match
    /// `[A-Za-z0-9_]+`; `env:VAR` remains the escape hatch for exotic names).
    /// The `String` is the offending field path within `[backend.auth]` — e.g.
    /// `"token"`, `"password"`, `"query_params.app_key"`.
    ///
    /// This is the credential sibling of [`Self::MalformedBackendBaseUrlRef`],
    /// and it exists because the two paths resolve UNSET references differently
    /// on purpose: a credential resolves an unset variable to the empty string
    /// so an optional credential is OMITTED. A MALFORMED reference is not an
    /// unset variable — no environment can ever satisfy it — so applying the
    /// omission rule to it silently sent every backend request UNAUTHENTICATED,
    /// with no error and no log line. Refusing it at load time turns that into
    /// an actionable, field-naming failure before the server ever boots.
    ///
    /// The message names the FIELD only and deliberately never echoes the
    /// configured value: a malformed value is by definition not a resolvable
    /// reference, so it may well be a mistyped literal secret.
    #[error(
        "[backend.auth].{0} is a malformed environment reference; a reference must be \
         exactly one `${{VAR}}` (name matching [A-Za-z0-9_]+) or `env:VAR` naming a single \
         variable — no environment can satisfy this value, so the credential would be \
         silently omitted and every backend request sent unauthenticated"
    )]
    MalformedBackendAuthRef(String),
    /// Per Phase 120 Plan 04 (PKG-03): a `[[config_slots]]` entry at `index`
    /// has an empty / whitespace-only `key` or `name`. A slot declaration whose
    /// key names no config path — or whose name names no environment variable —
    /// claims coverage it cannot deliver, and the package side would compare
    /// against an empty string.
    ///
    /// The sibling "unrecognized `kind`" check is NOT here: `kind` is the
    /// closed [`crate::config::ConfigSlotKind`] enum, so serde rejects an
    /// unknown discriminator at PARSE time (naming the accepted set) before
    /// `validate()` is ever called.
    #[error("[[config_slots]] entry at index {0} has an empty key or name")]
    EmptyConfigSlotField(usize),
    /// A `[[config_slots]]` entry at `index` is `kind = "secret"` but carries a
    /// `tested_value`. Identity-bearing slots structurally carry no value — the
    /// `tested_value` field on a secret declaration is the one place a REAL
    /// credential could sit in a config that is served but never packed (the
    /// pack-time agreement gate only runs on packaging), so the rule is
    /// enforced at validation time rather than trusted as prose. The message
    /// deliberately does not echo the value.
    #[error(
        "[[config_slots]] entry at index {0} is kind = \"secret\" but carries a tested_value; \
         identity-bearing slots record no value — remove it (a credential must never sit in \
         the config file)"
    )]
    SecretSlotCarriesTestedValue(usize),
}
