//! THE ERA MATRIX — the CONF-02 / CONF-03 era-comparison surface (Phase 118, D-16).
//!
//! One era target, one endpoint, two eras, one baseline join.
//!
//! # What this file is
//!
//! [`era_matrix_is_conformant`] spawns the dual-accept-list era target ONCE,
//! observes it under MCP `2025-11-25` and then under `2026-07-28` over that SAME
//! bound address, and joins the two observation maps against the checked-in
//! expected-difference baseline (`baselines/era-deltas.yaml`). The join is
//! BIDIRECTIONAL: an observed difference with no baseline entry fails, and a
//! baseline entry that no longer reproduces fails too.
//!
//! # What this file deliberately is NOT
//!
//! The 33 in-process fixtures replayed by `tests/conformance.rs` are the
//! **v1-only regression guard** under D-16, and they are deliberately NOT
//! replayed here. No fixture is loaded by this file, and no fixture format was
//! extended to carry an era dimension (D-17).
//!
//! # Why both arms run over real streamable HTTP
//!
//! So that a difference is an ERA difference and not a TRANSPORT difference.
//! Both `observe()` calls — and, in
//! [`deprecated_capabilities_complete_under_both_eras`], both typed clients —
//! cross the same wire against the same listening socket.
//!
//! The in-process route was abandoned for a measured reason:
//! `DuplexTransport` (`crates/pmcp-team-servers/src/transport.rs:47`) never
//! overrides `supports_negotiated_protocol_version`, so it inherits the trait
//! default `false` (`src/shared/transport.rs:351`). Selecting `2026-07-28` over
//! it is therefore INERT — the client stays in v1 emission mode — and
//! `ClientBuilder::build` (`src/client/mod.rs:5213`) says exactly that in a
//! `tracing::warn!`. A matrix built there would have compared v1 against v1 and
//! reported green.
//!
//! # Running it
//!
//! ```text
//! cargo test -p pmcp-team-servers --features http --test era_matrix
//! ```
//!
//! The `--features http` is LOAD-BEARING. Without it this whole file is
//! `cfg`-ed out, the binary reports `running 0 tests`, and cargo exits 0 — a
//! green run that measured nothing.

#![cfg(all(feature = "conformance", feature = "http"))]

use std::collections::BTreeSet;

use pmcp::types::protocol::Era;
use pmcp_team_servers::conformance::era_diff::{
    compare_eras, load_default_baseline, ClassifiedDifference, DifferenceClass, EraBaseline,
    EraComparisonReport,
};
use pmcp_team_servers::conformance::era_observations::{
    observe, EraObservations, ObservationId, ObservedValue, PROBE_REGISTRY,
};
use pmcp_team_servers::conformance::era_probe::EraProbeClient;
use pmcp_team_servers::conformance::era_target::spawn_era_target;

// ===========================================================================
// Non-vacuity floors.
//
// Both are written as LITERALS. A fence parameterised by the thing it checks
// cannot fire — deriving either from the registry's own length or from the
// baseline's own row count would make a shrinking registry (or a shrinking
// baseline) satisfy its own floor. That is the standing `D-115-AI(4)` rule.
// ===========================================================================

/// The number of ids [`PROBE_REGISTRY`] carries, written out as a LITERAL.
///
/// Deliberately NOT the registry's own `.len()`: a probe deleted from the registry
/// would silently lower a length-derived floor and the matrix would keep
/// reporting green over a smaller surface.
///
/// The remedy for a failure here is to restore the probe, NEVER to lower this
/// number.
const MINIMUM_OBSERVED_IDS: usize = 14;

/// Floor on the number of observations that must be classified
/// [`DifferenceClass::Expected`] — i.e. real, measured, baselined era
/// differences.
///
/// Twelve is what plan 118-06 measured over this same target
/// (`118-06-SUMMARY.md`, "the fourteen observed triples": twelve rows carry a
/// `Δ`). A matrix that observed NO difference cannot distinguish "the eras
/// agree" from "the v2 arm never ran", which is exactly the D-16 defect this
/// file exists to detect.
///
/// The remedy for a failure here is to fix the v2 arm, NEVER to delete this
/// assertion or lower the number.
const MINIMUM_ERA_DIFFERENCES: usize = 12;

// ===========================================================================
// Helpers.
// ===========================================================================

/// Render a comparison report into bytes, so a failing run prints the whole
/// picture instead of only the assertion that tripped.
fn render(report: &EraComparisonReport) -> String {
    let mut sink = Vec::<u8>::new();
    report
        .print_to_writer(&mut sink)
        .expect("a Vec<u8> writer cannot fail");
    String::from_utf8_lossy(&sink).into_owned()
}

/// Every id whose value is not ESTABLISHED, rendered with its reason.
///
/// An [`ObservedValue::Unavailable`] is a defect in the PROBE, not a finding
/// about the server: it means the probe ran and could not tell. Recording one as
/// a token would manufacture a difference (or erase one), so the matrix refuses
/// to classify a run that contains any.
fn unestablished(observed: &EraObservations) -> Vec<String> {
    observed
        .0
        .iter()
        .filter(|(_, value)| !value.is_established())
        .map(|(id, value)| match value {
            ObservedValue::Unavailable(reason) => format!("{id} -> Unavailable: {reason}"),
            other => format!("{id} -> {}", other.token()),
        })
        .collect()
}

/// One finding rendered for a failure message: the id, both observed tokens,
/// and — where the baseline has an entry — the tokens it records.
fn describe(row: &ClassifiedDifference, baseline: &EraBaseline) -> String {
    let recorded = baseline
        .find_by_observation_id(&row.observation_id)
        .map_or_else(
            || "no baseline entry".to_string(),
            |delta| format!("{} records v1 `{}` / v2 `{}`", delta.id, delta.v1, delta.v2),
        );
    let token = |v: Option<&ObservedValue>| {
        v.map_or_else(|| "not observed".to_string(), ObservedValue::token)
    };
    format!(
        "  [{}] {} — observed v1 `{}` / v2 `{}`; {recorded}",
        row.class.label(),
        row.observation_id,
        token(row.v1.as_ref()),
        token(row.v2.as_ref()),
    )
}

