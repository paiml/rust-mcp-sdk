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
//! # The two halves of this module
//!
//! Everything above the `Probes` banner is PURE DATA: the ids, the typed
//! observed values and the registry. It compiles with no HTTP stack at all, so a
//! `--no-default-features --features conformance` build still gets the substrate
//! and the baseline join.
//!
//! Everything below that banner is the WIRE half added by plan 118-06, gated on
//! `all(feature = "conformance", feature = "http")`. It needs an
//! [`EraProbeClient`](crate::conformance::era_probe::EraProbeClient) and a live
//! endpoint, so it is the one part of this module that knows how a target is
//! reached.
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

#[cfg(all(feature = "conformance", feature = "http"))]
use {
    crate::conformance::era_probe::{EraProbeClient, RawProbeOutcome, V2HeaderMode},
    crate::conformance::era_target::{
        LOG_LEVEL_META_KEY, LOG_LEVEL_SOURCE_REQUEST_META, LOG_RESULT_SOURCE_FIELD,
        TOOL_LIST_ROOTS, TOOL_LOG_EMIT, TOOL_REQUEST_SAMPLING,
    },
    pmcp::types::mrtr::{InputRequestKind, InputRequiredResult},
    pmcp::types::protocol::Era,
    serde_json::{json, Value},
};

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

// ===========================================================================
// Probes.
//
// One probe per registry id. Every probe obeys the era-INDEPENDENCE rule: it
// reads the wire and maps what it saw to a token, and NEVER consults `era` to
// decide what it "should" have seen. `era` reaches a probe for exactly one
// purpose — FRAMING the request it is about to send (which headers to set, which
// reserved `_meta` keys to attach, which protocol version to offer). A probe
// that consulted the era inside a classification arm would be asserting its own
// premise, and the matrix built on it would report agreement with the plan
// rather than with the server.
// ===========================================================================

/// The log level the `meta.log_level` probe asks the target to adopt.
///
/// A level that is NOT the target's own default, so an honoured `_meta` key and
/// an ignored one cannot produce the same reported level by coincidence.
#[cfg(all(feature = "conformance", feature = "http"))]
const PROBED_LOG_LEVEL: pmcp::types::notifications::LoggingLevel =
    pmcp::types::notifications::LoggingLevel::Debug;

/// The method the error-status-mapping probe sends.
///
/// A REAL protocol method that this target does not serve, deliberately NOT the
/// analog's fabricated `<crate>/definitely-not-a-method` string. MEASURED
/// against the era target: a method name outside the `ClientRequest` tagged enum
/// never reaches the era's status table at all on v1 — the v1 transport rejects
/// it during deserialization with `400` and `-32700`, while v2's own ingress
/// classifier answers `404` and `-32601`. Both statuses are non-`200`, so the
/// ported probe classified BOTH eras `era-gated-table` and the observation
/// collapsed to no observation.
///
/// `tasks/list` isolates the table instead: this target has no task store, so
/// BOTH eras refuse it with the SAME JSON-RPC code (`-32601`) and differ only in
/// the HTTP status they carry it under — `200` on v1, `404` on v2. Same code,
/// different status, which is exactly and only the fact ERA-10 is about.
///
/// The analog can afford a fabricated name because `mcp-tester` is pointed at
/// arbitrary third-party servers whose method sets it does not know. This crate
/// owns its target.
#[cfg(all(feature = "conformance", feature = "http"))]
const UNSERVED_METHOD: &str = "tasks/list";

