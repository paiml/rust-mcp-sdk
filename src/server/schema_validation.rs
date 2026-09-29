//! Runtime enforcement of a tool's declared `inputSchema` (Phase 128, D-01).
//!
//! This is the ONE **JSON-Schema** input-validation entry point in the SDK. It
//! lives in core `pmcp` rather than in a consumer crate so that a tool's
//! arguments and its `structuredContent` can never be checked under different
//! dialects, and so the compiled-validator cache, the draft pin and the
//! value-free refusal renderer exist exactly once.
//!
//! The qualifier is load-bearing: `pmcp_server_toolkit::workbook::input::validate_input`
//! is a SECOND input validator over a declared tool surface. It is not a second
//! copy of this one and does not belong here — it checks a `CalculateInput`
//! against a workbook `Manifest` + `CellMap` (dtype, closed-enum membership,
//! strict-constant overrides), which is a DTO/tier rule set with no JSON Schema
//! anywhere in it. Anything that IS a JSON Schema check on tool arguments belongs
//! in this module.
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
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

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
        .map(|e| violation(&e, schema))
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
///
/// `schema` is the DECLARED schema the error came from; it is what lets
/// `safe_pointer` tell a declared property name apart from a caller-chosen one.
fn violation(e: &jsonschema::ValidationError<'_>, schema: &Value) -> InputViolation {
    let (keyword, expected) =
        expectation(e).unwrap_or_else(|| ("schema", GENERIC_MISMATCH.to_string()));
    InputViolation {
        pointer: safe_pointer(e, schema),
        keyword,
        expected,
    }
}

/// The fixed token a non-declared pointer segment is replaced by.
///
/// Carries no length and no hash of the redacted key: a length is a side channel
/// on a value that may itself be PHI (T-128-08a).
///
/// `pub(crate)` so the E3 path (`super::typed_tool`'s `garde` mapping) redacts a
/// caller-chosen `garde::Path` segment with the SAME token rather than a second
/// literal that could drift — a D1 and an E3 refusal must read alike (T-128-17b).
pub(crate) const REDACTED_SEGMENT: &str = "<redacted>";

/// Project `e`'s instance pointer onto the DECLARED schema, redacting every
/// segment whose name came from the instance rather than from the declaration.
///
/// # Why this is a projection and not a copy
///
/// RESEARCH Finding 1c records that `instance_path()` "is safe for declared
/// properties" — and that qualifier is load-bearing. [`validate_input`] is public
/// and accepts arbitrary schemas, so the qualifier does not hold in general.
/// Measured counterexample: under
/// `{"type":"object","additionalProperties":{"type":"integer"}}` — or under any
/// schema using `patternProperties` — the property name is chosen by the CALLER
/// and appears verbatim in the pointer, so
/// `{"Jane Doe DOB 1970-01-01": "x"}` yields the pointer
/// `/Jane Doe DOB 1970-01-01`. Copying that into a refusal violates SC-7 even
/// though no `ValidationError` was ever `Display`-formatted, which is precisely
/// the leak this module exists to prevent.
///
/// A segment is emitted VERBATIM only when it is
///
/// - a key of the current schema node's `properties` map (a DECLARED name), or
/// - a base-10 integer, i.e. an array index, which carries no caller-chosen text.
///
/// Every other segment becomes [`REDACTED_SEGMENT`]. An `additionalProperties:
/// false` violation keeps the empty pointer `jsonschema` already reports for it,
/// so it never names a key at all.
fn safe_pointer(e: &jsonschema::ValidationError<'_>, schema: &Value) -> String {
    let raw = e.instance_path().as_str();
    if raw.is_empty() {
        return String::new();
    }
    let mut node = Some(schema);
    let mut out = String::new();
    for token in raw.trim_start_matches('/').split('/') {
        let (rendered, next) = project_pointer_segment(node, token);
        out.push('/');
        out.push_str(rendered);
        node = next;
    }
    out
}

/// One step of [`safe_pointer`]'s walk: what to emit, and the schema node the
/// next segment is resolved against.
fn project_pointer_segment<'a>(
    node: Option<&'a Value>,
    token: &'a str,
) -> (&'a str, Option<&'a Value>) {
    if !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_digit()) {
        // An array index. `items` is the 2020-12 object form; array-form `items`
        // does not compile under this module's pin (RESEARCH Finding 1h), so a
        // single subschema is the only shape reachable here.
        return (token, node.and_then(|n| n.get("items")));
    }
    let decoded = unescape_pointer_token(token);
    match node
        .and_then(|n| n.get("properties"))
        .and_then(|properties| properties.get(decoded.as_ref()))
    {
        // Declared: emit the ORIGINAL token, so the pointer stays valid RFC 6901.
        Some(child) => (token, Some(child)),
        None => (REDACTED_SEGMENT, None),
    }
}

/// Decode one RFC 6901 pointer token (`~1` -> `/`, `~0` -> `~`), in that order.
///
/// Only needed for the `properties` lookup — the emitted text is always the
/// original, still-escaped token.
fn unescape_pointer_token(token: &str) -> Cow<'_, str> {
    if token.contains('~') {
        Cow::Owned(token.replace("~1", "/").replace("~0", "~"))
    } else {
        Cow::Borrowed(token)
    }
}

/// Render a SCHEMA-COMPILATION error's own text.
///
/// This is the ONLY place in this module where a `jsonschema` error is
/// `Display`-formatted, and it is deliberately reachable from exactly two
/// callers: the server-side `tracing::warn!` in [`validate_input`] and
/// [`check_input_schema_compiles`], whose audience is a CONFIG AUTHOR.
///
/// The distinction is not cosmetic. A *validation* error's `Display` echoes the
/// rejected caller value for every keyword (RESEARCH Finding 1b) and must never
/// be rendered — that is what `expectation` exists for. A *compilation* error
/// describes author-supplied schema text and contains no caller data at all, so
/// rendering it is safe in both of those positions and in neither is it sent to
/// an MCP client on a `tools/call` path.
fn compile_error_detail(error: &jsonschema::ValidationError<'_>) -> String {
    format!("{error}")
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
        // `unexpected` is the CALLER-SUPPLIED key list — it is the attacker's own
        // text and may itself be PHI. It is read EXCLUSIVELY through `.len()`;
        // never iterate it, never index it, never format it.
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
        K::MaxLength { limit } => ("maxLength", format!("at most {limit} characters")),
        K::MinLength { limit } => ("minLength", format!("at least {limit} characters")),
        K::Pattern { pattern } => ("pattern", format!("must match {pattern}")),
        K::Maximum { limit } => ("maximum", format!("at most {limit}")),
        K::Minimum { limit } => ("minimum", format!("at least {limit}")),
        K::Enum { options } => ("enum", format!("one of {options}")),
        K::MaxItems { limit } => ("maxItems", format!("at most {limit} items")),
        K::Type { kind } => ("type", format!("must be {}", type_expectation(kind))),
        // Q1 turns format ASSERTION on for inputs, so this is a kind this module
        // actively produces rather than one it merely tolerates. The `format`
        // name is declared config content and is safe to echo; without this arm a
        // `format` refusal would be indistinguishable from an unknown failure and
        // would quietly undercut the reason Q1 chose to enforce it at all.
        K::Format { format } => ("format", format!("must be a valid {format}")),
        // The regex engine's OWN ReDoS guard firing on caller input against a
        // config-declared `pattern`. This is simultaneously a refusal and a
        // signal about the declaration, so an operator needs it in the logs; the
        // log line names the DECLARED schema position only (`schema_path`, e.g.
        // `/properties/cui/pattern`) and never the value. That log line is the
        // instrument for T-128-10's accepted assumption-A2 residual, and
        // `fuzz_placeholder_pattern_redos` (plan 10) is what looks for it
        // deliberately.
        K::BacktrackLimitExceeded { .. } => {
            tracing::warn!(
                schema_path = %e.schema_path(),
                "declared `pattern` hit the regex backtracking limit on caller input; refusing"
            );
            ("pattern", BACKTRACK_LIMIT.to_string())
        },
        _ => return None,
    })
}

/// Render a `Type` violation's DECLARED type (or type union), value-free.
fn type_expectation(kind: &jsonschema::error::TypeKind) -> String {
    use jsonschema::error::TypeKind;
    match kind {
        TypeKind::Single(declared) => declared.as_str().to_owned(),
        TypeKind::Multiple(declared) => declared
            .iter()
            .map(jsonschema::JsonType::as_str)
            .collect::<Vec<&str>>()
            .join(" or "),
    }
}

/// What a client is told when the engine's backtracking limit fires.
///
/// Names neither the pattern nor the value: which of the two is at fault is not
/// decidable from here, and the pair is exactly what an attacker probing a
/// pathological declared pattern would want confirmed.
const BACKTRACK_LIMIT: &str = "could not be evaluated against the declared pattern";

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

