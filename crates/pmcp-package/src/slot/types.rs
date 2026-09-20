//! Config-slot type system (/ /): typed slot declarations that structurally
//! cannot carry a secret or identity value.
//!
//! `SlotType` splits into two families (see `classification::classify` for the mapping):
//! - **Identity-bearing** (`Secret`, `OauthClient`, `ChannelBinding`, `HumanRole`) declare
//!   only a *name* (or, for `HumanRole`, descriptive metadata) — a resolved secret/identity
//!   value is not representable in this type at all. This is a compile-time absence, not a
//!   runtime check that could be forgotten: the strongest form of "secrets never travel"
//!   (§12 permanent non-goal) a Rust type system can offer.
//! - **Behavior-relevant** (`LlmProvider`, `BudgetOverride`, `Endpoint`, `AuthMode`) carry
//!   the `tested_value` that was exercised when the package was tested, so a later proposed
//!   binding can be compared against it (see `deviation::detect_deviation`). Swapping the
//!   backend a config server talks to, or the scheme it authenticates with, changes what the
//!   served tools DO — so both belong here rather than in the identity-bearing family.
//!
//! Note what is deliberately absent: there is no variant naming the OpenAPI spec. The spec
//! determines the served tool surface, so it is BAKED into the package and can only ever
//! move the package digest — it is never a slot a target environment fills in at unpack.

use serde::{Deserialize, Serialize};

/// A config slot's typed declaration. Serializes with a snake_case `type` discriminator
/// (e.g. `{"type":"secret","name":"LICHESS_API_KEY"}`).
///
/// Deliberately NOT `#[serde(deny_unknown_fields)]` — forward-compatible for future slot
/// kinds (RESEARCH Pitfall 4): an older reader silently ignores fields it doesn't know about
/// rather than hard-failing on a newer producer's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlotType {
    /// A named secret the component requires at runtime (e.g. an API key). Declares only
    /// the secret's `name` — CRITICAL: no `value`/`secret`/`credential` field exists on this
    /// variant; a resolved secret value cannot be constructed into it.
    Secret {
        /// The secret's declared name (e.g. `LICHESS_API_KEY`), never its resolved value.
        name: String,
    },
    /// A named OAuth client credential the component requires. Declares only `name` — never
    /// a client secret or token.
    OauthClient {
        /// The OAuth client credential's declared name.
        name: String,
    },
    /// A named channel-binding the component requires (e.g. which notification channel a
    /// component posts to). Declares only `name` — never the resolved channel/user identity.
    ChannelBinding {
        /// The channel binding's declared name.
        name: String,
    },
    /// A human role a team component needs filled (from `AgentTeam`/`TeamHumanMember`).
    /// Declares descriptive fields only — NEVER `userId`/`channelId`/`email` (those are
    /// identity, resolved at bind time, not representable here).
    HumanRole {
        /// The role label (e.g. "approver").
        role: String,
        /// A human-readable description of the role's purpose.
        description: String,
        /// The responsibilities this role is expected to cover.
        responsibilities: Vec<String>,
        /// Hints about which channel kinds are suitable for this role (display-only).
        channel_hints: Vec<String>,
    },
    /// A named LLM provider slot, carrying the `tested_value` (e.g. `"anthropic"`) that was
    /// exercised when the package was tested. Behavior-relevant — a proposed binding
    /// that differs from `tested_value` is a real behavioral change, not an identity swap.
    LlmProvider {
        /// The slot's declared name.
        name: String,
        /// The provider value exercised when the package was tested.
        tested_value: String,
    },
    /// A named budget-override slot, carrying the `tested_value` that was exercised when the
    /// package was tested. Behavior-relevant.
    BudgetOverride {
        /// The slot's declared name.
        name: String,
        /// The budget-override value exercised when the package was tested.
        tested_value: String,
    },
    /// A named backend-endpoint slot, carrying the `tested_value` (e.g.
    /// `"https://api.tfl.gov.uk"`) that was exercised when the package was tested.
    /// Behavior-relevant: pointing a config server at a different backend changes what its
    /// tools return, which is a real behavioral change and not an identity swap. Carries a
    /// URL, never a credential.
    Endpoint {
        /// The slot's declared name.
        name: String,
        /// The endpoint value exercised when the package was tested.
        tested_value: String,
    },
    /// A named authentication-mode slot, carrying the `tested_value` (e.g. `"api_key"`,
    /// `"bearer"`) that was exercised when the package was tested. Behavior-relevant: the
    /// auth SCHEME determines how requests are formed, so changing it is a behavioral
    /// change. CRITICAL: this is the scheme discriminator only — the credential itself is a
    /// separate identity-bearing `Secret` slot, and no resolved secret value is
    /// representable here.
    AuthMode {
        /// The slot's declared name.
        name: String,
        /// The auth-mode discriminator exercised when the package was tested.
        tested_value: String,
    },
}

