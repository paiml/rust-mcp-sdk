//! Identity-bearing vs behavior-relevant slot classification (/ §3.5).

use crate::slot::types::SlotType;

/// Which of the two slot families a `SlotType` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotClass {
    /// `Secret` / `OauthClient` / `ChannelBinding` / `HumanRole` — declares only identity,
    /// never a value; never subject to deviation detection (binding identity is not
    /// behavior).
    IdentityBearing,
    /// `LlmProvider` / `BudgetOverride` — carries a `tested_value`; a differing proposed
    /// value is a real behavioral change, surfaced by `deviation::detect_deviation`.
    BehaviorRelevant,
}

/// Classify a `SlotType` into its family. Pure, no I/O.
///
/// The identity/behavior split has a single source of truth: a variant is
/// behavior-relevant iff it carries a `tested_value` (see
/// [`SlotType::tested_value`]). Deriving the class from that predicate keeps
/// `classify` and `tested_value` from drifting apart when a new variant is
/// added.
pub fn classify(slot: &SlotType) -> SlotClass {
    if slot.tested_value().is_some() {
        SlotClass::BehaviorRelevant
    } else {
        SlotClass::IdentityBearing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_identity_bearing() {
        let slot = SlotType::Secret {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn oauth_client_is_identity_bearing() {
        let slot = SlotType::OauthClient {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn channel_binding_is_identity_bearing() {
        let slot = SlotType::ChannelBinding {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn human_role_is_identity_bearing() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: String::new(),
            responsibilities: vec![],
            channel_hints: vec![],
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn llm_provider_is_behavior_relevant() {
        let slot = SlotType::LlmProvider {
            name: "n".to_string(),
            tested_value: "anthropic".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }

    #[test]
    fn budget_override_is_behavior_relevant() {
        let slot = SlotType::BudgetOverride {
            name: "n".to_string(),
            tested_value: "1000".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }

    #[test]
    fn endpoint_is_behavior_relevant() {
        let slot = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }

    #[test]
    fn auth_mode_is_behavior_relevant() {
        let slot = SlotType::AuthMode {
            name: "backend.auth.type".to_string(),
            tested_value: "api_key".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }

    /// D-03 regression guard for the two variants added in phase 120: they must have
    /// landed in the BEHAVIOR-RELEVANT family, which is observable only through the
    /// downstream consequence. `detect_deviation` still returns `None` for equal
    /// identity-bearing slots (its narrower, deliberately-unchanged contract), and now
    /// returns `Some(..)` for two `Endpoint`s whose `tested_value`s differ. Had the
    /// `tested_value()` catch-all survived, `Endpoint` would classify as
    /// `IdentityBearing`, this assertion would fail, and every other test would stay green.
    #[test]
    fn new_variants_are_behavior_relevant_as_observed_through_detect_deviation() {
        use crate::slot::deviation::detect_deviation;

        let secret = SlotType::Secret {
            name: "TFL_API_KEY".to_string(),
        };
        assert_eq!(detect_deviation(&secret, &secret.clone()), None);

        let tested = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        };
        let proposed = SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://staging.tfl.gov.uk".to_string(),
        };
        let dev = detect_deviation(&tested, &proposed)
            .expect("Endpoint must be behavior-relevant, so a differing value is a deviation");
        assert_eq!(dev.slot_name, "backend.base_url");
        assert_eq!(dev.tested, "https://api.tfl.gov.uk");
        assert_eq!(dev.proposed, "https://staging.tfl.gov.uk");
    }
}
