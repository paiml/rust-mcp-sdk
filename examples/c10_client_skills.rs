//! # Client example: Skills via BOTH host flows (Phase 80)
//!
//! Walks the two host flows side-by-side and proves they carry the SAME
//! content (byte-equal).
//!
//! ## Flow A — SEP-2640 host
//!
//! Capable hosts call `resources/list`, see one entry per registered
//! SKILL.md, then `resources/read` each URI lazily. Reference files (like
//! `skill://code-mode/references/schema.graphql`) are read by URI without
//! being enumerated. Each read response carries the URI and the
//! per-resource MIME type per SEP-2640 §4 wire shape — locked by
//! 80-REVIEWS.md Fix 3.
//!
//! ## What this example does NOT do: call `skills/list` or `skills/get`
//!
//! SEP-2640 discovery is the `skills/list` / `skills/get` METHOD pair, which
//! pmcp answers over **streamable HTTP**. This example holds no client, no
//! server and no transport — it builds a [`Skills`] registry, calls
//! `into_handler()`, and drives the resulting `ResourceHandler` **in
//! process**. It therefore cannot issue either call, and it does not pretend
//! to.
//!
//! What it demonstrates instead is the entry PROJECTION: the same
//! [`pmcp::server::skills::SkillEntry`] values a `skills/list` response
//! carries, obtained directly from the registry via `Skills::entries()`. The
//! example ASSERTS on that projection so a regression panics here rather than
//! printing something plausible and wrong.
//!
//! The authoritative end-to-end wire proof — real `skills/list` and
//! `skills/get` POSTs against a live `StreamableHttpServer`, asserted on the
//! response bodies — lives in `tests/skills_routing.rs`. If you are looking
//! for a working `skills/get` CLIENT, this file is not it; copying it and
//! expecting one will not work.
//!
//! (An earlier revision read a synthesized `skill://` discovery-index
//! resource here. That resource was retired in favour of the method-based
//! surface — 125-CONTEXT D-08.)
//!
//! ## Flow B — legacy prompt host
//!
//! Older hosts (no SEP-2640 support) call `prompts/get start_code_mode`.
//! This example exercises the same code path by building a real `Server`
//! via `pmcp::Server::builder()` and retrieving the registered
//! `PromptHandler` via `server.get_prompt("start_code_mode")`, then
//! invoking `.handle(args, extra)` — the same path `prompts/get` executes
//! per `src/server/mod.rs` `handle_get_prompt`. This is the wire-level
//! legacy flow per 80-REVIEWS.md Fix 5 / Codex C6.
//!
//! The example asserts byte-equality between the concatenated SEP-2640 read
//! results and the legacy prompt body. If the invariant is ever broken,
//! `cargo run --example c10_client_skills` panics — by design, since a
//! silently-passing example that prints "OK" when the invariant is broken
//! is worse than no example at all.
//!
//! Pair with `s44_server_skills.rs` for the server-side counterpart.
//!
//! Run with: `cargo run --example c10_client_skills --features skills,full`

use std::collections::HashMap;

use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::types::Content;
use pmcp::{RequestHandlerExtra, ResourceHandler};

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

// 80-REVIEWS.md Fix 3 / Codex C4: SkillsHandler returns Content::Resource
// (not Content::Text), so each read response carries uri + mime_type
// alongside the body. Extract via pattern match on the Resource variant.
fn extract_resource(contents: &[Content]) -> (String, String, String) {
    match contents.first() {
        Some(Content::Resource {
            uri,
            text,
            mime_type,
            ..
        }) => (
            uri.clone(),
            text.clone().expect("skills handler always emits text body"),
            mime_type
                .clone()
                .expect("skills handler always emits mime_type"),
        ),
        other => panic!("expected Content::Resource from skill resource read, got {other:?}"),
    }
}

fn append_with_trailing_newline(out: &mut String, body: &str) {
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
}

async fn sep_2640_flow(handler: &dyn ResourceHandler, skill: &Skill) -> String {
    let extra = RequestHandlerExtra::default();

    // 1. resources/list — one entry per registered SKILL.md, and nothing
    //    else (references excluded per §9; no synthesized discovery entry).
    let list = handler.list(None, extra.clone()).await.unwrap();
    println!(
        "resources/list returned {} resource(s):",
        list.resources.len()
    );
    for r in &list.resources {
        println!("  {} ({})", r.uri, r.mime_type.as_deref().unwrap_or(""));
    }
    println!();

    // 2. resources/read SKILL.md — assert wire shape.
    let skill_uri = "skill://code-mode/SKILL.md";
    let md_result = handler.read(skill_uri, extra.clone()).await.unwrap();
    let (md_uri, main_text, md_mime) = extract_resource(&md_result.contents);
    assert_eq!(md_uri, skill_uri);
    assert_eq!(md_mime, "text/markdown");
    println!(
        "resources/read SKILL.md uri={md_uri} mime={md_mime} bytes={}",
        main_text.len()
    );

    // 3. resources/read each reference URI — registration order — per-reference MIME.
    let mut concatenated = String::new();
    append_with_trailing_newline(&mut concatenated, &main_text);
    for r in skill.references() {
        let uri = format!("skill://code-mode/{}", r.relative_path());
        let read = handler.read(&uri, extra.clone()).await.unwrap();
        let (resp_uri, body, resp_mime) = extract_resource(&read.contents);
        assert_eq!(resp_uri, uri);
        assert_eq!(
            resp_mime,
            r.mime_type(),
            "per-resource MIME type must round-trip (Fix 3)"
        );
        println!(
            "resources/read reference uri={resp_uri} mime={resp_mime} bytes={}",
            body.len()
        );
        concatenated.push_str("\n--- ");
        concatenated.push_str(r.relative_path());
        concatenated.push_str(" ---\n");
        append_with_trailing_newline(&mut concatenated, &body);
    }

    concatenated
}

