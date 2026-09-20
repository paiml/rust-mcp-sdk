//! The range-capable value the 17 whitelisted-function bodies consume (finding #4).
//!
//! The scalar evaluator ([`super::scalar_eval`]) evaluates ONLY scalar leaf
//! arithmetic — a single [`CellValue`] scalar per leaf. But the Excel-semantics
//! layer ([`super::semantics`]) needs to express RANGE arguments: `SUM(B2:B10)`,
//! `VLOOKUP(table_array, …)`, `INDEX(range, n)`, `MATCH(value, range)`.
//! [`EvalValue`] is the value the function bodies receive so they can accept
//! either a scalar OR a rectangular range of cells. Ranges NEVER enter the
//! scalar evaluator — they stay a semantics-layer concern (D-04, finding #4).

use serde::Serialize;

use super::value::CellValue;

/// A range-capable value: either a single [`CellValue`] scalar, or a
/// rectangular range of cells (`Vec<Vec<CellValue>>`, row-major).
///
/// Derive note: `PartialEq` but NOT `Eq` — [`CellValue::Number`] carries an
/// `f64`, exactly as the underlying [`CellValue`].
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub enum EvalValue {
    /// A single scalar cell value (the only thing that ever lowers to the evaluator).
    Scalar(CellValue),
    /// A rectangular range of cells, row-major (`rows[r][c]`). Used by
    /// `SUM`/`SUMIF`/`VLOOKUP`/`INDEX`/`MATCH`; NEVER enters the evaluator (finding #4).
    Range(Vec<Vec<CellValue>>),
}

impl EvalValue {
    /// View this value as a scalar, if it is one.
    pub fn as_scalar(&self) -> Option<&CellValue> {
        match self {
            EvalValue::Scalar(cv) => Some(cv),
            EvalValue::Range(_) => None,
        }
    }

    /// View this value as a range, if it is one.
    pub fn as_range(&self) -> Option<&Vec<Vec<CellValue>>> {
        match self {
            EvalValue::Range(rows) => Some(rows),
            EvalValue::Scalar(_) => None,
        }
    }

    /// Construct a scalar [`EvalValue`] from a [`CellValue`].
    pub fn scalar(cv: CellValue) -> EvalValue {
        EvalValue::Scalar(cv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::excel_error::ExcelError;

    #[test]
    fn scalar_helper_and_as_scalar_round_trip() {
        let v = EvalValue::scalar(CellValue::Number(42.0));
        assert_eq!(v.as_scalar(), Some(&CellValue::Number(42.0)));
        assert_eq!(v.as_range(), None);
    }

    #[test]
    fn as_range_returns_the_rows() {
        let rows = vec![
            vec![CellValue::Number(1.0), CellValue::Number(2.0)],
            vec![CellValue::Number(3.0), CellValue::Number(4.0)],
        ];
        let v = EvalValue::Range(rows.clone());
        assert_eq!(v.as_range(), Some(&rows));
        assert_eq!(v.as_scalar(), None);
    }

    #[test]
    fn scalar_can_carry_an_error() {
        let v = EvalValue::Scalar(CellValue::Error(ExcelError::Na));
        assert_eq!(v.as_scalar(), Some(&CellValue::Error(ExcelError::Na)));
    }

    #[test]
    fn serializes_externally_tagged() {
        let v = EvalValue::Scalar(CellValue::Number(238.728));
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["Scalar"]["Number"], 238.728);
    }
}
