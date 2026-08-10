//! Client-side caching hints for the MCP `2026-07-28` `CacheableResult` base.
//!
//! This module carries the MCP `2026-07-28` `CacheableResult` vocabulary: the
//! `ttlMs` freshness hint and the `cacheScope` sharing scope, plus the single
//! projection point that decides whether either key reaches the wire.
//!
//! # What carries these hints
//!
//! Exactly six results extend `CacheableResult` in the vendored
//! `2026-07-28` schema — `DiscoverResult`, `ListToolsResult`,
//! `ListResourcesResult`, `ListResourceTemplatesResult`, `ReadResourceResult`
//! and `ListPromptsResult`. Each of the corresponding Rust types carries an
//! `Option`-typed `ttl_ms` / `cache_scope` slot a handler MAY set.
//!
//! # v2 only (D-11)
//!
//! The hints exist on the **v2 projection only**. A `2025-11-25` (or earlier)
//! response never carries them: `project_caching_hints` REMOVES both keys on
//! every non-v2 input, so a handler that sets a hint and then serves a legacy
//! client emits a byte-identical legacy response. That severability is what
//! keeps the v1 compatibility layer cleanly removable.
//!
//! # Handler-set, SDK-defaulted (D-08 / D-12)
//!
//! The values are chosen by the handler and defaulted by the SDK at ONE shared
//! projection point. A handler that expresses no preference gets the safe
//! defaults — [`DEFAULT_TTL_MS`] and [`CacheScope::default()`] — injected on the
//! v2 wire, where both keys are REQUIRED.
//!
//! # `ttlMs` here is NOT a task TTL (D-10)
//!
//! The `ttlMs` in this module is a CACHE-FRESHNESS hint: how long a client may
//! reuse a response body. It is **not**
//! [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
//! LIFETIME: how long the server retains a task record. The two live in
//! deliberately separate modules (`types::caching` versus `types::tasks`) and
//! neither imports the other. Copying a long task lifetime into a cache hint
//! would make stale data look fresh.
//!
//! # Why this module carries no `cfg`
//!
//! This module is deliberately `cfg`-free, so it compiles on every target and
//! `project_caching_hints` is callable from ALL dispatchers: the native
//! `ServerCore` / `Server` paths (`src/server/core.rs` and `src/server/mod.rs`,
//! both gated `cfg(not(target_arch = "wasm32"))`) AND `WasmMcpServer`
//! (`src/server/wasm_server.rs`, gated `cfg(target_arch = "wasm32")`). Those two
//! `cfg` sets are disjoint, so a projector living in either one would be
//! structurally unreachable from the other — and the wasm dispatcher
//! serializes handler-constructed `ReadResourceResult` / `ListResourcesResult`
//! values directly, which is exactly the path a hint could leak onto a v1 wire.
//! Do not "simplify" this back into a server module.

use serde::{Deserialize, Serialize};

