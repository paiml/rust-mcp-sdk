//! The ONE place the `${VAR}` / `env:VAR` reference grammar is defined for the
//! whole toolkit.
//!
//! Every path that reads an operator-supplied value which MAY be an environment
//! reference routes through [`parse_env_ref`]:
//!
//! - outgoing credentials (`[backend.auth]` api_key / bearer token / basic
//!   password / oauth2 client_secret) — `crate::http::auth`;
//! - `[code_mode] token_secret` — `crate::code_mode`;
//! - `[backend] base_url` — [`crate::config::BackendSection::resolved_base_url`].
//!
//! # Why this module is NOT feature-gated
//!
//! The function previously lived as a private helper inside `crate::http::auth`,
//! and the whole `http` module is `#[cfg(feature = "http")]`. It compiled
//! (`BackendSection` is itself http-gated), but the accompanying claim — "the
//! single chokepoint every env-reference path shares" — was reachable only in
//! `http` builds, so it was architecturally false. Packaging tooling is about to
//! build a cross-crate grammar-parity claim on top of that statement, so the
//! statement has to be structurally true rather than aspirational: this module
//! carries NO `#[cfg(feature = ...)]` gate and compiles in every feature
//! configuration.
//!
//! # Grammar
//!
//! | Input | Result | Meaning |
//! |---|---|---|
//! | `env:VAR` | `Some("VAR")` | reference |
//! | `${VAR}` | `Some("VAR")` | reference (`VAR` must match `[A-Za-z0-9_]+`) |
//! | `${}` | `Some("")` | MALFORMED reference — a reference to an empty name |
//! | `${A}://${B}` | `Some("")` | MALFORMED reference — a multi-placeholder composition |
//! | `${VAR` | `None` | unterminated → a plain literal |
//! | `plain` | `None` | plain literal |
//!
//! The malformed-means-empty rule is deliberate: the empty name is the signal
//! that a value is reference-SHAPED but names nothing, so no caller ships the
//! literal `${...}` text to the wire. A `${...}` value is a reference to exactly
//! ONE variable — the grammar does not interpolate inside a larger string, so a
//! composition like `${SCHEME}://${HOST}` names nothing any environment could
//! set; compose the full value in one variable instead.
//!
//! # Callers diverge on UNSET, agree on MALFORMED, never diverge on PARSING
//!
//! Two different questions, two different answers:
//!
//! - **Unset variable** (`Some("VAR")`, `VAR` not exported) — caller policy, and
//!   it legitimately differs. A credential resolves it to the empty string so an
//!   OPTIONAL credential is omitted; an endpoint errors, because an empty
//!   endpoint is not a degraded request but a broken one.
//! - **Malformed reference** (`Some("")`) — NOT caller policy. Every caller
//!   refuses it, because no environment could ever satisfy it, so "omit" would
//!   mean silently proceeding without a value the operator believed they had
//!   supplied. The credential path once applied the unset rule here, which made
//!   `token = "${GITHUB-PAT}"` send every backend request unauthenticated with
//!   no error and no log line; it is now refused at load time
//!   ([`crate::error::ConfigValidationError::MalformedBackendAuthRef`]) and at
//!   provider-build time, matching `base_url`'s
//!   [`crate::error::ConfigValidationError::MalformedBackendBaseUrlRef`].
//!
//! What must NOT differ is the PARSE: a second `${}` parser with slightly
//! different edge cases is a latent security bug (one parser treating `${VAR` as
//! a reference and another as a literal is exactly how a placeholder reaches the
//! wire).
//!
//! Note that `pmcp-package` deliberately DUPLICATES this grammar rather than
//! depending on the toolkit — that crate is the workspace-excluded leaf and a
//! dependency in either direction inverts the layering. The duplication is kept
//! honest by a package-side grammar-parity table, not by a shared type.

/// The single brace/env-ref parse core shared by EVERY credential-resolution
/// path (api_key, bearer token, basic password, oauth2 client_secret).
///
/// Returns `Some(var_name)` when `raw` is a secret REFERENCE — either the
/// `"env:VAR"` or the `"${VAR}"` form — and `None` for a plain literal (which the
/// caller uses verbatim). A malformed brace reference (e.g. `"${}"`) is treated
/// as a reference to an empty name, i.e. `Some("")`, so the caller resolves it to
/// the empty string (omission) rather than shipping the literal `${}`.
///
/// This consolidates the two brace parsers that previously existed (the inline
/// `${`-strip in the old api_key resolver in `crate::http::auth` and
/// `expand_braced_var` in `crate::code_mode`): all env-reference resolution now
/// flows through this one chokepoint so the discipline cannot drift per-variant.
///
/// # Why this is `pub`
///
/// The grammar is duplicated by necessity in `pmcp-package`'s
/// `is_env_reference` — that crate is the workspace-excluded leaf and neither
/// crate may depend on the other. The two implementations are held to a shared
/// accept/reject table asserted from an INTEGRATION test in each crate
/// (`tests/env_ref_grammar_parity.rs` here). An integration test is an external
/// consumer, so the reference implementation has to be reachable from outside
/// the crate for that parity claim to be checkable at all.
///
/// (It was previously `pub(crate)` with an `#[allow(dead_code)]`, because in a
/// `--no-default-features` build neither `http` nor `code-mode` is compiled and
/// no in-crate caller exists. Being `pub` removes that need: the module is
/// still deliberately UNGATED, so the grammar is defined in one place that
/// compiles in every feature configuration.)
pub fn parse_env_ref(raw: &str) -> Option<&str> {
    if let Some(v) = raw.strip_prefix("env:") {
        Some(v)
    } else {
        // `${...}` → the inner name when it is a valid variable name. Any
        // other interior — the empty `${}` form, or a multi-placeholder
        // composition like `${A}://${B}` (whose interior would be the
        // unsettable `A}://${B`) — is MALFORMED: a reference to the empty
        // name. Callers resolve that to omission (credentials) or an error
        // (endpoints), so a placeholder never reaches the wire as a literal.
        raw.strip_prefix("${")
            .and_then(|s| s.strip_suffix('}'))
            .map(|name| {
                if is_valid_env_var_name(name) {
                    name
                } else {
                    ""
                }
            })
    }
}

