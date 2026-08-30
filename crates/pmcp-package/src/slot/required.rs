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
use crate::slot::types::{ConfigSlot, SlotType, SuppliedBy};

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

/// Every declared slot, classified and ordered — including the ones no operator
/// fills.
///
/// [`required_slots`] is this list minus the host- and runtime-supplied entries;
/// it is *defined* as such rather than filtering independently, so the two can
/// never disagree about which group a slot belongs to.
///
/// This exists because the excluded slots still have to be SHOWN. A slot that
/// nobody is asked to fill and that nothing renders is invisible, and
/// invisibility is the failure mode [`SuppliedBy`] was introduced to prevent. A
/// renderer takes this one list and splits it on
/// [`SuppliedBy::is_operator_supplied`], so every slot lands in exactly one
/// section by construction — there is no second filter that could drift out of
/// step with the first.
///
/// Ordering matches `required_slots`: stable sort by [`SlotType::key`].
#[must_use]
pub fn classify_slots(slots: &[ConfigSlot]) -> Vec<ClassifiedSlot> {
    let mut all: Vec<ClassifiedSlot> = slots
        .iter()
        .map(|entry| ClassifiedSlot {
            slot: entry.slot.clone(),
            class: classify(&entry.slot),
            config_key: entry.config_key.clone(),
            supplied_by: entry.supplied_by,
        })
        .collect();
    // Stable sort: duplicates (equal keys) keep their relative input order.
    all.sort_by(|a, b| a.slot.key().cmp(&b.slot.key()));
    all
}

/// One declared slot, classified, carrying who supplies it.
///
/// The superset element type: [`RequiredSlot`] is the operator-supplied
/// projection of this. `#[non_exhaustive]` for the same reason `RequiredSlot`
/// is — [`class`](Self::class) is DERIVED by [`classify`], never chosen, and a
/// caller-built value whose `class` disagreed with its `slot` would mislead
/// every consumer that branches on `.class`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ClassifiedSlot {
    /// The slot's typed declaration.
    pub slot: SlotType,
    /// Which family the slot belongs to, derived from [`classify`].
    pub class: SlotClass,
    /// The dotted TOML path in the server's own `config.toml` this slot fills.
    pub config_key: Option<String>,
    /// Who fills this slot's value.
    pub supplied_by: SuppliedBy,
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
    // R1.1: this is the enumerator of what a TARGET ENVIRONMENT must supply, so
    // a value the host or the runtime injects does not belong in it.
    //
    // Defined as a projection of `classify_slots` rather than as its own filter
    // over `slots`, so the "required" and "host-supplied" groups are complements
    // by construction. The excluded slots are NOT hidden: `package load`/`pull`
    // render them in their own labelled section, built by splitting that same
    // one list. (`package inspect` shows a count over the RAW `config_slots` and
    // is unaffected by this filter; `package show` renders the raw list flat.)
    // A slot no operator must fill is near-invisible, and near-invisibility is
    // the failure mode this whole vocabulary exists to prevent.
    //
    // Already ordered by `classify_slots`; filtering preserves that order.
    classify_slots(slots)
        .into_iter()
        .filter(|entry| entry.supplied_by.is_operator_supplied())
        .map(|entry| RequiredSlot {
            slot: entry.slot,
            class: entry.class,
            config_key: entry.config_key,
        })
        .collect()
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

    /// R1.3: `required_slots` is the operator-supplied PROJECTION of
    /// `classify_slots`, so the two agree on membership by construction.
    ///
    /// Asserts the split is a genuine partition — every declared slot lands in
    /// exactly one group, none in both, none in neither. A slot in neither is
    /// the invisibility bug `SuppliedBy` exists to prevent; a slot in both would
    /// have an operator filling a value the host also injects.
    #[test]
    fn classify_and_required_partition_every_slot_exactly_once() {
        let slots = vec![
            secret(),
            endpoint().with_supplied_by(SuppliedBy::Platform),
            ConfigSlot::new(SlotType::OauthClient {
                name: "primary-oauth".to_string(),
            })
            .with_supplied_by(SuppliedBy::Runtime),
        ];

        let all = classify_slots(&slots);
        let required = required_slots(&slots);
        let host_supplied: Vec<_> = all
            .iter()
            .filter(|e| !e.supplied_by.is_operator_supplied())
            .collect();

        // Total coverage: nothing falls out of both groups.
        assert_eq!(all.len(), slots.len());
        assert_eq!(required.len() + host_supplied.len(), slots.len());

        // Disjoint: no key appears in both groups.
        for entry in &required {
            assert!(
                !host_supplied
                    .iter()
                    .any(|h| h.slot.key() == entry.slot.key()),
                "{:?} appears in BOTH groups",
                entry.slot.key()
            );
        }

        assert_eq!(required.len(), 1);
        assert_eq!(required[0].slot.key(), ("secret", "TFL_APP_KEY"));
    }

    /// The projection must preserve the classification, not recompute it — a
    /// `RequiredSlot` built from a `ClassifiedSlot` carries that entry's own
    /// `class` and `config_key`, so the two views can never describe the same
    /// slot differently.
    #[test]
    fn the_projection_preserves_class_and_config_key() {
        let slots = vec![secret(), endpoint()];
        let all = classify_slots(&slots);
        let required = required_slots(&slots);

        assert_eq!(required.len(), all.len());
        for (projected, full) in required.iter().zip(all.iter()) {
            assert_eq!(projected.slot, full.slot);
            assert_eq!(projected.class, full.class);
            assert_eq!(projected.config_key, full.config_key);
        }
    }

    /// `supplied_by` is ORTHOGONAL to `kind`: a platform-supplied endpoint is
    /// still an endpoint and still behavior-relevant. Collapsing the two axes
    /// would re-hide exactly what `detect_deviation` exists to see.
    #[test]
    fn supplied_by_does_not_change_a_slots_class() {
        let operator = classify_slots(&[endpoint()]);
        let platform = classify_slots(&[endpoint().with_supplied_by(SuppliedBy::Platform)]);

        assert_eq!(operator[0].class, platform[0].class);
        assert_eq!(operator[0].slot.key(), platform[0].slot.key());
        assert_ne!(operator[0].supplied_by, platform[0].supplied_by);
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
