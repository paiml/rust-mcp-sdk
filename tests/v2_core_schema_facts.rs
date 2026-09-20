//! Re-derivation tripwire for the `2026-07-28` core schema facts Phase 115 is built on.
//!
//! # What this file is for
//!
//! Phase 115 implements four wire behaviours — `ttlMs`, `cacheScope`,
//! `CallToolResult.structuredContent` and `Tool.outputSchema` — whose *expected*
//! values came from reading the published MCP core schema. A value read once and
//! then written into Rust is a **decaying finding**: nothing notices when the
//! artifact it came from changes.
//!
//! So this file re-derives every one of those values, at test time, from the
//! bytes vendored at `schema/vendored/core-2026-07-28/`. A re-vendoring that
//! changes a fact breaks a test here instead of silently invalidating the
//! implementation.
//!
//! It is the direct answer to the failure mode the phase's research names as
//! "treating a network summary as verified": nothing below restates a summary,
//! and every number is read out of the artifact at runtime.
//!
//! # This file asserts CONTENT; `vendored_schema_provenance.rs` asserts ATTRIBUTION
//!
//! The two are complements and neither substitutes for the other:
//!
//! - `tests/vendored_schema_provenance.rs` proves the vendored bytes are the
//!   bytes that were fetched from a named upstream commit. It never looks at
//!   what they say.
//! - This file proves what those bytes *say*. It assumes, and does not
//!   re-check, that they are faithfully attributed.
//!
//! # The artifact is read at RUNTIME, never embedded at compile time
//!
//! The standard string-embedding macro bakes the bytes into the test binary
//! when it is BUILT, so a re-vendoring would leave a stale copy asserting stale
//! facts until something forced a rebuild. `read_to_string` at runtime is what
//! makes a re-vendoring move these assertions immediately — the same reasoning
//! `tests/v2_tasks_tripwires.rs` records for the `ext-tasks` tree. That macro's
//! name appears nowhere in this file, including in comments, so the property is
//! greppable.
//!
//! # The pointer is `$defs`
//!
//! The generated JSON Schema's top-level keys are exactly `["$defs", "$schema"]`.
//! The resolvable pointer is `/$defs/CacheableResult`. The older draft-04
//! spelling does not resolve in this artifact, and an assertion written against
//! it fails on a perfectly correct file — which is exactly what happened to the
//! pre-review version of this phase's plan set. That spelling appears nowhere in
//! this file, including in comments, on purpose.
//!
//! # The scanner primitives are DELIBERATELY duplicated
//!
//! A Rust integration test is its own crate, so this file cannot import
//! `tests/v2_tasks_tripwires.rs`'s `repo_root` / `rel` / `read` and that file
//! cannot import this one. The primitives below are therefore RESTATED rather
//! than shared, and the idiom is kept identical on purpose so the repository has
//! one source-scanning shape rather than three.
//!
//! # Naming
//!
//! Every test function is prefixed with this file's stem so BOTH
//! `-E 'binary(v2_core_schema_facts)'` and `-E 'test(/v2_core_schema_facts/)'`
//! select it. The bare-name form of a nextest selector silently matches zero
//! tests and exits 0, so the prefix is a correctness property, not a style.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The vendored core schema tree. Named in every failure message so a reader
/// knows which artifact to re-fetch.
const VENDORED_TREE: &str = "schema/vendored/core-2026-07-28";

/// The TypeScript protocol source, read at RUNTIME.
const SCHEMA_TS: &str = "schema/vendored/core-2026-07-28/schema.ts";

/// The generated JSON Schema, read at RUNTIME. This is the artifact a peer
/// actually validates against, so where the two disagree this one is the wire.
const SCHEMA_JSON: &str = "schema/vendored/core-2026-07-28/schema.json";

/// The base type carrying both caching hints.
const CACHEABLE_RESULT: &str = "CacheableResult";

/// The one pointer every assertion in this file hangs off.
const CACHEABLE_RESULT_POINTER: &str = "/$defs/CacheableResult";

/// The MEASURED required-key set of `CacheableResult` — three entries, not two.
///
/// `resultType` belongs to the same base and is already implemented: Phase 114's
/// `inject_v2_result_envelope` injects it. Nothing in Phase 115 adds it, which is
/// why a reader easily assumes it is not there.
const EXPECTED_CACHEABLE_KEYS: [&str; 3] = ["cacheScope", "resultType", "ttlMs"];

