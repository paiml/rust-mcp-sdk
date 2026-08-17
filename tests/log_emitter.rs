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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use pmcp::types::{LoggingLevel, Notification};
use pmcp::RequestHandlerExtra;

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

// ===========================================================================
// Capture harness. A sink is `Arc<dyn Fn(Notification) + Send + Sync>` — it
// returns `()` and therefore cannot report failure, which is the whole reason
// the emitter's `Ok(())` must not be read as delivery acknowledgement.
// ===========================================================================

/// A sink that records every notification handed to it.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Notification>>>);

impl Capture {
    fn new() -> Self {
        Self::default()
    }

    /// The sink to hand to `with_log_sink`.
    fn sink(&self) -> Arc<dyn Fn(Notification) + Send + Sync> {
        let slot = Arc::clone(&self.0);
        Arc::new(move |notification| {
            slot.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(notification);
        })
    }

    fn len(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Every captured notification, serialized. The fences assert on the JSON,
    /// not on the Rust enum: a `serde_json::to_value` round trip through pmcp's
    /// own types would only prove pmcp agrees with itself.
    fn json(&self) -> Vec<serde_json::Value> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|n| serde_json::to_value(n).expect("a notification must serialize"))
            .collect()
    }
}

// ===========================================================================
// Fence 2 — the no-sink contract (D-08).
// ===========================================================================

/// With no sink attached, both emitter methods succeed and emit nothing.
///
/// This is what keeps a handler callable outside a server:
/// `RequestHandlerExtra::default()` is documented for "testing and simple tool
/// invocations", and a handler that logs must not become un-unit-testable.
///
/// The accepted cost is stated at the emitter: a MISPLUMBED transport looks
/// identical to a quiet handler. The conformance fence — a test asserting logs
/// actually arrive over the wire — is what catches that, which is why this is a
/// production-diagnostics hole and not a false green in the gate.
#[test]
fn log_with_no_sink_is_ok_and_emits_nothing() {
    let extra = RequestHandlerExtra::default();

    assert!(
        extra.log(LoggingLevel::Error, "no sink here").is_ok(),
        "log with no sink must be Ok(())"
    );
    assert!(
        extra
            .log_with_data(LoggingLevel::Error, "no sink here either", json!({"k": 1}))
            .is_ok(),
        "log_with_data with no sink must be Ok(())"
    );

    // "Emits nothing" asserted POSITIVELY rather than inferred from the absence
    // of a panic: a live capture that stays empty across an emitter that was
    // never called proves the harness itself reports emptiness correctly.
    let capture = Capture::new();
    let _observed = RequestHandlerExtra::default().with_log_sink(capture.sink());
    assert_eq!(
        capture.len(),
        0,
        "a sink that was never emitted to must hold nothing"
    );
}

// ===========================================================================
// Fence 3 — the wire shape.
// ===========================================================================

