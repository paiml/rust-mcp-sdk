//! The `SlotResolver` seam + two impls + `resolve_agent` composition (AGNT-09).
//!
//! An `AgentPackage` declares its runtime dependencies as config *slots* (an LLM
//! provider, secrets, budget overrides) and connector refs. A [`SlotResolver`]
//! binds those to concrete values against a host environment, and
//! [`resolve_agent`] composes the whole package into a [`ResolvedAgentConfig`]
//! the engine/adapter can run.
//!
//! Two seam impls ship: [`EnvVarResolver`] (reads conventionally-named env vars)
//! and [`ProgrammaticBuilder`] (explicit in-memory values). Both share two
//! safety invariants:
//!
//! - **Deviations warn, never fail (D-15):** when a resolved behavior-relevant
//!   value differs from the package's `tested_value`,
//!   [`detect_deviation`](pmcp_package::detect_deviation) flags it and the
//!   resolver emits a loud `tracing::warn!` — then PROCEEDS. Silent config drift
//!   is the threat (T-108-05-02); a hard error is not (the package still runs).
//! - **Secrets never travel to logs (ASVS V7, T-108-05-01):** a resolved secret
//!   is wrapped in [`RedactedSecret`], whose `Debug`/`Display` reveal nothing.

use async_trait::async_trait;
use std::collections::HashMap;

use pmcp_package::{detect_deviation, AgentPackage, ConfigSlot, SlotType};

use super::endpoint::build_endpoint_map;
use super::ResolvedAgentConfig;

/// A secret string that never reveals itself in `Debug`, `Display`, or logs.
///
/// The inner value is readable only via [`expose`](RedactedSecret::expose), so a
/// resolved API key cannot leak through a stray `{:?}` or `tracing` field.
#[derive(Clone)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    /// Wrap a secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Reveal the secret — call sites are the audit surface for secret use.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedactedSecret(***)")
    }
}

/// A resolved slot value.
#[derive(Clone, Debug)]
pub enum ResolvedValue {
    /// A non-secret behavior value (e.g. an LLM model id).
    Plain(String),
    /// A secret value, redacted everywhere but [`RedactedSecret::expose`].
    Secret(RedactedSecret),
}

/// An error resolving a slot or endpoint. Never contains secret material.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    /// A required slot had no value in the environment.
    #[error("required slot '{0}' is not available")]
    MissingSlot(String),
    /// A connector had no configured endpoint.
    #[error("no endpoint configured for connector '{0}'")]
    MissingEndpoint(String),
    /// The slot could not be resolved to a usable value.
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

/// Resolves an `AgentPackage`'s config slots and connector endpoints.
#[async_trait]
pub trait SlotResolver: Send + Sync {
    /// Resolve one config slot to its runtime value.
    ///
    /// Behavior-relevant slots (LLM provider, budget override) resolve to a
    /// [`ResolvedValue::Plain`]; identity-bearing secret slots resolve to a
    /// [`ResolvedValue::Secret`]. A required-but-absent slot yields
    /// [`ResolveError::MissingSlot`] — never a panic.
    async fn resolve_slot(&self, slot: &ConfigSlot) -> Result<ResolvedValue, ResolveError>;

    /// Resolve a connector name to its endpoint (URL or command) (D-16).
    async fn resolve_endpoint(&self, name: &str) -> Result<String, ResolveError>;
}

/// Warn (and proceed) when a resolved value deviates from the slot's tested
/// value (D-15). No-op for identity-bearing slots (they have no tested value).
fn warn_if_deviates(tested: &SlotType, resolved: &str) {
    // `SlotType::with_tested_value` owns the behavior-relevant/identity-bearing
    // split — one exhaustive match in pmcp-package, next to `tested_value`, so a
    // future variant cannot be routed to different families here and there.
    // Identity-bearing slots return `None`: no tested value to deviate from.
    let Some(proposed) = tested.with_tested_value(resolved) else {
        return;
    };
    if let Some(dev) = detect_deviation(tested, &proposed) {
        tracing::warn!(
            slot = %dev.slot_name,
            tested = %dev.tested,
            running = %dev.proposed,
            "config deviates from tested value — running anyway (D-15)"
        );
    }
}

/// Shared slot-resolution policy for the two [`SlotResolver`] impls.
///
/// The only per-resolver difference is where a value comes from, so both
/// resolvers supply two lookups: `lookup_plain` for behavior-relevant slots
/// (which fall back to the tested default and warn on deviation, D-15) and
/// `lookup_secret` for identity-bearing slots (which must be present and are
/// returned redacted). `kind` names the resolver in the human-role error.
fn resolve_slot_with<P, S>(
    slot: &ConfigSlot,
    kind: &str,
    lookup_plain: P,
    lookup_secret: S,
) -> Result<ResolvedValue, ResolveError>
where
    P: FnOnce(&str) -> Option<String>,
    S: FnOnce(&str) -> Option<String>,
{
    let (_, name) = slot.slot.key();
    match &slot.slot {
        // Behavior-relevant: a supplied override, else the tested default. Either
        // way the run proceeds; a differing value only warns.
        SlotType::LlmProvider { tested_value, .. }
        | SlotType::BudgetOverride { tested_value, .. }
        | SlotType::Endpoint { tested_value, .. }
        | SlotType::AuthMode { tested_value, .. } => {
            let value = lookup_plain(name).unwrap_or_else(|| tested_value.clone());
            warn_if_deviates(&slot.slot, &value);
            Ok(ResolvedValue::Plain(value))
        },
        // Identity-bearing: must be supplied; absence is fatal.
        SlotType::Secret { .. }
        | SlotType::OauthClient { .. }
        | SlotType::ChannelBinding { .. } => {
            let value =
                lookup_secret(name).ok_or_else(|| ResolveError::MissingSlot(name.to_string()))?;
            Ok(ResolvedValue::Secret(RedactedSecret::new(value)))
        },
        SlotType::HumanRole { role, .. } => Err(ResolveError::Invalid(format!(
            "human role '{role}' is a team binding, not a {kind}-resolvable slot"
        ))),
    }
}

