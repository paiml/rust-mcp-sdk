//! `pmcp-workbook-dialect` — the versioned workbook dialect *contract*.
//!
//! This crate owns the SDK's governance contract for the constrained Excel
//! dialect: the deny-by-default function [`WHITELIST`], the [`DialectRules`]
//! machine-rules value (whitelist + colour→role palette + sheet-layering), the
//! [`CandidateRole`] colour-evidence ontology, and the doc↔const binding test
//! that prevents the published spec (`docs/workbook-dialect-spec.md`) and the
//! enforced `WHITELIST` const from ever drifting (WBDL-01).
//!
//! It is a **reader-free leaf** (D-01): it depends ONLY on
//! `pmcp-workbook-runtime` (whose finding types it re-exports per D-03) and
//! carries no workbook reader (no `umya` / `quick-xml` / `swc` / `pmcp-code-mode`).
//! The linter execution and the owned `WorkbookMap` are deliberately NOT here —
//! they are Phase 93 (D-02).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

// Re-export the runtime's finding types (D-03) so dialect consumers get them
// from here. The linter (Phase 93) layers on top of these.
pub use pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity};

/// The constrained-dialect function whitelist (DIA-05). 17 flat first-class
/// names that the lighthouse workbook authors, so it lints clean as-authored.
///
/// # D-05 decision (flat, no tiering)
///
/// There is NO core/widened split: all 17 functions are first-class. The set was
/// surfaced explicitly from what the lighthouse workbook authors — the dialect
/// accepts exactly these names rather than (a) silently auto-widening the
/// whitelist to whatever a workbook happens to use, or (b) refactoring a source
/// workbook to avoid them. Widen deliberately; do not auto-widen; do not rewrite
/// the source workbook. Removing any of these breaks the reference workbook's
/// clean lint (D-07).
///
/// ## Deliberate widening, dialect 1.1 (2026-09-06)
///
/// `ROUNDDOWN`, `MAX`, `MIN` and `XLOOKUP` were added as a DELIBERATE widening —
/// the four names a BA most often reaches for and previously got `#NAME?` from.
/// This note lives here because this rustdoc is the place that says "widen
/// deliberately", so every widening must leave its trace in it. `XLOOKUP` is
/// accepted only in its narrow 3/4-argument EXACT form (spec §3.2); a
/// `match_mode`/`search_mode` argument is a located parse-time rejection.
pub const WHITELIST: &[&str] = &[
    "IF",
    "VLOOKUP",
    "INDEX",
    "MATCH",
    "SUMIF",
    "SUM",
    "ROUNDUP",
    "CEILING",
    "IFERROR",
    "ISNUMBER",
    "SEARCH",
    "ROUND",
    "TEXT",
    "ROUNDDOWN",
    "MAX",
    "MIN",
    "XLOOKUP",
];

/// The baseline dialect version a workbook with NO `pmcp_dialect_version`
/// declaration targets (WBDL-02 / D-05). An absent declaration is NOT an error —
/// it compiles against this baseline. Parallel to [`WHITELIST`]: this crate owns
/// the version *contract*, bound to `docs/workbook-dialect-spec.md` by the
/// `dialect_version_spec` drift-guard test.
pub const BASELINE_DIALECT_VERSION: &str = "1.0";

/// The maximum `MAJOR.MINOR` dialect version the compiler supports (WBDL-02 /
/// D-04). A declared version with the same major and a minor `<=` this is
/// accepted; a different major OR a newer minor fails closed with a typed
/// `CompileError`. Bound to the spec doc by the `dialect_version_spec` drift guard
/// so the published contract and the enforced const can never drift.
pub const SUPPORTED_DIALECT_VERSION: &str = "1.1";

