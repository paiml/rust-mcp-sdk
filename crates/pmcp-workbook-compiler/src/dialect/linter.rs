//! The collect-all, located linter (WBDL-03) over a synthetic [`CellSource`].
//!
//! This is the running half of the dialect contract: a **deny-by-default**
//! structural + whitelist lint that accumulates EVERY finding into one
//! [`LintReport`] (collect-all, D-01 — never fail-fast) and reports `Error`
//! severity only for genuine dialect violations (D-02).
//!
//! # The [`CellSource`] seam (decouples 93-03 from 93-02's owned cell model)
//!
//! The linter reads ONLY the [`CellSource`] abstraction — a narrow,
//! cell-iteration interface — NOT the umya-produced owned cell model (which is
//! Plan 02's concern and lands in a parallel wave). The owned model that
//! Plan 02 produces will implement [`CellSource`] in the Plan 04 wiring; HERE
//! the tests drive a hand-built [`TestCells`] double. This is the seam that
//! keeps 93-02 and 93-03 genuinely parallel.
//!
//! # The contract is consumed, never re-declared
//!
//! The whitelist + colour palette + sheet-layering all come from
//! [`pmcp_workbook_dialect`] ([`DialectRules`] / [`WHITELIST`] /
//! [`CandidateRole`], re-exported through `crate::dialect`). There is NO second
//! `WHITELIST` here — a second copy would defeat the dialect crate's
//! spec-binding drift test.
//!
//! # Collect-all discipline
//!
//! Accumulate into the report; `push`/`extend` on each finding, NEVER
//! early-return. Structural checks run before per-cell work. The whitelist match
//! is a strict closed-set check; an out-of-set token is a finding, NEVER
//! auto-widened (D-02).

use crate::dialect::{CandidateRole, DialectRules, LintFinding, LintReport, Severity};
use pmcp_workbook_runtime::range_ref::RangeRef;

/// One cell's dialect-relevant evidence, surfaced by a [`CellSource`].
///
/// This is the narrowest view the linter needs of a single cell: its address,
/// optional formula text, colour signals, the array/dynamic-array flag, and
/// whether it carries a formula. It is owned and reader-free — no umya /
/// `quick-xml` / `zip` type appears here, so the linter never re-opens a source
/// file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellView {
    /// The A1 address within the owning sheet (e.g. `"E6"`).
    pub addr: String,
    /// The formula text WITHOUT the leading `=` (e.g. `"SUM(A1:A3)"`); `None`
    /// for a value cell.
    pub formula: Option<String>,
    /// The background fill ARGB (e.g. `"FFE2EFDA"`); `None` if uncoloured.
    pub fill_argb: Option<String>,
    /// The font ARGB (e.g. `"FF0000FF"`); `None` if default.
    pub font_argb: Option<String>,
    /// True for a CSE array formula or a dynamic-array spill — refused by the
    /// dialect (non-scalar semantics).
    pub is_array_formula: bool,
}

impl CellView {
    /// True iff this cell carries a formula.
    #[must_use]
    pub fn is_formula(&self) -> bool {
        self.formula.is_some()
    }
}

/// One sheet's dialect-relevant evidence, surfaced by a [`CellSource`].
///
/// Owned and reader-free: the linter reads the sheet name, its visibility
/// state, hidden-row numbers, and its cells, never the source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetView {
    /// The sheet name (e.g. `"1_Inputs"`).
    pub name: String,
    /// The visibility state: `"visible"`, `"hidden"`, or `"veryHidden"`.
    pub state: String,
    /// The 1-based hidden-row numbers on this sheet.
    pub hidden_rows: Vec<u32>,
    /// The cells on this sheet (sparse — only populated cells).
    pub cells: Vec<CellView>,
}

/// A defined-name target a [`CellSource`] surfaces for the out-of-bounds check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinedName {
    /// The defined name (e.g. `"TaxRate"`).
    pub name: String,
    /// The range the name targets (sheet + A1 start/end).
    pub target: RangeRef,
}

