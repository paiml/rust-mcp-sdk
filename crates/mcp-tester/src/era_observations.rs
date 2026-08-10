//! Dedicated era PROBES: the wire facts a `TestReport` structurally cannot
//! carry.
//!
//! # Why this module exists
//!
//! MEASURED at `crates/mcp-tester/src/report.rs:74-81` and `:153-158`:
//! [`TestResult`](crate::TestResult) is
//! `{name, category, status, duration, error, details}` and
//! [`TestReport`](crate::TestReport) is `{tests, duration, timestamp, summary}`.
//!
//! Now read `crates/mcp-tester/baselines/era-deltas.yaml`. Most of its fourteen
//! entries are WIRE FACTS — a response header's presence, a session id, the
//! `Last-Event-ID` resumability surface, a result-envelope field, the LOCATION
//! of a capability, a caching hint, an HTTP status mapping. Not one of those is
//! representable in the two structs above. `details` is free-form prose.
//!
//! So a dual-run comparison that diffed two `TestReport`s by test NAME and
//! STATUS could not observe most of the baseline at all. It would report
//! permanent false MISSING findings for every wire fact no test name encodes,
//! and — if it fell back to substring matching on display names to compensate —
//! it would hand out false confidence instead. Dedicated probes emitting STABLE
//! IDs are what make the evidence observable in the first place.
//!
//! # The shape
//!
//! One probe per baseline `observation_id`, each recording what it saw as an
//! [`ObservedValue`] whose [`ObservedValue::token`] is drawn from the SAME short
//! vocabulary the baseline's `v1:` / `v2:` columns use. That is what lets
//! [`crate::era_diff`] join an observation against a delta and check not merely
//! that the eras DIFFERED but that they differed in the RECORDED way.
//!
//! Derivation rules are documented on each probe. They are era-INDEPENDENT: a
//! probe reads the wire and maps what it saw to a token, and never consults the
//! era to decide what it "should" have seen. A probe that did would be
//! asserting its own premise.

use std::collections::BTreeMap;

use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::Era;
use serde::{Deserialize, Serialize};

use crate::tester::{RawProbeOutcome, ServerTester, V2HeaderMode};

/// The STABLE, MACHINE-FACING name of one wire fact.
///
/// A newtype over a `&'static str` so an ID can never be built from a runtime
/// display name: the whole point is that these are fixed identifiers, joined
/// against `EraDelta::observation_id`, and never renamed for readability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationId(pub &'static str);

impl ObservationId {
    /// The identifier as a string slice.
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// Resolve a wire string back to a registry id.
    ///
    /// `None` for anything not in [`PROBE_REGISTRY`] — which is the whole point
    /// of the `&'static str` newtype: an id is a fixed identifier this crate
    /// knows, not an arbitrary string a caller invented.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ObservedValue {
    /// The wire fact was observed PRESENT.
    Present,
    /// The wire fact was observed ABSENT.
    Absent,
    /// A short observed token, in the baseline's own vocabulary.
    ///
    /// This is the only carrier for a status (`status:405`, produced by
    /// [`error_token`]) or a location. Typed `Status(u16)` / `Pointer(String)`
    /// variants existed here and were never constructed by any probe — and
    /// because [`Self::token`] rendered `Status(405)` and `Text("status:405")`
    /// identically, they were two values that compared EQUAL through the join
    /// while being distinct on the wire. Reintroduce a typed variant only
    /// alongside a probe that emits it.
    Text(String),
    /// The probe ran but could not establish the fact; the string says why.
    Unavailable(String),
}

impl ObservedValue {
    /// The canonical token this observation compares as.
    ///
    /// This is what [`crate::era_diff`] matches against an `EraDelta`'s `v1:` /
    /// `v2:` column, so the vocabulary here and the vocabulary in
    /// `era-deltas.yaml` are ONE vocabulary.
    pub fn token(&self) -> String {
        match self {
            Self::Present => "present".to_string(),
            Self::Absent => "absent".to_string(),
            Self::Text(s) => s.clone(),
            Self::Unavailable(_) => "unavailable".to_string(),
        }
    }

