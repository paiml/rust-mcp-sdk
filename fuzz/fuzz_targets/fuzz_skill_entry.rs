//! Fuzz target for SEP-2640 skill ENTRY SYNTHESIS (Phase 125, plan 05 — the
//! CLAUDE.md ALWAYS/FUZZ half of the skills feature).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run fuzz_skill_entry` (plain
//! form, no `+nightly` — matches the repo Makefile `test-fuzz` target).
//!
//! # Why THIS boundary
//!
//! A SKILL.md body is UNTRUSTED input. Skills arrive from a package, a
//! repository or a directory a server operator points at, and nothing in front
//! of `Skills::entries()` sanitises them. The YAML frontmatter parse is the only
//! place in the whole skills surface where author-supplied bytes reach a
//! THIRD-PARTY parser (`serde_yaml` 0.9, whose transitive `unsafe-libyaml` is
//! already in this graph), and everything downstream of it — the digest walk,
//! the size arithmetic, the name-identity rule — is arithmetic over whatever
//! that parser decided the bytes meant. A panic anywhere on that path is an
//! unwind an author can trigger with a text file.
//!
//! Bytes are interpreted LOSSILY as UTF-8 rather than rejected on a UTF-8
//! error, so the campaign spends its budget inside the parser instead of
//! bouncing off `from_utf8`, and so mid-codepoint truncation reaches the
//! frontmatter delimiter scan the way a half-written file would.
//!
//! # Two registries per input, deliberately
//!
//! The same bytes are driven through the path twice:
//!
//! - **As the whole body.** Exercises the delimiter scan — no block, an
//!   unterminated block, a leading BOM, a lone `---`.
//! - **As the CONTENTS of a well-formed block.** Exercises `serde_yaml` itself
//!   with arbitrary bytes, which the first shape reaches only when the fuzzer
//!   has already guessed a `---` prefix. Without this the third-party parser —
//!   the actual reason this target exists — is barely reached.
//!
//! # Invariants
//!
//! 1. Entry synthesis NEVER panics, whatever the bytes. No `unwrap` on any
//!    `Result` derived from the input (T-125-19).
//! 2. Every emitted digest is `sha256:` followed by EXACTLY 64 LOWERCASE hex
//!    characters. Asserted char by char, not by prefix alone: an uppercase
//!    rendering is a silent host-side comparison failure.
//! 3. Every emitted `size` equals the byte length of the content that row
//!    describes, re-derived HERE from the strings this target constructed. It is
//!    therefore an independent statement rather than the production code
//!    agreeing with itself.
//! 4. The manifest is SKILL.md FIRST, then references, with no duplicate URI.
//! 5. `Skills::entries()` and `Skills::into_handler()` AGREE on acceptance. For
//!    a single-skill registry the duplicate-URI rule cannot fire, so the only
//!    thing either can reject on is the shared name-identity validation — a
//!    disagreement means a registry that lists what it will not serve, or
//!    serves what it will not list.
//! 6. An emitted `frontmatter` is always a JSON OBJECT. A scalar or a sequence
//!    must have taken the exclusion path instead of being emitted.
//!
//! # Corpus cases worth seeding
//!
//!   - `` (empty), and a lone `---`
//!   - `---\nname: fuzzed\ndescription: d\n---\nbody\n` (the accept path)
//!   - `---\nname: other\n---\n` (the name-identity reject path)
//!   - `\u{feff}---\nname: fuzzed\n---\n` (leading BOM)
//!   - `---\nname: fuzzed\n` (unterminated block)
//!   - `---\n- a\n- b\n---\n` and `---\njust a scalar\n---\n` (non-mapping)
//!   - `---\n&a [*a]\n---\n` (a YAML alias cycle)
//!   - deeply nested mappings, and a duplicate YAML key
//!   - invalid UTF-8, and a byte string truncated mid-codepoint

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::skills::{Skill, SkillReference, Skills};

/// The constructor name every skill this target builds carries. Its resolved
/// URI is therefore `skill://fuzzed/SKILL.md`, and the name-identity rule fires
/// exactly when the parsed frontmatter carries a `name` that is not this.
const SKILL_NAME: &str = "fuzzed";