/// The fallback colour-role palette ARGBs (the lighthouse's known direct fills /
/// fonts). A later synthesis phase MAY override the palette from the `0_Guide`
/// legend; [`DialectRules::default`] owns THIS hardcoded fallback.
const INPUT_FONT_ARGB: &str = "FF0000FF"; // blue font  → input
const CONSTANT_FILL_ARGB: &str = "FFE2EFDA"; // green fill → constant (governed)
const ASSUMPTION_FILL_ARGB: &str = "FFFFFF00"; // yellow fill → assumption (Guide-overridable)

/// The candidate-role label a colour signal implies. These are EVIDENCE labels
/// the linter/synthesis emit; a later phase maps them onto its `Role` enum
/// (notably `assumption` → `Constant { source: "yellow-assumption" }`). Kept as
/// a small enum (not free strings) so the ontology stays consistent across the
/// linter and synthesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateRole {
    /// Blue input font (`FF0000FF`) — a per-quote overridable input.
    Input,
    /// Green governed-constant fill (`FFE2EFDA`).
    Constant,
    /// Yellow assumption fill (Guide-overridable; default `FFFFFF00`).
    Assumption,
    /// Default font + a formula `<f>` — a derived/formula cell.
    Formula,
}

impl CandidateRole {
    /// The lowercase evidence label (`"input"`/`"constant"`/`"assumption"`/
    /// `"formula"`) a later phase keys its `Role` mapping on.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CandidateRole::Input => "input",
            CandidateRole::Constant => "constant",
            CandidateRole::Assumption => "assumption",
            CandidateRole::Formula => "formula",
        }
    }
}

/// The machine-rules value the linter + synthesis read: the whitelist, the
/// colour→role palette, and the expected sheet-prefix layering — all from ONE
/// place so the linter + synthesis construct rules identically.
#[derive(Debug, Clone)]
pub struct DialectRules {
    whitelist: &'static [&'static str],
    /// ARGB→[`CandidateRole`] palette (fill or font, depending on the role).
    input_font_argb: &'static str,
    constant_fill_argb: &'static str,
    assumption_fill_argb: &'static str,
    /// The expected ordered sheet-prefix layering (e.g. `0_`, `1_`, `2_`): the
    /// numbered-layer convention the linter checks sheet names against.
    sheet_layer_prefixes: Vec<String>,
}

impl Default for DialectRules {
    /// The fallback rules: the 17-name [`WHITELIST`] + the hardcoded colour
    /// palette + the numbered sheet-layer prefixes. A later phase may override
    /// the palette from the `0_Guide` legend. So both the linter and synthesis
    /// construct rules from one place: `DialectRules::default()`.
    fn default() -> Self {
        Self {
            whitelist: WHITELIST,
            input_font_argb: INPUT_FONT_ARGB,
            constant_fill_argb: CONSTANT_FILL_ARGB,
            assumption_fill_argb: ASSUMPTION_FILL_ARGB,
            sheet_layer_prefixes: vec![
                "0_".to_string(),
                "1_".to_string(),
                "2_".to_string(),
                "3_".to_string(),
            ],
        }
    }
}

impl DialectRules {
    /// The whitelisted function tokens the linter checks against (DIA-05).
    #[must_use]
    pub fn whitelist(&self) -> &[&str] {
        self.whitelist
    }

    /// The expected ordered sheet-layer prefixes.
    #[must_use]
    pub fn sheet_layer_prefixes(&self) -> &[String] {
        &self.sheet_layer_prefixes
    }