/// Run every probe against `probe` for `era` and collect what they saw.
///
/// Probes are SEQUENTIAL on purpose: they share one endpoint, and several of
/// them are about SESSION and HEADER state, which concurrent requests would make
/// unreadable.
///
/// The returned map carries a value for every id in [`PROBE_REGISTRY`]. Against
/// a healthy target every one of them is ESTABLISHED — an
/// [`ObservedValue::Unavailable`] anywhere is a defect in the PROBE, not a
/// finding about the server.
///
/// # Examples
///
/// ```
/// # async fn demo() {
/// use pmcp::types::protocol::Era;
/// use pmcp_team_servers::conformance::era_observations::{observe, PROBE_REGISTRY};
/// use pmcp_team_servers::conformance::era_probe::EraProbeClient;
/// use pmcp_team_servers::conformance::era_target::spawn_era_target;
///
/// let target = spawn_era_target().await.expect("the era target binds");
/// let probe = EraProbeClient::new(target.url().as_str()).expect("the probe builds");
/// let observed = observe(&probe, Era::V1).await;
/// assert_eq!(observed.len(), PROBE_REGISTRY.len());
/// target.shutdown();
/// # }
/// ```
#[cfg(all(feature = "conformance", feature = "http"))]
pub async fn observe(probe: &EraProbeClient, era: Era) -> EraObservations {
    let mut out = BTreeMap::new();

    // ESTABLISH A SESSION FIRST, on the v1 path.
    //
    // A stateful v1 server refuses every non-initialization request that arrives
    // without an `Mcp-Session-Id` (400, "Session ID required for
    // non-initialization requests"). Without this, every POST probe below would
    // observe THAT refusal and report it as though it were the fact the probe
    // was sent to establish. v2 mints no session (ERA-03), so on that path the
    // initialize attempt is the ERA-01 observation and nothing else.
    let initialize = probe_initialize_raw(probe, era).await;
    let session = initialize
        .as_ref()
        .ok()
        .and_then(|outcome| outcome.session_header.clone());
    let session = session.as_deref();

    out.insert(METHOD_INITIALIZE, classify_initialize(&initialize));
    out.insert(HEADER_MCP_SESSION_ID, classify_session_header(&initialize));
    out.insert(
        METHOD_SERVER_DISCOVER,
        probe_server_discover(probe, era, session).await,
    );
    out.insert(
        HEADER_MCP_METHOD_AND_NAME,
        probe_required_headers(probe, era, session).await,
    );

    // ONE `tools/list` answers three observations, so it is sent once and
    // classified three times.
    let list = send_standard(probe, era, session, "tools/list", json!({})).await;
    out.insert(
        RESULT_RESULT_TYPE,
        classify_envelope_key(&list, "resultType"),
    );
    out.insert(RESULT_SERVER_INFO, classify_server_info(&list));
    out.insert(RESULT_CACHE_SCOPE, classify_cache_hints(&list));

    out.insert(
        HTTP_STATUS_ERROR_CODE_MAPPING,
        probe_error_status_mapping(probe, era, session).await,
    );

    // The four CONF-03 observations (D-12 / D-17).
    out.insert(
        METHOD_LOGGING_SET_LEVEL,
        probe_logging_set_level(probe, era, session).await,
    );
    out.insert(
        META_LOG_LEVEL,
        probe_meta_log_level(probe, era, session).await,
    );
    out.insert(
        RESULT_INPUT_REQUIRED_SAMPLING,
        probe_input_required(
            probe,
            era,
            session,
            TOOL_REQUEST_SAMPLING,
            InputRequestKind::Sampling,
        )
        .await,
    );
    out.insert(
        RESULT_INPUT_REQUIRED_ROOTS,
        probe_input_required(
            probe,
            era,
            session,
            TOOL_LIST_ROOTS,
            InputRequestKind::Roots,
        )
        .await,
    );

    // THE GET PROBES. They run LAST, and they run on the session established
    // above.
    //
    // Both orderings matter, for different reasons:
    //
    //  * REUSING the session is what stops them LEAKING. A `GET` with no session
    //    id makes a stateful v1 server MINT a fresh session and register an SSE
    //    stream against it, and nothing ever tears those down — two leaked
    //    sessions and two live stream entries per era run.
    //
    //  * Running them LAST is what stops the reuse CORRUPTING every other
    //    observation. Once a `GET` registers an SSE stream for a session, the
    //    transport routes every later POST response for that session INTO the
    //    stream and answers the POST `202 Accepted` with no envelope — so every
    //    probe after it would observe `status:202` instead of the fact it was
    //    sent to establish.
    //
    // The observation map is a `BTreeMap`, so call order does not affect the
    // rendered order. These two probes read only HTTP verb behaviour and depend
    // on nothing the POST probes did.
    out.insert(
        HEADER_LAST_EVENT_ID,
        probe_last_event_id(probe, era, session).await,
    );
    out.insert(
        HTTP_VERB_GET_DELETE,
        probe_get_delete(probe, era, session).await,
    );

    // TEAR THE SESSION DOWN. The `initialize` above made a stateful v1 server
    // MINT and store a session, and the GET probes just registered an SSE stream
    // against it. Best effort: a v2 connection has no session to tear down, and
    // the outcome is never an observation.
    teardown_session(probe, era, session).await;

    EraObservations(out)
}

/// Issue the spec's `DELETE` teardown for a session these probes minted.
///
/// Best effort by design: its outcome is not an observation, and a failure to
/// tear down must never change what the probes reported.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn teardown_session(probe: &EraProbeClient, era: Era, session: Option<&str>) {
    let Some(session) = session else {
        return;
    };
    let _ = probe
        .raw_verb_probe(
            "DELETE",
            era,
            &[(pmcp::shared::http_constants::MCP_SESSION_ID, session)],
        )
        .await;
}

/// Shorthand for the transport-failure answer.
#[cfg(all(feature = "conformance", feature = "http"))]
fn unavailable(reason: &str) -> ObservedValue {
    ObservedValue::Unavailable(reason.to_string())
}

