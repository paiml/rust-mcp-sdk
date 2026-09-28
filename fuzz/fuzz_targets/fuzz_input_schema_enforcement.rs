//! Fuzz target for the `tools/call` INPUT-schema enforcement path and its
//! value-free refusal renderer (Phase 128, SC-7 / T-128-47).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//!
//! ```bash
//! cd fuzz && cargo +nightly fuzz run fuzz_input_schema_enforcement
//! ```
//!
//! **`+nightly` is REQUIRED and is not a style choice.** `cargo fuzz` passes
//! `-Zsanitizer=address`, which stable rustc refuses with "the option `Z` is
//! only accepted on the nightly compiler" — the build fails before a single
//! iteration runs. The repo's `make test-fuzz` invokes the PLAIN form and pipes
//! every non-zero exit into `|| echo "… completed"`, so on a stable default
//! toolchain that target reports success having fuzzed NOTHING. **Do not cite
//! `make test-fuzz` as evidence for this target.** `make test-fuzz-strict` is the
//! leg that runs it with `+nightly` and PROPAGATES a crash; that is the one to
//! cite, and after any run check that
//! `fuzz/artifacts/fuzz_input_schema_enforcement/` is empty.
//!
//! # What is being fuzzed, and why
//!
//! `src/server/schema_validation.rs` is the ONE input-validation entry point in
//! the SDK. `pmcp-server-toolkit` calls `validate_input` on every `tools/call` to
//! a config-synthesized tool, before the backend call, and renders any violation
//! with `render_refusal` — a string that goes back to the MCP client.
//!
//! Callers of this module handle PHI. The phase prohibition is absolute:
//!
//! > A refusal message returned to an MCP client must never contain any byte of
//! > the rejected value, and must never contain an attacker-supplied argument key.
//!
//! Roughly a dozen fixtures assert that today, one keyword at a time. A fixture
//! set survives a renderer edit only by luck; an invariant over arbitrary bytes
//! survives it by construction. That is what this target is for.
//!
//! # The oracle is PROVENANCE-based, not absence-based — do not "simplify" it
//!
//! The obvious oracle ("the refusal contains no substring of the instance") is
//! WRONG and produces false failures. Three ways, all measured during planning:
//!
//! - a DECLARED property name is intentionally echoed. `{"alpha": "<too long>"}`
//!   against `maxLength: 3` produces `/alpha: must be at most 3 characters` —
//!   naming the declared property is exactly what SC-7 requires, and the name is
//!   also a substring of the instance's key set;
//! - the `additionalProperties: false` message intentionally lists the ALLOWED
//!   names, which are declared names;
//! - an independently generated schema and instance can contain identical strings
//!   by coincidence, and a fuzzer explores precisely that space.
//!
//! A target that cries wolf gets its assertion relaxed until it asserts nothing,
//! which is the failure mode this phase exists to repair (T-128-49b). So the
//! oracle is built on PROVENANCE instead:
//!
//! 1. derive a per-case SENTINEL from the input's own bytes, shaped so that no
//!    schema this target can build is able to contain it (`Zq7PHI` + 16 hex
//!    digits; every declared name, keyword and pattern in the template table is
//!    lowercase ASCII or JSON Schema vocabulary);
//! 2. VERIFY the sentinel is absent from the serialized schema before asserting
//!    anything, and SKIP the case if it is present — a collision is a generator
//!    artifact, not a defect;
//! 3. place the sentinel in the instance as a VALUE under a declared name, as an
//!    UNDECLARED KEY (the PHI-shaped-key case, `{"Jane Doe DOB 1970-01-01": 1}`),
//!    or both;
//! 4. assert the sentinel appears NOWHERE in `render_refusal`'s output, nor in
//!    any `InputViolation`'s `pointer` or `expected` — on BYTES, not on `char`
//!    boundaries.
//!
//! Because the sentinel can only arrive from caller-supplied data, a declared
//! name can never trip it and the oracle needs no permitted-substring allowlist.
//! In particular it does NOT need to special-case `safe_pointer`'s redaction
//! token (`<redacted>`, `src/server/schema_validation.rs`): the token cannot
//! contain the sentinel, so redacting is indistinguishable from never having the
//! key — which is the property under test.
//!
//! Both envelope shapes are generated on purpose. A key matched only by
//! `additionalProperties` or `patternProperties` MUST NOT reach a refusal; that is
//! what `safe_pointer`'s projection buys, and this target is its durable guard.
//! A key in the schema's `properties` MAY appear, and does.
//!
//! # Why the schema side is a BOUNDED projection, and what it costs
//!
//! `validate_input` memoizes compiled validators in a process-global
//! `Mutex<HashMap<String, …>>` keyed on schema TEXT. A fuzzer produces a fresh
//! schema text on nearly every iteration, so driving arbitrary schemas through it
//! would grow that map for the whole process lifetime and turn a correctness
//! fuzzer into a memory-exhaustion one. A target that OOMs is indistinguishable
//! from a target that found nothing. `src/server/output_validation.rs` records
//! this exact hazard on `compile_for_era`, which was split out of
//! `cached_validator` precisely so `fuzz_schema_draft_pin` has an uncached path.
//!
//! `schema_validation` has no equivalent `fuzzing`-gated seam, and this plan was
//! directed to land with a ZERO-line diff on that file (plan 02 owns it; its 52
//! unit tests are the fence, and plans 05, 06, 08 and 09 each met their goals
//! without touching it). So the hazard is closed from this side instead, two ways:
//!
//! - the schema reaching `validate_input` is a bounded PROJECTION of the schema
//!   document — `project_schema` reads the document's own declared keywords and
//!   rebuilds from a fixed template table, so the number of distinct schema texts
//!   this target can ever produce is
//!   `ENVELOPES(4) × CONSTRAINTS(8) × REQUIRED_K(5) × BOUND(9) × DECLARED_K(4)`
//!   = **5760**, a hard cap on cache entries rather than an unbounded map;
//! - the *arbitrary* schema document still drives `check_input_schema_compiles`,
//!   which is PUBLIC and calls `compile_input_2020_12` directly — the UNCACHED
//!   compile path. So arbitrary-schema coverage of compilation is retained at zero
//!   cache cost.
//!
//! The residual, stated so it is not discovered later as a surprise: the
//! *instance* side is fully arbitrary but the *schema* side is not, so a defect
//! reachable only from a schema shape outside the template table is out of this
//! target's reach. Adding the `fuzzing`-gated uncached seam to
//! `schema_validation` (in the shape of `output_validation::fuzz_support`) is the
//! durable fix and is handed to whichever plan is permitted to touch that file.
//!
//! # Input layout
//!
//! Splitting raw bytes at an arbitrary point makes BOTH halves fail to parse as
//! JSON on nearly every iteration, which degenerates this into a JSON-parser fuzz
//! that never reaches validation at all. A length prefix fixes that, and — being
//! writable by hand — is what makes a committed seed corpus possible. Same layout
//! as `fuzz_schema_draft_pin.rs`, deliberately, so the crate has ONE convention:
//!
//! ```text
//! byte 0                  : selector — sentinel placement + knob fallbacks
//! bytes 1..5              : u32 little-endian schema_len
//! bytes 5..5+schema_len   : schema document bytes
//! bytes 5+schema_len..    : instance document bytes (the remainder)
//! input shorter than 5    : return immediately; no assertion is possible
//! schema_len > remaining  : clamped to the remainder, leaving an empty instance
//! ```
//!
//! libFuzzer still mutates the JSON text freely, so plenty of near-valid JSON is
//! explored around each seed. See `fuzz/corpus/fuzz_input_schema_enforcement/README.md`.
//!
//! # Invariants
//!
//! 1. **Totality of the UNCACHED config-time gate.** `check_input_schema_compiles`
//!    returns for ANY schema document that parses as JSON. It runs at server build
//!    time over author- or spec-supplied text, so a panic there is a startup crash
//!    on a hostile bundled `OpenAPI` spec.
//! 2. **SC-7 no-echo, the load-bearing one.** See the oracle section above.
//! 3. **Gate/runtime AGREEMENT.** When `check_input_schema_compiles` accepts a
//!    schema, `validate_input` must not then refuse every call to it with the
//!    `keyword: "schema"` uncompilable refusal. Those are two different entry
//!    points onto one compile (`compile_input_2020_12`); if they ever disagree, the
//!    config-time gate stops predicting runtime behaviour and SC-2's whole value —
//!    "a drifted declaration is caught before deploy" — evaporates silently.
//! 4. **Totality of `validate_input` + `render_refusal`.** Neither panics on any
//!    (projected schema, arbitrary instance) pair. This path runs inside request
//!    handling, so a panic is a remotely reachable unwind.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::schema_validation::{
    check_input_schema_compiles, render_refusal, validate_input,
};
use serde_json::{json, Map, Value};