    /// Map a cell's ARGB string + `is_formula` flag to the candidate role it
    /// implies, if any. `fill_argb` is the background colour, `font_argb` the
    /// font colour (either may be `None`). Resolution order: green fill →
    /// constant; yellow fill → assumption; blue font → input; otherwise a
    /// formula cell → formula; else `None` (no colour signal).
    #[must_use]
    pub fn candidate_role(
        &self,
        fill_argb: Option<&str>,
        font_argb: Option<&str>,
        is_formula: bool,
    ) -> Option<CandidateRole> {
        if fill_argb == Some(self.constant_fill_argb) {
            return Some(CandidateRole::Constant);
        }
        if fill_argb == Some(self.assumption_fill_argb) {
            return Some(CandidateRole::Assumption);
        }
        if font_argb == Some(self.input_font_argb) {
            return Some(CandidateRole::Input);
        }
        if is_formula {
            return Some(CandidateRole::Formula);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{CandidateRole, DialectRules, WHITELIST};

    // Exact membership is pinned by the `dialect_spec` doc-binding test against
    // the published spec table (an independent source); this canary owns only
    // the structural invariants, so a deliberate D-05 widening edits two lists
    // (const + spec table), not three.
    #[test]
    fn whitelist_canary_count_unique_uppercase() {
        assert_eq!(
            WHITELIST.len(),
            17,
            "deliberate widening must update this canary (D-05)"
        );
        let unique: std::collections::BTreeSet<_> = WHITELIST.iter().collect();
        assert_eq!(
            unique.len(),
            WHITELIST.len(),
            "duplicate names in WHITELIST"
        );
        assert!(
            WHITELIST
                .iter()
                .all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_uppercase())),
            "whitelist names must be non-empty ASCII uppercase"
        );
    }

    // Const↔dispatch binding: a name present in the dialect contract but not
    // dispatched by the runtime's `semantics::apply` would lint clean yet
    // evaluate to `#NAME?` at serve time — the drift the doc↔const binding
    // test cannot see.
    #[test]
    fn every_whitelisted_function_dispatches_in_the_runtime() {
        use pmcp_workbook_runtime::sheet_ir::semantics::apply;
        use pmcp_workbook_runtime::{CellValue, ExcelError};
        for name in WHITELIST {
            let v = apply(name, &[]);
            assert!(
                !matches!(v, CellValue::Error(ExcelError::Name)),
                "{name} is in WHITELIST but the runtime dispatches it to #NAME?"
            );
        }
    }

    #[test]
    fn default_constructs_and_exposes_whitelist_and_layering() {
        let rules = DialectRules::default();
        assert_eq!(rules.whitelist().len(), 17);
        assert!(rules.sheet_layer_prefixes().contains(&"1_".to_string()));
    }

    #[test]
    fn palette_maps_known_argbs_to_roles() {
        let rules = DialectRules::default();
        // blue font → input
        assert_eq!(
            rules.candidate_role(None, Some("FF0000FF"), false),
            Some(CandidateRole::Input)
        );
        // green fill → constant
        assert_eq!(
            rules.candidate_role(Some("FFE2EFDA"), None, false),
            Some(CandidateRole::Constant)
        );
        // yellow fill → assumption label
        let yellow = rules
            .candidate_role(Some("FFFFFF00"), None, false)
            .expect("yellow maps to a role");
        assert_eq!(yellow, CandidateRole::Assumption);
        assert_eq!(yellow.label(), "assumption");
        // a formula with no colour → formula
        assert_eq!(
            rules.candidate_role(None, None, true),
            Some(CandidateRole::Formula)
        );
        // no colour, no formula → no signal
        assert_eq!(rules.candidate_role(None, None, false), None);
    }
}

/// WBDL-01 binding test: the published human dialect spec
/// (`docs/workbook-dialect-spec.md`) and the machine [`WHITELIST`] const MUST
/// never drift. This test parses the function names out of the spec's whitelist
/// table and asserts set-equality with [`WHITELIST`] — it READS `WHITELIST`, it
/// does not redefine it. If either the doc table or the const changes without the
/// other, the build fails.
///
/// Named `dialect_spec` so `cargo test -p pmcp-workbook-dialect dialect_spec`
/// runs it.
#[cfg(test)]
mod dialect_spec {
    use super::WHITELIST;
    use std::collections::BTreeSet;

    /// The published dialect spec, resolved relative to this crate's manifest dir
    /// (`crates/pmcp-workbook-dialect` → `../../docs/...`).
    const SPEC_PATH: &str = "../../docs/workbook-dialect-spec.md";

