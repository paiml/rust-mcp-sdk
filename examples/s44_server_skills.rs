//! # Server example: SEP-2640 Skills (Phase 80)
//!
//! Demonstrates the three-tier skill registration pattern + the dual-surface
//! bootstrap. Skills live under `examples/skills/` and are embedded via
//! `include_str!` at compile time.
//!
//! **Why this example uses `Server::builder()` (not `ServerCoreBuilder`):**
//! `pmcp::Server::builder()` is the public, documented entry point for
//! constructing servers in PMCP. The skill API (`.skill`, `.skills`,
//! `.try_skills`, `.bootstrap_skill_and_prompt`) is wired onto BOTH
//! `ServerBuilder` (returned by `Server::builder()`) AND `ServerCoreBuilder`
//! (per 80-REVIEWS.md Fix 2 / Codex C3) so this example demonstrates the
//! recommended path.
//!
//! Run with: `cargo run --example s44_server_skills --features skills,full`
//!
//! What this example prints:
//! 1. The registered SKILL.md URIs (the `resources/list` surface).
//! 2. The SEP-2640 entry PROJECTION — the count of conforming entries the
//!    registry produced, plus the first entry's URI and digest.
//! 3. The fact that `bootstrap_skill_and_prompt(...)` registers BOTH a
//!    skill AND a parallel prompt from one `Skill` value.
//! 4. The byte length of the dual-surface text (skill body + references).
//!
//! ## Discovery is `skills/list` / `skills/get`, and this example does NOT call them
//!
//! A server built the way this example builds one answers the SEP-2640
//! `skills/list` and `skills/get` methods over **streamable HTTP**. This
//! example binds no socket and holds no client, so it does NOT exercise
//! either method over MCP. What it prints instead is the entry PROJECTION —
//! the very same [`pmcp::server::skills::SkillEntry`] values a `skills/list`
//! response carries — obtained directly from the registry via
//! `Skills::entries()`.
//!
//! The authoritative end-to-end wire proof lives in `tests/skills_routing.rs`,
//! which drives real `skills/list` and `skills/get` POSTs against a live
//! `StreamableHttpServer` and asserts on the response bodies. Read that file,
//! not this one, if you want to see the RPC.
//!
//! There is no synthesized discovery-index resource. One used to be minted
//! automatically and enumerated by `resources/list`; it was retired in favour
//! of the method-based surface (125-CONTEXT D-08).
//!
//! Note on SEP-2640 §9: reference URIs (e.g.
//! `skill://code-mode/references/schema.graphql`) are addressable via
//! `resources/read` but MUST NOT appear in `resources/list`. They are
//! intentionally absent from the printed URI list below — this is the
//! spec-required "readable but not listable" behavior, locked by the
//! integration test in `tests/skills_integration.rs`.
//!
//! Pair with `c10_client_skills.rs` to see the full client-side flow.

use pmcp::server::skills::{Skill, SkillReference, Skills};