impl SlotType {
    /// A stable `(kind, name)` key identifying this slot for dedup/aggregation purposes.
    /// `name` is the variant's identifying field — the slot's own `name` for named slots,
    /// and `role` for `HumanRole` (which has no `name` field).
    pub fn key(&self) -> (&'static str, &str) {
        match self {
            SlotType::Secret { name } => ("secret", name.as_str()),
            SlotType::OauthClient { name } => ("oauth_client", name.as_str()),
            SlotType::ChannelBinding { name } => ("channel_binding", name.as_str()),
            SlotType::HumanRole { role, .. } => ("human_role", role.as_str()),
            SlotType::LlmProvider { name, .. } => ("llm_provider", name.as_str()),
            SlotType::BudgetOverride { name, .. } => ("budget_override", name.as_str()),
            SlotType::Endpoint { name, .. } => ("endpoint", name.as_str()),
            SlotType::AuthMode { name, .. } => ("auth_mode", name.as_str()),
        }
    }

    /// The `tested_value` carried by a behavior-relevant variant, or `None` for an
    /// identity-bearing variant (which has no such field at all).
    ///
    /// This match is EXHAUSTIVE by design — there is deliberately no catch-all arm. A
    /// catch-all would make every future variant default to `None`, which
    /// [`classify`](crate::slot::classification::classify) reads as `IdentityBearing`, which
    /// in turn makes [`detect_deviation`](crate::slot::deviation::detect_deviation)
    /// short-circuit and never fire for it. That failure mode leaves every existing test
    /// green, so it is prevented structurally: with the arms enumerated, adding a variant
    /// without deciding its family is a compile error.
    pub fn tested_value(&self) -> Option<&str> {
        match self {
            // Identity-bearing: these variants have no `tested_value` field at all.
            SlotType::Secret { .. }
            | SlotType::OauthClient { .. }
            | SlotType::ChannelBinding { .. }
            | SlotType::HumanRole { .. } => None,
            // Behavior-relevant: each carries the value exercised when the package was tested.
            SlotType::LlmProvider { tested_value, .. }
            | SlotType::BudgetOverride { tested_value, .. }
            | SlotType::Endpoint { tested_value, .. }
            | SlotType::AuthMode { tested_value, .. } => Some(tested_value.as_str()),
        }
    }

    /// A copy of this slot with its `tested_value` replaced by `value`, or `None` for an
    /// identity-bearing variant (which has no such field to replace).
    ///
    /// This is the one place a "proposed" slot is built from a resolved value, so the
    /// behavior-relevant/identity-bearing split stays enumerated here — next to
    /// [`tested_value`](Self::tested_value), the match that defines it — instead of being
    /// re-encoded by each consumer that wants to feed
    /// [`detect_deviation`](crate::slot::deviation::detect_deviation) a resolved value.
    /// EXHAUSTIVE by design, no catch-all, for the same reason as `tested_value`: adding a
    /// variant without deciding its family must be a compile error, not a silent skip.
    pub fn with_tested_value(&self, value: &str) -> Option<SlotType> {
        match self {
            // Identity-bearing: no tested value to replace.
            SlotType::Secret { .. }
            | SlotType::OauthClient { .. }
            | SlotType::ChannelBinding { .. }
            | SlotType::HumanRole { .. } => None,
            // Behavior-relevant: mirror the variant with the resolved value.
            SlotType::LlmProvider { name, .. } => Some(SlotType::LlmProvider {
                name: name.clone(),
                tested_value: value.to_string(),
            }),
            SlotType::BudgetOverride { name, .. } => Some(SlotType::BudgetOverride {
                name: name.clone(),
                tested_value: value.to_string(),
            }),
            SlotType::Endpoint { name, .. } => Some(SlotType::Endpoint {
                name: name.clone(),
                tested_value: value.to_string(),
            }),
            SlotType::AuthMode { name, .. } => Some(SlotType::AuthMode {
                name: name.clone(),
                tested_value: value.to_string(),
            }),
        }
    }
}