    /// Parse the function names out of the whitelist table in the markdown spec.
    ///
    /// The table rows look like `| \`IF\` | whitelist | conditional |`. We take
    /// the FIRST backtick-quoted token on each table row whose SECOND column is
    /// the `whitelist` category (D-05 flat table). This deliberately ignores
    /// other backtick tokens elsewhere in the doc (rule ids, field names) so only
    /// the whitelist table feeds the comparison.
    fn parse_doc_whitelist(markdown: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for line in markdown.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') {
                continue;
            }
            let cols: Vec<&str> = trimmed.trim_matches('|').split('|').collect();
            if cols.len() < 2 {
                continue;
            }
            // Column 2 (category) must mark this as a whitelist-table data row.
            let category = cols[1].trim();
            if category != "whitelist" {
                continue;
            }
            // Column 1 holds the function name as a single backtick token.
            if let Some(name) = first_backtick_token(cols[0]) {
                names.insert(name);
            }
        }
        names
    }

    /// Extract the first `` `BACKTICKED` `` token from a markdown cell.
    fn first_backtick_token(cell: &str) -> Option<String> {
        let start = cell.find('`')? + 1;
        let rest = &cell[start..];
        let end = rest.find('`')?;
        Some(rest[..end].trim().to_string())
    }

    #[test]
    fn doc_whitelist_table_matches_const() {
        let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SPEC_PATH);
        // WR-05: this inline test SHIPS in the published crate, but the repo's
        // `docs/` tree does not — in a published-package context (vendored
        // workspace, distro packaging, path-replaced dep) the spec file is
        // absent and the test must SKIP, not fail unconditionally. In-repo,
        // absence must FAIL — otherwise deleting the spec silently disables
        // this drift check. "In-repo" is detected by the repo's `.git` two
        // levels up from the crate (a directory in a normal checkout, a file
        // in a git worktree — `exists()` covers both); published-package
        // contexts have no `.git` at that path.
        if !spec_path.exists() {
            let in_repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.git")
                .exists();
            assert!(
                !in_repo,
                "docs/workbook-dialect-spec.md is missing from the repo — the WBDL-01 \
                 doc↔const drift check would be silently disabled (fail-closed in-repo)"
            );
            eprintln!(
                "skipping doc-binding test: dialect spec not present at {} \
                 (published-package context)",
                spec_path.display()
            );
            return;
        }
        let markdown = std::fs::read_to_string(&spec_path)
            .unwrap_or_else(|e| panic!("read published dialect spec {}: {e}", spec_path.display()));

        let doc_set = parse_doc_whitelist(&markdown);
        let const_set: BTreeSet<String> = WHITELIST.iter().map(|s| (*s).to_string()).collect();

        // Pitfall 4 guard: catches a silent empty-parse if the table format drifts.
        assert!(
            !doc_set.is_empty(),
            "parsed zero function names from the spec whitelist table — table format drifted?"
        );
        assert_eq!(
            doc_set,
            const_set,
            "published dialect-spec whitelist table and WHITELIST const have DRIFTED.\n\
             doc-only: {:?}\nconst-only: {:?}",
            doc_set.difference(&const_set).collect::<Vec<_>>(),
            const_set.difference(&doc_set).collect::<Vec<_>>(),
        );
    }
}

/// WBDL-02 binding test: the published dialect-version policy section of
/// `docs/workbook-dialect-spec.md` and the [`SUPPORTED_DIALECT_VERSION`] /
/// [`BASELINE_DIALECT_VERSION`] consts MUST never drift. This test parses the two
/// version values out of the spec's version table and asserts string-equality with
/// the consts — it READS the consts, it does not redefine them. Flipping a const
/// without editing the doc (or vice versa) fails the build.
///
/// Named `dialect_version_spec` so
/// `cargo test -p pmcp-workbook-dialect dialect_version_spec` runs it.
#[cfg(test)]
mod dialect_version_spec {
    use super::{BASELINE_DIALECT_VERSION, SUPPORTED_DIALECT_VERSION};

