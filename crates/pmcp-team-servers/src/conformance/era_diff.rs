//! The expected-difference baseline between MCP 2025-11-25 (v1) and
//! MCP 2026-07-28 (v2) for the Phase-118 era target: its data model, its total
//! reader, and the `observation_id`-keyed join.
//!
//! Ported from `crates/mcp-tester/src/era_diff.rs` (Phase 117) under Phase 118
//! **D-16**. D-07 already said *"reuse, do not reinvent"*; this module transfers
//! the shipped machinery one crate over rather than rebuilding it.
//!
//! # What this module is for
//!
//! v2 legitimately differs from v1 BY DESIGN — no `initialize`, `resultType`
//! added, caching hints REQUIRED rather than optional, `logging/setLevel`
//! replaced by a `_meta` key — so a naive dual-run diff is pure noise. Encoding
//! the KNOWN deltas is what turns "the two runs differ" into "the two runs
//! differ IN A WAY WE DID NOT EXPECT", which is the only interesting signal.
//!
//! The encoding lives in `crates/pmcp-team-servers/baselines/era-deltas.yaml`,
//! is reviewable as a spec artifact by someone who does not read Rust, and is
//! the adjudication surface for CONF-02 and CONF-03.
//!
//! # Scope
//!
//! The baseline model, its parser, [`compare_eras`] and the
//! [`EraComparisonReport`] that carries the verdict. The comparison joins
//! observed [`ObservationId`]s against
//! [`EraDelta::observation_id`] — never against a fixture `case_id`, which could
//! not observe most of the baseline at all and would emit nothing whatsoever
//! when two eras both pass the same expected response.
//!
//! # What this module deliberately does NOT do
//!
//! It does not exempt a `provisional` entry from the `Missing` arm. The flag is
//! carried on [`ClassifiedDifference::provisional`] and changes only the RENDER.
//! Any exemption is the CONSUMER's decision (plan 118-07's matrix test), where
//! it is visible; baking it in here would make a stale provisional row
//! permanently invisible.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::era_observations::{EraObservations, ObservationId, ObservedValue};

/// Why a baseline could not be read.
///
/// A dedicated `thiserror` enum rather than a general-purpose error crate: this
/// crate already uses `thiserror` throughout (`src/fs/backend.rs`,
/// `src/mem/backend.rs`, `src/compose/wiring.rs`), and the Phase-118 threat
/// register commits this plan to introducing **no new third-party package**.
/// Every variant below is one of [`parse_baseline`]'s documented rejections, or
/// the file-read failure [`load_baseline`] adds on top of them.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    /// The bytes are not valid YAML for the baseline schema.
    #[error("failed to parse era-delta baseline YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Some delta's `id` is empty after trimming.
    #[error("era-delta baseline: an entry has an empty `id`")]
    EmptyId,

    /// Some delta's `observation_id` is empty after trimming.
    #[error("era-delta baseline: entry `{id}` has an empty `observation_id`")]
    EmptyObservationId {
        /// The offending entry's `ERA-NN` label.
        id: String,
    },

    /// Two deltas share an `id`.
    #[error("era-delta baseline: duplicate `id` `{id}`")]
    DuplicateId {
        /// The duplicated label.
        id: String,
    },

    /// Two deltas share an `observation_id` — which would silently merge two
    /// distinct wire facts.
    #[error("era-delta baseline: duplicate `observation_id` `{observation_id}` (on entry `{id}`)")]
    DuplicateObservationId {
        /// The duplicated join key.
        observation_id: String,
        /// The entry that re-used it.
        id: String,
    },

    /// The baseline file could not be read from disk.
    #[error("failed to read era-delta baseline `{path}`: {source}")]
    Read {
        /// The path that could not be read.
        path: String,
        /// The underlying I/O failure.
        source: std::io::Error,
    },
}

/// Result alias for the baseline reader.
pub type Result<T> = std::result::Result<T, BaselineError>;

