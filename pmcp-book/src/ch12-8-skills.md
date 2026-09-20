# Chapter 12.8: Skills — Agent Workflow Instructions (SEP-2640)

Skills are rich, structured agent-workflow instructions discovered via the existing `resources/*` primitives — no new RPC methods, but a small additive change to `ServerCapabilities` and three new builder methods. PMCP's Skills support is feature-gated behind `skills` in `Cargo.toml`. This chapter walks the same three-tier example shipped at [`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs): a Tier 1 trivial `hello-world` skill, a Tier 2 real-world `refunds` skill with namespaced URI, and a Tier 3 `code-mode` skill that composes with PMCP's Code Mode feature via the dual-surface bootstrap helper. By the end of the chapter you will know how to register Skills, why the dual-surface invariant matters, and how to use one builder call to make drift between the SEP-2640 surface and the prompt fallback structurally impossible.

```
ServerCoreBuilder
    │
    ├── .skill(Skill)                       ◄── Tier 1 / Tier 2 registration
    ├── .skills(Skills)                     ◄── batch registration
    └── .bootstrap_skill_and_prompt(skill, prompt_name)
         │                                  ◄── Tier 3 — dual surface
         │
         ▼
   ┌─────────────────────────┐    ┌─────────────────────────┐
   │   ResourceHandler       │    │     PromptHandler       │
   │   (SEP-2640 surface)    │    │   (legacy fallback)     │
   │                         │    │                         │
   │   resources/list        │    │   prompts/get           │
   │   resources/read        │    │     → as_prompt_text()  │
   └─────────────────────────┘    └─────────────────────────┘
              │                                │
              └────────  BYTE-EQUAL  ──────────┘
                  (Skill::as_prompt_text)
```

---

## The Dual-Surface Invariant

The load-bearing design property of PMCP's Skills support is this: the *skill surface* (one or more `resources/read` results concatenated with labelled-rule separators) and the *prompt surface* (the `prompts/get` body) are byte-equal by construction, derived from one `Skill` value via `Skill::as_prompt_text()`.

Why this matters: hosts that don't yet support SEP-2640 fall back to `prompts/get` to retrieve workflow context. A "pointer-style" prompt body — one that returns a literal `skill://...` URI string and expects the host to fetch it — silent-fails on those hosts because the LLM gets a URI string but the host has no mechanism to resolve it. The dual-surface invariant forces the prompt body to inline the same content the resource reads return, so legacy hosts get the complete workflow context in one round trip with no follow-up fetch required.

The synthesis happens inside `src/server/skills.rs` — quoting the whole `as_prompt_text` function:

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

The SKILL.md body comes first, with a trailing-newline normalization. Each reference is then inlined behind a `\n--- <relative_path> ---\n` separator, again newline-normalized. The same concatenation rule is what an SEP-2640-capable host produces when it reads each resource URI and stitches them together — the invariant is that these two paths produce identical bytes.

The test `tests/skills_integration.rs` asserts byte-equality at runtime: it walks both flows (`resources/list` + `resources/read` for every URI, vs. `prompts/get` via the registered `SkillPromptHandler`) and asserts the resulting strings are equal. This invariant is enforced in CI, not aspirational.

### Three Building Blocks

The Skills surface is just three public types in `src/server/skills.rs`:

- **`Skill`** — a single skill carrying `name`, the full SKILL.md `body`, an optional URI `path` override, a parsed `description`, and zero or more `references`. Constructed with `Skill::new(name, body)`; mutated with `.with_path(...)`, `.with_description(...)`, `.with_reference(...)` (panicking) and `.try_with_reference(...)` (fallible). The `description:` line is parsed eagerly out of the YAML frontmatter so per-skill metadata reads (e.g. `resources/list`) don't re-scan the body on every request.
- **`SkillReference`** — a supporting file within a skill's directory carrying a `relative_path` (e.g. `references/schema.graphql`), a `mime_type`, and a body. Path validation rejects empty values, null bytes, the literal `SKILL.md` (which would collide with the canonical URI), `..` segments, leading slashes, and embedded URI schemes; duplicates within a skill are also rejected at builder time.
- **`Skills`** — a registry that collects multiple `Skill` values via `.add(skill)` or `.merge(other)` and flattens them into a `ResourceHandler` via `Skills::into_handler()`. Duplicate URIs (either at the SKILL.md level or at the reference level) are rejected with a `Validation` error rather than silently overwritten.

