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
use crate::server::workflow::{DataSource, PromptContent, SequentialWorkflow, WorkflowStep};
use crate::types::PromptArgumentType;
use std::collections::BTreeMap;

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

/// Render one instruction's content as manual-runner prose.
///
/// Every arm emits either author text verbatim or a CONSTANT-shaped line —
/// never `{:?}`. [`PromptContent`] is `#[non_exhaustive]`, so a future upstream
/// variant lands on the catch-all; that arm emits one stable literal precisely
/// so the addition cannot silently change the rendered bytes and invalidate
/// every published `sha256` (D-14).
///
/// The `Image` arm names the MIME type and NOTHING else: the base64 `data` is
/// unbounded, would blow the SEP-2640 16 MiB per-skill limit, and would put
/// megabytes into a digest (T-126-03). The `ToolHandle` arm renders the tool
/// NAME only — no schema, no description (D-12), because the client has
/// `tools/list` one call away and a digested copy could only drift from it.
fn render_prompt_content(content: &PromptContent) -> String {
    match content {
        PromptContent::Text(text) => text.clone(),
        PromptContent::Image { mime_type, .. } => format!("(image content: {mime_type})"),
        PromptContent::ResourceUri(uri) => format!("Read the resource `{uri}`."),
        PromptContent::ResourceHandle(handle) => {
            format!("Read the resource `{}`.", handle.uri())
        },
        PromptContent::ToolHandle(handle) => format!("Uses tool `{}`.", handle.name()),
        // Joined with a blank line, matching what `PromptContent::to_protocol`
        // already does at the wire edge (`conversion.rs:97-106`).
        PromptContent::Multi(parts) => parts
            .iter()
            .map(|part| render_prompt_content(part))
            .collect::<Vec<_>>()
            .join("\n\n"),
        // Why: `PromptContent` is `#[non_exhaustive]`. Within this crate the
        // match above is exhaustive today, so the arm reads as unreachable —
        // but it is the D-14 tripwire that keeps a future variant from
        // reaching a `{:?}` fallback and silently moving the digest.
        #[allow(unreachable_patterns)]
        _ => "(unsupported instruction content)".to_string(),
    }
}

/// Render the `## Context` section from the workflow's instruction messages.
///
/// Emitted only when there are instructions — an empty heading is noise in a
/// document whose bytes are a published digest. Each message is prefixed with
/// its [`Role`](crate::types::Role) rendered through that type's `Display` impl
/// (`src/types/content.rs:816-822`), never `Debug`.
fn render_context(wf: &SequentialWorkflow) -> String {
    if wf.instructions().is_empty() {
        return String::new();
    }
    let mut out = String::from("## Context\n\n");
    for message in wf.instructions() {
        out.push_str(&format!(
            "{}: {}\n\n",
            message.role,
            render_prompt_content(&message.content)
        ));
    }
    out
}

/// The lowercase wire spelling of a prompt-argument type hint.
///
/// [`PromptArgumentType`] derives `Debug` but has **no `Display` impl**, so the
/// reflex formatter for it is `{:?}` — which would put the capitalised variant
/// spelling into a digested body, the exact hazard D-14 forbids. This explicit
/// match emits the type's own `#[serde(rename_all = "lowercase")]` wire
/// spelling instead. The enum is not `#[non_exhaustive]`, so the match is
/// exhaustive with no catch-all: if a variant is added, the compiler points
/// here, which is the desired failure mode.
fn prompt_argument_type_name(arg_type: PromptArgumentType) -> &'static str {
    match arg_type {
        PromptArgumentType::String => "string",
        PromptArgumentType::Number => "number",
        PromptArgumentType::Integer => "integer",
        PromptArgumentType::Boolean => "boolean",
    }
}

