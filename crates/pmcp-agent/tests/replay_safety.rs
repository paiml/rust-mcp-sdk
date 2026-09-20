//! AGNT-03 replay-safety property + golden fixtures.
//!
//! The load-bearing durability contract: feeding IDENTICAL effect results to the
//! engine must yield IDENTICAL decision SEQUENCES — not merely identical final
//! results (Codex MEDIUM). The property runs the engine TWICE over one recorded
//! [`EffectTrace`] via `ReplaySource`/`ReplayInvoker` and asserts the two
//! [`DecisionTrace`]s are byte-for-byte equal. Two golden fixtures pin the exact
//! terminal outcome + decision sequence for an end-turn run and a tool-loop run.

use pmcp::types::content::Role;
use pmcp::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
use pmcp_agent::iteration::AgentEngine;
use pmcp_agent::trace::OutcomeTag;
use pmcp_agent::{
    ConversationStore, DecisionTrace, EffectTrace, InMemoryStore, ReplayInvoker, ReplaySource,
    ResolvedAgentConfig, ToolCallResult,
};
use proptest::prelude::*;
use serde_json::json;

// Additions for the D-08 era-mismatch cases at the foot of this file.
use pmcp::types::protocol::{Era, PROTOCOL_VERSION_2026_07_28};
use pmcp_agent::seams::{ToolCall, ToolInvoker};

/// A config with high limits so the loop terminates on model decisions, not
/// limits — keeping the property focused on decision determinism.
fn replay_config() -> ResolvedAgentConfig {
    ResolvedAgentConfig::new("be helpful", "golden-model", u32::MAX, 1000)
}

/// Run the engine once over `trace`, returning the recorded decision sequence.
///
/// Uses `futures::executor::block_on`: every seam here is pure/in-memory (no
/// timers or real I/O), so no tokio runtime is needed and each proptest case is
/// cheap.
fn run_once(trace: &EffectTrace, config: &ResolvedAgentConfig) -> DecisionTrace {
    run_with(trace, config, None)
}

/// The shared engine wiring behind [`run_once`] and [`run_once_with_live_era`].
///
/// `live_era` is the ONLY difference between the two. Keeping one body is what
/// makes "identical in every other respect" a fact rather than a claim two
/// copies have to be manually kept in step with — and three of the tests below
/// compare the two runs directly, so a drift between them would silently
/// invalidate the comparison rather than fail it.
fn run_with(
    trace: &EffectTrace,
    config: &ResolvedAgentConfig,
    live_era: Option<Era>,
) -> DecisionTrace {
    futures::executor::block_on(async {
        let source = ReplaySource::from_trace(trace);
        let invoker = match live_era {
            Some(era) => ReplayInvoker::from_trace(trace).with_live_era(era),
            None => ReplayInvoker::from_trace(trace),
        };
        let store = InMemoryStore::new();
        let run_id = "replay";
        if let Some(state) = &trace.initial_state {
            store.save(run_id, state).await.expect("seed initial state");
        }
        let engine = AgentEngine::new(source, invoker, store, config.clone());
        let mut decisions = DecisionTrace::default();
        engine.run_traced(run_id, &mut decisions).await;
        decisions
    })
}

fn text_completion(stop: &str, text: &str) -> CreateMessageResultWithTools {
    CreateMessageResultWithTools::new(
        "golden-model",
        Role::Assistant,
        vec![SamplingMessageContent::Text {
            text: text.into(),
            meta: None,
        }],
    )
    .with_stop_reason(stop)
}

fn tool_use_completion(id: &str) -> CreateMessageResultWithTools {
    CreateMessageResultWithTools::new(
        "golden-model",
        Role::Assistant,
        vec![SamplingMessageContent::ToolUse {
            name: "act".into(),
            id: id.into(),
            input: json!({}),
            meta: None,
        }],
    )
    .with_stop_reason("tool_use")
}

/// One scripted step of a generated run.
#[derive(Clone, Debug)]
enum ScriptStep {
    /// A terminal completion (stop_reason "stop").
    Final,
    /// A tool-use completion paired with one tool-batch result.
    Tool,
}

fn arb_script() -> impl Strategy<Value = Vec<ScriptStep>> {
    prop::collection::vec(
        prop_oneof![Just(ScriptStep::Final), Just(ScriptStep::Tool)],
        0..6,
    )
}