/// The synthetic, reader-free cell-iteration interface the linter / parser / DAG
/// consume — the seam that keeps 93-03 parallel with 93-02 (its umya-produced
/// owned cell model will implement this trait in the Plan 04 wiring; tests here
/// use the hand-built [`TestCells`] double).
///
/// Every accessor returns owned views — no umya / `quick-xml` / `zip` type
/// crosses this boundary, so the linter can never re-open a source workbook.
/// (93-02's umya-produced owned cell model implements this trait in Plan 04.)
pub trait CellSource {
    /// Every sheet in the workbook, in workbook order.
    fn sheets(&self) -> &[SheetView];

    /// True iff the workbook carries VBA macros (refused — unverifiable code).
    fn has_macros(&self) -> bool;

    /// Each external-workbook reference text (each one is refused).
    fn external_links(&self) -> &[String];

    /// The defined names declared in the workbook (checked for out-of-bounds
    /// targets).
    fn defined_names(&self) -> &[DefinedName];

    /// The source file extension (e.g. `"xlsx"` / `"xlsm"`) — surfaced in the
    /// `structure/macro` finding message.
    fn source_extension(&self) -> &str;
}

/// Run the WORKBOOK-LEVEL refuse-set over a [`CellSource`] (no reader re-read):
/// macros, external links, and named ranges pointing outside the workbook's
/// sheet set. Returns every finding (collect-all).
///
/// - `has_macros() == true` → `structure/macro` (Error)
/// - each `external_links()` entry → `structure/external-link` (Error)
/// - a `defined_names()` target on a sheet NOT in the workbook →
///   `manifest/range-out-of-bounds` (Error)
pub fn lint_workbook_metadata(src: &dyn CellSource) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = Vec::new();

    // structure/macro — a macro-bearing workbook is refused. The compiler never
    // executes anything; macros are unverifiable code.
    if src.has_macros() {
        findings.push(LintFinding::new(
            Severity::Error,
            "structure/macro",
            String::new(),
            None,
            format!(
                "workbook carries VBA macros (source extension `{}`)",
                src.source_extension()
            ),
            "Remove all macros and save as a plain `.xlsx`; the dialect forbids macro-bearing workbooks",
        ));
    }

    // structure/external-link — each external-workbook reference is refused.
    // Located at workbook level (the reference text identifies it).
    for reference in src.external_links() {
        findings.push(LintFinding::new(
            Severity::Error,
            "structure/external-link",
            String::new(),
            None,
            format!("workbook references an external workbook: {reference}"),
            "Inline the referenced value; the dialect forbids external-workbook links",
        ));
    }

    // manifest/range-out-of-bounds — a defined name targeting a sheet that does
    // not exist in this workbook points outside the expected area.
    let sheet_names: std::collections::BTreeSet<&str> =
        src.sheets().iter().map(|s| s.name.as_str()).collect();
    for dn in src.defined_names() {
        let target_sheet = dn.target.sheet.as_str();
        if !target_sheet.is_empty() && !sheet_names.contains(target_sheet) {
            findings.push(LintFinding::new(
                Severity::Error,
                "manifest/range-out-of-bounds",
                target_sheet.to_string(),
                Some(dn.target.start.clone()),
                format!(
                    "defined name `{}` targets sheet `{}` which is not in the workbook",
                    dn.name, target_sheet
                ),
                "Point the named range at an existing layered sheet, or remove it",
            ));
        }
    }

    findings
}

