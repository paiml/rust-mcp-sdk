// Net-new code for Phase 128 plan 09 (E1 + E2).
// The two escape hatches: a `RequestPolicy` governing what may LEAVE the server,
// and a per-tool `ArgumentValidator` for rules a JSON Schema cannot express.

//! The two explicitly-registered validation escape hatches (Phase 128, E1 + E2).
//!
//! A config-declared `inputSchema` covers rules about ONE value. Two classes of
//! rule it cannot express get a Rust seam here, so no team has to wrap or fork a
//! toolkit internal:
//!
//! - **E1 — [`RequestPolicy`]**: a rule about what may LEAVE the server (a PHI
//!   policy, an endpoint allowlist, a per-session budget). It runs on both HTTP
//!   surfaces, BEFORE the outgoing credential exists.
//! - **E2 — [`ArgumentValidator`]**: a rule about a COMBINATION of values (a
//!   parameter required only when another has a particular value, a code-list
//!   lookup). It runs strictly AFTER the declared schema check, so it never sees
//!   arguments the schema already refused.
//!
//! Both are registered on a [`ToolkitHooks`] value and handed to an assembly
//! entry point as a parameter. There is no builder-field accumulation, and the
//! reason is structural rather than stylistic: `ServerBuilderExt` is implemented
//! for CORE's `pmcp::ServerBuilder`, whose fields are private, and a Rust
//! extension trait cannot add a field to a foreign type. The two rejected
//! alternatives are recorded on [`ToolkitHooks`].
//!
//! # What this module deliberately does NOT do
//!
//! It contains no schema logic. The declared-schema check, the placeholder
//! character floor and the value-free refusal renderer all live in core
//! `pmcp::server::schema_validation`; these hooks sit around them.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use serde_json::Value;

use crate::config::ServerConfig;

// -----------------------------------------------------------------------------
// E1 — RequestPolicy
// -----------------------------------------------------------------------------