    /// The published dialect spec, resolved relative to this crate's manifest dir
    /// (`crates/pmcp-workbook-dialect` → `../../docs/...`).
    const SPEC_PATH: &str = "../../docs/workbook-dialect-spec.md";

    /// Parse the `supported` / `baseline` version values out of the version table.
    ///
    /// The version table rows look like
    /// `| \`supported\` | \`1.0\` | ... |`. We take the FIRST backtick-quoted token
    /// of column 1 as the field key and the FIRST backtick-quoted token of column 2
    /// as its value, keeping only the `supported` / `baseline` rows. This ignores
    /// other backtick tokens elsewhere in the doc so only the version table feeds
    /// the comparison.
    fn parse_doc_versions(markdown: &str) -> (Option<String>, Option<String>) {
        let mut supported = None;
        let mut baseline = None;
        for line in markdown.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') {
                continue;
            }
            let cols: Vec<&str> = trimmed.trim_matches('|').split('|').collect();
            if cols.len() < 2 {
                continue;
            }
            let Some(key) = first_backtick_token(cols[0]) else {
                continue;
            };
            let Some(value) = first_backtick_token(cols[1]) else {
                continue;
            };
            match key.as_str() {
                "supported" => supported = Some(value),
                "baseline" => baseline = Some(value),
                _ => {},
            }
        }
        (supported, baseline)
    }

    /// Extract the first `` `BACKTICKED` `` token from a markdown cell.
    fn first_backtick_token(cell: &str) -> Option<String> {
        let start = cell.find('`')? + 1;
        let rest = &cell[start..];
        let end = rest.find('`')?;
        Some(rest[..end].trim().to_string())
    }

    #[test]
    fn doc_versions_match_consts() {
        let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SPEC_PATH);
        // WR-05: this inline test SHIPS in the published crate, but the repo's
        // `docs/` tree does not — in a published-package context the spec file is
        // absent and the test must SKIP, not fail unconditionally. In-repo,
        // absence must FAIL — otherwise deleting the spec silently disables this
        // drift check. "In-repo" is detected by the repo's `.git` two levels up
        // from the crate (a directory in a normal checkout, a file in a git
        // worktree — `exists()` covers both); published-package contexts have no
        // `.git` at that path.
        if !spec_path.exists() {
            let in_repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.git")
                .exists();
            assert!(
                !in_repo,
                "docs/workbook-dialect-spec.md is missing from the repo — the WBDL-02 \
                 version doc↔const drift check would be silently disabled (fail-closed in-repo)"
            );
            eprintln!(
                "skipping version doc-binding test: dialect spec not present at {} \
                 (published-package context)",
                spec_path.display()
            );
            return;
        }
        let markdown = std::fs::read_to_string(&spec_path)
            .unwrap_or_else(|e| panic!("read published dialect spec {}: {e}", spec_path.display()));

        let (doc_supported, doc_baseline) = parse_doc_versions(&markdown);

        // Pitfall guard: catches a silent empty-parse if the table format drifts.
        let doc_supported = doc_supported.expect(
            "parsed no `supported` version from the spec version table — table format drifted?",
        );
        let doc_baseline = doc_baseline.expect(
            "parsed no `baseline` version from the spec version table — table format drifted?",
        );

        assert_eq!(
            doc_supported, SUPPORTED_DIALECT_VERSION,
            "published `supported` dialect version and SUPPORTED_DIALECT_VERSION const have DRIFTED"
        );
        assert_eq!(
            doc_baseline, BASELINE_DIALECT_VERSION,
            "published `baseline` dialect version and BASELINE_DIALECT_VERSION const have DRIFTED"
        );
    }
}