/// A single declared config slot held by a package component. The one canonical "a component
/// declares this slot" type — packages hold `Vec<ConfigSlot>`.
///
/// `#[non_exhaustive]`: construct with [`ConfigSlot::new`] and [`ConfigSlot::with_config_key`]
/// rather than a struct literal. Adding `config_key` broke every struct literal in this
/// repository; the attribute is what stops the NEXT field from doing it again.
/// WHO fills a slot's value — orthogonal to WHAT kind of value it is.
///
/// Who supplies a value and whether its value is behaviour-relevant are
/// independent axes, so this never touches [`crate::slot::classify`] or
/// [`crate::slot::detect_deviation`]: a platform-supplied ENDPOINT stays
/// deviation-visible. Collapsing the two would re-hide exactly what deviation
/// detection exists to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SuppliedBy {
    /// The operator supplies it in the target environment. The default, and the
    /// only class [`crate::slot::required_slots`] enumerates.
    #[default]
    Environment,
    /// The hosting platform injects it at deploy time — never operator-supplied.
    Platform,
    /// The runtime injects it (e.g. `AWS_LAMBDA_FUNCTION_NAME`): neither the
    /// operator nor the platform supplies it, and nothing needs to.
    Runtime,
}

impl SuppliedBy {
    /// Whether a target environment's operator must supply this value.
    ///
    /// The one predicate `required_slots` and every renderer key on, so "who is
    /// asked for this" has a single definition rather than a `!= Environment`
    /// test repeated at each call site.
    #[must_use]
    pub fn is_operator_supplied(self) -> bool {
        matches!(self, Self::Environment)
    }
}

