//! Dedicated era PROBES: the wire facts a [`CaseResult`] structurally cannot
//! carry.
//!
//! Ported from `crates/mcp-tester/src/era_observations.rs` (Phase 117) under
//! Phase 118 **D-16**. D-07 already said *"reuse, do not reinvent"*; this module
//! transfers the shipped machinery one crate over rather than rebuilding it.
//!
//! # Why this module exists
//!
//! MEASURED at `crates/pmcp-team-servers/src/conformance/runner.rs:306`:
//! [`CaseResult`](crate::conformance::runner::CaseResult) is exactly
//! `{case_id, passed, detail}`, and
//! [`ConformanceReport`](crate::conformance::runner::ConformanceReport) is
//! `{passed, failed, cases}`.
//!
//! Now read `crates/pmcp-team-servers/baselines/era-deltas.yaml`. Most of its
//! fourteen entries are WIRE FACTS — a response header's presence, a session id,
//! the `Last-Event-ID` resumability surface, a result-envelope key, an HTTP
//! status mapping, a `_meta` key's effect. Not one of those is representable in
//! the two structs above. `detail` is free-form prose, and it is populated only
//! when a case FAILS.
//!
//! So an era comparison built by replaying `run_fixtures` twice and diffing the
//! results could not observe most of the baseline at all. It would report
//! permanent false MISSING findings for every wire fact no `case_id` encodes.
//! And — the decisive point, and the reason D-16 exists — **two eras both
//! passing the same expected response emit NO OBSERVATION AT ALL**: `passed:
//! true` on both sides is indistinguishable from `passed: true` on both sides,
//! whatever the wire actually carried. A comparison whose evidence vanishes
//! precisely when both eras behave is not a comparison.
//!
//! Dedicated probes emitting STABLE IDS are what make the evidence observable in
//! the first place. See
//! `.planning/phases/118-conformance-against-the-official-suite/118-REVIEWS.md`
//! for the review consensus that invalidated the pass/fail design, confirmed
//! against this repository's source rather than inferred from plan text.
//!
//! # The shape
//!
//! One probe per baseline `observation_id`, each recording what it saw as an
//! [`ObservedValue`] whose [`ObservedValue::token`] is drawn from the SAME short
//! vocabulary the baseline's `v1:` / `v2:` columns use. That is what lets
//! [`crate::conformance::era_diff`] join an observation against a delta and
//! check not merely that the eras DIFFERED but that they differed in the
//! RECORDED way.
//!
//! Derivation rules are documented on each probe. They are era-INDEPENDENT: a
//! probe reads the wire and maps what it saw to a token, and never consults the
//! era to decide what it "should" have seen. A probe that did would be asserting
//! its own premise.
//!
//! # What this module does NOT contain (yet)
//!
//! The probe BODIES and the `observe()` driver. Those need an HTTP client and a
//! live target; plan 118-06 adds them here. This module is deliberately
//! transport-free: it is pure data plus the registry, so nothing about it
//! depends on how the target is reached.
//!
//! # Ids deliberately NOT ported
//!
//! `crates/mcp-tester/src/era_observations.rs` carries fourteen ids too, but
//! four of them are NOT in this registry, and a future reader must not
//! "restore" them:
//!
//! * `method.tasks_list` and `capability.tasks_location` — the Phase-118 era
//!   target implements no Tasks surface, so BOTH eras would answer identically
//!   and the rows could only ever report MISSING. A row that can only ever be a
//!   false finding trains reviewers to ignore the gate.
//! * `method.resources_subscribe` and `method.subscriptions_listen` — these are
//!   CLIENT-side pmcp behaviours (`reject_if_retired_on_v2`,
//!   `src/client/mod.rs:727`) or capability-gated `-32601` in BOTH eras against
//!   a server that advertises no resources capability. Neither is observable
//!   from the server side of this target.
//!
//! Four ids are NEW here and are the CONF-03 surface, added under **D-17**
//! (which closed off the fixture-format extension that would otherwise have
//! carried them): `method.logging_set_level`, `meta.log_level`,
//! `result.input_required.sampling` and `result.input_required.roots`.
//!
//! The remaining ten strings are REUSED BYTE-IDENTICALLY from the mcp-tester
//! baseline because they name the SAME wire fact. The join key is never renamed
//! for readability, and reusing the string is what lets the two baselines be
//! read side by side.
//!
//! [`CaseResult`]: crate::conformance::runner::CaseResult

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The STABLE, MACHINE-FACING name of one wire fact.
///
/// A newtype over a `&'static str` so an ID can never be built from a runtime
/// display name: the whole point is that these are fixed identifiers, joined
/// against `EraDelta::observation_id`, and never renamed for readability.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_observations::{ObservationId, METHOD_INITIALIZE};
///
/// assert_eq!(METHOD_INITIALIZE.as_str(), "method.initialize");
/// assert_eq!(ObservationId::from_registry("method.initialize"), Some(METHOD_INITIALIZE));
/// // Anything this build has no probe for resolves to None — including the four
/// // ids the module doc records as deliberately not ported.
/// assert_eq!(ObservationId::from_registry("method.no_probe_for_this"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationId(pub &'static str);