/// Run the FULL collect-all lint pass over a [`CellSource`] against
/// [`DialectRules`]: workbook-level refuse-set, then per-sheet structural
/// refuse-set, then the per-cell whitelist token scan. Returns ONE
/// [`LintReport`] carrying every finding (never stops at the first; D-01).
///
/// `Error`-severity findings (an out-of-whitelist function, a macro, a hidden
/// sheet, an array formula, an out-of-bounds named range) block conformance via
/// [`LintReport::has_errors`]; `Warning`/`Info` are advisory.
pub fn lint(src: &dyn CellSource, rules: &DialectRules) -> LintReport {
    let mut report = LintReport::new();

    // 1. Workbook-level refuse-set (macros / external links / out-of-bounds names).
    report.extend(lint_workbook_metadata(src));

    // 2. Per-sheet structural refuse-set + per-cell whitelist scan.
    for sheet in src.sheets() {
        lint_sheet_structure(&mut report, sheet);
        for cell in &sheet.cells {
            lint_cell(&mut report, &sheet.name, cell, rules);
        }
    }

    report
}

/// The per-sheet structural refuse-set: hidden sheets and hidden rows.
fn lint_sheet_structure(report: &mut LintReport, sheet: &SheetView) {
    // structure/hidden-sheet — refuse BOTH Hidden and VeryHidden.
    if sheet.state == "hidden" || sheet.state == "veryHidden" {
        report.push(LintFinding::new(
            Severity::Error,
            "structure/hidden-sheet",
            sheet.name.clone(),
            None,
            format!("sheet `{}` is {} (concealed)", sheet.name, sheet.state),
            "Unhide the sheet (set it Visible) or delete it; the dialect forbids hidden/veryHidden sheets",
        ));
    }

    // structure/hidden-row — each hidden row is located by row number.
    for row in &sheet.hidden_rows {
        report.push(LintFinding::new(
            Severity::Error,
            "structure/hidden-row",
            sheet.name.clone(),
            Some(format!("{row}:{row}")),
            format!("row {row} on sheet `{}` is hidden", sheet.name),
            "Unhide the row, or move the content to a visible row; the dialect forbids hidden rows",
        ));
    }
}

/// The per-cell refuse-set: array formulas + the whitelist token scan.
fn lint_cell(report: &mut LintReport, sheet: &str, cell: &CellView, rules: &DialectRules) {
    let Some(formula) = &cell.formula else {
        return;
    };

    // formula/array — a CSE array or dynamic-array spill is refused (non-scalar
    // semantics). Reads the OWNED `is_array_formula` flag, not a re-parse.
    if cell.is_array_formula {
        report.push(LintFinding::new(
            Severity::Error,
            "formula/array",
            sheet.to_string(),
            Some(cell.addr.clone()),
            format!(
                "array formula at `{sheet}!{}` is not supported by the dialect",
                cell.addr
            ),
            "Express the calculation as scalar per-cell formulas; the dialect forbids array/spill formulas",
        ));
    }

    // whitelist/unsupported-fn — every function token in the formula text must
    // be in the whitelist (deny-by-default; never auto-widened).
    for token in extract_function_tokens(formula) {
        if !is_whitelisted(&token, rules.whitelist()) {
            report.push(LintFinding::new(
                Severity::Error,
                "whitelist/unsupported-fn",
                sheet.to_string(),
                Some(cell.addr.clone()),
                format!(
                    "function `{token}` at `{sheet}!{}` is not in the dialect whitelist",
                    cell.addr
                ),
                format!(
                    "Express it with the supported set: {}",
                    rules.whitelist().join(", ")
                ),
            ));
        }
    }
}

/// Lint COLOUR against the colour→role palette of [`DialectRules`]. For each
/// coloured cell the colour-implied [`CandidateRole`] is computed; this is the
/// evidence label a downstream synthesis maps onto a logical role. Returns an
/// `Info` finding per coloured cell (advisory only — colour proposes, never
/// decides; the manifest is canonical). Kept separate from [`lint`] so the
/// conformance gate stays purely structural + whitelist (D-02).
pub fn lint_colour_evidence(src: &dyn CellSource, rules: &DialectRules) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = Vec::new();
    for sheet in src.sheets() {
        for cell in &sheet.cells {
            let Some(candidate) = rules.candidate_role(
                cell.fill_argb.as_deref(),
                cell.font_argb.as_deref(),
                cell.is_formula(),
            ) else {
                continue;
            };
            findings.push(colour_evidence(&sheet.name, cell, candidate));
        }
    }
    findings
}