/// One outbound backend request, as it will be sent, handed to a
/// [`RequestPolicy`] for inspection.
///
/// # The D-12 guarantee, stated as a guarantee
///
/// This struct has NO credential-bearing field, and it is constructed BEFORE the
/// `HttpAuthProvider` runs on either HTTP surface. A policy implementation
/// therefore cannot observe an outgoing credential — not because it is asked not
/// to, but because the value it is handed was assembled before the credential
/// existed. `tests/request_policy.rs`'s credential-scan row asserts this by
/// searching every field for the configured secret.
///
/// Borrowed throughout (`&'a str`, `&'a [_]`): a server with no registered policy
/// never constructs one, so the empty case adds no allocation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct OutboundRequest<'a> {
    /// The MCP tool whose `tools/call` produced this request.
    ///
    /// Both shipped surfaces name a tool: the curated single-call surface passes
    /// the synthesized tool's own name, and the Code Mode surface passes the
    /// label attached to the executor at synthesis (a script tool's `[[tools]]`
    /// `name`, or `execute_code` for the generic Code Mode tool). On the Code
    /// Mode surface ONE `tools/call` may produce many outbound requests, and all
    /// of them carry that same label.
    ///
    /// Empty ONLY when a caller drives a connector directly rather than through a
    /// synthesized handler — there is then no tool to name. A policy that keys on
    /// the tool name should treat the empty string as "unattributed", never as a
    /// tool called `""`.
    pub tool: &'a str,

    /// The HTTP method, upper-cased (`GET`, `POST`, ...).
    pub method: &'a str,

    /// The FULLY RESOLVED request target: every path placeholder substituted and
    /// the configured base URL already joined on. The SDK appends no query string
    /// to it.
    ///
    /// It is the resolved path and never the template, so an endpoint allowlist
    /// sees the URL as it will be sent. The query pairs the SDK will add are
    /// carried separately in [`Self::query`].
    ///
    /// # An author-written `?` STAYS in `path`
    ///
    /// "The SDK appends no query string" is about what the SDK adds, not about what
    /// a script author wrote. On the Code Mode surface,
    /// `api.get('/search/current?string=x')` puts a literal `?string=x` in the path
    /// template, and the path floor deliberately permits ONE author-written `?`
    /// (`validate_resolved_target`), so it reaches a policy INSIDE `path` and never
    /// appears in [`Self::query`]. A policy that must see or refuse every query pair
    /// therefore has to look for a `?` in `path` as well as read `query`. The
    /// object form, `api.get(path, { .. })`, is what populates `query`.
    pub path: &'a str,

    /// The query pairs that will be appended to [`Self::path`], EXCLUDING any
    /// pair the auth provider contributes.
    ///
    /// An API-key-in-query credential is an auth contribution and is therefore
    /// absent here by construction — that omission is the D-12 guarantee, not an
    /// oversight.
    ///
    /// Populated on BOTH surfaces. On the Code Mode surface that required moving
    /// the non-auth half of the remaining-body-to-query conversion above the hook
    /// (Phase 128 plan 09); without that move a policy written to inspect query
    /// pairs would have inspected an empty slice while the pairs that were about
    /// to be sent still sat in [`Self::body`].
    pub query: &'a [(String, String)],

    /// The JSON request body, when one will be sent.
    ///
    /// `None` for a GET-like request, whose remaining fields have already been
    /// converted into [`Self::query`] by the time the policy runs.
    pub body: Option<&'a Value>,

    /// An opaque identifier for the `tools/call` that produced this request.
    ///
    /// **Stable across every request ONE `tools/call` makes.** On the Code Mode
    /// surface a single `execute_code` run can send many requests, and all of them
    /// carry the same id, so a policy can budget a whole run (total bytes, request
    /// count, distinct endpoints) instead of seeing each request in isolation. A
    /// per-request cap alone lets a caller split free text across several requests
    /// that each fit under it. On the curated surface a `tools/call` is one
    /// request, so the id is simply unique per request.
    ///
    /// Unique across calls within a process, and with overwhelming probability
    /// across restarts. It is NOT a secret and NOT a distributed trace id: it is
    /// generated here, never taken from the client, and it should not be put on
    /// the wire.
    ///
    /// Empty ONLY when unattributed (a caller driving a connector directly rather
    /// than through a synthesized handler), the same convention as [`Self::tool`].
    /// Treat the empty string as "no grouping", never as one shared bucket.
    pub call_id: &'a str,
}

impl<'a> OutboundRequest<'a> {
    /// Construct an [`OutboundRequest`].
    ///
    /// A constructor rather than a struct literal because the type is
    /// `#[non_exhaustive]`; this is also what lets an out-of-crate test build one
    /// to exercise a policy in isolation.
    #[must_use]
    pub fn new(
        tool: &'a str,
        method: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
        body: Option<&'a Value>,
    ) -> Self {
        Self {
            tool,
            method,
            path,
            query,
            body,
            call_id: "",
        }
    }

    /// Attach the per-`tools/call` identifier, see [`Self::call_id`].
    ///
    /// A builder rather than a sixth parameter of [`Self::new`], so existing
    /// callers of `new` keep compiling. The type is `#[non_exhaustive]`, which is
    /// what makes adding the field itself additive.
    #[must_use]
    pub fn with_call_id(mut self, call_id: &'a str) -> Self {
        self.call_id = call_id;
        self
    }
}

/// Mint the identifier for ONE `tools/call`, see [`OutboundRequest::call_id`].
///
/// A process-wide counter under a per-process prefix taken from the clock and the
/// process id. Dependency-free on purpose: this needs uniqueness, not
/// unpredictability, because the id is a grouping key for a policy and is never a
/// credential.
pub(crate) fn next_call_id() -> String {
    static PREFIX: OnceLock<u64> = OnceLock::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let prefix = *PREFIX.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        nanos ^ (u64::from(std::process::id()) << 32)
    });
    format!("{prefix:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// A [`RequestPolicy`]'s refusal of one outbound request.
///
/// # Residual: the message is authored by the policy, not by this phase
///
/// The message travels back to the MCP client. It is chosen by the policy
/// implementation, so the toolkit cannot mechanically constrain it — a policy
/// that interpolates the rejected value into its own message creates exactly the
/// value-echo leak every toolkit-authored refusal in this phase avoids
/// (T-128-40). Supply a FIXED string. Do not put a request value, a resolved
/// path, or a credential in it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRefusal {
    message: String,
}

