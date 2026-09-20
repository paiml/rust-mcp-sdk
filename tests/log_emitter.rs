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
/// # The `data` member is REQUIRED, and `message` rides alongside it
///
/// `schema/vendored/core-2026-07-28/schema.ts` declares
/// `LoggingMessageNotificationParams` as `{ level, logger?, data }` — `data` is
/// REQUIRED and there is no `message` member at all. Since plan 118.2-13 the
/// emitter DEFAULTS `data` to the message string when the caller supplied none,
/// so every frame pmcp puts on the wire satisfies that requirement. `message`
/// stays alongside as a pmcp extension: the schema does not close
/// `additionalProperties`, and the official reference client strips unknown
/// members rather than rejecting them.
///
/// This fence previously asserted the OPPOSITE — `data must be absent for a
/// plain log(..)` — under plan 118.2-08's verdict that no suite scenario
/// validates an emitted notification's params. Plan 118.2-11 MEASURED that
/// premise false: `WireSchemaValid` is not a scenario, it is a check that runs
/// inside scenarios over every frame the implementation sends, and it failed
/// with `LoggingMessageNotification/params: must have required property 'data'`.
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
    assert_eq!(
        params.get("data").and_then(serde_json::Value::as_str),
        Some("hello"),
        "data must be PRESENT for a plain log(..), carrying the message string — the vendored \
         schema marks it required and the reference client's `z.unknown()` is non-optional under \
         zod v4, so an absent `data` makes the client drop the frame; got {params}"
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
    // The fallback is a THUNK at both roots: `attach_request_log_sink` prefers
    // the request-scoped `TransportBackchannel` sink and never reads the fallback
    // on an HTTP-served request, so `Server` must not allocate its
    // `Arc<dyn Fn(..)>` before that branch is taken. The claim the fence pins is
    // unchanged — `ServerCore` supplies NOTHING, `Server` supplies its
    // notification_tx-derived sink — only the spelling of how it is handed over.
    assert!(
        core.contains("attach_request_log_sink(extra, || None)"),
        "the `ServerCore` root must call the shared unit, with the literal `None` fallback it has \
         no notification channel to fill"
    );
    assert!(
        server.contains("attach_request_log_sink(extra, || self.notification_tx_sink())"),
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

/// Exactly ONE site turns `Server::notification_tx` into a sink.
///
/// The progress path, the log path and the cancellation manager all read
/// `Server::notification_tx_sink`. Before Phase 118.2 plan 06 there were two
/// hand-rolled `try_send` closures and the log path would have made a third —
/// three chances to disagree about the send discipline, and the discipline is
/// load-bearing: `try_send` on a bounded channel is what keeps a saturated
/// client from blocking a handler, at the documented cost of silent loss.
///
/// Counted over non-comment lines of the ONE file that owns the field. The other
/// `try_send(notification)` in the tree
/// (`streamable_http_server::new_v2_progress_queue`) is a per-request v2 queue
/// created locally, not this server-wide channel.
#[test]
fn exactly_one_site_converts_the_server_notification_tx_into_a_sink() {
    let server = code_lines(SERVER_ROOT);
    let conversions = server.matches("tx.try_send(notification)").count();
    assert_eq!(
        conversions, 1,
        "src/server/mod.rs must contain exactly one `notification_tx`-to-sink conversion \
         (`Server::notification_tx_sink`); found {conversions}"
    );
    assert!(
        server.contains("fn notification_tx_sink(&self)"),
        "and it must be the named helper, so both consumers can reach it"
    );
}

// ===========================================================================
// WIRE FENCES (Phase 118.2 plan 07) — a real `StreamableHttpServer` on an
// ephemeral port, a raw stream-holding client, and real `notifications/message`
// frames on the wire.
//
// The fences above measure the EMITTER and the dispatch roots. These measure the
// thing a conforming client actually observes: that a `extra.log(..)` record
// leaves the process, on BOTH eras, filtered by a level the CLIENT chose.
//
// The client is raw TCP rather than `reqwest` for the same reason
// `tests/http_peer_roundtrip.rs` gives: the v1 vehicle is a GET
// `text/event-stream` body that never reaches EOF, so it must be read
// INCREMENTALLY, and each POST needs its own connection so HTTP keep-alive
// head-of-line ordering can never be mistaken for server-side ordering.
//
// pmcp's own client is deliberately NOT the observer here. Plan 10 owns the
// joint client/server fence; using it now would make a failure attributable to
// either side.
// ===========================================================================

use std::net::{Ipv4Addr, SocketAddr};

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;

use pmcp::server::http_middleware::ServerHttpMiddlewareChain;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use pmcp::{Server, ToolHandler};

/// The spec method every log record must arrive under. A LITERAL, never imported
/// from `src/` — a fence that reads its expectation out of the code under test
/// asserts nothing.
const LOG_METHOD: &str = "notifications/message";

/// The `_meta` key the 2026-07-28 transport carries a level in, spelled as a
/// literal here for the same reason.
const LOG_LEVEL_META_KEY: &str = "io.modelcontextprotocol/logLevel";

/// The v1 RPC the 2026-07-28 transport retired, likewise a literal.
const SET_LEVEL_METHOD: &str = "logging/setLevel";

/// The message on the `debug` record — delivered only when a client ASKED for
/// `debug`, suppressed by the `info` default (D-12).
const DEBUG_MESSAGE: &str = "log-emitter-debug-record";

/// The message on the `info` record — delivered under the default.
const INFO_MESSAGE: &str = "log-emitter-info-record";

/// A level value that is not a level, chosen IN THIS TEST.
///
/// Deliberately a string that appears nowhere in `src/`: an absence assertion
/// over a value the source also contains could be satisfied by a source string
/// the test never sent, which would make the no-echo claim vacuous.
const MALFORMED_LEVEL: &str = "NOT-A-LEVEL-2b7f1c";

/// The tool every wire fence calls. It emits one record at `debug` and one at
/// `info`, so a single call measures the filter in BOTH directions.
struct LoggingTool;

#[async_trait]
impl ToolHandler for LoggingTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        extra.log(LoggingLevel::Debug, DEBUG_MESSAGE)?;
        extra.log(LoggingLevel::Info, INFO_MESSAGE)?;
        Ok(json!("logged"))
    }
}

/// A stock v1 server: NOT opted into v2, which is what a default deployment is.
fn build_v1_server() -> Server {
    Server::builder()
        .name("log-emitter-v1")
        .version("1.0.0")
        .tool("logger", LoggingTool)
        .build()
        .expect("server builds")
}

/// The same fixture opted into BOTH eras, so one server can serve a v1 session
/// and a v2 stateless request without the two fences drifting apart.
fn build_dual_era_server() -> Server {
    Server::builder()
        .name("log-emitter-dual")
        .version("1.0.0")
        .tool("logger", LoggingTool)
        .with_supported_protocol_versions([
            pmcp::types::protocol::ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            pmcp::types::protocol::ProtocolVersion(
                pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            ),
        ])
        .build()
        .expect("server builds")
}

/// `StreamableHttpServerConfig::default()` keeps a live `session_id_generator`,
/// which is what a real deployment ships: the ERA decides whether sessions are
/// live for a given request, not the config.
fn stateful_config() -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
}