/// The intended sharing scope of a cached response.
///
/// Analogous to HTTP `Cache-Control: public` versus `Cache-Control: private`.
///
/// # Security
///
/// The MCP `2026-07-28` schema defines the two values as follows (quoted
/// verbatim from `schema/vendored/core-2026-07-28/schema.ts`):
///
/// > - `"public"`: The response does not contain user-specific data. Any
/// >   client or intermediary (e.g., shared gateway, caching proxy) MAY cache
/// >   the response and serve it across authorization contexts.
/// > - `"private"`: The response MAY be cached and reused only within the
/// >   same authorization context. Caches MUST NOT be shared across
/// >   authorization contexts (e.g., a different access token requires a different cache).
///
/// The consequence, in our own words: marking a per-user response
/// [`CacheScope::Public`] is a cross-authorization-context data leak. A shared
/// gateway is then entitled to serve one caller's response body to a different
/// caller holding a different access token, and the server has told it that is
/// allowed. When in doubt use [`CacheScope::Private`].
///
/// # Why `Private` is the SDK default
///
/// [`CacheScope::default()`] is [`CacheScope::Private`] (D-08). Defaulting to
/// `Public` would make every response nobody explicitly considered
/// cross-caller cacheable — a leak by omission rather than by decision. This is
/// the same defect class the tasks surface's own privacy rules exist to
/// prevent: the safe value is the one you get for free.
///
/// # Why this enum is NOT `#[non_exhaustive]`
///
/// The published `2026-07-28` schema declares the property as a CLOSED union of
/// exactly two values (`$defs.CacheableResult.properties.cacheScope.enum` is
/// `["private", "public"]`). Marking the enum `#[non_exhaustive]` would force
/// every downstream `match` to carry an unreachable catch-all arm for a variant
/// set the spec fixes. The closedness is deliberate and is fenced by a test
/// asserting that an unknown value FAILS to deserialize — do not add
/// `#[serde(other)]` or a catch-all variant.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::CacheScope;
///
/// // The SDK default is the safe one.
/// assert_eq!(CacheScope::default(), CacheScope::Private);
///
/// // The wire spellings are lowercase, as the schema's enum declares.
/// assert_eq!(
///     serde_json::to_string(&CacheScope::Public).unwrap(),
///     "\"public\""
/// );
/// assert_eq!(
///     serde_json::to_string(&CacheScope::Private).unwrap(),
///     "\"private\""
/// );
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    /// > `"public"`: The response does not contain user-specific data. Any
    /// > client or intermediary (e.g., shared gateway, caching proxy) MAY cache
    /// > the response and serve it across authorization contexts.
    ///
    /// Only assert this for a response whose body is identical for every
    /// caller regardless of identity, token or tenant.
    Public,
    /// > `"private"`: The response MAY be cached and reused only within the
    /// > same authorization context. Caches MUST NOT be shared across
    /// > authorization contexts (e.g., a different access token requires a different cache).
    ///
    /// The SDK default, because it is the value that cannot leak.
    #[default]
    Private,
}

/// Every [`CacheScope`] variant, for exhaustive table and property tests.
///
/// Declared BESIDE the enum on purpose: the union is closed by the published
/// `2026-07-28` schema, and a single enumeration point sitting next to the
/// variants is what makes "a variant was added without extending the tests"
/// visible in one screenful rather than in a grep.
///
/// `#[cfg(test)]` rather than `pub`: the crate's own precedent for this is
/// `types::tasks::ALL_STATUSES`, and exporting it would add a public item to a
/// phase whose `cargo public-api diff` is expected to be empty.
#[cfg(test)]
const ALL_SCOPES: &[CacheScope] = &[CacheScope::Public, CacheScope::Private];

/// The SDK-supplied default for `ttlMs` when a handler expresses no preference.
///
/// The value is `0`, which the MCP `2026-07-28` schema documents as:
///
/// > - If 0, The response SHOULD be considered immediately stale,
/// >   The client MAY re-fetch every time the result is needed.
///
/// That is precisely why `0` is the right default: it asserts NOTHING about
/// cacheability. A conformant peer receiving it behaves exactly as it would
/// have without the field, so the SDK-supplied default is inert while still
/// satisfying the v2 wire's requirement that the key be present.
///
/// # Why `u64`
///
/// This is a MEASURED mapping, not an inference. The TypeScript source spells
/// the field `ttlMs: number`, which would admit fractions — but the GENERATED
/// JSON Schema that a conformant peer actually validates against declares
/// `$defs.CacheableResult.properties.ttlMs` as
/// `{"type": "integer", "minimum": 0}`. Integrality and non-negativity are
/// therefore contract, and `u64` is exact across the whole declared domain
/// except for the absent upper bound. At millisecond resolution `u64::MAX` is
/// roughly 584 million years, so that residual is an ACCEPTED risk rather than
/// a reason to reach for a bignum type.
///
/// `tests/v2_core_schema_facts.rs` asserts the schema side of this and the
/// `cacheable_result_serde_locks` module below asserts the Rust side: if a
/// re-vendoring ever widens the declared type to `"number"`, those tests fail
/// and the Rust representation must change with them.
pub const DEFAULT_TTL_MS: u64 = 0;