impl PolicyRefusal {
    /// Refuse the request with a fixed message.
    ///
    /// The message must not carry any byte of the rejected request — see the type
    /// documentation for why the toolkit cannot enforce that for you.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The policy-supplied message, verbatim.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PolicyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PolicyRefusal {}

/// A rule about what may LEAVE the server, consulted before every outbound
/// backend request on both HTTP surfaces (Phase 128, E1 / D-12).
///
/// # Where it runs
///
/// Between the base-URL join and the auth application, on the curated
/// single-call surface (`http::HttpClient::execute`) and on the Code Mode /
/// script-tool surface (`code_mode::HttpCodeExecutor::execute_request`). The
/// position is what makes the D-12 guarantee structural: the request is fully
/// assembled, and the credential does not exist yet. A refusal returns before
/// the auth provider is called and before anything is sent.
///
/// # One invocation per LOGICAL request, not per wire attempt
///
/// The curated client's `send_with_retries` retries the ALREADY-BUILT request up
/// to three times on a 5xx / connect / timeout. Those retries happen after the
/// hook, so this trait gives exactly one invocation per logical outbound request.
/// A policy counting requests against a rate budget is therefore counting LOGICAL
/// requests; it will under-count wire attempts.
///
/// # What E1 does and does not govern
///
/// It governs the two HTTP egress surfaces named above. It does NOT intercept SQL
/// connector traffic: a `SqlConnector` request is a statement plus bound
/// parameters, not a method/path/query, so it needs a different seam and a
/// different trait. A team writing a PHI policy must know that its coverage stops
/// at HTTP egress.
///
/// Redirects are governed by construction rather than by this trait: the
/// OpenAPI binary's shared client is built with
/// `reqwest::redirect::Policy::none()`, so a redirect surfaces as a response the
/// caller handles rather than as a hop inside the client that this hook never
/// saw (T-128-39a). A client built elsewhere with reqwest's default
/// redirect policy re-opens that gap — every hop after the first would be
/// invisible here.
///
/// # Concurrency and latency
///
/// `check` takes `&self` and the trait requires `Send + Sync`, so ONE instance
/// serves every concurrent request. Any per-session state is the
/// implementation's own responsibility. `check` is `async` and the toolkit cannot
/// bound third-party code: a slow policy delays the request path (T-128-44,
/// accepted — the per-request time budget is a separate, deferred piece of work).
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::{async_trait, OutboundRequest, PolicyRefusal, RequestPolicy};
///
/// struct AllowlistPrefix(&'static str);
///
/// #[async_trait]
/// impl RequestPolicy for AllowlistPrefix {
///     async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
///         if req.path.starts_with(self.0) {
///             Ok(())
///         } else {
///             // A FIXED message: it names the rule, never the request.
///             Err(PolicyRefusal::new("outbound endpoint is not on the allowlist"))
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait RequestPolicy: Send + Sync {
    /// Allow or refuse one outbound backend request.
    ///
    /// # Errors
    ///
    /// Return [`PolicyRefusal`] to refuse. The request is then never
    /// authenticated and never sent, and the refusal's message is surfaced to the
    /// MCP client — so it must carry no byte of the rejected request.
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal>;
}

// -----------------------------------------------------------------------------
// E2 — ArgumentValidator
// -----------------------------------------------------------------------------

/// An [`ArgumentValidator`]'s refusal of one `tools/call`.
///
/// Carries the same implementation-supplied-message residual as
/// [`PolicyRefusal`]: the message reaches the MCP client and is authored by the
/// validator, so it must be a FIXED string carrying no argument value
/// (T-128-40).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentRefusal {
    message: String,
}

