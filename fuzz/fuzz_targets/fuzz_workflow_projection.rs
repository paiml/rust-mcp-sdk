//! Fuzz target for the Phase 126 WORKFLOW-TO-SKILL PROJECTION — the CLAUDE.md
//! ALWAYS/FUZZ half of `SequentialWorkflow::as_skill()`.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//! `cargo +nightly fuzz run fuzz_workflow_projection`
//!
//! The `+nightly` prefix is NOT decoration and NOT a local quirk. `cargo fuzz
//! run` passes `-Z` flags, so the plain form fails on a stable toolchain with
//! `error: the option 'Z' is only accepted on the nightly compiler`, and this
//! repository's `rust-toolchain.toml` pins stable. `make test-fuzz` iterates
//! `cargo fuzz list` and swallows every failure with `|| echo`, so it exits 0
//! having run NOTHING — it is not evidence that this target ever executed.
//! `.github/workflows/fuzz.yml` deletes `rust-toolchain.toml` and installs
//! nightly explicitly before it can run the matrix; do the equivalent locally.
//!
//! # Why THIS boundary
//!
//! `SequentialWorkflow::as_skill()` is a PUBLIC, INFALLIBLE method on a
//! published crate. Every string it renders — the workflow name, its
//! description, each step name, each piece of guidance — is server-author text
//! that arrives from a config file, a scaffold, or a code generator, and nothing
//! validates it before the renderer sees it. The method's signature offers the
//! caller no way to be told about bad input, so the ONLY failure mode available
//! to the renderer is a panic, and a panic here unwinds inside whatever server
//! is building its prompt surface (T-126-02).
//!
//! Two of the render steps are arithmetic over author text and are exactly where
//! a panic would live: slugification truncates the name to 64 units, and the
//! frontmatter encoder escapes arbitrary bytes into a YAML double-quoted scalar
//! whose output becomes a PUBLISHED sha256 digest (D-14). Bytes are interpreted
//! LOSSILY as UTF-8 rather than rejected, so mid-codepoint truncation and
//! invalid sequences reach those two steps the way a half-written config file
//! would.
//!
//! # Four chunks of the ORIGINAL BYTES — and why the order is mandatory
//!
//! This target splits `data: &[u8]` into four contiguous BYTE chunks FIRST and
//! calls `String::from_utf8_lossy` on each chunk INDEPENDENTLY. It never decodes
//! once and then slices the resulting `String`.
//!
//! Do not "optimise" the four decodes into one. `from_utf8_lossy` output
//! contains multi-byte characters — including the `U+FFFD` replacement character
//! it inserts for every invalid sequence, which is three bytes — so an arbitrary
//! computed index into that string is NOT guaranteed to be a char boundary.
//! Range-slicing a decoded `String` at its own `len() / 4` therefore panics with
//! `byte index N is not a char boundary`. (The offending expression is
//! DESCRIBED rather than written out, because an acceptance gate for this file
//! greps for the literal slicing syntax and a comment quoting it would report a
//! violation this file does not commit.) That
//! panic surfaces as a libFuzzer crash attributed to `as_skill()` while
//! `as_skill()` is innocent: a wasted debugging cycle at best, and at worst a
//! "fix" bolted onto production code that never had the defect (REVIEWS
//! finding 3 / T-126-22).
//!
//! Slicing a `&[u8]` at any in-range index is always safe, and each chunk is
//! independently lossy-decoded into a valid `String`. `split_at` is used rather
//! than range indexing so no computed integer indexes anything text-shaped at
//! all.
//!
//! # Invariants
//!
//! 1. `as_skill()` NEVER panics, whatever the bytes, across all four author-text
//!    positions (workflow name, description, step name, step guidance).
//! 2. The projected skill's name is a LEGAL agentskills slug: non-empty, at most
//!    64 characters, drawn only from `[a-z0-9-]`, and neither starting nor
//!    ending with `-`. The empty and all-punctuation inputs must therefore reach
//!    the deterministic fallback slug rather than producing an empty name, which
//!    would make the skill unaddressable.
//! 3. The rendered body ends with EXACTLY one newline — SC-5's precondition for
//!    `as_prompt_text() == body()`, and a shape a trailing-whitespace bug in any
//!    `render_*` leaf would break.
//! 4. The projection is DETERMINISTIC: two workflows built from the same bytes
//!    render byte-equal bodies. This is the invariant a unit test is least
//!    likely to catch, because the one nondeterministic accessor in the input
//!    surface is a `HashMap` whose iteration order varies per PROCESS, not per
//!    call — and D-14 makes these bytes a published supply-chain pin, so an
//!    unstable render is a digest that cannot be trusted.
//! 5. The frontmatter block is STRUCTURALLY INTACT: the body's first four lines
//!    are exactly `---`, a `name: "…"` line, a `description: "…"` line, and
//!    `---`. This is REVIEWS finding 1 / T-126-21 expressed as a property the
//!    fuzzer can break — an unescaped newline inside the description splits the
//!    `description:` line in two and the fourth line stops being `---`.
//!    (`parse_frontmatter_value` is crate-private and this target is an external
//!    crate, so the structure is asserted rather than re-parsed. The round-trip
//!    through `serde_yaml` is covered by `prop_frontmatter_roundtrips` in
//!    `src/server/skills/projection.rs`.)
//!
//! # Corpus cases worth seeding
//!
//!   - `` (empty) — every one of the four fields empty, so the name fallback and
//!     the description fallback both fire
//!   - `!!!!!!!!` — an all-punctuation name, which slugifies to nothing
//!   - `refund_flow` and a 300-byte ASCII run (the 64-character truncation)
//!   - a name that truncates to a trailing `-` at exactly 64 characters
//!   - `a: b #c` and a description carrying an embedded `\n` (finding 1)
//!   - `True`, `123`, `null`, `~` as the NAME (YAML-type-alike slugs, which are
//!     legal `[a-z0-9-]` and would defeat `validate_names`' `as_str()` guard if
//!     they were ever emitted unquoted)
//!   - invalid UTF-8, and a byte string truncated mid-codepoint
//!   - a `"` and a `\` in the description (the escape path)

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};