/// One expected v1-vs-v2 difference: a difference that is CORRECT BY DESIGN.
///
/// Field semantics are documented in the baseline file's own header, which is
/// the reviewer-facing copy of this contract.
///
/// `note` and `provisional` carry `#[serde(default)]` so the schema stays
/// forward-compatible: a future optional field can be added without
/// invalidating every checked-in baseline.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::load_default_baseline;
///
/// let baseline = load_default_baseline().expect("the shipped baseline parses");
/// let delta = baseline
///     .find_by_observation_id("method.server_discover")
///     .expect("ERA-02 is recorded");
/// assert_eq!(delta.v1, "error:-32601");
/// assert_eq!(delta.v2, "served");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraDelta {
    /// Stable human-facing label, `ERA-NN`. Unique across the baseline.
    pub id: String,

    /// The STABLE, MACHINE-FACING name of the wire fact this entry is about —
    /// namespaced, lowercase, dot-separated (`method.initialize`,
    /// `header.mcp_session_id`, `result.cache_scope`, …).
    ///
    /// This is the JOIN KEY [`compare_eras`] diffs on. It is REQUIRED (no
    /// `serde(default)`) and must be unique: a missing or duplicated value
    /// silently merges two distinct wire facts. It is NOT a human-facing test
    /// name and **must never be renamed for readability**.
    ///
    /// It exists because
    /// [`CaseResult`](crate::conformance::runner::CaseResult) carries only
    /// `{case_id, passed, detail}` — no header, no session id, no
    /// result-envelope key, no HTTP status — so a comparison keyed on fixture
    /// pass/fail could not observe most of the baseline's entries, and would
    /// emit nothing at all when two eras both pass the same expected response.
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
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::EraBaseline;
///
/// let empty = EraBaseline::empty();
/// assert!(empty.observation_ids().is_empty());
/// assert!(empty.find_by_observation_id("method.initialize").is_none());
/// ```
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
    /// Exists so a "baseline could not be parsed" fallback path does not have
    /// to spell out an exhaustive struct literal, which every new field would
    /// otherwise break.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::EraBaseline;
    ///
    /// assert_eq!(EraBaseline::empty().deltas.len(), 0);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::load_default_baseline;
    ///
    /// let baseline = load_default_baseline().expect("parses");
    /// assert!(baseline.find_by_observation_id("meta.log_level").is_some());
    /// assert!(baseline.find_by_observation_id("nothing.observes.this").is_none());
    /// ```
    #[must_use]
    pub fn find_by_observation_id(&self, observation_id: &str) -> Option<&EraDelta> {
        self.deltas
            .iter()
            .find(|d| d.observation_id == observation_id)
    }

    /// Every [`EraDelta::observation_id`] in file order.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::load_default_baseline;
    ///
    /// let baseline = load_default_baseline().expect("parses");
    /// assert_eq!(baseline.observation_ids().len(), baseline.deltas.len());
    /// ```
    #[must_use]
    pub fn observation_ids(&self) -> Vec<&str> {
        self.deltas
            .iter()
            .map(|d| d.observation_id.as_str())
            .collect()
    }
}

/// Path of the baseline shipped with this crate.
///
/// Derived from `CARGO_MANIFEST_DIR` so no machine-specific path is baked into
/// the source and the file resolves the same from a test, from a binary and
/// from a fuzz target. Most callers want [`load_default_baseline`], which reads
/// the COMPILED-IN bytes instead; this exists for callers that deliberately
/// want the on-disk copy (a reviewer diffing it, or an explicit override).
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::default_baseline_path;
///
/// assert!(default_baseline_path().ends_with("baselines/era-deltas.yaml"));
/// ```
#[must_use]
pub fn default_baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("baselines")
        .join("era-deltas.yaml")
}

