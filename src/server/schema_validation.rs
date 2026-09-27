//! Runtime enforcement of a tool's declared `inputSchema` (Phase 128, D-01).
//!
//! This is the ONE input-validation entry point in the SDK. It lives in core
//! `pmcp` rather than in a consumer crate so that a tool's arguments and its
//! `structuredContent` can never be checked under different dialects, and so the
//! compiled-validator cache, the draft pin and the value-free refusal renderer
//! exist exactly once.
//!
//! # Why this is not `super::output_validation`
//!
//! - **Inputs REFUSE; outputs only warn.** A `tools/call` whose arguments violate
//!   the declared schema must never reach a backend, so `validate_input`
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
//!   (`compile_input_2020_12`). Because that changes compile semantics, inputs
//!   keep their own compile entry point and their own validator cache
//!   (`cached_input_validator`, Q6) — a format-asserting and a non-asserting
//!   validator must never collide on one cache key.
//! - **Refusals are rendered value-free (SC-7).** A `ValidationError`'s `Display`
//!   echoes the rejected value for every keyword, and
//!   `ValidationErrorKind::AdditionalProperties::unexpected` is the caller's own
//!   key list. Neither ever reaches a rendered refusal: see `expectation` and
//!   `render_refusal`. Callers of this module handle PHI.
//!
//! No `jsonschema` type appears in any public signature in this module, so a
//! future `jsonschema` major bump is not a breaking `pmcp` change.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

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
    let validator = match cached_input_validator(schema, schema_key) {
        Ok(v) => v,
        Err(detail) => {
            // The DETAIL is a schema-compilation message (author-supplied schema
            // text, never caller arguments), so it is safe to log server-side —
            // and it is deliberately NOT what the client is told.
            tracing::warn!(
                detail = %detail,
                "declared inputSchema is not a valid JSON Schema; refusing every call to this tool"
            );
            return Err(vec![InputViolation {
                pointer: String::new(),
                keyword: "schema",
                expected: UNCOMPILABLE_SCHEMA.to_string(),
            }]);
        },
    };

    let arguments = effective_arguments(arguments);
    // Fast path: the conforming case is the common one and `is_valid` short-circuits
    // without building any error values.
    if validator.is_valid(arguments) {
        return Ok(());
    }

    let violations: Vec<InputViolation> = validator
        .iter_errors(arguments)
        .map(|e| violation(&e))
        .collect();
    if violations.is_empty() {
        // `is_valid` disagreed with `iter_errors`. Refuse rather than fall through
        // to the backend: an enforcement that cannot describe a failure must still
        // enforce it.
        return Err(vec![InputViolation {
            pointer: String::new(),
            keyword: "schema",
            expected: GENERIC_MISMATCH.to_string(),
        }]);
    }
    Err(violations)
}

/// What a client is told when the tool's own declared `inputSchema` does not
/// compile. Deliberately detail-free — the detail is logged server-side.
const UNCOMPILABLE_SCHEMA: &str = "the tool's declared inputSchema is not a valid JSON Schema";

/// The fallback expectation for a keyword with no value-free rendering yet.
const GENERIC_MISMATCH: &str = "does not match the declared schema";

/// Substitute a missing or `null` `arguments` with `{}` BEFORE the validator sees
/// it.
///
/// Measured necessary: `jsonschema` 0.49.2 refuses `null` against `type: object`
/// with a `Type` error whose message echoes `null`, whereas an empty object is
/// refused by `Required { property }` — a DECLARED name. This substitution is what
/// makes the two easily-conflated acceptance rows come out right: a zero-parameter
/// tool ACCEPTS, and a tool declaring `required` parameters is refused by
/// `required`, never by `type`.
fn effective_arguments(arguments: Option<&Value>) -> &Value {
    static EMPTY: OnceLock<Value> = OnceLock::new();
    match arguments {
        Some(v) if !v.is_null() => v,
        _ => EMPTY.get_or_init(|| Value::Object(serde_json::Map::new())),
    }
}

/// Project one `jsonschema` error into an [`InputViolation`], value-free.
fn violation(e: &jsonschema::ValidationError<'_>) -> InputViolation {
    let (keyword, expected) =
        expectation(e).unwrap_or_else(|| ("schema", GENERIC_MISMATCH.to_string()));
    InputViolation {
        pointer: e.instance_path().to_string(),
        keyword,
        expected,
    }
}

/// The DECLARED expectation behind a validation error, or `None` when this keyword
/// has no value-free rendering yet.
///
/// Every arm reads ONLY declared data. `AdditionalProperties.unexpected` is the
/// caller's own key list — which may itself be sensitive — and is therefore read
/// exclusively through `.len()`. A `ValidationError`'s `Display` is never used: it
/// echoes the rejected value for every keyword.
fn expectation(e: &jsonschema::ValidationError<'_>) -> Option<(&'static str, String)> {
    use jsonschema::error::ValidationErrorKind as K;

    Some(match e.kind() {
        // NEVER read `unexpected` — it is the caller's key list.
        K::AdditionalProperties { unexpected } => (
            "additionalProperties",
            format!("unknown argument(s): {}", unexpected.len()),
        ),
        K::Required { property } => (
            "required",
            format!(
                "`{}` is required",
                property.as_str().unwrap_or(UNNAMED_PROPERTY)
            ),
        ),
        _ => return None,
    })
}