/// The tool the fuzzed step invokes. Held CONSTANT deliberately: the untrusted
/// surface this target exists for is author PROSE reaching the renderer, and a
/// fixed tool name keeps every byte of the input budget on that prose.
const TOOL: &str = "orders_get";

/// The legal agentskills slug alphabet (invariant 2).
const SLUG_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz0123456789-";

/// Split `data` into four contiguous byte chunks and lossy-decode each one
/// INDEPENDENTLY.
///
/// See the module header: decoding once and slicing the result panics on a
/// non-char-boundary index. `split_at` is used rather than range indexing so
/// that no computed integer ever indexes anything text-shaped. Every `min` below
/// is what keeps `split_at`'s own precondition (`mid <= len`) true for a short
/// or empty input; short inputs simply yield empty later fields, which is a
/// case worth fuzzing in its own right.
fn four_lossy_fields(data: &[u8]) -> [String; 4] {
    let quarter = data.len() / 4;

    let (first, rest) = data.split_at(quarter.min(data.len()));
    let (second, rest) = rest.split_at(quarter.min(rest.len()));
    let (third, fourth) = rest.split_at(quarter.min(rest.len()));

    [
        String::from_utf8_lossy(first).into_owned(),
        String::from_utf8_lossy(second).into_owned(),
        String::from_utf8_lossy(third).into_owned(),
        String::from_utf8_lossy(fourth).into_owned(),
    ]
}

