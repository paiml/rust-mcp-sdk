//! Reusable `#[cfg(test)]` `.xlsx` fixture author with GENUINE Excel identity
//! (WBEX-01 critical-path landmine retirement).
//!
//! # Why this module exists (Pitfall 1 — the .xlsx authoring gap)
//!
//! There is NO in-repo `.xlsx` generator. The committed `tax-calc.xlsx` is an
//! externally-authored binary blob; every fixture the WBEX gates need (the loan
//! workbook in Plan 04, the Excel-quirk corpus in Plan 05) must be authored
//! through ONE proven helper rather than re-discovering the freshness-gate recipe
//! per fixture. This module is that helper.
//!
//! # Genuine Excel identity comes for free from `rust_xlsxwriter` (verified)
//!
//! The provenance gate ([`crate::provenance::gate::classify`]) admits ONLY a
//! workbook whose `docProps/app.xml` carries an anchored
//! `<Application>Microsoft Excel</Application>` AND a positive Excel marker (an
//! `<AppVersion>` build string AND a non-sentinel calcId). `rust_xlsxwriter`
//! 0.95 hard-codes EXACTLY this identity on every save:
//!
//! - `app.rs` writes `<Application>Microsoft Excel</Application>`,
//! - `app.rs` writes `<AppVersion>12.0000</AppVersion>` (a positive marker),
//! - `workbook.rs` writes `<calcPr calcId="124519" fullCalcOnLoad="1"/>`
//!   (`124519` is NOT the umya sentinel `122211`).
//!
//! So an authored workbook classifies [`ProvenanceClass::ExcelTrusted`]
//! WITHOUT any extra [`rust_xlsxwriter::DocProperties`] call — the only residual
//! freshness problem is the `fullCalcOnLoad=1` staleness signal, which the
//! `#[cfg(test)]` trusted-fixture override DEMOTES to a Warning. A umya-authored
//! fixture would classify [`ProvenanceClass::UmyaFabricated`] and be REFUSED
//! (that is the whole point of using `rust_xlsxwriter`, not umya).
//!
//! # The cached `<v>` IS the reconcile oracle
//!
//! Every formula is written via [`rust_xlsxwriter::Formula::set_result`] so the
//! emitted `<v>` carries the AUTHORED cached value. The compiler's reconcile
//! stage grades the executor's recomputation against that cached value — so the
//! cached result is the test oracle (the same mechanism the golden gate uses).
//!
//! # Reproducible, NON-MUTATING generation (Codex HIGH)
//!
//! A normal `cargo test` run NEVER writes into `tests/fixtures/`: the self-tests
//! author into a [`tempfile::TempDir`] only. Committed `.xlsx` fixtures (the
//! loan workbook, the quirk corpus, the leap probe) are written ONLY by the
//! `#[ignore]`d, env-gated [`regenerate_fixtures`] generator below — run
//! intentionally with `PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler
//! regenerate_fixtures -- --ignored`. Alongside every committed fixture the
//! generator emits a `*.gen.json` metadata sidecar (generator fn, input cells,
//! expected output cells, override reason) so the binary `.xlsx` stays
//! reproducible and traceable.

#![cfg(test)]

use std::path::Path;

use rust_xlsxwriter::{
    Color, DataValidation, DocProperties, ExcelDateTime, Format, Formula, Table, TableColumn,
    Workbook, XlsxError,
};

use crate::provenance::gate::{classify, ProvenanceClass};
use crate::provenance::raw_parts::{read_app_props, read_calc_pr};

/// The dialect colour-role palette ARGBs (mirrors
/// `pmcp-workbook-dialect/src/lib.rs:57-58`): a blue FONT marks an INPUT, a
/// green FILL marks a governed CONSTANT. The manifest synthesis classifies a
/// cell's [`crate::Role`] from these colours alone, so an authored fixture MUST
/// paint its inputs/constants with them to be roled correctly.
pub(crate) const INPUT_FONT_ARGB: u32 = 0x0000_00FF; // FF0000FF blue font → input
pub(crate) const CONSTANT_FILL_ARGB: u32 = 0x00E2_EFDA; // FFE2EFDA green fill → constant

/// The colour role to paint an authored cell with (drives the synth
/// classification — see [`INPUT_FONT_ARGB`] / [`CONSTANT_FILL_ARGB`]).
///
// Why: `Plain`/`Constant` (and `AuthoredCell::Text` below) are the reusable
// author surface the WBEX gates consume next — the loan workbook (Plan 04) paints
// governed constants + text labels, the quirk corpus (Plan 05) uses plain cells.
// This module is the ONE proven author both gates ride; the variants are part of
// its contract, exercised by Plan 04/05's fixtures, not dead.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellPaint {
    /// No fill / no font colour — a plain label, a formula, or an output cell.
    Plain,
    /// Blue font → the synth classifies this cell `Role::Input`.
    Input,
    /// Green fill → the synth classifies this cell `Role::Constant` (governed).
    Constant,
}

/// A single authored cell: its A1 address plus its kind. The kind carries the
/// value/formula so the author writes the right `rust_xlsxwriter` call AND so the
/// generated metadata can record what each cell is.
///
// Why: the `Text` variant is reusable author surface for Plan 04's loan workbook
// (header/label cells) — part of this module's contract, not dead.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum AuthoredCell {
    /// A numeric value cell (`write_number`), painted per [`CellPaint`].
    Number {
        /// A1 address (e.g. `"A1"`).
        addr: &'static str,
        /// The numeric value to write.
        value: f64,
        /// The colour role to paint.
        paint: CellPaint,
    },
    /// A numeric value cell carrying an Excel NUMBER FORMAT (the §3.3 unit
    /// witness): a currency format (`"$#,##0"`) → USD, a percent format
    /// (`"0.0%"`) → rate. Written via `write_number_with_format` over a format
    /// that combines the paint (input/constant) with the number-format string,
    /// so the §7 template's `value` cells carry their unit for the harvest to read.
    ///
    // Why a distinct variant (not a `num_format` field on `Number`): the existing
    // quirk/loan/leap specs author bare numbers with no format and must stay
    // byte-stable; this additive variant adds the unit witness ONLY where a
    // template value cell needs it, with zero churn to the prior fixtures.
    NumberFmt {
        /// A1 address.
        addr: &'static str,
        /// The numeric value to write.
        value: f64,
        /// The colour role to paint.
        paint: CellPaint,
        /// The Excel number-format string (e.g. `"$#,##0"` currency → USD unit,
        /// `"0.0%"` percent → rate unit).
        num_format: &'static str,
    },
    /// A text/label cell (`write_string`), always [`CellPaint::Plain`].
    Text {
        /// A1 address.
        addr: &'static str,
        /// The label text.
        text: &'static str,
    },
    /// A formula cell with an AUTHORED cached result (the reconcile oracle): the
    /// `<v>` carries `cached`, set via [`Formula::set_result`].
    Formula {
        /// A1 address.
        addr: &'static str,
        /// The Excel formula text WITHOUT the leading `=` (rust_xlsxwriter adds
        /// it), e.g. `"A1*2"`.
        formula: &'static str,
        /// The authored cached result string written into `<v>` — the oracle the
        /// reconcile stage grades the recomputation against.
        cached: &'static str,
    },
}

impl AuthoredCell {
    /// The A1 address of this cell.
    fn addr(&self) -> &'static str {
        match self {
            AuthoredCell::Number { addr, .. }
            | AuthoredCell::NumberFmt { addr, .. }
            | AuthoredCell::Text { addr, .. }
            | AuthoredCell::Formula { addr, .. } => addr,
        }
    }
}

/// A `name → A1-target` defined-name (named range). The `out_*` convention drives
/// [`crate::promote_named_outputs`]; a `pmcp_dialect_version` name exercises the
/// WBDL-02 present-path. `target` is the FULLY-QUALIFIED `'Sheet'!$A$1` form
/// `rust_xlsxwriter::Workbook::define_name` expects.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DefinedNameSpec {
    /// The defined-name identifier (e.g. `"out_result"`).
    pub(crate) name: &'static str,
    /// The fully-qualified single-cell target (e.g. `"'Calc'!$B$1"`).
    pub(crate) target: &'static str,
}

/// The kind of an authored data-validation (the enum/governance dropdown
/// source the harvest reads per §3.3). `List` dogfoods the enum-from-dropdown
/// mechanism: the tier column `{variable, strict}` AND a sample enum dropdown
/// are both authored as `List` validations.
///
// Why: `List` is the only kind the WBV2 template needs today (tier + enum
// dropdowns). A future range/decimal validation is a trivial additive variant;
// the enum keeps the author surface honest about what it emits.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DvKind {
    /// An inline-literal list dropdown — `rust_xlsxwriter`
    /// `DataValidation::allow_list_strings(values)`. Reads back with
    /// `dv_type == "list"`.
    List(&'static [&'static str]),
}

/// A single authored data-validation: the A1 range it covers and its kind.
/// `range` is a simple `A1:A1` / `A2:A3` form (the only shapes the WBV2 template
/// uses). Authored via `worksheet.add_data_validation(...)` over the parsed
/// range corners.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DataValidationSpec {
    /// The A1 range the validation applies to (e.g. `"D2:D3"`).
    pub(crate) range: &'static str,
    /// The validation kind (e.g. a `{variable, strict}` list dropdown).
    pub(crate) kind: DvKind,
}

/// An authored Excel Table (ListObject) — the §3 declaration primitive. The
/// table `name` is the served key surface (an Inputs table, or a tool name for
/// an output table); `columns` are the standard header names
/// (`name|value|description[|tier]`); `caption`, when present, is written in the
/// cell DIRECTLY ABOVE the header row (the tool description per §3.2); `rows`
/// are the body cells (reusing [`AuthoredCell`] so formula cells keep the
/// cached-`<v>` oracle); `data_validations` carry the tier/enum dropdowns.
//
// `sheet` is now LOAD-BEARING: the multi-sheet [`author_multi_sheet_xlsx`] consults
// it (via `write_sheet_contents`' `debug_assert_eq!`) to confirm each table is
// authored onto the worksheet it declares. The single-worksheet [`author_xlsx`]
// still writes every table onto the one [`WorkbookSpec::sheet`] it names (where the
// assertion is a tautology), so existing single-sheet fixtures are byte-unchanged.
#[derive(Debug, Clone)]
pub(crate) struct TableSpec {
    /// The ListObject / tool name (e.g. `"Inputs"`, `"Calculate_Tax"`).
    pub(crate) name: &'static str,
    /// The worksheet the table lives on (consulted by the multi-sheet author).
    pub(crate) sheet: &'static str,
    /// The zero-based `(col, row)` of the table's HEADER top-left corner.
    pub(crate) top_left: (u16, u32),
    /// The standard column header names (e.g. `["name","value","description","tier"]`).
    pub(crate) columns: &'static [&'static str],
    /// The caption written directly above the header row (the tool description),
    /// or `None` for an input table.
    pub(crate) caption: Option<&'static str>,
    /// The table body rows (each a row of [`AuthoredCell`]s) written by the table
    /// path. May be empty when the body cells are authored via
    /// [`WorkbookSpec::cells`] instead (the template path) — in that case
    /// [`TableSpec::body_rows`] declares the body height so the ListObject area
    /// still spans the body.
    pub(crate) rows: Vec<Vec<AuthoredCell>>,
    /// The number of BODY rows below the header (the table area spans
    /// `header_row ..= header_row + body_rows`). Defaults to `rows.len()` when the
    /// body is authored inline via [`TableSpec::rows`]; set explicitly when the
    /// body cells live in [`WorkbookSpec::cells`] (template path).
    pub(crate) body_rows: u32,
    /// The tier/enum dropdowns over value cells.
    pub(crate) data_validations: Vec<DataValidationSpec>,
}

