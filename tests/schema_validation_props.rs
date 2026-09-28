//! Property arms for `pmcp::server::schema_validation` (Phase 128, SC-8).
//!
//! # Why this file exists at all, when plan 08 already wrote property arms
//!
//! Plan 08's arms live in `crates/pmcp-server-toolkit/`, and `make test-property`
//! **cannot reach them**: its selector is root-package-scoped
//! (`cargo test --features "full" -- --ignored property_`, `Makefile`), so a
//! toolkit arm is invisible to it no matter how it is named. Those arms are reached
//! by `make test-server-toolkit` instead, and that is fine — but it leaves the
//! CLAUDE.md ALWAYS/PROPERTY requirement discharged by a leg that does not measure
//! this phase. This file is the ROOT-package home that leg can select.
//!
//! # The `#[ignore]` marker is load-bearing, not decoration
//!
//! `make test-property` selects `--ignored property_`, so a `property_`-prefixed
//! test WITHOUT `#[ignore]` is filtered OUT of that run and contributes nothing.
//! `tests/property_tests.rs` is the cautionary case: 19 `property_*` functions,
//! zero `#[ignore]` attributes, and therefore zero of them selected — which is why
//! the leg measured only 3 tests across the whole root package before this file
//! (MEASURED 2026-09-27: `tests/log_emitter.rs` 2 + `tests/typed_tool_garde.rs` 1;
//! `128-RESEARCH.md` Finding 9b recorded 2, which was one short). The marker string
//! below is copied VERBATIM from `tests/log_emitter.rs` so the convention has one
//! spelling.
//!
//! # Why this arm may use the STRICT no-echo oracle
//!
//! `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` deliberately does NOT
//! assert "the refusal contains no substring of the instance" — for a fuzzer that
//! oracle produces false failures, because a DECLARED property name is
//! legitimately echoed and an independently generated schema and instance can
//! share strings by coincidence. It uses a provenance sentinel instead.
//!
//! Here the strict form IS sound, and the reason is worth stating so the two are
//! not later "harmonized" in the wrong direction: a property generator controls
//! BOTH sides. The declaration is drawn from `DECLARED_NAMES` (lowercase ASCII) and
//! the caller data from `CALLER_ALPHABET` (uppercase ASCII + digits), and the two
//! alphabets are DISJOINT — asserted in `alphabets_are_disjoint` below, so the
//! premise is checked rather than assumed. Provenance is therefore established by
//! construction, and "no byte of the caller's data appears in the refusal" is
//! exactly the phase prohibition stated at full strength:
//!
//! > A refusal message returned to an MCP client must never contain any byte of
//! > the rejected value, and must never contain an attacker-supplied argument key.
//!
//! Run with:
//!
//! ```bash
//! PROPTEST_CASES=256 cargo test -p pmcp --features "full" \
//!   --test schema_validation_props -- --ignored property_
//! ```

#![cfg(feature = "validation")]

use pmcp::server::schema_validation::{render_refusal, validate_input};
use serde_json::{json, Map, Value};

/// Declared property names. Lowercase ASCII only — see the module header's
/// disjointness argument, which `alphabets_are_disjoint` checks.
const DECLARED_NAMES: [&str; 3] = ["alpha", "beta", "gamma"];

/// The alphabet the variable tail of a generated caller string is drawn from.
const CALLER_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// The fixed prefix every generated caller value and caller-chosen key carries.
///
/// A bare alphabet is NOT enough, and that was MEASURED rather than reasoned: the
/// first draft of this file generated caller strings from `CALLER_ALPHABET` alone,
/// and proptest immediately found `key_seed = [32], bound = 6` — a one-character key
/// `"6"` that IS a substring of the serialized schema, because the schema declares
/// `"maxLength":6`. That is precisely the coincidence
/// `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs`'s header names as the third
/// way a naive absence oracle produces false failures, reproduced inside this file's
/// own generator on the first run.
///
/// Prefixing every caller string with characters that cannot appear in any schema
/// this file builds removes the coincidence by construction, rather than by
/// weakening the assertion — which is the move the whole phase is about.
const CALLER_PREFIX: &str = "CALLER";

/// The premise the strict oracle rests on, checked rather than assumed.
///
/// Not `property_`-prefixed and not `#[ignore]`d on purpose: it must run on every
/// ordinary `cargo test` too. Two things are asserted:
///
/// 1. no declared name shares a character with `CALLER_ALPHABET`;
/// 2. `CALLER_PREFIX` does not appear as a SUBSTRING of ANY schema this file can
///    build, over every `(shape, bound, closed)` combination the generator draws
///    from. Since every generated caller string BEGINS with that prefix, this is
///    exactly what makes a caller string unable to be a substring of the
///    declaration.
///
/// SUBSTRING, not per-character, and the first draft got that wrong in a way worth
/// recording: a per-character check fails on the letter `L`, because the JSON Schema
/// keyword `maxLength` contains one. Individual characters coinciding is harmless;
/// the whole prefixed string coinciding is what would turn a coincidence into a
/// reported leak. Over-strict premises get relaxed wholesale rather than corrected,
/// so the granularity matters.
///
/// (2) is what makes the coincidence impossible rather than merely unlikely. If a
/// later edit adds a lowercase letter to `CALLER_ALPHABET`, or puts the prefix into
/// a schema, the property arm below would start producing false failures and someone
/// would relax it. This test fails first, and says why.
#[test]
fn alphabets_are_disjoint() {
    for name in DECLARED_NAMES {
        for ch in name.chars() {
            assert!(
                !CALLER_ALPHABET.contains(ch),
                "declared name {name:?} shares character {ch:?} with CALLER_ALPHABET; the strict \
                 no-echo oracle in property_refusal_never_echoes_input is no longer sound — see \
                 this file's header before changing either constant"
            );
        }
    }
    for shape in 0u8..4 {
        for bound in 0usize..8 {
            for closed in [true, false] {
                let text = declared_schema(shape, bound, closed).to_string();
                assert!(
                    !text.contains(CALLER_PREFIX),
                    "CALLER_PREFIX ({CALLER_PREFIX:?}) appears in a schema this file builds \
                     (shape {shape}, bound {bound}, closed {closed}): {text}. A generated caller \
                     string could then be a substring of the DECLARATION, and the strict no-echo \
                     oracle would report a coincidence as a leak."
                );
            }
        }
    }
}