/// The MEASURED set of result types that extend [`CACHEABLE_RESULT`].
///
/// SIX, not the five the phase requirement text names — `DiscoverResult` is the
/// measured sixth.
const EXPECTED_EXTENDERS: [&str; 6] = [
    "DiscoverResult",
    "ListPromptsResult",
    "ListResourceTemplatesResult",
    "ListResourcesResult",
    "ListToolsResult",
    "ReadResourceResult",
];

/// The one remedy every failure in this file points at.
///
/// Stated once and appended everywhere, because the wrong remedy — editing the
/// assertion until it matches the new artifact — is always available, always
/// faster, and always destroys the only thing this file does.
const REMEDY: &str = "WHAT TO DO: re-run the `## Change protocol` in \
                      schema/vendored/core-2026-07-28/PROVENANCE.md, then re-derive the Rust side \
                      from the new artifact. Do NOT edit this assertion to match — an assertion \
                      edited to fit records nothing and detects nothing thereafter.";

// ===========================================================================
// 1. Primitives — restated, not shared. See the module docs.
// ===========================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// One vendored file's text, read at RUNTIME.
fn read(path: &str) -> String {
    let full = repo_root().join(path);
    fs::read_to_string(&full).unwrap_or_else(|err| {
        panic!(
            "cannot read the vendored core schema at {}: {err}\n\n\
             Every wire fact Phase 115 implements is re-derived from that file. Without it these \
             assertions cannot run at all, and the values in src/ become unreviewable claims.\n\n\
             {REMEDY}",
            rel(&full)
        )
    })
}

/// The parsed generated JSON Schema.
fn schema_json() -> Value {
    let text = read(SCHEMA_JSON);
    serde_json::from_str(&text).unwrap_or_else(|err| {
        panic!(
            "{SCHEMA_JSON} is not valid JSON: {err}\n\n\
             It is a byte-for-byte copy of an upstream generated schema, so invalid JSON means it \
             was edited or truncated after the fetch.\n\n\
             {REMEDY}"
        )
    })
}

/// One `$defs` entry, or a NAMED failure rather than an unwrap panic.
///
/// The resolved pointer is `/$defs/{name}`.
fn schema_entry<'a>(schema: &'a Value, name: &str) -> &'a Value {
    let pointer = format!("/$defs/{name}");
    schema.pointer(&pointer).unwrap_or_else(|| {
        panic!(
            "the vendored schema no longer defines {name} at {pointer}\n\n\
             Artifact: {VENDORED_TREE}\n\n\
             A missing pointer is not a test bug: it means the upstream shape this phase was \
             built against changed name, moved, or was removed. The top-level keys of that file \
             are `$schema` and `$defs`; if a re-vendoring changed that structure, every \
             assertion in this file needs re-deriving, not repointing.\n\n\
             {REMEDY}"
        )
    })
}

/// One entry's `required` array, sorted, or a named failure.
fn required_of(entry: &Value, name: &str) -> Vec<String> {
    let array = entry
        .get("required")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "/$defs/{name} has no `required` array in {VENDORED_TREE}\n\n\
                 A result type that requires nothing cannot carry a required caching hint, so \
                 this is a wire-shape change and not a formatting one.\n\n\
                 {REMEDY}"
            )
        });
    let mut keys: Vec<String> = array
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| {
                    panic!("/$defs/{name}/required holds a non-string entry: {value}\n\n{REMEDY}")
                })
                .to_string()
        })
        .collect();
    keys.sort();
    keys
}

/// One entry's named property, or a named failure.
fn property<'a>(entry: &'a Value, entry_name: &str, prop: &str) -> &'a Value {
    entry
        .get("properties")
        .and_then(|props| props.get(prop))
        .unwrap_or_else(|| {
            panic!(
                "/$defs/{entry_name}/properties/{prop} is missing from {VENDORED_TREE}\n\n\
                 {REMEDY}"
            )
        })
}

