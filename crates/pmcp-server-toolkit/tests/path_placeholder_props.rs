//! Phase 128 D4(b) — the floor-then-narrow ORDERING and the percent-encoding
//! CLOSURE, pinned over GENERATED input rather than over a handful of fixtures.
//!
//! # Why a property and not more fixtures
//!
//! RESEARCH Finding 5b MEASURED that a declared `pattern` of `^.*$` — a shape
//! specs in the wild are full of — accepts every CR-01 payload. So a
//! pattern-supersedes-floor implementation would have been a no-op for exactly the
//! inputs the phase exists to refuse, and a fixture suite that happened not to
//! pair a floor-denied value with a permissive declared pattern would have stayed
//! green through the re-ordering. D-10 is an ordering claim about the INTERACTION
//! of two rules; the interaction is what a property covers and a fixture table
//! does not.
//!
//! # Why this binary has BOTH a non-ignored arm and `#[ignore]`d arms
//!
//! Not a style inconsistency — each convention protects something the other
//! cannot, and the combination is the only satisfiable one:
//!
//! - **The non-ignored smoke arm** (`path_placeholder_props_smoke_…`) is what
//!   makes the binary's DEFAULT-run passed count nonzero.
//!   `scripts/named-test-binary-count.awk` reads the PASSED count from the
//!   `test result:` line and `Makefile`'s `REQUIRED_TEST_BINARIES` loop rejects a
//!   zero, while the Makefile's own invocation does not pass `--ignored`. An
//!   all-`#[ignore]`d binary therefore reports `0 passed` and FAILS the leg —
//!   measured, not predicted. The smoke arm is deliberately small: its job is to
//!   prove the binary RAN.
//! - **The three `#[ignore]`d `property_` arms** carry the invariants and are
//!   selected by a SECOND, separately count-asserted `-- --ignored property_`
//!   invocation inside `make test-server-toolkit`. Without that invocation they
//!   would be selectable in principle and dead weight in every gate run.
//!
//! # SP-3, and the caveat that makes it necessary here
//!
//! The `#[ignore = "property arm — …"]` marker is carried verbatim from
//! `tests/log_emitter.rs`. But `make test-property` runs
//! `cargo test --features "full" -- --ignored property_`, a **ROOT-package**
//! selector — it cannot reach a toolkit test binary at all. So the marker is
//! convention-preserving here and NOT the thing that gets these arms run; the
//! `test-server-toolkit` invocation named above is. This phase's ROOT-package
//! property arm lives in `tests/schema_validation_props.rs` instead.
//!
//! # The `?` narrowing, and what this binary does and does not assert about it
//!
//! The operator narrowed the query-separator rule: an author-written query string
//! in a path is ACCEPTED, while a `?` arriving from a placeholder VALUE is still
//! REFUSED. That narrowing lives in ONE place — core's
//! `pmcp::server::schema_validation::validate_resolved_target`, which splits the
//! composed path at the FIRST `?` and passes BOTH sides through the UNMODIFIED
//! `validate_resolved_path`. Both production callers reach it from there:
//! `pmcp_code_mode::ResolvedPath::from_checked` on the Code Mode surface and
//! `HttpClient::check_composed_path` on the curated one.
//!
//! This binary asserts the two CORE facts that split composes, and deliberately
//! does not re-derive the split itself (one implementation of a rule, never two):
//! a `?` in a VALUE is refused by the floor, and core's composed check is strict
//! about `?` ANYWHERE. The ACCEPT direction is already pinned where the split
//! lives — `crates/pmcp-server-toolkit/src/http/client.rs`'s
//! `query_separator` module (3 accept rows) and `curated_path_injection.rs`'s
//! author-query control on the curated surface, and `pmcp-code-mode`'s
//! `executor::query_separator` (3 accept rows) on the Code Mode one.
//!
//! # Gating
//!
//! `#![cfg(all(feature = "http", feature = "input-validation"))]`, matching
//! `curated_path_injection.rs`: `input-validation` is what forwards
//! `pmcp/schema-validation` and therefore what compiles the functions under test,
//! and `http` is the toolkit feature the gate's invocation names. Deliberately NOT
//! `openapi-code-mode` — these are properties of the CORE primitives that both
//! HTTP surfaces call, and requiring the JS engine would test a heavier build than
//! the one they are about.
//!
//! Offline; no network, no wiremock, no runtime.

#![cfg(all(feature = "http", feature = "input-validation"))]

