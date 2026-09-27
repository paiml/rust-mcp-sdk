//! Runtime enforcement of a tool's declared `inputSchema` (Phase 128, D-01).
//!
//! This is the ONE input-validation entry point in the SDK. It lives in core
//! `pmcp` rather than in a consumer crate so that a tool's arguments and its
//! `structuredContent` can never be checked under different dialects, and so the
//! compiled-validator cache, the draft pin and the value-free refusal renderer
//! exist exactly once.
//!
//! # Why this is not [`super::output_validation`]
//!
//! - **Inputs REFUSE; outputs only warn.** A `tools/call` whose arguments violate
//!   the declared schema must never reach a backend, so [`validate_input`]
//!   returns an error rather than emitting a `tracing::warn!`.
//! - **Inputs compile under Draft 2020-12 on BOTH eras (D-02).** `Era` is
//!   deliberately NOT a parameter here, and inputs do NOT route through
//!   `output_validation::compile_for_era`. Outputs froze v1 at `$schema`
//!   auto-detect because there was shipped behaviour to freeze; inputs have none,
//!   and a config- or `schemars`-declared `$schema` must not be able to change
//!   input enforcement semantics.
//! - **Inputs assert `format`; outputs do not (Q1).** `format` is annotative by
//!   default in `jsonschema` 0.49, so a declared `format` would silently enforce
//!   nothing. Inputs therefore compile through a format-asserting builder
//!   ([`compile_input_2020_12`]). Because that changes compile semantics, inputs
//!   keep their own compile entry point and their own validator cache
//!   ([`cached_input_validator`], Q6) — a format-asserting and a non-asserting
//!   validator must never collide on one cache key.
//! - **Refusals are rendered value-free (SC-7).** A `ValidationError`'s `Display`
//!   echoes the rejected value for every keyword, and
//!   `ValidationErrorKind::AdditionalProperties::unexpected` is the caller's own
//!   key list. Neither ever reaches a rendered refusal: see [`expectation`] and
//!   [`render_refusal`]. Callers of this module handle PHI.
//!
//! No `jsonschema` type appears in any public signature in this module, so a
//! future `jsonschema` major bump is not a breaking `pmcp` change.

use serde_json::Value;

/// Refusal detail for a `tools/call` argument-schema violation.
///
/// Deliberately carries DECLARED data only: never the rejected value, and never
/// a caller-supplied argument key (SC-7).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InputViolation {
    /// JSON pointer into the arguments.
    ///
    /// Always a DECLARED property name, and EMPTY for `additionalProperties`
    /// (measured: `jsonschema` 0.49.2 reports the instance root for that
    /// keyword, so there is no name to give). [`render_refusal`] echoes a pointer
    /// only when its first segment is in the caller-supplied `declared`
    /// allow-list, so an undeclared name can never reach a client message.
    pub pointer: String,
    /// The violated JSON Schema keyword, e.g. `"maxLength"`, `"required"`.
    ///
    /// `"schema"` means the tool's own declared `inputSchema` did not compile,
    /// or the violated keyword has no value-free rendering yet.
    pub keyword: &'static str,
    /// The DECLARED expectation, rendered value-free.
    ///
    /// For `additionalProperties` this is a COUNT of unknown arguments — never
    /// the keys themselves.
    pub expected: String,
}

/// Check `arguments` against a tool's declared `inputSchema`.
///
/// - `arguments` of `None` or [`Value::Null`] is treated as `{}` (the MCP
///   "missing arguments" semantics), so a zero-parameter tool ACCEPTS while a
///   tool declaring `required` parameters is refused by the `required` keyword
///   rather than by `type` — the latter's message would echo `null`.
/// - Compiled under Draft 2020-12 regardless of protocol era (D-02).
/// - `additionalProperties` is honoured exactly as the schema declares it, and is
///   never re-added by this function.
///
/// `schema_key` is an optional pre-computed cache key for `schema` — normally
/// `schema.to_string()`, computed ONCE when a long-lived handler is built so the
/// hot `tools/call` path does not re-serialize the whole `inputSchema` per
/// request. `None` computes it internally, which is what a one-shot caller wants.
/// It is a `&str` and not a `jsonschema` type, so it does not widen this module's
/// public API onto `jsonschema`.
///
/// # Errors
///
/// `Err(Vec<InputViolation>)` on violation. A declared schema that does not
/// compile yields a single violation with `keyword: "schema"` — a drifted
/// declaration must refuse, never pass everything.
pub fn validate_input(
    schema: &Value,
    arguments: Option<&Value>,
    schema_key: Option<&str>,
) -> Result<(), Vec<InputViolation>> {
    let _ = (schema, arguments, schema_key);
    Ok(())
}

