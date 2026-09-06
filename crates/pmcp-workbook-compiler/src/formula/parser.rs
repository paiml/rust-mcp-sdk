//! The recursive-descent + Pratt parser (WBCO-03): turns the
//! [`super::token::Token`] stream into an owned [`Expr`] AST over the dialect
//! `WHITELIST`, validating function names AT PARSE TIME (the core security
//! primitive).
//!
//! # The whitelist gate (the core security primitive, T-93-03-INJ)
//!
//! When a `FuncOpen(name)` is parsed, the parser checks `name` against
//! [`WHITELIST`](pmcp_workbook_dialect::WHITELIST) IMMEDIATELY. On a MISS it
//! returns a typed [`ParseError::UnsupportedFunction`] — an out-of-whitelist
//! function is a parse-time REJECTION, never a silent accept, and never a
//! parsed-then-filtered node. This is the one hard rule: a rejected function
//! cannot reach the IR.
//!
//! # Parser error vs lint finding — a crisp boundary (Codex MEDIUM)
//!
//! The parser returns a typed [`ParseError`]; it NEVER pushes a `LintFinding`.
//! Emitting located dialect findings is the LINTER's job (`dialect::linter`).
//! Keeping the two surfaces distinct means the parser stays a pure
//! text→IR function with a typed failure mode, and the linter owns the
//! collect-all, cell-addressed reporting.
//!
//! # The supported-formula matrix (tied to the dialect `WHITELIST`)
//!
//! | Construct                       | Supported | On reject (typed error)            |
//! |---------------------------------|-----------|------------------------------------|
//! | sheet-qualified ref `S!A1`      | yes       | —                                  |
//! | quoted sheet name `'My S'!A1`   | yes       | —                                  |
//! | A1 range `A1:B2`                | yes       | —                                  |
//! | sheet-qualified range `S!A1:B2` | yes       | — (sheet recorded once)            |
//! | absolute ref `$A$1` / `$A1`     | yes       | — (anchors stripped at DAG layer)  |
//! | unary `+` / `-`                 | yes       | —                                  |
//! | postfix percent `x%`            | yes       | —                                  |
//! | string literal `"…"`            | yes       | —                                  |
//! | numeric / scientific `1.5E3`    | yes       | —                                  |
//! | boolean `TRUE` / `FALSE`        | yes       | —                                  |
//! | error value `#REF!`/`#DIV/0!`   | yes       | — (parsed to `Expr::ErrorLit`)     |
//! | whitelisted call (e.g. `ROUND`) | yes       | —                                  |
//! | out-of-whitelist call `OFFSET`  | NO        | `ParseError::UnsupportedFunction`  |
//! | external-workbook ref `[B]S!A1` | NO        | `ParseError::ExternalRef`          |
//! | array-formula braces `{…}`      | NO        | `ParseError::ArrayFormula`         |
//! | nesting past `MAX_PARSE_DEPTH`  | NO        | `ParseError::TooDeep`              |
//! | `VLOOKUP(k,t,c)` (no 4th arg)   | NO        | `ParseError::UnsupportedCallShape` |
//! | `VLOOKUP(k,t,c,TRUE)` / `…,1)`  | NO        | `ParseError::UnsupportedCallShape` |
//! | `MATCH(k,r)` (no match_type)    | NO        | `ParseError::UnsupportedCallShape` |
//! | `MATCH(k,r,1)` / `MATCH(k,r,-1)`| NO        | `ParseError::UnsupportedCallShape` |
//! | `XLOOKUP` w/ match/search_mode  | NO        | `ParseError::UnsupportedCallShape` |
//! | `XLOOKUP` non-conformable arrays| NO        | `ParseError::UnsupportedCallShape` |
//!
//! # The call-shape gate (dialect 1.1, D-Q1 / D-Q4)
//!
//! A WHITELISTED name may still be called in a shape the dialect refuses. After
//! the whitelist gate accepts the name and the argument list is collected,
//! `validate_call_shape` runs BEFORE the node is built. The refusals it raises
//! are dialect POLICY carrying a BA-actionable repair — distinct from
//! [`ParseError::Malformed`], which is a structural failure.
//!
//! The lookup rules exist because Excel's DEFAULT for `VLOOKUP`'s
//! `range_lookup` and `MATCH`'s `match_type` is APPROXIMATE. Before dialect 1.1
//! a bare `MATCH(x, rng)` silently evaluated as EXACT and returned a wrong
//! number with no signal; refusing it is the fix. The evaluator carries an
//! independent `#VALUE!` backstop for the same conditions.
//!
//! A bare NAME not followed by `(` is `Expr::Name(n)` (resolved at the DAG
//! layer) — DISTINCT from an out-of-whitelist function (it is NOT a parse
//! error).
//!
//! # Precedence (Microsoft table)
//!
//! reference op `:` (highest, folded in the primary) → unary `-`/`+`/`%` → `^`
//! (RIGHT-assoc) → `*`/`/` → `+`/`-` → `&` → comparisons (lowest, left-assoc).
//! Because unary binds TIGHTER than `^`, `-10^2` parses as `(-10)^2` (== 100)
//! and `2^3^2` is right-associative (`2^(3^2)`).
//!
//! # DoS guard (T-93-03-DOS)
//!
//! Recursion DEPTH is bounded by [`MAX_PARSE_DEPTH`]; past the cap the parser
//! returns [`ParseError::TooDeep`] instead of overflowing the stack. The limit
//! is enforced IN CODE before fuzzing — not discovered by the fuzzer.
//! Panic-freedom holds (lib.rs gate): every fallible step returns a `Result`,
//! never `.unwrap()`.