type InputValidatorCache = RwLock<HashMap<String, Result<Arc<jsonschema::Validator>, Arc<str>>>>;

/// The process-global memo behind [`cached_input_validator`]. Module-scope so the
/// bound below is observable from a test.
static INPUT_VALIDATOR_CACHE: OnceLock<InputValidatorCache> = OnceLock::new();

/// Most distinct schemas [`cached_input_validator`] will remember.
///
/// The memo is keyed on schema TEXT and never evicts, so without a bound its size
/// is a function of how many DISTINCT schemas the process ever sees. For a
/// config-driven server that is the number of declared tools and patterns, which
/// is small and fixed. But `validate_input` is `pub`, and
/// `fuzz_placeholder_pattern_redos` drives arbitrary declared patterns through
/// `declared_pattern_check`: measured, that target reached libFuzzer's 2048 MB RSS
/// limit in about 130k executions (~8 KB retained per distinct pattern), both in CI
/// and locally.
///
/// Once the map holds this many entries a NEW schema is compiled and returned
/// WITHOUT being stored. That is the same shape as the toolkit's
/// `MAX_REMEMBERED` memo in `code_mode.rs`, and for the same reason: enforcement
/// never depends on the memo. A full cache costs a recompile per call for schemas
/// beyond the bound; it never changes a verdict. Schemas already stored keep
/// hitting.
///
/// 4096 is deliberately far above any realistic server (a 500-tool server with a
/// patterned parameter each is ~1,500 schemas), so the ordinary case never sees the
/// bound, while worst-case retained memory stays in the tens of megabytes.
const MAX_CACHED_VALIDATORS: usize = 4096;

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
    let cache = INPUT_VALIDATOR_CACHE.get_or_init(InputValidatorCache::default);

    // An `RwLock` rather than a `Mutex` because the steady state is ALL reads: the
    // map is written once per distinct schema and then hit on every `tools/call`
    // forever. Under a `Mutex` every validation in the process serializes against
    // every other one, across all tools, for a lookup that mutates nothing.
    //
    // Why the poison recovery: a poisoned lock here only means another thread
    // panicked while inserting; the map itself is still usable — recover rather
    // than propagate a panic out of a request-path guard.
    //
    // The key is resolved BEFORE the lookup so that BOTH callers reach the read
    // path. Gating the read probe on `schema_key.is_some()` instead would leave
    // `declared_pattern_check` — which passes `None` and runs once per declared
    // `pattern` per placeholder per request — taking the EXCLUSIVE write lock on
    // every request, excluding exactly the readers an `RwLock` exists to admit.
    // A pre-computed key additionally lets a hit avoid re-serializing the schema.
    let key: Cow<'_, str> = match schema_key {
        Some(k) => Cow::Borrowed(k),
        None => Cow::Owned(schema.to_string()),
    };

    {
        let map = cache.read().unwrap_or_else(PoisonError::into_inner);
        if let Some(hit) = map.get(key.as_ref()) {
            return hit.clone();
        }
    }

    // Compiled OUTSIDE the write lock. Compilation builds the regex set for every
    // declared `pattern`, and holding the exclusive lock across it stalls every
    // reader in the process. Losing a race costs one redundant compile, which
    // `or_insert` then discards — cheaper than serializing all readers behind it.
    let compiled = compile_input_2020_12(schema)
        .map(Arc::new)
        // `compile_error_detail` is the ONE audited Display-render site; this is a
        // COMPILATION error, so it carries author-supplied schema text and no
        // caller data.
        .map_err(|error| Arc::from(compile_error_detail(&error).as_str()));

    let mut map = cache.write().unwrap_or_else(PoisonError::into_inner);
    remember_bounded(&mut map, MAX_CACHED_VALIDATORS, key.into_owned(), compiled)
}

/// Store `compiled` under `key` unless the map already holds `cap` entries, and
/// return the entry the caller should use.
///
/// Split out of [`cached_input_validator`] so the bound is testable on a LOCAL map
/// with a tiny cap. Testing it through the process-global cache would fill that
/// cache for every other test in the binary and break the ones that assert a
/// cache hit.
///
/// When full, a NEW key is returned uncached. A key already present — we lost a
/// compile race to another thread — still resolves to the stored entry, so two
/// racing callers never disagree about which `Arc` is canonical.
fn remember_bounded(
    map: &mut HashMap<String, Result<Arc<jsonschema::Validator>, Arc<str>>>,
    cap: usize,
    key: String,
    compiled: Result<Arc<jsonschema::Validator>, Arc<str>>,
) -> Result<Arc<jsonschema::Validator>, Arc<str>> {
    if map.len() >= cap && !map.contains_key(&key) {
        return compiled;
    }
    map.entry(key).or_insert(compiled).clone()
}

/// Check that `schema` compiles as a Draft 2020-12 input schema — the
/// CONFIG-TIME gate (SC-2).
///
/// This exists as a separate entry point from [`validate_input`] because its
/// audience is different. Here the schema is AUTHOR-supplied (a server's own
/// config, or a bundled `OpenAPI` spec) and there is no caller data anywhere in
/// scope, so echoing the compile error's own detail is exactly what the author
/// needs. The SC-7 no-echo rule governs the client-facing `tools/call` path, and
/// on that path a non-compiling declared schema still yields the detail-free
/// refusal [`validate_input`] returns.
///
/// The returned `pointer` comes from the compile error's `schema_path`, which
/// points straight at the offending declaration — measured as
/// `/properties/<param>/pattern` for a nested non-compiling `pattern`
/// (RESEARCH Finding 1e). No projection through `safe_pointer` is needed or
/// wanted: a schema path is entirely declaration-derived.
///
/// # `jsonschema::meta::is_valid` is NOT the check
///
/// It returns `true` for a schema whose nested `pattern` does not compile
/// (measured, RESEARCH Finding 1e), so a meta-schema validation would pass over
/// the single most common authoring mistake this gate exists to catch. The check
/// has to be a real Draft 2020-12 compile of the synthesized `inputSchema`.
///
/// # Errors
///
/// `Err(InputViolation)` with `keyword: "schema"` when `schema` does not compile.
pub fn check_input_schema_compiles(schema: &Value) -> Result<(), InputViolation> {
    match compile_input_2020_12(schema) {
        Ok(_) => Ok(()),
        Err(error) => Err(InputViolation {
            pointer: error.schema_path().as_str().to_string(),
            keyword: "schema",
            expected: compile_error_detail(&error),
        }),
    }
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

// ===========================================================================
// D4 — the ONE copy of the path-placeholder character floor.
// ===========================================================================

/// The hard upper bound on a path-placeholder value, in Unicode code points.
///
/// Deliberately a module CONSTANT and not a configuration value. D-08 requires
/// the length half of the CR-01 fix to hold regardless of how D3 is configured —
/// in particular when `default_max_length` is set to `0`, which would otherwise
/// silently disable the "Length via two placeholders" acceptance row along with
/// the free-text policy. A validation rule that a configuration value can switch
/// off is the silent-hole class this phase exists to remove.
///
/// Counted in code points, not bytes and not grapheme clusters, so it agrees
/// with `jsonschema`'s own `maxLength` semantics (RESEARCH Finding 1d).
pub const PLACEHOLDER_MAX_LENGTH: usize = 256;

/// The per-parameter narrowing a caller may declare on top of the floor.
///
/// `Default` means FLOOR-PLUS-CAP WITH NO NARROWING — all-`None` plus
/// `allow_slash: false`. It emphatically does NOT mean "no checks": the
/// unconditional floor and [`PLACEHOLDER_MAX_LENGTH`] still apply, which is why
/// [`PlaceholderRules::default()`] is a safe value for a trait's default method
/// body to return.
///
/// This struct is `#[non_exhaustive]`, so another crate cannot build it with a
/// struct literal. Use [`PlaceholderRules::default()`] and the `with_*` builders:
///
/// ```ignore
/// let rules = PlaceholderRules::default()
///     .with_pattern(Some("^C[0-9]+$"))
///     .with_max_length(Some(32));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct PlaceholderRules<'a> {
    /// A `pattern` declared for this parameter, which NARROWS the floor.
    ///
    /// Never replaces it: a spec pattern of `^.*$` accepts every CR-01 payload
    /// (measured, RESEARCH Finding 5b), so pattern-supersedes-denylist would be a
    /// no-op (D-10).
    pub declared_pattern: Option<&'a str>,
    /// A `maxLength` declared for this parameter.
    ///
    /// Narrows [`PLACEHOLDER_MAX_LENGTH`] further; a declared length LARGER than
    /// the constant never widens it.
    pub declared_max_length: Option<usize>,
    /// Whether `/` is permitted inside this parameter's value.
    ///
    /// Its ONLY legitimate source is an explicit per-parameter entry in the
    /// server's own config (D-11). An `OpenAPI` spec's `allowReserved` must never
    /// be wired to it: a spec is third-party content baked into the package and
    /// that keyword is widely copy-pasted without intent. Even when this is
    /// `true`, the parent-directory sequence is refused with no escape.
    pub allow_slash: bool,
}