/// The same config with an EMPTY HTTP middleware chain attached.
///
/// `handle_post_request` routes on `config.http_middleware.is_none()`, so an
/// empty chain is enough to send every POST down the MIDDLEWARE ingress path
/// instead of the fast path — which is exactly the deployment shape that a
/// resolution written at only one of the two call sites would silently ignore
/// (T-118.2-07-06). No middleware behaviour is needed to prove that; only the
/// route.
fn middleware_config() -> StreamableHttpServerConfig {
    StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(ServerHttpMiddlewareChain::new())),
        ..StreamableHttpServerConfig::default()
    }
}

/// Spawn on an EPHEMERAL port and read the bound address back from `start()`.
async fn spawn_server(
    server: Server,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let server = Arc::new(TokioMutex::new(server));
    StreamableHttpServer::with_config(addr, server, config)
        .start()
        .await
        .expect("server starts on an ephemeral port")
}

async fn teardown(handle: JoinHandle<()>, sockets: impl Send) {
    drop(sockets);
    handle.abort();
    let _ = handle.await;
}

// ---------------------------------------------------------------------------
// A minimal raw HTTP/1.1 client, lifted from `tests/http_peer_roundtrip.rs`.
// ---------------------------------------------------------------------------

/// One open HTTP response, readable frame by frame.
struct Conn {
    reader: BufReader<TcpStream>,
    status: u16,
    headers: Vec<(String, String)>,
    buffer: String,
    chunked: bool,
    remaining: usize,
    finished: bool,
}

