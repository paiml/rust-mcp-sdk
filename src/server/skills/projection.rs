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
//! Both frontmatter values go through the module-private `yaml_double_quoted`. A workflow
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
/// fallible builder path can surface them as structured warnings while the
/// logging wrapper emits them on the `mcp.skills` tracing target.
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

/// The maximum skill-name length agentskills permits (D-15a).
const MAX_SLUG_LEN: usize = 64;

/// The deterministic description substituted when a workflow has none.
///
/// Derived from the SLUG rather than the raw workflow name on purpose: the slug
/// is already `[a-z0-9-]`, so the substitute can carry no control character, no
/// quote and no YAML indicator, and needs no escaping of its own.
fn fallback_description(slug: &str) -> String {
    format!("Projected from the {slug} workflow.")
}

/// Normalize a workflow name into an agentskills-legal skill name.
///
/// The rule set is `[a-z0-9-]`, 1..=64 characters, with no leading, trailing or
/// doubled hyphen. Returns `None` when nothing legal survives — the caller then
/// substitutes [`fallback_slug`] (D-15).
fn slugify(name: &str) -> Option<String> {
    let mapped: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    let mut slug = mapped
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        return None;
    }

    if slug.len() > MAX_SLUG_LEN {
        // PANIC-FREE ONLY BECAUSE OF THE MAP ABOVE: every surviving character
        // is ASCII alphanumeric or `-`, so byte index equals char index and
        // `truncate` cannot land inside a multi-byte character. Reordering the
        // steps so this runs before the map reintroduces a panic, and D-15
        // says nothing panics.
        slug.truncate(MAX_SLUG_LEN);
        while slug.ends_with('-') {
            slug.pop();
        }
        if slug.is_empty() {
            return None;
        }
    }

    Some(slug)
}

/// Deterministic `workflow-{8 hex}` slug for a name with nothing legal in it.
///
/// Hashes the ORIGINAL un-normalized name, never the empty normalized string:
/// hashing the latter collides every failing workflow onto one slug, which
/// `Skills::into_handler`'s duplicate-URI check would then reject — converting
/// a warn path into a hard build failure.
fn fallback_slug(original_name: &str) -> String {
    let digest = super::sha256_digest_hex(original_name.as_bytes());
    // Iterator slicing rather than `&digest[..]`: no index can panic, even if
    // the digest format ever changes underneath this function.
    let hex: String = digest.chars().skip("sha256:".len()).take(8).collect();
    format!("workflow-{hex}")
}

/// Replace every control character with `U+FFFD` before the value reaches a
/// `tracing` field (T-126-01).
///
/// A workflow name is author-supplied and reaches a log sink; an embedded
/// newline or terminal escape could forge a second log record. Follows Phase
/// 125's WR-04 mitigation at `src/server/core.rs:2563`.
fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

/// Encode `s` as a YAML double-quoted scalar, INCLUDING its surrounding quotes.
///
/// # Always quoted, never conditionally
///
/// 1. A slug is `[a-z0-9-]`, but that alphabet still contains YAML type-alikes.
///    `slugify("True")` yields `true` and `slugify("123")` yields `123`;
///    unquoted, those parse to a YAML boolean and a YAML integer, the
///    registry's `as_str()` guard returns `None`, and the SC-1 name-identity
///    rule silently SKIPS the skill instead of enforcing it.
/// 2. Conditional quoting would make the emitted bytes depend on a predicate
///    over author text, and those bytes are a published digest (D-14).
///    Unconditional quoting keeps the shape constant.
///
/// The emitter is hand-written rather than delegated to `serde_yaml::to_string`
/// because that crate's quoting style and line wrapping are emitter heuristics,
/// and D-14 makes these bytes a supply-chain pin. `serde_yaml` is used as the
/// test ORACLE instead, through the module's own `parse_frontmatter_value`.
fn yaml_double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Every remaining C0/C1 control as `\xNN`. `char::is_control()` is
            // the Unicode `Cc` category, whose maximum scalar is `U+009F`, so
            // two lowercase hex digits always suffice.
            _ if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            // Everything else verbatim, including all non-ASCII: a
            // double-quoted YAML scalar carries UTF-8 directly, and
            // re-encoding it would break D-13's verbatim-description contract.
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The `---`-delimited frontmatter block: `name` then `description`, both
/// encoded.
///
/// Exactly two keys, in this order, and nothing else — the minimum
/// agentskills-legal set (D-13). Neither value is ever produced by
/// concatenating author text after a `key: ` literal (T-126-21).
fn render_frontmatter(slug: &str, description: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\n---\n",
        yaml_double_quoted(slug),
        yaml_double_quoted(description)
    )
}

/// Render one step as a `### Step {n}: {name}` section.
///
/// `index` is 1-based. A resource-only step carries no tool line; breadth
/// beyond the tool and the binding (arguments, guidance, resources, template
/// bindings) is deliberately absent here and is filled by a later plan — this
/// is a functionality gap, not an architectural one.
fn render_step(index: usize, step: &WorkflowStep) -> String {
    let mut out = format!("### Step {index}: {}\n", step.name());
    if let Some(tool) = step.tool() {
        out.push_str(&format!("Call tool `{}`.\n", tool.name()));
    }
    if let Some(binding) = step.binding() {
        out.push_str(&format!("Save the result as `{binding}`.\n"));
    }
    out.push('\n');
    out
}