impl<'a> PlaceholderRules<'a> {
    /// Declare a narrowing `pattern`.
    #[must_use]
    pub fn with_pattern(mut self, pattern: Option<&'a str>) -> Self {
        self.declared_pattern = pattern;
        self
    }

    /// Declare a narrowing `maxLength`.
    #[must_use]
    pub fn with_max_length(mut self, max_length: Option<usize>) -> Self {
        self.declared_max_length = max_length;
        self
    }

    /// Opt this parameter in to `/` (D-11 — config only, never spec-derived).
    #[must_use]
    pub fn allowing_slash(mut self, allow_slash: bool) -> Self {
        self.allow_slash = allow_slash;
        self
    }
}

/// Why a path-placeholder value was refused.
///
/// Carries the DECLARED parameter name and the DECLARED expectation only — never
/// a byte of the rejected value, and never which character tripped the floor
/// (naming the character would itself echo a byte of the value and turn the
/// refusal into a one-byte oracle).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PlaceholderRefusal {
    /// The DECLARED parameter name, safe to echo.
    pub param: String,
    /// Which rule refused: `"nonEmpty"`, `"percentEncoding"`, `"characterFloor"`,
    /// `"maxLength"`, `"pattern"`, `"segmentMaxLength"` or `"pathSegment"`.
    pub rule: &'static str,
    /// The DECLARED expectation, rendered value-free.
    pub expected: String,
}

impl std::fmt::Display for PlaceholderRefusal {
    /// Renders in the `render_scalar` house style (PATTERNS SP-1): name the
    /// parameter, state the declared expectation, never the value.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "param '{}' {}", self.param, self.expected)
    }
}

impl std::error::Error for PlaceholderRefusal {}

/// The expectation a floor refusal states when `/` is denied.
const FLOOR_EXPECTATION: &str = "must not contain a path separator, a \
     parent-directory sequence, a query or fragment marker, a backslash or any \
     control character — in literal or percent-encoded form — and must not be a \
     single dot";

/// The same, for a parameter that opted in to `/` (D-11).
const FLOOR_EXPECTATION_SLASH_ALLOWED: &str = "must not contain a \
     parent-directory sequence, a query or fragment marker, a backslash or any \
     control character — in literal or percent-encoded form — and must not be a \
     single dot";

/// The expectation a percent-handling refusal states.
const PERCENT_EXPECTATION: &str = "must not contain an encoded percent sign \
     (`%25`) or a malformed percent escape";

/// The fixed `param` a composed-path refusal carries.
///
/// [`validate_resolved_path`] is param-agnostic by construction: the composition
/// it checks belongs to no single parameter, so there is no declared name to give
/// and a caller-supplied one must never be substituted.
const COMPOSED_POSITION: &str = "path segment";

/// Validate ONE path-placeholder value before substitution (D4).
///
/// This is the single copy of the D4 character floor in the SDK. It lives in core
/// `pmcp` — not in `pmcp-code-mode` — because BOTH HTTP surfaces must reach it:
/// the curated single-call build resolves to the toolkit's `http` feature, whose
/// dependency list carries no `pmcp-code-mode` edge (RESEARCH Finding 6), so a
/// helper exported only from there would force either a new dependency edge that
/// widens the curated graph or two copies of one security rule — the drift class
/// this repo has already been bitten by. D-09's published-helper obligation is met
/// by a `pub use` re-export from `pmcp-code-mode`.
///
/// # The four steps, and why the order is load-bearing
///
/// 1. **Unconditional floor**, which no declared pattern can relax (D-10). It is
///    implemented as DECODE ONCE, THEN DENY — never as an enumerated denylist of
///    literal and pre-encoded spellings, because an enumeration is incomplete by
///    construction: a mixed form such as `.%2E` or `%2E.` decodes to the
///    parent-directory sequence while matching neither the literal nor the
///    fully-encoded spelling. Enumerating more spellings does not converge;
///    decoding does.
/// 2. **Always-on cap** at [`PLACEHOLDER_MAX_LENGTH`] code points (D-08).
/// 3. **Declared pattern narrows** — evaluated through `cached_input_validator`,
///    so a placeholder pattern and an `inputSchema` pattern resolve `\s` through
///    the identical engine AND a repeated pattern compiles once.
/// 4. **Declared length narrows further**; a declared length larger than the
///    module constant never widens it.
///
/// Running the floor FIRST is empirically justified, not stylistic: a spec pattern
/// of `^.*$` accepts every CR-01 payload (measured, RESEARCH Finding 5b), so
/// pattern-supersedes-floor would have silently disabled the check.
///
/// This is a pure function holding no shared mutable state of its own, so two
/// concurrent Code Mode calls on one executor cannot interleave placeholder state.
///
/// `value` is the value ALREADY RENDERED to a string by the caller (the toolkit's
/// `render_scalar`), so the floor and the cap see the rendered text rather than a
/// JSON number or bool.
///
/// # Not sufficient on its own
///
/// Per-value checks cannot establish final-path safety. Every caller MUST also run
/// [`validate_resolved_path`] on the composed path before dispatch.
///
/// # Errors
///
/// `Err(PlaceholderRefusal)` naming the DECLARED parameter and the DECLARED
/// expectation, never the value.
pub fn validate_path_placeholder(
    param: &str,
    value: &str,
    rules: &PlaceholderRules<'_>,
) -> Result<(), PlaceholderRefusal> {
    // 1. UNCONDITIONAL FLOOR — before any declared narrowing (D-10).
    placeholder_floor(param, value, rules.allow_slash)?;

    // 2. ALWAYS-ON CAP (D-08), in code points to agree with `maxLength`.
    let length = value.chars().count();
    if length > PLACEHOLDER_MAX_LENGTH {
        return Err(refusal(
            param,
            "maxLength",
            format!("must be at most {PLACEHOLDER_MAX_LENGTH} characters"),
        ));
    }

    // 3. DECLARED PATTERN NARROWS (never replaces).
    if let Some(pattern) = rules.declared_pattern {
        declared_pattern_check(param, value, pattern)?;
    }

    // 4. DECLARED LENGTH NARROWS FURTHER. A declared length larger than the
    //    module constant cannot widen it, because step 2 already ran.
    if let Some(declared) = rules.declared_max_length {
        if length > declared {
            return Err(refusal(
                param,
                "maxLength",
                format!("must be at most {declared} characters"),
            ));
        }
    }
    Ok(())
}

/// Validate the COMPOSED path after substitution and before dispatch.
///
/// # Why a per-value check is insufficient by construction
///
/// This is a proof, not a caution. Two values that each pass
/// [`validate_path_placeholder`] independently can compose into a refused form
/// across adjacent placeholders:
///
/// - `/search/{a}{b}` with `a` at 180 code points and `b` at 200 yields a
///   380-code-point segment — over the cap, with neither part over it.
/// - `/x/{a}{b}` with `a = "."` and `b = "."` yields the segment `..` — traversal
///   — from two values neither of which contains the sequence.
///
/// Adjacency is reachable rather than theoretical: on the Code Mode surface the
/// substitution is genuinely per-`{key}`, and the composed result is then parsed
/// as a URL, which is where a composed traversal becomes a different endpoint.
///
/// # Contract
///
/// The same decode-once normalization as the per-value floor runs over the WHOLE
/// path; then the path is split on `/` and each segment is refused when it is
/// longer than [`PLACEHOLDER_MAX_LENGTH`] code points, equal to the
/// parent-directory sequence, equal to a single dot, or empty other than the
/// leading segment a path starting with `/` produces. A residual `{` or `}` is
/// refused — an unsubstituted placeholder reaching the wire is its own defect —
/// and `?`, `#`, a backslash and any control byte are refused anywhere.
///
/// # The three empty-segment cases, decided rather than derived
///
/// The segment split reports an empty slice in three different situations, and
/// they get three different answers on purpose:
///
/// | Path | Verdict | Why |
/// |---|---|---|
/// | `/` | ACCEPTED | the absolute root has no segments; it is the shortest legal absolute path and the target of a `GET /` operation |
/// | `/a//b` | refused | a doubled separator changes the endpoint shape |
/// | `/a/b/` | refused | this is what `/search/{v}` composes to when `v` is empty — the empty-placeholder-at-the-tail case |
///
/// The first was a defect until Phase 128 CR-01: it fell out of the index
/// arithmetic (only index 0 is exempt, and a bare `/` produces empty slices at
/// indices 0 AND 1) rather than out of a decision, so `validate_resolved_path("/")`
/// refused and every call to a root operation failed at runtime with the
/// param-agnostic `param 'path segment' must not be empty`.
///
/// The refusal is param-agnostic: it carries the fixed position `path segment`
/// rather than a caller-supplied name, because the composition belongs to no
/// single parameter.
///
/// # Errors
///
/// `Err(PlaceholderRefusal)` describing the position and the expectation, never
/// the path.
pub fn validate_resolved_path(path: &str) -> Result<(), PlaceholderRefusal> {
    let decoded = decode_once(COMPOSED_POSITION, path)?;
    if decoded
        .iter()
        .any(|byte| denied_byte(*byte, /* allow_slash */ true) || matches!(byte, b'{' | b'}'))
    {
        return Err(refusal(
            COMPOSED_POSITION,
            "pathSegment",
            format!(
                "{FLOOR_EXPECTATION_SLASH_ALLOWED}, and must carry no unsubstituted placeholder"
            ),
        ));
    }
    check_resolved_segments(&decoded)
}

