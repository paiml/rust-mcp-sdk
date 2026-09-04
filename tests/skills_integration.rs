//! SEP-2640 Skills integration test.
//!
//! Exercises the SEP-2640 resource surface (resources/list, resources/read
//! for SKILL.md, resources/read for a reference) via direct trait-impl calls
//! on the `ResourceHandler` returned by `Skills::into_handler()`.
//!
//! Discovery itself is NOT a resource: it is the `skills/list` / `skills/get`
//! method pair, proven end to end over the wire in `tests/skills_routing.rs`.
//! The synthesized `skill://` discovery index this file used to read was
//! retired in Phase 125 plan 04; a replacement test asserts it now errors.
//!
//! The load-bearing tests are:
//!
//! - Construction-level dual-surface byte equality: the SEP-2640 surface
//!   (concatenated resource reads with labelled-rule separators) equals
//!   `Skill::as_prompt_text()`.
//!
//! - Wire-level dual-surface byte equality: the same SEP-2640 surface
//!   equals the body returned by the prompt handler that
//!   `Server::builder().bootstrap_skill_and_prompt(...).build()`
//!   registers, retrieved via `server.get_prompt("x")` and invoked via
//!   `.handle(args, extra)`. This is the same code path `prompts/get`
//!   executes at runtime.
//!
//! - CRLF resilience: the invariant holds whether the SKILL.md is
//!   authored with CRLF (Windows) or LF (Linux) line endings.
//!
//! - Per-resource wire shape: every `read()` response carries the URI and
//!   the per-resource MIME type via `Content::Resource`.

#![cfg(all(feature = "skills", not(target_arch = "wasm32")))]

use std::collections::HashMap;
use std::sync::Arc;

use pmcp::error::ErrorCode;
use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::types::Content;
use pmcp::{RequestHandlerExtra, ResourceHandler};
use proptest::prelude::*;

// Smaller content than the example's code-mode skill — keeps the test
// self-contained. The dual-surface invariant is content-agnostic.

fn build_widget_skill_lf() -> Skill {
    Skill::new(
        "widget-builder",
        "---\nname: widget-builder\ndescription: Build widgets per company spec\n---\n\n# Widget Builder Workflow\n\n1. Verify spec.\n2. Render component.\n3. Run smoke test.",
    )
    .with_reference(SkillReference::new(
        "references/spec.md",
        "text/markdown",
        "# Widget Spec\n\nWidgets MUST have a name, a body, and zero or more references.",
    ))
    .with_reference(SkillReference::new(
        "references/checklist.md",
        "text/markdown",
        "# Pre-flight Checklist\n\n- [ ] Spec matches\n- [ ] Smoke test green",
    ))
}

/// CRLF-authored counterpart — same content semantically, but every newline
/// is `\r\n`. The dual-surface invariant must still hold so authors on
/// Windows can't accidentally break consumers reading on Linux.
fn build_widget_skill_crlf() -> Skill {
    Skill::new(
        "widget-builder-crlf",
        "---\r\nname: widget-builder-crlf\r\ndescription: Build widgets per company spec (CRLF authored)\r\n---\r\n\r\n# Widget Builder Workflow\r\n\r\n1. Verify spec.\r\n2. Render component.\r\n3. Run smoke test.",
    )
    .with_reference(SkillReference::new(
        "references/spec.md",
        "text/markdown",
        "# Widget Spec\r\n\r\nWidgets MUST have a name, a body, and zero or more references.",
    ))
}

fn build_trivial_skill() -> Skill {
    Skill::new("hello", "---\nname: hello\n---\nHi.")
}

/// Extract URI + body + MIME from a Resource-variant `Content`. Reads MUST
/// be the Resource variant — `Content::Text` would drop the per-URI MIME.
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
        other => panic!("expected Content::Resource, got {other:?}"),
    }
}

fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{s}\n")
    }
}

async fn build_handler() -> Arc<dyn ResourceHandler> {
    Skills::new()
        .add(build_trivial_skill())
        .add(build_widget_skill_lf())
        .into_handler()
        .expect("test fixture must not produce duplicates")
}

/// Compute the SEP-2640 surface by reading SKILL.md + each reference URI
/// via the resource handler and concatenating with `as_prompt_text`'s
/// separators.
async fn compute_sep_2640_surface(
    handler: &dyn ResourceHandler,
    skill: &Skill,
    skill_name: &str,
) -> String {
    let extra = RequestHandlerExtra::default();
    let main_uri = format!("skill://{skill_name}/SKILL.md");
    let main = handler.read(&main_uri, extra.clone()).await.unwrap();
    let (m_uri, m_text, m_mime) = extract_resource(&main.contents);
    assert_eq!(m_uri, main_uri);
    assert_eq!(m_mime, "text/markdown");
    let mut sep = ensure_trailing_newline(&m_text);
    for r in skill.references() {
        let uri = format!("skill://{skill_name}/{}", r.relative_path());
        let read = handler.read(&uri, extra.clone()).await.unwrap();
        let (r_uri, r_body, r_mime) = extract_resource(&read.contents);
        assert_eq!(r_uri, uri);
        assert_eq!(r_mime, r.mime_type());
        sep.push_str("\n--- ");
        sep.push_str(r.relative_path());
        sep.push_str(" ---\n");
        sep.push_str(&ensure_trailing_newline(&r_body));
    }
    sep
}

// Build a server via the public Server::builder() path, retrieve the
// registered prompt handler via get_prompt, invoke .handle (the same
// path prompts/get executes at runtime), and return the first message's
// text body. Used by Tests 3.7 + 3.7a.
async fn wire_level_prompt_text(skill: Skill, prompt_name: &str) -> String {
    let server = pmcp::Server::builder()
        .name("integration-test")
        .version("1.0")
        .bootstrap_skill_and_prompt(skill, prompt_name)
        .build()
        .expect("server build");
    let prompt = server
        .get_prompt(prompt_name)
        .expect("bootstrap_skill_and_prompt registered the handler");
    let result = prompt
        .handle(HashMap::new(), RequestHandlerExtra::default())
        .await
        .unwrap();
    match &result.messages[0].content {
        Content::Text { text } => text.clone(),
        other => panic!("expected Content::Text for prompt message, got {other:?}"),
    }
}