/// Fail unless every registry id was observed under `era` and every value is
/// established.
fn assert_observation_floor(observed: &EraObservations, era: Era, rendered: &str) {
    assert!(
        observed.len() >= MINIMUM_OBSERVED_IDS,
        "FAILURE MODE: the {era:?} arm observed {} id(s); the floor is {MINIMUM_OBSERVED_IDS}.\n\
         CONSEQUENCE: a matrix over a shrinking surface keeps reporting green while measuring \
         less, so \"dual conformance\" quietly means less than it did last commit.\n\
         WHAT TO DO: restore the missing probe. NEVER lower {MINIMUM_OBSERVED_IDS}, and never \
         derive it from the registry's own length.\n{rendered}",
        observed.len()
    );

    let broken = unestablished(observed);
    assert!(
        broken.is_empty(),
        "FAILURE MODE: these observations were NOT established under {era:?}:\n  {}\n\
         CONSEQUENCE: an Unavailable value is a PROBE defect, not a server finding — the probe \
         ran and could not tell. Classifying one would manufacture or erase a difference.\n\
         WHAT TO DO: fix the probe named above; do NOT record Unavailable as a token.\n{rendered}",
        broken.join("\n  ")
    );
}

/// Fail on any UNEXPECTED row — an observed era difference the baseline does not
/// record, or records with different tokens.
fn assert_no_unexpected(report: &EraComparisonReport, baseline: &EraBaseline, rendered: &str) {
    let unexpected: Vec<String> = report
        .differences
        .iter()
        .filter(|row| row.class == DifferenceClass::Unexpected)
        .map(|row| describe(row, baseline))
        .collect();
    assert!(
        unexpected.is_empty(),
        "FAILURE MODE: {} observed era difference(s) are NOT recorded in the baseline:\n{}\n\
         CONSEQUENCE: an undocumented v1/v2 difference is either a regression or an undeclared \
         behaviour change; either way the written statement of what \"dual-version\" means is now \
         wrong.\n\
         WHAT TO DO: fix the server, or add a CITED row to baselines/era-deltas.yaml explaining \
         why the difference is correct by design. Do NOT widen the baseline to absorb a \
         regression.\n{rendered}",
        unexpected.len(),
        unexpected.join("\n")
    );
}

/// The `kind` marking a row that records a MEASURED SAMENESS rather than a
/// measured difference.
///
/// Kept in step with `baselines/era-deltas.yaml` and with
/// `tests/era_baseline.rs`, which caps how many rows may carry it.
const AGREEMENT_KIND: &str = "era-agreement";

/// Is this MISSING row an `era-agreement` row whose measured sameness still
/// holds EXACTLY as recorded?
///
/// Three independent conditions, all required:
///
/// 1. the baseline row's `kind` is exactly [`AGREEMENT_KIND`];
/// 2. the row's own `v1` and `v2` tokens are EQUAL — an "agreement" row that
///    records a difference is a contradiction and is never exempt;
/// 3. BOTH observed tokens equal that recorded value.
///
/// (3) is what stops this from being an allowlist. The row fires in both
/// directions: the eras starting to differ makes `compare_eras` classify
/// UNEXPECTED (never MISSING, so this function is not even consulted), and
/// either token MOVING while they still agree fails (3) here.
fn agreement_still_holds(row: &ClassifiedDifference, baseline: &EraBaseline) -> bool {
    let Some(delta) = baseline.find_by_observation_id(&row.observation_id) else {
        return false;
    };
    if delta.kind != AGREEMENT_KIND || delta.v1 != delta.v2 {
        return false;
    }
    let observed = |value: Option<&ObservedValue>| value.map(ObservedValue::token);
    observed(row.v1.as_ref()) == Some(delta.v1.clone())
        && observed(row.v2.as_ref()) == Some(delta.v2.clone())
}

/// Fail on any MISSING row that is neither provisional nor a still-holding
/// `era-agreement`.
///
/// The `provisional` exemption is applied HERE, by the consumer, and never
/// inside `compare_eras` — a comparator that skipped provisional rows would make
/// a stale one permanently invisible. Plan 118-07 Task 2 reconciled every row
/// against measurement, so no provisional row remains and that arm is dead by
/// construction; [`era_matrix_is_conformant`] asserts exactly that.
fn assert_no_missing(report: &EraComparisonReport, baseline: &EraBaseline, rendered: &str) {
    let missing: Vec<String> = report
        .differences
        .iter()
        .filter(|row| row.class == DifferenceClass::Missing)
        .filter(|row| !row.provisional && !agreement_still_holds(row, baseline))
        .map(|row| describe(row, baseline))
        .collect();
    assert!(
        missing.is_empty(),
        "FAILURE MODE: {} baseline entr(y/ies) no longer reproduce:\n{}\n\
         CONSEQUENCE: a documented era difference that stopped happening is a FINDING in the same \
         way an undocumented one is — either the spec moved, or the SDK regressed, and the \
         baseline is now a claim nothing checks.\n\
         WHAT TO DO: re-measure the fact, then either fix the server or update the row WITH its \
         citation. Do NOT delete the row to restore green — the two-direction coverage test in \
         tests/era_baseline.rs fails on an orphaned probe, and do NOT reach for \
         `kind: era-agreement` unless the two eras genuinely measure the SAME token (that kind \
         is capped in tests/era_baseline.rs and asserted against the observation, not just the \
         file).\n{rendered}",
        missing.len(),
        missing.join("\n")
    );
}

/// Fail if any baseline row is still PROVISIONAL, or if any provisional row went
/// MISSING.
///
/// Plan 118-07 measured every seeded claim, so the provisional exemption in
/// [`assert_no_missing`] is now dead by construction rather than merely unused.
/// This asserts that, because a standing exemption nothing exercises is exactly
/// how a stale row becomes permanently invisible.
fn assert_the_provisional_exemption_is_dead(report: &EraComparisonReport, baseline: &EraBaseline) {
    let still_provisional: Vec<&str> = baseline
        .deltas
        .iter()
        .filter(|delta| delta.provisional)
        .map(|delta| delta.id.as_str())
        .collect();
    assert!(
        still_provisional.is_empty(),
        "FAILURE MODE: {still_provisional:?} in baselines/era-deltas.yaml are still marked \
         provisional after plan 118-07 reconciled the file against measurement.\n\
         CONSEQUENCE: a provisional row is EXEMPT from the MISSING arm above, so it is a claim \
         nothing checks — worse than no claim.\n\
         WHAT TO DO: measure the row and clear the flag, or delete the row AND its probe."
    );

    let provisional_missing = report
        .differences
        .iter()
        .filter(|row| row.class == DifferenceClass::Missing && row.provisional)
        .count();
    assert_eq!(
        provisional_missing, 0,
        "FAILURE MODE: {provisional_missing} provisional baseline entr(y/ies) went MISSING.\n\
         CONSEQUENCE: the exemption in assert_no_missing is live again, which means a real \
         regression can hide behind an unsigned-off flag.\n\
         WHAT TO DO: measure the row and clear its provisional flag."
    );
}

