//! The inventory of slots a target environment must fill.
//!
//! Contrast with [`detect_deviation`](crate::slot::deviation::detect_deviation), which is
//! deliberately NARROWER: its contract is "this behavior-relevant value differs from what was
//! tested", and it returns `None` for every identity-bearing slot by design (a binding
//! supplies identity, not behavior — see its `never_flags_identity_bearing_slots` invariant).
//! That makes it structurally incapable of naming a credential slot, so it can never be the
//! thing that answers "what must the target environment supply?".
//!
//! [`required_slots`] answers that question, returning BOTH families. It is a separate
//! function rather than a widening of `detect_deviation` precisely because widening would
//! break that invariant, which is the guarantee that a deviation report never doubles as a
//! credential inventory.

use crate::slot::classification::{classify, SlotClass};
use crate::slot::types::{ConfigSlot, SlotType};

/// One slot a target environment must supply a value for, with the family it belongs to and
/// the dotted TOML config path it fills (if any).
///
/// `#[non_exhaustive]`: [`class`](Self::class) is DERIVED from
/// [`slot`](Self::slot) by [`classify`], never chosen. Leaving the struct
/// literal-constructible from outside the crate would let a caller build a
/// `RequiredSlot` whose `class` disagrees with its `slot` — e.g. a `Secret`
/// labelled `BehaviorRelevant` — which every consumer that branches on `.class`
/// (rather than re-running `classify`) would then act on. Construct these by
/// calling [`required_slots`]; read them by field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequiredSlot {
    /// The slot's typed declaration. For an identity-bearing variant this names the slot
    /// only — no resolved secret value is representable in the type.
    pub slot: SlotType,
    /// Which family the slot belongs to, derived from [`classify`] rather than a
    /// hand-maintained list, so this can never disagree with `SlotType::tested_value`.
    pub class: SlotClass,
    /// The dotted TOML path in the server's own `config.toml` this slot fills, copied from
    /// [`ConfigSlot::config_key`]. `None` for a slot that fills no config key.
    pub config_key: Option<String>,
}

