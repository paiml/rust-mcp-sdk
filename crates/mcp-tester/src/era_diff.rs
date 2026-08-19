//! The expected-difference baseline between MCP 2025-11-25 (v1) and
//! MCP 2026-07-28 (v2): its data model, its reader, and nothing else.
//!
//! # What this module is for
//!
//! v2 legitimately differs from v1 BY DESIGN — no `initialize`, no `tasks/list`,
//! `resultType` added, caching hints REQUIRED rather than optional — so a naive
//! dual-run diff of two `TestReport`s is pure noise. Encoding the KNOWN deltas
//! is what turns "the two runs differ" into "the two runs differ IN A WAY WE DID
//! NOT EXPECT", which is the only interesting signal. The encoding lives in
//! `crates/mcp-tester/baselines/era-deltas.yaml`, is reviewable as a spec
//! artifact by someone who does not read Rust, and is a direct input to
//! Phase 118's conformance work.
//!
//! # Why a NEW TOP-LEVEL MODULE (the A-D11 rule)
//!
//! This module follows the precedent set by [`crate::post_deploy_report`], whose
//! own header argues the case verbatim ("Why a new struct (vs. extending
//! `TestReport`)") and states the additivity rule: additive field changes on a
//! NEW type do not disturb existing consumers.
//!
//! Here that rule is not a style preference, it is a build constraint, and it is
//! ABSOLUTE. `cargo-pmcp` links `mcp-tester` as a LIBRARY, not as a JSON
//! producer:
//!
//! * `cargo-pmcp/src/commands/test/apps.rs:874-880` builds a
//!   [`crate::TestResult`] as an EXHAUSTIVE POSITIONAL STRUCT LITERAL, so a new
//!   field on `TestResult` is a hard compile break;
//! * `cargo-pmcp/src/commands/test/conformance.rs:276-289` matches
//!   [`crate::TestCategory`] with NO `_` arm, so a new variant is a hard compile
//!   break;
//! * `ServerTester::new` has five call sites in `cargo-pmcp`, so widening its
//!   arity is a hard compile break.
//!
//! Therefore NOTHING in this module adds a field to `TestResult`, a variant to
//! `TestCategory` or `TestStatus`, or a positional argument to
//! `ServerTester::new`. It defines its own types and reads its own file.
//!
//! # Scope
//!
//! The baseline data model and its reader, PLUS (added by plan 117-11) the
//! dual-run comparison and the [`DualRunReport`] that carries its verdict. The
//! comparison joins observed [`crate::era_observations::ObservationId`]s against
//! [`EraDelta::observation_id`] — never against a `TestResult` display name,
//! which could not observe most of the baseline at all.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::era_observations::{EraObservations, ObservedValue};
use crate::report::{TestReport, TestResult, TestStatus};

/// One expected v1-vs-v2 difference: a difference that is CORRECT BY DESIGN.
///
/// Field semantics are documented in the baseline file's own header, which is
/// the reviewer-facing copy of this contract.
///
/// `note` and `provisional` carry `#[serde(default)]` so the schema stays
/// forward-compatible: a future optional field can be added without invalidating
/// every checked-in baseline, exactly as
/// [`crate::post_deploy_report::PostDeployReport`] does for its optional fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraDelta {
    /// Stable human-facing label, `ERA-NN`. Unique across the baseline.
    pub id: String,

    /// The STABLE, MACHINE-FACING name of the wire fact this entry is about —
    /// namespaced, lowercase, dot-separated (`method.initialize`,
    /// `header.mcp_session_id`, `result.cache_scope`, …).
    ///
    /// This is the JOIN KEY a dual-run comparison diffs on. It is REQUIRED (no
    /// `serde(default)`) and must be unique: a missing or duplicated value
    /// silently merges two distinct wire facts. It is NOT a human-facing test
    /// name and must never be renamed for readability.
    ///
    /// It exists because [`crate::TestResult`] carries only
    /// `{name, category, status, duration, error, details}` — no header, no
    /// session id, no result-envelope key, no HTTP status — so a comparison
    /// keyed on test names could not observe most of the baseline's entries.
    pub observation_id: String,

    /// Human-readable wire surface the entry concerns.
    pub subject: String,

    /// What MCP 2025-11-25 does.
    pub v1: String,

    /// What MCP 2026-07-28 does.
    pub v2: String,

    /// Difference class, for grouping in a report.
    pub kind: String,

    /// Citation a reviewer can check without reading Rust.
    pub source: String,

    /// Optional prose. Required in practice on provisional entries, where it
    /// names the phase that owns the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    /// `true` when the owning phase is not signed off, so a change there
    /// produces a legible baseline edit rather than a mystery test failure.
    #[serde(default)]
    pub provisional: bool,
}

/// The whole checked-in baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraBaseline {
    /// Wire-format version of THIS file's schema. Currently `1`.
    pub schema_version: u32,

    /// The v1 protocol version the baseline was written against.
    pub v1_protocol: String,

    /// The v2 protocol version the baseline was written against.
    pub v2_protocol: String,

    /// The expected differences.
    pub deltas: Vec<EraDelta>,
}

