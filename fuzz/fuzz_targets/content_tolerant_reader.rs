#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::types::Content;
use serde_json::{from_slice, from_value, json, to_value, Value};

// Phase 118.1-02 (CONF-04, D-03): the `Content` deserialization surface, which
// plan 118.1-03 turns into a TOLERANT READER — accepting both the spec's nested
// `EmbeddedResource` (`{"type":"resource","resource":{…}}`) and pmcp's legacy flat
// shape (`{"type":"resource","uri":…}`) while emitting only the nested one.
//
// # This target is DELIBERATELY RED until 118.1-03 lands
//
// Invariants 2 and 3 below cannot hold on the unfixed tree: `Content` has only a
// derived `Deserialize` with `#[serde(tag = "type")]` and a REQUIRED top-level
// `uri`, so a spec-conformant nested embedded resource from any other SDK's
// server fails to parse at all, and everything pmcp does parse it re-emits flat.
// The invariants are written ACTIVE, not commented out and not feature-gated, so
// the target is the RED evidence for the fix rather than a fence that only starts
// measuring after the thing it measures is already correct.
//
// Neither the CI `fuzz.yml` matrix (a fixed four-target list) nor `make test-fuzz`
// (which swallows a non-zero exit behind `|| echo`) can be broken by that redness;
// plan 118.1-03 runs the real `-max_total_time=300` campaign after the fix, per
// the CLAUDE.md ALWAYS-fuzz requirement.
//
// # Why this exists rather than a generic serde fuzz
//
// Invariant 1 alone (no panic) is what any serde round-trip target gives you. The
// value here is invariants 2 and 3: a hand-written tolerant reader is exactly the
// kind of code that accepts an input and then emits something it cannot read
// back, or that converges the two accepted shapes onto DIFFERENT outputs. Both
// are silent, both are wire-visible, and neither is reachable by a type-level
// check. Fuzzing supplies the adversarial `uri` / `mimeType` / `text` strings
// (unicode, control characters, embedded quotes, empty, enormous) that a
// hand-written fixture never thinks to try.

/// Pull a string field off the fuzzer's own JSON, or fall back to a fixed value.
///
/// This is what makes the shapes in [`assert_shapes_converge`] WELL-FORMED by
/// construction while still being fuzzer-driven in content: asserting that
/// arbitrary fuzzer JSON must parse would report every malformed input as a
/// counterexample and drown the two real invariants in noise.
fn string_at(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

/// INVARIANT 2 (FIXED POINT), first half: anything pmcp accepts, pmcp must be
/// able to read back, and the second emission must equal the first.
///
/// An emitter that produces a shape its own reader rejects is the defect that
/// D-03 exists to prevent from being reintroduced in the other direction.
fn assert_serialization_is_a_fixed_point(content: &Content) {
    let once = to_value(content).expect("Content always serializes");
    let round = match from_value::<Content>(once.clone()) {
        Ok(round) => round,
        Err(error) => panic!(
            "INVARIANT 2 (FIXED POINT) violated: pmcp EMITTED {once} and then refused to \
             read its own output back: {error}"
        ),
    };
    let twice = to_value(&round).expect("Content always serializes");
    assert_eq!(
        once, twice,
        "INVARIANT 2 (FIXED POINT) violated: serializing a re-read value produced \
         different bytes, so serialize-then-deserialize is not idempotent"
    );
}

/// INVARIANT 2 (FIXED POINT), second half, and INVARIANT 3 (FLAT ACCEPTANCE).
///
/// Builds the SAME resource in both shapes and asserts:
///
/// * the flat legacy shape parses — the D-03 compatibility affordance, so every
///   pmcp server and every recorded payload written before the fix keeps working;
/// * the nested spec shape parses — the client-side half of the defect, the one
///   the official suite could not catch because it tests pmcp as a SERVER;
/// * the flat shape re-emits as NESTED, with a `resource` object and no top-level
///   `uri` — strict emitter, one shape on the wire;
/// * both shapes converge on byte-identical output — "nested in, nested out" and
///   "flat in, nested out" must not disagree, which is the property that makes
///   the tolerance an affordance rather than a second supported format.
fn assert_shapes_converge(input: &Value) {
    let uri = string_at(input, "uri", "fuzz://resource");
    let mime = string_at(input, "mimeType", "text/plain");
    let text = string_at(input, "text", "");

    let flat = json!({ "type": "resource", "uri": uri, "mimeType": mime, "text": text });
    let nested = json!({
        "type": "resource",
        "resource": { "uri": uri, "mimeType": mime, "text": text }
    });

    let from_flat = match from_value::<Content>(flat.clone()) {
        Ok(content) => content,
        Err(error) => panic!(
            "INVARIANT 3 (FLAT ACCEPTANCE) violated: the legacy flat resource shape must \
             still parse under the D-03 tolerant reader, got {error} for {flat}"
        ),
    };
    let from_nested = match from_value::<Content>(nested.clone()) {
        Ok(content) => content,
        Err(error) => panic!(
            "INVARIANT 2 (FIXED POINT, nested in) violated: the SPEC shape \
             `EmbeddedResource` must parse — a client that cannot read a conformant \
             embedded resource cannot talk to any other SDK's server. Got {error} for \
             {nested}"
        ),
    };

    let flat_out = to_value(&from_flat).expect("Content always serializes");
    let nested_out = to_value(&from_nested).expect("Content always serializes");

    assert!(
        flat_out.get("resource").and_then(Value::as_object).is_some(),
        "INVARIANT 3 (FLAT ACCEPTANCE) violated: a flat input must be re-emitted NESTED, \
         with the contents under a `resource` object. Emitted: {flat_out}"
    );
    assert!(
        flat_out.get("uri").is_none(),
        "INVARIANT 3 (FLAT ACCEPTANCE) violated: the emitted shape still carries a \
         TOP-LEVEL `uri`, which is the flat legacy shape the strict emitter must not \
         produce. Emitted: {flat_out}"
    );
    assert_eq!(
        flat_out, nested_out,
        "INVARIANT 2 (FIXED POINT) violated: the two ACCEPTED input shapes converged on \
         two DIFFERENT emitted shapes, so the tolerance is a second wire format rather \
         than a compatibility affordance"
    );
}

fuzz_target!(|data: &[u8]| {
    let Ok(json) = from_slice::<Value>(data) else {
        return;
    };

    // INVARIANT 1 — NO PANIC on arbitrary JSON. `from_value::<Content>` may only
    // return `Ok` or `Err`; the tolerant reader added by 118.1-03 is a new
    // untrusted parse surface (pmcp acting as a CLIENT reads whatever a server
    // sends), so an unwind here is a remotely reachable denial of service
    // (T-118.1-02-02).
    let parsed = from_value::<Content>(json.clone());

    // INVARIANT 2 — FIXED POINT, on whatever the reader actually accepted.
    if let Ok(content) = parsed {
        assert_serialization_is_a_fixed_point(&content);
    }

    // INVARIANT 2 (nested in) + INVARIANT 3 — on both canonical shapes, built
    // from the fuzzer's own strings.
    assert_shapes_converge(&json);
});