/// The standard-header JSON-RPC probe call.
///
/// Several probes below issue byte-identical calls modulo the method and params,
/// so the call SHAPE lives here once. A change to it — a new header mode, a body
/// cap, an extra argument — is then one edit rather than several that must stay
/// in lockstep, where a missed one silently changes what that observation
/// measures.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn send_standard(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
    method: &str,
    params: Value,
) -> Result<RawProbeOutcome, String> {
    probe
        .raw_jsonrpc_probe_with_session(method, "", params, era, V2HeaderMode::Standard, session)
        .await
}

/// A `tools/call` for `tool`, with `Mcp-Name` set to the tool name.
///
/// `tools/call` IS name-bearing, so since D-13/D-18 the server cross-checks the
/// header against `params.name`. Passing the tool name to BOTH is what keeps
/// these calls out of the `HEADER_MISMATCH` path — a probe rejected at the gate
/// would observe the gate, not the tool.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn call_tool(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
    tool: &str,
    extra_params: Value,
) -> Result<RawProbeOutcome, String> {
    let mut params = json!({ "name": tool, "arguments": {} });
    if let (Value::Object(target), Value::Object(extra)) = (&mut params, extra_params) {
        for (key, value) in extra {
            target.insert(key, value);
        }
    }
    probe
        .raw_jsonrpc_probe_with_session(
            "tools/call",
            tool,
            params,
            era,
            V2HeaderMode::Standard,
            session,
        )
        .await
}

/// The shared classification: a `result` means served, any refusal is recorded
/// with its own code so a CHANGED rejection shape shows up as a finding rather
/// than as agreement, and a transport failure stays `Unavailable` — never
/// `Absent`, because "we could not tell" is not "we saw nothing".
#[cfg(all(feature = "conformance", feature = "http"))]
fn served_or_error(probe: Result<RawProbeOutcome, String>, served: &str) -> ObservedValue {
    match probe {
        Ok(outcome) if outcome.is_result() => ObservedValue::Text(served.into()),
        Ok(outcome) => ObservedValue::Text(error_token(outcome.error_code, outcome.http_status)),
        Err(error) => unavailable(&error),
    }
}

/// Render a refusal as a stable token: the JSON-RPC code when there is one,
/// otherwise the HTTP status.
///
/// NEVER prose — a message-derived token is unstable string matching, and the
/// baseline's `v1:` / `v2:` columns would have to track server wording.
#[cfg(all(feature = "conformance", feature = "http"))]
fn error_token(error_code: Option<i64>, http_status: u16) -> String {
    error_code.map_or_else(|| format!("status:{http_status}"), |c| format!("error:{c}"))
}

/// Send a WELL-FORMED `initialize` for `era`.
///
/// The params MUST be well-formed. MEASURED: a request whose params omit
/// `clientInfo` / `capabilities` is refused `-32601` by the TYPED PARSE before
/// dispatch, so a refusal of a MALFORMED request says nothing about whether the
/// METHOD exists — and a probe built that way would report ERA-01 `absent`
/// against a server that serves `initialize` perfectly well.
///
/// Its response answers TWO observations — ERA-01 (was the method served) and
/// ERA-03 (did a session header come back) — which is why it is sent once and
/// classified twice.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_initialize_raw(probe: &EraProbeClient, era: Era) -> Result<RawProbeOutcome, String> {
    // Both sides come from pmcp's own constants. A hardcoded v1 version string
    // here rots the moment the SDK's v1 constant moves: the probe would then
    // offer a version the server does not support, be refused, and report ERA-01
    // `absent` against every conformant v1 server — a permanent false finding
    // produced by a string literal.
    //
    // This is the ONE place `era` is read outside a request-framing header
    // decision, and it is still framing: it selects which protocol version the
    // request OFFERS, not what the response is taken to mean.
    let version = if era == Era::V2 {
        pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28
    } else {
        pmcp::LATEST_PROTOCOL_VERSION
    };
    let params = json!({
        "protocolVersion": version,
        "clientInfo": { "name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION") },
        "capabilities": {},
    });
    probe
        .raw_jsonrpc_probe_with_session("initialize", "", params, era, V2HeaderMode::Standard, None)
        .await
}

/// ERA-01. Rule: a JSON-RPC `result` means the method is `served`; anything else
/// means it is `absent`.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_initialize(probe: &Result<RawProbeOutcome, String>) -> ObservedValue {
    match probe {
        Ok(outcome) if outcome.is_result() => ObservedValue::Text("served".into()),
        Ok(_) => ObservedValue::Text("absent".into()),
        Err(error) => unavailable(error),
    }
}