/// Parse a baseline from text. The PURE seam — no file I/O, no environment.
///
/// This is what the fuzz target (`fuzz/fuzz_targets/team_era_deltas_parser.rs`)
/// drives, so it MUST NOT PANIC on any input: every rejection below is an
/// `Err`.
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
/// (lowercase, dot-separated), the length of a `source`, whether a provisional
/// entry names its owning phase, and whether the id is in
/// [`PROBE_REGISTRY`](crate::conformance::era_observations::PROBE_REGISTRY). Those are
/// baseline-CONTENT rules, gated by
/// `crates/pmcp-team-servers/tests/era_baseline.rs` against the checked-in file
/// — not properties of arbitrary input.
///
/// # Errors
///
/// Returns a [`BaselineError`] for each of the four cases above.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::parse_baseline;
///
/// let text = "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas: []\n";
/// assert_eq!(parse_baseline(text).expect("parses").schema_version, 1);
///
/// // Arbitrary bytes are an Err, never a panic.
/// assert!(parse_baseline("{{{{").is_err());
/// ```
pub fn parse_baseline(text: &str) -> Result<EraBaseline> {
    let baseline: EraBaseline = serde_yaml::from_str(text)?;

    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_observation_ids: HashSet<&str> = HashSet::new();

    for delta in &baseline.deltas {
        if delta.id.trim().is_empty() {
            return Err(BaselineError::EmptyId);
        }
        if delta.observation_id.trim().is_empty() {
            return Err(BaselineError::EmptyObservationId {
                id: delta.id.clone(),
            });
        }
        if !seen_ids.insert(delta.id.as_str()) {
            return Err(BaselineError::DuplicateId {
                id: delta.id.clone(),
            });
        }
        if !seen_observation_ids.insert(delta.observation_id.as_str()) {
            return Err(BaselineError::DuplicateObservationId {
                observation_id: delta.observation_id.clone(),
                id: delta.id.clone(),
            });
        }
    }

    Ok(baseline)
}

/// Read and parse a baseline from an explicit path.
///
/// # Errors
///
/// Returns [`BaselineError::Read`] when the file cannot be read, or whatever
/// [`parse_baseline`] rejects its contents with.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::{default_baseline_path, load_baseline};
///
/// let baseline = load_baseline(default_baseline_path()).expect("the shipped file parses");
/// assert_eq!(baseline.schema_version, 1);
/// ```
pub fn load_baseline<P: AsRef<Path>>(path: P) -> Result<EraBaseline> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| BaselineError::Read {
        path: path.display().to_string(),
        source,
    })?;
    parse_baseline(&text)
}

/// The baseline text, COMPILED IN rather than read from disk at runtime.
///
/// This crate's manifest sets `exclude = [".planning/", "fuzz/", "tests/"]`, so
/// the `tests/`-resident schema gate does NOT ship while `baselines/` DOES.
/// That asymmetry is exactly why embedding matters more here than in the
/// mcp-tester analog: the DATA travels to downstream consumers while its gate
/// does not. A runtime path lookup resolves through `CARGO_MANIFEST_DIR`, which
/// for an installed crate points into `~/.cargo/registry/src/…` — a cache
/// directory that `cargo cache` and manual cleanup delete. Reading it at
/// runtime therefore works in every in-repo test and fuzz run (which execute
/// from the source tree) while silently failing for end users, so the failure
/// mode is structurally untestable in CI. Embedding the bytes removes it.
///
/// `CARGO_MANIFEST_DIR` expands to an absolute path and `include_str!` accepts
/// one; `concat!(env!("CARGO_MANIFEST_DIR"), "/…")` is the standard idiom and
/// is what the analog uses.
const DEFAULT_BASELINE_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/baselines/era-deltas.yaml"
));

/// Parse the baseline shipped with this crate.
///
/// The text is embedded at compile time, so this does no file I/O and cannot
/// fail for a missing or unreadable file. [`default_baseline_path`] remains
/// available for callers that deliberately want the on-disk copy.
///
/// # Errors
///
/// Only for the reasons [`parse_baseline`] rejects its contents — which, for
/// the checked-in file, is gated by
/// `crates/pmcp-team-servers/tests/era_baseline.rs`.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::load_default_baseline;
///
/// let baseline = load_default_baseline().expect("the shipped baseline must parse");
/// assert_eq!(baseline.v1_protocol, pmcp::LATEST_PROTOCOL_VERSION);
/// ```
pub fn load_default_baseline() -> Result<EraBaseline> {
    parse_baseline(DEFAULT_BASELINE_TEXT)
}