impl ObservationId {
    /// The identifier as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::META_LOG_LEVEL;
    ///
    /// assert_eq!(META_LOG_LEVEL.as_str(), "meta.log_level");
    /// ```
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// Resolve a wire string back to a registry id.
    ///
    /// `None` for anything not in [`PROBE_REGISTRY`] — which is the whole point
    /// of the `&'static str` newtype: an id is a fixed identifier this crate
    /// knows, not an arbitrary string a caller invented.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::{ObservationId, RESULT_CACHE_SCOPE};
    ///
    /// assert_eq!(ObservationId::from_registry("result.cache_scope"), Some(RESULT_CACHE_SCOPE));
    /// assert_eq!(ObservationId::from_registry("invented.by.a.caller"), None);
    /// ```
    #[must_use]
    pub fn from_registry(value: &str) -> Option<Self> {
        PROBE_REGISTRY.iter().copied().find(|id| id.0 == value)
    }
}

impl Serialize for ObservationId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for ObservationId {
    /// Deserializing VALIDATES against [`PROBE_REGISTRY`].
    ///
    /// The newtype holds a `&'static str`, so an arbitrary borrowed string
    /// cannot be turned into one; and the validation this forces is a feature,
    /// not a workaround. A stored report naming an id this build has no probe
    /// for is a report this build cannot honestly interpret, and saying so is
    /// better than silently accepting a leaked string.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::from_registry(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "`{raw}` is not a known observation id; this build's probe registry \
                 has no probe for it"
            ))
        })
    }
}

impl std::fmt::Display for ObservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// What a probe SAW.
///
/// Small on purpose — these are the only value kinds the fourteen probes need.
/// [`Self::Unavailable`] is the honest answer when a probe ran but could not
/// establish its fact; it is deliberately NOT the same as [`Self::Absent`],
/// which is a positive observation that something was not there.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_observations::ObservedValue;
///
/// assert_eq!(ObservedValue::Present.token(), "present");
/// assert!(ObservedValue::Absent.is_established());
/// // "we could not tell" is never "we saw nothing".
/// assert!(!ObservedValue::Unavailable("transport failed".into()).is_established());
/// assert_ne!(ObservedValue::Unavailable("x".into()), ObservedValue::Absent);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ObservedValue {
    /// The wire fact was observed PRESENT.
    Present,
    /// The wire fact was observed ABSENT.
    Absent,
    /// A short observed token, in the baseline's own vocabulary.
    ///
    /// This is the only carrier for a status (`status:405`) or a JSON-RPC
    /// refusal (`error:-32601`). Typed `Status(u16)` / `Pointer(String)`
    /// variants existed in the analog and were never constructed by any probe —
    /// and because [`Self::token`] rendered `Status(405)` and
    /// `Text("status:405")` identically, they were two values that compared
    /// EQUAL through the join while being distinct on the wire. Reintroduce a
    /// typed variant only alongside a probe that emits it.
    Text(String),
    /// The probe ran but could not establish the fact; the string says why.
    Unavailable(String),
}

impl ObservedValue {
    /// The canonical token this observation compares as.
    ///
    /// This is what [`crate::conformance::era_diff`] matches against an
    /// `EraDelta`'s `v1:` / `v2:` column, so the vocabulary here and the
    /// vocabulary in `baselines/era-deltas.yaml` are ONE vocabulary.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::ObservedValue;
    ///
    /// assert_eq!(ObservedValue::Text("error:-32601".into()).token(), "error:-32601");
    /// // Every `Unavailable` collapses to one token; the reason is not the value.
    /// assert_eq!(ObservedValue::Unavailable("timeout".into()).token(), "unavailable");
    /// ```
    #[must_use]
    pub fn token(&self) -> String {
        match self {
            Self::Present => "present".to_string(),
            Self::Absent => "absent".to_string(),
            Self::Text(s) => s.clone(),
            Self::Unavailable(_) => "unavailable".to_string(),
        }
    }