/// Every `$defs` entry OTHER than `CacheableResult` whose `properties` map
/// carries a `cacheScope` key — i.e. every result type that extends the
/// cacheable base, derived structurally rather than from a list.
///
/// Asserts non-vacuity before returning: a scanner that finds nothing must fail
/// HERE, loudly, rather than let a set comparison downstream succeed against an
/// empty set.
fn schema_entries_carrying_cache_scope(schema: &Value) -> BTreeSet<String> {
    let defs = schema.pointer("/$defs").and_then(Value::as_object)
        .unwrap_or_else(|| panic!(
            "{SCHEMA_JSON} has no `/$defs` object — the generated schema's whole shape changed.\n\n{REMEDY}"
        ));

    let found: BTreeSet<String> = defs
        .iter()
        .filter(|(name, _)| name.as_str() != CACHEABLE_RESULT)
        .filter(|(_, entry)| {
            entry
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|props| props.contains_key("cacheScope"))
        })
        .map(|(name, _)| name.clone())
        .collect();

    assert!(
        !found.is_empty(),
        "the scan of {SCHEMA_JSON} found ZERO schema entries carrying a `cacheScope` property, \
         out of {} entries under `/$defs`.\n\n\
         This is an anti-vacuity guard. Without it, an empty result would make the set \
         comparisons in this file compare two empty sets and report green while proving \
         nothing.\n\n\
         Either the caching hints were removed upstream, or the property was renamed.\n\n\
         {REMEDY}",
        defs.len(),
    );

    found
}

/// Whitespace collapsed to single spaces, so a declaration split across lines
/// reads the same as one written on a single line.
///
/// Load-bearing: `ListResourceTemplatesResult`'s `extends` clause is on the line
/// AFTER its name in the vendored source, so a line-oriented scan silently
/// misses it — and silently missing one of six is precisely the kind of
/// undercount this file exists to make impossible.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_was_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !previous_was_space {
                out.push(' ');
                previous_was_space = true;
            }
        } else {
            out.push(ch);
            previous_was_space = false;
        }
    }
    out
}

/// Every `export interface <Name> extends …CacheableResult…` in the TypeScript
/// source, derived independently of the JSON Schema.
///
/// The `extends` clause may list other bases first (`extends PaginatedResult,
/// CacheableResult`), so the whole clause up to the opening brace is searched
/// rather than the token immediately after `extends`.
fn ts_interfaces_extending_cacheable_result(ts: &str) -> BTreeSet<String> {
    const NEEDLE: &str = "export interface ";

    let flat = collapse_whitespace(ts);
    let mut found = BTreeSet::new();
    let mut cursor = 0usize;

    while let Some(offset) = flat[cursor..].find(NEEDLE) {
        let name_start = cursor + offset + NEEDLE.len();
        let name_end = flat[name_start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map_or(flat.len(), |index| name_start + index);
        let name = &flat[name_start..name_end];

        let clause_end = flat[name_end..]
            .find('{')
            .map_or(flat.len(), |index| name_end + index);
        let clause = &flat[name_end..clause_end];

        if name != CACHEABLE_RESULT && clause.contains(CACHEABLE_RESULT) {
            found.insert(name.to_string());
        }
        cursor = name_end;
    }

    found
}

fn expected_set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

// ===========================================================================
// 2. The CacheableResult contract.
// ===========================================================================

/// Resolved pointer: `/$defs/CacheableResult/required`.
#[test]
fn v2_core_schema_facts_cacheable_result_requires_all_three_fields() {
    let schema = schema_json();
    let entry = schema_entry(&schema, CACHEABLE_RESULT);
    let required = required_of(entry, CACHEABLE_RESULT);

    assert_eq!(
        required,
        EXPECTED_CACHEABLE_KEYS.to_vec(),
        "{CACHEABLE_RESULT_POINTER}/required changed.\n\n\
         Measured (sorted): {required:?}\n\
         Expected (sorted): {EXPECTED_CACHEABLE_KEYS:?}\n\
         Artifact:          {VENDORED_TREE}\n\n\
         Note before assuming the artifact is wrong: `resultType` belongs to this SAME base and \
         is ALREADY implemented — Phase 114's `inject_v2_result_envelope` injects it, which is \
         why nothing in Phase 115 adds it. A two-element expectation of \
         [\"cacheScope\", \"ttlMs\"] is the reader's error, not the schema's.\n\n\
         {REMEDY}"
    );
}

/// Resolved pointer: `/$defs/CacheableResult/properties/cacheScope`.
#[test]
fn v2_core_schema_facts_cache_scope_is_the_closed_public_private_union() {
    let schema = schema_json();
    let entry = schema_entry(&schema, CACHEABLE_RESULT);
    let cache_scope = property(entry, CACHEABLE_RESULT, "cacheScope");

    assert_eq!(
        cache_scope.get("type").and_then(Value::as_str),
        Some("string"),
        "{CACHEABLE_RESULT_POINTER}/properties/cacheScope is no longer a string.\n\n\
         Measured: {cache_scope}\n\n\
         {REMEDY}"
    );

    let variants: Vec<String> = cache_scope
        .get("enum")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "{CACHEABLE_RESULT_POINTER}/properties/cacheScope has no `enum` — it is no \
                 longer a CLOSED union.\n\n\
                 An open string is a different Rust type: the closed variant set is what makes a \
                 two-variant enum a faithful representation rather than a guess.\n\n\
                 {REMEDY}"
            )
        })
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    assert_eq!(
        variants,
        vec!["private".to_string(), "public".to_string()],
        "{CACHEABLE_RESULT_POINTER}/properties/cacheScope's variant set changed.\n\n\
         Measured (sorted): {variants:?}\n\
         Expected (sorted): [\"private\", \"public\"]\n\
         Artifact:          {VENDORED_TREE}\n\n\
         A WIDENED union means the two-variant Rust enum this phase ships cannot round-trip a \
         conformant peer's response, and a NARROWED one means it emits a value no peer accepts. \
         Either way the Rust type is now wrong.\n\n\
         {REMEDY}"
    );
}

