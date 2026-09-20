//! Emit-time validation of `structuredContent` against a declared `outputSchema`.
//!
//! When a tool declares an `outputSchema` and the dispatcher bridges the
//! handler's value into `structuredContent`, this module checks the value
//! against the schema and logs a WARNING (never an error result) on mismatch —
//! catching schema drift in dev/CI without adding a production failure mode.
//!
//! The module compiles unconditionally so dispatcher call sites stay plain
//! one-liners; the `validation` feature gate lives INSIDE
//! [`warn_on_schema_mismatch`], which is a no-op without the feature.
//! Compiled validators are cached per `(era, schema)` pair, so steady-state
//! cost per call is one lookup plus a short-circuit `is_valid` check — the
//! error-message pass runs only on actual mismatch.
//!
//! # Era
//!
//! Validation is era-branched (Phase 115, SCHM-01):
//!
//! - **v1** (`Era::V1`, and any request with no resolved protocol context)
//!   keeps today's behaviour EXACTLY: the dialect is auto-detected from the
//!   document's own `$schema` declaration (D-01). Nothing about v1 validation
//!   changed when the v2 pin landed. See [`compile_for_era`] — the single
//!   `jsonschema` auto-detect entry point in this module lives on that arm and
//!   nowhere else.
//! - **v2** (`Era::V2`) compiles every `outputSchema` as JSON Schema Draft
//!   2020-12, which MCP 2026-07-28 pins. On v2 the pin wins in every SCHEMA
//!   POSITION — the root, plus every node reachable without descending into a
//!   `const` / `enum` / `default` / `examples` payload, where the VALUES of a
//!   `properties` / `patternProperties` / `$defs` / `definitions` /
//!   `dependentSchemas` / `dependencies` map — the SIX keywords the JSON Schema
//!   meta-schemas themselves define as maps from author-chosen names to
//!   subschemas — are schema positions REGARDLESS OF THE NAME they
//!   are filed under. Every dialect declaration in such a position is
//!   rewritten: the root one, and the one on every embedded schema resource
//!   below it. A declared legacy `$schema` there is ignored — neither honoured
//!   nor rejected — and the ignoring is announced through a `tracing::warn!`
//!   (D-02). The scope is stated this narrowly because two WIDER statements of
//!   it shipped here and were both false: this bullet previously read "on v2
//!   the pin wins UNCONDITIONALLY … across the whole DOCUMENT", which ignored
//!   the data-only exception (a `$schema` inside a `const` / `enum` / `default`
//!   / `examples` payload is instance DATA and is deliberately never rewritten,
//!   so no whole-document total can ever hold) AND the name-position rule (a
//!   `$defs` entry an author named `default` was visited by neither walker, so
//!   its legacy declaration survived the pin — `115-VERIFICATION.md`, closed by
//!   115-14). The list of containers was then found INCOMPLETE — it omitted
//!   `dependencies`, so the same collision was reachable one keyword over
//!   (`115-REVIEW.md` CR-01, closed by 115-16). See
//!   [`normalize_schema_dialect`] for why "ignored" has to mean
//!   "rewritten" rather than "compiled as-is", for the measured bypass that
//!   rewriting only the root left open (closed by 115-12), for the measured
//!   bypass that a position-blind walk left open (closed by 115-14), and for
//!   the ONE residual that remains open by decision: an author-invented
//!   container is still name-dependent.
//!
//! A consequence worth stating: some draft-07 constructs cannot be expressed
//! under 2020-12 at all — `exclusiveMinimum: true` and array-form `items` are
//! the measured cases — and those schemas fail to COMPILE on v2, surfacing
//! through the existing "declared outputSchema is not a valid JSON Schema: …"
//! warning. That is the loud half of D-02, and it is deliberate: a schema the
//! server cannot evaluate should say so rather than silently pass everything.
//!
//! This module stays **warn-only on BOTH eras**. Escalating v2 to a hard error
//! result is deliberately NOT done here: it would be a new production failure
//! mode, and the diagnostic value of the warning is the whole point of the
//! module. See 115-RESEARCH § Finding 6.

// Why: same tug-of-war as `task_dispatch` — rustc's `unreachable_pub` demands
// pub(crate) on items in a crate-internal module, while clippy's
// `redundant_pub_crate` flags that as redundant inside a pub(crate) module.
// rustc wins; silence the clippy side.
#![allow(clippy::redundant_pub_crate)]

use crate::types::protocol::Era;
use serde_json::Value;

/// The JSON Schema Draft 2020-12 meta-schema URI — the dialect MCP 2026-07-28
/// pins for `outputSchema`.
#[cfg(feature = "validation")]
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Warn (via `tracing`) when `value` does not conform to the tool's declared
/// `outputSchema`. Never fails the call. No-op unless the `validation`
/// feature is enabled.
///
/// `era` is the ALREADY-RESOLVED protocol era of the request being served
/// (Phase 112's one ingress answer), threaded through so the v2 Draft 2020-12
/// pin applies only to v2 requests. `None` — no resolved protocol context —
/// conservatively means [`Era::V1`], matching
/// [`crate::types::protocol::protocol_era`]'s unknown-to-`V1` rule.
// Called from the native dispatch path only; on wasm32 that path is not
// compiled, so this has no caller THERE and nowhere else.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn warn_on_schema_mismatch(tool: &str, schema: &Value, value: &Value, era: Option<Era>) {
    #[cfg(feature = "validation")]
    {
        if !tracing::enabled!(tracing::Level::WARN) {
            return;
        }
        if let Some(mismatch) = schema_mismatch(schema, value, era) {
            tracing::warn!(
                tool,
                "structuredContent does not conform to the declared outputSchema: {mismatch}"
            );
        }
    }
    #[cfg(not(feature = "validation"))]
    let _ = (tool, schema, value, era);
}

/// Check `value` against `schema`, returning a human-readable description of
/// every violation, or `None` when the value conforms.
///
/// A schema that is itself invalid (fails to compile as JSON Schema) also
/// yields `Some` — a drifted declaration is exactly what this check exists
/// to surface.
///
/// `era` selects the dialect policy: `Some(Era::V2)` pins Draft 2020-12,
/// everything else (including `None`) keeps v1's `$schema` auto-detect.
#[cfg(feature = "validation")]
pub(crate) fn schema_mismatch(schema: &Value, value: &Value, era: Option<Era>) -> Option<String> {
    match cached_validator(era, schema) {
        Ok(validator) => {
            // Fast path: the conforming case is the common one and `is_valid`
            // short-circuits; build messages only on actual mismatch.
            if validator.is_valid(value) {
                return None;
            }
            let errors: Vec<String> = validator
                .iter_errors(value)
                .map(|e| format!("{} (at {})", e, e.instance_path()))
                .collect();
            Some(errors.join("; "))
        },
        Err(e) => Some(format!(
            "declared outputSchema is not a valid JSON Schema: {e}"
        )),
    }
}

/// Keywords whose VALUE is instance data rather than a subschema, and which the
/// dialect walk therefore must not descend into.
///
/// A `$schema` string inside one of these is DATA — part of the instance a
/// `const` pins, an `enum` alternative, a `default` a client may substitute, or
/// an `examples` entry. Rewriting it would change which instances conform, which
/// is a semantic corruption of the author's schema, not a normalization. Every
/// other keyword's value is either a subschema, a map of subschemas or an array
/// of subschemas, all of which a dialect declaration may legally appear inside.
#[cfg(feature = "validation")]
const DATA_ONLY_KEYWORDS: &[&str] = &["const", "enum", "default", "examples"];

/// Keywords whose VALUE is a MAP from AUTHOR-CHOSEN NAMES to subschemas.
///
/// The keys of these maps are NAMES, never keywords: an author may call a
/// `$defs` entry `default`, or declare an instance property named `examples`.
/// [`DATA_ONLY_KEYWORDS`] must therefore never be tested against them — doing so
/// is a category error, and it is the bypass `115-VERIFICATION.md` measured:
/// `$defs.default` carrying an `$id` plus a legacy `$schema` was visited by
/// neither [`first_legacy_dialect`] nor [`pin_dialect_in_place`], so the
/// declaration survived the v2 pin, resolved an EMPTY vocabulary set on that
/// embedded resource and produced the accept-everything sub-validator the pin
/// exists to prevent — `verdicts=(Conforms, Conforms)`, `rewritten=false`,
/// against the control `$defs.Inner` -> `(Conforms, Violates)`, `rewritten=true`.
///
/// The same distinction already existed two hundred lines away, in
/// `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`'s `is_neutral_subschema`, which
/// does descend into `$defs` / `properties` values BY NAME. It was simply never
/// applied on this side.
///
/// # The sixth entry, and why the list is a DERIVATION rather than a patch
///
/// `dependencies` — draft-04 / -06 / -07 and 2019-09's own
/// map-from-INSTANCE-PROPERTY-NAME-to-subschema keyword, still declared
/// (deprecated) by the 2020-12 meta-schema — was MISSING from this list until
/// 115-16, which is `115-REVIEW.md` CR-01. Its omission made normalization
/// name-dependent through that position: measured through this crate's own
/// `fuzz_support` seam, `dependencies.Inner` -> `rewritten=true` against
/// `dependencies.default` -> `rewritten=false`, so the legacy declaration
/// survived and `compile_2020_12`'s `tracing::warn!` — the only D-02 diagnostic
/// an author gets — silently did not fire.
///
/// This module's own `fuzz_support_tests` had recorded the very measurement that
/// makes the keyword's VALUES live schema positions: `D-115-03-C`, that
/// `jsonschema` 0.49.2 STILL HONOURS `dependencies` under the 2020-12 pin. The
/// two statements contradicted each other for two plans.
///
/// Because this was the third round in which the same allow-list shape was found
/// incomplete, the entry was established by ENUMERATION rather than by patching
/// the one case a reviewer found. See
/// [`v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`]
/// for the fence, and 115-16's SUMMARY for the derivation table.
///
/// # The derivation, so the next reader can re-run it instead of guessing
///
/// This list is the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09 /
/// 2020-12 meta-schema documents `jsonschema` 0.49.2 ships offline, of the
/// keywords each meta-schema's own `.properties` map binds to an OBJECT-typed
/// schema carrying an `additionalProperties` that REFERENCES THE META-SCHEMA
/// ITSELF — `{"$ref":"#"}` (draft-04/06/07), `{"$recursiveRef":"#"}` (2019-09),
/// `{"$dynamicRef":"#meta"}` (2020-12), or an `anyOf` with such a branch
/// (`dependencies`). That shape is precisely "a map whose keys are author-chosen
/// and whose values are subschemas".
///
/// Two keywords match the object-with-`additionalProperties` shape and are
/// REJECTED by the self-reference criterion, and naming them is what makes the
/// enumeration checkable rather than asserted:
///
/// - `$vocabulary` — `additionalProperties` is `{"type": "boolean"}`; its values
///   are enablement flags, not subschemas;
/// - `dependentRequired` — `additionalProperties` is a `stringArray` `$ref`; its
///   values are lists of property names, not subschemas.
///
/// The union is exactly these six. Re-derive with (guard the `object` type test
/// FIRST — `draft7.json` binds `default` and `const` to booleans, and an
/// unguarded `.value.type` makes `jq` exit 5 with the error on stderr and
/// nothing on stdout, which is that criterion's own pass condition):
///
/// ```text
/// jq -r '(.properties // {}) | to_entries[]
///        | select((.value|type)=="object")
///        | select(.value|has("additionalProperties"))
///        | "\(.key)\t\(.value.additionalProperties|tojson)"' <metaschema.json>
/// ```
///
/// Note the SCOPE BOUNDARY this list does not cross: an author-invented
/// container (`components`, a vendor extension) appears in no meta-schema and is
/// therefore not in the derived set, so a resource filed there under a colliding
/// name stays name-dependent — measured `components.default -> rewritten=false`.
/// That residual is stated on [`normalize_schema_dialect`] and booked in
/// `D-115-AD`; it is accepted, not overlooked.
///
/// The entry is ordered LAST deliberately: the restated mirrors in
/// `tests/property_tests.rs` and `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`,
/// and 115-19's drift gate, compare the three copies as ORDERED slices.
#[cfg(feature = "validation")]
const SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
    "dependencies", // draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME (D-115-03-C)
];