use pmcp_workbook_dialect::WHITELIST;
use pmcp_workbook_runtime::resolve::a1_to_zero_indexed_row_col;
use pmcp_workbook_runtime::{BinOp, ExcelError, Expr, RangeRef, UnOp};
use serde::Serialize;

use crate::formula::token::{tokenize, LexError, Token};

/// The maximum expression-nesting depth (T-93-03-DOS DoS guard). 256 is
/// generous yet bounded; a formula nesting deeper is rejected with
/// [`ParseError::TooDeep`] rather than overflowing the stack.
pub const MAX_PARSE_DEPTH: usize = 256;

/// The typed parse failure surface. The parser returns these — it NEVER pushes
/// a `LintFinding` (that is the linter's job; this keeps the parser-error vs
/// lint-finding boundary crisp — Codex MEDIUM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum ParseError {
    /// The tokenizer rejected the input (oversized, unterminated, bad char).
    Lex(LexError),
    /// A function call names a function NOT in the dialect `WHITELIST` — the
    /// core security rejection (T-93-03-INJ). Carries the offending name.
    UnsupportedFunction(String),
    /// An external/workbook reference (`[Book.xlsx]Sheet1!A1`) — the dialect
    /// never resolves cross-workbook refs. Carries the raw lexeme.
    ExternalRef(String),
    /// An array-formula brace `{ }` — non-scalar semantics are out of dialect.
    ArrayFormula,
    /// Expression nesting exceeded [`MAX_PARSE_DEPTH`] (DoS guard).
    TooDeep,
    /// The token stream is not a structurally valid expression (unbalanced
    /// parens, a stray operator, a malformed argument list, trailing tokens, an
    /// unexpected end). Carries a short human description.
    Malformed(String),
    /// A WHITELISTED function was called in a shape the dialect does not
    /// support — an approximate or omitted lookup argument, or an `XLOOKUP`
    /// `match_mode`/`search_mode` (D-Q1 / D-Q4, dialect 1.1).
    ///
    /// Deliberately NOT folded into [`ParseError::Malformed`]: this is a dialect
    /// POLICY decision carrying a BA-actionable `repair`, not a structural parse
    /// failure, and a caller must be able to tell the two apart.
    UnsupportedCallShape {
        /// The offending function name, as authored.
        name: String,
        /// The literal repair to write instead.
        repair: String,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Lex(e) => write!(f, "lex error: {e:?}"),
            ParseError::UnsupportedFunction(name) => {
                write!(f, "function `{name}` is not in the dialect whitelist")
            },
            ParseError::ExternalRef(raw) => {
                write!(f, "external-workbook reference {raw} is out of dialect")
            },
            ParseError::ArrayFormula => write!(f, "array-formula braces {{ }} are out of dialect"),
            ParseError::TooDeep => write!(
                f,
                "formula nesting exceeds the {MAX_PARSE_DEPTH}-level bound"
            ),
            ParseError::Malformed(msg) => write!(f, "malformed formula: {msg}"),
            ParseError::UnsupportedCallShape { name, repair } => {
                write!(f, "call to `{name}` has an unsupported shape: {repair}")
            },
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a formula (text WITHOUT a leading `=`) into an [`Expr`], validating
/// function names against the dialect `WHITELIST` at parse time (the core
/// security primitive).
///
/// On success returns the owned `Expr`. On any rejection returns a typed
/// [`ParseError`] — an out-of-whitelist function is [`ParseError::UnsupportedFunction`],
/// nesting past [`MAX_PARSE_DEPTH`] is [`ParseError::TooDeep`], etc. The
/// `sheet`/`addr` are accepted for caller symmetry (the DAG layer keys on them);
/// the parser itself is location-free.
///
/// # Errors
/// Returns [`ParseError`] for an unsupported function, external ref, array
/// brace, over-deep nesting, a lex error, or a structurally malformed formula.
pub fn parse(formula: &str, _sheet: &str, _addr: &str) -> Result<Expr, ParseError> {
    let tokens = tokenize(formula).map_err(ParseError::Lex)?;
    let mut p = Parser { tokens, pos: 0 };
    let expr = p.parse_expr(0, 0)?;
    if p.pos != p.tokens.len() {
        return Err(ParseError::Malformed(
            "unexpected trailing tokens after a complete expression".to_string(),
        ));
    }
    Ok(expr)
}

/// The parser state over an owned token slice.
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Enforce the recursion-DEPTH bound (T-93-03-DOS): past [`MAX_PARSE_DEPTH`],
    /// return [`ParseError::TooDeep`] so the caller's `?` unwinds the parse
    /// without overflowing the stack.
    fn check_depth(&self, depth: usize) -> Result<(), ParseError> {
        if depth > MAX_PARSE_DEPTH {
            return Err(ParseError::TooDeep);
        }
        Ok(())
    }

    /// Pratt precedence-climbing entry. `min_bp` is the minimum left binding
    /// power that may bind here; `depth` enforces the DoS bound.
    fn parse_expr(&mut self, min_bp: u8, depth: usize) -> Result<Expr, ParseError> {
        self.check_depth(depth)?;

        let mut lhs = self.parse_prefix(depth)?;

        // Bind while the next token is an infix operator (stop on EOF / non-op).
        while let Some(op) = self.peek().and_then(binop_of) {
            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }
            self.next(); // consume the operator
            let rhs = self.parse_expr(r_bp, depth + 1)?;
            lhs = Expr::BinaryOp {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    /// Parse a prefix position: unary operators, then a primary; a postfix `%`
    /// binds tighter than `^` so it is folded here too.
    fn parse_prefix(&mut self, depth: usize) -> Result<Expr, ParseError> {
        self.check_depth(depth)?;

        let mut expr = match self.peek() {
            Some(Token::Minus) => {
                self.next();
                // unary binds TIGHTER than `^` (Microsoft): operand parses with
                // a binding power ABOVE `^`'s, so `-10^2` == `(-10)^2`.
                let operand = self.parse_prefix(depth + 1)?;
                Expr::UnaryOp {
                    op: UnOp::Neg,
                    operand: Box::new(operand),
                }
            },
            Some(Token::Plus) => {
                self.next();
                let operand = self.parse_prefix(depth + 1)?;
                Expr::UnaryOp {
                    op: UnOp::Pos,
                    operand: Box::new(operand),
                }
            },
            _ => self.parse_primary(depth)?,
        };

        // Postfix `%` (binds tighter than `^`).
        while matches!(self.peek(), Some(Token::Percent)) {
            self.next();
            expr = Expr::UnaryOp {
                op: UnOp::Percent,
                operand: Box::new(expr),
            };
        }

        Ok(expr)
    }

    /// Parse a primary atom: literals, refs/ranges, parenthesized, function calls.
    fn parse_primary(&mut self, depth: usize) -> Result<Expr, ParseError> {
        let tok = self.next().ok_or_else(|| {
            ParseError::Malformed(
                "unexpected end of formula where a value was expected".to_string(),
            )
        })?;

        match tok {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::Str(s) => Ok(Expr::Str(s)),
            Token::ErrorLit(raw) => Ok(Expr::ErrorLit(error_of(&raw))),
            Token::CellRef(s) => self.parse_ref_or_range(s),
            Token::Name(n) => Ok(match n.to_ascii_uppercase().as_str() {
                "TRUE" => Expr::Bool(true),
                "FALSE" => Expr::Bool(false),
                // A bare name is the DAG's concern — NOT an unsupported function.
                _ => Expr::Name(n),
            }),
            Token::FuncOpen(name) => self.parse_call(name, depth),
            // T-93-03-INJ-adjacent: never resolve a cross-workbook ref.
            Token::ExternalRef(raw) => Err(ParseError::ExternalRef(raw)),
            Token::LParen => {
                let inner = self.parse_expr(0, depth + 1)?;
                match self.next() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err(ParseError::Malformed(
                        "missing closing parenthesis".to_string(),
                    )),
                }
            },
            // Array-formula brace — a typed rejection, never a panic.
            Token::LBrace => Err(ParseError::ArrayFormula),
            other => Err(ParseError::Malformed(format!(
                "unexpected token {other:?} where a value was expected"
            ))),
        }
    }

    /// A `CellRef` primary: fold a following `: CellRef` into one `Expr::Range`
    /// with the sheet recorded ONCE; otherwise it is an `Expr::Ref`.
    fn parse_ref_or_range(&mut self, first: String) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Some(Token::Colon)) {
            self.next(); // consume `:`
            let end_tok = match self.next() {
                Some(Token::CellRef(s)) => s,
                _ => {
                    return Err(ParseError::Malformed(
                        "range operator ':' not followed by a cell reference".to_string(),
                    ))
                },
            };
            let (sheet, start) = split_sheet(&first);
            // The end operand carries no sheet qualifier in a well-formed range;
            // if it somehow does, drop the duplicate (sheet recorded once).
            let (_end_sheet, end) = split_sheet(&end_tok);
            let sheet = sheet.unwrap_or_default();
            Ok(Expr::Range(RangeRef { sheet, start, end }))
        } else {
            Ok(Expr::Ref(first))
        }
    }

    /// Parse the argument list of a function call whose opener `name(` was
    /// already consumed, then check `name` against the WHITELIST AT PARSE TIME
    /// (the core security primitive). An out-of-whitelist function is a typed
    /// [`ParseError::UnsupportedFunction`] — the node is NEVER built for a
    /// rejected function.
    fn parse_call(&mut self, name: String, depth: usize) -> Result<Expr, ParseError> {
        // Whitelist-at-parse-time: reject BEFORE building the node. An
        // out-of-whitelist function never reaches the IR.
        if !WHITELIST.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
            return Err(ParseError::UnsupportedFunction(name));
        }

        let mut args: Vec<Expr> = Vec::new();

        // Empty arg list: `NAME()`.
        if matches!(self.peek(), Some(Token::RParen)) {
            self.next();
        } else {
            loop {
                let arg = self.parse_expr(0, depth + 1)?;
                args.push(arg);
                match self.next() {
                    Some(Token::Comma) => continue,
                    Some(Token::RParen) => break,
                    _ => {
                        return Err(ParseError::Malformed(format!(
                            "malformed argument list in call to {name}"
                        )))
                    },
                }
            }
        }

        // Call-shape gate (dialect 1.1): runs AFTER the whitelist gate and
        // BEFORE the node is built, so a refused shape never reaches the IR.
        validate_call_shape(&name, &args)?;

        Ok(Expr::Call { name, args })
    }
}

