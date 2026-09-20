---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
reviewed: 2026-09-04T21:02:47Z
depth: standard
files_reviewed: 21
files_reviewed_list:
  - src/server/skills/mod.rs
  - src/server/skills/projection.rs
  - src/server/workflow/sequential.rs
  - src/server/workflow/prompt_handler.rs
  - src/server/workflow/task_prompt_handler.rs
  - src/server/builder.rs
  - src/server/mod.rs
  - tests/skills_integration.rs
  - tests/skills_routing.rs
  - tests/golden/workflow_skill_projection.md
  - examples/s56_workflow_skill_projection.rs
  - fuzz/fuzz_targets/fuzz_workflow_projection.rs
  - fuzz/Cargo.toml
  - crates/pmcp-package/tests/attestation_opacity.rs
  - crates/pmcp-package/Cargo.toml
  - crates/pmcp-package/tests/attestation_opacity.proptest-regressions
  - Cargo.toml
  - Makefile
  - CHANGELOG.md
  - .gitattributes
  - .github/workflows/fuzz.yml
findings:
  critical: 1
  warning: 8
  info: 0
  total: 9
status: issues_found
---

# Phase 126: Code Review Report

**Reviewed:** 2026-09-04T21:02:47Z
**Depth:** standard
**Files Reviewed:** 21
**Status:** issues_found

## Summary

The projection is well-factored and the feature gating is correct: `ServerCoreBuilder::prompt_workflow`
is `#[cfg(not(target_arch = "wasm32"))]`, so the asymmetric field gate at `builder.rs:150` compiles in
every configuration, and the `#[cfg(not(feature = "skills"))]` twin of `projected_prepend`
(`prompt_handler.rs`) genuinely keeps the two handler branches on one statement sequence. The two
prompt-handler branches DO stay in sync — `TaskWorkflowPromptHandler`'s degrading path delegates to
`inner.handle`, and its independent path reads the same `projected_prepend()` producer. The
`#[allow(unreachable_patterns)]` arms are load-bearing (both enums are genuinely `#[non_exhaustive]`,
so the allows suppress a real in-crate lint). `cargo check -p pmcp --features
"skills,streamable-http,http-client,testing" --lib --tests` is clean.

One BLOCKER: the frontmatter encoder's escape predicate is `char::is_control()` (Unicode `Cc`), but the
YAML parser this crate resolves (`serde_yaml 0.9.34+deprecated` → `unsafe-libyaml 0.2.11`) also treats
`U+2028` and `U+2029` as line breaks. I reproduced the resulting parse failure and the silent
value corruption against the exact resolved dependency versions. Every guard the phase built — the
unit tests, `prop_frontmatter_roundtrips`, the fuzz target's structural check, and the wire test — is
structurally incapable of finding it, which is the shape of defect this phase's own context asked for.

The remaining eight findings are quality/robustness: a predicate asymmetry between the two projection
paths, three proptest generators that cannot produce the character class they claim to cover, raw
author text interpolated into the markdown body where the frontmatter is carefully encoded, and a
485-line assertion-bearing example that nothing in the gate ever runs.

## Critical Issues

### CR-01: `yaml_double_quoted` does not escape U+2028/U+2029, so a projected skill can be silently dropped from `skills/list`

**File:** `src/server/skills/projection.rs:287-309` (escape predicate at `:300`)
**Also:** `src/server/skills/mod.rs:1751-1806` (`parse_frontmatter_value`), `src/server/skills/projection.rs:66-91` (the claim this violates)

**Issue:**

`yaml_double_quoted` escapes `\\`, `"`, `\n`, `\r`, `\t`, and then everything for which
`char::is_control()` is true. `char::is_control()` is exactly the Unicode `Cc` category
(`U+0000..=U+001F`, `U+007F..=U+009F`) — the in-source comment at `:297-299` states this correctly and
draws the wrong conclusion from it. The YAML parser this crate actually uses is **YAML 1.1**:
`Cargo.toml:158` pins `serde_yaml = "0.9"`, which resolves to `0.9.34+deprecated` and delegates to
`unsafe-libyaml 0.2.11`, whose `IS_BREAK_AT!` macro
(`~/.cargo/registry/src/*/unsafe-libyaml-0.2.11/src/macros.rs:253-265`) treats **five** sequences as
line breaks: `\r`, `\n`, `U+0085`, `U+2028` and `U+2029`. `U+0085` is `Cc` and is escaped. `U+2028`
(`Zl`) and `U+2029` (`Zp`) are **not** `Cc`, `is_control()` returns `false` for both, and they are
emitted verbatim into the double-quoted scalar.

