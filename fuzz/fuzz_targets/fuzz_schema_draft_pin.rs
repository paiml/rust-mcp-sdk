//! Fuzz target for the era-branched `outputSchema` validation path.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//!
//! ```bash
//! cd fuzz && cargo +nightly fuzz run fuzz_schema_draft_pin
//! ```
//!
//! **`+nightly` is REQUIRED and is not a style choice.** `cargo fuzz` passes
//! `-Zsanitizer=address`, which stable rustc refuses with "the option `Z` is
//! only accepted on the nightly compiler" — the build fails before a single
//! iteration runs. This was MEASURED on 2026-08-01 with cargo-fuzz 0.13.1 and a
//! stable default toolchain. The repo's `make test-fuzz` invokes the PLAIN form
//! (`Makefile:241`) and pipes every non-zero exit into `|| echo "… completed"`,
//! so on a stable default toolchain that target reports success having fuzzed
//! NOTHING. Do not cite `make test-fuzz` as evidence for this target; run the
//! `+nightly` command above, or the corpus replay in
//! `fuzz/corpus/fuzz_schema_draft_pin/README.md`, and check
//! `fuzz/artifacts/fuzz_schema_draft_pin/` is empty afterwards.
//!
//! # What is being fuzzed, and why
//!
//! `src/server/output_validation.rs` checks a handler's `structuredContent`
//! against the tool's declared `outputSchema` on EVERY structured result, and
//! Phase 115 (SCHM-01) branched it by protocol era: v1 auto-detects the dialect
//! from the document's own `$schema`, v2 pins JSON Schema Draft 2020-12 by
//! REWRITING that declaration first. Both halves of the input are attacker- or
//! author-shaped JSON, and the path runs inside request handling, so a panic
//! here is a remotely reachable unwind. The target drives the whole branch
//! through the `feature = "fuzzing"`-gated `fuzz_support` seam, which uses the
//! UNCACHED compile path so fuzzing cannot grow the process-global validator
//! cache without bound.
//!
//! # Input layout
//!
//! Splitting raw bytes at an arbitrary point makes BOTH halves fail to parse as
//! JSON on nearly every iteration, which degenerates this into a JSON-parser
//! fuzz that never reaches schema compilation. A length prefix fixes that, and
//! — being writable by hand — is what makes a committed seed corpus possible:
//!
//! ```text
//! byte 0                  : case selector — 0 = RAW family, non-zero = JSON family
//! bytes 1..5              : u32 little-endian schema_len
//! bytes 5..5+schema_len   : schema bytes
//! bytes 5+schema_len..    : instance bytes (the remainder)
//! input shorter than 5    : return immediately; no assertion is possible
//! schema_len > remaining  : clamped to the remainder, leaving an empty instance
//! ```
//!
//! The selector's one behavioural effect: on the JSON family the schema is ALSO
//! validated against ITSELF (a JSON Schema document is itself a JSON instance),
//! which doubles the semantic coverage of an input whose author declared both
//! halves to be JSON, and is pointless on raw garbage. Every invariant below
//! holds for both families.
//!
//! libFuzzer still mutates the JSON text freely, so plenty of near-valid JSON is
//! explored around each seed. See `fuzz/corpus/fuzz_schema_draft_pin/README.md`.
//!
//! # Invariants
//!
//! 1. **Totality.** `validate_bytes` and `normalize_bytes` return for ANY input.
//!    A panic inside emit-time validation is a remotely reachable unwind
//!    (T-115-24).
//! 2. **Normalization idempotence and surgical scope.** Normalizing twice equals
//!    normalizing once, and the normalized document differs from the input ONLY
//!    at string-valued `$schema` keys, at ANY depth (115-12 made the normalizer
//!    recursive; a ROOT-only strip here would read a legitimate nested rewrite
//!    as collateral damage). A normalizer that also dropped a sibling keyword
//!    would silently weaken every v2 validator while every behavioural test
//!    still passed.
//! 3. **Dialect-neutral era AGREEMENT.** When the schema's keyword set is drawn
//!    only from keywords whose meaning is IDENTICAL in draft-07 and 2020-12, the
//!    two eras must return the SAME verdict. Skipped when either verdict is
//!    `InvalidSchema` — that comparison would be about compilation, not
//!    semantics, and the skip is only expressible because the verdict is
//!    three-state. This is the generalized fence against the vacuous-validator
//!    bypass (T-115-01): a vacuous v2 validator returns `Conforms` where v1
//!    returns `Violates`, which the equality catches.
//! 4. **Documented, NOT asserted.** An external `$ref` must stay a compile error
//!    rather than a fetch. The structural fence for that is
//!    `tests/v2_schema_tripwires.rs` (`jsonschema` is declared
//!    `default-features = false` everywhere, so no resolver is compiled in);
//!    this target's only obligation is not to hang. Seed `08` keeps the case in
//!    the corpus.
//! 5. **Post-normalization dialect PURITY.** No string-valued `$schema` at any
//!    SCHEMA POSITION in the NORMALIZED document may be anything but the
//!    2020-12 URI. Added by 115-13 to close `115-VERIFICATION.md` `missing:`
//!    item 4: the shipped root-only normalizer left a legacy declaration alive
//!    on an `$id`-bearing EMBEDDED SCHEMA RESOURCE, which resolves an EMPTY
//!    vocabulary set there and produces an accept-everything sub-validator, and
//!    invariants 2 and 3 were both blind to it (2 stripped only the root, 3
//!    skipped every document containing a nested `$schema`).
//!
//!    **CORRECTION (115-15).** 115-13 documented this invariant as "TOTAL — no
//!    skip condition, no neutrality reasoning — so it holds for every input that
//!    parses as JSON" and its scan as "implemented INDEPENDENTLY" of the crate's
//!    detector. Both claims are false as written, and the correction is kept
//!    beside them because believing them is what let a defect ship a third time:
//!
//!    - The scan DOES have a skip condition —
//!      [`collect_dialect_declarations`] does not descend into a
//!      [`DATA_ONLY_KEYWORDS`] payload, because a `$schema` string inside a
//!      `const`/`enum`/`default`/`examples` value is instance DATA that must
//!      SURVIVE. The invariant is therefore total over SCHEMA POSITIONS, not
//!      over every input; the term is what invariant 6 turns on.
//!    - Independence in IMPLEMENTATION is not independence in RULE. This scan
//!      restates the SAME traversal rule as the code under test, so it catches a
//!      detector/rewriter DISAGREEMENT and cannot catch a defect in the rule they
//!      share. That was MEASURED: against the pre-115-14 body, `$defs.default`
//!      carrying `$id` + a draft-07 `$schema` was skipped by the rewriter AND by
//!      this scan AND by the crate's own detector — all three agreed there was
//!      nothing there, and all three were wrong (115-14-SUMMARY, "The
//!      postcondition passed VACUOUSLY").
//! 6. **Rename invariance — the instrument for a defect in the RULE** (115-15,
//!    `115-REVIEW.md` WR-02). Renaming an entry of a `properties` /
//!    `patternProperties` / `$defs` / `definitions` / `dependentSchemas` /
//!    `dependencies` map must
//!    not change how that entry normalizes. DERIVED from a JSON Schema 2020-12
//!    fact rather than restated from the crate's keyword lists: the keys of those
//!    maps are AUTHOR-CHOSEN NAMES with no keyword semantics under the core and
//!    applicator vocabularies, so normalizing an entry cannot depend on the name
//!    it is filed under. It consults no `DATA_ONLY_KEYWORDS` list at all, which
//!    is precisely what invariants 2 and 5 cannot say — and it fires on a FUTURE
//!    rule defect too, e.g. a sixth data-only keyword gained without the position
//!    exception. Exercised by seeds `14_defs_named_default` and
//!    `15_dependencies_named_default`, and OBSERVED to trip against a deliberately
//!    restored position-blind normalizer on each.
//!
//!    Its REACH, however, is NOT derived: the container selection reads this
//!    file's [`SUBSCHEMA_MAP_KEYWORDS`], so a keyword absent from that list is a
//!    container this invariant never probes. See its rustdoc for the two
//!    mechanisms that cover the residual, and for the measured configuration in
//!    which this whole file detects nothing.
//!
//! # Why invariant 3 is an EQUALITY and not a monotonicity claim
//!
//! A global cross-dialect MONOTONICITY assertion — "the 2020-12 pin never
//! accepts what v1 rejects", or its converse — would be FALSE, and the repo
//! already contains the counterexamples in both directions, MEASURED on
//! `jsonschema` 0.49.2:
//!
//! - `contentEncoding` is an ASSERTION in draft-07 and only an ANNOTATION from
//!   2019-09 onwards, so a draft-07-declared `{"contentEncoding": "base64"}`
//!   REJECTS a non-base64 string on v1 and ACCEPTS it on v2
//!   (`src/server/output_validation.rs`,
//!   `same_schema_text_yields_independent_verdicts_per_era_in_one_process`).
//! - `$ref` siblings are ignored in draft-07 and honoured under 2020-12, which
//!   makes v2 the stricter era for that document.
//!
//! (115-03-PLAN and 115-RESEARCH both name `dependencies` as the divergence
//! case. That is WRONG as measured: `jsonschema` 0.49.2 still honours
//! `dependencies` under the 2020-12 pin and both eras agree on it — see
//! D-115-03-C. `contentEncoding` is the real case.)
//!
//! So divergent keywords are EXCLUDED from the neutrality predicate rather than
//! asserted about, and the assertion over what remains is an equality.
//!
//! ## The dialect-neutral keyword allowlist
//!
//! `type`, `properties`, `required`, `enum`, `const`, `minimum`, `maximum`,
//! `minLength`, `maxLength`, `pattern`, `additionalProperties`, `minItems`,
//! `maxItems`, and — added by 115-13 — the three REFERENCE keywords `$defs`,
//! `$id` and `$ref`:
//!
//! - `$defs` — a container of subschemas under AUTHOR-CHOSEN names. Not a
//!   draft-07 keyword (draft-07 spells the container `definitions`), but an
//!   unrecognized keyword is IGNORED in draft-07 rather than reinterpreted, and
//!   a `#/$defs/...` JSON pointer resolves identically under both drafts. Its
//!   values are recursed into the way `properties`' values are; its KEYS are
//!   names, so they are never allowlist-checked.
//! - `$id` — a base-URI declaration in draft-06 onwards, identical in draft-07
//!   and 2020-12. It carries a string, so there is nothing to recurse into. It
//!   is what makes a subschema an EMBEDDED SCHEMA RESOURCE, and admitting it is
//!   what lets invariant 3 reach that shape at all.
//! - `$ref` — admitted **only when it is the SOLE key of its object.** A `$ref`
//!   with assertion siblings diverges: siblings are IGNORED in draft-07 and
//!   HONOURED under 2020-12, a divergence this repo already records as measured
//!   and reachable (see "Why invariant 3 is an EQUALITY" above). It carries a
//!   string; the guard is structural, on the containing object.
//!
//! ## Excluded, each with its reason
//!
//! - `dependencies` — split into `dependentRequired` / `dependentSchemas` in
//!   2020-12 (a spec-level divergence, even though the crate still honours the
//!   old spelling today).
//!
//!   **`dependencies` IS in [`SUBSCHEMA_MAP_KEYWORDS`] and is deliberately absent
//!   HERE, and those are DIFFERENT QUESTIONS.** Do not "harmonize" the two lists:
//!   doing so turns one of them into a defect. This allowlist asks *do both eras
//!   agree about what this keyword MEANS* — no, so invariant 3 SKIPS any document
//!   containing it, which is why seed `05_draft07_dependencies` exercises that
//!   skip. The subschema-map list asks *are this keyword's VALUES schema positions
//!   the normalizer must reach* — yes, because `D-115-03-C` measured that
//!   `jsonschema` 0.49.2 still honours `dependencies` under the 2020-12 pin, so a
//!   legacy declaration left alive in there is live. A keyword can be
//!   semantically divergent (excluded here) and still hold real schema positions
//!   (included there); `definitions` and `patternProperties` are in the same
//!   position for the same reason.
//! - `items` (array form) — replaced by `prefixItems`; the array form is a
//!   COMPILE ERROR under 2020-12.
//! - `exclusiveMinimum` / `exclusiveMaximum` (boolean form) — became numeric in
//!   draft-06.
//! - `definitions` — replaced by `$defs`.
//! - `$ref` alongside siblings — sibling keywords are ignored in draft-07 and
//!   honoured under 2020-12.
//! - `contentEncoding` — an assertion in draft-07, an annotation from 2019-09.
//! - `id` (vs `$id`) — the draft-04 spelling.
//!
//! ## And the DIALECT DECLARATION itself is restricted
//!
//! Neutrality also requires the ROOT `$schema` to be absent, draft-07, or
//! 2020-12, and forbids a NESTED `$schema` anywhere. draft-04 and draft-06 are
//! excluded because they change the meaning of an ALLOWLISTED keyword: under
//! draft-04 `{"type": "integer"}` rejects `1.0`, which draft-06 onwards accepts.
//! A nested `$schema` is excluded because it is a per-resource dialect switch
//! from 2019-09 onwards and merely ignored in draft-07.
//!
//! **That nested-`$schema` exclusion was NOT relaxed by 115-13, deliberately.**
//! After the recursive pin, a legacy declaration on an embedded resource makes
//! v2 STRICTER than v1 — v1's auto-detect still honours the per-resource switch
//! and drops the keywords under it, while v2 normalizes it away and enforces
//! them — which is a LEGITIMATE era divergence that invariant 3's EQUALITY would
//! misreport as a bug. Invariant 5 is what covers those documents instead: it is
//! structural, over the normalized document, and needs no neutrality reasoning.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::output_validation::fuzz_support::{
    normalize_bytes, validate_bytes, SchemaVerdict,
};
use serde_json::Value;