/// Resolved pointer: `/$defs/CacheableResult/properties/ttlMs`.
///
/// # Why this assertion exists, and what it settles
///
/// The TypeScript source declares `ttlMs: number` with an `@minimum 0` doc
/// annotation. `number` in TypeScript admits fractions and negatives, so read
/// alone it argues for a floating-point Rust field and makes `u64` look like an
/// inference from a comment.
///
/// The GENERATED JSON Schema — the artifact a peer actually validates against —
/// narrows it to `{"type": "integer", "minimum": 0}`. That is what makes `u64` a
/// MEASURED mapping rather than an inference: a non-negative integer is exactly
/// what `u64` represents, and it is the generated schema, not the `.ts`, that a
/// conformance check runs against.
#[test]
fn v2_core_schema_facts_ttl_ms_is_a_nonnegative_integer() {
    let schema = schema_json();
    let entry = schema_entry(&schema, CACHEABLE_RESULT);
    let ttl_ms = property(entry, CACHEABLE_RESULT, "ttlMs");

    assert_eq!(
        ttl_ms.get("type").and_then(Value::as_str),
        Some("integer"),
        "{CACHEABLE_RESULT_POINTER}/properties/ttlMs is no longer typed `integer`.\n\n\
         Measured: {ttl_ms}\n\
         Artifact: {VENDORED_TREE}\n\n\
         This is the assertion that justifies `u64` in src/types/caching.rs. If a re-vendoring \
         changed this to `number`, a conformant peer may legitimately send a fractional TTL and \
         `u64` would REJECT it at deserialization — a spec-conformant response failing to parse. \
         The Rust representation must change with the schema; this assertion must not.\n\n\
         {REMEDY}"
    );

    assert_eq!(
        ttl_ms.get("minimum").and_then(Value::as_i64),
        Some(0),
        "{CACHEABLE_RESULT_POINTER}/properties/ttlMs no longer has `minimum: 0`.\n\n\
         Measured: {ttl_ms}\n\
         Artifact: {VENDORED_TREE}\n\n\
         `minimum: 0` is the other half of the `u64` justification: an integer WITHOUT a \
         non-negative floor admits negative TTLs, which `u64` in src/types/caching.rs cannot \
         represent and would reject from a conformant peer.\n\n\
         {REMEDY}"
    );
}

// ===========================================================================
// 3. Which result types carry the hints.
// ===========================================================================

/// Resolved pointers: every `/$defs/<Name>/properties/cacheScope`, cross-checked
/// against `export interface <Name> extends …CacheableResult` in `schema.ts`.
#[test]
fn v2_core_schema_facts_exactly_six_defs_extend_cacheable_result() {
    let schema = schema_json();
    let from_json = schema_entries_carrying_cache_scope(&schema);
    let expected = expected_set(&EXPECTED_EXTENDERS);

    assert_eq!(
        from_json, expected,
        "the set of result types carrying `cacheScope` changed in {SCHEMA_JSON}.\n\n\
         Measured: {from_json:?}\n\
         Expected: {expected:?}\n\
         Artifact: {VENDORED_TREE}\n\n\
         Before assuming the count is wrong: the phase requirement text says FIVE list/read \
         results. `DiscoverResult` is the measured SIXTH, read from the pinned artifact — the \
         requirement text is the side that is imprecise, not this set. `server/discover` is the \
         v2 replacement for `initialize` and the first call a v2 client makes, so omitting it \
         would ship a knowingly non-conformant response.\n\n\
         {REMEDY}"
    );

    let from_ts = ts_interfaces_extending_cacheable_result(&read(SCHEMA_TS));

    assert_eq!(
        from_ts, expected,
        "the TypeScript source and the expected set disagree.\n\n\
         Measured from {SCHEMA_TS}: {from_ts:?}\n\
         Expected:                  {expected:?}\n\n\
         {REMEDY}"
    );

    assert_eq!(
        from_ts,
        from_json,
        "the vendored `.ts` and `.json` DISAGREE about which results extend {CACHEABLE_RESULT}.\n\n\
         From schema.ts:   {from_ts:?}\n\
         From schema.json: {from_json:?}\n\n\
         The two files are generated from one another, so a disagreement is a FINDING about the \
         artifact — one of the pair is stale, or the vendoring picked two different commits. \
         Treat it as an upstream defect to report, not a test to relax; and until it is \
         resolved, prefer schema.json, because that is what a peer validates against.\n\n\
         {REMEDY}"
    );
}