// ===========================================================================
// THE MATRIX.
// ===========================================================================

/// ONE spawned target, TWO observation runs, ONE bidirectional baseline join.
///
/// The target is spawned exactly once and its bound address is read before the
/// first `observe()` and again after the second; the two reads MUST be equal.
/// That is this crate's analog of the sibling plan's PID check: it is what
/// proves both eras were measured against the same live process rather than
/// against two servers that merely looked alike.
#[tokio::test]
async fn era_matrix_is_conformant() {
    let target = spawn_era_target().await.expect("the era target binds");
    let bound_before = target.addr();
    let probe = EraProbeClient::new(target.url().as_str()).expect("the probe client builds");

    // ONE endpoint, ONE transport, two eras — read once, used twice.
    let v1 = observe(&probe, Era::V1).await;
    let v2 = observe(&probe, Era::V2).await;
    let bound_after = target.addr();

    assert_eq!(
        bound_before, bound_after,
        "FAILURE MODE: the era target's bound address changed between the v1 and the v2 \
         observation run ({bound_before} then {bound_after}).\n\
         CONSEQUENCE: the two arms did not measure the same process, so every difference below \
         could be a difference between two servers rather than between two eras.\n\
         WHAT TO DO: spawn the target ONCE and reuse the handle; do not restart it between arms."
    );

    let baseline = load_default_baseline().expect("the shipped baseline parses");
    let (differences, suspicion) = compare_eras(&v1, &v2, &baseline);

    let report = EraComparisonReport {
        schema_version: 1,
        era_support: "dual".to_string(),
        v1_observations: v1.clone(),
        v2_observations: v2.clone(),
        differences,
        suspicion,
        note: None,
    };
    let rendered = render(&report);

    assert_observation_floor(&v1, Era::V1, &rendered);
    assert_observation_floor(&v2, Era::V2, &rendered);

    // THE ANTI-VACUITY GUARD, CONSUMED. `compare_eras` sets `suspicion` when the
    // classified list is EMPTY. A matrix that observed nothing must not be able
    // to report all-green.
    assert!(
        report.suspicion.is_none(),
        "FAILURE MODE: {}\n\
         CONSEQUENCE: an empty difference list is indistinguishable from success, so the matrix \
         would certify \"dual conformance\" having compared nothing.\n\
         WHAT TO DO: fix the arm that produced no observations; do NOT ignore the suspicion \
         field.\n{rendered}",
        report.suspicion.as_deref().unwrap_or_default()
    );

    assert_no_unexpected(&report, &baseline, &rendered);
    assert_no_missing(&report, &baseline, &rendered);
    assert_the_provisional_exemption_is_dead(&report, &baseline);

    let expected = report.count(DifferenceClass::Expected);
    assert!(
        expected >= MINIMUM_ERA_DIFFERENCES,
        "FAILURE MODE: only {expected} observation(s) were classified EXPECTED; the floor is \
         {MINIMUM_ERA_DIFFERENCES}.\n\
         CONSEQUENCE: too few measured era differences means the v2 arm is partly (or wholly) \
         inert, and the matrix is agreeing with the plan rather than with the server.\n\
         WHAT TO DO: fix the v2 arm. NEVER lower {MINIMUM_ERA_DIFFERENCES}.\n{rendered}"
    );
}

/// Every registry id is answered under BOTH eras, and every value is
/// established.
///
/// Separate from [`era_matrix_is_conformant`] on purpose: a PROBE defect and a
/// BASELINE defect then fail under different test names, so the failing name
/// already tells a reader which half to go and look at.
#[tokio::test]
async fn era_matrix_observes_every_registry_id() {
    let target = spawn_era_target().await.expect("the era target binds");
    let probe = EraProbeClient::new(target.url().as_str()).expect("the probe client builds");

    let v1 = observe(&probe, Era::V1).await;
    let v2 = observe(&probe, Era::V2).await;

    let expected: BTreeSet<ObservationId> = PROBE_REGISTRY.iter().copied().collect();
    for (observed, era) in [(&v1, Era::V1), (&v2, Era::V2)] {
        let seen: BTreeSet<ObservationId> = observed.ids().into_iter().collect();
        let absent: Vec<&str> = expected.difference(&seen).map(|id| id.as_str()).collect();
        let extra: Vec<&str> = seen.difference(&expected).map(|id| id.as_str()).collect();
        assert!(
            absent.is_empty() && extra.is_empty(),
            "FAILURE MODE: the {era:?} observation map does not EQUAL the probe registry — \
             missing {absent:?}, unexpected {extra:?}.\n\
             CONSEQUENCE: a registry id nobody answers reports a permanent false MISSING; an id \
             nobody registered reports a permanent false UNEXPECTED. Either trains a reviewer to \
             ignore the gate.\n\
             WHAT TO DO: add (or remove) the probe in src/conformance/era_observations.rs so the \
             two sets agree."
        );

        let broken = unestablished(observed);
        assert!(
            broken.is_empty(),
            "FAILURE MODE: these {era:?} observations are not established:\n  {}\n\
             CONSEQUENCE: Unavailable is \"the probe could not tell\", which is a probe defect.\n\
             WHAT TO DO: fix the probe named above.",
            broken.join("\n  ")
        );
    }

    target.shutdown();
}

// ===========================================================================
// CONF-03 — the completion round trips (D-10, D-12, D-17).
//
// D-10: CONF-02 and CONF-03 are discharged by ONE mechanism — the same spawned
// target, the same test file — rather than by a second surface that rots
// independently of the first.
//
// D-12: "fully functional" means the capability is reachable VIA ITS ERA'S OWN
// MECHANISM. Under v2 that is the `_meta` key `io.modelcontextprotocol/logLevel`
// for Logging and `InputRequiredResult` (SEP-2322) for Sampling and Roots. The
// v1 RPC shapes stay green under v1 negotiation only.
//
// D-17: the evidence is PROBES plus TYPED CLIENT round trips, never a fixture.
// The fixture grammar under `contracts/team-servers/fixtures/` supports only
// `tools_list` and a single `tool_call`, so it cannot express a preceding
// `logging/setLevel`, a host-handler installation, a server->client exchange, or
// an MRTR gather/resend. THE FIXTURE FORMAT WAS NOT EXTENDED BY THIS PLAN, and
// no `deprecated-caps/` fixture directory was created.
// ===========================================================================