    /// Whether the probe actually established its fact.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::ObservedValue;
    ///
    /// assert!(ObservedValue::Present.is_established());
    /// assert!(!ObservedValue::Unavailable("boom".into()).is_established());
    /// ```
    #[must_use]
    pub fn is_established(&self) -> bool {
        !matches!(self, Self::Unavailable(_))
    }
}

/// Everything one era run observed, keyed by [`ObservationId`].
///
/// A `BTreeMap` rather than a `HashMap`: the report is RENDERED and compared as
/// bytes, so iteration order has to be deterministic.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use pmcp_team_servers::conformance::era_observations::{
///     EraObservations, ObservedValue, METHOD_INITIALIZE, RESULT_CACHE_SCOPE,
/// };
///
/// let mut map = BTreeMap::new();
/// map.insert(RESULT_CACHE_SCOPE, ObservedValue::Absent);
/// map.insert(METHOD_INITIALIZE, ObservedValue::Text("served".into()));
/// let observed = EraObservations(map);
///
/// // Sorted, not insertion-ordered — the render is byte-compared.
/// assert_eq!(observed.ids(), vec![METHOD_INITIALIZE, RESULT_CACHE_SCOPE]);
/// assert_eq!(observed.len(), 2);
/// assert!(!observed.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraObservations(pub BTreeMap<ObservationId, ObservedValue>);

impl EraObservations {
    /// Look one observation up.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::{
    ///     EraObservations, METHOD_INITIALIZE,
    /// };
    ///
    /// assert!(EraObservations::default().get(METHOD_INITIALIZE).is_none());
    /// ```
    #[must_use]
    pub fn get(&self, id: ObservationId) -> Option<&ObservedValue> {
        self.0.get(&id)
    }

    /// Number of observations recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// assert_eq!(EraObservations::default().len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing at all was observed.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// assert!(EraObservations::default().is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every id recorded, in sorted order.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// assert!(EraObservations::default().ids().is_empty());
    /// ```
    #[must_use]
    pub fn ids(&self) -> Vec<ObservationId> {
        self.0.keys().copied().collect()
    }
}

// ===========================================================================
// The probe registry.
//
// Ten ids REUSED BYTE-IDENTICALLY from `crates/mcp-tester/baselines/era-deltas.yaml`
// (they name the same wire fact), then four NEW CONF-03 ids (D-17).
// ===========================================================================

/// `method:initialize` — ERA-01. REUSED from the mcp-tester registry.
pub const METHOD_INITIALIZE: ObservationId = ObservationId("method.initialize");
/// `method:server/discover` — ERA-02. REUSED from the mcp-tester registry.
pub const METHOD_SERVER_DISCOVER: ObservationId = ObservationId("method.server_discover");
/// `header:Mcp-Session-Id` — ERA-03. REUSED from the mcp-tester registry.
pub const HEADER_MCP_SESSION_ID: ObservationId = ObservationId("header.mcp_session_id");
/// `header:Mcp-Method,Mcp-Name` — ERA-04. REUSED from the mcp-tester registry.
pub const HEADER_MCP_METHOD_AND_NAME: ObservationId = ObservationId("header.mcp_method_and_name");
/// `header:Last-Event-ID` — ERA-05. REUSED from the mcp-tester registry.
pub const HEADER_LAST_EVENT_ID: ObservationId = ObservationId("header.last_event_id");
/// `http-verb:GET,DELETE` — ERA-06. REUSED from the mcp-tester registry.
pub const HTTP_VERB_GET_DELETE: ObservationId = ObservationId("http.verb.get_delete");
/// `result-field:resultType` — ERA-07. REUSED from the mcp-tester registry.
pub const RESULT_RESULT_TYPE: ObservationId = ObservationId("result.result_type");
/// `result-field:serverInfo` — ERA-08. REUSED from the mcp-tester registry.
pub const RESULT_SERVER_INFO: ObservationId = ObservationId("result.server_info");
/// `result-field:ttlMs,cacheScope` — ERA-09. REUSED from the mcp-tester registry.
pub const RESULT_CACHE_SCOPE: ObservationId = ObservationId("result.cache_scope");
/// `http-status:JSON-RPC error code mapping` — ERA-10. REUSED from the
/// mcp-tester registry.
pub const HTTP_STATUS_ERROR_CODE_MAPPING: ObservationId =
    ObservationId("http.status.error_code_mapping");