/// Resolved pointers: every `/$defs/<Name>/required` for the six extenders.
#[test]
fn v2_core_schema_facts_each_of_the_six_requires_both_hints() {
    let schema = schema_json();

    for name in EXPECTED_EXTENDERS {
        let entry = schema_entry(&schema, name);
        let required = required_of(entry, name);

        for key in EXPECTED_CACHEABLE_KEYS {
            assert!(
                required.iter().any(|present| present == key),
                "/$defs/{name}/required does not list `{key}`.\n\n\
                 Measured (sorted): {required:?}\n\
                 Artifact:          {VENDORED_TREE}\n\n\
                 The caching hints being required on the CONCRETE result types — not merely on \
                 the shared {CACHEABLE_RESULT} base — is what makes \"required on the v2 \
                 projection\" a measured claim about each response this SDK actually emits. If \
                 a hint became optional upstream, the Rust projection may stop emitting it; \
                 until then it must always emit it.\n\n\
                 {REMEDY}"
            );
        }
    }
}

// ===========================================================================
// 4. Structured output and outputSchema.
// ===========================================================================

/// Resolved pointer: `/$defs/CallToolResult/properties/structuredContent`.
#[test]
fn v2_core_schema_facts_structured_content_is_any_json_value() {
    let schema = schema_json();
    let entry = schema_entry(&schema, "CallToolResult");
    let structured = property(entry, "CallToolResult", "structuredContent");

    assert!(
        structured
            .get("description")
            .and_then(Value::as_str)
            .is_some(),
        "/$defs/CallToolResult/properties/structuredContent has no `description`.\n\n\
         Measured: {structured}\n\n\
         The description is the only thing that distinguishes an intentionally unconstrained \
         value from an accidentally empty one, so its absence means the shape changed.\n\n\
         {REMEDY}"
    );

    assert!(
        structured.get("type").is_none() && structured.get("properties").is_none(),
        "/$defs/CallToolResult/properties/structuredContent is now CONSTRAINED.\n\n\
         Measured: {structured}\n\
         Artifact: {VENDORED_TREE}\n\n\
         This phase's structured-output work rests on the value being ANY JSON value — no \
         `type`, no `properties`. A constraint appearing upstream means the SDK's structured \
         content must be narrowed to match, and the tools that emit non-object values become \
         non-conformant.\n\n\
         {REMEDY}"
    );

    let ts = read(SCHEMA_TS);
    assert!(
        !ts.contains("Currently restricted to"),
        "{SCHEMA_TS} still carries the v1 sentence \"Currently restricted to\".\n\n\
         That sentence is what limited structured content to a single shape in the v1 schema. \
         Its ABSENCE from the 2026-07-28 source is this phase's premise; its reappearance means \
         the vendored tree is not the version it claims to be.\n\n\
         {REMEDY}"
    );
}