/// Selector byte plus the little-endian `u32` schema length.
const HEADER_LEN: usize = 5;

/// The selector value marking the RAW family: both halves are arbitrary bytes
/// with no claim that either is JSON.
const SELECTOR_RAW: u8 = 0;

/// Keywords whose meaning is IDENTICAL in draft-07 and 2020-12. See the module
/// docs for the exclusions and their reasons.
const DIALECT_NEUTRAL_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "enum",
    "const",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "additionalProperties",
    "minItems",
    "maxItems",
    // The three REFERENCE keywords, admitted by 115-13 so invariant 3 can reach
    // embedded-resource shapes. `$ref` additionally carries a SOLE-KEY guard in
    // `is_neutral_subschema`; see the module docs for all three reasons.
    "$defs",
    "$id",
    "$ref",
];

/// Keywords whose VALUE is instance DATA rather than a subschema.
///
/// Mirrors `DATA_ONLY_KEYWORDS` in `src/server/output_validation.rs`. The
/// shipped normalization walk never descends into these — a `$schema` string
/// inside a `const`/`enum`/`default`/`examples` payload is DATA, and rewriting
/// it would change which instances conform — so neither do the strip and the
/// scan below. Restated here rather than imported: the crate's copy is private,
/// and the independence is the point (see invariant 5 in the module docs).
const DATA_ONLY_KEYWORDS: &[&str] = &["const", "enum", "default", "examples"];