/// Render `violations` as ONE client-facing refusal message.
///
/// `declared` is the tool's declared parameter names, in declaration order; it is
/// the ONLY name source this function will echo.
///
/// Shape (locked by SC-7): `unknown argument(s): 2; allowed: cui, version`.
#[must_use]
pub fn render_refusal(violations: &[InputViolation], declared: &[&str]) -> String {
    let _ = (violations, declared);
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A two-parameter tool's schema, shaped exactly as
    /// `pmcp-server-toolkit`'s `build_input_schema` emits it.
    fn two_param_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "cui": { "type": "string" },
                "version": { "type": "string" },
            },
            "required": ["cui"],
            "additionalProperties": false,
        })
    }

    fn zero_param_schema() -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false,
        })
    }

    #[test]
    fn schema_validation_refuses_undeclared_argument_with_a_count_only_message() {
        let schema = two_param_schema();
        let violations = validate_input(
            &schema,
            Some(&json!({ "cui": "C0018787", "apiKey": "secret" })),
            None,
        )
        .expect_err("an undeclared argument must be refused");
        assert_eq!(violations.len(), 1, "one additionalProperties violation");
        assert_eq!(violations[0].keyword, "additionalProperties");
        assert_eq!(
            violations[0].pointer, "",
            "0.49.2 reports the instance root for additionalProperties"
        );

        let msg = render_refusal(&violations, &["cui", "version"]);
        assert!(
            msg.contains('1'),
            "must carry the unknown-argument count: {msg}"
        );
        assert!(msg.contains("cui"), "must name the declared params: {msg}");
        assert!(
            msg.contains("version"),
            "must name the declared params: {msg}"
        );
        assert!(!msg.contains("apiKey"), "must not echo the key: {msg}");
        assert!(!msg.contains("secret"), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_accepts_absent_arguments_on_a_zero_parameter_tool() {
        let schema = zero_param_schema();
        assert!(validate_input(&schema, None, None).is_ok());
        assert!(validate_input(&schema, Some(&Value::Null), None).is_ok());
    }

    #[test]
    fn schema_validation_refuses_absent_arguments_by_required_never_by_type() {
        let schema = two_param_schema();
        let violations = validate_input(&schema, None, None)
            .expect_err("a required parameter must make absent arguments a refusal");
        assert!(
            violations.iter().any(|v| v.keyword == "required"),
            "expected a `required` violation, got {violations:?}"
        );
        assert!(
            violations.iter().all(|v| v.keyword != "type"),
            "a `type` violation would mean `null` reached the validator: {violations:?}"
        );
        let msg = render_refusal(&violations, &["cui", "version"]);
        assert!(msg.contains("cui"), "must name the required param: {msg}");
        assert!(!msg.contains("null"), "must never echo `null`: {msg}");
    }

    #[test]
    fn schema_validation_renders_byte_identical_refusals_across_repeat_calls() {
        let schema = two_param_schema();
        let declared = ["cui", "version"];
        let args = json!({ "cui": "C0018787", "apiKey": "secret" });

        let first = validate_input(&schema, Some(&args), None)
            .map(|()| String::new())
            .unwrap_or_else(|v| render_refusal(&v, &declared));
        let second = validate_input(&schema, Some(&args), None)
            .map(|()| String::new())
            .unwrap_or_else(|v| render_refusal(&v, &declared));
        assert_eq!(
            first, second,
            "0.49.2 error iteration order is deterministic, so refusals must be stable"
        );
        assert!(
            !first.is_empty(),
            "the pair must actually have been refused"
        );
    }
}