/// Whether a given result is one of the six that extend `CacheableResult`.
///
/// The `2026-07-28` schema gives exactly six results a `CacheableResult` base:
/// `DiscoverResult`, `ListToolsResult`, `ListResourcesResult`,
/// `ListResourceTemplatesResult`, `ReadResourceResult` and `ListPromptsResult`.
/// Everything else — `tools/call`, `prompts/get`, every task method, every
/// notification acknowledgement — is [`Cacheable::No`].
///
/// The value is decided by the CALLER rather than derived inside the projector,
/// because at the native chokepoint where the projection runs the request has
/// already been moved and the response is an opaque `serde_json::Value`. The
/// classifier that produces it is a separate shared function
/// (`request_is_cacheable`), so the two native dispatchers cannot drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cacheable {
    /// The result extends `CacheableResult` and participates in the projection.
    Yes,
    /// The result does not extend `CacheableResult`; the projection is a no-op.
    No,
}

/// Project the `2026-07-28` caching hints onto (or off) a serialized result.
///
/// This is the ONLY writer of the `ttlMs` and `cacheScope` wire keys in the
/// tree (D-12 — a single shared projection point; fenced structurally by a
/// source tripwire). It is deliberately total: every input either ensures both
/// keys or removes both keys, so there is no path that leaves a result half
/// projected.
///
/// # Behaviour
///
/// - [`Cacheable::No`] — returns immediately, touching nothing (D-07: only the
///   six `CacheableResult` extenders carry these keys).
/// - A non-object `value` — returns; a scalar, array or null result body cannot
///   carry a key.
/// - `Some(Era::V2)` — ENSURES both keys, without overwriting: a handler-set
///   value survives verbatim and an unset one receives the safe defaults
///   [`DEFAULT_TTL_MS`] and [`CacheScope::default()`] (D-08). The default scope
///   is produced by SERIALIZING the enum, never by typing a string literal, so
///   the injected default and the enum cannot drift apart.
/// - Anything else — `Some(Era::V1)` **or** `None` — REMOVES both keys if
///   present (D-11). This is not merely "don't add": it is an active strip, so
///   a handler that deliberately set a hint and then served a legacy client
///   still emits a byte-identical legacy response. A strip is normal operation,
///   not an error, and is not logged.
///
/// # Why the `None` arm matters
///
/// `WasmMcpServer` (`src/server/wasm_server.rs`) has no era awareness at all —
/// it carries no `ProtocolContext` — so it passes `None`. Its `WasmResource`
/// handlers construct `ReadResourceResult` / `ListResourcesResult` values that
/// the file serializes directly, which without this arm would put a
/// handler-set hint straight onto the wasm server's v1 wire. The `None` arm is
/// what makes that leak structurally impossible, and it is why this function
/// lives in a `cfg`-free module rather than in either server module.
pub(crate) fn project_caching_hints(
    value: &mut serde_json::Value,
    era: Option<crate::types::protocol::Era>,
    cacheable: Cacheable,
) {
    if matches!(cacheable, Cacheable::No) {
        return;
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if matches!(era, Some(crate::types::protocol::Era::V2)) {
        object
            .entry("ttlMs")
            .or_insert_with(|| serde_json::Value::from(DEFAULT_TTL_MS));
        object.entry("cacheScope").or_insert_with(|| {
            serde_json::to_value(CacheScope::default()).expect("a unit enum always serializes")
        });
    } else {
        // `shift_remove`, NOT `remove`. This crate enables serde_json's
        // `preserve_order` feature (`Cargo.toml`), under which `Map::remove` is
        // `swap_remove`: it back-fills the vacated slot with the map's LAST
        // entry, REORDERING every key that followed. D-11's promise is a
        // BYTE-IDENTICAL legacy response, and key order is part of the bytes —
        // `tests/v1_lists_golden.rs` pins raw frames for exactly that reason,
        // but its hinted fixture deliberately pins no bytes, so a reorder
        // introduced here would not be caught. It is harmless only while
        // `ttl_ms` / `cache_scope` happen to be the LAST two fields declared on
        // all six `CacheableResult` extenders; `shift_remove` makes the strip
        // order-preserving unconditionally instead.
        object.shift_remove("ttlMs");
        object.shift_remove("cacheScope");
    }
}

/// Unit coverage for [`project_caching_hints`] (115-05, SCHM-03).
#[cfg(test)]
mod projection_tests {
    use super::{project_caching_hints, CacheScope, Cacheable, DEFAULT_TTL_MS};
    use crate::types::protocol::Era;
    use serde_json::json;

    #[test]
    fn v2_inserts_the_safe_defaults() {
        let mut value = json!({ "tools": [] });
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
        assert_eq!(
            value["ttlMs"],
            json!(DEFAULT_TTL_MS),
            "a v2 projection must carry the required `ttlMs`, got {value}"
        );
        assert_eq!(
            value["cacheScope"],
            json!("private"),
            "an un-considered response must default to the non-leaking scope, got {value}"
        );
        assert_eq!(value["tools"], json!([]), "existing keys must be untouched");
    }

    #[test]
    fn v2_preserves_handler_set_values() {
        let mut value = json!({ "ttlMs": 300_000, "cacheScope": "public" });
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
        assert_eq!(
            value["ttlMs"],
            json!(300_000),
            "a handler-set ttlMs must survive the projection verbatim, got {value}"
        );
        assert_eq!(
            value["cacheScope"],
            json!("public"),
            "a handler-set cacheScope must survive the projection verbatim, got {value}"
        );
    }

    #[test]
    fn v1_strips_handler_set_values() {
        let mut value = json!({ "resources": [], "ttlMs": 300_000, "cacheScope": "public" });
        project_caching_hints(&mut value, Some(Era::V1), Cacheable::Yes);
        assert!(
            value.get("ttlMs").is_none(),
            "D-11: a v1 response must never carry `ttlMs`, got {value}"
        );
        assert!(
            value.get("cacheScope").is_none(),
            "D-11: a v1 response must never carry `cacheScope`, got {value}"
        );
        assert_eq!(
            value["resources"],
            json!([]),
            "the strip must not disturb any other key"
        );
    }

    /// The `era = None` path, which is exactly what `WasmMcpServer` passes.
    ///
    /// `src/server/wasm_server.rs` carries no `ProtocolContext`, so it can only
    /// ever pass `None`; its `WasmResource` handlers construct results that the
    /// file serializes directly. That file is compiled ONLY for `wasm32`, and
    /// its own `cfg(all(test, target_arch = "wasm32"))` test module does not
    /// compile at all today, so this NATIVE unit test is the only RUNNABLE
    /// proof that the wasm dispatcher's era-less input strips rather than
    /// leaks. The compile-time proof is `make wasm-build`; the structural proof
    /// is the source tripwire added by 115-08.
    #[test]
    fn no_context_strips_both_keys_which_is_the_wasm_path() {
        let mut value = json!({
            "contents": [],
            "ttlMs": 300_000,
            "cacheScope": "public",
            "_meta": { "keep": true }
        });
        project_caching_hints(&mut value, None, Cacheable::Yes);
        assert!(
            value.get("ttlMs").is_none(),
            "an era-less dispatcher must strip `ttlMs`, got {value}"
        );
        assert!(
            value.get("cacheScope").is_none(),
            "an era-less dispatcher must strip `cacheScope`, got {value}"
        );
        assert_eq!(
            value["contents"],
            json!([]),
            "every other key must be untouched by the strip"
        );
        assert_eq!(
            value["_meta"],
            json!({ "keep": true }),
            "every other key must be untouched by the strip"
        );
    }

    #[test]
    fn not_cacheable_is_the_identity() {
        let before = json!({ "content": [], "ttlMs": 5, "cacheScope": "public" });
        let mut value = before.clone();
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::No);
        assert_eq!(
            value, before,
            "a non-CacheableResult body must not be touched at all"
        );

        let mut value = before.clone();
        project_caching_hints(&mut value, Some(Era::V1), Cacheable::No);
        assert_eq!(
            value, before,
            "the identity must hold on every era, not just v2"
        );
    }

    #[test]
    fn a_non_object_value_is_untouched() {
        for mut value in [json!(null), json!([1, 2, 3]), json!("a string"), json!(7)] {
            let before = value.clone();
            project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
            assert_eq!(
                value, before,
                "a non-object result body cannot carry a key and must be left alone"
            );
        }
        // The scope enum is still the safe one; nothing above may mutate it.
        assert_eq!(CacheScope::default(), CacheScope::Private);
    }
}