impl ArgumentRefusal {
    /// Refuse the call with a fixed message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The validator-supplied message, verbatim.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ArgumentRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ArgumentRefusal {}

/// A per-tool rule about a COMBINATION of argument values, run strictly AFTER
/// the declared `inputSchema` check (Phase 128, E2).
///
/// # Ordering is the contract
///
/// A validator NEVER sees arguments that failed the declared schema. It runs
/// inside the same decorator, after the schema check returns `Ok` and before the
/// inner handler. `tests/request_policy.rs`'s counting row fails if that order
/// inverts.
///
/// It also survives the schema opt-out: with `[server.validation]
/// enforce_input_schema = false` the decorator skips only the SCHEMA CHECK, and a
/// registered validator still runs. Turning off one enforcement must never
/// silently turn off another.
///
/// # Refuse-only: this signature cannot normalize
///
/// `validate` takes `&Value` and returns a refusal, so it cannot mutate the
/// arguments. Normalizing an argument before dispatch — rewriting a code, casing
/// a string — is therefore OUT OF SCOPE for this signature. That is a deliberate
/// restriction, not an omission: a mutating validator has a different contract
/// (idempotency, what the tool's published schema then describes, and whether
/// the schema check should re-run on the rewritten value), and that contract is
/// recorded as an open question rather than settled by the shape of a first
/// signature.
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::{ArgumentRefusal, ArgumentValidator};
/// use serde_json::Value;
///
/// struct EndAfterStart;
///
/// impl ArgumentValidator for EndAfterStart {
///     fn validate(&self, args: &Value) -> Result<(), ArgumentRefusal> {
///         let start = args.get("start").and_then(Value::as_i64);
///         let end = args.get("end").and_then(Value::as_i64);
///         match (start, end) {
///             (Some(s), Some(e)) if e < s => Err(ArgumentRefusal::new(
///                 "`end` must not precede `start`",
///             )),
///             _ => Ok(()),
///         }
///     }
/// }
/// ```
pub trait ArgumentValidator: Send + Sync {
    /// Allow or refuse one call's already-schema-valid arguments.
    ///
    /// # Errors
    ///
    /// Return [`ArgumentRefusal`] to refuse. The inner handler is then never
    /// invoked and no backend request is made.
    fn validate(&self, args: &Value) -> Result<(), ArgumentRefusal>;
}

/// A tool-name to [`ArgumentValidator`] registry.
///
/// Registration is LAST-ONE-WINS, and a replacement is announced once by a
/// `tracing::warn!` — a silently discarded validator is a rule the operator
/// believes is enforced and is not.
#[derive(Clone, Default)]
pub struct ArgumentValidators {
    map: HashMap<String, Arc<dyn ArgumentValidator>>,
}

impl ArgumentValidators {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `validator` for the tool named `tool`, REPLACING any validator
    /// already registered under that name and warning once when it does.
    pub fn insert(&mut self, tool: impl Into<String>, validator: Arc<dyn ArgumentValidator>) {
        let tool = tool.into();
        if self.map.contains_key(&tool) {
            tracing::warn!(
                target: "pmcp_server_toolkit::policy",
                tool = %tool,
                "an ArgumentValidator was already registered for this tool — the earlier one is \
                 REPLACED and will never run"
            );
        }
        self.map.insert(tool, validator);
    }

    /// The validator registered for `tool`, if any.
    #[must_use]
    pub fn get(&self, tool: &str) -> Option<Arc<dyn ArgumentValidator>> {
        self.map.get(tool).map(Arc::clone)
    }

    /// Whether any validator is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// How many validators are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Registered tool names, sorted, so the startup log is deterministic.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.map.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

impl fmt::Debug for ArgumentValidators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgumentValidators")
            .field("tools", &self.names())
            .finish()
    }
}

// -----------------------------------------------------------------------------
// ToolkitHooks — the registration value
// -----------------------------------------------------------------------------

