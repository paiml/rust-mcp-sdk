//! Fuzz target pointed at RESEARCH assumption **A2** — a config-supplied
//! `pattern` as a `ReDoS` vector on the path-placeholder narrowing path
//! (Phase 128, T-128-48).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//!
//! ```bash
//! cd fuzz && cargo +nightly fuzz run fuzz_placeholder_pattern_redos -- -timeout=5
//! ```
//!
//! **`+nightly` is REQUIRED and is not a style choice.** `cargo fuzz` passes
//! `-Zsanitizer=address`, which stable rustc refuses with "the option `Z` is only
//! accepted on the nightly compiler" — the build fails before a single iteration
//! runs. The repo's `make test-fuzz` invokes the PLAIN form and pipes every
//! non-zero exit into `|| echo "… completed"`, so on a stable default toolchain
//! that target reports success having fuzzed NOTHING. **Do not cite
//! `make test-fuzz` as evidence for this target.** `make test-fuzz-strict` is the
//! leg that runs it with `+nightly` and PROPAGATES a crash or a timeout.
//!
//! # This is a LOOK, not a mitigation
//!
//! Assumption A2 asks whether a `[[tools.parameters]] pattern` written by a config
//! author (or lifted from a bundled `OpenAPI` spec) can be made to blow up the
//! regex engine on a caller-supplied value. `128-RESEARCH.md` Finding 1j attempted
//! falsification at n <= 28 against BOTH the linear and the backtracking engine
//! and **did not reproduce**. That is evidence, not proof: it bounds what was
//! tried, not what exists.
//!
//! So the disposition on T-128-48 is **accept, instrumented** — this target is the
//! instrument. It exists to LOOK for the residual cheaply and repeatedly, because
//! a bounded look is honest where a speculative mitigation is not.
//!
//! **A timeout or a crash from this target is a FINDING TO FILE, not a licence to
//! hand-roll a regex guard.** File it with the reproducing artifact from
//! `fuzz/artifacts/fuzz_placeholder_pattern_redos/`, record the engine and the
//! `jsonschema` version, and let the fix be chosen with the measurement in hand.
//! Hand-rolling a pattern-complexity heuristic on a hunch is how a narrowing that
//! authors rely on starts refusing legitimate declarations.
//!
//! A2's own secondary instrument is named in `128-02-SUMMARY.md`: the
//! `BacktrackLimitExceeded` arm of `expectation`, whose `tracing::warn!` carries
//! `schema_path` only. If the engine bails out rather than hanging, that warning —
//! not a timeout here — is where the signal appears, and the refusal is
//! `could not be evaluated against the declared pattern`.
//!
//! # What is being fuzzed
//!
//! `pmcp::server::schema_validation::validate_path_placeholder`, the ONE copy of
//! the D4 character floor, with the declared `pattern` supplied as the narrowing
//! (step 3 of its four ordered steps). Both halves are hostile in production: the
//! pattern is author- or spec-supplied and the value is caller-supplied, and the
//! function runs on the `tools/call` path for every substituted path parameter on
//! both HTTP surfaces.
//!
//! Note the ORDERING that shapes what this target can reach: the unconditional
//! character floor (step 1) and the 256-code-point cap (step 2) both run BEFORE the
//! declared pattern. So a value carrying `/`, `\`, `?`, `#`, a control byte, a
//! malformed percent escape or more than `PLACEHOLDER_MAX_LENGTH` code points is
//! refused without the regex engine ever seeing it. That is the D-10 ordering
//! working as designed, and it means the reachable adversarial value space is
//! narrower than "arbitrary bytes" — which is itself part of A2's answer.
//!
//! # Cache growth: bounded by the RUN, not by construction
//!
//! Stated because it is the one place this target is weaker than
//! `fuzz_input_schema_enforcement`. Step 3 routes the declared pattern through
//! `cached_input_validator`, which memoizes on schema text in a process-global map,
//! so each distinct pattern adds one entry for the process lifetime. Unlike the
//! schema target, the pattern CANNOT be projected onto a bounded family without
//! destroying the point of the target — an arbitrary pattern is the input under
//! test.
//!
//! Two mitigations, both partial and both deliberate:
//!
//! - patterns longer than `MAX_PATTERN_LEN` bytes are skipped, which bounds each
//!   entry's SIZE (a `ReDoS` pattern is short by nature; catastrophic backtracking
//!   comes from nesting, not from length);
//! - the run itself is bounded. `make test-fuzz-strict` passes `-runs` and
//!   `-max_total_time`, and `fuzz.yml` caps at `-max_total_time=300`.
//!
//! Do NOT run this target unbounded and then read an out-of-memory abort as an A2
//! reproduction — that would be the exact indistinguishability the schema target's
//! projection exists to avoid. The durable fix is a `fuzzing`-gated UNCACHED seam
//! in `src/server/schema_validation.rs`, in the shape of
//! `output_validation::fuzz_support`; this plan was directed to leave that file at
//! a zero-line diff, so it is handed onward.
//!
//! # Input layout
//!
//! Same length-prefixed two-document layout as `fuzz_schema_draft_pin.rs` and
//! `fuzz_input_schema_enforcement.rs`, so the crate has ONE convention and a
//! corpus file can be written by hand:
//!
//! ```text
//! byte 0                   : selector — bit 0 allow_slash, bit 1 declare a max_length
//! bytes 1..5               : u32 little-endian pattern_len
//! bytes 5..5+pattern_len   : the declared `pattern` bytes
//! bytes 5+pattern_len..    : the caller-supplied value bytes (the remainder)
//! input shorter than 5     : return immediately; no assertion is possible
//! pattern_len > remaining  : clamped to the remainder, leaving an empty value
//! ```
//!
//! # Invariants
//!
//! 1. **Totality.** `validate_path_placeholder` returns for any (pattern, value)
//!    pair. A panic there is a remotely reachable unwind on the `tools/call` path.
//! 2. **No hang** — enforced by libFuzzer's `-timeout`, not by an assertion here.
//!    That is the A2 instrument.
//! 3. **The floor is never widened by a declared pattern.** A value the
//!    floor-and-cap default refuses must stay refused however permissive the
//!    declared `pattern` is. This is T-128-36 / the D-10 ordering, asserted here
//!    over arbitrary patterns rather than over the fixture `^.*$`: a pattern is a
//!    NARROWING, and any reachable pattern that widens the floor is a hole in the
//!    one place the phase cannot afford one.
//! 4. **Refusals stay value-free (SC-7).** A `PlaceholderRefusal` carries the
//!    DECLARED param name, a fixed rule token and a DECLARED expectation. Its
//!    `Display` must not echo the caller's value. Asserted with the same
//!    provenance discipline as the schema target: a per-case sentinel placed in the
//!    VALUE and verified absent from the declaration first.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::schema_validation::{validate_path_placeholder, PlaceholderRules};