/// Keywords whose VALUE is a MAP from AUTHOR-CHOSEN NAMES to subschemas.
///
/// Mirrors `SUBSCHEMA_MAP_KEYWORDS` in `src/server/output_validation.rs`
/// (115-14, widened to six by 115-16). The mirror is REQUIRED, not cosmetic
/// parity, and this copy going STALE against the shipped rule has a directly
/// observable symptom: with the six-keyword walk shipped and this copy at five,
/// an input shaped
/// `{"dependencies": {"$schema": "http://json-schema.org/draft-07/schema#"}}` — a
/// `dependencies` entry NAMED `$schema` whose value is a string, i.e. a NAME bound
/// to a NON-schema — is correctly left alone by the shipped walk, while a
/// position-blind [`collect_dialect_declarations`] descends into the map as though
/// the map itself were a schema and reports the name-bound STRING to invariant 5
/// as a surviving legacy declaration. That is a FALSE POSITIVE that CRASHES THE
/// FUZZER ON CORRECT BEHAVIOUR, and it was OBSERVED on this tree before the list
/// was widened: exit 77 inside [`assert_no_legacy_dialect_survives`], with the
/// panic's `normalized to:` document byte-identical to its `Input was:` document —
/// the shipped normalizer had touched nothing, and the scan invented the finding.
///
/// **The STRIP half cannot false-positive, and earlier revisions of this rustdoc
/// claimed it could** (`115-REVIEW.md` WR-06). [`strip_dialect_declarations`] is
/// applied to BOTH sides of invariant 2's comparison; on such an input the shipped
/// walk leaves the document unchanged, so `input == once`, so ANY deterministic
/// strip — position-blind included — keeps the two clones equal and the assertion
/// cannot fire. Measured in the control above: invariant 2 runs FIRST and PASSED;
/// the panic came from invariant 5. The strip mirrors the shipped rule so the two
/// walks here stay readable as one rule, not because it could false-positive.
///
/// [`DATA_ONLY_KEYWORDS`] is a list of KEYWORDS and must never be tested against
/// the keys of these maps. That category error was the 115-14 defect: an
/// `$id`-bearing embedded resource filed under a `$defs` entry an author had
/// NAMED `default` survived the v2 pin. `dependencies` is the SAME defect one
/// keyword over (`115-REVIEW.md` CR-01), and seed `15_dependencies_named_default`
/// is its reproduction document.
///
/// # The list is a DERIVATION, not a patch (115-16)
///
/// It is the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09 / 2020-12
/// meta-schema documents `jsonschema` 0.49.2 ships offline, of the keywords each
/// meta-schema's own `.properties` map binds to an OBJECT-typed schema whose
/// `additionalProperties` REFERENCES THE META-SCHEMA ITSELF — `{"$ref":"#"}`
/// (draft-04/06/07), `{"$recursiveRef":"#"}` (2019-09), `{"$dynamicRef":"#meta"}`
/// (2020-12), or an `anyOf` carrying such a branch (`dependencies`). That shape is
/// precisely "a map whose keys are author-chosen and whose values are subschemas".
/// Two keywords match the object-with-`additionalProperties` shape and are
/// REJECTED by the self-reference criterion, and naming them is what makes the
/// enumeration checkable rather than asserted: `$vocabulary` (its values are
/// enablement booleans) and `dependentRequired` (its values are lists of property
/// names). The union is exactly these six. The full derivation, including the `jq`
/// command to re-run it, is on the shipped constant.
///
/// # Why this copy stays an INDEPENDENT literal
///
/// 115-16 published both keyword lists through the `fuzzing` seam as `pub const`
/// re-exports, and importing them here would make drift structurally impossible.
/// **Do not do it.** It would also make this target structurally unable to detect
/// the defect class it exists for, and that trade is MEASURED rather than argued:
///
/// - With this copy INDEPENDENT and CORRECT while the crate's list is missing an
///   entry, invariant 5's scan reaches a position the crate's walk skipped, sees
///   the surviving legacy declaration and FIRES. That is a real detection
///   capability, and it is MEASURED — 115-18 control D, the shipped list cut to
///   five and this one at six, on seed `15_dependencies_named_default`: exit 77
///   in [`assert_no_legacy_dialect_survives`], reporting
///   `["http://json-schema.org/draft-07/schema#"]` with `normalized to:`
///   byte-identical to `Input was:`.
/// - With this copy DERIVED from the crate, the scan skips exactly what the walk
///   skipped, the two agree, and the target exits 0 on the very document that
///   reproduces the defect. The fuzzer becomes blind to every keyword-list
///   omission BY CONSTRUCTION. This is not hypothetical: 115-17 measured the
///   equivalent mistake one layer up in `tests/property_tests.rs`, where sourcing
///   the container DRAW from the gated mirror made all three of its negative
///   controls report `21 passed` (`D-115-AI(4)`). A gate makes a copy CORRECT; it
///   does not make a fence able to FIRE.
///
/// The cost of independence is silent DRIFT, and drift is what 115-19's
/// source-text gate closes: it reads this file, `tests/property_tests.rs` and
/// `src/server/output_validation.rs` as TEXT and asserts the three literals are
/// identical AND ORDERED — which is why `dependencies` is appended LAST here, as
/// it is in `src/`. That gate matters more here than anywhere else in the
/// repository: ledger `D-115-AB` records that the workspace `exclude` array means
/// `make quality-gate` formats, lints, builds and runs NOTHING under `fuzz/`, so
/// this copy has no other safety net.
///
/// # The LIMIT of this target
///
/// When this copy and the crate SHARE an omission, every invariant in this file
/// passes and the target exits 0 on the reproduction document: the strip is
/// symmetric (invariant 2 passes), the scan skips exactly what the walk skipped
/// (invariant 5 passes VACUOUSLY), [`assert_normalization_is_invariant_under_rename`]
/// never selects the container because its selection reads THIS list, and
/// invariant 3 skips the document because `dependencies` is not dialect-neutral.
///
/// **MEASURED** — 115-18 control F, both copies cut to five, on seed
/// `15_dependencies_named_default`: **exit 0**, 1 input executed in 9 ms, no
/// invariant reached. The seed that reproduces CR-01 is waved straight through.
///
/// **So a green fuzz run is NOT evidence that a keyword-list omission is absent.**
/// The primary instrument for a LIST omission is `src`'s own fence,
/// `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`,
/// which carries its OWN container literal for exactly this reason. That
/// attribution is measured too, not asserted: in the same both-blind tree the
/// fence FAILED at `src/server/output_validation.rs:1429` while this whole file
/// stayed green. The secondary mechanism is 115-19's source-text drift gate over
/// the three copies, which is what would catch the two lists being shortened
/// together in the first place.
const SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
    "dependencies", // draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME (D-115-03-C)
];

