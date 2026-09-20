# Chapter 23 Exercises

These exercises build your fluency with PMCP Skills (SEP-2640). Each one targets a specific skill from the chapter, ordered from mechanical setup (Tier 1) to composition with another advanced feature (Tier 3).

## Exercise 1: Register a Single-File Skill (`hello-world`)

**Difficulty:** Introductory (10 min)

Practice the mechanical steps of wiring up a single-file skill, with no
supporting references. The goal is to build a binary that registers
`skill://hello-world/SKILL.md` on a real `Server` and prints confirmation
of the registered URIs.

**Steps:**

1. Create a new binary project (`cargo new --bin skills-hello`) or add a
   `[[bin]]` target to an existing crate.
2. Add `pmcp = { version = "2", features = ["skills", "full"] }` to
   `Cargo.toml`.
3. Create a skill body string. The minimum body is a
   frontmatter-prefixed Markdown document, e.g.:
   ```text
   ---
   name: hello-world
   description: Demonstrates the simplest possible MCP skill
   ---

   # Hello World Skill

   When the user greets the agent, respond warmly and offer to help.
   ```
4. Call `pmcp::Server::builder().name("hello-skills").version("0.1.0").skill(Skill::new("hello-world", body)).build()?`.
5. Print a confirmation line naming the expected registered URI:
   `skill://hello-world/SKILL.md`.
6. Also print the SEP-2640 discovery projection — the value a `skills/list`
   response carries. Build a separate `Skills` registry holding the same
   skill and call `entries()` on it (before `into_handler()`, which
   consumes the registry):
   ```rust,ignore
   let registry = pmcp::server::skills::Skills::new().add(skill.clone());
   let entries = registry.entries()?;
   println!("{} conforming entry: {}", entries.len(), entries[0].uri());
   ```

### Verify your solution

Run the binary. The exercise passes when the printed output names
`skill://hello-world/SKILL.md` AND reports exactly ONE conforming entry
whose `uri()` is that same URI. If the URI is missing from the printed
list, the registration is incomplete — most likely you forgot to call
`.skill(...)` on the builder.

If the URI lists but the entry count is **zero**, the registration
succeeded and discovery did not: your body has no frontmatter block, so
the skill was excluded from the projection (a build-time warning names
it). If `entries()` returns an `Err` instead, your frontmatter `name` does
not equal the final segment of the resolved URI — the URI is the identity
and the frontmatter must agree with it.