/// ERA-03. Rule: a session-id header on the initialization response means the
/// server is minting AND echoing; its absence means it is doing neither.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_session_header(probe: &Result<RawProbeOutcome, String>) -> ObservedValue {
    match probe {
        Ok(outcome) if outcome.session_header.is_some() => {
            ObservedValue::Text("minted-and-echoed".into())
        },
        Ok(_) => ObservedValue::Text("never-minted-inbound-ignored".into()),
        Err(error) => unavailable(error),
    }
}

/// ERA-02. Rule: a `result` means `served`; a refusal is recorded with its own
/// code, so a CHANGED rejection shape shows up as a finding rather than as
/// agreement.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_server_discover(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    served_or_error(
        send_standard(probe, era, session, "server/discover", json!({})).await,
        "served",
    )
}

/// ERA-04. Rule: send a request DELIBERATELY omitting the method and name
/// routing headers. If the server still answers normally they are `not-sent`; if
/// it refuses, they are `required-and-cross-checked`.
///
/// This is the one observation that cannot be made by watching a conformant
/// request — a header that is not required looks exactly like one that is, until
/// you leave it out.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_required_headers(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    match probe
        .raw_jsonrpc_probe_with_session(
            "tools/list",
            "",
            json!({}),
            era,
            V2HeaderMode::OmitMethodAndName,
            session,
        )
        .await
    {
        Ok(outcome) if outcome.is_result() => ObservedValue::Text("not-sent".into()),
        Ok(_) => ObservedValue::Text("required-and-cross-checked".into()),
        Err(error) => unavailable(&error),
    }
}

/// The header list for a `GET` probe, carrying the ALREADY-ESTABLISHED session
/// when there is one.
///
/// A `GET` with no session id makes a stateful v1 server MINT a fresh session
/// and register an SSE stream against it, neither of which anything ever tears
/// down — one leak per GET probe, per era run. Reusing the session `observe`
/// already established keeps the count at one, which [`teardown_session`] then
/// deletes.
#[cfg(all(feature = "conformance", feature = "http"))]
fn get_probe_headers<'a>(
    session: Option<&'a str>,
    extra: &[(&'a str, &'a str)],
) -> Vec<(&'a str, &'a str)> {
    let mut headers: Vec<(&str, &str)> = extra.to_vec();
    if let Some(session) = session {
        headers.push((pmcp::shared::http_constants::MCP_SESSION_ID, session));
    }
    headers
}

/// ERA-05. Rule: `GET` the endpoint with a `Last-Event-ID` header. A `200` with
/// an SSE content type means resumability is `supported`; anything else means
/// the header is `ignored`.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_last_event_id(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    let headers = get_probe_headers(
        session,
        &[(pmcp::shared::http_constants::LAST_EVENT_ID, "0")],
    );
    match probe.raw_verb_probe("GET", era, &headers).await {
        Ok((status, content_type))
            if status == 200
                && content_type.contains(pmcp::shared::http_constants::TEXT_EVENT_STREAM) =>
        {
            ObservedValue::Text("supported".into())
        },
        Ok(_) => ObservedValue::Text("ignored".into()),
        Err(error) => unavailable(&error),
    }
}

/// ERA-06. Rule: probe both `GET` and `DELETE`. When BOTH answer `405` the verb
/// surface is rejected; otherwise at least one verb is still serving its v1 role
/// (an SSE stream or a session teardown).
///
/// The `GET` carries the established session (see [`get_probe_headers`]); the
/// `DELETE` deliberately does NOT, because a `DELETE` that named the session
/// would tear it down MID-OBSERVATION and every probe after this one would be
/// refused for an unknown session. Teardown is [`teardown_session`]'s job, at
/// the end.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_get_delete(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    let get = probe
        .raw_verb_probe("GET", era, &get_probe_headers(session, &[]))
        .await;
    let delete = probe.raw_verb_probe("DELETE", era, &[]).await;
    match (get, delete) {
        (Ok((405, _)), Ok((405, _))) => ObservedValue::Text("status:405".into()),
        (Ok(_), Ok(_)) => ObservedValue::Text("sse-stream-or-session-teardown".into()),
        (Err(error), _) | (_, Err(error)) => unavailable(&error),
    }
}

/// ERA-07. Rule: is `key` a present, non-null member of the JSON-RPC `result`?
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_envelope_key(probe: &Result<RawProbeOutcome, String>, key: &str) -> ObservedValue {
    match probe {
        Ok(outcome) => match outcome.result.as_ref().and_then(|result| result.get(key)) {
            Some(value) if !value.is_null() => ObservedValue::Present,
            _ => ObservedValue::Absent,
        },
        Err(error) => unavailable(error),
    }
}