/// Build the workflow under test from four author-supplied strings.
///
/// Pure and total, so `check_projection` can call it TWICE and compare the two
/// renders for invariant 4.
fn workflow_from(name: &str, description: &str, step: &str, guidance: &str) -> SequentialWorkflow {
    SequentialWorkflow::new(name, description)
        .argument("order_id", description, true)
        .step(
            WorkflowStep::new(step, ToolHandle::new(TOOL))
                .with_guidance(guidance)
                .bind("order"),
        )
}

/// Assert every invariant for one set of author strings.
fn check_projection(name: &str, description: &str, step: &str, guidance: &str) {
    // Invariant 1 is asserted by CONSTRUCTION: `as_skill()` is infallible, so a
    // panic anywhere below this line is the crash this target hunts.
    let skill = workflow_from(name, description, step, guidance).as_skill();

    // ── Invariant 2: the slug is a legal, addressable name ────────────
    let slug = skill.name();
    assert!(
        !slug.is_empty(),
        "projected skill name is empty, so the skill is unaddressable; input name was {name:?}"
    );
    assert!(
        slug.chars().count() <= 64,
        "projected skill name is {} characters, over the 64 limit: {slug:?}",
        slug.chars().count()
    );
    assert!(
        slug.chars().all(|c| SLUG_ALPHABET.contains(c)),
        "projected skill name carries a character outside [a-z0-9-]: {slug:?}"
    );
    assert!(
        !slug.starts_with('-') && !slug.ends_with('-'),
        "projected skill name must not start or end with a hyphen: {slug:?}"
    );

    // ── Invariant 3: exactly one trailing newline (SC-5) ──────────────
    let body = skill.body();
    assert!(
        body.ends_with('\n'),
        "projected body must end with a newline; it ended {:?}",
        body.chars().rev().take(4).collect::<String>()
    );
    assert!(
        !body.ends_with("\n\n"),
        "projected body must end with EXACTLY one newline, not a blank line"
    );

    // ── Invariant 5: the frontmatter block is structurally intact ─────
    //
    // Checked before invariant 4 so that a finding-1 style injection is reported
    // as the injection it is, rather than as whichever render happened first.
    let mut lines = body.lines();
    assert_eq!(
        lines.next(),
        Some("---"),
        "the body must OPEN with a frontmatter delimiter; body began:\n{body}"
    );
    let name_line = lines
        .next()
        .expect("a frontmatter block has a name line; body was truncated");
    assert!(
        name_line.starts_with("name: \"") && name_line.ends_with('"'),
        "the name line must be a QUOTED scalar on ONE line, got {name_line:?}"
    );
    let description_line = lines
        .next()
        .expect("a frontmatter block has a description line; body was truncated");
    assert!(
        description_line.starts_with("description: \"") && description_line.ends_with('"'),
        "the description line must be a QUOTED scalar on ONE line — an unescaped \
         newline in the author's description splits it in two — got {description_line:?}"
    );
    assert_eq!(
        lines.next(),
        Some("---"),
        "the frontmatter block must CLOSE on the fourth line; a third key (or a \
         split description) pushes the delimiter down. Body was:\n{body}"
    );

    // ── Invariant 4: determinism ──────────────────────────────────────
    let again = workflow_from(name, description, step, guidance)
        .as_skill()
        .body()
        .to_string();
    assert_eq!(
        body, again,
        "two projections of the same workflow rendered DIFFERENT bytes; D-14 \
         publishes these bytes as a sha256 pin, so an unstable render is an \
         untrustworthy digest"
    );
}

fuzz_target!(|data: &[u8]| {
    let [name, description, step, guidance] = four_lossy_fields(data);

    // Shape one: the four chunks land in their four natural positions.
    check_projection(&name, &description, &step, &guidance);

    // Shape two: the SAME bytes rotated one position, so a chunk that happened
    // to be short in the name slot also gets driven through the description
    // encoder — the escape path — without the fuzzer having to rediscover the
    // input at a different length.
    check_projection(&description, &guidance, &name, &step);
});