/// The declared property names. Lowercase ASCII, so none can contain a sentinel.
const DECLARED: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

/// The declared `pattern` vocabulary. Deliberately includes the permissive
/// `^.*$`, which is the shape that makes a declared narrowing vacuous.
const PATTERNS: [&str; 3] = ["^[a-z]+$", "^v[0-9]+$", "^.*$"];

/// The sentinel prefix. Chosen so that no template string, declared name, JSON
/// Schema keyword or pattern above can contain it.
const SENTINEL_PREFIX: &str = "Zq7PHI";

/// Upper bound on distinct schema texts — see the module header's cache argument.
/// Not used at runtime; stated as a constant so the bound is checkable by reading
/// the code rather than by trusting the prose.
#[allow(dead_code)]
const SCHEMA_SPACE: usize = 4 * 8 * 5 * 9 * 4;

/// The bounded knob set `project_schema` builds a schema from.
struct Knobs {
    /// 0 closed, 1 open subschema, 2 `patternProperties` + closed, 3 open pattern.
    envelope: u8,
    /// Which constraint `DECLARED[0]` carries; 0..=7.
    constraint: u8,
    /// How many of the declared names are `required`; 0..=4.
    required_k: usize,
    /// The numeric bound a `maxLength` / `minimum` / `maxItems` uses; 0..=8.
    bound: u64,
    /// How many of `DECLARED` are declared at all; 1..=4.
    declared_k: usize,
}