/// Build a CONSISTENT `EffectTrace`: one tool-batch per tool completion, always
/// terminated by a final completion (so the run Completes deterministically
/// rather than exhausting the trace).
fn build_trace(steps: &[ScriptStep]) -> EffectTrace {
    let mut completions = Vec::new();
    let mut batches = Vec::new();
    let mut ended = false;
    for (i, step) in steps.iter().enumerate() {
        match step {
            ScriptStep::Tool => {
                let tid = format!("tu-{i}");
                completions.push(tool_use_completion(&tid));
                batches.push(vec![ToolCallResult::ok(tid, json!({ "i": i as u64 }))]);
            },
            ScriptStep::Final => {
                completions.push(text_completion("stop", "final"));
                ended = true;
                break;
            },
        }
    }
    if !ended {
        completions.push(text_completion("stop", "final"));
    }
    EffectTrace::new(completions, batches)
}

proptest! {
    /// AGNT-03: identical effect results ⇒ identical DECISION SEQUENCES.
    #[test]
    fn identical_trace_yields_identical_decision_sequence(steps in arb_script()) {
        let trace = build_trace(&steps);
        let config = replay_config();
        let first = run_once(&trace, &config);
        let second = run_once(&trace, &config);
        // Full DecisionTrace equality = same step-by-step decisions AND outcome,
        // not just the final result.
        prop_assert_eq!(&first, &second);
        // The run terminated with an observable outcome.
        prop_assert!(first.outcome.is_some());
        // A generated run always terminates on a final completion.
        prop_assert_eq!(first.outcome, Some(OutcomeTag::Completed));
    }
}

#[test]
fn golden_end_turn_completes_in_one_step() {
    let trace: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_end_turn.json"))
            .expect("valid end-turn fixture");
    let decisions = run_once(&trace, &replay_config());

    assert_eq!(decisions.outcome, Some(OutcomeTag::Completed));
    assert_eq!(decisions.steps.len(), 1);
    assert!(decisions.steps[0].is_end_turn);
    assert!(decisions.steps[0].is_final);
    assert!(decisions.steps[0].tool_call_ids.is_empty());

    // Deterministic across runs.
    assert_eq!(decisions, run_once(&trace, &replay_config()));
}

#[test]
fn golden_tool_loop_dispatches_then_completes() {
    let trace: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_tool_loop.json"))
            .expect("valid tool-loop fixture");
    let decisions = run_once(&trace, &replay_config());

    assert_eq!(decisions.outcome, Some(OutcomeTag::Completed));
    assert_eq!(decisions.steps.len(), 2);

    // Step 0: dispatched tu-1, not final.
    assert!(!decisions.steps[0].is_end_turn);
    assert!(!decisions.steps[0].is_final);
    assert_eq!(decisions.steps[0].tool_call_ids, vec!["tu-1".to_string()]);

    // Step 1: end_turn ("stop"), final, no dispatch.
    assert!(decisions.steps[1].is_end_turn);
    assert!(decisions.steps[1].is_final);
    assert!(decisions.steps[1].tool_call_ids.is_empty());

    // Deterministic across runs.
    assert_eq!(decisions, run_once(&trace, &replay_config()));
}

// ===========================================================================
// D-08: a v1-recorded trace must not replay as v2 in SILENCE.
//
// Everything below is an ADDITION. No existing test body above is modified —
// which is itself the evidence that the undeclared-live-era default is right:
// every pre-117 caller keeps working with no edit.
// ===========================================================================

/// Run the engine once over `trace` with an explicitly DECLARED live era.
///
/// The era-aware sibling of [`run_once`]; identical in every other respect, so
/// a difference in the resulting `DecisionTrace` is attributable to the era
/// declaration and nothing else.
fn run_once_with_live_era(
    trace: &EffectTrace,
    config: &ResolvedAgentConfig,
    live_era: Era,
) -> DecisionTrace {
    run_with(trace, config, Some(live_era))
}

/// A one-tool-call trace recorded at `version`.
fn trace_recorded_at(version: &str) -> EffectTrace {
    EffectTrace::new(
        vec![
            tool_use_completion("tu-1"),
            text_completion("stop", "final"),
        ],
        vec![vec![ToolCallResult::ok("tu-1", json!({ "i": 0_u64 }))]],
    )
    .with_negotiated_version(version)
}

/// Drive `invoker` over `batch_count` batches and collect what it returned.
fn drive_batches(invoker: &ReplayInvoker, batch_count: usize) -> Vec<Vec<ToolCallResult>> {
    futures::executor::block_on(async {
        let mut observed = Vec::new();
        for index in 0..batch_count {
            let calls = vec![ToolCall {
                id: format!("tu-{index}"),
                name: "act".to_string(),
                arguments: json!({}),
                connector: None,
            }];
            observed.push(invoker.invoke_batch(calls).await);
        }
        observed
    })
}