// Test 3.1
//
// Renamed from `resources_list_returns_skill_md_and_index_only` in Phase 125
// plan 04: the old NAME encoded the retired shape as strongly as its length
// assertion did.
#[tokio::test]
async fn resources_list_returns_skill_md_only() {
    let handler = build_handler().await;
    let result = handler
        .list(None, RequestHandlerExtra::default())
        .await
        .unwrap();
    let uris: Vec<&str> = result.resources.iter().map(|r| r.uri.as_str()).collect();
    assert_eq!(
        result.resources.len(),
        2,
        "2 SKILL.md and nothing else = 2, got {uris:?}"
    );
    assert!(uris.contains(&"skill://hello/SKILL.md"));
    assert!(uris.contains(&"skill://widget-builder/SKILL.md"));
    assert!(
        !uris.contains(&"skill://index.json"),
        "the synthesized discovery index was retired (125-CONTEXT D-08): {uris:?}"
    );
    assert!(
        !uris.iter().any(|u| u.contains("/references/")),
        "SEP-2640 section 9: references MUST NOT be enumerated"
    );
}

// wire shape: SKILL.md reads return Content::Resource with the right MIME
#[tokio::test]
async fn resources_read_skill_md_returns_resource_with_text() {
    let handler = build_handler().await;
    let result = handler
        .read(
            "skill://widget-builder/SKILL.md",
            RequestHandlerExtra::default(),
        )
        .await
        .unwrap();
    let (uri, text, mime) = extract_resource(&result.contents);
    assert_eq!(uri, "skill://widget-builder/SKILL.md");
    assert_eq!(mime, "text/markdown");
    assert!(text.contains("Widget Builder Workflow"));
}

// wire shape: each reference read carries its own per-URI MIME type
#[tokio::test]
async fn resources_read_reference_carries_per_resource_mime() {
    let handler = build_handler().await;
    let result = handler
        .read(
            "skill://widget-builder/references/spec.md",
            RequestHandlerExtra::default(),
        )
        .await
        .unwrap();
    let (uri, text, mime) = extract_resource(&result.contents);
    assert_eq!(uri, "skill://widget-builder/references/spec.md");
    assert_eq!(mime, "text/markdown");
    assert!(text.contains("Widget Spec"));
}

// The retired discovery index is no longer readable.
//
// REPLACES `resources_read_index_returns_resource_with_text_application_json`,
// which asserted the synthesized index came back as an `application/json`
// resource. A deleted test is a silent coverage loss; a replaced one pins the
// decision, so a reintroduced short-circuit fails here (125-CONTEXT D-08).
//
// The retired URI takes the ORDINARY unknown-URI path — the same
// `METHOD_NOT_FOUND` `resources_read_unknown_uri_method_not_found` pins for a
// URI that was never registered. Note that is the HANDLER-level code: over
// streamable HTTP the dispatch tail re-wraps it, so a caller on the wire sees
// -32603 carrying -32601 inside the message (measured in 125-02).
#[tokio::test]
async fn resources_read_retired_index_uri_is_unknown() {
    let handler = build_handler().await;
    let err = handler
        .read("skill://index.json", RequestHandlerExtra::default())
        .await
        .expect_err("the retired discovery URI must no longer be served");
    match err {
        pmcp::Error::Protocol { code, message, .. } => {
            assert_eq!(code, ErrorCode::METHOD_NOT_FOUND);
            assert!(
                message.contains("skill://index.json"),
                "message = {message}"
            );
        },
        other => panic!("expected Protocol error with METHOD_NOT_FOUND, got {other:?}"),
    }
    // Control: the handler is not simply refusing every read.
    let ok = handler
        .read("skill://hello/SKILL.md", RequestHandlerExtra::default())
        .await
        .expect("a registered SKILL.md must still read");
    let (uri, _, _) = extract_resource(&ok.contents);
    assert_eq!(uri, "skill://hello/SKILL.md");
}

// Test 3.5 — METHOD_NOT_FOUND on unknown URI
#[tokio::test]
async fn resources_read_unknown_uri_method_not_found() {
    let handler = build_handler().await;
    let err = handler
        .read(
            "skill://nonexistent/SKILL.md",
            RequestHandlerExtra::default(),
        )
        .await
        .unwrap_err();
    match err {
        pmcp::Error::Protocol { code, .. } => assert_eq!(code, ErrorCode::METHOD_NOT_FOUND),
        other => panic!("expected Protocol error with METHOD_NOT_FOUND, got {other:?}"),
    }
}

// Test 3.6 — construction-level dual-surface byte equality
#[tokio::test]
async fn dual_surface_byte_equal_construction_level() {
    let skill = build_widget_skill_lf();
    let handler = Skills::new().add(skill.clone()).into_handler().unwrap();
    let sep_2640 = compute_sep_2640_surface(&*handler, &skill, "widget-builder").await;
    let prompt = skill.as_prompt_text();
    assert_eq!(
        sep_2640, prompt,
        "DUAL-SURFACE INVARIANT VIOLATED (construction-level): SEP-2640 read concatenation must equal as_prompt_text()"
    );
}

// wire-level dual-surface: byte equality via Server::builder + get_prompt
#[tokio::test]
async fn dual_surface_byte_equal_wire_level_via_get_prompt() {
    let skill = build_widget_skill_lf();
    let wire_prompt_text = wire_level_prompt_text(skill.clone(), "x").await;

    // Recompute the SEP-2640 surface from the same skill content.
    let handler = Skills::new().add(skill.clone()).into_handler().unwrap();
    let sep_2640 = compute_sep_2640_surface(&*handler, &skill, "widget-builder").await;

    assert_eq!(
        sep_2640, wire_prompt_text,
        "DUAL-SURFACE INVARIANT VIOLATED (WIRE LEVEL): SEP-2640 read concatenation must equal the prompt body retrieved via get_prompt + handle"
    );
}