/// Build a located `colour/evidence` Info finding naming the colour signal +
/// the candidate role it implies (advisory; the manifest stays canonical).
fn colour_evidence(sheet: &str, cell: &CellView, candidate: CandidateRole) -> LintFinding {
    let evidence = cell
        .fill_argb
        .as_deref()
        .or(cell.font_argb.as_deref())
        .unwrap_or("(formula)");
    LintFinding::new(
        Severity::Info,
        "colour/evidence",
        sheet.to_string(),
        Some(cell.addr.clone()),
        format!(
            "{sheet}!{} fill/font {evidence} implies candidate role `{}`",
            cell.addr,
            candidate.label()
        ),
        "colour is advisory evidence only; the ratified manifest is canonical (D-02)",
    )
}

/// Strict closed-set whitelist membership (case-INSENSITIVE on the function
/// name, since Excel function names are case-insensitive). An out-of-set token
/// is a finding, never widened.
fn is_whitelisted(token: &str, whitelist: &[&str]) -> bool {
    whitelist.iter().any(|w| w.eq_ignore_ascii_case(token))
}

/// Extract function-name tokens from formula text WITHOUT a full parser.
///
/// An identifier `[A-Za-z][A-Za-z0-9._]*` immediately followed (modulo
/// whitespace) by `(` is a function call. A leading `_xlfn.` future-function
/// prefix is STRIPPED before the token is returned, so `_xlfn.CONCAT(` yields
/// `CONCAT`.
///
/// String literals are skipped (a `"SUM("` inside double quotes does NOT yield a
/// `SUM` token) — this mitigates the string-literal false-positive.
fn extract_function_tokens(formula: &str) -> Vec<String> {
    let chars: Vec<char> = formula.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        // Skip double-quoted string literals ("" is an escaped quote inside one).
        if c == '"' {
            i = skip_string_literal(&chars, i);
            continue;
        }

        // An identifier start: a letter or underscore.
        if c.is_ascii_alphabetic() || c == '_' {
            let (token, next) = scan_function_token(&chars, i);
            if let Some(name) = token {
                tokens.push(name);
            }
            i = next;
            continue;
        }

        i += 1;
    }

    tokens
}

/// Scan an identifier starting at `start` and decide whether it is a function
/// call (the identifier is followed — modulo whitespace — by `(`). Returns the
/// recognised function token (with any `_xlfn.` prefix stripped) when it is a
/// call, plus the index just past the identifier so the caller can resume. A
/// non-call identifier yields `None` while still advancing the cursor.
fn scan_function_token(chars: &[char], start: usize) -> (Option<String>, usize) {
    let (ident, next) = read_identifier(chars, start);
    if !followed_by_open_paren(chars, next) {
        return (None, next);
    }
    let name = strip_xlfn_prefix(ident);
    let token = if name.is_empty() { None } else { Some(name) };
    (token, next)
}

/// True when the first non-whitespace character at or after `from` is `(`.
fn followed_by_open_paren(chars: &[char], from: usize) -> bool {
    let mut j = from;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    j < chars.len() && chars[j] == '('
}

