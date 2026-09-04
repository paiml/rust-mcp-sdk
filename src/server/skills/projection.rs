//! Deterministic [`SequentialWorkflow`] -> SEP-2640 [`Skill`] projection.
//!
//! A workflow already carries everything a manual runner needs — a name, a
//! description, argument specs, and an ordered list of steps naming tools and
//! bindings. This module renders that same content as an agentskills-legal
//! `SKILL.md` body, so the served skill and the served prompt are one content
//! rendered twice rather than two documents that can drift apart (D-03).
//!
//! [`SequentialWorkflow`]: crate::server::workflow::SequentialWorkflow
//! [`Skill`]: crate::server::skills::Skill
//!
//! # The rendered text is NOT semver-stable
//!
//! The exact bytes this module produces are pinned by a golden test so that no
//! change is accidental, but they are explicitly **not** part of the crate's
//! semver contract: the render may change on any minor bump (D-14). It changes
//! with a CHANGELOG entry every time, because the bytes become the `sha256`
//! digest published in the skill's `skills/list` entry, and a consumer that
//! pinned that digest must re-pin. A digest mismatch is a fatal pre-loop
//! revocation for such a consumer, not a warning — so a silent render change is
//! a supply-chain event, not a cosmetic one.
//!
//! # Two workflow accessors are excluded from the render on purpose
//!
//! [`SequentialWorkflow::has_task_support`] and [`WorkflowStep::is_retryable`]
//! are server-execution mechanics with no manual analogue: a human or a client
//! LLM following the rendered procedure by hand neither schedules a task nor
//! retries a step the way the server's own executor does. Rendering them would
//! put facts in the body that the reader cannot act on. They are excluded
//! deliberately, and a test asserts they do not appear (D-11).
//!
//! [`SequentialWorkflow::has_task_support`]: crate::server::workflow::SequentialWorkflow::has_task_support
//! [`WorkflowStep::is_retryable`]: crate::server::workflow::WorkflowStep::is_retryable
//!
//! # Frontmatter is written as encoded YAML scalars, never by concatenation
//!
//! Both frontmatter values go through [`yaml_double_quoted`]. A workflow
//! description is arbitrary author text, and pushing it raw after
//! `description: ` lets an ordinary string like `Refund an order: fast path`
//! break the YAML parse — which the registry DOWNGRADES to a diagnostic rather
//! than an error, silently skipping the name-identity check instead of
//! enforcing it. Encoding both values unconditionally removes the whole class.

use crate::server::skills::Skill;
use crate::server::workflow::{SequentialWorkflow, WorkflowStep};

/// A condition the infallible projection path resolved on the author's behalf.
///
/// The crate-private seam returns these instead of logging them, so the
/// fallible builder path can surface them as structured warnings while
/// [`project`] logs them on `mcp.skills`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectionNotice {
    /// The workflow name contained nothing legal; a deterministic
    /// `workflow-{8 hex}` slug was substituted.
    SlugFallback {
        /// The original, un-normalized workflow name.
        original: String,
        /// The substituted slug.
        slug: String,
    },
    /// The workflow description was empty; a deterministic legal substitute
    /// derived from the slug was used.
    EmptyDescription {
        /// The substituted description.
        substituted: String,
    },
}

/// Normalize a workflow name into an agentskills-legal skill name.
fn slugify(_name: &str) -> Option<String> {
    None
}

/// Deterministic `workflow-{8 hex}` slug for a name with nothing legal in it.
fn fallback_slug(_original_name: &str) -> String {
    String::new()
}

/// Replace every control character with `U+FFFD` before the value reaches a
/// `tracing` field (T-126-01).
fn sanitize_for_log(_s: &str) -> String {
    String::new()
}

/// Encode `s` as a YAML double-quoted scalar, INCLUDING its surrounding quotes.
fn yaml_double_quoted(_s: &str) -> String {
    String::new()
}

/// The `---`-delimited frontmatter block: `name` then `description`, both
/// encoded.
fn render_frontmatter(_slug: &str, _description: &str) -> String {
    String::new()
}