/// The registered E1 policy and E2 validators, passed as a PARAMETER to an
/// assembly entry point.
///
/// # Why a parameter and not a builder field
///
/// `ServerBuilderExt` is implemented for CORE's `pmcp::ServerBuilder`, whose
/// fields are private. A Rust extension trait cannot add a field to a foreign
/// type, so a `with_request_policy(self) -> Self` on that trait would have
/// nowhere to accumulate. Two alternatives were rejected: a newtype wrapper
/// around `ServerBuilder` would force every existing `ServerBuilderExt` user to
/// change type, and adding an extension-storage mechanism to core's
/// `ServerBuilder` is a core API change this phase did not scope.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use pmcp_server_toolkit::{ArgumentRefusal, ArgumentValidator, ToolkitHooks};
/// use serde_json::Value;
///
/// struct Never;
/// impl ArgumentValidator for Never {
///     fn validate(&self, _args: &Value) -> Result<(), ArgumentRefusal> {
///         Err(ArgumentRefusal::new("this tool is disabled"))
///     }
/// }
///
/// let hooks = ToolkitHooks::default().with_argument_validator("get_line_status", Arc::new(Never));
/// assert_eq!(hooks.validator_names(), vec!["get_line_status"]);
/// ```
#[derive(Clone, Default)]
pub struct ToolkitHooks {
    policy: Option<Arc<dyn RequestPolicy>>,
    validators: ArgumentValidators,
}

impl ToolkitHooks {
    /// No policy and no validators — the shape every pre-existing entry point
    /// passes, so a server that registers neither behaves exactly as before.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the E1 [`RequestPolicy`], replacing any already set.
    #[must_use]
    pub fn with_request_policy(mut self, policy: Arc<dyn RequestPolicy>) -> Self {
        if self.policy.is_some() {
            tracing::warn!(
                target: "pmcp_server_toolkit::policy",
                "a RequestPolicy was already registered — the earlier one is REPLACED and will \
                 never run"
            );
        }
        self.policy = Some(policy);
        self
    }

    /// Register an E2 [`ArgumentValidator`] for one tool name (last one wins).
    #[must_use]
    pub fn with_argument_validator(
        mut self,
        tool: impl Into<String>,
        validator: Arc<dyn ArgumentValidator>,
    ) -> Self {
        self.validators.insert(tool, validator);
        self
    }

    /// The registered [`RequestPolicy`], if any.
    #[must_use]
    pub fn request_policy(&self) -> Option<Arc<dyn RequestPolicy>> {
        self.policy.as_ref().map(Arc::clone)
    }

    /// The [`ArgumentValidator`] registered for `tool`, if any.
    ///
    /// Named `argument_validator_for` and NOT `validator_for`: the root crate's
    /// `tests/v2_schema_tripwires.rs` scans every workspace source file for the
    /// token `validator_for(` as the signature of a `jsonschema` validator being
    /// constructed, and requires each site to declare its dialect policy. A method
    /// with that name here — and every call to it — would fire that security
    /// tripwire on code that has nothing to do with JSON Schema dialects, and the
    /// only ways out would be to bloat a dialect allowlist with non-dialect entries
    /// or to re-fire on every future call site. Measured: the short name failed the
    /// tripwire with two UNKNOWN sites.
    #[must_use]
    pub fn argument_validator_for(&self, tool: &str) -> Option<Arc<dyn ArgumentValidator>> {
        self.validators.get(tool)
    }

    /// Registered validator tool names, sorted.
    #[must_use]
    pub fn validator_names(&self) -> Vec<&str> {
        self.validators.names()
    }

    /// Whether this value registers nothing at all — the `Default` shape.
    ///
    /// Read by the assembly paths so a hooks value that registers something can
    /// be reported in the startup log, and so a path with no surface to apply a
    /// policy to can say so rather than accept it silently.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.policy.is_none() && self.validators.is_empty()
    }
}

impl fmt::Debug for ToolkitHooks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolkitHooks")
            .field("request_policy", &self.policy.is_some())
            .field("validators", &self.validators)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// The once-at-startup enforcement log (Phase 128, D-07)