/// Whether `name` is a variable name a target environment can actually be
/// told to set: non-empty, ASCII alphanumerics and `_` only — the portable
/// intersection of POSIX shells, env files, and container manifests.
///
/// The `${NAME}` form requires this (it is the form config authors compose
/// by accident — `${SCHEME}://${HOST}` must not silently become one garbage
/// reference). The explicit `env:NAME` form deliberately does NOT: its prefix
/// is unambiguous, so it stays the runtime escape hatch for exotic names.
pub fn is_valid_env_var_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::parse_env_ref;
    use proptest::prelude::*;

    #[test]
    fn test_parse_env_ref_distinguishes_literal_from_reference() {
        assert_eq!(parse_env_ref("env:FOO"), Some("FOO"));
        assert_eq!(parse_env_ref("${FOO}"), Some("FOO"));
        assert_eq!(parse_env_ref("${}"), Some("")); // malformed-but-a-reference
        assert_eq!(parse_env_ref("plain"), None);
        assert_eq!(parse_env_ref("${FOO"), None); // unterminated → literal
    }

    #[test]
    fn test_multi_placeholder_compositions_are_malformed_references() {
        // A `${...}` value references exactly ONE variable; a composition is
        // reference-SHAPED (so it must never ship as a literal) but names
        // nothing settable → the empty name, same as `${}`.
        assert_eq!(parse_env_ref("${TFL_SCHEME}://${TFL_HOST}"), Some(""));
        assert_eq!(parse_env_ref("${A}-${B}"), Some(""));
        // A dash is not portably settable; `env:` remains the escape hatch.
        assert_eq!(parse_env_ref("${TFL-HOST}"), Some(""));
        assert_eq!(parse_env_ref("env:TFL-HOST"), Some("TFL-HOST"));
    }

    proptest! {
        /// PROPERTY/FUZZ (CLAUDE.md ALWAYS): the grammar chokepoint is total —
        /// every input yields `Some`/`None`, never a panic. This is the toolkit
        /// half of the guarantee `pmcp-package` pins for its duplicate
        /// (`config_validation.rs`'s never-panic properties); a parser that can
        /// unwind on adversarial config text would take config loading down
        /// with it.
        #[test]
        fn parse_env_ref_never_panics_on_arbitrary_text(raw in "\\PC{0,256}") {
            let _ = parse_env_ref(&raw);
        }

        /// PROPERTY: both reference forms round-trip any brace-free,
        /// colon-agnostic name — `${NAME}` and `env:NAME` must parse back to
        /// exactly `NAME`, so the two forms can never diverge on which
        /// variable they address.
        #[test]
        fn both_reference_forms_recover_the_exact_variable_name(
            name in "[A-Za-z0-9_]{1,64}"
        ) {
            let braced = format!("${{{name}}}");
            prop_assert_eq!(parse_env_ref(&braced), Some(name.as_str()));
            let prefixed = format!("env:{name}");
            prop_assert_eq!(parse_env_ref(&prefixed), Some(name.as_str()));
        }

        /// PROPERTY: a value that starts with neither `env:` nor `${` is
        /// ALWAYS a plain literal (`None`) — the rule that keeps an Athena
        /// `output_location` containing `${` mid-string, or any URL, from
        /// being misread as a reference.
        #[test]
        fn values_without_a_reference_prefix_are_always_literals(
            raw in "\\PC{0,256}"
        ) {
            prop_assume!(!raw.starts_with("env:") && !raw.starts_with("${"));
            prop_assert_eq!(parse_env_ref(&raw), None);
        }

        /// PROPERTY: a braced form NEVER yields an unsettable name — whatever
        /// the interior, the parse is either a valid variable name or the
        /// empty (malformed) name. This is the invariant that keeps a
        /// multi-placeholder composition from being looked up in the
        /// environment as one garbage variable.
        #[test]
        fn brace_forms_never_yield_an_unsettable_name(
            interior in "\\PC{0,64}"
        ) {
            let raw = format!("${{{interior}}}");
            match parse_env_ref(&raw) {
                Some(name) => prop_assert!(
                    name.is_empty() || super::is_valid_env_var_name(name)
                ),
                None => prop_assert!(false, "a `${{...}}` wrap must parse as a reference"),
            }
        }
    }
}