impl EraBaseline {
    /// An empty baseline — classifies NOTHING.
    ///
    /// Exists so the "baseline could not be parsed" path in
    /// `ConformanceRunner::run_dual` does not have to spell out an exhaustive
    /// struct literal, which every new field would otherwise break.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: 0,
            v1_protocol: String::new(),
            v2_protocol: String::new(),
            deltas: Vec::new(),
        }
    }

    /// Look an entry up by its stable [`EraDelta::observation_id`].
    pub fn find_by_observation_id(&self, observation_id: &str) -> Option<&EraDelta> {
        self.deltas
            .iter()
            .find(|d| d.observation_id == observation_id)
    }

    /// Every [`EraDelta::observation_id`] in file order.
    pub fn observation_ids(&self) -> Vec<&str> {
        self.deltas
            .iter()
            .map(|d| d.observation_id.as_str())
            .collect()
    }
}

/// Path of the baseline shipped with this crate.
///
/// Derived from `CARGO_MANIFEST_DIR` so no absolute path is ever baked in and
/// the file resolves the same from a test, from the binary and from a fuzz
/// target.
pub fn default_baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("baselines")
        .join("era-deltas.yaml")
}

/// Parse a baseline from text. The PURE seam — no file I/O, no environment.
///
/// This is what the fuzz target drives, so it MUST NOT PANIC on any input:
/// every rejection below is an `Err`.
///
/// # Rejections (the parser's contract)
///
/// A successfully returned [`EraBaseline`] is GUARANTEED to satisfy all four of
/// the following, because each is rejected here:
///
/// 1. the text is not valid YAML for the schema — `Err`;
/// 2. some delta's `id` is empty after trimming — `Err`;
/// 3. some delta's `observation_id` is empty after trimming — `Err`;
/// 4. two deltas share an `id`, or two deltas share an `observation_id` — `Err`.
///
/// Validation lives HERE rather than only in a test so that "a parsed baseline
/// has non-empty unique ids" is a PARSER CONTRACT. A downstream consumer that
/// keys on [`EraDelta::observation_id`] would otherwise silently merge two wire
/// facts, and a fuzz target asserting the property would crash on well-formed
/// input the parser had legitimately accepted.
///
/// Deliberately NOT rejected here: the lexical SHAPE of an `observation_id`
/// (lowercase, dot-separated), the length of a `source`, and whether a
/// provisional entry names its owning phase. Those are baseline-content rules,
/// gated by `crates/mcp-tester/tests/era_baseline.rs` against the checked-in
/// file — not properties of arbitrary input.
///
/// # Errors
///
/// Returns an error for each of the four cases above.
pub fn parse_baseline(text: &str) -> Result<EraBaseline> {
    let baseline: EraBaseline =
        serde_yaml::from_str(text).context("Failed to parse era-delta baseline YAML")?;

    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_observation_ids: HashSet<&str> = HashSet::new();

    for delta in &baseline.deltas {
        if delta.id.trim().is_empty() {
            bail!("era-delta baseline: an entry has an empty `id`");
        }
        if delta.observation_id.trim().is_empty() {
            bail!(
                "era-delta baseline: entry `{}` has an empty `observation_id`",
                delta.id
            );
        }
        if !seen_ids.insert(delta.id.as_str()) {
            bail!("era-delta baseline: duplicate `id` `{}`", delta.id);
        }
        if !seen_observation_ids.insert(delta.observation_id.as_str()) {
            bail!(
                "era-delta baseline: duplicate `observation_id` `{}` (on entry `{}`)",
                delta.observation_id,
                delta.id
            );
        }
    }

    Ok(baseline)
}

/// Read and parse a baseline from disk.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or for any reason
/// [`parse_baseline`] rejects its contents.
pub fn load_baseline<P: AsRef<Path>>(path: P) -> Result<EraBaseline> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .with_context(|| format!("Failed to read era-delta baseline: {}", path.display()))?;
    parse_baseline(&text)
        .with_context(|| format!("Failed to parse era-delta baseline: {}", path.display()))
}

/// The baseline text, COMPILED IN rather than read from disk at runtime.
///
/// `default_baseline_path()` resolves through `CARGO_MANIFEST_DIR`, which for a
/// `cargo install`ed binary points into `~/.cargo/registry/src/…/mcp-tester-*`
/// — a cache directory that `cargo cache` and manual cleanup delete. Reading it
/// at runtime therefore works in every in-repo test and fuzz run (which execute
/// from the source tree) while silently failing for end users, so the failure
/// mode is structurally untestable in CI. Embedding the bytes removes it.
const DEFAULT_BASELINE_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/baselines/era-deltas.yaml"
));

/// Parse the baseline shipped with this crate.
///
/// The text is embedded at compile time, so this does no file I/O and cannot
/// fail for a missing or unreadable file. [`default_baseline_path`] remains
/// available for callers that deliberately want the on-disk copy (a reviewer
/// diffing it, or an explicit `--baseline` override).
///
/// # Errors
///
/// Only for the reasons [`parse_baseline`] rejects its contents — which, for
/// the checked-in file, is gated by `crates/mcp-tester/tests/era_baseline.rs`.
pub fn load_default_baseline() -> Result<EraBaseline> {
    parse_baseline(DEFAULT_BASELINE_TEXT)
        .context("Failed to parse the compiled-in era-delta baseline")
}

// ===========================================================================
// The dual-run comparison (Phase 117, D-06).
// ===========================================================================

/// How one observation-id's v1-vs-v2 behaviour relates to the baseline.
///
/// [`Self::Unexpected`] and [`Self::Missing`] are BOTH findings, reported as
/// DISTINCT categories because they mean opposite things and call for opposite
/// remedies: an unexpected difference means the implementation moved, a missing
/// one means either the spec moved or a documented behaviour regressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DifferenceClass {
    /// The eras differed, and they differed exactly as the baseline records.
    /// CORRECT BY DESIGN.
    Expected,
    /// The eras differed with no baseline delta recording that difference (or
    /// recording it with different values). A FINDING.
    Unexpected,
    /// A baseline delta whose observation did NOT differ across the eras, or
    /// was never observed at all. Also a FINDING.
    Missing,
}