// -----------------------------------------------------------------------------

/// The severity one [`render_validation_report`] line is emitted at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLevel {
    /// An enforcement that is ON, or the absence of any opt-out.
    Info,
    /// An enforcement that is OFF, or a registration that cannot take effect.
    Warn,
}

/// One rendered enforcement-report line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLine {
    /// Whether the line reports something ON or something OFF.
    pub level: ReportLevel,
    /// The rendered text, drawn from DECLARATIONS only.
    pub text: String,
}

/// Render what this server actually enforces, as the lines
/// [`emit_validation_report`] logs (Phase 128, D-07).
///
/// Separate from the emission so a test can assert the exact line FORMATS without
/// installing a `tracing` subscriber — and so the documentation deliverable has one
/// authority for what an operator will see.
///
/// # It carries no request data, by construction
///
/// Every line is built from `[server.validation]`, from
/// [`ServerConfig::validation_report`] (itself built from `[[tools]]` declarations),
/// and from which tool NAMES carry a registered validator. No argument value and no
/// credential can reach it, because none is in scope here. That is the property
/// that keeps the log from becoming the PHI channel SC-7 closed everywhere else.
///
/// # What it reports
///
/// In order: the effective `[server.validation]` policy; the always-on floor; one
/// line per tool naming its enforced rules; one WARN per ACTIVE opt-out, or one
/// INFO stating that none is active; the E1 policy registration state; and the E2
/// validator registrations, with a WARN for any registered under a tool name the
/// config does not declare.
///
/// An enforcement that is OFF is always stated. A log that listed only what is on
/// would let an operator read silence as safety.
#[must_use]
pub fn render_validation_report(config: &ServerConfig, hooks: &ToolkitHooks) -> Vec<ReportLine> {
    let report = config.validation_report();
    let mut out = Vec::new();

    let schema_check = if cfg!(feature = "input-validation") {
        if report.enforce_input_schema {
            "ON"
        } else {
            "OFF"
        }
    } else {
        "OFF (feature)"
    };
    out.push(ReportLine {
        level: if schema_check == "ON" {
            ReportLevel::Info
        } else {
            ReportLevel::Warn
        },
        text: format!(
            "input validation: schema_check={schema_check} default_max_length={} \
             additional_properties={} strict={} tools={}",
            report.default_max_length,
            report.additional_properties,
            report.strict,
            report.tools.len()
        ),
    });

    if !cfg!(feature = "input-validation") {
        out.push(ReportLine {
            level: ReportLevel::Warn,
            text: "input validation: the `input-validation` feature is OFF, so NO tool's \
                   arguments are checked against its declared inputSchema. It is in the \
                   toolkit's default feature set — an unenforced build is an explicit opt-out."
                .to_string(),
        });
    }

    for tool in &report.tools {
        let rules = if tool.rules.is_empty() {
            "(none declared; only the always-on path-placeholder character floor and \
             length cap apply)"
                .to_string()
        } else {
            tool.rules.join("; ")
        };
        out.push(ReportLine {
            level: ReportLevel::Info,
            text: format!("input validation: tool '{}' enforces {rules}", tool.tool),
        });
    }

    if report.opt_outs.is_empty() {
        out.push(ReportLine {
            level: ReportLevel::Info,
            text: "input validation: no [server.validation] opt-out is active — every rule \
                   this config can enforce is enforced."
                .to_string(),
        });
    } else {
        for opt_out in &report.opt_outs {
            out.push(ReportLine {
                level: ReportLevel::Warn,
                text: format!("input validation: [server.validation] opt-out ACTIVE — {opt_out}"),
            });
        }
    }

    render_hooks_lines(config, hooks, &mut out);
    out
}