/// The dialect's per-function CALL-SHAPE gate (dialect 1.1): a whitelisted name
/// may still be called in a shape the dialect refuses.
///
/// Kept a free function rather than inline arms in `parse_call`, and split into
/// one small helper per function family, so neither this nor `parse_call` grows
/// past the cognitive-complexity cap. A name with no shape rule is `Ok(())`.
///
/// Matching is case-insensitive, mirroring the whitelist gate's
/// `eq_ignore_ascii_case`, so `vlookup(` reaches the same rule as `VLOOKUP(`.
///
/// # Errors
/// [`ParseError::UnsupportedCallShape`] carrying the offending name and a
/// literal, copy-pasteable repair.
fn validate_call_shape(name: &str, args: &[Expr]) -> Result<(), ParseError> {
    if name.eq_ignore_ascii_case("VLOOKUP") {
        return validate_vlookup_shape(name, args);
    }
    if name.eq_ignore_ascii_case("MATCH") {
        return validate_match_shape(name, args);
    }
    if name.eq_ignore_ascii_case("XLOOKUP") {
        return validate_xlookup_shape(name, args);
    }
    Ok(())
}

/// Build the typed shape refusal for `name` with the given literal `repair`.
fn shape_error(name: &str, repair: &str) -> ParseError {
    ParseError::UnsupportedCallShape {
        name: name.to_string(),
        repair: repair.to_string(),
    }
}