use pmcp::server::schema_validation::{
    validate_path_placeholder, validate_resolved_path, PlaceholderRules, PLACEHOLDER_MAX_LENGTH,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The CR-01 payloads RESEARCH Finding 5b measured `^.*$` accepting.
///
/// Six, not five: the Finding's probe table lists `2026AA?string=x`,
/// `current/../../search/current`, `%2e%2e%2f`, `%3Fstring%3Dx`, `a%00b` and
/// `a#frag`. All six are asserted rather than a subset, because the measurement is
/// the reason the ordering is the way it is.
const CR01_PAYLOADS: &[&str] = &[
    "2026AA?string=x",
    "current/../../search/current",
    "%2e%2e%2f",
    "%3Fstring%3Dx",
    "a%00b",
    "a#frag",
];

/// Declared patterns that ADMIT every value this file generates, so a refusal can
/// only have come from the floor or the cap.
///
/// `^.*$` is the measured catch-all. `^.+$` is its non-empty sibling. `^[\s\S]*$`
/// admits literally anything including a newline, so the set does not depend on
/// `.`'s newline semantics in any engine.
const PERMISSIVE_PATTERNS: &[&str] = &["^.*$", "^.+$", r"^[\s\S]*$"];

/// Floor-denied fragments, in the spellings the decode-once floor must catch: the
/// literal forms, the fully percent-encoded forms in BOTH hex cases, the mixed
/// form that matches neither enumeration, and the two percent-handling shapes.
const DENIED_FRAGMENTS: &[&str] = &[
    "?",
    "#",
    "/",
    "..",
    "\\",
    "%2e%2e%2f",
    "%2E%2E%2F",
    "%3Fstring%3Dx",
    ".%2E",
    "%00",
    "%25",
];

/// The refusal rules the unconditional floor may return. A refusal carrying any
/// OTHER rule for a floor-denied value would mean something later in the ordering
/// got there first, which is the D-10 inversion.
const FLOOR_RULES: &[&str] = &["characterFloor", "percentEncoding"];

/// Percent-encode every byte of `token`, choosing the hex case.
fn percent_encode(token: &str, upper: bool) -> String {
    token
        .bytes()
        .map(|b| {
            if upper {
                format!("%{b:02X}")
            } else {
                format!("%{b:02x}")
            }
        })
        .collect()
}

/// The longest window the no-echo assertion checks, in bytes.
///
/// "Contains no BYTE of the value" cannot be asserted literally: the refusal
/// message is English prose, so any single letter of a generated value collides
/// with it trivially. The real invariant is that no non-trivial RUN of the value
/// survives into the message. Six ASCII uppercase bytes cannot appear in the
/// lowercase prose the refusal renders, so the generators below draw from `[A-Z]`
/// and this window is collision-free by construction rather than by luck.
const ECHO_WINDOW: usize = 6;

// =============================================================================
// The NON-IGNORED arm. Its job is to make the binary's default-run passed count
// nonzero so `REQUIRED_TEST_BINARIES` is satisfiable; the invariants live in the
// three property arms below.
// =============================================================================

#[test]
fn path_placeholder_props_smoke_floor_refuses_cr01_payloads() {
    let catch_all = PlaceholderRules::default().with_pattern(Some("^.*$"));

    // The SHRUNK counterexample, pinned deterministically. When
    // `validate_path_placeholder`'s ordering was deliberately inverted to
    // pattern-supersedes-floor, `property_declared_pattern_never_widens_the_floor`
    // shrank to exactly this: the value "A?" against a declared `^.*$`. Two
    // characters is the whole of the D-10 hazard, and pinning it here means the
    // guard survives deletion of the `.proptest-regressions` seed file.
    assert!(
        validate_path_placeholder("v", "A?", &catch_all).is_err(),
        "the minimal D-10 counterexample: `^.*$` must not admit a value carrying `?`"
    );

    for payload in CR01_PAYLOADS {
        let bare = validate_path_placeholder("v", payload, &PlaceholderRules::default())
            .expect_err("the floor must refuse every CR-01 payload");
        let narrowed = validate_path_placeholder("v", payload, &catch_all)
            .expect_err("a `^.*$` declaration must not switch the floor off (D-10)");
        assert_eq!(
            bare.rule, narrowed.rule,
            "{payload:?}: the declared catch-all must not change WHICH rule refused"
        );
        assert!(
            FLOOR_RULES.contains(&narrowed.rule),
            "{payload:?}: refused by {:?}, expected one of {FLOOR_RULES:?}",
            narrowed.rule
        );
    }
}

// =============================================================================
// The three property arms.
// =============================================================================

/// D-10 as a property: the unconditional floor runs BEFORE any declared pattern,
/// so a permissive declaration cannot admit a floor-denied value.
///
/// # Why this is not vacuous
///
/// Every generated value carries a floor-denied fragment BY CONSTRUCTION, and
/// every one is paired with a declared pattern drawn from a set whose members all
/// ADMIT it — so every single case exercises the interaction, and the pattern-first
/// implementation this property exists to forbid would have accepted the value.
/// Two tallies are asserted nonzero at the end so a generator that silently
/// stopped producing the shape could not pass quietly.
///
/// Case count is proptest's default (256 via `PROPTEST_CASES` in the gate). Nothing
/// is bounded below the default here: each case is three string builds and up to
/// four calls into core, no regex compiled by this file.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_declared_pattern_never_widens_the_floor() {
    let floor_refusals = AtomicUsize::new(0);
    let catch_all_cases = AtomicUsize::new(0);

    proptest!(|(
        base in "[A-Z]{1,12}",
        tail in "[A-Z]{0,8}",
        fragment_idx in 0usize..DENIED_FRAGMENTS.len(),
        pattern_idx in 0usize..PERMISSIVE_PATTERNS.len(),
        author_query in prop::option::of("[a-z]{1,8}=[a-z]{1,8}"),
    )| {
        let fragment = DENIED_FRAGMENTS[fragment_idx];
        let pattern = PERMISSIVE_PATTERNS[pattern_idx];
        let injected = format!("{base}{fragment}{tail}");
        let clean = format!("{base}{tail}");

        let declared = PlaceholderRules::default().with_pattern(Some(pattern));

        // ---- SHAPE A: the fragment arrives from the VALUE. Always refused, and
        // ---- refused by the FLOOR even though the declared pattern admits it.
        let narrowed = validate_path_placeholder("v", &injected, &declared)
            .expect_err("a declared pattern must never admit a floor-denied value");
        prop_assert!(
            FLOOR_RULES.contains(&narrowed.rule),
            "refused by {:?}; a floor-denied value must be refused by the FLOOR, not by \
             a later step — {:?} with pattern {pattern:?}",
            narrowed.rule, fragment
        );
        prop_assert_ne!(
            narrowed.rule, "pattern",
            "the declared pattern must not be what refuses: that ordering would let \
             `^.*$` switch the floor off"
        );

        // The verdict is IDENTICAL with no declaration at all, which is the
        // ordering stated as an equality rather than as an inequality.
        let bare = validate_path_placeholder("v", &injected, &PlaceholderRules::default())
            .expect_err("the floor alone must refuse it too");
        prop_assert_eq!(bare.rule, narrowed.rule);
        floor_refusals.fetch_add(1, Ordering::Relaxed);
        if pattern == "^.*$" {
            catch_all_cases.fetch_add(1, Ordering::Relaxed);
        }

        // ---- SHAPE B: the SAME `?` byte, AUTHOR-WRITTEN into the template. The
        // ---- per-value check reads the VALUE and never the template, so a clean
        // ---- value stays clean no matter what the author wrote around it — and a
        // ---- value cannot borrow the author's exemption either.
        prop_assert!(
            validate_path_placeholder("v", &clean, &declared).is_ok(),
            "an uppercase-only value carries no floor-denied byte and must be accepted"
        );
        prop_assert!(
            validate_resolved_path(&format!("/api/{clean}")).is_ok(),
            "and must survive composition"
        );
        if let Some(query) = &author_query {
            // `validate_resolved_path` is STRICT about `?` anywhere. This Err is
            // precisely what its sibling `validate_resolved_target` relaxes for an
            // AUTHOR-written separator; it is asserted here so the property holds
            // under the narrowing rather than claiming a rule this function does
            // not have.
            prop_assert!(
                validate_resolved_path(&format!("/api/{clean}?{query}")).is_err(),
                "`validate_resolved_path` refuses `?` anywhere — the exemption lives \
                 in its sibling `validate_resolved_target`, not here"
            );
        }
    });

    assert!(
        floor_refusals.load(Ordering::Relaxed) > 0,
        "the generator produced no floor-denied value at all — the property proved nothing"
    );
    assert!(
        catch_all_cases.load(Ordering::Relaxed) > 0,
        "the generator never paired a floor-denied value with the measured `^.*$` \
         catch-all, which is the exact interaction D-10 is about"
    );
}

