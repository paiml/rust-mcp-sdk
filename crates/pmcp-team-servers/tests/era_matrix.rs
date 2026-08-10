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
    format!(
        "  [{}] {} — observed v1 `{}` / v2 `{}`; {recorded}",
        row.class.label(),
        row.observation_id,
        row.v1.as_ref().map_or_else(
            || "not observed".to_string(),
            pmcp_team_servers::conformance::ObservedValue::token
        ),
        row.v2.as_ref().map_or_else(
            || "not observed".to_string(),
            pmcp_team_servers::conformance::ObservedValue::token
        ),
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

/// Fail on any MISSING row whose baseline entry is NOT provisional.
///
/// The `provisional` exemption is applied HERE, by the consumer, and never
/// inside `compare_eras` — a comparator that skipped provisional rows would make
/// a stale one permanently invisible. Plan 118-07 Task 2 reconciles every row
/// against measurement, after which no provisional row remains and this
/// exemption is dead by construction.
fn assert_no_missing(report: &EraComparisonReport, baseline: &EraBaseline, rendered: &str) {
    let missing: Vec<String> = report
        .differences
        .iter()
        .filter(|row| row.class == DifferenceClass::Missing && !row.provisional)
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
         tests/era_baseline.rs fails on an orphaned probe.\n{rendered}",
        missing.len(),
        missing.join("\n")
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