/// Skip a double-quoted string literal starting at the opening quote `start`,
/// returning the index just past the closing quote (or end of input). A doubled
/// `""` stays inside the literal.
fn skip_string_literal(chars: &[char], start: usize) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '"' {
            // An escaped "" stays inside the string.
            if i + 1 < chars.len() && chars[i + 1] == '"' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

/// Read a `[A-Za-z0-9._]` identifier run starting at `start`, returning the
/// identifier text and the index just past it.
fn read_identifier(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start;
    while i < chars.len()
        && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
    {
        i += 1;
    }
    let ident: String = chars[start..i].iter().collect();
    (ident, i)
}

/// Strip a leading `_xlfn.` future-function prefix (case-insensitive) so the
/// future-function form `_xlfn.CONCAT` compares as `CONCAT`.
/// Takes `ident` BY VALUE so the common no-prefix case keeps the `String`
/// `read_identifier` already allocated instead of copying it — the same shape
/// `formula::parser::normalize_future_function_name` uses over the same helper.
fn strip_xlfn_prefix(ident: String) -> String {
    match super::xlfn_stripped(&ident) {
        Some(stripped) => stripped.to_string(),
        None => ident,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::WHITELIST;

    /// A hand-built [`CellSource`] double — the synthetic seam that keeps this
    /// plan parallel with 93-02 (no dependency on 93-02's owned cell model).
    #[derive(Default)]
    struct TestCells {
        sheets: Vec<SheetView>,
        has_macros: bool,
        external_links: Vec<String>,
        defined_names: Vec<DefinedName>,
        source_extension: String,
    }

    impl CellSource for TestCells {
        fn sheets(&self) -> &[SheetView] {
            &self.sheets
        }
        fn has_macros(&self) -> bool {
            self.has_macros
        }
        fn external_links(&self) -> &[String] {
            &self.external_links
        }
        fn defined_names(&self) -> &[DefinedName] {
            &self.defined_names
        }
        fn source_extension(&self) -> &str {
            &self.source_extension
        }
    }

    fn formula_cell(addr: &str, formula: &str) -> CellView {
        CellView {
            addr: addr.to_string(),
            formula: Some(formula.to_string()),
            fill_argb: None,
            font_argb: None,
            is_array_formula: false,
        }
    }

    fn sheet(name: &str, cells: Vec<CellView>) -> SheetView {
        SheetView {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: Vec::new(),
            cells,
        }
    }

    #[test]
    fn extracts_simple_function_tokens() {
        let toks = extract_function_tokens("SUM(A1:A3)+IF(B1>0,1,0)");
        assert!(toks.contains(&"SUM".to_string()));
        assert!(toks.contains(&"IF".to_string()));
    }

    #[test]
    fn lowercase_if_still_matches_the_whitelist() {
        let toks = extract_function_tokens("if(a1>0,1,0)");
        assert_eq!(toks, vec!["if".to_string()]);
        assert!(is_whitelisted("if", WHITELIST));
    }

    #[test]
    fn quoted_string_does_not_raise_a_spurious_function_token() {
        // TEXT("SUM(") — the SUM( lives inside the quoted format string.
        let toks = extract_function_tokens(r#"TEXT(A1,"SUM(")"#);
        assert_eq!(toks, vec!["TEXT".to_string()]);
        assert!(!toks.contains(&"SUM".to_string()));
    }

    #[test]
    fn xlfn_prefix_is_stripped_before_comparison() {
        let toks = extract_function_tokens("_xlfn.CONCAT(A1,B1)");
        assert_eq!(toks, vec!["CONCAT".to_string()]);
        assert!(!is_whitelisted("CONCAT", WHITELIST));
    }

    #[test]
    fn out_of_whitelist_function_is_detected() {
        let toks = extract_function_tokens("OFFSET($A$1,1,1)");
        assert_eq!(toks, vec!["OFFSET".to_string()]);
        assert!(!is_whitelisted("OFFSET", WHITELIST));
    }

    #[test]
    fn the_thirteen_whitelisted_functions_are_recognised() {
        for f in WHITELIST {
            assert!(is_whitelisted(f, WHITELIST), "{f} must be whitelisted");
        }
    }

    /// Collect-all (D-01): a workbook with MULTIPLE independent violations must
    /// report EVERY one in a single pass — never fail-fast on the first.
    #[test]
    fn collect_all_reports_every_violation_in_one_pass() {
        let src = TestCells {
            sheets: vec![
                // A hidden sheet AND an out-of-whitelist function on it.
                SheetView {
                    name: "1_Inputs".to_string(),
                    state: "hidden".to_string(),
                    hidden_rows: vec![7],
                    cells: vec![formula_cell("A2", "OFFSET(B1,1,1)")],
                },
                // A second sheet with its own out-of-whitelist function.
                sheet("2_Calc", vec![formula_cell("C3", "INDIRECT(D1)")]),
            ],
            has_macros: true,
            external_links: vec!["[External.xlsx]Sheet1!A1".to_string()],
            defined_names: Vec::new(),
            source_extension: "xlsm".to_string(),
        };
        let report = lint(&src, &DialectRules::default());
        assert!(report.has_errors(), "violations must trip has_errors");

        // EVERY independent violation is present in the one report (collect-all):
        // macro, external link, hidden sheet, hidden row, two whitelist misses.
        let rules: Vec<&str> = report.findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains(&"structure/macro"));
        assert!(rules.contains(&"structure/external-link"));
        assert!(rules.contains(&"structure/hidden-sheet"));
        assert!(rules.contains(&"structure/hidden-row"));
        let whitelist_misses = report
            .findings
            .iter()
            .filter(|f| f.rule == "whitelist/unsupported-fn")
            .count();
        assert_eq!(
            whitelist_misses, 2,
            "BOTH out-of-whitelist functions report (collect-all, not fail-fast): {:?}",
            report.findings
        );
    }

    /// A conforming workbook (whitelisted fns, visible sheets, no macros) lints
    /// clean — zero errors.
    #[test]
    fn conforming_workbook_lints_clean() {
        let src = TestCells {
            sheets: vec![sheet(
                "1_Inputs",
                vec![
                    formula_cell("C11", "ROUND(C10*1.05,2)"),
                    formula_cell("C14", "IF(C11>0,SUM(A1:A3),0)"),
                ],
            )],
            source_extension: "xlsx".to_string(),
            ..Default::default()
        };
        let report = lint(&src, &DialectRules::default());
        assert!(
            !report.has_errors(),
            "a conforming workbook must lint clean: {:?}",
            report.findings
        );
    }

    #[test]
    fn array_formula_is_an_error_finding() {
        let mut cell = formula_cell("A1", "SUM(B1:B3)");
        cell.is_array_formula = true;
        let src = TestCells {
            sheets: vec![sheet("S", vec![cell])],
            source_extension: "xlsx".to_string(),
            ..Default::default()
        };
        let report = lint(&src, &DialectRules::default());
        assert!(report.findings.iter().any(|f| f.rule == "formula/array"));
    }

    #[test]
    fn out_of_bounds_defined_name_is_an_error() {
        let src = TestCells {
            sheets: vec![sheet("1_Inputs", Vec::new())],
            defined_names: vec![DefinedName {
                name: "Ghost".to_string(),
                target: RangeRef {
                    sheet: "NoSuchSheet".to_string(),
                    start: "A1".to_string(),
                    end: "A1".to_string(),
                },
            }],
            source_extension: "xlsx".to_string(),
            ..Default::default()
        };
        let report = lint(&src, &DialectRules::default());
        let f = report
            .findings
            .iter()
            .find(|f| f.rule == "manifest/range-out-of-bounds")
            .expect("an out-of-bounds finding");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.sheet, "NoSuchSheet");
    }

    #[test]
    fn colour_evidence_is_advisory_info_only() {
        let mut blue = formula_cell("E6", "C1");
        blue.formula = None;
        blue.font_argb = Some("FF0000FF".to_string()); // blue input font
        let src = TestCells {
            sheets: vec![sheet("1_Inputs", vec![blue])],
            source_extension: "xlsx".to_string(),
            ..Default::default()
        };
        let findings = lint_colour_evidence(&src, &DialectRules::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].rule, "colour/evidence");
        // Colour evidence is advisory — it never trips the conformance gate.
        let report = lint(&src, &DialectRules::default());
        assert!(!report.has_errors());
    }
}