/// FNV-1a over the whole input, so the sentinel varies per case while staying
/// outside the shape any generated schema can hold.
fn sentinel(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{SENTINEL_PREFIX}{h:016x}")
}

/// Byte-wise substring search. Deliberately not `str::contains`, which is also
/// byte-wise today but carries no such guarantee in its contract — and a `char`-
/// boundary-aware search would miss a sentinel spliced mid-codepoint.
fn contains_bytes(haystack: &str, needle: &str) -> bool {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    !n.is_empty() && n.len() <= h.len() && h.windows(n.len()).any(|w| w == n)
}

/// Read the first integer among the bound-shaped keywords, at the top level or one
/// level into `properties`. Clamped to 0..=8.
fn read_bound(doc: &Value) -> Option<u64> {
    const KEYS: [&str; 3] = ["maxLength", "minimum", "maxItems"];
    let mut scan = |v: &Value| -> Option<u64> {
        KEYS.iter()
            .find_map(|k| v.get(*k).and_then(Value::as_u64))
            .map(|n| n.min(8))
    };
    scan(doc).or_else(|| {
        doc.get("properties")
            .and_then(Value::as_object)
            .and_then(|m| m.values().find_map(&mut scan))
    })
}

/// Which constraint the document's FIRST declared property asks for. Read in a
/// fixed keyword order so the mapping is deterministic.
fn read_constraint(doc: &Value) -> Option<u8> {
    let first = doc
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|m| m.values().next())?;
    if first.get("maxLength").is_some() {
        return Some(1);
    }
    if first.get("pattern").is_some() {
        return Some(2);
    }
    if first.get("enum").is_some() {
        return Some(3);
    }
    if first.get("minimum").is_some() {
        return Some(5);
    }
    if first.get("format").is_some() {
        return Some(6);
    }
    match first.get("type").and_then(Value::as_str) {
        Some("integer") => Some(4),
        Some("array") => Some(7),
        _ => Some(0),
    }
}