/// The name invariant 6 renames a subschema-map entry to.
///
/// Long and `__`-delimited so it cannot plausibly collide with a name a real
/// input (or a libFuzzer mutation of one) already uses; invariant 6 skips the
/// input outright if it does.
const RENAME_PROBE_NAME: &str = "__rename_probe__";

/// The Draft 2020-12 meta-schema URI the v2 pin rewrites every declaration to.
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Root `$schema` declarations under which every keyword in
/// [`DIALECT_NEUTRAL_KEYWORDS`] means the same thing. draft-04 and draft-06 are
/// deliberately absent — see the module docs.
const NEUTRAL_DIALECTS: &[&str] = &[
    "http://json-schema.org/draft-07/schema#",
    "https://json-schema.org/draft/2020-12/schema",
];

/// Whether the two eras are REQUIRED to agree about `schema`.
///
/// True only when the root dialect declaration is one this predicate can reason
/// about AND every keyword at every level is dialect-neutral.
fn is_dialect_neutral(schema: &Value) -> bool {
    match schema.get("$schema") {
        None => {},
        Some(Value::String(uri)) if NEUTRAL_DIALECTS.contains(&uri.as_str()) => {},
        // draft-04/06, a non-string, an unknown URI: not a dialect whose
        // keyword semantics this predicate has established.
        Some(_) => return false,
    }
    is_neutral_subschema(schema, true)
}