/// Is this argument the dialect's literal EXACT-match flag (`FALSE` or `0`)?
fn is_exact_match_literal(arg: &Expr) -> bool {
    match arg {
        Expr::Bool(b) => !*b,
        Expr::Number(n) => *n == 0.0,
        _ => false,
    }
}

/// `VLOOKUP(key, table, col, range_lookup)` — the 4th argument is REQUIRED and
/// must be literally `FALSE` or `0` (D-Q1). Excel's default is APPROXIMATE, so
/// an omitted argument is refused as firmly as an explicit `TRUE`.
fn validate_vlookup_shape(name: &str, args: &[Expr]) -> Result<(), ParseError> {
    if args.len() == 4 && is_exact_match_literal(&args[3]) {
        return Ok(());
    }
    Err(shape_error(
        name,
        "write VLOOKUP(key, table, col, FALSE) — the dialect supports EXACT match only",
    ))
}

/// `MATCH(key, range, match_type)` — the 3rd argument is REQUIRED and must be
/// literally `0` (D-Q1). Excel's default `match_type` is `1` (APPROXIMATE).
fn validate_match_shape(name: &str, args: &[Expr]) -> Result<(), ParseError> {
    if args.len() == 3 && matches!(&args[2], Expr::Number(n) if *n == 0.0) {
        return Ok(());
    }
    Err(shape_error(
        name,
        "write MATCH(key, range, 0) — the dialect supports EXACT match only; \
         Excel's default is approximate",
    ))
}

/// The one accepted `XLOOKUP` signature, quoted verbatim in every refusal.
const XLOOKUP_SIGNATURE: &str = "XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found])";

