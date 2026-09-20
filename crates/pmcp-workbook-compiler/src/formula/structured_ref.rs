//! Expansion of Excel single-column structured references into ordinary A1
//! ranges before the constrained formula parser runs.
//!
//! Excel Table authoring uses expressions such as `Inputs[description]`. The
//! runtime IR intentionally has one range representation (`RangeRef`), so the
//! compiler resolves that authoring syntax against the harvested `TableRecord`
//! metadata and feeds the parser an equivalent sheet-qualified A1 range.

use crate::ingest::{TableRecord, WorkbookMap};

/// A structured-reference expansion failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StructuredReferenceError {
    /// A known Table was referenced with a column it does not contain.
    UnknownColumn {
        /// Excel Table name.
        table: String,
        /// Requested column header.
        column: String,
    },
    /// Harvested Table coordinates were not a valid rectangular A1 range.
    InvalidTableArea {
        /// Excel Table name.
        table: String,
    },
}

impl std::fmt::Display for StructuredReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownColumn { table, column } => {
                write!(f, "Table `{table}` has no column named `{column}`")
            },
            Self::InvalidTableArea { table } => {
                write!(f, "Table `{table}` has an invalid or empty A1 area")
            },
        }
    }
}

impl std::error::Error for StructuredReferenceError {}

/// Expand every `Table[Column]` occurrence outside Excel string literals into
/// the Table body's sheet-qualified A1 range.
///
/// Matching is case-insensitive, like Excel. The header row is excluded. This
/// function deliberately supports only the single-column form used by the
/// workbook dialect; row-context (`[@Column]`), totals, and multi-column
/// selectors remain outside the constrained dialect.
pub(crate) fn expand_structured_references(
    formula: &str,
    map: &WorkbookMap,
) -> Result<String, StructuredReferenceError> {
    let mut tables: Vec<(&str, &str, &TableRecord)> = map
        .sheets
        .iter()
        .flat_map(|sheet| {
            sheet
                .table_records
                .iter()
                .map(move |table| (sheet.name.as_str(), table.name.as_str(), table))
        })
        .collect();
    expand_with_tables(formula, &mut tables)
}

fn expand_with_tables(
    formula: &str,
    tables: &mut [(&str, &str, &TableRecord)],
) -> Result<String, StructuredReferenceError> {
    // A longer Table name wins when one name is a prefix of another.
    tables.sort_by_key(|(_, name, _)| std::cmp::Reverse(name.len()));

    let mut out = String::with_capacity(formula.len());
    let mut pos = 0usize;
    while pos < formula.len() {
        let tail = &formula[pos..];
        if tail.starts_with('"') {
            pos = copy_string_literal(formula, pos, &mut out);
            continue;
        }

        if let Some((sheet, table, column, next)) = match_reference(formula, pos, tables) {
            out.push_str(&column_range(sheet, table, column)?);
            pos = next;
            continue;
        }

        let Some(ch) = tail.chars().next() else {
            break;
        };
        out.push(ch);
        pos += ch.len_utf8();
    }
    Ok(out)
}