`parse_frontmatter_value` builds its YAML block with Rust's `str::lines()`, which splits on `\n` only —
so the character stays inside the `description:` line and reaches libyaml, which then scans it as a
line break.

Two distinct failures result. Both were **measured** against `serde_yaml 0.9.34+deprecated` using a
verbatim copy of `yaml_double_quoted` and of `parse_frontmatter_value`'s block-extraction loop:

1. **Silent exclusion from `skills/list` (the serious one).** When the text following `U+2028`/`U+2029`
   begins with `--- ` or `... `, libyaml reports
   `found unexpected document indicator at line 3 column 1, while scanning a quoted scalar at line 2
   column 14`. That becomes `FrontmatterParse::Invalid` → `SkillDiagnostic::FrontmatterInvalid` →
   the skill is **excluded from `skills/list`** while `Skills::into_handler()` still returns `Ok`, and
   `validate_names` `continue`s past it because `artifact.frontmatter` is `None`. The server builds,
   serves the SKILL.md over `resources/read`, and the SEP-2640 discovery surface silently does not
   carry it. This is precisely the failure the module header claims to have eliminated:

   > "an ordinary string like `Refund an order: fast path` break the YAML parse — which the registry
   > DOWNGRADES to a diagnostic rather than an error, silently skipping the name-identity check instead
   > of enforcing it. **Encoding both values unconditionally removes the whole class.**"
   > (`projection.rs:83-86`)

   The class is not removed; only its `Cc` half is.

2. **Silent value corruption.** Whitespace adjacent to `U+2028`/`U+2029` is consumed as
   line-break-adjacent blanks: `"a\u{2028}   b"` round-trips as `"a\u{2028}b"`, and
   `"a   \u{2028}b"` likewise. The `frontmatter.description` a conforming host reads therefore differs
   from `Skill::resolved_description()` (the value `prompts/list` reports) and from
   `SequentialWorkflow::description()`, breaking `ProjectionOutput`'s verbatim-description contract
   (D-13) for a payload whose bytes are a published `sha256` pin (D-14).

**Why no existing guard can catch this:**

- `prop_frontmatter_roundtrips` (`:2413`) draws `description in ".*"`. `.` in `regex-syntax` is
  `[^\n]` over the full Unicode range, so `U+2028` is reachable with probability ~1e-6 per generated
  character — it will never be sampled in 256 cases of short strings.
- `yaml_double_quoted_escapes_in_order` (`:1618`) enumerates `\\ " \n \r \t \x00 \x7f` and a non-ASCII
  string; no line-separator case.
- `fuzz_workflow_projection.rs:195-221` checks frontmatter structure with `body.lines()`, which does not
  split on `U+2028`, and never parses the YAML — so even a fuzz hit on the byte sequence `E2 80 A8`
  passes every invariant.
- `projected_workflow_skill_survives_an_adversarial_description_on_the_wire`
  (`tests/skills_routing.rs`) fixes its fixture to `": "`, `"#"` and `"\n"`.

**Fix:**

```rust
// src/server/skills/projection.rs, inside yaml_double_quoted's match:
'\\' => out.push_str("\\\\"),
'"'  => out.push_str("\\\""),
'\n' => out.push_str("\\n"),
'\r' => out.push_str("\\r"),
'\t' => out.push_str("\\t"),
// YAML 1.1 (unsafe-libyaml `IS_BREAK_AT!`) treats U+2028/U+2029 as LINE BREAKS,
// and neither is Unicode `Cc`, so `is_control()` below does not reach them.
// Unescaped they either fold surrounding blanks out of the value or, when the
// following text starts with `---`/`...`, fail the parse outright and silently
// exclude the skill from skills/list.
'\u{2028}' => out.push_str("\\L"),   // or "\\u2028"
'\u{2029}' => out.push_str("\\P"),   // or "\\u2029"
_ if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
_ => out.push(c),
```

All four escape spellings (`\\L`, `\\P`, `\\u2028`, `\\u2029`) were verified to round-trip through
`serde_yaml 0.9.34` for the `"a<LS>--- b"` case that currently fails to parse.