/// Render the `## Inputs` section from the workflow's argument specs.
///
/// Emitted only when there are arguments. One bullet per entry of the
/// `IndexMap`, in insertion order — already deterministic, as
/// `sequential.rs:27-28` states in-source.
///
/// Each bullet carries the argument name, the literal word `required` or
/// `optional`, the type hint when `ArgumentSpec::arg_type` is `Some`, and the
/// description. Rendering the type hint is a deliberate choice: CONTEXT.md left
/// "`typed_argument` schema information" to discretion, and the field is public,
/// so it costs nothing and tells a manual runner what shape of value to supply.
fn render_inputs(wf: &SequentialWorkflow) -> String {
    if wf.arguments().is_empty() {
        return String::new();
    }
    let mut out = String::from("## Inputs\n\n");
    for (name, spec) in wf.arguments() {
        let requiredness = if spec.required {
            "required"
        } else {
            "optional"
        };
        let type_note = match spec.arg_type {
            Some(arg_type) => format!(", type `{}`", prompt_argument_type_name(arg_type)),
            None => String::new(),
        };
        out.push_str(&format!(
            "- `{name}` ({requiredness}{type_note}): {}\n",
            spec.description
        ));
    }
    out.push('\n');
    out
}

/// Render where one step argument gets its value from.
///
/// [`DataSource`] is `#[non_exhaustive]`, so the catch-all arm emits one stable
/// literal rather than `{:?}` — same D-14 reasoning as
/// [`render_prompt_content`].
///
/// # Constant key order is digest-significant
///
/// `Constant` renders through `serde_json::to_string` — compact and
/// single-line, because multi-line pretty JSON inside a markdown bullet is a
/// readability and diff hazard. This crate builds `serde_json` with
/// `preserve_order` (`Cargo.toml:119`), so object keys emit in **construction
/// order**. That is deterministic for a given workflow, which is what SC-2
/// requires, but it is NOT canonical: reordering the keys of a `json!` literal
/// in a workflow definition changes the rendered body and therefore the
/// published `sha256`, so it is a CHANGELOG-worthy change under D-14 exactly
/// like a render change.
///
/// The keys are deliberately NOT sorted here. The rendered constant is
/// documentation of what the workflow will actually SEND, and because
/// `preserve_order` is on, the sent JSON is in construction order too — a
/// sorted render would make the manual procedure disagree with the call it
/// describes. `constant_key_order_is_digest_significant` pins the difference so
/// this stays a decision rather than an accident.
fn render_data_source(source: &DataSource) -> String {
    match source {
        DataSource::PromptArg(name) => format!("the `{name}` input"),
        DataSource::StepOutput { step, field: None } => format!("the result of `{step}`"),
        DataSource::StepOutput {
            step,
            field: Some(field),
        } => format!("the `{field}` field of the result of `{step}`"),
        DataSource::Constant(value) => serde_json::to_string(value).map_or_else(
            |_| "an unrenderable constant value".to_string(),
            |json| format!("the constant value `{json}`"),
        ),
        // Why: `DataSource` is `#[non_exhaustive]` — see `render_prompt_content`.
        #[allow(unreachable_patterns)]
        _ => "an unsupported data source".to_string(),
    }
}

