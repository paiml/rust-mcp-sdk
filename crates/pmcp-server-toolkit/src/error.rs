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
    /// Per Phase 128 SC-2: a `[[tools]]` entry's synthesized `inputSchema` does
    /// not compile as a Draft 2020-12 schema — in practice always a
    /// `[[tools.parameters]]` `pattern` that is not a valid regular expression.
    ///
    /// Caught at CONFIG time rather than at call time, because a single
    /// non-compiling `pattern` fails the whole document: the tool's validator
    /// never builds, so every call to it is refused (or, if the compile error were
    /// swallowed, every call passes unchecked). Neither outcome should first be
    /// discovered by a client.
    ///
    /// # Why quoting `detail` is safe here
    ///
    /// `detail` is the engine's own compile-error text, which quotes the offending
    /// SCHEMA — author-supplied config, never caller-supplied argument data. This
    /// error is raised by [`crate::config::ServerConfig::validate`], which runs at
    /// load time with no request in scope, and its audience is the config author,
    /// who needs the detail to fix the regex. The SC-7 no-echo rule governs the
    /// CLIENT-facing `tools/call` refusal path, where a non-compiling schema still
    /// yields a detail-free message.
    ///
    /// `position` is the compile error's JSON schema path — e.g.
    /// `/properties/region/pattern` — so it names the offending parameter directly.
    #[error(
        "[[tools]] '{tool}' has a declared parameter schema that does not compile at \
         {position}: {detail}"
    )]
    UncompilableParamSchema {
        /// The `[[tools]]` `name` whose parameter schema failed to compile.
        tool: String,
        /// JSON schema path of the offending declaration, e.g.
        /// `/properties/region/pattern`.
        position: String,
        /// The engine's compile-error text. Schema-derived, never caller data.
        detail: String,
    },
    /// Per Phase 128 SC-2: a `[[tools.parameters]]` `pattern` was declared as the
    /// empty string.
    ///
    /// An empty `pattern` is a valid regular expression that matches every input,
    /// so it buys no enforcement at all while reading — in a config review, in a
    /// diff — exactly like a rule. Refused as a likely author error rather than
    /// accepted as a no-op.
    #[error(
        "[[tools]] '{tool}' parameter '{param}' declares an empty pattern; an empty pattern \
         matches every value and enforces nothing — remove the key or write the rule"
    )]
    EmptyParamPattern {
        /// The `[[tools]]` `name` carrying the offending parameter.
        tool: String,
        /// The `[[tools.parameters]]` `name` whose `pattern` is empty.
        param: String,
    },
    /// Per Phase 128 D3: a `[[tools.parameters]]` `minimum` or `maximum` is
    /// non-finite (`NaN` / infinity) or has a magnitude EXCEEDING 2^53.
    ///
    /// # What this establishes, precisely
    ///
    /// [`crate::config::ParamDecl::minimum`] and
    /// [`crate::config::ParamDecl::maximum`] are `f64`. A TOML integer above 2^53
    /// has therefore ALREADY been rounded by the time this check runs, so the check
    /// cannot see that rounding happened and does NOT promise to catch a bound
    /// sitting one unit past the boundary. It catches the non-finite and the wildly
    /// out-of-range cases, which is where a silently-mangled bound is most likely
    /// to be load-bearing.
    ///
    /// The honest contract, stated on the field itself as well: `minimum` /
    /// `maximum` are not a safe way to bound a 64-bit integer ID. Use a `pattern`
    /// over the string form for that.
    #[error(
        "[[tools]] '{tool}' parameter '{param}' declares a minimum/maximum that is \
         non-finite or exceeds 2^53; bounds are stored as f64, so such a value cannot be \
         represented exactly — bound a large integer ID with a `pattern` instead"
    )]
    NonFiniteParamBound {
        /// The `[[tools]]` `name` carrying the offending parameter.
        tool: String,
        /// The `[[tools.parameters]]` `name` whose bound cannot be represented.
        param: String,
    },
    /// Per Phase 128 D3 / D-07: a string parameter declares no `max_length` AND no
    /// default cap reaches it, and `[server.validation]` `strict = true` promotes
    /// that lint finding into a hard failure.
    ///
    /// Deliberately NOT described as "body-position". That is the usual case but not
    /// the only one: `[server.validation] default_max_length = 0` is a supported
    /// opt-out that switches the D3 cap off for EVERY position, so a PATH or QUERY
    /// parameter reaches this variant too. Naming a position the variant does not
    /// actually pin sent the operator looking at the wrong parameter. The position
    /// and the reason are carried by the paired
    /// [`crate::config::ServerConfig::lint`] finding, which has both in scope.
    ///
    /// Only reachable under `strict`. With `strict = false` (the default) the same
    /// config validates cleanly and the finding is reported by `lint` instead — a
    /// running server must never refuse to boot over an uncapped free-text field,
    /// which is the whole reason the lint channel exists separately from `validate`.
    #[error(
        "[[tools]] '{tool}' parameter '{param}' is an uncapped string — no declared \
         max_length and no default cap reaches it — and [server.validation] strict = true; \
         declare a max_length, or clear the strict flag (the paired lint finding names why \
         no cap reached it)"
    )]
    UncappedStringParam {
        /// The `[[tools]]` `name` carrying the uncapped parameter.
        tool: String,
        /// The `[[tools.parameters]]` `name` with no `max_length`.
        param: String,
    },
    /// Per Phase 128 D4(b): a single-call `[[tools]]` `path` carries a
    /// `/`-delimited segment that is not a supported placeholder shape.
    ///
    /// # The supported shape, and why anything else is an author error
    ///
    /// On the curated single-call surface a placeholder is a WHOLE segment: the
    /// `path_placeholder_names` helper recognizes `{name}` spanning an
    /// entire `/`-delimited segment and nothing else. A segment that contains a
    /// brace but is not exactly `{name}` therefore takes one of two bad routes at
    /// call time, neither of which is what the author meant:
    ///
    /// - `/search/{a}{b}` parses to the single parameter name `a}{b`, which no
    ///   `[[tools.parameters]]` entry can match, so nothing is substituted;
    /// - `/prefix-{id}` is not recognized as carrying a placeholder at all, so the
    ///   literal text `{id}` is what would travel toward the backend.
    ///
    /// Both used to pass config validation and fail obscurely later. Refusing here
    /// turns a silently-wrong request into a startup error naming the segment. The
    /// segment text is author-written configuration, so echoing it is safe and is
    /// what makes the error actionable — it carries no caller data.
    #[error(
        "[[tools]] '{tool}' path template segment '{segment}' is not a supported \
         placeholder shape: a segment either contains no braces at all, or is \
         exactly one non-empty '{{name}}' spanning the whole segment"
    )]
    MalformedPathTemplateSegment {
        /// The `[[tools]]` `name` whose `path` carries the offending segment.
        tool: String,
        /// The offending `/`-delimited segment, verbatim (author-written config).
        segment: String,
    },
    /// A `[code_mode]` key that only means something on the other kind of
    /// server: a SQL key on an OpenAPI server (one with a `[backend]`), or an
    /// operation-class key on a server with no `[backend]`. It would be
    /// silently ignored, so the server refuses to boot instead.
    #[error("[code_mode] key `{key}` does not apply to {server_kind} server; {hint}")]
    CodeModeKeyWrongBackend {
        /// The config key.
        key: &'static str,
        /// `"an OpenAPI"` or `"a SQL"`.
        server_kind: &'static str,
        /// What to use instead.
        hint: &'static str,
    },
    /// A class mode that needs a list that is empty: `allowlist` with no
    /// `allowed_operations`, or `blocklist` with no `blocked_operations`.
    #[error("[code_mode] `{class}_mode = \"{mode}\"` needs a non-empty `{list}`")]
    ClassModeNeedsList {
        /// `read`, `write`, `delete` or `admin`.
        class: &'static str,
        /// The mode.
        mode: &'static str,
        /// The list key it needs.
        list: &'static str,
    },
    /// A `[[code_mode.operations]]` entry with an empty `id` or `path`
    /// (index into the list).
    #[error("[[code_mode.operations]] entry at index {0} has an empty id or path")]
    EmptyOperationField(usize),
    /// Two `[[code_mode.operations]]` entries share an `id`.
    #[error("[[code_mode.operations]] id '{0}' is declared more than once")]
    DuplicateOperationId(String),
    /// A `[code_mode] auto_approve_levels` entry that is not `low`, `medium`,
    /// `high` or `critical`. It used to be skipped, which made a typo read as
    /// "nothing auto-approved".
    #[error(
        "[code_mode] auto_approve_levels entry '{0}' is not one of low, medium, high, critical"
    )]
    UnknownAutoApproveLevel(String),
    /// Operation-class keys were set, but this build cannot enforce them (the
    /// `openapi-code-mode` feature is off).
    #[error(
        "[code_mode] key `{0}` needs the `openapi-code-mode` feature, which this build \
         does not have, so it could not be enforced"
    )]
    ClassKeysUnenforceable(&'static str),
    /// A curated `[[tools]]` entry calls an operation the `[code_mode]` class
    /// policy refuses. The tool could never succeed, and a reader of the policy
    /// would assume it is blocked, so the contradiction fails the boot.
    #[error("[[tools]] '{tool}' is refused by the [code_mode] class policy: {reason}")]
    CuratedToolRefusedByPolicy {
        /// The `[[tools]]` `name`.
        tool: String,
        /// The policy's violation message (names the class and mode).
        reason: String,
    },
}

