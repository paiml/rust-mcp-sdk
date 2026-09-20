//! ALWAYS example (CLAUDE.md, NO EXCEPTIONS): the dialect 1.1 widening and the
//! exact-match narrowing, shown through the BA-facing messages.
//!
//! This uses ONLY the crate's PUBLIC API
//! ([`pmcp_workbook_compiler::formula::parse`]) — pure computation, no `.xlsx`,
//! no filesystem. It demonstrates:
//!
//!   (a) the four functions dialect 1.1 ADDS — `ROUNDDOWN`, `MAX`, `MIN`,
//!       `XLOOKUP` — parsing clean;
//!   (b) the exact-match lookup shapes the repo's own fixtures already author
//!       (`VLOOKUP(…, FALSE)` / `MATCH(…, 0)`) still accepted;
//!   (c) the four refusals dialect 1.1 ADDS, each printing the LOCATED repair
//!       message a BA would actually be given.
//!
//! Every `Err` below is EXPECTED: it is matched and printed, NOT propagated. The
//! process exits 0.
//!
//! Run with: `cargo run -p pmcp-workbook-compiler --example dialect_widening_demo`

use pmcp_workbook_compiler::formula::parse;
use pmcp_workbook_dialect::{SUPPORTED_DIALECT_VERSION, WHITELIST};

/// The sheet/address a real compile would key this cell on. The compiler wraps a
/// `ParseError` as `CompileError::Lint("parse {sheet}!{addr}: {e}")`, so the
/// refusal a BA sees is always cell-located; this demo prints the same prefix.
const SHEET: &str = "1_Inputs";
const ADDR: &str = "B7";

fn main() {
    println!(
        "constrained-dialect widening demo — dialect {SUPPORTED_DIALECT_VERSION}, \
         {} whitelisted functions\n",
        WHITELIST.len()
    );

    println!("(a) the four functions dialect 1.1 ADDS:");
    for formula in [
        "ROUNDDOWN(A1,2)",
        "MAX(A1:A9)",
        "MIN(A1:A9,B1)",
        "XLOOKUP(B2,D2:D4,E2:E4)",
        "XLOOKUP(B2,D2:D4,E2:E4,0.08)",
    ] {
        report(formula);
    }

    println!("\n(b) the exact-match lookup shapes the fixtures already author:");
    for formula in [
        "VLOOKUP(B2,D2:E4,2,FALSE)",
        "VLOOKUP(B2,D2:E4,2,0)",
        "IFERROR(INDEX(E2:E4,MATCH(B2,D2:D4,0)),0.08)",
    ] {
        report(formula);
    }

    println!("\n(c) the refusals dialect 1.1 ADDS — each prints its located repair:");
    println!("    (Excel's DEFAULT for both omitted arguments is APPROXIMATE, so an");
    println!("     omitted argument is refused as firmly as an explicit approximate one.)");
    for formula in [
        // The form that silently returned a wrong number before dialect 1.1.
        "MATCH(B2,D2:D4)",
        "MATCH(B2,D2:D4,1)",
        "VLOOKUP(B2,D2:E4,2)",
        "XLOOKUP(B2,D2:D4,E2:E4,0,0)",
        // Statically-knowable XLOOKUP array violations: both array arguments are
        // literal ranges, so the extents are measurable without evaluating.
        "XLOOKUP(B2,D2:D4,E2:F4)",
        "XLOOKUP(B2,D2:D4,E2:E9)",
    ] {
        report(formula);
    }

    println!("\n(d) a shape the gate cannot MEASURE is never guessed at — it passes");
    println!("    the static gate and reaches the evaluator's typed backstop instead:");
    report("XLOOKUP(B2,named_keys,named_values)");

    println!("\ndemo complete (every refusal above is expected, printed, and located)");
}

/// Parse one formula and print either its acceptance or its LOCATED refusal,
/// mirroring the `parse {sheet}!{addr}: {e}` wrapping `compile` applies.
fn report(formula: &str) {
    match parse(formula, SHEET, ADDR) {
        Ok(_) => println!("    accepted  {formula}"),
        Err(e) => println!("    REFUSED   {formula}\n              parse {SHEET}!{ADDR}: {e}"),
    }
}