/// The recursive half of [`is_dialect_neutral`].
///
/// `is_root` exists because a `$schema` at the root is the document's dialect
/// declaration (already checked by the caller), while a NESTED one is a
/// per-resource dialect switch from 2019-09 onwards and simply ignored in
/// draft-07 — a divergence, so it makes the document non-neutral.
fn is_neutral_subschema(schema: &Value, is_root: bool) -> bool {
    let Some(object) = schema.as_object() else {
        // A boolean schema (`true` / `false`) means the same in every draft.
        // Any other scalar is not a schema at all and fails to compile on both
        // eras, which invariant 3 skips anyway.
        return schema.is_boolean();
    };
    // `$ref` is dialect-neutral ONLY as the SOLE key of its object. Sibling
    // keywords alongside a `$ref` are IGNORED in draft-07 and HONOURED under
    // 2020-12 — a measured divergence in the v2-is-stricter direction, which an
    // EQUALITY invariant would misreport as a bug.
    if object.contains_key("$ref") && object.len() > 1 {
        return false;
    }
    for (key, value) in object {
        if key == "$schema" {
            if !is_root {
                return false;
            }
            continue;
        }
        if !DIALECT_NEUTRAL_KEYWORDS.contains(&key.as_str()) {
            return false;
        }
        let nested_is_neutral = match key.as_str() {
            // `properties` and `$defs` map AUTHOR-CHOSEN NAMES to subschemas, so
            // their keys are not keywords and must not be allowlist-checked;
            // only their values are schemas.
            //
            // DO NOT "FIX" THIS TO MATCH THE OTHER WALKERS. This predicate was
            // ALREADY position-aware when the two walkers below were not — the
            // same distinction, written two hundred lines away and never applied
            // on the crate's side (see `SUBSCHEMA_MAP_KEYWORDS` in
            // `src/server/output_validation.rs`). It is why
            // `115-VERIFICATION.md` measured invariant 3 as correctly SKIPPING
            // the defective document instead of misreporting it. The narrower
            // pair here is deliberate: this is a NEUTRALITY allowlist, and
            // `patternProperties` / `definitions` / `dependentSchemas` /
            // `dependencies` are absent from `DIALECT_NEUTRAL_KEYWORDS` on
            // purpose, so they can never reach this match.
            //
            // `dependencies` joined that list in 115-18 and is the sharpest case:
            // it IS one of the six `SUBSCHEMA_MAP_KEYWORDS` and is NOT
            // dialect-neutral, which is not a contradiction because the two lists
            // answer different questions — "do both eras agree about this
            // keyword's MEANING" (no: 2020-12 split it into `dependentRequired` /
            // `dependentSchemas`, so invariant 3 must skip such documents) versus
            // "are this keyword's VALUES schema positions the normalizer must
            // reach" (yes: `D-115-03-C` measured `jsonschema` 0.49.2 still
            // honouring it under the pin). Widening this allowlist to "match" the
            // other one would make invariant 3 assert an equality across a real
            // spec divergence.
            "properties" | "$defs" => value
                .as_object()
                .is_some_and(|map| map.values().all(|v| is_neutral_subschema(v, false))),
            // A schema (or a boolean) in its own right.
            "additionalProperties" => is_neutral_subschema(value, false),
            // `type`, `required`, `enum`, `const`, `$id`, `$ref` and the numeric
            // / string / array bounds carry DATA or a string, not subschemas —
            // nothing to recurse into.
            _ => true,
        };
        if !nested_is_neutral {
            return false;
        }
    }
    true
}

/// Remove every string-valued `$schema` at every SCHEMA POSITION, skipping the
/// values of [`DATA_ONLY_KEYWORDS`] in KEYWORD position only — the traversal
/// rule 115-12 shipped, corrected to the position rule 115-14 shipped.
fn strip_dialect_declarations(node: &mut Value) {
    match node {
        Value::Object(map) => {
            if map.get("$schema").is_some_and(Value::is_string) {
                map.remove("$schema");
            }
            for (key, value) in map.iter_mut() {
                strip_dialect_declarations_in_member(key, value);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(strip_dialect_declarations),
        _ => {},
    }
}

/// The three-way MEMBER dispatch of the stripper, mirroring
/// `pin_dialect_in_member` in `src/server/output_validation.rs`. See
/// [`SUBSCHEMA_MAP_KEYWORDS`] for why the mirror is required.
fn strip_dialect_declarations_in_member(member_key: &str, member_value: &mut Value) {
    if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
        // NAME position: descend into every value, never keyword-filter the
        // map's own keys. A non-object value is a malformed document and falls
        // through to the ordinary walk, so no coverage is lost relative to the
        // position-blind version.
        match member_value {
            Value::Object(named_subschemas) => {
                named_subschemas
                    .values_mut()
                    .for_each(strip_dialect_declarations);
            },
            malformed => strip_dialect_declarations(malformed),
        }
    } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
        strip_dialect_declarations(member_value);
    }
}

/// Every string-valued `$schema` at every SCHEMA POSITION, under the same
/// position-aware rule as [`strip_dialect_declarations`].
///
/// Implemented in a different TYPE from the crate's own `first_legacy_dialect`
/// (which is private and invisible here anyway), but NOT in a different RULE —
/// see the 115-15 correction on invariant 5 in the module docs. A separate walk
/// catches a detector/rewriter DISAGREEMENT; only
/// [`assert_normalization_is_invariant_under_rename`] catches a defect in the
/// rule all three copies share.
fn collect_dialect_declarations<'a>(node: &'a Value, out: &mut Vec<&'a str>) {
    match node {
        Value::Object(map) => {
            if let Some(declared) = map.get("$schema").and_then(Value::as_str) {
                out.push(declared);
            }
            for (key, value) in map {
                collect_dialect_declarations_in_member(key, value, out);
            }
        },
        Value::Array(items) => {
            for item in items {
                collect_dialect_declarations(item, out);
            }
        },
        _ => {},
    }
}