Add a deterministic regression alongside the existing eight adversarial-description tests, e.g.
`frontmatter_survives_a_line_separator_before_a_document_indicator`, asserting
`assert_description_roundtrips("a\u{2028}--- b")` and
`assert_description_roundtrips("a\u{2028}   b")`. Widen the fuzz target's invariant 5 or add a
crate-internal proptest that mixes `U+2028`/`U+2029` into the generator explicitly (a `.*` generator
cannot be relied on to reach them). The render only moves for inputs containing these two codepoints,
so the golden does not change — but the fix IS a render change under D-14 and needs the CHANGELOG line.

## Warnings

### WR-01: the two projection paths use different emptiness predicates, so a whitespace-only description slips through `as_skill()`

**File:** `src/server/skills/projection.rs:571-577` and `:621-625` vs `:1199`

**Issue:** `resolve_description` and `project_with_notices` test `wf.description().is_empty()`;
`SkillProjection::build()` tests `workflow.description().trim().is_empty()`. For
`SequentialWorkflow::new("refund_flow", "   ")`:

- `build()` returns `Err` ("has an empty description").
- `as_skill()` substitutes **nothing**, emits **no** `ProjectionNotice::EmptyDescription` and **no**
  `tracing::warn!`, and renders `description: "   "` plus a body heading of `#    ` — a
  content-free markdown heading in a document whose entire purpose is to be read.

The module header (`:93-99`) and `build()`'s `# Errors` (`:1147-1149`) both describe these as "the same
input, two dispositions", which is false for this input. Relatedly, the stated rationale — "`description:`
with nothing after it renders a YAML null" (`:96`, `:562`, `:1148`) — is stale since
`yaml_double_quoted` was introduced: an empty description now renders `description: ""`, an empty
*string*, not null. The substitution is still the right call; the reason recorded for it no longer
describes the code.

**Fix:** make the infallible path use the same predicate:

```rust
fn resolve_description(wf: &SequentialWorkflow, slug: &str) -> String {
    if wf.description().trim().is_empty() {
        fallback_description(slug)
    } else {
        wf.description().to_string()
    }
}
```

and mirror it in `project_with_notices`'s notice condition. Add
`assert_eq!(tracer_workflow("refund_flow", "   ").as_skill().resolved_description(),
"Projected from the refund-flow workflow.")`. Update the "renders a YAML null" phrasing in the three
places it appears.

### WR-02: three `".*"` proptest generators cannot produce `\n`, the escape they most need to cover

**File:** `src/server/skills/projection.rs:2247` (`prop_sc1_slug_is_agentskills_legal`), `:2287`
(`prop_no_panic_on_arbitrary_text`), `:2413` (`prop_frontmatter_roundtrips`)

**Issue:** `.` in `regex-syntax` (which proptest uses, with default flags — no
`dot_matches_new_line`) is `[^\n]`. Every one of these three properties is documented as covering
"arbitrary text": `prop_no_panic_on_arbitrary_text`'s doc says "Arbitrary text reaches the name, the
description, an argument description, an instruction and a step's guidance"; the module header calls
`prop_frontmatter_roundtrips` a "real check rather than a mirror of the encoder's own assumptions"
(`:88-91`). None of them can ever generate a newline, so `yaml_double_quoted`'s `'\n' =>
out.push_str("\\n")` arm — the single arm on which the newline-injection defence rests — is never
exercised by the property layer. It IS covered by
`frontmatter_survives_a_newline_injection_attempt` and by the wire test, so this is not a coverage
hole; it is a claim in three rustdoc blocks that the generator does not honour, of exactly the shape
this repo has recorded as "assertions that fire on something other than the property under test".

**Fix:** use a generator that includes the escape classes, e.g.
`description in "(?s).*"` (dot-matches-newline), or an explicit union such as
`proptest::string::string_regex("[\\PC\\n\\r\\t\\u{2028}\\u{2029}]{0,64}")`. Whichever is chosen, say so
in the rustdoc instead of calling `".*"` arbitrary.

### WR-03: author text is YAML-encoded for the frontmatter and interpolated raw into the markdown body

**File:** `src/server/skills/projection.rs:587-600` (`render_body`'s `# ` heading) and `:548-557`
(`render_closing`)

**Issue:** `render_body` pushes `"# "` followed by the **raw** description, and `render_closing`
interpolates the **raw** workflow name into two backticked code spans. The frontmatter half of the same
strings goes through `yaml_double_quoted` precisely because these are unvalidated author strings
(`SequentialWorkflow::new` validates nothing), yet the body half receives no normalization at all. A
description carrying a newline — which the phase's own wire fixture
`ADVERSARIAL_DESCRIPTION = "Refund an order: fast path #urgent\nmetadata: injected"` does — renders:

```
# Refund an order: fast path #urgent
metadata: injected
```

so the continuation becomes free-standing prose in a document a model reads as instructions. A
description containing `\n## Procedure` or `\n---` synthesizes a heading or a horizontal rule inside the
digested body; a workflow name containing a backtick breaks the code spans in the closing paragraph. The
render stays deterministic, so this is not a D-14 problem — it is a fidelity/robustness problem, and no
test asserts anything about the body's shape for those inputs (the wire test asserts only the
frontmatter and the digest).

**Fix:** normalize the two body interpolations, e.g. collapse control characters and line breaks to
spaces for the `# ` heading and for the two `` `{name}` `` spans:

```rust
fn markdown_single_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() || c == '\u{2028}' || c == '\u{2029}' { ' ' } else { c })
        .collect()
}
```

This is a render change under D-14 (CHANGELOG + golden re-record) — but only for inputs that already
produce a malformed document.

### WR-04: the 485-line example is compiled but never executed by anything

**File:** `examples/s56_workflow_skill_projection.rs`, `Cargo.toml` (the `[[example]]` stanza),
`Makefile:204`, `scripts/run-example-builds.sh:150`

**Issue:** The example carries ~20 `assert*!` calls and its own module doc states "Every item is
**asserted before it is printed** — a silently-passing example that prints 'OK' while the invariant is
broken is worse than no example at all." Nothing in the repository ever runs it:

- `scripts/run-example-builds.sh:150` runs `cargo build -p pmcp --all-features --examples` — build only.
- `make check` (`Makefile:204`) runs `cargo check --features "full" --examples`; `skills` is in neither
  `full` nor `full-v2`, so `required-features = ["skills", "full"]` is unsatisfied and cargo **silently
  skips** this target there.
- `grep -rn "s56" tests/ scripts/ Makefile` returns nothing — neither
  `tests/docs04_examples_run.rs` nor `tests/docs06_v2_examples_run.rs` spawns it.
- `make lint-skills` is `--lib --tests`, so the example is also outside the pedantic/nursery reach that
  leg exists to add.

CLAUDE.md's ALWAYS list names `cargo run --example feature_name` as a per-feature requirement. As
shipped, every assertion in this file is dead weight in the gate.

**Fix:** either add an entry to the existing example-run harness (`tests/docs06_v2_examples_run.rs`
pattern, `#![cfg(feature = "skills")]`-gated so it lands under `make test-skills`'s
`--test` selectors), or add a `run-s56` recipe chained into `test-skills`. Also reconsider whether
`full` belongs in `required-features` — the example uses `Server::builder`, `SimpleTool`,
`ResourceHandler` and `Skills`, and dropping `full` would at minimum let `make check --features
"skills"` reach it.

### WR-05: `## Procedure` is emitted unconditionally, contradicting the module's own empty-section rule

**File:** `src/server/skills/projection.rs:530-536`

**Issue:** `render_context` (`:369-372`) and `render_inputs` (`:413-416`) both return `String::new()`
when their collection is empty, with the stated rationale "an empty heading is noise in a document whose
bytes are a published digest" (`:365-366`). `render_procedure` applies no such guard: a workflow with
zero steps renders `"## Procedure\n\n"` immediately followed by
`"## Server-accelerated alternative"`. `as_skill()` does not call `SequentialWorkflow::validate()`, so
this is reachable from any caller that constructs a workflow without steps and projects it before
registering it.

**Fix:** either mirror the guard —

```rust
fn render_procedure(wf: &SequentialWorkflow) -> String {
    if wf.steps().is_empty() {
        return String::new();
    }
    ...
}
```

