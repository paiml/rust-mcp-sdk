//! Deterministic dedup + conflict-erroring aggregation of config slots across a component
//! graph (/).
//!
//! The caller supplies the walk (e.g. `components.iter().flat_map(|c| c.slots())`); this
//! module provides the dedup + conflict check. Ordering is via `BTreeMap` (never `HashMap`)
//! so the aggregated `Vec` is stable across runs — required for digest + manifest-diff.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crate::error::{PackageError, Result};
use crate::slot::types::ConfigSlot;

/// Aggregate a flat iterator of `ConfigSlot`s into one deduplicated entry per distinct
/// `(kind, name)` slot (see `SlotType::key`), in stable deterministic order.
///
/// - Two components declaring a byte-equal slot dedup silently into one entry.
/// - Two components declaring the SAME slot (same kind+name) with DIFFERENT
///   `config_key`s return `Err(PackageError::ConfigSlotViolation)` — silently
///   keeping whichever arrived first would make the aggregated output (and any
///   digest or pack-time agreement check over it) depend on input order, and
///   could drop a declared config path.
/// - Two components declaring the SAME behavior-relevant slot (same kind+name) with
///   DIFFERENT `tested_value`s return `Err(PackageError::SlotConflict)` — silently
///   discarding one tested value would mask a real behavioral difference.
/// - Identity-bearing collisions with equal declaration fields dedup silently (identity
///   slots have no tested value to conflict over). Ones with UNEQUAL declaration fields —
///   two `HumanRole`s sharing a `role` but differing in `description`,
///   `responsibilities` or `channel_hints` — return
///   `Err(PackageError::ConfigSlotViolation)` for the same order-independence reason as
///   the `config_key` rule above: `tested_value()` is `None` for every identity-bearing
///   variant, so the `SlotConflict` check cannot see them and first-wins would make the
///   output depend on walk order.
pub fn aggregate<'a>(slots: impl IntoIterator<Item = &'a ConfigSlot>) -> Result<Vec<ConfigSlot>> {
    // Key borrows the slot's name (`key()` returns `&str`) — no per-slot key
    // allocation. `entry` does a single lookup instead of get-then-insert.
    let mut map: BTreeMap<(&'static str, &'a str), ConfigSlot> = BTreeMap::new();
    for slot in slots {
        let key = slot.slot.key();
        match map.entry(key) {
            Entry::Vacant(e) => {
                e.insert(slot.clone());
            },
            // Every collision policy lives in one function so this loop stays a
            // loop; see `reconcile_collision` for the three cases and why each
            // one is what it is.
            Entry::Occupied(e) => reconcile_collision(key.1, e.get(), slot)?,
        }
    }
    Ok(map.into_values().collect())
}