    /// Whether the probe actually established its fact.
    pub fn is_established(&self) -> bool {
        !matches!(self, Self::Unavailable(_))
    }
}

/// Everything one era run observed, keyed by [`ObservationId`].
///
/// A `BTreeMap` rather than a `HashMap`: the report is RENDERED and compared as
/// bytes, so iteration order has to be deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraObservations(pub BTreeMap<ObservationId, ObservedValue>);

impl EraObservations {
    /// Look one observation up.
    pub fn get(&self, id: ObservationId) -> Option<&ObservedValue> {
        self.0.get(&id)
    }

    /// Number of observations recorded.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing at all was observed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every id recorded, in sorted order.
    pub fn ids(&self) -> Vec<ObservationId> {
        self.0.keys().copied().collect()
    }
}

// ===========================================================================
// The probe registry.
// ===========================================================================

/// `method:initialize` — ERA-01.
pub const METHOD_INITIALIZE: ObservationId = ObservationId("method.initialize");
/// `method:server/discover` — ERA-02.
pub const METHOD_SERVER_DISCOVER: ObservationId = ObservationId("method.server_discover");
/// `header:Mcp-Session-Id` — ERA-03.
pub const HEADER_MCP_SESSION_ID: ObservationId = ObservationId("header.mcp_session_id");
/// `header:Mcp-Method,Mcp-Name` — ERA-04.
pub const HEADER_MCP_METHOD_AND_NAME: ObservationId = ObservationId("header.mcp_method_and_name");
/// `header:Last-Event-ID` — ERA-05.
pub const HEADER_LAST_EVENT_ID: ObservationId = ObservationId("header.last_event_id");
/// `http-verb:GET,DELETE` — ERA-06.
pub const HTTP_VERB_GET_DELETE: ObservationId = ObservationId("http.verb.get_delete");
/// `result-field:resultType` — ERA-07.
pub const RESULT_RESULT_TYPE: ObservationId = ObservationId("result.result_type");
/// `result-field:serverInfo` — ERA-08.
pub const RESULT_SERVER_INFO: ObservationId = ObservationId("result.server_info");
/// `method:tasks/list` — ERA-09.
pub const METHOD_TASKS_LIST: ObservationId = ObservationId("method.tasks_list");
/// `capability:tasks` location — ERA-10.
pub const CAPABILITY_TASKS_LOCATION: ObservationId = ObservationId("capability.tasks_location");
/// `result-field:ttlMs,cacheScope` — ERA-11.
pub const RESULT_CACHE_SCOPE: ObservationId = ObservationId("result.cache_scope");
/// `method:resources/subscribe` — ERA-12.
pub const METHOD_RESOURCES_SUBSCRIBE: ObservationId = ObservationId("method.resources_subscribe");
/// `method:subscriptions/listen` — ERA-13.
pub const METHOD_SUBSCRIPTIONS_LISTEN: ObservationId = ObservationId("method.subscriptions_listen");
/// `http-status:JSON-RPC error code mapping` — ERA-14.
pub const HTTP_STATUS_ERROR_CODE_MAPPING: ObservationId =
    ObservationId("http.status.error_code_mapping");

/// Every [`ObservationId`] this module has a probe for.
///
/// This is one half of the two-direction coverage contract with
/// `crates/mcp-tester/baselines/era-deltas.yaml`; the tests below assert both
/// directions. A baseline entry with no probe is an entry nothing can ever
/// observe, which would produce a permanent false MISSING finding — exactly the
/// defect the observation-id design removes. A probe with no baseline entry
/// would report every run of it as an UNEXPECTED finding.
pub const PROBE_REGISTRY: &[ObservationId] = &[
    METHOD_INITIALIZE,
    METHOD_SERVER_DISCOVER,
    HEADER_MCP_SESSION_ID,
    HEADER_MCP_METHOD_AND_NAME,
    HEADER_LAST_EVENT_ID,
    HTTP_VERB_GET_DELETE,
    RESULT_RESULT_TYPE,
    RESULT_SERVER_INFO,
    METHOD_TASKS_LIST,
    CAPABILITY_TASKS_LOCATION,
    RESULT_CACHE_SCOPE,
    METHOD_RESOURCES_SUBSCRIBE,
    METHOD_SUBSCRIPTIONS_LISTEN,
    HTTP_STATUS_ERROR_CODE_MAPPING,
];