For an executable reference, see
[`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs)
and the `skill://hello-world/SKILL.md` line in its printed output.

**Questions to answer:**

- What is the default URI for a skill registered with `Skill::new("foo",
  body)` and no `.with_path(...)` call?
- What happens if you call `.skill(Skill::new("foo", ...))` twice with the
  same name on the same builder? (Hint: read
  `.planning/phases/80-sep-2640-skills-support/80-CONTEXT.md` D-5, and
  experiment with `Skills::into_handler()` directly to confirm what error
  surfaces.)

---

## Exercise 2: Register a Multi-File Skill with References (`refunds` Tier)

**Difficulty:** Intermediate (25 min)

Build a refunds skill with a `SKILL.md` body plus one or two
`references/*.md` supporting files. Demonstrate the §9 invariant: the
references appear in `resources/read` but NOT in `resources/list`.

**Steps:**

1. Create three files in your project:
   - `skills/refunds/SKILL.md`
   - `skills/refunds/references/policy.md`
   - `skills/refunds/references/examples.md`
2. Use `include_str!` to embed each at compile time.
3. Construct the skill:
   ```rust,ignore
   let refunds = Skill::new("refunds", REFUNDS)
       .with_path("acme/billing/refunds")
       .with_reference(SkillReference::new(
           "references/policy.md",
           "text/markdown",
           POLICY,
       ))
       .with_reference(SkillReference::new(
           "references/examples.md",
           "text/markdown",
           EXAMPLES,
       ));
   ```
4. Register via `.skill(refunds)` on the server builder and `.build()`
   the server.
5. Build a `SkillsHandler` separately (the same skill, but exposed as a
   `ResourceHandler` directly) so you can drive `list` and `read` from
   your test code:
   ```rust,ignore
   let handler = Skills::new().add(refunds.clone()).into_handler()?;
   ```
6. Call `handler.list(None, extra.clone()).await?` and
   `handler.read("skill://acme/billing/refunds/references/policy.md", extra.clone()).await?`.
7. Assert: the list contains the SKILL.md URI and NOTHING else — not
   either reference URI, and not any synthesized entry. `len() == 1` is
   the assertion to write.
8. Assert: the read of `references/policy.md` returns the file's body.
9. Assert on the discovery projection: call `entries()` on a registry
   holding the same skill (before `into_handler()` consumes it) and check
   that the single entry's `resources()` manifest DOES name every
   reference URI, each with a `sha256:`-prefixed `digest()` and a `size()`
   equal to the body length the read in step 8 returned.

### Verify your solution

Wrap your assertions in a `cargo test` (or `cargo run` with `assert!` in
`main`). The exercise solution is verified when ALL THREE of these
assertions pass simultaneously:

- (a) `resources/list` for the SkillsHandler returns URIs that EXCLUDE
  every `references/*.md` URI, and has length 1.
- (b) `resources/read` on a reference URI (e.g.
  `skill://acme/billing/refunds/references/policy.md`) returns a content
  body byte-equal to the embedded reference file.
- (c) the `skills/list` entry projection INCLUDES that same reference URI
  in its `resources` manifest, with a size matching (b)'s body length.

If only (b) passes, you forgot the §9 filter check. If only (a) passes,
your read path is wrong — likely a typo in the URI you passed. (c) is the
half that keeps the rule from being misread as "references are hidden":
they are not listed, but they are fully described by discovery.

For a working reference of the read path (including how to pattern-match
`Content::Resource` correctly), see
[`examples/c10_client_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/c10_client_skills.rs).

**Questions to answer:**

- Why does SEP-2640 §9 require this filtering? What attack or UX problem
  would arise if references were enumerated alongside SKILL.md entries?
- What MIME type would you set for a `.graphql` reference file? (Hint:
  search the example with
  `grep -n '\.graphql' examples/s44_server_skills.rs` and compare against
  what your code does.)

---

## Exercise 3: Dual-Surface Bootstrap for a Code-Mode Skill (`code-mode` Tier)

**Difficulty:** Advanced (45 min)

Bring it all together. Register the `code-mode` skill from
`examples/skills/code-mode/` (or your own copy) and ALSO publish a prompt
fallback via `bootstrap_skill_and_prompt`. Build a tiny client driver
that exercises BOTH host flows in the same process and asserts
byte-equality between the concatenated SEP-2640 read results and the
prompt body. That assertion is the runtime proof of the dual-surface
invariant.

**Steps:**

1. Copy or `include_str!` `examples/skills/code-mode/SKILL.md` and its
   three references (`schema.graphql`, `examples.md`, `policies.md`) into
   your project.
2. Construct the `Skill` with all three references, matching the
   `build_code_mode_skill()` function in `examples/s44_server_skills.rs`.
3. Register the skill via `bootstrap_skill_and_prompt`:
   ```rust,ignore
   let server = pmcp::Server::builder()
       .name("exercise3")
       .version("0.1.0")
       .bootstrap_skill_and_prompt(skill.clone(), "start_code_mode")
       .build()?;
   ```
4. Retrieve the registered prompt handler via
   `server.get_prompt("start_code_mode")` and invoke
   `.handle(HashMap::new(), extra).await?` to obtain the legacy host's
   prompt body.
5. Separately, build the SEP-2640 host flow:
   `let handler = Skills::new().add(skill.clone()).into_handler()?;`.
   Walk `list` and `read` for SKILL.md plus each reference URI in
   registration order, concatenating with `\n--- {relative_path} ---\n`
   separators (the same shape as `Skill::as_prompt_text()` produces).
6. Assert byte-equality between the two concatenated results:
   ```rust,ignore
   assert_eq!(sep_2640_text, prompt_text);
   ```

### Verify your solution

The `assert_eq!` between the two concatenated byte strings passes. If it
fails, the dual-surface invariant is broken — and your understanding of
what `Skill::as_prompt_text()` produces is wrong. (This is the exact
assertion `examples/c10_client_skills.rs` makes; you have a reference
implementation to compare against if you get stuck. The example panics
on failure rather than printing OK — copy that posture in your own
solution.)

**Try this:** Run `cargo run --example c10_client_skills --features
skills,full` and watch its output. The "Byte-equality assertion" block
at the end is exactly the check your Exercise 3 solution is implementing.

**Questions to answer:**

- Why does the chapter (and Phase 80) call the byte-equality property
  "load-bearing"? What silent-failure mode does it prevent on
  SEP-2640-blind hosts?
- If you registered the `code-mode` skill but did NOT call
  `bootstrap_skill_and_prompt` (just `.skill(skill)`), what would change
  for a host that doesn't support SEP-2640? (Hint:
  `.planning/phases/80-sep-2640-skills-support/80-CONTEXT.md` D-7 and the
  "pointer-style silent-failure" discussion in the chapter.)

---

## Prerequisites

Before starting these exercises, ensure you have:

- Completed Chapter 23 (Skills) including the dual-surface invariant
  discussion.
- A working Rust development environment with `pmcp 2.x` and the
  `skills` feature available.
- The PMCP examples checked out so you can compare against
  `s44_server_skills.rs` and `c10_client_skills.rs`.

## Next Steps

After completing these exercises, continue to:

- [Chapter 22 Exercises](./ch22-exercises.md) -- Code Mode hands-on
  practice (course ordering: Code Mode appears as Chapter 22, Skills as
  Chapter 23, per CONTEXT.md D-05).
- [Appendix B: Template Gallery](../appendix/template-gallery.md) --
  Production-ready templates including skill-enabled servers.
