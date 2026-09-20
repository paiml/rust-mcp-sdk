//! Public serde replay substrate for the agent loop.
//!
//! It carries `EffectTrace` (recorded effect RESULTS) and `DecisionTrace` (the
//! ordered DECISIONS the engine took), plus the `ReplaySource` / `ReplayInvoker`
//! seams that feed a recorded `EffectTrace` back through the SAME engine.
//!
//! This is the durability contract of the phase (design §8.1): feeding identical
//! effect results to the loop must yield identical decision sequences. The
//! replay-safety property (`tests/replay_safety.rs`, AGNT-03) runs the engine
//! twice over one `EffectTrace` and asserts the two `DecisionTrace`s are equal.
//! Everything here is `serde_json`-`preserve_order` clean with no floats, so a
//! trace round-trips deterministically.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use pmcp::types::protocol::{protocol_era, Era};
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};
use serde::{Deserialize, Serialize};

use crate::iteration::result::LimitDecision;
use crate::seams::{
    CompletionError, CompletionSource, RetryClass, RunState, ToolCall, ToolCallResult, ToolInvoker,
};

/// A recorded sequence of effect RESULTS — the input side of replay (D-08).
///
/// Records everything the loop consumes from its seams: an optional
/// `initial_state` to load (the resume scenario), the ordered `completions` a
/// [`ReplaySource`] returns one-per-`create_message`, and the ordered
/// `tool_batches` a [`ReplayInvoker`] returns one-per-`invoke_batch`. A public
/// `#[derive(Serialize, Deserialize)]` artifact so proptest can generate it and
/// golden traces can live as fixtures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTrace {
    /// Optional pre-seeded state the store loads (drives resume). `None` = fresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<RunState>,
    /// The protocol version this trace was RECORDED against, as the wire string
    /// (e.g. `"2026-07-28"`).
    ///
    /// `None` means the trace was recorded before era tracking existed
    /// (pre-117). Classify it with [`pmcp::types::protocol::protocol_era`]
    /// rather than comparing strings: that classifier's unknown-to-`V1`
    /// conservative fallback makes any unrecognised value SAFE instead of a
    /// panic or an accidental v2 claim.
    ///
    /// Populate it from
    /// [`ConnectorClient::negotiated_protocol_version`](crate::invoker::ConnectorClient::negotiated_protocol_version)
    /// via [`Self::with_negotiated_version`] when recording against a live
    /// connector. Without it, [`ReplayInvoker`] cannot tell that a v1-recorded
    /// trace is being replayed as v2 — the exact hole D-08 exists to close.
    ///
    /// Stored as the VERSION STRING, not an `Era`: `Era` derives no
    /// `Serialize`/`Deserialize`, and adding those derives would put a new wire
    /// spelling (`"V1"`/`"V2"`) onto the core's compatibility surface for no
    /// benefit. The string preserves strictly more information and touches zero
    /// core API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negotiated_version: Option<String>,
    /// Ordered completion results, one returned per `create_message` call.
    pub completions: Vec<CreateMessageResultWithTools>,
    /// Ordered tool-batch results, one returned per `invoke_batch` call.
    #[serde(default)]
    pub tool_batches: Vec<Vec<ToolCallResult>>,
}

impl EffectTrace {
    /// Build a trace from completions and tool batches (fresh initial state).
    ///
    /// Deliberately UNCHANGED in arity: `EffectTrace` is a public,
    /// all-`pub`-fields struct and this is its in-repo construction path, so
    /// the era is attached with [`Self::with_negotiated_version`] rather than by
    /// widening this signature.
    #[must_use]
    pub fn new(
        completions: Vec<CreateMessageResultWithTools>,
        tool_batches: Vec<Vec<ToolCallResult>>,
    ) -> Self {
        Self {
            initial_state: None,
            negotiated_version: None,
            completions,
            tool_batches,
        }
    }

    /// Record the protocol version this trace was captured against.
    ///
    /// Feed it
    /// [`ConnectorClient::negotiated_protocol_version`](crate::invoker::ConnectorClient::negotiated_protocol_version)
    /// from the live connector the run used. A trace recorded without it stays
    /// byte-identical on the wire to a pre-117 trace — the key is omitted
    /// entirely rather than emitted as `null` — so this is purely additive.
    #[must_use]
    pub fn with_negotiated_version(mut self, version: impl Into<String>) -> Self {
        self.negotiated_version = Some(version.into());
        self
    }
}