The builder method `.skill(Skill)` is a convenience over `.skills(Skills::new().add(skill))`. The builder accumulates registrations across calls and finalizes them once at `.build()` time, so a chained `.skill(...).skill(...).bootstrap_skill_and_prompt(...)` sequence works without per-call wrapper nesting.

### The Three Builder Methods

PMCP wires Skills onto both `ServerBuilder` (returned by `Server::builder()`) and `ServerCoreBuilder` (the lower-level form). The three relevant methods are:

| Method | Purpose | Surfaces Registered |
|--------|---------|---------------------|
| `.skill(Skill)` | Register one skill | SEP-2640 resource only |
| `.skills(Skills)` | Register a batch | SEP-2640 resource only |
| `.bootstrap_skill_and_prompt(Skill, &str)` | Register one skill + a parallel prompt | SEP-2640 resource AND prompt |

The first two are appropriate when the host is known to support SEP-2640 (and a prompt fallback would be redundant). The third is the dual-surface bootstrap: it registers BOTH surfaces from one `Skill` value so they cannot drift. The second argument is the prompt name (`"start_code_mode"` in the Tier 3 example) under which clients can call `prompts/get`.

Tiers 1–3 below build on this invariant. The bootstrap helper `bootstrap_skill_and_prompt(skill, prompt_name)` is what makes drift between the two surfaces structurally impossible — the skill text and the prompt text are **byte-equal** by construction.

---

## Tier 1: Hello-World Skill

Tier 1 is the smallest possible skill: one SKILL.md, no references, default URI path. This is the shape every SEP-2640 reference implementation ships as its introductory example so server authors can compare PMCP's ergonomics against TS SDK, gemini-cli, fast-agent, goose, and codex side-by-side.

The skill body lives at `examples/skills/hello-world/SKILL.md`:

```markdown
---
name: hello-world
description: Demonstrates the simplest possible MCP skill
---

# Hello World Skill

When the user greets the agent, respond warmly and offer to help.
```

Two pieces of YAML frontmatter (`name`, `description`) plus a one-line workflow body. PMCP's `Skill::new(name, body)` consumes that whole file verbatim — frontmatter intact — and parses the `description:` line eagerly so per-skill metadata reads avoid re-scanning the body on every request.

Registration in `examples/s44_server_skills.rs` is a single builder call:

```rust,ignore
let _server = pmcp::Server::builder()
    .name("skills-demo")
    .version("0.1.0")
    .skill(Skill::new("hello-world", HELLO_WORLD))
    .skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
    .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")
    .build()?;
```

(`HELLO_WORLD` is `include_str!("skills/hello-world/SKILL.md")` at the top of the file.)

`Skill::new(name, body)` is the minimum required. The default URI is `skill://<name>/SKILL.md` — so after this registration the resource is addressable at `skill://hello-world/SKILL.md`. The builder method `.skill(Skill)` is a convenience over `.skills(Skills::new().add(skill))` — when registering one skill, prefer `.skill`; when registering a batch, prefer `.skills`.

After this single registration, a `resources/list` call returns exactly one entry — one per registered SKILL.md, and nothing else:

- `skill://hello-world/SKILL.md` (mime: `text/markdown`)

That list is built once by `Skills::into_handler()` and reused on every list request; the registry is immutable post-construction, so list/read responses are precomputed rather than serialized per request.