/// The mismatch shape: ONE error on the first batch naming BOTH eras, empty
/// thereafter — and identical across two independent replays.
#[test]
fn an_era_mismatch_fails_deterministically() {
    let trace = trace_recorded_at("2025-11-25");

    let first = drive_batches(&ReplayInvoker::from_trace(&trace).with_live_era(Era::V2), 3);
    let second = drive_batches(&ReplayInvoker::from_trace(&trace).with_live_era(Era::V2), 3);

    assert_eq!(
        first.len(),
        3,
        "the invoker must answer every batch it is asked for"
    );
    assert_eq!(
        first[0].len(),
        1,
        "the FIRST batch carries exactly one error result; got {:?}",
        first[0]
    );
    assert!(
        first[0][0].is_error,
        "the mismatch result must be flagged as an error; got {:?}",
        first[0][0]
    );

    // The message names BOTH eras — not merely "something went wrong".
    let message = first[0][0].error.clone().unwrap_or_default();
    assert!(
        message.contains("V1"),
        "the mismatch message must name the RECORDED era; got {message:?}"
    );
    assert!(
        message.contains("V2"),
        "the mismatch message must name the LIVE era; got {message:?}"
    );

    assert!(
        first[1].is_empty() && first[2].is_empty(),
        "every batch after the first is empty, the same deterministic shape the exhaustion path \
         uses; got {:?}",
        &first[1..]
    );

    // DETERMINISM: two independent replays are equal.
    assert_eq!(
        first, second,
        "two replays of the same mismatched trace must be identical (AGNT-03)"
    );

    // And through the engine, the DecisionTraces are equal too.
    let config = replay_config();
    assert_eq!(
        run_once_with_live_era(&trace, &config, Era::V2),
        run_once_with_live_era(&trace, &config, Era::V2)
    );
}

/// A trace replayed at its OWN era behaves exactly as it does with no era
/// declared at all.
#[test]
fn a_matching_era_replays_unchanged() {
    let trace = trace_recorded_at(PROTOCOL_VERSION_2026_07_28);
    let config = replay_config();

    assert_eq!(
        ReplayInvoker::from_trace(&trace).recorded_era(),
        Era::V2,
        "a 2026-07-28 trace classifies as V2 via protocol_era"
    );

    let declared = run_once_with_live_era(&trace, &config, Era::V2);
    let undeclared = run_once(&trace, &config);
    assert_eq!(
        declared, undeclared,
        "declaring the MATCHING live era must change nothing"
    );
    assert_eq!(declared.outcome, Some(OutcomeTag::Completed));

    let batches = drive_batches(&ReplayInvoker::from_trace(&trace).with_live_era(Era::V2), 2);
    assert_eq!(
        batches[0],
        vec![ToolCallResult::ok("tu-1", json!({ "i": 0_u64 }))],
        "a matching era returns the RECORDED batch verbatim"
    );
    assert!(batches[1].is_empty(), "then exhausts as it always has");
}

/// THE LEGACY-TRACE POLICY, written down in a test.
///
/// A pre-117 trace records no version, so it classifies conservatively as V1.
/// Replaying it under an explicitly declared V2 live era therefore IS a mismatch
/// and DOES fail — a v1-recorded trace replayed as v2 is precisely the hole D-08
/// exists to close. The SAME trace with NO declared live era keeps behaving
/// exactly as it does today.
#[test]
fn a_legacy_version_less_trace_is_v1_and_fails_under_a_declared_v2_replay() {
    let legacy: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_tool_loop.json"))
            .expect("valid tool-loop fixture");
    assert_eq!(
        legacy.negotiated_version, None,
        "the pre-117 fixture is era-less on disk and stays that way"
    );
    assert_eq!(
        ReplayInvoker::from_trace(&legacy).recorded_era(),
        Era::V1,
        "an absent version classifies conservatively as V1"
    );

    // Declared V2 => mismatch, deterministic.
    let mismatched = drive_batches(
        &ReplayInvoker::from_trace(&legacy).with_live_era(Era::V2),
        2,
    );
    assert_eq!(mismatched[0].len(), 1);
    assert!(mismatched[0][0].is_error);
    assert!(mismatched[1].is_empty());

    // Declared V1 => no mismatch. Undeclared => no check at all. Both must be
    // byte-identical to the behaviour the golden test already pins.
    let config = replay_config();
    let undeclared = run_once(&legacy, &config);
    assert_eq!(undeclared.outcome, Some(OutcomeTag::Completed));
    assert_eq!(
        run_once_with_live_era(&legacy, &config, Era::V1),
        undeclared,
        "declaring the matching (V1) live era changes nothing for a legacy trace"
    );

    // MEASURED, and it is the whole reason the guard lives in the INVOKER: a
    // mismatched replay is INVISIBLE in a `DecisionTrace`. A `DecisionTrace`
    // records the DECISIONS (which tool ids were dispatched, end-turn, final,
    // limit, outcome) and never the effect CONTENT, so replaying a v1 trace
    // under v2 yields an identical decision sequence while the effects came
    // from a different protocol. That equality is exactly D-08's hole, so the
    // failure has to be injected where the agent can SEE it — as a tool result.
    assert_eq!(
        run_once_with_live_era(&legacy, &config, Era::V2),
        undeclared,
        "a DecisionTrace cannot see an era mismatch — if this ever differs, the guard has moved \
         out of the invoker and this test's premise needs rewriting"
    );
    let recorded_batches = drive_batches(&ReplayInvoker::from_trace(&legacy), 2);
    assert_ne!(
        mismatched, recorded_batches,
        "declaring V2 over a legacy (V1) trace must NOT hand back the recorded batches; the \
         mismatch is observable in the RESULTS, which is where it must be"
    );
}