/// The three-way MEMBER dispatch of the scan, mirroring
/// `first_legacy_dialect_in_member` in `src/server/output_validation.rs`.
fn collect_dialect_declarations_in_member<'a>(
    member_key: &str,
    member_value: &'a Value,
    out: &mut Vec<&'a str>,
) {
    if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) {
        match member_value {
            Value::Object(named_subschemas) => {
                for subschema in named_subschemas.values() {
                    collect_dialect_declarations(subschema, out);
                }
            },
            malformed => collect_dialect_declarations(malformed, out),
        }
    } else if !DATA_ONLY_KEYWORDS.contains(&member_key) {
        collect_dialect_declarations(member_value, out);
    }
}

/// Invariant 2, over one parsed schema.
fn assert_normalization_is_idempotent_and_surgical(schema_bytes: &[u8]) {
    let Some((input, once, twice)) = normalize_bytes(schema_bytes) else {
        return;
    };

    assert_eq!(
        once, twice,
        "normalize_schema_dialect is NOT idempotent: a second pass changed the document. \
         The v2 pin normalizes before compiling and the cache is keyed by schema TEXT, so a \
         non-idempotent rewrite means the same declaration can compile to two different \
         validators. Input was: {input}"
    );

    // RECURSIVE, not root-only: 115-12 made the normalizer rewrite every
    // string-valued `$schema` at any depth, so stripping only the root here
    // would read a legitimate NESTED rewrite as collateral damage and fire this
    // assertion on correct behaviour.
    let mut stripped_input = input.clone();
    let mut stripped_once = once.clone();
    strip_dialect_declarations(&mut stripped_input);
    strip_dialect_declarations(&mut stripped_once);
    assert_eq!(
        stripped_input, stripped_once,
        "normalization touched a key other than a string-valued $schema. Dropping or \
         rewriting a sibling keyword silently WEAKENS every v2 validator while behavioural \
         tests keep passing. Input was: {input}, normalized to: {once}"
    );
}

/// Invariant 5, over one parsed schema: NO legacy dialect declaration survives
/// normalization, anywhere in the document.
///
/// Total over SCHEMA POSITIONS under the traversal rule the shipped walk
/// implements — **NOT over every input**. A `$schema` inside a `const` / `enum` /
/// `default` / `examples` payload is instance DATA and must SURVIVE, so
/// [`collect_dialect_declarations`] deliberately does not descend there and this
/// assertion says nothing about such a document. See the 115-15 CORRECTION on
/// invariant 5 in the module docs for the full statement and for why the
/// independence claim beside it was retracted too.
///
/// **History, amended rather than deleted (`115-REVIEW.md` WR-03).** Through
/// 115-17 this rustdoc and the call-site comment in the `fuzz_target!` body both
/// asserted the invariant was total with no skip condition, holding for every
/// input that parses as JSON — three hundred lines from the module-doc correction
/// that had already labelled that claim false. `{"const":{"$schema":"…draft-07…"}}`
/// parses as JSON and is deliberately outside the invariant. Both copies were
/// corrected in 115-18; the retracted wording is quoted in `115-REVIEW.md` WR-03
/// rather than here, so a grep for it finds the review and not a live claim.
///
/// What survives of the original point is narrower and still true: this invariant
/// is a SEPARATE invariant rather than a relaxation of `is_dialect_neutral`,
/// because it also covers the documents that predicate excludes — a nested
/// `$schema` makes a document non-neutral, and those are precisely the
/// embedded-resource shapes this assertion exists for.
///
/// `normalize_bytes` is called ONCE here, at entry, and the `fuzz_target!` body
/// does not call it again on this input's behalf — the same discipline the
/// module docs record for invariants 1-3. Normalization is a pure walk plus a
/// clone on the rewrite path; it compiles nothing, so this second helper entry
/// costs a traversal, not a validator build.
fn assert_no_legacy_dialect_survives(schema_bytes: &[u8]) {
    let Some((input, once, _twice)) = normalize_bytes(schema_bytes) else {
        return;
    };

    let mut surviving = Vec::new();
    collect_dialect_declarations(&once, &mut surviving);
    let legacy: Vec<&&str> = surviving
        .iter()
        .filter(|declared| **declared != DRAFT_2020_12)
        .collect();
    assert!(
        legacy.is_empty(),
        "A LEGACY $schema SURVIVED NORMALIZATION: {legacy:?}. Under Draft 2020-12 a `$schema` \
         is legal at the root of any EMBEDDED SCHEMA RESOURCE (a subschema carrying `$id`) and \
         `jsonschema` honours it there, resolving an EMPTY vocabulary set on that resource and \
         producing a sub-validator that accepts EVERYTHING — the vacuous-validator bypass the \
         v2 pin exists to close, moved one level down. `115-VERIFICATION.md` measured it as \
         `root-draft07 + embedded (v1,v2) = (Violates, Conforms)`, v2 WEAKER than v1. \
         `normalize_schema_dialect` must rewrite EVERY declaration at EVERY depth, not just \
         the root one. Input was: {input}, normalized to: {once}"
    );
}

