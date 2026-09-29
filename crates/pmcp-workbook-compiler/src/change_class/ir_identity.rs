//! The CANONICAL IR sub-DAG identity hash (D-14, WBGV-03).
//!
//! [`ir_subdag_hash`] computes a STABLE hex hash of the dependency-ordered sub-DAG
//! that computes a named output. It is the formula-IDENTITY component of BOTH the
//! `FormulaLogic` class derivation ([`super::classify`]) and the redefinition
//! predicate ([`super::diff_outputs`]).
//!
//! The hash is ORDER-INDEPENDENT: it visits ONLY the transitive precedent set of
//! the output cell, traversed in a DETERMINISTIC dependency order (topological
//! order with ties broken by lexicographic cell-key sort). An unrelated cell
//! elsewhere in the IR map is never visited, so it cannot perturb the result. A
//! change to a node WITHIN the sub-DAG (operator/operand/precedent edge) DOES
//! change the hash — distinguishing numeric drift from semantic redefinition.
//!
//! The fold goes through the SHARED length-prefixed
//! [`pmcp_workbook_runtime::update_field`] helper, hex-encoded at the end — NEVER
//! the cell's Debug rendering and NEVER raw HashMap iteration. Each visited node
//! contributes a CANONICAL serialization of its expression (`serde_json` of the
//! owned `CellExpr`, whose field order is declaration-stable and map-free).

use std::collections::{BTreeSet, HashMap};

use pmcp_workbook_runtime::artifact_model::update_field;
use pmcp_workbook_runtime::formula::Expr;
use pmcp_workbook_runtime::range_ref::RangeRef;
use pmcp_workbook_runtime::sheet_ir::{Cell, CellExpr};

/// Compute the STABLE CANONICAL hash of the IR sub-DAG that computes
/// `output_region`.
///
/// If `output_region` is absent from the IR map, the hash folds only the output
/// tag + region (a stable hash for "no such sub-DAG") — never a panic.
#[must_use]
pub fn ir_subdag_hash(output_region: &str, ir: &HashMap<String, Cell>) -> String {
    use sha2::{Digest, Sha256};

    // (1) Transitive precedent set, INCLUDING the output cell itself when present.
    let visited = collect_subdag(output_region, ir);

    // (2) Deterministic dependency order: topological order over the sub-DAG, ties
    //     broken lexicographically, over ONLY the visited set.
    let order = dependency_order(&visited, ir);

    // (3) Length-prefixed canonical fold via the shared helper.
    let mut hasher = Sha256::new();
    update_field(&mut hasher, b"output", output_region.as_bytes());
    for key in &order {
        update_field(&mut hasher, b"node-key", key.as_bytes());
        let body = canonical_expr(ir.get(key));
        update_field(&mut hasher, b"node-expr", body.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// The canonical serialization of a cell's expression: stable `serde_json` of the
/// owned [`CellExpr`] (declaration-ordered, map-free — never `Debug`). An absent
/// node folds a fixed sentinel.
fn canonical_expr(cell: Option<&Cell>) -> String {
    match cell {
        Some(c) => {
            serde_json::to_string(&c.expr).unwrap_or_else(|_| "\"<unserializable>\"".to_string())
        },
        None => "\"<absent-leaf>\"".to_string(),
    }
}

/// Collect the transitive precedent set of `output_region` (including the output
/// cell key itself), following the expression edges. A cyclic IR is bounded by the
/// visited set (a node is expanded at most once).
fn collect_subdag(output_region: &str, ir: &HashMap<String, Cell>) -> BTreeSet<String> {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![output_region.to_string()];
    while let Some(key) = stack.pop() {
        if !visited.insert(key.clone()) {
            continue;
        }
        if let Some(cell) = ir.get(&key) {
            for precedent in precedents_of(cell) {
                if !visited.contains(&precedent) {
                    stack.push(precedent);
                }
            }
        }
    }
    visited
}

/// The precedent cell keys a [`Cell`] depends on, extracted by walking its
/// expression AST for `Ref` / `Range` / `Name` leaves.
fn precedents_of(cell: &Cell) -> Vec<String> {
    let mut out = Vec::new();
    match &cell.expr {
        CellExpr::Literal(_) => {},
        CellExpr::Formula(expr) => collect_refs(expr, &mut out),
    }
    out
}

/// Walk an [`Expr`] collecting referenced cell keys (`Ref`/`Range`/`Name`).
fn collect_refs(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Ref(key) => out.push(key.clone()),
        Expr::Name(name) => out.push(name.clone()),
        Expr::Range(range) => out.extend(expand_range_keys(range)),
        Expr::BinaryOp { left, right, .. } => {
            collect_refs(left, out);
            collect_refs(right, out);
        },
        Expr::UnaryOp { operand, .. } => collect_refs(operand, out),
        Expr::Call { args, .. } => {
            for a in args {
                collect_refs(a, out);
            }
        },
        Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::ErrorLit(_) => {},
    }
}

/// Expand a [`RangeRef`] into its member cell keys (`sheet!addr`) in row-major
/// order. A malformed A1 endpoint yields the structural `sheet!start:end` key as a
/// single stable token (never a panic).
fn expand_range_keys(range: &RangeRef) -> Vec<String> {
    match (parse_a1(&range.start), parse_a1(&range.end)) {
        (Some((c0, r0)), Some((c1, r1))) => {
            let (cmin, cmax) = (c0.min(c1), c0.max(c1));
            let (rmin, rmax) = (r0.min(r1), r0.max(r1));
            let mut keys = Vec::new();
            for r in rmin..=rmax {
                for c in cmin..=cmax {
                    keys.push(format!("{}!{}{}", range.sheet, col_to_a1(c), r));
                }
            }
            keys
        },
        _ => vec![format!("{}!{}:{}", range.sheet, range.start, range.end)],
    }
}

/// Parse a bare A1 address (e.g. `"B10"`) into `(column_index, row)` (1-based
/// column, 1-based row). Returns `None` for a malformed address.
fn parse_a1(addr: &str) -> Option<(u32, u32)> {
    let split = addr.find(|ch: char| ch.is_ascii_digit())?;
    let (col_part, row_part) = addr.split_at(split);
    if col_part.is_empty() || !col_part.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut col: u32 = 0;
    for ch in col_part.chars() {
        col = col
            .checked_mul(26)?
            .checked_add(u32::from(ch.to_ascii_uppercase()) - 64)?;
    }
    let row: u32 = row_part.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((col, row))
}

/// Convert a 1-based column index back to A1 letters (e.g. `1 -> "A"`, `27 -> "AA"`).
///
/// Delegates to [`crate::index_to_col`], the crate's single bijective-base-26
/// encoder. This module and `formula::structured_ref` each held a byte-identical
/// private copy; a future A1 fix must not have to be applied in three places.
fn col_to_a1(col: u32) -> String {
    crate::index_to_col(col)
}

/// The in-`visited` precedents of one IR cell key (the cell's precedents
/// restricted to the visited sub-DAG). Missing keys contribute no deps.
fn visited_precedents(
    key: &str,
    ir: &HashMap<String, Cell>,
    visited: &BTreeSet<String>,
) -> Vec<String> {
    ir.get(key)
        .map(|c| {
            precedents_of(c)
                .into_iter()
                .filter(|p| visited.contains(p))
                .collect()
        })
        .unwrap_or_default()
}

/// Build the adjacency map `key -> its in-visited precedents` for the sub-DAG.
fn build_deps<'a>(
    visited: &'a BTreeSet<String>,
    ir: &HashMap<String, Cell>,
) -> HashMap<&'a str, Vec<String>> {
    visited
        .iter()
        .map(|k| (k.as_str(), visited_precedents(k, ir, visited)))
        .collect()
}