/// Legacy host flow — exercises the wire-level prompt-handler path per Fix 5.
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

    // SkillPromptHandler returns a single PromptMessage::user(Content::text(...)).
    // (Prompt messages still use plain Content::Text — the wire-shape fix
    // applies only to ResourceHandler::read, not to PromptMessage content.)
    let prompt_text = match &result.messages[0].content {
        Content::Text { text } => text.clone(),
        other => panic!("expected Content::Text for prompt message, got {other:?}"),
    };

    println!(
        "prompts/get start_code_mode returned {} bytes",
        prompt_text.len()
    );
    println!("First 240 bytes of prompt body:");
    println!("{}", &prompt_text[..prompt_text.len().min(240)]);
    println!();
    prompt_text
}

/// The SEP-2640 entry projection — the values a `skills/list` response
/// carries — read directly from the registry.
///
/// This is NOT an RPC. `sep_2640_flow` above takes a `&dyn ResourceHandler`,
/// and `skills/get` is not a `ResourceHandler` method; reaching a real
/// `skills/get` would mean turning this example into an HTTP client/server
/// harness, which is a different example. So this function demonstrates the
/// projection honestly and says so, rather than simulating a call.
///
/// It asserts rather than only printing: an example that panics on regression
/// is a stronger doc surface than one that prints something plausible.
fn skills_entry_projection(registry: &Skills) {
    // `entries` borrows and `into_handler` consumes, so this must run BEFORE
    // the handler is built — hence the ordering in `main`. No clone needed.
    let entries = registry
        .entries()
        .expect("the code-mode skill's frontmatter name matches its URI segment");

    assert_eq!(entries.len(), 1, "one registered skill => one entry");
    let entry = &entries[0];

    assert_eq!(entry.uri(), "skill://code-mode/SKILL.md");
    assert_eq!(
        entry.frontmatter().get("name").and_then(|v| v.as_str()),
        Some("code-mode"),
        "frontmatter is emitted verbatim from SKILL.md"
    );
    assert_eq!(
        entry.resources().len(),
        4,
        "manifest = SKILL.md + 3 references, in registration order"
    );

    let digest = entry.resources()[0].digest();
    assert!(
        digest.starts_with("sha256:")
            && digest.len() == "sha256:".len() + 64
            && digest["sha256:".len()..]
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "digest must be sha256: + 64 lowercase hex, got {digest}"
    );

    println!("Entry projection (the value a skills/list response carries):");
    println!("  uri     = {}", entry.uri());
    println!("  name    = code-mode (verbatim from SKILL.md frontmatter)");
    println!(
        "  files   = {} (SKILL.md + 3 references)",
        entry.resources().len()
    );
    for r in entry.resources() {
        println!("    {} {} ({} bytes)", r.digest(), r.uri(), r.size());
    }
    println!();
    println!("  NOTE: this is Skills::entries(), read IN PROCESS. This example");
    println!("  holds no transport, so it does NOT exercise skills/list or");
    println!("  skills/get over MCP. The end-to-end wire proof — real POSTs");
    println!("  against a live StreamableHttpServer — is tests/skills_routing.rs.");
    println!();
}

#[tokio::main]
async fn main() {
    let skill = build_code_mode_skill();

    // Build the registry ONCE. `entries()` borrows it and `into_handler()`
    // consumes it, so the projection must be read first — that ordering is
    // why no clone of the registry is needed.
    let registry = Skills::new().add(skill.clone());

    println!("=== SEP-2640 discovery: skills/list + skills/get (projection only) ===");
    skills_entry_projection(&registry);

    // Build the handler that an SEP-2640-capable host would interact with.
    let handler = registry
        .into_handler()
        .expect("skill registration must not collide");

    println!("=== Flow A: SEP-2640-capable host (resources/list + resources/read) ===");
    let sep_2640_text = sep_2640_flow(&*handler, &skill).await;
    println!();

    println!("=== Flow B: legacy host (prompts/get start_code_mode via get_prompt) ===");
    let prompt_text = legacy_prompt_flow_via_get_prompt(skill).await;
    println!();

    println!("=== Byte-equality assertion ===");
    assert_eq!(
        sep_2640_text, prompt_text,
        "dual-surface invariant violated: SEP-2640 read concatenation != prompt body"
    );
    println!(
        "Both flows produced byte-equal context ({} bytes).",
        prompt_text.len()
    );
}