/// Decide what a `(kind, name)` collision means: silent dedup, or which typed
/// error.
///
/// The three cases, in the order they are checked:
///
/// 1. **Different `config_key`** — `ConfigSlotViolation`. First-wins would
///    silently discard a declared config path, and WHICH path survived would
///    depend on which component was walked first, violating this module's
///    permutation-stability contract. Checked first because the byte-equal arm
///    below can never fire for it (equal `ConfigSlot`s have equal
///    `config_key`s), so the order costs nothing and makes the message specific.
/// 2. **Byte-equal declaration** — pure dedup, keep the one already present.
/// 3. **Anything else** — the declarations differ. A differing `tested_value`
///    is the named, typed case (`SlotConflict`), because it is a real
///    behavioral difference and the error can quote both values. What remains
///    is an identity-bearing slot whose NON-value fields disagree — two
///    `HumanRole`s sharing a `role` with different
///    `description`/`responsibilities`/`channel_hints` is the live case, since
///    `tested_value()` is `None` for every identity-bearing variant so the
///    `SlotConflict` check structurally cannot see it. Falling through there
///    would be first-wins again, so it is a `ConfigSlotViolation` for the same
///    order-independence reason as case 1. Its values are NOT echoed: an
///    identity-bearing slot's fields are the one place a credential-adjacent
///    string could sit.
///
/// # Errors
///
/// [`PackageError::ConfigSlotViolation`] for cases 1 and 3,
/// [`PackageError::SlotConflict`] for a differing `tested_value`.
fn reconcile_collision(name: &str, existing: &ConfigSlot, incoming: &ConfigSlot) -> Result<()> {
    if existing.supplied_by != incoming.supplied_by {
        return Err(PackageError::ConfigSlotViolation {
            key: name.to_string(),
            reason: "declared with two different `supplied_by` values by different components; \
                     since `required_slots` enumerates only environment-supplied slots, \
                     resolving this by input order would either ask an operator for a value the \
                     host injects or fail to ask for one nobody supplies — both silent"
                .to_string(),
        });
    }
    if existing.config_key != incoming.config_key {
        return Err(PackageError::ConfigSlotViolation {
            key: name.to_string(),
            reason: "declared with two different `config_key` values by different components; \
                     one slot fills one config path, so this must be reconciled rather than \
                     silently resolved by input order"
                .to_string(),
        });
    }
    if existing.slot == incoming.slot {
        return Ok(());
    }
    if let (Some(tested), Some(proposed)) =
        (existing.slot.tested_value(), incoming.slot.tested_value())
    {
        if tested != proposed {
            return Err(PackageError::SlotConflict {
                slot: name.to_string(),
                tested: tested.to_string(),
                proposed: proposed.to_string(),
            });
        }
    }
    Err(PackageError::ConfigSlotViolation {
        key: name.to_string(),
        reason: "declared twice with different declaration fields by different components; one \
                 slot is one declaration, so this must be reconciled rather than silently \
                 resolved by input order"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot::types::SlotType;
    use proptest::prelude::*;

    #[test]
    fn dedup_two_identical_secrets_into_one_entry() {
        let a = ConfigSlot::new(SlotType::Secret {
            name: "LICHESS_API_KEY".to_string(),
        });
        let b = a.clone();
        let result = aggregate([&a, &b]).unwrap();
        assert_eq!(result, vec![a]);
    }

    #[test]
    fn conflicting_tested_values_return_slot_conflict_error() {
        let a = ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        });
        let b = ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "openai".to_string(),
        });
        let err = aggregate([&a, &b]).unwrap_err();
        assert!(matches!(
                    err,
                    PackageError::SlotConflict {
                        tested,
                        proposed,
        ..
                    } if tested == "anthropic" && proposed == "openai"
               ));
    }

    #[test]
    fn same_slot_with_two_different_config_keys_errors_in_either_order() {
        // Phase 120 regression: dedup used to compare only the SlotType, so two
        // slots identical except for `config_key` deduped FIRST-WINS — the
        // surviving config path depended on input order, violating the
        // permutation-stability contract the module doc states.
        let keyed = ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key");
        let rekeyed = ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.headers.x-api-key");

        for pair in [[&keyed, &rekeyed], [&rekeyed, &keyed]] {
            let err = aggregate(pair).unwrap_err();
            assert!(
                matches!(
                    &err,
                    PackageError::ConfigSlotViolation { key, .. } if key == "TFL_APP_KEY"
                ),
                "differing config_keys must be a ConfigSlotViolation naming the slot, got: {err:?}"
            );
        }
    }

    #[test]
    fn same_slot_keyed_and_unkeyed_errors_rather_than_first_wins() {
        let keyed = ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key");
        let unkeyed = ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        });

        // Whichever order the components are walked in, the outcome is the same
        // error — never a silent choice of "keyed" or "unkeyed".
        assert!(aggregate([&keyed, &unkeyed]).is_err());
        assert!(aggregate([&unkeyed, &keyed]).is_err());
    }

    #[test]
    fn identity_bearing_slots_differing_in_non_value_fields_error_in_either_order() {
        // `tested_value()` is `None` for every identity-bearing variant, so the
        // SlotConflict check cannot see this pair. Before the guard below existed
        // it fell through to first-wins and the aggregated Vec depended on which
        // component was walked first.
        let approver = |description: &str| {
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: description.to_string(),
                responsibilities: vec!["review".to_string()],
                channel_hints: vec!["slack".to_string()],
            })
        };
        let a = approver("Approves budget overrides");
        let b = approver("Signs off on spend");

        for pair in [[&a, &b], [&b, &a]] {
            let err = aggregate(pair).unwrap_err();
            assert!(
                matches!(
                    &err,
                    PackageError::ConfigSlotViolation { key, .. } if key == "approver"
                ),
                "differing identity-bearing declarations must be a ConfigSlotViolation naming \
                 the slot, got: {err:?}"
            );
        }

        // The byte-equal pair still dedups — the guard must not turn dedup into
        // an error.
        assert_eq!(aggregate([&a, &a.clone()]).unwrap(), vec![a]);
    }

    #[test]
    fn identical_slots_with_the_same_config_key_still_dedup() {
        let a = ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key");
        let b = a.clone();
        let result = aggregate([&a, &b]).unwrap();
        assert_eq!(result, vec![a]);
    }

    #[test]
    fn preserves_all_distinct_conflict_free_slots() {
        let a = ConfigSlot::new(SlotType::Secret {
            name: "A".to_string(),
        });
        let b = ConfigSlot::new(SlotType::OauthClient {
            name: "B".to_string(),
        });
        let result = aggregate([&a, &b]).unwrap();
        assert_eq!(result.len(), 2);
    }

    /// One `ConfigSlot` per `SlotType` variant, in a fixed authoring order that is
    /// deliberately NOT the aggregated order — so a permutation property over this set
    /// exercises the full eight-variant space rather than a single-variant slice.
    fn one_slot_per_variant() -> Vec<ConfigSlot> {
        vec![
            ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            }),
            ConfigSlot::new(SlotType::Secret {
                name: "TFL_API_KEY".to_string(),
            }),
            ConfigSlot::new(SlotType::AuthMode {
                name: "backend.auth.type".to_string(),
                tested_value: "api_key".to_string(),
            }),
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: "Approves budget overrides".to_string(),
                responsibilities: vec!["review".to_string()],
                channel_hints: vec!["slack".to_string()],
            }),
            ConfigSlot::new(SlotType::Endpoint {
                name: "backend.base_url".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            }),
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
        ]
    }

    /// Phase 120 Task 1 behavior 6: the two new variants aggregate alongside an
    /// identity-bearing one — all three survive, deduped, in `SlotType::key()` order.
    #[test]
    fn aggregates_secret_endpoint_and_auth_mode_into_deterministic_order() {
        let secret = ConfigSlot::new(SlotType::Secret {
            name: "TFL_API_KEY".to_string(),
        });
        let endpoint = ConfigSlot::new(SlotType::Endpoint {
            name: "backend.base_url".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        });
        let auth_mode = ConfigSlot::new(SlotType::AuthMode {
            name: "backend.auth.type".to_string(),
            tested_value: "api_key".to_string(),
        });

        // Duplicated inputs must dedup rather than multiply.
        let result = aggregate([&secret, &endpoint, &auth_mode, &endpoint.clone()]).unwrap();
        assert_eq!(result.len(), 3);
        let keys: Vec<(&str, &str)> = result.iter().map(|c| c.slot.key()).collect();
        assert_eq!(
            keys,
            vec![
                ("auth_mode", "backend.auth.type"),
                ("endpoint", "backend.base_url"),
                ("secret", "TFL_API_KEY"),
            ]
        );
    }

    proptest! {
        /// /: aggregating any permutation of a conflict-free slot set yields
        /// identical `Vec` output — the aggregated order must never depend on input order
        /// (so the digest stays stable regardless of which component contributed a slot
        /// first).
        #[test]
        fn aggregate_ordering_is_stable_under_permutation(seed in proptest::collection::vec(0u32..1000, 8)) {
            // Draws from the full EIGHT-variant space (both new phase-120 variants
            // included), not a single-variant slice: an ordering bug that only shows up
            // when kinds interleave would be invisible to a Secret-only generator.
            let slots: Vec<ConfigSlot> = one_slot_per_variant();
            prop_assert_eq!(slots.len(), 8);

            let mut indices: Vec<usize> = (0..8).collect();
            indices.sort_by_key(|&i| seed[i]);
            let permuted: Vec<&ConfigSlot> = indices.iter().map(|&i| &slots[i]).collect();

            let baseline = aggregate(slots.iter()).unwrap();
            let shuffled = aggregate(permuted).unwrap();
            prop_assert_eq!(baseline.len(), 8);
            prop_assert_eq!(baseline, shuffled);
        }
    }
}
