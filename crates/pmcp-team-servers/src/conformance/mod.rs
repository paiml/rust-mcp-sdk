//! Conformance harness: replay the `contracts/team-servers/fixtures/**` cases
//! against live reference servers over an in-process [`crate::DuplexTransport`].
//!
//! Two surfaces live here, and they measure DIFFERENT things:
//!
//! * [`runner`] — the fixture-replay harness (109-07, TEAM-06). Under Phase 118
//!   **D-16** it is a **v1-only regression guard**: it proves advertised ==
//!   enforced for the 33 checked-in fixtures, and it is no longer the
//!   era-comparison surface. Its [`CaseResult`](runner::CaseResult) carries
//!   `{case_id, passed, detail}`, which structurally cannot carry a wire fact.
//! * [`era_observations`] + [`era_diff`] — the Phase-118 era substrate, ported
//!   from `crates/mcp-tester` (Phase 117) under D-16. Observations come from
//!   EXPLICIT PROBE CODE keyed by a stable id, never inferred from a pass/fail
//!   bool, and are joined against a checked-in expected-difference baseline.

pub mod runner;

/// The typed observation substrate for the Phase-118 era comparison (D-16):
/// stable ids, typed observed values, and the probe registry.
pub mod era_observations;

/// The expected-difference baseline model, its total parser, and the
/// bidirectional `observation_id`-keyed join (D-16).
pub mod era_diff;

pub use era_observations::{EraObservations, ObservationId, ObservedValue, PROBE_REGISTRY};

pub use era_diff::{
    compare_eras, load_default_baseline, parse_baseline, ClassifiedDifference, DifferenceClass,
    EraBaseline, EraComparisonReport, EraDelta,
};