/// Enumerate every slot a target environment must fill, in deterministic order.
///
/// Returns BOTH families — identity-bearing (credentials, OAuth clients, channel bindings,
/// human roles) and behavior-relevant (LLM provider, budget override, endpoint, auth mode).
/// The `class` of each is derived from [`classify`], never from a list maintained here.
///
/// # Ordering
///
/// Output is ordered by [`SlotType::key`] — the same `(kind, name)` tuple
/// [`aggregate`](crate::slot::aggregate::aggregate) dedups on, so the two functions cannot
/// disagree about what makes two slots the same slot. The sort is stable, so entries sharing
/// a key retain their relative input order.
///
/// # Duplicates are PRESERVED, not resolved
///
/// The input is expected to be an already-`aggregate`-normalized slot set. Passing an
/// un-normalized set returns duplicate entries rather than resolving them: two `ConfigSlot`s
/// with an equal `SlotType::key()` yield two `RequiredSlot`s. This is deliberate — despite
/// the name reading like an actionable required-input inventory, `required_slots` is a pure
/// projection and `aggregate` owns dedup and conflict policy. Two rejected alternatives:
///
/// - **Return `Result` by calling `aggregate` internally** — couples a pure projection to
///   conflict policy and hands the caller two error surfaces for one operation.
/// - **Silently dedup** — hides a genuine double-declaration from the caller, and would let
///   this function and `aggregate` disagree about a conflicting duplicate.
///
/// # Examples
///
/// Note that each slot's `name` is the ENVIRONMENT VARIABLE the target
/// environment sets, while `config_key` is the dotted CONFIG PATH the resolved
/// value is written to — see [`ConfigSlot::config_key`]. They are never the
/// same string, and putting the config path in `name` produces a slot whose
/// derived variable (`BACKEND.BASE_URL`) no environment can portably set.
///
/// ```
/// use pmcp_package::{required_slots, ConfigSlot, SlotClass, SlotType};
///
/// let slots = vec![
///     ConfigSlot::new(SlotType::Secret {
///         name: "TFL_APP_KEY".to_string(),
///     })
///     .with_config_key("backend.auth.query_params.app_key"),
///     ConfigSlot::new(SlotType::Endpoint {
///         name: "TFL_BASE_URL".to_string(),
///         tested_value: "https://api.tfl.gov.uk".to_string(),
///     })
///     .with_config_key("backend.base_url"),
/// ];
///
/// let required = required_slots(&slots);
/// assert_eq!(required.len(), 2);
/// // Ordered by `SlotType::key()`: "endpoint" sorts before "secret".
/// assert_eq!(required[0].class, SlotClass::BehaviorRelevant);
/// assert_eq!(required[0].config_key.as_deref(), Some("backend.base_url"));
/// // The credential IS enumerated here — `detect_deviation` could never name it.
/// assert_eq!(required[1].class, SlotClass::IdentityBearing);
/// ```
#[must_use]
pub fn required_slots(slots: &[ConfigSlot]) -> Vec<RequiredSlot> {
    let mut required: Vec<RequiredSlot> = slots
        .iter()
        // R1.1: this is the enumerator of what a TARGET ENVIRONMENT must supply,
        // so a value the host or the runtime injects does not belong in it. The
        // excluded slots are NOT hidden — `package inspect`/`load` render them
        // in their own labelled section, because a slot no operator must fill is
        // near-invisible and near-invisibility is the failure mode this whole
        // vocabulary exists to prevent.
        .filter(|entry| entry.supplied_by.is_operator_supplied())
        .map(|entry| RequiredSlot {
            slot: entry.slot.clone(),
            class: classify(&entry.slot),
            config_key: entry.config_key.clone(),
        })
        .collect();
    // Stable sort: duplicates (equal keys) keep their relative input order rather than
    // being reordered arbitrarily.
    required.sort_by(|a, b| a.slot.key().cmp(&b.slot.key()));
    required
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot::deviation::detect_deviation;
    use proptest::prelude::*;

    fn secret() -> ConfigSlot {
        ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key")
    }

    // `name` is the ENVIRONMENT VARIABLE, `config_key` the CONFIG PATH — the two
    // are deliberately different strings here, matching the real london-tube
    // fixture (`name = "TFL_BASE_URL"`, `key = "backend.base_url"`) rather than
    // the config-path-in-name shape that would derive an unsettable variable.
    fn endpoint() -> ConfigSlot {
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url")
    }

    fn auth_mode() -> ConfigSlot {
        ConfigSlot::new(SlotType::AuthMode {
            name: "backend-auth-mode".to_string(),
            tested_value: "api_key".to_string(),
        })
        .with_config_key("backend.auth.type")
    }

    /// Test 1: both families are returned — the whole reason this is not `detect_deviation`.
    #[test]
    fn returns_both_identity_bearing_and_behavior_relevant_slots() {
        let slots = vec![secret(), endpoint(), auth_mode()];
        let required = required_slots(&slots);
        assert_eq!(required.len(), 3);
        let kinds: Vec<&str> = required.iter().map(|r| r.slot.key().0).collect();
        assert_eq!(kinds, vec!["auth_mode", "endpoint", "secret"]);
    }

    /// Test 2: each entry carries the right family and the slot's config key.
    #[test]
    fn each_required_slot_carries_its_class_and_config_key() {
        let slots = vec![secret(), endpoint(), auth_mode()];
        let required = required_slots(&slots);

        assert_eq!(required[0].class, SlotClass::BehaviorRelevant);
        assert_eq!(required[0].config_key.as_deref(), Some("backend.auth.type"));
        assert_eq!(required[1].class, SlotClass::BehaviorRelevant);
        assert_eq!(required[1].config_key.as_deref(), Some("backend.base_url"));
        assert_eq!(required[2].class, SlotClass::IdentityBearing);
        assert_eq!(
            required[2].config_key.as_deref(),
            Some("backend.auth.query_params.app_key")
        );
    }

    /// Test 4: an empty slot set is an empty inventory, not an error.
    #[test]
    fn empty_slot_set_returns_an_empty_inventory() {
        assert_eq!(required_slots(&[]), vec![]);
    }

    /// Test 5 (contrast guard, strong form): the credential IS in the required inventory,
    /// while `detect_deviation` cannot name it under ANY pairing — including the pairing a
    /// naive reader would expect to produce a deviation, two `Secret`s with DIFFERENT names.
    /// Pairing a slot against a clone of itself would prove nothing: equal behavior-relevant
    /// slots also return `None`, so that test would pass even if the identity-bearing
    /// short-circuit did not exist.
    #[test]
    fn required_slots_enumerates_a_credential_that_detect_deviation_never_can() {
        let a = SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        };
        let b = SlotType::Secret {
            name: "TFL_APP_KEY_ROTATED".to_string(),
        };
        // Two DIFFERENT secrets — still `None`, because identity is never behavior (D-03).
        assert!(detect_deviation(&a, &b).is_none());
        assert!(detect_deviation(&b, &a).is_none());

        // The same credential slot is nonetheless enumerated as required.
        let required = required_slots(&[secret()]);
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].slot.key(), ("secret", "TFL_APP_KEY"));
        assert_eq!(required[0].class, SlotClass::IdentityBearing);
    }

    /// Test 6: duplicate keys are PRESERVED, not deduped — `aggregate` owns dedup, and this
    /// function is a projection. Without this test the no-dedup choice would be an unstated
    /// assumption a caller could reasonably read the other way.
    #[test]
    fn duplicate_keys_are_preserved_rather_than_deduped() {
        let slots = vec![secret(), secret()];
        let required = required_slots(&slots);
        assert_eq!(required.len(), 2);
        assert_eq!(required[0].slot.key(), required[1].slot.key());
    }

    proptest! {
        /// Test 3: the returned order is deterministic and independent of input order.
        /// Draws from the full eight-variant space (distinct keys) so an ordering bug that
        /// only appears when kinds interleave is visible.
        #[test]
        fn ordering_is_stable_under_permutation(seed in proptest::collection::vec(0u32..1000, 8)) {
            let slots: Vec<ConfigSlot> = vec![
                ConfigSlot::new(SlotType::LlmProvider {
                    name: "primary-llm".to_string(),
                    tested_value: "anthropic".to_string(),
                }),
                secret(),
                auth_mode(),
                ConfigSlot::new(SlotType::HumanRole {
                    role: "approver".to_string(),
                    description: "Approves budget overrides".to_string(),
                    responsibilities: vec!["review".to_string()],
                    channel_hints: vec!["slack".to_string()],
                }),
                endpoint(),
                ConfigSlot::new(SlotType::OauthClient {
                    name: "primary-oauth".to_string(),
                }),
                ConfigSlot::new(SlotType::BudgetOverride {
                    name: "monthly-cap".to_string(),
                    tested_value: "1000".to_string(),
                }),
                ConfigSlot::new(SlotType::ChannelBinding {
                    name: "notify-channel".to_string(),
                }),
            ];

            let mut indices: Vec<usize> = (0..8).collect();
            indices.sort_by_key(|&i| seed[i]);
            let permuted: Vec<ConfigSlot> = indices.iter().map(|&i| slots[i].clone()).collect();

            let baseline = required_slots(&slots);
            let shuffled = required_slots(&permuted);
            prop_assert_eq!(baseline.len(), 8);
            prop_assert_eq!(baseline, shuffled);
        }
    }
}