/// One decision the engine made in a single completion step.
///
/// The comparison unit for replay-safety: it captures the OUTPUT of the pure
/// decision functions (end-turn?, final?, which tool ids were emitted, the
/// limit decision) — not wall-clock or ordering-sensitive state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionStep {
    /// The iteration index at which this decision was taken.
    pub iteration: u32,
    /// Whether the completion's stop reason ended the turn.
    pub is_end_turn: bool,
    /// Whether this step terminated the run.
    pub is_final: bool,
    /// The ids of the tool calls dispatched this step (empty when none).
    pub tool_call_ids: Vec<String>,
    /// The limit decision after this step, when one was evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<LimitDecision>,
}

/// The terminal outcome the engine returned, as a comparable tag.
///
/// Mirrors [`RunOutcome`](crate::iteration::RunOutcome) but drops the payload
/// message (which carries non-`Eq` SDK types) so a whole `DecisionTrace` is
/// `PartialEq + Eq` for the replay property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum OutcomeTag {
    /// The run completed.
    Completed,
    /// A limit stopped the run.
    LimitReached,
    /// A retryable failure occurred, carrying its classification.
    RetryRequired {
        /// The retry classification (transient / capacity).
        class: RetryClass,
    },
    /// A non-retryable failure occurred.
    Failed,
}

/// The ordered DECISIONS the engine took over one run — the replay artifact.
///
/// Two runs of the same engine over the same [`EffectTrace`] MUST produce equal
/// `DecisionTrace`s; that equality is the AGNT-03 replay-safety property.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionTrace {
    /// The ordered per-step decisions.
    pub steps: Vec<DecisionStep>,
    /// The terminal outcome tag, set once the run returns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<OutcomeTag>,
}

/// A [`CompletionSource`] that returns recorded completions from an [`EffectTrace`].
///
/// Replaces real I/O so the SAME engine runs deterministically. An exhausted
/// trace yields a fatal decode error (surfaced as `RunOutcome::Failed`) — which
/// is itself deterministic, so replay equality still holds.
#[derive(Debug)]
pub struct ReplaySource {
    completions: Vec<CreateMessageResultWithTools>,
    cursor: AtomicUsize,
}

impl ReplaySource {
    /// Build a replay source over an ordered list of completion results.
    #[must_use]
    pub fn new(completions: Vec<CreateMessageResultWithTools>) -> Self {
        Self {
            completions,
            cursor: AtomicUsize::new(0),
        }
    }

    /// Build a replay source from the completions recorded in `trace`.
    #[must_use]
    pub fn from_trace(trace: &EffectTrace) -> Self {
        Self::new(trace.completions.clone())
    }
}

#[async_trait]
impl CompletionSource for ReplaySource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        let index = self.cursor.fetch_add(1, Ordering::SeqCst);
        self.completions.get(index).cloned().ok_or_else(|| {
            CompletionError::Decode(format!("replay completion exhausted at index {index}"))
        })
    }
}

/// A [`ToolInvoker`] that returns recorded tool batches from an [`EffectTrace`].
///
/// Each `invoke_batch` call returns the next recorded batch; an exhausted trace
/// returns an empty batch (deterministic).
///
/// # Era mismatch (D-08)
///
/// A trace recorded against one protocol era and replayed under another is a
/// silent correctness hole: the two runs produce equal [`DecisionTrace`]s while
/// the underlying effects came from different protocols. This invoker carries
/// the era its trace was RECORDED under and, when a live era has been declared
/// with [`Self::with_live_era`], refuses to replay across a mismatch. See that
/// method for the two policies (undeclared live era, legacy version-less trace)
/// this behaviour deliberately encodes.
#[derive(Debug)]
pub struct ReplayInvoker {
    batches: Vec<Vec<ToolCallResult>>,
    cursor: AtomicUsize,
    /// The era the trace was RECORDED under, classified from the trace's
    /// `negotiated_version` by [`pmcp::types::protocol::protocol_era`].
    recorded_era: Era,
    /// The era the replay run is DECLARED to be executing under. `None` means
    /// no era claim was made, so no mismatch can be detected — see
    /// [`ReplayInvoker::with_live_era`].
    live_era: Option<Era>,
}

impl ReplayInvoker {
    /// Build a replay invoker over an ordered list of tool-batch results.
    ///
    /// A raw batch list carries no recorded version, so it classifies
    /// conservatively as [`Era::V1`] — the same unknown-to-`V1` rule the core's
    /// [`pmcp::types::protocol::protocol_era`] applies.
    #[must_use]
    pub fn new(batches: Vec<Vec<ToolCallResult>>) -> Self {
        Self {
            batches,
            cursor: AtomicUsize::new(0),
            recorded_era: Era::V1,
            live_era: None,
        }
    }