/// Which envelope the document asks for.
fn read_envelope(doc: &Value) -> u8 {
    let has_pattern_props = doc.get("patternProperties").is_some();
    let closed = matches!(doc.get("additionalProperties"), Some(Value::Bool(false)) | None);
    match (has_pattern_props, closed) {
        (true, true) => 2,
        (true, false) => 3,
        (false, true) => 0,
        (false, false) => 1,
    }
}

/// Derive the bounded knob set from the schema DOCUMENT, falling back to selector
/// bits for whatever the document does not determine. The document's own declared
/// keywords are what choose the envelope and the constraint, which is what makes a
/// hand-written corpus file exercising `additionalProperties` / `maxLength` /
/// `pattern` / `required` actually exercise them.
fn knobs(doc: &Value, selector: u8) -> Knobs {
    let s = usize::from(selector);
    Knobs {
        envelope: read_envelope(doc),
        constraint: read_constraint(doc).unwrap_or((selector >> 2) & 0x07),
        required_k: doc
            .get("required")
            .and_then(Value::as_array)
            .map_or((s >> 5) & 0x03, |a| a.len().min(4)),
        bound: read_bound(doc).unwrap_or(u64::from(selector & 0x07)),
        declared_k: doc
            .get("properties")
            .and_then(Value::as_object)
            .map_or(((s >> 3) & 0x03) + 1, |m| m.len().clamp(1, 4)),
    }
}

/// Build the constraint subschema for `DECLARED[0]`.
fn constraint_schema(k: &Knobs) -> Value {
    match k.constraint {
        1 => json!({ "type": "string", "maxLength": k.bound }),
        2 => json!({ "type": "string", "pattern": PATTERNS[(k.bound as usize) % 3] }),
        3 => json!({ "enum": ["one", "two"] }),
        4 => json!({ "type": "integer" }),
        5 => json!({ "type": "integer", "minimum": k.bound }),
        6 => json!({ "type": "string", "format": "uuid" }),
        7 => json!({
            "type": "array",
            "maxItems": k.bound,
            "items": { "type": "string", "maxLength": k.bound }
        }),
        _ => json!({ "type": "string" }),
    }
}

/// Rebuild a schema from the bounded knob set. Every string this emits is drawn
/// from `DECLARED`, `PATTERNS` or the JSON Schema vocabulary, which is what makes
/// the sentinel unreachable from the declaration side.
fn project_schema(k: &Knobs) -> Value {
    let mut properties = Map::new();
    properties.insert(DECLARED[0].to_string(), constraint_schema(k));
    for name in DECLARED.iter().take(k.declared_k).skip(1) {
        properties.insert((*name).to_string(), json!({ "type": "string" }));
    }
    let required: Vec<&str> = DECLARED.iter().take(k.required_k).copied().collect();

    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("required".to_string(), json!(required));
    match k.envelope {
        1 => {
            schema.insert("additionalProperties".to_string(), json!({"type":"integer"}));
        },
        2 => {
            schema.insert("patternProperties".to_string(), json!({"^p": {"type":"integer"}}));
            schema.insert("additionalProperties".to_string(), json!(false));
        },
        3 => {
            schema.insert(
                "patternProperties".to_string(),
                json!({"^.*$": {"type":"integer"}}),
            );
        },
        _ => {
            schema.insert("additionalProperties".to_string(), json!(false));
        },
    }
    Value::Object(schema)
}