/// The first dialect declaration in `node` that is not already
/// [`DRAFT_2020_12`], searched root-first, or `None` when the document declares
/// no legacy dialect anywhere.
///
/// This is the DETECTOR half of the normalization; [`pin_dialect_in_place`] is
/// the REWRITER half, and the two implement the identical traversal rule stated
/// on [`normalize_schema_dialect`]. They must agree: a detector that sees a
/// declaration the rewriter cannot reach yields a `Cow::Owned` that still
/// carries a legacy declaration, which `compile_2020_12` then announces as
/// "the declaration is ignored" while having ignored nothing.
#[cfg(feature = "validation")]
fn first_legacy_dialect(node: &Value) -> Option<&str> {
    match node {
        Value::Object(map) => {
            if let Some(declared) = map.get("$schema").and_then(Value::as_str) {
                if declared != DRAFT_2020_12 {
                    return Some(declared);
                }
            }
            map.iter().find_map(|(member_key, member_value)| {
                first_legacy_dialect_in_member(member_key, member_value)
            })
        },
        Value::Array(items) => items.iter().find_map(first_legacy_dialect),
        _ => None,
    }
}

/// The MEMBER-level dispatch of the detector's walk: the three-way decision on
/// one object member's key, mutually recursive with [`first_legacy_dialect`].
///
/// Split out rather than inlined, for the same reason [`compile_for_era`] is
/// split out of [`cached_validator`]: CI's `pmat quality-gate --checks
/// complexity` is PR-blocking, and measured with pmat 3.15.0 the inline form put
/// the REWRITER at cognitive 24 against a threshold of 23. Both halves are split
/// so the two remain visibly mirror-image; a reader comparing them should be
/// comparing like with like. Do not inline either back.
///
/// The plan for 115-14 specified this signature without lifetimes; it does not
/// compile that way (two input references, one borrowed output — elision is
/// ambiguous), so the output lifetime is tied explicitly to `member_value`.
#[cfg(feature = "validation")]
fn first_legacy_dialect_in_member<'a>(
    member_key: &str,
    member_value: &'a Value,
) -> Option<&'a str> {
    match member_value {
        // NAME position: the keys of THIS map are author-chosen names, so they
        // are never keyword-filtered. Descend into every value.
        Value::Object(named_subschemas) if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) => {
            named_subschemas.values().find_map(first_legacy_dialect)
        },
        // KEYWORD position: a data-only payload is not descended into. A
        // `$defs` / `properties` / … member whose value is NOT an object is a
        // malformed document and falls through to the ordinary walk below, so no
        // coverage is lost relative to the position-blind version.
        _ if DATA_ONLY_KEYWORDS.contains(&member_key) => None,
        _ => first_legacy_dialect(member_value),
    }
}

/// Overwrite EVERY dialect declaration in `node` with [`DRAFT_2020_12`], in
/// place.
///
/// The REWRITER half of the normalization; see [`first_legacy_dialect`] for the
/// detector it must agree with, and [`normalize_schema_dialect`] for the single
/// traversal rule both implement.
#[cfg(feature = "validation")]
fn pin_dialect_in_place(node: &mut Value) {
    match node {
        Value::Object(map) => {
            // A declaration is a STRING-valued `$schema`; anything else with
            // that key is data (see the traversal rule) and is left alone.
            if map.get("$schema").is_some_and(Value::is_string) {
                map.insert(
                    "$schema".to_string(),
                    Value::String(DRAFT_2020_12.to_string()),
                );
            }
            for (member_key, member_value) in map.iter_mut() {
                pin_dialect_in_member(member_key, member_value);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(pin_dialect_in_place),
        _ => {},
    }
}

/// The MEMBER-level dispatch of the rewriter's walk — the mirror image of
/// [`first_legacy_dialect_in_member`], mutually recursive with
/// [`pin_dialect_in_place`].
///
/// Split out for the measured reason recorded on its detector twin: inline, this
/// dispatch put `pin_dialect_in_place` at cognitive 24 against
/// `pmat quality-gate`'s threshold of 23 (measured with pmat 3.15.0; the same
/// gate reported 0 violations at the commit before 115-14, so the extraction is
/// this change's own cost and not an inherited one). Do not inline it back.
#[cfg(feature = "validation")]
fn pin_dialect_in_member(member_key: &str, member_value: &mut Value) {
    if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
        // NAME position: descend into every value of the map, and never
        // keyword-filter the map's own keys. A non-object value here is a
        // malformed document and gets the ordinary walk.
        match member_value {
            Value::Object(named_subschemas) => {
                named_subschemas.values_mut().for_each(pin_dialect_in_place);
            },
            malformed => pin_dialect_in_place(malformed),
        }
    } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
        pin_dialect_in_place(member_value);
    }
}

/// Rewrite EVERY dialect declaration in the document — at the root and at any
/// depth — to [`DRAFT_2020_12`], leaving every other byte alone.
///
/// Pure and idempotent. Returns `Cow::Borrowed` when no `$schema` anywhere in
/// the document names a dialect other than 2020-12 (the common case allocates
/// nothing, and the borrow makes "this function did not copy the document"
/// visible in the TYPE rather than only in a comment), and `Cow::Owned` of a
/// clone with every such `$schema` OVERWRITTEN otherwise. Overwritten, not
/// deleted, so the compiled document STATES the dialect it was evaluated under
/// — which also matches `outputSchema`'s own declared type in the 2026-07-28
/// schema, `{ "$schema"?: string, [key: string]: unknown }`.
///
/// # The traversal rule, stated once
///
/// [`first_legacy_dialect`] and [`pin_dialect_in_place`] implement exactly this,
/// and a disagreement between them is a defect:
///
/// 1. At an object node, the key `$schema` is a DIALECT DECLARATION **only when
///    its value is a `Value::String`**. A non-string value is not a declaration:
///    that is how a real declaration is told apart from a `properties` map entry
///    for an instance property literally named `$schema`, whose value is a
///    subschema (an object or a boolean) and never a string.
/// 2. A member whose key is one of the six [`SUBSCHEMA_MAP_KEYWORDS`] —
///    `properties`, `patternProperties`, `$defs`, `definitions`,
///    `dependentSchemas`, `dependencies` — has a VALUE that is a map from
///    AUTHOR-CHOSEN NAMES to subschemas. Recurse into every one of those values,
///    and NEVER test that map's own keys against rule 3: they are names, not
///    keywords. (A member with one of these keys whose value is not an object is
///    a malformed document and takes rule 3's ordinary path, so nothing is
///    skipped.) The six are DERIVED from the pinned meta-schemas, not assumed —
///    see [`SUBSCHEMA_MAP_KEYWORDS`], where the criterion and the two rejected
///    keywords are recorded.
/// 3. Otherwise, recurse into every member value EXCEPT the values of the
///    [`DATA_ONLY_KEYWORDS`] — `const`, `enum`, `default` and `examples` — which
///    carry instance data rather than subschemas.
/// 4. At an array node, recurse into every element. Scalars terminate.
///
/// Rules 2 and 3 are a POSITION distinction, and the whole of 115-14 is that
/// distinction: the same four words are a data-only KEYWORD in rule 3 and an
/// ordinary NAME in rule 2, and applying rule 3 to a rule-2 map is a category
/// error that produced a measured validation bypass.
///
/// The postcondition is therefore checkable, and is what replaces the `expect`
/// this function used to carry: after an `Owned` return,
/// `first_legacy_dialect(&owned)` is `None`. That is what guarantees an `Owned`
/// really was rewritten rather than silently handed back unchanged — a
/// non-object root now falls out of the walk naturally instead of needing a
/// panic to fence it.
///
/// **Read that postcondition for exactly what it is: a detector/rewriter
/// AGREEMENT check, not an independence check.** Both halves implement the rule
/// above, so a defect IN the rule satisfies it VACUOUSLY — measured, not
/// argued: against the position-blind body, `$defs.default` came back
/// `Cow::Borrowed` with nothing rewritten and
/// `first_legacy_dialect(&normalized) == None` PASSED, because the blind
/// detector agreed with the blind rewriter that there was nothing there.
/// `normalize_schema_dialect_changes_only_dollar_schema_keys` asserts the
/// postcondition over every fixed case and 115-13 re-states it in the fuzz
/// target, but a differently-TYPED walk restating the same RULE catches only a
/// disagreement. The independent instrument for the rule itself is the
/// rename-invariance fence 115-15 adds: renaming a `$defs` key must not change
/// the normalized document apart from that key.
///
/// # Why the rewrite is NOT cosmetic
///
/// It is tempting to read this as tidying. It is not: it is the whole safety
/// property of the v2 pin. `jsonschema`'s `with_draft` / `draft202012::new`
/// sets the KEYWORD SET, but a document that declares a legacy meta-schema
/// still resolves its VOCABULARIES from that declaration, and under 2020-12
/// vocabulary semantics a draft-04/06/07 declaration yields an EMPTY
/// vocabulary set — producing a validator that accepts EVERY instance.
///
/// Measured across `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2:
/// compiling a draft-07-declared document under the 2020-12 pin WITHOUT this
/// normalization silently drops `type`, `required`, `properties`, `enum`,
/// `$ref`, `minimum` and `additionalProperties`. That is a validation BYPASS,
/// not a "may validate differently" surprise — and `draft202012::meta::is_valid`
/// returns `true` for such a document, so there is no library-side detector.
/// The `$schema` key has to be inspected here.
///
/// # Why the walk is recursive, and what "measured" used to mean here
///
/// This function rewrote ONLY the root key until 115-12, on the strength of a
/// research measurement that a nested `$schema` — specifically one inside
/// `properties.a` with no `$id` — does not trigger the bypass. That measurement
/// is TRUE and is still fenced by `normalization_cases()`; the sentence
/// generalizing it to "a nested declaration does not trigger the bypass" was
/// not, and is the root cause of the whole gap. Under 2020-12 a `$schema` is
/// legal at the root of any EMBEDDED SCHEMA RESOURCE — any subschema that also
/// carries `$id` — and `jsonschema` 0.49.2 honours it there. Re-measured twice
/// on this tree (code review CR-01 and `115-VERIFICATION.md`) through
/// `fuzz_support::validate_bytes`, with `$defs.Inner` carrying `$id` +
/// `$schema: draft-07` + `type: integer`, `$ref`'d from `properties.n`, against
/// the instance `{"n": "NOT-AN-INTEGER"}`:
///
/// | Case | (v1, v2) before 115-12 |
/// |---|---|
/// | embedded legacy resource | `(Conforms, Conforms)` — `type` silently dropped |
/// | control, no nested `$schema` | `(Violates, Violates)` — enforcement works |
/// | root draft-07 + embedded | `(Violates, Conforms)` — **v2 weaker than v1** |
///
/// The third row is the regression direction SCHM-01 exists to forbid, so do
/// not narrow this walk back to the root. `v2_pin_still_enforces_an_embedded_legacy_resource`
/// is the fence, and it has been observed to fail against the root-only body.
///
/// # What the walk is a SUPERSET of, and what it is NOT
///
/// This paragraph has now been WRONG TWICE, and the reason it is spelled out
/// this narrowly is that both wider forms shipped and were both falsified. It
/// first read "on v2 the pin wins UNCONDITIONALLY … across the whole DOCUMENT"
/// (false: the data-only exception, and the name-position rule — closed by
/// 115-14). It then read "Rewriting every declaration is deliberately a SUPERSET
/// of what `jsonschema` honours", unqualified — also false, because the walk was
/// name-dependent at `dependencies` positions (`115-REVIEW.md` CR-01, closed by
/// 115-16) and REMAINS name-dependent at author-invented containers. The scope
/// the code actually has is:
///
/// - **Within the six SPEC-DEFINED subschema-map containers** — the
///   [`SUBSCHEMA_MAP_KEYWORDS`] — **and every ordinary member position**, the
///   walk IS a superset of what `jsonschema` honours: a nested declaration on a
///   subschema with no `$id` is inert, and is rewritten anyway. That is strictly
///   safer, and it is what makes the postcondition above statable without a
///   per-node `$id` analysis.
/// - **It is NOT a superset over an author-invented container.** A resource
///   filed under a key that no JSON Schema vocabulary defines as a subschema map
///   — `components`, or any vendor extension — under a NAME colliding with a
///   [`DATA_ONLY_KEYWORDS`] entry is still SKIPPED. Measured on this tree:
///   `components.default` -> `rewritten=false` (`115-VERIFICATION.md`,
///   `115-REVIEW.md` CR-01).
/// - **Why that residual is accepted rather than closed.** Closing it needs the
///   INVERSE walk — descend only into positions the core/applicator vocabularies
///   DEFINE as subschemas, treating everything else as opaque — which 115-14
///   deliberately declined because it would REDUCE what is normalized, including
///   under vendor containers that really do hold subschemas. Ledger `D-115-AD`
///   carries that decision; 115-19 re-books the residual.
///
/// # Why the walk is position-aware
///
/// 115-12 made the walk recursive but POSITION-BLIND: it tested
/// [`DATA_ONLY_KEYWORDS`] against every object key uniformly. A `$defs` key is
/// an AUTHOR-CHOSEN NAME, so filtering it against a keyword list is a category
/// error — and a reachable one. Measured on this tree through
/// `fuzz_support::{validate_bytes, normalize_bytes}` with two documents
/// differing ONLY in the NAME of the `$defs` entry, each holding an
/// `$id`-bearing embedded resource with `$schema: draft-07` + `type: integer`,
/// against the instance `{"n": "NOT-AN-INTEGER"}`:
///
/// | Document | normalization | `(v1, v2)` |
/// |---|---|---|
/// | `$defs.Inner` (control) | rewritten (`Cow::Owned`) | `(Conforms, Violates)` |
/// | `$defs.default` (renamed) | byte-identical, nothing rewritten | `(Conforms, Conforms)` |
///
/// The second row is the vacuous sub-validator this module exists to prevent,
/// reached by renaming a definition. The sentence that shipped alongside it —
/// "on v2 the pin wins UNCONDITIONALLY … across the whole DOCUMENT" — was
/// FALSE as shipped, in two independent ways at once: the data-only exception
/// (a `$schema` inside a `const` / `enum` / `default` / `examples` payload is
/// instance DATA and is never rewritten) and the name-position rule this
/// section states. That is why the scope is now spelled out rather than
/// asserted. `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`
/// is the fence, and it was observed to fail against the position-blind body
/// before this rule landed.
///
/// The maps whose VALUES are schema positions are the SIX
/// [`SUBSCHEMA_MAP_KEYWORDS`]: `properties`, `patternProperties`, `$defs`,
/// `definitions`, `dependentSchemas` and `dependencies`. The sixth was MISSING
/// until 115-16, which made the identical collision reachable one keyword over
/// (`115-REVIEW.md` CR-01) — measured `dependencies.Inner -> rewritten=true`
/// against `dependencies.default -> rewritten=false`, with BOTH names giving the
/// same `(Violates, Violates)` verdict pair on `jsonschema` 0.49.2, which is why
/// that gap could only be fenced STRUCTURALLY.
/// `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map` is
/// that fence; it carries its OWN container literal, because a fence
/// parameterised by the list whose incompleteness is the defect cannot fire on
/// that defect.
///
/// The rule changes behaviour on exactly ONE other, malformed shape, and it is
/// worth naming:
/// `{"<container>": {"$schema": "http://json-schema.org/draft-07/schema#"}}`
/// for each of the six containers — the shape was first measured on
/// `properties`, and 115-16 extended it to `dependencies` by adding the sixth
/// entry, so it is stated over the set rather than re-enumerated in a way that
/// will rot. The old walk descended into the container MAP as though the map
/// were itself a schema, saw a string-valued `$schema` there and rewrote it.
/// Under the position rule that key is an author-chosen NAME bound to a
/// non-schema value, and it is left alone — which is correct, and is precisely
/// why the two RESTATED copies of this rule (`tests/property_tests.rs` and
/// `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`) MUST be updated in lockstep:
/// until they are, such an input makes their surviving-declaration SCAN report
/// a FALSE positive. 115-15 did that for the five-keyword list; **115-17 and
/// 115-18 own it for the sixth (`dependencies`), and until they land the two
/// mirrors are stale by construction — do not run the fuzz target to judge a
/// tree in that window.**
#[cfg(feature = "validation")]
fn normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value> {
    use std::borrow::Cow;

    // No legacy declaration anywhere — including the undeclared document, which
    // is already 2020-12 (`Draft::default() == Draft202012`, and the MCP spec
    // says the same). Nothing to rewrite, nothing to allocate.
    if first_legacy_dialect(schema).is_none() {
        return Cow::Borrowed(schema);
    }
    let mut pinned = schema.clone();
    pin_dialect_in_place(&mut pinned);
    Cow::Owned(pinned)
}