// ---------------------------------------------------------------------------
// INDEPENDENT ORACLE — spelled here, deliberately NOT imported.
//
// `era_target.rs`'s banner says every wire string is spelled once and imported;
// `era_observations.rs` follows that rule. This file deliberately does not, and
// the reason is the one already recorded for `LOG_LEVEL_META_KEY`: these tests
// assert the WIRE CONTRACT. Importing the era target's own constant would make
// each assertion agree with the target BY CONSTRUCTION — the target could
// rename a field on the wire and every test here would follow it silently.
//
// The rationale is stated once, here, for all eight. The names below MATCH
// `era_target.rs`'s exactly so the relationship stays greppable: changing a
// field in the target and grepping its constant name finds these assertions.
// (They previously carried different names, which hid exactly that link.)
// ---------------------------------------------------------------------------

/// The tool-result field the era target reports the observed log level under.
const LOG_RESULT_LEVEL_FIELD: &str = "observedLevel";
/// The tool-result field naming WHERE that level came from.
const LOG_RESULT_SOURCE_FIELD: &str = "levelSource";
/// The `levelSource` value meaning "the per-request v2 `_meta` key set it".
const LOG_LEVEL_SOURCE_REQUEST_META: &str = "request-meta";
/// The `levelSource` value meaning "no `_meta` key applied; the server default".
const LOG_LEVEL_SOURCE_SERVER_DEFAULT: &str = "server-default";
/// The `status` field both continuation tools report under.
const DEP_RESULT_STATUS_FIELD: &str = "status";
/// `status` when the whole capability round trip finished.
const DEP_STATUS_COMPLETED: &str = "completed";
/// `status` when the era target declined to reach for the capability at all.
///
/// No longer the EXPECTED v1 value — phase 118.1 closed G-3's server half — but
/// still named here because it is one of the two regressions the tripwire below
/// must be able to describe: a return to this value means the peer went absent
/// again.
const DEP_STATUS_CAPABILITY_NOT_OFFERED: &str = "capability-not-offered";
/// `status` when the peer WAS present, the request WAS issued, and the answer
/// never came back.
///
/// The v1 expectation since phase 118.1 plan 11, and still the v1 expectation
/// after phase 118.2 — but **for a different reason**, which is why the arm below
/// now also asserts [`DEP_RESULT_DETAIL_FIELD`]. The wire spelling is a
/// CATCH-ALL: `era_target::undelivered()` reports it for any peer error at all,
/// so it cannot by itself distinguish "there was no stream to deliver on" (the
/// pre-118.2 state) from "it was delivered and nobody answered" (now). The name
/// is kept matching `era_target`'s so the two stay greppable; the module doc
/// carries the account of what moved.
///
/// Re-spelled here rather than imported for the same reason every other token in
/// this block is: a test that reads its expectation out of the code under test
/// asserts nothing.
const DEP_STATUS_NO_LIVE_STREAM: &str = "no-live-stream";

/// The result field carrying the transport error text behind
/// [`DEP_STATUS_NO_LIVE_STREAM`].
const DEP_RESULT_DETAIL_FIELD: &str = "detail";

/// What the `detail` must NAME now: the server waited out its dispatch budget.
///
/// A substring rather than the whole message, because the request id inside it
/// (`dispatch-1`) is a counter this file has no business pinning.
const DISPATCH_TIMEOUT_MARKER: &str = "timed out";

/// What the `detail` said at the phase base `cb5d1365`, and must NOT say now.
///
/// Asserted in the NEGATIVE alongside [`DISPATCH_TIMEOUT_MARKER`], because
/// "which hop is missing" is a property of the PAIR: a one-directional check
/// would pass against an implementation whose error text happened to contain
/// both. This is the marker that makes the detail assertion non-vacuous — it is
/// exactly what the base tree reported, in 0.18 s, when no live client stream
/// existed.
const BASE_FAIL_FAST_MARKER: &str = "Dispatch oneshot channel closed";

/// The per-request v2 `_meta` key that REPLACES the `logging/setLevel` RPC.
const LOG_LEVEL_META_KEY: &str = "io.modelcontextprotocol/logLevel";

/// The level the v2 arm asks for. Deliberately NOT the era target's default
/// (`info`), so an honoured `_meta` key and an ignored one cannot produce the
/// same reported level by coincidence.
const PROBED_LEVEL: &str = "debug";

/// A fixed, offline, deterministic sampling completion.
///
/// No network, no clock, no randomness — the same shape as the `EndTurnMock`
/// completion source at `crates/pmcp-team-servers/tests/conformance.rs:312-329`.
struct FixedCompletion;

#[async_trait::async_trait]
impl pmcp::client::host::HostSamplingHandler for FixedCompletion {
    async fn handle_create_message(
        &self,
        _params: pmcp::types::sampling::CreateMessageParams,
    ) -> pmcp::Result<pmcp::types::sampling::CreateMessageResult> {
        Ok(pmcp::types::sampling::CreateMessageResult::new(
            pmcp::types::Content::text("era matrix fixed completion"),
            FIXED_MODEL,
        ))
    }
}

/// The model name the fixed completion reports. Asserted on, so a handler that
/// silently stopped being consulted cannot pass.
const FIXED_MODEL: &str = "era-matrix-model";

/// The single fixed root the roots provider answers with.
const FIXED_ROOT_URI: &str = "file:///era-matrix";

/// The fixed, offline roots answer.
async fn fixed_roots() -> pmcp::Result<pmcp::types::roots::ListRootsResult> {
    Ok(pmcp::types::roots::ListRootsResult {
        roots: vec![pmcp::types::roots::Root {
            uri: FIXED_ROOT_URI.to_string(),
            name: Some("era-matrix".to_string()),
        }],
    })
}

/// Read a tool result's JSON payload — `structuredContent` when the tool
/// declares an output schema, otherwise the text voice parsed back.
///
/// Asserting on the PAYLOAD rather than on "the server returned something" is
/// what makes this a COMPLETION check: a test that only checked for a response
/// would pass against a server that answered with an error result.
fn payload(result: &pmcp::types::CallToolResult) -> serde_json::Value {
    assert!(
        !result.is_error,
        "FAILURE MODE: the tool call returned an ERROR result: {result:?}\n\
         CONSEQUENCE: CONF-03 claims the capability is REACHABLE; an error result is the \
         opposite of reachable.\n\
         WHAT TO DO: fix the server or the client wiring; do not relax this to `is_error || ok`."
    );
    if let Some(structured) = result.structured_content.as_ref() {
        return structured.clone();
    }
    for item in &result.content {
        if let pmcp::types::Content::Text { text } = item {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                return value;
            }
        }
    }
    panic!(
        "FAILURE MODE: the tool result carried no readable JSON payload: {result:?}\n\
         WHAT TO DO: the era target's tools all return a JSON object; find out what replaced it."
    )
}