/// The E1 / E2 registration half of [`render_validation_report`].
///
/// Its own function to keep the caller under the cognitive-complexity 25 gate. A
/// validator registered for a tool name the config does not declare is a WARN and
/// NOT an error: a typo must be visible, but failing hard on a name a later config
/// edit will introduce is worse than a warning.
fn render_hooks_lines(config: &ServerConfig, hooks: &ToolkitHooks, out: &mut Vec<ReportLine>) {
    out.push(ReportLine {
        level: ReportLevel::Info,
        text: format!(
            "input validation: E1 RequestPolicy registered={}",
            hooks.request_policy().is_some()
        ),
    });

    let names = hooks.validator_names();
    if names.is_empty() {
        out.push(ReportLine {
            level: ReportLevel::Info,
            text: "input validation: no E2 ArgumentValidator is registered".to_string(),
        });
        return;
    }
    out.push(ReportLine {
        level: ReportLevel::Info,
        text: format!(
            "input validation: E2 ArgumentValidator registered for {}",
            names.join(", ")
        ),
    });
    for name in names {
        if !config.tools.iter().any(|t| t.name == name) {
            out.push(ReportLine {
                level: ReportLevel::Warn,
                text: format!(
                    "input validation: an ArgumentValidator is registered for '{name}', which \
                     this config declares no [[tools]] entry for — it will never run"
                ),
            });
        }
    }
}

/// Emit the enforcement report ONCE per server, at startup (Phase 128, D-07).
///
/// The ONE formatter, called from BOTH assembly sites:
/// [`crate::ServerBuilderExt::try_tools_from_config_with`] and
/// `pmcp-openapi-server`'s `build_server`. Two call sites rather than two
/// formatters, because the OpenAPI binary reaches the free synthesizer directly and
/// never goes through the builder path — a report emitted only there would be
/// absent from the deployment that most needs it (T-128-42a).
///
/// This log is the mechanism for tracing a server whose previously-unenforced
/// `enum` or `pattern` starts refusing calls. It is not optional polish.
///
/// # Emitted once per SERVER, not once per call
///
/// Deduplicated on a hash of the RENDERED lines plus the server name and version,
/// so a process that reaches both assembly paths for the same server logs once
/// while a process hosting two DIFFERENT servers logs for each. Two servers with a
/// byte-identical config and identical registrations log once between them — an
/// accepted and stated limitation, since there is nothing in the report that would
/// differ.
///
/// # It carries no request data
///
/// Guaranteed by [`render_validation_report`], which has no argument value and no
/// credential in scope.
pub fn emit_validation_report(config: &ServerConfig, hooks: &ToolkitHooks) {
    let lines = render_validation_report(config, hooks);
    if !claim_report_emission(&config.server.name, &config.server.version, &lines) {
        return;
    }
    for line in lines {
        match line.level {
            ReportLevel::Info => {
                tracing::info!(target: "pmcp_server_toolkit::policy", "{}", line.text);
            },
            ReportLevel::Warn => {
                tracing::warn!(target: "pmcp_server_toolkit::policy", "{}", line.text);
            },
        }
    }
}

/// Claim the right to emit for this (server, report) pair, returning `false` when
/// it was already claimed.
fn claim_report_emission(name: &str, version: &str, lines: &[ReportLine]) -> bool {
    static EMITTED: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    version.hash(&mut hasher);
    for line in lines {
        line.text.hash(&mut hasher);
    }
    let key = hasher.finish();
    EMITTED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map_or(true, |mut seen| seen.insert(key))
}