/// Compile `schema` under an explicitly-pinned Draft 2020-12 (the v2 dialect),
/// normalizing the document's `$schema` declaration first.
///
/// Warns when the normalization actually rewrote something: that warning is the
/// only signal a tool author gets that their declared dialect was ignored, and
/// it is the diagnostic D-02 leaves available.
#[cfg(feature = "validation")]
fn compile_2020_12(
    schema: &Value,
) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    let normalized = normalize_schema_dialect(schema);
    if matches!(normalized, std::borrow::Cow::Owned(_)) {
        // Sourced from the DETECTOR, not from the root key: the declaration
        // that triggered the rewrite may sit on an embedded schema resource,
        // and reading `schema["$schema"]` would then report `<unknown>` — or,
        // worse, a misleading `2020-12` — for the very case the warning exists
        // to explain. This runs only on the rewrite path, which already clones.
        let declared = first_legacy_dialect(schema).unwrap_or("<unknown>");
        tracing::warn!(
            declared,
            "outputSchema declares JSON Schema {declared} at the document root or on an embedded \
             schema resource; MCP 2026-07-28 pins Draft 2020-12, so every such declaration is \
             ignored and the schema is validated as 2020-12"
        );
    }
    jsonschema::draft202012::new(&normalized)
}

/// Compile a validator for `schema` under `era`'s dialect policy, WITHOUT
/// touching the cache.
///
/// This is deliberately split out of [`cached_validator`] rather than inlined
/// into it: the process-global cache is unbounded by design (bounded in
/// practice by the number of distinct DECLARED schemas), so a property or fuzz
/// target that compiles arbitrary generated schemas needs a path that does not
/// grow it without limit. Keep this function separate — do not inline it back.
///
/// Splitting normalization, compilation and caching across three functions is
/// also what keeps each of them under CI's cognitive-complexity cap.
#[cfg(feature = "validation")]
fn compile_for_era(era: Era, schema: &Value) -> Result<jsonschema::Validator, std::sync::Arc<str>> {
    match era {
        // D-01 freezes v1: this arm is today's behaviour VERBATIM — the dialect
        // is auto-detected from the document's own `$schema` declaration.
        Era::V1 => jsonschema::validator_for(schema),
        Era::V2 => compile_2020_12(schema),
    }
    .map_err(|e| std::sync::Arc::from(e.to_string().as_str()))
}

/// Fetch (or compile and cache) the validator for `(era, schema)`.
///
/// Keyed by `(era, canonical schema text)`. The schema-text half is correct
/// across servers sharing the process (unlike a tool-name key) and bounded by
/// the number of distinct declared schemas; the era half is REQUIRED because
/// D-01 makes the same text compile to two DIFFERENT validators — keying on
/// text alone would be first-writer-wins for the process lifetime, silently
/// serving one era's validator to the other. Compilation errors are cached
/// too, as the error string.
///
/// `None` resolves to [`Era::V1`], the same conservative fallback
/// [`crate::types::protocol::protocol_era`] applies to unknown versions: a
/// request with no resolved protocol context must never reach v2 behaviour.
#[cfg(feature = "validation")]
fn cached_validator(
    era: Option<Era>,
    schema: &Value,
) -> Result<std::sync::Arc<jsonschema::Validator>, std::sync::Arc<str>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    type Cache = Mutex<HashMap<(Era, String), Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let resolved_era = era.unwrap_or(Era::V1);
    let key = (resolved_era, schema.to_string());
    let cache = CACHE.get_or_init(Cache::default);
    // Why: a poisoned mutex here only means another thread panicked while
    // inserting; the map itself is still usable — recover rather than
    // propagate a panic out of a warn-only diagnostics path.
    let mut map = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key)
        .or_insert_with(|| compile_for_era(resolved_era, schema).map(Arc::new))
        .clone()
}

/// Minimal seam for `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`.
///
/// # ⚠️ Not stable API
///
/// This module exists only behind `feature = "fuzzing"`, which is in neither
/// `default` nor `full` (`Cargo.toml:220`), so `cargo public-api` never sees it
/// on the shipped surface. Do not depend on it. The second gate,
/// `feature = "validation"`, is what compiles the validator this seam drives —
/// without it there is nothing here to reach.
///
/// The shape is verbatim the one `crate::server::request_state::fuzz_support`
/// established, so the crate has ONE convention for a fuzz seam rather than
/// three.
#[cfg(all(feature = "fuzzing", feature = "validation"))]
pub mod fuzz_support {
    use super::{compile_for_era, normalize_schema_dialect};
    use crate::types::protocol::Era;
    use serde_json::Value;

    /// The shipped [`super::DATA_ONLY_KEYWORDS`], readable from `tests/` and
    /// `fuzz/`.
    ///
    /// # Why this exists
    ///
    /// This rule is RESTATED, by hand, in three places: here in `src/`, in
    /// `tests/property_tests.rs` and in
    /// `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`. Nothing kept the three in
    /// sync (`115-REVIEW.md` WR-01), and BOTH drift directions are silent:
    ///
    /// - the crate list gains an entry and the mirrors do not — the property and
    ///   fuzz fences then hold the OLD rule against the NEW behaviour and become
    ///   FALSE-POSITIVE generators, crashing on correct code. That is not
    ///   hypothetical: it is the window 115-14 opened and 115-16 re-opened for
    ///   `dependencies`, closed by 115-17 / 115-18;
    /// - an entry is removed from all three in lockstep — coverage vanishes with
    ///   ZERO test failures. `patternProperties` and `dependentSchemas` sat in
    ///   that state from 115-14 to 115-16.
    ///
    /// Published here so the mirrors can be GATED against the shipped values
    /// instead of hand-maintained a fourth time. The module is `fuzzing`-gated
    /// and `fuzzing` is in neither `default` nor `full`, so this adds NOTHING to
    /// the shipped public API — `cargo public-api` never sees it.
    ///
    /// The private originals are deliberately left private and the walkers keep
    /// reading them, so this re-export cannot drift from what ships.
    pub const DATA_ONLY_KEYWORDS: &[&str] = super::DATA_ONLY_KEYWORDS;

    /// The shipped [`super::SUBSCHEMA_MAP_KEYWORDS`], readable from `tests/` and
    /// `fuzz/`.
    ///
    /// See [`DATA_ONLY_KEYWORDS`] for why both lists are published here rather
    /// than restated a fourth time. This is the list whose incompleteness was
    /// `115-REVIEW.md` CR-01, so a mirror that can drift from it is exactly the
    /// hazard worth gating.
    pub const SUBSCHEMA_MAP_KEYWORDS: &[&str] = super::SUBSCHEMA_MAP_KEYWORDS;