/// Match a known `Table[Column]` at `pos`; unknown Tables are left for the
/// parser's existing external/unsupported-reference refusal.
fn match_reference<'formula, 'table>(
    formula: &'formula str,
    pos: usize,
    tables: &[(&'table str, &'table str, &'table TableRecord)],
) -> Option<(&'table str, &'table TableRecord, &'formula str, usize)> {
    let previous_is_identifier = formula[..pos]
        .chars()
        .next_back()
        .is_some_and(is_identifier_char);
    if previous_is_identifier {
        return None;
    }

    let tail = &formula[pos..];
    for (sheet, name, table) in tables {
        if tail.len() <= name.len()
            || !tail
                .get(..name.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            || tail.as_bytes().get(name.len()) != Some(&b'[')
        {
            continue;
        }
        let column_start = pos + name.len() + 1;
        let close_offset = formula[column_start..].find(']')?;
        let column_end = column_start + close_offset;
        let column = &formula[column_start..column_end];
        return Some((sheet, table, column, column_end + 1));
    }
    None
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.')
}

/// Copy one Excel string literal verbatim, including doubled-quote escapes.
fn copy_string_literal(formula: &str, start: usize, out: &mut String) -> usize {
    let mut chars = formula[start..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        out.push(ch);
        if offset == 0 || ch != '"' {
            continue;
        }
        if chars.peek().is_some_and(|(_, next)| *next == '"') {
            if let Some((_, escaped)) = chars.next() {
                out.push(escaped);
            }
            continue;
        }
        return start + offset + ch.len_utf8();
    }
    formula.len()
}

/// Resolve one Table column to its body range (header excluded).
fn column_range(
    sheet: &str,
    table: &TableRecord,
    requested_column: &str,
) -> Result<String, StructuredReferenceError> {
    let column_offset = table
        .columns
        .iter()
        .position(|column| column.eq_ignore_ascii_case(requested_column))
        .ok_or_else(|| StructuredReferenceError::UnknownColumn {
            table: table.name.clone(),
            column: requested_column.to_string(),
        })?;
    let (start_column, header_row) =
        parse_a1(&table.area.start).ok_or_else(|| StructuredReferenceError::InvalidTableArea {
            table: table.name.clone(),
        })?;
    let (_, end_row) =
        parse_a1(&table.area.end).ok_or_else(|| StructuredReferenceError::InvalidTableArea {
            table: table.name.clone(),
        })?;
    let body_start =
        header_row
            .checked_add(1)
            .ok_or_else(|| StructuredReferenceError::InvalidTableArea {
                table: table.name.clone(),
            })?;
    if body_start > end_row {
        return Err(StructuredReferenceError::InvalidTableArea {
            table: table.name.clone(),
        });
    }
    let column_index = start_column
        .checked_add(column_offset as u32)
        .ok_or_else(|| StructuredReferenceError::InvalidTableArea {
            table: table.name.clone(),
        })?;
    let column = column_letters(column_index);
    let escaped_sheet = sheet.replace('\'', "''");
    Ok(format!(
        "'{escaped_sheet}'!{column}{body_start}:{column}{end_row}"
    ))
}

fn parse_a1(address: &str) -> Option<(u32, u32)> {
    let split = address.find(|ch: char| ch.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (column, row) = address.split_at(split);
    let mut column_index = 0u32;
    for ch in column.bytes() {
        if !ch.is_ascii_alphabetic() {
            return None;
        }
        column_index = column_index
            .checked_mul(26)?
            .checked_add(u32::from(ch.to_ascii_uppercase() - b'A' + 1))?;
    }
    Some((column_index, row.parse().ok()?))
}

fn column_letters(mut index: u32) -> String {
    let mut letters = Vec::new();
    while index > 0 {
        let remainder = ((index - 1) % 26) as u8;
        letters.push(b'A' + remainder);
        index = (index - 1) / 26;
    }
    letters.reverse();
    String::from_utf8(letters).unwrap_or_default()
}

/// Fuzz-only hook over the scanner with a representative Table binding. The
/// production surface remains crate-private; cargo-fuzz reaches it through the
/// `formula` module's cfg-gated wrapper.
#[cfg(fuzzing)]
pub(crate) fn fuzz_expand_structured_references(formula: &str) {
    use pmcp_workbook_runtime::RangeRef;

    let table = TableRecord {
        name: "Inputs".to_string(),
        area: RangeRef {
            sheet: "Data".to_string(),
            start: "A3".to_string(),
            end: "D18".to_string(),
        },
        columns: vec![
            "name".to_string(),
            "value".to_string(),
            "description".to_string(),
            "tier".to_string(),
        ],
    };
    let mut tables = vec![("Data", "Inputs", &table)];
    let _ = expand_with_tables(formula, &mut tables);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::SheetRecord;
    use pmcp_workbook_runtime::RangeRef;
    use proptest::prelude::*;

    fn workbook() -> WorkbookMap {
        WorkbookMap {
            sheets: vec![SheetRecord {
                name: "Data Sheet".to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                table_records: vec![TableRecord {
                    name: "Inputs".to_string(),
                    area: RangeRef {
                        sheet: "Data Sheet".to_string(),
                        start: "A3".to_string(),
                        end: "D18".to_string(),
                    },
                    columns: vec![
                        "name".to_string(),
                        "value".to_string(),
                        "description".to_string(),
                        "tier".to_string(),
                    ],
                }],
                data_validations: vec![],
                notes: vec![],
                cells: vec![],
            }],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    #[test]
    fn expands_table_columns_to_body_ranges() {
        let expanded = expand_structured_references(
            "XLOOKUP(A1,Inputs[description],Inputs[value])",
            &workbook(),
        )
        .expect("structured references expand");
        assert_eq!(
            expanded,
            "XLOOKUP(A1,'Data Sheet'!C4:C18,'Data Sheet'!B4:B18)"
        );
        crate::formula::parse(&expanded, "Data Sheet", "B22")
            .expect("the expanded formula reaches the constrained parser");
    }

    #[test]
    fn does_not_expand_reference_shaped_text_inside_a_string() {
        let expanded =
            expand_structured_references("IF(A1=\"Inputs[value]\",Inputs[value],0)", &workbook())
                .expect("structured reference expands");
        assert_eq!(expanded, "IF(A1=\"Inputs[value]\",'Data Sheet'!B4:B18,0)");
    }

    #[test]
    fn unknown_column_is_a_typed_error() {
        assert_eq!(
            expand_structured_references("SUM(Inputs[missing])", &workbook()),
            Err(StructuredReferenceError::UnknownColumn {
                table: "Inputs".to_string(),
                column: "missing".to_string(),
            })
        );
    }

    proptest! {
        #[test]
        fn expansion_is_total_over_arbitrary_formula_text(formula in ".{0,2048}") {
            let _ = expand_structured_references(&formula, &workbook());
        }
    }
}