/// The CLOSED-union property, held over arbitrary input (115-09, SCHM-03).
///
/// `cacheable_result_serde_locks` below pins the two wire spellings and the one
/// hand-picked rejection (`"shared"`). These two properties generalize that: the
/// round trip must be the identity for EVERY variant, and an arbitrary string
/// must deserialize if and ONLY if it is one of the two. A `#[serde(other)]`
/// arm or a catch-all variant added later fails the second one immediately,
/// whatever value the author happened to think of.
///
/// # ALWAYS Requirement
///
/// CLAUDE.md makes property tests mandatory for every new feature. Deliberately
/// NOT `#[ignore]`d: `make test-property` (`Makefile:230-233`) selects only
/// `--ignored property_` tests, so an `#[ignore]` here would move these OUT of
/// the default `cargo test` run and INTO a target that no `property_*` function
/// in this repo currently reaches — trading a silent gate for a silent test.
/// The verification command is
/// `cargo nextest run --lib --features full -E 'test(/types::caching/)'`.
#[cfg(test)]
mod caching_properties {
    use super::{CacheScope, ALL_SCOPES};

    proptest::proptest! {
        /// Serializing then deserializing any variant is the identity, and the
        /// serialized form is always one of exactly two byte strings.
        #[test]
        fn property_cache_scope_serde_round_trips_for_every_variant(
            index in 0usize..ALL_SCOPES.len(),
        ) {
            let scope = ALL_SCOPES[index];
            let raw = serde_json::to_string(&scope).expect("a unit enum always serializes");

            proptest::prop_assert!(
                raw == "\"public\"" || raw == "\"private\"",
                "the schema declares a two-value enum; {:?} serialized to {}",
                scope,
                raw
            );

            let back: CacheScope =
                serde_json::from_str(&raw).expect("the serialized form must deserialize");
            proptest::prop_assert_eq!(
                back,
                scope,
                "a CacheScope round trip must be the identity"
            );
        }

        /// An arbitrary string deserializes as a `CacheScope` if and only if it
        /// is exactly `public` or `private` — the closed union held over
        /// arbitrary input rather than one hand-picked counterexample.
        #[test]
        fn property_an_arbitrary_string_is_accepted_as_a_cache_scope_iff_it_is_one_of_the_two(
            candidate in "[a-zA-Z_-]{0,16}",
        ) {
            let json = serde_json::to_string(&candidate).expect("a string always serializes");
            let parsed = serde_json::from_str::<CacheScope>(&json);
            let is_declared = candidate == "public" || candidate == "private";

            proptest::prop_assert_eq!(
                parsed.is_ok(),
                is_declared,
                "CacheScope is a CLOSED union: {} must deserialize iff it is one of the two \
                 declared values, but from_str returned {:?}",
                json,
                parsed.map(|scope| format!("{scope:?}"))
            );
        }
    }
}