/// Invariant 6, over one parsed schema: normalizing an entry of a subschema map
/// must not depend on the NAME it is filed under.
///
/// # Why this is the only invariant here that a defect in the RULE cannot satisfy
///
/// Invariants 2 and 5 RESTATE the shipped traversal rule (in a different type,
/// but the same rule). Two copies of one rule can only disagree with each other;
/// when the rule itself is wrong they AGREE, and the assertion passes vacuously —
/// measured against the pre-115-14 body, where the rewriter, this file's scan and
/// the crate's own detector all skipped a `$defs` entry named `default` and all
/// three were wrong.
///
/// This invariant is DERIVED instead, from a JSON Schema 2020-12 fact: the keys
/// of `properties`, `patternProperties`, `$defs`, `definitions`,
/// `dependentSchemas` and `dependencies` are AUTHOR-CHOSEN NAMES with no keyword
/// semantics under the core and applicator vocabularies. Therefore normalizing an
/// entry cannot depend on the name it is filed under, and two documents differing
/// ONLY in that name must produce equal normalized subtrees. It consults no
/// [`DATA_ONLY_KEYWORDS`] list at all, so it also fires on a FUTURE rule defect
/// that special-cases some other name.
///
/// # Its REACH is not derived, and that is the remaining limit
///
/// The invariant is derived from a spec fact; the set of positions it VISITS is
/// not. The container selection below reads this file's [`SUBSCHEMA_MAP_KEYWORDS`],
/// so a keyword missing from that list is a container this invariant never probes
/// — it does not fail, it never runs. That is `115-REVIEW.md` CR-01's *"the one
/// instrument the round advertises as derived … is in fact gated by a
/// crate-derived list one line earlier"*, and it is why 115-18's control F
/// (both copies blind) exits 0 on the reproduction document.
///
/// Two mechanisms cover that residual, and neither of them is in this file:
/// 115-19's source-text drift gate, which asserts the three restated literals are
/// identical and ordered; and `src`'s own fence
/// `v2_pin_rewrites_an_embedded_resource_in_every_spec_defined_subschema_map`,
/// which carries its OWN container literal so an omission FROM the shipped list is
/// visible to it. Naming them here makes this a pointer rather than a complaint.
///
/// # Is it really the ONLY one? Checked, not assumed
///
/// Invariant 3's [`is_dialect_neutral`] is also independent of the crate's keyword
/// lists — it was position-aware before either walker was — so it is a candidate
/// second derived fence. It is NOT one for this defect class, structurally: a
/// nested `$schema` makes a document non-neutral, and every reproduction document
/// here carries one, so invariant 3 SKIPS them by design (see "That nested-`$schema`
/// exclusion was NOT relaxed" in the module docs). `dependencies` is additionally
/// absent from [`DIALECT_NEUTRAL_KEYWORDS`], which skips CR-01's document a second
/// time over. The check is worth recording because the equivalent taxonomy in
/// `tests/property_tests.rs` claimed one derived fence and 115-17 MEASURED two
/// (`D-115-AI(5)`): there, the embedded-resource POINTER assertion also fires
/// without consulting a keyword list. This file has no such assertion, so the
/// count here really is one — for that reason, not by inheritance.
///
/// # Cost, and the discipline recorded for invariants 1-3
///
/// This helper makes its OWN `normalize_bytes` calls, on two small probe
/// documents it constructs — not on the fuzzer's input. That is unavoidable
/// (there is no other way to normalize the renamed variant) and it is cheap:
/// normalization COMPILES NOTHING; it is a walk plus a clone on the rewrite path.
/// The module docs' exec/s note is about repeat VALIDATION — a second
/// `validate_bytes` on the deliberately UNCACHED seam builds a second validator —
/// and repeat traversal is not that. The work is bounded to the ROOT-LEVEL
/// subschema maps: every entry of each is probed once, the entries' subtrees are
/// disjoint, and nested containers are not descended into — so the total is
/// linear in the document, the same order as invariant 5's scan.
///
/// 115-15-PLAN specified "the FIRST container and its FIRST entry" instead. That
/// was MEASURED to be blind to this phase's own reproduction document: in seed
/// `14_defs_named_default` the first root-level subschema map is `properties`
/// and its first entry is `n` (a plain `$ref` holder carrying no `$schema`), so
/// the interesting entry — `$defs.default` — was never probed and this invariant
/// PASSED against a fully position-blind normalizer. A fence that cannot fire on
/// the case it exists for is the exact failure mode this plan closes, so the
/// selection was widened. See `D-115-AF`.
///
/// The probe documents' `$ref`s (if the subtree contains any) will dangle.
/// That is a legitimate input: normalization never resolves `$ref`s, and nothing
/// here compiles the probe.
fn assert_normalization_is_invariant_under_rename(schema_bytes: &[u8]) {
    let Ok(schema) = serde_json::from_slice::<Value>(schema_bytes) else {
        return;
    };
    let Some(root) = schema.as_object() else {
        return;
    };

    // Every entry of every ROOT-LEVEL subschema map. Bounded: each entry's
    // subtree is probed once, the subtrees are disjoint, and normalization is a
    // walk plus a clone — so the total stays linear in the document, the same
    // order as invariant 5's scan. Nested containers are deliberately NOT
    // descended into; that would re-walk the same bytes once per level.
    for (container, container_value) in root {
        if !SUBSCHEMA_MAP_KEYWORDS.contains(&container.as_str()) {
            continue;
        }
        let Some(named_subschemas) = container_value.as_object() else {
            continue;
        };
        for (original_name, subtree) in named_subschemas {
            assert_entry_normalizes_the_same_under_any_name(container, original_name, subtree);
        }
    }
}

/// One entry of one subschema map, normalized under its own name and under
/// [`RENAME_PROBE_NAME`], asserted equal. The per-entry half of invariant 6.
fn assert_entry_normalizes_the_same_under_any_name(
    container: &str,
    original_name: &str,
    subtree: &Value,
) {
    if original_name == RENAME_PROBE_NAME {
        return;
    }

    let probe_document = |entry_name: &str| {
        let mut named = serde_json::Map::new();
        named.insert(entry_name.to_string(), subtree.clone());
        let mut document = serde_json::Map::new();
        document.insert(container.to_string(), Value::Object(named));
        serde_json::to_vec(&Value::Object(document)).ok()
    };
    let (Some(original_bytes), Some(renamed_bytes)) = (
        probe_document(original_name),
        probe_document(RENAME_PROBE_NAME),
    ) else {
        return;
    };
    let (Some((_, original_once, _)), Some((_, renamed_once, _))) = (
        normalize_bytes(&original_bytes),
        normalize_bytes(&renamed_bytes),
    ) else {
        return;
    };

    let original_subtree = original_once
        .get(container)
        .and_then(|map| map.get(original_name));
    let renamed_subtree = renamed_once
        .get(container)
        .and_then(|map| map.get(RENAME_PROBE_NAME));
    assert_eq!(
        original_subtree, renamed_subtree,
        "RENAME INVARIANCE VIOLATED. The keys of properties / patternProperties / $defs / \
         definitions / dependentSchemas / dependencies are AUTHOR-CHOSEN NAMES with no keyword \
         semantics under the JSON Schema 2020-12 core and applicator vocabularies, so \
         normalizing an entry \
         CANNOT depend on the name it is filed under. A difference here means the traversal is \
         treating a NAME as a KEYWORD — the `115-VERIFICATION.md` defect class, measured as \
         `$defs.default -> verdicts=(Conforms, Conforms), rewritten=false` against the control \
         `$defs.Inner -> (Conforms, Violates), rewritten=true`. A legacy declaration that \
         survives on an `$id`-bearing embedded resource resolves an EMPTY vocabulary set there \
         and produces a sub-validator that accepts EVERYTHING. This invariant is DERIVED from \
         the spec rather than restated from the crate's keyword lists, which is precisely what \
         invariants 2 and 5 are not — a defect in the rule they share satisfies both of them. \
         container: {container}, name: {original_name}, normalized under the original name: \
         {original_once}, under the probe: {renamed_once}"
    );
}

