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

/// The RAW streamable-HTTP wire seam the era probes issue their requests
/// through (D-16). Behind the non-default `http` feature, so the DEFAULT and
/// wasm32 builds of this crate stay reqwest-free (T-118-71).
#[cfg(feature = "http")]
pub mod era_probe;

/// The dual-accept-list ERA TARGET (D-16): a purpose-built server that
/// advertises BOTH protocol versions and exposes the three CONF-03 deprecated
/// capabilities as probeable tools. Not a reference server; nothing ships it.
#[cfg(all(feature = "conformance", feature = "http"))]
pub mod era_target;

pub use era_observations::{EraObservations, ObservationId, ObservedValue, PROBE_REGISTRY};

#[cfg(feature = "http")]
pub use era_probe::{
    build_probe_body, extract_jsonrpc_envelope, EraProbeClient, RawProbeOutcome, V2HeaderMode,
};

#[cfg(all(feature = "conformance", feature = "http"))]
pub use era_target::{build_era_target_server, spawn_era_target, EraTargetHandle};

pub use era_diff::{
    compare_eras, load_default_baseline, parse_baseline, ClassifiedDifference, DifferenceClass,
    EraBaseline, EraComparisonReport, EraDelta,
};
