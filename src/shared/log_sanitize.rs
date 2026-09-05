//! Neutralising author- and caller-supplied text before it reaches a log sink or
//! an error message.
//!
//! # Why this is one module and not a helper next to each consumer
//!
//! The SDK has more than one place where untrusted text is rendered into a
//! line-oriented sink — a `tracing` field, a JSON-RPC error message — and each
//! of them has to answer the same question: which characters can forge a record?
//! That question was answered twice, independently, in two modules that could
//! not see each other, and the two answers had already drifted: one tested
//! `char::is_control()` alone, the other tested `is_control()` plus the two
//! Unicode line separators. The classification lives here so a third consumer
//! inherits the answer instead of re-deriving it.
//!
//! Substitution is lossy and unconditional (`U+FFFD`). That is the right
//! disposition for DISPLAY text, which has no escape vocabulary to preserve the
//! original character with. It is deliberately NOT the right disposition for a
//! re-parsable scalar — the skills projection's YAML encoder ESCAPES the same
//! characters rather than replacing them, and shares only the classification
//! below (WR-06).

// Items here are `pub(crate)` so the enclosing `pub(crate) mod log_sanitize`
// keeps them crate-internal AND `unreachable_pub`-clean. `redundant_pub_crate`
// would prefer bare `pub`, but bare `pub` then trips `unreachable_pub` (the
// module is not publicly reachable) — the two lints are mutually exclusive
// here, so we keep `pub(crate)` and silence the former, the same way
// `shared::pending_slot` does.
#![allow(clippy::redundant_pub_crate)]

/// `true` for the two Unicode line separators that are NOT Unicode `Cc`.
///
/// `char::is_control()` is exactly the `Cc` category (`U+0000..=U+001F`,
/// `U+007F..=U+009F`), so a bare `is_control()` test does NOT reach LINE
/// SEPARATOR (`U+2028`, category `Zl`) or PARAGRAPH SEPARATOR (`U+2029`,
/// category `Zp`). Both are nevertheless line terminators to the consumers this
/// crate hands text to — YAML 1.1's scanner, line-oriented log processors and
/// JS-based log viewers alike — so every site that reasons about "characters
/// that can end a line" must test this IN ADDITION to `is_control()`.
///
/// Shared so that the skills projection's YAML encoder, [`sanitize_for_log`] and
/// the `skills/get` URI echo cannot drift apart on the classification again;
/// they differ only in what they substitute (CR-01, WR-06).
pub(crate) const fn is_unicode_line_separator(c: char) -> bool {
    matches!(c, '\u{2028}' | '\u{2029}')
}

/// Replace every character that could terminate a log record with `U+FFFD`.
///
/// The values this protects are author- or caller-supplied and reach a log sink
/// or an error message; an embedded newline or terminal escape could forge a
/// second record. A `uri` of `skill://x\n2026-01-01 ERROR audit: auth bypassed`
/// is the worked example — well under any length bound, and its own line in any
/// line-oriented log that renders it verbatim.
///
/// Both the `Cc` set and the two [`is_unicode_line_separator`] characters are
/// replaced. Callers that need a length bound as well should bound the input
/// FIRST and sanitize the bounded head, so the scan stays proportional to the
/// budget rather than to hostile input.
pub(crate) fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || is_unicode_line_separator(c) {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The home test for the classification both consumers now share.
    ///
    /// The `Cc` half and the `Zl`/`Zp` half are asserted together because the
    /// defect this module exists to prevent was precisely a consumer that had
    /// one half and not the other. The `is_control()` premise is asserted so
    /// the second half cannot pass for a stale reason if that predicate ever
    /// widens.
    #[test]
    fn sanitize_for_log_replaces_every_record_forging_character() {
        assert_eq!(sanitize_for_log("a\nb\u{0}c"), "a\u{fffd}b\u{fffd}c");
        assert_eq!(sanitize_for_log("a\u{1b}[31m"), "a\u{fffd}[31m");

        assert!(!'\u{2028}'.is_control());
        assert!(!'\u{2029}'.is_control());
        assert_eq!(
            sanitize_for_log("a\u{2028}b\u{2029}c"),
            "a\u{fffd}b\u{fffd}c"
        );

        // Anti-vacuity: ordinary text survives byte for byte.
        assert_eq!(sanitize_for_log("plain"), "plain");
    }

    #[test]
    fn the_line_separator_predicate_is_exactly_two_characters() {
        assert!(is_unicode_line_separator('\u{2028}'));
        assert!(is_unicode_line_separator('\u{2029}'));
        assert!(!is_unicode_line_separator('\u{2027}'));
        assert!(!is_unicode_line_separator('\u{202a}'));
        assert!(!is_unicode_line_separator('a'));
        assert!(!is_unicode_line_separator('\n'));
    }
}