impl Conn {
    /// Send a request and read only as far as the response headers.
    async fn open(addr: SocketAddr, verb: &str, extra: &[(String, String)], body: &str) -> Self {
        let stream = TcpStream::connect(addr).await.expect("connects");
        let accept = if verb == "GET" {
            "text/event-stream"
        } else {
            "application/json, text/event-stream"
        };
        let mut request = format!(
            "{verb} / HTTP/1.1\r\nHost: {addr}\r\nAccept: {accept}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);

        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .expect("request written");

        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .await
            .expect("status line");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("header line");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }

        let chunked = headers
            .iter()
            .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"));
        let remaining = headers
            .iter()
            .find(|(n, _)| n == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        Self {
            reader,
            status,
            headers,
            buffer: String::new(),
            chunked,
            remaining,
            finished: false,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Pull more body bytes into [`Self::buffer`] in whichever framing applies.
    async fn pull(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if !self.chunked {
            let mut payload = vec![0u8; self.remaining];
            let ok = self.remaining > 0 && self.reader.read_exact(&mut payload).await.is_ok();
            self.finished = true;
            if !ok {
                return false;
            }
            self.buffer.push_str(&String::from_utf8_lossy(&payload));
            return true;
        }
        let mut size_line = String::new();
        if self.reader.read_line(&mut size_line).await.unwrap_or(0) == 0 {
            self.finished = true;
            return false;
        }
        let size_token = size_line.trim().split(';').next().unwrap_or("").to_string();
        let Ok(size) = usize::from_str_radix(&size_token, 16) else {
            self.finished = true;
            return false;
        };
        if size == 0 {
            self.finished = true;
            return false;
        }
        let mut payload = vec![0u8; size];
        if self.reader.read_exact(&mut payload).await.is_err() {
            self.finished = true;
            return false;
        }
        let mut crlf = [0u8; 2];
        let _ = self.reader.read_exact(&mut crlf).await;
        self.buffer.push_str(&String::from_utf8_lossy(&payload));
        true
    }

    /// Pop one complete SSE block (`…\n\n`) from the buffer, if present.
    fn take_block(&mut self) -> Option<String> {
        let end = self.buffer.find("\n\n")?;
        let block = self.buffer[..end].to_string();
        self.buffer.drain(..end + 2);
        Some(block)
    }

    /// The next `data:` payload, or `None` at end of stream. UNBOUNDED — always
    /// reach it through [`Self::frame`].
    async fn next_data(&mut self) -> Option<String> {
        loop {
            if let Some(block) = self.take_block() {
                let mut data = String::new();
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        data.push_str(rest.trim_start());
                    }
                }
                if !data.is_empty() {
                    return Some(data);
                }
                continue;
            }
            if !self.pull().await {
                if !self.buffer.trim().is_empty() {
                    let rest = std::mem::take(&mut self.buffer);
                    return Some(rest.trim().to_string());
                }
                return None;
            }
        }
    }

    /// The next protocol frame on this stream, parsed as JSON and BOUNDED.
    ///
    /// On a wedged transport an unbounded await does not FAIL, it hangs, and a
    /// hung test reads as a slow test in CI rather than as a red one.
    async fn frame(&mut self, what: &str) -> Value {
        let data = tokio::time::timeout(BOUND, self.next_data())
            .await
            .unwrap_or_else(|_| panic!("{what} must not hang"))
            .unwrap_or_else(|| panic!("{what}: the stream ended with no frame"));
        serde_json::from_str(&data).expect("every frame on this stream is JSON")
    }

    /// Read the whole body of a completed response.
    async fn body(&mut self) -> String {
        while self.pull().await {}
        std::mem::take(&mut self.buffer)
    }
}

// ---------------------------------------------------------------------------
// Request construction.
// ---------------------------------------------------------------------------

fn envelope(method: &str, id: i64, params: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn init_body() -> String {
    envelope(
        "initialize",
        1,
        &json!({
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "log-emitter-fence", "version": "1.0.0" }
        }),
    )
}

fn call_body(id: i64) -> String {
    envelope(
        "tools/call",
        id,
        &json!({ "name": "logger", "arguments": {} }),
    )
}

/// A v1 `logging/setLevel` carrying `level` VERBATIM, so a fence can send a value
/// that is not a level at all.
fn set_level_body(id: i64, level: &Value) -> String {
    envelope(SET_LEVEL_METHOD, id, &json!({ "level": level }))
}

/// A v2 `tools/call` with the three reserved `_meta` keys VERS-05 requires, plus
/// an optional log level.
///
/// The reserved key SPELLINGS come from `pmcp::testing` so a rename in the crate
/// breaks this fence rather than silently turning it into a probe of the
/// rejection path. The LOG LEVEL key is a local literal on purpose — it is the
/// wire contract under test.
fn v2_call_body(id: i64, log_level: Option<&Value>) -> String {
    let mut meta = json!({
        pmcp::testing::META_PROTOCOL_VERSION:
            pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28,
        pmcp::testing::META_CLIENT_INFO: { "name": "log-emitter-fence", "version": "1.0.0" },
        pmcp::testing::META_CLIENT_CAPABILITIES: {},
    });
    if let (Some(level), Some(map)) = (log_level, meta.as_object_mut()) {
        map.insert(LOG_LEVEL_META_KEY.to_string(), level.clone());
    }
    envelope(
        "tools/call",
        id,
        &json!({ "name": "logger", "arguments": {}, "_meta": meta }),
    )
}

fn v2_call_headers() -> Vec<(String, String)> {
    vec![
        ("MCP-Method".to_string(), "tools/call".to_string()),
        ("Mcp-Name".to_string(), "logger".to_string()),
        (
            "MCP-Protocol-Version".to_string(),
            pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
        ),
    ]
}

fn session_header(session: &str) -> Vec<(String, String)> {
    vec![("Mcp-Session-Id".to_string(), session.to_string())]
}

/// POST `body` and return `(status, body)`, BOUNDED.
async fn post(
    addr: SocketAddr,
    extra: &[(String, String)],
    body: &str,
    what: &str,
) -> (u16, String) {
    let mut conn = tokio::time::timeout(BOUND, Conn::open(addr, "POST", extra, body))
        .await
        .unwrap_or_else(|_| panic!("{what} must not hang"));
    let status = conn.status;
    let text = tokio::time::timeout(BOUND, conn.body())
        .await
        .unwrap_or_else(|_| panic!("{what} body must not hang"));
    (status, text)
}

/// Handshake: POST `initialize` and return the minted session id.
async fn open_session(addr: SocketAddr) -> String {
    let mut conn = tokio::time::timeout(BOUND, Conn::open(addr, "POST", &[], &init_body()))
        .await
        .expect("initialize must not hang");
    assert_eq!(conn.status, 200, "initialize is answered inline");
    let session = conn
        .header("mcp-session-id")
        .expect("a stateful server mints a session id")
        .to_string();
    let _ = tokio::time::timeout(BOUND, conn.body())
        .await
        .expect("initialize body must not hang");
    session
}

/// Open the session's live SSE stream — the v1 server-to-client vehicle.
async fn open_stream(addr: SocketAddr, session: &str) -> Conn {
    let conn = tokio::time::timeout(BOUND, Conn::open(addr, "GET", &session_header(session), ""))
        .await
        .expect("opening the SSE stream must not hang");
    assert_eq!(conn.status, 200, "the SSE stream opens");
    conn
}

/// Fire a `tools/call` WITHOUT awaiting its HTTP response.
///
/// With a live SSE stream the tool's reply is delivered onto that stream and the
/// POST answers `202 Accepted` only once the handler has finished, so a fence
/// that awaited it here could not read the records the handler emitted.
fn spawn_call(addr: SocketAddr, session: &str, id: i64) -> JoinHandle<(u16, String)> {
    let headers = session_header(session);
    let body = call_body(id);
    tokio::spawn(async move { post(addr, &headers, &body, "a queued tools/call").await })
}

// ---------------------------------------------------------------------------
// Frame readers.
// ---------------------------------------------------------------------------

/// Drain a session stream until the reply to `id` arrives, collecting every
/// `notifications/message` frame seen BEFORE it.
///
/// Ordering is a property of the TRANSPORT, not of timing: the handler's records
/// are pushed onto the session's sender while it runs and its reply only after it
/// returns, through the same FIFO channel. So "the record arrived, or it did not"
/// is decided by the time the reply lands — no quiet window and no sleep.
async fn records_before_reply(conn: &mut Conn, id: i64, what: &str) -> Vec<Value> {
    let mut records = Vec::new();
    loop {
        let frame = conn.frame(what).await;
        if frame.get("method").and_then(Value::as_str) == Some(LOG_METHOD) {
            records.push(frame);
            continue;
        }
        if frame.get("id").and_then(Value::as_i64) == Some(id) {
            return records;
        }
    }
}

/// The next `n` frames on a stream, each of which MUST be a log record.
///
/// For the ingress path whose reply is answered inline: by the time the POST has
/// returned, every record the handler emitted has already been pushed onto the
/// session's sender, so this reads settled state rather than racing it.
async fn take_records(conn: &mut Conn, n: usize, what: &str) -> Vec<Value> {
    let mut records = Vec::new();
    for _ in 0..n {
        let frame = conn.frame(what).await;
        assert_eq!(
            frame.get("method").and_then(Value::as_str),
            Some(LOG_METHOD),
            "{what}: expected a {LOG_METHOD} frame, got {frame} — a MISSING record shows up here \
             as the next frame in line, which is what a level resolved on only one ingress path \
             looks like"
        );
        records.push(frame);
    }
    records
}

/// Every `notifications/message` frame in a completed multi-frame SSE body.
fn records_in_body(body: &str) -> Vec<Value> {
    body.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
        .filter(|frame| frame.get("method").and_then(Value::as_str) == Some(LOG_METHOD))
        .collect()
}

/// The `params.message` of each record, in arrival order.
fn messages_of(records: &[Value]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| record.pointer("/params/message").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

// ===========================================================================
// Wire fence 1 — a v1 record reaches the live session stream.
// ===========================================================================

#[tokio::test]
async fn a_v1_handler_log_record_reaches_the_live_session_stream() {
    let (addr, handle) = spawn_server(build_v1_server(), stateful_config()).await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let call = spawn_call(addr, &session, 2);
    let records = records_before_reply(&mut stream, 2, "the v1 tool reply").await;
    let _ = call.await;

    assert_eq!(
        messages_of(&records),
        vec![INFO_MESSAGE.to_string()],
        "the `info` record must arrive on the session's SSE stream, and the `debug` record must \
         not — nothing set a level, so the `info` default applies (D-12)"
    );
    assert_eq!(
        records[0]["params"]["level"], "info",
        "the level rides the record as its lowercase wire spelling"
    );
    assert_eq!(
        records[0]["method"], LOG_METHOD,
        "and the frame is the spec notification, not a progress frame"
    );

    teardown(handle, stream).await;
}

// ===========================================================================
// Wire fence 2 — a v2 record rides the multi-frame POST body.
// ===========================================================================

#[tokio::test]
async fn a_v2_handler_log_record_rides_the_multi_frame_post_body() {
    let (addr, handle) = spawn_server(build_dual_era_server(), stateful_config()).await;

    // The request AUTHORIZES logging. This fence is about the CARRIER — that a
    // v2 record rides the POST response body rather than a session stream — and
    // on v2 there is no record at all unless the caller asked for one
    // (SEP-2575; see `a_v2_request_without_a_log_level_emits_nothing`).
    let (status, body) = post(
        addr,
        &v2_call_headers(),
        &v2_call_body(2, Some(&json!("info"))),
        "the v2 logging call",
    )
    .await;

    assert_eq!(status, 200, "a v2 tools/call succeeds: {body}");
    let records = records_in_body(&body);
    assert_eq!(
        messages_of(&records),
        vec![INFO_MESSAGE.to_string()],
        "v2 delivers the record on the POST RESPONSE BODY as a multi-frame SSE notification: {body}"
    );
    assert!(
        body.rfind("\"result\"") > body.find(LOG_METHOD),
        "and the result frame comes AFTER the record, not before it: {body}"
    );

    teardown(handle, ()).await;
}

/// THE SEP-2575 negative, on the wire.
///
/// The vendored 2026-07-28 schema is explicit about the absent key: *"If absent,
/// the server MUST NOT send any notifications/message"*. The official suite's
/// `sep-2575-server-no-log-without-loglevel` scenario reds the server on a
/// SINGLE frame, so the assertion here is zero records, not few.
///
/// pmcp used to fail this: with no resolved level the emitter fell back to
/// `DEFAULT_LOG_LEVEL` (`info`, D-12) and emitted. That default is right for v1,
/// where absence means "the server MAY decide", and wrong for v2, where absence
/// is a prohibition. The rule now lives in
/// `server::core::attach_request_log_sink`, which gives such a request no log
/// SINK at all — so it holds for every handler rather than only the ones that
/// remember to check `extra.log_level`.
#[tokio::test]
async fn a_v2_request_without_a_log_level_emits_nothing() {
    let (addr, handle) = spawn_server(build_dual_era_server(), stateful_config()).await;

    let (status, body) = post(
        addr,
        &v2_call_headers(),
        &v2_call_body(2, None),
        "the v2 call carrying no log level",
    )
    .await;

    assert_eq!(
        status, 200,
        "the call still SERVES — logging is not authorized, which is not a failure: {body}"
    );
    assert_eq!(
        messages_of(&records_in_body(&body)),
        Vec::<String>::new(),
        "not ONE `notifications/message` frame: the handler emits unconditionally, so anything \
         here means the emitter still has a vehicle the client never authorized: {body}"
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// Wire fence 3 — the v1 level is PER SESSION (D-11 / T-118.2-07-01, ASVS V3).
//
// This is the fence that would have caught the deferred cross-session
// `client_capabilities` misattribution had it existed: the defect class is "one
// client's request changed another client's behaviour", and it is invisible to
// every single-session test in the suite.
// ===========================================================================

#[tokio::test]
async fn the_v1_level_is_per_session_and_client_b_cannot_change_client_a() {
    let (addr, handle) = spawn_server(build_v1_server(), stateful_config()).await;
    let session_a = open_session(addr).await;
    let session_b = open_session(addr).await;
    assert_ne!(session_a, session_b, "two distinct sessions are in play");

    // B asks for `debug`; A asks for nothing. Sent BEFORE either stream opens so
    // the reply is answered inline instead of landing on a stream a later
    // assertion reads.
    let (status, body) = post(
        addr,
        &session_header(&session_b),
        &set_level_body(2, &json!("debug")),
        "B's logging/setLevel",
    )
    .await;
    assert_eq!(status, 200, "logging/setLevel is served on v1: {body}");

    let mut stream_a = open_stream(addr, &session_a).await;
    let mut stream_b = open_stream(addr, &session_b).await;

    let call_a = spawn_call(addr, &session_a, 3);
    let records_a = records_before_reply(&mut stream_a, 3, "A's tool reply").await;
    let _ = call_a.await;

    let call_b = spawn_call(addr, &session_b, 4);
    let records_b = records_before_reply(&mut stream_b, 4, "B's tool reply").await;
    let _ = call_b.await;

    assert_eq!(
        messages_of(&records_b),
        vec![DEBUG_MESSAGE.to_string(), INFO_MESSAGE.to_string()],
        "B set `debug`, so B receives BOTH records"
    );
    assert_eq!(
        messages_of(&records_a),
        vec![INFO_MESSAGE.to_string()],
        "A set nothing, so A must still be filtering at the `info` default — a level stored on the \
         shared Arc<Mutex<Server>> instead of on the session would have leaked B's choice into A's \
         stream (T-118.2-07-01)"
    );

    teardown(handle, (stream_a, stream_b)).await;
}

// ===========================================================================
// Wire fence 4 — the v2 `_meta` key is PER REQUEST (D-10).
// ===========================================================================

#[tokio::test]
async fn the_v2_meta_key_sets_the_level_for_that_request_only() {
    let (addr, handle) = spawn_server(build_dual_era_server(), stateful_config()).await;

    let (status, with_key) = post(
        addr,
        &v2_call_headers(),
        &v2_call_body(2, Some(&json!("debug"))),
        "the v2 call carrying a log level",
    )
    .await;
    assert_eq!(status, 200, "the v2 call with a level succeeds: {with_key}");

    let (status, without_key) = post(
        addr,
        &v2_call_headers(),
        &v2_call_body(3, None),
        "the v2 call carrying no log level",
    )
    .await;
    assert_eq!(status, 200, "the v2 call without a level succeeds too");

    assert_eq!(
        messages_of(&records_in_body(&with_key)),
        vec![DEBUG_MESSAGE.to_string(), INFO_MESSAGE.to_string()],
        "the request that carried `{LOG_LEVEL_META_KEY}: debug` receives both records — before \
         this plan the key was declared and read by nothing: {with_key}"
    );
    assert_eq!(
        messages_of(&records_in_body(&without_key)),
        Vec::<String>::new(),
        "and the very next request, which carried no key, receives NOTHING — v2 is session-free, \
         so a level that persisted would be state the era does not have, and on v2 an absent key \
         is a prohibition rather than a fall-back to the default (SEP-2575): {without_key}"
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// Wire fence 5 — the default is `info` when nothing set one (D-12).
// ===========================================================================

#[tokio::test]
async fn an_unset_level_defaults_to_info() {
    let (addr, handle) = spawn_server(build_v1_server(), stateful_config()).await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let call = spawn_call(addr, &session, 2);
    let records = records_before_reply(&mut stream, 2, "the tool reply").await;
    let _ = call.await;

    let messages = messages_of(&records);
    assert!(
        messages.iter().any(|m| m == INFO_MESSAGE),
        "an `info` record is DELIVERED under the default: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m == DEBUG_MESSAGE),
        "and a `debug` record is SUPPRESSED — both directions, because a default that let \
         everything through would pass the delivery half alone: {messages:?}"
    );

    teardown(handle, stream).await;
}

// ===========================================================================
// Wire fence 6 — a malformed level is inert, unechoed and non-fatal
// (T-118.2-07-03 / T-118.2-07-04, ASVS V5 / V7).
// ===========================================================================

#[tokio::test]
async fn a_malformed_level_value_is_ignored_and_not_echoed() {
    let (addr, handle) = spawn_server(build_dual_era_server(), stateful_config()).await;

    // (a) the v2 `_meta` key.
    let (status, body) = post(
        addr,
        &v2_call_headers(),
        &v2_call_body(2, Some(&json!(MALFORMED_LEVEL))),
        "the v2 call carrying a malformed level",
    )
    .await;
    assert_eq!(
        status, 200,
        "the request still SERVES — an advisory diagnostic hint must not become an availability \
         failure: {body}"
    );
    assert!(
        !body.contains(MALFORMED_LEVEL),
        "and the peer's own bytes are never echoed back, in the result or in any emitted record: \
         {body}"
    );
    let records = records_in_body(&body);
    assert_eq!(
        messages_of(&records),
        Vec::<String>::new(),
        "an unparseable level is no level, and on v2 no level is a PROHIBITION (SEP-2575) — the \
         fail-closed reading, so a peer cannot obtain logging it did not correctly ask for: {body}"
    );
    for record in &records {
        assert!(
            !record.to_string().contains(MALFORMED_LEVEL),
            "no emitted record may carry the peer's value: {record}"
        );
    }

    // (b) the v1 RPC, whose whole purpose IS the level.
    //
    // MEASURED, and it differs from the v2 arm above: a `logging/setLevel` whose
    // `params.level` is not a level fails TYPED PARSING in
    // `parse_transport_message_fast`, long before this plan's ingress capture
    // runs, and is answered `400` / `-32601`. That rejection is PRE-EXISTING —
    // it is the shared `ClientRequest` deserializer, not the capture — and
    // changing it means changing the deserialization of a public type, which is
    // outside this plan. Recorded in the phase's `deferred-items.md` rather than
    // asserted away.
    //
    // What this plan DOES claim of that path is asserted here and holds: no
    // panic, no echo of the peer's bytes, and nothing stored.
    let session = open_session(addr).await;
    let (status, body) = post(
        addr,
        &session_header(&session),
        &set_level_body(3, &json!(MALFORMED_LEVEL)),
        "a v1 logging/setLevel carrying a malformed level",
    )
    .await;
    assert_eq!(
        status, 400,
        "the pre-existing typed parse rejects it — see the note above; a change here is a \
         deliberate edit, not drift: {body}"
    );
    assert!(
        !body.contains(MALFORMED_LEVEL),
        "and the rejection does NOT echo the peer's value, which is this plan's claim \
         (T-118.2-07-04): {body}"
    );

    let mut stream = open_stream(addr, &session).await;
    let call = spawn_call(addr, &session, 4);
    let records = records_before_reply(&mut stream, 4, "the tool reply after a bad setLevel").await;
    let _ = call.await;
    assert_eq!(
        messages_of(&records),
        vec![INFO_MESSAGE.to_string()],
        "a malformed setLevel stores nothing, so the session is still at the `info` default"
    );

    teardown(handle, stream).await;
}

// ===========================================================================
// Wire fence 7 — a setLevel for an unknown session mints nothing
// (T-118.2-07-02).
// ===========================================================================

#[tokio::test]
async fn a_set_level_for_an_unknown_session_id_inserts_no_session() {
    let (addr, handle) = spawn_server(build_v1_server(), stateful_config()).await;
    let invented = "session-that-was-never-issued";

    let (status, body) = post(
        addr,
        &session_header(invented),
        &set_level_body(2, &json!("debug")),
        "a setLevel for an invented session id",
    )
    .await;
    assert_eq!(
        status, 404,
        "an unknown session id is rejected before dispatch: {body}"
    );

    // The observable for "nothing was minted": the SAME id is still unknown. A
    // write that had grown a row would make this second request succeed, which is
    // exactly how a caller would inflate the session map by guessing ids.
    let (status, body) = post(
        addr,
        &session_header(invented),
        &call_body(3),
        "a follow-up call on the invented session id",
    )
    .await;
    assert_eq!(
        status, 404,
        "the invented session is STILL unknown, so the setLevel inserted no row: {body}"
    );

    teardown(handle, ()).await;
}

// ===========================================================================
// Wire fence 8 — the level is honoured behind HTTP middleware too
// (T-118.2-07-06).
//
// BEHAVIOURAL, not structural. `handle_post_request` routes on
// `config.http_middleware.is_none()`, so an EMPTY chain is enough to send every
// POST down the middleware ingress path — which means the both-paths claim can be
// measured on the wire rather than asserted about the source. Without this fence
// a resolution written at only one of the two `attach_v2_progress_sink` call
// sites passes every other fence in this file.
//
// # Why this fence AWAITS the call instead of draining to the reply
//
// MEASURED, and it is a PRE-EXISTING divergence between the two ingress paths
// that has nothing to do with the log level: the fast path frames its reply
// through `build_response`, which hands it to the session's SSE stream and
// answers `202`; the middleware path frames its reply through
// `build_success_response_with_middleware`, which always answers INLINE and never
// consults the stream. The log RECORDS still reach the stream on both paths —
// they ride `attach_session_backchannel`'s session-keyed notification sink, which
// is shared — so this fence awaits the POST (after which every record has
// already been pushed) and then reads them off the stream.
// ===========================================================================

#[tokio::test]
async fn the_level_is_honoured_behind_http_middleware_too() {
    let (addr, handle) = spawn_server(build_v1_server(), middleware_config()).await;
    let session = open_session(addr).await;

    let (status, body) = post(
        addr,
        &session_header(&session),
        &set_level_body(2, &json!("debug")),
        "a setLevel on the middleware ingress path",
    )
    .await;
    assert_eq!(status, 200, "logging/setLevel is served here too: {body}");

    let mut stream = open_stream(addr, &session).await;
    let (status, body) = post(
        addr,
        &session_header(&session),
        &call_body(3),
        "the tool call behind middleware",
    )
    .await;
    assert_eq!(status, 200, "the tool call is served here too: {body}");
    let records = take_records(&mut stream, 2, "the records behind middleware").await;

    assert_eq!(
        messages_of(&records),
        vec![DEBUG_MESSAGE.to_string(), INFO_MESSAGE.to_string()],
        "the middleware ingress path must resolve the level exactly as the fast path does — a \
         control present in one deployment shape and absent in the other is indistinguishable \
         from no control at all for the deployment that takes the other path"
    );

    teardown(handle, stream).await;
}

// ===========================================================================
// Wire fence 9 — `logging/setLevel` answers a LITERAL empty object on v1
// (Phase 118.2 plan 08, D-13 / Pitfall 8 / T-118.2-08-02).
//
// The referee's own predicate, copied VERBATIM out of the pinned suite's
// `2025-11-25:logging-set-level` scenario
// (`conformance/node_modules/@modelcontextprotocol/conformance/dist/index.js`):
//
//     let n = await e.connect(),
//         r = await n.request(`logging/setLevel`, {level:`info`}),
//         i = [];
//     r && Object.keys(r).length > 0 && i.push(`Expected empty object {} response`)
//
// ANY non-empty object is a FAILURE — an acknowledgement object, and an echo of
// the level just set, both red a currently-green BLOCKING scenario.
//
// This fence is deliberately STRICTER than the referee in one direction: `null`
// passes the referee, because `r &&` short-circuits, but the scenario's own
// description says "Return empty object `{}`" and a `null` result is not that.
// So the assertion is structural in two parts — `is_object()` AND zero keys —
// rather than a truthiness check or a bare `== json!({})`, which would accept
// `null` on the first count and say nothing about WHY on the second.
//
// Both ingress paths are measured, because they deliver the reply differently:
// the fast path frames it through the session's SSE stream and answers `202`,
// while the middleware path always answers INLINE. That divergence predates this
// phase and has nothing to do with logging (recorded by plan 07); the SHAPE
// under test is the same on both, which is the point of asserting it twice.
// ===========================================================================

/// Assert a JSON-RPC reply carries a result that is an object with ZERO keys.
fn assert_literal_empty_object(reply: &Value, what: &str) {
    assert!(
        reply.get("error").is_none(),
        "{what}: `logging/setLevel` is SERVED on v1, not refused: {reply}"
    );
    let result = reply
        .get("result")
        .unwrap_or_else(|| panic!("{what}: the reply carries no `result`: {reply}"));
    let object = result.as_object().unwrap_or_else(|| {
        panic!(
            "{what}: the result must be an OBJECT — `null` passes the referee's `r &&` \
             short-circuit but is not the `{{}}` the scenario asks for: {reply}"
        )
    });
    assert!(
        object.is_empty(),
        "{what}: the result must have ZERO keys — `Object.keys(r).length > 0` is the referee's \
         failure predicate, so an acknowledgement object or an echo of the level just set reds a \
         currently-green blocking scenario (and echoing would hand a caller a READ of session \
         state through a WRITE endpoint): {reply}"
    );
}

/// Drain a session stream until the reply to `id` arrives.
async fn reply_on_stream(conn: &mut Conn, id: i64, what: &str) -> Value {
    loop {
        let frame = conn.frame(what).await;
        if frame.get("id").and_then(Value::as_i64) == Some(id) {
            return frame;
        }
    }
}

/// The JSON-RPC frame for `id` in a completed response body, whether that body is
/// a bare JSON object or an SSE `data:`-framed stream.
fn reply_in_body(body: &str, id: i64, what: &str) -> Value {
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            if let Ok(frame) = serde_json::from_str::<Value>(data.trim()) {
                if frame.get("id").and_then(Value::as_i64) == Some(id) {
                    return frame;
                }
            }
        }
    }
    if let Ok(frame) = serde_json::from_str::<Value>(body.trim()) {
        if frame.get("id").and_then(Value::as_i64) == Some(id) {
            return frame;
        }
    }
    panic!("{what}: no JSON-RPC reply for id {id} in {body}");
}

#[tokio::test]
async fn v1_set_logging_level_answers_a_literal_empty_object() {
    // (a) the FAST ingress path — reply framed onto the session stream.
    let (addr, handle) = spawn_server(build_v1_server(), stateful_config()).await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;
    let (status, body) = post(
        addr,
        &session_header(&session),
        &set_level_body(2, &json!("info")),
        "a well-formed v1 logging/setLevel on the fast path",
    )
    .await;
    assert_eq!(
        status, 202,
        "the fast path hands the reply to the session's SSE stream (a PRE-EXISTING ingress \
         divergence recorded by plan 07, not a logging behaviour): {body}"
    );
    let reply = reply_on_stream(&mut stream, 2, "the setLevel reply on the fast path").await;
    assert_literal_empty_object(&reply, "the fast ingress path");
    teardown(handle, stream).await;

    // (b) the MIDDLEWARE ingress path — reply answered inline.
    let (addr, handle) = spawn_server(build_v1_server(), middleware_config()).await;
    let session = open_session(addr).await;
    let (status, body) = post(
        addr,
        &session_header(&session),
        &set_level_body(2, &json!("info")),
        "a well-formed v1 logging/setLevel behind middleware",
    )
    .await;
    assert_eq!(status, 200, "the middleware path answers inline: {body}");
    let reply = reply_in_body(&body, 2, "the setLevel reply behind middleware");
    assert_literal_empty_object(&reply, "the middleware ingress path");
    teardown(handle, ()).await;
}

// ===========================================================================
// Scope fence — `Subscribe`, `Unsubscribe` and `Ping` are UNCHANGED.
//
// Without this, "only `SetLoggingLevel` moved out of the residual arm" is a
// claim rather than a fact. `ping` in particular already carries a recorded
// 118.1 v2 behaviour change at the transport gate; disturbing it here would be a
// NEW divergence wearing a conformance fix's clothes.
//
// Both halves are asserted: the SOURCE still shows the three-method residual arm
// intact on one root and the `-32601` catch-all still covering them on the
// other, and the BEHAVIOUR still answers what it answered before.
// ===========================================================================

#[tokio::test]
async fn subscribe_unsubscribe_and_ping_are_unchanged_by_this_plan() {
    use pmcp::server::core::ProtocolHandler;
    use pmcp::types::{ClientRequest, Request};

    // --- source: the residual arm still holds exactly THREE methods.
    let residual = "ClientRequest::Subscribe(_) | ClientRequest::Unsubscribe(_) \
                    | ClientRequest::Ping => { Ok(serde_json::json!({})) }";
    assert!(
        squeezed_code(SERVER_ROOT).contains(residual),
        "the `Subscribe | Unsubscribe | Ping` residual arm in `Server::process_client_request` \
         must still answer `json!({{}})` — this plan removes `SetLoggingLevel` from it and \
         NOTHING else"
    );
    assert!(
        !residual.contains("SetLoggingLevel"),
        "and the needle above must not silently re-admit the method this plan split out"
    );

    // --- source: `ServerCore`'s catch-all still covers the three.
    assert!(
        squeezed_code(CORE_ROOT).contains("METHOD_NOT_SUPPORTED_MESSAGE.to_string()"),
        "`ServerCore`'s `_ =>` catch-all — the arm the three residual methods still fall through \
         to — must still answer `METHOD_NOT_FOUND`"
    );

    // --- behaviour, `ServerCore` root: all three still fall through to -32601.
    //
    // `ServerCore` implements the PUBLIC `ProtocolHandler` trait, so this root
    // can be driven directly. `Server` cannot: its `handle_request` is a private
    // inherent method and it implements no public dispatch trait, which is why
    // its half of this fence is the source assertion above plus the wire probe
    // below rather than a matching in-process call.
    let core = pmcp::server::builder::ServerCoreBuilder::new()
        .name("residual-scope-fence-core")
        .version("1.0.0")
        .stateless_mode(true)
        .build()
        .expect("core builds");

    for (name, request) in [
        (
            "resources/subscribe",
            ClientRequest::Subscribe(pmcp::types::SubscribeRequest {
                uri: "mem://x".to_string(),
            }),
        ),
        (
            "resources/unsubscribe",
            ClientRequest::Unsubscribe(pmcp::types::UnsubscribeRequest {
                uri: "mem://x".to_string(),
            }),
        ),
        ("ping", ClientRequest::Ping),
    ] {
        let answer = core
            .handle_request(
                pmcp::RequestId::from(1i64),
                Request::Client(Box::new(request)),
                None,
            )
            .await;
        let json = serde_json::to_value(&answer).expect("serializes");
        assert_eq!(
            json.pointer("/error/code"),
            Some(&json!(-32601)),
            "{name}: `ServerCore`'s `_ =>` catch-all must still answer -32601. The divergence \
             from `Server`'s `{{}}` is RECORDED, not fixed — and `ping` in particular already \
             carries a 118.1 v2 behaviour change that must not be disturbed here: {json}"
        );
    }

    // --- behaviour, `Server` root, over the wire: v1 `ping` still answers `{}`.
    //
    // `ping` is the sharp one of the three: it is the only residual method with
    // a recorded 118.1 v2 behaviour change, so a fence that only read the source
    // would not notice this plan disturbing its v1 answer.
    let (addr, handle) = spawn_server(build_v1_server(), middleware_config()).await;
    let session = open_session(addr).await;
    let (status, body) = post(
        addr,
        &session_header(&session),
        &envelope("ping", 2, &json!({})),
        "a v1 ping",
    )
    .await;
    assert_eq!(status, 200, "v1 ping is served: {body}");
    let reply = reply_in_body(&body, 2, "the v1 ping reply");
    assert_eq!(
        reply.get("result"),
        Some(&json!({})),
        "v1 `ping` must still answer a bare `{{}}` through the residual arm — unchanged by this \
         plan: {reply}"
    );
    teardown(handle, ()).await;
}

// ===========================================================================
// Structural fence — one shared unit, both roots, and the crate-internal
// behavioural fences still exist.
//
// The v2 half of D-13 is NOT reachable from an integration test:
// `ClientRequest::SetLoggingLevel` carries no `_meta`, so
// `ProtocolHandler::handle_request` — the only PUBLIC dispatch entry — always
// resolves `era == None` for it and can exercise the v1 branch only. The
// era-bearing seams (`Server::handle_request_with_context`,
// `ServerCore::handle_request_internal`) are `pub(crate)` and private. So the
// behavioural v2 and twin-root fences live in `src/server/core.rs`, the one file
// that can reach both, exactly as plan 06's sink fences do — and this fence
// guards them from silent deletion by NAME.
// ===========================================================================

/// The crate-internal fences this file cannot host but must not lose.
const CRATE_INTERNAL_SET_LEVEL_FENCES: [&str; 3] = [
    "v2_set_logging_level_is_retired_on_the_dispatch_root",
    "both_dispatch_roots_agree_about_set_logging_level",
    "the_v1_answer_is_an_object_with_zero_keys_on_both_roots",
];

/// [`code_lines`] with every run of whitespace collapsed to a single space.
///
/// A needle spanning more than one token must survive `rustfmt` deciding to put
/// a match arm on one line instead of three. Squeezing keeps the assertion about
/// the CODE rather than about the formatter's current line-width arithmetic.
fn squeezed_code(path: &str) -> String {
    code_lines(path)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn both_dispatch_roots_answer_set_logging_level_from_one_shared_unit() {
    let core_root = squeezed_code(CORE_ROOT);
    let server_root = squeezed_code(SERVER_ROOT);

    assert!(
        core_root.contains("pub(crate) fn set_logging_level_response("),
        "the shared unit is DEFINED in core.rs — `mod.rs` calls the units this file owns, it never \
         defines its own (the twin-site parity rule)"
    );
    assert!(
        core_root
            .contains("ClientRequest::SetLoggingLevel { .. } => { set_logging_level_response("),
        "`ServerCore`'s dispatch arm must CALL the shared unit rather than falling through to the \
         `_ =>` catch-all, which answered -32601 on BOTH eras"
    );
    assert!(
        server_root.contains("crate::server::core::set_logging_level_response("),
        "`Server`'s adapter must call the SAME unit — two copies of an era branch are two chances \
         to disagree, silently"
    );
    assert!(
        core_root.contains("METHOD_NOT_SUPPORTED_MESSAGE.to_string()"),
        "and the v2 retirement must reuse the one message literal this file's `_ =>` catch-all \
         already answers with, so the retirement of `logging/setLevel` is spelled exactly like \
         the retirement of its three residual siblings on this root"
    );

    // The era branch reaches the arm through the SAME projection both roots use.
    for (label, source) in [("core.rs", &core_root), ("mod.rs", &server_root)] {
        assert!(
            source.contains("protocol_context.as_ref().map(|ctx| ctx.era)"),
            "{label}: the arm must read the ALREADY-RESOLVED era off the request's \
             `ProtocolContext` — never re-derive it, which would make this layer a second era \
             resolver"
        );
    }

    for fence in CRATE_INTERNAL_SET_LEVEL_FENCES {
        assert!(
            core_root.contains(fence),
            "the crate-internal fence `{fence}` is gone. It cannot be rewritten here — an \
             integration test cannot present a v2 `logging/setLevel` to a native dispatch root at \
             all — so deleting it silently removes the only coverage the v2 half has"
        );
    }
}

// ===========================================================================
// The `LogMessageParams` wire contract — RESOLVED (plan 118.2-13, Option A).
//
// `schema/vendored/core-2026-07-28/schema.ts` declares
// `LoggingMessageNotificationParams` as `{ level, logger?, data }` — `data` is
// REQUIRED and there is no `message` member at all. pmcp's `LogMessageParams` is
// the reverse shape in RUST: a required `message: String` and an
// `Option<Value> data` skipped when `None`.
//
// # This fence used to assert the two sides DISAGREE. They no longer do.
//
// Plan 118.2-08 wrote it that way on the premise that "no suite scenario
// validates an emitted notification's params", and on that basis DECLARED the
// divergence rather than fixing it. Plan 118.2-11 measured the premise FALSE.
// `WireSchemaValid` is not a scenario — it is a check that runs INSIDE scenarios
// and validates every frame the implementation sends. At the held
// `0.2.0-alpha.11` pin the `2025-11-25` leg regressed 72/2 -> 71/3, entirely on
// `tools-call-with-logging` (1/1 -> 0/2), and `WireSchemaValid` newly failed with
// `messagesValidated: 10`, quoting all three frames as
// `LoggingMessageNotification/params: must have required property 'data'`.
// Reproduced directly against the pinned bundle:
//
//     {level:'info', message:'Tool execution started'}  -> parse ok: false
//         invalid_type at params.data: expected nonoptional, received undefined
//     {level:'info', data:'Tool execution started'}     -> parse ok: true
//
// The reference client's `LoggingMessageNotificationParamsSchema` uses
// `z.unknown()`, which is NON-OPTIONAL under the bundled zod v4, so a frame
// without `data` is dropped on the floor.
//
// # The resolution: Option A — default `data`, change no Rust API
//
// `emit_log_record` now populates `data` with the message string when the caller
// supplied none. `message` stays on the wire as a pmcp extension: the schema does
// not close `additionalProperties`, and the measurement above shows the
// `data`-bearing frame parses `ok: true` even with `message` also present.
//
// Rejected at the same checkpoint: B (wrap `data` as `{"message": ...}`),
// C (drop `message` — breaking), and D (change only the conformance fixture,
// rejected as gaming the referee: it turns the suite green while every real
// `extra.log` caller keeps emitting non-conformant frames).
//
// The fence keeps its valuable half — it READS the in-repo vendored schema rather
// than a hardcoded copy, so a re-vendor still moves it — and inverts its payload
// half: the emitted frame must now SATISFY the schema's `data` requirement.
// ===========================================================================

const VENDORED_V2_SCHEMA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schema/vendored/core-2026-07-28/schema.ts"
);

#[test]
fn the_emitted_frame_satisfies_the_vendored_schemas_required_data_member() {
    let schema = std::fs::read_to_string(VENDORED_V2_SCHEMA)
        .unwrap_or_else(|e| panic!("{VENDORED_V2_SCHEMA} is readable: {e}"));
    let start = schema
        .find("export interface LoggingMessageNotificationParams")
        .expect("the vendored schema declares LoggingMessageNotificationParams");
    let body = &schema[start
        ..start
            + schema[start..]
                .find("\n}")
                .expect("the interface is closed")];

    // MEMBER lines only. The `data` member's own doc comment reads "such as a
    // string message", so an absence assertion over the whole interface body
    // would be satisfied by prose rather than by a declaration.
    let members: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('*') && !line.starts_with("/*") && !line.starts_with("//"))
        .collect();
    assert!(
        members.contains(&"data: unknown;"),
        "the vendored 2026-07-28 schema must still declare `data` as REQUIRED (no `?`). If this \
         changed, the verdict recorded above changed with it: {members:?}"
    );
    assert!(
        !members.iter().any(|line| line.starts_with("message")),
        "and it must still declare NO `message` member: {members:?}"
    );

    // The other side of the divergence, from a live emission rather than from a
    // reading of `src/` — the shape a client actually receives today.
    let capture = Capture::default();
    let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());
    extra
        .log(LoggingLevel::Warning, "hello")
        .expect("the emit succeeds");
    let records = capture.json();
    let emitted = records.first().expect("exactly one record was emitted");
    let params = emitted
        .pointer("/params")
        .unwrap_or_else(|| panic!("the record carries params: {emitted}"));

    assert_eq!(
        params.get("data").and_then(Value::as_str),
        Some("hello"),
        "the emitted frame must SATISFY the schema's required `data` member. An absent `data` is \
         what made the reference client drop every frame — its `z.unknown()` is non-optional \
         under zod v4 — and cost `2025-11-25:tools-call-with-logging` its 1/1: {params}"
    );
    assert_eq!(
        params.get("message").and_then(Value::as_str),
        Some("hello"),
        "and `message` still rides alongside as a pmcp extension. The schema does not close \
         `additionalProperties` and the reference client strips unknown members rather than \
         rejecting them, so keeping it costs nothing and removing it would be a breaking change \
         to a public type (Option C, rejected): {params}"
    );
}

// ===========================================================================
// The required-`data` contract, at the emitter (plan 118.2-13).
//
// Four fences, in the order the plan names them: the wire shape of a plain
// `log(..)`, the pass-through guarantee of `log_with_data(..)`, and the
// early-return ordering the change must not disturb. The rewritten schema fence
// above is the fourth.
// ===========================================================================

/// A plain `extra.log(level, message)` emits a `data` member holding the message.
///
/// The fence the whole plan exists for. `level` and `message` must still be
/// there, and `logger` must still be ABSENT — the emitter does not synthesise
/// one, and defaulting `data` must not have grown a second guess alongside it.
#[test]
fn a_plain_log_emits_the_required_data_member_carrying_the_message() {
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());

    extra
        .log(LoggingLevel::Info, "Tool execution started")
        .expect("log must be Ok(())");

    let records = capture.json();
    assert_eq!(records.len(), 1, "exactly one record must reach the sink");
    let params = records[0]
        .pointer("/params")
        .unwrap_or_else(|| panic!("the record carries params: {}", records[0]));

    // The member the vendored schema marks REQUIRED, and the exact frame the
    // pinned reference client parsed `ok: true` where the `message`-only one
    // parsed `ok: false`.
    assert_eq!(
        params.get("data").and_then(serde_json::Value::as_str),
        Some("Tool execution started"),
        "`data` must be present and must carry the message string; got {params}"
    );
    assert_eq!(
        params.get("level").and_then(serde_json::Value::as_str),
        Some("info"),
        "`level` must survive unchanged; got {params}"
    );
    assert_eq!(
        params.get("message").and_then(serde_json::Value::as_str),
        Some("Tool execution started"),
        "`message` must survive alongside `data` — Option A keeps it as a pmcp extension rather \
         than removing it (Option C, breaking); got {params}"
    );
    assert!(
        params.get("logger").is_none(),
        "`logger` must remain ABSENT — a synthesised logger category is a guess that looks \
         authoritative, and defaulting `data` is not licence to start guessing; got {params}"
    );
}

/// An explicitly supplied `data` is emitted verbatim and is NEVER overwritten.
///
/// A non-string JSON object on purpose: a naive "always set `data` to the
/// message" implementation would replace it with a string and this fence would
/// catch that. It passes both before and after the change — it is the regression
/// guard on the half of the behaviour that was already correct.
#[test]
fn an_explicitly_supplied_data_value_survives_verbatim() {
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());