/// Assert one string field of a tool payload.
fn assert_field(payload: &serde_json::Value, field: &str, expected: &str, context: &str) {
    let observed = payload.get(field).and_then(serde_json::Value::as_str);
    assert_eq!(
        observed,
        Some(expected),
        "FAILURE MODE: {context}: expected `{field}` = `{expected}`, observed {observed:?}.\n\
         CONSEQUENCE: the capability did not travel through the mechanism this era is supposed \
         to reach it by, so the CONF-03 claim would be asserted over a different code path than \
         the one it names.\n\
         WHAT TO DO: read the payload below and fix the arm that produced it.\n  {payload}"
    );
}

/// Build the ONLY transport permitted for an era arm.
///
/// `StreamableHttpTransport::supports_negotiated_protocol_version` returns
/// `true` (`src/shared/streamable_http.rs:1779`) — it is the one client
/// transport in this SDK with a wire representation for `2026-07-28`.
fn era_transport(url: &url::Url) -> pmcp::shared::streamable_http::StreamableHttpTransport {
    pmcp::shared::streamable_http::StreamableHttpTransport::new(
        pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder::new(url.clone())
            .build(),
    )
}

/// Roots, Sampling and Logging reached through EACH ERA'S OWN MECHANISM, over
/// the same live process the matrix measures.
///
/// One `spawn_era_target()` serves BOTH typed clients; the bound address is read
/// before the first client is built and again after the last call, and the two
/// reads must be equal. Both clients run over `StreamableHttpTransport` — the
/// only client transport that carries `2026-07-28` — and the v2 client asserts
/// that positively BEFORE it is built, so a future transport swap fails loudly
/// here instead of silently degrading to v1.
///
/// # What the two tripwired arms assert NOW
///
/// Phase 118 decision D-21 scopes the conformance claim to what genuinely
/// passes and DECLARES the rest, so these arms assert measured reality with a
/// tripwire message rather than a seeded expectation. What that reality IS moved
/// during phase 118.1; the history is kept so a reader can tell a fix from a
/// regression.
///
/// * **v2 `logging/setLevel` is RETIRED, at the transport gate AND at both
///   native dispatch roots.** Phase 118.1 plan 05 retired it by method STRING at
///   the v2 HTTP ingress (the `V2_RETIRED_METHODS` table), which is what the
///   arm below asserts on the wire. Until phase 118.2-08 that was the ONLY
///   layer that knew: `src/server/mod.rs` lumped `SetLoggingLevel` in with
///   `Subscribe`/`Unsubscribe`/`Ping` and answered `Ok(json!({}))` with no era
///   branch anywhere in `src/`, while `ServerCore`'s `_ =>` catch-all answered
///   `-32601` on BOTH eras — so a caller reaching a dispatch root off the HTTP
///   path got an answer the suite had never measured. 118.2-08 (D-13) split the
///   method out of that residual arm and gave both roots ONE era-branched
///   shared unit, `server::core::set_logging_level_response`: a literal `{}` on
///   v1, `-32601` on v2. The REPLACEMENT mechanism works too — the `_meta` arm
///   below proves it. (Gap G-5 in `118-CONFORMANCE-GAPS.md` was CLOSED by phase
///   118.1 plan 05 for the verbs it named; the residual `logging/setLevel`
///   dispatch-root observation that survived it is closed by 118.2-08.)
/// * **v1 Sampling/Roots REACH the capability over `StreamableHttpServer`, the
///   request is now DELIVERED to pmcp's own client, and the round trip still
///   stops one hop short — at the ANSWER.** The status this file asserts is
///   `no-live-stream`, not `capability-not-offered`, and the difference is the
///   whole of gap G-3's server half:
///   - phase 118.1 plan 08 folded the v1 `initialize` handshake capabilities
///     into the per-request context, so `client_capabilities()` is populated on
///     v1 (it was previously fed only by the v2 `_meta` reserved key — gap G-9);
///   - plans 10 and 11 put a SESSION-BOUND peer handle on the `StreamableHTTP`
///     dispatch path, so `extra.peer()` is `Some` (it was previously set only in
///     `Server::run()`, which this transport never calls).
///
///   **What phase 118.2 FIXED, and what it did not.** pmcp's own
///   `StreamableHttpTransport` now opens the GET session stream after the
///   `initialized` notification is answered `202`, and reads that body
///   INCREMENTALLY. Both halves of that were defects and both are closed:
///   118.2-01 found the `start_sse(None)` call sitting inside
///   `if !response.status().is_success()` — and `202 Accepted` IS a success
///   status, so the branch was dead and no GET was ever issued; 118.2-03 replaced
///   the whole-body `collect()` at both read sites with the one incremental
///   reader, because a session stream has no end-of-body for a `collect()` to
///   wait for. 118.2-04 added bounded reconnect with `Last-Event-ID` on top.
///   The pmcp-on-BOTH-ends proof of the resulting channel is
///   `tests/pmcp_both_ends_logging.rs` in the `pmcp` crate: a handler-emitted
///   `notifications/message` reaching a real `StreamableHttpTransport` over a
///   live v1 session stream, before the call's own reply.
///
///   **The residual, and it MOVED rather than vanished.** Measured at the phase
///   base `cb5d1365` and again at 118.2-10, with this arm's expectation
///   temporarily flipped so the payload printed:
///   - BASE, in 0.18 s: `detail: "Protocol error: -32603 - Dispatch oneshot
///     channel closed"` — there was no live client stream, so the server failed
///     the correlation AT ONCE;
///   - NOW, in ~30 s: `detail: "Protocol error: -32001 - Server request
///     dispatch-1 timed out"` — the request IS delivered (a scratch probe read
///     `Request { id: "dispatch-1", request: Client(CreateMessage(..)) }` off
///     pmcp's own client queue) and the server then waits out its dispatch budget
///     for an answer that never comes.
///   The client cannot ANSWER because `Client::dispatch_request` awaits
///   `transport.send(..)` to COMPLETE before entering the receive loop that would
///   dispatch the inbound request, and the server holds the `tools/call` POST open
///   for the whole handler — so the client is parked inside its own `send()` while
///   the handler waits on it. That is a lifecycle deadlock in `src/`, not a
///   missing stream, and closing it is a design decision (overlap the client's
///   send with its receive loop, or answer the request POST `202` before running
///   the handler) which 118.2-10 recorded in the phase's `deferred-items.md`
///   rather than smuggling in behind a test change. `DEP_STATUS_NO_LIVE_STREAM` is
///   the value `era_target::undelivered()` reports for ANY peer error, so the wire
///   spelling did not have to move for the CAUSE to; that is exactly why the arm
///   below now also asserts the `detail`.
///
///   The SERVER half is measured directly, and
///   green, by `tests/http_peer_roundtrip.rs` in the `pmcp` crate, which drives
///   `sample`, `list_roots` and `elicit` to completion over v1 HTTP with a client
///   that does hold a live stream. The v1 MECHANISM in-process is proved by
///   [`v1_sampling_and_roots_complete_via_server_to_client_requests`].
#[tokio::test]
async fn deprecated_capabilities_complete_under_both_eras() {
    let target = spawn_era_target().await.expect("the era target binds");
    let bound_before = target.addr();

    v1_capability_arm(target.url()).await;
    v2_capability_arm(target.url()).await;

    assert_eq!(
        bound_before,
        target.addr(),
        "FAILURE MODE: the era target's bound address changed between the v1 and the v2 \
         capability arm.\n\
         CONSEQUENCE: the two arms did not exercise the same process, so a per-era difference \
         could be a per-server difference.\n\
         WHAT TO DO: spawn the target ONCE and reuse the handle."
    );
    target.shutdown();
}