/// `XLOOKUP` in its constrained 3/4-argument exact form (D-Q4).
///
/// Arity is always checkable. The array shape checks fire ONLY when both array
/// arguments are literal [`Expr::Range`]s AND both resolve — a shape this gate
/// cannot MEASURE is left to the evaluator's typed backstop rather than guessed
/// at.
fn validate_xlookup_shape(name: &str, args: &[Expr]) -> Result<(), ParseError> {
    if args.len() < 3 || args.len() > 4 {
        return Err(shape_error(
            name,
            &format!(
                "write {XLOOKUP_SIGNATURE} — match_mode and search_mode are not supported; \
                 the dialect is exact-match only"
            ),
        ));
    }
    let (Expr::Range(lookup), Expr::Range(ret)) = (&args[1], &args[2]) else {
        return Ok(());
    };
    let (Some((lookup_rows, lookup_cols)), Some((ret_rows, ret_cols))) =
        (range_extent(lookup), range_extent(ret))
    else {
        return Ok(());
    };
    if ret_cols > 1 {
        return Err(shape_error(
            name,
            &format!(
                "return_array must be a SINGLE column — write {XLOOKUP_SIGNATURE} with a \
                 one-column return_array"
            ),
        ));
    }
    if lookup_rows * u32::from(lookup_cols) != ret_rows * u32::from(ret_cols) {
        return Err(shape_error(
            name,
            &format!(
                "lookup_array and return_array must cover the SAME number of cells — \
                 write {XLOOKUP_SIGNATURE} with conformable arrays"
            ),
        ));
    }
    Ok(())
}

/// The `(rows, cols)` extent of a literal A1 range, or `None` when either
/// endpoint is not a resolvable A1 address (a shape the gate must not guess at).
fn range_extent(range: &RangeRef) -> Option<(u32, u16)> {
    let (r0, c0) = a1_to_zero_indexed_row_col(&range.start)?;
    let (r1, c1) = a1_to_zero_indexed_row_col(&range.end)?;
    Some((r1.abs_diff(r0) + 1, c1.abs_diff(c0) + 1))
}

/// Map a `#…` error lexeme to the shared [`ExcelError`]. An unrecognized error
/// name falls back to `#NAME?` semantics (the closest "unknown name" error).
fn error_of(raw: &str) -> ExcelError {
    match raw.to_ascii_uppercase().as_str() {
        "#REF!" => ExcelError::Ref,
        "#VALUE!" => ExcelError::Value,
        "#DIV/0!" => ExcelError::DivZero,
        "#N/A" => ExcelError::Na,
        "#NUM!" => ExcelError::Num,
        "#NULL!" => ExcelError::Null,
        _ => ExcelError::Name, // "#NAME?" and any unrecognized error tag
    }
}

/// Split a possibly sheet-qualified A1 ref text into `(sheet, addr)`. A quoted
/// sheet (`'Quoted Sheet'!A1`) has its surrounding quotes stripped and `''`
/// un-doubled so the sheet is a plain string recorded ONCE on the `RangeRef`.
fn split_sheet(text: &str) -> (Option<String>, String) {
    if text.starts_with('\'') {
        return split_quoted_sheet(text);
    }
    match text.find('!') {
        Some(idx) => (Some(text[..idx].to_string()), text[idx + 1..].to_string()),
        None => (None, text.to_string()),
    }
}

/// Split a `'Quoted''Sheet'!A1` ref: find the closing (un-doubled) quote, then
/// the `!`, returning the un-quoted sheet + the address.
fn split_quoted_sheet(text: &str) -> (Option<String>, String) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 1usize;
    let mut sheet = String::new();
    let mut closed = false;
    while i < chars.len() {
        if chars[i] == '\'' {
            if i + 1 < chars.len() && chars[i + 1] == '\'' {
                sheet.push('\'');
                i += 2;
            } else {
                i += 1;
                closed = true;
                break;
            }
        } else {
            sheet.push(chars[i]);
            i += 1;
        }
    }
    if closed && i < chars.len() && chars[i] == '!' {
        let addr: String = chars[i + 1..].iter().collect();
        return (Some(sheet), addr);
    }
    // Malformed quoted ref — treat the whole thing as the address.
    (None, text.to_string())
}

/// Map an infix-operator token to its [`BinOp`] (range `:` is folded in the
/// primary parser, so it is NOT an infix BinOp here).
fn binop_of(t: &Token) -> Option<BinOp> {
    Some(match t {
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
        Token::Star => BinOp::Mul,
        Token::Slash => BinOp::Div,
        Token::Caret => BinOp::Pow,
        Token::Amp => BinOp::Concat,
        Token::Eq => BinOp::Eq,
        Token::Ne => BinOp::Ne,
        Token::Lt => BinOp::Lt,
        Token::Gt => BinOp::Gt,
        Token::Le => BinOp::Le,
        Token::Ge => BinOp::Ge,
        _ => return None,
    })
}