impl DifferenceClass {
    /// Whether this class is a finding a reader must act on.
    pub fn is_finding(self) -> bool {
        !matches!(self, Self::Expected)
    }

    /// Stable label for the rendered report.
    pub fn label(self) -> &'static str {
        match self {
            Self::Expected => "EXPECTED",
            Self::Unexpected => "UNEXPECTED",
            Self::Missing => "MISSING",
        }
    }
}

/// One classified observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifiedDifference {
    /// The stable wire-fact id this row is about.
    pub observation_id: String,
    /// How it relates to the baseline.
    pub class: DifferenceClass,
    /// What the v1 run observed, if it observed anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v1: Option<ObservedValue>,
    /// What the v2 run observed, if it observed anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v2: Option<ObservedValue>,
    /// The matching baseline entry's `ERA-NN` label, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_id: Option<String>,
    /// Whether the matching baseline entry is PROVISIONAL.
    ///
    /// A provisional delta that goes MISSING is a legible consequence of
    /// Phase-114/115 churn in an unsigned-off area; a non-provisional one going
    /// MISSING is a regression. Rendering them identically would make the first
    /// look alarming and the second look routine.
    #[serde(default)]
    pub provisional: bool,
    /// Human-readable explanation of this row's classification.
    pub detail: String,
}

/// The dual-run verdict: two suite reports plus the baseline-keyed comparison.
///
/// # Why a NEW TOP-LEVEL STRUCT (the A-D11 rule)
///
/// The same rule this module's header states, applied to the comparison rather
/// than to the baseline. `cargo-pmcp` links `mcp-tester` as a LIBRARY: it builds
/// [`crate::TestResult`] as an exhaustive positional struct literal and matches
/// [`crate::TestCategory`] with no `_` arm, so hanging the comparison off either
/// type is a hard workspace compile break. `post_deploy_report` is the in-repo
/// precedent for exactly this escape hatch, and its header argues the case
/// verbatim ("Why a new struct (vs. extending `TestReport`)").
///
/// So this struct carries the two `TestReport`s WHOLE — the human-facing suite
/// results, unmodified — alongside the two [`EraObservations`] the comparison
/// actually reads. Nothing is added to `TestResult`.
///
/// `schema_version` is the wire-format guard, and every optional field carries
/// `#[serde(default, skip_serializing_if = "Option::is_none")]` so a future
/// additive field does not invalidate a stored report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualRunReport {
    /// Wire-format version of this struct. Currently `1`.
    pub schema_version: u32,
    /// Which eras the target was detected to serve (`dual`, `v1-only`, …).
    pub era_support: String,
    /// The v1 suite result, verbatim.
    pub v1_report: TestReport,
    /// The v2 suite result, verbatim.
    pub v2_report: TestReport,
    /// What the v1 probes saw.
    pub v1_observations: EraObservations,
    /// What the v2 probes saw.
    pub v2_observations: EraObservations,
    /// Every classified observation, in stable id order.
    pub differences: Vec<ClassifiedDifference>,
    /// Set when the comparison itself looks untrustworthy — see
    /// [`compare_eras`]'s anti-vacuity guard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspicion: Option<String>,
    /// Free-form note, e.g. why a dual run degraded to a single run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl DualRunReport {
    /// Rows that are findings (UNEXPECTED or MISSING).
    pub fn findings(&self) -> Vec<&ClassifiedDifference> {
        self.differences
            .iter()
            .filter(|d| d.class.is_finding())
            .collect()
    }

    /// Count of rows in one class.
    pub fn count(&self, class: DifferenceClass) -> usize {
        self.differences.iter().filter(|d| d.class == class).count()
    }

    /// Render the comparison into any writer.
    ///
    /// Follows `report.rs:238-255`'s `print_to_writer` shape so a test can
    /// capture into a `Vec<u8>` and assert on BYTES. Printed only when
    /// `--dual-run` was requested; nothing here is on the single-run path.
    ///
    /// # Errors
    ///
    /// Propagates the writer's I/O errors.
    pub fn print_to_writer<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w)?;
        writeln!(w, "DUAL-RUN ERA COMPARISON")?;
        writeln!(w, "{}", "=".repeat(60))?;
        writeln!(w, "Era support : {}", self.era_support)?;
        writeln!(
            w,
            "v1 suite    : {} tests, {} failed",
            self.v1_report.summary.total, self.v1_report.summary.failed
        )?;
        writeln!(
            w,
            "v2 suite    : {} tests, {} failed",
            self.v2_report.summary.total, self.v2_report.summary.failed
        )?;
        writeln!(
            w,
            "Differences : {} expected, {} unexpected, {} missing",
            self.count(DifferenceClass::Expected),
            self.count(DifferenceClass::Unexpected),
            self.count(DifferenceClass::Missing),
        )?;
        if let Some(note) = &self.note {
            writeln!(w, "Note        : {note}")?;
        }
        writeln!(w)?;

        // The v2 suite's FAILING tests, named.
        //
        // Only v2 is rendered here, and that asymmetry is deliberate: the v1
        // report is the one this command RETURNS, so `handle_command_result`
        // already prints it in full as `TEST RESULTS`. The v2 report has no
        // such outlet — before this block it existed in `DualRunReport` and was
        // never shown, so `v2 suite : N tests, 1 failed` was the entire signal a
        // user got. A count without an identity cannot be acted on; a real
        // deployed server is what exposed it.
        let v2_failures: Vec<&TestResult> = self
            .v2_report
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Failed)
            .collect();
        if !v2_failures.is_empty() {
            writeln!(w, "V2 SUITE FAILURES ({})", v2_failures.len())?;
            for test in v2_failures {
                writeln!(w, "  {} [{:?}]", test.name, test.category)?;
                if let Some(error) = &test.error {
                    writeln!(w, "      {error}")?;
                }
            }
            writeln!(w)?;
        }

        for class in [
            DifferenceClass::Unexpected,
            DifferenceClass::Missing,
            DifferenceClass::Expected,
        ] {
            let rows: Vec<&ClassifiedDifference> = self
                .differences
                .iter()
                .filter(|d| d.class == class)
                .collect();
            if rows.is_empty() {
                continue;
            }
            writeln!(w, "{} ({})", class.label(), rows.len())?;
            for row in rows {
                let provisional = if row.provisional && class == DifferenceClass::Missing {
                    " [PROVISIONAL]"
                } else if row.provisional {
                    " [provisional]"
                } else {
                    ""
                };
                writeln!(
                    w,
                    "  {}{}{}",
                    row.observation_id,
                    row.delta_id
                        .as_ref()
                        .map(|d| format!(" ({d})"))
                        .unwrap_or_default(),
                    provisional
                )?;
                writeln!(w, "      {}", row.detail)?;
            }
            writeln!(w)?;
        }

        if let Some(suspicion) = &self.suspicion {
            writeln!(w, "SUSPICIOUS: {suspicion}")?;
            writeln!(w)?;
        }
        Ok(())
    }
}