// ===========================================================================
// The comparison.
// ===========================================================================

/// How one observation-id's v1-vs-v2 behaviour relates to the baseline.
///
/// [`Self::Unexpected`] and [`Self::Missing`] are BOTH findings, reported as
/// DISTINCT categories because they mean opposite things and call for opposite
/// remedies: an unexpected difference means the implementation moved, a missing
/// one means either the spec moved or a documented behaviour regressed.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::DifferenceClass;
///
/// assert!(!DifferenceClass::Expected.is_finding());
/// assert!(DifferenceClass::Unexpected.is_finding());
/// assert!(DifferenceClass::Missing.is_finding());
/// assert_eq!(DifferenceClass::Missing.label(), "MISSING");
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::DifferenceClass;
    ///
    /// assert!(!DifferenceClass::Expected.is_finding());
    /// ```
    #[must_use]
    pub fn is_finding(self) -> bool {
        !matches!(self, Self::Expected)
    }

    /// Stable label for the rendered report.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::DifferenceClass;
    ///
    /// assert_eq!(DifferenceClass::Unexpected.label(), "UNEXPECTED");
    /// ```
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Expected => "EXPECTED",
            Self::Unexpected => "UNEXPECTED",
            Self::Missing => "MISSING",
        }
    }
}

/// One classified observation.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::{ClassifiedDifference, DifferenceClass};
///
/// let row = ClassifiedDifference {
///     observation_id: "meta.log_level".to_string(),
///     class: DifferenceClass::Unexpected,
///     v1: None,
///     v2: None,
///     delta_id: None,
///     provisional: false,
///     detail: "the eras differ with NO baseline entry".to_string(),
/// };
/// assert!(row.class.is_finding());
/// ```
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
    /// A provisional delta that goes MISSING is a legible consequence of an
    /// unsigned-off area still settling; a non-provisional one going MISSING is
    /// a regression. Rendering them identically would make the first look
    /// alarming and the second look routine — so the flag is recorded here and
    /// shown in the render, and the CONSUMER decides what to do about it.
    #[serde(default)]
    pub provisional: bool,
    /// Human-readable explanation of this row's classification.
    pub detail: String,
}

/// The era-comparison verdict: two observation sets plus their classification.
///
/// The team-servers analog of `mcp-tester`'s `DualRunReport`, MINUS the two
/// `TestReport` fields — this crate has no `TestReport`, and the 33 in-process
/// fixtures are a v1-only regression guard rather than an era surface (D-16).
///
/// `schema_version` is the wire-format guard, and every optional field carries
/// `#[serde(default, skip_serializing_if = "Option::is_none")]` so a future
/// additive field does not invalidate a stored report.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::conformance::era_diff::{DifferenceClass, EraComparisonReport};
/// use pmcp_team_servers::conformance::era_observations::EraObservations;
///
/// let report = EraComparisonReport {
///     schema_version: 1,
///     era_support: "dual".to_string(),
///     v1_observations: EraObservations::default(),
///     v2_observations: EraObservations::default(),
///     differences: Vec::new(),
///     suspicion: None,
///     note: None,
/// };
/// assert!(report.findings().is_empty());
/// assert_eq!(report.count(DifferenceClass::Missing), 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraComparisonReport {
    /// Wire-format version of this struct. Currently `1`.
    pub schema_version: u32,
    /// Which eras the target was detected to serve (`dual`, `v1-only`, …).
    pub era_support: String,
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

impl EraComparisonReport {
    /// Rows that are findings (UNEXPECTED or MISSING).
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::EraComparisonReport;
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// let report = EraComparisonReport {
    ///     schema_version: 1,
    ///     era_support: "dual".to_string(),
    ///     v1_observations: EraObservations::default(),
    ///     v2_observations: EraObservations::default(),
    ///     differences: Vec::new(),
    ///     suspicion: None,
    ///     note: None,
    /// };
    /// assert!(report.findings().is_empty());
    /// ```
    #[must_use]
    pub fn findings(&self) -> Vec<&ClassifiedDifference> {
        self.differences
            .iter()
            .filter(|d| d.class.is_finding())
            .collect()
    }