// dual-surface invariant survives CRLF + mixed-line-ending SKILL.md authoring
#[tokio::test]
async fn dual_surface_byte_equal_crlf_and_mixed_line_endings() {
    for skill in [build_widget_skill_lf(), build_widget_skill_crlf()] {
        let name = skill.name().to_string();
        let handler = Skills::new().add(skill.clone()).into_handler().unwrap();
        let sep_2640 = compute_sep_2640_surface(&*handler, &skill, &name).await;
        let construction_prompt = skill.as_prompt_text();
        assert_eq!(
            sep_2640, construction_prompt,
            "DUAL-SURFACE INVARIANT VIOLATED for {name} (construction level)"
        );

        let wire_text = wire_level_prompt_text(skill.clone(), "x").await;
        assert_eq!(
            sep_2640, wire_text,
            "DUAL-SURFACE INVARIANT VIOLATED for {name} (wire level)"
        );
    }
}

// ── SEP-2640 entry manifests (Phase 125, plan 03) ─────────────────────

/// D-02: a frontmatter-less skill is excluded from the SKILLS listing ONLY.
/// It stays enumerated by `resources/list` and stays readable byte-identically
/// through `resources/read` — the exclusion never removes it from the resource
/// surface, which is what makes a partial listing SEP-legal rather than lossy.
#[tokio::test]
async fn frontmatter_less_skill_is_excluded_from_entries_but_still_served() {
    const BARE_BODY: &str = "# Bare\n\nThis skill has no YAML frontmatter at all.\n";

    let registry = Skills::new()
        .add(build_widget_skill_lf())
        .add(Skill::new("bare", BARE_BODY));

    let entries = registry.entries().expect("entries build");
    assert_eq!(entries.len(), 1, "only the annotated skill is listed");
    assert_eq!(entries[0].uri(), "skill://widget-builder/SKILL.md");

    let handler = registry.into_handler().expect("fixture has no duplicates");
    let list = handler
        .list(None, RequestHandlerExtra::default())
        .await
        .unwrap();
    let uris: Vec<&str> = list.resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(
        uris.contains(&"skill://bare/SKILL.md"),
        "the excluded skill is still enumerated by resources/list, got {uris:?}"
    );

    let read = handler
        .read("skill://bare/SKILL.md", RequestHandlerExtra::default())
        .await
        .expect("the excluded skill is still readable");
    let (uri, text, mime) = extract_resource(&read.contents);
    assert_eq!(uri, "skill://bare/SKILL.md");
    assert_eq!(mime, "text/markdown");
    assert_eq!(text, BARE_BODY, "served byte-identically");
}

/// The entry manifest names EVERY file the skill is made of — its `SKILL.md`
/// first, then each reference in registration order — and every row's `size` is
/// the byte length of what `resources/read` actually returns for that URI.
///
/// Reading the bytes back THROUGH the handler is the point: a test that
/// re-derived them from the `Skill` would pass against an implementation where
/// the manifest and the served content agreed only because both were wrong.
#[tokio::test]
async fn entry_manifest_names_every_file_and_sizes_match_the_served_bytes() {
    let registry = Skills::new().add(build_widget_skill_lf());
    let entries = registry.entries().expect("entries build");
    assert_eq!(entries.len(), 1, "the LF fixture carries frontmatter");

    let manifest = entries[0].resources();
    let uris: Vec<&str> = manifest.iter().map(|r| r.uri()).collect();
    assert_eq!(
        uris,
        vec![
            "skill://widget-builder/SKILL.md",
            "skill://widget-builder/references/spec.md",
            "skill://widget-builder/references/checklist.md",
        ],
        "SKILL.md first, then references in registration order"
    );

    let handler = registry.into_handler().expect("fixture has no duplicates");
    for row in manifest {
        let read = handler
            .read(row.uri(), RequestHandlerExtra::default())
            .await
            .expect("every manifest URI must be readable");
        let (uri, text, _mime) = extract_resource(&read.contents);
        assert_eq!(uri, row.uri());
        assert_eq!(
            text.len(),
            row.size(),
            "manifest size must equal the served byte length for {}",
            row.uri()
        );
        assert!(
            row.digest().starts_with("sha256:") && row.digest().len() == "sha256:".len() + 64,
            "digest shape for {}: {}",
            row.uri(),
            row.digest()
        );
    }
}

/// An LF-authored SKILL.md and a CRLF twin of the SAME content produce
/// byte-identical `frontmatter` JSON.
///
/// The CRLF twin is derived from the LF fixture's own body here rather than
/// taken from [`build_widget_skill_crlf`]: that shipped fixture deliberately
/// differs in BOTH of its frontmatter fields (`name` and `description`), so
/// "equal except for the differing keys" would compare an empty key set and
/// prove nothing. The shipped fixture is still exercised below for its key set.
#[test]
fn lf_and_crlf_frontmatter_are_identical() {
    let lf_skill = build_widget_skill_lf();
    let crlf_body = lf_skill.body().replace('\n', "\r\n");

    let lf_entries = Skills::new()
        .add(lf_skill.clone())
        .entries()
        .expect("entries build");
    let crlf_entries = Skills::new()
        .add(Skill::new("widget-builder", crlf_body))
        .entries()
        .expect("entries build");

    assert_eq!(
        lf_entries[0].frontmatter(),
        crlf_entries[0].frontmatter(),
        "line endings must not reach the emitted frontmatter"
    );
    assert_eq!(
        lf_entries[0].frontmatter()["name"],
        "widget-builder",
        "and the comparison above is not vacuous — the object has real keys"
    );

    // The shipped CRLF fixture parses to the same KEY SET (its values differ by
    // design), which is what locks the CRLF path for a separately-authored file.
    let shipped = Skills::new()
        .add(build_widget_skill_crlf())
        .entries()
        .expect("entries build");
    let key_set = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        key_set(lf_entries[0].frontmatter()),
        key_set(shipped[0].frontmatter())
    );
}