/// The v1 arm: `logging/setLevel` over the RPC, and the two continuation tools.
async fn v1_capability_arm(url: &url::Url) {
    let mut client = pmcp::ClientBuilder::new(era_transport(url))
        .on_sampling(FixedCompletion)
        .on_roots(fixed_roots)
        .build();
    client
        .initialize(pmcp::types::ClientCapabilities::default())
        .await
        .expect("the v1 client handshakes with the era target");

    // LOGGING, v1 mechanism: the RPC is served and succeeds.
    client
        .set_logging_level(pmcp::types::notifications::LoggingLevel::Debug)
        .await
        .expect("v1 logging/setLevel is served");

    // The level the tool reports did NOT come from `_meta` — the v2 key is a
    // 2026-07-28 mechanism and a v1 request carrying it is carrying a key its
    // own era does not define, so the target ignores it (baseline row ERA-12).
    let logged = payload(
        &client
            .call_tool("dep__log_emit".to_string(), serde_json::json!({}))
            .await
            .expect("v1 dep__log_emit completes"),
    );
    assert_field(
        &logged,
        LOG_RESULT_SOURCE_FIELD,
        LOG_LEVEL_SOURCE_SERVER_DEFAULT,
        "v1 dep__log_emit",
    );
    assert!(
        logged
            .get(LOG_RESULT_LEVEL_FIELD)
            .and_then(serde_json::Value::as_str)
            != Some(PROBED_LEVEL),
        "FAILURE MODE: the v1 arm reported the level the v2 `_meta` key asks for.\n\
         CONSEQUENCE: `{LOG_LEVEL_META_KEY}` would then be honoured under BOTH eras and baseline \
         row ERA-12 (`ignored` -> `honored`) could never be observed.\n\
         WHAT TO DO: re-read the era gate in the target's log tool.\n  {logged}"
    );

    // SAMPLING and ROOTS, v1 mechanism, over StreamableHttpServer. The target
    // REACHES for the capability — phase 118.1 plans 08/10/11 closed the server
    // half of G-3 — and since phase 118.2 the request is also DELIVERED to
    // pmcp's own client over a live GET SSE stream. What is still missing is the
    // ANSWER, so the server waits out its dispatch budget. See the module doc
    // for the measured before/after and for why the wire STATUS did not move.
    for tool in ["dep__request_sampling", "dep__list_roots"] {
        let result = payload(
            &client
                .call_tool(tool.to_string(), serde_json::json!({}))
                .await
                .unwrap_or_else(|error| panic!("v1 {tool} completes as a tool call: {error}")),
        );
        assert_field(
            &result,
            DEP_RESULT_STATUS_FIELD,
            DEP_STATUS_NO_LIVE_STREAM,
            &format!(
                "v1 {tool} over StreamableHttpServer. Two regressions are possible here and they \
                 mean OPPOSITE things.\n\
                 IF THIS REPORTS `{DEP_STATUS_CAPABILITY_NOT_OFFERED}`: the peer handle has gone \
                 ABSENT from the HTTP dispatch path again, or the v1 handshake capabilities have \
                 stopped reaching the request context. That is a SERVER REGRESSION of gap G-3 — \
                 re-run `cargo nextest run -E 'binary(http_peer_roundtrip)'`, which measures the \
                 server half directly and must be green.\n\
                 IF THIS REPORTS `{DEP_STATUS_COMPLETED}`: a pmcp CLIENT has learned to ANSWER a \
                 server-to-client request issued while its own call is outstanding, so the round \
                 trip now lands. That is good news — update THIS assertion and the detail \
                 assertion below it, close the client-lifecycle deferred item, and re-measure \
                 baseline rows ERA-13/ERA-14."
            ),
        );

        // THE DETAIL. The status alone is a catch-all — `undelivered()` reports
        // it for any peer error — so it cannot tell "no stream existed" from
        // "delivered, unanswered". Pinning the detail is what makes this arm
        // record WHICH hop is missing, and it is measurably non-vacuous: at the
        // phase base `cb5d1365` this same field read
        // `BASE_FAIL_FAST_MARKER` in 0.18 s.
        let detail = result
            .get(DEP_RESULT_DETAIL_FIELD)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "FAILURE MODE: v1 {tool} reported `{DEP_STATUS_NO_LIVE_STREAM}` with no \
                     `{DEP_RESULT_DETAIL_FIELD}`.\n\
                     CONSEQUENCE: the evidence for WHY the round trip did not complete is gone, \
                     and the status alone cannot distinguish an undelivered request from an \
                     unanswered one.\n\
                     WHAT TO DO: restore the detail field in `era_target::undelivered()`.\n  \
                     {result}"
                )
            });
        assert!(
            detail.contains(DISPATCH_TIMEOUT_MARKER) && !detail.contains(BASE_FAIL_FAST_MARKER),
            "FAILURE MODE: v1 {tool} did not fail the way phase 118.2 left it failing.\n\
             EXPECTED: a detail containing `{DISPATCH_TIMEOUT_MARKER}` and NOT containing \
             `{BASE_FAIL_FAST_MARKER}` — the request is delivered on a live stream and the \
             server waits out its dispatch budget for an answer.\n\
             IF IT NOW CONTAINS `{BASE_FAIL_FAST_MARKER}`: the CLIENT has stopped holding a live \
             GET SSE stream and the server is failing the correlation at once again. That is a \
             REGRESSION of phase 118.2's client half — re-run \
             `cargo nextest run -E 'binary(client_sse_stream)'` and \
             `-E 'binary(pmcp_both_ends_logging)'` in the `pmcp` crate.\n\
             OBSERVED: {detail}"
        );
    }
}