/// Place the sentinel into the instance. `mode` picks the provenance under test:
/// a DECLARED name's value, an UNDECLARED key, or both.
fn with_sentinel(instance: &Value, sent: &str, mode: u8) -> Value {
    let mut obj = instance.as_object().cloned().unwrap_or_default();
    match mode % 3 {
        0 => {
            obj.insert(DECLARED[0].to_string(), json!(sent));
        },
        1 => {
            obj.insert(sent.to_string(), instance.clone());
        },
        _ => {
            obj.insert(DECLARED[0].to_string(), json!(sent));
            obj.insert(sent.to_string(), json!([sent, { "k": sent }]));
        },
    }
    Value::Object(obj)
}

/// Invariant 2 + 4: run one enforcement and assert nothing caller-supplied echoes.
fn assert_no_echo(schema: &Value, instance: &Value, sent: &str, declared: &[&str]) {
    if let Err(violations) = validate_input(schema, Some(instance), None) {
        for v in &violations {
            assert!(
                !contains_bytes(&v.pointer, sent),
                "SC-7 VIOLATION: InputViolation::pointer echoed caller data.\n  pointer = {}\n  keyword = {}\n  schema  = {schema}",
                v.pointer,
                v.keyword
            );
            assert!(
                !contains_bytes(&v.expected, sent),
                "SC-7 VIOLATION: InputViolation::expected echoed caller data.\n  expected = {}\n  keyword  = {}\n  schema   = {schema}",
                v.expected,
                v.keyword
            );
        }
        let rendered = render_refusal(&violations, declared);
        assert!(
            !contains_bytes(&rendered, sent),
            "SC-7 VIOLATION: the rendered refusal echoed caller data.\n  refusal = {rendered}\n  schema  = {schema}"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let selector = data[0];
    let declared_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let rest = &data[5..];
    let split = declared_len.min(rest.len());
    let (schema_bytes, instance_bytes) = rest.split_at(split);

    let Ok(schema_doc) = serde_json::from_slice::<Value>(schema_bytes) else {
        return;
    };

    // Invariant 1 — the arbitrary document drives the UNCACHED config-time gate.
    let gate_accepted = check_input_schema_compiles(&schema_doc).is_ok();

    let k = knobs(&schema_doc, selector);
    let schema = project_schema(&k);
    let schema_text = schema.to_string();
    let sent = sentinel(data);

    // Step 2 of the oracle: a sentinel present in the DECLARATION would make the
    // assertion meaningless, so skip rather than fail. Structurally impossible
    // given the template table; checked anyway, because "impossible by
    // construction" is a claim about code that can be edited.
    if contains_bytes(&schema_text, &sent) {
        return;
    }

    // Invariant 3 — the two entry points onto one compile must agree.
    if check_input_schema_compiles(&schema).is_ok() {
        if let Err(violations) = validate_input(&schema, Some(&json!({})), None) {
            assert!(
                !violations.iter().any(|v| v.keyword == "schema"),
                "gate/runtime DISAGREEMENT: check_input_schema_compiles accepted a schema that \
                 validate_input reports as uncompilable.\n  schema = {schema_text}"
            );
        }
    }

    let declared: Vec<&str> = DECLARED.iter().take(k.declared_k).copied().collect();
    let instance = serde_json::from_slice::<Value>(instance_bytes).unwrap_or(Value::Null);

    // Invariant 4 — totality over the raw arbitrary instance, no sentinel.
    assert_no_echo(&schema, &instance, &sent, &declared);

    // Invariant 2 — all three provenances, on every iteration, so a placement is
    // never left unexplored because the selector byte happened not to pick it.
    for mode in 0..3u8 {
        let carried = with_sentinel(&instance, &sent, mode);
        assert_no_echo(&schema, &carried, &sent, &declared);
    }

    // Keep the gate's verdict observable to the coverage-guided mutator rather
    // than dead — without this the whole invariant-1 call can be optimized toward
    // a single branch.
    if gate_accepted {
        let _ = check_input_schema_compiles(&schema_doc);
    }
});