/// The floor is closed under percent-encoding: if the DECODED form of a value
/// would be refused, the ENCODED form is refused too.
///
/// Both hex cases are generated, so `%2E` versus `%2e` is exercised rather than
/// assumed. This is the property behind the decode-once implementation: an
/// enumerated denylist of literal and pre-encoded spellings is incomplete by
/// construction, and `.%2E` — generated here — is the witness, decoding to the
/// parent-directory sequence while matching neither enumeration.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_refusal_is_closed_under_percent_encoding() {
    /// Literal (already-decoded) fragments, so `percent_encode` has something
    /// meaningful to encode. The percent-spelled entries in `DENIED_FRAGMENTS` are
    /// excluded here: encoding an encoding is a different question, covered by the
    /// `%25` row in the smoke arm.
    const LITERALS: &[&str] = &["/", "?", "#", "\\", "..", ".", "\u{0}", "\u{7f}", "A"];

    let non_vacuous = AtomicUsize::new(0);
    let both_cases = [AtomicUsize::new(0), AtomicUsize::new(0)];

    proptest!(|(
        base in "[A-Z]{0,8}",
        tail in "[A-Z]{0,8}",
        literal_idx in 0usize..LITERALS.len(),
        upper_hex in any::<bool>(),
    )| {
        let literal = LITERALS[literal_idx];
        let decoded = format!("{base}{literal}{tail}");
        let encoded = format!("{base}{}{tail}", percent_encode(literal, upper_hex));
        both_cases[usize::from(upper_hex)].fetch_add(1, Ordering::Relaxed);

        if validate_path_placeholder("v", &decoded, &PlaceholderRules::default()).is_err() {
            non_vacuous.fetch_add(1, Ordering::Relaxed);
            prop_assert!(
                validate_path_placeholder("v", &encoded, &PlaceholderRules::default()).is_err(),
                "{decoded:?} is refused but its encoded form {encoded:?} is not — the floor \
                 is not closed under percent-encoding, which is the hole an enumerated \
                 denylist always has"
            );
        }
    });

    assert!(
        non_vacuous.load(Ordering::Relaxed) > 0,
        "no generated value was refused in its decoded form, so the implication was never \
         tested — the antecedent must be reachable"
    );
    for (i, count) in both_cases.iter().enumerate() {
        assert!(
            count.load(Ordering::Relaxed) > 0,
            "hex case {i} (0 = lowercase, 1 = uppercase) was never generated"
        );
    }
}