/// Classify every observation against `baseline`, joining on
/// [`crate::era_observations::ObservationId`] — never on a display name.
///
/// # The join rule
///
/// For each id in the union of (v1 observations ∪ v2 observations ∪ baseline
/// ids):
///
/// * the eras DIFFERED when both runs established a value and the two tokens
///   are unequal (an unestablished value on either side is "we could not tell",
///   which is not a difference);
/// * DIFFERED + a delta whose recorded `v1:`/`v2:` columns EQUAL the observed
///   tokens ⇒ [`DifferenceClass::Expected`];
/// * DIFFERED + no such delta ⇒ [`DifferenceClass::Unexpected`] (including the
///   case where a delta exists but records different values — the difference is
///   real and is not the documented one);
/// * NOT DIFFERED + a delta exists ⇒ [`DifferenceClass::Missing`];
/// * NOT DIFFERED + no delta ⇒ not reported at all.
///
/// # The anti-vacuity guard
///
/// Two independent era runs against a DUAL-era server MUST differ: the baseline
/// lists fourteen differences that are correct by design. So an empty
/// difference list, with two non-empty suite reports, does not mean "all
/// clear" — it means the comparison probably did not run, or every probe came
/// back `Unavailable`. That is surfaced in
/// [`DualRunReport::suspicion`] rather than rendered as a clean bill of health,
/// because a silently-never-matching comparison would classify everything as
/// agreement and be indistinguishable from success.
pub fn compare_eras(
    v1: &EraObservations,
    v2: &EraObservations,
    baseline: &EraBaseline,
) -> (Vec<ClassifiedDifference>, Option<String>) {
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for id in v1.ids() {
        ids.insert(id.as_str());
    }
    for id in v2.ids() {
        ids.insert(id.as_str());
    }
    for id in baseline.observation_ids() {
        ids.insert(id);
    }

    let mut differences = Vec::new();
    for id in ids {
        // `EraObservations::get` is the newtype's own lookup; reaching past it
        // into the inner map hand-rolled a linear scan AND left `get` dead in
        // production code, so a reader could not tell the scan was incidental.
        // An observation can only ever be keyed by a registry id, so a
        // non-registry `id` correctly yields `None` from both sides.
        let key = crate::era_observations::ObservationId::from_registry(id);
        let observed_v1 = key.and_then(|k| v1.get(k));
        let observed_v2 = key.and_then(|k| v2.get(k));
        let delta = baseline.find_by_observation_id(id);

        let differed = match (observed_v1, observed_v2) {
            (Some(a), Some(b)) => {
                a.is_established() && b.is_established() && a.token() != b.token()
            },
            _ => false,
        };
        let agrees_with_delta = match (delta, observed_v1, observed_v2) {
            (Some(d), Some(a), Some(b)) => a.token() == d.v1 && b.token() == d.v2,
            _ => false,
        };

        let (class, detail) = match (differed, delta, agrees_with_delta) {
            (true, Some(d), true) => (
                DifferenceClass::Expected,
                format!(
                    "{}: v1 `{}` vs v2 `{}` — correct by design ({})",
                    d.subject, d.v1, d.v2, d.source
                ),
            ),
            (true, Some(d), false) => (
                DifferenceClass::Unexpected,
                format!(
                    "the eras differ, but NOT as {} records: observed v1 `{}` / v2 `{}`, \
                     baseline records v1 `{}` / v2 `{}`",
                    d.id,
                    token_of(observed_v1),
                    token_of(observed_v2),
                    d.v1,
                    d.v2
                ),
            ),
            (true, None, _) => (
                DifferenceClass::Unexpected,
                format!(
                    "the eras differ with NO baseline entry: observed v1 `{}` / v2 `{}`",
                    token_of(observed_v1),
                    token_of(observed_v2)
                ),
            ),
            (false, Some(d), _) => (
                DifferenceClass::Missing,
                format!(
                    "{} no longer reproduces: baseline records v1 `{}` / v2 `{}`, \
                     observed v1 `{}` / v2 `{}`",
                    d.id,
                    d.v1,
                    d.v2,
                    token_of(observed_v1),
                    token_of(observed_v2)
                ),
            ),
            (false, None, _) => continue,
        };

        differences.push(ClassifiedDifference {
            observation_id: id.to_string(),
            class,
            v1: observed_v1.cloned(),
            v2: observed_v2.cloned(),
            delta_id: delta.map(|d| d.id.clone()),
            provisional: delta.is_some_and(|d| d.provisional),
            detail,
        });
    }

    let suspicion = if differences.is_empty() {
        Some(format!(
            "the classified difference list is EMPTY, yet the baseline records {} \
             differences that are correct by design. Two independent era runs \
             against a dual-era server MUST differ, so this is far more likely to \
             mean the comparison did not run (or every probe returned \
             `unavailable`) than that the eras agree.",
            baseline.deltas.len()
        ))
    } else {
        None
    };

    (differences, suspicion)
}