/// A record serializes as the spec's `notifications/message` envelope.
///
/// Asserted against LITERALS. Nothing here is imported from `src/`, because a
/// test that reads its expectation out of the code under test asserts nothing.
///
/// # Known divergence from the vendored schema
///
/// `schema/vendored/core-2026-07-28/schema.ts` declares
/// `LoggingMessageNotificationParams` as `{ level, logger?, data }` — `data` is
/// REQUIRED and there is no `message` member. pmcp's `LogMessageParams` instead
/// carries a required `message` and an OPTIONAL `data`. This fence pins what
/// pmcp emits TODAY so the divergence is a recorded, visible fact rather than a
/// surprise discovered by a conformance run. Changing `LogMessageParams` is a
/// breaking change to a public type and is out of this plan's scope; see the
/// phase's `deferred-items.md`.
#[test]
fn a_log_record_serializes_as_the_spec_notifications_message_shape() {
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());

    extra
        .log(LoggingLevel::Warning, "hello")
        .expect("log must be Ok(())");
    extra
        .log_with_data(LoggingLevel::Error, "boom", json!({"k": 1}))
        .expect("log_with_data must be Ok(())");

    let records = capture.json();
    assert_eq!(records.len(), 2, "both calls must reach the sink");

    let warning = &records[0];
    assert_eq!(
        warning.get("method").and_then(serde_json::Value::as_str),
        Some("notifications/message"),
        "method must be the spec literal; got {warning}"
    );
    let params = warning.get("params").expect("params must be present");
    assert_eq!(
        params.get("level").and_then(serde_json::Value::as_str),
        Some("warning"),
        "level must serialize lowercase"
    );
    assert_eq!(
        params.get("message").and_then(serde_json::Value::as_str),
        Some("hello")
    );
    assert!(
        params.get("logger").is_none(),
        "logger must be ABSENT — the emitter does not synthesise one, because a \
         synthesised logger name would be a guess; got {params}"
    );
    assert!(
        params.get("data").is_none(),
        "data must be absent for a plain log(..); got {params}"
    );

    let error = &records[1];
    assert_eq!(
        error.get("method").and_then(serde_json::Value::as_str),
        Some("notifications/message")
    );
    let params = error.get("params").expect("params must be present");
    assert_eq!(
        params.get("level").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        params.get("message").and_then(serde_json::Value::as_str),
        Some("boom")
    );
    assert!(
        params.get("logger").is_none(),
        "logger must still be ABSENT"
    );
    assert_eq!(
        params.get("data"),
        Some(&json!({"k": 1})),
        "log_with_data must carry the structured payload verbatim"
    );
}

// ===========================================================================
// Fence 4 — the level filter, both directions, configured and defaulted.
// ===========================================================================

/// A record below the effective level never reaches the sink, and one at or
/// above it always does — for an explicitly configured level AND for the D-12
/// default.
#[test]
fn a_record_below_the_configured_level_is_not_sent() {
    // --- explicitly configured at `warning` ---
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default()
        .with_log_sink(capture.sink())
        .with_log_level(LoggingLevel::Warning);

    for suppressed in [
        LoggingLevel::Debug,
        LoggingLevel::Info,
        LoggingLevel::Notice,
    ] {
        assert!(
            extra.log(suppressed, "below").is_ok(),
            "a suppressed record still returns Ok(()) — suppression is not an error"
        );
    }
    assert_eq!(
        capture.len(),
        0,
        "debug/info/notice must be suppressed under a `warning` filter"
    );

    let mut expected = 0usize;
    for sent in [
        LoggingLevel::Warning,
        LoggingLevel::Error,
        LoggingLevel::Emergency,
    ] {
        extra.log(sent, "at or above").expect("must be Ok(())");
        expected += 1;
        assert_eq!(
            capture.len(),
            expected,
            "{sent:?} is at or above `warning` and must be delivered"
        );
    }

    // --- unconfigured: the D-12 default is `info` ---
    let defaulted = Capture::new();
    let extra = RequestHandlerExtra::default().with_log_sink(defaulted.sink());

    extra
        .log(LoggingLevel::Debug, "below the default")
        .expect("must be Ok(())");
    assert_eq!(
        defaulted.len(),
        0,
        "with NO level configured the default is `info`, so `debug` is suppressed — \
         a chatty handler cannot flood a client that never opted in"
    );

    extra
        .log(LoggingLevel::Info, "at the default")
        .expect("must be Ok(())");
    assert_eq!(
        defaulted.len(),
        1,
        "`info` is AT the default and must be delivered unconfigured — which is why \
         the conformance scenario passes without any level being set"
    );
}

// ===========================================================================
// Fence 5 — the downstream loss policy.
// ===========================================================================

/// How many records the modelled bounded sink holds.
///
/// Small on purpose. The real v2 vehicle is `new_v2_progress_queue()` with
/// `V2_PROGRESS_QUEUE_CAPACITY`; that constant is `pub(crate)` and out of reach
/// from an integration test, and the number is not what is under test here —
/// the POLICY is.
const SATURATION_CAPACITY: usize = 4;

/// Records emitted past capacity, so the overflow is unambiguous.
const SATURATION_OVERFLOW: usize = 8;