/// [`validate_resolved_path`] widened by the single author-written query
/// separator, for a composed path that may carry a `?`.
///
/// Both HTTP surfaces compose a path template with resolved placeholder values
/// and then check the result. An operator may write a literal `?` in the
/// template, so the composed string is a path AND a query — while
/// [`validate_resolved_path`] denies `?` anywhere, deliberately, because a `?`
/// arriving from a *value* silently changes the endpoint.
///
/// The narrowing is therefore: split at the FIRST `?` and apply the unmodified
/// rule to each half. It lives HERE, beside the rule it widens, rather than at
/// either call site — the curated surface (`HttpClient::check_composed_path`)
/// and the Code Mode surface (`ResolvedPath::from_checked`) previously held one
/// copy each, which made this the one part of the floor that could drift
/// between them. One more sibling, never a second copy.
///
/// What the split does NOT relax:
///
/// - A SECOND `?` is still refused: only the first is split off, so the query
///   portion faces the unmodified rule, which denies `?`.
/// - An empty query portion is still refused — a dangling `/x?` is a trailing
///   separator, the same class as a trailing `/`.
/// - A `?` reaching the composed string from a placeholder VALUE never gets
///   here; the per-value floor has already refused it.
///
/// # Two inherited conservatisms, stated so they are not a surprise
///
/// Both come from applying the unmodified rule to the query half, and both are
/// what the two former call-site copies already did — neither is new here.
///
/// 1. `%25` is refused outright (it is what bounds the decode to a single pass),
///    so a query carrying a percent-encoded percent sign is refused.
/// 2. The query half also faces the rules about path SHAPE — no `//`, no trailing
///    `/`, no empty segment — because `validate_resolved_path` checks bytes AND
///    segment structure together. So `/a?redirect=https://example.com`,
///    `/a?b=//x` and `/a?b=1&c=x/` are all refused. This is the one an operator
///    actually hits: a query value holding a URL does not compose. Asserted by
///    `resolved_target_applies_path_segment_structure_to_the_query_too` so it
///    cannot change silently. Relaxing it means splitting the byte floor from the
///    segment-structure rules and applying only the former to the query half —
///    deliberately NOT done here, because widening a security floor is a decision
///    for its own change, not a side effect of de-duplicating two copies.
///
/// # Errors
///
/// The [`PlaceholderRefusal`] from [`validate_resolved_path`]. It is value-free:
/// it names the rule and the declared expectation, never a byte of the path.
pub fn validate_resolved_target(path: &str) -> Result<(), PlaceholderRefusal> {
    match path.split_once('?') {
        // No author-written separator: the whole string is a path.
        None => validate_resolved_path(path),
        // Exactly one author-written `?`. The separator itself is permitted;
        // both sides still face the full, unmodified rule set.
        Some((path_part, query_part)) => {
            validate_resolved_path(path_part).and_then(|()| validate_resolved_path(query_part))
        },
    }
}

/// Per-segment half of [`validate_resolved_path`], split out to keep both
/// functions inside the cognitive-complexity budget.
fn check_resolved_segments(decoded: &[u8]) -> Result<(), PlaceholderRefusal> {
    // The absolute root is a legal path with NO segments at all — the shortest
    // legal absolute path, and the target of the `GET /` health/index operation
    // every second `OpenAPI` document declares. `[b'/'].split(b'/')` reports it as
    // TWO empty slices (before and after the separator), and only index 0 is
    // exempt, so without this the root path is refused with the param-agnostic
    // `must not be empty` on every single request.
    //
    // Deliberately an equality test on the WHOLE decoded path rather than a
    // relaxation of the empty-segment rule: `//` and a trailing `/` stay refused,
    // both by decision. `/a/b/` is what `/search/{v}` composes to when `v` is
    // empty, which is the case the non-leading-empty rule exists to close, and a
    // doubled separator changes the endpoint shape. Asserted in both directions by
    // `resolved_path_accepts_the_absolute_root_and_still_refuses_doubled_and_trailing`.
    if decoded == b"/" {
        return Ok(());
    }
    let leading_slash = decoded.first() == Some(&b'/');
    for (index, segment) in decoded.split(|byte| *byte == b'/').enumerate() {
        check_one_resolved_segment(segment, index == 0 && leading_slash)?;
    }
    Ok(())
}

/// One composed path segment. Split out from [`check_resolved_segments`] because
/// the two together measured cognitive complexity 28 against the blocking CI cap
/// of 25 — the same reason `output_validation.rs` carries its three-function
/// split.
///
/// `leading` marks the one legitimate empty segment: the one an absolute path
/// produces before its first `/`.
fn check_one_resolved_segment(segment: &[u8], leading: bool) -> Result<(), PlaceholderRefusal> {
    if segment.is_empty() {
        // Any OTHER empty segment means a doubled or trailing `/`, which changes
        // the endpoint shape — and is exactly what an empty placeholder value
        // substituted at the tail of a template produces.
        if leading {
            return Ok(());
        }
        return Err(refusal(
            COMPOSED_POSITION,
            "pathSegment",
            "must not be empty".to_string(),
        ));
    }
    if segment == b".." || segment == b"." {
        return Err(refusal(
            COMPOSED_POSITION,
            "pathSegment",
            "must not be a relative path reference".to_string(),
        ));
    }
    if String::from_utf8_lossy(segment).chars().count() > PLACEHOLDER_MAX_LENGTH {
        return Err(refusal(
            COMPOSED_POSITION,
            "segmentMaxLength",
            format!("must be at most {PLACEHOLDER_MAX_LENGTH} characters"),
        ));
    }
    Ok(())
}

/// Build a refusal. Kept as one helper so no call site can forget that the
/// rejected value is never a field.
fn refusal(param: &str, rule: &'static str, expected: String) -> PlaceholderRefusal {
    PlaceholderRefusal {
        param: param.to_owned(),
        rule,
        expected,
    }
}

/// Step 1 — the unconditional floor (D-10), as decode-once-then-deny.
fn placeholder_floor(
    param: &str,
    value: &str,
    allow_slash: bool,
) -> Result<(), PlaceholderRefusal> {
    if value.is_empty() {
        return Err(refusal(param, "nonEmpty", "must not be empty".to_string()));
    }
    let decoded = decode_once(param, value)?;
    let floor_expectation = if allow_slash {
        FLOOR_EXPECTATION_SLASH_ALLOWED
    } else {
        FLOOR_EXPECTATION
    };
    let denied = decoded.iter().any(|byte| denied_byte(*byte, allow_slash))
        // The parent-directory sequence has NO escape, even with `allow_slash`
        // (D-11).
        || decoded.windows(2).any(|pair| pair == b"..")
        // A single dot is a meaningful path segment (`a/./b` normalizes to
        // `a/b`), so two adjacent placeholders each holding `.` compose to `..`.
        // Refusing it closes that composition at the value layer as well as in
        // `validate_resolved_path`.
        // `&decoded[..]` and not `decoded.as_ref()`: the latter picks its target
        // type by inference, so widening `decode_once` to another `Cow` target
        // would silently re-resolve this comparison rather than fail to compile.
        || &decoded[..] == b".";
    if denied {
        return Err(refusal(
            param,
            "characterFloor",
            floor_expectation.to_string(),
        ));
    }
    Ok(())
}