/// Stand-in for a `Required { property }` payload that is not a JSON string. Not
/// reachable from a well-formed schema; present so this path cannot panic.
const UNNAMED_PROPERTY: &str = "<unnamed>";

/// Compile `schema` under an explicitly-pinned Draft 2020-12 with `format`
/// ASSERTION enabled (Q1).
///
/// `format` is annotative by default in `jsonschema` 0.49 — a declared
/// `format: "uri"` accepts `"!!!not-a-uri!!!"` — and a config keyword that
/// silently enforces nothing is the defect class this phase exists to remove. The
/// opt-in changes compile semantics, which is exactly why inputs have their own
/// compile entry point instead of sharing `output_validation::compile_2020_12`.
///
/// `normalize_schema_dialect` is REUSED from `output_validation` rather than
/// copied, so a declared legacy `$schema` is normalized identically for inputs and
/// outputs. That module's `normalize_schema_dialect` / `compile_2020_12` /
/// `cached_validator` split exists to stay under the CI cognitive-complexity gate:
/// this function is a FOURTH sibling of it, never a fifth branch inside it.
fn compile_input_2020_12(
    schema: &Value,
) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    let normalized = super::output_validation::normalize_schema_dialect(schema);
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(&normalized)
}

/// Fetch (or compile and cache) the input validator for `schema`.
///
/// Keyed on the canonical schema TEXT alone, and deliberately SEPARATE from
/// `output_validation::cached_validator`:
///
/// - a format-asserting validator (this one) and a non-asserting one (outputs)
///   must never collide on the same key; and
/// - `Era` is not part of the key because inputs are era-free (D-02) — an
///   `Era`-keyed input cache would encode a distinction that does not exist.
///
/// Compilation errors are cached too, as the error string, so a drifted schema
/// does not recompile on every call.
///
/// `schema_key`, when supplied, is the caller's pre-computed `schema.to_string()`;
/// a cache HIT then costs no serialization at all.
fn cached_input_validator(
    schema: &Value,
    schema_key: Option<&str>,
) -> Result<Arc<jsonschema::Validator>, Arc<str>> {
    type Cache = Mutex<HashMap<String, Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let cache = CACHE.get_or_init(Cache::default);
    // Why: a poisoned mutex here only means another thread panicked while
    // inserting; the map itself is still usable — recover rather than propagate a
    // panic out of a request-path guard.
    let mut map = cache.lock().unwrap_or_else(PoisonError::into_inner);

    // A pre-computed key lets a hit avoid re-serializing the schema entirely.
    if let Some(hit) = schema_key.and_then(|k| map.get(k)) {
        return hit.clone();
    }
    let key = match schema_key {
        Some(k) => k.to_string(),
        None => schema.to_string(),
    };
    map.entry(key)
        .or_insert_with(|| {
            compile_input_2020_12(schema)
                .map(Arc::new)
                .map_err(|e| Arc::from(e.to_string().as_str()))
        })
        .clone()
}

/// Render `violations` as ONE client-facing refusal message.
///
/// `declared` is the tool's declared parameter names, in declaration order; it is
/// the ONLY name source this function will echo.
///
/// Shape (locked by SC-7): `unknown argument(s): 2; allowed: cui, version`.
#[must_use]
pub fn render_refusal(violations: &[InputViolation], declared: &[&str]) -> String {
    let allowed = if declared.is_empty() {
        "(this tool declares no parameters)".to_string()
    } else {
        declared.join(", ")
    };
    if violations.is_empty() {
        return format!("arguments do not match the declared schema; allowed: {allowed}");
    }
    violations
        .iter()
        .map(|v| render_one(v, declared, &allowed))
        .collect::<Vec<String>>()
        .join("; ")
}

/// Render ONE violation.
///
/// An `additionalProperties` refusal is the locked SC-7 shape: a COUNT plus the
/// declared allow-list, because the keyword reports the instance root and there is
/// no safe name to give. Every other keyword may name its location — but ONLY when
/// the pointer's first segment is a DECLARED parameter, so a pointer segment that
/// came from the caller can never reach a client message.
fn render_one(v: &InputViolation, declared: &[&str], allowed: &str) -> String {
    if v.keyword == "additionalProperties" {
        return format!("{}; allowed: {allowed}", v.expected);
    }
    let first = v.pointer.trim_start_matches('/').split('/').next();
    match first {
        Some(name) if !name.is_empty() && declared.contains(&name) => {
            format!("{}: {}", v.pointer, v.expected)
        },
        _ => v.expected.clone(),
    }
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

        let refuse = || {
            let violations = validate_input(&schema, Some(&args), None)
                .expect_err("the pair must actually have been refused");
            render_refusal(&violations, &declared)
        };
        let first = refuse();
        let second = refuse();
        assert_eq!(
            first, second,
            "0.49.2 error iteration order is deterministic, so refusals must be stable"
        );
        assert!(!first.is_empty(), "a refusal must never render empty");
    }
}