/// A saturated bounded sink drops the excess; the emitter still returns `Ok`.
///
/// This fence exists because the v2 limitation must be a PINNED behaviour rather
/// than prose nobody checks. The sink modelled here mirrors
/// `new_v2_progress_queue()`'s policy exactly: an `mpsc::channel(N)` whose
/// closure `try_send`s and swallows the error, because the closure is
/// synchronous and must never block the handler's task.
///
/// The consequence, asserted rather than asserted-about: a handler emitting more
/// than the queue's capacity in one call LOSES THE EXCESS, and every one of
/// those losing calls returned `Ok(())`. That is precisely why the emitter's
/// rustdoc must not claim delivery acknowledgement — `Ok(())` cannot possibly
/// mean "delivered" when the sink's own type is `Fn(Notification) -> ()`.
///
/// D-09's "no rate limit" is about the EMITTER not adding a limiter of its own.
/// It never claimed no record can be dropped downstream, and this fence is what
/// keeps the two statements from being confused.
#[test]
fn a_saturated_bounded_sink_drops_the_excess_and_the_emitter_still_returns_ok() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Notification>(SATURATION_CAPACITY);
    let sink: Arc<dyn Fn(Notification) + Send + Sync> = Arc::new(move |notification| {
        // DROP-NEWEST on a full queue, matching `new_v2_progress_queue`.
        let _ = tx.try_send(notification);
    });

    let extra = RequestHandlerExtra::default().with_log_sink(sink);

    let total = SATURATION_CAPACITY + SATURATION_OVERFLOW;
    for i in 0..total {
        assert!(
            extra
                .log(LoggingLevel::Error, format!("record-{i}"))
                .is_ok(),
            "(a) the emitter never blocks and never reports downstream loss — \
             call {i} must still be Ok(())"
        );
    }

    let mut delivered = Vec::new();
    while let Ok(notification) = rx.try_recv() {
        let value = serde_json::to_value(&notification).expect("must serialize");
        delivered.push(
            value["params"]["message"]
                .as_str()
                .expect("message must be a string")
                .to_string(),
        );
    }

    // (b) the receiver holds exactly N.
    assert_eq!(
        delivered.len(),
        SATURATION_CAPACITY,
        "a channel of capacity {SATURATION_CAPACITY} fed {total} records must hold \
         exactly {SATURATION_CAPACITY}"
    );

    // (c) they are the FIRST N — drop-NEWEST, not drop-oldest. A fence that only
    // checked the count could not tell the two policies apart.
    let expected: Vec<String> = (0..SATURATION_CAPACITY)
        .map(|i| format!("record-{i}"))
        .collect();
    assert_eq!(
        delivered, expected,
        "the surviving records must be the FIRST {SATURATION_CAPACITY} — drop-newest, \
         matching the vehicle rather than silently differing from it"
    );
}

// ===========================================================================
// Fence 6 — BOTH dispatch roots attach the log sink (Phase 118.2 plan 06, D-07).
//
// STRUCTURAL, not behavioural, and the reason is a visibility fact rather than a
// convenience: `server::core::attach_request_log_sink`,
// `ProtocolContext::with_resolved_log_level` and `TransportBackchannel` are all
// `pub(crate)`, and `Server::attach_peer` / `Server::notification_tx` are
// private. An integration test cannot construct the request-scoped half of the
// precedence rule at all, so the BEHAVIOURAL fences live crate-internally beside
// the units they measure:
//
//   * `src/server/core.rs` :: `core_log_sink_tests`
//   * `src/server/mod.rs`  :: `log_sink_precedence_tests`
//
// which is the same placement, for the same stated reason, that Phase 118.1
// chose for the `attach_peer` precedence suite. This fence is what keeps those
// two modules from being deleted or renamed silently, and what pins the
// both-roots claim in the file a reader of CONF-10 actually opens. Plan 07's
// wire fences cover the behavioural half end-to-end.
// ===========================================================================

const CORE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/server/core.rs");
const SERVER_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/server/mod.rs");