### Discovery is `skills/list` and `skills/get`, not a resource

`resources/list` tells a host which SKILL.md documents exist. **Discovery** — the metadata a host needs to decide *which* skill to load — is the SEP-2640 method pair `skills/list` and `skills/get`, which pmcp answers over streamable HTTP. There is no discovery-index resource: earlier versions synthesized a `skill://` index into `resources/list`, and it was retired, both because the methods supersede it and because its URI violated the draft's own structure rule (its name segment is not a skill name and it has no `/SKILL.md` sibling).

A `skills/list` response looks like this on the wire:

<!-- synthetic -->
```json
{
  "resultType": "complete",
  "skills": [
    {
      "uri": "skill://hello-world/SKILL.md",
      "frontmatter": {
        "name": "hello-world",
        "description": "Demonstrates the simplest possible MCP skill"
      },
      "resources": [
        {
          "uri": "skill://hello-world/SKILL.md",
          "digest": "sha256:a5777e88496e95687177aa658f37dab174074d1dbe0622219166a9494700d43d",
          "size": 172
        }
      ]
    }
  ]
}
```

`skills/get` returns the same entry shape for a single `params.uri`, and answers `-32602` for any URI it does not serve.

The `frontmatter` object is emitted **verbatim** from the SKILL.md YAML block — not reconstructed — so a non-required field such as `license:` survives to the host untouched. The `resources` array is the skill's complete manifest: its own SKILL.md first, then every registered reference, each with a `sha256:` digest and a byte-accurate `size` over exactly the bytes `resources/read` serves for that URI. A skill whose body carries no frontmatter is excluded from this projection with a build-time warning; it remains listable and readable as a resource.

You can obtain the same values locally, without a transport, by calling `Skills::entries()` on the registry:

```rust,ignore
let registry = Skills::new().add(Skill::new("hello-world", HELLO_WORLD));
// Read the projection BEFORE `into_handler()` consumes the registry —
// `entries` borrows, `into_handler` takes ownership.
let entries = registry.entries()?;
assert_eq!(entries[0].uri(), "skill://hello-world/SKILL.md");
assert_eq!(
    entries[0].frontmatter().get("name").and_then(|v| v.as_str()),
    Some("hello-world"),
);
let handler = registry.into_handler()?;
```

That is a local read of the projection, **not** an MCP request — the same distinction `examples/c10_client_skills.rs` makes, and for the same reason: neither the snippet above nor that example holds a client transport, so neither can issue `skills/list`. The end-to-end wire proof, real POSTs against a live `StreamableHttpServer` asserted on the response bodies, is `tests/skills_routing.rs`.