    /// The three distinguishable outcomes of checking ONE instance against ONE
    /// schema under ONE era.
    ///
    /// Three states, not two, and the third is load-bearing. A `(bool, bool)`
    /// seam built from `schema_mismatch(..).is_none()` collapses *the schema
    /// did not compile* and *the schema compiled and the instance violates it*
    /// into the same `false`, so a caller cannot skip the compile-failure case
    /// — and comparing eras across a compile failure compares COMPILATION, not
    /// semantics. [`InvalidSchema`](Self::InvalidSchema) is what makes that
    /// skip expressible.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SchemaVerdict {
        /// The schema compiled under this era's dialect and the instance
        /// conforms to it.
        Conforms,
        /// The schema compiled under this era's dialect and the instance does
        /// NOT conform to it.
        Violates,
        /// The schema itself failed to compile under this era's dialect, so no
        /// instance-level verdict exists. Under the v2 pin this is also how the
        /// SEP-2106 external-`$ref` refusal surfaces.
        InvalidSchema,
    }

    /// Drive emit-time validation with arbitrary schema and instance bytes,
    /// returning the `(v1, v2)` verdict pair.
    ///
    /// Returns `None` when EITHER slice fails to parse as JSON — the caller
    /// then has no schema/instance pair to hold any semantic invariant over,
    /// only the totality one.
    ///
    /// # Why this uses the UNCACHED compile path
    ///
    /// Every verdict is built through [`compile_for_era`], never through
    /// `cached_validator`. The cache is a process-global
    /// `OnceLock<Mutex<HashMap<(Era, String), _>>>` that is unbounded by design
    /// — bounded in practice only by the number of distinct DECLARED schemas.
    /// A fuzzer generates a fresh schema text on nearly every iteration, so
    /// routing this seam through the cache would grow that map without limit
    /// for the whole process lifetime, turning a correctness fuzzer into a
    /// memory-exhaustion one. Do NOT "optimize" this onto `cached_validator`;
    /// `compile_for_era` was split out of it by 115-03 precisely so this path
    /// exists.
    #[must_use]
    pub fn validate_bytes(
        schema_bytes: &[u8],
        instance_bytes: &[u8],
    ) -> Option<(SchemaVerdict, SchemaVerdict)> {
        let schema: Value = serde_json::from_slice(schema_bytes).ok()?;
        let instance: Value = serde_json::from_slice(instance_bytes).ok()?;
        Some((
            verdict(Era::V1, &schema, &instance),
            verdict(Era::V2, &schema, &instance),
        ))
    }

    /// One era's verdict, with the compile failure kept distinct from the
    /// instance-level failure.
    fn verdict(era: Era, schema: &Value, instance: &Value) -> SchemaVerdict {
        match compile_for_era(era, schema) {
            Ok(validator) => {
                if validator.is_valid(instance) {
                    SchemaVerdict::Conforms
                } else {
                    SchemaVerdict::Violates
                }
            },
            Err(_) => SchemaVerdict::InvalidSchema,
        }
    }

    /// Parse `schema_bytes` and return `(input, normalized_once,
    /// normalized_twice)`.
    ///
    /// Handing back all three documents lets a caller hold idempotence
    /// (`once == twice`) and surgical scope (`once` and `input` differ only at
    /// `$schema` keys, at any depth) DIRECTLY, rather than inferring them from
    /// downstream validation behaviour — which cannot distinguish "the
    /// normalizer dropped a sibling keyword" from "the instance happened to
    /// conform anyway".
    ///
    /// Returns `None` when the bytes do not parse as JSON.
    #[must_use]
    pub fn normalize_bytes(schema_bytes: &[u8]) -> Option<(Value, Value, Value)> {
        let input: Value = serde_json::from_slice(schema_bytes).ok()?;
        let once = normalize_schema_dialect(&input).into_owned();
        let twice = normalize_schema_dialect(&once).into_owned();
        Some((input, once, twice))
    }
}

/// The seam cannot rot silently: if `fuzz_support` is removed, its verdict
/// discriminants shift, or its skip condition collapses back into a two-state
/// boolean, these fail under `--features "full fuzzing"`.
///
/// Named `fuzz_support_tests` (not `tests`) so `cargo nextest run -E
/// 'test(/output_validation::fuzz_support/)'` selects exactly these five and
/// nothing else — `test(/fuzz_support/)` alone also matches
/// `server::request_state::tests::fuzz_support_seam_rejects_garbage`, which
/// predates this module.
#[cfg(all(test, feature = "fuzzing", feature = "validation"))]
mod fuzz_support_tests {
    use super::fuzz_support::{normalize_bytes, validate_bytes, SchemaVerdict};

    /// Unparseable bytes on EITHER side yield `None` — the target then has only
    /// the totality invariant to hold, which is the whole point of the `Option`.
    #[test]
    fn fuzz_support_returns_none_for_unparseable_input() {
        assert_eq!(
            validate_bytes(b"{not json", b"{}"),
            None,
            "an unparseable SCHEMA must produce no verdict pair"
        );
        assert_eq!(
            validate_bytes(b"{}", b"{not json"),
            None,
            "an unparseable INSTANCE must produce no verdict pair"
        );
        assert_eq!(
            normalize_bytes(b"\xff\xfe\xfd"),
            None,
            "normalize_bytes must refuse non-JSON rather than panicking"
        );
    }