/// ERA-08. Rule: the server identity may ride at the top of the result or under
/// the result's `_meta` (v2 carries it as reserved response metadata), so BOTH
/// locations count as present. A probe that only looked at the top level would
/// report a permanent false `absent`.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_server_info(probe: &Result<RawProbeOutcome, String>) -> ObservedValue {
    match probe {
        Ok(outcome) => {
            let Some(result) = outcome.result.as_ref() else {
                return ObservedValue::Absent;
            };
            let top = result.get("serverInfo").is_some_and(|v| !v.is_null());
            let in_meta = result
                .get("_meta")
                .and_then(|meta| meta.get(pmcp::testing::META_SERVER_INFO))
                .is_some_and(|v| !v.is_null());
            if top || in_meta {
                ObservedValue::Present
            } else {
                ObservedValue::Absent
            }
        },
        Err(error) => unavailable(error),
    }
}

/// ERA-09. Rule: both `ttlMs` AND `cacheScope` present means the pair is
/// `required`; neither present means `absent`; exactly one is `partial`, which
/// matches no baseline column and so surfaces as a finding — which is correct,
/// because the schema declares both without `?`.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_cache_hints(probe: &Result<RawProbeOutcome, String>) -> ObservedValue {
    match probe {
        Ok(outcome) => {
            let Some(result) = outcome.result.as_ref() else {
                return ObservedValue::Absent;
            };
            let ttl = result.get("ttlMs").is_some_and(|v| !v.is_null());
            let scope = result.get("cacheScope").is_some_and(|v| !v.is_null());
            match (ttl, scope) {
                (true, true) => ObservedValue::Text("required".into()),
                (false, false) => ObservedValue::Absent,
                _ => ObservedValue::Text("partial".into()),
            }
        },
        Err(error) => unavailable(error),
    }
}

/// ERA-10. Rule: send [`UNSERVED_METHOD`] and read the HTTP STATUS the JSON-RPC
/// error came back under. The legacy table returns `200` for every
/// JSON-RPC-level error; the era-gated table maps the code onto a real HTTP
/// status. So `200` means the legacy table is in force and anything else means
/// the era-gated one is.
///
/// Note this reads the STATUS, not the era: it is the MAPPING that is being
/// observed, and consulting the era to decide would make the observation
/// circular.
///
/// Two outcomes are `Unavailable` rather than tokens, because in both of them
/// the request never reached the table and NOTHING was established about it:
///
/// * the method was SERVED (there is no error to carry a status), which would
///   happen if a future target grew a task store; and
/// * the request was rejected at PARSE, before dispatch — the exact way the
///   analog's fabricated method name silently collapsed this observation. See
///   [`UNSERVED_METHOD`].
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_error_status_mapping(probed: Result<RawProbeOutcome, String>) -> ObservedValue {
    let outcome = match probed {
        Ok(outcome) => outcome,
        Err(error) => return unavailable(&error),
    };
    let Some(code) = outcome.error_code else {
        return unavailable(&format!(
            "`{UNSERVED_METHOD}` was SERVED (status {}), so no error carried a status to read",
            outcome.http_status
        ));
    };
    if code == i64::from(pmcp::types::protocol::error_codes::PARSE_ERROR) {
        return unavailable(&format!(
            "`{UNSERVED_METHOD}` was rejected at PARSE (status {}), before any era status \
             mapping ran",
            outcome.http_status
        ));
    }
    if outcome.http_status == 200 {
        ObservedValue::Text("unchanged-legacy-table".into())
    } else {
        ObservedValue::Text("era-gated-table".into())
    }
}

/// ERA-10, over the wire. See [`classify_error_status_mapping`] for the rule.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_error_status_mapping(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    classify_error_status_mapping(
        send_standard(probe, era, session, UNSERVED_METHOD, json!({})).await,
    )
}

/// ERA-11. Rule: a `result` means the RPC is still `served`; a refusal is
/// recorded with its own code.
///
/// The level is serialized from the SDK's own [`pmcp::types::notifications::LoggingLevel`]
/// rather than spelled, so a schema rename cannot make this probe silently send
/// an invalid level and observe a parse refusal instead of the method's fate.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_logging_set_level(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    let level = serde_json::to_value(PROBED_LOG_LEVEL).unwrap_or(Value::Null);
    served_or_error(
        send_standard(
            probe,
            era,
            session,
            "logging/setLevel",
            json!({ "level": level }),
        )
        .await,
        "served",
    )
}

