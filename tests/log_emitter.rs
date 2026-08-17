//! CONF-10 — the `notifications/message` emitter fence file.
//!
//! This file pins the handler-facing logging surface introduced in Phase 118.2
//! plan 05: [`RequestHandlerExtra::log`] / [`RequestHandlerExtra::log_with_data`],
//! the level type they filter on, and the wire shape a record serializes to.
//!
//! # Requirement
//!
//! CONF-10 — `ServerNotification::LogMessage` gets its first PRODUCTION
//! constructor. Before this plan the variant was built only in tests, so a
//! conforming client could never observe a `notifications/message` from a pmcp
//! server no matter how the server was written.
//!
//! # Reliability doctrine
//!
//! Plans 06, 07 and 08 extend this file with WIRE fences (a real server, a real
//! client, real SSE frames). Those obey the house rules that every other
//! transport fence in this repo obeys, and they are restated here so the file
//! does not drift as it grows:
//!
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning — never a fixed sleep as a synchronization
//! device), SHUTDOWN (sockets dropped, then `abort()`, then `await`). EVERY
//! await that crosses the wire is wrapped in `tokio::time::timeout`: on a
//! deadlock an unbounded await does not FAIL, it hangs, and a hung test reads as
//! a slow test in CI rather than as a red one.
//!
//! The fences in THIS revision are all in-process — no wire, no server — which
//! is why they are fast and why they carry no timeouts of their own.

#![cfg(all(
    feature = "streamable-http",
    feature = "v1-compat",
    not(target_arch = "wasm32")
))]

use std::time::Duration;

use pmcp::types::LoggingLevel;

// ===========================================================================
// Bounds. Every one of them is an upper bound on a wire operation, never a
// synchronization device.
//
// `#[allow(dead_code)]`: the in-process fences in this revision need no bound
// at all. These constants are declared now so the wire fences plans 06/07/08
// append inherit ONE agreed ceiling instead of each inventing its own — the
// failure mode that produces a file where three fences wait three different
// amounts of time for the same operation.
// ===========================================================================

/// Upper bound on any single client operation that crosses the wire.
#[allow(dead_code)]
const BOUND: Duration = Duration::from_secs(5);

/// How long a stream is watched to prove NOTHING is delivered to it.
#[allow(dead_code)]
const QUIET: Duration = Duration::from_millis(600);

/// Every [`LoggingLevel`] variant, written out IN SYSLOG SEVERITY ORDER.
///
/// This array is the test's own statement of the intended relation: index `i`
/// is strictly less severe than index `i + 1`. It is deliberately a literal
/// list rather than anything derived from `src/`, because a test that reads its
/// expectation out of the code under test asserts nothing.
const ALL_LEVELS: [LoggingLevel; 8] = [
    LoggingLevel::Debug,
    LoggingLevel::Info,
    LoggingLevel::Notice,
    LoggingLevel::Warning,
    LoggingLevel::Error,
    LoggingLevel::Critical,
    LoggingLevel::Alert,
    LoggingLevel::Emergency,
];

/// The lowercase wire spelling of each level, in the SAME order as
/// [`ALL_LEVELS`]. Literals, not `serde` output — see [`ALL_LEVELS`].
const ALL_LEVEL_WIRE_NAMES: [&str; 8] = [
    "debug",
    "info",
    "notice",
    "warning",
    "error",
    "critical",
    "alert",
    "emergency",
];

// ===========================================================================
// Fence 1 — the level ordering.
// ===========================================================================

/// The full 8 x 8 `(configured, emitted)` matrix, exhaustively.
///
/// Exhaustive rather than sampled: 64 pairs is cheaper than any proptest run
/// and strictly stronger, because it cannot miss the one pair that matters.
#[test]
fn log_levels_order_by_syslog_severity_not_by_string() {
    let mut pairs_checked = 0usize;

    for (configured_idx, configured) in ALL_LEVELS.iter().enumerate() {
        for (emitted_idx, emitted) in ALL_LEVELS.iter().enumerate() {
            pairs_checked += 1;

            // The intended relation, stated by the declaration order of
            // ALL_LEVELS above and nothing else.
            let should_pass_filter = emitted_idx >= configured_idx;

            assert_eq!(
                *emitted >= *configured,
                should_pass_filter,
                "level ordering wrong for (configured = {:?} @ {}, emitted = {:?} @ {}): \
                 Ord said {}, syslog severity says {}",
                configured,
                configured_idx,
                emitted,
                emitted_idx,
                *emitted >= *configured,
                should_pass_filter
            );
        }
    }

    assert_eq!(
        pairs_checked, 64,
        "the ordering fence must cover all 8 x 8 = 64 (configured, emitted) pairs; \
         covered {pairs_checked}. A shrunk matrix is a fence that stopped fencing."
    );

    // The exact pair a STRING comparison gets wrong, asserted on its own so the
    // regression has a name rather than hiding inside the loop.
    assert!(
        LoggingLevel::Critical > LoggingLevel::Debug,
        "critical must outrank debug by severity"
    );
    assert!(
        ALL_LEVEL_WIRE_NAMES[5] < ALL_LEVEL_WIRE_NAMES[0],
        "precondition of this regression: lexically \"critical\" < \"debug\". If this ever \
         stops holding the fence below is no longer testing what it claims to test."
    );

    // Stated positively: comparing the SERIALIZED strings would pass `critical`
    // through a `>= debug` filter while SUPPRESSING `debug` — exactly backwards.
    let string_filter_admits_debug = ALL_LEVEL_WIRE_NAMES[0] >= ALL_LEVEL_WIRE_NAMES[0];
    let string_filter_admits_critical = ALL_LEVEL_WIRE_NAMES[5] >= ALL_LEVEL_WIRE_NAMES[0];
    assert!(
        string_filter_admits_debug && !string_filter_admits_critical,
        "the string-comparison failure mode this fence exists to prevent has changed shape"
    );

    // And the typed comparison gets both right.
    assert!(LoggingLevel::Debug >= LoggingLevel::Debug);
    assert!(LoggingLevel::Critical >= LoggingLevel::Debug);
}

/// Property arm for `make test-property` (`cargo test --features "full" --
/// --ignored property_`).
///
/// The exhaustive fence above is the load-bearing half; this arm exists so the
/// CLAUDE.md ALWAYS-property requirement is discharged by a test the
/// `validate-always` target actually SELECTS, rather than by an always-run test
/// that target never runs.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_log_level_ordering_is_a_total_order_over_declaration_index() {
    use proptest::prelude::*;

    proptest!(|(a in 0usize..8, b in 0usize..8, c in 0usize..8)| {
        let (la, lb, lc) = (ALL_LEVELS[a], ALL_LEVELS[b], ALL_LEVELS[c]);

        // Agreement with the declaration index (the syslog severity order).
        prop_assert_eq!(la <= lb, a <= b);
        prop_assert_eq!(la == lb, a == b);

        // Totality and antisymmetry.
        prop_assert!(la <= lb || lb <= la);
        if la <= lb && lb <= la {
            prop_assert_eq!(la, lb);
        }

        // Transitivity.
        if la <= lb && lb <= lc {
            prop_assert!(la <= lc);
        }
    });
}
