# Skills: Agent Workflow Instructions (SEP-2640)

Agent-workflow instructions belong on the server, not buried in client code.
That sentence is the entire premise of SEP-2640 — and the entire premise of
PMCP's Skills feature. A Skill is a structured, versionable, discoverable
piece of agent context: a `SKILL.md` plus optional supporting files,
published as MCP resources through the existing `resources/*` primitives.
There are no new RPC methods to learn. There is one additive capability
declaration and three builder methods. PMCP's Skills support is feature-gated
behind `skills`, ships in v2, and pairs with a dual-surface bootstrap so
SEP-2640-blind hosts keep working. This chapter walks the same three-tier
example shipped at `examples/s44_server_skills.rs`, then hands you exercises
in `ch23-exercises.md` and a comprehension quiz at the bottom of this page.

## Learning Objectives

By the end of this chapter, you will be able to:

- Explain the **dual-surface invariant** and articulate why a pointer-style
  prompt body silent-fails on SEP-2640-blind hosts (the load-bearing design
  property of PMCP's Skills implementation).
- Register a single-file Skill on a server builder using
  `pmcp::Server::builder().skill(...)`.
- Register a multi-file Skill (`SKILL.md` + references) using
  `Skill::with_reference(...)` and explain SEP-2640 §9 visibility filtering
  (references readable via `resources/read`, never listed via
  `resources/list`).
- Use `bootstrap_skill_and_prompt(skill, prompt_name)` to publish a Skill
  that also exposes a prompt fallback for hosts that don't yet support
  SEP-2640.
- Identify which working example in the PMCP repository demonstrates each
  skill tier (`hello-world` for trivial, `refunds` for real-world,
  `code-mode` for composition with another advanced feature).
- Cite the SEP-2640 reference implementations the PMCP example structure
  deliberately matches (TS SDK, gemini-cli, fast-agent, goose, codex) and
  explain why side-by-side comparability matters for enterprise deployments.

## Why Skills Matter for Enterprise MCP

Without Skills, agent-workflow instructions tend to live as system prompts
inside client-side code. That is a poor home for them. System prompts are
hard to version (no commit history per workflow), hard to ship cross-host
(every client embeds its own copy), and drift between teams (the
"refund-handling instructions" in your support chatbot diverge from the
ones in your operations console). Worst of all, the people who *understand*
the workflow — the server authors who own the underlying tools — have no
authoritative way to publish the agent-facing instructions that describe
how to call those tools correctly.

With Skills, instructions are server-published *resources*. They are
discoverable via the standard `resources/list` call, readable via
`resources/read`, version-controllable as files inside the server's
repository, and reviewable through normal pull-request flow. The dual-surface
bootstrap (covered in the next section) makes Skills work today, on hosts
that don't yet support SEP-2640 — there is zero "wait for the host ecosystem
to catch up" friction in adopting Skills.

```
+-------------------------------------------------------------------------+
|              Where do agent-workflow instructions live?                  |
+-------------------------------------------------------------------------+
|                                                                         |
|  Approach              Versioned?  Cross-host?  Owned by?  Discoverable?|
|  ===================== =========== ============ =========== ============|
|  Client-side prompts   Per-client  No           Each team   No          |
|  Server resources      Yes (repo)  Yes          Server      Yes (list)  |
|  Skills (SEP-2640)     Yes (repo)  Yes          Server      Yes (list)  |
|                                                                         |
|  Skills add to "server resources" the canonical structure (SKILL.md +   |
|  references), visibility filtering (§9 - top-level only in list), and   |
|  the dual-surface invariant so SEP-2640-blind hosts also work.          |
+-------------------------------------------------------------------------+
```

For enterprise deployments that target multiple agent platforms, this is
load-bearing: a Skill written once in the server's repo can be consumed by
Claude Desktop, ChatGPT, VS Code, gemini-cli, codex, and any other SEP-2640-
compatible host without per-host packaging. The dual-surface bootstrap covers
the remaining hosts that haven't yet adopted SEP-2640.

## The Dual-Surface Invariant

The single most important design property in PMCP's Skills implementation
is this: **the skill surface and the prompt surface are byte-equal by
construction.** Both are derived from one `Skill` value via the
`Skill::as_prompt_text()` method. When a server author calls
`bootstrap_skill_and_prompt(skill, prompt_name)`, PMCP registers the skill
data under both the SEP-2640 skill surface (where capable hosts will find
it via `resources/list` + `resources/read`) AND a fallback MCP prompt
surface (where SEP-2640-blind hosts will load it via `prompts/get
prompt_name`). The two surfaces *cannot drift*. They are byte-for-byte
identical because the prompt body is the literal output of
`skill.as_prompt_text()`.

Why is this load-bearing? Because the obvious alternative — a pointer-style
prompt body that says "load `skill://refunds/SKILL.md` for instructions" —
**silent-fails** on SEP-2640-blind hosts. The LLM receives a literal string
mentioning a `skill://` URI, but the host has no mechanism to fetch that
URI. The model dutifully proceeds without the workflow instructions, often
producing plausible-sounding but wrong behavior. There is no error; nothing
crashes. The agent simply behaves as if the Skill didn't exist.

The byte-equal pattern eliminates this failure mode by construction.
Here is the implementation of `Skill::as_prompt_text()` from
`src/server/skills.rs`:

```rust,ignore
pub fn as_prompt_text(&self) -> String {
    let mut out = String::new();
    out.push_str(&self.body);
    if !self.body.ends_with('\n') {
        out.push('\n');
    }
    for r in &self.references {
        out.push_str("\n--- ");
        out.push_str(&r.relative_path);
        out.push_str(" ---\n");
        out.push_str(&r.body);
        if !r.body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
```

The same concatenation rule (SKILL.md body + `\n--- {path} ---\n` separator
+ reference body, normalized to trailing newlines) is what an SEP-2640
client produces when it walks `resources/read` for the skill URIs in
registration order. The result is the same bytes. The integration test at
`tests/skills_integration.rs` asserts byte-equality at runtime in CI — the
invariant is enforced, not aspirational.

The takeaway: **the bootstrap helper makes drift between the two surfaces
structurally impossible — the skill text and the prompt text are
byte-equal by construction.** The next three tiers build on this invariant.

## Tier 1: Hello-World Skill

The first tier exists to make the mechanical steps obvious. The hello-world
skill is a single-file `SKILL.md` with no references, registered with one
builder call. From `examples/skills/hello-world/SKILL.md`:

```markdown
---
name: hello-world
description: Demonstrates the simplest possible MCP skill
---

# Hello World Skill

When the user greets the agent, respond warmly and offer to help.
```

The registration is a one-liner inside the standard `Server::builder()` chain.
From `examples/s44_server_skills.rs`:

```rust,ignore
const HELLO_WORLD: &str = include_str!("skills/hello-world/SKILL.md");

let _server = pmcp::Server::builder()
    .name("skills-demo")
    .version("0.1.0")
    .skill(Skill::new("hello-world", HELLO_WORLD))
    .build()?;
```

`Skill::new(name, body)` constructs the value. `.skill(...)` registers it
on the builder. The resulting URI is `skill://hello-world/SKILL.md` (the
default path is derived from the skill name), and it is the only entry
`resources/list` returns for this server.

**Discovery is a method pair, not a resource.** An SEP-2640 host learns
what a server offers by calling `skills/list` — and `skills/get` for a
single skill — which PMCP answers over streamable HTTP. Each response
carries one entry per skill: the SKILL.md `uri`, the skill's frontmatter
emitted verbatim, and a `resources` manifest naming every file (SKILL.md
first, then each reference) with a `sha256:` digest and a byte size.

A skill whose body carries no frontmatter is **excluded** from that
projection, with a build-time warning naming it — which is why every
snippet in this chapter gives its skill a real frontmatter block. It
remains listable and readable as a resource; it is simply not discoverable.

(Earlier versions of PMCP also synthesized a `skill://` discovery-index
resource into `resources/list`. It was retired: the methods supersede it,
and its URI violated the draft's own structure rule.)

Full example:
[`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs).

**Try this:** Run `cargo run --example s44_server_skills --features
skills,full` and observe the printed URI list. Note that no `references/...`
URI appears in that list — that's §9 visibility filtering, not a bug. We
explore it in Tier 2.

## Tier 2: Refunds Skill with References (SEP-2640 §9 Visibility Filtering)

The second tier introduces the SEP-2640 directory model. A real-world skill
typically isn't a single file — it's a `SKILL.md` plus supporting reference
files (`policy.md`, `examples.md`, an OpenAPI schema, a GraphQL schema,
etc.). The refunds skill demonstrates the pattern. From
`examples/skills/refunds/SKILL.md`:

```markdown
---
name: refunds
description: Process customer refund requests per company policy
---

# Refund Workflow

1. Verify the order ID exists.
2. Check that the request is within the 30-day window.
3. Validate the reason against the allowed-reasons list.
4. Issue the refund via the billing tool.
```

Registration uses `.with_path(...)` to namespace the skill (so
`acme/billing/refunds` becomes the URI segment instead of just `refunds`)
and would chain `.with_reference(...)` calls for each supporting file. The
real example in `s44_server_skills.rs` registers refunds without references
to keep the registration line readable; the multi-reference pattern is
exercised by the `code-mode` skill in Tier 3 and looks identical for
refunds:

```rust,ignore
.skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
```

The crucial SEP-2640 §9 rule: **supporting files are addressable via
`resources/read`, but they MUST NOT appear in `resources/list`.** That
listing is intentionally focused on top-level skills — exactly one entry
per `SKILL.md`, and nothing else. References live at predictable URIs
(e.g. `skill://acme/billing/refunds/references/policy.md`) and are
fetched by URI when the agent decides it needs them, based on what the
`SKILL.md` body tells it to fetch.

Not listed is not the same as hidden. A skill's **complete** file
manifest — every reference, each with its digest and size — is carried by
that skill's `skills/list` / `skills/get` entry. That is where a host
learns which reference URIs exist and may then read them.

The reason for the rule is UX: an agent picking a skill from
`resources/list` should see "one refund workflow" — not "one refund workflow
plus its policy.md, plus its examples.md, plus its appeals-process.md."
Listing every reference would explode the discovery surface and force the
agent to filter clutter on every list call. The skill author decides what
the agent reads, in what order, by writing it into `SKILL.md`.

PMCP enforces this in `SkillsHandler::list()` — references are simply
never returned, regardless of how many are registered. The test
`tests/skills_integration.rs` asserts that listing returns only `SKILL.md`
URIs, never reference URIs and nothing synthesized.

Full example:
[`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs).

**Try this:** After running the example, manually call `resources/list`
(via `mcp-tester stdio ./target/debug/examples/s44_server_skills` or your
preferred MCP client). Compare the listed URIs against what you'd expect
from the filesystem under `examples/skills/`. The supporting files exist on
disk and are *readable* through `resources/read`, but they are *not listed*.
Confirm this matches your reading of the chapter — and notice how the
discovery surface stays clean even as the underlying skill grows in size.

## Tier 3: Code-Mode Skill (Composition with Another Advanced Feature)

The third tier shows that Skills compose with other advanced PMCP features.
The `code-mode` skill is a multi-file skill whose body refers to three
references and whose purpose is to bootstrap an LLM into PMCP's Code Mode
feature (covered in Chapter 22). The skill teaches the agent the schema,
the canonical patterns, and the validation policies it must obey when
generating GraphQL queries.

The skill body lives at `examples/skills/code-mode/SKILL.md`:

```markdown
---
name: code-mode
description: Generate validated GraphQL queries against this server's schema
---

# Code Mode

This server exposes `validate_code` and `execute_code` tools for running
LLM-generated GraphQL queries with cryptographically signed approval tokens.

## Before you generate a query

1. Read `skill://code-mode/references/schema.graphql` for available types.
2. Read `skill://code-mode/references/examples.md` for canonical patterns.
3. Read `skill://code-mode/references/policies.md` for what's allowed.
```

The construction shows the full multi-reference pattern. From
`examples/s44_server_skills.rs`:

```rust,ignore
fn build_code_mode_skill() -> Skill {
    Skill::new("code-mode", CODE_MODE)
        .with_reference(SkillReference::new(
            "references/schema.graphql",
            "application/graphql",
            CODE_MODE_SCHEMA,
        ))
        .with_reference(SkillReference::new(
            "references/examples.md",
            "text/markdown",
            CODE_MODE_EXAMPLES,
        ))
        .with_reference(SkillReference::new(
            "references/policies.md",
            "text/markdown",
            CODE_MODE_POLICIES,
        ))
}
```

The registration uses the dual-surface bootstrap so SEP-2640-blind hosts
also get the full bootstrap context:

```rust,ignore
let _server = pmcp::Server::builder()
    .name("skills-demo")
    .version("0.1.0")
    .skill(Skill::new("hello-world", HELLO_WORLD))
    .skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
    .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")
    .build()?;
```

`bootstrap_skill_and_prompt(skill, "start_code_mode")` registers BOTH:

1. The SEP-2640 skill surface — `skill://code-mode/SKILL.md` plus the three
   reference URIs, discoverable via `resources/list`, readable via
   `resources/read`.
2. A parallel prompt named `start_code_mode` whose body is the literal
   output of `code_mode_skill.as_prompt_text()` — that is, the SKILL.md
   body concatenated with each reference body separated by `\n--- {path}
   ---\n` markers.

A capable host fetches the skill resources lazily; a blind host fetches
the prompt once. Both code paths receive byte-equal context. The client
example at `examples/c10_client_skills.rs` exercises both flows in the same
process and asserts byte-equality on exit:

```rust,ignore
assert_eq!(
    sep_2640_text, prompt_text,
    "dual-surface invariant violated: SEP-2640 read concatenation != prompt body"
);
```

If the invariant ever broke, the example would panic on exit. That is
intentional — a silently-passing demo that prints "OK" when the
load-bearing invariant is violated is worse than no demo at all.

Full example:
[`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs).
Client counterpart:
[`examples/c10_client_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/c10_client_skills.rs).

**Try this:** Run the client example with `cargo run --example
c10_client_skills --features skills,full`. Observe the byte-equality
assertion at the end of `main()`. The example *panics* if the invariant
breaks — why is panicking better than silently passing? (Hint: think about
what happens to the developer who pushes a change that subtly breaks the
invariant if the example just prints "OK" anyway.)

## Cross-SDK Compatibility (Why Three Tiers Match Other Reference Implementations)

Phase 80 deliberately chose the three-tier shape — hello-world, refunds,
code-mode — so that PMCP's developer experience can be compared
side-by-side against the SEP-2640 reference implementations in other
SDKs. The TS SDK, gemini-cli, fast-agent, goose, and codex all ship the
same two trivial-to-real-world demonstration skills. A developer
evaluating which SDK to adopt can implement the same `hello-world` and
`refunds` skill in each, count lines of code, and form a calibrated
judgment about ergonomics rather than guessing.

Cross-SDK compatibility matters for enterprise deployments that target
multiple agent platforms. A skill that works in one platform's MCP host
should work in another's. The whole point of an SEP is to lock the wire
shape so that doesn't depend on each SDK's idiosyncratic API surface.
PMCP's three-tier example structure is the most concrete commitment to
that goal: same skills, same wire shape, comparable API surface.

The deliberate addition of `code-mode` as a third tier — beyond what
other SDKs ship — demonstrates that PMCP Skills *compose* with another
advanced PMCP feature. That composition story is what lets enterprise
teams structure their server around primitives rather than ad-hoc
glue. Code Mode (Chapter 22) needs an agent-facing bootstrap that
teaches the schema, examples, and policies. Skills are the natural
mechanism. The two features were designed to compose, not to bolt
together.

You can read the canonical SEP-2640 reference implementations and
compare the hello-world / refunds shape at
[experimental-ext-skills](https://github.com/modelcontextprotocol/experimental-ext-skills).

## Future Work (Deferred from Phase 80)

Two Phase 80 non-goals are worth flagging as forward-looking notes:

1. **`#[pmcp::skill]` procedural macro.** A macro that lets server authors
   write `Skill::from_disk("skills/hello-world/SKILL.md")` (or an
   equivalent attribute-form `#[pmcp::skill("skills/hello-world")]`) and
   have PMCP load the SKILL.md and its references at compile time would
   eliminate the boilerplate `include_str!` calls visible in
   `examples/s44_server_skills.rs`. The macro is deferred; until it lands,
   server authors use `Skill::new(name, include_str!(...))` to embed
   SKILL.md at compile time.
2. **SEP-2640 §4 archive distribution.** SEP-2640 §4 describes an
   `application/gzip` blob mode for distributing skills as compressed
   archives, intended for very large skill bundles. PMCP's
   `Content::Resource` type doesn't yet carry a `blob` field; until that
   gap closes, PMCP ships text-mode skills only. The SEP marks archive
   mode optional, so this isn't a compliance blocker — but it's the
   right next step for the Skills feature.

Both are explicit Phase 80 non-goals and remain out of scope for v2.x.

## Chapter Contents

This chapter has two hands-on continuations:

1. **[Chapter 23 Exercises](./ch23-exercises.md)** -- Practice registering
   single-file skills, multi-file skills, and dual-surface bootstrapping.
   Three exercises spanning Introductory / Intermediate / Advanced
   difficulty, each with a falsifiable Verify-your-solution check.
2. **Knowledge check below** -- Quick comprehension questions before
   continuing.

## Knowledge Check

Before continuing, make sure you can answer:

- **What does the dual-surface invariant guarantee, and why does it
  require byte-equality between the skill surface and the prompt surface
  (not just semantic equivalence)?** A semantic-equivalence claim would
  drift the moment one surface re-orders references or normalizes
  whitespace differently. Byte-equality is enforceable at runtime — see
  the assertion in `tests/skills_integration.rs`.
- **Why are reference files (`skill://<name>/references/...`) addressable
  via `resources/read` but absent from `resources/list`?** SEP-2640 §9
  filters the discovery surface to top-level skills so agents pick a
  skill cleanly; the skill body decides what references the agent
  fetches, in what order.
- **What deferred Phase 80 item (mentioned in the "Future Work"
  section) would change how skill bodies are embedded into a server
  binary if implemented?** The `#[pmcp::skill]` procedural macro —
  it would let `Skill::from_disk(...)` (or an attribute form) embed
  SKILL.md and references at compile time without the `include_str!`
  boilerplate.

{{#quiz ../quizzes/ch23-skills.toml}}

---

*Continue to [Chapter 23 Exercises](./ch23-exercises.md) ->*
