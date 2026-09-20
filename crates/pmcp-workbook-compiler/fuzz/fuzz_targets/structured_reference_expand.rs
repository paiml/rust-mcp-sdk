//! Fuzz target over Excel `Table[Column]` expansion (ALWAYS: FUZZ).
//!
//! Workbook formulas are untrusted text. The structured-reference scanner must
//! return an expansion or a typed error for every UTF-8 input without panicking,
//! hanging, or indexing across a Unicode boundary.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_workbook_compiler::formula::fuzz_expand_structured_references;

fuzz_target!(|data: &[u8]| {
    if let Ok(formula) = std::str::from_utf8(data) {
        fuzz_expand_structured_references(formula);
    }
});