    let supplied = json!({ "elapsedMs": 1_840, "table": "orders", "message": "not this one" });
    extra
        .log_with_data(LoggingLevel::Warning, "slow query", supplied.clone())
        .expect("log_with_data must be Ok(())");

    let records = capture.json();
    assert_eq!(records.len(), 1, "exactly one record must reach the sink");
    let params = records[0]
        .pointer("/params")
        .unwrap_or_else(|| panic!("the record carries params: {}", records[0]));

    assert_eq!(
        params.get("data"),
        Some(&supplied),
        "the caller's `data` must reach the wire byte-for-byte. If this is the message string, \
         the default was applied where a value was already supplied; got {params}"
    );
    assert_eq!(
        params.get("message").and_then(serde_json::Value::as_str),
        Some("slow query"),
        "and `message` is still the caller's message, not the data's own `message` key; \
         got {params}"
    );
}

/// A record below the effective level constructs NOTHING and never reaches the
/// sink.
///
/// This pins the early-return ordering the `data` default must not disturb: the
/// level short-circuit and the no-sink return both happen BEFORE any payload is
/// built, so a below-bar record allocates no data value. Asserted over every
/// level strictly below the bar rather than one sample, because the ordering is a
/// property of the function and not of a particular level.
#[test]
fn a_below_bar_record_never_reaches_the_sink_and_builds_no_payload() {
    let capture = Capture::new();
    let extra = RequestHandlerExtra::default()
        .with_log_sink(capture.sink())
        .with_log_level(LoggingLevel::Error);

    for below in [
        LoggingLevel::Debug,
        LoggingLevel::Info,
        LoggingLevel::Notice,
        LoggingLevel::Warning,
    ] {
        extra
            .log(below, "below the bar")
            .expect("a suppressed record still returns Ok(())");
        extra
            .log_with_data(below, "below the bar", json!({ "k": 1 }))
            .expect("a suppressed record still returns Ok(())");
    }

    assert_eq!(
        capture.len(),
        0,
        "nothing below the bar may reach the sink — the level check must stay the FIRST thing \
         `emit_log_record` does, ahead of the `data` default"
    );

    // And the bar itself still delivers, so the fence above is not passing
    // because the emitter went silent altogether.
    extra
        .log(LoggingLevel::Error, "at the bar")
        .expect("must be Ok(())");
    assert_eq!(
        capture.len(),
        1,
        "a record AT the bar must still be delivered"
    );
}