#[cfg(test)]
mod tests {
    use super::{
        next_call_id, ArgumentRefusal, ArgumentValidator, ArgumentValidators, OutboundRequest,
        PolicyRefusal, RequestPolicy, ToolkitHooks,
    };
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Refuse(&'static str);

    #[async_trait::async_trait]
    impl RequestPolicy for Refuse {
        async fn check(&self, _req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
            Err(PolicyRefusal::new(self.0))
        }
    }

    struct CountingValidator(Arc<AtomicUsize>);

    impl ArgumentValidator for CountingValidator {
        fn validate(&self, _args: &Value) -> Result<(), ArgumentRefusal> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn outbound_request_constructor_exposes_every_field() {
        let query = vec![("q".to_string(), "x".to_string())];
        let body = json!({ "note": "n" });
        let req = OutboundRequest::new("t", "GET", "https://h/p/1", &query, Some(&body));
        assert_eq!(req.tool, "t");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "https://h/p/1");
        assert_eq!(req.query.len(), 1);
        assert_eq!(req.body, Some(&body));
    }

    #[test]
    fn refusals_display_their_own_message_verbatim() {
        assert_eq!(PolicyRefusal::new("nope").to_string(), "nope");
        assert_eq!(ArgumentRefusal::new("bad combo").to_string(), "bad combo");
        assert_eq!(PolicyRefusal::new("nope").message(), "nope");
        assert_eq!(ArgumentRefusal::new("bad combo").message(), "bad combo");
    }

    #[tokio::test]
    async fn a_policy_refusal_carries_the_policy_message() {
        let policy = Refuse("blocked by test policy");
        let empty: Vec<(String, String)> = Vec::new();
        let req = OutboundRequest::new("t", "GET", "https://h/p", &empty, None);
        let err = policy.check(&req).await.expect_err("refuses");
        assert_eq!(err.message(), "blocked by test policy");
    }

    #[test]
    fn validator_registration_is_last_one_wins() {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let mut reg = ArgumentValidators::new();
        assert!(reg.is_empty());
        reg.insert("t", Arc::new(CountingValidator(Arc::clone(&first))));
        reg.insert("t", Arc::new(CountingValidator(Arc::clone(&second))));
        assert_eq!(reg.len(), 1);
        reg.get("t")
            .expect("registered")
            .validate(&json!({}))
            .expect("allows");
        assert_eq!(
            first.load(Ordering::SeqCst),
            0,
            "the replaced validator ran"
        );
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn default_hooks_register_nothing() {
        let hooks = ToolkitHooks::default();
        assert!(hooks.is_empty());
        assert!(hooks.request_policy().is_none());
        assert!(hooks.argument_validator_for("anything").is_none());
        assert!(hooks.validator_names().is_empty());
    }

    #[test]
    fn hooks_builder_records_both_kinds() {
        let hooks = ToolkitHooks::new()
            .with_request_policy(Arc::new(Refuse("x")))
            .with_argument_validator(
                "b",
                Arc::new(CountingValidator(Arc::new(AtomicUsize::new(0)))),
            )
            .with_argument_validator(
                "a",
                Arc::new(CountingValidator(Arc::new(AtomicUsize::new(0)))),
            );
        assert!(!hooks.is_empty());
        assert!(hooks.request_policy().is_some());
        assert_eq!(hooks.validator_names(), vec!["a", "b"]);
    }

    #[test]
    fn hooks_debug_never_renders_a_policy_body() {
        let hooks = ToolkitHooks::new().with_request_policy(Arc::new(Refuse("secret-ish")));
        let rendered = format!("{hooks:?}");
        assert!(rendered.contains("request_policy: true"));
        assert!(!rendered.contains("secret-ish"));
    }

    /// `OutboundRequest::new` leaves `call_id` empty ("unattributed") and
    /// `with_call_id` sets it, so existing five-argument callers keep compiling and
    /// keep their old meaning.
    #[test]
    fn call_id_defaults_to_unattributed_and_the_builder_sets_it() {
        let req = OutboundRequest::new("t", "GET", "/p", &[], None);
        assert_eq!(req.call_id, "", "new() must not invent an id");
        assert_eq!(req.with_call_id("abc").call_id, "abc");
    }

    /// A grouping key that repeats is worse than none: a budget keyed on it would
    /// charge one caller for another's traffic.
    #[test]
    fn next_call_id_is_non_empty_and_never_repeats() {
        let ids: std::collections::HashSet<String> = (0..2000).map(|_| next_call_id()).collect();
        assert_eq!(ids.len(), 2000, "every minted id must be distinct");
        assert!(ids.iter().all(|id| !id.is_empty()));
    }
}