Full example: [`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs)

---

## Tier 2: Refunds Skill with References (SEP-2640 §9 Visibility Filtering)

Tier 2 is a real-world skill — SKILL.md body plus optional supporting files (references) — and demonstrates `.with_path("acme/billing/refunds")` for namespaced URIs that match enterprise-internal paths.

The skill body at `examples/skills/refunds/SKILL.md`:

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

Registration with `with_path` to override the default URI:

```rust,ignore
.skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
```

The resulting resource lives at `skill://acme/billing/refunds/SKILL.md` rather than `skill://refunds/SKILL.md`. Path-overrides matter in larger workspaces where two skills might share a frontmatter `name:` (e.g. two teams both calling their workflow "refunds") and the URI needs to disambiguate them.

### SEP-2640 §9 Visibility Filtering

Refunds is the canonical place to teach SEP-2640 §9's "readable but not listable" behavior. Supporting files placed in a skill's `references/` subdirectory ARE addressable via `resources/read` — clients that already know the URI can fetch them lazily. But they MUST NOT appear in `resources/list`. This is the spec-defined contract that keeps the resource surface scoped to top-level skill entries; references are pulled in on demand, not enumerated upfront.

They are not hidden from a host, though — they are simply not *listed*. A skill's complete file manifest, digests and all, is carried by its `skills/list` / `skills/get` entry (see the `resources` array above), which is where a host learns the reference URIs it may then read.

PMCP's `SkillsHandler::list()` filters explicitly: it emits one entry per registered SKILL.md and nothing else. The example file calls this out in its module doc comment ("reference URIs ... MUST NOT appear in `resources/list`. They are intentionally absent from the printed URI list below — this is the spec-required 'readable but not listable' behavior"), and `tests/skills_integration.rs` locks the behavior in CI with a property test that walks all generated URIs and asserts none of them contain `/references/` in the listed output.

### Reference Path Validation

Every `SkillReference` runs through path validation when added to a `Skill`. The rejection list (enforced by `try_with_reference`) is deliberately small and load-bearing for security:

| Invalid Path | Why Rejected |
|--------------|--------------|
| `""` (empty) | Cannot resolve to any URI |
| Contains `\0` | Null bytes break URI handling on some hosts |
| `"SKILL.md"` | Collides with the canonical `skill://<name>/SKILL.md` URI |
| Contains `..` segment | Path-traversal escape; references must stay inside the skill directory |
| Starts with `/` | References must be relative to the skill directory |
| Contains `://` | URI schemes are not allowed in relative paths |
| Duplicate within same skill | Two references with the same `relative_path` cannot coexist |

The panicking `with_reference(...)` is appropriate when references are hardcoded in source (as in `s44_server_skills.rs`); the fallible `try_with_reference(...)` returns `Err(pmcp::Error::Validation)` and is the right choice when references are loaded from disk at runtime.

Cross-skill collision (two skills' reference URIs resolving to the same `skill://...` URI) is caught later, at `Skills::into_handler()` time, with a structured `Validation` error listing every duplicate URI detected.

Full example: [`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs)

---

## Tier 3: Code-Mode Skill (Composition with Another Advanced Feature)

Tier 3 is where Skills compose with another PMCP advanced feature without modifying that feature. The `validate_code` / `execute_code` tools from Code Mode (Chapter 12.9) are unchanged; only the *bootstrap layer* moves from a hand-rolled prompt plus scattered resources into a single SEP-2640 skill that ALSO exposes a prompt fallback — registered via one builder call.

The Tier 3 skill is multi-file. `build_code_mode_skill()` in `examples/s44_server_skills.rs` shows the full constructor:

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

Three references attach to one skill via the builder-pattern `.with_reference(...)`. Each reference carries its own per-resource MIME type — `application/graphql` for the schema file, `text/markdown` for the prose references. The MIME type travels with each `resources/read` response so consumers can pick the right syntax highlighter or parser without inferring from the URI.

The dual-surface registration is one call:

```rust,ignore
let _server = pmcp::Server::builder()
    .name("skills-demo")
    .version("0.1.0")
    .skill(Skill::new("hello-world", HELLO_WORLD))
    .skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
    .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")
    .build()?;
```

`bootstrap_skill_and_prompt(skill, prompt_name)` does two things in one move: it registers the `Skill` for SEP-2640-capable hosts (resource list + read), and it registers a `PromptHandler` named `"start_code_mode"` whose body is `Skill::as_prompt_text()` for legacy hosts that only speak `prompts/get`. Because both handlers are derived from the same `Skill` value, the two surfaces *cannot drift* — the dual-surface invariant from the section above is preserved by construction.

Cross-link: see [Chapter 12.9: Code Mode — LLM Code Validation and Execution](ch12-9-code-mode.md) for the `validate_code` / `execute_code` tool surface. This section does NOT duplicate that chapter's content; the job here is to show how the same workflow context flows through both the skill surface (SEP-2640 host) and the prompt surface (legacy host) without code duplication.

### How the Two Hosts See the Same Data

`examples/c10_client_skills.rs` walks both flows side-by-side. Flow A is what an SEP-2640-capable host does — discovery first, then `resources/list` and a `resources/read` for the SKILL.md and each reference URI.

The example opens with the discovery half. It reads the ENTRY PROJECTION — the same `SkillEntry` values a `skills/list` response carries — directly from the registry, because the example holds no client and no transport and therefore cannot issue the method itself:

```rust,ignore
// `entries` borrows and `into_handler` consumes, so read the projection
// FIRST. This is a local read, NOT a skills/list RPC.
let registry = Skills::new().add(skill.clone());
let entries = registry.entries().unwrap();

assert_eq!(entries.len(), 1);
assert_eq!(entries[0].uri(), "skill://code-mode/SKILL.md");
assert_eq!(
    entries[0].frontmatter().get("name").and_then(|v| v.as_str()),
    Some("code-mode"),
);
// SKILL.md + 3 references, each with a sha256: digest and a byte size.
assert_eq!(entries[0].resources().len(), 4);

let handler = registry.into_handler().unwrap();
```

Then the resource half, which does run against a real `ResourceHandler`:

```rust,ignore
async fn sep_2640_flow(handler: &dyn ResourceHandler, skill: &Skill) -> String {
    let extra = RequestHandlerExtra::default();

    // 1. resources/list — one entry per registered SKILL.md, and nothing
    //    else (references excluded per §9).
    let list = handler.list(None, extra.clone()).await.unwrap();
    // ...

    // 2. resources/read SKILL.md — assert wire shape.
    let skill_uri = "skill://code-mode/SKILL.md";
    let md_result = handler.read(skill_uri, extra.clone()).await.unwrap();
    // ...

    // 3. resources/read each reference URI — registration order — per-reference MIME.
    let mut concatenated = String::new();
    // <concatenation with --- relative_path --- separators>
    concatenated
}
```

Neither snippet issues `skills/list` or `skills/get` over MCP; both drive a handler in process. `tests/skills_routing.rs` is the file that sends real POSTs to a live `StreamableHttpServer` and asserts on the JSON-RPC bodies that come back — read it if you want the wire behavior rather than the projection.

Flow B is the legacy fallback — `prompts/get` invoked on a real `Server` built via the dual-surface bootstrap. The legacy host does not see references as separate resources; it gets the entire workflow context inlined in one prompt body:

```rust,ignore
async fn legacy_prompt_flow_via_get_prompt(skill: Skill) -> String {
    let server = pmcp::Server::builder()
        .name("skills-demo-client")
        .version("0.1.0")
        .bootstrap_skill_and_prompt(skill, "start_code_mode")
        .build()
        .expect("server build");

    let prompt_handler = server
        .get_prompt("start_code_mode")
        .expect("bootstrap_skill_and_prompt registered the handler");

    let extra = RequestHandlerExtra::default();
    let result = prompt_handler.handle(HashMap::new(), extra).await.unwrap();
    // <extract Content::Text from the single user message>
}
```

These two flows produce byte-equal context. The next section proves it.

### Runtime Proof: Byte-Equality Assertion

The dual-surface invariant from the "## The Dual-Surface Invariant" section is not just prose — `examples/c10_client_skills.rs` walks both flows and asserts the result:

```rust,ignore
println!("=== Byte-equality assertion ===");
assert_eq!(
    sep_2640_text, prompt_text,
    "dual-surface invariant violated: SEP-2640 read concatenation != prompt body"
);
println!(
    "Both flows produced byte-equal context ({} bytes).",
    prompt_text.len()
);
```

This is the runtime proof the chapter's dual-surface invariant section described. If a future refactor were to ever break byte-equality between `Skill::as_prompt_text()` and the concatenated SEP-2640 reads, `cargo run --example c10_client_skills` would panic — by design, since a silently-passing example that prints "OK" when the invariant is broken is worse than no example at all.

Full example: [`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs) + [`examples/c10_client_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/c10_client_skills.rs)

---

## Cross-SDK Compatibility (Why Three Tiers Match Other Reference Implementations)

The three-tier example structure (`hello-world`, `refunds`, `code-mode`) was deliberately chosen so that `hello-world` and `refunds` match the reference implementations shipped by the TypeScript MCP SDK, gemini-cli, fast-agent, goose, and codex against SEP-2640. The shared shape enables side-by-side developer-experience comparison: a server author can run their SDK's hello-world server alongside PMCP's `s44_server_skills.rs` and inspect the registration-time DX, the wire shape, and the prompt fallback in each.

The reference-implementations index lives at `https://github.com/modelcontextprotocol/experimental-ext-skills`.

PMCP's Skills surface is the differentiator: 5-line server-author code per skill (`Skill::new(...).with_path(...).with_reference(...)`), the dual-surface bootstrap reduced to a single builder call (`.bootstrap_skill_and_prompt(...)`), and the byte-equality invariant enforced in CI via `tests/skills_integration.rs`. To compare against another SDK, run their reference server and PMCP's `s44_server_skills.rs` in parallel and inspect what server authors actually have to write.

The SEP itself: `https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2640` (as-of commit `ade2a58`, April 2026).

### Comparing the DX Surfaces

A useful exercise when evaluating MCP SDKs is to count what server authors actually have to write for an equivalent skill registration. For a Tier 1 `hello-world` skill, PMCP's surface is two lines (the constant `include_str!` and the builder call), and the runtime artifact is one `resources/list` entry plus one conforming `skills/list` entry. Tier 2 adds one builder method (`.with_path(...)`) to demonstrate URI namespacing. Tier 3 adds `.bootstrap_skill_and_prompt(...)` — the single line that doubles the surface a host can use to fetch the same workflow context. That builder-method count (3) is the relevant signal: it does not grow with the number of references, the number of skills, or the number of advanced features the skill composes with.

The aspect that distinguishes PMCP from other SDKs at the wire level is the per-resource MIME type round-trip. The `Content::Resource` variant carries `uri`, `text`, and `mime_type` on every read response — so a reference file like `schema.graphql` keeps its `application/graphql` MIME type all the way from registration through `resources/read`. Some SDKs collapse to a string body without per-resource metadata; PMCP does not.

---

## Future Work (Deferred from Phase 80)

Two items are explicit non-goals for Phase 80 and Phase 81 — surfacing them here so readers are not misled into thinking they exist in the current SDK.

**`#[pmcp::skill]` procedural macro.** A `#[pmcp::skill]` attribute macro would read a SKILL.md file from disk at compile time and emit a `Skill` constant, eliminating the `include_str!("skills/.../SKILL.md")` boilerplate. The macro was deferred from Phase 80 as a follow-on spike — the design questions (relative-path resolution from the macro-invocation site, frontmatter validation at compile time, dependency-tracking with `proc_macro::tracked_path::path` so changes to SKILL.md trigger rebuilds) need their own focused investigation. Until the macro ships, server authors use the explicit `Skill::new(name, include_str!("skills/.../SKILL.md"))` pattern shown in the Tier 1/2/3 sections above.

**SEP-2640 §4 archive distribution (`application/gzip` blob).** SEP-2640 §4 defines an optional archive-distribution mode where a skill's entire directory is delivered as a single `application/gzip` blob. This is blocked by GAP #2 from spike 001: PMCP's `Content::Resource` variant has no `blob` field, so the archive mode cannot be wired without an additive protocol-types change. The SEP marks archive mode as optional and text-mode skills work fully without it; archive distribution will likely land as a follow-on phase paired with the protocol-types extension. For now, every skill in PMCP is text-mode: SKILL.md plus reference files delivered as individual `resources/read` results.

---

## Try It Yourself

The doctest below registers a single skill plus a dual-surface prompt, then asserts on the prompt text. It is compile-verified via `cargo test --doc -p pmcp --features skills,full`. The same doctest is embedded in `src/server/skills.rs` so it stays in sync with the production code — the two are byte-equal mirrors and must be edited together.

Note the frontmatter block. It is not decoration: a skill whose body carries none is excluded from `skills/list` (with a build-time warning), so a snippet without it would teach a construction that silently disappears from discovery.

```rust,no_run
use pmcp::server::skills::Skill;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SEP-2640 frontmatter. `name` MUST equal the final segment of the
    // resolved URI (`skill://hello-world/SKILL.md`) or the build is
    // rejected; a body with no frontmatter at all is excluded from
    // `skills/list` while remaining readable via `resources/read`.
    let greeting = Skill::new(
        "hello-world",
        "---\nname: hello-world\ndescription: A minimal skill\n---\n\n# Hello\nThis is a minimal skill.\n",
    );
    // The prompt surface serves the SKILL.md verbatim, frontmatter included.
    let prompt_text = greeting.as_prompt_text();
    assert!(prompt_text.starts_with("---\nname: hello-world\n"));
    assert!(prompt_text.contains("# Hello"));

    let _server = pmcp::Server::builder()
        .name("doctest-skills-demo")
        .version("0.1.0")
        .bootstrap_skill_and_prompt(greeting, "hello_prompt")
        .build()?;
    Ok(())
}
```

Run the full example with:

```bash
cargo run --example s44_server_skills --features skills,full
cargo run --example c10_client_skills --features skills,full
```

The server example prints the registered SKILL.md URIs, the entry projection a `skills/list` response carries (count, URI, frontmatter name, file count and digest per entry), the bootstrap result for the code-mode skill, and the byte length of the dual-surface text. The client example walks both host flows (the entry projection plus `resources/list` + `resources/read` for the SEP-2640 path, `prompts/get` for the legacy path) and asserts byte-equality at the end.

Neither example exercises `skills/list` or `skills/get` over MCP — both hold a handler in process rather than a client transport, and both say so in their output. `tests/skills_routing.rs` is the end-to-end proof.

### Where to Go from Here

Once you have the doctest above working, try the following extensions in order:

1. **Add a second skill** with `.skill(Skill::new("second-skill", "---\nname: second-skill\ndescription: ...\n---\n..."))` and verify it appears in the `resources/list` output. Then call `Skills::entries()` on a registry holding both and confirm the projection — the value a `skills/list` response carries — has two entries. Give the body real frontmatter: a skill with none is excluded from the projection (with a build-time warning) even though it stays listable and readable as a resource.
2. **Add a reference to your skill** with `.with_reference(SkillReference::new("references/notes.md", "text/markdown", "..."))` and verify the reference URI is NOT in `resources/list` but IS readable via `resources/read` — and that it DOES appear in the entry's `resources` manifest, with a digest matching the bytes the read returns. This is the SEP-2640 §9 visibility-filtering behavior the Tier 2 section describes.
3. **Use `.with_path("team/topic")`** to override the default URI, and confirm the new URI shows up in the listed entries. This is how to namespace skills inside larger workspaces. Note that if the skill carries a frontmatter `name:`, it must equal the URI's final segment (`topic` here) or the build is rejected — the URI is the identity, and the frontmatter must agree with it.
4. **Use `.bootstrap_skill_and_prompt(skill, "prompt_name")`** instead of `.skill(skill)` and verify that a `prompts/get prompt_name` call returns the same content the SEP-2640 resource reads return — byte-equal, per the chapter's central invariant.

**Related chapters:**

- Chapter 12.7 covers MCP Tasks for long-running operations — Skills compose with Tasks when a skill references task-augmented tools.
- Chapter 12.9 covers Code Mode in depth — the Tier 3 example here delegates execution to the `validate_code` / `execute_code` tools described there.

**Crate reference:** [`pmcp` on crates.io](https://crates.io/crates/pmcp) — Skills support is feature-gated behind `skills` in `Cargo.toml`.