/// Invariant 3, over one schema/instance pair.
fn assert_dialect_neutral_eras_agree(schema_bytes: &[u8], instance_bytes: &[u8]) {
    let Some((v1, v2)) = validate_bytes(schema_bytes, instance_bytes) else {
        return;
    };
    // The skip that only a THREE-state verdict can express: comparing eras
    // across a compile failure compares COMPILATION, not semantics.
    if v1 == SchemaVerdict::InvalidSchema || v2 == SchemaVerdict::InvalidSchema {
        return;
    }
    let Ok(schema) = serde_json::from_slice::<Value>(schema_bytes) else {
        return;
    };
    if !is_dialect_neutral(&schema) {
        return;
    }

    // NOTE: this is an EQUALITY, deliberately. A cross-dialect MONOTONICITY
    // assertion — `!(v2 == Conforms && v1 == Violates)` — would be FALSE by this
    // phase's own design and would fire on the repo's OWN tests: a
    // draft-07-declared `{"contentEncoding": "base64"}` violates on v1 and
    // conforms on v2, because `contentEncoding` is an assertion in draft-07 and
    // only an annotation from 2019-09 (`output_validation.rs`,
    // `same_schema_text_yields_independent_verdicts_per_era_in_one_process`).
    // The converse direction is reachable too, via `$ref` siblings. Divergent
    // keywords are therefore EXCLUDED by `is_dialect_neutral` rather than
    // asserted about — and equality over what remains still catches the bypass
    // this target exists for, because a vacuous v2 validator returns `Conforms`
    // where v1 returns `Violates` on a neutral schema.
    assert_eq!(
        v1,
        v2,
        "DIALECT-NEUTRAL ERA DISAGREEMENT. This schema uses only keywords whose meaning is \
         identical in draft-07 and 2020-12, so both eras must reach the same verdict. The \
         usual cause is the vacuous-validator bypass: a legacy `$schema` declaration compiled \
         under the 2020-12 pin WITHOUT the normalization step yields an empty vocabulary set, \
         producing a validator that accepts everything — v2 says `Conforms` where v1 says \
         `Violates`. Restore the normalize-then-pin step in `compile_2020_12`. \
         schema: {}, instance: {}",
        String::from_utf8_lossy(schema_bytes),
        String::from_utf8_lossy(instance_bytes)
    );
}

fuzz_target!(|data: &[u8]| {
    if data.len() < HEADER_LEN {
        return;
    }
    let selector = data[0];
    let declared_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let body = &data[HEADER_LEN..];
    // Clamp rather than reject: a truncated input is still a legitimate case,
    // and rejecting it would throw away every mutation that shortened the body.
    let schema_len = declared_len.min(body.len());
    let (schema_bytes, instance_bytes) = body.split_at(schema_len);

    // Invariants 1 + 2. Invariant 1 is totality: both seam entry points must
    // RETURN for any bytes, and a panic inside either fails the target. It needs
    // no separate discarding call — `assert_normalization_is_idempotent_and_surgical`
    // calls `normalize_bytes` as its first statement, before any early return,
    // and `assert_dialect_neutral_eras_agree` does the same with `validate_bytes`.
    // Calling them here as well compiled the same schema twice per invariant,
    // which halved exec/s on the deliberately UNCACHED fuzz seam.
    assert_normalization_is_idempotent_and_surgical(schema_bytes);

    // Invariant 5. Total over SCHEMA POSITIONS under the traversal rule the
    // shipped walk implements — NOT over every input: a `$schema` inside a
    // `const` / `enum` / `default` / `examples` payload is instance DATA and must
    // survive, so the scan does not descend there. (This comment asserted the
    // unqualified version through 115-17; see the 115-15 correction on invariant 5
    // in the module docs, and `115-REVIEW.md` WR-03.) It still covers the
    // documents invariant 3 skips, which is why it exists as a separate invariant
    // rather than as a relaxed neutrality predicate. Its own single
    // `normalize_bytes` call lives at its entry.
    assert_no_legacy_dialect_survives(schema_bytes);

    // Invariant 6. The only fence in this file DERIVED from a spec fact rather
    // than restated from the crate's traversal rule, and therefore the only one
    // a defect in that rule cannot satisfy. It normalizes two probe documents it
    // builds itself; see its rustdoc for why that does not violate the exec/s
    // discipline recorded above.
    assert_normalization_is_invariant_under_rename(schema_bytes);

    // Invariants 1 + 3.
    assert_dialect_neutral_eras_agree(schema_bytes, instance_bytes);

    // The JSON family declares both halves to be JSON, so the schema document is
    // itself a usable instance — free extra semantic coverage from one input.
    if selector != SELECTOR_RAW {
        assert_dialect_neutral_eras_agree(schema_bytes, schema_bytes);
    }
});