    /// Build a replay invoker from the tool batches recorded in `trace`.
    ///
    /// The trace's `negotiated_version` is classified with
    /// [`pmcp::types::protocol::protocol_era`]; a `None` version (every pre-117
    /// trace) classifies conservatively as [`Era::V1`].
    #[must_use]
    pub fn from_trace(trace: &EffectTrace) -> Self {
        let recorded_era = trace
            .negotiated_version
            .as_deref()
            .map_or(Era::V1, protocol_era);
        Self {
            recorded_era,
            ..Self::new(trace.tool_batches.clone())
        }
    }

    /// Declare the protocol era this replay run is executing under.
    ///
    /// # The default is a POLICY, not a convenience
    ///
    /// A replay constructed WITHOUT this call performs no era check at all: an
    /// undeclared live era means "no era claim was made, so no mismatch can be
    /// detected", and it is deliberately NOT treated as matching. That preserves
    /// every pre-117 caller byte-for-byte. Any caller that cares about replay
    /// determinism is expected to declare the live era.
    ///
    /// # The LEGACY-TRACE policy
    ///
    /// A trace with `negotiated_version == None` (every pre-117 fixture)
    /// classifies via the unknown-to-`V1` conservative fallback, so replaying it
    /// under an explicitly declared [`Era::V2`] live era IS a mismatch and DOES
    /// fail. That is deliberate: a v1-recorded trace replayed as v2 is exactly
    /// the hole D-08 exists to close, and a legacy trace is a v1 trace. Do not
    /// "fix" this into silence.
    #[must_use]
    pub fn with_live_era(mut self, era: Era) -> Self {
        self.live_era = Some(era);
        self
    }

    /// The era this invoker's trace was recorded under.
    #[must_use]
    pub fn recorded_era(&self) -> Era {
        self.recorded_era
    }

    /// The declared live era, when it disagrees with the recorded one.
    ///
    /// `None` whenever no live era was declared — an absent claim is not a
    /// match, it is simply not a check.
    fn mismatched_live_era(&self) -> Option<Era> {
        self.live_era.filter(|live| *live != self.recorded_era)
    }

    /// The deterministic result an era mismatch produces for a batch.
    ///
    /// # Three constraints, and why each one
    ///
    /// - **Fail fast** — the first batch carries the error, so a mismatched
    ///   replay cannot quietly consume recorded effects from the wrong era.
    /// - **Fail DETERMINISTICALLY** — `tests/replay_safety.rs` (AGNT-03) asserts
    ///   two runs over one trace produce EQUAL `DecisionTrace`s, so the failure
    ///   shape must depend only on the batch index and the two eras. It is
    ///   modelled on [`ReplaySource`]'s exhaustion path for exactly that reason.
    /// - **Do NOT panic and do NOT silently proceed** — a panic breaks the
    ///   property harness, and silence is the hole D-08 exists to close.
    fn mismatch_batch(
        &self,
        index: usize,
        calls: &[ToolCall],
        live_era: Era,
    ) -> Vec<ToolCallResult> {
        if index > 0 {
            return Vec::new();
        }
        // ONE result per call, not one per BATCH. `invoke_batch`'s contract is
        // positional — the engine pairs the Nth result with the Nth call — so a
        // single error for an N-call batch leaves N-1 calls unanswered and
        // mis-associates the one answer it does give. An empty batch still
        // yields one unattributed error rather than nothing, so a mismatch is
        // never silent.
        if calls.is_empty() {
            return vec![ToolCallResult::error("", self.mismatch_message(live_era))];
        }
        calls
            .iter()
            .map(|call| ToolCallResult::error(call.id.as_str(), self.mismatch_message(live_era)))
            .collect()
    }

    /// The single wording of the era-mismatch failure, shared by every result
    /// [`Self::mismatch_batch`] and [`ToolInvoker::invoke`] produce.
    fn mismatch_message(&self, live_era: Era) -> String {
        format!(
            "replay era mismatch: trace recorded under era {:?} but replayed under live era \
             {live_era:?}",
            self.recorded_era
        )
    }
}