// Compiled-in skill bodies (matches spike 002 reference impl byte-for-byte
// — see `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs`
// lines 391-512).
const HELLO_WORLD: &str = include_str!("skills/hello-world/SKILL.md");
const REFUNDS: &str = include_str!("skills/refunds/SKILL.md");
const CODE_MODE: &str = include_str!("skills/code-mode/SKILL.md");
const CODE_MODE_SCHEMA: &str = include_str!("skills/code-mode/references/schema.graphql");
const CODE_MODE_EXAMPLES: &str = include_str!("skills/code-mode/references/examples.md");
const CODE_MODE_POLICIES: &str = include_str!("skills/code-mode/references/policies.md");

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let code_mode_skill = build_code_mode_skill();
    let hello_world_skill = Skill::new("hello-world", HELLO_WORLD);
    let refunds_skill = Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds");

    // A SEPARATE registry holding CLONES of the same three skills, built
    // purely so `entries()` can be called on it. Two constraints force this
    // apparent duplication — neither is avoidable from an example crate:
    //
    //   1. Passing this registry to `.skills(...)` and ALSO keeping
    //      `.bootstrap_skill_and_prompt(code_mode_skill, ...)` would register
    //      code-mode twice. `ServerBuilder::skill` / `::skills` /
    //      `::bootstrap_skill_and_prompt` all merge into ONE pending registry
    //      finalized at `.build()`, so the merged registry would present a
    //      duplicate `skill://code-mode/SKILL.md` and `.build()` would fail.
    //
    //   2. Dropping `bootstrap_skill_and_prompt` in favour of a direct
    //      `.prompt(...)` is not possible either: the dual-surface prompt
    //      handler (`SkillPromptHandler`) is `pub(crate)`, and `examples/` is
    //      a separate crate. `bootstrap_skill_and_prompt` is the ONLY route
    //      from an example to that surface.
    //
    // So the registration chain below is left intact and this registry exists
    // alongside it, for projection only. `Skill` is `Clone`, so this costs a
    // copy of three already-embedded `&'static str` bodies.
    let projection_registry = Skills::new()
        .add(hello_world_skill.clone())
        .add(refunds_skill.clone())
        .add(code_mode_skill.clone());

    // pmcp::Server::builder() returns ServerBuilder (see src/server/mod.rs:637).
    // The skill API is wired on BOTH ServerBuilder AND ServerCoreBuilder per
    // 80-REVIEWS.md Fix 2; we use the public path here so the example
    // mirrors how a real server author would write this code.
    //
    // Tier 1: hello-world (trivial, default path = skill://hello-world/SKILL.md).
    // Tier 2: refunds (path-overridden — demonstrates skill://acme/billing/refunds/SKILL.md).
    // Tier 3: code-mode (multi-file + dual-surface bootstrap).
    //
    // The chained `.skill().skill().bootstrap_skill_and_prompt(...)` sequence
    // exercises the accumulator pattern from 80-REVIEWS.md Fix 1: every
    // call accumulates into a single pending registry, finalized once at
    // .build() time. There is no per-call wrapper nesting.
    let _server = pmcp::Server::builder()
        .name("skills-demo")
        .version("0.1.0")
        .skill(hello_world_skill)
        .skill(refunds_skill)
        .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")
        .build()?;

    // For the full code-mode tool wiring (validate_code + execute_code tools),
    // see examples/s41_code_mode_graphql.rs. Inlining it here would add ~50
    // lines of unrelated GraphQL scaffolding; the cross-reference keeps this
    // example focused on the skill registration story.

    println!("Skills demo server built successfully via pmcp::Server::builder().");
    println!();
    println!("Registered SKILL.md URIs (the resources/list surface):");
    println!("  skill://hello-world/SKILL.md");
    println!("  skill://acme/billing/refunds/SKILL.md");
    println!("  skill://code-mode/SKILL.md");
    println!();

    // The SEP-2640 discovery surface: a server built like the one above
    // answers `skills/list` and `skills/get` over streamable HTTP. What
    // follows is the PROJECTION those responses carry, read straight from
    // the registry — NOT an RPC round trip. This example holds no client
    // and no transport, so it cannot issue either call.
    let entries = projection_registry.entries()?;
    println!("SEP-2640 discovery surface: skills/list + skills/get.");
    println!(
        "  {} conforming entr{} in the projection a skills/list response carries:",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    for e in &entries {
        let name = e
            .frontmatter()
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<no frontmatter name>");
        let digest = e.resources().first().map_or(
            "<no resources>",
            pmcp::server::skills::SkillResourceRef::digest,
        );
        println!(
            "    {} name={name} files={} digest={digest}",
            e.uri(),
            e.resources().len()
        );
    }
    println!("  NOT an RPC: this is Skills::entries(), read in process.");
    println!("  End-to-end wire proof lives in tests/skills_routing.rs.");
    println!();
    println!("Code-mode dual-surface registered as:");
    println!("  Skill at:  skill://code-mode/SKILL.md (+ 3 references)");
    println!("  Prompt at: start_code_mode");
    println!();
    println!(
        "Dual-surface text length: {} bytes",
        code_mode_skill.as_prompt_text().len()
    );
    println!();
    println!("Pair this with: cargo run --example c10_client_skills --features skills,full");

    Ok(())
}