// Test 3.8 — proptest construction-level byte equality under arbitrary content
proptest! {
    #[test]
    fn proptest_byte_equality_under_arbitrary_skill_content(
        body in "[a-zA-Z0-9 \\n.,!?-]{0,200}",
        ref_paths in proptest::collection::vec("references/[a-z]{1,10}\\.md", 0..4),
        ref_bodies in proptest::collection::vec("[a-zA-Z0-9 \\n.,!?-]{0,80}", 0..4),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let n = ref_paths.len().min(ref_bodies.len());
            let mut skill = Skill::new("propskill", body);
            for i in 0..n {
                // try_with_reference returns Err on invalid paths (e.g. duplicates); skip those.
                // A rejected path leaves `skill` as it was and the loop moves
                // on — proptest generates paths `validate_reference_path` is
                // entitled to refuse.
                if let Ok(s) = skill.clone().try_with_reference(SkillReference::new(
                    &ref_paths[i],
                    "text/markdown",
                    &ref_bodies[i],
                )) {
                    skill = s;
                }
            }
            let prompt = skill.as_prompt_text();
            let handler = Skills::new().add(skill.clone()).into_handler().unwrap();
            let sep_2640 = compute_sep_2640_surface(&*handler, &skill, "propskill").await;
            prop_assert_eq!(sep_2640, prompt);
            Ok::<(), proptest::test_runner::TestCaseError>(())
        })?;
    }
}

// proptest wire-shape: every read response carries URI + MIME
proptest! {
    #[test]
    fn proptest_read_responses_always_carry_uri_and_mime(
        body in "[a-zA-Z0-9 \\n]{0,80}",
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let skill = Skill::new("propmime", body).with_reference(
                SkillReference::new("references/a.md", "text/markdown", "ref body"),
            );
            let handler = Skills::new().add(skill).into_handler().unwrap();
            let extra = RequestHandlerExtra::default();
            // The retired discovery URI is deliberately absent from this
            // list: it no longer reads, so asserting a wire shape for it
            // would assert that an error is a well-formed resource.
            for uri in [
                "skill://propmime/SKILL.md",
                "skill://propmime/references/a.md",
            ] {
                let r = handler.read(uri, extra.clone()).await.unwrap();
                match &r.contents[0] {
                    Content::Resource { uri: u, text, mime_type, .. } => {
                        prop_assert_eq!(u.as_str(), uri);
                        prop_assert!(text.is_some(), "text must be present");
                        prop_assert!(mime_type.is_some(), "mime_type must be present");
                    }
                    other => prop_assert!(false, "expected Content::Resource, got {:?}", other),
                }
            }
            Ok::<(), proptest::test_runner::TestCaseError>(())
        })?;
    }
}

// ── Phase 126 plan 01 (SC-4, in-process half) ─────────────────────────
//
// A `SequentialWorkflow` projects to a `Skill`, registers through the real
// `Skills::into_handler()` choke point, and reads back byte-identical from the
// real `ResourceHandler`. This is SC-4's in-process half; the wire half lives
// in `tests/skills_routing.rs`.
//
// These tests live HERE rather than in a new `tests/skills_projection.rs`
// because a new integration file is invisible to all four `make test-skills`
// selectors and would need a fifth Makefile selector to run at all.

use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};

fn tracer_workflow(description: &str) -> SequentialWorkflow {
    SequentialWorkflow::new("refund_flow", description)
        .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
}

#[tokio::test]
async fn projected_workflow_skill_reads_back_byte_identical() {
    let skill = tracer_workflow("Process a refund").as_skill();

    // Anti-vacuity: a projection that rendered nothing would otherwise make
    // the byte-identity assertion below trivially true.
    assert!(
        skill.body().len() > 120,
        "projected body was only {} bytes",
        skill.body().len()
    );
    assert!(
        skill.body().starts_with("---\nname: \"refund-flow\"\n"),
        "projected body began: {:?}",
        &skill.body()[..skill.body().len().min(60)]
    );

    let handler = Skills::new()
        .add(skill.clone())
        .into_handler()
        .expect("a projected skill must register cleanly");

    let result = handler
        .read(
            "skill://refund-flow/SKILL.md",
            RequestHandlerExtra::default(),
        )
        .await
        .expect("the projected skill's SKILL.md must read");
    let (uri, text, mime) = extract_resource(&result.contents);

    assert_eq!(uri, "skill://refund-flow/SKILL.md");
    assert_eq!(mime, "text/markdown");
    assert_eq!(
        text,
        skill.body(),
        "the served bytes must equal the projected body exactly"
    );
}

/// The same proof for a description that would break a RAW frontmatter
/// concatenation (REVIEWS finding 1): it carries a `: ` mapping indicator AND
/// a `#` comment indicator. `into_handler()` returning `Ok` is the load-bearing
/// half — it proves the skill did NOT take the diagnostic-downgrade path that
/// silently skips the SC-1 name-identity check.
#[tokio::test]
async fn projected_skill_with_an_awkward_description_still_registers_and_reads() {
    let workflow = tracer_workflow("Refund an order: fast path #urgent");
    let skill = workflow.as_skill();

    assert_eq!(skill.name(), "refund-flow");
    assert_eq!(skill.resolved_description(), workflow.description());

    let handler = Skills::new()
        .add(skill.clone())
        .into_handler()
        .expect("an awkward description must not fail registration");

    let result = handler
        .read(
            "skill://refund-flow/SKILL.md",
            RequestHandlerExtra::default(),
        )
        .await
        .expect("the projected skill's SKILL.md must read");
    let (_uri, text, _mime) = extract_resource(&result.contents);
    assert_eq!(text, skill.body());

    // The registry's own name-identity validation runs inside `entries()`;
    // reaching it at all requires the frontmatter to have PARSED.
    let entries = Skills::new()
        .add(skill)
        .entries()
        .expect("entries must build for a projected skill");
    assert_eq!(entries.len(), 1);
}