/// The payload a tool result carries, from either voice.
///
/// A tool with a declared `outputSchema` gets its value emitted as
/// `structuredContent`; one without carries it as a serialized string in the
/// text voice. Reading BOTH means a probe does not silently depend on the
/// target's schema declarations.
#[cfg(all(feature = "conformance", feature = "http"))]
fn tool_payload(outcome: &RawProbeOutcome) -> Option<Value> {
    let result = outcome.result.as_ref()?;
    if let Some(structured) = result.get("structuredContent").filter(|v| !v.is_null()) {
        return Some(structured.clone());
    }
    let text = result
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;
    serde_json::from_str(text).ok()
}

/// ERA-12. Rule: call the log-emit tool with the per-request `_meta` level key
/// set, and map the tool's REPORTED SOURCE to a token — `honored` when the
/// server says the level came from `_meta`, `ignored` when it says it came from
/// anywhere else.
///
/// Reading the source rather than the level is what makes this observable at
/// all: a server that ignored the key and one that honoured a level equal to its
/// own default would report the same LEVEL, and the observation would collapse.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_meta_log_level(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    let level = serde_json::to_value(PROBED_LOG_LEVEL).unwrap_or(Value::Null);
    let extra = json!({ "_meta": { LOG_LEVEL_META_KEY: level } });
    match call_tool(probe, era, session, TOOL_LOG_EMIT, extra).await {
        Ok(outcome) => classify_log_level_source(&outcome),
        Err(error) => unavailable(&error),
    }
}

/// The ERA-12 classifier, split out so it is testable without a live endpoint.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_log_level_source(outcome: &RawProbeOutcome) -> ObservedValue {
    match tool_payload(outcome)
        .as_ref()
        .and_then(|payload| payload.get(LOG_RESULT_SOURCE_FIELD))
        .and_then(Value::as_str)
    {
        Some(source) if source == LOG_LEVEL_SOURCE_REQUEST_META => {
            ObservedValue::Text("honored".into())
        },
        Some(_) => ObservedValue::Text("ignored".into()),
        // The tool reported no source at all, so nothing was established about
        // the key: a refusal, an empty envelope or a changed tool shape. That is
        // NOT the same as "the key was ignored", and recording it as such would
        // manufacture an observation out of a broken probe.
        None => unavailable(&format!(
            "{TOOL_LOG_EMIT} reported no `{LOG_RESULT_SOURCE_FIELD}` (status {}, error {:?})",
            outcome.http_status, outcome.error_code
        )),
    }
}

/// ERA-13 / ERA-14. Rule: call the tool and classify the RESULT ENVELOPE —
/// `present` when it is an `input_required` continuation carrying an
/// `inputRequests` entry of `kind`, `absent` otherwise.
#[cfg(all(feature = "conformance", feature = "http"))]
async fn probe_input_required(
    probe: &EraProbeClient,
    era: Era,
    session: Option<&str>,
    tool: &str,
    kind: InputRequestKind,
) -> ObservedValue {
    match call_tool(probe, era, session, tool, json!({})).await {
        Ok(outcome) => classify_input_required(outcome.result.as_ref(), kind),
        Err(error) => unavailable(&error),
    }
}

/// The ERA-13 / ERA-14 classifier.
///
/// The envelope is READ THROUGH the SDK's own
/// [`InputRequiredResult`](pmcp::types::mrtr::InputRequiredResult), so neither
/// the discriminator field name nor its `input_required` value is spelled here.
/// A schema rename then breaks the SDK type — loudly — instead of turning this
/// probe into a permanent false `absent`.
#[cfg(all(feature = "conformance", feature = "http"))]
fn classify_input_required(result: Option<&Value>, kind: InputRequestKind) -> ObservedValue {
    let Some(result) = result else {
        return ObservedValue::Absent;
    };
    let Ok(parsed) = serde_json::from_value::<InputRequiredResult>(result.clone()) else {
        return ObservedValue::Absent;
    };
    let carries_kind = parsed.is_input_required()
        && parsed
            .input_requests
            .as_ref()
            .is_some_and(|requests| requests.values().any(|request| request.kind() == kind));
    if carries_kind {
        ObservedValue::Present
    } else {
        ObservedValue::Absent
    }
}

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

/// Pure-classifier tests for the wire half. No I/O, no endpoint.
#[cfg(all(test, feature = "http"))]
mod classifier_tests {
    use super::*;

    fn outcome(result: Option<Value>) -> RawProbeOutcome {
        RawProbeOutcome {
            http_status: 200,
            session_header: None,
            result,
            error_code: None,
        }
    }

    #[test]
    fn error_token_prefers_the_jsonrpc_code_and_falls_back_to_the_status() {
        assert_eq!(error_token(Some(-32601), 404), "error:-32601");
        assert_eq!(error_token(None, 405), "status:405");
    }