/// A reusable workbook author spec consumed by [`author_xlsx`]. One sheet of
/// authored cells plus its workbook-global defined names. (A single sheet covers
/// every fixture this phase needs; a multi-sheet author is a trivial extension if
/// a later plan needs cross-sheet refs.)
#[derive(Debug, Clone)]
pub(crate) struct WorkbookSpec {
    /// The single worksheet name.
    pub(crate) sheet: &'static str,
    /// The authored cells on that sheet.
    pub(crate) cells: Vec<AuthoredCell>,
    /// The workbook-global defined names (named ranges).
    pub(crate) defined_names: Vec<DefinedNameSpec>,
    /// The authored Excel Tables (ListObjects) — the §3 declaration primitive.
    pub(crate) tables: Vec<TableSpec>,
}

/// Author a `.xlsx` at `path` from `spec` using `rust_xlsxwriter` (a pure
/// writer). The saved workbook carries a GENUINE Excel identity (so it classifies
/// [`ProvenanceClass::ExcelTrusted`]) and every formula carries its authored
/// cached `<v>` (the reconcile oracle).
///
/// # Errors
/// Returns the underlying [`XlsxError`] on any write/save failure (test path —
/// the caller `.expect`s it).
pub(crate) fn author_xlsx(path: &Path, spec: &WorkbookSpec) -> Result<(), XlsxError> {
    let palette = CellFormats::new();

    let mut workbook = new_deterministic_workbook();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(spec.sheet)?;
    write_sheet_contents(worksheet, spec.sheet, &spec.cells, &spec.tables, &palette)?;

    for dn in &spec.defined_names {
        workbook.define_name(dn.name, dn.target)?;
    }

    workbook.save(path)?;
    Ok(())
}

/// One worksheet of a [`MultiSheetSpec`]: its name plus its loose cells and Excel
/// Tables. The per-sheet content shape mirrors the single-sheet [`WorkbookSpec`]
/// (cells + tables); workbook-global defined names live on the [`MultiSheetSpec`].
///
/// Cell/table A1 addresses are LOCAL to this sheet — `author_multi_sheet_xlsx`
/// writes them onto THIS sheet, so a fixture can carry the SAME `B2` on two sheets
/// and a formula on one sheet can reach the other via a cross-sheet `'Sheet'!B2`.
#[derive(Debug, Clone)]
pub(crate) struct SheetSpec {
    /// The worksheet name (e.g. `"Data"`, `"Aux"`).
    pub(crate) name: &'static str,
    /// The loose authored cells on this sheet (non-table cells).
    pub(crate) cells: Vec<AuthoredCell>,
    /// The authored Excel Tables (ListObjects) on this sheet.
    pub(crate) tables: Vec<TableSpec>,
}

/// A MULTI-SHEET workbook author spec (the additive sibling of [`WorkbookSpec`],
/// consumed by [`author_multi_sheet_xlsx`]). Each [`SheetSpec`] is written onto its
/// own worksheet IN ORDER, honouring the per-cell/per-table sheet so a fixture can
/// carry a cross-sheet reference (an input on a SECOND sheet reached only via
/// `'Sheet'!Bx`) and a `SUM(range)`-reached input on the first.
///
/// Defined names are workbook-global (the `target` carries its own `'Sheet'!$A$1`
/// qualification), exactly as [`WorkbookSpec::defined_names`].
#[derive(Debug, Clone)]
pub(crate) struct MultiSheetSpec {
    /// The worksheets, written in order (the first is the primary sheet).
    pub(crate) sheets: Vec<SheetSpec>,
    /// The workbook-global defined names (named ranges), fully `'Sheet'!$A$1`-qualified.
    pub(crate) defined_names: Vec<DefinedNameSpec>,
}

/// Author a MULTI-SHEET `.xlsx` at `path` from `spec` using `rust_xlsxwriter` (a
/// pure writer) — the additive sibling of [`author_xlsx`] for fixtures that need a
/// `SUM(range)` input AND a cross-sheet-reached input. Each [`SheetSpec`] becomes
/// its own worksheet, honouring its per-cell/per-table sheet (the single-sheet
/// author wrote every table onto the one [`WorkbookSpec::sheet`]; this consults the
/// sheet so a cross-sheet ref resolves to a REAL second worksheet).
///
/// Carries the SAME genuine Excel identity ([`ProvenanceClass::ExcelTrusted`]) and
/// byte-deterministic creation datetime as [`author_xlsx`] (shared helpers), so a
/// multi-sheet fixture stays as reproducible + trusted as the single-sheet corpus.
///
/// # Errors
/// Returns the underlying [`XlsxError`] on any write/save failure (test path —
/// the caller `.expect`s it).
pub(crate) fn author_multi_sheet_xlsx(path: &Path, spec: &MultiSheetSpec) -> Result<(), XlsxError> {
    let palette = CellFormats::new();
    let mut workbook = new_deterministic_workbook();

    for sheet in &spec.sheets {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet.name)?;
        write_sheet_contents(worksheet, sheet.name, &sheet.cells, &sheet.tables, &palette)?;
    }

    for dn in &spec.defined_names {
        workbook.define_name(dn.name, dn.target)?;
    }

    workbook.save(path)?;
    Ok(())
}

/// Build a fresh [`Workbook`] with the PINNED creation datetime so regeneration is
/// byte-deterministic: the default `DocProperties` stamps `ExcelDateTime::utc_now()`
/// into `docProps/core.xml`, which would make every regen produce different bytes
/// (the reproducible-fixture discipline requires deterministic output). This touches
/// core.xml ONLY — the provenance gate reads app.xml's `<Application>`/`<AppVersion>`
/// and the calcPr, so the ExcelTrusted identity is untouched. Shared by the
/// single-sheet [`author_xlsx`] and the multi-sheet [`author_multi_sheet_xlsx`] so
/// BOTH carry the identical deterministic identity.
fn new_deterministic_workbook() -> Workbook {
    let mut workbook = Workbook::new();
    let fixed_creation =
        ExcelDateTime::from_ymd(2026, 1, 1).expect("fixed fixture creation date is valid");
    workbook.set_properties(&DocProperties::new().set_creation_datetime(&fixed_creation));
    workbook
}

/// Write one worksheet's loose cells then its Excel Tables (the shared per-sheet
/// body the single-sheet and multi-sheet authors both run). Kept separate so each
/// author stays a thin orchestrator (PMAT cog ≤25).
fn write_sheet_contents(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    sheet_name: &str,
    cells: &[AuthoredCell],
    tables: &[TableSpec],
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    for cell in cells {
        write_cell(worksheet, cell, palette)?;
    }
    for table in tables {
        debug_assert_eq!(
            table.sheet, sheet_name,
            "a TableSpec's `sheet` must match the worksheet it is authored onto"
        );
        write_table(worksheet, table, palette)?;
    }
    Ok(())
}

/// Author one Excel Table (ListObject) onto `worksheet`: write its caption (if
/// any) directly above the header, write the body cells, overlay the
/// `rust_xlsxwriter` [`Table`] with its name + column headers, then add its
/// data-validation dropdowns. One helper per concern keeps cognitive complexity
/// low (PMAT cog ≤25).
fn write_table(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    spec: &TableSpec,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    let (first_col, header_row) = spec.top_left;
    write_caption(worksheet, spec, first_col, header_row)?;
    write_table_body(worksheet, spec, palette)?;
    // The body height: an explicit `body_rows` (template path, body in
    // `WorkbookSpec::cells`) else the inline `rows.len()` (self-test path). A
    // table always spans at least the header + one body row.
    let body_rows = spec
        .body_rows
        .max(u32::try_from(spec.rows.len()).expect("row count fits a u32"));
    let last_row = header_row + body_rows;
    let last_col = first_col + col_span(spec.columns);
    let table = build_table(spec);
    worksheet.add_table(header_row, first_col, last_row, last_col, &table)?;
    write_data_validations(worksheet, spec)?;
    Ok(())
}

/// Write the caption string (the tool description) in the cell directly above
/// the table's header row, when the spec carries one.
fn write_caption(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    spec: &TableSpec,
    first_col: u16,
    header_row: u32,
) -> Result<(), XlsxError> {
    if let (Some(caption), Some(caption_row)) = (spec.caption, header_row.checked_sub(1)) {
        worksheet.write_string(caption_row, first_col, caption)?;
    }
    Ok(())
}

/// Write the table body cells (each row reuses [`write_cell`] so formula cells
/// keep their cached-`<v>` oracle and numbers keep their paint).
fn write_table_body(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    spec: &TableSpec,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    for row in &spec.rows {
        for cell in row {
            write_cell(worksheet, cell, palette)?;
        }
    }
    Ok(())
}

/// Build the `rust_xlsxwriter` [`Table`] for `spec`: its ListObject name plus one
/// [`TableColumn`] per standard header.
fn build_table(spec: &TableSpec) -> Table {
    let columns: Vec<TableColumn> = spec
        .columns
        .iter()
        .map(|h| TableColumn::new().set_header(*h))
        .collect();
    Table::new().set_name(spec.name).set_columns(&columns)
}

/// Add the tier/enum dropdown data-validations over their A1 ranges.
fn write_data_validations(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    spec: &TableSpec,
) -> Result<(), XlsxError> {
    for dv in &spec.data_validations {
        let (first, last) = parse_a1_range(dv.range);
        let validation = match dv.kind {
            DvKind::List(values) => DataValidation::new().allow_list_strings(values)?,
        };
        worksheet.add_data_validation(first.0, first.1, last.0, last.1, &validation)?;
    }
    Ok(())
}