// ===========================================================================
// Probes.
// ===========================================================================

/// Run every probe against `tester` for `era` and collect what they saw.
///
/// `tester` must already have established its connection (the capability
/// observations read the projection it holds). Probes are sequential on
/// purpose: they share one endpoint and several of them are about SESSION and
/// HEADER state, which concurrent requests would make unreadable.
pub async fn observe(tester: &ServerTester, era: Era) -> EraObservations {
    let mut out = BTreeMap::new();

    // Establish a session FIRST on the v1 path.
    //
    // A stateful v1 server refuses every non-initialization request that
    // arrives without an `Mcp-Session-Id` (400, "Session ID required for
    // non-initialization requests"). Without this, four probes below would
    // observe that refusal and report it as though it were the fact they were
    // sent to establish. v2 mints no session (ERA-03), so on that path the
    // initialize attempt is the ERA-01 observation and nothing else.
    let initialize = probe_initialize_raw(tester, era).await;
    let session = initialize
        .as_ref()
        .ok()
        .and_then(|o| o.session_header.clone());
    let session = session.as_deref();

    out.insert(METHOD_INITIALIZE, classify_initialize(&initialize));
    out.insert(
        METHOD_SERVER_DISCOVER,
        probe_server_discover(tester, era, session).await,
    );
    out.insert(HEADER_MCP_SESSION_ID, classify_session_header(&initialize));
    out.insert(
        HEADER_MCP_METHOD_AND_NAME,
        probe_required_headers(tester, era, session).await,
    );

    let list = tester
        .raw_jsonrpc_probe_with_session(
            "tools/list",
            "",
            serde_json::json!({}),
            era,
            V2HeaderMode::Standard,
            session,
        )
        .await;
    out.insert(
        RESULT_RESULT_TYPE,
        classify_envelope_key(&list, "resultType"),
    );
    out.insert(RESULT_SERVER_INFO, classify_server_info(&list));
    out.insert(RESULT_CACHE_SCOPE, classify_cache_hints(&list));

    out.insert(
        METHOD_TASKS_LIST,
        probe_tasks_list(tester, era, session).await,
    );
    out.insert(
        CAPABILITY_TASKS_LOCATION,
        probe_tasks_capability_location(tester),
    );
    out.insert(
        METHOD_RESOURCES_SUBSCRIBE,
        probe_resources_subscribe(tester, era, session).await,
    );
    out.insert(
        METHOD_SUBSCRIPTIONS_LISTEN,
        probe_subscriptions_listen(tester, era, session).await,
    );
    out.insert(
        HTTP_STATUS_ERROR_CODE_MAPPING,
        probe_error_status_mapping(tester, era, session).await,
    );

    // THE GET PROBES RUN LAST, and they run on the session established above.
    //
    // Both orderings matter, for different reasons:
    //
    //  * REUSING the session is what stops them LEAKING. A `GET` with no
    //    `Mcp-Session-Id` makes a stateful v1 server MINT a fresh session and
    //    register an SSE stream against it, and nothing ever tears those down —
    //    two leaked sessions and two live stream entries per era run, which is
    //    exactly hazard (b) that `detect_eras` already guards against.
    //
    //  * Running them LAST is what stops the reuse CORRUPTING every other
    //    observation. Once a `GET` registers an SSE stream for a session, the
    //    transport routes every later POST response for that session INTO the
    //    stream and answers the POST `202 Accepted` with no envelope — so
    //    `tasks/list`, `resources/subscribe` and the error-status mapping would
    //    all observe `status:202` instead of the fact they were sent to
    //    establish.
    //
    // The observation map is a `BTreeMap`, so call order does not affect the
    // rendered order. These two probes read only HTTP verb behaviour and depend
    // on nothing the POST probes did.
    out.insert(
        HEADER_LAST_EVENT_ID,
        probe_last_event_id(tester, era, session).await,
    );
    out.insert(
        HTTP_VERB_GET_DELETE,
        probe_get_delete(tester, era, session).await,
    );

    // TEAR THE SESSION DOWN, for the same reason `detect_eras` does: the
    // `initialize` above made a stateful v1 server MINT and store a session, and
    // the GET probes just registered an SSE stream against it. Best effort: a
    // stateless server answers 405 and a v2 connection has no session to tear
    // down.
    teardown_session(tester, era, session).await;

    EraObservations(out)
}

