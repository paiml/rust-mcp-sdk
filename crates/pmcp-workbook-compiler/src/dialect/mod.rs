//! Dialect linter stage — runs against the SDK dialect contract (WBDL-03).
//!
//! The linter executes against the `WHITELIST`/`DialectRules`/`CandidateRole`
//! contract owned by `pmcp-workbook-dialect` (Phase 91) and emits the runtime's
//! collect-all `LintFinding`/`LintReport` types — it re-uses both contracts
//! rather than re-declaring a second copy (a second `WHITELIST` would defeat the
//! dialect crate's spec-binding drift test).
//!
//! # The [`CellSource`] seam
//!
//! The linter reads a synthetic, reader-free [`CellSource`] abstraction — NOT
//! 93-02's umya-produced owned cell model — so this plan stays parallel with
//! 93-02. The real owned model implements [`CellSource`] in the Plan 04 wiring;
//! tests here drive a hand-built `TestCells` double.

/// The collect-all, located lint pass over a [`CellSource`] against
/// [`DialectRules`] (WBDL-03).
pub mod linter;

/// Excel's OOXML future-function compatibility prefix, stripped case-insensitively.
///
/// Returns the name WITHOUT the prefix when a leading, dot-terminated `_xlfn.` is
/// present, and `None` otherwise — letting each caller decide whether to allocate.
/// Lookalikes (`_xlfnFOO`, `x_xlfn.FOO`) return `None` and stay subject to the
/// normal whitelist rejection.
///
/// # One copy, and it is the non-panicking one
///
/// The linter and the formula parser must not disagree about what a function is
/// named — that disagreement is how a name passes the lint and then fails to
/// parse. They previously held a copy each, and the copies were not equivalent:
/// the linter's sliced with `ident[..PREFIX.len()]`, which panics when byte 6 of
/// `ident` is not a UTF-8 char boundary (e.g. `_xlfn\u{00e9}`); the parser's used
/// `str::get`, which cannot. This is the parser's form.
///
/// That panic was NOT reachable through the linter's caller, and the claim that it
/// was would itself be the "documented mechanism that does not exist" defect this
/// crate's `// Why:` comments exist to avoid. `strip_xlfn_prefix` has one caller,
/// `scan_function_token`, which passes `read_identifier`'s output, and
/// `read_identifier` advances only over `is_ascii_alphanumeric() || '.' || '_'` —
/// so the ident is ASCII by construction and byte 6 is always a boundary. The
/// non-panicking form is chosen so that WIDENING `read_identifier` to accept
/// non-ASCII function names cannot reintroduce the panic, not because the panic
/// is reachable today.
pub(crate) fn xlfn_stripped(ident: &str) -> Option<&str> {
    const PREFIX: &str = "_xlfn.";
    ident
        .get(..PREFIX.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(PREFIX))
        .then(|| &ident[PREFIX.len()..])
}

#[cfg(test)]
mod xlfn_stripped_tests {
    use super::xlfn_stripped;

    #[test]
    fn strips_only_a_leading_dot_terminated_prefix_case_insensitively() {
        assert_eq!(xlfn_stripped("_xlfn.CONCAT"), Some("CONCAT"));
        assert_eq!(xlfn_stripped("_XLFN.CONCAT"), Some("CONCAT"));
        assert_eq!(xlfn_stripped("_xlfn."), Some(""));
        // Lookalikes stay subject to the normal whitelist rejection.
        assert_eq!(xlfn_stripped("_xlfnFOO"), None);
        assert_eq!(xlfn_stripped("x_xlfn.FOO"), None);
        assert_eq!(xlfn_stripped("SUM"), None);
        assert_eq!(xlfn_stripped(""), None);
    }

    #[test]
    fn a_multi_byte_char_at_the_prefix_boundary_does_not_panic() {
        // The REASON this function uses `str::get` rather than `ident[..6]`. The
        // linter's caller passes ASCII-only idents today, so the slice form could
        // not panic through it — this row is the guard that keeps the safe form if
        // `read_identifier` is ever widened to accept non-ASCII function names.
        assert_eq!(xlfn_stripped("_xlfn\u{00e9}"), None);
        assert_eq!(xlfn_stripped("_xlf\u{00e9}n."), None);
        assert_eq!(xlfn_stripped("\u{00e9}"), None);
    }
}

// The dialect contract the linter runs against — re-exported from the dialect
// crate, NEVER re-declared (a second WHITELIST copy would defeat the dialect
// crate's spec-binding drift test).
pub use pmcp_workbook_dialect::{CandidateRole, DialectRules, WHITELIST};

// The collect-all located lint findings the linter emits — re-exported from the
// runtime, NEVER re-declared.
pub use pmcp_workbook_runtime::{LintFinding, LintReport, Severity};

// The running linter surface.
pub use linter::{
    lint, lint_colour_evidence, lint_workbook_metadata, CellSource, CellView, DefinedName,
    SheetView,
};