/// Render the `## Procedure` section over every step in order.
fn render_procedure(wf: &SequentialWorkflow) -> String {
    let mut out = String::from("## Procedure\n\n");
    for (offset, step) in wf.steps().iter().enumerate() {
        out.push_str(&render_step(offset + 1, step));
    }
    out
}

/// Render the closing `## Server-accelerated alternative` instruction (D-06).
///
/// One instruction addressed to the MODEL, not a cross-reference for a reader:
/// MCP prompts are user-controlled, so a model cannot invoke one and a
/// "prefer the prompt" note would be inert. Its job is closing the DISCOVERY
/// gap for users who do not know the prompt exists. It is conditioned on
/// context the model can actually resolve, so the same bytes read correctly
/// both as a cold skill read and as message [0] of a prompt result.
///
/// Uses the raw workflow name, not the slug — that is the prompt's own name.
fn render_closing(wf: &SequentialWorkflow) -> String {
    let name = wf.name();
    format!(
        "## Server-accelerated alternative\n\n\
         If you are reading this as part of a result from this server's `{name}` prompt, \
         the steps above have already been executed server-side. Otherwise, mention once, \
         at the end of your reply, that this server also offers the `{name}` prompt, \
         which runs these steps server-side.\n"
    )
}

/// The description the render actually uses (D-15 / REVIEWS finding 6).
///
/// `SequentialWorkflow::new` validates nothing, so an empty description is
/// legal input — and `description:` with nothing after it renders YAML null,
/// violating agentskills' non-empty rule. The infallible path substitutes a
/// deterministic legal string; the fallible builder path rejects instead, so a
/// strict caller is pushed to write a description.
///
/// No length bound is imposed: `sep-2640-conformance.md` states the digest
/// format, the per-skill file/byte limits and the name-identity rule, but NO
/// description length limit, and the registry's existing over-limit warning
/// already covers a pathologically large body.
fn resolve_description(wf: &SequentialWorkflow, slug: &str) -> String {
    if wf.description().is_empty() {
        fallback_description(slug)
    } else {
        wf.description().to_string()
    }
}

/// Compose the whole SKILL.md body.
///
/// A straight sequence over leaf helpers and nothing else. The layout is
/// locked: frontmatter, an `# ` heading, `## Procedure` with one section per
/// step, then the closing instruction — later plans extend it but do not
/// rearrange it. The body terminates in exactly one `'\n'`, which is half of
/// what makes `Skill::as_prompt_text() == Skill::body()` hold (SC-5).
fn render_body(wf: &SequentialWorkflow, slug: &str) -> String {
    let description = resolve_description(wf, slug);
    let mut out = String::new();
    out.push_str(&render_frontmatter(slug, &description));
    out.push('\n');
    out.push_str("# ");
    out.push_str(&description);
    out.push_str("\n\n");
    out.push_str(&render_procedure(wf));
    out.push_str(&render_closing(wf));
    out
}

/// The crate-private projection seam.
///
/// Returns the notices instead of logging them so a fallible builder path can
/// surface them as structured data; [`project`] is the logging wrapper. Kept
/// crate-private so a later plan can wrap it without rewriting it.
pub(crate) fn project_with_notices(wf: &SequentialWorkflow) -> (Skill, Vec<ProjectionNotice>) {
    let mut notices = Vec::new();

    let slug = if let Some(slug) = slugify(wf.name()) {
        slug
    } else {
        let slug = fallback_slug(wf.name());
        notices.push(ProjectionNotice::SlugFallback {
            original: wf.name().to_string(),
            slug: slug.clone(),
        });
        slug
    };

    if wf.description().is_empty() {
        notices.push(ProjectionNotice::EmptyDescription {
            substituted: fallback_description(&slug),
        });
    }

    let description = resolve_description(wf, &slug);
    let body = render_body(wf, &slug);

    // NEVER set a URI path override on a projected skill: leaving the path
    // unset is what makes `resolved_path() == slug`, the URI's final segment
    // `== slug`, and the frontmatter `name` `== slug` all agree, so the
    // registry's name-identity validation passes by construction (SC-1).
    //
    // `.with_description(...)` is explicit because `Skill::new` would
    // otherwise derive the description via a `strip_prefix("description: ")`
    // scan with NO YAML decoding, surfacing the ENCODED scalar — quotes and
    // backslashes intact — in `prompts/list`.
    let skill = Skill::new(slug, body).with_description(description);

    (skill, notices)
}

/// Project `wf` into a [`Skill`], logging every notice on `mcp.skills`.
pub(crate) fn project(wf: &SequentialWorkflow) -> Skill {
    let (skill, notices) = project_with_notices(wf);
    for notice in &notices {
        match notice {
            ProjectionNotice::SlugFallback { original, slug } => {
                tracing::warn!(
                    target: "mcp.skills",
                    workflow = %sanitize_for_log(original),
                    slug = %slug,
                    "workflow name has no agentskills-legal characters; \
                     projected skill uses a deterministic fallback slug"
                );
            },
            ProjectionNotice::EmptyDescription { substituted } => {
                tracing::warn!(
                    target: "mcp.skills",
                    workflow = %sanitize_for_log(wf.name()),
                    substituted = %substituted,
                    "workflow description is empty; projected skill uses a \
                     deterministic fallback description"
                );
            },
        }
    }
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