// ── Phase 126 plan 05 (D-04a): the projected-skill prepend ────────────
//
// These tests live HERE, and NOT in `prompt_handler.rs`'s `mod tests`, for a
// measured reason: that module path matches no `make test-skills` selector, and
// the tests would be `#[cfg(feature = "skills")]`-conditional so no
// `--features "full"` leg reaches them either. They would pass locally and be
// invisible to the entire quality gate. `tests/workflow_prompt_e2e_test.rs` is
// wrong for the same class of reason — its `#![cfg]` header is
// `streamable-http`-gated and socket-based, not skills-gated.
//
// The harness is the in-process one from `prompt_handler.rs`'s own tests,
// copied rather than referenced: `SimpleTool` -> `metadata()` ->
// `workflow::conversion::ToolInfo` -> `tools` / `tool_handlers` maps ->
// `WorkflowPromptHandler::new(..)` -> `handler.handle(args, extra).await`.
// `Server::handle_request` is private, so the trait method is the only
// in-process route (CONVENTIONS.md:102-106).

use async_trait::async_trait;
use pmcp::server::tasks::TaskRouter;
use pmcp::server::workflow::conversion::ToolInfo as WorkflowToolInfo;
use pmcp::server::workflow::{TaskWorkflowPromptHandler, WorkflowPromptHandler};
use pmcp::types::{PromptMessage, Role};
use pmcp::{PromptHandler, SimpleTool, ToolHandler};
use serde_json::Value;

/// Two tool steps, so the suppressed assistant-plan message has a step list
/// worth suppressing and the projected `## Procedure` has something to
/// duplicate.
fn prepend_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new("refund_flow", "process a refund")
        .argument("order_id", "Order identifier", true)
        .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
        .step(WorkflowStep::new("issue_refund", ToolHandle::new("refunds_create")).bind("refund"))
}

fn prepend_args() -> HashMap<String, String> {
    let mut args = HashMap::new();
    args.insert("order_id".to_string(), "ord-42".to_string());
    args
}

fn register_tool<T: ToolHandler + 'static>(
    tools: &mut HashMap<Arc<str>, WorkflowToolInfo>,
    handlers: &mut HashMap<Arc<str>, Arc<dyn ToolHandler>>,
    tool: T,
) {
    let metadata = tool.metadata().expect("SimpleTool always reports metadata");
    let name: Arc<str> = Arc::from(metadata.name.as_str());
    tools.insert(
        Arc::clone(&name),
        WorkflowToolInfo {
            name: metadata.name.clone(),
            description: metadata.description.clone().unwrap_or_default(),
            input_schema: metadata.input_schema.clone(),
        },
    );
    handlers.insert(name, Arc::new(tool));
}

#[allow(clippy::type_complexity)]
fn prepend_registry() -> (
    HashMap<Arc<str>, WorkflowToolInfo>,
    HashMap<Arc<str>, Arc<dyn ToolHandler>>,
) {
    let mut tools = HashMap::new();
    let mut handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>> = HashMap::new();

    register_tool(
        &mut tools,
        &mut handlers,
        SimpleTool::new("orders_get", |_args, _extra| {
            Box::pin(async move { Ok(serde_json::json!({"order": {"id": "ord-42"}})) })
        })
        .with_description("Fetch an order")
        .with_schema(serde_json::json!({"type": "object"})),
    );
    register_tool(
        &mut tools,
        &mut handlers,
        SimpleTool::new("refunds_create", |_args, _extra| {
            Box::pin(async move { Ok(serde_json::json!({"refund": {"id": "rf-1"}})) })
        })
        .with_description("Create a refund")
        .with_schema(serde_json::json!({"type": "object"})),
    );

    (tools, handlers)
}

/// The opt-in left at its DEFAULT — the setter is never called. This is the
/// shape every server registered before D-04a has, and its transcript is the
/// byte-identity baseline.
fn handler_default() -> WorkflowPromptHandler {
    let (tools, handlers) = prepend_registry();
    WorkflowPromptHandler::new(prepend_workflow(), tools, handlers, None)
}

/// The opt-in explicitly OFF. Must be indistinguishable from the default.
fn handler_prepend_off() -> WorkflowPromptHandler {
    let (tools, handlers) = prepend_registry();
    WorkflowPromptHandler::new(prepend_workflow(), tools, handlers, None)
        .with_projected_skill_prepend(false)
}

/// The opt-in explicitly ON.
fn handler_prepend_on() -> WorkflowPromptHandler {
    let (tools, handlers) = prepend_registry();
    WorkflowPromptHandler::new(prepend_workflow(), tools, handlers, None)
        .with_projected_skill_prepend(true)
}

fn message_text(message: &PromptMessage) -> &str {
    match &message.content {
        Content::Text { text } => text.as_str(),
        other => panic!("expected Content::Text in a transcript, got {other:?}"),
    }
}

async fn run(handler: &dyn PromptHandler) -> pmcp::types::GetPromptResult {
    handler
        .handle(prepend_args(), RequestHandlerExtra::default())
        .await
        .expect("the fixture workflow must execute cleanly")
}