/// `skip_serializing_if` hands the field by reference, so the public
/// `Copy`-taking predicate cannot be named there directly.
fn is_environment_supplied(supplied_by: &SuppliedBy) -> bool {
    supplied_by.is_operator_supplied()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ConfigSlot {
    /// The slot's typed declaration.
    pub slot: SlotType,
    /// The dotted TOML path in the server's own `config.toml` that this slot fills — e.g.
    /// `backend.base_url`, `backend.auth.query_params.app_key`, `backend.auth.type`.
    ///
    /// Distinct from the slot's `name`: for a [`SlotType::Secret`] the name is the
    /// ENVIRONMENT VARIABLE name (`TFL_APP_KEY`), while `config_key` is the CONFIG PATH the
    /// resolved value is written to. A slot that fills no config key (every agent/team slot
    /// today) leaves this `None`. It is a path, never a value — no credential is
    /// representable here.
    ///
    /// This is what plan 120-05's pack-time placeholder validation looks up, and what tells a
    /// target environment WHERE to put the value it supplies.
    ///
    /// # Compatibility — both halves, because only stating one under-scopes the next change
    ///
    /// - **Serde/wire: ADDITIVE.** `#[serde(default)]` means slot JSON written before this
    ///   field existed still deserializes (yielding `None`), and `skip_serializing_if` means
    ///   nothing new is emitted for a `None`. No checked-in fixture byte and no pinned digest
    ///   moves. `skip_serializing_if` is load-bearing here, not cosmetic.
    /// - **Rust source: BREAKING.** `ConfigSlot` had exactly one field before this, so a
    ///   second public field breaks every struct literal in the language, everywhere — 40
    ///   construction sites across `pmcp-package`, `pmcp-agent`, `pmcp-team-servers`,
    ///   `cargo-pmcp` and their examples and integration tests. A reader who takes "additive"
    ///   at face value will under-scope the next field addition exactly as this one was
    ///   originally under-scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_key: Option<String>,
    /// Who fills this slot's value. Defaults to [`SuppliedBy::Environment`].
    ///
    /// # Compatibility — ADDITIVE on both axes, unlike `config_key`
    ///
    /// - **Serde/wire:** `#[serde(default)]` deserializes pre-`supplied_by` slot
    ///   JSON to `Environment`, and `skip_serializing_if` emits nothing for it,
    ///   so no checked-in fixture byte and no pinned digest moves.
    /// - **Rust source:** ALSO additive, which is the difference from
    ///   `config_key`'s addition. `ConfigSlot` is `#[non_exhaustive]` — added in
    ///   response to that break — so no crate outside this one can write a
    ///   struct literal, and a repo-wide sweep finds ZERO literals even inside
    ///   it: every construction goes through [`ConfigSlot::new`] and the
    ///   `with_*` builders. The attribute did the job it was added for.
    #[serde(default, skip_serializing_if = "is_environment_supplied")]
    pub supplied_by: SuppliedBy,
}

impl ConfigSlot {
    /// A config slot that fills no config key — the shape every agent/team package slot takes.
    ///
    /// ```
    /// use pmcp_package::{ConfigSlot, SlotType};
    ///
    /// let slot = ConfigSlot::new(SlotType::Secret {
    ///     name: "TFL_APP_KEY".to_string(),
    /// });
    /// assert_eq!(slot.config_key, None);
    /// // Nothing is emitted for a `None` config key, so existing fixtures stay byte-identical.
    /// let json = serde_json::to_value(&slot).unwrap();
    /// assert!(json.get("config_key").is_none());
    /// ```
    pub fn new(slot: SlotType) -> Self {
        Self {
            slot,
            config_key: None,
            supplied_by: SuppliedBy::Environment,
        }
    }

    /// Name the dotted TOML config path this slot fills.
    ///
    /// Note the two strings below are DIFFERENT on purpose, per the `config_key`
    /// field doc above: `name` is the ENVIRONMENT VARIABLE the target
    /// environment sets (`TFL_BASE_URL`, which is also what the config's
    /// `${TFL_BASE_URL}` placeholder reads), while `config_key` is the CONFIG
    /// PATH the resolved value is written to. Putting the config path in `name`
    /// makes `pmcp-agent`'s resolver derive the unsettable variable
    /// `BACKEND.BASE_URL`, so the slot silently falls back to its tested value.
    ///
    /// ```
    /// use pmcp_package::{ConfigSlot, SlotType};
    ///
    /// let slot = ConfigSlot::new(SlotType::Endpoint {
    ///     name: "TFL_BASE_URL".to_string(),
    ///     tested_value: "https://api.tfl.gov.uk".to_string(),
    /// })
    /// .with_config_key("backend.base_url");
    /// assert_eq!(slot.config_key.as_deref(), Some("backend.base_url"));
    /// ```
    #[must_use]
    pub fn with_config_key(mut self, key: impl Into<String>) -> Self {
        self.config_key = Some(key.into());
        self
    }

    /// Declare who fills this slot. See [`SuppliedBy`].
    #[must_use]
    pub fn with_supplied_by(mut self, supplied_by: SuppliedBy) -> Self {
        self.supplied_by = supplied_by;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::Secret {
            name: "LICHESS_API_KEY".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "secret");
        assert_eq!(json["name"], "LICHESS_API_KEY");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn oauth_client_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::OauthClient {
            name: "primary-oauth".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "oauth_client");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn channel_binding_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::ChannelBinding {
            name: "notify-channel".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "channel_binding");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn human_role_round_trips_with_all_fields_and_no_identity_field() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: "Approves budget overrides".to_string(),
            responsibilities: vec!["review".to_string(), "approve".to_string()],
            channel_hints: vec!["slack".to_string()],
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "human_role");
        assert_eq!(json["role"], "approver");
        assert!(json.get("userId").is_none());
        assert!(json.get("channelId").is_none());
        assert!(json.get("email").is_none());
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn llm_provider_round_trips_with_tested_value() {
        let slot = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "llm_provider");
        assert_eq!(json["tested_value"], "anthropic");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn budget_override_round_trips_with_tested_value() {
        let slot = SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "1000".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "budget_override");
        assert_eq!(json["tested_value"], "1000");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    /// Compile-documented proof: constructing `Secret` requires — and permits — only a
    /// `name` field. If a future contributor added a `value`/`secret`/`credential` field to
    /// this variant, this call site (and every other Secret construction in this crate) would
    /// fail to compile until updated, making the structural guarantee impossible to silently
    /// erode.
    #[test]
    fn secret_variant_constructs_with_only_a_name_field() {
        let _ = SlotType::Secret {
            name: "X".to_string(),
        };
    }

    #[test]
    fn key_uses_role_as_identifying_field_for_human_role() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: String::new(),
            responsibilities: vec![],
            channel_hints: vec![],
        };
        assert_eq!(slot.key(), ("human_role", "approver"));
    }

    #[test]
    fn tested_value_is_none_for_identity_bearing_variants() {
        let slot = SlotType::Secret {
            name: "X".to_string(),
        };
        assert_eq!(slot.tested_value(), None);
    }

    #[test]
    fn tested_value_is_some_for_behavior_relevant_variants() {
        let slot = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        assert_eq!(slot.tested_value(), Some("anthropic"));
    }

    #[test]
    fn endpoint_round_trips_with_tested_value() {
        let slot = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "endpoint");
        assert_eq!(json["name"], "backend.base_url");
        assert_eq!(json["tested_value"], "https://api.tfl.gov.uk");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn auth_mode_round_trips_with_tested_value() {
        let slot = SlotType::AuthMode {
            name: "backend.auth.type".to_string(),
            tested_value: "api_key".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "auth_mode");
        assert_eq!(json["name"], "backend.auth.type");
        assert_eq!(json["tested_value"], "api_key");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn key_uses_endpoint_and_auth_mode_discriminators() {
        let endpoint = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        };
        assert_eq!(endpoint.key(), ("endpoint", "backend.base_url"));
        let auth_mode = SlotType::AuthMode {
            name: "backend.auth.type".to_string(),
            tested_value: "api_key".to_string(),
        };
        assert_eq!(auth_mode.key(), ("auth_mode", "backend.auth.type"));
    }

    #[test]
    fn config_slot_without_a_key_emits_no_config_key_field_at_all() {
        let slot = ConfigSlot::new(SlotType::Secret {
            name: "X".to_string(),
        });
        assert_eq!(slot.config_key, None);
        let json = serde_json::to_value(&slot).unwrap();
        // `skip_serializing_if` — the key is ABSENT, not `null`. This is what keeps the
        // checked-in golden fixtures and all four pinned digests byte-identical.
        assert!(json.get("config_key").is_none());
        assert_eq!(
            json,
            serde_json::json!({"slot": {"type": "secret", "name": "X"}})
        );
    }

    #[test]
    fn config_slot_with_a_key_serializes_it_and_round_trips() {
        let slot = ConfigSlot::new(SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url");
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["config_key"], "backend.base_url");
        let round: ConfigSlot = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn legacy_config_slot_json_without_config_key_deserializes_to_none() {
        // The exact shape written before the field existed (see
        // tests/golden_fixtures/server_team_fs_v1.json).
        let legacy = serde_json::json!({"slot": {"type": "secret", "name": "X"}});
        let slot: ConfigSlot = serde_json::from_value(legacy).unwrap();
        assert_eq!(slot.config_key, None);
        assert_eq!(
            slot.slot,
            SlotType::Secret {
                name: "X".to_string()
            }
        );
    }

    #[test]
    fn tested_value_is_some_for_the_two_new_behavior_relevant_variants() {
        let endpoint = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        };
        assert_eq!(endpoint.tested_value(), Some("https://api.tfl.gov.uk"));
        let auth_mode = SlotType::AuthMode {
            name: "backend.auth.type".to_string(),
            tested_value: "api_key".to_string(),
        };
        assert_eq!(auth_mode.tested_value(), Some("api_key"));
    }
}