/// Left/right binding powers per the Microsoft precedence table. A LARGER number
/// binds tighter. `^` is RIGHT-associative (right bp < left bp); every other
/// binary operator is left-associative. Unary `-`/`+` and postfix `%` bind
/// TIGHTER than `^` and are handled in `parse_prefix`, so they need no entry.
fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        // comparisons — lowest, left-assoc
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => (1, 2),
        // `&` concat — above comparisons, left-assoc
        BinOp::Concat => (3, 4),
        // `+` / `-` — left-assoc
        BinOp::Add | BinOp::Sub => (5, 6),
        // `*` / `/` — left-assoc
        BinOp::Mul | BinOp::Div => (7, 8),
        // `^` — RIGHT-assoc (right bp < left bp so 2^3^2 == 2^(3^2))
        BinOp::Pow => (10, 9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(f: &str) -> Result<Expr, ParseError> {
        parse(f, "7_Quote", "C14")
    }

    // ---- Behavior tests (the six the plan mandates) ----

    /// whitelist_at_parse_rejects: a function NOT in the WHITELIST is rejected
    /// AT PARSE TIME with a typed error — NOT a lint finding, NOT a built node.
    #[test]
    fn whitelist_at_parse_rejects() {
        let err = p("OFFSET(A1,1,1)").expect_err("OFFSET must be rejected at parse time");
        assert_eq!(err, ParseError::UnsupportedFunction("OFFSET".to_string()));
    }

    /// whitelisted_function_parses: a whitelisted function (ROUND) parses into
    /// runtime `Expr`.
    #[test]
    fn whitelisted_function_parses() {
        let expr = p("ROUND(C11,2)").expect("ROUND is whitelisted");
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "ROUND");
                assert_eq!(args.len(), 2);
            },
            other => panic!("expected a ROUND Call, got {other:?}"),
        }
    }

    /// depth_limit_rejects: nesting beyond MAX_PARSE_DEPTH is a typed
    /// depth-exceeded error (enforced in code, not discovered by the fuzzer).
    #[test]
    fn depth_limit_rejects() {
        let depth = MAX_PARSE_DEPTH + 50;
        let f = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let err = p(&f).expect_err("an over-deep formula must be rejected");
        assert_eq!(err, ParseError::TooDeep);
    }

    /// supported_construct_matrix: table-driven over each documented matrix row.
    #[test]
    fn supported_construct_matrix() {
        // (formula, expect_ok)
        let supported = [
            "S!A1",          // sheet-qualified ref
            "'My Sheet'!A1", // quoted sheet name
            "SUM(A1:B2)",    // A1 range
            "SUM(S!A1:B2)",  // sheet-qualified range
            "$A$1",          // absolute ref
            "-A1",           // unary minus
            "A1%",           // postfix percent
            "\"hello\"",     // string literal
            "1.5E3",         // scientific number
            "TRUE",          // boolean
            "#REF!",         // error value
            "ROUND(A1,2)",   // whitelisted call
        ];
        for f in supported {
            assert!(
                p(f).is_ok(),
                "matrix row `{f}` must be SUPPORTED: {:?}",
                p(f)
            );
        }

        // Rejections, each with its documented typed error.
        assert_eq!(
            p("OFFSET(A1)").unwrap_err(),
            ParseError::UnsupportedFunction("OFFSET".to_string())
        );
        assert_eq!(
            p("[Book.xlsx]Sheet1!A1").unwrap_err(),
            ParseError::ExternalRef("[Book.xlsx]Sheet1!A1".to_string())
        );
        assert_eq!(p("{1,2,3}").unwrap_err(), ParseError::ArrayFormula);
    }

    // ---- Structural / precedence tests ----

    #[test]
    fn unknown_bare_name_is_a_name_not_an_unsupported_function() {
        // A bare name is DISTINCT from an out-of-whitelist function — it is
        // resolved at the DAG layer, never a parse error.
        let expr = p("FooBar").expect("a bare name parses");
        assert!(matches!(expr, Expr::Name(ref n) if n == "FooBar"));
    }

    #[test]
    fn sheet_qualified_range_folds_to_single_range_sheet_once() {
        let expr = p("SUM(Sheet1!A1:B2)").expect("parse");
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "SUM");
                match &args[0] {
                    Expr::Range(r) => {
                        assert_eq!(r.sheet, "Sheet1", "sheet recorded once");
                        assert_eq!(r.start, "A1");
                        assert_eq!(r.end, "B2");
                    },
                    other => panic!("expected Range arg, got {other:?}"),
                }
            },
            other => panic!("expected SUM call, got {other:?}"),
        }
    }

    #[test]
    fn quoted_sheet_anchored_range_folds_with_sheet_once() {
        let expr = p("SUM('Quoted Sheet'!$A$1:$B$2)").expect("parse");
        match expr {
            Expr::Call { args, .. } => match &args[0] {
                Expr::Range(r) => {
                    assert_eq!(r.sheet, "Quoted Sheet", "quotes stripped, sheet once");
                    assert_eq!(r.start, "$A$1");
                    assert_eq!(r.end, "$B$2");
                },
                other => panic!("expected Range, got {other:?}"),
            },
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn nested_whitelisted_formula_parses() {
        let f = "IF(ROUND(C11,2)=1594.93,\"OK\",\"FAIL: \"&TEXT(C11,\"#,##0.00\"))";
        let expr = p(f).expect("a fully-whitelisted nested formula parses");
        match expr {
            Expr::Call { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            },
            other => panic!("expected top-level IF, got {other:?}"),
        }
    }

    #[test]
    fn concat_binds_tighter_than_comparison() {
        let expr = p("A1&B1=\"xy\"").expect("parse");
        match expr {
            Expr::BinaryOp { left, op, right } => {
                assert_eq!(op, BinOp::Eq);
                assert!(matches!(
                    *left,
                    Expr::BinaryOp {
                        op: BinOp::Concat,
                        ..
                    }
                ));
                assert!(matches!(*right, Expr::Str(_)));
            },
            other => panic!("expected top-level Eq, got {other:?}"),
        }
    }

    #[test]
    fn unary_binds_tighter_than_pow() {
        // `-10^2` parses as `(-10)^2`.
        let expr = p("-10^2").expect("parse");
        match expr {
            Expr::BinaryOp { left, op, right } => {
                assert_eq!(op, BinOp::Pow);
                assert!(matches!(*left, Expr::UnaryOp { op: UnOp::Neg, .. }));
                assert!(matches!(*right, Expr::Number(n) if n == 2.0));
            },
            other => panic!("expected (-10)^2, got {other:?}"),
        }
    }

    #[test]
    fn pow_is_right_associative() {
        // `2^3^2` parses as `2^(3^2)`.
        let expr = p("2^3^2").expect("parse");
        match expr {
            Expr::BinaryOp { left, op, right } => {
                assert_eq!(op, BinOp::Pow);
                assert!(matches!(*left, Expr::Number(n) if n == 2.0));
                assert!(matches!(*right, Expr::BinaryOp { op: BinOp::Pow, .. }));
            },
            other => panic!("expected 2^(3^2), got {other:?}"),
        }
    }

    #[test]
    fn error_literal_parses_to_excel_error() {
        assert!(matches!(p("#REF!"), Ok(Expr::ErrorLit(ExcelError::Ref))));
    }

    #[test]
    fn bool_keywords_parse_to_bool() {
        assert!(matches!(p("TRUE"), Ok(Expr::Bool(true))));
        assert!(matches!(p("FALSE"), Ok(Expr::Bool(false))));
    }

    #[test]
    fn arithmetic_precedence_mul_above_add() {
        let expr = p("A1+B1*C1").expect("parse");
        match expr {
            Expr::BinaryOp { op, right, .. } => {
                assert_eq!(op, BinOp::Add);
                assert!(matches!(*right, Expr::BinaryOp { op: BinOp::Mul, .. }));
            },
            other => panic!("expected Add at top, got {other:?}"),
        }
    }

    #[test]
    fn trailing_tokens_are_malformed() {
        assert!(matches!(p("A1 B1"), Err(ParseError::Malformed(_))));
    }

    #[test]
    fn unbalanced_paren_is_malformed() {
        assert!(matches!(p("(A1+B1"), Err(ParseError::Malformed(_))));
    }

    // ---- Call-shape gate (dialect 1.1, D-Q1 / D-Q4) ----
    //
    // The dialect supports EXACT lookup only. Excel's DEFAULT for VLOOKUP's
    // `range_lookup` and MATCH's `match_type` is APPROXIMATE, so an OMITTED
    // argument is refused exactly as firmly as an explicit approximate one —
    // assuming exact is the silently-wrong answer this gate exists to prevent.

    /// The exact shapes the repo's own fixtures already author must keep parsing.
    #[test]
    fn authored_fixture_shapes_still_parse() {
        for formula in [
            "IFERROR(VLOOKUP(B2,D2:E4,2,FALSE),0.08)",
            "IFERROR(INDEX(E2:E4,MATCH(B2,D2:D4,0)),0.08)",
            "VLOOKUP(B2,D2:E4,2,0)",
        ] {
            assert!(
                p(formula).is_ok(),
                "authored fixture shape must still parse: {formula}"
            );
        }
    }

    #[test]
    fn the_four_new_functions_parse_clean() {
        for formula in [
            "ROUNDDOWN(A1,2)",
            "MAX(A1:A9)",
            "MIN(A1:A9,B1)",
            "XLOOKUP(B2,D2:D4,E2:E4)",
            "XLOOKUP(B2,D2:D4,E2:E4,0.08)",
        ] {
            assert!(
                p(formula).is_ok(),
                "dialect 1.1 shape must parse: {formula}"
            );
        }
    }

    #[test]
    fn vlookup_without_the_exact_match_argument_is_a_shape_error() {
        let err = p("VLOOKUP(B2,D2:E4,2)").expect_err("a bare VLOOKUP must be refused");
        assert!(
            matches!(err, ParseError::UnsupportedCallShape { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn vlookup_with_an_approximate_match_argument_is_a_shape_error() {
        for formula in ["VLOOKUP(B2,D2:E4,2,TRUE)", "VLOOKUP(B2,D2:E4,2,1)"] {
            let err = p(formula).expect_err("an approximate VLOOKUP must be refused");
            assert!(
                matches!(err, ParseError::UnsupportedCallShape { .. }),
                "{formula} got {err:?}"
            );
        }
    }

    #[test]
    fn match_without_the_match_type_argument_is_a_shape_error() {
        // Excel's DEFAULT match_type is 1 (APPROXIMATE) — this is the form that
        // silently returned a wrong number before dialect 1.1.
        let err = p("MATCH(B2,D2:D4)").expect_err("a bare MATCH must be refused");
        assert!(
            matches!(err, ParseError::UnsupportedCallShape { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn match_with_an_approximate_match_type_is_a_shape_error() {
        for formula in ["MATCH(B2,D2:D4,1)", "MATCH(B2,D2:D4,-1)"] {
            let err = p(formula).expect_err("an approximate MATCH must be refused");
            assert!(
                matches!(err, ParseError::UnsupportedCallShape { .. }),
                "{formula} got {err:?}"
            );
        }
    }

    #[test]
    fn xlookup_with_a_mode_argument_is_a_shape_error() {
        for formula in [
            "XLOOKUP(B2,D2:D4,E2:E4,0,0)",
            "XLOOKUP(B2,D2:D4,E2:E4,0,0,1)",
        ] {
            let err = p(formula).expect_err("match_mode/search_mode must be refused");
            assert!(
                matches!(err, ParseError::UnsupportedCallShape { .. }),
                "{formula} got {err:?}"
            );
        }
    }

    #[test]
    fn xlookup_statically_knowable_array_violations_are_shape_errors() {
        // Both array arguments are literal ranges, so the widths and member
        // counts are knowable WITHOUT evaluating anything.
        let wide =
            p("XLOOKUP(B2,D2:D4,E2:F4)").expect_err("a multi-column return_array must be refused");
        assert!(
            matches!(wide, ParseError::UnsupportedCallShape { .. }),
            "got {wide:?}"
        );

        let ragged =
            p("XLOOKUP(B2,D2:D4,E2:E9)").expect_err("non-conformable arrays must be refused");
        assert!(
            matches!(ragged, ParseError::UnsupportedCallShape { .. }),
            "got {ragged:?}"
        );
    }

    /// A shape the gate cannot MEASURE statically is not guessed at — it is left
    /// to the evaluator's typed backstop. Never reject a form you could not
    /// measure.
    #[test]
    fn xlookup_with_non_literal_arrays_is_not_statically_refused() {
        assert!(
            p("XLOOKUP(B2,named_keys,named_values)").is_ok(),
            "a non-Range array argument must pass the static gate untouched"
        );
    }

    /// MESSAGE QUALITY: each refusal's rendered Display must name the offending
    /// function AND carry the literal repair a BA can act on. These strings are
    /// the BA-facing contract, so they are asserted, not merely produced.
    #[test]
    fn shape_errors_render_a_named_actionable_repair() {
        let vlookup = p("VLOOKUP(B2,D2:E4,2)").expect_err("refused").to_string();
        assert!(vlookup.contains("VLOOKUP"), "{vlookup}");
        assert!(vlookup.contains("FALSE"), "{vlookup}");

        let match_err = p("MATCH(B2,D2:D4)").expect_err("refused").to_string();
        assert!(match_err.contains("MATCH"), "{match_err}");
        assert!(
            match_err.contains("MATCH(key, range, 0)"),
            "the repair must be literal and copy-pasteable: {match_err}"
        );

        let xlookup = p("XLOOKUP(B2,D2:D4,E2:E4,0,0)")
            .expect_err("refused")
            .to_string();
        assert!(xlookup.contains("XLOOKUP"), "{xlookup}");
        assert!(
            xlookup.contains("XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found])"),
            "the accepted signature must be named: {xlookup}"
        );
    }

    /// REGRESSION: the whitelist gate must still run FIRST, so an
    /// out-of-whitelist name is an UnsupportedFunction and never a shape error.
    #[test]
    fn the_whitelist_gate_is_not_shadowed_by_the_shape_gate() {
        let err = p("OFFSET(A1,1,1)").expect_err("OFFSET is out of whitelist");
        assert_eq!(err, ParseError::UnsupportedFunction("OFFSET".to_string()));
    }
}