/// D-04's unchanged-execution property. Asserts ROLE and TEXT for every
/// message, never the count alone — a count-only assertion cannot tell a
/// suppressed assistant plan from a dropped guidance message.
#[tokio::test]
async fn flag_off_transcript_is_unchanged() {
    let result = run(&handler_default()).await;
    let messages = &result.messages;

    // Anti-vacuity: an empty transcript would satisfy any per-message loop.
    assert!(
        messages.len() >= 2,
        "flag-off transcript collapsed to {} message(s)",
        messages.len()
    );
    assert_eq!(
        messages.len(),
        6,
        "expected intent + plan + (announce, result) x 2"
    );

    // [0] user intent — carries the caller's actual argument VALUES.
    assert_eq!(messages[0].role, Role::User);
    let intent = message_text(&messages[0]);
    assert!(
        intent.starts_with("I want to process a refund."),
        "intent began: {intent:?}"
    );
    assert!(intent.contains("Parameters:"), "intent was: {intent:?}");
    assert!(
        intent.contains("order_id: \"ord-42\""),
        "intent was: {intent:?}"
    );

    // [1] assistant plan — the step-and-tool list D-04a suppresses when ON.
    assert_eq!(messages[1].role, Role::Assistant);
    let plan = message_text(&messages[1]);
    assert!(plan.starts_with("Here's my plan:\n"), "plan was: {plan:?}");
    assert!(
        plan.contains("1. orders_get - Fetch an order"),
        "plan was: {plan:?}"
    );
    assert!(
        plan.contains("2. refunds_create - Create a refund"),
        "plan was: {plan:?}"
    );

    // [2..] tool-call announcement / tool result pairs, unchanged.
    assert_eq!(messages[2].role, Role::Assistant);
    assert!(message_text(&messages[2]).contains("Calling tool 'orders_get'"));
    assert_eq!(messages[3].role, Role::User);
    assert!(message_text(&messages[3]).contains("Tool result"));
    assert_eq!(messages[4].role, Role::Assistant);
    assert!(message_text(&messages[4]).contains("Calling tool 'refunds_create'"));
    assert_eq!(messages[5].role, Role::User);
    assert!(message_text(&messages[5]).contains("Tool result"));

    // An explicit `false` must be indistinguishable from never calling the
    // setter at all — the opt-in's default is off, not merely usually off.
    let explicit_off = run(&handler_prepend_off()).await;
    assert_eq!(explicit_off.messages.len(), messages.len());
    for (a, b) in explicit_off.messages.iter().zip(messages.iter()) {
        assert_eq!(a.role, b.role);
        assert_eq!(message_text(a), message_text(b));
    }
}

/// D-04a / D-05: message [0] IS the projected body, byte for byte.
#[tokio::test]
async fn flag_on_message_zero_is_the_skill_body() {
    let body = prepend_workflow().as_skill().body().to_string();

    // Anti-vacuity: an empty body would make the equality below trivial.
    assert!(
        body.len() > 200,
        "projected body was only {} bytes",
        body.len()
    );

    let result = run(&handler_prepend_on()).await;
    assert_eq!(result.messages[0].role, Role::User);
    assert_eq!(message_text(&result.messages[0]), body);
}

/// D-04a's rejected option (c) was NOT taken: `create_user_intent` survives at
/// [1], because its `Parameters:` block is the only place the caller's actual
/// argument VALUES appear. The projected body carries argument SPECS.
#[tokio::test]
async fn flag_on_keeps_user_intent_at_index_one() {
    let result = run(&handler_prepend_on()).await;

    assert_eq!(result.messages[1].role, Role::User);
    let intent = message_text(&result.messages[1]);
    assert!(
        intent.starts_with("I want to process a refund."),
        "intent began: {intent:?}"
    );
    assert!(intent.contains("Parameters:"), "intent was: {intent:?}");
    assert!(
        intent.contains("order_id: \"ord-42\""),
        "intent was: {intent:?}"
    );

    // The distinction that makes suppressing [1] information-destroying: the
    // runtime VALUE appears in the intent and nowhere in the body.
    let body = message_text(&result.messages[0]);
    assert!(
        !body.contains("ord-42"),
        "the projected body must carry argument SPECS, not runtime VALUES"
    );
    assert!(
        body.contains("order_id"),
        "the projected body must name the argument"
    );
}

/// The assistant-plan MESSAGE is gone with the flag on, and the transcript is
/// exactly as long as the flag-off one (one message added, one removed).
#[tokio::test]
async fn flag_on_suppresses_assistant_plan() {
    let off = run(&handler_default()).await;
    let on = run(&handler_prepend_on()).await;

    let plan_text = message_text(&off.messages[1]).to_string();
    assert!(plan_text.starts_with("Here's my plan:\n"));

    assert!(
        on.messages
            .iter()
            .all(|m| message_text(m) != plan_text.as_str()),
        "the assistant-plan message survived with the flag on"
    );
    assert_eq!(
        on.messages.len(),
        off.messages.len(),
        "one message prepended, one suppressed"
    );
}

/// Stub router whose ONLY job is to put `TaskWorkflowPromptHandler::handle` on
/// its INDEPENDENT branch. Every real `TaskRouter` impl in this tree is private
/// to a test module, so none is reachable from here.
struct PrependTestTaskRouter;

#[async_trait]
impl TaskRouter for PrependTestTaskRouter {
    async fn handle_task_call(
        &self,
        _tool_name: &str,
        _arguments: Value,
        _task_params: Value,
        _owner_id: &str,
        _progress_token: Option<Value>,
    ) -> pmcp::error::Result<Value> {
        Err(pmcp::Error::internal("unused by this test"))
    }

    async fn handle_tasks_get(
        &self,
        _params: Value,
        _owner_id: &str,
    ) -> pmcp::error::Result<Value> {
        Err(pmcp::Error::internal("unused by this test"))
    }

    async fn handle_tasks_result(
        &self,
        _params: Value,
        _owner_id: &str,
    ) -> pmcp::error::Result<Value> {
        Err(pmcp::Error::internal("unused by this test"))
    }

    async fn handle_tasks_list(
        &self,
        _params: Value,
        _owner_id: &str,
    ) -> pmcp::error::Result<Value> {
        Err(pmcp::Error::internal("unused by this test"))
    }

    async fn handle_tasks_cancel(
        &self,
        _params: Value,
        _owner_id: &str,
    ) -> pmcp::error::Result<Value> {
        Err(pmcp::Error::internal("unused by this test"))
    }

    fn resolve_owner(
        &self,
        _subject: Option<&str>,
        _client_id: Option<&str>,
        _session_id: Option<&str>,
    ) -> String {
        "prepend-test-owner".to_string()
    }