/// The zero-based column span of a header list, i.e. `len - 1` (the last column
/// offset). A table always carries ≥1 column.
fn col_span(columns: &[&str]) -> u16 {
    u16::try_from(columns.len().saturating_sub(1)).expect("column span fits a u16")
}

/// Parse a simple `A1:B2` range into `((row,col), (row,col))` zero-based corners
/// for `rust_xlsxwriter`. Test-only: a malformed range is a fixture-author bug
/// and panics.
fn parse_a1_range(range: &str) -> ((u32, u16), (u32, u16)) {
    let (start, end) = range
        .split_once(':')
        .expect("authored range is `A1:B2` form");
    (parse_a1(start), parse_a1(end))
}

/// The two paint-driven cell formats authored fixtures use: the INPUT font
/// colour and the CONSTANT fill colour. Built once per [`author_xlsx`] call.
struct CellFormats {
    input: Format,
    constant: Format,
}

impl CellFormats {
    fn new() -> Self {
        Self {
            input: Format::new().set_font_color(Color::RGB(INPUT_FONT_ARGB)),
            constant: Format::new().set_background_color(Color::RGB(CONSTANT_FILL_ARGB)),
        }
    }
}

/// Write a single authored cell to `worksheet` at its A1 address, dispatching on
/// the cell kind. Numbers carry their paint-driven format; formulas carry their
/// authored cached `<v>` oracle. Surfaces any underlying [`XlsxError`].
fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    cell: &AuthoredCell,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    let (row, col) = parse_a1(cell.addr());
    match cell {
        AuthoredCell::Number { value, paint, .. } => {
            write_number_cell(worksheet, row, col, *value, *paint, palette)
        },
        AuthoredCell::NumberFmt {
            value,
            paint,
            num_format,
            ..
        } => {
            let format = paint_format(*paint).set_num_format(*num_format);
            worksheet
                .write_number_with_format(row, col, *value, &format)
                .map(|_| ())
        },
        AuthoredCell::Text { text, .. } => worksheet.write_string(row, col, *text).map(|_| ()),
        AuthoredCell::Formula {
            formula, cached, ..
        } => {
            let f = Formula::new(*formula).set_result(*cached);
            worksheet.write_formula(row, col, f).map(|_| ())
        },
    }
}

/// Build the paint-driven base [`Format`] for a [`CellPaint`] (a blue INPUT
/// font, a green CONSTANT fill, or a plain format). The [`AuthoredCell::NumberFmt`]
/// path then layers its number-format string on top via `set_num_format`.
fn paint_format(paint: CellPaint) -> Format {
    match paint {
        CellPaint::Input => Format::new().set_font_color(Color::RGB(INPUT_FONT_ARGB)),
        CellPaint::Constant => Format::new().set_background_color(Color::RGB(CONSTANT_FILL_ARGB)),
        CellPaint::Plain => Format::new(),
    }
}

/// Write a numeric cell, selecting the paint-driven format (INPUT font colour,
/// CONSTANT fill, or PLAIN no-format).
fn write_number_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: f64,
    paint: CellPaint,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    match paint {
        CellPaint::Input => worksheet
            .write_number_with_format(row, col, value, &palette.input)
            .map(|_| ()),
        CellPaint::Constant => worksheet
            .write_number_with_format(row, col, value, &palette.constant)
            .map(|_| ()),
        CellPaint::Plain => worksheet.write_number(row, col, value).map(|_| ()),
    }
}