/// Render one step as a `### Step {n}: {name}` section.
fn render_step(_index: usize, _step: &WorkflowStep) -> String {
    String::new()
}

/// Render the `## Procedure` section over every step in order.
fn render_procedure(_wf: &SequentialWorkflow) -> String {
    String::new()
}

/// Render the closing `## Server-accelerated alternative` instruction (D-06).
fn render_closing(_wf: &SequentialWorkflow) -> String {
    String::new()
}

/// The description the render actually uses (D-15 / REVIEWS finding 6).
fn resolve_description(_wf: &SequentialWorkflow, _slug: &str) -> String {
    String::new()
}

/// Compose the whole SKILL.md body.
fn render_body(_wf: &SequentialWorkflow, _slug: &str) -> String {
    String::new()
}

/// The crate-private projection seam.
pub(crate) fn project_with_notices(_wf: &SequentialWorkflow) -> (Skill, Vec<ProjectionNotice>) {
    (Skill::new("", ""), Vec::new())
}

/// Project `wf` into a [`Skill`], logging every notice on `mcp.skills`.
pub(crate) fn project(wf: &SequentialWorkflow) -> Skill {
    let (skill, _notices) = project_with_notices(wf);
    skill
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::{ToolHandle, WorkflowStep};
    use proptest::prelude::*;

    fn tracer_workflow(name: &str, description: &str) -> SequentialWorkflow {
        SequentialWorkflow::new(name, description)
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
    }

    /// Parse the projected body's frontmatter through the crate's OWN parser
    /// (which is `serde_yaml`), so the encoder is verified by the decoder the
    /// registry actually uses.
    fn parsed_frontmatter(body: &str) -> serde_json::Value {
        match super::super::parse_frontmatter_value(body) {
            super::super::FrontmatterParse::Parsed(value) => value,
            other => panic!("frontmatter did not parse cleanly: {other:?}\nbody was:\n{body}"),
        }
    }

    /// Assert the finding-1 round-trip for one description.
    fn assert_description_roundtrips(description: &str) {
        let wf = tracer_workflow("refund_flow", description);
        let skill = wf.as_skill();
        let value = parsed_frontmatter(skill.body());
        let obj = value
            .as_object()
            .expect("frontmatter must parse to a mapping");
        assert_eq!(obj.len(), 2, "frontmatter must carry exactly two keys");
        assert_eq!(
            obj.get("name").and_then(|v| v.as_str()),
            Some("refund-flow")
        );
        assert_eq!(
            obj.get("description").and_then(|v| v.as_str()),
            Some(description)
        );
    }

    // ── slugify ───────────────────────────────────────────────────────

    #[test]
    fn slugify_maps_underscores_to_hyphens() {
        assert_eq!(slugify("refund_flow"), Some("refund-flow".to_string()));
    }

    #[test]
    fn slugify_collapses_and_strips_hyphens() {
        assert_eq!(
            slugify("  --Refund  Flow!!--  "),
            Some("refund-flow".to_string())
        );
    }

    #[test]
    fn slugify_rejects_a_name_with_nothing_legal() {
        assert_eq!(slugify("!!!"), None);
    }

    #[test]
    fn slugify_rejects_the_empty_name() {
        assert_eq!(slugify(""), None);
    }

    #[test]
    fn slugify_truncates_to_sixty_four() {
        let slug = slugify(&"a".repeat(90)).expect("ascii letters always survive");
        assert_eq!(slug.len(), 64);
    }

    #[test]
    fn slugify_restrips_a_hyphen_created_by_truncation() {
        let input = format!("{}!x", "a".repeat(63));
        let slug = slugify(&input).expect("ascii letters always survive");
        assert!(!slug.ends_with('-'), "slug was {slug:?}");
        assert_eq!(slug, "a".repeat(63));
    }

    #[test]
    fn fallback_slug_is_shaped_and_stable() {
        let first = fallback_slug("!!!");
        let second = fallback_slug("!!!");
        assert_eq!(first, second, "the fallback must be deterministic");
        let hex = first
            .strip_prefix("workflow-")
            .expect("fallback must be prefixed");
        assert_eq!(hex.len(), 8);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert_ne!(
            fallback_slug("!!!"),
            fallback_slug("???"),
            "distinct names must not collide onto one slug"
        );
    }

    // ── log sanitization (T-126-01) ───────────────────────────────────

    #[test]
    fn sanitize_for_log_replaces_control_characters() {
        assert_eq!(sanitize_for_log("a\nb\u{0}c"), "a\u{fffd}b\u{fffd}c");
        assert_eq!(sanitize_for_log("plain"), "plain");
    }

    // ── SC-1: identity ────────────────────────────────────────────────

    #[test]
    fn as_skill_name_is_the_slugified_workflow_name() {
        let wf = tracer_workflow("refund_flow", "Process a refund");
        assert_eq!(wf.as_skill().name(), "refund-flow");
    }

    // ── SC-5 preconditions + anti-vacuity ─────────────────────────────

    #[test]
    fn projected_body_shape_satisfies_the_sc5_preconditions() {
        let wf = tracer_workflow("refund_flow", "Process a refund");
        let skill = wf.as_skill();
        let body = skill.body();
        assert!(
            body.starts_with("---\nname: \"refund-flow\"\n"),
            "body began: {:?}",
            &body[..body.len().min(60)]
        );
        assert!(body.ends_with('\n'), "body must end with a newline");
        assert!(
            !body.ends_with("\n\n"),
            "body must end with EXACTLY one newline"
        );
        assert!(body.len() > 120, "body was only {} bytes", body.len());
        assert_eq!(skill.references().count(), 0);
        assert_eq!(skill.as_prompt_text(), body);
    }

    #[test]
    fn projected_body_renders_the_procedure() {
        let wf = tracer_workflow("refund_flow", "Process a refund");
        let skill = wf.as_skill();
        let body = skill.body();
        assert!(body.contains("## Procedure"), "body was:\n{body}");
        assert!(
            body.contains("### Step 1: fetch_order"),
            "body was:\n{body}"
        );
        assert!(
            body.contains("Call tool `orders_get`."),
            "body was:\n{body}"
        );
        assert!(
            body.contains("Save the result as `order`."),
            "body was:\n{body}"
        );
        assert!(
            body.contains("## Server-accelerated alternative"),
            "body was:\n{body}"
        );
        assert!(body.contains("`refund_flow`"), "body was:\n{body}");
    }

    // ── D-11: the two deliberate exclusions ───────────────────────────

    #[test]
    fn projected_body_excludes_the_server_execution_mechanics() {
        let wf = SequentialWorkflow::new("refund_flow", "Process a refund")
            .with_task_support(true)
            .step(
                WorkflowStep::new("fetch_order", ToolHandle::new("orders_get"))
                    .bind("order")
                    .retryable(true),
            );
        let body = wf.as_skill().body().to_string();
        assert!(!body.contains("retryable"), "body was:\n{body}");
        assert!(!body.contains("task support"), "body was:\n{body}");
        assert!(!body.contains("task_support"), "body was:\n{body}");
    }

    // ── REVIEWS finding 1: the eight adversarial DESCRIPTION shapes ───

    #[test]
    fn frontmatter_survives_a_mapping_indicator() {
        assert_description_roundtrips("Refund an order: fast path");
    }

    #[test]
    fn frontmatter_survives_a_comment_indicator() {
        assert_description_roundtrips("Handle #123");
    }

    #[test]
    fn frontmatter_survives_a_flow_sequence() {
        assert_description_roundtrips("[urgent] refund");
    }

    #[test]
    fn frontmatter_survives_embedded_quotes() {
        assert_description_roundtrips("\"quoted\"");
    }

    #[test]
    fn frontmatter_survives_a_leading_dash() {
        assert_description_roundtrips("- leading dash");
    }

    #[test]
    fn frontmatter_survives_a_document_delimiter() {
        assert_description_roundtrips("---");
    }

    #[test]
    fn frontmatter_survives_a_newline_injection_attempt() {
        let description = "Refund orders:\nmetadata: injected";
        assert_description_roundtrips(description);
        let wf = tracer_workflow("refund_flow", description);
        let value = parsed_frontmatter(wf.as_skill().body());
        assert!(
            value.get("metadata").is_none(),
            "a newline injected an extra frontmatter key"
        );
    }

    #[test]
    fn frontmatter_survives_a_yaml_boolean_alike_description() {
        assert_description_roundtrips("yes");
    }

    // ── REVIEWS finding 1: the two adversarial NAME shapes ────────────

    #[test]
    fn a_boolean_alike_slug_stays_a_json_string() {
        let wf = tracer_workflow("True", "Process a refund");
        let skill = wf.as_skill();
        assert_eq!(skill.name(), "true");
        let value = parsed_frontmatter(skill.body());
        let name = value.get("name").expect("name key must be present");
        assert!(name.is_string(), "name parsed as {name:?}, not a string");
        assert_eq!(name.as_str(), Some("true"));
    }

    #[test]
    fn an_integer_alike_slug_stays_a_json_string() {
        let wf = tracer_workflow("123", "Process a refund");
        let skill = wf.as_skill();
        assert_eq!(skill.name(), "123");
        let value = parsed_frontmatter(skill.body());
        let name = value.get("name").expect("name key must be present");
        assert!(name.is_string(), "name parsed as {name:?}, not a string");
        assert_eq!(name.as_str(), Some("123"));
    }

    // ── REVIEWS finding 6: the empty description ──────────────────────

    #[test]
    fn empty_description_gets_the_deterministic_fallback() {
        let wf = tracer_workflow("refund_flow", "");
        let skill = wf.as_skill();
        let value = parsed_frontmatter(skill.body());
        assert_eq!(
            value.get("description").and_then(|v| v.as_str()),
            Some("Projected from the refund-flow workflow.")
        );
    }

    // ── the `prompts/list` half of the fix ────────────────────────────

    #[test]
    fn resolved_description_is_the_raw_workflow_description() {
        let description = "Refund an order: fast path with a \" in it";
        let wf = tracer_workflow("refund_flow", description);
        let skill = wf.as_skill();
        assert_eq!(skill.resolved_description(), description);
        assert_eq!(skill.resolved_description(), wf.description());
    }

    // ── yaml_double_quoted, directly ──────────────────────────────────

    #[test]
    fn yaml_double_quoted_escapes_in_order() {
        assert_eq!(yaml_double_quoted("plain"), "\"plain\"");
        assert_eq!(yaml_double_quoted("a\\b"), "\"a\\\\b\"");
        assert_eq!(yaml_double_quoted("a\"b"), "\"a\\\"b\"");
        assert_eq!(yaml_double_quoted("a\nb"), "\"a\\nb\"");
        assert_eq!(yaml_double_quoted("a\rb"), "\"a\\rb\"");
        assert_eq!(yaml_double_quoted("a\tb"), "\"a\\tb\"");
        assert_eq!(yaml_double_quoted("a\u{0}b"), "\"a\\x00b\"");
        assert_eq!(yaml_double_quoted("a\u{7f}b"), "\"a\\x7fb\"");
        // Non-ASCII is carried verbatim — a double-quoted YAML scalar is UTF-8.
        assert_eq!(yaml_double_quoted("héllo — 日本"), "\"héllo — 日本\"");
    }

    // ── REVIEWS finding 1: the round-trip property ────────────────────

    proptest! {
        #[test]
        fn prop_frontmatter_roundtrips(description in ".*") {
            let wf = tracer_workflow("refund_flow", &description);
            let skill = wf.as_skill();
            let value = parsed_frontmatter(skill.body());
            let obj = value.as_object().expect("frontmatter must parse to a mapping");
            prop_assert_eq!(obj.len(), 2);
            prop_assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("refund-flow"));
            let parsed_description = obj.get("description").and_then(|v| v.as_str());
            if description.is_empty() {
                prop_assert_eq!(
                    parsed_description,
                    Some("Projected from the refund-flow workflow.")
                );
            } else {
                prop_assert_eq!(parsed_description, Some(description.as_str()));
            }
        }
    }
}