/// Issue the spec's `DELETE` teardown for a session these probes minted.
///
/// Best effort by design: its outcome is not an observation, and a failure to
/// tear down must never change what the probes reported.
async fn teardown_session(tester: &ServerTester, era: Era, session: Option<&str>) {
    let Some(session) = session else {
        return;
    };
    let _ = tester
        .raw_verb_probe(
            "DELETE",
            era,
            &[(pmcp::shared::http_constants::MCP_SESSION_ID, session)],
        )
        .await;
}

/// Shorthand for the transport-failure answer.
fn unavailable(e: &str) -> ObservedValue {
    ObservedValue::Unavailable(e.to_string())
}

/// The standard-header JSON-RPC probe call.
///
/// Six probes below issue byte-identical calls modulo the method and params, so
/// the call SHAPE lives here once. A change to it — a new header mode, a body
/// cap, an extra argument — is then one edit rather than six that must stay in
/// lockstep, where a missed one silently changes what that observation measures.
async fn send_standard(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
    method: &str,
    params: serde_json::Value,
) -> std::result::Result<RawProbeOutcome, String> {
    tester
        .raw_jsonrpc_probe_with_session(method, "", params, era, V2HeaderMode::Standard, session)
        .await
}

/// The shared classification: a `result` means served, any refusal is recorded
/// with its own code so a CHANGED rejection shape shows up as a finding rather
/// than as agreement, and a transport failure stays `Unavailable` (never
/// `Absent` — see `unavailable_is_not_absent`).
fn served_or_error(
    probe: std::result::Result<RawProbeOutcome, String>,
    served: &str,
) -> ObservedValue {
    match probe {
        Ok(o) if o.is_result() => ObservedValue::Text(served.into()),
        Ok(o) => ObservedValue::Text(error_token(o.error_code, o.http_status)),
        Err(e) => unavailable(&e),
    }
}