/// Render one step as a `### Step {n}: {name}` section.
///
/// `index` is 1-based. A resource-only step carries no tool line (D-11 renders
/// every fact a manual runner needs, and a step with no tool has none to name).
///
/// # The one determinism landmine in the whole input surface
///
/// [`WorkflowStep::template_bindings`] returns a `&HashMap`, and Rust's
/// `HashMap` iteration order is randomized per instance. Every other accessor
/// this renderer reads — `wf.arguments()`, `step.arguments()`, `wf.steps()`,
/// `wf.instructions()`, `step.resources()` — is an `IndexMap` or a slice and is
/// already ordered. The bindings are therefore collected into a [`BTreeMap`]
/// before iterating. Byte order via `String`'s `Ord` is the right key precisely
/// because it is locale-independent: a locale-aware collation would break
/// byte-equality across machines, which is SC-2.
fn render_step(index: usize, step: &WorkflowStep) -> String {
    let mut out = format!("### Step {index}: {}\n", step.name());
    if let Some(tool) = step.tool() {
        out.push_str(&format!("Call tool `{}`.\n", tool.name()));
    }
    for (name, source) in step.arguments() {
        out.push_str(&format!(
            "- Argument `{name}`: {}\n",
            render_data_source(source)
        ));
    }
    let template_bindings: BTreeMap<&str, &DataSource> = step
        .template_bindings()
        .iter()
        .map(|(name, source)| (name.as_str(), source))
        .collect();
    for (name, source) in template_bindings {
        out.push_str(&format!(
            "- Template variable `{name}`: {}\n",
            render_data_source(source)
        ));
    }
    for resource in step.resources() {
        out.push_str(&format!("Read the resource `{}`.\n", resource.uri()));
    }
    if let Some(binding) = step.binding() {
        out.push_str(&format!("Save the result as `{binding}`.\n"));
    }
    if let Some(guidance) = step.guidance() {
        out.push_str(&format!("Judgment: {guidance}\n"));
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
/// locked: frontmatter, an `# ` heading, the optional `## Context` and
/// `## Inputs` sections, `## Procedure` with one section per step, then the
/// closing instruction — later plans extend it but do not rearrange it. The
/// body terminates in exactly one `'\n'`, which is half of what makes
/// `Skill::as_prompt_text() == Skill::body()` hold (SC-5).
fn render_body(wf: &SequentialWorkflow, slug: &str) -> String {
    let description = resolve_description(wf, slug);
    let mut out = String::new();
    out.push_str(&render_frontmatter(slug, &description));
    out.push('\n');
    out.push_str("# ");
    out.push_str(&description);
    out.push_str("\n\n");
    out.push_str(&render_context(wf));
    out.push_str(&render_inputs(wf));
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
    use crate::server::workflow::{
        DataSource, InternalPromptMessage, PromptContent, ToolHandle, WorkflowStep,
    };
    use crate::types::{PromptArgumentType, Role};
    use proptest::prelude::*;
    use serde_json::json;

    fn tracer_workflow(name: &str, description: &str) -> SequentialWorkflow {
        SequentialWorkflow::new(name, description)
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
    }

    /// Render a workflow whose single step binds one argument to `value`.
    fn body_with_constant(value: serde_json::Value) -> String {
        SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(
                WorkflowStep::new("fetch_order", ToolHandle::new("orders_get"))
                    .arg("filter", DataSource::constant(value)),
            )
            .as_skill()
            .body()
            .to_string()
    }

    /// The SC-3 fixture: one workflow exercising every renderable fact.
    ///
    /// Shared by the SC-3 coverage suite and the SC-2 / SC-5 property suite so
    /// the coverage surface and the determinism surface are provably the same
    /// workflow.
    fn kitchen_sink_workflow() -> SequentialWorkflow {
        kitchen_sink_workflow_with_execution_mechanics(false)
    }

    /// The SC-3 fixture, with both D-11-excluded server-execution accessors
    /// driven together.
    ///
    /// `enabled` sets `SequentialWorkflow::with_task_support` AND
    /// `WorkflowStep::retryable` on both tool steps. Driving them together is
    /// what makes the exclusion pin non-vacuous: a fixture that sets only one,
    /// or neither, would compare two bodies that were never going to differ for
    /// a reason the test is claiming to prove.
    fn kitchen_sink_workflow_with_execution_mechanics(enabled: bool) -> SequentialWorkflow {
        SequentialWorkflow::new("refund_flow", "Process a customer refund")
            .with_task_support(enabled)
            .argument("order_id", "The order to refund", true)
            .argument("reason", "Why the customer wants a refund", false)
            .instruction(InternalPromptMessage::new(
                Role::System,
                PromptContent::Text("Never refund more than the original charge.".to_string()),
            ))
            .step(
                WorkflowStep::new("fetch_order", ToolHandle::new("orders_get"))
                    .arg("id", DataSource::prompt_arg("order_id"))
                    .arg("include_lines", DataSource::constant(json!(true)))
                    .bind("order")
                    .retryable(enabled),
            )
            .step(
                WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
                    .arg("amount", DataSource::from_step_field("order", "total"))
                    .with_guidance("Confirm with the customer before issuing the refund.")
                    .with_template_binding(
                        "zeta_region",
                        DataSource::from_step_field("order", "region"),
                    )
                    .with_template_binding("alpha_currency", DataSource::prompt_arg("reason"))
                    .bind("refund")
                    .retryable(enabled),
            )
            .step(
                WorkflowStep::fetch_resources("read_policy")
                    .with_resource("docs://refund-policy")
                    .expect("valid resource URI"),
            )
    }

    /// Render a workflow carrying exactly one instruction message.
    fn body_with_instruction(content: PromptContent) -> String {
        SequentialWorkflow::new("refund_flow", "Process a refund")
            .instruction(InternalPromptMessage::new(Role::System, content))
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
            .as_skill()
            .body()
            .to_string()
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

    // ── SC-3 breadth: `## Context` ────────────────────────────────────

    #[test]
    fn context_renders_instruction_text_verbatim() {
        let body = body_with_instruction(PromptContent::Text("Be careful.".to_string()));
        assert!(body.contains("## Context"), "body was:\n{body}");
        assert!(
            body.contains("Be careful."),
            "instruction text must render verbatim; body was:\n{body}"
        );
        assert!(
            !body.contains("Text("),
            "a Debug-formatted PromptContent leaked into the body:\n{body}"
        );
    }

    #[test]
    fn a_workflow_without_instructions_omits_the_context_heading() {
        let body = tracer_workflow("refund_flow", "Process a refund")
            .as_skill()
            .body()
            .to_string();
        assert!(
            !body.contains("## Context"),
            "an empty Context section must not be emitted; body was:\n{body}"
        );
    }

    #[test]
    fn an_image_instruction_renders_only_its_mime_type() {
        let payload = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo=";
        let body = body_with_instruction(PromptContent::Image {
            data: payload.to_string(),
            mime_type: "image/png".to_string(),
        });
        assert!(
            body.contains("image/png"),
            "the MIME type must render; body was:\n{body}"
        );
        assert!(
            !body.contains(payload),
            "the base64 payload must NEVER reach the body (T-126-03); body was:\n{body}"
        );
    }

    #[test]
    fn a_multi_instruction_renders_every_part_joined_by_a_blank_line() {
        let parts: Vec<Box<PromptContent>> = vec![
            Box::new(PromptContent::Text("First part.".to_string())),
            Box::new(PromptContent::ResourceUri("docs://policy".to_string())),
        ];
        let body = body_with_instruction(PromptContent::Multi(parts.into()));
        assert!(
            body.contains("First part.\n\nRead the resource `docs://policy`."),
            "Multi parts must join with a blank line; body was:\n{body}"
        );
        assert!(
            !body.contains("Multi("),
            "a Debug-formatted PromptContent leaked into the body:\n{body}"
        );
    }

    #[test]
    fn a_tool_handle_instruction_renders_the_tool_name_only() {
        let body = body_with_instruction(PromptContent::ToolHandle(ToolHandle::new("orders_get")));
        assert!(
            body.contains("Uses tool `orders_get`."),
            "body was:\n{body}"
        );
        assert!(
            !body.contains("ToolHandle("),
            "a Debug-formatted PromptContent leaked into the body:\n{body}"
        );
    }

    // ── SC-3 breadth: `## Inputs` ─────────────────────────────────────

    #[test]
    fn inputs_render_name_description_and_requiredness() {
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .argument("order_id", "The order to refund", true)
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
            .as_skill()
            .body()
            .to_string();
        assert!(body.contains("## Inputs"), "body was:\n{body}");
        assert!(body.contains("`order_id`"), "body was:\n{body}");
        assert!(body.contains("The order to refund"), "body was:\n{body}");
        assert!(body.contains("required"), "body was:\n{body}");
    }

    #[test]
    fn an_optional_argument_renders_optional_not_required() {
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .argument("reason", "Why the refund is being issued", false)
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
            .as_skill()
            .body()
            .to_string();
        assert!(body.contains("optional"), "body was:\n{body}");
        assert!(
            !body.contains("required"),
            "an optional argument must not render as required; body was:\n{body}"
        );
    }

    #[test]
    fn a_workflow_without_arguments_omits_the_inputs_heading() {
        let body = tracer_workflow("refund_flow", "Process a refund")
            .as_skill()
            .body()
            .to_string();
        assert!(
            !body.contains("## Inputs"),
            "an empty Inputs section must not be emitted; body was:\n{body}"
        );
    }

    #[test]
    fn typed_arguments_render_the_lowercase_wire_spellings() {
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .typed_argument("count", "How many units", true, PromptArgumentType::Integer)
            .typed_argument(
                "dry_run",
                "Whether to simulate",
                false,
                PromptArgumentType::Boolean,
            )
            .typed_argument(
                "rate",
                "The exchange rate",
                false,
                PromptArgumentType::Number,
            )
            .typed_argument(
                "note",
                "A free-text note",
                false,
                PromptArgumentType::String,
            )
            .step(WorkflowStep::new("fetch_order", ToolHandle::new("orders_get")).bind("order"))
            .as_skill()
            .body()
            .to_string();
        for spelling in ["integer", "boolean", "number", "string"] {
            assert!(
                body.contains(spelling),
                "the `{spelling}` wire spelling must render; body was:\n{body}"
            );
        }
        // The capitalised spellings are `PromptArgumentType`'s `Debug` output.
        // The type has NO `Display` impl, so `{:?}` is the reflex reach and the
        // exact hazard D-14 forbids in a digested body.
        for debug_spelling in ["Integer", "Boolean", "Number", "String"] {
            assert!(
                !body.contains(debug_spelling),
                "`{debug_spelling}` is PromptArgumentType's Debug spelling and must never \
                 reach the body; body was:\n{body}"
            );
        }
    }

    // ── SC-3 breadth: per-step detail ─────────────────────────────────

    #[test]
    fn step_arguments_render_every_name_and_source() {
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .argument("order_id", "The order to refund", true)
            .step(
                WorkflowStep::new("fetch_order", ToolHandle::new("orders_get"))
                    .arg("id", DataSource::prompt_arg("order_id"))
                    .bind("order"),
            )
            .step(
                WorkflowStep::new("issue_refund", ToolHandle::new("payments_refund"))
                    .arg("amount", DataSource::from_step_field("order", "total"))
                    .arg("whole", DataSource::from_step("order"))
                    .bind("refund"),
            )
            .as_skill()
            .body()
            .to_string();
        assert!(body.contains("`id`"), "body was:\n{body}");
        assert!(body.contains("`amount`"), "body was:\n{body}");
        assert!(body.contains("`whole`"), "body was:\n{body}");
        assert!(
            body.contains("the `order_id` input"),
            "the PromptArg source must render; body was:\n{body}"
        );
        assert!(
            body.contains("the `total` field of the result of `order`"),
            "the StepOutput-with-field source must render; body was:\n{body}"
        );
        assert!(
            body.contains("the result of `order`"),
            "the whole-output StepOutput source must render; body was:\n{body}"
        );
        assert!(
            !body.contains("StepOutput"),
            "a Debug-formatted DataSource leaked into the body:\n{body}"
        );
    }

    #[test]
    fn a_constant_renders_in_construction_order() {
        let body = body_with_constant(json!({"b": 2, "a": 1}));
        assert!(
            body.contains(r#"{"b":2,"a":1}"#),
            "serde_json is built with `preserve_order`, so keys must emit in \
             construction order; body was:\n{body}"
        );
    }

    #[test]
    fn constant_key_order_is_digest_significant() {
        let ab = body_with_constant(json!({"a": 1, "b": 2}));
        let ba = body_with_constant(json!({"b": 2, "a": 1}));
        assert_ne!(
            ab, ba,
            "constant key order is digest-significant by DESIGN (REVIEWS gemini \
             finding 4): the rendered constant documents what will actually be SENT, \
             and `preserve_order` means the sent bytes are in construction order too. \
             Sorting the render here would make the documentation disagree with the call."
        );
    }

    #[test]
    fn template_bindings_render_in_sorted_key_order() {
        let step = WorkflowStep::fetch_resources("read_guide")
            .with_resource("docs://guide")
            .expect("valid resource URI")
            .with_template_binding("zeta", DataSource::prompt_arg("z"))
            .with_template_binding("alpha", DataSource::prompt_arg("a"));
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(step)
            .as_skill()
            .body()
            .to_string();
        let alpha = body.find("alpha").expect("alpha binding must render");
        let zeta = body.find("zeta").expect("zeta binding must render");
        assert!(
            alpha < zeta,
            "template bindings must render in sorted key order, not insertion order; \
             body was:\n{body}"
        );
    }

    #[test]
    fn a_resource_only_step_renders_its_heading_and_every_resource() {
        let step = WorkflowStep::fetch_resources("read_policy")
            .with_resource("docs://refund-policy")
            .expect("valid resource URI")
            .with_resource("docs://escalation-matrix")
            .expect("valid resource URI");
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(step)
            .as_skill()
            .body()
            .to_string();
        assert!(
            body.contains("### Step 1: read_policy"),
            "body was:\n{body}"
        );
        assert!(body.contains("docs://refund-policy"), "body was:\n{body}");
        assert!(
            body.contains("docs://escalation-matrix"),
            "body was:\n{body}"
        );
        assert!(
            !body.contains("Call tool"),
            "a resource-only step must not render a tool line; body was:\n{body}"
        );
    }

    #[test]
    fn guidance_renders_a_judgment_line() {
        let body = SequentialWorkflow::new("refund_flow", "Process a refund")
            .step(
                WorkflowStep::new("confirm", ToolHandle::new("notify"))
                    .with_guidance("Confirm with the customer first."),
            )
            .as_skill()
            .body()
            .to_string();
        assert!(
            body.contains("Judgment: Confirm with the customer first."),
            "body was:\n{body}"
        );
    }

    // ── SC-3: string-by-string coverage of every workflow fact ────────

    /// SC-3 / D-11: every renderable workflow fact gets its OWN assertion with
    /// its OWN failure message naming that fact.
    ///
    /// SC-3's wording is explicit — "asserted string-by-string rather than by a
    /// length or substring heuristic" — so a single omnibus `contains` over a
    /// concatenation would be exactly the heuristic it forbids.
    #[test]
    #[allow(clippy::cognitive_complexity)]
    // Why: a flat sequence of independent one-fact assertions. PMAT counts each
    // `assert!` as a branch, but there is no nesting and no control flow to
    // decompose — splitting this into helper fns would hide which fact failed,
    // which is the entire point of SC-3's per-fact failure messages.
    fn sc3_every_workflow_fact_renders() {
        let body = kitchen_sink_workflow().as_skill().body().to_string();

        // Anti-vacuity first: a scan over an empty body proves nothing.
        assert!(
            body.len() > 400,
            "SC-3 anti-vacuity: body was only {} bytes",
            body.len()
        );

        // — identity —
        assert!(
            body.starts_with("---\nname: \"refund-flow\"\n"),
            "fact: the slugified workflow name in the frontmatter `name` key; body was:\n{body}"
        );
        assert!(
            body.contains("Process a customer refund"),
            "fact: the workflow description; body was:\n{body}"
        );

        // — arguments —
        assert!(
            body.contains("`order_id`"),
            "fact: argument name `order_id`; body was:\n{body}"
        );
        assert!(
            body.contains("The order to refund"),
            "fact: the `order_id` description; body was:\n{body}"
        );
        assert!(
            body.contains("`order_id` (required)"),
            "fact: the `required` marker on `order_id`; body was:\n{body}"
        );
        assert!(
            body.contains("`reason`"),
            "fact: argument name `reason`; body was:\n{body}"
        );
        assert!(
            body.contains("Why the customer wants a refund"),
            "fact: the `reason` description; body was:\n{body}"
        );
        assert!(
            body.contains("`reason` (optional)"),
            "fact: the `optional` marker on `reason`; body was:\n{body}"
        );

        // — instructions —
        assert!(
            body.contains("Never refund more than the original charge."),
            "fact: the workflow-level instruction text; body was:\n{body}"
        );

        // — step names —
        assert!(
            body.contains("### Step 1: fetch_order"),
            "fact: step name `fetch_order`; body was:\n{body}"
        );
        assert!(
            body.contains("### Step 2: issue_refund"),
            "fact: step name `issue_refund`; body was:\n{body}"
        );
        assert!(
            body.contains("### Step 3: read_policy"),
            "fact: step name `read_policy` (the resource-only step); body was:\n{body}"
        );

        // — tool names —
        assert!(
            body.contains("Call tool `orders_get`."),
            "fact: tool name `orders_get`; body was:\n{body}"
        );
        assert!(
            body.contains("Call tool `payments_refund`."),
            "fact: tool name `payments_refund`; body was:\n{body}"
        );

        // — argument bindings and their data sources —
        assert!(
            body.contains("- Argument `id`: the `order_id` input"),
            "fact: the `id` binding and its PromptArg source; body was:\n{body}"
        );
        assert!(
            body.contains("- Argument `include_lines`: the constant value `true`"),
            "fact: the `include_lines` binding and its Constant source; body was:\n{body}"
        );
        assert!(
            body.contains("- Argument `amount`: the `total` field of the result of `order`"),
            "fact: the `amount` binding and its StepOutput source; body was:\n{body}"
        );

        // — result bindings —
        assert!(
            body.contains("Save the result as `order`."),
            "fact: the `order` result binding; body was:\n{body}"
        );
        assert!(
            body.contains("Save the result as `refund`."),
            "fact: the `refund` result binding; body was:\n{body}"
        );

        // — template bindings —
        assert!(
            body.contains("- Template variable `alpha_currency`: the `reason` input"),
            "fact: the `alpha_currency` template binding; body was:\n{body}"
        );
        assert!(
            body.contains(
                "- Template variable `zeta_region`: the `region` field of the result of `order`"
            ),
            "fact: the `zeta_region` template binding; body was:\n{body}"
        );

        // — attached resources —
        assert!(
            body.contains("Read the resource `docs://refund-policy`."),
            "fact: the attached resource URI; body was:\n{body}"
        );

        // — guidance —
        assert!(
            body.contains("Judgment: Confirm with the customer before issuing the refund."),
            "fact: the `with_guidance` line; body was:\n{body}"
        );
    }

    // ── D-11: the exclusion pin (REVIEWS fable (f)) ───────────────────

    /// The PRIMARY D-11 exclusion proof: the two excluded server-execution
    /// accessors cannot influence a single rendered byte.
    ///
    /// Asserting only that their NAMES are absent proves little — the render
    /// was never going to print an accessor's identifier. Byte equality across
    /// the two settings is the real invariant.
    #[test]
    fn sc3_excluded_execution_mechanics_change_no_byte() {
        let defaults = kitchen_sink_workflow_with_execution_mechanics(false)
            .as_skill()
            .body()
            .to_string();
        let enabled = kitchen_sink_workflow_with_execution_mechanics(true)
            .as_skill()
            .body()
            .to_string();

        // Anti-vacuity BEFORE the equality: two empty bodies are also equal.
        assert!(
            defaults.len() > 400,
            "D-11 anti-vacuity: the defaults body was only {} bytes",
            defaults.len()
        );
        assert!(
            defaults.starts_with("---\nname: \"refund-flow\"\n"),
            "D-11 anti-vacuity: the defaults body did not begin with the expected \
             frontmatter; body was:\n{defaults}"
        );

        assert_eq!(
            defaults, enabled,
            "D-11: `has_task_support` and `is_retryable` are excluded from the \
             render on purpose, so setting both to non-default values must not \
             change a single byte"
        );
    }

    /// Supplementary readability guard beside the byte-equality pin above.
    /// It no longer carries the D-11 claim.
    #[test]
    fn sc3_excluded_accessor_names_are_absent() {
        let body = kitchen_sink_workflow_with_execution_mechanics(true)
            .as_skill()
            .body()
            .to_string();
        for name in [
            "retryable",
            "is_retryable",
            "task_support",
            "has_task_support",
            "task support",
        ] {
            assert!(
                !body.contains(name),
                "the excluded accessor `{name}` must not appear in the body:\n{body}"
            );
        }
    }

    /// D-12: no tool description and no tool input schema reaches the body.
    ///
    /// The client has `tools/list` one call away; a digested copy of a tool's
    /// description could only drift from the live surface.
    #[test]
    fn sc3_no_tool_schema_or_description_reaches_the_body() {
        let body = kitchen_sink_workflow().as_skill().body().to_string();
        for marker in ["inputSchema", "input_schema", "properties", "\"type\":"] {
            assert!(
                !body.contains(marker),
                "D-12: `{marker}` suggests a tool schema reached the body:\n{body}"
            );
        }
    }

    /// D-13: the frontmatter carries exactly two keys.
    #[test]
    fn sc3_frontmatter_carries_exactly_two_keys() {
        let skill = kitchen_sink_workflow().as_skill();
        let value = parsed_frontmatter(skill.body());
        let obj = value
            .as_object()
            .expect("frontmatter must parse to a mapping");
        assert_eq!(
            obj.len(),
            2,
            "D-13: frontmatter must carry exactly `name` and `description`, got {obj:?}"
        );
        assert!(obj.contains_key("name"), "D-13: missing `name`");
        assert!(
            obj.contains_key("description"),
            "D-13: missing `description`"
        );
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