    #[test]
    fn a_plain_tool_result_is_not_an_input_required_continuation() {
        let plain = json!({ "content": [{ "type": "text", "text": "{}" }] });
        assert_eq!(
            classify_input_required(Some(&plain), InputRequestKind::Sampling),
            ObservedValue::Absent
        );
        assert_eq!(
            classify_input_required(None, InputRequestKind::Roots),
            ObservedValue::Absent
        );
    }

    /// The continuation envelope is recognised, AND the KIND is discriminated —
    /// a roots continuation must not satisfy the sampling observation.
    #[test]
    fn an_input_required_continuation_is_classified_by_kind() {
        let sampling = json!({
            "resultType": "input_required",
            "inputRequests": {
                "k": { "method": "sampling/createMessage", "params": { "messages": [] } }
            },
            "requestState": "opaque",
        });
        assert_eq!(
            classify_input_required(Some(&sampling), InputRequestKind::Sampling),
            ObservedValue::Present
        );
        assert_eq!(
            classify_input_required(Some(&sampling), InputRequestKind::Roots),
            ObservedValue::Absent
        );

        let roots = json!({
            "resultType": "input_required",
            "inputRequests": { "k": { "method": "roots/list" } },
            "requestState": "opaque",
        });
        assert_eq!(
            classify_input_required(Some(&roots), InputRequestKind::Roots),
            ObservedValue::Present
        );
    }

    #[test]
    fn the_log_level_source_maps_to_honored_ignored_or_unavailable() {
        let honored = outcome(Some(json!({
            "structuredContent": { LOG_RESULT_SOURCE_FIELD: LOG_LEVEL_SOURCE_REQUEST_META }
        })));
        assert_eq!(
            classify_log_level_source(&honored),
            ObservedValue::Text("honored".into())
        );

        let ignored = outcome(Some(json!({
            "structuredContent": { LOG_RESULT_SOURCE_FIELD: "server-default" }
        })));
        assert_eq!(
            classify_log_level_source(&ignored),
            ObservedValue::Text("ignored".into())
        );

        // No source at all is NOT "ignored" — nothing was established.
        assert!(!classify_log_level_source(&outcome(Some(json!({})))).is_established());
        assert!(!classify_log_level_source(&outcome(None)).is_established());
    }

    /// The text voice is read when a tool declares no output schema, so a probe
    /// does not silently depend on the target's schema declarations.
    #[test]
    fn tool_payload_reads_both_voices() {
        let structured = outcome(Some(json!({ "structuredContent": { "a": 1 } })));
        assert_eq!(tool_payload(&structured), Some(json!({ "a": 1 })));

        let text = outcome(Some(json!({
            "content": [{ "type": "text", "text": "{\"a\":2}" }]
        })));
        assert_eq!(tool_payload(&text), Some(json!({ "a": 2 })));

        assert_eq!(tool_payload(&outcome(None)), None);
    }

    #[test]
    fn served_or_error_keeps_unavailable_distinct_from_a_refusal() {
        let refused = RawProbeOutcome {
            http_status: 404,
            session_header: None,
            result: None,
            error_code: Some(-32601),
        };
        assert_eq!(
            served_or_error(Ok(refused), "served"),
            ObservedValue::Text("error:-32601".into())
        );
        assert_eq!(
            served_or_error(Ok(outcome(Some(json!({})))), "served"),
            ObservedValue::Text("served".into())
        );
        assert!(!served_or_error(Err("boom".to_string()), "served").is_established());
    }

    /// The ERA-10 rule, including the two cases the ported probe got wrong: a
    /// PARSE rejection and a SERVED method both establish nothing, and must not
    /// be reported as though the era-gated table had been observed.
    #[test]
    fn the_status_mapping_rule_reads_the_status_and_refuses_to_guess() {
        let refusal = |status: u16, code: i64| RawProbeOutcome {
            http_status: status,
            session_header: None,
            result: None,
            error_code: Some(code),
        };
        assert_eq!(
            classify_error_status_mapping(Ok(refusal(200, -32601))),
            ObservedValue::Text("unchanged-legacy-table".into())
        );
        assert_eq!(
            classify_error_status_mapping(Ok(refusal(404, -32601))),
            ObservedValue::Text("era-gated-table".into())
        );
        // A parse rejection never reached the table.
        assert!(!classify_error_status_mapping(Ok(refusal(400, -32700))).is_established());
        // A served method carries no error to read a status off.
        assert!(!classify_error_status_mapping(Ok(outcome(Some(json!({}))))).is_established());
        assert!(!classify_error_status_mapping(Err("boom".to_string())).is_established());
    }

    #[test]
    fn a_get_probe_carries_the_established_session_and_nothing_else() {
        assert_eq!(get_probe_headers(None, &[]), Vec::new());
        let with_session = get_probe_headers(Some("abc"), &[("Last-Event-ID", "0")]);
        assert_eq!(with_session.len(), 2);
        assert!(with_session.iter().any(|(key, value)| *key
            == pmcp::shared::http_constants::MCP_SESSION_ID
            && *value == "abc"));
    }
}

