//! `sheet_ir` skeleton + the Excel-semantics layer (CMP-03).
//!
//! Holds the eval-boundary value type ([`value::CellValue`], D-04), the
//! range-capable [`eval_value::EvalValue`] (finding #4), the D-03/D-04
//! [`eval_bridge`] (now over the PURE-RUST [`crate::scalar_eval`], Phase 11 Plan
//! 05), the deterministic [`rounding`] helpers, the [`semantics`] bodies for all
//! 17 whitelisted functions, and the topo-ordered [`executor`] SERVE-time
//! [`executor::run`].
//!
//! The `Cell`/`CellExpr` skeleton below is the per-cell IR unit the topo executor
//! fills and runs.

use serde::{Deserialize, Serialize};

pub mod eval_bridge;
pub mod eval_value;
pub mod executor;
pub mod rounding;
pub mod semantics;
pub mod value;

pub use eval_bridge::CellEnv;
pub use eval_value::EvalValue;
pub use executor::{build_dag, run, EvalTrace, RunResult};
pub use value::{CellValue, ExcelError};

/// The expression a [`Cell`] holds: either a parsed formula AST or a literal
/// cell value (a constant / input cell with no `<f>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CellExpr {
    /// A parsed formula expression (the [`crate::formula::Expr`] AST).
    Formula(crate::formula::Expr),
    /// A literal value (a constant or input cell carrying no formula).
    Literal(CellValue),
}

/// A single cell in the `sheet_ir`: its canonical `cell_key(sheet, addr)` and
/// the expression it evaluates to. The topo executor walks these in dependency
/// order, lowering each [`CellExpr::Formula`]'s leaf arithmetic through
/// [`eval_bridge`] and dispatching its calls through [`semantics`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Cell {
    /// The canonical cell key (`sheet!addr`, e.g. `"5_Quantities!C6"`).
    pub key: String,
    /// The expression this cell computes.
    pub expr: CellExpr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_holds_a_literal_expr() {
        let c = Cell {
            key: "2_Constants!C17".to_string(),
            expr: CellExpr::Literal(CellValue::Number(1.05)),
        };
        let j = serde_json::to_value(&c).unwrap();
        assert_eq!(j["key"], "2_Constants!C17");
        assert_eq!(j["expr"]["Literal"]["Number"], 1.05);
    }

    #[test]
    fn cell_holds_a_formula_expr() {
        let c = Cell {
            key: "S!A1".to_string(),
            expr: CellExpr::Formula(crate::formula::Expr::Number(2.0)),
        };
        let j = serde_json::to_value(&c).unwrap();
        assert_eq!(j["expr"]["Formula"]["Number"], 2.0);
    }
}