/// After `key` is emitted, decrement the remaining-precedent count of every node
/// that depends on `key`.
fn decrement_dependents<'a>(
    key: &str,
    deps: &HashMap<&'a str, Vec<String>>,
    remaining: &mut HashMap<&'a str, usize>,
) {
    for (other, d) in deps {
        if remaining.contains_key(other) && d.iter().any(|p| p == key) {
            if let Some(n) = remaining.get_mut(other) {
                *n = n.saturating_sub(1);
            }
        }
    }
}

/// The set of nodes with zero remaining precedents, sorted lexicographically.
fn ready_nodes<'a>(remaining: &HashMap<&'a str, usize>) -> Vec<&'a str> {
    let mut ready: Vec<&str> = remaining
        .iter()
        .filter(|(_, &n)| n == 0)
        .map(|(k, _)| *k)
        .collect();
    ready.sort_unstable();
    ready
}

/// Order the visited sub-DAG nodes in a DETERMINISTIC dependency order: precedents
/// before dependents, ties broken by lexicographic key sort. Thin Kahn's-algorithm
/// driver over the helpers above.
fn dependency_order(visited: &BTreeSet<String>, ir: &HashMap<String, Cell>) -> Vec<String> {
    let deps = build_deps(visited, ir);
    let mut remaining: HashMap<&str, usize> = deps.iter().map(|(k, d)| (*k, d.len())).collect();

    let mut order: Vec<String> = Vec::with_capacity(visited.len());
    loop {
        let ready = ready_nodes(&remaining);
        if ready.is_empty() {
            break;
        }
        for key in ready {
            remaining.remove(key);
            order.push(key.to_string());
            decrement_dependents(key, &deps, &mut remaining);
        }
    }

    // Append any residual cyclic nodes in sorted order (totality).
    let mut residual: Vec<&str> = remaining.keys().copied().collect();
    residual.sort_unstable();
    order.extend(residual.into_iter().map(str::to_string));
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::CellValue;

    fn formula_cell(key: &str, expr: Expr) -> Cell {
        Cell {
            key: key.to_string(),
            expr: CellExpr::Formula(expr),
        }
    }

    fn literal_cell(key: &str, n: f64) -> Cell {
        Cell {
            key: key.to_string(),
            expr: CellExpr::Literal(CellValue::Number(n)),
        }
    }

    fn insert(ir: &mut HashMap<String, Cell>, cell: Cell) {
        ir.insert(cell.key.clone(), cell);
    }

    /// out = A + B, with A and B literal leaves.
    fn base_ir() -> HashMap<String, Cell> {
        let mut ir = HashMap::new();
        insert(&mut ir, literal_cell("S!A1", 10.0));
        insert(&mut ir, literal_cell("S!B1", 5.0));
        insert(
            &mut ir,
            formula_cell(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: pmcp_workbook_runtime::BinOp::Add,
                    right: Box::new(Expr::Ref("S!B1".to_string())),
                },
            ),
        );
        ir
    }

    #[test]
    fn reordering_unrelated_cell_preserves_identity() {
        let ir = base_ir();
        let baseline = ir_subdag_hash("S!C1", &ir);

        let mut with_unrelated = base_ir();
        insert(&mut with_unrelated, literal_cell("S!Z9", 999.0));
        insert(
            &mut with_unrelated,
            formula_cell(
                "S!Y9",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!Z9".to_string())),
                    op: pmcp_workbook_runtime::BinOp::Mul,
                    right: Box::new(Expr::Number(2.0)),
                },
            ),
        );
        let with_extra = ir_subdag_hash("S!C1", &with_unrelated);

        assert_eq!(
            baseline, with_extra,
            "an unrelated cell must NOT change the output's identity hash"
        );
    }

    #[test]
    fn numeric_drift_vs_semantic_redefinition() {
        // WBGV-03: a numeric-only change to a LITERAL precedent changes the hash
        // (the value is part of the sub-DAG identity); changing the OPERATOR
        // (a semantic redefinition) also changes it — both are detectable, and the
        // hash stays equal when nothing in the sub-DAG changed.
        let ir = base_ir();
        let baseline = ir_subdag_hash("S!C1", &ir);

        // Re-emit of the SAME sub-DAG is stable.
        assert_eq!(
            baseline,
            ir_subdag_hash("S!C1", &base_ir()),
            "stable re-emit"
        );

        // A numeric change to a literal precedent within the sub-DAG.
        let mut numeric = base_ir();
        insert(&mut numeric, literal_cell("S!A1", 11.0));
        assert_ne!(
            baseline,
            ir_subdag_hash("S!C1", &numeric),
            "a numeric change within the sub-DAG changes identity"
        );

        // A semantic redefinition (operator change).
        let mut semantic = base_ir();
        insert(
            &mut semantic,
            formula_cell(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: pmcp_workbook_runtime::BinOp::Mul, // was Add
                    right: Box::new(Expr::Ref("S!B1".to_string())),
                },
            ),
        );
        assert_ne!(
            baseline,
            ir_subdag_hash("S!C1", &semantic),
            "an operator change is a semantic redefinition"
        );
    }

    #[test]
    fn structurally_identical_subdags_hash_equal_regardless_of_insertion_order() {
        let a = base_ir();
        let mut b = HashMap::new();
        insert(
            &mut b,
            formula_cell(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: pmcp_workbook_runtime::BinOp::Add,
                    right: Box::new(Expr::Ref("S!B1".to_string())),
                },
            ),
        );
        insert(&mut b, literal_cell("S!B1", 5.0));
        insert(&mut b, literal_cell("S!A1", 10.0));
        assert_eq!(ir_subdag_hash("S!C1", &a), ir_subdag_hash("S!C1", &b));
    }

    #[test]
    fn absent_output_hashes_without_panic() {
        let ir = base_ir();
        let h = ir_subdag_hash("S!NOPE", &ir);
        assert_eq!(h.len(), 64, "hash is a 64-char sha256 hex");
    }

    #[test]
    fn range_precedents_are_expanded_deterministically() {
        let mut ir = HashMap::new();
        insert(&mut ir, literal_cell("S!B2", 1.0));
        insert(&mut ir, literal_cell("S!B3", 2.0));
        insert(
            &mut ir,
            formula_cell(
                "S!C1",
                Expr::Call {
                    name: "SUM".to_string(),
                    args: vec![Expr::Range(RangeRef {
                        sheet: "S".to_string(),
                        start: "B2".to_string(),
                        end: "B3".to_string(),
                    })],
                },
            ),
        );
        let h1 = ir_subdag_hash("S!C1", &ir);
        insert(&mut ir, literal_cell("S!B3", 99.0));
        let h2 = ir_subdag_hash("S!C1", &ir);
        assert_ne!(h1, h2, "a range member change is a sub-DAG change");
    }
}