/// Resolved pointer: `/$defs/Tool/properties/outputSchema`.
#[test]
fn v2_core_schema_facts_output_schema_declares_an_optional_dollar_schema() {
    let schema = schema_json();
    let entry = schema_entry(&schema, "Tool");
    let output_schema = property(entry, "Tool", "outputSchema");

    assert_eq!(
        output_schema.get("type").and_then(Value::as_str),
        Some("object"),
        "/$defs/Tool/properties/outputSchema is no longer an object.\n\n\
         Measured: {output_schema}\n\n\
         {REMEDY}"
    );

    let dollar_schema = output_schema
        .get("properties")
        .and_then(|props| props.get("$schema"))
        .unwrap_or_else(|| {
            panic!(
                "/$defs/Tool/properties/outputSchema does not declare a `$schema` property.\n\n\
                 Measured: {output_schema}\n\n\
                 The spec ITSELF declaring an optional `$schema` key is what makes this SDK's \
                 decision to pin one dialect a spec-AWARE choice rather than a workaround. If \
                 the key is gone, that reasoning has to be rewritten, not merely re-checked.\n\n\
                 {REMEDY}"
            )
        });

    assert_eq!(
        dollar_schema.get("type").and_then(Value::as_str),
        Some("string"),
        "/$defs/Tool/properties/outputSchema/properties/$schema is not a string.\n\n\
         Measured: {dollar_schema}\n\n\
         {REMEDY}"
    );

    let declared_required = output_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    assert!(
        !declared_required.iter().any(|key| key == "$schema"),
        "/$defs/Tool/properties/outputSchema now REQUIRES `$schema`.\n\n\
         Measured `required`: {declared_required:?}\n\
         Artifact:            {VENDORED_TREE}\n\n\
         The key being OPTIONAL is what lets a tool omit it and get the default dialect. If it \
         became mandatory, every tool schema this SDK emits must declare it and the omission \
         path becomes non-conformant.\n\n\
         {REMEDY}"
    );
}

// ===========================================================================
// 5. Anti-vacuity: the artifact is really there and really parsed.
// ===========================================================================

/// Guards every other test in this file against passing over nothing.
///
/// A truncated, emptied or replaced artifact would make several assertions above
/// pass trivially — a missing key is not a wrong key. These floors make an
/// artifact that is not the artifact fail HERE, by name.
#[test]
fn v2_core_schema_facts_the_scan_is_not_vacuous() {
    let ts = read(SCHEMA_TS);
    let json_text = read(SCHEMA_JSON);

    assert!(
        ts.len() > 90_000,
        "{SCHEMA_TS} is {} bytes, below the 90,000-byte floor.\n\n\
         The vendored 2026-07-28 TypeScript source is ~98 KB. A file this small is truncated, \
         emptied, or is not that artifact — and a scan of it would find nothing while reporting \
         green.\n\n\
         {REMEDY}",
        ts.len()
    );

    assert!(
        json_text.len() > 150_000,
        "{SCHEMA_JSON} is {} bytes, below the 150,000-byte floor.\n\n\
         The vendored 2026-07-28 generated schema is ~181 KB.\n\n\
         {REMEDY}",
        json_text.len()
    );

    let schema = schema_json();
    let top_level: Vec<String> = schema
        .as_object()
        .unwrap_or_else(|| panic!("{SCHEMA_JSON} is not a JSON object.\n\n{REMEDY}"))
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    assert_eq!(
        top_level,
        vec!["$defs".to_string(), "$schema".to_string()],
        "{SCHEMA_JSON}'s top-level key set changed.\n\n\
         Measured (sorted): {top_level:?}\n\
         Expected (sorted): [\"$defs\", \"$schema\"]\n\n\
         Every pointer in this file resolves through `/$defs`. A different top-level shape means \
         the pointers are wrong, not the values.\n\n\
         {REMEDY}"
    );

    let entry_count = schema
        .pointer("/$defs")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);

    assert!(
        entry_count > 100,
        "{SCHEMA_JSON} declares only {entry_count} entries under `/$defs`, expected well over \
         100.\n\n\
         The vendored artifact carries 155. A far smaller count means a partial or different \
         schema.\n\n\
         {REMEDY}"
    );

    // The same scan the six-extender test relies on. Asserted here too, so the
    // anti-vacuity property is stated in the test that owns it and not only as a
    // side effect of another test's helper.
    let carriers = schema_entries_carrying_cache_scope(&schema);
    assert!(
        !carriers.is_empty(),
        "the `cacheScope` scan found nothing, so every set comparison in this file would be \
         comparing empty sets.\n\n\
         {REMEDY}"
    );

    let from_ts = ts_interfaces_extending_cacheable_result(&ts);
    assert!(
        !from_ts.is_empty(),
        "the TypeScript `extends {CACHEABLE_RESULT}` scan found nothing in {SCHEMA_TS}.\n\n\
         The cross-check in the six-extender test would then compare an empty set against an \
         empty set and pass. Note that one of the six splits its `extends` clause across two \
         lines, so a line-oriented scan undercounts — this file collapses whitespace first for \
         exactly that reason.\n\n\
         {REMEDY}"
    );
}