/// The one reference path this target registers, so the manifest has a second
/// row and invariant 4's ordering claim is not vacuous.
const REF_PATH: &str = "references/data.md";

/// Is `digest` the `sha256:` + 64-lowercase-hex form SEP-2640 hosts compare on?
///
/// Spelled out here rather than delegated to a crate helper: invariant 2 is only
/// evidence if the shape it demands is stated independently of the code that
/// produced it.
fn digest_is_well_formed(digest: &str) -> bool {
    match digest.strip_prefix("sha256:") {
        Some(hex) => {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        },
        None => false,
    }
}

/// Drive ONE registry through the whole build path and assert invariants 2-6.
///
/// `body` and `reference_body` are the exact strings this target handed to the
/// registry, so the size checks below are re-derived rather than read back out
/// of the value under test.
fn check_registry(body: &str, reference_body: &str) {
    let skill = Skill::new(SKILL_NAME, body);
    // `with_reference` PANICS on a rejected path; the fallible form is the only
    // one a fuzz target may call, even with a path this target controls.
    let skill = match skill.try_with_reference(SkillReference::new(
        REF_PATH,
        "text/markdown",
        reference_body,
    )) {
        Ok(s) => s,
        Err(_) => Skill::new(SKILL_NAME, body),
    };

    // Invariant 1 for both consumers: neither may unwind on these bytes.
    let entries = Skills::new().add(skill.clone()).entries();
    let handler = Skills::new().add(skill).into_handler();

    // Invariant 5: for a ONE-skill registry the duplicate-URI rule cannot fire,
    // so the only shared rejection cause is `validate_names`. A disagreement
    // means a registry that lists what it refuses to serve, or the reverse.
    assert_eq!(
        entries.is_ok(),
        handler.is_ok(),
        "entries() and into_handler() disagreed on acceptance for a single-skill \
         registry; they are documented to run the SAME name validation"
    );

    let Ok(entries) = entries else { return };

    // A frontmatter-less or malformed body is EXCLUDED (D-02), so an empty
    // entry set is a legitimate outcome and carries nothing to check.
    for entry in &entries {
        // Invariant 6.
        assert!(
            entry.frontmatter().is_object(),
            "an emitted entry's frontmatter must be a JSON object; a scalar or a \
             sequence is documented to take the exclusion path instead"
        );

        let manifest = entry.resources();
        assert!(
            !manifest.is_empty(),
            "an emitted entry always carries at least its own SKILL.md row"
        );

        // Invariant 4: SKILL.md first, then references, no duplicate URI.
        assert_eq!(
            manifest[0].uri(),
            entry.uri(),
            "the manifest's first row must be the skill's own SKILL.md"
        );
        for i in 1..manifest.len() {
            assert_ne!(
                manifest[i].uri(),
                manifest[0].uri(),
                "a reference row must not repeat the SKILL.md URI"
            );
        }

        for row in manifest {
            // Invariant 2.
            assert!(
                digest_is_well_formed(row.digest()),
                "digest `{}` is not `sha256:` + 64 lowercase hex",
                row.digest()
            );

            // Invariant 3: sizes re-derived from the strings THIS target built.
            let expected = if row.uri() == entry.uri() {
                body.len()
            } else {
                reference_body.len()
            };
            assert_eq!(
                row.size(),
                expected,
                "row `{}` reported {} bytes for content this target built as {} bytes",
                row.uri(),
                row.size(),
                expected
            );
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let body = String::from_utf8_lossy(data).into_owned();

    // Shape one: the bytes ARE the SKILL.md body — the delimiter scan.
    check_registry(&body, &body);

    // Shape two: the bytes are the CONTENTS of a well-formed frontmatter block,
    // so `serde_yaml` is reached without the fuzzer first having to guess a
    // `---` prefix. This is the third-party parse this target exists for.
    let framed = format!("---\n{body}\n---\n\n# Body\n");
    check_registry(&framed, &body);
});