/// `method:logging/setLevel` — ERA-11. NEW, CONF-03 (D-12/D-17): the v2 schema
/// REPLACES the RPC with a per-request `_meta` key.
pub const METHOD_LOGGING_SET_LEVEL: ObservationId = ObservationId("method.logging_set_level");
/// `_meta:io.modelcontextprotocol/logLevel` — ERA-12. NEW, CONF-03 (D-12/D-17):
/// the replacement mechanism, honoured on v2 and inert on v1.
pub const META_LOG_LEVEL: ObservationId = ObservationId("meta.log_level");
/// `result-field:inputRequests (sampling)` — ERA-13. NEW, CONF-03 (D-12/D-17):
/// v2's stateless replacement for mid-request `sampling/createMessage`.
pub const RESULT_INPUT_REQUIRED_SAMPLING: ObservationId =
    ObservationId("result.input_required.sampling");
/// `result-field:inputRequests (roots)` — ERA-14. NEW, CONF-03 (D-12/D-17):
/// v2's stateless replacement for mid-request `roots/list`.
pub const RESULT_INPUT_REQUIRED_ROOTS: ObservationId = ObservationId("result.input_required.roots");

/// Every [`ObservationId`] this crate has (or will have) a probe for.
///
/// This is one half of the two-direction coverage contract with
/// `crates/pmcp-team-servers/baselines/era-deltas.yaml`;
/// `crates/pmcp-team-servers/tests/era_baseline.rs` asserts both directions. A
/// baseline entry with no registry id is an entry nothing can ever observe,
/// which would produce a permanent false MISSING finding — exactly the defect
/// the observation-id design removes. A registry id with no baseline entry
/// would report every run of it as an UNEXPECTED finding.
///
/// The REGISTRY, not the file, is the authority: adding a row to the YAML
/// without adding an id here fails
/// `every_baseline_entry_has_a_probe`.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_observations::PROBE_REGISTRY;
///
/// assert_eq!(PROBE_REGISTRY.len(), 14);
/// assert!(PROBE_REGISTRY.iter().any(|id| id.as_str() == "meta.log_level"));
/// ```
pub const PROBE_REGISTRY: &[ObservationId] = &[
    METHOD_INITIALIZE,
    METHOD_SERVER_DISCOVER,
    HEADER_MCP_SESSION_ID,
    HEADER_MCP_METHOD_AND_NAME,
    HEADER_LAST_EVENT_ID,
    HTTP_VERB_GET_DELETE,
    RESULT_RESULT_TYPE,
    RESULT_SERVER_INFO,
    RESULT_CACHE_SCOPE,
    HTTP_STATUS_ERROR_CODE_MAPPING,
    METHOD_LOGGING_SET_LEVEL,
    META_LOG_LEVEL,
    RESULT_INPUT_REQUIRED_SAMPLING,
    RESULT_INPUT_REQUIRED_ROOTS,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A duplicated id would silently merge two distinct wire facts, hiding a
    /// regression under an expected row (T-118-14).
    #[test]
    fn the_registry_has_no_duplicates() {
        let unique: BTreeSet<&str> = PROBE_REGISTRY.iter().map(|id| id.as_str()).collect();
        assert_eq!(
            unique.len(),
            PROBE_REGISTRY.len(),
            "a duplicated ObservationId would silently merge two wire facts"
        );
        assert_eq!(
            PROBE_REGISTRY.len(),
            14,
            "the registry holds exactly fourteen ids: ten reused from mcp-tester \
             plus four new CONF-03 ids"
        );
    }

    /// The ten reused strings are BYTE-IDENTICAL to their mcp-tester siblings.
    /// A rename "for readability" silently stops the two baselines from being
    /// readable side by side and stops the join from matching.
    #[test]
    fn the_ten_reused_ids_are_byte_identical_to_their_mcp_tester_siblings() {
        let reused = [
            "method.initialize",
            "method.server_discover",
            "header.mcp_session_id",
            "header.mcp_method_and_name",
            "header.last_event_id",
            "http.verb.get_delete",
            "result.result_type",
            "result.server_info",
            "result.cache_scope",
            "http.status.error_code_mapping",
        ];
        for id in reused {
            assert!(
                ObservationId::from_registry(id).is_some(),
                "reused id `{id}` must be in PROBE_REGISTRY verbatim"
            );
        }
    }

    // The mirror assertion — that the four DELIBERATELY-NOT-PORTED ids stay
    // unresolvable — deliberately does NOT live here. Spelling those ids as
    // string literals in this file would defeat the mechanical check that the
    // registry does not carry them (Task-1 acceptance criterion). It lives in
    // `crates/pmcp-team-servers/tests/era_baseline.rs` instead, next to the
    // two-direction coverage tests it belongs with.

    /// The four CONF-03 ids (D-17) are the whole reason the fixture-format
    /// extension could be closed off.
    #[test]
    fn the_four_conf_03_ids_are_present() {
        for id in [
            METHOD_LOGGING_SET_LEVEL,
            META_LOG_LEVEL,
            RESULT_INPUT_REQUIRED_SAMPLING,
            RESULT_INPUT_REQUIRED_ROOTS,
        ] {
            assert_eq!(ObservationId::from_registry(id.as_str()), Some(id));
        }
    }

    /// The token vocabulary is the JOIN vocabulary; if it drifts, every
    /// EXPECTED classification silently becomes UNEXPECTED.
    #[test]
    fn observed_value_tokens_are_stable() {
        assert_eq!(ObservedValue::Present.token(), "present");
        assert_eq!(ObservedValue::Absent.token(), "absent");
        assert_eq!(ObservedValue::Text("served".into()).token(), "served");
        assert_eq!(
            ObservedValue::Text("status:405".into()).token(),
            "status:405"
        );
        assert_eq!(
            ObservedValue::Unavailable("x".into()).token(),
            "unavailable"
        );
        assert!(!ObservedValue::Unavailable("x".into()).is_established());
        assert!(ObservedValue::Absent.is_established());
    }

    /// `Unavailable` (the probe could not tell) must never be confusable with
    /// `Absent` (the probe established that nothing was there).
    #[test]
    fn unavailable_is_not_absent() {
        assert_ne!(
            ObservedValue::Unavailable("boom".into()),
            ObservedValue::Absent
        );
        assert_ne!(
            ObservedValue::Unavailable("boom".into()).token(),
            ObservedValue::Absent.token()
        );
    }

    #[test]
    fn observations_iterate_deterministically() {
        let mut map = BTreeMap::new();
        map.insert(RESULT_CACHE_SCOPE, ObservedValue::Absent);
        map.insert(METHOD_INITIALIZE, ObservedValue::Text("served".into()));
        let observations = EraObservations(map);
        assert_eq!(
            observations.ids(),
            vec![METHOD_INITIALIZE, RESULT_CACHE_SCOPE],
            "BTreeMap order is the rendering order and must be sorted"
        );
        assert_eq!(observations.len(), 2);
        assert!(!observations.is_empty());
        assert_eq!(
            observations.get(METHOD_INITIALIZE),
            Some(&ObservedValue::Text("served".into()))
        );
    }

    /// An id can never be minted from a runtime string — that is the whole
    /// reason for the `&'static str` newtype.
    #[test]
    fn from_registry_resolves_only_registry_ids() {
        assert_eq!(
            ObservationId::from_registry("method.initialize"),
            Some(METHOD_INITIALIZE)
        );
        assert_eq!(ObservationId::from_registry(""), None);
        assert_eq!(ObservationId::from_registry("method.initialize "), None);
    }

    /// Deserialization VALIDATES: a stored report naming an id this build has
    /// no probe for is a report this build cannot honestly interpret.
    #[test]
    fn deserializing_an_unknown_observation_id_is_an_error() {
        let ok: Result<ObservationId, _> = serde_json::from_str("\"meta.log_level\"");
        assert_eq!(ok.expect("a registry id must deserialize"), META_LOG_LEVEL);

        let err = serde_json::from_str::<ObservationId>("\"method.no_probe_for_this\"")
            .expect_err("a non-registry id must be rejected");
        assert!(
            err.to_string().contains("not a known observation id"),
            "the error must name the failure: {err}"
        );
    }

    #[test]
    fn observation_ids_round_trip_through_serde() {
        let json = serde_json::to_string(&METHOD_INITIALIZE).expect("serializes");
        assert_eq!(json, "\"method.initialize\"");
        assert_eq!(METHOD_INITIALIZE.to_string(), "method.initialize");
    }

    // CLAUDE.md ALWAYS / PROPERTY testing.
    proptest::proptest! {
        /// `token()` is TOTAL and never empty — an empty token would join
        /// against nothing and quietly classify every delta as MISSING.
        #[test]
        fn every_observed_value_has_a_non_empty_token(text in "[^\\s]{1,32}", code in 0u16..600) {
            for value in [
                ObservedValue::Present,
                ObservedValue::Absent,
                ObservedValue::Text(text.clone()),
                ObservedValue::Text(format!("status:{code}")),
                ObservedValue::Unavailable(text.clone()),
            ] {
                proptest::prop_assert!(!value.token().is_empty());
            }
        }

        /// `from_registry` is TOTAL over arbitrary text and never yields an id
        /// outside the registry.
        #[test]
        fn from_registry_never_invents_an_id(raw in ".*") {
            if let Some(id) = ObservationId::from_registry(&raw) {
                proptest::prop_assert!(PROBE_REGISTRY.contains(&id));
                proptest::prop_assert_eq!(id.as_str(), raw.as_str());
            }
        }
    }
}