/// Parse a simple `A1`-style address into zero-based `(row, col)` for
/// `rust_xlsxwriter`. Supports single/multi-letter columns and 1-based rows
/// (the only forms the authored fixtures use). Test-only: a malformed address is
/// a fixture-author bug and panics.
fn parse_a1(addr: &str) -> (u32, u16) {
    let split = addr
        .find(|c: char| c.is_ascii_digit())
        .expect("authored A1 address has a row digit");
    let (col_letters, row_digits) = addr.split_at(split);
    let mut col: u32 = 0;
    for ch in col_letters.chars() {
        let v = (ch.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        col = col * 26 + v;
    }
    let col = u16::try_from(col - 1).expect("authored column fits a u16");
    let row: u32 = row_digits.parse::<u32>().expect("authored row is numeric") - 1;
    (row, col)
}

/// Write the paired `*.provenance-override.json` trusted-fixture marker beside
/// `xlsx_path` (clone of `tax-calc.provenance-override.json`, retargeting
/// `fixture` to the authored file's name). Returns the marker path.
///
/// # Errors
/// Returns the underlying I/O / serialization error on failure.
pub(crate) fn write_override_marker(xlsx_path: &Path, reason: &str) -> Result<(), std::io::Error> {
    let file_name = xlsx_path
        .file_name()
        .and_then(|s| s.to_str())
        .expect("xlsx path has a UTF-8 file name");
    let marker_path = xlsx_path.with_extension("provenance-override.json");
    let marker = serde_json::json!({
        "kind": "trusted-fixture",
        "fixture": file_name,
        "reason": reason,
        "authored_by": "rust_xlsxwriter",
        "scope": "test-path-only",
    });
    let body = serde_json::to_string_pretty(&marker)?;
    std::fs::write(marker_path, body)
}

/// Write the GENERATION METADATA sidecar (`*.gen.json`) beside `xlsx_path`
/// (Codex suggestion): the generator fn name, the input cells, the expected
/// output cells, and the override reason — so a committed binary `.xlsx` is
/// reproducible and traceable to the exact author call that produced it.
///
/// # Errors
/// Returns the underlying I/O / serialization error on failure.
pub(crate) fn write_gen_metadata(
    xlsx_path: &Path,
    generator_fn: &str,
    spec: &WorkbookSpec,
    override_reason: &str,
) -> Result<(), std::io::Error> {
    let inputs: Vec<&str> = spec
        .cells
        .iter()
        .filter_map(|c| match c {
            AuthoredCell::Number {
                addr,
                paint: CellPaint::Input,
                ..
            }
            | AuthoredCell::NumberFmt {
                addr,
                paint: CellPaint::Input,
                ..
            } => Some(*addr),
            _ => None,
        })
        .collect();
    let expected_outputs: Vec<serde_json::Value> = spec
        .defined_names
        .iter()
        .filter(|d| d.name.starts_with("out_"))
        .map(|d| serde_json::json!({ "name": d.name, "target": d.target }))
        .collect();
    let formulas: Vec<serde_json::Value> = spec
        .cells
        .iter()
        .filter_map(|c| match c {
            AuthoredCell::Formula {
                addr,
                formula,
                cached,
            } => Some(serde_json::json!({
                "cell": addr, "formula": formula, "cached_oracle": cached,
            })),
            _ => None,
        })
        .collect();
    let meta = serde_json::json!({
        "generator_fn": generator_fn,
        "authored_by": "rust_xlsxwriter",
        "sheet": spec.sheet,
        "input_cells": inputs,
        "formula_oracles": formulas,
        "expected_outputs": expected_outputs,
        "override_reason": override_reason,
    });
    let gen_path = xlsx_path.with_extension("gen.json");
    let body = serde_json::to_string_pretty(&meta)?;
    std::fs::write(gen_path, body)
}

/// The provenance class of an authored `.xlsx`, read the SAME way the production
/// gate reads it: parse `docProps/app.xml` + `xl/workbook.xml` from the ORIGINAL
/// on-disk bytes via the quarantined raw reader, then [`classify`]. This is the
/// DIRECT provenance assertion target (Codex HIGH) — a self-test calls this and
/// asserts `== ExcelTrusted`, never inferring trust from compile success.
pub(crate) fn classify_authored(bytes: &[u8]) -> ProvenanceClass {
    let app = read_app_props(bytes).expect("authored .xlsx has a readable app.xml");
    let calc = read_calc_pr(bytes).expect("authored .xlsx has a readable calcPr");
    classify(
        app.application.as_deref(),
        app.app_version.as_deref(),
        calc.calc_id,
    )
}

/// A trivial 1-formula spec: a blue-font input `A1`, a formula `B1 = A1*2` with an
/// authored cached oracle, and an `out_result` named range targeting `B1`. The
/// canonical smoke-test workbook the self-tests author into a TempDir.
fn trivial_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Calc",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 10.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1*2",
                cached: "20",
            },
        ],
        defined_names: vec![
            // F1: every Role::Input cell needs an `in_*` named range so it gets a
            // callable semantic key (otherwise compile now fails loud).
            DefinedNameSpec {
                name: "in_a1",
                target: "'Calc'!$A$1",
            },
            DefinedNameSpec {
                name: "out_result",
                target: "'Calc'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// REPRODUCIBLE, NON-MUTATING fixture generator (Codex HIGH). This is the ONLY
/// path that writes committed `.xlsx` fixtures into the source tree, and it is
/// `#[ignore]`d + env-gated so a normal `cargo test` run NEVER mutates
/// `tests/fixtures/`. Run intentionally with:
///
/// ```text
/// PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler \
///     regenerate_fixtures -- --ignored
/// ```
///
/// Without the `PMCP_REGEN_FIXTURES` env var the body is a no-op even if invoked
/// directly with `--ignored`, so it can never silently rewrite tracked fixtures.
/// Plans 04 (loan workbook) and 05 (quirk corpus + the leap probe) extend the
/// `targets` list below; each committed fixture is paired with its
/// `*.provenance-override.json` marker and `*.gen.json` metadata sidecar.
#[test]
#[ignore = "writes committed fixtures into the source tree; run only with PMCP_REGEN_FIXTURES=1"]
fn regenerate_fixtures() {
    if std::env::var("PMCP_REGEN_FIXTURES").is_err() {
        eprintln!(
            "[regenerate_fixtures] PMCP_REGEN_FIXTURES not set — no-op (the generator never \
             mutates tests/fixtures/ on a normal run)"
        );
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixtures dir");

    // The leap-year probe (Plan 96-03 Task 2): serial-arithmetic over f64 with
    // whitelisted ops only — see SPIKE-1900-leap.md for the disposition.
    let probe = leap1900_probe_spec();
    let probe_path = fixtures.join("leap1900-probe.xlsx");
    let probe_reason = "authored by rust_xlsxwriter; the 1900-leap serial offset encoded as \
                        whitelisted f64 arithmetic (NO date function) — honoured ONLY on the \
                        test/dev path. Production still refuses non-fresh provenance.";
    author_xlsx(&probe_path, &probe).expect("author leap1900 probe");
    write_override_marker(&probe_path, probe_reason).expect("probe marker");
    write_gen_metadata(&probe_path, "leap1900_probe_spec", &probe, probe_reason)
        .expect("probe gen metadata");
    eprintln!("[regenerate_fixtures] wrote {}", probe_path.display());

    // The loan/mortgage rate-tier calculator (Plan 96-04 Task 1 — the WBEX-01
    // generalization fixture): a fixed-cell DAG whose divergence from tax-calc
    // comes ENTIRELY from whitelist-legal lookup families (VLOOKUP +
    // INDEX/MATCH against a rate-tier table, IFERROR guards, nested-IF tiering,
    // ROUND/CEILING to currency) — NO PMT/POWER/exponentiation (D-02). Fully
    // synthetic (zero customer/TowelRads material).
    let loan = loan_calc_spec();
    let loan_path = fixtures.join("loan-calc.xlsx");
    let loan_reason = "authored by rust_xlsxwriter; a synthetic loan/mortgage rate-tier \
                       calculator (the WBEX-01 second-workbook generalization fixture). \
                       Whitelist-legal (VLOOKUP/INDEX-MATCH/IFERROR/nested-IF/ROUND/CEILING, \
                       NO PMT/POWER) — honoured ONLY on the test/dev path. Production still \
                       refuses non-fresh provenance.";
    author_xlsx(&loan_path, &loan).expect("author loan-calc fixture");
    write_override_marker(&loan_path, loan_reason).expect("loan marker");
    write_gen_metadata(&loan_path, "loan_calc_spec", &loan, loan_reason)
        .expect("loan gen metadata");
    eprintln!("[regenerate_fixtures] wrote {}", loan_path.display());

    // The WBEX-02 Excel-quirk reconcile corpus (Plan 96-05 Task 2): each tiny
    // workbook encodes ONE numerically-expressible Excel quirk as a whitelisted
    // formula DAG with a cached `<v>` oracle, so the quirks_reconcile harness can
    // grade the executor's recomputation against the oracle through the REAL
    // penny-reconcile path. (The 1900-leap reconcile fixture is the committed
    // `leap1900-probe.xlsx` above — Plan 96-03 disposition A — so it is NOT
    // re-authored here; this generator emits only the six NON-leap reconcile
    // fixtures.) Authored into the quirks/ subdir, each with its override marker
    // and gen-metadata sidecar.
    let quirks_dir = fixtures.join("quirks");
    std::fs::create_dir_all(&quirks_dir).expect("quirks fixtures dir");
    for (file_stem, generator_fn, spec) in quirk_reconcile_specs() {
        let path = quirks_dir.join(format!("{file_stem}.xlsx"));
        let reason = "authored by rust_xlsxwriter; a WBEX-02 Excel-quirk reconcile fixture \
                      (one quirk, whitelisted formula DAG, cached <v> oracle) — honoured ONLY \
                      on the test/dev path. Production still refuses non-fresh provenance.";
        author_xlsx(&path, &spec).expect("author quirk fixture");
        write_override_marker(&path, reason).expect("quirk marker");
        write_gen_metadata(&path, generator_fn, &spec, reason).expect("quirk gen metadata");
        eprintln!("[regenerate_fixtures] wrote {}", path.display());
    }

    // The WR-01 HARDENING range+cross-sheet fixture (Plan 100-08-HARDENING): a REAL
    // multi-sheet, Table-authored workbook whose output `total` reaches q1/q2 ONLY via
    // SUM(B2:B3) and `adjustment` ONLY via the cross-sheet Aux!B2 ref. Committed so the
    // cargo-pmcp `workbook_explain` integration test can drive `explain_workbook` over
    // it (the explain CLI reads a real .xlsx via the read-only Preview policy — no
    // override marker needed there; the marker below arms the compiler override path).
    let rx = range_cross_sheet_spec();
    let rx_path = fixtures.join("range-cross-sheet.xlsx");
    let rx_reason = "authored by rust_xlsxwriter; the WR-01 hardening multi-sheet fixture (a \
                     SUM(range)-reached input AND a cross-sheet-reached input through a REAL \
                     compile) — honoured ONLY on the test/dev path. Production still refuses \
                     non-fresh provenance.";
    author_multi_sheet_xlsx(&rx_path, &rx).expect("author the range+cross-sheet fixture");
    write_override_marker(&rx_path, rx_reason).expect("range+cross-sheet marker");
    write_multi_sheet_gen_metadata(&rx_path, "range_cross_sheet_spec", &rx, rx_reason)
        .expect("range+cross-sheet gen metadata");
    eprintln!("[regenerate_fixtures] wrote {}", rx_path.display());
}

/// Write a generation-metadata sidecar (`*.gen.json`) for a MULTI-SHEET fixture (the
/// [`MultiSheetSpec`] analogue of [`write_gen_metadata`]): records the generator fn,
/// the per-sheet input cells, the formula oracles, and the override reason so a
/// committed multi-sheet `.xlsx` stays reproducible + traceable.
///
/// # Errors
/// Returns the underlying I/O / serialization error on failure.
pub(crate) fn write_multi_sheet_gen_metadata(
    xlsx_path: &Path,
    generator_fn: &str,
    spec: &MultiSheetSpec,
    override_reason: &str,
) -> Result<(), std::io::Error> {
    let sheets: Vec<serde_json::Value> = spec
        .sheets
        .iter()
        .map(|sheet| {
            let cells: Vec<&AuthoredCell> = sheet
                .tables
                .iter()
                .flat_map(|t| t.rows.iter().flatten())
                .chain(sheet.cells.iter())
                .collect();
            let inputs: Vec<&str> = cells
                .iter()
                .filter_map(|c| match c {
                    AuthoredCell::Number {
                        addr,
                        paint: CellPaint::Input,
                        ..
                    }
                    | AuthoredCell::NumberFmt {
                        addr,
                        paint: CellPaint::Input,
                        ..
                    } => Some(*addr),
                    _ => None,
                })
                .collect();
            let formulas: Vec<serde_json::Value> = cells
                .iter()
                .filter_map(|c| match c {
                    AuthoredCell::Formula {
                        addr,
                        formula,
                        cached,
                    } => Some(serde_json::json!({
                        "cell": addr, "formula": formula, "cached_oracle": cached,
                    })),
                    _ => None,
                })
                .collect();
            serde_json::json!({
                "sheet": sheet.name,
                "input_cells": inputs,
                "formula_oracles": formulas,
            })
        })
        .collect();
    let meta = serde_json::json!({
        "generator_fn": generator_fn,
        "authored_by": "rust_xlsxwriter",
        "multi_sheet": true,
        "sheets": sheets,
        "override_reason": override_reason,
    });
    let gen_path = xlsx_path.with_extension("gen.json");
    let body = serde_json::to_string_pretty(&meta)?;
    std::fs::write(gen_path, body)
}

/// REPRODUCIBLE, NON-MUTATING generator for the WBV2-01 shipped `template.xlsx`
/// (§7), the env-gated sibling of [`regenerate_fixtures`]. It authors the
/// CANONICAL template once under the CLI templates dir
/// (`cargo-pmcp/src/templates/workbook_bundle/template.xlsx`), then copies the
/// produced bytes BYTE-FOR-BYTE into the compiler test-fixtures dir
/// (`crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx`) — ONE generator,
/// two identical copies (review finding #8). A `template.gen.json` sidecar is
/// written beside the canonical copy for reproducibility. NO
/// `template.provenance-override.json` is written: the template is genuinely
/// `rust_xlsxwriter`-authored and classifies RAW
/// [`ProvenanceClass::ExcelTrusted`] (review finding #5).
///
/// Run intentionally with:
///
/// ```text
/// PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler \
///     regenerate_template -- --ignored
/// ```
#[test]
#[ignore = "writes the committed template.xlsx into the source tree; run only with PMCP_REGEN_FIXTURES=1"]
fn regenerate_template() {
    if std::env::var("PMCP_REGEN_FIXTURES").is_err() {
        eprintln!(
            "[regenerate_template] PMCP_REGEN_FIXTURES not set — no-op (the generator never \
             mutates the committed template on a normal run)"
        );
        return;
    }

    // CARGO_MANIFEST_DIR = crates/pmcp-workbook-compiler; the workspace root is two
    // levels up. The canonical template lives under the CLI templates dir; the
    // test-fixtures copy lives in this crate's tests/fixtures.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above the compiler crate");
    let cli_templates = workspace_root.join("cargo-pmcp/src/templates/workbook_bundle");
    std::fs::create_dir_all(&cli_templates).expect("CLI templates dir");
    let fixtures = crate_dir.join("tests/fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixtures dir");

    let spec = template_spec();
    let canonical = cli_templates.join("template.xlsx");
    author_xlsx(&canonical, &spec).expect("author the canonical template.xlsx");

    // The .gen.json reproducibility sidecar beside the canonical copy. NO
    // provenance-override sidecar — the template is RAW ExcelTrusted.
    let gen_reason =
        "authored by rust_xlsxwriter; the WBV2-01 shipped tax-suite template — the BA \
                      starting point + training artifact + honest reference fixture (§7). \
                      Classifies RAW ExcelTrusted with NO provenance-override sidecar.";
    write_gen_metadata(&canonical, "template_spec", &spec, gen_reason)
        .expect("template gen metadata");
    eprintln!("[regenerate_template] wrote {}", canonical.display());

    // Copy the canonical bytes BYTE-FOR-BYTE into the test-fixtures dir (review
    // finding #8: one generator, two identical copies — the byte-equality test
    // fails CI on any drift between them).
    let fixture_copy = fixtures.join("template.xlsx");
    std::fs::copy(&canonical, &fixture_copy).expect("copy template to fixtures dir");
    eprintln!("[regenerate_template] copied to {}", fixture_copy.display());
}

/// The WBEX-02 quirk reconcile-fixture specs (Plan 96-05 Task 2): each is a tiny
/// single-formula DAG encoding ONE numerically-expressible Excel quirk, with the
/// cached `<v>` carrying the Excel oracle the penny-reconcile harness grades the
/// recomputation against. The returned tuples are `(file_stem, generator_fn_name,
/// spec)` for the env-gated generator above. (The 1900-leap reconcile fixture is
/// the committed `leap1900-probe.xlsx` — Plan 96-03 disposition A — so it is not
/// in this list; the four NAMED roadmap quirks are covered as: 1900-leap = the
/// probe; half-rounding = `quirk-half-rounding`; empty-cell coercion =
/// `quirk-empty-coercion`; error/`#DIV/0!` propagation = the scalar_eval layer +
/// the SPIKE-documented runtime limitation, see `quirks_reconcile.rs`.)
pub(crate) fn quirk_reconcile_specs() -> Vec<(&'static str, &'static str, WorkbookSpec)> {
    vec![
        (
            "quirk-half-rounding",
            "quirk_half_rounding_spec",
            quirk_half_rounding_spec(),
        ),
        (
            "quirk-negative-rounding",
            "quirk_negative_rounding_spec",
            quirk_negative_rounding_spec(),
        ),
        (
            "quirk-empty-coercion",
            "quirk_empty_coercion_spec",
            quirk_empty_coercion_spec(),
        ),
        (
            "quirk-float-boundary",
            "quirk_float_boundary_spec",
            quirk_float_boundary_spec(),
        ),
        (
            "quirk-text-coercion",
            "quirk_text_coercion_spec",
            quirk_text_coercion_spec(),
        ),
    ]
}

/// QUIRK: half-rounding boundary. `ROUND(1594.925, 2)` is `1594.93` (half away
/// from zero), NOT the naive binary-f64 `1594.92` — the executor's `excel_round`
/// applies the boundary epsilon. {formula `ROUND(A1,2)`, context: ROUND of a
/// decimal-half input; oracle `1594.93`; reconcile key `out_rounded` → B1}.
fn quirk_half_rounding_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 1594.925,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "ROUND(A1,2)",
                cached: "1594.93",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "out_rounded",
                target: "'Quirk'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// QUIRK: negative-value rounding sign. `ROUND(-2.5, 0)` is `-3` (Excel rounds
/// half AWAY FROM ZERO, sign preserved), NOT `-2`. {formula `ROUND(A1,0)`,
/// context: ROUND of a negative half; oracle `-3`; reconcile key `out_rounded`
/// → B1}.
fn quirk_negative_rounding_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: -2.5,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "ROUND(A1,0)",
                cached: "-3",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "out_rounded",
                target: "'Quirk'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// QUIRK: empty-cell coercion (named roadmap quirk). An EMPTY cell coerces to 0
/// in additive arithmetic (Excel's empty-cell-as-0). The empty cell is PRODUCED
/// by a 2-arg `IF` whose condition is false — `IF(A2>9999, 1)` has no else
/// branch, so Excel (and the runtime semantics layer) returns EMPTY (not a hard
/// `#REF!` the way an absent range member would). Then `A2 + A1` adds the
/// number to the empty cell: `5 + (empty->0) = 5`. {formula `A2+A1` where
/// `A1 = IF(A2>9999,1)` is empty; context: empty cell in additive `+`; oracle
/// `5`; reconcile key `out_sum` → B1}. The ONLY input is A2.
///
/// (Why not a blank cell in a SUM range: on this runtime an ABSENT range member
/// is a hard `#REF!`, not Empty — empty-cell-as-0 applies to a cell that RESOLVES
/// to Empty, which the 2-arg `IF` produces deterministically.)
fn quirk_empty_coercion_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A2",
                value: 5.0,
                paint: CellPaint::Input,
            },
            // A1: a deterministically-EMPTY helper (false 2-arg IF, no else).
            AuthoredCell::Formula {
                addr: "A1",
                formula: "IF(A2>9999,1)",
                cached: "",
            },
            // B1: the named output — adds the number to the empty cell (->0).
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A2+A1",
                cached: "5",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_sum",
                target: "'Quirk'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// QUIRK: float boundary. `0.1 + 0.2` is stored in binary-f64 as
/// `0.30000000000000004`, not exactly `0.3` — which is WHY money compares go
/// through `within_tol` (±0.01), never exact-float `==`. {formula `A1+A2`,
/// context: binary-f64 additive boundary; oracle `0.3` (graded within_tol);
/// reconcile key `out_sum` → B1}.
fn quirk_float_boundary_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 0.1,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A2",
                value: 0.2,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1+A2",
                cached: "0.3",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_a",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "in_b",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_sum",
                target: "'Quirk'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// QUIRK: text→number coercion. In a MULTIPLICATIVE context Excel coerces a
/// numeric-text operand to its number: `"5.5" * 2` is `11`. (Context is
/// load-bearing: in an ADDITIVE `&`/concat context the same text would NOT be
/// summed — see the scalar_eval layer, which pins the `+`-vs-`*` distinction.)
/// {formula `A1*A2`, context: numeric-text in `*`; oracle `11`; reconcile key
/// `out_product` → B1}. A1 is authored as TEXT `"5.5"`.
fn quirk_text_coercion_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Text {
                addr: "A1",
                text: "5.5",
            },
            AuthoredCell::Number {
                addr: "A2",
                value: 2.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1*A2",
                cached: "11",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_factor",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_product",
                target: "'Quirk'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// The synthetic loan/mortgage rate-tier calculator spec (Plan 96-04 Task 1 —
/// the WBEX-01 second-workbook generalization fixture).
///
/// # Why this workbook is deliberately divergent from `tax-calc` (D-01/D-02)
///
/// It is a fixed-cell formula DAG whose divergence is sourced ENTIRELY from
/// whitelist-legal LOOKUP families — proving the manifest-driven serve path
/// generalizes to a workbook whose shape `tax-calc` never exercised:
///
/// - a rate-tier TABLE (governed constants `D2:E4`, painted green) mapping a
///   credit-tier index → an annual rate,
/// - `VLOOKUP(...)` AND `INDEX(.., MATCH(.., 0))` both resolving the tier→rate
///   (two lookup families, cross-checked),
/// - `IFERROR(..)` guarding a lookup miss,
/// - a nested `IF(.., IF(..))` tiering the credit score into a band index,
/// - `ROUND`/`CEILING` to currency (the executor's `excel_round`/`excel_ceiling`
///   source of truth — the cached `<v>` oracle below is computed against them).
///
/// It is whitelist-legal: NO `PMT`, NO `POWER`, NO exponentiation (an
/// arbitrary-term `(1+r)^n` amortization is NOT expressible and is deferred);
/// the first-period interest model below stays inside the 17-fn whitelist.
///
/// # Named inputs / outputs (with units)
///
/// Inputs (blue font → `Role::Input`): `loan_amount` (A2, USD), `term_months`
/// (A3), `credit_score` (A4).
///
/// Outputs (`out_*` named ranges → `Role::Output`, MULTIPLE — no privileged
/// single headline, the Anti-Pattern this fixture guards against):
/// - `out_credit_tier` (B2) — the resolved tier index,
/// - `out_applied_rate` (B3, unit `percent`) — the VLOOKUP'd annual rate (the
///   custom-unit output that exercises the served schema's unit projection),
/// - `out_monthly_interest` (B5, unit `USD`) — first-period interest,
/// - `out_total_interest` (B6, unit `USD`) — `CEILING`'d simple-interest total,
/// - `out_tier_rate` (B8, unit `percent`) — the INDEX/MATCH cross-check of the
///   same tier→rate (proves both lookup families resolve identically).
///
/// # The reconcile oracle (cached `<v>`)
///
/// Worked for `loan_amount=240000, term_months=360, credit_score=700`:
/// - B2 tier  = `IF(700>=740,3,IF(700>=680,2,1))`                     → `2`
/// - B3 rate  = `IFERROR(VLOOKUP(2,D2:E4,2,FALSE),0.08)`             → `0.06`
/// - B4 m.rate= `ROUND(0.06/12,6)`                                    → `0.005`
/// - B5 m.int = `ROUND(240000*0.005,2)`                              → `1200`
/// - B6 t.int = `CEILING(1200*360,1)`                               → `432000`
/// - B8 tier  = `IFERROR(INDEX(E2:E4,MATCH(2,D2:D4,0)),0.08)`        → `0.06`
fn loan_calc_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Loan",
        cells: vec![
            // ---- labels (documentation, not role-bearing) ----
            AuthoredCell::Text {
                addr: "A1",
                text: "Loan rate-tier calculator (synthetic)",
            },
            AuthoredCell::Text {
                addr: "C1",
                text: "tier",
            },
            AuthoredCell::Text {
                addr: "D1",
                text: "rate",
            },
            // ---- inputs (blue font → Role::Input) ----
            AuthoredCell::Number {
                addr: "A2",
                value: 240_000.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A3",
                value: 360.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A4",
                value: 700.0,
                paint: CellPaint::Input,
            },
            // ---- rate-tier table (governed constants, green fill) ----
            AuthoredCell::Number {
                addr: "D2",
                value: 1.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E2",
                value: 0.08,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "D3",
                value: 2.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E3",
                value: 0.06,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "D4",
                value: 3.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E4",
                value: 0.045,
                paint: CellPaint::Constant,
            },
            // ---- the formula DAG (cached <v> = the reconcile oracle) ----
            AuthoredCell::Formula {
                addr: "B2",
                formula: "IF(A4>=740,3,IF(A4>=680,2,1))",
                cached: "2",
            },
            AuthoredCell::Formula {
                addr: "B3",
                formula: "IFERROR(VLOOKUP(B2,D2:E4,2,FALSE),0.08)",
                cached: "0.06",
            },
            AuthoredCell::Formula {
                addr: "B4",
                formula: "ROUND(B3/12,6)",
                cached: "0.005",
            },
            AuthoredCell::Formula {
                addr: "B5",
                formula: "ROUND(A2*B4,2)",
                cached: "1200",
            },
            AuthoredCell::Formula {
                addr: "B6",
                formula: "CEILING(B5*A3,1)",
                cached: "432000",
            },
            AuthoredCell::Formula {
                addr: "B8",
                formula: "IFERROR(INDEX(E2:E4,MATCH(B2,D2:D4,0)),0.08)",
                cached: "0.06",
            },
            // ---- the WBDL-02 present-path declaration (compatible value) ----
            AuthoredCell::Text {
                addr: "A10",
                text: "1.0",
            },
        ],
        defined_names: vec![
            // `in_*` INPUT named ranges (the input analogue of `out_*`): they give
            // each blue-font input a STABLE semantic served key (`loan_amount`)
            // instead of the cell's numeric value. See `name_named_inputs` (lib.rs).
            DefinedNameSpec {
                name: "in_loan_amount",
                target: "'Loan'!$A$2",
            },
            DefinedNameSpec {
                name: "in_term_months",
                target: "'Loan'!$A$3",
            },
            DefinedNameSpec {
                name: "in_credit_score",
                target: "'Loan'!$A$4",
            },
            DefinedNameSpec {
                name: "out_credit_tier",
                target: "'Loan'!$B$2",
            },
            DefinedNameSpec {
                name: "out_applied_rate",
                target: "'Loan'!$B$3",
            },
            DefinedNameSpec {
                name: "out_monthly_interest",
                target: "'Loan'!$B$5",
            },
            DefinedNameSpec {
                name: "out_total_interest",
                target: "'Loan'!$B$6",
            },
            DefinedNameSpec {
                name: "out_tier_rate",
                target: "'Loan'!$B$8",
            },
            DefinedNameSpec {
                name: "pmcp_dialect_version",
                target: "'Loan'!$A$10",
            },
        ],
        tables: vec![],
    }
}

/// The 1900-leap-year probe spec (Plan 96-03 Task 2 disposition A).
///
/// Excel treats 1900 as a leap year, so the date serial for any day on/after
/// 1900-03-01 is ONE GREATER than the astronomically-correct count from the
/// 1900-01-01 epoch (the phantom serial 60 = 1900-02-29). The probe encodes this
/// as PURE f64 serial arithmetic with whitelisted ops only (`IF` + `+`/`-`/
/// comparison) — NO `DATE`/`DATEVALUE`, NO new dialect function:
///
/// - `A1` (input): a raw day-count from the 1900-01-01 epoch (here `61`, which is
///   1900-03-01 counting 1900-01-01 as serial 1).
/// - `B1` (formula `IF(A1>59, A1+1, A1)` cached `62`): adds the phantom-leap
///   offset for serials past 1900-02-28 — exactly Excel's serial behaviour.
/// - `out_excel_serial` → `B1`: the Excel serial the reconcile oracle grades.
fn leap1900_probe_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Serial",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 61.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "IF(A1>59,A1+1,A1)",
                cached: "62",
            },
        ],
        defined_names: vec![
            // F1: the serial input needs an `in_*` named range for a callable key.
            DefinedNameSpec {
                name: "in_excel_serial",
                target: "'Serial'!$A$1",
            },
            DefinedNameSpec {
                name: "out_excel_serial",
                target: "'Serial'!$B$1",
            },
        ],
        tables: vec![],
    }
}