/// One non-fatal finding from [`crate::config::ServerConfig::lint`]
/// (Phase 128, D-07).
///
/// # Why this exists instead of a `validate()` variant
///
/// [`ConfigValidationError`] is first-error-wins `Result<(), _>` with no warning
/// channel, so D-07's "warns" is unexpressible in that signature. A running server
/// must NOT refuse to boot because a free-text body parameter has no `max_length`
/// (D-05) — but the author still has to be told, and the startup log is what makes
/// a later regression traceable. Hence a separate additive `-> Vec<ConfigWarning>`
/// channel that leaves `validate`'s existing behaviour untouched.
///
/// `[server.validation] strict = true` is what promotes a finding into a
/// [`ConfigValidationError::UncappedStringParam`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    /// The `[[tools]]` `name` the finding concerns; `None` for a server-level
    /// finding — an active `[server.validation]` opt-out belongs to the server, not
    /// to a tool.
    pub tool: Option<String>,
    /// The `[[tools.parameters]]` `name` the finding concerns; `None` when the
    /// finding is not about one parameter.
    ///
    /// A server-level finding has neither `tool` nor `param`. A TOOL-level finding,
    /// one that concerns the `[[tools]]` entry as a whole rather than any one
    /// parameter (`lint_against_spec`'s `configured-template-not-in-spec` is the
    /// live case), has a `tool` and no `param`. A parameter-level finding has both.
    /// `None` is the only "absent" value: an empty string is a name, not a scope.
    pub param: Option<String>,
    /// Stable machine-readable rule identifier, e.g. `"uncapped-string"`.
    ///
    /// A `&'static str` rather than an enum so plan 07's CLI and plan 09's startup
    /// log can group and filter on it without either taking a dependency on a
    /// closed set that every new rule would widen.
    pub rule: &'static str,
    /// Human-readable explanation, including the remedy.
    ///
    /// Author-facing, like [`ConfigValidationError`]'s messages and unlike a
    /// client-facing refusal: it may name config keys and declared limits. It never
    /// contains caller data — `lint` runs at load time with no request in scope.
    pub detail: String,
}