    fn tool_requires_task(&self, _tool_name: &str, _tool_execution: Option<&Value>) -> bool {
        false
    }

    fn task_capabilities(&self) -> Value {
        serde_json::json!({})
    }

    /// The one method that must SUCCEED. Its return shape is exactly what the
    /// task-id extractor reads.
    async fn create_workflow_task(
        &self,
        _workflow_name: &str,
        _owner_id: &str,
        _progress: Value,
    ) -> pmcp::error::Result<Value> {
        Ok(serde_json::json!({"task": {"taskId": "task-prepend-test-1"}}))
    }
}

/// RESEARCH Pitfall 6: a prepend added to only one of the two prompt handlers
/// would make `has_task_support(true)` workflows behave differently from plain
/// ones — the exact drift this projection exists to prevent.
///
/// Branch selection, quoted from `task_prompt_handler.rs`: `handle` calls
/// `create_workflow_task`, extracts `value["task"]["taskId"].as_str()`, and if
/// that yields `None` it runs
/// `let Some(task_id) = task_id else { return self.inner.handle(args, extra).await }`
/// and DELEGATES. The stub above makes the extraction succeed, so the
/// independent branch runs. The `_meta` assertion below is the proof: only the
/// independent branch enriches its result with `_meta` (step 7 of `handle`);
/// the delegating branch returns the inner result unchanged and carries none.
#[tokio::test]
async fn both_handlers_produce_the_same_message_zero() {
    let plain = run(&handler_prepend_on()).await;

    let task_handler = TaskWorkflowPromptHandler::new(
        handler_prepend_on(),
        Arc::new(PrependTestTaskRouter) as Arc<dyn TaskRouter>,
        prepend_workflow(),
    );
    let tasked = run(&task_handler).await;

    let meta = tasked._meta.as_ref().expect(
        "no _meta: the task handler took the DELEGATING branch, so this test proved nothing",
    );
    assert_eq!(
        meta.get("task_id").and_then(Value::as_str),
        Some("task-prepend-test-1"),
        "_meta did not carry the stub's minted task id"
    );
    assert!(
        meta.contains_key("steps"),
        "_meta must carry the independent branch's step plan"
    );

    assert_eq!(
        message_text(&tasked.messages[0]),
        message_text(&plain.messages[0]),
        "the two handlers disagreed on message [0]"
    );
    assert_eq!(tasked.messages[0].role, plain.messages[0].role);
}

/// D-05's one-string claim, proven across BOTH surfaces in one test: the bytes
/// served at `skill://{slug}/SKILL.md` through the real `ResourceHandler` are
/// the bytes the prompt transcript opens with.
#[tokio::test]
async fn prepended_body_equals_served_skill_bytes() {
    let skill = prepend_workflow().as_skill();
    let uri = format!("skill://{}/SKILL.md", skill.name());

    let resource_handler = Skills::new()
        .add(skill)
        .into_handler()
        .expect("a projected skill must register cleanly");
    let read = resource_handler
        .read(&uri, RequestHandlerExtra::default())
        .await
        .expect("the projected skill's SKILL.md must read");
    let (read_uri, served, mime) = extract_resource(&read.contents);
    assert_eq!(read_uri, uri);
    assert_eq!(mime, "text/markdown");
    assert!(served.len() > 200, "served body was {} bytes", served.len());

    let transcript = run(&handler_prepend_on()).await;
    assert_eq!(message_text(&transcript.messages[0]), served);
}

/// REVIEWS fable (a): enabling the flag must not delete the unregistered-tool
/// validation. `create_assistant_plan()` is where that error is raised, so the
/// CALL is kept and only its message is suppressed. Both flag states must
/// therefore fail identically.
#[tokio::test]
async fn flag_on_still_rejects_an_unregistered_tool() {
    // Empty registry: every step names a tool absent from `tools`.
    let on = WorkflowPromptHandler::new(prepend_workflow(), HashMap::new(), HashMap::new(), None)
        .with_projected_skill_prepend(true);
    let err_on = on
        .handle(prepend_args(), RequestHandlerExtra::default())
        .await
        .expect_err("flag ON must still reject an unregistered tool");
    assert!(
        err_on.to_string().contains("not found in registry"),
        "flag-ON error was: {err_on}"
    );

    let off = WorkflowPromptHandler::new(prepend_workflow(), HashMap::new(), HashMap::new(), None);
    let err_off = off
        .handle(prepend_args(), RequestHandlerExtra::default())
        .await
        .expect_err("flag OFF must reject an unregistered tool");
    assert!(
        err_off.to_string().contains("not found in registry"),
        "flag-OFF error was: {err_off}"
    );

    assert_eq!(
        err_on.to_string(),
        err_off.to_string(),
        "the two flag states must fail identically"
    );
}

// ── Phase 126 plan 05 Task 4 (GATE B = `add-builder-path`) ────────────
//
// REVIEWS finding 2: an opt-in reachable only by hand-constructing a
// `WorkflowPromptHandler` makes the anti-drift claim true per workflow VALUE
// and false per SERVER. These two tests are the difference between the claim
// being proven and merely asserted: one reaches message [0] through the NORMAL
// registration API, the other proves the default is still off.