/// Serde locks binding the RUST side to the vendored `2026-07-28` contract.
///
/// These assertions read the vendored artifact at COMPILE time rather than
/// restating its contents, so a re-vendoring moves them automatically instead
/// of leaving them asserting yesterday's contract. `tests/v2_core_schema_facts.rs`
/// locks the SCHEMA itself; this module locks the Rust representation AGAINST
/// the schema, and deliberately does not duplicate the former's assertions.
///
/// Every failure message names the same remedy: if the vendored contract
/// genuinely changed, re-run the `## Change protocol` in
/// `schema/vendored/core-2026-07-28/PROVENANCE.md` and update the RUST side —
/// never the assertion.
#[cfg(test)]
mod cacheable_result_serde_locks {
    use super::{CacheScope, DEFAULT_TTL_MS};
    use serde_json::Value;

    /// The vendored `2026-07-28` core JSON Schema, embedded at compile time.
    const CORE_SCHEMA_JSON: &str =
        include_str!("../../schema/vendored/core-2026-07-28/schema.json");

    /// The remedy every failure message in this module points at.
    const REMEDY: &str = "if the vendored contract changed, re-run the `## Change protocol` in \
         schema/vendored/core-2026-07-28/PROVENANCE.md and update the RUST side, never this assertion";

    /// The one `CacheableResult.required` entry that is deliberately NOT a
    /// struct field: Phase 114's `inject_v2_result_envelope` supplies it.
    const INJECTED_ELSEWHERE: &[&str] = &["resultType"];