    /// Count of rows in one class.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::{DifferenceClass, EraComparisonReport};
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// let report = EraComparisonReport {
    ///     schema_version: 1,
    ///     era_support: "dual".to_string(),
    ///     v1_observations: EraObservations::default(),
    ///     v2_observations: EraObservations::default(),
    ///     differences: Vec::new(),
    ///     suspicion: None,
    ///     note: None,
    /// };
    /// assert_eq!(report.count(DifferenceClass::Expected), 0);
    /// ```
    #[must_use]
    pub fn count(&self, class: DifferenceClass) -> usize {
        self.differences.iter().filter(|d| d.class == class).count()
    }

    /// Render the comparison into any writer.
    ///
    /// A writer rather than `println!` so a test can capture into a `Vec<u8>`
    /// and assert on BYTES.
    ///
    /// # Errors
    ///
    /// Propagates the writer's I/O errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_team_servers::conformance::era_diff::EraComparisonReport;
    /// use pmcp_team_servers::conformance::era_observations::EraObservations;
    ///
    /// let report = EraComparisonReport {
    ///     schema_version: 1,
    ///     era_support: "dual".to_string(),
    ///     v1_observations: EraObservations::default(),
    ///     v2_observations: EraObservations::default(),
    ///     differences: Vec::new(),
    ///     suspicion: None,
    ///     note: None,
    /// };
    /// let mut sink = Vec::<u8>::new();
    /// report.print_to_writer(&mut sink).expect("renders");
    /// assert!(String::from_utf8(sink).unwrap().contains("ERA COMPARISON"));
    /// ```
    pub fn print_to_writer<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        self.print_header(w)?;
        for class in [
            DifferenceClass::Unexpected,
            DifferenceClass::Missing,
            DifferenceClass::Expected,
        ] {
            self.print_class(w, class)?;
        }
        if let Some(suspicion) = &self.suspicion {
            writeln!(w, "SUSPICIOUS: {suspicion}")?;
            writeln!(w)?;
        }
        Ok(())
    }

    /// The summary block. Split out of [`Self::print_to_writer`] so neither
    /// half approaches the cognitive-complexity ceiling (CLAUDE.md C-3).
    fn print_header<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w)?;
        writeln!(w, "ERA COMPARISON")?;
        writeln!(w, "{}", "=".repeat(60))?;
        writeln!(w, "Era support : {}", self.era_support)?;
        writeln!(
            w,
            "v1 observed : {} observation(s)",
            self.v1_observations.len()
        )?;
        writeln!(
            w,
            "v2 observed : {} observation(s)",
            self.v2_observations.len()
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
        writeln!(w)
    }

    /// One labelled section. A PROVISIONAL row that went MISSING is marked
    /// `[PROVISIONAL]`; a provisional row in any other class is marked
    /// `[provisional]`. That distinction is what makes a provisional MISSING
    /// legible instead of alarming, and the RENDER is the ONLY place
    /// `provisional` changes anything.
    fn print_class<W: Write>(&self, w: &mut W, class: DifferenceClass) -> std::io::Result<()> {
        let rows: Vec<&ClassifiedDifference> = self
            .differences
            .iter()
            .filter(|d| d.class == class)
            .collect();
        if rows.is_empty() {
            return Ok(());
        }
        writeln!(w, "{} ({})", class.label(), rows.len())?;
        for row in rows {
            let provisional = match (row.provisional, class) {
                (true, DifferenceClass::Missing) => " [PROVISIONAL]",
                (true, _) => " [provisional]",
                (false, _) => "",
            };
            let delta = row
                .delta_id
                .as_ref()
                .map_or_else(String::new, |d| format!(" ({d})"));
            writeln!(w, "  {}{}{}", row.observation_id, delta, provisional)?;
            writeln!(w, "      {}", row.detail)?;
        }
        writeln!(w)
    }
}