    /// The ordinary two-state case, on both eras: an object schema and a scalar
    /// instance.
    #[test]
    fn fuzz_support_reports_violates_for_a_scalar_against_an_object_schema() {
        assert_eq!(
            validate_bytes(br#"{"type":"object"}"#, b"42"),
            Some((SchemaVerdict::Violates, SchemaVerdict::Violates)),
            "an object schema must report a scalar instance on both eras"
        );
    }

    /// SEP-2106's refusal expressed as the THIRD state. A `(bool, bool)` seam
    /// could not tell this apart from a violating instance, which is why the
    /// verdict is three-state.
    #[test]
    fn fuzz_support_reports_invalid_schema_for_an_external_ref() {
        assert_eq!(
            validate_bytes(br#"{"$ref":"https://example.com/x.json"}"#, b"{}"),
            Some((SchemaVerdict::InvalidSchema, SchemaVerdict::InvalidSchema)),
            "an external $ref must be a COMPILE failure — never a fetch, and never \
             indistinguishable from a violating instance"
        );
    }

    /// The CONCRETE case that makes a cross-dialect MONOTONICITY claim false.
    ///
    /// `contentEncoding` is an ASSERTION in draft-07 and only an ANNOTATION
    /// from 2019-09 onwards, so a non-base64 string VIOLATES under v1's
    /// auto-detect and CONFORMS under the v2 2020-12 pin — `v2 == Conforms &&
    /// v1 == Violates`. Asserting it here documents, at the seam itself, why
    /// `fuzz_schema_draft_pin` must not claim "v2 rejects everything v1
    /// rejects".
    ///
    /// **This is `contentEncoding`, not `dependencies`.** 115-03 measured on
    /// `jsonschema` 0.49.2 that the crate still honours `dependencies` under
    /// the 2020-12 pin, so both eras return the SAME verdict for it and it is
    /// not a divergence case at all (D-115-03-C). The converse direction is
    /// reachable too — `$ref` siblings are ignored in draft-07 and honoured
    /// under 2020-12 — so the era relation is non-monotonic in BOTH directions.
    ///
    /// **That same measurement is why `dependencies` belongs in
    /// [`super::SUBSCHEMA_MAP_KEYWORDS`].** A keyword the pinned library still
    /// HONOURS is a keyword whose values are live schema positions, so a legacy
    /// declaration on an `$id`-bearing resource filed there is a real hazard and
    /// must be normalized. For two plans this comment recorded the measurement
    /// while the list omitted the keyword — the two statements contradicted each
    /// other, which is `115-REVIEW.md` CR-01 and `115-VERIFICATION.md`'s
    /// key-link warning. 115-16 closed it; they now reinforce each other.
    #[test]
    fn fuzz_support_reports_the_divergent_content_encoding_case_asymmetrically() {
        let schema = br#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "string",
            "contentEncoding": "base64",
            "description": "fuzz-seam-divergence"
        }"#;
        assert_eq!(
            validate_bytes(schema, br#""!!!not-base64!!!""#),
            Some((SchemaVerdict::Violates, SchemaVerdict::Conforms)),
            "the eras must genuinely disagree here; if they agree, either the v1 arm stopped \
             auto-detecting draft-07 or the v2 pin stopped applying, and the fuzz target's \
             dialect-NEUTRAL restriction has lost its reason to exist"
        );
    }

    /// Normalizing twice equals normalizing once, and the rewrite is surgical —
    /// the fixed-example half of what the fuzz target holds over arbitrary
    /// generated schemas.
    #[test]
    fn fuzz_support_normalize_bytes_is_idempotent() {
        let (input, once, twice) = normalize_bytes(
            br#"{"$schema":"http://json-schema.org/draft-07/schema#","type":"object"}"#,
        )
        .expect("the literal above is valid JSON");

        assert_eq!(once, twice, "normalization must be idempotent");
        assert_eq!(
            once.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "the root $schema must be OVERWRITTEN with the 2020-12 URI, not deleted"
        );

        let mut before = input;
        let mut after = once;
        for document in [&mut before, &mut after] {
            if let Some(object) = document.as_object_mut() {
                object.remove("$schema");
            }
        }
        assert_eq!(
            before, after,
            "normalization touched a key other than a $schema key"
        );
    }
}

#[cfg(all(test, feature = "validation"))]
mod tests {
    use super::*;
    use serde_json::json;

    fn person_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name"]
        })
    }

    #[test]
    fn conforming_value_yields_none() {
        let value = json!({ "name": "Ada", "age": 36 });
        assert_eq!(schema_mismatch(&person_schema(), &value, None), None);
    }

    #[test]
    fn non_conforming_value_yields_message() {
        let value = json!({ "age": "not-a-number" });
        let mismatch = schema_mismatch(&person_schema(), &value, None)
            .expect("missing required field + wrong type must be reported");
        assert!(
            mismatch.contains("name"),
            "message names the missing required field: {mismatch}"
        );
    }

    #[test]
    fn invalid_schema_yields_message() {
        let bad_schema = json!({ "type": 42 });
        let mismatch = schema_mismatch(&bad_schema, &json!({}), None)
            .expect("an uncompilable schema must be reported, not ignored");
        assert!(
            mismatch.contains("outputSchema"),
            "message says the schema itself is at fault: {mismatch}"
        );
    }

    #[test]
    fn repeated_checks_reuse_the_cached_validator() {
        // Same schema, many values: exercises the cache path both ways.
        let schema = person_schema();
        for i in 0..8 {
            let ok = json!({ "name": format!("p{i}") });
            assert_eq!(schema_mismatch(&schema, &ok, None), None);
            assert!(schema_mismatch(&schema, &json!({ "age": i }), None).is_some());
        }
    }

    #[test]
    fn warn_never_panics_on_mismatch() {
        warn_on_schema_mismatch("demo_tool", &person_schema(), &json!({ "age": 1 }), None);
    }

    // =======================================================================
    // Phase 115 / SCHM-01 — the era branch and the bypass it exists to avoid
    // =======================================================================

    /// The draft-07 meta-schema URI, spelled out once so every fence below
    /// uses the exact string the measured bypass was observed with.
    const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";

    /// `{n: integer}`, required, declaring draft-07 — the exact document the
    /// bypass was measured on.
    fn draft_07_declared_schema() -> Value {
        json!({
            "$schema": DRAFT_07,
            "type": "object",
            "properties": { "n": { "type": "integer" } },
            "required": ["n"]
        })
    }

    /// THE fence for SCHM-01. A schema that declares draft-07 must still be
    /// ENFORCED under the v2 Draft 2020-12 pin.
    ///
    /// Measured across `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 /
    /// 0.49.2: handing this document to `draft202012::new` AS-IS produces a
    /// validator that accepts every instance. If this test ever goes green
    /// only because the assertions were relaxed, output validation has become
    /// a no-op for every legacy-declared schema in the wild.
    #[test]
    fn v2_pin_still_enforces_a_draft_07_declared_schema() {
        let schema = draft_07_declared_schema();

        assert!(
            schema_mismatch(&schema, &json!({ "wrong": true }), Some(Era::V2)).is_some(),
            "BYPASS: the v2 Draft 2020-12 pin accepted an instance missing the REQUIRED `n`. A \
             `None` here means the pin compiled the draft-07 declaration into a VACUOUS validator \
             (empty vocabulary set) and emit-time output validation has silently become a no-op \
             for every schema that declares a legacy $schema. Restore the normalize-then-pin step \
             in `compile_2020_12`."
        );
        assert!(
            schema_mismatch(&schema, &json!({ "n": "not-an-int" }), Some(Era::V2)).is_some(),
            "BYPASS: the v2 Draft 2020-12 pin accepted a STRING where the schema declares \
             `integer`. See the message above — `type` is one of the seven keywords measured to \
             be silently dropped when the $schema declaration is not normalized first."
        );
        assert_eq!(
            schema_mismatch(&schema, &json!({ "n": 7 }), Some(Era::V2)),
            None,
            "a conforming instance must still pass under the pin — the fix restores enforcement, \
             it does not make everything fail"
        );
    }

    /// An EMBEDDED SCHEMA RESOURCE — a subschema carrying its own `$id` — with
    /// its dialect declaration on it, filed under a `$defs` entry the CALLER
    /// names. Under 2020-12 this is the sanctioned way to put a `$schema` below
    /// the root, and `jsonschema` 0.49.2 honours it.
    ///
    /// The NAME is a parameter because it is the whole variable of the 115-14
    /// closure: a `$defs` key is an AUTHOR-CHOSEN NAME, never a keyword, so the
    /// document's meaning must not change when it is spelled `default` instead
    /// of `Inner`. See
    /// [`v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`].
    ///
    /// The `$id` host is `example.test` and is deliberately NOT dereferenceable:
    /// SEP-2106 requires zero network and filesystem I/O, and an `$id` alone
    /// establishes a base URI without any fetch. The only `$ref` here is a
    /// LOCAL JSON pointer.
    fn embedded_legacy_resource_named(definition_name: &str) -> Value {
        json!({
            "type": "object",
            "properties": { "n": { "$ref": format!("#/$defs/{definition_name}") } },
            "$defs": {
                definition_name: {
                    "$id": "https://example.test/inner",
                    "$schema": DRAFT_07,
                    "type": "integer"
                }
            }
        })
    }

    /// The historical spelling of the fixture above, kept as the one name the
    /// existing fences and the module rustdoc already refer to. One source of
    /// truth for the shape; only the definition NAME varies.
    fn embedded_legacy_resource_schema() -> Value {
        embedded_legacy_resource_named("Inner")
    }

    /// The collision, parameterised over BOTH axes: an `$id`-bearing embedded
    /// schema resource carrying a legacy `$schema`, filed under an entry of an
    /// arbitrary subschema-map CONTAINER under an arbitrary author-chosen NAME.
    ///
    /// One shape, two variables, so a fence can sweep the whole grid and a
    /// failure names the exact (container, name) cell rather than a document.
    ///
    /// There is deliberately NO `$ref` here and the document is never compiled:
    /// the fences that use it assert the NORMALIZATION, not a verdict. The `$id`
    /// host is the reserved, non-resolvable `example.test` for the SEP-2106
    /// reason the sibling fixtures already state — an `$id` alone establishes a
    /// base URI without any fetch.
    fn embedded_legacy_resource_in_container(container: &str, name: &str) -> Value {
        json!({
            "type": "object",
            container: {
                name: {
                    "$id": "https://example.test/inner",
                    "$schema": DRAFT_07,
                    "type": "integer"
                }
            }
        })
    }

    /// The ARRAY-POSITION analog: an `$id`-bearing embedded schema resource
    /// carrying a legacy `$schema`, filed at `container[index]` — the shape
    /// `allOf` / `anyOf` / `oneOf` / `prefixItems` carry.
    ///
    /// `115-REVIEW.md` **WR-03**. The descent is implemented (rule 4 of the
    /// module rustdoc; the `Value::Array` arms of [`first_legacy_dialect`] and
    /// [`pin_dialect_in_place`]) and was reachable by NO test, property draw or
    /// corpus seed — deleting both arms passed the entire suite, measured twice
    /// independently. This fixture is what makes the position reachable.
    ///
    /// # Why this needs no NAME parameter, unlike its map twin
    ///
    /// An array element has no key. There is therefore no author-chosen name for
    /// a [`DATA_ONLY_KEYWORDS`] entry to collide with, which is the exact shape
    /// that reopened SCHM-01 in rounds 1, 2 and CR-01 — it structurally cannot
    /// occur here. The variable this fixture parameterises is the INDEX instead:
    /// a walk that visits only element 0 is a real and distinct defect.
    ///
    /// The `index` inert `{}` fillers ahead of the resource are what make a
    /// non-zero index addressable. They carry no `$schema`, so they are
    /// invisible to both halves and cannot themselves satisfy the assertion.
    ///
    /// Same `example.test` non-resolvable `$id` and same deliberate absence of a
    /// `$ref` as [`embedded_legacy_resource_in_container`]: the fences that use
    /// this assert the NORMALIZATION, not a verdict, and an `$id` alone
    /// establishes a base URI without any fetch (SEP-2106).
    fn embedded_legacy_resource_in_array(container: &str, index: usize) -> Value {
        let mut branches: Vec<Value> = (0..index).map(|_| json!({})).collect();
        branches.push(json!({
            "$id": "https://example.test/inner",
            "$schema": DRAFT_07,
            "type": "integer"
        }));
        json!({ "type": "object", container: branches })
    }

    /// The same collision one keyword over: an `$id`-bearing embedded resource
    /// carrying a legacy `$schema`, filed under a `properties` entry the CALLER
    /// names rather than under a `$defs` entry.
    ///
    /// `properties` keys are instance-property names — author-chosen, exactly
    /// like `$defs` keys — so the position rule has to hold here too. This half
    /// is fenced STRUCTURALLY rather than behaviourally; see the test for why.
    ///
    /// Delegates to [`embedded_legacy_resource_in_container`] so there is ONE
    /// shape and only the container and the name vary.
    fn properties_embedded_legacy_resource_named(property_name: &str) -> Value {
        embedded_legacy_resource_in_container("properties", property_name)
    }

    /// The control: identical, minus the embedded declaration. Enforcement is
    /// known to work here on both eras, which is what makes the other two rows
    /// a statement about the DECLARATION rather than about `$ref` resolution.
    fn embedded_resource_control_schema() -> Value {
        json!({
            "type": "object",
            "properties": { "n": { "$ref": "#/$defs/Inner" } },
            "$defs": {
                "Inner": {
                    "$id": "https://example.test/inner",
                    "type": "integer"
                }
            }
        })
    }

    /// The regression row: a legacy declaration at the root AND on the embedded
    /// resource. Before 115-12 this measured `(v1, v2) = (Violates, Conforms)`.
    fn root_and_embedded_legacy_schema() -> Value {
        let mut schema = embedded_legacy_resource_schema();
        schema
            .as_object_mut()
            .expect("the literal above is an object")
            .insert("$schema".to_string(), Value::String(DRAFT_07.to_string()));
        schema
    }

    /// THE fence for the `115-VERIFICATION.md` BLOCKER. A legacy dialect
    /// declaration on an EMBEDDED SCHEMA RESOURCE must not survive the v2 pin.
    ///
    /// Measured twice independently on this tree (`115-REVIEW.md` CR-01 and
    /// `115-VERIFICATION.md`) against `jsonschema` 0.49.2, through
    /// `fuzz_support::validate_bytes`, with the instance `{"n":
    /// "NOT-AN-INTEGER"}`:
    ///
    /// | Case | `(v1, v2)` BEFORE 115-12 |
    /// |---|---|
    /// | embedded legacy resource | `(Conforms, Conforms)` — `type` silently dropped |
    /// | control, no embedded declaration | `(Violates, Violates)` |
    /// | root draft-07 + embedded | `(Violates, Conforms)` — **v2 weaker than v1** |
    ///
    /// This test lives in `mod tests`, NOT in `fuzz_support_tests`: `fuzzing` is
    /// in neither `default` nor `full`, so a fence written there does not run
    /// under `make quality-gate`. All three of the would-be fences for this
    /// defect (`normalization_cases()`, `arb_schema_document()`,
    /// `is_dialect_neutral`) either excluded the shape structurally or sat
    /// behind a feature the gate does not enable, which is exactly why it
    /// shipped.
    #[test]
    fn v2_pin_still_enforces_an_embedded_legacy_resource() {
        let violating = json!({ "n": "NOT-AN-INTEGER" });
        let conforming = json!({ "n": 7 });

        let rows = [
            (
                "embedded-legacy-resource",
                embedded_legacy_resource_schema(),
            ),
            (
                "control-no-nested-schema",
                embedded_resource_control_schema(),
            ),
            ("root-draft07 + embedded", root_and_embedded_legacy_schema()),
        ];

        for (label, schema) in &rows {
            assert!(
                schema_mismatch(schema, &violating, Some(Era::V2)).is_some(),
                "BYPASS ({label}): the v2 Draft 2020-12 pin accepted a STRING where the embedded \
                 schema resource declares `integer`. A `None` here means the legacy `$schema` on \
                 the `$id`-bearing `$defs.Inner` survived normalization, resolved an EMPTY \
                 vocabulary set there and produced a sub-validator that accepts everything — the \
                 vacuous-validator bypass the pin exists to close, moved one level down. \
                 `normalize_schema_dialect` must rewrite EVERY dialect declaration, not just the \
                 root one."
            );
            assert_eq!(
                schema_mismatch(schema, &conforming, Some(Era::V2)),
                None,
                "({label}) a conforming instance must still pass under the pin — the fix restores \
                 enforcement, it does not make everything fail"
            );
        }

        // The regression DIRECTION, stated as its own assertion: row 3's v1
        // column was `Violates` all along, so a `None` on v2 here means v2 is
        // measurably WEAKER than v1 — the one direction SCHM-01 forbids.
        let regression_direction = root_and_embedded_legacy_schema();
        assert!(
            schema_mismatch(&regression_direction, &violating, Some(Era::V1)).is_some(),
            "v1 must keep rejecting this instance — D-01 freezes the v1 arm, so if this became a \
             `None` the v1 auto-detect wire moved, which this phase declined to do"
        );
        assert!(
            schema_mismatch(&regression_direction, &violating, Some(Era::V2)).is_some(),
            "REGRESSION DIRECTION: `(v1, v2) = (Violates, Conforms)` — v2 accepting an instance \
             v1 correctly rejects is the exact regression SCHM-01 was written to forbid. \
             Measured as (Violates, Conforms) before 115-12; it must now be (Violates, Violates)."
        );

        // Row 1's v1 column is NOT this phase's to move. D-01 freezes the v1
        // arm at `jsonschema::validator_for`, whose auto-detect honours the
        // embedded draft-07 declaration and therefore also drops `type` — it
        // measured `(Conforms, Conforms)`. Changing that is a v1 BEHAVIOUR
        // change this phase explicitly declined; assert it stayed put.
        assert_eq!(
            schema_mismatch(
                &embedded_legacy_resource_schema(),
                &violating,
                Some(Era::V1)
            ),
            None,
            "v1 is frozen by D-01: its auto-detect honours the embedded draft-07 declaration and \
             drops `type` there, measured `(Conforms, Conforms)`. A `Some` here means the v1 arm \
             changed behaviour, which is a breaking change for every 2025-11-25 server and is not \
             what 115-12 was allowed to do"
        );
    }

    /// THE fence for `115-VERIFICATION.md`'s POSITION blocker: an `$id`-bearing
    /// EMBEDDED SCHEMA RESOURCE filed under a `$defs` or `properties` entry
    /// whose AUTHOR-CHOSEN NAME collides with one of the
    /// [`DATA_ONLY_KEYWORDS`] is still a SCHEMA POSITION, and a legacy dialect
    /// declaration on it must not survive the v2 pin.
    ///
    /// Measured on this tree before 115-14, through
    /// `fuzz_support::{validate_bytes, normalize_bytes}` against `jsonschema`
    /// 0.49.2, with two documents differing ONLY in the NAME of the `$defs`
    /// entry, and the instance `{"n": "NOT-AN-INTEGER"}`:
    ///
    /// | Document | `normalize_schema_dialect` | `(v1, v2)` |
    /// |---|---|---|
    /// | `$defs.Inner` (control) | rewritten (`Cow::Owned`) | `(Conforms, Violates)` |
    /// | `$defs.default` (renamed) | byte-identical — nothing rewritten | `(Conforms, Conforms)` |
    ///
    /// The assertions run in BOTH directions, which is what makes this a fence
    /// for the RULE rather than for one document. Without the KEYWORD-position
    /// half, the cheapest way to make the NAME-position half pass is to delete
    /// [`DATA_ONLY_KEYWORDS`] — which silently corrupts every author's `const`,
    /// `enum`, `default` and `examples` payload.
    #[test]
    fn v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword() {
        use std::borrow::Cow;

        let violating = json!({ "n": "NOT-AN-INTEGER" });
        let conforming = json!({ "n": 7 });

        // (a) NAME position, `$defs`, BEHAVIOURAL. `Inner` is the control: it
        // was already enforced before this closure, so a failure confined to
        // the other four names is a statement about the NAME and nothing else.
        for definition_name in ["const", "enum", "default", "examples", "Inner"] {
            let schema = embedded_legacy_resource_named(definition_name);
            assert!(
                schema_mismatch(&schema, &violating, Some(Era::V2)).is_some(),
                "BYPASS ($defs.{definition_name}): the v2 Draft 2020-12 pin accepted a STRING \
                 where the embedded schema resource declares `integer`. Measured before 115-14: \
                 `$defs.default` -> verdicts=(Conforms, Conforms), rewritten=false, against the \
                 control `$defs.Inner` -> (Conforms, Violates), rewritten=true. A `$defs` key is \
                 an AUTHOR-CHOSEN NAME, never a keyword, so DATA_ONLY_KEYWORDS must NOT be \
                 applied to it — the values of a $defs / properties / patternProperties / \
                 definitions / dependentSchemas map are schema positions REGARDLESS of the name \
                 they are filed under. See SUBSCHEMA_MAP_KEYWORDS."
            );
            assert_eq!(
                schema_mismatch(&schema, &conforming, Some(Era::V2)),
                None,
                "($defs.{definition_name}) a conforming instance must still pass under the pin — \
                 the position rule restores enforcement, it does not make everything fail"
            );
        }

        // (b) NAME position, `properties`, STRUCTURAL — deliberately not
        // behavioural. `jsonschema` 0.49.2 still enforces `type` under a
        // `properties` entry carrying a surviving legacy declaration, so a
        // behavioural assertion here would pass against the position-BLIND
        // walkers: a fence that cannot fire. The module's own doc states the
        // walk is deliberately a SUPERSET of what the library honours precisely
        // so correctness cannot depend on that library detail, so the property
        // asserted here is the rewrite itself.
        for &property_name in DATA_ONLY_KEYWORDS {
            let schema = properties_embedded_legacy_resource_named(property_name);
            let normalized = normalize_schema_dialect(&schema);
            assert!(
                matches!(normalized, Cow::Owned(_)),
                "properties.{property_name} carries an $id-bearing embedded resource with a \
                 legacy $schema and was NOT rewritten (Cow::Borrowed). `properties` keys are \
                 instance-property NAMES, author-chosen exactly like $defs keys, so the \
                 DATA_ONLY_KEYWORDS filter must not reach them. This half is structural because \
                 jsonschema 0.49.2 happens to still enforce `type` here today — a behavioural \
                 assertion would pass against the defective code."
            );
            assert_eq!(
                normalized
                    .pointer(&format!("/properties/{property_name}/$schema"))
                    .and_then(Value::as_str),
                Some(DRAFT_2020_12),
                "properties.{property_name}/$schema must be OVERWRITTEN with the 2020-12 URI. A \
                 surviving legacy declaration on an $id-bearing resource resolves an EMPTY \
                 vocabulary set the moment the library's current behaviour changes."
            );
        }

        // (c) KEYWORD position, the twin. The SAME four words used as REAL
        // keywords carry instance DATA, and a `$schema` inside one of them must
        // still come back byte-identical. This is what makes the fix a POSITION
        // distinction rather than a deleted data guard.
        for &keyword in DATA_ONLY_KEYWORDS {
            let document = json!({
                "type": "object",
                keyword: { "$schema": DRAFT_07, "note": "data" }
            });
            let normalized = normalize_schema_dialect(&document);
            assert!(
                matches!(normalized, Cow::Borrowed(_)),
                "a $schema inside a REAL `{keyword}` payload is instance DATA, not a dialect \
                 declaration, so nothing must be cloned for {document}. If this allocated, the \
                 position-aware fix was implemented by DELETING the data guard instead of by \
                 distinguishing NAME position from KEYWORD position."
            );
            assert_eq!(
                *normalized, document,
                "a $schema inside a REAL `{keyword}` payload must come back byte-identical — \
                 rewriting it changes which instances conform, which is a semantic corruption of \
                 the author's schema and not a normalization"
            );
        }
    }

    /// THE fence for `115-REVIEW.md` CR-01: an `$id`-bearing EMBEDDED SCHEMA
    /// RESOURCE is rewritten in EVERY spec-defined subschema-map container,
    /// whatever author-chosen NAME it is filed under.
    ///
    /// # The container list below is deliberately NOT `SUBSCHEMA_MAP_KEYWORDS`
    ///
    /// It is this test's OWN literal, written out by hand, and that is the
    /// entire point. Every fence added before this one enumerated
    /// [`SUBSCHEMA_MAP_KEYWORDS`], so an OMISSION FROM that list was invisible
    /// to all of them — CR-01's sharpest sentence is that the one instrument
    /// advertised as consulting no crate-derived list "is in fact gated by a
    /// crate-derived list one line earlier". **A fence parameterised by the list
    /// whose incompleteness is the defect cannot fire on that defect.** The
    /// list membership is asserted separately, at the bottom, as a guard.
    ///
    /// The six containers are not a guess: they are the keywords the pinned
    /// `jsonschema` 0.49.2 meta-schemas (draft-04 / -06 / -07, 2019-09, 2020-12)
    /// themselves define as OBJECT-typed keywords whose `additionalProperties`
    /// references the meta-schema itself (`{"$ref":"#"}`, `{"$recursiveRef":"#"}`,
    /// `{"$dynamicRef":"#meta"}`, or an `anyOf` with such a branch). `$vocabulary`
    /// (values are booleans) and `dependentRequired` (values are string arrays)
    /// are excluded by that same criterion. See [`SUBSCHEMA_MAP_KEYWORDS`].
    ///
    /// # Why this fence is STRUCTURAL and not behavioural
    ///
    /// Both `dependencies.Inner` and `dependencies.default` measure
    /// `(Violates, Violates)` on the pinned `jsonschema` 0.49.2 — the library
    /// still enforces `type` through a surviving legacy declaration at that
    /// position today. **A verdict assertion would therefore PASS against the
    /// defective code**: a fence that cannot fire, which is the exact failure
    /// mode this phase shipped three times. The observable that distinguishes
    /// fixed from broken is the NORMALIZATION — the `Cow` borrow/own decision,
    /// which is also precisely what gates `compile_2020_12`'s `tracing::warn!`,
    /// the only D-02 diagnostic an author gets. With `Cow::Borrowed` the author
    /// is told nothing at all. The rewritten-pointer assertion is what ties the
    /// rewrite to the position under test, since `Owned` alone would be
    /// satisfied by a clone made for some other declaration.
    #[test]
    fn v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map() {
        use std::borrow::Cow;

        // This test's OWN literal — NOT SUBSCHEMA_MAP_KEYWORDS. See the rustdoc.
        let containers = [
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
            "dependencies",
        ];

        let mut examined = 0usize;
        let mut violations: Vec<String> = Vec::new();

        for container in containers {
            for &name in DATA_ONLY_KEYWORDS {
                examined += 1;
                let schema = embedded_legacy_resource_in_container(container, name);
                let normalized = normalize_schema_dialect(&schema);
                let rewritten = matches!(normalized, Cow::Owned(_));
                let declared = normalized
                    .pointer(&format!("/{container}/{name}/$schema"))
                    .and_then(Value::as_str);
                if !rewritten || declared != Some(DRAFT_2020_12) {
                    violations.push(format!(
                        "{container}/{name}: rewritten={rewritten}, \
                         /{container}/{name}/$schema={declared:?}"
                    ));
                }
            }
        }

        // Anti-vacuity: the grid was actually swept.
        assert_eq!(
            examined,
            containers.len() * DATA_ONLY_KEYWORDS.len(),
            "the fence must examine every (container, name) pair — {} containers x {} \
             data-only names",
            containers.len(),
            DATA_ONLY_KEYWORDS.len()
        );

        // Collected, never asserted in-loop: a first-failure abort would report
        // ONE pair and hide which of the six containers are actually broken.
        // The complete broken set is the evidence this fence exists to produce.
        assert!(
            violations.is_empty(),
            "an $id-bearing embedded schema resource carrying a legacy $schema was NOT rewritten \
             in {} of {examined} (container, name) positions:\n{violations:#?}\nEach of the six \
             containers above is a spec-defined map from AUTHOR-CHOSEN NAMES to subschemas, so \
             DATA_ONLY_KEYWORDS must never be tested against its keys. This assertion is \
             STRUCTURAL, not behavioural, on purpose: on jsonschema 0.49.2 both \
             `dependencies.Inner` and `dependencies.default` report (Violates, Violates), so a \
             verdict assertion would pass against the defective code. The observable is the \
             borrow/own decision — which is also what gates compile_2020_12's tracing::warn!, so \
             with Cow::Borrowed the author is told nothing at all. See SUBSCHEMA_MAP_KEYWORDS.",
            violations.len()
        );

        // The list-membership guard, asserted SEPARATELY from the sweep above so
        // the sweep stays independent of the list it is checking.
        assert!(
            SUBSCHEMA_MAP_KEYWORDS.contains(&"dependencies"),
            "SUBSCHEMA_MAP_KEYWORDS omits `dependencies` — 115-REVIEW.md CR-01. It is \
             draft-04..2019-09's own map-from-instance-property-NAME-to-subschema keyword, and \
             `jsonschema` 0.49.2 still honours it under the 2020-12 pin (D-115-03-C, measured by \
             this module's own fuzz_support_tests), which is what makes its VALUES live schema \
             positions."
        );
    }

    /// The ARRAY-position twin of
    /// [`v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`]:
    /// an embedded schema resource carrying a legacy `$schema` is rewritten at
    /// EVERY spec-defined array position, at index 0 and beyond.
    ///
    /// `115-REVIEW.md` **WR-03**, the round-4 residual. Before this fence,
    /// deleting BOTH `Value::Array` arms — the detector's at
    /// [`first_legacy_dialect`] and the rewriter's at [`pin_dialect_in_place`] —
    /// passed the entire suite: 25/25 here, 21/21 property draws and a clean
    /// fuzz `cargo check`, measured independently by the round-4 reviewer and
    /// again by the round-4 verifier. The descent was correct and unfenced.
    ///
    /// # Why the literal below is this test's OWN, and why there is no shipped mirror
    ///
    /// Array descent consults NO list — it is unconditional on `Value::Array`,
    /// which is precisely why the position is safe from the list-incompleteness
    /// defect class (CR-01, WR-02) that a `SUBSCHEMA_ARRAY_KEYWORDS` constant
    /// would import. A shipped constant here would be a fourth mirror to keep in
    /// lockstep for zero implementation benefit. The literal stays local for the
    /// standing `D-115-AI(4)` reason: a fence's reachability must not be derived
    /// from the artifact it checks.
    ///
    /// # The anti-vacuity assertion is a HARD-CODED count, deliberately
    ///
    /// `115-REVIEW.md` WR-01 showed that an expectation spelled as the PRODUCT
    /// OF THE TWO LITERALS' OWN LENGTHS recomputes the loop bound from the loop
    /// bound and can never fail — it does not notice a SHORTENED literal, which
    /// is the drift it is credited with catching. `8` is written out so that
    /// deleting a keyword or an index from the sweep fails this test.
    #[test]
    fn v2_pin_rewrites_an_embedded_resource_at_every_spec_defined_array_position() {
        use std::borrow::Cow;

        // This test's OWN literal — there is no shipped list. See the rustdoc.
        let containers = ["allOf", "anyOf", "oneOf", "prefixItems"];
        // 0 catches "descends into arrays at all"; 2 catches "visits only the
        // first element", which is a distinct and real walk defect.
        let indices = [0usize, 2];

        let mut examined = 0usize;
        let mut violations: Vec<String> = Vec::new();

        for container in containers {
            for index in indices {
                examined += 1;
                let schema = embedded_legacy_resource_in_array(container, index);
                let normalized = normalize_schema_dialect(&schema);
                let rewritten = matches!(normalized, Cow::Owned(_));
                let declared = normalized
                    .pointer(&format!("/{container}/{index}/$schema"))
                    .and_then(Value::as_str);
                if !rewritten || declared != Some(DRAFT_2020_12) {
                    violations.push(format!(
                        "{container}[{index}]: rewritten={rewritten}, \
                         /{container}/{index}/$schema={declared:?}"
                    ));
                }
            }
        }

        // Anti-vacuity, NOT recomputed from the loop bounds (WR-01's defect):
        // 4 array-position keywords x 2 indices.
        assert_eq!(
            examined, 8,
            "the fence must examine all 8 (array keyword, index) positions — a shortened \
             `containers` or `indices` literal is exactly the drift this hard-coded count \
             exists to catch, and a length-derived expectation cannot see it"
        );

        // Collected, never asserted in-loop, for the reason the map twin states.
        assert!(
            violations.is_empty(),
            "an $id-bearing embedded schema resource carrying a legacy $schema was NOT rewritten \
             in {} of {examined} ARRAY positions:\n{violations:#?}\nEach keyword above holds its \
             subschemas in a JSON array, so both halves reach them through their `Value::Array` \
             arm — deleting those arms is what this fence exists to catch (115-REVIEW.md WR-03). \
             An array ELEMENT has no key, so DATA_ONLY_KEYWORDS can never apply here and no \
             name-collision shape is possible; the observable is the borrow/own decision, which \
             is also what gates compile_2020_12's tracing::warn!.",
            violations.len()
        );
    }

    /// The two keyword lists must be DISJOINT — a silent precondition of both
    /// member dispatches (`115-REVIEW.md` WR-05).
    ///
    /// The two halves are written in different SHAPES, and that is what makes
    /// this a fence rather than a tautology. [`first_legacy_dialect_in_member`]
    /// is a `match` whose first arm guards on the VALUE kind and the key class
    /// TOGETHER; [`pin_dialect_in_member`] is an `if` chain testing the KEY class
    /// FIRST. For a key present in BOTH lists whose value is NOT an object, the
    /// two therefore disagree: the detector falls to the `DATA_ONLY_KEYWORDS` arm
    /// and returns `None`, while the rewriter takes the `SUBSCHEMA_MAP_KEYWORDS`
    /// branch and DESCENDS. That is a detector/rewriter divergence — which
    /// [`first_legacy_dialect`]'s own doc states "is a defect" — and it produces
    /// a `Cow::Owned` document still carrying a legacy declaration, which
    /// [`compile_2020_12`] then announces as "the declaration is ignored" having
    /// ignored nothing.
    ///
    /// The lists ARE disjoint today. 115-16 narrowed the margin by adding
    /// `dependencies`, which is a subschema map in draft-07 and pure data in some
    /// vendor dialects — precisely the kind of keyword a future edit might
    /// reasonably want in both places.
    ///
    /// Rewriting the two dispatches into a common shape is WR-05's other half; it
    /// touches both walkers' bodies and is deliberately NOT done here.
    #[test]
    fn keyword_lists_are_disjoint() {
        let overlap: Vec<&&str> = SUBSCHEMA_MAP_KEYWORDS
            .iter()
            .filter(|keyword| DATA_ONLY_KEYWORDS.contains(keyword))
            .collect();

        assert!(
            overlap.is_empty(),
            "{overlap:?} appear in BOTH SUBSCHEMA_MAP_KEYWORDS and DATA_ONLY_KEYWORDS. The two \
             member dispatches silently depend on these lists being disjoint: the DETECTOR \
             (first_legacy_dialect_in_member) is a `match` guarding on the VALUE kind and the key \
             class together, while the REWRITER (pin_dialect_in_member) is an `if` chain testing \
             the KEY class first. For such a key with a NON-OBJECT value the detector returns None \
             while the rewriter DESCENDS — a detector/rewriter divergence, which this module's own \
             docs state is a defect, yielding a Cow::Owned that still carries a legacy declaration \
             while compile_2020_12 announces that declaration as ignored. Either keep the lists \
             disjoint or rewrite BOTH dispatches into one shape (115-REVIEW.md WR-05)."
        );
    }

    /// D-01's freeze, asserted rather than assumed: the same draft-07 document
    /// behaves on v1 exactly as it did before the v2 pin existed, and an
    /// absent protocol context (`None`) resolves to v1, never to v2.
    #[test]
    fn v1_validation_is_unchanged_by_the_v2_pin() {
        let schema = draft_07_declared_schema();

        for era in [Some(Era::V1), None] {
            assert!(
                schema_mismatch(&schema, &json!({ "wrong": true }), era).is_some(),
                "v1 auto-detect must keep enforcing draft-07 `required` (era: {era:?})"
            );
            assert!(
                schema_mismatch(&schema, &json!({ "n": "not-an-int" }), era).is_some(),
                "v1 auto-detect must keep enforcing draft-07 `type` (era: {era:?})"
            );
            assert_eq!(
                schema_mismatch(&schema, &json!({ "n": 7 }), era),
                None,
                "v1 must keep accepting a conforming instance (era: {era:?})"
            );
        }
    }

    /// A schema whose verdict genuinely DIFFERS between the two eras.
    ///
    /// `contentEncoding` is an ASSERTION in draft-07 but only an ANNOTATION
    /// from 2019-09 onwards, so a non-base64 string is rejected under v1's
    /// auto-detect and accepted under the v2 2020-12 pin. Measured on
    /// `jsonschema` 0.49.2.
    ///
    /// The `description` carries `prefix` purely to make each caller's
    /// document a DISTINCT cache key, so a test can start from a cold cache.
    /// `description` is an annotation in every draft and changes no verdict.
    fn era_divergent_schema(prefix: &str) -> Value {
        serde_json::from_str(&format!(
            r#"{{
                "$schema": "{DRAFT_07}",
                "type": "string",
                "contentEncoding": "base64",
                "description": "{prefix}"
            }}"#
        ))
        .expect("the template above is valid JSON")
    }

    /// An instance the [`era_divergent_schema`] eras disagree about.
    fn era_divergent_instance() -> Value {
        json!("!!!not-base64!!!")
    }

    /// The cache fence: one process, one schema text, two eras — the second
    /// era must NOT be served the first era's validator.
    ///
    /// Before the key was widened to `(Era, schema text)` this was
    /// first-writer-wins for the whole process lifetime. CI runs with
    /// `--test-threads=1`, so this must not depend on parallel execution; the
    /// order is expressed by the call sequence inside the test, and the twin
    /// test below runs the opposite order over a DISTINCT document so it too
    /// starts from a cold cache.
    ///
    /// Note carefully what this schema demonstrates: here `Era::V2` is MORE
    /// PERMISSIVE than `Era::V1`, deliberately, because `contentEncoding` is
    /// an assertion in draft-07 and only an annotation under 2020-12. The
    /// converse direction is ALSO reachable — `$ref` siblings are ignored in
    /// draft-07 but apply under 2020-12, making v2 stricter there. Both
    /// directions were measured on `jsonschema` 0.49.2, so any cross-era
    /// monotonicity claim ("v2 rejects everything v1 rejects", or its
    /// converse) is FALSE in BOTH directions. None is made anywhere in this
    /// phase — 115-09's fuzz target must not assert one either.
    #[test]
    fn same_schema_text_yields_independent_verdicts_per_era_in_one_process() {
        let schema = era_divergent_schema("v1first");
        let instance = era_divergent_instance();

        let v1 = schema_mismatch(&schema, &instance, Some(Era::V1));
        let v2 = schema_mismatch(&schema, &instance, Some(Era::V2));

        assert!(
            v1.is_some(),
            "v1 auto-detects draft-07, where `contentEncoding` is an ASSERTION: {v1:?}"
        );
        assert_eq!(
            v2, None,
            "v2 pins 2020-12, where `contentEncoding` is only an ANNOTATION and asserts \
             nothing. A `Some` here means the V1 entry was served for the V2 lookup — the cache \
             key lost its era half."
        );
    }

    /// The twin of the test above with the call order REVERSED, over a
    /// distinct document so the process-global cache is cold on entry. Both
    /// orders must produce the same per-era answers; if either order can flip
    /// a verdict, the cache is era-blind.
    #[test]
    fn same_schema_text_yields_independent_verdicts_in_the_opposite_order() {
        let schema = era_divergent_schema("v2first");
        let instance = era_divergent_instance();

        let v2 = schema_mismatch(&schema, &instance, Some(Era::V2));
        let v1 = schema_mismatch(&schema, &instance, Some(Era::V1));

        assert_eq!(
            v2, None,
            "v2-first must give the same v2 answer as v2-second: {v2:?}"
        );
        assert!(
            v1.is_some(),
            "v1-second must give the same v1 answer as v1-first. A `None` here means the V2 \
             entry was served for the V1 lookup — first-writer-wins across eras."
        );
    }

    /// The LOUD half of D-02: draft-07 constructs that cannot be expressed
    /// under 2020-12 fail to COMPILE, and the failure is reported through the
    /// existing schema-invalid message rather than silently passing.
    #[test]
    fn structurally_incompatible_draft_07_constructs_report_a_schema_error_not_silence() {
        // draft-04/07 boolean `exclusiveMinimum`; 2020-12 requires a number.
        let boolean_exclusive_minimum = json!({
            "$schema": DRAFT_07,
            "exclusiveMinimum": true
        });
        // draft-07 array-form `items`; 2020-12 spells this `prefixItems` and
        // requires `items` to be a single schema.
        let tuple_items = json!({
            "$schema": DRAFT_07,
            "items": [{ "type": "string" }, { "type": "number" }]
        });

        for schema in [&boolean_exclusive_minimum, &tuple_items] {
            let mismatch = schema_mismatch(schema, &json!({}), Some(Era::V2)).expect(
                "a draft-07 construct that 2020-12 cannot express must be REPORTED under the v2 \
                 pin, not silently accepted",
            );
            assert!(
                mismatch.contains("outputSchema"),
                "the message must say the schema itself is at fault: {mismatch}"
            );
        }
    }

    /// SEP-2106, behavioural half: an external `$ref` must fail to compile,
    /// with no network or filesystem I/O, on BOTH eras.
    ///
    /// This is the *behavioural* half only. The *structural* fence — that
    /// `jsonschema`'s `resolve-http` / `resolve-file` features never enter the
    /// build graph through cargo feature unification — is 115-08's manifest
    /// tripwire, because a behavioural test like this one would still pass
    /// (merely louder, and after a live fetch) if `resolve-http` were enabled.
    #[test]
    fn external_ref_fails_to_compile_with_no_network_io() {
        let remote = json!({ "$ref": "https://example.com/remote.json" });
        let local_file = json!({ "$ref": "file:///etc/passwd" });
        let relative_under_http_id = json!({
            "$id": "https://example.com/root.json",
            "$ref": "sibling.json"
        });

        let started = std::time::Instant::now();
        for era in [Some(Era::V1), Some(Era::V2)] {
            for schema in [&remote, &local_file, &relative_under_http_id] {
                assert!(
                    schema_mismatch(schema, &json!({}), era).is_some(),
                    "an external $ref must be a hard compile error, never a fetch (era: \
                     {era:?}, schema: {schema})"
                );
            }
        }
        let elapsed = started.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "six external-$ref refusals took {elapsed:?}. Refusal is measured at ~60 µs each; a \
             wall-clock cost anywhere near a second means something is resolving the URI over \
             the network or the filesystem."
        );
    }

    /// D-04, which 115-RESEARCH § Finding 7 measured is ALREADY true today: an
    /// object-typed schema does not conform to a scalar, an array or `null`,
    /// on either era.
    ///
    /// "Reject" here means **a warning is logged** — not that the call fails.
    /// [`warn_on_schema_mismatch`] returns normally for every one of these
    /// inputs and produces no error result. Escalating v2 to a hard error is
    /// deliberately OUT OF SCOPE for this phase: it is a new production
    /// failure mode, and 115-10 books it as a deferred item.
    #[test]
    fn an_object_schema_rejects_a_scalar_on_both_eras_warn_only() {
        let schema = person_schema();
        let non_objects = [json!(42), json!(null), json!([1, 2]), json!("s")];

        for era in [Some(Era::V1), Some(Era::V2)] {
            for value in &non_objects {
                assert!(
                    schema_mismatch(&schema, value, era).is_some(),
                    "an object schema must report a non-object value (era: {era:?}, value: \
                     {value})"
                );
                // Warn-only: this returns, it does not fail the call.
                warn_on_schema_mismatch("demo_tool", &schema, value, era);
            }
        }
    }

    /// The blast-radius fence for D-01. `Draft::default() == Draft202012`, so
    /// an `outputSchema` that declares no `$schema` at all already compiles as
    /// 2020-12 under today's auto-detect. The v2 pin therefore changes
    /// behaviour ONLY for schemas that declare something — this test is what
    /// keeps that claim honest.
    #[test]
    fn an_undeclared_schema_behaves_identically_on_both_eras() {
        let schema = person_schema();
        let values = [
            json!({ "name": "Ada", "age": 36 }),
            json!({ "age": "not-a-number" }),
            json!({}),
            json!(42),
        ];

        for value in &values {
            assert_eq!(
                schema_mismatch(&schema, value, Some(Era::V1)),
                schema_mismatch(&schema, value, Some(Era::V2)),
                "an undeclared schema must give the identical verdict on both eras (value: \
                 {value})"
            );
        }
    }

    /// The `normalize_schema_dialect` cases, as a set, so both the structural
    /// test and the idempotence test cover the same ground.
    ///
    /// Each entry is `(schema, expected_owned)` — `true` means the normalizer
    /// must have rewritten the document.
    fn normalization_cases() -> Vec<(Value, bool)> {
        vec![
            // (a) no `$schema` at all
            (person_schema(), false),
            // (b) already 2020-12
            (json!({ "$schema": DRAFT_2020_12, "type": "object" }), false),
            // (c) a draft-07 root declaration, with siblings that must survive
            (draft_07_declared_schema(), true),
            // (d) a NESTED `$schema` and no root one. `$id`-less, so it is
            // INERT — `jsonschema` does not honour it and it cannot trigger the
            // bypass. It is rewritten anyway: the walk is deliberately a
            // superset of what the library honours, which is what makes the
            // "no legacy declaration survives" postcondition statable without a
            // per-node `$id` analysis.
            (
                json!({
                    "type": "object",
                    "properties": { "a": { "$schema": DRAFT_07, "type": "string" } }
                }),
                true,
            ),
            // (e) an EMBEDDED SCHEMA RESOURCE: `$id` + `$schema` on a `$defs`
            // entry, and no root declaration. THIS is the shape 2020-12
            // sanctions and `jsonschema` honours, and it is the one the
            // root-only normalizer left unrewritten — the measured BLOCKER in
            // `115-VERIFICATION.md`. It lives in the case list rather than in a
            // standalone test so it flows through BOTH the structural fence and
            // the idempotence fence automatically.
            (embedded_legacy_resource_schema(), true),
            // (f) NAME POSITION, `$defs`: the identical embedded resource, filed
            // under an entry an author NAMED `default`. The position-blind walk
            // never visited it — measured `verdicts=(Conforms, Conforms)`,
            // `rewritten=false` — which is `115-VERIFICATION.md`'s BLOCKER.
            (embedded_legacy_resource_named("default"), true),
            // (g) NAME POSITION, `properties`: the same collision one keyword
            // over, in `properties` rather than `$defs`. Its behavioural half
            // cannot fence anything (jsonschema 0.49.2 still enforces `type`
            // there today), so it is fenced STRUCTURALLY, here and in
            // `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`.
            (properties_embedded_legacy_resource_named("examples"), true),
            // (h) NAME POSITION, `dependencies` — `115-REVIEW.md` CR-01. The
            // sixth spec-defined subschema map, omitted from
            // SUBSCHEMA_MAP_KEYWORDS until 115-16. Measured before the fix:
            // `dependencies.default` -> rewritten=FALSE against the control
            // `dependencies.Inner` -> rewritten=true, with BOTH names reporting
            // the same (Violates, Violates) verdict pair — which is why this row
            // is here, in the borrow/own case list, rather than in a behavioural
            // fence that could not have fired.
            (
                embedded_legacy_resource_in_container("dependencies", "default"),
                true,
            ),
            // (i)/(j)/(k) `115-REVIEW.md` WR-02: `patternProperties`,
            // `dependentSchemas` and `definitions` have been in
            // SUBSCHEMA_MAP_KEYWORDS since 115-14 and were exercised by no test,
            // no property draw and no corpus seed — deleting all three from every
            // copy passed the entire suite. These rows close that.
            // `patternProperties` keys are REGEXES and `default` is a valid one,
            // so no escaping is needed.
            (
                embedded_legacy_resource_in_container("patternProperties", "default"),
                true,
            ),
            (
                embedded_legacy_resource_in_container("dependentSchemas", "default"),
                true,
            ),
            (
                embedded_legacy_resource_in_container("definitions", "default"),
                true,
            ),
            // (l) ARRAY POSITION — `115-REVIEW.md` WR-03. `allOf`/`anyOf`/
            // `oneOf`/`prefixItems` hold their subschemas in an ARRAY, a
            // position no test, property draw or corpus seed reached: deleting
            // both `Value::Array` arms passed the entire suite, measured twice
            // independently. Index **1**, not 0, so a walk that visits only the
            // first element fails this row. It lives here rather than in a
            // standalone test for the reason row (e) states — it then flows
            // through BOTH the structural fence and the idempotence fence.
            (embedded_legacy_resource_in_array("allOf", 1), true),
        ]
    }

    /// Remove every `$schema` key at every depth, so two documents can be
    /// compared for "identical apart from the dialect declarations".
    ///
    /// Recursive on purpose: a root-only strip would report a LEGITIMATE nested
    /// rewrite as collateral damage, which is how this helper read before
    /// 115-12 made the normalizer recursive.
    fn strip_every_dollar_schema(node: &mut Value) {
        match node {
            Value::Object(map) => {
                map.remove("$schema");
                for value in map.values_mut() {
                    strip_every_dollar_schema(value);
                }
            },
            Value::Array(items) => items.iter_mut().for_each(strip_every_dollar_schema),
            _ => {},
        }
    }

    /// The PURE-function fence: `normalize_schema_dialect` alters `$schema`
    /// keys and NOTHING else.
    ///
    /// Behavioural equivalence through [`schema_mismatch`] cannot prove this —
    /// a normalizer that also dropped a sibling key would still make
    /// `v2_pin_still_enforces_a_draft_07_declared_schema` pass on the cases it
    /// happens to check. This asserts the rewrite is surgical: the borrow/own
    /// decision, the rewritten value, deep equality of everything else, and the
    /// postcondition that no legacy declaration survives at any depth.
    #[test]
    fn normalize_schema_dialect_changes_only_dollar_schema_keys() {
        use std::borrow::Cow;

        for (schema, expected_owned) in normalization_cases() {
            let normalized = normalize_schema_dialect(&schema);

            assert_eq!(
                matches!(normalized, Cow::Owned(_)),
                expected_owned,
                "borrow/own decision is wrong for {schema} — the no-op cases must allocate \
                 nothing"
            );

            // The postcondition, over EVERY case: after normalization no
            // `$schema` string anywhere in the document names a dialect other
            // than 2020-12. This is the single assertion that catches a
            // detector/rewriter disagreement — a `first_legacy_dialect` that
            // sees a declaration `pin_dialect_in_place` cannot reach would
            // return an `Owned` document that still carries it.
            assert_eq!(
                first_legacy_dialect(&normalized),
                None,
                "a legacy dialect declaration survived normalization of {schema} — \
                 first_legacy_dialect and pin_dialect_in_place have stopped agreeing on the \
                 traversal rule"
            );

            if expected_owned {
                // Everything except the `$schema` keys must be deep-equal.
                let mut before = schema.clone();
                let mut after = normalized.into_owned();
                for document in [&mut before, &mut after] {
                    strip_every_dollar_schema(document);
                }
                assert_eq!(
                    before, after,
                    "normalization touched a key other than a $schema key"
                );
            } else {
                assert_eq!(
                    *normalized, schema,
                    "a document needing no rewrite must come back byte-identical: {schema}"
                );
            }
        }

        // (c) in detail: the ROOT declaration is OVERWRITTEN, not deleted.
        let rooted = normalize_schema_dialect(&draft_07_declared_schema()).into_owned();
        assert_eq!(
            rooted.get("$schema").and_then(Value::as_str),
            Some(DRAFT_2020_12),
            "the root $schema must be OVERWRITTEN with the 2020-12 URI, not deleted"
        );

        // (d) in detail: the NESTED declaration is rewritten too. This
        // assertion was INVERTED before 115-12 — it asserted the nested
        // declaration stayed draft-07, which is precisely the shipped bypass.
        let nested = json!({
            "type": "object",
            "properties": { "a": { "$schema": DRAFT_07, "type": "string" } }
        });
        let normalized = normalize_schema_dialect(&nested);
        assert_eq!(
            normalized
                .pointer("/properties/a/$schema")
                .and_then(Value::as_str),
            Some(DRAFT_2020_12),
            "a nested $schema must be rewritten too: an $id-bearing sibling of this shape is an \
             embedded schema resource whose declaration jsonschema DOES honour, and leaving it \
             alone is the measured (Violates, Conforms) bypass 115-VERIFICATION.md reported"
        );
    }

    /// The DATA guard. A `$schema` that is instance data — not a dialect
    /// declaration — must come back byte-identical.
    ///
    /// This is the fence against the corruption the CR-01 fix sketch would have
    /// introduced: that sketch rewrote every `$schema` KEY unconditionally,
    /// which turns a `properties` entry for an instance property literally
    /// named `$schema` into an uncompilable schema (its subschema value is
    /// replaced by a string), and silently changes which instances match a
    /// `const`, an `enum` alternative, a `default` or an `examples` entry.
    ///
    /// The two rules that prevent it, both of which this test pins:
    /// a declaration is a STRING-valued `$schema`, and the walk never descends
    /// into a `const` / `enum` / `default` / `examples` payload
    /// (`DATA_ONLY_KEYWORDS`).
    #[test]
    fn normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone() {
        use std::borrow::Cow;

        // An instance property literally named `$schema`. Its value is a
        // SUBSCHEMA (an object), never a string, so it is not a declaration.
        let property_named_dollar_schema = json!({
            "type": "object",
            "properties": { "$schema": { "type": "string" } }
        });
        // A `$schema` string sitting inside a `const` payload: pure instance
        // DATA. Rewriting it would change which instances conform.
        let dollar_schema_inside_const = json!({
            "const": { "$schema": DRAFT_07, "note": "this is data, not a dialect" }
        });
        // The same, in a `default` payload — a value a client may substitute
        // for an absent instance. Added by 115-14: it passes both before and
        // after the position-aware fix, and exists so that a future edit which
        // "fixes" the NAME-position bypass by DELETING the data guard reports
        // exactly why it broke.
        let dollar_schema_inside_default = json!({
            "type": "object",
            "default": { "$schema": DRAFT_07, "note": "this is data, not a dialect" }
        });
        // And in an `examples` payload, which is an ARRAY of instance data —
        // the walk must not reach into it through the array either.
        let dollar_schema_inside_examples = json!({
            "type": "object",
            "examples": [{ "$schema": DRAFT_07, "note": "this is data, not a dialect" }]
        });

        for document in [
            &property_named_dollar_schema,
            &dollar_schema_inside_const,
            &dollar_schema_inside_default,
            &dollar_schema_inside_examples,
        ] {
            let normalized = normalize_schema_dialect(document);
            assert!(
                matches!(normalized, Cow::Borrowed(_)),
                "a $schema that is DATA is not a dialect declaration, so nothing must be cloned \
                 for {document}. If this allocated, either the string-valued rule or the \
                 DATA_ONLY_KEYWORDS skip (`const`, `enum`, `default`, `examples`) was dropped \
                 from first_legacy_dialect"
            );
            assert_eq!(
                *normalized, *document,
                "a $schema that is DATA must come back byte-identical: {document}"
            );
        }

        // The mixed document: a REAL root declaration alongside both data
        // shapes. The root is rewritten; the data is not.
        let mixed = json!({
            "$schema": DRAFT_07,
            "type": "object",
            "properties": { "$schema": { "type": "string" } },
            "const": { "$schema": DRAFT_07, "note": "this is data, not a dialect" }
        });
        let normalized = normalize_schema_dialect(&mixed);
        assert!(
            matches!(normalized, Cow::Owned(_)),
            "the mixed document DOES carry a real root declaration and must be rewritten"
        );
        assert_eq!(
            normalized.get("$schema").and_then(Value::as_str),
            Some(DRAFT_2020_12),
            "the real root declaration must be overwritten with the 2020-12 URI"
        );
        assert_eq!(
            normalized.pointer("/properties/$schema"),
            mixed.pointer("/properties/$schema"),
            "the `properties` entry for an instance property named `$schema` is a SUBSCHEMA, not \
             a dialect declaration — rewriting it to a string makes the document uncompilable"
        );
        assert_eq!(
            normalized.pointer("/const"),
            mixed.pointer("/const"),
            "a `const` payload is instance DATA. The walk must skip DATA_ONLY_KEYWORDS \
             (`const`, `enum`, `default`, `examples`); rewriting inside one changes which \
             instances conform, which is a semantic corruption, not a normalization"
        );
    }

    /// Normalizing twice equals normalizing once, for every case in
    /// [`normalization_cases`] — the fixed-example half of the idempotence
    /// property 115-09 holds over arbitrary generated input.
    #[test]
    fn normalize_schema_dialect_is_idempotent() {
        for (schema, _) in normalization_cases() {
            let once = normalize_schema_dialect(&schema).into_owned();
            let twice = normalize_schema_dialect(&once).into_owned();
            assert_eq!(
                once, twice,
                "normalization must be idempotent, but a second pass changed {schema}"
            );
        }
    }
}
