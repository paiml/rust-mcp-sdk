//! Fuzz target for `pmcp::client::oauth::DcrResponse` JSON parser.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run dcr_response_parser`.
//!
//! Invariant: `serde_json::from_slice::<DcrResponse>` must never panic on
//! arbitrary bytes. Error paths are acceptable; panics are not. Also validates
//! that a hostile registration_endpoint returning malformed JSON can't crash
//! the SDK's DCR parser (threat T-74-C).
//!
//! Phase 116-08 (AUTH-02) EXTENSION: `application_type()` projects a value out
//! of the flattened `extra` map, so it reads the same attacker-influenced bytes
//! this target already generates — extended here rather than duplicated into a
//! new target so the accessor inherits this one's DCR-shaped corpus. Whenever it
//! returns `Some`, the value must have come VERBATIM from a JSON string in the
//! input; the check is skipped for inputs containing a backslash, because a JSON
//! escape decodes to characters that never appear consecutively in the source
//! bytes (threat T-116-27c).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must return Result, never panic.
    let Ok(response) = serde_json::from_slice::<pmcp::client::oauth::DcrResponse>(data) else {
        return;
    };

    // Must return Option, never panic and never stringify.
    let Some(application_type) = response.application_type() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if text.contains('\\') {
        return;
    }
    assert!(
        text.contains(application_type),
        "DcrResponse::application_type() returned `{application_type}`, which does not appear \
         anywhere in the input bytes"
    );
});