/// A mismatched batch answers EVERY call in it, not just the first.
///
/// `invoke_batch` is a POSITIONAL contract — the Nth result belongs to the Nth
/// call — so a single error for an N-call batch leaves N-1 calls unanswered and
/// mis-attributes the one answer it does give. The failure must be visible on
/// each call the caller made.
#[test]
fn an_era_mismatch_answers_every_call_in_the_batch() {
    let trace = trace_recorded_at("2025-11-25");
    let invoker = ReplayInvoker::from_trace(&trace).with_live_era(Era::V2);

    let calls: Vec<ToolCall> = ["a", "b", "c"]
        .iter()
        .map(|id| ToolCall {
            id: (*id).to_string(),
            name: "act".to_string(),
            arguments: json!({}),
            connector: None,
        })
        .collect();
    let results = futures::executor::block_on(invoker.invoke_batch(calls));

    assert_eq!(
        results.len(),
        3,
        "one result per CALL, not one per batch; got {results:?}"
    );
    let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["a", "b", "c"],
        "each error must carry the id of the call it answers"
    );
    assert!(results.iter().all(|r| r.is_error));
}

/// The guard covers `invoke` too, not only `invoke_batch`.
///
/// `ReplayInvoker` is PUBLIC, so the single-call path is reachable by any
/// caller. A silent `ok(null)` there would be exactly the cross-era replay D-08
/// exists to close, reached through the other door.
#[test]
fn an_era_mismatch_also_fails_the_single_call_path() {
    let trace = trace_recorded_at("2025-11-25");
    let call = || ToolCall {
        id: "single".to_string(),
        name: "act".to_string(),
        arguments: json!({}),
        connector: None,
    };

    let mismatched = futures::executor::block_on(
        ReplayInvoker::from_trace(&trace)
            .with_live_era(Era::V2)
            .invoke(call()),
    );
    assert!(
        mismatched.is_error,
        "a mismatched single call must NOT come back ok; got {mismatched:?}"
    );
    assert_eq!(mismatched.id, "single");
    let message = mismatched.error.clone().unwrap_or_default();
    assert!(
        message.contains("V1") && message.contains("V2"),
        "{message:?}"
    );

    // The matching and undeclared cases stay byte-identical to before.
    let matching = futures::executor::block_on(
        ReplayInvoker::from_trace(&trace)
            .with_live_era(Era::V1)
            .invoke(call()),
    );
    assert!(!matching.is_error);
    let undeclared = futures::executor::block_on(ReplayInvoker::from_trace(&trace).invoke(call()));
    assert!(!undeclared.is_error);
}

proptest! {
    /// Under a mismatch, two independent replays are ALWAYS equal, for any
    /// number of batches.
    #[test]
    fn an_era_mismatch_is_deterministic_over_arbitrary_batch_counts(
        batch_count in 0_usize..8,
        recorded_is_v2 in any::<bool>(),
    ) {
        let recorded = if recorded_is_v2 { PROTOCOL_VERSION_2026_07_28 } else { "2025-11-25" };
        let live = if recorded_is_v2 { Era::V1 } else { Era::V2 };
        let trace = trace_recorded_at(recorded);

        let first = drive_batches(&ReplayInvoker::from_trace(&trace).with_live_era(live), batch_count);
        let second = drive_batches(&ReplayInvoker::from_trace(&trace).with_live_era(live), batch_count);

        prop_assert_eq!(&first, &second);
        // The shape is the deterministic one, whatever the batch count.
        if batch_count > 0 {
            prop_assert_eq!(first[0].len(), 1);
            prop_assert!(first[0][0].is_error);
            prop_assert!(first[1..].iter().all(Vec::is_empty));
        }
    }
}