impl std::fmt::Display for ConfigWarning {
    /// Renders the three scopes the two optional fields encode: server-level,
    /// tool-level (no parameter) and parameter-level. Branching on `tool` alone
    /// would render a tool-level finding as `... 'get_cui' parameter '': ...`,
    /// which `lint_against_spec` can produce and `pmcp-openapi-server` prints
    /// verbatim to the deploy log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.tool.as_deref(), self.param.as_deref()) {
            // Server-level: an active `[server.validation]` opt-out belongs to the
            // server, not to any tool.
            (None, _) => write!(f, "[{}] {}", self.rule, self.detail),
            // Tool-level: the finding concerns the `[[tools]]` entry as a whole.
            (Some(tool), None) => {
                write!(f, "[{}] [[tools]] '{tool}': {}", self.rule, self.detail)
            },
            // Parameter-level.
            (Some(tool), Some(param)) => write!(
                f,
                "[{}] [[tools]] '{tool}' parameter '{param}': {}",
                self.rule, self.detail
            ),
        }
    }
}

#[cfg(test)]
mod config_warning_display {
    use super::ConfigWarning;

    fn warning(tool: &str, param: &str) -> ConfigWarning {
        // The test helper keeps "" as shorthand for "no such scope".
        let scope = |s: &str| (!s.is_empty()).then(|| s.to_string());
        ConfigWarning {
            tool: scope(tool),
            param: scope(param),
            rule: "a-rule",
            detail: "a detail".to_string(),
        }
    }

    /// The two sentinel fields encode THREE scopes. Asserted because nothing else
    /// in the tree renders a `ConfigWarning`: `lint()`'s own tests check `.rule`
    /// and `.detail` only, and the sole production consumer is a
    /// `tracing::warn!("{finding}")` in `pmcp-openapi-server`'s deploy log — so a
    /// regression in this `match` is invisible to every other test.
    #[test]
    fn renders_server_tool_and_parameter_scopes_distinctly() {
        assert_eq!(warning("", "").to_string(), "[a-rule] a detail");
        assert_eq!(
            warning("get_cui", "").to_string(),
            "[a-rule] [[tools]] 'get_cui': a detail"
        );
        assert_eq!(
            warning("get_cui", "version").to_string(),
            "[a-rule] [[tools]] 'get_cui' parameter 'version': a detail"
        );
    }

    /// The specific regression the three-arm form exists to prevent: a tool-level
    /// finding must not be rendered as a parameter-level one with an empty name.
    #[test]
    fn a_tool_level_finding_never_renders_an_empty_parameter_name() {
        let rendered = warning("get_cui", "").to_string();
        assert!(
            !rendered.contains("parameter"),
            "a tool-level finding must not claim a parameter: {rendered}"
        );
        assert!(
            !rendered.contains("''"),
            "no empty-name sentinel: {rendered}"
        );
    }
}