/// Steps 1a-1c — refuse `%25` outright, refuse a malformed escape, then
/// percent-decode exactly ONCE.
///
/// Refusing `%25` in any hex case BEFORE decoding is what makes one pass
/// sufficient rather than the first round of an unbounded regress: with `%25`
/// refused, no surviving input can encode a further `%`, so a single decode
/// reaches ground truth. A malformed escape is refused because leaving it
/// undecided would mean every downstream layer deciding for itself whether to
/// treat it as a literal `%` or as an error.
fn decode_once<'v>(param: &str, value: &'v str) -> Result<Cow<'v, [u8]>, PlaceholderRefusal> {
    let bytes = value.as_bytes();

    // Fast path: a value with no `%` at all has nothing to decode and nothing that
    // could be a `%25`, so it needs neither the pre-scan nor a copy. This is the
    // overwhelmingly common case, and both callers only READ the result — the
    // composed-path check and the per-value floor each scan it and hand it to
    // `check_resolved_segments`. Borrow instead of allocating.
    if !bytes.contains(&b'%') {
        return Ok(Cow::Borrowed(bytes));
    }

    if contains_ascii_case_insensitive(value, "%25") {
        return Err(refusal(
            param,
            "percentEncoding",
            PERCENT_EXPECTATION.to_string(),
        ));
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let decoded = bytes
                .get(index + 1)
                .zip(bytes.get(index + 2))
                .and_then(|(high, low)| Some(hex_nibble(*high)? * 16 + hex_nibble(*low)?));
            let Some(byte) = decoded else {
                return Err(refusal(
                    param,
                    "percentEncoding",
                    PERCENT_EXPECTATION.to_string(),
                ));
            };
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    Ok(Cow::Owned(out))
}

/// One ASCII hex digit's value, case-insensitively; `None` for a non-hex byte.
fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Step 1d's denylist, over ONE decoded byte.
///
/// `\` is denied because IIS, some nginx rewrite configurations and AWS API
/// Gateway normalize it toward `/` and `..\` toward traversal, so a floor that
/// stops `../` and passes `..\` is deployment-dependent rather than sound. CR, LF
/// and the remaining ASCII control characters are denied because an unencoded
/// newline in a path reaching a logging or transport layer is response-splitting
/// and request-smuggling surface — and because admitting some control characters
/// and not others invites exactly the enumeration gap the decode-once design
/// exists to avoid.
const fn denied_byte(byte: u8, allow_slash: bool) -> bool {
    match byte {
        b'?' | b'#' | b'\\' => true,
        b'/' => !allow_slash,
        0x00..=0x1F | 0x7F => true,
        _ => false,
    }
}

/// ASCII-case-insensitive substring search.
///
/// `%2E` and `%2e` decode identically, so a case-SENSITIVE `contains` is a bypass
/// (RESEARCH Finding 5b).
fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let (haystack, needle) = (haystack.as_bytes(), needle.as_bytes());
    needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Step 3 — the declared `pattern`, routed through the CACHED input validator.