    /// Resolve `/$defs/CacheableResult` from the vendored artifact.
    ///
    /// The pointer root is `$defs`, NOT `definitions`.
    fn cacheable_result_def() -> Value {
        let schema: Value =
            serde_json::from_str(CORE_SCHEMA_JSON).expect("the vendored core schema parses");
        schema
            .pointer("/$defs/CacheableResult")
            .unwrap_or_else(|| panic!("/$defs/CacheableResult must resolve — {REMEDY}"))
            .clone()
    }

    /// The sorted `required` array of `/$defs/CacheableResult`.
    fn cacheable_result_required() -> Vec<String> {
        let def = cacheable_result_def();
        let mut required: Vec<String> = def["required"]
            .as_array()
            .unwrap_or_else(|| panic!("$defs.CacheableResult.required is an array — {REMEDY}"))
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("a required entry is a string")
                    .to_string()
            })
            .collect();
        required.sort();
        required
    }

    /// A `ListResourcesResult` with BOTH caching hints set by the "handler".
    fn hinted_list_resources() -> crate::types::ListResourcesResult {
        crate::types::ListResourcesResult::new(vec![])
            .with_ttl_ms(60_000)
            .with_cache_scope(CacheScope::Public)
    }

    /// The Rust field spellings must match the vendored `required` key set.
    ///
    /// The vendored array has THREE entries. Two are emitted by the Rust
    /// structs this phase edits (`cacheScope`, `ttlMs`); the third,
    /// `resultType`, belongs to the same base but is supplied by Phase 114's
    /// `inject_v2_result_envelope` and is deliberately NOT a struct field —
    /// so its absence from the serialized struct is accounted for, not a gap.
    #[test]
    fn rust_field_spellings_match_the_vendored_required_set() {
        let required = cacheable_result_required();
        assert_eq!(
            required,
            vec!["cacheScope", "resultType", "ttlMs"],
            "the vendored CacheableResult.required set moved — {REMEDY}"
        );

        let raw = serde_json::to_string(&hinted_list_resources()).expect("serializes");
        let emitted: Value = serde_json::from_str(&raw).expect("round-trips");
        let emitted = emitted
            .as_object()
            .expect("a result serializes to an object");

        // `INJECTED_ELSEWHERE` is the by-design exception: Phase 114 owns the
        // v2 result envelope, so `resultType` is accounted for, not missing.
        for key in &required {
            if INJECTED_ELSEWHERE.contains(&key.as_str()) {
                assert!(
                    !emitted.contains_key(key),
                    "`{key}` is injected by inject_v2_result_envelope and must NOT be a \
                     struct field; found it in {raw}"
                );
            } else {
                assert!(
                    emitted.contains_key(key),
                    "the vendored contract requires `{key}` but no Rust field emits it — {REMEDY}"
                );
            }
        }

        // A missing struct-level `rename_all` would be invisible to a purely
        // structural test, so assert the snake_case spellings never reach the wire.
        assert!(
            !raw.contains("ttl_ms"),
            "the wire spelling is `ttlMs`; a snake_case key leaked into {raw}"
        );
        assert!(
            !raw.contains("cache_scope"),
            "the wire spelling is `cacheScope`; a snake_case key leaked into {raw}"
        );
    }

    /// The `CacheScope` wire values must match the vendored enum exactly.
    #[test]
    fn cache_scope_wire_values_match_the_vendored_enum() {
        let def = cacheable_result_def();
        let mut declared: Vec<String> = def["properties"]["cacheScope"]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("cacheScope declares an enum — {REMEDY}"))
            .iter()
            .map(|v| v.as_str().expect("an enum entry is a string").to_string())
            .collect();
        declared.sort();
        assert_eq!(
            declared,
            vec!["private", "public"],
            "the vendored cacheScope variant set moved — {REMEDY}"
        );

        assert_eq!(
            serde_json::to_string(&CacheScope::Public).expect("serializes"),
            "\"public\"",
            "CacheScope::Public must spell `public` on the wire"
        );
        assert_eq!(
            serde_json::to_string(&CacheScope::Private).expect("serializes"),
            "\"private\"",
            "CacheScope::Private must spell `private` on the wire"
        );

        for variant in [CacheScope::Public, CacheScope::Private] {
            let raw = serde_json::to_string(&variant).expect("serializes");
            let back: CacheScope = serde_json::from_str(&raw).expect("round-trips");
            assert_eq!(
                back, variant,
                "a CacheScope round-trip must be the identity, {raw} came back as {back:?}"
            );
        }
    }

    /// `u64` must remain the correct Rust mapping for the declared JSON type.
    ///
    /// If a re-vendoring ever changes `ttlMs.type` to `"number"`, `u64` can
    /// REJECT a conformant peer's fractional value, and the Rust
    /// representation is what must change — never this assertion.
    #[test]
    fn ttl_ms_rust_type_matches_the_vendored_json_schema_type() {
        let def = cacheable_result_def();
        let ttl = &def["properties"]["ttlMs"];
        assert_eq!(
            ttl["type"], "integer",
            "ttlMs is no longer an integer; `u64` would reject a conformant peer — {REMEDY}"
        );
        assert_eq!(
            ttl["minimum"], 0,
            "ttlMs's declared minimum moved — {REMEDY}"
        );

        // The declared minimum is representable; the absent upper bound is the
        // one ACCEPTED residual (u64::MAX ms is ~584 million years).
        assert_eq!(u64::MIN, 0, "u64 must represent the declared minimum of 0");

        let extreme = crate::types::ListResourcesResult::new(vec![]).with_ttl_ms(u64::MAX);
        let emitted: Value =
            serde_json::from_str(&serde_json::to_string(&extreme).expect("serializes"))
                .expect("round-trips");
        assert!(
            emitted["ttlMs"].is_u64(),
            "ttlMs must serialize as a JSON integer, not a float or a string; got {}",
            emitted["ttlMs"]
        );
    }

    /// The union is CLOSED: an unknown value must fail to deserialize.
    ///
    /// This is what "closed union" means operationally, and it is the fence
    /// against a later `#[serde(other)]` or catch-all variant.
    #[test]
    fn an_unknown_cache_scope_value_is_rejected() {
        let parsed = serde_json::from_str::<CacheScope>("\"shared\"");
        assert!(
            parsed.is_err(),
            "CacheScope is a CLOSED union; `shared` must not deserialize, got {parsed:?}"
        );
    }

    /// Unset hints must emit NO key at all, on every one of the six types.
    ///
    /// This is the byte-neutral property `tests/v1_lists_golden.rs` depends on,
    /// asserted here at the type level rather than only at the HTTP level.
    #[test]
    fn unset_hints_emit_no_key_at_all() {
        let bodies = vec![
            (
                "ListToolsResult",
                serde_json::to_string(&crate::types::ListToolsResult::new(vec![])),
            ),
            (
                "ListResourcesResult",
                serde_json::to_string(&crate::types::ListResourcesResult::new(vec![])),
            ),
            (
                "ListResourceTemplatesResult",
                serde_json::to_string(&crate::types::ListResourceTemplatesResult::new(vec![])),
            ),
            (
                "ReadResourceResult",
                serde_json::to_string(&crate::types::ReadResourceResult::new(vec![])),
            ),
            (
                "ListPromptsResult",
                serde_json::to_string(&crate::types::ListPromptsResult::new(vec![])),
            ),
            (
                "ServerDiscoverResult",
                serde_json::to_string(&crate::types::ServerDiscoverResult {
                    protocol_version: "2026-07-28".to_string(),
                    capabilities: crate::types::ServerCapabilities::default(),
                    server_info: crate::types::Implementation::new("t", "0.0.0"),
                    // This fixture asserts the ABSENCE of the unset cache hints;
                    // the accept-list is irrelevant to that question (G-7).
                    supported_versions: Vec::new(),
                    ttl_ms: None,
                    cache_scope: None,
                }),
            ),
        ];
        for (name, raw) in bodies {
            let raw = raw.expect("serializes");
            assert!(
                !raw.contains("ttlMs"),
                "{name} with an unset hint must not emit `ttlMs`, got {raw}"
            );
            assert!(
                !raw.contains("cacheScope"),
                "{name} with an unset hint must not emit `cacheScope`, got {raw}"
            );
        }
    }

    /// The SAFE defaults, pinned.
    #[test]
    fn the_default_cache_scope_is_private_and_the_default_ttl_is_zero() {
        assert_eq!(
            CacheScope::default(),
            CacheScope::Private,
            "changing the default to Public is a cross-authorization-context data leak: a \
             shared gateway would be authorized to serve one caller's response body to \
             another caller holding a different access token"
        );
        assert_eq!(
            DEFAULT_TTL_MS, 0,
            "the SDK default must assert NOTHING about cacheability; 0 means immediately stale"
        );
    }

    /// Anti-vacuity: a schema-shape change must fail loudly, not pass over nothing.
    #[test]
    fn the_vendored_schema_lookup_is_not_vacuous() {
        let schema: Value =
            serde_json::from_str(CORE_SCHEMA_JSON).expect("the vendored core schema parses");
        assert!(
            schema.pointer("/$defs/CacheableResult").is_some(),
            "the CacheableResult definition must resolve at /$defs/CacheableResult — {REMEDY}"
        );
        // Resolved from the `schema` already parsed above rather than through
        // `cacheable_result_required()`, which would re-parse the same 177 KB
        // artifact a second time within this one test.
        assert_eq!(
            schema
                .pointer("/$defs/CacheableResult/required")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3),
            "CacheableResult.required must have exactly three entries — {REMEDY}"
        );
        assert!(
            CORE_SCHEMA_JSON.len() > 150_000,
            "the vendored artifact shrank to {} bytes; these locks may be asserting over \
             a truncated schema — {REMEDY}",
            CORE_SCHEMA_JSON.len()
        );
    }
}