/// The v2 arm: the `_meta` log-level key, and both `InputRequiredResult`
/// continuations driven to completion by the client's MRTR loop.
async fn v2_capability_arm(url: &url::Url) {
    let transport = era_transport(url);

    // THE TRIPWIRE. `ClientBuilder::build` (`src/client/mod.rs:5213`) only
    // WARNS when a v2 selection lands on a transport with no wire
    // representation for it, and a warning in a test run is a warning nobody
    // reads. Asserting it here means a future swap to the plain HTTP transport
    // (`src/shared/http.rs:476`, which does not override this method and so
    // inherits the trait default `false` from `src/shared/transport.rs:351`)
    // fails LOUDLY instead of silently measuring v1 twice.
    assert!(
        pmcp::shared::Transport::supports_negotiated_protocol_version(&transport),
        "FAILURE MODE: the transport chosen for the v2 arm has NO wire representation for \
         2026-07-28, so `with_protocol_version` is INERT and this arm would measure v1.\n\
         CONSEQUENCE: every v2 claim below would be a v1 measurement wearing a v2 label — the \
         exact D-16 defect this file exists to prevent.\n\
         WHAT TO DO: use StreamableHttpTransport. The plain HTTP transport at \
         src/shared/http.rs:476 does not override supports_negotiated_protocol_version and \
         would degrade silently."
    );

    let client = pmcp::ClientBuilder::new(transport)
        .with_protocol_version(pmcp::types::protocol::ProtocolVersion(
            pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
        ))
        .expect("2026-07-28 is a selectable protocol version")
        .on_sampling(FixedCompletion)
        .on_roots(fixed_roots)
        .build();

    // LOGGING, v2 mechanism: the per-request `_meta` key that the 2026-07-28
    // schema describes as replacing the `logging/setLevel` RPC.
    let meta = pmcp::types::protocol::RequestMeta::new()
        .with_meta(LOG_LEVEL_META_KEY, serde_json::json!(PROBED_LEVEL));
    let logged = payload(
        &client
            .call_tool_with_meta("dep__log_emit".to_string(), serde_json::json!({}), meta)
            .await
            .expect("v2 dep__log_emit completes"),
    );
    assert_field(
        &logged,
        LOG_RESULT_LEVEL_FIELD,
        PROBED_LEVEL,
        "v2 dep__log_emit",
    );
    assert_field(
        &logged,
        LOG_RESULT_SOURCE_FIELD,
        LOG_LEVEL_SOURCE_REQUEST_META,
        "v2 dep__log_emit",
    );

    // G-5, CLOSED AND PINNED. The 2026-07-28 schema retires the RPC, and since
    // Phase 118.1 plan 05 this SDK retires it too — by method STRING at the v2
    // ingress, so the refusal does not depend on the params parsing. The
    // tripwire now guards the CLOSED state: it fires if the retirement is ever
    // undone.
    //
    // This assertion was FLIPPED by plan 05, following the instruction its own
    // pre-flip failure message gave verbatim. ERA-11 moved to v1 `served` / v2
    // `error:-32601` with `kind: method-removed` in the SAME commit.
    let retired = client
        .set_logging_level(pmcp::types::notifications::LoggingLevel::Debug)
        .await;
    let Err(error) = retired else {
        panic!(
            "FAILURE MODE: v2 `logging/setLevel` was SERVED, not refused.\n\
             CONSEQUENCE: gap G-5 has REGRESSED — the SDK is answering an RPC the 2026-07-28 \
             core schema removed, so it serves a surface the era it claims to speak does not \
             define, and the official conformance suite's removed-methods probe will fail on it.\n\
             WHAT TO DO: restore the v2 ingress retirement in \
             src/server/streamable_http_server.rs (the V2_RETIRED_METHODS table). Do NOT relax \
             this assertion, and do NOT move baseline row ERA-11 back to `era-agreement`; \
             tests/v2_retired_methods.rs is the wire-level proof of the same fact and will be \
             red alongside it."
        );
    };
    assert_eq!(
        error.error_code(),
        Some(pmcp::ErrorCode::METHOD_NOT_FOUND),
        "FAILURE MODE: v2 `logging/setLevel` was refused with {error:?}, not METHOD_NOT_FOUND.\n\
         CONSEQUENCE: the method is unreachable, but for the wrong reason — a retirement the \
         suite scores on the CODE (-32601) would not be credited, and the refusal could be a \
         capability, auth or params rejection wearing a retirement's clothes.\n\
         WHAT TO DO: check that the retirement still emits METHOD_NOT_FOUND through \
         v2_status_for_code rather than some nearer-to-hand error."
    );

    // SAMPLING and ROOTS, v2 mechanism: the server answers `input_required`,
    // the client's MRTR loop answers it from the registered host handlers and
    // RESENDS, and the second leg completes. `call_tool` drives that loop on a
    // v2 connection (`src/client/mod.rs:1127-1133`).
    let sampled = payload(
        &client
            .call_tool("dep__request_sampling".to_string(), serde_json::json!({}))
            .await
            .expect("v2 dep__request_sampling completes through InputRequiredResult"),
    );
    assert_field(
        &sampled,
        DEP_RESULT_STATUS_FIELD,
        DEP_STATUS_COMPLETED,
        "v2 dep__request_sampling",
    );

    let rooted = payload(
        &client
            .call_tool("dep__list_roots".to_string(), serde_json::json!({}))
            .await
            .expect("v2 dep__list_roots completes through InputRequiredResult"),
    );
    assert_field(
        &rooted,
        DEP_RESULT_STATUS_FIELD,
        DEP_STATUS_COMPLETED,
        "v2 dep__list_roots",
    );
}

// ===========================================================================
// The v1 MECHANISM control.
// ===========================================================================

