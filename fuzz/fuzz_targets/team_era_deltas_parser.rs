//! Fuzz target for `pmcp_team_servers::conformance::era_diff::parse_baseline`,
//! the pmcp-team-servers expected-difference baseline parser.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run team_era_deltas_parser`.
//!
//! Invariant: `parse_baseline` must never panic on arbitrary bytes. Error paths
//! are acceptable; panics are not. The parser is reachable from a user-supplied
//! baseline path (`era_diff::load_baseline`), so a malformed or hostile file
//! must produce an `Err`, not an unwind (threat T-118-10, the T-117-27 class).
//!
//! # The `Ok`-path assertions are the parser's OWN documented contract
//!
//! `parse_baseline`'s doc comment enumerates exactly four rejections: an empty
//! `id`, an empty `observation_id`, a duplicated `id`, and a duplicated
//! `observation_id`. Those four — and only those four — are asserted below.
//! Asserting anything else would crash the fuzzer on well-formed input the
//! parser legitimately accepts: valid YAML carrying `id: ""` would parse fine if
//! the emptiness rule lived only here, so the rule was put INTO the parser
//! instead. If this target ever grows an assertion, first check that
//! `parse_baseline` documents rejecting its negation.
//!
//! In particular this target does NOT assert that an `observation_id` is in
//! `PROBE_REGISTRY`, that a `source` clears the citation floor, or that a
//! provisional entry names its phase. Those are baseline-CONTENT rules, gated
//! against the CHECKED-IN file by `crates/pmcp-team-servers/tests/era_baseline.rs`
//! — they are not properties of arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_team_servers::conformance::era_diff::parse_baseline;
use std::collections::HashSet;

fuzz_target!(|data: &[u8]| {
    // YAML is defined over text; non-UTF-8 bytes are out of the parser's domain.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // Must return Result, never panic.
    let Ok(baseline) = parse_baseline(text) else {
        return;
    };

    let mut ids: HashSet<&str> = HashSet::new();
    let mut observation_ids: HashSet<&str> = HashSet::new();

    for delta in &baseline.deltas {
        assert!(
            !delta.id.trim().is_empty(),
            "parse_baseline accepted a delta with an empty `id`, which its doc comment says it \
             rejects"
        );
        assert!(
            !delta.observation_id.trim().is_empty(),
            "parse_baseline accepted delta `{}` with an empty `observation_id`, which its doc \
             comment says it rejects",
            delta.id
        );
        assert!(
            ids.insert(delta.id.as_str()),
            "parse_baseline accepted a duplicated `id` `{}`, which its doc comment says it rejects",
            delta.id
        );
        assert!(
            observation_ids.insert(delta.observation_id.as_str()),
            "parse_baseline accepted a duplicated `observation_id` `{}` (on entry `{}`), which its \
             doc comment says it rejects",
            delta.observation_id,
            delta.id
        );
    }

    // The collection accessor must agree with the collection it projects.
    assert_eq!(
        baseline.observation_ids().len(),
        baseline.deltas.len(),
        "observation_ids() must yield exactly one entry per delta"
    );
});