/// SC-7 as a property: a refusal never echoes the value it refused.
///
/// Asserted as "no `ECHO_WINDOW`-byte run of the value appears in the rendered
/// refusal", which is the strongest form the claim can take — the message is
/// English prose, so a literal per-byte claim is unsatisfiable for any value at
/// all. The generators draw from `[A-Z]`, and a six-byte uppercase run cannot
/// appear in the lowercase prose the refusal renders.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_refusal_never_echoes_the_placeholder_value() {
    let checked = AtomicUsize::new(0);

    proptest!(|(
        base in "[A-Z]{2,24}",
        tail in "[A-Z]{0,8}",
        fragment_idx in 0usize..DENIED_FRAGMENTS.len(),
        over_cap in any::<bool>(),
    )| {
        let mut value = format!("{base}{}{tail}", DENIED_FRAGMENTS[fragment_idx]);
        if over_cap {
            // Also exercise the CAP's refusal message, not only the floor's.
            value.push_str(&"Z".repeat(PLACEHOLDER_MAX_LENGTH));
        }
        prop_assume!(value.len() >= 2);
        prop_assert!(value.is_ascii(), "the byte-window check assumes ASCII");

        let refusal = validate_path_placeholder("v", &value, &PlaceholderRules::default())
            .expect_err("every generated value carries a floor-denied fragment");
        let rendered = refusal.to_string();

        prop_assert!(
            !rendered.contains(&value),
            "the refusal echoed the whole value: {rendered:?}"
        );
        let window = ECHO_WINDOW.min(value.len());
        for start in 0..=value.len() - window {
            let run = &value[start..start + window];
            prop_assert!(
                !rendered.contains(run),
                "the refusal echoed a {window}-byte run {run:?} of the value: {rendered:?}"
            );
        }
        // The DECLARED name is safe to echo, and is the only thing the caller is
        // told. A refusal that named neither the param nor the rule would be
        // value-free and useless.
        prop_assert!(rendered.contains("param 'v'"), "{rendered:?}");
        prop_assert!(!refusal.rule.is_empty());
        checked.fetch_add(1, Ordering::Relaxed);
    });

    assert!(
        checked.load(Ordering::Relaxed) > 0,
        "no value reached the no-echo assertion — `prop_assume!` rejected everything"
    );
}