/// The declared parameter name. Lowercase ASCII, so it cannot hold a sentinel.
const PARAM: &str = "version";

/// Longest declared pattern this target will drive. See the cache-growth section:
/// bounds each cache entry's size without bounding their number.
const MAX_PATTERN_LEN: usize = 96;

/// A value the floor-and-cap default MUST refuse whatever the declared pattern
/// says — the invariant-3 probe. Traversal plus a separator, so it trips the
/// character floor on two independent grounds.
const FLOOR_DENIED: &str = "a/../../etc/passwd";

/// Sentinel prefix, chosen so no declared name or rule token can contain it.
const SENTINEL_PREFIX: &str = "Zq7PHI";

/// FNV-1a over the whole input, so the sentinel varies per case.
fn sentinel(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{SENTINEL_PREFIX}{h:016x}")
}

/// Byte-wise substring search — see `fuzz_input_schema_enforcement` for why this
/// is not `str::contains`.
fn contains_bytes(haystack: &str, needle: &str) -> bool {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    !n.is_empty() && n.len() <= h.len() && h.windows(n.len()).any(|w| w == n)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let selector = data[0];
    let pattern_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let rest = &data[5..];
    let split = pattern_len.min(rest.len());
    let (pattern_bytes, value_bytes) = rest.split_at(split);

    if pattern_bytes.len() > MAX_PATTERN_LEN {
        return;
    }
    // A declared `pattern` is a JSON string, so a non-UTF-8 one cannot occur in a
    // real config. Skipping keeps the corpus about regex shapes rather than about
    // `from_utf8` — which `fuzz_schema_draft_pin` already covers.
    let Ok(pattern) = std::str::from_utf8(pattern_bytes) else {
        return;
    };
    let Ok(value) = std::str::from_utf8(value_bytes) else {
        return;
    };

    let allow_slash = selector & 0x01 != 0;
    let declared_max = if selector & 0x02 != 0 {
        Some(usize::from(selector >> 2).max(1))
    } else {
        None
    };

    let rules = PlaceholderRules::default()
        .with_pattern(Some(pattern))
        .with_max_length(declared_max)
        .allowing_slash(allow_slash);

    // Invariants 1 and 2 — totality, and no hang (the latter enforced by
    // libFuzzer's -timeout rather than by any assertion here).
    let verdict = validate_path_placeholder(PARAM, value, &rules);

    // Invariant 3 — the floor is never widened by a declared pattern. `allow_slash`
    // is deliberately forced OFF for this probe: with it on, `/` is legitimately
    // permitted, and the probe would be asserting the opposite of the documented
    // contract. The traversal half of FLOOR_DENIED has no escape either way.
    let floor_probe = PlaceholderRules::default()
        .with_pattern(Some(pattern))
        .with_max_length(declared_max);
    assert!(
        validate_path_placeholder(PARAM, FLOOR_DENIED, &floor_probe).is_err(),
        "D-10 ORDERING VIOLATION: a declared pattern widened the unconditional floor.\n  \
         pattern = {pattern:?}\n  declared_max = {declared_max:?}"
    );

    // Invariant 4 — refusals stay value-free. Provenance oracle: the sentinel is
    // verified absent from the DECLARATION (the pattern) before it is placed in the
    // value, and a collision skips rather than fails.
    let sent = sentinel(data);
    if !contains_bytes(pattern, &sent) {
        let carried = format!("{value}{sent}");
        if let Err(refusal) = validate_path_placeholder(PARAM, &carried, &rules) {
            let rendered = refusal.to_string();
            assert!(
                !contains_bytes(&rendered, &sent),
                "SC-7 VIOLATION: a placeholder refusal echoed the caller's value.\n  \
                 refusal = {rendered}\n  rule = {}\n  pattern = {pattern:?}",
                refusal.rule
            );
            assert!(
                !contains_bytes(&refusal.expected, &sent),
                "SC-7 VIOLATION: PlaceholderRefusal::expected echoed the caller's value.\n  \
                 expected = {}\n  rule = {}",
                refusal.expected,
                refusal.rule
            );
        }
    }

    // Keep the first verdict observable to the coverage-guided mutator.
    if verdict.is_ok() {
        let _ = validate_path_placeholder(PARAM, value, &rules);
    }
});