/// Under v1 negotiation, Sampling and Roots ARE reachable through their v1
/// server-to-client requests — proved by a typed client that COMPLETES the call.
///
/// # Why this control does not ride the HTTP endpoint
///
/// Because it cannot, and the reason is a measured SDK gap rather than a
/// property of the v1 mechanism. `Server::peer_handle` is assigned only inside
/// `Server::run()` (`src/server/mod.rs:1173`), and `StreamableHttpServer` never
/// calls `run()` — it dispatches through `handle_request_with_context`
/// (`src/server/streamable_http_server.rs:3001`). So over HTTP `extra.peer()` is
/// `None` and no server-to-client request can be issued at all. That is gap G-3
/// in `118-CONFORMANCE-GAPS.md`, already declared under D-21.
///
/// # This control selects NO era
///
/// It never calls `with_protocol_version`, so `DuplexTransport`'s
/// `supports_negotiated_protocol_version` (trait default `false`) is irrelevant
/// here: there is no selection to be inert. Nothing in this test is compared
/// across eras, and it contributes nothing to the matrix — it exists solely to
/// keep the CONF-03 v1 claim honest by proving the mechanism completes when the
/// peer channel exists.
///
/// # Why it uses a test-local server rather than the era target
///
/// The era target's v1 arm gates on `extra.client_capabilities()`, which is
/// populated ONLY from the v2 per-request `_meta` reserved key
/// (`src/types/protocol/context.rs:384-390`); the v1 `initialize` handshake's
/// capabilities land on `Server::client_capabilities`
/// (`src/server/mod.rs:1684`) and are never threaded onto
/// `RequestHandlerExtra`. That gate is therefore permanently `false` on v1.
/// Correcting it would mean editing `crates/pmcp-team-servers/src/`, which this
/// plan does not touch, so the control brings its own two-tool server instead.
#[tokio::test]
async fn v1_sampling_and_roots_complete_via_server_to_client_requests() {
    use pmcp_team_servers::DuplexTransport;

    let (client_transport, server_transport) = DuplexTransport::pair();
    let server = build_v1_mechanism_server();
    let serving = tokio::spawn(async move {
        let _ = server.run(server_transport).await;
    });

    let mut client = pmcp::ClientBuilder::new(client_transport)
        .on_sampling(FixedCompletion)
        .on_roots(fixed_roots)
        .build();
    client
        .initialize(pmcp::types::ClientCapabilities::default())
        .await
        .expect("the v1 control client handshakes");

    let sampled = payload(
        &client
            .call_tool(V1_CONTROL_SAMPLING_TOOL.to_string(), serde_json::json!({}))
            .await
            .expect("the v1 sampling round trip completes"),
    );
    assert_field(
        &sampled,
        DEP_RESULT_STATUS_FIELD,
        DEP_STATUS_COMPLETED,
        "v1 server-to-client sampling/createMessage",
    );
    assert_eq!(
        sampled.get("model").and_then(serde_json::Value::as_str),
        Some(FIXED_MODEL),
        "FAILURE MODE: the completion did not come from the registered offline handler.\n\
         CONSEQUENCE: a round trip that did not reach the client's handler proves nothing about \
         v1 Sampling reachability.\n\
         WHAT TO DO: check the host registry wiring.\n  {sampled}"
    );

    let rooted = payload(
        &client
            .call_tool(V1_CONTROL_ROOTS_TOOL.to_string(), serde_json::json!({}))
            .await
            .expect("the v1 roots round trip completes"),
    );
    assert_field(
        &rooted,
        DEP_RESULT_STATUS_FIELD,
        DEP_STATUS_COMPLETED,
        "v1 server-to-client roots/list",
    );
    assert_eq!(
        rooted.get("rootUri").and_then(serde_json::Value::as_str),
        Some(FIXED_ROOT_URI),
        "FAILURE MODE: the roots answer did not come from the registered offline provider.\n\
         WHAT TO DO: check the host registry wiring.\n  {rooted}"
    );

    serving.abort();
}

/// The v1 control's sampling tool name.
const V1_CONTROL_SAMPLING_TOOL: &str = "v1ctl__request_sampling";
/// The v1 control's roots tool name.
const V1_CONTROL_ROOTS_TOOL: &str = "v1ctl__list_roots";

/// A v1 server-to-client sampling tool with NO capability gate.
struct V1ControlSampling;

#[async_trait::async_trait]
impl pmcp::ToolHandler for V1ControlSampling {
    async fn handle(
        &self,
        _args: serde_json::Value,
        extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        let peer = extra
            .peer()
            .ok_or_else(|| pmcp::Error::internal("no peer handle: the v1 channel is missing"))?;
        let completion = peer
            .sample(pmcp::types::sampling::CreateMessageParams::new(vec![
                pmcp::types::sampling::SamplingMessage::new(
                    pmcp::types::Role::User,
                    pmcp::types::sampling::SamplingMessageContent::Text {
                        text: "v1 mechanism control".to_string(),
                        meta: None,
                    },
                ),
            ]))
            .await?;
        Ok(serde_json::json!({
            DEP_RESULT_STATUS_FIELD: DEP_STATUS_COMPLETED,
            "model": completion.model,
        }))
    }
}

/// A v1 server-to-client roots tool with NO capability gate.
struct V1ControlRoots;

#[async_trait::async_trait]
impl pmcp::ToolHandler for V1ControlRoots {
    async fn handle(
        &self,
        _args: serde_json::Value,
        extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        let peer = extra
            .peer()
            .ok_or_else(|| pmcp::Error::internal("no peer handle: the v1 channel is missing"))?;
        let roots = peer.list_roots().await?;
        Ok(serde_json::json!({
            DEP_RESULT_STATUS_FIELD: DEP_STATUS_COMPLETED,
            "rootUri": roots.roots.first().map(|root| root.uri.clone()),
        }))
    }
}

/// Build the v1 control server: two tools, no accept-list widening, v1 only.
fn build_v1_mechanism_server() -> pmcp::Server {
    pmcp::Server::builder()
        .name("pmcp-era-v1-mechanism-control")
        .version("0.0.0")
        .capabilities({
            let mut capabilities = pmcp::types::ServerCapabilities::default();
            capabilities.tools = Some(pmcp::types::ToolCapabilities { list_changed: None });
            capabilities
        })
        .tool_arc(
            V1_CONTROL_SAMPLING_TOOL,
            std::sync::Arc::new(V1ControlSampling) as std::sync::Arc<dyn pmcp::ToolHandler>,
        )
        .tool_arc(
            V1_CONTROL_ROOTS_TOOL,
            std::sync::Arc::new(V1ControlRoots) as std::sync::Arc<dyn pmcp::ToolHandler>,
        )
        .build()
        .expect("the v1 mechanism control server builds")
}
