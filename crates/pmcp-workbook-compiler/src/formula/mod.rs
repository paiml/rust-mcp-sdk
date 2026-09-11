//! Formula front-end (WBCO-03): the tokenizer + Pratt parser that builds the
//! owned AST over the dialect `WHITELIST`, validating function names AT PARSE
//! TIME (the core security primitive).
//!
//! The owned AST (`Expr`/`BinOp`/`UnOp`) lives in `pmcp-workbook-runtime`; this
//! module re-exports it FROM there (so `crate::formula::{Expr, BinOp, UnOp}`
//! resolves) and keeps the compiler-side `token` lexer + `parser` (whitelist-at-
//! parse, depth-limited, typed `ParseError`) + `rebase` (row-block templating).
//!
//! # Parser-error vs lint-finding boundary (Codex MEDIUM)
//!
//! [`parser::parse`] returns a typed [`parser::ParseError`] — it NEVER pushes a
//! `LintFinding`. The linter (`crate::dialect::linter`) owns the collect-all,
//! cell-addressed dialect findings. The two surfaces stay crisp.

/// The Excel-formula tokenizer.
pub mod parser;
/// Per-cell row-offset rebasing for a loop / row-block template.
pub mod rebase;
/// Excel Table structured-reference expansion into the runtime's A1-range IR.
pub(crate) mod structured_ref;
/// The Excel-formula tokenizer.
pub mod token;

// The AST lives in `pmcp-workbook-runtime`; re-export it (NEVER re-declared) so
// `crate::formula::{Expr, BinOp, UnOp}` resolves.
pub use pmcp_workbook_runtime::{BinOp, Expr, UnOp};

pub use parser::{parse, ParseError, MAX_PARSE_DEPTH};
pub use rebase::{rebase, BlockRange};
pub use token::{tokenize, LexError, Token, MAX_FORMULA_LEN};

/// Fuzz-only totality hook for Excel Table structured-reference expansion.
#[cfg(fuzzing)]
pub fn fuzz_expand_structured_references(formula: &str) {
    structured_ref::fuzz_expand_structured_references(formula);
}