/// Classify every observation against `baseline`, joining on
/// [`ObservationId`] — never on a
/// display name and never on a fixture `case_id`.
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
/// `provisional` is NOT consulted anywhere in that rule. A provisional delta
/// that no longer reproduces is classified `Missing` exactly like a
/// non-provisional one, and the flag is carried on the row so the render can
/// distinguish them. Baking the exemption in here would make a stale
/// provisional row permanently invisible.
///
/// # The anti-vacuity guard
///
/// Two independent era runs against a DUAL-era server MUST differ: the baseline
/// lists fourteen differences that are correct by design. So an empty
/// difference list does not mean "all clear" — it means the comparison probably
/// did not run, or every probe came back `Unavailable`. That is surfaced as the
/// returned `Option<String>` rather than rendered as a clean bill of health,
/// because a silently-never-matching comparison would classify everything as
/// agreement and be indistinguishable from success.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use pmcp_team_servers::conformance::era_diff::{
///     compare_eras, load_default_baseline, DifferenceClass,
/// };
/// use pmcp_team_servers::conformance::era_observations::{
///     EraObservations, ObservedValue, METHOD_SERVER_DISCOVER,
/// };
///
/// let baseline = load_default_baseline().expect("parses");
///
/// let mut v1 = BTreeMap::new();
/// v1.insert(METHOD_SERVER_DISCOVER, ObservedValue::Text("error:-32601".into()));
/// let mut v2 = BTreeMap::new();
/// v2.insert(METHOD_SERVER_DISCOVER, ObservedValue::Text("served".into()));
///
/// let (differences, _suspicion) =
///     compare_eras(&EraObservations(v1), &EraObservations(v2), &baseline);
/// let row = differences
///     .iter()
///     .find(|d| d.observation_id == "method.server_discover")
///     .expect("classified");
/// assert_eq!(row.class, DifferenceClass::Expected);
/// ```
#[must_use]
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
        // `EraObservations::get` is the newtype's own lookup. An observation
        // can only ever be keyed by a registry id, so a non-registry `id`
        // correctly yields `None` from both sides.
        let key = ObservationId::from_registry(id);
        let observed_v1 = key.and_then(|k| v1.get(k));
        let observed_v2 = key.and_then(|k| v2.get(k));
        let delta = baseline.find_by_observation_id(id);

        if let Some(row) = classify_one(id, observed_v1, observed_v2, delta) {
            differences.push(row);
        }
    }

    let suspicion = suspicion_for(&differences, baseline);
    (differences, suspicion)
}

/// Classify ONE id. Extracted from [`compare_eras`] so neither function
/// approaches the cognitive-complexity ceiling (CLAUDE.md C-3, technique P1).
///
/// `None` is the "NOT DIFFERED + no delta" arm — not reported at all.
fn classify_one(
    id: &str,
    observed_v1: Option<&ObservedValue>,
    observed_v2: Option<&ObservedValue>,
    delta: Option<&EraDelta>,
) -> Option<ClassifiedDifference> {
    let differed = match (observed_v1, observed_v2) {
        (Some(a), Some(b)) => a.is_established() && b.is_established() && a.token() != b.token(),
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
        (false, None, _) => return None,
    };

    Some(ClassifiedDifference {
        observation_id: id.to_string(),
        class,
        v1: observed_v1.cloned(),
        v2: observed_v2.cloned(),
        delta_id: delta.map(|d| d.id.clone()),
        provisional: delta.is_some_and(|d| d.provisional),
        detail,
    })
}

/// THE ANTI-VACUITY GUARD. An empty difference list is reported as SUSPICIOUS,
/// never as a clean bill of health.
fn suspicion_for(differences: &[ClassifiedDifference], baseline: &EraBaseline) -> Option<String> {
    if !differences.is_empty() {
        return None;
    }
    Some(format!(
        "the classified difference list is EMPTY, yet the baseline records {} \
         differences that are correct by design. Two independent era runs \
         against a dual-era server MUST differ, so this is far more likely to \
         mean the comparison did not run (or every probe returned \
         `unavailable`) than that the eras agree.",
        baseline.deltas.len()
    ))
}

/// Render an optional observation as its token, or `not observed`.
fn token_of(value: Option<&ObservedValue>) -> String {
    value.map_or_else(|| "not observed".to_string(), ObservedValue::token)
}