/// The LIVE smoke test: both eras, one endpoint, one transport.
///
/// This module is where the D-16 anti-vacuity control lives at its earliest
/// possible point. It runs the whole probe surface against a real
/// streamable-HTTP era target and asserts three things:
///
/// 1. every registry id has a value,
/// 2. every value is ESTABLISHED, and
/// 3. the v1 and v2 maps DIFFER.
///
/// (3) is the one that matters. If the v2 arm were inert — the exact defect D-16
/// exists to prevent — the two maps would be EQUAL, and this test, not a
/// downstream baseline join, is what says so.
///
/// It deliberately does NOT assert WHICH ids differ or WHAT the tokens are: that
/// is the baseline reconciliation, and it belongs to plan 118-07.
#[cfg(all(test, feature = "http"))]
mod live {
    use super::*;
    use crate::conformance::era_target::spawn_era_target;
    use std::collections::BTreeSet;

    /// Every registry id is present and ESTABLISHED, naming the offender when it
    /// is not — an unestablished value is a probe defect and must be
    /// diagnosable without a re-run.
    fn assert_every_id_is_established(observed: &EraObservations, era: Era) {
        let seen: BTreeSet<ObservationId> = observed.ids().into_iter().collect();
        let expected: BTreeSet<ObservationId> = PROBE_REGISTRY.iter().copied().collect();
        let missing: Vec<&str> = expected.difference(&seen).map(|id| id.as_str()).collect();
        assert!(
            missing.is_empty(),
            "FAILURE MODE: observe() under {era:?} produced NO value for {missing:?}.\n\
             WHAT TO DO: add the probe to observe(); every PROBE_REGISTRY id must be \
             answered on every run, or the baseline join reports a permanent false MISSING."
        );

        let unestablished: Vec<String> = observed
            .0
            .iter()
            .filter(|(_, value)| !value.is_established())
            .map(|(id, value)| match value {
                ObservedValue::Unavailable(reason) => format!("{id} -> Unavailable: {reason}"),
                other => format!("{id} -> {}", other.token()),
            })
            .collect();
        assert!(
            unestablished.is_empty(),
            "FAILURE MODE: these observations were NOT established under {era:?}:\n  {}\n\
             An Unavailable value is a defect in the PROBE, not a finding about the server \
             — it means the probe ran and could not tell.\n\
             WHAT TO DO: fix the probe named above; do NOT record Unavailable as a token.",
            unestablished.join("\n  ")
        );
    }

    #[tokio::test]
    async fn both_eras_observe_every_id_over_one_endpoint_and_they_differ() {
        let target = spawn_era_target().await.expect("the era target binds");
        let probe = EraProbeClient::new(target.url().as_str()).expect("the probe client builds");

        // ONE endpoint, ONE transport, two eras. That is what makes a difference
        // an ERA difference rather than a transport difference.
        let v1 = observe(&probe, Era::V1).await;
        let v2 = observe(&probe, Era::V2).await;

        assert_eq!(v1.len(), PROBE_REGISTRY.len());
        assert_eq!(v2.len(), PROBE_REGISTRY.len());
        assert_every_id_is_established(&v1, Era::V1);
        assert_every_id_is_established(&v2, Era::V2);

        let differing: Vec<String> = PROBE_REGISTRY
            .iter()
            .filter(|id| {
                v1.get(**id).map(ObservedValue::token) != v2.get(**id).map(ObservedValue::token)
            })
            .map(|id| {
                format!(
                    "{id}: v1 `{}` vs v2 `{}`",
                    v1.get(*id).map(ObservedValue::token).unwrap_or_default(),
                    v2.get(*id).map(ObservedValue::token).unwrap_or_default(),
                )
            })
            .collect();
        assert!(
            !differing.is_empty(),
            "FAILURE MODE: the v1 and v2 observation maps are IDENTICAL. The v2 arm measured \
             NOTHING — this is the Phase 118 D-16 defect reproducing.\n\
             FIRST THINGS TO CHECK: (a) that the probe really sets the v2 protocol-version \
             header and the reserved `_meta` keys, and (b) that the era target's accept-list \
             really lists BOTH versions. If a typed client is ever substituted here, check \
             `supports_negotiated_protocol_version` on its transport: the trait default is \
             `false`, and both `DuplexTransport` and `pmcp::HttpTransport` inherit it, which \
             silently degrades a v2 selection to v1.\n\
             WHAT TO DO: fix the arm; do NOT relax this assertion."
        );

        target.shutdown();
    }
}