— or write down why `## Procedure` is deliberately unconditional (e.g. "a skill without a procedure is
not a skill; the empty heading is the tell"). Either is defensible; the silent inconsistency is not.
Note that changing this is a D-14 render change for step-less workflows only.

### WR-06: `sanitize_for_log` shares CR-01's `Cc`-only assumption

**File:** `src/server/skills/projection.rs:264-268`

**Issue:** T-126-01's log-injection mitigation replaces `char::is_control()` characters with `U+FFFD`.
`U+2028` and `U+2029` are not `Cc` and survive into the `workflow`, `step` and `tool` `tracing` fields
at `:650-666` and `:1231-1240`. Several log processors and JS-based log viewers treat LS/PS as record
or line terminators, which is the same forged-record risk the function exists to close. The
accompanying test `sanitize_for_log_replaces_control_characters` (`:1441`) only covers `\n` and `\0`.

**Fix:**

```rust
fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() || c == '\u{2028}' || c == '\u{2029}' { '\u{fffd}' } else { c })
        .collect()
}
```

and extend the unit test to assert `sanitize_for_log("a\u{2028}b") == "a\u{fffd}b"`.

### WR-07: the flag-on transcript emits two consecutive `Role::User` messages and nothing pins the role sequence

**File:** `src/server/workflow/prompt_handler.rs` (`projected_prepend` returns
`PromptMessage::user(...)`; the prepend lands ahead of `create_user_intent`), and the same sequence in
`src/server/workflow/task_prompt_handler.rs:700-712`

**Issue:** With `with_projected_skill_prepend(true)` the transcript becomes
`[User: SKILL.md] [User: intent] [Assistant: announce] [User: result] ...`. The setter's rustdoc
enumerates `[0]`, `[1]` and `[2..]` in detail but never mentions that the result is two adjacent
user-role messages where every previous transcript alternated from message `[1]` onward. Hosts that map
a `prompts/get` result straight onto a chat-completions payload have historically had to merge or reject
consecutive same-role turns, so this is a transcript-shape change worth stating explicitly. No test
asserts anything about the role sequence as a whole — `flag_on_message_zero_is_the_skill_body` and
`flag_on_keeps_user_intent_at_index_one` each assert one message's role in isolation.

**Fix:** document the adjacency in `with_projected_skill_prepend`'s "With the flag ON, the transcript is"
list, and add an assertion over the whole role vector in
`tests/skills_integration.rs::flag_on_message_zero_is_the_skill_body`, e.g.

```rust
let roles: Vec<_> = result.messages.iter().map(|m| m.role).collect();
assert_eq!(roles[..2], [Role::User, Role::User]);
```

so the shape is pinned rather than incidental. (If adjacency turns out to be a real host problem, the
alternative is `PromptMessage::assistant` for `[0]`, which is a bigger decision than a review can make.)

### WR-08: dead `.min()` guards in the fuzz target's chunker

**File:** `fuzz/fuzz_targets/fuzz_workflow_projection.rs:125-129`

**Issue:**

```rust
let quarter = data.len() / 4;
let (first, rest) = data.split_at(quarter.min(data.len()));
```

`quarter` is `data.len() / 4`, so `quarter <= data.len()` always holds and the first `.min()` can never
change the value. The function's own doc claims "Every `min` below is what keeps `split_at`'s own
precondition (`mid <= len`) true for a short or empty input", which is true for the second and third
calls (`rest` shrinks) and false for the first. A reader auditing this file for the panic-safety
argument it makes at length will spend time deciding whether the first guard matters.

**Fix:** drop the redundant `.min(data.len())` on the first `split_at` and narrow the doc sentence to the
two calls that need it, or leave the call and correct the doc to say the first `min` is defensive rather
than load-bearing.

---

## Verified — not defects

Recorded so a later reviewer does not re-litigate them:

- `builder.rs:150`'s `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` field against
  `builder.rs:1324`'s `#[cfg(feature = "skills")]` read is sound: `prompt_workflow` is itself
  `#[cfg(not(target_arch = "wasm32"))]` (`builder.rs:1276`), so the read is unreachable on wasm32.
- `crates/pmcp-package`'s `unicode-normalization = "0.1"` is correctly in `[dev-dependencies]` and
  resolves to the single `0.1.25` already in the lock via `olpc-cjson`. The NFC-modulo assertion does not
  weaken the inertness property it guards.
- `.gitattributes`' `tests/golden/** text eol=lf` covers exactly one file, which is already LF-only, so
  it introduces no renormalization churn.
- `serde_json`'s `preserve_order` is enabled unconditionally at `Cargo.toml:119`, so
  `render_data_source`'s `Constant` key order is stable for every consumer, not only for this workspace.
- `#[allow(unreachable_patterns)]` at `projection.rs:358` and `:474` suppresses a real in-crate lint
  (`PromptContent` and `DataSource` are both genuinely `#[non_exhaustive]`), so it is not a no-op allow.
- The `TaskWorkflowPromptHandler` degradation path (`task_prompt_handler.rs:676-679`) delegates to
  `inner.handle`, so the prepend cannot be lost on that branch.

---

_Reviewed: 2026-09-04T21:02:47Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