///
/// `cached_input_validator` and not `compile_input_2020_12`: the uncached entry
/// point would compile a fresh regex on every placeholder check on every request,
/// which is both a per-request cost and a direct amplifier of the `ReDoS` residual
/// T-128-10 accepts. It also means a placeholder `pattern` and an `inputSchema`
/// `pattern` resolve `\s` through the identical engine, which matters because
/// `\s` is not a single rule in this engine (RESEARCH Finding 1f).
///
/// A pattern that does not compile is itself a refusal; the cache stores that
/// failure too, so a broken declared pattern is not recompiled per request either.
fn declared_pattern_check(
    param: &str,
    value: &str,
    pattern: &str,
) -> Result<(), PlaceholderRefusal> {
    let schema = serde_json::json!({ "type": "string", "pattern": pattern });
    match cached_input_validator(&schema, None) {
        Ok(validator) => {
            if validator.is_valid(&Value::String(value.to_owned())) {
                Ok(())
            } else {
                Err(refusal(param, "pattern", format!("must match {pattern}")))
            }
        },
        // The compile DETAIL is author-supplied schema text, but this refusal is
        // client-facing, so it stays detail-free — matching `validate_input`'s
        // treatment of a non-compiling declared `inputSchema`.
        Err(_) => Err(refusal(
            param,
            "pattern",
            "has a declared pattern that is not a valid regular expression".to_string(),
        )),
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

    // ===================================================================
    // Plan 02 Task 1 — every `ValidationErrorKind` arm this phase can emit.
    // ===================================================================

    /// A one-property object schema, so each arm can be exercised in isolation.
    fn one_prop(property: &Value) -> Value {
        json!({
            "type": "object",
            "properties": { "p": property },
            "additionalProperties": false,
        })
    }

    /// Refuse `args` against `schema` and render the client-facing message.
    fn refusal_for(schema: &Value, args: &Value, declared: &[&str]) -> String {
        let violations =
            validate_input(schema, Some(args), None).expect_err("the pair must be refused");
        render_refusal(&violations, declared)
    }

    #[test]
    fn schema_validation_max_length_refusal_names_the_limit_not_the_value() {
        let schema = one_prop(&json!({ "type": "string", "maxLength": 8 }));
        let value = "x".repeat(5000);
        let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
        assert!(msg.contains('8'), "must carry the declared limit: {msg}");
        assert!(
            !msg.contains(&"x".repeat(10)),
            "must not echo the rejected value: {msg}"
        );
        assert!(!msg.contains(&value), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_enum_refusal_lists_declared_options_only() {
        let schema = one_prop(&json!({ "enum": ["exact", "words"] }));
        let value = "Jane Doe DOB 1970-01-01";
        let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
        assert!(msg.contains("exact"), "must list declared options: {msg}");
        assert!(msg.contains("words"), "must list declared options: {msg}");
        assert!(!msg.contains(value), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_pattern_refusal_names_the_declared_pattern() {
        let schema = one_prop(&json!({ "type": "string", "pattern": "^C[0-9]+$" }));
        let value = "Jane Doe DOB 1970-01-01";
        let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
        assert!(
            msg.contains("^C[0-9]+$"),
            "must name the declared pattern: {msg}"
        );
        assert!(!msg.contains(value), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_bound_refusals_name_the_declared_bound_only() {
        let cases: &[(Value, Value, &str)] = &[
            (
                json!({ "type": "integer", "maximum": 10 }),
                json!(4242),
                "10",
            ),
            (json!({ "type": "integer", "minimum": 10 }), json!(-7), "10"),
            (
                json!({ "type": "string", "minLength": 4 }),
                json!("ab"),
                "4",
            ),
            (
                json!({ "type": "array", "maxItems": 2 }),
                json!(["a", "b", "c"]),
                "2",
            ),
        ];
        for (property, value, declared_bound) in cases {
            let schema = one_prop(property);
            let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
            assert!(
                msg.contains(declared_bound),
                "must name the declared bound {declared_bound}: {msg}"
            );
            let rendered_value = format!("{value}");
            assert!(
                !msg.contains(&rendered_value),
                "must not echo the rejected value: {msg}"
            );
        }
    }

    #[test]
    fn schema_validation_type_refusal_names_the_declared_type_only() {
        let schema = one_prop(&json!({ "type": "integer" }));
        let msg = refusal_for(&schema, &json!({ "p": "Jane Doe" }), &["p"]);
        assert!(
            msg.contains("integer"),
            "must name the declared type: {msg}"
        );
        assert!(!msg.contains("Jane Doe"), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_format_refusal_names_the_declared_format_only() {
        // Q1: inputs compile through a format-ASSERTING builder, so `format` is a
        // kind this phase actively produces and must render value-free.
        let schema = one_prop(&json!({ "type": "string", "format": "uri" }));
        let value = "!!!not-a-uri!!!";
        let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
        assert!(msg.contains("uri"), "must name the declared format: {msg}");
        assert!(!msg.contains(value), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_required_refusal_names_the_declared_property() {
        let schema = two_param_schema();
        let msg = refusal_for(
            &schema,
            &json!({ "version": "2026AA" }),
            &["cui", "version"],
        );
        assert!(
            msg.contains("cui"),
            "must name the declared required property: {msg}"
        );
    }

    #[test]
    fn schema_validation_additional_properties_refusal_carries_a_count_and_no_keys() {
        let schema = two_param_schema();
        let msg = refusal_for(
            &schema,
            &json!({ "cui": "C1", "apiKey": "secret", "Jane Doe DOB 1970-01-01": "x" }),
            &["cui", "version"],
        );
        assert!(msg.contains('2'), "must carry the count 2: {msg}");
        assert!(msg.contains("cui"), "must name the allow-list: {msg}");
        assert!(msg.contains("version"), "must name the allow-list: {msg}");
        assert!(!msg.contains("apiKey"), "must not echo a key: {msg}");
        assert!(
            !msg.contains("Jane Doe"),
            "must not echo a caller key: {msg}"
        );
    }

    #[test]
    fn schema_validation_empty_rejected_value_refusal_is_value_free() {
        let schema = one_prop(&json!({ "type": "string", "minLength": 3 }));
        let msg = refusal_for(&schema, &json!({ "p": "" }), &["p"]);
        assert!(msg.contains('3'), "must name the declared minimum: {msg}");
        assert!(
            msg.contains("at least"),
            "must state the declared expectation: {msg}"
        );
    }

    #[test]
    fn schema_validation_astral_and_combining_value_never_reaches_the_refusal() {
        // `maxLength` counts code points (RESEARCH Finding 1d): 4 emoji + a
        // decomposed `é` is over a cap of 3.
        let schema = one_prop(&json!({ "type": "string", "maxLength": 3 }));
        // Deliberately ALL non-ASCII, so a per-code-point absence assertion is
        // meaningful: an ASCII code point from the value would also occur in the
        // declared expectation's own English prose, which would make the
        // assertion vacuous rather than strict.
        let value = "\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{00E9}\u{0301}";
        let msg = refusal_for(&schema, &json!({ "p": value }), &["p"]);
        for ch in value.chars() {
            assert!(
                !msg.contains(ch),
                "code point {ch:?} from the rejected value reached the refusal: {msg}"
            );
        }
        assert!(!msg.contains(value), "must not echo the value: {msg}");
    }

    #[test]
    fn schema_validation_array_form_items_schema_refuses_with_a_schema_keyword() {
        // Draft-07 array-form `items` does not compile under the 2020-12 pin
        // (RESEARCH Finding 1h): a drifted declaration must REFUSE, value-free.
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": { "tags": { "type": "array", "items": [ { "type": "string" } ] } },
        });
        let violations = validate_input(&schema, Some(&json!({ "tags": ["a"] })), None)
            .expect_err("a non-compiling declared schema must refuse every call");
        assert_eq!(violations.len(), 1, "exactly one schema violation");
        assert_eq!(violations[0].keyword, "schema");
        let msg = render_refusal(&violations, &["tags"]);
        assert!(
            !msg.contains("type"),
            "the compile detail is logged server-side, never rendered: {msg}"
        );
    }

    #[test]
    fn schema_validation_pattern_properties_key_never_reaches_the_refusal() {
        // `patternProperties` names the CALLER's key in the instance pointer, so a
        // raw `instance_path()` copy would leak it (Codex HIGH / T-128-08a).
        let schema = json!({
            "type": "object",
            "patternProperties": { "^.*$": { "type": "integer" } },
        });
        let args = json!({ "Jane Doe DOB 1970-01-01": "x" });
        let violations =
            validate_input(&schema, Some(&args), None).expect_err("a string is not an integer");
        // The POINTER itself must be sanitized, not merely suppressed by
        // `render_refusal`: `InputViolation` is public and its `pointer` reaches
        // logs, execution records and third-party renderers.
        assert!(
            !violations[0].pointer.contains("Jane Doe"),
            "a caller-chosen property name reached `InputViolation::pointer`: {}",
            violations[0].pointer
        );
        let msg = render_refusal(&violations, &["p"]);
        assert!(
            !msg.contains("Jane Doe"),
            "a caller-chosen property name reached the refusal: {msg}"
        );
        assert!(
            !msg.contains("1970-01-01"),
            "a caller-chosen property name reached the refusal: {msg}"
        );
    }

    #[test]
    fn schema_validation_additional_properties_subschema_key_never_reaches_the_refusal() {
        // `additionalProperties` as a SUBSCHEMA (not `false`) also puts the
        // caller's key in the pointer.
        let schema = json!({
            "type": "object",
            "properties": { "cui": { "type": "string" } },
            "additionalProperties": { "type": "integer" },
        });
        let args = json!({ "Jane Doe DOB 1970-01-01": "x" });
        let violations =
            validate_input(&schema, Some(&args), None).expect_err("a string is not an integer");
        assert!(
            !violations[0].pointer.contains("Jane Doe"),
            "a caller-chosen property name reached `InputViolation::pointer`: {}",
            violations[0].pointer
        );
        let msg = render_refusal(&violations, &["cui"]);
        assert!(
            !msg.contains("Jane Doe"),
            "a caller-chosen property name reached the refusal: {msg}"
        );
    }

    #[test]
    fn schema_validation_declared_property_pointer_survives_the_projection() {
        // The redaction must not be a blanket suppression: a DECLARED name is
        // still what makes a refusal actionable for the legitimate caller.
        let schema = one_prop(&json!({ "type": "string", "maxLength": 2 }));
        let violations = validate_input(&schema, Some(&json!({ "p": "abc" })), None)
            .expect_err("over the declared maxLength");
        assert_eq!(violations[0].pointer, "/p", "declared names stay verbatim");
    }

    #[test]
    fn schema_validation_declared_array_index_pointer_survives_the_projection() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": { "type": "array", "items": { "type": "string", "maxLength": 2 } },
            },
        });
        let violations = validate_input(&schema, Some(&json!({ "tags": ["ok", "toolong"] })), None)
            .expect_err("the second item is over the declared maxLength");
        assert_eq!(
            violations[0].pointer, "/tags/1",
            "a base-10 array index carries no caller-chosen text"
        );
    }

    #[test]
    fn schema_validation_check_input_schema_compiles_names_the_offending_property_path() {
        let schema = json!({
            "type": "object",
            "properties": { "bad": { "type": "string", "pattern": "^[A-Z" } },
        });
        let violation = check_input_schema_compiles(&schema)
            .expect_err("a nested non-compiling `pattern` must be caught at config time");
        assert_eq!(violation.keyword, "schema");
        assert!(
            violation.pointer.contains("bad"),
            "the pointer must name the offending property path: {}",
            violation.pointer
        );
    }

    #[test]
    fn schema_validation_check_input_schema_compiles_accepts_a_well_formed_schema() {
        assert!(check_input_schema_compiles(&two_param_schema()).is_ok());
    }

    // ===================================================================
    // Plan 02 Task 2 — the D4 character floor and the composed-path check.
    // ===================================================================

    /// The CR-01 probe payloads, verbatim from `128-CHANGE-REQUEST.md` and from
    /// RESEARCH Finding 5b's measured `^.*$` probe.
    const CR01_PAYLOADS: &[&str] = &[
        "2026AA?string=x",
        "current/../../search/current",
        "current/../../search/current?string=x",
        "%2e%2e%2f",
        "%3Fstring%3Dx",
        "a%00b",
        "a#frag",
    ];

    #[test]
    fn placeholder_accepts_the_cap_and_refuses_one_more() {
        let rules = PlaceholderRules::default();
        let at_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH);
        assert!(validate_path_placeholder("v", &at_cap, &rules).is_ok());

        let over_cap = "a".repeat(PLACEHOLDER_MAX_LENGTH + 1);
        let refusal = validate_path_placeholder("v", &over_cap, &rules)
            .expect_err("one code point over the cap must be refused");
        assert_eq!(refusal.rule, "maxLength");
    }

    #[test]
    fn placeholder_counts_code_points_not_bytes() {
        // A 256-emoji value is 1024 bytes and exactly at the cap.
        let rules = PlaceholderRules::default();
        let at_cap = "\u{1F600}".repeat(PLACEHOLDER_MAX_LENGTH);
        assert_eq!(at_cap.len(), PLACEHOLDER_MAX_LENGTH * 4, "1024 bytes");
        assert!(validate_path_placeholder("v", &at_cap, &rules).is_ok());
    }

    #[test]
    fn placeholder_refuses_an_empty_value() {
        let refusal = validate_path_placeholder("v", "", &PlaceholderRules::default())
            .expect_err("an empty path segment changes the URL shape");
        assert_eq!(refusal.rule, "nonEmpty");
    }

    #[test]
    fn placeholder_refuses_a_bare_slash_unless_the_parameter_opts_in() {
        assert!(
            validate_path_placeholder("v", "/", &PlaceholderRules::default()).is_err(),
            "`/` is denied by default"
        );
        let opted_in = PlaceholderRules::default().allowing_slash(true);
        assert!(
            validate_path_placeholder("v", "/", &opted_in).is_ok(),
            "D-11: `/` is liftable by per-parameter CONFIG opt-in"
        );
    }

    #[test]
    fn placeholder_refuses_the_parent_directory_sequence_with_and_without_slash_opt_in() {
        for allow_slash in [false, true] {
            let rules = PlaceholderRules::default().allowing_slash(allow_slash);
            for value in ["..", "a/../b", "%2e%2e", "%2E%2E"] {
                assert!(
                    validate_path_placeholder("v", value, &rules).is_err(),
                    "traversal has NO escape even with allow_slash={allow_slash}: {value}"
                );
            }
        }
    }

    #[test]
    fn placeholder_refuses_every_cr01_payload() {
        let rules = PlaceholderRules::default();
        for payload in CR01_PAYLOADS {
            assert!(
                validate_path_placeholder("version", payload, &rules).is_err(),
                "CR-01 payload must be refused: {payload}"
            );
        }
    }

    #[test]
    fn placeholder_percent_scan_is_case_insensitive_on_hex() {
        let rules = PlaceholderRules::default();
        for value in ["%2e%2e%2f", "%2E%2E%2F", "%2f", "%2F", "%3f", "%3F"] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "the hex scan must be ASCII-case-insensitive: {value}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_double_encoded_percent() {
        let rules = PlaceholderRules::default();
        for value in ["%252e", "%252E", "%25", "a%2525b"] {
            let refusal = validate_path_placeholder("v", value, &rules)
                .expect_err("`%25` must be refused outright, in any hex case");
            assert_eq!(
                refusal.rule, "percentEncoding",
                "refusing `%25` up front is what BOUNDS the decode to one pass: {value}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_a_malformed_percent_escape() {
        let rules = PlaceholderRules::default();
        for value in ["%zz", "%2", "%", "a%g0b"] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "a malformed escape has no legitimate use in a path value: {value}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_mixed_literal_and_encoded_traversal() {
        // THE decode-once row. An enumerated literal-plus-encoded denylist admits
        // both of these; decoding once does not.
        let rules = PlaceholderRules::default();
        for value in [".%2E", "%2E.", ".%2e", "%2e."] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "step 1 was implemented as an enumeration, not as decode-once: {value}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_backslash_in_literal_and_encoded_form() {
        let rules = PlaceholderRules::default();
        for value in ["\\", "..\\", "a\\b", "%5c", "%5C"] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "reverse proxies normalize `\\` toward `/`: {value}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_carriage_return_and_line_feed_in_both_forms() {
        let rules = PlaceholderRules::default();
        for value in ["a\rb", "a\nb", "a%0db", "a%0Db", "a%0ab", "a%0Ab", "a\tb"] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "a control character in a path is response-splitting surface: {value:?}"
            );
        }
    }

    #[test]
    fn placeholder_refuses_a_bare_single_dot() {
        let rules = PlaceholderRules::default();
        for value in [".", "%2e", "%2E"] {
            assert!(
                validate_path_placeholder("v", value, &rules).is_err(),
                "two adjacent single dots compose to traversal: {value}"
            );
        }
    }

    #[test]
    fn placeholder_floor_runs_before_a_permissive_declared_pattern() {
        // D-10, empirically justified: `^.*$` accepts every CR-01 payload
        // (RESEARCH Finding 5b), so pattern-supersedes-floor would be a no-op.
        let rules = PlaceholderRules::default().with_pattern(Some("^.*$"));
        for payload in CR01_PAYLOADS {
            assert!(
                validate_path_placeholder("version", payload, &rules).is_err(),
                "a permissive declared pattern must not relax the floor: {payload}"
            );
        }
    }

    #[test]
    fn placeholder_declared_pattern_narrows() {
        let rules = PlaceholderRules::default().with_pattern(Some("^C[0-9]+$"));
        assert!(validate_path_placeholder("cui", "C0018787", &rules).is_ok());
        let refusal = validate_path_placeholder("cui", "ABC", &rules)
            .expect_err("the declared pattern must narrow");
        assert_eq!(refusal.rule, "pattern");
    }

    #[test]
    fn placeholder_refuses_a_declared_pattern_that_does_not_compile() {
        let rules = PlaceholderRules::default().with_pattern(Some("^[A-Z"));
        let refusal = validate_path_placeholder("cui", "C1", &rules)
            .expect_err("a non-compiling declared pattern must refuse, never pass everything");
        assert_eq!(refusal.rule, "pattern");
        let rendered = refusal.to_string();
        assert!(rendered.contains("cui"), "must name the param: {rendered}");
        assert!(
            !rendered.contains("C1"),
            "must not echo the value: {rendered}"
        );
    }

    #[test]
    fn placeholder_declared_max_length_never_widens_the_module_cap() {
        let rules = PlaceholderRules::default().with_max_length(Some(512));
        let value = "a".repeat(300);
        let refusal = validate_path_placeholder("v", &value, &rules)
            .expect_err("the module constant is the HARD cap");
        assert_eq!(refusal.rule, "maxLength");
    }

    #[test]
    fn placeholder_declared_max_length_narrows_further() {
        let rules = PlaceholderRules::default().with_max_length(Some(8));
        assert!(validate_path_placeholder("v", "12345678", &rules).is_ok());
        let refusal = validate_path_placeholder("v", "123456789", &rules)
            .expect_err("the declared length narrows the cap");
        assert_eq!(refusal.rule, "maxLength");
    }

    #[test]
    fn placeholder_refusals_name_the_param_and_never_echo_the_value() {
        let rules = PlaceholderRules::default().with_max_length(Some(4));
        let values = [
            "",
            "2026AA?string=x",
            "current/../../search/current",
            "%252e",
            "%zz",
            "\\",
            ".",
            "aaaaaaaaaa",
        ];
        for value in values {
            let refusal = validate_path_placeholder("version", value, &rules)
                .expect_err("every one of these is refused");
            let rendered = refusal.to_string();
            assert!(
                rendered.contains("version"),
                "must name the declared param: {rendered}"
            );
            if !value.is_empty() {
                assert!(
                    !rendered.contains(value),
                    "refusal echoed the rejected value {value:?}: {rendered}"
                );
            }
        }
    }

    #[test]
    fn placeholder_default_rules_are_floored_and_capped_never_permissive() {
        let rules = PlaceholderRules::default();
        assert!(rules.declared_pattern.is_none());
        assert!(rules.declared_max_length.is_none());
        assert!(!rules.allow_slash);
        // `Clone` is required by plan 08's owned-rules construction.
        let cloned = rules.clone();
        for payload in CR01_PAYLOADS {
            assert!(
                validate_path_placeholder("v", payload, &cloned).is_err(),
                "`PlaceholderRules::default()` must be floored and capped: {payload}"
            );
        }
    }

    /// The memo must not grow without bound. `fuzz_placeholder_pattern_redos` drove
    /// the process to libFuzzer's 2 GB RSS limit through exactly this map, in CI
    /// and locally, because every distinct generated pattern was stored forever.
    ///
    /// Run on a LOCAL map with a tiny cap: the process-global cache would be left
    /// full for every other test in the binary.
    #[test]
    fn cache_stops_storing_new_schemas_once_full() {
        let compile = |max: u64| {
            compile_input_2020_12(&serde_json::json!({ "type": "string", "maxLength": max }))
                .map(Arc::new)
                .map_err(|_| Arc::<str>::from("unexpected compile failure"))
        };
        let mut map = HashMap::new();
        for n in 0..3_u64 {
            remember_bounded(&mut map, 3, format!("k{n}"), compile(n)).expect("compiles");
        }
        assert_eq!(map.len(), 3, "below the bound every schema is stored");

        // Beyond the cap: still returns a working validator, stores nothing.
        let overflow = remember_bounded(&mut map, 3, "k-new".to_string(), compile(99))
            .expect("an overflow schema still compiles and validates");
        assert_eq!(map.len(), 3, "a full cache must not grow");
        assert!(
            !map.contains_key("k-new"),
            "the overflow entry must not be stored"
        );
        assert!(overflow.is_valid(&Value::String("x".repeat(99))));
        assert!(!overflow.is_valid(&Value::String("x".repeat(100))));

        // Entries stored before the cap keep hitting: the SAME Arc comes back.
        let stored = map["k1"].clone().expect("stored");
        let again = remember_bounded(&mut map, 3, "k1".to_string(), compile(1)).expect("compiles");
        assert!(
            Arc::ptr_eq(&stored, &again),
            "a key already present must resolve to the stored entry, not a fresh compile"
        );
    }

    /// The bound must not turn a compile FAILURE into a success or drop it: a
    /// broken declared pattern past the cap is still refused.
    #[test]
    fn cache_bound_still_reports_compile_failures() {
        let mut map = HashMap::new();
        map.insert("only".to_string(), Err(Arc::<str>::from("stored")));
        let failed = remember_bounded(
            &mut map,
            1,
            "other".to_string(),
            Err(Arc::<str>::from("does not compile")),
        );
        assert!(failed.is_err(), "an overflow failure must still be an Err");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn placeholder_declared_pattern_compiles_once_per_pattern() {
        // Observe the MEMO, not the wall clock: a second lookup of the same schema
        // text must hand back the SAME `Arc`, which is only true on a cache hit.
        // This is what makes T-128-10's memoization claim checkable.
        let compiling = json!({ "type": "string", "pattern": "^C[0-9]+$" });
        let first = cached_input_validator(&compiling, None).expect("compiles");
        let second = cached_input_validator(&compiling, None).expect("compiles");
        assert!(
            Arc::ptr_eq(&first, &second),
            "the second lookup recompiled instead of hitting the cache"
        );

        let broken = json!({ "type": "string", "pattern": "^[A-Z" });
        let first_err = cached_input_validator(&broken, None).expect_err("does not compile");
        let second_err = cached_input_validator(&broken, None).expect_err("does not compile");
        assert!(
            Arc::ptr_eq(&first_err, &second_err),
            "a compile FAILURE must be cached too, or a broken declared pattern is \
             recompiled on every request"
        );

        let rules = PlaceholderRules::default().with_pattern(Some("^[A-Z"));
        let one = validate_path_placeholder("cui", "C1", &rules)
            .expect_err("a broken pattern refuses")
            .to_string();
        let two = validate_path_placeholder("cui", "C1", &rules)
            .expect_err("a broken pattern refuses")
            .to_string();
        assert_eq!(one, two, "refusals must be byte-identical across calls");
    }

    #[test]
    fn placeholder_operates_on_the_rendered_string_not_a_json_value() {
        // A non-string argument is rendered to a string by the CALLER
        // (`render_scalar`) and the RENDERED string is what the floor and the cap
        // see — which is why this primitive takes `&str` and never a `Value`.
        let rules = PlaceholderRules::default();
        for rendered in ["42", "true", "null", "1.5"] {
            assert!(
                validate_path_placeholder("v", rendered, &rules).is_ok(),
                "a rendered scalar is an ordinary value: {rendered}"
            );
        }
    }

    #[test]
    fn placeholder_is_a_pure_function_safe_under_concurrency() {
        // D4 concurrency edge: no shared mutable state of its own, so two Code
        // Mode calls on one executor cannot interleave placeholder state. The
        // validator cache behind the declared-pattern step is the only shared
        // state and it is an `RwLock<HashMap<…>>`, read-probed before any write.
        let handles: Vec<_> = (0..8)
            .map(|worker| {
                std::thread::spawn(move || {
                    let rules = PlaceholderRules::default().with_pattern(Some("^C[0-9]+$"));
                    let good = format!("C{worker}");
                    assert!(validate_path_placeholder("cui", &good, &rules).is_ok());
                    assert!(validate_path_placeholder("cui", "../etc", &rules).is_err());
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("no worker panicked");
        }
    }

    #[test]
    fn resolved_path_refuses_a_segment_over_the_cap_composed_from_two_passing_values() {
        // The adjacency proof: 180 + 200 code points compose to a 380-code-point
        // segment, and NEITHER part alone exceeds the cap.
        let rules = PlaceholderRules::default();
        let a = "a".repeat(180);
        let b = "b".repeat(200);
        assert!(validate_path_placeholder("a", &a, &rules).is_ok());
        assert!(validate_path_placeholder("b", &b, &rules).is_ok());

        let composed = format!("/search/{a}{b}");
        let refusal = validate_resolved_path(&composed)
            .expect_err("the composed segment is over the cap even though each value passed");
        assert_eq!(refusal.rule, "segmentMaxLength");
    }

    #[test]
    fn resolved_path_refuses_a_traversal_segment_composed_from_two_single_dots() {
        // `.` + `.` composes to `..` — traversal from two values neither of which
        // contains the sequence. Closed at BOTH layers.
        let rules = PlaceholderRules::default();
        assert!(
            validate_path_placeholder("a", ".", &rules).is_err(),
            "the single-dot floor closes this at the value layer"
        );
        let refusal = validate_resolved_path("/x/..")
            .expect_err("the composed segment is the parent-directory sequence");
        assert_eq!(refusal.rule, "pathSegment");
    }

    #[test]
    fn resolved_path_refuses_a_residual_placeholder_brace() {
        for path in ["/x/{unsubstituted}", "/x/}", "/x/{"] {
            assert!(
                validate_resolved_path(path).is_err(),
                "an unsubstituted placeholder must never reach the wire: {path}"
            );
        }
    }

    #[test]
    fn resolved_path_refuses_an_empty_interior_segment_and_accepts_a_clean_path() {
        assert!(validate_resolved_path("/a//b").is_err(), "empty interior");
        assert!(validate_resolved_path("/a/b/").is_err(), "empty trailing");
        assert!(validate_resolved_path("/a/b").is_ok(), "a clean path");
        assert!(validate_resolved_path("/content/current/CUI/C0018787").is_ok());
    }

    /// Phase 128 CR-01 — the absolute root is a legal path, and exempting it must
    /// not re-admit either of the two empty-segment shapes that are refused BY
    /// DECISION.
    ///
    /// The accept half and the two refuse halves are one test on purpose: the
    /// accept alone is satisfiable by relaxing the non-leading-empty rule, which
    /// would silently re-open `/a/b/` — the composed form of `/search/{v}` with an
    /// empty `v`, i.e. the exact case that rule was written to close.
    ///
    /// Fails on removal: delete the `decoded == b"/"` exemption in
    /// `check_resolved_segments` and the first assertion reports
    /// `param 'path segment' must not be empty`.
    #[test]
    fn resolved_path_accepts_the_absolute_root_and_still_refuses_doubled_and_trailing() {
        // ACCEPT: the root, in the two spellings that decode to a bare `/`.
        for path in ["/", "%2F"] {
            assert!(
                validate_resolved_path(path).is_ok(),
                "the absolute root is the shortest legal absolute path, and a `GET /` \
                 operation must be callable: {path:?} -> {:?}",
                validate_resolved_path(path)
            );
        }

        // STILL REFUSED, by decision — a doubled separator changes the endpoint
        // shape, and a trailing `/` is what an empty tail placeholder composes to.
        for path in ["//", "///", "/a/b/", "/a//b", "/a/", "/search/"] {
            let refusal =
                validate_resolved_path(path).expect_err(&format!("must stay refused: {path:?}"));
            assert_eq!(
                refusal.rule, "pathSegment",
                "the empty-segment refusal keeps its rule token: {path:?}"
            );
        }

        // The exemption is an EQUALITY test on the whole decoded path, so it cannot
        // be reached by a path that merely starts or ends with the root.
        assert!(validate_resolved_path("/a").is_ok());
        assert!(
            validate_resolved_path("").is_err(),
            "an empty composed path is not the root and has no endpoint"
        );
    }

    #[test]
    fn resolved_path_refuses_encoded_traversal_after_decode_once() {
        for path in ["/a/%2e%2e/b", "/a/%2E%2E/b", "/a/%252e%252e/b", "/a/%zz/b"] {
            assert!(
                validate_resolved_path(path).is_err(),
                "decode-once must reach this: {path}"
            );
        }
    }

    #[test]
    fn resolved_path_refuses_query_and_fragment_markers_and_control_bytes() {
        for path in ["/a?b=c", "/a#frag", "/a%3Fb", "/a\\b", "/a%00b", "/a\nb"] {
            assert!(
                validate_resolved_path(path).is_err(),
                "must be refused anywhere in the composed path: {path:?}"
            );
        }
    }

    #[test]
    fn resolved_path_refuses_a_single_dot_segment() {
        assert!(validate_resolved_path("/a/./b").is_err());
        assert!(validate_resolved_path("/a/%2e/b").is_err());
    }

    #[test]
    fn resolved_path_refusal_names_the_position_not_a_caller_supplied_name() {
        let refusal = validate_resolved_path("/x/../secret").expect_err("traversal");
        let rendered = refusal.to_string();
        assert!(
            rendered.contains("path segment"),
            "the composed check is param-agnostic: {rendered}"
        );
        assert!(
            !rendered.contains("secret"),
            "must not echo the composed path: {rendered}"
        );
    }

    // `validate_resolved_target`'s guarantees are exercised end-to-end by
    // `pmcp_server_toolkit::http::client::query_separator` and by
    // `pmcp_code_mode::executor::query_separator` (11 rows each). These rows live
    // HERE as well, beside the rule, so `cargo test -p pmcp` alone cannot be green
    // against an edit to this security floor — the reason the narrowing was moved
    // into this module in the first place.

    #[test]
    fn resolved_target_accepts_exactly_one_author_written_separator() {
        assert!(validate_resolved_target("/a/b").is_ok());
        assert!(validate_resolved_target("/a?b=c").is_ok());
        assert!(validate_resolved_target("/a?b=c&d=e").is_ok());
    }

    #[test]
    fn resolved_target_refuses_a_second_separator_and_an_empty_query() {
        // Only the FIRST `?` is split off, so the query portion faces the
        // unmodified rule, which denies `?`.
        assert!(validate_resolved_target("/a?b=c?d=e").is_err());
        // A dangling `?` is a trailing separator, the same class as a trailing `/`.
        assert!(validate_resolved_target("/a?").is_err());
        // And the path portion keeps every rule it had.
        assert!(validate_resolved_target("/a/../secret?b=c").is_err());
        assert!(validate_resolved_target("/a?b=%00").is_err());
        assert!(validate_resolved_target("/a#frag?b=c").is_err());
    }

    #[test]
    fn resolved_target_applies_path_segment_structure_to_the_query_too() {
        // DOCUMENTED CONSEQUENCE, asserted so it cannot change silently: the query
        // half faces `validate_resolved_path` whole, including the rules about path
        // SHAPE (`//`, a trailing `/`, an empty segment). So a query value carrying
        // a URL or a trailing slash is refused. That is inherited conservatism, not
        // a new rule — both former call-site copies did exactly this — but it is the
        // most surprising thing about the narrowing and the one an operator hits.
        assert!(validate_resolved_target("/a?redirect=https://example.com").is_err());
        assert!(validate_resolved_target("/a?b=//x").is_err());
        assert!(validate_resolved_target("/a?b=1&c=x/").is_err());
    }
}