#[async_trait]
impl ToolInvoker for ReplayInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        // The engine always dispatches via invoke_batch, but `ReplayInvoker` is
        // PUBLIC, so this path is reachable by any caller — and D-08's whole
        // point is that a cross-era replay must never proceed silently. The
        // guard is therefore applied on BOTH entry points, not only the one the
        // in-repo engine happens to take.
        if let Some(live_era) = self.mismatched_live_era() {
            return ToolCallResult::error(call.id.as_str(), self.mismatch_message(live_era));
        }
        // Otherwise the single-call path echoes an empty, non-error result.
        ToolCallResult::ok(call.id, serde_json::Value::Null)
    }

    async fn invoke_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolCallResult> {
        let index = self.cursor.fetch_add(1, Ordering::SeqCst);
        if let Some(live_era) = self.mismatched_live_era() {
            return self.mismatch_batch(index, &calls, live_era);
        }
        self.batches.get(index).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{DecisionStep, DecisionTrace, EffectTrace, OutcomeTag, ReplaySource};
    use crate::seams::{CompletionSource, RetryClass};
    use pmcp::types::content::Role;
    use pmcp::types::protocol::{protocol_era, Era, PROTOCOL_VERSION_2026_07_28};
    use pmcp::types::sampling::{
        CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
    };

    fn end_turn_completion() -> CreateMessageResultWithTools {
        CreateMessageResultWithTools::new(
            "m",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "done".into(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn")
    }

    #[test]
    fn effect_trace_round_trips_camel_case() {
        let trace = EffectTrace::new(vec![end_turn_completion()], vec![]);
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("completions"));
        let back: EffectTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(back.completions.len(), 1);
        assert_eq!(back.negotiated_version, None);
    }

    /// A version-less trace must serialize with NO `negotiatedVersion` key at
    /// all — not merely with the key set to `null`.
    ///
    /// Omitting the key entirely — rather than emitting `null` — is what makes
    /// the field additive: a `None`-version trace is byte-identical to a pre-117
    /// trace of the same content, so no already-recorded fixture or stored trace
    /// changes shape.
    #[test]
    fn a_version_less_trace_omits_the_key_entirely() {
        let trace = EffectTrace::new(vec![end_turn_completion()], vec![]);
        let json = serde_json::to_string(&trace).unwrap();
        assert!(
            !json.contains("negotiatedVersion"),
            "a None-version trace must omit the key, not emit null; got {json}"
        );
        assert!(
            !json.contains("null"),
            "no null placeholder may appear: {json}"
        );

        // And the SAME trace with a version attached does carry the key.
        let versioned = EffectTrace::new(vec![end_turn_completion()], vec![])
            .with_negotiated_version(PROTOCOL_VERSION_2026_07_28);
        let versioned_json = serde_json::to_string(&versioned).unwrap();
        assert!(versioned_json.contains("negotiatedVersion"));
        let back: EffectTrace = serde_json::from_str(&versioned_json).unwrap();
        assert_eq!(
            back.negotiated_version.as_deref(),
            Some(PROTOCOL_VERSION_2026_07_28)
        );
        assert_eq!(
            back.negotiated_version.as_deref().map(protocol_era),
            Some(Era::V2)
        );
    }

    /// Both pre-117 golden fixtures are ERA-LESS on disk, and they must keep
    /// deserializing untouched. Their shape IS the backward-compatibility
    /// evidence, which is why they are never regenerated.
    #[test]
    fn pre_117_golden_fixtures_deserialize_with_no_recorded_version() {
        for (name, raw) in [
            (
                "golden_trace_end_turn.json",
                include_str!("../tests/fixtures/golden_trace_end_turn.json"),
            ),
            (
                "golden_trace_tool_loop.json",
                include_str!("../tests/fixtures/golden_trace_tool_loop.json"),
            ),
        ] {
            assert!(
                !raw.contains("negotiatedVersion"),
                "{name} must stay era-less on disk"
            );
            let trace: EffectTrace = serde_json::from_str(raw)
                .unwrap_or_else(|err| panic!("{name} must still deserialize: {err}"));
            assert_eq!(
                trace.negotiated_version, None,
                "{name} must classify as recorded before era tracking existed"
            );
            // The conservative unknown-to-V1 fallback is what makes an absent
            // version safe rather than ambiguous.
            assert_eq!(
                trace
                    .negotiated_version
                    .as_deref()
                    .map_or(Era::V1, protocol_era),
                Era::V1
            );
        }
    }

    #[test]
    fn decision_trace_is_eq_for_replay() {
        let a = DecisionTrace {
            steps: vec![DecisionStep {
                iteration: 0,
                is_end_turn: true,
                is_final: true,
                tool_call_ids: vec![],
                limit: None,
            }],
            outcome: Some(OutcomeTag::Completed),
        };
        let b = a.clone();
        assert_eq!(a, b);

        let different = DecisionTrace {
            outcome: Some(OutcomeTag::RetryRequired {
                class: RetryClass::Fatal,
            }),
            ..a.clone()
        };
        assert_ne!(a, different);
    }

    #[tokio::test]
    async fn replay_source_returns_recorded_then_exhausts() {
        let src = ReplaySource::new(vec![end_turn_completion()]);
        let first = src
            .create_message(CreateMessageParams::new(vec![]))
            .await
            .unwrap();
        assert_eq!(first.stop_reason.as_deref(), Some("end_turn"));
        // Second call exhausts the trace deterministically.
        assert!(src
            .create_message(CreateMessageParams::new(vec![]))
            .await
            .is_err());
    }
}