/// Every line of `path` that is not a comment, joined back together.
///
/// A claim satisfied by a doc comment is not a claim: the whole point is that
/// both roots CALL the unit, not that both roots mention it.
fn code_lines(path: &str) -> String {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path} is readable: {e}"));
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn both_dispatch_roots_attach_the_log_sink() {
    let core = code_lines(CORE_ROOT);
    let server = code_lines(SERVER_ROOT);

    assert!(
        core.contains("pub(crate) fn attach_request_log_sink("),
        "the shared unit must be DEFINED in src/server/core.rs, beside its `attach_request_peer` \
         twin"
    );
    assert!(
        core.contains("attach_request_log_sink(extra, None)"),
        "the `ServerCore` root must call the shared unit, with the literal `None` fallback it has \
         no notification channel to fill"
    );
    assert!(
        server.contains("attach_request_log_sink(extra, self.notification_tx_sink())"),
        "the `Server` root must call the SAME unit, passing its notification_tx-derived fallback \
         — changing one root and not the other is this phase's most likely silent defect"
    );

    // The behavioural halves, which cannot live in this file. Named here so
    // deleting one is a red test rather than a silent loss of coverage.
    for fence in [
        "fn a_request_scoped_sink_wins_over_the_root_fallback(",
        "fn the_root_fallback_is_used_when_no_request_scoped_sink_exists(",
        "fn attach_request_log_sink_is_a_no_op_when_neither_source_exists(",
        "fn a_resolved_log_level_on_the_context_reaches_the_extra(",
        "fn the_server_core_root_attaches_the_request_scoped_log_sink(",
        "fn the_server_core_root_has_no_fallback_sink_of_its_own(",
    ] {
        assert!(
            core.contains(fence),
            "the crate-internal behavioural fence `{fence}` must still exist in src/server/core.rs"
        );
    }
    for fence in [
        "fn the_server_root_attaches_its_notification_tx_derived_fallback_log_sink(",
        "fn the_request_scoped_sink_wins_over_the_notification_tx_fallback(",
        "fn the_progress_token_gate_still_applies_to_progress_only(",
        "fn the_progress_and_log_paths_share_one_notification_tx_sink(",
    ] {
        assert!(
            server.contains(fence),
            "the crate-internal behavioural fence `{fence}` must still exist in src/server/mod.rs"
        );
    }
}

/// The progress-token gate is still on the PROGRESS reporter, in source.
///
/// The behavioural half lives in `log_sink_precedence_tests`; this pins the one
/// line, because "unify the two paths" is a tidy-looking refactor that would
/// make progress notifications unconditional (T-118.2-06-04).
#[test]
fn the_progress_token_gate_is_still_the_first_line_of_progress_reporter_for() {
    let server = code_lines(SERVER_ROOT);
    assert!(
        server.contains("let token = meta.and_then(|meta| meta.progress_token.as_ref())?;"),
        "`progress_reporter_for` must still gate on the progress token — D-07 removed that gate \
         from the LOG sink only"
    );
}

// ===========================================================================
// Fence 7 — the log sink is UNGATED by the progress token (D-07), measured over
// the public surface.
//
// The crate-internal twin proves the DISPATCH root does this. This proves the
// EMITTER does: a `RequestHandlerExtra` with a live log sink and no progress
// reporter at all logs happily, which is the shape every non-progress request
// now has.
// ===========================================================================

#[test]
fn the_log_sink_is_live_without_any_progress_reporter() {
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());

    assert!(
        extra.progress_reporter.is_none(),
        "this fence is only meaningful for a request with NO progress reporter"
    );
    extra
        .log(LoggingLevel::Info, "no progress token was ever sent")
        .expect("the emitter always returns Ok");

    assert_eq!(
        capture.len(),
        1,
        "a client that never asked for progress must still receive `notifications/message` — the \
         progress token gates the reporter, not the sink (D-07)"
    );
    let record = &capture.json()[0];
    assert_eq!(
        record["method"], "notifications/message",
        "and it must be the spec method, not a progress notification"
    );
}