#[cfg(test)]
mod comparison {
    use super::*;
    use crate::conformance::era_observations::{
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

    fn report_of(differences: Vec<ClassifiedDifference>) -> EraComparisonReport {
        EraComparisonReport {
            schema_version: 1,
            era_support: "dual".to_string(),
            v1_observations: EraObservations::default(),
            v2_observations: EraObservations::default(),
            differences,
            suspicion: None,
            note: None,
        }
    }

    /// THE JOIN RULE, exercised with a matching AND a non-matching case. A rule
    /// that silently never matched would classify every delta EXPECTED (or
    /// every one UNEXPECTED) and could not fail either half of this test.
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

    /// A provisional delta going MISSING is classified EXACTLY like a
    /// non-provisional one — the flag changes the RENDER, never the class.
    #[test]
    fn provisional_missing_is_reported_distinctly_from_non_provisional() {
        let baseline = baseline_with(&[
            ("method.initialize", "served", "absent", false),
            ("result.cache_scope", "absent", "required", true),
        ]);
        let same = |id| observations(&[(id, ObservedValue::Text("same".into()))]);
        let mut merged = same(METHOD_INITIALIZE).0;
        merged.extend(same(RESULT_CACHE_SCOPE).0);
        let observations_both = EraObservations(merged);
        let (differences, _) = compare_eras(&observations_both, &observations_both, &baseline);

        let non_provisional = differences
            .iter()
            .find(|d| d.observation_id == "method.initialize")
            .expect("classified");
        let provisional = differences
            .iter()
            .find(|d| d.observation_id == "result.cache_scope")
            .expect("classified");
        assert_eq!(
            non_provisional.class,
            DifferenceClass::Missing,
            "the classification must not consult `provisional`"
        );
        assert_eq!(
            provisional.class,
            DifferenceClass::Missing,
            "a PROVISIONAL delta is classified Missing exactly like a non-provisional one"
        );
        assert!(!non_provisional.provisional);
        assert!(provisional.provisional);

        let mut sink = Vec::<u8>::new();
        report_of(differences)
            .print_to_writer(&mut sink)
            .expect("render");
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
        let report = report_of(differences);
        let mut sink = Vec::<u8>::new();
        report.print_to_writer(&mut sink).expect("render");
        let text = String::from_utf8(sink).expect("utf8");
        assert!(text.contains("UNEXPECTED (1)"), "{text}");
        assert!(text.contains("MISSING (1)"), "{text}");
        assert_eq!(report.findings().len(), 2);
        assert_eq!(report.count(DifferenceClass::Missing), 1);
    }

    /// An EXPECTED row renders in its own labelled section too, so a reader can
    /// see what was adjudicated rather than only what failed.
    #[test]
    fn render_reports_expected_rows_in_their_own_section() {
        let baseline = baseline_with(&[("method.initialize", "served", "absent", true)]);
        let (differences, _) = compare_eras(
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("served".into()))]),
            &observations(&[(METHOD_INITIALIZE, ObservedValue::Text("absent".into()))]),
            &baseline,
        );
        let report = report_of(differences);
        let mut sink = Vec::<u8>::new();
        report.print_to_writer(&mut sink).expect("render");
        let text = String::from_utf8(sink).expect("utf8");
        assert!(text.contains("EXPECTED (1)"), "{text}");
        assert!(
            text.contains("[provisional]") && !text.contains("[PROVISIONAL]"),
            "a provisional row outside MISSING gets the quiet marker:\n{text}"
        );
        assert!(report.findings().is_empty());
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

    /// Optional fields must be omitted, not serialized as `null`.
    #[test]
    fn optional_fields_are_omitted_when_absent() {
        let json = serde_json::to_string(&report_of(Vec::new())).expect("serializes");
        assert!(!json.contains("\"note\""), "{json}");
        assert!(!json.contains("\"suspicion\""), "{json}");
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
            let mut report = report_of(vec![ClassifiedDifference {
                observation_id: "x.y".to_string(),
                class: DifferenceClass::Unexpected,
                v1: None,
                v2: None,
                delta_id: None,
                provisional: false,
                detail,
            }]);
            report.era_support = era_support;
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
    source: "src/client/mod.rs:761"
  - id: ERA-02
    observation_id: method.server_discover
    subject: "method:server/discover"
    v1: "error:-32601"
    v2: served
    kind: method-added
    source: "src/server/core.rs:1944"
    provisional: true
    note: "Phase 118 plan 07 owns this."
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
        assert_eq!(
            baseline.deltas.len(),
            14,
            "the shipped baseline carries one row per PROBE_REGISTRY entry, found {}",
            baseline.deltas.len()
        );
        assert!(baseline
            .find_by_observation_id("method.initialize")
            .is_some());
        assert_eq!(baseline.observation_ids().len(), baseline.deltas.len());
    }

    /// The on-disk copy and the compiled-in copy must be the SAME bytes; a
    /// stale build artifact would otherwise gate against yesterday's file.
    #[test]
    fn the_compiled_in_baseline_matches_the_on_disk_file() {
        let from_disk = load_baseline(default_baseline_path()).expect("on-disk copy parses");
        let compiled_in = load_default_baseline().expect("compiled-in copy parses");
        assert_eq!(from_disk, compiled_in);
    }

    #[test]
    fn load_baseline_reports_a_missing_file_rather_than_panicking() {
        let err = load_baseline(default_baseline_path().join("nope.yaml"))
            .expect_err("a missing file must be an Err");
        assert!(
            err.to_string()
                .contains("failed to read era-delta baseline"),
            "the error must name the failure: {err}"
        );
    }

    // CLAUDE.md ALWAYS / PROPERTY testing: the parser's total-function property.
    proptest::proptest! {
        /// `parse_baseline` is TOTAL over arbitrary text: it returns, never
        /// unwinds. Complements the fuzz target, which drives arbitrary BYTES.
        #[test]
        fn era_diff_parse_baseline_never_panics_on_arbitrary_text(text in ".*") {
            let _ = parse_baseline(&text);
        }

        /// A generated delta list parses to `Ok` EXACTLY when every `id` and
        /// every `observation_id` is non-empty-after-trim and pairwise unique.
        /// The oracle is computed from the GENERATED inputs, never read back
        /// out of the parser under test.
        #[test]
        fn era_diff_accepts_exactly_the_nonempty_unique_baselines(
            ids in proptest::collection::vec("[ A-Za-z0-9-]{0,6}", 0..5),
            observation_ids in proptest::collection::vec("[ a-z0-9._]{0,8}", 0..5),
        ) {
            let pairs: Vec<(&String, &String)> = ids.iter().zip(observation_ids.iter()).collect();

            let deltas: Vec<EraDelta> = pairs
                .iter()
                .map(|(id, observation_id)| EraDelta {
                    id: (*id).clone(),
                    observation_id: (*observation_id).clone(),
                    subject: "s".to_string(),
                    v1: "a".to_string(),
                    v2: "b".to_string(),
                    kind: "k".to_string(),
                    source: "unit test citation".to_string(),
                    note: None,
                    provisional: false,
                })
                .collect();
            let baseline = EraBaseline {
                schema_version: 1,
                v1_protocol: "2025-11-25".to_string(),
                v2_protocol: "2026-07-28".to_string(),
                deltas,
            };
            let yaml = serde_yaml::to_string(&baseline).expect("serializes");

            // The ORACLE, computed from the generated inputs.
            let nonempty = pairs
                .iter()
                .all(|(id, oid)| !id.trim().is_empty() && !oid.trim().is_empty());
            let unique_ids: HashSet<&str> = pairs.iter().map(|(id, _)| id.as_str()).collect();
            let unique_oids: HashSet<&str> = pairs.iter().map(|(_, oid)| oid.as_str()).collect();
            let unique = unique_ids.len() == pairs.len() && unique_oids.len() == pairs.len();

            proptest::prop_assert_eq!(parse_baseline(&yaml).is_ok(), nonempty && unique);
        }
    }
}