/// Send a WELL-FORMED `initialize` for `era`.
///
/// The params MUST be well-formed. MEASURED: a request whose params omit
/// `clientInfo`/`capabilities` is refused `-32601` by the TYPED PARSE before
/// dispatch, so a refusal of a malformed request says nothing about whether the
/// METHOD exists — and a probe built that way would report `absent` against a
/// server that serves `initialize` perfectly well.
///
/// Its response answers TWO observations: ERA-01 (was the method served) and
/// ERA-03 (did a session header come back), which is why it is sent once and
/// classified twice.
async fn probe_initialize_raw(
    tester: &ServerTester,
    era: Era,
) -> std::result::Result<crate::tester::RawProbeOutcome, String> {
    // Both sides come from pmcp's own constants. A hardcoded `"2025-11-25"` here
    // rots the moment the SDK's v1 constant moves: the probe would then offer a
    // version the server does not support, be refused, and report ERA-01 as
    // `absent` against every conformant v1 server — a permanent false finding
    // produced by a string literal.
    let version = if era == Era::V2 {
        pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28
    } else {
        pmcp::LATEST_PROTOCOL_VERSION
    };
    let params = serde_json::json!({
        "protocolVersion": version,
        "clientInfo": { "name": "mcp-tester", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": {},
    });
    tester
        .raw_jsonrpc_probe("initialize", "", params, era, V2HeaderMode::Standard)
        .await
}

/// ERA-01. Rule: a JSON-RPC `result` means the method is `served`; anything
/// else means it is `absent`.
fn classify_initialize(
    probe: &std::result::Result<crate::tester::RawProbeOutcome, String>,
) -> ObservedValue {
    match probe {
        Ok(o) if o.is_result() => ObservedValue::Text("served".into()),
        Ok(_) => ObservedValue::Text("absent".into()),
        Err(e) => unavailable(e),
    }
}

/// ERA-03. Rule: an `Mcp-Session-Id` on the initialization response means the
/// server is minting and echoing; its absence means it is doing neither.
fn classify_session_header(
    probe: &std::result::Result<crate::tester::RawProbeOutcome, String>,
) -> ObservedValue {
    match probe {
        Ok(o) if o.session_header.is_some() => ObservedValue::Text("minted-and-echoed".into()),
        Ok(_) => ObservedValue::Text("never-minted-inbound-ignored".into()),
        Err(e) => unavailable(e),
    }
}

/// ERA-02. Rule: a `result` means `served`; the specific `-32601` refusal is
/// recorded as `error:-32601` because that exact code is what the baseline
/// records for v1; any other refusal is recorded with its own code so a
/// changed rejection shape shows up as a finding rather than as agreement.
async fn probe_server_discover(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    served_or_error(
        send_standard(
            tester,
            era,
            session,
            "server/discover",
            serde_json::json!({}),
        )
        .await,
        "served",
    )
}

/// ERA-04. Rule: send a request DELIBERATELY omitting `Mcp-Method` and
/// `Mcp-Name`. If the server still answers normally the headers are not
/// required (`not-sent`); if it refuses, they are required and cross-checked.
///
/// This is the one observation that cannot be made by watching a conformant
/// request — a header that is not required looks exactly like one that is,
/// until you leave it out.
async fn probe_required_headers(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    match tester
        .raw_jsonrpc_probe_with_session(
            "tools/list",
            "",
            serde_json::json!({}),
            era,
            V2HeaderMode::OmitMethodAndName,
            session,
        )
        .await
    {
        Ok(o) if o.is_result() => ObservedValue::Text("not-sent".into()),
        Ok(_) => ObservedValue::Text("required-and-cross-checked".into()),
        Err(e) => unavailable(&e),
    }
}

/// The header list for a `GET` probe, carrying the ALREADY-ESTABLISHED session
/// when there is one.
///
/// A `GET` with no `Mcp-Session-Id` makes a stateful v1 server MINT a fresh
/// session and register an SSE stream against it, neither of which anything ever
/// tears down — one leak per GET probe, per dual run. Reusing the session
/// `observe` already established keeps the count at one, which
/// [`teardown_session`] then deletes.
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
async fn probe_last_event_id(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    match tester
        .raw_verb_probe(
            "GET",
            era,
            &get_probe_headers(session, &[("Last-Event-ID", "0")]),
        )
        .await
    {
        Ok((status, ct)) if status == 200 && ct.contains("text/event-stream") => {
            ObservedValue::Text("supported".into())
        },
        Ok(_) => ObservedValue::Text("ignored".into()),
        Err(e) => unavailable(&e),
    }
}

/// ERA-06. Rule: probe both `GET` and `DELETE`. When BOTH answer `405` the verb
/// surface is rejected; otherwise at least one verb is still serving its v1
/// role (an SSE stream or a session teardown).
///
/// The `GET` carries the established session (see [`get_probe_headers`]); the
/// `DELETE` deliberately does NOT, because a `DELETE` that named the session
/// would tear it down MID-OBSERVATION and every probe after this one would be
/// refused for an unknown session. Teardown is [`teardown_session`]'s job, at
/// the end.
async fn probe_get_delete(tester: &ServerTester, era: Era, session: Option<&str>) -> ObservedValue {
    let get = tester
        .raw_verb_probe("GET", era, &get_probe_headers(session, &[]))
        .await;
    let delete = tester.raw_verb_probe("DELETE", era, &[]).await;
    match (get, delete) {
        (Ok((405, _)), Ok((405, _))) => ObservedValue::Text("status:405".into()),
        (Ok(_), Ok(_)) => ObservedValue::Text("sse-stream-or-session-teardown".into()),
        (Err(e), _) | (_, Err(e)) => unavailable(&e),
    }
}

/// ERA-07. Rule: is `key` a present, non-null member of the JSON-RPC `result`?
fn classify_envelope_key(
    probe: &std::result::Result<crate::tester::RawProbeOutcome, String>,
    key: &str,
) -> ObservedValue {
    match probe {
        Ok(o) => match o.result.as_ref().and_then(|r| r.get(key)) {
            Some(v) if !v.is_null() => ObservedValue::Present,
            _ => ObservedValue::Absent,
        },
        Err(e) => unavailable(e),
    }
}

/// ERA-08. Rule: `serverInfo` may ride at the top of the result or under the
/// result's `_meta` (v2 carries it as reserved response metadata), so BOTH
/// locations count as present. A probe that only looked at the top level would
/// report a permanent false `absent`.
fn classify_server_info(
    probe: &std::result::Result<crate::tester::RawProbeOutcome, String>,
) -> ObservedValue {
    match probe {
        Ok(o) => {
            let Some(result) = o.result.as_ref() else {
                return ObservedValue::Absent;
            };
            let top = result.get("serverInfo").is_some_and(|v| !v.is_null());
            let in_meta = result
                .get("_meta")
                .and_then(|m| m.get("io.modelcontextprotocol/serverInfo"))
                .is_some_and(|v| !v.is_null());
            if top || in_meta {
                ObservedValue::Present
            } else {
                ObservedValue::Absent
            }
        },
        Err(e) => unavailable(e),
    }
}

/// ERA-11. Rule: both `ttlMs` AND `cacheScope` present means the pair is
/// `required`; neither present means `absent`; exactly one is `partial`, which
/// matches no baseline column and so surfaces as a finding — which is correct,
/// because the schema declares both without `?`.
fn classify_cache_hints(
    probe: &std::result::Result<crate::tester::RawProbeOutcome, String>,
) -> ObservedValue {
    match probe {
        Ok(o) => {
            let Some(result) = o.result.as_ref() else {
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
        Err(e) => unavailable(e),
    }
}

/// ERA-09. Rule: a `result` means `served`; a refusal is recorded with its code.
async fn probe_tasks_list(tester: &ServerTester, era: Era, session: Option<&str>) -> ObservedValue {
    served_or_error(
        send_standard(tester, era, session, "tasks/list", serde_json::json!({})).await,
        "served",
    )
}

/// ERA-10. Rule: read the connection's capability structure and report WHERE a
/// tasks surface was found — the v1 spellings (`capabilities.tasks`,
/// `experimental.tasks`) or the v2 extension key. Both spellings present at
/// once, or neither, are reported distinctly so the relocation cannot be
/// half-observed.
///
/// This is the one probe that reads the tester's established projection rather
/// than sending its own request: the capability structure is a property of the
/// connection, and re-fetching it would just repeat ERA-02.
fn probe_tasks_capability_location(tester: &ServerTester) -> ObservedValue {
    let Some(caps) = tester.server_capabilities() else {
        return unavailable("no capability structure on this connection");
    };
    let v1_location = caps.tasks.is_some()
        || caps
            .experimental
            .as_ref()
            .is_some_and(|e| e.contains_key("tasks"));
    let v2_location = caps
        .extensions
        .as_ref()
        .is_some_and(|e| e.contains_key(TASKS_EXTENSION_KEY));
    match (v1_location, v2_location) {
        (true, false) => ObservedValue::Text("capabilities.tasks + experimental.tasks".into()),
        (false, true) => ObservedValue::Text(format!("extensions[{TASKS_EXTENSION_KEY}] = {{}}")),
        (true, true) => ObservedValue::Text("both-locations".into()),
        (false, false) => ObservedValue::Absent,
    }
}

/// ERA-12. Rule: a `result` means `served`; any refusal means `retired`.
async fn probe_resources_subscribe(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    match tester
        .raw_jsonrpc_probe_with_session(
            "resources/subscribe",
            "",
            serde_json::json!({ "uri": "mcp-tester://era-probe" }),
            era,
            V2HeaderMode::Standard,
            session,
        )
        .await
    {
        Ok(o) if o.is_result() => ObservedValue::Text("served".into()),
        Ok(_) => ObservedValue::Text("retired".into()),
        Err(e) => unavailable(&e),
    }
}

/// ERA-13. Rule: `-32601` is recorded as such; anything the server actually
/// serves is the capability-gated stream. A server that is method-aware but
/// capability-gated answers something other than `-32601`, which the baseline
/// note explicitly calls SKIPPED-conformant.
async fn probe_subscriptions_listen(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    // Only a RESULT counts as served (which is what `served_or_error` encodes).
    // Recording any non-`-32601` refusal as "the stream is capability-gated"
    // would read a `-32600` parse failure as a served method — the same
    // mis-inference the malformed `initialize` probe would have made.
    served_or_error(
        send_standard(
            tester,
            era,
            session,
            "subscriptions/listen",
            serde_json::json!({}),
        )
        .await,
        "sse-stream-capability-gated",
    )
}

/// ERA-14. Rule: send a method that cannot exist and read the HTTP STATUS the
/// JSON-RPC error came back under. The legacy table returns `200` for every
/// JSON-RPC-level error; the era-gated table maps the code onto a real HTTP
/// status. So `200` means the legacy table is in force and anything else means
/// the era-gated one is.
///
/// Note this reads the status, NOT the era: it is the mapping that is being
/// observed, and consulting the era to decide would make the observation
/// circular.
async fn probe_error_status_mapping(
    tester: &ServerTester,
    era: Era,
    session: Option<&str>,
) -> ObservedValue {
    match tester
        .raw_jsonrpc_probe_with_session(
            "mcp-tester/definitely-not-a-method",
            "",
            serde_json::json!({}),
            era,
            V2HeaderMode::Standard,
            session,
        )
        .await
    {
        Ok(o) if o.http_status == 200 => ObservedValue::Text("unchanged-legacy-table".into()),
        Ok(_) => ObservedValue::Text("era-gated-table".into()),
        Err(e) => unavailable(&e),
    }
}

/// Render a refusal as a stable token: the JSON-RPC code when there is one,
/// otherwise the HTTP status. Never prose — a message-derived token would be
/// exactly the unstable string matching § Q4.3 forbids.
fn error_token(error_code: Option<i64>, http_status: u16) -> String {
    error_code.map_or_else(|| format!("status:{http_status}"), |c| format!("error:{c}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::era_diff::load_default_baseline;
    use std::collections::BTreeSet;

    /// DIRECTION 1: every baseline entry has a probe.
    ///
    /// A baseline entry nothing observes can only ever be reported MISSING, in
    /// every run, forever — the permanent-false-finding defect the
    /// observation-id design exists to remove.
    #[test]
    fn every_baseline_entry_has_a_probe() {
        let baseline = load_default_baseline().expect("the shipped baseline must load");
        let probes: BTreeSet<&str> = PROBE_REGISTRY.iter().map(|id| id.as_str()).collect();
        let unprobed: Vec<&str> = baseline
            .observation_ids()
            .into_iter()
            .filter(|id| !probes.contains(id))
            .collect();
        assert!(
            unprobed.is_empty(),
            "these baseline observation_ids have NO probe and would report a \
             permanent false MISSING finding: {unprobed:?}"
        );
    }

    /// DIRECTION 2: every probe has a baseline entry.
    ///
    /// A probe with no entry would report every difference it sees as an
    /// UNEXPECTED finding, which is the same defect pointing the other way.
    #[test]
    fn every_probe_has_a_baseline_entry() {
        let baseline = load_default_baseline().expect("the shipped baseline must load");
        let entries: BTreeSet<&str> = baseline.observation_ids().into_iter().collect();
        let unbaselined: Vec<&str> = PROBE_REGISTRY
            .iter()
            .map(|id| id.as_str())
            .filter(|id| !entries.contains(id))
            .collect();
        assert!(
            unbaselined.is_empty(),
            "these probes have NO baseline entry and would report every run as an \
             UNEXPECTED finding: {unbaselined:?}"
        );
    }

    #[test]
    fn the_registry_has_no_duplicates() {
        let unique: BTreeSet<&str> = PROBE_REGISTRY.iter().map(|id| id.as_str()).collect();
        assert_eq!(
            unique.len(),
            PROBE_REGISTRY.len(),
            "a duplicated ObservationId would silently merge two wire facts"
        );
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
    fn error_token_prefers_the_jsonrpc_code() {
        assert_eq!(error_token(Some(-32601), 200), "error:-32601");
        assert_eq!(error_token(None, 404), "status:404");
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

    /// The envelope-key classifier is the shared rule behind ERA-07; it must
    /// treat an explicit `null` as absent, not as present.
    #[test]
    fn envelope_key_classifier_treats_null_as_absent() {
        let present = Ok(crate::tester::RawProbeOutcome {
            http_status: 200,
            session_header: None,
            result: Some(serde_json::json!({ "resultType": "complete" })),
            error_code: None,
            error_message: None,
        });
        assert_eq!(
            classify_envelope_key(&present, "resultType"),
            ObservedValue::Present
        );

        let null = Ok(crate::tester::RawProbeOutcome {
            http_status: 200,
            session_header: None,
            result: Some(serde_json::json!({ "resultType": null })),
            error_code: None,
            error_message: None,
        });
        assert_eq!(
            classify_envelope_key(&null, "resultType"),
            ObservedValue::Absent
        );

        let failed: std::result::Result<crate::tester::RawProbeOutcome, String> =
            Err("boom".into());
        assert!(!classify_envelope_key(&failed, "resultType").is_established());
    }

    #[test]
    fn cache_hint_classifier_distinguishes_partial_from_required() {
        let outcome = |result: serde_json::Value| {
            Ok(crate::tester::RawProbeOutcome {
                http_status: 200,
                session_header: None,
                result: Some(result),
                error_code: None,
                error_message: None,
            })
        };
        assert_eq!(
            classify_cache_hints(&outcome(
                serde_json::json!({ "ttlMs": 0, "cacheScope": "private" })
            )),
            ObservedValue::Text("required".into())
        );
        assert_eq!(
            classify_cache_hints(&outcome(serde_json::json!({}))),
            ObservedValue::Absent
        );
        assert_eq!(
            classify_cache_hints(&outcome(serde_json::json!({ "ttlMs": 0 }))),
            ObservedValue::Text("partial".into()),
            "one hint without the other must not read as conformant"
        );
    }

    #[test]
    fn server_info_classifier_accepts_both_locations() {
        let outcome = |result: serde_json::Value| {
            Ok(crate::tester::RawProbeOutcome {
                http_status: 200,
                session_header: None,
                result: Some(result),
                error_code: None,
                error_message: None,
            })
        };
        assert_eq!(
            classify_server_info(&outcome(
                serde_json::json!({ "serverInfo": { "name": "x" } })
            )),
            ObservedValue::Present
        );
        assert_eq!(
            classify_server_info(&outcome(serde_json::json!({
                "_meta": { "io.modelcontextprotocol/serverInfo": { "name": "x" } }
            }))),
            ObservedValue::Present
        );
        assert_eq!(
            classify_server_info(&outcome(serde_json::json!({}))),
            ObservedValue::Absent
        );
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

        /// `error_token` is TOTAL and always yields one of the two stable
        /// prefixes — never server prose.
        #[test]
        fn error_token_is_always_a_stable_prefix(code in -40000i64..40000, status in 0u16..600) {
            let with_code = error_token(Some(code), status);
            proptest::prop_assert!(with_code.starts_with("error:"));
            let without = error_token(None, status);
            proptest::prop_assert!(without.starts_with("status:"));
        }
    }
}