/// Drive a `ServerCoreBuilder`-built core to a `prompts/get` and return the
/// first message's text.
///
/// `ServerCore` exposes no prompt accessor — its only ingress is
/// `ProtocolHandler::handle_request` — and a v1 core gates every non-`initialize`
/// request behind the handshake, so the handshake runs first.
async fn core_builder_first_prompt_message(prepend: bool) -> String {
    use pmcp::server::core::ProtocolHandler;
    use pmcp::types::jsonrpc::ResponsePayload;
    use pmcp::types::{ClientRequest, Request, RequestId};

    let mut builder = pmcp::server::builder::ServerCoreBuilder::new()
        .name("prepend-core-fixture")
        .version("1.0.0")
        // Tools must be registered BEFORE `prompt_workflow`, which snapshots the
        // tool registry at registration time; without them
        // `create_assistant_plan()?` fails and no transcript is produced at all.
        .tool(
            "orders_get",
            SimpleTool::new("orders_get", |_args, _extra| {
                Box::pin(async move { Ok(serde_json::json!({"order": {"id": "ord-42"}})) })
            })
            .with_description("Fetch an order")
            .with_schema(serde_json::json!({"type": "object"})),
        )
        .tool(
            "refunds_create",
            SimpleTool::new("refunds_create", |_args, _extra| {
                Box::pin(async move { Ok(serde_json::json!({"refund": {"id": "rf-1"}})) })
            })
            .with_description("Create a refund")
            .with_schema(serde_json::json!({"type": "object"})),
        );

    if prepend {
        // Ordering is load-bearing and documented on the setter: it applies to
        // workflows registered AFTER this call.
        builder = builder.with_workflow_skill_prepend(true);
    }

    let core = builder
        .prompt_workflow(prepend_workflow())
        .expect("the fixture workflow validates")
        .build()
        .expect("the core builds");
    let core: Arc<dyn ProtocolHandler> = Arc::new(core);

    let init: ClientRequest = serde_json::from_value(serde_json::json!({
        "method": "initialize",
        "params": {
            "protocolVersion": pmcp::LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "prepend-fixture", "version": "0.0.0" },
        },
    }))
    .expect("initialize deserializes into ClientRequest");
    let handshake = core
        .handle_request(RequestId::from(0i64), Request::Client(Box::new(init)), None)
        .await;
    assert!(
        matches!(handshake.payload, ResponsePayload::Result(_)),
        "the handshake must succeed before the prompts/get below means anything"
    );

    let get: ClientRequest = serde_json::from_value(serde_json::json!({
        "method": "prompts/get",
        "params": { "name": "refund_flow", "arguments": { "order_id": "ord-42" } },
    }))
    .expect("prompts/get deserializes into ClientRequest");
    let response = core
        .handle_request(RequestId::from(1i64), Request::Client(Box::new(get)), None)
        .await;
    let ResponsePayload::Result(result) = response.payload else {
        panic!("a ServerCoreBuilder core answers prompts/get, got {response:?}");
    };

    result["messages"][0]["content"]["text"]
        .as_str()
        .expect("message [0] carries text")
        .to_string()
}

/// GATE B, `ServerCoreBuilder` half: the prepend is reachable from
/// `prompt_workflow`, so the anti-drift guarantee holds per SERVER.
#[tokio::test]
async fn server_core_builder_prompt_workflow_reaches_the_prepend() {
    let body = prepend_workflow().as_skill().body().to_string();
    assert!(body.len() > 200, "projected body was {} bytes", body.len());

    assert_eq!(core_builder_first_prompt_message(true).await, body);
}

/// ...and the default is OFF, so an existing server's transcript does not move.
#[tokio::test]
async fn server_core_builder_prompt_workflow_defaults_to_no_prepend() {
    let body = prepend_workflow().as_skill().body().to_string();
    let first = core_builder_first_prompt_message(false).await;

    assert_ne!(first, body, "the prepend must not be default-on");
    assert!(
        first.starts_with("I want to process a refund."),
        "message [0] must still be the user intent, got: {first:?}"
    );
}

/// Register the two fixture tools on a `ServerBuilder`. `prompt_workflow`
/// snapshots the tool registry at registration time, so this must run BEFORE it
/// or `create_assistant_plan()?` fails and no transcript is produced at all.
fn with_fixture_tools(builder: pmcp::ServerBuilder) -> pmcp::ServerBuilder {
    builder
        .tool(
            "orders_get",
            SimpleTool::new("orders_get", |_args, _extra| {
                Box::pin(async move { Ok(serde_json::json!({"order": {"id": "ord-42"}})) })
            })
            .with_description("Fetch an order")
            .with_schema(serde_json::json!({"type": "object"})),
        )
        .tool(
            "refunds_create",
            SimpleTool::new("refunds_create", |_args, _extra| {
                Box::pin(async move { Ok(serde_json::json!({"refund": {"id": "rf-1"}})) })
            })
            .with_description("Create a refund")
            .with_schema(serde_json::json!({"type": "object"})),
        )
}

/// GATE B, `ServerBuilder` half. This builder has no task wrap, and
/// `Server::get_prompt` hands back the registered handler directly, so the
/// transcript is reachable without the request/response round trip above.
#[tokio::test]
async fn server_builder_prompt_workflow_reaches_the_prepend() {
    let body = prepend_workflow().as_skill().body().to_string();

    let server = with_fixture_tools(
        pmcp::Server::builder()
            .name("prepend-server-fixture")
            .version("1.0.0"),
    )
    .with_workflow_skill_prepend(true)
    .prompt_workflow(prepend_workflow())
    .expect("the fixture workflow validates")
    .build()
    .expect("the server builds");
    let prompt = server
        .get_prompt("refund_flow")
        .expect("prompt_workflow registered the handler");
    let result = prompt
        .handle(prepend_args(), RequestHandlerExtra::default())
        .await
        .expect("the workflow prompt resolves");

    assert_eq!(message_text(&result.messages[0]), body);

    // Negative case: the same construction WITHOUT the setter is flag-off.
    let plain = with_fixture_tools(
        pmcp::Server::builder()
            .name("prepend-server-fixture")
            .version("1.0.0"),
    )
    .prompt_workflow(prepend_workflow())
    .expect("the fixture workflow validates")
    .build()
    .expect("the server builds");
    let plain_result = plain
        .get_prompt("refund_flow")
        .expect("prompt_workflow registered the handler")
        .handle(prepend_args(), RequestHandlerExtra::default())
        .await
        .expect("the workflow prompt resolves");
    assert_ne!(message_text(&plain_result.messages[0]), body);
    assert!(message_text(&plain_result.messages[0]).starts_with("I want to process a refund."));
}