/// Property arm for `make test-property` (`cargo test --features "full" --
/// --ignored property_`).
///
/// The unit fences above pin the contract at chosen points; this arm asserts it
/// as an INVARIANT over arbitrary messages and every level at or above the bar:
///
/// * every delivered frame has a `data` member — never absent, whichever level
///   or message produced it;
/// * a plain `log(..)` puts the message string there, so the two members agree;
/// * an explicitly supplied `data` survives byte-for-byte, whatever the message
///   is — including the adversarial case of a message that is itself the JSON
///   text of some other value.
///
/// The message generator deliberately includes the empty string and text with
/// quotes and braces: `""` is the input a "default only when non-empty" bug
/// would slip through, and the rest would break a default implemented by string
/// concatenation rather than by `Value::String`.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_every_delivered_log_frame_carries_a_data_member() {
    use proptest::prelude::*;

    // Levels at or above the bar, so every generated record is DELIVERED. The
    // below-bar half is pinned exhaustively by
    // `a_below_bar_record_never_reaches_the_sink_and_builds_no_payload`.
    let at_or_above = prop::sample::select(vec![
        LoggingLevel::Info,
        LoggingLevel::Notice,
        LoggingLevel::Warning,
        LoggingLevel::Error,
        LoggingLevel::Critical,
        LoggingLevel::Alert,
        LoggingLevel::Emergency,
    ]);

    proptest!(|(level in at_or_above, message in r#"[a-zA-Z0-9 {}"':,\-]{0,64}"#)| {
        // --- a plain log(..): `data` is defaulted to the message ---
        let capture = Capture::new();
        let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());
        extra.log(level, message.clone()).expect("log must be Ok(())");

        let records = capture.json();
        prop_assert_eq!(records.len(), 1, "a level at or above `info` must be delivered");
        let params = records[0]
            .pointer("/params")
            .expect("the record carries params")
            .clone();

        prop_assert_eq!(
            params.get("data").and_then(serde_json::Value::as_str),
            Some(message.as_str()),
            "every delivered frame must carry `data`, holding the message: {}",
            params
        );
        prop_assert_eq!(
            params.get("message").and_then(serde_json::Value::as_str),
            Some(message.as_str()),
            "and `message` must still be there alongside it: {}",
            params
        );

        // --- log_with_data(..): the caller's value is never overwritten ---
        let capture = Capture::new();
        let extra = RequestHandlerExtra::default().with_log_sink(capture.sink());
        let supplied = json!({ "supplied": true, "echo": message.clone() });
        extra
            .log_with_data(level, message.clone(), supplied.clone())
            .expect("log_with_data must be Ok(())");

        let records = capture.json();
        prop_assert_eq!(records.len(), 1, "the record must be delivered");
        let params = records[0]
            .pointer("/params")
            .expect("the record carries params")
            .clone();

        prop_assert_eq!(
            params.get("data"),
            Some(&supplied),
            "an explicitly supplied `data` must survive verbatim, never replaced by the \
             message: {}",
            params
        );
    });
}