/// Render an optional observation as its token, or `not observed`.
fn token_of(value: Option<&ObservedValue>) -> String {
    value.map_or_else(|| "not observed".to_string(), ObservedValue::token)
}

/// Build a [`DualRunReport`] from two suite reports and two observation sets.
///
/// The anti-vacuity guard is applied here too: an empty difference list is only
/// SUSPICIOUS when both suites actually ran tests. A comparison over two empty
/// reports is merely empty.
pub fn build_dual_run_report(
    era_support: &str,
    v1_report: TestReport,
    v2_report: TestReport,
    v1_observations: EraObservations,
    v2_observations: EraObservations,
    baseline: &EraBaseline,
) -> DualRunReport {
    let (differences, mut suspicion) = compare_eras(&v1_observations, &v2_observations, baseline);
    if v1_report.summary.total == 0 || v2_report.summary.total == 0 {
        suspicion = None;
    }
    DualRunReport {
        schema_version: 1,
        era_support: era_support.to_string(),
        v1_report,
        v2_report,
        v1_observations,
        v2_observations,
        differences,
        suspicion,
        note: None,
    }
}

#[cfg(test)]
mod comparison {
    use super::*;
    use crate::era_observations::{
        EraObservations, ObservationId, ObservedValue, METHOD_INITIALIZE, RESULT_CACHE_SCOPE,
    };
    use std::collections::BTreeMap;

    fn baseline_with(entries: &[(&str, &str, &str, bool)]) -> EraBaseline {
        EraBaseline {
            schema_version: 1,
            v1_protocol: "2025-11-25".to_string(),
            v2_protocol: "2026-07-28".to_string(),
            deltas: entries
                .iter()
                .enumerate()
                .map(|(i, (observation_id, v1, v2, provisional))| EraDelta {
                    id: format!("ERA-{:02}", i + 1),
                    observation_id: (*observation_id).to_string(),
                    subject: format!("subject for {observation_id}"),
                    v1: (*v1).to_string(),
                    v2: (*v2).to_string(),
                    kind: "test".to_string(),
                    source: "unit test".to_string(),
                    note: None,
                    provisional: *provisional,
                })
                .collect(),
        }
    }

    fn observations(entries: &[(ObservationId, ObservedValue)]) -> EraObservations {
        let mut map = BTreeMap::new();
        for (id, value) in entries {
            map.insert(*id, value.clone());
        }
        EraObservations(map)
    }