/// The WBV2-01 shipped tax-suite TEMPLATE spec (§7 annotated reference diagram):
/// the one `template.xlsx` that triples as the BA starting point, the
/// training/documentation artifact, and the honest reference fixture replacing
/// the misleading hand-authored `tax-calc`/`leap1900` fixtures.
///
/// # Layout (single `Data` sheet)
///
/// - `0_meta` (optional, OQ-1 minimal key set): `server: tax-suite`, `version: 1`
///   in row 1.
/// - `Inputs` Excel Table (`name|value|description|tier`, header row 3): `income`
///   (currency → USD unit witness), `filing` (a `{single,married}` sample enum
///   dropdown → enum-harvest demo), `withheld` (currency), `rate` (percent,
///   `tier=strict` → NOT caller-exposed). The `tier` column carries a
///   `{variable,strict}` dropdown (dogfooding the enum-from-dropdown mechanism).
/// - a small VLOOKUP/rate REFERENCE region (cols F:G, the DAG interior — not
///   exposed) the tax formula reads.
/// - two named output Excel Tables, each with a caption (= tool description)
///   directly above it:
///   - `Calculate_Tax` — `tax_owed` (18241) / `effective_rate` (0.182) as
///     `Formula::set_result` cached oracles reachable from `income`+`filing`,
///   - `Estimate_Refund` — `refund` (-3241) reachable from `income`+`filing`+
///     `withheld` (so Plan 03's DAG-derived inputs prove the §4.2 example).
///
/// # Provenance
///
/// Authored ENTIRELY by `rust_xlsxwriter`, so it classifies RAW
/// [`ProvenanceClass::ExcelTrusted`] (§6 orthogonal provenance) with NO
/// `template.provenance-override.json` sidecar — the `template_provenance.rs`
/// test asserts the override-free RAW class.
fn template_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Data",
        cells: vec![
            // ---- 0_meta (optional, minimal {server, version}) ----
            AuthoredCell::Text {
                addr: "A1",
                text: "server",
            },
            AuthoredCell::Text {
                addr: "B1",
                text: "tax-suite",
            },
            AuthoredCell::Text {
                addr: "C1",
                text: "version",
            },
            AuthoredCell::Number {
                addr: "D1",
                value: 1.0,
                paint: CellPaint::Plain,
            },
            // ---- Inputs table BODY (header row 3 written by the Table) ----
            // income (currency → USD unit witness), blue-font input.
            AuthoredCell::Text {
                addr: "A4",
                text: "income",
            },
            AuthoredCell::NumberFmt {
                addr: "B4",
                value: 100_000.0,
                paint: CellPaint::Input,
                num_format: "$#,##0",
            },
            AuthoredCell::Text {
                addr: "C4",
                text: "annual gross (USD $)",
            },
            AuthoredCell::Text {
                addr: "D4",
                text: "variable",
            },
            // filing (a {single,married} sample enum dropdown), text value.
            AuthoredCell::Text {
                addr: "A5",
                text: "filing",
            },
            AuthoredCell::Text {
                addr: "B5",
                text: "single",
            },
            AuthoredCell::Text {
                addr: "C5",
                text: "filing status",
            },
            AuthoredCell::Text {
                addr: "D5",
                text: "variable",
            },
            // withheld (currency → USD), blue-font input.
            AuthoredCell::Text {
                addr: "A6",
                text: "withheld",
            },
            AuthoredCell::NumberFmt {
                addr: "B6",
                value: 15_000.0,
                paint: CellPaint::Input,
                num_format: "$#,##0",
            },
            AuthoredCell::Text {
                addr: "C6",
                text: "tax withheld YTD (USD)",
            },
            AuthoredCell::Text {
                addr: "D6",
                text: "variable",
            },
            // rate (percent → rate unit), tier=strict → NOT caller-exposed.
            AuthoredCell::Text {
                addr: "A7",
                text: "rate",
            },
            AuthoredCell::NumberFmt {
                addr: "B7",
                value: 0.22,
                paint: CellPaint::Constant,
                num_format: "0.0%",
            },
            AuthoredCell::Text {
                addr: "C7",
                text: "statutory bracket rate",
            },
            AuthoredCell::Text {
                addr: "D7",
                text: "strict",
            },
            // ---- reference / lookup region (cols F:G, DAG interior) ----
            AuthoredCell::Text {
                addr: "F1",
                text: "bracket",
            },
            AuthoredCell::Text {
                addr: "G1",
                text: "rate",
            },
            AuthoredCell::Number {
                addr: "F2",
                value: 0.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::NumberFmt {
                addr: "G2",
                value: 0.10,
                paint: CellPaint::Constant,
                num_format: "0.0%",
            },
            AuthoredCell::Number {
                addr: "F3",
                value: 40_000.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::NumberFmt {
                addr: "G3",
                value: 0.22,
                paint: CellPaint::Constant,
                num_format: "0.0%",
            },
            // ---- Calculate_Tax output table BODY (caption A9, header row 10) ----
            // tax_owed reachable ONLY from income (B4) + the rate ref (G3).
            AuthoredCell::Text {
                addr: "A11",
                text: "tax_owed",
            },
            AuthoredCell::Formula {
                addr: "B11",
                // tax_owed = ROUND(income*rate - 3759, 0) = ROUND(100000*0.22 - 3759, 0)
                // = ROUND(18241, 0) = 18241 — the cached <v> oracle. The constant is
                // -3759 (NOT -1759) so the RECOMPUTATION equals the authored cached
                // value; the production reconcile grades computed-formula vs cached-<v>,
                // so the template MUST be self-consistent to compile (WBV2-04 E2E).
                formula: "ROUND(B4*G3-3759,0)",
                cached: "18241",
            },
            AuthoredCell::Text {
                addr: "C11",
                text: "federal tax liability (USD)",
            },
            // effective_rate reachable from income (B4) + tax_owed (B11).
            AuthoredCell::Text {
                addr: "A12",
                text: "effective_rate",
            },
            AuthoredCell::Formula {
                addr: "B12",
                formula: "ROUND(B11/B4,3)",
                cached: "0.182",
            },
            AuthoredCell::Text {
                addr: "C12",
                text: "effective tax rate (%)",
            },
            // ---- Estimate_Refund output table BODY (caption A15, header row 16) ----
            // refund reachable from withheld (B6) + tax_owed (B11).
            AuthoredCell::Text {
                addr: "A17",
                text: "refund",
            },
            AuthoredCell::Formula {
                addr: "B17",
                formula: "ROUND(B6-B11,0)",
                cached: "-3241",
            },
            AuthoredCell::Text {
                addr: "C17",
                text: "estimated refund (neg = owed)",
            },
        ],
        defined_names: vec![],
        tables: vec![
            TableSpec {
                name: "Inputs",
                sheet: "Data",
                top_left: (0, 2), // header A3, body rows A4:D7
                columns: &["name", "value", "description", "tier"],
                caption: None,
                rows: vec![],
                body_rows: 4, // income, filing, withheld, rate
                data_validations: vec![
                    // tier column {variable, strict} over D4:D7.
                    DataValidationSpec {
                        range: "D4:D7",
                        kind: DvKind::List(&["variable", "strict"]),
                    },
                    // sample enum {single, married} over the filing value B5.
                    DataValidationSpec {
                        range: "B5:B5",
                        kind: DvKind::List(&["single", "married"]),
                    },
                ],
            },
            TableSpec {
                name: "Calculate_Tax",
                sheet: "Data",
                top_left: (0, 9), // caption A9, header A10, body A11:C12
                columns: &["name", "value", "description"],
                caption: Some("Compute federal tax from income & filing"),
                rows: vec![],
                body_rows: 2, // tax_owed, effective_rate
                data_validations: vec![],
            },
            TableSpec {
                name: "Estimate_Refund",
                sheet: "Data",
                top_left: (0, 15), // caption A15, header A16, body A17:C17
                columns: &["name", "value", "description"],
                caption: Some("Estimate refund given withholding"),
                rows: vec![],
                body_rows: 1, // refund
                data_validations: vec![],
            },
        ],
    }
}