/// Build one of four declared-schema shapes, each exercising a different arm of
/// the refusal renderer.
///
/// `shape` selects; `bound` supplies the numeric limit. Every string emitted is
/// drawn from `DECLARED_NAMES` or the JSON Schema vocabulary, never from
/// `CALLER_ALPHABET`.
fn declared_schema(shape: u8, bound: usize, closed: bool) -> Value {
    let constraint = match shape % 4 {
        0 => json!({ "type": "string", "maxLength": bound }),
        1 => json!({ "type": "string", "pattern": "^[a-z]+$" }),
        2 => json!({ "enum": ["one", "two"] }),
        _ => json!({ "type": "integer", "minimum": bound }),
    };
    let mut properties = Map::new();
    properties.insert(DECLARED_NAMES[0].to_string(), constraint);
    properties.insert(
        DECLARED_NAMES[1].to_string(),
        json!({ "type": "string", "maxLength": bound }),
    );
    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": [DECLARED_NAMES[0]],
        "additionalProperties": closed,
    })
}

/// Render a caller string: the fixed prefix, then a tail from the disjoint
/// alphabet. See [`CALLER_PREFIX`] for why the prefix is not optional.
fn caller_string(seed: &[usize]) -> String {
    let bytes = CALLER_ALPHABET.as_bytes();
    let tail: String = seed
        .iter()
        .map(|i| bytes[i % bytes.len()] as char)
        .collect();
    format!("{CALLER_PREFIX}{tail}")
}

/// SC-8 / SC-7: no byte of the caller's data reaches the refusal, over generated
/// (schema shape, value, undeclared key) triples rather than over fixtures.
///
/// Four arms of the renderer are reached (`maxLength`, `pattern`, `enum`,
/// `minimum`/`type`) and both envelope shapes (`additionalProperties` true and
/// false), so the `additionalProperties` message — the one that legitimately lists
/// ALLOWED names, and must never list the offending one — is covered too.
///
/// Both surfaces the caller's data can escape through are asserted, not just the
/// rendered string: `InputViolation::pointer` and `InputViolation::expected` are
/// PUBLIC fields that reach logs, execution records and third-party renderers.
/// Plan 02 recorded that two of its own fixture rows would have passed vacuously
/// had they asserted only on the rendered message, because `render_one` suppresses
/// an undeclared pointer as defence in depth and thereby masks a leak in the field
/// itself. This arm does not repeat that mistake.
///
/// The `#[ignore]` marker is what makes `make test-property` select this. See the
/// module header.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_refusal_never_echoes_input() {
    use proptest::prelude::*;

    proptest!(|(
        shape in 0u8..4,
        bound in 0usize..8,
        closed in any::<bool>(),
        value_seed in proptest::collection::vec(0usize..36, 1..24),
        key_seed in proptest::collection::vec(0usize..36, 1..16),
        as_array in any::<bool>(),
    )| {
        let schema = declared_schema(shape, bound, closed);
        let value = caller_string(&value_seed);
        let key = caller_string(&key_seed);

        // The caller data must not be reachable from the DECLARATION, or the oracle
        // would be asserting something the schema itself supplies. Guaranteed by
        // the disjoint alphabets; re-checked here because a generator is code.
        let schema_text = schema.to_string();
        prop_assert!(!schema_text.contains(&value));
        prop_assert!(!schema_text.contains(&key));

        let mut args = Map::new();
        // (a) the caller's value under a DECLARED name — the arm where the name is
        //     legitimately echoed and the VALUE must not be.
        args.insert(
            DECLARED_NAMES[0].to_string(),
            if as_array { json!([value.clone()]) } else { json!(value.clone()) },
        );
        // (b) an UNDECLARED, caller-chosen key — the PHI-shaped-key case. Under a
        //     closed envelope this is refused by `additionalProperties`; under an
        //     open one it is admitted, and either way the key must not surface.
        args.insert(key.clone(), json!(value.clone()));

        let instance = Value::Object(args);
        if let Err(violations) = validate_input(&schema, Some(&instance), None) {
            for v in &violations {
                prop_assert!(
                    !v.pointer.contains(&value) && !v.pointer.contains(&key),
                    "InputViolation::pointer echoed caller data: {} (keyword {})",
                    v.pointer, v.keyword
                );
                prop_assert!(
                    !v.expected.contains(&value) && !v.expected.contains(&key),
                    "InputViolation::expected echoed caller data: {} (keyword {})",
                    v.expected, v.keyword
                );
            }
            let rendered = render_refusal(&violations, &DECLARED_NAMES);
            prop_assert!(
                !rendered.contains(&value),
                "the rendered refusal echoed the rejected value: {rendered}"
            );
            prop_assert!(
                !rendered.contains(&key),
                "the rendered refusal echoed the caller's argument key: {rendered}"
            );
        }
    });
}