    /// THE JOIN RULE, exercised directly with a matching AND a non-matching
    /// case. A rule that silently never matched would classify every delta
    /// EXPECTED (or every one UNEXPECTED) and could not fail either half of
    /// this test.
    #[test]
    fn join_rule_matches_a_recorded_delta_and_rejects_a_mismatched_one() {
        let baseline = baseline_with(&[
            ("method.initialize", "served", "absent", false),
            ("result.cache_scope", "absent", "required", false),
        ]);

        // MATCHING: observed values equal the recorded columns.
        let (matched, _) = compare_eras(
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("absent".into()))]),
            &baseline,
        );
        let initialize = matched
            .iter()
            .find(|d| d.observation_id == "method.initialize")
            .expect("the observed id must be classified");
        assert_eq!(
            initialize.class,
            DifferenceClass::Expected,
            "a difference recorded in the baseline is correct by design"
        );
        assert_eq!(initialize.delta_id.as_deref(), Some("ERA-01"));

        // NON-MATCHING: the eras differ, but not in the recorded way.
        let (mismatched, _) = compare_eras(
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &observations(&[(
                METHOD_INITIALIZE,
                ObservedValue::Text("served-but-deprecated".into()),
            )]),
            &baseline,
        );
        let initialize = mismatched
            .iter()
            .find(|d| d.observation_id == "method.initialize")
            .expect("the observed id must be classified");
        assert_eq!(
            initialize.class,
            DifferenceClass::Unexpected,
            "a difference that is not the DOCUMENTED difference is a finding"
        );
        assert!(initialize.detail.contains("baseline records"));
    }

    #[test]
    fn an_unrecorded_difference_is_unexpected() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        let (differences, _) = compare_eras(
            &observations(&[(RESULT_CACHE_SCOPE, ObservedValue::Absent)]),
            &observations(&[(RESULT_CACHE_SCOPE, ObservedValue::Text("required".into()))]),
            &baseline,
        );
        let row = differences
            .iter()
            .find(|d| d.observation_id == "result.cache_scope")
            .expect("an unbaselined difference must still be reported");
        assert_eq!(row.class, DifferenceClass::Unexpected);
        assert!(row.delta_id.is_none());
        assert!(row.detail.contains("NO baseline entry"));
    }

    #[test]
    fn a_delta_that_no_longer_reproduces_is_missing() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        let (differences, _) = compare_eras(
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &baseline,
        );
        let row = &differences[0];
        assert_eq!(row.class, DifferenceClass::Missing);
        assert!(row.detail.contains("no longer reproduces"));
    }

    #[test]
    fn a_delta_never_observed_is_missing_not_silently_dropped() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        let (differences, _) = compare_eras(
            &EraObservations::default(),
            &EraObservations::default(),
            &baseline,
        );
        assert_eq!(differences.len(), 1);
        assert_eq!(differences[0].class, DifferenceClass::Missing);
        assert!(differences[0].detail.contains("not observed"));
    }

    /// An `Unavailable` observation is "we could not tell", NOT a difference —
    /// otherwise a flaky probe would manufacture findings.
    #[test]
    fn an_unavailable_observation_is_not_a_difference() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        let (differences, _) = compare_eras(
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &observations(&[(
                METHOD_INITIALIZE,
                ObservedValue::Unavailable("transport failed".into()),
            )]),
            &baseline,
        );
        assert_eq!(differences[0].class, DifferenceClass::Missing);
    }

    /// A provisional delta going MISSING must be legible as Phase-114/115 churn
    /// rather than as an alarming regression.
    #[test]
    fn provisional_missing_is_reported_distinctly_from_non_provisional() {
        let baseline = baseline_with(&[
            ("method.initialize", "served", "absent", false),
            ("result.cache_scope", "absent", "required", true),
        ]);
        let same = |id| observations(&[(id, ObservedValue::Text("same".into()))]);
        let mut v1 = same(METHOD_INITIALIZE).0;
        v1.extend(same(RESULT_CACHE_SCOPE).0);
        let observations_both = EraObservations(v1);
        let (differences, _) = compare_eras(&observations_both, &observations_both, &baseline);

        let non_provisional = differences
            .iter()
            .find(|d| d.observation_id == "method.initialize")
            .expect("classified");
        let provisional = differences
            .iter()
            .find(|d| d.observation_id == "result.cache_scope")
            .expect("classified");
        assert_eq!(non_provisional.class, DifferenceClass::Missing);
        assert_eq!(provisional.class, DifferenceClass::Missing);
        assert!(!non_provisional.provisional);
        assert!(provisional.provisional);

        let report = DualRunReport {
            schema_version: 1,
            era_support: "dual".into(),
            v1_report: TestReport::new(),
            v2_report: TestReport::new(),
            v1_observations: EraObservations::default(),
            v2_observations: EraObservations::default(),
            differences,
            suspicion: None,
            note: None,
        };
        let mut sink = Vec::<u8>::new();
        report.print_to_writer(&mut sink).expect("render");
        let text = String::from_utf8(sink).expect("utf8");
        assert!(
            text.contains("result.cache_scope (ERA-02) [PROVISIONAL]"),
            "a provisional MISSING must be marked in the render:\n{text}"
        );
        assert!(
            text.contains("method.initialize (ERA-01)\n"),
            "a non-provisional MISSING must NOT be marked:\n{text}"
        );
    }

    /// A v2-only failure must be IDENTIFIABLE from the dual-run render.
    ///
    /// Regression fence for the gap a real deployed server exposed: the header
    /// said `v2 suite : 19 tests, 1 failed` and NOTHING anywhere named the
    /// failing test — not in `pretty`, not in `json`, not at `-v 3`. A failure
    /// count with no failure identity is the "a red that says only `assertion
    /// failed`" shape this repository polices everywhere else; for a tool whose
    /// entire job is diagnosing someone else's server it is the difference
    /// between a usable report and a dead end.
    ///
    /// The data was never missing — `DualRunReport` has always carried
    /// `v2_report` in full. Only the renderer dropped it.
    #[test]
    fn a_v2_only_failure_is_named_in_the_render() {
        use crate::report::TestCategory;
        use std::time::Duration;

        let mut v2_report = TestReport::new();
        v2_report.add_test(TestResult::failed(
            "Core: initialize handshake",
            TestCategory::Core,
            Duration::from_millis(12),
            "expected no initialize on v2, server answered one",
        ));
        v2_report.add_test(TestResult::passed(
            "Tools: list returns valid ToolInfo",
            TestCategory::Tools,
            Duration::from_millis(3),
            "Found 10 tools",
        ));

        let report = DualRunReport {
            schema_version: 1,
            era_support: "dual".into(),
            v1_report: TestReport::new(),
            v2_report,
            v1_observations: EraObservations::default(),
            v2_observations: EraObservations::default(),
            differences: Vec::new(),
            suspicion: None,
            note: None,
        };
        let mut sink = Vec::<u8>::new();
        report.print_to_writer(&mut sink).expect("render");
        let text = String::from_utf8(sink).expect("utf8");

        assert!(
            text.contains("Core: initialize handshake"),
            "the failing v2 test must be NAMED, not just counted:\n{text}"
        );
        assert!(
            text.contains("expected no initialize on v2, server answered one"),
            "the failure REASON must travel with the name:\n{text}"
        );
        assert!(
            !text.contains("Tools: list returns valid ToolInfo"),
            "only FAILURES belong in this section; a passing v2 test would bury \
             the signal:\n{text}"
        );
    }

    /// MISSING and UNEXPECTED must appear as separate, labelled sections.
    #[test]
    fn render_reports_missing_and_unexpected_as_distinct_categories() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        let (differences, _) = compare_eras(
            &observations(&[
                (METHOD_INITIALIZE, ObservedValue::Text("served".into())),
                (RESULT_CACHE_SCOPE, ObservedValue::Absent),
            ]),
            &observations(&[
                (METHOD_INITIALIZE, ObservedValue::Text("served".into())),
                (RESULT_CACHE_SCOPE, ObservedValue::Text("required".into())),
            ]),
            &baseline,
        );
        let report = DualRunReport {
            schema_version: 1,
            era_support: "dual".into(),
            v1_report: TestReport::new(),
            v2_report: TestReport::new(),
            v1_observations: EraObservations::default(),
            v2_observations: EraObservations::default(),
            differences,
            suspicion: None,
            note: None,
        };
        let mut sink = Vec::<u8>::new();
        report.print_to_writer(&mut sink).expect("render");
        let text = String::from_utf8(sink).expect("utf8");
        assert!(text.contains("UNEXPECTED (1)"), "{text}");
        assert!(text.contains("MISSING (1)"), "{text}");
        assert_eq!(report.findings().len(), 2);
    }

    /// THE ANTI-VACUITY GUARD.
    #[test]
    fn an_empty_difference_list_is_surfaced_as_suspicious() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
        // Nothing to classify at all: no observation ids, no baseline overlap.
        let empty_baseline = baseline_with(&[]);
        let (differences, suspicion) = compare_eras(
            &EraObservations::default(),
            &EraObservations::default(),
            &empty_baseline,
        );
        assert!(differences.is_empty());
        let suspicion = suspicion.expect("an empty difference list must be suspicious");
        assert!(suspicion.contains("MUST differ"), "{suspicion}");

        // …but with a real baseline the list is NOT empty, so no suspicion.
        let (differences, suspicion) = compare_eras(
            &EraObservations::default(),
            &EraObservations::default(),
            &baseline,
        );
        assert!(!differences.is_empty());
        assert!(suspicion.is_none());
    }

    /// The guard is only meaningful when both suites actually ran.
    #[test]
    fn suspicion_is_suppressed_when_a_suite_ran_no_tests() {
        let report = build_dual_run_report(
            "dual",
            TestReport::new(),
            TestReport::new(),
            EraObservations::default(),
            EraObservations::default(),
            &baseline_with(&[]),
        );
        assert!(
            report.suspicion.is_none(),
            "a comparison over two empty reports is empty, not suspicious"
        );
        assert_eq!(report.schema_version, 1);
    }

    /// Optional fields must be omitted, not serialized as `null` — the
    /// forward-compatibility shape `post_deploy_report` established.
    #[test]
    fn optional_fields_are_omitted_when_absent() {
        let report = build_dual_run_report(
            "dual",
            TestReport::new(),
            TestReport::new(),
            EraObservations::default(),
            EraObservations::default(),
            &baseline_with(&[]),
        );
        let json = serde_json::to_string(&report).expect("serializes");
        assert!(!json.contains("\"note\""), "{json}");
        assert!(json.contains("\"schema_version\":1"), "{json}");
    }

    /// Classification order is deterministic, because the report is rendered
    /// and compared as bytes.
    #[test]
    fn classification_order_is_deterministic() {
        let baseline = baseline_with(&[
            ("method.initialize", "served", "absent", false),
            ("result.cache_scope", "absent", "required", false),
        ]);
        let first = compare_eras(
            &EraObservations::default(),
            &EraObservations::default(),
            &baseline,
        )
        .0;
        let second = compare_eras(
            &EraObservations::default(),
            &EraObservations::default(),
            &baseline,
        )
        .0;
        assert_eq!(first, second);
        let ids: Vec<&str> = first.iter().map(|d| d.observation_id.as_str()).collect();
        assert_eq!(ids, vec!["method.initialize", "result.cache_scope"]);
    }

    // CLAUDE.md ALWAYS / PROPERTY testing.
    proptest::proptest! {
        /// The comparison is TOTAL: any pair of observation sets classifies
        /// without panicking, and every baseline id is always accounted for.
        #[test]
        fn compare_eras_accounts_for_every_baseline_id(
            v1_token in "[a-z]{1,8}",
            v2_token in "[a-z]{1,8}",
        ) {
            let baseline = baseline_with(&[("method.initialize", "served", "absent", false)]);
            let (differences, _) = compare_eras(
                &observations(&[(METHOD_INITIALIZE, ObservedValue::Text(v1_token.clone()))]),
                &observations(&[(METHOD_INITIALIZE, ObservedValue::Text(v2_token.clone()))]),
                &baseline,
            );
            proptest::prop_assert_eq!(differences.len(), 1);
            let expected = v1_token == "served" && v2_token == "absent";
            proptest::prop_assert_eq!(
                differences[0].class == DifferenceClass::Expected,
                expected
            );
        }

        /// The renderer never panics, whatever it is handed.
        #[test]
        fn render_never_panics(era_support in ".*", detail in ".*") {
            let report = DualRunReport {
                schema_version: 1,
                era_support,
                v1_report: TestReport::new(),
                v2_report: TestReport::new(),
                v1_observations: EraObservations::default(),
                v2_observations: EraObservations::default(),
                differences: vec![ClassifiedDifference {
                    observation_id: "x.y".to_string(),
                    class: DifferenceClass::Unexpected,
                    v1: None,
                    v2: None,
                    delta_id: None,
                    provisional: false,
                    detail,
                }],
                suspicion: None,
                note: None,
            };
            let mut sink = Vec::<u8>::new();
            proptest::prop_assert!(report.print_to_writer(&mut sink).is_ok());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal well-formed baseline, used as the mutation base for the negative
    /// cases below.
    fn valid_yaml() -> String {
        r#"
schema_version: 1
v1_protocol: "2025-11-25"
v2_protocol: "2026-07-28"
deltas:
  - id: ERA-01
    observation_id: method.initialize
    subject: "method:initialize"
    v1: served
    v2: absent
    kind: method-removed
    source: "REQUIREMENTS.md:911 (CLNT-01)"
  - id: ERA-02
    observation_id: method.server_discover
    subject: "method:server/discover"
    v1: "error:-32601"
    v2: served
    kind: method-added
    source: "src/client/mod.rs:887"
    provisional: true
    note: "Phase 112 owns this."
"#
        .to_string()
    }

    #[test]
    fn era_diff_parses_a_well_formed_baseline() {
        let baseline = parse_baseline(&valid_yaml()).expect("valid baseline must parse");
        assert_eq!(baseline.schema_version, 1);
        assert_eq!(baseline.deltas.len(), 2);
        assert!(!baseline.deltas[0].provisional, "default is false");
        assert!(baseline.deltas[0].note.is_none(), "default is None");
        assert!(baseline.deltas[1].provisional);
    }

    #[test]
    fn era_diff_rejects_an_empty_id() {
        let yaml = valid_yaml().replace("id: ERA-01", "id: \"   \"");
        let err = parse_baseline(&yaml).expect_err("an empty `id` must be rejected");
        assert!(
            err.to_string().contains("empty `id`"),
            "error must name the failure: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_an_empty_observation_id() {
        let yaml =
            valid_yaml().replace("observation_id: method.initialize", "observation_id: \"\"");
        let err = parse_baseline(&yaml).expect_err("an empty `observation_id` must be rejected");
        assert!(
            err.to_string().contains("empty `observation_id`"),
            "error must name the failure: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_a_duplicate_id() {
        let yaml = valid_yaml().replace("id: ERA-02", "id: ERA-01");
        let err = parse_baseline(&yaml).expect_err("a duplicate `id` must be rejected");
        assert!(
            err.to_string().contains("duplicate `id` `ERA-01`"),
            "error must name the duplicate: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_a_duplicate_observation_id() {
        let yaml = valid_yaml().replace(
            "observation_id: method.server_discover",
            "observation_id: method.initialize",
        );
        let err = parse_baseline(&yaml).expect_err("a duplicate `observation_id` must be rejected");
        assert!(
            err.to_string()
                .contains("duplicate `observation_id` `method.initialize`"),
            "error must name the duplicate: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_garbage_without_panicking() {
        for garbage in [
            "",
            "\u{0}\u{1}\u{2}",
            "not: a: baseline",
            "deltas: []",
            "schema_version: \"one\"",
            "[1, 2, 3]",
        ] {
            assert!(
                parse_baseline(garbage).is_err(),
                "garbage input must be an Err, not a panic: {garbage:?}"
            );
        }
    }

    #[test]
    fn era_diff_loads_the_checked_in_baseline() {
        let baseline = load_default_baseline().expect("the shipped baseline must load");
        assert!(
            baseline.deltas.len() >= 14,
            "the shipped baseline must carry the seeded entries, found {}",
            baseline.deltas.len()
        );
        assert!(baseline
            .find_by_observation_id("method.initialize")
            .is_some());
        assert_eq!(baseline.observation_ids().len(), baseline.deltas.len());
    }

    // CLAUDE.md ALWAYS / PROPERTY testing: the parser's total-function property.
    proptest::proptest! {
        /// `parse_baseline` is TOTAL over arbitrary text: it returns, never
        /// unwinds. Complements the fuzz target, which drives arbitrary BYTES.
        #[test]
        fn era_diff_parse_baseline_never_panics_on_arbitrary_text(text in ".*") {
            let _ = parse_baseline(&text);
        }

        /// Whenever the parser ACCEPTS, its documented contract holds: every
        /// `id` and `observation_id` is non-empty and unique.
        #[test]
        fn era_diff_accepted_baselines_have_unique_nonempty_ids(suffix in "[a-z]{1,8}") {
            let yaml = valid_yaml().replace("method.server_discover", &format!("method.{suffix}"));
            if let Ok(baseline) = parse_baseline(&yaml) {
                let ids: std::collections::HashSet<&str> =
                    baseline.deltas.iter().map(|d| d.id.as_str()).collect();
                let observation_ids: std::collections::HashSet<&str> =
                    baseline.deltas.iter().map(|d| d.observation_id.as_str()).collect();
                proptest::prop_assert_eq!(ids.len(), baseline.deltas.len());
                proptest::prop_assert_eq!(observation_ids.len(), baseline.deltas.len());
                proptest::prop_assert!(baseline.deltas.iter().all(|d| !d.id.trim().is_empty()));
                proptest::prop_assert!(
                    baseline.deltas.iter().all(|d| !d.observation_id.trim().is_empty())
                );
            }
        }
    }
}