/// The WR-01 HARDENING fixture (Plan 100-08-HARDENING): a REAL multi-sheet,
/// Table-authored workbook that mirrors the synthetic
/// `build_tools_surfaces_range_and_cross_sheet_inputs`
/// ([`crate::artifact::cell_map`]) shape — but compiled THROUGH the production
/// pipeline so the explain↔served parity is met BY CONSTRUCTION, not only by the
/// projection-equivalence proptest.
///
/// # Why this fixture exists
///
/// The H1 parity test (`explain_projection_matches_the_served_tool_surface`) runs
/// over `template.xlsx`, which is single-sheet and has NO `SUM(range)` and NO
/// cross-sheet ref. So the range/cross-sheet coverage lived ONLY in a
/// `build_tools`-DIRECT synthetic test (never exercised THROUGH the explain
/// projection over a real compile). This fixture closes that gap: a tool input
/// reached ONLY via `SUM(range)` AND a tool input reached ONLY via a cross-sheet
/// ref are BOTH surfaced on the production projection (== the served surface).
///
/// # Shape (two sheets, all whitelist-legal)
///
/// - Sheet `Data`: an `Inputs` Excel Table with two blue-font inputs `q1` (B2=100)
///   and `q2` (B3=200) — reached by the output ONLY via `SUM(B2:B3)` (the §3 range
///   case) — and an output Table `Total_Sales` whose `total` formula is
///   `ROUND(SUM(B2:B3)+Aux!B2,0)`.
/// - Sheet `Aux`: an `Adjustments` Excel Table with ONE blue-font input
///   `adjustment` (B2=50) — reached by the output ONLY via the cross-sheet `Aux!B2`
///   reference (the §3 cross-sheet case).
///
/// # The reconcile oracle (cached `<v>`)
///
/// `total = ROUND(SUM(100,200)+50, 0) = ROUND(350, 0) = 350` — the authored cached
/// `<v>` the production reconcile grades the executor's recomputation against. The
/// workbook is therefore self-consistent and compiles green via the trusted-fixture
/// override (its `fullCalcOnLoad=1` staleness is demoted on the test path only).
///
/// # Served surface this produces
///
/// ONE tool `total_sales` whose DAG-derived `input_keys` are exactly
/// `{q1, q2, adjustment}` — the range members AND the cross-sheet input — and whose
/// single output is `total`. The parity test asserts these per-tool input keys EQUAL
/// the served `input_schema_for_tool` keys (true explain↔served parity over a real
/// range + cross-sheet workbook).
pub(crate) fn range_cross_sheet_spec() -> MultiSheetSpec {
    let data = SheetSpec {
        name: "Data",
        cells: vec![],
        tables: vec![
            // The Inputs Table: q1/q2 reached ONLY via SUM(B2:B3) (a range).
            TableSpec {
                name: "Inputs",
                sheet: "Data",
                top_left: (0, 0), // header A1, body A2:C3
                columns: &["name", "value", "description"],
                caption: None,
                rows: vec![
                    vec![
                        AuthoredCell::Text {
                            addr: "A2",
                            text: "q1",
                        },
                        AuthoredCell::Number {
                            addr: "B2",
                            value: 100.0,
                            paint: CellPaint::Input,
                        },
                        AuthoredCell::Text {
                            addr: "C2",
                            text: "first-quarter sales",
                        },
                    ],
                    vec![
                        AuthoredCell::Text {
                            addr: "A3",
                            text: "q2",
                        },
                        AuthoredCell::Number {
                            addr: "B3",
                            value: 200.0,
                            paint: CellPaint::Input,
                        },
                        AuthoredCell::Text {
                            addr: "C3",
                            text: "second-quarter sales",
                        },
                    ],
                ],
                body_rows: 0, // inline rows declare the body height
                data_validations: vec![],
            },
            // The output Table: `total` reaches q1/q2 ONLY via SUM(B2:B3) and
            // `adjustment` ONLY via the cross-sheet Aux!B2 ref.
            TableSpec {
                name: "Total_Sales",
                sheet: "Data",
                top_left: (0, 5), // caption A5, header A6, body A7:C7
                columns: &["name", "value", "description"],
                caption: Some("Total sales across quarters plus the cross-sheet adjustment"),
                rows: vec![vec![
                    AuthoredCell::Text {
                        addr: "A7",
                        text: "total",
                    },
                    AuthoredCell::Formula {
                        addr: "B7",
                        // total = ROUND(SUM(100,200) + 50, 0) = 350 (the cached <v>
                        // oracle). q1/q2 are reached ONLY through SUM(B2:B3); the
                        // adjustment ONLY through the cross-sheet Aux!B2 reference.
                        formula: "ROUND(SUM(B2:B3)+Aux!B2,0)",
                        cached: "350",
                    },
                    AuthoredCell::Text {
                        addr: "C7",
                        text: "total sales (USD)",
                    },
                ]],
                body_rows: 0, // inline row declares the body height
                data_validations: vec![],
            },
        ],
    };
    let aux = SheetSpec {
        name: "Aux",
        cells: vec![],
        tables: vec![TableSpec {
            name: "Adjustments",
            sheet: "Aux",
            top_left: (0, 0), // header A1, body A2:C2
            columns: &["name", "value", "description"],
            caption: None,
            rows: vec![vec![
                AuthoredCell::Text {
                    addr: "A2",
                    text: "adjustment",
                },
                AuthoredCell::Number {
                    addr: "B2",
                    value: 50.0,
                    paint: CellPaint::Input,
                },
                AuthoredCell::Text {
                    addr: "C2",
                    text: "manual cross-sheet adjustment",
                },
            ]],
            body_rows: 0,
            data_validations: vec![],
        }],
    };
    MultiSheetSpec {
        sheets: vec![data, aux],
        defined_names: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_workbook, compile_workbook_with_fixture_override};

    /// A small WorkbookSpec carrying ONE input Excel Table named `Inputs` with the
    /// standard `name|value|description|tier` columns, a `{variable,strict}` tier
    /// dropdown + a sample `{single,married}` enum dropdown, and an output Table
    /// `Calculate_Tax` with a caption directly above it. The minimal exerciser for
    /// the Table-author surface round-trip self-tests.
    fn table_spec_workbook() -> WorkbookSpec {
        let inputs = TableSpec {
            name: "Inputs",
            sheet: "Data",
            top_left: (0, 0), // header at A1; body rows below
            columns: &["name", "value", "description", "tier"],
            caption: None,
            rows: vec![
                vec![
                    AuthoredCell::Text {
                        addr: "A2",
                        text: "income",
                    },
                    AuthoredCell::Number {
                        addr: "B2",
                        value: 100_000.0,
                        paint: CellPaint::Input,
                    },
                    AuthoredCell::Text {
                        addr: "C2",
                        text: "annual gross",
                    },
                    AuthoredCell::Text {
                        addr: "D2",
                        text: "variable",
                    },
                ],
                vec![
                    AuthoredCell::Text {
                        addr: "A3",
                        text: "filing",
                    },
                    AuthoredCell::Text {
                        addr: "B3",
                        text: "single",
                    },
                    AuthoredCell::Text {
                        addr: "C3",
                        text: "filing status",
                    },
                    AuthoredCell::Text {
                        addr: "D3",
                        text: "variable",
                    },
                ],
            ],
            body_rows: 0, // inline rows above declare the body height
            data_validations: vec![
                // tier column dropdown {variable, strict} over D2:D3
                DataValidationSpec {
                    range: "D2:D3",
                    kind: DvKind::List(&["variable", "strict"]),
                },
                // sample enum dropdown {single, married} over the filing value B3
                DataValidationSpec {
                    range: "B3:B3",
                    kind: DvKind::List(&["single", "married"]),
                },
            ],
        };
        let output = TableSpec {
            name: "Calculate_Tax",
            sheet: "Data",
            top_left: (0, 5), // (col=0=A, row=5) → header at A6, caption at A5
            columns: &["name", "value", "description"],
            caption: Some("Compute federal tax from income & filing"),
            rows: vec![vec![
                AuthoredCell::Text {
                    addr: "A7",
                    text: "tax_owed",
                },
                AuthoredCell::Number {
                    addr: "B7",
                    value: 18_241.0,
                    paint: CellPaint::Plain,
                },
                AuthoredCell::Text {
                    addr: "C7",
                    text: "federal tax liability",
                },
            ]],
            body_rows: 0, // inline row above declares the body height
            data_validations: vec![],
        };
        WorkbookSpec {
            sheet: "Data",
            cells: vec![],
            defined_names: vec![],
            tables: vec![inputs, output],
        }
    }

    /// (Round-trip #1) A WorkbookSpec carrying TableSpecs authored via `author_xlsx`
    /// produces an `.xlsx` whose worksheet, re-read by umya `tables()`, reports the
    /// same table names and the same column header names.
    #[test]
    fn authored_tables_roundtrip_name_and_columns() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("tables.xlsx");
        author_xlsx(&xlsx, &table_spec_workbook()).expect("author tables workbook");

        let book = umya_spreadsheet::reader::xlsx::read(&xlsx).expect("re-read authored workbook");
        let ws = book.sheet_by_name("Data").expect("Data sheet exists");
        let tables = ws.tables();

        let names: Vec<&str> = tables.iter().map(umya_spreadsheet::Table::name).collect();
        assert!(
            names.contains(&"Inputs"),
            "the Inputs table name round-trips (got {names:?})"
        );
        assert!(
            names.contains(&"Calculate_Tax"),
            "the Calculate_Tax table name round-trips (got {names:?})"
        );

        let inputs = tables
            .iter()
            .find(|t| t.name() == "Inputs")
            .expect("Inputs table present");
        let headers: Vec<&str> = inputs.columns().iter().map(|c| c.name()).collect();
        assert_eq!(
            headers,
            vec!["name", "value", "description", "tier"],
            "the Inputs table column headers round-trip"
        );
    }

    /// (Round-trip #2) A TableSpec with a tier-column data-validation `{variable,
    /// strict}` and a sample enum data-validation produces an `.xlsx` whose
    /// `data_validations()` reports a `list` dv_type over the corresponding ranges.
    #[test]
    fn authored_tables_roundtrip_data_validations() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("tables.xlsx");
        author_xlsx(&xlsx, &table_spec_workbook()).expect("author tables workbook");

        let book = umya_spreadsheet::reader::xlsx::read(&xlsx).expect("re-read authored workbook");
        let ws = book.sheet_by_name("Data").expect("Data sheet exists");
        let dvs = ws
            .data_validations()
            .expect("the sheet carries data validations");

        let list_dvs: Vec<_> = dvs
            .data_validation_list()
            .iter()
            .filter(|dv| {
                use umya_spreadsheet::structs::EnumTrait;
                dv.get_type().value_string() == "list"
            })
            .collect();
        assert!(
            list_dvs.len() >= 2,
            "both the tier dropdown and the sample enum dropdown are `list` dvs (got {})",
            list_dvs.len()
        );
    }

    /// (Round-trip #3) An output TableSpec authored with a caption cell directly
    /// above the table writes that caption string at `(header_row-1, first_col)`.
    #[test]
    fn authored_table_caption_written_above() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("tables.xlsx");
        author_xlsx(&xlsx, &table_spec_workbook()).expect("author tables workbook");

        let book = umya_spreadsheet::reader::xlsx::read(&xlsx).expect("re-read authored workbook");
        let ws = book.sheet_by_name("Data").expect("Data sheet exists");
        // Calculate_Tax header is at A6 (row index 5), so the caption is at A5.
        let caption = ws.value((1u32, 5u32)); // (col=1=A, row=5)
        assert_eq!(
            caption, "Compute federal tax from income & filing",
            "the output table caption is written directly above the header row"
        );
    }

    /// (Multi-sheet round-trip) The WR-01 hardening fixture
    /// ([`range_cross_sheet_spec`]) authored via [`author_multi_sheet_xlsx`] writes
    /// REAL second worksheet: re-read by umya, `Data` carries `Inputs`+`Total_Sales`
    /// and `Aux` carries `Adjustments` — the `Aux` sheet that the `Total_Sales`
    /// formula reaches via the cross-sheet `Aux!B2` ref genuinely exists.
    #[test]
    fn multi_sheet_author_writes_both_sheets_and_their_tables() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("range-cross-sheet.xlsx");
        author_multi_sheet_xlsx(&xlsx, &range_cross_sheet_spec())
            .expect("author the multi-sheet fixture");

        let book = umya_spreadsheet::reader::xlsx::read(&xlsx).expect("re-read authored workbook");

        // Both worksheets exist (the cross-sheet ref target Aux is a REAL sheet).
        let data = book.sheet_by_name("Data").expect("Data sheet exists");
        let aux = book.sheet_by_name("Aux").expect("Aux sheet exists");

        let data_tables: Vec<&str> = data
            .tables()
            .iter()
            .map(umya_spreadsheet::Table::name)
            .collect();
        assert!(
            data_tables.contains(&"Inputs") && data_tables.contains(&"Total_Sales"),
            "Data carries the Inputs + Total_Sales tables (got {data_tables:?})"
        );
        let aux_tables: Vec<&str> = aux
            .tables()
            .iter()
            .map(umya_spreadsheet::Table::name)
            .collect();
        assert!(
            aux_tables.contains(&"Adjustments"),
            "Aux carries the Adjustments table on the SECOND sheet (got {aux_tables:?})"
        );

        // The cross-sheet input value lives on the Aux sheet at B2 (the Aux!B2 the
        // Total_Sales formula reaches).
        assert_eq!(
            aux.value((2u32, 2u32)),
            "50",
            "Aux!B2 = the adjustment input"
        );
        // The Total_Sales output formula (B7) reaches q1/q2 via SUM(range) AND the
        // adjustment via the cross-sheet Aux!B2 ref.
        let formula = data
            .cell("B7")
            .map(|c| c.formula().to_string())
            .unwrap_or_default();
        assert!(
            formula.contains("Aux!B2") && formula.contains("SUM(B2:B3)"),
            "the Total_Sales formula reaches q1/q2 via SUM(range) AND adjustment via \
             cross-sheet Aux!B2 (got {formula:?})"
        );
    }

    /// (Byte-determinism) [`author_multi_sheet_xlsx`] is byte-deterministic — the
    /// PINNED creation datetime means authoring the SAME spec twice produces
    /// byte-identical `.xlsx` output (the reproducible-fixture discipline the
    /// single-sheet author already holds, now shared with the multi-sheet author).
    #[test]
    fn multi_sheet_author_is_byte_deterministic() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let a = dir.path().join("a.xlsx");
        let b = dir.path().join("b.xlsx");
        let spec = range_cross_sheet_spec();
        author_multi_sheet_xlsx(&a, &spec).expect("author a");
        author_multi_sheet_xlsx(&b, &spec).expect("author b");
        let bytes_a = std::fs::read(&a).expect("read a");
        let bytes_b = std::fs::read(&b).expect("read b");
        assert_eq!(
            bytes_a, bytes_b,
            "two authorings of the same multi-sheet spec are byte-identical \
             (deterministic creation datetime)"
        );
    }

    /// (Direct provenance assertion — multi-sheet) The multi-sheet authored workbook
    /// classifies `ProvenanceClass::ExcelTrusted` directly from its bytes, exactly as
    /// the single-sheet author does (the genuine Excel identity is workbook-global,
    /// independent of sheet count).
    #[test]
    fn multi_sheet_author_classifies_excel_trusted_directly() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("range-cross-sheet.xlsx");
        author_multi_sheet_xlsx(&xlsx, &range_cross_sheet_spec()).expect("author");
        let bytes = std::fs::read(&xlsx).expect("read authored bytes");
        assert_eq!(
            classify_authored(&bytes),
            ProvenanceClass::ExcelTrusted,
            "a multi-sheet rust_xlsxwriter-authored workbook carries genuine Excel \
             identity and MUST classify ExcelTrusted directly"
        );
    }

    /// (Direct provenance assertion #1) An authored workbook classifies
    /// `ProvenanceClass::ExcelTrusted` — read DIRECTLY from the authored bytes via
    /// the same raw-reader + classify path the production gate uses, NOT inferred
    /// from a successful compile. A umya-authored fixture would classify
    /// `UmyaFabricated` and fail this assertion.
    #[test]
    fn authored_xlsx_classifies_excel_trusted_directly() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let bytes = std::fs::read(&xlsx).expect("read authored bytes");
        assert_eq!(
            classify_authored(&bytes),
            ProvenanceClass::ExcelTrusted,
            "a rust_xlsxwriter-authored workbook carries genuine Excel identity \
             (<Application>Microsoft Excel</Application> + an <AppVersion> build + a \
             non-sentinel calcId) and MUST classify ExcelTrusted directly"
        );
    }

    /// (End-to-end #2) The authored workbook compiles through the `#[cfg(test)]`
    /// trusted-fixture override (the recipe passes the freshness gate end-to-end:
    /// genuine identity + demoted `fullCalcOnLoad` staleness), and the executor's
    /// recomputation reconciles against the authored cached `<v>` oracle.
    #[test]
    fn authored_xlsx_compiles_via_fixture_override() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        let lock = compile_workbook_with_fixture_override(
            &xlsx,
            &out_root,
            "trivial",
            "1.0.0",
            "fixture-author-proof",
        )
        .expect("the authored workbook compiles via the trusted-fixture override");
        assert_eq!(lock.version, "1.0.0");
    }

    /// (Production-refusal guard #3 — T-96-07) The SAME authored bytes are REFUSED
    /// on the production `compile_workbook` (Enforce) path: only the `#[cfg(test)]`
    /// override accepts the `fullCalcOnLoad=1` staleness; production never weakens.
    #[test]
    fn production_compile_refuses_authored_fixture() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        let result = compile_workbook(&xlsx, &out_root, "trivial", "1.0.0", "prod-approver");
        assert!(
            result.is_err(),
            "production compile_workbook (Enforce) MUST refuse the authored fixture's \
             fullCalcOnLoad staleness — the trusted-fixture override is test-only and \
             never weakens production (T-96-07)"
        );
    }

    /// The override marker + generation-metadata sidecars are written beside the
    /// fixture with the cloned trusted-fixture shape and the recorded provenance
    /// (generator fn, input cells, formula oracles, expected outputs). Authored
    /// into a TempDir — a normal test run writes NOTHING into `tests/fixtures/`.
    #[test]
    fn author_emits_override_and_gen_metadata() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        let spec = trivial_spec();
        author_xlsx(&xlsx, &spec).expect("author");
        write_override_marker(&xlsx, "smoke-test fixture").expect("marker");
        write_gen_metadata(&xlsx, "trivial_spec", &spec, "smoke-test fixture").expect("gen meta");

        let marker = std::fs::read_to_string(dir.path().join("trivial.provenance-override.json"))
            .expect("marker exists");
        let marker_json: serde_json::Value =
            serde_json::from_str(&marker).expect("marker is valid json");
        assert_eq!(marker_json["kind"], "trusted-fixture");
        assert_eq!(marker_json["fixture"], "trivial.xlsx");
        assert_eq!(marker_json["authored_by"], "rust_xlsxwriter");

        let gen = std::fs::read_to_string(dir.path().join("trivial.gen.json")).expect("gen exists");
        let gen_json: serde_json::Value = serde_json::from_str(&gen).expect("gen is valid json");
        assert_eq!(gen_json["generator_fn"], "trivial_spec");
        assert_eq!(gen_json["input_cells"][0], "A1");
        assert_eq!(gen_json["formula_oracles"][0]["cached_oracle"], "20");
        assert_eq!(gen_json["expected_outputs"][0]["name"], "out_result");
    }

    /// `parse_a1` maps A1-notation to zero-based `(row, col)` for the cells the
    /// fixtures use (single/multi-letter columns, 1-based rows).
    #[test]
    fn parse_a1_maps_addresses() {
        assert_eq!(parse_a1("A1"), (0, 0));
        assert_eq!(parse_a1("B2"), (1, 1));
        assert_eq!(parse_a1("Z1"), (0, 25));
        assert_eq!(parse_a1("AA1"), (0, 26));
    }

    /// The committed 1900-leap probe fixture (`tests/fixtures/leap1900-probe.xlsx`).
    fn committed_leap_probe() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/leap1900-probe.xlsx")
    }

    /// (SPIKE disposition A — Plan 96-03 Task 2) The COMMITTED leap-year probe
    /// compiles + RECONCILES via the trusted-fixture override: the executor
    /// recomputes `IF(A1>59, A1+1, A1)` (the Excel-serial phantom-leap offset over
    /// bare `f64`, whitelisted ops only) and reconciles against the authored cached
    /// `<v>` oracle (`62`). This PROVES the 1900-leap quirk is DAG-expressible
    /// without any date function — see SPIKE-1900-leap.md.
    #[test]
    fn leap1900_probe_compiles_and_reconciles() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("leap1900-probe.xlsx");
        std::fs::copy(committed_leap_probe(), &xlsx).expect("copy committed probe");

        // Direct provenance assertion: the committed probe is ExcelTrusted.
        let bytes = std::fs::read(&xlsx).expect("read probe bytes");
        assert_eq!(classify_authored(&bytes), ProvenanceClass::ExcelTrusted);

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        // A clean compile means the reconcile gate matched the executor's
        // recomputation (the serial offset) to the cached oracle — disposition A.
        compile_workbook_with_fixture_override(
            &xlsx,
            &out_root,
            "leap1900-probe",
            "1.0.0",
            "spike-1900-leap",
        )
        .expect("the leap1900 probe compiles + reconciles via the override (disposition A)");
    }
}