/// Resolves slots from conventionally-named environment variables.
///
/// A slot named `primary-llm` maps to `{PREFIX}PRIMARY_LLM`; a connector named
/// `london-tube` maps to `{PREFIX}LONDON_TUBE_ENDPOINT` (`-` → `_`, uppercased).
#[derive(Debug, Clone, Default)]
pub struct EnvVarResolver {
    prefix: String,
}

impl EnvVarResolver {
    /// Create a resolver with no env-var prefix.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver that prefixes every env-var name with `prefix`.
    #[must_use]
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn env_name(&self, slot_name: &str) -> String {
        format!(
            "{}{}",
            self.prefix,
            slot_name.to_uppercase().replace('-', "_")
        )
    }

    fn endpoint_env_name(&self, connector: &str) -> String {
        format!("{}_ENDPOINT", self.env_name(connector))
    }
}

#[async_trait]
impl SlotResolver for EnvVarResolver {
    async fn resolve_slot(&self, slot: &ConfigSlot) -> Result<ResolvedValue, ResolveError> {
        // Env resolution reads the same conventional variable for both plain and
        // secret slots; the shared policy decides fallback vs. required.
        let lookup = |name: &str| std::env::var(self.env_name(name)).ok();
        resolve_slot_with(slot, "env", lookup, lookup)
    }

    async fn resolve_endpoint(&self, name: &str) -> Result<String, ResolveError> {
        std::env::var(self.endpoint_env_name(name))
            .map_err(|_| ResolveError::MissingEndpoint(name.to_string()))
    }
}

/// Resolves slots from explicit, in-memory values (tests, embedded hosts).
#[derive(Debug, Clone, Default)]
pub struct ProgrammaticBuilder {
    values: HashMap<String, String>,
    secrets: HashMap<String, String>,
    endpoints: HashMap<String, String>,
}

impl ProgrammaticBuilder {
    /// Create an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a behavior-relevant slot (e.g. the LLM model id) by slot name.
    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(name.into(), value.into());
        self
    }

    /// Bind a secret slot by name — stored redacted, never logged.
    #[must_use]
    pub fn with_secret(mut self, name: impl Into<String>, secret: impl Into<String>) -> Self {
        self.secrets.insert(name.into(), secret.into());
        self
    }

    /// Bind a connector name to an endpoint.
    #[must_use]
    pub fn with_endpoint(mut self, name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        self.endpoints.insert(name.into(), endpoint.into());
        self
    }
}

#[async_trait]
impl SlotResolver for ProgrammaticBuilder {
    async fn resolve_slot(&self, slot: &ConfigSlot) -> Result<ResolvedValue, ResolveError> {
        resolve_slot_with(
            slot,
            "programmatic",
            |name| self.values.get(name).cloned(),
            |name| self.secrets.get(name).cloned(),
        )
    }

    async fn resolve_endpoint(&self, name: &str) -> Result<String, ResolveError> {
        self.endpoints
            .get(name)
            .cloned()
            .ok_or_else(|| ResolveError::MissingEndpoint(name.to_string()))
    }
}

/// Clamp a package's `i64` limit into the config's `u32` (non-negative, capped).
fn clamp_u32(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

/// Flatten an `AgentPackage.tool_selection` document into a flat tool-name list.
///
/// The source shape is `{ "<connector>": ["tool_a", ...], ... }`; connector keys
/// are iterated in document order (`serde_json` preserves order in this crate),
/// so the resolved tool list is deterministic.
fn extract_tool_names(selection: Option<&serde_json::Value>) -> Vec<String> {
    let Some(serde_json::Value::Object(map)) = selection else {
        return Vec::new();
    };
    let mut tools = Vec::new();
    for tool_list in map.values() {
        if let serde_json::Value::Array(items) = tool_list {
            for item in items {
                if let serde_json::Value::String(name) = item {
                    tools.push(name.clone());
                }
            }
        }
    }
    tools
}

/// Compose an `AgentPackage` into a fully-resolved [`ResolvedAgentConfig`]
/// through `resolver` — the end-to-end AGNT-09 path.
///
/// Resolves the LLM model slot (warning on deviation), flattens the tool
/// selection, clamps the integer limits, carries the I/O schemas verbatim, and
/// builds the connector-name → endpoint map.
///
/// # Errors
///
/// Returns [`ResolveError`] if a required slot/endpoint is unavailable or the
/// LLM slot resolves to a secret (a misconfiguration).
pub async fn resolve_agent(
    pkg: &AgentPackage,
    resolver: &dyn SlotResolver,
) -> Result<ResolvedAgentConfig, ResolveError> {
    let model = match resolver.resolve_slot(&pkg.llm).await? {
        ResolvedValue::Plain(value) => value,
        ResolvedValue::Secret(_) => {
            return Err(ResolveError::Invalid(
                "llm slot resolved to a secret value".to_string(),
            ))
        },
    };

    let endpoints = build_endpoint_map(&pkg.connectors, resolver).await?;

    let mut config = ResolvedAgentConfig::new(
        pkg.instructions.clone(),
        model,
        clamp_u32(pkg.max_tokens),
        clamp_u32(pkg.max_iterations),
    );
    config.tools = extract_tool_names(pkg.tool_selection.as_ref());
    config.input_schema.clone_from(&pkg.input_schema);
    config.output_schema.clone_from(&pkg.output_schema);
    config.endpoints = endpoints;
    Ok(config)
}
