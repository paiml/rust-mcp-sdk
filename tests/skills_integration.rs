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
